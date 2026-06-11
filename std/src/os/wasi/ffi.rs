//! 针对 [`std::ffi`] 模块中各类基础类型的 WASI 平台特定扩展
//!
//! [`std::ffi`]: crate::ffi

#![stable(feature = "rust1", since = "1.0.0")]

#[path = "../unix/ffi/os_str.rs"]
mod os_str;

#[stable(feature = "rust1", since = "1.0.0")]
pub use self::os_str::{OsStrExt, OsStringExt};
