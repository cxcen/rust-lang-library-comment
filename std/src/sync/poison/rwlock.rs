use crate::cell::UnsafeCell;
use crate::fmt;
use crate::marker::PhantomData;
use crate::mem::{self, ManuallyDrop, forget};
use crate::ops::{Deref, DerefMut};
use crate::ptr::NonNull;
use crate::sync::{LockResult, PoisonError, TryLockError, TryLockResult, poison};
use crate::sys::sync as sys;

/// 一种读写锁（reader-writer lock）
///
/// 这类锁在任一时刻允许若干个读者（reader）或至多一个写者（writer）。本锁的
/// 写部分通常允许修改底层数据（独占访问 exclusive access），而本锁的读部分
/// 通常只允许只读访问（共享访问 shared access）。
///
/// 相比之下，[`Mutex`] 不区分获取锁的是读者还是写者，因此会阻塞任何等待锁
/// 变为可用的线程。只要没有写者持有锁，`RwLock` 就允许任意数量的读者获取锁。
///
/// 锁的优先级策略取决于底层操作系统的实现，本类型不保证会采用任何特定策略。
/// 特别是，一个正等待在 `write` 中获取锁的写者，可能会、也可能不会阻塞并发的
/// `read` 调用，例如：
///
/// <details><summary>潜在死锁示例</summary>
///
/// ```text
/// // 线程 1                  |  // 线程 2
/// let _rg1 = lock.read();  |
///                          |  // 将会阻塞
///                          |  let _wg = lock.write();
/// // 可能死锁               |
/// let _rg2 = lock.read();  |
/// ```
///
/// </details>
///
/// 类型参数 `T` 代表这个锁所保护的数据。要求 `T` 满足 [`Send`] 才能在线程间
/// 共享，满足 [`Sync`] 才能允许通过多个读者并发访问。由加锁方法返回的 RAII
/// 守卫实现了 [`Deref`]（对 `write` 系列方法还实现了 [`DerefMut`]），以允许
/// 访问锁的内容。
///
/// # 中毒（Poisoning）
///
/// 与 [`Mutex`] 一样，`RwLock` 在 panic 时 [通常][usually] 会中毒。但请注意，
/// `RwLock` 只有在被以独占方式（写模式 write mode）锁定期间发生 panic 时才
/// 可能中毒。如果 panic 发生在任一读者中，则该锁不会中毒。
///
/// [usually]: super::Mutex#poisoning
///
/// # 示例
///
/// ```
/// use std::sync::RwLock;
///
/// let lock = RwLock::new(5);
///
/// // 可以同时持有多个读锁（reader lock）
/// {
///     let r1 = lock.read().unwrap();
///     let r2 = lock.read().unwrap();
///     assert_eq!(*r1, 5);
///     assert_eq!(*r2, 5);
/// } // 读锁在此处被 drop
///
/// // 然而，同一时刻只能持有一个写锁（write lock）
/// {
///     let mut w = lock.write().unwrap();
///     *w += 1;
///     assert_eq!(*w, 6);
/// } // 写锁在此处被 drop
/// ```
///
/// [`Mutex`]: super::Mutex
#[stable(feature = "rust1", since = "1.0.0")]
#[cfg_attr(not(test), rustc_diagnostic_item = "RwLock")]
pub struct RwLock<T: ?Sized> {
    /// 用于同步线程对受保护数据的访问的内部 [`sys::RwLock`]。
    inner: sys::RwLock,
    /// 一个标记，表明这个 `RwLock` 是否已中毒。
    poison: poison::Flag,
    /// 受锁保护的数据。
    data: UnsafeCell<T>,
}

#[stable(feature = "rust1", since = "1.0.0")]
unsafe impl<T: ?Sized + Send> Send for RwLock<T> {}

#[stable(feature = "rust1", since = "1.0.0")]
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
#[stable(feature = "rust1", since = "1.0.0")]
#[clippy::has_significant_drop]
#[cfg_attr(not(test), rustc_diagnostic_item = "RwLockReadGuard")]
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

#[stable(feature = "rust1", since = "1.0.0")]
impl<T: ?Sized> !Send for RwLockReadGuard<'_, T> {}

#[stable(feature = "rwlock_guard_sync", since = "1.23.0")]
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
#[stable(feature = "rust1", since = "1.0.0")]
#[clippy::has_significant_drop]
#[cfg_attr(not(test), rustc_diagnostic_item = "RwLockWriteGuard")]
pub struct RwLockWriteGuard<'rwlock, T: ?Sized + 'rwlock> {
    /// 一个指向我们已加写锁的 [`RwLock`] 的引用。
    lock: &'rwlock RwLock<T>,
    /// 中毒守卫（poison guard）。更多信息参见 [`poison`] 模块。
    poison: poison::Guard,
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T: ?Sized> !Send for RwLockWriteGuard<'_, T> {}

#[stable(feature = "rwlock_guard_sync", since = "1.23.0")]
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
impl<T: ?Sized> !Send for MappedRwLockReadGuard<'_, T> {}

#[unstable(feature = "mapped_lock_guards", issue = "117108")]
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
#[clippy::has_significant_drop]
pub struct MappedRwLockWriteGuard<'rwlock, T: ?Sized + 'rwlock> {
    /// 一个指向受 `RwLock` 保护的数据的指针。注意，这里我们用裸指针而非
    /// `&'rwlock T`，以避免违反 `noalias`，因为一个 `MappedRwLockWriteGuard`
    /// 实例只在它被 drop 之前保持唯一性（uniqueness），而非在其整个作用域内。
    /// 相比 `*const T`，`NonNull` 更可取，因为它允许 niche 优化。
    data: NonNull<T>,
    /// `NonNull` 对 `T` 是协变的，所以我们在这里加一个
    /// `PhantomData<&'rwlock mut T>` 字段，以强制对 `T` 施加正确的不变性
    /// （invariance）。
    _variance: PhantomData<&'rwlock mut T>,
    /// 一个指向我们已加写锁的内部 [`sys::RwLock`] 的引用。
    inner_lock: &'rwlock sys::RwLock,
    /// 一个指向原始 `RwLock` 中毒状态的引用。
    poison_flag: &'rwlock poison::Flag,
    /// 中毒守卫（poison guard）。更多信息参见 [`poison`] 模块。
    poison_guard: poison::Guard,
}

#[unstable(feature = "mapped_lock_guards", issue = "117108")]
impl<T: ?Sized> !Send for MappedRwLockWriteGuard<'_, T> {}

#[unstable(feature = "mapped_lock_guards", issue = "117108")]
unsafe impl<T: ?Sized + Sync> Sync for MappedRwLockWriteGuard<'_, T> {}

////////////////////////////////////////////////////////////////////////////////////////////////////
// 各项实现（Implementations）
////////////////////////////////////////////////////////////////////////////////////////////////////

impl<T> RwLock<T> {
    /// 创建一个新的 `RwLock<T>` 实例，初始为未加锁状态。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::sync::RwLock;
    ///
    /// let lock = RwLock::new(5);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_locks", since = "1.63.0")]
    #[inline]
    pub const fn new(t: T) -> RwLock<T> {
        RwLock { inner: sys::RwLock::new(), poison: poison::Flag::new(), data: UnsafeCell::new(t) }
    }

    /// 通过克隆（cloning）返回所包含的值。
    ///
    /// # 错误(Errors）
    ///
    /// 如果该 `RwLock` 已中毒，则此函数会返回一个错误。每当某个写者在持有独占锁
    /// 期间发生 panic 时，`RwLock` 即被中毒。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(lock_value_accessors)]
    ///
    /// use std::sync::RwLock;
    ///
    /// let mut lock = RwLock::new(7);
    ///
    /// assert_eq!(lock.get_cloned().unwrap(), 7);
    /// ```
    #[unstable(feature = "lock_value_accessors", issue = "133407")]
    pub fn get_cloned(&self) -> Result<T, PoisonError<()>>
    where
        T: Clone,
    {
        match self.read() {
            Ok(guard) => Ok((*guard).clone()),
            Err(_) => Err(PoisonError::new(())),
        }
    }

    /// 设置所包含的值。
    ///
    /// # 错误(Errors）
    ///
    /// 如果该 `RwLock` 已中毒，则此函数会返回一个错误，其中包含所提供的 `value`。
    /// 每当某个写者在持有独占锁期间发生 panic 时，`RwLock` 即被中毒。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(lock_value_accessors)]
    ///
    /// use std::sync::RwLock;
    ///
    /// let mut lock = RwLock::new(7);
    ///
    /// assert_eq!(lock.get_cloned().unwrap(), 7);
    /// lock.set(11).unwrap();
    /// assert_eq!(lock.get_cloned().unwrap(), 11);
    /// ```
    #[unstable(feature = "lock_value_accessors", issue = "133407")]
    #[rustc_should_not_be_called_on_const_items]
    pub fn set(&self, value: T) -> Result<(), PoisonError<T>> {
        if mem::needs_drop::<T>() {
            // 如果所包含的值带有非平凡（non-trivial）的析构函数，我们会在锁被
            // 释放之后再调用该析构函数。
            self.replace(value).map(drop)
        } else {
            match self.write() {
                Ok(mut guard) => {
                    *guard = value;

                    Ok(())
                }
                Err(_) => Err(PoisonError::new(value)),
            }
        }
    }

    /// 用 `value` 替换所包含的值，并返回旧的所包含值。
    ///
    /// # 错误(Errors）
    ///
    /// 如果该 `RwLock` 已中毒，则此函数会返回一个错误，其中包含所提供的 `value`。
    /// 每当某个写者在持有独占锁期间发生 panic 时，`RwLock` 即被中毒。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(lock_value_accessors)]
    ///
    /// use std::sync::RwLock;
    ///
    /// let mut lock = RwLock::new(7);
    ///
    /// assert_eq!(lock.replace(11).unwrap(), 7);
    /// assert_eq!(lock.get_cloned().unwrap(), 11);
    /// ```
    #[unstable(feature = "lock_value_accessors", issue = "133407")]
    #[rustc_should_not_be_called_on_const_items]
    pub fn replace(&self, value: T) -> LockResult<T> {
        match self.write() {
            Ok(mut guard) => Ok(mem::replace(&mut *guard, value)),
            Err(_) => Err(PoisonError::new(value)),
        }
    }
}

impl<T: ?Sized> RwLock<T> {
    /// 以共享读访问（shared read access）锁定这个 `RwLock`，阻塞当前线程直到
    /// 锁可以被获取为止。
    ///
    /// 调用线程将一直被阻塞，直到不再有任何写者持有该锁。当此方法返回时，锁内
    /// 可能仍有其他读者。本方法不对竞争中的读者与写者谁先获取锁的顺序作任何保证。
    ///
    /// 返回一个 RAII 守卫，它在被 drop 时释放本线程的共享访问。
    ///
    /// # 错误(Errors）
    ///
    /// 如果该 `RwLock` 已中毒，则此函数会返回一个错误。每当某个写者在持有独占锁
    /// 期间发生 panic 时，`RwLock` 即被中毒。该失败会在锁被获取之后立即发生。
    /// 获取到的锁守卫将被包含在返回的错误中。
    ///
    /// # Panics
    ///
    /// 如果调用时该锁已被当前线程持有，则此函数可能 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::sync::{Arc, RwLock};
    /// use std::thread;
    ///
    /// let lock = Arc::new(RwLock::new(1));
    /// let c_lock = Arc::clone(&lock);
    ///
    /// let n = lock.read().unwrap();
    /// assert_eq!(*n, 1);
    ///
    /// thread::spawn(move || {
    ///     let r = c_lock.read();
    ///     assert!(r.is_ok());
    /// }).join().unwrap();
    /// ```
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_should_not_be_called_on_const_items]
    pub fn read(&self) -> LockResult<RwLockReadGuard<'_, T>> {
        unsafe {
            self.inner.read();
            RwLockReadGuard::new(self)
        }
    }

    /// 尝试以共享读访问（shared read access）获取这个 `RwLock`。
    ///
    /// 如果此刻无法授予该访问，则返回 `Err`。否则，返回一个 RAII 守卫，它在被
    /// drop 时释放共享访问。
    ///
    /// 此函数不会阻塞。
    ///
    /// 本函数不对竞争中的读者与写者谁先获取锁的顺序作任何保证。
    ///
    /// # 错误(Errors）
    ///
    /// 如果该 `RwLock` 已中毒，则此函数会返回 [`Poisoned`] 错误。每当某个写者在
    /// 持有独占锁期间发生 panic 时，`RwLock` 即被中毒。`Poisoned` 只在锁原本本来
    /// 会被获取的情况下才会返回。获取到的锁守卫将被包含在返回的错误中。
    ///
    /// 如果由于该 `RwLock` 已被以独占方式锁定而无法获取，则此函数会返回
    /// [`WouldBlock`] 错误。
    ///
    /// [`Poisoned`]: TryLockError::Poisoned
    /// [`WouldBlock`]: TryLockError::WouldBlock
    ///
    /// # 示例
    ///
    /// ```
    /// use std::sync::RwLock;
    ///
    /// let lock = RwLock::new(1);
    ///
    /// match lock.try_read() {
    ///     Ok(n) => assert_eq!(*n, 1),
    ///     Err(_) => unreachable!(),
    /// };
    /// ```
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_should_not_be_called_on_const_items]
    pub fn try_read(&self) -> TryLockResult<RwLockReadGuard<'_, T>> {
        unsafe {
            if self.inner.try_read() {
                Ok(RwLockReadGuard::new(self)?)
            } else {
                Err(TryLockError::WouldBlock)
            }
        }
    }

    /// 以独占写访问（exclusive write access）锁定这个 `RwLock`，阻塞当前线程
    /// 直到锁可以被获取为止。
    ///
    /// 当其他写者或其他读者当前持有该锁的访问权时，此函数不会返回。
    ///
    /// 返回一个 RAII 守卫，它在被 drop 时释放本 `RwLock` 的写访问。
    ///
    /// # 错误(Errors）
    ///
    /// 如果该 `RwLock` 已中毒，则此函数会返回一个错误。每当某个写者在持有独占锁
    /// 期间发生 panic 时，`RwLock` 即被中毒。锁被获取时会返回一个错误。获取到的
    /// 锁守卫将被包含在返回的错误中。
    ///
    /// # Panics
    ///
    /// 如果调用时该锁已被当前线程持有，则此函数可能 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::sync::RwLock;
    ///
    /// let lock = RwLock::new(1);
    ///
    /// let mut n = lock.write().unwrap();
    /// *n = 2;
    ///
    /// assert!(lock.try_read().is_err());
    /// ```
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_should_not_be_called_on_const_items]
    pub fn write(&self) -> LockResult<RwLockWriteGuard<'_, T>> {
        unsafe {
            self.inner.write();
            RwLockWriteGuard::new(self)
        }
    }

    /// 尝试以独占写访问（exclusive write access）锁定这个 `RwLock`。
    ///
    /// 如果此刻无法获取该锁，则返回 `Err`。否则，返回一个 RAII 守卫，它在被
    /// drop 时释放该锁。
    ///
    /// 此函数不会阻塞。
    ///
    /// 本函数不对竞争中的读者与写者谁先获取锁的顺序作任何保证。
    ///
    /// # 错误(Errors）
    ///
    /// 如果该 `RwLock` 已中毒，则此函数会返回 [`Poisoned`] 错误。每当某个写者在
    /// 持有独占锁期间发生 panic 时，`RwLock` 即被中毒。`Poisoned` 只在锁原本本来
    /// 会被获取的情况下才会返回。获取到的锁守卫将被包含在返回的错误中。
    ///
    /// 如果由于该 `RwLock` 已被锁定而无法获取，则此函数会返回 [`WouldBlock`] 错误。
    ///
    /// [`Poisoned`]: TryLockError::Poisoned
    /// [`WouldBlock`]: TryLockError::WouldBlock
    ///
    ///
    /// # 示例
    ///
    /// ```
    /// use std::sync::RwLock;
    ///
    /// let lock = RwLock::new(1);
    ///
    /// let n = lock.read().unwrap();
    /// assert_eq!(*n, 1);
    ///
    /// assert!(lock.try_write().is_err());
    /// ```
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_should_not_be_called_on_const_items]
    pub fn try_write(&self) -> TryLockResult<RwLockWriteGuard<'_, T>> {
        unsafe {
            if self.inner.try_write() {
                Ok(RwLockWriteGuard::new(self)?)
            } else {
                Err(TryLockError::WouldBlock)
            }
        }
    }

    /// 判定该锁是否已中毒。
    ///
    /// 如果另一个线程处于活动状态，该锁仍可能在任意时刻变为中毒。在没有额外
    /// 同步的情况下，你不应为了程序正确性而信赖 `false` 这个返回值。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::sync::{Arc, RwLock};
    /// use std::thread;
    ///
    /// let lock = Arc::new(RwLock::new(0));
    /// let c_lock = Arc::clone(&lock);
    ///
    /// let _ = thread::spawn(move || {
    ///     let _lock = c_lock.write().unwrap();
    ///     panic!(); // 该锁因此中毒
    /// }).join();
    /// assert_eq!(lock.is_poisoned(), true);
    /// ```
    #[inline]
    #[stable(feature = "sync_poison", since = "1.2.0")]
    pub fn is_poisoned(&self) -> bool {
        self.poison.get()
    }

    /// 清除一个锁的中毒状态。
    ///
    /// 如果该锁已中毒，它将一直保持中毒，直到此函数被调用为止。这使得可以从中毒
    /// 状态中恢复，并标记它已经恢复。例如，如果该值被一个已知良好的值覆写，那么
    /// 这个锁就可以被标记为未中毒。又或者，可以检查该值以判定它是否处于一致状态，
    /// 如果是，则移除中毒标记。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::sync::{Arc, RwLock};
    /// use std::thread;
    ///
    /// let lock = Arc::new(RwLock::new(0));
    /// let c_lock = Arc::clone(&lock);
    ///
    /// let _ = thread::spawn(move || {
    ///     let _lock = c_lock.write().unwrap();
    ///     panic!(); // 该锁因此中毒
    /// }).join();
    ///
    /// assert_eq!(lock.is_poisoned(), true);
    /// let guard = lock.write().unwrap_or_else(|mut e| {
    ///     **e.get_mut() = 1;
    ///     lock.clear_poison();
    ///     e.into_inner()
    /// });
    /// assert_eq!(lock.is_poisoned(), false);
    /// assert_eq!(*guard, 1);
    /// ```
    #[inline]
    #[stable(feature = "mutex_unpoison", since = "1.77.0")]
    pub fn clear_poison(&self) {
        self.poison.clear();
    }

    /// 消耗（consume）这个 `RwLock`，返回其底层数据。
    ///
    /// # 错误(Errors）
    ///
    /// 如果该 `RwLock` 已中毒，则此函数会返回一个错误，其中包含底层数据。每当
    /// 某个写者在持有独占锁期间发生 panic 时，`RwLock` 即被中毒。只有在锁原本
    /// 本来会被获取的情况下才会返回错误。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::sync::RwLock;
    ///
    /// let lock = RwLock::new(String::new());
    /// {
    ///     let mut s = lock.write().unwrap();
    ///     *s = "modified".to_owned();
    /// }
    /// assert_eq!(lock.into_inner().unwrap(), "modified");
    /// ```
    #[stable(feature = "rwlock_into_inner", since = "1.6.0")]
    pub fn into_inner(self) -> LockResult<T>
    where
        T: Sized,
    {
        let data = self.data.into_inner();
        poison::map_result(self.poison.borrow(), |()| data)
    }

    /// 返回一个指向底层数据的可变引用。
    ///
    /// 由于此调用以可变方式借用了该 `RwLock`，所以无需进行任何实际的加锁——
    /// 这个可变借用在静态层面就保证了：只要此引用存在，就不可能获取新的锁。
    /// 注意，本方法不会清除任何先前被遗弃（abandoned）的锁（例如，通过对一个
    /// [`RwLockReadGuard`] 或 [`RwLockWriteGuard`] 调用 [`forget()`]）。
    ///
    /// # 错误(Errors）
    ///
    /// 如果该 `RwLock` 已中毒，则此函数会返回一个错误，其中包含一个指向底层数据
    /// 的可变引用。每当某个写者在持有独占锁期间发生 panic 时，`RwLock` 即被中毒。
    /// 只有在锁原本本来会被获取的情况下才会返回错误。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::sync::RwLock;
    ///
    /// let mut lock = RwLock::new(0);
    /// *lock.get_mut().unwrap() = 10;
    /// assert_eq!(*lock.read().unwrap(), 10);
    /// ```
    #[stable(feature = "rwlock_get_mut", since = "1.6.0")]
    pub fn get_mut(&mut self) -> LockResult<&mut T> {
        let data = self.data.get_mut();
        poison::map_result(self.poison.borrow(), |()| data)
    }

    /// 返回一个指向底层数据的裸指针（raw pointer）。
    ///
    /// 返回的指针始终非空且正确对齐，但确保通过它进行的任何读写都被正确同步以
    /// 避免数据竞争（data race）、以及确保不在锁被 drop 之后再通过它进行读写，
    /// 是使用者的责任。
    #[unstable(feature = "rwlock_data_ptr", issue = "140368")]
    pub const fn data_ptr(&self) -> *mut T {
        self.data.get()
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T: ?Sized + fmt::Debug> fmt::Debug for RwLock<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut d = f.debug_struct("RwLock");
        match self.try_read() {
            Ok(guard) => {
                d.field("data", &&*guard);
            }
            Err(TryLockError::Poisoned(err)) => {
                d.field("data", &&**err.get_ref());
            }
            Err(TryLockError::WouldBlock) => {
                d.field("data", &format_args!("<locked>"));
            }
        }
        d.field("poisoned", &self.poison.get());
        d.finish_non_exhaustive()
    }
}

#[stable(feature = "rw_lock_default", since = "1.10.0")]
impl<T: Default> Default for RwLock<T> {
    /// 用 T 的 `Default` 值创建一个新的 `RwLock<T>`。
    fn default() -> RwLock<T> {
        RwLock::new(Default::default())
    }
}

#[stable(feature = "rw_lock_from", since = "1.24.0")]
impl<T> From<T> for RwLock<T> {
    /// 创建一个新的 `RwLock<T>` 实例，初始为未加锁状态。
    /// 这等价于 [`RwLock::new`]。
    fn from(t: T) -> Self {
        RwLock::new(t)
    }
}

impl<'rwlock, T: ?Sized> RwLockReadGuard<'rwlock, T> {
    /// 从一个 `RwLock<T>` 创建一个新的 `RwLockReadGuard<T>` 实例。
    ///
    /// # 安全性(Safety）
    ///
    /// 当且仅当同一线程在实例化本对象之前已经成功且安全地调用过
    /// `lock.inner.read()`、`lock.inner.try_read()` 或 `lock.inner.downgrade()`
    /// 时，此函数才是安全的。
    unsafe fn new(lock: &'rwlock RwLock<T>) -> LockResult<RwLockReadGuard<'rwlock, T>> {
        poison::map_result(lock.poison.borrow(), |()| RwLockReadGuard {
            data: unsafe { NonNull::new_unchecked(lock.data.get()) },
            inner_lock: &lock.inner,
        })
    }

    /// 为所借用数据的某个组成部分（例如某个枚举变体）制作一个
    /// [`MappedRwLockReadGuard`]。
    ///
    /// 该 `RwLock` 已经被锁定为读模式，所以此操作不会失败。
    ///
    /// 这是一个关联函数（associated function），需要以
    /// `RwLockReadGuard::map(...)` 的形式使用。如果写成方法，则会与通过 `Deref`
    /// 访问的 `RwLockReadGuard` 内容上同名的方法相冲突。
    ///
    /// # Panics
    ///
    /// 如果闭包发生 panic，则守卫将被 drop（解锁），且该 RwLock 不会中毒。
    #[unstable(feature = "mapped_lock_guards", issue = "117108")]
    pub fn map<U, F>(orig: Self, f: F) -> MappedRwLockReadGuard<'rwlock, U>
    where
        F: FnOnce(&T) -> &U,
        U: ?Sized,
    {
        // SAFETY: 当原始守卫被创建时，`RwLockReadGuard::new` 的各项条件已被满足，
        // 并且在整个 `map` 和/或 `filter_map` 过程中始终得到维持。
        // 闭包的签名保证了它不会“泄漏”传给它的引用的生命周期。如果闭包发生
        // panic，守卫将被 drop。
        let data = NonNull::from(f(unsafe { orig.data.as_ref() }));
        let orig = ManuallyDrop::new(orig);
        MappedRwLockReadGuard { data, inner_lock: &orig.inner_lock }
    }

    /// 为所借用数据的某个组成部分制作一个 [`MappedRwLockReadGuard`]。如果闭包
    /// 返回 `None`，则原始守卫会以 `Err(...)` 的形式返回。
    ///
    /// 该 `RwLock` 已经被锁定为读模式，所以此操作不会失败。
    ///
    /// 这是一个关联函数，需要以 `RwLockReadGuard::filter_map(...)` 的形式使用。
    /// 如果写成方法，则会与通过 `Deref` 访问的 `RwLockReadGuard` 内容上同名的
    /// 方法相冲突。
    ///
    /// # Panics
    ///
    /// 如果闭包发生 panic，则守卫将被 drop（解锁），且该 RwLock 不会中毒。
    #[unstable(feature = "mapped_lock_guards", issue = "117108")]
    pub fn filter_map<U, F>(orig: Self, f: F) -> Result<MappedRwLockReadGuard<'rwlock, U>, Self>
    where
        F: FnOnce(&T) -> Option<&U>,
        U: ?Sized,
    {
        // SAFETY: 当原始守卫被创建时，`RwLockReadGuard::new` 的各项条件已被满足，
        // 并且在整个 `map` 和/或 `filter_map` 过程中始终得到维持。
        // 闭包的签名保证了它不会“泄漏”传给它的引用的生命周期。如果闭包发生
        // panic，守卫将被 drop。
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
    /// # 安全性(Safety）
    ///
    /// 当且仅当同一线程在实例化本对象之前已经成功且安全地调用过
    /// `lock.inner.write()`、`lock.inner.try_write()` 或 `lock.inner.try_upgrade`
    /// 时，此函数才是安全的。
    unsafe fn new(lock: &'rwlock RwLock<T>) -> LockResult<RwLockWriteGuard<'rwlock, T>> {
        poison::map_result(lock.poison.guard(), |guard| RwLockWriteGuard { lock, poison: guard })
    }

    /// 将一个已加写锁的 `RwLockWriteGuard` 降级（downgrade）为一个已加读锁的
    /// [`RwLockReadGuard`]。
    ///
    /// 既然我们持有 `RwLockWriteGuard`，那么该 [`RwLock`] 必然已经被锁定为写模式，
    /// 所以此方法不会失败。
    ///
    /// 降级之后，其他读者将被允许读取受保护的数据。
    ///
    /// # 示例
    ///
    /// `downgrade` 取得 `RwLockWriteGuard` 的所有权，并返回一个 [`RwLockReadGuard`]。
    ///
    /// ```
    /// use std::sync::{RwLock, RwLockWriteGuard};
    ///
    /// let rw = RwLock::new(0);
    ///
    /// let mut write_guard = rw.write().unwrap();
    /// *write_guard = 42;
    ///
    /// let read_guard = RwLockWriteGuard::downgrade(write_guard);
    /// assert_eq!(42, *read_guard);
    /// ```
    ///
    /// `downgrade` 会 _原子地（atomically）_ 将该 [`RwLock`] 的状态从独占模式变为
    /// 共享模式。这意味着，另一个正在写入的线程不可能挤进“某个线程调用 `downgrade`”
    /// 与“它在降级后执行的任何读取”这两者之间。
    ///
    /// ```
    /// use std::sync::{Arc, RwLock, RwLockWriteGuard};
    ///
    /// let rw = Arc::new(RwLock::new(1));
    ///
    /// // 将该锁置于写模式。
    /// let mut main_write_guard = rw.write().unwrap();
    ///
    /// let rw_clone = rw.clone();
    /// let evil_handle = std::thread::spawn(move || {
    ///     // 在主线程 drop 掉 `main_read_guard` 之前，这里不会返回。
    ///     let mut evil_guard = rw_clone.write().unwrap();
    ///
    ///     assert_eq!(*evil_guard, 2);
    ///     *evil_guard = 3;
    /// });
    ///
    /// *main_write_guard = 2;
    ///
    /// // 原子地将写守卫降级为读守卫。
    /// let main_read_guard = RwLockWriteGuard::downgrade(main_write_guard);
    ///
    /// // 由于 `downgrade` 是原子的，写者线程不可能改变受保护的数据。
    /// assert_eq!(*main_read_guard, 2, "`downgrade` was not atomic");
    /// #
    /// # drop(main_read_guard);
    /// # evil_handle.join().unwrap();
    /// #
    /// # let final_check = rw.read().unwrap();
    /// # assert_eq!(*final_check, 3);
    /// ```
    #[stable(feature = "rwlock_downgrade", since = "1.92.0")]
    pub fn downgrade(s: Self) -> RwLockReadGuard<'rwlock, T> {
        let lock = s.lock;

        // 我们不想调用析构函数，因为它会调用 `write_unlock`。
        forget(s);

        // SAFETY: 我们取得了一个写守卫的所有权，所以我们必然已经把该 `RwLock`
        // 置于写模式，从而满足 `downgrade` 的契约。
        unsafe { lock.inner.downgrade() };

        // SAFETY: 我们刚刚成功调用了 `downgrade`，所以我们满足其安全契约。
        unsafe { RwLockReadGuard::new(lock).unwrap_or_else(PoisonError::into_inner) }
    }

    /// 为所借用数据的某个组成部分（例如某个枚举变体）制作一个
    /// [`MappedRwLockWriteGuard`]。
    ///
    /// 该 `RwLock` 已经被锁定为写模式，所以此操作不会失败。
    ///
    /// 这是一个关联函数，需要以 `RwLockWriteGuard::map(...)` 的形式使用。如果
    /// 写成方法，则会与通过 `Deref` 访问的 `RwLockWriteGuard` 内容上同名的方法
    /// 相冲突。
    ///
    /// # Panics
    ///
    /// 如果闭包发生 panic，则守卫将被 drop（解锁），且该 RwLock 将会中毒。
    #[unstable(feature = "mapped_lock_guards", issue = "117108")]
    pub fn map<U, F>(orig: Self, f: F) -> MappedRwLockWriteGuard<'rwlock, U>
    where
        F: FnOnce(&mut T) -> &mut U,
        U: ?Sized,
    {
        // SAFETY: 当原始守卫被创建时，`RwLockWriteGuard::new` 的各项条件已被满足，
        // 并且在整个 `map` 和/或 `filter_map` 过程中始终得到维持。
        // 闭包的签名保证了它不会“泄漏”传给它的引用的生命周期。如果闭包发生
        // panic，守卫将被 drop。
        let data = NonNull::from(f(unsafe { &mut *orig.lock.data.get() }));
        let orig = ManuallyDrop::new(orig);
        MappedRwLockWriteGuard {
            data,
            inner_lock: &orig.lock.inner,
            poison_flag: &orig.lock.poison,
            poison_guard: orig.poison.clone(),
            _variance: PhantomData,
        }
    }

    /// 为所借用数据的某个组成部分制作一个 [`MappedRwLockWriteGuard`]。如果闭包
    /// 返回 `None`，则原始守卫会以 `Err(...)` 的形式返回。
    ///
    /// 该 `RwLock` 已经被锁定为写模式，所以此操作不会失败。
    ///
    /// 这是一个关联函数，需要以 `RwLockWriteGuard::filter_map(...)` 的形式使用。
    /// 如果写成方法，则会与通过 `Deref` 访问的 `RwLockWriteGuard` 内容上同名的
    /// 方法相冲突。
    ///
    /// # Panics
    ///
    /// 如果闭包发生 panic，则守卫将被 drop（解锁），且该 RwLock 将会中毒。
    #[unstable(feature = "mapped_lock_guards", issue = "117108")]
    pub fn filter_map<U, F>(orig: Self, f: F) -> Result<MappedRwLockWriteGuard<'rwlock, U>, Self>
    where
        F: FnOnce(&mut T) -> Option<&mut U>,
        U: ?Sized,
    {
        // SAFETY: 当原始守卫被创建时，`RwLockWriteGuard::new` 的各项条件已被满足，
        // 并且在整个 `map` 和/或 `filter_map` 过程中始终得到维持。
        // 闭包的签名保证了它不会“泄漏”传给它的引用的生命周期。如果闭包发生
        // panic，守卫将被 drop。
        match f(unsafe { &mut *orig.lock.data.get() }) {
            Some(data) => {
                let data = NonNull::from(data);
                let orig = ManuallyDrop::new(orig);
                Ok(MappedRwLockWriteGuard {
                    data,
                    inner_lock: &orig.lock.inner,
                    poison_flag: &orig.lock.poison,
                    poison_guard: orig.poison.clone(),
                    _variance: PhantomData,
                })
            }
            None => Err(orig),
        }
    }
}

impl<'rwlock, T: ?Sized> MappedRwLockReadGuard<'rwlock, T> {
    /// 为所借用数据的某个组成部分（例如某个枚举变体）制作一个
    /// [`MappedRwLockReadGuard`]。
    ///
    /// 该 `RwLock` 已经被锁定为读模式，所以此操作不会失败。
    ///
    /// 这是一个关联函数，需要以 `MappedRwLockReadGuard::map(...)` 的形式使用。
    /// 如果写成方法，则会与通过 `Deref` 访问的 `MappedRwLockReadGuard` 内容上
    /// 同名的方法相冲突。
    ///
    /// # Panics
    ///
    /// 如果闭包发生 panic，则守卫将被 drop（解锁），且该 RwLock 不会中毒。
    #[unstable(feature = "mapped_lock_guards", issue = "117108")]
    pub fn map<U, F>(orig: Self, f: F) -> MappedRwLockReadGuard<'rwlock, U>
    where
        F: FnOnce(&T) -> &U,
        U: ?Sized,
    {
        // SAFETY: 当原始守卫被创建时，`RwLockReadGuard::new` 的各项条件已被满足，
        // 并且在整个 `map` 和/或 `filter_map` 过程中始终得到维持。
        // 闭包的签名保证了它不会“泄漏”传给它的引用的生命周期。如果闭包发生
        // panic，守卫将被 drop。
        let data = NonNull::from(f(unsafe { orig.data.as_ref() }));
        let orig = ManuallyDrop::new(orig);
        MappedRwLockReadGuard { data, inner_lock: &orig.inner_lock }
    }

    /// 为所借用数据的某个组成部分制作一个 [`MappedRwLockReadGuard`]。如果闭包
    /// 返回 `None`，则原始守卫会以 `Err(...)` 的形式返回。
    ///
    /// 该 `RwLock` 已经被锁定为读模式，所以此操作不会失败。
    ///
    /// 这是一个关联函数，需要以 `MappedRwLockReadGuard::filter_map(...)` 的形式
    /// 使用。如果写成方法，则会与通过 `Deref` 访问的 `MappedRwLockReadGuard`
    /// 内容上同名的方法相冲突。
    ///
    /// # Panics
    ///
    /// 如果闭包发生 panic，则守卫将被 drop（解锁），且该 RwLock 不会中毒。
    #[unstable(feature = "mapped_lock_guards", issue = "117108")]
    pub fn filter_map<U, F>(orig: Self, f: F) -> Result<MappedRwLockReadGuard<'rwlock, U>, Self>
    where
        F: FnOnce(&T) -> Option<&U>,
        U: ?Sized,
    {
        // SAFETY: 当原始守卫被创建时，`RwLockReadGuard::new` 的各项条件已被满足，
        // 并且在整个 `map` 和/或 `filter_map` 过程中始终得到维持。
        // 闭包的签名保证了它不会“泄漏”传给它的引用的生命周期。如果闭包发生
        // panic，守卫将被 drop。
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
    /// 为所借用数据的某个组成部分（例如某个枚举变体）制作一个
    /// [`MappedRwLockWriteGuard`]。
    ///
    /// 该 `RwLock` 已经被锁定为写模式，所以此操作不会失败。
    ///
    /// 这是一个关联函数，需要以 `MappedRwLockWriteGuard::map(...)` 的形式使用。
    /// 如果写成方法，则会与通过 `Deref` 访问的 `MappedRwLockWriteGuard` 内容上
    /// 同名的方法相冲突。
    ///
    /// # Panics
    ///
    /// 如果闭包发生 panic，则守卫将被 drop（解锁），且该 RwLock 将会中毒。
    #[unstable(feature = "mapped_lock_guards", issue = "117108")]
    pub fn map<U, F>(mut orig: Self, f: F) -> MappedRwLockWriteGuard<'rwlock, U>
    where
        F: FnOnce(&mut T) -> &mut U,
        U: ?Sized,
    {
        // SAFETY: 当原始守卫被创建时，`RwLockWriteGuard::new` 的各项条件已被满足，
        // 并且在整个 `map` 和/或 `filter_map` 过程中始终得到维持。
        // 闭包的签名保证了它不会“泄漏”传给它的引用的生命周期。如果闭包发生
        // panic，守卫将被 drop。
        let data = NonNull::from(f(unsafe { orig.data.as_mut() }));
        let orig = ManuallyDrop::new(orig);
        MappedRwLockWriteGuard {
            data,
            inner_lock: orig.inner_lock,
            poison_flag: orig.poison_flag,
            poison_guard: orig.poison_guard.clone(),
            _variance: PhantomData,
        }
    }

    /// 为所借用数据的某个组成部分制作一个 [`MappedRwLockWriteGuard`]。如果闭包
    /// 返回 `None`，则原始守卫会以 `Err(...)` 的形式返回。
    ///
    /// 该 `RwLock` 已经被锁定为写模式，所以此操作不会失败。
    ///
    /// 这是一个关联函数，需要以 `MappedRwLockWriteGuard::filter_map(...)` 的形式
    /// 使用。如果写成方法，则会与通过 `Deref` 访问的 `MappedRwLockWriteGuard`
    /// 内容上同名的方法相冲突。
    ///
    /// # Panics
    ///
    /// 如果闭包发生 panic，则守卫将被 drop（解锁），且该 RwLock 将会中毒。
    #[unstable(feature = "mapped_lock_guards", issue = "117108")]
    pub fn filter_map<U, F>(
        mut orig: Self,
        f: F,
    ) -> Result<MappedRwLockWriteGuard<'rwlock, U>, Self>
    where
        F: FnOnce(&mut T) -> Option<&mut U>,
        U: ?Sized,
    {
        // SAFETY: 当原始守卫被创建时，`RwLockWriteGuard::new` 的各项条件已被满足，
        // 并且在整个 `map` 和/或 `filter_map` 过程中始终得到维持。
        // 闭包的签名保证了它不会“泄漏”传给它的引用的生命周期。如果闭包发生
        // panic，守卫将被 drop。
        match f(unsafe { orig.data.as_mut() }) {
            Some(data) => {
                let data = NonNull::from(data);
                let orig = ManuallyDrop::new(orig);
                Ok(MappedRwLockWriteGuard {
                    data,
                    inner_lock: orig.inner_lock,
                    poison_flag: orig.poison_flag,
                    poison_guard: orig.poison_guard.clone(),
                    _variance: PhantomData,
                })
            }
            None => Err(orig),
        }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T: ?Sized> Drop for RwLockReadGuard<'_, T> {
    fn drop(&mut self) {
        // SAFETY: 创建时已满足 `RwLockReadGuard::new` 的各项条件。
        unsafe {
            self.inner_lock.read_unlock();
        }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T: ?Sized> Drop for RwLockWriteGuard<'_, T> {
    fn drop(&mut self) {
        self.lock.poison.done(&self.poison);
        // SAFETY: 创建时已满足 `RwLockWriteGuard::new` 的各项条件。
        unsafe {
            self.lock.inner.write_unlock();
        }
    }
}

#[unstable(feature = "mapped_lock_guards", issue = "117108")]
impl<T: ?Sized> Drop for MappedRwLockReadGuard<'_, T> {
    fn drop(&mut self) {
        // SAFETY: 当原始守卫被创建时，`RwLockReadGuard::new` 的各项条件已被满足，
        // 并且在整个 `map` 和/或 `filter_map` 过程中始终得到维持。
        unsafe {
            self.inner_lock.read_unlock();
        }
    }
}

#[unstable(feature = "mapped_lock_guards", issue = "117108")]
impl<T: ?Sized> Drop for MappedRwLockWriteGuard<'_, T> {
    fn drop(&mut self) {
        self.poison_flag.done(&self.poison_guard);
        // SAFETY: 当原始守卫被创建时，`RwLockWriteGuard::new` 的各项条件已被满足，
        // 并且在整个 `map` 和/或 `filter_map` 过程中始终得到维持。
        unsafe {
            self.inner_lock.write_unlock();
        }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T: ?Sized> Deref for RwLockReadGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        // SAFETY: 创建时已满足 `RwLockReadGuard::new` 的各项条件。
        unsafe { self.data.as_ref() }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T: ?Sized> Deref for RwLockWriteGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        // SAFETY: 创建时已满足 `RwLockWriteGuard::new` 的各项条件。
        unsafe { &*self.lock.data.get() }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T: ?Sized> DerefMut for RwLockWriteGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        // SAFETY: 创建时已满足 `RwLockWriteGuard::new` 的各项条件。
        unsafe { &mut *self.lock.data.get() }
    }
}

#[unstable(feature = "mapped_lock_guards", issue = "117108")]
impl<T: ?Sized> Deref for MappedRwLockReadGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        // SAFETY: 当原始守卫被创建时，`RwLockReadGuard::new` 的各项条件已被满足，
        // 并且在整个 `map` 和/或 `filter_map` 过程中始终得到维持。
        unsafe { self.data.as_ref() }
    }
}

#[unstable(feature = "mapped_lock_guards", issue = "117108")]
impl<T: ?Sized> Deref for MappedRwLockWriteGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        // SAFETY: 当原始守卫被创建时，`RwLockWriteGuard::new` 的各项条件已被满足，
        // 并且在整个 `map` 和/或 `filter_map` 过程中始终得到维持。
        unsafe { self.data.as_ref() }
    }
}

#[unstable(feature = "mapped_lock_guards", issue = "117108")]
impl<T: ?Sized> DerefMut for MappedRwLockWriteGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        // SAFETY: 当原始守卫被创建时，`RwLockWriteGuard::new` 的各项条件已被满足，
        // 并且在整个 `map` 和/或 `filter_map` 过程中始终得到维持。
        unsafe { self.data.as_mut() }
    }
}

#[stable(feature = "std_debug", since = "1.16.0")]
impl<T: ?Sized + fmt::Debug> fmt::Debug for RwLockReadGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}

#[stable(feature = "std_guard_impls", since = "1.20.0")]
impl<T: ?Sized + fmt::Display> fmt::Display for RwLockReadGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}

#[stable(feature = "std_debug", since = "1.16.0")]
impl<T: ?Sized + fmt::Debug> fmt::Debug for RwLockWriteGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}

#[stable(feature = "std_guard_impls", since = "1.20.0")]
impl<T: ?Sized + fmt::Display> fmt::Display for RwLockWriteGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}

#[unstable(feature = "mapped_lock_guards", issue = "117108")]
impl<T: ?Sized + fmt::Debug> fmt::Debug for MappedRwLockReadGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}

#[unstable(feature = "mapped_lock_guards", issue = "117108")]
impl<T: ?Sized + fmt::Display> fmt::Display for MappedRwLockReadGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}

#[unstable(feature = "mapped_lock_guards", issue = "117108")]
impl<T: ?Sized + fmt::Debug> fmt::Debug for MappedRwLockWriteGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}

#[unstable(feature = "mapped_lock_guards", issue = "117108")]
impl<T: ?Sized + fmt::Display> fmt::Display for MappedRwLockWriteGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}
