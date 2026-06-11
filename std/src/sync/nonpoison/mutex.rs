use crate::cell::UnsafeCell;
use crate::fmt;
use crate::marker::PhantomData;
use crate::mem::{self, ManuallyDrop};
use crate::ops::{Deref, DerefMut};
use crate::ptr::NonNull;
use crate::sync::nonpoison::{TryLockResult, WouldBlock};
use crate::sys::sync as sys;

/// 一种互斥（mutual exclusion）原语，用于保护共享数据，但 **不** 追踪锁的
/// 中毒（lock poisoning）。
///
/// 关于互斥锁的更多信息，请查阅本锁的中毒变体的文档：[`poison::Mutex`]。
///
/// [`poison::Mutex`]: crate::sync::poison::Mutex
///
/// # 示例
///
/// 注意，这个 `Mutex` **不会** 通过中毒来传播「在持锁期间发生 panic 的
/// 线程」。如果你需要这一功能，请参见 [`poison::Mutex`]。
///
/// ```
/// #![feature(nonpoison_mutex)]
///
/// use std::thread;
/// use std::sync::{Arc, nonpoison::Mutex};
///
/// let mutex = Arc::new(Mutex::new(0u32));
/// let mut handles = Vec::new();
///
/// for n in 0..10 {
///     let m = Arc::clone(&mutex);
///     let handle = thread::spawn(move || {
///         let mut guard = m.lock();
///         *guard += 1;
///         panic!("panic from thread {n} {guard}")
///     });
///     handles.push(handle);
/// }
///
/// for h in handles {
///     let _ = h.join();
/// }
///
/// println!("Finished, locked {} times", mutex.lock());
/// ```
#[unstable(feature = "nonpoison_mutex", issue = "134645")]
#[cfg_attr(not(test), rustc_diagnostic_item = "NonPoisonMutex")]
pub struct Mutex<T: ?Sized> {
    inner: sys::Mutex,
    data: UnsafeCell<T>,
}

/// `T` 必须是 `Send`，[`Mutex`] 才能是 `Send`，因为可以通过 [`into_inner`]
/// 从该 `Mutex` 取出被拥有的 `T`。
///
/// [`into_inner`]: Mutex::into_inner
#[unstable(feature = "nonpoison_mutex", issue = "134645")]
unsafe impl<T: ?Sized + Send> Send for Mutex<T> {}

/// `T` 必须是 `Send`，[`Mutex`] 才能是 `Sync`。
/// 这确保了受保护的数据能从多个线程被安全访问，而不引发数据竞争或其他不安全
/// 行为。
///
/// [`Mutex<T>`] 一次只向一个线程提供对 `T` 的可变访问。然而，`T` 是 `Send`
/// 至关重要，因为以这种方式访问非 `Send` 的结构是不安全的。举例来说，考虑
/// [`Rc`]——一种非原子的引用计数智能指针，它不是 `Send`。借助 `Rc`，我们可以
/// 有多个副本以非原子的引用计数指向同一块堆分配。如果我们使用 `Mutex<Rc<_>>`，
/// 它将只能保护其中一个 `Rc` 实例免于共享访问，而其余副本仍可能遭遇数据竞争。
///
/// 还要注意，`T` 不必是 `Sync`，因为当 `T` 不是 `Sync` 时，`&T` 一次只会被
/// 提供给一个线程。
///
/// [`Rc`]: crate::rc::Rc
#[unstable(feature = "nonpoison_mutex", issue = "134645")]
unsafe impl<T: ?Sized + Send> Sync for Mutex<T> {}

/// 互斥锁的「作用域锁」（scoped lock）的 RAII 实现。当该结构体被 drop
/// （离开作用域）时，锁会被解锁。
///
/// 受互斥锁保护的数据可通过该守卫的 [`Deref`] 与 [`DerefMut`] 实现来访问。
///
/// 该结构体由 [`Mutex`] 上的 [`lock`] 与 [`try_lock`] 方法创建。
///
/// [`lock`]: Mutex::lock
/// [`try_lock`]: Mutex::try_lock
#[must_use = "if unused the Mutex will immediately unlock"]
#[must_not_suspend = "holding a MutexGuard across suspend \
                      points can cause deadlocks, delays, \
                      and cause Futures to not implement `Send`"]
#[unstable(feature = "nonpoison_mutex", issue = "134645")]
#[clippy::has_significant_drop]
#[cfg_attr(not(test), rustc_diagnostic_item = "NonPoisonMutexGuard")]
pub struct MutexGuard<'a, T: ?Sized + 'a> {
    lock: &'a Mutex<T>,
}

/// 为最大化平台可移植性，[`MutexGuard`] 不是 `Send`。
///
/// 在使用 POSIX 线程（通常称为 pthreads）的平台上，要求在获取互斥锁的同一
/// 线程上释放它。出于这个原因，[`MutexGuard`] 不可实现 `Send`，以防止它被
/// 从另一个线程 drop。
#[unstable(feature = "nonpoison_mutex", issue = "134645")]
impl<T: ?Sized> !Send for MutexGuard<'_, T> {}

/// `T` 必须是 `Sync`，[`MutexGuard<T>`] 才能是 `Sync`，因为可以从
/// `&MutexGuard`（经由 `Deref`）得到一个 `&T`。
#[unstable(feature = "nonpoison_mutex", issue = "134645")]
unsafe impl<T: ?Sized + Sync> Sync for MutexGuard<'_, T> {}

/// 由 `MutexGuard::map` 返回的 RAII 互斥守卫，它可以指向受保护数据的某个
/// 子字段。当该结构体被 drop（离开作用域）时，锁会被解锁。
///
/// `MappedMutexGuard` 与 [`MutexGuard`] 的主要区别在于：前者不能与
/// [`Condvar`] 一起使用，因为如果在 `Mutex` 解锁期间，被锁定的对象被另一个
/// 线程修改，那将引入健全性（soundness）问题。
///
/// 受互斥锁保护的数据可通过该守卫的 [`Deref`] 与 [`DerefMut`] 实现来访问。
///
/// 该结构体由 [`MutexGuard`] 上的 [`map`] 与 [`filter_map`] 方法创建。
///
/// [`map`]: MutexGuard::map
/// [`filter_map`]: MutexGuard::filter_map
/// [`Condvar`]: crate::sync::nonpoison::Condvar
#[must_use = "if unused the Mutex will immediately unlock"]
#[must_not_suspend = "holding a MappedMutexGuard across suspend \
                      points can cause deadlocks, delays, \
                      and cause Futures to not implement `Send`"]
#[unstable(feature = "mapped_lock_guards", issue = "117108")]
// #[unstable(feature = "nonpoison_mutex", issue = "134645")]
#[clippy::has_significant_drop]
pub struct MappedMutexGuard<'a, T: ?Sized + 'a> {
    // 注意（NB）：我们用裸指针而非 `&'a mut T`，以避免违反 `noalias`，因为
    // 一个 `MappedMutexGuard` 参数在其整个作用域内并不保持唯一性，只在它被
    // drop 之前保持。`NonNull` 对 `T` 是协变（covariant）的，所以我们在下面
    // 加了一个 `PhantomData<&'a mut T>` 字段以得到对 `T` 正确的型变（不变性
    // invariance）。
    data: NonNull<T>,
    inner: &'a sys::Mutex,
    _variance: PhantomData<&'a mut T>,
}

#[unstable(feature = "mapped_lock_guards", issue = "117108")]
// #[unstable(feature = "nonpoison_mutex", issue = "134645")]
impl<T: ?Sized> !Send for MappedMutexGuard<'_, T> {}
#[unstable(feature = "mapped_lock_guards", issue = "117108")]
// #[unstable(feature = "nonpoison_mutex", issue = "134645")]
unsafe impl<T: ?Sized + Sync> Sync for MappedMutexGuard<'_, T> {}

impl<T> Mutex<T> {
    /// 创建一个新的互斥锁，处于未锁定状态，可随时使用。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(nonpoison_mutex)]
    ///
    /// use std::sync::nonpoison::Mutex;
    ///
    /// let mutex = Mutex::new(0);
    /// ```
    #[unstable(feature = "nonpoison_mutex", issue = "134645")]
    #[inline]
    pub const fn new(t: T) -> Mutex<T> {
        Mutex { inner: sys::Mutex::new(), data: UnsafeCell::new(t) }
    }

    /// 通过克隆（cloning）返回其中所含的值。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(nonpoison_mutex)]
    /// #![feature(lock_value_accessors)]
    ///
    /// use std::sync::nonpoison::Mutex;
    ///
    /// let mut mutex = Mutex::new(7);
    ///
    /// assert_eq!(mutex.get_cloned(), 7);
    /// ```
    #[unstable(feature = "lock_value_accessors", issue = "133407")]
    // #[unstable(feature = "nonpoison_mutex", issue = "134645")]
    pub fn get_cloned(&self) -> T
    where
        T: Clone,
    {
        self.lock().clone()
    }

    /// 设置其中所含的值。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(nonpoison_mutex)]
    /// #![feature(lock_value_accessors)]
    ///
    /// use std::sync::nonpoison::Mutex;
    ///
    /// let mut mutex = Mutex::new(7);
    ///
    /// assert_eq!(mutex.get_cloned(), 7);
    /// mutex.set(11);
    /// assert_eq!(mutex.get_cloned(), 11);
    /// ```
    #[unstable(feature = "lock_value_accessors", issue = "133407")]
    // #[unstable(feature = "nonpoison_mutex", issue = "134645")]
    pub fn set(&self, value: T) {
        if mem::needs_drop::<T>() {
            // 如果所含的值带有非平凡（non-trivial）的析构函数，我们就在锁
            // 已被释放之后再调用该析构函数（避免持锁期间执行可能较慢的 drop）。
            drop(self.replace(value))
        } else {
            *self.lock() = value;
        }
    }

    /// 用 `value` 替换其中所含的值，并返回旧的值。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(nonpoison_mutex)]
    /// #![feature(lock_value_accessors)]
    ///
    /// use std::sync::nonpoison::Mutex;
    ///
    /// let mut mutex = Mutex::new(7);
    ///
    /// assert_eq!(mutex.replace(11), 7);
    /// assert_eq!(mutex.get_cloned(), 11);
    /// ```
    #[unstable(feature = "lock_value_accessors", issue = "133407")]
    // #[unstable(feature = "nonpoison_mutex", issue = "134645")]
    pub fn replace(&self, value: T) -> T {
        let mut guard = self.lock();
        mem::replace(&mut *guard, value)
    }
}

impl<T: ?Sized> Mutex<T> {
    /// 获取互斥锁，阻塞当前线程直到能够成功获取为止。
    ///
    /// 本函数会阻塞本地线程，直到能够获取该互斥锁。返回时，该线程是唯一持有
    /// 该锁的线程。返回一个 RAII 守卫，以允许对锁进行作用域化的解锁。当该守卫
    /// 离开作用域时，互斥锁将被解锁。
    ///
    /// 在已经持有锁的线程上再次锁定该互斥锁，其确切行为未作规定。但本函数在
    /// 第二次调用时不会返回（举例来说，它可能 panic 或死锁）。
    ///
    /// # Panics
    ///
    /// 如果该锁已被当前线程持有，调用本函数时可能 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(nonpoison_mutex)]
    ///
    /// use std::sync::{Arc, nonpoison::Mutex};
    /// use std::thread;
    ///
    /// let mutex = Arc::new(Mutex::new(0));
    /// let c_mutex = Arc::clone(&mutex);
    ///
    /// thread::spawn(move || {
    ///     *c_mutex.lock() = 10;
    /// }).join().expect("thread::spawn failed");
    /// assert_eq!(*mutex.lock(), 10);
    /// ```
    #[unstable(feature = "nonpoison_mutex", issue = "134645")]
    pub fn lock(&self) -> MutexGuard<'_, T> {
        unsafe {
            self.inner.lock();
            MutexGuard::new(self)
        }
    }

    /// 尝试获取该锁。
    ///
    /// 本函数不会阻塞。如果此刻无法获取该锁，则返回 [`WouldBlock`]。否则返回
    /// 一个 RAII 守卫。
    ///
    /// 当该守卫被 drop 时，锁将被解锁。
    ///
    /// # Errors
    ///
    /// 如果因互斥锁已被锁定而无法获取它，则本调用会返回 [`WouldBlock`] 错误。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::sync::{Arc, Mutex};
    /// use std::thread;
    ///
    /// let mutex = Arc::new(Mutex::new(0));
    /// let c_mutex = Arc::clone(&mutex);
    ///
    /// thread::spawn(move || {
    ///     let mut lock = c_mutex.try_lock();
    ///     if let Ok(ref mut mutex) = lock {
    ///         **mutex = 10;
    ///     } else {
    ///         println!("try_lock failed");
    ///     }
    /// }).join().expect("thread::spawn failed");
    /// assert_eq!(*mutex.lock().unwrap(), 10);
    /// ```
    #[unstable(feature = "nonpoison_mutex", issue = "134645")]
    pub fn try_lock(&self) -> TryLockResult<MutexGuard<'_, T>> {
        unsafe { if self.inner.try_lock() { Ok(MutexGuard::new(self)) } else { Err(WouldBlock) } }
    }

    /// 消耗这个互斥锁，返回其底层数据。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(nonpoison_mutex)]
    ///
    /// use std::sync::nonpoison::Mutex;
    ///
    /// let mutex = Mutex::new(0);
    /// assert_eq!(mutex.into_inner(), 0);
    /// ```
    #[unstable(feature = "nonpoison_mutex", issue = "134645")]
    pub fn into_inner(self) -> T
    where
        T: Sized,
    {
        self.data.into_inner()
    }

    /// 返回底层数据的可变引用。
    ///
    /// 由于本调用以可变方式借用 `Mutex`，无需进行任何实际的加锁——可变借用
    /// 在静态层面即保证不存在任何锁。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(nonpoison_mutex)]
    ///
    /// use std::sync::nonpoison::Mutex;
    ///
    /// let mut mutex = Mutex::new(0);
    /// *mutex.get_mut() = 10;
    /// assert_eq!(*mutex.lock(), 10);
    /// ```
    #[unstable(feature = "nonpoison_mutex", issue = "134645")]
    pub fn get_mut(&mut self) -> &mut T {
        self.data.get_mut()
    }

    /// 返回底层数据的裸指针（raw pointer）。
    ///
    /// 返回的指针总是非空且对齐良好的，但用户有责任确保：通过它进行的任何
    /// 读写都已正确同步以避免数据竞争，并且在该互斥锁被 drop 之后不再通过它
    /// 读写。
    #[unstable(feature = "mutex_data_ptr", issue = "140368")]
    // #[unstable(feature = "nonpoison_mutex", issue = "134645")]
    pub const fn data_ptr(&self) -> *mut T {
        self.data.get()
    }

    /// 获取该互斥锁，并通过把一个可变引用传给给定闭包，提供对底层数据的可变
    /// 访问。
    ///
    /// 本方法获取锁，以指向数据的可变引用调用所提供的闭包，并返回该闭包的
    /// 结果。即使闭包 panic，锁也会在闭包完成后被释放。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(lock_value_accessors, nonpoison_mutex)]
    ///
    /// use std::sync::nonpoison::Mutex;
    ///
    /// let mutex = Mutex::new(2);
    ///
    /// let result = mutex.with_mut(|data| {
    ///     *data += 3;
    ///
    ///     *data + 5
    /// });
    ///
    /// assert_eq!(*mutex.lock(), 5);
    /// assert_eq!(result, 10);
    /// ```
    #[unstable(feature = "lock_value_accessors", issue = "133407")]
    // #[unstable(feature = "nonpoison_mutex", issue = "134645")]
    pub fn with_mut<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut T) -> R,
    {
        f(&mut self.lock())
    }
}

#[unstable(feature = "nonpoison_mutex", issue = "134645")]
impl<T> From<T> for Mutex<T> {
    /// 创建一个新的互斥锁，处于未锁定状态，可随时使用。
    /// 这等价于 [`Mutex::new`]。
    fn from(t: T) -> Self {
        Mutex::new(t)
    }
}

#[unstable(feature = "nonpoison_mutex", issue = "134645")]
impl<T: Default> Default for Mutex<T> {
    /// 用 T 的 `Default` 值创建一个 `Mutex<T>`。
    fn default() -> Mutex<T> {
        Mutex::new(Default::default())
    }
}

#[unstable(feature = "nonpoison_mutex", issue = "134645")]
impl<T: ?Sized + fmt::Debug> fmt::Debug for Mutex<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut d = f.debug_struct("Mutex");
        match self.try_lock() {
            Ok(guard) => {
                d.field("data", &&*guard);
            }
            Err(WouldBlock) => {
                d.field("data", &"<locked>");
            }
        }
        d.finish_non_exhaustive()
    }
}

impl<'mutex, T: ?Sized> MutexGuard<'mutex, T> {
    unsafe fn new(lock: &'mutex Mutex<T>) -> MutexGuard<'mutex, T> {
        return MutexGuard { lock };
    }
}

#[unstable(feature = "nonpoison_mutex", issue = "134645")]
impl<T: ?Sized> Deref for MutexGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.lock.data.get() }
    }
}

#[unstable(feature = "nonpoison_mutex", issue = "134645")]
impl<T: ?Sized> DerefMut for MutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.lock.data.get() }
    }
}

#[unstable(feature = "nonpoison_mutex", issue = "134645")]
impl<T: ?Sized> Drop for MutexGuard<'_, T> {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            self.lock.inner.unlock();
        }
    }
}

#[unstable(feature = "nonpoison_mutex", issue = "134645")]
impl<T: ?Sized + fmt::Debug> fmt::Debug for MutexGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

#[unstable(feature = "nonpoison_mutex", issue = "134645")]
impl<T: ?Sized + fmt::Display> fmt::Display for MutexGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}

/// 供 [`nonpoison::condvar`](super::condvar) 使用。
pub(super) fn guard_lock<'a, T: ?Sized>(guard: &MutexGuard<'a, T>) -> &'a sys::Mutex {
    &guard.lock.inner
}

impl<'a, T: ?Sized> MutexGuard<'a, T> {
    /// 为被借用数据的某个组成部分（例如某个枚举变体）制作一个
    /// [`MappedMutexGuard`]。
    ///
    /// 此时 `Mutex` 已被锁定，因此本操作不会失败。
    ///
    /// 这是一个关联函数（associated function），需以 `MutexGuard::map(...)`
    /// 的形式使用。若设计为方法，则会与通过 `Deref` 访问的 `MutexGuard`
    /// 内容上同名的方法相冲突。
    #[unstable(feature = "mapped_lock_guards", issue = "117108")]
    // #[unstable(feature = "nonpoison_mutex", issue = "134645")]
    pub fn map<U, F>(orig: Self, f: F) -> MappedMutexGuard<'a, U>
    where
        F: FnOnce(&mut T) -> &mut U,
        U: ?Sized,
    {
        // SAFETY: 创建原始守卫时，`MutexGuard::new` 的各项条件均已满足，且在
        // 整个 `map` 与/或 `filter_map` 过程中始终得到维持。该闭包的签名保证
        // 它不会「泄漏」传给它的引用的生命周期。如果该闭包 panic，守卫会被 drop。
        let data = NonNull::from(f(unsafe { &mut *orig.lock.data.get() }));
        let orig = ManuallyDrop::new(orig);
        MappedMutexGuard { data, inner: &orig.lock.inner, _variance: PhantomData }
    }

    /// 为被借用数据的某个组成部分制作一个 [`MappedMutexGuard`]。如果该闭包
    /// 返回 `None`，则把原始守卫作为 `Err(...)` 返回。
    ///
    /// 此时 `Mutex` 已被锁定，因此本操作不会失败。
    ///
    /// 这是一个关联函数，需以 `MutexGuard::filter_map(...)` 的形式使用。若设计
    /// 为方法，则会与通过 `Deref` 访问的 `MutexGuard` 内容上同名的方法相冲突。
    #[unstable(feature = "mapped_lock_guards", issue = "117108")]
    // #[unstable(feature = "nonpoison_mutex", issue = "134645")]
    pub fn filter_map<U, F>(orig: Self, f: F) -> Result<MappedMutexGuard<'a, U>, Self>
    where
        F: FnOnce(&mut T) -> Option<&mut U>,
        U: ?Sized,
    {
        // SAFETY: 创建原始守卫时，`MutexGuard::new` 的各项条件均已满足，且在
        // 整个 `map` 与/或 `filter_map` 过程中始终得到维持。该闭包的签名保证
        // 它不会「泄漏」传给它的引用的生命周期。如果该闭包 panic，守卫会被 drop。
        match f(unsafe { &mut *orig.lock.data.get() }) {
            Some(data) => {
                let data = NonNull::from(data);
                let orig = ManuallyDrop::new(orig);
                Ok(MappedMutexGuard { data, inner: &orig.lock.inner, _variance: PhantomData })
            }
            None => Err(orig),
        }
    }
}

#[unstable(feature = "mapped_lock_guards", issue = "117108")]
impl<T: ?Sized> Deref for MappedMutexGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { self.data.as_ref() }
    }
}

#[unstable(feature = "mapped_lock_guards", issue = "117108")]
impl<T: ?Sized> DerefMut for MappedMutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { self.data.as_mut() }
    }
}

#[unstable(feature = "mapped_lock_guards", issue = "117108")]
impl<T: ?Sized> Drop for MappedMutexGuard<'_, T> {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            self.inner.unlock();
        }
    }
}

#[unstable(feature = "mapped_lock_guards", issue = "117108")]
impl<T: ?Sized + fmt::Debug> fmt::Debug for MappedMutexGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

#[unstable(feature = "mapped_lock_guards", issue = "117108")]
impl<T: ?Sized + fmt::Display> fmt::Display for MappedMutexGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}

impl<'a, T: ?Sized> MappedMutexGuard<'a, T> {
    /// 为被借用数据的某个组成部分（例如某个枚举变体）制作一个
    /// [`MappedMutexGuard`]。
    ///
    /// 此时 `Mutex` 已被锁定，因此本操作不会失败。
    ///
    /// 这是一个关联函数，需以 `MappedMutexGuard::map(...)` 的形式使用。若设计
    /// 为方法，则会与通过 `Deref` 访问的 `MutexGuard` 内容上同名的方法相冲突。
    #[unstable(feature = "mapped_lock_guards", issue = "117108")]
    // #[unstable(feature = "nonpoison_mutex", issue = "134645")]
    pub fn map<U, F>(mut orig: Self, f: F) -> MappedMutexGuard<'a, U>
    where
        F: FnOnce(&mut T) -> &mut U,
        U: ?Sized,
    {
        // SAFETY: 创建原始守卫时，`MutexGuard::new` 的各项条件均已满足，且在
        // 整个 `map` 与/或 `filter_map` 过程中始终得到维持。该闭包的签名保证
        // 它不会「泄漏」传给它的引用的生命周期。如果该闭包 panic，守卫会被 drop。
        let data = NonNull::from(f(unsafe { orig.data.as_mut() }));
        let orig = ManuallyDrop::new(orig);
        MappedMutexGuard { data, inner: orig.inner, _variance: PhantomData }
    }

    /// 为被借用数据的某个组成部分制作一个 [`MappedMutexGuard`]。如果该闭包
    /// 返回 `None`，则把原始守卫作为 `Err(...)` 返回。
    ///
    /// 此时 `Mutex` 已被锁定，因此本操作不会失败。
    ///
    /// 这是一个关联函数，需以 `MappedMutexGuard::filter_map(...)` 的形式使用。
    /// 若设计为方法，则会与通过 `Deref` 访问的 `MutexGuard` 内容上同名的方法
    /// 相冲突。
    #[unstable(feature = "mapped_lock_guards", issue = "117108")]
    // #[unstable(feature = "nonpoison_mutex", issue = "134645")]
    pub fn filter_map<U, F>(mut orig: Self, f: F) -> Result<MappedMutexGuard<'a, U>, Self>
    where
        F: FnOnce(&mut T) -> Option<&mut U>,
        U: ?Sized,
    {
        // SAFETY: 创建原始守卫时，`MutexGuard::new` 的各项条件均已满足，且在
        // 整个 `map` 与/或 `filter_map` 过程中始终得到维持。该闭包的签名保证
        // 它不会「泄漏」传给它的引用的生命周期。如果该闭包 panic，守卫会被 drop。
        match f(unsafe { orig.data.as_mut() }) {
            Some(data) => {
                let data = NonNull::from(data);
                let orig = ManuallyDrop::new(orig);
                Ok(MappedMutexGuard { data, inner: orig.inner, _variance: PhantomData })
            }
            None => Err(orig),
        }
    }
}
