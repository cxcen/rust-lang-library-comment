cfg_select! {
    any(
        target_os = "linux", target_os = "android",
        target_os = "hurd",
        target_os = "dragonfly", target_os = "freebsd",
        target_os = "openbsd", target_os = "netbsd",
        target_os = "solaris", target_os = "illumos",
        target_os = "haiku", target_os = "nto",
        target_os = "cygwin",
    ) => {
        use libc::MSG_NOSIGNAL;
    }
    _ => {
        const MSG_NOSIGNAL: core::ffi::c_int = 0x0;
    }
}

use super::{SocketAddr, sockaddr_un};
#[cfg(any(doc, target_os = "android", target_os = "linux", target_os = "cygwin"))]
use super::{SocketAncillary, recv_vectored_with_ancillary_from, send_vectored_with_ancillary_to};
#[cfg(any(
    target_os = "android",
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "nto",
    target_vendor = "apple",
    target_os = "cygwin"
))]
use super::{UCred, peer_cred};
use crate::fmt;
use crate::io::{self, IoSlice, IoSliceMut};
use crate::net::Shutdown;
use crate::os::unix::io::{AsFd, AsRawFd, BorrowedFd, FromRawFd, IntoRawFd, OwnedFd, RawFd};
use crate::path::Path;
use crate::sealed::Sealed;
use crate::sys::net::Socket;
use crate::sys::{AsInner, FromInner, cvt};
use crate::time::Duration;

/// 一个 Unix 流式套接字（stream socket）。
///
/// # 示例
///
/// ```no_run
/// use std::os::unix::net::UnixStream;
/// use std::io::prelude::*;
///
/// fn main() -> std::io::Result<()> {
///     let mut stream = UnixStream::connect("/path/to/my/socket")?;
///     stream.write_all(b"hello world")?;
///     let mut response = String::new();
///     stream.read_to_string(&mut response)?;
///     println!("{response}");
///     Ok(())
/// }
/// ```
///
/// # `SOCK_CLOEXEC`
///
/// 在支持它的平台上，我们传入 close-on-exec 标志以原子地创建套接字并将其设置为 CLOEXEC。
/// 在 Linux 上，这是在 2.6.27 中加入的。更多信息参见 [`socket(2)`]。
///
/// [`socket(2)`]: https://www.man7.org/linux/man-pages/man2/socket.2.html#:~:text=SOCK_CLOEXEC
///
/// # `SIGPIPE`
///
/// 在 `SOCK_STREAM` 模式下，对底层套接字的写入会带上 `MSG_NOSIGNAL` 标志。
/// 这会在向已断开连接的套接字写入时抑制 `SIGPIPE` 信号的发出。
/// 在某些情况下，收到 `SIGPIPE` 会触发进程终止。
#[stable(feature = "unix_socket", since = "1.10.0")]
pub struct UnixStream(pub(super) Socket);

/// 允许 `std` 内部使用的扩展 trait。
#[unstable(feature = "sealed", issue = "none")]
impl Sealed for UnixStream {}

#[stable(feature = "unix_socket", since = "1.10.0")]
impl fmt::Debug for UnixStream {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut builder = fmt.debug_struct("UnixStream");
        builder.field("fd", self.0.as_inner());
        if let Ok(addr) = self.local_addr() {
            builder.field("local", &addr);
        }
        if let Ok(addr) = self.peer_addr() {
            builder.field("peer", &addr);
        }
        builder.finish()
    }
}

impl UnixStream {
    /// 连接到由 `path` 命名的套接字。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::os::unix::net::UnixStream;
    ///
    /// let socket = match UnixStream::connect("/tmp/sock") {
    ///     Ok(sock) => sock,
    ///     Err(e) => {
    ///         println!("Couldn't connect: {e:?}");
    ///         return
    ///     }
    /// };
    /// ```
    #[stable(feature = "unix_socket", since = "1.10.0")]
    pub fn connect<P: AsRef<Path>>(path: P) -> io::Result<UnixStream> {
        unsafe {
            let inner = Socket::new(libc::AF_UNIX, libc::SOCK_STREAM)?;
            let (addr, len) = sockaddr_un(path.as_ref())?;

            cvt(libc::connect(inner.as_raw_fd(), (&raw const addr) as *const _, len))?;
            Ok(UnixStream(inner))
        }
    }

    /// 连接到由 [`address`] 指定的套接字。
    ///
    /// [`address`]: crate::os::unix::net::SocketAddr
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::os::unix::net::{UnixListener, UnixStream};
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let listener = UnixListener::bind("/path/to/the/socket")?;
    ///     let addr = listener.local_addr()?;
    ///
    ///     let sock = match UnixStream::connect_addr(&addr) {
    ///         Ok(sock) => sock,
    ///         Err(e) => {
    ///             println!("Couldn't connect: {e:?}");
    ///             return Err(e)
    ///         }
    ///     };
    ///     Ok(())
    /// }
    /// ````
    #[stable(feature = "unix_socket_abstract", since = "1.70.0")]
    pub fn connect_addr(socket_addr: &SocketAddr) -> io::Result<UnixStream> {
        unsafe {
            let inner = Socket::new(libc::AF_UNIX, libc::SOCK_STREAM)?;
            cvt(libc::connect(
                inner.as_raw_fd(),
                (&raw const socket_addr.addr) as *const _,
                socket_addr.len,
            ))?;
            Ok(UnixStream(inner))
        }
    }

    /// 创建一对未命名的、彼此相连的套接字。
    ///
    /// 返回两个彼此相连的 `UnixStream`。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::os::unix::net::UnixStream;
    ///
    /// let (sock1, sock2) = match UnixStream::pair() {
    ///     Ok((sock1, sock2)) => (sock1, sock2),
    ///     Err(e) => {
    ///         println!("Couldn't create a pair of sockets: {e:?}");
    ///         return
    ///     }
    /// };
    /// ```
    #[stable(feature = "unix_socket", since = "1.10.0")]
    pub fn pair() -> io::Result<(UnixStream, UnixStream)> {
        let (i1, i2) = Socket::new_pair(libc::AF_UNIX, libc::SOCK_STREAM)?;
        Ok((UnixStream(i1), UnixStream(i2)))
    }

    /// 为底层套接字创建一个新的、独立拥有所有权的句柄。
    ///
    /// 返回的 `UnixStream` 引用的是与此对象所引用的同一个流。两个句柄都将读写同一个数据流，
    /// 且在一个流上设置的选项将传播到另一个流。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::os::unix::net::UnixStream;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let socket = UnixStream::connect("/tmp/sock")?;
    ///     let sock_copy = socket.try_clone().expect("Couldn't clone socket");
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "unix_socket", since = "1.10.0")]
    pub fn try_clone(&self) -> io::Result<UnixStream> {
        self.0.duplicate().map(UnixStream)
    }

    /// 返回此连接本地半边（local half）的套接字地址。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::os::unix::net::UnixStream;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let socket = UnixStream::connect("/tmp/sock")?;
    ///     let addr = socket.local_addr().expect("Couldn't get local address");
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "unix_socket", since = "1.10.0")]
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        SocketAddr::new(|addr, len| unsafe { libc::getsockname(self.as_raw_fd(), addr, len) })
    }

    /// 返回此连接远程半边（remote half）的套接字地址。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::os::unix::net::UnixStream;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let socket = UnixStream::connect("/tmp/sock")?;
    ///     let addr = socket.peer_addr().expect("Couldn't get peer address");
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "unix_socket", since = "1.10.0")]
    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        SocketAddr::new(|addr, len| unsafe { libc::getpeername(self.as_raw_fd(), addr, len) })
    }

    /// 获取此 Unix 域套接字的对端凭据（peer credentials）。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// #![feature(peer_credentials_unix_socket)]
    /// use std::os::unix::net::UnixStream;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let socket = UnixStream::connect("/tmp/sock")?;
    ///     let peer_cred = socket.peer_cred().expect("Couldn't get peer credentials");
    ///     Ok(())
    /// }
    /// ```
    #[unstable(feature = "peer_credentials_unix_socket", issue = "42839")]
    #[cfg(any(
        target_os = "android",
        target_os = "linux",
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "nto",
        target_vendor = "apple",
        target_os = "cygwin"
    ))]
    pub fn peer_cred(&self) -> io::Result<UCred> {
        peer_cred(self)
    }

    /// 设置套接字的读超时（read timeout）。
    ///
    /// 如果所提供的值是 [`None`]，那么 [`read`] 调用将无限期阻塞。如果向此方法传入了
    /// 零值 [`Duration`]，则返回 [`Err`]。
    ///
    /// [`read`]: io::Read::read
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::os::unix::net::UnixStream;
    /// use std::time::Duration;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let socket = UnixStream::connect("/tmp/sock")?;
    ///     socket.set_read_timeout(Some(Duration::new(1, 0))).expect("Couldn't set read timeout");
    ///     Ok(())
    /// }
    /// ```
    ///
    /// 如果向此方法传入了零值 [`Duration`]，则返回 [`Err`]：
    ///
    /// ```no_run
    /// use std::io;
    /// use std::os::unix::net::UnixStream;
    /// use std::time::Duration;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let socket = UnixStream::connect("/tmp/sock")?;
    ///     let result = socket.set_read_timeout(Some(Duration::new(0, 0)));
    ///     let err = result.unwrap_err();
    ///     assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "unix_socket", since = "1.10.0")]
    pub fn set_read_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        self.0.set_timeout(timeout, libc::SO_RCVTIMEO)
    }

    /// 设置套接字的写超时（write timeout）。
    ///
    /// 如果所提供的值是 [`None`]，那么 [`write`] 调用将无限期阻塞。如果向此方法传入了
    /// 零值 [`Duration`]，则返回 [`Err`]。
    ///
    /// [`read`]: io::Read::read
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::os::unix::net::UnixStream;
    /// use std::time::Duration;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let socket = UnixStream::connect("/tmp/sock")?;
    ///     socket.set_write_timeout(Some(Duration::new(1, 0)))
    ///         .expect("Couldn't set write timeout");
    ///     Ok(())
    /// }
    /// ```
    ///
    /// 如果向此方法传入了零值 [`Duration`]，则返回 [`Err`]：
    ///
    /// ```no_run
    /// use std::io;
    /// use std::os::unix::net::UnixStream;
    /// use std::time::Duration;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let socket = UnixStream::connect("/tmp/sock")?;
    ///     let result = socket.set_write_timeout(Some(Duration::new(0, 0)));
    ///     let err = result.unwrap_err();
    ///     assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "unix_socket", since = "1.10.0")]
    pub fn set_write_timeout(&self, timeout: Option<Duration>) -> io::Result<()> {
        self.0.set_timeout(timeout, libc::SO_SNDTIMEO)
    }

    /// 返回此套接字的读超时（read timeout）。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::os::unix::net::UnixStream;
    /// use std::time::Duration;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let socket = UnixStream::connect("/tmp/sock")?;
    ///     socket.set_read_timeout(Some(Duration::new(1, 0))).expect("Couldn't set read timeout");
    ///     assert_eq!(socket.read_timeout()?, Some(Duration::new(1, 0)));
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "unix_socket", since = "1.10.0")]
    pub fn read_timeout(&self) -> io::Result<Option<Duration>> {
        self.0.timeout(libc::SO_RCVTIMEO)
    }

    /// 返回此套接字的写超时（write timeout）。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::os::unix::net::UnixStream;
    /// use std::time::Duration;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let socket = UnixStream::connect("/tmp/sock")?;
    ///     socket.set_write_timeout(Some(Duration::new(1, 0)))
    ///         .expect("Couldn't set write timeout");
    ///     assert_eq!(socket.write_timeout()?, Some(Duration::new(1, 0)));
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "unix_socket", since = "1.10.0")]
    pub fn write_timeout(&self) -> io::Result<Option<Duration>> {
        self.0.timeout(libc::SO_SNDTIMEO)
    }

    /// 将套接字切换进入或退出非阻塞（nonblocking）模式。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::os::unix::net::UnixStream;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let socket = UnixStream::connect("/tmp/sock")?;
    ///     socket.set_nonblocking(true).expect("Couldn't set nonblocking");
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "unix_socket", since = "1.10.0")]
    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        self.0.set_nonblocking(nonblocking)
    }

    /// 出于网络过滤（network filtering）目的设置套接字的 id
    ///
    #[cfg_attr(
        any(target_os = "linux", target_os = "freebsd", target_os = "openbsd"),
        doc = "```no_run"
    )]
    #[cfg_attr(
        not(any(target_os = "linux", target_os = "freebsd", target_os = "openbsd")),
        doc = "```ignore"
    )]
    /// #![feature(unix_set_mark)]
    /// use std::os::unix::net::UnixStream;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let sock = UnixStream::connect("/tmp/sock")?;
    ///     sock.set_mark(32)?;
    ///     Ok(())
    /// }
    /// ```
    #[cfg(any(doc, target_os = "linux", target_os = "freebsd", target_os = "openbsd",))]
    #[unstable(feature = "unix_set_mark", issue = "96467")]
    pub fn set_mark(&self, mark: u32) -> io::Result<()> {
        self.0.set_mark(mark)
    }

    /// 返回 `SO_ERROR` 选项的值。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::os::unix::net::UnixStream;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let socket = UnixStream::connect("/tmp/sock")?;
    ///     if let Ok(Some(err)) = socket.take_error() {
    ///         println!("Got error: {err:?}");
    ///     }
    ///     Ok(())
    /// }
    /// ```
    ///
    /// # Platform specific
    /// 在 Redox 上，这总是返回 `None`。
    #[stable(feature = "unix_socket", since = "1.10.0")]
    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        self.0.take_error()
    }

    /// 关闭此连接的读半边、写半边，或两者。
    ///
    /// 此函数将导致所指定部分上所有挂起的与未来的 I/O 调用立即返回一个适当的值
    ///（参见 [`Shutdown`] 的文档）。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::os::unix::net::UnixStream;
    /// use std::net::Shutdown;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let socket = UnixStream::connect("/tmp/sock")?;
    ///     socket.shutdown(Shutdown::Both).expect("shutdown function failed");
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "unix_socket", since = "1.10.0")]
    pub fn shutdown(&self, how: Shutdown) -> io::Result<()> {
        self.0.shutdown(how)
    }

    /// 从此套接字所连接的远程地址接收数据，但不将该数据从队列中移除。成功时，
    /// 返回窥视（peeked）的字节数。
    ///
    /// 连续多次调用会返回相同的数据。这是通过把 `MSG_PEEK` 作为标志传给底层的 `recv`
    /// 系统调用实现的。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// #![feature(unix_socket_peek)]
    ///
    /// use std::os::unix::net::UnixStream;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let socket = UnixStream::connect("/tmp/sock")?;
    ///     let mut buf = [0; 10];
    ///     let len = socket.peek(&mut buf).expect("peek failed");
    ///     Ok(())
    /// }
    /// ```
    #[unstable(feature = "unix_socket_peek", issue = "76923")]
    pub fn peek(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.peek(buf)
    }

    /// 从套接字接收数据与辅助数据（ancillary data）。
    ///
    /// 成功时，返回读取的字节数。
    ///
    /// # 示例
    ///
    #[cfg_attr(
        any(target_os = "android", target_os = "linux", target_os = "cygwin"),
        doc = "```no_run"
    )]
    #[cfg_attr(
        not(any(target_os = "android", target_os = "linux", target_os = "cygwin")),
        doc = "```ignore"
    )]
    /// #![feature(unix_socket_ancillary_data)]
    /// use std::os::unix::net::{UnixStream, SocketAncillary, AncillaryData};
    /// use std::io::IoSliceMut;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let socket = UnixStream::connect("/tmp/sock")?;
    ///     let mut buf1 = [1; 8];
    ///     let mut buf2 = [2; 16];
    ///     let mut buf3 = [3; 8];
    ///     let mut bufs = &mut [
    ///         IoSliceMut::new(&mut buf1),
    ///         IoSliceMut::new(&mut buf2),
    ///         IoSliceMut::new(&mut buf3),
    ///     ][..];
    ///     let mut fds = [0; 8];
    ///     let mut ancillary_buffer = [0; 128];
    ///     let mut ancillary = SocketAncillary::new(&mut ancillary_buffer[..]);
    ///     let size = socket.recv_vectored_with_ancillary(bufs, &mut ancillary)?;
    ///     println!("received {size}");
    ///     for ancillary_result in ancillary.messages() {
    ///         if let AncillaryData::ScmRights(scm_rights) = ancillary_result.unwrap() {
    ///             for fd in scm_rights {
    ///                 println!("receive file descriptor: {fd}");
    ///             }
    ///         }
    ///     }
    ///     Ok(())
    /// }
    /// ```
    #[cfg(any(doc, target_os = "android", target_os = "linux", target_os = "cygwin"))]
    #[unstable(feature = "unix_socket_ancillary_data", issue = "76915")]
    pub fn recv_vectored_with_ancillary(
        &self,
        bufs: &mut [IoSliceMut<'_>],
        ancillary: &mut SocketAncillary<'_>,
    ) -> io::Result<usize> {
        let (count, _, _) = recv_vectored_with_ancillary_from(&self.0, bufs, ancillary)?;

        Ok(count)
    }

    /// 在套接字上发送数据与辅助数据（ancillary data）。
    ///
    /// 成功时，返回写入的字节数。
    ///
    /// # 示例
    ///
    #[cfg_attr(
        any(target_os = "android", target_os = "linux", target_os = "cygwin"),
        doc = "```no_run"
    )]
    #[cfg_attr(
        not(any(target_os = "android", target_os = "linux", target_os = "cygwin")),
        doc = "```ignore"
    )]
    /// #![feature(unix_socket_ancillary_data)]
    /// use std::os::unix::net::{UnixStream, SocketAncillary};
    /// use std::io::IoSlice;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let socket = UnixStream::connect("/tmp/sock")?;
    ///     let buf1 = [1; 8];
    ///     let buf2 = [2; 16];
    ///     let buf3 = [3; 8];
    ///     let bufs = &[
    ///         IoSlice::new(&buf1),
    ///         IoSlice::new(&buf2),
    ///         IoSlice::new(&buf3),
    ///     ][..];
    ///     let fds = [0, 1, 2];
    ///     let mut ancillary_buffer = [0; 128];
    ///     let mut ancillary = SocketAncillary::new(&mut ancillary_buffer[..]);
    ///     ancillary.add_fds(&fds[..]);
    ///     socket.send_vectored_with_ancillary(bufs, &mut ancillary)
    ///         .expect("send_vectored_with_ancillary function failed");
    ///     Ok(())
    /// }
    /// ```
    #[cfg(any(doc, target_os = "android", target_os = "linux", target_os = "cygwin"))]
    #[unstable(feature = "unix_socket_ancillary_data", issue = "76915")]
    pub fn send_vectored_with_ancillary(
        &self,
        bufs: &[IoSlice<'_>],
        ancillary: &mut SocketAncillary<'_>,
    ) -> io::Result<usize> {
        send_vectored_with_ancillary_to(&self.0, None, bufs, ancillary)
    }
}

#[stable(feature = "unix_socket", since = "1.10.0")]
impl io::Read for UnixStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        io::Read::read(&mut &*self, buf)
    }

    fn read_buf(&mut self, buf: io::BorrowedCursor<'_>) -> io::Result<()> {
        io::Read::read_buf(&mut &*self, buf)
    }

    fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        io::Read::read_vectored(&mut &*self, bufs)
    }

    #[inline]
    fn is_read_vectored(&self) -> bool {
        io::Read::is_read_vectored(&&*self)
    }
}

#[stable(feature = "unix_socket", since = "1.10.0")]
impl<'a> io::Read for &'a UnixStream {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.read(buf)
    }

    fn read_buf(&mut self, buf: io::BorrowedCursor<'_>) -> io::Result<()> {
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

#[stable(feature = "unix_socket", since = "1.10.0")]
impl io::Write for UnixStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        io::Write::write(&mut &*self, buf)
    }

    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        io::Write::write_vectored(&mut &*self, bufs)
    }

    #[inline]
    fn is_write_vectored(&self) -> bool {
        io::Write::is_write_vectored(&&*self)
    }

    fn flush(&mut self) -> io::Result<()> {
        io::Write::flush(&mut &*self)
    }
}

#[stable(feature = "unix_socket", since = "1.10.0")]
impl<'a> io::Write for &'a UnixStream {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.send_with_flags(buf, MSG_NOSIGNAL)
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

#[stable(feature = "unix_socket", since = "1.10.0")]
impl AsRawFd for UnixStream {
    #[inline]
    fn as_raw_fd(&self) -> RawFd {
        self.0.as_raw_fd()
    }
}

#[stable(feature = "unix_socket", since = "1.10.0")]
impl FromRawFd for UnixStream {
    #[inline]
    unsafe fn from_raw_fd(fd: RawFd) -> UnixStream {
        UnixStream(Socket::from_inner(FromInner::from_inner(OwnedFd::from_raw_fd(fd))))
    }
}

#[stable(feature = "unix_socket", since = "1.10.0")]
impl IntoRawFd for UnixStream {
    #[inline]
    fn into_raw_fd(self) -> RawFd {
        self.0.into_raw_fd()
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl AsFd for UnixStream {
    #[inline]
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0.as_fd()
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl From<UnixStream> for OwnedFd {
    /// 取得一个 [`UnixStream`] 的套接字文件描述符的所有权。
    #[inline]
    fn from(unix_stream: UnixStream) -> OwnedFd {
        unsafe { OwnedFd::from_raw_fd(unix_stream.into_raw_fd()) }
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl From<OwnedFd> for UnixStream {
    #[inline]
    fn from(owned: OwnedFd) -> Self {
        unsafe { Self::from_raw_fd(owned.into_raw_fd()) }
    }
}

impl AsInner<Socket> for UnixStream {
    #[inline]
    fn as_inner(&self) -> &Socket {
        &self.0
    }
}
