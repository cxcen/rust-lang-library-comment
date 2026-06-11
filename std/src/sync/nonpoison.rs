//! 不中毒（non-poisoning）的同步锁。
//!
//! 与 [`poison`] 模块中的锁的区别在于：本模块中的锁在某个线程持有守卫
//! （guard）期间发生 panic 时 **不会** 变成中毒状态。
//!
//! [`poison`]: super::poison

use crate::fmt;

/// 一个非阻塞加锁方法所返回结果的类型别名。
#[unstable(feature = "sync_nonpoison", issue = "134645")]
pub type TryLockResult<Guard> = Result<Guard, WouldBlock>;

/// 此刻无法获取该锁，因为该操作否则就会发生阻塞。
#[unstable(feature = "sync_nonpoison", issue = "134645")]
pub struct WouldBlock;

#[unstable(feature = "sync_nonpoison", issue = "134645")]
impl fmt::Debug for WouldBlock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        "WouldBlock".fmt(f)
    }
}

#[unstable(feature = "sync_nonpoison", issue = "134645")]
impl fmt::Display for WouldBlock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        "try_lock failed because the operation would block".fmt(f)
    }
}

#[unstable(feature = "nonpoison_condvar", issue = "134645")]
pub use self::condvar::Condvar;
#[unstable(feature = "mapped_lock_guards", issue = "117108")]
pub use self::mutex::MappedMutexGuard;
#[unstable(feature = "nonpoison_mutex", issue = "134645")]
pub use self::mutex::{Mutex, MutexGuard};
#[unstable(feature = "mapped_lock_guards", issue = "117108")]
pub use self::rwlock::{MappedRwLockReadGuard, MappedRwLockWriteGuard};
#[unstable(feature = "nonpoison_rwlock", issue = "134645")]
pub use self::rwlock::{RwLock, RwLockReadGuard, RwLockWriteGuard};

mod condvar;
mod mutex;
mod rwlock;
