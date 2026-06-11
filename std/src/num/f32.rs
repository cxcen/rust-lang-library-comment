//! `f32` 单精度（single-precision）浮点类型的相关常量。
//!
//! *[另见 `f32` 原始类型](primitive@f32)。*
//!
//! 具有数学意义的重要数值由 `consts` 子模块提供。
//!
//! 对于直接定义在本模块中的常量
//! （区别于在 `consts` 子模块中定义的那些常量），
//! 新代码应改用
//! 直接定义在 `f32` 类型上的关联常量。
//!
//! 实现说明：本模块在 std 而非 core 中提供 `f32` 的浮点数学方法（如 `sqrt`、
//! `sin`、`exp`、`ln`、`hypot` 等），是因为这些函数的实现依赖编译器内建函数
//! （intrinsics）或底层平台/系统的数学库（在 Unix 与 Windows 上即 libc/libm）。
//! 这类运行时依赖在不依赖操作系统的 core 中无法满足，因此只有 std 才能提供它们；
//! 而那些不依赖运行时、纯位运算即可完成的方法则定义在 core 上。

#![stable(feature = "rust1", since = "1.0.0")]
#![allow(missing_docs)]

#[stable(feature = "rust1", since = "1.0.0")]
#[allow(deprecated, deprecated_in_future)]
pub use core::f32::{
    DIGITS, EPSILON, INFINITY, MANTISSA_DIGITS, MAX, MAX_10_EXP, MAX_EXP, MIN, MIN_10_EXP, MIN_EXP,
    MIN_POSITIVE, NAN, NEG_INFINITY, RADIX, consts,
};

#[cfg(not(test))]
use crate::intrinsics;
#[cfg(not(test))]
use crate::sys::cmath;

#[cfg(not(test))]
impl f32 {
    /// 返回小于等于 `self` 的最大整数。
    ///
    /// 本函数始终返回精确结果。
    ///
    /// # 示例
    ///
    /// ```
    /// let f = 3.7_f32;
    /// let g = 3.0_f32;
    /// let h = -3.7_f32;
    ///
    /// assert_eq!(f.floor(), 3.0);
    /// assert_eq!(g.floor(), 3.0);
    /// assert_eq!(h.floor(), -4.0);
    /// ```
    #[rustc_allow_incoherent_impl]
    #[must_use = "method returns a new number and does not mutate the original value"]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_float_round_methods", since = "1.90.0")]
    #[inline]
    pub const fn floor(self) -> f32 {
        core::f32::math::floor(self)
    }

    /// 返回大于等于 `self` 的最小整数。
    ///
    /// 本函数始终返回精确结果。
    ///
    /// # 示例
    ///
    /// ```
    /// let f = 3.01_f32;
    /// let g = 4.0_f32;
    ///
    /// assert_eq!(f.ceil(), 4.0);
    /// assert_eq!(g.ceil(), 4.0);
    /// ```
    #[doc(alias = "ceiling")]
    #[rustc_allow_incoherent_impl]
    #[must_use = "method returns a new number and does not mutate the original value"]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_float_round_methods", since = "1.90.0")]
    #[inline]
    pub const fn ceil(self) -> f32 {
        core::f32::math::ceil(self)
    }

    /// 返回最接近 `self` 的整数。如果某个值恰好处于两个整数正中间，
    /// 则朝远离 `0.0` 的方向舍入。
    ///
    /// 本函数始终返回精确结果。
    ///
    /// # 示例
    ///
    /// ```
    /// let f = 3.3_f32;
    /// let g = -3.3_f32;
    /// let h = -3.7_f32;
    /// let i = 3.5_f32;
    /// let j = 4.5_f32;
    ///
    /// assert_eq!(f.round(), 3.0);
    /// assert_eq!(g.round(), -3.0);
    /// assert_eq!(h.round(), -4.0);
    /// assert_eq!(i.round(), 4.0);
    /// assert_eq!(j.round(), 5.0);
    /// ```
    #[rustc_allow_incoherent_impl]
    #[must_use = "method returns a new number and does not mutate the original value"]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_float_round_methods", since = "1.90.0")]
    #[inline]
    pub const fn round(self) -> f32 {
        core::f32::math::round(self)
    }

    /// 返回最接近某个数的整数。对于恰好处于正中间的情形，舍入到最低有效位为偶数的那个数
    /// （即四舍六入五成双）。
    ///
    /// 本函数始终返回精确结果。
    ///
    /// # 示例
    ///
    /// ```
    /// let f = 3.3_f32;
    /// let g = -3.3_f32;
    /// let h = 3.5_f32;
    /// let i = 4.5_f32;
    ///
    /// assert_eq!(f.round_ties_even(), 3.0);
    /// assert_eq!(g.round_ties_even(), -3.0);
    /// assert_eq!(h.round_ties_even(), 4.0);
    /// assert_eq!(i.round_ties_even(), 4.0);
    /// ```
    #[rustc_allow_incoherent_impl]
    #[must_use = "method returns a new number and does not mutate the original value"]
    #[stable(feature = "round_ties_even", since = "1.77.0")]
    #[rustc_const_stable(feature = "const_float_round_methods", since = "1.90.0")]
    #[inline]
    pub const fn round_ties_even(self) -> f32 {
        core::f32::math::round_ties_even(self)
    }

    /// 返回 `self` 的整数部分。
    /// 这意味着非整数总是朝零方向截断。
    ///
    /// 本函数始终返回精确结果。
    ///
    /// # 示例
    ///
    /// ```
    /// let f = 3.7_f32;
    /// let g = 3.0_f32;
    /// let h = -3.7_f32;
    ///
    /// assert_eq!(f.trunc(), 3.0);
    /// assert_eq!(g.trunc(), 3.0);
    /// assert_eq!(h.trunc(), -3.0);
    /// ```
    #[doc(alias = "truncate")]
    #[rustc_allow_incoherent_impl]
    #[must_use = "method returns a new number and does not mutate the original value"]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_float_round_methods", since = "1.90.0")]
    #[inline]
    pub const fn trunc(self) -> f32 {
        core::f32::math::trunc(self)
    }

    /// 返回 `self` 的小数部分。
    ///
    /// 本函数始终返回精确结果。
    ///
    /// # 示例
    ///
    /// ```
    /// let x = 3.6_f32;
    /// let y = -3.6_f32;
    /// let abs_difference_x = (x.fract() - 0.6).abs();
    /// let abs_difference_y = (y.fract() - (-0.6)).abs();
    ///
    /// assert!(abs_difference_x <= f32::EPSILON);
    /// assert!(abs_difference_y <= f32::EPSILON);
    /// ```
    #[rustc_allow_incoherent_impl]
    #[must_use = "method returns a new number and does not mutate the original value"]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_float_round_methods", since = "1.90.0")]
    #[inline]
    pub const fn fract(self) -> f32 {
        core::f32::math::fract(self)
    }

    /// 融合乘加（fused multiply-add）。计算 `(self * a) + b`，全程只产生一次舍入
    /// 误差，因此得到的结果比未融合的乘加更精确。
    ///
    /// 如果目标架构有专门的 `fma` CPU 指令，使用 `mul_add` *可能* 比未融合的乘加更高效。
    /// 然而，这并非总是成立，
    /// 它在很大程度上取决于针对特定目标硬件来设计算法。
    /// 它在很大程度上取决于针对特定目标硬件来设计算法。
    ///
    /// # 精度(Precision）
    ///
    /// 本运算的结果保证是对无限精度结果进行舍入后的值。
    /// 无限精度结果进行舍入后的值。它由 IEEE 754 规定为
    /// `fusedMultiplyAdd`，并保证不会改变。
    ///
    /// # 示例
    ///
    /// ```
    /// let m = 10.0_f32;
    /// let x = 4.0_f32;
    /// let b = 60.0_f32;
    ///
    /// assert_eq!(m.mul_add(x, b), 100.0);
    /// assert_eq!(m * x + b, 100.0);
    ///
    /// let one_plus_eps = 1.0_f32 + f32::EPSILON;
    /// let one_minus_eps = 1.0_f32 - f32::EPSILON;
    /// let minus_one = -1.0_f32;
    ///
    /// // 精确结果为 (1 + eps) * (1 - eps) = 1 - eps * eps。
    /// assert_eq!(one_plus_eps.mul_add(one_minus_eps, minus_one), -f32::EPSILON * f32::EPSILON);
    /// // 未融合的乘法和加法会产生不同的舍入。
    /// assert_eq!(one_plus_eps * one_minus_eps + minus_one, 0.0);
    /// ```
    #[rustc_allow_incoherent_impl]
    #[doc(alias = "fmaf", alias = "fusedMultiplyAdd")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    #[rustc_const_stable(feature = "const_mul_add", since = "1.94.0")]
    pub const fn mul_add(self, a: f32, b: f32) -> f32 {
        core::f32::math::mul_add(self, a, b)
    }

    /// 计算欧几里得除法，是与 `rem_euclid` 相匹配的方法。
    ///
    /// 它计算出满足下式的整数 `n`：
    /// `self = n * rhs + self.rem_euclid(rhs)`。
    /// 换句话说，结果是把 `self / rhs` 向整数 `n` 舍入后的值，
    /// 使得 `self >= n * rhs`。
    ///
    /// # 精度(Precision）
    ///
    /// 本运算的结果保证是对无限精度结果进行舍入后的值。
    /// 无限精度结果进行舍入后的值。
    ///
    /// # 示例
    ///
    /// ```
    /// let a: f32 = 7.0;
    /// let b = 4.0;
    /// assert_eq!(a.div_euclid(b), 1.0); // 7.0 > 4.0 * 1.0
    /// assert_eq!((-a).div_euclid(b), -2.0); // -7.0 >= 4.0 * -2.0
    /// assert_eq!(a.div_euclid(-b), -1.0); // 7.0 >= -4.0 * -1.0
    /// assert_eq!((-a).div_euclid(-b), 2.0); // -7.0 >= -4.0 * 2.0
    /// ```
    #[rustc_allow_incoherent_impl]
    #[must_use = "method returns a new number and does not mutate the original value"]
    #[inline]
    #[stable(feature = "euclidean_division", since = "1.38.0")]
    pub fn div_euclid(self, rhs: f32) -> f32 {
        core::f32::math::div_euclid(self, rhs)
    }

    /// 计算 `self` 除以下述除数后的最小非负余数：
    /// `rhs`。
    ///
    /// 具体而言，返回值 `r` 在大多数情况下满足 `0.0 <= r < rhs.abs()`。
    /// 然而，由于浮点舍入误差，在某些情况下可能得到 `r == rhs.abs()`，从而违反数学定义：
    /// 即当满足下述条件时：
    /// 当 `self` 的量级远小于 `rhs.abs()` 且 `self < 0.0` 时则会出现这种情况。
    /// 该结果并不属于函数的值域，但它是实数中最接近的浮点数，
    /// 因此它近似地满足如下性质：
    /// 性质 `self == self.div_euclid(rhs) * rhs + self.rem_euclid(rhs)`
    /// （近似地）。
    ///
    /// # 精度(Precision）
    ///
    /// 本运算的结果保证是对无限精度结果进行舍入后的值。
    /// 无限精度结果进行舍入后的值。
    ///
    /// # 示例
    ///
    /// ```
    /// let a: f32 = 7.0;
    /// let b = 4.0;
    /// assert_eq!(a.rem_euclid(b), 3.0);
    /// assert_eq!((-a).rem_euclid(b), 1.0);
    /// assert_eq!(a.rem_euclid(-b), 3.0);
    /// assert_eq!((-a).rem_euclid(-b), 1.0);
    /// // 由舍入误差导致的局限
    /// assert!((-f32::EPSILON).rem_euclid(3.0) != 0.0);
    /// ```
    #[doc(alias = "modulo", alias = "mod")]
    #[rustc_allow_incoherent_impl]
    #[must_use = "method returns a new number and does not mutate the original value"]
    #[inline]
    #[stable(feature = "euclidean_division", since = "1.38.0")]
    pub fn rem_euclid(self, rhs: f32) -> f32 {
        core::f32::math::rem_euclid(self, rhs)
    }

    /// 计算一个数的整数次幂。
    ///
    /// 使用本函数通常比使用 `powf` 更快。
    /// 它的舍入操作序列可能与 `powf` 不同，
    /// 因此不保证两者结果一致。
    ///
    /// 注意本函数较为特殊：对于 NaN 输入它也可能返回非 NaN 的结果。例如，
    /// 例如，`f32::powi(f32::NAN, 0)` 返回 `1.0`。但是，如果某个输入是一个 *signaling*（信号）
    /// NaN（即 *signaling* NaN），则结果不确定：要么是 NaN，要么是对应的安静（quiet）
    /// NaN 所产生的结果。
    ///
    /// # 未指定精度(Unspecified precision）
    ///
    /// 本函数的精度是不确定的。这意味着它会随平台、Rust 版本而变化，
    /// 甚至在同一次执行中，前后两次调用之间也可能不同。
    ///
    /// # 示例
    ///
    /// ```
    /// let x = 2.0_f32;
    /// let abs_difference = (x.powi(2) - (x * x)).abs();
    /// assert!(abs_difference <= 1e-5);
    ///
    /// assert_eq!(f32::powi(f32::NAN, 0), 1.0);
    /// assert_eq!(f32::powi(0.0, 0), 1.0);
    /// ```
    #[rustc_allow_incoherent_impl]
    #[must_use = "method returns a new number and does not mutate the original value"]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn powi(self, n: i32) -> f32 {
        core::f32::math::powi(self, n)
    }

    /// 计算一个数的浮点数次幂。
    ///
    /// 注意本函数较为特殊：对于 NaN 输入它也可能返回非 NaN 的结果。例如，
    /// 例如，`f32::powf(f32::NAN, 0.0)` 返回 `1.0`。但是，如果某个输入是一个 *signaling*（信号）
    /// NaN（即 *signaling* NaN），则结果不确定：要么是 NaN，要么是对应的安静（quiet）
    /// NaN 所产生的结果。
    ///
    /// # 未指定精度(Unspecified precision）
    ///
    /// 本函数的精度是不确定的。这意味着它会随平台、Rust 版本而变化，
    /// 甚至在同一次执行中，前后两次调用之间也可能不同。
    ///
    /// # 示例
    ///
    /// ```
    /// let x = 2.0_f32;
    /// let abs_difference = (x.powf(2.0) - (x * x)).abs();
    /// assert!(abs_difference <= 1e-5);
    ///
    /// assert_eq!(f32::powf(1.0, f32::NAN), 1.0);
    /// assert_eq!(f32::powf(f32::NAN, 0.0), 1.0);
    /// assert_eq!(f32::powf(0.0, 0.0), 1.0);
    /// ```
    #[rustc_allow_incoherent_impl]
    #[must_use = "method returns a new number and does not mutate the original value"]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn powf(self, n: f32) -> f32 {
        intrinsics::powf32(self, n)
    }

    /// 返回一个数的平方根。
    ///
    /// 若 `self` 是除 `-0.0` 以外的负数，则返回 NaN。
    ///
    /// # 精度(Precision）
    ///
    /// 本运算的结果保证是对无限精度结果进行舍入后的值。
    /// 无限精度结果进行舍入后的值。它由 IEEE 754 规定为 `squareRoot`，
    /// 并保证不会改变。
    ///
    /// # 示例
    ///
    /// ```
    /// let positive = 4.0_f32;
    /// let negative = -4.0_f32;
    /// let negative_zero = -0.0_f32;
    ///
    /// assert_eq!(positive.sqrt(), 2.0);
    /// assert!(negative.sqrt().is_nan());
    /// assert!(negative_zero.sqrt() == negative_zero);
    /// ```
    #[doc(alias = "squareRoot")]
    #[rustc_allow_incoherent_impl]
    #[must_use = "method returns a new number and does not mutate the original value"]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn sqrt(self) -> f32 {
        core::f32::math::sqrt(self)
    }

    /// 返回 `e^(self)`（即指数函数）。
    ///
    /// # 未指定精度(Unspecified precision）
    ///
    /// 本函数的精度是不确定的。这意味着它会随平台、Rust 版本而变化，
    /// 甚至在同一次执行中，前后两次调用之间也可能不同。
    ///
    /// # 示例
    ///
    /// ```
    /// let one = 1.0f32;
    /// // e^1
    /// let e = one.exp();
    ///
    /// // ln(e) - 1 == 0
    /// let abs_difference = (e.ln() - 1.0).abs();
    ///
    /// assert!(abs_difference <= 1e-6);
    /// ```
    #[rustc_allow_incoherent_impl]
    #[must_use = "method returns a new number and does not mutate the original value"]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn exp(self) -> f32 {
        intrinsics::expf32(self)
    }

    /// 返回 `2^(self)`。
    ///
    /// # 未指定精度(Unspecified precision）
    ///
    /// 本函数的精度是不确定的。这意味着它会随平台、Rust 版本而变化，
    /// 甚至在同一次执行中，前后两次调用之间也可能不同。
    ///
    /// # 示例
    ///
    /// ```
    /// let f = 2.0f32;
    ///
    /// // 2^2 - 4 == 0
    /// let abs_difference = (f.exp2() - 4.0).abs();
    ///
    /// assert!(abs_difference <= 1e-5);
    /// ```
    #[rustc_allow_incoherent_impl]
    #[must_use = "method returns a new number and does not mutate the original value"]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn exp2(self) -> f32 {
        intrinsics::exp2f32(self)
    }

    /// 返回该数的自然对数。
    ///
    /// 当该数为负时返回 NaN，当该数为零时返回负无穷。
    ///
    /// # 未指定精度(Unspecified precision）
    ///
    /// 本函数的精度是不确定的。这意味着它会随平台、Rust 版本而变化，
    /// 甚至在同一次执行中，前后两次调用之间也可能不同。
    ///
    /// # 示例
    ///
    /// ```
    /// let one = 1.0f32;
    /// // e^1
    /// let e = one.exp();
    ///
    /// // ln(e) - 1 == 0
    /// let abs_difference = (e.ln() - 1.0).abs();
    ///
    /// assert!(abs_difference <= 1e-6);
    /// ```
    ///
    /// 非正值：
    /// ```
    /// assert_eq!(0_f32.ln(), f32::NEG_INFINITY);
    /// assert!((-42_f32).ln().is_nan());
    /// ```
    #[rustc_allow_incoherent_impl]
    #[must_use = "method returns a new number and does not mutate the original value"]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn ln(self) -> f32 {
        intrinsics::logf32(self)
    }

    /// 返回该数关于任意底数的对数。
    ///
    /// 当该数为负时返回 NaN，当该数为零时返回负无穷。
    ///
    /// 由于实现细节，结果可能未被正确舍入；
    /// 对于以 2 为底，`self.log2()` 能给出更精确的结果，
    /// 对于以 10 为底，`self.log10()` 能给出更精确的结果。
    ///
    /// # 未指定精度(Unspecified precision）
    ///
    /// 本函数的精度是不确定的。这意味着它会随平台、Rust 版本而变化，
    /// 甚至在同一次执行中，前后两次调用之间也可能不同。
    ///
    /// # 示例
    ///
    /// ```
    /// let five = 5.0f32;
    ///
    /// // log5(5) - 1 == 0
    /// let abs_difference = (five.log(5.0) - 1.0).abs();
    ///
    /// assert!(abs_difference <= 1e-6);
    /// ```
    ///
    /// 非正值：
    /// ```
    /// assert_eq!(0_f32.log(10.0), f32::NEG_INFINITY);
    /// assert!((-42_f32).log(10.0).is_nan());
    /// ```
    #[rustc_allow_incoherent_impl]
    #[must_use = "method returns a new number and does not mutate the original value"]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn log(self, base: f32) -> f32 {
        self.ln() / base.ln()
    }

    /// 返回该数的以 2 为底的对数。
    ///
    /// 当该数为负时返回 NaN，当该数为零时返回负无穷。
    ///
    /// # 未指定精度(Unspecified precision）
    ///
    /// 本函数的精度是不确定的。这意味着它会随平台、Rust 版本而变化，
    /// 甚至在同一次执行中，前后两次调用之间也可能不同。
    ///
    /// # 示例
    ///
    /// ```
    /// let two = 2.0f32;
    ///
    /// // log2(2) - 1 == 0
    /// let abs_difference = (two.log2() - 1.0).abs();
    ///
    /// assert!(abs_difference <= 1e-6);
    /// ```
    ///
    /// 非正值：
    /// ```
    /// assert_eq!(0_f32.log2(), f32::NEG_INFINITY);
    /// assert!((-42_f32).log2().is_nan());
    /// ```
    #[rustc_allow_incoherent_impl]
    #[must_use = "method returns a new number and does not mutate the original value"]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn log2(self) -> f32 {
        intrinsics::log2f32(self)
    }

    /// 返回该数的以 10 为底的对数。
    ///
    /// 当该数为负时返回 NaN，当该数为零时返回负无穷。
    ///
    /// # 未指定精度(Unspecified precision）
    ///
    /// 本函数的精度是不确定的。这意味着它会随平台、Rust 版本而变化，
    /// 甚至在同一次执行中，前后两次调用之间也可能不同。
    ///
    /// # 示例
    ///
    /// ```
    /// let ten = 10.0f32;
    ///
    /// // log10(10) - 1 == 0
    /// let abs_difference = (ten.log10() - 1.0).abs();
    ///
    /// assert!(abs_difference <= 1e-6);
    /// ```
    ///
    /// 非正值：
    /// ```
    /// assert_eq!(0_f32.log10(), f32::NEG_INFINITY);
    /// assert!((-42_f32).log10().is_nan());
    /// ```
    #[rustc_allow_incoherent_impl]
    #[must_use = "method returns a new number and does not mutate the original value"]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn log10(self) -> f32 {
        intrinsics::log10f32(self)
    }

    /// 两个数的正差值（positive difference）。
    ///
    /// * 若 `self <= other`：`0.0`
    /// * 否则：`self - other`
    ///
    /// # 未指定精度(Unspecified precision）
    ///
    /// 本函数的精度是不确定的。这意味着它会随平台、Rust 版本而变化，
    /// 甚至在同一次执行中，前后两次调用之间也可能不同。
    /// 本函数目前对应于 libc 的 `fdimf`（在 Unix
    /// 与 Windows 上）。注意这在将来可能会改变。
    ///
    /// # 示例
    ///
    /// ```
    /// let x = 3.0f32;
    /// let y = -3.0f32;
    ///
    /// let abs_difference_x = (x.abs_sub(1.0) - 2.0).abs();
    /// let abs_difference_y = (y.abs_sub(1.0) - 0.0).abs();
    ///
    /// assert!(abs_difference_x <= 1e-6);
    /// assert!(abs_difference_y <= 1e-6);
    /// ```
    #[rustc_allow_incoherent_impl]
    #[must_use = "method returns a new number and does not mutate the original value"]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    #[deprecated(
        since = "1.10.0",
        note = "you probably meant `(self - other).abs()`: \
                this operation is `(self - other).max(0.0)` \
                except that `abs_sub` also propagates NaNs (also \
                known as `fdimf` in C). If you truly need the positive \
                difference, consider using that expression or the C function \
                `fdimf`, depending on how you wish to handle NaN (please consider \
                filing an issue describing your use-case too)."
    )]
    pub fn abs_sub(self, other: f32) -> f32 {
        #[allow(deprecated)]
        core::f32::math::abs_sub(self, other)
    }

    /// 返回一个数的立方根。
    ///
    /// # 未指定精度(Unspecified precision）
    ///
    /// 本函数的精度是不确定的。这意味着它会随平台、Rust 版本而变化，
    /// 甚至在同一次执行中，前后两次调用之间也可能不同。
    /// 本函数目前对应于 libc 的 `cbrtf`（在 Unix
    /// 与 Windows 上）。注意这在将来可能会改变。
    ///
    /// # 示例
    ///
    /// ```
    /// let x = 8.0f32;
    ///
    /// // x^(1/3) - 2 == 0
    /// let abs_difference = (x.cbrt() - 2.0).abs();
    ///
    /// assert!(abs_difference <= 1e-6);
    /// ```
    #[rustc_allow_incoherent_impl]
    #[must_use = "method returns a new number and does not mutate the original value"]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn cbrt(self) -> f32 {
        core::f32::math::cbrt(self)
    }

    /// 计算原点到欧几里得平面上某点 (`x`, `y`) 的距离。
    /// 等价地说，即计算一个直角三角形斜边的长度，
    /// 该三角形的另外两条边长分别为 `x.abs()` 与
    /// `y.abs()`。
    ///
    /// # 未指定精度(Unspecified precision）
    ///
    /// 本函数的精度是不确定的。这意味着它会随平台、Rust 版本而变化，
    /// 甚至在同一次执行中，前后两次调用之间也可能不同。
    /// 本函数目前对应于 libc 的 `hypotf`（在 Unix
    /// 与 Windows 上）。注意这在将来可能会改变。
    ///
    /// # 示例
    ///
    /// ```
    /// let x = 2.0f32;
    /// let y = 3.0f32;
    ///
    /// // sqrt(x^2 + y^2)
    /// let abs_difference = (x.hypot(y) - (x.powi(2) + y.powi(2)).sqrt()).abs();
    ///
    /// assert!(abs_difference <= 1e-5);
    /// ```
    #[rustc_allow_incoherent_impl]
    #[must_use = "method returns a new number and does not mutate the original value"]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn hypot(self, other: f32) -> f32 {
        cmath::hypotf(self, other)
    }

    /// 计算一个数（以弧度为单位）的正弦。
    ///
    /// # 未指定精度(Unspecified precision）
    ///
    /// 本函数的精度是不确定的。这意味着它会随平台、Rust 版本而变化，
    /// 甚至在同一次执行中，前后两次调用之间也可能不同。
    ///
    /// # 示例
    ///
    /// ```
    /// let x = std::f32::consts::FRAC_PI_2;
    ///
    /// let abs_difference = (x.sin() - 1.0).abs();
    ///
    /// assert!(abs_difference <= 1e-6);
    /// ```
    #[rustc_allow_incoherent_impl]
    #[must_use = "method returns a new number and does not mutate the original value"]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn sin(self) -> f32 {
        intrinsics::sinf32(self)
    }

    /// 计算一个数（以弧度为单位）的余弦。
    ///
    /// # 未指定精度(Unspecified precision）
    ///
    /// 本函数的精度是不确定的。这意味着它会随平台、Rust 版本而变化，
    /// 甚至在同一次执行中，前后两次调用之间也可能不同。
    ///
    /// # 示例
    ///
    /// ```
    /// let x = 2.0 * std::f32::consts::PI;
    ///
    /// let abs_difference = (x.cos() - 1.0).abs();
    ///
    /// assert!(abs_difference <= 1e-6);
    /// ```
    #[rustc_allow_incoherent_impl]
    #[must_use = "method returns a new number and does not mutate the original value"]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn cos(self) -> f32 {
        intrinsics::cosf32(self)
    }

    /// 计算一个数（以弧度为单位）的正切。
    ///
    /// # 未指定精度(Unspecified precision）
    ///
    /// 本函数的精度是不确定的。这意味着它会随平台、Rust 版本而变化，
    /// 甚至在同一次执行中，前后两次调用之间也可能不同。
    /// 本函数目前对应于 libc 的 `tanf`（在 Unix 与
    /// Windows 上）。注意这在将来可能会改变。
    ///
    /// # 示例
    ///
    /// ```
    /// let x = std::f32::consts::FRAC_PI_4;
    /// let abs_difference = (x.tan() - 1.0).abs();
    ///
    /// assert!(abs_difference <= 1e-6);
    /// ```
    #[rustc_allow_incoherent_impl]
    #[must_use = "method returns a new number and does not mutate the original value"]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn tan(self) -> f32 {
        cmath::tanf(self)
    }

    /// 计算一个数的反正弦。返回值为弧度，
    /// 取值范围为 [-pi/2, pi/2]；如果该数落在 [-1, 1] 范围之外则为 NaN。
    /// [-1, 1].
    ///
    /// # 未指定精度(Unspecified precision）
    ///
    /// 本函数的精度是不确定的。这意味着它会随平台、Rust 版本而变化，
    /// 甚至在同一次执行中，前后两次调用之间也可能不同。
    /// 本函数目前对应于 libc 的 `asinf`（在 Unix
    /// 与 Windows 上）。注意这在将来可能会改变。
    ///
    /// # 示例
    ///
    /// ```
    /// let f = std::f32::consts::FRAC_PI_4;
    ///
    /// // asin(sin(pi/2))
    /// let abs_difference = (f.sin().asin() - f).abs();
    ///
    /// assert!(abs_difference <= 1e-6);
    /// ```
    #[doc(alias = "arcsin")]
    #[rustc_allow_incoherent_impl]
    #[must_use = "method returns a new number and does not mutate the original value"]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn asin(self) -> f32 {
        cmath::asinf(self)
    }

    /// 计算一个数的反余弦。返回值为弧度，
    /// 取值范围为 [0, pi]；如果该数落在 [-1, 1] 范围之外则为 NaN。
    /// [-1, 1].
    ///
    /// # 未指定精度(Unspecified precision）
    ///
    /// 本函数的精度是不确定的。这意味着它会随平台、Rust 版本而变化，
    /// 甚至在同一次执行中，前后两次调用之间也可能不同。
    /// 本函数目前对应于 libc 的 `acosf`（在 Unix
    /// 与 Windows 上）。注意这在将来可能会改变。
    ///
    /// # 示例
    ///
    /// ```
    /// let f = std::f32::consts::FRAC_PI_4;
    ///
    /// // acos(cos(pi/4))
    /// let abs_difference = (f.cos().acos() - std::f32::consts::FRAC_PI_4).abs();
    ///
    /// assert!(abs_difference <= 1e-6);
    /// ```
    #[doc(alias = "arccos")]
    #[rustc_allow_incoherent_impl]
    #[must_use = "method returns a new number and does not mutate the original value"]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn acos(self) -> f32 {
        cmath::acosf(self)
    }

    /// 计算一个数的反正切。返回值为弧度，
    /// 取值范围为 [-pi/2, pi/2]；
    ///
    /// # 未指定精度(Unspecified precision）
    ///
    /// 本函数的精度是不确定的。这意味着它会随平台、Rust 版本而变化，
    /// 甚至在同一次执行中，前后两次调用之间也可能不同。
    /// 本函数目前对应于 libc 的 `atanf`（在 Unix
    /// 与 Windows 上）。注意这在将来可能会改变。
    ///
    /// # 示例
    ///
    /// ```
    /// let f = 1.0f32;
    ///
    /// // atan(tan(1))
    /// let abs_difference = (f.tan().atan() - 1.0).abs();
    ///
    /// assert!(abs_difference <= 1e-6);
    /// ```
    #[doc(alias = "arctan")]
    #[rustc_allow_incoherent_impl]
    #[must_use = "method returns a new number and does not mutate the original value"]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn atan(self) -> f32 {
        cmath::atanf(self)
    }

    /// 以弧度为单位计算 `self`（`y`）与 `other`（`x`）的四象限反正切。
    ///
    ///  | `x`     | `y`     | 分段定义             | 范围          |
    ///  |---------|---------|----------------------|---------------|
    ///  | `>= +0` | `>= +0` | `arctan(y/x)`        | `[+0, +pi/2]` |
    ///  | `>= +0` | `<= -0` | `arctan(y/x)`        | `[-pi/2, -0]` |
    ///  | `<= -0` | `>= +0` | `arctan(y/x) + pi`   | `[+pi/2, +pi]`|
    ///  | `<= -0` | `<= -0` | `arctan(y/x) - pi`   | `[-pi, -pi/2]`|
    ///
    /// # 未指定精度(Unspecified precision）
    ///
    /// 本函数的精度是不确定的。这意味着它会随平台、Rust 版本而变化，
    /// 甚至在同一次执行中，前后两次调用之间也可能不同。
    /// 本函数目前对应于 libc 的 `atan2f`（在 Unix
    /// 与 Windows 上）。注意这在将来可能会改变。
    ///
    /// # 示例
    ///
    /// ```
    /// // 正角度从正 x 轴起
    /// // （逆时针为正）
    /// // -pi/4 弧度（顺时针 45 度）
    /// let x1 = 3.0f32;
    /// let y1 = -3.0f32;
    ///
    /// // 3pi/4 弧度（逆时针 135 度）
    /// let x2 = -3.0f32;
    /// let y2 = 3.0f32;
    ///
    /// let abs_difference_1 = (y1.atan2(x1) - (-std::f32::consts::FRAC_PI_4)).abs();
    /// let abs_difference_2 = (y2.atan2(x2) - (3.0 * std::f32::consts::FRAC_PI_4)).abs();
    ///
    /// assert!(abs_difference_1 <= 1e-5);
    /// assert!(abs_difference_2 <= 1e-5);
    /// ```
    #[rustc_allow_incoherent_impl]
    #[must_use = "method returns a new number and does not mutate the original value"]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn atan2(self, other: f32) -> f32 {
        cmath::atan2f(self, other)
    }

    /// 同时计算数 `x` 的正弦与余弦，返回
    /// `(sin(x), cos(x))`。
    ///
    /// # 未指定精度(Unspecified precision）
    ///
    /// 本函数的精度是不确定的。这意味着它会随平台、Rust 版本而变化，
    /// 甚至在同一次执行中，前后两次调用之间也可能不同。
    /// 本函数目前对应于 `(f32::sin(x),
    /// f32::cos(x))`。注意这在将来可能会改变。
    ///
    /// # 示例
    ///
    /// ```
    /// let x = std::f32::consts::FRAC_PI_4;
    /// let f = x.sin_cos();
    ///
    /// let abs_difference_0 = (f.0 - x.sin()).abs();
    /// let abs_difference_1 = (f.1 - x.cos()).abs();
    ///
    /// assert!(abs_difference_0 <= 1e-4);
    /// assert!(abs_difference_1 <= 1e-4);
    /// ```
    #[doc(alias = "sincos")]
    #[rustc_allow_incoherent_impl]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn sin_cos(self) -> (f32, f32) {
        (self.sin(), self.cos())
    }

    /// 以一种即便在数值接近零时也保持精确的方式返回 `e^(self) - 1`。
    /// ——也就是说，即便该数接近零也保持精确。
    ///
    /// # 未指定精度(Unspecified precision）
    ///
    /// 本函数的精度是不确定的。这意味着它会随平台、Rust 版本而变化，
    /// 甚至在同一次执行中，前后两次调用之间也可能不同。
    /// 本函数目前对应于 libc 的 `expm1f`（在 Unix
    /// 与 Windows 上）。注意这在将来可能会改变。
    ///
    /// # 示例
    ///
    /// ```
    /// let x = 1e-8_f32;
    ///
    /// // 对于非常小的 x，e^x 近似等于 1 + x + x^2 / 2
    /// let approx = x + x * x / 2.0;
    /// let abs_difference = (x.exp_m1() - approx).abs();
    ///
    /// assert!(abs_difference < 1e-10);
    /// ```
    #[rustc_allow_incoherent_impl]
    #[must_use = "method returns a new number and does not mutate the original value"]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn exp_m1(self) -> f32 {
        cmath::expm1f(self)
    }

    /// 返回 `ln(1+n)`（自然对数），其精度高于分别执行各步运算的结果。
    /// 其精度高于分别执行各步运算的结果。
    ///
    /// 当 `n < -1.0` 时返回 NaN，当 `n == -1.0` 时返回负无穷。
    ///
    /// # 未指定精度(Unspecified precision）
    ///
    /// 本函数的精度是不确定的。这意味着它会随平台、Rust 版本而变化，
    /// 甚至在同一次执行中，前后两次调用之间也可能不同。
    /// 本函数目前对应于 libc 的 `log1pf`（在 Unix
    /// 与 Windows 上）。注意这在将来可能会改变。
    ///
    /// # 示例
    ///
    /// ```
    /// let x = 1e-8_f32;
    ///
    /// // 对于非常小的 x，ln(1 + x) 近似等于 x - x^2 / 2
    /// let approx = x - x * x / 2.0;
    /// let abs_difference = (x.ln_1p() - approx).abs();
    ///
    /// assert!(abs_difference < 1e-10);
    /// ```
    ///
    /// 超出范围的值：
    /// ```
    /// assert_eq!((-1.0_f32).ln_1p(), f32::NEG_INFINITY);
    /// assert!((-2.0_f32).ln_1p().is_nan());
    /// ```
    #[doc(alias = "log1p")]
    #[rustc_allow_incoherent_impl]
    #[must_use = "method returns a new number and does not mutate the original value"]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn ln_1p(self) -> f32 {
        cmath::log1pf(self)
    }

    /// 双曲正弦函数。
    ///
    /// # 未指定精度(Unspecified precision）
    ///
    /// 本函数的精度是不确定的。这意味着它会随平台、Rust 版本而变化，
    /// 甚至在同一次执行中，前后两次调用之间也可能不同。
    /// 本函数目前对应于 libc 的 `sinhf`（在 Unix
    /// 与 Windows 上）。注意这在将来可能会改变。
    ///
    /// # 示例
    ///
    /// ```
    /// let e = std::f32::consts::E;
    /// let x = 1.0f32;
    ///
    /// let f = x.sinh();
    /// // 在 1 处求 sinh() 得到 `(e^2-1)/(2e)`
    /// let g = ((e * e) - 1.0) / (2.0 * e);
    /// let abs_difference = (f - g).abs();
    ///
    /// assert!(abs_difference <= 1e-6);
    /// ```
    #[rustc_allow_incoherent_impl]
    #[must_use = "method returns a new number and does not mutate the original value"]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn sinh(self) -> f32 {
        cmath::sinhf(self)
    }

    /// 双曲余弦函数。
    ///
    /// # 未指定精度(Unspecified precision）
    ///
    /// 本函数的精度是不确定的。这意味着它会随平台、Rust 版本而变化，
    /// 甚至在同一次执行中，前后两次调用之间也可能不同。
    /// 本函数目前对应于 libc 的 `coshf`（在 Unix
    /// 与 Windows 上）。注意这在将来可能会改变。
    ///
    /// # 示例
    ///
    /// ```
    /// let e = std::f32::consts::E;
    /// let x = 1.0f32;
    /// let f = x.cosh();
    /// // 在 1 处求 cosh() 得到此结果
    /// let g = ((e * e) + 1.0) / (2.0 * e);
    /// let abs_difference = (f - g).abs();
    ///
    /// // 结果相同
    /// assert!(abs_difference <= 1e-6);
    /// ```
    #[rustc_allow_incoherent_impl]
    #[must_use = "method returns a new number and does not mutate the original value"]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn cosh(self) -> f32 {
        cmath::coshf(self)
    }

    /// 双曲正切函数。
    ///
    /// # 未指定精度(Unspecified precision）
    ///
    /// 本函数的精度是不确定的。这意味着它会随平台、Rust 版本而变化，
    /// 甚至在同一次执行中，前后两次调用之间也可能不同。
    /// 本函数目前对应于 libc 的 `tanhf`（在 Unix
    /// 与 Windows 上）。注意这在将来可能会改变。
    ///
    /// # 示例
    ///
    /// ```
    /// let e = std::f32::consts::E;
    /// let x = 1.0f32;
    ///
    /// let f = x.tanh();
    /// // 在 1 处求 tanh() 得到 `(1 - e^(-2))/(1 + e^(-2))`
    /// let g = (1.0 - e.powi(-2)) / (1.0 + e.powi(-2));
    /// let abs_difference = (f - g).abs();
    ///
    /// assert!(abs_difference <= 1e-6);
    /// ```
    #[rustc_allow_incoherent_impl]
    #[must_use = "method returns a new number and does not mutate the original value"]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn tanh(self) -> f32 {
        cmath::tanhf(self)
    }

    /// 反双曲正弦函数。
    ///
    /// # 未指定精度(Unspecified precision）
    ///
    /// 本函数的精度是不确定的。这意味着它会随平台、Rust 版本而变化，
    /// 甚至在同一次执行中，前后两次调用之间也可能不同。
    ///
    /// # 示例
    ///
    /// ```
    /// let x = 1.0f32;
    /// let f = x.sinh().asinh();
    ///
    /// let abs_difference = (f - x).abs();
    ///
    /// assert!(abs_difference <= 1e-6);
    /// ```
    #[doc(alias = "arcsinh")]
    #[rustc_allow_incoherent_impl]
    #[must_use = "method returns a new number and does not mutate the original value"]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn asinh(self) -> f32 {
        let ax = self.abs();
        let ix = 1.0 / ax;
        (ax + (ax / (Self::hypot(1.0, ix) + ix))).ln_1p().copysign(self)
    }

    /// 反双曲余弦函数。
    ///
    /// # 未指定精度(Unspecified precision）
    ///
    /// 本函数的精度是不确定的。这意味着它会随平台、Rust 版本而变化，
    /// 甚至在同一次执行中，前后两次调用之间也可能不同。
    ///
    /// # 示例
    ///
    /// ```
    /// let x = 1.0f32;
    /// let f = x.cosh().acosh();
    ///
    /// let abs_difference = (f - x).abs();
    ///
    /// assert!(abs_difference <= 1e-6);
    /// ```
    #[doc(alias = "arccosh")]
    #[rustc_allow_incoherent_impl]
    #[must_use = "method returns a new number and does not mutate the original value"]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn acosh(self) -> f32 {
        if self < 1.0 {
            Self::NAN
        } else {
            (self + ((self - 1.0).sqrt() * (self + 1.0).sqrt())).ln()
        }
    }

    /// 反双曲正切函数。
    ///
    /// # 未指定精度(Unspecified precision）
    ///
    /// 本函数的精度是不确定的。这意味着它会随平台、Rust 版本而变化，
    /// 甚至在同一次执行中，前后两次调用之间也可能不同。
    ///
    /// # 示例
    ///
    /// ```
    /// let x = std::f32::consts::FRAC_PI_6;
    /// let f = x.tanh().atanh();
    ///
    /// let abs_difference = (f - x).abs();
    ///
    /// assert!(abs_difference <= 1e-5);
    /// ```
    #[doc(alias = "arctanh")]
    #[rustc_allow_incoherent_impl]
    #[must_use = "method returns a new number and does not mutate the original value"]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn atanh(self) -> f32 {
        0.5 * ((2.0 * self) / (1.0 - self)).ln_1p()
    }

    /// 伽马函数（gamma function）。
    ///
    /// # 未指定精度(Unspecified precision）
    ///
    /// 本函数的精度是不确定的。这意味着它会随平台、Rust 版本而变化，
    /// 甚至在同一次执行中，前后两次调用之间也可能不同。
    /// 本函数目前对应于 libc 的 `tgammaf`（在 Unix
    /// 与 Windows 上）。注意这在将来可能会改变。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(float_gamma)]
    /// let x = 5.0f32;
    ///
    /// let abs_difference = (x.gamma() - 24.0).abs();
    ///
    /// assert!(abs_difference <= 1e-5);
    /// ```
    #[rustc_allow_incoherent_impl]
    #[must_use = "method returns a new number and does not mutate the original value"]
    #[unstable(feature = "float_gamma", issue = "99842")]
    #[inline]
    pub fn gamma(self) -> f32 {
        cmath::tgammaf(self)
    }

    /// 伽马函数绝对值的自然对数。
    ///
    /// 元组中的整数部分表示伽马函数的符号。
    ///
    /// # 未指定精度(Unspecified precision）
    ///
    /// 本函数的精度是不确定的。这意味着它会随平台、Rust 版本而变化，
    /// 甚至在同一次执行中，前后两次调用之间也可能不同。
    /// 本函数目前对应于 libc 的 `lgamma_r`（在 Unix
    /// 与 Windows 上）。注意这在将来可能会改变。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(float_gamma)]
    /// let x = 2.0f32;
    ///
    /// let abs_difference = (x.ln_gamma().0 - 0.0).abs();
    ///
    /// assert!(abs_difference <= f32::EPSILON);
    /// ```
    #[rustc_allow_incoherent_impl]
    #[must_use = "method returns a new number and does not mutate the original value"]
    #[unstable(feature = "float_gamma", issue = "99842")]
    #[inline]
    pub fn ln_gamma(self) -> (f32, i32) {
        let mut signgamp: i32 = 0;
        let x = cmath::lgammaf_r(self, &mut signgamp);
        (x, signgamp)
    }

    /// 误差函数（error function）。
    ///
    /// # 未指定精度(Unspecified precision）
    ///
    /// 本函数的精度是不确定的。这意味着它会随平台、
    /// Rust 版本而变化，甚至在同一次执行中，前后两次调用之间也可能不同。
    ///
    /// 本函数目前对应于 libc 的 `erff`（在 Unix
    /// 与 Windows 上）。注意这在将来可能会改变。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(float_erf)]
    /// /// 误差函数描述了正态分布中有百分之多少落在
    /// /// `x` 个标准差以内（按 `1/sqrt(2)` 缩放）。
    /// fn within_standard_deviations(x: f32) -> f32 {
    ///     (x * std::f32::consts::FRAC_1_SQRT_2).erf() * 100.0
    /// }
    ///
    /// // 正态分布中有 68% 落在一个标准差以内
    /// assert!((within_standard_deviations(1.0) - 68.269).abs() < 0.01);
    /// // 正态分布中有 95% 落在两个标准差以内
    /// assert!((within_standard_deviations(2.0) - 95.450).abs() < 0.01);
    /// // 正态分布中有 99.7% 落在三个标准差以内
    /// assert!((within_standard_deviations(3.0) - 99.730).abs() < 0.01);
    /// ```
    #[rustc_allow_incoherent_impl]
    #[must_use = "method returns a new number and does not mutate the original value"]
    #[unstable(feature = "float_erf", issue = "136321")]
    #[inline]
    pub fn erf(self) -> f32 {
        cmath::erff(self)
    }

    /// 互补误差函数（complementary error function）。
    ///
    /// # 未指定精度(Unspecified precision）
    ///
    /// 本函数的精度是不确定的。这意味着它会随平台、
    /// Rust 版本而变化，甚至在同一次执行中，前后两次调用之间也可能不同。
    ///
    /// 本函数目前对应于 libc 的 `erfcf`（在 Unix
    /// 与 Windows 上）。注意这在将来可能会改变。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(float_erf)]
    /// let x: f32 = 0.123;
    ///
    /// let one = x.erf() + x.erfc();
    /// let abs_difference = (one - 1.0).abs();
    ///
    /// assert!(abs_difference <= 1e-6);
    /// ```
    #[rustc_allow_incoherent_impl]
    #[must_use = "method returns a new number and does not mutate the original value"]
    #[unstable(feature = "float_erf", issue = "136321")]
    #[inline]
    pub fn erfc(self) -> f32 {
        cmath::erfcf(self)
    }
}
