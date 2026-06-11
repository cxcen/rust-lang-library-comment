//! 系统 Mutex
//!
//! Windows 的 mutex 实现有点奇怪，初看可能不太容易明白发生了什么。最主要的怪异
//! 之处在于：这里用的是 SRWLock 而不是 CriticalSection，之所以这么做是因为：
//!
//! 1. 根据在 Windows 8 和 Windows 7 上都做过的基准测试，SRWLock 比 CriticalSection
//!    快好几倍。
//!
//! 2. CriticalSection 允许递归加锁，而 SRWLock 会死锁。Unix 的实现也是死锁，
//!    所以这里更倾向于保持一致性。详见 #19962。
//!
//! 3. 虽然 CriticalSection 是公平的而 SRWLock 不是，但当前 Rust 的策略是
//!    不对公平性做任何保证。

use crate::cell::UnsafeCell;
use crate::sys::c;

pub struct Mutex {
    srwlock: UnsafeCell<c::SRWLOCK>,
}

unsafe impl Send for Mutex {}
unsafe impl Sync for Mutex {}

#[inline]
pub unsafe fn raw(m: &Mutex) -> *mut c::SRWLOCK {
    m.srwlock.get()
}

impl Mutex {
    #[inline]
    pub const fn new() -> Mutex {
        Mutex { srwlock: UnsafeCell::new(c::SRWLOCK_INIT) }
    }

    #[inline]
    pub fn lock(&self) {
        unsafe {
            c::AcquireSRWLockExclusive(raw(self));
        }
    }

    #[inline]
    pub fn try_lock(&self) -> bool {
        unsafe { c::TryAcquireSRWLockExclusive(raw(self)) }
    }

    #[inline]
    pub unsafe fn unlock(&self) {
        c::ReleaseSRWLockExclusive(raw(self));
    }
}
