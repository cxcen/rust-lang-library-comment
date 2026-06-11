use crate::cell::UnsafeCell;
use crate::fmt;
use crate::marker::PhantomData;
use crate::mem::{self, ManuallyDrop, forget};
use crate::ops::{Deref, DerefMut};
use crate::ptr::NonNull;
use crate::sync::nonpoison::{TryLockResult, WouldBlock};
use crate::sys::sync as sys;

/// 一种读写锁（reader-writer lock），不追踪锁的中毒（lock poisoning）。
///
/// 关于读写锁的更多信息，请查阅本锁的中毒变体的文档（可在 [`poison::RwLock`]
/// 处找到）。
///
/// [`poison::RwLock`]: crate::sync::poison::RwLock
///
/// # 示例
///
/// ```
/// #![feature(nonpoison_rwlock)]
///
/// use std::sync::nonpoison::RwLock;
///
/// let lock = RwLock::new(5);
///
/// // 可以同时持有多个读锁（reader lock）
/// {
///     let r1 = lock.read();
///     let r2 = lock.read();
///     assert_eq!(*r1, 5);
///     assert_eq!(*r2, 5);
/// } // 读锁在此处被 drop
///
/// // 然而，同一时刻只能持有一个写锁（write lock）
/// {
///     let mut w = lock.write();
///     *w += 1;
///     assert_eq!(*w, 6);
/// } // 写锁在此处被 drop
/// ```
#[unstable(feature = "nonpoison_rwlock", issue = "134645")]
#[cfg_attr(not(test), rustc_diagnostic_item = "NonPoisonRwLock")]
pub struct RwLock<T: ?Sized> {
    /// 用于同步线程对受保护数据的访问的内部 [`sys::RwLock`]。
    inner: sys::RwLock,
    /// 受锁保护的数据。
    data: UnsafeCell<T>,
}

#[unstable(feature = "nonpoison_rwlock", issue = "134645")]
unsafe impl<T: ?Sized + Send> Send for RwLock<T> {}

#[unstable(feature = "nonpoison_rwlock", issue = "134645")]
unsafe impl<T: ?Sized + Send + Sync> Sync for RwLock<T> {}

////////////////////////////////////////////////////////////////////////////////////////////////////
// 守卫（Guards）
////////////////////////////////////////////////////////////////////////////////////////////////////

/// 一个 RAII 结构体，在被 drop 时释放锁的共享读访问（shared read access）。
///
/// 该结构体由 [`RwLock`] 上的 [`read`] 与 [`try_read`] 方法创建。
///
/// [`read`]: RwLock::read
/// [`try_read`]: RwLock::try_read
#[must_use = "if unused the RwLock will immediately unlock"]
#[must_not_suspend = "holding a RwLockReadGuard across suspend \
                      points can cause deadlocks, delays, \
                      and cause Futures to not implement `Send`"]
#[unstable(feature = "nonpoison_rwlock", issue = "134645")]
#[clippy::has_significant_drop]
#[cfg_attr(not(test), rustc_diagnostic_item = "NonPoisonRwLockReadGuard")]
pub struct RwLockReadGuard<'rwlock, T: ?Sized + 'rwlock> {
    /// 一个指向受 `RwLock` 保护的数据的指针。注意，这里我们用裸指针而非
    /// `&'rwlock T`，以避免违反 `noalias`，因为一个 `RwLockReadGuard` 实例
    /// 只在它被 drop 之前保持不可变性（immutability），而非在其整个作用域内。
    /// 相比 `*const T`，`NonNull` 更可取，因为它允许 niche 优化。`NonNull`
    /// 同样对 `T` 是协变（covariant）的，正如我们用 `&T` 时那样。
    data: NonNull<T>,
    /// 一个指向我们已加读锁的内部 [`sys::RwLock`] 的引用。
    inner_lock: &'rwlock sys::RwLock,
}

#[unstable(feature = "nonpoison_rwlock", issue = "134645")]
impl<T: ?Sized> !Send for RwLockReadGuard<'_, T> {}

#[unstable(feature = "nonpoison_rwlock", issue = "134645")]
unsafe impl<T: ?Sized + Sync> Sync for RwLockReadGuard<'_, T> {}

/// 一个 RAII 结构体，在被 drop 时释放锁的独占写访问（exclusive write access）。
///
/// 该结构体由 [`RwLock`] 上的 [`write`] 与 [`try_write`] 方法创建。
///
/// [`write`]: RwLock::write
/// [`try_write`]: RwLock::try_write
#[must_use = "if unused the RwLock will immediately unlock"]
#[must_not_suspend = "holding a RwLockWriteGuard across suspend \
                      points can cause deadlocks, delays, \
                      and cause Future's to not implement `Send`"]
#[unstable(feature = "nonpoison_rwlock", issue = "134645")]
#[clippy::has_significant_drop]
#[cfg_attr(not(test), rustc_diagnostic_item = "NonPoisonRwLockWriteGuard")]
pub struct RwLockWriteGuard<'rwlock, T: ?Sized + 'rwlock> {
    /// 一个指向我们已加写锁的 [`RwLock`] 的引用。
    lock: &'rwlock RwLock<T>,
}

#[unstable(feature = "nonpoison_rwlock", issue = "134645")]
impl<T: ?Sized> !Send for RwLockWriteGuard<'_, T> {}

#[unstable(feature = "nonpoison_rwlock", issue = "134645")]
unsafe impl<T: ?Sized + Sync> Sync for RwLockWriteGuard<'_, T> {}

/// 一个 RAII 结构体，在被 drop 时释放锁的共享读访问，它可以指向受保护数据的
/// 某个子字段。
///
/// 该结构体由 [`RwLockReadGuard`] 上的 [`map`] 与 [`filter_map`] 方法创建。
///
/// [`map`]: RwLockReadGuard::map
/// [`filter_map`]: RwLockReadGuard::filter_map
#[must_use = "if unused the RwLock will immediately unlock"]
#[must_not_suspend = "holding a MappedRwLockReadGuard across suspend \
                      points can cause deadlocks, delays, \
                      and cause Futures to not implement `Send`"]
#[unstable(feature = "mapped_lock_guards", issue = "117108")]
// #[unstable(feature = "nonpoison_rwlock", issue = "134645")]
#[clippy::has_significant_drop]
pub struct MappedRwLockReadGuard<'rwlock, T: ?Sized + 'rwlock> {
    /// 一个指向受 `RwLock` 保护的数据的指针。注意，这里我们用裸指针而非
    /// `&'rwlock T`，以避免违反 `noalias`，因为一个 `MappedRwLockReadGuard`
    /// 实例只在它被 drop 之前保持不可变性，而非在其整个作用域内。
    /// 相比 `*const T`，`NonNull` 更可取，因为它允许 niche 优化。`NonNull`
    /// 同样对 `T` 是协变的，正如我们用 `&T` 时那样。
    data: NonNull<T>,
    /// 一个指向我们已加读锁的内部 [`sys::RwLock`] 的引用。
    inner_lock: &'rwlock sys::RwLock,
}

#[unstable(feature = "mapped_lock_guards", issue = "117108")]
// #[unstable(feature = "nonpoison_rwlock", issue = "134645")]
impl<T: ?Sized> !Send for MappedRwLockReadGuard<'_, T> {}

#[unstable(feature = "mapped_lock_guards", issue = "117108")]
// #[unstable(feature = "nonpoison_rwlock", issue = "134645")]
unsafe impl<T: ?Sized + Sync> Sync for MappedRwLockReadGuard<'_, T> {}

/// 一个 RAII 结构体，在被 drop 时释放锁的独占写访问，它可以指向受保护数据的
/// 某个子字段。
///
/// 该结构体由 [`RwLockWriteGuard`] 上的 [`map`] 与 [`filter_map`] 方法创建。
///
/// [`map`]: RwLockWriteGuard::map
/// [`filter_map`]: RwLockWriteGuard::filter_map
#[must_use = "if unused the RwLock will immediately unlock"]
#[must_not_suspend = "holding a MappedRwLockWriteGuard across suspend \
                      points can cause deadlocks, delays, \
                      and cause Future's to not implement `Send`"]
#[unstable(feature = "mapped_lock_guards", issue = "117108")]
// #[unstable(feature = "nonpoison_rwlock", issue = "134645")]
#[clippy::has_significant_drop]
pub struct MappedRwLockWriteGuard<'rwlock, T: ?Sized + 'rwlock> {
    /// 一个指向受 `RwLock` 保护的数据的指针。注意，这里我们用裸指针而非
    /// `&'rwlock T`，以避免违反 `noalias`，因为一个 `MappedRwLockWriteGuard`
    /// 实例只在它被 drop 之前保持唯一性（uniqueness），而非在其整个作用域内。
    /// 相比 `*const T`，`NonNull` 更可取，因为它允许 niche 优化。
    data: NonNull<T>,
    /// `NonNull` 对 `T` 是协变的，所以我们在这里加一个 `PhantomData<&'rwlock mut T>`
    /// 字段，以强制对 `T` 施加正确的不变性（invariance）。
    _variance: PhantomData<&'rwlock mut T>,
    /// 一个指向我们已加写锁的内部 [`sys::RwLock`] 的引用。
    inner_lock: &'rwlock sys::RwLock,
}

#[unstable(feature = "mapped_lock_guards", issue = "117108")]
// #[unstable(feature = "nonpoison_rwlock", issue = "134645")]
impl<T: ?Sized> !Send for MappedRwLockWriteGuard<'_, T> {}

#[unstable(feature = "mapped_lock_guards", issue = "117108")]
// #[unstable(feature = "nonpoison_rwlock", issue = "134645")]
unsafe impl<T: ?Sized + Sync> Sync for MappedRwLockWriteGuard<'_, T> {}

////////////////////////////////////////////////////////////////////////////////////////////////////
// 各项实现（Implementations）
////////////////////////////////////////////////////////////////////////////////////////////////////

impl<T> RwLock<T> {
    /// 创建一个新的、处于未锁定状态的 `RwLock<T>` 实例。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(nonpoison_rwlock)]
    ///
    /// use std::sync::nonpoison::RwLock;
    ///
    /// let lock = RwLock::new(5);
    /// ```
    #[unstable(feature = "nonpoison_rwlock", issue = "134645")]
    #[inline]
    pub const fn new(t: T) -> RwLock<T> {
        RwLock { inner: sys::RwLock::new(), data: UnsafeCell::new(t) }
    }

    /// 通过克隆返回其中所含的值。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(nonpoison_rwlock)]
    /// #![feature(lock_value_accessors)]
    ///
    /// use std::sync::nonpoison::RwLock;
    ///
    /// let mut lock = RwLock::new(7);
    ///
    /// assert_eq!(lock.get_cloned(), 7);
    /// ```
    #[unstable(feature = "lock_value_accessors", issue = "133407")]
    // #[unstable(feature = "nonpoison_rwlock", issue = "134645")]
    pub fn get_cloned(&self) -> T
    where
        T: Clone,
    {
        self.read().clone()
    }

    /// 设置其中所含的值。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(nonpoison_rwlock)]
    /// #![feature(lock_value_accessors)]
    ///
    /// use std::sync::nonpoison::RwLock;
    ///
    /// let mut lock = RwLock::new(7);
    ///
    /// assert_eq!(lock.get_cloned(), 7);
    /// lock.set(11);
    /// assert_eq!(lock.get_cloned(), 11);
    /// ```
    #[unstable(feature = "lock_value_accessors", issue = "133407")]
    // #[unstable(feature = "nonpoison_rwlock", issue = "134645")]
    pub fn set(&self, value: T) {
        if mem::needs_drop::<T>() {
            // 如果所含的值带有非平凡的析构函数，我们就在锁已被释放之后再调用
            // 该析构函数（避免持锁期间执行可能较慢的 drop）。
            drop(self.replace(value))
        } else {
            *self.write() = value;
        }
    }

    /// 用 `value` 替换其中所含的值，并返回旧的值。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(nonpoison_rwlock)]
    /// #![feature(lock_value_accessors)]
    ///
    /// use std::sync::nonpoison::RwLock;
    ///
    /// let mut lock = RwLock::new(7);
    ///
    /// assert_eq!(lock.replace(11), 7);
    /// assert_eq!(lock.get_cloned(), 11);
    /// ```
    #[unstable(feature = "lock_value_accessors", issue = "133407")]
    // #[unstable(feature = "nonpoison_rwlock", issue = "134645")]
    pub fn replace(&self, value: T) -> T {
        let mut guard = self.write();
        mem::replace(&mut *guard, value)
    }
}

impl<T: ?Sized> RwLock<T> {
    /// 以共享读访问（shared read access）锁定这个 `RwLock`，阻塞当前线程直到
    /// 能够获取为止。
    ///
    /// 调用线程将被阻塞，直到不再有写者（writer）持有该锁。本方法返回时，可能
    /// 仍有其他读者（reader）正处于锁内。对于「相互争用的读者与写者谁会先获取
    /// 到锁」这一点，本方法不提供任何顺序保证。
    ///
    /// 返回一个 RAII 守卫，一经 drop 便会释放本线程的共享访问。
    ///
    /// # Panics
    ///
    /// 如果该锁已被当前线程持有，调用本函数时可能 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(nonpoison_rwlock)]
    ///
    /// use std::sync::Arc;
    /// use std::sync::nonpoison::RwLock;
    /// use std::thread;
    ///
    /// let lock = Arc::new(RwLock::new(1));
    /// let c_lock = Arc::clone(&lock);
    ///
    /// let n = lock.read();
    /// assert_eq!(*n, 1);
    ///
    /// thread::spawn(move || {
    ///     let r = c_lock.read();
    /// }).join().unwrap();
    /// ```
    #[inline]
    #[unstable(feature = "nonpoison_rwlock", issue = "134645")]
    pub fn read(&self) -> RwLockReadGuard<'_, T> {
        unsafe {
            self.inner.read();
            RwLockReadGuard::new(self)
        }
    }

    /// 尝试以共享读访问获取这个 `RwLock`。
    ///
    /// 如果此刻无法授予该访问，则返回 `Err`。否则返回一个 RAII 守卫，它一经
    /// drop 便会释放该共享访问。
    ///
    /// 本函数不会阻塞。
    ///
    /// 对于「相互争用的读者与写者谁会先获取到锁」这一点，本函数不提供任何
    /// 顺序保证。
    ///
    /// # Errors
    ///
    /// 如果因这个 `RwLock` 已被以独占方式锁定而无法获取它，本函数将返回
    /// [`WouldBlock`] 错误。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(nonpoison_rwlock)]
    ///
    /// use std::sync::nonpoison::RwLock;
    ///
    /// let lock = RwLock::new(1);
    ///
    /// match lock.try_read() {
    ///     Ok(n) => assert_eq!(*n, 1),
    ///     Err(_) => unreachable!(),
    /// };
    /// ```
    #[inline]
    #[unstable(feature = "nonpoison_rwlock", issue = "134645")]
    pub fn try_read(&self) -> TryLockResult<RwLockReadGuard<'_, T>> {
        unsafe {
            if self.inner.try_read() { Ok(RwLockReadGuard::new(self)) } else { Err(WouldBlock) }
        }
    }

    /// 以独占写访问（exclusive write access）锁定这个 `RwLock`，阻塞当前线程
    /// 直到能够获取为止。
    ///
    /// 当其他写者或其他读者当前正访问该锁时，本函数不会返回。
    ///
    /// 返回一个 RAII 守卫，一经 drop 便会释放这个 `RwLock` 的写访问。
    ///
    /// # Panics
    ///
    /// 如果该锁已被当前线程持有，调用本函数时可能 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(nonpoison_rwlock)]
    ///
    /// use std::sync::nonpoison::RwLock;
    ///
    /// let lock = RwLock::new(1);
    ///
    /// let mut n = lock.write();
    /// *n = 2;
    ///
    /// assert!(lock.try_read().is_err());
    /// ```
    #[inline]
    #[unstable(feature = "nonpoison_rwlock", issue = "134645")]
    pub fn write(&self) -> RwLockWriteGuard<'_, T> {
        unsafe {
            self.inner.write();
            RwLockWriteGuard::new(self)
        }
    }

    /// 尝试以独占写访问锁定这个 `RwLock`。
    ///
    /// 如果此刻无法获取该锁，则返回 `Err`。否则返回一个 RAII 守卫，它一经
    /// drop 便会释放该锁。
    ///
    /// 本函数不会阻塞。
    ///
    /// 对于「相互争用的读者与写者谁会先获取到锁」这一点，本函数不提供任何
    /// 顺序保证。
    ///
    /// # Errors
    ///
    /// 如果因这个 `RwLock` 已被锁定而无法获取它，本函数将返回 [`WouldBlock`]
    /// 错误。
    ///
    /// [`WouldBlock`]: WouldBlock
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(nonpoison_rwlock)]
    ///
    /// use std::sync::nonpoison::RwLock;
    ///
    /// let lock = RwLock::new(1);
    ///
    /// let n = lock.read();
    /// assert_eq!(*n, 1);
    ///
    /// assert!(lock.try_write().is_err());
    /// ```
    #[inline]
    #[unstable(feature = "nonpoison_rwlock", issue = "134645")]
    pub fn try_write(&self) -> TryLockResult<RwLockWriteGuard<'_, T>> {
        unsafe {
            if self.inner.try_write() { Ok(RwLockWriteGuard::new(self)) } else { Err(WouldBlock) }
        }
    }

    /// 消耗这个 `RwLock`，返回其底层数据。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(nonpoison_rwlock)]
    ///
    /// use std::sync::nonpoison::RwLock;
    ///
    /// let lock = RwLock::new(String::new());
    /// {
    ///     let mut s = lock.write();
    ///     *s = "modified".to_owned();
    /// }
    /// assert_eq!(lock.into_inner(), "modified");
    /// ```
    #[unstable(feature = "nonpoison_rwlock", issue = "134645")]
    pub fn into_inner(self) -> T
    where
        T: Sized,
    {
        self.data.into_inner()
    }

    /// 返回底层数据的可变引用。
    ///
    /// 由于本调用以可变方式借用 `RwLock`，无需进行任何实际的加锁——可变借用
    /// 在静态层面即保证：当这个引用存在期间，不可能获取任何新的锁。注意，本
    /// 方法不会清除任何先前被遗弃（abandoned）的锁（例如通过对
    /// [`RwLockReadGuard`] 或 [`RwLockWriteGuard`] 调用 [`forget()`]）。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(nonpoison_rwlock)]
    ///
    /// use std::sync::nonpoison::RwLock;
    ///
    /// let mut lock = RwLock::new(0);
    /// *lock.get_mut() = 10;
    /// assert_eq!(*lock.read(), 10);
    /// ```
    #[unstable(feature = "nonpoison_rwlock", issue = "134645")]
    pub fn get_mut(&mut self) -> &mut T {
        self.data.get_mut()
    }

    /// 返回底层数据的裸指针（raw pointer）。
    ///
    /// 返回的指针总是非空且对齐良好的，但用户有责任确保：通过它进行的任何
    /// 读写都已正确同步以避免数据竞争，并且在该锁被 drop 之后不再通过它读写。
    #[unstable(feature = "rwlock_data_ptr", issue = "140368")]
    // #[unstable(feature = "nonpoison_rwlock", issue = "134645")]
    pub const fn data_ptr(&self) -> *mut T {
        self.data.get()
    }

    /// 以对底层数据的共享读访问锁定这个 `RwLock`，并把一个引用传给给定闭包。
    ///
    /// 本方法获取锁，以指向数据的引用调用所提供的闭包，并返回该闭包的结果。
    /// 即使闭包 panic，锁也会在闭包完成后被释放。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(lock_value_accessors, nonpoison_rwlock)]
    ///
    /// use std::sync::nonpoison::RwLock;
    ///
    /// let rwlock = RwLock::new(2);
    /// let result = rwlock.with(|data| *data + 3);
    ///
    /// assert_eq!(result, 5);
    /// ```
    #[unstable(feature = "lock_value_accessors", issue = "133407")]
    // #[unstable(feature = "nonpoison_rwlock", issue = "134645")]
    pub fn with<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&T) -> R,
    {
        f(&self.read())
    }

    /// 以对底层数据的独占写访问锁定这个 `RwLock`，并把一个可变引用传给给定
    /// 闭包。
    ///
    /// 本方法获取锁，以指向数据的可变引用调用所提供的闭包，并返回该闭包的
    /// 结果。即使闭包 panic，锁也会在闭包完成后被释放。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(lock_value_accessors, nonpoison_rwlock)]
    ///
    /// use std::sync::nonpoison::RwLock;
    ///
    /// let rwlock = RwLock::new(2);
    ///
    /// let result = rwlock.with_mut(|data| {
    ///     *data += 3;
    ///
    ///     *data + 5
    /// });
    ///
    /// assert_eq!(*rwlock.read(), 5);
    /// assert_eq!(result, 10);
    /// ```
    #[unstable(feature = "lock_value_accessors", issue = "133407")]
    // #[unstable(feature = "nonpoison_rwlock", issue = "134645")]
    pub fn with_mut<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut T) -> R,
    {
        f(&mut self.write())
    }
}

#[unstable(feature = "nonpoison_rwlock", issue = "134645")]
impl<T: ?Sized + fmt::Debug> fmt::Debug for RwLock<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut d = f.debug_struct("RwLock");
        match self.try_read() {
            Ok(guard) => {
                d.field("data", &&*guard);
            }
            Err(WouldBlock) => {
                d.field("data", &format_args!("<locked>"));
            }
        }
        d.finish_non_exhaustive()
    }
}

#[unstable(feature = "nonpoison_rwlock", issue = "134645")]
impl<T: Default> Default for RwLock<T> {
    /// 用 T 的 `Default` 值创建一个新的 `RwLock<T>`。
    fn default() -> RwLock<T> {
        RwLock::new(Default::default())
    }
}

#[unstable(feature = "nonpoison_rwlock", issue = "134645")]
impl<T> From<T> for RwLock<T> {
    /// 创建一个新的、处于未锁定状态的 `RwLock<T>` 实例。
    /// 这等价于 [`RwLock::new`]。
    fn from(t: T) -> Self {
        RwLock::new(t)
    }
}

impl<'rwlock, T: ?Sized> RwLockReadGuard<'rwlock, T> {
    /// 从一个 `RwLock<T>` 创建一个新的 `RwLockReadGuard<T>` 实例。
    ///
    /// # Safety
    ///
    /// 当且仅当同一线程在实例化本对象之前，已成功且安全地调用过
    /// `lock.inner.read()`、`lock.inner.try_read()` 或 `lock.inner.downgrade()`
    /// 时，本函数才是安全的。
    unsafe fn new(lock: &'rwlock RwLock<T>) -> RwLockReadGuard<'rwlock, T> {
        RwLockReadGuard {
            data: unsafe { NonNull::new_unchecked(lock.data.get()) },
            inner_lock: &lock.inner,
        }
    }

    /// 为被借用数据的某个组成部分（例如某个枚举变体）制作一个
    /// [`MappedRwLockReadGuard`]。
    ///
    /// 此时 `RwLock` 已被加读锁，因此本操作不会失败。
    ///
    /// 这是一个关联函数，需以 `RwLockReadGuard::map(...)` 的形式使用。若设计
    /// 为方法，则会与通过 `Deref` 访问的 `RwLockReadGuard` 内容上同名的方法
    /// 相冲突。
    ///
    /// # Panics
    ///
    /// 如果该闭包 panic，守卫将被 drop（解锁）。
    #[unstable(feature = "mapped_lock_guards", issue = "117108")]
    // #[unstable(feature = "nonpoison_rwlock", issue = "134645")]
    pub fn map<U, F>(orig: Self, f: F) -> MappedRwLockReadGuard<'rwlock, U>
    where
        F: FnOnce(&T) -> &U,
        U: ?Sized,
    {
        // SAFETY: 创建原始守卫时，`RwLockReadGuard::new` 的各项条件均已满足，且在
        // 整个 `map` 与/或 `filter_map` 过程中始终得到维持。该闭包的签名保证
        // 它不会「泄漏」传给它的引用的生命周期。如果该闭包 panic，守卫会被 drop。
        let data = NonNull::from(f(unsafe { orig.data.as_ref() }));
        let orig = ManuallyDrop::new(orig);
        MappedRwLockReadGuard { data, inner_lock: &orig.inner_lock }
    }

    /// 为被借用数据的某个组成部分制作一个 [`MappedRwLockReadGuard`]。如果该
    /// 闭包返回 `None`，则把原始守卫作为 `Err(...)` 返回。
    ///
    /// 此时 `RwLock` 已被加读锁，因此本操作不会失败。
    ///
    /// 这是一个关联函数，需以 `RwLockReadGuard::filter_map(...)` 的形式使用。
    /// 若设计为方法，则会与通过 `Deref` 访问的 `RwLockReadGuard` 内容上同名的
    /// 方法相冲突。
    ///
    /// # Panics
    ///
    /// 如果该闭包 panic，守卫将被 drop（解锁）。
    #[unstable(feature = "mapped_lock_guards", issue = "117108")]
    // #[unstable(feature = "nonpoison_rwlock", issue = "134645")]
    pub fn filter_map<U, F>(orig: Self, f: F) -> Result<MappedRwLockReadGuard<'rwlock, U>, Self>
    where
        F: FnOnce(&T) -> Option<&U>,
        U: ?Sized,
    {
        // SAFETY: 创建原始守卫时，`RwLockReadGuard::new` 的各项条件均已满足，且在
        // 整个 `map` 与/或 `filter_map` 过程中始终得到维持。该闭包的签名保证
        // 它不会「泄漏」传给它的引用的生命周期。如果该闭包 panic，守卫会被 drop。
        match f(unsafe { orig.data.as_ref() }) {
            Some(data) => {
                let data = NonNull::from(data);
                let orig = ManuallyDrop::new(orig);
                Ok(MappedRwLockReadGuard { data, inner_lock: &orig.inner_lock })
            }
            None => Err(orig),
        }
    }
}

impl<'rwlock, T: ?Sized> RwLockWriteGuard<'rwlock, T> {
    /// 从一个 `RwLock<T>` 创建一个新的 `RwLockWriteGuard<T>` 实例。
    ///
    /// # Safety
    ///
    /// 当且仅当同一线程在实例化本对象之前，已成功且安全地调用过
    /// `lock.inner.write()`、`lock.inner.try_write()` 或 `lock.inner.try_upgrade`
    /// 时，本函数才是安全的。
    unsafe fn new(lock: &'rwlock RwLock<T>) -> RwLockWriteGuard<'rwlock, T> {
        RwLockWriteGuard { lock }
    }

    /// 把一个加了写锁的 `RwLockWriteGuard` 降级（downgrade）为一个加了读锁的
    /// [`RwLockReadGuard`]。
    ///
    /// 既然我们持有 `RwLockWriteGuard`，那么 [`RwLock`] 必定已被加了写锁，
    /// 因此本方法不会失败。
    ///
    /// 降级之后，其他读者将被允许读取受保护的数据。
    ///
    /// # 示例
    ///
    /// `downgrade` 取得 `RwLockWriteGuard` 的所有权，并返回一个
    /// [`RwLockReadGuard`]。
    ///
    /// ```
    /// #![feature(nonpoison_rwlock)]
    ///
    /// use std::sync::nonpoison::{RwLock, RwLockWriteGuard};
    ///
    /// let rw = RwLock::new(0);
    ///
    /// let mut write_guard = rw.write();
    /// *write_guard = 42;
    ///
    /// let read_guard = RwLockWriteGuard::downgrade(write_guard);
    /// assert_eq!(42, *read_guard);
    /// ```
    ///
    /// `downgrade` 会 _原子地_ 把 [`RwLock`] 的状态从独占模式（exclusive mode）
    /// 转为共享模式（shared mode）。这意味着：在一个线程调用 `downgrade` 与它
    /// 降级后执行的任何读取之间，另一个写线程不可能插进来。
    ///
    /// ```
    /// #![feature(nonpoison_rwlock)]
    ///
    /// use std::sync::Arc;
    /// use std::sync::nonpoison::{RwLock, RwLockWriteGuard};
    ///
    /// let rw = Arc::new(RwLock::new(1));
    ///
    /// // 把锁置于写模式。
    /// let mut main_write_guard = rw.write();
    ///
    /// let rw_clone = rw.clone();
    /// let evil_handle = std::thread::spawn(move || {
    ///     // 在主线程 drop 掉 `main_read_guard` 之前，这一句不会返回。
    ///     let mut evil_guard = rw_clone.write();
    ///
    ///     assert_eq!(*evil_guard, 2);
    ///     *evil_guard = 3;
    /// });
    ///
    /// *main_write_guard = 2;
    ///
    /// // 原子地把写守卫降级为读守卫。
    /// let main_read_guard = RwLockWriteGuard::downgrade(main_write_guard);
    ///
    /// // 由于 `downgrade` 是原子的，那个写线程不可能改变受保护的数据。
    /// assert_eq!(*main_read_guard, 2, "`downgrade` was not atomic");
    /// #
    /// # drop(main_read_guard);
    /// # evil_handle.join().unwrap();
    /// #
    /// # let final_check = rw.read();
    /// # assert_eq!(*final_check, 3);
    /// ```
    #[unstable(feature = "nonpoison_rwlock", issue = "134645")]
    pub fn downgrade(s: Self) -> RwLockReadGuard<'rwlock, T> {
        let lock = s.lock;

        // 我们不想调用析构函数，因为那会调用 `write_unlock`。
        forget(s);

        // SAFETY: 我们取得了一个写守卫的所有权，所以我们必定已经把 `RwLock`
        // 置于写模式，满足了 `downgrade` 的契约。
        unsafe { lock.inner.downgrade() };

        // SAFETY: 我们刚刚成功调用了 `downgrade`，因此满足了安全性契约。
        unsafe { RwLockReadGuard::new(lock) }
    }

    /// 为被借用数据的某个组成部分（例如某个枚举变体）制作一个
    /// [`MappedRwLockWriteGuard`]。
    ///
    /// 此时 `RwLock` 已被加写锁，因此本操作不会失败。
    ///
    /// 这是一个关联函数，需以 `RwLockWriteGuard::map(...)` 的形式使用。若设计
    /// 为方法，则会与通过 `Deref` 访问的 `RwLockWriteGuard` 内容上同名的方法
    /// 相冲突。
    ///
    /// # Panics
    ///
    /// 如果该闭包 panic，守卫将被 drop（解锁）。
    #[unstable(feature = "mapped_lock_guards", issue = "117108")]
    // #[unstable(feature = "nonpoison_rwlock", issue = "134645")]
    pub fn map<U, F>(orig: Self, f: F) -> MappedRwLockWriteGuard<'rwlock, U>
    where
        F: FnOnce(&mut T) -> &mut U,
        U: ?Sized,
    {
        // SAFETY: 创建原始守卫时，`RwLockWriteGuard::new` 的各项条件均已满足，且在
        // 整个 `map` 与/或 `filter_map` 过程中始终得到维持。该闭包的签名保证
        // 它不会「泄漏」传给它的引用的生命周期。如果该闭包 panic，守卫会被 drop。
        let data = NonNull::from(f(unsafe { &mut *orig.lock.data.get() }));
        let orig = ManuallyDrop::new(orig);
        MappedRwLockWriteGuard { data, inner_lock: &orig.lock.inner, _variance: PhantomData }
    }

    /// 为被借用数据的某个组成部分制作一个 [`MappedRwLockWriteGuard`]。如果该
    /// 闭包返回 `None`，则把原始守卫作为 `Err(...)` 返回。
    ///
    /// 此时 `RwLock` 已被加写锁，因此本操作不会失败。
    ///
    /// 这是一个关联函数，需以 `RwLockWriteGuard::filter_map(...)` 的形式使用。
    /// 若设计为方法，则会与通过 `Deref` 访问的 `RwLockWriteGuard` 内容上同名的
    /// 方法相冲突。
    ///
    /// # Panics
    ///
    /// 如果该闭包 panic，守卫将被 drop（解锁）。
    #[unstable(feature = "mapped_lock_guards", issue = "117108")]
    // #[unstable(feature = "nonpoison_rwlock", issue = "134645")]
    pub fn filter_map<U, F>(orig: Self, f: F) -> Result<MappedRwLockWriteGuard<'rwlock, U>, Self>
    where
        F: FnOnce(&mut T) -> Option<&mut U>,
        U: ?Sized,
    {
        // SAFETY: 创建原始守卫时，`RwLockWriteGuard::new` 的各项条件均已满足，且在
        // 整个 `map` 与/或 `filter_map` 过程中始终得到维持。该闭包的签名保证
        // 它不会「泄漏」传给它的引用的生命周期。如果该闭包 panic，守卫会被 drop。
        match f(unsafe { &mut *orig.lock.data.get() }) {
            Some(data) => {
                let data = NonNull::from(data);
                let orig = ManuallyDrop::new(orig);
                Ok(MappedRwLockWriteGuard {
                    data,
                    inner_lock: &orig.lock.inner,
                    _variance: PhantomData,
                })
            }
            None => Err(orig),
        }
    }
}

impl<'rwlock, T: ?Sized> MappedRwLockReadGuard<'rwlock, T> {
    /// 为被借用数据的某个组成部分（例如某个枚举变体）制作一个
    /// [`MappedRwLockReadGuard`]。
    ///
    /// 此时 `RwLock` 已被加读锁，因此本操作不会失败。
    ///
    /// 这是一个关联函数，需以 `MappedRwLockReadGuard::map(...)` 的形式使用。
    /// 若设计为方法，则会与通过 `Deref` 访问的 `MappedRwLockReadGuard` 内容上
    /// 同名的方法相冲突。
    ///
    /// # Panics
    ///
    /// 如果该闭包 panic，守卫将被 drop（解锁）。
    #[unstable(feature = "mapped_lock_guards", issue = "117108")]
    // #[unstable(feature = "nonpoison_rwlock", issue = "134645")]
    pub fn map<U, F>(orig: Self, f: F) -> MappedRwLockReadGuard<'rwlock, U>
    where
        F: FnOnce(&T) -> &U,
        U: ?Sized,
    {
        // SAFETY: 创建原始守卫时，`RwLockReadGuard::new` 的各项条件均已满足，且在
        // 整个 `map` 与/或 `filter_map` 过程中始终得到维持。该闭包的签名保证
        // 它不会「泄漏」传给它的引用的生命周期。如果该闭包 panic，守卫会被 drop。
        let data = NonNull::from(f(unsafe { orig.data.as_ref() }));
        let orig = ManuallyDrop::new(orig);
        MappedRwLockReadGuard { data, inner_lock: &orig.inner_lock }
    }

    /// 为被借用数据的某个组成部分制作一个 [`MappedRwLockReadGuard`]。如果该
    /// 闭包返回 `None`，则把原始守卫作为 `Err(...)` 返回。
    ///
    /// 此时 `RwLock` 已被加读锁，因此本操作不会失败。
    ///
    /// 这是一个关联函数，需以 `MappedRwLockReadGuard::filter_map(...)` 的形式
    /// 使用。若设计为方法，则会与通过 `Deref` 访问的 `MappedRwLockReadGuard`
    /// 内容上同名的方法相冲突。
    ///
    /// # Panics
    ///
    /// 如果该闭包 panic，守卫将被 drop（解锁）。
    #[unstable(feature = "mapped_lock_guards", issue = "117108")]
    // #[unstable(feature = "nonpoison_rwlock", issue = "134645")]
    pub fn filter_map<U, F>(orig: Self, f: F) -> Result<MappedRwLockReadGuard<'rwlock, U>, Self>
    where
        F: FnOnce(&T) -> Option<&U>,
        U: ?Sized,
    {
        // SAFETY: 创建原始守卫时，`RwLockReadGuard::new` 的各项条件均已满足，且在
        // 整个 `map` 与/或 `filter_map` 过程中始终得到维持。该闭包的签名保证
        // 它不会「泄漏」传给它的引用的生命周期。如果该闭包 panic，守卫会被 drop。
        match f(unsafe { orig.data.as_ref() }) {
            Some(data) => {
                let data = NonNull::from(data);
                let orig = ManuallyDrop::new(orig);
                Ok(MappedRwLockReadGuard { data, inner_lock: &orig.inner_lock })
            }
            None => Err(orig),
        }
    }
}

impl<'rwlock, T: ?Sized> MappedRwLockWriteGuard<'rwlock, T> {
    /// 为被借用数据的某个组成部分（例如某个枚举变体）制作一个
    /// [`MappedRwLockWriteGuard`]。
    ///
    /// 此时 `RwLock` 已被加写锁，因此本操作不会失败。
    ///
    /// 这是一个关联函数，需以 `MappedRwLockWriteGuard::map(...)` 的形式使用。
    /// 若设计为方法，则会与通过 `Deref` 访问的 `MappedRwLockWriteGuard` 内容上
    /// 同名的方法相冲突。
    ///
    /// # Panics
    ///
    /// 如果该闭包 panic，守卫将被 drop（解锁）。
    #[unstable(feature = "mapped_lock_guards", issue = "117108")]
    // #[unstable(feature = "nonpoison_rwlock", issue = "134645")]
    pub fn map<U, F>(mut orig: Self, f: F) -> MappedRwLockWriteGuard<'rwlock, U>
    where
        F: FnOnce(&mut T) -> &mut U,
        U: ?Sized,
    {
        // SAFETY: 创建原始守卫时，`RwLockWriteGuard::new` 的各项条件均已满足，且在
        // 整个 `map` 与/或 `filter_map` 过程中始终得到维持。该闭包的签名保证
        // 它不会「泄漏」传给它的引用的生命周期。如果该闭包 panic，守卫会被 drop。
        let data = NonNull::from(f(unsafe { orig.data.as_mut() }));
        let orig = ManuallyDrop::new(orig);
        MappedRwLockWriteGuard { data, inner_lock: orig.inner_lock, _variance: PhantomData }
    }

    /// 为被借用数据的某个组成部分制作一个 [`MappedRwLockWriteGuard`]。如果该
    /// 闭包返回 `None`，则把原始守卫作为 `Err(...)` 返回。
    ///
    /// 此时 `RwLock` 已被加写锁，因此本操作不会失败。
    ///
    /// 这是一个关联函数，需以 `MappedRwLockWriteGuard::filter_map(...)` 的形式
    /// 使用。若设计为方法，则会与通过 `Deref` 访问的 `MappedRwLockWriteGuard`
    /// 内容上同名的方法相冲突。
    ///
    /// # Panics
    ///
    /// 如果该闭包 panic，守卫将被 drop（解锁）。
    #[unstable(feature = "mapped_lock_guards", issue = "117108")]
    // #[unstable(feature = "nonpoison_rwlock", issue = "134645")]
    pub fn filter_map<U, F>(
        mut orig: Self,
        f: F,
    ) -> Result<MappedRwLockWriteGuard<'rwlock, U>, Self>
    where
        F: FnOnce(&mut T) -> Option<&mut U>,
        U: ?Sized,
    {
        // SAFETY: 创建原始守卫时，`RwLockWriteGuard::new` 的各项条件均已满足，且在
        // 整个 `map` 与/或 `filter_map` 过程中始终得到维持。该闭包的签名保证
        // 它不会「泄漏」传给它的引用的生命周期。如果该闭包 panic，守卫会被 drop。
        match f(unsafe { orig.data.as_mut() }) {
            Some(data) => {
                let data = NonNull::from(data);
                let orig = ManuallyDrop::new(orig);
                Ok(MappedRwLockWriteGuard {
                    data,
                    inner_lock: orig.inner_lock,
                    _variance: PhantomData,
                })
            }
            None => Err(orig),
        }
    }
}

#[unstable(feature = "nonpoison_rwlock", issue = "134645")]
impl<T: ?Sized> Drop for RwLockReadGuard<'_, T> {
    fn drop(&mut self) {
        // SAFETY: 创建该守卫时，`RwLockReadGuard::new` 的各项条件均已满足。
        unsafe {
            self.inner_lock.read_unlock();
        }
    }
}

#[unstable(feature = "nonpoison_rwlock", issue = "134645")]
impl<T: ?Sized> Drop for RwLockWriteGuard<'_, T> {
    fn drop(&mut self) {
        // SAFETY: 创建该守卫时，`RwLockWriteGuard::new` 的各项条件均已满足。
        unsafe {
            self.lock.inner.write_unlock();
        }
    }
}

#[unstable(feature = "mapped_lock_guards", issue = "117108")]
// #[unstable(feature = "nonpoison_rwlock", issue = "134645")]
impl<T: ?Sized> Drop for MappedRwLockReadGuard<'_, T> {
    fn drop(&mut self) {
        // SAFETY: 创建原始守卫时，`RwLockReadGuard::new` 的各项条件均已满足，且在
        // 整个 `map` 与/或 `filter_map` 过程中始终得到维持。
        unsafe {
            self.inner_lock.read_unlock();
        }
    }
}

#[unstable(feature = "mapped_lock_guards", issue = "117108")]
// #[unstable(feature = "nonpoison_rwlock", issue = "134645")]
impl<T: ?Sized> Drop for MappedRwLockWriteGuard<'_, T> {
    fn drop(&mut self) {
        // SAFETY: 创建原始守卫时，`RwLockWriteGuard::new` 的各项条件均已满足，且在
        // 整个 `map` 与/或 `filter_map` 过程中始终得到维持。
        unsafe {
            self.inner_lock.write_unlock();
        }
    }
}

#[unstable(feature = "nonpoison_rwlock", issue = "134645")]
impl<T: ?Sized> Deref for RwLockReadGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        // SAFETY: 创建该守卫时，`RwLockReadGuard::new` 的各项条件均已满足。
        unsafe { self.data.as_ref() }
    }
}

#[unstable(feature = "nonpoison_rwlock", issue = "134645")]
impl<T: ?Sized> Deref for RwLockWriteGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        // SAFETY: 创建该守卫时，`RwLockWriteGuard::new` 的各项条件均已满足。
        unsafe { &*self.lock.data.get() }
    }
}

#[unstable(feature = "nonpoison_rwlock", issue = "134645")]
impl<T: ?Sized> DerefMut for RwLockWriteGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        // SAFETY: 创建该守卫时，`RwLockWriteGuard::new` 的各项条件均已满足。
        unsafe { &mut *self.lock.data.get() }
    }
}

#[unstable(feature = "mapped_lock_guards", issue = "117108")]
// #[unstable(feature = "nonpoison_rwlock", issue = "134645")]
impl<T: ?Sized> Deref for MappedRwLockReadGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        // SAFETY: 创建原始守卫时，`RwLockReadGuard::new` 的各项条件均已满足，且在
        // 整个 `map` 与/或 `filter_map` 过程中始终得到维持。
        unsafe { self.data.as_ref() }
    }
}

#[unstable(feature = "mapped_lock_guards", issue = "117108")]
// #[unstable(feature = "nonpoison_rwlock", issue = "134645")]
impl<T: ?Sized> Deref for MappedRwLockWriteGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        // SAFETY: 创建原始守卫时，`RwLockWriteGuard::new` 的各项条件均已满足，且在
        // 整个 `map` 与/或 `filter_map` 过程中始终得到维持。
        unsafe { self.data.as_ref() }
    }
}

#[unstable(feature = "mapped_lock_guards", issue = "117108")]
// #[unstable(feature = "nonpoison_rwlock", issue = "134645")]
impl<T: ?Sized> DerefMut for MappedRwLockWriteGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        // SAFETY: 创建原始守卫时，`RwLockWriteGuard::new` 的各项条件均已满足，且在
        // 整个 `map` 与/或 `filter_map` 过程中始终得到维持。
        unsafe { self.data.as_mut() }
    }
}

#[unstable(feature = "nonpoison_rwlock", issue = "134645")]
impl<T: ?Sized + fmt::Debug> fmt::Debug for RwLockReadGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}

#[unstable(feature = "nonpoison_rwlock", issue = "134645")]
impl<T: ?Sized + fmt::Display> fmt::Display for RwLockReadGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}

#[unstable(feature = "nonpoison_rwlock", issue = "134645")]
impl<T: ?Sized + fmt::Debug> fmt::Debug for RwLockWriteGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}

#[unstable(feature = "nonpoison_rwlock", issue = "134645")]
impl<T: ?Sized + fmt::Display> fmt::Display for RwLockWriteGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}

#[unstable(feature = "mapped_lock_guards", issue = "117108")]
// #[unstable(feature = "nonpoison_rwlock", issue = "134645")]
impl<T: ?Sized + fmt::Debug> fmt::Debug for MappedRwLockReadGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}

#[unstable(feature = "mapped_lock_guards", issue = "117108")]
// #[unstable(feature = "nonpoison_rwlock", issue = "134645")]
impl<T: ?Sized + fmt::Display> fmt::Display for MappedRwLockReadGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}

#[unstable(feature = "mapped_lock_guards", issue = "117108")]
// #[unstable(feature = "nonpoison_rwlock", issue = "134645")]
impl<T: ?Sized + fmt::Debug> fmt::Debug for MappedRwLockWriteGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}

#[unstable(feature = "mapped_lock_guards", issue = "117108")]
// #[unstable(feature = "nonpoison_rwlock", issue = "134645")]
impl<T: ?Sized + fmt::Display> fmt::Display for MappedRwLockWriteGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}
