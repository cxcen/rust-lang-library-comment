use super::Result;
use super::builder::Builder;
use super::current::current_or_unnamed;
use super::lifecycle::{JoinInner, spawn_unchecked};
use super::thread::Thread;
use crate::marker::PhantomData;
use crate::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use crate::sync::Arc;
use crate::sync::atomic::{Atomic, AtomicBool, AtomicUsize, Ordering};
use crate::{fmt, io};

/// 一个可在其中派生作用域线程（scoped thread）的作用域。
///
/// 详见 [`scope`]。
#[stable(feature = "scoped_threads", since = "1.63.0")]
pub struct Scope<'scope, 'env: 'scope> {
    data: Arc<ScopeData>,
    /// 对 'scope 保持不变型（invariance），以确保 'scope 不能收缩，这对于
    /// 健全性（soundness）是必需的。
    ///
    /// 如果没有不变型，下面的代码会编译通过，但却是不健全的：
    ///
    /// ```compile_fail,E0373
    /// std::thread::scope(|s| {
    ///     s.spawn(|| {
    ///         let a = String::from("abcd");
    ///         s.spawn(|| println!("{a:?}")); // 可能在 `a` 被丢弃之后才运行
    ///     });
    /// });
    /// ```
    scope: PhantomData<&'scope mut &'scope ()>,
    env: PhantomData<&'env mut &'env ()>,
}

/// 一项可用于 join 某个作用域线程（阻塞等待其终止）的、被拥有的许可。
///
/// 详见 [`Scope::spawn`]。
#[stable(feature = "scoped_threads", since = "1.63.0")]
pub struct ScopedJoinHandle<'scope, T>(JoinInner<'scope, T>);

pub(super) struct ScopeData {
    num_running_threads: Atomic<usize>,
    a_thread_panicked: Atomic<bool>,
    main_thread: Thread,
}

impl ScopeData {
    pub(super) fn increment_num_running_threads(&self) {
        // 我们用 usize::MAX / 2 来检查“溢出”，以确保它绝无可能溢出回 0，
        // 因为那会导致不健全（unsoundness）。
        if self.num_running_threads.fetch_add(1, Ordering::Relaxed) > usize::MAX / 2 {
            // 这只可能在 mem::forget() 了大量 ScopedJoinHandle 时合理地发生。
            self.overflow();
        }
    }

    #[cold]
    fn overflow(&self) {
        self.decrement_num_running_threads(false);
        panic!("too many running threads in thread scope");
    }

    pub(super) fn decrement_num_running_threads(&self, panic: bool) {
        if panic {
            self.a_thread_panicked.store(true, Ordering::Relaxed);
        }
        if self.num_running_threads.fetch_sub(1, Ordering::Release) == 1 {
            self.main_thread.unpark();
        }
    }
}

/// 创建一个用于派生作用域线程的作用域。
///
/// 传给 `scope` 的函数会得到一个 [`Scope`] 对象，作用域线程可以通过它来
/// [派生][`Scope::spawn`]。
///
/// 与非作用域线程不同，作用域线程可以借用非 `'static` 的数据，因为该作用域保证
/// 所有线程都会在作用域结束时被 join。
///
/// 在作用域内派生的、且未被手动 join 的所有线程，都会在本函数返回之前被自动
/// join。
///
/// # Panics
///
/// 如果任何被自动 join 的线程发生了 panic，本函数将会 panic。
///
/// 如果你想处理来自派生线程的 panic，请在作用域结束之前
/// [join][ScopedJoinHandle::join] 它们。
///
/// # Example
///
/// ```
/// use std::thread;
///
/// let mut a = vec![1, 2, 3];
/// let mut x = 0;
///
/// thread::scope(|s| {
///     s.spawn(|| {
///         println!("hello from the first scoped thread");
///         // 我们可以在这里借用 `a`。
///         dbg!(&a);
///     });
///     s.spawn(|| {
///         println!("hello from the second scoped thread");
///         // 我们甚至可以在这里可变地借用 `x`，
///         // 因为没有其他线程在使用它。
///         x += a[0] + a[2];
///     });
///     println!("hello from the main thread");
/// });
///
/// // 作用域结束之后，我们可以再次修改和访问我们的变量：
/// a.push(4);
/// assert_eq!(x, a.len());
/// ```
///
/// # Lifetimes
///
/// 作用域线程涉及两个生命周期：`'scope` 和 `'env`。
///
/// `'scope` 生命周期代表作用域本身的生命周期。也就是说：可以派生新的作用域线程
/// 的那段时间，同时也是这些线程可能仍在运行的那段时间。一旦这个生命周期结束，
/// 所有作用域线程都会被 join。这个生命周期在 `scope` 函数内、在 `f`（`scope`
/// 的参数）开始之前就开始。它在 `f` 返回、且所有作用域线程都已被 join 之后、但
/// 在 `scope` 返回之前结束。
///
/// `'env` 生命周期代表被作用域线程所借用的那些东西的生命周期。这个生命周期必须
/// 比对 `scope` 的调用活得更久，因此它不能小于 `'scope`。它可以小到只是对
/// `scope` 的那次调用，这意味着任何比这次调用活得更久的东西（例如就定义在作用域
/// 之前的局部变量）都可以被作用域线程借用。
///
/// `'env: 'scope` 这一约束是 `Scope` 类型定义的一部分。
#[track_caller]
#[stable(feature = "scoped_threads", since = "1.63.0")]
pub fn scope<'env, F, T>(f: F) -> T
where
    F: for<'scope> FnOnce(&'scope Scope<'scope, 'env>) -> T,
{
    // 我们把 `ScopeData` 放进一个 `Arc`，以便其他线程即便在本函数返回之后也能
    // 完成它们的 `decrement_num_running_threads`。
    let scope = Scope {
        data: Arc::new(ScopeData {
            num_running_threads: AtomicUsize::new(0),
            main_thread: current_or_unnamed(),
            a_thread_panicked: AtomicBool::new(false),
        }),
        env: PhantomData,
        scope: PhantomData,
    };

    // 运行 `f`，但捕获 panic，以便确保我们会等待所有线程被 join。
    let result = catch_unwind(AssertUnwindSafe(|| f(&scope)));

    // 等待直到所有线程都已完成。
    while scope.data.num_running_threads.load(Ordering::Acquire) != 0 {
        // SAFETY: 这里是主线程，该句柄属于我们自己。
        unsafe { scope.data.main_thread.park() };
    }

    // 抛出来自 `f` 的任何 panic；如果没有线程发生 panic，则返回 `f` 的返回值。
    match result {
        Err(e) => resume_unwind(e),
        Ok(_) if scope.data.a_thread_panicked.load(Ordering::Relaxed) => {
            panic!("a scoped thread panicked")
        }
        Ok(result) => result,
    }
}

impl<'scope, 'env> Scope<'scope, 'env> {
    /// 在一个作用域内派生一个新线程，并返回它的 [`ScopedJoinHandle`]。
    ///
    /// 与非作用域线程不同，用本函数派生的线程可以从作用域外部借用非 `'static`
    /// 的数据。详见 [`scope`]。
    ///
    /// 这个 join 句柄提供了一个 [`join`] 方法，可用于 join 派生出来的线程。如果
    /// 派生线程发生 panic，[`join`] 会返回一个 [`Err`]，其中包含 panic 载荷。
    ///
    /// 如果这个 join 句柄被丢弃，派生线程会在作用域结束时被隐式 join。在这种
    /// 情况下，如果派生线程发生 panic，那么在所有线程都被 join 之后 [`scope`]
    /// 会 panic。
    ///
    /// 本函数使用 [`Builder`] 的默认参数来创建线程。要指定新线程的栈大小或名字，
    /// 请使用 [`Builder::spawn_scoped`]。
    ///
    /// # Panics
    ///
    /// 如果操作系统创建线程失败，则会 panic；可以使用 [`Builder::spawn_scoped`]
    /// 从此类错误中恢复。
    ///
    /// [`join`]: ScopedJoinHandle::join
    #[stable(feature = "scoped_threads", since = "1.63.0")]
    pub fn spawn<F, T>(&'scope self, f: F) -> ScopedJoinHandle<'scope, T>
    where
        F: FnOnce() -> T + Send + 'scope,
        T: Send + 'scope,
    {
        Builder::new().spawn_scoped(self, f).expect("failed to spawn thread")
    }
}

impl Builder {
    /// 使用通过这个 `Builder` 所做的设置，派生一个新的作用域线程。
    ///
    /// 与 [`Scope::spawn`] 不同，本方法会返回一个 [`io::Result`]，以捕获在
    /// 操作系统层面创建线程时的任何失败。
    ///
    /// # Panics
    ///
    /// 如果设置了线程名且它包含空字节，则会 panic。
    ///
    /// # Example
    ///
    /// ```
    /// use std::thread;
    ///
    /// let mut a = vec![1, 2, 3];
    /// let mut x = 0;
    ///
    /// thread::scope(|s| {
    ///     thread::Builder::new()
    ///         .name("first".to_string())
    ///         .spawn_scoped(s, ||
    ///     {
    ///         println!("hello from the {:?} scoped thread", thread::current().name());
    ///         // 我们可以在这里借用 `a`。
    ///         dbg!(&a);
    ///     })
    ///     .unwrap();
    ///     thread::Builder::new()
    ///         .name("second".to_string())
    ///         .spawn_scoped(s, ||
    ///     {
    ///         println!("hello from the {:?} scoped thread", thread::current().name());
    ///         // 我们甚至可以在这里可变地借用 `x`，
    ///         // 因为没有其他线程在使用它。
    ///         x += a[0] + a[2];
    ///     })
    ///     .unwrap();
    ///     println!("hello from the main thread");
    /// });
    ///
    /// // 作用域结束之后，我们可以再次修改和访问我们的变量：
    /// a.push(4);
    /// assert_eq!(x, a.len());
    /// ```
    #[stable(feature = "scoped_threads", since = "1.63.0")]
    pub fn spawn_scoped<'scope, 'env, F, T>(
        self,
        scope: &'scope Scope<'scope, 'env>,
        f: F,
    ) -> io::Result<ScopedJoinHandle<'scope, T>>
    where
        F: FnOnce() -> T + Send + 'scope,
        T: Send + 'scope,
    {
        let Builder { name, stack_size, no_hooks } = self;
        Ok(ScopedJoinHandle(unsafe {
            spawn_unchecked(name, stack_size, no_hooks, Some(scope.data.clone()), f)
        }?))
    }
}

impl<'scope, T> ScopedJoinHandle<'scope, T> {
    /// 取出指向底层线程的句柄。
    ///
    /// # Examples
    ///
    /// ```
    /// use std::thread;
    ///
    /// thread::scope(|s| {
    ///     let t = s.spawn(|| {
    ///         println!("hello");
    ///     });
    ///     println!("thread id: {:?}", t.thread().id());
    /// });
    /// ```
    #[must_use]
    #[stable(feature = "scoped_threads", since = "1.63.0")]
    pub fn thread(&self) -> &Thread {
        self.0.thread()
    }

    /// 等待关联的线程完成。
    ///
    /// 如果关联的线程已经完成，本函数会立即返回。
    ///
    /// 用[原子内存序][atomic memory orderings]的术语来说，关联线程的完成与本函数
    /// 的返回之间存在同步（synchronizes with）关系。
    /// 换句话说，那个线程所执行的所有操作都
    /// [发生于（happen before）](https://doc.rust-lang.org/nomicon/atomics.html#data-accesses)
    /// `join` 返回之后发生的所有操作之前。
    ///
    /// 如果关联的线程发生 panic，则返回 [`Err`]，其中包含 panic 载荷。
    ///
    /// [atomic memory orderings]: crate::sync::atomic
    ///
    /// # Examples
    ///
    /// ```
    /// use std::thread;
    ///
    /// thread::scope(|s| {
    ///     let t = s.spawn(|| {
    ///         panic!("oh no");
    ///     });
    ///     assert!(t.join().is_err());
    /// });
    /// ```
    #[stable(feature = "scoped_threads", since = "1.63.0")]
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
    #[stable(feature = "scoped_threads", since = "1.63.0")]
    pub fn is_finished(&self) -> bool {
        self.0.is_finished()
    }
}

#[stable(feature = "scoped_threads", since = "1.63.0")]
impl fmt::Debug for Scope<'_, '_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Scope")
            .field("num_running_threads", &self.data.num_running_threads.load(Ordering::Relaxed))
            .field("a_thread_panicked", &self.data.a_thread_panicked.load(Ordering::Relaxed))
            .field("main_thread", &self.data.main_thread)
            .finish_non_exhaustive()
    }
}

#[stable(feature = "scoped_threads", since = "1.63.0")]
impl<'scope, T> fmt::Debug for ScopedJoinHandle<'scope, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ScopedJoinHandle").finish_non_exhaustive()
    }
}
