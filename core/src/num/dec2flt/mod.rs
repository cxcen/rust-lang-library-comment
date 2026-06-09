//! 把十进制字符串转换为 IEEE 754 二进制浮点数。
//!
//! # 问题陈述
//!
//! 输入是类似 `12.34e56` 的十进制字符串。它由整数部分(`12`)、小数部分(`34`)和指数
//! 部分(`56`)组成；各部分在语法允许时可以缺省，并按对应的默认含义解释为 0 或 1。
//!
//! 目标是找出最接近该十进制精确值的 IEEE 754 浮点数。许多十进制值在二进制中没有有限
//! 表示，因此结果必须舍入到最后一位的 0.5 ULP 以内，也就是尽可能接近。若十进制值
//! 正好落在两个相邻浮点数中间，则采用 ties-to-even（也称 banker's rounding）规则。
//!
//! 这在实现复杂度和 CPU 成本上都很困难，因为解析器必须同时处理巨大指数、很多有效
//! 数字、NaN/Inf 特殊值以及舍入边界。
//!
//! # 实现思路
//!
//! 首先忽略符号。更准确地说，转换一开始就移除符号，最后再把符号应用回结果。IEEE
//! 754 浮点数关于零对称，取负只翻转符号位，因此这种做法对零、有限数、Inf 和 NaN
//! 等边界情况都成立。
//!
//! 随后通过调整指数“移除”小数点：概念上，`12.34e56` 会变成 `1234e54`，并表示为
//! 正整数 `f = 1234` 和整数指数 `e = 54`。解析阶段之后几乎所有代码都使用 `(f, e)`
//! 这种表示。
//!
//! 然后依次尝试一串越来越通用、也越来越昂贵的特例路径：先使用机器字大小整数和小型
//! 固定浮点数（`f32`/`f64`），再使用带 64 位 significand 的中间类型。扩展精度路径
//! 使用 Eisel-Lemire algorithm，它借助 128 位（有时 192 位）表示快速且准确地覆盖绝大
//! 多数输入。所有快速路径都无法给出正确舍入时，才退回大十进制表示：把数字移入合适
//! 范围，计算高有效位，并精确舍入到最近的可表示值。
//!
//! 另一个关键点是几乎所有函数都通过 `RawFloat` trait 按目标浮点类型参数化。不能先解析
//! 成 `f64` 再转换成 `f32`；那会引入二次舍入，而问题与使用二进制或 ties-to-even
//! 本身无关。
//!
//! 举例说，假设有 `d2` 和 `d4` 两种十进制类型，分别保留两位和四位十进制数字，输入为
//! `"0.01499"`，并采用 half-up rounding。直接舍入到两位会得到 `0.01`；但若先舍入到
//! 四位，会得到 `0.0150`，再舍入到两位就变成 `0.02`。同样原则适用于浮点解析：若要
//! 达到 0.5 ULP 精度，所有中间步骤都必须以目标类型所需的完整精度执行，并且只在最后
//! 综合所有被截断位**舍入一次**。
//!
//! 该模块及其子模块主要实现下列论文描述的算法：
//! "Number Parsing at a Gigabyte per Second"，在线地址：
//! <https://arxiv.org/abs/2101.11408>.
//!
//! # 其他约束
//!
//! 转换过程原则上**不应 panic**。代码中确实存在断言和显式 panic，但它们只作为内部一致性
//! 检查，正常输入不应触发；任何 panic 都应视为实现 bug。
//!
//! 单元测试覆盖面不足，只能发现少量可能错误。更完整的浮点解析测试位于
//! `src/tools/test-float-parse`，以 Rust 程序形式维护。
//!
//! 关于整数溢出：本文件很多地方会对十进制指数 `e` 做算术，主要是在移动小数点，例如
//! 移到第一个十进制数字之前或最后一个十进制数字之后。若不谨慎，这些计算可能溢出。
//! 因此解析子模块只会交出“足够小”的指数：这里的“足够”指 `指数 +/- 十进制数字个数`
//! 仍能放进 64 位整数。更大的指数仍可被接受，但不会继续参与这类算术，而是立即归约为
//! 正/负的零或 Inf。
//!
//! # 记号
//!
//! 本模块使用与 Lemire 论文相同的记号：
//!
//! - `m`：二进制 mantissa，始终非负
//! - `p`：二进制 exponent，有符号整数
//! - `w`：十进制 significand，始终非负
//! - `q`：十进制 exponent，有符号整数
//!
//! 于是二进制浮点数表示为 `m * 2^p`，对应的十进制精确值表示为 `w * 10^q`。

#![doc(hidden)]
#![unstable(
    feature = "dec2flt",
    reason = "internal routines only exposed for testing",
    issue = "none"
)]

use self::common::BiasedFp;
use self::float::RawFloat;
use self::lemire::compute_float;
use self::parse::{parse_inf_nan, parse_number};
use self::slow::parse_long_mantissa;
use crate::error::Error;
use crate::fmt;
use crate::str::FromStr;

mod common;
pub mod decimal;
pub mod decimal_seq;
mod fpu;
mod slow;
mod table;
// `float` 会被 flt2dec 使用；这些子模块也都会被单元测试直接覆盖。
pub mod float;
pub mod lemire;
pub mod parse;

macro_rules! from_str_float_impl {
    ($t:ty) => {
        #[stable(feature = "rust1", since = "1.0.0")]
        impl FromStr for $t {
            type Err = ParseFloatError;

            /// 把十进制字符串转换为浮点数。
            ///
            /// 支持可选的十进制指数部分。
            ///
            /// 该函数接受如下字符串：
            ///
            /// * '3.14'
            /// * '-3.14'
            /// * '2.5E10', or equivalently, '2.5e10'
            /// * '2.5E-10'
            /// * '5.'
            /// * '.5', or, equivalently, '0.5'
            /// * '7'
            /// * '007'
            /// * 'inf', '-inf', '+infinity', 'NaN'
            ///
            /// 字母不区分大小写。
            ///
            /// 前导或尾随空白字符会导致错误；调用方需要自行决定是否先 `trim`。
            ///
            /// # Grammar
            ///
            /// 字符串转成小写后，只要符合下面的 [EBNF] 文法，就会返回 [`Ok`]：
            ///
            /// ```txt
            /// Float  ::= Sign? ( 'inf' | 'infinity' | 'nan' | Number )
            /// Number ::= ( Digit+ |
            ///              Digit+ '.' Digit* |
            ///              Digit* '.' Digit+ ) Exp?
            /// Exp    ::= 'e' Sign? Digit+
            /// Sign   ::= [+-]
            /// Digit  ::= [0-9]
            /// ```
            ///
            /// [EBNF]: https://www.w3.org/TR/REC-xml/#sec-notation
            ///
            /// # 参数
            ///
            /// * src - 待解析字符串
            ///
            /// # 返回值
            ///
            /// 如果字符串不是有效浮点数字面量，返回 `Err(ParseFloatError)`。否则返回
            /// `Ok(n)`，其中 `n` 是离 `src` 表示的精确数值最近的可表示浮点数，舍入规则
            /// 与原始浮点运算结果一致。
            // 这里添加 `#[inline(never)]`，因为函数体最终会被 `#[inline(always)]` 的
            // `dec2flt` 填充。`dec2flt` 是泛型函数；若在这里使用普通 inline，且 `dec2flt`
            // 本身不限制内联，就会重复生成大量 `dec2flt` 实例，尽管理论上最多只会有两个
            // 目标类型实例。`#[inline(never)]` 可以避免这种代码体积膨胀。
            #[inline(never)]
            fn from_str(src: &str) -> Result<Self, ParseFloatError> {
                dec2flt(src)
            }
        }
    };
}

#[cfg(target_has_reliable_f16)]
from_str_float_impl!(f16);
from_str_float_impl!(f32);
from_str_float_impl!(f64);

// FIXME(f16_f128): 当后端和目标平台不能可靠支持 f16 时使用 fallback，以避免 ICE。

#[cfg(not(target_has_reliable_f16))]
impl FromStr for f16 {
    type Err = ParseFloatError;

    #[inline]
    fn from_str(_src: &str) -> Result<Self, ParseFloatError> {
        unimplemented!("requires target_has_reliable_f16")
    }
}

/// 解析浮点数时可能返回的错误。
///
/// 该错误是 [`f32`]、[`f64`] 以及受支持浮点类型的 [`FromStr`] 实现所使用的错误类型。
/// 它只区分空输入和无效字面量；下溢、上溢以及精度损失不是解析错误，而会按 IEEE 754
/// 规则产生零、Inf 或最近可表示值。
///
/// # 示例
///
/// ```
/// use std::str::FromStr;
///
/// if let Err(e) = f64::from_str("a.12") {
///     println!("Failed conversion to f64: {e}");
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[stable(feature = "rust1", since = "1.0.0")]
pub struct ParseFloatError {
    kind: FloatErrorKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum FloatErrorKind {
    Empty,
    Invalid,
}

#[stable(feature = "rust1", since = "1.0.0")]
impl Error for ParseFloatError {}

#[stable(feature = "rust1", since = "1.0.0")]
impl fmt::Display for ParseFloatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            FloatErrorKind::Empty => "cannot parse float from empty string",
            FloatErrorKind::Invalid => "invalid float literal",
        }
        .fmt(f)
    }
}

#[inline]
pub(super) fn pfe_empty() -> ParseFloatError {
    ParseFloatError { kind: FloatErrorKind::Empty }
}

// 供单元测试使用，保持公开。这样比公开 `FloatErrorKind` 和 `ParseFloatError::kind`
// 更能保持错误类型的稳定封装边界。
#[inline]
pub fn pfe_invalid() -> ParseFloatError {
    ParseFloatError { kind: FloatErrorKind::Invalid }
}

/// 把 `BiasedFp` 转换为对应的机器浮点类型。
///
/// 调用方已经完成舍入并给出了带偏置指数；这里仅把 mantissa 和 exponent 字段重新组装成
/// IEEE 754 位模式。
fn biased_fp_to_float<F: RawFloat>(x: BiasedFp) -> F {
    let mut word = x.m;
    word |= (x.p_biased as u64) << F::SIG_BITS;
    F::from_u64_bits(word)
}

/// 把十进制字符串转换为浮点数。
#[inline(always)] // 会内联进上面带 `#[inline(never)]` 的函数体。
pub fn dec2flt<F: RawFloat>(s: &str) -> Result<F, ParseFloatError> {
    let mut s = s.as_bytes();
    let Some(&c) = s.first() else { return Err(pfe_empty()) };
    let negative = c == b'-';
    if c == b'-' || c == b'+' {
        s = &s[1..];
    }
    if s.is_empty() {
        return Err(pfe_invalid());
    }

    let mut num = match parse_number(s) {
        Some(r) => r,
        None if let Some(value) = parse_inf_nan(s, negative) => return Ok(value),
        None => return Err(pfe_invalid()),
    };
    num.negative = negative;
    if !cfg!(feature = "optimize_for_size") {
        if let Some(value) = num.try_fast_path::<F>() {
            return Ok(value);
        }
    }

    // 如果有效数字曾被截断，只有当 `mantissa + 1` 会产生不同结果时才可能存在舍入误差。
    // 同时，如果 Eisel-Lemire algorithm 第一次无法正确舍入，也避免重复尝试同一路径。
    let mut fp = compute_float::<F>(num.exponent, num.mantissa);
    if num.many_digits
        && fp.p_biased >= 0
        && fp != compute_float::<F>(num.exponent, num.mantissa + 1)
    {
        fp.p_biased = -1;
    }
    // Eisel-Lemire algorithm 无法正确舍入该浮点值；退回更慢但保证正确的算法。
    if fp.p_biased < 0 {
        fp = parse_long_mantissa::<F>(s);
    }

    let mut float = biased_fp_to_float::<F>(fp);
    if num.negative {
        float = -float;
    }
    Ok(float)
}
