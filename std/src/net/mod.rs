//! 用于 TCP/UDP 通信的网络原语。
//!
//! 本模块提供面向传输控制协议（Transmission Control Protocol，TCP）与用户数据报
//! 协议（User Datagram Protocol，UDP）的网络功能，同时提供表示 IP 地址、套接字地址
//! 的类型，以及与网络属性相关的函数。
//!
//! # 模块组织(Organization）
//!
//! * [`TcpListener`] 与 [`TcpStream`] 提供基于 TCP 进行通信的功能
//! * [`UdpSocket`] 提供基于 UDP 进行通信的功能
//! * [`IpAddr`] 表示 IPv4 或 IPv6 的 IP 地址；[`Ipv4Addr`] 与
//!   [`Ipv6Addr`] 分别表示 IPv4 与 IPv6 地址
//! * [`SocketAddr`] 表示 IPv4 或 IPv6 的套接字地址；[`SocketAddrV4`]
//!   与 [`SocketAddrV6`] 分别表示 IPv4 与 IPv6 套接字地址
//! * [`ToSocketAddrs`] 是一个 trait，用于在与 [`TcpListener`]、[`TcpStream`] 或
//!   [`UdpSocket`] 等网络对象交互时进行泛型化的地址解析
//! * 其余类型则是本模块中各种方法的返回类型或参数类型
//!
//! 在可能的情况下，Rust 默认禁止套接字对象被子进程继承。例如，在 UNIX 系统上
//! 通过 `CLOEXEC` 标志、在 Windows 上通过 `HANDLE_FLAG_INHERIT` 标志来实现。

#![stable(feature = "rust1", since = "1.0.0")]

#[stable(feature = "rust1", since = "1.0.0")]
pub use core::net::AddrParseError;

#[unstable(feature = "gethostname", issue = "135142")]
pub use self::hostname::hostname;
#[stable(feature = "rust1", since = "1.0.0")]
pub use self::ip_addr::{IpAddr, Ipv4Addr, Ipv6Addr, Ipv6MulticastScope};
#[stable(feature = "rust1", since = "1.0.0")]
pub use self::socket_addr::{SocketAddr, SocketAddrV4, SocketAddrV6, ToSocketAddrs};
#[unstable(feature = "tcplistener_into_incoming", issue = "88373")]
pub use self::tcp::IntoIncoming;
#[stable(feature = "rust1", since = "1.0.0")]
pub use self::tcp::{Incoming, TcpListener, TcpStream};
#[stable(feature = "rust1", since = "1.0.0")]
pub use self::udp::UdpSocket;

mod hostname;
mod ip_addr;
mod socket_addr;
mod tcp;
#[cfg(test)]
pub(crate) mod test;
mod udp;

/// 可以传递给 [`TcpStream::shutdown`] 方法的取值集合。
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
#[stable(feature = "rust1", since = "1.0.0")]
pub enum Shutdown {
    /// 应当关闭 [`TcpStream`] 的读取部分（半关闭读方向）。
    ///
    /// 所有当前阻塞中的以及将来的 [reads] 都会返回 <code>[Ok]\(0)</code>。
    ///
    /// [reads]: crate::io::Read "io::Read"
    #[stable(feature = "rust1", since = "1.0.0")]
    Read,
    /// 应当关闭 [`TcpStream`] 的写入部分（半关闭写方向）。
    ///
    /// 所有当前阻塞中的以及将来的 [writes] 都会返回一个错误。
    ///
    /// [writes]: crate::io::Write "io::Write"
    #[stable(feature = "rust1", since = "1.0.0")]
    Write,
    /// 应当同时关闭 [`TcpStream`] 的读取部分与写入部分。
    ///
    /// 更多信息参见 [`Shutdown::Read`] 与 [`Shutdown::Write`]。
    #[stable(feature = "rust1", since = "1.0.0")]
    Both,
}
