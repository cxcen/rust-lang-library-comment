//! `char` 与整数、字符串之间的转换。
//!
//! 这些转换围绕 `char` 的核心不变量展开：它是 Unicode 标量值，
//! 数值必须小于等于 U+10FFFF，且不能位于 UTF-16 代理项范围 U+D800..=U+DFFF。

use crate::char::TryFromCharError;
use crate::error::Error;
use crate::fmt;
use crate::mem::transmute;
use crate::str::FromStr;
use crate::ub_checks::assert_unsafe_precondition;

/// 将 `u32` 转换为 `char`。见 [`char::from_u32`]。
#[must_use]
#[inline]
pub(super) const fn from_u32(i: u32) -> Option<char> {
    // FIXME(const-hack): 当 `Result::ok` 成为 const fn 后，改回在这里使用它。
    match char_try_from_u32(i) {
        Ok(c) => Some(c),
        Err(_) => None,
    }
}

/// 忽略有效性检查，将 `u32` 转换为 `char`。见 [`char::from_u32_unchecked`]。
///
/// 调用方必须保证 `i` 已经是 Unicode scalar value；否则 `transmute` 会制造无效 `char`，
/// 破坏编译器和标准库对 `char` 的有效性假设。
#[inline]
#[must_use]
#[allow(unnecessary_transmutes)]
#[track_caller]
pub(super) const unsafe fn from_u32_unchecked(i: u32) -> char {
    // SAFETY: 调用方必须保证 `i` 是合法 `char` 值，即不超过 U+10FFFF 且不在代理项范围内。
    unsafe {
        assert_unsafe_precondition!(
            check_language_ub,
            "invalid value for `char`",
            (i: u32 = i) => char_try_from_u32(i).is_ok()
        );
        transmute(i)
    }
}

#[stable(feature = "char_convert", since = "1.13.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl const From<char> for u32 {
    /// 将 [`char`] 转换为 [`u32`]。
    ///
    /// # 示例
    ///
    /// ```
    /// let c = 'c';
    /// let u = u32::from(c);
    ///
    /// assert!(4 == size_of_val(&u))
    /// ```
    #[inline]
    fn from(c: char) -> Self {
        c as u32
    }
}

#[stable(feature = "more_char_conversions", since = "1.51.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl const From<char> for u64 {
    /// 将 [`char`] 转换为 [`u64`]。
    ///
    /// # 示例
    ///
    /// ```
    /// let c = '👤';
    /// let u = u64::from(c);
    ///
    /// assert!(8 == size_of_val(&u))
    /// ```
    #[inline]
    fn from(c: char) -> Self {
        // `char` 先被转换为对应 code point 的数值，再零扩展到 64 位。
        // 见 [https://doc.rust-lang.org/reference/expressions/operator-expr.html#semantics]。
        c as u64
    }
}

#[stable(feature = "more_char_conversions", since = "1.51.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl const From<char> for u128 {
    /// 将 [`char`] 转换为 [`u128`]。
    ///
    /// # 示例
    ///
    /// ```
    /// let c = '⚙';
    /// let u = u128::from(c);
    ///
    /// assert!(16 == size_of_val(&u))
    /// ```
    #[inline]
    fn from(c: char) -> Self {
        // `char` 先被转换为对应 code point 的数值，再零扩展到 128 位。
        // 见 [https://doc.rust-lang.org/reference/expressions/operator-expr.html#semantics]。
        c as u128
    }
}

/// 将 code point 位于 U+0000 到 U+00FF（含）之间的 `char` 映射为
/// `0x00..=0xFF` 中同值的字节；如果 code point 大于 U+00FF，则转换失败。
///
/// 编码细节见 [`impl From<u8> for char`](char#impl-From<u8>-for-char)。
#[stable(feature = "u8_from_char", since = "1.59.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl const TryFrom<char> for u8 {
    type Error = TryFromCharError;

    /// 尝试将 [`char`] 转换为 [`u8`]。
    ///
    /// # 示例
    ///
    /// ```
    /// let a = 'ÿ'; // U+00FF
    /// let b = 'Ā'; // U+0100
    ///
    /// assert_eq!(u8::try_from(a), Ok(0xFF_u8));
    /// assert!(u8::try_from(b).is_err());
    /// ```
    #[inline]
    fn try_from(c: char) -> Result<u8, Self::Error> {
        // FIXME(const-hack): 这里应在 const 支持后改用 `map_err`。
        match u8::try_from(u32::from(c)) {
            Ok(b) => Ok(b),
            Err(_) => Err(TryFromCharError(())),
        }
    }
}

/// 将 code point 位于 U+0000 到 U+FFFF（含）之间的 `char` 映射为
/// `0x0000..=0xFFFF` 中同值的 `u16`；如果 code point 大于 U+FFFF，则转换失败。
///
/// 这对应 ISO/IEC 10646:2003 中规定的 UCS-2 编码。由于 `char` 不包含代理项，
/// 成功结果也不会是 UTF-16 代理项 code unit。
#[stable(feature = "u16_from_char", since = "1.74.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl const TryFrom<char> for u16 {
    type Error = TryFromCharError;

    /// 尝试将 [`char`] 转换为 [`u16`]。
    ///
    /// # 示例
    ///
    /// ```
    /// let trans_rights = '⚧'; // U+26A7
    /// let ninjas = '🥷'; // U+1F977
    ///
    /// assert_eq!(u16::try_from(trans_rights), Ok(0x26A7_u16));
    /// assert!(u16::try_from(ninjas).is_err());
    /// ```
    #[inline]
    fn try_from(c: char) -> Result<u16, Self::Error> {
        // FIXME(const-hack): 这里应在 const 支持后改用 `map_err`。
        match u16::try_from(u32::from(c)) {
            Ok(x) => Ok(x),
            Err(_) => Err(TryFromCharError(())),
        }
    }
}

/// 将 code point 位于 U+0000 到 U+10FFFF（含）之间的 `char` 映射为
/// `0x0000..=0x10FFFF` 中同值的 `usize`；如果目标平台的 `usize` 无法表示最终值，
/// 则转换失败。
///
/// 一般来说，该转换可以理解为取得字符对应的 UTF-32 code point，
/// 但结果仍受目标平台指针宽度可表示范围限制。
#[stable(feature = "usize_try_from_char", since = "1.94.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl const TryFrom<char> for usize {
    type Error = TryFromCharError;

    /// 尝试将 [`char`] 转换为 [`usize`]。
    ///
    /// # 示例
    ///
    /// ```
    /// let a = '\u{FFFF}'; // 总能成功。
    /// let b = '\u{10FFFF}'; // 取决于平台 `usize` 宽度。
    ///
    /// assert_eq!(usize::try_from(a), Ok(0xFFFF));
    ///
    /// if size_of::<usize>() >= size_of::<u32>() {
    ///     assert_eq!(usize::try_from(b), Ok(0x10FFFF));
    /// } else {
    ///     assert!(matches!(usize::try_from(b), Err(_)));
    /// }
    /// ```
    #[inline]
    fn try_from(c: char) -> Result<usize, Self::Error> {
        // FIXME(const-hack): 这里应在 const 支持后改用 `map_err`。
        match usize::try_from(u32::from(c)) {
            Ok(x) => Ok(x),
            Err(_) => Err(TryFromCharError(())),
        }
    }
}

/// 将 `0x00..=0xFF` 中的字节映射为 code point 位于 U+0000 到 U+00FF（含）
/// 且数值相同的 `char`。
///
/// Unicode 的设计使该转换等价于按 IANA 所称的 ISO-8859-1 字符编码解码字节。
/// 该编码与 ASCII 兼容。
///
/// 注意，这不同于 ISO/IEC 8859-1，也就是少一个连字符的 ISO 8859-1；
/// 后者留下了一些未分配给任何字符的 “blanks” 字节值。
/// IANA 的 ISO-8859-1 会把这些字节分配给 C0 和 C1 控制码。
///
/// 还要注意，这也不同于 Windows-1252（也称 code page 1252）。Windows-1252 是
/// ISO/IEC 8859-1 的超集，会把部分（不是全部）blanks 分配给标点和各种拉丁字符。
///
/// 更容易混淆的是，在 [Web 编码标准](https://encoding.spec.whatwg.org/) 中，
/// `ascii`、`iso-8859-1` 和 `windows-1252` 都是某个 Windows-1252 超集的别名；
/// 该超集还用对应的 C0 和 C1 控制码填充了剩余 blanks。
#[stable(feature = "char_convert", since = "1.13.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl const From<u8> for char {
    /// 将 [`u8`] 转换为 [`char`]。
    ///
    /// # 示例
    ///
    /// ```
    /// let u = 32 as u8;
    /// let c = char::from(u);
    ///
    /// assert!(4 == size_of_val(&c))
    /// ```
    #[inline]
    fn from(i: u8) -> Self {
        i as char
    }
}

/// 解析 `char` 时可能返回的错误。
///
/// 使用 [`char::from_str`] 方法时会创建该 `struct`。
#[stable(feature = "char_from_str", since = "1.20.0")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseCharError {
    kind: CharErrorKind,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
enum CharErrorKind {
    EmptyString,
    TooManyChars,
}

#[stable(feature = "char_from_str", since = "1.20.0")]
impl Error for ParseCharError {}

#[stable(feature = "char_from_str", since = "1.20.0")]
impl fmt::Display for ParseCharError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            CharErrorKind::EmptyString => "cannot parse char from empty string",
            CharErrorKind::TooManyChars => "too many characters in string",
        }
        .fmt(f)
    }
}

#[stable(feature = "char_from_str", since = "1.20.0")]
impl FromStr for char {
    type Err = ParseCharError;

    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut chars = s.chars();
        match (chars.next(), chars.next()) {
            (None, _) => Err(ParseCharError { kind: CharErrorKind::EmptyString }),
            (Some(c), None) => Ok(c),
            _ => Err(ParseCharError { kind: CharErrorKind::TooManyChars }),
        }
    }
}

#[inline]
#[allow(unnecessary_transmutes)]
const fn char_try_from_u32(i: u32) -> Result<char, CharTryFromError> {
    // 这是以下检查的优化版本：
    // (i > MAX as u32) || (i >= 0xD800 && i <= 0xDFFF),
    // 也可写成：
    // i >= 0x110000 || (i >= 0xD800 && i < 0xE000).
    //
    // 与 0xD800 做 XOR 会重排范围，使 0xD800..0xE000 映射到 0x0000..0x0800，
    // 同时保持 0xFFFF 之外的高位不变；特别是 >= 0x110000 的数仍留在该大范围内。
    //
    // 再减去 0x800 会让 0x0000..0x0800 发生环绕；于是一次与 0x110000 - 0x800
    // 的无符号比较，就能同时检测环绕后的代理项范围和原本大于 0x110000 的数。
    if (i ^ 0xD800).wrapping_sub(0x800) >= 0x110000 - 0x800 {
        Err(CharTryFromError(()))
    } else {
        // SAFETY: 已检查该数值是合法 Unicode scalar value，可作为 `char`。
        Ok(unsafe { transmute(i) })
    }
}

#[stable(feature = "try_from", since = "1.34.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl const TryFrom<u32> for char {
    type Error = CharTryFromError;

    #[inline]
    fn try_from(i: u32) -> Result<Self, Self::Error> {
        char_try_from_u32(i)
    }
}

/// 从 [`prim@u32`] 转换到 [`prim@char`] 失败时返回的错误类型。
///
/// 该 `struct` 由 [`char::try_from<u32>`](char#impl-TryFrom<u32>-for-char) 方法创建；
/// 更多说明见该方法文档。
#[stable(feature = "try_from", since = "1.34.0")]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct CharTryFromError(());

#[stable(feature = "try_from", since = "1.34.0")]
impl fmt::Display for CharTryFromError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        "converted integer out of range for `char`".fmt(f)
    }
}

/// 将给定基数中的数字转换为 `char`。见 [`char::from_digit`]。
#[inline]
#[must_use]
pub(super) const fn from_digit(num: u32, radix: u32) -> Option<char> {
    if radix > 36 {
        panic!("from_digit: radix is too high (maximum 36)");
    }
    if num < radix {
        let num = num as u8;
        if num < 10 { Some((b'0' + num) as char) } else { Some((b'a' + num - 10) as char) }
    } else {
        None
    }
}
