#![stable(feature = "metadata_ext", since = "1.1.0")]

use crate::fs::Metadata;
#[allow(deprecated)]
use crate::os::redox::raw;
use crate::sys::AsInner;

/// 针对 [`fs::Metadata`] 的操作系统特定扩展。
///
/// [`fs::Metadata`]: crate::fs::Metadata
#[stable(feature = "metadata_ext", since = "1.1.0")]
pub trait MetadataExt {
    /// 获取一个对底层 `stat` 结构体的引用，其中包含
    /// 由操作系统返回的原始信息。
    ///
    /// 返回的 [`stat`] 的内容在各 Unix 平台之间**并不**一致。
    /// `os::unix::fs::MetadataExt` trait 包含了原始 stat 中所蕴含的
    /// 跨 Unix 的抽象。
    ///
    /// [`stat`]: crate::os::redox::raw::stat
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs;
    /// use std::io;
    /// use std::os::redox::fs::MetadataExt;
    ///
    /// fn main() -> io::Result<()> {
    ///     let meta = fs::metadata("some_file")?;
    ///     let stat = meta.as_raw_stat();
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "metadata_ext", since = "1.1.0")]
    #[deprecated(
        since = "1.8.0",
        note = "deprecated in favor of the accessor \
                methods of this trait"
    )]
    #[allow(deprecated)]
    fn as_raw_stat(&self) -> &raw::stat;

    /// 返回此文件所在的设备 ID。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs;
    /// use std::io;
    /// use std::os::redox::fs::MetadataExt;
    ///
    /// fn main() -> io::Result<()> {
    ///     let meta = fs::metadata("some_file")?;
    ///     println!("{}", meta.st_dev());
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "metadata_ext2", since = "1.8.0")]
    fn st_dev(&self) -> u64;
    /// 返回 inode 编号。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs;
    /// use std::io;
    /// use std::os::redox::fs::MetadataExt;
    ///
    /// fn main() -> io::Result<()> {
    ///     let meta = fs::metadata("some_file")?;
    ///     println!("{}", meta.st_ino());
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "metadata_ext2", since = "1.8.0")]
    fn st_ino(&self) -> u64;
    /// 返回文件类型和模式（mode）。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs;
    /// use std::io;
    /// use std::os::redox::fs::MetadataExt;
    ///
    /// fn main() -> io::Result<()> {
    ///     let meta = fs::metadata("some_file")?;
    ///     println!("{}", meta.st_mode());
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "metadata_ext2", since = "1.8.0")]
    fn st_mode(&self) -> u32;
    /// 返回指向该文件的硬链接数。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs;
    /// use std::io;
    /// use std::os::redox::fs::MetadataExt;
    ///
    /// fn main() -> io::Result<()> {
    ///     let meta = fs::metadata("some_file")?;
    ///     println!("{}", meta.st_nlink());
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "metadata_ext2", since = "1.8.0")]
    fn st_nlink(&self) -> u64;
    /// 返回文件所有者的用户 ID。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs;
    /// use std::io;
    /// use std::os::redox::fs::MetadataExt;
    ///
    /// fn main() -> io::Result<()> {
    ///     let meta = fs::metadata("some_file")?;
    ///     println!("{}", meta.st_uid());
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "metadata_ext2", since = "1.8.0")]
    fn st_uid(&self) -> u32;
    /// 返回文件所有者的组 ID。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs;
    /// use std::io;
    /// use std::os::redox::fs::MetadataExt;
    ///
    /// fn main() -> io::Result<()> {
    ///     let meta = fs::metadata("some_file")?;
    ///     println!("{}", meta.st_gid());
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "metadata_ext2", since = "1.8.0")]
    fn st_gid(&self) -> u32;
    /// 返回此文件所表示的设备 ID。仅对特殊文件有意义。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs;
    /// use std::io;
    /// use std::os::redox::fs::MetadataExt;
    ///
    /// fn main() -> io::Result<()> {
    ///     let meta = fs::metadata("some_file")?;
    ///     println!("{}", meta.st_rdev());
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "metadata_ext2", since = "1.8.0")]
    fn st_rdev(&self) -> u64;
    /// 返回文件的大小（若它是常规文件或符号链接），以字节为单位。
    ///
    /// 符号链接的大小是它所包含的路径名的长度，
    /// 不包括结尾的空字节（null byte）。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs;
    /// use std::io;
    /// use std::os::redox::fs::MetadataExt;
    ///
    /// fn main() -> io::Result<()> {
    ///     let meta = fs::metadata("some_file")?;
    ///     println!("{}", meta.st_size());
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "metadata_ext2", since = "1.8.0")]
    fn st_size(&self) -> u64;
    /// 返回文件的最后访问时间，以自 Unix 纪元（Epoch）起的秒数表示。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs;
    /// use std::io;
    /// use std::os::redox::fs::MetadataExt;
    ///
    /// fn main() -> io::Result<()> {
    ///     let meta = fs::metadata("some_file")?;
    ///     println!("{}", meta.st_atime());
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "metadata_ext2", since = "1.8.0")]
    fn st_atime(&self) -> i64;
    /// 返回文件的最后访问时间，以自 [`st_atime`] 起的纳秒数表示。
    ///
    /// [`st_atime`]: Self::st_atime
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs;
    /// use std::io;
    /// use std::os::redox::fs::MetadataExt;
    ///
    /// fn main() -> io::Result<()> {
    ///     let meta = fs::metadata("some_file")?;
    ///     println!("{}", meta.st_atime_nsec());
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "metadata_ext2", since = "1.8.0")]
    fn st_atime_nsec(&self) -> i64;
    /// 返回文件的最后修改时间，以自 Unix 纪元（Epoch）起的秒数表示。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs;
    /// use std::io;
    /// use std::os::redox::fs::MetadataExt;
    ///
    /// fn main() -> io::Result<()> {
    ///     let meta = fs::metadata("some_file")?;
    ///     println!("{}", meta.st_mtime());
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "metadata_ext2", since = "1.8.0")]
    fn st_mtime(&self) -> i64;
    /// 返回文件的最后修改时间，以自 [`st_mtime`] 起的纳秒数表示。
    ///
    /// [`st_mtime`]: Self::st_mtime
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs;
    /// use std::io;
    /// use std::os::redox::fs::MetadataExt;
    ///
    /// fn main() -> io::Result<()> {
    ///     let meta = fs::metadata("some_file")?;
    ///     println!("{}", meta.st_mtime_nsec());
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "metadata_ext2", since = "1.8.0")]
    fn st_mtime_nsec(&self) -> i64;
    /// 返回文件的最后状态变更时间，以自 Unix 纪元（Epoch）起的秒数表示。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs;
    /// use std::io;
    /// use std::os::redox::fs::MetadataExt;
    ///
    /// fn main() -> io::Result<()> {
    ///     let meta = fs::metadata("some_file")?;
    ///     println!("{}", meta.st_ctime());
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "metadata_ext2", since = "1.8.0")]
    fn st_ctime(&self) -> i64;
    /// 返回文件的最后状态变更时间，以自 [`st_ctime`] 起的纳秒数表示。
    ///
    /// [`st_ctime`]: Self::st_ctime
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs;
    /// use std::io;
    /// use std::os::redox::fs::MetadataExt;
    ///
    /// fn main() -> io::Result<()> {
    ///     let meta = fs::metadata("some_file")?;
    ///     println!("{}", meta.st_ctime_nsec());
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "metadata_ext2", since = "1.8.0")]
    fn st_ctime_nsec(&self) -> i64;
    /// 返回用于高效文件系统 I/O 的“首选”块大小。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs;
    /// use std::io;
    /// use std::os::redox::fs::MetadataExt;
    ///
    /// fn main() -> io::Result<()> {
    ///     let meta = fs::metadata("some_file")?;
    ///     println!("{}", meta.st_blksize());
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "metadata_ext2", since = "1.8.0")]
    fn st_blksize(&self) -> u64;
    /// 返回分配给该文件的块数，以 512 字节为单位。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs;
    /// use std::io;
    /// use std::os::redox::fs::MetadataExt;
    ///
    /// fn main() -> io::Result<()> {
    ///     let meta = fs::metadata("some_file")?;
    ///     println!("{}", meta.st_blocks());
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "metadata_ext2", since = "1.8.0")]
    fn st_blocks(&self) -> u64;
}

#[stable(feature = "metadata_ext", since = "1.1.0")]
impl MetadataExt for Metadata {
    #[allow(deprecated)]
    fn as_raw_stat(&self) -> &raw::stat {
        unsafe { &*(self.as_inner().as_inner() as *const libc::stat as *const raw::stat) }
    }
    fn st_dev(&self) -> u64 {
        self.as_inner().as_inner().st_dev as u64
    }
    fn st_ino(&self) -> u64 {
        self.as_inner().as_inner().st_ino as u64
    }
    fn st_mode(&self) -> u32 {
        self.as_inner().as_inner().st_mode as u32
    }
    fn st_nlink(&self) -> u64 {
        self.as_inner().as_inner().st_nlink as u64
    }
    fn st_uid(&self) -> u32 {
        self.as_inner().as_inner().st_uid as u32
    }
    fn st_gid(&self) -> u32 {
        self.as_inner().as_inner().st_gid as u32
    }
    fn st_rdev(&self) -> u64 {
        self.as_inner().as_inner().st_rdev as u64
    }
    fn st_size(&self) -> u64 {
        self.as_inner().as_inner().st_size as u64
    }
    fn st_atime(&self) -> i64 {
        self.as_inner().as_inner().st_atime as i64
    }
    fn st_atime_nsec(&self) -> i64 {
        self.as_inner().as_inner().st_atime_nsec as i64
    }
    fn st_mtime(&self) -> i64 {
        self.as_inner().as_inner().st_mtime as i64
    }
    fn st_mtime_nsec(&self) -> i64 {
        self.as_inner().as_inner().st_mtime_nsec as i64
    }
    fn st_ctime(&self) -> i64 {
        self.as_inner().as_inner().st_ctime as i64
    }
    fn st_ctime_nsec(&self) -> i64 {
        self.as_inner().as_inner().st_ctime_nsec as i64
    }
    fn st_blksize(&self) -> u64 {
        self.as_inner().as_inner().st_blksize as u64
    }
    fn st_blocks(&self) -> u64 {
        self.as_inner().as_inner().st_blocks as u64
    }
}
