use crate::num::TryFromIntError;

mod private {
    /// 让本 trait 从 crate 外部无法触及,从而阻止了对 `FloatToInt` trait 的
    /// 其他实现,这就为在该 trait 被标记为 `#[stable]` 之后仍有可能增加更多
    /// trait 方法留出了余地。
    #[unstable(feature = "convert_float_to_int", issue = "67057")]
    pub trait Sealed {}
}

/// 为 `f32` 和 `f64` 的固有方法(例如 `to_int_unchecked`)提供支持的 trait。
/// 通常不需要直接使用它。
#[unstable(feature = "convert_float_to_int", issue = "67057")]
pub trait FloatToInt<Int>: private::Sealed + Sized {
    #[unstable(feature = "convert_float_to_int", issue = "67057")]
    #[doc(hidden)]
    unsafe fn to_int_unchecked(self) -> Int;
}

macro_rules! impl_float_to_int {
    ($Float:ty => $($Int:ty),+) => {
        #[unstable(feature = "convert_float_to_int", issue = "67057")]
        impl private::Sealed for $Float {}
        $(
            #[unstable(feature = "convert_float_to_int", issue = "67057")]
            impl FloatToInt<$Int> for $Float {
                #[inline]
                unsafe fn to_int_unchecked(self) -> $Int {
                    // SAFETY:安全契约必须由调用者来维护。
                    unsafe { crate::intrinsics::float_to_int_unchecked(self) }
                }
            }
        )+
    }
}

impl_float_to_int!(f16 => u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize);
impl_float_to_int!(f32 => u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize);
impl_float_to_int!(f64 => u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize);
impl_float_to_int!(f128 => u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize);

// 面向原始整数和浮点类型的转换 trait
// T -> T 的转换由一条覆盖性实现覆盖,因此排除在外
// 出于可移植性方面的考虑,某些与 usize/isize 之间的转换没有实现
macro_rules! impl_from {
    (bool => $Int:ty $(,)?) => {
        impl_from!(
            bool => $Int,
            #[stable(feature = "from_bool", since = "1.28.0")],
            concat!(
                "把一个 [`bool`] 无损地转换为 [`", stringify!($Int), "`]。\n",
                "对于 `false` 值结果为 `0`,对于 `true` 值结果为 `1`。\n",
                "\n",
                "# 示例\n",
                "\n",
                "```\n",
                "assert_eq!(", stringify!($Int), "::from(true), 1);\n",
                "assert_eq!(", stringify!($Int), "::from(false), 0);\n",
                "```\n",
            ),
        );
    };
    ($Small:ty => $Large:ty, #[$attr:meta] $(,)?) => {
        impl_from!(
            $Small => $Large,
            #[$attr],
            concat!("把 [`", stringify!($Small), "`] 无损地转换为 [`", stringify!($Large), "`]。"),
        );
    };
    ($Small:ty => $Large:ty, #[$attr:meta], $doc:expr $(,)?) => {
        #[$attr]
        #[rustc_const_unstable(feature = "const_convert", issue = "143773")]
        impl const From<$Small> for $Large {
            // impl 块上的 rustdoc 会显示一个“[+] 显示未文档化条目”的切换开关。
            // 而函数上的 rustdoc 则没有。
            #[doc = $doc]
            #[inline(always)]
            fn from(small: $Small) -> Self {
                small as Self
            }
        }
    };
}

// 布尔 -> 整数
impl_from!(bool => u8);
impl_from!(bool => u16);
impl_from!(bool => u32);
impl_from!(bool => u64);
impl_from!(bool => u128);
impl_from!(bool => usize);
impl_from!(bool => i8);
impl_from!(bool => i16);
impl_from!(bool => i32);
impl_from!(bool => i64);
impl_from!(bool => i128);
impl_from!(bool => isize);

// 无符号整数 -> 无符号整数
impl_from!(u8 => u16, #[stable(feature = "lossless_int_conv", since = "1.5.0")]);
impl_from!(u8 => u32, #[stable(feature = "lossless_int_conv", since = "1.5.0")]);
impl_from!(u8 => u64, #[stable(feature = "lossless_int_conv", since = "1.5.0")]);
impl_from!(u8 => u128, #[stable(feature = "i128", since = "1.26.0")]);
impl_from!(u8 => usize, #[stable(feature = "lossless_int_conv", since = "1.5.0")]);
impl_from!(u16 => u32, #[stable(feature = "lossless_int_conv", since = "1.5.0")]);
impl_from!(u16 => u64, #[stable(feature = "lossless_int_conv", since = "1.5.0")]);
impl_from!(u16 => u128, #[stable(feature = "i128", since = "1.26.0")]);
impl_from!(u32 => u64, #[stable(feature = "lossless_int_conv", since = "1.5.0")]);
impl_from!(u32 => u128, #[stable(feature = "i128", since = "1.26.0")]);
impl_from!(u64 => u128, #[stable(feature = "i128", since = "1.26.0")]);

// 有符号整数 -> 有符号整数
impl_from!(i8 => i16, #[stable(feature = "lossless_int_conv", since = "1.5.0")]);
impl_from!(i8 => i32, #[stable(feature = "lossless_int_conv", since = "1.5.0")]);
impl_from!(i8 => i64, #[stable(feature = "lossless_int_conv", since = "1.5.0")]);
impl_from!(i8 => i128, #[stable(feature = "i128", since = "1.26.0")]);
impl_from!(i8 => isize, #[stable(feature = "lossless_int_conv", since = "1.5.0")]);
impl_from!(i16 => i32, #[stable(feature = "lossless_int_conv", since = "1.5.0")]);
impl_from!(i16 => i64, #[stable(feature = "lossless_int_conv", since = "1.5.0")]);
impl_from!(i16 => i128, #[stable(feature = "i128", since = "1.26.0")]);
impl_from!(i32 => i64, #[stable(feature = "lossless_int_conv", since = "1.5.0")]);
impl_from!(i32 => i128, #[stable(feature = "i128", since = "1.26.0")]);
impl_from!(i64 => i128, #[stable(feature = "i128", since = "1.26.0")]);

// 无符号整数 -> 有符号整数
impl_from!(u8 => i16, #[stable(feature = "lossless_int_conv", since = "1.5.0")]);
impl_from!(u8 => i32, #[stable(feature = "lossless_int_conv", since = "1.5.0")]);
impl_from!(u8 => i64, #[stable(feature = "lossless_int_conv", since = "1.5.0")]);
impl_from!(u8 => i128, #[stable(feature = "i128", since = "1.26.0")]);
impl_from!(u16 => i32, #[stable(feature = "lossless_int_conv", since = "1.5.0")]);
impl_from!(u16 => i64, #[stable(feature = "lossless_int_conv", since = "1.5.0")]);
impl_from!(u16 => i128, #[stable(feature = "i128", since = "1.26.0")]);
impl_from!(u32 => i64, #[stable(feature = "lossless_int_conv", since = "1.5.0")]);
impl_from!(u32 => i128, #[stable(feature = "i128", since = "1.26.0")]);
impl_from!(u64 => i128, #[stable(feature = "i128", since = "1.26.0")]);

// C99 标准对 INTPTR_MIN、INTPTR_MAX 和 UINTPTR_MAX 定义了边界,
// 这隐含着指针大小的整数必须至少有 16 位:
// https://port70.net/~nsz/c/c99/n1256.html#7.18.2.4
impl_from!(u16 => usize, #[stable(feature = "lossless_iusize_conv", since = "1.26.0")]);
impl_from!(u8 => isize, #[stable(feature = "lossless_iusize_conv", since = "1.26.0")]);
impl_from!(i16 => isize, #[stable(feature = "lossless_iusize_conv", since = "1.26.0")]);

// RISC-V 定义了 128 位地址空间(RV128)的可能性。

// CHERI 提出了 128 位的“能力(capability)”。尚不清楚这是否与 usize/isize 相关。
// https://www.cl.cam.ac.uk/research/security/ctsrd/pdfs/20171017a-cheri-poster.pdf
// https://www.cl.cam.ac.uk/techreports/UCAM-CL-TR-951.pdf

// 注意:整数只有在能放进尾数(significand)时,才能在浮点数中以完整精度表示,
// 尾数位数为:
// * f16 中 11 位
// * f32 中 24 位
// * f64 中 53 位
// * f128 中 113 位
// 目前尚未实现有损的浮点转换。
// FIXME(f16_f128):`f16`/`f128` 这些 impl 的 `#[stable]` 属性,应当在 `f16`/`f128`
// 被稳定化后改为引用 `f16`/`f128`(trait impl 必须带有 `#[stable]` 属性,但由于 `f16`
// 和 `f128` 类型尚不稳定,所以这些 `f16`/`f128` 的 impl 在 stable 上都无法使用)。

// 有符号整数 -> 浮点
impl_from!(i8 => f16, #[stable(feature = "lossless_float_conv", since = "1.6.0")]);
impl_from!(i8 => f32, #[stable(feature = "lossless_float_conv", since = "1.6.0")]);
impl_from!(i8 => f64, #[stable(feature = "lossless_float_conv", since = "1.6.0")]);
impl_from!(i8 => f128, #[stable(feature = "lossless_float_conv", since = "1.6.0")]);
impl_from!(i16 => f32, #[stable(feature = "lossless_float_conv", since = "1.6.0")]);
impl_from!(i16 => f64, #[stable(feature = "lossless_float_conv", since = "1.6.0")]);
impl_from!(i16 => f128, #[stable(feature = "lossless_float_conv", since = "1.6.0")]);
impl_from!(i32 => f64, #[stable(feature = "lossless_float_conv", since = "1.6.0")]);
impl_from!(i32 => f128, #[stable(feature = "lossless_float_conv", since = "1.6.0")]);
// FIXME(f16_f128):这条 impl 会让 `f128` 在稳定化之前就能在 stable 上使用。
// impl_from!(i64 => f128, #[stable(feature = "lossless_float_conv", since = "1.6.0")]);

// 无符号整数 -> 浮点
impl_from!(u8 => f16, #[stable(feature = "lossless_float_conv", since = "1.6.0")]);
impl_from!(u8 => f32, #[stable(feature = "lossless_float_conv", since = "1.6.0")]);
impl_from!(u8 => f64, #[stable(feature = "lossless_float_conv", since = "1.6.0")]);
impl_from!(u8 => f128, #[stable(feature = "lossless_float_conv", since = "1.6.0")]);
impl_from!(u16 => f32, #[stable(feature = "lossless_float_conv", since = "1.6.0")]);
impl_from!(u16 => f64, #[stable(feature = "lossless_float_conv", since = "1.6.0")]);
impl_from!(u16 => f128, #[stable(feature = "lossless_float_conv", since = "1.6.0")]);
impl_from!(u32 => f64, #[stable(feature = "lossless_float_conv", since = "1.6.0")]);
impl_from!(u32 => f128, #[stable(feature = "lossless_float_conv", since = "1.6.0")]);
// FIXME(f16_f128):这条 impl 会让 `f128` 在稳定化之前就能在 stable 上使用。
// impl_from!(u64 => f128, #[stable(feature = "lossless_float_conv", since = "1.6.0")]);

// 浮点 -> 浮点
// FIXME(f16_f128):给 `f32` 添加额外的 `From<{float}>` impl 会破坏类型推断。参见
// <https://github.com/rust-lang/rust/issues/123831>
impl_from!(f16 => f64, #[stable(feature = "lossless_float_conv", since = "1.6.0")]);
impl_from!(f16 => f128, #[stable(feature = "lossless_float_conv", since = "1.6.0")]);
impl_from!(f32 => f64, #[stable(feature = "lossless_float_conv", since = "1.6.0")]);
impl_from!(f32 => f128, #[stable(feature = "lossless_float_conv", since = "1.6.0")]);
impl_from!(f64 => f128, #[stable(feature = "lossless_float_conv", since = "1.6.0")]);

macro_rules! impl_float_from_bool {
    (
        $float:ty $(;
            doctest_prefix: $(#[doc = $doctest_prefix:literal])*
            doctest_suffix: $(#[doc = $doctest_suffix:literal])*
        )?
    ) => {
        #[stable(feature = "float_from_bool", since = "1.68.0")]
        #[rustc_const_unstable(feature = "const_convert", issue = "143773")]
            impl const From<bool> for $float {
            #[doc = concat!("把一个 [`bool`] 无损地转换为 [`", stringify!($float),"`]。")]
            /// 对于 `false` 值结果为正的 `0.0`,对于 `true` 值结果为 `1.0`。
            ///
            /// # 示例
            /// ```
            $($(#[doc = $doctest_prefix])*)?
            #[doc = concat!("let x: ", stringify!($float)," = false.into();")]
            /// assert_eq!(x, 0.0);
            /// assert!(x.is_sign_positive());
            ///
            #[doc = concat!("let y: ", stringify!($float)," = true.into();")]
            /// assert_eq!(y, 1.0);
            $($(#[doc = $doctest_suffix])*)?
            /// ```
            #[inline]
            fn from(small: bool) -> Self {
                small as u8 as Self
            }
        }
    };
}

// 布尔 -> 浮点
impl_float_from_bool!(
    f16;
    doctest_prefix:
    // rustdoc 不会移除 `///` 之后那个约定俗成的空格
    ///#![feature(f16)]
    ///# #[cfg(all(target_arch = "x86_64", target_os = "linux"))] {
    ///
    doctest_suffix:
    ///# }
);
impl_float_from_bool!(f32);
impl_float_from_bool!(f64);
impl_float_from_bool!(
    f128;
    doctest_prefix:
    ///#![feature(f128)]
    ///# #[cfg(all(target_arch = "x86_64", target_os = "linux"))] {
    ///
    doctest_suffix:
    ///# }
);

// 不可能违反边界
macro_rules! impl_try_from_unbounded {
    ($source:ty => $($target:ty),+) => {$(
        #[stable(feature = "try_from", since = "1.34.0")]
        #[rustc_const_unstable(feature = "const_convert", issue = "143773")]
        impl const TryFrom<$source> for $target {
            type Error = TryFromIntError;

            /// 尝试从源数值类型创建目标数值类型。如果源值超出目标类型的
            /// 取值范围,则返回一个错误。
            #[inline]
            fn try_from(value: $source) -> Result<Self, Self::Error> {
                Ok(value as Self)
            }
        }
    )*}
}

// 仅有负向边界
macro_rules! impl_try_from_lower_bounded {
    ($source:ty => $($target:ty),+) => {$(
        #[stable(feature = "try_from", since = "1.34.0")]
        #[rustc_const_unstable(feature = "const_convert", issue = "143773")]
        impl const TryFrom<$source> for $target {
            type Error = TryFromIntError;

            /// 尝试从源数值类型创建目标数值类型。如果源值超出目标类型的
            /// 取值范围,则返回一个错误。
            #[inline]
            fn try_from(u: $source) -> Result<Self, Self::Error> {
                if u >= 0 {
                    Ok(u as Self)
                } else {
                    Err(TryFromIntError(()))
                }
            }
        }
    )*}
}

// 无符号转有符号(仅有正向边界)
macro_rules! impl_try_from_upper_bounded {
    ($source:ty => $($target:ty),+) => {$(
        #[stable(feature = "try_from", since = "1.34.0")]
        #[rustc_const_unstable(feature = "const_convert", issue = "143773")]
        impl const TryFrom<$source> for $target {
            type Error = TryFromIntError;

            /// 尝试从源数值类型创建目标数值类型。如果源值超出目标类型的
            /// 取值范围,则返回一个错误。
            #[inline]
            fn try_from(u: $source) -> Result<Self, Self::Error> {
                if u > (Self::MAX as $source) {
                    Err(TryFromIntError(()))
                } else {
                    Ok(u as Self)
                }
            }
        }
    )*}
}

// 所有其他情形
macro_rules! impl_try_from_both_bounded {
    ($source:ty => $($target:ty),+) => {$(
        #[stable(feature = "try_from", since = "1.34.0")]
        #[rustc_const_unstable(feature = "const_convert", issue = "143773")]
        impl const TryFrom<$source> for $target {
            type Error = TryFromIntError;

            /// 尝试从源数值类型创建目标数值类型。如果源值超出目标类型的
            /// 取值范围,则返回一个错误。
            #[inline]
            fn try_from(u: $source) -> Result<Self, Self::Error> {
                let min = Self::MIN as $source;
                let max = Self::MAX as $source;
                if u < min || u > max {
                    Err(TryFromIntError(()))
                } else {
                    Ok(u as Self)
                }
            }
        }
    )*}
}

macro_rules! rev {
    ($mac:ident, $source:ty => $($target:ty),+) => {$(
        $mac!($target => $source);
    )*}
}

// 无符号整数 -> 无符号整数
impl_try_from_upper_bounded!(u16 => u8);
impl_try_from_upper_bounded!(u32 => u8, u16);
impl_try_from_upper_bounded!(u64 => u8, u16, u32);
impl_try_from_upper_bounded!(u128 => u8, u16, u32, u64);

// 有符号整数 -> 有符号整数
impl_try_from_both_bounded!(i16 => i8);
impl_try_from_both_bounded!(i32 => i8, i16);
impl_try_from_both_bounded!(i64 => i8, i16, i32);
impl_try_from_both_bounded!(i128 => i8, i16, i32, i64);

// 无符号整数 -> 有符号整数
impl_try_from_upper_bounded!(u8 => i8);
impl_try_from_upper_bounded!(u16 => i8, i16);
impl_try_from_upper_bounded!(u32 => i8, i16, i32);
impl_try_from_upper_bounded!(u64 => i8, i16, i32, i64);
impl_try_from_upper_bounded!(u128 => i8, i16, i32, i64, i128);

// 有符号整数 -> 无符号整数
impl_try_from_lower_bounded!(i8 => u8, u16, u32, u64, u128);
impl_try_from_both_bounded!(i16 => u8);
impl_try_from_lower_bounded!(i16 => u16, u32, u64, u128);
impl_try_from_both_bounded!(i32 => u8, u16);
impl_try_from_lower_bounded!(i32 => u32, u64, u128);
impl_try_from_both_bounded!(i64 => u8, u16, u32);
impl_try_from_lower_bounded!(i64 => u64, u128);
impl_try_from_both_bounded!(i128 => u8, u16, u32, u64);
impl_try_from_lower_bounded!(i128 => u128);

// usize/isize
impl_try_from_upper_bounded!(usize => isize);
impl_try_from_lower_bounded!(isize => usize);

#[cfg(target_pointer_width = "16")]
mod ptr_try_from_impls {
    use super::TryFromIntError;

    impl_try_from_upper_bounded!(usize => u8);
    impl_try_from_unbounded!(usize => u16, u32, u64, u128);
    impl_try_from_upper_bounded!(usize => i8, i16);
    impl_try_from_unbounded!(usize => i32, i64, i128);

    impl_try_from_both_bounded!(isize => u8);
    impl_try_from_lower_bounded!(isize => u16, u32, u64, u128);
    impl_try_from_both_bounded!(isize => i8);
    impl_try_from_unbounded!(isize => i16, i32, i64, i128);

    rev!(impl_try_from_upper_bounded, usize => u32, u64, u128);
    rev!(impl_try_from_lower_bounded, usize => i8, i16);
    rev!(impl_try_from_both_bounded, usize => i32, i64, i128);

    rev!(impl_try_from_upper_bounded, isize => u16, u32, u64, u128);
    rev!(impl_try_from_both_bounded, isize => i32, i64, i128);
}

#[cfg(target_pointer_width = "32")]
mod ptr_try_from_impls {
    use super::TryFromIntError;

    impl_try_from_upper_bounded!(usize => u8, u16);
    impl_try_from_unbounded!(usize => u32, u64, u128);
    impl_try_from_upper_bounded!(usize => i8, i16, i32);
    impl_try_from_unbounded!(usize => i64, i128);

    impl_try_from_both_bounded!(isize => u8, u16);
    impl_try_from_lower_bounded!(isize => u32, u64, u128);
    impl_try_from_both_bounded!(isize => i8, i16);
    impl_try_from_unbounded!(isize => i32, i64, i128);

    rev!(impl_try_from_unbounded, usize => u32);
    rev!(impl_try_from_upper_bounded, usize => u64, u128);
    rev!(impl_try_from_lower_bounded, usize => i8, i16, i32);
    rev!(impl_try_from_both_bounded, usize => i64, i128);

    rev!(impl_try_from_unbounded, isize => u16);
    rev!(impl_try_from_upper_bounded, isize => u32, u64, u128);
    rev!(impl_try_from_unbounded, isize => i32);
    rev!(impl_try_from_both_bounded, isize => i64, i128);
}

#[cfg(target_pointer_width = "64")]
mod ptr_try_from_impls {
    use super::TryFromIntError;

    impl_try_from_upper_bounded!(usize => u8, u16, u32);
    impl_try_from_unbounded!(usize => u64, u128);
    impl_try_from_upper_bounded!(usize => i8, i16, i32, i64);
    impl_try_from_unbounded!(usize => i128);

    impl_try_from_both_bounded!(isize => u8, u16, u32);
    impl_try_from_lower_bounded!(isize => u64, u128);
    impl_try_from_both_bounded!(isize => i8, i16, i32);
    impl_try_from_unbounded!(isize => i64, i128);

    rev!(impl_try_from_unbounded, usize => u32, u64);
    rev!(impl_try_from_upper_bounded, usize => u128);
    rev!(impl_try_from_lower_bounded, usize => i8, i16, i32, i64);
    rev!(impl_try_from_both_bounded, usize => i128);

    rev!(impl_try_from_unbounded, isize => u16, u32);
    rev!(impl_try_from_upper_bounded, isize => u64, u128);
    rev!(impl_try_from_unbounded, isize => i32, i64);
    rev!(impl_try_from_both_bounded, isize => i128);
}

// 面向非零整数类型的转换 trait
use crate::num::NonZero;

macro_rules! impl_nonzero_int_from_nonzero_int {
    ($Small:ty => $Large:ty) => {
        #[stable(feature = "nz_int_conv", since = "1.41.0")]
        #[rustc_const_unstable(feature = "const_convert", issue = "143773")]
        impl const From<NonZero<$Small>> for NonZero<$Large> {
            // impl 块上的 rustdoc 会显示一个“[+] 显示未文档化条目”的切换开关。
            // 而函数上的 rustdoc 则没有。
            #[doc = concat!("Converts <code>[NonZero]\\<[", stringify!($Small), "]></code> ")]
            #[doc = concat!("to <code>[NonZero]\\<[", stringify!($Large), "]></code> losslessly.")]
            #[inline]
            fn from(small: NonZero<$Small>) -> Self {
                // SAFETY: input type guarantees the value is non-zero
                unsafe { Self::new_unchecked(From::from(small.get())) }
            }
        }
    };
}

// 非零无符号整数 -> 非零无符号整数
impl_nonzero_int_from_nonzero_int!(u8 => u16);
impl_nonzero_int_from_nonzero_int!(u8 => u32);
impl_nonzero_int_from_nonzero_int!(u8 => u64);
impl_nonzero_int_from_nonzero_int!(u8 => u128);
impl_nonzero_int_from_nonzero_int!(u8 => usize);
impl_nonzero_int_from_nonzero_int!(u16 => u32);
impl_nonzero_int_from_nonzero_int!(u16 => u64);
impl_nonzero_int_from_nonzero_int!(u16 => u128);
impl_nonzero_int_from_nonzero_int!(u16 => usize);
impl_nonzero_int_from_nonzero_int!(u32 => u64);
impl_nonzero_int_from_nonzero_int!(u32 => u128);
impl_nonzero_int_from_nonzero_int!(u64 => u128);

// 非零有符号整数 -> 非零有符号整数
impl_nonzero_int_from_nonzero_int!(i8 => i16);
impl_nonzero_int_from_nonzero_int!(i8 => i32);
impl_nonzero_int_from_nonzero_int!(i8 => i64);
impl_nonzero_int_from_nonzero_int!(i8 => i128);
impl_nonzero_int_from_nonzero_int!(i8 => isize);
impl_nonzero_int_from_nonzero_int!(i16 => i32);
impl_nonzero_int_from_nonzero_int!(i16 => i64);
impl_nonzero_int_from_nonzero_int!(i16 => i128);
impl_nonzero_int_from_nonzero_int!(i16 => isize);
impl_nonzero_int_from_nonzero_int!(i32 => i64);
impl_nonzero_int_from_nonzero_int!(i32 => i128);
impl_nonzero_int_from_nonzero_int!(i64 => i128);

// 非零无符号 -> 非零有符号整数
impl_nonzero_int_from_nonzero_int!(u8 => i16);
impl_nonzero_int_from_nonzero_int!(u8 => i32);
impl_nonzero_int_from_nonzero_int!(u8 => i64);
impl_nonzero_int_from_nonzero_int!(u8 => i128);
impl_nonzero_int_from_nonzero_int!(u8 => isize);
impl_nonzero_int_from_nonzero_int!(u16 => i32);
impl_nonzero_int_from_nonzero_int!(u16 => i64);
impl_nonzero_int_from_nonzero_int!(u16 => i128);
impl_nonzero_int_from_nonzero_int!(u32 => i64);
impl_nonzero_int_from_nonzero_int!(u32 => i128);
impl_nonzero_int_from_nonzero_int!(u64 => i128);

macro_rules! impl_nonzero_int_try_from_int {
    ($Int:ty) => {
        #[stable(feature = "nzint_try_from_int_conv", since = "1.46.0")]
        #[rustc_const_unstable(feature = "const_convert", issue = "143773")]
        impl const TryFrom<$Int> for NonZero<$Int> {
            type Error = TryFromIntError;

            // impl 块上的 rustdoc 会显示一个“[+] 显示未文档化条目”的切换开关。
            // 而函数上的 rustdoc 则没有。
            #[doc = concat!("Attempts to convert [`", stringify!($Int), "`] ")]
            #[doc = concat!("to <code>[NonZero]\\<[", stringify!($Int), "]></code>.")]
            #[inline]
            fn try_from(value: $Int) -> Result<Self, Self::Error> {
                Self::new(value).ok_or(TryFromIntError(()))
            }
        }
    };
}

// 整数 -> 非零整数
impl_nonzero_int_try_from_int!(u8);
impl_nonzero_int_try_from_int!(u16);
impl_nonzero_int_try_from_int!(u32);
impl_nonzero_int_try_from_int!(u64);
impl_nonzero_int_try_from_int!(u128);
impl_nonzero_int_try_from_int!(usize);
impl_nonzero_int_try_from_int!(i8);
impl_nonzero_int_try_from_int!(i16);
impl_nonzero_int_try_from_int!(i32);
impl_nonzero_int_try_from_int!(i64);
impl_nonzero_int_try_from_int!(i128);
impl_nonzero_int_try_from_int!(isize);

macro_rules! impl_nonzero_int_try_from_nonzero_int {
    ($source:ty => $($target:ty),+) => {$(
        #[stable(feature = "nzint_try_from_nzint_conv", since = "1.49.0")]
        #[rustc_const_unstable(feature = "const_convert", issue = "143773")]
        impl const TryFrom<NonZero<$source>> for NonZero<$target> {
            type Error = TryFromIntError;

            // impl 块上的 rustdoc 会显示一个“[+] 显示未文档化条目”的切换开关。
            // 而函数上的 rustdoc 则没有。
            #[doc = concat!("Attempts to convert <code>[NonZero]\\<[", stringify!($source), "]></code> ")]
            #[doc = concat!("to <code>[NonZero]\\<[", stringify!($target), "]></code>.")]
            #[inline]
            fn try_from(value: NonZero<$source>) -> Result<Self, Self::Error> {
                // SAFETY: Input is guaranteed to be non-zero.
                Ok(unsafe { Self::new_unchecked(<$target>::try_from(value.get())?) })
            }
        }
    )*};
}

// 非零无符号整数 -> 非零无符号整数
impl_nonzero_int_try_from_nonzero_int!(u16 => u8);
impl_nonzero_int_try_from_nonzero_int!(u32 => u8, u16, usize);
impl_nonzero_int_try_from_nonzero_int!(u64 => u8, u16, u32, usize);
impl_nonzero_int_try_from_nonzero_int!(u128 => u8, u16, u32, u64, usize);
impl_nonzero_int_try_from_nonzero_int!(usize => u8, u16, u32, u64, u128);

// 非零有符号整数 -> 非零有符号整数
impl_nonzero_int_try_from_nonzero_int!(i16 => i8);
impl_nonzero_int_try_from_nonzero_int!(i32 => i8, i16, isize);
impl_nonzero_int_try_from_nonzero_int!(i64 => i8, i16, i32, isize);
impl_nonzero_int_try_from_nonzero_int!(i128 => i8, i16, i32, i64, isize);
impl_nonzero_int_try_from_nonzero_int!(isize => i8, i16, i32, i64, i128);

// 非零无符号整数 -> 非零有符号整数
impl_nonzero_int_try_from_nonzero_int!(u8 => i8);
impl_nonzero_int_try_from_nonzero_int!(u16 => i8, i16, isize);
impl_nonzero_int_try_from_nonzero_int!(u32 => i8, i16, i32, isize);
impl_nonzero_int_try_from_nonzero_int!(u64 => i8, i16, i32, i64, isize);
impl_nonzero_int_try_from_nonzero_int!(u128 => i8, i16, i32, i64, i128, isize);
impl_nonzero_int_try_from_nonzero_int!(usize => i8, i16, i32, i64, i128, isize);

// 非零有符号整数 -> 非零无符号整数
impl_nonzero_int_try_from_nonzero_int!(i8 => u8, u16, u32, u64, u128, usize);
impl_nonzero_int_try_from_nonzero_int!(i16 => u8, u16, u32, u64, u128, usize);
impl_nonzero_int_try_from_nonzero_int!(i32 => u8, u16, u32, u64, u128, usize);
impl_nonzero_int_try_from_nonzero_int!(i64 => u8, u16, u32, u64, u128, usize);
impl_nonzero_int_try_from_nonzero_int!(i128 => u8, u16, u32, u64, u128, usize);
impl_nonzero_int_try_from_nonzero_int!(isize => u8, u16, u32, u64, u128, usize);
