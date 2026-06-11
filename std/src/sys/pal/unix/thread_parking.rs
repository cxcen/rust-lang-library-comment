// 仅在 NetBSD 上使用。如果其他平台也开始使用基于 id 的 parking，
// 请为每个平台使用各自独立的模块。
#![cfg(target_os = "netbsd")]

use libc::{_lwp_park, _lwp_self, _lwp_unpark, CLOCK_MONOTONIC, c_long, lwpid_t, time_t, timespec};

use crate::ptr;
use crate::time::Duration;

pub type ThreadId = lwpid_t;

#[inline]
pub fn current() -> ThreadId {
    unsafe { _lwp_self() }
}

#[inline]
pub fn park(hint: usize) {
    unsafe {
        _lwp_park(0, 0, ptr::null_mut(), 0, ptr::without_provenance(hint), ptr::null_mut());
    }
}

pub fn park_timeout(dur: Duration, hint: usize) {
    let mut timeout = timespec {
        // 做饱和处理，使该操作一定会超时
        //（即便那是在宇宙热寂之后）。
        tv_sec: dur.as_secs().try_into().ok().unwrap_or(time_t::MAX),
        tv_nsec: dur.subsec_nanos() as c_long,
    };

    // timeout 需要是可变的，因为在 NetBSD 9.0 及以上版本它会被修改。
    unsafe {
        _lwp_park(
            CLOCK_MONOTONIC,
            0,
            &mut timeout,
            0,
            ptr::without_provenance(hint),
            ptr::null_mut(),
        );
    }
}

#[inline]
pub fn unpark(tid: ThreadId, hint: usize) {
    unsafe {
        _lwp_unpark(tid, ptr::without_provenance(hint));
    }
}
