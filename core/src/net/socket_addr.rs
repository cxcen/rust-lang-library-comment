use super::display_buffer::DisplayBuffer;
use crate::fmt::{self, Write};
use crate::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// 一个互联网 socket 地址，可以是 IPv4 或 IPv6。
///
/// 互联网 socket 地址由一个 [IP address]、一个 16 位端口号组成；对某些 IP
/// 版本来说，还会携带额外的版本相关信息。IPv4 与 IPv6 的具体字段语义分别见
/// [`SocketAddrV4`] 和 [`SocketAddrV6`] 的文档。
///
/// [IP address]: IpAddr
///
/// # 可移植性
///
/// `SocketAddr` 旨在作为 socket 地址的可移植表示；它通常不同于目标操作系统 API
/// 内部使用的 socket 地址类型。不过，和所有 `repr(Rust)` 结构一样，它的精确布局
/// 仍未定义，不能在不同构建之间依赖该布局。
///
/// # 示例
///
/// ```
/// use std::net::{IpAddr, Ipv4Addr, SocketAddr};
///
/// let socket = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
///
/// assert_eq!("127.0.0.1:8080".parse(), Ok(socket));
/// assert_eq!(socket.port(), 8080);
/// assert_eq!(socket.is_ipv4(), true);
/// ```
#[derive(Copy, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[stable(feature = "rust1", since = "1.0.0")]
pub enum SocketAddr {
    /// IPv4 socket 地址。
    #[stable(feature = "rust1", since = "1.0.0")]
    V4(#[stable(feature = "rust1", since = "1.0.0")] SocketAddrV4),
    /// IPv6 socket 地址。
    #[stable(feature = "rust1", since = "1.0.0")]
    V6(#[stable(feature = "rust1", since = "1.0.0")] SocketAddrV6),
}

/// 一个 IPv4 socket 地址。
///
/// IPv4 socket 地址由一个 [`IPv4` address] 和一个 16 位端口号组成，
/// 如 [IETF RFC 793] 所述。
///
/// 需要同时覆盖 IPv4 和 IPv6 socket 地址的类型时，请参见 [`SocketAddr`]。
///
/// [IETF RFC 793]: https://tools.ietf.org/html/rfc793
/// [`IPv4` address]: Ipv4Addr
///
/// # 可移植性
///
/// `SocketAddrV4` 旨在作为 IPv4 socket 地址的可移植表示；它通常不同于目标操作系统
/// API 内部使用的 IPv4 socket 地址结构。不过，和所有 `repr(Rust)` 结构一样，
/// 它的精确布局仍未定义，不能在不同构建之间依赖该布局。
///
/// # 文本表示
///
/// `SocketAddrV4` 提供 [`FromStr`](crate::str::FromStr) 实现。它接受一个采用
/// [textual representation] 的 IPv4 地址，后接一个 `:`，再后接以十进制整数编码的端口号。
/// 其他格式不会被接受。
///
/// [textual representation]: Ipv4Addr#textual-representation
///
/// # 示例
///
/// ```
/// use std::net::{Ipv4Addr, SocketAddrV4};
///
/// let socket = SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 8080);
///
/// assert_eq!("127.0.0.1:8080".parse(), Ok(socket));
/// assert_eq!(socket.ip(), &Ipv4Addr::new(127, 0, 0, 1));
/// assert_eq!(socket.port(), 8080);
/// ```
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[stable(feature = "rust1", since = "1.0.0")]
pub struct SocketAddrV4 {
    ip: Ipv4Addr,
    port: u16,
}

/// 一个 IPv6 socket 地址。
///
/// IPv6 socket 地址由一个 [`IPv6` address]、一个 16 位端口号，以及包含 traffic class、
/// flow label 和 scope identifier 的字段组成（详情见 [IETF RFC 2553, Section 3.3]）。
///
/// 需要同时覆盖 IPv4 和 IPv6 socket 地址的类型时，请参见 [`SocketAddr`]。
///
/// [IETF RFC 2553, Section 3.3]: https://tools.ietf.org/html/rfc2553#section-3.3
/// [`IPv6` address]: Ipv6Addr
///
/// # 可移植性
///
/// `SocketAddrV6` 旨在作为 IPv6 socket 地址的可移植表示；它通常不同于目标操作系统
/// API 内部使用的 IPv6 socket 地址结构。不过，和所有 `repr(Rust)` 结构一样，
/// 它的精确布局仍未定义，不能在不同构建之间依赖该布局。
///
/// # 文本表示
///
/// `SocketAddrV6` 提供 [`FromStr`](crate::str::FromStr) 实现。该实现基于
/// [IETF RFC 5952] 推荐的方括号格式，并按 [IETF RFC 4007] 中的规则处理
/// scope identifier。
///
/// 它接受按以下顺序组成的地址：
///   - 左方括号（`[`）
///   - IPv6 地址的 [textual representation]
///   - _可选地_，一个百分号（`%`），后接以十进制整数编码的 scope identifier
///   - 右方括号（`]`）
///   - 冒号（`:`）
///   - 以十进制整数编码的端口号。
///
/// 例如，字符串 `[2001:db8::413]:443` 表示地址为 `2001:db8::413`、端口为 `443`
/// 的 `SocketAddrV6`。字符串 `[2001:db8::413%612]:443` 表示相同的地址和端口，
/// 但额外带有值为 `612` 的 scope identifier。
///
/// 其他格式不会被接受。
///
/// [IETF RFC 5952]: https://tools.ietf.org/html/rfc5952#section-6
/// [IETF RFC 4007]: https://tools.ietf.org/html/rfc4007#section-11
/// [textual representation]: Ipv6Addr#textual-representation
///
/// # 示例
///
/// ```
/// use std::net::{Ipv6Addr, SocketAddrV6};
///
/// let socket = SocketAddrV6::new(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1), 8080, 0, 0);
///
/// assert_eq!("[2001:db8::1]:8080".parse(), Ok(socket));
/// assert_eq!(socket.ip(), &Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1));
/// assert_eq!(socket.port(), 8080);
///
/// let mut with_scope = socket.clone();
/// with_scope.set_scope_id(3);
/// assert_eq!("[2001:db8::1%3]:8080".parse(), Ok(with_scope));
/// ```
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[stable(feature = "rust1", since = "1.0.0")]
pub struct SocketAddrV6 {
    ip: Ipv6Addr,
    port: u16,
    flowinfo: u32,
    scope_id: u32,
}

impl SocketAddr {
    /// 根据 [IP address] 和端口号创建新的 socket 地址。
    ///
    /// [IP address]: IpAddr
    ///
    /// # 示例
    ///
    /// ```
    /// use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    ///
    /// let socket = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
    /// assert_eq!(socket.ip(), IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)));
    /// assert_eq!(socket.port(), 8080);
    /// ```
    #[stable(feature = "ip_addr", since = "1.7.0")]
    #[must_use]
    #[rustc_const_stable(feature = "const_socketaddr", since = "1.69.0")]
    #[inline]
    pub const fn new(ip: IpAddr, port: u16) -> SocketAddr {
        match ip {
            IpAddr::V4(a) => SocketAddr::V4(SocketAddrV4::new(a, port)),
            IpAddr::V6(a) => SocketAddr::V6(SocketAddrV6::new(a, port, 0, 0)),
        }
    }

    /// 返回与此 socket 地址关联的 IP 地址。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    ///
    /// let socket = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
    /// assert_eq!(socket.ip(), IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)));
    /// ```
    #[must_use]
    #[stable(feature = "ip_addr", since = "1.7.0")]
    #[rustc_const_stable(feature = "const_socketaddr", since = "1.69.0")]
    #[inline]
    pub const fn ip(&self) -> IpAddr {
        match *self {
            SocketAddr::V4(ref a) => IpAddr::V4(*a.ip()),
            SocketAddr::V6(ref a) => IpAddr::V6(*a.ip()),
        }
    }

    /// 修改与此 socket 地址关联的 IP 地址。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    ///
    /// let mut socket = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
    /// socket.set_ip(IpAddr::V4(Ipv4Addr::new(10, 10, 0, 1)));
    /// assert_eq!(socket.ip(), IpAddr::V4(Ipv4Addr::new(10, 10, 0, 1)));
    /// ```
    #[inline]
    #[stable(feature = "sockaddr_setters", since = "1.9.0")]
    #[rustc_const_stable(feature = "const_sockaddr_setters", since = "1.87.0")]
    pub const fn set_ip(&mut self, new_ip: IpAddr) {
        // `match (*self, new_ip)` 只会修改 `self` 的副本，随后又丢弃该副本。
        match (self, new_ip) {
            (&mut SocketAddr::V4(ref mut a), IpAddr::V4(new_ip)) => a.set_ip(new_ip),
            (&mut SocketAddr::V6(ref mut a), IpAddr::V6(new_ip)) => a.set_ip(new_ip),
            (self_, new_ip) => *self_ = Self::new(new_ip, self_.port()),
        }
    }

    /// 返回与此 socket 地址关联的端口号。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    ///
    /// let socket = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
    /// assert_eq!(socket.port(), 8080);
    /// ```
    #[must_use]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_socketaddr", since = "1.69.0")]
    #[inline]
    pub const fn port(&self) -> u16 {
        match *self {
            SocketAddr::V4(ref a) => a.port(),
            SocketAddr::V6(ref a) => a.port(),
        }
    }

    /// 修改与此 socket 地址关联的端口号。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    ///
    /// let mut socket = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
    /// socket.set_port(1025);
    /// assert_eq!(socket.port(), 1025);
    /// ```
    #[inline]
    #[stable(feature = "sockaddr_setters", since = "1.9.0")]
    #[rustc_const_stable(feature = "const_sockaddr_setters", since = "1.87.0")]
    pub const fn set_port(&mut self, new_port: u16) {
        match *self {
            SocketAddr::V4(ref mut a) => a.set_port(new_port),
            SocketAddr::V6(ref mut a) => a.set_port(new_port),
        }
    }

    /// 如果此 `SocketAddr` 中的 [IP address] 是 [`IPv4` address]，则返回 [`true`]；
    /// 否则返回 [`false`]。
    ///
    /// [IP address]: IpAddr
    /// [`IPv4` address]: IpAddr::V4
    ///
    /// # 示例
    ///
    /// ```
    /// use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    ///
    /// let socket = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
    /// assert_eq!(socket.is_ipv4(), true);
    /// assert_eq!(socket.is_ipv6(), false);
    /// ```
    #[must_use]
    #[stable(feature = "sockaddr_checker", since = "1.16.0")]
    #[rustc_const_stable(feature = "const_socketaddr", since = "1.69.0")]
    #[inline]
    pub const fn is_ipv4(&self) -> bool {
        matches!(*self, SocketAddr::V4(_))
    }

    /// 如果此 `SocketAddr` 中的 [IP address] 是 [`IPv6` address]，则返回 [`true`]；
    /// 否则返回 [`false`]。
    ///
    /// [IP address]: IpAddr
    /// [`IPv6` address]: IpAddr::V6
    ///
    /// # 示例
    ///
    /// ```
    /// use std::net::{IpAddr, Ipv6Addr, SocketAddr};
    ///
    /// let socket = SocketAddr::new(IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 65535, 0, 1)), 8080);
    /// assert_eq!(socket.is_ipv4(), false);
    /// assert_eq!(socket.is_ipv6(), true);
    /// ```
    #[must_use]
    #[stable(feature = "sockaddr_checker", since = "1.16.0")]
    #[rustc_const_stable(feature = "const_socketaddr", since = "1.69.0")]
    #[inline]
    pub const fn is_ipv6(&self) -> bool {
        matches!(*self, SocketAddr::V6(_))
    }
}

impl SocketAddrV4 {
    /// 根据 [`IPv4` address] 和端口号创建新的 socket 地址。
    ///
    /// [`IPv4` address]: Ipv4Addr
    ///
    /// # 示例
    ///
    /// ```
    /// use std::net::{SocketAddrV4, Ipv4Addr};
    ///
    /// let socket = SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 8080);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[must_use]
    #[rustc_const_stable(feature = "const_socketaddr", since = "1.69.0")]
    #[inline]
    pub const fn new(ip: Ipv4Addr, port: u16) -> SocketAddrV4 {
        SocketAddrV4 { ip, port }
    }

    /// 返回与此 socket 地址关联的 IP 地址。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::net::{SocketAddrV4, Ipv4Addr};
    ///
    /// let socket = SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 8080);
    /// assert_eq!(socket.ip(), &Ipv4Addr::new(127, 0, 0, 1));
    /// ```
    #[must_use]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_socketaddr", since = "1.69.0")]
    #[inline]
    pub const fn ip(&self) -> &Ipv4Addr {
        &self.ip
    }

    /// 修改与此 socket 地址关联的 IP 地址。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::net::{SocketAddrV4, Ipv4Addr};
    ///
    /// let mut socket = SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 8080);
    /// socket.set_ip(Ipv4Addr::new(192, 168, 0, 1));
    /// assert_eq!(socket.ip(), &Ipv4Addr::new(192, 168, 0, 1));
    /// ```
    #[inline]
    #[stable(feature = "sockaddr_setters", since = "1.9.0")]
    #[rustc_const_stable(feature = "const_sockaddr_setters", since = "1.87.0")]
    pub const fn set_ip(&mut self, new_ip: Ipv4Addr) {
        self.ip = new_ip;
    }

    /// 返回与此 socket 地址关联的端口号。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::net::{SocketAddrV4, Ipv4Addr};
    ///
    /// let socket = SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 8080);
    /// assert_eq!(socket.port(), 8080);
    /// ```
    #[must_use]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_socketaddr", since = "1.69.0")]
    #[inline]
    pub const fn port(&self) -> u16 {
        self.port
    }

    /// 修改与此 socket 地址关联的端口号。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::net::{SocketAddrV4, Ipv4Addr};
    ///
    /// let mut socket = SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 8080);
    /// socket.set_port(4242);
    /// assert_eq!(socket.port(), 4242);
    /// ```
    #[inline]
    #[stable(feature = "sockaddr_setters", since = "1.9.0")]
    #[rustc_const_stable(feature = "const_sockaddr_setters", since = "1.87.0")]
    pub const fn set_port(&mut self, new_port: u16) {
        self.port = new_port;
    }
}

impl SocketAddrV6 {
    /// 根据 [`IPv6` address]、16 位端口号以及 `flowinfo` 和 `scope_id` 字段
    /// 创建新的 socket 地址。
    ///
    /// `flowinfo` 和 `scope_id` 参数的含义与布局见 [IETF RFC 2553, Section 3.3]。
    ///
    /// [IETF RFC 2553, Section 3.3]: https://tools.ietf.org/html/rfc2553#section-3.3
    /// [`IPv6` address]: Ipv6Addr
    ///
    /// # 示例
    ///
    /// ```
    /// use std::net::{SocketAddrV6, Ipv6Addr};
    ///
    /// let socket = SocketAddrV6::new(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1), 8080, 0, 0);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[must_use]
    #[rustc_const_stable(feature = "const_socketaddr", since = "1.69.0")]
    #[inline]
    pub const fn new(ip: Ipv6Addr, port: u16, flowinfo: u32, scope_id: u32) -> SocketAddrV6 {
        SocketAddrV6 { ip, port, flowinfo, scope_id }
    }

    /// 返回与此 socket 地址关联的 IP 地址。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::net::{SocketAddrV6, Ipv6Addr};
    ///
    /// let socket = SocketAddrV6::new(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1), 8080, 0, 0);
    /// assert_eq!(socket.ip(), &Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1));
    /// ```
    #[must_use]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_socketaddr", since = "1.69.0")]
    #[inline]
    pub const fn ip(&self) -> &Ipv6Addr {
        &self.ip
    }

    /// 修改与此 socket 地址关联的 IP 地址。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::net::{SocketAddrV6, Ipv6Addr};
    ///
    /// let mut socket = SocketAddrV6::new(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1), 8080, 0, 0);
    /// socket.set_ip(Ipv6Addr::new(76, 45, 0, 0, 0, 0, 0, 0));
    /// assert_eq!(socket.ip(), &Ipv6Addr::new(76, 45, 0, 0, 0, 0, 0, 0));
    /// ```
    #[inline]
    #[stable(feature = "sockaddr_setters", since = "1.9.0")]
    #[rustc_const_stable(feature = "const_sockaddr_setters", since = "1.87.0")]
    pub const fn set_ip(&mut self, new_ip: Ipv6Addr) {
        self.ip = new_ip;
    }

    /// 返回与此 socket 地址关联的端口号。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::net::{SocketAddrV6, Ipv6Addr};
    ///
    /// let socket = SocketAddrV6::new(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1), 8080, 0, 0);
    /// assert_eq!(socket.port(), 8080);
    /// ```
    #[must_use]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_socketaddr", since = "1.69.0")]
    #[inline]
    pub const fn port(&self) -> u16 {
        self.port
    }

    /// 修改与此 socket 地址关联的端口号。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::net::{SocketAddrV6, Ipv6Addr};
    ///
    /// let mut socket = SocketAddrV6::new(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1), 8080, 0, 0);
    /// socket.set_port(4242);
    /// assert_eq!(socket.port(), 4242);
    /// ```
    #[inline]
    #[stable(feature = "sockaddr_setters", since = "1.9.0")]
    #[rustc_const_stable(feature = "const_sockaddr_setters", since = "1.87.0")]
    pub const fn set_port(&mut self, new_port: u16) {
        self.port = new_port;
    }

    /// 返回与此地址关联的 flow information。
///
    /// 该信息对应 C 的 `netinet/in.h` 中的 `sin6_flowinfo` 字段，如
    /// [IETF RFC 2553, Section 3.3] 所述。它组合了 flow label 和 traffic class
    /// 信息；两者分别由 [IETF RFC 2460] 的 [Section 6] 和 [Section 7] 规定。
    ///
    /// [IETF RFC 2553, Section 3.3]: https://tools.ietf.org/html/rfc2553#section-3.3
    /// [IETF RFC 2460]: https://tools.ietf.org/html/rfc2460
    /// [Section 6]: https://tools.ietf.org/html/rfc2460#section-6
    /// [Section 7]: https://tools.ietf.org/html/rfc2460#section-7
    ///
    /// # 示例
    ///
    /// ```
    /// use std::net::{SocketAddrV6, Ipv6Addr};
    ///
    /// let socket = SocketAddrV6::new(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1), 8080, 10, 0);
    /// assert_eq!(socket.flowinfo(), 10);
    /// ```
    #[must_use]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_socketaddr", since = "1.69.0")]
    #[inline]
    pub const fn flowinfo(&self) -> u32 {
        self.flowinfo
    }

    /// 修改与此 socket 地址关联的 flow information。
///
    /// 更多细节请参见 [`SocketAddrV6::flowinfo`] 的文档。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::net::{SocketAddrV6, Ipv6Addr};
    ///
    /// let mut socket = SocketAddrV6::new(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1), 8080, 10, 0);
    /// socket.set_flowinfo(56);
    /// assert_eq!(socket.flowinfo(), 56);
    /// ```
    #[inline]
    #[stable(feature = "sockaddr_setters", since = "1.9.0")]
    #[rustc_const_stable(feature = "const_sockaddr_setters", since = "1.87.0")]
    pub const fn set_flowinfo(&mut self, new_flowinfo: u32) {
        self.flowinfo = new_flowinfo;
    }

    /// 返回与此地址关联的 scope ID。
///
    /// 该信息对应 C 的 `netinet/in.h` 中的 `sin6_scope_id` 字段，如
    /// [IETF RFC 2553, Section 3.3] 所述。
    ///
    /// [IETF RFC 2553, Section 3.3]: https://tools.ietf.org/html/rfc2553#section-3.3
    ///
    /// # 示例
    ///
    /// ```
    /// use std::net::{SocketAddrV6, Ipv6Addr};
    ///
    /// let socket = SocketAddrV6::new(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1), 8080, 0, 78);
    /// assert_eq!(socket.scope_id(), 78);
    /// ```
    #[must_use]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_socketaddr", since = "1.69.0")]
    #[inline]
    pub const fn scope_id(&self) -> u32 {
        self.scope_id
    }

    /// 修改与此 socket 地址关联的 scope ID。
///
    /// 更多细节请参见 [`SocketAddrV6::scope_id`] 的文档。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::net::{SocketAddrV6, Ipv6Addr};
    ///
    /// let mut socket = SocketAddrV6::new(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1), 8080, 0, 78);
    /// socket.set_scope_id(42);
    /// assert_eq!(socket.scope_id(), 42);
    /// ```
    #[inline]
    #[stable(feature = "sockaddr_setters", since = "1.9.0")]
    #[rustc_const_stable(feature = "const_sockaddr_setters", since = "1.87.0")]
    pub const fn set_scope_id(&mut self, new_scope_id: u32) {
        self.scope_id = new_scope_id;
    }
}

#[stable(feature = "ip_from_ip", since = "1.16.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl const From<SocketAddrV4> for SocketAddr {
    /// 将 [`SocketAddrV4`] 转换为 [`SocketAddr::V4`]。
    #[inline]
    fn from(sock4: SocketAddrV4) -> SocketAddr {
        SocketAddr::V4(sock4)
    }
}

#[stable(feature = "ip_from_ip", since = "1.16.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl const From<SocketAddrV6> for SocketAddr {
    /// 将 [`SocketAddrV6`] 转换为 [`SocketAddr::V6`]。
    #[inline]
    fn from(sock6: SocketAddrV6) -> SocketAddr {
        SocketAddr::V6(sock6)
    }
}

#[stable(feature = "addr_from_into_ip", since = "1.17.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl<I: [const] Into<IpAddr>> const From<(I, u16)> for SocketAddr {
    /// 将元组结构（Into<[`IpAddr`]>, `u16`）转换为 [`SocketAddr`]。
///
    /// 该转换会为 [`IpAddr::V4`] 创建 [`SocketAddr::V4`]，为 [`IpAddr::V6`]
    /// 创建 [`SocketAddr::V6`]。
///
    /// `u16` 会被视为新建 [`SocketAddr`] 的端口号。
    fn from(pieces: (I, u16)) -> SocketAddr {
        SocketAddr::new(pieces.0.into(), pieces.1)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl fmt::Display for SocketAddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            SocketAddr::V4(ref a) => a.fmt(f),
            SocketAddr::V6(ref a) => a.fmt(f),
        }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl fmt::Debug for SocketAddr {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, fmt)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl fmt::Display for SocketAddrV4 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // 如果没有对齐要求，则直接把 socket 地址写入 `f`。
        // 否则先写入本地缓冲区，再调用 `f.pad`。
        if f.precision().is_none() && f.width().is_none() {
            write!(f, "{}:{}", self.ip(), self.port())
        } else {
            const LONGEST_IPV4_SOCKET_ADDR: &str = "255.255.255.255:65535";

            let mut buf = DisplayBuffer::<{ LONGEST_IPV4_SOCKET_ADDR.len() }>::new();
            // 缓冲区足以容纳最长的 IPv4 socket 地址，因此这里不应失败。
            write!(buf, "{}:{}", self.ip(), self.port()).unwrap();

            f.pad(buf.as_str())
        }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl fmt::Debug for SocketAddrV4 {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, fmt)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl fmt::Display for SocketAddrV6 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // 如果没有对齐要求，则直接把 socket 地址写入 `f`。
        // 否则先写入本地缓冲区，再调用 `f.pad`。
        if f.precision().is_none() && f.width().is_none() {
            match self.scope_id() {
                0 => write!(f, "[{}]:{}", self.ip(), self.port()),
                scope_id => write!(f, "[{}%{}]:{}", self.ip(), scope_id, self.port()),
            }
        } else {
            const LONGEST_IPV6_SOCKET_ADDR: &str =
                "[ffff:ffff:ffff:ffff:ffff:ffff:ffff:ffff%4294967295]:65535";

            let mut buf = DisplayBuffer::<{ LONGEST_IPV6_SOCKET_ADDR.len() }>::new();
            match self.scope_id() {
                0 => write!(buf, "[{}]:{}", self.ip(), self.port()),
                scope_id => write!(buf, "[{}%{}]:{}", self.ip(), scope_id, self.port()),
            }
            // 缓冲区足以容纳最长的 IPv6 socket 地址，因此这里不应失败。
            .unwrap();

            f.pad(buf.as_str())
        }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl fmt::Debug for SocketAddrV6 {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, fmt)
    }
}
