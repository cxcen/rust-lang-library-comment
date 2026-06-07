//! `f32` 浮点类型的常量。
//!
//! *另请参阅 [`f32` 原始类型][f32]。*
//!
//! 具有数学意义的常数位于 `consts` 子模块中。
//!
//! 对于直接定义在本模块中的常量
//! （不同于 `consts` 子模块中的数学常数），
//! 新代码应改用
//! 直接定义在 `f32` 类型上的关联常量。

#![stable(feature = "rust1", since = "1.0.0")]

use crate::convert::FloatToInt;
use crate::num::FpCategory;
use crate::panic::const_assert;
use crate::{cfg_select, intrinsics, mem};

/// `f32` 内部表示使用的基数。
/// 请改用 [`f32::RADIX`]。
///
/// # 示例
///
/// ```rust
/// // deprecated way
/// # #[allow(deprecated, deprecated_in_future)]
/// let r = std::f32::RADIX;
///
/// // intended way
/// let r = f32::RADIX;
/// ```
#[stable(feature = "rust1", since = "1.0.0")]
#[deprecated(since = "TBD", note = "replaced by the `RADIX` associated constant on `f32`")]
#[rustc_diagnostic_item = "f32_legacy_const_radix"]
pub const RADIX: u32 = f32::RADIX;

/// 以 2 为底的有效数字位数。
/// 请改用 [`f32::MANTISSA_DIGITS`]。
///
/// # 示例
///
/// ```rust
/// // deprecated way
/// # #[allow(deprecated, deprecated_in_future)]
/// let d = std::f32::MANTISSA_DIGITS;
///
/// // intended way
/// let d = f32::MANTISSA_DIGITS;
/// ```
#[stable(feature = "rust1", since = "1.0.0")]
#[deprecated(
    since = "TBD",
    note = "replaced by the `MANTISSA_DIGITS` associated constant on `f32`"
)]
#[rustc_diagnostic_item = "f32_legacy_const_mantissa_dig"]
pub const MANTISSA_DIGITS: u32 = f32::MANTISSA_DIGITS;

/// 以 10 为底时近似的有效十进制数字位数。
/// 请改用 [`f32::DIGITS`]。
///
/// # 示例
///
/// ```rust
/// // deprecated way
/// # #[allow(deprecated, deprecated_in_future)]
/// let d = std::f32::DIGITS;
///
/// // intended way
/// let d = f32::DIGITS;
/// ```
#[stable(feature = "rust1", since = "1.0.0")]
#[deprecated(since = "TBD", note = "replaced by the `DIGITS` associated constant on `f32`")]
#[rustc_diagnostic_item = "f32_legacy_const_digits"]
pub const DIGITS: u32 = f32::DIGITS;

/// `f32` 的 [Machine epsilon] 值。
/// 请改用 [`f32::EPSILON`]。
///
/// 这是 `1.0` 与下一个更大的可表示数之间的差值。
///
/// [Machine epsilon]: https://en.wikipedia.org/wiki/Machine_epsilon
///
/// # 示例
///
/// ```rust
/// // deprecated way
/// # #[allow(deprecated, deprecated_in_future)]
/// let e = std::f32::EPSILON;
///
/// // intended way
/// let e = f32::EPSILON;
/// ```
#[stable(feature = "rust1", since = "1.0.0")]
#[deprecated(since = "TBD", note = "replaced by the `EPSILON` associated constant on `f32`")]
#[rustc_diagnostic_item = "f32_legacy_const_epsilon"]
pub const EPSILON: f32 = f32::EPSILON;

/// 最小的有限 `f32` 值。
/// 请改用 [`f32::MIN`]。
///
/// # 示例
///
/// ```rust
/// // deprecated way
/// # #[allow(deprecated, deprecated_in_future)]
/// let min = std::f32::MIN;
///
/// // intended way
/// let min = f32::MIN;
/// ```
#[stable(feature = "rust1", since = "1.0.0")]
#[deprecated(since = "TBD", note = "replaced by the `MIN` associated constant on `f32`")]
#[rustc_diagnostic_item = "f32_legacy_const_min"]
pub const MIN: f32 = f32::MIN;

/// 最小的正正规 `f32` 值。
/// 请改用 [`f32::MIN_POSITIVE`]。
///
/// # 示例
///
/// ```rust
/// // deprecated way
/// # #[allow(deprecated, deprecated_in_future)]
/// let min = std::f32::MIN_POSITIVE;
///
/// // intended way
/// let min = f32::MIN_POSITIVE;
/// ```
#[stable(feature = "rust1", since = "1.0.0")]
#[deprecated(since = "TBD", note = "replaced by the `MIN_POSITIVE` associated constant on `f32`")]
#[rustc_diagnostic_item = "f32_legacy_const_min_positive"]
pub const MIN_POSITIVE: f32 = f32::MIN_POSITIVE;

/// 最大的有限 `f32` 值。
/// 请改用 [`f32::MAX`]。
///
/// # 示例
///
/// ```rust
/// // deprecated way
/// # #[allow(deprecated, deprecated_in_future)]
/// let max = std::f32::MAX;
///
/// // intended way
/// let max = f32::MAX;
/// ```
#[stable(feature = "rust1", since = "1.0.0")]
#[deprecated(since = "TBD", note = "replaced by the `MAX` associated constant on `f32`")]
#[rustc_diagnostic_item = "f32_legacy_const_max"]
pub const MAX: f32 = f32::MAX;

/// 比可能的最小正规 2 次幂指数大 1 的值。
/// 请改用 [`f32::MIN_EXP`]。
///
/// # 示例
///
/// ```rust
/// // deprecated way
/// # #[allow(deprecated, deprecated_in_future)]
/// let min = std::f32::MIN_EXP;
///
/// // intended way
/// let min = f32::MIN_EXP;
/// ```
#[stable(feature = "rust1", since = "1.0.0")]
#[deprecated(since = "TBD", note = "replaced by the `MIN_EXP` associated constant on `f32`")]
#[rustc_diagnostic_item = "f32_legacy_const_min_exp"]
pub const MIN_EXP: i32 = f32::MIN_EXP;

/// 可能的最大 2 次幂指数。
/// 请改用 [`f32::MAX_EXP`]。
///
/// # 示例
///
/// ```rust
/// // deprecated way
/// # #[allow(deprecated, deprecated_in_future)]
/// let max = std::f32::MAX_EXP;
///
/// // intended way
/// let max = f32::MAX_EXP;
/// ```
#[stable(feature = "rust1", since = "1.0.0")]
#[deprecated(since = "TBD", note = "replaced by the `MAX_EXP` associated constant on `f32`")]
#[rustc_diagnostic_item = "f32_legacy_const_max_exp"]
pub const MAX_EXP: i32 = f32::MAX_EXP;

/// 可能的最小正规 10 次幂指数。
/// 请改用 [`f32::MIN_10_EXP`]。
///
/// # 示例
///
/// ```rust
/// // deprecated way
/// # #[allow(deprecated, deprecated_in_future)]
/// let min = std::f32::MIN_10_EXP;
///
/// // intended way
/// let min = f32::MIN_10_EXP;
/// ```
#[stable(feature = "rust1", since = "1.0.0")]
#[deprecated(since = "TBD", note = "replaced by the `MIN_10_EXP` associated constant on `f32`")]
#[rustc_diagnostic_item = "f32_legacy_const_min_10_exp"]
pub const MIN_10_EXP: i32 = f32::MIN_10_EXP;

/// 可能的最大 10 次幂指数。
/// 请改用 [`f32::MAX_10_EXP`]。
///
/// # 示例
///
/// ```rust
/// // deprecated way
/// # #[allow(deprecated, deprecated_in_future)]
/// let max = std::f32::MAX_10_EXP;
///
/// // intended way
/// let max = f32::MAX_10_EXP;
/// ```
#[stable(feature = "rust1", since = "1.0.0")]
#[deprecated(since = "TBD", note = "replaced by the `MAX_10_EXP` associated constant on `f32`")]
#[rustc_diagnostic_item = "f32_legacy_const_max_10_exp"]
pub const MAX_10_EXP: i32 = f32::MAX_10_EXP;

/// 非数（NaN）。
/// 请改用 [`f32::NAN`]。
///
/// # 示例
///
/// ```rust
/// // deprecated way
/// # #[allow(deprecated, deprecated_in_future)]
/// let nan = std::f32::NAN;
///
/// // intended way
/// let nan = f32::NAN;
/// ```
#[stable(feature = "rust1", since = "1.0.0")]
#[deprecated(since = "TBD", note = "replaced by the `NAN` associated constant on `f32`")]
#[rustc_diagnostic_item = "f32_legacy_const_nan"]
pub const NAN: f32 = f32::NAN;

/// 正无穷（Inf，∞）。
/// 请改用 [`f32::INFINITY`]。
///
/// # 示例
///
/// ```rust
/// // deprecated way
/// # #[allow(deprecated, deprecated_in_future)]
/// let inf = std::f32::INFINITY;
///
/// // intended way
/// let inf = f32::INFINITY;
/// ```
#[stable(feature = "rust1", since = "1.0.0")]
#[deprecated(since = "TBD", note = "replaced by the `INFINITY` associated constant on `f32`")]
#[rustc_diagnostic_item = "f32_legacy_const_infinity"]
pub const INFINITY: f32 = f32::INFINITY;

/// 负无穷（-Inf，−∞）。
/// 请改用 [`f32::NEG_INFINITY`]。
///
/// # 示例
///
/// ```rust
/// // deprecated way
/// # #[allow(deprecated, deprecated_in_future)]
/// let ninf = std::f32::NEG_INFINITY;
///
/// // intended way
/// let ninf = f32::NEG_INFINITY;
/// ```
#[stable(feature = "rust1", since = "1.0.0")]
#[deprecated(since = "TBD", note = "replaced by the `NEG_INFINITY` associated constant on `f32`")]
#[rustc_diagnostic_item = "f32_legacy_const_neg_infinity"]
pub const NEG_INFINITY: f32 = f32::NEG_INFINITY;

/// 基本数学常数。
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_diagnostic_item = "f32_consts_mod"]
pub mod consts {
    // FIXME: 后续可替换为来自 cmath 的数学常数。

    /// 阿基米德常数（π）
    #[stable(feature = "rust1", since = "1.0.0")]
    pub const PI: f32 = 3.14159265358979323846264338327950288_f32;

    /// 整圆常数（τ）
    ///
    /// 等于 2π。
    #[stable(feature = "tau_constant", since = "1.47.0")]
    pub const TAU: f32 = 6.28318530717958647692528676655900577_f32;

    /// 黄金比例（φ）
    #[stable(feature = "euler_gamma_golden_ratio", since = "1.94.0")]
    pub const GOLDEN_RATIO: f32 = 1.618033988749894848204586834365638118_f32;

    /// 欧拉-马歇罗尼常数（γ）
    #[stable(feature = "euler_gamma_golden_ratio", since = "1.94.0")]
    pub const EULER_GAMMA: f32 = 0.577215664901532860606512090082402431_f32;

    /// π/2
    #[stable(feature = "rust1", since = "1.0.0")]
    pub const FRAC_PI_2: f32 = 1.57079632679489661923132169163975144_f32;

    /// π/3
    #[stable(feature = "rust1", since = "1.0.0")]
    pub const FRAC_PI_3: f32 = 1.04719755119659774615421446109316763_f32;

    /// π/4
    #[stable(feature = "rust1", since = "1.0.0")]
    pub const FRAC_PI_4: f32 = 0.785398163397448309615660845819875721_f32;

    /// π/6
    #[stable(feature = "rust1", since = "1.0.0")]
    pub const FRAC_PI_6: f32 = 0.52359877559829887307710723054658381_f32;

    /// π/8
    #[stable(feature = "rust1", since = "1.0.0")]
    pub const FRAC_PI_8: f32 = 0.39269908169872415480783042290993786_f32;

    /// 1/π
    #[stable(feature = "rust1", since = "1.0.0")]
    pub const FRAC_1_PI: f32 = 0.318309886183790671537767526745028724_f32;

    /// 1/sqrt（π）
    #[unstable(feature = "more_float_constants", issue = "146939")]
    pub const FRAC_1_SQRT_PI: f32 = 0.564189583547756286948079451560772586_f32;

    /// 1/sqrt（2π）
    #[doc(alias = "FRAC_1_SQRT_TAU")]
    #[unstable(feature = "more_float_constants", issue = "146939")]
    pub const FRAC_1_SQRT_2PI: f32 = 0.398942280401432677939946059934381868_f32;

    /// 2/π
    #[stable(feature = "rust1", since = "1.0.0")]
    pub const FRAC_2_PI: f32 = 0.636619772367581343075535053490057448_f32;

    /// 2/sqrt（π）
    #[stable(feature = "rust1", since = "1.0.0")]
    pub const FRAC_2_SQRT_PI: f32 = 1.12837916709551257389615890312154517_f32;

    /// sqrt（2）
    #[stable(feature = "rust1", since = "1.0.0")]
    pub const SQRT_2: f32 = 1.41421356237309504880168872420969808_f32;

    /// 1/sqrt（2）
    #[stable(feature = "rust1", since = "1.0.0")]
    pub const FRAC_1_SQRT_2: f32 = 0.707106781186547524400844362104849039_f32;

    /// sqrt（3）
    #[unstable(feature = "more_float_constants", issue = "146939")]
    pub const SQRT_3: f32 = 1.732050807568877293527446341505872367_f32;

    /// 1/sqrt（3）
    #[unstable(feature = "more_float_constants", issue = "146939")]
    pub const FRAC_1_SQRT_3: f32 = 0.577350269189625764509148780501957456_f32;

    /// 欧拉数（e）
    #[stable(feature = "rust1", since = "1.0.0")]
    pub const E: f32 = 2.71828182845904523536028747135266250_f32;

    /// log<sub>2</sub>(e)
    #[stable(feature = "rust1", since = "1.0.0")]
    pub const LOG2_E: f32 = 1.44269504088896340735992468100189214_f32;

    /// log<sub>2</sub>(10)
    #[stable(feature = "extra_log_consts", since = "1.43.0")]
    pub const LOG2_10: f32 = 3.32192809488736234787031942948939018_f32;

    /// log<sub>10</sub>(e)
    #[stable(feature = "rust1", since = "1.0.0")]
    pub const LOG10_E: f32 = 0.434294481903251827651128918916605082_f32;

    /// log<sub>10</sub>(2)
    #[stable(feature = "extra_log_consts", since = "1.43.0")]
    pub const LOG10_2: f32 = 0.301029995663981195213738894724493027_f32;

    /// ln(2)
    #[stable(feature = "rust1", since = "1.0.0")]
    pub const LN_2: f32 = 0.693147180559945309417232121458176568_f32;

    /// ln(10)
    #[stable(feature = "rust1", since = "1.0.0")]
    pub const LN_10: f32 = 2.30258509299404568401799145468436421_f32;
}

impl f32 {
    /// `f32` 内部表示使用的基数。
    #[stable(feature = "assoc_int_consts", since = "1.43.0")]
    pub const RADIX: u32 = 2;

    /// 以 2 为底的有效数字位数。
    ///
    /// 注意，按位表示里的尾数字段大小比这个值小 1，
    /// 因为前导 1 不会显式存储。
    #[stable(feature = "assoc_int_consts", since = "1.43.0")]
    pub const MANTISSA_DIGITS: u32 = 24;

    /// 以 10 为底时近似的有效十进制数字位数。
    ///
    /// 这是最大的 <i>x</i>，使得任何具有 <i>x</i>
    /// 位有效数字的十进制数都能无损转换为 `f32` 再转换回来。
    ///
    /// 等于 floor(log<sub>10</sub>&nbsp;2<sup>[`MANTISSA_DIGITS`]&nbsp;&minus;&nbsp;1</sup>)。
    ///
    /// [`MANTISSA_DIGITS`]: f32::MANTISSA_DIGITS
    #[stable(feature = "assoc_int_consts", since = "1.43.0")]
    pub const DIGITS: u32 = 6;

    /// `f32` 的 [Machine epsilon] 值。
    ///
    /// 这是 `1.0` 与下一个更大的可表示数之间的差值。
    ///
    /// 等于 2<sup>1&nbsp;&minus;&nbsp;[`MANTISSA_DIGITS`]</sup>。
    ///
    /// [Machine epsilon]: https://en.wikipedia.org/wiki/Machine_epsilon
    /// [`MANTISSA_DIGITS`]: f32::MANTISSA_DIGITS
    #[stable(feature = "assoc_int_consts", since = "1.43.0")]
    #[rustc_diagnostic_item = "f32_epsilon"]
    pub const EPSILON: f32 = 1.19209290e-07_f32;

    /// 最小的有限 `f32` 值。
    ///
    /// 等于 &minus;[`MAX`]。
    ///
    /// [`MAX`]: f32::MAX
    #[stable(feature = "assoc_int_consts", since = "1.43.0")]
    pub const MIN: f32 = -3.40282347e+38_f32;
    /// 最小的正正规 `f32` 值。
    ///
    /// 等于 2<sup>[`MIN_EXP`]&nbsp;&minus;&nbsp;1</sup>。
    ///
    /// [`MIN_EXP`]: f32::MIN_EXP
    #[stable(feature = "assoc_int_consts", since = "1.43.0")]
    pub const MIN_POSITIVE: f32 = 1.17549435e-38_f32;
    /// 最大的有限 `f32` 值。
    ///
    /// 等于
    /// (1&nbsp;&minus;&nbsp;2<sup>&minus;[`MANTISSA_DIGITS`]</sup>)&nbsp;2<sup>[`MAX_EXP`]</sup>。
    ///
    /// [`MANTISSA_DIGITS`]: f32::MANTISSA_DIGITS
    /// [`MAX_EXP`]: f32::MAX_EXP
    #[stable(feature = "assoc_int_consts", since = "1.43.0")]
    pub const MAX: f32 = 3.40282347e+38_f32;

    /// 比可能的最小*正规* 2 次幂指数大 1 的值
    /// 其中有效数范围为 1 ≤ x < 2（即 IEEE 定义）。
    ///
    /// 这对应于可能的精确最小*正规* 2 次幂指数，
    /// 其中有效数范围为 0.5 ≤ x < 1（即 C 定义）。
    /// 换言之，该类型能表示的所有正规数都
    /// 大于或等于 0.5&nbsp;×&nbsp;2<sup><i>MIN_EXP</i></sup>。
    #[stable(feature = "assoc_int_consts", since = "1.43.0")]
    pub const MIN_EXP: i32 = -125;
    /// 比可能的最大 2 次幂指数大 1 的值
    /// 其中有效数范围为 1 ≤ x < 2（即 IEEE 定义）。
    ///
    /// 这对应于可能的精确最大 2 次幂指数，
    /// 其中有效数范围为 0.5 ≤ x < 1（即 C 定义）。
    /// 换言之，该类型能表示的所有数都
    /// 严格小于 2<sup><i>MAX_EXP</i></sup>。
    #[stable(feature = "assoc_int_consts", since = "1.43.0")]
    pub const MAX_EXP: i32 = 128;

    /// 使 10<sup><i>x</i></sup> 成为正规数的最小 <i>x</i>。
    ///
    /// 等于 ceil(log<sub>10</sub>&nbsp;[`MIN_POSITIVE`])。
    ///
    /// [`MIN_POSITIVE`]: f32::MIN_POSITIVE
    #[stable(feature = "assoc_int_consts", since = "1.43.0")]
    pub const MIN_10_EXP: i32 = -37;
    /// 使 10<sup><i>x</i></sup> 成为正规数的最大 <i>x</i>。
    ///
    /// 等于 floor(log<sub>10</sub>&nbsp;[`MAX`])。
    ///
    /// [`MAX`]: f32::MAX
    #[stable(feature = "assoc_int_consts", since = "1.43.0")]
    pub const MAX_10_EXP: i32 = 38;

    /// 非数（NaN）。
    ///
    /// 注意，IEEE 754 并不只定义单一的 NaN 值；大量不同的位模式都会
    /// 被视为 NaN。此外，标准区分 "signaling" NaN 和
    /// "quiet" NaN，并允许检查其 "payload"（位模式中未指定的位）
    /// 及符号。更多信息见 [NaN 位模式规范](f32#nan-bit-patterns)。
    ///
    ///
    /// 该常量保证是 quiet NaN（在遵循 Rust 假设的目标上，
    /// quiet/signaling 位为 1 表示 quiet NaN）。除此之外，
    /// 这里选择的具体位模式没有额外保证：payload 和符号都是任意的。
    /// 具体位模式可能随 Rust 版本和目标平台变化。
    #[stable(feature = "assoc_int_consts", since = "1.43.0")]
    #[rustc_diagnostic_item = "f32_nan"]
    #[allow(clippy::eq_op)]
    pub const NAN: f32 = 0.0_f32 / 0.0_f32;
    /// 正无穷（Inf，∞）。
    #[stable(feature = "assoc_int_consts", since = "1.43.0")]
    pub const INFINITY: f32 = 1.0_f32 / 0.0_f32;
    /// 负无穷（-Inf，−∞）。
    #[stable(feature = "assoc_int_consts", since = "1.43.0")]
    pub const NEG_INFINITY: f32 = -1.0_f32 / 0.0_f32;

    /// 符号位
    pub(crate) const SIGN_MASK: u32 = 0x8000_0000;

    /// 指数掩码
    pub(crate) const EXP_MASK: u32 = 0x7f80_0000;

    /// 尾数掩码
    pub(crate) const MAN_MASK: u32 = 0x007f_ffff;

    /// 最小可表示正值（最小次正规数）
    const TINY_BITS: u32 = 0x1;

    /// 最小可表示负值（最小负次正规数）
    const NEG_TINY_BITS: u32 = Self::TINY_BITS | Self::SIGN_MASK;

    /// 如果该值是 NaN，则返回 `true`。
    ///
    /// ```
    /// let nan = f32::NAN;
    /// let f = 7.0_f32;
    ///
    /// assert!(nan.is_nan());
    /// assert!(!f.is_nan());
    /// ```
    #[must_use]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_float_classify", since = "1.83.0")]
    #[inline]
    #[allow(clippy::eq_op)] // > if you intended to check if the operand is NaN, use `.is_nan()` instead :)
    pub const fn is_nan(self) -> bool {
        self != self
    }

    /// 如果该值是正无穷或负无穷，则返回 `true`；
    /// 否则返回 `false`。
    ///
    /// ```
    /// let f = 7.0f32;
    /// let inf = f32::INFINITY;
    /// let neg_inf = f32::NEG_INFINITY;
    /// let nan = f32::NAN;
    ///
    /// assert!(!f.is_infinite());
    /// assert!(!nan.is_infinite());
    ///
    /// assert!(inf.is_infinite());
    /// assert!(neg_inf.is_infinite());
    /// ```
    #[must_use]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_float_classify", since = "1.83.0")]
    #[inline]
    pub const fn is_infinite(self) -> bool {
        // 在某些 FPU 上，试图用 transmutation 取巧可能得到错误答案
        // FIXME: 修改 Rust <-> Rust 调用约定以避免这个问题。
        // 见 https://github.com/rust-lang/rust/issues/72327
        (self == f32::INFINITY) | (self == f32::NEG_INFINITY)
    }

    /// 如果该数既不是无穷也不是 NaN，则返回 `true`。
    ///
    /// ```
    /// let f = 7.0f32;
    /// let inf = f32::INFINITY;
    /// let neg_inf = f32::NEG_INFINITY;
    /// let nan = f32::NAN;
    ///
    /// assert!(f.is_finite());
    ///
    /// assert!(!nan.is_finite());
    /// assert!(!inf.is_finite());
    /// assert!(!neg_inf.is_finite());
    /// ```
    #[must_use]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_float_classify", since = "1.83.0")]
    #[inline]
    pub const fn is_finite(self) -> bool {
        // 无需单独处理 NaN：如果 self 是 NaN，
        // 比较结果正好不是 true。
        self.abs() < Self::INFINITY
    }

    /// 如果该数是[次正规数]，则返回 `true`。
    ///
    /// ```
    /// let min = f32::MIN_POSITIVE; // 1.17549435e-38f32
    /// let max = f32::MAX;
    /// let lower_than_min = 1.0e-40_f32;
    /// let zero = 0.0_f32;
    ///
    /// assert!(!min.is_subnormal());
    /// assert!(!max.is_subnormal());
    ///
    /// assert!(!zero.is_subnormal());
    /// assert!(!f32::NAN.is_subnormal());
    /// assert!(!f32::INFINITY.is_subnormal());
    /// // Values between `0` and `min` are Subnormal.
    /// assert!(lower_than_min.is_subnormal());
    /// ```
    /// [subnormal]: https://en.wikipedia.org/wiki/Denormal_number
    #[must_use]
    #[stable(feature = "is_subnormal", since = "1.53.0")]
    #[rustc_const_stable(feature = "const_float_classify", since = "1.83.0")]
    #[inline]
    pub const fn is_subnormal(self) -> bool {
        matches!(self.classify(), FpCategory::Subnormal)
    }

    /// 如果该数既不是零、无穷、
    /// [次正规数]，也不是 NaN，则返回 `true`。
    ///
    /// ```
    /// let min = f32::MIN_POSITIVE; // 1.17549435e-38f32
    /// let max = f32::MAX;
    /// let lower_than_min = 1.0e-40_f32;
    /// let zero = 0.0_f32;
    ///
    /// assert!(min.is_normal());
    /// assert!(max.is_normal());
    ///
    /// assert!(!zero.is_normal());
    /// assert!(!f32::NAN.is_normal());
    /// assert!(!f32::INFINITY.is_normal());
    /// // Values between `0` and `min` are Subnormal.
    /// assert!(!lower_than_min.is_normal());
    /// ```
    /// [subnormal]: https://en.wikipedia.org/wiki/Denormal_number
    #[must_use]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_float_classify", since = "1.83.0")]
    #[inline]
    pub const fn is_normal(self) -> bool {
        matches!(self.classify(), FpCategory::Normal)
    }

    /// 返回该数的浮点分类。如果只需要测试一个性质
    /// 通常使用对应的专用谓词会更快，
    /// 而不是先调用 `classify`。
    ///
    /// ```
    /// use std::num::FpCategory;
    ///
    /// let num = 12.4_f32;
    /// let inf = f32::INFINITY;
    ///
    /// assert_eq!(num.classify(), FpCategory::Normal);
    /// assert_eq!(inf.classify(), FpCategory::Infinite);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_float_classify", since = "1.83.0")]
    pub const fn classify(self) -> FpCategory {
        // 这里曾经使用复杂逻辑，避开简单的按位测试，以便
        // 绕过 x87 目标上的错误代码生成（见
        // https://github.com/rust-lang/rust/issues/114479）。但经历若干 LLVM 版本后，
        // 我们的测试已无法发现复杂版本与朴素版本
        // 之间有差异，因此现在回到朴素版本。
        let b = self.to_bits();
        match (b & Self::MAN_MASK, b & Self::EXP_MASK) {
            (0, Self::EXP_MASK) => FpCategory::Infinite,
            (_, Self::EXP_MASK) => FpCategory::Nan,
            (0, 0) => FpCategory::Zero,
            (_, 0) => FpCategory::Subnormal,
            _ => FpCategory::Normal,
        }
    }

    /// 如果 `self` 带正号则返回 `true`，这包括 `+0.0`、
    /// 符号位为正的 NaN 以及正无穷。
    ///
    /// 注意，IEEE 754 并不为 NaN 情形下的符号位赋予数学含义，
    /// 而 Rust 也不保证 NaN 的位模式会在算术运算中
    /// 保持不变，因此对 NaN 调用 `is_sign_positive` 的结果
    /// 可能产生意外或不可移植的结果。更多信息见[规范
    /// 中的 NaN 位模式](f32#nan-bit-patterns)。如果需要完全可移植的行为，请使用 `self.signum() == 1.0`
    /// （它会对所有 NaN 返回 `false`）。
    ///
    /// ```
    /// let f = 7.0_f32;
    /// let g = -7.0_f32;
    ///
    /// assert!(f.is_sign_positive());
    /// assert!(!g.is_sign_positive());
    /// ```
    #[must_use]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_float_classify", since = "1.83.0")]
    #[inline]
    pub const fn is_sign_positive(self) -> bool {
        !self.is_sign_negative()
    }

    /// 如果 `self` 带负号则返回 `true`，这包括 `-0.0`、
    /// 符号位为负的 NaN 以及负无穷。
    ///
    /// 注意，IEEE 754 并不为 NaN 情形下的符号位赋予数学含义，
    /// 而 Rust 也不保证 NaN 的位模式会在算术运算中
    /// 保持不变，因此对 NaN 调用 `is_sign_negative` 的结果
    /// 可能产生意外或不可移植的结果。更多信息见[规范
    /// 中的 NaN 位模式](f32#nan-bit-patterns)。如果需要完全可移植的行为，请使用 `self.signum() == -1.0`
    /// （它会对所有 NaN 返回 `false`）。
    ///
    /// ```
    /// let f = 7.0f32;
    /// let g = -7.0f32;
    ///
    /// assert!(!f.is_sign_negative());
    /// assert!(g.is_sign_negative());
    /// ```
    #[must_use]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_float_classify", since = "1.83.0")]
    #[inline]
    pub const fn is_sign_negative(self) -> bool {
        // IEEE 754 规定：当且仅当 x 带负号时，isSignMinus(x) 为 true。isSignMinus
        // 同样适用于零和 NaN。
        self.to_bits() & 0x8000_0000 != 0
    }

    /// 返回严格大于 `self` 的最小可表示数。
    ///
    /// 设 `TINY` 为最小可表示正 `f32` 值，则：
    ///   - 如果 `self.is_nan()`，返回 `self`；
    ///   - 如果 `self` 是 [`NEG_INFINITY`]，返回 [`MIN`]；
    ///   - 如果 `self` 是 `-TINY`，返回 -0.0；
    ///   - 如果 `self` 是 -0.0 或 +0.0，返回 `TINY`；
    ///   - 如果 `self` 是 [`MAX`] 或 [`INFINITY`]，返回 [`INFINITY`]；
    ///   - 否则返回唯一一个严格大于 `self` 的最小值。
    ///
    /// 对所有非 NaN 的 `x`，恒等式 `x.next_up() == -(-x).next_down()` 成立。当 `x`
    /// 为有限值时，`x == x.next_up().next_down()` 也成立。
    ///
    /// ```rust
    /// // f32::EPSILON is the difference between 1.0 and the next number up.
    /// assert_eq!(1.0f32.next_up(), 1.0 + f32::EPSILON);
    /// // But not for most numbers.
    /// assert!(0.1f32.next_up() < 0.1 + f32::EPSILON);
    /// assert_eq!(16777216f32.next_up(), 16777218.0);
    /// ```
    ///
    /// 该操作对应 IEEE 754 的 `nextUp`。
    ///
    /// [`NEG_INFINITY`]: Self::NEG_INFINITY
    /// [`INFINITY`]: Self::INFINITY
    /// [`MIN`]: Self::MIN
    /// [`MAX`]: Self::MAX
    #[inline]
    #[doc(alias = "nextUp")]
    #[stable(feature = "float_next_up_down", since = "1.86.0")]
    #[rustc_const_stable(feature = "float_next_up_down", since = "1.86.0")]
    pub const fn next_up(self) -> Self {
        // 某些目标会违反 Rust 对 IEEE 语义的假设，例如把
        // 非正规数刷新为零。这通常是不 sound 且不受支持的，但这里
        // 仍尽力在这些目标上产生正确结果。
        let bits = self.to_bits();
        if self.is_nan() || bits == Self::INFINITY.to_bits() {
            return self;
        }

        let abs = bits & !Self::SIGN_MASK;
        let next_bits = if abs == 0 {
            Self::TINY_BITS
        } else if bits == abs {
            bits + 1
        } else {
            bits - 1
        };
        Self::from_bits(next_bits)
    }

    /// 返回严格小于 `self` 的最大可表示数。
    ///
    /// 设 `TINY` 为最小可表示正 `f32` 值，则：
    ///   - 如果 `self.is_nan()`，返回 `self`；
    ///   - 如果 `self` 是 [`INFINITY`]，返回 [`MAX`]；
    ///   - 如果 `self` 是 `TINY`，返回 0.0；
    ///   - 如果 `self` 是 -0.0 或 +0.0，返回 `-TINY`；
    ///   - 如果 `self` 是 [`MIN`] 或 [`NEG_INFINITY`]，返回 [`NEG_INFINITY`]；
    ///   - 否则返回唯一一个严格小于 `self` 的最大值。
    ///
    /// 对所有非 NaN 的 `x`，恒等式 `x.next_down() == -(-x).next_up()` 成立。当 `x`
    /// 为有限值时，`x == x.next_down().next_up()` 也成立。
    ///
    /// ```rust
    /// let x = 1.0f32;
    /// // Clamp value into range [0, 1).
    /// let clamped = x.clamp(0.0, 1.0f32.next_down());
    /// assert!(clamped < 1.0);
    /// assert_eq!(clamped.next_up(), 1.0);
    /// ```
    ///
    /// 该操作对应 IEEE 754 的 `nextDown`。
    ///
    /// [`NEG_INFINITY`]: Self::NEG_INFINITY
    /// [`INFINITY`]: Self::INFINITY
    /// [`MIN`]: Self::MIN
    /// [`MAX`]: Self::MAX
    #[inline]
    #[doc(alias = "nextDown")]
    #[stable(feature = "float_next_up_down", since = "1.86.0")]
    #[rustc_const_stable(feature = "float_next_up_down", since = "1.86.0")]
    pub const fn next_down(self) -> Self {
        // 某些目标会违反 Rust 对 IEEE 语义的假设，例如把
        // 非正规数刷新为零。这通常是不 sound 且不受支持的，但这里
        // 仍尽力在这些目标上产生正确结果。
        let bits = self.to_bits();
        if self.is_nan() || bits == Self::NEG_INFINITY.to_bits() {
            return self;
        }

        let abs = bits & !Self::SIGN_MASK;
        let next_bits = if abs == 0 {
            Self::NEG_TINY_BITS
        } else if bits == abs {
            bits - 1
        } else {
            bits + 1
        };
        Self::from_bits(next_bits)
    }

    /// 取一个数的倒数（逆），即 `1/x`。
    ///
    /// ```
    /// let x = 2.0_f32;
    /// let abs_difference = (x.recip() - (1.0 / x)).abs();
    ///
    /// assert!(abs_difference <= f32::EPSILON);
    /// ```
    #[must_use = "this returns the result of the operation, without modifying the original"]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_float_methods", since = "1.85.0")]
    #[inline]
    pub const fn recip(self) -> f32 {
        1.0 / self
    }

    /// 将弧度转换为角度。
    ///
    /// # 未指定精度
    ///
    /// 该函数的精度是不确定的；这意味着它会随平台、
    /// Rust 版本变化，甚至同一次执行中不同调用之间也可能不同。
    ///
    /// # 示例
    ///
    /// ```
    /// let angle = std::f32::consts::PI;
    ///
    /// let abs_difference = (angle.to_degrees() - 180.0).abs();
    /// # #[cfg(any(not(target_arch = "x86"), target_feature = "sse2"))]
    /// assert!(abs_difference <= f32::EPSILON);
    /// ```
    #[must_use = "this returns the result of the operation, \
                  without modifying the original"]
    #[stable(feature = "f32_deg_rad_conversions", since = "1.7.0")]
    #[rustc_const_stable(feature = "const_float_methods", since = "1.85.0")]
    #[inline]
    pub const fn to_degrees(self) -> f32 {
        // 使用字面量以避免双重舍入；`consts::PI` 已经舍入，
        // 再进行除法会再次舍入。
        const PIS_IN_180: f32 = 57.2957795130823208767981548141051703_f32;
        self * PIS_IN_180
    }

    /// 将角度转换为弧度。
    ///
    /// # 未指定精度
    ///
    /// 该函数的精度是不确定的；这意味着它会随平台、
    /// Rust 版本变化，甚至同一次执行中不同调用之间也可能不同。
    ///
    /// # 示例
    ///
    /// ```
    /// let angle = 180.0f32;
    ///
    /// let abs_difference = (angle.to_radians() - std::f32::consts::PI).abs();
    ///
    /// assert!(abs_difference <= f32::EPSILON);
    /// ```
    #[must_use = "this returns the result of the operation, \
                  without modifying the original"]
    #[stable(feature = "f32_deg_rad_conversions", since = "1.7.0")]
    #[rustc_const_stable(feature = "const_float_methods", since = "1.85.0")]
    #[inline]
    pub const fn to_radians(self) -> f32 {
        // 这里的除法相对于 π/180 的真实值正确舍入。
        // 虽然 π 是无理数且已经被舍入，但这里发生的双重舍入
        // 恰好能为 `f32` 产生正确结果。
        const RADS_PER_DEG: f32 = consts::PI / 180.0;
        self * RADS_PER_DEG
    }

    /// 返回两个数中的最大值，并忽略 NaN。
    ///
    /// 如果恰好一个参数是 NaN（quiet 或 signaling），则返回另一个参数；
    /// 如果两个参数都是 NaN，则返回 NaN，其位模式会
    /// 按照通常的[算术运算规则](f32#nan-bit-patterns) 选择。如果输入
    /// 比较为相等（例如 `+0.0` 和 `-0.0`），则可能以非确定方式
    /// 返回任一输入。
    ///
    /// NaN 的处理遵循 IEEE 754-2019 中 `maximumNumber` 的语义，将所有
    /// NaN 同等处理以确保操作满足结合律。有符号零的处理
    /// 遵循 IEEE 754-2008 中 `maxNum` 的语义。
    ///
    /// ```
    /// let x = 1.0f32;
    /// let y = 2.0f32;
    ///
    /// assert_eq!(x.max(y), y);
    /// assert_eq!(x.max(f32::NAN), x);
    /// ```
    #[must_use = "this returns the result of the comparison, without modifying either input"]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_float_methods", since = "1.85.0")]
    #[inline]
    pub const fn max(self, other: f32) -> f32 {
        intrinsics::maxnumf32(self, other)
    }

    /// 返回两个数中的最小值，并忽略 NaN。
    ///
    /// 如果恰好一个参数是 NaN（quiet 或 signaling），则返回另一个参数；
    /// 如果两个参数都是 NaN，则返回 NaN，其位模式会
    /// 按照通常的[算术运算规则](f32#nan-bit-patterns) 选择。如果输入
    /// 比较为相等（例如 `+0.0` 和 `-0.0`），则可能以非确定方式
    /// 返回任一输入。
    ///
    /// NaN 的处理遵循 IEEE 754-2019 中 `minimumNumber` 的语义，将所有
    /// NaN 同等处理以确保操作满足结合律。有符号零的处理
    /// 遵循 IEEE 754-2008 中 `minNum` 的语义。
    ///
    /// ```
    /// let x = 1.0f32;
    /// let y = 2.0f32;
    ///
    /// assert_eq!(x.min(y), x);
    /// assert_eq!(x.min(f32::NAN), x);
    /// ```
    #[must_use = "this returns the result of the comparison, without modifying either input"]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_float_methods", since = "1.85.0")]
    #[inline]
    pub const fn min(self, other: f32) -> f32 {
        intrinsics::minnumf32(self, other)
    }

    /// 返回两个数中的最大值，并传播 NaN。
    ///
    /// 如果至少一个参数是 NaN，则返回 NaN，其位模式
    /// 按照通常的[算术运算规则](f32#nan-bit-patterns) 选择。此外，
    /// `-0.0` 被认为小于 `+0.0`，因此该函数对
    /// 非 NaN 输入完全确定。
    ///
    /// 这与 [`f32::max`] 不同：后者只有在*两个*参数都是 NaN 时才返回 NaN，
    /// 且不会可靠地为 `-0.0` 与 `+0.0` 排序。
    ///
    /// 这遵循 IEEE 754-2019 中 `maximum` 的语义。
    ///
    /// ```
    /// #![feature(float_minimum_maximum)]
    /// let x = 1.0f32;
    /// let y = 2.0f32;
    ///
    /// assert_eq!(x.maximum(y), y);
    /// assert!(x.maximum(f32::NAN).is_nan());
    /// ```
    #[must_use = "this returns the result of the comparison, without modifying either input"]
    #[unstable(feature = "float_minimum_maximum", issue = "91079")]
    #[inline]
    pub const fn maximum(self, other: f32) -> f32 {
        intrinsics::maximumf32(self, other)
    }

    /// 返回两个数中的最小值，并传播 NaN。
    ///
    /// 如果至少一个参数是 NaN，则返回 NaN，其位模式
    /// 按照通常的[算术运算规则](f32#nan-bit-patterns) 选择。此外，
    /// `-0.0` 被认为小于 `+0.0`，因此该函数对
    /// 非 NaN 输入完全确定。
    ///
    /// 这与 [`f32::min`] 不同：后者只有在*两个*参数都是 NaN 时才返回 NaN，
    /// 且不会可靠地为 `-0.0` 与 `+0.0` 排序。
    ///
    /// 这遵循 IEEE 754-2019 中 `minimum` 的语义。
    ///
    /// ```
    /// #![feature(float_minimum_maximum)]
    /// let x = 1.0f32;
    /// let y = 2.0f32;
    ///
    /// assert_eq!(x.minimum(y), x);
    /// assert!(x.minimum(f32::NAN).is_nan());
    /// ```
    #[must_use = "this returns the result of the comparison, without modifying either input"]
    #[unstable(feature = "float_minimum_maximum", issue = "91079")]
    #[inline]
    pub const fn minimum(self, other: f32) -> f32 {
        intrinsics::minimumf32(self, other)
    }

    /// 计算 `self` 与 `rhs` 之间的中点（平均值）。
    ///
    /// 如果*任一*参数是 NaN，或者参数组合为
    /// +inf 与 -inf，则返回 NaN。
    ///
    /// # 示例
    ///
    /// ```
    /// assert_eq!(1f32.midpoint(4.0), 2.5);
    /// assert_eq!((-5.5f32).midpoint(8.0), 1.25);
    /// ```
    #[inline]
    #[doc(alias = "average")]
    #[stable(feature = "num_midpoint", since = "1.85.0")]
    #[rustc_const_stable(feature = "num_midpoint", since = "1.85.0")]
    pub const fn midpoint(self, other: f32) -> f32 {
        cfg_select! {
            // 允许使用更快实现的目标必须已知拥有可靠的 64 位浮点
            // 实现。对于没有 64 位硬件浮点或实现有缺陷的目标，
            // 回退到带分支的代码。
            // https://github.com/rust-lang/rust/pull/121062#issuecomment-2123408114
            any(
                target_arch = "x86_64",
                target_arch = "aarch64",
                all(any(target_arch = "riscv32", target_arch = "riscv64"), target_feature = "d"),
                all(target_arch = "loongarch64", target_feature = "d"),
                all(target_arch = "arm", target_feature = "vfp2"),
                target_arch = "wasm32",
                target_arch = "wasm64",
            ) => {
                ((self as f64 + other as f64) / 2.0) as f32
            }
            _ => {
                const HI: f32 = f32::MAX / 2.;

                let (a, b) = (self, other);
                let abs_a = a.abs();
                let abs_b = b.abs();

                if abs_a <= HI && abs_b <= HI {
                    // 溢出不可能发生
                    (a + b) / 2.
                } else {
                    (a / 2.) + (b / 2.)
                }
            }
        }
    }

    /// 向零舍入，并转换为任意原始整数类型，
    /// 前提是该值有限且能放入目标类型。
    ///
    /// ```
    /// let value = 4.6_f32;
    /// let rounded = unsafe { value.to_int_unchecked::<u16>() };
    /// assert_eq!(rounded, 4);
    ///
    /// let value = -128.9_f32;
    /// let rounded = unsafe { value.to_int_unchecked::<i8>() };
    /// assert_eq!(rounded, i8::MIN);
    /// ```
    ///
    /// # 安全性(Safety）
    ///
    /// 该值必须满足：
    ///
    /// * 不是 `NaN`
    /// * 不是无穷
    /// * 截断小数部分之后，能用返回类型 `Int` 表示
    #[must_use = "this returns the result of the operation, \
                  without modifying the original"]
    #[stable(feature = "float_approx_unchecked_to", since = "1.44.0")]
    #[inline]
    pub unsafe fn to_int_unchecked<Int>(self) -> Int
    where
        Self: FloatToInt<Int>,
    {
        // SAFETY: 调用方必须满足
        // `FloatToInt::to_int_unchecked` 的安全契约。
        unsafe { FloatToInt::<Int>::to_int_unchecked(self) }
    }

    /// 原始 transmutation 到 `u32`。
    ///
    /// 目前在所有平台上，它都等同于 `transmute::<f32, u32>(self)`。
    ///
    /// 关于该操作可移植性的讨论见 [`from_bits`](Self::from_bits)
    /// （几乎没有问题）。
    ///
    /// 注意，该函数不同于 `as` 转换；`as` 试图
    /// 保留*数值*，而不是位级值。
    ///
    /// # 示例
    ///
    /// ```
    /// assert_ne!((1f32).to_bits(), 1f32 as u32); // to_bits() is not casting!
    /// assert_eq!((12.5f32).to_bits(), 0x41480000);
    ///
    /// ```
    #[must_use = "this returns the result of the operation, \
                  without modifying the original"]
    #[stable(feature = "float_bits_conv", since = "1.20.0")]
    #[rustc_const_stable(feature = "const_float_bits_conv", since = "1.83.0")]
    #[inline]
    #[allow(unnecessary_transmutes)]
    pub const fn to_bits(self) -> u32 {
        // SAFETY: `u32` 是普通旧数据类型，因此总能 transmute 到它。
        unsafe { mem::transmute(self) }
    }

    /// 从 `u32` 原始 transmutation。
    ///
    /// 目前在所有平台上，它都等同于 `transmute::<u32, f32>(v)`。
    /// 事实证明这具有很强的可移植性，原因有两个：
    ///
    /// * 在所有受支持平台上，浮点数与整数具有相同端序。
    /// * IEEE 754 对浮点数位布局作出了非常精确的规定。
    ///
    /// 但有一个注意点：在 IEEE 754 的 2008 版本之前，
    /// NaN signaling 位的解释方式并未实际指定。大多数平台
    /// （尤其是 x86 和 ARM）选择了后来最终
    /// 在 2008 年标准化的解释，但有些平台并非如此（尤其是 MIPS）。因此，
    /// MIPS 上的 signaling NaN 在 x86 上会是 quiet NaN，反之亦然。
    ///
    /// 该实现并不试图跨平台保留 signaling/quiet 属性，
    /// 而是优先保留精确位模式。这意味着
    /// 即使该方法的结果从一台 x86 机器经网络发送到一台 MIPS 机器，
    /// 编码在 NaN 中的任何 payload 也会保留下来。
    ///
    /// 如果该方法的结果只由产生它们的同一种
    /// 架构继续处理，则不存在可移植性问题。
    ///
    /// 如果输入不是 NaN，则不存在可移植性问题。
    ///
    /// 如果你不关心 signaling/quiet 属性（通常如此），则不存在
    /// 可移植性问题。
    ///
    /// 注意，该函数不同于 `as` 转换；`as` 试图
    /// 保留*数值*，而不是位级值。
    ///
    /// # 示例
    ///
    /// ```
    /// let v = f32::from_bits(0x41480000);
    /// assert_eq!(v, 12.5);
    /// ```
    #[stable(feature = "float_bits_conv", since = "1.20.0")]
    #[rustc_const_stable(feature = "const_float_bits_conv", since = "1.83.0")]
    #[must_use]
    #[inline]
    #[allow(unnecessary_transmutes)]
    pub const fn from_bits(v: u32) -> Self {
        // 事实证明 sNaN 相关的安全问题被高估了。
        // SAFETY: `u32` 是普通旧数据类型，因此总能从它 transmute。
        unsafe { mem::transmute(v) }
    }

    /// 返回该浮点数的内存表示，形式为字节数组，端序为
    /// 大端（网络）字节序。
    ///
    /// 关于该操作可移植性的讨论见 [`from_bits`](Self::from_bits)
    /// （几乎没有问题）。
    ///
    /// # 示例
    ///
    /// ```
    /// let bytes = 12.5f32.to_be_bytes();
    /// assert_eq!(bytes, [0x41, 0x48, 0x00, 0x00]);
    /// ```
    #[must_use = "this returns the result of the operation, \
                  without modifying the original"]
    #[stable(feature = "float_to_from_bytes", since = "1.40.0")]
    #[rustc_const_stable(feature = "const_float_bits_conv", since = "1.83.0")]
    #[inline]
    pub const fn to_be_bytes(self) -> [u8; 4] {
        self.to_bits().to_be_bytes()
    }

    /// 返回该浮点数的内存表示，形式为字节数组，端序为
    /// 小端字节序。
    ///
    /// 关于该操作可移植性的讨论见 [`from_bits`](Self::from_bits)
    /// （几乎没有问题）。
    ///
    /// # 示例
    ///
    /// ```
    /// let bytes = 12.5f32.to_le_bytes();
    /// assert_eq!(bytes, [0x00, 0x00, 0x48, 0x41]);
    /// ```
    #[must_use = "this returns the result of the operation, \
                  without modifying the original"]
    #[stable(feature = "float_to_from_bytes", since = "1.40.0")]
    #[rustc_const_stable(feature = "const_float_bits_conv", since = "1.83.0")]
    #[inline]
    pub const fn to_le_bytes(self) -> [u8; 4] {
        self.to_bits().to_le_bytes()
    }

    /// 返回该浮点数的内存表示，形式为字节数组，端序为
    /// 本机字节序。
    ///
    /// 由于使用目标平台的本机端序，可移植代码
    /// 应按需要改用 [`to_be_bytes`] 或 [`to_le_bytes`]。
    ///
    /// [`to_be_bytes`]: f32::to_be_bytes
    /// [`to_le_bytes`]: f32::to_le_bytes
    ///
    /// 关于该操作可移植性的讨论见 [`from_bits`](Self::from_bits)
    /// （几乎没有问题）。
    ///
    /// # 示例
    ///
    /// ```
    /// let bytes = 12.5f32.to_ne_bytes();
    /// assert_eq!(
    ///     bytes,
    ///     if cfg!(target_endian = "big") {
    ///         [0x41, 0x48, 0x00, 0x00]
    ///     } else {
    ///         [0x00, 0x00, 0x48, 0x41]
    ///     }
    /// );
    /// ```
    #[must_use = "this returns the result of the operation, \
                  without modifying the original"]
    #[stable(feature = "float_to_from_bytes", since = "1.40.0")]
    #[rustc_const_stable(feature = "const_float_bits_conv", since = "1.83.0")]
    #[inline]
    pub const fn to_ne_bytes(self) -> [u8; 4] {
        self.to_bits().to_ne_bytes()
    }

    /// 从大端字节序数组表示创建浮点值。
    ///
    /// 关于该操作可移植性的讨论见 [`from_bits`](Self::from_bits)
    /// （几乎没有问题）。
    ///
    /// # 示例
    ///
    /// ```
    /// let value = f32::from_be_bytes([0x41, 0x48, 0x00, 0x00]);
    /// assert_eq!(value, 12.5);
    /// ```
    #[stable(feature = "float_to_from_bytes", since = "1.40.0")]
    #[rustc_const_stable(feature = "const_float_bits_conv", since = "1.83.0")]
    #[must_use]
    #[inline]
    pub const fn from_be_bytes(bytes: [u8; 4]) -> Self {
        Self::from_bits(u32::from_be_bytes(bytes))
    }

    /// 从小端字节序数组表示创建浮点值。
    ///
    /// 关于该操作可移植性的讨论见 [`from_bits`](Self::from_bits)
    /// （几乎没有问题）。
    ///
    /// # 示例
    ///
    /// ```
    /// let value = f32::from_le_bytes([0x00, 0x00, 0x48, 0x41]);
    /// assert_eq!(value, 12.5);
    /// ```
    #[stable(feature = "float_to_from_bytes", since = "1.40.0")]
    #[rustc_const_stable(feature = "const_float_bits_conv", since = "1.83.0")]
    #[must_use]
    #[inline]
    pub const fn from_le_bytes(bytes: [u8; 4]) -> Self {
        Self::from_bits(u32::from_le_bytes(bytes))
    }

    /// 从本机端序字节数组表示创建浮点值。
    ///
    /// 由于使用目标平台的本机端序，可移植代码
    /// 通常应按需要改用 [`from_be_bytes`] 或 [`from_le_bytes`]，
    /// 以保持可移植性。
    ///
    /// [`from_be_bytes`]: f32::from_be_bytes
    /// [`from_le_bytes`]: f32::from_le_bytes
    ///
    /// 关于该操作可移植性的讨论见 [`from_bits`](Self::from_bits)
    /// （几乎没有问题）。
    ///
    /// # 示例
    ///
    /// ```
    /// let value = f32::from_ne_bytes(if cfg!(target_endian = "big") {
    ///     [0x41, 0x48, 0x00, 0x00]
    /// } else {
    ///     [0x00, 0x00, 0x48, 0x41]
    /// });
    /// assert_eq!(value, 12.5);
    /// ```
    #[stable(feature = "float_to_from_bytes", since = "1.40.0")]
    #[rustc_const_stable(feature = "const_float_bits_conv", since = "1.83.0")]
    #[must_use]
    #[inline]
    pub const fn from_ne_bytes(bytes: [u8; 4]) -> Self {
        Self::from_bits(u32::from_ne_bytes(bytes))
    }

    /// 返回 `self` 与 `other` 之间的排序。
    ///
    /// 不同于浮点数标准偏序比较，
    /// 该比较总是按照
    /// IEEE 754（2008 修订版）浮点标准中定义的 `totalOrder` 谓词
    /// 产生全序。值按以下顺序排列：
    ///
    /// - 负 quiet NaN
    /// - 负 signaling NaN
    /// - 负无穷
    /// - 负数
    /// - 负次正规数
    /// - 负零
    /// - 正零
    /// - 正次正规数
    /// - 正数
    /// - 正无穷
    /// - 正 signaling NaN
    /// - 正 quiet NaN。
    ///
    /// 该函数建立的顺序并不总是与
    /// `f32` 的 [`PartialOrd`] 和 [`PartialEq`] 实现一致。例如，
    /// 这些实现认为负零和正零相等，而 `total_cmp`
    /// 不会。
    ///
    /// 对 signaling NaN 位的解释遵循
    /// IEEE 754 标准的定义，这可能与某些
    /// 较旧且不符合标准的硬件实现（例如 MIPS）的解释不同。
    ///
    /// # 示例
    ///
    /// ```
    /// struct GoodBoy {
    ///     name: String,
    ///     weight: f32,
    /// }
    ///
    /// let mut bois = vec![
    ///     GoodBoy { name: "Pucci".to_owned(), weight: 0.1 },
    ///     GoodBoy { name: "Woofer".to_owned(), weight: 99.0 },
    ///     GoodBoy { name: "Yapper".to_owned(), weight: 10.0 },
    ///     GoodBoy { name: "Chonk".to_owned(), weight: f32::INFINITY },
    ///     GoodBoy { name: "Abs. Unit".to_owned(), weight: f32::NAN },
    ///     GoodBoy { name: "Floaty".to_owned(), weight: -5.0 },
    /// ];
    ///
    /// bois.sort_by(|a, b| a.weight.total_cmp(&b.weight));
    ///
    /// // `f32::NAN` could be positive or negative, which will affect the sort order.
    /// if f32::NAN.is_sign_negative() {
    ///     assert!(bois.into_iter().map(|b| b.weight)
    ///         .zip([f32::NAN, -5.0, 0.1, 10.0, 99.0, f32::INFINITY].iter())
    ///         .all(|(a, b)| a.to_bits() == b.to_bits()))
    /// } else {
    ///     assert!(bois.into_iter().map(|b| b.weight)
    ///         .zip([-5.0, 0.1, 10.0, 99.0, f32::INFINITY, f32::NAN].iter())
    ///         .all(|(a, b)| a.to_bits() == b.to_bits()))
    /// }
    /// ```
    #[stable(feature = "total_cmp", since = "1.62.0")]
    #[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
    #[must_use]
    #[inline]
    pub const fn total_cmp(&self, other: &Self) -> crate::cmp::Ordering {
        let mut left = self.to_bits() as i32;
        let mut right = other.to_bits() as i32;

        // 对于负数，翻转除符号位以外的所有位，
        // 从而得到类似二进制补码整数的布局
        //
        // 为什么这样可行？IEEE 754 浮点数由三个字段组成：
        // 符号位, exponent and mantissa. The set of exponent and mantissa
        // 字段整体具有一个性质：其按位顺序
        // 等于已定义数值大小处的数值幅度顺序。
        // NaN 值通常没有定义幅度，但
        // IEEE 754 totalOrder 也定义 NaN 值遵循
        // 按位顺序。这就得到文档注释中说明的排序。
        // 不过，负数和正数的幅度表示相同，
        // 只有符号位不同。
        // 为了便于把浮点数当作有符号整数比较，
        // 需要在负数情形下翻转指数和尾数位。
        // 这实际上把这些数转换成了“二进制补码”形式。
        //
        // 为了执行翻转，我们构造一个掩码并与其异或。
        // 我们以无分支方式从负号值计算出“除符号位外全为 1”的
        // 掩码：右移会进行符号扩展，
        // 因此用符号位“填满”掩码，然后
        // 转换为无符号值以再推入一个零位。
        // 对于正值，掩码全为零，因此这是空操作。
        left ^= (((left >> 31) as u32) >> 1) as i32;
        right ^= (((right >> 31) as u32) >> 1) as i32;

        left.cmp(&right)
    }

    /// 将值限制在某个区间内，但 NaN 除外。
    ///
    /// 如果 `self` 大于 `max` 则返回 `max`；如果 `self`
    /// 小于 `min` 则返回 `min`；否则返回 `self`。
    ///
    /// 注意，如果初始值是 NaN，该函数也返回 NaN。
    /// 如果结果为零，并且 `self`、`min`、`max` 三个输入中存在
    /// zeros with different sign, either `0.0` or `-0.0` is returned 返回任一输入。
    ///
    /// # Panics
    ///
    /// 如果 `min > max`、`min` 是 NaN 或 `max` 是 NaN，则 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// assert!((-3.0f32).clamp(-2.0, 1.0) == -2.0);
    /// assert!((0.0f32).clamp(-2.0, 1.0) == 0.0);
    /// assert!((2.0f32).clamp(-2.0, 1.0) == 1.0);
    /// assert!((f32::NAN).clamp(-2.0, 1.0).is_nan());
    ///
    /// // These always returns zero, but the sign (which is ignored by `==`) is non-deterministic.
    /// assert!((0.0f32).clamp(-0.0, -0.0) == 0.0);
    /// assert!((1.0f32).clamp(-0.0, 0.0) == 0.0);
    /// // This is definitely a negative zero.
    /// assert!((-1.0f32).clamp(-0.0, 1.0).is_sign_negative());
    /// ```
    #[must_use = "method returns a new number and does not mutate the original value"]
    #[stable(feature = "clamp", since = "1.50.0")]
    #[rustc_const_stable(feature = "const_float_methods", since = "1.85.0")]
    #[inline]
    pub const fn clamp(mut self, min: f32, max: f32) -> f32 {
        const_assert!(
            min <= max,
            "min > max, or either was NaN",
            "min > max, or either was NaN. min = {min:?}, max = {max:?}",
            min: f32,
            max: f32,
        );

        if self < min {
            self = min;
        }
        if self > max {
            self = max;
        }
        self
    }

    /// 把该数限制在以零为中心的对称范围内。
    ///
    /// 该方法把该数的幅度（绝对值）限制为不超过 `limit`。
    ///
    /// 它在功能上等价于 `self.clamp(-limit, limit)`，但更
    /// 明确表达调用意图。
    ///
    /// # Panics
    ///
    /// 如果 `limit` 为负或 NaN，则 panic，因为这表示逻辑错误。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(clamp_magnitude)]
    /// assert_eq!(5.0f32.clamp_magnitude(3.0), 3.0);
    /// assert_eq!((-5.0f32).clamp_magnitude(3.0), -3.0);
    /// assert_eq!(2.0f32.clamp_magnitude(3.0), 2.0);
    /// assert_eq!((-2.0f32).clamp_magnitude(3.0), -2.0);
    /// ```
    #[must_use = "this returns the clamped value and does not modify the original"]
    #[unstable(feature = "clamp_magnitude", issue = "148519")]
    #[inline]
    pub fn clamp_magnitude(self, limit: f32) -> f32 {
        assert!(limit >= 0.0, "limit must be non-negative");
        let limit = limit.abs(); // Canonicalises -0.0 to 0.0
        self.clamp(-limit, limit)
    }

    /// 计算 `self` 的绝对值。
    ///
    /// 该函数总是返回精确结果。
    ///
    /// # 示例
    ///
    /// ```
    /// let x = 3.5_f32;
    /// let y = -3.5_f32;
    ///
    /// assert_eq!(x.abs(), x);
    /// assert_eq!(y.abs(), -y);
    ///
    /// assert!(f32::NAN.abs().is_nan());
    /// ```
    #[must_use = "method returns a new number and does not mutate the original value"]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_float_methods", since = "1.85.0")]
    #[inline]
    pub const fn abs(self) -> f32 {
        intrinsics::fabsf32(self)
    }

    /// 返回一个表示 `self` 符号的数。
    ///
    /// - 如果该数为正、`+0.0` 或 `INFINITY`，返回 `1.0`
    /// - 如果该数为负、`-0.0` 或 `NEG_INFINITY`，返回 `-1.0`
    /// - 如果该数是 NaN，则返回 NaN
    ///
    /// # 示例
    ///
    /// ```
    /// let f = 3.5_f32;
    ///
    /// assert_eq!(f.signum(), 1.0);
    /// assert_eq!(f32::NEG_INFINITY.signum(), -1.0);
    ///
    /// assert!(f32::NAN.signum().is_nan());
    /// ```
    #[must_use = "method returns a new number and does not mutate the original value"]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_float_methods", since = "1.85.0")]
    #[inline]
    pub const fn signum(self) -> f32 {
        if self.is_nan() { Self::NAN } else { 1.0_f32.copysign(self) }
    }

    /// 返回一个由 `self` 的幅度和
    /// `sign` 的符号组成的数。
    ///
    /// 如果 `self` 与 `sign` 的符号相同，则等于 `self`；否则等于 `-self`。
    /// 如果 `self` 是 NaN，则返回一个 payload 与 `self` 相同、符号位来自 `sign` 的 NaN。
    ///
    ///
    /// 如果 `sign` 是 NaN，该操作仍会把它的符号带到结果中。注意，
    /// IEEE 754 并不为 NaN 情形下的符号位赋予数学含义，而 Rust
    /// 也不保证 NaN 的位模式会在算术运算中保持不变，
    /// 当 `sign` 为 NaN 时，`copysign` 的结果可能产生意外或不可移植的
    /// 结果。更多信息见 [NaN 位模式规范](primitive@f32#nan-bit-patterns)。
    ///
    ///
    /// # 示例
    ///
    /// ```
    /// let f = 3.5_f32;
    ///
    /// assert_eq!(f.copysign(0.42), 3.5_f32);
    /// assert_eq!(f.copysign(-0.42), -3.5_f32);
    /// assert_eq!((-f).copysign(0.42), 3.5_f32);
    /// assert_eq!((-f).copysign(-0.42), -3.5_f32);
    ///
    /// assert!(f32::NAN.copysign(1.0).is_nan());
    /// ```
    #[must_use = "method returns a new number and does not mutate the original value"]
    #[inline]
    #[stable(feature = "copysign", since = "1.35.0")]
    #[rustc_const_stable(feature = "const_float_methods", since = "1.85.0")]
    pub const fn copysign(self, sign: f32) -> f32 {
        intrinsics::copysignf32(self, sign)
    }

    /// 允许基于代数规则进行优化的浮点加法。
    ///
    /// 更多信息见[代数运算符](primitive@f32#algebraic-operators)。
    #[must_use = "method returns a new number and does not mutate the original value"]
    #[unstable(feature = "float_algebraic", issue = "136469")]
    #[rustc_const_unstable(feature = "float_algebraic", issue = "136469")]
    #[inline]
    pub const fn algebraic_add(self, rhs: f32) -> f32 {
        intrinsics::fadd_algebraic(self, rhs)
    }

    /// 允许基于代数规则进行优化的浮点减法。
    ///
    /// 更多信息见[代数运算符](primitive@f32#algebraic-operators)。
    #[must_use = "method returns a new number and does not mutate the original value"]
    #[unstable(feature = "float_algebraic", issue = "136469")]
    #[rustc_const_unstable(feature = "float_algebraic", issue = "136469")]
    #[inline]
    pub const fn algebraic_sub(self, rhs: f32) -> f32 {
        intrinsics::fsub_algebraic(self, rhs)
    }

    /// 允许基于代数规则进行优化的浮点乘法。
    ///
    /// 更多信息见[代数运算符](primitive@f32#algebraic-operators)。
    #[must_use = "method returns a new number and does not mutate the original value"]
    #[unstable(feature = "float_algebraic", issue = "136469")]
    #[rustc_const_unstable(feature = "float_algebraic", issue = "136469")]
    #[inline]
    pub const fn algebraic_mul(self, rhs: f32) -> f32 {
        intrinsics::fmul_algebraic(self, rhs)
    }

    /// 允许基于代数规则进行优化的浮点除法。
    ///
    /// 更多信息见[代数运算符](primitive@f32#algebraic-operators)。
    #[must_use = "method returns a new number and does not mutate the original value"]
    #[unstable(feature = "float_algebraic", issue = "136469")]
    #[rustc_const_unstable(feature = "float_algebraic", issue = "136469")]
    #[inline]
    pub const fn algebraic_div(self, rhs: f32) -> f32 {
        intrinsics::fdiv_algebraic(self, rhs)
    }

    /// 允许基于代数规则进行优化的浮点取余。
    ///
    /// 更多信息见[代数运算符](primitive@f32#algebraic-operators)。
    #[must_use = "method returns a new number and does not mutate the original value"]
    #[unstable(feature = "float_algebraic", issue = "136469")]
    #[rustc_const_unstable(feature = "float_algebraic", issue = "136469")]
    #[inline]
    pub const fn algebraic_rem(self, rhs: f32) -> f32 {
        intrinsics::frem_algebraic(self, rhs)
    }
}

/// `core` 中浮点函数的实验性实现。
///
/// _本模块中的独立函数仅用于测试。
/// 它们将作为固有方法稳定下来。_
#[unstable(feature = "core_float_math", issue = "137578")]
pub mod math {
    use crate::intrinsics;
    use crate::num::libm;

    /// `core` 中 `floor` 的实验版本。详细行为见 [`f32::floor`]。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(core_float_math)]
    ///
    /// use core::f32;
    ///
    /// let f = 3.7_f32;
    /// let g = 3.0_f32;
    /// let h = -3.7_f32;
    ///
    /// assert_eq!(f32::math::floor(f), 3.0);
    /// assert_eq!(f32::math::floor(g), 3.0);
    /// assert_eq!(f32::math::floor(h), -4.0);
    /// ```
    ///
    /// _这个独立函数仅用于测试。
    /// 它将作为固有方法稳定下来。_
    ///
    /// [`f32::floor`]: ../../../std/primitive.f32.html#method.floor
    #[inline]
    #[unstable(feature = "core_float_math", issue = "137578")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    pub const fn floor(x: f32) -> f32 {
        intrinsics::floorf32(x)
    }

    /// `core` 中 `ceil` 的实验版本。详细行为见 [`f32::ceil`]。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(core_float_math)]
    ///
    /// use core::f32;
    ///
    /// let f = 3.01_f32;
    /// let g = 4.0_f32;
    ///
    /// assert_eq!(f32::math::ceil(f), 4.0);
    /// assert_eq!(f32::math::ceil(g), 4.0);
    /// ```
    ///
    /// _这个独立函数仅用于测试。
    /// 它将作为固有方法稳定下来。_
    ///
    /// [`f32::ceil`]: ../../../std/primitive.f32.html#method.ceil
    #[inline]
    #[doc(alias = "ceiling")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    #[unstable(feature = "core_float_math", issue = "137578")]
    pub const fn ceil(x: f32) -> f32 {
        intrinsics::ceilf32(x)
    }

    /// `core` 中 `round` 的实验版本。详细行为见 [`f32::round`]。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(core_float_math)]
    ///
    /// use core::f32;
    ///
    /// let f = 3.3_f32;
    /// let g = -3.3_f32;
    /// let h = -3.7_f32;
    /// let i = 3.5_f32;
    /// let j = 4.5_f32;
    ///
    /// assert_eq!(f32::math::round(f), 3.0);
    /// assert_eq!(f32::math::round(g), -3.0);
    /// assert_eq!(f32::math::round(h), -4.0);
    /// assert_eq!(f32::math::round(i), 4.0);
    /// assert_eq!(f32::math::round(j), 5.0);
    /// ```
    ///
    /// _这个独立函数仅用于测试。
    /// 它将作为固有方法稳定下来。_
    ///
    /// [`f32::round`]: ../../../std/primitive.f32.html#method.round
    #[inline]
    #[unstable(feature = "core_float_math", issue = "137578")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    pub const fn round(x: f32) -> f32 {
        intrinsics::roundf32(x)
    }

    /// `core` 中 `round_ties_even` 的实验版本。详细行为见 [`f32::round_ties_even`]。
    /// 详细信息。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(core_float_math)]
    ///
    /// use core::f32;
    ///
    /// let f = 3.3_f32;
    /// let g = -3.3_f32;
    /// let h = 3.5_f32;
    /// let i = 4.5_f32;
    ///
    /// assert_eq!(f32::math::round_ties_even(f), 3.0);
    /// assert_eq!(f32::math::round_ties_even(g), -3.0);
    /// assert_eq!(f32::math::round_ties_even(h), 4.0);
    /// assert_eq!(f32::math::round_ties_even(i), 4.0);
    /// ```
    ///
    /// _这个独立函数仅用于测试。
    /// 它将作为固有方法稳定下来。_
    ///
    /// [`f32::round_ties_even`]: ../../../std/primitive.f32.html#method.round_ties_even
    #[inline]
    #[unstable(feature = "core_float_math", issue = "137578")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    pub const fn round_ties_even(x: f32) -> f32 {
        intrinsics::round_ties_even_f32(x)
    }

    /// `core` 中 `trunc` 的实验版本。详细行为见 [`f32::trunc`]。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(core_float_math)]
    ///
    /// use core::f32;
    ///
    /// let f = 3.7_f32;
    /// let g = 3.0_f32;
    /// let h = -3.7_f32;
    ///
    /// assert_eq!(f32::math::trunc(f), 3.0);
    /// assert_eq!(f32::math::trunc(g), 3.0);
    /// assert_eq!(f32::math::trunc(h), -3.0);
    /// ```
    ///
    /// _这个独立函数仅用于测试。
    /// 它将作为固有方法稳定下来。_
    ///
    /// [`f32::trunc`]: ../../../std/primitive.f32.html#method.trunc
    #[inline]
    #[doc(alias = "truncate")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    #[unstable(feature = "core_float_math", issue = "137578")]
    pub const fn trunc(x: f32) -> f32 {
        intrinsics::truncf32(x)
    }

    /// `core` 中 `fract` 的实验版本。详细行为见 [`f32::fract`]。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(core_float_math)]
    ///
    /// use core::f32;
    ///
    /// let x = 3.6_f32;
    /// let y = -3.6_f32;
    /// let abs_difference_x = (f32::math::fract(x) - 0.6).abs();
    /// let abs_difference_y = (f32::math::fract(y) - (-0.6)).abs();
    ///
    /// assert!(abs_difference_x <= f32::EPSILON);
    /// assert!(abs_difference_y <= f32::EPSILON);
    /// ```
    ///
    /// _这个独立函数仅用于测试。
    /// 它将作为固有方法稳定下来。_
    ///
    /// [`f32::fract`]: ../../../std/primitive.f32.html#method.fract
    #[inline]
    #[unstable(feature = "core_float_math", issue = "137578")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    pub const fn fract(x: f32) -> f32 {
        x - trunc(x)
    }

    /// `core` 中 `mul_add` 的实验版本。详细行为见 [`f32::mul_add`]。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(core_float_math)]
    ///
    /// # // FIXME(#140515): mingw has an incorrect fma
    /// # // https://sourceforge.net/p/mingw-w64/bugs/848/
    /// # #[cfg(all(target_os = "windows", target_env = "gnu", not(target_abi = "llvm")))] {
    /// use core::f32;
    ///
    /// let m = 10.0_f32;
    /// let x = 4.0_f32;
    /// let b = 60.0_f32;
    ///
    /// assert_eq!(f32::math::mul_add(m, x, b), 100.0);
    /// assert_eq!(m * x + b, 100.0);
    ///
    /// let one_plus_eps = 1.0_f32 + f32::EPSILON;
    /// let one_minus_eps = 1.0_f32 - f32::EPSILON;
    /// let minus_one = -1.0_f32;
    ///
    /// // The exact result (1 + eps) * (1 - eps) = 1 - eps * eps.
    /// assert_eq!(
    ///     f32::math::mul_add(one_plus_eps, one_minus_eps, minus_one),
    ///     -f32::EPSILON * f32::EPSILON
    /// );
    /// // Different rounding with the non-fused multiply and add.
    /// assert_eq!(one_plus_eps * one_minus_eps + minus_one, 0.0);
    /// # }
    /// ```
    ///
    /// _这个独立函数仅用于测试。
    /// 它将作为固有方法稳定下来。_
    ///
    /// [`f32::mul_add`]: ../../../std/primitive.f32.html#method.mul_add
    #[inline]
    #[doc(alias = "fmaf", alias = "fusedMultiplyAdd")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    #[unstable(feature = "core_float_math", issue = "137578")]
    pub const fn mul_add(x: f32, y: f32, z: f32) -> f32 {
        intrinsics::fmaf32(x, y, z)
    }

    /// `core` 中 `div_euclid` 的实验版本。详细行为见 [`f32::div_euclid`]。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(core_float_math)]
    ///
    /// use core::f32;
    ///
    /// let a: f32 = 7.0;
    /// let b = 4.0;
    /// assert_eq!(f32::math::div_euclid(a, b), 1.0); // 7.0 > 4.0 * 1.0
    /// assert_eq!(f32::math::div_euclid(-a, b), -2.0); // -7.0 >= 4.0 * -2.0
    /// assert_eq!(f32::math::div_euclid(a, -b), -1.0); // 7.0 >= -4.0 * -1.0
    /// assert_eq!(f32::math::div_euclid(-a, -b), 2.0); // -7.0 >= -4.0 * 2.0
    /// ```
    ///
    /// _这个独立函数仅用于测试。
    /// 它将作为固有方法稳定下来。_
    ///
    /// [`f32::div_euclid`]: ../../../std/primitive.f32.html#method.div_euclid
    #[inline]
    #[unstable(feature = "core_float_math", issue = "137578")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    pub fn div_euclid(x: f32, rhs: f32) -> f32 {
        let q = trunc(x / rhs);
        if x % rhs < 0.0 {
            return if rhs > 0.0 { q - 1.0 } else { q + 1.0 };
        }
        q
    }

    /// `core` 中 `rem_euclid` 的实验版本。详细行为见 [`f32::rem_euclid`]。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(core_float_math)]
    ///
    /// use core::f32;
    ///
    /// let a: f32 = 7.0;
    /// let b = 4.0;
    /// assert_eq!(f32::math::rem_euclid(a, b), 3.0);
    /// assert_eq!(f32::math::rem_euclid(-a, b), 1.0);
    /// assert_eq!(f32::math::rem_euclid(a, -b), 3.0);
    /// assert_eq!(f32::math::rem_euclid(-a, -b), 1.0);
    /// // limitation due to round-off error
    /// assert!(f32::math::rem_euclid(-f32::EPSILON, 3.0) != 0.0);
    /// ```
    ///
    /// _这个独立函数仅用于测试。
    /// 它将作为固有方法稳定下来。_
    ///
    /// [`f32::rem_euclid`]: ../../../std/primitive.f32.html#method.rem_euclid
    #[inline]
    #[doc(alias = "modulo", alias = "mod")]
    #[unstable(feature = "core_float_math", issue = "137578")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    pub fn rem_euclid(x: f32, rhs: f32) -> f32 {
        let r = x % rhs;
        if r < 0.0 { r + rhs.abs() } else { r }
    }

    /// `core` 中 `powi` 的实验版本。详细行为见 [`f32::powi`]。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(core_float_math)]
    ///
    /// use core::f32;
    ///
    /// let x = 2.0_f32;
    /// let abs_difference = (f32::math::powi(x, 2) - (x * x)).abs();
    /// assert!(abs_difference <= 1e-5);
    ///
    /// assert_eq!(f32::math::powi(f32::NAN, 0), 1.0);
    /// ```
    ///
    /// _这个独立函数仅用于测试。
    /// 它将作为固有方法稳定下来。_
    ///
    /// [`f32::powi`]: ../../../std/primitive.f32.html#method.powi
    #[inline]
    #[must_use = "method returns a new number and does not mutate the original value"]
    #[unstable(feature = "core_float_math", issue = "137578")]
    pub fn powi(x: f32, n: i32) -> f32 {
        intrinsics::powif32(x, n)
    }

    /// `core` 中 `sqrt` 的实验版本。详细行为见 [`f32::sqrt`]。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(core_float_math)]
    ///
    /// use core::f32;
    ///
    /// let positive = 4.0_f32;
    /// let negative = -4.0_f32;
    /// let negative_zero = -0.0_f32;
    ///
    /// assert_eq!(f32::math::sqrt(positive), 2.0);
    /// assert!(f32::math::sqrt(negative).is_nan());
    /// assert_eq!(f32::math::sqrt(negative_zero), negative_zero);
    /// ```
    ///
    /// _这个独立函数仅用于测试。
    /// 它将作为固有方法稳定下来。_
    ///
    /// [`f32::sqrt`]: ../../../std/primitive.f32.html#method.sqrt
    #[inline]
    #[doc(alias = "squareRoot")]
    #[unstable(feature = "core_float_math", issue = "137578")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    pub fn sqrt(x: f32) -> f32 {
        intrinsics::sqrtf32(x)
    }

    /// `core` 中 `abs_sub` 的实验版本。详细行为见 [`f32::abs_sub`]。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(core_float_math)]
    ///
    /// use core::f32;
    ///
    /// let x = 3.0f32;
    /// let y = -3.0f32;
    ///
    /// let abs_difference_x = (f32::math::abs_sub(x, 1.0) - 2.0).abs();
    /// let abs_difference_y = (f32::math::abs_sub(y, 1.0) - 0.0).abs();
    ///
    /// assert!(abs_difference_x <= 1e-6);
    /// assert!(abs_difference_y <= 1e-6);
    /// ```
    ///
    /// _这个独立函数仅用于测试。
    /// 它将作为固有方法稳定下来。_
    ///
    /// [`f32::abs_sub`]: ../../../std/primitive.f32.html#method.abs_sub
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
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
    #[must_use = "method returns a new number and does not mutate the original value"]
    pub fn abs_sub(x: f32, other: f32) -> f32 {
        libm::fdimf(x, other)
    }

    /// `core` 中 `cbrt` 的实验版本。详细行为见 [`f32::cbrt`]。
    ///
    /// # 未指定精度
    ///
    /// 该函数的精度是不确定的；这意味着它会随平台、Rust 版本变化，
    /// 甚至同一次执行中不同调用之间也可能不同。
    /// 该函数当前在 Unix 和 Windows 上对应 libc 的 `cbrtf`，
    /// 但未来可能改变。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(core_float_math)]
    ///
    /// use core::f32;
    ///
    /// let x = 8.0f32;
    ///
    /// // x^(1/3) - 2 == 0
    /// let abs_difference = (f32::math::cbrt(x) - 2.0).abs();
    ///
    /// assert!(abs_difference <= 1e-6);
    /// ```
    ///
    /// _这个独立函数仅用于测试。
    /// 它将作为固有方法稳定下来。_
    ///
    /// [`f32::cbrt`]: ../../../std/primitive.f32.html#method.cbrt
    #[inline]
    #[must_use = "method returns a new number and does not mutate the original value"]
    #[unstable(feature = "core_float_math", issue = "137578")]
    pub fn cbrt(x: f32) -> f32 {
        libm::cbrtf(x)
    }
}
