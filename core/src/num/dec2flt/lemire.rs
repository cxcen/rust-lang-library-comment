//! Eisel-Lemire algorithm 的实现。
//!
//! 这是 dec2flt 的快速扩展精度路径。它用预计算的 5 的幂把十进制 `(w, q)` 缩放到二进制，
//! 并尽量在 128 位（必要时参考 192 位）中判断结果能否无歧义地舍入到目标 IEEE 754
//! 浮点数。若接近半点或近似精度不足，本模块会返回错误标记，让调用方退回慢速精确路径。

use crate::num::dec2flt::common::BiasedFp;
use crate::num::dec2flt::float::RawFloat;
use crate::num::dec2flt::table::{
    LARGEST_POWER_OF_FIVE, POWER_OF_FIVE_128, SMALLEST_POWER_OF_FIVE,
};

/// 使用扩展精度浮点表示计算 `w * 10^q`。
///
/// 该算法把十进制有效数字和十进制 exponent 快速转换为二进制扩展表示。它能准确解析
/// 绝大多数输入，主要使用 128 位表示，并在需要时借助额外乘法取得接近 192 位的有效信息。
///
/// 算法使用预计算的 5 的幂按十进制 exponent 缩放，并判断当前表示是否能无歧义地舍入到
/// 最近机器浮点数。near-halfway 情况不在这里处理，而是通过负的带偏置二进制 exponent
/// 表示“需要慢路径”。
///
/// 算法详见 "Daniel Lemire, Number Parsing at a Gigabyte per Second" 的第 5 节
/// "Fast Algorithm" 和第 6 节 "Exact Numbers And Ties"：
/// <https://arxiv.org/abs/2101.11408.pdf>.
pub fn compute_float<F: RawFloat>(q: i64, mut w: u64) -> BiasedFp {
    let fp_zero = BiasedFp::zero_pow2(0);
    let fp_inf = BiasedFp::zero_pow2(F::INFINITE_POWER);
    let fp_error = BiasedFp::zero_pow2(-1);

    // 如果该值只能舍入为字面零或 Inf，直接返回。
    if w == 0 || q < F::SMALLEST_POWER_OF_TEN as i64 {
        return fp_zero;
    } else if q > F::LARGEST_POWER_OF_TEN as i64 {
        return fp_inf;
    }
    // 规格化有效数字，使最高有效位被置位。
    let lz = w.leading_zeros();
    w <<= lz;
    let (lo, hi) = compute_product_approx(q, w, F::SIG_BITS as usize + 3);
    if lo == 0xFFFF_FFFF_FFFF_FFFF {
        // 如果 128 位值无法充分近似 `w * 5^-q`，加 1 可能先造成溢出，再跨过半点向上舍入，
        // 从而导致浮点结果舍入错误。
        //
        // 不过这种情况只会在 `q ∈ [-27, 55]` 中发生。上界 55 来自 `5^55 < 2^128`；
        // 但只有当 `5^q > 2^64` 时才需要担心，否则乘积可用 64 位精确表示。对负指数，
        // ties-to-even 只可能在 `5^-q < 2^64` 时发生。
        //
        // 负指数舍入的详细解释见：
        // <https://arxiv.org/pdf/2101.11408.pdf#section.9.1>. For detailed
        // 正指数舍入的详细解释见：
        // <https://arxiv.org/pdf/2101.11408.pdf#section.8>.
        let inside_safe_exponent = (q >= -27) && (q <= 55);
        if !inside_safe_exponent {
            return fp_error;
        }
    }
    let upperbit = (hi >> 63) as i32;
    let mut mantissa = hi >> (upperbit + 64 - F::SIG_BITS as i32 - 3);
    let mut power2 = power(q as i32) + upperbit - lz as i32 - F::EXP_MIN + 1;
    if power2 <= 0 {
        if -power2 + 1 >= 64 {
            // 低于最小 exponent 的部分超过 64 位，只能舍入为 0。
            return fp_zero;
        }
        // 得到 subnormal 值。
        mantissa >>= -power2 + 1;
        mantissa += mantissa & 1;
        mantissa >>= 1;
        power2 = (mantissa >= (1_u64 << F::SIG_BITS)) as i32;
        return BiasedFp { m: mantissa, p_biased: power2 };
    }
    // 需要处理 ties。通常需要向上舍入；但如果精确落在中间且低位基准为偶数，则需要向下，
    // 以满足 ties-to-even。
    //
    // 这种情况只会在下列条件同时成立时发生：
    //  1. 128 位表示的低 64 位为 0，也就是 `5^q` 可放入单个 64 位字。
    //  2. 截断前 mantissa 的最低有效位为奇数。
    //  3. 右移到 mantissa 位数加 1 时，被截断的所有位都是 0。
    //
    // 换言之，当前值正好位于两个浮点数中间。
    if lo <= 1
        && q >= F::MIN_EXPONENT_ROUND_TO_EVEN as i64
        && q <= F::MAX_EXPONENT_ROUND_TO_EVEN as i64
        && mantissa & 0b11 == 0b01
        && (mantissa << (upperbit + 64 - F::SIG_BITS as i32 - 3)) == hi
    {
        // 清零最低位，避免向上舍入。
        mantissa &= !1_u64;
    }
    // 执行 ties-to-even 舍入，然后把有效数字移动到目标位置。
    mantissa += mantissa & 1;
    mantissa >>= 1;
    if mantissa >= (2_u64 << F::SIG_BITS) {
        // 向上舍入产生溢出并设置了进位位。把 mantissa 设为 1（只保留隐含隐藏位），
        // 并增加 exponent。
        mantissa = 1_u64 << F::SIG_BITS;
        power2 += 1;
    }
    // 清掉隐藏位。
    mantissa &= !(1_u64 << F::SIG_BITS);
    if power2 >= F::INFINITE_POWER {
        // exponent 超过最大 normal 值，只能是 Inf。
        return fp_inf;
    }
    BiasedFp { m: mantissa, p_biased: power2 }
}

/// 根据十进制 exponent 估算二进制 exponent。
///
/// 这里使用预计算的 `log2(10)` 整数近似；在所有非有限十进制 exponent 范围内，
/// `217706 / 2^16` 都足够准确。
#[inline]
fn power(q: i32) -> i32 {
    (q.wrapping_mul(152_170 + 65536) >> 16) + 63
}

#[inline]
fn full_multiplication(a: u64, b: u64) -> (u64, u64) {
    let r = (a as u128) * (b as u128);
    (r as u64, (r >> 64) as u64)
}

// 计算（更准确地说是近似）`w * 5^q`，并返回一对近似结果的 64 位字：
// high 部分对应最高有效位，low 部分对应最低有效位。
fn compute_product_approx(q: i64, w: u64, precision: usize) -> (u64, u64) {
    debug_assert!(q >= SMALLEST_POWER_OF_FIVE as i64);
    debug_assert!(q <= LARGEST_POWER_OF_FIVE as i64);
    debug_assert!(precision <= 64);

    let mask = if precision < 64 {
        0xFFFF_FFFF_FFFF_FFFF_u64 >> precision
    } else {
        0xFFFF_FFFF_FFFF_FFFF_u64
    };

    // 如果 `5^q < 2^64`，乘法总能给出精确值。这意味着需要 ties-to-even 时，
    // 我们手里一定有精确值。
    let index = (q - SMALLEST_POWER_OF_FIVE as i64) as usize;
    let (lo5, hi5) = POWER_OF_FIVE_128[index];
    // 只要显式 mantissa 位中还有一个 0 位，就只需要一次乘法；额外位分别用于隐藏位、
    // 判定舍入方向，以及处理计算出的乘积带前导零的情况。
    let (mut first_lo, mut first_hi) = full_multiplication(w, lo5);
    if first_hi & mask == mask {
        // 需要第二次乘法来获得低位乘积的更高精度。`q < 55` 时它总是精确的，因为
        // `5^55 < 2^128`。如果这里发生 wrapping，就需要把高位乘积向上进位。
        let (_, second_hi) = full_multiplication(w, hi5);
        first_lo = first_lo.wrapping_add(second_hi);
        if second_hi > first_lo {
            first_hi += 1;
        }
    }
    (first_lo, first_hi)
}
