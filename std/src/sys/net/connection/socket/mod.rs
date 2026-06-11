#[cfg(test)]
mod tests;

use crate::ffi::{c_int, c_void};
use crate::io::{self, BorrowedCursor, ErrorKind, IoSlice, IoSliceMut};
use crate::mem::MaybeUninit;
use crate::net::{
    Ipv4Addr, Ipv6Addr, Shutdown, SocketAddr, SocketAddrV4, SocketAddrV6, ToSocketAddrs,
};
use crate::sys::helpers::run_with_cstr;
use crate::sys::net::connection::each_addr;
use crate::sys::{AsInner, FromInner};
use crate::time::Duration;
use crate::{cmp, fmt, mem, ptr};

cfg_select! {
    target_os = "hermit" => {
        mod hermit;
        pub use hermit::*;
    }
    target_os = "solid_asp3" => {
        mod solid;
        pub use solid::*;
    }
    any(target_family = "unix", target_os = "wasi") => {
        mod unix;
        pub use unix::*;
    }
    target_os = "windows" => {
        mod windows;
        pub use windows::*;
    }
    _ => {}
}

use netc as c;

cfg_select! {
    any(
        target_os = "dragonfly",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "illumos",
        target_os = "solaris",
        target_os = "haiku",
        target_os = "l4re",
        target_os = "nto",
        target_os = "nuttx",
        target_vendor = "apple",
    ) => {
        use c::IPV6_JOIN_GROUP as IPV6_ADD_MEMBERSHIP;
        use c::IPV6_LEAVE_GROUP as IPV6_DROP_MEMBERSHIP;
    }
    _ => {
        use c::IPV6_ADD_MEMBERSHIP;
        use c::IPV6_DROP_MEMBERSHIP;
    }
}

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
        const MSG_NOSIGNAL: c_int = 0x0;
    }
}

cfg_select! {
    any(
        target_os = "dragonfly", target_os = "freebsd",
        target_os = "openbsd", target_os = "netbsd",
        target_os = "solaris", target_os = "illumos",
        target_os = "nto",
    ) => {
        use crate::ffi::c_uchar;
        type IpV4MultiCastType = c_uchar;
    }
    _ => {
        type IpV4MultiCastType = c_int;
    }
}

////////////////////////////////////////////////////////////////////////////////
// 地址转换
////////////////////////////////////////////////////////////////////////////////

fn ip_v4_addr_to_c(addr: &Ipv4Addr) -> c::in_addr {
    // `s_addr` 在所有机器上都以大端（BE）存储，而该数组也是大端字节序。
    // 因此这里使用本机字节序的转换方法，使其永远不会被交换。
    c::in_addr { s_addr: u32::from_ne_bytes(addr.octets()) }
}

fn ip_v6_addr_to_c(addr: &Ipv6Addr) -> c::in6_addr {
    c::in6_addr { s6_addr: addr.octets() }
}

fn ip_v4_addr_from_c(addr: c::in_addr) -> Ipv4Addr {
    Ipv4Addr::from(addr.s_addr.to_ne_bytes())
}

fn ip_v6_addr_from_c(addr: c::in6_addr) -> Ipv6Addr {
    Ipv6Addr::from(addr.s6_addr)
}

fn socket_addr_v4_to_c(addr: &SocketAddrV4) -> c::sockaddr_in {
    c::sockaddr_in {
        sin_family: c::AF_INET as c::sa_family_t,
        sin_port: addr.port().to_be(),
        sin_addr: ip_v4_addr_to_c(addr.ip()),
        ..unsafe { mem::zeroed() }
    }
}

fn socket_addr_v6_to_c(addr: &SocketAddrV6) -> c::sockaddr_in6 {
    c::sockaddr_in6 {
        sin6_family: c::AF_INET6 as c::sa_family_t,
        sin6_port: addr.port().to_be(),
        sin6_addr: ip_v6_addr_to_c(addr.ip()),
        sin6_flowinfo: addr.flowinfo(),
        sin6_scope_id: addr.scope_id(),
        ..unsafe { mem::zeroed() }
    }
}

fn socket_addr_v4_from_c(addr: c::sockaddr_in) -> SocketAddrV4 {
    SocketAddrV4::new(ip_v4_addr_from_c(addr.sin_addr), u16::from_be(addr.sin_port))
}

fn socket_addr_v6_from_c(addr: c::sockaddr_in6) -> SocketAddrV6 {
    SocketAddrV6::new(
        ip_v6_addr_from_c(addr.sin6_addr),
        u16::from_be(addr.sin6_port),
        addr.sin6_flowinfo,
        addr.sin6_scope_id,
    )
}

/// 一个与 `c::sockaddr` 具有相同内存布局的类型。用于将 Rust 层面的
/// SocketAddr* 类型转换为它们的系统表示。相比使用 `c::sockaddr_storage`，
/// 这个特定类型的好处在于：它恰好只有所需的大小，而不会大出很多；并且
/// 它可以在 Rust 中以更干净的方式初始化。
#[repr(C)]
union SocketAddrCRepr {
    v4: c::sockaddr_in,
    v6: c::sockaddr_in6,
}

impl SocketAddrCRepr {
    fn as_ptr(&self) -> *const c::sockaddr {
        self as *const _ as *const c::sockaddr
    }
}

fn socket_addr_to_c(addr: &SocketAddr) -> (SocketAddrCRepr, c::socklen_t) {
    match addr {
        SocketAddr::V4(a) => {
            let sockaddr = SocketAddrCRepr { v4: socket_addr_v4_to_c(a) };
            (sockaddr, size_of::<c::sockaddr_in>() as c::socklen_t)
        }
        SocketAddr::V6(a) => {
            let sockaddr = SocketAddrCRepr { v6: socket_addr_v6_to_c(a) };
            (sockaddr, size_of::<c::sockaddr_in6>() as c::socklen_t)
        }
    }
}

fn addr_family(addr: &SocketAddr) -> c_int {
    match addr {
        SocketAddr::V4(..) => c::AF_INET,
        SocketAddr::V6(..) => c::AF_INET6,
    }
}

/// 将存储在 `storage` 中的 C socket 地址转换为 Rust 的 `SocketAddr`。
///
/// # 安全性(Safety）
/// * `storage` 必须包含一个有效的 C socket 地址，其长度不得大于 `len`。
unsafe fn socket_addr_from_c(
    storage: *const c::sockaddr_storage,
    len: usize,
) -> io::Result<SocketAddr> {
    match (*storage).ss_family as c_int {
        c::AF_INET => {
            assert!(len >= size_of::<c::sockaddr_in>());
            Ok(SocketAddr::V4(socket_addr_v4_from_c(unsafe {
                *(storage as *const _ as *const c::sockaddr_in)
            })))
        }
        c::AF_INET6 => {
            assert!(len >= size_of::<c::sockaddr_in6>());
            Ok(SocketAddr::V6(socket_addr_v6_from_c(unsafe {
                *(storage as *const _ as *const c::sockaddr_in6)
            })))
        }
        _ => Err(io::const_error!(ErrorKind::InvalidInput, "invalid argument")),
    }
}

////////////////////////////////////////////////////////////////////////////////
// sockaddr 及其他杂项绑定
////////////////////////////////////////////////////////////////////////////////

/// 设置某个 socket 选项的值。
///
/// # 安全性(Safety）
/// `T` 必须是与给定 socket 选项相关联的类型。
pub unsafe fn setsockopt<T>(
    sock: &Socket,
    level: c_int,
    option_name: c_int,
    option_value: T,
) -> io::Result<()> {
    let option_len = size_of::<T>() as c::socklen_t;
    // SAFETY:
    // * 在本次调用期间 `sock` 处于打开状态，因为 `sock` 拥有该 socket。
    // * 指向 `option_value` 的指针在 `size_of::<T>` 字节的大小范围内可读。
    // * `option_value` 的值对于给定的 socket 选项而言具有有效的类型
    //   （由调用方保证）。
    cvt(unsafe {
        c::setsockopt(
            sock.as_raw(),
            level,
            option_name,
            (&raw const option_value) as *const _,
            option_len,
        )
    })?;
    Ok(())
}

/// 获取某个 socket 选项的值。
///
/// # 安全性(Safety）
/// `T` 必须是与给定 socket 选项相关联的类型。
pub unsafe fn getsockopt<T: Copy>(
    sock: &Socket,
    level: c_int,
    option_name: c_int,
) -> io::Result<T> {
    let mut option_value = MaybeUninit::<T>::zeroed();
    let mut option_len = size_of::<T>() as c::socklen_t;

    // SAFETY:
    // * 在本次调用期间 `sock` 处于打开状态，因为 `sock` 拥有该 socket。
    // * 指向 `option_value` 的指针可写，且栈上的分配有 `size_of::<T>`
    //   字节的空间。
    cvt(unsafe {
        c::getsockopt(
            sock.as_raw(),
            level,
            option_name,
            option_value.as_mut_ptr().cast(),
            &mut option_len,
        )
    })?;

    // SAFETY: `getsockopt` 调用成功，且调用方保证 `T` 是该选项的类型，
    //         因此 `option_value` 必定已被系统初始化。
    Ok(unsafe { option_value.assume_init() })
}

/// 包装对某个返回 socket 地址的平台函数的调用。
///
/// # 安全性(Safety）
/// * 如果 `f` 返回成功（即对其返回值调用 `cvt` 时返回 `Ok`），则提供给
///   `f` 的缓冲区必定已被初始化为一个有效的 C socket 地址，且其长度必定
///   已写入第二个参数。
unsafe fn sockname<F>(f: F) -> io::Result<SocketAddr>
where
    F: FnOnce(*mut c::sockaddr, *mut c::socklen_t) -> c_int,
{
    let mut storage = MaybeUninit::<c::sockaddr_storage>::zeroed();
    let mut len = size_of::<c::sockaddr_storage>() as c::socklen_t;
    cvt(f(storage.as_mut_ptr().cast(), &mut len))?;
    // SAFETY:
    // 调用方保证：如果 `f` 返回成功，则 storage 已被成功初始化，
    // 且其大小已写入 `len`。
    unsafe { socket_addr_from_c(storage.as_ptr(), len as usize) }
}

#[cfg(target_os = "android")]
fn to_ipv6mr_interface(value: u32) -> c_int {
    value as c_int
}

#[cfg(not(target_os = "android"))]
fn to_ipv6mr_interface(value: u32) -> crate::ffi::c_uint {
    value as crate::ffi::c_uint
}

////////////////////////////////////////////////////////////////////////////////
// lookup_host
////////////////////////////////////////////////////////////////////////////////

pub struct LookupHost {
    original: *mut c::addrinfo,
    cur: *mut c::addrinfo,
    port: u16,
}

impl Iterator for LookupHost {
    type Item = SocketAddr;
    fn next(&mut self) -> Option<SocketAddr> {
        loop {
            unsafe {
                let cur = self.cur.as_ref()?;
                self.cur = cur.ai_next;
                match socket_addr_from_c(cur.ai_addr.cast(), cur.ai_addrlen as usize) {
                    Ok(mut addr) => {
                        addr.set_port(self.port);
                        return Some(addr);
                    }
                    Err(_) => continue,
                }
            }
        }
    }
}

unsafe impl Sync for LookupHost {}
unsafe impl Send for LookupHost {}

impl Drop for LookupHost {
    fn drop(&mut self) {
        unsafe { c::freeaddrinfo(self.original) }
    }
}

pub fn lookup_host(host: &str, port: u16) -> io::Result<LookupHost> {
    init();
    run_with_cstr(host.as_bytes(), &|c_host| {
        let mut hints: c::addrinfo = unsafe { mem::zeroed() };
        hints.ai_socktype = c::SOCK_STREAM;
        let mut res = ptr::null_mut();
        unsafe {
            cvt_gai(c::getaddrinfo(c_host.as_ptr(), ptr::null(), &hints, &mut res))
                .map(|_| LookupHost { original: res, cur: res, port })
        }
    })
}

////////////////////////////////////////////////////////////////////////////////
// TCP 流（TcpStream）
////////////////////////////////////////////////////////////////////////////////

pub struct TcpStream {
    inner: Socket,
}

impl TcpStream {
    pub fn connect<A: ToSocketAddrs>(addr: A) -> io::Result<TcpStream> {
        init();
        return each_addr(addr, inner);

        fn inner(addr: &SocketAddr) -> io::Result<TcpStream> {
            let sock = Socket::new(addr_family(addr), c::SOCK_STREAM)?;
            sock.connect(addr)?;
            Ok(TcpStream { inner: sock })
        }
    }

    pub fn connect_timeout(addr: &SocketAddr, timeout: Duration) -> io::Result<TcpStream> {
        init();

        let sock = Socket::new(addr_family(addr), c::SOCK_STREAM)?;
        sock.connect_timeout(addr, timeout)?;
        Ok(TcpStream { inner: sock })
    }

    #[inline]
    pub fn socket(&self) -> &Socket {
        &self.inner
    }

    pub fn into_socket(self) -> Socket {
        self.inner
    }

    pub fn set_read_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        self.inner.set_timeout(dur, c::SO_RCVTIMEO)
    }

    pub fn set_write_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        self.inner.set_timeout(dur, c::SO_SNDTIMEO)
    }

    pub fn read_timeout(&self) -> io::Result<Option<Duration>> {
        self.inner.timeout(c::SO_RCVTIMEO)
    }

    pub fn write_timeout(&self) -> io::Result<Option<Duration>> {
        self.inner.timeout(c::SO_SNDTIMEO)
    }

    pub fn peek(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.peek(buf)
    }

    pub fn read(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buf)
    }

    pub fn read_buf(&self, buf: BorrowedCursor<'_>) -> io::Result<()> {
        self.inner.read_buf(buf)
    }

    pub fn read_vectored(&self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        self.inner.read_vectored(bufs)
    }

    #[inline]
    pub fn is_read_vectored(&self) -> bool {
        self.inner.is_read_vectored()
    }

    pub fn write(&self, buf: &[u8]) -> io::Result<usize> {
        let len = cmp::min(buf.len(), <wrlen_t>::MAX as usize) as wrlen_t;
        let ret = cvt(unsafe {
            c::send(self.inner.as_raw(), buf.as_ptr() as *const c_void, len, MSG_NOSIGNAL)
        })?;
        Ok(ret as usize)
    }

    pub fn write_vectored(&self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        self.inner.write_vectored(bufs)
    }

    #[inline]
    pub fn is_write_vectored(&self) -> bool {
        self.inner.is_write_vectored()
    }

    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        unsafe { sockname(|buf, len| c::getpeername(self.inner.as_raw(), buf, len)) }
    }

    pub fn socket_addr(&self) -> io::Result<SocketAddr> {
        unsafe { sockname(|buf, len| c::getsockname(self.inner.as_raw(), buf, len)) }
    }

    pub fn shutdown(&self, how: Shutdown) -> io::Result<()> {
        self.inner.shutdown(how)
    }

    pub fn duplicate(&self) -> io::Result<TcpStream> {
        self.inner.duplicate().map(|s| TcpStream { inner: s })
    }

    pub fn set_linger(&self, linger: Option<Duration>) -> io::Result<()> {
        self.inner.set_linger(linger)
    }

    pub fn linger(&self) -> io::Result<Option<Duration>> {
        self.inner.linger()
    }

    pub fn set_nodelay(&self, nodelay: bool) -> io::Result<()> {
        self.inner.set_nodelay(nodelay)
    }

    pub fn nodelay(&self) -> io::Result<bool> {
        self.inner.nodelay()
    }

    pub fn set_ttl(&self, ttl: u32) -> io::Result<()> {
        unsafe { setsockopt(&self.inner, c::IPPROTO_IP, c::IP_TTL, ttl as c_int) }
    }

    pub fn ttl(&self) -> io::Result<u32> {
        let raw: c_int = unsafe { getsockopt(&self.inner, c::IPPROTO_IP, c::IP_TTL)? };
        Ok(raw as u32)
    }

    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        self.inner.take_error()
    }

    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        self.inner.set_nonblocking(nonblocking)
    }
}

impl AsInner<Socket> for TcpStream {
    #[inline]
    fn as_inner(&self) -> &Socket {
        &self.inner
    }
}

impl FromInner<Socket> for TcpStream {
    fn from_inner(socket: Socket) -> TcpStream {
        TcpStream { inner: socket }
    }
}

impl fmt::Debug for TcpStream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut res = f.debug_struct("TcpStream");

        if let Ok(addr) = self.socket_addr() {
            res.field("addr", &addr);
        }

        if let Ok(peer) = self.peer_addr() {
            res.field("peer", &peer);
        }

        let name = if cfg!(windows) { "socket" } else { "fd" };
        res.field(name, &self.inner.as_raw()).finish()
    }
}

////////////////////////////////////////////////////////////////////////////////
// TCP 监听器（TcpListener）
////////////////////////////////////////////////////////////////////////////////

pub struct TcpListener {
    inner: Socket,
}

impl TcpListener {
    pub fn bind<A: ToSocketAddrs>(addr: A) -> io::Result<TcpListener> {
        init();
        return each_addr(addr, inner);

        fn inner(addr: &SocketAddr) -> io::Result<TcpListener> {
            let sock = Socket::new(addr_family(addr), c::SOCK_STREAM)?;

            // 在使用 Berkeley 派生 socket 的平台上，这允许快速重新绑定一个
            // socket，而无需等待操作系统清理掉前一个 socket。
            //
            // 在 Windows 上，这会允许重新绑定正在被使用的 socket，从而可能导致
            //“socket 劫持（socket hijacking）”，所以我们在这里明确不设置它。
            // https://docs.microsoft.com/en-us/windows/win32/winsock/using-so-reuseaddr-and-so-exclusiveaddruse
            #[cfg(not(windows))]
            unsafe {
                setsockopt(&sock, c::SOL_SOCKET, c::SO_REUSEADDR, 1 as c_int)?
            };

            // 绑定我们新建的 socket
            let (addr, len) = socket_addr_to_c(addr);
            cvt(unsafe { c::bind(sock.as_raw(), addr.as_ptr(), len as _) })?;

            let backlog = if cfg!(target_os = "horizon") {
                // 3DS 不支持很大的连接 backlog。有时它允许多达约 37，
                // 但有时甚至连 32 都不接受。这可能是某种全局限制导致的。
                20
            } else if cfg!(target_os = "haiku") {
                // Haiku 不支持队列长度 > 32
                // https://github.com/haiku/haiku/blob/979a0bc487864675517fb2fab28f87dc8bf43041/headers/posix/sys/socket.h#L81
                32
            } else {
                // 所有其他平台的默认值
                128
            };

            // 开始监听
            cvt(unsafe { c::listen(sock.as_raw(), backlog) })?;
            Ok(TcpListener { inner: sock })
        }
    }

    #[inline]
    pub fn socket(&self) -> &Socket {
        &self.inner
    }

    pub fn into_socket(self) -> Socket {
        self.inner
    }

    pub fn socket_addr(&self) -> io::Result<SocketAddr> {
        unsafe { sockname(|buf, len| c::getsockname(self.inner.as_raw(), buf, len)) }
    }

    pub fn accept(&self) -> io::Result<(TcpStream, SocketAddr)> {
        // `accept` 函数会用地址填充 storage，
        // 因此我们在这里不需要将其清零。
        // 参考：https://linux.die.net/man/2/accept4
        let mut storage = MaybeUninit::<c::sockaddr_storage>::uninit();
        let mut len = size_of::<c::sockaddr_storage>() as c::socklen_t;
        let sock = self.inner.accept(storage.as_mut_ptr() as *mut _, &mut len)?;
        let addr = unsafe { socket_addr_from_c(storage.as_ptr(), len as usize)? };
        Ok((TcpStream { inner: sock }, addr))
    }

    pub fn duplicate(&self) -> io::Result<TcpListener> {
        self.inner.duplicate().map(|s| TcpListener { inner: s })
    }

    pub fn set_ttl(&self, ttl: u32) -> io::Result<()> {
        unsafe { setsockopt(&self.inner, c::IPPROTO_IP, c::IP_TTL, ttl as c_int) }
    }

    pub fn ttl(&self) -> io::Result<u32> {
        let raw: c_int = unsafe { getsockopt(&self.inner, c::IPPROTO_IP, c::IP_TTL)? };
        Ok(raw as u32)
    }

    pub fn set_only_v6(&self, only_v6: bool) -> io::Result<()> {
        unsafe { setsockopt(&self.inner, c::IPPROTO_IPV6, c::IPV6_V6ONLY, only_v6 as c_int) }
    }

    pub fn only_v6(&self) -> io::Result<bool> {
        let raw: c_int = unsafe { getsockopt(&self.inner, c::IPPROTO_IPV6, c::IPV6_V6ONLY)? };
        Ok(raw != 0)
    }

    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        self.inner.take_error()
    }

    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        self.inner.set_nonblocking(nonblocking)
    }
}

impl FromInner<Socket> for TcpListener {
    fn from_inner(socket: Socket) -> TcpListener {
        TcpListener { inner: socket }
    }
}

impl fmt::Debug for TcpListener {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut res = f.debug_struct("TcpListener");

        if let Ok(addr) = self.socket_addr() {
            res.field("addr", &addr);
        }

        let name = if cfg!(windows) { "socket" } else { "fd" };
        res.field(name, &self.inner.as_raw()).finish()
    }
}

////////////////////////////////////////////////////////////////////////////////
// UDP
////////////////////////////////////////////////////////////////////////////////

pub struct UdpSocket {
    inner: Socket,
}

impl UdpSocket {
    pub fn bind<A: ToSocketAddrs>(addr: A) -> io::Result<UdpSocket> {
        init();
        return each_addr(addr, inner);

        fn inner(addr: &SocketAddr) -> io::Result<UdpSocket> {
            let sock = Socket::new(addr_family(addr), c::SOCK_DGRAM)?;
            let (addr, len) = socket_addr_to_c(addr);
            cvt(unsafe { c::bind(sock.as_raw(), addr.as_ptr(), len as _) })?;
            Ok(UdpSocket { inner: sock })
        }
    }

    #[inline]
    pub fn socket(&self) -> &Socket {
        &self.inner
    }

    pub fn into_socket(self) -> Socket {
        self.inner
    }

    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        unsafe { sockname(|buf, len| c::getpeername(self.inner.as_raw(), buf, len)) }
    }

    pub fn socket_addr(&self) -> io::Result<SocketAddr> {
        unsafe { sockname(|buf, len| c::getsockname(self.inner.as_raw(), buf, len)) }
    }

    pub fn recv_from(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        self.inner.recv_from(buf)
    }

    pub fn peek_from(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        self.inner.peek_from(buf)
    }

    pub fn send_to(&self, buf: &[u8], dst: &SocketAddr) -> io::Result<usize> {
        let len = cmp::min(buf.len(), <wrlen_t>::MAX as usize) as wrlen_t;
        let (dst, dstlen) = socket_addr_to_c(dst);
        let ret = cvt(unsafe {
            c::sendto(
                self.inner.as_raw(),
                buf.as_ptr() as *const c_void,
                len,
                MSG_NOSIGNAL,
                dst.as_ptr(),
                dstlen,
            )
        })?;
        Ok(ret as usize)
    }

    pub fn duplicate(&self) -> io::Result<UdpSocket> {
        self.inner.duplicate().map(|s| UdpSocket { inner: s })
    }

    pub fn set_read_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        self.inner.set_timeout(dur, c::SO_RCVTIMEO)
    }

    pub fn set_write_timeout(&self, dur: Option<Duration>) -> io::Result<()> {
        self.inner.set_timeout(dur, c::SO_SNDTIMEO)
    }

    pub fn read_timeout(&self) -> io::Result<Option<Duration>> {
        self.inner.timeout(c::SO_RCVTIMEO)
    }

    pub fn write_timeout(&self) -> io::Result<Option<Duration>> {
        self.inner.timeout(c::SO_SNDTIMEO)
    }

    pub fn set_broadcast(&self, broadcast: bool) -> io::Result<()> {
        unsafe { setsockopt(&self.inner, c::SOL_SOCKET, c::SO_BROADCAST, broadcast as c_int) }
    }

    pub fn broadcast(&self) -> io::Result<bool> {
        let raw: c_int = unsafe { getsockopt(&self.inner, c::SOL_SOCKET, c::SO_BROADCAST)? };
        Ok(raw != 0)
    }

    pub fn set_multicast_loop_v4(&self, multicast_loop_v4: bool) -> io::Result<()> {
        unsafe {
            setsockopt(
                &self.inner,
                c::IPPROTO_IP,
                c::IP_MULTICAST_LOOP,
                multicast_loop_v4 as IpV4MultiCastType,
            )
        }
    }

    pub fn multicast_loop_v4(&self) -> io::Result<bool> {
        let raw: IpV4MultiCastType =
            unsafe { getsockopt(&self.inner, c::IPPROTO_IP, c::IP_MULTICAST_LOOP)? };
        Ok(raw != 0)
    }

    pub fn set_multicast_ttl_v4(&self, multicast_ttl_v4: u32) -> io::Result<()> {
        unsafe {
            setsockopt(
                &self.inner,
                c::IPPROTO_IP,
                c::IP_MULTICAST_TTL,
                multicast_ttl_v4 as IpV4MultiCastType,
            )
        }
    }

    pub fn multicast_ttl_v4(&self) -> io::Result<u32> {
        let raw: IpV4MultiCastType =
            unsafe { getsockopt(&self.inner, c::IPPROTO_IP, c::IP_MULTICAST_TTL)? };
        Ok(raw as u32)
    }

    pub fn set_multicast_loop_v6(&self, multicast_loop_v6: bool) -> io::Result<()> {
        unsafe {
            setsockopt(
                &self.inner,
                c::IPPROTO_IPV6,
                c::IPV6_MULTICAST_LOOP,
                multicast_loop_v6 as c_int,
            )
        }
    }

    pub fn multicast_loop_v6(&self) -> io::Result<bool> {
        let raw: c_int =
            unsafe { getsockopt(&self.inner, c::IPPROTO_IPV6, c::IPV6_MULTICAST_LOOP)? };
        Ok(raw != 0)
    }

    pub fn join_multicast_v4(&self, multiaddr: &Ipv4Addr, interface: &Ipv4Addr) -> io::Result<()> {
        let mreq = c::ip_mreq {
            imr_multiaddr: ip_v4_addr_to_c(multiaddr),
            imr_interface: ip_v4_addr_to_c(interface),
        };
        unsafe { setsockopt(&self.inner, c::IPPROTO_IP, c::IP_ADD_MEMBERSHIP, mreq) }
    }

    pub fn join_multicast_v6(&self, multiaddr: &Ipv6Addr, interface: u32) -> io::Result<()> {
        let mreq = c::ipv6_mreq {
            ipv6mr_multiaddr: ip_v6_addr_to_c(multiaddr),
            ipv6mr_interface: to_ipv6mr_interface(interface),
        };
        unsafe { setsockopt(&self.inner, c::IPPROTO_IPV6, IPV6_ADD_MEMBERSHIP, mreq) }
    }

    pub fn leave_multicast_v4(&self, multiaddr: &Ipv4Addr, interface: &Ipv4Addr) -> io::Result<()> {
        let mreq = c::ip_mreq {
            imr_multiaddr: ip_v4_addr_to_c(multiaddr),
            imr_interface: ip_v4_addr_to_c(interface),
        };
        unsafe { setsockopt(&self.inner, c::IPPROTO_IP, c::IP_DROP_MEMBERSHIP, mreq) }
    }

    pub fn leave_multicast_v6(&self, multiaddr: &Ipv6Addr, interface: u32) -> io::Result<()> {
        let mreq = c::ipv6_mreq {
            ipv6mr_multiaddr: ip_v6_addr_to_c(multiaddr),
            ipv6mr_interface: to_ipv6mr_interface(interface),
        };
        unsafe { setsockopt(&self.inner, c::IPPROTO_IPV6, IPV6_DROP_MEMBERSHIP, mreq) }
    }

    pub fn set_ttl(&self, ttl: u32) -> io::Result<()> {
        unsafe { setsockopt(&self.inner, c::IPPROTO_IP, c::IP_TTL, ttl as c_int) }
    }

    pub fn ttl(&self) -> io::Result<u32> {
        let raw: c_int = unsafe { getsockopt(&self.inner, c::IPPROTO_IP, c::IP_TTL)? };
        Ok(raw as u32)
    }

    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        self.inner.take_error()
    }

    pub fn set_nonblocking(&self, nonblocking: bool) -> io::Result<()> {
        self.inner.set_nonblocking(nonblocking)
    }

    pub fn recv(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buf)
    }

    pub fn peek(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.peek(buf)
    }

    pub fn send(&self, buf: &[u8]) -> io::Result<usize> {
        let len = cmp::min(buf.len(), <wrlen_t>::MAX as usize) as wrlen_t;
        let ret = cvt(unsafe {
            c::send(self.inner.as_raw(), buf.as_ptr() as *const c_void, len, MSG_NOSIGNAL)
        })?;
        Ok(ret as usize)
    }

    pub fn connect<A: ToSocketAddrs>(&self, addr: A) -> io::Result<()> {
        return each_addr(addr, |addr| inner(self, addr));

        fn inner(this: &UdpSocket, addr: &SocketAddr) -> io::Result<()> {
            let (addr, len) = socket_addr_to_c(addr);
            cvt_r(|| unsafe { c::connect(this.inner.as_raw(), addr.as_ptr(), len) }).map(drop)
        }
    }
}

impl FromInner<Socket> for UdpSocket {
    fn from_inner(socket: Socket) -> UdpSocket {
        UdpSocket { inner: socket }
    }
}

impl fmt::Debug for UdpSocket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut res = f.debug_struct("UdpSocket");

        if let Ok(addr) = self.socket_addr() {
            res.field("addr", &addr);
        }

        let name = if cfg!(windows) { "socket" } else { "fd" };
        res.field(name, &self.inner.as_raw()).finish()
    }
}
