//! HermitCore 的系统绑定（System bindings）
//!
//! 本模块包含 HermitCore 上 OS 级功能的门面（facade，即平台特定）实现。
//!
//! 这一切都还处于高度实验性阶段，目前并不真正打算用于广泛/生产环境，
//! 仍完全属于实验性范畴。随着时间推移，这些很可能会发生变化。
//!
//! 目前这里的所有函数基本上都是立即返回错误的桩（stub）。我们希望借助一个
//! 可移植性 lint，实际上可以直接移除所有这些代码：当我们为 wasm 编译时，
//! 干脆省略标准库中的相应部分。这样一来，对于那些注定会在运行时出错的东西，
//! 就能变成编译期错误！

#![deny(unsafe_op_in_unsafe_fn)]
#![allow(missing_docs, nonstandard_style)]

use crate::io;
use crate::os::hermit::hermit_abi;
use crate::os::raw::c_char;
use crate::sys::env;

pub mod futex;
pub mod os;
pub mod time;

pub fn unsupported<T>() -> io::Result<T> {
    Err(unsupported_err())
}

pub fn unsupported_err() -> io::Error {
    io::const_error!(io::ErrorKind::Unsupported, "operation not supported on HermitCore yet")
}

pub fn abort_internal() -> ! {
    unsafe { hermit_abi::abort() }
}

// SAFETY: 必须在运行时初始化期间仅调用一次。
// NOTE: 不保证一定会被运行，例如当 Rust 代码被外部调用时。
pub unsafe fn init(argc: isize, argv: *const *const u8, _sigpipe: u8) {
    unsafe {
        crate::sys::args::init(argc, argv);
    }
}

// SAFETY: 必须在运行时清理期间仅调用一次。
// NOTE: 不保证一定会被运行，例如当程序中止（abort）时。
pub unsafe fn cleanup() {}

#[cfg(not(test))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn runtime_entry(
    argc: i32,
    argv: *const *const c_char,
    env: *const *const c_char,
) -> ! {
    unsafe extern "C" {
        fn main(argc: isize, argv: *const *const c_char) -> i32;
    }

    // 初始化环境变量
    env::init(env);

    let result = unsafe { main(argc as isize, argv) };

    unsafe {
        crate::sys::thread_local::destructors::run();
    }
    crate::rt::thread_cleanup();

    unsafe {
        hermit_abi::exit(result);
    }
}

#[doc(hidden)]
pub trait IsNegative {
    fn is_negative(&self) -> bool;
    fn negate(&self) -> i32;
}

macro_rules! impl_is_negative {
    ($($t:ident)*) => ($(impl IsNegative for $t {
        fn is_negative(&self) -> bool {
            *self < 0
        }

        fn negate(&self) -> i32 {
            i32::try_from(-(*self)).unwrap()
        }
    })*)
}

impl IsNegative for i32 {
    fn is_negative(&self) -> bool {
        *self < 0
    }

    fn negate(&self) -> i32 {
        -(*self)
    }
}
impl_is_negative! { i8 i16 i64 isize }

pub fn cvt<T: IsNegative>(t: T) -> io::Result<T> {
    if t.is_negative() { Err(io::Error::from_raw_os_error(t.negate())) } else { Ok(t) }
}

pub fn cvt_r<T, F>(mut f: F) -> io::Result<T>
where
    T: IsNegative,
    F: FnMut() -> T,
{
    loop {
        match cvt(f()) {
            Err(ref e) if e.is_interrupted() => {}
            other => return other,
        }
    }
}
