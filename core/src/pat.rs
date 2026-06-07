//! 导出 `pattern_type` 宏所需的辅助模块。

use crate::marker::{Freeze, PointeeSized, Unsize};
use crate::ops::{CoerceUnsized, DispatchFromDyn};

/// 创建一个 pattern type。
/// ```ignore (cannot test this from within core yet)
/// type Positive = std::pat::pattern_type!(i32 is 1..);
/// ```
#[macro_export]
#[rustc_builtin_macro(pattern_type)]
#[unstable(feature = "pattern_type_macro", issue = "123646")]
macro_rules! pattern_type {
    ($($arg:tt)*) => {
        /* compiler built-in */
    };
}

/// 由整数类型和 `char` 实现的 trait。
/// 未来可用于泛型 pattern type；目前则用于简化 pattern type range 的 AST lowering。
#[unstable(feature = "pattern_type_range_trait", issue = "123646")]
#[rustc_const_unstable(feature = "pattern_type_range_trait", issue = "123646")]
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a valid base type for range patterns",
    label = "only integer types and `char` are supported"
)]
pub const trait RangePattern {
    /// inherent `MIN` 关联常量的 trait 版本。
    #[lang = "RangeMin"]
    const MIN: Self;

    /// inherent `MAX` 关联常量的 trait 版本。
    #[lang = "RangeMax"]
    const MAX: Self;

    /// 为排除式 range 减 1 的编译期辅助函数。
    #[lang = "RangeSub"]
    #[track_caller]
    fn sub_one(self) -> Self;
}

macro_rules! impl_range_pat {
    ($($ty:ty,)*) => {
        $(
            #[rustc_const_unstable(feature = "pattern_type_range_trait", issue = "123646")]
            impl const RangePattern for $ty {
                const MIN: $ty = <$ty>::MIN;
                const MAX: $ty = <$ty>::MAX;
                fn sub_one(self) -> Self {
                    match self.checked_sub(1) {
                        Some(val) => val,
                        None => panic!("exclusive range end at minimum value of type")
                    }
                }
            }
        )*
    }
}

impl_range_pat! {
    i8, i16, i32, i64, i128, isize,
    u8, u16, u32, u64, u128, usize,
}

#[rustc_const_unstable(feature = "pattern_type_range_trait", issue = "123646")]
impl const RangePattern for char {
    const MIN: Self = char::MIN;

    const MAX: Self = char::MAX;

    fn sub_one(self) -> Self {
        match char::from_u32(self as u32 - 1) {
            None => panic!("exclusive range to start of valid chars"),
            Some(val) => val,
        }
    }
}

impl<T: PointeeSized, U: PointeeSized> CoerceUnsized<pattern_type!(*const U is !null)> for pattern_type!(*const T is !null) where
    T: Unsize<U>
{
}

impl<T: DispatchFromDyn<U>, U> DispatchFromDyn<pattern_type!(U is !null)> for pattern_type!(T is !null) {}

impl<T: PointeeSized> Unpin for pattern_type!(*const T is !null) {}

unsafe impl<T: PointeeSized> Freeze for pattern_type!(*const T is !null) {}

unsafe impl<T: PointeeSized> Freeze for pattern_type!(*mut T is !null) {}
