#![stable(feature = "metadata_ext", since = "1.1.0")]

use crate::fs::Metadata;
#[allow(deprecated)]
use crate::os::android::raw;
use crate::sys::AsInner;

/// 针对 [`fs::Metadata`] 的 OS 平台特有扩展。
///
/// [`fs::Metadata`]: crate::fs::Metadata
#[stable(feature = "metadata_ext", since = "1.1.0")]
pub trait MetadataExt {
    /// 获取底层 `stat` 结构的引用，其中包含 OS 返回的原始信息。
    ///
    /// 返回的 `stat` 内容在各个 Unix 平台之间**并不**一致。
    /// `os::unix::fs::MetadataExt` trait 则包含从原始 stat 中
    /// 提炼出的、跨 Unix 通用的抽象。
    #[stable(feature = "metadata_ext", since = "1.1.0")]
    #[deprecated(
        since = "1.8.0",
        note = "deprecated in favor of the accessor \
                methods of this trait"
    )]
    #[allow(deprecated)]
    fn as_raw_stat(&self) -> &raw::stat;

    #[stable(feature = "metadata_ext2", since = "1.8.0")]
    fn st_dev(&self) -> u64;
    #[stable(feature = "metadata_ext2", since = "1.8.0")]
    fn st_ino(&self) -> u64;
    #[stable(feature = "metadata_ext2", since = "1.8.0")]
    fn st_mode(&self) -> u32;
    #[stable(feature = "metadata_ext2", since = "1.8.0")]
    fn st_nlink(&self) -> u64;
    #[stable(feature = "metadata_ext2", since = "1.8.0")]
    fn st_uid(&self) -> u32;
    #[stable(feature = "metadata_ext2", since = "1.8.0")]
    fn st_gid(&self) -> u32;
    #[stable(feature = "metadata_ext2", since = "1.8.0")]
    fn st_rdev(&self) -> u64;
    #[stable(feature = "metadata_ext2", since = "1.8.0")]
    fn st_size(&self) -> u64;
    #[stable(feature = "metadata_ext2", since = "1.8.0")]
    fn st_atime(&self) -> i64;
    #[stable(feature = "metadata_ext2", since = "1.8.0")]
    fn st_atime_nsec(&self) -> i64;
    #[stable(feature = "metadata_ext2", since = "1.8.0")]
    fn st_mtime(&self) -> i64;
    #[stable(feature = "metadata_ext2", since = "1.8.0")]
    fn st_mtime_nsec(&self) -> i64;
    #[stable(feature = "metadata_ext2", since = "1.8.0")]
    fn st_ctime(&self) -> i64;
    #[stable(feature = "metadata_ext2", since = "1.8.0")]
    fn st_ctime_nsec(&self) -> i64;
    #[stable(feature = "metadata_ext2", since = "1.8.0")]
    fn st_blksize(&self) -> u64;
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
