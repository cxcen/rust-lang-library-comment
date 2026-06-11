//! Linux 与 Android 特有的套接字功能。

use crate::io;
use crate::os::unix::net;
use crate::sealed::Sealed;
use crate::sys::AsInner;

/// 针对 `AF_UNIX` 套接字 [`UnixDatagram`] 与 [`UnixStream`] 的 Linux 特有功能。
///
/// [`UnixDatagram`]: net::UnixDatagram
/// [`UnixStream`]: net::UnixStream
#[unstable(feature = "unix_socket_ancillary_data", issue = "76915")]
pub trait UnixSocketExt: Sealed {
    /// 查询套接字选项 `SO_PASSCRED` 的当前设置。
    #[unstable(feature = "unix_socket_ancillary_data", issue = "76915")]
    fn passcred(&self) -> io::Result<bool>;

    /// 启用或禁用套接字选项 `SO_PASSCRED`。
    ///
    /// 该选项使得发送进程的凭据能够作为一条控制消息在 [`AncillaryData`] 中被接收。
    ///
    /// [`AncillaryData`]: net::AncillaryData
    ///
    /// # 示例
    ///
    /// ```no_run
    /// #![feature(unix_socket_ancillary_data)]
    /// #[cfg(target_os = "linux")]
    /// use std::os::linux::net::UnixSocketExt;
    /// #[cfg(target_os = "android")]
    /// use std::os::android::net::UnixSocketExt;
    /// use std::os::unix::net::UnixDatagram;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let sock = UnixDatagram::unbound()?;
    ///     sock.set_passcred(true).expect("set_passcred failed");
    ///     Ok(())
    /// }
    /// ```
    #[unstable(feature = "unix_socket_ancillary_data", issue = "76915")]
    fn set_passcred(&self, passcred: bool) -> io::Result<()>;
}

#[unstable(feature = "unix_socket_ancillary_data", issue = "76915")]
impl UnixSocketExt for net::UnixDatagram {
    fn passcred(&self) -> io::Result<bool> {
        self.as_inner().passcred()
    }

    fn set_passcred(&self, passcred: bool) -> io::Result<()> {
        self.as_inner().set_passcred(passcred)
    }
}

#[unstable(feature = "unix_socket_ancillary_data", issue = "76915")]
impl UnixSocketExt for net::UnixStream {
    fn passcred(&self) -> io::Result<bool> {
        self.as_inner().passcred()
    }

    fn set_passcred(&self, passcred: bool) -> io::Result<()> {
        self.as_inner().set_passcred(passcred)
    }
}
