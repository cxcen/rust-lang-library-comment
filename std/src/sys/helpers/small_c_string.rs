use crate::ffi::{CStr, CString};
use crate::mem::MaybeUninit;
use crate::path::Path;
use crate::{io, ptr, slice};

// 务必保持在 4096 以下，以免编译器插入探测栈帧（probe frame）：
// https://docs.rs/compiler_builtins/latest/compiler_builtins/probestack/index.html
#[cfg(not(target_os = "espidf"))]
const MAX_STACK_ALLOCATION: usize = 384;
#[cfg(target_os = "espidf")]
const MAX_STACK_ALLOCATION: usize = 32;

const NUL_ERR: io::Error =
    io::const_error!(io::ErrorKind::InvalidInput, "file name contained an unexpected NUL byte");

#[inline]
pub fn run_path_with_cstr<T>(path: &Path, f: &dyn Fn(&CStr) -> io::Result<T>) -> io::Result<T> {
    run_with_cstr(path.as_os_str().as_encoded_bytes(), f)
}

#[inline]
pub fn run_with_cstr<T>(bytes: &[u8], f: &dyn Fn(&CStr) -> io::Result<T>) -> io::Result<T> {
    // 对闭包类型进行分发并做 dyn 擦除，以避免单态化膨胀（mono bloat）。
    // 参见 https://github.com/rust-lang/rust/pull/121101。
    if bytes.len() >= MAX_STACK_ALLOCATION {
        run_with_cstr_allocating(bytes, f)
    } else {
        unsafe { run_with_cstr_stack(bytes, f) }
    }
}

/// # 安全性(Safety）
///
/// `bytes` 的长度必须小于 `MAX_STACK_ALLOCATION`。
unsafe fn run_with_cstr_stack<T>(
    bytes: &[u8],
    f: &dyn Fn(&CStr) -> io::Result<T>,
) -> io::Result<T> {
    let mut buf = MaybeUninit::<[u8; MAX_STACK_ALLOCATION]>::uninit();
    let buf_ptr = buf.as_mut_ptr() as *mut u8;

    unsafe {
        ptr::copy_nonoverlapping(bytes.as_ptr(), buf_ptr, bytes.len());
        buf_ptr.add(bytes.len()).write(0);
    }

    match CStr::from_bytes_with_nul(unsafe { slice::from_raw_parts(buf_ptr, bytes.len() + 1) }) {
        Ok(s) => f(s),
        Err(_) => Err(NUL_ERR),
    }
}

#[cold]
#[inline(never)]
fn run_with_cstr_allocating<T>(bytes: &[u8], f: &dyn Fn(&CStr) -> io::Result<T>) -> io::Result<T> {
    match CString::new(bytes) {
        Ok(s) => f(&s),
        Err(_) => Err(NUL_ERR),
    }
}
