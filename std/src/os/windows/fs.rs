//! Windows 平台对 [`std::fs`] 模块中各原语的特定扩展。
//!
//! [`std::fs`]: crate::fs

#![stable(feature = "rust1", since = "1.0.0")]

use crate::fs::{self, Metadata, OpenOptions};
use crate::io::BorrowedCursor;
use crate::path::Path;
use crate::sealed::Sealed;
use crate::sys::{AsInner, AsInnerMut, IntoInner};
use crate::time::SystemTime;
use crate::{io, sys};

/// Windows 平台对 [`fs::File`] 的特定扩展。
#[stable(feature = "file_offset", since = "1.15.0")]
pub trait FileExt {
    /// 定位到给定位置并读取若干字节。
    ///
    /// 返回读取到的字节数。
    ///
    /// 该偏移量相对于文件起始处，因此与当前游标无关。但当前游标 **会** 受本函数影响，
    /// 它会被设置到本次读取的末尾处。
    ///
    /// 在文件末尾之外进行读取将始终返回长度 0。
    ///
    /// 注意：与 `File::read` 类似，发生“短读”（返回的字节数少于请求量）并不算错误。
    /// 从这样的短读返回时，文件指针仍会被更新。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::io;
    /// use std::fs::File;
    /// use std::os::windows::prelude::*;
    ///
    /// fn main() -> io::Result<()> {
    ///     let mut file = File::open("foo.txt")?;
    ///     let mut buffer = [0; 10];
    ///
    ///     // 从文件起始处偏移 72 字节开始，读取 10 字节。
    ///     file.seek_read(&mut buffer[..], 72)?;
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "file_offset", since = "1.15.0")]
    fn seek_read(&self, buf: &mut [u8], offset: u64) -> io::Result<usize>;

    /// 定位到给定位置并把若干字节读入缓冲区。
    ///
    /// 这等价于 [`seek_read`](FileExt::seek_read) 方法，区别在于它接收一个
    /// [`BorrowedCursor`] 而非 `&mut [u8]`，以便能用于未初始化的缓冲区。新读到的数据
    /// 将被追加到 `buf` 已有的内容之后。
    ///
    /// 在文件末尾之外进行读取将始终成功，且不会读到任何字节。
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
    /// use std::os::windows::prelude::*;
    ///
    /// fn main() -> io::Result<()> {
    ///     let mut file = File::open("pi.txt")?;
    ///
    ///     // 从偏移 2 开始读取若干字节
    ///     let mut buf: [MaybeUninit<u8>; 10] = [MaybeUninit::uninit(); 10];
    ///     let mut buf = BorrowedBuf::from(buf.as_mut_slice());
    ///     file.seek_read_buf(buf.unfilled(), 2)?;
    ///
    ///     assert!(buf.filled().starts_with(b"1"));
    ///
    ///     Ok(())
    /// }
    /// ```
    #[unstable(feature = "read_buf_at", issue = "140771")]
    fn seek_read_buf(&self, buf: BorrowedCursor<'_>, offset: u64) -> io::Result<()> {
        io::default_read_buf(|b| self.seek_read(b, offset), buf)
    }

    /// 定位到给定位置并写入若干字节。
    ///
    /// 返回写入的字节数。
    ///
    /// 该偏移量相对于文件起始处，因此与当前游标无关。但当前游标 **会** 受本函数影响，
    /// 它会被设置到本次写入的末尾处。
    ///
    /// 当写入到文件末尾之外时，文件会被相应地扩展，中间的字节会被置为零。
    ///
    /// 注意：与 `File::write` 类似，发生“短写”（写入的字节数少于请求量）并不算错误。
    /// 从这样的短写返回时，文件指针仍会被更新。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs::File;
    /// use std::os::windows::prelude::*;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let mut buffer = File::create("foo.txt")?;
    ///
    ///     // 从文件起始处偏移 72 字节开始，写入一个字节串。
    ///     buffer.seek_write(b"some bytes", 72)?;
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "file_offset", since = "1.15.0")]
    fn seek_write(&self, buf: &[u8], offset: u64) -> io::Result<usize>;
}

#[stable(feature = "file_offset", since = "1.15.0")]
impl FileExt for fs::File {
    fn seek_read(&self, buf: &mut [u8], offset: u64) -> io::Result<usize> {
        self.as_inner().read_at(buf, offset)
    }

    fn seek_read_buf(&self, buf: BorrowedCursor<'_>, offset: u64) -> io::Result<()> {
        self.as_inner().read_buf_at(buf, offset)
    }

    fn seek_write(&self, buf: &[u8], offset: u64) -> io::Result<usize> {
        self.as_inner().write_at(buf, offset)
    }
}

/// Windows 平台对 [`fs::OpenOptions`] 的特定扩展。
#[stable(feature = "open_options_ext", since = "1.10.0")]
pub trait OpenOptionsExt {
    /// 用指定的值覆盖调用 [`CreateFile`] 时所传入的 `dwDesiredAccess` 参数。
    ///
    /// 这会覆盖 `OpenOptions` 结构上的 `read`、`write` 和 `append` 标志。本方法对读取、
    /// 写入、追加数据以及属性（如 hidden、system）和扩展属性的权限提供细粒度控制。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs::OpenOptions;
    /// use std::os::windows::prelude::*;
    ///
    /// // 不带读写权限地打开，例如当你只需要对该文件调用 `stat` 时
    /// let file = OpenOptions::new().access_mode(0).open("foo.txt");
    /// ```
    ///
    /// [`CreateFile`]: https://docs.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilea
    #[stable(feature = "open_options_ext", since = "1.10.0")]
    fn access_mode(&mut self, access: u32) -> &mut Self;

    /// 用指定的值覆盖调用 [`CreateFile`] 时所传入的 `dwShareMode` 参数。
    ///
    /// 默认情况下 `share_mode` 被设置为
    /// `FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE`。这允许其他进程在该文件
    /// 打开期间对同一文件进行读取、写入和删除/重命名。去掉其中任何一个标志，都会阻止其他
    /// 进程执行对应操作，直到该文件 handle 被关闭为止。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs::OpenOptions;
    /// use std::os::windows::prelude::*;
    ///
    /// // 在我们以写入方式打开该文件期间，不允许其他进程读取或修改它。
    /// let file = OpenOptions::new()
    ///     .write(true)
    ///     .share_mode(0)
    ///     .open("foo.txt");
    /// ```
    ///
    /// [`CreateFile`]: https://docs.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilea
    #[stable(feature = "open_options_ext", since = "1.10.0")]
    fn share_mode(&mut self, val: u32) -> &mut Self;

    /// 把调用 [`CreateFile2`] 时所传入的 `dwFileFlags` 参数的额外标志设置为指定的值
    /// （或者把它与 `attributes` 和 `security_qos_flags` 组合起来，用以设置
    /// [`CreateFile`] 的 `dwFlagsAndAttributes`）。
    ///
    /// 自定义标志只能设置标志位，而不能移除由 Rust 的选项所设置的标志位。本选项会覆盖
    /// 之前设置的任何自定义标志。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// # #![allow(unexpected_cfgs)]
    /// # #[cfg(for_demonstration_only)]
    /// extern crate winapi;
    /// # mod winapi { pub const FILE_FLAG_DELETE_ON_CLOSE: u32 = 0x04000000; }
    ///
    /// use std::fs::OpenOptions;
    /// use std::os::windows::prelude::*;
    ///
    /// let file = OpenOptions::new()
    ///     .create(true)
    ///     .write(true)
    ///     .custom_flags(winapi::FILE_FLAG_DELETE_ON_CLOSE)
    ///     .open("foo.txt");
    /// ```
    ///
    /// [`CreateFile`]: https://docs.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilea
    /// [`CreateFile2`]: https://docs.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfile2
    #[stable(feature = "open_options_ext", since = "1.10.0")]
    fn custom_flags(&mut self, flags: u32) -> &mut Self;

    /// 把调用 [`CreateFile2`] 时所传入的 `dwFileAttributes` 参数设置为指定的值
    /// （或者把它与 `custom_flags` 和 `security_qos_flags` 组合起来，用以设置
    /// [`CreateFile`] 的 `dwFlagsAndAttributes`）。
    ///
    /// 如果因文件尚不存在、且指定了 `.create(true)` 或 `.create_new(true)` 而创建了一个
    /// _新_ 文件，则该新文件会被赋予用 `.attributes()` 声明的属性。
    ///
    /// 如果用 `.create(true).truncate(true)` 打开一个 _已存在_ 的文件，则其已有属性会被
    /// 保留，并与用 `.attributes()` 声明的属性组合起来。
    ///
    /// 在所有其他情况下，这些属性都会被忽略。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// # #![allow(unexpected_cfgs)]
    /// # #[cfg(for_demonstration_only)]
    /// extern crate winapi;
    /// # mod winapi { pub const FILE_ATTRIBUTE_HIDDEN: u32 = 2; }
    ///
    /// use std::fs::OpenOptions;
    /// use std::os::windows::prelude::*;
    ///
    /// let file = OpenOptions::new()
    ///     .write(true)
    ///     .create(true)
    ///     .attributes(winapi::FILE_ATTRIBUTE_HIDDEN)
    ///     .open("foo.txt");
    /// ```
    ///
    /// [`CreateFile`]: https://docs.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilea
    /// [`CreateFile2`]: https://docs.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfile2
    #[stable(feature = "open_options_ext", since = "1.10.0")]
    fn attributes(&mut self, val: u32) -> &mut Self;

    /// 把调用 [`CreateFile2`] 时所传入的 `dwSecurityQosFlags` 参数设置为指定的值
    /// （或者把它与 `custom_flags` 和 `attributes` 组合起来，用以设置 [`CreateFile`] 的
    /// `dwFlagsAndAttributes`）。
    ///
    /// 默认情况下 `security_qos_flags` 不被设置。在打开命名管道（named pipe）时应当指定它，
    /// 用以控制服务端进程能在多大程度上代表客户端进程行事（即安全模拟级别，security
    /// impersonation level）。
    ///
    /// 当 `security_qos_flags` 未被设置时，一个特权 Rust 进程若允许打开用户指定的路径，
    /// 恶意程序就有可能诱骗它去打开一个命名管道，从而窃取该进程的提升特权。因此可以说，
    /// 在打开任意路径时也应当设置 `security_qos_flags`。不过，这些位随后可能会与其他标志
    /// 冲突，具体来说是 `FILE_FLAG_OPEN_NO_RECALL`。
    ///
    /// 关于可能取值的信息，参见 Windows 开发者中心网站上的 [Impersonation Levels]。
    /// 使用本方法时会自动设置 `SECURITY_SQOS_PRESENT` 标志。

    /// # 示例
    ///
    /// ```no_run
    /// # #![allow(unexpected_cfgs)]
    /// # #[cfg(for_demonstration_only)]
    /// extern crate winapi;
    /// # mod winapi { pub const SECURITY_IDENTIFICATION: u32 = 0; }
    /// use std::fs::OpenOptions;
    /// use std::os::windows::prelude::*;
    ///
    /// let file = OpenOptions::new()
    ///     .write(true)
    ///     .create(true)
    ///
    ///     // 把标志值设置为 `SecurityIdentification`。
    ///     .security_qos_flags(winapi::SECURITY_IDENTIFICATION)
    ///
    ///     .open(r"\\.\pipe\MyPipe");
    /// ```
    ///
    /// [`CreateFile`]: https://docs.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfilea
    /// [`CreateFile2`]: https://docs.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createfile2
    /// [Impersonation Levels]:
    ///     https://docs.microsoft.com/en-us/windows/win32/api/winnt/ne-winnt-security_impersonation_level
    #[stable(feature = "open_options_ext", since = "1.10.0")]
    fn security_qos_flags(&mut self, flags: u32) -> &mut Self;

    /// 若设置为 `true`，则阻止该文件的“最后访问时间”（last access time）被改变。
    ///
    /// 默认为 `false`。
    #[unstable(feature = "windows_freeze_file_times", issue = "149715")]
    fn freeze_last_access_time(&mut self, freeze: bool) -> &mut Self;

    /// 若设置为 `true`，则阻止该文件的“最后写入时间”（last write time）被改变。
    ///
    /// 默认为 `false`。
    #[unstable(feature = "windows_freeze_file_times", issue = "149715")]
    fn freeze_last_write_time(&mut self, freeze: bool) -> &mut Self;
}

#[stable(feature = "open_options_ext", since = "1.10.0")]
impl OpenOptionsExt for OpenOptions {
    fn access_mode(&mut self, access: u32) -> &mut OpenOptions {
        self.as_inner_mut().access_mode(access);
        self
    }

    fn share_mode(&mut self, share: u32) -> &mut OpenOptions {
        self.as_inner_mut().share_mode(share);
        self
    }

    fn custom_flags(&mut self, flags: u32) -> &mut OpenOptions {
        self.as_inner_mut().custom_flags(flags);
        self
    }

    fn attributes(&mut self, attributes: u32) -> &mut OpenOptions {
        self.as_inner_mut().attributes(attributes);
        self
    }

    fn security_qos_flags(&mut self, flags: u32) -> &mut OpenOptions {
        self.as_inner_mut().security_qos_flags(flags);
        self
    }

    fn freeze_last_access_time(&mut self, freeze: bool) -> &mut Self {
        self.as_inner_mut().freeze_last_access_time(freeze);
        self
    }

    fn freeze_last_write_time(&mut self, freeze: bool) -> &mut Self {
        self.as_inner_mut().freeze_last_write_time(freeze);
        self
    }
}

/// Windows 平台对 [`fs::Metadata`] 的特定扩展。
///
/// 本 trait 所暴露的数据成员，对应于 [`BY_HANDLE_FILE_INFORMATION`] 结构体的成员。
///
/// [`BY_HANDLE_FILE_INFORMATION`]:
///     https://docs.microsoft.com/windows/win32/api/fileapi/ns-fileapi-by_handle_file_information
#[stable(feature = "metadata_ext", since = "1.1.0")]
pub trait MetadataExt {
    /// 返回本元数据中 `dwFileAttributes` 字段的值。
    ///
    /// 该字段包含文件或目录的文件系统属性信息。关于可能取值及其描述，参见 Windows
    /// 开发者中心的 [File Attribute Constants]。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::io;
    /// use std::fs;
    /// use std::os::windows::prelude::*;
    ///
    /// fn main() -> io::Result<()> {
    ///     let metadata = fs::metadata("foo.txt")?;
    ///     let attributes = metadata.file_attributes();
    ///     Ok(())
    /// }
    /// ```
    ///
    /// [File Attribute Constants]:
    ///     https://docs.microsoft.com/windows/win32/fileio/file-attribute-constants
    #[stable(feature = "metadata_ext", since = "1.1.0")]
    fn file_attributes(&self) -> u32;

    /// 返回本元数据中 `ftCreationTime` 字段的值。
    ///
    /// 返回的 64 位值等价于一个 [`FILETIME`] 结构体，它表示自 1601 年 1 月 1 日（UTC）
    /// 以来经过的 100 纳秒间隔的数目。该结构体会被自动转换为一个 `u64` 值，因为这是推荐的
    /// 使用方式。
    ///
    /// 如果底层文件系统不支持创建时间，则返回值为 0。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::io;
    /// use std::fs;
    /// use std::os::windows::prelude::*;
    ///
    /// fn main() -> io::Result<()> {
    ///     let metadata = fs::metadata("foo.txt")?;
    ///     let creation_time = metadata.creation_time();
    ///     Ok(())
    /// }
    /// ```
    ///
    /// [`FILETIME`]: https://docs.microsoft.com/windows/win32/api/minwinbase/ns-minwinbase-filetime
    #[stable(feature = "metadata_ext", since = "1.1.0")]
    fn creation_time(&self) -> u64;

    /// 返回本元数据中 `ftLastAccessTime` 字段的值。
    ///
    /// 返回的 64 位值等价于一个 [`FILETIME`] 结构体，它表示自 1601 年 1 月 1 日（UTC）
    /// 以来经过的 100 纳秒间隔的数目。该结构体会被自动转换为一个 `u64` 值，因为这是推荐的
    /// 使用方式。
    ///
    /// 对于文件，该值给出了文件最后一次被读取或写入的时间。对于目录，该值给出了目录被创建
    /// 的时间。对于文件和目录，所给出的日期都是正确的，但一天中的具体时刻总是被设为午夜。
    ///
    /// 如果底层文件系统不支持最后访问时间，则返回值为 0。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::io;
    /// use std::fs;
    /// use std::os::windows::prelude::*;
    ///
    /// fn main() -> io::Result<()> {
    ///     let metadata = fs::metadata("foo.txt")?;
    ///     let last_access_time = metadata.last_access_time();
    ///     Ok(())
    /// }
    /// ```
    ///
    /// [`FILETIME`]: https://docs.microsoft.com/windows/win32/api/minwinbase/ns-minwinbase-filetime
    #[stable(feature = "metadata_ext", since = "1.1.0")]
    fn last_access_time(&self) -> u64;

    /// 返回本元数据中 `ftLastWriteTime` 字段的值。
    ///
    /// 返回的 64 位值等价于一个 [`FILETIME`] 结构体，它表示自 1601 年 1 月 1 日（UTC）
    /// 以来经过的 100 纳秒间隔的数目。该结构体会被自动转换为一个 `u64` 值，因为这是推荐的
    /// 使用方式。
    ///
    /// 对于文件，该值给出了文件最后一次被写入的时间。对于目录，该结构体给出了目录被创建
    /// 的时间。
    ///
    /// 如果底层文件系统不支持最后写入时间，则返回值为 0。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::io;
    /// use std::fs;
    /// use std::os::windows::prelude::*;
    ///
    /// fn main() -> io::Result<()> {
    ///     let metadata = fs::metadata("foo.txt")?;
    ///     let last_write_time = metadata.last_write_time();
    ///     Ok(())
    /// }
    /// ```
    ///
    /// [`FILETIME`]: https://docs.microsoft.com/windows/win32/api/minwinbase/ns-minwinbase-filetime
    #[stable(feature = "metadata_ext", since = "1.1.0")]
    fn last_write_time(&self) -> u64;

    /// 返回本元数据中 `nFileSize` 字段的值。
    ///
    /// 对于目录，返回值没有意义。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::io;
    /// use std::fs;
    /// use std::os::windows::prelude::*;
    ///
    /// fn main() -> io::Result<()> {
    ///     let metadata = fs::metadata("foo.txt")?;
    ///     let file_size = metadata.file_size();
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "metadata_ext", since = "1.1.0")]
    fn file_size(&self) -> u64;

    /// 返回本元数据中 `dwVolumeSerialNumber` 字段的值。
    ///
    /// 如果该 `Metadata` 实例是由对 `DirEntry::metadata` 的调用创建的，则本方法返回
    /// `None`。如果该 `Metadata` 是通过 `fs::metadata` 或 `File::metadata` 创建的，则本
    /// 方法返回 `Some`。
    #[unstable(feature = "windows_by_handle", issue = "63010")]
    fn volume_serial_number(&self) -> Option<u32>;

    /// 返回本元数据中 `nNumberOfLinks` 字段的值。
    ///
    /// 如果该 `Metadata` 实例是由对 `DirEntry::metadata` 的调用创建的，则本方法返回
    /// `None`。如果该 `Metadata` 是通过 `fs::metadata` 或 `File::metadata` 创建的，则本
    /// 方法返回 `Some`。
    #[unstable(feature = "windows_by_handle", issue = "63010")]
    fn number_of_links(&self) -> Option<u32>;

    /// 返回本元数据中 `nFileIndex` 字段的值。
    ///
    /// 如果该 `Metadata` 实例是由对 `DirEntry::metadata` 的调用创建的，则本方法返回
    /// `None`。如果该 `Metadata` 是通过 `fs::metadata` 或 `File::metadata` 创建的，则本
    /// 方法返回 `Some`。
    #[unstable(feature = "windows_by_handle", issue = "63010")]
    fn file_index(&self) -> Option<u64>;

    /// 返回本元数据中 `ChangeTime` 字段的值。
    ///
    /// `ChangeTime` 是文件元数据最后一次被改变的时间，例如重命名、属性变更等。
    ///
    /// 如果该 `Metadata` 实例是由对 `DirEntry::metadata` 的调用创建的，或者当前
    /// `target_vendor` 超出了本 api 当前支持的平台范围，则本方法返回 `None`。
    #[unstable(feature = "windows_change_time", issue = "121478")]
    fn change_time(&self) -> Option<u64>;
}

#[stable(feature = "metadata_ext", since = "1.1.0")]
impl MetadataExt for Metadata {
    fn file_attributes(&self) -> u32 {
        self.as_inner().attrs()
    }
    fn creation_time(&self) -> u64 {
        self.as_inner().created_u64()
    }
    fn last_access_time(&self) -> u64 {
        self.as_inner().accessed_u64()
    }
    fn last_write_time(&self) -> u64 {
        self.as_inner().modified_u64()
    }
    fn file_size(&self) -> u64 {
        self.as_inner().size()
    }
    fn volume_serial_number(&self) -> Option<u32> {
        self.as_inner().volume_serial_number()
    }
    fn number_of_links(&self) -> Option<u32> {
        self.as_inner().number_of_links()
    }
    fn file_index(&self) -> Option<u64> {
        self.as_inner().file_index()
    }
    fn change_time(&self) -> Option<u64> {
        self.as_inner().changed_u64()
    }
}

/// Windows 平台对 [`fs::FileType`] 的特定扩展。
///
/// 在 Windows 上，符号链接（symbolic link）知道自己指向的是文件还是目录。
#[stable(feature = "windows_file_type_ext", since = "1.64.0")]
pub trait FileTypeExt: Sealed {
    /// 如果该文件类型是一个同时也是目录的符号链接，则返回 `true`。
    #[stable(feature = "windows_file_type_ext", since = "1.64.0")]
    fn is_symlink_dir(&self) -> bool;
    /// 如果该文件类型是一个同时也是文件的符号链接，则返回 `true`。
    #[stable(feature = "windows_file_type_ext", since = "1.64.0")]
    fn is_symlink_file(&self) -> bool;
}

#[stable(feature = "windows_file_type_ext", since = "1.64.0")]
impl Sealed for fs::FileType {}

#[stable(feature = "windows_file_type_ext", since = "1.64.0")]
impl FileTypeExt for fs::FileType {
    fn is_symlink_dir(&self) -> bool {
        self.as_inner().is_symlink_dir()
    }
    fn is_symlink_file(&self) -> bool {
        self.as_inner().is_symlink_file()
    }
}

/// Windows 平台对 [`fs::FileTimes`] 的特定扩展。
#[stable(feature = "file_set_times", since = "1.75.0")]
pub trait FileTimesExt: Sealed {
    /// 设置文件的创建时间。
    #[stable(feature = "file_set_times", since = "1.75.0")]
    fn set_created(self, t: SystemTime) -> Self;
}

#[stable(feature = "file_set_times", since = "1.75.0")]
impl FileTimesExt for fs::FileTimes {
    fn set_created(mut self, t: SystemTime) -> Self {
        self.as_inner_mut().set_created(t.into_inner());
        self
    }
}

/// 在文件系统上创建一个指向非目录文件的新符号链接。
///
/// `link` 路径将成为一个指向 `original` 路径的文件符号链接。
///
/// `original` 路径不应是目录或指向目录的符号链接，否则该符号链接将是损坏的。对于目录，
/// 请使用 [`symlink_dir`]。
///
/// 本函数目前对应于 [`CreateSymbolicLinkW`][CreateSymbolicLinkW]。注意这[在将来可能改变][changes]。
///
/// [CreateSymbolicLinkW]: https://docs.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-createsymboliclinkw
/// [changes]: io#platform-specific-behavior
///
/// # 示例
///
/// ```no_run
/// use std::os::windows::fs;
///
/// fn main() -> std::io::Result<()> {
///     fs::symlink_file("a.txt", "b.txt")?;
///     Ok(())
/// }
/// ```
///
/// # Limitations
///
/// Windows 把符号链接的创建视为一种 [特权操作][symlink-security]，因此本函数很可能会失败，
/// 除非用户对其系统做出更改以允许创建符号链接。用户可以尝试启用开发者模式（Developer
/// Mode）、授予 `SeCreateSymbolicLinkPrivilege` 特权，或以管理员身份运行该进程。
///
/// [symlink-security]: https://docs.microsoft.com/en-us/windows/security/threat-protection/security-policy-settings/create-symbolic-links
#[stable(feature = "symlink", since = "1.1.0")]
pub fn symlink_file<P: AsRef<Path>, Q: AsRef<Path>>(original: P, link: Q) -> io::Result<()> {
    sys::fs::symlink_inner(original.as_ref(), link.as_ref(), false)
}

/// 在文件系统上创建一个指向目录的新符号链接。
///
/// `link` 路径将成为一个指向 `original` 路径的目录符号链接。
///
/// `original` 路径必须是目录或指向目录的符号链接，否则该符号链接将是损坏的。对于其他文件，
/// 请使用 [`symlink_file`]。
///
/// 本函数目前对应于 [`CreateSymbolicLinkW`][CreateSymbolicLinkW]。注意这[在将来可能改变][changes]。
///
/// [CreateSymbolicLinkW]: https://docs.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-createsymboliclinkw
/// [changes]: io#platform-specific-behavior
///
/// # 示例
///
/// ```no_run
/// use std::os::windows::fs;
///
/// fn main() -> std::io::Result<()> {
///     fs::symlink_dir("a", "b")?;
///     Ok(())
/// }
/// ```
///
/// # Limitations
///
/// Windows 把符号链接的创建视为一种 [特权操作][symlink-security]，因此本函数很可能会失败，
/// 除非用户对其系统做出更改以允许创建符号链接。用户可以尝试启用开发者模式（Developer
/// Mode）、授予 `SeCreateSymbolicLinkPrivilege` 特权，或以管理员身份运行该进程。
///
/// [symlink-security]: https://docs.microsoft.com/en-us/windows/security/threat-protection/security-policy-settings/create-symbolic-links
#[stable(feature = "symlink", since = "1.1.0")]
pub fn symlink_dir<P: AsRef<Path>, Q: AsRef<Path>>(original: P, link: Q) -> io::Result<()> {
    sys::fs::symlink_inner(original.as_ref(), link.as_ref(), true)
}

/// 创建一个联结点（junction point）。
///
/// `link` 路径将成为一个指向 original 路径的目录联结（directory junction）。
/// 如果 `link` 是相对路径，则在创建联结点之前它会被转换为绝对路径。
/// `original` 路径必须是目录或指向目录的链接，否则该联结点将是损坏的。
///
/// 如果两个路径中有任何一个不是本地文件路径，则本函数会失败。
#[unstable(feature = "junction_point", issue = "121709")]
pub fn junction_point<P: AsRef<Path>, Q: AsRef<Path>>(original: P, link: Q) -> io::Result<()> {
    sys::fs::junction_point(original.as_ref(), link.as_ref())
}
