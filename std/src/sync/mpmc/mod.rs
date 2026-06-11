//! 多生产者、多消费者（multi-producer, multi-consumer）FIFO 队列通信原语。
//!
//! 本模块提供基于消息的通道（channel）通信，具体由两个类型定义：
//!
//! * [`Sender`]
//! * [`Receiver`]
//!
//! [`Sender`] 用于把数据发送给一组 [`Receiver`]；其中每一条被发送的消息都会被投递给
//! （至多）一个接收者。发送端与接收端都可以克隆（多生产者），因此多个线程可以同时向多个
//! 接收者发送（多消费者）。
//!
//! 这些通道有两种 flavor（风味/变体）：
//!
//! 1. 异步、缓冲区无限大的通道。[`channel`] 函数返回一个 `(Sender, Receiver)` 元组，其中所有
//!    发送都是 **异步的**（永不阻塞）。该通道在概念上拥有一个无限大的缓冲区。
//!
//! 2. 同步、有界的通道。[`sync_channel`] 函数返回一个 `(Sender, Receiver)` 元组，其中待处理
//!    消息的存储是一块预先分配、固定大小的缓冲区。所有发送都是 **同步的**：当缓冲区满时会
//!    阻塞，直到有空位为止。注意：边界（bound）为 0 是允许的，此时通道变为“会合”
//!    （rendezvous）通道，即每个发送者以原子方式把一条消息直接交到某个接收者手中。
//!
//! [`send`]: Sender::send
//!
//! ## 断连（Disconnection）
//!
//! 通道上的发送与接收操作都会返回一个 [`Result`]，用于指示操作是否成功。一次不成功的操作
//! 通常意味着通道的另一半已在其对应线程中被丢弃（drop），即“挂断”（hung up）了。
//!
//! 一旦通道的某一半被释放，大多数操作便无法再继续推进，于是会返回 [`Err`]。许多应用会对
//! 本模块返回的结果直接 [`unwrap`]，从而在某个线程意外死亡时，将失败在线程间传播开来。
//!
//! [`unwrap`]: Result::unwrap
//!
//! # 示例
//!
//! 简单用法：
//!
//! ```
//! #![feature(mpmc_channel)]
//!
//! use std::thread;
//! use std::sync::mpmc::channel;
//!
//! // 创建一个简单的流式通道
//! let (tx, rx) = channel();
//! thread::spawn(move || {
//!     tx.send(10).unwrap();
//! });
//! assert_eq!(rx.recv().unwrap(), 10);
//! ```
//!
//! 共享用法：
//!
//! ```
//! #![feature(mpmc_channel)]
//!
//! use std::thread;
//! use std::sync::mpmc::channel;
//!
//! thread::scope(|s| {
//!     // 创建一个可在多个线程间传递的共享通道，
//!     // 其中 tx 是发送半（tx 取自 transmission），rx 是接收半
//!     // （rx 取自 receiving）。
//!     let (tx, rx) = channel();
//!     for i in 0..10 {
//!         let tx = tx.clone();
//!         s.spawn(move || {
//!             tx.send(i).unwrap();
//!         });
//!     }
//!
//!     for _ in 0..5 {
//!         let rx1 = rx.clone();
//!         let rx2 = rx.clone();
//!         s.spawn(move || {
//!             let j = rx1.recv().unwrap();
//!             assert!(0 <= j && j < 10);
//!         });
//!         s.spawn(move || {
//!             let j = rx2.recv().unwrap();
//!             assert!(0 <= j && j < 10);
//!         });
//!     }
//! })
//! ```
//!
//! 传播 panic：
//!
//! ```
//! #![feature(mpmc_channel)]
//!
//! use std::sync::mpmc::channel;
//!
//! // 这次 recv() 调用会返回一个错误，因为通道已经
//! // 挂断（或已被释放）
//! let (tx, rx) = channel::<i32>();
//! drop(tx);
//! assert!(rx.recv().is_err());
//! ```

// 本模块被用作 `sync::mpsc` 中各通道的实现基础。
// 该实现来自 crossbeam-channel crate：
//
// Copyright (c) 2019 The Crossbeam Project Developers
//
// Permission is hereby granted, free of charge, to any
// person obtaining a copy of this software and associated
// documentation files (the "Software"), to deal in the
// Software without restriction, including without
// limitation the rights to use, copy, modify, merge,
// publish, distribute, sublicense, and/or sell copies of
// the Software, and to permit persons to whom the Software
// is furnished to do so, subject to the following
// conditions:
//
// The above copyright notice and this permission notice
// shall be included in all copies or substantial portions
// of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF
// ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED
// TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A
// PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT
// SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
// CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
// OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR
// IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
// DEALINGS IN THE SOFTWARE.

mod array;
mod context;
mod counter;
mod error;
mod list;
mod select;
mod utils;
mod waker;
mod zero;

pub use error::*;

use crate::fmt;
use crate::panic::{RefUnwindSafe, UnwindSafe};
use crate::time::{Duration, Instant};

/// 创建一个新的异步通道，返回发送端/接收端这一对句柄。
///
/// 在 [`Sender`] 上发送的所有数据都会以发送时的相同顺序在 [`Receiver`] 处变为可用，且任何
/// [`send`] 都不会阻塞调用线程（该通道拥有“无限缓冲区”，这与 [`sync_channel`] 不同——后者
/// 在缓冲区达到上限后会阻塞）。只要至少还存在一个 [`Sender`]（含其克隆），[`recv`] 就会
/// 阻塞，直到有消息可用为止。
///
/// [`Sender`] 可以被克隆，以便多次向同一通道 [`send`]。[`Receiver`] 同样可以被克隆，以拥有
/// 多个接收者。
///
/// 如果在用 [`Sender`] 尝试 [`send`] 时 [`Receiver`] 已断连，则 [`send`] 方法会返回一个
/// [`SendError`]。同理，如果在尝试 [`recv`] 时 [`Sender`] 已断连，则 [`recv`] 方法会返回一个
/// [`RecvError`]。
///
/// [`send`]: Sender::send
/// [`recv`]: Receiver::recv
///
/// # 示例
///
/// ```
/// #![feature(mpmc_channel)]
///
/// use std::sync::mpmc::channel;
/// use std::thread;
///
/// let (sender, receiver) = channel();
///
/// // 启动一项开销很大的计算
/// thread::spawn(move || {
/// #   fn expensive_computation() {}
///     sender.send(expensive_computation()).unwrap();
/// });
///
/// // 在此期间做一些有用的工作
///
/// // 来看看那个答案是什么
/// println!("{:?}", receiver.recv().unwrap());
/// ```
#[must_use]
#[unstable(feature = "mpmc_channel", issue = "126840")]
pub fn channel<T>() -> (Sender<T>, Receiver<T>) {
    let (s, r) = counter::new(list::Channel::new());
    let s = Sender { flavor: SenderFlavor::List(s) };
    let r = Receiver { flavor: ReceiverFlavor::List(r) };
    (s, r)
}

/// 创建一个新的同步、有界的通道。
///
/// 在 [`Sender`] 上发送的所有数据都会以发送时的相同顺序在 [`Receiver`] 处变为可用。与异步的
/// [`channel`] 一样，[`Receiver`] 会阻塞直到有消息可用。然而 `sync_channel` 在发送端的语义上
/// 差别很大。
///
/// 该通道拥有一块内部缓冲区，消息会在其中排队。`bound` 指定缓冲区大小。当内部缓冲区变满时，
/// 后续的发送将 *阻塞*，等待缓冲区腾出空位。注意：缓冲区大小为 0 是合法的，此时通道变为
/// “会合通道”（rendezvous channel），即每次 [`send`] 都不会返回，直到有一次 [`recv`] 与之配对。
///
/// [`Sender`] 可以被克隆，以便多次向同一通道 [`send`]。[`Receiver`] 同样可以被克隆，以拥有
/// 多个接收者。
///
/// 与异步通道一样，如果在用 [`Sender`] 尝试 [`send`] 时 [`Receiver`] 已断连，则 [`send`] 方法
/// 会返回一个 [`SendError`]。同理，如果在尝试 [`recv`] 时 [`Sender`] 已断连，则 [`recv`] 方法
/// 会返回一个 [`RecvError`]。
///
/// [`send`]: Sender::send
/// [`recv`]: Receiver::recv
///
/// # 示例
///
/// ```
/// use std::sync::mpsc::sync_channel;
/// use std::thread;
///
/// let (sender, receiver) = sync_channel(1);
///
/// // 这次调用立即返回
/// sender.send(1).unwrap();
///
/// thread::spawn(move || {
///     // 这次发送会阻塞，直到前一条消息被接收
///     sender.send(2).unwrap();
/// });
///
/// assert_eq!(receiver.recv().unwrap(), 1);
/// assert_eq!(receiver.recv().unwrap(), 2);
/// ```
#[must_use]
#[unstable(feature = "mpmc_channel", issue = "126840")]
pub fn sync_channel<T>(cap: usize) -> (Sender<T>, Receiver<T>) {
    if cap == 0 {
        let (s, r) = counter::new(zero::Channel::new());
        let s = Sender { flavor: SenderFlavor::Zero(s) };
        let r = Receiver { flavor: ReceiverFlavor::Zero(r) };
        (s, r)
    } else {
        let (s, r) = counter::new(array::Channel::with_capacity(cap));
        let s = Sender { flavor: SenderFlavor::Array(s) };
        let r = Receiver { flavor: ReceiverFlavor::Array(r) };
        (s, r)
    }
}

/// Rust 同步 [`channel`] 类型的发送半（sending-half）。
///
/// 可以通过 [`send`] 经由该通道发送消息。
///
/// 注意：所有发送者（包括最初的那个及其全部克隆）都必须被丢弃，接收者才会停止阻塞，
/// 不再用 [`Receiver::recv`] 等待接收消息。
///
/// [`send`]: Sender::send
///
/// # 示例
///
/// ```rust
/// #![feature(mpmc_channel)]
///
/// use std::sync::mpmc::channel;
/// use std::thread;
///
/// let (sender, receiver) = channel();
/// let sender2 = sender.clone();
///
/// // 第一个线程拥有 sender
/// thread::spawn(move || {
///     sender.send(1).unwrap();
/// });
///
/// // 第二个线程拥有 sender2
/// thread::spawn(move || {
///     sender2.send(2).unwrap();
/// });
///
/// let msg = receiver.recv().unwrap();
/// let msg2 = receiver.recv().unwrap();
///
/// assert_eq!(3, msg + msg2);
/// ```
#[unstable(feature = "mpmc_channel", issue = "126840")]
pub struct Sender<T> {
    flavor: SenderFlavor<T>,
}

/// 发送端的各种 flavor（变体）。
enum SenderFlavor<T> {
    /// 基于预分配数组的有界通道。
    Array(counter::Sender<array::Channel<T>>),

    /// 以链表实现的无界通道。
    List(counter::Sender<list::Channel<T>>),

    /// 容量为零的通道。
    Zero(counter::Sender<zero::Channel<T>>),
}

#[unstable(feature = "mpmc_channel", issue = "126840")]
unsafe impl<T: Send> Send for Sender<T> {}
#[unstable(feature = "mpmc_channel", issue = "126840")]
unsafe impl<T: Send> Sync for Sender<T> {}

#[unstable(feature = "mpmc_channel", issue = "126840")]
impl<T> UnwindSafe for Sender<T> {}
#[unstable(feature = "mpmc_channel", issue = "126840")]
impl<T> RefUnwindSafe for Sender<T> {}

impl<T> Sender<T> {
    /// 尝试以非阻塞方式向通道发送一条消息。
    ///
    /// 此方法要么立即把消息发入通道，要么在通道已满或已断连时返回一个错误。返回的错误中
    /// 携带着原始消息。
    ///
    /// 若在容量为零的通道上调用此方法，则只有当此刻恰好有一个接收操作正位于通道另一端时，
    /// 才会把消息发送出去。
    ///
    /// # 示例
    ///
    /// ```rust
    /// #![feature(mpmc_channel)]
    ///
    /// use std::sync::mpmc::{channel, Receiver, Sender};
    ///
    /// let (sender, _receiver): (Sender<i32>, Receiver<i32>) = channel();
    ///
    /// assert!(sender.try_send(1).is_ok());
    /// ```
    #[unstable(feature = "mpmc_channel", issue = "126840")]
    pub fn try_send(&self, msg: T) -> Result<(), TrySendError<T>> {
        match &self.flavor {
            SenderFlavor::Array(chan) => chan.try_send(msg),
            SenderFlavor::List(chan) => chan.try_send(msg),
            SenderFlavor::Zero(chan) => chan.try_send(msg),
        }
    }

    /// 尝试在该通道上发送一个值；若无法发送，则将其原样返回。
    ///
    /// 当确定通道的另一端尚未挂断时，发送即为成功。发送不成功则意味着对应的接收者已被
    /// 释放。注意：返回值为 [`Err`] 意味着数据永远不会被接收到；但返回值为 [`Ok`] 并 *不*
    /// 意味着数据一定会被接收到——对应的接收者完全可能在本函数返回 [`Ok`] 之后立刻挂断。
    /// 不过，如果通道容量为零，它就充当一个会合（rendezvous）通道，此时返回值为 [`Ok`] 即
    /// 表示数据已经被接收。
    ///
    /// 如果通道已满且未断连，本次调用会阻塞，直到发送操作能够继续推进。如果通道变为断连，
    /// 本次调用会被唤醒并返回一个错误。返回的错误中携带着原始消息。
    ///
    /// 若在容量为零的通道上调用此方法，则它会等待，直到通道另一端出现一个接收操作。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(mpmc_channel)]
    ///
    /// use std::sync::mpmc::channel;
    ///
    /// let (tx, rx) = channel();
    ///
    /// // 这次发送总是成功的
    /// tx.send(1).unwrap();
    ///
    /// // 这次发送会失败，因为接收者已经不在了
    /// drop(rx);
    /// assert!(tx.send(1).is_err());
    /// ```
    #[unstable(feature = "mpmc_channel", issue = "126840")]
    pub fn send(&self, msg: T) -> Result<(), SendError<T>> {
        match &self.flavor {
            SenderFlavor::Array(chan) => chan.send(msg, None),
            SenderFlavor::List(chan) => chan.send(msg, None),
            SenderFlavor::Zero(chan) => chan.send(msg, None),
        }
        .map_err(|err| match err {
            SendTimeoutError::Disconnected(msg) => SendError(msg),
            SendTimeoutError::Timeout(_) => unreachable!(),
        })
    }
}

impl<T> Sender<T> {
    /// 等待一条消息被发送进通道，但只等待有限的一段时间。
    ///
    /// 如果通道已满且未断连，本次调用会阻塞，直到发送操作能够继续推进，或者操作超时。如果
    /// 通道变为断连，本次调用会被唤醒并返回一个错误。返回的错误中携带着原始消息。
    ///
    /// 若在容量为零的通道上调用此方法，则它会等待，直到通道另一端出现一个接收操作。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(mpmc_channel)]
    ///
    /// use std::sync::mpmc::channel;
    /// use std::time::Duration;
    ///
    /// let (tx, rx) = channel();
    ///
    /// tx.send_timeout(1, Duration::from_millis(400)).unwrap();
    /// ```
    #[unstable(feature = "mpmc_channel", issue = "126840")]
    pub fn send_timeout(&self, msg: T, timeout: Duration) -> Result<(), SendTimeoutError<T>> {
        match Instant::now().checked_add(timeout) {
            Some(deadline) => self.send_deadline(msg, deadline),
            // 超时点远在未来，实际上等同于无限期等待。
            None => self.send(msg).map_err(SendTimeoutError::from),
        }
    }

    /// 等待一条消息被发送进通道，但只等到给定的截止时刻（deadline）。
    ///
    /// 如果通道已满且未断连，本次调用会阻塞，直到发送操作能够继续推进，或者操作超时。如果
    /// 通道变为断连，本次调用会被唤醒并返回一个错误。返回的错误中携带着原始消息。
    ///
    /// 若在容量为零的通道上调用此方法，则它会等待，直到通道另一端出现一个接收操作。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(mpmc_channel)]
    ///
    /// use std::sync::mpmc::channel;
    /// use std::time::{Duration, Instant};
    ///
    /// let (tx, rx) = channel();
    ///
    /// let t = Instant::now() + Duration::from_millis(400);
    /// tx.send_deadline(1, t).unwrap();
    /// ```
    #[unstable(feature = "mpmc_channel", issue = "126840")]
    pub fn send_deadline(&self, msg: T, deadline: Instant) -> Result<(), SendTimeoutError<T>> {
        match &self.flavor {
            SenderFlavor::Array(chan) => chan.send(msg, Some(deadline)),
            SenderFlavor::List(chan) => chan.send(msg, Some(deadline)),
            SenderFlavor::Zero(chan) => chan.send(msg, Some(deadline)),
        }
    }

    /// 如果通道为空，返回 `true`。
    ///
    /// 注意：容量为零的通道永远为空。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(mpmc_channel)]
    ///
    /// use std::sync::mpmc;
    /// use std::thread;
    ///
    /// let (send, _recv) = mpmc::channel();
    ///
    /// let tx1 = send.clone();
    /// let tx2 = send.clone();
    ///
    /// assert!(tx1.is_empty());
    ///
    /// let handle = thread::spawn(move || {
    ///     tx2.send(1u8).unwrap();
    /// });
    ///
    /// handle.join().unwrap();
    ///
    /// assert!(!tx1.is_empty());
    /// ```
    #[unstable(feature = "mpmc_channel", issue = "126840")]
    pub fn is_empty(&self) -> bool {
        match &self.flavor {
            SenderFlavor::Array(chan) => chan.is_empty(),
            SenderFlavor::List(chan) => chan.is_empty(),
            SenderFlavor::Zero(chan) => chan.is_empty(),
        }
    }

    /// 如果通道已满，返回 `true`。
    ///
    /// 注意：容量为零的通道永远是满的。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(mpmc_channel)]
    ///
    /// use std::sync::mpmc;
    /// use std::thread;
    ///
    /// let (send, _recv) = mpmc::sync_channel(1);
    ///
    /// let (tx1, tx2) = (send.clone(), send.clone());
    /// assert!(!tx1.is_full());
    ///
    /// let handle = thread::spawn(move || {
    ///     tx2.send(1u8).unwrap();
    /// });
    ///
    /// handle.join().unwrap();
    ///
    /// assert!(tx1.is_full());
    /// ```
    #[unstable(feature = "mpmc_channel", issue = "126840")]
    pub fn is_full(&self) -> bool {
        match &self.flavor {
            SenderFlavor::Array(chan) => chan.is_full(),
            SenderFlavor::List(chan) => chan.is_full(),
            SenderFlavor::Zero(chan) => chan.is_full(),
        }
    }

    /// 返回通道中消息的数量。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(mpmc_channel)]
    ///
    /// use std::sync::mpmc;
    /// use std::thread;
    ///
    /// let (send, _recv) = mpmc::channel();
    /// let (tx1, tx2) = (send.clone(), send.clone());
    ///
    /// assert_eq!(tx1.len(), 0);
    ///
    /// let handle = thread::spawn(move || {
    ///     tx2.send(1u8).unwrap();
    /// });
    ///
    /// handle.join().unwrap();
    ///
    /// assert_eq!(tx1.len(), 1);
    /// ```
    #[unstable(feature = "mpmc_channel", issue = "126840")]
    pub fn len(&self) -> usize {
        match &self.flavor {
            SenderFlavor::Array(chan) => chan.len(),
            SenderFlavor::List(chan) => chan.len(),
            SenderFlavor::Zero(chan) => chan.len(),
        }
    }

    /// 如果通道是有界的，返回其容量。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(mpmc_channel)]
    ///
    /// use std::sync::mpmc;
    /// use std::thread;
    ///
    /// let (send, _recv) = mpmc::sync_channel(3);
    /// let (tx1, tx2) = (send.clone(), send.clone());
    ///
    /// assert_eq!(tx1.capacity(), Some(3));
    ///
    /// let handle = thread::spawn(move || {
    ///     tx2.send(1u8).unwrap();
    /// });
    ///
    /// handle.join().unwrap();
    ///
    /// assert_eq!(tx1.capacity(), Some(3));
    /// ```
    #[unstable(feature = "mpmc_channel", issue = "126840")]
    pub fn capacity(&self) -> Option<usize> {
        match &self.flavor {
            SenderFlavor::Array(chan) => chan.capacity(),
            SenderFlavor::List(chan) => chan.capacity(),
            SenderFlavor::Zero(chan) => chan.capacity(),
        }
    }

    /// 如果两个发送者属于同一个通道，返回 `true`。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(mpmc_channel)]
    ///
    /// use std::sync::mpmc;
    ///
    /// let (tx1, _) = mpmc::channel::<i32>();
    /// let (tx2, _) = mpmc::channel::<i32>();
    ///
    /// assert!(tx1.same_channel(&tx1));
    /// assert!(!tx1.same_channel(&tx2));
    /// ```
    #[unstable(feature = "mpmc_channel", issue = "126840")]
    pub fn same_channel(&self, other: &Sender<T>) -> bool {
        match (&self.flavor, &other.flavor) {
            (SenderFlavor::Array(a), SenderFlavor::Array(b)) => a == b,
            (SenderFlavor::List(a), SenderFlavor::List(b)) => a == b,
            (SenderFlavor::Zero(a), SenderFlavor::Zero(b)) => a == b,
            _ => false,
        }
    }
}

#[unstable(feature = "mpmc_channel", issue = "126840")]
impl<T> Drop for Sender<T> {
    fn drop(&mut self) {
        unsafe {
            match &self.flavor {
                SenderFlavor::Array(chan) => chan.release(|c| c.disconnect_senders()),
                SenderFlavor::List(chan) => chan.release(|c| c.disconnect_senders()),
                SenderFlavor::Zero(chan) => chan.release(|c| c.disconnect()),
            }
        }
    }
}

#[unstable(feature = "mpmc_channel", issue = "126840")]
impl<T> Clone for Sender<T> {
    fn clone(&self) -> Self {
        let flavor = match &self.flavor {
            SenderFlavor::Array(chan) => SenderFlavor::Array(chan.acquire()),
            SenderFlavor::List(chan) => SenderFlavor::List(chan.acquire()),
            SenderFlavor::Zero(chan) => SenderFlavor::Zero(chan.acquire()),
        };

        Sender { flavor }
    }
}

#[unstable(feature = "mpmc_channel", issue = "126840")]
impl<T> fmt::Debug for Sender<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Sender").finish_non_exhaustive()
    }
}

/// Rust [`channel`]（或 [`sync_channel`]）类型的接收半（receiving half）。
/// 不同的线程可以通过克隆来共享这个 [`Receiver`]。
///
/// 发往通道的消息可以用 [`recv`] 取出。
///
/// [`recv`]: Receiver::recv
///
/// # 示例
///
/// ```rust
/// #![feature(mpmc_channel)]
///
/// use std::sync::mpmc::channel;
/// use std::thread;
/// use std::time::Duration;
///
/// let (send, recv) = channel();
///
/// let tx_thread = thread::spawn(move || {
///     send.send("Hello world!").unwrap();
///     thread::sleep(Duration::from_secs(2)); // 阻塞两秒
///     send.send("Delayed for 2 seconds").unwrap();
/// });
///
/// let (rx1, rx2) = (recv.clone(), recv.clone());
/// let rx_thread_1 = thread::spawn(move || {
///     println!("{}", rx1.recv().unwrap()); // 立即收到
/// });
/// let rx_thread_2 = thread::spawn(move || {
///     println!("{}", rx2.recv().unwrap()); // 两秒后收到
/// });
///
/// tx_thread.join().unwrap();
/// rx_thread_1.join().unwrap();
/// rx_thread_2.join().unwrap();
/// ```
#[unstable(feature = "mpmc_channel", issue = "126840")]
pub struct Receiver<T> {
    flavor: ReceiverFlavor<T>,
}

/// 遍历 [`Receiver`] 上消息的迭代器，由 [`iter`] 创建。
///
/// 每次调用 [`next`] 时，该迭代器都会阻塞，等待一条新消息；当对应的通道挂断时，将返回
/// [`None`]。
///
/// [`iter`]: Receiver::iter
/// [`next`]: Iterator::next
///
/// # 示例
///
/// ```rust
/// #![feature(mpmc_channel)]
///
/// use std::sync::mpmc::channel;
/// use std::thread;
///
/// let (send, recv) = channel();
///
/// thread::spawn(move || {
///     send.send(1u8).unwrap();
///     send.send(2u8).unwrap();
///     send.send(3u8).unwrap();
/// });
///
/// for x in recv.iter() {
///     println!("Got: {x}");
/// }
/// ```
#[unstable(feature = "mpmc_channel", issue = "126840")]
#[derive(Debug)]
pub struct Iter<'a, T: 'a> {
    rx: &'a Receiver<T>,
}

/// 一个尝试取出 [`Receiver`] 上所有待处理（pending）值的迭代器，由 [`try_iter`] 创建。
///
/// 当没有剩余的待处理值、或对应的通道已挂断时，将返回 [`None`]。
///
/// 该迭代器为了等待数据可用而 **永不** 阻塞调用方；相反，它会返回 [`None`]。
///
/// [`try_iter`]: Receiver::try_iter
///
/// # 示例
///
/// ```rust
/// #![feature(mpmc_channel)]
///
/// use std::sync::mpmc::channel;
/// use std::thread;
/// use std::time::Duration;
///
/// let (sender, receiver) = channel();
///
/// // 缓冲区里暂时还什么都没有
/// assert!(receiver.try_iter().next().is_none());
/// println!("Nothing in the buffer...");
///
/// thread::spawn(move || {
///     sender.send(1).unwrap();
///     sender.send(2).unwrap();
///     sender.send(3).unwrap();
/// });
///
/// println!("Going to sleep...");
/// thread::sleep(Duration::from_secs(2)); // 阻塞两秒
///
/// for x in receiver.try_iter() {
///     println!("Got: {x}");
/// }
/// ```
#[unstable(feature = "mpmc_channel", issue = "126840")]
#[derive(Debug)]
pub struct TryIter<'a, T: 'a> {
    rx: &'a Receiver<T>,
}

/// 一个拥有所有权（owning）、遍历 [`Receiver`] 上消息的迭代器，由 [`into_iter`] 创建。
///
/// 每次调用 [`next`] 时，该迭代器都会阻塞，等待一条新消息；当对应的通道挂断时，将返回
/// [`None`]。
///
/// [`into_iter`]: Receiver::into_iter
/// [`next`]: Iterator::next
///
/// # 示例
///
/// ```rust
/// #![feature(mpmc_channel)]
///
/// use std::sync::mpmc::channel;
/// use std::thread;
///
/// let (send, recv) = channel();
///
/// thread::spawn(move || {
///     send.send(1u8).unwrap();
///     send.send(2u8).unwrap();
///     send.send(3u8).unwrap();
/// });
///
/// for x in recv.into_iter() {
///     println!("Got: {x}");
/// }
/// ```
#[unstable(feature = "mpmc_channel", issue = "126840")]
#[derive(Debug)]
pub struct IntoIter<T> {
    rx: Receiver<T>,
}

#[unstable(feature = "mpmc_channel", issue = "126840")]
impl<'a, T> Iterator for Iter<'a, T> {
    type Item = T;

    fn next(&mut self) -> Option<T> {
        self.rx.recv().ok()
    }
}

#[unstable(feature = "mpmc_channel", issue = "126840")]
impl<'a, T> Iterator for TryIter<'a, T> {
    type Item = T;

    fn next(&mut self) -> Option<T> {
        self.rx.try_recv().ok()
    }
}

#[unstable(feature = "mpmc_channel", issue = "126840")]
impl<'a, T> IntoIterator for &'a Receiver<T> {
    type Item = T;
    type IntoIter = Iter<'a, T>;

    fn into_iter(self) -> Iter<'a, T> {
        self.iter()
    }
}

#[unstable(feature = "mpmc_channel", issue = "126840")]
impl<T> Iterator for IntoIter<T> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        self.rx.recv().ok()
    }
}

#[unstable(feature = "mpmc_channel", issue = "126840")]
impl<T> IntoIterator for Receiver<T> {
    type Item = T;
    type IntoIter = IntoIter<T>;

    fn into_iter(self) -> IntoIter<T> {
        IntoIter { rx: self }
    }
}

/// 接收端的各种 flavor（变体）。
enum ReceiverFlavor<T> {
    /// 基于预分配数组的有界通道。
    Array(counter::Receiver<array::Channel<T>>),

    /// 以链表实现的无界通道。
    List(counter::Receiver<list::Channel<T>>),

    /// 容量为零的通道。
    Zero(counter::Receiver<zero::Channel<T>>),
}

#[unstable(feature = "mpmc_channel", issue = "126840")]
unsafe impl<T: Send> Send for Receiver<T> {}
#[unstable(feature = "mpmc_channel", issue = "126840")]
unsafe impl<T: Send> Sync for Receiver<T> {}

#[unstable(feature = "mpmc_channel", issue = "126840")]
impl<T> UnwindSafe for Receiver<T> {}
#[unstable(feature = "mpmc_channel", issue = "126840")]
impl<T> RefUnwindSafe for Receiver<T> {}

impl<T> Receiver<T> {
    /// 尝试以非阻塞方式从通道接收一条消息。
    ///
    /// 此方法为了等待数据可用而 **永不** 阻塞调用方；相反，它总是立即返回，结果是一个可能
    /// 携带通道上待处理数据的 option。
    ///
    /// 若在容量为零的通道上调用此方法，则只有当此刻恰好有一个发送操作正位于通道另一端时，
    /// 才会接收到一条消息。
    ///
    /// 这对于在决定阻塞于某个接收者之前进行一种“乐观检查”（optimistic check）很有用。
    ///
    /// 与 [`recv`] 相比，本函数有两种失败情形而非一种（一种是断连，另一种是缓冲区为空）。
    ///
    /// [`recv`]: Self::recv
    ///
    /// # 示例
    ///
    /// ```rust
    /// #![feature(mpmc_channel)]
    ///
    /// use std::sync::mpmc::{Receiver, channel};
    ///
    /// let (_, receiver): (_, Receiver<i32>) = channel();
    ///
    /// assert!(receiver.try_recv().is_err());
    /// ```
    #[unstable(feature = "mpmc_channel", issue = "126840")]
    pub fn try_recv(&self) -> Result<T, TryRecvError> {
        match &self.flavor {
            ReceiverFlavor::Array(chan) => chan.try_recv(),
            ReceiverFlavor::List(chan) => chan.try_recv(),
            ReceiverFlavor::Zero(chan) => chan.try_recv(),
        }
    }

    /// 尝试在该接收者上等待一个值；如果对应的通道已挂断，则返回一个错误。
    ///
    /// 只要没有数据可用、且仍有可能发来更多数据（即至少还存在一个发送者），本函数就总会
    /// 阻塞当前线程。一旦有消息被发往对应的 [`Sender`]，该接收者便会被唤醒并返回那条消息。
    ///
    /// 如果对应的 [`Sender`] 已断连，或在本次调用阻塞期间发生断连，本次调用会被唤醒并返回
    /// [`Err`]，以表明此通道上永远不会再收到任何消息了。不过，由于通道是带缓冲的，在断连
    /// 之前发送的消息仍会被正确接收。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(mpmc_channel)]
    ///
    /// use std::sync::mpmc;
    /// use std::thread;
    ///
    /// let (send, recv) = mpmc::channel();
    /// let handle = thread::spawn(move || {
    ///     send.send(1u8).unwrap();
    /// });
    ///
    /// handle.join().unwrap();
    ///
    /// assert_eq!(Ok(1), recv.recv());
    /// ```
    ///
    /// 缓冲行为：
    ///
    /// ```
    /// #![feature(mpmc_channel)]
    ///
    /// use std::sync::mpmc;
    /// use std::thread;
    /// use std::sync::mpmc::RecvError;
    ///
    /// let (send, recv) = mpmc::channel();
    /// let handle = thread::spawn(move || {
    ///     send.send(1u8).unwrap();
    ///     send.send(2).unwrap();
    ///     send.send(3).unwrap();
    ///     drop(send);
    /// });
    ///
    /// // 等待该线程 join，以确保 sender 已被丢弃
    /// handle.join().unwrap();
    ///
    /// assert_eq!(Ok(1), recv.recv());
    /// assert_eq!(Ok(2), recv.recv());
    /// assert_eq!(Ok(3), recv.recv());
    /// assert_eq!(Err(RecvError), recv.recv());
    /// ```
    #[unstable(feature = "mpmc_channel", issue = "126840")]
    pub fn recv(&self) -> Result<T, RecvError> {
        match &self.flavor {
            ReceiverFlavor::Array(chan) => chan.recv(None),
            ReceiverFlavor::List(chan) => chan.recv(None),
            ReceiverFlavor::Zero(chan) => chan.recv(None),
        }
        .map_err(|_| RecvError)
    }

    /// 尝试在该接收者上等待一个值；如果对应的通道已挂断、或等待时间超过 `timeout`，则返回
    /// 一个错误。
    ///
    /// 只要没有数据可用、且仍有可能发来更多数据（即至少还存在一个发送者），本函数就总会
    /// 阻塞当前线程。一旦有消息被发往对应的 [`Sender`]，该接收者便会被唤醒并返回那条消息。
    ///
    /// 如果对应的 [`Sender`] 已断连，或在本次调用阻塞期间发生断连，本次调用会被唤醒并返回
    /// [`Err`]，以表明此通道上永远不会再收到任何消息了。不过，由于通道是带缓冲的，在断连
    /// 之前发送的消息仍会被正确接收。
    ///
    /// # 示例
    ///
    /// 在遇到超时之前成功收到值：
    ///
    /// ```no_run
    /// #![feature(mpmc_channel)]
    ///
    /// use std::thread;
    /// use std::time::Duration;
    /// use std::sync::mpmc;
    ///
    /// let (send, recv) = mpmc::channel();
    ///
    /// thread::spawn(move || {
    ///     send.send('a').unwrap();
    /// });
    ///
    /// assert_eq!(
    ///     recv.recv_timeout(Duration::from_millis(400)),
    ///     Ok('a')
    /// );
    /// ```
    ///
    /// 到达超时时收到一个错误：
    ///
    /// ```no_run
    /// #![feature(mpmc_channel)]
    ///
    /// use std::thread;
    /// use std::time::Duration;
    /// use std::sync::mpmc;
    ///
    /// let (send, recv) = mpmc::channel();
    ///
    /// thread::spawn(move || {
    ///     thread::sleep(Duration::from_millis(800));
    ///     send.send('a').unwrap();
    /// });
    ///
    /// assert_eq!(
    ///     recv.recv_timeout(Duration::from_millis(400)),
    ///     Err(mpmc::RecvTimeoutError::Timeout)
    /// );
    /// ```
    #[unstable(feature = "mpmc_channel", issue = "126840")]
    pub fn recv_timeout(&self, timeout: Duration) -> Result<T, RecvTimeoutError> {
        match Instant::now().checked_add(timeout) {
            Some(deadline) => self.recv_deadline(deadline),
            // 超时点远在未来，实际上等同于无限期等待。
            None => self.recv().map_err(RecvTimeoutError::from),
        }
    }

    /// 尝试在该接收者上等待一个值；如果对应的通道已挂断、或到达了 `deadline`，则返回一个
    /// 错误。
    ///
    /// 只要没有数据可用、且仍有可能发来更多数据，本函数就总会阻塞当前线程。一旦有消息被
    /// 发往对应的 [`Sender`]，该接收者便会被唤醒并返回那条消息。
    ///
    /// 如果对应的 [`Sender`] 已断连，或在本次调用阻塞期间发生断连，本次调用会被唤醒并返回
    /// [`Err`]，以表明此通道上永远不会再收到任何消息了。不过，由于通道是带缓冲的，在断连
    /// 之前发送的消息仍会被正确接收。
    ///
    /// # 示例
    ///
    /// 在到达截止时刻之前成功收到值：
    ///
    /// ```no_run
    /// #![feature(mpmc_channel)]
    ///
    /// use std::thread;
    /// use std::time::{Duration, Instant};
    /// use std::sync::mpmc;
    ///
    /// let (send, recv) = mpmc::channel();
    ///
    /// thread::spawn(move || {
    ///     send.send('a').unwrap();
    /// });
    ///
    /// assert_eq!(
    ///     recv.recv_deadline(Instant::now() + Duration::from_millis(400)),
    ///     Ok('a')
    /// );
    /// ```
    ///
    /// 到达截止时刻时收到一个错误：
    ///
    /// ```no_run
    /// #![feature(mpmc_channel)]
    ///
    /// use std::thread;
    /// use std::time::{Duration, Instant};
    /// use std::sync::mpmc;
    ///
    /// let (send, recv) = mpmc::channel();
    ///
    /// thread::spawn(move || {
    ///     thread::sleep(Duration::from_millis(800));
    ///     send.send('a').unwrap();
    /// });
    ///
    /// assert_eq!(
    ///     recv.recv_deadline(Instant::now() + Duration::from_millis(400)),
    ///     Err(mpmc::RecvTimeoutError::Timeout)
    /// );
    /// ```
    #[unstable(feature = "mpmc_channel", issue = "126840")]
    pub fn recv_deadline(&self, deadline: Instant) -> Result<T, RecvTimeoutError> {
        match &self.flavor {
            ReceiverFlavor::Array(chan) => chan.recv(Some(deadline)),
            ReceiverFlavor::List(chan) => chan.recv(Some(deadline)),
            ReceiverFlavor::Zero(chan) => chan.recv(Some(deadline)),
        }
    }

    /// 返回一个尝试取出所有待处理值的迭代器。当没有更多待处理值、或通道已挂断时，它会返回
    /// `None`。该迭代器既不会 [`panic!`]，也不会因等待值而阻塞使用者。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// #![feature(mpmc_channel)]
    ///
    /// use std::sync::mpmc::channel;
    /// use std::thread;
    /// use std::time::Duration;
    ///
    /// let (sender, receiver) = channel();
    ///
    /// // 缓冲区里暂时还什么都没有
    /// assert!(receiver.try_iter().next().is_none());
    ///
    /// thread::spawn(move || {
    ///     thread::sleep(Duration::from_secs(1));
    ///     sender.send(1).unwrap();
    ///     sender.send(2).unwrap();
    ///     sender.send(3).unwrap();
    /// });
    ///
    /// // 缓冲区里暂时还什么都没有
    /// assert!(receiver.try_iter().next().is_none());
    ///
    /// // 阻塞两秒
    /// thread::sleep(Duration::from_secs(2));
    ///
    /// let mut iter = receiver.try_iter();
    /// assert_eq!(iter.next(), Some(1));
    /// assert_eq!(iter.next(), Some(2));
    /// assert_eq!(iter.next(), Some(3));
    /// assert_eq!(iter.next(), None);
    /// ```
    #[unstable(feature = "mpmc_channel", issue = "126840")]
    pub fn try_iter(&self) -> TryIter<'_, T> {
        TryIter { rx: self }
    }
}

impl<T> Receiver<T> {
    /// 如果通道为空，返回 `true`。
    ///
    /// 注意：容量为零的通道永远为空。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(mpmc_channel)]
    ///
    /// use std::sync::mpmc;
    /// use std::thread;
    ///
    /// let (send, recv) = mpmc::channel();
    ///
    /// assert!(recv.is_empty());
    ///
    /// let handle = thread::spawn(move || {
    ///     send.send(1u8).unwrap();
    /// });
    ///
    /// handle.join().unwrap();
    ///
    /// assert!(!recv.is_empty());
    /// ```
    #[unstable(feature = "mpmc_channel", issue = "126840")]
    pub fn is_empty(&self) -> bool {
        match &self.flavor {
            ReceiverFlavor::Array(chan) => chan.is_empty(),
            ReceiverFlavor::List(chan) => chan.is_empty(),
            ReceiverFlavor::Zero(chan) => chan.is_empty(),
        }
    }

    /// 如果通道已满，返回 `true`。
    ///
    /// 注意：容量为零的通道永远是满的。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(mpmc_channel)]
    ///
    /// use std::sync::mpmc;
    /// use std::thread;
    ///
    /// let (send, recv) = mpmc::sync_channel(1);
    ///
    /// assert!(!recv.is_full());
    ///
    /// let handle = thread::spawn(move || {
    ///     send.send(1u8).unwrap();
    /// });
    ///
    /// handle.join().unwrap();
    ///
    /// assert!(recv.is_full());
    /// ```
    #[unstable(feature = "mpmc_channel", issue = "126840")]
    pub fn is_full(&self) -> bool {
        match &self.flavor {
            ReceiverFlavor::Array(chan) => chan.is_full(),
            ReceiverFlavor::List(chan) => chan.is_full(),
            ReceiverFlavor::Zero(chan) => chan.is_full(),
        }
    }

    /// 返回通道中消息的数量。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(mpmc_channel)]
    ///
    /// use std::sync::mpmc;
    /// use std::thread;
    ///
    /// let (send, recv) = mpmc::channel();
    ///
    /// assert_eq!(recv.len(), 0);
    ///
    /// let handle = thread::spawn(move || {
    ///     send.send(1u8).unwrap();
    /// });
    ///
    /// handle.join().unwrap();
    ///
    /// assert_eq!(recv.len(), 1);
    /// ```
    #[unstable(feature = "mpmc_channel", issue = "126840")]
    pub fn len(&self) -> usize {
        match &self.flavor {
            ReceiverFlavor::Array(chan) => chan.len(),
            ReceiverFlavor::List(chan) => chan.len(),
            ReceiverFlavor::Zero(chan) => chan.len(),
        }
    }

    /// 如果通道是有界的，返回其容量。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(mpmc_channel)]
    ///
    /// use std::sync::mpmc;
    /// use std::thread;
    ///
    /// let (send, recv) = mpmc::sync_channel(3);
    ///
    /// assert_eq!(recv.capacity(), Some(3));
    ///
    /// let handle = thread::spawn(move || {
    ///     send.send(1u8).unwrap();
    /// });
    ///
    /// handle.join().unwrap();
    ///
    /// assert_eq!(recv.capacity(), Some(3));
    /// ```
    #[unstable(feature = "mpmc_channel", issue = "126840")]
    pub fn capacity(&self) -> Option<usize> {
        match &self.flavor {
            ReceiverFlavor::Array(chan) => chan.capacity(),
            ReceiverFlavor::List(chan) => chan.capacity(),
            ReceiverFlavor::Zero(chan) => chan.capacity(),
        }
    }

    /// 如果两个接收者属于同一个通道，返回 `true`。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(mpmc_channel)]
    ///
    /// use std::sync::mpmc;
    ///
    /// let (_, rx1) = mpmc::channel::<i32>();
    /// let (_, rx2) = mpmc::channel::<i32>();
    ///
    /// assert!(rx1.same_channel(&rx1));
    /// assert!(!rx1.same_channel(&rx2));
    /// ```
    #[unstable(feature = "mpmc_channel", issue = "126840")]
    pub fn same_channel(&self, other: &Receiver<T>) -> bool {
        match (&self.flavor, &other.flavor) {
            (ReceiverFlavor::Array(a), ReceiverFlavor::Array(b)) => a == b,
            (ReceiverFlavor::List(a), ReceiverFlavor::List(b)) => a == b,
            (ReceiverFlavor::Zero(a), ReceiverFlavor::Zero(b)) => a == b,
            _ => false,
        }
    }

    /// 返回一个会阻塞等待消息、但绝不会 [`panic!`] 的迭代器。当通道挂断时，它会返回
    /// [`None`]。
    ///
    /// # 示例
    ///
    /// ```rust
    /// #![feature(mpmc_channel)]
    ///
    /// use std::sync::mpmc::channel;
    /// use std::thread;
    ///
    /// let (send, recv) = channel();
    ///
    /// thread::spawn(move || {
    ///     send.send(1).unwrap();
    ///     send.send(2).unwrap();
    ///     send.send(3).unwrap();
    /// });
    ///
    /// let mut iter = recv.iter();
    /// assert_eq!(iter.next(), Some(1));
    /// assert_eq!(iter.next(), Some(2));
    /// assert_eq!(iter.next(), Some(3));
    /// assert_eq!(iter.next(), None);
    /// ```
    #[unstable(feature = "mpmc_channel", issue = "126840")]
    pub fn iter(&self) -> Iter<'_, T> {
        Iter { rx: self }
    }
}

#[unstable(feature = "mpmc_channel", issue = "126840")]
impl<T> Drop for Receiver<T> {
    fn drop(&mut self) {
        unsafe {
            match &self.flavor {
                ReceiverFlavor::Array(chan) => chan.release(|c| c.disconnect_receivers()),
                ReceiverFlavor::List(chan) => chan.release(|c| c.disconnect_receivers()),
                ReceiverFlavor::Zero(chan) => chan.release(|c| c.disconnect()),
            }
        }
    }
}

#[unstable(feature = "mpmc_channel", issue = "126840")]
impl<T> Clone for Receiver<T> {
    fn clone(&self) -> Self {
        let flavor = match &self.flavor {
            ReceiverFlavor::Array(chan) => ReceiverFlavor::Array(chan.acquire()),
            ReceiverFlavor::List(chan) => ReceiverFlavor::List(chan.acquire()),
            ReceiverFlavor::Zero(chan) => ReceiverFlavor::Zero(chan.acquire()),
        };

        Receiver { flavor }
    }
}

#[unstable(feature = "mpmc_channel", issue = "126840")]
impl<T> fmt::Debug for Receiver<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Receiver").finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests;
