//! 数值相关的附加功能。
//!
//! 本模块为数值计算提供一些额外的类型。其实质内容（`NonZero`、`Wrapping`、
//! `Saturating`、各类解析错误等）全部定义在 `core::num` 中，这里只是把它们
//! 重导出到 `std::num` 命名空间，以便不感知 core/std 拆分的使用者直接通过
//! `std::num` 访问。各项的具体说明请参见对应的文档。

#![stable(feature = "rust1", since = "1.0.0")]
#![allow(missing_docs)]

#[stable(feature = "int_error_matching", since = "1.55.0")]
pub use core::num::IntErrorKind;
#[stable(feature = "generic_nonzero", since = "1.79.0")]
pub use core::num::NonZero;
#[stable(feature = "saturating_int_impl", since = "1.74.0")]
pub use core::num::Saturating;
#[stable(feature = "rust1", since = "1.0.0")]
pub use core::num::Wrapping;
#[unstable(
    feature = "nonzero_internals",
    reason = "implementation detail which may disappear or be replaced at any time",
    issue = "none"
)]
pub use core::num::ZeroablePrimitive;
#[stable(feature = "rust1", since = "1.0.0")]
pub use core::num::{FpCategory, ParseFloatError, ParseIntError, TryFromIntError};
#[stable(feature = "signed_nonzero", since = "1.34.0")]
pub use core::num::{NonZeroI8, NonZeroI16, NonZeroI32, NonZeroI64, NonZeroI128, NonZeroIsize};
#[stable(feature = "nonzero", since = "1.28.0")]
pub use core::num::{NonZeroU8, NonZeroU16, NonZeroU32, NonZeroU64, NonZeroU128, NonZeroUsize};
