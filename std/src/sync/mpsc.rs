//! 多生产者、单消费者（multi-producer, single-consumer）FIFO 队列通信原语。
//!
//! 本模块提供基于消息的通道（channel）通信，具体由三个类型定义：
//!
//! * [`Sender`]
//! * [`SyncSender`]
//! * [`Receiver`]
//!
//! [`Sender`] 或 [`SyncSender`] 用于向 [`Receiver`] 发送数据。两种发送者都可克隆
//! （多生产者），因此多个线程可以同时向同一个接收者发送（单消费者）。
//!
//! 这些通道有两种 flavor（风味/变体）：
//!
//! 1. 异步、缓冲区无限大的通道。[`channel`] 函数返回一个 `(Sender, Receiver)` 元组，其中所有
//!    发送都是 **异步的**（永不阻塞）。该通道在概念上拥有一个无限大的缓冲区。
//!
//! 2. 同步、有界的通道。[`sync_channel`] 函数返回一个 `(SyncSender, Receiver)` 元组，其中待
//!    处理消息的存储是一块预先分配、固定大小的缓冲区。所有发送都是 **同步的**：当缓冲区满时
//!    会阻塞，直到有空位为止。注意：边界（bound）为 0 是允许的，此时通道变为“会合”
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
//! use std::thread;
//! use std::sync::mpsc::channel;
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
//! use std::thread;
//! use std::sync::mpsc::channel;
//!
//! // 创建一个可在多个线程间传递的共享通道，
//! // 其中 tx 是发送半（tx 取自 transmission），rx 是接收半
//! // （rx 取自 receiving）。
//! let (tx, rx) = channel();
//! for i in 0..10 {
//!     let tx = tx.clone();
//!     thread::spawn(move || {
//!         tx.send(i).unwrap();
//!     });
//! }
//!
//! for _ in 0..10 {
//!     let j = rx.recv().unwrap();
//!     assert!(0 <= j && j < 10);
//! }
//! ```
//!
//! 传播 panic：
//!
//! ```
//! use std::sync::mpsc::channel;
//!
//! // 这次 recv() 调用会返回一个错误，因为通道已经
//! // 挂断（或已被释放）
//! let (tx, rx) = channel::<i32>();
//! drop(tx);
//! assert!(rx.recv().is_err());
//! ```
//!
//! 同步通道：
//!
//! ```
//! use std::thread;
//! use std::sync::mpsc::sync_channel;
//!
//! let (tx, rx) = sync_channel::<i32>(0);
//! thread::spawn(move || {
//!     // 这将等待父线程开始接收
//!     tx.send(53).unwrap();
//! });
//! rx.recv().unwrap();
//! ```
//!
//! 无界接收循环：
//!
//! ```
//! use std::sync::mpsc::sync_channel;
//! use std::thread;
//!
//! let (tx, rx) = sync_channel(3);
//!
//! for _ in 0..3 {
//!     // 这里即便不用线程和 clone 结果也一样，
//!     // 因为始终还会剩下一个 `tx`。
//!     let tx = tx.clone();
//!     // 克隆出的 tx 在线程内被丢弃
//!     thread::spawn(move || tx.send("ok").unwrap());
//! }
//!
//! // 丢弃最后一个发送者，以停止 `rx` 对消息的等待。
//! // 如果把这一行注释掉，程序将不会结束。
//! // 必须丢弃 **所有** `tx`，`rx` 才会得到 `Err`。
//! drop(tx);
//!
//! // 无界接收者等待所有发送者完成。
//! while let Ok(msg) = rx.recv() {
//!     println!("{msg}");
//! }
//!
//! println!("completed");
//! ```

#![stable(feature = "rust1", since = "1.0.0")]

// MPSC 通道被构建为对 MPMC 通道的一层封装，而后者是从 `crossbeam-channel` crate 移植而来。
// MPMC 通道并未对外公开，但如果你对其实现感到好奇，所有东西都在那里。

use crate::sync::mpmc;
use crate::time::{Duration, Instant};
use crate::{error, fmt};

/// Rust [`channel`]（或 [`sync_channel`]）类型的接收半（receiving half）。
/// 这一半只能被一个线程拥有。
///
/// 发往通道的消息可以用 [`recv`] 取出。
///
/// [`recv`]: Receiver::recv
///
/// # 示例
///
/// ```rust
/// use std::sync::mpsc::channel;
/// use std::thread;
/// use std::time::Duration;
///
/// let (send, recv) = channel();
///
/// thread::spawn(move || {
///     send.send("Hello world!").unwrap();
///     thread::sleep(Duration::from_secs(2)); // 阻塞两秒
///     send.send("Delayed for 2 seconds").unwrap();
/// });
///
/// println!("{}", recv.recv().unwrap()); // 立即收到
/// println!("Waiting...");
/// println!("{}", recv.recv().unwrap()); // 两秒后收到
/// ```
#[stable(feature = "rust1", since = "1.0.0")]
#[cfg_attr(not(test), rustc_diagnostic_item = "Receiver")]
pub struct Receiver<T> {
    inner: mpmc::Receiver<T>,
}

// 接收端口可以在不同位置之间传递，只要不被用来接收不可发送（non-sendable）的东西即可。
#[stable(feature = "rust1", since = "1.0.0")]
unsafe impl<T: Send> Send for Receiver<T> {}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T> !Sync for Receiver<T> {}

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
/// use std::sync::mpsc::channel;
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
#[stable(feature = "rust1", since = "1.0.0")]
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
/// use std::sync::mpsc::channel;
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
#[stable(feature = "receiver_try_iter", since = "1.15.0")]
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
/// use std::sync::mpsc::channel;
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
#[stable(feature = "receiver_into_iter", since = "1.1.0")]
#[derive(Debug)]
pub struct IntoIter<T> {
    rx: Receiver<T>,
}

/// Rust 异步 [`channel`] 类型的发送半（sending-half）。
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
/// use std::sync::mpsc::channel;
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
#[stable(feature = "rust1", since = "1.0.0")]
pub struct Sender<T> {
    inner: mpmc::Sender<T>,
}

// 发送端口可以在不同位置之间传递，只要不被用来发送不可发送（non-sendable）的东西即可。
#[stable(feature = "rust1", since = "1.0.0")]
unsafe impl<T: Send> Send for Sender<T> {}

#[stable(feature = "mpsc_sender_sync", since = "1.72.0")]
unsafe impl<T: Send> Sync for Sender<T> {}

/// Rust 同步 [`sync_channel`] 类型的发送半（sending-half）。
///
/// 可以通过 [`send`] 或 [`try_send`] 经由该通道发送消息。
///
/// 如果内部缓冲区没有空间，[`send`] 会阻塞。
///
/// [`send`]: SyncSender::send
/// [`try_send`]: SyncSender::try_send
///
/// # 示例
///
/// ```rust
/// use std::sync::mpsc::sync_channel;
/// use std::thread;
///
/// // 创建一个缓冲区大小为 2 的 sync_channel
/// let (sync_sender, receiver) = sync_channel(2);
/// let sync_sender2 = sync_sender.clone();
///
/// // 第一个线程拥有 sync_sender
/// thread::spawn(move || {
///     sync_sender.send(1).unwrap();
///     sync_sender.send(2).unwrap();
/// });
///
/// // 第二个线程拥有 sync_sender2
/// thread::spawn(move || {
///     sync_sender2.send(3).unwrap();
///     // 由于缓冲区已满，线程此时会阻塞
///     println!("Thread unblocked!");
/// });
///
/// let mut msg;
///
/// msg = receiver.recv().unwrap();
/// println!("message {msg} received");
///
/// // 现在会打印出 "Thread unblocked!"
///
/// msg = receiver.recv().unwrap();
/// println!("message {msg} received");
///
/// msg = receiver.recv().unwrap();
///
/// println!("message {msg} received");
/// ```
#[stable(feature = "rust1", since = "1.0.0")]
pub struct SyncSender<T> {
    inner: mpmc::Sender<T>,
}

#[stable(feature = "rust1", since = "1.0.0")]
unsafe impl<T: Send> Send for SyncSender<T> {}

/// 从 **channel** 上的 [`Sender::send`] 或 [`SyncSender::send`] 函数返回的错误。
///
/// 一次 **send** 操作只有在通道的接收端已断连时才会失败，这意味着这份数据永远不可能被接收
/// 到。该错误以负载（payload）的形式携带着正在发送的数据，以便将其取回。
#[stable(feature = "rust1", since = "1.0.0")]
#[derive(PartialEq, Eq, Clone, Copy)]
pub struct SendError<T>(#[stable(feature = "rust1", since = "1.0.0")] pub T);

/// 从 [`Receiver`] 上的 [`recv`] 函数返回的错误。
///
/// [`recv`] 操作只有在 [`channel`]（或 [`sync_channel`]）的发送半已断连时才会失败，这意味着
/// 此后将永远不会再接收到任何消息。
///
/// [`recv`]: Receiver::recv
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
#[stable(feature = "rust1", since = "1.0.0")]
pub struct RecvError;

/// 这个枚举列出了 [`try_recv`] 被调用时可能无法返回数据的各种原因。这在 [`channel`] 和
/// [`sync_channel`] 上都可能发生。
///
/// [`try_recv`]: Receiver::try_recv
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
#[stable(feature = "rust1", since = "1.0.0")]
pub enum TryRecvError {
    /// 该 **channel** 当前为空，但 **Sender** 们尚未断连，因此之后数据仍可能变为可用。
    #[stable(feature = "rust1", since = "1.0.0")]
    Empty,

    /// 该 **channel** 的发送半已变为断连状态，此后将永远不会再从它接收到任何数据。
    #[stable(feature = "rust1", since = "1.0.0")]
    Disconnected,
}

/// 这个枚举列出了 [`recv_timeout`] 被调用时无法返回数据的各种可能错误。这在 [`channel`] 和
/// [`sync_channel`] 上都可能发生。
///
/// [`recv_timeout`]: Receiver::recv_timeout
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
#[stable(feature = "mpsc_recv_timeout", since = "1.12.0")]
pub enum RecvTimeoutError {
    /// 该 **channel** 当前为空，但 **Sender** 们尚未断连，因此之后数据仍可能变为可用。
    #[stable(feature = "mpsc_recv_timeout", since = "1.12.0")]
    Timeout,
    /// 该 **channel** 的发送半已变为断连状态，此后将永远不会再从它接收到任何数据。
    #[stable(feature = "mpsc_recv_timeout", since = "1.12.0")]
    Disconnected,
}

/// 这个枚举列出了 [`try_send`] 方法可能产生的各种错误结果。
///
/// [`try_send`]: SyncSender::try_send
#[stable(feature = "rust1", since = "1.0.0")]
#[derive(PartialEq, Eq, Clone, Copy)]
pub enum TrySendError<T> {
    /// 数据无法在该 [`sync_channel`] 上发送，因为发送它需要让调用方阻塞。
    ///
    /// 如果这是一个带缓冲的通道，那么此刻缓冲区已满。如果这不是带缓冲的通道，那么此刻没有
    /// 可用的 [`Receiver`] 来取走这份数据。
    #[stable(feature = "rust1", since = "1.0.0")]
    Full(#[stable(feature = "rust1", since = "1.0.0")] T),

    /// 该 [`sync_channel`] 的接收半已断连，因此数据无法被发送。这种情况下，数据会被退还给
    /// 调用方。
    #[stable(feature = "rust1", since = "1.0.0")]
    Disconnected(#[stable(feature = "rust1", since = "1.0.0")] T),
}

/// 创建一个新的异步通道，返回发送端/接收端这一对句柄。
///
/// 在 [`Sender`] 上发送的所有数据都会以发送时的相同顺序在 [`Receiver`] 处变为可用，且任何
/// [`send`] 都不会阻塞调用线程（该通道拥有“无限缓冲区”，这与 [`sync_channel`] 不同——后者
/// 在缓冲区达到上限后会阻塞）。只要至少还存在一个 [`Sender`]（含其克隆），[`recv`] 就会
/// 阻塞，直到有消息可用为止。
///
/// [`Sender`] 可以被克隆，以便多次向同一通道 [`send`]，但只支持一个 [`Receiver`]。
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
/// use std::sync::mpsc::channel;
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
#[stable(feature = "rust1", since = "1.0.0")]
pub fn channel<T>() -> (Sender<T>, Receiver<T>) {
    let (tx, rx) = mpmc::channel();
    (Sender { inner: tx }, Receiver { inner: rx })
}

/// 创建一个新的同步、有界的通道。
///
/// 在 [`SyncSender`] 上发送的所有数据都会以发送时的相同顺序在 [`Receiver`] 处变为可用。与
/// 异步的 [`channel`] 一样，[`Receiver`] 会阻塞直到有消息可用。然而 `sync_channel` 在发送端的
/// 语义上差别很大。
///
/// 该通道拥有一块内部缓冲区，消息会在其中排队。`bound` 指定缓冲区大小。当内部缓冲区变满时，
/// 后续的发送将 *阻塞*，等待缓冲区腾出空位。注意：缓冲区大小为 0 是合法的，此时通道变为
/// “会合通道”（rendezvous channel），即每次 [`send`] 都不会返回，直到有一次 [`recv`] 与之配对。
///
/// [`SyncSender`] 可以被克隆，以便多次向同一通道 [`send`]，但只支持一个 [`Receiver`]。
///
/// 与异步通道一样，如果在用 [`SyncSender`] 尝试 [`send`] 时 [`Receiver`] 已断连，则 [`send`]
/// 方法会返回一个 [`SendError`]。同理，如果在尝试 [`recv`] 时 [`SyncSender`] 已断连，则
/// [`recv`] 方法会返回一个 [`RecvError`]。
///
/// [`send`]: SyncSender::send
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
///     // 这将阻塞，直到前一条消息被接收
///     sender.send(2).unwrap();
/// });
///
/// assert_eq!(receiver.recv().unwrap(), 1);
/// assert_eq!(receiver.recv().unwrap(), 2);
/// ```
#[must_use]
#[stable(feature = "rust1", since = "1.0.0")]
pub fn sync_channel<T>(bound: usize) -> (SyncSender<T>, Receiver<T>) {
    let (tx, rx) = mpmc::sync_channel(bound);
    (SyncSender { inner: tx }, Receiver { inner: rx })
}

////////////////////////////////////////////////////////////////////////////////
// Sender
////////////////////////////////////////////////////////////////////////////////

impl<T> Sender<T> {
    /// 尝试在该通道上发送一个值；若无法发送，则将其原样返回。
    ///
    /// 当确定通道的另一端尚未挂断时，发送即为成功。发送不成功则意味着对应的接收者已被
    /// 释放。注意：返回值为 [`Err`] 意味着数据永远不会被接收到；但返回值为 [`Ok`] 并 *不*
    /// 意味着数据一定会被接收到——对应的接收者完全可能在本函数返回 [`Ok`] 之后立刻挂断。
    ///
    /// 此方法永远不会阻塞当前线程。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::sync::mpsc::channel;
    ///
    /// let (tx, rx) = channel();
    ///
    /// // 这次发送总是成功的
    /// tx.send(1).unwrap();
    ///
    /// // 这次发送会失败，因为接收者已经不在了
    /// drop(rx);
    /// assert_eq!(tx.send(1).unwrap_err().0, 1);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn send(&self, t: T) -> Result<(), SendError<T>> {
        self.inner.send(t)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T> Clone for Sender<T> {
    /// 克隆一个发送者，以便向其他线程发送。
    ///
    /// 注意，要留意发送者的生命周期：所有发送者（包括最初的那个）都必须被丢弃，
    /// [`Receiver::recv`] 才会停止阻塞。
    fn clone(&self) -> Sender<T> {
        Sender { inner: self.inner.clone() }
    }
}

#[stable(feature = "mpsc_debug", since = "1.8.0")]
impl<T> fmt::Debug for Sender<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Sender").finish_non_exhaustive()
    }
}

////////////////////////////////////////////////////////////////////////////////
// SyncSender
////////////////////////////////////////////////////////////////////////////////

impl<T> SyncSender<T> {
    /// 在该同步通道上发送一个值。
    ///
    /// 本函数将 *阻塞*，直到内部缓冲区出现空位，或者出现一个可供交付该消息的接收者。
    ///
    /// 注意：如果该通道带有缓冲区，则一次成功的发送 *并不* 保证接收者最终一定能看到这份
    /// 数据。条目可能被排入内部缓冲区，留待接收者稍后接收。然而，如果缓冲区大小为 0，则通道
    /// 变为会合（rendezvous）通道，此时只要本函数返回成功，就保证接收者确实已经收到了数据。
    ///
    /// 本函数永远不会 panic，但如果 [`Receiver`] 已断连、无法再接收信息，它可能返回 [`Err`]。
    ///
    /// # 示例
    ///
    /// ```rust
    /// use std::sync::mpsc::sync_channel;
    /// use std::thread;
    ///
    /// // 创建一个缓冲区大小为 0 的会合（rendezvous）sync_channel
    /// let (sync_sender, receiver) = sync_channel(0);
    ///
    /// thread::spawn(move || {
    ///    println!("sending message...");
    ///    sync_sender.send(1).unwrap();
    ///    // 线程此时会阻塞，直到消息被接收
    ///
    ///    println!("...message received!");
    /// });
    ///
    /// let msg = receiver.recv().unwrap();
    /// assert_eq!(1, msg);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn send(&self, t: T) -> Result<(), SendError<T>> {
        self.inner.send(t)
    }

    /// 尝试以非阻塞方式在该通道上发送一个值。
    ///
    /// 此方法与 [`send`] 的区别在于：如果通道的缓冲区已满、或没有接收者正在等待获取数据，
    /// 它会立即返回。与 [`send`] 相比，本函数有两种失败情形而非一种（一种是断连，另一种是
    /// 缓冲区已满）。
    ///
    /// 关于本函数成功时“接收者是否已收到数据”的保证说明，请参阅 [`send`]。
    ///
    /// [`send`]: Self::send
    ///
    /// # 示例
    ///
    /// ```rust
    /// use std::sync::mpsc::sync_channel;
    /// use std::thread;
    ///
    /// // 创建一个缓冲区大小为 1 的 sync_channel
    /// let (sync_sender, receiver) = sync_channel(1);
    /// let sync_sender2 = sync_sender.clone();
    ///
    /// // 第一个线程拥有 sync_sender
    /// let handle1 = thread::spawn(move || {
    ///     sync_sender.send(1).unwrap();
    ///     sync_sender.send(2).unwrap();
    ///     // 线程阻塞
    /// });
    ///
    /// // 第二个线程拥有 sync_sender2
    /// let handle2 = thread::spawn(move || {
    ///     // 如果缓冲区已满，这将返回一个错误
    ///     // 且不会发送任何消息
    ///     let _ = sync_sender2.try_send(3);
    /// });
    ///
    /// let mut msg;
    /// msg = receiver.recv().unwrap();
    /// println!("message {msg} received");
    ///
    /// msg = receiver.recv().unwrap();
    /// println!("message {msg} received");
    ///
    /// // 第三条消息可能从未被发送
    /// match receiver.try_recv() {
    ///     Ok(msg) => println!("message {msg} received"),
    ///     Err(_) => println!("the third message was never sent"),
    /// }
    ///
    /// // 等待线程完成
    /// handle1.join().unwrap();
    /// handle2.join().unwrap();
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn try_send(&self, t: T) -> Result<(), TrySendError<T>> {
        self.inner.try_send(t)
    }

    // 尝试在该接收者上等待一个值；如果对应的通道已挂断、或等待时间超过 `timeout`，则返回
    // 一个错误。
    //
    // 此方法目前仅用于测试。
    #[unstable(issue = "none", feature = "std_internals")]
    #[doc(hidden)]
    pub fn send_timeout(&self, t: T, timeout: Duration) -> Result<(), mpmc::SendTimeoutError<T>> {
        self.inner.send_timeout(t, timeout)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T> Clone for SyncSender<T> {
    fn clone(&self) -> SyncSender<T> {
        SyncSender { inner: self.inner.clone() }
    }
}

#[stable(feature = "mpsc_debug", since = "1.8.0")]
impl<T> fmt::Debug for SyncSender<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SyncSender").finish_non_exhaustive()
    }
}

////////////////////////////////////////////////////////////////////////////////
// Receiver
////////////////////////////////////////////////////////////////////////////////

impl<T> Receiver<T> {
    /// 尝试以非阻塞方式返回该接收者上一个待处理（pending）的值。
    ///
    /// 此方法为了等待数据可用而 **永不** 阻塞调用方；相反，它总是立即返回，结果是一个可能
    /// 携带通道上待处理数据的 option。
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
    /// use std::sync::mpsc::{Receiver, channel};
    ///
    /// let (_, receiver): (_, Receiver<i32>) = channel();
    ///
    /// assert!(receiver.try_recv().is_err());
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn try_recv(&self) -> Result<T, TryRecvError> {
        self.inner.try_recv()
    }

    /// 尝试在该接收者上等待一个值；如果对应的通道已挂断，则返回一个错误。
    ///
    /// 只要没有数据可用、且仍有可能发来更多数据（即至少还存在一个发送者），本函数就总会
    /// 阻塞当前线程。一旦有消息被发往对应的 [`Sender`]（或 [`SyncSender`]），该接收者便会被
    /// 唤醒并返回那条消息。
    ///
    /// 如果对应的 [`Sender`] 已断连，或在本次调用阻塞期间发生断连，本次调用会被唤醒并返回
    /// [`Err`]，以表明此通道上永远不会再收到任何消息了。不过，由于通道是带缓冲的，在断连
    /// 之前发送的消息仍会被正确接收。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::sync::mpsc;
    /// use std::thread;
    ///
    /// let (send, recv) = mpsc::channel();
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
    /// use std::sync::mpsc;
    /// use std::thread;
    /// use std::sync::mpsc::RecvError;
    ///
    /// let (send, recv) = mpsc::channel();
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
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn recv(&self) -> Result<T, RecvError> {
        self.inner.recv()
    }

    /// 尝试在该接收者上等待一个值；如果对应的通道已挂断、或等待时间超过 `timeout`，则返回
    /// 一个错误。
    ///
    /// 只要没有数据可用、且仍有可能发来更多数据（即至少还存在一个发送者），本函数就总会
    /// 阻塞当前线程。一旦有消息被发往对应的 [`Sender`]（或 [`SyncSender`]），该接收者便会被
    /// 唤醒并返回那条消息。
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
    /// use std::thread;
    /// use std::time::Duration;
    /// use std::sync::mpsc;
    ///
    /// let (send, recv) = mpsc::channel();
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
    /// use std::thread;
    /// use std::time::Duration;
    /// use std::sync::mpsc;
    ///
    /// let (send, recv) = mpsc::channel();
    ///
    /// thread::spawn(move || {
    ///     thread::sleep(Duration::from_millis(800));
    ///     send.send('a').unwrap();
    /// });
    ///
    /// assert_eq!(
    ///     recv.recv_timeout(Duration::from_millis(400)),
    ///     Err(mpsc::RecvTimeoutError::Timeout)
    /// );
    /// ```
    #[stable(feature = "mpsc_recv_timeout", since = "1.12.0")]
    pub fn recv_timeout(&self, timeout: Duration) -> Result<T, RecvTimeoutError> {
        self.inner.recv_timeout(timeout)
    }

    /// 尝试在该接收者上等待一个值；如果对应的通道已挂断、或到达了 `deadline`，则返回一个
    /// 错误。
    ///
    /// 只要没有数据可用、且仍有可能发来更多数据，本函数就总会阻塞当前线程。一旦有消息被
    /// 发往对应的 [`Sender`]（或 [`SyncSender`]），该接收者便会被唤醒并返回那条消息。
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
    /// #![feature(deadline_api)]
    /// use std::thread;
    /// use std::time::{Duration, Instant};
    /// use std::sync::mpsc;
    ///
    /// let (send, recv) = mpsc::channel();
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
    /// #![feature(deadline_api)]
    /// use std::thread;
    /// use std::time::{Duration, Instant};
    /// use std::sync::mpsc;
    ///
    /// let (send, recv) = mpsc::channel();
    ///
    /// thread::spawn(move || {
    ///     thread::sleep(Duration::from_millis(800));
    ///     send.send('a').unwrap();
    /// });
    ///
    /// assert_eq!(
    ///     recv.recv_deadline(Instant::now() + Duration::from_millis(400)),
    ///     Err(mpsc::RecvTimeoutError::Timeout)
    /// );
    /// ```
    #[unstable(feature = "deadline_api", issue = "46316")]
    pub fn recv_deadline(&self, deadline: Instant) -> Result<T, RecvTimeoutError> {
        self.inner.recv_deadline(deadline)
    }

    /// 返回一个会阻塞等待消息、但绝不会 [`panic!`] 的迭代器。当通道挂断时，它会返回
    /// [`None`]。
    ///
    /// # 示例
    ///
    /// ```rust
    /// use std::sync::mpsc::channel;
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
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn iter(&self) -> Iter<'_, T> {
        Iter { rx: self }
    }

    /// 返回一个会尝试取出所有待处理值的迭代器。当没有更多待处理值、或通道已挂断时，它会
    /// 返回 `None`。该迭代器既不会 [`panic!`]，也不会因等待值而阻塞使用者。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::sync::mpsc::channel;
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
    #[stable(feature = "receiver_try_iter", since = "1.15.0")]
    pub fn try_iter(&self) -> TryIter<'_, T> {
        TryIter { rx: self }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<'a, T> Iterator for Iter<'a, T> {
    type Item = T;

    fn next(&mut self) -> Option<T> {
        self.rx.recv().ok()
    }
}

#[stable(feature = "receiver_try_iter", since = "1.15.0")]
impl<'a, T> Iterator for TryIter<'a, T> {
    type Item = T;

    fn next(&mut self) -> Option<T> {
        self.rx.try_recv().ok()
    }
}

#[stable(feature = "receiver_into_iter", since = "1.1.0")]
impl<'a, T> IntoIterator for &'a Receiver<T> {
    type Item = T;
    type IntoIter = Iter<'a, T>;

    fn into_iter(self) -> Iter<'a, T> {
        self.iter()
    }
}

#[stable(feature = "receiver_into_iter", since = "1.1.0")]
impl<T> Iterator for IntoIter<T> {
    type Item = T;
    fn next(&mut self) -> Option<T> {
        self.rx.recv().ok()
    }
}

#[stable(feature = "receiver_into_iter", since = "1.1.0")]
impl<T> IntoIterator for Receiver<T> {
    type Item = T;
    type IntoIter = IntoIter<T>;

    fn into_iter(self) -> IntoIter<T> {
        IntoIter { rx: self }
    }
}

#[stable(feature = "mpsc_debug", since = "1.8.0")]
impl<T> fmt::Debug for Receiver<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Receiver").finish_non_exhaustive()
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T> fmt::Debug for SendError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SendError").finish_non_exhaustive()
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T> fmt::Display for SendError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        "sending on a closed channel".fmt(f)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T> error::Error for SendError<T> {}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T> fmt::Debug for TrySendError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            TrySendError::Full(..) => f.debug_tuple("TrySendError::Full").finish_non_exhaustive(),
            TrySendError::Disconnected(..) => {
                f.debug_tuple("TrySendError::Disconnected").finish_non_exhaustive()
            }
        }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T> fmt::Display for TrySendError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            TrySendError::Full(..) => "sending on a full channel".fmt(f),
            TrySendError::Disconnected(..) => "sending on a closed channel".fmt(f),
        }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T> error::Error for TrySendError<T> {}

#[stable(feature = "mpsc_error_conversions", since = "1.24.0")]
impl<T> From<SendError<T>> for TrySendError<T> {
    /// 把一个 `SendError<T>` 转换为 `TrySendError<T>`。
    ///
    /// 此转换总是返回一个 `TrySendError::Disconnected`，其中携带着 `SendError<T>` 里的数据。
    ///
    /// 不会在堆上分配任何数据。
    fn from(err: SendError<T>) -> TrySendError<T> {
        match err {
            SendError(t) => TrySendError::Disconnected(t),
        }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl fmt::Display for RecvError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        "receiving on a closed channel".fmt(f)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl error::Error for RecvError {}

#[stable(feature = "rust1", since = "1.0.0")]
impl fmt::Display for TryRecvError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            TryRecvError::Empty => "receiving on an empty channel".fmt(f),
            TryRecvError::Disconnected => "receiving on a closed channel".fmt(f),
        }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl error::Error for TryRecvError {}

#[stable(feature = "mpsc_error_conversions", since = "1.24.0")]
impl From<RecvError> for TryRecvError {
    /// 把一个 `RecvError` 转换为 `TryRecvError`。
    ///
    /// 此转换总是返回 `TryRecvError::Disconnected`。
    ///
    /// 不会在堆上分配任何数据。
    fn from(err: RecvError) -> TryRecvError {
        match err {
            RecvError => TryRecvError::Disconnected,
        }
    }
}

#[stable(feature = "mpsc_recv_timeout_error", since = "1.15.0")]
impl fmt::Display for RecvTimeoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            RecvTimeoutError::Timeout => "timed out waiting on channel".fmt(f),
            RecvTimeoutError::Disconnected => "channel is empty and sending half is closed".fmt(f),
        }
    }
}

#[stable(feature = "mpsc_recv_timeout_error", since = "1.15.0")]
impl error::Error for RecvTimeoutError {}

#[stable(feature = "mpsc_error_conversions", since = "1.24.0")]
impl From<RecvError> for RecvTimeoutError {
    /// 把一个 `RecvError` 转换为 `RecvTimeoutError`。
    ///
    /// 此转换总是返回 `RecvTimeoutError::Disconnected`。
    ///
    /// 不会在堆上分配任何数据。
    fn from(err: RecvError) -> RecvTimeoutError {
        match err {
            RecvError => RecvTimeoutError::Disconnected,
        }
    }
}
