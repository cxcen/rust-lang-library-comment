//! `Saturating<T>` 的定义。
//!
//! `Saturating` 把整数运算显式解释为饱和算术：当数学结果超出类型范围时，不回绕也不
//! panic，而是夹到该类型的 `MIN` 或 `MAX`。这适合计数器、音频采样、像素通道等需要
//! “到边界为止”的场景。

use crate::fmt;
use crate::ops::{
    Add, AddAssign, BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Div, DivAssign,
    Mul, MulAssign, Neg, Not, Rem, RemAssign, Sub, SubAssign,
};

/// 为 `T` 提供有意的 saturating 算术。
///
/// 对 `u32` 这样的普通整数执行 `+` 时，通常认为溢出不是业务语义；某些 debug 配置会检测
/// 溢出并 panic。但有些代码明确需要并依赖饱和语义，即结果超出范围时停在边界值。
///
/// saturating 算术既可以通过 `saturating_add` 这类方法逐次表达，也可以通过
/// `Saturating<T>` 类型表达。后者表示底层值上的标准算术操作整体都采用 saturating 语义。
///
/// 可以通过 `Saturating` 元组的 `.0` 字段取回底层值。
///
/// # 示例
///
/// ```
/// use std::num::Saturating;
///
/// let max = Saturating(u32::MAX);
/// let one = Saturating(1u32);
///
/// assert_eq!(u32::MAX, (max + one).0);
/// ```
#[stable(feature = "saturating_int_impl", since = "1.74.0")]
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Default, Hash)]
#[repr(transparent)]
#[rustc_diagnostic_item = "Saturating"]
pub struct Saturating<T>(#[stable(feature = "saturating_int_impl", since = "1.74.0")] pub T);

#[stable(feature = "saturating_int_impl", since = "1.74.0")]
impl<T: fmt::Debug> fmt::Debug for Saturating<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[stable(feature = "saturating_int_impl", since = "1.74.0")]
impl<T: fmt::Display> fmt::Display for Saturating<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[stable(feature = "saturating_int_impl", since = "1.74.0")]
impl<T: fmt::Binary> fmt::Binary for Saturating<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[stable(feature = "saturating_int_impl", since = "1.74.0")]
impl<T: fmt::Octal> fmt::Octal for Saturating<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[stable(feature = "saturating_int_impl", since = "1.74.0")]
impl<T: fmt::LowerHex> fmt::LowerHex for Saturating<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[stable(feature = "saturating_int_impl", since = "1.74.0")]
impl<T: fmt::UpperHex> fmt::UpperHex for Saturating<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

// FIXME 正确实现尚不明确。等待真实用例，见 https://github.com/rust-lang/libs-team/issues/230
//
// #[allow(unused_macros)]
// macro_rules! sh_impl_signed {
//     ($t:ident, $f:ident) => {
//         // FIXME 这里正确实现尚不清楚，见讨论 https://github.com/rust-lang/rust/pull/87921#discussion_r695870065
//         //
//         // #[unstable(feature = "saturating_int_impl", issue = "87920")]
//         // impl Shl<$f> for Saturating<$t> {
//         //     type Output = Saturating<$t>;
//         //
//         //     #[inline]
//         //     fn shl(self, other: $f) -> Saturating<$t> {
//         //         if other < 0 {
//         //             Saturating(self.0.shr((-other & self::shift_max::$t as $f) as u32))
//         //         } else {
//         //             Saturating(self.0.shl((other & self::shift_max::$t as $f) as u32))
//         //         }
//         //     }
//         // }
//         // forward_ref_binop! { impl Shl, shl for Saturating<$t>, $f,
//         // #[unstable(feature = "saturating_int_impl", issue = "87920")] }
//         //
//         // #[unstable(feature = "saturating_int_impl", issue = "87920")]
//         // impl ShlAssign<$f> for Saturating<$t> {
//         //     #[inline]
//         //     fn shl_assign(&mut self, other: $f) {
//         //         *self = *self << other;
//         //     }
//         // }
//         // forward_ref_op_assign! { impl ShlAssign, shl_assign for Saturating<$t>, $f,
//         // #[unstable(feature = "saturating_int_impl", issue = "87920")] }
//
//         #[unstable(feature = "saturating_int_impl", issue = "87920")]
//         impl Shr<$f> for Saturating<$t> {
//             type Output = Saturating<$t>;
//
//             #[inline]
//             fn shr(self, other: $f) -> Saturating<$t> {
//                 if other < 0 {
//                     Saturating(self.0.shl((-other & self::shift_max::$t as $f) as u32))
//                 } else {
//                     Saturating(self.0.shr((other & self::shift_max::$t as $f) as u32))
//                 }
//             }
//         }
//         forward_ref_binop! { impl Shr, shr for Saturating<$t>, $f,
//         #[unstable(feature = "saturating_int_impl", issue = "87920")] }
//
//         #[unstable(feature = "saturating_int_impl", issue = "87920")]
//         impl ShrAssign<$f> for Saturating<$t> {
//             #[inline]
//             fn shr_assign(&mut self, other: $f) {
//                 *self = *self >> other;
//             }
//         }
//         forward_ref_op_assign! { impl ShrAssign, shr_assign for Saturating<$t>, $f,
//         #[unstable(feature = "saturating_int_impl", issue = "87920")] }
//     };
// }
//
// macro_rules! sh_impl_unsigned {
//     ($t:ident, $f:ident) => {
//         #[unstable(feature = "saturating_int_impl", issue = "87920")]
//         impl Shl<$f> for Saturating<$t> {
//             type Output = Saturating<$t>;
//
//             #[inline]
//             fn shl(self, other: $f) -> Saturating<$t> {
//                 Saturating(self.0.wrapping_shl(other as u32))
//             }
//         }
//         forward_ref_binop! { impl Shl, shl for Saturating<$t>, $f,
//         #[unstable(feature = "saturating_int_impl", issue = "87920")] }
//
//         #[unstable(feature = "saturating_int_impl", issue = "87920")]
//         impl ShlAssign<$f> for Saturating<$t> {
//             #[inline]
//             fn shl_assign(&mut self, other: $f) {
//                 *self = *self << other;
//             }
//         }
//         forward_ref_op_assign! { impl ShlAssign, shl_assign for Saturating<$t>, $f,
//         #[unstable(feature = "saturating_int_impl", issue = "87920")] }
//
//         #[unstable(feature = "saturating_int_impl", issue = "87920")]
//         impl Shr<$f> for Saturating<$t> {
//             type Output = Saturating<$t>;
//
//             #[inline]
//             fn shr(self, other: $f) -> Saturating<$t> {
//                 Saturating(self.0.wrapping_shr(other as u32))
//             }
//         }
//         forward_ref_binop! { impl Shr, shr for Saturating<$t>, $f,
//         #[unstable(feature = "saturating_int_impl", issue = "87920")] }
//
//         #[unstable(feature = "saturating_int_impl", issue = "87920")]
//         impl ShrAssign<$f> for Saturating<$t> {
//             #[inline]
//             fn shr_assign(&mut self, other: $f) {
//                 *self = *self >> other;
//             }
//         }
//         forward_ref_op_assign! { impl ShrAssign, shr_assign for Saturating<$t>, $f,
//         #[unstable(feature = "saturating_int_impl", issue = "87920")] }
//     };
// }
//
// // FIXME (#23545): 取消注释剩余 impl。
// macro_rules! sh_impl_all {
//     ($($t:ident)*) => ($(
//         //sh_impl_unsigned! { $t, u8 }
//         //sh_impl_unsigned! { $t, u16 }
//         //sh_impl_unsigned! { $t, u32 }
//         //sh_impl_unsigned! { $t, u64 }
//         //sh_impl_unsigned! { $t, u128 }
//         sh_impl_unsigned! { $t, usize }
//
//         //sh_impl_signed! { $t, i8 }
//         //sh_impl_signed! { $t, i16 }
//         //sh_impl_signed! { $t, i32 }
//         //sh_impl_signed! { $t, i64 }
//         //sh_impl_signed! { $t, i128 }
//         //sh_impl_signed! { $t, isize }
//     )*)
// }
//
// sh_impl_all! { u8 u16 u32 u64 u128 usize i8 i16 i32 i64 i128 isize }

// FIXME(30524): 为 Saturating<T> 实现 Op<T> 和 OpAssign<T>。
macro_rules! saturating_impl {
    ($($t:ty)*) => ($(
        #[stable(feature = "saturating_int_impl", since = "1.74.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const Add for Saturating<$t> {
            type Output = Saturating<$t>;

            #[inline]
            fn add(self, other: Saturating<$t>) -> Saturating<$t> {
                Saturating(self.0.saturating_add(other.0))
            }
        }
        forward_ref_binop! { impl Add, add for Saturating<$t>, Saturating<$t>,
        #[stable(feature = "saturating_int_impl", since = "1.74.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }

        #[stable(feature = "saturating_int_impl", since = "1.74.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const AddAssign for Saturating<$t> {
            #[inline]
            fn add_assign(&mut self, other: Saturating<$t>) {
                *self = *self + other;
            }
        }
        forward_ref_op_assign! { impl AddAssign, add_assign for Saturating<$t>, Saturating<$t>,
        #[stable(feature = "saturating_int_impl", since = "1.74.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }

        #[stable(feature = "saturating_int_assign_impl", since = "1.74.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const AddAssign<$t> for Saturating<$t> {
            #[inline]
            fn add_assign(&mut self, other: $t) {
                *self = *self + Saturating(other);
            }
        }
        forward_ref_op_assign! { impl AddAssign, add_assign for Saturating<$t>, $t,
        #[stable(feature = "saturating_int_impl", since = "1.74.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }

        #[stable(feature = "saturating_int_impl", since = "1.74.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const Sub for Saturating<$t> {
            type Output = Saturating<$t>;

            #[inline]
            fn sub(self, other: Saturating<$t>) -> Saturating<$t> {
                Saturating(self.0.saturating_sub(other.0))
            }
        }
        forward_ref_binop! { impl Sub, sub for Saturating<$t>, Saturating<$t>,
        #[stable(feature = "saturating_int_impl", since = "1.74.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }

        #[stable(feature = "saturating_int_impl", since = "1.74.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const SubAssign for Saturating<$t> {
            #[inline]
            fn sub_assign(&mut self, other: Saturating<$t>) {
                *self = *self - other;
            }
        }
        forward_ref_op_assign! { impl SubAssign, sub_assign for Saturating<$t>, Saturating<$t>,
        #[stable(feature = "saturating_int_impl", since = "1.74.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }

        #[stable(feature = "saturating_int_assign_impl", since = "1.74.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const SubAssign<$t> for Saturating<$t> {
            #[inline]
            fn sub_assign(&mut self, other: $t) {
                *self = *self - Saturating(other);
            }
        }
        forward_ref_op_assign! { impl SubAssign, sub_assign for Saturating<$t>, $t,
        #[stable(feature = "saturating_int_impl", since = "1.74.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }

        #[stable(feature = "saturating_int_impl", since = "1.74.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const Mul for Saturating<$t> {
            type Output = Saturating<$t>;

            #[inline]
            fn mul(self, other: Saturating<$t>) -> Saturating<$t> {
                Saturating(self.0.saturating_mul(other.0))
            }
        }
        forward_ref_binop! { impl Mul, mul for Saturating<$t>, Saturating<$t>,
        #[stable(feature = "saturating_int_impl", since = "1.74.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }

        #[stable(feature = "saturating_int_impl", since = "1.74.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const MulAssign for Saturating<$t> {
            #[inline]
            fn mul_assign(&mut self, other: Saturating<$t>) {
                *self = *self * other;
            }
        }
        forward_ref_op_assign! { impl MulAssign, mul_assign for Saturating<$t>, Saturating<$t>,
        #[stable(feature = "saturating_int_impl", since = "1.74.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }

        #[stable(feature = "saturating_int_assign_impl", since = "1.74.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const MulAssign<$t> for Saturating<$t> {
            #[inline]
            fn mul_assign(&mut self, other: $t) {
                *self = *self * Saturating(other);
            }
        }
        forward_ref_op_assign! { impl MulAssign, mul_assign for Saturating<$t>, $t,
        #[stable(feature = "saturating_int_impl", since = "1.74.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }

        /// # 示例
        ///
        /// ```
        /// use std::num::Saturating;
        ///
        #[doc = concat!("assert_eq!(Saturating(2", stringify!($t), "), Saturating(5", stringify!($t), ") / Saturating(2));")]
        #[doc = concat!("assert_eq!(Saturating(", stringify!($t), "::MAX), Saturating(", stringify!($t), "::MAX) / Saturating(1));")]
        #[doc = concat!("assert_eq!(Saturating(", stringify!($t), "::MIN), Saturating(", stringify!($t), "::MIN) / Saturating(1));")]
        /// ```
        ///
        /// ```should_panic
        /// use std::num::Saturating;
        ///
        #[doc = concat!("let _ = Saturating(0", stringify!($t), ") / Saturating(0);")]
        /// ```
        #[stable(feature = "saturating_int_impl", since = "1.74.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const Div for Saturating<$t> {
            type Output = Saturating<$t>;

            #[inline]
            fn div(self, other: Saturating<$t>) -> Saturating<$t> {
                Saturating(self.0.saturating_div(other.0))
            }
        }
        forward_ref_binop! { impl Div, div for Saturating<$t>, Saturating<$t>,
        #[stable(feature = "saturating_int_impl", since = "1.74.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }

        #[stable(feature = "saturating_int_impl", since = "1.74.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const DivAssign for Saturating<$t> {
            #[inline]
            fn div_assign(&mut self, other: Saturating<$t>) {
                *self = *self / other;
            }
        }
        forward_ref_op_assign! { impl DivAssign, div_assign for Saturating<$t>, Saturating<$t>,
        #[stable(feature = "saturating_int_impl", since = "1.74.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }

        #[stable(feature = "saturating_int_assign_impl", since = "1.74.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const DivAssign<$t> for Saturating<$t> {
            #[inline]
            fn div_assign(&mut self, other: $t) {
                *self = *self / Saturating(other);
            }
        }
        forward_ref_op_assign! { impl DivAssign, div_assign for Saturating<$t>, $t,
        #[stable(feature = "saturating_int_impl", since = "1.74.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }

        #[stable(feature = "saturating_int_impl", since = "1.74.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const Rem for Saturating<$t> {
            type Output = Saturating<$t>;

            #[inline]
            fn rem(self, other: Saturating<$t>) -> Saturating<$t> {
                Saturating(self.0.rem(other.0))
            }
        }
        forward_ref_binop! { impl Rem, rem for Saturating<$t>, Saturating<$t>,
        #[stable(feature = "saturating_int_impl", since = "1.74.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }

        #[stable(feature = "saturating_int_impl", since = "1.74.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const RemAssign for Saturating<$t> {
            #[inline]
            fn rem_assign(&mut self, other: Saturating<$t>) {
                *self = *self % other;
            }
        }
        forward_ref_op_assign! { impl RemAssign, rem_assign for Saturating<$t>, Saturating<$t>,
        #[stable(feature = "saturating_int_impl", since = "1.74.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }

        #[stable(feature = "saturating_int_assign_impl", since = "1.74.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const RemAssign<$t> for Saturating<$t> {
            #[inline]
            fn rem_assign(&mut self, other: $t) {
                *self = *self % Saturating(other);
            }
        }
        forward_ref_op_assign! { impl RemAssign, rem_assign for Saturating<$t>, $t,
        #[stable(feature = "saturating_int_impl", since = "1.74.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }

        #[stable(feature = "saturating_int_impl", since = "1.74.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const Not for Saturating<$t> {
            type Output = Saturating<$t>;

            #[inline]
            fn not(self) -> Saturating<$t> {
                Saturating(!self.0)
            }
        }
        forward_ref_unop! { impl Not, not for Saturating<$t>,
        #[stable(feature = "saturating_int_impl", since = "1.74.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }

        #[stable(feature = "saturating_int_impl", since = "1.74.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const BitXor for Saturating<$t> {
            type Output = Saturating<$t>;

            #[inline]
            fn bitxor(self, other: Saturating<$t>) -> Saturating<$t> {
                Saturating(self.0 ^ other.0)
            }
        }
        forward_ref_binop! { impl BitXor, bitxor for Saturating<$t>, Saturating<$t>,
        #[stable(feature = "saturating_int_impl", since = "1.74.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }

        #[stable(feature = "saturating_int_impl", since = "1.74.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const BitXorAssign for Saturating<$t> {
            #[inline]
            fn bitxor_assign(&mut self, other: Saturating<$t>) {
                *self = *self ^ other;
            }
        }
        forward_ref_op_assign! { impl BitXorAssign, bitxor_assign for Saturating<$t>, Saturating<$t>,
        #[stable(feature = "saturating_int_impl", since = "1.74.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }

        #[stable(feature = "saturating_int_assign_impl", since = "1.74.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const BitXorAssign<$t> for Saturating<$t> {
            #[inline]
            fn bitxor_assign(&mut self, other: $t) {
                *self = *self ^ Saturating(other);
            }
        }
        forward_ref_op_assign! { impl BitXorAssign, bitxor_assign for Saturating<$t>, $t,
        #[stable(feature = "saturating_int_impl", since = "1.74.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }

        #[stable(feature = "saturating_int_impl", since = "1.74.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const BitOr for Saturating<$t> {
            type Output = Saturating<$t>;

            #[inline]
            fn bitor(self, other: Saturating<$t>) -> Saturating<$t> {
                Saturating(self.0 | other.0)
            }
        }
        forward_ref_binop! { impl BitOr, bitor for Saturating<$t>, Saturating<$t>,
        #[stable(feature = "saturating_int_impl", since = "1.74.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }

        #[stable(feature = "saturating_int_impl", since = "1.74.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const BitOrAssign for Saturating<$t> {
            #[inline]
            fn bitor_assign(&mut self, other: Saturating<$t>) {
                *self = *self | other;
            }
        }
        forward_ref_op_assign! { impl BitOrAssign, bitor_assign for Saturating<$t>, Saturating<$t>,
        #[stable(feature = "saturating_int_impl", since = "1.74.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }

        #[stable(feature = "saturating_int_assign_impl", since = "1.74.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const BitOrAssign<$t> for Saturating<$t> {
            #[inline]
            fn bitor_assign(&mut self, other: $t) {
                *self = *self | Saturating(other);
            }
        }
        forward_ref_op_assign! { impl BitOrAssign, bitor_assign for Saturating<$t>, $t,
        #[stable(feature = "saturating_int_impl", since = "1.74.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }

        #[stable(feature = "saturating_int_impl", since = "1.74.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const BitAnd for Saturating<$t> {
            type Output = Saturating<$t>;

            #[inline]
            fn bitand(self, other: Saturating<$t>) -> Saturating<$t> {
                Saturating(self.0 & other.0)
            }
        }
        forward_ref_binop! { impl BitAnd, bitand for Saturating<$t>, Saturating<$t>,
        #[stable(feature = "saturating_int_impl", since = "1.74.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }

        #[stable(feature = "saturating_int_impl", since = "1.74.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const BitAndAssign for Saturating<$t> {
            #[inline]
            fn bitand_assign(&mut self, other: Saturating<$t>) {
                *self = *self & other;
            }
        }
        forward_ref_op_assign! { impl BitAndAssign, bitand_assign for Saturating<$t>, Saturating<$t>,
        #[stable(feature = "saturating_int_impl", since = "1.74.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }

        #[stable(feature = "saturating_int_assign_impl", since = "1.74.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const BitAndAssign<$t> for Saturating<$t> {
            #[inline]
            fn bitand_assign(&mut self, other: $t) {
                *self = *self & Saturating(other);
            }
        }
        forward_ref_op_assign! { impl BitAndAssign, bitand_assign for Saturating<$t>, $t,
        #[stable(feature = "saturating_int_impl", since = "1.74.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }

    )*)
}

saturating_impl! { usize u8 u16 u32 u64 u128 isize i8 i16 i32 i64 i128 }

macro_rules! saturating_int_impl {
    ($($t:ty)*) => ($(
        impl Saturating<$t> {
            /// 返回该整数类型能够表示的最小值。
            ///
            /// # 示例
            ///
            /// ```
            /// use std::num::Saturating;
            ///
            #[doc = concat!("assert_eq!(<Saturating<", stringify!($t), ">>::MIN, Saturating(", stringify!($t), "::MIN));")]
            /// ```
            #[stable(feature = "saturating_int_impl", since = "1.74.0")]
            pub const MIN: Self = Self(<$t>::MIN);

            /// 返回该整数类型能够表示的最大值。
            ///
            /// # 示例
            ///
            /// ```
            /// use std::num::Saturating;
            ///
            #[doc = concat!("assert_eq!(<Saturating<", stringify!($t), ">>::MAX, Saturating(", stringify!($t), "::MAX));")]
            /// ```
            #[stable(feature = "saturating_int_impl", since = "1.74.0")]
            pub const MAX: Self = Self(<$t>::MAX);

            /// 返回该整数类型的位宽。
            ///
            /// # 示例
            ///
            /// ```
            /// use std::num::Saturating;
            ///
            #[doc = concat!("assert_eq!(<Saturating<", stringify!($t), ">>::BITS, ", stringify!($t), "::BITS);")]
            /// ```
            #[stable(feature = "saturating_int_impl", since = "1.74.0")]
            pub const BITS: u32 = <$t>::BITS;

            /// 返回 `self` 的二进制表示中 1 的数量。
            ///
            /// # 示例
            ///
            /// ```
            /// use std::num::Saturating;
            ///
            #[doc = concat!("let n = Saturating(0b01001100", stringify!($t), ");")]
            ///
            /// assert_eq!(n.count_ones(), 3);
            /// ```
            #[inline]
            #[doc(alias = "popcount")]
            #[doc(alias = "popcnt")]
            #[must_use = "this returns the result of the operation, \
                          without modifying the original"]
            #[rustc_const_stable(feature = "saturating_int_impl", since = "1.74.0")]
            #[stable(feature = "saturating_int_impl", since = "1.74.0")]
            pub const fn count_ones(self) -> u32 {
                self.0.count_ones()
            }

            /// 返回 `self` 的二进制表示中 0 的数量。
            ///
            /// # 示例
            ///
            /// ```
            /// use std::num::Saturating;
            ///
            #[doc = concat!("assert_eq!(Saturating(!0", stringify!($t), ").count_zeros(), 0);")]
            /// ```
            #[inline]
            #[must_use = "this returns the result of the operation, \
                          without modifying the original"]
            #[rustc_const_stable(feature = "saturating_int_impl", since = "1.74.0")]
            #[stable(feature = "saturating_int_impl", since = "1.74.0")]
            pub const fn count_zeros(self) -> u32 {
                self.0.count_zeros()
            }

            /// 返回 `self` 的二进制表示中尾随 0 的数量。
            ///
            /// # 示例
            ///
            /// ```
            /// use std::num::Saturating;
            ///
            #[doc = concat!("let n = Saturating(0b0101000", stringify!($t), ");")]
            ///
            /// assert_eq!(n.trailing_zeros(), 3);
            /// ```
            #[inline]
            #[must_use = "this returns the result of the operation, \
                          without modifying the original"]
            #[rustc_const_stable(feature = "saturating_int_impl", since = "1.74.0")]
            #[stable(feature = "saturating_int_impl", since = "1.74.0")]
            pub const fn trailing_zeros(self) -> u32 {
                self.0.trailing_zeros()
            }

            /// 将位模式向左旋转指定数量 `n`，并把被截掉的高位绕回结果的低位。
            ///
            /// 注意这不是 `<<` 移位运算符；旋转不会丢弃位，只会改变位的位置。
            ///
            /// # 示例
            ///
            /// ```
            /// use std::num::Saturating;
            ///
            /// let n: Saturating<i64> = Saturating(0x0123456789ABCDEF);
            /// let m: Saturating<i64> = Saturating(-0x76543210FEDCBA99);
            ///
            /// assert_eq!(n.rotate_left(32), m);
            /// ```
            #[inline]
            #[must_use = "this returns the result of the operation, \
                          without modifying the original"]
            #[rustc_const_stable(feature = "saturating_int_impl", since = "1.74.0")]
            #[stable(feature = "saturating_int_impl", since = "1.74.0")]
            pub const fn rotate_left(self, n: u32) -> Self {
                Saturating(self.0.rotate_left(n))
            }

            /// 将位模式向右旋转指定数量 `n`，并把被截掉的低位绕回结果的高位。
            ///
            /// 注意这不是 `>>` 移位运算符；旋转不会丢弃位，只会改变位的位置。
            ///
            /// # 示例
            ///
            /// ```
            /// use std::num::Saturating;
            ///
            /// let n: Saturating<i64> = Saturating(0x0123456789ABCDEF);
            /// let m: Saturating<i64> = Saturating(-0xFEDCBA987654322);
            ///
            /// assert_eq!(n.rotate_right(4), m);
            /// ```
            #[inline]
            #[must_use = "this returns the result of the operation, \
                          without modifying the original"]
            #[rustc_const_stable(feature = "saturating_int_impl", since = "1.74.0")]
            #[stable(feature = "saturating_int_impl", since = "1.74.0")]
            pub const fn rotate_right(self, n: u32) -> Self {
                Saturating(self.0.rotate_right(n))
            }

            /// 反转该整数的字节顺序。
            ///
            /// # 示例
            ///
            /// ```
            /// use std::num::Saturating;
            ///
            /// let n: Saturating<i16> = Saturating(0b0000000_01010101);
            /// assert_eq!(n, Saturating(85));
            ///
            /// let m = n.swap_bytes();
            ///
            /// assert_eq!(m, Saturating(0b01010101_00000000));
            /// assert_eq!(m, Saturating(21760));
            /// ```
            #[inline]
            #[must_use = "this returns the result of the operation, \
                          without modifying the original"]
            #[rustc_const_stable(feature = "saturating_int_impl", since = "1.74.0")]
            #[stable(feature = "saturating_int_impl", since = "1.74.0")]
            pub const fn swap_bytes(self) -> Self {
                Saturating(self.0.swap_bytes())
            }

            /// 反转该整数的位模式。
            ///
            /// # 示例
            ///
            /// 注意该示例在多个整数类型之间共享，因此使用 `i16`。
            ///
            /// ```
            /// use std::num::Saturating;
            ///
            /// let n = Saturating(0b0000000_01010101i16);
            /// assert_eq!(n, Saturating(85));
            ///
            /// let m = n.reverse_bits();
            ///
            /// assert_eq!(m.0 as u16, 0b10101010_00000000);
            /// assert_eq!(m, Saturating(-22016));
            /// ```
            #[inline]
            #[rustc_const_stable(feature = "saturating_int_impl", since = "1.74.0")]
            #[stable(feature = "saturating_int_impl", since = "1.74.0")]
            #[must_use = "this returns the result of the operation, \
                          without modifying the original"]
            pub const fn reverse_bits(self) -> Self {
                Saturating(self.0.reverse_bits())
            }

            /// 把一个 big endian 整数转换为目标平台字节序。
            ///
            /// 在 big endian 平台上这是 no-op；在 little endian 平台上会交换字节。
            ///
            /// # 示例
            ///
            /// ```
            /// use std::num::Saturating;
            ///
            #[doc = concat!("let n = Saturating(0x1A", stringify!($t), ");")]
            ///
            /// if cfg!(target_endian = "big") {
            #[doc = concat!("    assert_eq!(<Saturating<", stringify!($t), ">>::from_be(n), n)")]
            /// } else {
            #[doc = concat!("    assert_eq!(<Saturating<", stringify!($t), ">>::from_be(n), n.swap_bytes())")]
            /// }
            /// ```
            #[inline]
            #[must_use]
            #[rustc_const_stable(feature = "saturating_int_impl", since = "1.74.0")]
            #[stable(feature = "saturating_int_impl", since = "1.74.0")]
            pub const fn from_be(x: Self) -> Self {
                Saturating(<$t>::from_be(x.0))
            }

            /// 把一个 little endian 整数转换为目标平台字节序。
            ///
            /// 在 little endian 平台上这是 no-op；在 big endian 平台上会交换字节。
            ///
            /// # 示例
            ///
            /// ```
            /// use std::num::Saturating;
            ///
            #[doc = concat!("let n = Saturating(0x1A", stringify!($t), ");")]
            ///
            /// if cfg!(target_endian = "little") {
            #[doc = concat!("    assert_eq!(<Saturating<", stringify!($t), ">>::from_le(n), n)")]
            /// } else {
            #[doc = concat!("    assert_eq!(<Saturating<", stringify!($t), ">>::from_le(n), n.swap_bytes())")]
            /// }
            /// ```
            #[inline]
            #[must_use]
            #[rustc_const_stable(feature = "saturating_int_impl", since = "1.74.0")]
            #[stable(feature = "saturating_int_impl", since = "1.74.0")]
            pub const fn from_le(x: Self) -> Self {
                Saturating(<$t>::from_le(x.0))
            }

            /// 把 `self` 从目标平台字节序转换为 big endian。
            ///
            /// 在 big endian 平台上这是 no-op；在 little endian 平台上会交换字节。
            ///
            /// # 示例
            ///
            /// ```
            /// use std::num::Saturating;
            ///
            #[doc = concat!("let n = Saturating(0x1A", stringify!($t), ");")]
            ///
            /// if cfg!(target_endian = "big") {
            ///     assert_eq!(n.to_be(), n)
            /// } else {
            ///     assert_eq!(n.to_be(), n.swap_bytes())
            /// }
            /// ```
            #[inline]
            #[rustc_const_stable(feature = "saturating_int_impl", since = "1.74.0")]
            #[stable(feature = "saturating_int_impl", since = "1.74.0")]
            #[must_use = "this returns the result of the operation, \
                          without modifying the original"]
            pub const fn to_be(self) -> Self {
                Saturating(self.0.to_be())
            }

            /// 把 `self` 从目标平台字节序转换为 little endian。
            ///
            /// 在 little endian 平台上这是 no-op；在 big endian 平台上会交换字节。
            ///
            /// # 示例
            ///
            /// ```
            /// use std::num::Saturating;
            ///
            #[doc = concat!("let n = Saturating(0x1A", stringify!($t), ");")]
            ///
            /// if cfg!(target_endian = "little") {
            ///     assert_eq!(n.to_le(), n)
            /// } else {
            ///     assert_eq!(n.to_le(), n.swap_bytes())
            /// }
            /// ```
            #[inline]
            #[rustc_const_stable(feature = "saturating_int_impl", since = "1.74.0")]
            #[stable(feature = "saturating_int_impl", since = "1.74.0")]
            #[must_use = "this returns the result of the operation, \
                          without modifying the original"]
            pub const fn to_le(self) -> Self {
                Saturating(self.0.to_le())
            }

            /// 使用平方求幂计算 `self` 的 `exp` 次幂。
            ///
            /// # 示例
            ///
            /// ```
            /// use std::num::Saturating;
            ///
            #[doc = concat!("assert_eq!(Saturating(3", stringify!($t), ").pow(4), Saturating(81));")]
            /// ```
            ///
            /// 过大的结果会按 saturating 语义夹到边界：
            ///
            /// ```
            /// use std::num::Saturating;
            ///
            /// assert_eq!(Saturating(3i8).pow(5), Saturating(127));
            /// assert_eq!(Saturating(3i8).pow(6), Saturating(127));
            /// ```
            #[inline]
            #[rustc_const_stable(feature = "saturating_int_impl", since = "1.74.0")]
            #[stable(feature = "saturating_int_impl", since = "1.74.0")]
            #[must_use = "this returns the result of the operation, \
                          without modifying the original"]
            pub const fn pow(self, exp: u32) -> Self {
                Saturating(self.0.saturating_pow(exp))
            }
        }
    )*)
}

saturating_int_impl! { usize u8 u16 u32 u64 u128 isize i8 i16 i32 i64 i128 }

macro_rules! saturating_int_impl_signed {
    ($($t:ty)*) => ($(
        impl Saturating<$t> {
            /// 返回 `self` 的二进制表示中前导 0 的数量。
            ///
            /// # 示例
            ///
            /// ```
            /// use std::num::Saturating;
            ///
            #[doc = concat!("let n = Saturating(", stringify!($t), "::MAX >> 2);")]
            ///
            /// assert_eq!(n.leading_zeros(), 3);
            /// ```
            #[inline]
            #[rustc_const_stable(feature = "saturating_int_impl", since = "1.74.0")]
            #[stable(feature = "saturating_int_impl", since = "1.74.0")]
            #[must_use = "this returns the result of the operation, \
                          without modifying the original"]
            pub const fn leading_zeros(self) -> u32 {
                self.0.leading_zeros()
            }

            /// saturating 绝对值。
            ///
            /// 计算 `self.abs()`；若 `self == MIN`，数学结果无法由该有符号类型表示，
            /// 因此返回 `MAX` 而不是溢出。
            ///
            /// # 示例
            ///
            /// ```
            /// use std::num::Saturating;
            ///
            #[doc = concat!("assert_eq!(Saturating(100", stringify!($t), ").abs(), Saturating(100));")]
            #[doc = concat!("assert_eq!(Saturating(-100", stringify!($t), ").abs(), Saturating(100));")]
            #[doc = concat!("assert_eq!(Saturating(", stringify!($t), "::MIN).abs(), Saturating((", stringify!($t), "::MIN + 1).abs()));")]
            #[doc = concat!("assert_eq!(Saturating(", stringify!($t), "::MIN).abs(), Saturating(", stringify!($t), "::MIN.saturating_abs()));")]
            #[doc = concat!("assert_eq!(Saturating(", stringify!($t), "::MIN).abs(), Saturating(", stringify!($t), "::MAX));")]
            /// ```
            #[inline]
            #[rustc_const_stable(feature = "saturating_int_impl", since = "1.74.0")]
            #[stable(feature = "saturating_int_impl", since = "1.74.0")]
            #[must_use = "this returns the result of the operation, \
                          without modifying the original"]
            pub const fn abs(self) -> Saturating<$t> {
                Saturating(self.0.saturating_abs())
            }

            /// 返回表示 `self` 符号的数。
            ///
            ///  - 数值为零时返回 `0`
            ///  - 数值为正时返回 `1`
            ///  - 数值为负时返回 `-1`
            ///
            /// # 示例
            ///
            /// ```
            /// use std::num::Saturating;
            ///
            #[doc = concat!("assert_eq!(Saturating(10", stringify!($t), ").signum(), Saturating(1));")]
            #[doc = concat!("assert_eq!(Saturating(0", stringify!($t), ").signum(), Saturating(0));")]
            #[doc = concat!("assert_eq!(Saturating(-10", stringify!($t), ").signum(), Saturating(-1));")]
            /// ```
            #[inline]
            #[rustc_const_stable(feature = "saturating_int_impl", since = "1.74.0")]
            #[stable(feature = "saturating_int_impl", since = "1.74.0")]
            #[must_use = "this returns the result of the operation, \
                          without modifying the original"]
            pub const fn signum(self) -> Saturating<$t> {
                Saturating(self.0.signum())
            }

            /// 当 `self` 为正数时返回 `true`；为零或负数时返回 `false`。
            ///
            /// # 示例
            ///
            /// ```
            /// use std::num::Saturating;
            ///
            #[doc = concat!("assert!(Saturating(10", stringify!($t), ").is_positive());")]
            #[doc = concat!("assert!(!Saturating(-10", stringify!($t), ").is_positive());")]
            /// ```
            #[must_use]
            #[inline]
            #[rustc_const_stable(feature = "saturating_int_impl", since = "1.74.0")]
            #[stable(feature = "saturating_int_impl", since = "1.74.0")]
            pub const fn is_positive(self) -> bool {
                self.0.is_positive()
            }

            /// 当 `self` 为负数时返回 `true`；为零或正数时返回 `false`。
            ///
            /// # 示例
            ///
            /// ```
            /// use std::num::Saturating;
            ///
            #[doc = concat!("assert!(Saturating(-10", stringify!($t), ").is_negative());")]
            #[doc = concat!("assert!(!Saturating(10", stringify!($t), ").is_negative());")]
            /// ```
            #[must_use]
            #[inline]
            #[rustc_const_stable(feature = "saturating_int_impl", since = "1.74.0")]
            #[stable(feature = "saturating_int_impl", since = "1.74.0")]
            pub const fn is_negative(self) -> bool {
                self.0.is_negative()
            }
        }

        #[stable(feature = "saturating_int_impl", since = "1.74.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const Neg for Saturating<$t> {
            type Output = Self;
            #[inline]
            fn neg(self) -> Self {
                Saturating(self.0.saturating_neg())
            }
        }
        forward_ref_unop! { impl Neg, neg for Saturating<$t>,
        #[stable(feature = "saturating_int_impl", since = "1.74.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }
    )*)
}

saturating_int_impl_signed! { isize i8 i16 i32 i64 i128 }

macro_rules! saturating_int_impl_unsigned {
    ($($t:ty)*) => ($(
        impl Saturating<$t> {
            /// 返回 `self` 的二进制表示中前导 0 的数量。
            ///
            /// # 示例
            ///
            /// ```
            /// use std::num::Saturating;
            ///
            #[doc = concat!("let n = Saturating(", stringify!($t), "::MAX >> 2);")]
            ///
            /// assert_eq!(n.leading_zeros(), 2);
            /// ```
            #[inline]
            #[rustc_const_stable(feature = "saturating_int_impl", since = "1.74.0")]
            #[stable(feature = "saturating_int_impl", since = "1.74.0")]
            #[must_use = "this returns the result of the operation, \
                          without modifying the original"]
            pub const fn leading_zeros(self) -> u32 {
                self.0.leading_zeros()
            }

            /// 当且仅当存在某个 `k` 使 `self == 2^k` 时返回 `true`。
            ///
            /// # 示例
            ///
            /// ```
            /// use std::num::Saturating;
            ///
            #[doc = concat!("assert!(Saturating(16", stringify!($t), ").is_power_of_two());")]
            #[doc = concat!("assert!(!Saturating(10", stringify!($t), ").is_power_of_two());")]
            /// ```
            #[must_use]
            #[inline]
            #[rustc_const_stable(feature = "saturating_int_impl", since = "1.74.0")]
            #[stable(feature = "saturating_int_impl", since = "1.74.0")]
            pub const fn is_power_of_two(self) -> bool {
                self.0.is_power_of_two()
            }

        }
    )*)
}

saturating_int_impl_unsigned! { usize u8 u16 u32 u64 u128 }

// 与未来可能的 Shl 和 ShlAssign 实现有关。
//
// mod shift_max {
//     #![allow(non_upper_case_globals)]
//
//     #[cfg(target_pointer_width = "16")]
//     mod platform {
//         pub const usize: u32 = super::u16;
//         pub const isize: u32 = super::i16;
//     }
//
//     #[cfg(target_pointer_width = "32")]
//     mod platform {
//         pub const usize: u32 = super::u32;
//         pub const isize: u32 = super::i32;
//     }
//
//     #[cfg(target_pointer_width = "64")]
//     mod platform {
//         pub const usize: u32 = super::u64;
//         pub const isize: u32 = super::i64;
//     }
//
//     pub const i8: u32 = (1 << 3) - 1;
//     pub const i16: u32 = (1 << 4) - 1;
//     pub const i32: u32 = (1 << 5) - 1;
//     pub const i64: u32 = (1 << 6) - 1;
//     pub const i128: u32 = (1 << 7) - 1;
//     pub use self::platform::isize;
//
//     pub const u8: u32 = i8;
//     pub const u16: u32 = i16;
//     pub const u32: u32 = i32;
//     pub const u64: u32 = i64;
//     pub const u128: u32 = i128;
//     pub use self::platform::usize;
// }
