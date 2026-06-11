#![stable(feature = "rust1", since = "1.0.0")]
#![deny(unsafe_op_in_unsafe_fn)]

#[allow(unused_extern_crates)]
#[stable(feature = "rust1", since = "1.0.0")]
pub extern crate hermit_abi;

pub mod ffi;
pub mod io;

/// 一个用于便捷编写平台特定代码的 prelude。
///
/// 包含所有扩展 trait，以及一些重要的类型定义。
#[stable(feature = "rust1", since = "1.0.0")]
pub mod prelude {
    #[doc(no_inline)]
    #[stable(feature = "rust1", since = "1.0.0")]
    pub use super::ffi::{OsStrExt, OsStringExt};
}
