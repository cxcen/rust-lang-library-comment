use libc::{MSG_PEEK, c_int, c_void, size_t, sockaddr, socklen_t};

#[cfg(not(any(target_os = "espidf", target_os = "nuttx")))]
use crate::ffi::CStr;
use crate::io::{self, BorrowedBuf, BorrowedCursor, IoSlice, IoSliceMut};
use crate::net::{Shutdown, SocketAddr};
use crate::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, IntoRawFd, RawFd};
use crate::sys::fd::FileDesc;
use crate::sys::net::{getsockopt, setsockopt};
use crate::sys::pal::IsMinusOne;
use crate::sys::{AsInner, FromInner, IntoInner};
use crate::time::{Duration, Instant};
use crate::{cmp, mem};

cfg_select! {
    target_vendor = "apple" => {
        use libc::SO_LINGER_SEC as SO_LINGER;
    }
    _ => {
        use libc::SO_LINGER;
    }
}

pub(super) use libc as netc;

use super::{socket_addr_from_c, socket_addr_to_c};
pub use crate::sys::{cvt, cvt_r};

#[expect(non_camel_case_types)]
pub type wrlen_t = size_t;

pub struct Socket(FileDesc);

pub fn init() {}

pub fn cvt_gai(err: c_int) -> io::Result<()> {
    if err == 0 {
        return Ok(());
    }

    // 我们可能需要触发一个针对 glibc 的变通处理。详见 on_resolver_failure()。
    on_resolver_failure();

    #[cfg(not(any(target_os = "espidf", target_os = "nuttx")))]
    if err == libc::EAI_SYSTEM {
        return Err(io::Error::last_os_error());
    }

    #[cfg(not(any(target_os = "espidf", target_os = "nuttx")))]
    let detail = unsafe {
        // 我们不能总是假定身处 UTF-8 的环境。当没有这份运气时，
        // 给出一条低质量的错误消息也好过完全没有消息。
        CStr::from_ptr(libc::gai_strerror(err)).to_string_lossy()
    };

    #[cfg(any(target_os = "espidf", target_os = "nuttx"))]
    let detail = "";

    Err(io::Error::new(
        io::ErrorKind::Uncategorized,
        &format!("failed to lookup address information: {detail}")[..],
    ))
}

impl Socket {
    pub fn new(family: c_int, ty: c_int) -> io::Result<Socket> {
        cfg_select! {
            any(
                target_os = "android",
                target_os = "dragonfly",
                target_os = "freebsd",
                target_os = "illumos",
                target_os = "hurd",
                target_os = "linux",
                target_os = "netbsd",
                target_os = "openbsd",
                target_os = "cygwin",
                target_os = "nto",
                target_os = "solaris",
            ) => {
                // 在支持的平台上，我们传入 SOCK_CLOEXEC 标志，
                // 以原子地创建 socket 并将其设置为 CLOEXEC。
                // 在 Linux 上，这是在 2.6.27 中加入的。
                let fd = cvt(unsafe { libc::socket(family, ty | libc::SOCK_CLOEXEC, 0) })?;
                let socket = Socket(unsafe { FileDesc::from_raw_fd(fd) });

                // DragonFlyBSD、FreeBSD 和 NetBSD 使用 `SO_NOSIGPIPE` 作为
                // `setsockopt` 标志，以禁止 socket 发出 `SIGPIPE`。
                #[cfg(any(target_os = "freebsd", target_os = "netbsd", target_os = "dragonfly"))]
                unsafe { setsockopt(&socket, libc::SOL_SOCKET, libc::SO_NOSIGPIPE, 1)? };

                Ok(socket)
            }
            _ => {
                let fd = cvt(unsafe { libc::socket(family, ty, 0) })?;
                let fd = unsafe { FileDesc::from_raw_fd(fd) };
                fd.set_cloexec()?;
                let socket = Socket(fd);

                // macOS 和 iOS 使用 `SO_NOSIGPIPE` 作为 `setsockopt` 标志，
                // 以禁止 socket 发出 `SIGPIPE`。
                #[cfg(target_vendor = "apple")]
                unsafe { setsockopt(&socket, libc::SOL_SOCKET, libc::SO_NOSIGPIPE, 1)? };

                Ok(socket)
            }
        }
    }

    #[cfg(not(any(target_os = "vxworks", target_os = "wasi")))]
    pub fn new_pair(fam: c_int, ty: c_int) -> io::Result<(Socket, Socket)> {
        unsafe {
            let mut fds = [0, 0];

            cfg_select! {
                any(
                    target_os = "android",
                    target_os = "dragonfly",
                    target_os = "freebsd",
                    target_os = "illumos",
                    target_os = "linux",
                    target_os = "hurd",
                    target_os = "netbsd",
                    target_os = "openbsd",
                    target_os = "cygwin",
                    target_os = "nto",
                ) => {
                    // 与上面一样，原子地设置 cloexec
                    cvt(libc::socketpair(fam, ty | libc::SOCK_CLOEXEC, 0, fds.as_mut_ptr()))?;
                    Ok((Socket(FileDesc::from_raw_fd(fds[0])), Socket(FileDesc::from_raw_fd(fds[1]))))
                }
                _ => {
                    cvt(libc::socketpair(fam, ty, 0, fds.as_mut_ptr()))?;
                    let a = FileDesc::from_raw_fd(fds[0]);
                    let b = FileDesc::from_raw_fd(fds[1]);
                    a.set_cloexec()?;
                    b.set_cloexec()?;
                    Ok((Socket(a), Socket(b)))
                }
            }
        }
    }

    #[cfg(target_os = "vxworks")]
    pub fn new_pair(_fam: c_int, _ty: c_int) -> io::Result<(Socket, Socket)> {
        unimplemented!()
    }

    pub fn connect(&self, addr: &SocketAddr) -> io::Result<()> {
        let (addr, len) = socket_addr_to_c(addr);
        loop {
            let result = unsafe { libc::connect(self.as_raw_fd(), addr.as_ptr(), len) };
            if result.is_minus_one() {
                let err = crate::sys::io::errno();
                match err {
                    libc::EINTR => continue,
                    libc::EISCONN => return Ok(()),
                    _ => return Err(io::Error::from_raw_os_error(err)),
                }
            }
            return Ok(());
        }
    }

    pub fn connect_timeout(&self, addr: &SocketAddr, timeout: Duration) -> io::Result<()> {
        self.set_nonblocking(true)?;
        let r = unsafe {
            let (addr, len) = socket_addr_to_c(addr);
            cvt(libc::connect(self.as_raw_fd(), addr.as_ptr(), len))
        };
        self.set_nonblocking(false)?;

        match r {
            Ok(_) => return Ok(()),
            // EINPROGRESS 没有对应的 ErrorKind :(
            Err(ref e) if e.raw_os_error() == Some(libc::EINPROGRESS) => {}
            Err(e) => return Err(e),
        }

        let mut pollfd = libc::pollfd { fd: self.as_raw_fd(), events: libc::POLLOUT, revents: 0 };

        if timeout.as_secs() == 0 && timeout.subsec_nanos() == 0 {
            return Err(io::Error::ZERO_TIMEOUT);
        }

        let start = Instant::now();

        loop {
            let elapsed = start.elapsed();
            if elapsed >= timeout {
                return Err(io::const_error!(io::ErrorKind::TimedOut, "connection timed out"));
            }

            let timeout = timeout - elapsed;
            let mut timeout = timeout
                .as_secs()
                .saturating_mul(1_000)
                .saturating_add(timeout.subsec_nanos() as u64 / 1_000_000);
            if timeout == 0 {
                timeout = 1;
            }

            let timeout = cmp::min(timeout, c_int::MAX as u64) as c_int;

            match unsafe { libc::poll(&mut pollfd, 1, timeout) } {
                -1 => {
                    let err = io::Error::last_os_error();
                    if !err.is_interrupted() {
                        return Err(err);
                    }
                }
                0 => {}
                _ => {
                    if cfg!(target_os = "vxworks") {
                        // VxWorks 的 poll 不会在 revents 中返回 POLLHUP 或 POLLERR。
                        // 检查连接是否确实成功，仅当 socket 就绪且未发现错误时才
                        // 返回 ok。
                        if let Some(e) = self.take_error()? {
                            return Err(e);
                        }
                    } else {
                        // linux 对于被拒绝的连接会返回 POLLOUT|POLLERR|POLLHUP（!），
                        // 因此应查找 POLLHUP 或 POLLERR，而不是查看读就绪状态。
                        if pollfd.revents & (libc::POLLHUP | libc::POLLERR) != 0 {
                            let e = self.take_error()?.unwrap_or_else(|| {
                                io::const_error!(
                                    io::ErrorKind::Uncategorized,
                                    "no error set after POLLHUP",
                                )
                            });
                            return Err(e);
                        }
                    }

                    return Ok(());
                }
            }
        }
    }

    pub fn accept(&self, storage: *mut sockaddr, len: *mut socklen_t) -> io::Result<Socket> {
        // 目前不幸的是，要想原子地 accept 一个 socket 并设置 CLOEXEC 标志，
        // 已知的唯一办法是在支持的平台上使用 `accept4` 系统调用。
        // 在 Linux 上，这是在 2.6.28、glibc 2.10 和 musl 0.9.5 中加入的。
        cfg_select! {
            any(
                target_os = "android",
                target_os = "dragonfly",
                target_os = "freebsd",
                target_os = "illumos",
                target_os = "linux",
                target_os = "hurd",
                target_os = "netbsd",
                target_os = "openbsd",
                target_os = "cygwin",
            ) => {
                unsafe {
                    let fd = cvt_r(|| libc::accept4(self.as_raw_fd(), storage, len, libc::SOCK_CLOEXEC))?;
                    Ok(Socket(FileDesc::from_raw_fd(fd)))
                }
            }
            _ => {
                unsafe {
                    let fd = cvt_r(|| libc::accept(self.as_raw_fd(), storage, len))?;
                    let fd = FileDesc::from_raw_fd(fd);
                    fd.set_cloexec()?;
                    Ok(Socket(fd))
                }
            }
        }
    }

    pub fn duplicate(&self) -> io::Result<Socket> {
        self.0.duplicate().map(Socket)
    }

    #[cfg(not(target_os = "wasi"))]
    pub fn send_with_flags(&self, buf: &[u8], flags: c_int) -> io::Result<usize> {
        let len = cmp::min(buf.len(), <wrlen_t>::MAX as usize) as wrlen_t;
        let ret = cvt(unsafe {
            libc::send(self.as_raw_fd(), buf.as_ptr() as *const c_void, len, flags)
        })?;
        Ok(ret as usize)
    }

    fn recv_with_flags(&self, mut buf: BorrowedCursor<'_>, flags: c_int) -> io::Result<()> {
        let ret = cvt(unsafe {
            libc::recv(
                self.as_raw_fd(),
                buf.as_mut().as_mut_ptr() as *mut c_void,
                buf.capacity(),
                flags,
            )
        })?;
        unsafe {
            buf.advance_unchecked(ret as usize);
        }
        Ok(())
    }

    pub fn read(&self, buf: &mut [u8]) -> io::Result<usize> {
        let mut buf = BorrowedBuf::from(buf);
        self.recv_with_flags(buf.unfilled(), 0)?;
        Ok(buf.len())
    }

    pub fn peek(&self, buf: &mut [u8]) -> io::Result<usize> {
        let mut buf = BorrowedBuf::from(buf);
        self.recv_with_flags(buf.unfilled(), MSG_PEEK)?;
        Ok(buf.len())
    }

    pub fn read_buf(&self, buf: BorrowedCursor<'_>) -> io::Result<()> {
        self.recv_with_flags(buf, 0)
    }

    pub fn read_vectored(&self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        self.0.read_vectored(bufs)
    }

    #[inline]
    pub fn is_read_vectored(&self) -> bool {
        self.0.is_read_vectored()
    }

    fn recv_from_with_flags(
        &self,
        buf: &mut [u8],
        flags: c_int,
    ) -> io::Result<(usize, SocketAddr)> {
        // `recvfrom` 函数会用地址填充 storage，
        // 因此我们在这里不需要将其清零。
        // 参考：https://linux.die.net/man/2/recvfrom
        let mut storage: mem::MaybeUninit<libc::sockaddr_storage> = mem::MaybeUninit::uninit();
        let mut addrlen = size_of_val(&storage) as libc::socklen_t;

        let n = cvt(unsafe {
            libc::recvfrom(
                self.as_raw_fd(),
                buf.as_mut_ptr() as *mut c_void,
                buf.len(),
                flags,
                (&raw mut storage) as *mut _,
                &mut addrlen,
            )
        })?;
        Ok((n as usize, unsafe { socket_addr_from_c(storage.as_ptr(), addrlen as usize)? }))
    }

    pub fn recv_from(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        self.recv_from_with_flags(buf, 0)
    }

    #[cfg(any(target_os = "android", target_os = "linux", target_os = "cygwin"))]
    pub fn recv_msg(&self, msg: &mut libc::msghdr) -> io::Result<usize> {
        let n = cvt(unsafe { libc::recvmsg(self.as_raw_fd(), msg, libc::MSG_CMSG_CLOEXEC) })?;
        Ok(n as usize)
    }

    pub fn peek_from(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        self.recv_from_with_flags(buf, MSG_PEEK)
    }

    #[cfg(not(target_os = "wasi"))]
    pub fn write(&self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }

    pub fn write_vectored(&self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        self.0.write_vectored(bufs)
    }

    #[inline]
    pub fn is_write_vectored(&self) -> bool {
        self.0.is_write_vectored()
    }

    #[cfg(any(target_os = "android", target_os = "linux", target_os = "cygwin"))]
    pub fn send_msg(&self, msg: &mut libc::msghdr) -> io::Result<usize> {
        let n = cvt(unsafe { libc::sendmsg(self.as_raw_fd(), msg, 0) })?;
        Ok(n as usize)
    }

    pub fn set_timeout(&self, dur: Option<Duration>, kind: libc::c_int) -> io::Result<()> {
        let timeout = match dur {
            Some(dur) => {
                if dur.as_secs() == 0 && dur.subsec_nanos() == 0 {
                    return Err(io::Error::ZERO_TIMEOUT);
                }

                let secs = if dur.as_secs() > libc::time_t::MAX as u64 {
                    libc::time_t::MAX
                } else {
                    dur.as_secs() as libc::time_t
                };
                let mut timeout = libc::timeval {
                    tv_sec: secs,
                    tv_usec: dur.subsec_micros() as libc::suseconds_t,
                };
                if timeout.tv_sec == 0 && timeout.tv_usec == 0 {
                    timeout.tv_usec = 1;
                }
                timeout
            }
            None => libc::timeval { tv_sec: 0, tv_usec: 0 },
        };
        unsafe { setsockopt(self, libc::SOL_SOCKET, kind, timeout) }
    }

    pub fn timeout(&self, kind: libc::c_int) -> io::Result<Option<Duration>> {
        let raw: libc::timeval = unsafe { getsockopt(self, libc::SOL_SOCKET, kind)? };
        if raw.tv_sec == 0 && raw.tv_usec == 0 {
            Ok(None)
        } else {
            let sec = raw.tv_sec as u64;
            let nsec = (raw.tv_usec as u32) * 1000;
            Ok(Some(Duration::new(sec, nsec)))
        }
    }

    pub fn shutdown(&self, how: Shutdown) -> io::Result<()> {
        let how = match how {
            Shutdown::Write => libc::SHUT_WR,
            Shutdown::Read => libc::SHUT_RD,
            Shutdown::Both => libc::SHUT_RDWR,
        };
        cvt(unsafe { libc::shutdown(self.as_raw_fd(), how) })?;
        Ok(())
    }

    #[cfg(not(target_os = "cygwin"))]
    pub fn set_linger(&self, linger: Option<Duration>) -> io::Result<()> {
        let linger = libc::linger {
            l_onoff: linger.is_some() as libc::c_int,
            l_linger: linger.unwrap_or_default().as_secs() as libc::c_int,
        };

        unsafe { setsockopt(self, libc::SOL_SOCKET, SO_LINGER, linger) }
    }

    #[cfg(target_os = "cygwin")]
    pub fn set_linger(&self, linger: Option<Duration>) -> io::Result<()> {
        let linger = libc::linger {
            l_onoff: linger.is_some() as libc::c_ushort,
            l_linger: linger.unwrap_or_default().as_secs() as libc::c_ushort,
        };

        unsafe { setsockopt(self, libc::SOL_SOCKET, SO_LINGER, linger) }
    }

    pub fn linger(&self) -> io::Result<Option<Duration>> {
        let val: libc::linger = unsafe { getsockopt(self, libc::SOL_SOCKET, SO_LINGER)? };

        Ok((val.l_onoff != 0).then(|| Duration::from_secs(val.l_linger as u64)))
    }

    pub fn set_nodelay(&self, nodelay: bool) -> io::Result<()> {
        unsafe { setsockopt(self, libc::IPPROTO_TCP, libc::TCP_NODELAY, nodelay as c_int) }
    }

    pub fn nodelay(&self) -> io::Result<bool> {
        let raw: c_int = unsafe { getsockopt(self, libc::IPPROTO_TCP, libc::TCP_NODELAY)? };
        Ok(raw != 0)
    }

    #[cfg(any(target_os = "android", target_os = "linux", target_os = "cygwin"))]
    pub fn set_quickack(&self, quickack: bool) -> io::Result<()> {
        unsafe { setsockopt(self, libc::IPPROTO_TCP, libc::TCP_QUICKACK, quickack as c_int) }
    }

    #[cfg(any(target_os = "android", target_os = "linux", target_os = "cygwin"))]
    pub fn quickack(&self) -> io::Result<bool> {
        let raw: c_int = unsafe { getsockopt(self, libc::IPPROTO_TCP, libc::TCP_QUICKACK)? };
        Ok(raw != 0)
    }

    // bionic libc 不使用此标志
    #[cfg(target_os = "linux")]
    pub fn set_deferaccept(&self, accept: Duration) -> io::Result<()> {
        let val = cmp::min(accept.as_secs(), c_int::MAX as u64) as c_int;
        unsafe { setsockopt(self, libc::IPPROTO_TCP, libc::TCP_DEFER_ACCEPT, val) }
    }

    #[cfg(target_os = "linux")]
    pub fn deferaccept(&self) -> io::Result<Duration> {
        let raw: c_int = unsafe { getsockopt(self, libc::IPPROTO_TCP, libc::TCP_DEFER_ACCEPT)? };
        Ok(Duration::from_secs(raw as _))
    }

    #[cfg(any(target_os = "freebsd", target_os = "netbsd"))]
    pub fn set_acceptfilter(&self, name: &CStr) -> io::Result<()> {
        if !name.to_bytes().is_empty() {
            const AF_NAME_MAX: usize = 16;
            let mut buf = [0; AF_NAME_MAX];
            for (src, dst) in name.to_bytes().iter().zip(&mut buf[..AF_NAME_MAX - 1]) {
                *dst = *src as libc::c_char;
            }
            let mut arg: libc::accept_filter_arg = unsafe { mem::zeroed() };
            arg.af_name = buf;
            unsafe { setsockopt(self, libc::SOL_SOCKET, libc::SO_ACCEPTFILTER, &mut arg) }
        } else {
            unsafe {
                setsockopt(
                    self,
                    libc::SOL_SOCKET,
                    libc::SO_ACCEPTFILTER,
                    core::ptr::null_mut() as *mut c_void,
                )
            }
        }
    }

    #[cfg(any(target_os = "freebsd", target_os = "netbsd"))]
    pub fn acceptfilter(&self) -> io::Result<&CStr> {
        let arg: libc::accept_filter_arg =
            unsafe { getsockopt(self, libc::SOL_SOCKET, libc::SO_ACCEPTFILTER)? };
        let s: &[u8] =
            unsafe { core::slice::from_raw_parts(arg.af_name.as_ptr() as *const u8, 16) };
        let name = CStr::from_bytes_with_nul(s).unwrap();
        Ok(name)
    }

    #[cfg(any(target_os = "solaris", target_os = "illumos"))]
    pub fn set_exclbind(&self, excl: bool) -> io::Result<()> {
        // libc crate 中尚无此项
        const SO_EXCLBIND: i32 = 0x1015;
        unsafe { setsockopt(self, libc::SOL_SOCKET, SO_EXCLBIND, excl) }
    }

    #[cfg(any(target_os = "solaris", target_os = "illumos"))]
    pub fn exclbind(&self) -> io::Result<bool> {
        // libc crate 中尚无此项
        const SO_EXCLBIND: i32 = 0x1015;
        let raw: c_int = unsafe { getsockopt(self, libc::SOL_SOCKET, SO_EXCLBIND)? };
        Ok(raw != 0)
    }

    #[cfg(any(target_os = "android", target_os = "linux", target_os = "cygwin"))]
    pub fn set_passcred(&self, passcred: bool) -> io::Result<()> {
        unsafe { setsockopt(self, libc::SOL_SOCKET, libc::SO_PASSCRED, passcred as libc::c_int) }
    }

    #[cfg(any(target_os = "android", target_os = "linux", target_os = "cygwin"))]
    pub fn passcred(&self) -> io::Result<bool> {
        let passcred: libc::c_int =
            unsafe { getsockopt(self, libc::SOL_SOCKET, libc::SO_PASSCRED)? };
        Ok(passcred != 0)
    }

    #[cfg(target_os = "netbsd")]
    pub fn set_local_creds(&self, local_creds: bool) -> io::Result<()> {
        unsafe { setsockopt(self, 0 as libc::c_int, libc::LOCAL_CREDS, local_creds as libc::c_int) }
    }

    #[cfg(target_os = "netbsd")]
    pub fn local_creds(&self) -> io::Result<bool> {
        let local_creds: libc::c_int =
            unsafe { getsockopt(self, 0 as libc::c_int, libc::LOCAL_CREDS)? };
        Ok(local_creds != 0)
    }

    #[cfg(target_os = "freebsd")]
    pub fn set_local_creds_persistent(&self, local_creds_persistent: bool) -> io::Result<()> {
        unsafe {
            setsockopt(
                self,
                libc::AF_LOCAL,
                libc::LOCAL_CREDS_PERSISTENT,
                local_creds_persistent as libc::c_int,
            )
        }
    }

    #[cfg(target_os = "freebsd")]
    pub fn local_creds_persistent(&self) -> io::Result<bool> {
        let local_creds_persistent: libc::c_int =
            unsafe { getsockopt(self, libc::AF_LOCAL, libc::LOCAL_CREDS_PERSISTENT)? };
        Ok(local_creds_persistent != 0)
    }

    #[cfg(not(any(target_os = "solaris", target_os = "illumos", target_os = "vita")))]
    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        let mut nonblocking = nonblocking as libc::c_int;
        cvt(unsafe { libc::ioctl(self.as_raw_fd(), libc::FIONBIO, &mut nonblocking) }).map(drop)
    }

    #[cfg(target_os = "vita")]
    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        let option = nonblocking as libc::c_int;
        unsafe { setsockopt(self, libc::SOL_SOCKET, libc::SO_NONBLOCK, option) }
    }

    #[cfg(any(target_os = "solaris", target_os = "illumos"))]
    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        // 在 illumos/Solaris 上 FIONBIO 对于 socket 而言并不适用，
        // 因此改用 FileDesc 提供的基于 fcntl(F_[GS]ETFL) 的方法。
        self.0.set_nonblocking(nonblocking)
    }

    #[cfg(any(target_os = "linux", target_os = "freebsd", target_os = "openbsd"))]
    pub fn set_mark(&self, mark: u32) -> io::Result<()> {
        #[cfg(target_os = "linux")]
        let option = libc::SO_MARK;
        #[cfg(target_os = "freebsd")]
        let option = libc::SO_USER_COOKIE;
        #[cfg(target_os = "openbsd")]
        let option = libc::SO_RTABLE;
        unsafe { setsockopt(self, libc::SOL_SOCKET, option, mark as libc::c_int) }
    }

    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        let raw: c_int = unsafe { getsockopt(self, libc::SOL_SOCKET, libc::SO_ERROR)? };
        if raw == 0 { Ok(None) } else { Ok(Some(io::Error::from_raw_os_error(raw as i32))) }
    }

    pub fn as_raw(&self) -> RawFd {
        self.as_raw_fd()
    }
}

impl AsInner<FileDesc> for Socket {
    #[inline]
    fn as_inner(&self) -> &FileDesc {
        &self.0
    }
}

impl IntoInner<FileDesc> for Socket {
    fn into_inner(self) -> FileDesc {
        self.0
    }
}

impl FromInner<FileDesc> for Socket {
    fn from_inner(file_desc: FileDesc) -> Self {
        Self(file_desc)
    }
}

impl AsFd for Socket {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0.as_fd()
    }
}

impl AsRawFd for Socket {
    #[inline]
    fn as_raw_fd(&self) -> RawFd {
        self.0.as_raw_fd()
    }
}

impl IntoRawFd for Socket {
    fn into_raw_fd(self) -> RawFd {
        self.0.into_raw_fd()
    }
}

impl FromRawFd for Socket {
    unsafe fn from_raw_fd(raw_fd: RawFd) -> Self {
        Self(FromRawFd::from_raw_fd(raw_fd))
    }
}

// 在 2.26 之前版本的 glibc 中存在一个 bug：DNS 解析器会缓存
// /etc/resolv.conf 的内容，因此磁盘上对该文件的改动可能会被长期运行的
// 程序忽略。这会破坏 DNS 查询，例如在网络时断时续的笔记本电脑上。详见
// https://sourceware.org/bugzilla/show_bug.cgi?id=984。不过请注意，
// 包括 Debian 在内的一些发行版很久以前就给 glibc 打了补丁来修复这个问题。
//
// 该 bug 的一种变通办法是调用 libc 的 res_init 函数，以清除缓存的配置。
// 不幸的是，虽然我们相信 glibc 对 res_init 的实现是线程安全的，但我们知道
// 其他实现并非如此（https://github.com/rust-lang/rust/issues/43592）。std 中
// 此处的代码可以尝试用 Mutex 来同步它对 res_init 的调用，但那无法保护那些
// 以其他方式调用 libc 的程序。因此，我们不会无条件地调用 res_init，
// 而只在检测到我们链接的是版本 < 2.26 的 glibc 时才调用它。（也就是说，
// 当我们既知道它有必要、又相信它是线程安全的时候。）
#[cfg(all(target_os = "linux", target_env = "gnu"))]
fn on_resolver_failure() {
    use crate::sys;

    // 如果版本解析失败，我们将其按“非 glibc”同等处理。
    if let Some(version) = sys::os::glibc_version() {
        if version < (2, 26) {
            unsafe { libc::res_init() };
        }
    }
}

#[cfg(not(all(target_os = "linux", target_env = "gnu")))]
fn on_resolver_failure() {}
