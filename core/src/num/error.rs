//! 整数转换与整数解析的错误类型。
//!
//! `core` 中的数值转换必须把两类失败区分清楚：一种是已有整数值转换到目标整数类型时
//! 超出范围，另一种是从字符串解析整数时输入本身不合法。前者由 `TryFromIntError`
//! 表示；后者由 `ParseIntError` 和可匹配的 `IntErrorKind` 描述，方便调用方区分空输入、
//! 非法数字、正/负溢出以及 `NonZero` 类型不允许零值等情况。

use crate::convert::Infallible;
use crate::error::Error;
use crate::fmt;

/// 检查型整数转换失败时返回的错误类型。
///
/// 该类型不暴露内部字段，因为失败原因只有一种：源整数不能无损表示为目标整数类型。
/// 例如把过大的 `u64` 转成 `u8`，或把负数转成无符号整数。具体错误文本保留为标准库
/// 既有的英文消息，以维持稳定的用户可见行为。
#[stable(feature = "try_from", since = "1.34.0")]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct TryFromIntError(pub(crate) ());

#[stable(feature = "try_from", since = "1.34.0")]
impl fmt::Display for TryFromIntError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        "out of range integral type conversion attempted".fmt(f)
    }
}

#[stable(feature = "try_from", since = "1.34.0")]
impl Error for TryFromIntError {}

#[stable(feature = "try_from", since = "1.34.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl const From<Infallible> for TryFromIntError {
    fn from(x: Infallible) -> TryFromIntError {
        match x {}
    }
}

#[unstable(feature = "never_type", issue = "35121")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl const From<!> for TryFromIntError {
    #[inline]
    fn from(never: !) -> TryFromIntError {
        // 使用 match 而不是强制类型转换，是为了在 `Infallible` 未来成为 `!` 的别名时，
        // 上面的 `From<Infallible> for TryFromIntError` 这类代码仍然按同一逻辑工作。
        match never {}
    }
}

/// 解析整数时可能返回的错误。
///
/// 原始整数类型上的 `from_str_radix()` 函数（例如 [`i8::from_str_radix`]）会返回该错误，
/// 它们的 [`FromStr`] 实现也使用同一错误类型。错误只描述解析阶段观察到的分类，不持有
/// 原始字符串，因此可以在 `core` 中无分配地传递。
///
/// [`FromStr`]: crate::str::FromStr
///
/// # 可能原因
///
/// `ParseIntError` 的常见原因包括空字符串、前后存在空白字符、基数下不允许的数字、
/// 符号位置错误、数值超出目标类型范围，以及解析到 `NonZero` 类型时实际值为零。
/// 例如从标准输入读取字符串时通常会带换行符，先调用 [`str::trim()`] 可以在解析前去掉
/// 这些空白。
///
/// # 示例
///
/// ```
/// if let Err(e) = i32::from_str_radix("a12", 10) {
///     println!("Failed conversion to i32: {e}");
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[stable(feature = "rust1", since = "1.0.0")]
pub struct ParseIntError {
    pub(super) kind: IntErrorKind,
}

/// 记录整数解析失败原因的枚举。
///
/// 该枚举是非穷尽的，标准库将来可以添加新的错误分类；匹配时应保留通配分支。现在的
/// 变体覆盖了解析器最重要的决策点：是否有输入、字符是否能作为当前基数的数字、结果
/// 是否超出目标类型，以及 `NonZero` 的非零不变量是否被输入破坏。
///
/// # 示例
///
/// ```
/// # fn main() {
/// if let Err(e) = i32::from_str_radix("a12", 10) {
///     println!("Failed conversion to i32: {:?}", e.kind());
/// }
/// # }
/// ```
#[stable(feature = "int_error_matching", since = "1.55.0")]
#[derive(Debug, Clone, PartialEq, Eq, Copy, Hash)]
#[non_exhaustive]
pub enum IntErrorKind {
    /// 被解析的值为空。
    ///
    /// 解析空字符串时会构造该变体。
    #[stable(feature = "int_error_matching", since = "1.55.0")]
    Empty,
    /// 输入中包含当前解析上下文下非法的数字。
    ///
    /// 例如字符串中包含非 ASCII 字符，或字符虽然是 ASCII 但不属于给定基数允许的数字。
    ///
    /// 当 `+` 或 `-` 单独出现、出现在数字中间，或以其他错误位置出现时，也会使用该变体。
    #[stable(feature = "int_error_matching", since = "1.55.0")]
    InvalidDigit,
    /// 正方向上的整数值过大，无法存入目标整数类型。
    #[stable(feature = "int_error_matching", since = "1.55.0")]
    PosOverflow,
    /// 负方向上的整数值过小，无法存入目标整数类型。
    #[stable(feature = "int_error_matching", since = "1.55.0")]
    NegOverflow,
    /// 解析结果为零。
    ///
    /// 当目标类型是 `NonZero` 系列而输入数值为零时会产生该变体；零会破坏 `NonZero`
    /// 的非零不变量，也会让 `Option<NonZero>` 等 niche 优化失去布局前提。
    #[stable(feature = "int_error_matching", since = "1.55.0")]
    Zero,
}

impl ParseIntError {
    /// 返回整数解析失败的详细分类。
    ///
    /// 调用方可以据此把用户输入错误、范围溢出和 `NonZero` 的零值违规分开处理。
    #[must_use]
    #[rustc_const_stable(feature = "const_int_from_str", since = "1.82.0")]
    #[stable(feature = "int_error_matching", since = "1.55.0")]
    pub const fn kind(&self) -> &IntErrorKind {
        &self.kind
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl fmt::Display for ParseIntError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            IntErrorKind::Empty => "cannot parse integer from empty string",
            IntErrorKind::InvalidDigit => "invalid digit found in string",
            IntErrorKind::PosOverflow => "number too large to fit in target type",
            IntErrorKind::NegOverflow => "number too small to fit in target type",
            IntErrorKind::Zero => "number would be zero for non-zero type",
        }
        .fmt(f)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl Error for ParseIntError {}
