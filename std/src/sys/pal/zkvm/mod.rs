//! risc0 zkvm 平台的系统绑定（System bindings）
//!
//! 本模块包含 zkvm 上 OS 级功能的门面（facade，即平台特定）实现。
//!
//! 这一切都还处于高度实验性阶段，目前并不真正打算用于广泛/生产环境，
//! 仍完全属于实验性范畴。随着时间推移，这些很可能会发生变化。
#![forbid(unsafe_op_in_unsafe_fn)]

pub const WORD_SIZE: usize = size_of::<u32>();

pub mod abi;
pub mod os;
#[path = "../unsupported/time.rs"]
pub mod time;

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
