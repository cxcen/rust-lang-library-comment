//! IPv4、IPv6 和 socket 地址的私有解析器实现。
//!
//! 本模块通过下面的 `FromStr` 实现“公开导出”其解析能力。

use crate::error::Error;
use crate::fmt;
use crate::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6};
use crate::str::FromStr;

trait ReadNumberHelper: Sized {
    const ZERO: Self;
    fn checked_mul(&self, other: u32) -> Option<Self>;
    fn checked_add(&self, other: u32) -> Option<Self>;
}

macro_rules! impl_helper {
    ($($t:ty)*) => ($(impl ReadNumberHelper for $t {
        const ZERO: Self = 0;
        #[inline]
        fn checked_mul(&self, other: u32) -> Option<Self> {
            Self::checked_mul(*self, other.try_into().ok()?)
        }
        #[inline]
        fn checked_add(&self, other: u32) -> Option<Self> {
            Self::checked_add(*self, other.try_into().ok()?)
        }
    })*)
}

impl_helper! { u8 u16 u32 }

struct Parser<'a> {
    // 输入按 ASCII 解析，因此可以直接使用字节切片。
    state: &'a [u8],
}

impl<'a> Parser<'a> {
    fn new(input: &'a [u8]) -> Parser<'a> {
        Parser { state: input }
    }

    /// 运行一个解析器；如果解析失败，则恢复到解析前的状态。
    fn read_atomically<T, F>(&mut self, inner: F) -> Option<T>
    where
        F: FnOnce(&mut Parser<'_>) -> Option<T>,
    {
        let state = self.state;
        let result = inner(self);
        if result.is_none() {
            self.state = state;
        }
        result
    }

    /// 运行一个解析器，但如果没有消费完整输入则视为失败。
    /// 该过程不是原子的；失败时不会自动回滚内部状态。
    fn parse_with<T, F>(&mut self, inner: F, kind: AddrKind) -> Result<T, AddrParseError>
    where
        F: FnOnce(&mut Parser<'_>) -> Option<T>,
    {
        let result = inner(self);
        if self.state.is_empty() { result } else { None }.ok_or(AddrParseError(kind))
    }

    /// 查看输入中的下一个字符，但不消费它。
    fn peek_char(&self) -> Option<char> {
        self.state.first().map(|&b| char::from(b))
    }

    /// 从输入中读取并消费下一个字符。
    fn read_char(&mut self) -> Option<char> {
        self.state.split_first().map(|(&b, tail)| {
            self.state = tail;
            char::from(b)
        })
    }

    #[must_use]
    /// 如果输入中的下一个字符与目标字符匹配，则读取并消费它。
    fn read_given_char(&mut self, target: char) -> Option<()> {
        self.read_atomically(|p| {
            p.read_char().and_then(|c| if c == target { Some(()) } else { None })
        })
    }

    /// 在带索引循环中读取分隔符的辅助函数。仅当 `index > 0` 时才读取分隔符字符，
    /// 随后运行传入的解析器。循环使用时，分隔符只会在 `index > 0` 的轮次被读取；
    /// `read_ipv4_addr` 展示了这种用法。
    fn read_separator<T, F>(&mut self, sep: char, index: usize, inner: F) -> Option<T>
    where
        F: FnOnce(&mut Parser<'_>) -> Option<T>,
    {
        self.read_atomically(move |p| {
            if index > 0 {
                p.read_given_char(sep)?;
            }
            inner(p)
        })
    }

    // 按给定基数从输入开头读取一个数字，遇到第一个非数字字符或输入结束时停止。
    // 如果数字位数超过 `max_digits`，或者根本没有读到数字，则解析失败。
    //
    // INVARIANT: `max_digits` 必须小于 `u32` 可表示范围的最大十进制位数。
    fn read_number<T: ReadNumberHelper + TryFrom<u32>>(
        &mut self,
        radix: u32,
        max_digits: Option<usize>,
        allow_zero_prefix: bool,
    ) -> Option<T> {
        self.read_atomically(move |p| {
            let mut digit_count = 0;
            let has_leading_zero = p.peek_char() == Some('0');

            // 如果 `max_digits.is_some()`，当前解析的是 `u8` 或 `u16`；
            // 它们一定能放进 `u32`，因此无需使用带检查的算术运算。
            let result = if let Some(max_digits) = max_digits {
                // u32::MAX = 4_294_967_295u32，共 10 位十进制数字。
                // `max_digits` 必须小于 10，才能确保不会溢出 `u32`。
                debug_assert!(max_digits < 10);

                let mut result = 0_u32;
                while let Some(digit) = p.read_atomically(|p| p.read_char()?.to_digit(radix)) {
                    result *= radix;
                    result += digit;
                    digit_count += 1;

                    if digit_count > max_digits {
                        return None;
                    }
                }

                result.try_into().ok()
            } else {
                let mut result = T::ZERO;

                while let Some(digit) = p.read_atomically(|p| p.read_char()?.to_digit(radix)) {
                    result = result.checked_mul(radix)?;
                    result = result.checked_add(digit)?;
                    digit_count += 1;
                }

                Some(result)
            };

            if digit_count == 0 {
                None
            } else if !allow_zero_prefix && has_leading_zero && digit_count > 1 {
                None
            } else {
                result
            }
        })
    }

    /// 读取一个 IPv4 地址。
    fn read_ipv4_addr(&mut self) -> Option<Ipv4Addr> {
        self.read_atomically(|p| {
            let mut groups = [0; 4];

            for (i, slot) in groups.iter_mut().enumerate() {
                *slot = p.read_separator('.', i, |p| {
                    // IP 字符串中不允许使用八进制数字。
                    // https://tools.ietf.org/html/rfc6943#section-3.1.1
                    p.read_number(10, Some(3), false)
                })?;
            }

            Some(groups.into())
        })
    }

    /// 读取一个 IPv6 地址。
    fn read_ipv6_addr(&mut self) -> Option<Ipv6Addr> {
        /// 将 IPv6 地址的一段读入 `groups`。返回已读取的组数，以及一个 bool，
        /// 表示是否读到了末尾嵌入的 IPv4 地址。具体来说，它会读取一串以冒号分隔的
        /// IPv6 组（0x0000 - 0xFFFF），末尾可以附带一个嵌入的 IPv4 地址。
        fn read_groups(p: &mut Parser<'_>, groups: &mut [u16]) -> (usize, bool) {
            let limit = groups.len();

            for (i, slot) in groups.iter_mut().enumerate() {
                // 尝试读取末尾嵌入的 IPv4 地址；此时必须至少还剩两个 IPv6 组位置。
                if i < limit - 1 {
                    let ipv4 = p.read_separator(':', i, |p| p.read_ipv4_addr());

                    if let Some(v4_addr) = ipv4 {
                        let [one, two, three, four] = v4_addr.octets();
                        groups[i + 0] = u16::from_be_bytes([one, two]);
                        groups[i + 1] = u16::from_be_bytes([three, four]);
                        return (i + 2, true);
                    }
                }

                let group = p.read_separator(':', i, |p| p.read_number(16, Some(4), true));

                match group {
                    Some(g) => *slot = g,
                    None => return (i, false),
                }
            }
            (groups.len(), false)
        }

        self.read_atomically(|p| {
            // 读取地址的前半部分；可能是完整地址，也可能只读到第一个 `::` 之前。
            let mut head = [0; 8];
            let (head_size, head_ipv4) = read_groups(p, &mut head);

            if head_size == 8 {
                return Some(head.into());
            }

            // 嵌入的 IPv4 部分不允许出现在 `::` 之前。
            if head_ipv4 {
                return None;
            }

            // 如果前面的代码解析出的组数少于 8 组，则读取 `::`。
            // `::` 表示一个或多个 16 位全零组。
            p.read_given_char(':')?;
            p.read_given_char(':')?;

            // 读取地址的后半部分。`::` 必须至少代表一组全零值，所以后半部分最多 7 组。
            let mut tail = [0; 7];
            let limit = 8 - (head_size + 1);
            let (tail_size, _) = read_groups(p, &mut tail[..limit]);

            // 拼接 IP 地址的前半部分和后半部分。
            head[(8 - tail_size)..8].copy_from_slice(&tail[..tail_size]);

            Some(head.into())
        })
    }

    /// 读取一个 IP 地址，可以是 IPv4 或 IPv6。
    fn read_ip_addr(&mut self) -> Option<IpAddr> {
        self.read_ipv4_addr().map(IpAddr::V4).or_else(move || self.read_ipv6_addr().map(IpAddr::V6))
    }

    /// 读取一个 `:`，随后读取十进制端口号。
    fn read_port(&mut self) -> Option<u16> {
        self.read_atomically(|p| {
            p.read_given_char(':')?;
            p.read_number(10, None, true)
        })
    }

    /// 读取一个 `%`，随后读取十进制 scope ID。
    fn read_scope_id(&mut self) -> Option<u32> {
        self.read_atomically(|p| {
            p.read_given_char('%')?;
            p.read_number(10, None, true)
        })
    }

    /// 读取带端口号的 IPv4 地址。
    fn read_socket_addr_v4(&mut self) -> Option<SocketAddrV4> {
        self.read_atomically(|p| {
            let ip = p.read_ipv4_addr()?;
            let port = p.read_port()?;
            Some(SocketAddrV4::new(ip, port))
        })
    }

    /// 读取带端口号的 IPv6 地址。
    fn read_socket_addr_v6(&mut self) -> Option<SocketAddrV6> {
        self.read_atomically(|p| {
            p.read_given_char('[')?;
            let ip = p.read_ipv6_addr()?;
            let scope_id = p.read_scope_id().unwrap_or(0);
            p.read_given_char(']')?;

            let port = p.read_port()?;
            Some(SocketAddrV6::new(ip, port, 0, scope_id))
        })
    }

    /// 读取带端口号的 IP 地址。
    fn read_socket_addr(&mut self) -> Option<SocketAddr> {
        self.read_socket_addr_v4()
            .map(SocketAddr::V4)
            .or_else(|| self.read_socket_addr_v6().map(SocketAddr::V6))
    }
}

impl IpAddr {
    /// 从字节切片解析 IP 地址。
    ///
    /// ```
    /// #![feature(addr_parse_ascii)]
    ///
    /// use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
    ///
    /// let localhost_v4 = IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1));
    /// let localhost_v6 = IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1));
    ///
    /// assert_eq!(IpAddr::parse_ascii(b"127.0.0.1"), Ok(localhost_v4));
    /// assert_eq!(IpAddr::parse_ascii(b"::1"), Ok(localhost_v6));
    /// ```
    #[unstable(feature = "addr_parse_ascii", issue = "101035")]
    pub fn parse_ascii(b: &[u8]) -> Result<Self, AddrParseError> {
        Parser::new(b).parse_with(|p| p.read_ip_addr(), AddrKind::Ip)
    }
}

#[stable(feature = "ip_addr", since = "1.7.0")]
impl FromStr for IpAddr {
    type Err = AddrParseError;
    fn from_str(s: &str) -> Result<IpAddr, AddrParseError> {
        Self::parse_ascii(s.as_bytes())
    }
}

impl Ipv4Addr {
    /// 从字节切片解析 IPv4 地址。
    ///
    /// ```
    /// #![feature(addr_parse_ascii)]
    ///
    /// use std::net::Ipv4Addr;
    ///
    /// let localhost = Ipv4Addr::new(127, 0, 0, 1);
    ///
    /// assert_eq!(Ipv4Addr::parse_ascii(b"127.0.0.1"), Ok(localhost));
    /// ```
    #[unstable(feature = "addr_parse_ascii", issue = "101035")]
    pub fn parse_ascii(b: &[u8]) -> Result<Self, AddrParseError> {
        // 过长的输入不再尝试解析。
        if b.len() > 15 {
            Err(AddrParseError(AddrKind::Ipv4))
        } else {
            Parser::new(b).parse_with(|p| p.read_ipv4_addr(), AddrKind::Ipv4)
        }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl FromStr for Ipv4Addr {
    type Err = AddrParseError;
    fn from_str(s: &str) -> Result<Ipv4Addr, AddrParseError> {
        Self::parse_ascii(s.as_bytes())
    }
}

impl Ipv6Addr {
    /// 从字节切片解析 IPv6 地址。
    ///
    /// ```
    /// #![feature(addr_parse_ascii)]
    ///
    /// use std::net::Ipv6Addr;
    ///
    /// let localhost = Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1);
    ///
    /// assert_eq!(Ipv6Addr::parse_ascii(b"::1"), Ok(localhost));
    /// ```
    #[unstable(feature = "addr_parse_ascii", issue = "101035")]
    pub fn parse_ascii(b: &[u8]) -> Result<Self, AddrParseError> {
        Parser::new(b).parse_with(|p| p.read_ipv6_addr(), AddrKind::Ipv6)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl FromStr for Ipv6Addr {
    type Err = AddrParseError;
    fn from_str(s: &str) -> Result<Ipv6Addr, AddrParseError> {
        Self::parse_ascii(s.as_bytes())
    }
}

impl SocketAddrV4 {
    /// 从字节切片解析 IPv4 socket 地址。
    ///
    /// ```
    /// #![feature(addr_parse_ascii)]
    ///
    /// use std::net::{Ipv4Addr, SocketAddrV4};
    ///
    /// let socket = SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 8080);
    ///
    /// assert_eq!(SocketAddrV4::parse_ascii(b"127.0.0.1:8080"), Ok(socket));
    /// ```
    #[unstable(feature = "addr_parse_ascii", issue = "101035")]
    pub fn parse_ascii(b: &[u8]) -> Result<Self, AddrParseError> {
        Parser::new(b).parse_with(|p| p.read_socket_addr_v4(), AddrKind::SocketV4)
    }
}

#[stable(feature = "socket_addr_from_str", since = "1.5.0")]
impl FromStr for SocketAddrV4 {
    type Err = AddrParseError;
    fn from_str(s: &str) -> Result<SocketAddrV4, AddrParseError> {
        Self::parse_ascii(s.as_bytes())
    }
}

impl SocketAddrV6 {
    /// 从字节切片解析 IPv6 socket 地址。
    ///
    /// ```
    /// #![feature(addr_parse_ascii)]
    ///
    /// use std::net::{Ipv6Addr, SocketAddrV6};
    ///
    /// let socket = SocketAddrV6::new(Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1), 8080, 0, 0);
    ///
    /// assert_eq!(SocketAddrV6::parse_ascii(b"[2001:db8::1]:8080"), Ok(socket));
    /// ```
    #[unstable(feature = "addr_parse_ascii", issue = "101035")]
    pub fn parse_ascii(b: &[u8]) -> Result<Self, AddrParseError> {
        Parser::new(b).parse_with(|p| p.read_socket_addr_v6(), AddrKind::SocketV6)
    }
}

#[stable(feature = "socket_addr_from_str", since = "1.5.0")]
impl FromStr for SocketAddrV6 {
    type Err = AddrParseError;
    fn from_str(s: &str) -> Result<SocketAddrV6, AddrParseError> {
        Self::parse_ascii(s.as_bytes())
    }
}

impl SocketAddr {
    /// 从字节切片解析 socket 地址。
    ///
    /// ```
    /// #![feature(addr_parse_ascii)]
    ///
    /// use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
    ///
    /// let socket_v4 = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 8080);
    /// let socket_v6 = SocketAddr::new(IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1)), 8080);
    ///
    /// assert_eq!(SocketAddr::parse_ascii(b"127.0.0.1:8080"), Ok(socket_v4));
    /// assert_eq!(SocketAddr::parse_ascii(b"[::1]:8080"), Ok(socket_v6));
    /// ```
    #[unstable(feature = "addr_parse_ascii", issue = "101035")]
    pub fn parse_ascii(b: &[u8]) -> Result<Self, AddrParseError> {
        Parser::new(b).parse_with(|p| p.read_socket_addr(), AddrKind::Socket)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl FromStr for SocketAddr {
    type Err = AddrParseError;
    fn from_str(s: &str) -> Result<SocketAddr, AddrParseError> {
        Self::parse_ascii(s.as_bytes())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum AddrKind {
    Ip,
    Ipv4,
    Ipv6,
    Socket,
    SocketV4,
    SocketV6,
}

/// 解析 IP 地址或 socket 地址时可能返回的错误。
///
/// 该错误用作 [`IpAddr`]、[`Ipv4Addr`]、[`Ipv6Addr`]、[`SocketAddr`]、
/// [`SocketAddrV4`] 和 [`SocketAddrV6`] 的 [`FromStr`] 实现的错误类型。
///
/// # 可能原因
///
/// 如果提供的字符串无法按目标类型解析，就可能产生 `AddrParseError`。常见情况是：
/// 字符串中包含了另一个地址类型才会处理的信息。
///
/// ```should_panic
/// use std::net::IpAddr;
/// let _foo: IpAddr = "127.0.0.1:8080".parse().expect("Cannot handle the socket port");
/// ```
///
/// [`IpAddr`] 不处理端口号；需要端口语义时应使用 [`SocketAddr`]。
///
/// ```
/// use std::net::SocketAddr;
///
/// // 没问题,`panic!` 消息已经消失了。
/// let _foo: SocketAddr = "127.0.0.1:8080".parse().expect("unreachable panic");
/// ```
#[stable(feature = "rust1", since = "1.0.0")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddrParseError(AddrKind);

#[stable(feature = "addr_parse_error_error", since = "1.4.0")]
impl fmt::Display for AddrParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            AddrKind::Ip => "invalid IP address syntax",
            AddrKind::Ipv4 => "invalid IPv4 address syntax",
            AddrKind::Ipv6 => "invalid IPv6 address syntax",
            AddrKind::Socket => "invalid socket address syntax",
            AddrKind::SocketV4 => "invalid IPv4 socket address syntax",
            AddrKind::SocketV6 => "invalid IPv6 socket address syntax",
        }
        .fmt(f)
    }
}

#[stable(feature = "addr_parse_error_error", since = "1.4.0")]
impl Error for AddrParseError {}
