#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "solaris",
    target_os = "illumos",
    target_os = "haiku",
    target_os = "nto",
    target_os = "cygwin"
))]
use libc::MSG_NOSIGNAL;

use super::{SocketAddr, sockaddr_un};
#[cfg(any(doc, target_os = "android", target_os = "linux", target_os = "cygwin"))]
use super::{SocketAncillary, recv_vectored_with_ancillary_from, send_vectored_with_ancillary_to};
#[cfg(any(doc, target_os = "android", target_os = "linux", target_os = "cygwin"))]
use crate::io::{IoSlice, IoSliceMut};
use crate::net::Shutdown;
use crate::os::unix::io::{AsFd, AsRawFd, BorrowedFd, FromRawFd, IntoRawFd, OwnedFd, RawFd};
use crate::path::Path;
use crate::sealed::Sealed;
use crate::sys::net::Socket;
use crate::sys::{AsInner, FromInner, IntoInner, cvt};
use crate::time::Duration;
use crate::{fmt, io};
#[cfg(not(any(
    target_os = "linux",
    target_os = "android",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "solaris",
    target_os = "illumos",
    target_os = "haiku",
    target_os = "nto",
    target_os = "cygwin"
)))]
const MSG_NOSIGNAL: core::ffi::c_int = 0x0;

/// 一个 Unix 数据报套接字（datagram socket）。
///
/// # 示例
///
/// ```no_run
/// use std::os::unix::net::UnixDatagram;
///
/// fn main() -> std::io::Result<()> {
///     let socket = UnixDatagram::bind("/path/to/my/socket")?;
///     socket.send_to(b"hello world", "/path/to/other/socket")?;
///     let mut buf = [0; 100];
///     let (count, address) = socket.recv_from(&mut buf)?;
///     println!("socket {:?} sent {:?}", address, &buf[..count]);
///     Ok(())
/// }
/// ```
#[stable(feature = "unix_socket", since = "1.10.0")]
pub struct UnixDatagram(Socket);

/// 允许 `std` 内部使用的扩展 trait。
#[unstable(feature = "sealed", issue = "none")]
impl Sealed for UnixDatagram {}

#[stable(feature = "unix_socket", since = "1.10.0")]
impl fmt::Debug for UnixDatagram {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut builder = fmt.debug_struct("UnixDatagram");
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

impl UnixDatagram {
    /// 创建一个绑定到给定路径的 Unix 数据报套接字。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::os::unix::net::UnixDatagram;
    ///
    /// let sock = match UnixDatagram::bind("/path/to/the/socket") {
    ///     Ok(sock) => sock,
    ///     Err(e) => {
    ///         println!("Couldn't bind: {e:?}");
    ///         return
    ///     }
    /// };
    /// ```
    #[stable(feature = "unix_socket", since = "1.10.0")]
    pub fn bind<P: AsRef<Path>>(path: P) -> io::Result<UnixDatagram> {
        unsafe {
            let socket = UnixDatagram::unbound()?;
            let (addr, len) = sockaddr_un(path.as_ref())?;

            cvt(libc::bind(socket.as_raw_fd(), (&raw const addr) as *const _, len as _))?;

            Ok(socket)
        }
    }

    /// 创建一个绑定到某个地址的 Unix 数据报套接字。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::os::unix::net::{UnixDatagram};
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let sock1 = UnixDatagram::bind("path/to/socket")?;
    ///     let addr = sock1.local_addr()?;
    ///
    ///     let sock2 = match UnixDatagram::bind_addr(&addr) {
    ///         Ok(sock) => sock,
    ///         Err(err) => {
    ///             println!("Couldn't bind: {err:?}");
    ///             return Err(err);
    ///         }
    ///     };
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "unix_socket_abstract", since = "1.70.0")]
    pub fn bind_addr(socket_addr: &SocketAddr) -> io::Result<UnixDatagram> {
        unsafe {
            let socket = UnixDatagram::unbound()?;
            cvt(libc::bind(
                socket.as_raw_fd(),
                (&raw const socket_addr.addr) as *const _,
                socket_addr.len as _,
            ))?;
            Ok(socket)
        }
    }

    /// 创建一个不绑定到任何地址的 Unix 数据报套接字。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::os::unix::net::UnixDatagram;
    ///
    /// let sock = match UnixDatagram::unbound() {
    ///     Ok(sock) => sock,
    ///     Err(e) => {
    ///         println!("Couldn't unbound: {e:?}");
    ///         return
    ///     }
    /// };
    /// ```
    #[stable(feature = "unix_socket", since = "1.10.0")]
    pub fn unbound() -> io::Result<UnixDatagram> {
        let inner = Socket::new(libc::AF_UNIX, libc::SOCK_DGRAM)?;
        Ok(UnixDatagram(inner))
    }

    /// 创建一对未命名的、彼此相连的套接字。
    ///
    /// 返回两个彼此相连的 `UnixDatagrams`。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::os::unix::net::UnixDatagram;
    ///
    /// let (sock1, sock2) = match UnixDatagram::pair() {
    ///     Ok((sock1, sock2)) => (sock1, sock2),
    ///     Err(e) => {
    ///         println!("Couldn't unbound: {e:?}");
    ///         return
    ///     }
    /// };
    /// ```
    #[stable(feature = "unix_socket", since = "1.10.0")]
    pub fn pair() -> io::Result<(UnixDatagram, UnixDatagram)> {
        let (i1, i2) = Socket::new_pair(libc::AF_UNIX, libc::SOCK_DGRAM)?;
        Ok((UnixDatagram(i1), UnixDatagram(i2)))
    }

    /// 将套接字连接到指定的路径地址。
    ///
    /// [`send`] 方法可用于向指定地址发送数据。
    /// [`recv`] 与 [`recv_from`] 将只从该地址接收数据。
    ///
    /// [`send`]: UnixDatagram::send
    /// [`recv`]: UnixDatagram::recv
    /// [`recv_from`]: UnixDatagram::recv_from
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::os::unix::net::UnixDatagram;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let sock = UnixDatagram::unbound()?;
    ///     match sock.connect("/path/to/the/socket") {
    ///         Ok(sock) => sock,
    ///         Err(e) => {
    ///             println!("Couldn't connect: {e:?}");
    ///             return Err(e)
    ///         }
    ///     };
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "unix_socket", since = "1.10.0")]
    pub fn connect<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
        unsafe {
            let (addr, len) = sockaddr_un(path.as_ref())?;

            cvt(libc::connect(self.as_raw_fd(), (&raw const addr) as *const _, len))?;
        }
        Ok(())
    }

    /// 将套接字连接到某个地址。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::os::unix::net::{UnixDatagram};
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let bound = UnixDatagram::bind("/path/to/socket")?;
    ///     let addr = bound.local_addr()?;
    ///
    ///     let sock = UnixDatagram::unbound()?;
    ///     match sock.connect_addr(&addr) {
    ///         Ok(sock) => sock,
    ///         Err(e) => {
    ///             println!("Couldn't connect: {e:?}");
    ///             return Err(e)
    ///         }
    ///     };
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "unix_socket_abstract", since = "1.70.0")]
    pub fn connect_addr(&self, socket_addr: &SocketAddr) -> io::Result<()> {
        unsafe {
            cvt(libc::connect(
                self.as_raw_fd(),
                (&raw const socket_addr.addr) as *const _,
                socket_addr.len,
            ))?;
        }
        Ok(())
    }

    /// 为底层套接字创建一个新的、独立拥有所有权的句柄。
    ///
    /// 返回的 `UnixDatagram` 引用的是与此对象所引用的同一个套接字。两个句柄都可用于接受
    /// 入站连接，且在一侧设置的选项将影响另一侧。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::os::unix::net::UnixDatagram;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let sock = UnixDatagram::bind("/path/to/the/socket")?;
    ///     let sock_copy = sock.try_clone().expect("try_clone failed");
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "unix_socket", since = "1.10.0")]
    pub fn try_clone(&self) -> io::Result<UnixDatagram> {
        self.0.duplicate().map(UnixDatagram)
    }

    /// 返回此套接字的地址。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::os::unix::net::UnixDatagram;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let sock = UnixDatagram::bind("/path/to/the/socket")?;
    ///     let addr = sock.local_addr().expect("Couldn't get local address");
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "unix_socket", since = "1.10.0")]
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        SocketAddr::new(|addr, len| unsafe { libc::getsockname(self.as_raw_fd(), addr, len) })
    }

    /// 返回此套接字对端的地址。
    ///
    /// [`connect`] 方法会把套接字连接到一个对端。
    ///
    /// [`connect`]: UnixDatagram::connect
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::os::unix::net::UnixDatagram;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let sock = UnixDatagram::unbound()?;
    ///     sock.connect("/path/to/the/socket")?;
    ///
    ///     let addr = sock.peer_addr().expect("Couldn't get peer address");
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "unix_socket", since = "1.10.0")]
    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        SocketAddr::new(|addr, len| unsafe { libc::getpeername(self.as_raw_fd(), addr, len) })
    }

    fn recv_from_flags(
        &self,
        buf: &mut [u8],
        flags: core::ffi::c_int,
    ) -> io::Result<(usize, SocketAddr)> {
        let mut count = 0;
        let addr = SocketAddr::new(|addr, len| unsafe {
            count = libc::recvfrom(
                self.as_raw_fd(),
                buf.as_mut_ptr() as *mut _,
                buf.len(),
                flags,
                addr,
                len,
            );
            if count > 0 {
                1
            } else if count == 0 {
                0
            } else {
                -1
            }
        })?;

        Ok((count as usize, addr))
    }

    /// 从套接字接收数据。
    ///
    /// 成功时，返回读取的字节数以及数据来源的地址。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::os::unix::net::UnixDatagram;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let sock = UnixDatagram::unbound()?;
    ///     let mut buf = vec![0; 10];
    ///     let (size, sender) = sock.recv_from(buf.as_mut_slice())?;
    ///     println!("received {size} bytes from {sender:?}");
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "unix_socket", since = "1.10.0")]
    pub fn recv_from(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        self.recv_from_flags(buf, 0)
    }

    /// 从套接字接收数据。
    ///
    /// 成功时，返回读取的字节数。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::os::unix::net::UnixDatagram;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let sock = UnixDatagram::bind("/path/to/the/socket")?;
    ///     let mut buf = vec![0; 10];
    ///     sock.recv(buf.as_mut_slice()).expect("recv function failed");
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "unix_socket", since = "1.10.0")]
    pub fn recv(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.read(buf)
    }

    /// 从套接字接收数据与辅助数据（ancillary data）。
    ///
    /// 成功时，返回读取的字节数、数据是否被截断（truncated），以及消息来源的地址。
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
    /// use std::os::unix::net::{UnixDatagram, SocketAncillary, AncillaryData};
    /// use std::io::IoSliceMut;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let sock = UnixDatagram::unbound()?;
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
    ///     let (size, _truncated, sender) = sock.recv_vectored_with_ancillary_from(bufs, &mut ancillary)?;
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
    pub fn recv_vectored_with_ancillary_from(
        &self,
        bufs: &mut [IoSliceMut<'_>],
        ancillary: &mut SocketAncillary<'_>,
    ) -> io::Result<(usize, bool, SocketAddr)> {
        let (count, truncated, addr) = recv_vectored_with_ancillary_from(&self.0, bufs, ancillary)?;
        let addr = addr?;

        Ok((count, truncated, addr))
    }

    /// 从套接字接收数据与辅助数据（ancillary data）。
    ///
    /// 成功时，返回读取的字节数，以及数据是否被截断（truncated）。
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
    /// use std::os::unix::net::{UnixDatagram, SocketAncillary, AncillaryData};
    /// use std::io::IoSliceMut;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let sock = UnixDatagram::unbound()?;
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
    ///     let (size, _truncated) = sock.recv_vectored_with_ancillary(bufs, &mut ancillary)?;
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
    ) -> io::Result<(usize, bool)> {
        let (count, truncated, addr) = recv_vectored_with_ancillary_from(&self.0, bufs, ancillary)?;
        addr?;

        Ok((count, truncated))
    }

    /// 在套接字上向指定地址发送数据。
    ///
    /// 成功时，返回写入的字节数。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::os::unix::net::UnixDatagram;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let sock = UnixDatagram::unbound()?;
    ///     sock.send_to(b"omelette au fromage", "/some/sock").expect("send_to function failed");
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "unix_socket", since = "1.10.0")]
    pub fn send_to<P: AsRef<Path>>(&self, buf: &[u8], path: P) -> io::Result<usize> {
        unsafe {
            let (addr, len) = sockaddr_un(path.as_ref())?;

            let count = cvt(libc::sendto(
                self.as_raw_fd(),
                buf.as_ptr() as *const _,
                buf.len(),
                MSG_NOSIGNAL,
                (&raw const addr) as *const _,
                len,
            ))?;
            Ok(count as usize)
        }
    }

    /// 在套接字上向指定的 [SocketAddr] 发送数据。
    ///
    /// 成功时，返回写入的字节数。
    ///
    /// [SocketAddr]: crate::os::unix::net::SocketAddr
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::os::unix::net::{UnixDatagram};
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let bound = UnixDatagram::bind("/path/to/socket")?;
    ///     let addr = bound.local_addr()?;
    ///
    ///     let sock = UnixDatagram::unbound()?;
    ///     sock.send_to_addr(b"bacon egg and cheese", &addr).expect("send_to_addr function failed");
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "unix_socket_abstract", since = "1.70.0")]
    pub fn send_to_addr(&self, buf: &[u8], socket_addr: &SocketAddr) -> io::Result<usize> {
        unsafe {
            let count = cvt(libc::sendto(
                self.as_raw_fd(),
                buf.as_ptr() as *const _,
                buf.len(),
                MSG_NOSIGNAL,
                (&raw const socket_addr.addr) as *const _,
                socket_addr.len,
            ))?;
            Ok(count as usize)
        }
    }

    /// 在套接字上向套接字的对端发送数据。
    ///
    /// 对端地址可由 `connect` 方法设置；如果该套接字尚未被连接，此方法将返回错误。
    ///
    /// 成功时，返回写入的字节数。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::os::unix::net::UnixDatagram;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let sock = UnixDatagram::unbound()?;
    ///     sock.connect("/some/sock").expect("Couldn't connect");
    ///     sock.send(b"omelette au fromage").expect("send_to function failed");
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "unix_socket", since = "1.10.0")]
    pub fn send(&self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }

    /// 在套接字上向指定地址发送数据与辅助数据（ancillary data）。
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
    /// use std::os::unix::net::{UnixDatagram, SocketAncillary};
    /// use std::io::IoSlice;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let sock = UnixDatagram::unbound()?;
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
    ///     sock.send_vectored_with_ancillary_to(bufs, &mut ancillary, "/some/sock")
    ///         .expect("send_vectored_with_ancillary_to function failed");
    ///     Ok(())
    /// }
    /// ```
    #[cfg(any(doc, target_os = "android", target_os = "linux", target_os = "cygwin"))]
    #[unstable(feature = "unix_socket_ancillary_data", issue = "76915")]
    pub fn send_vectored_with_ancillary_to<P: AsRef<Path>>(
        &self,
        bufs: &[IoSlice<'_>],
        ancillary: &mut SocketAncillary<'_>,
        path: P,
    ) -> io::Result<usize> {
        send_vectored_with_ancillary_to(&self.0, Some(path.as_ref()), bufs, ancillary)
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
    /// use std::os::unix::net::{UnixDatagram, SocketAncillary};
    /// use std::io::IoSlice;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let sock = UnixDatagram::unbound()?;
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
    ///     sock.send_vectored_with_ancillary(bufs, &mut ancillary)
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

    /// 设置套接字的读超时（read timeout）。
    ///
    /// 如果所提供的值是 [`None`]，那么 [`recv`] 与 [`recv_from`] 调用将无限期阻塞。
    /// 如果向此方法传入了零值 [`Duration`]，则返回 [`Err`]。
    ///
    /// [`recv`]: UnixDatagram::recv
    /// [`recv_from`]: UnixDatagram::recv_from
    ///
    /// # 示例
    ///
    /// ```
    /// use std::os::unix::net::UnixDatagram;
    /// use std::time::Duration;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let sock = UnixDatagram::unbound()?;
    ///     sock.set_read_timeout(Some(Duration::new(1, 0)))
    ///         .expect("set_read_timeout function failed");
    ///     Ok(())
    /// }
    /// ```
    ///
    /// 如果向此方法传入了零值 [`Duration`]，则返回 [`Err`]：
    ///
    /// ```no_run
    /// use std::io;
    /// use std::os::unix::net::UnixDatagram;
    /// use std::time::Duration;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let socket = UnixDatagram::unbound()?;
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
    /// 如果所提供的值是 [`None`]，那么 [`send`] 与 [`send_to`] 调用将无限期阻塞。
    /// 如果向此方法传入了零值 [`Duration`]，则返回 [`Err`]。
    ///
    /// [`send`]: UnixDatagram::send
    /// [`send_to`]: UnixDatagram::send_to
    ///
    /// # 示例
    ///
    /// ```
    /// use std::os::unix::net::UnixDatagram;
    /// use std::time::Duration;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let sock = UnixDatagram::unbound()?;
    ///     sock.set_write_timeout(Some(Duration::new(1, 0)))
    ///         .expect("set_write_timeout function failed");
    ///     Ok(())
    /// }
    /// ```
    ///
    /// 如果向此方法传入了零值 [`Duration`]，则返回 [`Err`]：
    ///
    /// ```no_run
    /// use std::io;
    /// use std::os::unix::net::UnixDatagram;
    /// use std::time::Duration;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let socket = UnixDatagram::unbound()?;
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
    /// ```
    /// use std::os::unix::net::UnixDatagram;
    /// use std::time::Duration;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let sock = UnixDatagram::unbound()?;
    ///     sock.set_read_timeout(Some(Duration::new(1, 0)))
    ///         .expect("set_read_timeout function failed");
    ///     assert_eq!(sock.read_timeout()?, Some(Duration::new(1, 0)));
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
    /// ```
    /// use std::os::unix::net::UnixDatagram;
    /// use std::time::Duration;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let sock = UnixDatagram::unbound()?;
    ///     sock.set_write_timeout(Some(Duration::new(1, 0)))
    ///         .expect("set_write_timeout function failed");
    ///     assert_eq!(sock.write_timeout()?, Some(Duration::new(1, 0)));
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
    /// ```
    /// use std::os::unix::net::UnixDatagram;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let sock = UnixDatagram::unbound()?;
    ///     sock.set_nonblocking(true).expect("set_nonblocking function failed");
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
    /// use std::os::unix::net::UnixDatagram;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let sock = UnixDatagram::unbound()?;
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
    /// use std::os::unix::net::UnixDatagram;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let sock = UnixDatagram::unbound()?;
    ///     if let Ok(Some(err)) = sock.take_error() {
    ///         println!("Got error: {err:?}");
    ///     }
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "unix_socket", since = "1.10.0")]
    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        self.0.take_error()
    }

    /// 关闭此连接的读半边、写半边，或两者。
    ///
    /// 此函数将导致所指定部分上所有挂起的与未来的 I/O 调用立即返回一个适当的值
    ///（参见 [`Shutdown`] 的文档）。
    ///
    /// ```no_run
    /// use std::os::unix::net::UnixDatagram;
    /// use std::net::Shutdown;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let sock = UnixDatagram::unbound()?;
    ///     sock.shutdown(Shutdown::Both).expect("shutdown function failed");
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
    /// use std::os::unix::net::UnixDatagram;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let socket = UnixDatagram::bind("/tmp/sock")?;
    ///     let mut buf = [0; 10];
    ///     let len = socket.peek(&mut buf).expect("peek failed");
    ///     Ok(())
    /// }
    /// ```
    #[unstable(feature = "unix_socket_peek", issue = "76923")]
    pub fn peek(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.peek(buf)
    }

    /// 在套接字上接收单条数据报消息，但不将其从队列中移除。成功时，返回读取的字节数
    /// 以及来源（origin）。
    ///
    /// 调用时必须传入一个有效且大小足以容纳消息字节的字节数组 `buf`。如果某条消息太长、
    /// 无法装入所提供的缓冲区，超出的字节可能会被丢弃。
    ///
    /// 连续多次调用会返回相同的数据。这是通过把 `MSG_PEEK` 作为标志传给底层的 `recvfrom`
    /// 系统调用实现的。
    ///
    /// 不要用此函数来实现忙等待（busy waiting）；应改用 `libc::poll` 来同步一个或多个
    /// 套接字上的 IO 事件。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// #![feature(unix_socket_peek)]
    ///
    /// use std::os::unix::net::UnixDatagram;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let socket = UnixDatagram::bind("/tmp/sock")?;
    ///     let mut buf = [0; 10];
    ///     let (len, addr) = socket.peek_from(&mut buf).expect("peek failed");
    ///     Ok(())
    /// }
    /// ```
    #[unstable(feature = "unix_socket_peek", issue = "76923")]
    pub fn peek_from(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        self.recv_from_flags(buf, libc::MSG_PEEK)
    }
}

#[stable(feature = "unix_socket", since = "1.10.0")]
impl AsRawFd for UnixDatagram {
    #[inline]
    fn as_raw_fd(&self) -> RawFd {
        self.0.as_inner().as_raw_fd()
    }
}

#[stable(feature = "unix_socket", since = "1.10.0")]
impl FromRawFd for UnixDatagram {
    #[inline]
    unsafe fn from_raw_fd(fd: RawFd) -> UnixDatagram {
        UnixDatagram(Socket::from_inner(FromInner::from_inner(OwnedFd::from_raw_fd(fd))))
    }
}

#[stable(feature = "unix_socket", since = "1.10.0")]
impl IntoRawFd for UnixDatagram {
    #[inline]
    fn into_raw_fd(self) -> RawFd {
        self.0.into_inner().into_inner().into_raw_fd()
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl AsFd for UnixDatagram {
    #[inline]
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0.as_inner().as_fd()
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl From<UnixDatagram> for OwnedFd {
    /// 取得一个 [`UnixDatagram`] 的套接字文件描述符的所有权。
    #[inline]
    fn from(unix_datagram: UnixDatagram) -> OwnedFd {
        unsafe { OwnedFd::from_raw_fd(unix_datagram.into_raw_fd()) }
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl From<OwnedFd> for UnixDatagram {
    #[inline]
    fn from(owned: OwnedFd) -> Self {
        unsafe { Self::from_raw_fd(owned.into_raw_fd()) }
    }
}

impl AsInner<Socket> for UnixDatagram {
    #[inline]
    fn as_inner(&self) -> &Socket {
        &self.0
    }
}
