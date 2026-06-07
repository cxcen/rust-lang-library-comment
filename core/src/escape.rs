//! 字符转义的辅助代码。

use crate::ascii;
use crate::fmt::{self, Write};
use crate::marker::PhantomData;
use crate::num::NonZero;
use crate::ops::Range;

const HEX_DIGITS: [ascii::Char; 16] = *b"0123456789abcdef".as_ascii().unwrap();

/// 使用 `\x` 形式转义字符。
///
/// 返回包含转义表示的缓冲区及其对应范围。
#[inline]
const fn backslash<const N: usize>(a: ascii::Char) -> ([ascii::Char; N], Range<u8>) {
    const { assert!(N >= 2) };

    let mut output = [ascii::Char::Null; N];

    output[0] = ascii::Char::ReverseSolidus;
    output[1] = a;

    (output, 0..2)
}

/// 使用 `\xNN` 形式转义字符。
///
/// 返回包含转义表示的缓冲区及其对应范围。
#[inline]
const fn hex_escape<const N: usize>(byte: u8) -> ([ascii::Char; N], Range<u8>) {
    const { assert!(N >= 4) };

    let mut output = [ascii::Char::Null; N];

    let hi = HEX_DIGITS[(byte >> 4) as usize];
    let lo = HEX_DIGITS[(byte & 0xf) as usize];

    output[0] = ascii::Char::ReverseSolidus;
    output[1] = ascii::Char::SmallX;
    output[2] = hi;
    output[3] = lo;

    (output, 0..4)
}

/// 返回包含原样字符的缓冲区及其对应范围。
#[inline]
const fn verbatim<const N: usize>(a: ascii::Char) -> ([ascii::Char; N], Range<u8>) {
    const { assert!(N >= 1) };

    let mut output = [ascii::Char::Null; N];

    output[0] = a;

    (output, 0..1)
}

/// 转义 ASCII 字符。
///
/// 返回包含转义表示的缓冲区及其对应范围。
const fn escape_ascii<const N: usize>(byte: u8) -> ([ascii::Char; N], Range<u8>) {
    const { assert!(N >= 4) };

    #[cfg(feature = "optimize_for_size")]
    {
        match byte {
            b'\t' => backslash(ascii::Char::SmallT),
            b'\r' => backslash(ascii::Char::SmallR),
            b'\n' => backslash(ascii::Char::SmallN),
            b'\\' => backslash(ascii::Char::ReverseSolidus),
            b'\'' => backslash(ascii::Char::Apostrophe),
            b'"' => backslash(ascii::Char::QuotationMark),
            0x00..=0x1F | 0x7F => hex_escape(byte),
            _ => match ascii::Char::from_u8(byte) {
                Some(a) => verbatim(a),
                None => hex_escape(byte),
            },
        }
    }

    #[cfg(not(feature = "optimize_for_size"))]
    {
        /// 查找表帮助确定字符的显示方式。
        ///
        /// 因为 ASCII 字符始终只有 7 位，所以可以利用第 8 位来表示结果是否经过转义。
        ///
        /// 另外使用 0x80（转义后的 NUL 字符）来表示十六进制转义字节，因为不会出现
        /// 转义后的 NUL。
        const LOOKUP: [u8; 256] = {
            let mut arr = [0; 256];
            let mut idx = 0;
            while idx <= 255 {
                arr[idx] = match idx as u8 {
                    // 使用第 8 位表示已转义。
                    b'\t' => 0x80 | b't',
                    b'\r' => 0x80 | b'r',
                    b'\n' => 0x80 | b'n',
                    b'\\' => 0x80 | b'\\',
                    b'\'' => 0x80 | b'\'',
                    b'"' => 0x80 | b'"',

                    // 使用 NUL 表示十六进制转义。
                    0x00..=0x1F | 0x7F..=0xFF => 0x80 | b'\0',

                    idx => idx,
                };
                idx += 1;
            }
            arr
        };

        let lookup = LOOKUP[byte as usize];

        // 第 8 位表示需要转义。
        let lookup_escaped = lookup & 0x80 != 0;

        // SAFETY: 这里显式屏蔽第 8 位，得到的值一定是 7 位 ASCII 字符。
        let lookup_ascii = unsafe { ascii::Char::from_u8_unchecked(lookup & 0x7F) };

        if lookup_escaped {
            // NUL 表示十六进制转义。
            if matches!(lookup_ascii, ascii::Char::Null) {
                hex_escape(byte)
            } else {
                backslash(lookup_ascii)
            }
        } else {
            verbatim(lookup_ascii)
        }
    }
}

/// 使用 `\u{NNNN}` 形式转义字符。
///
/// 返回包含转义表示的缓冲区及其对应范围。
const fn escape_unicode<const N: usize>(c: char) -> ([ascii::Char; N], Range<u8>) {
    const { assert!(N >= 10 && N < u8::MAX as usize) };

    let c = c as u32;

    // 与 `1` 做 OR 可确保 `c == 0` 时仍计算出需要打印一位数字。
    let start = (c | 1).leading_zeros() as usize / 4 - 2;

    let mut output = [ascii::Char::Null; N];
    output[3] = HEX_DIGITS[((c >> 20) & 15) as usize];
    output[4] = HEX_DIGITS[((c >> 16) & 15) as usize];
    output[5] = HEX_DIGITS[((c >> 12) & 15) as usize];
    output[6] = HEX_DIGITS[((c >> 8) & 15) as usize];
    output[7] = HEX_DIGITS[((c >> 4) & 15) as usize];
    output[8] = HEX_DIGITS[((c >> 0) & 15) as usize];
    output[9] = ascii::Char::RightCurlyBracket;
    output[start + 0] = ascii::Char::ReverseSolidus;
    output[start + 1] = ascii::Char::SmallU;
    output[start + 2] = ascii::Char::LeftCurlyBracket;

    (output, (start as u8)..(N as u8))
}

#[derive(Clone, Copy)]
union MaybeEscapedCharacter<const N: usize> {
    pub escape_seq: [ascii::Char; N],
    pub literal: char,
}

/// 标记类型，表示字符总是经过转义，用于优化迭代器实现。
#[derive(Clone, Copy)]
#[non_exhaustive]
pub(crate) struct AlwaysEscaped;

/// 标记类型，表示字符可能经过转义，用于优化迭代器实现。
#[derive(Clone, Copy)]
#[non_exhaustive]
pub(crate) struct MaybeEscaped;

/// 遍历可能经过转义的字符的迭代器。
#[derive(Clone)]
pub(crate) struct EscapeIterInner<const N: usize, ESCAPING> {
    // 不变量：
    //
    // 如果 `alive.end <= Self::LITERAL_ESCAPE_START`，则 `data` 必须处于
    // `escape_seq` variant，并且 `alive` 范围内必须是可打印 ASCII 字符。
    //
    // 如果 `alive.end > Self::LITERAL_ESCAPE_START`，则 `data` 必须处于
    // `literal` variant 并包含一个 `char`，且 `alive` 范围的长度最多为 `1`。
    data: MaybeEscapedCharacter<N>,
    alive: Range<u8>,
    escaping: PhantomData<ESCAPING>,
}

impl<const N: usize, ESCAPING> EscapeIterInner<N, ESCAPING> {
    const LITERAL_ESCAPE_START: u8 = 128;

    /// # 安全性(Safety）
    ///
    /// `data` 必须处于 `escape_seq` variant，且 `alive` 给出的范围内必须包含有效的转义序列。
    #[inline]
    const unsafe fn new(data: MaybeEscapedCharacter<N>, alive: Range<u8>) -> Self {
        // 因为 `alive.end` 最大为 `Self::LITERAL_ESCAPE_START`，
        // 更长的转义序列没有用处。
        const { assert!(N < Self::LITERAL_ESCAPE_START as usize) };

        // 检查边界，这也会隐式检查 `alive.end <= Self::LITERAL_ESCAPE_START` 不变量。
        debug_assert!(alive.end <= (N + 1) as u8);

        Self { data, alive, escaping: PhantomData }
    }

    pub(crate) const fn backslash(c: ascii::Char) -> Self {
        let (escape_seq, alive) = backslash(c);
        // SAFETY: `escape_seq` 在 `alive` 给出的范围内包含有效的转义序列。
        unsafe { Self::new(MaybeEscapedCharacter { escape_seq }, alive) }
    }

    pub(crate) const fn ascii(c: u8) -> Self {
        let (escape_seq, alive) = escape_ascii(c);
        // SAFETY: `escape_seq` 在 `alive` 给出的范围内包含有效的转义序列。
        unsafe { Self::new(MaybeEscapedCharacter { escape_seq }, alive) }
    }

    pub(crate) const fn unicode(c: char) -> Self {
        let (escape_seq, alive) = escape_unicode(c);
        // SAFETY: `escape_seq` 在 `alive` 给出的范围内包含有效的转义序列。
        unsafe { Self::new(MaybeEscapedCharacter { escape_seq }, alive) }
    }

    #[inline]
    pub(crate) const fn empty() -> Self {
        // SAFETY: `0..0` 保证这是一个空转义序列。
        unsafe { Self::new(MaybeEscapedCharacter { escape_seq: [ascii::Char::Null; N] }, 0..0) }
    }

    #[inline]
    pub(crate) fn len(&self) -> usize {
        usize::from(self.alive.end - self.alive.start)
    }

    #[inline]
    pub(crate) fn advance_by(&mut self, n: usize) -> Result<(), NonZero<usize>> {
        self.alive.advance_by(n)
    }

    #[inline]
    pub(crate) fn advance_back_by(&mut self, n: usize) -> Result<(), NonZero<usize>> {
        self.alive.advance_back_by(n)
    }

    /// 如果 `self.data` 的 `literal` variant 中包含 `char`，返回该 `char`。
    #[inline]
    const fn to_char(&self) -> Option<char> {
        if self.alive.end > Self::LITERAL_ESCAPE_START {
            // SAFETY: 上面已经检查 `self.data` 处于 `literal` variant，
            //         因而其中包含一个 `char`。
            return Some(unsafe { self.data.literal });
        }

        None
    }

    /// 将 `self.data` 的 `escape_seq` variant 中的可打印 ASCII 字符作为字符串返回。
    ///
    /// # 安全性(Safety）
    ///
    /// - `self.data` 必须处于 `escape_seq` variant，且其中包含可打印 ASCII 字符。
    /// - `self.alive` 必须是 `self.data.escape_seq` 的有效范围。
    #[inline]
    unsafe fn to_str_unchecked(&self) -> &str {
        debug_assert!(self.alive.end <= Self::LITERAL_ESCAPE_START);

        // SAFETY: 调用者保证 `self.data` 处于 `escape_seq` variant 并包含
        //         可打印 ASCII 字符，且 `self.alive` 是 `self.data.escape_seq`
        //         的有效范围。
        unsafe {
            self.data
                .escape_seq
                .get_unchecked(usize::from(self.alive.start)..usize::from(self.alive.end))
                .as_str()
        }
    }
}

impl<const N: usize> EscapeIterInner<N, AlwaysEscaped> {
    pub(crate) fn next(&mut self) -> Option<u8> {
        let i = self.alive.next()?;

        // SAFETY: `AlwaysEscaped` 标记保证 `self.data` 处于 `escape_seq`
        //         variant 且包含可打印 ASCII 字符；`i` 来自 `alive`，
        //         因而是 `self.data.escape_seq` 的有效索引。
        unsafe { Some(self.data.escape_seq.get_unchecked(usize::from(i)).to_u8()) }
    }

    pub(crate) fn next_back(&mut self) -> Option<u8> {
        let i = self.alive.next_back()?;

        // SAFETY: `AlwaysEscaped` 标记保证 `self.data` 处于 `escape_seq`
        //         variant 且包含可打印 ASCII 字符；`i` 来自 `alive`，
        //         因而是 `self.data.escape_seq` 的有效索引。
        unsafe { Some(self.data.escape_seq.get_unchecked(usize::from(i)).to_u8()) }
    }
}

impl<const N: usize> EscapeIterInner<N, MaybeEscaped> {
    // 这是唯一会创建 `self.data` 的 `literal` variant 中包含 `char` 的
    // `EscapeIterInner` 的路径；因此 `AlwaysEscaped` 标记可以保证
    // `self.data` 处于 `escape_seq` variant 且包含可打印 ASCII 字符。
    pub(crate) const fn printable(c: char) -> Self {
        Self {
            data: MaybeEscapedCharacter { literal: c },
            // 维持 `alive.end > Self::LITERAL_ESCAPE_START` 不变量，并确保
            // `len` 在遍历一个字符字面量时行为正确。
            alive: Self::LITERAL_ESCAPE_START..(Self::LITERAL_ESCAPE_START + 1),
            escaping: PhantomData,
        }
    }

    pub(crate) fn next(&mut self) -> Option<char> {
        let i = self.alive.next()?;

        if let Some(c) = self.to_char() {
            return Some(c);
        }

        // SAFETY: 走到这里时，`self.data` 必须处于 `escape_seq` variant
        //         且包含可打印 ASCII 字符；`i` 来自 `alive`，因而是
        //         `self.data.escape_seq` 的有效索引。
        Some(char::from(unsafe { self.data.escape_seq.get_unchecked(usize::from(i)).to_u8() }))
    }
}

impl<const N: usize> fmt::Display for EscapeIterInner<N, AlwaysEscaped> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // SAFETY: `AlwaysEscaped` 标记保证 `self.data` 处于 `escape_seq`
        //         variant 且包含可打印 ASCII 字符；`self.alive` 也保证是
        //         `self.data` 的有效范围。
        f.write_str(unsafe { self.to_str_unchecked() })
    }
}

impl<const N: usize> fmt::Display for EscapeIterInner<N, MaybeEscaped> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(c) = self.to_char() {
            return f.write_char(c);
        }

        // SAFETY: 走到这里时，`self.data` 必须处于 `escape_seq` variant
        //         且包含可打印 ASCII 字符；`self.alive` 保证是 `self.data`
        //         的有效范围。
        f.write_str(unsafe { self.to_str_unchecked() })
    }
}

impl<const N: usize> fmt::Debug for EscapeIterInner<N, AlwaysEscaped> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("EscapeIterInner").field(&format_args!("'{}'", self)).finish()
    }
}

impl<const N: usize> fmt::Debug for EscapeIterInner<N, MaybeEscaped> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("EscapeIterInner").field(&format_args!("'{}'", self)).finish()
    }
}
