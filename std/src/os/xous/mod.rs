#![stable(feature = "rust1", since = "1.0.0")]
#![doc(cfg(target_os = "xous"))]
#![forbid(unsafe_op_in_unsafe_fn)]

pub mod ffi;

#[stable(feature = "rust1", since = "1.0.0")]
pub mod services;

/// 一个 prelude，便于编写平台特定的代码。
///
/// 包含所有扩展 trait，以及一些重要的类型定义。
#[stable(feature = "rust1", since = "1.0.0")]
pub mod prelude {
    #[doc(no_inline)]
    #[stable(feature = "rust1", since = "1.0.0")]
    pub use super::ffi::{OsStrExt, OsStringExt};
}
