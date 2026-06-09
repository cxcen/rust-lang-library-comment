//! 用于 IP 通信的网络基础类型。
//!
//! 本模块提供 IP 地址和 socket 地址相关的类型。
//!
//! # 组织结构
//!
//! * [`IpAddr`] 表示 IPv4 或 IPv6 IP 地址；[`Ipv4Addr`] 和 [`Ipv6Addr`]
//!   分别表示 IPv4 地址和 IPv6 地址。
//! * [`SocketAddr`] 表示 IPv4 或 IPv6 socket 地址；[`SocketAddrV4`]
//!   和 [`SocketAddrV6`] 分别表示 IPv4 socket 地址和 IPv6 socket 地址。

#![stable(feature = "ip_in_core", since = "1.77.0")]

#[stable(feature = "rust1", since = "1.0.0")]
pub use self::ip_addr::{IpAddr, Ipv4Addr, Ipv6Addr, Ipv6MulticastScope};
#[stable(feature = "rust1", since = "1.0.0")]
pub use self::parser::AddrParseError;
#[stable(feature = "rust1", since = "1.0.0")]
pub use self::socket_addr::{SocketAddr, SocketAddrV4, SocketAddrV6};

mod display_buffer;
mod ip_addr;
mod parser;
mod socket_addr;
