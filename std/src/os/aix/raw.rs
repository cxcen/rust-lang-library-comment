//! AIX 平台特有的原始类型定义。

#![stable(feature = "raw_ext", since = "1.1.0")]

#[stable(feature = "pthread_t", since = "1.8.0")]
pub use libc::pthread_t;
#[stable(feature = "raw_ext", since = "1.1.0")]
pub use libc::{blkcnt_t, blksize_t, dev_t, ino_t, mode_t, nlink_t, off_t, stat, time_t};
