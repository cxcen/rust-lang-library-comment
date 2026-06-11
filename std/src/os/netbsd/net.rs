//! NetBSD 平台特有的网络功能。

#![unstable(feature = "unix_socket_ancillary_data", issue = "76915")]

use crate::ffi::CStr;
use crate::io;
use crate::os::unix::net;
use crate::sealed::Sealed;
use crate::sys::AsInner;

/// 针对 `AF_UNIX` 套接字 [`UnixDatagram`] 与 [`UnixStream`] 的
/// NetBSD 平台特有功能。
///
/// [`UnixDatagram`]: net::UnixDatagram
/// [`UnixStream`]: net::UnixStream
#[unstable(feature = "unix_socket_ancillary_data", issue = "76915")]
pub trait UnixSocketExt: Sealed {
    /// 查询套接字选项 `LOCAL_CREDS` 的当前设置。
    #[unstable(feature = "unix_socket_ancillary_data", issue = "76915")]
    fn local_creds(&self) -> io::Result<bool>;

    /// 启用或禁用套接字选项 `LOCAL_CREDS`。
    ///
    /// 该选项使得发送方进程的凭据（credentials）能够作为控制消息（control message）
    /// 在 [`AncillaryData`] 中被接收。
    ///
    /// [`AncillaryData`]: net::AncillaryData
    ///
    /// # 示例
    ///
    /// ```no_run
    /// #![feature(unix_socket_ancillary_data)]
    /// use std::os::netbsd::net::UnixSocketExt;
    /// use std::os::unix::net::UnixDatagram;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let sock = UnixDatagram::unbound()?;
    ///     sock.set_local_creds(true).expect("set_local_creds failed");
    ///     Ok(())
    /// }
    /// ```
    #[unstable(feature = "unix_socket_ancillary_data", issue = "76915")]
    fn set_local_creds(&self, local_creds: bool) -> io::Result<()>;

    /// 获取此前若在套接字上设置过的过滤器（filter）名称。
    #[unstable(feature = "acceptfilter", issue = "121891")]
    fn acceptfilter(&self) -> io::Result<&CStr>;

    /// 在套接字上设置或禁用过滤器，用于过滤传入连接，
    /// 以便在 accept(2) 之前对其进行延迟（defer）处理
    #[unstable(feature = "acceptfilter", issue = "121891")]
    fn set_acceptfilter(&self, name: &CStr) -> io::Result<()>;
}

#[unstable(feature = "unix_socket_ancillary_data", issue = "76915")]
impl UnixSocketExt for net::UnixDatagram {
    fn local_creds(&self) -> io::Result<bool> {
        self.as_inner().local_creds()
    }

    fn set_local_creds(&self, local_creds: bool) -> io::Result<()> {
        self.as_inner().set_local_creds(local_creds)
    }

    fn acceptfilter(&self) -> io::Result<&CStr> {
        self.as_inner().acceptfilter()
    }

    fn set_acceptfilter(&self, name: &CStr) -> io::Result<()> {
        self.as_inner().set_acceptfilter(name)
    }
}

#[unstable(feature = "unix_socket_ancillary_data", issue = "76915")]
impl UnixSocketExt for net::UnixStream {
    fn local_creds(&self) -> io::Result<bool> {
        self.as_inner().local_creds()
    }

    fn set_local_creds(&self, local_creds: bool) -> io::Result<()> {
        self.as_inner().set_local_creds(local_creds)
    }

    fn acceptfilter(&self) -> io::Result<&CStr> {
        self.as_inner().acceptfilter()
    }

    fn set_acceptfilter(&self, name: &CStr) -> io::Result<()> {
        self.as_inner().set_acceptfilter(name)
    }
}
