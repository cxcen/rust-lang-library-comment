#![allow(dead_code)]
#![allow(missing_docs, nonstandard_style)]
#![forbid(unsafe_op_in_unsafe_fn)]

use crate::io;

pub mod abi;

#[path = "../itron"]
pub mod itron {
    pub mod abi;
    pub mod error;
    pub mod spin;
    pub mod task;
    pub mod thread_parking;
    pub mod time;
}

// `error` 是 `pub(crate)`，以便 `itron/error.rs` 能以
// `crate::sys::error` 的方式访问它
pub(crate) mod error;
pub mod os;
pub use self::itron::thread_parking;
pub mod time;

// SAFETY: 只能在运行时初始化期间调用一次。
// 注意：它不保证一定会运行，例如当 Rust 代码被外部调用时。
pub unsafe fn init(_argc: isize, _argv: *const *const u8, _sigpipe: u8) {}

// SAFETY: 只能在运行时清理期间调用一次。
pub unsafe fn cleanup() {}

pub fn unsupported<T>() -> io::Result<T> {
    Err(unsupported_err())
}

pub fn unsupported_err() -> io::Error {
    io::Error::UNSUPPORTED_PLATFORM
}

#[inline]
pub fn abort_internal() -> ! {
    unsafe { libc::abort() }
}
