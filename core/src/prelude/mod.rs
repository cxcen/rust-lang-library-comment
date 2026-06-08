//! core prelude
//!
//! 本模块面向使用 core 且不同时链接 std 的用户。
//! 使用 `#![no_std]` 时，本模块会以与标准库 prelude 相同的方式默认导入。

// 不要格式化：本文件只有重新导出，且值得保留它们的顺序。
#![cfg_attr(rustfmt, rustfmt::skip)]

#![stable(feature = "core_prelude", since = "1.4.0")]

pub mod v1;

/// core prelude 的 2015 版本。
///
/// 更多内容见[模块级文档](self)。
#[stable(feature = "prelude_2015", since = "1.55.0")]
pub mod rust_2015 {
    #[stable(feature = "prelude_2015", since = "1.55.0")]
    #[doc(no_inline)]
    pub use super::v1::*;
}

/// core prelude 的 2018 版本。
///
/// 更多内容见[模块级文档](self)。
#[stable(feature = "prelude_2018", since = "1.55.0")]
pub mod rust_2018 {
    #[stable(feature = "prelude_2018", since = "1.55.0")]
    #[doc(no_inline)]
    pub use super::v1::*;
}

/// core prelude 的 2021 版本。
///
/// 更多内容见[模块级文档](self)。
#[stable(feature = "prelude_2021", since = "1.55.0")]
pub mod rust_2021 {
    #[stable(feature = "prelude_2021", since = "1.55.0")]
    #[doc(no_inline)]
    pub use super::v1::*;

    #[stable(feature = "prelude_2021", since = "1.55.0")]
    #[doc(no_inline)]
    pub use crate::iter::FromIterator;

    #[stable(feature = "prelude_2021", since = "1.55.0")]
    #[doc(no_inline)]
    pub use crate::convert::{TryFrom, TryInto};
}

/// core prelude 的 2024 版本。
///
/// 更多内容见[模块级文档](self)。
#[stable(feature = "prelude_2024", since = "1.85.0")]
pub mod rust_2024 {
    #[stable(feature = "rust1", since = "1.0.0")]
    #[doc(no_inline)]
    pub use super::v1::*;

    #[stable(feature = "prelude_2021", since = "1.55.0")]
    #[doc(no_inline)]
    pub use crate::iter::FromIterator;

    #[stable(feature = "prelude_2021", since = "1.55.0")]
    #[doc(no_inline)]
    pub use crate::convert::{TryFrom, TryInto};

    #[stable(feature = "prelude_2024", since = "1.85.0")]
    #[doc(no_inline)]
    pub use crate::future::{Future, IntoFuture};
}

/// core prelude 的未来版本。
///
/// 更多内容见[模块级文档](self)。
#[doc(hidden)]
#[unstable(feature = "prelude_future", issue = "none")]
pub mod rust_future {
    #[stable(feature = "rust1", since = "1.0.0")]
    #[doc(no_inline)]
    pub use super::v1::*;

    #[stable(feature = "prelude_2021", since = "1.55.0")]
    #[doc(no_inline)]
    pub use crate::iter::FromIterator;

    #[stable(feature = "prelude_2021", since = "1.55.0")]
    #[doc(no_inline)]
    pub use crate::convert::{TryFrom, TryInto};

    #[stable(feature = "prelude_2024", since = "1.85.0")]
    #[doc(no_inline)]
    pub use crate::future::{Future, IntoFuture};
}
