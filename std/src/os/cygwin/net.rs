//! Cygwin 平台特有的网络功能。
//!
//! Cygwin 上的 Unix 域套接字存在一些限制：
//! * 系统调用 `accept` 和 `connect` 需要
//! [握手（handshake）](https://inbox.sourceware.org/cygwin/Z_UERXFI1g-1v3p2@calimero.vinschen.de/T/#t)。
//! * 无法绑定到抽象地址（abstract addr）。
//! * 未绑定的 unix 套接字拥有一个抽象的本地地址。
//! * 不支持携带控制数据（control data）的 recvmsg。

#![stable(feature = "unix_socket_abstract", since = "1.70.0")]

#[stable(feature = "unix_socket_abstract", since = "1.70.0")]
pub use crate::os::net::linux_ext::addr::SocketAddrExt;
#[unstable(feature = "unix_socket_ancillary_data", issue = "76915")]
pub use crate::os::net::linux_ext::socket::UnixSocketExt;
#[stable(feature = "tcp_quickack", since = "1.89.0")]
pub use crate::os::net::linux_ext::tcp::TcpStreamExt;
