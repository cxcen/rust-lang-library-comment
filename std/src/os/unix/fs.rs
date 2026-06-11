//! 针对 [`std::fs`] 模块中各基础类型的 Unix 特有扩展。
//!
//! [`std::fs`]: crate::fs

#![stable(feature = "rust1", since = "1.0.0")]

#[allow(unused_imports)]
use io::{Read, Write};

use super::platform::fs::MetadataExt as _;
// 用于文档内链接中的 `File::read`
use crate::ffi::OsStr;
use crate::fs::{self, OpenOptions, Permissions};
use crate::io::BorrowedCursor;
use crate::os::unix::io::{AsFd, AsRawFd};
use crate::path::Path;
use crate::sealed::Sealed;
use crate::sys::{AsInner, AsInnerMut, FromInner};
use crate::{io, sys};

// 本模块的测试
#[cfg(test)]
mod tests;

/// 针对 [`fs::File`] 的 Unix 特有扩展。
#[stable(feature = "file_offset", since = "1.15.0")]
pub trait FileExt {
    /// 从给定偏移量开始读取若干字节。
    ///
    /// 返回读取的字节数。
    ///
    /// 偏移量相对于文件起始处，因此与当前游标（cursor）无关。
    ///
    /// 当前文件游标不受此函数影响。
    ///
    /// 注意，与 [`File::read`] 类似，发生短读（short read）并返回不算错误。
    ///
    /// [`File::read`]: fs::File::read
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::io;
    /// use std::fs::File;
    /// use std::os::unix::prelude::FileExt;
    ///
    /// fn main() -> io::Result<()> {
    ///     let mut buf = [0u8; 8];
    ///     let file = File::open("foo.txt")?;
    ///
    ///     // 我们现在从偏移量 10 处读取 8 个字节。
    ///     let num_bytes_read = file.read_at(&mut buf, 10)?;
    ///     println!("read {num_bytes_read} bytes: {buf:?}");
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "file_offset", since = "1.15.0")]
    fn read_at(&self, buf: &mut [u8], offset: u64) -> io::Result<usize>;

    /// 与 `read_at` 类似，但它读取到一个缓冲区切片中。
    ///
    /// 数据被依次复制以填充每个缓冲区，最后一个被写入的缓冲区可能只被部分填充。
    /// 此方法的行为必须等同于对拼接后的缓冲区进行一次 read 调用。
    #[unstable(feature = "unix_file_vectored_at", issue = "89517")]
    fn read_vectored_at(&self, bufs: &mut [io::IoSliceMut<'_>], offset: u64) -> io::Result<usize> {
        io::default_read_vectored(|b| self.read_at(b, offset), bufs)
    }

    /// 从给定偏移量开始读取恰好填满 `buf` 所需数量的字节。
    ///
    /// 偏移量相对于文件起始处，因此与当前游标（cursor）无关。
    ///
    /// 当前文件游标不受此函数影响。
    ///
    /// 与 [`io::Read::read_exact`] 类似，但使用 [`read_at`] 而非 `read`。
    ///
    /// [`read_at`]: FileExt::read_at
    ///
    /// # Errors
    ///
    /// 如果此函数遇到类型为 [`io::ErrorKind::Interrupted`] 的错误，则该错误会被忽略，
    /// 操作将继续。
    ///
    /// 如果此函数在完全填满缓冲区之前遇到“文件结束（end of file）”，它会返回一个
    /// 类型为 [`io::ErrorKind::UnexpectedEof`] 的错误。此情况下 `buf` 的内容未指定。
    ///
    /// 如果遇到任何其他读取错误，此函数会立即返回。此情况下 `buf` 的内容未指定。
    ///
    /// 如果此函数返回错误，它已读取了多少字节是未指定的，但它绝不会读取超过完全填满
    /// 缓冲区所需的字节数。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::io;
    /// use std::fs::File;
    /// use std::os::unix::prelude::FileExt;
    ///
    /// fn main() -> io::Result<()> {
    ///     let mut buf = [0u8; 8];
    ///     let file = File::open("foo.txt")?;
    ///
    ///     // 我们现在从偏移量 10 处恰好读取 8 个字节。
    ///     file.read_exact_at(&mut buf, 10)?;
    ///     println!("read {} bytes: {:?}", buf.len(), buf);
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "rw_exact_all_at", since = "1.33.0")]
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

    /// 从给定偏移量开始读取一些字节到缓冲区中。
    ///
    /// 这等价于 [`read_at`](FileExt::read_at) 方法，区别在于它接收一个
    /// [`BorrowedCursor`] 而非 `&mut [u8]`，以允许配合未初始化的缓冲区使用。新数据
    /// 将被追加到 `buf` 已有的任何内容之后。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// #![feature(core_io_borrowed_buf)]
    /// #![feature(read_buf_at)]
    ///
    /// use std::io;
    /// use std::io::BorrowedBuf;
    /// use std::fs::File;
    /// use std::mem::MaybeUninit;
    /// use std::os::unix::prelude::*;
    ///
    /// fn main() -> io::Result<()> {
    ///     let mut file = File::open("pi.txt")?;
    ///
    ///     // 从偏移量 2 开始读取一些字节
    ///     let mut buf: [MaybeUninit<u8>; 10] = [MaybeUninit::uninit(); 10];
    ///     let mut buf = BorrowedBuf::from(buf.as_mut_slice());
    ///     file.read_buf_at(buf.unfilled(), 2)?;
    ///
    ///     assert!(buf.filled().starts_with(b"1"));
    ///
    ///     Ok(())
    /// }
    /// ```
    #[unstable(feature = "read_buf_at", issue = "140771")]
    fn read_buf_at(&self, buf: BorrowedCursor<'_>, offset: u64) -> io::Result<()> {
        io::default_read_buf(|b| self.read_at(b, offset), buf)
    }

    /// 从给定偏移量开始读取恰好填满缓冲区所需数量的字节。
    ///
    /// 这等价于 [`read_exact_at`](FileExt::read_exact_at) 方法，区别在于它接收一个
    /// [`BorrowedCursor`] 而非 `&mut [u8]`，以允许配合未初始化的缓冲区使用。新数据
    /// 将被追加到 `buf` 已有的任何内容之后。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// #![feature(core_io_borrowed_buf)]
    /// #![feature(read_buf_at)]
    ///
    /// use std::io;
    /// use std::io::BorrowedBuf;
    /// use std::fs::File;
    /// use std::mem::MaybeUninit;
    /// use std::os::unix::prelude::*;
    ///
    /// fn main() -> io::Result<()> {
    ///     let mut file = File::open("pi.txt")?;
    ///
    ///     // 从偏移量 2 开始恰好读取 10 个字节
    ///     let mut buf: [MaybeUninit<u8>; 10] = [MaybeUninit::uninit(); 10];
    ///     let mut buf = BorrowedBuf::from(buf.as_mut_slice());
    ///     file.read_buf_exact_at(buf.unfilled(), 2)?;
    ///
    ///     assert_eq!(buf.filled(), b"1415926535");
    ///
    ///     Ok(())
    /// }
    /// ```
    #[unstable(feature = "read_buf_at", issue = "140771")]
    fn read_buf_exact_at(&self, mut buf: BorrowedCursor<'_>, mut offset: u64) -> io::Result<()> {
        while buf.capacity() > 0 {
            let prev_written = buf.written();
            match self.read_buf_at(buf.reborrow(), offset) {
                Ok(()) => {}
                Err(e) if e.is_interrupted() => {}
                Err(e) => return Err(e),
            }
            let n = buf.written() - prev_written;
            offset += n as u64;
            if n == 0 {
                return Err(io::Error::READ_EXACT_EOF);
            }
        }
        Ok(())
    }

    /// 从给定偏移量开始写入若干字节。
    ///
    /// 返回写入的字节数。
    ///
    /// 偏移量相对于文件起始处，因此与当前游标（cursor）无关。
    ///
    /// 当前文件游标不受此函数影响。
    ///
    /// 当写入超过文件末尾时，文件会被相应地扩展，中间的字节会被初始化为值 0。
    ///
    /// 注意，与 [`File::write`] 类似，发生短写（short write）并返回不算错误。
    ///
    /// # Bug
    /// 在某些系统上，`write_at` 借助 [`pwrite64`] 来写入文件。然而，该系统调用存在一个
    /// [bug]：以 `O_APPEND` 标志打开的文件不遵守 offset 参数，总是改为追加到文件末尾。
    ///
    /// 可能会在无意中设置该标志，正如下面的示例所示。因此，在更改选项时务必保持警惕，
    /// 以减轻意外行为。
    ///
    /// ```no_run
    /// use std::fs::File;
    /// use std::io;
    /// use std::os::unix::prelude::FileExt;
    ///
    /// fn main() -> io::Result<()> {
    ///     // 以 append 选项打开一个文件（会设置 `O_APPEND` 标志）
    ///     let file = File::options().append(true).open("foo.txt")?;
    ///
    ///     // 我们尝试在偏移量 10 处写入；却被追加到了 EOF
    ///     file.write_at(b"sushi", 10)?;
    ///
    ///     // foo.txt 长 5 字节而非 15 字节
    ///     Ok(())
    /// }
    /// ```
    ///
    /// [`File::write`]: fs::File::write
    /// [`pwrite64`]: https://man7.org/linux/man-pages/man2/pwrite.2.html
    /// [bug]: https://man7.org/linux/man-pages/man2/pwrite.2.html#BUGS
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs::File;
    /// use std::io;
    /// use std::os::unix::prelude::FileExt;
    ///
    /// fn main() -> io::Result<()> {
    ///     let file = File::create("foo.txt")?;
    ///
    ///     // 我们现在在偏移量 10 处写入。
    ///     file.write_at(b"sushi", 10)?;
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "file_offset", since = "1.15.0")]
    fn write_at(&self, buf: &[u8], offset: u64) -> io::Result<usize>;

    /// 与 `write_at` 类似，但它从一个缓冲区切片写入。
    ///
    /// 数据被依次从每个缓冲区复制，最后一个被读取的缓冲区可能只被部分消耗。
    /// 此方法的行为必须等同于对拼接后的缓冲区进行一次 `write_at` 调用。
    #[unstable(feature = "unix_file_vectored_at", issue = "89517")]
    fn write_vectored_at(&self, bufs: &[io::IoSlice<'_>], offset: u64) -> io::Result<usize> {
        io::default_write_vectored(|b| self.write_at(b, offset), bufs)
    }

    /// 尝试从给定偏移量开始写入整个缓冲区。
    ///
    /// 偏移量相对于文件起始处，因此与当前游标（cursor）无关。
    ///
    /// 当前文件游标不受此函数影响。
    ///
    /// 此方法将持续调用 [`write_at`]，直到没有更多数据需要写入，或返回了一个非
    /// [`io::ErrorKind::Interrupted`] 类型的错误。在整个缓冲区被成功写入或发生此类
    /// 错误之前，此方法不会返回。此方法产生的第一个非 [`io::ErrorKind::Interrupted`]
    /// 类型的错误将被返回。
    ///
    /// # Errors
    ///
    /// 此函数将返回 [`write_at`] 所返回的第一个非 [`io::ErrorKind::Interrupted`]
    /// 类型的错误。
    ///
    /// [`write_at`]: FileExt::write_at
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs::File;
    /// use std::io;
    /// use std::os::unix::prelude::FileExt;
    ///
    /// fn main() -> io::Result<()> {
    ///     let file = File::open("foo.txt")?;
    ///
    ///     // 我们现在在偏移量 10 处写入。
    ///     file.write_all_at(b"sushi", 10)?;
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "rw_exact_all_at", since = "1.33.0")]
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
}

#[stable(feature = "file_offset", since = "1.15.0")]
impl FileExt for fs::File {
    fn read_at(&self, buf: &mut [u8], offset: u64) -> io::Result<usize> {
        self.as_inner().read_at(buf, offset)
    }
    fn read_buf_at(&self, buf: BorrowedCursor<'_>, offset: u64) -> io::Result<()> {
        self.as_inner().read_buf_at(buf, offset)
    }
    fn read_vectored_at(&self, bufs: &mut [io::IoSliceMut<'_>], offset: u64) -> io::Result<usize> {
        self.as_inner().read_vectored_at(bufs, offset)
    }
    fn write_at(&self, buf: &[u8], offset: u64) -> io::Result<usize> {
        self.as_inner().write_at(buf, offset)
    }
    fn write_vectored_at(&self, bufs: &[io::IoSlice<'_>], offset: u64) -> io::Result<usize> {
        self.as_inner().write_vectored_at(bufs, offset)
    }
}

/// 针对 [`fs::Permissions`] 的 Unix 特有扩展。
///
/// # 示例
///
/// ```no_run
/// use std::fs::{File, Permissions};
/// use std::io::{ErrorKind, Result as IoResult};
/// use std::os::unix::fs::PermissionsExt;
///
/// fn main() -> IoResult<()> {
///     let name = "test_file_for_permissions";
///
///     // 确保文件不存在
///     let _ = std::fs::remove_file(name);
///     assert_eq!(
///         File::open(name).unwrap_err().kind(),
///         ErrorKind::NotFound,
///         "file already exists"
///     );
///
///     // 文件所有者的完整读/写/执行模式位（mode bits），
///     // 这些是我们想要添加到现有模式位上的
///     let my_mode = 0o700;
///
///     // 以指定权限创建新文件
///     {
///         let file = File::create(name)?;
///         let mut permissions = file.metadata()?.permissions();
///         eprintln!("Current permissions: {:o}", permissions.mode());
///
///         // 确保新权限尚未被设置
///         assert!(
///             permissions.mode() & my_mode != my_mode,
///             "permissions already set"
///         );
///
///         // 要么使用 `set_mode` 来更改一个现有的 Permissions 结构体
///         permissions.set_mode(permissions.mode() | my_mode);
///
///         // 要么使用 `from_mode` 来构造一个新的 Permissions 结构体
///         permissions = Permissions::from_mode(permissions.mode() | my_mode);
///
///         // 将新权限写入文件
///         file.set_permissions(permissions)?;
///     }
///
///     let permissions = File::open(name)?.metadata()?.permissions();
///     eprintln!("New permissions: {:o}", permissions.mode());
///
///     // 断言新权限已被设置
///     assert_eq!(
///         permissions.mode() & my_mode,
///         my_mode,
///         "new permissions not set"
///     );
///     Ok(())
/// }
/// ```
///
/// ```no_run
/// use std::fs::Permissions;
/// use std::os::unix::fs::PermissionsExt;
///
/// // 所有者可读/写，其他人可读
/// let my_mode = 0o644;
/// let mut permissions = Permissions::from_mode(my_mode);
/// assert_eq!(permissions.mode(), my_mode);
///
/// // 所有者可读/写/执行
/// let other_mode = 0o700;
/// permissions.set_mode(other_mode);
/// assert_eq!(permissions.mode(), other_mode);
/// ```
#[stable(feature = "fs_ext", since = "1.1.0")]
pub trait PermissionsExt {
    /// 返回 mode 权限位（mode permission bits）
    #[stable(feature = "fs_ext", since = "1.1.0")]
    fn mode(&self) -> u32;

    /// 设置 mode 权限位（mode permission bits）。
    #[stable(feature = "fs_ext", since = "1.1.0")]
    fn set_mode(&mut self, mode: u32);

    /// 从给定的 mode 权限位创建一个新实例。
    #[stable(feature = "fs_ext", since = "1.1.0")]
    #[cfg_attr(not(test), rustc_diagnostic_item = "permissions_from_mode")]
    fn from_mode(mode: u32) -> Self;
}

#[stable(feature = "fs_ext", since = "1.1.0")]
impl PermissionsExt for Permissions {
    fn mode(&self) -> u32 {
        self.as_inner().mode()
    }

    fn set_mode(&mut self, mode: u32) {
        *self = Permissions::from_inner(FromInner::from_inner(mode));
    }

    fn from_mode(mode: u32) -> Permissions {
        Permissions::from_inner(FromInner::from_inner(mode))
    }
}

/// 针对 [`fs::OpenOptions`] 的 Unix 特有扩展。
#[stable(feature = "fs_ext", since = "1.1.0")]
pub trait OpenOptionsExt {
    /// 设置创建新文件时所用的 mode 位（mode bits）。
    ///
    /// 如果作为 `OpenOptions::open` 调用的一部分创建了新文件，那么这个指定的 `mode`
    /// 将被用作新文件的权限位。如果未设置 `mode`，则使用默认值 `0o666`。
    /// 操作系统会用系统的 `umask` 把某些位掩去，从而产生最终的权限。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs::OpenOptions;
    /// use std::os::unix::fs::OpenOptionsExt;
    ///
    /// # fn main() {
    /// let mut options = OpenOptions::new();
    /// options.mode(0o644); // 给予所有者读/写权限，给予其他人读权限。
    /// let file = options.open("foo.txt");
    /// # }
    /// ```
    #[stable(feature = "fs_ext", since = "1.1.0")]
    fn mode(&mut self, mode: u32) -> &mut Self;

    /// 向 `open` 的 `flags` 参数传入自定义标志。
    ///
    /// 定义访问模式（access mode）的那些位会被 `O_ACCMODE` 掩去，以确保它们不会干扰由
    /// Rust 的选项所设置的访问模式。
    ///
    /// 自定义标志只能设置标志，而不能移除由 Rust 的选项所设置的标志。
    /// 此函数会覆盖任何先前设置的自定义标志。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// # mod libc { pub const O_NOFOLLOW: i32 = 0; }
    /// use std::fs::OpenOptions;
    /// use std::os::unix::fs::OpenOptionsExt;
    ///
    /// # fn main() {
    /// let mut options = OpenOptions::new();
    /// options.write(true);
    /// options.custom_flags(libc::O_NOFOLLOW);
    /// let file = options.open("foo.txt");
    /// # }
    /// ```
    #[stable(feature = "open_options_ext", since = "1.10.0")]
    fn custom_flags(&mut self, flags: i32) -> &mut Self;
}

#[stable(feature = "fs_ext", since = "1.1.0")]
impl OpenOptionsExt for OpenOptions {
    fn mode(&mut self, mode: u32) -> &mut OpenOptions {
        self.as_inner_mut().mode(mode);
        self
    }

    fn custom_flags(&mut self, flags: i32) -> &mut OpenOptions {
        self.as_inner_mut().custom_flags(flags);
        self
    }
}

/// 针对 [`fs::Metadata`] 的 Unix 特有扩展。
#[stable(feature = "metadata_ext", since = "1.1.0")]
pub trait MetadataExt {
    /// 返回包含该文件的设备的 ID。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::io;
    /// use std::fs;
    /// use std::os::unix::fs::MetadataExt;
    ///
    /// fn main() -> io::Result<()> {
    ///     let meta = fs::metadata("some_file")?;
    ///     let dev_id = meta.dev();
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "metadata_ext", since = "1.1.0")]
    fn dev(&self) -> u64;
    /// 返回 inode 号。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs;
    /// use std::os::unix::fs::MetadataExt;
    /// use std::io;
    ///
    /// fn main() -> io::Result<()> {
    ///     let meta = fs::metadata("some_file")?;
    ///     let inode = meta.ino();
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "metadata_ext", since = "1.1.0")]
    fn ino(&self) -> u64;
    /// 返回应用于该文件的权限（rights）。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs;
    /// use std::os::unix::fs::MetadataExt;
    /// use std::io;
    ///
    /// fn main() -> io::Result<()> {
    ///     let meta = fs::metadata("some_file")?;
    ///     let mode = meta.mode();
    ///     let user_has_write_access      = mode & 0o200;
    ///     let user_has_read_write_access = mode & 0o600;
    ///     let group_has_read_access      = mode & 0o040;
    ///     let others_have_exec_access    = mode & 0o001;
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "metadata_ext", since = "1.1.0")]
    fn mode(&self) -> u32;
    /// 返回指向该文件的硬链接（hard links）数量。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs;
    /// use std::os::unix::fs::MetadataExt;
    /// use std::io;
    ///
    /// fn main() -> io::Result<()> {
    ///     let meta = fs::metadata("some_file")?;
    ///     let nb_hard_links = meta.nlink();
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "metadata_ext", since = "1.1.0")]
    fn nlink(&self) -> u64;
    /// 返回该文件所有者的用户 ID。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs;
    /// use std::os::unix::fs::MetadataExt;
    /// use std::io;
    ///
    /// fn main() -> io::Result<()> {
    ///     let meta = fs::metadata("some_file")?;
    ///     let user_id = meta.uid();
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "metadata_ext", since = "1.1.0")]
    fn uid(&self) -> u32;
    /// 返回该文件所有者的组 ID。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs;
    /// use std::os::unix::fs::MetadataExt;
    /// use std::io;
    ///
    /// fn main() -> io::Result<()> {
    ///     let meta = fs::metadata("some_file")?;
    ///     let group_id = meta.gid();
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "metadata_ext", since = "1.1.0")]
    fn gid(&self) -> u32;
    /// 返回该文件的设备 ID（如果它是一个特殊文件）。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs;
    /// use std::os::unix::fs::MetadataExt;
    /// use std::io;
    ///
    /// fn main() -> io::Result<()> {
    ///     let meta = fs::metadata("some_file")?;
    ///     let device_id = meta.rdev();
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "metadata_ext", since = "1.1.0")]
    fn rdev(&self) -> u64;
    /// 返回该文件以字节为单位的总大小。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs;
    /// use std::os::unix::fs::MetadataExt;
    /// use std::io;
    ///
    /// fn main() -> io::Result<()> {
    ///     let meta = fs::metadata("some_file")?;
    ///     let file_size = meta.size();
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "metadata_ext", since = "1.1.0")]
    fn size(&self) -> u64;
    /// 返回该文件的上次访问时间，以自 Unix Epoch 以来的秒数表示。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs;
    /// use std::os::unix::fs::MetadataExt;
    /// use std::io;
    ///
    /// fn main() -> io::Result<()> {
    ///     let meta = fs::metadata("some_file")?;
    ///     let last_access_time = meta.atime();
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "metadata_ext", since = "1.1.0")]
    fn atime(&self) -> i64;
    /// 返回该文件的上次访问时间，以自 [`atime`] 以来的纳秒数表示。
    ///
    /// [`atime`]: MetadataExt::atime
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs;
    /// use std::os::unix::fs::MetadataExt;
    /// use std::io;
    ///
    /// fn main() -> io::Result<()> {
    ///     let meta = fs::metadata("some_file")?;
    ///     let nano_last_access_time = meta.atime_nsec();
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "metadata_ext", since = "1.1.0")]
    fn atime_nsec(&self) -> i64;
    /// 返回该文件的上次修改时间，以自 Unix Epoch 以来的秒数表示。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs;
    /// use std::os::unix::fs::MetadataExt;
    /// use std::io;
    ///
    /// fn main() -> io::Result<()> {
    ///     let meta = fs::metadata("some_file")?;
    ///     let last_modification_time = meta.mtime();
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "metadata_ext", since = "1.1.0")]
    fn mtime(&self) -> i64;
    /// 返回该文件的上次修改时间，以自 [`mtime`] 以来的纳秒数表示。
    ///
    /// [`mtime`]: MetadataExt::mtime
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs;
    /// use std::os::unix::fs::MetadataExt;
    /// use std::io;
    ///
    /// fn main() -> io::Result<()> {
    ///     let meta = fs::metadata("some_file")?;
    ///     let nano_last_modification_time = meta.mtime_nsec();
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "metadata_ext", since = "1.1.0")]
    fn mtime_nsec(&self) -> i64;
    /// 返回该文件的上次状态变更时间，以自 Unix Epoch 以来的秒数表示。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs;
    /// use std::os::unix::fs::MetadataExt;
    /// use std::io;
    ///
    /// fn main() -> io::Result<()> {
    ///     let meta = fs::metadata("some_file")?;
    ///     let last_status_change_time = meta.ctime();
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "metadata_ext", since = "1.1.0")]
    fn ctime(&self) -> i64;
    /// 返回该文件的上次状态变更时间，以自 [`ctime`] 以来的纳秒数表示。
    ///
    /// [`ctime`]: MetadataExt::ctime
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs;
    /// use std::os::unix::fs::MetadataExt;
    /// use std::io;
    ///
    /// fn main() -> io::Result<()> {
    ///     let meta = fs::metadata("some_file")?;
    ///     let nano_last_status_change_time = meta.ctime_nsec();
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "metadata_ext", since = "1.1.0")]
    fn ctime_nsec(&self) -> i64;
    /// 返回文件系统 I/O 的块大小（block size）。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs;
    /// use std::os::unix::fs::MetadataExt;
    /// use std::io;
    ///
    /// fn main() -> io::Result<()> {
    ///     let meta = fs::metadata("some_file")?;
    ///     let block_size = meta.blksize();
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "metadata_ext", since = "1.1.0")]
    fn blksize(&self) -> u64;
    /// 返回分配给该文件的块（block）数量，以 512 字节为单位。
    ///
    /// 请注意，当文件存在空洞（holes）时，这可能小于 `st_size / 512`。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs;
    /// use std::os::unix::fs::MetadataExt;
    /// use std::io;
    ///
    /// fn main() -> io::Result<()> {
    ///     let meta = fs::metadata("some_file")?;
    ///     let blocks = meta.blocks();
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "metadata_ext", since = "1.1.0")]
    fn blocks(&self) -> u64;
    #[cfg(target_os = "vxworks")]
    #[stable(feature = "metadata_ext", since = "1.1.0")]
    fn attrib(&self) -> u8;
}

#[stable(feature = "metadata_ext", since = "1.1.0")]
impl MetadataExt for fs::Metadata {
    fn dev(&self) -> u64 {
        self.st_dev()
    }
    fn ino(&self) -> u64 {
        self.st_ino()
    }
    fn mode(&self) -> u32 {
        self.st_mode()
    }
    fn nlink(&self) -> u64 {
        self.st_nlink()
    }
    fn uid(&self) -> u32 {
        self.st_uid()
    }
    fn gid(&self) -> u32 {
        self.st_gid()
    }
    fn rdev(&self) -> u64 {
        self.st_rdev()
    }
    fn size(&self) -> u64 {
        self.st_size()
    }
    fn atime(&self) -> i64 {
        self.st_atime()
    }
    fn atime_nsec(&self) -> i64 {
        self.st_atime_nsec()
    }
    fn mtime(&self) -> i64 {
        self.st_mtime()
    }
    fn mtime_nsec(&self) -> i64 {
        self.st_mtime_nsec()
    }
    fn ctime(&self) -> i64 {
        self.st_ctime()
    }
    fn ctime_nsec(&self) -> i64 {
        self.st_ctime_nsec()
    }
    fn blksize(&self) -> u64 {
        self.st_blksize()
    }
    fn blocks(&self) -> u64 {
        self.st_blocks()
    }
    #[cfg(target_os = "vxworks")]
    fn attrib(&self) -> u8 {
        self.st_attrib()
    }
}

/// 针对 [`fs::FileType`] 的 Unix 特有扩展。
///
/// 增加了对诸如块设备/字符设备、管道（pipe）和套接字（socket）等特殊 Unix 文件类型的支持。
#[stable(feature = "file_type_ext", since = "1.5.0")]
pub trait FileTypeExt {
    /// 如果此文件类型是块设备（block device），则返回 `true`。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs;
    /// use std::os::unix::fs::FileTypeExt;
    /// use std::io;
    ///
    /// fn main() -> io::Result<()> {
    ///     let meta = fs::metadata("block_device_file")?;
    ///     let file_type = meta.file_type();
    ///     assert!(file_type.is_block_device());
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "file_type_ext", since = "1.5.0")]
    fn is_block_device(&self) -> bool;
    /// 如果此文件类型是字符设备（char device），则返回 `true`。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs;
    /// use std::os::unix::fs::FileTypeExt;
    /// use std::io;
    ///
    /// fn main() -> io::Result<()> {
    ///     let meta = fs::metadata("char_device_file")?;
    ///     let file_type = meta.file_type();
    ///     assert!(file_type.is_char_device());
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "file_type_ext", since = "1.5.0")]
    fn is_char_device(&self) -> bool;
    /// 如果此文件类型是 fifo，则返回 `true`。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs;
    /// use std::os::unix::fs::FileTypeExt;
    /// use std::io;
    ///
    /// fn main() -> io::Result<()> {
    ///     let meta = fs::metadata("fifo_file")?;
    ///     let file_type = meta.file_type();
    ///     assert!(file_type.is_fifo());
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "file_type_ext", since = "1.5.0")]
    fn is_fifo(&self) -> bool;
    /// 如果此文件类型是套接字（socket），则返回 `true`。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs;
    /// use std::os::unix::fs::FileTypeExt;
    /// use std::io;
    ///
    /// fn main() -> io::Result<()> {
    ///     let meta = fs::metadata("unix.socket")?;
    ///     let file_type = meta.file_type();
    ///     assert!(file_type.is_socket());
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "file_type_ext", since = "1.5.0")]
    fn is_socket(&self) -> bool;
}

#[stable(feature = "file_type_ext", since = "1.5.0")]
impl FileTypeExt for fs::FileType {
    fn is_block_device(&self) -> bool {
        self.as_inner().is(libc::S_IFBLK)
    }
    fn is_char_device(&self) -> bool {
        self.as_inner().is(libc::S_IFCHR)
    }
    fn is_fifo(&self) -> bool {
        self.as_inner().is(libc::S_IFIFO)
    }
    fn is_socket(&self) -> bool {
        self.as_inner().is(libc::S_IFSOCK)
    }
}

/// 针对 [`fs::DirEntry`] 的 Unix 特有扩展方法。
#[stable(feature = "dir_entry_ext", since = "1.1.0")]
pub trait DirEntryExt {
    /// 返回所含 `dirent` 结构体中底层的 `d_ino` 字段。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::fs;
    /// use std::os::unix::fs::DirEntryExt;
    ///
    /// if let Ok(entries) = fs::read_dir(".") {
    ///     for entry in entries {
    ///         if let Ok(entry) = entry {
    ///             // 这里，`entry` 是一个 `DirEntry`。
    ///             println!("{:?}: {}", entry.file_name(), entry.ino());
    ///         }
    ///     }
    /// }
    /// ```
    #[stable(feature = "dir_entry_ext", since = "1.1.0")]
    fn ino(&self) -> u64;
}

#[stable(feature = "dir_entry_ext", since = "1.1.0")]
impl DirEntryExt for fs::DirEntry {
    fn ino(&self) -> u64 {
        self.as_inner().ino()
    }
}

/// 针对 [`fs::DirEntry`] 的、封闭的（sealed）Unix 特有扩展方法。
#[unstable(feature = "dir_entry_ext2", issue = "85573")]
pub trait DirEntryExt2: Sealed {
    /// 返回对此条目文件名底层 `OsStr` 的引用。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(dir_entry_ext2)]
    /// use std::os::unix::fs::DirEntryExt2;
    /// use std::{fs, io};
    ///
    /// fn main() -> io::Result<()> {
    ///     let mut entries = fs::read_dir(".")?.collect::<Result<Vec<_>, io::Error>>()?;
    ///     entries.sort_unstable_by(|a, b| a.file_name_ref().cmp(b.file_name_ref()));
    ///
    ///     for p in entries {
    ///         println!("{p:?}");
    ///     }
    ///
    ///     Ok(())
    /// }
    /// ```
    fn file_name_ref(&self) -> &OsStr;
}

/// 允许 `std` 内部使用的扩展 trait。
#[unstable(feature = "sealed", issue = "none")]
impl Sealed for fs::DirEntry {}

#[unstable(feature = "dir_entry_ext2", issue = "85573")]
impl DirEntryExt2 for fs::DirEntry {
    fn file_name_ref(&self) -> &OsStr {
        self.as_inner().file_name_os_str()
    }
}

/// 在文件系统上创建一个新的符号链接（symbolic link）。
///
/// `link` 路径将是一个指向 `original` 路径的符号链接。
///
/// # 示例
///
/// ```no_run
/// use std::os::unix::fs;
///
/// fn main() -> std::io::Result<()> {
///     fs::symlink("a.txt", "b.txt")?;
///     Ok(())
/// }
/// ```
#[stable(feature = "symlink", since = "1.1.0")]
pub fn symlink<P: AsRef<Path>, Q: AsRef<Path>>(original: P, link: Q) -> io::Result<()> {
    sys::fs::symlink(original.as_ref(), link.as_ref())
}

/// 针对 [`fs::DirBuilder`] 的 Unix 特有扩展。
#[stable(feature = "dir_builder", since = "1.6.0")]
pub trait DirBuilderExt {
    /// 设置创建新目录时所用的 mode。此选项默认为 0o777。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs::DirBuilder;
    /// use std::os::unix::fs::DirBuilderExt;
    ///
    /// let mut builder = DirBuilder::new();
    /// builder.mode(0o755);
    /// ```
    #[stable(feature = "dir_builder", since = "1.6.0")]
    fn mode(&mut self, mode: u32) -> &mut Self;
}

#[stable(feature = "dir_builder", since = "1.6.0")]
impl DirBuilderExt for fs::DirBuilder {
    fn mode(&mut self, mode: u32) -> &mut fs::DirBuilder {
        self.as_inner_mut().set_mode(mode);
        self
    }
}

/// 更改指定路径的所有者（owner）与所属组（group）。
///
/// 把 uid 或 gid 指定为 `None` 将使其保持不变。
///
/// 更改所有者通常需要特权，例如 root 或某项特定的 capability。
/// 更改所属组通常要么需要是该文件的所有者且为该组的成员，要么需要拥有特权。
///
/// 请注意，根据 POSIX，在大多数情况下更改所有者会清除 `suid` 与 `sgid` 权限位，
/// 通常即便用户是 root 也是如此。当文件对组不可执行（non-group-executable）时，sgid
/// 不会被清除。参见：<https://www.man7.org/linux/man-pages/man2/chown.2.html>
/// 如果存在文件 capabilities，此调用也可能将其清除。
///
/// 如果在符号链接上调用，这将更改链接目标（link target）的所有者与所属组。若要更改
/// 链接本身的所有者与所属组，参见 [`lchown`]。
///
/// # 示例
///
/// ```no_run
/// use std::os::unix::fs;
///
/// fn main() -> std::io::Result<()> {
///     fs::chown("/sandbox", Some(0), Some(0))?;
///     Ok(())
/// }
/// ```
#[stable(feature = "unix_chown", since = "1.73.0")]
pub fn chown<P: AsRef<Path>>(dir: P, uid: Option<u32>, gid: Option<u32>) -> io::Result<()> {
    sys::fs::chown(dir.as_ref(), uid.unwrap_or(u32::MAX), gid.unwrap_or(u32::MAX))
}

/// 更改由指定的已打开文件描述符所引用的文件的所有者与所属组。
///
/// 关于语义与所需特权，参见 [`chown`]。
///
/// # 示例
///
/// ```no_run
/// use std::os::unix::fs;
///
/// fn main() -> std::io::Result<()> {
///     let f = std::fs::File::open("/file")?;
///     fs::fchown(&f, Some(0), Some(0))?;
///     Ok(())
/// }
/// ```
#[stable(feature = "unix_chown", since = "1.73.0")]
pub fn fchown<F: AsFd>(fd: F, uid: Option<u32>, gid: Option<u32>) -> io::Result<()> {
    sys::fs::fchown(fd.as_fd().as_raw_fd(), uid.unwrap_or(u32::MAX), gid.unwrap_or(u32::MAX))
}

/// 更改指定路径的所有者与所属组，且不对符号链接解引用（dereferencing）。
///
/// 与 [`chown`] 相同，区别在于如果在符号链接上调用，这将更改链接本身的所有者与所属组，
/// 而非链接目标的所有者与所属组。
///
/// # 示例
///
/// ```no_run
/// use std::os::unix::fs;
///
/// fn main() -> std::io::Result<()> {
///     fs::lchown("/symlink", Some(0), Some(0))?;
///     Ok(())
/// }
/// ```
#[stable(feature = "unix_chown", since = "1.73.0")]
pub fn lchown<P: AsRef<Path>>(dir: P, uid: Option<u32>, gid: Option<u32>) -> io::Result<()> {
    sys::fs::lchown(dir.as_ref(), uid.unwrap_or(u32::MAX), gid.unwrap_or(u32::MAX))
}

/// 将当前进程的根目录更改为指定路径。
///
/// 这通常需要特权，例如 root 或某项特定的 capability。
///
/// 这不会更改当前工作目录；之后你应当调用
/// [`std::env::set_current_dir`][`crate::env::set_current_dir`]。
///
/// # 示例
///
/// ```no_run
/// use std::os::unix::fs;
///
/// fn main() -> std::io::Result<()> {
///     fs::chroot("/sandbox")?;
///     std::env::set_current_dir("/")?;
///     // 在沙箱中继续工作
///     Ok(())
/// }
/// ```
#[stable(feature = "unix_chroot", since = "1.56.0")]
#[cfg(not(target_os = "fuchsia"))]
pub fn chroot<P: AsRef<Path>>(dir: P) -> io::Result<()> {
    sys::fs::chroot(dir.as_ref())
}

/// 在指定路径以指定 mode 创建一个 FIFO 特殊文件。
///
/// # 示例
///
/// ```no_run
/// # #![feature(unix_mkfifo)]
/// # #[cfg(not(unix))]
/// # fn main() {}
/// # #[cfg(unix)]
/// # fn main() -> std::io::Result<()> {
/// # use std::{
/// #     os::unix::fs::{mkfifo, PermissionsExt},
/// #     fs::{File, Permissions, remove_file},
/// #     io::{Write, Read},
/// # };
/// # let _ = remove_file("/tmp/fifo");
/// mkfifo("/tmp/fifo", Permissions::from_mode(0o774))?;
///
/// let mut wx = File::options().read(true).write(true).open("/tmp/fifo")?;
/// let mut rx = File::open("/tmp/fifo")?;
///
/// wx.write_all(b"hello, world!")?;
/// drop(wx);
///
/// let mut s = String::new();
/// rx.read_to_string(&mut s)?;
///
/// assert_eq!(s, "hello, world!");
/// # Ok(())
/// # }
/// ```
#[unstable(feature = "unix_mkfifo", issue = "139324")]
pub fn mkfifo<P: AsRef<Path>>(path: P, permissions: Permissions) -> io::Result<()> {
    sys::fs::mkfifo(path.as_ref(), permissions.mode())
}
