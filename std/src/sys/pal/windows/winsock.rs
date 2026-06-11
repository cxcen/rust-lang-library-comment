use super::c;
use crate::ffi::c_int;
use crate::sync::atomic::Atomic;
use crate::sync::atomic::Ordering::{AcqRel, Relaxed};
use crate::{io, mem};

static WSA_STARTED: Atomic<bool> = Atomic::<bool>::new(false);

/// 检查 Windows 套接字接口是否已经启动，如果尚未启动，则将其启动。
#[inline]
pub fn startup() {
    if !WSA_STARTED.load(Relaxed) {
        wsa_startup();
    }
}

#[cold]
fn wsa_startup() {
    unsafe {
        let mut data: c::WSADATA = mem::zeroed();
        let ret = c::WSAStartup(
            0x202, // 版本 2.2
            &mut data,
        );
        assert_eq!(ret, 0);
        if WSA_STARTED.swap(true, AcqRel) {
            // 如果另一个线程与我们发生竞争并先调用了 WSAStartup，则调用
            // WSACleanup，使得效果如同 WSAStartup 只被调用了一次。
            c::WSACleanup();
        }
    }
}

pub fn cleanup() {
    // 这里不需要调用 WSACleanup，因为进程退出会让操作系统替我们清理一切，
    // 这比手动清理更快。
    // 参见 #141799。
}

/// 返回 Windows 套接字接口的最近一次错误。
pub fn last_error() -> io::Error {
    io::Error::from_raw_os_error(unsafe { c::WSAGetLastError() })
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

/// 检查该有符号整数是否为 Windows 常量 `SOCKET_ERROR`（-1），如果是，
/// 则返回 Windows 套接字接口的最近一次错误。必须在再次调用套接字 API 之前
/// 调用本函数。
pub fn cvt<T: IsMinusOne>(t: T) -> io::Result<T> {
    if t.is_minus_one() { Err(last_error()) } else { Ok(t) }
}

/// `cvt` 的一个变体，用于 `getaddrinfo`，它在成功时返回 0。
pub fn cvt_gai(err: c_int) -> io::Result<()> {
    if err == 0 { Ok(()) } else { Err(last_error()) }
}

/// 仅仅是为了提供与 sys/pal/unix/net.rs 相同的接口
pub fn cvt_r<T, F>(mut f: F) -> io::Result<T>
where
    T: IsMinusOne,
    F: FnMut() -> T,
{
    cvt(f())
}
