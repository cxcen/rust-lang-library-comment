//! UTF-8 和 UTF-16 解码迭代器。

use crate::error::Error;
use crate::fmt;
use crate::iter::FusedIterator;

/// 从 `u16` 迭代器中解码 UTF-16 code unit 的迭代器。
///
/// 该 `struct` 由 [`char`] 上的 [`decode_utf16`] 方法创建；更多行为说明见该方法文档。
/// 它会把合法代理对合成为一个 `char`，并把未配对代理项作为错误返回。
///
/// [`decode_utf16`]: char::decode_utf16
#[stable(feature = "decode_utf16", since = "1.9.0")]
#[derive(Clone, Debug)]
pub struct DecodeUtf16<I>
where
    I: Iterator<Item = u16>,
{
    iter: I,
    buf: Option<u16>,
}

/// 解码 UTF-16 code unit 时可能返回的错误。
///
/// 使用 [`DecodeUtf16`] 类型时会创建该 `struct`，表示输入中出现了无法组成合法
/// Unicode 标量值的未配对代理项。
#[stable(feature = "decode_utf16", since = "1.9.0")]
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct DecodeUtf16Error {
    code: u16,
}

/// 为 `iter` 中的 UTF-16 code unit 创建迭代器，遇到未配对代理项时返回 `Err`。
/// 见 [`char::decode_utf16`]。
#[inline]
pub(super) fn decode_utf16<I: IntoIterator<Item = u16>>(iter: I) -> DecodeUtf16<I::IntoIter> {
    DecodeUtf16 { iter: iter.into_iter(), buf: None }
}

#[stable(feature = "decode_utf16", since = "1.9.0")]
impl<I: Iterator<Item = u16>> Iterator for DecodeUtf16<I> {
    type Item = Result<char, DecodeUtf16Error>;

    fn next(&mut self) -> Option<Result<char, DecodeUtf16Error>> {
        let u = match self.buf.take() {
            Some(buf) => buf,
            None => self.iter.next()?,
        };

        if !u.is_utf16_surrogate() {
            // SAFETY: 该 `u16` 不是代理项，且必然不超过 U+FFFF，因此是合法 Unicode scalar value。
            Some(Ok(unsafe { char::from_u32_unchecked(u as u32) }))
        } else if u >= 0xDC00 {
            // 单独出现尾随代理项，无法组成合法代理对。
            Some(Err(DecodeUtf16Error { code: u }))
        } else {
            let u2 = match self.iter.next() {
                Some(u2) => u2,
                // 输入结束，前导代理项未能配对。
                None => return Some(Err(DecodeUtf16Error { code: u })),
            };
            if u2 < 0xDC00 || u2 > 0xDFFF {
                // `u2` 不是尾随代理项，因此当前前导代理项无法组成合法代理对；
                // 把 `u2` 放回缓冲区，下一轮按新的起点重新解码。
                self.buf = Some(u2);
                return Some(Err(DecodeUtf16Error { code: u }));
            }

            // 代理对合法，可以合成为补充平面的 Unicode scalar value。
            let c = (((u & 0x3ff) as u32) << 10 | (u2 & 0x3ff) as u32) + 0x1_0000;
            // SAFETY: 已确认 `u` 是前导代理项、`u2` 是尾随代理项；
            // 公式得到的值位于 U+10000..=U+10FFFF，是合法 `char`。
            Some(Ok(unsafe { char::from_u32_unchecked(c) }))
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let (low, high) = self.iter.size_hint();

        let (low_buf, high_buf) = match self.buf {
            // 缓冲为空，不会从缓冲产生额外元素。
            None => (0, 0),
            // `u` 不是代理项，因此一定会作为额外 `char` 产生。
            Some(u) if !u.is_utf16_surrogate() => (1, 1),
            // `u` 是前导代理项：前一分支已排除非代理项，且它不可能是尾随代理项；
            // 此时 `self.iter` 为空。
            //
            // 因为后续输入为空，`u` 无法配对，所以一定会成为一个额外元素（错误）。
            Some(_u) if high == Some(0) => (1, 1),
            // `u` 是前导代理项，且 `iter` 可能仍有输入。
            //
            // `u` 可能与下一个尾随代理项配对，此时不会额外增加元素；
            // 也可能无法配对而成为一个额外错误元素。
            Some(_u) => (0, 1),
        };

        // `self.iter` 可能全部由合法代理对组成（每 2 个 code unit 产生 1 个 `char`），
        // 也可能全部是非代理项（每 1 个 code unit 产生 1 个 `char`）。
        //
        // 当下界为奇数时，至少有一个元素无法与 `self.iter` 中其他元素配对，因此向上取整。
        let low = low.div_ceil(2) + low_buf;
        let high = high.and_then(|h| h.checked_add(high_buf));

        (low, high)
    }
}

#[stable(feature = "decode_utf16_fused_iterator", since = "1.75.0")]
impl<I: Iterator<Item = u16> + FusedIterator> FusedIterator for DecodeUtf16<I> {}

impl DecodeUtf16Error {
    /// 返回导致该错误的未配对代理项。
    #[must_use]
    #[stable(feature = "decode_utf16", since = "1.9.0")]
    pub fn unpaired_surrogate(&self) -> u16 {
        self.code
    }
}

#[stable(feature = "decode_utf16", since = "1.9.0")]
impl fmt::Display for DecodeUtf16Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unpaired surrogate found: {:x}", self.code)
    }
}

#[stable(feature = "decode_utf16", since = "1.9.0")]
impl Error for DecodeUtf16Error {}
