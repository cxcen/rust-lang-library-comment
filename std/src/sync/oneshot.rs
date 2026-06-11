//! 单生产者、单消费者（single-producer, single-consumer，简称 oneshot）通道。
//!
//! 这是一个实验性模块，因此其 API 很可能会发生变化。

use crate::sync::mpmc;
use crate::sync::mpsc::{RecvError, SendError};
use crate::time::{Duration, Instant};
use crate::{error, fmt};

/// 创建一个新的 oneshot 通道，返回发送端/接收端这一对句柄。
///
/// # 示例
///
/// ```
/// #![feature(oneshot_channel)]
/// use std::sync::oneshot;
/// use std::thread;
///
/// let (sender, receiver) = oneshot::channel();
///
/// // 启动一项开销很大的计算。
/// thread::spawn(move || {
/// #   fn expensive_computation() -> i32 { 42 }
///     sender.send(expensive_computation()).unwrap();
///     // `sender` 已被 `send` 消耗（consume），因此我们无法再使用它。
/// });
///
/// # fn do_other_work() -> i32 { 42 }
/// do_other_work();
///
/// // 来看看那个答案是什么……
/// println!("{:?}", receiver.recv().unwrap());
/// // `receiver` 已被 `recv` 消耗（consume），因此我们无法再使用它。
/// ```
#[must_use]
#[unstable(feature = "oneshot_channel", issue = "143674")]
pub fn channel<T>() -> (Sender<T>, Receiver<T>) {
    // 使用一个容量为 1 的 `sync_channel`，意味着其内部会采用 `Array` flavor 的通道实现。
    let (sender, receiver) = mpmc::sync_channel(1);
    (Sender { inner: sender }, Receiver { inner: receiver })
}

////////////////////////////////////////////////////////////////////////////////////////////////////
// Sender
////////////////////////////////////////////////////////////////////////////////////////////////////

/// oneshot 通道的发送半（sending half）。
///
/// # 示例
///
/// ```
/// #![feature(oneshot_channel)]
/// use std::sync::oneshot;
/// use std::thread;
///
/// let (sender, receiver) = oneshot::channel();
///
/// thread::spawn(move || {
///     sender.send("Hello from thread!").unwrap();
/// });
///
/// assert_eq!(receiver.recv().unwrap(), "Hello from thread!");
/// ```
///
/// 如果 `Sender` 发送的是非 `Send` 类型，则它不能在线程间传递。
///
/// ```compile_fail
/// #![feature(oneshot_channel)]
/// use std::sync::oneshot;
/// use std::thread;
/// use std::ptr;
///
/// let (sender, receiver) = oneshot::channel();
///
/// struct NotSend(*mut ());
/// thread::spawn(move || {
///     sender.send(NotSend(ptr::null_mut()));
/// });
///
/// let reply = receiver.try_recv().unwrap();
/// ```
#[unstable(feature = "oneshot_channel", issue = "143674")]
pub struct Sender<T> {
    /// `oneshot` 通道只是对 `mpmc` 通道的一层简单封装。
    inner: mpmc::Sender<T>,
}

// SAFETY: 由于唯一需要进行同步的那些方法都会取得 [`Sender`] 的完整所有权，因此在线程间共享
// 一个 `&Sender` 是完全安全的（因为没有所有权时它实际上毫无用处）。
#[unstable(feature = "oneshot_channel", issue = "143674")]
unsafe impl<T> Sync for Sender<T> {}

impl<T> Sender<T> {
    /// 尝试通过该通道发送一个值。只有当对应的 [`Receiver<T>`] 已被丢弃时，本操作才会失败。
    ///
    /// 此方法是非阻塞的（wait-free，无等待）。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(oneshot_channel)]
    /// use std::sync::oneshot;
    /// use std::thread;
    ///
    /// let (tx, rx) = oneshot::channel();
    ///
    /// thread::spawn(move || {
    ///     // 执行一些计算。
    ///     let result = 2 + 2;
    ///     tx.send(result).unwrap();
    /// });
    ///
    /// assert_eq!(rx.recv().unwrap(), 4);
    /// ```
    #[unstable(feature = "oneshot_channel", issue = "143674")]
    pub fn send(self, t: T) -> Result<(), SendError<T>> {
        self.inner.send(t)
    }
}

#[unstable(feature = "oneshot_channel", issue = "143674")]
impl<T> fmt::Debug for Sender<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Sender").finish_non_exhaustive()
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////
// Receiver
////////////////////////////////////////////////////////////////////////////////////////////////////

/// oneshot 通道的接收半（receiving half）。
///
/// # 示例
///
/// ```
/// #![feature(oneshot_channel)]
/// use std::sync::oneshot;
/// use std::thread;
/// use std::time::Duration;
///
/// let (sender, receiver) = oneshot::channel();
///
/// thread::spawn(move || {
///     thread::sleep(Duration::from_millis(100));
///     sender.send("Hello after delay!").unwrap();
/// });
///
/// println!("Waiting for message...");
/// println!("{}", receiver.recv().unwrap());
/// ```
///
/// 如果 `Receiver` 接收的是非 `Send` 类型，则它不能在线程间传递。
///
/// ```compile_fail
/// # #![feature(oneshot_channel)]
/// # use std::sync::oneshot;
/// # use std::thread;
/// # use std::ptr;
/// #
/// let (sender, receiver) = oneshot::channel();
///
/// struct NotSend(*mut ());
/// sender.send(NotSend(ptr::null_mut()));
///
/// thread::spawn(move || {
///     let reply = receiver.try_recv().unwrap();
/// });
/// ```
#[unstable(feature = "oneshot_channel", issue = "143674")]
pub struct Receiver<T> {
    /// `oneshot` 通道只是对 `mpmc` 通道的一层简单封装。
    inner: mpmc::Receiver<T>,
}

// SAFETY: 由于唯一需要进行同步的那些方法都会取得 [`Receiver`] 的完整所有权，因此在线程间
// 共享一个 `&Receiver` 是完全安全的（因为没有所有权时它无法接收任何值）。
#[unstable(feature = "oneshot_channel", issue = "143674")]
unsafe impl<T> Sync for Receiver<T> {}

impl<T> Receiver<T> {
    /// 从发送端接收值，并阻塞调用线程直到取得该值为止。
    ///
    /// 只有当对应的 [`Sender<T>`] 已被丢弃时，本操作才会失败。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(oneshot_channel)]
    /// use std::sync::oneshot;
    /// use std::thread;
    /// use std::time::Duration;
    ///
    /// let (tx, rx) = oneshot::channel();
    ///
    /// thread::spawn(move || {
    ///     thread::sleep(Duration::from_millis(500));
    ///     tx.send("Done!").unwrap();
    /// });
    ///
    /// // 这将阻塞，直到消息到达。
    /// println!("{}", rx.recv().unwrap());
    /// ```
    #[unstable(feature = "oneshot_channel", issue = "143674")]
    pub fn recv(self) -> Result<T, RecvError> {
        self.inner.recv()
    }

    // 可失败（fallible）的方法。

    /// 尝试以非阻塞方式返回该接收者上一个待处理（pending）的值。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(oneshot_channel)]
    /// use std::sync::oneshot;
    /// use std::thread;
    /// use std::time::Duration;
    ///
    /// let (sender, mut receiver) = oneshot::channel();
    ///
    /// thread::spawn(move || {
    ///     thread::sleep(Duration::from_millis(100));
    ///     sender.send(42).unwrap();
    /// });
    ///
    /// // 不断重试直到拿到消息，期间还顺便做些别的工作。
    /// loop {
    ///     match receiver.try_recv() {
    ///         Ok(value) => {
    ///             assert_eq!(value, 42);
    ///             break;
    ///         }
    ///         Err(oneshot::TryRecvError::Empty(rx)) => {
    ///             // 重新取回接收者的所有权。
    ///             receiver = rx;
    /// #           fn do_other_work() { thread::sleep(Duration::from_millis(25)); }
    ///             do_other_work();
    ///         }
    ///         Err(oneshot::TryRecvError::Disconnected) => panic!("Sender disconnected"),
    ///     }
    /// }
    /// ```
    #[unstable(feature = "oneshot_channel", issue = "143674")]
    pub fn try_recv(self) -> Result<T, TryRecvError<T>> {
        self.inner.try_recv().map_err(|err| match err {
            mpmc::TryRecvError::Empty => TryRecvError::Empty(self),
            mpmc::TryRecvError::Disconnected => TryRecvError::Disconnected,
        })
    }

    /// 尝试在该接收者上等待一个值；如果该通道对应的 [`Sender`] 半已被丢弃、或等待时间超过
    /// `timeout`，则返回一个错误。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(oneshot_channel)]
    /// use std::sync::oneshot;
    /// use std::thread;
    /// use std::time::Duration;
    ///
    /// let (sender, receiver) = oneshot::channel();
    ///
    /// thread::spawn(move || {
    ///     thread::sleep(Duration::from_millis(500));
    ///     sender.send("Success!").unwrap();
    /// });
    ///
    /// // 最多等待 1 秒来获取消息
    /// match receiver.recv_timeout(Duration::from_secs(1)) {
    ///     Ok(msg) => println!("Received: {}", msg),
    ///     Err(oneshot::RecvTimeoutError::Timeout(_)) => println!("Timed out!"),
    ///     Err(oneshot::RecvTimeoutError::Disconnected) => println!("Sender dropped!"),
    /// }
    /// ```
    #[unstable(feature = "oneshot_channel", issue = "143674")]
    pub fn recv_timeout(self, timeout: Duration) -> Result<T, RecvTimeoutError<T>> {
        self.inner.recv_timeout(timeout).map_err(|err| match err {
            mpmc::RecvTimeoutError::Timeout => RecvTimeoutError::Timeout(self),
            mpmc::RecvTimeoutError::Disconnected => RecvTimeoutError::Disconnected,
        })
    }

    /// 尝试在该接收者上等待一个值；如果该通道对应的 [`Sender`] 半已被丢弃、或到达了
    /// `deadline`，则返回一个错误。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(oneshot_channel)]
    /// use std::sync::oneshot;
    /// use std::thread;
    /// use std::time::{Duration, Instant};
    ///
    /// let (sender, receiver) = oneshot::channel();
    ///
    /// thread::spawn(move || {
    ///     thread::sleep(Duration::from_millis(100));
    ///     sender.send("Just in time!").unwrap();
    /// });
    ///
    /// let deadline = Instant::now() + Duration::from_millis(500);
    /// match receiver.recv_deadline(deadline) {
    ///     Ok(msg) => println!("Received: {}", msg),
    ///     Err(oneshot::RecvTimeoutError::Timeout(_)) => println!("Missed deadline!"),
    ///     Err(oneshot::RecvTimeoutError::Disconnected) => println!("Sender dropped!"),
    /// }
    /// ```
    #[unstable(feature = "oneshot_channel", issue = "143674")]
    pub fn recv_deadline(self, deadline: Instant) -> Result<T, RecvTimeoutError<T>> {
        self.inner.recv_deadline(deadline).map_err(|err| match err {
            mpmc::RecvTimeoutError::Timeout => RecvTimeoutError::Timeout(self),
            mpmc::RecvTimeoutError::Disconnected => RecvTimeoutError::Disconnected,
        })
    }
}

#[unstable(feature = "oneshot_channel", issue = "143674")]
impl<T> fmt::Debug for Receiver<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Receiver").finish_non_exhaustive()
    }
}

////////////////////////////////////////////////////////////////////////////////////////////////////
// Receiver Errors
////////////////////////////////////////////////////////////////////////////////////////////////////

/// 从 [`try_recv`](Receiver::try_recv) 方法返回的错误。
///
/// 关于如何使用该错误的更多信息，请参阅 [`try_recv`] 的文档。
///
/// [`try_recv`]: Receiver::try_recv
#[unstable(feature = "oneshot_channel", issue = "143674")]
pub enum TryRecvError<T> {
    /// [`Sender`] 尚未发送消息，但将来仍可能发送（因为它尚未断连）。该变体携带着被
    /// [`try_recv`](Receiver::try_recv) 取得所有权的那个 [`Receiver`]。
    Empty(Receiver<T>),
    /// 该通道对应的 [`Sender`] 半已变为断连状态，此后通道上将永远不会再有任何数据被发送。
    Disconnected,
}

/// 从 [`recv_timeout`](Receiver::recv_timeout) 或 [`recv_deadline`](Receiver::recv_deadline)
/// 方法返回的错误。
///
/// # 示例
///
/// 该错误的用法与 [`TryRecvError`] 类似。
///
/// ```
/// #![feature(oneshot_channel)]
/// use std::sync::oneshot::{self, RecvTimeoutError};
/// use std::thread;
/// use std::time::Duration;
///
/// let (sender, receiver) = oneshot::channel();
///
/// let send_failure = thread::spawn(move || {
///     // 模拟一项耗时超过我们超时上限的长计算。
///     thread::sleep(Duration::from_millis(250));
///
///     // 这次发送很可能会失败，因为我们在主线程中丢弃了接收者。
///     sender.send("Goodbye!".to_string()).unwrap();
/// });
///
/// // 用一个较短的超时来尝试接收消息。
/// match receiver.recv_timeout(Duration::from_millis(10)) {
///     Ok(msg) => println!("Received: {}", msg),
///     Err(RecvTimeoutError::Timeout(rx)) => {
///         println!("Timed out waiting for message!");
///
///         // 注意，你无需丢弃接收者就可以将其复用。
///         drop(rx);
///     },
///     Err(RecvTimeoutError::Disconnected) => println!("Sender dropped!"),
/// }
///
/// send_failure.join().unwrap_err();
/// ```
#[unstable(feature = "oneshot_channel", issue = "143674")]
pub enum RecvTimeoutError<T> {
    /// [`Sender`] 尚未发送消息，但将来仍可能发送（因为它尚未断连）。该变体携带着被
    /// [`recv_timeout`](Receiver::recv_timeout) 或 [`recv_deadline`](Receiver::recv_deadline)
    /// 取得所有权的那个 [`Receiver`]。
    Timeout(Receiver<T>),
    /// 该通道对应的 [`Sender`] 半已变为断连状态，此后通道上将永远不会再有任何数据被发送。
    Disconnected,
}

#[unstable(feature = "oneshot_channel", issue = "143674")]
impl<T> fmt::Debug for TryRecvError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("TryRecvError").finish_non_exhaustive()
    }
}

#[unstable(feature = "oneshot_channel", issue = "143674")]
impl<T> fmt::Display for TryRecvError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            TryRecvError::Empty(..) => "receiving on an empty oneshot channel".fmt(f),
            TryRecvError::Disconnected => "receiving on a closed oneshot channel".fmt(f),
        }
    }
}

#[unstable(feature = "oneshot_channel", issue = "143674")]
impl<T> error::Error for TryRecvError<T> {}

#[unstable(feature = "oneshot_channel", issue = "143674")]
impl<T> From<RecvError> for TryRecvError<T> {
    /// 把一个 `RecvError` 转换为 `TryRecvError`。
    ///
    /// 此转换总是返回 `TryRecvError::Disconnected`。
    ///
    /// 不会在堆上分配任何数据。
    fn from(err: RecvError) -> TryRecvError<T> {
        match err {
            RecvError => TryRecvError::Disconnected,
        }
    }
}

#[unstable(feature = "oneshot_channel", issue = "143674")]
impl<T> fmt::Debug for RecvTimeoutError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("RecvTimeoutError").finish_non_exhaustive()
    }
}

#[unstable(feature = "oneshot_channel", issue = "143674")]
impl<T> fmt::Display for RecvTimeoutError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            RecvTimeoutError::Timeout(..) => "timed out waiting on oneshot channel".fmt(f),
            RecvTimeoutError::Disconnected => "receiving on a closed oneshot channel".fmt(f),
        }
    }
}

#[unstable(feature = "oneshot_channel", issue = "143674")]
impl<T> error::Error for RecvTimeoutError<T> {}

#[unstable(feature = "oneshot_channel", issue = "143674")]
impl<T> From<RecvError> for RecvTimeoutError<T> {
    /// 把一个 `RecvError` 转换为 `RecvTimeoutError`。
    ///
    /// 此转换总是返回 `RecvTimeoutError::Disconnected`。
    ///
    /// 不会在堆上分配任何数据。
    fn from(err: RecvError) -> RecvTimeoutError<T> {
        match err {
            RecvError => RecvTimeoutError::Disconnected,
        }
    }
}
