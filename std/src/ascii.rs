//! 针对 ASCII 字符串与字符的操作。
//!
//! 本模块是对 `core::ascii` 的薄封装：`Char`、`EscapeDefault`、`escape_default`
//! 等核心定义都来自 `core`，这里仅做重导出；模块自身真正提供的只有已废弃的
//! [`AsciiExt`] 扩展 trait（保留它是为了向后兼容）。
//!
//! Rust 中绝大多数字符串操作面向 UTF-8 字符串。但有时只针对 ASCII 字符集来
//! 做某项操作会更合适。
//!
//! [`AsciiExt`] trait 提供的方法只作用于 ASCII 子集，对非 ASCII 字符保持原样。
//!
//! [`escape_default`] 函数返回一个迭代器，遍历给定字符转义后版本的各个字节。

#![stable(feature = "rust1", since = "1.0.0")]

#[unstable(feature = "ascii_char", issue = "110998")]
pub use core::ascii::Char;
#[stable(feature = "rust1", since = "1.0.0")]
pub use core::ascii::{EscapeDefault, escape_default};

/// 仅针对 ASCII 子集进行操作的扩展方法。
///
/// 请注意，对看似非 ASCII 的字符进行操作有时会得到意料之外的结果。看下面这个
/// 例子：
///
/// ```
/// use std::ascii::AsciiExt;
///
/// assert_eq!(AsciiExt::to_ascii_uppercase("café"), "CAFÉ");
/// assert_eq!(AsciiExt::to_ascii_uppercase("café"), "CAFé");
/// ```
///
/// 第一个例子中，这个小写字符串的内部表示是 `"cafe\u{301}"`（最后一个字符是一个
/// 锐音符 [combining character]，即组合字符）。与字符串中的其他字符不同，该组合
/// 字符不会被映射到大写形式，于是结果为 `"CAFE\u{301}"`。第二个例子中，小写
/// 字符串的内部表示是 `"caf\u{e9}"`（最后一个字符是一个表示带锐音符 'e' 的单个
/// Unicode 字符）。由于该字符落在 ASCII 范围之外，它不会被映射到大写形式，于是
/// 结果为 `"CAF\u{e9}"`。
///
/// [combining character]: https://en.wikipedia.org/wiki/Combining_character
#[stable(feature = "rust1", since = "1.0.0")]
#[deprecated(since = "1.26.0", note = "use inherent methods instead")]
pub trait AsciiExt {
    /// 用于存放复制出来的 ASCII 字符的容器类型。
    #[stable(feature = "rust1", since = "1.0.0")]
    type Owned;

    /// 检查该值是否落在 ASCII 范围内。
    ///
    /// # Note
    ///
    /// 本方法已废弃，请改用 `u8`、`char`、`[u8]` 和 `str` 上同名的固有方法。
    #[stable(feature = "rust1", since = "1.0.0")]
    fn is_ascii(&self) -> bool;

    /// 返回该值的 ASCII 大写等价形式的一份拷贝。
    ///
    /// ASCII 字母 'a' 到 'z' 会被映射为 'A' 到 'Z'，非 ASCII 字母保持不变。
    ///
    /// 若要就地（in-place）转为大写，请使用 [`make_ascii_uppercase`]。
    ///
    /// 若要在转换 ASCII 字符的同时也转换非 ASCII 字符，请使用
    /// [`str::to_uppercase`]。
    ///
    /// # Note
    ///
    /// 本方法已废弃，请改用 `u8`、`char`、`[u8]` 和 `str` 上同名的固有方法。
    ///
    /// [`make_ascii_uppercase`]: AsciiExt::make_ascii_uppercase
    #[stable(feature = "rust1", since = "1.0.0")]
    #[allow(deprecated)]
    fn to_ascii_uppercase(&self) -> Self::Owned;

    /// 返回该值的 ASCII 小写等价形式的一份拷贝。
    ///
    /// ASCII 字母 'A' 到 'Z' 会被映射为 'a' 到 'z'，非 ASCII 字母保持不变。
    ///
    /// 若要就地（in-place）转为小写，请使用 [`make_ascii_lowercase`]。
    ///
    /// 若要在转换 ASCII 字符的同时也转换非 ASCII 字符，请使用
    /// [`str::to_lowercase`]。
    ///
    /// # Note
    ///
    /// 本方法已废弃，请改用 `u8`、`char`、`[u8]` 和 `str` 上同名的固有方法。
    ///
    /// [`make_ascii_lowercase`]: AsciiExt::make_ascii_lowercase
    #[stable(feature = "rust1", since = "1.0.0")]
    #[allow(deprecated)]
    fn to_ascii_lowercase(&self) -> Self::Owned;

    /// 检查两个值在忽略 ASCII 大小写的意义下是否相等。
    ///
    /// 等价于 `to_ascii_lowercase(a) == to_ascii_lowercase(b)`，但无需为临时值
    /// 分配内存和拷贝。
    ///
    /// # Note
    ///
    /// 本方法已废弃，请改用 `u8`、`char`、`[u8]` 和 `str` 上同名的固有方法。
    #[stable(feature = "rust1", since = "1.0.0")]
    fn eq_ignore_ascii_case(&self, other: &Self) -> bool;

    /// 就地（in-place）把该类型的值转换为其 ASCII 大写等价形式。
    ///
    /// ASCII 字母 'a' 到 'z' 会被映射为 'A' 到 'Z'，非 ASCII 字母保持不变。
    ///
    /// 若要在不修改原值的前提下返回一个新的大写值，请使用
    /// [`to_ascii_uppercase`]。
    ///
    /// # Note
    ///
    /// 本方法已废弃，请改用 `u8`、`char`、`[u8]` 和 `str` 上同名的固有方法。
    ///
    /// [`to_ascii_uppercase`]: AsciiExt::to_ascii_uppercase
    #[stable(feature = "ascii", since = "1.9.0")]
    fn make_ascii_uppercase(&mut self);

    /// 就地（in-place）把该类型的值转换为其 ASCII 小写等价形式。
    ///
    /// ASCII 字母 'A' 到 'Z' 会被映射为 'a' 到 'z'，非 ASCII 字母保持不变。
    ///
    /// 若要在不修改原值的前提下返回一个新的小写值，请使用
    /// [`to_ascii_lowercase`]。
    ///
    /// # Note
    ///
    /// 本方法已废弃，请改用 `u8`、`char`、`[u8]` 和 `str` 上同名的固有方法。
    ///
    /// [`to_ascii_lowercase`]: AsciiExt::to_ascii_lowercase
    #[stable(feature = "ascii", since = "1.9.0")]
    fn make_ascii_lowercase(&mut self);
}

macro_rules! delegating_ascii_methods {
    () => {
        #[inline]
        fn is_ascii(&self) -> bool {
            self.is_ascii()
        }

        #[inline]
        fn to_ascii_uppercase(&self) -> Self::Owned {
            self.to_ascii_uppercase()
        }

        #[inline]
        fn to_ascii_lowercase(&self) -> Self::Owned {
            self.to_ascii_lowercase()
        }

        #[inline]
        fn eq_ignore_ascii_case(&self, o: &Self) -> bool {
            self.eq_ignore_ascii_case(o)
        }

        #[inline]
        fn make_ascii_uppercase(&mut self) {
            self.make_ascii_uppercase();
        }

        #[inline]
        fn make_ascii_lowercase(&mut self) {
            self.make_ascii_lowercase();
        }
    };
}

#[stable(feature = "rust1", since = "1.0.0")]
#[allow(deprecated)]
impl AsciiExt for u8 {
    type Owned = u8;

    delegating_ascii_methods!();
}

#[stable(feature = "rust1", since = "1.0.0")]
#[allow(deprecated)]
impl AsciiExt for char {
    type Owned = char;

    delegating_ascii_methods!();
}

#[stable(feature = "rust1", since = "1.0.0")]
#[allow(deprecated)]
impl AsciiExt for [u8] {
    type Owned = Vec<u8>;

    delegating_ascii_methods!();
}

#[stable(feature = "rust1", since = "1.0.0")]
#[allow(deprecated)]
impl AsciiExt for str {
    type Owned = String;

    delegating_ascii_methods!();
}
