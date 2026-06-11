//! 以链表（linked list）实现的无界通道。

use super::context::Context;
use super::error::*;
use super::select::{Operation, Selected, Token};
use super::utils::{Backoff, CachePadded};
use super::waker::SyncWaker;
use crate::cell::UnsafeCell;
use crate::marker::PhantomData;
use crate::mem::MaybeUninit;
use crate::ptr;
use crate::sync::atomic::{self, Atomic, AtomicPtr, AtomicUsize, Ordering};
use crate::time::Instant;

// 用于表示一个槽位（slot）状态的若干比特位：
// * 如果一条消息已被写入该槽位，则置位 `WRITE`。
// * 如果一条消息已从该槽位被读出，则置位 `READ`。
// * 如果该 block 正在被销毁，则置位 `DESTROY`。
const WRITE: usize = 1;
const READ: usize = 2;
const DESTROY: usize = 4;

// 每个 block 覆盖索引的一“圈”（lap）。
const LAP: usize = 32;
// 一个 block 能容纳的消息数量上限。
const BLOCK_CAP: usize = LAP - 1;
// 索引低位中预留给元数据的比特位数。
const SHIFT: usize = 1;
// 有两种不同用途：
// * 若在 head 中置位，表示该 block 不是最后一个。
// * 若在 tail 中置位，表示通道已断连（disconnected）。
const MARK_BIT: usize = 1;

/// 一个 block 中的槽位（slot）。
struct Slot<T> {
    /// 消息本体。
    msg: UnsafeCell<MaybeUninit<T>>,

    /// 该槽位的状态。
    state: Atomic<usize>,
}

impl<T> Slot<T> {
    /// 等待直到有一条消息被写入该槽位。
    fn wait_write(&self) {
        let backoff = Backoff::new();
        while self.state.load(Ordering::Acquire) & WRITE == 0 {
            backoff.spin_heavy();
        }
    }
}

/// 链表中的一个 block（块）。
///
/// 链表中的每个 block 最多可容纳 `BLOCK_CAP` 条消息。
struct Block<T> {
    /// 链表中的下一个 block。
    next: Atomic<*mut Block<T>>,

    /// 用于存放消息的槽位。
    slots: [Slot<T>; BLOCK_CAP],
}

impl<T> Block<T> {
    /// 创建一个空的 block。
    fn new() -> Box<Block<T>> {
        // SAFETY: 这是安全的，因为：
        //  [1] `Block::next`（Atomic<*mut _>）可以被安全地零初始化。
        //  [2] `Block::slots`（数组）由于 [3, 4] 可以被安全地零初始化。
        //  [3] `Slot::msg`（UnsafeCell）由于它持有一个 MaybeUninit，可以被安全地零初始化。
        //  [4] `Slot::state`（Atomic<usize>）可以被安全地零初始化。
        unsafe { Box::new_zeroed().assume_init() }
    }

    /// 等待直到 next 指针被设置好。
    fn wait_next(&self) -> *mut Block<T> {
        let backoff = Backoff::new();
        loop {
            let next = self.next.load(Ordering::Acquire);
            if !next.is_null() {
                return next;
            }
            backoff.spin_heavy();
        }
    }

    /// 从 `start` 起在各槽位中置位 `DESTROY` 比特，并销毁该 block。
    unsafe fn destroy(this: *mut Block<T>, start: usize) {
        // 没有必要在最后一个槽位中置位 `DESTROY`，因为那个槽位正是发起本 block 销毁的那一个。
        for i in start..BLOCK_CAP - 1 {
            let slot = unsafe { (*this).slots.get_unchecked(i) };

            // 如果仍有线程在使用该槽位，则标记 `DESTROY` 比特。
            if slot.state.load(Ordering::Acquire) & READ == 0
                && slot.state.fetch_or(DESTROY, Ordering::AcqRel) & READ == 0
            {
                // 如果仍有线程在使用该槽位，则将由它来继续完成本 block 的销毁。
                return;
            }
        }

        // 已经没有线程在使用该 block，现在销毁它是安全的。
        drop(unsafe { Box::from_raw(this) });
    }
}

/// 通道中的一个位置（position）。
#[derive(Debug)]
struct Position<T> {
    /// 通道内的索引。
    index: Atomic<usize>,

    /// 链表中对应的 block。
    block: Atomic<*mut Block<T>>,
}

/// list flavor 的 token 类型。
#[derive(Debug)]
pub(crate) struct ListToken {
    /// 槽位所在的 block。
    block: *const u8,

    /// 在该 block 内的偏移量。
    offset: usize,
}

impl Default for ListToken {
    #[inline]
    fn default() -> Self {
        ListToken { block: ptr::null(), offset: 0 }
    }
}

/// 以链表实现的无界通道。
///
/// 发入通道的每条消息都会被分配一个序号（sequence number），即一个索引。索引以 `usize` 类型
/// 的数字表示，并在溢出时回绕（wrap）。
///
/// 连续的消息会被分组到若干 block 中，以减轻分配器的压力并提升缓存效率。
pub(crate) struct Channel<T> {
    /// 通道的头部（head）。
    head: CachePadded<Position<T>>,

    /// 通道的尾部（tail）。
    tail: CachePadded<Position<T>>,

    /// 在通道为空且未断连期间等待的接收者。
    receivers: SyncWaker,

    /// 表明丢弃一个 `Channel<T>` 时可能会丢弃类型为 `T` 的消息。
    _marker: PhantomData<T>,
}

impl<T> Channel<T> {
    /// 创建一个新的无界通道。
    pub(crate) fn new() -> Self {
        Channel {
            head: CachePadded::new(Position {
                block: AtomicPtr::new(ptr::null_mut()),
                index: AtomicUsize::new(0),
            }),
            tail: CachePadded::new(Position {
                block: AtomicPtr::new(ptr::null_mut()),
                index: AtomicUsize::new(0),
            }),
            receivers: SyncWaker::new(),
            _marker: PhantomData,
        }
    }

    /// 尝试为发送一条消息预订（reserve）一个槽位。
    fn start_send(&self, token: &mut Token) -> bool {
        let backoff = Backoff::new();
        let mut tail = self.tail.index.load(Ordering::Acquire);
        let mut block = self.tail.block.load(Ordering::Acquire);
        let mut next_block = None;

        loop {
            // 检查通道是否已断连。
            if tail & MARK_BIT != 0 {
                token.list.block = ptr::null();
                return true;
            }

            // 计算该索引在 block 内的偏移量。
            let offset = (tail >> SHIFT) % LAP;

            // 如果我们已到达本 block 的末尾，则等待直到下一个 block 被装上。
            if offset == BLOCK_CAP {
                backoff.spin_heavy();
                tail = self.tail.index.load(Ordering::Acquire);
                block = self.tail.block.load(Ordering::Acquire);
                continue;
            }

            // 如果我们即将需要装上下一个 block，则提前把它分配好，以便把其他线程的等待时间
            // 缩到最短。
            if offset + 1 == BLOCK_CAP && next_block.is_none() {
                next_block = Some(Block::<T>::new());
            }

            // 如果这是发入通道的第一条消息，我们需要分配第一个 block 并把它装上。
            if block.is_null() {
                let new = Box::into_raw(Block::<T>::new());

                if self
                    .tail
                    .block
                    .compare_exchange(block, new, Ordering::Release, Ordering::Relaxed)
                    .is_ok()
                {
                    // 这个让出点（yield point）会让通道处于一种半初始化状态：tail.block 指针
                    // 已设置，而 head.block 尚未设置。它被用来配合 miri 测试
                    // src/tools/miri/tests/pass/issues/issue-139553.rs。
                    #[cfg(miri)]
                    crate::thread::yield_now();
                    self.head.block.store(new, Ordering::Release);
                    block = new;
                } else {
                    next_block = unsafe { Some(Box::from_raw(new)) };
                    tail = self.tail.index.load(Ordering::Acquire);
                    block = self.tail.block.load(Ordering::Acquire);
                    continue;
                }
            }

            let new_tail = tail + (1 << SHIFT);

            // 尝试把 tail 向前推进。
            match self.tail.index.compare_exchange_weak(
                tail,
                new_tail,
                Ordering::SeqCst,
                Ordering::Acquire,
            ) {
                Ok(_) => unsafe {
                    // 如果我们已到达本 block 的末尾，则装上下一个 block。
                    if offset + 1 == BLOCK_CAP {
                        let next_block = Box::into_raw(next_block.unwrap());
                        self.tail.block.store(next_block, Ordering::Release);
                        self.tail.index.fetch_add(1 << SHIFT, Ordering::Release);
                        (*block).next.store(next_block, Ordering::Release);
                    }

                    token.list.block = block as *const u8;
                    token.list.offset = offset;
                    return true;
                },
                Err(_) => {
                    backoff.spin_light();
                    tail = self.tail.index.load(Ordering::Acquire);
                    block = self.tail.block.load(Ordering::Acquire);
                }
            }
        }
    }

    /// 向通道写入一条消息。
    pub(crate) unsafe fn write(&self, token: &mut Token, msg: T) -> Result<(), T> {
        // 如果没有槽位，说明通道已断连。
        if token.list.block.is_null() {
            return Err(msg);
        }

        // 把消息写入槽位。
        let block = token.list.block as *mut Block<T>;
        let offset = token.list.offset;
        unsafe {
            let slot = (*block).slots.get_unchecked(offset);
            slot.msg.get().write(MaybeUninit::new(msg));
            slot.state.fetch_or(WRITE, Ordering::Release);
        }

        // 唤醒一个正在睡眠的接收者。
        self.receivers.notify();
        Ok(())
    }

    /// 尝试为接收一条消息预订（reserve）一个槽位。
    fn start_recv(&self, token: &mut Token) -> bool {
        let backoff = Backoff::new();
        let mut head = self.head.index.load(Ordering::Acquire);
        let mut block = self.head.block.load(Ordering::Acquire);

        loop {
            // 计算该索引在 block 内的偏移量。
            let offset = (head >> SHIFT) % LAP;

            // 如果我们已到达本 block 的末尾，则等待直到下一个 block 被装上。
            if offset == BLOCK_CAP {
                backoff.spin_heavy();
                head = self.head.index.load(Ordering::Acquire);
                block = self.head.block.load(Ordering::Acquire);
                continue;
            }

            let mut new_head = head + (1 << SHIFT);

            if new_head & MARK_BIT == 0 {
                atomic::fence(Ordering::SeqCst);
                let tail = self.tail.index.load(Ordering::Relaxed);

                // 如果 tail 等于 head，意味着通道为空。
                if head >> SHIFT == tail >> SHIFT {
                    // 如果通道已断连……
                    if tail & MARK_BIT != 0 {
                        // ……那么接收到一个错误。
                        token.list.block = ptr::null();
                        return true;
                    } else {
                        // 否则，接收操作尚未就绪。
                        return false;
                    }
                }

                // 如果 head 与 tail 不在同一个 block 中，则在 head 中置位 `MARK_BIT`。
                if (head >> SHIFT) / LAP != (tail >> SHIFT) / LAP {
                    new_head |= MARK_BIT;
                }
            }

            // 此处 block 为 null 的唯一可能，是正有第一条消息被发入通道。这种情况下，只需
            // 等待直到它被初始化。
            if block.is_null() {
                backoff.spin_heavy();
                head = self.head.index.load(Ordering::Acquire);
                block = self.head.block.load(Ordering::Acquire);
                continue;
            }

            // 尝试把 head 索引向前移动。
            match self.head.index.compare_exchange_weak(
                head,
                new_head,
                Ordering::SeqCst,
                Ordering::Acquire,
            ) {
                Ok(_) => unsafe {
                    // 如果我们已到达本 block 的末尾，则移动到下一个 block。
                    if offset + 1 == BLOCK_CAP {
                        let next = (*block).wait_next();
                        let mut next_index = (new_head & !MARK_BIT).wrapping_add(1 << SHIFT);
                        if !(*next).next.load(Ordering::Relaxed).is_null() {
                            next_index |= MARK_BIT;
                        }

                        self.head.block.store(next, Ordering::Release);
                        self.head.index.store(next_index, Ordering::Release);
                    }

                    token.list.block = block as *const u8;
                    token.list.offset = offset;
                    return true;
                },
                Err(_) => {
                    backoff.spin_light();
                    head = self.head.index.load(Ordering::Acquire);
                    block = self.head.block.load(Ordering::Acquire);
                }
            }
        }
    }

    /// 从通道读取一条消息。
    pub(crate) unsafe fn read(&self, token: &mut Token) -> Result<T, ()> {
        if token.list.block.is_null() {
            // 通道已断连。
            return Err(());
        }

        // 读取消息。
        let block = token.list.block as *mut Block<T>;
        let offset = token.list.offset;
        unsafe {
            let slot = (*block).slots.get_unchecked(offset);
            slot.wait_write();
            let msg = slot.msg.get().read().assume_init();

            // 如果我们已到达末尾，或者另一线程曾想销毁该 block 但因我们正忙于从槽位读取而
            // 未能成功，则在此销毁该 block。
            if offset + 1 == BLOCK_CAP {
                Block::destroy(block, 0);
            } else if slot.state.fetch_or(READ, Ordering::AcqRel) & DESTROY != 0 {
                Block::destroy(block, offset + 1);
            }

            Ok(msg)
        }
    }

    /// 尝试向通道发送一条消息。
    pub(crate) fn try_send(&self, msg: T) -> Result<(), TrySendError<T>> {
        self.send(msg, None).map_err(|err| match err {
            SendTimeoutError::Disconnected(msg) => TrySendError::Disconnected(msg),
            SendTimeoutError::Timeout(_) => unreachable!(),
        })
    }

    /// 向通道发送一条消息。
    pub(crate) fn send(
        &self,
        msg: T,
        _deadline: Option<Instant>,
    ) -> Result<(), SendTimeoutError<T>> {
        let token = &mut Token::default();
        assert!(self.start_send(token));
        unsafe { self.write(token, msg).map_err(SendTimeoutError::Disconnected) }
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
            if self.start_recv(token) {
                unsafe {
                    return self.read(token).map_err(|_| RecvTimeoutError::Disconnected);
                }
            }

            if let Some(d) = deadline {
                if Instant::now() >= d {
                    return Err(RecvTimeoutError::Timeout);
                }
            }

            // 准备阻塞，直到某个发送者把我们唤醒。
            Context::with(|cx| {
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
            // 先加载 tail 索引，再加载 head 索引。
            let mut tail = self.tail.index.load(Ordering::SeqCst);
            let mut head = self.head.index.load(Ordering::SeqCst);

            // 如果 tail 索引没有改变，说明我们拿到了一对一致（consistent）的索引可供计算。
            if self.tail.index.load(Ordering::SeqCst) == tail {
                // 抹掉低位。
                tail &= !((1 << SHIFT) - 1);
                head &= !((1 << SHIFT) - 1);

                // 如果索引正好落在 block 的末尾，则修正它们。
                if (tail >> SHIFT) & (LAP - 1) == LAP - 1 {
                    tail = tail.wrapping_add(1 << SHIFT);
                }
                if (head >> SHIFT) & (LAP - 1) == LAP - 1 {
                    head = head.wrapping_add(1 << SHIFT);
                }

                // 旋转（rotate）索引，使 head 落入第一个 block。
                let lap = (head >> SHIFT) / LAP;
                tail = tail.wrapping_sub((lap * LAP) << SHIFT);
                head = head.wrapping_sub((lap * LAP) << SHIFT);

                // 移除低位。
                tail >>= SHIFT;
                head >>= SHIFT;

                // 返回二者之差，再减去 tail 与 head 之间跨越的 block 数量。
                return tail - head - tail / LAP;
            }
        }
    }

    /// 返回通道的容量。
    pub(crate) fn capacity(&self) -> Option<usize> {
        None
    }

    /// 断开（disconnect）发送者，并唤醒所有被阻塞的接收者。
    ///
    /// 如果本次调用使通道断连，返回 `true`。
    pub(crate) fn disconnect_senders(&self) -> bool {
        let tail = self.tail.index.fetch_or(MARK_BIT, Ordering::SeqCst);

        if tail & MARK_BIT == 0 {
            self.receivers.disconnect();
            true
        } else {
            false
        }
    }

    /// 断开（disconnect）接收者。
    ///
    /// 如果本次调用使通道断连，返回 `true`。
    pub(crate) fn disconnect_receivers(&self) -> bool {
        let tail = self.tail.index.fetch_or(MARK_BIT, Ordering::SeqCst);

        if tail & MARK_BIT == 0 {
            // 如果接收者先被丢弃，则丢弃所有消息，以尽早释放内存。
            self.discard_all_messages();
            true
        } else {
            false
        }
    }

    /// 丢弃所有消息。
    ///
    /// 此方法只应在所有接收者都已被丢弃后调用。
    fn discard_all_messages(&self) {
        let backoff = Backoff::new();
        let mut tail = self.tail.index.load(Ordering::Acquire);
        loop {
            let offset = (tail >> SHIFT) % LAP;
            if offset != BLOCK_CAP {
                break;
            }

            // 对 tail 的新更新会因 MARK_BIT 而被拒绝并中止，除非它恰好处在边界上。我们需要
            // 等待这些更新生效，否则可能发生内存泄漏。
            backoff.spin_heavy();
            tail = self.tail.index.load(Ordering::Acquire);
        }

        let mut head = self.head.index.load(Ordering::Acquire);
        // 通道可能尚未初始化，因此我们必须用 swap，以免覆盖某个发送者在察觉到接收者已断连
        // 之前对第一个 block 所做的初始化尝试。那些较晚的分配将由发送者在 Drop 中释放。
        let mut block = self.head.block.swap(ptr::null_mut(), Ordering::AcqRel);

        // 如果我们将要丢弃消息，就需要与初始化过程进行同步。
        if head >> SHIFT != tail >> SHIFT {
            // 此处 block 为 null 的唯一可能是：当一个发送者正在初始化通道的过程中，另一个
            // 发送者设法把一条消息插入这个半初始化的通道并推进了 tail。这种情况下，只需
            // 等待直到它被初始化。
            while block.is_null() {
                backoff.spin_heavy();
                block = self.head.block.swap(ptr::null_mut(), Ordering::AcqRel);
            }
        }
        // 自此之后 `head.block` 不会再被修改；如果它非空，将会被释放。在本函数之后运行的
        // 通道 `Drop` 代码同样会尝试释放非空的 `head.block`。因此本函数必须维持这样一条
        // 不变式：一旦尝试释放 head.block，就必须同时把它置为 NULL。否则将导致 Drop 代码
        // 重复释放（double free）。正因如此，上面两处读取都采用原子 swap 而非简单的原子
        // load。

        unsafe {
            // 丢弃 head 与 tail 之间的所有消息，并释放在堆上分配的各个 block。
            while head >> SHIFT != tail >> SHIFT {
                let offset = (head >> SHIFT) % LAP;

                if offset < BLOCK_CAP {
                    // 丢弃槽位中的消息。
                    let slot = (*block).slots.get_unchecked(offset);
                    slot.wait_write();
                    let p = &mut *slot.msg.get();
                    p.as_mut_ptr().drop_in_place();
                } else {
                    (*block).wait_next();
                    // 释放该 block 并移动到下一个。
                    let next = (*block).next.load(Ordering::Acquire);
                    drop(Box::from_raw(block));
                    block = next;
                }

                head = head.wrapping_add(1 << SHIFT);
            }

            // 释放最后剩下的那个 block。
            if !block.is_null() {
                drop(Box::from_raw(block));
            }
        }

        head &= !MARK_BIT;
        self.head.index.store(head, Ordering::Release);
    }

    /// 如果通道已断连，返回 `true`。
    pub(crate) fn is_disconnected(&self) -> bool {
        self.tail.index.load(Ordering::SeqCst) & MARK_BIT != 0
    }

    /// 如果通道为空，返回 `true`。
    pub(crate) fn is_empty(&self) -> bool {
        let head = self.head.index.load(Ordering::SeqCst);
        let tail = self.tail.index.load(Ordering::SeqCst);
        head >> SHIFT == tail >> SHIFT
    }

    /// 如果通道已满，返回 `true`。
    pub(crate) fn is_full(&self) -> bool {
        false
    }
}

impl<T> Drop for Channel<T> {
    fn drop(&mut self) {
        let mut head = self.head.index.load(Ordering::Relaxed);
        let mut tail = self.tail.index.load(Ordering::Relaxed);
        let mut block = self.head.block.load(Ordering::Relaxed);

        // 抹掉低位。
        head &= !((1 << SHIFT) - 1);
        tail &= !((1 << SHIFT) - 1);

        unsafe {
            // 丢弃 head 与 tail 之间的所有消息，并释放在堆上分配的各个 block。
            while head != tail {
                let offset = (head >> SHIFT) % LAP;

                if offset < BLOCK_CAP {
                    // 丢弃槽位中的消息。
                    let slot = (*block).slots.get_unchecked(offset);
                    let p = &mut *slot.msg.get();
                    p.as_mut_ptr().drop_in_place();
                } else {
                    // 释放该 block 并移动到下一个。
                    let next = (*block).next.load(Ordering::Relaxed);
                    drop(Box::from_raw(block));
                    block = next;
                }

                head = head.wrapping_add(1 << SHIFT);
            }

            // 释放最后剩下的那个 block。
            if !block.is_null() {
                drop(Box::from_raw(block));
            }
        }
    }
}
