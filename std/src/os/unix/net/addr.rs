use crate::bstr::ByteStr;
use crate::ffi::OsStr;
#[cfg(any(doc, target_os = "android", target_os = "linux", target_os = "cygwin"))]
use crate::os::net::linux_ext;
use crate::os::unix::ffi::OsStrExt;
use crate::path::Path;
use crate::sealed::Sealed;
use crate::sys::cvt;
use crate::{fmt, io, mem, ptr};

// FIXME(#43348): 让 libc 适配 #[doc(cfg(...))]，这样我们就不需要在此放置这些伪定义？
#[cfg(not(unix))]
#[allow(non_camel_case_types)]
mod libc {
    pub use core::ffi::c_int;
    pub type socklen_t = u32;
    pub struct sockaddr;
    #[derive(Clone)]
    pub struct sockaddr_un {
        pub sun_path: [u8; 1],
    }
}

const SUN_PATH_OFFSET: usize = mem::offset_of!(libc::sockaddr_un, sun_path);

pub(super) fn sockaddr_un(path: &Path) -> io::Result<(libc::sockaddr_un, libc::socklen_t)> {
    // SAFETY: 全零是 `sockaddr_un` 的一个有效表示。
    let mut addr: libc::sockaddr_un = unsafe { mem::zeroed() };
    addr.sun_family = libc::AF_UNIX as libc::sa_family_t;

    let bytes = path.as_os_str().as_bytes();

    if bytes.contains(&0) {
        return Err(io::const_error!(
            io::ErrorKind::InvalidInput,
            "paths must not contain interior null bytes",
        ));
    }

    if bytes.len() >= addr.sun_path.len() {
        return Err(io::const_error!(
            io::ErrorKind::InvalidInput,
            "path must be shorter than SUN_LEN",
        ));
    }
    // SAFETY: `bytes` 与 `addr.sun_path` 不重叠，且二者都指向有效内存。
    // 注意：我们在上面已将内存清零，因此该路径已经以 null 结尾。
    unsafe {
        ptr::copy_nonoverlapping(bytes.as_ptr(), addr.sun_path.as_mut_ptr().cast(), bytes.len())
    };

    let mut len = SUN_PATH_OFFSET + bytes.len();
    match bytes.get(0) {
        Some(&0) | None => {}
        Some(_) => len += 1,
    }
    Ok((addr, len as libc::socklen_t))
}

enum AddressKind<'a> {
    Unnamed,
    Pathname(&'a Path),
    Abstract(&'a ByteStr),
}

/// 与某个 Unix 套接字关联的地址。
///
/// # 示例
///
/// ```
/// use std::os::unix::net::UnixListener;
///
/// let socket = match UnixListener::bind("/tmp/sock") {
///     Ok(sock) => sock,
///     Err(e) => {
///         println!("Couldn't bind: {e:?}");
///         return
///     }
/// };
/// let addr = socket.local_addr().expect("Couldn't get local address");
/// ```
#[derive(Clone)]
#[stable(feature = "unix_socket", since = "1.10.0")]
pub struct SocketAddr {
    pub(super) addr: libc::sockaddr_un,
    pub(super) len: libc::socklen_t,
}

impl SocketAddr {
    pub(super) fn new<F>(f: F) -> io::Result<SocketAddr>
    where
        F: FnOnce(*mut libc::sockaddr, *mut libc::socklen_t) -> libc::c_int,
    {
        unsafe {
            let mut addr: libc::sockaddr_un = mem::zeroed();
            let mut len = size_of::<libc::sockaddr_un>() as libc::socklen_t;
            cvt(f((&raw mut addr) as *mut _, &mut len))?;
            SocketAddr::from_parts(addr, len)
        }
    }

    pub(super) fn from_parts(
        addr: libc::sockaddr_un,
        mut len: libc::socklen_t,
    ) -> io::Result<SocketAddr> {
        if cfg!(target_os = "openbsd") {
            // 在 OpenBSD 上，getsockname(2) 返回的是套接字地址的实际大小，
            // 而非其内容的长度。我们自己来计算这个长度。
            // https://marc.info/?l=openbsd-bugs&m=170105481926736&w=2
            let sun_path: &[u8] =
                unsafe { mem::transmute::<&[libc::c_char], &[u8]>(&addr.sun_path) };
            len = core::slice::memchr::memchr(0, sun_path)
                .map_or(len, |new_len| (new_len + SUN_PATH_OFFSET) as libc::socklen_t);
        }

        if len == 0 {
            // 当存在一个来自未命名 unix 套接字的数据报时，
            // linux 返回零字节的地址
            len = SUN_PATH_OFFSET as libc::socklen_t; // 即长度为零的地址
        } else if addr.sun_family != libc::AF_UNIX as libc::sa_family_t {
            return Err(io::const_error!(
                io::ErrorKind::InvalidInput,
                "file descriptor did not correspond to a Unix socket",
            ));
        }

        Ok(SocketAddr { addr, len })
    }

    /// 用家族 `AF_UNIX` 与所提供的路径构造一个 `SockAddr`。
    ///
    /// # Errors
    ///
    /// 如果路径长度超过 `SUN_LEN`，或它包含 NULL 字节，则返回错误。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::os::unix::net::SocketAddr;
    /// use std::path::Path;
    ///
    /// # fn main() -> std::io::Result<()> {
    /// let address = SocketAddr::from_pathname("/path/to/socket")?;
    /// assert_eq!(address.as_pathname(), Some(Path::new("/path/to/socket")));
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// 用一个 NULL 字节创建 `SocketAddr` 会导致错误。
    ///
    /// ```
    /// use std::os::unix::net::SocketAddr;
    ///
    /// assert!(SocketAddr::from_pathname("/path/with/\0/bytes").is_err());
    /// ```
    #[stable(feature = "unix_socket_creation", since = "1.61.0")]
    pub fn from_pathname<P>(path: P) -> io::Result<SocketAddr>
    where
        P: AsRef<Path>,
    {
        sockaddr_un(path.as_ref()).map(|(addr, len)| SocketAddr { addr, len })
    }

    /// 如果该地址是未命名的（unnamed），则返回 `true`。
    ///
    /// # 示例
    ///
    /// 一个有名字的地址：
    ///
    /// ```no_run
    /// use std::os::unix::net::UnixListener;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let socket = UnixListener::bind("/tmp/sock")?;
    ///     let addr = socket.local_addr().expect("Couldn't get local address");
    ///     assert_eq!(addr.is_unnamed(), false);
    ///     Ok(())
    /// }
    /// ```
    ///
    /// 一个未命名的地址：
    ///
    /// ```
    /// use std::os::unix::net::UnixDatagram;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let socket = UnixDatagram::unbound()?;
    ///     let addr = socket.local_addr().expect("Couldn't get local address");
    ///     assert_eq!(addr.is_unnamed(), true);
    ///     Ok(())
    /// }
    /// ```
    #[must_use]
    #[stable(feature = "unix_socket", since = "1.10.0")]
    pub fn is_unnamed(&self) -> bool {
        matches!(self.address(), AddressKind::Unnamed)
    }

    /// 如果该地址是 `pathname`（路径名）地址，则返回其内容。
    ///
    /// # 示例
    ///
    /// 带有路径名时：
    ///
    /// ```no_run
    /// use std::os::unix::net::UnixListener;
    /// use std::path::Path;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let socket = UnixListener::bind("/tmp/sock")?;
    ///     let addr = socket.local_addr().expect("Couldn't get local address");
    ///     assert_eq!(addr.as_pathname(), Some(Path::new("/tmp/sock")));
    ///     Ok(())
    /// }
    /// ```
    ///
    /// 不带路径名时：
    ///
    /// ```
    /// use std::os::unix::net::UnixDatagram;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let socket = UnixDatagram::unbound()?;
    ///     let addr = socket.local_addr().expect("Couldn't get local address");
    ///     assert_eq!(addr.as_pathname(), None);
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "unix_socket", since = "1.10.0")]
    #[must_use]
    pub fn as_pathname(&self) -> Option<&Path> {
        if let AddressKind::Pathname(path) = self.address() { Some(path) } else { None }
    }

    fn address(&self) -> AddressKind<'_> {
        let len = self.len as usize - SUN_PATH_OFFSET;
        let path = unsafe { mem::transmute::<&[libc::c_char], &[u8]>(&self.addr.sun_path) };

        // 对于未命名地址，macOS 似乎会返回长度 16 与一段清零的 sun_path
        if len == 0
            || (cfg!(not(any(target_os = "linux", target_os = "android", target_os = "cygwin")))
                && self.addr.sun_path[0] == 0)
        {
            AddressKind::Unnamed
        } else if self.addr.sun_path[0] == 0 {
            AddressKind::Abstract(ByteStr::from_bytes(&path[1..len]))
        } else {
            AddressKind::Pathname(OsStr::from_bytes(&path[..len - 1]).as_ref())
        }
    }
}

#[stable(feature = "unix_socket_abstract", since = "1.70.0")]
impl Sealed for SocketAddr {}

#[doc(cfg(any(target_os = "android", target_os = "linux", target_os = "cygwin")))]
#[cfg(any(doc, target_os = "android", target_os = "linux", target_os = "cygwin"))]
#[stable(feature = "unix_socket_abstract", since = "1.70.0")]
impl linux_ext::addr::SocketAddrExt for SocketAddr {
    fn as_abstract_name(&self) -> Option<&[u8]> {
        if let AddressKind::Abstract(name) = self.address() { Some(name.as_bytes()) } else { None }
    }

    fn from_abstract_name<N>(name: N) -> io::Result<Self>
    where
        N: AsRef<[u8]>,
    {
        let name = name.as_ref();
        unsafe {
            let mut addr: libc::sockaddr_un = mem::zeroed();
            addr.sun_family = libc::AF_UNIX as libc::sa_family_t;

            if name.len() + 1 > addr.sun_path.len() {
                return Err(io::const_error!(
                    io::ErrorKind::InvalidInput,
                    "abstract socket name must be shorter than SUN_LEN",
                ));
            }

            crate::ptr::copy_nonoverlapping(
                name.as_ptr(),
                addr.sun_path.as_mut_ptr().add(1) as *mut u8,
                name.len(),
            );
            let len = (SUN_PATH_OFFSET + 1 + name.len()) as libc::socklen_t;
            SocketAddr::from_parts(addr, len)
        }
    }
}

#[stable(feature = "unix_socket", since = "1.10.0")]
impl fmt::Debug for SocketAddr {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.address() {
            AddressKind::Unnamed => write!(fmt, "(unnamed)"),
            AddressKind::Abstract(name) => write!(fmt, "{name:?} (abstract)"),
            AddressKind::Pathname(path) => write!(fmt, "{path:?} (pathname)"),
        }
    }
}
