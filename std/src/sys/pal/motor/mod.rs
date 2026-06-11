#![allow(unsafe_op_in_unsafe_fn)]

pub mod os;
pub mod time;

pub use moto_rt::futex;

use crate::io;

pub(crate) fn map_motor_error(err: moto_rt::Error) -> io::Error {
    let error_code: moto_rt::ErrorCode = err.into();
    io::Error::from_raw_os_error(error_code.into())
}

#[cfg(not(test))]
#[unsafe(no_mangle)]
pub extern "C" fn motor_start() -> ! {
    // 初始化运行时（runtime）。
    moto_rt::start();

    // 调用 main。
    unsafe extern "C" {
        fn main(_: isize, _: *const *const u8, _: u8) -> i32;
    }
    let result = unsafe { main(0, core::ptr::null(), 0) };

    // 终止进程。
    moto_rt::process::exit(result)
}

// SAFETY: 必须在运行时初始化期间仅调用一次。
// NOTE: Motor OS 使用 moto_rt::start() 来初始化运行时（见上文）。
pub unsafe fn init(_argc: isize, _argv: *const *const u8, _sigpipe: u8) {}

// SAFETY: 必须在运行时清理期间仅调用一次。
// NOTE: 不保证一定会被运行，例如当程序中止（abort）时。
pub unsafe fn cleanup() {}

pub fn unsupported<T>() -> io::Result<T> {
    Err(unsupported_err())
}

pub fn unsupported_err() -> io::Error {
    io::Error::UNSUPPORTED_PLATFORM
}

pub fn abort_internal() -> ! {
    core::intrinsics::abort();
}
