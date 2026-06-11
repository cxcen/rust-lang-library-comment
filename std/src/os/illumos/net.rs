//! illumos 平台特有的网络功能。

#![unstable(feature = "unix_socket_exclbind", issue = "123481")]

use crate::io;
use crate::os::unix::net;
use crate::sealed::Sealed;
use crate::sys::AsInner;

/// 针对 `AF_UNIX` 套接字 [`UnixDatagram`] 与 [`UnixStream`] 的
/// illumos 平台特有功能。
///
/// [`UnixDatagram`]: net::UnixDatagram
/// [`UnixStream`]: net::UnixStream
#[unstable(feature = "unix_socket_exclbind", issue = "123481")]
pub trait UnixSocketExt: Sealed {
    /// 启用套接字上的独占绑定（exclusive binding）。
    ///
    /// 如果设为 true，并且该套接字此前设置过 `SO_REUSEADDR`，
    /// 则会抵消（neutralise）其效果。
    /// 参阅 [`man 3 tcp`](https://docs.oracle.com/cd/E88353_01/html/E37843/setsockopt-3c.html)
    #[unstable(feature = "unix_socket_exclbind", issue = "123481")]
    fn so_exclbind(&self, excl: bool) -> io::Result<()>;

    /// 获取套接字的独占绑定（bind exclusivity）状态。
    #[unstable(feature = "unix_socket_exclbind", issue = "123481")]
    fn exclbind(&self) -> io::Result<bool>;
}

#[unstable(feature = "unix_socket_exclbind", issue = "123481")]
impl UnixSocketExt for net::UnixDatagram {
    fn exclbind(&self) -> io::Result<bool> {
        self.as_inner().exclbind()
    }

    fn so_exclbind(&self, excl: bool) -> io::Result<()> {
        self.as_inner().set_exclbind(excl)
    }
}

#[unstable(feature = "unix_socket_exclbind", issue = "123481")]
impl UnixSocketExt for net::UnixStream {
    fn exclbind(&self) -> io::Result<bool> {
        self.as_inner().exclbind()
    }

    fn so_exclbind(&self, excl: bool) -> io::Result<()> {
        self.as_inner().set_exclbind(excl)
    }
}
