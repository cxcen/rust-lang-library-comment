//! 针对 Fortanix SGX 平台的系统绑定
//!
//! 本模块包含针对 Fortanix SGX 的、对操作系统级功能的门面（即平台特定）实现。
#![deny(unsafe_op_in_unsafe_fn)]
#![allow(fuzzy_provenance_casts)] // FIXME: this entire module systematically confuses pointers and integers

use crate::io;
use crate::sync::atomic::{Atomic, AtomicBool, Ordering};

pub mod abi;
mod libunwind_integration;
pub mod os;
pub mod thread_parking;
pub mod time;
pub mod waitqueue;

// SAFETY: 只能在运行时初始化期间调用一次。
// 注意：它不保证一定会运行，例如当 Rust 代码被外部调用时。
pub unsafe fn init(argc: isize, argv: *const *const u8, _sigpipe: u8) {
    unsafe {
        crate::sys::args::init(argc, argv);
    }
}

// SAFETY: 只能在运行时清理期间调用一次。
// 注意：它不保证一定会运行，例如当程序中止（abort）时。
pub unsafe fn cleanup() {}

/// 此函数用于实现那些根本不存在的功能。依赖该功能的程序需要自行处理这个错误。
pub fn unsupported<T>() -> io::Result<T> {
    Err(unsupported_err())
}

pub fn unsupported_err() -> io::Error {
    io::const_error!(io::ErrorKind::Unsupported, "operation not supported on SGX yet")
}

/// 此函数用于实现各种并不存在、但其缺失未必构成错误理由的功能。如果不返回错误，
/// 程序很可能依然能够正常运作。当 `SGX_INEFFECTIVE_ERROR` 被设置为 `true` 时，
/// 就是这种情形。如果它为 `false`，则行为与 `unsupported` 相同。
pub fn sgx_ineffective<T>(v: T) -> io::Result<T> {
    static SGX_INEFFECTIVE_ERROR: Atomic<bool> = AtomicBool::new(false);
    if SGX_INEFFECTIVE_ERROR.load(Ordering::Relaxed) {
        Err(io::const_error!(
            io::ErrorKind::Uncategorized,
            "operation can't be trusted to have any effect on SGX",
        ))
    } else {
        Ok(v)
    }
}

pub fn abort_internal() -> ! {
    abi::usercalls::exit(true)
}

// 此函数为 libunwind 所需。该符号在目标规格（target specification）的 pre-link
// args 中被命名引用，因此请保持二者同步。
// 注意：与 `crate::rt` 中的 `__rust_abort` 不同，此处使用 `no_mangle`，
//       因为它实际上是从 C 代码中被使用的。由于标注了
//       #[rustc_std_internal_symbol] 的符号会被 mangle，这样不会导致链接器冲突。
#[cfg(not(test))]
#[unsafe(no_mangle)]
pub extern "C" fn __rust_abort() {
    abort_internal();
}

pub trait TryIntoInner<Inner>: Sized {
    fn try_into_inner(self) -> Result<Inner, Self>;
}
