//! 基于预分配数组（preallocated array）的有界通道。
//!
//! 这种 flavor 拥有一个固定且为正数的容量。
//!
//! 其实现基于 Dmitry Vyukov 的有界 MPMC 队列。
//!
//! 来源：
//!   - <http://www.1024cores.net/home/lock-free-algorithms/queues/bounded-mpmc-queue>
//!   - <https://docs.google.com/document/d/1yIAYmbvL3JxOKOjuCyon7JhW4cSv1wy5hC0ApeGMV9s/pub>

use super::context::Context;
use super::error::*;
use super::select::{Operation, Selected, Token};
use super::utils::{Backoff, CachePadded};
use super::waker::SyncWaker;
use crate::cell::UnsafeCell;
use crate::mem::MaybeUninit;
use crate::ptr;
use crate::sync::atomic::{self, Atomic, AtomicUsize, Ordering};
use crate::time::Instant;

/// 通道中的一个槽位（slot）。
struct Slot<T> {
    /// 当前的戳记（stamp）。
    stamp: Atomic<usize>,

    /// 该槽位中的消息。要么在 `read` 中被读出，要么通过 `discard_all_messages` 被丢弃。
    msg: UnsafeCell<MaybeUninit<T>>,
}

/// array flavor 的 token 类型。
#[derive(Debug)]
pub(crate) struct ArrayToken {
    /// 要读取或写入的槽位。
    slot: *const u8,

    /// 读取或写入之后要存入该槽位的戳记（stamp）。
    stamp: usize,
}

impl Default for ArrayToken {
    #[inline]
    fn default() -> Self {
        ArrayToken { slot: ptr::null(), stamp: 0 }
    }
}

/// 基于预分配数组的有界通道。
pub(crate) struct Channel<T> {
    /// 通道的头部（head）。
    ///
    /// 该值是一个“戳记”（stamp），由缓冲区内的一个索引、一个标记位（mark bit）和一个圈数
    /// （lap）组成，并被打包进单个 `usize`。低位表示索引，高位表示圈数。head 中的标记位
    /// 永远为零。
    ///
    /// 消息从通道的头部弹出（pop）。
    head: CachePadded<Atomic<usize>>,

    /// 通道的尾部（tail）。
    ///
    /// 该值是一个“戳记”（stamp），由缓冲区内的一个索引、一个标记位（mark bit）和一个圈数
    /// （lap）组成，并被打包进单个 `usize`。低位表示索引，高位表示圈数。标记位表示通道
    /// 已断连（disconnected）。
    ///
    /// 消息被压入（push）通道的尾部。
    tail: CachePadded<Atomic<usize>>,

    /// 持有各槽位的缓冲区。
    buffer: Box<[Slot<T>]>,

    /// 通道的容量。
    cap: usize,

    /// 一个值为 `{ lap: 1, mark: 0, index: 0 }` 的戳记。
    one_lap: usize,

    /// 如果该比特位在 tail 中被置位，意味着通道已断连。
    mark_bit: usize,

    /// 在通道已满期间等待的发送者。
    senders: SyncWaker,

    /// 在通道为空且未断连期间等待的接收者。
    receivers: SyncWaker,
}

impl<T> Channel<T> {
    /// 创建一个容量为 `cap` 的有界通道。
    pub(crate) fn with_capacity(cap: usize) -> Self {
        assert!(cap > 0, "capacity must be positive");

        // 计算常量 `mark_bit` 与 `one_lap`。
        let mark_bit = (cap + 1).next_power_of_two();
        let one_lap = mark_bit * 2;

        // head 初始化为 `{ lap: 0, mark: 0, index: 0 }`。
        let head = 0;
        // tail 初始化为 `{ lap: 0, mark: 0, index: 0 }`。
        let tail = 0;

        // 分配一个含 `cap` 个槽位的缓冲区，并为每个槽位初始化戳记。
        let buffer: Box<[Slot<T>]> = (0..cap)
            .map(|i| {
                // 把戳记设为 `{ lap: 0, mark: 0, index: i }`。
                Slot { stamp: AtomicUsize::new(i), msg: UnsafeCell::new(MaybeUninit::uninit()) }
            })
            .collect();

        Channel {
            buffer,
            cap,
            one_lap,
            mark_bit,
            head: CachePadded::new(AtomicUsize::new(head)),
            tail: CachePadded::new(AtomicUsize::new(tail)),
            senders: SyncWaker::new(),
            receivers: SyncWaker::new(),
        }
    }

    /// 尝试为发送一条消息预订（reserve）一个槽位。
    fn start_send(&self, token: &mut Token) -> bool {
        let backoff = Backoff::new();
        let mut tail = self.tail.load(Ordering::Relaxed);

        loop {
            // 检查通道是否已断连。
            if tail & self.mark_bit != 0 {
                token.array.slot = ptr::null();
                token.array.stamp = 0;
                return true;
            }

            // 拆解 tail。
            let index = tail & (self.mark_bit - 1);
            let lap = tail & !(self.one_lap - 1);

            // 查看对应的槽位。
            debug_assert!(index < self.buffer.len());
            let slot = unsafe { self.buffer.get_unchecked(index) };
            let stamp = slot.stamp.load(Ordering::Acquire);

            // 如果 tail 与戳记相匹配，我们就可以尝试压入（push）。
            if tail == stamp {
                let new_tail = if index + 1 < self.cap {
                    // 同一圈，索引加一。
                    // 设为 `{ lap: lap, mark: 0, index: index + 1 }`。
                    tail + 1
                } else {
                    // 前进一圈，索引回绕到零。
                    // 设为 `{ lap: lap.wrapping_add(1), mark: 0, index: 0 }`。
                    lap.wrapping_add(self.one_lap)
                };

                // 尝试移动 tail。
                match self.tail.compare_exchange_weak(
                    tail,
                    new_tail,
                    Ordering::SeqCst,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => {
                        // 为随后调用的 `write` 准备好 token。
                        token.array.slot = slot as *const Slot<T> as *const u8;
                        token.array.stamp = tail + 1;
                        return true;
                    }
                    Err(_) => {
                        backoff.spin_light();
                        tail = self.tail.load(Ordering::Relaxed);
                    }
                }
            } else if stamp.wrapping_add(self.one_lap) == tail + 1 {
                atomic::fence(Ordering::SeqCst);
                let head = self.head.load(Ordering::Relaxed);

                // 如果 head 同样落后 tail 整整一圈……
                if head.wrapping_add(self.one_lap) == tail {
                    // ……那么通道已满。
                    return false;
                }

                backoff.spin_light();
                tail = self.tail.load(Ordering::Relaxed);
            } else {
                // 小睡一下（snooze），因为我们需要等待戳记被更新。
                backoff.spin_heavy();
                tail = self.tail.load(Ordering::Relaxed);
            }
        }
    }

    /// 向通道写入一条消息。
    pub(crate) unsafe fn write(&self, token: &mut Token, msg: T) -> Result<(), T> {
        // 如果没有槽位，说明通道已断连。
        if token.array.slot.is_null() {
            return Err(msg);
        }

        // 把消息写入槽位，并更新戳记。
        unsafe {
            let slot: &Slot<T> = &*(token.array.slot as *const Slot<T>);
            slot.msg.get().write(MaybeUninit::new(msg));
            slot.stamp.store(token.array.stamp, Ordering::Release);
        }

        // 唤醒一个正在睡眠的接收者。
        self.receivers.notify();
        Ok(())
    }

    /// 尝试为接收一条消息预订（reserve）一个槽位。
    fn start_recv(&self, token: &mut Token) -> bool {
        let backoff = Backoff::new();
        let mut head = self.head.load(Ordering::Relaxed);

        loop {
            // 拆解 head。
            let index = head & (self.mark_bit - 1);
            let lap = head & !(self.one_lap - 1);

            // 查看对应的槽位。
            debug_assert!(index < self.buffer.len());
            let slot = unsafe { self.buffer.get_unchecked(index) };
            let stamp = slot.stamp.load(Ordering::Acquire);

            // 如果戳记比 head 超前 1，我们就可以尝试弹出（pop）。
            if head + 1 == stamp {
                let new = if index + 1 < self.cap {
                    // 同一圈，索引加一。
                    // 设为 `{ lap: lap, mark: 0, index: index + 1 }`。
                    head + 1
                } else {
                    // 前进一圈，索引回绕到零。
                    // 设为 `{ lap: lap.wrapping_add(1), mark: 0, index: 0 }`。
                    lap.wrapping_add(self.one_lap)
                };

                // 尝试移动 head。
                match self.head.compare_exchange_weak(
                    head,
                    new,
                    Ordering::SeqCst,
                    Ordering::Relaxed,
                ) {
                    Ok(_) => {
                        // 为随后调用的 `read` 准备好 token。
                        token.array.slot = slot as *const Slot<T> as *const u8;
                        token.array.stamp = head.wrapping_add(self.one_lap);
                        return true;
                    }
                    Err(_) => {
                        backoff.spin_light();
                        head = self.head.load(Ordering::Relaxed);
                    }
                }
            } else if stamp == head {
                atomic::fence(Ordering::SeqCst);
                let tail = self.tail.load(Ordering::Relaxed);

                // 如果 tail 等于 head，意味着通道为空。
                if (tail & !self.mark_bit) == head {
                    // 如果通道已断连……
                    if tail & self.mark_bit != 0 {
                        // ……那么接收到一个错误。
                        token.array.slot = ptr::null();
                        token.array.stamp = 0;
                        return true;
                    } else {
                        // 否则，接收操作尚未就绪。
                        return false;
                    }
                }

                backoff.spin_light();
                head = self.head.load(Ordering::Relaxed);
            } else {
                // 小睡一下（snooze），因为我们需要等待戳记被更新。
                backoff.spin_heavy();
                head = self.head.load(Ordering::Relaxed);
            }
        }
    }

    /// 从通道读取一条消息。
    pub(crate) unsafe fn read(&self, token: &mut Token) -> Result<T, ()> {
        if token.array.slot.is_null() {
            // 通道已断连。
            return Err(());
        }

        // 从槽位读取消息，并更新戳记。
        let msg = unsafe {
            let slot: &Slot<T> = &*(token.array.slot as *const Slot<T>);

            let msg = slot.msg.get().read().assume_init();
            slot.stamp.store(token.array.stamp, Ordering::Release);
            msg
        };

        // 唤醒一个正在睡眠的发送者。
        self.senders.notify();
        Ok(msg)
    }

    /// 尝试向通道发送一条消息。
    pub(crate) fn try_send(&self, msg: T) -> Result<(), TrySendError<T>> {
        let token = &mut Token::default();
        if self.start_send(token) {
            unsafe { self.write(token, msg).map_err(TrySendError::Disconnected) }
        } else {
            Err(TrySendError::Full(msg))
        }
    }

    /// 向通道发送一条消息。
    pub(crate) fn send(
        &self,
        msg: T,
        deadline: Option<Instant>,
    ) -> Result<(), SendTimeoutError<T>> {
        let token = &mut Token::default();
        loop {
            // 尝试发送一条消息。
            if self.start_send(token) {
                let res = unsafe { self.write(token, msg) };
                return res.map_err(SendTimeoutError::Disconnected);
            }

            if let Some(d) = deadline {
                if Instant::now() >= d {
                    return Err(SendTimeoutError::Timeout(msg));
                }
            }

            Context::with(|cx| {
                // 准备阻塞，直到某个接收者把我们唤醒。
                let oper = Operation::hook(token);
                self.senders.register(oper, cx);

                // 通道是不是刚好在此刻变为就绪了？
                if !self.is_full() || self.is_disconnected() {
                    let _ = cx.try_select(Selected::Aborted);
                }

                // 阻塞当前线程。
                // SAFETY: 该上下文属于当前线程。
                let sel = unsafe { cx.wait_until(deadline) };

                match sel {
                    Selected::Waiting => unreachable!(),
                    Selected::Aborted | Selected::Disconnected => {
                        self.senders.unregister(oper).unwrap();
                    }
                    Selected::Operation(_) => {}
                }
            });
        }
    }

    /// 尝试以非阻塞方式接收一条消息。
    pub(crate) fn try_recv(&self) -> Result<T, TryRecvError> {
        let token = &mut Token::default();

        if self.start_recv(token) {
            unsafe { self.read(token).map_err(|_| TryRecvError::Disconnected) }
        } else {
            Err(TryRecvError::Empty)
        }
    }

    /// 从通道接收一条消息。
    pub(crate) fn recv(&self, deadline: Option<Instant>) -> Result<T, RecvTimeoutError> {
        let token = &mut Token::default();
        loop {
            // 尝试接收一条消息。
            if self.start_recv(token) {
                let res = unsafe { self.read(token) };
                return res.map_err(|_| RecvTimeoutError::Disconnected);
            }

            if let Some(d) = deadline {
                if Instant::now() >= d {
                    return Err(RecvTimeoutError::Timeout);
                }
            }

            Context::with(|cx| {
                // 准备阻塞，直到某个发送者把我们唤醒。
                let oper = Operation::hook(token);
                self.receivers.register(oper, cx);

                // 通道是不是刚好在此刻变为就绪了？
                if !self.is_empty() || self.is_disconnected() {
                    let _ = cx.try_select(Selected::Aborted);
                }

                // 阻塞当前线程。
                // SAFETY: 该上下文属于当前线程。
                let sel = unsafe { cx.wait_until(deadline) };

                match sel {
                    Selected::Waiting => unreachable!(),
                    Selected::Aborted | Selected::Disconnected => {
                        self.receivers.unregister(oper).unwrap();
                        // 即便通道已断连，我们仍然必须检查是否还有剩余的消息。
                    }
                    Selected::Operation(_) => {}
                }
            });
        }
    }

    /// 返回通道中当前的消息数量。
    pub(crate) fn len(&self) -> usize {
        loop {
            // 先加载 tail，再加载 head。
            let tail = self.tail.load(Ordering::SeqCst);
            let head = self.head.load(Ordering::SeqCst);

            // 如果 tail 没有改变，说明我们拿到了一对一致（consistent）的值可供计算。
            if self.tail.load(Ordering::SeqCst) == tail {
                let hix = head & (self.mark_bit - 1);
                let tix = tail & (self.mark_bit - 1);

                return if hix < tix {
                    tix - hix
                } else if hix > tix {
                    self.cap - hix + tix
                } else if (tail & !self.mark_bit) == head {
                    0
                } else {
                    self.cap
                };
            }
        }
    }

    /// 返回通道的容量。
    #[allow(clippy::unnecessary_wraps)] // 这是有意为之。
    pub(crate) fn capacity(&self) -> Option<usize> {
        Some(self.cap)
    }

    /// 断开（disconnect）发送者，并唤醒所有被阻塞的接收者。
    ///
    /// 如果本次调用使通道断连，返回 `true`。
    pub(crate) fn disconnect_senders(&self) -> bool {
        let tail = self.tail.fetch_or(self.mark_bit, Ordering::SeqCst);

        if tail & self.mark_bit == 0 {
            self.receivers.disconnect();
            true
        } else {
            false
        }
    }

    /// 断开（disconnect）接收者，并唤醒所有被阻塞的发送者。
    ///
    /// 如果本次调用使通道断连，返回 `true`。
    ///
    /// # Safety
    /// 只能在丢弃最后一个接收者时调用一次。所有其他接收者的销毁都必须已经以 acquire 或更强
    /// 的内存序被观察到。
    pub(crate) unsafe fn disconnect_receivers(&self) -> bool {
        let tail = self.tail.fetch_or(self.mark_bit, Ordering::SeqCst);
        let disconnected = if tail & self.mark_bit == 0 {
            self.senders.disconnect();
            true
        } else {
            false
        };

        unsafe { self.discard_all_messages(tail) };
        disconnected
    }

    /// 丢弃所有消息。
    ///
    /// `tail` 应当是 `tail` 的当前值（因而也是最后一个值）。
    ///
    /// # Panicking
    /// 如果某个析构函数 panic，则剩余的消息会被泄漏（leak），这与无界通道的行为一致。
    ///
    /// # Safety
    /// 此方法只能在丢弃最后一个接收者时调用。所有其他接收者的销毁都必须已经以 acquire 或
    /// 更强的内存序被观察到。
    unsafe fn discard_all_messages(&self, tail: usize) {
        debug_assert!(self.is_disconnected());

        // 只有接收者会修改 `head`，因此既然我们是最后一个，这个值就不会再改变、也不会被
        // 观察到（因为断连之后不可能再发送任何新消息）。
        let mut head = self.head.load(Ordering::Relaxed);
        let tail = tail & !self.mark_bit;

        let backoff = Backoff::new();
        loop {
            // 拆解 head。
            let index = head & (self.mark_bit - 1);
            let lap = head & !(self.one_lap - 1);

            // 查看对应的槽位。
            debug_assert!(index < self.buffer.len());
            let slot = unsafe { self.buffer.get_unchecked(index) };
            let stamp = slot.stamp.load(Ordering::Acquire);

            // 如果戳记比 head 超前 1，我们就可以丢弃这条消息。
            if head + 1 == stamp {
                head = if index + 1 < self.cap {
                    // 同一圈，索引加一。
                    // 设为 `{ lap: lap, mark: 0, index: index + 1 }`。
                    head + 1
                } else {
                    // 前进一圈，索引回绕到零。
                    // 设为 `{ lap: lap.wrapping_add(1), mark: 0, index: 0 }`。
                    lap.wrapping_add(self.one_lap)
                };

                unsafe {
                    (*slot.msg.get()).assume_init_drop();
                }
            // 如果 tail 等于 head，意味着通道为空。
            } else if tail == head {
                return;
            // 否则，说明有一个发送者即将写入该槽位，因此我们需要等待它更新戳记。
            } else {
                backoff.spin_heavy();
            }
        }
    }

    /// 如果通道已断连，返回 `true`。
    pub(crate) fn is_disconnected(&self) -> bool {
        self.tail.load(Ordering::SeqCst) & self.mark_bit != 0
    }

    /// 如果通道为空，返回 `true`。
    pub(crate) fn is_empty(&self) -> bool {
        let head = self.head.load(Ordering::SeqCst);
        let tail = self.tail.load(Ordering::SeqCst);

        // tail 是否等于 head？
        //
        // 注意：如果 head 恰好在我们加载 tail 之前发生了变化，那意味着曾经有那么一刻通道
        // 非空，因此直接返回 `false` 是安全的。
        (tail & !self.mark_bit) == head
    }

    /// 如果通道已满，返回 `true`。
    pub(crate) fn is_full(&self) -> bool {
        let tail = self.tail.load(Ordering::SeqCst);
        let head = self.head.load(Ordering::SeqCst);

        // head 是否落后 tail 整整一圈？
        //
        // 注意：如果 tail 恰好在我们加载 head 之前发生了变化，那意味着曾经有那么一刻通道
        // 未满，因此直接返回 `false` 是安全的。
        head.wrapping_add(self.one_lap) == tail & !self.mark_bit
    }
}
