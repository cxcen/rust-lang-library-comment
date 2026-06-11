//! Teeos 平台的系统绑定（System bindings）
//!
//! 本模块包含 Teeos 上 OS 级功能的门面（facade，即平台特定）实现。
#![deny(unsafe_op_in_unsafe_fn)]
#![allow(unused_variables)]
#![allow(dead_code)]

pub mod os;
#[allow(non_upper_case_globals)]
#[path = "../unix/time.rs"]
pub mod time;

#[path = "../unix/sync"]
pub mod sync {
    mod condvar;
    mod mutex;
    pub use condvar::Condvar;
    pub use mutex::Mutex;
}

use crate::io;

pub fn abort_internal() -> ! {
    unsafe { libc::abort() }
}

// 在 Teeos 上，可信应用（Trusted Application）是作为动态库加载的，
// 因此这个函数永远不应该被调用。
pub fn init(argc: isize, argv: *const *const u8, sigpipe: u8) {}

// SAFETY: 必须在运行时清理期间仅调用一次。
// 不保证一定会被运行，例如当程序中止（abort）时。
pub unsafe fn cleanup() {
    unimplemented!()
    // 我们没有栈溢出（stack overflow）处理器，因为当它发生时 TEE OS 会直接杀死 TA。
    // 因此 cleanup 被注释掉了
    // stack_overflow::cleanup();
}

#[doc(hidden)]
pub trait IsMinusOne {
    fn is_minus_one(&self) -> bool;
}

macro_rules! impl_is_minus_one {
    ($($t:ident)*) => ($(impl IsMinusOne for $t {
        fn is_minus_one(&self) -> bool {
            *self == -1
        }
    })*)
}

impl_is_minus_one! { i8 i16 i32 i64 isize }

pub fn cvt<T: IsMinusOne>(t: T) -> io::Result<T> {
    if t.is_minus_one() { Err(io::Error::last_os_error()) } else { Ok(t) }
}

pub fn cvt_r<T, F>(mut f: F) -> io::Result<T>
where
    T: IsMinusOne,
    F: FnMut() -> T,
{
    loop {
        match cvt(f()) {
            Err(ref e) if e.kind() == io::ErrorKind::Interrupted => {}
            other => return other,
        }
    }
}

pub fn cvt_nz(error: libc::c_int) -> io::Result<()> {
    if error == 0 { Ok(()) } else { Err(io::Error::from_raw_os_error(error)) }
}

pub fn unsupported<T>() -> io::Result<T> {
    Err(unsupported_err())
}

pub fn unsupported_err() -> io::Error {
    io::Error::UNSUPPORTED_PLATFORM
}
