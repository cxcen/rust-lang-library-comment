#![allow(nonstandard_style)]
#![allow(unsafe_op_in_unsafe_fn)]
// miri 在这里有一些特殊的 hack，会让某些东西变成未使用的。
#![cfg_attr(miri, allow(unused))]

#[cfg(test)]
mod tests;

#[cfg(all(target_os = "linux", target_env = "gnu"))]
use libc::c_char;
#[cfg(any(
    all(target_os = "linux", not(target_env = "musl")),
    target_os = "android",
    target_os = "fuchsia",
    target_os = "hurd",
    target_os = "illumos",
    target_vendor = "apple",
))]
use libc::dirfd;
#[cfg(any(target_os = "fuchsia", target_os = "illumos", target_vendor = "apple"))]
use libc::fstatat as fstatat64;
#[cfg(any(all(target_os = "linux", not(target_env = "musl")), target_os = "hurd"))]
use libc::fstatat64;
#[cfg(any(
    target_os = "aix",
    target_os = "android",
    target_os = "freebsd",
    target_os = "fuchsia",
    target_os = "illumos",
    target_os = "nto",
    target_os = "redox",
    target_os = "solaris",
    target_os = "vita",
    target_os = "wasi",
    all(target_os = "linux", target_env = "musl"),
))]
use libc::readdir as readdir64;
#[cfg(not(any(
    target_os = "aix",
    target_os = "android",
    target_os = "freebsd",
    target_os = "fuchsia",
    target_os = "hurd",
    target_os = "illumos",
    target_os = "l4re",
    target_os = "linux",
    target_os = "nto",
    target_os = "redox",
    target_os = "solaris",
    target_os = "vita",
    target_os = "wasi",
)))]
use libc::readdir_r as readdir64_r;
#[cfg(any(all(target_os = "linux", not(target_env = "musl")), target_os = "hurd"))]
use libc::readdir64;
#[cfg(target_os = "l4re")]
use libc::readdir64_r;
use libc::{c_int, mode_t};
#[cfg(target_os = "android")]
use libc::{
    dirent as dirent64, fstat as fstat64, fstatat as fstatat64, ftruncate64, lseek64,
    lstat as lstat64, off64_t, open as open64, stat as stat64,
};
#[cfg(not(any(
    all(target_os = "linux", not(target_env = "musl")),
    target_os = "l4re",
    target_os = "android",
    target_os = "hurd",
)))]
use libc::{
    dirent as dirent64, fstat as fstat64, ftruncate as ftruncate64, lseek as lseek64,
    lstat as lstat64, off_t as off64_t, open as open64, stat as stat64,
};
#[cfg(any(
    all(target_os = "linux", not(target_env = "musl")),
    target_os = "l4re",
    target_os = "hurd"
))]
use libc::{dirent64, fstat64, ftruncate64, lseek64, lstat64, off64_t, open64, stat64};

use crate::ffi::{CStr, OsStr, OsString};
use crate::fmt::{self, Write as _};
use crate::fs::TryLockError;
use crate::io::{self, BorrowedCursor, Error, IoSlice, IoSliceMut, SeekFrom};
use crate::os::fd::{AsFd, AsRawFd, BorrowedFd, FromRawFd, IntoRawFd};
#[cfg(target_family = "unix")]
use crate::os::unix::prelude::*;
#[cfg(target_os = "wasi")]
use crate::os::wasi::prelude::*;
use crate::path::{Path, PathBuf};
use crate::sync::Arc;
use crate::sys::fd::FileDesc;
pub use crate::sys::fs::common::exists;
use crate::sys::helpers::run_path_with_cstr;
use crate::sys::time::SystemTime;
#[cfg(all(target_os = "linux", target_env = "gnu"))]
use crate::sys::weak::syscall;
#[cfg(target_os = "android")]
use crate::sys::weak::weak;
use crate::sys::{AsInner, AsInnerMut, FromInner, IntoInner, cvt, cvt_r};
use crate::{mem, ptr};

pub struct File(FileDesc);

// FIXME: 它应当在所有 `target_env` 的 Linux 上都可用。
// 但目前只有 glibc 暴露了 `statx` 函数和相关结构体。
// 我们不想在这里直接导入未经验证的原始 C 结构体。
// https://github.com/rust-lang/rust/pull/67774
macro_rules! cfg_has_statx {
    ({ $($then_tt:tt)* } else { $($else_tt:tt)* }) => {
        cfg_select! {
            all(target_os = "linux", target_env = "gnu") => {
                $($then_tt)*
            }
            _ => {
                $($else_tt)*
            }
        }
    };
    ($($block_inner:tt)*) => {
        #[cfg(all(target_os = "linux", target_env = "gnu"))]
        {
            $($block_inner)*
        }
    };
}

cfg_has_statx! {{
    #[derive(Clone)]
    pub struct FileAttr {
        stat: stat64,
        statx_extra_fields: Option<StatxExtraFields>,
    }

    #[derive(Clone)]
    struct StatxExtraFields {
        // 这是检查文件系统是否支持 btime 所必需的。
        stx_mask: u32,
        stx_btime: libc::statx_timestamp,
        // 借助 statx，我们也能克服 32 位 `time_t` 的限制。
        #[cfg(target_pointer_width = "32")]
        stx_atime: libc::statx_timestamp,
        #[cfg(target_pointer_width = "32")]
        stx_ctime: libc::statx_timestamp,
        #[cfg(target_pointer_width = "32")]
        stx_mtime: libc::statx_timestamp,

    }

    // 如果可用，我们在 Linux 上更倾向于使用 `statx`，因为它包含文件创建时间，
    // 以及各种类型的 64 位时间戳。
    // 默认的 `stat64` 不含创建时间，并且可能使用 32 位的 `time_t`。
    unsafe fn try_statx(
        fd: c_int,
        path: *const c_char,
        flags: i32,
        mask: u32,
    ) -> Option<io::Result<FileAttr>> {
        use crate::sync::atomic::{Atomic, AtomicU8, Ordering};

        // 4.11 之前的 Linux 内核或 glibc 2.28 之前的 glibc 不支持 `statx`。
        // 我们在首次失败时检查它，并记住其可用性，以避免不得不再次检查。
        #[repr(u8)]
        enum STATX_STATE{ Unknown = 0, Present, Unavailable }
        static STATX_SAVED_STATE: Atomic<u8> = AtomicU8::new(STATX_STATE::Unknown as u8);

        syscall!(
            fn statx(
                fd: c_int,
                pathname: *const c_char,
                flags: c_int,
                mask: libc::c_uint,
                statxbuf: *mut libc::statx,
            ) -> c_int;
        );

        let statx_availability = STATX_SAVED_STATE.load(Ordering::Relaxed);
        if statx_availability == STATX_STATE::Unavailable as u8 {
            return None;
        }

        let mut buf: libc::statx = mem::zeroed();
        if let Err(err) = cvt(statx(fd, path, flags, mask, &mut buf)) {
            if STATX_SAVED_STATE.load(Ordering::Relaxed) == STATX_STATE::Present as u8 {
                return Some(Err(err));
            }

            // 我们尚不完全确定 `statx` 在这个内核上是否可用。
            // 系统调用返回的错误可能并非来自内核本身，
            // 例如，如果用 seccomp 来阻止该系统调用，可能会返回 `EPERM`，
            // 或者一个有缺陷的 FUSE 驱动可能会返回 `ENOSYS`。
            //
            // 可用性的检查方式是：执行一次调用，如果该系统调用可用，它就应当返回 `EFAULT`。
            //
            // 参见：https://github.com/rust-lang/rust/issues/65662
            //
            // FIXME 那像 `ENOMEM` 这样的瞬时（transient）情况又该怎么办呢？
            let err2 = cvt(statx(0, ptr::null(), 0, libc::STATX_BASIC_STATS | libc::STATX_BTIME, ptr::null_mut()))
                .err()
                .and_then(|e| e.raw_os_error());
            if err2 == Some(libc::EFAULT) {
                STATX_SAVED_STATE.store(STATX_STATE::Present as u8, Ordering::Relaxed);
                return Some(Err(err));
            } else {
                STATX_SAVED_STATE.store(STATX_STATE::Unavailable as u8, Ordering::Relaxed);
                return None;
            }
        }
        if statx_availability == STATX_STATE::Unknown as u8 {
            STATX_SAVED_STATE.store(STATX_STATE::Present as u8, Ordering::Relaxed);
        }

        // 由于存在私有的填充字段（padding fields），我们无法穷尽式地填充 `stat64`。
        let mut stat: stat64 = mem::zeroed();
        // 在 gnu-mips 上是 `c_ulong`，其他平台上是 `dev_t`
        stat.st_dev = libc::makedev(buf.stx_dev_major, buf.stx_dev_minor) as _;
        stat.st_ino = buf.stx_ino as libc::ino64_t;
        stat.st_nlink = buf.stx_nlink as libc::nlink_t;
        stat.st_mode = buf.stx_mode as libc::mode_t;
        stat.st_uid = buf.stx_uid as libc::uid_t;
        stat.st_gid = buf.stx_gid as libc::gid_t;
        stat.st_rdev = libc::makedev(buf.stx_rdev_major, buf.stx_rdev_minor) as _;
        stat.st_size = buf.stx_size as off64_t;
        stat.st_blksize = buf.stx_blksize as libc::blksize_t;
        stat.st_blocks = buf.stx_blocks as libc::blkcnt64_t;
        stat.st_atime = buf.stx_atime.tv_sec as libc::time_t;
        // 在 gnu-x86_64-x32 上是 `i64`，其他平台上是 `c_ulong`。
        stat.st_atime_nsec = buf.stx_atime.tv_nsec as _;
        stat.st_mtime = buf.stx_mtime.tv_sec as libc::time_t;
        stat.st_mtime_nsec = buf.stx_mtime.tv_nsec as _;
        stat.st_ctime = buf.stx_ctime.tv_sec as libc::time_t;
        stat.st_ctime_nsec = buf.stx_ctime.tv_nsec as _;

        let extra = StatxExtraFields {
            stx_mask: buf.stx_mask,
            stx_btime: buf.stx_btime,
            // 存储完整的时间，以避免 32 位 `time_t` 截断。
            #[cfg(target_pointer_width = "32")]
            stx_atime: buf.stx_atime,
            #[cfg(target_pointer_width = "32")]
            stx_ctime: buf.stx_ctime,
            #[cfg(target_pointer_width = "32")]
            stx_mtime: buf.stx_mtime,
        };

        Some(Ok(FileAttr { stat, statx_extra_fields: Some(extra) }))
    }

} else {
    #[derive(Clone)]
    pub struct FileAttr {
        stat: stat64,
    }
}}

// 所有 DirEntry 都会持有一个对该结构体的引用
struct InnerReadDir {
    dirp: DirStream,
    root: PathBuf,
}

pub struct ReadDir {
    inner: Arc<InnerReadDir>,
    end_of_stream: bool,
}

impl ReadDir {
    fn new(inner: InnerReadDir) -> Self {
        Self { inner: Arc::new(inner), end_of_stream: false }
    }
}

struct DirStream(*mut libc::DIR);

// dir::Dir 需要 openat 支持
cfg_select! {
    any(
        target_os = "redox",
        target_os = "espidf",
        target_os = "horizon",
        target_os = "vita",
        target_os = "nto",
        target_os = "vxworks",
    ) => {
        pub use crate::sys::fs::common::Dir;
    }
    _ => {
        mod dir;
        pub use dir::Dir;
    }
}

fn debug_path_fd<'a, 'b>(
    fd: c_int,
    f: &'a mut fmt::Formatter<'b>,
    name: &str,
) -> fmt::DebugStruct<'a, 'b> {
    let mut b = f.debug_struct(name);

    fn get_mode(fd: c_int) -> Option<(bool, bool)> {
        let mode = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        if mode == -1 {
            return None;
        }
        match mode & libc::O_ACCMODE {
            libc::O_RDONLY => Some((true, false)),
            libc::O_RDWR => Some((true, true)),
            libc::O_WRONLY => Some((false, true)),
            _ => None,
        }
    }

    b.field("fd", &fd);
    if let Some(path) = get_path_from_fd(fd) {
        b.field("path", &path);
    }
    if let Some((read, write)) = get_mode(fd) {
        b.field("read", &read).field("write", &write);
    }

    b
}

fn get_path_from_fd(fd: c_int) -> Option<PathBuf> {
    #[cfg(any(target_os = "linux", target_os = "illumos", target_os = "solaris"))]
    fn get_path(fd: c_int) -> Option<PathBuf> {
        let mut p = PathBuf::from("/proc/self/fd");
        p.push(&fd.to_string());
        run_path_with_cstr(&p, &readlink).ok()
    }

    #[cfg(any(target_vendor = "apple", target_os = "netbsd"))]
    fn get_path(fd: c_int) -> Option<PathBuf> {
        // FIXME: 通常不鼓励使用 PATH_MAX，但在这种情况下这是不可避免的，
        // 因为 Apple 平台和 NetBSD 是用 `MAXPATHLEN` 来定义带 `F_GETPATH` 的 `fcntl` 的，
        // 而且没有别的替代方案。如果有人发明了更好的方法，应当改用它。
        let mut buf = vec![0; libc::PATH_MAX as usize];
        let n = unsafe { libc::fcntl(fd, libc::F_GETPATH, buf.as_ptr()) };
        if n == -1 {
            cfg_select! {
                target_os = "netbsd" => {
                    // 作为最后手段，回退（fallback）到 procfs
                    let mut p = PathBuf::from("/proc/self/fd");
                    p.push(&fd.to_string());
                    return run_path_with_cstr(&p, &readlink).ok()
                }
                _ => {
                    return None;
                }
            }
        }
        let l = buf.iter().position(|&c| c == 0).unwrap();
        buf.truncate(l as usize);
        buf.shrink_to_fit();
        Some(PathBuf::from(OsString::from_vec(buf)))
    }

    #[cfg(target_os = "freebsd")]
    fn get_path(fd: c_int) -> Option<PathBuf> {
        let info = Box::<libc::kinfo_file>::new_zeroed();
        let mut info = unsafe { info.assume_init() };
        info.kf_structsize = size_of::<libc::kinfo_file>() as libc::c_int;
        let n = unsafe { libc::fcntl(fd, libc::F_KINFO, &mut *info) };
        if n == -1 {
            return None;
        }
        let buf = unsafe { CStr::from_ptr(info.kf_path.as_mut_ptr()).to_bytes().to_vec() };
        Some(PathBuf::from(OsString::from_vec(buf)))
    }

    #[cfg(target_os = "vxworks")]
    fn get_path(fd: c_int) -> Option<PathBuf> {
        let mut buf = vec![0; libc::PATH_MAX as usize];
        let n = unsafe { libc::ioctl(fd, libc::FIOGETNAME, buf.as_ptr()) };
        if n == -1 {
            return None;
        }
        let l = buf.iter().position(|&c| c == 0).unwrap();
        buf.truncate(l as usize);
        Some(PathBuf::from(OsString::from_vec(buf)))
    }

    #[cfg(not(any(
        target_os = "linux",
        target_os = "vxworks",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "illumos",
        target_os = "solaris",
        target_vendor = "apple",
    )))]
    fn get_path(_fd: c_int) -> Option<PathBuf> {
        // FIXME(#24570): 为其他 Unix 平台实现这一点
        None
    }

    get_path(fd)
}

#[cfg(any(
    target_os = "aix",
    target_os = "android",
    target_os = "freebsd",
    target_os = "fuchsia",
    target_os = "hurd",
    target_os = "illumos",
    target_os = "linux",
    target_os = "nto",
    target_os = "redox",
    target_os = "solaris",
    target_os = "vita",
    target_os = "wasi",
))]
pub struct DirEntry {
    dir: Arc<InnerReadDir>,
    entry: dirent64_min,
    // 在使用 readdir()（而非 readdir_r()）的平台上，我们需要存储一份条目名称的
    // 拥有副本（owned copy），因为：a) struct dirent 可能使用柔性数组（flexible array）
    // 来存储名称；b) 它只在下一次调用 readdir() 之前有效。
    name: crate::ffi::CString,
}

// 定义我们需要从 `dirent64` 中用到的一个最小字段子集，尤其因为在这些目标平台上
// 我们并不直接使用就地的 `d_name`。把它作为 `DirEntry` 中的一个 `entry` 字段保存，
// 有助于减少别处的 `cfg` 样板代码。
#[cfg(any(
    target_os = "aix",
    target_os = "android",
    target_os = "freebsd",
    target_os = "fuchsia",
    target_os = "hurd",
    target_os = "illumos",
    target_os = "linux",
    target_os = "nto",
    target_os = "redox",
    target_os = "solaris",
    target_os = "vita",
    target_os = "wasi",
))]
struct dirent64_min {
    d_ino: u64,
    #[cfg(not(any(
        target_os = "solaris",
        target_os = "illumos",
        target_os = "aix",
        target_os = "nto",
        target_os = "vita",
    )))]
    d_type: u8,
}

#[cfg(not(any(
    target_os = "aix",
    target_os = "android",
    target_os = "freebsd",
    target_os = "fuchsia",
    target_os = "hurd",
    target_os = "illumos",
    target_os = "linux",
    target_os = "nto",
    target_os = "redox",
    target_os = "solaris",
    target_os = "vita",
    target_os = "wasi",
)))]
pub struct DirEntry {
    dir: Arc<InnerReadDir>,
    // 完整的条目包含一个定长的 `d_name`。
    entry: dirent64,
}

#[derive(Clone)]
pub struct OpenOptions {
    // 通用部分
    read: bool,
    write: bool,
    append: bool,
    truncate: bool,
    create: bool,
    create_new: bool,
    // 系统特定部分
    custom_flags: i32,
    mode: mode_t,
}

#[derive(Clone, PartialEq, Eq)]
pub struct FilePermissions {
    mode: mode_t,
}

#[derive(Copy, Clone, Debug, Default)]
pub struct FileTimes {
    accessed: Option<SystemTime>,
    modified: Option<SystemTime>,
    #[cfg(target_vendor = "apple")]
    created: Option<SystemTime>,
}

#[derive(Copy, Clone, Eq)]
pub struct FileType {
    mode: mode_t,
}

impl PartialEq for FileType {
    fn eq(&self, other: &Self) -> bool {
        self.masked() == other.masked()
    }
}

impl core::hash::Hash for FileType {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.masked().hash(state);
    }
}

pub struct DirBuilder {
    mode: mode_t,
}

#[derive(Copy, Clone)]
struct Mode(mode_t);

cfg_has_statx! {{
    impl FileAttr {
        fn from_stat64(stat: stat64) -> Self {
            Self { stat, statx_extra_fields: None }
        }

        #[cfg(target_pointer_width = "32")]
        pub fn stx_mtime(&self) -> Option<&libc::statx_timestamp> {
            if let Some(ext) = &self.statx_extra_fields {
                if (ext.stx_mask & libc::STATX_MTIME) != 0 {
                    return Some(&ext.stx_mtime);
                }
            }
            None
        }

        #[cfg(target_pointer_width = "32")]
        pub fn stx_atime(&self) -> Option<&libc::statx_timestamp> {
            if let Some(ext) = &self.statx_extra_fields {
                if (ext.stx_mask & libc::STATX_ATIME) != 0 {
                    return Some(&ext.stx_atime);
                }
            }
            None
        }

        #[cfg(target_pointer_width = "32")]
        pub fn stx_ctime(&self) -> Option<&libc::statx_timestamp> {
            if let Some(ext) = &self.statx_extra_fields {
                if (ext.stx_mask & libc::STATX_CTIME) != 0 {
                    return Some(&ext.stx_ctime);
                }
            }
            None
        }
    }
} else {
    impl FileAttr {
        fn from_stat64(stat: stat64) -> Self {
            Self { stat }
        }
    }
}}

impl FileAttr {
    pub fn size(&self) -> u64 {
        self.stat.st_size as u64
    }
    pub fn perm(&self) -> FilePermissions {
        FilePermissions { mode: (self.stat.st_mode as mode_t) }
    }

    pub fn file_type(&self) -> FileType {
        FileType { mode: self.stat.st_mode as mode_t }
    }
}

#[cfg(target_os = "netbsd")]
impl FileAttr {
    pub fn modified(&self) -> io::Result<SystemTime> {
        SystemTime::new(self.stat.st_mtime as i64, self.stat.st_mtimensec as i64)
    }

    pub fn accessed(&self) -> io::Result<SystemTime> {
        SystemTime::new(self.stat.st_atime as i64, self.stat.st_atimensec as i64)
    }

    pub fn created(&self) -> io::Result<SystemTime> {
        SystemTime::new(self.stat.st_birthtime as i64, self.stat.st_birthtimensec as i64)
    }
}

#[cfg(target_os = "aix")]
impl FileAttr {
    pub fn modified(&self) -> io::Result<SystemTime> {
        SystemTime::new(self.stat.st_mtime.tv_sec as i64, self.stat.st_mtime.tv_nsec as i64)
    }

    pub fn accessed(&self) -> io::Result<SystemTime> {
        SystemTime::new(self.stat.st_atime.tv_sec as i64, self.stat.st_atime.tv_nsec as i64)
    }

    pub fn created(&self) -> io::Result<SystemTime> {
        SystemTime::new(self.stat.st_ctime.tv_sec as i64, self.stat.st_ctime.tv_nsec as i64)
    }
}

#[cfg(not(any(target_os = "netbsd", target_os = "nto", target_os = "aix", target_os = "wasi")))]
impl FileAttr {
    #[cfg(not(any(
        target_os = "vxworks",
        target_os = "espidf",
        target_os = "horizon",
        target_os = "vita",
        target_os = "hurd",
        target_os = "rtems",
        target_os = "nuttx",
    )))]
    pub fn modified(&self) -> io::Result<SystemTime> {
        #[cfg(target_pointer_width = "32")]
        cfg_has_statx! {
            if let Some(mtime) = self.stx_mtime() {
                return SystemTime::new(mtime.tv_sec, mtime.tv_nsec as i64);
            }
        }

        SystemTime::new(self.stat.st_mtime as i64, self.stat.st_mtime_nsec as i64)
    }

    #[cfg(any(
        target_os = "vxworks",
        target_os = "espidf",
        target_os = "vita",
        target_os = "rtems",
    ))]
    pub fn modified(&self) -> io::Result<SystemTime> {
        SystemTime::new(self.stat.st_mtime as i64, 0)
    }

    #[cfg(any(target_os = "horizon", target_os = "hurd", target_os = "nuttx"))]
    pub fn modified(&self) -> io::Result<SystemTime> {
        SystemTime::new(self.stat.st_mtim.tv_sec as i64, self.stat.st_mtim.tv_nsec as i64)
    }

    #[cfg(not(any(
        target_os = "vxworks",
        target_os = "espidf",
        target_os = "horizon",
        target_os = "vita",
        target_os = "hurd",
        target_os = "rtems",
        target_os = "nuttx",
    )))]
    pub fn accessed(&self) -> io::Result<SystemTime> {
        #[cfg(target_pointer_width = "32")]
        cfg_has_statx! {
            if let Some(atime) = self.stx_atime() {
                return SystemTime::new(atime.tv_sec, atime.tv_nsec as i64);
            }
        }

        SystemTime::new(self.stat.st_atime as i64, self.stat.st_atime_nsec as i64)
    }

    #[cfg(any(
        target_os = "vxworks",
        target_os = "espidf",
        target_os = "vita",
        target_os = "rtems"
    ))]
    pub fn accessed(&self) -> io::Result<SystemTime> {
        SystemTime::new(self.stat.st_atime as i64, 0)
    }

    #[cfg(any(target_os = "horizon", target_os = "hurd", target_os = "nuttx"))]
    pub fn accessed(&self) -> io::Result<SystemTime> {
        SystemTime::new(self.stat.st_atim.tv_sec as i64, self.stat.st_atim.tv_nsec as i64)
    }

    #[cfg(any(
        target_os = "freebsd",
        target_os = "openbsd",
        target_vendor = "apple",
        target_os = "cygwin",
    ))]
    pub fn created(&self) -> io::Result<SystemTime> {
        SystemTime::new(self.stat.st_birthtime as i64, self.stat.st_birthtime_nsec as i64)
    }

    #[cfg(not(any(
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "vita",
        target_vendor = "apple",
        target_os = "cygwin",
    )))]
    pub fn created(&self) -> io::Result<SystemTime> {
        cfg_has_statx! {
            if let Some(ext) = &self.statx_extra_fields {
                return if (ext.stx_mask & libc::STATX_BTIME) != 0 {
                    SystemTime::new(ext.stx_btime.tv_sec, ext.stx_btime.tv_nsec as i64)
                } else {
                    Err(io::const_error!(
                        io::ErrorKind::Unsupported,
                        "creation time is not available for the filesystem",
                    ))
                };
            }
        }

        Err(io::const_error!(
            io::ErrorKind::Unsupported,
            "creation time is not available on this platform currently",
        ))
    }

    #[cfg(target_os = "vita")]
    pub fn created(&self) -> io::Result<SystemTime> {
        SystemTime::new(self.stat.st_ctime as i64, 0)
    }
}

#[cfg(any(target_os = "nto", target_os = "wasi"))]
impl FileAttr {
    pub fn modified(&self) -> io::Result<SystemTime> {
        SystemTime::new(self.stat.st_mtim.tv_sec, self.stat.st_mtim.tv_nsec.into())
    }

    pub fn accessed(&self) -> io::Result<SystemTime> {
        SystemTime::new(self.stat.st_atim.tv_sec, self.stat.st_atim.tv_nsec.into())
    }

    pub fn created(&self) -> io::Result<SystemTime> {
        SystemTime::new(self.stat.st_ctim.tv_sec, self.stat.st_ctim.tv_nsec.into())
    }
}

impl AsInner<stat64> for FileAttr {
    #[inline]
    fn as_inner(&self) -> &stat64 {
        &self.stat
    }
}

impl FilePermissions {
    pub fn readonly(&self) -> bool {
        // 检查是否有任意一类（owner、group、others）拥有写权限
        self.mode & 0o222 == 0
    }

    pub fn set_readonly(&mut self, readonly: bool) {
        if readonly {
            // 移除所有类别的写权限；等价于 `chmod a-w <file>`
            self.mode &= !0o222;
        } else {
            // 为所有类别添加写权限；等价于 `chmod a+w <file>`
            self.mode |= 0o222;
        }
    }
    #[cfg(not(target_os = "wasi"))]
    pub fn mode(&self) -> u32 {
        self.mode as u32
    }
}

impl FileTimes {
    pub fn set_accessed(&mut self, t: SystemTime) {
        self.accessed = Some(t);
    }

    pub fn set_modified(&mut self, t: SystemTime) {
        self.modified = Some(t);
    }

    #[cfg(target_vendor = "apple")]
    pub fn set_created(&mut self, t: SystemTime) {
        self.created = Some(t);
    }
}

impl FileType {
    pub fn is_dir(&self) -> bool {
        self.is(libc::S_IFDIR)
    }
    pub fn is_file(&self) -> bool {
        self.is(libc::S_IFREG)
    }
    pub fn is_symlink(&self) -> bool {
        self.is(libc::S_IFLNK)
    }

    pub fn is(&self, mode: mode_t) -> bool {
        self.masked() == mode
    }

    fn masked(&self) -> mode_t {
        self.mode & libc::S_IFMT
    }
}

impl fmt::Debug for FileType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let FileType { mode } = self;
        f.debug_struct("FileType").field("mode", &Mode(*mode)).finish()
    }
}

impl FromInner<u32> for FilePermissions {
    fn from_inner(mode: u32) -> FilePermissions {
        FilePermissions { mode: mode as mode_t }
    }
}

impl fmt::Debug for FilePermissions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let FilePermissions { mode } = self;
        f.debug_struct("FilePermissions").field("mode", &Mode(*mode)).finish()
    }
}

impl fmt::Debug for ReadDir {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // 它只会从 std::fs::ReadDir 中被调用，后者会添加一个 "ReadDir()" 帧。
        // 因此结果会是例如 'ReadDir("/home")' 这样的形式。
        fmt::Debug::fmt(&*self.inner.root, f)
    }
}

impl Iterator for ReadDir {
    type Item = io::Result<DirEntry>;

    #[cfg(any(
        target_os = "aix",
        target_os = "android",
        target_os = "freebsd",
        target_os = "fuchsia",
        target_os = "hurd",
        target_os = "illumos",
        target_os = "linux",
        target_os = "nto",
        target_os = "redox",
        target_os = "solaris",
        target_os = "vita",
        target_os = "wasi",
    ))]
    fn next(&mut self) -> Option<io::Result<DirEntry>> {
        use crate::sys::io::{errno, set_errno};

        if self.end_of_stream {
            return None;
        }

        unsafe {
            loop {
                // 截至 POSIX.1-2017，readdir() 并不要求是线程安全的；只有
                // readdir_r() 才要求线程安全。然而，readdir_r() 无法正确处理那些
                // 具有无限或可变 NAME_MAX 的平台。许多现代平台保证：只要不并发访问
                // 同一个 DIR*，readdir() 就是线程安全的，这对 Rust 来说已经足够。
                set_errno(0);
                let entry_ptr: *const dirent64 = readdir64(self.inner.dirp.0);
                if entry_ptr.is_null() {
                    // 我们要么遇到了一个错误，要么到达了末尾。无论哪种情况，
                    // 下一次调用 next() 都应当返回 None。
                    self.end_of_stream = true;

                    // 为了区分错误和目录结束（end-of-directory），我们必须事先清空
                    // errno，以便现在能检查是否发生了错误。
                    return match errno() {
                        0 => None,
                        e => Some(Err(Error::from_raw_os_error(e))),
                    };
                }

                // dirent64 结构体是个奇怪的、想象出来的东西，本就不该按值（by value）来使用。
                // 它末尾的 d_name 字段在不同系统上被声明为 [c_char; 256] 或 [c_char; 1]，
                // 但无论哪种方式，那个大小都没有意义；只有 d_name 的偏移量才是有意义的。
                // libc 从 readdir64 返回的 dirent64 指针，允许指向比该结构体定义所暗示的
                // 大小更小 _或_ 更大的分配（allocation）。
                //
                // 因此，相比于其内容“只是”部分初始化的数据，我们需要对 dirent64 更加小心。
                //
                // 和未初始化内容的情况一样，把 entry_ptr 转换为 `&dirent64` 是不合法的。
                // 不过，我们可以使用 `&raw const (*entry_ptr).d_name` 来单独引用各个字段，
                // 因为该操作等价于 `byte_offset`，因此并不要求 `*entry_ptr` 的完整范围
                // 都落在同一个分配的边界内，而只要求所引用字段的那个偏移量落在边界内。

                // d_name 保证以 null 结尾。
                let name = CStr::from_ptr((&raw const (*entry_ptr).d_name).cast());
                let name_bytes = name.to_bytes();
                if name_bytes == b"." || name_bytes == b".." {
                    continue;
                }

                // 从字段加载（load）时，我们可以省略 `&raw const`；作为值表达式的
                // `(*entry_ptr).d_ino` 会做正确的事：先 `byte_offset` 到该字段，
                // 然后只访问那些字节。
                #[cfg(not(target_os = "vita"))]
                let entry = dirent64_min {
                    #[cfg(target_os = "freebsd")]
                    d_ino: (*entry_ptr).d_fileno,
                    #[cfg(not(target_os = "freebsd"))]
                    d_ino: (*entry_ptr).d_ino as u64,
                    #[cfg(not(any(
                        target_os = "solaris",
                        target_os = "illumos",
                        target_os = "aix",
                        target_os = "nto",
                    )))]
                    d_type: (*entry_ptr).d_type as u8,
                };

                #[cfg(target_os = "vita")]
                let entry = dirent64_min { d_ino: 0u64 };

                return Some(Ok(DirEntry {
                    entry,
                    name: name.to_owned(),
                    dir: Arc::clone(&self.inner),
                }));
            }
        }
    }

    #[cfg(not(any(
        target_os = "aix",
        target_os = "android",
        target_os = "freebsd",
        target_os = "fuchsia",
        target_os = "hurd",
        target_os = "illumos",
        target_os = "linux",
        target_os = "nto",
        target_os = "redox",
        target_os = "solaris",
        target_os = "vita",
        target_os = "wasi",
    )))]
    fn next(&mut self) -> Option<io::Result<DirEntry>> {
        if self.end_of_stream {
            return None;
        }

        unsafe {
            let mut ret = DirEntry { entry: mem::zeroed(), dir: Arc::clone(&self.inner) };
            let mut entry_ptr = ptr::null_mut();
            loop {
                let err = readdir64_r(self.inner.dirp.0, &mut ret.entry, &mut entry_ptr);
                if err != 0 {
                    if entry_ptr.is_null() {
                        // 我们遇到了一个错误（它将在本次迭代中返回），但同时也
                        // 到达了目录流（directory stream）的末尾。启用 `end_of_stream`
                        // 标志，以确保我们在下一次迭代中返回 `None`（而不是永远循环下去）。
                        self.end_of_stream = true;
                    }
                    return Some(Err(Error::from_raw_os_error(err)));
                }
                if entry_ptr.is_null() {
                    return None;
                }
                if ret.name_bytes() != b"." && ret.name_bytes() != b".." {
                    return Some(Ok(ret));
                }
            }
        }
    }
}

/// 如果启用了 debug assert，当某个文件描述符未打开时，中止（abort）进程
///
/// 许多 IO 系统调用在 EBADF 错误码方面无法被完全信任，因为这些错误码可能是从
/// 远程 FUSE 服务器冒泡上来的，而不一定是当前进程中的文件描述符真的无效。
///
/// 因此，我们转而检查文件标志（file flags），它存在于文件描述符上而非底层文件上。
/// 缺点是这会多花一次系统调用，所以我们只在 debug 时这么做。
#[inline]
pub(crate) fn debug_assert_fd_is_open(fd: RawFd) {
    use crate::sys::io::errno;

    // 这类似于 assert_unsafe_precondition!()，但它不要求 const
    if core::ub_checks::check_library_ub() {
        if unsafe { libc::fcntl(fd, libc::F_GETFD) } == -1 && errno() == libc::EBADF {
            rtabort!("IO Safety violation: owned file descriptor already closed");
        }
    }
}

impl Drop for DirStream {
    fn drop(&mut self) {
        // dirfd 并非所有平台都支持
        #[cfg(not(any(
            miri,
            target_os = "redox",
            target_os = "nto",
            target_os = "vita",
            target_os = "hurd",
            target_os = "espidf",
            target_os = "horizon",
            target_os = "vxworks",
            target_os = "rtems",
            target_os = "nuttx",
        )))]
        {
            let fd = unsafe { libc::dirfd(self.0) };
            debug_assert_fd_is_open(fd);
        }
        let r = unsafe { libc::closedir(self.0) };
        assert!(
            r == 0 || crate::io::Error::last_os_error().is_interrupted(),
            "unexpected error during closedir: {:?}",
            crate::io::Error::last_os_error()
        );
    }
}

// SAFETY: `int dirfd (DIR *dirstream)` 是 MT-safe（多线程安全）的，这意味着该指针
// 可以安全地在线程之间传递。
unsafe impl Send for DirStream {}
unsafe impl Sync for DirStream {}

impl DirEntry {
    pub fn path(&self) -> PathBuf {
        self.dir.root.join(self.file_name_os_str())
    }

    pub fn file_name(&self) -> OsString {
        self.file_name_os_str().to_os_string()
    }

    #[cfg(all(
        any(
            all(target_os = "linux", not(target_env = "musl")),
            target_os = "android",
            target_os = "fuchsia",
            target_os = "hurd",
            target_os = "illumos",
            target_vendor = "apple",
        ),
        not(miri) // no dirfd on Miri
    ))]
    pub fn metadata(&self) -> io::Result<FileAttr> {
        let fd = cvt(unsafe { dirfd(self.dir.dirp.0) })?;
        let name = self.name_cstr().as_ptr();

        cfg_has_statx! {
            if let Some(ret) = unsafe { try_statx(
                fd,
                name,
                libc::AT_SYMLINK_NOFOLLOW | libc::AT_STATX_SYNC_AS_STAT,
                libc::STATX_BASIC_STATS | libc::STATX_BTIME,
            ) } {
                return ret;
            }
        }

        let mut stat: stat64 = unsafe { mem::zeroed() };
        cvt(unsafe { fstatat64(fd, name, &mut stat, libc::AT_SYMLINK_NOFOLLOW) })?;
        Ok(FileAttr::from_stat64(stat))
    }

    #[cfg(any(
        not(any(
            all(target_os = "linux", not(target_env = "musl")),
            target_os = "android",
            target_os = "fuchsia",
            target_os = "hurd",
            target_os = "illumos",
            target_vendor = "apple",
        )),
        miri
    ))]
    pub fn metadata(&self) -> io::Result<FileAttr> {
        run_path_with_cstr(&self.path(), &lstat)
    }

    #[cfg(any(
        target_os = "solaris",
        target_os = "illumos",
        target_os = "haiku",
        target_os = "vxworks",
        target_os = "aix",
        target_os = "nto",
        target_os = "vita",
    ))]
    pub fn file_type(&self) -> io::Result<FileType> {
        self.metadata().map(|m| m.file_type())
    }

    #[cfg(not(any(
        target_os = "solaris",
        target_os = "illumos",
        target_os = "haiku",
        target_os = "vxworks",
        target_os = "aix",
        target_os = "nto",
        target_os = "vita",
    )))]
    pub fn file_type(&self) -> io::Result<FileType> {
        match self.entry.d_type {
            libc::DT_CHR => Ok(FileType { mode: libc::S_IFCHR }),
            libc::DT_FIFO => Ok(FileType { mode: libc::S_IFIFO }),
            libc::DT_LNK => Ok(FileType { mode: libc::S_IFLNK }),
            libc::DT_REG => Ok(FileType { mode: libc::S_IFREG }),
            libc::DT_SOCK => Ok(FileType { mode: libc::S_IFSOCK }),
            libc::DT_DIR => Ok(FileType { mode: libc::S_IFDIR }),
            libc::DT_BLK => Ok(FileType { mode: libc::S_IFBLK }),
            _ => self.metadata().map(|m| m.file_type()),
        }
    }

    #[cfg(any(
        target_os = "aix",
        target_os = "android",
        target_os = "cygwin",
        target_os = "emscripten",
        target_os = "espidf",
        target_os = "freebsd",
        target_os = "fuchsia",
        target_os = "haiku",
        target_os = "horizon",
        target_os = "hurd",
        target_os = "illumos",
        target_os = "l4re",
        target_os = "linux",
        target_os = "nto",
        target_os = "redox",
        target_os = "rtems",
        target_os = "solaris",
        target_os = "vita",
        target_os = "vxworks",
        target_os = "wasi",
        target_vendor = "apple",
    ))]
    pub fn ino(&self) -> u64 {
        self.entry.d_ino as u64
    }

    #[cfg(any(target_os = "openbsd", target_os = "netbsd", target_os = "dragonfly"))]
    pub fn ino(&self) -> u64 {
        self.entry.d_fileno as u64
    }

    #[cfg(target_os = "nuttx")]
    pub fn ino(&self) -> u64 {
        // 暂且把它保留为 0，因为 NuttX 在其目录条目中不提供 inode 编号。
        0
    }

    #[cfg(any(
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "dragonfly",
        target_vendor = "apple",
    ))]
    fn name_bytes(&self) -> &[u8] {
        use crate::slice;
        unsafe {
            slice::from_raw_parts(
                self.entry.d_name.as_ptr() as *const u8,
                self.entry.d_namlen as usize,
            )
        }
    }
    #[cfg(not(any(
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "dragonfly",
        target_vendor = "apple",
    )))]
    fn name_bytes(&self) -> &[u8] {
        self.name_cstr().to_bytes()
    }

    #[cfg(not(any(
        target_os = "android",
        target_os = "freebsd",
        target_os = "linux",
        target_os = "solaris",
        target_os = "illumos",
        target_os = "fuchsia",
        target_os = "redox",
        target_os = "aix",
        target_os = "nto",
        target_os = "vita",
        target_os = "hurd",
        target_os = "wasi",
    )))]
    fn name_cstr(&self) -> &CStr {
        unsafe { CStr::from_ptr(self.entry.d_name.as_ptr()) }
    }
    #[cfg(any(
        target_os = "android",
        target_os = "freebsd",
        target_os = "linux",
        target_os = "solaris",
        target_os = "illumos",
        target_os = "fuchsia",
        target_os = "redox",
        target_os = "aix",
        target_os = "nto",
        target_os = "vita",
        target_os = "hurd",
        target_os = "wasi",
    ))]
    fn name_cstr(&self) -> &CStr {
        &self.name
    }

    pub fn file_name_os_str(&self) -> &OsStr {
        OsStr::from_bytes(self.name_bytes())
    }
}

impl OpenOptions {
    pub fn new() -> OpenOptions {
        OpenOptions {
            // 通用部分
            read: false,
            write: false,
            append: false,
            truncate: false,
            create: false,
            create_new: false,
            // 系统特定部分
            custom_flags: 0,
            mode: 0o666,
        }
    }

    pub fn read(&mut self, read: bool) {
        self.read = read;
    }
    pub fn write(&mut self, write: bool) {
        self.write = write;
    }
    pub fn append(&mut self, append: bool) {
        self.append = append;
    }
    pub fn truncate(&mut self, truncate: bool) {
        self.truncate = truncate;
    }
    pub fn create(&mut self, create: bool) {
        self.create = create;
    }
    pub fn create_new(&mut self, create_new: bool) {
        self.create_new = create_new;
    }

    pub fn custom_flags(&mut self, flags: i32) {
        self.custom_flags = flags;
    }
    #[cfg(not(target_os = "wasi"))]
    pub fn mode(&mut self, mode: u32) {
        self.mode = mode as mode_t;
    }

    fn get_access_mode(&self) -> io::Result<c_int> {
        match (self.read, self.write, self.append) {
            (true, false, false) => Ok(libc::O_RDONLY),
            (false, true, false) => Ok(libc::O_WRONLY),
            (true, true, false) => Ok(libc::O_RDWR),
            (false, _, true) => Ok(libc::O_WRONLY | libc::O_APPEND),
            (true, _, true) => Ok(libc::O_RDWR | libc::O_APPEND),
            (false, false, false) => {
                // 如果没有设置任何访问模式，检查是否设置了任何创建标志（creation flags），
                // 以便提供一条更具描述性的错误信息
                if self.create || self.create_new || self.truncate {
                    Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "creating or truncating a file requires write or append access",
                    ))
                } else {
                    Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "must specify at least one of read, write, or append access",
                    ))
                }
            }
        }
    }

    fn get_creation_mode(&self) -> io::Result<c_int> {
        match (self.write, self.append) {
            (true, false) => {}
            (false, false) => {
                if self.truncate || self.create || self.create_new {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "creating or truncating a file requires write or append access",
                    ));
                }
            }
            (_, true) => {
                if self.truncate && !self.create_new {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "creating or truncating a file requires write or append access",
                    ));
                }
            }
        }

        Ok(match (self.create, self.truncate, self.create_new) {
            (false, false, false) => 0,
            (true, false, false) => libc::O_CREAT,
            (false, true, false) => libc::O_TRUNC,
            (true, true, false) => libc::O_CREAT | libc::O_TRUNC,
            (_, _, true) => libc::O_CREAT | libc::O_EXCL,
        })
    }
}

impl fmt::Debug for OpenOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let OpenOptions { read, write, append, truncate, create, create_new, custom_flags, mode } =
            self;
        f.debug_struct("OpenOptions")
            .field("read", read)
            .field("write", write)
            .field("append", append)
            .field("truncate", truncate)
            .field("create", create)
            .field("create_new", create_new)
            .field("custom_flags", custom_flags)
            .field("mode", &Mode(*mode))
            .finish()
    }
}

impl File {
    pub fn open(path: &Path, opts: &OpenOptions) -> io::Result<File> {
        run_path_with_cstr(path, &|path| File::open_c(path, opts))
    }

    pub fn open_c(path: &CStr, opts: &OpenOptions) -> io::Result<File> {
        let flags = libc::O_CLOEXEC
            | opts.get_access_mode()?
            | opts.get_creation_mode()?
            | (opts.custom_flags as c_int & !libc::O_ACCMODE);
        // `open64` 的第三个参数被文档规定为 `mode_t` 类型。在某些平台上
        // （例如 macOS，那里 `open64` 实际上就是 `open`），`mode_t` 是 `u16`。
        // 然而，由于这是一个可变参数（variadic）函数，根据 C 的整数提升（integer promotion）
        // 规则，在 ABI 层面它仍然会作为 `c_int`（在 Unix 平台上即 `u32`）传递。
        let fd = cvt_r(|| unsafe { open64(path.as_ptr(), flags, opts.mode as c_int) })?;
        Ok(File(unsafe { FileDesc::from_raw_fd(fd) }))
    }

    pub fn file_attr(&self) -> io::Result<FileAttr> {
        let fd = self.as_raw_fd();

        cfg_has_statx! {
            if let Some(ret) = unsafe { try_statx(
                fd,
                c"".as_ptr() as *const c_char,
                libc::AT_EMPTY_PATH | libc::AT_STATX_SYNC_AS_STAT,
                libc::STATX_BASIC_STATS | libc::STATX_BTIME,
            ) } {
                return ret;
            }
        }

        let mut stat: stat64 = unsafe { mem::zeroed() };
        cvt(unsafe { fstat64(fd, &mut stat) })?;
        Ok(FileAttr::from_stat64(stat))
    }

    pub fn fsync(&self) -> io::Result<()> {
        cvt_r(|| unsafe { os_fsync(self.as_raw_fd()) })?;
        return Ok(());

        #[cfg(target_vendor = "apple")]
        unsafe fn os_fsync(fd: c_int) -> c_int {
            libc::fcntl(fd, libc::F_FULLFSYNC)
        }
        #[cfg(not(target_vendor = "apple"))]
        unsafe fn os_fsync(fd: c_int) -> c_int {
            libc::fsync(fd)
        }
    }

    pub fn datasync(&self) -> io::Result<()> {
        cvt_r(|| unsafe { os_datasync(self.as_raw_fd()) })?;
        return Ok(());

        #[cfg(target_vendor = "apple")]
        unsafe fn os_datasync(fd: c_int) -> c_int {
            libc::fcntl(fd, libc::F_FULLFSYNC)
        }
        #[cfg(any(
            target_os = "freebsd",
            target_os = "fuchsia",
            target_os = "linux",
            target_os = "cygwin",
            target_os = "android",
            target_os = "netbsd",
            target_os = "openbsd",
            target_os = "nto",
            target_os = "hurd",
        ))]
        unsafe fn os_datasync(fd: c_int) -> c_int {
            libc::fdatasync(fd)
        }
        #[cfg(not(any(
            target_os = "android",
            target_os = "fuchsia",
            target_os = "freebsd",
            target_os = "linux",
            target_os = "cygwin",
            target_os = "netbsd",
            target_os = "openbsd",
            target_os = "nto",
            target_os = "hurd",
            target_vendor = "apple",
        )))]
        unsafe fn os_datasync(fd: c_int) -> c_int {
            libc::fsync(fd)
        }
    }

    #[cfg(any(
        target_os = "freebsd",
        target_os = "fuchsia",
        target_os = "hurd",
        target_os = "linux",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "cygwin",
        target_os = "illumos",
        target_os = "aix",
        target_vendor = "apple",
    ))]
    pub fn lock(&self) -> io::Result<()> {
        cvt(unsafe { libc::flock(self.as_raw_fd(), libc::LOCK_EX) })?;
        return Ok(());
    }

    #[cfg(target_os = "solaris")]
    pub fn lock(&self) -> io::Result<()> {
        let mut flock: libc::flock = unsafe { mem::zeroed() };
        flock.l_type = libc::F_WRLCK as libc::c_short;
        flock.l_whence = libc::SEEK_SET as libc::c_short;
        cvt(unsafe { libc::fcntl(self.as_raw_fd(), libc::F_SETLKW, &flock) })?;
        Ok(())
    }

    #[cfg(not(any(
        target_os = "freebsd",
        target_os = "fuchsia",
        target_os = "hurd",
        target_os = "linux",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "cygwin",
        target_os = "solaris",
        target_os = "illumos",
        target_os = "aix",
        target_vendor = "apple",
    )))]
    pub fn lock(&self) -> io::Result<()> {
        Err(io::const_error!(io::ErrorKind::Unsupported, "lock() not supported"))
    }

    #[cfg(any(
        target_os = "freebsd",
        target_os = "fuchsia",
        target_os = "hurd",
        target_os = "linux",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "cygwin",
        target_os = "illumos",
        target_os = "aix",
        target_vendor = "apple",
    ))]
    pub fn lock_shared(&self) -> io::Result<()> {
        cvt(unsafe { libc::flock(self.as_raw_fd(), libc::LOCK_SH) })?;
        return Ok(());
    }

    #[cfg(target_os = "solaris")]
    pub fn lock_shared(&self) -> io::Result<()> {
        let mut flock: libc::flock = unsafe { mem::zeroed() };
        flock.l_type = libc::F_RDLCK as libc::c_short;
        flock.l_whence = libc::SEEK_SET as libc::c_short;
        cvt(unsafe { libc::fcntl(self.as_raw_fd(), libc::F_SETLKW, &flock) })?;
        Ok(())
    }

    #[cfg(not(any(
        target_os = "freebsd",
        target_os = "fuchsia",
        target_os = "hurd",
        target_os = "linux",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "cygwin",
        target_os = "solaris",
        target_os = "illumos",
        target_os = "aix",
        target_vendor = "apple",
    )))]
    pub fn lock_shared(&self) -> io::Result<()> {
        Err(io::const_error!(io::ErrorKind::Unsupported, "lock_shared() not supported"))
    }

    #[cfg(any(
        target_os = "freebsd",
        target_os = "fuchsia",
        target_os = "hurd",
        target_os = "linux",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "cygwin",
        target_os = "illumos",
        target_os = "aix",
        target_vendor = "apple",
    ))]
    pub fn try_lock(&self) -> Result<(), TryLockError> {
        let result = cvt(unsafe { libc::flock(self.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) });
        if let Err(err) = result {
            if err.kind() == io::ErrorKind::WouldBlock {
                Err(TryLockError::WouldBlock)
            } else {
                Err(TryLockError::Error(err))
            }
        } else {
            Ok(())
        }
    }

    #[cfg(target_os = "solaris")]
    pub fn try_lock(&self) -> Result<(), TryLockError> {
        let mut flock: libc::flock = unsafe { mem::zeroed() };
        flock.l_type = libc::F_WRLCK as libc::c_short;
        flock.l_whence = libc::SEEK_SET as libc::c_short;
        let result = cvt(unsafe { libc::fcntl(self.as_raw_fd(), libc::F_SETLK, &flock) });
        if let Err(err) = result {
            if err.kind() == io::ErrorKind::WouldBlock {
                Err(TryLockError::WouldBlock)
            } else {
                Err(TryLockError::Error(err))
            }
        } else {
            Ok(())
        }
    }

    #[cfg(not(any(
        target_os = "freebsd",
        target_os = "fuchsia",
        target_os = "hurd",
        target_os = "linux",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "cygwin",
        target_os = "solaris",
        target_os = "illumos",
        target_os = "aix",
        target_vendor = "apple",
    )))]
    pub fn try_lock(&self) -> Result<(), TryLockError> {
        Err(TryLockError::Error(io::const_error!(
            io::ErrorKind::Unsupported,
            "try_lock() not supported"
        )))
    }

    #[cfg(any(
        target_os = "freebsd",
        target_os = "fuchsia",
        target_os = "hurd",
        target_os = "linux",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "cygwin",
        target_os = "illumos",
        target_os = "aix",
        target_vendor = "apple",
    ))]
    pub fn try_lock_shared(&self) -> Result<(), TryLockError> {
        let result = cvt(unsafe { libc::flock(self.as_raw_fd(), libc::LOCK_SH | libc::LOCK_NB) });
        if let Err(err) = result {
            if err.kind() == io::ErrorKind::WouldBlock {
                Err(TryLockError::WouldBlock)
            } else {
                Err(TryLockError::Error(err))
            }
        } else {
            Ok(())
        }
    }

    #[cfg(target_os = "solaris")]
    pub fn try_lock_shared(&self) -> Result<(), TryLockError> {
        let mut flock: libc::flock = unsafe { mem::zeroed() };
        flock.l_type = libc::F_RDLCK as libc::c_short;
        flock.l_whence = libc::SEEK_SET as libc::c_short;
        let result = cvt(unsafe { libc::fcntl(self.as_raw_fd(), libc::F_SETLK, &flock) });
        if let Err(err) = result {
            if err.kind() == io::ErrorKind::WouldBlock {
                Err(TryLockError::WouldBlock)
            } else {
                Err(TryLockError::Error(err))
            }
        } else {
            Ok(())
        }
    }

    #[cfg(not(any(
        target_os = "freebsd",
        target_os = "fuchsia",
        target_os = "hurd",
        target_os = "linux",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "cygwin",
        target_os = "solaris",
        target_os = "illumos",
        target_os = "aix",
        target_vendor = "apple",
    )))]
    pub fn try_lock_shared(&self) -> Result<(), TryLockError> {
        Err(TryLockError::Error(io::const_error!(
            io::ErrorKind::Unsupported,
            "try_lock_shared() not supported"
        )))
    }

    #[cfg(any(
        target_os = "freebsd",
        target_os = "fuchsia",
        target_os = "hurd",
        target_os = "linux",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "cygwin",
        target_os = "illumos",
        target_os = "aix",
        target_vendor = "apple",
    ))]
    pub fn unlock(&self) -> io::Result<()> {
        cvt(unsafe { libc::flock(self.as_raw_fd(), libc::LOCK_UN) })?;
        return Ok(());
    }

    #[cfg(target_os = "solaris")]
    pub fn unlock(&self) -> io::Result<()> {
        let mut flock: libc::flock = unsafe { mem::zeroed() };
        flock.l_type = libc::F_UNLCK as libc::c_short;
        flock.l_whence = libc::SEEK_SET as libc::c_short;
        cvt(unsafe { libc::fcntl(self.as_raw_fd(), libc::F_SETLKW, &flock) })?;
        Ok(())
    }

    #[cfg(not(any(
        target_os = "freebsd",
        target_os = "fuchsia",
        target_os = "hurd",
        target_os = "linux",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "cygwin",
        target_os = "solaris",
        target_os = "illumos",
        target_os = "aix",
        target_vendor = "apple",
    )))]
    pub fn unlock(&self) -> io::Result<()> {
        Err(io::const_error!(io::ErrorKind::Unsupported, "unlock() not supported"))
    }

    pub fn truncate(&self, size: u64) -> io::Result<()> {
        let size: off64_t =
            size.try_into().map_err(|e| io::Error::new(io::ErrorKind::InvalidInput, e))?;
        cvt_r(|| unsafe { ftruncate64(self.as_raw_fd(), size) }).map(drop)
    }

    pub fn read(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.read(buf)
    }

    pub fn read_vectored(&self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        self.0.read_vectored(bufs)
    }

    #[inline]
    pub fn is_read_vectored(&self) -> bool {
        self.0.is_read_vectored()
    }

    pub fn read_at(&self, buf: &mut [u8], offset: u64) -> io::Result<usize> {
        self.0.read_at(buf, offset)
    }

    pub fn read_buf(&self, cursor: BorrowedCursor<'_>) -> io::Result<()> {
        self.0.read_buf(cursor)
    }

    pub fn read_buf_at(&self, cursor: BorrowedCursor<'_>, offset: u64) -> io::Result<()> {
        self.0.read_buf_at(cursor, offset)
    }

    pub fn read_vectored_at(&self, bufs: &mut [IoSliceMut<'_>], offset: u64) -> io::Result<usize> {
        self.0.read_vectored_at(bufs, offset)
    }

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

    pub fn write_at(&self, buf: &[u8], offset: u64) -> io::Result<usize> {
        self.0.write_at(buf, offset)
    }

    pub fn write_vectored_at(&self, bufs: &[IoSlice<'_>], offset: u64) -> io::Result<usize> {
        self.0.write_vectored_at(bufs, offset)
    }

    #[inline]
    pub fn flush(&self) -> io::Result<()> {
        Ok(())
    }

    pub fn seek(&self, pos: SeekFrom) -> io::Result<u64> {
        let (whence, pos) = match pos {
            // 转换为 `i64` 是没问题的，过大的值会变成负数，
            // 这会在 `lseek64` 中导致一个错误。
            SeekFrom::Start(off) => (libc::SEEK_SET, off as i64),
            SeekFrom::End(off) => (libc::SEEK_END, off),
            SeekFrom::Current(off) => (libc::SEEK_CUR, off),
        };
        let n = cvt(unsafe { lseek64(self.as_raw_fd(), pos as off64_t, whence) })?;
        Ok(n as u64)
    }

    pub fn size(&self) -> Option<io::Result<u64>> {
        match self.file_attr().map(|attr| attr.size()) {
            // 如果返回的大小为 0，则回退到默认实现，
            // 因为我们可能处于一个 proc 挂载点（proc mount）中。
            Ok(0) => None,
            result => Some(result),
        }
    }

    pub fn tell(&self) -> io::Result<u64> {
        self.seek(SeekFrom::Current(0))
    }

    pub fn duplicate(&self) -> io::Result<File> {
        self.0.duplicate().map(File)
    }

    pub fn set_permissions(&self, perm: FilePermissions) -> io::Result<()> {
        cvt_r(|| unsafe { libc::fchmod(self.as_raw_fd(), perm.mode) })?;
        Ok(())
    }

    pub fn set_times(&self, times: FileTimes) -> io::Result<()> {
        cfg_select! {
            any(target_os = "redox", target_os = "espidf", target_os = "horizon", target_os = "nuttx") => {
                // Redox 似乎不支持 `UTIME_OMIT`。
                // ESP-IDF 和 HorizonOS 完全不支持 `futimens`，因此这些操作系统的行为
                // 与 Redox 相同。
                let _ = times;
                Err(io::const_error!(
                    io::ErrorKind::Unsupported,
                    "setting file times not supported",
                ))
            }
            target_vendor = "apple" => {
                let ta = TimesAttrlist::from_times(&times)?;
                cvt(unsafe { libc::fsetattrlist(
                    self.as_raw_fd(),
                    ta.attrlist(),
                    ta.times_buf(),
                    ta.times_buf_size(),
                    0
                ) })?;
                Ok(())
            }
            target_os = "android" => {
                let times = [file_time_to_timespec(times.accessed)?, file_time_to_timespec(times.modified)?];
                // futimens 要求 Android API level 19
                cvt(unsafe {
                    weak!(
                        fn futimens(fd: c_int, times: *const libc::timespec) -> c_int;
                    );
                    match futimens.get() {
                        Some(futimens) => futimens(self.as_raw_fd(), times.as_ptr()),
                        None => return Err(io::const_error!(
                            io::ErrorKind::Unsupported,
                            "setting file times requires Android API level >= 19",
                        )),
                    }
                })?;
                Ok(())
            }
            _ => {
                #[cfg(all(target_os = "linux", target_env = "gnu", target_pointer_width = "32", not(target_arch = "riscv32")))]
                {
                    use crate::sys::{time::__timespec64, weak::weak};

                    // 在 glibc 2.34 中添加
                    weak!(
                        fn __futimens64(fd: c_int, times: *const __timespec64) -> c_int;
                    );

                    if let Some(futimens64) = __futimens64.get() {
                        let to_timespec = |time: Option<SystemTime>| time.map(|time| time.t.to_timespec64())
                            .unwrap_or(__timespec64::new(0, libc::UTIME_OMIT as _));
                        let times = [to_timespec(times.accessed), to_timespec(times.modified)];
                        cvt(unsafe { futimens64(self.as_raw_fd(), times.as_ptr()) })?;
                        return Ok(());
                    }
                }
                let times = [file_time_to_timespec(times.accessed)?, file_time_to_timespec(times.modified)?];
                cvt(unsafe { libc::futimens(self.as_raw_fd(), times.as_ptr()) })?;
                Ok(())
            }
        }
    }
}

#[cfg(not(any(
    target_os = "redox",
    target_os = "espidf",
    target_os = "horizon",
    target_os = "nuttx",
)))]
fn file_time_to_timespec(time: Option<SystemTime>) -> io::Result<libc::timespec> {
    match time {
        Some(time) if let Some(ts) = time.t.to_timespec() => Ok(ts),
        Some(time) if time > crate::sys::time::UNIX_EPOCH => Err(io::const_error!(
            io::ErrorKind::InvalidInput,
            "timestamp is too large to set as a file time",
        )),
        Some(_) => Err(io::const_error!(
            io::ErrorKind::InvalidInput,
            "timestamp is too small to set as a file time",
        )),
        None => Ok(libc::timespec { tv_sec: 0, tv_nsec: libc::UTIME_OMIT as _ }),
    }
}

#[cfg(target_vendor = "apple")]
struct TimesAttrlist {
    buf: [mem::MaybeUninit<libc::timespec>; 3],
    attrlist: libc::attrlist,
    num_times: usize,
}

#[cfg(target_vendor = "apple")]
impl TimesAttrlist {
    fn from_times(times: &FileTimes) -> io::Result<Self> {
        let mut this = Self {
            buf: [mem::MaybeUninit::<libc::timespec>::uninit(); 3],
            attrlist: unsafe { mem::zeroed() },
            num_times: 0,
        };
        this.attrlist.bitmapcount = libc::ATTR_BIT_MAP_COUNT;
        if times.created.is_some() {
            this.buf[this.num_times].write(file_time_to_timespec(times.created)?);
            this.num_times += 1;
            this.attrlist.commonattr |= libc::ATTR_CMN_CRTIME;
        }
        if times.modified.is_some() {
            this.buf[this.num_times].write(file_time_to_timespec(times.modified)?);
            this.num_times += 1;
            this.attrlist.commonattr |= libc::ATTR_CMN_MODTIME;
        }
        if times.accessed.is_some() {
            this.buf[this.num_times].write(file_time_to_timespec(times.accessed)?);
            this.num_times += 1;
            this.attrlist.commonattr |= libc::ATTR_CMN_ACCTIME;
        }
        Ok(this)
    }

    fn attrlist(&self) -> *mut libc::c_void {
        (&raw const self.attrlist).cast::<libc::c_void>().cast_mut()
    }

    fn times_buf(&self) -> *mut libc::c_void {
        self.buf.as_ptr().cast::<libc::c_void>().cast_mut()
    }

    fn times_buf_size(&self) -> usize {
        self.num_times * size_of::<libc::timespec>()
    }
}

impl DirBuilder {
    pub fn new() -> DirBuilder {
        DirBuilder { mode: 0o777 }
    }

    pub fn mkdir(&self, p: &Path) -> io::Result<()> {
        run_path_with_cstr(p, &|p| cvt(unsafe { libc::mkdir(p.as_ptr(), self.mode) }).map(|_| ()))
    }

    #[cfg(not(target_os = "wasi"))]
    pub fn set_mode(&mut self, mode: u32) {
        self.mode = mode as mode_t;
    }
}

impl fmt::Debug for DirBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let DirBuilder { mode } = self;
        f.debug_struct("DirBuilder").field("mode", &Mode(*mode)).finish()
    }
}

impl AsInner<FileDesc> for File {
    #[inline]
    fn as_inner(&self) -> &FileDesc {
        &self.0
    }
}

impl AsInnerMut<FileDesc> for File {
    #[inline]
    fn as_inner_mut(&mut self) -> &mut FileDesc {
        &mut self.0
    }
}

impl IntoInner<FileDesc> for File {
    fn into_inner(self) -> FileDesc {
        self.0
    }
}

impl FromInner<FileDesc> for File {
    fn from_inner(file_desc: FileDesc) -> Self {
        Self(file_desc)
    }
}

impl AsFd for File {
    #[inline]
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0.as_fd()
    }
}

impl AsRawFd for File {
    #[inline]
    fn as_raw_fd(&self) -> RawFd {
        self.0.as_raw_fd()
    }
}

impl IntoRawFd for File {
    fn into_raw_fd(self) -> RawFd {
        self.0.into_raw_fd()
    }
}

impl FromRawFd for File {
    unsafe fn from_raw_fd(raw_fd: RawFd) -> Self {
        Self(FromRawFd::from_raw_fd(raw_fd))
    }
}

impl fmt::Debug for File {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let fd = self.as_raw_fd();
        let mut b = debug_path_fd(fd, f, "File");
        b.finish()
    }
}

// 以八进制格式输出，后跟 `ls -l` 中使用的 mode 格式。
//
// 参考资料：
//   https://pubs.opengroup.org/onlinepubs/009696899/utilities/ls.html
//   https://www.gnu.org/software/libc/manual/html_node/Testing-File-Type.html
//   https://www.gnu.org/software/libc/manual/html_node/Permission-Bits.html
//
// 示例：
//   0o100664 (-rw-rw-r--)
impl fmt::Debug for Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self(mode) = *self;
        write!(f, "0o{mode:06o}")?;

        let entry_type = match mode & libc::S_IFMT {
            libc::S_IFDIR => 'd',
            libc::S_IFBLK => 'b',
            libc::S_IFCHR => 'c',
            libc::S_IFLNK => 'l',
            libc::S_IFIFO => 'p',
            libc::S_IFREG => '-',
            _ => return Ok(()),
        };

        f.write_str(" (")?;
        f.write_char(entry_type)?;

        // 属主（Owner）权限
        f.write_char(if mode & libc::S_IRUSR != 0 { 'r' } else { '-' })?;
        f.write_char(if mode & libc::S_IWUSR != 0 { 'w' } else { '-' })?;
        let owner_executable = mode & libc::S_IXUSR != 0;
        let setuid = mode as c_int & libc::S_ISUID as c_int != 0;
        f.write_char(match (owner_executable, setuid) {
            (true, true) => 's',  // 可执行且设置了 setuid
            (false, true) => 'S', // 设置了 setuid
            (true, false) => 'x', // 可执行
            (false, false) => '-',
        })?;

        // 用户组（Group）权限
        f.write_char(if mode & libc::S_IRGRP != 0 { 'r' } else { '-' })?;
        f.write_char(if mode & libc::S_IWGRP != 0 { 'w' } else { '-' })?;
        let group_executable = mode & libc::S_IXGRP != 0;
        let setgid = mode as c_int & libc::S_ISGID as c_int != 0;
        f.write_char(match (group_executable, setgid) {
            (true, true) => 's',  // 可执行且设置了 setgid
            (false, true) => 'S', // 设置了 setgid
            (true, false) => 'x', // 可执行
            (false, false) => '-',
        })?;

        // 其他用户（Other）权限
        f.write_char(if mode & libc::S_IROTH != 0 { 'r' } else { '-' })?;
        f.write_char(if mode & libc::S_IWOTH != 0 { 'w' } else { '-' })?;
        let other_executable = mode & libc::S_IXOTH != 0;
        let sticky = mode as c_int & libc::S_ISVTX as c_int != 0;
        f.write_char(match (entry_type, other_executable, sticky) {
            ('d', true, true) => 't',  // 可搜索且受限删除（restricted deletion）
            ('d', false, true) => 'T', // 受限删除（restricted deletion）
            (_, true, _) => 'x',       // 可执行
            (_, false, _) => '-',
        })?;

        f.write_char(')')
    }
}

pub fn readdir(path: &Path) -> io::Result<ReadDir> {
    let ptr = run_path_with_cstr(path, &|p| unsafe { Ok(libc::opendir(p.as_ptr())) })?;
    if ptr.is_null() {
        Err(Error::last_os_error())
    } else {
        let root = path.to_path_buf();
        let inner = InnerReadDir { dirp: DirStream(ptr), root };
        Ok(ReadDir::new(inner))
    }
}

pub fn unlink(p: &CStr) -> io::Result<()> {
    cvt(unsafe { libc::unlink(p.as_ptr()) }).map(|_| ())
}

pub fn rename(old: &CStr, new: &CStr) -> io::Result<()> {
    cvt(unsafe { libc::rename(old.as_ptr(), new.as_ptr()) }).map(|_| ())
}

pub fn set_perm(p: &CStr, perm: FilePermissions) -> io::Result<()> {
    cvt_r(|| unsafe { libc::chmod(p.as_ptr(), perm.mode) }).map(|_| ())
}

pub fn rmdir(p: &CStr) -> io::Result<()> {
    cvt(unsafe { libc::rmdir(p.as_ptr()) }).map(|_| ())
}

pub fn readlink(c_path: &CStr) -> io::Result<PathBuf> {
    let p = c_path.as_ptr();

    let mut buf = Vec::with_capacity(256);

    loop {
        let buf_read =
            cvt(unsafe { libc::readlink(p, buf.as_mut_ptr() as *mut _, buf.capacity()) })? as usize;

        unsafe {
            buf.set_len(buf_read);
        }

        if buf_read != buf.capacity() {
            buf.shrink_to_fit();

            return Ok(PathBuf::from(OsString::from_vec(buf)));
        }

        // 通过要求比当前容量更多的空间，来触发 `Vec` 内部的缓冲区扩容逻辑。
        // 由于上面的 if 语句，长度（length）保证与容量（capacity）相同。
        buf.reserve(1);
    }
}

pub fn symlink(original: &CStr, link: &CStr) -> io::Result<()> {
    cvt(unsafe { libc::symlink(original.as_ptr(), link.as_ptr()) }).map(|_| ())
}

pub fn link(original: &CStr, link: &CStr) -> io::Result<()> {
    cfg_select! {
        any(
            // VxWorks、Redox 和 ESP-IDF 缺少 `linkat`，因此改用 `link`。
            // POSIX 把 `link` 是否跟随符号链接（symlinks）留作由实现定义（implementation-defined），
            // 因此依赖 library/std/src/fs/tests.rs 中的 `symlink_hard_link` 测试来检查其行为。
            target_os = "vxworks",
            target_os = "redox",
            target_os = "espidf",
            // Android 在较新版本上有 `linkat`，但我们恰好知道
            // `link` 始终具有正确的行为，所以它也列在这里。
            target_os = "android",
            // wasi-sdk-29 及更早版本的 `linkat` 有 bug，因此在 wasi-sdk 更新之前
            // 改用 `link`（参见 WebAssembly/wasi-libc#690）
            target_os = "wasi",
            // 其他杂项平台
            target_os = "horizon",
            target_os = "vita",
            target_env = "nto70",
        ) => {
            cvt(unsafe { libc::link(original.as_ptr(), link.as_ptr()) })?;
        }
        _ => {
            // 在可以的情况下，使用 `linkat` 而非 `link`；关于原因的细节，
            // 参见上面那条注释。
            cvt(unsafe { libc::linkat(libc::AT_FDCWD, original.as_ptr(), libc::AT_FDCWD, link.as_ptr(), 0) })?;
        }
    }
    Ok(())
}

pub fn stat(p: &CStr) -> io::Result<FileAttr> {
    cfg_has_statx! {
        if let Some(ret) = unsafe { try_statx(
            libc::AT_FDCWD,
            p.as_ptr(),
            libc::AT_STATX_SYNC_AS_STAT,
            libc::STATX_BASIC_STATS | libc::STATX_BTIME,
        ) } {
            return ret;
        }
    }

    let mut stat: stat64 = unsafe { mem::zeroed() };
    cvt(unsafe { stat64(p.as_ptr(), &mut stat) })?;
    Ok(FileAttr::from_stat64(stat))
}

pub fn lstat(p: &CStr) -> io::Result<FileAttr> {
    cfg_has_statx! {
        if let Some(ret) = unsafe { try_statx(
            libc::AT_FDCWD,
            p.as_ptr(),
            libc::AT_SYMLINK_NOFOLLOW | libc::AT_STATX_SYNC_AS_STAT,
            libc::STATX_BASIC_STATS | libc::STATX_BTIME,
        ) } {
            return ret;
        }
    }

    let mut stat: stat64 = unsafe { mem::zeroed() };
    cvt(unsafe { lstat64(p.as_ptr(), &mut stat) })?;
    Ok(FileAttr::from_stat64(stat))
}

pub fn canonicalize(path: &CStr) -> io::Result<PathBuf> {
    let r = unsafe { libc::realpath(path.as_ptr(), ptr::null_mut()) };
    if r.is_null() {
        return Err(io::Error::last_os_error());
    }
    Ok(PathBuf::from(OsString::from_vec(unsafe {
        let buf = CStr::from_ptr(r).to_bytes().to_vec();
        libc::free(r as *mut _);
        buf
    })))
}

fn open_from(from: &Path) -> io::Result<(crate::fs::File, crate::fs::Metadata)> {
    use crate::fs::File;
    use crate::sys::fs::common::NOT_FILE_ERROR;

    let reader = File::open(from)?;
    let metadata = reader.metadata()?;
    if !metadata.is_file() {
        return Err(NOT_FILE_ERROR);
    }
    Ok((reader, metadata))
}

fn set_times_impl(p: &CStr, times: FileTimes, follow_symlinks: bool) -> io::Result<()> {
    cfg_select! {
       any(target_os = "redox", target_os = "espidf", target_os = "horizon", target_os = "nuttx", target_os = "vita", target_os = "rtems") => {
            let _ = (p, times, follow_symlinks);
            Err(io::const_error!(
                io::ErrorKind::Unsupported,
                "setting file times not supported",
            ))
       }
       target_vendor = "apple" => {
            // Apple 平台使用 setattrlist，它支持在符号链接上设置时间
            let ta = TimesAttrlist::from_times(&times)?;
            let options = if follow_symlinks {
                0
            } else {
                libc::FSOPT_NOFOLLOW
            };

            cvt(unsafe { libc::setattrlist(
                p.as_ptr(),
                ta.attrlist(),
                ta.times_buf(),
                ta.times_buf_size(),
                options as u32
            ) })?;
            Ok(())
       }
       target_os = "android" => {
            let times = [file_time_to_timespec(times.accessed)?, file_time_to_timespec(times.modified)?];
            let flags = if follow_symlinks { 0 } else { libc::AT_SYMLINK_NOFOLLOW };
            // utimensat 要求 Android API level 19
            cvt(unsafe {
                weak!(
                    fn utimensat(dirfd: c_int, path: *const libc::c_char, times: *const libc::timespec, flags: c_int) -> c_int;
                );
                match utimensat.get() {
                    Some(utimensat) => utimensat(libc::AT_FDCWD, p.as_ptr(), times.as_ptr(), flags),
                    None => return Err(io::const_error!(
                        io::ErrorKind::Unsupported,
                        "setting file times requires Android API level >= 19",
                    )),
                }
            })?;
            Ok(())
       }
       _ => {
            let flags = if follow_symlinks { 0 } else { libc::AT_SYMLINK_NOFOLLOW };
            #[cfg(all(target_os = "linux", target_env = "gnu", target_pointer_width = "32", not(target_arch = "riscv32")))]
            {
                use crate::sys::{time::__timespec64, weak::weak};

                // 在 glibc 2.34 中添加
                weak!(
                    fn __utimensat64(dirfd: c_int, path: *const c_char, times: *const __timespec64, flags: c_int) -> c_int;
                );

                if let Some(utimensat64) = __utimensat64.get() {
                    let to_timespec = |time: Option<SystemTime>| time.map(|time| time.t.to_timespec64())
                        .unwrap_or(__timespec64::new(0, libc::UTIME_OMIT as _));
                    let times = [to_timespec(times.accessed), to_timespec(times.modified)];
                    cvt(unsafe { utimensat64(libc::AT_FDCWD, p.as_ptr(), times.as_ptr(), flags) })?;
                    return Ok(());
                }
            }
            let times = [file_time_to_timespec(times.accessed)?, file_time_to_timespec(times.modified)?];
            cvt(unsafe { libc::utimensat(libc::AT_FDCWD, p.as_ptr(), times.as_ptr(), flags) })?;
            Ok(())
         }
    }
}

#[inline(always)]
pub fn set_times(p: &CStr, times: FileTimes) -> io::Result<()> {
    set_times_impl(p, times, true)
}

#[inline(always)]
pub fn set_times_nofollow(p: &CStr, times: FileTimes) -> io::Result<()> {
    set_times_impl(p, times, false)
}

#[cfg(any(target_os = "espidf", target_os = "wasi"))]
fn open_to_and_set_permissions(
    to: &Path,
    _reader_metadata: &crate::fs::Metadata,
) -> io::Result<(crate::fs::File, crate::fs::Metadata)> {
    use crate::fs::OpenOptions;
    let writer = OpenOptions::new().write(true).create(true).truncate(true).open(to)?;
    let writer_metadata = writer.metadata()?;
    Ok((writer, writer_metadata))
}

#[cfg(not(any(target_os = "espidf", target_os = "wasi")))]
fn open_to_and_set_permissions(
    to: &Path,
    reader_metadata: &crate::fs::Metadata,
) -> io::Result<(crate::fs::File, crate::fs::Metadata)> {
    use crate::fs::OpenOptions;
    use crate::os::unix::fs::{OpenOptionsExt, PermissionsExt};

    let perm = reader_metadata.permissions();
    let writer = OpenOptions::new()
        // 立即以正确的 mode 创建文件
        .mode(perm.mode())
        .write(true)
        .create(true)
        .truncate(true)
        .open(to)?;
    let writer_metadata = writer.metadata()?;
    // fchmod 在 vita 上有问题
    #[cfg(not(target_os = "vita"))]
    if writer_metadata.is_file() {
        // 设置正确的文件权限，以防该文件已经存在。
        // 不要对已存在的非普通文件（如管道/FIFO 或设备节点）设置权限。
        writer.set_permissions(perm)?;
    }
    Ok((writer, writer_metadata))
}

mod cfm {
    use crate::fs::{File, Metadata};
    use crate::io::{BorrowedCursor, IoSlice, IoSliceMut, Read, Result, Write};

    #[allow(dead_code)]
    pub struct CachedFileMetadata(pub File, pub Metadata);

    impl Read for CachedFileMetadata {
        fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
            self.0.read(buf)
        }
        fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> Result<usize> {
            self.0.read_vectored(bufs)
        }
        fn read_buf(&mut self, cursor: BorrowedCursor<'_>) -> Result<()> {
            self.0.read_buf(cursor)
        }
        #[inline]
        fn is_read_vectored(&self) -> bool {
            self.0.is_read_vectored()
        }
        fn read_to_end(&mut self, buf: &mut Vec<u8>) -> Result<usize> {
            self.0.read_to_end(buf)
        }
        fn read_to_string(&mut self, buf: &mut String) -> Result<usize> {
            self.0.read_to_string(buf)
        }
    }
    impl Write for CachedFileMetadata {
        fn write(&mut self, buf: &[u8]) -> Result<usize> {
            self.0.write(buf)
        }
        fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> Result<usize> {
            self.0.write_vectored(bufs)
        }
        #[inline]
        fn is_write_vectored(&self) -> bool {
            self.0.is_write_vectored()
        }
        #[inline]
        fn flush(&mut self) -> Result<()> {
            self.0.flush()
        }
    }
}
#[cfg(any(target_os = "linux", target_os = "android"))]
pub(in crate::sys) use cfm::CachedFileMetadata;

#[cfg(not(target_vendor = "apple"))]
pub fn copy(from: &Path, to: &Path) -> io::Result<u64> {
    let (reader, reader_metadata) = open_from(from)?;
    let (writer, writer_metadata) = open_to_and_set_permissions(to, &reader_metadata)?;

    io::copy(
        &mut cfm::CachedFileMetadata(reader, reader_metadata),
        &mut cfm::CachedFileMetadata(writer, writer_metadata),
    )
}

#[cfg(target_vendor = "apple")]
pub fn copy(from: &Path, to: &Path) -> io::Result<u64> {
    const COPYFILE_ALL: libc::copyfile_flags_t = libc::COPYFILE_METADATA | libc::COPYFILE_DATA;

    struct FreeOnDrop(libc::copyfile_state_t);
    impl Drop for FreeOnDrop {
        fn drop(&mut self) {
            // 下面的代码确保 `FreeOnDrop` 永远不会是空指针
            unsafe {
                // 如果 `to` 或 `from` 文件无法被关闭，`copyfile_state_free` 会返回 -1。
                // 然而，这并不被视为一个错误。
                libc::copyfile_state_free(self.0);
            }
        }
    }

    let (reader, reader_metadata) = open_from(from)?;

    let clonefile_result = run_path_with_cstr(to, &|to| {
        cvt(unsafe { libc::fclonefileat(reader.as_raw_fd(), libc::AT_FDCWD, to.as_ptr(), 0) })
    });
    match clonefile_result {
        Ok(_) => return Ok(reader_metadata.len()),
        Err(e) => match e.raw_os_error() {
            // 在以下情况下，`fclonefileat` 会失败：卷不是 APFS、目标已存在，
            // 或者源和目标位于不同的设备上。在所有这些情况下，`fcopyfile`
            // 应当能够成功。
            Some(libc::ENOTSUP) | Some(libc::EEXIST) | Some(libc::EXDEV) => (),
            _ => return Err(e),
        },
    }

    // 如果 `fclonefileat` 未能成功，则回退到使用 `fcopyfile`。
    let (writer, writer_metadata) = open_to_and_set_permissions(to, &reader_metadata)?;

    // 我们确保 `FreeOnDrop` 永远不会包含空指针，因此调用
    // `copyfile_state_free` 始终是安全的
    let state = unsafe {
        let state = libc::copyfile_state_alloc();
        if state.is_null() {
            return Err(crate::io::Error::last_os_error());
        }
        FreeOnDrop(state)
    };

    let flags = if writer_metadata.is_file() { COPYFILE_ALL } else { libc::COPYFILE_DATA };

    cvt(unsafe { libc::fcopyfile(reader.as_raw_fd(), writer.as_raw_fd(), state.0, flags) })?;

    let mut bytes_copied: libc::off_t = 0;
    cvt(unsafe {
        libc::copyfile_state_get(
            state.0,
            libc::COPYFILE_STATE_COPIED as u32,
            (&raw mut bytes_copied) as *mut libc::c_void,
        )
    })?;
    Ok(bytes_copied as u64)
}

#[cfg(not(target_os = "wasi"))]
pub fn chown(path: &Path, uid: u32, gid: u32) -> io::Result<()> {
    run_path_with_cstr(path, &|path| {
        cvt(unsafe { libc::chown(path.as_ptr(), uid as libc::uid_t, gid as libc::gid_t) })
            .map(|_| ())
    })
}

#[cfg(not(target_os = "wasi"))]
pub fn fchown(fd: c_int, uid: u32, gid: u32) -> io::Result<()> {
    cvt(unsafe { libc::fchown(fd, uid as libc::uid_t, gid as libc::gid_t) })?;
    Ok(())
}

#[cfg(not(any(target_os = "vxworks", target_os = "wasi")))]
pub fn lchown(path: &Path, uid: u32, gid: u32) -> io::Result<()> {
    run_path_with_cstr(path, &|path| {
        cvt(unsafe { libc::lchown(path.as_ptr(), uid as libc::uid_t, gid as libc::gid_t) })
            .map(|_| ())
    })
}

#[cfg(target_os = "vxworks")]
pub fn lchown(path: &Path, uid: u32, gid: u32) -> io::Result<()> {
    let (_, _, _) = (path, uid, gid);
    Err(io::const_error!(io::ErrorKind::Unsupported, "lchown not supported by vxworks"))
}

#[cfg(not(any(target_os = "fuchsia", target_os = "vxworks", target_os = "wasi")))]
pub fn chroot(dir: &Path) -> io::Result<()> {
    run_path_with_cstr(dir, &|dir| cvt(unsafe { libc::chroot(dir.as_ptr()) }).map(|_| ()))
}

#[cfg(target_os = "vxworks")]
pub fn chroot(dir: &Path) -> io::Result<()> {
    let _ = dir;
    Err(io::const_error!(io::ErrorKind::Unsupported, "chroot not supported by vxworks"))
}

#[cfg(not(target_os = "wasi"))]
pub fn mkfifo(path: &Path, mode: u32) -> io::Result<()> {
    run_path_with_cstr(path, &|path| {
        cvt(unsafe { libc::mkfifo(path.as_ptr(), mode.try_into().unwrap()) }).map(|_| ())
    })
}

pub use remove_dir_impl::remove_dir_all;

// 针对 REDOX、ESP-ID、Horizon、Vita、Vxworks 和 Miri 的回退实现
#[cfg(any(
    target_os = "redox",
    target_os = "espidf",
    target_os = "horizon",
    target_os = "vita",
    target_os = "nto",
    target_os = "vxworks",
    miri
))]
mod remove_dir_impl {
    pub use crate::sys::fs::common::remove_dir_all;
}

// 使用 openat()、unlinkat() 和 fdopendir() 的现代实现
#[cfg(not(any(
    target_os = "redox",
    target_os = "espidf",
    target_os = "horizon",
    target_os = "vita",
    target_os = "nto",
    target_os = "vxworks",
    miri
)))]
mod remove_dir_impl {
    #[cfg(not(all(target_os = "linux", target_env = "gnu")))]
    use libc::{fdopendir, openat, unlinkat};
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    use libc::{fdopendir, openat64 as openat, unlinkat};

    use super::{
        AsRawFd, DirEntry, DirStream, FromRawFd, InnerReadDir, IntoRawFd, OwnedFd, RawFd, ReadDir,
        lstat,
    };
    use crate::ffi::CStr;
    use crate::io;
    use crate::path::{Path, PathBuf};
    use crate::sys::helpers::{ignore_notfound, run_path_with_cstr};
    use crate::sys::{cvt, cvt_r};

    pub fn openat_nofollow_dironly(parent_fd: Option<RawFd>, p: &CStr) -> io::Result<OwnedFd> {
        let fd = cvt_r(|| unsafe {
            openat(
                parent_fd.unwrap_or(libc::AT_FDCWD),
                p.as_ptr(),
                libc::O_CLOEXEC | libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_DIRECTORY,
            )
        })?;
        Ok(unsafe { OwnedFd::from_raw_fd(fd) })
    }

    fn fdreaddir(dir_fd: OwnedFd) -> io::Result<(ReadDir, RawFd)> {
        let ptr = unsafe { fdopendir(dir_fd.as_raw_fd()) };
        if ptr.is_null() {
            return Err(io::Error::last_os_error());
        }
        let dirp = DirStream(ptr);
        // 现在文件描述符会由 libc::closedir() 自动关闭，因此放弃其所有权（ownership）
        let new_parent_fd = dir_fd.into_raw_fd();
        // 不需要一个有效的 root，因为我们不会调用任何涉及 `DirEntry` 完整路径的函数。
        let dummy_root = PathBuf::new();
        let inner = InnerReadDir { dirp, root: dummy_root };
        Ok((ReadDir::new(inner), new_parent_fd))
    }

    #[cfg(any(
        target_os = "solaris",
        target_os = "illumos",
        target_os = "haiku",
        target_os = "vxworks",
        target_os = "aix",
    ))]
    fn is_dir(_ent: &DirEntry) -> Option<bool> {
        None
    }

    #[cfg(not(any(
        target_os = "solaris",
        target_os = "illumos",
        target_os = "haiku",
        target_os = "vxworks",
        target_os = "aix",
    )))]
    fn is_dir(ent: &DirEntry) -> Option<bool> {
        match ent.entry.d_type {
            libc::DT_UNKNOWN => None,
            libc::DT_DIR => Some(true),
            _ => Some(false),
        }
    }

    fn is_enoent(result: &io::Result<()>) -> bool {
        if let Err(err) = result
            && matches!(err.raw_os_error(), Some(libc::ENOENT))
        {
            true
        } else {
            false
        }
    }

    fn remove_dir_all_recursive(parent_fd: Option<RawFd>, path: &CStr) -> io::Result<()> {
        // 尝试以目录方式打开
        let fd = match openat_nofollow_dironly(parent_fd, &path) {
            Err(err) if matches!(err.raw_os_error(), Some(libc::ENOTDIR | libc::ELOOP)) => {
                // 不是目录——不再继续向下遍历
                //（对于符号链接，较早版本的 Linux 内核可能返回 ELOOP 而非 ENOTDIR）
                return match parent_fd {
                    // unlink……
                    Some(parent_fd) => {
                        cvt(unsafe { unlinkat(parent_fd, path.as_ptr(), 0) }).map(drop)
                    }
                    // ……除非这本应是删除操作的根目录
                    None => Err(err),
                };
            }
            result => result?,
        };

        // 打开该目录，并将 fd 的所有权传递出去
        let (dir, fd) = fdreaddir(fd)?;

        // 对于 WASI，会先读取该目录的所有目录条目，然后才进行任何删除操作。
        // 这是为了绕开如下事实：WASIp1 用于读取目录的 API 在设计上不擅长处理
        // 多次读取目录调用之间的改动（mutations）。通过一次性读取所有条目，
        // 这确保了——至少在没有并发修改的情况下——应当能够删除所有内容。
        #[cfg(target_os = "wasi")]
        let dir = dir.collect::<Vec<_>>();

        for child in dir {
            let child = child?;
            let child_name = child.name_cstr();
            // 我们需要一个内层 try 块，因为如果这些目录中有一个已经被删除了，
            // 那么我们需要继续循环，而不是返回 ok。
            let result: io::Result<()> = try {
                match is_dir(&child) {
                    Some(true) => {
                        remove_dir_all_recursive(Some(fd), child_name)?;
                    }
                    Some(false) => {
                        cvt(unsafe { unlinkat(fd, child_name.as_ptr(), 0) })?;
                    }
                    None => {
                        // POSIX 规定，如果进程拥有相应的权限，对一个目录调用
                        // unlink()/unlinkat(..., 0) 是可以成功的。然而这可能导致
                        // 孤立（orphaned）目录、需要执行 fsck，例如在 Solaris 和 Illumos 上。
                        // 所以我们先尝试递归进入它，而不是尝试 unlink() 它。
                        remove_dir_all_recursive(Some(fd), child_name)?;
                    }
                }
            };
            if result.is_err() && !is_enoent(&result) {
                return result;
            }
        }

        // 在删除目录的内容之后，再 unlink 该目录本身
        ignore_notfound(cvt(unsafe {
            unlinkat(parent_fd.unwrap_or(libc::AT_FDCWD), path.as_ptr(), libc::AT_REMOVEDIR)
        }))?;
        Ok(())
    }

    fn remove_dir_all_modern(p: &CStr) -> io::Result<()> {
        // 这里我们不能直接调用 remove_dir_all_recursive()，因为那样不会删除传入的
        // 符号链接。无需担心竞态（races），因为 remove_dir_all_recursive() 不会
        // 递归进入符号链接。
        let attr = lstat(p)?;
        if attr.file_type().is_symlink() {
            super::unlink(p)
        } else {
            remove_dir_all_recursive(None, &p)
        }
    }

    pub fn remove_dir_all(p: &Path) -> io::Result<()> {
        run_path_with_cstr(p, &remove_dir_all_modern)
    }
}
