//! WASI 平台特定的网络功能

#![unstable(feature = "wasi_ext", issue = "71213")]

use crate::os::fd::AsRawFd;
use crate::sys::err2io;
use crate::{io, net};

/// 针对 [`std::net::TcpListener`] 的 WASI 平台特定扩展。
///
/// [`std::net::TcpListener`]: crate::net::TcpListener
pub trait TcpListenerExt {
    /// 接受一个套接字连接。
    ///
    /// 对应 `sock_accept` 系统调用。
    fn sock_accept(&self, flags: u16) -> io::Result<u32>;
}

impl TcpListenerExt for net::TcpListener {
    fn sock_accept(&self, flags: u16) -> io::Result<u32> {
        unsafe { wasi::sock_accept(self.as_raw_fd() as wasi::Fd, flags).map_err(err2io) }
    }
}
