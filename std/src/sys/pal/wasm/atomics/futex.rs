#[cfg(target_arch = "wasm32")]
use core::arch::wasm32 as wasm;
#[cfg(target_arch = "wasm64")]
use core::arch::wasm64 as wasm;

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

/// 等待某个 futex_wake 操作来唤醒我们。
///
/// 如果该 futex 并未持有预期的值，则直接返回。
///
/// 超时返回 false，其他所有情况返回 true。
pub fn futex_wait(futex: &Atomic<u32>, expected: u32, timeout: Option<Duration>) -> bool {
    let timeout = timeout.and_then(|t| t.as_nanos().try_into().ok()).unwrap_or(-1);
    unsafe {
        wasm::memory_atomic_wait32(
            futex as *const Atomic<u32> as *mut i32,
            expected as i32,
            timeout,
        ) < 2
    }
}

/// 唤醒一个阻塞在此 futex 的 `futex_wait` 上的线程。
///
/// 如果确实唤醒了这样一个线程，则返回 true；
/// 如果没有线程在此 futex 上等待，则返回 false。
pub fn futex_wake(futex: &Atomic<u32>) -> bool {
    unsafe { wasm::memory_atomic_notify(futex as *const Atomic<u32> as *mut i32, 1) > 0 }
}

/// 唤醒所有正在此 futex 的 `futex_wait` 上等待的线程。
pub fn futex_wake_all(futex: &Atomic<u32>) {
    unsafe {
        wasm::memory_atomic_notify(futex as *const Atomic<u32> as *mut i32, i32::MAX as u32);
    }
}
