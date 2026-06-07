//! 定义 UTF-8 相关错误类型。

use crate::error::Error;
use crate::fmt;

/// 尝试把 [`u8`] 序列解释为字符串时可能出现的错误。
///
/// 因此，作用于 [`String`] 和 [`&str`] 的 `from_utf8` 系列函数和方法都会使用该错误。
/// 它记录“到哪里为止已经确认合法”和“错误序列有多长”，使调用方能做增量解码或有损替换。
///
/// [`String`]: ../../std/string/struct.String.html#method.from_utf8
/// [`&str`]: super::from_utf8
///
/// # 示例
///
/// 该错误类型的方法可用于在不分配堆内存的情况下构造类似
/// `String::from_utf8_lossy` 的功能：
///
/// ```
/// fn from_utf8_lossy<F>(mut input: &[u8], mut push: F) where F: FnMut(&str) {
///     loop {
///         match std::str::from_utf8(input) {
///             Ok(valid) => {
///                 push(valid);
///                 break
///             }
///             Err(error) => {
///                 let (valid, after_valid) = input.split_at(error.valid_up_to());
///                 unsafe {
///                     push(std::str::from_utf8_unchecked(valid))
///                 }
///                 push("\u{FFFD}");
///
///                 if let Some(invalid_sequence_length) = error.error_len() {
///                     input = &after_valid[invalid_sequence_length..]
///                 } else {
///                     break
///                 }
///             }
///         }
///     }
/// }
/// ```
#[derive(Copy, Eq, PartialEq, Clone, Debug)]
#[stable(feature = "rust1", since = "1.0.0")]
pub struct Utf8Error {
    pub(super) valid_up_to: usize,
    pub(super) error_len: Option<u8>,
}

impl Utf8Error {
    /// 返回给定输入中已验证为合法 UTF-8 的前缀结束索引。
    ///
    /// 这是能让 `from_utf8(&input[..index])` 返回 `Ok(_)` 的最大索引。
    /// 该索引总是位于字节边界上，也就是可安全切出已验证的 `&str` 前缀。
    ///
    /// # 示例
    ///
    /// 基本用法：
    ///
    /// ```
    /// use std::str;
    ///
    /// // vector 中的一些非法字节。
    /// let sparkle_heart = vec![0, 159, 146, 150];
    ///
    /// // std::str::from_utf8 返回 Utf8Error。
    /// let error = str::from_utf8(&sparkle_heart).unwrap_err();
    ///
    /// // 这里第二个字节非法。
    /// assert_eq!(1, error.valid_up_to());
    /// ```
    #[stable(feature = "utf8_error", since = "1.5.0")]
    #[rustc_const_stable(feature = "const_str_from_utf8_shared", since = "1.63.0")]
    #[must_use]
    #[inline]
    pub const fn valid_up_to(&self) -> usize {
        self.valid_up_to
    }

    /// 提供关于失败原因的更多信息：
    ///
    /// * `None`：输入意外结束。`self.valid_up_to()` 距离输入末尾还有 1 到 3 个字节。
    ///   如果正在增量解码字节流（例如文件或网络套接字），这可能只是一个合法 `char`
    ///   的 UTF-8 字节序列跨越了多个数据块。
    ///
    /// * `Some(len)`：遇到了非预期字节。`len` 是从 `valid_up_to()` 给出的索引开始的
    ///   非法字节序列长度。有损解码时，通常应先插入
    ///   [`U+FFFD REPLACEMENT CHARACTER`][U+FFFD]，再从该非法序列之后继续解码。
    ///
    /// [U+FFFD]: ../../std/char/constant.REPLACEMENT_CHARACTER.html
    #[stable(feature = "utf8_error_error_len", since = "1.20.0")]
    #[rustc_const_stable(feature = "const_str_from_utf8_shared", since = "1.63.0")]
    #[must_use]
    #[inline]
    pub const fn error_len(&self) -> Option<usize> {
        // FIXME(const-hack): 当 `map` 可用于 const 后，恢复为 `map`。
        match self.error_len {
            Some(len) => Some(len as usize),
            None => None,
        }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl fmt::Display for Utf8Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(error_len) = self.error_len {
            write!(
                f,
                "invalid utf-8 sequence of {} bytes from index {}",
                error_len, self.valid_up_to
            )
        } else {
            write!(f, "incomplete utf-8 byte sequence from index {}", self.valid_up_to)
        }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl Error for Utf8Error {}

/// 使用 [`from_str`] 解析 `bool` 失败时返回的错误。
///
/// [`from_str`]: super::FromStr::from_str
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
#[stable(feature = "rust1", since = "1.0.0")]
pub struct ParseBoolError;

#[stable(feature = "rust1", since = "1.0.0")]
impl fmt::Display for ParseBoolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        "provided string was not `true` or `false`".fmt(f)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl Error for ParseBoolError {}
