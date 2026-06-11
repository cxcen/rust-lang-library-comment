//! 本模块中的函数为 libunwind 所需。这些符号在目标规格（target specification）的
//! pre-link args 中被命名引用，因此请保持二者同步。

#![cfg(not(test))]

use crate::sys::sync::RwLock;
use crate::{slice, str};

// 验证 libunwind 用来初始化 RwLock 的字节模式（byte pattern）与 RwLock::new()
// 的值是否等价。如果该值发生变化，libunwind 中的 `src/UnwindRustSgx.h` 也需要
// 相应修改。
const _: () = unsafe {
    let bits_rust: usize = crate::mem::transmute(RwLock::new());
    assert!(bits_rust == 0);
};

const EINVAL: i32 = 22;

#[unsafe(no_mangle)]
pub unsafe extern "C" fn __rust_rwlock_rdlock(p: *mut RwLock) -> i32 {
    if p.is_null() {
        return EINVAL;
    }

    // 在 unlock 中我们无法区分读与写，因此总是使用写锁（write-lock）。反正
    // 栈展开（unwinding）本来也不在热点路径上。
    unsafe { (*p).write() };
    return 0;
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn __rust_rwlock_wrlock(p: *mut RwLock) -> i32 {
    if p.is_null() {
        return EINVAL;
    }
    unsafe { (*p).write() };
    return 0;
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn __rust_rwlock_unlock(p: *mut RwLock) -> i32 {
    if p.is_null() {
        return EINVAL;
    }
    unsafe { (*p).write_unlock() };
    return 0;
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn __rust_print_err(m: *mut u8, s: i32) {
    if s < 0 {
        return;
    }
    let buf = unsafe { slice::from_raw_parts(m as *const u8, s as _) };
    if let Ok(s) = str::from_utf8(&buf[..buf.iter().position(|&b| b == 0).unwrap_or(buf.len())]) {
        eprint!("{s}");
    }
}
