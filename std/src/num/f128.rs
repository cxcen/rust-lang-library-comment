//! `f128` 四精度（quadruple-precision）浮点类型的相关常量。
//!
//! *[另见 `f128` 原始类型](primitive@f128)。*
//!
//! 具有数学意义的重要数值由 `consts` 子模块提供。
//!
//! 实现说明：本模块在 std 而非 core 中提供 `f128` 的浮点数学方法（如 `sqrt`、
//! `sin`、`exp`、`ln`、`hypot` 等），是因为这些函数的实现依赖编译器内建函数
//! （intrinsics）或底层平台/系统的数学库（在 Unix 与 Windows 上即 libc/libm）。
//! 这类运行时依赖在不依赖操作系统的 core 中无法满足，因此只有 std 才能提供它们；
//! 而那些不依赖运行时、纯位运算即可完成的方法则定义在 core 上。

#![unstable(feature = "f128", issue = "116909")]
#![doc(test(attr(feature(cfg_target_has_reliable_f16_f128), expect(internal_features))))]

#[unstable(feature = "f128", issue = "116909")]
pub use core::f128::consts;

#[cfg(not(test))]
use crate::intrinsics;
#[cfg(not(test))]
use crate::sys::cmath;

#[cfg(not(test))]
impl f128 {
    /// 计算一个数的浮点数次幂。
    ///
    /// 注意本函数较为特殊：对于 NaN 输入它也可能返回非 NaN 的结果。例如，
    /// 例如，`f128::powf(f128::NAN, 0.0)` 返回 `1.0`。但是，如果某个输入是一个 *signaling*（信号）
    /// NaN（即 *signaling* NaN），则结果不确定：要么是 NaN，要么是对应的安静（quiet）
    /// NaN 所产生的结果。
    ///
    /// # 未指定精度(Unspecified precision）
    ///
    /// 本函数的精度是不确定的。这意味着它会随平台、
    /// Rust 版本而变化，甚至在同一次执行中，前后两次调用之间也可能不同。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(f128)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f128_math)] {
    ///
    /// let x = 2.0_f128;
    /// let abs_difference = (x.powf(2.0) - (x * x)).abs();
    /// assert!(abs_difference <= f128::EPSILON);
    ///
    /// assert_eq!(f128::powf(1.0, f128::NAN), 1.0);
    /// assert_eq!(f128::powf(f128::NAN, 0.0), 1.0);
    /// assert_eq!(f128::powf(0.0, 0.0), 1.0);
    /// # }
    /// ```
    #[inline]
    #[rustc_allow_incoherent_impl]
    #[unstable(feature = "f128", issue = "116909")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    pub fn powf(self, n: f128) -> f128 {
        intrinsics::powf128(self, n)
    }

    /// 返回 `e^(self)`（即指数函数）。
    ///
    /// # 未指定精度(Unspecified precision）
    ///
    /// 本函数的精度是不确定的。这意味着它会随平台、
    /// Rust 版本而变化，甚至在同一次执行中，前后两次调用之间也可能不同。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(f128)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f128_math)] {
    ///
    /// let one = 1.0f128;
    /// // e^1
    /// let e = one.exp();
    ///
    /// // ln(e) - 1 == 0
    /// let abs_difference = (e.ln() - 1.0).abs();
    ///
    /// assert!(abs_difference <= f128::EPSILON);
    /// # }
    /// ```
    #[inline]
    #[rustc_allow_incoherent_impl]
    #[unstable(feature = "f128", issue = "116909")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    pub fn exp(self) -> f128 {
        intrinsics::expf128(self)
    }

    /// 返回 `2^(self)`。
    ///
    /// # 未指定精度(Unspecified precision）
    ///
    /// 本函数的精度是不确定的。这意味着它会随平台、
    /// Rust 版本而变化，甚至在同一次执行中，前后两次调用之间也可能不同。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(f128)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f128_math)] {
    ///
    /// let f = 2.0f128;
    ///
    /// // 2^2 - 4 == 0
    /// let abs_difference = (f.exp2() - 4.0).abs();
    ///
    /// assert!(abs_difference <= f128::EPSILON);
    /// # }
    /// ```
    #[inline]
    #[rustc_allow_incoherent_impl]
    #[unstable(feature = "f128", issue = "116909")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    pub fn exp2(self) -> f128 {
        intrinsics::exp2f128(self)
    }

    /// 返回该数的自然对数。
    ///
    /// 当该数为负时返回 NaN，当该数为零时返回负无穷。
    ///
    /// # 未指定精度(Unspecified precision）
    ///
    /// 本函数的精度是不确定的。这意味着它会随平台、
    /// Rust 版本而变化，甚至在同一次执行中，前后两次调用之间也可能不同。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(f128)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f128_math)] {
    ///
    /// let one = 1.0f128;
    /// // e^1
    /// let e = one.exp();
    ///
    /// // ln(e) - 1 == 0
    /// let abs_difference = (e.ln() - 1.0).abs();
    ///
    /// assert!(abs_difference <= f128::EPSILON);
    /// # }
    /// ```
    ///
    /// 非正值：
    /// ```
    /// #![feature(f128)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f128_math)] {
    ///
    /// assert_eq!(0_f128.ln(), f128::NEG_INFINITY);
    /// assert!((-42_f128).ln().is_nan());
    /// # }
    /// ```
    #[inline]
    #[rustc_allow_incoherent_impl]
    #[unstable(feature = "f128", issue = "116909")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    pub fn ln(self) -> f128 {
        intrinsics::logf128(self)
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
    /// 本函数的精度是不确定的。这意味着它会随平台、
    /// Rust 版本而变化，甚至在同一次执行中，前后两次调用之间也可能不同。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(f128)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f128_math)] {
    ///
    /// let five = 5.0f128;
    ///
    /// // log5(5) - 1 == 0
    /// let abs_difference = (five.log(5.0) - 1.0).abs();
    ///
    /// assert!(abs_difference <= f128::EPSILON);
    /// # }
    /// ```
    ///
    /// 非正值：
    /// ```
    /// #![feature(f128)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f128_math)] {
    ///
    /// assert_eq!(0_f128.log(10.0), f128::NEG_INFINITY);
    /// assert!((-42_f128).log(10.0).is_nan());
    /// # }
    /// ```
    #[inline]
    #[rustc_allow_incoherent_impl]
    #[unstable(feature = "f128", issue = "116909")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    pub fn log(self, base: f128) -> f128 {
        self.ln() / base.ln()
    }

    /// 返回该数的以 2 为底的对数。
    ///
    /// 当该数为负时返回 NaN，当该数为零时返回负无穷。
    ///
    /// # 未指定精度(Unspecified precision）
    ///
    /// 本函数的精度是不确定的。这意味着它会随平台、
    /// Rust 版本而变化，甚至在同一次执行中，前后两次调用之间也可能不同。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(f128)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f128_math)] {
    ///
    /// let two = 2.0f128;
    ///
    /// // log2(2) - 1 == 0
    /// let abs_difference = (two.log2() - 1.0).abs();
    ///
    /// assert!(abs_difference <= f128::EPSILON);
    /// # }
    /// ```
    ///
    /// 非正值：
    /// ```
    /// #![feature(f128)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f128_math)] {
    ///
    /// assert_eq!(0_f128.log2(), f128::NEG_INFINITY);
    /// assert!((-42_f128).log2().is_nan());
    /// # }
    /// ```
    #[inline]
    #[rustc_allow_incoherent_impl]
    #[unstable(feature = "f128", issue = "116909")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    pub fn log2(self) -> f128 {
        intrinsics::log2f128(self)
    }

    /// 返回该数的以 10 为底的对数。
    ///
    /// 当该数为负时返回 NaN，当该数为零时返回负无穷。
    ///
    /// # 未指定精度(Unspecified precision）
    ///
    /// 本函数的精度是不确定的。这意味着它会随平台、
    /// Rust 版本而变化，甚至在同一次执行中，前后两次调用之间也可能不同。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(f128)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f128_math)] {
    ///
    /// let ten = 10.0f128;
    ///
    /// // log10(10) - 1 == 0
    /// let abs_difference = (ten.log10() - 1.0).abs();
    ///
    /// assert!(abs_difference <= f128::EPSILON);
    /// # }
    /// ```
    ///
    /// 非正值：
    /// ```
    /// #![feature(f128)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f128_math)] {
    ///
    /// assert_eq!(0_f128.log10(), f128::NEG_INFINITY);
    /// assert!((-42_f128).log10().is_nan());
    /// # }
    /// ```
    #[inline]
    #[rustc_allow_incoherent_impl]
    #[unstable(feature = "f128", issue = "116909")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    pub fn log10(self) -> f128 {
        intrinsics::log10f128(self)
    }

    /// 返回一个数的立方根。
    ///
    /// # 未指定精度(Unspecified precision）
    ///
    /// 本函数的精度是不确定的。这意味着它会随平台、
    /// Rust 版本而变化，甚至在同一次执行中，前后两次调用之间也可能不同。
    ///
    ///
    /// 本函数目前对应于 libc 的 `cbrtf128`（在 Unix
    /// 与 Windows 上）。注意这在将来可能会改变。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(f128)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f128_math)] {
    ///
    /// let x = 8.0f128;
    ///
    /// // x^(1/3) - 2 == 0
    /// let abs_difference = (x.cbrt() - 2.0).abs();
    ///
    /// assert!(abs_difference <= f128::EPSILON);
    /// # }
    /// ```
    #[inline]
    #[rustc_allow_incoherent_impl]
    #[unstable(feature = "f128", issue = "116909")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    pub fn cbrt(self) -> f128 {
        cmath::cbrtf128(self)
    }

    /// 计算原点到欧几里得平面上某点 (`x`, `y`) 的距离。
    /// 等价地说，即计算一个直角三角形斜边的长度，
    /// 该三角形的另外两条边长分别为 `x.abs()` 与
    /// `y.abs()`。
    ///
    /// # 未指定精度(Unspecified precision）
    ///
    /// 本函数的精度是不确定的。这意味着它会随平台、
    /// Rust 版本而变化，甚至在同一次执行中，前后两次调用之间也可能不同。
    ///
    ///
    /// 本函数目前对应于 libc 的 `hypotf128`（在 Unix
    /// 与 Windows 上）。注意这在将来可能会改变。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(f128)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f128_math)] {
    ///
    /// let x = 2.0f128;
    /// let y = 3.0f128;
    ///
    /// // sqrt(x^2 + y^2)
    /// let abs_difference = (x.hypot(y) - (x.powi(2) + y.powi(2)).sqrt()).abs();
    ///
    /// assert!(abs_difference <= f128::EPSILON);
    /// # }
    /// ```
    #[inline]
    #[rustc_allow_incoherent_impl]
    #[unstable(feature = "f128", issue = "116909")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    pub fn hypot(self, other: f128) -> f128 {
        cmath::hypotf128(self, other)
    }

    /// 计算一个数（以弧度为单位）的正弦。
    ///
    /// # 未指定精度(Unspecified precision）
    ///
    /// 本函数的精度是不确定的。这意味着它会随平台、
    /// Rust 版本而变化，甚至在同一次执行中，前后两次调用之间也可能不同。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(f128)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f128_math)] {
    ///
    /// let x = std::f128::consts::FRAC_PI_2;
    ///
    /// let abs_difference = (x.sin() - 1.0).abs();
    ///
    /// assert!(abs_difference <= f128::EPSILON);
    /// # }
    /// ```
    #[inline]
    #[rustc_allow_incoherent_impl]
    #[unstable(feature = "f128", issue = "116909")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    pub fn sin(self) -> f128 {
        intrinsics::sinf128(self)
    }

    /// 计算一个数（以弧度为单位）的余弦。
    ///
    /// # 未指定精度(Unspecified precision）
    ///
    /// 本函数的精度是不确定的。这意味着它会随平台、
    /// Rust 版本而变化，甚至在同一次执行中，前后两次调用之间也可能不同。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(f128)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f128_math)] {
    ///
    /// let x = 2.0 * std::f128::consts::PI;
    ///
    /// let abs_difference = (x.cos() - 1.0).abs();
    ///
    /// assert!(abs_difference <= f128::EPSILON);
    /// # }
    /// ```
    #[inline]
    #[rustc_allow_incoherent_impl]
    #[unstable(feature = "f128", issue = "116909")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    pub fn cos(self) -> f128 {
        intrinsics::cosf128(self)
    }

    /// 计算一个数（以弧度为单位）的正切。
    ///
    /// # 未指定精度(Unspecified precision）
    ///
    /// 本函数的精度是不确定的。这意味着它会随平台、
    /// Rust 版本而变化，甚至在同一次执行中，前后两次调用之间也可能不同。
    ///
    /// 本函数目前对应于 libc 的 `tanf128`（在 Unix 与
    /// Windows 上）。注意这在将来可能会改变。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(f128)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f128_math)] {
    ///
    /// let x = std::f128::consts::FRAC_PI_4;
    /// let abs_difference = (x.tan() - 1.0).abs();
    ///
    /// assert!(abs_difference <= f128::EPSILON);
    /// # }
    /// ```
    #[inline]
    #[rustc_allow_incoherent_impl]
    #[unstable(feature = "f128", issue = "116909")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    pub fn tan(self) -> f128 {
        cmath::tanf128(self)
    }

    /// 计算一个数的反正弦。返回值为弧度，
    /// 取值范围为 [-pi/2, pi/2]；如果该数落在 [-1, 1] 范围之外则为 NaN。
    /// [-1, 1].
    ///
    /// # 未指定精度(Unspecified precision）
    ///
    /// 本函数的精度是不确定的。这意味着它会随平台、
    /// Rust 版本而变化，甚至在同一次执行中，前后两次调用之间也可能不同。
    ///
    /// 本函数目前对应于 libc 的 `asinf128`（在 Unix
    /// 与 Windows 上）。注意这在将来可能会改变。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(f128)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f128_math)] {
    ///
    /// let f = std::f128::consts::FRAC_PI_4;
    ///
    /// // asin(sin(pi/2))
    /// let abs_difference = (f.sin().asin() - f).abs();
    ///
    /// assert!(abs_difference <= f128::EPSILON);
    /// # }
    /// ```
    #[inline]
    #[doc(alias = "arcsin")]
    #[rustc_allow_incoherent_impl]
    #[unstable(feature = "f128", issue = "116909")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    pub fn asin(self) -> f128 {
        cmath::asinf128(self)
    }

    /// 计算一个数的反余弦。返回值为弧度，
    /// 取值范围为 [0, pi]；如果该数落在 [-1, 1] 范围之外则为 NaN。
    /// [-1, 1].
    ///
    /// # 未指定精度(Unspecified precision）
    ///
    /// 本函数的精度是不确定的。这意味着它会随平台、
    /// Rust 版本而变化，甚至在同一次执行中，前后两次调用之间也可能不同。
    ///
    /// 本函数目前对应于 libc 的 `acosf128`（在 Unix
    /// 与 Windows 上）。注意这在将来可能会改变。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(f128)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f128_math)] {
    ///
    /// let f = std::f128::consts::FRAC_PI_4;
    ///
    /// // acos(cos(pi/4))
    /// let abs_difference = (f.cos().acos() - std::f128::consts::FRAC_PI_4).abs();
    ///
    /// assert!(abs_difference <= f128::EPSILON);
    /// # }
    /// ```
    #[inline]
    #[doc(alias = "arccos")]
    #[rustc_allow_incoherent_impl]
    #[unstable(feature = "f128", issue = "116909")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    pub fn acos(self) -> f128 {
        cmath::acosf128(self)
    }

    /// 计算一个数的反正切。返回值为弧度，
    /// 取值范围为 [-pi/2, pi/2]；
    ///
    /// # 未指定精度(Unspecified precision）
    ///
    /// 本函数的精度是不确定的。这意味着它会随平台、
    /// Rust 版本而变化，甚至在同一次执行中，前后两次调用之间也可能不同。
    ///
    /// 本函数目前对应于 libc 的 `atanf128`（在 Unix
    /// 与 Windows 上）。注意这在将来可能会改变。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(f128)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f128_math)] {
    ///
    /// let f = 1.0f128;
    ///
    /// // atan(tan(1))
    /// let abs_difference = (f.tan().atan() - 1.0).abs();
    ///
    /// assert!(abs_difference <= f128::EPSILON);
    /// # }
    /// ```
    #[inline]
    #[doc(alias = "arctan")]
    #[rustc_allow_incoherent_impl]
    #[unstable(feature = "f128", issue = "116909")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    pub fn atan(self) -> f128 {
        cmath::atanf128(self)
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
    /// 本函数的精度是不确定的。这意味着它会随平台、
    /// Rust 版本而变化，甚至在同一次执行中，前后两次调用之间也可能不同。
    ///
    /// 本函数目前对应于 libc 的 `atan2f128`（在 Unix
    /// 与 Windows 上）。注意这在将来可能会改变。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(f128)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f128_math)] {
    ///
    /// // 正角度从正 x 轴起
    /// // （逆时针为正）
    /// // -pi/4 弧度（顺时针 45 度）
    /// let x1 = 3.0f128;
    /// let y1 = -3.0f128;
    ///
    /// // 3pi/4 弧度（逆时针 135 度）
    /// let x2 = -3.0f128;
    /// let y2 = 3.0f128;
    ///
    /// let abs_difference_1 = (y1.atan2(x1) - (-std::f128::consts::FRAC_PI_4)).abs();
    /// let abs_difference_2 = (y2.atan2(x2) - (3.0 * std::f128::consts::FRAC_PI_4)).abs();
    ///
    /// assert!(abs_difference_1 <= f128::EPSILON);
    /// assert!(abs_difference_2 <= f128::EPSILON);
    /// # }
    /// ```
    #[inline]
    #[rustc_allow_incoherent_impl]
    #[unstable(feature = "f128", issue = "116909")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    pub fn atan2(self, other: f128) -> f128 {
        cmath::atan2f128(self, other)
    }

    /// 同时计算数 `x` 的正弦与余弦，返回
    /// `(sin(x), cos(x))`。
    ///
    /// # 未指定精度(Unspecified precision）
    ///
    /// 本函数的精度是不确定的。这意味着它会随平台、
    /// Rust 版本而变化，甚至在同一次执行中，前后两次调用之间也可能不同。
    ///
    /// 本函数目前对应于 `(f128::sin(x),
    /// f128::cos(x))`。注意这在将来可能会改变。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(f128)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f128_math)] {
    ///
    /// let x = std::f128::consts::FRAC_PI_4;
    /// let f = x.sin_cos();
    ///
    /// let abs_difference_0 = (f.0 - x.sin()).abs();
    /// let abs_difference_1 = (f.1 - x.cos()).abs();
    ///
    /// assert!(abs_difference_0 <= f128::EPSILON);
    /// assert!(abs_difference_1 <= f128::EPSILON);
    /// # }
    /// ```
    #[inline]
    #[doc(alias = "sincos")]
    #[rustc_allow_incoherent_impl]
    #[unstable(feature = "f128", issue = "116909")]
    pub fn sin_cos(self) -> (f128, f128) {
        (self.sin(), self.cos())
    }

    /// 以一种即便在数值接近零时也保持精确的方式返回 `e^(self) - 1`。
    /// ——也就是说，即便该数接近零也保持精确。
    ///
    /// # 未指定精度(Unspecified precision）
    ///
    /// 本函数的精度是不确定的。这意味着它会随平台、
    /// Rust 版本而变化，甚至在同一次执行中，前后两次调用之间也可能不同。
    ///
    /// 本函数目前对应于 libc 的 `expm1f128`（在 Unix
    /// 与 Windows 上）。注意这在将来可能会改变。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(f128)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f128_math)] {
    ///
    /// let x = 1e-8_f128;
    ///
    /// // 对于非常小的 x，e^x 近似等于 1 + x + x^2 / 2
    /// let approx = x + x * x / 2.0;
    /// let abs_difference = (x.exp_m1() - approx).abs();
    ///
    /// assert!(abs_difference < 1e-10);
    /// # }
    /// ```
    #[inline]
    #[rustc_allow_incoherent_impl]
    #[unstable(feature = "f128", issue = "116909")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    pub fn exp_m1(self) -> f128 {
        cmath::expm1f128(self)
    }

    /// 返回 `ln(1+n)`（自然对数），其精度高于分别执行各步运算的结果。
    /// 其精度高于分别执行各步运算的结果。
    ///
    /// 当 `n < -1.0` 时返回 NaN，当 `n == -1.0` 时返回负无穷。
    ///
    /// # 未指定精度(Unspecified precision）
    ///
    /// 本函数的精度是不确定的。这意味着它会随平台、
    /// Rust 版本而变化，甚至在同一次执行中，前后两次调用之间也可能不同。
    ///
    /// 本函数目前对应于 libc 的 `log1pf128`（在 Unix
    /// 与 Windows 上）。注意这在将来可能会改变。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(f128)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f128_math)] {
    ///
    /// let x = 1e-8_f128;
    ///
    /// // 对于非常小的 x，ln(1 + x) 近似等于 x - x^2 / 2
    /// let approx = x - x * x / 2.0;
    /// let abs_difference = (x.ln_1p() - approx).abs();
    ///
    /// assert!(abs_difference < 1e-10);
    /// # }
    /// ```
    ///
    /// 超出范围的值：
    /// ```
    /// #![feature(f128)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f128_math)] {
    ///
    /// assert_eq!((-1.0_f128).ln_1p(), f128::NEG_INFINITY);
    /// assert!((-2.0_f128).ln_1p().is_nan());
    /// # }
    /// ```
    #[inline]
    #[doc(alias = "log1p")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    #[rustc_allow_incoherent_impl]
    #[unstable(feature = "f128", issue = "116909")]
    pub fn ln_1p(self) -> f128 {
        cmath::log1pf128(self)
    }

    /// 双曲正弦函数。
    ///
    /// # 未指定精度(Unspecified precision）
    ///
    /// 本函数的精度是不确定的。这意味着它会随平台、
    /// Rust 版本而变化，甚至在同一次执行中，前后两次调用之间也可能不同。
    ///
    /// 本函数目前对应于 libc 的 `sinhf128`（在 Unix
    /// 与 Windows 上）。注意这在将来可能会改变。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(f128)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f128_math)] {
    ///
    /// let e = std::f128::consts::E;
    /// let x = 1.0f128;
    ///
    /// let f = x.sinh();
    /// // 在 1 处求 sinh() 得到 `(e^2-1)/(2e)`
    /// let g = ((e * e) - 1.0) / (2.0 * e);
    /// let abs_difference = (f - g).abs();
    ///
    /// assert!(abs_difference <= f128::EPSILON);
    /// # }
    /// ```
    #[inline]
    #[rustc_allow_incoherent_impl]
    #[unstable(feature = "f128", issue = "116909")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    pub fn sinh(self) -> f128 {
        cmath::sinhf128(self)
    }

    /// 双曲余弦函数。
    ///
    /// # 未指定精度(Unspecified precision）
    ///
    /// 本函数的精度是不确定的。这意味着它会随平台、
    /// Rust 版本而变化，甚至在同一次执行中，前后两次调用之间也可能不同。
    ///
    /// 本函数目前对应于 libc 的 `coshf128`（在 Unix
    /// 与 Windows 上）。注意这在将来可能会改变。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(f128)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f128_math)] {
    ///
    /// let e = std::f128::consts::E;
    /// let x = 1.0f128;
    /// let f = x.cosh();
    /// // 在 1 处求 cosh() 得到此结果
    /// let g = ((e * e) + 1.0) / (2.0 * e);
    /// let abs_difference = (f - g).abs();
    ///
    /// // 结果相同
    /// assert!(abs_difference <= f128::EPSILON);
    /// # }
    /// ```
    #[inline]
    #[rustc_allow_incoherent_impl]
    #[unstable(feature = "f128", issue = "116909")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    pub fn cosh(self) -> f128 {
        cmath::coshf128(self)
    }

    /// 双曲正切函数。
    ///
    /// # 未指定精度(Unspecified precision）
    ///
    /// 本函数的精度是不确定的。这意味着它会随平台、
    /// Rust 版本而变化，甚至在同一次执行中，前后两次调用之间也可能不同。
    ///
    /// 本函数目前对应于 libc 的 `tanhf128`（在 Unix
    /// 与 Windows 上）。注意这在将来可能会改变。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(f128)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f128_math)] {
    ///
    /// let e = std::f128::consts::E;
    /// let x = 1.0f128;
    ///
    /// let f = x.tanh();
    /// // 在 1 处求 tanh() 得到 `(1 - e^(-2))/(1 + e^(-2))`
    /// let g = (1.0 - e.powi(-2)) / (1.0 + e.powi(-2));
    /// let abs_difference = (f - g).abs();
    ///
    /// assert!(abs_difference <= f128::EPSILON);
    /// # }
    /// ```
    #[inline]
    #[rustc_allow_incoherent_impl]
    #[unstable(feature = "f128", issue = "116909")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    pub fn tanh(self) -> f128 {
        cmath::tanhf128(self)
    }

    /// 反双曲正弦函数。
    ///
    /// # 未指定精度(Unspecified precision）
    ///
    /// 本函数的精度是不确定的。这意味着它会随平台、
    /// Rust 版本而变化，甚至在同一次执行中，前后两次调用之间也可能不同。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(f128)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f128_math)] {
    ///
    /// let x = 1.0f128;
    /// let f = x.sinh().asinh();
    ///
    /// let abs_difference = (f - x).abs();
    ///
    /// assert!(abs_difference <= f128::EPSILON);
    /// # }
    /// ```
    #[inline]
    #[doc(alias = "arcsinh")]
    #[rustc_allow_incoherent_impl]
    #[unstable(feature = "f128", issue = "116909")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    pub fn asinh(self) -> f128 {
        let ax = self.abs();
        let ix = 1.0 / ax;
        (ax + (ax / (Self::hypot(1.0, ix) + ix))).ln_1p().copysign(self)
    }

    /// 反双曲余弦函数。
    ///
    /// # 未指定精度(Unspecified precision）
    ///
    /// 本函数的精度是不确定的。这意味着它会随平台、
    /// Rust 版本而变化，甚至在同一次执行中，前后两次调用之间也可能不同。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(f128)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f128_math)] {
    ///
    /// let x = 1.0f128;
    /// let f = x.cosh().acosh();
    ///
    /// let abs_difference = (f - x).abs();
    ///
    /// assert!(abs_difference <= f128::EPSILON);
    /// # }
    /// ```
    #[inline]
    #[doc(alias = "arccosh")]
    #[rustc_allow_incoherent_impl]
    #[unstable(feature = "f128", issue = "116909")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    pub fn acosh(self) -> f128 {
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
    /// 本函数的精度是不确定的。这意味着它会随平台、
    /// Rust 版本而变化，甚至在同一次执行中，前后两次调用之间也可能不同。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(f128)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f128_math)] {
    ///
    /// let x = std::f128::consts::FRAC_PI_6;
    /// let f = x.tanh().atanh();
    ///
    /// let abs_difference = (f - x).abs();
    ///
    /// assert!(abs_difference <= 1e-5);
    /// # }
    /// ```
    #[inline]
    #[doc(alias = "arctanh")]
    #[rustc_allow_incoherent_impl]
    #[unstable(feature = "f128", issue = "116909")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    pub fn atanh(self) -> f128 {
        0.5 * ((2.0 * self) / (1.0 - self)).ln_1p()
    }

    /// 伽马函数（gamma function）。
    ///
    /// # 未指定精度(Unspecified precision）
    ///
    /// 本函数的精度是不确定的。这意味着它会随平台、
    /// Rust 版本而变化，甚至在同一次执行中，前后两次调用之间也可能不同。
    ///
    /// 本函数目前对应于 libc 的 `tgammaf128`（在 Unix
    /// 与 Windows 上）。注意这在将来可能会改变。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(f128)]
    /// #![feature(float_gamma)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f128_math)] {
    ///
    /// let x = 5.0f128;
    ///
    /// let abs_difference = (x.gamma() - 24.0).abs();
    ///
    /// assert!(abs_difference <= f128::EPSILON);
    /// # }
    /// ```
    #[inline]
    #[rustc_allow_incoherent_impl]
    #[unstable(feature = "f128", issue = "116909")]
    // #[unstable(feature = "float_gamma", issue = "99842")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    pub fn gamma(self) -> f128 {
        cmath::tgammaf128(self)
    }

    /// 伽马函数绝对值的自然对数。
    ///
    /// 元组中的整数部分表示伽马函数的符号。
    ///
    /// # 未指定精度(Unspecified precision）
    ///
    /// 本函数的精度是不确定的。这意味着它会随平台、
    /// Rust 版本而变化，甚至在同一次执行中，前后两次调用之间也可能不同。
    ///
    /// 本函数目前对应于 libc 的 `lgammaf128_r`（在 Unix
    /// 与 Windows 上）。注意这在将来可能会改变。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(f128)]
    /// #![feature(float_gamma)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f128_math)] {
    ///
    /// let x = 2.0f128;
    ///
    /// let abs_difference = (x.ln_gamma().0 - 0.0).abs();
    ///
    /// assert!(abs_difference <= f128::EPSILON);
    /// # }
    /// ```
    #[inline]
    #[rustc_allow_incoherent_impl]
    #[unstable(feature = "f128", issue = "116909")]
    // #[unstable(feature = "float_gamma", issue = "99842")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    pub fn ln_gamma(self) -> (f128, i32) {
        let mut signgamp: i32 = 0;
        let x = cmath::lgammaf128_r(self, &mut signgamp);
        (x, signgamp)
    }

    /// 误差函数（error function）。
    ///
    /// # 未指定精度(Unspecified precision）
    ///
    /// 本函数的精度是不确定的。这意味着它会随平台、
    /// Rust 版本而变化，甚至在同一次执行中，前后两次调用之间也可能不同。
    ///
    /// 本函数目前对应于 libc 的 `erff128`（在 Unix
    /// 与 Windows 上）。注意这在将来可能会改变。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(f128)]
    /// #![feature(float_erf)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f128_math)] {
    /// /// 误差函数描述了正态分布中有百分之多少落在
    /// /// `x` 个标准差以内（按 `1/sqrt(2)` 缩放）。
    /// fn within_standard_deviations(x: f128) -> f128 {
    ///     (x * std::f128::consts::FRAC_1_SQRT_2).erf() * 100.0
    /// }
    ///
    /// // 正态分布中有 68% 落在一个标准差以内
    /// assert!((within_standard_deviations(1.0) - 68.269).abs() < 0.01);
    /// // 正态分布中有 95% 落在两个标准差以内
    /// assert!((within_standard_deviations(2.0) - 95.450).abs() < 0.01);
    /// // 正态分布中有 99.7% 落在三个标准差以内
    /// assert!((within_standard_deviations(3.0) - 99.730).abs() < 0.01);
    /// # }
    /// ```
    #[rustc_allow_incoherent_impl]
    #[must_use = "method returns a new number and does not mutate the original value"]
    #[unstable(feature = "f128", issue = "116909")]
    // #[unstable(feature = "float_erf", issue = "136321")]
    #[inline]
    pub fn erf(self) -> f128 {
        cmath::erff128(self)
    }

    /// 互补误差函数（complementary error function）。
    ///
    /// # 未指定精度(Unspecified precision）
    ///
    /// 本函数的精度是不确定的。这意味着它会随平台、
    /// Rust 版本而变化，甚至在同一次执行中，前后两次调用之间也可能不同。
    ///
    /// 本函数目前对应于 libc 的 `erfcf128`（在 Unix
    /// 与 Windows 上）。注意这在将来可能会改变。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(f128)]
    /// #![feature(float_erf)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f128_math)] {
    /// let x: f128 = 0.123;
    ///
    /// let one = x.erf() + x.erfc();
    /// let abs_difference = (one - 1.0).abs();
    ///
    /// assert!(abs_difference <= f128::EPSILON);
    /// # }
    /// ```
    #[rustc_allow_incoherent_impl]
    #[must_use = "method returns a new number and does not mutate the original value"]
    #[unstable(feature = "f128", issue = "116909")]
    // #[unstable(feature = "float_erf", issue = "136321")]
    #[inline]
    pub fn erfc(self) -> f128 {
        cmath::erfcf128(self)
    }
}
