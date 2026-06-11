use super::Result;
use super::lifecycle::JoinInner;
use super::thread::Thread;
use crate::fmt;
use crate::sys::{AsInner, IntoInner, thread as imp};

/// 一项可用于 join 某个线程（阻塞等待其终止）的、被拥有的许可。
///
/// `JoinHandle` 在被丢弃时会*分离*（detach）其关联的线程，这意味着此后不再有
/// 任何指向该线程的句柄，也无法再对它 `join`。
///
/// 由于平台限制，无法 [`Clone`] 这个句柄：join 一个线程的能力是一项唯一拥有
/// （uniquely-owned）的许可。
///
/// 这个 `struct` 由 [`thread::spawn`] 函数和 [`thread::Builder::spawn`] 方法创建。
///
/// # Examples
///
/// 由 [`thread::spawn`] 创建：
///
/// ```
/// use std::thread;
///
/// let join_handle: thread::JoinHandle<_> = thread::spawn(|| {
///     // 此处是一些工作
/// });
/// ```
///
/// 由 [`thread::Builder::spawn`] 创建：
///
/// ```
/// use std::thread;
///
/// let builder = thread::Builder::new();
///
/// let join_handle: thread::JoinHandle<_> = builder.spawn(|| {
///     // 此处是一些工作
/// }).unwrap();
/// ```
///
/// 一个被分离、并比派生它的线程活得更久的线程：
///
/// ```no_run
/// use std::thread;
/// use std::time::Duration;
///
/// let original_thread = thread::spawn(|| {
///     let _detached_thread = thread::spawn(|| {
///         // 这里我们睡眠一下，以确保第一个线程先返回。
///         thread::sleep(Duration::from_millis(10));
///         // 即便 JoinHandle 被丢弃，这一句也会被执行。
///         println!("♫ Still alive ♫");
///     });
/// });
///
/// original_thread.join().expect("The thread being joined has panicked");
/// println!("Original thread is joined.");
///
/// // 我们确保新线程有时间运行，在主线程返回之前。
///
/// thread::sleep(Duration::from_millis(1000));
/// ```
///
/// [`thread::Builder::spawn`]: super::Builder::spawn
/// [`thread::spawn`]: super::spawn
#[stable(feature = "rust1", since = "1.0.0")]
#[cfg_attr(target_os = "teeos", must_use)]
pub struct JoinHandle<T>(pub(super) JoinInner<'static, T>);

#[stable(feature = "joinhandle_impl_send_sync", since = "1.29.0")]
unsafe impl<T> Send for JoinHandle<T> {}
#[stable(feature = "joinhandle_impl_send_sync", since = "1.29.0")]
unsafe impl<T> Sync for JoinHandle<T> {}

impl<T> JoinHandle<T> {
    /// 取出指向底层线程的句柄。
    ///
    /// # Examples
    ///
    /// ```
    /// use std::thread;
    ///
    /// let builder = thread::Builder::new();
    ///
    /// let join_handle: thread::JoinHandle<_> = builder.spawn(|| {
    ///     // 此处是一些工作
    /// }).unwrap();
    ///
    /// let thread = join_handle.thread();
    /// println!("thread id: {:?}", thread.id());
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[must_use]
    pub fn thread(&self) -> &Thread {
        self.0.thread()
    }

    /// 等待关联的线程完成。
    ///
    /// 如果关联的线程已经完成，本函数会立即返回。
    ///
    /// 用[原子内存序][atomic memory orderings]的术语来说，关联线程的完成与本函数
    /// 的返回之间存在同步（synchronizes with）关系。换句话说，那个线程所执行的
    /// 所有操作都[发生于（happen
    /// before）](https://doc.rust-lang.org/nomicon/atomics.html#data-accesses)
    /// `join` 返回之后发生的所有操作之前。
    ///
    /// 如果关联的线程发生 panic，则返回 [`Err`]，其中包含传给 [`panic!`] 的参数
    /// （不过请参阅下面的 Notes）。
    ///
    /// [`Err`]: crate::result::Result::Err
    /// [atomic memory orderings]: crate::sync::atomic
    ///
    /// # Panics
    ///
    /// 在某些平台上，如果一个线程试图 join 它自己，或以其他方式可能与正在 join
    /// 的线程造成死锁，本函数可能会 panic。
    ///
    /// # Examples
    ///
    /// ```
    /// use std::thread;
    ///
    /// let builder = thread::Builder::new();
    ///
    /// let join_handle: thread::JoinHandle<_> = builder.spawn(|| {
    ///     // 此处是一些工作
    /// }).unwrap();
    /// join_handle.join().expect("Couldn't join on the associated thread");
    /// ```
    ///
    /// # Notes
    ///
    /// 如果一个“外来（foreign）”的展开操作（例如从 C++ 代码抛出的异常，或来自以
    /// 不同运行时编译/链接的 Rust 代码中的 `panic!`）一路展开到线程根部，进程
    /// 可能会中止（abort）；参见 [`thread::spawn`] 上的 Notes。如果进程没有中止，
    /// 本函数会返回一个 `Result::Err`，其中包含一个不透明类型。
    ///
    /// [`catch_unwind`]: ../../std/panic/fn.catch_unwind.html
    /// [`thread::spawn`]: super::spawn
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn join(self) -> Result<T> {
        self.0.join()
    }

    /// 检查关联的线程是否已经运行完它的主函数。
    ///
    /// `is_finished` 支持实现一个非阻塞的 join 操作，做法是先检查 `is_finished`，
    /// 如果它返回 `true` 再调用 `join`。本函数不会阻塞。要阻塞等待线程完成，
    /// 请使用 [`join`][Self::join]。
    ///
    /// 在线程的主函数已经返回、但线程自身尚未停止运行的那一小段时间里，本函数
    /// 可能会返回 `true`。不过，一旦它返回 `true`，就可以预期
    /// [`join`][Self::join] 会很快返回，不会有任何明显的阻塞。
    #[stable(feature = "thread_is_running", since = "1.61.0")]
    pub fn is_finished(&self) -> bool {
        self.0.is_finished()
    }
}

impl<T> AsInner<imp::Thread> for JoinHandle<T> {
    fn as_inner(&self) -> &imp::Thread {
        self.0.as_inner()
    }
}

impl<T> IntoInner<imp::Thread> for JoinHandle<T> {
    fn into_inner(self) -> imp::Thread {
        self.0.into_inner()
    }
}

#[stable(feature = "std_debug", since = "1.16.0")]
impl<T> fmt::Debug for JoinHandle<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("JoinHandle").finish_non_exhaustive()
    }
}
