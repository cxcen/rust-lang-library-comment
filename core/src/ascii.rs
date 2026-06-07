//! ASCII 字符串与字符相关操作。
//!
//! Rust 中大多数字符串操作都作用于 UTF-8 字符串。但某些协议、文件格式或转义规则只关心
//! ASCII 字符集,此时把问题限制在 7-bit ASCII 上反而更明确、更高效。
//!
//! [`escape_default`] 函数会为给定字节的转义表示提供一个按字节产出的迭代器。

#![stable(feature = "core_ascii", since = "1.26.0")]

use crate::escape::{AlwaysEscaped, EscapeIterInner};
use crate::fmt;
use crate::iter::FusedIterator;
use crate::num::NonZero;

mod ascii_char;
#[doc(alias("AsciiChar"))]
#[unstable(feature = "ascii_char", issue = "110998")]
pub use ascii_char::AsciiChar as Char;

/// 遍历某个字节的转义表示。
///
/// 本 `struct` 由 [`escape_default`] 创建;具体转义规则见该函数文档。
#[must_use = "iterators are lazy and do nothing unless consumed"]
#[stable(feature = "rust1", since = "1.0.0")]
#[derive(Clone)]
pub struct EscapeDefault(EscapeIterInner<4, AlwaysEscaped>);

/// 返回一个迭代器,逐字节产出 `u8` 的转义表示。
///
/// 默认规则偏向生成能在多种语言中作为字面量出现的文本,包括 C++11 以及类似的 C 系语言。
/// 精确规则如下:
///
/// * Tab 转义为 `\t`。
/// * Carriage return 转义为 `\r`。
/// * Line feed 转义为 `\n`。
/// * Single quote 转义为 `\'`。
/// * Double quote 转义为 `\"`。
/// * Backslash 转义为 `\\`。
/// * 位于“可打印 ASCII”范围 `0x20` .. `0x7e`(含端点)内的字符不转义。
/// * 其他字符使用 `\xNN` 形式的十六进制转义。
/// * 本函数从不生成 Unicode 转义。
///
/// # 示例
///
/// ```
/// use std::ascii;
///
/// let escaped = ascii::escape_default(b'0').next().unwrap();
/// assert_eq!(b'0', escaped);
///
/// let mut escaped = ascii::escape_default(b'\t');
///
/// assert_eq!(b'\\', escaped.next().unwrap());
/// assert_eq!(b't', escaped.next().unwrap());
///
/// let mut escaped = ascii::escape_default(b'\r');
///
/// assert_eq!(b'\\', escaped.next().unwrap());
/// assert_eq!(b'r', escaped.next().unwrap());
///
/// let mut escaped = ascii::escape_default(b'\n');
///
/// assert_eq!(b'\\', escaped.next().unwrap());
/// assert_eq!(b'n', escaped.next().unwrap());
///
/// let mut escaped = ascii::escape_default(b'\'');
///
/// assert_eq!(b'\\', escaped.next().unwrap());
/// assert_eq!(b'\'', escaped.next().unwrap());
///
/// let mut escaped = ascii::escape_default(b'"');
///
/// assert_eq!(b'\\', escaped.next().unwrap());
/// assert_eq!(b'"', escaped.next().unwrap());
///
/// let mut escaped = ascii::escape_default(b'\\');
///
/// assert_eq!(b'\\', escaped.next().unwrap());
/// assert_eq!(b'\\', escaped.next().unwrap());
///
/// let mut escaped = ascii::escape_default(b'\x9d');
///
/// assert_eq!(b'\\', escaped.next().unwrap());
/// assert_eq!(b'x', escaped.next().unwrap());
/// assert_eq!(b'9', escaped.next().unwrap());
/// assert_eq!(b'd', escaped.next().unwrap());
/// ```
#[stable(feature = "rust1", since = "1.0.0")]
pub fn escape_default(c: u8) -> EscapeDefault {
    EscapeDefault::new(c)
}

impl EscapeDefault {
    #[inline]
    pub(crate) const fn new(c: u8) -> Self {
        Self(EscapeIterInner::ascii(c))
    }

    #[inline]
    pub(crate) fn empty() -> Self {
        Self(EscapeIterInner::empty())
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl Iterator for EscapeDefault {
    type Item = u8;

    #[inline]
    fn next(&mut self) -> Option<u8> {
        self.0.next()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = self.0.len();
        (n, Some(n))
    }

    #[inline]
    fn count(self) -> usize {
        self.0.len()
    }

    #[inline]
    fn last(mut self) -> Option<u8> {
        self.0.next_back()
    }

    #[inline]
    fn advance_by(&mut self, n: usize) -> Result<(), NonZero<usize>> {
        self.0.advance_by(n)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl DoubleEndedIterator for EscapeDefault {
    #[inline]
    fn next_back(&mut self) -> Option<u8> {
        self.0.next_back()
    }

    #[inline]
    fn advance_back_by(&mut self, n: usize) -> Result<(), NonZero<usize>> {
        self.0.advance_back_by(n)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl ExactSizeIterator for EscapeDefault {
    #[inline]
    fn len(&self) -> usize {
        self.0.len()
    }
}

#[stable(feature = "fused", since = "1.26.0")]
impl FusedIterator for EscapeDefault {}

#[stable(feature = "ascii_escape_display", since = "1.39.0")]
impl fmt::Display for EscapeDefault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

#[stable(feature = "std_debug", since = "1.16.0")]
impl fmt::Debug for EscapeDefault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EscapeDefault").finish_non_exhaustive()
    }
}
