// 本模块的测试
#[cfg(all(test, not(any(target_os = "emscripten", all(target_os = "wasi", target_env = "p1")))))]
mod tests;

#[stable(feature = "rust1", since = "1.0.0")]
pub use core::net::{SocketAddr, SocketAddrV4, SocketAddrV6};

use crate::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use crate::{io, iter, option, slice, vec};

/// 用于表示“可被转换或解析为一个或多个 [`SocketAddr`] 值”的对象的 trait。
///
/// 此 trait 用于在构造网络对象时进行泛型化的地址解析。默认情况下，它为以下
/// 类型实现：
///
///  * [`SocketAddr`]：[`to_socket_addrs`] 即恒等函数（identity function）。
///
///  * [`SocketAddrV4`]、[`SocketAddrV6`]、<code>([IpAddr], [u16])</code>、
///    <code>([Ipv4Addr], [u16])</code>、<code>([Ipv6Addr], [u16])</code>：
///    [`to_socket_addrs`] 会平凡地（trivially）构造出一个 [`SocketAddr`]。
///
///  * <code>(&[str], [u16])</code>：<code>&[str]</code> 应当是
///    [`IpAddr`] 地址（其形式由 [`FromStr`] 实现所期望）的字符串表示，或者是一个主机
///    名。[`u16`] 是端口号。
///
///  * <code>&[str]</code>：该字符串应当是一个 [`SocketAddr`] 的字符串表示（其形式由
///    它的 [`FromStr`] 实现所期望），或者是一个形如 `<host_name>:<port>` 的配对，其中
///    `<port>` 是一个 [`u16`] 值。
///
///  * <code>&[[SocketAddr]]</code>：切片中所有的 [`SocketAddr`] 值都会被使用。
///
/// 此 trait 让你能够以各种类型的值作为绑定/连接地址，轻松构造像 [`TcpStream`] 或
/// [`UdpSocket`] 这样的网络对象。之所以需要它，是因为有时某一种类型比另一种更
/// 合适：对于简单用途，像 `"localhost:12345"` 这样的字符串远比手工构造对应的
/// [`SocketAddr`] 来得舒服；但有时 [`SocketAddr`] 值才是地址*主要*的来源，把它转换
/// 成其他类型（例如字符串）、再在构造方法里转换回 [`SocketAddr`]，就毫无意义了。
///
/// 操作系统返回的、非 IP 地址的地址会被静默忽略。
///
/// [`FromStr`]: crate::str::FromStr "std::str::FromStr"
/// [`TcpStream`]: crate::net::TcpStream "net::TcpStream"
/// [`to_socket_addrs`]: ToSocketAddrs::to_socket_addrs
/// [`UdpSocket`]: crate::net::UdpSocket "net::UdpSocket"
///
/// # 示例(Examples）
///
/// 创建一个只产出一个元素的 [`SocketAddr`] 迭代器：
///
/// ```
/// use std::net::{ToSocketAddrs, SocketAddr};
///
/// let addr = SocketAddr::from(([127, 0, 0, 1], 443));
/// let mut addrs_iter = addr.to_socket_addrs().unwrap();
///
/// assert_eq!(Some(addr), addrs_iter.next());
/// assert!(addrs_iter.next().is_none());
/// ```
///
/// 从一个主机名创建 [`SocketAddr`] 迭代器：
///
/// ```no_run
/// use std::net::{SocketAddr, ToSocketAddrs};
///
/// // 假设 'localhost' 解析为 127.0.0.1
/// let mut addrs_iter = "localhost:443".to_socket_addrs().unwrap();
/// assert_eq!(addrs_iter.next(), Some(SocketAddr::from(([127, 0, 0, 1], 443))));
/// assert!(addrs_iter.next().is_none());
///
/// // 假设 'foo' 无法解析
/// assert!("foo:443".to_socket_addrs().is_err());
/// ```
///
/// 创建一个产出多个元素的 [`SocketAddr`] 迭代器：
///
/// ```
/// use std::net::{SocketAddr, ToSocketAddrs};
///
/// let addr1 = SocketAddr::from(([0, 0, 0, 0], 80));
/// let addr2 = SocketAddr::from(([127, 0, 0, 1], 443));
/// let addrs = vec![addr1, addr2];
///
/// let mut addrs_iter = (&addrs[..]).to_socket_addrs().unwrap();
///
/// assert_eq!(Some(addr1), addrs_iter.next());
/// assert_eq!(Some(addr2), addrs_iter.next());
/// assert!(addrs_iter.next().is_none());
/// ```
///
/// 尝试从一个格式不正确的套接字地址 `&str`（缺少端口）创建 [`SocketAddr`]
/// 迭代器：
///
/// ```
/// use std::io;
/// use std::net::ToSocketAddrs;
///
/// let err = "127.0.0.1".to_socket_addrs().unwrap_err();
/// assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
/// ```
///
/// [`TcpStream::connect`] 是一个把 `ToSocketAddrs` 用作其参数 trait 约束的函数
/// 示例，目的是接受不同的类型：
///
/// ```no_run
/// use std::net::{TcpStream, Ipv4Addr};
///
/// let stream = TcpStream::connect(("127.0.0.1", 443));
/// // 或者
/// let stream = TcpStream::connect("127.0.0.1:443");
/// // 或者
/// let stream = TcpStream::connect((Ipv4Addr::new(127, 0, 0, 1), 443));
/// ```
///
/// [`TcpStream::connect`]: crate::net::TcpStream::connect
#[stable(feature = "rust1", since = "1.0.0")]
pub trait ToSocketAddrs {
    /// 此类型可能对应到的套接字地址的返回迭代器。
    #[stable(feature = "rust1", since = "1.0.0")]
    type Iter: Iterator<Item = SocketAddr>;

    /// 将此对象转换为一个由解析得到的 [`SocketAddr`] 组成的迭代器。
    ///
    /// 取决于所执行的解析结果，返回的迭代器实际上可能不产出任何值。
    ///
    /// 注意，在执行解析期间，此函数可能会阻塞当前线程（例如触发 DNS 查询时）。
    #[stable(feature = "rust1", since = "1.0.0")]
    fn to_socket_addrs(&self) -> io::Result<Self::Iter>;
}

#[stable(feature = "rust1", since = "1.0.0")]
impl ToSocketAddrs for SocketAddr {
    type Iter = option::IntoIter<SocketAddr>;
    fn to_socket_addrs(&self) -> io::Result<option::IntoIter<SocketAddr>> {
        Ok(Some(*self).into_iter())
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl ToSocketAddrs for SocketAddrV4 {
    type Iter = option::IntoIter<SocketAddr>;
    fn to_socket_addrs(&self) -> io::Result<option::IntoIter<SocketAddr>> {
        SocketAddr::V4(*self).to_socket_addrs()
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl ToSocketAddrs for SocketAddrV6 {
    type Iter = option::IntoIter<SocketAddr>;
    fn to_socket_addrs(&self) -> io::Result<option::IntoIter<SocketAddr>> {
        SocketAddr::V6(*self).to_socket_addrs()
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl ToSocketAddrs for (IpAddr, u16) {
    type Iter = option::IntoIter<SocketAddr>;
    fn to_socket_addrs(&self) -> io::Result<option::IntoIter<SocketAddr>> {
        let (ip, port) = *self;
        match ip {
            IpAddr::V4(ref a) => (*a, port).to_socket_addrs(),
            IpAddr::V6(ref a) => (*a, port).to_socket_addrs(),
        }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl ToSocketAddrs for (Ipv4Addr, u16) {
    type Iter = option::IntoIter<SocketAddr>;
    fn to_socket_addrs(&self) -> io::Result<option::IntoIter<SocketAddr>> {
        let (ip, port) = *self;
        SocketAddrV4::new(ip, port).to_socket_addrs()
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl ToSocketAddrs for (Ipv6Addr, u16) {
    type Iter = option::IntoIter<SocketAddr>;
    fn to_socket_addrs(&self) -> io::Result<option::IntoIter<SocketAddr>> {
        let (ip, port) = *self;
        SocketAddrV6::new(ip, port, 0, 0).to_socket_addrs()
    }
}

fn lookup_host(host: &str, port: u16) -> io::Result<vec::IntoIter<SocketAddr>> {
    let addrs = crate::sys::net::lookup_host(host, port)?;
    Ok(Vec::from_iter(addrs).into_iter())
}

#[stable(feature = "rust1", since = "1.0.0")]
impl ToSocketAddrs for (&str, u16) {
    type Iter = vec::IntoIter<SocketAddr>;
    fn to_socket_addrs(&self) -> io::Result<vec::IntoIter<SocketAddr>> {
        let (host, port) = *self;

        // 先尝试把 host 解析为常规的 IP 地址
        if let Ok(addr) = host.parse::<IpAddr>() {
            let addr = SocketAddr::new(addr, port);
            return Ok(vec![addr].into_iter());
        }

        // 否则，交由系统进行查询（lookup）。
        lookup_host(host, port)
    }
}

#[stable(feature = "string_u16_to_socket_addrs", since = "1.46.0")]
impl ToSocketAddrs for (String, u16) {
    type Iter = vec::IntoIter<SocketAddr>;
    fn to_socket_addrs(&self) -> io::Result<vec::IntoIter<SocketAddr>> {
        (&*self.0, self.1).to_socket_addrs()
    }
}

// 接受形如 'localhost:12345' 的字符串
#[stable(feature = "rust1", since = "1.0.0")]
impl ToSocketAddrs for str {
    type Iter = vec::IntoIter<SocketAddr>;
    fn to_socket_addrs(&self) -> io::Result<vec::IntoIter<SocketAddr>> {
        // 先尝试把整个字符串解析为常规的 SocketAddr
        if let Ok(addr) = self.parse() {
            return Ok(vec![addr].into_iter());
        }

        // 否则，按 ':' 拆分字符串，并把后半部分转换为 u16……
        let Some((host, port_str)) = self.rsplit_once(':') else {
            return Err(io::const_error!(io::ErrorKind::InvalidInput, "invalid socket address"));
        };
        let Ok(port) = port_str.parse::<u16>() else {
            return Err(io::const_error!(io::ErrorKind::InvalidInput, "invalid port value"));
        };

        // ……然后让系统去查询该 host。
        lookup_host(host, port)
    }
}

#[stable(feature = "slice_to_socket_addrs", since = "1.8.0")]
impl<'a> ToSocketAddrs for &'a [SocketAddr] {
    type Iter = iter::Cloned<slice::Iter<'a, SocketAddr>>;

    fn to_socket_addrs(&self) -> io::Result<Self::Iter> {
        Ok(self.iter().cloned())
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T: ToSocketAddrs + ?Sized> ToSocketAddrs for &T {
    type Iter = T::Iter;
    fn to_socket_addrs(&self) -> io::Result<T::Iter> {
        (**self).to_socket_addrs()
    }
}

#[stable(feature = "string_to_socket_addrs", since = "1.16.0")]
impl ToSocketAddrs for String {
    type Iter = vec::IntoIter<SocketAddr>;
    fn to_socket_addrs(&self) -> io::Result<vec::IntoIter<SocketAddr>> {
        (&**self).to_socket_addrs()
    }
}
