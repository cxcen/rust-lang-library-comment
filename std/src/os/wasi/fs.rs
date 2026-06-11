//! 针对 [`std::fs`] 模块中各类基础类型的 WASIp1 平台特定扩展。
//!
//! [`std::fs`]: crate::fs

#![unstable(feature = "wasi_ext", issue = "71213")]

// 用于文档内链接中对 `File::read` 的引用
#[allow(unused_imports)]
use io::{Read, Write};

#[cfg(target_env = "p1")]
use crate::ffi::OsStr;
use crate::fs::{self, File, OpenOptions};
use crate::io::{self, BorrowedCursor, IoSlice, IoSliceMut};
#[cfg(target_env = "p1")]
use crate::os::fd::AsRawFd;
use crate::path::Path;
#[cfg(target_env = "p1")]
use crate::sys::err2io;
use crate::sys::{AsInner, AsInnerMut};

/// 针对 [`File`] 的 WASI 平台特定扩展。
pub trait FileExt {
    /// 从给定偏移量开始读取若干字节。
    ///
    /// 返回读取到的字节数。
    ///
    /// 偏移量相对于文件起始位置，因此与当前游标位置无关。
    ///
    /// 本函数不会影响当前的文件游标位置。
    ///
    /// 注意，与 [`File::read`] 类似，返回一次短读（读取字节数少于请求）并不算错误。
    fn read_at(&self, buf: &mut [u8], offset: u64) -> io::Result<usize>;

    /// 从给定偏移量开始读取若干字节。
    ///
    /// 返回读取到的字节数。
    ///
    /// 偏移量相对于文件起始位置，因此与当前游标位置无关。
    ///
    /// 本函数不会影响当前的文件游标位置。
    ///
    /// 注意，与 [`File::read_vectored`] 类似，返回一次短读并不算错误。
    fn read_vectored_at(&self, bufs: &mut [IoSliceMut<'_>], offset: u64) -> io::Result<usize>;

    /// 从给定偏移量开始读取若干字节到缓冲区中。
    ///
    /// 这等价于 [`read_at`](FileExt::read_at) 方法，区别在于它接收一个
    /// [`BorrowedCursor`] 而非 `&mut [u8]`，以便能用于未初始化的缓冲区。新读取的
    /// 数据会追加到 `buf` 已有内容之后。
    fn read_buf_at(&self, buf: BorrowedCursor<'_>, offset: u64) -> io::Result<()>;

    /// 从给定偏移量开始读取恰好填满 `buf` 所需的字节数。
    ///
    /// 偏移量相对于文件起始位置，因此与当前游标位置无关。
    ///
    /// 本函数不会影响当前的文件游标位置。
    ///
    /// 与 [`Read::read_exact`] 类似，但使用 [`read_at`] 而非 `read`。
    ///
    /// [`read_at`]: FileExt::read_at
    ///
    /// # Errors
    ///
    /// 如果本函数遇到 [`io::ErrorKind::Interrupted`] 类型的错误，则忽略该错误并继续操作。
    ///
    /// 如果本函数在完全填满缓冲区之前遇到“文件结束”，则返回
    /// [`io::ErrorKind::UnexpectedEof`] 类型的错误。此时 `buf` 的内容是未指定的。
    ///
    /// 如果遇到任何其他读取错误，本函数会立即返回。此时 `buf` 的内容是未指定的。
    ///
    /// 如果本函数返回错误，则它已读取的字节数是未指定的，但它读取的字节数绝不会超过
    /// 完全填满缓冲区所需的数量。
    fn read_exact_at(&self, mut buf: &mut [u8], mut offset: u64) -> io::Result<()> {
        while !buf.is_empty() {
            match self.read_at(buf, offset) {
                Ok(0) => break,
                Ok(n) => {
                    let tmp = buf;
                    buf = &mut tmp[n..];
                    offset += n as u64;
                }
                Err(ref e) if e.is_interrupted() => {}
                Err(e) => return Err(e),
            }
        }
        if !buf.is_empty() { Err(io::Error::READ_EXACT_EOF) } else { Ok(()) }
    }

    /// 从给定偏移量开始写入若干字节。
    ///
    /// 返回写入的字节数。
    ///
    /// 偏移量相对于文件起始位置，因此与当前游标位置无关。
    ///
    /// 本函数不会影响当前的文件游标位置。
    ///
    /// 当写入超出文件末尾时，文件会被相应地扩展，中间的字节以数值 0 初始化。
    ///
    /// 注意，与 [`File::write`] 类似，返回一次短写并不算错误。
    fn write_at(&self, buf: &[u8], offset: u64) -> io::Result<usize>;

    /// 从给定偏移量开始写入若干字节。
    ///
    /// 返回写入的字节数。
    ///
    /// 偏移量相对于文件起始位置，因此与当前游标位置无关。
    ///
    /// 本函数不会影响当前的文件游标位置。
    ///
    /// 当写入超出文件末尾时，文件会被相应地扩展，中间的字节以数值 0 初始化。
    ///
    /// 注意，与 [`File::write_vectored`] 类似，返回一次短写并不算错误。
    fn write_vectored_at(&self, bufs: &[IoSlice<'_>], offset: u64) -> io::Result<usize>;

    /// 尝试从给定偏移量开始写入整个缓冲区。
    ///
    /// 偏移量相对于文件起始位置，因此与当前游标位置无关。
    ///
    /// 本函数不会影响当前的文件游标位置。
    ///
    /// 本方法会持续调用 [`write_at`]，直到没有更多数据需要写入，或返回了一个
    /// 非 [`io::ErrorKind::Interrupted`] 类型的错误。本方法在整个缓冲区成功写入
    /// 或发生此类错误之前不会返回。本方法产生的第一个非 [`io::ErrorKind::Interrupted`]
    /// 类型的错误将被返回。
    ///
    /// # Errors
    ///
    /// 本函数将返回 [`write_at`] 产生的第一个非 [`io::ErrorKind::Interrupted`] 类型的错误。
    ///
    /// [`write_at`]: FileExt::write_at
    fn write_all_at(&self, mut buf: &[u8], mut offset: u64) -> io::Result<()> {
        while !buf.is_empty() {
            match self.write_at(buf, offset) {
                Ok(0) => {
                    return Err(io::Error::WRITE_ALL_EOF);
                }
                Ok(n) => {
                    buf = &buf[n..];
                    offset += n as u64
                }
                Err(ref e) if e.is_interrupted() => {}
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    /// 调整与此文件关联的标志位。
    ///
    /// 对应 `fd_fdstat_set_flags` 系统调用。
    #[doc(alias = "fd_fdstat_set_flags")]
    #[cfg(target_env = "p1")]
    fn fdstat_set_flags(&self, flags: u16) -> io::Result<()>;

    /// 调整与此文件关联的权限（rights）。
    ///
    /// 对应 `fd_fdstat_set_rights` 系统调用。
    #[doc(alias = "fd_fdstat_set_rights")]
    #[cfg(target_env = "p1")]
    fn fdstat_set_rights(&self, rights: u64, inheriting: u64) -> io::Result<()>;

    /// 为文件描述符提供文件的访问建议信息（advisory information）。
    ///
    /// 对应 `fd_advise` 系统调用。
    #[doc(alias = "fd_advise")]
    #[cfg(target_env = "p1")]
    fn advise(&self, offset: u64, len: u64, advice: u8) -> io::Result<()>;

    /// 强制为文件分配空间。
    ///
    /// 对应 `fd_allocate` 系统调用。
    #[doc(alias = "fd_allocate")]
    #[cfg(target_env = "p1")]
    fn allocate(&self, offset: u64, len: u64) -> io::Result<()>;

    /// 创建一个目录。
    ///
    /// 对应 `path_create_directory` 系统调用。
    #[doc(alias = "path_create_directory")]
    #[cfg(target_env = "p1")]
    fn create_directory<P: AsRef<Path>>(&self, dir: P) -> io::Result<()>;

    /// 取消链接（删除）一个文件。
    ///
    /// 对应 `path_unlink_file` 系统调用。
    #[doc(alias = "path_unlink_file")]
    #[cfg(target_env = "p1")]
    fn remove_file<P: AsRef<Path>>(&self, path: P) -> io::Result<()>;

    /// 删除一个目录。
    ///
    /// 对应 `path_remove_directory` 系统调用。
    #[doc(alias = "path_remove_directory")]
    #[cfg(target_env = "p1")]
    fn remove_directory<P: AsRef<Path>>(&self, path: P) -> io::Result<()>;
}

// FIXME: 绑定 fd_fdstat_get —— 需要定义一个自定义的返回类型
// FIXME: 绑定 fd_readdir —— 无法返回 `ReadDir`，因为我们只有条目名称
// FIXME: 也许绑定 fd_filestat_set_times？—— crates.io 上已有针对 unix 的实现
// FIXME: 也许绑定 path_filestat_set_times？—— crates.io 上已有针对 unix 的实现
// FIXME: 也许绑定 poll_oneoff？—— 大概应该等待 I/O 稳定下来
// FIXME: 也许绑定 random_get？—— crates.io 上已有针对 unix 的实现

impl FileExt for File {
    fn read_at(&self, buf: &mut [u8], offset: u64) -> io::Result<usize> {
        self.as_inner().read_at(buf, offset)
    }

    fn read_buf_at(&self, buf: BorrowedCursor<'_>, offset: u64) -> io::Result<()> {
        self.as_inner().read_buf_at(buf, offset)
    }

    fn read_vectored_at(&self, bufs: &mut [IoSliceMut<'_>], offset: u64) -> io::Result<usize> {
        self.as_inner().read_vectored_at(bufs, offset)
    }

    fn write_at(&self, buf: &[u8], offset: u64) -> io::Result<usize> {
        self.as_inner().write_at(buf, offset)
    }

    fn write_vectored_at(&self, bufs: &[IoSlice<'_>], offset: u64) -> io::Result<usize> {
        self.as_inner().write_vectored_at(bufs, offset)
    }

    #[cfg(target_env = "p1")]
    fn fdstat_set_flags(&self, flags: u16) -> io::Result<()> {
        unsafe { wasi::fd_fdstat_set_flags(self.as_raw_fd() as wasi::Fd, flags).map_err(err2io) }
    }

    #[cfg(target_env = "p1")]
    fn fdstat_set_rights(&self, rights: u64, inheriting: u64) -> io::Result<()> {
        unsafe {
            wasi::fd_fdstat_set_rights(self.as_raw_fd() as wasi::Fd, rights, inheriting)
                .map_err(err2io)
        }
    }

    #[cfg(target_env = "p1")]
    fn advise(&self, offset: u64, len: u64, advice: u8) -> io::Result<()> {
        let advice = match advice {
            a if a == wasi::ADVICE_NORMAL.raw() => wasi::ADVICE_NORMAL,
            a if a == wasi::ADVICE_SEQUENTIAL.raw() => wasi::ADVICE_SEQUENTIAL,
            a if a == wasi::ADVICE_RANDOM.raw() => wasi::ADVICE_RANDOM,
            a if a == wasi::ADVICE_WILLNEED.raw() => wasi::ADVICE_WILLNEED,
            a if a == wasi::ADVICE_DONTNEED.raw() => wasi::ADVICE_DONTNEED,
            a if a == wasi::ADVICE_NOREUSE.raw() => wasi::ADVICE_NOREUSE,
            _ => {
                return Err(io::const_error!(
                    io::ErrorKind::InvalidInput,
                    "invalid parameter 'advice'",
                ));
            }
        };

        unsafe {
            wasi::fd_advise(self.as_raw_fd() as wasi::Fd, offset, len, advice).map_err(err2io)
        }
    }

    #[cfg(target_env = "p1")]
    fn allocate(&self, offset: u64, len: u64) -> io::Result<()> {
        unsafe { wasi::fd_allocate(self.as_raw_fd() as wasi::Fd, offset, len).map_err(err2io) }
    }

    #[cfg(target_env = "p1")]
    fn create_directory<P: AsRef<Path>>(&self, dir: P) -> io::Result<()> {
        let path = osstr2str(dir.as_ref().as_ref())?;
        unsafe { wasi::path_create_directory(self.as_raw_fd() as wasi::Fd, path).map_err(err2io) }
    }

    #[cfg(target_env = "p1")]
    fn remove_file<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
        let path = osstr2str(path.as_ref().as_ref())?;
        unsafe { wasi::path_unlink_file(self.as_raw_fd() as wasi::Fd, path).map_err(err2io) }
    }

    #[cfg(target_env = "p1")]
    fn remove_directory<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
        let path = osstr2str(path.as_ref().as_ref())?;
        unsafe { wasi::path_remove_directory(self.as_raw_fd() as wasi::Fd, path).map_err(err2io) }
    }
}

/// 针对 [`OpenOptions`] 的 WASI 平台特定扩展。
pub trait OpenOptionsExt {
    /// 向 `open` 的 `flags` 参数传入自定义标志位。
    fn custom_flags(&mut self, flags: i32) -> &mut Self;
}

impl OpenOptionsExt for OpenOptions {
    fn custom_flags(&mut self, flags: i32) -> &mut OpenOptions {
        self.as_inner_mut().custom_flags(flags);
        self
    }
}

/// 针对 [`fs::Metadata`] 的 WASI 平台特定扩展。
pub trait MetadataExt {
    /// 返回内部 `filestat_t` 的 `st_dev` 字段
    fn dev(&self) -> u64;
    /// 返回内部 `filestat_t` 的 `st_ino` 字段
    fn ino(&self) -> u64;
    /// 返回内部 `filestat_t` 的 `st_nlink` 字段
    fn nlink(&self) -> u64;
}

impl MetadataExt for fs::Metadata {
    fn dev(&self) -> u64 {
        self.as_inner().as_inner().st_dev
    }
    fn ino(&self) -> u64 {
        self.as_inner().as_inner().st_ino
    }
    fn nlink(&self) -> u64 {
        self.as_inner().as_inner().st_nlink
    }
}

/// 针对 [`fs::FileType`] 的 WASI 平台特定扩展。
///
/// 增加了对 WASI 特殊文件类型的支持，例如块设备/字符设备、管道和套接字。
pub trait FileTypeExt {
    /// 如果此文件类型是块设备，则返回 `true`。
    fn is_block_device(&self) -> bool;
    /// 如果此文件类型是字符设备，则返回 `true`。
    fn is_char_device(&self) -> bool;
    /// 如果此文件类型是任意类型的套接字，则返回 `true`。
    fn is_socket(&self) -> bool;
}

impl FileTypeExt for fs::FileType {
    fn is_block_device(&self) -> bool {
        self.as_inner().is(libc::S_IFBLK)
    }
    fn is_char_device(&self) -> bool {
        self.as_inner().is(libc::S_IFCHR)
    }
    fn is_socket(&self) -> bool {
        self.as_inner().is(libc::S_IFSOCK)
    }
}

/// 针对 [`fs::DirEntry`] 的 WASI 平台特定扩展方法。
pub trait DirEntryExt {
    /// 返回底层 `dirent_t` 的 `d_ino` 字段
    fn ino(&self) -> u64;
}

impl DirEntryExt for fs::DirEntry {
    fn ino(&self) -> u64 {
        self.as_inner().ino()
    }
}

/// 创建一个硬链接。
///
/// 对应 `path_link` 系统调用。
#[doc(alias = "path_link")]
#[cfg(target_env = "p1")]
pub fn link<P: AsRef<Path>, U: AsRef<Path>>(
    old_fd: &File,
    old_flags: u32,
    old_path: P,
    new_fd: &File,
    new_path: U,
) -> io::Result<()> {
    unsafe {
        wasi::path_link(
            old_fd.as_raw_fd() as wasi::Fd,
            old_flags,
            osstr2str(old_path.as_ref().as_ref())?,
            new_fd.as_raw_fd() as wasi::Fd,
            osstr2str(new_path.as_ref().as_ref())?,
        )
        .map_err(err2io)
    }
}

/// 重命名一个文件或目录。
///
/// 对应 `path_rename` 系统调用。
#[doc(alias = "path_rename")]
#[cfg(target_env = "p1")]
pub fn rename<P: AsRef<Path>, U: AsRef<Path>>(
    old_fd: &File,
    old_path: P,
    new_fd: &File,
    new_path: U,
) -> io::Result<()> {
    unsafe {
        wasi::path_rename(
            old_fd.as_raw_fd() as wasi::Fd,
            osstr2str(old_path.as_ref().as_ref())?,
            new_fd.as_raw_fd() as wasi::Fd,
            osstr2str(new_path.as_ref().as_ref())?,
        )
        .map_err(err2io)
    }
}

/// 创建一个符号链接。
///
/// 对应 `path_symlink` 系统调用。
#[doc(alias = "path_symlink")]
#[cfg(target_env = "p1")]
pub fn symlink<P: AsRef<Path>, U: AsRef<Path>>(
    old_path: P,
    fd: &File,
    new_path: U,
) -> io::Result<()> {
    unsafe {
        wasi::path_symlink(
            osstr2str(old_path.as_ref().as_ref())?,
            fd.as_raw_fd() as wasi::Fd,
            osstr2str(new_path.as_ref().as_ref())?,
        )
        .map_err(err2io)
    }
}

/// 创建一个符号链接。
///
/// 这是一个便捷 API，类似于 `std::os::unix::fs::symlink`、
/// `std::os::windows::fs::symlink_file` 以及 `std::os::windows::fs::symlink_dir`。
pub fn symlink_path<P: AsRef<Path>, U: AsRef<Path>>(old_path: P, new_path: U) -> io::Result<()> {
    crate::sys::fs::symlink(old_path.as_ref(), new_path.as_ref())
}

#[cfg(target_env = "p1")]
fn osstr2str(f: &OsStr) -> io::Result<&str> {
    f.to_str().ok_or_else(|| io::const_error!(io::ErrorKind::Uncategorized, "input must be utf-8"))
}
