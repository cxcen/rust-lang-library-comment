//! 容量为零的通道（zero-capacity channel）。
//!
//! 这种通道也被称为 *rendezvous*（会合）通道。

use super::context::Context;
use super::error::*;
use super::select::{Operation, Selected, Token};
use super::utils::Backoff;
use super::waker::Waker;
use crate::cell::UnsafeCell;
use crate::marker::PhantomData;
use crate::sync::Mutex;
use crate::sync::atomic::{Atomic, AtomicBool, Ordering};
use crate::time::Instant;
use crate::{fmt, ptr};

/// 指向一个 packet 的指针。
pub(crate) struct ZeroToken(*mut ());

impl Default for ZeroToken {
    fn default() -> Self {
        Self(ptr::null_mut())
    }
}

impl fmt::Debug for ZeroToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&(self.0 as usize), f)
    }
}

/// 一个用于把一条消息从发送者传递给接收者的槽位（packet）。
struct Packet<T> {
    /// 如果该 packet 分配在栈上，则等于 `true`。
    on_stack: bool,

    /// 一旦该 packet 已就绪、可供读取或写入，则等于 `true`。
    ready: Atomic<bool>,

    /// 消息本体。
    msg: UnsafeCell<Option<T>>,
}

impl<T> Packet<T> {
    /// 在栈上创建一个空的 packet。
    fn empty_on_stack() -> Packet<T> {
        Packet { on_stack: true, ready: AtomicBool::new(false), msg: UnsafeCell::new(None) }
    }

    /// 在栈上创建一个携带消息的 packet。
    fn message_on_stack(msg: T) -> Packet<T> {
        Packet { on_stack: true, ready: AtomicBool::new(false), msg: UnsafeCell::new(Some(msg)) }
    }

    /// 等待直到该 packet 就绪、可供读取或写入。
    fn wait_ready(&self) {
        let backoff = Backoff::new();
        while !self.ready.load(Ordering::Acquire) {
            backoff.spin_heavy();
        }
    }
}

/// 容量为零的通道的内部表示。
struct Inner {
    /// 正等待与某个接收操作配对的发送者。
    senders: Waker,

    /// 正等待与某个发送操作配对的接收者。
    receivers: Waker,

    /// 当通道已断连（disconnected）时等于 `true`。
    is_disconnected: bool,
}

/// 容量为零的通道。
pub(crate) struct Channel<T> {
    /// 通道的内部表示。
    inner: Mutex<Inner>,

    /// 表明丢弃一个 `Channel<T>` 时可能会丢弃类型为 `T` 的值。
    _marker: PhantomData<T>,
}

impl<T> Channel<T> {
    /// 构造一个新的容量为零的通道。
    pub(crate) fn new() -> Self {
        Channel {
            inner: Mutex::new(Inner {
                senders: Waker::new(),
                receivers: Waker::new(),
                is_disconnected: false,
            }),
            _marker: PhantomData,
        }
    }

    /// 把一条消息写入 packet。
    pub(crate) unsafe fn write(&self, token: &mut Token, msg: T) -> Result<(), T> {
        // 如果没有 packet，说明通道已断连。
        if token.zero.0.is_null() {
            return Err(msg);
        }

        unsafe {
            let packet = &*(token.zero.0 as *const Packet<T>);
            packet.msg.get().write(Some(msg));
            packet.ready.store(true, Ordering::Release);
        }
        Ok(())
    }

    /// 从 packet 读取一条消息。
    pub(crate) unsafe fn read(&self, token: &mut Token) -> Result<T, ()> {
        // 如果没有 packet，说明通道已断连。
        if token.zero.0.is_null() {
            return Err(());
        }

        let packet = unsafe { &*(token.zero.0 as *const Packet<T>) };

        if packet.on_stack {
            // 消息从一开始就在 packet 里，因此无需等待它。不过，读取消息之后我们需要把
            // `ready` 设为 `true`，以示意该 packet 可以被销毁了。
            let msg = unsafe { packet.msg.get().replace(None) }.unwrap();
            packet.ready.store(true, Ordering::Release);
            Ok(msg)
        } else {
            // 等待直到消息变为可用，然后读取它，并销毁这个在堆上分配的 packet。
            packet.wait_ready();
            unsafe {
                let msg = packet.msg.get().replace(None).unwrap();
                drop(Box::from_raw(token.zero.0 as *mut Packet<T>));
                Ok(msg)
            }
        }
    }

    /// 尝试向通道发送一条消息。
    pub(crate) fn try_send(&self, msg: T) -> Result<(), TrySendError<T>> {
        let token = &mut Token::default();
        let mut inner = self.inner.lock().unwrap();

        // 如果有一个正在等待的接收者，就与它配对。
        if let Some(operation) = inner.receivers.try_select() {
            token.zero.0 = operation.packet;
            drop(inner);
            unsafe {
                self.write(token, msg).ok().unwrap();
            }
            Ok(())
        } else if inner.is_disconnected {
            Err(TrySendError::Disconnected(msg))
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
        let mut inner = self.inner.lock().unwrap();

        // 如果有一个正在等待的接收者，就与它配对。
        if let Some(operation) = inner.receivers.try_select() {
            token.zero.0 = operation.packet;
            drop(inner);
            unsafe {
                self.write(token, msg).ok().unwrap();
            }
            return Ok(());
        }

        if inner.is_disconnected {
            return Err(SendTimeoutError::Disconnected(msg));
        }

        Context::with(|cx| {
            // 准备阻塞，直到某个接收者把我们唤醒。
            let oper = Operation::hook(token);
            let mut packet = Packet::<T>::message_on_stack(msg);
            inner.senders.register_with_packet(oper, (&raw mut packet) as *mut (), cx);
            inner.receivers.notify();
            drop(inner);

            // 阻塞当前线程。
            // SAFETY: 该上下文属于当前线程。
            let sel = unsafe { cx.wait_until(deadline) };

            match sel {
                Selected::Waiting => unreachable!(),
                Selected::Aborted => {
                    self.inner.lock().unwrap().senders.unregister(oper).unwrap();
                    let msg = unsafe { packet.msg.get().replace(None).unwrap() };
                    Err(SendTimeoutError::Timeout(msg))
                }
                Selected::Disconnected => {
                    self.inner.lock().unwrap().senders.unregister(oper).unwrap();
                    let msg = unsafe { packet.msg.get().replace(None).unwrap() };
                    Err(SendTimeoutError::Disconnected(msg))
                }
                Selected::Operation(_) => {
                    // 等待直到消息被读走，然后丢弃该 packet。
                    packet.wait_ready();
                    Ok(())
                }
            }
        })
    }

    /// 尝试以非阻塞方式接收一条消息。
    pub(crate) fn try_recv(&self) -> Result<T, TryRecvError> {
        let token = &mut Token::default();
        let mut inner = self.inner.lock().unwrap();

        // 如果有一个正在等待的发送者，就与它配对。
        if let Some(operation) = inner.senders.try_select() {
            token.zero.0 = operation.packet;
            drop(inner);
            unsafe { self.read(token).map_err(|_| TryRecvError::Disconnected) }
        } else if inner.is_disconnected {
            Err(TryRecvError::Disconnected)
        } else {
            Err(TryRecvError::Empty)
        }
    }

    /// 从通道接收一条消息。
    pub(crate) fn recv(&self, deadline: Option<Instant>) -> Result<T, RecvTimeoutError> {
        let token = &mut Token::default();
        let mut inner = self.inner.lock().unwrap();

        // 如果有一个正在等待的发送者，就与它配对。
        if let Some(operation) = inner.senders.try_select() {
            token.zero.0 = operation.packet;
            drop(inner);
            unsafe {
                return self.read(token).map_err(|_| RecvTimeoutError::Disconnected);
            }
        }

        if inner.is_disconnected {
            return Err(RecvTimeoutError::Disconnected);
        }

        Context::with(|cx| {
            // 准备阻塞，直到某个发送者把我们唤醒。
            let oper = Operation::hook(token);
            let mut packet = Packet::<T>::empty_on_stack();
            inner.receivers.register_with_packet(oper, (&raw mut packet) as *mut (), cx);
            inner.senders.notify();
            drop(inner);

            // 阻塞当前线程。
            // SAFETY: 该上下文属于当前线程。
            let sel = unsafe { cx.wait_until(deadline) };

            match sel {
                Selected::Waiting => unreachable!(),
                Selected::Aborted => {
                    self.inner.lock().unwrap().receivers.unregister(oper).unwrap();
                    Err(RecvTimeoutError::Timeout)
                }
                Selected::Disconnected => {
                    self.inner.lock().unwrap().receivers.unregister(oper).unwrap();
                    Err(RecvTimeoutError::Disconnected)
                }
                Selected::Operation(_) => {
                    // 等待直到消息被提供，然后读取它。
                    packet.wait_ready();
                    unsafe { Ok(packet.msg.get().replace(None).unwrap()) }
                }
            }
        })
    }

    /// 断开（disconnect）通道，并唤醒所有被阻塞的发送者和接收者。
    ///
    /// 如果本次调用使通道断连，返回 `true`。
    pub(crate) fn disconnect(&self) -> bool {
        let mut inner = self.inner.lock().unwrap();

        if !inner.is_disconnected {
            inner.is_disconnected = true;
            inner.senders.disconnect();
            inner.receivers.disconnect();
            true
        } else {
            false
        }
    }

    /// 返回通道中当前的消息数量。
    pub(crate) fn len(&self) -> usize {
        0
    }

    /// 返回通道的容量。
    #[allow(clippy::unnecessary_wraps)] // 这是有意为之。
    pub(crate) fn capacity(&self) -> Option<usize> {
        Some(0)
    }

    /// 如果通道为空，返回 `true`。
    pub(crate) fn is_empty(&self) -> bool {
        true
    }

    /// 如果通道已满，返回 `true`。
    pub(crate) fn is_full(&self) -> bool {
        true
    }
}
