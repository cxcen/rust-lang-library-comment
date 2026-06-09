//! 面向泛型浮点类型的辅助 trait。

use core::f64;

use crate::fmt::{Debug, LowerExp};
use crate::num::FpCategory;
use crate::ops::{self, Add, Div, Mul, Neg};

/// 两个类型之间可能有损的 `as` 转换。
pub trait CastInto<T: Copy>: Copy {
    fn cast(self) -> T;
}

/// 让算法能对不同整数位宽保持泛型的一组 trait 约束。
pub trait Integer:
    Sized
    + Clone
    + Copy
    + Debug
    + ops::Shr<u32, Output = Self>
    + ops::Shl<u32, Output = Self>
    + ops::BitAnd<Output = Self>
    + ops::BitOr<Output = Self>
    + PartialEq
    + CastInto<i16>
{
    const ZERO: Self;
    const ONE: Self;
}

macro_rules! int {
    ($($ty:ty),+) => {
        $(
            impl CastInto<i16> for $ty {
                fn cast(self) -> i16 {
                    self as i16
                }
            }

            impl Integer for $ty {
                const ZERO: Self = 0;
                const ONE: Self = 1;
            }
        )+
    }
}

int!(u16, u32, u64);

/// 避免为每种 IEEE 754 浮点类型重复编写转换代码的辅助 trait。
///
/// 为什么不能先解析为 `f64` 再转换为较小类型，见父模块文档中关于二次舍入的说明。
///
/// 该 trait 只描述标准浮点格式的内部契约，**绝不**应为其他类型实现，也不应在 `dec2flt`
/// 模块之外使用。
#[doc(hidden)]
pub trait RawFloat:
    Sized
    + Div<Output = Self>
    + Neg<Output = Self>
    + Mul<Output = Self>
    + Add<Output = Self>
    + LowerExp
    + PartialEq
    + PartialOrd
    + Default
    + Clone
    + Copy
    + Debug
{
    /// 与浮点类型具有相同大小的无符号整数类型。
    type Int: Integer + Into<u64>;

    /* 通用常量 */

    const INFINITY: Self;
    const NEG_INFINITY: Self;
    const NAN: Self;
    const NEG_NAN: Self;

    /// 浮点类型的总位宽。
    const BITS: u32;

    /// significand 的位数，**包含**隐藏位。
    const SIG_TOTAL_BITS: u32;

    const EXP_MASK: Self::Int;
    const SIG_MASK: Self::Int;

    /// significand 的位数，**不包含**隐藏位。
    const SIG_BITS: u32 = Self::SIG_TOTAL_BITS - 1;

    /// exponent 字段的位数。
    const EXP_BITS: u32 = Self::BITS - Self::SIG_BITS - 1;

    /// exponent 的饱和值（最大位模式），也就是 Inf/NaN 使用的 exponent 编码。
    ///
    /// 该值尚未左移到 exponent 字段位置；需要已移位掩码时使用 `EXP_MASK`。
    const EXP_SAT: u32 = (1 << Self::EXP_BITS) - 1;

    /// `EXP_SAT` 的有符号版本，便于大量指数计算复用。
    const INFINITE_POWER: i32 = Self::EXP_SAT as i32;

    /// exponent bias 值；它也是有限普通值可用 exponent 的最大无偏值。
    const EXP_BIAS: u32 = Self::EXP_SAT >> 1;

    /// 普通(normal)值的最小无偏 exponent。
    const EXP_MIN: i32 = -(Self::EXP_BIAS as i32 - 1);

    /// ties-to-even 只需要在有限的十进制指数范围内额外处理。
    ///
    /// 当 `q >= 0` 时，有 `5^q <= 2m + 1`。对 64 位路径，
    /// `5^q <= 2m + 1 <= 2^54`，因此 `q <= 23`；对 32 位路径，
    /// `5^q <= 2m + 1 <= 2^25`，因此 `q <= 10`。
    ///
    /// 当 `q < 0` 时，有 `w >= (2m + 1) * 5^-q`。由于必须满足 `w < 2^64`，
    /// 因而 `(2m + 1) * 5^-q < 2^64`。64 位情况下 `2m + 1 > 2^53`，
    /// 32 位情况下 `2m + 1 > 2^24`，所以分别需要
    /// `2^53 * 5^-q < 2^64` 与 `2^24 * 5^-q < 2^64`。这推出 64 位下
    /// `5^-q < 2^11`，即 `q >= -4`；32 位下 `5^-q < 2^40`，即 `q >= -17`。
    ///
    /// 因此只有 `q` 位于 64 位的 `[-4, 23]` 或 32 位的 `[-17, 10]` 时，才需要显式检查
    /// ties-to-even。两个范围内的 `5^|q|` 都能放进 64 位字。
    const MIN_EXPONENT_ROUND_TO_EVEN: i32;
    const MAX_EXPONENT_ROUND_TO_EVEN: i32;

    /* 与 fast path 有关的边界 */

    /// 仍可能得到非 Inf 值的最大十进制 exponent。
    ///
    /// 这是把二进制最大 exponent 转换成十进制后的边界。大于 `10^LARGEST_POWER_OF_TEN`
    /// 的输入会舍入为 Inf，可以快速处理。
    const LARGEST_POWER_OF_TEN: i32 = {
        let largest_pow2 = Self::EXP_BIAS + 1;
        pow2_to_pow10(largest_pow2 as i64) as i32
    };

    /// 仍可能得到非零值的最小十进制 exponent。
    ///
    /// 小于 `10^SMALLEST_POWER_OF_TEN` 的输入会舍入为零，可以快速处理。
    ///
    /// 最小 10 的幂表示为 `floor(log10(2^-n / (2^64 - 1)))`，其中 `n` 是最小 2 的幂。
    /// 分母 `2^64 - 1` 来自中间存储格式可表示的值数量；这个存储格式为何进入该界限，
    /// 仍是当前实现沿用的算法细节。
    ///
    /// 这些值可以用公式计算；但中间量会超过 `f64` 范围，因此无法在编译期直接计算。
    const SMALLEST_POWER_OF_TEN: i32;

    /// fast path 支持的最大 exponent，即 `floor((SIG_BITS + 1) / log2(5))`。
    // 假设 FLT_EVAL_METHOD = 0。
    const MAX_EXPONENT_FAST_PATH: i64 = {
        let log2_5 = f64::consts::LOG2_10 - 1.0;
        (Self::SIG_TOTAL_BITS as f64 / log2_5) as i64
    };

    /// fast path 支持的最小 exponent，即 `-floor((SIG_BITS + 1) / log2(5))`。
    const MIN_EXPONENT_FAST_PATH: i64 = -Self::MAX_EXPONENT_FAST_PATH;

    /// disguised-fast path 能表示的最大 exponent。
    ///
    /// 该值为 `MAX_EXPONENT_FAST_PATH + floor((SIG_BITS + 1) / log2(10))`。
    const MAX_EXPONENT_DISGUISED_FAST_PATH: i64 =
        Self::MAX_EXPONENT_FAST_PATH + (Self::SIG_TOTAL_BITS as f64 / f64::consts::LOG2_10) as i64;

    /// fast path 允许的最大 mantissa（对 f64 是 `1 << 53`）。
    const MAX_MANTISSA_FAST_PATH: u64 = 1 << Self::SIG_TOTAL_BITS;

    /// 通过 `as` 转换把整数转成浮点。
    ///
    /// 该函数只在 fast path 算法中调用；调用前会保证值不超过
    /// `Self::MAX_MANTISSA_FAST_PATH`，因此不会丢失精度。
    fn from_u64(v: u64) -> Self;

    /// 从整数位模式构造浮点值。
    fn from_u64_bits(v: u64) -> Self;

    /// 取得 fast path 乘法用的小型 10 的幂。
    fn pow10_fast_path(exponent: usize) -> Self;

    /// 返回该数所属的浮点分类。
    fn classify(self) -> FpCategory;

    /// 转换为底层整数位表示。
    fn to_bits(self) -> Self::Int;

    /// 以整数形式返回 mantissa、exponent 和符号。
    ///
    /// 返回 `(m, p, s)`，满足 `s * m * 2^p` 表示原浮点数。对 0，exponent 是
    /// `-(EXP_BIAS + SIG_BITS)`，也就是最小 subnormal 幂。对 Inf 或 NaN，exponent 是
    /// `EXP_SAT - EXP_BIAS - SIG_BITS`。
    ///
    /// 若值为 subnormal，mantissa 会左移一位；否则返回时会补上显式隐藏位，但不做其他移位。
    ///
    /// `s` 只会是 `+1` 或 `-1`。
    fn integer_decode(self) -> (u64, i16, i8) {
        let bits = self.to_bits();
        let sign: i8 = if bits >> (Self::BITS - 1) == Self::Int::ZERO { 1 } else { -1 };
        let mut exponent: i16 = ((bits & Self::EXP_MASK) >> Self::SIG_BITS).cast();
        let mantissa = if exponent == 0 {
            (bits & Self::SIG_MASK) << 1
        } else {
            (bits & Self::SIG_MASK) | (Self::Int::ONE << Self::SIG_BITS)
        };
        // exponent bias 加上 mantissa 位移。
        exponent -= (Self::EXP_BIAS + Self::SIG_BITS) as i16;
        (mantissa.into(), exponent, sign)
    }
}

/// 求解满足 `10^b = 2^a` 的 `b`。
const fn pow2_to_pow10(a: i64) -> i64 {
    let res = (a as f64) / f64::consts::LOG2_10;
    res as i64
}

#[cfg(target_has_reliable_f16)]
impl RawFloat for f16 {
    type Int = u16;

    const INFINITY: Self = Self::INFINITY;
    const NEG_INFINITY: Self = Self::NEG_INFINITY;
    const NAN: Self = Self::NAN;
    const NEG_NAN: Self = -Self::NAN;

    const BITS: u32 = 16;
    const SIG_TOTAL_BITS: u32 = Self::MANTISSA_DIGITS;
    const EXP_MASK: Self::Int = Self::EXP_MASK;
    const SIG_MASK: Self::Int = Self::MAN_MASK;

    const MIN_EXPONENT_ROUND_TO_EVEN: i32 = -22;
    const MAX_EXPONENT_ROUND_TO_EVEN: i32 = 5;
    const SMALLEST_POWER_OF_TEN: i32 = -27;

    #[inline]
    fn from_u64(v: u64) -> Self {
        debug_assert!(v <= Self::MAX_MANTISSA_FAST_PATH);
        v as _
    }

    #[inline]
    fn from_u64_bits(v: u64) -> Self {
        Self::from_bits((v & 0xFFFF) as u16)
    }

    fn pow10_fast_path(exponent: usize) -> Self {
        #[allow(clippy::use_self)]
        const TABLE: [f16; 8] = [1e0, 1e1, 1e2, 1e3, 1e4, 0.0, 0.0, 0.];
        TABLE[exponent & 7]
    }

    fn to_bits(self) -> Self::Int {
        self.to_bits()
    }

    fn classify(self) -> FpCategory {
        self.classify()
    }
}

impl RawFloat for f32 {
    type Int = u32;

    const INFINITY: Self = f32::INFINITY;
    const NEG_INFINITY: Self = f32::NEG_INFINITY;
    const NAN: Self = f32::NAN;
    const NEG_NAN: Self = -f32::NAN;

    const BITS: u32 = 32;
    const SIG_TOTAL_BITS: u32 = Self::MANTISSA_DIGITS;
    const EXP_MASK: Self::Int = Self::EXP_MASK;
    const SIG_MASK: Self::Int = Self::MAN_MASK;

    const MIN_EXPONENT_ROUND_TO_EVEN: i32 = -17;
    const MAX_EXPONENT_ROUND_TO_EVEN: i32 = 10;
    const SMALLEST_POWER_OF_TEN: i32 = -65;

    #[inline]
    fn from_u64(v: u64) -> Self {
        debug_assert!(v <= Self::MAX_MANTISSA_FAST_PATH);
        v as _
    }

    #[inline]
    fn from_u64_bits(v: u64) -> Self {
        f32::from_bits((v & 0xFFFFFFFF) as u32)
    }

    fn pow10_fast_path(exponent: usize) -> Self {
        #[allow(clippy::use_self)]
        const TABLE: [f32; 16] =
            [1e0, 1e1, 1e2, 1e3, 1e4, 1e5, 1e6, 1e7, 1e8, 1e9, 1e10, 0., 0., 0., 0., 0.];
        TABLE[exponent & 15]
    }

    fn to_bits(self) -> Self::Int {
        self.to_bits()
    }

    fn classify(self) -> FpCategory {
        self.classify()
    }
}

impl RawFloat for f64 {
    type Int = u64;

    const INFINITY: Self = Self::INFINITY;
    const NEG_INFINITY: Self = Self::NEG_INFINITY;
    const NAN: Self = Self::NAN;
    const NEG_NAN: Self = -Self::NAN;

    const BITS: u32 = 64;
    const SIG_TOTAL_BITS: u32 = Self::MANTISSA_DIGITS;
    const EXP_MASK: Self::Int = Self::EXP_MASK;
    const SIG_MASK: Self::Int = Self::MAN_MASK;

    const MIN_EXPONENT_ROUND_TO_EVEN: i32 = -4;
    const MAX_EXPONENT_ROUND_TO_EVEN: i32 = 23;
    const SMALLEST_POWER_OF_TEN: i32 = -342;

    #[inline]
    fn from_u64(v: u64) -> Self {
        debug_assert!(v <= Self::MAX_MANTISSA_FAST_PATH);
        v as _
    }

    #[inline]
    fn from_u64_bits(v: u64) -> Self {
        f64::from_bits(v)
    }

    fn pow10_fast_path(exponent: usize) -> Self {
        const TABLE: [f64; 32] = [
            1e0, 1e1, 1e2, 1e3, 1e4, 1e5, 1e6, 1e7, 1e8, 1e9, 1e10, 1e11, 1e12, 1e13, 1e14, 1e15,
            1e16, 1e17, 1e18, 1e19, 1e20, 1e21, 1e22, 0., 0., 0., 0., 0., 0., 0., 0., 0.,
        ];
        TABLE[exponent & 31]
    }

    fn to_bits(self) -> Self::Int {
        self.to_bits()
    }

    fn classify(self) -> FpCategory {
        self.classify()
    }
}
