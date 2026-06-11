//! 文件系统操作。
//!
//! 本模块包含操作本地文件系统内容的基础方法。模块中的所有方法都表示跨平台的
//! 文件系统操作。额外的平台特定功能可以在 `std::os::$platform` 的扩展 trait
//! 中找到。
//!
//! # Time of Check to Time of Use (TOCTOU)
//!
//! 许多文件系统操作都会受到一种被称为“检查时刻到使用时刻”(TOCTOU) 的竞态条件影响。
//! 当程序先检查某个条件（例如文件是否存在或权限如何），然后根据检查结果做出决策时，
//! 这种竞态就会发生——因为该条件在检查与使用之间可能已经被改变了。
//!
//! 例如，先检查文件是否存在、再在不存在时创建它，就容易受到 TOCTOU 影响：
//! 另一个进程可能在你的检查与创建尝试之间把文件创建出来。
//!
//! 另一个例子涉及符号链接：在移除一个目录时，如果另一个进程在检查与移除操作之间
//! 把该目录替换成了符号链接，那么移除操作可能会作用到错误的位置上。这正是为什么
//! 像 [`remove_dir_all`] 这样的操作需要使用原子操作来防止此类竞态条件。
//!
//! 为避免 TOCTOU 问题：
//! - 要意识到元数据操作（例如 [`metadata`] 或 [`symlink_metadata`]）可能会受到
//! 其他进程所做改动的影响。
//! - 尽可能使用原子操作（例如用 [`File::create_new`] 而不是先检查存在性再创建）。
//! - 在操作的整个期间保持文件处于打开状态。

#![stable(feature = "rust1", since = "1.0.0")]
#![deny(unsafe_op_in_unsafe_fn)]

#[cfg(all(
    test,
    not(any(
        target_os = "emscripten",
        target_os = "wasi",
        target_env = "sgx",
        target_os = "xous",
        target_os = "trusty",
    ))
))]
mod tests;

use crate::ffi::OsString;
use crate::io::{self, BorrowedCursor, IoSlice, IoSliceMut, Read, Seek, SeekFrom, Write};
use crate::path::{Path, PathBuf};
use crate::sealed::Sealed;
use crate::sync::Arc;
use crate::sys::{AsInner, AsInnerMut, FromInner, IntoInner, fs as fs_imp};
use crate::time::SystemTime;
use crate::{error, fmt};

/// 提供对文件系统中已打开文件的访问的对象。
///
/// 一个 `File` 实例可读和/或可写，具体取决于它是用什么选项打开的。文件还实现了
/// [`Seek`]，用于改变文件内部维护的逻辑游标。
///
/// 文件在离开作用域时会被自动关闭。`Drop` 的实现会忽略关闭时检测到的错误。
/// 如果必须手动处理这些错误，请使用 [`sync_all`] 方法。
///
/// `File` 不会缓冲读写。出于效率考虑，在执行许多小规模的 [`read`] 或 [`write`]
/// 调用时，除非确实需要无缓冲读写，否则考虑用 [`BufReader`] 或 [`BufWriter`]
/// 包裹该文件。
///
/// # 示例
///
/// 创建一个新文件并向其写入字节（你也可以使用 [`write`]）：
///
/// ```no_run
/// use std::fs::File;
/// use std::io::prelude::*;
///
/// fn main() -> std::io::Result<()> {
///     let mut file = File::create("foo.txt")?;
///     file.write_all(b"Hello, world!")?;
///     Ok(())
/// }
/// ```
///
/// 将文件内容读入一个 [`String`]（你也可以使用 [`read`]）：
///
/// ```no_run
/// use std::fs::File;
/// use std::io::prelude::*;
///
/// fn main() -> std::io::Result<()> {
///     let mut file = File::open("foo.txt")?;
///     let mut contents = String::new();
///     file.read_to_string(&mut contents)?;
///     assert_eq!(contents, "Hello, world!");
///     Ok(())
/// }
/// ```
///
/// 使用带缓冲的 [`Read`]er：
///
/// ```no_run
/// use std::fs::File;
/// use std::io::BufReader;
/// use std::io::prelude::*;
///
/// fn main() -> std::io::Result<()> {
///     let file = File::open("foo.txt")?;
///     let mut buf_reader = BufReader::new(file);
///     let mut contents = String::new();
///     buf_reader.read_to_string(&mut contents)?;
///     assert_eq!(contents, "Hello, world!");
///     Ok(())
/// }
/// ```
///
/// 注意，尽管由于 [`Read`] 和 [`Write`] 接口的缘故，读写方法都需要 `&mut File`，
/// 但持有 `&File` 的一方仍然可以修改文件——既可以通过那些接收 `&File` 的方法，
/// 也可以通过取出底层 OS 对象并以那种方式修改文件。此外，许多操作系统允许不同进程
/// 并发修改文件。不要假设持有 `&File` 就意味着文件不会发生变化。
///
/// # 平台特定行为
///
/// 在 Windows 上，`File` 的 [`Read`] 和 [`Write`] trait 实现执行同步 I/O 操作。
/// 因此底层文件不能是为异步 I/O 打开的（例如使用了 `FILE_FLAG_OVERLAPPED`）。
///
/// [`BufReader`]: io::BufReader
/// [`BufWriter`]: io::BufWriter
/// [`sync_all`]: File::sync_all
/// [`write`]: File::write
/// [`read`]: File::read
#[stable(feature = "rust1", since = "1.0.0")]
#[cfg_attr(not(test), rustc_diagnostic_item = "File")]
pub struct File {
    inner: fs_imp::File,
}

/// 在 [`File`] 上调用 [`try_lock`] 方法和 [`try_lock_shared`] 方法尝试获取锁时
/// 可能发生的各类错误的枚举。
///
/// [`try_lock`]: File::try_lock
/// [`try_lock_shared`]: File::try_lock_shared
#[stable(feature = "file_lock", since = "1.89.0")]
pub enum TryLockError {
    /// 由于对文件的 I/O 错误，锁未能获取。标准库不会在 [`TryLockError::Error`]
    /// 内部返回 [`ErrorKind::WouldBlock`] 错误。
    ///
    /// [`ErrorKind::WouldBlock`]: io::ErrorKind::WouldBlock
    Error(io::Error),
    /// 由于锁当前被另一个句柄/进程持有，此刻无法获取锁。
    WouldBlock,
}

/// 提供对文件系统中某个目录的访问的对象。
///
/// 目录在离开作用域时会被自动关闭。`Drop` 的实现会忽略关闭时检测到的错误。
///
/// # 平台特定行为
///
/// 在受支持的系统上（包括 Windows 和某些基于 UNIX 的 OS），此函数会获取该目录的
/// 句柄/文件描述符。这使得像 [`Dir::open_file`] 这样的函数在目录本身被移动时
/// 仍能避免 [TOCTOU] 错误。
///
/// 在其他系统上，它存储的是一个绝对路径（见 [`canonicalize()`]）。在后一种情况下，
/// 不提供任何 [TOCTOU] 保证。
///
/// # 示例
///
/// 打开一个目录，然后打开其中的一个文件。
///
/// ```no_run
/// #![feature(dirfd)]
/// use std::{fs::Dir, io};
///
/// fn main() -> std::io::Result<()> {
///     let dir = Dir::open("foo")?;
///     let mut file = dir.open_file("bar.txt")?;
///     let contents = io::read_to_string(file)?;
///     assert_eq!(contents, "Hello, world!");
///     Ok(())
/// }
/// ```
///
/// [TOCTOU]: self#time-of-check-to-time-of-use-toctou
#[unstable(feature = "dirfd", issue = "120426")]
pub struct Dir {
    inner: fs_imp::Dir,
}

/// 关于文件的元数据信息。
///
/// 该结构由 [`metadata`] 或 [`symlink_metadata`] 函数或方法返回，表示关于文件的
/// 已知元数据，例如它的权限、大小、修改时间等。
#[stable(feature = "rust1", since = "1.0.0")]
#[derive(Clone)]
pub struct Metadata(fs_imp::FileAttr);

/// 遍历目录中各条目的迭代器。
///
/// 该迭代器由本模块的 [`read_dir`] 函数返回，会产出
/// <code>[io::Result]<[DirEntry]></code> 实例。通过 [`DirEntry`] 可以获知条目的
/// 路径之类的信息，可能还能获取其他元数据。
///
/// 此迭代器返回条目的顺序取决于平台和文件系统。
///
/// # 错误(Errors）
/// 如果在从 OS 获取下一个条目时发生错误，此 [`io::Result`] 将是一个 [`Err`]。
#[stable(feature = "rust1", since = "1.0.0")]
#[derive(Debug)]
pub struct ReadDir(fs_imp::ReadDir);

/// 由 [`ReadDir`] 迭代器返回的条目。
///
/// 一个 `DirEntry` 实例表示文件系统中某个目录内部的一个条目。每个条目都可以通过
/// 方法来检查，以了解其完整路径，或者通过各平台的扩展 trait 可能获取其他元数据。
///
/// # 平台特定行为
///
/// 在 Unix 上，`DirEntry` 结构内部包含一个对已打开目录的引用。即使 `ReadDir`
/// 迭代器已被丢弃，持有 `DirEntry` 对象仍会占用一个文件句柄。
///
/// 注意，此行为[未来可能改变][changes]。
///
/// [changes]: io#platform-specific-behavior
#[stable(feature = "rust1", since = "1.0.0")]
pub struct DirEntry(fs_imp::DirEntry);

/// 可用于配置如何打开文件的各项选项和标志。
///
/// 此构建器暴露了配置 [`File`] 打开方式以及对已打开文件允许哪些操作的能力。
/// [`File::open`] 和 [`File::create`] 方法是使用此构建器的常用选项的别名。
///
/// 一般来说，使用 `OpenOptions` 时，你会先调用 [`OpenOptions::new`]，然后链式调用
/// 各个方法来设置每个选项，最后调用 [`OpenOptions::open`]，传入你想打开的文件路径。
/// 这会给你一个 [`io::Result`]，其中包含一个可以进一步操作的 [`File`]。
///
/// # 示例
///
/// 打开文件以读取：
///
/// ```no_run
/// use std::fs::OpenOptions;
///
/// let file = OpenOptions::new().read(true).open("foo.txt");
/// ```
///
/// 打开文件以同时读写，并在文件不存在时创建它：
///
/// ```no_run
/// use std::fs::OpenOptions;
///
/// let file = OpenOptions::new()
///             .read(true)
///             .write(true)
///             .create(true)
///             .open("foo.txt");
/// ```
#[derive(Clone, Debug)]
#[stable(feature = "rust1", since = "1.0.0")]
#[cfg_attr(not(test), rustc_diagnostic_item = "FsOpenOptions")]
pub struct OpenOptions(fs_imp::OpenOptions);

/// 文件上各种时间戳的表示。
#[derive(Copy, Clone, Debug, Default)]
#[stable(feature = "file_set_times", since = "1.75.0")]
pub struct FileTimes(fs_imp::FileTimes);

/// 文件上各种权限的表示。
///
/// 本模块目前仅提供一项信息，即 [`Permissions::readonly`]，它在当前所有受支持的
/// 平台上都有暴露。Unix 特有的功能（例如 mode 位）可通过 [`PermissionsExt`]
/// trait 获取。
///
/// [`PermissionsExt`]: crate::os::unix::fs::PermissionsExt
#[derive(Clone, PartialEq, Eq, Debug)]
#[stable(feature = "rust1", since = "1.0.0")]
#[cfg_attr(not(test), rustc_diagnostic_item = "FsPermissions")]
pub struct Permissions(fs_imp::FilePermissions);

/// 表示文件类型的结构，带有针对每种文件类型的访问器。
/// 它由 [`Metadata::file_type`] 方法返回。
#[stable(feature = "file_type", since = "1.1.0")]
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(not(test), rustc_diagnostic_item = "FileType")]
pub struct FileType(fs_imp::FileType);

/// 用于以各种方式创建目录的构建器。
///
/// 此构建器还支持平台特定的选项。
#[stable(feature = "dir_builder", since = "1.6.0")]
#[cfg_attr(not(test), rustc_diagnostic_item = "DirBuilder")]
#[derive(Debug)]
pub struct DirBuilder {
    inner: fs_imp::DirBuilder,
    recursive: bool,
}

/// 将一个文件的全部内容读入一个字节向量。
///
/// 这是一个便捷函数，等价于使用 [`File::open`] 和 [`read_to_end`]，但导入更少、
/// 也无需中间变量。
///
/// [`read_to_end`]: Read::read_to_end
///
/// # 错误(Errors）
///
/// 如果 `path` 尚不存在，此函数将返回一个错误。根据 [`OpenOptions::open`]，
/// 也可能返回其他错误。
///
/// 在从文件读取时，此函数会自动重试以处理 [`io::ErrorKind::Interrupted`]。
/// 详见 [io::Read] 文档。
///
/// # 示例
///
/// ```no_run
/// use std::fs;
///
/// fn main() -> Result<(), Box<dyn std::error::Error + 'static>> {
///     let data: Vec<u8> = fs::read("image.jpg")?;
///     assert_eq!(data[0..3], [0xFF, 0xD8, 0xFF]);
///     Ok(())
/// }
/// ```
#[stable(feature = "fs_read_write_bytes", since = "1.26.0")]
pub fn read<P: AsRef<Path>>(path: P) -> io::Result<Vec<u8>> {
    fn inner(path: &Path) -> io::Result<Vec<u8>> {
        let mut file = File::open(path)?;
        let size = file.metadata().map(|m| usize::try_from(m.len()).unwrap_or(usize::MAX)).ok();
        let mut bytes = Vec::try_with_capacity(size.unwrap_or(0))?;
        io::default_read_to_end(&mut file, &mut bytes, size)?;
        Ok(bytes)
    }
    inner(path.as_ref())
}

/// 将一个文件的全部内容读入一个字符串。
///
/// 这是一个便捷函数，等价于使用 [`File::open`] 和 [`read_to_string`]，但导入更少、
/// 也无需中间变量。
///
/// [`read_to_string`]: Read::read_to_string
///
/// # 错误(Errors）
///
/// 如果 `path` 尚不存在，此函数将返回一个错误。根据 [`OpenOptions::open`]，
/// 也可能返回其他错误。
///
/// 如果文件内容不是有效的 UTF-8，也会返回一个错误。
///
/// 在从文件读取时，此函数会自动重试以处理 [`io::ErrorKind::Interrupted`]。
/// 详见 [io::Read] 文档。
///
/// # 示例
///
/// ```no_run
/// use std::fs;
/// use std::error::Error;
///
/// fn main() -> Result<(), Box<dyn Error>> {
///     let message: String = fs::read_to_string("message.txt")?;
///     println!("{}", message);
///     Ok(())
/// }
/// ```
#[stable(feature = "fs_read_write", since = "1.26.0")]
pub fn read_to_string<P: AsRef<Path>>(path: P) -> io::Result<String> {
    fn inner(path: &Path) -> io::Result<String> {
        let mut file = File::open(path)?;
        let size = file.metadata().map(|m| usize::try_from(m.len()).unwrap_or(usize::MAX)).ok();
        let mut string = String::new();
        string.try_reserve_exact(size.unwrap_or(0))?;
        io::default_read_to_string(&mut file, &mut string, size)?;
        Ok(string)
    }
    inner(path.as_ref())
}

/// 将一个切片作为文件的全部内容写入。
///
/// 如果文件不存在，此函数会创建它；如果文件已存在，则会完全替换其内容。
///
/// 取决于平台，如果完整的目录路径不存在，此函数可能会失败。
///
/// 这是一个便捷函数，等价于使用 [`File::create`] 和 [`write_all`]，但导入更少。
///
/// [`write_all`]: Write::write_all
///
/// # 示例
///
/// ```no_run
/// use std::fs;
///
/// fn main() -> std::io::Result<()> {
///     fs::write("foo.txt", b"Lorem ipsum")?;
///     fs::write("bar.txt", "dolor sit")?;
///     Ok(())
/// }
/// ```
#[stable(feature = "fs_read_write_bytes", since = "1.26.0")]
pub fn write<P: AsRef<Path>, C: AsRef<[u8]>>(path: P, contents: C) -> io::Result<()> {
    fn inner(path: &Path, contents: &[u8]) -> io::Result<()> {
        File::create(path)?.write_all(contents)
    }
    inner(path.as_ref(), contents.as_ref())
}

/// 更改指定路径处文件或目录的时间戳。
///
/// 此函数会尝试将访问时间和修改时间设置为所指定的时间。如果路径指向一个符号链接，
/// 此函数将跟随该链接，更改目标文件的时间戳。
///
/// # 平台特定行为
///
/// 此函数目前在 Unix 平台上对应 `utimensat` 函数，在 Apple 平台上对应
/// `setattrlist` 函数，在 Windows 上对应 `SetFileTime` 函数。
///
/// # 错误(Errors）
///
/// 如果用户缺乏更改目标文件或符号链接时间戳的权限，此函数将返回一个错误。
/// 如果 OS 不支持该操作，也可能返回错误。
///
/// # 示例
///
/// ```no_run
/// #![feature(fs_set_times)]
/// use std::fs::{self, FileTimes};
/// use std::time::SystemTime;
///
/// fn main() -> std::io::Result<()> {
///     let now = SystemTime::now();
///     let times = FileTimes::new()
///         .set_accessed(now)
///         .set_modified(now);
///     fs::set_times("foo.txt", times)?;
///     Ok(())
/// }
/// ```
#[unstable(feature = "fs_set_times", issue = "147455")]
#[doc(alias = "utimens")]
#[doc(alias = "utimes")]
#[doc(alias = "utime")]
pub fn set_times<P: AsRef<Path>>(path: P, times: FileTimes) -> io::Result<()> {
    fs_imp::set_times(path.as_ref(), times.0)
}

/// 更改指定路径处文件或符号链接的时间戳。
///
/// 此函数会尝试将访问时间和修改时间设置为所指定的时间。与 `set_times` 不同，
/// 如果路径指向一个符号链接，此函数将更改符号链接自身的时间戳，而不是目标文件的。
///
/// # 平台特定行为
///
/// 此函数目前在 Unix 平台上对应带 `AT_SYMLINK_NOFOLLOW` 的 `utimensat` 函数，
/// 在 Apple 平台上对应带 `FSOPT_NOFOLLOW` 的 `setattrlist` 函数，在 Windows 上
/// 对应 `SetFileTime` 函数。
///
/// # 错误(Errors）
///
/// 如果用户缺乏更改目标文件或符号链接时间戳的权限，此函数将返回一个错误。
/// 如果 OS 不支持该操作，也可能返回错误。
///
/// # 示例
///
/// ```no_run
/// #![feature(fs_set_times)]
/// use std::fs::{self, FileTimes};
/// use std::time::SystemTime;
///
/// fn main() -> std::io::Result<()> {
///     let now = SystemTime::now();
///     let times = FileTimes::new()
///         .set_accessed(now)
///         .set_modified(now);
///     fs::set_times_nofollow("symlink.txt", times)?;
///     Ok(())
/// }
/// ```
#[unstable(feature = "fs_set_times", issue = "147455")]
#[doc(alias = "utimensat")]
#[doc(alias = "lutimens")]
#[doc(alias = "lutimes")]
pub fn set_times_nofollow<P: AsRef<Path>>(path: P, times: FileTimes) -> io::Result<()> {
    fs_imp::set_times_nofollow(path.as_ref(), times.0)
}

#[stable(feature = "file_lock", since = "1.89.0")]
impl error::Error for TryLockError {}

#[stable(feature = "file_lock", since = "1.89.0")]
impl fmt::Debug for TryLockError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TryLockError::Error(err) => err.fmt(f),
            TryLockError::WouldBlock => "WouldBlock".fmt(f),
        }
    }
}

#[stable(feature = "file_lock", since = "1.89.0")]
impl fmt::Display for TryLockError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TryLockError::Error(_) => "lock acquisition failed due to I/O error",
            TryLockError::WouldBlock => "lock acquisition failed because the operation would block",
        }
        .fmt(f)
    }
}

#[stable(feature = "file_lock", since = "1.89.0")]
impl From<TryLockError> for io::Error {
    fn from(err: TryLockError) -> io::Error {
        match err {
            TryLockError::Error(err) => err,
            TryLockError::WouldBlock => io::ErrorKind::WouldBlock.into(),
        }
    }
}

impl File {
    /// 尝试以只读模式打开一个文件。
    ///
    /// 更多细节见 [`OpenOptions::open`] 方法。
    ///
    /// 如果你只需要读取整个文件内容，可考虑改用
    /// [`std::fs::read()`][self::read] 或
    /// [`std::fs::read_to_string()`][self::read_to_string]。
    ///
    /// # 错误(Errors）
    ///
    /// 如果 `path` 尚不存在，此函数将返回一个错误。根据 [`OpenOptions::open`]，
    /// 也可能返回其他错误。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs::File;
    /// use std::io::Read;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let mut f = File::open("foo.txt")?;
    ///     let mut data = vec![];
    ///     f.read_to_end(&mut data)?;
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn open<P: AsRef<Path>>(path: P) -> io::Result<File> {
        OpenOptions::new().read(true).open(path.as_ref())
    }

    /// 尝试以带缓冲的只读模式打开一个文件。
    ///
    /// 更多细节见 [`OpenOptions::open`] 方法、[`BufReader`][io::BufReader] 类型
    /// 以及 [`BufRead`][io::BufRead] trait。
    ///
    /// 如果你只需要读取整个文件内容，可考虑改用
    /// [`std::fs::read()`][self::read] 或
    /// [`std::fs::read_to_string()`][self::read_to_string]。
    ///
    /// # 错误(Errors）
    ///
    /// 如果 `path` 尚不存在，或者为新缓冲区分配内存失败，此函数将返回一个错误。
    /// 根据 [`OpenOptions::open`]，也可能返回其他错误。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// #![feature(file_buffered)]
    /// use std::fs::File;
    /// use std::io::BufRead;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let mut f = File::open_buffered("foo.txt")?;
    ///     assert!(f.capacity() > 0);
    ///     for (line, i) in f.lines().zip(1..) {
    ///         println!("{i:6}: {}", line?);
    ///     }
    ///     Ok(())
    /// }
    /// ```
    #[unstable(feature = "file_buffered", issue = "130804")]
    pub fn open_buffered<P: AsRef<Path>>(path: P) -> io::Result<io::BufReader<File>> {
        // 先分配缓冲区，这样在分配失败时就不会对文件系统造成任何影响。
        let buffer = io::BufReader::<Self>::try_new_buffer()?;
        let file = File::open(path)?;
        Ok(io::BufReader::with_buffer(file, buffer))
    }

    /// 以只写模式打开一个文件。
    ///
    /// 如果文件不存在，此函数会创建它；如果文件已存在，则会将其截断。
    ///
    /// 取决于平台，如果完整的目录路径不存在，此函数可能会失败。
    /// 更多细节见 [`OpenOptions::open`] 函数。
    ///
    /// 另见 [`std::fs::write()`][self::write]，这是一个用给定数据创建文件的
    /// 简单函数。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs::File;
    /// use std::io::Write;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let mut f = File::create("foo.txt")?;
    ///     f.write_all(&1234_u32.to_be_bytes())?;
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn create<P: AsRef<Path>>(path: P) -> io::Result<File> {
        OpenOptions::new().write(true).create(true).truncate(true).open(path.as_ref())
    }

    /// 以带缓冲的只写模式打开一个文件。
    ///
    /// 如果文件不存在，此函数会创建它；如果文件已存在，则会将其截断。
    ///
    /// 取决于平台，如果完整的目录路径不存在，此函数可能会失败。
    ///
    /// 更多细节见 [`OpenOptions::open`] 方法和
    /// [`BufWriter`][io::BufWriter] 类型。
    ///
    /// 另见 [`std::fs::write()`][self::write]，这是一个用给定数据创建文件的
    /// 简单函数。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// #![feature(file_buffered)]
    /// use std::fs::File;
    /// use std::io::Write;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let mut f = File::create_buffered("foo.txt")?;
    ///     assert!(f.capacity() > 0);
    ///     for i in 0..100 {
    ///         writeln!(&mut f, "{i}")?;
    ///     }
    ///     f.flush()?;
    ///     Ok(())
    /// }
    /// ```
    #[unstable(feature = "file_buffered", issue = "130804")]
    pub fn create_buffered<P: AsRef<Path>>(path: P) -> io::Result<io::BufWriter<File>> {
        // 先分配缓冲区，这样在分配失败时就不会对文件系统造成任何影响。
        let buffer = io::BufWriter::<Self>::try_new_buffer()?;
        let file = File::create(path)?;
        Ok(io::BufWriter::with_buffer(file, buffer))
    }

    /// 以读写模式创建一个新文件；如果文件已存在则报错。
    ///
    /// 如果文件不存在，此函数会创建它；如果文件已存在，则返回一个错误。这样一来，
    /// 只要调用成功，返回的文件就保证是新创建的。
    /// 如果目标位置已存在一个文件，创建新文件将以 [`AlreadyExists`] 失败，或根据
    /// 具体情况返回其他错误。可能出现的错误的非穷尽列表见 [`OpenOptions::open`]。
    ///
    /// 此选项之所以有用，是因为它是原子的。否则，在检查文件是否存在与创建新文件
    /// 之间，文件可能已被另一个进程创建（一种 [TOCTOU] 竞态条件 / 攻击）。
    ///
    /// 这也可以写成
    /// `File::options().read(true).write(true).create_new(true).open(...)`。
    ///
    /// [`AlreadyExists`]: crate::io::ErrorKind::AlreadyExists
    /// [TOCTOU]: self#time-of-check-to-time-of-use-toctou
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs::File;
    /// use std::io::Write;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let mut f = File::create_new("foo.txt")?;
    ///     f.write_all("Hello, world!".as_bytes())?;
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "file_create_new", since = "1.77.0")]
    pub fn create_new<P: AsRef<Path>>(path: P) -> io::Result<File> {
        OpenOptions::new().read(true).write(true).create_new(true).open(path.as_ref())
    }

    /// 返回一个新的 OpenOptions 对象。
    ///
    /// 当 `open()` 或 `create()` 不合适时，此函数返回一个新的 OpenOptions 对象，
    /// 你可以用它以特定选项打开或创建文件。
    ///
    /// 它等价于 `OpenOptions::new()`，但能让你写出更易读的代码。相比于
    /// `OpenOptions::new().append(true).open("example.log")`，
    /// 你可以写 `File::options().append(true).open("example.log")`。这也避免了
    /// 导入 `OpenOptions` 的需要。
    ///
    /// 更多细节见 [`OpenOptions::new`] 函数。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs::File;
    /// use std::io::Write;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let mut f = File::options().append(true).open("example.log")?;
    ///     writeln!(&mut f, "new line")?;
    ///     Ok(())
    /// }
    /// ```
    #[must_use]
    #[stable(feature = "with_options", since = "1.58.0")]
    #[cfg_attr(not(test), rustc_diagnostic_item = "file_options")]
    pub fn options() -> OpenOptions {
        OpenOptions::new()
    }

    /// 尝试将所有 OS 内部的文件内容和元数据同步到磁盘。
    ///
    /// 此函数会尝试确保所有内存中的数据在返回之前到达文件系统。
    ///
    /// 这可以用来处理那些原本只有在 `File` 关闭时才会被捕获的错误，因为丢弃一个
    /// `File` 会忽略所有错误。但要注意，`sync_all` 通常比通过丢弃来关闭文件更昂贵，
    /// 因为后者并不要求阻塞到数据已写入文件系统为止。
    ///
    /// 如果无需同步元数据，请改用 [`sync_data`]。
    ///
    /// [`sync_data`]: File::sync_data
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs::File;
    /// use std::io::prelude::*;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let mut f = File::create("foo.txt")?;
    ///     f.write_all(b"Hello, world!")?;
    ///
    ///     f.sync_all()?;
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[doc(alias = "fsync")]
    pub fn sync_all(&self) -> io::Result<()> {
        self.inner.fsync()
    }

    /// 此函数与 [`sync_all`] 类似，区别在于它可能不会把文件元数据同步到文件系统。
    ///
    /// 它适用于那些必须同步内容、但不需要元数据落盘的场景。此方法的目标是减少磁盘
    /// 操作。
    ///
    /// 注意，某些平台可能直接用 [`sync_all`] 来实现它。
    ///
    /// [`sync_all`]: File::sync_all
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs::File;
    /// use std::io::prelude::*;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let mut f = File::create("foo.txt")?;
    ///     f.write_all(b"Hello, world!")?;
    ///
    ///     f.sync_data()?;
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[doc(alias = "fdatasync")]
    pub fn sync_data(&self) -> io::Result<()> {
        self.inner.datasync()
    }

    /// 获取文件上的独占锁。阻塞直到能够获取该锁为止。
    ///
    /// 这会获取一个独占锁；指向此文件的其他任何文件句柄都不能再获取任何锁。
    ///
    /// 此锁可能是建议性的，也可能是强制性的。此锁意在与 [`lock`]、[`try_lock`]、
    /// [`lock_shared`]、[`try_lock_shared`] 和 [`unlock`] 相互配合。它与其他方法
    /// （例如 [`read`] 和 [`write`]）的交互是平台特定的，可能会、也可能不会导致
    /// 非持锁者阻塞。
    ///
    /// 如果此文件句柄/描述符，或它的某个克隆，已经持有一个锁，那么确切行为是未指定
    /// 且依赖平台的，包括可能发生死锁。不过，只要此方法返回，就表示持有了一个独占锁。
    ///
    /// 如果文件不是为写入而打开的，此函数是否返回错误是未指定的。
    ///
    /// 当此文件（连同任何从它复制或继承的其他文件描述符/句柄）被关闭，或调用了
    /// [`unlock`] 方法时，锁将被释放。
    ///
    /// # 平台特定行为
    ///
    /// 此函数目前在 Unix 上对应带 `LOCK_EX` 标志的 `flock` 函数，在 Windows 上
    /// 对应带 `LOCKFILE_EXCLUSIVE_LOCK` 标志的 `LockFileEx` 函数。注意，这
    /// [未来可能改变][changes]。
    ///
    /// 在 Windows 上，如果文件仅以追加方式打开，则对其加锁会失败。要给文件加锁，
    /// 请使用 `.read(true)`、`.read(true).append(true)` 或 `.write(true)`
    /// 之一来打开它。
    ///
    /// [changes]: io#platform-specific-behavior
    ///
    /// [`lock`]: File::lock
    /// [`lock_shared`]: File::lock_shared
    /// [`try_lock`]: File::try_lock
    /// [`try_lock_shared`]: File::try_lock_shared
    /// [`unlock`]: File::unlock
    /// [`read`]: Read::read
    /// [`write`]: Write::write
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs::File;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let f = File::create("foo.txt")?;
    ///     f.lock()?;
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "file_lock", since = "1.89.0")]
    pub fn lock(&self) -> io::Result<()> {
        self.inner.lock()
    }

    /// 获取文件上的共享（非独占）锁。阻塞直到能够获取该锁为止。
    ///
    /// 这会获取一个共享锁；可以有多个文件句柄同时持有共享锁，但任何句柄都不能同时
    /// 持有独占锁。
    ///
    /// 此锁可能是建议性的，也可能是强制性的。此锁意在与 [`lock`]、[`try_lock`]、
    /// [`lock_shared`]、[`try_lock_shared`] 和 [`unlock`] 相互配合。它与其他方法
    /// （例如 [`read`] 和 [`write`]）的交互是平台特定的，可能会、也可能不会导致
    /// 非持锁者阻塞。
    ///
    /// 如果此文件句柄/描述符，或它的某个克隆，已经持有一个锁，那么确切行为是未指定
    /// 且依赖平台的，包括可能发生死锁。不过，只要此方法返回，就表示持有了一个共享锁。
    ///
    /// 当此文件（连同任何从它复制或继承的其他文件描述符/句柄）被关闭，或调用了
    /// [`unlock`] 方法时，锁将被释放。
    ///
    /// # 平台特定行为
    ///
    /// 此函数目前在 Unix 上对应带 `LOCK_SH` 标志的 `flock` 函数，在 Windows 上
    /// 对应 `LockFileEx` 函数。注意，这
    /// [未来可能改变][changes]。
    ///
    /// 在 Windows 上，如果文件仅以追加方式打开，则对其加锁会失败。要给文件加锁，
    /// 请使用 `.read(true)`、`.read(true).append(true)` 或 `.write(true)`
    /// 之一来打开它。
    ///
    /// [changes]: io#platform-specific-behavior
    ///
    /// [`lock`]: File::lock
    /// [`lock_shared`]: File::lock_shared
    /// [`try_lock`]: File::try_lock
    /// [`try_lock_shared`]: File::try_lock_shared
    /// [`unlock`]: File::unlock
    /// [`read`]: Read::read
    /// [`write`]: Write::write
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs::File;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let f = File::open("foo.txt")?;
    ///     f.lock_shared()?;
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "file_lock", since = "1.89.0")]
    pub fn lock_shared(&self) -> io::Result<()> {
        self.inner.lock_shared()
    }

    /// 尝试获取文件上的独占锁。
    ///
    /// 如果此文件上已经持有不同的锁（通过另一个句柄/描述符），返回
    /// `Err(TryLockError::WouldBlock)`。
    ///
    /// 这会获取一个独占锁；指向此文件的其他任何文件句柄都不能再获取任何锁。
    ///
    /// 此锁可能是建议性的，也可能是强制性的。此锁意在与 [`lock`]、[`try_lock`]、
    /// [`lock_shared`]、[`try_lock_shared`] 和 [`unlock`] 相互配合。它与其他方法
    /// （例如 [`read`] 和 [`write`]）的交互是平台特定的，可能会、也可能不会导致
    /// 非持锁者阻塞。
    ///
    /// 如果此文件句柄/描述符，或它的某个克隆，已经持有一个锁，那么确切行为是未指定
    /// 且依赖平台的，包括可能发生死锁。不过，只要此方法返回 `Ok(())`，就表示它已经
    /// 获取了一个独占锁。
    ///
    /// 如果文件不是为写入而打开的，此函数是否返回错误是未指定的。
    ///
    /// 当此文件（连同任何从它复制或继承的其他文件描述符/句柄）被关闭，或调用了
    /// [`unlock`] 方法时，锁将被释放。
    ///
    /// # 平台特定行为
    ///
    /// 此函数目前在 Unix 上对应带 `LOCK_EX` 和 `LOCK_NB` 标志的 `flock` 函数，
    /// 在 Windows 上对应带 `LOCKFILE_EXCLUSIVE_LOCK` 和 `LOCKFILE_FAIL_IMMEDIATELY`
    /// 标志的 `LockFileEx` 函数。注意，这
    /// [未来可能改变][changes]。
    ///
    /// 在 Windows 上，如果文件仅以追加方式打开，则对其加锁会失败。要给文件加锁，
    /// 请使用 `.read(true)`、`.read(true).append(true)` 或 `.write(true)`
    /// 之一来打开它。
    ///
    /// [changes]: io#platform-specific-behavior
    ///
    /// [`lock`]: File::lock
    /// [`lock_shared`]: File::lock_shared
    /// [`try_lock`]: File::try_lock
    /// [`try_lock_shared`]: File::try_lock_shared
    /// [`unlock`]: File::unlock
    /// [`read`]: Read::read
    /// [`write`]: Write::write
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs::{File, TryLockError};
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let f = File::create("foo.txt")?;
    ///     // 显式处理 WouldBlock 错误
    ///     match f.try_lock() {
    ///         Ok(_) => (),
    ///         Err(TryLockError::WouldBlock) => (), // 未获取到锁
    ///         Err(TryLockError::Error(err)) => return Err(err),
    ///     }
    ///     // 或者，将错误作为 io::Error 向上传播
    ///     f.try_lock()?;
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "file_lock", since = "1.89.0")]
    pub fn try_lock(&self) -> Result<(), TryLockError> {
        self.inner.try_lock()
    }

    /// 尝试获取文件上的共享（非独占）锁。
    ///
    /// 如果此文件上已经持有不同的锁（通过另一个句柄/描述符），返回
    /// `Err(TryLockError::WouldBlock)`。
    ///
    /// 这会获取一个共享锁；可以有多个文件句柄同时持有共享锁，但任何句柄都不能同时
    /// 持有独占锁。
    ///
    /// 此锁可能是建议性的，也可能是强制性的。此锁意在与 [`lock`]、[`try_lock`]、
    /// [`lock_shared`]、[`try_lock_shared`] 和 [`unlock`] 相互配合。它与其他方法
    /// （例如 [`read`] 和 [`write`]）的交互是平台特定的，可能会、也可能不会导致
    /// 非持锁者阻塞。
    ///
    /// 如果此文件句柄，或它的某个克隆，已经持有一个锁，那么确切行为是未指定
    /// 且依赖平台的，包括可能发生死锁。不过，只要此方法返回 `Ok(())`，就表示它已经
    /// 获取了一个共享锁。
    ///
    /// 当此文件（连同任何从它复制或继承的其他文件描述符/句柄）被关闭，或调用了
    /// [`unlock`] 方法时，锁将被释放。
    ///
    /// # 平台特定行为
    ///
    /// 此函数目前在 Unix 上对应带 `LOCK_SH` 和 `LOCK_NB` 标志的 `flock` 函数，
    /// 在 Windows 上对应带 `LOCKFILE_FAIL_IMMEDIATELY` 标志的 `LockFileEx`
    /// 函数。注意，这
    /// [未来可能改变][changes]。
    ///
    /// 在 Windows 上，如果文件仅以追加方式打开，则对其加锁会失败。要给文件加锁，
    /// 请使用 `.read(true)`、`.read(true).append(true)` 或 `.write(true)`
    /// 之一来打开它。
    ///
    /// [changes]: io#platform-specific-behavior
    ///
    /// [`lock`]: File::lock
    /// [`lock_shared`]: File::lock_shared
    /// [`try_lock`]: File::try_lock
    /// [`try_lock_shared`]: File::try_lock_shared
    /// [`unlock`]: File::unlock
    /// [`read`]: Read::read
    /// [`write`]: Write::write
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs::{File, TryLockError};
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let f = File::open("foo.txt")?;
    ///     // 显式处理 WouldBlock 错误
    ///     match f.try_lock_shared() {
    ///         Ok(_) => (),
    ///         Err(TryLockError::WouldBlock) => (), // 未获取到锁
    ///         Err(TryLockError::Error(err)) => return Err(err),
    ///     }
    ///     // 或者，将错误作为 io::Error 向上传播
    ///     f.try_lock_shared()?;
    ///
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "file_lock", since = "1.89.0")]
    pub fn try_lock_shared(&self) -> Result<(), TryLockError> {
        self.inner.try_lock_shared()
    }

    /// 释放文件上的所有锁。
    ///
    /// 当文件（连同任何从它复制或继承的其他文件描述符/句柄）被关闭时，所有锁都会被
    /// 释放。此方法允许在不关闭文件的情况下释放锁。
    ///
    /// 如果当前没有通过此文件描述符/句柄持有任何锁，此方法可能返回一个错误，也可能
    /// 成功返回而不采取任何动作。
    ///
    /// # 平台特定行为
    ///
    /// 此函数目前在 Unix 上对应带 `LOCK_UN` 标志的 `flock` 函数，在 Windows 上
    /// 对应 `UnlockFile` 函数。注意，这
    /// [未来可能改变][changes]。
    ///
    /// 在 Windows 上，如果文件仅以追加方式打开，则对其加锁会失败。要给文件加锁，
    /// 请使用 `.read(true)`、`.read(true).append(true)` 或 `.write(true)`
    /// 之一来打开它。
    ///
    /// [changes]: io#platform-specific-behavior
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs::File;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let f = File::open("foo.txt")?;
    ///     f.lock()?;
    ///     f.unlock()?;
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "file_lock", since = "1.89.0")]
    pub fn unlock(&self) -> io::Result<()> {
        self.inner.unlock()
    }

    /// 截断或扩展底层文件，将此文件的大小更新为 `size`。
    ///
    /// 如果 `size` 小于当前文件大小，文件会被缩小。如果它大于当前文件大小，文件会被
    /// 扩展到 `size`，并将其间的所有数据用 0 填充。
    ///
    /// 文件游标不会改变。特别地，如果游标位于末尾，而文件因本操作被缩小，那么游标
    /// 此时将位于末尾之后。
    ///
    /// # 错误(Errors）
    ///
    /// 如果文件不是为写入而打开的，此函数将返回一个错误。
    /// 另外，如果由于实现细节导致期望的长度会发生溢出，将返回
    /// [`std::io::ErrorKind::InvalidInput`](crate::io::ErrorKind::InvalidInput)。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs::File;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let mut f = File::create("foo.txt")?;
    ///     f.set_len(10)?;
    ///     Ok(())
    /// }
    /// ```
    ///
    /// 注意，尽管此方法接收的是 `&self` 而非 `&mut self`，它仍会改变底层文件的内容。
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn set_len(&self, size: u64) -> io::Result<()> {
        self.inner.truncate(size)
    }

    /// 查询底层文件的元数据。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs::File;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let mut f = File::open("foo.txt")?;
    ///     let metadata = f.metadata()?;
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn metadata(&self) -> io::Result<Metadata> {
        self.inner.file_attr().map(Metadata)
    }

    /// 创建一个新的 `File` 实例，它与已有的 `File` 实例共享同一个底层文件句柄。
    /// 读、写、seek 会同时影响两个 `File` 实例。
    ///
    /// # 示例
    ///
    /// 为名为 `foo.txt` 的文件创建两个句柄：
    ///
    /// ```no_run
    /// use std::fs::File;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let mut file = File::open("foo.txt")?;
    ///     let file_copy = file.try_clone()?;
    ///     Ok(())
    /// }
    /// ```
    ///
    /// 假设有一个名为 `foo.txt` 的文件，内容为 `abcdef\n`，创建两个句柄，对其中
    /// 一个执行 seek，然后从另一个句柄读取剩余的字节：
    ///
    /// ```no_run
    /// use std::fs::File;
    /// use std::io::SeekFrom;
    /// use std::io::prelude::*;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let mut file = File::open("foo.txt")?;
    ///     let mut file_copy = file.try_clone()?;
    ///
    ///     file.seek(SeekFrom::Start(3))?;
    ///
    ///     let mut contents = vec![];
    ///     file_copy.read_to_end(&mut contents)?;
    ///     assert_eq!(contents, b"def\n");
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "file_try_clone", since = "1.9.0")]
    pub fn try_clone(&self) -> io::Result<File> {
        Ok(File { inner: self.inner.duplicate()? })
    }

    /// 更改底层文件的权限。
    ///
    /// # 平台特定行为
    ///
    /// 此函数目前在 Unix 上对应 `fchmod` 函数，在 Windows 上对应
    /// `SetFileInformationByHandle` 函数。注意，这
    /// [未来可能改变][changes]。
    ///
    /// [changes]: io#platform-specific-behavior
    ///
    /// # 错误(Errors）
    ///
    /// 如果用户缺乏更改底层文件属性的权限，此函数将返回一个错误。在其他 OS 特定的
    /// 未指定情形下也可能返回错误。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// fn main() -> std::io::Result<()> {
    ///     use std::fs::File;
    ///
    ///     let file = File::open("foo.txt")?;
    ///     let mut perms = file.metadata()?.permissions();
    ///     perms.set_readonly(true);
    ///     file.set_permissions(perms)?;
    ///     Ok(())
    /// }
    /// ```
    ///
    /// 注意，尽管此方法接收的是 `&self` 而非 `&mut self`，它仍会改变底层文件的权限。
    #[doc(alias = "fchmod", alias = "SetFileInformationByHandle")]
    #[stable(feature = "set_permissions_atomic", since = "1.16.0")]
    pub fn set_permissions(&self, perm: Permissions) -> io::Result<()> {
        self.inner.set_permissions(perm.0)
    }

    /// 更改底层文件的时间戳。
    ///
    /// # 平台特定行为
    ///
    /// 此函数目前在 Unix 上对应 `futimens` 函数（在 10.13 之前的 macOS 上回退到
    /// `futimes`），在 Windows 上对应 `SetFileTime` 函数。注意，这
    /// [未来可能改变][changes]。
    ///
    /// 在大多数平台上，包括 UNIX 和 Windows 平台，此函数也可以更改目录的时间戳。
    /// 要获得一个代表目录的 `File` 以便调用 `set_times`，请用 `File::open` 打开
    /// 该目录，且不要尝试获取写权限。
    ///
    /// [changes]: io#platform-specific-behavior
    ///
    /// # 错误(Errors）
    ///
    /// 如果用户缺乏更改底层文件时间戳的权限，此函数将返回一个错误。在其他 OS 特定的
    /// 未指定情形下也可能返回错误。
    ///
    /// 如果操作系统缺乏更改 `FileTimes` 结构中所设置的一个或多个时间戳的支持，
    /// 此函数可能返回一个错误。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// fn main() -> std::io::Result<()> {
    ///     use std::fs::{self, File, FileTimes};
    ///
    ///     let src = fs::metadata("src")?;
    ///     let dest = File::open("dest")?;
    ///     let times = FileTimes::new()
    ///         .set_accessed(src.accessed()?)
    ///         .set_modified(src.modified()?);
    ///     dest.set_times(times)?;
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "file_set_times", since = "1.75.0")]
    #[doc(alias = "futimens")]
    #[doc(alias = "futimes")]
    #[doc(alias = "SetFileTime")]
    pub fn set_times(&self, times: FileTimes) -> io::Result<()> {
        self.inner.set_times(times.0)
    }

    /// 更改底层文件的修改时间。
    ///
    /// 这是 `set_times(FileTimes::new().set_modified(time))` 的别名。
    #[stable(feature = "file_set_times", since = "1.75.0")]
    #[inline]
    pub fn set_modified(&self, time: SystemTime) -> io::Result<()> {
        self.set_times(FileTimes::new().set_modified(time))
    }
}

// 除了这里的各 `impl` 之外，`File` 在 Unix 和 WASI 上还有
// `AsFd`/`From<OwnedFd>`/`Into<OwnedFd>` 以及
// `AsRawFd`/`IntoRawFd`/`FromRawFd` 的 `impl`，在 Windows 上则有
// `AsHandle`/`From<OwnedHandle>`/`Into<OwnedHandle>` 以及
// `AsRawHandle`/`IntoRawHandle`/`FromRawHandle` 的 `impl`。

impl AsInner<fs_imp::File> for File {
    #[inline]
    fn as_inner(&self) -> &fs_imp::File {
        &self.inner
    }
}
impl FromInner<fs_imp::File> for File {
    fn from_inner(f: fs_imp::File) -> File {
        File { inner: f }
    }
}
impl IntoInner<fs_imp::File> for File {
    fn into_inner(self) -> fs_imp::File {
        self.inner
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl fmt::Debug for File {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}

/// 指示还需要多少额外容量才能读取文件的剩余部分。
fn buffer_capacity_required(mut file: &File) -> Option<usize> {
    let size = file.metadata().map(|m| m.len()).ok()?;
    let pos = file.stream_position().ok()?;
    // 无需担心 `usize` 溢出，因为在那种情况下读取无论如何都会失败。
    Some(size.saturating_sub(pos) as usize)
}

#[stable(feature = "rust1", since = "1.0.0")]
impl Read for &File {
    /// 从文件读取若干字节。
    ///
    /// 更多信息见 [`Read::read`] 文档。
    ///
    /// # 平台特定行为
    ///
    /// 此函数目前在 Unix 上对应 `read` 函数，在 Windows 上对应 `NtReadFile`
    /// 函数。注意，这[未来可能改变][changes]。
    ///
    /// [changes]: io#platform-specific-behavior
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buf)
    }

    /// 与 `read` 类似，区别在于它读取到一个缓冲区切片中。
    ///
    /// 更多信息见 [`Read::read_vectored`] 文档。
    ///
    /// # 平台特定行为
    ///
    /// 此函数目前在 Unix 上对应 `readv` 函数，在 Windows 上回退到 `read`
    /// 实现。注意，这
    /// [未来可能改变][changes]。
    ///
    /// [changes]: io#platform-specific-behavior
    #[inline]
    fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        self.inner.read_vectored(bufs)
    }

    #[inline]
    fn read_buf(&mut self, cursor: BorrowedCursor<'_>) -> io::Result<()> {
        self.inner.read_buf(cursor)
    }

    /// 判断 `File` 是否具有高效的 `read_vectored` 实现。
    ///
    /// 更多信息见 [`Read::is_read_vectored`] 文档。
    ///
    /// # 平台特定行为
    ///
    /// 此函数目前在 Unix 上返回 `true`，在 Windows 上返回 `false`。
    /// 注意，这[未来可能改变][changes]。
    ///
    /// [changes]: io#platform-specific-behavior
    #[inline]
    fn is_read_vectored(&self) -> bool {
        self.inner.is_read_vectored()
    }

    // 在文件大小可用时，根据其大小预留缓冲区空间。
    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> io::Result<usize> {
        let size = buffer_capacity_required(self);
        buf.try_reserve(size.unwrap_or(0))?;
        io::default_read_to_end(self, buf, size)
    }

    // 在文件大小可用时，根据其大小预留缓冲区空间。
    fn read_to_string(&mut self, buf: &mut String) -> io::Result<usize> {
        let size = buffer_capacity_required(self);
        buf.try_reserve(size.unwrap_or(0))?;
        io::default_read_to_string(self, buf, size)
    }
}
#[stable(feature = "rust1", since = "1.0.0")]
impl Write for &File {
    /// 向文件写入若干字节。
    ///
    /// 更多信息见 [`Write::write`] 文档。
    ///
    /// # 平台特定行为
    ///
    /// 此函数目前在 Unix 上对应 `write` 函数，在 Windows 上对应 `NtWriteFile`
    /// 函数。注意，这[未来可能改变][changes]。
    ///
    /// [changes]: io#platform-specific-behavior
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.write(buf)
    }

    /// 与 `write` 类似，区别在于它从一个缓冲区切片写入。
    ///
    /// 更多信息见 [`Write::write_vectored`] 文档。
    ///
    /// # 平台特定行为
    ///
    /// 此函数目前在 Unix 上对应 `writev` 函数，在 Windows 上回退到 `write`
    /// 实现。注意，这
    /// [未来可能改变][changes]。
    ///
    /// [changes]: io#platform-specific-behavior
    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        self.inner.write_vectored(bufs)
    }

    /// 判断 `File` 是否具有高效的 `write_vectored` 实现。
    ///
    /// 更多信息见 [`Write::is_write_vectored`] 文档。
    ///
    /// # 平台特定行为
    ///
    /// 此函数目前在 Unix 上返回 `true`，在 Windows 上返回 `false`。
    /// 注意，这[未来可能改变][changes]。
    ///
    /// [changes]: io#platform-specific-behavior
    #[inline]
    fn is_write_vectored(&self) -> bool {
        self.inner.is_write_vectored()
    }

    /// 刷新文件，确保所有中间缓冲的内容到达其目的地。
    ///
    /// 更多信息见 [`Write::flush`] 文档。
    ///
    /// # 平台特定行为
    ///
    /// 由于 `File` 结构不含任何缓冲区，此函数目前在 Unix 和 Windows 上都是
    /// 空操作（no-op）。注意，这[未来可能改变][changes]。
    ///
    /// [changes]: io#platform-specific-behavior
    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}
#[stable(feature = "rust1", since = "1.0.0")]
impl Seek for &File {
    /// 在文件中按字节 seek 到某个偏移处。
    ///
    /// 更多信息见 [`Seek::seek`] 文档。
    ///
    /// # 平台特定行为
    ///
    /// 此函数目前在 Unix 上对应 `lseek64` 函数，在 Windows 上对应
    /// `SetFilePointerEx` 函数。注意，这[未来
    /// 可能改变][changes]。
    ///
    /// [changes]: io#platform-specific-behavior
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.inner.seek(pos)
    }

    /// 返回此文件的长度（以字节为单位）。
    ///
    /// 更多信息见 [`Seek::stream_len`] 文档。
    ///
    /// # 平台特定行为
    ///
    /// 此函数目前在 Linux 上对应 `statx` 函数（带回退），在 Windows 上对应
    /// `GetFileSizeEx` 函数。注意，
    /// 这[未来可能改变][changes]。
    ///
    /// [changes]: io#platform-specific-behavior
    fn stream_len(&mut self) -> io::Result<u64> {
        if let Some(result) = self.inner.size() {
            return result;
        }
        io::stream_len_default(self)
    }

    fn stream_position(&mut self) -> io::Result<u64> {
        self.inner.tell()
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl Read for File {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        (&*self).read(buf)
    }
    fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        (&*self).read_vectored(bufs)
    }
    fn read_buf(&mut self, cursor: BorrowedCursor<'_>) -> io::Result<()> {
        (&*self).read_buf(cursor)
    }
    #[inline]
    fn is_read_vectored(&self) -> bool {
        (&&*self).is_read_vectored()
    }
    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> io::Result<usize> {
        (&*self).read_to_end(buf)
    }
    fn read_to_string(&mut self, buf: &mut String) -> io::Result<usize> {
        (&*self).read_to_string(buf)
    }
}
#[stable(feature = "rust1", since = "1.0.0")]
impl Write for File {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        (&*self).write(buf)
    }
    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        (&*self).write_vectored(bufs)
    }
    #[inline]
    fn is_write_vectored(&self) -> bool {
        (&&*self).is_write_vectored()
    }
    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        (&*self).flush()
    }
}
#[stable(feature = "rust1", since = "1.0.0")]
impl Seek for File {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        (&*self).seek(pos)
    }
    fn stream_len(&mut self) -> io::Result<u64> {
        (&*self).stream_len()
    }
    fn stream_position(&mut self) -> io::Result<u64> {
        (&*self).stream_position()
    }
}

#[stable(feature = "io_traits_arc", since = "1.73.0")]
impl Read for Arc<File> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        (&**self).read(buf)
    }
    fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        (&**self).read_vectored(bufs)
    }
    fn read_buf(&mut self, cursor: BorrowedCursor<'_>) -> io::Result<()> {
        (&**self).read_buf(cursor)
    }
    #[inline]
    fn is_read_vectored(&self) -> bool {
        (&**self).is_read_vectored()
    }
    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> io::Result<usize> {
        (&**self).read_to_end(buf)
    }
    fn read_to_string(&mut self, buf: &mut String) -> io::Result<usize> {
        (&**self).read_to_string(buf)
    }
}
#[stable(feature = "io_traits_arc", since = "1.73.0")]
impl Write for Arc<File> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        (&**self).write(buf)
    }
    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        (&**self).write_vectored(bufs)
    }
    #[inline]
    fn is_write_vectored(&self) -> bool {
        (&**self).is_write_vectored()
    }
    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        (&**self).flush()
    }
}
#[stable(feature = "io_traits_arc", since = "1.73.0")]
impl Seek for Arc<File> {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        (&**self).seek(pos)
    }
    fn stream_len(&mut self) -> io::Result<u64> {
        (&**self).stream_len()
    }
    fn stream_position(&mut self) -> io::Result<u64> {
        (&**self).stream_position()
    }
}

impl Dir {
    /// 尝试以只读模式打开 `path` 处的一个目录。
    ///
    /// # 错误(Errors）
    ///
    /// 如果 `path` 没有指向一个已存在的目录，此函数将返回一个错误。
    /// 根据 [`OpenOptions::open`]，也可能返回其他错误。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// #![feature(dirfd)]
    /// use std::{fs::Dir, io};
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let dir = Dir::open("foo")?;
    ///     let mut f = dir.open_file("bar.txt")?;
    ///     let contents = io::read_to_string(f)?;
    ///     assert_eq!(contents, "Hello, world!");
    ///     Ok(())
    /// }
    /// ```
    #[unstable(feature = "dirfd", issue = "120426")]
    pub fn open<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        fs_imp::Dir::open(path.as_ref(), &OpenOptions::new().read(true).0)
            .map(|inner| Self { inner })
    }

    /// 尝试相对于此目录以只读模式打开一个文件。
    ///
    /// # 错误(Errors）
    ///
    /// 如果 `path` 没有指向一个已存在的文件，此函数将返回一个错误。
    /// 根据 [`OpenOptions::open`]，也可能返回其他错误。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// #![feature(dirfd)]
    /// use std::{fs::Dir, io};
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let dir = Dir::open("foo")?;
    ///     let mut f = dir.open_file("bar.txt")?;
    ///     let contents = io::read_to_string(f)?;
    ///     assert_eq!(contents, "Hello, world!");
    ///     Ok(())
    /// }
    /// ```
    #[unstable(feature = "dirfd", issue = "120426")]
    pub fn open_file<P: AsRef<Path>>(&self, path: P) -> io::Result<File> {
        self.inner
            .open_file(path.as_ref(), &OpenOptions::new().read(true).0)
            .map(|f| File { inner: f })
    }
}

impl AsInner<fs_imp::Dir> for Dir {
    #[inline]
    fn as_inner(&self) -> &fs_imp::Dir {
        &self.inner
    }
}
impl FromInner<fs_imp::Dir> for Dir {
    fn from_inner(f: fs_imp::Dir) -> Dir {
        Dir { inner: f }
    }
}
impl IntoInner<fs_imp::Dir> for Dir {
    fn into_inner(self) -> fs_imp::Dir {
        self.inner
    }
}

#[unstable(feature = "dirfd", issue = "120426")]
impl fmt::Debug for Dir {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}

impl OpenOptions {
    /// 创建一组空白的、准备好进行配置的新选项。
    ///
    /// 所有选项初始都被设置为 `false`。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs::OpenOptions;
    ///
    /// let mut options = OpenOptions::new();
    /// let file = options.read(true).open("foo.txt");
    /// ```
    #[cfg_attr(not(test), rustc_diagnostic_item = "open_options_new")]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[must_use]
    pub fn new() -> Self {
        OpenOptions(fs_imp::OpenOptions::new())
    }

    /// 设置读访问选项。
    ///
    /// 此选项为 true 时，表示文件被打开后应当是可 `read` 的。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs::OpenOptions;
    ///
    /// let file = OpenOptions::new().read(true).open("foo.txt");
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn read(&mut self, read: bool) -> &mut Self {
        self.0.read(read);
        self
    }

    /// 设置写访问选项。
    ///
    /// 此选项为 true 时，表示文件被打开后应当是可 `write` 的。
    ///
    /// 如果文件已存在，对它的任何 write 调用都会覆盖其内容，但不会截断它。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs::OpenOptions;
    ///
    /// let file = OpenOptions::new().write(true).open("foo.txt");
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn write(&mut self, write: bool) -> &mut Self {
        self.0.write(write);
        self
    }

    /// 设置追加模式选项。
    ///
    /// 此选项为 true 时，意味着写入将追加到文件而不是覆盖之前的内容。
    /// 注意，设置 `.write(true).append(true)` 与仅设置 `.append(true)`
    /// 效果相同。
    ///
    /// 追加模式保证写入会被定位到文件的当前末尾，即使有其他进程或线程同时在向同一个
    /// 文件追加内容也是如此。这不同于 <code>[seek]\([SeekFrom]::[End]\(0))</code>
    /// 后跟 `write()`：后者在 seek 与 write 之间存在竞态，期间另一个写入者可能写入，
    /// 从而被我们的 `write()` 覆盖其数据。
    ///
    /// 请记住，这并不必然保证不同进程或线程追加的数据不会交错。单次 `write()` 调用
    /// 所接受的数据量取决于操作系统和文件系统。一次成功的 `write()` 允许只写入所给
    /// 数据的一部分，因此即使你小心地在单次 `write()` 调用中提供了完整消息，也无法
    /// 保证它会被完整写出。如果你依赖文件系统在单次写入中接受整条消息，请确保把所有
    /// 属于一体的数据放在一次操作中写入。这可以通过在传给 [`write()`] 之前先把字符串
    /// 拼接起来来实现。
    ///
    /// 如果文件同时以读访问和追加访问打开，要注意：打开之后，以及每次写入之后，读取
    /// 位置可能会被设置在文件末尾。因此，在写入之前，先保存当前位置（使用
    /// <code>[Seek]::[stream_position]</code>），并在下次读取之前将其恢复。
    ///
    /// ## Note
    ///
    /// 如果文件不存在，此函数不会创建它。要创建它，请使用
    /// [`OpenOptions::create`] 方法。
    ///
    /// [`write()`]: Write::write "io::Write::write"
    /// [`flush()`]: Write::flush "io::Write::flush"
    /// [stream_position]: Seek::stream_position "io::Seek::stream_position"
    /// [seek]: Seek::seek "io::Seek::seek"
    /// [Current]: SeekFrom::Current "io::SeekFrom::Current"
    /// [End]: SeekFrom::End "io::SeekFrom::End"
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs::OpenOptions;
    ///
    /// let file = OpenOptions::new().append(true).open("foo.txt");
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn append(&mut self, append: bool) -> &mut Self {
        self.0.append(append);
        self
    }

    /// 设置截断已有文件的选项。
    ///
    /// 如果文件以此选项设为 true 的方式成功打开，且该文件已存在，则会将其截断到
    /// 0 长度。
    ///
    /// 文件必须以写访问打开，截断才能生效。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs::OpenOptions;
    ///
    /// let file = OpenOptions::new().write(true).truncate(true).open("foo.txt");
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn truncate(&mut self, truncate: bool) -> &mut Self {
        self.0.truncate(truncate);
        self
    }

    /// 设置“创建新文件，或在文件已存在时打开它”的选项。
    ///
    /// 为了能创建文件，必须使用 [`OpenOptions::write`] 或
    /// [`OpenOptions::append`] 访问权限。
    ///
    /// 另见 [`std::fs::write()`][self::write]，这是一个用给定数据创建文件的
    /// 简单函数。
    ///
    /// # 错误(Errors）
    ///
    /// 如果设置了 `.create(true)` 而没有设置 `.write(true)` 或 `.append(true)`，
    /// 调用 [`open`](Self::open) 将以 [`InvalidInput`](io::ErrorKind::InvalidInput) 错误失败。
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs::OpenOptions;
    ///
    /// let file = OpenOptions::new().write(true).create(true).open("foo.txt");
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn create(&mut self, create: bool) -> &mut Self {
        self.0.create(create);
        self
    }

    /// 设置“创建新文件，如果文件已存在则失败”的选项。
    ///
    /// 目标位置不允许已存在任何文件，也不允许存在（悬空的）符号链接。这样一来，
    /// 只要调用成功，返回的文件就保证是新创建的。
    /// 如果目标位置已存在一个文件，创建新文件将以 [`AlreadyExists`] 失败，或根据
    /// 具体情况返回其他错误。可能出现的错误的非穷尽列表见 [`OpenOptions::open`]。
    ///
    /// 此选项之所以有用，是因为它是原子的。否则，在检查文件是否存在与创建新文件
    /// 之间，文件可能已被另一个进程创建（一种 [TOCTOU] 竞态条件 / 攻击）。
    ///
    /// 如果设置了 `.create_new(true)`，则 [`.create()`] 和 [`.truncate()`]
    /// 会被忽略。
    ///
    /// 文件必须以写访问或追加访问打开，才能创建新文件。
    ///
    /// [`.create()`]: OpenOptions::create
    /// [`.truncate()`]: OpenOptions::truncate
    /// [`AlreadyExists`]: io::ErrorKind::AlreadyExists
    /// [TOCTOU]: self#time-of-check-to-time-of-use-toctou
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs::OpenOptions;
    ///
    /// let file = OpenOptions::new().write(true)
    ///                              .create_new(true)
    ///                              .open("foo.txt");
    /// ```
    #[stable(feature = "expand_open_options2", since = "1.9.0")]
    pub fn create_new(&mut self, create_new: bool) -> &mut Self {
        self.0.create_new(create_new);
        self
    }

    /// 用 `self` 指定的选项打开 `path` 处的一个文件。
    ///
    /// # 错误(Errors）
    ///
    /// 此函数在多种不同情形下都会返回错误。其中一些错误条件连同它们的
    /// [`io::ErrorKind`] 列举如下。到 [`io::ErrorKind`] 的映射不属于此函数的
    /// 兼容性契约的一部分。
    ///
    /// * [`NotFound`]：指定的文件不存在，且 `create` 或 `create_new` 都未设置。
    /// * [`NotFound`]：文件路径的某个目录组成部分不存在。
    /// * [`PermissionDenied`]：用户缺乏获取文件所指定访问权限的权限。
    /// * [`PermissionDenied`]：用户缺乏打开所指定路径中某个目录组成部分的权限。
    /// * [`AlreadyExists`]：指定了 `create_new` 而文件已存在。
    /// * [`InvalidInput`]：打开选项的无效组合（在无写访问的情况下 truncate、
    ///   在无写访问或追加访问的情况下 create、未设置任何访问模式等）。
    ///
    /// 以下错误目前不匹配任何已有的 [`io::ErrorKind`]：
    /// * 指定文件路径的某个目录组成部分实际上并不是一个目录。
    /// * 文件系统层级的错误：磁盘已满、在只读文件系统上请求了写权限、超出磁盘配额、
    ///   打开的文件过多、文件名过长、所指定路径中符号链接过多（仅限类 Unix 系统）等。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs::OpenOptions;
    ///
    /// let file = OpenOptions::new().read(true).open("foo.txt");
    /// ```
    ///
    /// [`AlreadyExists`]: io::ErrorKind::AlreadyExists
    /// [`InvalidInput`]: io::ErrorKind::InvalidInput
    /// [`NotFound`]: io::ErrorKind::NotFound
    /// [`PermissionDenied`]: io::ErrorKind::PermissionDenied
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn open<P: AsRef<Path>>(&self, path: P) -> io::Result<File> {
        self._open(path.as_ref())
    }

    fn _open(&self, path: &Path) -> io::Result<File> {
        fs_imp::File::open(path, &self.0).map(|inner| File { inner })
    }
}

impl AsInner<fs_imp::OpenOptions> for OpenOptions {
    #[inline]
    fn as_inner(&self) -> &fs_imp::OpenOptions {
        &self.0
    }
}

impl AsInnerMut<fs_imp::OpenOptions> for OpenOptions {
    #[inline]
    fn as_inner_mut(&mut self) -> &mut fs_imp::OpenOptions {
        &mut self.0
    }
}

impl Metadata {
    /// 返回此元数据对应的文件类型。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// fn main() -> std::io::Result<()> {
    ///     use std::fs;
    ///
    ///     let metadata = fs::metadata("foo.txt")?;
    ///
    ///     println!("{:?}", metadata.file_type());
    ///     Ok(())
    /// }
    /// ```
    #[must_use]
    #[stable(feature = "file_type", since = "1.1.0")]
    pub fn file_type(&self) -> FileType {
        FileType(self.0.file_type())
    }

    /// 如果此元数据对应的是一个目录，返回 `true`。该结果与
    /// [`Metadata::is_file`] 的结果互斥；对于通过 [`symlink_metadata`] 获取的
    /// 符号链接元数据，此方法将返回 false。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// fn main() -> std::io::Result<()> {
    ///     use std::fs;
    ///
    ///     let metadata = fs::metadata("foo.txt")?;
    ///
    ///     assert!(!metadata.is_dir());
    ///     Ok(())
    /// }
    /// ```
    #[must_use]
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn is_dir(&self) -> bool {
        self.file_type().is_dir()
    }

    /// 如果此元数据对应的是一个常规文件，返回 `true`。该结果与
    /// [`Metadata::is_dir`] 的结果互斥；对于通过 [`symlink_metadata`] 获取的
    /// 符号链接元数据，此方法将返回 false。
    ///
    /// 当目标仅仅是从源读取（或向源写入）时，测试源是否可读（或可写）的最可靠方式
    /// 是打开它。例如在类 Unix 系统上，仅使用 `is_file` 可能会破坏像
    /// `diff <( prog_a )` 这样的工作流。更多信息见 [`File::open`] 或
    /// [`OpenOptions::open`]。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let metadata = fs::metadata("foo.txt")?;
    ///
    ///     assert!(metadata.is_file());
    ///     Ok(())
    /// }
    /// ```
    #[must_use]
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn is_file(&self) -> bool {
        self.file_type().is_file()
    }

    /// 如果此元数据对应的是一个符号链接，返回 `true`。
    ///
    /// # 示例
    ///
    #[cfg_attr(unix, doc = "```no_run")]
    #[cfg_attr(not(unix), doc = "```ignore")]
    /// use std::fs;
    /// use std::path::Path;
    /// use std::os::unix::fs::symlink;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let link_path = Path::new("link");
    ///     symlink("/origin_does_not_exist/", link_path)?;
    ///
    ///     let metadata = fs::symlink_metadata(link_path)?;
    ///
    ///     assert!(metadata.is_symlink());
    ///     Ok(())
    /// }
    /// ```
    #[must_use]
    #[stable(feature = "is_symlink", since = "1.58.0")]
    pub fn is_symlink(&self) -> bool {
        self.file_type().is_symlink()
    }

    /// 返回此元数据所对应文件的大小（以字节为单位）。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let metadata = fs::metadata("foo.txt")?;
    ///
    ///     assert_eq!(0, metadata.len());
    ///     Ok(())
    /// }
    /// ```
    #[must_use]
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn len(&self) -> u64 {
        self.0.size()
    }

    /// 返回此元数据所对应文件的权限。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let metadata = fs::metadata("foo.txt")?;
    ///
    ///     assert!(!metadata.permissions().readonly());
    ///     Ok(())
    /// }
    /// ```
    #[must_use]
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn permissions(&self) -> Permissions {
        Permissions(self.0.perm())
    }

    /// 返回此元数据中所列出的最后修改时间。
    ///
    /// 返回值在 Unix 平台上对应 `stat` 的 `mtime` 字段，在 Windows 平台上对应
    /// `ftLastWriteTime` 字段。
    ///
    /// # 错误(Errors）
    ///
    /// 此字段可能并非在所有平台上都可用，在不可用的平台上将返回一个 `Err`。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let metadata = fs::metadata("foo.txt")?;
    ///
    ///     if let Ok(time) = metadata.modified() {
    ///         println!("{time:?}");
    ///     } else {
    ///         println!("Not supported on this platform");
    ///     }
    ///     Ok(())
    /// }
    /// ```
    #[doc(alias = "mtime", alias = "ftLastWriteTime")]
    #[stable(feature = "fs_time", since = "1.10.0")]
    pub fn modified(&self) -> io::Result<SystemTime> {
        self.0.modified().map(FromInner::from_inner)
    }

    /// 返回此元数据的最后访问时间。
    ///
    /// 返回值在 Unix 平台上对应 `stat` 的 `atime` 字段，在 Windows 平台上对应
    /// `ftLastAccessTime` 字段。
    ///
    /// 注意，并非所有平台都会在文件元数据中持续更新此字段，例如 Windows 有一个选项
    /// 可以在访问文件时禁用更新该时间，Linux 也类似地有 `noatime`。
    ///
    /// # 错误(Errors）
    ///
    /// 此字段可能并非在所有平台上都可用，在不可用的平台上将返回一个 `Err`。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let metadata = fs::metadata("foo.txt")?;
    ///
    ///     if let Ok(time) = metadata.accessed() {
    ///         println!("{time:?}");
    ///     } else {
    ///         println!("Not supported on this platform");
    ///     }
    ///     Ok(())
    /// }
    /// ```
    #[doc(alias = "atime", alias = "ftLastAccessTime")]
    #[stable(feature = "fs_time", since = "1.10.0")]
    pub fn accessed(&self) -> io::Result<SystemTime> {
        self.0.accessed().map(FromInner::from_inner)
    }

    /// 返回此元数据中所列出的创建时间。
    ///
    /// 返回值在 4.11 及以上的 Linux 内核上对应 `statx` 的 `btime` 字段，在其他
    /// Unix 平台上对应 `stat` 的 `birthtime` 字段，在 Windows 平台上对应
    /// `ftCreationTime` 字段。
    ///
    /// # 错误(Errors）
    ///
    /// 此字段可能并非在所有平台上都可用，在不可用的平台或文件系统上将返回一个 `Err`。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let metadata = fs::metadata("foo.txt")?;
    ///
    ///     if let Ok(time) = metadata.created() {
    ///         println!("{time:?}");
    ///     } else {
    ///         println!("Not supported on this platform or filesystem");
    ///     }
    ///     Ok(())
    /// }
    /// ```
    #[doc(alias = "btime", alias = "birthtime", alias = "ftCreationTime")]
    #[stable(feature = "fs_time", since = "1.10.0")]
    pub fn created(&self) -> io::Result<SystemTime> {
        self.0.created().map(FromInner::from_inner)
    }
}

#[stable(feature = "std_debug", since = "1.16.0")]
impl fmt::Debug for Metadata {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut debug = f.debug_struct("Metadata");
        debug.field("file_type", &self.file_type());
        debug.field("permissions", &self.permissions());
        debug.field("len", &self.len());
        if let Ok(modified) = self.modified() {
            debug.field("modified", &modified);
        }
        if let Ok(accessed) = self.accessed() {
            debug.field("accessed", &accessed);
        }
        if let Ok(created) = self.created() {
            debug.field("created", &created);
        }
        debug.finish_non_exhaustive()
    }
}

impl AsInner<fs_imp::FileAttr> for Metadata {
    #[inline]
    fn as_inner(&self) -> &fs_imp::FileAttr {
        &self.0
    }
}

impl FromInner<fs_imp::FileAttr> for Metadata {
    fn from_inner(attr: fs_imp::FileAttr) -> Metadata {
        Metadata(attr)
    }
}

impl FileTimes {
    /// 创建一个未设置任何时间的新 `FileTimes`。
    ///
    /// 在 [`File::set_times`] 中使用这样的 `FileTimes` 不会修改任何时间戳。
    #[stable(feature = "file_set_times", since = "1.75.0")]
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置文件的最后访问时间。
    #[stable(feature = "file_set_times", since = "1.75.0")]
    pub fn set_accessed(mut self, t: SystemTime) -> Self {
        self.0.set_accessed(t.into_inner());
        self
    }

    /// 设置文件的最后修改时间。
    #[stable(feature = "file_set_times", since = "1.75.0")]
    pub fn set_modified(mut self, t: SystemTime) -> Self {
        self.0.set_modified(t.into_inner());
        self
    }
}

impl AsInnerMut<fs_imp::FileTimes> for FileTimes {
    fn as_inner_mut(&mut self) -> &mut fs_imp::FileTimes {
        &mut self.0
    }
}

// 用于在 `std::os` 中实现 OS 扩展 trait
#[stable(feature = "file_set_times", since = "1.75.0")]
impl Sealed for FileTimes {}

impl Permissions {
    /// 如果这些权限描述的是一个只读（不可写）文件，返回 `true`。
    ///
    /// # Note
    ///
    /// 此函数不考虑访问控制列表 (ACL)、Unix 组成员关系以及其他细微差别。
    /// 因此，不能依赖此函数的返回值来预测对文件的读取或写入尝试是否真的会成功。
    ///
    /// # Windows
    ///
    /// 在 Windows 上，这返回 [`FILE_ATTRIBUTE_READONLY`](https://docs.microsoft.com/en-us/windows/win32/fileio/file-attribute-constants)。
    /// 如果设置了 `FILE_ATTRIBUTE_READONLY`，那么对文件的写入会失败，但用户仍可能
    /// 有权限更改此标志。如果 `FILE_ATTRIBUTE_READONLY` *未*被设置，写入仍可能因
    /// 缺乏写权限而失败。
    /// 此属性对目录的行为取决于 Windows 版本。
    ///
    /// # Unix (including macOS)
    ///
    /// 在基于 Unix 的平台上，这会检查所有者、组或其他人的写权限位中是否设置了*任意一个*。
    /// 它不考虑其他任何因素，包括：
    ///
    /// * 当前用户是否在文件的所属组中。
    /// * 由 ACL 授予的权限。
    /// * `root` 用户可以写入未设置任何写位的文件。
    /// * 挂载为只读的文件系统上的可写文件。
    ///
    /// [`PermissionsExt`] trait 提供了对权限位的直接访问，但同样不会读取 ACL。
    ///
    /// [`PermissionsExt`]: crate::os::unix::fs::PermissionsExt
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs::File;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let mut f = File::create("foo.txt")?;
    ///     let metadata = f.metadata()?;
    ///
    ///     assert_eq!(false, metadata.permissions().readonly());
    ///     Ok(())
    /// }
    /// ```
    #[must_use = "call `set_readonly` to modify the readonly flag"]
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn readonly(&self) -> bool {
        self.0.readonly()
    }

    /// 修改这组权限的只读标志。如果 `readonly` 参数为 `true`，使用所得到的
    /// `Permission` 会更新文件权限以禁止写入。相反，如果它为 `false`，使用所得到的
    /// `Permission` 会更新文件权限以允许写入。
    ///
    /// 此操作**不会**修改文件的属性。它只会更改该 `Permissions` 实例在内存中的这些
    /// 属性值。要修改文件的属性，请使用 [`set_permissions`] 函数，它会把这些属性
    /// 更改提交到文件。
    ///
    /// # Note
    ///
    /// 在 Unix 上，`set_readonly(false)` 会让文件变为*所有人可写*。
    /// 你可以在 Unix 上使用 [`PermissionsExt`] trait 来避免此问题。
    ///
    /// 它同样不考虑访问控制列表 (ACL) 或 Unix 组成员关系。
    ///
    /// # Windows
    ///
    /// 在 Windows 上，这会设置或清除 [`FILE_ATTRIBUTE_READONLY`](https://docs.microsoft.com/en-us/windows/win32/fileio/file-attribute-constants)。
    /// 如果设置了 `FILE_ATTRIBUTE_READONLY`，那么对文件的写入会失败，但用户仍可能
    /// 有权限更改此标志。如果 `FILE_ATTRIBUTE_READONLY` *未*被设置，那么在用户没有
    /// 写入文件的权限时，写入仍可能失败。
    ///
    /// 在 Windows 7 及更早版本中，此属性会阻止删除空目录。它不会阻止修改目录内容。
    /// 在更新的 Windows 版本中，此属性对目录会被忽略。
    ///
    /// # Unix (including macOS)
    ///
    /// 在基于 Unix 的平台上，这会为所有者、组*和*其他人设置或清除写访问位，等价于
    /// `chmod a+w <file>` 或 `chmod a-w <file>`。后者会向所有用户授予写访问权限！
    /// 你可以在 Unix 上使用 [`PermissionsExt`] trait 来避免此问题。
    ///
    /// [`PermissionsExt`]: crate::os::unix::fs::PermissionsExt
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs::File;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let f = File::create("foo.txt")?;
    ///     let metadata = f.metadata()?;
    ///     let mut permissions = metadata.permissions();
    ///
    ///     permissions.set_readonly(true);
    ///
    ///     // 文件系统不会改变，只改变只读权限的内存中状态
    ///     assert_eq!(false, metadata.permissions().readonly());
    ///
    ///     // 只改变这个特定的 `permissions`。
    ///     assert_eq!(true, permissions.readonly());
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn set_readonly(&mut self, readonly: bool) {
        self.0.set_readonly(readonly)
    }
}

impl FileType {
    /// 测试此文件类型是否表示一个目录。该结果与
    /// [`is_file`] 和 [`is_symlink`] 的结果互斥；这三个测试中最多只有一个会通过。
    ///
    /// [`is_file`]: FileType::is_file
    /// [`is_symlink`]: FileType::is_symlink
    ///
    /// # 示例
    ///
    /// ```no_run
    /// fn main() -> std::io::Result<()> {
    ///     use std::fs;
    ///
    ///     let metadata = fs::metadata("foo.txt")?;
    ///     let file_type = metadata.file_type();
    ///
    ///     assert_eq!(file_type.is_dir(), false);
    ///     Ok(())
    /// }
    /// ```
    #[must_use]
    #[stable(feature = "file_type", since = "1.1.0")]
    pub fn is_dir(&self) -> bool {
        self.0.is_dir()
    }

    /// 测试此文件类型是否表示一个常规文件。
    /// 该结果与 [`is_dir`] 和 [`is_symlink`] 的结果互斥；这三个测试中最多只有一个
    /// 会通过。
    ///
    /// 当目标仅仅是从源读取（或向源写入）时，测试源是否可读（或可写）的最可靠方式
    /// 是打开它。例如在类 Unix 系统上，仅使用 `is_file` 可能会破坏像
    /// `diff <( prog_a )` 这样的工作流。更多信息见 [`File::open`] 或
    /// [`OpenOptions::open`]。
    ///
    /// [`is_dir`]: FileType::is_dir
    /// [`is_symlink`]: FileType::is_symlink
    ///
    /// # 示例
    ///
    /// ```no_run
    /// fn main() -> std::io::Result<()> {
    ///     use std::fs;
    ///
    ///     let metadata = fs::metadata("foo.txt")?;
    ///     let file_type = metadata.file_type();
    ///
    ///     assert_eq!(file_type.is_file(), true);
    ///     Ok(())
    /// }
    /// ```
    #[must_use]
    #[stable(feature = "file_type", since = "1.1.0")]
    pub fn is_file(&self) -> bool {
        self.0.is_file()
    }

    /// 测试此文件类型是否表示一个符号链接。
    /// 该结果与 [`is_dir`] 和 [`is_file`] 的结果互斥；这三个测试中最多只有一个
    /// 会通过。
    ///
    /// 底层的 [`Metadata`] 结构需要用 [`fs::symlink_metadata`] 函数来获取，
    /// 而不是 [`fs::metadata`] 函数。[`fs::metadata`] 函数会跟随符号链接，因此对
    /// 目标文件而言 [`is_symlink`] 总会返回 `false`。
    ///
    /// [`fs::metadata`]: metadata
    /// [`fs::symlink_metadata`]: symlink_metadata
    /// [`is_dir`]: FileType::is_dir
    /// [`is_file`]: FileType::is_file
    /// [`is_symlink`]: FileType::is_symlink
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let metadata = fs::symlink_metadata("foo.txt")?;
    ///     let file_type = metadata.file_type();
    ///
    ///     assert_eq!(file_type.is_symlink(), false);
    ///     Ok(())
    /// }
    /// ```
    #[must_use]
    #[stable(feature = "file_type", since = "1.1.0")]
    pub fn is_symlink(&self) -> bool {
        self.0.is_symlink()
    }
}

#[stable(feature = "std_debug", since = "1.16.0")]
impl fmt::Debug for FileType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FileType")
            .field("is_file", &self.is_file())
            .field("is_dir", &self.is_dir())
            .field("is_symlink", &self.is_symlink())
            .finish_non_exhaustive()
    }
}

impl AsInner<fs_imp::FileType> for FileType {
    #[inline]
    fn as_inner(&self) -> &fs_imp::FileType {
        &self.0
    }
}

impl FromInner<fs_imp::FilePermissions> for Permissions {
    fn from_inner(f: fs_imp::FilePermissions) -> Permissions {
        Permissions(f)
    }
}

impl AsInner<fs_imp::FilePermissions> for Permissions {
    #[inline]
    fn as_inner(&self) -> &fs_imp::FilePermissions {
        &self.0
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl Iterator for ReadDir {
    type Item = io::Result<DirEntry>;

    fn next(&mut self) -> Option<io::Result<DirEntry>> {
        self.0.next().map(|entry| entry.map(DirEntry))
    }
}

impl DirEntry {
    /// 返回此条目所表示文件的完整路径。
    ///
    /// 完整路径是将传给 `read_dir` 的原始路径与此条目的文件名拼接而成的。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     for entry in fs::read_dir(".")? {
    ///         let dir = entry?;
    ///         println!("{:?}", dir.path());
    ///     }
    ///     Ok(())
    /// }
    /// ```
    ///
    /// 这会打印出类似如下的输出：
    ///
    /// ```text
    /// "./whatever.txt"
    /// "./foo.html"
    /// "./hello_world.rs"
    /// ```
    ///
    /// 当然，具体的文本取决于 `.` 中有哪些文件。
    #[must_use]
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn path(&self) -> PathBuf {
        self.0.path()
    }

    /// 返回此条目所指向文件的元数据。
    ///
    /// 如果此条目指向一个符号链接，此函数不会遍历该符号链接。要遍历符号链接，
    /// 请使用 [`fs::metadata`] 或 [`fs::File::metadata`]。
    ///
    /// [`fs::metadata`]: metadata
    /// [`fs::File::metadata`]: File::metadata
    ///
    /// # 平台特定行为
    ///
    /// 在 Windows 上调用此函数代价很低（无需额外的系统调用），但在 Unix 平台上，
    /// 此函数等价于对该路径调用 `symlink_metadata`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::fs;
    ///
    /// if let Ok(entries) = fs::read_dir(".") {
    ///     for entry in entries {
    ///         if let Ok(entry) = entry {
    ///             // 这里 `entry` 是一个 `DirEntry`。
    ///             if let Ok(metadata) = entry.metadata() {
    ///                 // 现在我们来展示该条目的权限！
    ///                 println!("{:?}: {:?}", entry.path(), metadata.permissions());
    ///             } else {
    ///                 println!("Couldn't get metadata for {:?}", entry.path());
    ///             }
    ///         }
    ///     }
    /// }
    /// ```
    #[stable(feature = "dir_entry_ext", since = "1.1.0")]
    pub fn metadata(&self) -> io::Result<Metadata> {
        self.0.metadata().map(Metadata)
    }

    /// 返回此条目所指向文件的文件类型。
    ///
    /// 如果此条目指向一个符号链接，此函数不会遍历该符号链接。
    ///
    /// # 平台特定行为
    ///
    /// 在 Windows 和大多数 Unix 平台上，此函数是无开销的（无需额外的系统调用），
    /// 但某些 Unix 平台可能需要等价于 `symlink_metadata` 的调用才能获知目标的
    /// 文件类型。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::fs;
    ///
    /// if let Ok(entries) = fs::read_dir(".") {
    ///     for entry in entries {
    ///         if let Ok(entry) = entry {
    ///             // 这里 `entry` 是一个 `DirEntry`。
    ///             if let Ok(file_type) = entry.file_type() {
    ///                 // 现在我们来展示该条目的文件类型！
    ///                 println!("{:?}: {:?}", entry.path(), file_type);
    ///             } else {
    ///                 println!("Couldn't get file type for {:?}", entry.path());
    ///             }
    ///         }
    ///     }
    /// }
    /// ```
    #[stable(feature = "dir_entry_ext", since = "1.1.0")]
    pub fn file_type(&self) -> io::Result<FileType> {
        self.0.file_type().map(FileType)
    }

    /// 返回此目录条目的文件名，不带任何前导路径组成部分。
    ///
    /// 举例来说，对于以下所有路径，此函数的输出都将是 "foo"：
    /// - "./foo"
    /// - "/the/foo"
    /// - "../../foo"
    ///
    /// # 示例
    ///
    /// ```
    /// use std::fs;
    ///
    /// if let Ok(entries) = fs::read_dir(".") {
    ///     for entry in entries {
    ///         if let Ok(entry) = entry {
    ///             // 这里 `entry` 是一个 `DirEntry`。
    ///             println!("{:?}", entry.file_name());
    ///         }
    ///     }
    /// }
    /// ```
    #[must_use]
    #[stable(feature = "dir_entry_ext", since = "1.1.0")]
    pub fn file_name(&self) -> OsString {
        self.0.file_name()
    }
}

#[stable(feature = "dir_entry_debug", since = "1.13.0")]
impl fmt::Debug for DirEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("DirEntry").field(&self.path()).finish()
    }
}

impl AsInner<fs_imp::DirEntry> for DirEntry {
    #[inline]
    fn as_inner(&self) -> &fs_imp::DirEntry {
        &self.0
    }
}

/// 从文件系统中移除一个文件。
///
/// 注意，并不保证文件会被立即删除（例如，取决于平台，其他打开的文件描述符可能
/// 会阻止立即移除）。
///
/// # 平台特定行为
///
/// 此函数目前在 Unix 上对应 `unlink` 函数。
/// 在 Windows 上，对只读文件使用 `DeleteFile`，或使用 `CreateFileW` 和
/// `SetInformationByHandle`。
/// 注意，这[未来可能改变][changes]。
///
/// [changes]: io#platform-specific-behavior
///
/// # 错误(Errors）
///
/// 此函数将在以下情形中返回错误，但不限于这些情形：
///
/// * `path` 指向一个目录。
/// * 文件不存在。
/// * 用户缺乏移除该文件的权限。
///
/// 仅当给定路径不存在时，此函数才会返回 `NotFound` 类型的错误。注意，反过来并不
/// 成立，即如果某路径不存在，对它的移除仍可能因多种原因失败，例如权限不足。
///
/// # 示例
///
/// ```no_run
/// use std::fs;
///
/// fn main() -> std::io::Result<()> {
///     fs::remove_file("a.txt")?;
///     Ok(())
/// }
/// ```
#[doc(alias = "rm", alias = "unlink", alias = "DeleteFile")]
#[stable(feature = "rust1", since = "1.0.0")]
pub fn remove_file<P: AsRef<Path>>(path: P) -> io::Result<()> {
    fs_imp::remove_file(path.as_ref())
}

/// 给定一个路径，查询文件系统以获取关于文件、目录等的信息。
///
/// 此函数会遍历符号链接，查询目标文件的信息。
///
/// # 平台特定行为
///
/// 此函数目前在 Unix 上对应 `stat` 函数，在 Windows 上对应
/// `GetFileInformationByHandle` 函数。
/// 注意，这[未来可能改变][changes]。
///
/// [changes]: io#platform-specific-behavior
///
/// # 错误(Errors）
///
/// 此函数将在以下情形中返回错误，但不限于这些情形：
///
/// * 用户缺乏对 `path` 执行 `metadata` 调用的权限。
/// * `path` 不存在。
///
/// # 示例
///
/// ```rust,no_run
/// use std::fs;
///
/// fn main() -> std::io::Result<()> {
///     let attr = fs::metadata("/some/file/path.txt")?;
///     // 检查 attr ...
///     Ok(())
/// }
/// ```
#[doc(alias = "stat")]
#[stable(feature = "rust1", since = "1.0.0")]
pub fn metadata<P: AsRef<Path>>(path: P) -> io::Result<Metadata> {
    fs_imp::metadata(path.as_ref()).map(Metadata)
}

/// 查询关于某个文件的元数据，但不跟随符号链接。
///
/// # 平台特定行为
///
/// 此函数目前在 Unix 上对应 `lstat` 函数，在 Windows 上对应
/// `GetFileInformationByHandle` 函数。
/// 注意，这[未来可能改变][changes]。
///
/// [changes]: io#platform-specific-behavior
///
/// # 错误(Errors）
///
/// 此函数将在以下情形中返回错误，但不限于这些情形：
///
/// * 用户缺乏对 `path` 执行 `metadata` 调用的权限。
/// * `path` 不存在。
///
/// # 示例
///
/// ```rust,no_run
/// use std::fs;
///
/// fn main() -> std::io::Result<()> {
///     let attr = fs::symlink_metadata("/some/file/path.txt")?;
///     // 检查 attr ...
///     Ok(())
/// }
/// ```
#[doc(alias = "lstat")]
#[stable(feature = "symlink_metadata", since = "1.1.0")]
pub fn symlink_metadata<P: AsRef<Path>>(path: P) -> io::Result<Metadata> {
    fs_imp::symlink_metadata(path.as_ref()).map(Metadata)
}

/// 将文件或目录重命名为新名称，如果 `to` 已存在则替换原文件。
///
/// 如果新名称位于不同的挂载点上，此操作将无法成功。
///
/// # 平台特定行为
///
/// 此函数目前在 Unix 上对应 `rename` 函数，在 Windows 上对应 `MoveFileExW`
/// 或 `SetFileInformationByHandle` 函数。
///
/// 正因如此，当 `from` 和 `to` 都存在时，行为有所不同。在 Unix 上，如果 `from`
/// 是目录，那么 `to` 也必须是一个（空）目录；如果 `from` 不是目录，那么 `to`
/// 也必须不是目录。在 Windows 10 1607 及更高版本上，如果文件系统支持
/// `FileRenameInfoEx`，则行为与之相同；否则，`from` 可以是任意类型，但 `to`
/// *不能*是目录。
///
/// 注意，这[未来可能改变][changes]。
///
/// [changes]: io#platform-specific-behavior
///
/// # 错误(Errors）
///
/// 此函数将在以下情形中返回错误，但不限于这些情形：
///
/// * `from` 不存在。
/// * 用户缺乏查看内容的权限。
/// * `from` 和 `to` 位于不同的文件系统上。
///
/// # 示例
///
/// ```no_run
/// use std::fs;
///
/// fn main() -> std::io::Result<()> {
///     fs::rename("a.txt", "b.txt")?; // 将 a.txt 重命名为 b.txt
///     Ok(())
/// }
/// ```
#[doc(alias = "mv", alias = "MoveFile", alias = "MoveFileEx")]
#[stable(feature = "rust1", since = "1.0.0")]
pub fn rename<P: AsRef<Path>, Q: AsRef<Path>>(from: P, to: Q) -> io::Result<()> {
    fs_imp::rename(from.as_ref(), to.as_ref())
}

/// 将一个文件的内容复制到另一个文件。此函数还会将原文件的权限位复制到目标文件。
///
/// 此函数会**覆盖** `to` 的内容。
///
/// 注意，如果 `from` 和 `to` 都指向同一个文件，那么该文件很可能会被此操作截断。
///
/// 成功时，返回复制的总字节数，它等于由 `metadata` 报告的 `to` 文件的长度。
///
/// 如果你想把一个文件的内容复制到另一个文件，且你正在使用 [`File`]，请参见
/// [`io::copy`](io::copy()) 函数。
///
/// # 平台特定行为
///
/// 此函数目前在 Unix 上对应 `open` 函数：对 `from` 使用 `O_RDONLY`，对 `to`
/// 使用 `O_WRONLY`、`O_CREAT` 和 `O_TRUNC`。返回的文件描述符上设置了
/// `O_CLOEXEC`。
///
/// 在 Linux（包括 Android）上，此函数尝试使用 `copy_file_range(2)`，
/// 在不可行时回退到读取和写入。
///
/// 在 Windows 上，此函数目前对应 `CopyFileEx`。备用 NTFS 流会被复制，但此函数
/// 只返回主流的大小。
///
/// 在 MacOS 上，此函数对应 `fclonefileat` 和 `fcopyfile`。
///
/// 注意，平台特定行为[未来可能改变][changes]。
///
/// [changes]: io#platform-specific-behavior
///
/// # 错误(Errors）
///
/// 此函数将在以下情形中返回错误，但不限于这些情形：
///
/// * `from` 既不是常规文件，也不是指向常规文件的符号链接。
/// * `from` 不存在。
/// * 当前进程没有读取 `from` 或写入 `to` 的权限。
/// * `to` 的父目录不存在。
///
/// # 示例
///
/// ```no_run
/// use std::fs;
///
/// fn main() -> std::io::Result<()> {
///     fs::copy("foo.txt", "bar.txt")?;  // 将 foo.txt 复制到 bar.txt
///     Ok(())
/// }
/// ```
#[doc(alias = "cp")]
#[doc(alias = "CopyFile", alias = "CopyFileEx")]
#[doc(alias = "fclonefileat", alias = "fcopyfile")]
#[stable(feature = "rust1", since = "1.0.0")]
pub fn copy<P: AsRef<Path>, Q: AsRef<Path>>(from: P, to: Q) -> io::Result<u64> {
    fs_imp::copy(from.as_ref(), to.as_ref())
}

/// 在文件系统上创建一个新的硬链接。
///
/// `link` 路径将成为一个指向 `original` 路径的链接。注意，系统通常要求这两个路径
/// 都位于同一个文件系统上。
///
/// 如果 `original` 命名的是一个符号链接，那么是否跟随该符号链接取决于平台。在那些
/// 可以不跟随它的平台上，将不跟随它，所创建的硬链接指向符号链接本身。
///
/// # 平台特定行为
///
/// 此函数目前在 Windows 上对应 `CreateHardLink` 函数。
/// 在大多数 Unix 系统上，它对应不带任何标志的 `linkat` 函数。
/// 在 Android、VxWorks 和 Redox 上，它转而对应 `link` 函数。
/// 在 MacOS 上，如果 `linkat` 可用则使用它，但在 `linkat` 不可用的非常老旧的
/// 系统上，会在运行时改选 `link`。
/// 注意，这[未来可能改变][changes]。
///
/// [changes]: io#platform-specific-behavior
///
/// # 错误(Errors）
///
/// 此函数将在以下情形中返回错误，但不限于这些情形：
///
/// * `original` 路径不是文件，或不存在。
/// * 'link' 路径已存在。
///
/// # 示例
///
/// ```no_run
/// use std::fs;
///
/// fn main() -> std::io::Result<()> {
///     fs::hard_link("a.txt", "b.txt")?; // 将 a.txt 硬链接到 b.txt
///     Ok(())
/// }
/// ```
#[doc(alias = "CreateHardLink", alias = "linkat")]
#[stable(feature = "rust1", since = "1.0.0")]
pub fn hard_link<P: AsRef<Path>, Q: AsRef<Path>>(original: P, link: Q) -> io::Result<()> {
    fs_imp::hard_link(original.as_ref(), link.as_ref())
}

/// 在文件系统上创建一个新的符号链接。
///
/// `link` 路径将成为一个指向 `original` 路径的符号链接。
/// 在 Windows 上，这将是一个文件符号链接，而不是目录符号链接；
/// 因此，应改用平台特定的 [`std::os::unix::fs::symlink`]
/// 以及 [`std::os::windows::fs::symlink_file`] 或 [`symlink_dir`]，
/// 以使意图明确。
///
/// [`std::os::unix::fs::symlink`]: crate::os::unix::fs::symlink
/// [`std::os::windows::fs::symlink_file`]: crate::os::windows::fs::symlink_file
/// [`symlink_dir`]: crate::os::windows::fs::symlink_dir
///
/// # 示例
///
/// ```no_run
/// use std::fs;
///
/// fn main() -> std::io::Result<()> {
///     fs::soft_link("a.txt", "b.txt")?;
///     Ok(())
/// }
/// ```
#[stable(feature = "rust1", since = "1.0.0")]
#[deprecated(
    since = "1.1.0",
    note = "replaced with std::os::unix::fs::symlink and \
            std::os::windows::fs::{symlink_file, symlink_dir}"
)]
pub fn soft_link<P: AsRef<Path>, Q: AsRef<Path>>(original: P, link: Q) -> io::Result<()> {
    fs_imp::symlink(original.as_ref(), link.as_ref())
}

/// 读取一个符号链接，返回该链接所指向的文件。
///
/// # 平台特定行为
///
/// 此函数目前在 Unix 上对应 `readlink` 函数，在 Windows 上对应带
/// `FILE_FLAG_OPEN_REPARSE_POINT` 和 `FILE_FLAG_BACKUP_SEMANTICS` 标志的
/// `CreateFile` 函数。
/// 注意，这[未来可能改变][changes]。
///
/// [changes]: io#platform-specific-behavior
///
/// # 错误(Errors）
///
/// 此函数将在以下情形中返回错误，但不限于这些情形：
///
/// * `path` 不是一个符号链接。
/// * `path` 不存在。
///
/// # 示例
///
/// ```no_run
/// use std::fs;
///
/// fn main() -> std::io::Result<()> {
///     let path = fs::read_link("a.txt")?;
///     Ok(())
/// }
/// ```
#[stable(feature = "rust1", since = "1.0.0")]
pub fn read_link<P: AsRef<Path>>(path: P) -> io::Result<PathBuf> {
    fs_imp::read_link(path.as_ref())
}

/// 返回一个路径的规范的、绝对的形式，其中所有中间组成部分都被规范化，所有符号链接
/// 都被解析。
///
/// # 平台特定行为
///
/// 此函数目前在 Unix 上对应 `realpath` 函数，在 Windows 上对应 `CreateFile`
/// 和 `GetFinalPathNameByHandle` 函数。
/// 注意，这[未来可能改变][changes]。
///
/// 在 Windows 上，这会把路径转换为使用[扩展长度路径][path]语法，它允许你的程序
/// 使用更长的路径名，但意味着你只能向其拼接以反斜杠分隔的路径，而且它可能与其他
/// 应用程序不兼容（如果在命令行上传给应用程序，或写入另一个应用程序可能读取的文件）。
///
/// [changes]: io#platform-specific-behavior
/// [path]: https://docs.microsoft.com/en-us/windows/win32/fileio/naming-a-file
///
/// # 错误(Errors）
///
/// 此函数将在以下情形中返回错误，但不限于这些情形：
///
/// * `path` 不存在。
/// * path 中的某个非末尾组成部分不是目录。
///
/// # 示例
///
/// ```no_run
/// use std::fs;
///
/// fn main() -> std::io::Result<()> {
///     let path = fs::canonicalize("../a/../foo.txt")?;
///     Ok(())
/// }
/// ```
#[doc(alias = "realpath")]
#[doc(alias = "GetFinalPathNameByHandle")]
#[stable(feature = "fs_canonicalize", since = "1.5.0")]
pub fn canonicalize<P: AsRef<Path>>(path: P) -> io::Result<PathBuf> {
    fs_imp::canonicalize(path.as_ref())
}

/// 在所提供的路径处创建一个新的空目录。
///
/// # 平台特定行为
///
/// 此函数目前在 Unix 上对应 `mkdir` 函数，在 Windows 上对应 `CreateDirectoryW`
/// 函数。
/// 注意，这[未来可能改变][changes]。
///
/// [changes]: io#platform-specific-behavior
///
/// **NOTE**：如果给定路径的某个父级不存在，此函数将返回一个错误。要同时创建一个
/// 目录及其所有缺失的父级，请使用 [`create_dir_all`] 函数。
///
/// # 错误(Errors）
///
/// 此函数将在以下情形中返回错误，但不限于这些情形：
///
/// * 用户缺乏在 `path` 处创建目录的权限。
/// * 给定路径的某个父级不存在。（要同时创建一个目录及其所有缺失的父级，请使用
///   [`create_dir_all`] 函数。）
/// * `path` 已存在。
///
/// # 示例
///
/// ```no_run
/// use std::fs;
///
/// fn main() -> std::io::Result<()> {
///     fs::create_dir("/some/dir")?;
///     Ok(())
/// }
/// ```
#[doc(alias = "mkdir", alias = "CreateDirectory")]
#[stable(feature = "rust1", since = "1.0.0")]
#[cfg_attr(not(test), rustc_diagnostic_item = "fs_create_dir")]
pub fn create_dir<P: AsRef<Path>>(path: P) -> io::Result<()> {
    DirBuilder::new().create(path.as_ref())
}

/// 递归地创建一个目录及其所有缺失的父级组成部分。
///
/// 此函数不是原子的。如果它返回一个错误，它已经能够创建的任何父级组成部分都会保留
/// 下来。
///
/// 如果向此函数传入空路径，它总是成功返回而不创建任何目录。
///
/// # 平台特定行为
///
/// 此函数目前在 Unix 上对应对 `mkdir` 函数的多次调用，在 Windows 上对应
/// `CreateDirectoryW` 函数。
///
/// 注意，这[未来可能改变][changes]。
///
/// [changes]: io#platform-specific-behavior
///
/// # 错误(Errors）
///
/// 如果 path 中指定的任何目录不存在且无法创建，此函数将返回一个错误。可能还有其他
/// 错误条件；具体见 [`fs::create_dir`]。
///
/// 一个值得注意的例外是：当 `path` 中指定的某个目录因正在被并发地创建而无法创建时，
/// 这种情形被视为成功。也就是说，从多个线程或进程并发调用 `create_dir_all`，
/// 保证不会因与自身的竞态条件而失败。
///
/// [`fs::create_dir`]: create_dir
///
/// # 示例
///
/// ```no_run
/// use std::fs;
///
/// fn main() -> std::io::Result<()> {
///     fs::create_dir_all("/some/dir")?;
///     Ok(())
/// }
/// ```
#[stable(feature = "rust1", since = "1.0.0")]
pub fn create_dir_all<P: AsRef<Path>>(path: P) -> io::Result<()> {
    DirBuilder::new().recursive(true).create(path.as_ref())
}

/// 移除一个空目录。
///
/// 如果你想移除一个非空目录及其所有内容（递归地），请考虑改用 [`remove_dir_all`]。
///
/// # 平台特定行为
///
/// 此函数目前在 Unix 上对应 `rmdir` 函数，在 Windows 上对应 `RemoveDirectory`
/// 函数。
/// 注意，这[未来可能改变][changes]。
///
/// [changes]: io#platform-specific-behavior
///
/// # 错误(Errors）
///
/// 此函数将在以下情形中返回错误，但不限于这些情形：
///
/// * `path` 不存在。
/// * `path` 不是一个目录。
/// * 用户缺乏移除所提供 `path` 处目录的权限。
/// * 目录不为空。
///
/// 仅当给定路径不存在时，此函数才会返回 `NotFound` 类型的错误。注意，反过来并不
/// 成立，即如果某路径不存在，对它的移除仍可能因多种原因失败，例如权限不足。
///
/// # 示例
///
/// ```no_run
/// use std::fs;
///
/// fn main() -> std::io::Result<()> {
///     fs::remove_dir("/some/dir")?;
///     Ok(())
/// }
/// ```
#[doc(alias = "rmdir", alias = "RemoveDirectory")]
#[stable(feature = "rust1", since = "1.0.0")]
pub fn remove_dir<P: AsRef<Path>>(path: P) -> io::Result<()> {
    fs_imp::remove_dir(path.as_ref())
}

/// 移除此路径处的目录，在此之前移除其所有内容。请谨慎使用！
///
/// 此函数**不会**跟随符号链接，它只会移除符号链接本身。
///
/// # 平台特定行为
///
/// 这些实现细节[未来可能改变][changes]。
///
/// - “类 Unix”：默认情况下，此函数目前在 Unix 家族平台上对应
/// `openat`、`fdopendir`、`unlinkat` 和 `lstat`，
/// 另有说明者除外。
/// - “Windows”：此函数目前对应 `CreateFileW`、
/// `GetFileInformationByHandleEx`、`SetFileInformationByHandle` 和 `NtCreateFile`。
///
/// ## Time-of-check to time-of-use (TOCTOU) 竞态条件
/// 见[模块级的 TOCTOU 说明](self#time-of-check-to-time-of-use-toctou)。
///
/// 在大多数平台上，`fs::remove_dir_all` 默认会防止符号链接 TOCTOU 竞态。
/// 然而，在以下平台上不提供这种防护，因此此函数不应在安全敏感的上下文中使用：
/// - **Miri**：即使在模拟那些底层实现会防止 TOCTOU 竞态的目标时，Miri 也不会
///   这么做。
/// - **Redox OS**：此函数不防止 TOCTOU 竞态，因为 Redox 没有实现做到这一点所需的
///   平台支持。
///
/// [TOCTOU]: self#time-of-check-to-time-of-use-toctou
/// [changes]: io#platform-specific-behavior
///
/// # 错误(Errors）
///
/// 见 [`fs::remove_file`] 和 [`fs::remove_dir`]。
///
/// 如果 [`remove_dir`] 或 [`remove_file`] 在*任何*组成路径（*包括*根 `path`）上
/// 失败，[`remove_dir_all`] 都会失败。因此，
///
/// - 你要删除的目录*必须*存在，这意味着此函数*不是幂等的*。
/// - 如果 `path` *不是*目录，[`remove_dir_all`] 将失败。
///
/// 如果对你的用例来说，无需校验移除是否成功，可考虑忽略该错误。
///
/// 如果目录正在被并发写入，此函数可能返回 [`io::ErrorKind::DirectoryNotEmpty`]，
/// 这通常表示部分内容已被移除但并非全部。
/// 仅当没有发生任何移除时，才会返回 [`io::ErrorKind::NotFound`]。
///
/// [`fs::remove_file`]: remove_file
/// [`fs::remove_dir`]: remove_dir
///
/// # 示例
///
/// ```no_run
/// use std::fs;
///
/// fn main() -> std::io::Result<()> {
///     fs::remove_dir_all("/some/dir")?;
///     Ok(())
/// }
/// ```
#[stable(feature = "rust1", since = "1.0.0")]
pub fn remove_dir_all<P: AsRef<Path>>(path: P) -> io::Result<()> {
    fs_imp::remove_dir_all(path.as_ref())
}

/// 返回一个遍历目录中各条目的迭代器。
///
/// 该迭代器会产出 <code>[io::Result]<[DirEntry]></code> 实例。
/// 在迭代器初次构造之后，可能会遇到新的错误。
/// 当前目录和父目录的条目（通常是 `.` 和 `..`）会被跳过。
///
/// `read_dir` 返回条目的顺序可能在不同调用之间发生变化。如果需要可复现的顺序，
/// 应当对条目显式排序。
///
/// # 平台特定行为
///
/// 此函数目前在 Unix 上对应 `opendir` 函数，在 Windows 上对应 `FindFirstFileEx`
/// 函数。推进迭代器目前在 Unix 上对应 `readdir`，在 Windows 上对应
/// `FindNextFile`。
/// 注意，这[未来可能改变][changes]。
///
/// [changes]: io#platform-specific-behavior
///
/// 此迭代器返回条目的顺序取决于平台和文件系统。
///
/// # 错误(Errors）
///
/// 此函数将在以下情形中返回错误，但不限于这些情形：
///
/// * 所提供的 `path` 不存在。
/// * 进程缺乏查看内容的权限。
/// * `path` 指向一个非目录文件。
///
/// # 示例
///
/// ```
/// use std::io;
/// use std::fs::{self, DirEntry};
/// use std::path::Path;
///
/// // 遍历目录、仅访问文件的一种可能实现
/// fn visit_dirs(dir: &Path, cb: &dyn Fn(&DirEntry)) -> io::Result<()> {
///     if dir.is_dir() {
///         for entry in fs::read_dir(dir)? {
///             let entry = entry?;
///             let path = entry.path();
///             if path.is_dir() {
///                 visit_dirs(&path, cb)?;
///             } else {
///                 cb(&entry);
///             }
///         }
///     }
///     Ok(())
/// }
/// ```
///
/// ```rust,no_run
/// use std::{fs, io};
///
/// fn main() -> io::Result<()> {
///     let mut entries = fs::read_dir(".")?
///         .map(|res| res.map(|e| e.path()))
///         .collect::<Result<Vec<_>, io::Error>>()?;
///
///     // `read_dir` 返回条目的顺序无法保证。如果需要可复现的顺序，
///     // 应当对条目显式排序。
///
///     entries.sort();
///
///     // 现在条目已按其路径排好序。
///
///     Ok(())
/// }
/// ```
#[doc(alias = "ls", alias = "opendir", alias = "FindFirstFile", alias = "FindNextFile")]
#[stable(feature = "rust1", since = "1.0.0")]
pub fn read_dir<P: AsRef<Path>>(path: P) -> io::Result<ReadDir> {
    fs_imp::read_dir(path.as_ref()).map(ReadDir)
}

/// 更改文件或目录上的权限。
///
/// # 平台特定行为
///
/// 此函数目前在 Unix 上对应 `chmod` 函数，在 Windows 上对应 `SetFileAttributes`
/// 函数。
/// 注意，这[未来可能改变][changes]。
///
/// [changes]: io#platform-specific-behavior
///
/// ## Symlinks
/// 在类 UNIX 系统上，此函数会更新符号链接所指向文件的权限位。
///
/// 注意，这种行为可能导致提权漏洞：在某个目录中创建符号链接的能力，会让你能够导致
/// 另一个文件或目录的权限被修改。
///
/// 因此，应当避免对符号链接使用此函数。
/// 在可能的情况下，应当在创建时就设置好权限。
///
/// # Rationale
/// POSIX 没有规定 `lchmod` 函数，
/// 而且无论设置了什么权限位，符号链接都可能被跟随。
///
/// # 错误(Errors）
///
/// 此函数将在以下情形中返回错误，但不限于这些情形：
///
/// * `path` 不存在。
/// * 用户缺乏更改该文件属性的权限。
///
/// # 示例
///
/// ```no_run
/// use std::fs;
///
/// fn main() -> std::io::Result<()> {
///     let mut perms = fs::metadata("foo.txt")?.permissions();
///     perms.set_readonly(true);
///     fs::set_permissions("foo.txt", perms)?;
///     Ok(())
/// }
/// ```
#[doc(alias = "chmod", alias = "SetFileAttributes")]
#[stable(feature = "set_permissions", since = "1.1.0")]
pub fn set_permissions<P: AsRef<Path>>(path: P, perm: Permissions) -> io::Result<()> {
    fs_imp::set_permissions(path.as_ref(), perm.0)
}

/// 设置文件的权限，除非它是一个符号链接。
///
/// 注意，非末尾的路径元素允许是符号链接。
///
/// # 平台特定行为
///
/// 目前在 Windows 上未实现。
///
/// 在 Unix 平台上，如果末尾元素是符号链接，会导致一个 [`FilesystemLoop`] 错误。
///
/// 此行为未来可能改变。
///
/// [`FilesystemLoop`]: crate::io::ErrorKind::FilesystemLoop
#[doc(alias = "chmod", alias = "SetFileAttributes")]
#[unstable(feature = "set_permissions_nofollow", issue = "141607")]
pub fn set_permissions_nofollow<P: AsRef<Path>>(path: P, perm: Permissions) -> io::Result<()> {
    fs_imp::set_permissions_nofollow(path.as_ref(), perm)
}

impl DirBuilder {
    /// 创建一组新选项，对所有平台采用默认的 mode/安全设置，并且为非递归。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::fs::DirBuilder;
    ///
    /// let builder = DirBuilder::new();
    /// ```
    #[stable(feature = "dir_builder", since = "1.6.0")]
    #[must_use]
    pub fn new() -> DirBuilder {
        DirBuilder { inner: fs_imp::DirBuilder::new(), recursive: false }
    }

    /// 指示应当递归地创建目录，即创建所有父目录。不存在的父级会以相同的安全和权限
    /// 设置创建。
    ///
    /// 此选项默认为 `false`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::fs::DirBuilder;
    ///
    /// let mut builder = DirBuilder::new();
    /// builder.recursive(true);
    /// ```
    #[stable(feature = "dir_builder", since = "1.6.0")]
    pub fn recursive(&mut self, recursive: bool) -> &mut Self {
        self.recursive = recursive;
        self
    }

    /// 使用此构建器中配置的选项创建指定的目录。
    ///
    /// 除非启用了递归模式，否则目录已存在会被视为一个错误。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs::{self, DirBuilder};
    ///
    /// let path = "/tmp/foo/bar/baz";
    /// DirBuilder::new()
    ///     .recursive(true)
    ///     .create(path).unwrap();
    ///
    /// assert!(fs::metadata(path).unwrap().is_dir());
    /// ```
    #[stable(feature = "dir_builder", since = "1.6.0")]
    pub fn create<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
        self._create(path.as_ref())
    }

    fn _create(&self, path: &Path) -> io::Result<()> {
        if self.recursive { self.create_dir_all(path) } else { self.inner.mkdir(path) }
    }

    fn create_dir_all(&self, path: &Path) -> io::Result<()> {
        // 如果 path 的父级为 None，它就是 "/" 路径，应当立即返回 Ok
        if path == Path::new("") || path.parent() == None {
            return Ok(());
        }

        let ancestors = path.ancestors();
        let mut uncreated_dirs = 0;

        for ancestor in ancestors {
            // 对于像 "foo/bar" 这样的相对路径，"foo" 的父级会是 ""，
            // 无需对它发起 mkdir 系统调用
            if ancestor == Path::new("") || ancestor.parent() == None {
                break;
            }

            match self.inner.mkdir(ancestor) {
                Ok(()) => break,
                Err(e) if e.kind() == io::ErrorKind::NotFound => uncreated_dirs += 1,
                // 我们检查 err 是否为 AlreadyExists，有两个原因
                //    - 以防路径以*文件*形式存在
                //    - 并且为了避免在其他错误（如 PermissionDenied）情况下
                //      调用 .is_dir()
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists && ancestor.is_dir() => break,
                Err(e) => return Err(e),
            }
        }

        // 只收集未创建的目录，且不让 vec 发生扩容
        let mut uncreated_dirs_vec = Vec::with_capacity(uncreated_dirs);
        uncreated_dirs_vec.extend(ancestors.take(uncreated_dirs));

        for uncreated_dir in uncreated_dirs_vec.iter().rev() {
            if let Err(e) = self.inner.mkdir(uncreated_dir) {
                if e.kind() != io::ErrorKind::AlreadyExists || !uncreated_dir.is_dir() {
                    return Err(e);
                }
            }
        }

        Ok(())
    }
}

impl AsInnerMut<fs_imp::DirBuilder> for DirBuilder {
    #[inline]
    fn as_inner_mut(&mut self) -> &mut fs_imp::DirBuilder {
        &mut self.inner
    }
}

/// 如果路径指向一个已存在的实体，返回 `Ok(true)`。
///
/// 此函数会遍历符号链接，查询目标文件的信息。对于断裂的符号链接，这将返回
/// `Ok(false)`。
///
/// 与 [`Path::exists`] 方法不同，此函数只有在路径被_验证_为存在或不存在时，才会
/// 返回 `Ok(true)` 或 `Ok(false)`。如果其存在性既无法确认也无法否认，将转而向上
/// 传播一个 `Err(_)`。例如，当某个父目录上的列举权限被拒绝时，就可能出现这种情况。
///
/// 注意，虽然这避免了 `exists()` 方法的一些陷阱，但它仍然无法防止检查时刻到使用时刻
/// ([TOCTOU]) 缺陷。你应当只在那些此类缺陷不构成问题的场景中使用它。
///
/// # 示例
///
/// ```no_run
/// use std::fs;
///
/// assert!(!fs::exists("does_not_exist.txt").expect("Can't check existence of file does_not_exist.txt"));
/// assert!(fs::exists("/root/secret_file.txt").is_err());
/// ```
///
/// [`Path::exists`]: crate::path::Path::exists
/// [TOCTOU]: self#time-of-check-to-time-of-use-toctou
#[stable(feature = "fs_try_exists", since = "1.81.0")]
#[inline]
pub fn exists<P: AsRef<Path>>(path: P) -> io::Result<bool> {
    fs_imp::exists(path.as_ref())
}
