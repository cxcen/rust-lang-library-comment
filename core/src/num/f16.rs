//! `f16` 浮点类型的常量。
//!
//! *另请参阅 [`f16` 原始类型][f16]。*
//!
//! 具有数学意义的常数位于 `consts` 子模块中。
//!
//! 对于直接定义在本模块中的常量
//! （不同于 `consts` 子模块中的数学常数），
//! 新代码应改用
//! 直接定义在 `f16` 类型上的关联常量。

#![unstable(feature = "f16", issue = "116909")]

use crate::convert::FloatToInt;
use crate::num::FpCategory;
#[cfg(not(test))]
use crate::num::libm;
use crate::panic::const_assert;
use crate::{intrinsics, mem};

/// 基本数学常数。
#[unstable(feature = "f16", issue = "116909")]
#[rustc_diagnostic_item = "f16_consts_mod"]
pub mod consts {
    // FIXME: 后续可替换为来自 cmath 的数学常数。

    /// 阿基米德常数（π）
    #[unstable(feature = "f16", issue = "116909")]
    pub const PI: f16 = 3.14159265358979323846264338327950288_f16;

    /// 整圆常数（τ）
    ///
    /// 等于 2π。
    #[unstable(feature = "f16", issue = "116909")]
    pub const TAU: f16 = 6.28318530717958647692528676655900577_f16;

    /// 黄金比例（φ）
    #[unstable(feature = "f16", issue = "116909")]
    pub const GOLDEN_RATIO: f16 = 1.618033988749894848204586834365638118_f16;

    /// 欧拉-马歇罗尼常数（γ）
    #[unstable(feature = "f16", issue = "116909")]
    pub const EULER_GAMMA: f16 = 0.577215664901532860606512090082402431_f16;

    /// π/2
    #[unstable(feature = "f16", issue = "116909")]
    pub const FRAC_PI_2: f16 = 1.57079632679489661923132169163975144_f16;

    /// π/3
    #[unstable(feature = "f16", issue = "116909")]
    pub const FRAC_PI_3: f16 = 1.04719755119659774615421446109316763_f16;

    /// π/4
    #[unstable(feature = "f16", issue = "116909")]
    pub const FRAC_PI_4: f16 = 0.785398163397448309615660845819875721_f16;

    /// π/6
    #[unstable(feature = "f16", issue = "116909")]
    pub const FRAC_PI_6: f16 = 0.52359877559829887307710723054658381_f16;

    /// π/8
    #[unstable(feature = "f16", issue = "116909")]
    pub const FRAC_PI_8: f16 = 0.39269908169872415480783042290993786_f16;

    /// 1/π
    #[unstable(feature = "f16", issue = "116909")]
    pub const FRAC_1_PI: f16 = 0.318309886183790671537767526745028724_f16;

    /// 1/sqrt（π）
    #[unstable(feature = "f16", issue = "116909")]
    // 同样适用：#[unstable(feature = "more_float_constants", issue = "146939")]
    pub const FRAC_1_SQRT_PI: f16 = 0.564189583547756286948079451560772586_f16;

    /// 1/sqrt（2π）
    #[doc(alias = "FRAC_1_SQRT_TAU")]
    #[unstable(feature = "f16", issue = "116909")]
    // 同样适用：#[unstable(feature = "more_float_constants", issue = "146939")]
    pub const FRAC_1_SQRT_2PI: f16 = 0.398942280401432677939946059934381868_f16;

    /// 2/π
    #[unstable(feature = "f16", issue = "116909")]
    pub const FRAC_2_PI: f16 = 0.636619772367581343075535053490057448_f16;

    /// 2/sqrt（π）
    #[unstable(feature = "f16", issue = "116909")]
    pub const FRAC_2_SQRT_PI: f16 = 1.12837916709551257389615890312154517_f16;

    /// sqrt（2）
    #[unstable(feature = "f16", issue = "116909")]
    pub const SQRT_2: f16 = 1.41421356237309504880168872420969808_f16;

    /// 1/sqrt（2）
    #[unstable(feature = "f16", issue = "116909")]
    pub const FRAC_1_SQRT_2: f16 = 0.707106781186547524400844362104849039_f16;

    /// sqrt（3）
    #[unstable(feature = "f16", issue = "116909")]
    // 同样适用：#[unstable(feature = "more_float_constants", issue = "146939")]
    pub const SQRT_3: f16 = 1.732050807568877293527446341505872367_f16;

    /// 1/sqrt（3）
    #[unstable(feature = "f16", issue = "116909")]
    // 同样适用：#[unstable(feature = "more_float_constants", issue = "146939")]
    pub const FRAC_1_SQRT_3: f16 = 0.577350269189625764509148780501957456_f16;

    /// 欧拉数（e）
    #[unstable(feature = "f16", issue = "116909")]
    pub const E: f16 = 2.71828182845904523536028747135266250_f16;

    /// log<sub>2</sub>(10)
    #[unstable(feature = "f16", issue = "116909")]
    pub const LOG2_10: f16 = 3.32192809488736234787031942948939018_f16;

    /// log<sub>2</sub>(e)
    #[unstable(feature = "f16", issue = "116909")]
    pub const LOG2_E: f16 = 1.44269504088896340735992468100189214_f16;

    /// log<sub>10</sub>(2)
    #[unstable(feature = "f16", issue = "116909")]
    pub const LOG10_2: f16 = 0.301029995663981195213738894724493027_f16;

    /// log<sub>10</sub>(e)
    #[unstable(feature = "f16", issue = "116909")]
    pub const LOG10_E: f16 = 0.434294481903251827651128918916605082_f16;

    /// ln(2)
    #[unstable(feature = "f16", issue = "116909")]
    pub const LN_2: f16 = 0.693147180559945309417232121458176568_f16;

    /// ln(10)
    #[unstable(feature = "f16", issue = "116909")]
    pub const LN_10: f16 = 2.30258509299404568401799145468436421_f16;
}

impl f16 {
    // FIXME(f16_f128): 这个 `impl` 中几乎所有方法都还缺少示例和 `const`
    // 实现。等所有平台都能运行相关代码，且 CTFE 支持 f16/f128 后再补齐。

    /// `f16` 内部表示使用的基数。
    #[unstable(feature = "f16", issue = "116909")]
    pub const RADIX: u32 = 2;

    /// 以 2 为底的有效数字位数。
    ///
    /// 注意，按位表示里的尾数字段大小比这个值小 1，
    /// 因为前导 1 不会显式存储。
    #[unstable(feature = "f16", issue = "116909")]
    pub const MANTISSA_DIGITS: u32 = 11;

    /// 以 10 为底时近似的有效十进制数字位数。
    ///
    /// 这是最大的 <i>x</i>，使得任何具有 <i>x</i>
    /// 位有效数字的十进制数都能无损转换为 `f16` 再转换回来。
    ///
    /// 等于 floor(log<sub>10</sub>&nbsp;2<sup>[`MANTISSA_DIGITS`]&nbsp;&minus;&nbsp;1</sup>)。
    ///
    /// [`MANTISSA_DIGITS`]: f16::MANTISSA_DIGITS
    #[unstable(feature = "f16", issue = "116909")]
    pub const DIGITS: u32 = 3;

    /// `f16` 的 [Machine epsilon] 值。
    ///
    /// 这是 `1.0` 与下一个更大的可表示数之间的差值。
    ///
    /// 等于 2<sup>1&nbsp;&minus;&nbsp;[`MANTISSA_DIGITS`]</sup>。
    ///
    /// [Machine epsilon]: https://en.wikipedia.org/wiki/Machine_epsilon
    /// [`MANTISSA_DIGITS`]: f16::MANTISSA_DIGITS
    #[unstable(feature = "f16", issue = "116909")]
    #[rustc_diagnostic_item = "f16_epsilon"]
    pub const EPSILON: f16 = 9.7656e-4_f16;

    /// 最小的有限 `f16` 值。
    ///
    /// 等于 &minus;[`MAX`]。
    ///
    /// [`MAX`]: f16::MAX
    #[unstable(feature = "f16", issue = "116909")]
    pub const MIN: f16 = -6.5504e+4_f16;
    /// 最小的正正规 `f16` 值。
    ///
    /// 等于 2<sup>[`MIN_EXP`]&nbsp;&minus;&nbsp;1</sup>。
    ///
    /// [`MIN_EXP`]: f16::MIN_EXP
    #[unstable(feature = "f16", issue = "116909")]
    pub const MIN_POSITIVE: f16 = 6.1035e-5_f16;
    /// 最大的有限 `f16` 值。
    ///
    /// 等于
    /// (1&nbsp;&minus;&nbsp;2<sup>&minus;[`MANTISSA_DIGITS`]</sup>)&nbsp;2<sup>[`MAX_EXP`]</sup>。
    ///
    /// [`MANTISSA_DIGITS`]: f16::MANTISSA_DIGITS
    /// [`MAX_EXP`]: f16::MAX_EXP
    #[unstable(feature = "f16", issue = "116909")]
    pub const MAX: f16 = 6.5504e+4_f16;

    /// 比可能的最小*正规* 2 次幂指数大 1 的值
    /// 其中有效数范围为 1 ≤ x < 2（即 IEEE 定义）。
    ///
    /// 这对应于可能的精确最小*正规* 2 次幂指数，
    /// 其中有效数范围为 0.5 ≤ x < 1（即 C 定义）。
    /// 换言之，该类型能表示的所有正规数都
    /// 大于或等于 0.5&nbsp;×&nbsp;2<sup><i>MIN_EXP</i></sup>。
    #[unstable(feature = "f16", issue = "116909")]
    pub const MIN_EXP: i32 = -13;
    /// 比可能的最大 2 次幂指数大 1 的值
    /// 其中有效数范围为 1 ≤ x < 2（即 IEEE 定义）。
    ///
    /// 这对应于可能的精确最大 2 次幂指数，
    /// 其中有效数范围为 0.5 ≤ x < 1（即 C 定义）。
    /// 换言之，该类型能表示的所有数都
    /// 严格小于 2<sup><i>MAX_EXP</i></sup>。
    #[unstable(feature = "f16", issue = "116909")]
    pub const MAX_EXP: i32 = 16;

    /// 使 10<sup><i>x</i></sup> 成为正规数的最小 <i>x</i>。
    ///
    /// 等于 ceil(log<sub>10</sub>&nbsp;[`MIN_POSITIVE`])。
    ///
    /// [`MIN_POSITIVE`]: f16::MIN_POSITIVE
    #[unstable(feature = "f16", issue = "116909")]
    pub const MIN_10_EXP: i32 = -4;
    /// 使 10<sup><i>x</i></sup> 成为正规数的最大 <i>x</i>。
    ///
    /// 等于 floor(log<sub>10</sub>&nbsp;[`MAX`])。
    ///
    /// [`MAX`]: f16::MAX
    #[unstable(feature = "f16", issue = "116909")]
    pub const MAX_10_EXP: i32 = 4;

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
    #[allow(clippy::eq_op)]
    #[rustc_diagnostic_item = "f16_nan"]
    #[unstable(feature = "f16", issue = "116909")]
    pub const NAN: f16 = 0.0_f16 / 0.0_f16;

    /// 正无穷（Inf，∞）。
    #[unstable(feature = "f16", issue = "116909")]
    pub const INFINITY: f16 = 1.0_f16 / 0.0_f16;

    /// 负无穷（-Inf，−∞）。
    #[unstable(feature = "f16", issue = "116909")]
    pub const NEG_INFINITY: f16 = -1.0_f16 / 0.0_f16;

    /// 符号位
    pub(crate) const SIGN_MASK: u16 = 0x8000;

    /// 指数掩码
    pub(crate) const EXP_MASK: u16 = 0x7c00;

    /// 尾数掩码
    pub(crate) const MAN_MASK: u16 = 0x03ff;

    /// 最小可表示正值（最小次正规数）
    const TINY_BITS: u16 = 0x1;

    /// 最小可表示负值（最小负次正规数）
    const NEG_TINY_BITS: u16 = Self::TINY_BITS | Self::SIGN_MASK;

    /// 如果该值是 NaN，则返回 `true`。
    ///
    /// ```
    /// #![feature(f16)]
    /// # #[cfg(all(target_arch = "x86_64", target_os = "linux"))] {
    ///
    /// let nan = f16::NAN;
    /// let f = 7.0_f16;
    ///
    /// assert!(nan.is_nan());
    /// assert!(!f.is_nan());
    /// # }
    /// ```
    #[inline]
    #[must_use]
    #[unstable(feature = "f16", issue = "116909")]
    #[allow(clippy::eq_op)] // > if you intended to check if the operand is NaN, use `.is_nan()` instead :)
    pub const fn is_nan(self) -> bool {
        self != self
    }

    /// 如果该值是正无穷或负无穷，则返回 `true`；
    /// 否则返回 `false`。
    ///
    /// ```
    /// #![feature(f16)]
    /// # #[cfg(all(target_arch = "x86_64", target_os = "linux"))] {
    ///
    /// let f = 7.0f16;
    /// let inf = f16::INFINITY;
    /// let neg_inf = f16::NEG_INFINITY;
    /// let nan = f16::NAN;
    ///
    /// assert!(!f.is_infinite());
    /// assert!(!nan.is_infinite());
    ///
    /// assert!(inf.is_infinite());
    /// assert!(neg_inf.is_infinite());
    /// # }
    /// ```
    #[inline]
    #[must_use]
    #[unstable(feature = "f16", issue = "116909")]
    pub const fn is_infinite(self) -> bool {
        (self == f16::INFINITY) | (self == f16::NEG_INFINITY)
    }

    /// 如果该数既不是无穷也不是 NaN，则返回 `true`。
    ///
    /// ```
    /// #![feature(f16)]
    /// # #[cfg(all(target_arch = "x86_64", target_os = "linux"))] {
    ///
    /// let f = 7.0f16;
    /// let inf: f16 = f16::INFINITY;
    /// let neg_inf: f16 = f16::NEG_INFINITY;
    /// let nan: f16 = f16::NAN;
    ///
    /// assert!(f.is_finite());
    ///
    /// assert!(!nan.is_finite());
    /// assert!(!inf.is_finite());
    /// assert!(!neg_inf.is_finite());
    /// # }
    /// ```
    #[inline]
    #[must_use]
    #[unstable(feature = "f16", issue = "116909")]
    #[rustc_const_unstable(feature = "f16", issue = "116909")]
    pub const fn is_finite(self) -> bool {
        // 无需单独处理 NaN：如果 self 是 NaN，
        // 比较结果正好不是 true。
        self.abs() < Self::INFINITY
    }

    /// 如果该数是[次正规数]，则返回 `true`。
    ///
    /// ```
    /// #![feature(f16)]
    /// # #[cfg(all(target_arch = "x86_64", target_os = "linux"))] {
    ///
    /// let min = f16::MIN_POSITIVE; // 6.1035e-5
    /// let max = f16::MAX;
    /// let lower_than_min = 1.0e-7_f16;
    /// let zero = 0.0_f16;
    ///
    /// assert!(!min.is_subnormal());
    /// assert!(!max.is_subnormal());
    ///
    /// assert!(!zero.is_subnormal());
    /// assert!(!f16::NAN.is_subnormal());
    /// assert!(!f16::INFINITY.is_subnormal());
    /// // `0` 和 `min` 之间的值是次正规数（Subnormal）。
    /// assert!(lower_than_min.is_subnormal());
    /// # }
    /// ```
    /// [subnormal]: https://en.wikipedia.org/wiki/Denormal_number
    #[inline]
    #[must_use]
    #[unstable(feature = "f16", issue = "116909")]
    pub const fn is_subnormal(self) -> bool {
        matches!(self.classify(), FpCategory::Subnormal)
    }

    /// 如果该数不是零、无穷、[次正规数]或 NaN，则返回 `true`。
    ///
    /// ```
    /// #![feature(f16)]
    /// # #[cfg(all(target_arch = "x86_64", target_os = "linux"))] {
    ///
    /// let min = f16::MIN_POSITIVE; // 6.1035e-5
    /// let max = f16::MAX;
    /// let lower_than_min = 1.0e-7_f16;
    /// let zero = 0.0_f16;
    ///
    /// assert!(min.is_normal());
    /// assert!(max.is_normal());
    ///
    /// assert!(!zero.is_normal());
    /// assert!(!f16::NAN.is_normal());
    /// assert!(!f16::INFINITY.is_normal());
    /// // `0` 和 `min` 之间的值是次正规数（Subnormal）。
    /// assert!(!lower_than_min.is_normal());
    /// # }
    /// ```
    /// [subnormal]: https://en.wikipedia.org/wiki/Denormal_number
    #[inline]
    #[must_use]
    #[unstable(feature = "f16", issue = "116909")]
    pub const fn is_normal(self) -> bool {
        matches!(self.classify(), FpCategory::Normal)
    }

    /// 返回该数的浮点分类。如果只需要测试一个性质
    /// 通常使用对应的专用谓词会更快，
    /// 而不是先调用 `classify`。
    ///
    /// ```
    /// #![feature(f16)]
    /// # #[cfg(all(target_arch = "x86_64", target_os = "linux"))] {
    ///
    /// use std::num::FpCategory;
    ///
    /// let num = 12.4_f16;
    /// let inf = f16::INFINITY;
    ///
    /// assert_eq!(num.classify(), FpCategory::Normal);
    /// assert_eq!(inf.classify(), FpCategory::Infinite);
    /// # }
    /// ```
    #[inline]
    #[unstable(feature = "f16", issue = "116909")]
    pub const fn classify(self) -> FpCategory {
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
    /// #![feature(f16)]
    /// # // FIXME(f16_f128): s390x 上 LLVM 会崩溃，llvm/llvm-project#50374
    /// # #[cfg(all(target_arch = "x86_64", target_os = "linux"))] {
    ///
    /// let f = 7.0_f16;
    /// let g = -7.0_f16;
    ///
    /// assert!(f.is_sign_positive());
    /// assert!(!g.is_sign_positive());
    /// # }
    /// ```
    #[inline]
    #[must_use]
    #[unstable(feature = "f16", issue = "116909")]
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
    /// #![feature(f16)]
    /// # // FIXME(f16_f128): s390x 上 LLVM 会崩溃，llvm/llvm-project#50374
    /// # #[cfg(all(target_arch = "x86_64", target_os = "linux"))] {
    ///
    /// let f = 7.0_f16;
    /// let g = -7.0_f16;
    ///
    /// assert!(!f.is_sign_negative());
    /// assert!(g.is_sign_negative());
    /// # }
    /// ```
    #[inline]
    #[must_use]
    #[unstable(feature = "f16", issue = "116909")]
    pub const fn is_sign_negative(self) -> bool {
        // IEEE 754 规定：当且仅当 x 带负号时，isSignMinus(x) 为 true。isSignMinus
        // 同样适用于零和 NaN。
        // SAFETY: 这里只是通过位表示取得符号位，是安全的。
        (self.to_bits() & (1 << 15)) != 0
    }

    /// 返回严格大于 `self` 的最小可表示数。
    ///
    /// 设 `TINY` 为最小可表示正 `f16` 值，则：
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
    /// #![feature(f16)]
    /// # // FIXME(f16_f128): MSVC 上存在 ABI 问题
    /// # #[cfg(all(target_arch = "x86_64", target_os = "linux"))] {
    ///
    /// // f16::EPSILON 是 1.0 与下一个更大的数之间的差值。
    /// assert_eq!(1.0f16.next_up(), 1.0 + f16::EPSILON);
    /// // 但对大多数数字而言并非如此。
    /// assert!(0.1f16.next_up() < 0.1 + f16::EPSILON);
    /// assert_eq!(4356f16.next_up(), 4360.0);
    /// # }
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
    #[unstable(feature = "f16", issue = "116909")]
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
    /// 设 `TINY` 为最小可表示正 `f16` 值，则：
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
    /// #![feature(f16)]
    /// # // FIXME(f16_f128): MSVC 上存在 ABI 问题
    /// # #[cfg(all(target_arch = "x86_64", target_os = "linux"))] {
    ///
    /// let x = 1.0f16;
    /// // 将值限制到范围 [0, 1) 内。
    /// let clamped = x.clamp(0.0, 1.0f16.next_down());
    /// assert!(clamped < 1.0);
    /// assert_eq!(clamped.next_up(), 1.0);
    /// # }
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
    #[unstable(feature = "f16", issue = "116909")]
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
    /// #![feature(f16)]
    /// # // FIXME(f16_f128): 许多平台缺少 extendhfsf2、truncsfhf2、__gnu_h2f_ieee、__gnu_f2h_ieee
    /// # #[cfg(all(target_arch = "x86_64", target_os = "linux"))] {
    ///
    /// let x = 2.0_f16;
    /// let abs_difference = (x.recip() - (1.0 / x)).abs();
    ///
    /// assert!(abs_difference <= f16::EPSILON);
    /// # }
    /// ```
    #[inline]
    #[unstable(feature = "f16", issue = "116909")]
    #[must_use = "this returns the result of the operation, without modifying the original"]
    pub const fn recip(self) -> Self {
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
    /// #![feature(f16)]
    /// # // FIXME(f16_f128): 许多平台缺少 extendhfsf2、truncsfhf2、__gnu_h2f_ieee、__gnu_f2h_ieee
    /// # #[cfg(all(target_arch = "x86_64", target_os = "linux"))] {
    ///
    /// let angle = std::f16::consts::PI;
    ///
    /// let abs_difference = (angle.to_degrees() - 180.0).abs();
    /// assert!(abs_difference <= 0.5);
    /// # }
    /// ```
    #[inline]
    #[unstable(feature = "f16", issue = "116909")]
    #[must_use = "this returns the result of the operation, without modifying the original"]
    pub const fn to_degrees(self) -> Self {
        // 使用字面量以避免双重舍入；`consts::PI` 已经舍入，
        // 再进行除法会再次舍入。
        const PIS_IN_180: f16 = 57.2957795130823208767981548141051703_f16;
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
    /// #![feature(f16)]
    /// # // FIXME(f16_f128): 许多平台缺少 extendhfsf2、truncsfhf2、__gnu_h2f_ieee、__gnu_f2h_ieee
    /// # #[cfg(all(target_arch = "x86_64", target_os = "linux"))] {
    ///
    /// let angle = 180.0f16;
    ///
    /// let abs_difference = (angle.to_radians() - std::f16::consts::PI).abs();
    ///
    /// assert!(abs_difference <= 0.01);
    /// # }
    /// ```
    #[inline]
    #[unstable(feature = "f16", issue = "116909")]
    #[must_use = "this returns the result of the operation, without modifying the original"]
    pub const fn to_radians(self) -> f16 {
        // 使用字面量以避免双重舍入；`consts::PI` 已经舍入，
        // 再进行除法会再次舍入。
        const RADS_PER_DEG: f16 = 0.017453292519943295769236907684886_f16;
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
    /// #![feature(f16)]
    /// # #[cfg(target_arch = "aarch64")] { // FIXME(f16_F128): rust-lang/rust#123885
    ///
    /// let x = 1.0f16;
    /// let y = 2.0f16;
    ///
    /// assert_eq!(x.max(y), y);
    /// assert_eq!(x.max(f16::NAN), x);
    /// # }
    /// ```
    #[inline]
    #[unstable(feature = "f16", issue = "116909")]
    #[rustc_const_unstable(feature = "f16", issue = "116909")]
    #[must_use = "this returns the result of the comparison, without modifying either input"]
    pub const fn max(self, other: f16) -> f16 {
        intrinsics::maxnumf16(self, other)
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
    /// #![feature(f16)]
    /// # #[cfg(target_arch = "aarch64")] { // FIXME(f16_F128): rust-lang/rust#123885
    ///
    /// let x = 1.0f16;
    /// let y = 2.0f16;
    ///
    /// assert_eq!(x.min(y), x);
    /// assert_eq!(x.min(f16::NAN), x);
    /// # }
    /// ```
    #[inline]
    #[unstable(feature = "f16", issue = "116909")]
    #[rustc_const_unstable(feature = "f16", issue = "116909")]
    #[must_use = "this returns the result of the comparison, without modifying either input"]
    pub const fn min(self, other: f16) -> f16 {
        intrinsics::minnumf16(self, other)
    }

    /// 返回两个数中的最大值，并传播 NaN。
    ///
    /// 如果至少一个参数是 NaN，则返回 NaN，其位模式
    /// 按照通常的[算术运算规则](f32#nan-bit-patterns) 选择。此外，
    /// `-0.0` 被认为小于 `+0.0`，因此该函数对
    /// 非 NaN 输入完全确定。
    ///
    /// 这与 [`f16::max`] 不同：后者只有在*两个*参数都是 NaN 时才返回 NaN，
    /// 且不会可靠地为 `-0.0` 与 `+0.0` 排序。
    ///
    /// 这遵循 IEEE 754-2019 中 `maximum` 的语义。
    ///
    /// ```
    /// #![feature(f16)]
    /// #![feature(float_minimum_maximum)]
    /// # #[cfg(target_arch = "aarch64")] { // FIXME(f16_F128): rust-lang/rust#123885
    ///
    /// let x = 1.0f16;
    /// let y = 2.0f16;
    ///
    /// assert_eq!(x.maximum(y), y);
    /// assert!(x.maximum(f16::NAN).is_nan());
    /// # }
    /// ```
    #[inline]
    #[unstable(feature = "f16", issue = "116909")]
    // #[unstable(feature = "float_minimum_maximum", issue = "91079")]
    #[must_use = "this returns the result of the comparison, without modifying either input"]
    pub const fn maximum(self, other: f16) -> f16 {
        intrinsics::maximumf16(self, other)
    }

    /// 返回两个数中的最小值，并传播 NaN。
    ///
    /// 如果至少一个参数是 NaN，则返回 NaN，其位模式
    /// 按照通常的[算术运算规则](f32#nan-bit-patterns) 选择。此外，
    /// `-0.0` 被认为小于 `+0.0`，因此该函数对
    /// 非 NaN 输入完全确定。
    ///
    /// 这与 [`f16::min`] 不同：后者只有在*两个*参数都是 NaN 时才返回 NaN，
    /// 且不会可靠地为 `-0.0` 与 `+0.0` 排序。
    ///
    /// 这遵循 IEEE 754-2019 中 `minimum` 的语义。
    ///
    /// ```
    /// #![feature(f16)]
    /// #![feature(float_minimum_maximum)]
    /// # #[cfg(target_arch = "aarch64")] { // FIXME(f16_F128): rust-lang/rust#123885
    ///
    /// let x = 1.0f16;
    /// let y = 2.0f16;
    ///
    /// assert_eq!(x.minimum(y), x);
    /// assert!(x.minimum(f16::NAN).is_nan());
    /// # }
    /// ```
    #[inline]
    #[unstable(feature = "f16", issue = "116909")]
    // #[unstable(feature = "float_minimum_maximum", issue = "91079")]
    #[must_use = "this returns the result of the comparison, without modifying either input"]
    pub const fn minimum(self, other: f16) -> f16 {
        intrinsics::minimumf16(self, other)
    }

    /// 计算 `self` 与 `rhs` 之间的中点（平均值）。
    ///
    /// 如果*任一*参数是 NaN，或者参数组合为
    /// +inf 与 -inf，则返回 NaN。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(f16)]
    /// # #[cfg(target_arch = "aarch64")] { // FIXME(f16_F128): rust-lang/rust#123885
    ///
    /// assert_eq!(1f16.midpoint(4.0), 2.5);
    /// assert_eq!((-5.5f16).midpoint(8.0), 1.25);
    /// # }
    /// ```
    #[inline]
    #[doc(alias = "average")]
    #[unstable(feature = "f16", issue = "116909")]
    #[rustc_const_unstable(feature = "f16", issue = "116909")]
    pub const fn midpoint(self, other: f16) -> f16 {
        const HI: f16 = f16::MAX / 2.;

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

    /// 向零舍入，并转换为任意原始整数类型，
    /// 前提是该值有限且能放入目标类型。
    ///
    /// ```
    /// #![feature(f16)]
    /// # #[cfg(all(target_arch = "x86_64", target_os = "linux"))] {
    ///
    /// let value = 4.6_f16;
    /// let rounded = unsafe { value.to_int_unchecked::<u16>() };
    /// assert_eq!(rounded, 4);
    ///
    /// let value = -128.9_f16;
    /// let rounded = unsafe { value.to_int_unchecked::<i8>() };
    /// assert_eq!(rounded, i8::MIN);
    /// # }
    /// ```
    ///
    /// # 安全性(Safety）
    ///
    /// 该值必须满足：
    ///
    /// * 不是 `NaN`
    /// * 不是无穷
    /// * 截断小数部分之后，能用返回类型 `Int` 表示
    #[inline]
    #[unstable(feature = "f16", issue = "116909")]
    #[must_use = "this returns the result of the operation, without modifying the original"]
    pub unsafe fn to_int_unchecked<Int>(self) -> Int
    where
        Self: FloatToInt<Int>,
    {
        // SAFETY: 调用方必须满足
        // `FloatToInt::to_int_unchecked` 的安全契约。
        unsafe { FloatToInt::<Int>::to_int_unchecked(self) }
    }

    /// 原始 transmutation 到 `u16`。
    ///
    /// 目前在所有平台上，它都等同于 `transmute::<f16, u16>(self)`。
    ///
    /// 关于该操作可移植性的讨论见 [`from_bits`](#method.from_bits)
    /// （几乎没有问题）。
    ///
    /// 注意，该函数不同于 `as` 转换；`as` 试图
    /// 保留*数值*，而不是位级值。
    ///
    /// ```
    /// #![feature(f16)]
    /// # #[cfg(all(target_arch = "x86_64", target_os = "linux"))] {
    ///
    /// # // FIXME(f16_f128): 等 const 类型转换可用后再启用此项
    /// # // assert_ne!((1f16).to_bits(), 1f16 as u128); // to_bits() 不是类型转换！
    /// assert_eq!((12.5f16).to_bits(), 0x4a40);
    /// # }
    /// ```
    #[inline]
    #[unstable(feature = "f16", issue = "116909")]
    #[must_use = "this returns the result of the operation, without modifying the original"]
    #[allow(unnecessary_transmutes)]
    pub const fn to_bits(self) -> u16 {
        // SAFETY: `u16` 是普通旧数据类型，因此总能 transmute 到它。
        unsafe { mem::transmute(self) }
    }

    /// 从 `u16` 原始 transmutation。
    ///
    /// 目前在所有平台上，它都等同于 `transmute::<u16, f16>(v)`。
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
    /// ```
    /// #![feature(f16)]
    /// # #[cfg(all(target_arch = "x86_64", target_os = "linux"))] {
    ///
    /// let v = f16::from_bits(0x4a40);
    /// assert_eq!(v, 12.5);
    /// # }
    /// ```
    #[inline]
    #[must_use]
    #[unstable(feature = "f16", issue = "116909")]
    #[allow(unnecessary_transmutes)]
    pub const fn from_bits(v: u16) -> Self {
        // 事实证明 sNaN 相关的安全问题被高估了。
        // SAFETY: `u16` 是普通旧数据类型，因此总能从它 transmute。
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
    /// #![feature(f16)]
    /// # // FIXME(f16_f128): s390x 上 LLVM 会崩溃，llvm/llvm-project#50374
    /// # #[cfg(all(target_arch = "x86_64", target_os = "linux"))] {
    ///
    /// let bytes = 12.5f16.to_be_bytes();
    /// assert_eq!(bytes, [0x4a, 0x40]);
    /// # }
    /// ```
    #[inline]
    #[unstable(feature = "f16", issue = "116909")]
    #[must_use = "this returns the result of the operation, without modifying the original"]
    pub const fn to_be_bytes(self) -> [u8; 2] {
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
    /// #![feature(f16)]
    /// # // FIXME(f16_f128): s390x 上 LLVM 会崩溃，llvm/llvm-project#50374
    /// # #[cfg(all(target_arch = "x86_64", target_os = "linux"))] {
    ///
    /// let bytes = 12.5f16.to_le_bytes();
    /// assert_eq!(bytes, [0x40, 0x4a]);
    /// # }
    /// ```
    #[inline]
    #[unstable(feature = "f16", issue = "116909")]
    #[must_use = "this returns the result of the operation, without modifying the original"]
    pub const fn to_le_bytes(self) -> [u8; 2] {
        self.to_bits().to_le_bytes()
    }

    /// 返回该浮点数的内存表示，形式为字节数组，端序为
    /// 本机字节序。
    ///
    /// 由于使用目标平台的本机端序，可移植代码
    /// 应按需要改用 [`to_be_bytes`] 或 [`to_le_bytes`]。
    ///
    /// [`to_be_bytes`]: f16::to_be_bytes
    /// [`to_le_bytes`]: f16::to_le_bytes
    ///
    /// 关于该操作可移植性的讨论见 [`from_bits`](Self::from_bits)
    /// （几乎没有问题）。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(f16)]
    /// # // FIXME(f16_f128): s390x 上 LLVM 会崩溃，llvm/llvm-project#50374
    /// # #[cfg(all(target_arch = "x86_64", target_os = "linux"))] {
    ///
    /// let bytes = 12.5f16.to_ne_bytes();
    /// assert_eq!(
    ///     bytes,
    ///     if cfg!(target_endian = "big") {
    ///         [0x4a, 0x40]
    ///     } else {
    ///         [0x40, 0x4a]
    ///     }
    /// );
    /// # }
    /// ```
    #[inline]
    #[unstable(feature = "f16", issue = "116909")]
    #[must_use = "this returns the result of the operation, without modifying the original"]
    pub const fn to_ne_bytes(self) -> [u8; 2] {
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
    /// #![feature(f16)]
    /// # #[cfg(all(target_arch = "x86_64", target_os = "linux"))] {
    ///
    /// let value = f16::from_be_bytes([0x4a, 0x40]);
    /// assert_eq!(value, 12.5);
    /// # }
    /// ```
    #[inline]
    #[must_use]
    #[unstable(feature = "f16", issue = "116909")]
    pub const fn from_be_bytes(bytes: [u8; 2]) -> Self {
        Self::from_bits(u16::from_be_bytes(bytes))
    }

    /// 从小端字节序数组表示创建浮点值。
    ///
    /// 关于该操作可移植性的讨论见 [`from_bits`](Self::from_bits)
    /// （几乎没有问题）。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(f16)]
    /// # #[cfg(all(target_arch = "x86_64", target_os = "linux"))] {
    ///
    /// let value = f16::from_le_bytes([0x40, 0x4a]);
    /// assert_eq!(value, 12.5);
    /// # }
    /// ```
    #[inline]
    #[must_use]
    #[unstable(feature = "f16", issue = "116909")]
    pub const fn from_le_bytes(bytes: [u8; 2]) -> Self {
        Self::from_bits(u16::from_le_bytes(bytes))
    }

    /// 从本机端序字节数组表示创建浮点值。
    ///
    /// 由于使用目标平台的本机端序，可移植代码
    /// 通常应按需要改用 [`from_be_bytes`] 或 [`from_le_bytes`]，
    /// 以保持可移植性。
    ///
    /// [`from_be_bytes`]: f16::from_be_bytes
    /// [`from_le_bytes`]: f16::from_le_bytes
    ///
    /// 关于该操作可移植性的讨论见 [`from_bits`](Self::from_bits)
    /// （几乎没有问题）。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(f16)]
    /// # #[cfg(all(target_arch = "x86_64", target_os = "linux"))] {
    ///
    /// let value = f16::from_ne_bytes(if cfg!(target_endian = "big") {
    ///     [0x4a, 0x40]
    /// } else {
    ///     [0x40, 0x4a]
    /// });
    /// assert_eq!(value, 12.5);
    /// # }
    /// ```
    #[inline]
    #[must_use]
    #[unstable(feature = "f16", issue = "116909")]
    pub const fn from_ne_bytes(bytes: [u8; 2]) -> Self {
        Self::from_bits(u16::from_ne_bytes(bytes))
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
    /// `f16` 的 [`PartialOrd`] 和 [`PartialEq`] 实现一致。例如，
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
    /// #![feature(f16)]
    /// # // FIXME(f16_f128): 许多平台缺少 extendhfsf2、truncsfhf2、__gnu_h2f_ieee、__gnu_f2h_ieee
    /// # #[cfg(all(target_arch = "x86_64", target_os = "linux"))] {
    ///
    /// struct GoodBoy {
    ///     name: &'static str,
    ///     weight: f16,
    /// }
    ///
    /// let mut bois = vec![
    ///     GoodBoy { name: "Pucci", weight: 0.1 },
    ///     GoodBoy { name: "Woofer", weight: 99.0 },
    ///     GoodBoy { name: "Yapper", weight: 10.0 },
    ///     GoodBoy { name: "Chonk", weight: f16::INFINITY },
    ///     GoodBoy { name: "Abs. Unit", weight: f16::NAN },
    ///     GoodBoy { name: "Floaty", weight: -5.0 },
    /// ];
    ///
    /// bois.sort_by(|a, b| a.weight.total_cmp(&b.weight));
    ///
    /// // `f16::NAN` 可能为正也可能为负，这会影响排序顺序。
    /// if f16::NAN.is_sign_negative() {
    ///     bois.into_iter().map(|b| b.weight)
    ///         .zip([f16::NAN, -5.0, 0.1, 10.0, 99.0, f16::INFINITY].iter())
    ///         .for_each(|(a, b)| assert_eq!(a.to_bits(), b.to_bits()))
    /// } else {
    ///     bois.into_iter().map(|b| b.weight)
    ///         .zip([-5.0, 0.1, 10.0, 99.0, f16::INFINITY, f16::NAN].iter())
    ///         .for_each(|(a, b)| assert_eq!(a.to_bits(), b.to_bits()))
    /// }
    /// # }
    /// ```
    #[inline]
    #[must_use]
    #[unstable(feature = "f16", issue = "116909")]
    #[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
    pub const fn total_cmp(&self, other: &Self) -> crate::cmp::Ordering {
        let mut left = self.to_bits() as i16;
        let mut right = other.to_bits() as i16;

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
        left ^= (((left >> 15) as u16) >> 1) as i16;
        right ^= (((right >> 15) as u16) >> 1) as i16;

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
    /// #![feature(f16)]
    /// # #[cfg(all(target_arch = "x86_64", target_os = "linux"))] {
    ///
    /// assert!((-3.0f16).clamp(-2.0, 1.0) == -2.0);
    /// assert!((0.0f16).clamp(-2.0, 1.0) == 0.0);
    /// assert!((2.0f16).clamp(-2.0, 1.0) == 1.0);
    /// assert!((f16::NAN).clamp(-2.0, 1.0).is_nan());
    ///
    /// // 这些总是返回零，但其符号（被 `==` 忽略）是不确定的。
    /// assert!((0.0f16).clamp(-0.0, -0.0) == 0.0);
    /// assert!((1.0f16).clamp(-0.0, 0.0) == 0.0);
    /// // 这肯定是一个负零。
    /// assert!((-1.0f16).clamp(-0.0, 1.0).is_sign_negative());
    /// # }
    /// ```
    #[inline]
    #[unstable(feature = "f16", issue = "116909")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    pub const fn clamp(mut self, min: f16, max: f16) -> f16 {
        const_assert!(
            min <= max,
            "min > max, or either was NaN",
            "min > max, or either was NaN. min = {min:?}, max = {max:?}",
            min: f16,
            max: f16,
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
    /// #![feature(f16)]
    /// #![feature(clamp_magnitude)]
    /// # #[cfg(all(target_arch = "x86_64", target_os = "linux"))] {
    /// assert_eq!(5.0f16.clamp_magnitude(3.0), 3.0);
    /// assert_eq!((-5.0f16).clamp_magnitude(3.0), -3.0);
    /// assert_eq!(2.0f16.clamp_magnitude(3.0), 2.0);
    /// assert_eq!((-2.0f16).clamp_magnitude(3.0), -2.0);
    /// # }
    /// ```
    #[inline]
    #[unstable(feature = "clamp_magnitude", issue = "148519")]
    #[must_use = "this returns the clamped value and does not modify the original"]
    pub fn clamp_magnitude(self, limit: f16) -> f16 {
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
    /// #![feature(f16)]
    /// # #[cfg(all(target_arch = "x86_64", target_os = "linux"))] {
    ///
    /// let x = 3.5_f16;
    /// let y = -3.5_f16;
    ///
    /// assert_eq!(x.abs(), x);
    /// assert_eq!(y.abs(), -y);
    ///
    /// assert!(f16::NAN.abs().is_nan());
    /// # }
    /// ```
    #[inline]
    #[unstable(feature = "f16", issue = "116909")]
    #[rustc_const_unstable(feature = "f16", issue = "116909")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    pub const fn abs(self) -> Self {
        // FIXME(f16_f128): 可用时替换为 `intrinsics::fabsf16`
        Self::from_bits(self.to_bits() & !(1 << 15))
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
    /// #![feature(f16)]
    /// # #[cfg(all(target_arch = "x86_64", target_os = "linux"))] {
    ///
    /// let f = 3.5_f16;
    ///
    /// assert_eq!(f.signum(), 1.0);
    /// assert_eq!(f16::NEG_INFINITY.signum(), -1.0);
    ///
    /// assert!(f16::NAN.signum().is_nan());
    /// # }
    /// ```
    #[inline]
    #[unstable(feature = "f16", issue = "116909")]
    #[rustc_const_unstable(feature = "f16", issue = "116909")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    pub const fn signum(self) -> f16 {
        if self.is_nan() { Self::NAN } else { 1.0_f16.copysign(self) }
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
    /// #![feature(f16)]
    /// # #[cfg(all(target_arch = "x86_64", target_os = "linux"))] {
    ///
    /// let f = 3.5_f16;
    ///
    /// assert_eq!(f.copysign(0.42), 3.5_f16);
    /// assert_eq!(f.copysign(-0.42), -3.5_f16);
    /// assert_eq!((-f).copysign(0.42), 3.5_f16);
    /// assert_eq!((-f).copysign(-0.42), -3.5_f16);
    ///
    /// assert!(f16::NAN.copysign(1.0).is_nan());
    /// # }
    /// ```
    #[inline]
    #[unstable(feature = "f16", issue = "116909")]
    #[rustc_const_unstable(feature = "f16", issue = "116909")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    pub const fn copysign(self, sign: f16) -> f16 {
        intrinsics::copysignf16(self, sign)
    }

    /// 允许基于代数规则进行优化的浮点加法。
    ///
    /// 更多信息见[代数运算符](primitive@f32#algebraic-operators)。
    #[must_use = "method returns a new number and does not mutate the original value"]
    #[unstable(feature = "float_algebraic", issue = "136469")]
    #[rustc_const_unstable(feature = "float_algebraic", issue = "136469")]
    #[inline]
    pub const fn algebraic_add(self, rhs: f16) -> f16 {
        intrinsics::fadd_algebraic(self, rhs)
    }

    /// 允许基于代数规则进行优化的浮点减法。
    ///
    /// 更多信息见[代数运算符](primitive@f32#algebraic-operators)。
    #[must_use = "method returns a new number and does not mutate the original value"]
    #[unstable(feature = "float_algebraic", issue = "136469")]
    #[rustc_const_unstable(feature = "float_algebraic", issue = "136469")]
    #[inline]
    pub const fn algebraic_sub(self, rhs: f16) -> f16 {
        intrinsics::fsub_algebraic(self, rhs)
    }

    /// 允许基于代数规则进行优化的浮点乘法。
    ///
    /// 更多信息见[代数运算符](primitive@f32#algebraic-operators)。
    #[must_use = "method returns a new number and does not mutate the original value"]
    #[unstable(feature = "float_algebraic", issue = "136469")]
    #[rustc_const_unstable(feature = "float_algebraic", issue = "136469")]
    #[inline]
    pub const fn algebraic_mul(self, rhs: f16) -> f16 {
        intrinsics::fmul_algebraic(self, rhs)
    }

    /// 允许基于代数规则进行优化的浮点除法。
    ///
    /// 更多信息见[代数运算符](primitive@f32#algebraic-operators)。
    #[must_use = "method returns a new number and does not mutate the original value"]
    #[unstable(feature = "float_algebraic", issue = "136469")]
    #[rustc_const_unstable(feature = "float_algebraic", issue = "136469")]
    #[inline]
    pub const fn algebraic_div(self, rhs: f16) -> f16 {
        intrinsics::fdiv_algebraic(self, rhs)
    }

    /// 允许基于代数规则进行优化的浮点取余。
    ///
    /// 更多信息见[代数运算符](primitive@f32#algebraic-operators)。
    #[must_use = "method returns a new number and does not mutate the original value"]
    #[unstable(feature = "float_algebraic", issue = "136469")]
    #[rustc_const_unstable(feature = "float_algebraic", issue = "136469")]
    #[inline]
    pub const fn algebraic_rem(self, rhs: f16) -> f16 {
        intrinsics::frem_algebraic(self, rhs)
    }
}

// 本模块中的函数归入 `core_float_math`
// #[unstable(feature = "core_float_math", issue = "137578")]
#[cfg(not(test))]
#[doc(test(attr(feature(cfg_target_has_reliable_f16_f128), expect(internal_features))))]
impl f16 {
    /// 返回小于或等于 `self` 的最大整数。
    ///
    /// 该函数总是返回精确结果。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(f16)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f16_math)] {
    ///
    /// let f = 3.7_f16;
    /// let g = 3.0_f16;
    /// let h = -3.7_f16;
    ///
    /// assert_eq!(f.floor(), 3.0);
    /// assert_eq!(g.floor(), 3.0);
    /// assert_eq!(h.floor(), -4.0);
    /// # }
    /// ```
    #[inline]
    #[rustc_allow_incoherent_impl]
    #[unstable(feature = "f16", issue = "116909")]
    #[rustc_const_unstable(feature = "f16", issue = "116909")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    pub const fn floor(self) -> f16 {
        intrinsics::floorf16(self)
    }

    /// 返回大于或等于 `self` 的最小整数。
    ///
    /// 该函数总是返回精确结果。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(f16)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f16_math)] {
    ///
    /// let f = 3.01_f16;
    /// let g = 4.0_f16;
    ///
    /// assert_eq!(f.ceil(), 4.0);
    /// assert_eq!(g.ceil(), 4.0);
    /// # }
    /// ```
    #[inline]
    #[doc(alias = "ceiling")]
    #[rustc_allow_incoherent_impl]
    #[unstable(feature = "f16", issue = "116909")]
    #[rustc_const_unstable(feature = "f16", issue = "116909")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    pub const fn ceil(self) -> f16 {
        intrinsics::ceilf16(self)
    }

    /// 返回最接近 `self` 的整数。如果某个值正好位于两个
    /// 整数中间，则向远离 `0.0` 的方向舍入。
    ///
    /// 该函数总是返回精确结果。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(f16)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f16_math)] {
    ///
    /// let f = 3.3_f16;
    /// let g = -3.3_f16;
    /// let h = -3.7_f16;
    /// let i = 3.5_f16;
    /// let j = 4.5_f16;
    ///
    /// assert_eq!(f.round(), 3.0);
    /// assert_eq!(g.round(), -3.0);
    /// assert_eq!(h.round(), -4.0);
    /// assert_eq!(i.round(), 4.0);
    /// assert_eq!(j.round(), 5.0);
    /// # }
    /// ```
    #[inline]
    #[rustc_allow_incoherent_impl]
    #[unstable(feature = "f16", issue = "116909")]
    #[rustc_const_unstable(feature = "f16", issue = "116909")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    pub const fn round(self) -> f16 {
        intrinsics::roundf16(self)
    }

    /// 返回最接近该数的整数。对于恰好位于中间的情形，舍入到
    /// 最低有效数字为偶数的那个数。
    ///
    /// 该函数总是返回精确结果。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(f16)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f16_math)] {
    ///
    /// let f = 3.3_f16;
    /// let g = -3.3_f16;
    /// let h = 3.5_f16;
    /// let i = 4.5_f16;
    ///
    /// assert_eq!(f.round_ties_even(), 3.0);
    /// assert_eq!(g.round_ties_even(), -3.0);
    /// assert_eq!(h.round_ties_even(), 4.0);
    /// assert_eq!(i.round_ties_even(), 4.0);
    /// # }
    /// ```
    #[inline]
    #[rustc_allow_incoherent_impl]
    #[unstable(feature = "f16", issue = "116909")]
    #[rustc_const_unstable(feature = "f16", issue = "116909")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    pub const fn round_ties_even(self) -> f16 {
        intrinsics::round_ties_even_f16(self)
    }

    /// 返回 `self` 的整数部分。
    /// 这意味着非整数总是向零截断。
    ///
    /// 该函数总是返回精确结果。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(f16)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f16_math)] {
    ///
    /// let f = 3.7_f16;
    /// let g = 3.0_f16;
    /// let h = -3.7_f16;
    ///
    /// assert_eq!(f.trunc(), 3.0);
    /// assert_eq!(g.trunc(), 3.0);
    /// assert_eq!(h.trunc(), -3.0);
    /// # }
    /// ```
    #[inline]
    #[doc(alias = "truncate")]
    #[rustc_allow_incoherent_impl]
    #[unstable(feature = "f16", issue = "116909")]
    #[rustc_const_unstable(feature = "f16", issue = "116909")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    pub const fn trunc(self) -> f16 {
        intrinsics::truncf16(self)
    }

    /// 返回 `self` 的小数部分。
    ///
    /// 该函数总是返回精确结果。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(f16)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f16_math)] {
    ///
    /// let x = 3.6_f16;
    /// let y = -3.6_f16;
    /// let abs_difference_x = (x.fract() - 0.6).abs();
    /// let abs_difference_y = (y.fract() - (-0.6)).abs();
    ///
    /// assert!(abs_difference_x <= f16::EPSILON);
    /// assert!(abs_difference_y <= f16::EPSILON);
    /// # }
    /// ```
    #[inline]
    #[rustc_allow_incoherent_impl]
    #[unstable(feature = "f16", issue = "116909")]
    #[rustc_const_unstable(feature = "f16", issue = "116909")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    pub const fn fract(self) -> f16 {
        self - self.trunc()
    }

    /// 融合乘加。计算 `(self * a) + b`，整个操作只发生一次舍入
    /// 误差，因此通常比非融合乘加更精确。
    ///
    /// 如果目标架构拥有专用 `fma` CPU 指令，`mul_add`*可能*
    /// 比非融合乘加性能更好。不过，
    /// 这并不总是真的，而且高度依赖算法是否
    /// 针对特定目标硬件设计。
    ///
    /// # 精度
    ///
    /// 该操作的结果保证是
    /// 无限精度结果经过舍入后的值。IEEE 754 将它指定为
    /// `fusedMultiplyAdd`，并保证该语义不会改变。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(f16)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f16_math)] {
    ///
    /// let m = 10.0_f16;
    /// let x = 4.0_f16;
    /// let b = 60.0_f16;
    ///
    /// assert_eq!(m.mul_add(x, b), 100.0);
    /// assert_eq!(m * x + b, 100.0);
    ///
    /// let one_plus_eps = 1.0_f16 + f16::EPSILON;
    /// let one_minus_eps = 1.0_f16 - f16::EPSILON;
    /// let minus_one = -1.0_f16;
    ///
    /// // 精确结果 (1 + eps) * (1 - eps) = 1 - eps * eps。
    /// assert_eq!(one_plus_eps.mul_add(one_minus_eps, minus_one), -f16::EPSILON * f16::EPSILON);
    /// // 非融合乘加运算会产生不同的舍入结果。
    /// assert_eq!(one_plus_eps * one_minus_eps + minus_one, 0.0);
    /// # }
    /// ```
    #[inline]
    #[rustc_allow_incoherent_impl]
    #[unstable(feature = "f16", issue = "116909")]
    #[doc(alias = "fmaf16", alias = "fusedMultiplyAdd")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    pub const fn mul_add(self, a: f16, b: f16) -> f16 {
        intrinsics::fmaf16(self, a, b)
    }

    /// 计算欧几里得除法，这是与 `rem_euclid` 配套的方法。
    ///
    /// 它计算满足以下关系的整数 `n`：
    /// `self = n * rhs + self.rem_euclid(rhs)`。
    /// 换言之，结果是把 `self / rhs` 舍入到整数 `n`，
    /// 并满足 `self >= n * rhs`。
    ///
    /// # 精度
    ///
    /// 该操作的结果保证是
    /// 无限精度结果经过舍入后的值。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(f16)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f16_math)] {
    ///
    /// let a: f16 = 7.0;
    /// let b = 4.0;
    /// assert_eq!(a.div_euclid(b), 1.0); // 7.0 > 4.0 * 1.0
    /// assert_eq!((-a).div_euclid(b), -2.0); // -7.0 >= 4.0 * -2.0
    /// assert_eq!(a.div_euclid(-b), -1.0); // 7.0 >= -4.0 * -1.0
    /// assert_eq!((-a).div_euclid(-b), 2.0); // -7.0 >= -4.0 * 2.0
    /// # }
    /// ```
    #[inline]
    #[rustc_allow_incoherent_impl]
    #[unstable(feature = "f16", issue = "116909")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    pub fn div_euclid(self, rhs: f16) -> f16 {
        let q = (self / rhs).trunc();
        if self % rhs < 0.0 {
            return if rhs > 0.0 { q - 1.0 } else { q + 1.0 };
        }
        q
    }

    /// 计算 `self` 除以
    /// `rhs` 时的最小非负余数。
    ///
    /// 特别地，返回值 `r` 在大多数情况下满足 `0.0 <= r < rhs.abs()`。
    /// 不过，由于浮点舍入误差，在某些情况下它可能
    /// 得到 `r == rhs.abs()`，从而违反数学定义；这会发生在
    /// `self` 的幅度远小于 `rhs.abs()` 且 `self < 0.0` 时。
    /// 该结果不属于函数的数学陪域，但它是实数意义上
    /// 最接近的浮点数，因此近似满足
    /// 性质 `self == self.div_euclid(rhs) * rhs + self.rem_euclid(rhs)`
    /// 。
    ///
    /// # 精度
    ///
    /// 该操作的结果保证是
    /// 无限精度结果经过舍入后的值。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(f16)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f16_math)] {
    ///
    /// let a: f16 = 7.0;
    /// let b = 4.0;
    /// assert_eq!(a.rem_euclid(b), 3.0);
    /// assert_eq!((-a).rem_euclid(b), 1.0);
    /// assert_eq!(a.rem_euclid(-b), 3.0);
    /// assert_eq!((-a).rem_euclid(-b), 1.0);
    /// // 受舍入误差所限
    /// assert!((-f16::EPSILON).rem_euclid(3.0) != 0.0);
    /// # }
    /// ```
    #[inline]
    #[rustc_allow_incoherent_impl]
    #[doc(alias = "modulo", alias = "mod")]
    #[unstable(feature = "f16", issue = "116909")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    pub fn rem_euclid(self, rhs: f16) -> f16 {
        let r = self % rhs;
        if r < 0.0 { r + rhs.abs() } else { r }
    }

    /// 将一个数提升到整数幂。
    ///
    /// 使用该函数通常比使用 `powf` 更快。
    /// 它的舍入操作序列可能与 `powf` 不同，
    /// 因此结果不保证一致。
    ///
    /// 注意，该函数的特殊之处在于它可能对 NaN 输入返回非 NaN 结果。例如，
    /// 例如，`f16::powi(f16::NAN, 0)` 返回 `1.0`。不过，如果输入是*signaling*
    /// NaN，则结果会以非确定方式成为 NaN，或者成为
    /// 对应 quiet NaN 会产生的结果。
    ///
    /// # 未指定精度
    ///
    /// 该函数的精度是不确定的；这意味着它会随平台、
    /// Rust 版本变化，甚至同一次执行中不同调用之间也可能不同。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(f16)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f16_math)] {
    ///
    /// let x = 2.0_f16;
    /// let abs_difference = (x.powi(2) - (x * x)).abs();
    /// assert!(abs_difference <= f16::EPSILON);
    ///
    /// assert_eq!(f16::powi(f16::NAN, 0), 1.0);
    /// assert_eq!(f16::powi(0.0, 0), 1.0);
    /// # }
    /// ```
    #[inline]
    #[rustc_allow_incoherent_impl]
    #[unstable(feature = "f16", issue = "116909")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    pub fn powi(self, n: i32) -> f16 {
        intrinsics::powif16(self, n)
    }

    /// 返回一个数的平方根。
    ///
    /// 如果 `self` 是除 `-0.0` 以外的负数，则返回 NaN。
    ///
    /// # 精度
    ///
    /// 该操作的结果保证是
    /// 无限精度结果经过舍入后的值。IEEE 754 将它指定为 `squareRoot`
    /// 并保证该语义不会改变。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(f16)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f16_math)] {
    ///
    /// let positive = 4.0_f16;
    /// let negative = -4.0_f16;
    /// let negative_zero = -0.0_f16;
    ///
    /// assert_eq!(positive.sqrt(), 2.0);
    /// assert!(negative.sqrt().is_nan());
    /// assert!(negative_zero.sqrt() == negative_zero);
    /// # }
    /// ```
    #[inline]
    #[doc(alias = "squareRoot")]
    #[rustc_allow_incoherent_impl]
    #[unstable(feature = "f16", issue = "116909")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    pub fn sqrt(self) -> f16 {
        intrinsics::sqrtf16(self)
    }

    /// 返回一个数的立方根。
    ///
    /// # 未指定精度
    ///
    /// 该函数的精度是不确定的；这意味着它会随平台、
    /// Rust 版本变化，甚至同一次执行中不同调用之间也可能不同。
    ///
    /// 该函数当前在 Unix 和 Windows 上对应 libc 的 `cbrtf`，
    /// 但未来可能改变。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(f16)]
    /// # #[cfg(not(miri))]
    /// # #[cfg(target_has_reliable_f16_math)] {
    ///
    /// let x = 8.0f16;
    ///
    /// // x^(1/3) - 2 == 0
    /// let abs_difference = (x.cbrt() - 2.0).abs();
    ///
    /// assert!(abs_difference <= f16::EPSILON);
    /// # }
    /// ```
    #[inline]
    #[rustc_allow_incoherent_impl]
    #[unstable(feature = "f16", issue = "116909")]
    #[must_use = "method returns a new number and does not mutate the original value"]
    pub fn cbrt(self) -> f16 {
        libm::cbrtf(self as f32) as f16
    }
}
