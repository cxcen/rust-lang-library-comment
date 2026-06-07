//! `Wrapping<T>` 的定义。
//!
//! `Wrapping` 把整数运算显式解释为模 `2^N` 算术。普通整数的 `+`、`-`、`*` 等操作在
//! debug 配置下可能检查溢出并 panic，而 `Wrapping<T>` 用类型告诉读者和编译器：这里的
//! 溢出是预期语义，截断后的低 N 位就是结果。

use crate::fmt;
use crate::ops::{
    Add, AddAssign, BitAnd, BitAndAssign, BitOr, BitOrAssign, BitXor, BitXorAssign, Div, DivAssign,
    Mul, MulAssign, Neg, Not, Rem, RemAssign, Shl, ShlAssign, Shr, ShrAssign, Sub, SubAssign,
};

/// 为 `T` 提供有意的 wrapping 算术。
///
/// 对 `u32` 这样的普通整数执行 `+` 时，通常认为溢出不是业务语义；某些 debug 配置会检测
/// 溢出并 panic。但有些代码明确需要并依赖模算术，例如哈希、随机数、密码学前置处理或
/// 位混合算法。
///
/// wrapping 算术既可以通过 `wrapping_add` 这类方法逐次表达，也可以通过 `Wrapping<T>`
/// 类型表达。后者表示底层值上的标准算术操作整体都采用 wrapping 语义。
///
/// 可以通过 `Wrapping` 元组的 `.0` 字段取回底层值。
///
/// # 示例
///
/// ```
/// use std::num::Wrapping;
///
/// let zero = Wrapping(0u32);
/// let one = Wrapping(1u32);
///
/// assert_eq!(u32::MAX, (zero - one).0);
/// ```
///
/// # 布局(Layout)
///
/// `Wrapping<T>` 保证与 `T` 具有相同布局和 ABI。
#[stable(feature = "rust1", since = "1.0.0")]
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Default, Hash)]
#[repr(transparent)]
#[rustc_diagnostic_item = "Wrapping"]
pub struct Wrapping<T>(#[stable(feature = "rust1", since = "1.0.0")] pub T);

#[stable(feature = "rust1", since = "1.0.0")]
impl<T: fmt::Debug> fmt::Debug for Wrapping<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[stable(feature = "wrapping_display", since = "1.10.0")]
impl<T: fmt::Display> fmt::Display for Wrapping<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[stable(feature = "wrapping_fmt", since = "1.11.0")]
impl<T: fmt::Binary> fmt::Binary for Wrapping<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[stable(feature = "wrapping_fmt", since = "1.11.0")]
impl<T: fmt::Octal> fmt::Octal for Wrapping<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[stable(feature = "wrapping_fmt", since = "1.11.0")]
impl<T: fmt::LowerHex> fmt::LowerHex for Wrapping<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[stable(feature = "wrapping_fmt", since = "1.11.0")]
impl<T: fmt::UpperHex> fmt::UpperHex for Wrapping<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[allow(unused_macros)]
macro_rules! sh_impl_signed {
    ($t:ident, $f:ident) => {
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const Shl<$f> for Wrapping<$t> {
            type Output = Wrapping<$t>;

            #[inline]
            fn shl(self, other: $f) -> Wrapping<$t> {
                if other < 0 {
                    Wrapping(self.0.wrapping_shr(-other as u32))
                } else {
                    Wrapping(self.0.wrapping_shl(other as u32))
                }
            }
        }
        forward_ref_binop! { impl Shl, shl for Wrapping<$t>, $f,
        #[stable(feature = "wrapping_ref_ops", since = "1.39.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }

        #[stable(feature = "op_assign_traits", since = "1.8.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const ShlAssign<$f> for Wrapping<$t> {
            #[inline]
            fn shl_assign(&mut self, other: $f) {
                *self = *self << other;
            }
        }
        forward_ref_op_assign! { impl ShlAssign, shl_assign for Wrapping<$t>, $f,
        #[stable(feature = "op_assign_builtins_by_ref", since = "1.22.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }

        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const Shr<$f> for Wrapping<$t> {
            type Output = Wrapping<$t>;

            #[inline]
            fn shr(self, other: $f) -> Wrapping<$t> {
                if other < 0 {
                    Wrapping(self.0.wrapping_shl(-other as u32))
                } else {
                    Wrapping(self.0.wrapping_shr(other as u32))
                }
            }
        }
        forward_ref_binop! { impl Shr, shr for Wrapping<$t>, $f,
        #[stable(feature = "wrapping_ref_ops", since = "1.39.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }

        #[stable(feature = "op_assign_traits", since = "1.8.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const ShrAssign<$f> for Wrapping<$t> {
            #[inline]
            fn shr_assign(&mut self, other: $f) {
                *self = *self >> other;
            }
        }
        forward_ref_op_assign! { impl ShrAssign, shr_assign for Wrapping<$t>, $f,
        #[stable(feature = "op_assign_builtins_by_ref", since = "1.22.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }
    };
}

macro_rules! sh_impl_unsigned {
    ($t:ident, $f:ident) => {
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const Shl<$f> for Wrapping<$t> {
            type Output = Wrapping<$t>;

            #[inline]
            fn shl(self, other: $f) -> Wrapping<$t> {
                Wrapping(self.0.wrapping_shl(other as u32))
            }
        }
        forward_ref_binop! { impl Shl, shl for Wrapping<$t>, $f,
        #[stable(feature = "wrapping_ref_ops", since = "1.39.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }

        #[stable(feature = "op_assign_traits", since = "1.8.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const ShlAssign<$f> for Wrapping<$t> {
            #[inline]
            fn shl_assign(&mut self, other: $f) {
                *self = *self << other;
            }
        }
        forward_ref_op_assign! { impl ShlAssign, shl_assign for Wrapping<$t>, $f,
        #[stable(feature = "op_assign_builtins_by_ref", since = "1.22.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }

        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const Shr<$f> for Wrapping<$t> {
            type Output = Wrapping<$t>;

            #[inline]
            fn shr(self, other: $f) -> Wrapping<$t> {
                Wrapping(self.0.wrapping_shr(other as u32))
            }
        }
        forward_ref_binop! { impl Shr, shr for Wrapping<$t>, $f,
        #[stable(feature = "wrapping_ref_ops", since = "1.39.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }

        #[stable(feature = "op_assign_traits", since = "1.8.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const ShrAssign<$f> for Wrapping<$t> {
            #[inline]
            fn shr_assign(&mut self, other: $f) {
                *self = *self >> other;
            }
        }
        forward_ref_op_assign! { impl ShrAssign, shr_assign for Wrapping<$t>, $f,
        #[stable(feature = "op_assign_builtins_by_ref", since = "1.22.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }
    };
}

// FIXME (#23545): 取消注释剩余 impl。
macro_rules! sh_impl_all {
    ($($t:ident)*) => ($(
        //sh_impl_unsigned! { $t, u8 }
        //sh_impl_unsigned! { $t, u16 }
        //sh_impl_unsigned! { $t, u32 }
        //sh_impl_unsigned! { $t, u64 }
        //sh_impl_unsigned! { $t, u128 }
        sh_impl_unsigned! { $t, usize }

        //sh_impl_signed! { $t, i8 }
        //sh_impl_signed! { $t, i16 }
        //sh_impl_signed! { $t, i32 }
        //sh_impl_signed! { $t, i64 }
        //sh_impl_signed! { $t, i128 }
        //sh_impl_signed! { $t, isize }
    )*)
}

sh_impl_all! { u8 u16 u32 u64 u128 usize i8 i16 i32 i64 i128 isize }

// FIXME(30524): 为 Wrapping<T> 实现 Op<T> 和 OpAssign<T>。
macro_rules! wrapping_impl {
    ($($t:ty)*) => ($(
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const Add for Wrapping<$t> {
            type Output = Wrapping<$t>;

            #[inline]
            fn add(self, other: Wrapping<$t>) -> Wrapping<$t> {
                Wrapping(self.0.wrapping_add(other.0))
            }
        }
        forward_ref_binop! { impl Add, add for Wrapping<$t>, Wrapping<$t>,
        #[stable(feature = "wrapping_ref", since = "1.14.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }

        #[stable(feature = "op_assign_traits", since = "1.8.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const AddAssign for Wrapping<$t> {
            #[inline]
            fn add_assign(&mut self, other: Wrapping<$t>) {
                *self = *self + other;
            }
        }
        forward_ref_op_assign! { impl AddAssign, add_assign for Wrapping<$t>, Wrapping<$t>,
        #[stable(feature = "op_assign_builtins_by_ref", since = "1.22.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }

        #[stable(feature = "wrapping_int_assign_impl", since = "1.60.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const AddAssign<$t> for Wrapping<$t> {
            #[inline]
            fn add_assign(&mut self, other: $t) {
                *self = *self + Wrapping(other);
            }
        }
        forward_ref_op_assign! { impl AddAssign, add_assign for Wrapping<$t>, $t,
        #[stable(feature = "op_assign_builtins_by_ref", since = "1.22.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }

        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const Sub for Wrapping<$t> {
            type Output = Wrapping<$t>;

            #[inline]
            fn sub(self, other: Wrapping<$t>) -> Wrapping<$t> {
                Wrapping(self.0.wrapping_sub(other.0))
            }
        }
        forward_ref_binop! { impl Sub, sub for Wrapping<$t>, Wrapping<$t>,
        #[stable(feature = "wrapping_ref", since = "1.14.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }

        #[stable(feature = "op_assign_traits", since = "1.8.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const SubAssign for Wrapping<$t> {
            #[inline]
            fn sub_assign(&mut self, other: Wrapping<$t>) {
                *self = *self - other;
            }
        }
        forward_ref_op_assign! { impl SubAssign, sub_assign for Wrapping<$t>, Wrapping<$t>,
        #[stable(feature = "op_assign_builtins_by_ref", since = "1.22.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }

        #[stable(feature = "wrapping_int_assign_impl", since = "1.60.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const SubAssign<$t> for Wrapping<$t> {
            #[inline]
            fn sub_assign(&mut self, other: $t) {
                *self = *self - Wrapping(other);
            }
        }
        forward_ref_op_assign! { impl SubAssign, sub_assign for Wrapping<$t>, $t,
        #[stable(feature = "op_assign_builtins_by_ref", since = "1.22.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }

        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const Mul for Wrapping<$t> {
            type Output = Wrapping<$t>;

            #[inline]
            fn mul(self, other: Wrapping<$t>) -> Wrapping<$t> {
                Wrapping(self.0.wrapping_mul(other.0))
            }
        }
        forward_ref_binop! { impl Mul, mul for Wrapping<$t>, Wrapping<$t>,
        #[stable(feature = "wrapping_ref", since = "1.14.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }

        #[stable(feature = "op_assign_traits", since = "1.8.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const MulAssign for Wrapping<$t> {
            #[inline]
            fn mul_assign(&mut self, other: Wrapping<$t>) {
                *self = *self * other;
            }
        }
        forward_ref_op_assign! { impl MulAssign, mul_assign for Wrapping<$t>, Wrapping<$t>,
        #[stable(feature = "op_assign_builtins_by_ref", since = "1.22.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }

        #[stable(feature = "wrapping_int_assign_impl", since = "1.60.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const MulAssign<$t> for Wrapping<$t> {
            #[inline]
            fn mul_assign(&mut self, other: $t) {
                *self = *self * Wrapping(other);
            }
        }
        forward_ref_op_assign! { impl MulAssign, mul_assign for Wrapping<$t>, $t,
        #[stable(feature = "op_assign_builtins_by_ref", since = "1.22.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }

        #[stable(feature = "wrapping_div", since = "1.3.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const Div for Wrapping<$t> {
            type Output = Wrapping<$t>;

            #[inline]
            fn div(self, other: Wrapping<$t>) -> Wrapping<$t> {
                Wrapping(self.0.wrapping_div(other.0))
            }
        }
        forward_ref_binop! { impl Div, div for Wrapping<$t>, Wrapping<$t>,
        #[stable(feature = "wrapping_ref", since = "1.14.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }

        #[stable(feature = "op_assign_traits", since = "1.8.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const DivAssign for Wrapping<$t> {
            #[inline]
            fn div_assign(&mut self, other: Wrapping<$t>) {
                *self = *self / other;
            }
        }
        forward_ref_op_assign! { impl DivAssign, div_assign for Wrapping<$t>, Wrapping<$t>,
        #[stable(feature = "op_assign_builtins_by_ref", since = "1.22.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }

        #[stable(feature = "wrapping_int_assign_impl", since = "1.60.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const DivAssign<$t> for Wrapping<$t> {
            #[inline]
            fn div_assign(&mut self, other: $t) {
                *self = *self / Wrapping(other);
            }
        }
        forward_ref_op_assign! { impl DivAssign, div_assign for Wrapping<$t>, $t,
        #[stable(feature = "op_assign_builtins_by_ref", since = "1.22.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }

        #[stable(feature = "wrapping_impls", since = "1.7.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const Rem for Wrapping<$t> {
            type Output = Wrapping<$t>;

            #[inline]
            fn rem(self, other: Wrapping<$t>) -> Wrapping<$t> {
                Wrapping(self.0.wrapping_rem(other.0))
            }
        }
        forward_ref_binop! { impl Rem, rem for Wrapping<$t>, Wrapping<$t>,
        #[stable(feature = "wrapping_ref", since = "1.14.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }

        #[stable(feature = "op_assign_traits", since = "1.8.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const RemAssign for Wrapping<$t> {
            #[inline]
            fn rem_assign(&mut self, other: Wrapping<$t>) {
                *self = *self % other;
            }
        }
        forward_ref_op_assign! { impl RemAssign, rem_assign for Wrapping<$t>, Wrapping<$t>,
        #[stable(feature = "op_assign_builtins_by_ref", since = "1.22.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }

        #[stable(feature = "wrapping_int_assign_impl", since = "1.60.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const RemAssign<$t> for Wrapping<$t> {
            #[inline]
            fn rem_assign(&mut self, other: $t) {
                *self = *self % Wrapping(other);
            }
        }
        forward_ref_op_assign! { impl RemAssign, rem_assign for Wrapping<$t>, $t,
        #[stable(feature = "op_assign_builtins_by_ref", since = "1.22.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }

        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const Not for Wrapping<$t> {
            type Output = Wrapping<$t>;

            #[inline]
            fn not(self) -> Wrapping<$t> {
                Wrapping(!self.0)
            }
        }
        forward_ref_unop! { impl Not, not for Wrapping<$t>,
        #[stable(feature = "wrapping_ref", since = "1.14.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }

        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const BitXor for Wrapping<$t> {
            type Output = Wrapping<$t>;

            #[inline]
            fn bitxor(self, other: Wrapping<$t>) -> Wrapping<$t> {
                Wrapping(self.0 ^ other.0)
            }
        }
        forward_ref_binop! { impl BitXor, bitxor for Wrapping<$t>, Wrapping<$t>,
        #[stable(feature = "wrapping_ref", since = "1.14.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }

        #[stable(feature = "op_assign_traits", since = "1.8.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const BitXorAssign for Wrapping<$t> {
            #[inline]
            fn bitxor_assign(&mut self, other: Wrapping<$t>) {
                *self = *self ^ other;
            }
        }
        forward_ref_op_assign! { impl BitXorAssign, bitxor_assign for Wrapping<$t>, Wrapping<$t>,
        #[stable(feature = "op_assign_builtins_by_ref", since = "1.22.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }

        #[stable(feature = "wrapping_int_assign_impl", since = "1.60.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const BitXorAssign<$t> for Wrapping<$t> {
            #[inline]
            fn bitxor_assign(&mut self, other: $t) {
                *self = *self ^ Wrapping(other);
            }
        }
        forward_ref_op_assign! { impl BitXorAssign, bitxor_assign for Wrapping<$t>, $t,
        #[stable(feature = "op_assign_builtins_by_ref", since = "1.22.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }

        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const BitOr for Wrapping<$t> {
            type Output = Wrapping<$t>;

            #[inline]
            fn bitor(self, other: Wrapping<$t>) -> Wrapping<$t> {
                Wrapping(self.0 | other.0)
            }
        }
        forward_ref_binop! { impl BitOr, bitor for Wrapping<$t>, Wrapping<$t>,
        #[stable(feature = "wrapping_ref", since = "1.14.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }

        #[stable(feature = "op_assign_traits", since = "1.8.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const BitOrAssign for Wrapping<$t> {
            #[inline]
            fn bitor_assign(&mut self, other: Wrapping<$t>) {
                *self = *self | other;
            }
        }
        forward_ref_op_assign! { impl BitOrAssign, bitor_assign for Wrapping<$t>, Wrapping<$t>,
        #[stable(feature = "op_assign_builtins_by_ref", since = "1.22.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }

        #[stable(feature = "wrapping_int_assign_impl", since = "1.60.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const BitOrAssign<$t> for Wrapping<$t> {
            #[inline]
            fn bitor_assign(&mut self, other: $t) {
                *self = *self | Wrapping(other);
            }
        }
        forward_ref_op_assign! { impl BitOrAssign, bitor_assign for Wrapping<$t>, $t,
        #[stable(feature = "op_assign_builtins_by_ref", since = "1.22.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }

        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const BitAnd for Wrapping<$t> {
            type Output = Wrapping<$t>;

            #[inline]
            fn bitand(self, other: Wrapping<$t>) -> Wrapping<$t> {
                Wrapping(self.0 & other.0)
            }
        }
        forward_ref_binop! { impl BitAnd, bitand for Wrapping<$t>, Wrapping<$t>,
        #[stable(feature = "wrapping_ref", since = "1.14.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }

        #[stable(feature = "op_assign_traits", since = "1.8.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const BitAndAssign for Wrapping<$t> {
            #[inline]
            fn bitand_assign(&mut self, other: Wrapping<$t>) {
                *self = *self & other;
            }
        }
        forward_ref_op_assign! { impl BitAndAssign, bitand_assign for Wrapping<$t>, Wrapping<$t>,
        #[stable(feature = "op_assign_builtins_by_ref", since = "1.22.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }

        #[stable(feature = "wrapping_int_assign_impl", since = "1.60.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const BitAndAssign<$t> for Wrapping<$t> {
            #[inline]
            fn bitand_assign(&mut self, other: $t) {
                *self = *self & Wrapping(other);
            }
        }
        forward_ref_op_assign! { impl BitAndAssign, bitand_assign for Wrapping<$t>, $t,
        #[stable(feature = "op_assign_builtins_by_ref", since = "1.22.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }

        #[stable(feature = "wrapping_neg", since = "1.10.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")]
        impl const Neg for Wrapping<$t> {
            type Output = Self;
            #[inline]
            fn neg(self) -> Self {
                Wrapping(0) - self
            }
        }
        forward_ref_unop! { impl Neg, neg for Wrapping<$t>,
        #[stable(feature = "wrapping_ref", since = "1.14.0")]
        #[rustc_const_unstable(feature = "const_ops", issue = "143802")] }

    )*)
}

wrapping_impl! { usize u8 u16 u32 u64 u128 isize i8 i16 i32 i64 i128 }

macro_rules! wrapping_int_impl {
    ($($t:ty)*) => ($(
        impl Wrapping<$t> {
            /// 返回该整数类型能够表示的最小值。
            ///
            /// # 示例
            ///
            /// 基本用法：
            ///
            /// ```
            /// #![feature(wrapping_int_impl)]
            /// use std::num::Wrapping;
            ///
            #[doc = concat!("assert_eq!(<Wrapping<", stringify!($t), ">>::MIN, Wrapping(", stringify!($t), "::MIN));")]
            /// ```
            #[unstable(feature = "wrapping_int_impl", issue = "32463")]
            pub const MIN: Self = Self(<$t>::MIN);

            /// 返回该整数类型能够表示的最大值。
            ///
            /// # 示例
            ///
            /// 基本用法：
            ///
            /// ```
            /// #![feature(wrapping_int_impl)]
            /// use std::num::Wrapping;
            ///
            #[doc = concat!("assert_eq!(<Wrapping<", stringify!($t), ">>::MAX, Wrapping(", stringify!($t), "::MAX));")]
            /// ```
            #[unstable(feature = "wrapping_int_impl", issue = "32463")]
            pub const MAX: Self = Self(<$t>::MAX);

            /// 返回该整数类型的位宽。
            ///
            /// # 示例
            ///
            /// 基本用法：
            ///
            /// ```
            /// #![feature(wrapping_int_impl)]
            /// use std::num::Wrapping;
            ///
            #[doc = concat!("assert_eq!(<Wrapping<", stringify!($t), ">>::BITS, ", stringify!($t), "::BITS);")]
            /// ```
            #[unstable(feature = "wrapping_int_impl", issue = "32463")]
            pub const BITS: u32 = <$t>::BITS;

            /// 返回 `self` 的二进制表示中 1 的数量。
            ///
            /// # 示例
            ///
            /// 基本用法：
            ///
            /// ```
            /// #![feature(wrapping_int_impl)]
            /// use std::num::Wrapping;
            ///
            #[doc = concat!("let n = Wrapping(0b01001100", stringify!($t), ");")]
            ///
            /// assert_eq!(n.count_ones(), 3);
            /// ```
            #[inline]
            #[doc(alias = "popcount")]
            #[doc(alias = "popcnt")]
            #[must_use = "this returns the result of the operation, \
                          without modifying the original"]
            #[unstable(feature = "wrapping_int_impl", issue = "32463")]
            pub const fn count_ones(self) -> u32 {
                self.0.count_ones()
            }

            /// 返回 `self` 的二进制表示中 0 的数量。
            ///
            /// # 示例
            ///
            /// 基本用法：
            ///
            /// ```
            /// #![feature(wrapping_int_impl)]
            /// use std::num::Wrapping;
            ///
            #[doc = concat!("assert_eq!(Wrapping(!0", stringify!($t), ").count_zeros(), 0);")]
            /// ```
            #[inline]
            #[must_use = "this returns the result of the operation, \
                          without modifying the original"]
            #[unstable(feature = "wrapping_int_impl", issue = "32463")]
            pub const fn count_zeros(self) -> u32 {
                self.0.count_zeros()
            }

            /// 返回 `self` 的二进制表示中尾随 0 的数量。
            ///
            /// # 示例
            ///
            /// 基本用法：
            ///
            /// ```
            /// #![feature(wrapping_int_impl)]
            /// use std::num::Wrapping;
            ///
            #[doc = concat!("let n = Wrapping(0b0101000", stringify!($t), ");")]
            ///
            /// assert_eq!(n.trailing_zeros(), 3);
            /// ```
            #[inline]
            #[must_use = "this returns the result of the operation, \
                          without modifying the original"]
            #[unstable(feature = "wrapping_int_impl", issue = "32463")]
            pub const fn trailing_zeros(self) -> u32 {
                self.0.trailing_zeros()
            }

            /// 将位模式向左旋转指定数量 `n`，并把被截掉的高位绕回结果的低位。
            ///
            /// 注意这不是 `<<` 移位运算符；旋转不会丢弃位，只会改变位的位置。
            ///
            /// # 示例
            ///
            /// 基本用法：
            ///
            /// ```
            /// #![feature(wrapping_int_impl)]
            /// use std::num::Wrapping;
            ///
            /// let n: Wrapping<i64> = Wrapping(0x0123456789ABCDEF);
            /// let m: Wrapping<i64> = Wrapping(-0x76543210FEDCBA99);
            ///
            /// assert_eq!(n.rotate_left(32), m);
            /// ```
            #[inline]
            #[must_use = "this returns the result of the operation, \
                          without modifying the original"]
            #[unstable(feature = "wrapping_int_impl", issue = "32463")]
            pub const fn rotate_left(self, n: u32) -> Self {
                Wrapping(self.0.rotate_left(n))
            }

            /// 将位模式向右旋转指定数量 `n`，并把被截掉的低位绕回结果的高位。
            ///
            /// 注意这不是 `>>` 移位运算符；旋转不会丢弃位，只会改变位的位置。
            ///
            /// # 示例
            ///
            /// 基本用法：
            ///
            /// ```
            /// #![feature(wrapping_int_impl)]
            /// use std::num::Wrapping;
            ///
            /// let n: Wrapping<i64> = Wrapping(0x0123456789ABCDEF);
            /// let m: Wrapping<i64> = Wrapping(-0xFEDCBA987654322);
            ///
            /// assert_eq!(n.rotate_right(4), m);
            /// ```
            #[inline]
            #[must_use = "this returns the result of the operation, \
                          without modifying the original"]
            #[unstable(feature = "wrapping_int_impl", issue = "32463")]
            pub const fn rotate_right(self, n: u32) -> Self {
                Wrapping(self.0.rotate_right(n))
            }

            /// 反转该整数的字节顺序。
            ///
            /// # 示例
            ///
            /// 基本用法：
            ///
            /// ```
            /// #![feature(wrapping_int_impl)]
            /// use std::num::Wrapping;
            ///
            /// let n: Wrapping<i16> = Wrapping(0b0000000_01010101);
            /// assert_eq!(n, Wrapping(85));
            ///
            /// let m = n.swap_bytes();
            ///
            /// assert_eq!(m, Wrapping(0b01010101_00000000));
            /// assert_eq!(m, Wrapping(21760));
            /// ```
            #[inline]
            #[must_use = "this returns the result of the operation, \
                          without modifying the original"]
            #[unstable(feature = "wrapping_int_impl", issue = "32463")]
            pub const fn swap_bytes(self) -> Self {
                Wrapping(self.0.swap_bytes())
            }

            /// 反转该整数的位模式。
            ///
            /// # 示例
            ///
            /// 注意该示例在多个整数类型之间共享，因此使用 `i16`。
            ///
            /// 基本用法：
            ///
            /// ```
            /// use std::num::Wrapping;
            ///
            /// let n = Wrapping(0b0000000_01010101i16);
            /// assert_eq!(n, Wrapping(85));
            ///
            /// let m = n.reverse_bits();
            ///
            /// assert_eq!(m.0 as u16, 0b10101010_00000000);
            /// assert_eq!(m, Wrapping(-22016));
            /// ```
            #[stable(feature = "reverse_bits", since = "1.37.0")]
            #[rustc_const_stable(feature = "const_reverse_bits", since = "1.37.0")]
            #[must_use = "this returns the result of the operation, \
                          without modifying the original"]
            #[inline]
            pub const fn reverse_bits(self) -> Self {
                Wrapping(self.0.reverse_bits())
            }

            /// 把一个 big endian 整数转换为目标平台字节序。
            ///
            /// 在 big endian 平台上这是 no-op；在 little endian 平台上会交换字节。
            ///
            /// # 示例
            ///
            /// 基本用法：
            ///
            /// ```
            /// #![feature(wrapping_int_impl)]
            /// use std::num::Wrapping;
            ///
            #[doc = concat!("let n = Wrapping(0x1A", stringify!($t), ");")]
            ///
            /// if cfg!(target_endian = "big") {
            #[doc = concat!("    assert_eq!(<Wrapping<", stringify!($t), ">>::from_be(n), n)")]
            /// } else {
            #[doc = concat!("    assert_eq!(<Wrapping<", stringify!($t), ">>::from_be(n), n.swap_bytes())")]
            /// }
            /// ```
            #[inline]
            #[must_use]
            #[unstable(feature = "wrapping_int_impl", issue = "32463")]
            pub const fn from_be(x: Self) -> Self {
                Wrapping(<$t>::from_be(x.0))
            }

            /// 把一个 little endian 整数转换为目标平台字节序。
            ///
            /// 在 little endian 平台上这是 no-op；在 big endian 平台上会交换字节。
            ///
            /// # 示例
            ///
            /// 基本用法：
            ///
            /// ```
            /// #![feature(wrapping_int_impl)]
            /// use std::num::Wrapping;
            ///
            #[doc = concat!("let n = Wrapping(0x1A", stringify!($t), ");")]
            ///
            /// if cfg!(target_endian = "little") {
            #[doc = concat!("    assert_eq!(<Wrapping<", stringify!($t), ">>::from_le(n), n)")]
            /// } else {
            #[doc = concat!("    assert_eq!(<Wrapping<", stringify!($t), ">>::from_le(n), n.swap_bytes())")]
            /// }
            /// ```
            #[inline]
            #[must_use]
            #[unstable(feature = "wrapping_int_impl", issue = "32463")]
            pub const fn from_le(x: Self) -> Self {
                Wrapping(<$t>::from_le(x.0))
            }

            /// 把 `self` 从目标平台字节序转换为 big endian。
            ///
            /// 在 big endian 平台上这是 no-op；在 little endian 平台上会交换字节。
            ///
            /// # 示例
            ///
            /// 基本用法：
            ///
            /// ```
            /// #![feature(wrapping_int_impl)]
            /// use std::num::Wrapping;
            ///
            #[doc = concat!("let n = Wrapping(0x1A", stringify!($t), ");")]
            ///
            /// if cfg!(target_endian = "big") {
            ///     assert_eq!(n.to_be(), n)
            /// } else {
            ///     assert_eq!(n.to_be(), n.swap_bytes())
            /// }
            /// ```
            #[inline]
            #[must_use = "this returns the result of the operation, \
                          without modifying the original"]
            #[unstable(feature = "wrapping_int_impl", issue = "32463")]
            pub const fn to_be(self) -> Self {
                Wrapping(self.0.to_be())
            }

            /// 把 `self` 从目标平台字节序转换为 little endian。
            ///
            /// 在 little endian 平台上这是 no-op；在 big endian 平台上会交换字节。
            ///
            /// # 示例
            ///
            /// 基本用法：
            ///
            /// ```
            /// #![feature(wrapping_int_impl)]
            /// use std::num::Wrapping;
            ///
            #[doc = concat!("let n = Wrapping(0x1A", stringify!($t), ");")]
            ///
            /// if cfg!(target_endian = "little") {
            ///     assert_eq!(n.to_le(), n)
            /// } else {
            ///     assert_eq!(n.to_le(), n.swap_bytes())
            /// }
            /// ```
            #[inline]
            #[must_use = "this returns the result of the operation, \
                          without modifying the original"]
            #[unstable(feature = "wrapping_int_impl", issue = "32463")]
            pub const fn to_le(self) -> Self {
                Wrapping(self.0.to_le())
            }

            /// 使用平方求幂计算 `self` 的 `exp` 次幂。
            ///
            /// # 示例
            ///
            /// 基本用法：
            ///
            /// ```
            /// #![feature(wrapping_int_impl)]
            /// use std::num::Wrapping;
            ///
            #[doc = concat!("assert_eq!(Wrapping(3", stringify!($t), ").pow(4), Wrapping(81));")]
            /// ```
            ///
            /// 过大的结果会按 wrapping 语义回绕：
            ///
            /// ```
            /// #![feature(wrapping_int_impl)]
            /// use std::num::Wrapping;
            ///
            /// assert_eq!(Wrapping(3i8).pow(5), Wrapping(-13));
            /// assert_eq!(Wrapping(3i8).pow(6), Wrapping(-39));
            /// ```
            #[inline]
            #[must_use = "this returns the result of the operation, \
                          without modifying the original"]
            #[unstable(feature = "wrapping_int_impl", issue = "32463")]
            pub fn pow(self, exp: u32) -> Self {
                Wrapping(self.0.wrapping_pow(exp))
            }
        }
    )*)
}

wrapping_int_impl! { usize u8 u16 u32 u64 u128 isize i8 i16 i32 i64 i128 }

macro_rules! wrapping_int_impl_signed {
    ($($t:ty)*) => ($(
        impl Wrapping<$t> {
            /// 返回 `self` 的二进制表示中前导 0 的数量。
            ///
            /// # 示例
            ///
            /// 基本用法：
            ///
            /// ```
            /// #![feature(wrapping_int_impl)]
            /// use std::num::Wrapping;
            ///
            #[doc = concat!("let n = Wrapping(", stringify!($t), "::MAX) >> 2;")]
            ///
            /// assert_eq!(n.leading_zeros(), 3);
            /// ```
            #[inline]
            #[must_use = "this returns the result of the operation, \
                          without modifying the original"]
            #[unstable(feature = "wrapping_int_impl", issue = "32463")]
            pub const fn leading_zeros(self) -> u32 {
                self.0.leading_zeros()
            }

            /// 计算 `self` 的绝对值，并在类型边界处按 wrapping 语义回绕。
            ///
            /// 唯一会发生这种回绕的情况，是对该类型的最小负值取绝对值：其数学结果是一个
            /// 无法由该有符号类型表示的正值。此时函数返回 `MIN` 本身。
            ///
            /// # 示例
            ///
            /// 基本用法：
            ///
            /// ```
            /// #![feature(wrapping_int_impl)]
            /// use std::num::Wrapping;
            ///
            #[doc = concat!("assert_eq!(Wrapping(100", stringify!($t), ").abs(), Wrapping(100));")]
            #[doc = concat!("assert_eq!(Wrapping(-100", stringify!($t), ").abs(), Wrapping(100));")]
            #[doc = concat!("assert_eq!(Wrapping(", stringify!($t), "::MIN).abs(), Wrapping(", stringify!($t), "::MIN));")]
            /// assert_eq!(Wrapping(-128i8).abs().0 as u8, 128u8);
            /// ```
            #[inline]
            #[must_use = "this returns the result of the operation, \
                          without modifying the original"]
            #[unstable(feature = "wrapping_int_impl", issue = "32463")]
            pub fn abs(self) -> Wrapping<$t> {
                Wrapping(self.0.wrapping_abs())
            }

            /// 返回表示 `self` 符号的数。
            ///
            ///  - 数值为零时返回 `0`
            ///  - 数值为正时返回 `1`
            ///  - 数值为负时返回 `-1`
            ///
            /// # 示例
            ///
            /// 基本用法：
            ///
            /// ```
            /// #![feature(wrapping_int_impl)]
            /// use std::num::Wrapping;
            ///
            #[doc = concat!("assert_eq!(Wrapping(10", stringify!($t), ").signum(), Wrapping(1));")]
            #[doc = concat!("assert_eq!(Wrapping(0", stringify!($t), ").signum(), Wrapping(0));")]
            #[doc = concat!("assert_eq!(Wrapping(-10", stringify!($t), ").signum(), Wrapping(-1));")]
            /// ```
            #[inline]
            #[must_use = "this returns the result of the operation, \
                          without modifying the original"]
            #[unstable(feature = "wrapping_int_impl", issue = "32463")]
            pub fn signum(self) -> Wrapping<$t> {
                Wrapping(self.0.signum())
            }

            /// 当 `self` 为正数时返回 `true`；为零或负数时返回 `false`。
            ///
            /// # 示例
            ///
            /// 基本用法：
            ///
            /// ```
            /// #![feature(wrapping_int_impl)]
            /// use std::num::Wrapping;
            ///
            #[doc = concat!("assert!(Wrapping(10", stringify!($t), ").is_positive());")]
            #[doc = concat!("assert!(!Wrapping(-10", stringify!($t), ").is_positive());")]
            /// ```
            #[must_use]
            #[inline]
            #[unstable(feature = "wrapping_int_impl", issue = "32463")]
            pub const fn is_positive(self) -> bool {
                self.0.is_positive()
            }

            /// 当 `self` 为负数时返回 `true`；为零或正数时返回 `false`。
            ///
            /// # 示例
            ///
            /// 基本用法：
            ///
            /// ```
            /// #![feature(wrapping_int_impl)]
            /// use std::num::Wrapping;
            ///
            #[doc = concat!("assert!(Wrapping(-10", stringify!($t), ").is_negative());")]
            #[doc = concat!("assert!(!Wrapping(10", stringify!($t), ").is_negative());")]
            /// ```
            #[must_use]
            #[inline]
            #[unstable(feature = "wrapping_int_impl", issue = "32463")]
            pub const fn is_negative(self) -> bool {
                self.0.is_negative()
            }
        }
    )*)
}

wrapping_int_impl_signed! { isize i8 i16 i32 i64 i128 }

macro_rules! wrapping_int_impl_unsigned {
    ($($t:ty)*) => ($(
        impl Wrapping<$t> {
            /// 返回 `self` 的二进制表示中前导 0 的数量。
            ///
            /// # 示例
            ///
            /// 基本用法：
            ///
            /// ```
            /// #![feature(wrapping_int_impl)]
            /// use std::num::Wrapping;
            ///
            #[doc = concat!("let n = Wrapping(", stringify!($t), "::MAX) >> 2;")]
            ///
            /// assert_eq!(n.leading_zeros(), 2);
            /// ```
            #[inline]
            #[must_use = "this returns the result of the operation, \
                          without modifying the original"]
            #[unstable(feature = "wrapping_int_impl", issue = "32463")]
            pub const fn leading_zeros(self) -> u32 {
                self.0.leading_zeros()
            }

            /// 当且仅当存在某个 `k` 使 `self == 2^k` 时返回 `true`。
            ///
            /// # 示例
            ///
            /// 基本用法：
            ///
            /// ```
            /// #![feature(wrapping_int_impl)]
            /// use std::num::Wrapping;
            ///
            #[doc = concat!("assert!(Wrapping(16", stringify!($t), ").is_power_of_two());")]
            #[doc = concat!("assert!(!Wrapping(10", stringify!($t), ").is_power_of_two());")]
            /// ```
            #[must_use]
            #[inline]
            #[unstable(feature = "wrapping_int_impl", issue = "32463")]
            pub fn is_power_of_two(self) -> bool {
                self.0.is_power_of_two()
            }

            /// 返回大于或等于 `self` 的最小 2 的幂。
            ///
            /// 当返回值溢出时（例如对 `uN` 类型有 `self > (1 << (N - 1))`），结果按
            /// wrapping 语义回绕为 `2^N = 0`。
            ///
            /// # 示例
            ///
            /// 基本用法：
            ///
            /// ```
            /// #![feature(wrapping_next_power_of_two)]
            /// use std::num::Wrapping;
            ///
            #[doc = concat!("assert_eq!(Wrapping(2", stringify!($t), ").next_power_of_two(), Wrapping(2));")]
            #[doc = concat!("assert_eq!(Wrapping(3", stringify!($t), ").next_power_of_two(), Wrapping(4));")]
            #[doc = concat!("assert_eq!(Wrapping(200_u8).next_power_of_two(), Wrapping(0));")]
            /// ```
            #[inline]
            #[must_use = "this returns the result of the operation, \
                          without modifying the original"]
            #[unstable(feature = "wrapping_next_power_of_two", issue = "32463",
                       reason = "needs decision on wrapping behavior")]
            pub fn next_power_of_two(self) -> Self {
                Wrapping(self.0.wrapping_next_power_of_two())
            }
        }
    )*)
}

wrapping_int_impl_unsigned! { usize u8 u16 u32 u64 u128 }
