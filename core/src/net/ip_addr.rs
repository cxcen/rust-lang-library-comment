use super::display_buffer::DisplayBuffer;
use crate::cmp::Ordering;
use crate::fmt::{self, Write};
use crate::hash::{Hash, Hasher};
use crate::mem::transmute;
use crate::ops::{BitAnd, BitAndAssign, BitOr, BitOrAssign, Not};

/// 一个 IP 地址，可以是 IPv4 或 IPv6。
///
/// 该枚举可以包含 [`Ipv4Addr`] 或 [`Ipv6Addr`]；更多细节请分别参见这两个类型的文档。
///
/// # 示例
///
/// ```
/// use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
///
/// let localhost_v4 = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
/// let localhost_v6 = IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1));
///
/// assert_eq!("127.0.0.1".parse(), Ok(localhost_v4));
/// assert_eq!("::1".parse(), Ok(localhost_v6));
///
/// assert_eq!(localhost_v4.is_ipv6(), false);
/// assert_eq!(localhost_v4.is_ipv4(), true);
/// ```
#[rustc_diagnostic_item = "IpAddr"]
#[stable(feature = "ip_addr", since = "1.7.0")]
#[derive(Copy, Clone, Eq, PartialEq, Hash, PartialOrd, Ord)]
pub enum IpAddr {
    /// IPv4 地址。
    #[stable(feature = "ip_addr", since = "1.7.0")]
    V4(#[stable(feature = "ip_addr", since = "1.7.0")] Ipv4Addr),
    /// IPv6 地址。
    #[stable(feature = "ip_addr", since = "1.7.0")]
    V6(#[stable(feature = "ip_addr", since = "1.7.0")] Ipv6Addr),
}

/// 一个 IPv4 地址。
///
/// [IETF RFC 791] 将 IPv4 地址定义为 32 位整数。它通常表示为四个 octet。
///
/// 需要同时覆盖 IPv4 和 IPv6 地址的类型时，请参见 [`IpAddr`]。
///
/// [IETF RFC 791]: https://tools.ietf.org/html/rfc791
///
/// # 文本表示
///
/// `Ipv4Addr` 提供 [`FromStr`] 实现。四个 octet 以十进制记法书写，并用 `.`
/// 分隔；这种格式称为“点分十进制记法”。按照 [IETF RFC 6943]，带前导 `0`
/// 的八进制数字和带前导 `0x` 的十六进制数字都不被允许。
///
/// [IETF RFC 6943]: https://tools.ietf.org/html/rfc6943#section-3.1.1
/// [`FromStr`]: crate::str::FromStr
///
/// # 示例
///
/// ```
/// use std::net::Ipv4Addr;
///
/// let localhost = Ipv4Addr::new(127, 0, 0, 1);
/// assert_eq!("127.0.0.1".parse(), Ok(localhost));
/// assert_eq!(localhost.is_loopback(), true);
/// assert!("012.004.002.000".parse::<Ipv4Addr>().is_err()); // all octets are in octal
/// assert!("0000000.0.0.0".parse::<Ipv4Addr>().is_err()); // first octet is a zero in octal
/// assert!("0xcb.0x0.0x71.0x00".parse::<Ipv4Addr>().is_err()); // all octets are in hex
/// ```
#[rustc_diagnostic_item = "Ipv4Addr"]
#[derive(Copy, Clone, PartialEq, Eq)]
#[stable(feature = "rust1", since = "1.0.0")]
pub struct Ipv4Addr {
    octets: [u8; 4],
}

#[stable(feature = "rust1", since = "1.0.0")]
impl Hash for Ipv4Addr {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // 对固定宽度整数做哈希通常比对字节串做哈希更高效，因此先转换再哈希。
        // 这里不使用 `to_bits()`，因为它可能引入不必要的字节交换。
        u32::from_ne_bytes(self.octets).hash(state);
    }
}

/// 一个 IPv6 地址。
///
/// [IETF RFC 4291] 将 IPv6 地址定义为 128 位整数。它通常表示为八个 16 位 segment。
///
/// [IETF RFC 4291]: https://tools.ietf.org/html/rfc4291
///
/// # 嵌入 IPv4 地址
///
/// 需要同时覆盖 IPv4 和 IPv6 地址的类型时，请参见 [`IpAddr`]。
///
/// 为协助从 IPv4 过渡到 IPv6，标准定义了两类嵌入 IPv4 地址的 IPv6 地址：
/// IPv4-compatible 地址和 IPv4-mapped 地址。其中 IPv4-compatible 地址已经正式废弃。
///
/// 除相关标准规定的含义外，本实现不会为这两类地址额外赋予特殊语义。这意味着
/// `::ffff:127.0.0.1` 这样的地址虽然表示一个 IPv4 loopback 地址，但它本身并不是
/// IPv6 loopback 地址；IPv6 loopback 地址只有 `::1`。处理这类所谓
/// “IPv4-in-IPv6” 地址时，应先将其转换为规范的 IPv4 地址。
///
/// ### IPv4-Compatible IPv6 地址
///
/// IPv4-compatible IPv6 地址定义于 [IETF RFC 4291 Section 2.5.5.1]，并且已经正式废弃。
/// RFC 对 “IPv4-Compatible IPv6 address” 的格式描述如下：
///
/// ```text
/// |                80 bits               | 16 |      32 bits        |
/// +--------------------------------------+--------------------------+
/// |0000..............................0000|0000|    IPv4 address     |
/// +--------------------------------------+----+---------------------+
/// ```
/// 因此，`::a.b.c.d` 是一个 IPv4-compatible IPv6 地址，表示 IPv4 地址 `a.b.c.d`。
///
/// 使用 [`Ipv4Addr::to_ipv6_compatible`] 可将 IPv4 地址转换为 IPv4-compatible IPv6
/// 地址。使用 [`Ipv6Addr::to_ipv4`] 可将 IPv4-compatible IPv6 地址转换回规范的
/// IPv4 地址。
///
/// [IETF RFC 4291 Section 2.5.5.1]: https://datatracker.ietf.org/doc/html/rfc4291#section-2.5.5.1
///
/// ### IPv4-Mapped IPv6 地址
///
/// IPv4-mapped IPv6 地址定义于 [IETF RFC 4291 Section 2.5.5.2]。
/// RFC 对 “IPv4-Mapped IPv6 address” 的格式描述如下：
///
/// ```text
/// |                80 bits               | 16 |      32 bits        |
/// +--------------------------------------+--------------------------+
/// |0000..............................0000|FFFF|    IPv4 address     |
/// +--------------------------------------+----+---------------------+
/// ```
/// 因此，`::ffff:a.b.c.d` 是一个 IPv4-mapped IPv6 地址，表示 IPv4 地址 `a.b.c.d`。
///
/// 使用 [`Ipv4Addr::to_ipv6_mapped`] 可将 IPv4 地址转换为 IPv4-mapped IPv6 地址。
/// 使用 [`Ipv6Addr::to_ipv4`] 可将 IPv4-mapped IPv6 地址转换回规范的 IPv4 地址。
/// 注意，该方法也会把 IPv6 loopback 地址 `::1` 转换为 `0.0.0.1`；如果要避免这种情况，
/// 请使用 [`Ipv6Addr::to_ipv4_mapped`]。
///
/// [IETF RFC 4291 Section 2.5.5.2]: https://datatracker.ietf.org/doc/html/rfc4291#section-2.5.5.2
///
/// # 文本表示
///
/// `Ipv6Addr` 提供 [`FromStr`] 实现。IPv6 地址有多种文本写法；通常每个 segment
/// 使用十六进制记法，segment 之间用 `:` 分隔。更多信息见 [IETF RFC 5952]。
///
/// [`FromStr`]: crate::str::FromStr
/// [IETF RFC 5952]: https://tools.ietf.org/html/rfc5952
///
/// # 示例
///
/// ```
/// use std::net::Ipv6Addr;
///
/// let localhost = Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1);
/// assert_eq!("::1".parse(), Ok(localhost));
/// assert_eq!(localhost.is_loopback(), true);
/// ```
#[rustc_diagnostic_item = "Ipv6Addr"]
#[derive(Copy, Clone, PartialEq, Eq)]
#[stable(feature = "rust1", since = "1.0.0")]
pub struct Ipv6Addr {
    octets: [u8; 16],
}

#[stable(feature = "rust1", since = "1.0.0")]
impl Hash for Ipv6Addr {
    fn hash<H: Hasher>(&self, state: &mut H) {
        // 对固定宽度整数做哈希通常比对字节串做哈希更高效，因此先转换再哈希。
        // 这里不使用 `to_bits()`，因为它可能引入不必要的字节交换。
        u128::from_ne_bytes(self.octets).hash(state);
    }
}

/// [IETF RFC 7346 section 2] 定义的 [IPv6 multicast address] scope。
///
/// # 稳定性保证
///
/// multicast scope 的所有可能取值并非都已经分配。未来 RFC 可能引入新的 scope，
/// 这些 scope 会作为新 variant 加入此枚举；因此该枚举标记为 `#[non_exhaustive]`。
///
/// # 示例
/// ```
/// #![feature(ip)]
///
/// use std::net::Ipv6Addr;
/// use std::net::Ipv6MulticastScope::*;
///
/// // 一个具有全局作用域的 IPv6 组播地址（`ff0e::`）。
/// let address = Ipv6Addr::new(0xff0e, 0, 0, 0, 0, 0, 0, 0);
///
/// // 将打印 "Global scope"。
/// match address.multicast_scope() {
///     Some(InterfaceLocal) => println!("Interface-Local scope"),
///     Some(LinkLocal) => println!("Link-Local scope"),
///     Some(RealmLocal) => println!("Realm-Local scope"),
///     Some(AdminLocal) => println!("Admin-Local scope"),
///     Some(SiteLocal) => println!("Site-Local scope"),
///     Some(OrganizationLocal) => println!("Organization-Local scope"),
///     Some(Global) => println!("Global scope"),
///     Some(_) => println!("Unknown scope"),
///     None => println!("Not a multicast address!")
/// }
///
/// ```
///
/// [IPv6 multicast address]: Ipv6Addr
/// [IETF RFC 7346 section 2]: https://tools.ietf.org/html/rfc7346#section-2
#[derive(Copy, PartialEq, Eq, Clone, Hash, Debug)]
#[unstable(feature = "ip", issue = "27709")]
#[non_exhaustive]
pub enum Ipv6MulticastScope {
    /// Interface-Local（接口本地）作用域。
    InterfaceLocal,
    /// Link-Local（链路本地）作用域。
    LinkLocal,
    /// Realm-Local（域本地）作用域。
    RealmLocal,
    /// Admin-Local（管理本地）作用域。
    AdminLocal,
    /// Site-Local（站点本地）作用域。
    SiteLocal,
    /// Organization-Local（组织本地）作用域。
    OrganizationLocal,
    /// Global（全局）作用域。
    Global,
}

impl IpAddr {
    /// 如果是特殊的 “unspecified” 地址，则返回 [`true`]。
///
    /// 更多细节请参见 [`Ipv4Addr::is_unspecified()`] 和
    /// [`Ipv6Addr::is_unspecified()`] 的文档。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
    ///
    /// assert_eq!(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 0)).is_unspecified(), true);
    /// assert_eq!(IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0)).is_unspecified(), true);
    /// ```
    #[rustc_const_stable(feature = "const_ip_50", since = "1.50.0")]
    #[stable(feature = "ip_shared", since = "1.12.0")]
    #[must_use]
    #[inline]
    pub const fn is_unspecified(&self) -> bool {
        match self {
            IpAddr::V4(ip) => ip.is_unspecified(),
            IpAddr::V6(ip) => ip.is_unspecified(),
        }
    }

    /// 如果这是 loopback 地址，则返回 [`true`]。
///
    /// 更多细节请参见 [`Ipv4Addr::is_loopback()`] 和
    /// [`Ipv6Addr::is_loopback()`] 的文档。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
    ///
    /// assert_eq!(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)).is_loopback(), true);
    /// assert_eq!(IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0x1)).is_loopback(), true);
    /// ```
    #[rustc_const_stable(feature = "const_ip_50", since = "1.50.0")]
    #[stable(feature = "ip_shared", since = "1.12.0")]
    #[must_use]
    #[inline]
    pub const fn is_loopback(&self) -> bool {
        match self {
            IpAddr::V4(ip) => ip.is_loopback(),
            IpAddr::V6(ip) => ip.is_loopback(),
        }
    }

    /// 如果该地址看起来是全局可路由的，则返回 [`true`]。
///
    /// 更多细节请参见 [`Ipv4Addr::is_global()`] 和 [`Ipv6Addr::is_global()`] 的文档。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(ip)]
    ///
    /// use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
    ///
    /// assert_eq!(IpAddr::V4(Ipv4Addr::new(80, 9, 12, 3)).is_global(), true);
    /// assert_eq!(IpAddr::V6(Ipv6Addr::new(0, 0, 0x1c9, 0, 0, 0xafc8, 0, 0x1)).is_global(), true);
    /// ```
    #[unstable(feature = "ip", issue = "27709")]
    #[must_use]
    #[inline]
    pub const fn is_global(&self) -> bool {
        match self {
            IpAddr::V4(ip) => ip.is_global(),
            IpAddr::V6(ip) => ip.is_global(),
        }
    }

    /// 如果这是 multicast 地址，则返回 [`true`]。
///
    /// 更多细节请参见 [`Ipv4Addr::is_multicast()`] 和
    /// [`Ipv6Addr::is_multicast()`] 的文档。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
    ///
    /// assert_eq!(IpAddr::V4(Ipv4Addr::new(224, 254, 0, 0)).is_multicast(), true);
    /// assert_eq!(IpAddr::V6(Ipv6Addr::new(0xff00, 0, 0, 0, 0, 0, 0, 0)).is_multicast(), true);
    /// ```
    #[rustc_const_stable(feature = "const_ip_50", since = "1.50.0")]
    #[stable(feature = "ip_shared", since = "1.12.0")]
    #[must_use]
    #[inline]
    pub const fn is_multicast(&self) -> bool {
        match self {
            IpAddr::V4(ip) => ip.is_multicast(),
            IpAddr::V6(ip) => ip.is_multicast(),
        }
    }

    /// 如果此地址位于保留给文档示例使用的地址范围内，则返回 [`true`]。
///
    /// 更多细节请参见 [`Ipv4Addr::is_documentation()`] 和
    /// [`Ipv6Addr::is_documentation()`] 的文档。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(ip)]
    ///
    /// use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
    ///
    /// assert_eq!(IpAddr::V4(Ipv4Addr::new(203, 0, 113, 6)).is_documentation(), true);
    /// assert_eq!(
    ///     IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 0)).is_documentation(),
    ///     true
    /// );
    /// ```
    #[unstable(feature = "ip", issue = "27709")]
    #[must_use]
    #[inline]
    pub const fn is_documentation(&self) -> bool {
        match self {
            IpAddr::V4(ip) => ip.is_documentation(),
            IpAddr::V6(ip) => ip.is_documentation(),
        }
    }

    /// 如果此地址位于保留给基准测试使用的地址范围内，则返回 [`true`]。
///
    /// 更多细节请参见 [`Ipv4Addr::is_benchmarking()`] 和
    /// [`Ipv6Addr::is_benchmarking()`] 的文档。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(ip)]
    ///
    /// use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
    ///
    /// assert_eq!(IpAddr::V4(Ipv4Addr::new(198, 19, 255, 255)).is_benchmarking(), true);
    /// assert_eq!(IpAddr::V6(Ipv6Addr::new(0x2001, 0x2, 0, 0, 0, 0, 0, 0)).is_benchmarking(), true);
    /// ```
    #[unstable(feature = "ip", issue = "27709")]
    #[must_use]
    #[inline]
    pub const fn is_benchmarking(&self) -> bool {
        match self {
            IpAddr::V4(ip) => ip.is_benchmarking(),
            IpAddr::V6(ip) => ip.is_benchmarking(),
        }
    }

    /// 如果此地址是 [`IPv4` address]，则返回 [`true`]；否则返回 [`false`]。
    ///
    /// [`IPv4` address]: IpAddr::V4
    ///
    /// # 示例
    ///
    /// ```
    /// use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
    ///
    /// assert_eq!(IpAddr::V4(Ipv4Addr::new(203, 0, 113, 6)).is_ipv4(), true);
    /// assert_eq!(IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 0)).is_ipv4(), false);
    /// ```
    #[rustc_const_stable(feature = "const_ip_50", since = "1.50.0")]
    #[stable(feature = "ipaddr_checker", since = "1.16.0")]
    #[must_use]
    #[inline]
    pub const fn is_ipv4(&self) -> bool {
        matches!(self, IpAddr::V4(_))
    }

    /// 如果此地址是 [`IPv6` address]，则返回 [`true`]；否则返回 [`false`]。
    ///
    /// [`IPv6` address]: IpAddr::V6
    ///
    /// # 示例
    ///
    /// ```
    /// use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
    ///
    /// assert_eq!(IpAddr::V4(Ipv4Addr::new(203, 0, 113, 6)).is_ipv6(), false);
    /// assert_eq!(IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 0)).is_ipv6(), true);
    /// ```
    #[rustc_const_stable(feature = "const_ip_50", since = "1.50.0")]
    #[stable(feature = "ipaddr_checker", since = "1.16.0")]
    #[must_use]
    #[inline]
    pub const fn is_ipv6(&self) -> bool {
        matches!(self, IpAddr::V6(_))
    }

    /// 如果此地址是 IPv4-mapped IPv6 地址，则转换为 `IpAddr::V4`；否则原样返回 `self`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
    ///
    /// let localhost_v4 = Ipv4Addr::new(127, 0, 0, 1);
    ///
    /// assert_eq!(IpAddr::V4(localhost_v4).to_canonical(), localhost_v4);
    /// assert_eq!(IpAddr::V6(localhost_v4.to_ipv6_mapped()).to_canonical(), localhost_v4);
    /// assert_eq!(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)).to_canonical().is_loopback(), true);
    /// assert_eq!(IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0xffff, 0x7f00, 0x1)).is_loopback(), false);
    /// assert_eq!(IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0xffff, 0x7f00, 0x1)).to_canonical().is_loopback(), true);
    /// ```
    #[inline]
    #[must_use = "this returns the result of the operation, \
                  without modifying the original"]
    #[stable(feature = "ip_to_canonical", since = "1.75.0")]
    #[rustc_const_stable(feature = "ip_to_canonical", since = "1.75.0")]
    pub const fn to_canonical(&self) -> IpAddr {
        match self {
            IpAddr::V4(_) => *self,
            IpAddr::V6(v6) => v6.to_canonical(),
        }
    }

    /// 以切片形式返回组成此地址的 8 位整数。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(ip_as_octets)]
    ///
    /// use std::net::{Ipv4Addr, Ipv6Addr, IpAddr};
    ///
    /// assert_eq!(IpAddr::V4(Ipv4Addr::LOCALHOST).as_octets(), &[127, 0, 0, 1]);
    /// assert_eq!(IpAddr::V6(Ipv6Addr::LOCALHOST).as_octets(),
    ///            &[0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1])
    /// ```
    #[unstable(feature = "ip_as_octets", issue = "137259")]
    #[inline]
    pub const fn as_octets(&self) -> &[u8] {
        match self {
            IpAddr::V4(ip) => ip.as_octets().as_slice(),
            IpAddr::V6(ip) => ip.as_octets().as_slice(),
        }
    }
}

impl Ipv4Addr {
    /// 根据四个 8 位 octet 创建新的 IPv4 地址。
///
    /// 结果表示 IP 地址 `a`.`b`.`c`.`d`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::net::Ipv4Addr;
    ///
    /// let addr = Ipv4Addr::new(127, 0, 0, 1);
    /// ```
    #[rustc_const_stable(feature = "const_ip_32", since = "1.32.0")]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[must_use]
    #[inline]
    pub const fn new(a: u8, b: u8, c: u8, d: u8) -> Ipv4Addr {
        Ipv4Addr { octets: [a, b, c, d] }
    }

    /// IPv4 地址的位数。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::net::Ipv4Addr;
    ///
    /// assert_eq!(Ipv4Addr::BITS, 32);
    /// ```
    #[stable(feature = "ip_bits", since = "1.80.0")]
    pub const BITS: u32 = 32;

    /// 将 IPv4 地址转换为使用本机字节序的 `u32` 表示。
///
    /// 虽然 IPv4 地址按 big-endian 定义，但返回的 `u32` 值使用目标平台的本机字节序。
    /// 也就是说，这个 `u32` 是 IPv4 地址的整数表示，而不是把 IPv4 地址的 big-endian
    /// 位串按平台字节序重新解释得到的整数。因此，无论目标平台采用哪种端序，对返回值应用
    /// `0xffffff00` 掩码都会把地址中的最后一个 octet 置为 0。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::net::Ipv4Addr;
    ///
    /// let addr = Ipv4Addr::new(0x12, 0x34, 0x56, 0x78);
    /// assert_eq!(0x12345678, addr.to_bits());
    /// ```
    ///
    /// ```
    /// use std::net::Ipv4Addr;
    ///
    /// let addr = Ipv4Addr::new(0x12, 0x34, 0x56, 0x78);
    /// let addr_bits = addr.to_bits() & 0xffffff00;
    /// assert_eq!(Ipv4Addr::new(0x12, 0x34, 0x56, 0x00), Ipv4Addr::from_bits(addr_bits));
    ///
    /// ```
    #[rustc_const_stable(feature = "ip_bits", since = "1.80.0")]
    #[stable(feature = "ip_bits", since = "1.80.0")]
    #[must_use]
    #[inline]
    pub const fn to_bits(self) -> u32 {
        u32::from_be_bytes(self.octets)
    }

    /// 将使用本机字节序的 `u32` 转换为 IPv4 地址。
///
    /// 端序语义请参见 [`Ipv4Addr::to_bits`]。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::net::Ipv4Addr;
    ///
    /// let addr = Ipv4Addr::from_bits(0x12345678);
    /// assert_eq!(Ipv4Addr::new(0x12, 0x34, 0x56, 0x78), addr);
    /// ```
    #[rustc_const_stable(feature = "ip_bits", since = "1.80.0")]
    #[stable(feature = "ip_bits", since = "1.80.0")]
    #[must_use]
    #[inline]
    pub const fn from_bits(bits: u32) -> Ipv4Addr {
        Ipv4Addr { octets: bits.to_be_bytes() }
    }

    /// 指向 localhost 的 IPv4 地址：`127.0.0.1`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::net::Ipv4Addr;
    ///
    /// let addr = Ipv4Addr::LOCALHOST;
    /// assert_eq!(addr, Ipv4Addr::new(127, 0, 0, 1));
    /// ```
    #[stable(feature = "ip_constructors", since = "1.30.0")]
    pub const LOCALHOST: Self = Ipv4Addr::new(127, 0, 0, 1);

    /// 表示 unspecified 地址的 IPv4 地址：`0.0.0.0`。
///
    /// 这对应其他语言中的常量 `INADDR_ANY`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::net::Ipv4Addr;
    ///
    /// let addr = Ipv4Addr::UNSPECIFIED;
    /// assert_eq!(addr, Ipv4Addr::new(0, 0, 0, 0));
    /// ```
    #[doc(alias = "INADDR_ANY")]
    #[stable(feature = "ip_constructors", since = "1.30.0")]
    pub const UNSPECIFIED: Self = Ipv4Addr::new(0, 0, 0, 0);

    /// 表示 broadcast 地址的 IPv4 地址：`255.255.255.255`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::net::Ipv4Addr;
    ///
    /// let addr = Ipv4Addr::BROADCAST;
    /// assert_eq!(addr, Ipv4Addr::new(255, 255, 255, 255));
    /// ```
    #[stable(feature = "ip_constructors", since = "1.30.0")]
    pub const BROADCAST: Self = Ipv4Addr::new(255, 255, 255, 255);

    /// 返回组成此地址的四个 8 位整数。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::net::Ipv4Addr;
    ///
    /// let addr = Ipv4Addr::new(127, 0, 0, 1);
    /// assert_eq!(addr.octets(), [127, 0, 0, 1]);
    /// ```
    #[rustc_const_stable(feature = "const_ip_50", since = "1.50.0")]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[must_use]
    #[inline]
    pub const fn octets(&self) -> [u8; 4] {
        self.octets
    }

    /// 根据包含四个元素的字节数组创建 `Ipv4Addr`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::net::Ipv4Addr;
    ///
    /// let addr = Ipv4Addr::from_octets([13u8, 12u8, 11u8, 10u8]);
    /// assert_eq!(Ipv4Addr::new(13, 12, 11, 10), addr);
    /// ```
    #[stable(feature = "ip_from", since = "1.91.0")]
    #[rustc_const_stable(feature = "ip_from", since = "1.91.0")]
    #[must_use]
    #[inline]
    pub const fn from_octets(octets: [u8; 4]) -> Ipv4Addr {
        Ipv4Addr { octets }
    }

    /// 以切片形式返回组成此地址的四个 8 位整数。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(ip_as_octets)]
    ///
    /// use std::net::Ipv4Addr;
    ///
    /// let addr = Ipv4Addr::new(127, 0, 0, 1);
    /// assert_eq!(addr.as_octets(), &[127, 0, 0, 1]);
    /// ```
    #[unstable(feature = "ip_as_octets", issue = "137259")]
    #[inline]
    pub const fn as_octets(&self) -> &[u8; 4] {
        &self.octets
    }

    /// 如果是特殊的 “unspecified” 地址（`0.0.0.0`），则返回 [`true`]。
///
    /// 该属性定义于 _UNIX Network Programming, Second Edition_,
    /// W. Richard Stevens, p. 891；另见 [ip7]。
    ///
    /// [ip7]: https://man7.org/linux/man-pages/man7/ip.7.html
    ///
    /// # 示例
    ///
    /// ```
    /// use std::net::Ipv4Addr;
    ///
    /// assert_eq!(Ipv4Addr::new(0, 0, 0, 0).is_unspecified(), true);
    /// assert_eq!(Ipv4Addr::new(45, 22, 13, 197).is_unspecified(), false);
    /// ```
    #[rustc_const_stable(feature = "const_ip_32", since = "1.32.0")]
    #[stable(feature = "ip_shared", since = "1.12.0")]
    #[must_use]
    #[inline]
    pub const fn is_unspecified(&self) -> bool {
        u32::from_be_bytes(self.octets) == 0
    }

    /// 如果这是 loopback 地址（`127.0.0.0/8`），则返回 [`true`]。
///
    /// 该属性由 [IETF RFC 1122] 定义。
    ///
    /// [IETF RFC 1122]: https://tools.ietf.org/html/rfc1122
    ///
    /// # 示例
    ///
    /// ```
    /// use std::net::Ipv4Addr;
    ///
    /// assert_eq!(Ipv4Addr::new(127, 0, 0, 1).is_loopback(), true);
    /// assert_eq!(Ipv4Addr::new(45, 22, 13, 197).is_loopback(), false);
    /// ```
    #[rustc_const_stable(feature = "const_ip_50", since = "1.50.0")]
    #[stable(since = "1.7.0", feature = "ip_17")]
    #[must_use]
    #[inline]
    pub const fn is_loopback(&self) -> bool {
        self.octets()[0] == 127
    }

    /// 如果这是 private 地址，则返回 [`true`]。
///
    /// private 地址范围定义于 [IETF RFC 1918]，包括：
    ///
    ///  - `10.0.0.0/8`
    ///  - `172.16.0.0/12`
    ///  - `192.168.0.0/16`
    ///
    /// [IETF RFC 1918]: https://tools.ietf.org/html/rfc1918
    ///
    /// # 示例
    ///
    /// ```
    /// use std::net::Ipv4Addr;
    ///
    /// assert_eq!(Ipv4Addr::new(10, 0, 0, 1).is_private(), true);
    /// assert_eq!(Ipv4Addr::new(10, 10, 10, 10).is_private(), true);
    /// assert_eq!(Ipv4Addr::new(172, 16, 10, 10).is_private(), true);
    /// assert_eq!(Ipv4Addr::new(172, 29, 45, 14).is_private(), true);
    /// assert_eq!(Ipv4Addr::new(172, 32, 0, 2).is_private(), false);
    /// assert_eq!(Ipv4Addr::new(192, 168, 0, 2).is_private(), true);
    /// assert_eq!(Ipv4Addr::new(192, 169, 0, 2).is_private(), false);
    /// ```
    #[rustc_const_stable(feature = "const_ip_50", since = "1.50.0")]
    #[stable(since = "1.7.0", feature = "ip_17")]
    #[must_use]
    #[inline]
    pub const fn is_private(&self) -> bool {
        match self.octets() {
            [10, ..] => true,
            [172, b, ..] if b >= 16 && b <= 31 => true,
            [192, 168, ..] => true,
            _ => false,
        }
    }

    /// 如果该地址是 link-local 地址（`169.254.0.0/16`），则返回 [`true`]。
///
    /// 该属性由 [IETF RFC 3927] 定义。
    ///
    /// [IETF RFC 3927]: https://tools.ietf.org/html/rfc3927
    ///
    /// # 示例
    ///
    /// ```
    /// use std::net::Ipv4Addr;
    ///
    /// assert_eq!(Ipv4Addr::new(169, 254, 0, 0).is_link_local(), true);
    /// assert_eq!(Ipv4Addr::new(169, 254, 10, 65).is_link_local(), true);
    /// assert_eq!(Ipv4Addr::new(16, 89, 10, 65).is_link_local(), false);
    /// ```
    #[rustc_const_stable(feature = "const_ip_50", since = "1.50.0")]
    #[stable(since = "1.7.0", feature = "ip_17")]
    #[must_use]
    #[inline]
    pub const fn is_link_local(&self) -> bool {
        matches!(self.octets(), [169, 254, ..])
    }

    /// 如果该地址按 [IANA IPv4 Special-Purpose Address Registry] 看起来是全局可达的，
    /// 则返回 [`true`]。
///
    /// 地址在实际网络中能否到达取决于具体网络配置。除非某个 IPv4 地址被明确标记为
    /// *非*全局可达，大多数 IPv4 地址都视为全局可达。
///
    /// 以下列出一些重要但并非穷尽的非全局可达地址：
///
    /// - [unspecified address]（[`is_unspecified`](Ipv4Addr::is_unspecified)）
    /// - 保留给 private use 的地址（[`is_private`](Ipv4Addr::is_private)）
    /// - shared address space 中的地址（[`is_shared`](Ipv4Addr::is_shared)）
    /// - loopback 地址（[`is_loopback`](Ipv4Addr::is_loopback)）
    /// - link-local 地址（[`is_link_local`](Ipv4Addr::is_link_local)）
    /// - 保留给文档示例的地址（[`is_documentation`](Ipv4Addr::is_documentation)）
    /// - 保留给基准测试的地址（[`is_benchmarking`](Ipv4Addr::is_benchmarking)）
    /// - reserved 地址（[`is_reserved`](Ipv4Addr::is_reserved)）
    /// - [broadcast address]（[`is_broadcast`](Ipv4Addr::is_broadcast)）
///
    /// 哪些地址全局可达的完整概览，请参见 [IANA IPv4 Special-Purpose Address Registry] 中的表格。
    ///
    /// [IANA IPv4 Special-Purpose Address Registry]: https://www.iana.org/assignments/iana-ipv4-special-registry/iana-ipv4-special-registry.xhtml
    /// [unspecified address]: Ipv4Addr::UNSPECIFIED
    /// [broadcast address]: Ipv4Addr::BROADCAST
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(ip)]
    ///
    /// use std::net::Ipv4Addr;
    ///
    /// // 大多数 IPv4 地址都是全局可达的：
    /// assert_eq!(Ipv4Addr::new(80, 9, 12, 3).is_global(), true);
    ///
    /// // 不过有些地址被赋予了特殊含义，
    /// // 使它们变得不可全局到达。一些例子如下：
    ///
    /// // 未指定地址（`0.0.0.0`）
    /// assert_eq!(Ipv4Addr::UNSPECIFIED.is_global(), false);
    ///
    /// // 保留供私有使用的地址（`10.0.0.0/8`、`172.16.0.0/12`、192.168.0.0/16）
    /// assert_eq!(Ipv4Addr::new(10, 254, 0, 0).is_global(), false);
    /// assert_eq!(Ipv4Addr::new(192, 168, 10, 65).is_global(), false);
    /// assert_eq!(Ipv4Addr::new(172, 16, 10, 65).is_global(), false);
    ///
    /// // 共享地址空间中的地址（`100.64.0.0/10`）
    /// assert_eq!(Ipv4Addr::new(100, 100, 0, 0).is_global(), false);
    ///
    /// // 环回地址（`127.0.0.0/8`）
    /// assert_eq!(Ipv4Addr::LOCALHOST.is_global(), false);
    ///
    /// // 链路本地地址（`169.254.0.0/16`）
    /// assert_eq!(Ipv4Addr::new(169, 254, 45, 1).is_global(), false);
    ///
    /// // 保留供文档使用的地址（`192.0.2.0/24`、`198.51.100.0/24`、`203.0.113.0/24`）
    /// assert_eq!(Ipv4Addr::new(192, 0, 2, 255).is_global(), false);
    /// assert_eq!(Ipv4Addr::new(198, 51, 100, 65).is_global(), false);
    /// assert_eq!(Ipv4Addr::new(203, 0, 113, 6).is_global(), false);
    ///
    /// // 保留供基准测试使用的地址（`198.18.0.0/15`）
    /// assert_eq!(Ipv4Addr::new(198, 18, 0, 0).is_global(), false);
    ///
    /// // 保留地址（`240.0.0.0/4`）
    /// assert_eq!(Ipv4Addr::new(250, 10, 20, 30).is_global(), false);
    ///
    /// // 广播地址（`255.255.255.255`）
    /// assert_eq!(Ipv4Addr::BROADCAST.is_global(), false);
    ///
    /// // 完整的概览请参阅 IANA IPv4 Special-Purpose Address Registry。
    /// ```
    #[unstable(feature = "ip", issue = "27709")]
    #[must_use]
    #[inline]
    pub const fn is_global(&self) -> bool {
        !(self.octets()[0] == 0 // "This network"
            || self.is_private()
            || self.is_shared()
            || self.is_loopback()
            || self.is_link_local()
            // 为未来协议保留的地址（`192.0.0.0/24`）。
            // .9 和 .10 在文档中标记为全局可达，因此排除在外。
            || (
                self.octets()[0] == 192 && self.octets()[1] == 0 && self.octets()[2] == 0
                && self.octets()[3] != 9 && self.octets()[3] != 10
            )
            || self.is_documentation()
            || self.is_benchmarking()
            || self.is_reserved()
            || self.is_broadcast())
    }

    /// 如果此地址属于 [IETF RFC 6598] 定义的 Shared Address Space（`100.64.0.0/10`），
    /// 则返回 [`true`]。
    ///
    /// [IETF RFC 6598]: https://tools.ietf.org/html/rfc6598
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(ip)]
    /// use std::net::Ipv4Addr;
    ///
    /// assert_eq!(Ipv4Addr::new(100, 64, 0, 0).is_shared(), true);
    /// assert_eq!(Ipv4Addr::new(100, 127, 255, 255).is_shared(), true);
    /// assert_eq!(Ipv4Addr::new(100, 128, 0, 0).is_shared(), false);
    /// ```
    #[unstable(feature = "ip", issue = "27709")]
    #[must_use]
    #[inline]
    pub const fn is_shared(&self) -> bool {
        self.octets()[0] == 100 && (self.octets()[1] & 0b1100_0000 == 0b0100_0000)
    }

    /// 如果此地址属于 `198.18.0.0/15` 范围，则返回 [`true`]；该范围保留给网络设备基准测试。
///
    /// [IETF RFC 2544] 将该范围定义为 `192.18.0.0` 到 `198.19.255.255`，
    /// 但 [errata 423] 将其更正为 `198.18.0.0/15`。
    ///
    /// [IETF RFC 2544]: https://tools.ietf.org/html/rfc2544
    /// [errata 423]: https://www.rfc-editor.org/errata/eid423
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(ip)]
    /// use std::net::Ipv4Addr;
    ///
    /// assert_eq!(Ipv4Addr::new(198, 17, 255, 255).is_benchmarking(), false);
    /// assert_eq!(Ipv4Addr::new(198, 18, 0, 0).is_benchmarking(), true);
    /// assert_eq!(Ipv4Addr::new(198, 19, 255, 255).is_benchmarking(), true);
    /// assert_eq!(Ipv4Addr::new(198, 20, 0, 0).is_benchmarking(), false);
    /// ```
    #[unstable(feature = "ip", issue = "27709")]
    #[must_use]
    #[inline]
    pub const fn is_benchmarking(&self) -> bool {
        self.octets()[0] == 198 && (self.octets()[1] & 0xfe) == 18
    }

    /// 如果此地址被 IANA 保留给未来使用，则返回 [`true`]。
///
    /// [IETF RFC 1112] 将 reserved 地址块定义为 `240.0.0.0/4`。该范围通常包含
    /// broadcast 地址 `255.255.255.255`，但本实现明确将其排除，因为它显然不是
    /// 保留给未来使用的地址。
    ///
    /// [IETF RFC 1112]: https://tools.ietf.org/html/rfc1112
    ///
    /// # 警告
///
    /// 随着 IANA 分配新地址，本方法会随之更新。依赖旧版本本方法的代码中，
    /// 原本不属于 reserved 的地址可能会在新版本中被视为 reserved。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(ip)]
    /// use std::net::Ipv4Addr;
    ///
    /// assert_eq!(Ipv4Addr::new(240, 0, 0, 0).is_reserved(), true);
    /// assert_eq!(Ipv4Addr::new(255, 255, 255, 254).is_reserved(), true);
    ///
    /// assert_eq!(Ipv4Addr::new(239, 255, 255, 255).is_reserved(), false);
    /// // 本实现不将广播地址视为保留供将来使用
    /// assert_eq!(Ipv4Addr::new(255, 255, 255, 255).is_reserved(), false);
    /// ```
    #[unstable(feature = "ip", issue = "27709")]
    #[must_use]
    #[inline]
    pub const fn is_reserved(&self) -> bool {
        self.octets()[0] & 240 == 240 && !self.is_broadcast()
    }

    /// 如果这是 multicast 地址（`224.0.0.0/4`），则返回 [`true`]。
///
    /// multicast 地址的最高有效 octet 位于 `224` 到 `239` 之间，由 [IETF RFC 5771] 定义。
    ///
    /// [IETF RFC 5771]: https://tools.ietf.org/html/rfc5771
    ///
    /// # 示例
    ///
    /// ```
    /// use std::net::Ipv4Addr;
    ///
    /// assert_eq!(Ipv4Addr::new(224, 254, 0, 0).is_multicast(), true);
    /// assert_eq!(Ipv4Addr::new(236, 168, 10, 65).is_multicast(), true);
    /// assert_eq!(Ipv4Addr::new(172, 16, 10, 65).is_multicast(), false);
    /// ```
    #[rustc_const_stable(feature = "const_ip_50", since = "1.50.0")]
    #[stable(since = "1.7.0", feature = "ip_17")]
    #[must_use]
    #[inline]
    pub const fn is_multicast(&self) -> bool {
        self.octets()[0] >= 224 && self.octets()[0] <= 239
    }

    /// 如果这是 broadcast 地址（`255.255.255.255`），则返回 [`true`]。
///
    /// 按 [IETF RFC 919] 定义，broadcast 地址的所有 octet 都设置为 `255`。
    ///
    /// [IETF RFC 919]: https://tools.ietf.org/html/rfc919
    ///
    /// # 示例
    ///
    /// ```
    /// use std::net::Ipv4Addr;
    ///
    /// assert_eq!(Ipv4Addr::new(255, 255, 255, 255).is_broadcast(), true);
    /// assert_eq!(Ipv4Addr::new(236, 168, 10, 65).is_broadcast(), false);
    /// ```
    #[rustc_const_stable(feature = "const_ip_50", since = "1.50.0")]
    #[stable(since = "1.7.0", feature = "ip_17")]
    #[must_use]
    #[inline]
    pub const fn is_broadcast(&self) -> bool {
        u32::from_be_bytes(self.octets()) == u32::from_be_bytes(Self::BROADCAST.octets())
    }

    /// 如果此地址位于保留给文档示例使用的范围内，则返回 [`true`]。
///
    /// 这些范围定义于 [IETF RFC 5737]：
    ///
    /// - `192.0.2.0/24` (TEST-NET-1)
    /// - `198.51.100.0/24` (TEST-NET-2)
    /// - `203.0.113.0/24` (TEST-NET-3)
    ///
    /// [IETF RFC 5737]: https://tools.ietf.org/html/rfc5737
    ///
    /// # 示例
    ///
    /// ```
    /// use std::net::Ipv4Addr;
    ///
    /// assert_eq!(Ipv4Addr::new(192, 0, 2, 255).is_documentation(), true);
    /// assert_eq!(Ipv4Addr::new(198, 51, 100, 65).is_documentation(), true);
    /// assert_eq!(Ipv4Addr::new(203, 0, 113, 6).is_documentation(), true);
    /// assert_eq!(Ipv4Addr::new(193, 34, 17, 19).is_documentation(), false);
    /// ```
    #[rustc_const_stable(feature = "const_ip_50", since = "1.50.0")]
    #[stable(since = "1.7.0", feature = "ip_17")]
    #[must_use]
    #[inline]
    pub const fn is_documentation(&self) -> bool {
        matches!(self.octets(), [192, 0, 2, _] | [198, 51, 100, _] | [203, 0, 113, _])
    }

    /// 将此地址转换为 [IPv4-compatible] [`IPv6` address]。
///
    /// `a.b.c.d` 会变为 `::a.b.c.d`。
///
    /// 注意，IPv4-compatible 地址已经正式废弃。除非出于遗留兼容原因明确需要
    /// IPv4-compatible 地址，否则应考虑改用 `to_ipv6_mapped`。
    ///
    /// [IPv4-compatible]: Ipv6Addr#ipv4-compatible-ipv6-addresses
    /// [`IPv6` address]: Ipv6Addr
    ///
    /// # 示例
    ///
    /// ```
    /// use std::net::{Ipv4Addr, Ipv6Addr};
    ///
    /// assert_eq!(
    ///     Ipv4Addr::new(192, 0, 2, 255).to_ipv6_compatible(),
    ///     Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0xc000, 0x2ff)
    /// );
    /// ```
    #[rustc_const_stable(feature = "const_ip_50", since = "1.50.0")]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[must_use = "this returns the result of the operation, \
                  without modifying the original"]
    #[inline]
    pub const fn to_ipv6_compatible(&self) -> Ipv6Addr {
        let [a, b, c, d] = self.octets();
        Ipv6Addr { octets: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, a, b, c, d] }
    }

    /// 将此地址转换为 [IPv4-mapped] [`IPv6` address]。
///
    /// `a.b.c.d` 会变为 `::ffff:a.b.c.d`。
    ///
    /// [IPv4-mapped]: Ipv6Addr#ipv4-mapped-ipv6-addresses
    /// [`IPv6` address]: Ipv6Addr
    ///
    /// # 示例
    ///
    /// ```
    /// use std::net::{Ipv4Addr, Ipv6Addr};
    ///
    /// assert_eq!(Ipv4Addr::new(192, 0, 2, 255).to_ipv6_mapped(),
    ///            Ipv6Addr::new(0, 0, 0, 0, 0, 0xffff, 0xc000, 0x2ff));
    /// ```
    #[rustc_const_stable(feature = "const_ip_50", since = "1.50.0")]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[must_use = "this returns the result of the operation, \
                  without modifying the original"]
    #[inline]
    pub const fn to_ipv6_mapped(&self) -> Ipv6Addr {
        let [a, b, c, d] = self.octets();
        Ipv6Addr { octets: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xFF, 0xFF, a, b, c, d] }
    }
}

#[stable(feature = "ip_addr", since = "1.7.0")]
impl fmt::Display for IpAddr {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            IpAddr::V4(ip) => ip.fmt(fmt),
            IpAddr::V6(ip) => ip.fmt(fmt),
        }
    }
}

#[stable(feature = "ip_addr", since = "1.7.0")]
impl fmt::Debug for IpAddr {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, fmt)
    }
}

#[stable(feature = "ip_from_ip", since = "1.16.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl const From<Ipv4Addr> for IpAddr {
    /// 将此地址复制为新的 `IpAddr::V4`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::net::{IpAddr, Ipv4Addr};
    ///
    /// let addr = Ipv4Addr::new(127, 0, 0, 1);
    ///
    /// assert_eq!(
    ///     IpAddr::V4(addr),
    ///     IpAddr::from(addr)
    /// )
    /// ```
    #[inline]
    fn from(ipv4: Ipv4Addr) -> IpAddr {
        IpAddr::V4(ipv4)
    }
}

#[stable(feature = "ip_from_ip", since = "1.16.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl const From<Ipv6Addr> for IpAddr {
    /// 将此地址复制为新的 `IpAddr::V6`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::net::{IpAddr, Ipv6Addr};
    ///
    /// let addr = Ipv6Addr::new(0, 0, 0, 0, 0, 0xffff, 0xc00a, 0x2ff);
    ///
    /// assert_eq!(
    ///     IpAddr::V6(addr),
    ///     IpAddr::from(addr)
    /// );
    /// ```
    #[inline]
    fn from(ipv6: Ipv6Addr) -> IpAddr {
        IpAddr::V6(ipv6)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl fmt::Display for Ipv4Addr {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        let octets = self.octets();

        // 如果没有对齐要求，则直接把 IP 地址写入 `f`。
        // 否则先写入本地缓冲区，再调用 `f.pad`。
        if fmt.precision().is_none() && fmt.width().is_none() {
            write!(fmt, "{}.{}.{}.{}", octets[0], octets[1], octets[2], octets[3])
        } else {
            const LONGEST_IPV4_ADDR: &str = "255.255.255.255";

            let mut buf = DisplayBuffer::<{ LONGEST_IPV4_ADDR.len() }>::new();
            // 缓冲区足以容纳最长的 IPv4 地址，因此这里不应失败。
            write!(buf, "{}.{}.{}.{}", octets[0], octets[1], octets[2], octets[3]).unwrap();

            fmt.pad(buf.as_str())
        }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl fmt::Debug for Ipv4Addr {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, fmt)
    }
}

#[stable(feature = "ip_cmp", since = "1.16.0")]
impl PartialEq<Ipv4Addr> for IpAddr {
    #[inline]
    fn eq(&self, other: &Ipv4Addr) -> bool {
        match self {
            IpAddr::V4(v4) => v4 == other,
            IpAddr::V6(_) => false,
        }
    }
}

#[stable(feature = "ip_cmp", since = "1.16.0")]
impl PartialEq<IpAddr> for Ipv4Addr {
    #[inline]
    fn eq(&self, other: &IpAddr) -> bool {
        match other {
            IpAddr::V4(v4) => self == v4,
            IpAddr::V6(_) => false,
        }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl PartialOrd for Ipv4Addr {
    #[inline]
    fn partial_cmp(&self, other: &Ipv4Addr) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[stable(feature = "ip_cmp", since = "1.16.0")]
impl PartialOrd<Ipv4Addr> for IpAddr {
    #[inline]
    fn partial_cmp(&self, other: &Ipv4Addr) -> Option<Ordering> {
        match self {
            IpAddr::V4(v4) => v4.partial_cmp(other),
            IpAddr::V6(_) => Some(Ordering::Greater),
        }
    }
}

#[stable(feature = "ip_cmp", since = "1.16.0")]
impl PartialOrd<IpAddr> for Ipv4Addr {
    #[inline]
    fn partial_cmp(&self, other: &IpAddr) -> Option<Ordering> {
        match other {
            IpAddr::V4(v4) => self.partial_cmp(v4),
            IpAddr::V6(_) => Some(Ordering::Less),
        }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl Ord for Ipv4Addr {
    #[inline]
    fn cmp(&self, other: &Ipv4Addr) -> Ordering {
        self.octets.cmp(&other.octets)
    }
}

#[stable(feature = "ip_u32", since = "1.1.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl const From<Ipv4Addr> for u32 {
    /// 使用 [`Ipv4Addr::to_bits`] 将 IPv4 地址转换为本机字节序的 `u32`。
    #[inline]
    fn from(ip: Ipv4Addr) -> u32 {
        ip.to_bits()
    }
}

#[stable(feature = "ip_u32", since = "1.1.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl const From<u32> for Ipv4Addr {
    /// 使用 [`Ipv4Addr::from_bits`] 将本机字节序的 `u32` 转换为 IPv4 地址。
    #[inline]
    fn from(ip: u32) -> Ipv4Addr {
        Ipv4Addr::from_bits(ip)
    }
}

#[stable(feature = "from_slice_v4", since = "1.9.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl const From<[u8; 4]> for Ipv4Addr {
    /// 根据包含四个元素的字节数组创建 `Ipv4Addr`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::net::Ipv4Addr;
    ///
    /// let addr = Ipv4Addr::from([13u8, 12u8, 11u8, 10u8]);
    /// assert_eq!(Ipv4Addr::new(13, 12, 11, 10), addr);
    /// ```
    #[inline]
    fn from(octets: [u8; 4]) -> Ipv4Addr {
        Ipv4Addr { octets }
    }
}

#[stable(feature = "ip_from_slice", since = "1.17.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl const From<[u8; 4]> for IpAddr {
    /// 根据包含四个元素的字节数组创建 `IpAddr::V4`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::net::{IpAddr, Ipv4Addr};
    ///
    /// let addr = IpAddr::from([13u8, 12u8, 11u8, 10u8]);
    /// assert_eq!(IpAddr::V4(Ipv4Addr::new(13, 12, 11, 10)), addr);
    /// ```
    #[inline]
    fn from(octets: [u8; 4]) -> IpAddr {
        IpAddr::V4(Ipv4Addr::from(octets))
    }
}

impl Ipv6Addr {
    /// 根据八个 16 位 segment 创建新的 IPv6 地址。
///
    /// 结果表示 IP 地址 `a:b:c:d:e:f:g:h`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::net::Ipv6Addr;
    ///
    /// let addr = Ipv6Addr::new(0, 0, 0, 0, 0, 0xffff, 0xc00a, 0x2ff);
    /// ```
    #[rustc_const_stable(feature = "const_ip_32", since = "1.32.0")]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[must_use]
    #[inline]
    pub const fn new(a: u16, b: u16, c: u16, d: u16, e: u16, f: u16, g: u16, h: u16) -> Ipv6Addr {
        let addr16 = [
            a.to_be(),
            b.to_be(),
            c.to_be(),
            d.to_be(),
            e.to_be(),
            f.to_be(),
            g.to_be(),
            h.to_be(),
        ];
        Ipv6Addr {
            // `addr16` 中的所有元素都是 big endian。
            // SAFETY: `[u16; 8]` 总是可以安全地转置为 `[u8; 16]`。
            octets: unsafe { transmute::<_, [u8; 16]>(addr16) },
        }
    }

    /// IPv6 地址的位数。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::net::Ipv6Addr;
    ///
    /// assert_eq!(Ipv6Addr::BITS, 128);
    /// ```
    #[stable(feature = "ip_bits", since = "1.80.0")]
    pub const BITS: u32 = 128;

    /// 将 IPv6 地址转换为使用本机字节序的 `u128` 表示。
///
    /// 虽然 IPv6 地址按 big-endian 定义，但返回的 `u128` 值使用目标平台的本机字节序。
    /// 也就是说，这个 `u128` 是 IPv6 地址的整数表示，而不是把 IPv6 地址的 big-endian
    /// 位串按平台字节序重新解释得到的整数。因此，无论目标平台采用哪种端序，对返回值应用
    /// `0xffffffffffffffffffffffffffff0000_u128` 掩码都会把地址中的最后一个 segment 置为 0。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::net::Ipv6Addr;
    ///
    /// let addr = Ipv6Addr::new(
    ///     0x1020, 0x3040, 0x5060, 0x7080,
    ///     0x90A0, 0xB0C0, 0xD0E0, 0xF00D,
    /// );
    /// assert_eq!(0x102030405060708090A0B0C0D0E0F00D_u128, addr.to_bits());
    /// ```
    ///
    /// ```
    /// use std::net::Ipv6Addr;
    ///
    /// let addr = Ipv6Addr::new(
    ///     0x1020, 0x3040, 0x5060, 0x7080,
    ///     0x90A0, 0xB0C0, 0xD0E0, 0xF00D,
    /// );
    /// let addr_bits = addr.to_bits() & 0xffffffffffffffffffffffffffff0000_u128;
    /// assert_eq!(
    ///     Ipv6Addr::new(
    ///         0x1020, 0x3040, 0x5060, 0x7080,
    ///         0x90A0, 0xB0C0, 0xD0E0, 0x0000,
    ///     ),
    ///     Ipv6Addr::from_bits(addr_bits));
    ///
    /// ```
    #[rustc_const_stable(feature = "ip_bits", since = "1.80.0")]
    #[stable(feature = "ip_bits", since = "1.80.0")]
    #[must_use]
    #[inline]
    pub const fn to_bits(self) -> u128 {
        u128::from_be_bytes(self.octets)
    }

    /// 将使用本机字节序的 `u128` 转换为 IPv6 地址。
///
    /// 端序语义请参见 [`Ipv6Addr::to_bits`]。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::net::Ipv6Addr;
    ///
    /// let addr = Ipv6Addr::from_bits(0x102030405060708090A0B0C0D0E0F00D_u128);
    /// assert_eq!(
    ///     Ipv6Addr::new(
    ///         0x1020, 0x3040, 0x5060, 0x7080,
    ///         0x90A0, 0xB0C0, 0xD0E0, 0xF00D,
    ///     ),
    ///     addr);
    /// ```
    #[rustc_const_stable(feature = "ip_bits", since = "1.80.0")]
    #[stable(feature = "ip_bits", since = "1.80.0")]
    #[must_use]
    #[inline]
    pub const fn from_bits(bits: u128) -> Ipv6Addr {
        Ipv6Addr { octets: bits.to_be_bytes() }
    }

    /// 表示 localhost 的 IPv6 地址：`::1`。
///
    /// 这对应其他语言中的常量 `IN6ADDR_LOOPBACK_INIT` 或 `in6addr_loopback`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::net::Ipv6Addr;
    ///
    /// let addr = Ipv6Addr::LOCALHOST;
    /// assert_eq!(addr, Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1));
    /// ```
    #[doc(alias = "IN6ADDR_LOOPBACK_INIT")]
    #[doc(alias = "in6addr_loopback")]
    #[stable(feature = "ip_constructors", since = "1.30.0")]
    pub const LOCALHOST: Self = Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1);

    /// 表示 unspecified 地址的 IPv6 地址：`::`。
///
    /// 这对应其他语言中的常量 `IN6ADDR_ANY_INIT` 或 `in6addr_any`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::net::Ipv6Addr;
    ///
    /// let addr = Ipv6Addr::UNSPECIFIED;
    /// assert_eq!(addr, Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0));
    /// ```
    #[doc(alias = "IN6ADDR_ANY_INIT")]
    #[doc(alias = "in6addr_any")]
    #[stable(feature = "ip_constructors", since = "1.30.0")]
    pub const UNSPECIFIED: Self = Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0);

    /// 返回组成此地址的八个 16 位 segment。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::net::Ipv6Addr;
    ///
    /// assert_eq!(Ipv6Addr::new(0, 0, 0, 0, 0, 0xffff, 0xc00a, 0x2ff).segments(),
    ///            [0, 0, 0, 0, 0, 0xffff, 0xc00a, 0x2ff]);
    /// ```
    #[rustc_const_stable(feature = "const_ip_50", since = "1.50.0")]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[must_use]
    #[inline]
    pub const fn segments(&self) -> [u16; 8] {
        // `self.octets` 中的所有元素都必须是 big endian。
        // SAFETY: `[u8; 16]` 总是可以安全地转置为 `[u16; 8]`。
        let [a, b, c, d, e, f, g, h] = unsafe { transmute::<_, [u16; 8]>(self.octets) };
        // 这里需要本机端序的 `u16`。
        [
            u16::from_be(a),
            u16::from_be(b),
            u16::from_be(c),
            u16::from_be(d),
            u16::from_be(e),
            u16::from_be(f),
            u16::from_be(g),
            u16::from_be(h),
        ]
    }

    /// 根据包含八个元素的 16 位数组创建 `Ipv6Addr`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::net::Ipv6Addr;
    ///
    /// let addr = Ipv6Addr::from_segments([
    ///     0x20du16, 0x20cu16, 0x20bu16, 0x20au16,
    ///     0x209u16, 0x208u16, 0x207u16, 0x206u16,
    /// ]);
    /// assert_eq!(
    ///     Ipv6Addr::new(
    ///         0x20d, 0x20c, 0x20b, 0x20a,
    ///         0x209, 0x208, 0x207, 0x206,
    ///     ),
    ///     addr
    /// );
    /// ```
    #[stable(feature = "ip_from", since = "1.91.0")]
    #[rustc_const_stable(feature = "ip_from", since = "1.91.0")]
    #[must_use]
    #[inline]
    pub const fn from_segments(segments: [u16; 8]) -> Ipv6Addr {
        let [a, b, c, d, e, f, g, h] = segments;
        Ipv6Addr::new(a, b, c, d, e, f, g, h)
    }

    /// 如果是特殊的 “unspecified” 地址（`::`），则返回 [`true`]。
///
    /// 该属性定义于 [IETF RFC 4291]。
    ///
    /// [IETF RFC 4291]: https://tools.ietf.org/html/rfc4291
    ///
    /// # 示例
    ///
    /// ```
    /// use std::net::Ipv6Addr;
    ///
    /// assert_eq!(Ipv6Addr::new(0, 0, 0, 0, 0, 0xffff, 0xc00a, 0x2ff).is_unspecified(), false);
    /// assert_eq!(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0).is_unspecified(), true);
    /// ```
    #[rustc_const_stable(feature = "const_ip_50", since = "1.50.0")]
    #[stable(since = "1.7.0", feature = "ip_17")]
    #[must_use]
    #[inline]
    pub const fn is_unspecified(&self) -> bool {
        u128::from_be_bytes(self.octets()) == u128::from_be_bytes(Ipv6Addr::UNSPECIFIED.octets())
    }

    /// 如果这是 [loopback address]（`::1`），则返回 [`true`]；
    /// 该地址定义于 [IETF RFC 4291 section 2.5.3]。
///
    /// 与 IPv4 不同，IPv6 只有一个 loopback 地址。
    ///
    /// [loopback address]: Ipv6Addr::LOCALHOST
    /// [IETF RFC 4291 section 2.5.3]: https://tools.ietf.org/html/rfc4291#section-2.5.3
    ///
    /// # 示例
    ///
    /// ```
    /// use std::net::Ipv6Addr;
    ///
    /// assert_eq!(Ipv6Addr::new(0, 0, 0, 0, 0, 0xffff, 0xc00a, 0x2ff).is_loopback(), false);
    /// assert_eq!(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 0x1).is_loopback(), true);
    /// ```
    #[rustc_const_stable(feature = "const_ip_50", since = "1.50.0")]
    #[stable(since = "1.7.0", feature = "ip_17")]
    #[must_use]
    #[inline]
    pub const fn is_loopback(&self) -> bool {
        u128::from_be_bytes(self.octets()) == u128::from_be_bytes(Ipv6Addr::LOCALHOST.octets())
    }

    /// 如果该地址按 [IANA IPv6 Special-Purpose Address Registry] 看起来是全局可达的，
    /// 则返回 [`true`]。
///
    /// 地址在实际网络中能否到达取决于具体网络配置。除非某个 IPv6 地址被明确标记为
    /// *非*全局可达，大多数 IPv6 地址都视为全局可达。
///
    /// 以下列出一些重要但并非穷尽的非全局可达地址：
    /// - [unspecified address]（[`is_unspecified`](Ipv6Addr::is_unspecified)）
    /// - [loopback address]（[`is_loopback`](Ipv6Addr::is_loopback)）
    /// - IPv4-mapped 地址
    /// - 保留给基准测试的地址（[`is_benchmarking`](Ipv6Addr::is_benchmarking)）
    /// - 保留给文档示例的地址（[`is_documentation`](Ipv6Addr::is_documentation)）
    /// - unique local 地址（[`is_unique_local`](Ipv6Addr::is_unique_local)）
    /// - 具有 link-local scope 的 unicast 地址（[`is_unicast_link_local`](Ipv6Addr::is_unicast_link_local)）
///
    /// 哪些地址全局可达的完整概览，请参见 [IANA IPv6 Special-Purpose Address Registry] 中的表格。
///
    /// 注意，地址具有 global scope 并不等同于全局可达；这两个概念没有直接对应关系。
    /// 有些地址具有 global scope 却不是全局可达（例如 unique local 地址），也有些地址
    /// 全局可达但没有 global scope（例如非 global scope 的 multicast 地址）。
    ///
    /// [IANA IPv6 Special-Purpose Address Registry]: https://www.iana.org/assignments/iana-ipv6-special-registry/iana-ipv6-special-registry.xhtml
    /// [unspecified address]: Ipv6Addr::UNSPECIFIED
    /// [loopback address]: Ipv6Addr::LOCALHOST
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(ip)]
    ///
    /// use std::net::Ipv6Addr;
    ///
    /// // 大多数 IPv6 地址都是全局可达的：
    /// assert_eq!(Ipv6Addr::new(0x26, 0, 0x1c9, 0, 0, 0xafc8, 0x10, 0x1).is_global(), true);
    ///
    /// // 不过有些地址被赋予了特殊含义，
    /// // 使它们变得不可全局到达。一些例子如下：
    ///
    /// // 未指定地址（`::`）
    /// assert_eq!(Ipv6Addr::UNSPECIFIED.is_global(), false);
    ///
    /// // 环回地址（`::1`）
    /// assert_eq!(Ipv6Addr::LOCALHOST.is_global(), false);
    ///
    /// // IPv4 映射地址（`::ffff:0:0/96`）
    /// assert_eq!(Ipv6Addr::new(0, 0, 0, 0, 0, 0xffff, 0xc00a, 0x2ff).is_global(), false);
    ///
    /// // 保留供基准测试使用的地址（`2001:2::/48`）
    /// assert_eq!(Ipv6Addr::new(0x2001, 2, 0, 0, 0, 0, 0, 1,).is_global(), false);
    ///
    /// // 保留供文档使用的地址（`2001:db8::/32` 和 `3fff::/20`）
    /// assert_eq!(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1).is_global(), false);
    /// assert_eq!(Ipv6Addr::new(0x3fff, 0, 0, 0, 0, 0, 0, 0).is_global(), false);
    ///
    /// // 唯一本地地址（`fc00::/7`）
    /// assert_eq!(Ipv6Addr::new(0xfc02, 0, 0, 0, 0, 0, 0, 1).is_global(), false);
    ///
    /// // 具有链路本地作用域的单播地址（`fe80::/10`）
    /// assert_eq!(Ipv6Addr::new(0xfe81, 0, 0, 0, 0, 0, 0, 1).is_global(), false);
    ///
    /// // 完整的概览请参阅 IANA IPv6 Special-Purpose Address Registry。
    /// ```
    #[unstable(feature = "ip", issue = "27709")]
    #[must_use]
    #[inline]
    pub const fn is_global(&self) -> bool {
        !(self.is_unspecified()
            || self.is_loopback()
            // IPv4-mapped Address（`::ffff:0:0/96`）
            || matches!(self.segments(), [0, 0, 0, 0, 0, 0xffff, _, _])
            // IPv4-IPv6 Translat.（`64:ff9b:1::/48`）
            || matches!(self.segments(), [0x64, 0xff9b, 1, _, _, _, _, _])
            // Discard-Only Address Block（`100::/64`）
            || matches!(self.segments(), [0x100, 0, 0, 0, _, _, _, _])
            // IETF Protocol Assignments（`2001::/23`）
            || (matches!(self.segments(), [0x2001, b, _, _, _, _, _, _] if b < 0x200)
                && !(
                    // Port Control Protocol Anycast（`2001:1::1`）
                    u128::from_be_bytes(self.octets()) == 0x2001_0001_0000_0000_0000_0000_0000_0001
                    // Traversal Using Relays around NAT Anycast（`2001:1::2`）
                    || u128::from_be_bytes(self.octets()) == 0x2001_0001_0000_0000_0000_0000_0000_0002
                    // AMT（`2001:3::/32`）
                    || matches!(self.segments(), [0x2001, 3, _, _, _, _, _, _])
                    // AS112-v6（`2001:4:112::/48`）
                    || matches!(self.segments(), [0x2001, 4, 0x112, _, _, _, _, _])
                    // ORCHIDv2（`2001:20::/28`）
                    // Drone Remote ID Protocol Entity Tags（DETs）Prefix（`2001:30::/28`）`
                    || matches!(self.segments(), [0x2001, b, _, _, _, _, _, _] if b >= 0x20 && b <= 0x3F)
                ))
            // 6to4（`2002::/16`）没有明确记录为全局可达，IANA 标注为 N/A。
            || matches!(self.segments(), [0x2002, _, _, _, _, _, _, _])
            || self.is_documentation()
            // Segment Routing（SRv6）SIDs（`5f00::/16`）
            || matches!(self.segments(), [0x5f00, ..])
            || self.is_unique_local()
            || self.is_unicast_link_local())
    }

    /// 如果这是 unique local 地址（`fc00::/7`），则返回 [`true`]。
///
    /// 该属性定义于 [IETF RFC 4193]。
    ///
    /// [IETF RFC 4193]: https://tools.ietf.org/html/rfc4193
    ///
    /// # 示例
    ///
    /// ```
    /// use std::net::Ipv6Addr;
    ///
    /// assert_eq!(Ipv6Addr::new(0, 0, 0, 0, 0, 0xffff, 0xc00a, 0x2ff).is_unique_local(), false);
    /// assert_eq!(Ipv6Addr::new(0xfc02, 0, 0, 0, 0, 0, 0, 0).is_unique_local(), true);
    /// ```
    #[must_use]
    #[inline]
    #[stable(feature = "ipv6_is_unique_local", since = "1.84.0")]
    #[rustc_const_stable(feature = "ipv6_is_unique_local", since = "1.84.0")]
    pub const fn is_unique_local(&self) -> bool {
        (self.segments()[0] & 0xfe00) == 0xfc00
    }

    /// 如果这是 [IETF RFC 4291] 定义的 unicast 地址，则返回 [`true`]。
    /// 任何不是 [multicast address]（`ff00::/8`）的地址都是 unicast 地址。
    ///
    /// [IETF RFC 4291]: https://tools.ietf.org/html/rfc4291
    /// [multicast address]: Ipv6Addr::is_multicast
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(ip)]
    ///
    /// use std::net::Ipv6Addr;
    ///
    /// // 未指定地址和环回地址都是单播地址。
    /// assert_eq!(Ipv6Addr::UNSPECIFIED.is_unicast(), true);
    /// assert_eq!(Ipv6Addr::LOCALHOST.is_unicast(), true);
    ///
    /// // 任何不是组播地址（`ff00::/8`）的地址都是单播地址。
    /// assert_eq!(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 0).is_unicast(), true);
    /// assert_eq!(Ipv6Addr::new(0xff00, 0, 0, 0, 0, 0, 0, 0).is_unicast(), false);
    /// ```
    #[unstable(feature = "ip", issue = "27709")]
    #[must_use]
    #[inline]
    pub const fn is_unicast(&self) -> bool {
        !self.is_multicast()
    }

    /// 如果该地址是 [RFC 4291] 定义的、具有 link-local scope 的 unicast 地址，则返回 `true`。
///
    /// 按 [RFC 4291 section 2.4]，带 `fe80::/10` 前缀的 unicast 地址具有 link-local scope。
    /// 注意，这覆盖的地址多于 [RFC 4291 section 2.5.6] 中定义的范围；后者描述的
    /// “Link-Local IPv6 Unicast Addresses” 使用下面更严格的格式：
    ///
    /// ```text
    /// | 10 bits  |         54 bits         |          64 bits           |
    /// +----------+-------------------------+----------------------------+
    /// |1111111010|           0             |       interface ID         |
    /// +----------+-------------------------+----------------------------+
    /// ```
    /// 因此，虽然应用目前会遇到的 link-local scope 地址都在 `fe80::/64` 中，
    /// 但未来发布的新标准可能改变这一点。`fe80::/10` 中可能分配更多地址，
    /// 这些地址也会具有 link-local scope。
///
    /// 还要注意，[RFC 4291 section 2.5.3] 提到 [loopback address]（`::1`）
    /// “it is treated as having Link-Local scope”，但这并不表示 loopback 地址实际上
    /// 具有 link-local scope；本方法会对它返回 `false`。
    ///
    /// [RFC 4291]: https://tools.ietf.org/html/rfc4291
    /// [RFC 4291 section 2.4]: https://tools.ietf.org/html/rfc4291#section-2.4
    /// [RFC 4291 section 2.5.3]: https://tools.ietf.org/html/rfc4291#section-2.5.3
    /// [RFC 4291 section 2.5.6]: https://tools.ietf.org/html/rfc4291#section-2.5.6
    /// [loopback address]: Ipv6Addr::LOCALHOST
    ///
    /// # 示例
    ///
    /// ```
    /// use std::net::Ipv6Addr;
    ///
    /// // 环回地址（`::1`）实际上并不具有链路本地作用域。
    /// assert_eq!(Ipv6Addr::LOCALHOST.is_unicast_link_local(), false);
    ///
    /// // 只有 `fe80::/10` 范围内的地址才具有链路本地作用域。
    /// assert_eq!(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 0).is_unicast_link_local(), false);
    /// assert_eq!(Ipv6Addr::new(0xfe80, 0, 0, 0, 0, 0, 0, 0).is_unicast_link_local(), true);
    ///
    /// // 更严格的 `fe80::/64` 之外的地址同样具有链路本地作用域。
    /// assert_eq!(Ipv6Addr::new(0xfe80, 0, 0, 1, 0, 0, 0, 0).is_unicast_link_local(), true);
    /// assert_eq!(Ipv6Addr::new(0xfe81, 0, 0, 0, 0, 0, 0, 0).is_unicast_link_local(), true);
    /// ```
    #[must_use]
    #[inline]
    #[stable(feature = "ipv6_is_unique_local", since = "1.84.0")]
    #[rustc_const_stable(feature = "ipv6_is_unique_local", since = "1.84.0")]
    pub const fn is_unicast_link_local(&self) -> bool {
        (self.segments()[0] & 0xffc0) == 0xfe80
    }

    /// 如果这是保留给文档示例使用的地址（`2001:db8::/32` 和 `3fff::/20`），
    /// 则返回 [`true`]。
///
    /// 该属性由 [IETF RFC 3849] 和 [IETF RFC 9637] 定义。
    ///
    /// [IETF RFC 3849]: https://tools.ietf.org/html/rfc3849
    /// [IETF RFC 9637]: https://tools.ietf.org/html/rfc9637
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(ip)]
    ///
    /// use std::net::Ipv6Addr;
    ///
    /// assert_eq!(Ipv6Addr::new(0, 0, 0, 0, 0, 0xffff, 0xc00a, 0x2ff).is_documentation(), false);
    /// assert_eq!(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 0).is_documentation(), true);
    /// assert_eq!(Ipv6Addr::new(0x3fff, 0, 0, 0, 0, 0, 0, 0).is_documentation(), true);
    /// ```
    #[unstable(feature = "ip", issue = "27709")]
    #[must_use]
    #[inline]
    pub const fn is_documentation(&self) -> bool {
        matches!(self.segments(), [0x2001, 0xdb8, ..] | [0x3fff, 0..=0x0fff, ..])
    }

    /// 如果这是保留给基准测试使用的地址（`2001:2::/48`），则返回 [`true`]。
///
    /// 该属性定义于 [IETF RFC 5180]；该 RFC 误将范围写为 `2001:0200::/48`。
    /// [IETF RFC Errata 1752] 将其更正为 `2001:0002::/48`。
    ///
    /// [IETF RFC 5180]: https://tools.ietf.org/html/rfc5180
    /// [IETF RFC Errata 1752]: https://www.rfc-editor.org/errata_search.php?eid=1752
    ///
    /// ```
    /// #![feature(ip)]
    ///
    /// use std::net::Ipv6Addr;
    ///
    /// assert_eq!(Ipv6Addr::new(0, 0, 0, 0, 0, 0xffff, 0xc613, 0x0).is_benchmarking(), false);
    /// assert_eq!(Ipv6Addr::new(0x2001, 0x2, 0, 0, 0, 0, 0, 0).is_benchmarking(), true);
    /// ```
    #[unstable(feature = "ip", issue = "27709")]
    #[must_use]
    #[inline]
    pub const fn is_benchmarking(&self) -> bool {
        (self.segments()[0] == 0x2001) && (self.segments()[1] == 0x2) && (self.segments()[2] == 0)
    }

    /// 如果该地址是全局可路由的 unicast 地址，则返回 [`true`]。
///
    /// 以下情况返回 false：
///
    /// - loopback 地址
    /// - link-local 地址
    /// - unique local 地址
    /// - unspecified 地址
    /// - 保留给文档示例使用的地址范围
///
    /// 根据 [RFC 4291 section 2.5.7]，本方法会对 site-local 地址返回 [`true`]。
    ///
    /// ```no_rust
    /// The special behavior of [the site-local unicast] prefix defined in [RFC3513] must no longer
    /// be supported in new implementations (i.e., new implementations must treat this prefix as
    /// Global Unicast).
    /// ```
    ///
    /// [RFC 4291 section 2.5.7]: https://tools.ietf.org/html/rfc4291#section-2.5.7
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(ip)]
    ///
    /// use std::net::Ipv6Addr;
    ///
    /// assert_eq!(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 0).is_unicast_global(), false);
    /// assert_eq!(Ipv6Addr::new(0, 0, 0, 0, 0, 0xffff, 0xc00a, 0x2ff).is_unicast_global(), true);
    /// ```
    #[unstable(feature = "ip", issue = "27709")]
    #[must_use]
    #[inline]
    pub const fn is_unicast_global(&self) -> bool {
        self.is_unicast()
            && !self.is_loopback()
            && !self.is_unicast_link_local()
            && !self.is_unique_local()
            && !self.is_unspecified()
            && !self.is_documentation()
            && !self.is_benchmarking()
    }

    /// 如果该地址是 multicast 地址，则返回它的 multicast scope。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(ip)]
    ///
    /// use std::net::{Ipv6Addr, Ipv6MulticastScope};
    ///
    /// assert_eq!(
    ///     Ipv6Addr::new(0xff0e, 0, 0, 0, 0, 0, 0, 0).multicast_scope(),
    ///     Some(Ipv6MulticastScope::Global)
    /// );
    /// assert_eq!(Ipv6Addr::new(0, 0, 0, 0, 0, 0xffff, 0xc00a, 0x2ff).multicast_scope(), None);
    /// ```
    #[unstable(feature = "ip", issue = "27709")]
    #[must_use]
    #[inline]
    pub const fn multicast_scope(&self) -> Option<Ipv6MulticastScope> {
        if self.is_multicast() {
            match self.segments()[0] & 0x000f {
                1 => Some(Ipv6MulticastScope::InterfaceLocal),
                2 => Some(Ipv6MulticastScope::LinkLocal),
                3 => Some(Ipv6MulticastScope::RealmLocal),
                4 => Some(Ipv6MulticastScope::AdminLocal),
                5 => Some(Ipv6MulticastScope::SiteLocal),
                8 => Some(Ipv6MulticastScope::OrganizationLocal),
                14 => Some(Ipv6MulticastScope::Global),
                _ => None,
            }
        } else {
            None
        }
    }

    /// 如果这是 multicast 地址（`ff00::/8`），则返回 [`true`]。
///
    /// 该属性由 [IETF RFC 4291] 定义。
    ///
    /// [IETF RFC 4291]: https://tools.ietf.org/html/rfc4291
    ///
    /// # 示例
    ///
    /// ```
    /// use std::net::Ipv6Addr;
    ///
    /// assert_eq!(Ipv6Addr::new(0xff00, 0, 0, 0, 0, 0, 0, 0).is_multicast(), true);
    /// assert_eq!(Ipv6Addr::new(0, 0, 0, 0, 0, 0xffff, 0xc00a, 0x2ff).is_multicast(), false);
    /// ```
    #[rustc_const_stable(feature = "const_ip_50", since = "1.50.0")]
    #[stable(since = "1.7.0", feature = "ip_17")]
    #[must_use]
    #[inline]
    pub const fn is_multicast(&self) -> bool {
        (self.segments()[0] & 0xff00) == 0xff00
    }

    /// 如果该地址是 IPv4-mapped 地址（`::ffff:0:0/96`），则返回 [`true`]。
///
    /// IPv4-mapped 地址可通过 [`to_ipv4_mapped`](Ipv6Addr::to_ipv4_mapped)
    /// 转换为其规范 IPv4 地址。
    ///
    /// # 示例
    /// ```
    /// #![feature(ip)]
    ///
    /// use std::net::{Ipv4Addr, Ipv6Addr};
    ///
    /// let ipv4_mapped = Ipv4Addr::new(192, 0, 2, 255).to_ipv6_mapped();
    /// assert_eq!(ipv4_mapped.is_ipv4_mapped(), true);
    /// assert_eq!(Ipv6Addr::new(0, 0, 0, 0, 0, 0xffff, 0xc000, 0x2ff).is_ipv4_mapped(), true);
    ///
    /// assert_eq!(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 0).is_ipv4_mapped(), false);
    /// ```
    #[unstable(feature = "ip", issue = "27709")]
    #[must_use]
    #[inline]
    pub const fn is_ipv4_mapped(&self) -> bool {
        matches!(self.segments(), [0, 0, 0, 0, 0, 0xffff, _, _])
    }

    /// 如果此地址是 [IETF RFC 4291 section 2.5.5.2] 定义的 [IPv4-mapped] 地址，
    /// 则将其转换为 [`IPv4` address]；否则返回 [`None`]。
///
    /// `::ffff:a.b.c.d` 会变为 `a.b.c.d`。
    /// 所有*不是*以 `::ffff` 开头的地址都会返回 `None`。
    ///
    /// [`IPv4` address]: Ipv4Addr
    /// [IPv4-mapped]: Ipv6Addr
    /// [IETF RFC 4291 section 2.5.5.2]: https://tools.ietf.org/html/rfc4291#section-2.5.5.2
    ///
    /// # 示例
    ///
    /// ```
    /// use std::net::{Ipv4Addr, Ipv6Addr};
    ///
    /// assert_eq!(Ipv6Addr::new(0xff00, 0, 0, 0, 0, 0, 0, 0).to_ipv4_mapped(), None);
    /// assert_eq!(Ipv6Addr::new(0, 0, 0, 0, 0, 0xffff, 0xc00a, 0x2ff).to_ipv4_mapped(),
    ///            Some(Ipv4Addr::new(192, 10, 2, 255)));
    /// assert_eq!(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1).to_ipv4_mapped(), None);
    /// ```
    #[inline]
    #[must_use = "this returns the result of the operation, \
                  without modifying the original"]
    #[stable(feature = "ipv6_to_ipv4_mapped", since = "1.63.0")]
    #[rustc_const_stable(feature = "const_ipv6_to_ipv4_mapped", since = "1.75.0")]
    pub const fn to_ipv4_mapped(&self) -> Option<Ipv4Addr> {
        match self.octets() {
            [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xff, 0xff, a, b, c, d] => {
                Some(Ipv4Addr::new(a, b, c, d))
            }
            _ => None,
        }
    }

    /// 如果此地址是 [IETF RFC 4291 section 2.5.5.1] 定义的 [IPv4-compatible] 地址，
    /// 或是 [IETF RFC 4291 section 2.5.5.2] 定义的 [IPv4-mapped] 地址，
    /// 则将其转换为 [`IPv4` address]；否则返回 [`None`]。
///
    /// 注意，这会把 IPv6 loopback 地址 `::1` 转换为一个 [`IPv4` address]。
    /// 如果要避免这种情况，请使用 [`Ipv6Addr::to_ipv4_mapped`]。
///
    /// `::a.b.c.d` 和 `::ffff:a.b.c.d` 会变为 `a.b.c.d`。`::1` 会变为 `0.0.0.1`。
    /// 所有*不是*以全零前缀或 `::ffff` 开头的地址都会返回 `None`。
    ///
    /// [`IPv4` address]: Ipv4Addr
    /// [IPv4-compatible]: Ipv6Addr#ipv4-compatible-ipv6-addresses
    /// [IPv4-mapped]: Ipv6Addr#ipv4-mapped-ipv6-addresses
    /// [IETF RFC 4291 section 2.5.5.1]: https://tools.ietf.org/html/rfc4291#section-2.5.5.1
    /// [IETF RFC 4291 section 2.5.5.2]: https://tools.ietf.org/html/rfc4291#section-2.5.5.2
    ///
    /// # 示例
    ///
    /// ```
    /// use std::net::{Ipv4Addr, Ipv6Addr};
    ///
    /// assert_eq!(Ipv6Addr::new(0xff00, 0, 0, 0, 0, 0, 0, 0).to_ipv4(), None);
    /// assert_eq!(Ipv6Addr::new(0, 0, 0, 0, 0, 0xffff, 0xc00a, 0x2ff).to_ipv4(),
    ///            Some(Ipv4Addr::new(192, 10, 2, 255)));
    /// assert_eq!(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1).to_ipv4(),
    ///            Some(Ipv4Addr::new(0, 0, 0, 1)));
    /// ```
    #[rustc_const_stable(feature = "const_ip_50", since = "1.50.0")]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[must_use = "this returns the result of the operation, \
                  without modifying the original"]
    #[inline]
    pub const fn to_ipv4(&self) -> Option<Ipv4Addr> {
        if let [0, 0, 0, 0, 0, 0 | 0xffff, ab, cd] = self.segments() {
            let [a, b] = ab.to_be_bytes();
            let [c, d] = cd.to_be_bytes();
            Some(Ipv4Addr::new(a, b, c, d))
        } else {
            None
        }
    }

    /// 如果此地址是 IPv4-mapped 地址，则转换为 `IpAddr::V4`；否则将自身包装为 `IpAddr::V6` 返回。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::net::Ipv6Addr;
    ///
    /// assert_eq!(Ipv6Addr::new(0, 0, 0, 0, 0, 0xffff, 0x7f00, 0x1).is_loopback(), false);
    /// assert_eq!(Ipv6Addr::new(0, 0, 0, 0, 0, 0xffff, 0x7f00, 0x1).to_canonical().is_loopback(), true);
    /// ```
    #[inline]
    #[must_use = "this returns the result of the operation, \
                  without modifying the original"]
    #[stable(feature = "ip_to_canonical", since = "1.75.0")]
    #[rustc_const_stable(feature = "ip_to_canonical", since = "1.75.0")]
    pub const fn to_canonical(&self) -> IpAddr {
        if let Some(mapped) = self.to_ipv4_mapped() {
            return IpAddr::V4(mapped);
        }
        IpAddr::V6(*self)
    }

    /// 返回组成此 IPv6 地址的十六个 8 位整数。
    ///
    /// ```
    /// use std::net::Ipv6Addr;
    ///
    /// assert_eq!(Ipv6Addr::new(0xff00, 0, 0, 0, 0, 0, 0, 0).octets(),
    ///            [0xff, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
    /// ```
    #[rustc_const_stable(feature = "const_ip_32", since = "1.32.0")]
    #[stable(feature = "ipv6_to_octets", since = "1.12.0")]
    #[must_use]
    #[inline]
    pub const fn octets(&self) -> [u8; 16] {
        self.octets
    }

    /// 根据包含十六个元素的字节数组创建 `Ipv6Addr`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::net::Ipv6Addr;
    ///
    /// let addr = Ipv6Addr::from_octets([
    ///     0x19u8, 0x18u8, 0x17u8, 0x16u8, 0x15u8, 0x14u8, 0x13u8, 0x12u8,
    ///     0x11u8, 0x10u8, 0x0fu8, 0x0eu8, 0x0du8, 0x0cu8, 0x0bu8, 0x0au8,
    /// ]);
    /// assert_eq!(
    ///     Ipv6Addr::new(
    ///         0x1918, 0x1716, 0x1514, 0x1312,
    ///         0x1110, 0x0f0e, 0x0d0c, 0x0b0a,
    ///     ),
    ///     addr
    /// );
    /// ```
    #[stable(feature = "ip_from", since = "1.91.0")]
    #[rustc_const_stable(feature = "ip_from", since = "1.91.0")]
    #[must_use]
    #[inline]
    pub const fn from_octets(octets: [u8; 16]) -> Ipv6Addr {
        Ipv6Addr { octets }
    }

    /// 以切片形式返回组成此 IPv6 地址的十六个 8 位整数。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(ip_as_octets)]
    ///
    /// use std::net::Ipv6Addr;
    ///
    /// assert_eq!(Ipv6Addr::new(0xff00, 0, 0, 0, 0, 0, 0, 0).as_octets(),
    ///            &[255, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0])
    /// ```
    #[unstable(feature = "ip_as_octets", issue = "137259")]
    #[inline]
    pub const fn as_octets(&self) -> &[u8; 16] {
        &self.octets
    }
}

/// 按 [RFC 5952](https://tools.ietf.org/html/rfc5952) 描述的规范样式写出 `Ipv6Addr`。
#[stable(feature = "rust1", since = "1.0.0")]
impl fmt::Display for Ipv6Addr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // 如果没有对齐要求，则直接把 IP 地址写入 `f`。
        // 否则先写入本地缓冲区，再调用 `f.pad`。
        if f.precision().is_none() && f.width().is_none() {
            let segments = self.segments();

            if let Some(ipv4) = self.to_ipv4_mapped() {
                write!(f, "::ffff:{}", ipv4)
            } else {
                #[derive(Copy, Clone, Default)]
                struct Span {
                    start: usize,
                    len: usize,
                }

                // 找出内部最长的 0 segment 连续区间。
                let zeroes = {
                    let mut longest = Span::default();
                    let mut current = Span::default();

                    for (i, &segment) in segments.iter().enumerate() {
                        if segment == 0 {
                            if current.len == 0 {
                                current.start = i;
                            }

                            current.len += 1;

                            if current.len > longest.len {
                                longest = current;
                            }
                        } else {
                            current = Span::default();
                        }
                    }

                    longest
                };

                /// 写出地址中以冒号分隔的一段。
                #[inline]
                fn fmt_subslice(f: &mut fmt::Formatter<'_>, chunk: &[u16]) -> fmt::Result {
                    if let Some((first, tail)) = chunk.split_first() {
                        write!(f, "{:x}", first)?;
                        for segment in tail {
                            f.write_char(':')?;
                            write!(f, "{:x}", segment)?;
                        }
                    }
                    Ok(())
                }

                if zeroes.len > 1 {
                    fmt_subslice(f, &segments[..zeroes.start])?;
                    f.write_str("::")?;
                    fmt_subslice(f, &segments[zeroes.start + zeroes.len..])
                } else {
                    fmt_subslice(f, &segments)
                }
            }
        } else {
            const LONGEST_IPV6_ADDR: &str = "ffff:ffff:ffff:ffff:ffff:ffff:ffff:ffff";

            let mut buf = DisplayBuffer::<{ LONGEST_IPV6_ADDR.len() }>::new();
            // 缓冲区足以容纳最长的 IPv6 地址，因此这里不应失败。
            write!(buf, "{}", self).unwrap();

            f.pad(buf.as_str())
        }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl fmt::Debug for Ipv6Addr {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, fmt)
    }
}

#[stable(feature = "ip_cmp", since = "1.16.0")]
impl PartialEq<IpAddr> for Ipv6Addr {
    #[inline]
    fn eq(&self, other: &IpAddr) -> bool {
        match other {
            IpAddr::V4(_) => false,
            IpAddr::V6(v6) => self == v6,
        }
    }
}

#[stable(feature = "ip_cmp", since = "1.16.0")]
impl PartialEq<Ipv6Addr> for IpAddr {
    #[inline]
    fn eq(&self, other: &Ipv6Addr) -> bool {
        match self {
            IpAddr::V4(_) => false,
            IpAddr::V6(v6) => v6 == other,
        }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl PartialOrd for Ipv6Addr {
    #[inline]
    fn partial_cmp(&self, other: &Ipv6Addr) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[stable(feature = "ip_cmp", since = "1.16.0")]
impl PartialOrd<Ipv6Addr> for IpAddr {
    #[inline]
    fn partial_cmp(&self, other: &Ipv6Addr) -> Option<Ordering> {
        match self {
            IpAddr::V4(_) => Some(Ordering::Less),
            IpAddr::V6(v6) => v6.partial_cmp(other),
        }
    }
}

#[stable(feature = "ip_cmp", since = "1.16.0")]
impl PartialOrd<IpAddr> for Ipv6Addr {
    #[inline]
    fn partial_cmp(&self, other: &IpAddr) -> Option<Ordering> {
        match other {
            IpAddr::V4(_) => Some(Ordering::Greater),
            IpAddr::V6(v6) => self.partial_cmp(v6),
        }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl Ord for Ipv6Addr {
    #[inline]
    fn cmp(&self, other: &Ipv6Addr) -> Ordering {
        self.segments().cmp(&other.segments())
    }
}

#[stable(feature = "i128", since = "1.26.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl const From<Ipv6Addr> for u128 {
    /// 使用 [`Ipv6Addr::to_bits`] 将 IPv6 地址转换为本机字节序的 `u128`。
    #[inline]
    fn from(ip: Ipv6Addr) -> u128 {
        ip.to_bits()
    }
}
#[stable(feature = "i128", since = "1.26.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl const From<u128> for Ipv6Addr {
    /// 使用 [`Ipv6Addr::from_bits`] 将本机字节序的 `u128` 转换为 IPv6 地址。
    #[inline]
    fn from(ip: u128) -> Ipv6Addr {
        Ipv6Addr::from_bits(ip)
    }
}

#[stable(feature = "ipv6_from_octets", since = "1.9.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl const From<[u8; 16]> for Ipv6Addr {
    /// 根据包含十六个元素的字节数组创建 `Ipv6Addr`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::net::Ipv6Addr;
    ///
    /// let addr = Ipv6Addr::from([
    ///     0x19u8, 0x18u8, 0x17u8, 0x16u8, 0x15u8, 0x14u8, 0x13u8, 0x12u8,
    ///     0x11u8, 0x10u8, 0x0fu8, 0x0eu8, 0x0du8, 0x0cu8, 0x0bu8, 0x0au8,
    /// ]);
    /// assert_eq!(
    ///     Ipv6Addr::new(
    ///         0x1918, 0x1716, 0x1514, 0x1312,
    ///         0x1110, 0x0f0e, 0x0d0c, 0x0b0a,
    ///     ),
    ///     addr
    /// );
    /// ```
    #[inline]
    fn from(octets: [u8; 16]) -> Ipv6Addr {
        Ipv6Addr { octets }
    }
}

#[stable(feature = "ipv6_from_segments", since = "1.16.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl const From<[u16; 8]> for Ipv6Addr {
    /// 根据包含八个元素的 16 位数组创建 `Ipv6Addr`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::net::Ipv6Addr;
    ///
    /// let addr = Ipv6Addr::from([
    ///     0x20du16, 0x20cu16, 0x20bu16, 0x20au16,
    ///     0x209u16, 0x208u16, 0x207u16, 0x206u16,
    /// ]);
    /// assert_eq!(
    ///     Ipv6Addr::new(
    ///         0x20d, 0x20c, 0x20b, 0x20a,
    ///         0x209, 0x208, 0x207, 0x206,
    ///     ),
    ///     addr
    /// );
    /// ```
    #[inline]
    fn from(segments: [u16; 8]) -> Ipv6Addr {
        let [a, b, c, d, e, f, g, h] = segments;
        Ipv6Addr::new(a, b, c, d, e, f, g, h)
    }
}

#[stable(feature = "ip_from_slice", since = "1.17.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl const From<[u8; 16]> for IpAddr {
    /// 根据包含十六个元素的字节数组创建 `IpAddr::V6`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::net::{IpAddr, Ipv6Addr};
    ///
    /// let addr = IpAddr::from([
    ///     0x19u8, 0x18u8, 0x17u8, 0x16u8, 0x15u8, 0x14u8, 0x13u8, 0x12u8,
    ///     0x11u8, 0x10u8, 0x0fu8, 0x0eu8, 0x0du8, 0x0cu8, 0x0bu8, 0x0au8,
    /// ]);
    /// assert_eq!(
    ///     IpAddr::V6(Ipv6Addr::new(
    ///         0x1918, 0x1716, 0x1514, 0x1312,
    ///         0x1110, 0x0f0e, 0x0d0c, 0x0b0a,
    ///     )),
    ///     addr
    /// );
    /// ```
    #[inline]
    fn from(octets: [u8; 16]) -> IpAddr {
        IpAddr::V6(Ipv6Addr::from(octets))
    }
}

#[stable(feature = "ip_from_slice", since = "1.17.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl const From<[u16; 8]> for IpAddr {
    /// 根据包含八个元素的 16 位数组创建 `IpAddr::V6`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::net::{IpAddr, Ipv6Addr};
    ///
    /// let addr = IpAddr::from([
    ///     0x20du16, 0x20cu16, 0x20bu16, 0x20au16,
    ///     0x209u16, 0x208u16, 0x207u16, 0x206u16,
    /// ]);
    /// assert_eq!(
    ///     IpAddr::V6(Ipv6Addr::new(
    ///         0x20d, 0x20c, 0x20b, 0x20a,
    ///         0x209, 0x208, 0x207, 0x206,
    ///     )),
    ///     addr
    /// );
    /// ```
    #[inline]
    fn from(segments: [u16; 8]) -> IpAddr {
        IpAddr::V6(Ipv6Addr::from(segments))
    }
}

#[stable(feature = "ip_bitops", since = "1.75.0")]
#[rustc_const_unstable(feature = "const_ops", issue = "143802")]
impl const Not for Ipv4Addr {
    type Output = Ipv4Addr;

    #[inline]
    fn not(mut self) -> Ipv4Addr {
        let mut idx = 0;
        while idx < 4 {
            self.octets[idx] = !self.octets[idx];
            idx += 1;
        }
        self
    }
}

#[stable(feature = "ip_bitops", since = "1.75.0")]
#[rustc_const_unstable(feature = "const_ops", issue = "143802")]
impl const Not for &'_ Ipv4Addr {
    type Output = Ipv4Addr;

    #[inline]
    fn not(self) -> Ipv4Addr {
        !*self
    }
}

#[stable(feature = "ip_bitops", since = "1.75.0")]
#[rustc_const_unstable(feature = "const_ops", issue = "143802")]
impl const Not for Ipv6Addr {
    type Output = Ipv6Addr;

    #[inline]
    fn not(mut self) -> Ipv6Addr {
        let mut idx = 0;
        while idx < 16 {
            self.octets[idx] = !self.octets[idx];
            idx += 1;
        }
        self
    }
}

#[stable(feature = "ip_bitops", since = "1.75.0")]
#[rustc_const_unstable(feature = "const_ops", issue = "143802")]
impl const Not for &'_ Ipv6Addr {
    type Output = Ipv6Addr;

    #[inline]
    fn not(self) -> Ipv6Addr {
        !*self
    }
}

macro_rules! bitop_impls {
    ($(
        $(#[$attr:meta])*
        impl ($BitOp:ident, $BitOpAssign:ident) for $ty:ty = ($bitop:ident, $bitop_assign:ident);
    )*) => {
        $(
            $(#[$attr])*
            impl const $BitOpAssign for $ty {
                fn $bitop_assign(&mut self, rhs: $ty) {
                    let mut idx = 0;
                    while idx < self.octets.len() {
                        self.octets[idx].$bitop_assign(rhs.octets[idx]);
                        idx += 1;
                    }
                }
            }

            $(#[$attr])*
            impl const $BitOpAssign<&'_ $ty> for $ty {
                fn $bitop_assign(&mut self, rhs: &'_ $ty) {
                    self.$bitop_assign(*rhs);
                }
            }

            $(#[$attr])*
            impl const $BitOp for $ty {
                type Output = $ty;

                #[inline]
                fn $bitop(mut self, rhs: $ty) -> $ty {
                    self.$bitop_assign(rhs);
                    self
                }
            }

            $(#[$attr])*
            impl const $BitOp<&'_ $ty> for $ty {
                type Output = $ty;

                #[inline]
                fn $bitop(mut self, rhs: &'_ $ty) -> $ty {
                    self.$bitop_assign(*rhs);
                    self
                }
            }

            $(#[$attr])*
            impl const $BitOp<$ty> for &'_ $ty {
                type Output = $ty;

                #[inline]
                fn $bitop(self, rhs: $ty) -> $ty {
                    let mut lhs = *self;
                    lhs.$bitop_assign(rhs);
                    lhs
                }
            }

            $(#[$attr])*
            impl const $BitOp<&'_ $ty> for &'_ $ty {
                type Output = $ty;

                #[inline]
                fn $bitop(self, rhs: &'_ $ty) -> $ty {
                    let mut lhs = *self;
                    lhs.$bitop_assign(*rhs);
                    lhs
                }
            }
        )*
    };
}

bitop_impls! {
    #[stable(feature = "ip_bitops", since = "1.75.0")]
    #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
    impl (BitAnd, BitAndAssign) for Ipv4Addr = (bitand, bitand_assign);
    #[stable(feature = "ip_bitops", since = "1.75.0")]
    #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
    impl (BitOr, BitOrAssign) for Ipv4Addr = (bitor, bitor_assign);

    #[stable(feature = "ip_bitops", since = "1.75.0")]
    #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
    impl (BitAnd, BitAndAssign) for Ipv6Addr = (bitand, bitand_assign);
    #[stable(feature = "ip_bitops", since = "1.75.0")]
    #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
    impl (BitOr, BitOrAssign) for Ipv6Addr = (bitor, bitor_assign);
}
