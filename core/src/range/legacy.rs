//! # 旧版 range 类型
//!
//! 本模块中的类型将由父模块 [`core::range`] 中的 [`Range`]、
//! [`RangeInclusive`]、[`RangeToInclusive`] 和 [`RangeFrom`] 类型取代。
//!
//! 这里的类型等价于 [`core::ops`] 中的同名类型。

#[doc(inline)]
pub use crate::ops::{Range, RangeFrom, RangeInclusive, RangeToInclusive};
