use crate::io;

pub fn errno() -> i32 {
    unsafe { (*libc::__errno_location()) as i32 }
}

#[inline]
pub fn is_interrupted(errno: i32) -> bool {
    errno == libc::EINTR
}

// 注意：下面的代码是从 unix/mod.rs 1:1 复制过来的
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
        libc::ENOTEMPTY => DirectoryNotEmpty,
        libc::EPIPE => BrokenPipe,
        libc::EROFS => ReadOnlyFilesystem,
        libc::ESPIPE => NotSeekable,
        libc::ESTALE => StaleNetworkFileHandle,
        libc::ETIMEDOUT => TimedOut,
        libc::ETXTBSY => ExecutableFileBusy,
        libc::EXDEV => CrossesDevices,

        libc::EACCES | libc::EPERM => PermissionDenied,

        // 这两个常量在某些系统上可能有相同的值，
        // 但在另一些系统上有不同的值，所以我们不能使用 match
        // 分支
        x if x == libc::EAGAIN || x == libc::EWOULDBLOCK => WouldBlock,

        _ => Uncategorized,
    }
}

pub fn error_string(_errno: i32) -> String {
    "error string unimplemented".to_string()
}
