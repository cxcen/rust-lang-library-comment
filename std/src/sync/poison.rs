//! 采用中毒（poisoning）机制的同步对象。
//!
//! # 中毒（Poisoning）
//!
//! 本模块中的所有同步对象都实现了一种名为「中毒」的策略：当一个原语察觉到
//! 某个线程在持有该原语所授予的独占访问权（exclusive access）期间发生了
//! panic 时，该原语就会变成「中毒」状态。随后这一信息会传播给所有其他线程，
//! 用以表明该原语所保护的数据很可能已被污染（某些不变式 invariant 未被维持）。
//!
//! 「中毒」状态究竟如何影响其他线程，以及对 panic 的察觉是可靠的还是
//! 尽力而为（best-effort）的，取决于具体的原语。参见下方 [Overview](#overview)。
//!
//! 本模块中的同步对象在 [`std::sync::nonpoison`] 模块中有不采用中毒机制的
//! 替代实现。
//!
//! [`std::sync::nonpoison`]: crate::sync::nonpoison
//!
//! # 概览（Overview）
//!
//! 下面列出本模块提供的同步对象，并对每个对象给出高层概览，以及它如何
//! 运用「中毒」机制的说明。
//!
//! - [`Condvar`]：Condition Variable（条件变量），提供在等待某事件发生时
//!   阻塞线程的能力。
//!
//!   条件变量通常与一个布尔谓词（一个条件）和一个互斥锁关联在一起。
//!   本实现与采用中毒机制的 [`poison::Mutex`](Mutex) 关联。
//!   正因如此，[`Condvar::wait()`] 会返回 [`LockResult`]，
//!   就像 [`poison::Mutex::lock()`](Mutex::lock) 那样。
//!
//! - [`Mutex`]：Mutual Exclusion（互斥）机制，确保任一时刻至多一个线程
//!   能访问某些数据。
//!
//!   在持有锁期间发生 panic 通常会使该互斥锁中毒，但并不保证在所有情形下
//!   都能检测到这一状况。[`Mutex::lock()`] 返回 [`LockResult`]，提供了处理
//!   中毒状态的途径。详见 [`Mutex` 的文档](Mutex#poisoning)。
//!
//! - [`RwLock`]：提供一种互斥机制，允许多个读者同时读，而同一时刻只允许
//!   一个写者。在某些情况下，这比互斥锁更高效。
//!
//!   本实现与 [`Mutex`] 一样，通常会在 panic 时中毒。但请注意，`RwLock`
//!   只有在被以独占方式（写模式 write mode）锁定期间发生 panic 时才可能中毒。
//!   如果 panic 发生在任一读者中，则该锁不会中毒。
//!
//! 注意，[`Once`] 类型也采用了中毒机制，但由于它自身带有不中毒的 `force`
//! 系列方法，因此没有单独的 `nonpoison` 与 `poison` 版本之分。
//!
//! [`Once`]: crate::sync::Once

// 如果我们不进行栈展开（unwinding），那么 `PoisonError` 是不可居留的
// （uninhabited，即不可能存在该类型的值）。
#![cfg_attr(not(panic = "unwind"), expect(unreachable_code))]

#[stable(feature = "rust1", since = "1.0.0")]
pub use self::condvar::Condvar;
#[unstable(feature = "mapped_lock_guards", issue = "117108")]
pub use self::mutex::MappedMutexGuard;
#[stable(feature = "rust1", since = "1.0.0")]
pub use self::mutex::{Mutex, MutexGuard};
#[unstable(feature = "mapped_lock_guards", issue = "117108")]
pub use self::rwlock::{MappedRwLockReadGuard, MappedRwLockWriteGuard};
#[stable(feature = "rust1", since = "1.0.0")]
pub use self::rwlock::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use crate::error::Error;
use crate::fmt;
#[cfg(panic = "unwind")]
use crate::sync::atomic::{Atomic, AtomicBool, Ordering};
#[cfg(panic = "unwind")]
use crate::thread;

mod condvar;
#[stable(feature = "rust1", since = "1.0.0")]
mod mutex;
mod rwlock;

pub(crate) struct Flag {
    #[cfg(panic = "unwind")]
    failed: Atomic<bool>,
}

// 注意：下面访问 `Flag` 的 `failed` 字段时，所用的内存序（Ordering）始终是
// `Relaxed`。这是因为它实际上并不保护任何数据，它只是一个标记，用来表示
// 我们是否已经发生过 panic。
//
// 真正要紧的位置是在互斥锁被 **加锁（locked）** 的时候——在那里我们有外部
// 同步（external synchronization）来确保我们能看到对此标志的内存读/写。
//
// 因此，在真正要紧的所有情形下，我们都应当看到 `failed` 的正确取值。

impl Flag {
    #[inline]
    pub const fn new() -> Flag {
        Flag {
            #[cfg(panic = "unwind")]
            failed: AtomicBool::new(false),
        }
    }

    /// 为一次「无守卫借用」（unguarded borrow）检查该标志，此时我们只关心
    /// 已有的中毒状态。
    #[inline]
    pub fn borrow(&self) -> LockResult<()> {
        if self.get() { Err(PoisonError::new(())) } else { Ok(()) }
    }

    /// 为一次「有守卫借用」（guarded borrow）检查该标志，此时我们也可能在
    /// `done` 时设置中毒。
    #[inline]
    pub fn guard(&self) -> LockResult<Guard> {
        let ret = Guard {
            #[cfg(panic = "unwind")]
            panicking: thread::panicking(),
        };
        if self.get() { Err(PoisonError::new(ret)) } else { Ok(ret) }
    }

    #[inline]
    #[cfg(panic = "unwind")]
    pub fn done(&self, guard: &Guard) {
        // 仅当：当初获取守卫时该线程并未在 panic，而现在却正在 panic，
        // 才把标志置为已失败——也就是说，panic 是在持有锁期间新发生的。
        if !guard.panicking && thread::panicking() {
            self.failed.store(true, Ordering::Relaxed);
        }
    }

    #[inline]
    #[cfg(not(panic = "unwind"))]
    pub fn done(&self, _guard: &Guard) {}

    #[inline]
    #[cfg(panic = "unwind")]
    pub fn get(&self) -> bool {
        self.failed.load(Ordering::Relaxed)
    }

    #[inline(always)]
    #[cfg(not(panic = "unwind"))]
    pub fn get(&self) -> bool {
        false
    }

    #[inline]
    pub fn clear(&self) {
        #[cfg(panic = "unwind")]
        self.failed.store(false, Ordering::Relaxed)
    }
}

#[derive(Clone)]
pub(crate) struct Guard {
    // 记录构造该守卫时所在线程是否正在 panic，用于在 `done` 中区分
    // 「panic 是否是在持锁期间新发生的」。
    #[cfg(panic = "unwind")]
    panicking: bool,
}

/// 一种在获取锁时可能返回的错误类型。
///
/// 每当某个线程在持有锁期间失败（fail）时，[`Mutex`] 和 [`RwLock`] 都会中毒。
/// 锁在何种确切语义下会中毒，记录在各个锁各自的文档中。对于处于中毒状态的锁，
/// 除非手动清除该状态，否则今后所有的获取操作都会返回这个错误。
///
/// # 示例
///
/// ```
/// use std::sync::{Arc, Mutex};
/// use std::thread;
///
/// let mutex = Arc::new(Mutex::new(1));
///
/// // 使该互斥锁中毒
/// let c_mutex = Arc::clone(&mutex);
/// let _ = thread::spawn(move || {
///     let mut data = c_mutex.lock().unwrap();
///     *data = 2;
///     panic!();
/// }).join();
///
/// match mutex.lock() {
///     Ok(_) => unreachable!(),
///     Err(p_err) => {
///         let data = p_err.get_ref();
///         println!("recovered: {data}");
///     }
/// };
/// ```
/// [`Mutex`]: crate::sync::Mutex
/// [`RwLock`]: crate::sync::RwLock
#[stable(feature = "rust1", since = "1.0.0")]
pub struct PoisonError<T> {
    data: T,
    // 在 `panic="abort"` 构建下，该类型不可能被构造出来：用 `!`（never 类型）
    // 字段把它标记为不可居留（uninhabited）。
    #[cfg(not(panic = "unwind"))]
    _never: !,
}

/// 与 [`TryLockResult`] 关联的、在尝试获取锁时可能发生的各种错误的枚举；
/// 这些错误来自 [`Mutex`] 上的 [`try_lock`] 方法，或 [`RwLock`] 上的
/// [`try_read`] 与 [`try_write`] 方法。
///
/// [`try_lock`]: crate::sync::Mutex::try_lock
/// [`try_read`]: crate::sync::RwLock::try_read
/// [`try_write`]: crate::sync::RwLock::try_write
/// [`Mutex`]: crate::sync::Mutex
/// [`RwLock`]: crate::sync::RwLock
#[stable(feature = "rust1", since = "1.0.0")]
pub enum TryLockError<T> {
    /// 无法获取该锁，因为另一个线程在持有该锁期间失败了（即锁已中毒）。
    #[stable(feature = "rust1", since = "1.0.0")]
    Poisoned(#[stable(feature = "rust1", since = "1.0.0")] PoisonError<T>),
    /// 此刻无法获取该锁，因为该操作否则就会发生阻塞。
    #[stable(feature = "rust1", since = "1.0.0")]
    WouldBlock,
}

/// 一个可能中毒的加锁方法所返回结果的类型别名。
///
/// 该结果的 [`Ok`] 变体表示原语未中毒，操作结果包含于其中。[`Err`] 变体表示
/// 原语已中毒。注意 [`Err`] 变体 *同样* 携带一个由加锁方法赋予的关联值，
/// 可通过 [`into_inner`] 方法取得。该关联值的语义取决于对应的加锁方法。
///
/// [`into_inner`]: PoisonError::into_inner
#[stable(feature = "rust1", since = "1.0.0")]
pub type LockResult<T> = Result<T, PoisonError<T>>;

/// 一个非阻塞加锁方法所返回结果的类型别名。
///
/// 更多信息参见 [`LockResult`]。`TryLockResult` 的 [`Err`] 类型中不一定会
/// 持有关联的守卫，因为锁也可能是由于其他原因而未获取到。
#[stable(feature = "rust1", since = "1.0.0")]
pub type TryLockResult<Guard> = Result<Guard, TryLockError<Guard>>;

#[stable(feature = "rust1", since = "1.0.0")]
impl<T> fmt::Debug for PoisonError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PoisonError").finish_non_exhaustive()
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T> fmt::Display for PoisonError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        "poisoned lock: another task failed inside".fmt(f)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T> Error for PoisonError<T> {}

impl<T> PoisonError<T> {
    /// 创建一个 `PoisonError`。
    ///
    /// 它通常由 [`Mutex::lock`](crate::sync::Mutex::lock) 或
    /// [`RwLock::read`](crate::sync::RwLock::read) 这类方法创建。
    ///
    /// 如果 std 是以 `panic="abort"` 构建的，本方法可能 panic。
    #[cfg(panic = "unwind")]
    #[stable(feature = "sync_poison", since = "1.2.0")]
    pub fn new(data: T) -> PoisonError<T> {
        PoisonError { data }
    }

    /// 创建一个 `PoisonError`。
    ///
    /// 它通常由 [`Mutex::lock`](crate::sync::Mutex::lock) 或
    /// [`RwLock::read`](crate::sync::RwLock::read) 这类方法创建。
    ///
    /// 如果 std 是以 `panic="abort"` 构建的，本方法可能 panic。
    #[cfg(not(panic = "unwind"))]
    #[stable(feature = "sync_poison", since = "1.2.0")]
    #[track_caller]
    pub fn new(_data: T) -> PoisonError<T> {
        // 在 `panic="abort"` 下不会有中毒发生，因此 `PoisonError` 本不该被构造；
        // 一旦走到这里说明逻辑有误，直接 panic。
        panic!("PoisonError created in a libstd built with panic=\"abort\"")
    }

    /// 消耗这个表示锁已中毒的错误，返回其关联的数据。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashSet;
    /// use std::sync::{Arc, Mutex};
    /// use std::thread;
    ///
    /// let mutex = Arc::new(Mutex::new(HashSet::new()));
    ///
    /// // 使该互斥锁中毒
    /// let c_mutex = Arc::clone(&mutex);
    /// let _ = thread::spawn(move || {
    ///     let mut data = c_mutex.lock().unwrap();
    ///     data.insert(10);
    ///     panic!();
    /// }).join();
    ///
    /// let p_err = mutex.lock().unwrap_err();
    /// let data = p_err.into_inner();
    /// println!("recovered {} items", data.len());
    /// ```
    #[stable(feature = "sync_poison", since = "1.2.0")]
    pub fn into_inner(self) -> T {
        self.data
    }

    /// 探入这个表示锁已中毒的错误，返回其关联数据的一个引用。
    #[stable(feature = "sync_poison", since = "1.2.0")]
    pub fn get_ref(&self) -> &T {
        &self.data
    }

    /// 探入这个表示锁已中毒的错误，返回其关联数据的一个可变引用。
    #[stable(feature = "sync_poison", since = "1.2.0")]
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.data
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T> From<PoisonError<T>> for TryLockError<T> {
    fn from(err: PoisonError<T>) -> TryLockError<T> {
        TryLockError::Poisoned(err)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T> fmt::Debug for TryLockError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            #[cfg(panic = "unwind")]
            TryLockError::Poisoned(..) => "Poisoned(..)".fmt(f),
            #[cfg(not(panic = "unwind"))]
            TryLockError::Poisoned(ref p) => match p._never {},
            TryLockError::WouldBlock => "WouldBlock".fmt(f),
        }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T> fmt::Display for TryLockError<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            #[cfg(panic = "unwind")]
            TryLockError::Poisoned(..) => "poisoned lock: another task failed inside",
            #[cfg(not(panic = "unwind"))]
            TryLockError::Poisoned(ref p) => match p._never {},
            TryLockError::WouldBlock => "try_lock failed because the operation would block",
        }
        .fmt(f)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T> Error for TryLockError<T> {
    #[allow(deprecated)]
    fn cause(&self) -> Option<&dyn Error> {
        match *self {
            #[cfg(panic = "unwind")]
            TryLockError::Poisoned(ref p) => Some(p),
            #[cfg(not(panic = "unwind"))]
            TryLockError::Poisoned(ref p) => match p._never {},
            _ => None,
        }
    }
}

pub(crate) fn map_result<T, U, F>(result: LockResult<T>, f: F) -> LockResult<U>
where
    F: FnOnce(T) -> U,
{
    // 把 `f` 应用到结果内部的值上，同时保留其中毒状态：Ok 仍是 Ok，
    // 中毒的 Err 仍是携带映射后数据的中毒 Err。
    match result {
        Ok(t) => Ok(f(t)),
        #[cfg(panic = "unwind")]
        Err(PoisonError { data }) => Err(PoisonError::new(f(data))),
    }
}
