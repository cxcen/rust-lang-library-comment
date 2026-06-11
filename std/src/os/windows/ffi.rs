//! Windows 平台对 [`std::ffi`] 模块中各原语的特定扩展。
//!
//! # 概述
//!
//! 出于历史原因，Windows API 对字符串使用一种可能不规范（ill-formed）的 UTF-16
//! 编码形式。具体来说，Windows 字符串中的 16 位码元（code unit）可能包含
//! [未成对、孤立出现的代理（surrogate）码点][ill-formed-utf-16]。Unicode 标准要求
//! 代理码点（即 U+D800 到 U+DFFF 范围内的码点）必须始终 *成对* 出现，因为在 UTF-16
//! 编码中，要用一个 *代理码元对* 来编码单个字符。为了与那些不强制这种配对的代码兼容，
//! Windows 自身也不强制要求它们成对。
//!
//! 虽然把这样的字符串无损地转换成合法的 UTF-16 字符串（甚至 UTF-8）并非总能做到，
//! 但人们往往希望能够把这样的字符串无损地往返（round-trip）传递给 Windows API 再取回。
//! 例如，某些 Rust 代码可能只是在“桥接”若干 Windows API，仅仅在这些 API 之间传递
//! `WCHAR` 字符串，而从不真正深入查看这些字符串的内容。
//!
//! 如果 Rust 代码 *确实* 需要查看这些字符串的内容，它可以把它们转换成合法的 UTF-8，
//! 转换时可能有损：用 [`U+FFFD REPLACEMENT CHARACTER`（替换字符）][U+FFFD] 来替换非法
//! 序列，这正是其他处理字符串编码的 Rust API 中惯常的做法。
//!
//! # `OsStringExt` 与 `OsStrExt`
//!
//! [`OsString`] 是 Rust 对“以操作系统首选表示形式持有的拥有式字符串”的封装。在 Windows
//! 上，这个 struct 被补充实现了 [`OsStringExt`] trait，其中提供了一个
//! [`OsStringExt::from_wide`] 方法。它让你能够从一个 `&[u16]` 切片创建出 [`OsString`]；
//! 这个切片大概率就是你从某个 `WCHAR` 形式的 Windows API 那里得到的。
//!
//! 类似地，[`OsStr`] 是 Rust 对“以操作系统首选表示形式持有的借用式字符串”的封装。在
//! Windows 上，[`OsStrExt`] trait 提供了 [`OsStrExt::encode_wide`] 方法，它输出一个
//! [`EncodeWide`] 迭代器。例如你可以 [`collect`] 这个迭代器以得到一个 `Vec<u16>`；
//! 随后即可取得指向该 vector 内容的指针，并把它喂给 Windows API。
//!
//! 这些 trait 与 [`OsString`]、[`OsStr`] 协同工作，使得即便字符串是不规范的 UTF-16，
//! 也能够把字符串从 Windows 取出再送回，做到 **往返（round-trip）** 而不丢失任何数据。
//!
//! [ill-formed-utf-16]: https://simonsapin.github.io/wtf-8/#ill-formed-utf-16
//! [`collect`]: crate::iter::Iterator::collect
//! [U+FFFD]: crate::char::REPLACEMENT_CHARACTER
//! [`std::ffi`]: crate::ffi

#![stable(feature = "rust1", since = "1.0.0")]

use alloc::wtf8::Wtf8Buf;

use crate::ffi::{OsStr, OsString};
use crate::fmt;
use crate::iter::FusedIterator;
use crate::sealed::Sealed;
use crate::sys::os_str::Buf;
use crate::sys::{AsInner, FromInner};

/// Windows 平台对 [`OsString`] 的特定扩展。
///
/// 本 trait 是密封的（sealed）：无法在标准库之外被实现。这样一来，将来新增方法就不会
/// 构成破坏性变更（breaking change）。
#[stable(feature = "rust1", since = "1.0.0")]
pub trait OsStringExt: Sealed {
    /// 从一个可能不规范（ill-formed）的、由 16 位码元构成的 UTF-16 切片创建出
    /// 一个 `OsString`。
    ///
    /// 这是无损的：对所得字符串调用 [`OsStrExt::encode_wide`] 将始终返回原始的码元。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::ffi::OsString;
    /// use std::os::windows::prelude::*;
    ///
    /// // "Unicode" 的 UTF-16 编码。
    /// let source = [0x0055, 0x006E, 0x0069, 0x0063, 0x006F, 0x0064, 0x0065];
    ///
    /// let string = OsString::from_wide(&source[..]);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    fn from_wide(wide: &[u16]) -> Self;
}

#[stable(feature = "rust1", since = "1.0.0")]
impl OsStringExt for OsString {
    fn from_wide(wide: &[u16]) -> OsString {
        FromInner::from_inner(Buf { inner: Wtf8Buf::from_wide(wide) })
    }
}

/// Windows 平台对 [`OsStr`] 的特定扩展。
///
/// 本 trait 是密封的（sealed）：无法在标准库之外被实现。这样一来，将来新增方法就不会
/// 构成破坏性变更（breaking change）。
#[stable(feature = "rust1", since = "1.0.0")]
pub trait OsStrExt: Sealed {
    /// 把一个 `OsStr` 重新编码为宽字符序列，即可能不规范（ill-formed）的 UTF-16。
    ///
    /// 这是无损的：调用 [`OsStringExt::from_wide`] 之后再对结果调用 `encode_wide`，
    /// 将得到原始的码元。注意该编码不会在末尾添加 null 终止符。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::ffi::OsString;
    /// use std::os::windows::prelude::*;
    ///
    /// // "Unicode" 的 UTF-16 编码。
    /// let source = [0x0055, 0x006E, 0x0069, 0x0063, 0x006F, 0x0064, 0x0065];
    ///
    /// let string = OsString::from_wide(&source[..]);
    ///
    /// let result: Vec<u16> = string.encode_wide().collect();
    /// assert_eq!(&source[..], &result[..]);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    fn encode_wide(&self) -> EncodeWide<'_>;
}

#[stable(feature = "rust1", since = "1.0.0")]
impl OsStrExt for OsStr {
    #[inline]
    fn encode_wide(&self) -> EncodeWide<'_> {
        EncodeWide { inner: self.as_inner().inner.encode_wide() }
    }
}

/// 由 [`OsStrExt::encode_wide`] 返回的迭代器。
#[stable(feature = "rust1", since = "1.0.0")]
#[derive(Clone)]
pub struct EncodeWide<'a> {
    inner: alloc::wtf8::EncodeWide<'a>,
}
#[stable(feature = "encode_wide_debug", since = "1.91.0")]
impl fmt::Debug for EncodeWide<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.inner, f)
    }
}
#[stable(feature = "rust1", since = "1.0.0")]
impl Iterator for EncodeWide<'_> {
    type Item = u16;

    #[inline]
    fn next(&mut self) -> Option<u16> {
        self.inner.next()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}
#[stable(feature = "encode_wide_fused_iterator", since = "1.62.0")]
impl FusedIterator for EncodeWide<'_> {}
