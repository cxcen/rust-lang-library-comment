//! 当 Eisel-Lemire algorithm 无法完成正确舍入时使用的慢速 fallback 算法。

use crate::num::dec2flt::common::BiasedFp;
use crate::num::dec2flt::decimal_seq::{DecimalSeq, parse_decimal_seq};
use crate::num::dec2flt::float::RawFloat;

/// 解析浮点数的有效数字和带偏置二进制 exponent。
///
/// 这是使用大整数/大十进制表示的 fallback 算法，因此明显慢于快速近似路径。但它总能判定
/// 有效数字应如何舍入到最近的机器浮点数，从而处理 near-halfway 情况。
///
/// near-halfway 指数值位于两个相邻机器浮点数中间附近。例如 `16777217.0` 的位表示可看作
/// `100000000000000000000000 1`；舍入到单精度时，末尾的 `1` 会被截断。按照
/// round-nearest, tie-even，任何大于 `16777217.0` 的值都必须向上舍入到 `16777218.0`，
/// 而小于或等于 `16777217.0` 的值都必须向下舍入到 `16777216.0`。因此这类转换可能
/// 需要大量十进制数字才能无歧义地决定舍入方向。
///
/// 这里的算法基于 "Processing Long Numbers Quickly"：
/// <https://arxiv.org/pdf/2101.11408.pdf#section.11>。
pub(crate) fn parse_long_mantissa<F: RawFloat>(s: &[u8]) -> BiasedFp {
    const MAX_SHIFT: usize = 60;
    const NUM_POWERS: usize = 19;
    const POWERS: [u8; 19] =
        [0, 3, 6, 9, 13, 16, 19, 23, 26, 29, 33, 36, 39, 43, 46, 49, 53, 56, 59];

    let get_shift = |n| {
        if n < NUM_POWERS { POWERS[n] as usize } else { MAX_SHIFT }
    };

    let fp_zero = BiasedFp::zero_pow2(0);
    let fp_inf = BiasedFp::zero_pow2(F::INFINITE_POWER);

    let mut d = parse_decimal_seq(s);

    // 如果该值只能舍入为字面零或 Inf，直接返回。
    if d.num_digits == 0 || d.decimal_point < -324 {
        return fp_zero;
    } else if d.decimal_point >= 310 {
        return fp_inf;
    }
    let mut exp2 = 0_i32;
    // 向右移位，把值推向 `(1/2 ... 1]` 区间。
    while d.decimal_point > 0 {
        let n = d.decimal_point as usize;
        let shift = get_shift(n);
        d.right_shift(shift);
        if d.decimal_point < -DecimalSeq::DECIMAL_POINT_RANGE {
            return fp_zero;
        }
        exp2 += shift as i32;
    }
    // 向左移位，把值推向 `(1/2 ... 1]` 区间。
    while d.decimal_point <= 0 {
        let shift = if d.decimal_point == 0 {
            match d.digits[0] {
                digit if digit >= 5 => break,
                0 | 1 => 2,
                _ => 1,
            }
        } else {
            get_shift((-d.decimal_point) as _)
        };
        d.left_shift(shift);
        if d.decimal_point > DecimalSeq::DECIMAL_POINT_RANGE {
            return fp_inf;
        }
        exp2 -= shift as i32;
    }
    // 现在位于 `[1/2 ... 1]`，但二进制浮点格式使用 `[1 ... 2]`。
    exp2 -= 1;
    while F::EXP_MIN > exp2 {
        let mut n = (F::EXP_MIN - exp2) as usize;
        if n > MAX_SHIFT {
            n = MAX_SHIFT;
        }
        d.right_shift(n);
        exp2 += n as i32;
    }
    if (exp2 - F::EXP_MIN + 1) >= F::INFINITE_POWER {
        return fp_inf;
    }
    // 把十进制值移动到隐藏位位置，然后舍入，取得高 `mantissa + 1` 位。
    d.left_shift(F::SIG_BITS as usize + 1);
    let mut mantissa = d.round();
    if mantissa >= (1_u64 << (F::SIG_BITS + 1)) {
        // 向上舍入溢出到进位位，需要右移回隐藏位位置。
        d.right_shift(1);
        exp2 += 1;
        mantissa = d.round();
        if (exp2 - F::EXP_MIN + 1) >= F::INFINITE_POWER {
            return fp_inf;
        }
    }
    let mut power2 = exp2 - F::EXP_MIN + 1;
    if mantissa < (1_u64 << F::SIG_BITS) {
        power2 -= 1;
    }
    // 清掉显式 mantissa 位以上的所有位。
    mantissa &= (1_u64 << F::SIG_BITS) - 1;
    BiasedFp { m: mantissa, p_biased: power2 }
}
