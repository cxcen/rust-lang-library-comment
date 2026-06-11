//! 针对套接字地址的 Linux 与 Android 特有扩展。

use crate::os::unix::net::SocketAddr;
use crate::sealed::Sealed;

/// 针对 [`SocketAddr`] 的平台特有扩展。
#[stable(feature = "unix_socket_abstract", since = "1.70.0")]
pub trait SocketAddrExt: Sealed {
    /// 在抽象命名空间（abstract namespace）中创建一个 Unix 套接字地址。
    ///
    /// 抽象命名空间是 Linux 特有的扩展，它允许 Unix 套接字在不于文件系统中创建条目的
    /// 情况下进行绑定。抽象套接字不受文件系统布局或权限的影响，且在套接字关闭时
    /// 无需任何清理。
    ///
    /// 抽象套接字地址名可以包含任意字节，包括零字节。
    ///
    /// # Errors
    ///
    /// 如果名字长度超过 `SUN_LEN - 1`，则返回错误。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::os::unix::net::{UnixListener, SocketAddr};
    /// #[cfg(target_os = "linux")]
    /// use std::os::linux::net::SocketAddrExt;
    /// #[cfg(target_os = "android")]
    /// use std::os::android::net::SocketAddrExt;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let addr = SocketAddr::from_abstract_name(b"hidden")?;
    ///     let listener = match UnixListener::bind_addr(&addr) {
    ///         Ok(sock) => sock,
    ///         Err(err) => {
    ///             println!("Couldn't bind: {err:?}");
    ///             return Err(err);
    ///         }
    ///     };
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "unix_socket_abstract", since = "1.70.0")]
    fn from_abstract_name<N>(name: N) -> crate::io::Result<SocketAddr>
    where
        N: AsRef<[u8]>;

    /// 如果该地址位于抽象命名空间中，则返回其内容。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::os::unix::net::{UnixListener, SocketAddr};
    /// #[cfg(target_os = "linux")]
    /// use std::os::linux::net::SocketAddrExt;
    /// #[cfg(target_os = "android")]
    /// use std::os::android::net::SocketAddrExt;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let name = b"hidden";
    ///     let name_addr = SocketAddr::from_abstract_name(name)?;
    ///     let socket = UnixListener::bind_addr(&name_addr)?;
    ///     let local_addr = socket.local_addr().expect("Couldn't get local address");
    ///     assert_eq!(local_addr.as_abstract_name(), Some(&name[..]));
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "unix_socket_abstract", since = "1.70.0")]
    fn as_abstract_name(&self) -> Option<&[u8]>;
}
