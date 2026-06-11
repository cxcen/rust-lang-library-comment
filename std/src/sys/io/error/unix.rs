use crate::ffi::{CStr, c_char, c_int};
use crate::io;

unsafe extern "C" {
    #[cfg(not(any(target_os = "dragonfly", target_os = "vxworks", target_os = "rtems")))]
    #[cfg_attr(
        any(
            target_os = "linux",
            target_os = "emscripten",
            target_os = "fuchsia",
            target_os = "l4re",
            target_os = "hurd",
        ),
        link_name = "__errno_location"
    )]
    #[cfg_attr(
        any(
            target_os = "netbsd",
            target_os = "openbsd",
            target_os = "cygwin",
            target_os = "android",
            target_os = "redox",
            target_os = "nuttx",
            target_env = "newlib"
        ),
        link_name = "__errno"
    )]
    #[cfg_attr(any(target_os = "solaris", target_os = "illumos"), link_name = "___errno")]
    #[cfg_attr(target_os = "nto", link_name = "__get_errno_ptr")]
    #[cfg_attr(any(target_os = "freebsd", target_vendor = "apple"), link_name = "__error")]
    #[cfg_attr(target_os = "haiku", link_name = "_errnop")]
    #[cfg_attr(target_os = "aix", link_name = "_Errno")]
    // SAFETY: 在给定线程上它总是返回同一个指针。
    #[unsafe(ffi_const)]
    pub safe fn errno_location() -> *mut c_int;
}

/// 返回平台特定的 errno 值
#[cfg(not(any(target_os = "dragonfly", target_os = "vxworks", target_os = "rtems")))]
#[inline]
pub fn errno() -> i32 {
    unsafe { (*errno_location()) as i32 }
}

/// 设置平台特定的 errno 值
// readdir 和 syscall! 需要它
#[cfg(all(not(target_os = "dragonfly"), not(target_os = "vxworks"), not(target_os = "rtems")))]
#[allow(dead_code)] // 但并非所有目标 cfg 最终都会用到它
#[inline]
pub fn set_errno(e: i32) {
    unsafe { *errno_location() = e as c_int }
}

#[cfg(target_os = "vxworks")]
#[inline]
pub fn errno() -> i32 {
    unsafe { libc::errnoGet() }
}

#[cfg(target_os = "rtems")]
#[inline]
pub fn errno() -> i32 {
    unsafe extern "C" {
        #[thread_local]
        static _tls_errno: c_int;
    }

    unsafe { _tls_errno as i32 }
}

#[cfg(target_os = "dragonfly")]
#[inline]
pub fn errno() -> i32 {
    unsafe extern "C" {
        #[thread_local]
        static errno: c_int;
    }

    unsafe { errno as i32 }
}

#[cfg(target_os = "dragonfly")]
#[allow(dead_code)]
#[inline]
pub fn set_errno(e: i32) {
    unsafe extern "C" {
        #[thread_local]
        static mut errno: c_int;
    }

    unsafe {
        errno = e;
    }
}

#[inline]
pub fn is_interrupted(errno: i32) -> bool {
    errno == libc::EINTR
}

pub fn decode_error_kind(errno: i32) -> io::ErrorKind {
    use io::ErrorKind::*;
    match errno as libc::c_int {
        libc::E2BIG => ArgumentListTooLong,
        libc::EADDRINUSE => AddrInUse,
        libc::EADDRNOTAVAIL => AddrNotAvailable,
        libc::EBUSY => ResourceBusy,
        libc::ECONNABORTED => ConnectionAborted,
        libc::ECONNREFUSED => ConnectionRefused,
        libc::ECONNRESET => ConnectionReset,
        libc::EDEADLK => Deadlock,
        libc::EDQUOT => QuotaExceeded,
        libc::EEXIST => AlreadyExists,
        libc::EFBIG => FileTooLarge,
        libc::EHOSTUNREACH => HostUnreachable,
        libc::EINTR => Interrupted,
        libc::EINVAL => InvalidInput,
        libc::EISDIR => IsADirectory,
        libc::ELOOP => FilesystemLoop,
        libc::ENOENT => NotFound,
        libc::ENOMEM => OutOfMemory,
        libc::ENOSPC => StorageFull,
        libc::ENOSYS => Unsupported,
        libc::EMLINK => TooManyLinks,
        libc::ENAMETOOLONG => InvalidFilename,
        libc::ENETDOWN => NetworkDown,
        libc::ENETUNREACH => NetworkUnreachable,
        libc::ENOTCONN => NotConnected,
        libc::ENOTDIR => NotADirectory,
        #[cfg(not(target_os = "aix"))]
        libc::ENOTEMPTY => DirectoryNotEmpty,
        libc::EPIPE => BrokenPipe,
        libc::EROFS => ReadOnlyFilesystem,
        libc::ESPIPE => NotSeekable,
        libc::ESTALE => StaleNetworkFileHandle,
        libc::ETIMEDOUT => TimedOut,
        libc::ETXTBSY => ExecutableFileBusy,
        libc::EXDEV => CrossesDevices,
        libc::EINPROGRESS => InProgress,
        libc::EOPNOTSUPP => Unsupported,

        libc::EACCES | libc::EPERM => PermissionDenied,

        // 这两个常量在某些系统上可能有相同的值，
        // 但在另一些系统上有不同的值，所以我们不能使用 match
        // 分支
        x if x == libc::EAGAIN || x == libc::EWOULDBLOCK => WouldBlock,

        _ => Uncategorized,
    }
}

/// 获取给定错误号的详细字符串描述。
pub fn error_string(errno: i32) -> String {
    const TMPBUF_SZ: usize = 128;

    unsafe extern "C" {
        #[cfg_attr(
            all(
                any(
                    target_os = "linux",
                    target_os = "hurd",
                    target_env = "newlib",
                    target_os = "cygwin"
                ),
                not(target_env = "ohos")
            ),
            link_name = "__xpg_strerror_r"
        )]
        fn strerror_r(errnum: c_int, buf: *mut c_char, buflen: libc::size_t) -> c_int;
    }

    let mut buf = [0 as c_char; TMPBUF_SZ];

    let p = buf.as_mut_ptr();
    unsafe {
        if strerror_r(errno as c_int, p, buf.len()) < 0 {
            panic!("strerror_r failure");
        }

        let p = p as *const _;
        // 我们并不总是能指望有一个 UTF-8 环境。当我们没有这份运气时，
        // 给出一条低质量的错误消息也比完全没有要好。
        String::from_utf8_lossy(CStr::from_ptr(p).to_bytes()).into()
    }
}
