use super::join_handle::JoinHandle;
use super::lifecycle::spawn_unchecked;
use crate::io;

/// 线程工厂，可用于配置新线程的各项属性。
///
/// 可以在它上面链式调用方法来进行配置。
///
/// 目前可用的两项配置是：
///
/// - [`name`]：为线程指定[一个关联名字][naming-threads]
/// - [`stack_size`]：为线程指定[期望的栈大小][stack-size]
///
/// [`spawn`] 方法会取得 builder 的所有权，并根据给定配置创建出一个指向线程句柄
/// 的 [`io::Result`]。
///
/// [`thread::spawn`] 自由函数使用一个采用默认配置的 `Builder`，并对它的返回值
/// 调用 [`unwrap`]。
///
/// 当你希望能从启动线程失败的情形中恢复时，可能会想用 [`spawn`] 而不是
/// [`thread::spawn`]——的确，自由函数会 panic，而 `Builder` 方法则会返回一个
/// [`io::Result`]。
///
/// # Examples
///
/// ```
/// use std::thread;
///
/// let builder = thread::Builder::new();
///
/// let handler = builder.spawn(|| {
///     // 线程代码
/// }).unwrap();
///
/// handler.join().unwrap();
/// ```
///
/// [`stack_size`]: Builder::stack_size
/// [`name`]: Builder::name
/// [`spawn`]: Builder::spawn
/// [`thread::spawn`]: super::spawn
/// [`unwrap`]: crate::result::Result::unwrap
/// [naming-threads]: ./index.html#naming-threads
/// [stack-size]: ./index.html#stack-size
#[must_use = "must eventually spawn the thread"]
#[stable(feature = "rust1", since = "1.0.0")]
#[derive(Debug)]
pub struct Builder {
    /// 待创建线程的名字，用于在 panic 消息中进行标识
    pub(super) name: Option<String>,
    /// 派生线程的栈大小（以字节为单位）
    pub(super) stack_size: Option<usize>,
    /// 跳过运行和继承线程的 spawn hook
    pub(super) no_hooks: bool,
}

impl Builder {
    /// 生成派生线程所需的基础配置，可在其上链式调用各配置方法。
    ///
    /// # Examples
    ///
    /// ```
    /// use std::thread;
    ///
    /// let builder = thread::Builder::new()
    ///                               .name("foo".into())
    ///                               .stack_size(32 * 1024);
    ///
    /// let handler = builder.spawn(|| {
    ///     // 线程代码
    /// }).unwrap();
    ///
    /// handler.join().unwrap();
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn new() -> Builder {
        Builder { name: None, stack_size: None, no_hooks: false }
    }

    /// 为待创建的线程命名。目前该名字仅用于在 panic 消息中进行标识。
    ///
    /// 名字不能包含空字节（`\0`）。
    ///
    /// 关于具名线程的更多信息，请参阅
    /// [此模块级文档][naming-threads]。
    ///
    /// # Examples
    ///
    /// ```
    /// use std::thread;
    ///
    /// let builder = thread::Builder::new()
    ///     .name("foo".into());
    ///
    /// let handler = builder.spawn(|| {
    ///     assert_eq!(thread::current().name(), Some("foo"))
    /// }).unwrap();
    ///
    /// handler.join().unwrap();
    /// ```
    ///
    /// [naming-threads]: ./index.html#naming-threads
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn name(mut self, name: String) -> Builder {
        self.name = Some(name);
        self
    }

    /// 设置新线程的栈大小（以字节为单位）。
    ///
    /// 如果平台规定了一个最小栈大小，则实际栈大小可能大于这个值。
    ///
    /// 关于线程栈大小的更多信息，请参阅
    /// [此模块级文档][stack-size]。
    ///
    /// # Examples
    ///
    /// ```
    /// use std::thread;
    ///
    /// let builder = thread::Builder::new().stack_size(32 * 1024);
    /// ```
    ///
    /// [stack-size]: ./index.html#stack-size
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn stack_size(mut self, size: usize) -> Builder {
        self.stack_size = Some(size);
        self
    }

    /// 禁用运行和继承 [spawn hook][spawn hooks]。
    ///
    /// 如果父线程对子线程毫不相关，就使用它。例如，为线程池惰性地派生线程时。
    ///
    /// [spawn hooks]: super::add_spawn_hook
    #[unstable(feature = "thread_spawn_hook", issue = "132951")]
    pub fn no_hooks(mut self) -> Builder {
        self.no_hooks = true;
        self
    }

    /// 通过取得 `Builder` 的所有权来派生一个新线程，并返回一个指向其
    /// [`JoinHandle`] 的 [`io::Result`]。
    ///
    /// 派生出来的线程可能比调用者活得更久（除非调用者线程是主线程；当主线程结束
    /// 时整个进程都会终止）。这个 join 句柄可用于阻塞等待派生线程的终止，包括
    /// 恢复（recover）它的 panic。
    ///
    /// 更完整的文档请参阅 [`thread::spawn`]。
    ///
    /// # Errors
    ///
    /// 与 [`spawn`] 自由函数不同，本方法会返回一个 [`io::Result`]，以捕获在
    /// 操作系统层面创建线程时的任何失败。
    ///
    /// # Panics
    ///
    /// 如果设置了线程名且它包含空字节，则会 panic。
    ///
    /// # Examples
    ///
    /// ```
    /// use std::thread;
    ///
    /// let builder = thread::Builder::new();
    ///
    /// let handler = builder.spawn(|| {
    ///     // 线程代码
    /// }).unwrap();
    ///
    /// handler.join().unwrap();
    /// ```
    ///
    /// [`thread::spawn`]: super::spawn
    /// [`spawn`]: super::spawn
    #[stable(feature = "rust1", since = "1.0.0")]
    #[cfg_attr(miri, track_caller)] // 即便没有 panic，这也有助于 Miri 的回溯（backtrace）
    pub fn spawn<F, T>(self, f: F) -> io::Result<JoinHandle<T>>
    where
        F: FnOnce() -> T,
        F: Send + 'static,
        T: Send + 'static,
    {
        unsafe { self.spawn_unchecked(f) }
    }

    /// 通过取得 `Builder` 的所有权来派生一个不受生命周期限制的新线程，并返回一个
    /// 指向其 [`JoinHandle`] 的 [`io::Result`]。
    ///
    /// 派生出来的线程可能比调用者活得更久（除非调用者线程是主线程；当主线程结束
    /// 时整个进程都会终止）。这个 join 句柄可用于阻塞等待派生线程的终止，包括
    /// 恢复（recover）它的 panic。
    ///
    /// 本方法与 [`thread::Builder::spawn`][`Builder::spawn`] 相同，区别仅在于
    /// 放宽了生命周期约束，这也正是它不安全的原因。更完整的文档请参阅
    /// [`thread::spawn`]。
    ///
    /// # Errors
    ///
    /// 与 [`spawn`] 自由函数不同，本方法会返回一个 [`io::Result`]，以捕获在
    /// 操作系统层面创建线程时的任何失败。
    ///
    /// # Panics
    ///
    /// 如果设置了线程名且它包含空字节，则会 panic。
    ///
    /// # Safety
    ///
    /// 调用者必须确保派生出来的线程不会比所提供的线程闭包及其返回类型中的任何
    /// 引用活得更久。这可以通过以下两种方式之一来保证：
    ///
    /// - 确保在任何被引用的数据被丢弃之前调用 [`join`][`JoinHandle::join`]
    /// - 只使用具有 `'static` 生命周期约束的类型，即没有引用、或只含 `'static`
    /// 引用的类型（[`thread::Builder::spawn`][`Builder::spawn`] 和
    /// [`thread::spawn`] 都会在静态层面强制这一属性）
    ///
    /// # Examples
    ///
    /// ```
    /// use std::thread;
    ///
    /// let builder = thread::Builder::new();
    ///
    /// let x = 1;
    /// let thread_x = &x;
    ///
    /// let handler = unsafe {
    ///     builder.spawn_unchecked(move || {
    ///         println!("x = {}", *thread_x);
    ///     }).unwrap()
    /// };
    ///
    /// // 调用者必须确保 `join()` 被调用，否则如果 `x` 在线程闭包执行之前
    /// // 就被丢弃，就可能访问到已释放的内存！
    /// handler.join().unwrap();
    /// ```
    ///
    /// [`thread::spawn`]: super::spawn
    /// [`spawn`]: super::spawn
    #[stable(feature = "thread_spawn_unchecked", since = "1.82.0")]
    #[cfg_attr(miri, track_caller)] // 即便没有 panic，这也有助于 Miri 的回溯（backtrace）
    pub unsafe fn spawn_unchecked<F, T>(self, f: F) -> io::Result<JoinHandle<T>>
    where
        F: FnOnce() -> T,
        F: Send,
        T: Send,
    {
        let Builder { name, stack_size, no_hooks } = self;
        Ok(JoinHandle(unsafe { spawn_unchecked(name, stack_size, no_hooks, None, f) }?))
    }
}
