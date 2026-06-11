//! Windows 平台对 `std` 的特定扩展。
//!
//! 提供对 Windows 平台级信息的访问，并暴露一些 Windows 特有的惯用法——
//! 这些惯用法若直接放进核心 `std` 库中会显得不合适。这些扩展让开发者得以在
//! Windows 上以平台无关惯用法本来无法支持的方式使用 `std` 类型与惯用法。
//!
//! # 示例
//!
//! ```no_run
//! use std::fs::File;
//! use std::os::windows::prelude::*;
//!
//! fn main() -> std::io::Result<()> {
//!     let f = File::create("foo.txt")?;
//!     let handle = f.as_raw_handle();
//!
//!     // 将 handle 与原生 windows 绑定一起使用
//!
//!     Ok(())
//! }
//! ```

#![stable(feature = "rust1", since = "1.0.0")]
#![doc(cfg(windows))]
#![deny(unsafe_op_in_unsafe_fn)]

pub mod ffi;
pub mod fs;
pub mod io;
pub mod process;
pub mod raw;
pub mod thread;

/// 一个 prelude，便于编写平台特定的代码。
///
/// 包含所有扩展 trait，以及一些重要的类型定义。
#[stable(feature = "rust1", since = "1.0.0")]
pub mod prelude {
    #[doc(no_inline)]
    #[stable(feature = "rust1", since = "1.0.0")]
    pub use super::ffi::{OsStrExt, OsStringExt};
    #[doc(no_inline)]
    #[stable(feature = "file_offset", since = "1.15.0")]
    pub use super::fs::FileExt;
    #[doc(no_inline)]
    #[stable(feature = "rust1", since = "1.0.0")]
    pub use super::fs::{MetadataExt, OpenOptionsExt};
    #[doc(no_inline)]
    #[stable(feature = "rust1", since = "1.0.0")]
    pub use super::io::{
        AsHandle, AsSocket, BorrowedHandle, BorrowedSocket, FromRawHandle, FromRawSocket,
        HandleOrInvalid, IntoRawHandle, IntoRawSocket, OwnedHandle, OwnedSocket,
    };
    #[doc(no_inline)]
    #[stable(feature = "rust1", since = "1.0.0")]
    pub use super::io::{AsRawHandle, AsRawSocket, RawHandle, RawSocket};
}
