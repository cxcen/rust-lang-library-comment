#![cfg(any(
    target_os = "linux",
    target_os = "android",
    all(target_os = "emscripten", target_feature = "atomics"),
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "dragonfly",
    target_os = "fuchsia",
))]

use crate::sync::atomic::Atomic;
use crate::time::Duration;

/// 用作 futex 的原子类型，至少 32 位，但可能更大
pub type Futex = Atomic<Primitive>;
/// 必须是 Futex 的底层类型
pub type Primitive = u32;

/// 用作 futex 的原子类型，至少 8 位，但可能更大。
pub type SmallFutex = Atomic<SmallPrimitive>;
/// 必须是 SmallFutex 的底层类型
pub type SmallPrimitive = u32;

/// 等待一次 `futex_wake` 操作来唤醒我们。
///
/// 如果 futex 没有持有期望的值，则直接返回。
///
/// 超时返回 false，其他所有情况返回 true。
#[cfg(any(target_os = "linux", target_os = "android", target_os = "freebsd"))]
pub fn futex_wait(futex: &Atomic<u32>, expected: u32, timeout: Option<Duration>) -> bool {
    use super::time::Timespec;
    use crate::ptr::null;
    use crate::sync::atomic::Ordering::Relaxed;

    // 把超时计算成一个绝对的 timespec。
    //
    // 溢出会被向上取整为无限超时（None）。
    let timespec = timeout
        .and_then(|d| Timespec::now(libc::CLOCK_MONOTONIC).checked_add_duration(&d))
        .and_then(|t| t.to_timespec());

    loop {
        // 如果值已经改变，就无需等待。
        if futex.load(Relaxed) != expected {
            return true;
        }

        let r = unsafe {
            cfg_select! {
                target_os = "freebsd" => {
                    // FreeBSD 没有 futex()，但它有
                    // _umtx_op(UMTX_OP_WAIT_UINT_PRIVATE)，二者几乎完全相同。
                    // 它通过 _umtx_time 结构体里的一个标志位支持绝对超时。
                    let umtx_timeout = timespec.map(|t| libc::_umtx_time {
                        _timeout: t,
                        _flags: libc::UMTX_ABSTIME,
                        _clockid: libc::CLOCK_MONOTONIC as u32,
                    });
                    let umtx_timeout_ptr = umtx_timeout.as_ref().map_or(null(), |t| t as *const _);
                    let umtx_timeout_size = umtx_timeout.as_ref().map_or(0, |t| size_of_val(t));
                    libc::_umtx_op(
                        futex as *const Atomic<u32> as *mut _,
                        libc::UMTX_OP_WAIT_UINT_PRIVATE,
                        expected as libc::c_ulong,
                        crate::ptr::without_provenance_mut(umtx_timeout_size),
                        umtx_timeout_ptr as *mut _,
                    )
                }
                any(target_os = "linux", target_os = "android") => {
                    // 使用 FUTEX_WAIT_BITSET 而非 FUTEX_WAIT，以便能够给出
                    // 绝对时间而非相对时间。
                    libc::syscall(
                        libc::SYS_futex,
                        futex as *const Atomic<u32>,
                        libc::FUTEX_WAIT_BITSET | libc::FUTEX_PRIVATE_FLAG,
                        expected,
                        timespec.as_ref().map_or(null(), |t| t as *const libc::timespec),
                        null::<u32>(), // 该参数对 FUTEX_WAIT_BITSET 未使用。
                        !0u32,         // 一个全 1 的位掩码，使其行为与普通的 FUTEX_WAIT 相同。
                    )
                }
                _ => {
                    compile_error!("unknown target_os");
                }
            }
        };

        match (r < 0).then(crate::sys::io::errno) {
            Some(libc::ETIMEDOUT) => return false,
            Some(libc::EINTR) => continue,
            _ => return true,
        }
    }
}

/// 唤醒一个阻塞在该 futex 上 `futex_wait` 的线程。
///
/// 如果确实唤醒了这样一个线程则返回 true，
/// 如果没有线程在该 futex 上等待则返回 false。
///
/// 在某些平台上，它始终返回 false。
#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn futex_wake(futex: &Atomic<u32>) -> bool {
    let ptr = futex as *const Atomic<u32>;
    let op = libc::FUTEX_WAKE | libc::FUTEX_PRIVATE_FLAG;
    unsafe { libc::syscall(libc::SYS_futex, ptr, op, 1) > 0 }
}

/// 唤醒所有在该 futex 上 `futex_wait` 等待的线程。
#[cfg(any(target_os = "linux", target_os = "android"))]
pub fn futex_wake_all(futex: &Atomic<u32>) {
    let ptr = futex as *const Atomic<u32>;
    let op = libc::FUTEX_WAKE | libc::FUTEX_PRIVATE_FLAG;
    unsafe {
        libc::syscall(libc::SYS_futex, ptr, op, i32::MAX);
    }
}

// FreeBSD 不会告诉我们唤醒了多少个线程，因此它始终返回 false。
#[cfg(target_os = "freebsd")]
pub fn futex_wake(futex: &Atomic<u32>) -> bool {
    use crate::ptr::null_mut;
    unsafe {
        libc::_umtx_op(
            futex as *const Atomic<u32> as *mut _,
            libc::UMTX_OP_WAKE_PRIVATE,
            1,
            null_mut(),
            null_mut(),
        )
    };
    false
}

#[cfg(target_os = "freebsd")]
pub fn futex_wake_all(futex: &Atomic<u32>) {
    use crate::ptr::null_mut;
    unsafe {
        libc::_umtx_op(
            futex as *const Atomic<u32> as *mut _,
            libc::UMTX_OP_WAKE_PRIVATE,
            i32::MAX as libc::c_ulong,
            null_mut(),
            null_mut(),
        )
    };
}

#[cfg(target_os = "openbsd")]
pub fn futex_wait(futex: &Atomic<u32>, expected: u32, timeout: Option<Duration>) -> bool {
    use super::time::Timespec;
    use crate::ptr::{null, null_mut};

    // 溢出会被向上取整为无限超时（None）。
    let timespec = timeout
        .and_then(|d| Timespec::zero().checked_add_duration(&d))
        .and_then(|t| t.to_timespec());

    let r = unsafe {
        libc::futex(
            futex as *const Atomic<u32> as *mut u32,
            libc::FUTEX_WAIT,
            expected as i32,
            timespec.as_ref().map_or(null(), |t| t as *const libc::timespec),
            null_mut(),
        )
    };

    r == 0 || crate::sys::io::errno() != libc::ETIMEDOUT
}

#[cfg(target_os = "openbsd")]
pub fn futex_wake(futex: &Atomic<u32>) -> bool {
    use crate::ptr::{null, null_mut};
    unsafe {
        libc::futex(
            futex as *const Atomic<u32> as *mut u32,
            libc::FUTEX_WAKE,
            1,
            null(),
            null_mut(),
        ) > 0
    }
}

#[cfg(target_os = "openbsd")]
pub fn futex_wake_all(futex: &Atomic<u32>) {
    use crate::ptr::{null, null_mut};
    unsafe {
        libc::futex(
            futex as *const Atomic<u32> as *mut u32,
            libc::FUTEX_WAKE,
            i32::MAX,
            null(),
            null_mut(),
        );
    }
}

#[cfg(target_os = "dragonfly")]
pub fn futex_wait(futex: &Atomic<u32>, expected: u32, timeout: Option<Duration>) -> bool {
    // 超时为 0 表示无限。
    // 我们把更小的超时向上取整为 1 毫秒。
    // 溢出会被向上取整为无限超时。
    let timeout_ms =
        timeout.and_then(|d| Some(i32::try_from(d.as_millis()).ok()?.max(1))).unwrap_or(0);

    let r = unsafe {
        libc::umtx_sleep(futex as *const Atomic<u32> as *const i32, expected as i32, timeout_ms)
    };

    r == 0 || crate::sys::io::errno() != libc::ETIMEDOUT
}

// DragonflyBSD 不会告诉我们唤醒了多少个线程，因此它始终返回 false。
#[cfg(target_os = "dragonfly")]
pub fn futex_wake(futex: &Atomic<u32>) -> bool {
    unsafe { libc::umtx_wakeup(futex as *const Atomic<u32> as *const i32, 1) };
    false
}

#[cfg(target_os = "dragonfly")]
pub fn futex_wake_all(futex: &Atomic<u32>) {
    unsafe { libc::umtx_wakeup(futex as *const Atomic<u32> as *const i32, i32::MAX) };
}

#[cfg(target_os = "emscripten")]
unsafe extern "C" {
    fn emscripten_futex_wake(addr: *const Atomic<u32>, count: libc::c_int) -> libc::c_int;
    fn emscripten_futex_wait(
        addr: *const Atomic<u32>,
        val: libc::c_uint,
        max_wait_ms: libc::c_double,
    ) -> libc::c_int;
}

#[cfg(target_os = "emscripten")]
pub fn futex_wait(futex: &Atomic<u32>, expected: u32, timeout: Option<Duration>) -> bool {
    unsafe {
        emscripten_futex_wait(
            futex,
            expected,
            timeout.map_or(f64::INFINITY, |d| d.as_secs_f64() * 1000.0),
        ) != -libc::ETIMEDOUT
    }
}

#[cfg(target_os = "emscripten")]
pub fn futex_wake(futex: &Atomic<u32>) -> bool {
    unsafe { emscripten_futex_wake(futex, 1) > 0 }
}

#[cfg(target_os = "emscripten")]
pub fn futex_wake_all(futex: &Atomic<u32>) {
    unsafe { emscripten_futex_wake(futex, i32::MAX) };
}

#[cfg(target_os = "fuchsia")]
pub fn futex_wait(futex: &Atomic<u32>, expected: u32, timeout: Option<Duration>) -> bool {
    use super::fuchsia::*;

    // 如果超时比 i64 能容纳的还长，就永久休眠。
    let deadline = timeout
        .and_then(|d| i64::try_from(d.as_nanos()).ok()?.checked_add(zx_clock_get_monotonic()))
        .unwrap_or(ZX_TIME_INFINITE);

    unsafe {
        zx_futex_wait(futex, zx_futex_t::new(expected), ZX_HANDLE_INVALID, deadline)
            != ZX_ERR_TIMED_OUT
    }
}

// Fuchsia 不会告诉我们唤醒了多少个线程，因此它始终返回 false。
#[cfg(target_os = "fuchsia")]
pub fn futex_wake(futex: &Atomic<u32>) -> bool {
    unsafe { super::fuchsia::zx_futex_wake(futex, 1) };
    false
}

#[cfg(target_os = "fuchsia")]
pub fn futex_wake_all(futex: &Atomic<u32>) {
    unsafe { super::fuchsia::zx_futex_wake(futex, u32::MAX) };
}
