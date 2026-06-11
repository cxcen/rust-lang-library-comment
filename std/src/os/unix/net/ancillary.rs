// FIXME: 目前在 *BSD 上被禁用。

use super::{SocketAddr, sockaddr_un};
use crate::io::{self, IoSlice, IoSliceMut};
use crate::marker::PhantomData;
use crate::mem::zeroed;
use crate::os::unix::io::RawFd;
use crate::path::Path;
use crate::ptr::{eq, read_unaligned};
use crate::slice::from_raw_parts;
use crate::sys::net::Socket;

// FIXME(#43348): 让 libc 适配 #[doc(cfg(...))]，这样我们就不需要在此放置这些伪定义？
#[cfg(all(
    doc,
    not(target_os = "linux"),
    not(target_os = "android"),
    not(target_os = "netbsd"),
    not(target_os = "freebsd"),
    not(target_os = "cygwin"),
))]
#[allow(non_camel_case_types)]
mod libc {
    pub use core::ffi::c_int;
    pub struct ucred;
    pub struct cmsghdr;
    pub struct sockcred2;
    pub type pid_t = i32;
    pub type gid_t = u32;
    pub type uid_t = u32;
}

pub(super) fn recv_vectored_with_ancillary_from(
    socket: &Socket,
    bufs: &mut [IoSliceMut<'_>],
    ancillary: &mut SocketAncillary<'_>,
) -> io::Result<(usize, bool, io::Result<SocketAddr>)> {
    unsafe {
        let mut msg_name: libc::sockaddr_un = zeroed();
        let mut msg: libc::msghdr = zeroed();
        msg.msg_name = (&raw mut msg_name) as *mut _;
        msg.msg_namelen = size_of::<libc::sockaddr_un>() as libc::socklen_t;
        msg.msg_iov = bufs.as_mut_ptr().cast();
        msg.msg_iovlen = bufs.len() as _;
        msg.msg_controllen = ancillary.buffer.len() as _;
        // macos 要求当 len 为 0 时控制指针必须为 null。
        if msg.msg_controllen > 0 {
            msg.msg_control = ancillary.buffer.as_mut_ptr().cast();
        }

        let count = socket.recv_msg(&mut msg)?;

        ancillary.length = msg.msg_controllen as usize;
        ancillary.truncated = msg.msg_flags & libc::MSG_CTRUNC == libc::MSG_CTRUNC;

        let truncated = msg.msg_flags & libc::MSG_TRUNC == libc::MSG_TRUNC;
        let addr = SocketAddr::from_parts(msg_name, msg.msg_namelen);

        Ok((count, truncated, addr))
    }
}

pub(super) fn send_vectored_with_ancillary_to(
    socket: &Socket,
    path: Option<&Path>,
    bufs: &[IoSlice<'_>],
    ancillary: &mut SocketAncillary<'_>,
) -> io::Result<usize> {
    unsafe {
        let (mut msg_name, msg_namelen) =
            if let Some(path) = path { sockaddr_un(path)? } else { (zeroed(), 0) };

        let mut msg: libc::msghdr = zeroed();
        msg.msg_name = (&raw mut msg_name) as *mut _;
        msg.msg_namelen = msg_namelen;
        msg.msg_iov = bufs.as_ptr() as *mut _;
        msg.msg_iovlen = bufs.len() as _;
        msg.msg_controllen = ancillary.length as _;
        // macos 要求当 len 为 0 时控制指针必须为 null。
        if msg.msg_controllen > 0 {
            msg.msg_control = ancillary.buffer.as_mut_ptr().cast();
        }

        ancillary.truncated = false;

        socket.send_msg(&mut msg)
    }
}

fn add_to_ancillary_data<T>(
    buffer: &mut [u8],
    length: &mut usize,
    source: &[T],
    cmsg_level: libc::c_int,
    cmsg_type: libc::c_int,
) -> bool {
    #[cfg(not(target_os = "freebsd"))]
    let cmsg_size = source.len().checked_mul(size_of::<T>());
    #[cfg(target_os = "freebsd")]
    let cmsg_size = Some(unsafe { libc::SOCKCRED2SIZE(1) });

    let source_len = if let Some(source_len) = cmsg_size {
        if let Ok(source_len) = u32::try_from(source_len) {
            source_len
        } else {
            return false;
        }
    } else {
        return false;
    };

    unsafe {
        let additional_space = libc::CMSG_SPACE(source_len) as usize;

        let new_length = if let Some(new_length) = additional_space.checked_add(*length) {
            new_length
        } else {
            return false;
        };

        if new_length > buffer.len() {
            return false;
        }

        buffer[*length..new_length].fill(0);

        *length = new_length;

        let mut msg: libc::msghdr = zeroed();
        msg.msg_control = buffer.as_mut_ptr().cast();
        msg.msg_controllen = *length as _;

        let mut cmsg = libc::CMSG_FIRSTHDR(&msg);
        let mut previous_cmsg = cmsg;
        while !cmsg.is_null() {
            previous_cmsg = cmsg;
            cmsg = libc::CMSG_NXTHDR(&msg, cmsg);

            // 大多数操作系统（但 Linux 与 emscripten 除外）在前一个指针的长度为零时
            // 会返回它本身。因此，检查前一个指针是否与当前指针相同。
            if eq(cmsg, previous_cmsg) {
                break;
            }
        }

        if previous_cmsg.is_null() {
            return false;
        }

        (*previous_cmsg).cmsg_level = cmsg_level;
        (*previous_cmsg).cmsg_type = cmsg_type;
        (*previous_cmsg).cmsg_len = libc::CMSG_LEN(source_len) as _;

        let data = libc::CMSG_DATA(previous_cmsg).cast();

        libc::memcpy(data, source.as_ptr().cast(), source_len as usize);
    }
    true
}

struct AncillaryDataIter<'a, T> {
    data: &'a [u8],
    phantom: PhantomData<T>,
}

impl<'a, T> AncillaryDataIter<'a, T> {
    /// 创建 `AncillaryDataIter` 结构体，用于遍历控制消息中的数据单元（data unit）。
    ///
    /// # Safety
    ///
    /// `data` 必须包含一条有效的控制消息。
    unsafe fn new(data: &'a [u8]) -> AncillaryDataIter<'a, T> {
        AncillaryDataIter { data, phantom: PhantomData }
    }
}

impl<'a, T> Iterator for AncillaryDataIter<'a, T> {
    type Item = T;

    fn next(&mut self) -> Option<T> {
        if size_of::<T>() <= self.data.len() {
            unsafe {
                let unit = read_unaligned(self.data.as_ptr().cast());
                self.data = &self.data[size_of::<T>()..];
                Some(unit)
            }
        } else {
            None
        }
    }
}

#[cfg(all(
    doc,
    not(target_os = "android"),
    not(target_os = "linux"),
    not(target_os = "netbsd"),
    not(target_os = "freebsd"),
    not(target_os = "cygwin"),
))]
#[unstable(feature = "unix_socket_ancillary_data", issue = "76915")]
#[derive(Clone)]
pub struct SocketCred(());

/// Unix 凭据（credential）。
#[cfg(any(target_os = "android", target_os = "linux", target_os = "cygwin"))]
#[unstable(feature = "unix_socket_ancillary_data", issue = "76915")]
#[derive(Clone)]
pub struct SocketCred(libc::ucred);

#[cfg(target_os = "netbsd")]
#[unstable(feature = "unix_socket_ancillary_data", issue = "76915")]
#[derive(Clone)]
pub struct SocketCred(libc::sockcred);

#[cfg(target_os = "freebsd")]
#[unstable(feature = "unix_socket_ancillary_data", issue = "76915")]
#[derive(Clone)]
pub struct SocketCred(libc::sockcred2);

#[doc(cfg(any(target_os = "android", target_os = "linux", target_os = "cygwin")))]
#[cfg(any(target_os = "android", target_os = "linux", target_os = "cygwin"))]
impl SocketCred {
    /// 创建一个 Unix 凭据（credential）结构体。
    ///
    /// PID、UID 与 GID 均被设置为 0。
    #[unstable(feature = "unix_socket_ancillary_data", issue = "76915")]
    #[must_use]
    pub fn new() -> SocketCred {
        SocketCred(libc::ucred { pid: 0, uid: 0, gid: 0 })
    }

    /// 设置 PID。
    #[unstable(feature = "unix_socket_ancillary_data", issue = "76915")]
    pub fn set_pid(&mut self, pid: libc::pid_t) {
        self.0.pid = pid;
    }

    /// 获取当前的 PID。
    #[must_use]
    #[unstable(feature = "unix_socket_ancillary_data", issue = "76915")]
    pub fn get_pid(&self) -> libc::pid_t {
        self.0.pid
    }

    /// 设置 UID。
    #[unstable(feature = "unix_socket_ancillary_data", issue = "76915")]
    pub fn set_uid(&mut self, uid: libc::uid_t) {
        self.0.uid = uid;
    }

    /// 获取当前的 UID。
    #[must_use]
    #[unstable(feature = "unix_socket_ancillary_data", issue = "76915")]
    pub fn get_uid(&self) -> libc::uid_t {
        self.0.uid
    }

    /// 设置 GID。
    #[unstable(feature = "unix_socket_ancillary_data", issue = "76915")]
    pub fn set_gid(&mut self, gid: libc::gid_t) {
        self.0.gid = gid;
    }

    /// 获取当前的 GID。
    #[must_use]
    #[unstable(feature = "unix_socket_ancillary_data", issue = "76915")]
    pub fn get_gid(&self) -> libc::gid_t {
        self.0.gid
    }
}

#[cfg(target_os = "freebsd")]
impl SocketCred {
    /// 创建一个 Unix 凭据（credential）结构体。
    ///
    /// PID、UID 与 GID 均被设置为 0。
    #[unstable(feature = "unix_socket_ancillary_data", issue = "76915")]
    #[must_use]
    pub fn new() -> SocketCred {
        SocketCred(libc::sockcred2 {
            sc_version: 0,
            sc_pid: 0,
            sc_uid: 0,
            sc_euid: 0,
            sc_gid: 0,
            sc_egid: 0,
            sc_ngroups: 0,
            sc_groups: [0; 1],
        })
    }

    /// 设置 PID。
    #[unstable(feature = "unix_socket_ancillary_data", issue = "76915")]
    pub fn set_pid(&mut self, pid: libc::pid_t) {
        self.0.sc_pid = pid;
    }

    /// 获取当前的 PID。
    #[must_use]
    #[unstable(feature = "unix_socket_ancillary_data", issue = "76915")]
    pub fn get_pid(&self) -> libc::pid_t {
        self.0.sc_pid
    }

    /// 设置 UID。
    #[unstable(feature = "unix_socket_ancillary_data", issue = "76915")]
    pub fn set_uid(&mut self, uid: libc::uid_t) {
        self.0.sc_euid = uid;
    }

    /// 获取当前的 UID。
    #[must_use]
    #[unstable(feature = "unix_socket_ancillary_data", issue = "76915")]
    pub fn get_uid(&self) -> libc::uid_t {
        self.0.sc_euid
    }

    /// 设置 GID。
    #[unstable(feature = "unix_socket_ancillary_data", issue = "76915")]
    pub fn set_gid(&mut self, gid: libc::gid_t) {
        self.0.sc_egid = gid;
    }

    /// 获取当前的 GID。
    #[must_use]
    #[unstable(feature = "unix_socket_ancillary_data", issue = "76915")]
    pub fn get_gid(&self) -> libc::gid_t {
        self.0.sc_egid
    }
}

#[cfg(target_os = "netbsd")]
impl SocketCred {
    /// 创建一个 Unix 凭据（credential）结构体。
    ///
    /// PID、UID 与 GID 均被设置为 0。
    #[unstable(feature = "unix_socket_ancillary_data", issue = "76915")]
    pub fn new() -> SocketCred {
        SocketCred(libc::sockcred {
            sc_pid: 0,
            sc_uid: 0,
            sc_euid: 0,
            sc_gid: 0,
            sc_egid: 0,
            sc_ngroups: 0,
            sc_groups: [0u32; 1],
        })
    }

    /// 设置 PID。
    #[unstable(feature = "unix_socket_ancillary_data", issue = "76915")]
    pub fn set_pid(&mut self, pid: libc::pid_t) {
        self.0.sc_pid = pid;
    }

    /// 获取当前的 PID。
    #[must_use]
    #[unstable(feature = "unix_socket_ancillary_data", issue = "76915")]
    pub fn get_pid(&self) -> libc::pid_t {
        self.0.sc_pid
    }

    /// 设置 UID。
    #[unstable(feature = "unix_socket_ancillary_data", issue = "76915")]
    pub fn set_uid(&mut self, uid: libc::uid_t) {
        self.0.sc_uid = uid;
    }

    /// 获取当前的 UID。
    #[must_use]
    #[unstable(feature = "unix_socket_ancillary_data", issue = "76915")]
    pub fn get_uid(&self) -> libc::uid_t {
        self.0.sc_uid
    }

    /// 设置 GID。
    #[unstable(feature = "unix_socket_ancillary_data", issue = "76915")]
    pub fn set_gid(&mut self, gid: libc::gid_t) {
        self.0.sc_gid = gid;
    }

    /// 获取当前的 GID。
    #[must_use]
    #[unstable(feature = "unix_socket_ancillary_data", issue = "76915")]
    pub fn get_gid(&self) -> libc::gid_t {
        self.0.sc_gid
    }
}

/// 这条控制消息包含文件描述符。
///
/// 其 level 等于 `SOL_SOCKET`，type 等于 `SCM_RIGHTS`。
#[unstable(feature = "unix_socket_ancillary_data", issue = "76915")]
pub struct ScmRights<'a>(AncillaryDataIter<'a, RawFd>);

#[unstable(feature = "unix_socket_ancillary_data", issue = "76915")]
impl<'a> Iterator for ScmRights<'a> {
    type Item = RawFd;

    fn next(&mut self) -> Option<RawFd> {
        self.0.next()
    }
}

#[cfg(all(
    doc,
    not(target_os = "android"),
    not(target_os = "linux"),
    not(target_os = "netbsd"),
    not(target_os = "freebsd"),
    not(target_os = "cygwin"),
))]
#[unstable(feature = "unix_socket_ancillary_data", issue = "76915")]
pub struct ScmCredentials<'a>(AncillaryDataIter<'a, ()>);

/// 这条控制消息包含 unix 凭据（credentials）。
///
/// 其 level 等于 `SOL_SOCKET`，type 等于 `SCM_CREDENTIALS` 或 `SCM_CREDS`。
#[cfg(any(target_os = "android", target_os = "linux", target_os = "cygwin"))]
#[unstable(feature = "unix_socket_ancillary_data", issue = "76915")]
pub struct ScmCredentials<'a>(AncillaryDataIter<'a, libc::ucred>);

#[cfg(target_os = "freebsd")]
#[unstable(feature = "unix_socket_ancillary_data", issue = "76915")]
pub struct ScmCredentials<'a>(AncillaryDataIter<'a, libc::sockcred2>);

#[cfg(target_os = "netbsd")]
#[unstable(feature = "unix_socket_ancillary_data", issue = "76915")]
pub struct ScmCredentials<'a>(AncillaryDataIter<'a, libc::sockcred>);

#[cfg(any(
    doc,
    target_os = "android",
    target_os = "linux",
    target_os = "netbsd",
    target_os = "freebsd",
    target_os = "cygwin",
))]
#[unstable(feature = "unix_socket_ancillary_data", issue = "76915")]
impl<'a> Iterator for ScmCredentials<'a> {
    type Item = SocketCred;

    fn next(&mut self) -> Option<SocketCred> {
        Some(SocketCred(self.0.next()?))
    }
}

/// 在解析某条控制消息的类型时返回的错误类型。
#[non_exhaustive]
#[derive(Debug)]
#[unstable(feature = "unix_socket_ancillary_data", issue = "76915")]
pub enum AncillaryError {
    Unknown { cmsg_level: i32, cmsg_type: i32 },
}

/// 此枚举表示一条类型可变的控制消息。
#[unstable(feature = "unix_socket_ancillary_data", issue = "76915")]
pub enum AncillaryData<'a> {
    ScmRights(ScmRights<'a>),
    #[cfg(any(
        doc,
        target_os = "android",
        target_os = "linux",
        target_os = "netbsd",
        target_os = "freebsd",
        target_os = "cygwin",
    ))]
    ScmCredentials(ScmCredentials<'a>),
}

impl<'a> AncillaryData<'a> {
    /// 创建一个 `AncillaryData::ScmRights` 变体。
    ///
    /// # Safety
    ///
    /// `data` 必须包含一条有效的控制消息，且该控制消息的 type 必须为 `SOL_SOCKET`、
    /// level 必须为 `SCM_RIGHTS`。
    unsafe fn as_rights(data: &'a [u8]) -> Self {
        let ancillary_data_iter = AncillaryDataIter::new(data);
        let scm_rights = ScmRights(ancillary_data_iter);
        AncillaryData::ScmRights(scm_rights)
    }

    /// 创建一个 `AncillaryData::ScmCredentials` 变体。
    ///
    /// # Safety
    ///
    /// `data` 必须包含一条有效的控制消息，且该控制消息的 type 必须为 `SOL_SOCKET`、
    /// level 必须为 `SCM_CREDENTIALS` 或 `SCM_CREDS`。
    #[cfg(any(
        doc,
        target_os = "android",
        target_os = "linux",
        target_os = "netbsd",
        target_os = "freebsd",
        target_os = "cygwin",
    ))]
    unsafe fn as_credentials(data: &'a [u8]) -> Self {
        let ancillary_data_iter = AncillaryDataIter::new(data);
        let scm_credentials = ScmCredentials(ancillary_data_iter);
        AncillaryData::ScmCredentials(scm_credentials)
    }

    fn try_from_cmsghdr(cmsg: &'a libc::cmsghdr) -> Result<Self, AncillaryError> {
        unsafe {
            let cmsg_len_zero = libc::CMSG_LEN(0) as usize;
            let data_len = (*cmsg).cmsg_len as usize - cmsg_len_zero;
            let data = libc::CMSG_DATA(cmsg).cast();
            let data = from_raw_parts(data, data_len);

            match (*cmsg).cmsg_level {
                libc::SOL_SOCKET => match (*cmsg).cmsg_type {
                    libc::SCM_RIGHTS => Ok(AncillaryData::as_rights(data)),
                    #[cfg(any(target_os = "android", target_os = "linux", target_os = "cygwin"))]
                    libc::SCM_CREDENTIALS => Ok(AncillaryData::as_credentials(data)),
                    #[cfg(target_os = "freebsd")]
                    libc::SCM_CREDS2 => Ok(AncillaryData::as_credentials(data)),
                    #[cfg(target_os = "netbsd")]
                    libc::SCM_CREDS => Ok(AncillaryData::as_credentials(data)),
                    cmsg_type => {
                        Err(AncillaryError::Unknown { cmsg_level: libc::SOL_SOCKET, cmsg_type })
                    }
                },
                cmsg_level => {
                    Err(AncillaryError::Unknown { cmsg_level, cmsg_type: (*cmsg).cmsg_type })
                }
            }
        }
    }
}

/// 此结构体用于遍历各条控制消息。
#[must_use = "iterators are lazy and do nothing unless consumed"]
#[unstable(feature = "unix_socket_ancillary_data", issue = "76915")]
pub struct Messages<'a> {
    buffer: &'a [u8],
    current: Option<&'a libc::cmsghdr>,
}

#[unstable(feature = "unix_socket_ancillary_data", issue = "76915")]
impl<'a> Iterator for Messages<'a> {
    type Item = Result<AncillaryData<'a>, AncillaryError>;

    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            let mut msg: libc::msghdr = zeroed();
            msg.msg_control = self.buffer.as_ptr() as *mut _;
            msg.msg_controllen = self.buffer.len() as _;

            let cmsg = if let Some(current) = self.current {
                libc::CMSG_NXTHDR(&msg, current)
            } else {
                libc::CMSG_FIRSTHDR(&msg)
            };

            let cmsg = cmsg.as_ref()?;

            // 大多数操作系统（但 Linux 与 emscripten 除外）在前一个指针的长度为零时
            // 会返回它本身。因此，检查前一个指针是否与当前指针相同。
            if let Some(current) = self.current {
                if eq(current, cmsg) {
                    return None;
                }
            }

            self.current = Some(cmsg);
            let ancillary_result = AncillaryData::try_from_cmsghdr(cmsg);
            Some(ancillary_result)
        }
    }
}

/// 一个 Unix 套接字辅助数据（Ancillary data）结构体。
///
/// # 示例
/// ```no_run
/// #![feature(unix_socket_ancillary_data)]
/// use std::os::unix::net::{UnixStream, SocketAncillary, AncillaryData};
/// use std::io::IoSliceMut;
///
/// fn main() -> std::io::Result<()> {
///     let sock = UnixStream::connect("/tmp/sock")?;
///
///     let mut fds = [0; 8];
///     let mut ancillary_buffer = [0; 128];
///     let mut ancillary = SocketAncillary::new(&mut ancillary_buffer[..]);
///
///     let mut buf = [1; 8];
///     let mut bufs = &mut [IoSliceMut::new(&mut buf[..])][..];
///     sock.recv_vectored_with_ancillary(bufs, &mut ancillary)?;
///
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
#[unstable(feature = "unix_socket_ancillary_data", issue = "76915")]
#[derive(Debug)]
pub struct SocketAncillary<'a> {
    buffer: &'a mut [u8],
    length: usize,
    truncated: bool,
}

impl<'a> SocketAncillary<'a> {
    /// 用给定的缓冲区创建一份辅助数据（ancillary data）。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// # #![allow(unused_mut)]
    /// #![feature(unix_socket_ancillary_data)]
    /// use std::os::unix::net::SocketAncillary;
    /// let mut ancillary_buffer = [0; 128];
    /// let mut ancillary = SocketAncillary::new(&mut ancillary_buffer[..]);
    /// ```
    #[unstable(feature = "unix_socket_ancillary_data", issue = "76915")]
    pub fn new(buffer: &'a mut [u8]) -> Self {
        SocketAncillary { buffer, length: 0, truncated: false }
    }

    /// 返回缓冲区的容量。
    #[must_use]
    #[unstable(feature = "unix_socket_ancillary_data", issue = "76915")]
    pub fn capacity(&self) -> usize {
        self.buffer.len()
    }

    /// 如果该辅助数据为空，则返回 `true`。
    #[must_use]
    #[unstable(feature = "unix_socket_ancillary_data", issue = "76915")]
    pub fn is_empty(&self) -> bool {
        self.length == 0
    }

    /// 返回已使用的字节数。
    #[must_use]
    #[unstable(feature = "unix_socket_ancillary_data", issue = "76915")]
    pub fn len(&self) -> usize {
        self.length
    }

    /// 返回遍历各条控制消息的迭代器。
    #[unstable(feature = "unix_socket_ancillary_data", issue = "76915")]
    pub fn messages(&self) -> Messages<'_> {
        Messages { buffer: &self.buffer[..self.length], current: None }
    }

    /// 如果在一次 recv 操作期间辅助数据被截断（truncated），则为 `true`。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// #![feature(unix_socket_ancillary_data)]
    /// use std::os::unix::net::{UnixStream, SocketAncillary};
    /// use std::io::IoSliceMut;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let sock = UnixStream::connect("/tmp/sock")?;
    ///
    ///     let mut ancillary_buffer = [0; 128];
    ///     let mut ancillary = SocketAncillary::new(&mut ancillary_buffer[..]);
    ///
    ///     let mut buf = [1; 8];
    ///     let mut bufs = &mut [IoSliceMut::new(&mut buf[..])][..];
    ///     sock.recv_vectored_with_ancillary(bufs, &mut ancillary)?;
    ///
    ///     println!("Is truncated: {}", ancillary.truncated());
    ///     Ok(())
    /// }
    /// ```
    #[must_use]
    #[unstable(feature = "unix_socket_ancillary_data", issue = "76915")]
    pub fn truncated(&self) -> bool {
        self.truncated
    }

    /// 向辅助数据（ancillary data）中添加文件描述符。
    ///
    /// 如果缓冲区中有足够空间，该函数返回 `true`。
    /// 如果空间不足，则不会追加任何文件描述符。
    /// 从技术上讲，这意味着此操作会添加一条 level 为 `SOL_SOCKET`、type 为 `SCM_RIGHTS`
    /// 的控制消息。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// #![feature(unix_socket_ancillary_data)]
    /// use std::os::unix::net::{UnixStream, SocketAncillary};
    /// use std::os::unix::io::AsRawFd;
    /// use std::io::IoSlice;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let sock = UnixStream::connect("/tmp/sock")?;
    ///
    ///     let mut ancillary_buffer = [0; 128];
    ///     let mut ancillary = SocketAncillary::new(&mut ancillary_buffer[..]);
    ///     ancillary.add_fds(&[sock.as_raw_fd()][..]);
    ///
    ///     let buf = [1; 8];
    ///     let mut bufs = &mut [IoSlice::new(&buf[..])][..];
    ///     sock.send_vectored_with_ancillary(bufs, &mut ancillary)?;
    ///     Ok(())
    /// }
    /// ```
    #[unstable(feature = "unix_socket_ancillary_data", issue = "76915")]
    pub fn add_fds(&mut self, fds: &[RawFd]) -> bool {
        self.truncated = false;
        add_to_ancillary_data(
            &mut self.buffer,
            &mut self.length,
            fds,
            libc::SOL_SOCKET,
            libc::SCM_RIGHTS,
        )
    }

    /// 向辅助数据（ancillary data）中添加凭据（credentials）。
    ///
    /// 如果缓冲区中有足够空间，该函数返回 `true`。
    /// 如果空间不足，则不会追加任何凭据。
    /// 从技术上讲，这意味着此操作会添加一条 level 为 `SOL_SOCKET`、type 为
    /// `SCM_CREDENTIALS`、`SCM_CREDS` 或 `SCM_CREDS2` 的控制消息。
    ///
    #[cfg(any(
        doc,
        target_os = "android",
        target_os = "linux",
        target_os = "netbsd",
        target_os = "freebsd",
        target_os = "cygwin",
    ))]
    #[unstable(feature = "unix_socket_ancillary_data", issue = "76915")]
    pub fn add_creds(&mut self, creds: &[SocketCred]) -> bool {
        self.truncated = false;
        add_to_ancillary_data(
            &mut self.buffer,
            &mut self.length,
            creds,
            libc::SOL_SOCKET,
            #[cfg(not(any(target_os = "netbsd", target_os = "freebsd")))]
            libc::SCM_CREDENTIALS,
            #[cfg(target_os = "freebsd")]
            libc::SCM_CREDS2,
            #[cfg(target_os = "netbsd")]
            libc::SCM_CREDS,
        )
    }

    /// 清空辅助数据（ancillary data），移除所有值。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// #![feature(unix_socket_ancillary_data)]
    /// use std::os::unix::net::{UnixStream, SocketAncillary, AncillaryData};
    /// use std::io::IoSliceMut;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let sock = UnixStream::connect("/tmp/sock")?;
    ///
    ///     let mut fds1 = [0; 8];
    ///     let mut fds2 = [0; 8];
    ///     let mut ancillary_buffer = [0; 128];
    ///     let mut ancillary = SocketAncillary::new(&mut ancillary_buffer[..]);
    ///
    ///     let mut buf = [1; 8];
    ///     let mut bufs = &mut [IoSliceMut::new(&mut buf[..])][..];
    ///
    ///     sock.recv_vectored_with_ancillary(bufs, &mut ancillary)?;
    ///     for ancillary_result in ancillary.messages() {
    ///         if let AncillaryData::ScmRights(scm_rights) = ancillary_result.unwrap() {
    ///             for fd in scm_rights {
    ///                 println!("receive file descriptor: {fd}");
    ///             }
    ///         }
    ///     }
    ///
    ///     ancillary.clear();
    ///
    ///     sock.recv_vectored_with_ancillary(bufs, &mut ancillary)?;
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
    #[unstable(feature = "unix_socket_ancillary_data", issue = "76915")]
    pub fn clear(&mut self) {
        self.length = 0;
        self.truncated = false;
    }
}
