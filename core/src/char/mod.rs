//! `char` primitive type 的工具。
//!
//! *[另见 `char` primitive type](primitive@char)。*
//!
//! `char` 表示一个 Unicode 标量值（[Unicode scalar value]），而不是用户感知的“字符”。
//! Unicode 中“字符”本身不是严格技术概念；一个用户看到的字形可能由多个 code point 组成。
//! `char` 的取值范围是所有 Unicode code point 中排除 UTF-16 代理项 U+D800..=U+DFFF
//! 的部分，因此它不会包含代理对的任一半，也不能表示非 Unicode 标量值。
//!
//! [Unicode scalar value]: https://www.unicode.org/glossary/#unicode_scalar_value
//! [Unicode code point]: https://www.unicode.org/glossary/#code_point
//!
//! 本模块主要出于组织和技术原因存在；`char` 的主体文档直接位于
//! [`char` primitive type][char] 页面。
//!
//! 这里保存 `char` 上若干迭代器类型的实现，以及与 `char` 互相转换时需要的常量和函数。
//! 这些转换必须维护 `char` 的核心不变量：数值不超过 U+10FFFF，且不能落入代理项范围。

#![allow(non_snake_case)]
#![stable(feature = "rust1", since = "1.0.0")]

mod convert;
mod decode;
mod methods;

// 稳定的重新导出。
#[rustfmt::skip]
#[stable(feature = "try_from", since = "1.34.0")]
pub use self::convert::CharTryFromError;
#[stable(feature = "char_from_str", since = "1.20.0")]
pub use self::convert::ParseCharError;
#[stable(feature = "decode_utf16", since = "1.9.0")]
pub use self::decode::{DecodeUtf16, DecodeUtf16Error};

// 永久不稳定的重新导出。
#[rustfmt::skip]
#[unstable(feature = "char_internals", reason = "exposed only for libstd", issue = "none")]
pub use self::methods::encode_utf16_raw; // perma-unstable
#[unstable(feature = "char_internals", reason = "exposed only for libstd", issue = "none")]
pub use self::methods::{encode_utf8_raw, encode_utf8_raw_unchecked}; // perma-unstable

#[rustfmt::skip]
use crate::ascii;
pub(crate) use self::methods::EscapeDebugExtArgs;
use crate::error::Error;
use crate::escape::{AlwaysEscaped, EscapeIterInner, MaybeEscaped};
use crate::fmt::{self, Write};
use crate::iter::{FusedIterator, TrustedLen, TrustedRandomAccess, TrustedRandomAccessNoCoerce};
use crate::num::NonZero;

// 用于把 `char` 编码为 UTF-8 的范围界限和前缀标签。
const TAG_CONT: u8 = 0b1000_0000;
const TAG_TWO_B: u8 = 0b1100_0000;
const TAG_THREE_B: u8 = 0b1110_0000;
const TAG_FOUR_B: u8 = 0b1111_0000;
const MAX_ONE_B: u32 = 0x80;
const MAX_TWO_B: u32 = 0x800;
const MAX_THREE_B: u32 = 0x10000;

/*
    Lu  Uppercase_Letter        大写字母
    Ll  Lowercase_Letter        小写字母
    Lt  Titlecase_Letter        首部分为大写的双字母 titlecase 字符
    Lm  Modifier_Letter         修饰字母
    Lo  Other_Letter            其他字母，包括音节文字和表意文字
    Mn  Nonspacing_Mark         非间距组合标记（零前进宽度）
    Mc  Spacing_Mark            间距组合标记（正前进宽度）
    Me  Enclosing_Mark          包围式组合标记
    Nd  Decimal_Number          十进制数字
    Nl  Letter_Number           类字母数字字符
    No  Other_Number            其他类型的数字字符
    Pc  Connector_Punctuation   连接标点，例如连结线
    Pd  Dash_Punctuation        破折号或连字符类标点
    Ps  Open_Punctuation        成对标点中的开标点
    Pe  Close_Punctuation       成对标点中的闭标点
    Pi  Initial_Punctuation     起始引号
    Pf  Final_Punctuation       结束引号
    Po  Other_Punctuation       其他类型的标点
    Sm  Math_Symbol             主要用于数学的符号
    Sc  Currency_Symbol         货币符号
    Sk  Modifier_Symbol         非字母型修饰符号
    So  Other_Symbol            其他类型的符号
    Zs  Space_Separator         各种非零宽度的空格字符
    Zl  Line_Separator          仅 U+2028 LINE SEPARATOR
    Zp  Paragraph_Separator     仅 U+2029 PARAGRAPH SEPARATOR
    Cc  Control                 C0 或 C1 控制码
    Cf  Format                  格式控制字符
    Cs  Surrogate               代理项 code point；它不是 `char` 可表示的 Unicode 标量值
    Co  Private_Use             私用字符
    Cn  Unassigned              保留的未分配 code point 或 noncharacter
*/

/// `char` 可具有的最高有效 code point，`'\u{10FFFF}'`。请改用 [`char::MAX`]。
#[stable(feature = "rust1", since = "1.0.0")]
pub const MAX: char = char::MAX;

/// 将 `char` [编码](char::encode_utf8)为 UTF-8 时最多需要的字节数。
#[unstable(feature = "char_max_len", issue = "121714")]
pub const MAX_LEN_UTF8: usize = char::MAX_LEN_UTF8;

/// 将 `char` [编码](char::encode_utf16)为 UTF-16 时最多需要的 2 字节 code unit 数。
#[unstable(feature = "char_max_len", issue = "121714")]
pub const MAX_LEN_UTF16: usize = char::MAX_LEN_UTF16;

/// `U+FFFD REPLACEMENT CHARACTER`（�）在 Unicode 中用于表示解码错误。
/// 请改用 [`char::REPLACEMENT_CHARACTER`]。
#[stable(feature = "decode_utf16", since = "1.9.0")]
pub const REPLACEMENT_CHARACTER: char = char::REPLACEMENT_CHARACTER;

/// `char` 和 `str` 的 Unicode 相关方法所依据的
/// [Unicode](https://www.unicode.org/) 版本。请改用 [`char::UNICODE_VERSION`]。
#[stable(feature = "unicode_version", since = "1.45.0")]
pub const UNICODE_VERSION: (u8, u8, u8) = char::UNICODE_VERSION;

/// 为 `iter` 中的 UTF-16 code unit 创建解码迭代器，遇到未配对代理项时返回 `Err`。
/// 请改用 [`char::decode_utf16`]。
#[stable(feature = "decode_utf16", since = "1.9.0")]
#[inline]
pub fn decode_utf16<I: IntoIterator<Item = u16>>(iter: I) -> DecodeUtf16<I::IntoIter> {
    self::decode::decode_utf16(iter)
}

/// 将 `u32` 转换为 `char`。请改用 [`char::from_u32`]。
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_stable(feature = "const_char_convert", since = "1.67.0")]
#[must_use]
#[inline]
pub const fn from_u32(i: u32) -> Option<char> {
    self::convert::from_u32(i)
}

/// 忽略有效性检查，将 `u32` 转换为 `char`。请改用 [`char::from_u32_unchecked`]。
#[stable(feature = "char_from_unchecked", since = "1.5.0")]
#[rustc_const_stable(feature = "const_char_from_u32_unchecked", since = "1.81.0")]
#[must_use]
#[inline]
pub const unsafe fn from_u32_unchecked(i: u32) -> char {
    // SAFETY: 调用方必须维护 `char::from_u32_unchecked` 的契约：
    // `i` 必须是不超过 U+10FFFF 且不位于代理项范围内的 Unicode 标量值。
    unsafe { self::convert::from_u32_unchecked(i) }
}

/// 将给定基数中的数字转换为 `char`。请改用 [`char::from_digit`]。
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_stable(feature = "const_char_convert", since = "1.67.0")]
#[must_use]
#[inline]
pub const fn from_digit(num: u32, radix: u32) -> Option<char> {
    self::convert::from_digit(num, radix)
}

/// 返回一个迭代器，以 `char` 形式产生字符的十六进制 Unicode 转义序列。
///
/// 该 `struct` 由 [`char`] 上的 [`escape_unicode`] 方法创建；更多行为说明见该方法文档。
///
/// [`escape_unicode`]: char::escape_unicode
#[derive(Clone, Debug)]
#[stable(feature = "rust1", since = "1.0.0")]
pub struct EscapeUnicode(EscapeIterInner<10, AlwaysEscaped>);

impl EscapeUnicode {
    #[inline]
    const fn new(c: char) -> Self {
        Self(EscapeIterInner::unicode(c))
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl Iterator for EscapeUnicode {
    type Item = char;

    #[inline]
    fn next(&mut self) -> Option<char> {
        self.0.next().map(char::from)
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
    fn last(mut self) -> Option<char> {
        self.0.next_back().map(char::from)
    }

    #[inline]
    fn advance_by(&mut self, n: usize) -> Result<(), NonZero<usize>> {
        self.0.advance_by(n)
    }
}

#[stable(feature = "exact_size_escape", since = "1.11.0")]
impl ExactSizeIterator for EscapeUnicode {
    #[inline]
    fn len(&self) -> usize {
        self.0.len()
    }
}

#[stable(feature = "fused", since = "1.26.0")]
impl FusedIterator for EscapeUnicode {}

#[stable(feature = "char_struct_display", since = "1.16.0")]
impl fmt::Display for EscapeUnicode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

/// 产生 `char` 字面量转义序列的迭代器。
///
/// 该 `struct` 由 [`char`] 上的 [`escape_default`] 方法创建；更多行为说明见该方法文档。
///
/// [`escape_default`]: char::escape_default
#[derive(Clone, Debug)]
#[stable(feature = "rust1", since = "1.0.0")]
pub struct EscapeDefault(EscapeIterInner<10, AlwaysEscaped>);

impl EscapeDefault {
    #[inline]
    const fn printable(c: ascii::Char) -> Self {
        Self(EscapeIterInner::ascii(c.to_u8()))
    }

    #[inline]
    const fn backslash(c: ascii::Char) -> Self {
        Self(EscapeIterInner::backslash(c))
    }

    #[inline]
    const fn unicode(c: char) -> Self {
        Self(EscapeIterInner::unicode(c))
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl Iterator for EscapeDefault {
    type Item = char;

    #[inline]
    fn next(&mut self) -> Option<char> {
        self.0.next().map(char::from)
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
    fn last(mut self) -> Option<char> {
        self.0.next_back().map(char::from)
    }

    #[inline]
    fn advance_by(&mut self, n: usize) -> Result<(), NonZero<usize>> {
        self.0.advance_by(n)
    }
}

#[stable(feature = "exact_size_escape", since = "1.11.0")]
impl ExactSizeIterator for EscapeDefault {
    #[inline]
    fn len(&self) -> usize {
        self.0.len()
    }
}

#[stable(feature = "fused", since = "1.26.0")]
impl FusedIterator for EscapeDefault {}

#[stable(feature = "char_struct_display", since = "1.16.0")]
impl fmt::Display for EscapeDefault {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

/// 产生适用于调试输出的 `char` 字面量转义序列的迭代器。
///
/// 该 `struct` 由 [`char`] 上的 [`escape_debug`] 方法创建；更多行为说明见该方法文档。
///
/// [`escape_debug`]: char::escape_debug
#[stable(feature = "char_escape_debug", since = "1.20.0")]
#[derive(Clone, Debug)]
pub struct EscapeDebug(EscapeIterInner<10, MaybeEscaped>);

impl EscapeDebug {
    #[inline]
    const fn printable(chr: char) -> Self {
        Self(EscapeIterInner::printable(chr))
    }

    #[inline]
    const fn backslash(c: ascii::Char) -> Self {
        Self(EscapeIterInner::backslash(c))
    }

    #[inline]
    const fn unicode(c: char) -> Self {
        Self(EscapeIterInner::unicode(c))
    }
}

#[stable(feature = "char_escape_debug", since = "1.20.0")]
impl Iterator for EscapeDebug {
    type Item = char;

    #[inline]
    fn next(&mut self) -> Option<char> {
        self.0.next()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = self.len();
        (n, Some(n))
    }

    #[inline]
    fn count(self) -> usize {
        self.len()
    }
}

#[stable(feature = "char_escape_debug", since = "1.20.0")]
impl ExactSizeIterator for EscapeDebug {
    fn len(&self) -> usize {
        self.0.len()
    }
}

#[stable(feature = "fused", since = "1.26.0")]
impl FusedIterator for EscapeDebug {}

#[stable(feature = "char_escape_debug", since = "1.20.0")]
impl fmt::Display for EscapeDebug {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

macro_rules! casemappingiter_impls {
    ($(#[$attr:meta])* $ITER_NAME:ident) => {
        $(#[$attr])*
        #[stable(feature = "rust1", since = "1.0.0")]
        #[derive(Debug, Clone)]
        pub struct $ITER_NAME(CaseMappingIter);

        #[stable(feature = "rust1", since = "1.0.0")]
        impl Iterator for $ITER_NAME {
            type Item = char;
            fn next(&mut self) -> Option<char> {
                self.0.next()
            }

            fn size_hint(&self) -> (usize, Option<usize>) {
                self.0.size_hint()
            }

            fn fold<Acc, Fold>(self, init: Acc, fold: Fold) -> Acc
            where
                Fold: FnMut(Acc, Self::Item) -> Acc,
            {
                self.0.fold(init, fold)
            }

            fn count(self) -> usize {
                self.0.count()
            }

            fn last(self) -> Option<Self::Item> {
                self.0.last()
            }

            fn advance_by(&mut self, n: usize) -> Result<(), NonZero<usize>> {
                self.0.advance_by(n)
            }

            unsafe fn __iterator_get_unchecked(&mut self, idx: usize) -> Self::Item {
                // SAFETY: 该包装迭代器不改变索引语义，直接把 unchecked 访问的前置条件转交给调用方。
                unsafe { self.0.__iterator_get_unchecked(idx) }
            }
        }

        #[stable(feature = "case_mapping_double_ended", since = "1.59.0")]
        impl DoubleEndedIterator for $ITER_NAME {
            fn next_back(&mut self) -> Option<char> {
                self.0.next_back()
            }

            fn rfold<Acc, Fold>(self, init: Acc, rfold: Fold) -> Acc
            where
                Fold: FnMut(Acc, Self::Item) -> Acc,
            {
                self.0.rfold(init, rfold)
            }

            fn advance_back_by(&mut self, n: usize) -> Result<(), NonZero<usize>> {
                self.0.advance_back_by(n)
            }
        }

        #[stable(feature = "fused", since = "1.26.0")]
        impl FusedIterator for $ITER_NAME {}

        #[stable(feature = "exact_size_case_mapping_iter", since = "1.35.0")]
        impl ExactSizeIterator for $ITER_NAME {
            fn len(&self) -> usize {
                self.0.len()
            }

            fn is_empty(&self) -> bool {
                self.0.is_empty()
            }
        }

        // SAFETY: 长度上下界完全由内部 `array::IntoIter` 决定，包装层不额外丢弃或生成元素。
        #[unstable(feature = "trusted_len", issue = "37572")]
        unsafe impl TrustedLen for $ITER_NAME {}

        // SAFETY: 随机访问能力来自内部 `array::IntoIter`，包装层只转发访问。
        #[doc(hidden)]
        #[unstable(feature = "std_internals", issue = "none")]
        unsafe impl TrustedRandomAccessNoCoerce for $ITER_NAME {
            const MAY_HAVE_SIDE_EFFECT: bool = false;
        }

        // SAFETY: 该迭代器的 `Item` 固定为 `char`，没有可导致协变替换问题的子类型/父类型关系。
        #[doc(hidden)]
        #[unstable(feature = "std_internals", issue = "none")]
        unsafe impl TrustedRandomAccess for $ITER_NAME {}

        #[stable(feature = "char_struct_display", since = "1.16.0")]
        impl fmt::Display for $ITER_NAME {
            #[inline]
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Display::fmt(&self.0, f)
            }
        }
    }
}

casemappingiter_impls! {
    /// 返回产生 `char` 小写等价形式的迭代器。
    ///
    /// 该 `struct` 由 [`char`] 上的 [`to_lowercase`] 方法创建；更多行为说明见该方法文档。
    ///
    /// [`to_lowercase`]: char::to_lowercase
    ToLowercase
}

casemappingiter_impls! {
    /// 返回产生 `char` 大写等价形式的迭代器。
    ///
    /// 该 `struct` 由 [`char`] 上的 [`to_uppercase`] 方法创建；更多行为说明见该方法文档。
    ///
    /// [`to_uppercase`]: char::to_uppercase
    ToUppercase
}

#[derive(Debug, Clone)]
struct CaseMappingIter(core::array::IntoIter<char, 3>);

impl CaseMappingIter {
    #[inline]
    fn new(chars: [char; 3]) -> CaseMappingIter {
        let mut iter = chars.into_iter();
        if chars[2] == '\0' {
            iter.next_back();
            if chars[1] == '\0' {
                iter.next_back();

                // 有意不检查 `chars[0]`：`'\0'` 的小写映射仍是它自身，不能用它判断是否存在元素。
            }
        }
        CaseMappingIter(iter)
    }
}

impl Iterator for CaseMappingIter {
    type Item = char;

    fn next(&mut self) -> Option<char> {
        self.0.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }

    fn fold<Acc, Fold>(self, init: Acc, fold: Fold) -> Acc
    where
        Fold: FnMut(Acc, Self::Item) -> Acc,
    {
        self.0.fold(init, fold)
    }

    fn count(self) -> usize {
        self.0.count()
    }

    fn last(self) -> Option<Self::Item> {
        self.0.last()
    }

    fn advance_by(&mut self, n: usize) -> Result<(), NonZero<usize>> {
        self.0.advance_by(n)
    }

    unsafe fn __iterator_get_unchecked(&mut self, idx: usize) -> Self::Item {
        // SAFETY: 该包装迭代器不改变索引语义，unchecked 访问的前置条件仍由调用方承担。
        unsafe { self.0.__iterator_get_unchecked(idx) }
    }
}

impl DoubleEndedIterator for CaseMappingIter {
    fn next_back(&mut self) -> Option<char> {
        self.0.next_back()
    }

    fn rfold<Acc, Fold>(self, init: Acc, rfold: Fold) -> Acc
    where
        Fold: FnMut(Acc, Self::Item) -> Acc,
    {
        self.0.rfold(init, rfold)
    }

    fn advance_back_by(&mut self, n: usize) -> Result<(), NonZero<usize>> {
        self.0.advance_back_by(n)
    }
}

impl ExactSizeIterator for CaseMappingIter {
    fn len(&self) -> usize {
        self.0.len()
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl FusedIterator for CaseMappingIter {}

// SAFETY: 长度上下界完全由内部 `array::IntoIter` 决定。
unsafe impl TrustedLen for CaseMappingIter {}

// SAFETY: 随机访问能力来自内部 `array::IntoIter`，包装层只转发访问。
unsafe impl TrustedRandomAccessNoCoerce for CaseMappingIter {
    const MAY_HAVE_SIDE_EFFECT: bool = false;
}

// SAFETY: `CaseMappingIter` 的 `Item` 固定为 `char`，没有子类型/父类型替换问题。
unsafe impl TrustedRandomAccess for CaseMappingIter {}

impl fmt::Display for CaseMappingIter {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for c in self.0.clone() {
            f.write_char(c)?;
        }
        Ok(())
    }
}

/// 检查式 `char` 转换失败时返回的错误类型。
#[stable(feature = "u8_from_char", since = "1.59.0")]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct TryFromCharError(pub(crate) ());

#[stable(feature = "u8_from_char", since = "1.59.0")]
impl fmt::Display for TryFromCharError {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        "unicode code point out of range".fmt(fmt)
    }
}

#[stable(feature = "u8_from_char", since = "1.59.0")]
impl Error for TryFromCharError {}
