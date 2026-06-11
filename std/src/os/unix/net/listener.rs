use super::{SocketAddr, UnixStream, sockaddr_un};
use crate::os::unix::io::{AsFd, AsRawFd, BorrowedFd, FromRawFd, IntoRawFd, OwnedFd, RawFd};
use crate::path::Path;
use crate::sys::net::Socket;
use crate::sys::{AsInner, FromInner, IntoInner, cvt};
use crate::{fmt, io, mem};

/// 表示一个 Unix 域套接字（domain socket）服务器的结构体。
///
/// # 示例
///
/// ```no_run
/// use std::thread;
/// use std::os::unix::net::{UnixStream, UnixListener};
///
/// fn handle_client(stream: UnixStream) {
///     // ...
/// }
///
/// fn main() -> std::io::Result<()> {
///     let listener = UnixListener::bind("/path/to/the/socket")?;
///
///     // 接受连接并处理它们，为每个连接派生一个新线程
///     for stream in listener.incoming() {
///         match stream {
///             Ok(stream) => {
///                 /* 连接成功 */
///                 thread::spawn(|| handle_client(stream));
///             }
///             Err(err) => {
///                 /* 连接失败 */
///                 break;
///             }
///         }
///     }
///     Ok(())
/// }
/// ```
#[stable(feature = "unix_socket", since = "1.10.0")]
pub struct UnixListener(Socket);

#[stable(feature = "unix_socket", since = "1.10.0")]
impl fmt::Debug for UnixListener {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut builder = fmt.debug_struct("UnixListener");
        builder.field("fd", self.0.as_inner());
        if let Ok(addr) = self.local_addr() {
            builder.field("local", &addr);
        }
        builder.finish()
    }
}

impl UnixListener {
    /// 创建一个绑定到指定套接字的新 `UnixListener`。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::os::unix::net::UnixListener;
    ///
    /// let listener = match UnixListener::bind("/path/to/the/socket") {
    ///     Ok(sock) => sock,
    ///     Err(e) => {
    ///         println!("Couldn't connect: {e:?}");
    ///         return
    ///     }
    /// };
    /// ```
    #[stable(feature = "unix_socket", since = "1.10.0")]
    pub fn bind<P: AsRef<Path>>(path: P) -> io::Result<UnixListener> {
        unsafe {
            let inner = Socket::new(libc::AF_UNIX, libc::SOCK_STREAM)?;
            let (addr, len) = sockaddr_un(path.as_ref())?;
            #[cfg(any(
                target_os = "windows",
                target_os = "redox",
                target_os = "espidf",
                target_os = "horizon"
            ))]
            const backlog: core::ffi::c_int = 128;
            #[cfg(any(
                // 被静默地限制到 `/proc/sys/net/core/somaxconn`。
                target_os = "linux",
                // 被静默地限制到 `kern.ipc.soacceptqueue`。
                target_os = "freebsd",
                // 被静默地限制到 `kern.somaxconn sysctl`。
                target_os = "openbsd",
                // 被静默地限制到默认值 128。
                target_vendor = "apple",
            ))]
            const backlog: core::ffi::c_int = -1;
            #[cfg(not(any(
                target_os = "windows",
                target_os = "redox",
                target_os = "espidf",
                target_os = "horizon",
                target_os = "linux",
                target_os = "freebsd",
                target_os = "openbsd",
                target_vendor = "apple",
            )))]
            const backlog: libc::c_int = libc::SOMAXCONN;

            cvt(libc::bind(inner.as_inner().as_raw_fd(), (&raw const addr) as *const _, len as _))?;
            cvt(libc::listen(inner.as_inner().as_raw_fd(), backlog))?;

            Ok(UnixListener(inner))
        }
    }

    /// 创建一个绑定到指定[`套接字地址`]的新 `UnixListener`。
    ///
    /// [`套接字地址`]: crate::os::unix::net::SocketAddr
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::os::unix::net::{UnixListener};
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let listener1 = UnixListener::bind("path/to/socket")?;
    ///     let addr = listener1.local_addr()?;
    ///
    ///     let listener2 = match UnixListener::bind_addr(&addr) {
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
    pub fn bind_addr(socket_addr: &SocketAddr) -> io::Result<UnixListener> {
        unsafe {
            let inner = Socket::new(libc::AF_UNIX, libc::SOCK_STREAM)?;
            #[cfg(target_os = "linux")]
            const backlog: core::ffi::c_int = -1;
            #[cfg(not(target_os = "linux"))]
            const backlog: core::ffi::c_int = 128;
            cvt(libc::bind(
                inner.as_raw_fd(),
                (&raw const socket_addr.addr) as *const _,
                socket_addr.len as _,
            ))?;
            cvt(libc::listen(inner.as_raw_fd(), backlog))?;
            Ok(UnixListener(inner))
        }
    }

    /// 接受一个到此监听器的新入站连接。
    ///
    /// 此函数会阻塞调用线程，直到一个新的 Unix 连接建立。建立之后，将返回相应的
    /// [`UnixStream`] 以及远程对端的地址。
    ///
    /// [`UnixStream`]: crate::os::unix::net::UnixStream
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::os::unix::net::UnixListener;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let listener = UnixListener::bind("/path/to/the/socket")?;
    ///
    ///     match listener.accept() {
    ///         Ok((socket, addr)) => println!("Got a client: {addr:?}"),
    ///         Err(e) => println!("accept function failed: {e:?}"),
    ///     }
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "unix_socket", since = "1.10.0")]
    pub fn accept(&self) -> io::Result<(UnixStream, SocketAddr)> {
        let mut storage: libc::sockaddr_un = unsafe { mem::zeroed() };
        let mut len = size_of_val(&storage) as libc::socklen_t;
        let sock = self.0.accept((&raw mut storage) as *mut _, &mut len)?;
        let addr = SocketAddr::from_parts(storage, len)?;
        Ok((UnixStream(sock), addr))
    }

    /// 为底层套接字创建一个新的、独立拥有所有权的句柄。
    ///
    /// 返回的 `UnixListener` 引用的是与此对象所引用的同一个套接字。两个句柄都可用于接受
    /// 入站连接，且在一个监听器上设置的选项将影响另一个。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::os::unix::net::UnixListener;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let listener = UnixListener::bind("/path/to/the/socket")?;
    ///     let listener_copy = listener.try_clone().expect("try_clone failed");
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "unix_socket", since = "1.10.0")]
    pub fn try_clone(&self) -> io::Result<UnixListener> {
        self.0.duplicate().map(UnixListener)
    }

    /// 返回此监听器的本地套接字地址。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::os::unix::net::UnixListener;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let listener = UnixListener::bind("/path/to/the/socket")?;
    ///     let addr = listener.local_addr().expect("Couldn't get local address");
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "unix_socket", since = "1.10.0")]
    pub fn local_addr(&self) -> io::Result<SocketAddr> {
        SocketAddr::new(|addr, len| unsafe { libc::getsockname(self.as_raw_fd(), addr, len) })
    }

    /// 将套接字切换进入或退出非阻塞（nonblocking）模式。
    ///
    /// 这会使 `accept` 操作变为非阻塞，即从其调用中立即返回。如果该 IO 操作成功，
    /// 返回 `Ok` 且无需进一步操作。如果该 IO 操作无法完成、需要重试，则返回一个类型为
    /// [`io::ErrorKind::WouldBlock`] 的错误。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::os::unix::net::UnixListener;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let listener = UnixListener::bind("/path/to/the/socket")?;
    ///     listener.set_nonblocking(true).expect("Couldn't set non blocking");
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "unix_socket", since = "1.10.0")]
    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        self.0.set_nonblocking(nonblocking)
    }

    /// 返回 `SO_ERROR` 选项的值。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::os::unix::net::UnixListener;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let listener = UnixListener::bind("/tmp/sock")?;
    ///
    ///     if let Ok(Some(err)) = listener.take_error() {
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

    /// 返回一个遍历入站连接的迭代器。
    ///
    /// 该迭代器永远不会返回 [`None`]，也不会产出对端的 [`SocketAddr`] 结构体。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::thread;
    /// use std::os::unix::net::{UnixStream, UnixListener};
    ///
    /// fn handle_client(stream: UnixStream) {
    ///     // ...
    /// }
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let listener = UnixListener::bind("/path/to/the/socket")?;
    ///
    ///     for stream in listener.incoming() {
    ///         match stream {
    ///             Ok(stream) => {
    ///                 thread::spawn(|| handle_client(stream));
    ///             }
    ///             Err(err) => {
    ///                 break;
    ///             }
    ///         }
    ///     }
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "unix_socket", since = "1.10.0")]
    pub fn incoming(&self) -> Incoming<'_> {
        Incoming { listener: self }
    }
}

#[stable(feature = "unix_socket", since = "1.10.0")]
impl AsRawFd for UnixListener {
    #[inline]
    fn as_raw_fd(&self) -> RawFd {
        self.0.as_inner().as_raw_fd()
    }
}

#[stable(feature = "unix_socket", since = "1.10.0")]
impl FromRawFd for UnixListener {
    #[inline]
    unsafe fn from_raw_fd(fd: RawFd) -> UnixListener {
        UnixListener(Socket::from_inner(FromInner::from_inner(OwnedFd::from_raw_fd(fd))))
    }
}

#[stable(feature = "unix_socket", since = "1.10.0")]
impl IntoRawFd for UnixListener {
    #[inline]
    fn into_raw_fd(self) -> RawFd {
        self.0.into_inner().into_inner().into_raw_fd()
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl AsFd for UnixListener {
    #[inline]
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0.as_inner().as_fd()
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl From<OwnedFd> for UnixListener {
    #[inline]
    fn from(fd: OwnedFd) -> UnixListener {
        UnixListener(Socket::from_inner(FromInner::from_inner(fd)))
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl From<UnixListener> for OwnedFd {
    /// 取得一个 [`UnixListener`] 的套接字文件描述符的所有权。
    #[inline]
    fn from(listener: UnixListener) -> OwnedFd {
        listener.0.into_inner().into_inner()
    }
}

#[stable(feature = "unix_socket", since = "1.10.0")]
impl<'a> IntoIterator for &'a UnixListener {
    type Item = io::Result<UnixStream>;
    type IntoIter = Incoming<'a>;

    fn into_iter(self) -> Incoming<'a> {
        self.incoming()
    }
}

/// 一个遍历到某个 [`UnixListener`] 的入站连接的迭代器。
///
/// 它永远不会返回 [`None`]。
///
/// # 示例
///
/// ```no_run
/// use std::thread;
/// use std::os::unix::net::{UnixStream, UnixListener};
///
/// fn handle_client(stream: UnixStream) {
///     // ...
/// }
///
/// fn main() -> std::io::Result<()> {
///     let listener = UnixListener::bind("/path/to/the/socket")?;
///
///     for stream in listener.incoming() {
///         match stream {
///             Ok(stream) => {
///                 thread::spawn(|| handle_client(stream));
///             }
///             Err(err) => {
///                 break;
///             }
///         }
///     }
///     Ok(())
/// }
/// ```
#[derive(Debug)]
#[must_use = "iterators are lazy and do nothing unless consumed"]
#[stable(feature = "unix_socket", since = "1.10.0")]
pub struct Incoming<'a> {
    listener: &'a UnixListener,
}

#[stable(feature = "unix_socket", since = "1.10.0")]
impl<'a> Iterator for Incoming<'a> {
    type Item = io::Result<UnixStream>;

    fn next(&mut self) -> Option<io::Result<UnixStream>> {
        Some(self.listener.accept().map(|s| s.0))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (usize::MAX, None)
    }
}
