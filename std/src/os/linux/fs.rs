//! 针对 [`std::fs`] 模块中各类型的 Linux 平台特有扩展。
//!
//! [`std::fs`]: crate::fs

#![stable(feature = "metadata_ext", since = "1.1.0")]

use crate::fs::Metadata;
#[allow(deprecated)]
use crate::os::linux::raw;
use crate::sys::AsInner;

/// 针对 [`fs::Metadata`] 的 OS 平台特有扩展。
///
/// [`fs::Metadata`]: crate::fs::Metadata
#[stable(feature = "metadata_ext", since = "1.1.0")]
pub trait MetadataExt {
    /// 获取底层 `stat` 结构的引用，其中包含 OS 返回的原始信息。
    ///
    /// 返回的 [`stat`] 内容在各个 Unix 平台之间**并不**一致。
    /// `os::unix::fs::MetadataExt` trait 则包含从原始 stat 中
    /// 提炼出的、跨 Unix 通用的抽象。
    ///
    /// [`stat`]: struct@crate::os::linux::raw::stat
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs;
    /// use std::io;
    /// use std::os::linux::fs::MetadataExt;
    ///
    /// fn main() -> io::Result<()> {
    ///     let meta = fs::metadata("some_file")?;
    ///     let stat = meta.as_raw_stat();
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "metadata_ext", since = "1.1.0")]
    #[deprecated(since = "1.8.0", note = "other methods of this trait are now preferred")]
    #[allow(deprecated)]
    fn as_raw_stat(&self) -> &raw::stat;

    /// 返回该文件所在的设备 ID。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs;
    /// use std::io;
    /// use std::os::linux::fs::MetadataExt;
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
    /// use std::os::linux::fs::MetadataExt;
    ///
    /// fn main() -> io::Result<()> {
    ///     let meta = fs::metadata("some_file")?;
    ///     println!("{}", meta.st_ino());
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "metadata_ext2", since = "1.8.0")]
    fn st_ino(&self) -> u64;
    /// 返回文件类型与权限模式（mode）。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs;
    /// use std::io;
    /// use std::os::linux::fs::MetadataExt;
    ///
    /// fn main() -> io::Result<()> {
    ///     let meta = fs::metadata("some_file")?;
    ///     println!("{}", meta.st_mode());
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "metadata_ext2", since = "1.8.0")]
    fn st_mode(&self) -> u32;
    /// 返回指向该文件的硬链接数量。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs;
    /// use std::io;
    /// use std::os::linux::fs::MetadataExt;
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
    /// use std::os::linux::fs::MetadataExt;
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
    /// use std::os::linux::fs::MetadataExt;
    ///
    /// fn main() -> io::Result<()> {
    ///     let meta = fs::metadata("some_file")?;
    ///     println!("{}", meta.st_gid());
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "metadata_ext2", since = "1.8.0")]
    fn st_gid(&self) -> u32;
    /// 返回该文件所代表的设备 ID。仅对特殊文件（special file）有意义。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs;
    /// use std::io;
    /// use std::os::linux::fs::MetadataExt;
    ///
    /// fn main() -> io::Result<()> {
    ///     let meta = fs::metadata("some_file")?;
    ///     println!("{}", meta.st_rdev());
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "metadata_ext2", since = "1.8.0")]
    fn st_rdev(&self) -> u64;
    /// 返回文件大小（字节数，仅当其为常规文件或符号链接时）。
    ///
    /// 符号链接的大小为它所包含的路径名长度，不含结尾的空字节（null byte）。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs;
    /// use std::io;
    /// use std::os::linux::fs::MetadataExt;
    ///
    /// fn main() -> io::Result<()> {
    ///     let meta = fs::metadata("some_file")?;
    ///     println!("{}", meta.st_size());
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "metadata_ext2", since = "1.8.0")]
    fn st_size(&self) -> u64;
    /// 返回文件的最后访问时间，以自 Unix 纪元（Epoch）以来的秒数表示。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs;
    /// use std::io;
    /// use std::os::linux::fs::MetadataExt;
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
    /// use std::os::linux::fs::MetadataExt;
    ///
    /// fn main() -> io::Result<()> {
    ///     let meta = fs::metadata("some_file")?;
    ///     println!("{}", meta.st_atime_nsec());
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "metadata_ext2", since = "1.8.0")]
    fn st_atime_nsec(&self) -> i64;
    /// 返回文件的最后修改时间，以自 Unix 纪元（Epoch）以来的秒数表示。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs;
    /// use std::io;
    /// use std::os::linux::fs::MetadataExt;
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
    /// use std::os::linux::fs::MetadataExt;
    ///
    /// fn main() -> io::Result<()> {
    ///     let meta = fs::metadata("some_file")?;
    ///     println!("{}", meta.st_mtime_nsec());
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "metadata_ext2", since = "1.8.0")]
    fn st_mtime_nsec(&self) -> i64;
    /// 返回文件的最后状态变更时间，以自 Unix 纪元（Epoch）以来的秒数表示。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs;
    /// use std::io;
    /// use std::os::linux::fs::MetadataExt;
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
    /// use std::os::linux::fs::MetadataExt;
    ///
    /// fn main() -> io::Result<()> {
    ///     let meta = fs::metadata("some_file")?;
    ///     println!("{}", meta.st_ctime_nsec());
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "metadata_ext2", since = "1.8.0")]
    fn st_ctime_nsec(&self) -> i64;
    /// 返回用于高效文件系统 I/O 的“首选”（preferred）块大小。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs;
    /// use std::io;
    /// use std::os::linux::fs::MetadataExt;
    ///
    /// fn main() -> io::Result<()> {
    ///     let meta = fs::metadata("some_file")?;
    ///     println!("{}", meta.st_blksize());
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "metadata_ext2", since = "1.8.0")]
    fn st_blksize(&self) -> u64;
    /// 返回分配给该文件的块数量，以 512 字节为单位。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs;
    /// use std::io;
    /// use std::os::linux::fs::MetadataExt;
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
        #[cfg(target_env = "musl")]
        unsafe {
            &*(self.as_inner().as_inner() as *const libc::stat as *const raw::stat)
        }
        #[cfg(not(target_env = "musl"))]
        unsafe {
            &*(self.as_inner().as_inner() as *const libc::stat64 as *const raw::stat)
        }
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
        let file_attr = self.as_inner();
        #[cfg(all(target_env = "gnu", target_pointer_width = "32"))]
        if let Some(atime) = file_attr.stx_atime() {
            return atime.tv_sec;
        }
        file_attr.as_inner().st_atime as i64
    }
    fn st_atime_nsec(&self) -> i64 {
        self.as_inner().as_inner().st_atime_nsec as i64
    }
    fn st_mtime(&self) -> i64 {
        let file_attr = self.as_inner();
        #[cfg(all(target_env = "gnu", target_pointer_width = "32"))]
        if let Some(mtime) = file_attr.stx_mtime() {
            return mtime.tv_sec;
        }
        file_attr.as_inner().st_mtime as i64
    }
    fn st_mtime_nsec(&self) -> i64 {
        self.as_inner().as_inner().st_mtime_nsec as i64
    }
    fn st_ctime(&self) -> i64 {
        let file_attr = self.as_inner();
        #[cfg(all(target_env = "gnu", target_pointer_width = "32"))]
        if let Some(ctime) = file_attr.stx_ctime() {
            return ctime.tv_sec;
        }
        file_attr.as_inner().st_ctime as i64
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
