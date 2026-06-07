//! exponent 估算器。
//!
//! flt2dec 的数字生成算法需要一个接近 `ceil(log10(value))` 的十进制尺度估计值。这里使用
//! 整数近似，保证估计值只可能是真实值或差 1，从而后续算法可以用少量校正得到正确尺度。

/// 寻找 `k_0`，使 `10^(k_0-1) < mant * 2^exp <= 10^(k_0+1)`。
///
/// 它用于近似 `k = ceil(log_10 (mant * 2^exp))`；真实的 `k` 要么是 `k_0`，
/// 要么是 `k_0 + 1`。
#[doc(hidden)]
pub fn estimate_scaling_factor(mant: u64, exp: i16) -> i16 {
    // 当 `mant > 0` 时，`2^(nbits - 1) < mant <= 2^nbits`。
    let nbits = 64 - (mant - 1).leading_zeros() as i64;
    // 1292913986 = floor(2^32 * log_10 2)，因此该估算总是偏小（或正好精确），
    // 但误差不大。
    (((nbits + exp as i64) * 1292913986) >> 32) as i16
}
