use crate::cell::UnsafeCell;
use crate::fmt;
use crate::ops::Deref;
use crate::panic::{RefUnwindSafe, UnwindSafe};
use crate::sys::sync as sys;
use crate::thread::{ThreadId, current_id};

/// 一种可重入（re-entrant）的互斥锁
///
/// 这个锁会阻塞 *其他* 等待该锁可用的线程。已经锁定该互斥锁的线程可以多次
/// 对它加锁而不会阻塞，从而避免了一类常见的死锁来源。
///
/// # 示例
///
/// 允许在回调内部递归地调用某个需要同步的函数（[`StdoutLock`](crate::io::StdoutLock)
/// 目前正是这样实现的）：
///
/// ```
/// #![feature(reentrant_lock)]
///
/// use std::cell::RefCell;
/// use std::sync::ReentrantLock;
///
/// pub struct Log {
///     data: RefCell<String>,
/// }
///
/// impl Log {
///     pub fn append(&self, msg: &str) {
///         self.data.borrow_mut().push_str(msg);
///     }
/// }
///
/// static LOG: ReentrantLock<Log> = ReentrantLock::new(Log { data: RefCell::new(String::new()) });
///
/// pub fn with_log<R>(f: impl FnOnce(&Log) -> R) -> R {
///     let log = LOG.lock();
///     f(&*log)
/// }
///
/// with_log(|log| {
///     log.append("Hello");
///     with_log(|log| log.append(" there!"));
/// });
/// ```
///
// # 实现细节
//
// `owner` 字段追踪哪个线程锁定了该互斥锁。
//
// 我们用 thread::current_id() 作为线程标识符，它就是当前线程的 ThreadId，
// 因此在整个进程生命周期内是唯一的。
//
// 如果 `owner` 被设为当前线程的标识符，我们就认为该互斥锁已经被锁住，于是
// 不再重复加锁，而是递增 `lock_count`。
//
// 解锁时，我们递减 `lock_count`，且仅当它归零时才真正解锁互斥锁。
//
// `lock_count` 受该互斥锁保护，且只被锁定了该互斥锁的那个线程访问，因此
// 无需同步。
//
// `owner` 可能被其他想看看自己是否已持有该锁的线程检查，因此需要是原子的。
// 如果比较结果相等，说明我们就在持有该互斥锁的同一线程上，此时内存访问可以
// 使用 relaxed 内存序，因为我们并不涉及多个线程。如果不相等，同步则交由
// 互斥锁负责，这使得 `owner` 字段在所有情形下使用 relaxed 内存序都没问题。
//
// 在不支持 64 位原子操作的系统上，我们还会把一个 TLS 变量的地址连同 64 位
// TID 一起存储。然后我们先把该地址与当前线程上那个变量的地址作比较，仅当
// 二者相等时才比较实际的 TID。由于我们只会在写入 TID 的那个线程上（或与
// 写入线程共享同一 TLS 块的线程上）读取该 TID，因此无需对 TID 访问做进一步
// 同步，它们可以是普通的 64 位非原子访问。
#[unstable(feature = "reentrant_lock", issue = "121440")]
pub struct ReentrantLock<T: ?Sized> {
    mutex: sys::Mutex,
    owner: Tid,
    lock_count: UnsafeCell<u32>,
    data: T,
}

cfg_select!(
    target_has_atomic = "64" => {
        use crate::sync::atomic::{Atomic, AtomicU64, Ordering::Relaxed};

        struct Tid(Atomic<u64>);

        impl Tid {
            const fn new() -> Self {
                Self(AtomicU64::new(0))
            }

            #[inline]
            fn contains(&self, owner: ThreadId) -> bool {
                owner.as_u64().get() == self.0.load(Relaxed)
            }

            #[inline]
            // 这里标为 unsafe 只是为了与下面那个 Tid 类型的 API 保持一致。
            unsafe fn set(&self, tid: Option<ThreadId>) {
                let value = tid.map_or(0, |tid| tid.as_u64().get());
                self.0.store(value, Relaxed);
            }
        }
    }
    _ => {
        /// 返回一个 TLS 变量的地址。保证它在当前所有存活线程之间是唯一的。
        fn tls_addr() -> usize {
            thread_local! { static X: u8 = const { 0u8 } };

            X.with(|p| <*const u8>::addr(p))
        }

        use crate::sync::atomic::{
            Atomic,
            AtomicUsize,
            Ordering,
        };

        struct Tid {
            // 当某线程调用 `set()` 时，这个值会被更新为该线程上一个线程局部
            // 变量的地址。它在 `contains()` 中用作第一道检查；如果 `tls_addr`
            // 与当前线程的 TLS 地址不匹配，那么 ThreadId 也不可能匹配。只有当
            // TLS 地址确实匹配时，我们才读出实际的 TID。
            // 还要注意，我们这里可以使用 relaxed 原子操作，因为我们仅在
            // `tls_addr` 与当前 TLS 地址匹配时才读取 tid。在那种情况下，
            // 要么 tid 是由当前线程设置的，要么是由一个在当前线程的 `tls_addr`
            // 被分配之前就已终止的线程设置的。无论哪种情况都不需要进一步同步
            //（依据 <https://github.com/rust-lang/miri/issues/3450>）
            tls_addr: Atomic<usize>,
            tid: UnsafeCell<u64>,
        }

        unsafe impl Send for Tid {}
        unsafe impl Sync for Tid {}

        impl Tid {
            const fn new() -> Self {
                Self { tls_addr: AtomicUsize::new(0), tid: UnsafeCell::new(0) }
            }

            #[inline]
            // 注意：这里假定 `owner` 是当前线程的 ID；若并非如此，可能虚假地
            // 返回 `false`。
            fn contains(&self, owner: ThreadId) -> bool {
                // 我们必须在执行加载 *之前* 调用 `tls_addr()`，以确保：若我们
                // 复用了某个更早线程的地址，下面的 `tls_addr.load()` 就会
                // happens-after 那个线程所做的一切。
                let tls_addr = tls_addr();
                // SAFETY: 参见该结构体定义处的注释。
                self.tls_addr.load(Ordering::Relaxed) == tls_addr
                    && unsafe { *self.tid.get() } == owner.as_u64().get()
            }

            #[inline]
            // 同一时刻只能由一个线程调用本方法，否则可能引发竞态条件。
            unsafe fn set(&self, tid: Option<ThreadId>) {
                // 关键在于：当 tid 被清除时，我们要把 `self.tls_addr` 设为 0。
                // 否则 `set()` 与 `get()` 之间可能产生竞态条件。
                let tls_addr = if tid.is_some() { tls_addr() } else { 0 };
                let value = tid.map_or(0, |tid| tid.as_u64().get());
                self.tls_addr.store(tls_addr, Ordering::Relaxed);
                unsafe { *self.tid.get() = value };
            }
        }
    }
);

#[unstable(feature = "reentrant_lock", issue = "121440")]
unsafe impl<T: Send + ?Sized> Send for ReentrantLock<T> {}
#[unstable(feature = "reentrant_lock", issue = "121440")]
unsafe impl<T: Send + ?Sized> Sync for ReentrantLock<T> {}

// 由于存在 `UnsafeCell`，这些 trait 不会被自动实现
#[unstable(feature = "reentrant_lock", issue = "121440")]
impl<T: UnwindSafe + ?Sized> UnwindSafe for ReentrantLock<T> {}
#[unstable(feature = "reentrant_lock", issue = "121440")]
impl<T: RefUnwindSafe + ?Sized> RefUnwindSafe for ReentrantLock<T> {}

/// 可重入锁的「作用域锁」（scoped lock）的 RAII 实现。当该结构体被 drop
/// （离开作用域）时，锁会被解锁。
///
/// 受互斥锁保护的数据可通过该守卫的 [`Deref`] 实现来访问。
///
/// 该结构体由 [`ReentrantLock`] 上的 [`lock`](ReentrantLock::lock) 方法创建。
///
/// # 可变性（Mutability）
///
/// 与 [`MutexGuard`](super::MutexGuard) 不同，`ReentrantLockGuard` 并不实现
/// [`DerefMut`](crate::ops::DerefMut)，因为实现该 trait 会违反 Rust 的引用
/// 别名（aliasing）规则。要修改被守卫的数据，请使用内部可变性（interior
/// mutability，通常是 [`RefCell`](crate::cell::RefCell)）。
#[must_use = "if unused the ReentrantLock will immediately unlock"]
#[unstable(feature = "reentrant_lock", issue = "121440")]
pub struct ReentrantLockGuard<'a, T: ?Sized + 'a> {
    lock: &'a ReentrantLock<T>,
}

#[unstable(feature = "reentrant_lock", issue = "121440")]
impl<T: ?Sized> !Send for ReentrantLockGuard<'_, T> {}

#[unstable(feature = "reentrant_lock", issue = "121440")]
unsafe impl<T: ?Sized + Sync> Sync for ReentrantLockGuard<'_, T> {}

#[unstable(feature = "reentrant_lock", issue = "121440")]
impl<T> ReentrantLock<T> {
    /// 创建一个新的可重入锁，处于未锁定状态，可随时使用。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(reentrant_lock)]
    /// use std::sync::ReentrantLock;
    ///
    /// let lock = ReentrantLock::new(0);
    /// ```
    pub const fn new(t: T) -> ReentrantLock<T> {
        ReentrantLock {
            mutex: sys::Mutex::new(),
            owner: Tid::new(),
            lock_count: UnsafeCell::new(0),
            data: t,
        }
    }

    /// 消耗这个锁，返回其底层数据。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(reentrant_lock)]
    ///
    /// use std::sync::ReentrantLock;
    ///
    /// let lock = ReentrantLock::new(0);
    /// assert_eq!(lock.into_inner(), 0);
    /// ```
    pub fn into_inner(self) -> T {
        self.data
    }
}

#[unstable(feature = "reentrant_lock", issue = "121440")]
impl<T: ?Sized> ReentrantLock<T> {
    /// 获取该锁，阻塞当前线程直到能够成功获取为止。
    ///
    /// 本函数会阻塞调用方，直到能够获取该锁。返回时，该线程是唯一持有该锁的
    /// 线程。当调用本方法的线程已经持有该锁时，调用会成功且不阻塞。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(reentrant_lock)]
    /// use std::cell::Cell;
    /// use std::sync::{Arc, ReentrantLock};
    /// use std::thread;
    ///
    /// let lock = Arc::new(ReentrantLock::new(Cell::new(0)));
    /// let c_lock = Arc::clone(&lock);
    ///
    /// thread::spawn(move || {
    ///     c_lock.lock().set(10);
    /// }).join().expect("thread::spawn failed");
    /// assert_eq!(lock.lock().get(), 10);
    /// ```
    pub fn lock(&self) -> ReentrantLockGuard<'_, T> {
        let this_thread = current_id();
        // 安全性：只有在我们拥有内部互斥锁时才会触碰 lock_count。此外，我们
        // 仅在持有内部互斥锁的同时调用 `self.owner.set()`，因此不会有两个
        // 线程并发调用它。
        unsafe {
            if self.owner.contains(this_thread) {
                // 已经是持锁者：这是一次重入，递增计数而不重复加锁。
                self.increment_lock_count().expect("lock count overflow in reentrant mutex");
            } else {
                // 尚未持锁：真正锁定内部互斥锁，登记 owner，并把计数置 1。
                self.mutex.lock();
                self.owner.set(Some(this_thread));
                debug_assert_eq!(*self.lock_count.get(), 0);
                *self.lock_count.get() = 1;
            }
        }
        ReentrantLockGuard { lock: self }
    }

    /// 返回底层数据的可变引用。
    ///
    /// 由于本调用以可变方式借用 `ReentrantLock`，无需进行任何实际的加锁——
    /// 可变借用在静态层面即保证不存在任何锁。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(reentrant_lock)]
    /// use std::sync::ReentrantLock;
    ///
    /// let mut lock = ReentrantLock::new(0);
    /// *lock.get_mut() = 10;
    /// assert_eq!(*lock.lock(), 10);
    /// ```
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.data
    }

    /// 尝试获取该锁。
    ///
    /// 如果此刻无法获取该锁，则返回 `None`。否则返回一个 RAII 守卫。
    ///
    /// 本函数不会阻塞。
    // FIXME 也许把它作为 API 的公开部分？
    #[unstable(issue = "none", feature = "std_internals")]
    #[doc(hidden)]
    pub fn try_lock(&self) -> Option<ReentrantLockGuard<'_, T>> {
        let this_thread = current_id();
        // 安全性：只有在我们拥有内部互斥锁时才会触碰 lock_count。此外，我们
        // 仅在持有内部互斥锁的同时调用 `self.owner.set()`，因此不会有两个
        // 线程并发调用它。
        unsafe {
            if self.owner.contains(this_thread) {
                self.increment_lock_count()?;
                Some(ReentrantLockGuard { lock: self })
            } else if self.mutex.try_lock() {
                self.owner.set(Some(this_thread));
                debug_assert_eq!(*self.lock_count.get(), 0);
                *self.lock_count.get() = 1;
                Some(ReentrantLockGuard { lock: self })
            } else {
                None
            }
        }
    }

    /// 返回底层数据的裸指针（raw pointer）。
    ///
    /// 返回的指针总是非空且对齐良好的，但用户有责任确保：通过它进行的任何
    /// 读取都已正确同步以避免数据竞争，并且在该锁被 drop 之后不再通过它读取。
    #[unstable(feature = "reentrant_lock_data_ptr", issue = "140368")]
    pub const fn data_ptr(&self) -> *const T {
        &raw const self.data
    }

    unsafe fn increment_lock_count(&self) -> Option<()> {
        unsafe {
            // 使用 `checked_add`：若计数溢出则返回 None，以免回绕导致提前解锁。
            *self.lock_count.get() = (*self.lock_count.get()).checked_add(1)?;
        }
        Some(())
    }
}

#[unstable(feature = "reentrant_lock", issue = "121440")]
impl<T: fmt::Debug + ?Sized> fmt::Debug for ReentrantLock<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut d = f.debug_struct("ReentrantLock");
        match self.try_lock() {
            Some(v) => d.field("data", &&*v),
            None => d.field("data", &format_args!("<locked>")),
        };
        d.finish_non_exhaustive()
    }
}

#[unstable(feature = "reentrant_lock", issue = "121440")]
impl<T: Default> Default for ReentrantLock<T> {
    fn default() -> Self {
        Self::new(T::default())
    }
}

#[unstable(feature = "reentrant_lock", issue = "121440")]
impl<T> From<T> for ReentrantLock<T> {
    fn from(t: T) -> Self {
        Self::new(t)
    }
}

#[unstable(feature = "reentrant_lock", issue = "121440")]
impl<T: ?Sized> Deref for ReentrantLockGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.lock.data
    }
}

#[unstable(feature = "reentrant_lock", issue = "121440")]
impl<T: fmt::Debug + ?Sized> fmt::Debug for ReentrantLockGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}

#[unstable(feature = "reentrant_lock", issue = "121440")]
impl<T: fmt::Display + ?Sized> fmt::Display for ReentrantLockGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}

#[unstable(feature = "reentrant_lock", issue = "121440")]
impl<T: ?Sized> Drop for ReentrantLockGuard<'_, T> {
    #[inline]
    fn drop(&mut self) {
        // 安全性：我们持有该锁。
        unsafe {
            // 每个守卫离开作用域时递减一次计数；仅当它归零（即最外层那次
            // 加锁也被释放）时，才清除 owner 并真正解锁内部互斥锁。
            *self.lock.lock_count.get() -= 1;
            if *self.lock.lock_count.get() == 0 {
                self.lock.owner.set(None);
                self.lock.mutex.unlock();
            }
        }
    }
}
