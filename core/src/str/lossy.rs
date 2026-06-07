use super::char::EscapeDebugExtArgs;
use super::from_utf8_unchecked;
use super::validations::utf8_char_width;
use crate::fmt;
use crate::fmt::{Formatter, Write};
use crate::iter::FusedIterator;

impl [u8] {
    /// 创建一个迭代器，遍历该切片中连续的合法 UTF-8 片段，
    /// 以及夹在它们之间的非 UTF-8 片段。
    ///
    /// 该迭代器产生的条目详见 [`Utf8Chunk`] 类型文档。
    ///
    /// # 示例
    ///
    /// 该函数把任意但大多是 UTF-8 的字节格式化为 Rust 源码中的 C 字符串字面量（`c"..."`）形式。
    ///
    /// ```
    /// use std::fmt::Write as _;
    ///
    /// pub fn cstr_literal(bytes: &[u8]) -> String {
    ///     let mut repr = String::new();
    ///     repr.push_str("c\"");
    ///     for chunk in bytes.utf8_chunks() {
    ///         for ch in chunk.valid().chars() {
    ///             // 转义 \0、\t、\r、\n、\\、\'、\"，并对不可打印字符使用 \u{...}。
    ///             write!(repr, "{}", ch.escape_debug()).unwrap();
    ///         }
    ///         for byte in chunk.invalid() {
    ///             write!(repr, "\\x{:02X}", byte).unwrap();
    ///         }
    ///     }
    ///     repr.push('"');
    ///     repr
    /// }
    ///
    /// fn main() {
    ///     let lit = cstr_literal(b"\xferris the \xf0\x9f\xa6\x80\x07");
    ///     let expected = stringify!(c"\xFErris the 🦀\u{7}");
    ///     assert_eq!(lit, expected);
    /// }
    /// ```
    #[stable(feature = "utf8_chunks", since = "1.79.0")]
    pub fn utf8_chunks(&self) -> Utf8Chunks<'_> {
        Utf8Chunks { source: self }
    }
}

/// [`Utf8Chunks`] 迭代器返回的条目。
///
/// `Utf8Chunk` 保存 UTF-8 解码时遇到的下一段内容：先是已经验证合法的 [`u8`] 序列，
/// 然后是导致本轮失败的非法片段。调用方可用它实现无分配的有损解码。
///
/// # 示例
///
/// ```
/// // 一个非法 UTF-8 字符串。
/// let bytes = b"foo\xF1\x80bar";
///
/// // 解码第一个 `Utf8Chunk`。
/// let chunk = bytes.utf8_chunks().next().unwrap();
///
/// // 前三个字符是合法 UTF-8。
/// assert_eq!("foo", chunk.valid());
///
/// // 第四个字符的字节序列是损坏的。
/// assert_eq!(b"\xF1\x80", chunk.invalid());
/// ```
#[stable(feature = "utf8_chunks", since = "1.79.0")]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Utf8Chunk<'a> {
    valid: &'a str,
    invalid: &'a [u8],
}

impl<'a> Utf8Chunk<'a> {
    /// 返回下一段已验证合法的 UTF-8 子串。
    ///
    /// 该子串可能为空，例如输入开头就是非法片段，或两个损坏 UTF-8 片段相邻。
    #[must_use]
    #[stable(feature = "utf8_chunks", since = "1.79.0")]
    pub fn valid(&self) -> &'a str {
        self.valid
    }

    /// 返回导致验证失败的非法字节序列。
    ///
    /// 返回切片最长为 3 字节，位于 [`valid`] 返回的合法子串之后。
    /// 解码应从该序列之后继续。
    ///
    /// 如果为空，说明这是字符串中的最后一个 chunk。若非空，则表示遇到了非预期字节，
    /// 或输入在一个 UTF-8 序列中途意外结束。
    ///
    /// 有损解码通常会把该序列替换为 [`U+FFFD REPLACEMENT CHARACTER`]。
    ///
    /// [`valid`]: Self::valid
    /// [`U+FFFD REPLACEMENT CHARACTER`]: crate::char::REPLACEMENT_CHARACTER
    #[must_use]
    #[stable(feature = "utf8_chunks", since = "1.79.0")]
    pub fn invalid(&self) -> &'a [u8] {
        self.invalid
    }
}

#[must_use]
#[unstable(feature = "str_internals", issue = "none")]
pub struct Debug<'a>(&'a [u8]);

#[unstable(feature = "str_internals", issue = "none")]
impl fmt::Debug for Debug<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_char('"')?;

        for chunk in self.0.utf8_chunks() {
            // 合法部分。
            // 这里会再次部分解析 UTF-8，性能上并非最优。
            {
                let valid = chunk.valid();
                let mut from = 0;
                for (i, c) in valid.char_indices() {
                    let esc = c.escape_debug_ext(EscapeDebugExtArgs {
                        escape_grapheme_extended: true,
                        escape_single_quote: false,
                        escape_double_quote: true,
                    });
                    // 如果该 char 需要转义，先写出目前积压的原文片段再写转义；否则继续累积。
                    if esc.len() != 1 {
                        f.write_str(&valid[from..i])?;
                        for c in esc {
                            f.write_char(c)?;
                        }
                        from = i + c.len_utf8();
                    }
                }
                f.write_str(&valid[from..])?;
            }

            // 字符串中损坏的部分按十六进制转义输出。
            for &b in chunk.invalid() {
                write!(f, "\\x{:02X}", b)?;
            }
        }

        f.write_char('"')
    }
}

/// 用于把大多是 UTF-8 的字节切片解码为字符串切片（[`&str`]）和字节切片
///（[`&[u8]`][byteslice]）的迭代器。
///
/// 该结构体由字节切片上的 [`utf8_chunks`] 方法创建。如果只需要把完全合法的 UTF-8
/// 字节切片转换为字符串切片，使用 [`from_utf8`] 更简单。
///
/// 该迭代器产生的条目详见 [`Utf8Chunk`] 类型文档。
///
/// [byteslice]: slice
/// [`utf8_chunks`]: slice::utf8_chunks
/// [`from_utf8`]: super::from_utf8
///
/// # 示例
///
/// 这可用于在不分配堆内存的情况下构造类似 [`String::from_utf8_lossy`] 的功能：
///
/// ```
/// fn from_utf8_lossy<F>(input: &[u8], mut push: F) where F: FnMut(&str) {
///     for chunk in input.utf8_chunks() {
///         push(chunk.valid());
///
///         if !chunk.invalid().is_empty() {
///             push("\u{FFFD}");
///         }
///     }
/// }
/// ```
///
/// [`String::from_utf8_lossy`]: ../../std/string/struct.String.html#method.from_utf8_lossy
#[must_use = "iterators are lazy and do nothing unless consumed"]
#[stable(feature = "utf8_chunks", since = "1.79.0")]
#[derive(Clone)]
pub struct Utf8Chunks<'a> {
    source: &'a [u8],
}

impl<'a> Utf8Chunks<'a> {
    #[doc(hidden)]
    #[unstable(feature = "str_internals", issue = "none")]
    pub fn debug(&self) -> Debug<'_> {
        Debug(self.source)
    }
}

#[stable(feature = "utf8_chunks", since = "1.79.0")]
impl<'a> Iterator for Utf8Chunks<'a> {
    type Item = Utf8Chunk<'a>;

    fn next(&mut self) -> Option<Utf8Chunk<'a>> {
        if self.source.is_empty() {
            return None;
        }

        const TAG_CONT_U8: u8 = 128;
        fn safe_get(xs: &[u8], i: usize) -> u8 {
            *xs.get(i).unwrap_or(&0)
        }

        let mut i = 0;
        let mut valid_up_to = 0;
        while i < self.source.len() {
            // SAFETY: 上一行已经保证 `i < self.source.len()`。
            // 出于某些原因，下面两种写法都会明显更慢：
            // while let Some(&byte) = self.source.get(i) {
            // while let Some(byte) = self.source.get(i).copied() {
            let byte = unsafe { *self.source.get_unchecked(i) };
            i += 1;

            if byte < 128 {
                // 这本可作为下面 match 的 `1 => ...` 分支处理；但对全 ASCII 输入这一常见情形，
                // 这里能避免把较大的 UTF8_CHAR_WIDTH 表加载进缓存。
            } else {
                let w = utf8_char_width(byte);

                match w {
                    2 => {
                        if safe_get(self.source, i) & 192 != TAG_CONT_U8 {
                            break;
                        }
                        i += 1;
                    }
                    3 => {
                        match (byte, safe_get(self.source, i)) {
                            (0xE0, 0xA0..=0xBF) => (),
                            (0xE1..=0xEC, 0x80..=0xBF) => (),
                            (0xED, 0x80..=0x9F) => (),
                            (0xEE..=0xEF, 0x80..=0xBF) => (),
                            _ => break,
                        }
                        i += 1;
                        if safe_get(self.source, i) & 192 != TAG_CONT_U8 {
                            break;
                        }
                        i += 1;
                    }
                    4 => {
                        match (byte, safe_get(self.source, i)) {
                            (0xF0, 0x90..=0xBF) => (),
                            (0xF1..=0xF3, 0x80..=0xBF) => (),
                            (0xF4, 0x80..=0x8F) => (),
                            _ => break,
                        }
                        i += 1;
                        if safe_get(self.source, i) & 192 != TAG_CONT_U8 {
                            break;
                        }
                        i += 1;
                        if safe_get(self.source, i) & 192 != TAG_CONT_U8 {
                            break;
                        }
                        i += 1;
                    }
                    _ => break,
                }
            }

            valid_up_to = i;
        }

        // SAFETY: `i <= self.source.len()`。`i` 只通过 `i += 1` 增长，
        // 且每次增长之间都会与 `self.source.len()` 比较；比较要么直接发生在 while
        // 条件 `i < self.source.len()` 中，要么通过
        // `safe_get(self.source, i) & 192 != TAG_CONT_U8` 间接完成。
        // 当最近一次 `i += 1` 使其不再小于长度时循环立即终止，因此它最多等于长度。
        let (inspected, remaining) = unsafe { self.source.split_at_unchecked(i) };
        self.source = remaining;

        // SAFETY: `valid_up_to <= i`，因为它只通过 `valid_up_to = i` 赋值，而 `i` 单调递增。
        let (valid, invalid) = unsafe { inspected.split_at_unchecked(valid_up_to) };

        Some(Utf8Chunk {
            // SAFETY: 到 `valid_up_to` 为止的所有字节都已按 UTF-8 规则验证为合法。
            valid: unsafe { from_utf8_unchecked(valid) },
            invalid,
        })
    }
}

#[stable(feature = "utf8_chunks", since = "1.79.0")]
impl FusedIterator for Utf8Chunks<'_> {}

#[stable(feature = "utf8_chunks", since = "1.79.0")]
impl fmt::Debug for Utf8Chunks<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("Utf8Chunks").field("source", &self.debug()).finish()
    }
}
