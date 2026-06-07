//! 把浮点值解码为独立字段和舍入误差区间。
//!
//! flt2dec 需要知道哪些实数会在回读时舍入到原始浮点值。这里把 IEEE 754 位模式拆成
//! 无符号 mantissa、共享二进制 exponent，以及上下误差边界，供 shortest/exact/fixed
//! 数字生成算法判断十进制输出是否仍会回到同一个浮点值。

use crate::num::FpCategory;
use crate::num::dec2flt::float::RawFloat;

/// 解码后的无符号有限值，并满足：
///
/// - 原始值等于 `mant * 2^exp`。
///
/// - 从 `(mant - minus) * 2^exp` 到 `(mant + plus) * 2^exp` 的任意数都会舍入到原始值。
///   仅当 `inclusive` 为 `true` 时，该区间边界才是闭合的。
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct Decoded {
    /// 缩放后的 mantissa。
    pub mant: u64,
    /// 下侧误差范围。
    pub minus: u64,
    /// 上侧误差范围。
    pub plus: u64,
    /// 以 2 为底的共享 exponent。
    pub exp: i16,
    /// 误差区间是否包含边界。
    ///
    /// 在 IEEE 754 的 ties-to-even 规则下，原始 mantissa 为偶数时该值为 true。
    pub inclusive: bool,
}

/// 解码后的无符号值。
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum FullDecoded {
    /// NaN。
    Nan,
    /// 正或负 Inf。
    Infinite,
    /// 正或负零。
    Zero,
    /// 带有进一步解码字段的有限数。
    Finite(Decoded),
}

/// 可由 `decode` 拆解的浮点类型。
pub trait DecodableFloat: RawFloat + Copy {
    /// 最小正 normal 值。
    fn min_pos_norm_value() -> Self;
}

#[cfg(target_has_reliable_f16)]
impl DecodableFloat for f16 {
    fn min_pos_norm_value() -> Self {
        f16::MIN_POSITIVE
    }
}

impl DecodableFloat for f32 {
    fn min_pos_norm_value() -> Self {
        f32::MIN_POSITIVE
    }
}

impl DecodableFloat for f64 {
    fn min_pos_norm_value() -> Self {
        f64::MIN_POSITIVE
    }
}

/// 从给定浮点数返回符号（负数时为 true）和 `FullDecoded` 值。
pub fn decode<T: DecodableFloat>(v: T) -> (/*negative?*/ bool, FullDecoded) {
    let (mant, exp, sign) = v.integer_decode();
    let even = (mant & 1) == 0;
    let decoded = match v.classify() {
        FpCategory::Nan => FullDecoded::Nan,
        FpCategory::Infinite => FullDecoded::Infinite,
        FpCategory::Zero => FullDecoded::Zero,
        FpCategory::Subnormal => {
            // 相邻值：(mant - 2, exp) -- (mant, exp) -- (mant + 2, exp)。
            // `Float::integer_decode` 始终保留 exponent，因此 subnormal 的 mantissa 已经缩放。
            FullDecoded::Finite(Decoded { mant, minus: 1, plus: 1, exp, inclusive: even })
        }
        FpCategory::Normal => {
            let minnorm = <T as DecodableFloat>::min_pos_norm_value().integer_decode();
            if mant == minnorm.0 {
                // 相邻值：(maxmant, exp - 1) -- (minnormmant, exp) --
                // (minnormmant + 1, exp)，其中 maxmant = minnormmant * 2 - 1。
                FullDecoded::Finite(Decoded {
                    mant: mant << 2,
                    minus: 1,
                    plus: 2,
                    exp: exp - 2,
                    inclusive: even,
                })
            } else {
                // 相邻值：(mant - 1, exp) -- (mant, exp) -- (mant + 1, exp)。
                FullDecoded::Finite(Decoded {
                    mant: mant << 1,
                    minus: 1,
                    plus: 1,
                    exp: exp - 1,
                    inclusive: even,
                })
            }
        }
    };
    (sign < 0, decoded)
}
