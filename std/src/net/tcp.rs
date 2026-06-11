#![deny(unsafe_op_in_unsafe_fn)]

#[cfg(all(
    test,
    not(any(
        target_os = "emscripten",
        all(target_os = "wasi", target_env = "p1"),
        target_os = "xous",
        target_os = "trusty",
    ))
))]
mod tests;

use crate::fmt;
use crate::io::prelude::*;
use crate::io::{self, BorrowedCursor, IoSlice, IoSliceMut};
use crate::iter::FusedIterator;
use crate::net::{Shutdown, SocketAddr, ToSocketAddrs};
use crate::sys::{AsInner, FromInner, IntoInner, net as net_imp};
use crate::time::Duration;

/// 本地套接字与远端套接字之间的 TCP 流（stream）。
///
/// 通过 [`connect`] 连接到远端主机，或在 [`TcpListener`] 上 [`accept`] 一个连接，
/// 即可创建出 `TcpStream`；之后便可对其进行 [reading]（读）和 [writing]（写）来传输
/// 数据。
///
/// 当该值被丢弃（drop）时，连接会被关闭。也可以用 [`shutdown`] 方法单独关闭连接的
/// 读取部分与写入部分（即半关闭）。
///
/// 传输控制协议在 [IETF RFC 793] 中有详细规定。
///
/// [`accept`]: TcpListener::accept
/// [`connect`]: TcpStream::connect
/// [IETF RFC 793]: https://tools.ietf.org/html/rfc793
/// [reading]: Read
/// [`shutdown`]: TcpStream::shutdown
/// [writing]: Write
///
/// # 示例(Examples）
///
/// ```no_run
/// use std::io::prelude::*;
/// use std::net::TcpStream;
///
/// fn main() -> std::io::Result<()> {
///     let mut stream = TcpStream::connect("127.0.0.1:34254")?;
///
///     stream.write(&[1])?;
///     stream.read(&mut [0; 128])?;
///     Ok(())
/// } // 流在此处被关闭
/// ```
///
/// # 平台特定行为(Platform-specific Behavior）
///
/// 在 Unix 上，对处于 `SOCK_STREAM` 模式的底层套接字进行写入时会带上
/// `MSG_NOSIGNAL` 标志。这会抑制在向已断开的套接字写入时发出 `SIGPIPE` 信号。
/// 在某些情况下，收到 `SIGPIPE` 会触发进程终止。
#[stable(feature = "rust1", since = "1.0.0")]
pub struct TcpStream(net_imp::TcpStream);

/// 一个 TCP 套接字服务器，监听连接。
///
/// 通过把 `TcpListener` [`bind`]（绑定）到某个套接字地址来创建它之后，它便会监听
/// 入站的 TCP 连接。可以通过调用 [`accept`]，或通过遍历
/// [`incoming`][`TcpListener::incoming`] 返回的 [`Incoming`] 迭代器来接受这些连接。
///
/// 当该值被丢弃（drop）时，套接字会被关闭。
///
/// 传输控制协议在 [IETF RFC 793] 中有详细规定。
///
/// [`accept`]: TcpListener::accept
/// [`bind`]: TcpListener::bind
/// [IETF RFC 793]: https://tools.ietf.org/html/rfc793
///
/// # 示例(Examples）
///
/// ```no_run
/// use std::net::{TcpListener, TcpStream};
///
/// fn handle_client(stream: TcpStream) {
///     // ...
/// }
///
/// fn main() -> std::io::Result<()> {
///     let listener = TcpListener::bind("127.0.0.1:80")?;
///
///     // 接受连接并依次串行处理
///     for stream in listener.incoming() {
///         handle_client(stream?);
///     }
///     Ok(())
/// }
/// ```
#[stable(feature = "rust1", since = "1.0.0")]
pub struct TcpListener(net_imp::TcpListener);

/// 一个无限地在 [`TcpListener`] 上 [`accept`] 连接的迭代器。
///
/// 此 `struct` 由 [`TcpListener::incoming`] 方法创建。
/// 更多信息参见其文档。
///
/// [`accept`]: TcpListener::accept
#[must_use = "iterators are lazy and do nothing unless consumed"]
#[stable(feature = "rust1", since = "1.0.0")]
#[derive(Debug)]
pub struct Incoming<'a> {
    listener: &'a TcpListener,
}

/// 一个无限地在 [`TcpListener`] 上 [`accept`] 连接的迭代器。
///
/// 此 `struct` 由 [`TcpListener::into_incoming`] 方法创建。
/// 更多信息参见其文档。
///
/// [`accept`]: TcpListener::accept
#[derive(Debug)]
#[unstable(feature = "tcplistener_into_incoming", issue = "88373")]
pub struct IntoIncoming {
    listener: TcpListener,
}

impl TcpStream {
    /// 打开一条到远端主机的 TCP 连接。
    ///
    /// `addr` 是远端主机的地址。任何实现了 [`ToSocketAddrs`] trait 的类型都可以作为
    /// 地址传入；具体示例参见该 trait 的文档。
    ///
    /// 如果 `addr` 产出多个地址，`connect` 会逐一尝试这些地址，直到某次连接成功为止。
    /// 如果所有地址都无法成功建立连接，则返回最后一次连接尝试（即最后一个地址）所返回
    /// 的错误。
    ///
    /// # 示例(Examples）
    ///
    /// 打开一条到 `127.0.0.1:8080` 的 TCP 连接：
    ///
    /// ```no_run
    /// use std::net::TcpStream;
    ///
    /// if let Ok(stream) = TcpStream::connect("127.0.0.1:8080") {
    ///     println!("Connected to the server!");
    /// } else {
    ///     println!("Couldn't connect to server...");
    /// }
    /// ```
    ///
    /// 打开一条到 `127.0.0.1:8080` 的 TCP 连接。如果连接失败，则打开一条到
    /// `127.0.0.1:8081` 的 TCP 连接：
    ///
    /// ```no_run
    /// use std::net::{SocketAddr, TcpStream};
    ///
    /// let addrs = [
    ///     SocketAddr::from(([127, 0, 0, 1], 8080)),
    ///     SocketAddr::from(([127, 0, 0, 1], 8081)),
    /// ];
    /// if let Ok(stream) = TcpStream::connect(&addrs[..]) {
    ///     println!("Connected to the server!");
    /// } else {
    ///     println!("Couldn't connect to server...");
    /// }
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn connect<A: ToSocketAddrs>(addr: A) -> io::Result<TcpStream> {
        net_imp::TcpStream::connect(addr).map(TcpStream)
    }

    /// 带超时地打开一条到远端主机的 TCP 连接。
    ///
    /// 与 `connect` 不同，`connect_timeout` 只接受单个 [`SocketAddr`]，因为超时必须
    /// 施加到具体的某个地址上。
    ///
    /// 向此函数传入零值 `Duration` 是错误的。
    ///
    /// 与 `TcpStream` 上的其他方法不同，此方法并不对应单个系统调用。它会先以非阻塞
    /// 模式调用 `connect`，然后使用操作系统特定的机制来等待连接请求完成。
    #[stable(feature = "tcpstream_connect_timeout", since = "1.21.0")]
    pub fn connect_timeout(addr: &SocketAddr, timeout: Duration) -> io::Result<TcpStream> {
        net_imp::TcpStream::connect_timeout(addr, timeout).map(TcpStream)
    }

    /// 返回此 TCP 连接的远端对等方（peer）的套接字地址。
    ///
    /// # 示例(Examples）
    ///
    /// ```no_run
    /// use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpStream};
    ///
    /// let stream = TcpStream::connect("127.0.0.1:8080")
    ///                        .expect("Couldn't connect to the server...");
    /// assert_eq!(stream.peer_addr().unwrap(),
    ///            SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 8080)));
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        self.0.peer_addr()
    }

    /// 返回此 TCP 连接本地一端的套接字地址。
    ///
    /// # 示例(Examples）
    ///
    /// ```no_run
    /// use std::net::{IpAddr, Ipv4Addr, TcpStream};
    ///
    /// let stream = TcpStream::connect("127.0.0.1:8080")
    ///                        .expect("Couldn't connect to the server...");
    /// assert_eq!(stream.local_addr().unwrap().ip(),
    ///            IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)));
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.0.socket_addr()
    }

    /// 关闭此连接的读取一端、写入一端，或两者皆关闭。
    ///
    /// 此函数会使指定部分上所有挂起的以及将来的 I/O 立即返回一个相应的值（参见
    /// [`Shutdown`] 的文档）。
    ///
    /// # 平台特定行为(Platform-specific behavior）
    ///
    /// 多次调用此函数可能导致不同的行为，取决于操作系统。在 Linux 上，第二次调用会
    /// 返回 `Ok(())`，但在 macOS 上会返回 `ErrorKind::NotConnected`。这一点将来可能
    /// 发生变化。
    ///
    /// # 示例(Examples）
    ///
    /// ```no_run
    /// use std::net::{Shutdown, TcpStream};
    ///
    /// let stream = TcpStream::connect("127.0.0.1:8080")
    ///                        .expect("Couldn't connect to the server...");
    /// stream.shutdown(Shutdown::Both).expect("shutdown call failed");
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn shutdown(&self, how: Shutdown) -> io::Result<()> {
        self.0.shutdown(how)
    }

    /// 创建一个独立持有的、指向底层套接字的新句柄。
    ///
    /// 返回的 `TcpStream` 是对此对象所引用的同一条流的引用。两个句柄都会读写同一条
    /// 数据流，且在其中一条流上设置的选项会传播到另一条流上。
    ///
    /// # 示例(Examples）
    ///
    /// ```no_run
    /// use std::net::TcpStream;
    ///
    /// let stream = TcpStream::connect("127.0.0.1:8080")
    ///                        .expect("Couldn't connect to the server...");
    /// let stream_clone = stream.try_clone().expect("clone failed...");
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn try_clone(&self) -> io::Result<TcpStream> {
        self.0.duplicate().map(TcpStream)
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
    /// [`read`]: Read::read
    /// [`WouldBlock`]: io::ErrorKind::WouldBlock
    /// [`TimedOut`]: io::ErrorKind::TimedOut
    ///
    /// # 示例(Examples）
    ///
    /// ```no_run
    /// use std::net::TcpStream;
    ///
    /// let stream = TcpStream::connect("127.0.0.1:8080")
    ///                        .expect("Couldn't connect to the server...");
    /// stream.set_read_timeout(None).expect("set_read_timeout call failed");
    /// ```
    ///
    /// 如果向此方法传入零值 [`Duration`]，则返回一个 [`Err`]：
    ///
    /// ```no_run
    /// use std::io;
    /// use std::net::TcpStream;
    /// use std::time::Duration;
    ///
    /// let stream = TcpStream::connect("127.0.0.1:8080").unwrap();
    /// let result = stream.set_read_timeout(Some(Duration::new(0, 0)));
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
    /// [`write`]: Write::write
    /// [`WouldBlock`]: io::ErrorKind::WouldBlock
    /// [`TimedOut`]: io::ErrorKind::TimedOut
    ///
    /// # 示例(Examples）
    ///
    /// ```no_run
    /// use std::net::TcpStream;
    ///
    /// let stream = TcpStream::connect("127.0.0.1:8080")
    ///                        .expect("Couldn't connect to the server...");
    /// stream.set_write_timeout(None).expect("set_write_timeout call failed");
    /// ```
    ///
    /// 如果向此方法传入零值 [`Duration`]，则返回一个 [`Err`]：
    ///
    /// ```no_run
    /// use std::io;
    /// use std::net::TcpStream;
    /// use std::time::Duration;
    ///
    /// let stream = TcpStream::connect("127.0.0.1:8080").unwrap();
    /// let result = stream.set_write_timeout(Some(Duration::new(0, 0)));
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
    /// # 平台特定行为(Platform-specific behavior）
    ///
    /// 有些平台不提供访问当前超时值的途径。
    ///
    /// [`read`]: Read::read
    ///
    /// # 示例(Examples）
    ///
    /// ```no_run
    /// use std::net::TcpStream;
    ///
    /// let stream = TcpStream::connect("127.0.0.1:8080")
    ///                        .expect("Couldn't connect to the server...");
    /// stream.set_read_timeout(None).expect("set_read_timeout call failed");
    /// assert_eq!(stream.read_timeout().unwrap(), None);
    /// ```
    #[stable(feature = "socket_timeout", since = "1.4.0")]
    pub fn read_timeout(&self) -> io::Result<Option<Duration>> {
        self.0.read_timeout()
    }

    /// 返回此套接字的写入超时。
    ///
    /// 如果超时为 [`None`]，则 [`write`] 调用会无限期地阻塞。
    ///
    /// # 平台特定行为(Platform-specific behavior）
    ///
    /// 有些平台不提供访问当前超时值的途径。
    ///
    /// [`write`]: Write::write
    ///
    /// # 示例(Examples）
    ///
    /// ```no_run
    /// use std::net::TcpStream;
    ///
    /// let stream = TcpStream::connect("127.0.0.1:8080")
    ///                        .expect("Couldn't connect to the server...");
    /// stream.set_write_timeout(None).expect("set_write_timeout call failed");
    /// assert_eq!(stream.write_timeout().unwrap(), None);
    /// ```
    #[stable(feature = "socket_timeout", since = "1.4.0")]
    pub fn write_timeout(&self) -> io::Result<Option<Duration>> {
        self.0.write_timeout()
    }

    /// 从此套接字所连接到的远端地址接收数据，但不把这些数据从队列中移除。成功时，
    /// 返回窥视（peek）到的字节数。
    ///
    /// 连续多次调用会返回相同的数据。这是通过向底层的 `recv` 系统调用传入 `MSG_PEEK`
    /// 标志来实现的。
    ///
    /// # 示例(Examples）
    ///
    /// ```no_run
    /// use std::net::TcpStream;
    ///
    /// let stream = TcpStream::connect("127.0.0.1:8000")
    ///                        .expect("Couldn't connect to the server...");
    /// let mut buf = [0; 10];
    /// let len = stream.peek(&mut buf).expect("peek failed");
    /// ```
    #[stable(feature = "peek", since = "1.18.0")]
    pub fn peek(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.peek(buf)
    }

    /// 设置此套接字上 `SO_LINGER` 选项的值。
    ///
    /// 此值控制当仍有数据等待发送时套接字如何被关闭。如果设置了 `SO_LINGER`，套接字
    /// 会在系统尝试发送挂起数据的过程中保持打开状态长达指定的时长。否则，系统可能
    /// 立即关闭套接字，或者等待一个默认的超时。
    ///
    /// # 示例(Examples）
    ///
    /// ```no_run
    /// #![feature(tcp_linger)]
    ///
    /// use std::net::TcpStream;
    /// use std::time::Duration;
    ///
    /// let stream = TcpStream::connect("127.0.0.1:8080")
    ///                        .expect("Couldn't connect to the server...");
    /// stream.set_linger(Some(Duration::from_secs(0))).expect("set_linger call failed");
    /// ```
    #[unstable(feature = "tcp_linger", issue = "88494")]
    pub fn set_linger(&self, linger: Option<Duration>) -> io::Result<()> {
        self.0.set_linger(linger)
    }

    /// 获取此套接字上 `SO_LINGER` 选项的值。
    ///
    /// 关于此选项的更多信息，参见 [`TcpStream::set_linger`]。
    ///
    /// # 示例(Examples）
    ///
    /// ```no_run
    /// #![feature(tcp_linger)]
    ///
    /// use std::net::TcpStream;
    /// use std::time::Duration;
    ///
    /// let stream = TcpStream::connect("127.0.0.1:8080")
    ///                        .expect("Couldn't connect to the server...");
    /// stream.set_linger(Some(Duration::from_secs(0))).expect("set_linger call failed");
    /// assert_eq!(stream.linger().unwrap(), Some(Duration::from_secs(0)));
    /// ```
    #[unstable(feature = "tcp_linger", issue = "88494")]
    pub fn linger(&self) -> io::Result<Option<Duration>> {
        self.0.linger()
    }

    /// 设置此套接字上 `TCP_NODELAY` 选项的值。
    ///
    /// 如果设置了此选项，它会禁用 Nagle 算法。这意味着即使只有少量数据，分段
    /// （segment）也总是会尽快被发送出去。未设置时，数据会被缓冲，直到积累到足以
    /// 发送的量为止，从而避免频繁发送小数据包。
    ///
    /// # 示例(Examples）
    ///
    /// ```no_run
    /// use std::net::TcpStream;
    ///
    /// let stream = TcpStream::connect("127.0.0.1:8080")
    ///                        .expect("Couldn't connect to the server...");
    /// stream.set_nodelay(true).expect("set_nodelay call failed");
    /// ```
    #[stable(feature = "net2_mutators", since = "1.9.0")]
    pub fn set_nodelay(&self, nodelay: bool) -> io::Result<()> {
        self.0.set_nodelay(nodelay)
    }

    /// 获取此套接字上 `TCP_NODELAY` 选项的值。
    ///
    /// 关于此选项的更多信息，参见 [`TcpStream::set_nodelay`]。
    ///
    /// # 示例(Examples）
    ///
    /// ```no_run
    /// use std::net::TcpStream;
    ///
    /// let stream = TcpStream::connect("127.0.0.1:8080")
    ///                        .expect("Couldn't connect to the server...");
    /// stream.set_nodelay(true).expect("set_nodelay call failed");
    /// assert_eq!(stream.nodelay().unwrap_or(false), true);
    /// ```
    #[stable(feature = "net2_mutators", since = "1.9.0")]
    pub fn nodelay(&self) -> io::Result<bool> {
        self.0.nodelay()
    }

    /// 设置此套接字上 `IP_TTL` 选项的值。
    ///
    /// 此值设置在每个从此套接字发出的数据包中使用的生存时间（time-to-live）字段。
    ///
    /// # 示例(Examples）
    ///
    /// ```no_run
    /// use std::net::TcpStream;
    ///
    /// let stream = TcpStream::connect("127.0.0.1:8080")
    ///                        .expect("Couldn't connect to the server...");
    /// stream.set_ttl(100).expect("set_ttl call failed");
    /// ```
    #[stable(feature = "net2_mutators", since = "1.9.0")]
    pub fn set_ttl(&self, ttl: u32) -> io::Result<()> {
        self.0.set_ttl(ttl)
    }

    /// 获取此套接字上 `IP_TTL` 选项的值。
    ///
    /// 关于此选项的更多信息，参见 [`TcpStream::set_ttl`]。
    ///
    /// # 示例(Examples）
    ///
    /// ```no_run
    /// use std::net::TcpStream;
    ///
    /// let stream = TcpStream::connect("127.0.0.1:8080")
    ///                        .expect("Couldn't connect to the server...");
    /// stream.set_ttl(100).expect("set_ttl call failed");
    /// assert_eq!(stream.ttl().unwrap_or(0), 100);
    /// ```
    #[stable(feature = "net2_mutators", since = "1.9.0")]
    pub fn ttl(&self) -> io::Result<u32> {
        self.0.ttl()
    }

    /// 获取此套接字上 `SO_ERROR` 选项的值。
    ///
    /// 这会取出底层套接字中存储的错误，并在此过程中清空该字段。这对于在多次调用之间
    /// 检查错误很有用。
    ///
    /// # 示例(Examples）
    ///
    /// ```no_run
    /// use std::net::TcpStream;
    ///
    /// let stream = TcpStream::connect("127.0.0.1:8080")
    ///                        .expect("Couldn't connect to the server...");
    /// stream.take_error().expect("No error was expected...");
    /// ```
    #[stable(feature = "net2_mutators", since = "1.9.0")]
    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        self.0.take_error()
    }

    /// 将此 TCP 流切换进或切换出非阻塞模式。
    ///
    /// 这会使 `read`、`write`、`recv` 和 `send` 等系统操作变为非阻塞，即立即从调用中
    /// 返回。如果 IO 操作成功，则返回 `Ok` 且无需进一步处理。如果 IO 操作无法完成、
    /// 需要重试，则返回一个类型为 [`io::ErrorKind::WouldBlock`] 的错误。
    ///
    /// 在 Unix 平台上，调用此方法对应于调用 `fcntl` 的 `FIONBIO`。在 Windows 上调用
    /// 此方法对应于调用 `ioctlsocket` 的 `FIONBIO`。
    ///
    /// # 示例(Examples）
    ///
    /// 以非阻塞模式从一条 TCP 流读取字节：
    ///
    /// ```no_run
    /// use std::io::{self, Read};
    /// use std::net::TcpStream;
    ///
    /// let mut stream = TcpStream::connect("127.0.0.1:7878")
    ///     .expect("Couldn't connect to the server...");
    /// stream.set_nonblocking(true).expect("set_nonblocking call failed");
    ///
    /// # fn wait_for_fd() { unimplemented!() }
    /// let mut buf = vec![];
    /// loop {
    ///     match stream.read_to_end(&mut buf) {
    ///         Ok(_) => break,
    ///         Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
    ///             // 等待网络套接字就绪，通常通过 epoll、IOCP 等平台特定 API 实现
    ///             wait_for_fd();
    ///         }
    ///         Err(e) => panic!("encountered IO error: {e}"),
    ///     };
    /// };
    /// println!("bytes: {buf:?}");
    /// ```
    #[stable(feature = "net2_mutators", since = "1.9.0")]
    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        self.0.set_nonblocking(nonblocking)
    }
}

// 除了这里的这些 `impl` 之外，`TcpStream` 在 Unix 与 WASI 上还实现了
// `AsFd`/`From<OwnedFd>`/`Into<OwnedFd>` 与
// `AsRawFd`/`IntoRawFd`/`FromRawFd`，在 Windows 上则实现了
// `AsSocket`/`From<OwnedSocket>`/`Into<OwnedSocket>` 与
// `AsRawSocket`/`IntoRawSocket`/`FromRawSocket`。

#[stable(feature = "rust1", since = "1.0.0")]
impl Read for TcpStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.read(buf)
    }

    fn read_buf(&mut self, buf: BorrowedCursor<'_>) -> io::Result<()> {
        self.0.read_buf(buf)
    }

    fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        self.0.read_vectored(bufs)
    }

    #[inline]
    fn is_read_vectored(&self) -> bool {
        self.0.is_read_vectored()
    }
}
#[stable(feature = "rust1", since = "1.0.0")]
impl Write for TcpStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }

    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        self.0.write_vectored(bufs)
    }

    #[inline]
    fn is_write_vectored(&self) -> bool {
        self.0.is_write_vectored()
    }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
#[stable(feature = "rust1", since = "1.0.0")]
impl Read for &TcpStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.read(buf)
    }

    fn read_buf(&mut self, buf: BorrowedCursor<'_>) -> io::Result<()> {
        self.0.read_buf(buf)
    }

    fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        self.0.read_vectored(bufs)
    }

    #[inline]
    fn is_read_vectored(&self) -> bool {
        self.0.is_read_vectored()
    }
}
#[stable(feature = "rust1", since = "1.0.0")]
impl Write for &TcpStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }

    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        self.0.write_vectored(bufs)
    }

    #[inline]
    fn is_write_vectored(&self) -> bool {
        self.0.is_write_vectored()
    }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl AsInner<net_imp::TcpStream> for TcpStream {
    #[inline]
    fn as_inner(&self) -> &net_imp::TcpStream {
        &self.0
    }
}

impl FromInner<net_imp::TcpStream> for TcpStream {
    fn from_inner(inner: net_imp::TcpStream) -> TcpStream {
        TcpStream(inner)
    }
}

impl IntoInner<net_imp::TcpStream> for TcpStream {
    fn into_inner(self) -> net_imp::TcpStream {
        self.0
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl fmt::Debug for TcpStream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl TcpListener {
    /// 创建一个新的 `TcpListener`，它将被绑定到指定的地址。
    ///
    /// 返回的监听器已准备好接受连接。
    ///
    /// 以端口号 0 进行绑定，会请求操作系统为此监听器分配一个端口。可以通过
    /// [`TcpListener::local_addr`] 方法查询分配到的端口。
    ///
    /// 地址类型可以是 [`ToSocketAddrs`] trait 的任意实现者。具体示例参见其文档。
    ///
    /// 如果 `addr` 产出多个地址，`bind` 会逐一尝试这些地址，直到某个成功并返回监听器
    /// 为止。如果所有地址都未能成功创建出监听器，则返回最后一次尝试（即最后一个地址）
    /// 所返回的错误。
    ///
    /// # 示例(Examples）
    ///
    /// 创建一个绑定到 `127.0.0.1:80` 的 TCP 监听器：
    ///
    /// ```no_run
    /// use std::net::TcpListener;
    ///
    /// let listener = TcpListener::bind("127.0.0.1:80").unwrap();
    /// ```
    ///
    /// 创建一个绑定到 `127.0.0.1:80` 的 TCP 监听器。如果失败，则创建一个绑定到
    /// `127.0.0.1:443` 的 TCP 监听器：
    ///
    /// ```no_run
    /// use std::net::{SocketAddr, TcpListener};
    ///
    /// let addrs = [
    ///     SocketAddr::from(([127, 0, 0, 1], 80)),
    ///     SocketAddr::from(([127, 0, 0, 1], 443)),
    /// ];
    /// let listener = TcpListener::bind(&addrs[..]).unwrap();
    /// ```
    ///
    /// 创建一个绑定到 `127.0.0.1` 上由操作系统分配的端口的 TCP 监听器。
    ///
    /// ```no_run
    /// use std::net::TcpListener;
    ///
    /// let socket = TcpListener::bind("127.0.0.1:0").unwrap();
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn bind<A: ToSocketAddrs>(addr: A) -> io::Result<TcpListener> {
        net_imp::TcpListener::bind(addr).map(TcpListener)
    }

    /// 返回此监听器的本地套接字地址。
    ///
    /// # 示例(Examples）
    ///
    /// ```no_run
    /// use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpListener};
    ///
    /// let listener = TcpListener::bind("127.0.0.1:8080").unwrap();
    /// assert_eq!(listener.local_addr().unwrap(),
    ///            SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 8080)));
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        self.0.socket_addr()
    }

    /// 创建一个独立持有的、指向底层套接字的新句柄。
    ///
    /// 返回的 [`TcpListener`] 是对此对象所引用的同一个套接字的引用。两个句柄都可用于
    /// 接受入站连接，且在其中一个监听器上设置的选项会影响另一个。
    ///
    /// # 示例(Examples）
    ///
    /// ```no_run
    /// use std::net::TcpListener;
    ///
    /// let listener = TcpListener::bind("127.0.0.1:8080").unwrap();
    /// let listener_clone = listener.try_clone().unwrap();
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn try_clone(&self) -> io::Result<TcpListener> {
        self.0.duplicate().map(TcpListener)
    }

    /// 从此监听器接受一个新的入站连接。
    ///
    /// 此函数会阻塞调用线程，直到一条新的 TCP 连接建立为止。建立成功后，会返回相应的
    /// [`TcpStream`] 以及远端对等方的地址。
    ///
    /// # 示例(Examples）
    ///
    /// ```no_run
    /// use std::net::TcpListener;
    ///
    /// let listener = TcpListener::bind("127.0.0.1:8080").unwrap();
    /// match listener.accept() {
    ///     Ok((_socket, addr)) => println!("new client: {addr:?}"),
    ///     Err(e) => println!("couldn't get client: {e:?}"),
    /// }
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn accept(&self) -> io::Result<(TcpStream, SocketAddr)> {
        // 在 WASM 上，`TcpStream` 是无人居住类型（uninhabited，因为它不被支持），
        // 因此这里的 `a` 变量从技术上说是未使用的。
        #[cfg_attr(target_arch = "wasm32", allow(unused_variables))]
        self.0.accept().map(|(a, b)| (TcpStream(a), b))
    }

    /// 返回一个迭代器，遍历此监听器上正在接收到的连接。
    ///
    /// 返回的迭代器永远不会返回 [`None`]，也不会产出对等方的 [`SocketAddr`] 结构体。
    /// 遍历它等价于在循环中调用 [`TcpListener::accept`]。
    ///
    /// # 示例(Examples）
    ///
    /// ```no_run
    /// use std::net::{TcpListener, TcpStream};
    ///
    /// fn handle_connection(stream: TcpStream) {
    ///    //...
    /// }
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let listener = TcpListener::bind("127.0.0.1:80")?;
    ///
    ///     for stream in listener.incoming() {
    ///         match stream {
    ///             Ok(stream) => {
    ///                 handle_connection(stream);
    ///             }
    ///             Err(e) => { /* connection failed */ }
    ///         }
    ///     }
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn incoming(&self) -> Incoming<'_> {
        Incoming { listener: self }
    }

    /// 把它转换为一个迭代器，遍历此监听器上正在接收到的连接。
    ///
    /// 返回的迭代器永远不会返回 [`None`]，也不会产出对等方的 [`SocketAddr`] 结构体。
    /// 遍历它等价于在循环中调用 [`TcpListener::accept`]。
    ///
    /// # 示例(Examples）
    ///
    /// ```no_run
    /// #![feature(tcplistener_into_incoming)]
    /// use std::net::{TcpListener, TcpStream};
    ///
    /// fn listen_on(port: u16) -> impl Iterator<Item = TcpStream> {
    ///     let listener = TcpListener::bind(("127.0.0.1", port)).unwrap();
    ///     listener.into_incoming()
    ///         .filter_map(Result::ok) /* 忽略失败的连接 */
    /// }
    ///
    /// fn main() -> std::io::Result<()> {
    ///     for stream in listen_on(80) {
    ///         /* 在此处理连接 */
    ///     }
    ///     Ok(())
    /// }
    /// ```
    #[must_use = "`self` will be dropped if the result is not used"]
    #[unstable(feature = "tcplistener_into_incoming", issue = "88373")]
    pub fn into_incoming(self) -> IntoIncoming {
        IntoIncoming { listener: self }
    }

    /// 设置此套接字上 `IP_TTL` 选项的值。
    ///
    /// 此值设置在每个从此套接字发出的数据包中使用的生存时间（time-to-live）字段。
    ///
    /// # 示例(Examples）
    ///
    /// ```no_run
    /// use std::net::TcpListener;
    ///
    /// let listener = TcpListener::bind("127.0.0.1:80").unwrap();
    /// listener.set_ttl(100).expect("could not set TTL");
    /// ```
    #[stable(feature = "net2_mutators", since = "1.9.0")]
    pub fn set_ttl(&self, ttl: u32) -> io::Result<()> {
        self.0.set_ttl(ttl)
    }

    /// 获取此套接字上 `IP_TTL` 选项的值。
    ///
    /// 关于此选项的更多信息，参见 [`TcpListener::set_ttl`]。
    ///
    /// # 示例(Examples）
    ///
    /// ```no_run
    /// use std::net::TcpListener;
    ///
    /// let listener = TcpListener::bind("127.0.0.1:80").unwrap();
    /// listener.set_ttl(100).expect("could not set TTL");
    /// assert_eq!(listener.ttl().unwrap_or(0), 100);
    /// ```
    #[stable(feature = "net2_mutators", since = "1.9.0")]
    pub fn ttl(&self) -> io::Result<u32> {
        self.0.ttl()
    }

    #[stable(feature = "net2_mutators", since = "1.9.0")]
    #[deprecated(since = "1.16.0", note = "this option can only be set before the socket is bound")]
    #[allow(missing_docs)]
    pub fn set_only_v6(&self, only_v6: bool) -> io::Result<()> {
        self.0.set_only_v6(only_v6)
    }

    #[stable(feature = "net2_mutators", since = "1.9.0")]
    #[deprecated(since = "1.16.0", note = "this option can only be set before the socket is bound")]
    #[allow(missing_docs)]
    pub fn only_v6(&self) -> io::Result<bool> {
        self.0.only_v6()
    }

    /// 获取此套接字上 `SO_ERROR` 选项的值。
    ///
    /// 这会取出底层套接字中存储的错误，并在此过程中清空该字段。这对于在多次调用之间
    /// 检查错误很有用。
    ///
    /// # 示例(Examples）
    ///
    /// ```no_run
    /// use std::net::TcpListener;
    ///
    /// let listener = TcpListener::bind("127.0.0.1:80").unwrap();
    /// listener.take_error().expect("No error was expected");
    /// ```
    #[stable(feature = "net2_mutators", since = "1.9.0")]
    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        self.0.take_error()
    }

    /// 将此 TCP 流切换进或切换出非阻塞模式。
    ///
    /// 这会使 `accept` 操作变为非阻塞，即立即从调用中返回。如果 IO 操作成功，则返回
    /// `Ok` 且无需进一步处理。如果 IO 操作无法完成、需要重试，则返回一个类型为
    /// [`io::ErrorKind::WouldBlock`] 的错误。
    ///
    /// 在 Unix 平台上，调用此方法对应于调用 `fcntl` 的 `FIONBIO`。在 Windows 上调用
    /// 此方法对应于调用 `ioctlsocket` 的 `FIONBIO`。
    ///
    /// # 示例(Examples）
    ///
    /// 把一个 TCP 监听器绑定到某个地址，监听连接，并以非阻塞模式读取字节：
    ///
    /// ```no_run
    /// use std::io;
    /// use std::net::TcpListener;
    ///
    /// let listener = TcpListener::bind("127.0.0.1:7878").unwrap();
    /// listener.set_nonblocking(true).expect("Cannot set non-blocking");
    ///
    /// # fn wait_for_fd() { unimplemented!() }
    /// # fn handle_connection(stream: std::net::TcpStream) { unimplemented!() }
    /// for stream in listener.incoming() {
    ///     match stream {
    ///         Ok(s) => {
    ///             // 对该 TcpStream 做些事情
    ///             handle_connection(s);
    ///         }
    ///         Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => {
    ///             // 等待网络套接字就绪，通常通过 epoll、IOCP 等平台特定 API 实现
    ///             wait_for_fd();
    ///             continue;
    ///         }
    ///         Err(e) => panic!("encountered IO error: {e}"),
    ///     }
    /// }
    /// ```
    #[stable(feature = "net2_mutators", since = "1.9.0")]
    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        self.0.set_nonblocking(nonblocking)
    }
}

// 除了这里的这些 `impl` 之外，`TcpListener` 在 Unix 与 WASI 上还实现了
// `AsFd`/`From<OwnedFd>`/`Into<OwnedFd>` 与
// `AsRawFd`/`IntoRawFd`/`FromRawFd`，在 Windows 上则实现了
// `AsSocket`/`From<OwnedSocket>`/`Into<OwnedSocket>` 与
// `AsRawSocket`/`IntoRawSocket`/`FromRawSocket`。

#[stable(feature = "rust1", since = "1.0.0")]
impl<'a> Iterator for Incoming<'a> {
    type Item = io::Result<TcpStream>;
    fn next(&mut self) -> Option<io::Result<TcpStream>> {
        Some(self.listener.accept().map(|p| p.0))
    }
}

#[stable(feature = "tcp_listener_incoming_fused_iterator", since = "1.64.0")]
impl FusedIterator for Incoming<'_> {}

#[unstable(feature = "tcplistener_into_incoming", issue = "88373")]
impl Iterator for IntoIncoming {
    type Item = io::Result<TcpStream>;
    fn next(&mut self) -> Option<io::Result<TcpStream>> {
        Some(self.listener.accept().map(|p| p.0))
    }
}

#[unstable(feature = "tcplistener_into_incoming", issue = "88373")]
impl FusedIterator for IntoIncoming {}

impl AsInner<net_imp::TcpListener> for TcpListener {
    #[inline]
    fn as_inner(&self) -> &net_imp::TcpListener {
        &self.0
    }
}

impl FromInner<net_imp::TcpListener> for TcpListener {
    fn from_inner(inner: net_imp::TcpListener) -> TcpListener {
        TcpListener(inner)
    }
}

impl IntoInner<net_imp::TcpListener> for TcpListener {
    fn into_inner(self) -> net_imp::TcpListener {
        self.0
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl fmt::Debug for TcpListener {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}
