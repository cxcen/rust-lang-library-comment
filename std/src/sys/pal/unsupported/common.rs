use crate::io as std_io;

// SAFETY: 必须在运行时初始化期间仅调用一次。
// NOTE: 不保证一定会被运行，例如当 Rust 代码被外部调用时。
pub unsafe fn init(_argc: isize, _argv: *const *const u8, _sigpipe: u8) {}

// SAFETY: 必须在运行时清理期间仅调用一次。
// NOTE: 不保证一定会被运行，例如当程序中止（abort）时。
pub unsafe fn cleanup() {}

pub fn unsupported<T>() -> std_io::Result<T> {
    Err(unsupported_err())
}

pub fn unsupported_err() -> std_io::Error {
    std_io::Error::UNSUPPORTED_PLATFORM
}

pub fn abort_internal() -> ! {
    core::intrinsics::abort();
}
