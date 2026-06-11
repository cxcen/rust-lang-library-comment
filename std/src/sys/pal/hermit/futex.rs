use super::hermit_abi;
use crate::ptr::null;
use crate::sync::atomic::Atomic;
use crate::time::Duration;

/// 用作 futex 的原子类型，至少 32 位，但也可能更大
pub type Futex = Atomic<Primitive>;
/// 必须是 Futex 的底层类型
pub type Primitive = u32;

/// 用作 futex 的原子类型，至少 8 位，但也可能更大。
pub type SmallFutex = Atomic<SmallPrimitive>;
/// 必须是 SmallFutex 的底层类型
pub type SmallPrimitive = u32;

pub fn futex_wait(futex: &Atomic<u32>, expected: u32, timeout: Option<Duration>) -> bool {
    // 将超时计算为一个相对的 timespec。
    //
    // 溢出会被向上取整为无限超时（None）。
    let timespec = timeout.and_then(|dur| {
        Some(hermit_abi::timespec {
            tv_sec: dur.as_secs().try_into().ok()?,
            tv_nsec: dur.subsec_nanos().try_into().ok()?,
        })
    });

    let r = unsafe {
        hermit_abi::futex_wait(
            futex.as_ptr(),
            expected,
            timespec.as_ref().map_or(null(), |t| t as *const hermit_abi::timespec),
            hermit_abi::FUTEX_RELATIVE_TIMEOUT,
        )
    };

    r != -hermit_abi::errno::ETIMEDOUT
}

#[inline]
pub fn futex_wake(futex: &Atomic<u32>) -> bool {
    unsafe { hermit_abi::futex_wake(futex.as_ptr(), 1) > 0 }
}

#[inline]
pub fn futex_wake_all(futex: &Atomic<u32>) {
    unsafe {
        hermit_abi::futex_wake(futex.as_ptr(), i32::MAX);
    }
}
