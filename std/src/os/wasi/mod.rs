//! 针对 WebAssembly System Interface (WASI) 的 `std` 平台特定扩展。
//!
//! 提供对 WASI 上平台级信息的访问，并暴露一些 WASI 特定的函数 ——
//! 这些函数若作为核心 `std` 库的一部分则并不合适。
//!
//! 它暴露了更多处理平台特定字符串（`OsStr`、`OsString`）的方式，允许更细粒度地
//! 设置权限，从文件和套接字中提取底层文件描述符，并提供了用于派生进程的平台特定辅助工具。
//!
//! # 示例
//!
//! ```no_run
//! use std::fs::File;
//! use std::os::wasi::prelude::*;
//!
//! fn main() -> std::io::Result<()> {
//!     let f = File::create("foo.txt")?;
//!     let fd = f.as_raw_fd();
//!
//!     // 将 fd 用于原生的 WASI 绑定
//!
//!     Ok(())
//! }
//! ```
//!
//! [`OsStr`]: crate::ffi::OsStr
//! [`OsString`]: crate::ffi::OsString

#![cfg_attr(not(target_env = "p2"), stable(feature = "rust1", since = "1.0.0"))]
#![cfg_attr(target_env = "p2", unstable(feature = "wasip2", issue = "none"))]
#![forbid(unsafe_op_in_unsafe_fn)]
#![doc(cfg(target_os = "wasi"))]

pub mod ffi;
pub mod fs;
pub mod io;

#[cfg(all(target_os = "wasi", target_env = "p1"))]
pub mod net;

/// 一个用于便捷编写平台特定代码的 prelude。
///
/// 包含所有扩展 trait，以及一些重要的类型定义。
#[stable(feature = "rust1", since = "1.0.0")]
pub mod prelude {
    #[doc(no_inline)]
    #[stable(feature = "rust1", since = "1.0.0")]
    pub use super::ffi::{OsStrExt, OsStringExt};
    #[doc(no_inline)]
    #[stable(feature = "rust1", since = "1.0.0")]
    pub use super::fs::FileTypeExt;
    #[doc(no_inline)]
    #[stable(feature = "rust1", since = "1.0.0")]
    pub use super::fs::{DirEntryExt, FileExt, MetadataExt, OpenOptionsExt};
    #[doc(no_inline)]
    #[stable(feature = "rust1", since = "1.0.0")]
    pub use super::io::{AsFd, AsRawFd, BorrowedFd, FromRawFd, IntoRawFd, OwnedFd, RawFd};
}
