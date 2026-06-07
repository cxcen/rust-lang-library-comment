//! ASCII `[u8]` 上的操作。

use core::ascii::EscapeDefault;

use crate::fmt::{self, Write};
#[cfg(not(any(
    all(target_arch = "x86_64", target_feature = "sse2"),
    all(target_arch = "loongarch64", target_feature = "lsx")
)))]
use crate::intrinsics::const_eval_select;
use crate::{ascii, iter, ops};

impl [u8] {
    /// 检查该切片中的所有字节是否都位于 ASCII 范围内。
///
    /// 空切片返回 `true`。
    #[stable(feature = "ascii_methods_on_intrinsics", since = "1.23.0")]
    #[rustc_const_stable(feature = "const_slice_is_ascii", since = "1.74.0")]
    #[must_use]
    #[inline]
    pub const fn is_ascii(&self) -> bool {
        is_ascii(self)
    }

    /// 如果该切片满足 [`is_ascii`](Self::is_ascii)，将其作为
    /// [ASCII characters](`ascii::Char`) 切片返回；否则返回 `None`。
    #[unstable(feature = "ascii_char", issue = "110998")]
    #[must_use]
    #[inline]
    pub const fn as_ascii(&self) -> Option<&[ascii::Char]> {
        if self.is_ascii() {
            // SAFETY: 刚刚已经检查所有字节都是 ASCII。
            Some(unsafe { self.as_ascii_unchecked() })
        } else {
            None
        }
    }

    /// 不检查有效性，直接把字节切片转换成 ASCII 字符切片。
    ///
    /// # 安全性(Safety）
    ///
    /// 切片中的每个字节都必须位于 `0..=127`；否则本函数会产生 UB。
    #[unstable(feature = "ascii_char", issue = "110998")]
    #[must_use]
    #[inline]
    pub const unsafe fn as_ascii_unchecked(&self) -> &[ascii::Char] {
        let byte_ptr: *const [u8] = self;
        let ascii_ptr = byte_ptr as *const [ascii::Char];
        // SAFETY: 调用方承诺所有字节都是 ASCII。
        unsafe { &*ascii_ptr }
    }

    /// 检查两个切片按 ASCII 大小写无关规则是否匹配。
///
    /// 语义等同 `to_ascii_lowercase(a) == to_ascii_lowercase(b)`，
    /// 但不会分配或复制临时值。
    #[stable(feature = "ascii_methods_on_intrinsics", since = "1.23.0")]
    #[rustc_const_stable(feature = "const_eq_ignore_ascii_case", since = "1.89.0")]
    #[must_use]
    #[inline]
    pub const fn eq_ignore_ascii_case(&self, other: &[u8]) -> bool {
        if self.len() != other.len() {
            return false;
        }

        // FIXME(const-hack): 当 const 中允许 `core::iter::zip` 后，可以恢复原实现：
        //  self.len() == other.len() && iter::zip(self, other).all(|(a, b)| a.eq_ignore_ascii_case(b))
        let mut a = self;
        let mut b = other;

        while let ([first_a, rest_a @ ..], [first_b, rest_b @ ..]) = (a, b) {
            if first_a.eq_ignore_ascii_case(&first_b) {
                a = rest_a;
                b = rest_b;
            } else {
                return false;
            }
        }

        true
    }

    /// 原地把该切片转换成等价的 ASCII 大写形式。
///
    /// ASCII 字母 'a' 到 'z' 会映射为 'A' 到 'Z'；非 ASCII 字节保持不变。
///
    /// 如需返回新的大写值且不修改现有值，请使用 [`to_ascii_uppercase`]。
    ///
    /// [`to_ascii_uppercase`]: #method.to_ascii_uppercase
    #[stable(feature = "ascii_methods_on_intrinsics", since = "1.23.0")]
    #[rustc_const_stable(feature = "const_make_ascii", since = "1.84.0")]
    #[inline]
    pub const fn make_ascii_uppercase(&mut self) {
        // FIXME(const-hack): 理想情况下可用 `for` 循环迭代，但当前常量表达式中还不允许。
        let mut i = 0;
        while i < self.len() {
            let byte = &mut self[i];
            byte.make_ascii_uppercase();
            i += 1;
        }
    }

    /// 原地把该切片转换成等价的 ASCII 小写形式。
///
    /// ASCII 字母 'A' 到 'Z' 会映射为 'a' 到 'z'；非 ASCII 字节保持不变。
///
    /// 如需返回新的小写值且不修改现有值，请使用 [`to_ascii_lowercase`]。
    ///
    /// [`to_ascii_lowercase`]: #method.to_ascii_lowercase
    #[stable(feature = "ascii_methods_on_intrinsics", since = "1.23.0")]
    #[rustc_const_stable(feature = "const_make_ascii", since = "1.84.0")]
    #[inline]
    pub const fn make_ascii_lowercase(&mut self) {
        // FIXME(const-hack): 理想情况下可用 `for` 循环迭代，但当前常量表达式中还不允许。
        let mut i = 0;
        while i < self.len() {
            let byte = &mut self[i];
            byte.make_ascii_lowercase();
            i += 1;
        }
    }

    /// 返回一个迭代器，把该切片当作 ASCII 字符串并产出转义后的字节。
    ///
    /// # 示例
    ///
    /// ```
    /// let s = b"0\t\r\n'\"\\\x9d";
    /// let escaped = s.escape_ascii().to_string();
    /// assert_eq!(escaped, "0\\t\\r\\n\\'\\\"\\\\\\x9d");
    /// ```
    #[must_use = "this returns the escaped bytes as an iterator, \
                  without modifying the original"]
    #[stable(feature = "inherent_ascii_escape", since = "1.60.0")]
    pub fn escape_ascii(&self) -> EscapeAscii<'_> {
        EscapeAscii { inner: self.iter().flat_map(EscapeByte) }
    }

    /// 返回去掉开头 ASCII 空白字节后的字节切片。
///
    /// “空白”采用 [`u8::is_ascii_whitespace`] 使用的定义。
    ///
    /// # 示例
    ///
    /// ```
    /// assert_eq!(b" \t hello world\n".trim_ascii_start(), b"hello world\n");
    /// assert_eq!(b"  ".trim_ascii_start(), b"");
    /// assert_eq!(b"".trim_ascii_start(), b"");
    /// ```
    #[stable(feature = "byte_slice_trim_ascii", since = "1.80.0")]
    #[rustc_const_stable(feature = "byte_slice_trim_ascii", since = "1.80.0")]
    #[inline]
    pub const fn trim_ascii_start(&self) -> &[u8] {
        let mut bytes = self;
        // Note: 使用基于模式匹配的写法（而不是索引）可让该函数成为 const。
        while let [first, rest @ ..] = bytes {
            if first.is_ascii_whitespace() {
                bytes = rest;
            } else {
                break;
            }
        }
        bytes
    }

    /// 返回去掉末尾 ASCII 空白字节后的字节切片。
///
    /// “空白”采用 [`u8::is_ascii_whitespace`] 使用的定义。
    ///
    /// # 示例
    ///
    /// ```
    /// assert_eq!(b"\r hello world\n ".trim_ascii_end(), b"\r hello world");
    /// assert_eq!(b"  ".trim_ascii_end(), b"");
    /// assert_eq!(b"".trim_ascii_end(), b"");
    /// ```
    #[stable(feature = "byte_slice_trim_ascii", since = "1.80.0")]
    #[rustc_const_stable(feature = "byte_slice_trim_ascii", since = "1.80.0")]
    #[inline]
    pub const fn trim_ascii_end(&self) -> &[u8] {
        let mut bytes = self;
        // Note: 使用基于模式匹配的写法（而不是索引）可让该函数成为 const。
        while let [rest @ .., last] = bytes {
            if last.is_ascii_whitespace() {
                bytes = rest;
            } else {
                break;
            }
        }
        bytes
    }

    /// 返回去掉开头和末尾 ASCII 空白字节后的字节切片。
///
    /// “空白”采用 [`u8::is_ascii_whitespace`] 使用的定义。
    ///
    /// # 示例
    ///
    /// ```
    /// assert_eq!(b"\r hello world\n ".trim_ascii(), b"hello world");
    /// assert_eq!(b"  ".trim_ascii(), b"");
    /// assert_eq!(b"".trim_ascii(), b"");
    /// ```
    #[stable(feature = "byte_slice_trim_ascii", since = "1.80.0")]
    #[rustc_const_stable(feature = "byte_slice_trim_ascii", since = "1.80.0")]
    #[inline]
    pub const fn trim_ascii(&self) -> &[u8] {
        self.trim_ascii_start().trim_ascii_end()
    }
}

impl_fn_for_zst! {
    #[derive(Clone)]
    struct EscapeByte impl Fn = |byte: &u8| -> ascii::EscapeDefault {
        ascii::escape_default(*byte)
    };
}

/// 遍历字节切片转义后形式的迭代器。
///
/// 该 `struct` 由 [`slice::escape_ascii`] 方法创建；更多信息见该方法文档。
#[stable(feature = "inherent_ascii_escape", since = "1.60.0")]
#[derive(Clone)]
#[must_use = "iterators are lazy and do nothing unless consumed"]
pub struct EscapeAscii<'a> {
    inner: iter::FlatMap<super::Iter<'a, u8>, ascii::EscapeDefault, EscapeByte>,
}

#[stable(feature = "inherent_ascii_escape", since = "1.60.0")]
impl<'a> iter::Iterator for EscapeAscii<'a> {
    type Item = u8;
    #[inline]
    fn next(&mut self) -> Option<u8> {
        self.inner.next()
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
    #[inline]
    fn try_fold<Acc, Fold, R>(&mut self, init: Acc, fold: Fold) -> R
    where
        Fold: FnMut(Acc, Self::Item) -> R,
        R: ops::Try<Output = Acc>,
    {
        self.inner.try_fold(init, fold)
    }
    #[inline]
    fn fold<Acc, Fold>(self, init: Acc, fold: Fold) -> Acc
    where
        Fold: FnMut(Acc, Self::Item) -> Acc,
    {
        self.inner.fold(init, fold)
    }
    #[inline]
    fn last(mut self) -> Option<u8> {
        self.next_back()
    }
}

#[stable(feature = "inherent_ascii_escape", since = "1.60.0")]
impl<'a> iter::DoubleEndedIterator for EscapeAscii<'a> {
    fn next_back(&mut self) -> Option<u8> {
        self.inner.next_back()
    }
}
#[stable(feature = "inherent_ascii_escape", since = "1.60.0")]
impl<'a> iter::FusedIterator for EscapeAscii<'a> {}
#[stable(feature = "inherent_ascii_escape", since = "1.60.0")]
impl<'a> fmt::Display for EscapeAscii<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // 拆开迭代器，包括 flatmap 的前/后部分，以处理迭代器已经被部分消费的情况。
        let (front, slice, back) = self.clone().inner.into_parts();
        let front = front.unwrap_or(EscapeDefault::empty());
        let mut bytes = slice.unwrap_or_default().as_slice();
        let back = back.unwrap_or(EscapeDefault::empty());

        // 通常为空，因此 formatter 不需要做额外工作。
        for byte in front {
            f.write_char(byte as char)?;
        }

        fn needs_escape(b: u8) -> bool {
            b > 0x7E || b < 0x20 || b == b'\\' || b == b'\'' || b == b'"'
        }

        while bytes.len() > 0 {
            // 可打印且无需转义的 ASCII 子集 fast path。
            let prefix = bytes.iter().take_while(|&&b| !needs_escape(b)).count();
            // SAFETY: prefix 长度来自对同一切片中字节的计数，因此在边界内。
            let (prefix, remainder) = unsafe { bytes.split_at_unchecked(prefix) };
            // SAFETY: prefix 是 ASCII 子集，因此是有效 UTF-8 序列。
            let prefix = unsafe { crate::str::from_utf8_unchecked(prefix) };

            f.write_str(prefix)?; // fast path 部分。

            bytes = remainder;

            if let Some(&b) = bytes.first() {
                // 已保证非空；按 str 写入更好。
                fmt::Display::fmt(&ascii::escape_default(b), f)?;
                bytes = &bytes[1..];
            }
        }

        // 同样通常为空。
        for byte in back {
            f.write_char(byte as char)?;
        }
        Ok(())
    }
}
#[stable(feature = "inherent_ascii_escape", since = "1.60.0")]
impl<'a> fmt::Debug for EscapeAscii<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EscapeAscii").finish_non_exhaustive()
    }
}

/// *不使用* chunk-at-a-time 优化的 ASCII 检测。
///
/// 这里被仔细组织成能生成较小代码的形式：在 `-O` 下比“显而易见”的写法在
/// `-C opt-level=s` 下还小。如果修改它，请务必运行汇编测试，并在需要时更新测试。
#[unstable(feature = "str_internals", issue = "none")]
#[doc(hidden)]
#[inline]
pub const fn is_ascii_simple(mut bytes: &[u8]) -> bool {
    while let [rest @ .., last] = bytes {
        if !last.is_ascii() {
            break;
        }
        bytes = rest;
    }
    bytes.is_empty()
}

/// 优化版 ASCII 检测；可能时使用 usize-at-a-time 操作，而不是 byte-at-a-time 操作。
///
/// 这里使用的算法很简单。如果 `s` 太短，就逐字节检查。否则：
///
/// - 用未对齐 load 读取第一个 word。
/// - 对齐指针，然后用对齐 load 读取后续 word 直到末尾。
/// - 用未对齐 load 读取 `s` 的最后一个 `usize`。
///
/// 如果这些 load 中任意一个产生的值让上面的 `contains_nonascii` 返回 true，
/// 就可确定答案为 false。
#[cfg(not(any(
    all(target_arch = "x86_64", target_feature = "sse2"),
    all(target_arch = "loongarch64", target_feature = "lsx")
)))]
#[inline]
#[rustc_allow_const_fn_unstable(const_eval_select)] // fallback 实现具有相同行为
const fn is_ascii(s: &[u8]) -> bool {
    // 运行时版本与编译期版本行为相同，只是优化更多。
    const_eval_select!(
        @capture { s: &[u8] } -> bool:
        if const {
            is_ascii_simple(s)
        } else {
            /// 如果 word `v` 中任意字节不是 ASCII（>= 128），返回 `true`。这借鉴自
            /// `../str/mod.rs`，那里用类似技巧做 UTF-8 校验。
            const fn contains_nonascii(v: usize) -> bool {
                const NONASCII_MASK: usize = usize::repeat_u8(0x80);
                (NONASCII_MASK & v) != 0
            }

            const USIZE_SIZE: usize = size_of::<usize>();

            let len = s.len();
            let align_offset = s.as_ptr().align_offset(USIZE_SIZE);

            // 如果 word-at-a-time 实现没有收益，就退回标量循环。
            //
            // 对 `size_of::<usize>()` 不足以作为 `usize` 对齐的架构，也走这条路径；
            // 这是一个少见边界情况。
            if len < USIZE_SIZE || len < align_offset || USIZE_SIZE < align_of::<usize>() {
                return is_ascii_simple(s);
            }

            // 我们总是用未对齐方式读取第一个 word；如果 `align_offset` 为 0，
            // 对齐读取会再次读到同一个值。
            let offset_to_aligned = if align_offset == 0 { USIZE_SIZE } else { align_offset };

            let start = s.as_ptr();
            // SAFETY: 上面已经排除 `len < USIZE_SIZE` 的情况。
            let first_word = unsafe { (start as *const usize).read_unaligned() };

            if contains_nonascii(first_word) {
                return false;
            }
            // 上面已经隐式检查过这一点。注意 `offset_to_aligned` 要么是 `align_offset`，
            // 要么是 `USIZE_SIZE`，两者都已在上面显式检查。
            debug_assert!(offset_to_aligned <= len);

            // SAFETY: word_ptr 是正确对齐的 usize 指针，用来读取切片中间 chunk。
            let mut word_ptr = unsafe { start.add(offset_to_aligned) as *const usize };

            // `byte_pos` 是 `word_ptr` 的字节索引，用于循环结束检查。
            let mut byte_pos = offset_to_aligned;

            // 对齐的防御性检查，因为接下来要做多次 load。实践中除非 `align_offset` 有 bug，
            // 否则这里不应失败。虽然该方法在 CTFE 中允许伪失败，但如果没有对齐信息，
            // 它应当早先为 `align_offset` 返回 `usize::MAX`，从而走标量路径而不是这里；
            // 因此只要到达这里，该检查就应通过。
            debug_assert!(word_ptr.is_aligned_to(align_of::<usize>()));

            // 读取后续 word，直到最后一个对齐 word 之前；最后一个对齐 word 留给后面的尾部检查，
            // 以确保 tail 最多只有一个 `usize`，从而避免额外的 `byte_pos == len` 分支。
            while byte_pos < len - USIZE_SIZE {
                // 健全性检查：读取位于边界内。
                debug_assert!(byte_pos + USIZE_SIZE <= len);
                // 并检查我们关于 `byte_pos` 的假设成立。
                debug_assert!(word_ptr.cast::<u8>() == start.wrapping_add(byte_pos));

                // SAFETY: 已知 `word_ptr` 正确对齐（由 `align_offset` 保证），且从
                // `word_ptr` 到末尾有足够字节。
                let word = unsafe { word_ptr.read() };
                if contains_nonascii(word) {
                    return false;
                }

                byte_pos += USIZE_SIZE;
                // SAFETY: 已知 `byte_pos <= len - USIZE_SIZE`，因此这次 `add` 后
                // `word_ptr` 至多到达一过末尾位置。
                word_ptr = unsafe { word_ptr.add(1) };
            }

            // 健全性检查：确认确实至多只剩一个 `usize`；这应由循环条件保证。
            debug_assert!(byte_pos <= len && len - byte_pos <= USIZE_SIZE);

            // SAFETY: 依赖 `len >= USIZE_SIZE`，该条件已在开头检查。
            let last_word = unsafe { (start.add(len - USIZE_SIZE) as *const usize).read_unaligned() };

            !contains_nonascii(last_word)
        }
    )
}

/// 针对 `x86-64` 使用 `pmovmskb` 指令、针对 `loongarch64` 使用 `vmskltz.b` 指令优化的
/// ASCII 检测。
///
/// 其它平台不太可能从这种代码结构获益，因此会使用 SWAR 技巧按 `usize` 大小的 chunk 检测 ASCII。
#[cfg(any(
    all(target_arch = "x86_64", target_feature = "sse2"),
    all(target_arch = "loongarch64", target_feature = "lsx")
))]
#[inline]
const fn is_ascii(bytes: &[u8]) -> bool {
    // fast path 每次处理 32 字节 chunk，以启用自动向量化并使用 `pmovmskb`。
    // 两个 128-bit vector register 可以 OR 到一起，再检测结果 vector 中是否存在非 ASCII 字节。
    const CHUNK_SIZE: usize = 32;

    let mut i = 0;

    while i + CHUNK_SIZE <= bytes.len() {
        let chunk_end = i + CHUNK_SIZE;

        // 让 LLVM 在 x86-64 上生成 `pmovmskb` 指令，从每个字节的最高位
        // 构造掩码。ASCII 字节小于 128 (0x80)，因此最高位不会被设置。
        let mut count = 0;
        while i < chunk_end {
            count += bytes[i].is_ascii() as u8;
            i += 1;
        }

        // 所有字节都应 <= 127，因此计数应等于 chunk 大小。
        if count != CHUNK_SIZE as u8 {
            return false;
        }
    }

    // 处理剩余的 `bytes.len() % N` 个字节。
    let mut is_ascii = true;
    while i < bytes.len() {
        is_ascii &= bytes[i].is_ascii();
        i += 1;
    }

    is_ascii
}
