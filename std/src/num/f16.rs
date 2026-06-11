//! `f16` 半精度（half-precision）浮点类型的相关常量。
//!
//! *[另见 `f16` 原始类型](primitive@f16)。*
//!
//! 具有数学意义的重要数值由 `consts` 子模块提供。
//!
//! 实现说明：本模块在 std 而非 core 中提供 `f16` 的浮点数学方法（如 `sqrt`、
//! `sin`、`exp`、`ln`、`hypot` 等），是因为这些函数的实现依赖编译器内建函数
//! （intrinsics）或底层平台/系统的数学库（在 Unix 与 Windows 上即 libc/libm）。
//! 这类运行时依赖在不依赖操作系统的 core 中无法满足，因此只有 std 才能提供它们；
//! 而那些不依赖运行时、纯位运算即可完成的方法则定义在 core 上。

#![unstable(feature = "f16", issue = "116909")]
#![doc(test(attr(feature(cfg_target_has_reliable_f16_f128), expect(internal_features))))]

#[unstable(feature = "f16", issue = "116909")]
pub use core::f16::consts;

#[cfg(not(test))]
use crate::intrinsics;
#[cfg(not(test))]
use crate::sys::cmath;

#[cfg(not(test))]
impl f16 {
    /// 计算一个数的浮点数次幂。
    ///
    /// 注意本函数较为特殊：对于 NaN 输入它也可能返回非 NaN 的结果。例如，
    /// 例如，`f16::powf(f16::NAN, 0.0)` 返回 `1.0`。但是，如果某个输入是一个 *signaling*（信号）
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
    /// #![feature(f16)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f16_math)] {
    ///
    /// let x = 2.0_f16;
    /// let abs_difference = (x.powf(2.0) - (x * x)).abs();
    /// assert!(abs_difference <= f16::EPSILON);
    ///
    /// assert_eq!(f16::powf(1.0, f16::NAN), 1.0);
    /// assert_eq!(f16::powf(f16::NAN, 0.0), 1.0);
    /// assert_eq!(f16::powf(0.0, 0.0), 1.0);
    /// # }
    /// ```
    #[inline]
    #[rustc_allow_incoherent_impl]
    #[unstable(feature = "f16", issue = "116909")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    pub fn powf(self, n: f16) -> f16 {
        intrinsics::powf16(self, n)
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
    /// #![feature(f16)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f16_math)] {
    ///
    /// let one = 1.0f16;
    /// // e^1
    /// let e = one.exp();
    ///
    /// // ln(e) - 1 == 0
    /// let abs_difference = (e.ln() - 1.0).abs();
    ///
    /// assert!(abs_difference <= f16::EPSILON);
    /// # }
    /// ```
    #[inline]
    #[rustc_allow_incoherent_impl]
    #[unstable(feature = "f16", issue = "116909")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    pub fn exp(self) -> f16 {
        intrinsics::expf16(self)
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
    /// #![feature(f16)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f16_math)] {
    ///
    /// let f = 2.0f16;
    ///
    /// // 2^2 - 4 == 0
    /// let abs_difference = (f.exp2() - 4.0).abs();
    ///
    /// assert!(abs_difference <= f16::EPSILON);
    /// # }
    /// ```
    #[inline]
    #[rustc_allow_incoherent_impl]
    #[unstable(feature = "f16", issue = "116909")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    pub fn exp2(self) -> f16 {
        intrinsics::exp2f16(self)
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
    /// #![feature(f16)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f16_math)] {
    ///
    /// let one = 1.0f16;
    /// // e^1
    /// let e = one.exp();
    ///
    /// // ln(e) - 1 == 0
    /// let abs_difference = (e.ln() - 1.0).abs();
    ///
    /// assert!(abs_difference <= f16::EPSILON);
    /// # }
    /// ```
    ///
    /// 非正值：
    /// ```
    /// #![feature(f16)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f16_math)] {
    ///
    /// assert_eq!(0_f16.ln(), f16::NEG_INFINITY);
    /// assert!((-42_f16).ln().is_nan());
    /// # }
    /// ```
    #[inline]
    #[rustc_allow_incoherent_impl]
    #[unstable(feature = "f16", issue = "116909")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    pub fn ln(self) -> f16 {
        intrinsics::logf16(self)
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
    /// #![feature(f16)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f16_math)] {
    ///
    /// let five = 5.0f16;
    ///
    /// // log5(5) - 1 == 0
    /// let abs_difference = (five.log(5.0) - 1.0).abs();
    ///
    /// assert!(abs_difference <= f16::EPSILON);
    /// # }
    /// ```
    ///
    /// 非正值：
    /// ```
    /// #![feature(f16)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f16_math)] {
    ///
    /// assert_eq!(0_f16.log(10.0), f16::NEG_INFINITY);
    /// assert!((-42_f16).log(10.0).is_nan());
    /// # }
    /// ```
    #[inline]
    #[rustc_allow_incoherent_impl]
    #[unstable(feature = "f16", issue = "116909")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    pub fn log(self, base: f16) -> f16 {
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
    /// #![feature(f16)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f16_math)] {
    ///
    /// let two = 2.0f16;
    ///
    /// // log2(2) - 1 == 0
    /// let abs_difference = (two.log2() - 1.0).abs();
    ///
    /// assert!(abs_difference <= f16::EPSILON);
    /// # }
    /// ```
    ///
    /// 非正值：
    /// ```
    /// #![feature(f16)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f16_math)] {
    ///
    /// assert_eq!(0_f16.log2(), f16::NEG_INFINITY);
    /// assert!((-42_f16).log2().is_nan());
    /// # }
    /// ```
    #[inline]
    #[rustc_allow_incoherent_impl]
    #[unstable(feature = "f16", issue = "116909")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    pub fn log2(self) -> f16 {
        intrinsics::log2f16(self)
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
    /// #![feature(f16)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f16_math)] {
    ///
    /// let ten = 10.0f16;
    ///
    /// // log10(10) - 1 == 0
    /// let abs_difference = (ten.log10() - 1.0).abs();
    ///
    /// assert!(abs_difference <= f16::EPSILON);
    /// # }
    /// ```
    ///
    /// 非正值：
    /// ```
    /// #![feature(f16)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f16_math)] {
    ///
    /// assert_eq!(0_f16.log10(), f16::NEG_INFINITY);
    /// assert!((-42_f16).log10().is_nan());
    /// # }
    /// ```
    #[inline]
    #[rustc_allow_incoherent_impl]
    #[unstable(feature = "f16", issue = "116909")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    pub fn log10(self) -> f16 {
        intrinsics::log10f16(self)
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
    /// 本函数目前对应于 libc 的 `hypotf`（在 Unix
    /// 与 Windows 上）。注意这在将来可能会改变。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(f16)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f16_math)] {
    ///
    /// let x = 2.0f16;
    /// let y = 3.0f16;
    ///
    /// // sqrt(x^2 + y^2)
    /// let abs_difference = (x.hypot(y) - (x.powi(2) + y.powi(2)).sqrt()).abs();
    ///
    /// assert!(abs_difference <= f16::EPSILON);
    /// # }
    /// ```
    #[inline]
    #[rustc_allow_incoherent_impl]
    #[unstable(feature = "f16", issue = "116909")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    pub fn hypot(self, other: f16) -> f16 {
        cmath::hypotf(self as f32, other as f32) as f16
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
    /// #![feature(f16)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f16_math)] {
    ///
    /// let x = std::f16::consts::FRAC_PI_2;
    ///
    /// let abs_difference = (x.sin() - 1.0).abs();
    ///
    /// assert!(abs_difference <= f16::EPSILON);
    /// # }
    /// ```
    #[inline]
    #[rustc_allow_incoherent_impl]
    #[unstable(feature = "f16", issue = "116909")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    pub fn sin(self) -> f16 {
        intrinsics::sinf16(self)
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
    /// #![feature(f16)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f16_math)] {
    ///
    /// let x = 2.0 * std::f16::consts::PI;
    ///
    /// let abs_difference = (x.cos() - 1.0).abs();
    ///
    /// assert!(abs_difference <= f16::EPSILON);
    /// # }
    /// ```
    #[inline]
    #[rustc_allow_incoherent_impl]
    #[unstable(feature = "f16", issue = "116909")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    pub fn cos(self) -> f16 {
        intrinsics::cosf16(self)
    }

    /// 计算一个数（以弧度为单位）的正切。
    ///
    /// # 未指定精度(Unspecified precision）
    ///
    /// 本函数的精度是不确定的。这意味着它会随平台、
    /// Rust 版本而变化，甚至在同一次执行中，前后两次调用之间也可能不同。
    ///
    /// 本函数目前对应于 libc 的 `tanf`（在 Unix 与
    /// Windows 上）。注意这在将来可能会改变。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(f16)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f16_math)] {
    ///
    /// let x = std::f16::consts::FRAC_PI_4;
    /// let abs_difference = (x.tan() - 1.0).abs();
    ///
    /// assert!(abs_difference <= f16::EPSILON);
    /// # }
    /// ```
    #[inline]
    #[rustc_allow_incoherent_impl]
    #[unstable(feature = "f16", issue = "116909")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    pub fn tan(self) -> f16 {
        cmath::tanf(self as f32) as f16
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
    /// 本函数目前对应于 libc 的 `asinf`（在 Unix
    /// 与 Windows 上）。注意这在将来可能会改变。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(f16)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f16_math)] {
    ///
    /// let f = std::f16::consts::FRAC_PI_4;
    ///
    /// // asin(sin(pi/2))
    /// let abs_difference = (f.sin().asin() - f).abs();
    ///
    /// assert!(abs_difference <= f16::EPSILON);
    /// # }
    /// ```
    #[inline]
    #[doc(alias = "arcsin")]
    #[rustc_allow_incoherent_impl]
    #[unstable(feature = "f16", issue = "116909")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    pub fn asin(self) -> f16 {
        cmath::asinf(self as f32) as f16
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
    /// 本函数目前对应于 libc 的 `acosf`（在 Unix
    /// 与 Windows 上）。注意这在将来可能会改变。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(f16)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f16_math)] {
    ///
    /// let f = std::f16::consts::FRAC_PI_4;
    ///
    /// // acos(cos(pi/4))
    /// let abs_difference = (f.cos().acos() - std::f16::consts::FRAC_PI_4).abs();
    ///
    /// assert!(abs_difference <= f16::EPSILON);
    /// # }
    /// ```
    #[inline]
    #[doc(alias = "arccos")]
    #[rustc_allow_incoherent_impl]
    #[unstable(feature = "f16", issue = "116909")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    pub fn acos(self) -> f16 {
        cmath::acosf(self as f32) as f16
    }

    /// 计算一个数的反正切。返回值为弧度，
    /// 取值范围为 [-pi/2, pi/2]；
    ///
    /// # 未指定精度(Unspecified precision）
    ///
    /// 本函数的精度是不确定的。这意味着它会随平台、
    /// Rust 版本而变化，甚至在同一次执行中，前后两次调用之间也可能不同。
    ///
    /// 本函数目前对应于 libc 的 `atanf`（在 Unix
    /// 与 Windows 上）。注意这在将来可能会改变。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(f16)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f16_math)] {
    ///
    /// let f = 1.0f16;
    ///
    /// // atan(tan(1))
    /// let abs_difference = (f.tan().atan() - 1.0).abs();
    ///
    /// assert!(abs_difference <= f16::EPSILON);
    /// # }
    /// ```
    #[inline]
    #[doc(alias = "arctan")]
    #[rustc_allow_incoherent_impl]
    #[unstable(feature = "f16", issue = "116909")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    pub fn atan(self) -> f16 {
        cmath::atanf(self as f32) as f16
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
    /// 本函数目前对应于 libc 的 `atan2f`（在 Unix
    /// 与 Windows 上）。注意这在将来可能会改变。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(f16)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f16_math)] {
    ///
    /// // 正角度从正 x 轴起
    /// // （逆时针为正）
    /// // -pi/4 弧度（顺时针 45 度）
    /// let x1 = 3.0f16;
    /// let y1 = -3.0f16;
    ///
    /// // 3pi/4 弧度（逆时针 135 度）
    /// let x2 = -3.0f16;
    /// let y2 = 3.0f16;
    ///
    /// let abs_difference_1 = (y1.atan2(x1) - (-std::f16::consts::FRAC_PI_4)).abs();
    /// let abs_difference_2 = (y2.atan2(x2) - (3.0 * std::f16::consts::FRAC_PI_4)).abs();
    ///
    /// assert!(abs_difference_1 <= f16::EPSILON);
    /// assert!(abs_difference_2 <= f16::EPSILON);
    /// # }
    /// ```
    #[inline]
    #[rustc_allow_incoherent_impl]
    #[unstable(feature = "f16", issue = "116909")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    pub fn atan2(self, other: f16) -> f16 {
        cmath::atan2f(self as f32, other as f32) as f16
    }

    /// 同时计算数 `x` 的正弦与余弦，返回
    /// `(sin(x), cos(x))`。
    ///
    /// # 未指定精度(Unspecified precision）
    ///
    /// 本函数的精度是不确定的。这意味着它会随平台、
    /// Rust 版本而变化，甚至在同一次执行中，前后两次调用之间也可能不同。
    ///
    /// 本函数目前对应于 `(f16::sin(x),
    /// f16::cos(x))`。注意这在将来可能会改变。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(f16)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f16_math)] {
    ///
    /// let x = std::f16::consts::FRAC_PI_4;
    /// let f = x.sin_cos();
    ///
    /// let abs_difference_0 = (f.0 - x.sin()).abs();
    /// let abs_difference_1 = (f.1 - x.cos()).abs();
    ///
    /// assert!(abs_difference_0 <= f16::EPSILON);
    /// assert!(abs_difference_1 <= f16::EPSILON);
    /// # }
    /// ```
    #[inline]
    #[doc(alias = "sincos")]
    #[rustc_allow_incoherent_impl]
    #[unstable(feature = "f16", issue = "116909")]
    pub fn sin_cos(self) -> (f16, f16) {
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
    /// 本函数目前对应于 libc 的 `expm1f`（在 Unix
    /// 与 Windows 上）。注意这在将来可能会改变。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(f16)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f16_math)] {
    ///
    /// let x = 1e-4_f16;
    ///
    /// // 对于非常小的 x，e^x 近似等于 1 + x + x^2 / 2
    /// let approx = x + x * x / 2.0;
    /// let abs_difference = (x.exp_m1() - approx).abs();
    ///
    /// assert!(abs_difference < 1e-4);
    /// # }
    /// ```
    #[inline]
    #[rustc_allow_incoherent_impl]
    #[unstable(feature = "f16", issue = "116909")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    pub fn exp_m1(self) -> f16 {
        cmath::expm1f(self as f32) as f16
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
    /// 本函数目前对应于 libc 的 `log1pf`（在 Unix
    /// 与 Windows 上）。注意这在将来可能会改变。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(f16)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f16_math)] {
    ///
    /// let x = 1e-4_f16;
    ///
    /// // 对于非常小的 x，ln(1 + x) 近似等于 x - x^2 / 2
    /// let approx = x - x * x / 2.0;
    /// let abs_difference = (x.ln_1p() - approx).abs();
    ///
    /// assert!(abs_difference < 1e-4);
    /// # }
    /// ```
    ///
    /// 超出范围的值：
    /// ```
    /// #![feature(f16)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f16_math)] {
    ///
    /// assert_eq!((-1.0_f16).ln_1p(), f16::NEG_INFINITY);
    /// assert!((-2.0_f16).ln_1p().is_nan());
    /// # }
    /// ```
    #[inline]
    #[doc(alias = "log1p")]
    #[rustc_allow_incoherent_impl]
    #[unstable(feature = "f16", issue = "116909")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    pub fn ln_1p(self) -> f16 {
        cmath::log1pf(self as f32) as f16
    }

    /// 双曲正弦函数。
    ///
    /// # 未指定精度(Unspecified precision）
    ///
    /// 本函数的精度是不确定的。这意味着它会随平台、
    /// Rust 版本而变化，甚至在同一次执行中，前后两次调用之间也可能不同。
    ///
    /// 本函数目前对应于 libc 的 `sinhf`（在 Unix
    /// 与 Windows 上）。注意这在将来可能会改变。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(f16)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f16_math)] {
    ///
    /// let e = std::f16::consts::E;
    /// let x = 1.0f16;
    ///
    /// let f = x.sinh();
    /// // 在 1 处求 sinh() 得到 `(e^2-1)/(2e)`
    /// let g = ((e * e) - 1.0) / (2.0 * e);
    /// let abs_difference = (f - g).abs();
    ///
    /// assert!(abs_difference <= f16::EPSILON);
    /// # }
    /// ```
    #[inline]
    #[rustc_allow_incoherent_impl]
    #[unstable(feature = "f16", issue = "116909")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    pub fn sinh(self) -> f16 {
        cmath::sinhf(self as f32) as f16
    }

    /// 双曲余弦函数。
    ///
    /// # 未指定精度(Unspecified precision）
    ///
    /// 本函数的精度是不确定的。这意味着它会随平台、
    /// Rust 版本而变化，甚至在同一次执行中，前后两次调用之间也可能不同。
    ///
    /// 本函数目前对应于 libc 的 `coshf`（在 Unix
    /// 与 Windows 上）。注意这在将来可能会改变。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(f16)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f16_math)] {
    ///
    /// let e = std::f16::consts::E;
    /// let x = 1.0f16;
    /// let f = x.cosh();
    /// // 在 1 处求 cosh() 得到此结果
    /// let g = ((e * e) + 1.0) / (2.0 * e);
    /// let abs_difference = (f - g).abs();
    ///
    /// // 结果相同
    /// assert!(abs_difference <= f16::EPSILON);
    /// # }
    /// ```
    #[inline]
    #[rustc_allow_incoherent_impl]
    #[unstable(feature = "f16", issue = "116909")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    pub fn cosh(self) -> f16 {
        cmath::coshf(self as f32) as f16
    }

    /// 双曲正切函数。
    ///
    /// # 未指定精度(Unspecified precision）
    ///
    /// 本函数的精度是不确定的。这意味着它会随平台、
    /// Rust 版本而变化，甚至在同一次执行中，前后两次调用之间也可能不同。
    ///
    /// 本函数目前对应于 libc 的 `tanhf`（在 Unix
    /// 与 Windows 上）。注意这在将来可能会改变。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(f16)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f16_math)] {
    ///
    /// let e = std::f16::consts::E;
    /// let x = 1.0f16;
    ///
    /// let f = x.tanh();
    /// // 在 1 处求 tanh() 得到 `(1 - e^(-2))/(1 + e^(-2))`
    /// let g = (1.0 - e.powi(-2)) / (1.0 + e.powi(-2));
    /// let abs_difference = (f - g).abs();
    ///
    /// assert!(abs_difference <= f16::EPSILON);
    /// # }
    /// ```
    #[inline]
    #[rustc_allow_incoherent_impl]
    #[unstable(feature = "f16", issue = "116909")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    pub fn tanh(self) -> f16 {
        cmath::tanhf(self as f32) as f16
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
    /// #![feature(f16)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f16_math)] {
    ///
    /// let x = 1.0f16;
    /// let f = x.sinh().asinh();
    ///
    /// let abs_difference = (f - x).abs();
    ///
    /// assert!(abs_difference <= f16::EPSILON);
    /// # }
    /// ```
    #[inline]
    #[doc(alias = "arcsinh")]
    #[rustc_allow_incoherent_impl]
    #[unstable(feature = "f16", issue = "116909")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    pub fn asinh(self) -> f16 {
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
    /// #![feature(f16)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f16_math)] {
    ///
    /// let x = 1.0f16;
    /// let f = x.cosh().acosh();
    ///
    /// let abs_difference = (f - x).abs();
    ///
    /// assert!(abs_difference <= f16::EPSILON);
    /// # }
    /// ```
    #[inline]
    #[doc(alias = "arccosh")]
    #[rustc_allow_incoherent_impl]
    #[unstable(feature = "f16", issue = "116909")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    pub fn acosh(self) -> f16 {
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
    /// #![feature(f16)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f16_math)] {
    ///
    /// let x = std::f16::consts::FRAC_PI_6;
    /// let f = x.tanh().atanh();
    ///
    /// let abs_difference = (f - x).abs();
    ///
    /// assert!(abs_difference <= 0.01);
    /// # }
    /// ```
    #[inline]
    #[doc(alias = "arctanh")]
    #[rustc_allow_incoherent_impl]
    #[unstable(feature = "f16", issue = "116909")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    pub fn atanh(self) -> f16 {
        0.5 * ((2.0 * self) / (1.0 - self)).ln_1p()
    }

    /// 伽马函数（gamma function）。
    ///
    /// # 未指定精度(Unspecified precision）
    ///
    /// 本函数的精度是不确定的。这意味着它会随平台、
    /// Rust 版本而变化，甚至在同一次执行中，前后两次调用之间也可能不同。
    ///
    /// 本函数目前对应于 libc 的 `tgammaf`（在 Unix
    /// 与 Windows 上）。注意这在将来可能会改变。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(f16)]
    /// #![feature(float_gamma)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f16_math)] {
    ///
    /// let x = 5.0f16;
    ///
    /// let abs_difference = (x.gamma() - 24.0).abs();
    ///
    /// assert!(abs_difference <= f16::EPSILON);
    /// # }
    /// ```
    #[inline]
    #[rustc_allow_incoherent_impl]
    #[unstable(feature = "f16", issue = "116909")]
    // #[unstable(feature = "float_gamma", issue = "99842")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    pub fn gamma(self) -> f16 {
        cmath::tgammaf(self as f32) as f16
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
    /// 本函数目前对应于 libc 的 `lgamma_r`（在 Unix
    /// 与 Windows 上）。注意这在将来可能会改变。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(f16)]
    /// #![feature(float_gamma)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f16_math)] {
    ///
    /// let x = 2.0f16;
    ///
    /// let abs_difference = (x.ln_gamma().0 - 0.0).abs();
    ///
    /// assert!(abs_difference <= f16::EPSILON);
    /// # }
    /// ```
    #[inline]
    #[rustc_allow_incoherent_impl]
    #[unstable(feature = "f16", issue = "116909")]
    // #[unstable(feature = "float_gamma", issue = "99842")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    pub fn ln_gamma(self) -> (f16, i32) {
        let mut signgamp: i32 = 0;
        let x = cmath::lgammaf_r(self as f32, &mut signgamp) as f16;
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
    /// #![feature(f16)]
    /// #![feature(float_erf)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f16_math)] {
    /// /// 误差函数描述了正态分布中有百分之多少落在
    /// /// `x` 个标准差以内（按 `1/sqrt(2)` 缩放）。
    /// fn within_standard_deviations(x: f16) -> f16 {
    ///     (x * std::f16::consts::FRAC_1_SQRT_2).erf() * 100.0
    /// }
    ///
    /// // 正态分布中有 68% 落在一个标准差以内
    /// assert!((within_standard_deviations(1.0) - 68.269).abs() < 0.1);
    /// // 正态分布中有 95% 落在两个标准差以内
    /// assert!((within_standard_deviations(2.0) - 95.450).abs() < 0.1);
    /// // 正态分布中有 99.7% 落在三个标准差以内
    /// assert!((within_standard_deviations(3.0) - 99.730).abs() < 0.1);
    /// # }
    /// ```
    #[rustc_allow_incoherent_impl]
    #[must_use = "method returns a new number and does not mutate the original value"]
    #[unstable(feature = "f16", issue = "116909")]
    // #[unstable(feature = "float_erf", issue = "136321")]
    #[inline]
    pub fn erf(self) -> f16 {
        cmath::erff(self as f32) as f16
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
    /// #![feature(f16)]
    /// #![feature(float_erf)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f16_math)] {
    /// let x: f16 = 0.123;
    ///
    /// let one = x.erf() + x.erfc();
    /// let abs_difference = (one - 1.0).abs();
    ///
    /// assert!(abs_difference <= f16::EPSILON);
    /// # }
    /// ```
    #[rustc_allow_incoherent_impl]
    #[must_use = "method returns a new number and does not mutate the original value"]
    #[unstable(feature = "f16", issue = "116909")]
    // #[unstable(feature = "float_erf", issue = "136321")]
    #[inline]
    pub fn erfc(self) -> f16 {
        cmath::erfcf(self as f32) as f16
    }
}
