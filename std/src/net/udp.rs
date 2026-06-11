#[cfg(all(
    test,
    not(any(
        target_os = "emscripten",
        all(target_os = "wasi", target_env = "p1"),
        target_env = "sgx",
        target_os = "xous",
        target_os = "trusty",
    ))
))]
mod tests;

use crate::fmt;
use crate::io::{self, ErrorKind};
use crate::net::{Ipv4Addr, Ipv6Addr, SocketAddr, ToSocketAddrs};
use crate::sys::{AsInner, FromInner, IntoInner, net as net_imp};
use crate::time::Duration;

/// 一个 UDP 套接字。
///
/// 通过把 `UdpSocket` [`bind`]（绑定）到某个套接字地址来创建它之后，便可向任意其他
/// 套接字地址 [sent to]（发送）数据，以及从任意其他套接字地址 [received from]（接收）
/// 数据。
///
/// 尽管 UDP 是一种无连接（connectionless）协议，但此实现提供了一个接口，用于设置数据
/// 应当被发往以及接收自的地址。用 [`connect`] 设置远端地址之后，便可用 [`send`] 和
/// [`recv`] 向该地址发送数据、从该地址接收数据。
///
/// 正如用户数据报协议在 [IETF RFC 768] 中的规范所述，UDP 是一种无序、不可靠的协议；
/// TCP 原语请参阅 [`TcpListener`] 与 [`TcpStream`]。
///
/// [`bind`]: UdpSocket::bind
/// [`connect`]: UdpSocket::connect
/// [IETF RFC 768]: https://tools.ietf.org/html/rfc768
/// [`recv`]: UdpSocket::recv
/// [received from]: UdpSocket::recv_from
/// [`send`]: UdpSocket::send
/// [sent to]: UdpSocket::send_to
/// [`TcpListener`]: crate::net::TcpListener
/// [`TcpStream`]: crate::net::TcpStream
///
/// # 示例(Examples）
///
/// ```no_run
/// use std::net::UdpSocket;
///
/// fn main() -> std::io::Result<()> {
///     {
///         let socket = UdpSocket::bind("127.0.0.1:34254")?;
///
///         // 在套接字上接收单个数据报消息。如果 `buf` 太小，无法容纳整条
///         // 消息，消息会被截断。
///         let mut buf = [0; 10];
///         let (amt, src) = socket.recv_from(&mut buf)?;
///
///         // 把 `buf` 重新声明为接收到的数据的切片，并把数据反转后回发给来源方。
///         let buf = &mut buf[..amt];
///         buf.reverse();
///         socket.send_to(buf, &src)?;
///     } // 套接字在此处被关闭
///     Ok(())
/// }
/// ```
#[stable(feature = "rust1", since = "1.0.0")]
pub struct UdpSocket(net_imp::UdpSocket);

impl UdpSocket {
    /// 从给定地址创建一个 UDP 套接字。
    ///
    /// 地址类型可以是 [`ToSocketAddrs`] trait 的任意实现者。具体示例参见其文档。
    ///
    /// 如果 `addr` 产出多个地址，`bind` 会逐一尝试这些地址，直到某个成功并返回套接字
    /// 为止。如果所有地址都未能成功创建出套接字，则返回最后一次尝试（即最后一个地址）
    /// 所返回的错误。
    ///
    /// # 示例(Examples）
    ///
    /// 创建一个绑定到 `127.0.0.1:3400` 的 UDP 套接字：
    ///
    /// ```no_run
    /// use std::net::UdpSocket;
    ///
    /// let socket = UdpSocket::bind("127.0.0.1:3400").expect("couldn't bind to address");
    /// ```
    ///
    /// 创建一个绑定到 `127.0.0.1:3400` 的 UDP 套接字。如果套接字无法绑定到该地址，
    /// 则创建一个绑定到 `127.0.0.1:3401` 的 UDP 套接字：
    ///
    /// ```no_run
    /// use std::net::{SocketAddr, UdpSocket};
    ///
    /// let addrs = [
    ///     SocketAddr::from(([127, 0, 0, 1], 3400)),
    ///     SocketAddr::from(([127, 0, 0, 1], 3401)),
    /// ];
    /// let socket = UdpSocket::bind(&addrs[..]).expect("couldn't bind to address");
    /// ```
    ///
    /// 创建一个绑定到 `127.0.0.1` 上由操作系统分配的端口的 UDP 套接字。
    ///
    /// ```no_run
    /// use std::net::UdpSocket;
    ///
    /// let socket = UdpSocket::bind("127.0.0.1:0").unwrap();
    /// ```
    ///
    /// 注意，`bind` 声明了你网络连接的作用范围。你只能从该网络视图中的参与方接收
    /// 数据报、向其发送数据报。例如，像上面的示例那样绑定到一个回环（loopback）地址，
    /// 会使你无法向你本地网络中的另一台设备发送数据报。
    ///
    /// 为了尽可能少地限制你的网络视图，可以 `bind` 到 [`Ipv4Addr::UNSPECIFIED`] 或
    /// [`Ipv6Addr::UNSPECIFIED`]。
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn bind<A: ToSocketAddrs>(addr: A) -> io::Result<UdpSocket> {
        net_imp::UdpSocket::bind(addr).map(UdpSocket)
    }

    /// 在套接字上接收单个数据报消息。成功时，返回读取的字节数以及来源方地址。
    ///
    /// 调用此函数时必须传入一个有效的、大小足以容纳消息字节的字节数组 `buf`。如果某条
    /// 消息太长，无法放入所提供的缓冲区，多余的字节可能被丢弃。
    ///
    /// # 示例(Examples）
    ///
    /// ```no_run
    /// use std::net::UdpSocket;
    ///
    /// let socket = UdpSocket::bind("127.0.0.1:34254").expect("couldn't bind to address");
    /// let mut buf = [0; 10];
    /// let (number_of_bytes, src_addr) = socket.recv_from(&mut buf)
    ///                                         .expect("Didn't receive data");
    /// let filled_buf = &mut buf[..number_of_bytes];
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn recv_from(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        self.0.recv_from(buf)
    }

    /// 在套接字上接收单个数据报消息，但不把它从队列中移除。成功时，返回读取的字节数
    /// 以及来源方地址。
    ///
    /// 调用此函数时必须传入一个有效的、大小足以容纳消息字节的字节数组 `buf`。如果某条
    /// 消息太长，无法放入所提供的缓冲区，多余的字节可能被丢弃。
    ///
    /// 连续多次调用会返回相同的数据。这是通过向底层的 `recvfrom` 系统调用传入
    /// `MSG_PEEK` 标志来实现的。
    ///
    /// 不要用此函数来实现忙等待（busy waiting），而应使用 `libc::poll` 来同步一个或多个
    /// 套接字上的 IO 事件。
    ///
    /// # 示例(Examples）
    ///
    /// ```no_run
    /// use std::net::UdpSocket;
    ///
    /// let socket = UdpSocket::bind("127.0.0.1:34254").expect("couldn't bind to address");
    /// let mut buf = [0; 10];
    /// let (number_of_bytes, src_addr) = socket.peek_from(&mut buf)
    ///                                         .expect("Didn't receive data");
    /// let filled_buf = &mut buf[..number_of_bytes];
    /// ```
    #[stable(feature = "peek", since = "1.18.0")]
    pub fn peek_from(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        self.0.peek_from(buf)
    }

    /// 在套接字上向给定地址发送数据。成功时，返回写入的字节数。注意操作系统可能拒绝
    /// 大于 65507 的缓冲区。不过，在缓冲区大小超过 `i32::MAX` 之前，是不会发生部分写入
    /// （partial writes）的。
    ///
    /// 地址类型可以是 [`ToSocketAddrs`] trait 的任意实现者。具体示例参见其文档。
    ///
    /// `addr` 有可能产出多个地址，但 `send_to` 只会把数据发送到 `addr` 产出的第一个
    /// 地址。
    ///
    /// 当本地套接字的 IP 版本与 [`ToSocketAddrs`] 返回的 IP 版本不匹配时，这会返回一个
    /// 错误。
    ///
    /// 更多细节参见 [Issue #34202]。
    ///
    /// # 示例(Examples）
    ///
    /// ```no_run
    /// use std::net::UdpSocket;
    ///
    /// let socket = UdpSocket::bind("127.0.0.1:34254").expect("couldn't bind to address");
    /// socket.send_to(&[0; 10], "127.0.0.1:4242").expect("couldn't send data");
    /// ```
    ///
    /// [Issue #34202]: https://github.com/rust-lang/rust/issues/34202
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn send_to<A: ToSocketAddrs>(&self, buf: &[u8], addr: A) -> io::Result<usize> {
        match addr.to_socket_addrs()?.next() {
            Some(addr) => self.0.send_to(buf, &addr),
            None => Err(io::const_error!(ErrorKind::InvalidInput, "no addresses to send data to")),
        }
    }

    /// 返回此套接字曾连接到的远端对等方（peer）的套接字地址。
    ///
    /// # 示例(Examples）
    ///
    /// ```no_run
    /// use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket};
    ///
    /// let socket = UdpSocket::bind("127.0.0.1:34254").expect("couldn't bind to address");
    /// socket.connect("192.168.0.1:41203").expect("couldn't connect to address");
    /// assert_eq!(socket.peer_addr().unwrap(),
    ///            SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(192, 168, 0, 1), 41203)));
    /// ```
    ///
    /// 如果套接字未连接，它会返回一个 [`NotConnected`] 错误。
    ///
    /// [`NotConnected`]: io::ErrorKind::NotConnected
    ///
    /// ```no_run
    /// use std::net::UdpSocket;
    ///
    /// let socket = UdpSocket::bind("127.0.0.1:34254").expect("couldn't bind to address");
    /// assert_eq!(socket.peer_addr().unwrap_err().kind(),
    ///            std::io::ErrorKind::NotConnected);
    /// ```
    #[stable(feature = "udp_peer_addr", since = "1.40.0")]
    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        self.0.peer_addr()
    }

    /// 返回此套接字创建时所用的套接字地址。
    ///
    /// # 示例(Examples）
    ///
    /// ```no_run
    /// use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, UdpSocket};
    ///
    /// let socket = UdpSocket::bind("127.0.0.1:34254").expect("couldn't bind to address");
    /// assert_eq!(socket.local_addr().unwrap(),
    ///            SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 34254)));
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.0.socket_addr()
    }

    /// 创建一个独立持有的、指向底层套接字的新句柄。
    ///
    /// 返回的 `UdpSocket` 是对此对象所引用的同一个套接字的引用。两个句柄都会读写同一个
    /// 端口，且在其中一个套接字上设置的选项会传播到另一个。
    ///
    /// # 示例(Examples）
    ///
    /// ```no_run
    /// use std::net::UdpSocket;
    ///
    /// let socket = UdpSocket::bind("127.0.0.1:34254").expect("couldn't bind to address");
    /// let socket_clone = socket.try_clone().expect("couldn't clone the socket");
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn try_clone(&self) -> io::Result<UdpSocket> {
        self.0.duplicate().map(UdpSocket)
    }

    /// 将读取超时设置为指定的超时时长。
    ///
    /// 如果指定的值为 [`None`]，则 [`read`] 调用会无限期地阻塞。如果向此方法传入零值
    /// [`Duration`]，则返回一个 [`Err`]。
    ///
    /// # 平台特定行为(Platform-specific behavior）
    ///
    /// 每当读取因设置了此选项而超时时，不同平台可能返回不同的错误码。例如 Unix 通常
    /// 返回 [`WouldBlock`] 类型的错误，但 Windows 可能返回 [`TimedOut`]。
    ///
    /// [`read`]: io::Read::read
    /// [`WouldBlock`]: io::ErrorKind::WouldBlock
    /// [`TimedOut`]: io::ErrorKind::TimedOut
    ///
    /// # 示例(Examples）
    ///
    /// ```no_run
    /// use std::net::UdpSocket;
    ///
    /// let socket = UdpSocket::bind("127.0.0.1:34254").expect("couldn't bind to address");
    /// socket.set_read_timeout(None).expect("set_read_timeout call failed");
    /// ```
    ///
    /// 如果向此方法传入零值 [`Duration`]，则返回一个 [`Err`]：
    ///
    /// ```no_run
    /// use std::io;
    /// use std::net::UdpSocket;
    /// use std::time::Duration;
    ///
    /// let socket = UdpSocket::bind("127.0.0.1:34254").unwrap();
    /// let result = socket.set_read_timeout(Some(Duration::new(0, 0)));
    /// let err = result.unwrap_err();
    /// assert_eq!(err.kind(), io::ErrorKind::InvalidInput)
    /// ```
    #[stable(feature = "socket_timeout", since = "1.4.0")]
    pub fn set_read_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        self.0.set_read_timeout(dur)
    }

    /// 将写入超时设置为指定的超时时长。
    ///
    /// 如果指定的值为 [`None`]，则 [`write`] 调用会无限期地阻塞。如果向此方法传入零值
    /// [`Duration`]，则返回一个 [`Err`]。
    ///
    /// # 平台特定行为(Platform-specific behavior）
    ///
    /// 每当写入因设置了此选项而超时时，不同平台可能返回不同的错误码。例如 Unix 通常
    /// 返回 [`WouldBlock`] 类型的错误，但 Windows 可能返回 [`TimedOut`]。
    ///
    /// [`write`]: io::Write::write
    /// [`WouldBlock`]: io::ErrorKind::WouldBlock
    /// [`TimedOut`]: io::ErrorKind::TimedOut
    ///
    /// # 示例(Examples）
    ///
    /// ```no_run
    /// use std::net::UdpSocket;
    ///
    /// let socket = UdpSocket::bind("127.0.0.1:34254").expect("couldn't bind to address");
    /// socket.set_write_timeout(None).expect("set_write_timeout call failed");
    /// ```
    ///
    /// 如果向此方法传入零值 [`Duration`]，则返回一个 [`Err`]：
    ///
    /// ```no_run
    /// use std::io;
    /// use std::net::UdpSocket;
    /// use std::time::Duration;
    ///
    /// let socket = UdpSocket::bind("127.0.0.1:34254").unwrap();
    /// let result = socket.set_write_timeout(Some(Duration::new(0, 0)));
    /// let err = result.unwrap_err();
    /// assert_eq!(err.kind(), io::ErrorKind::InvalidInput)
    /// ```
    #[stable(feature = "socket_timeout", since = "1.4.0")]
    pub fn set_write_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        self.0.set_write_timeout(dur)
    }

    /// 返回此套接字的读取超时。
    ///
    /// 如果超时为 [`None`]，则 [`read`] 调用会无限期地阻塞。
    ///
    /// [`read`]: io::Read::read
    ///
    /// # 示例(Examples）
    ///
    /// ```no_run
    /// use std::net::UdpSocket;
    ///
    /// let socket = UdpSocket::bind("127.0.0.1:34254").expect("couldn't bind to address");
    /// socket.set_read_timeout(None).expect("set_read_timeout call failed");
    /// assert_eq!(socket.read_timeout().unwrap(), None);
    /// ```
    #[stable(feature = "socket_timeout", since = "1.4.0")]
    pub fn read_timeout(&self) -> io::Result<Option<Duration>> {
        self.0.read_timeout()
    }

    /// 返回此套接字的写入超时。
    ///
    /// 如果超时为 [`None`]，则 [`write`] 调用会无限期地阻塞。
    ///
    /// [`write`]: io::Write::write
    ///
    /// # 示例(Examples）
    ///
    /// ```no_run
    /// use std::net::UdpSocket;
    ///
    /// let socket = UdpSocket::bind("127.0.0.1:34254").expect("couldn't bind to address");
    /// socket.set_write_timeout(None).expect("set_write_timeout call failed");
    /// assert_eq!(socket.write_timeout().unwrap(), None);
    /// ```
    #[stable(feature = "socket_timeout", since = "1.4.0")]
    pub fn write_timeout(&self) -> io::Result<Option<Duration>> {
        self.0.write_timeout()
    }

    /// 设置此套接字上 `SO_BROADCAST` 选项的值。
    ///
    /// 启用时，此套接字被允许向广播（broadcast）地址发送数据包。
    ///
    /// # 示例(Examples）
    ///
    /// ```no_run
    /// use std::net::UdpSocket;
    ///
    /// let socket = UdpSocket::bind("127.0.0.1:34254").expect("couldn't bind to address");
    /// socket.set_broadcast(false).expect("set_broadcast call failed");
    /// ```
    #[stable(feature = "net2_mutators", since = "1.9.0")]
    pub fn set_broadcast(&self, broadcast: bool) -> io::Result<()> {
        self.0.set_broadcast(broadcast)
    }

    /// 获取此套接字上 `SO_BROADCAST` 选项的值。
    ///
    /// 关于此选项的更多信息，参见 [`UdpSocket::set_broadcast`]。
    ///
    /// # 示例(Examples）
    ///
    /// ```no_run
    /// use std::net::UdpSocket;
    ///
    /// let socket = UdpSocket::bind("127.0.0.1:34254").expect("couldn't bind to address");
    /// socket.set_broadcast(false).expect("set_broadcast call failed");
    /// assert_eq!(socket.broadcast().unwrap(), false);
    /// ```
    #[stable(feature = "net2_mutators", since = "1.9.0")]
    pub fn broadcast(&self) -> io::Result<bool> {
        self.0.broadcast()
    }

    /// 设置此套接字上 `IP_MULTICAST_LOOP` 选项的值。
    ///
    /// 如果启用，组播（multicast）数据包会被环回（loop back）到本地套接字。注意这在
    /// IPv6 套接字上可能没有任何效果。
    ///
    /// # 示例(Examples）
    ///
    /// ```no_run
    /// use std::net::UdpSocket;
    ///
    /// let socket = UdpSocket::bind("127.0.0.1:34254").expect("couldn't bind to address");
    /// socket.set_multicast_loop_v4(false).expect("set_multicast_loop_v4 call failed");
    /// ```
    #[stable(feature = "net2_mutators", since = "1.9.0")]
    pub fn set_multicast_loop_v4(&self, multicast_loop_v4: bool) -> io::Result<()> {
        self.0.set_multicast_loop_v4(multicast_loop_v4)
    }

    /// 获取此套接字上 `IP_MULTICAST_LOOP` 选项的值。
    ///
    /// 关于此选项的更多信息，参见 [`UdpSocket::set_multicast_loop_v4`]。
    ///
    /// # 示例(Examples）
    ///
    /// ```no_run
    /// use std::net::UdpSocket;
    ///
    /// let socket = UdpSocket::bind("127.0.0.1:34254").expect("couldn't bind to address");
    /// socket.set_multicast_loop_v4(false).expect("set_multicast_loop_v4 call failed");
    /// assert_eq!(socket.multicast_loop_v4().unwrap(), false);
    /// ```
    #[stable(feature = "net2_mutators", since = "1.9.0")]
    pub fn multicast_loop_v4(&self) -> io::Result<bool> {
        self.0.multicast_loop_v4()
    }

    /// 设置此套接字上 `IP_MULTICAST_TTL` 选项的值。
    ///
    /// 指示此套接字发出的组播数据包的生存时间（time-to-live）值。默认值为 1，这意味着
    /// 除非显式请求，否则组播数据包不会离开本地网络。
    ///
    /// 注意这在 IPv6 套接字上可能没有任何效果。
    ///
    /// # 示例(Examples）
    ///
    /// ```no_run
    /// use std::net::UdpSocket;
    ///
    /// let socket = UdpSocket::bind("127.0.0.1:34254").expect("couldn't bind to address");
    /// socket.set_multicast_ttl_v4(42).expect("set_multicast_ttl_v4 call failed");
    /// ```
    #[stable(feature = "net2_mutators", since = "1.9.0")]
    pub fn set_multicast_ttl_v4(&self, multicast_ttl_v4: u32) -> io::Result<()> {
        self.0.set_multicast_ttl_v4(multicast_ttl_v4)
    }

    /// 获取此套接字上 `IP_MULTICAST_TTL` 选项的值。
    ///
    /// 关于此选项的更多信息，参见 [`UdpSocket::set_multicast_ttl_v4`]。
    ///
    /// # 示例(Examples）
    ///
    /// ```no_run
    /// use std::net::UdpSocket;
    ///
    /// let socket = UdpSocket::bind("127.0.0.1:34254").expect("couldn't bind to address");
    /// socket.set_multicast_ttl_v4(42).expect("set_multicast_ttl_v4 call failed");
    /// assert_eq!(socket.multicast_ttl_v4().unwrap(), 42);
    /// ```
    #[stable(feature = "net2_mutators", since = "1.9.0")]
    pub fn multicast_ttl_v4(&self) -> io::Result<u32> {
        self.0.multicast_ttl_v4()
    }

    /// 设置此套接字上 `IPV6_MULTICAST_LOOP` 选项的值。
    ///
    /// 控制此套接字是否能看到自己发出的组播数据包。注意这在 IPv4 套接字上可能没有任何
    /// 效果。
    ///
    /// # 示例(Examples）
    ///
    /// ```no_run
    /// use std::net::UdpSocket;
    ///
    /// let socket = UdpSocket::bind("127.0.0.1:34254").expect("couldn't bind to address");
    /// socket.set_multicast_loop_v6(false).expect("set_multicast_loop_v6 call failed");
    /// ```
    #[stable(feature = "net2_mutators", since = "1.9.0")]
    pub fn set_multicast_loop_v6(&self, multicast_loop_v6: bool) -> io::Result<()> {
        self.0.set_multicast_loop_v6(multicast_loop_v6)
    }

    /// 获取此套接字上 `IPV6_MULTICAST_LOOP` 选项的值。
    ///
    /// 关于此选项的更多信息，参见 [`UdpSocket::set_multicast_loop_v6`]。
    ///
    /// # 示例(Examples）
    ///
    /// ```no_run
    /// use std::net::UdpSocket;
    ///
    /// let socket = UdpSocket::bind("127.0.0.1:34254").expect("couldn't bind to address");
    /// socket.set_multicast_loop_v6(false).expect("set_multicast_loop_v6 call failed");
    /// assert_eq!(socket.multicast_loop_v6().unwrap(), false);
    /// ```
    #[stable(feature = "net2_mutators", since = "1.9.0")]
    pub fn multicast_loop_v6(&self) -> io::Result<bool> {
        self.0.multicast_loop_v6()
    }

    /// 设置此套接字上 `IP_TTL` 选项的值。
    ///
    /// 此值设置在每个从此套接字发出的数据包中使用的生存时间（time-to-live）字段。
    ///
    /// # 示例(Examples）
    ///
    /// ```no_run
    /// use std::net::UdpSocket;
    ///
    /// let socket = UdpSocket::bind("127.0.0.1:34254").expect("couldn't bind to address");
    /// socket.set_ttl(42).expect("set_ttl call failed");
    /// ```
    #[stable(feature = "net2_mutators", since = "1.9.0")]
    pub fn set_ttl(&self, ttl: u32) -> io::Result<()> {
        self.0.set_ttl(ttl)
    }

    /// 获取此套接字上 `IP_TTL` 选项的值。
    ///
    /// 关于此选项的更多信息，参见 [`UdpSocket::set_ttl`]。
    ///
    /// # 示例(Examples）
    ///
    /// ```no_run
    /// use std::net::UdpSocket;
    ///
    /// let socket = UdpSocket::bind("127.0.0.1:34254").expect("couldn't bind to address");
    /// socket.set_ttl(42).expect("set_ttl call failed");
    /// assert_eq!(socket.ttl().unwrap(), 42);
    /// ```
    #[stable(feature = "net2_mutators", since = "1.9.0")]
    pub fn ttl(&self) -> io::Result<u32> {
        self.0.ttl()
    }

    /// 执行一次 `IP_ADD_MEMBERSHIP` 类型的操作。
    ///
    /// 此函数为此套接字指定一个要加入的新组播组。地址必须是有效的组播地址，而
    /// `interface` 是系统应当借以加入该组播组的本地接口的地址。如果它等于
    /// [`UNSPECIFIED`](Ipv4Addr::UNSPECIFIED)，则由系统选择一个合适的接口。
    #[stable(feature = "net2_mutators", since = "1.9.0")]
    pub fn join_multicast_v4(&self, multiaddr: &Ipv4Addr, interface: &Ipv4Addr) -> io::Result<()> {
        self.0.join_multicast_v4(multiaddr, interface)
    }

    /// 执行一次 `IPV6_ADD_MEMBERSHIP` 类型的操作。
    ///
    /// 此函数为此套接字指定一个要加入的新组播组。地址必须是有效的组播地址，而
    /// `interface` 是要加入/离开的接口的索引（或用 0 表示任意接口）。
    #[stable(feature = "net2_mutators", since = "1.9.0")]
    pub fn join_multicast_v6(&self, multiaddr: &Ipv6Addr, interface: u32) -> io::Result<()> {
        self.0.join_multicast_v6(multiaddr, interface)
    }

    /// 执行一次 `IP_DROP_MEMBERSHIP` 类型的操作。
    ///
    /// 关于此选项的更多信息，参见 [`UdpSocket::join_multicast_v4`]。
    #[stable(feature = "net2_mutators", since = "1.9.0")]
    pub fn leave_multicast_v4(&self, multiaddr: &Ipv4Addr, interface: &Ipv4Addr) -> io::Result<()> {
        self.0.leave_multicast_v4(multiaddr, interface)
    }

    /// 执行一次 `IPV6_DROP_MEMBERSHIP` 类型的操作。
    ///
    /// 关于此选项的更多信息，参见 [`UdpSocket::join_multicast_v6`]。
    #[stable(feature = "net2_mutators", since = "1.9.0")]
    pub fn leave_multicast_v6(&self, multiaddr: &Ipv6Addr, interface: u32) -> io::Result<()> {
        self.0.leave_multicast_v6(multiaddr, interface)
    }

    /// 获取此套接字上 `SO_ERROR` 选项的值。
    ///
    /// 这会取出底层套接字中存储的错误，并在此过程中清空该字段。这对于在多次调用之间
    /// 检查错误很有用。
    ///
    /// # 示例(Examples）
    ///
    /// ```no_run
    /// use std::net::UdpSocket;
    ///
    /// let socket = UdpSocket::bind("127.0.0.1:34254").expect("couldn't bind to address");
    /// match socket.take_error() {
    ///     Ok(Some(error)) => println!("UdpSocket error: {error:?}"),
    ///     Ok(None) => println!("No error"),
    ///     Err(error) => println!("UdpSocket.take_error failed: {error:?}"),
    /// }
    /// ```
    #[stable(feature = "net2_mutators", since = "1.9.0")]
    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        self.0.take_error()
    }

    /// 把此 UDP 套接字连接到一个远端地址，使得 `send` 和 `recv` 系统调用可被用来发送
    /// 数据，同时施加过滤器，只接收来自指定地址的数据。
    ///
    /// 如果 `addr` 产出多个地址，`connect` 会逐一尝试这些地址，直到底层操作系统函数
    /// 返回无错误为止。注意，`connect` 调用成功通常并不表示远端服务器正在该端口上
    /// 监听；相反，这类错误只有在第一次发送之后才会被检测到。如果操作系统对每个指定
    /// 地址都返回错误，则返回最后一次连接尝试（即最后一个地址）所返回的错误。
    ///
    /// # 示例(Examples）
    ///
    /// 创建一个绑定到 `127.0.0.1:3400` 的 UDP 套接字，并把它连接到 `127.0.0.1:8080`：
    ///
    /// ```no_run
    /// use std::net::UdpSocket;
    ///
    /// let socket = UdpSocket::bind("127.0.0.1:3400").expect("couldn't bind to address");
    /// socket.connect("127.0.0.1:8080").expect("connect function failed");
    /// ```
    ///
    /// 与 TCP 的情形不同，向 UDP 套接字的 `connect` 函数传入一个地址数组并没有什么用：
    /// 在应用程序发送数据之前，操作系统无法判断远端地址上是否有东西在监听。
    ///
    /// 如果你的第一次 `connect` 是连接到回环（loopback）地址，那么后续连接到非回环
    /// 地址的 `connect` 可能会失败，这取决于平台。
    #[stable(feature = "net2_mutators", since = "1.9.0")]
    pub fn connect<A: ToSocketAddrs>(&self, addr: A) -> io::Result<()> {
        self.0.connect(addr)
    }

    /// 在套接字上向它所连接到的远端地址发送数据。成功时，返回写入的字节数。注意操作
    /// 系统可能拒绝大于 65507 的缓冲区。不过，在缓冲区大小超过 `i32::MAX` 之前，是不会
    /// 发生部分写入（partial writes）的。
    ///
    /// [`UdpSocket::connect`] 会把此套接字连接到一个远端地址。如果套接字未连接，此方法
    /// 会失败。
    ///
    /// # 示例(Examples）
    ///
    /// ```no_run
    /// use std::net::UdpSocket;
    ///
    /// let socket = UdpSocket::bind("127.0.0.1:34254").expect("couldn't bind to address");
    /// socket.connect("127.0.0.1:8080").expect("connect function failed");
    /// socket.send(&[0, 1, 2]).expect("couldn't send message");
    /// ```
    #[stable(feature = "net2_mutators", since = "1.9.0")]
    pub fn send(&self, buf: &[u8]) -> io::Result<usize> {
        self.0.send(buf)
    }

    /// 在套接字上接收来自它所连接到的远端地址的单个数据报消息。成功时，返回读取的
    /// 字节数。
    ///
    /// 调用此函数时必须传入一个有效的、大小足以容纳消息字节的字节数组 `buf`。如果某条
    /// 消息太长，无法放入所提供的缓冲区，多余的字节可能被丢弃。
    ///
    /// [`UdpSocket::connect`] 会把此套接字连接到一个远端地址。如果套接字未连接，此方法
    /// 会失败。
    ///
    /// # 示例(Examples）
    ///
    /// ```no_run
    /// use std::net::UdpSocket;
    ///
    /// let socket = UdpSocket::bind("127.0.0.1:34254").expect("couldn't bind to address");
    /// socket.connect("127.0.0.1:8080").expect("connect function failed");
    /// let mut buf = [0; 10];
    /// match socket.recv(&mut buf) {
    ///     Ok(received) => println!("received {received} bytes {:?}", &buf[..received]),
    ///     Err(e) => println!("recv function failed: {e:?}"),
    /// }
    /// ```
    #[stable(feature = "net2_mutators", since = "1.9.0")]
    pub fn recv(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.recv(buf)
    }

    /// 在套接字上接收来自它所连接到的远端地址的单个数据报，但不把该消息从输入队列中
    /// 移除。成功时，返回窥视（peek）到的字节数。
    ///
    /// 调用此函数时必须传入一个有效的、大小足以容纳消息字节的字节数组 `buf`。如果某条
    /// 消息太长，无法放入所提供的缓冲区，多余的字节可能被丢弃。
    ///
    /// 连续多次调用会返回相同的数据。这是通过向底层的 `recv` 系统调用传入 `MSG_PEEK`
    /// 标志来实现的。
    ///
    /// 不要用此函数来实现忙等待（busy waiting），而应使用 `libc::poll` 来同步一个或多个
    /// 套接字上的 IO 事件。
    ///
    /// [`UdpSocket::connect`] 会把此套接字连接到一个远端地址。如果套接字未连接，此方法
    /// 会失败。
    ///
    /// # 错误(Errors）
    ///
    /// 如果套接字未连接，此方法会失败。`connect` 方法会把此套接字连接到一个远端地址。
    ///
    /// # 示例(Examples）
    ///
    /// ```no_run
    /// use std::net::UdpSocket;
    ///
    /// let socket = UdpSocket::bind("127.0.0.1:34254").expect("couldn't bind to address");
    /// socket.connect("127.0.0.1:8080").expect("connect function failed");
    /// let mut buf = [0; 10];
    /// match socket.peek(&mut buf) {
    ///     Ok(received) => println!("received {received} bytes"),
    ///     Err(e) => println!("peek function failed: {e:?}"),
    /// }
    /// ```
    #[stable(feature = "peek", since = "1.18.0")]
    pub fn peek(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.peek(buf)
    }

    /// 将此 UDP 套接字切换进或切换出非阻塞模式。
    ///
    /// 这会使 `recv`、`recv_from`、`send` 和 `send_to` 等系统操作变为非阻塞，即立即从
    /// 调用中返回。如果 IO 操作成功，则返回 `Ok` 且无需进一步处理。如果 IO 操作无法
    /// 完成、需要重试，则返回一个类型为 [`io::ErrorKind::WouldBlock`] 的错误。
    ///
    /// 在 Unix 平台上，调用此方法对应于调用 `fcntl` 的 `FIONBIO`。在 Windows 上调用
    /// 此方法对应于调用 `ioctlsocket` 的 `FIONBIO`。
    ///
    /// # 示例(Examples）
    ///
    /// 创建一个绑定到 `127.0.0.1:7878` 的 UDP 套接字，并以非阻塞模式读取字节：
    ///
    /// ```no_run
    /// use std::io;
    /// use std::net::UdpSocket;
    ///
    /// let socket = UdpSocket::bind("127.0.0.1:7878").unwrap();
    /// socket.set_nonblocking(true).unwrap();
    ///
    /// # fn wait_for_fd() { unimplemented!() }
    /// let mut buf = [0; 10];
    /// let (num_bytes_read, _) = loop {
    ///     match socket.recv_from(&mut buf) {
    ///         Ok(n) => break n,
    ///         Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
    ///             // 等待网络套接字就绪，通常通过 epoll、IOCP 等平台特定 API 实现
    ///             wait_for_fd();
    ///         }
    ///         Err(e) => panic!("encountered IO error: {e}"),
    ///     }
    /// };
    /// println!("bytes: {:?}", &buf[..num_bytes_read]);
    /// ```
    #[stable(feature = "net2_mutators", since = "1.9.0")]
    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        self.0.set_nonblocking(nonblocking)
    }
}

// 除了这里的这些 `impl` 之外，`UdpSocket` 在 Unix 与 WASI 上还实现了
// `AsFd`/`From<OwnedFd>`/`Into<OwnedFd>` 与
// `AsRawFd`/`IntoRawFd`/`FromRawFd`，在 Windows 上则实现了
// `AsSocket`/`From<OwnedSocket>`/`Into<OwnedSocket>` 与
// `AsRawSocket`/`IntoRawSocket`/`FromRawSocket`。

impl AsInner<net_imp::UdpSocket> for UdpSocket {
    #[inline]
    fn as_inner(&self) -> &net_imp::UdpSocket {
        &self.0
    }
}

impl FromInner<net_imp::UdpSocket> for UdpSocket {
    fn from_inner(inner: net_imp::UdpSocket) -> UdpSocket {
        UdpSocket(inner)
    }
}

impl IntoInner<net_imp::UdpSocket> for UdpSocket {
    fn into_inner(self) -> net_imp::UdpSocket {
        self.0
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl fmt::Debug for UdpSocket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
