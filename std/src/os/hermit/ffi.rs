//! 针对 `std::ffi` 模块中各类基础类型的 HermitCore 平台特定扩展
//!
//! # 示例
//!
//! ```
//! use std::ffi::OsString;
//! use std::os::hermit::ffi::OsStringExt;
//!
//! let bytes = b"foo".to_vec();
//!
//! // OsStringExt::from_vec
//! let os_string = OsString::from_vec(bytes);
//! assert_eq!(os_string.to_str(), Some("foo"));
//!
//! // OsStringExt::into_vec
//! let bytes = os_string.into_vec();
//! assert_eq!(bytes, b"foo");
//! ```
//!
//! ```
//! use std::ffi::OsStr;
//! use std::os::hermit::ffi::OsStrExt;
//!
//! let bytes = b"foo";
//!
//! // OsStrExt::from_bytes
//! let os_str = OsStr::from_bytes(bytes);
//! assert_eq!(os_str.to_str(), Some("foo"));
//!
//! // OsStrExt::as_bytes
//! let bytes = os_str.as_bytes();
//! assert_eq!(bytes, b"foo");
//! ```

#![stable(feature = "rust1", since = "1.0.0")]

#[path = "../unix/ffi/os_str.rs"]
mod os_str;

#[stable(feature = "rust1", since = "1.0.0")]
pub use self::os_str::{OsStrExt, OsStringExt};
