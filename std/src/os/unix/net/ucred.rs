// 注意：本文件中的代码大量基于 GitHub 上 tokio-uds 仓库 PR 13 所完成的工作。
//
//       供参考，链接在此：https://github.com/tokio-rs/tokio-uds/pull/13
//       感谢 Martin Habovštiak（GitHub 用户名 Kixunil）及各贡献者所做的这项工作。

use libc::{gid_t, pid_t, uid_t};

/// 用于凭据传递（credentials passing）的 UNIX 进程凭据。
#[unstable(feature = "peer_credentials_unix_socket", issue = "42839")]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct UCred {
    /// 对端凭据中的 UID 部分。这是位于域套接字（domain socket）端点处的进程的
    /// 有效 UID（effective UID）。
    pub uid: uid_t,
    /// 对端凭据中的 GID 部分。这是位于域套接字（domain socket）端点处的进程的
    /// 有效 GID（effective GID）。
    pub gid: gid_t,
    /// 对端凭据中的 PID 部分。此字段是可选的，因为对端凭据中的 PID 部分并非在每个平台上
    /// 都受支持。在存在发现 PID 机制的平台上，此字段将被填充为位于域套接字端点处的进程的
    /// PID。否则，它将被设置为 None。
    pub pid: Option<pid_t>,
}

#[cfg(target_vendor = "apple")]
pub(super) use self::impl_apple::peer_cred;
#[cfg(any(
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "nto"
))]
pub(super) use self::impl_bsd::peer_cred;
#[cfg(any(target_os = "android", target_os = "linux", target_os = "cygwin"))]
pub(super) use self::impl_linux::peer_cred;

#[cfg(any(target_os = "linux", target_os = "android", target_os = "cygwin"))]
mod impl_linux {
    use libc::{SO_PEERCRED, SOL_SOCKET, c_void, getsockopt, socklen_t, ucred};

    use super::UCred;
    use crate::io;
    use crate::os::unix::io::AsRawFd;
    use crate::os::unix::net::UnixStream;

    pub fn peer_cred(socket: &UnixStream) -> io::Result<UCred> {
        let ucred_size = size_of::<ucred>();

        // 简单的合理性检查。
        assert!(size_of::<u32>() <= size_of::<usize>());
        assert!(ucred_size <= u32::MAX as usize);

        let mut ucred_size = ucred_size as socklen_t;
        let mut ucred: ucred = ucred { pid: 1, uid: 1, gid: 1 };

        unsafe {
            let ret = getsockopt(
                socket.as_raw_fd(),
                SOL_SOCKET,
                SO_PEERCRED,
                (&raw mut ucred) as *mut c_void,
                &mut ucred_size,
            );

            if ret == 0 && ucred_size as usize == size_of::<ucred>() {
                Ok(UCred { uid: ucred.uid, gid: ucred.gid, pid: Some(ucred.pid) })
            } else {
                Err(io::Error::last_os_error())
            }
        }
    }
}

#[cfg(any(
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "nto",
))]
mod impl_bsd {
    use super::UCred;
    use crate::io;
    use crate::os::unix::io::AsRawFd;
    use crate::os::unix::net::UnixStream;

    pub fn peer_cred(socket: &UnixStream) -> io::Result<UCred> {
        let mut cred = UCred { uid: 1, gid: 1, pid: None };
        unsafe {
            let ret = libc::getpeereid(socket.as_raw_fd(), &mut cred.uid, &mut cred.gid);

            if ret == 0 { Ok(cred) } else { Err(io::Error::last_os_error()) }
        }
    }
}

#[cfg(target_vendor = "apple")]
mod impl_apple {
    use libc::{LOCAL_PEERPID, SOL_LOCAL, c_void, getpeereid, getsockopt, pid_t, socklen_t};

    use super::UCred;
    use crate::io;
    use crate::os::unix::io::AsRawFd;
    use crate::os::unix::net::UnixStream;

    pub fn peer_cred(socket: &UnixStream) -> io::Result<UCred> {
        let mut cred = UCred { uid: 1, gid: 1, pid: None };
        unsafe {
            let ret = getpeereid(socket.as_raw_fd(), &mut cred.uid, &mut cred.gid);

            if ret != 0 {
                return Err(io::Error::last_os_error());
            }

            let mut pid: pid_t = 1;
            let mut pid_size = size_of::<pid_t>() as socklen_t;

            let ret = getsockopt(
                socket.as_raw_fd(),
                SOL_LOCAL,
                LOCAL_PEERPID,
                (&raw mut pid) as *mut c_void,
                &mut pid_size,
            );

            if ret == 0 && pid_size as usize == size_of::<pid_t>() {
                cred.pid = Some(pid);
                Ok(cred)
            } else {
                Err(io::Error::last_os_error())
            }
        }
    }
}
