//! [WTF-8 encoding](https://simonsapin.github.io/wtf-8/) 的核心实现。
//!
//! 本模块借助 Rust 的类型系统维护
//! [well-formedness](https://simonsapin.github.io/wtf-8/#well-formed)
//! 不变量，作用类似 `String` 和 `&str` 对 UTF-8 有效性的维护：
//! 一旦构造出 `Wtf8`，后续安全接口就只能观察或派生仍然满足 WTF-8 规则的数据。
//!
//! 由于 [WTF-8 不能作为通用交换格式使用](https://simonsapin.github.io/wtf-8/#intended-audience)，
//! 这里故意不提供从任意字节解码 WTF-8 的公开入口，也不让外部随意把内部字节当作协议格式传播。
//! WTF-8 的主要用途是桥接 OS 字符串：它能无损承载 UTF-8、UTF-16
//! 以及单独的 Unicode code point，特别是某些平台 OS 字符串中可能出现的未配对代理项。
#![unstable(
    feature = "wtf8_internals",
    issue = "none",
    reason = "this is internal code for representing OsStr on some platforms and not a public API"
)]
// rustdoc 的限制：模块上的 doc(hidden) 不能阻止模块内类型出现在 trait 实现页面中，
// 因而这些内部类型仍需要分别标注额外的 doc(hidden)。
#![doc(hidden)]

use crate::char::{EscapeDebugExtArgs, encode_utf16_raw};
use crate::clone::CloneToUninit;
use crate::fmt::{self, Write};
use crate::hash::{Hash, Hasher};
use crate::iter::FusedIterator;
use crate::num::niche_types::CodePointInner;
use crate::str::next_code_point;
use crate::{ops, slice, str};

/// 一个 Unicode code point，取值范围为 U+0000 到 U+10FFFF。
///
/// 它比 `char` 覆盖的集合更大：`char` 表示 Unicode 标量值，
/// 即不包含代理项的 code point；而 `CodePoint` 允许 U+D800 到 U+DFFF
/// 这段代理项范围，用于 WTF-8 与可能非良构 UTF-16 的无损桥接。
#[derive(Eq, PartialEq, Ord, PartialOrd, Clone, Copy)]
#[doc(hidden)]
pub struct CodePoint(CodePointInner);

/// 将 code point 格式化为 `U+` 后接四到六位十六进制数字。
/// 例如：`U+1F4A9`。
impl fmt::Debug for CodePoint {
    #[inline]
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "U+{:04X}", self.0.as_inner())
    }
}

impl CodePoint {
    /// 不检查数值范围，直接构造新的 `CodePoint`。
    ///
    /// 调用方必须已经知道 `value <= 0x10FFFF`。超过该范围的值不是 Unicode code point，
    /// 会破坏 `CodePointInner` 的 niche 不变量，并让后续编码、比较或转换逻辑在错误前提下运行。
    #[inline]
    pub unsafe fn from_u32_unchecked(value: u32) -> CodePoint {
        // SAFETY: 调用方已经承诺 `value` 不超过 0x10FFFF，满足 `CodePointInner` 的有效取值范围。
        CodePoint(unsafe { CodePointInner::new_unchecked(value) })
    }

    /// 当数值是合法 Unicode code point 时创建新的 `CodePoint`。
    ///
    /// 如果 `value` 大于 0x10FFFF，则返回 `None`；这类数值不属于 Unicode 编码空间。
    #[inline]
    pub fn from_u32(value: u32) -> Option<CodePoint> {
        Some(CodePoint(CodePointInner::new(value)?))
    }

    /// 从 `char` 创建新的 `CodePoint`。
    ///
    /// 所有 Unicode 标量值都是 code point，且 `char` 已经排除了代理项和越界值，
    /// 因此该转换总能成功。
    #[inline]
    pub fn from_char(value: char) -> CodePoint {
        // SAFETY: 每个 `char` 都是合法 Unicode 标量值，也就必然是 `CodePoint` 可表示的 code point。
        unsafe { CodePoint::from_u32_unchecked(value as u32) }
    }

    /// 返回该 code point 的数值表示。
    #[inline]
    pub fn to_u32(&self) -> u32 {
        self.0.as_inner()
    }

    /// 如果该 code point 是前导代理项，则返回它的数值表示。
    #[inline]
    pub fn to_lead_surrogate(&self) -> Option<u16> {
        match self.to_u32() {
            lead @ 0xD800..=0xDBFF => Some(lead as u16),
            _ => None,
        }
    }

    /// 如果该 code point 是尾随代理项，则返回它的数值表示。
    #[inline]
    pub fn to_trail_surrogate(&self) -> Option<u16> {
        match self.to_u32() {
            trail @ 0xDC00..=0xDFFF => Some(trail as u16),
            _ => None,
        }
    }

    /// 尝试把该 code point 转成 Unicode 标量值。
    ///
    /// 如果 code point 位于 U+D800 到 U+DFFF 的代理项范围内，则返回 `None`；
    /// 代理项不是 Unicode 标量值，因而不能由 `char` 表示。
    #[inline]
    pub fn to_char(&self) -> Option<char> {
        match self.to_u32() {
            0xD800..=0xDFFF => None,
            // SAFETY: 上面的分支已经排除了代理项，且 `CodePoint` 自身保证不超过 U+10FFFF。
            valid => Some(unsafe { char::from_u32_unchecked(valid) }),
        }
    }

    /// 返回该 code point 对应的 Unicode 标量值。
    ///
    /// 如果 code point 是 U+D800 到 U+DFFF 范围内的代理项，则返回
    /// `'\u{FFFD}'`（替换字符），用于有损地显示无法由 `char` 表示的值。
    #[inline]
    pub fn to_char_lossy(&self) -> char {
        self.to_char().unwrap_or(char::REPLACEMENT_CHARACTER)
    }
}

/// 一个借用的、满足 WTF-8 well-formedness 规则的数据切片。
///
/// 它类似 `&str`，同样用类型维护编码有效性；区别在于 WTF-8 允许保留未配对代理项
/// code point，以便 OS 字符串和可能非良构 UTF-16 能够往返转换而不丢失信息。
#[derive(Eq, Ord, PartialEq, PartialOrd)]
#[repr(transparent)]
#[rustc_has_incoherent_inherent_impls]
#[doc(hidden)]
pub struct Wtf8 {
    bytes: [u8],
}

impl AsRef<[u8]> for Wtf8 {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        &self.bytes
    }
}

/// 以双引号包围的形式格式化字符串。
///
/// 普通 Unicode 标量值按照 [`char::escape_debug`] 的规则转义；
/// 未配对代理项无法由 `char` 表示，因此写成 `\u{xxxx}`，其中每个 `x` 都是十六进制数字。
impl fmt::Debug for Wtf8 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fn write_str_escaped(f: &mut fmt::Formatter<'_>, s: &str) -> fmt::Result {
            use crate::fmt::Write as _;
            for c in s.chars().flat_map(|c| {
                c.escape_debug_ext(EscapeDebugExtArgs {
                    escape_grapheme_extended: true,
                    escape_single_quote: false,
                    escape_double_quote: true,
                })
            }) {
                f.write_char(c)?
            }
            Ok(())
        }

        formatter.write_char('"')?;
        let mut pos = 0;
        while let Some((surrogate_pos, surrogate)) = self.next_surrogate(pos) {
            // SAFETY: `next_surrogate` 返回的 `surrogate_pos` 之前只包含合法 UTF-8 字节，
            // 因而该子切片满足 `from_utf8_unchecked` 的有效性前置条件。
            write_str_escaped(formatter, unsafe {
                str::from_utf8_unchecked(&self.bytes[pos..surrogate_pos])
            })?;
            write!(formatter, "\\u{{{:x}}}", surrogate)?;
            pos = surrogate_pos + 3;
        }

        // SAFETY: `next_surrogate` 返回 `None` 后，剩余部分不含代理项编码，仍是合法 UTF-8。
        write_str_escaped(formatter, unsafe { str::from_utf8_unchecked(&self.bytes[pos..]) })?;
        formatter.write_char('"')
    }
}

/// 格式化字符串，并把未配对代理项替换为 U+FFFD。
///
/// 这是面向显示的有损路径：它保留合法 UTF-8 片段，遇到 `char` 无法表示的代理项时使用替换字符。
impl fmt::Display for Wtf8 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let wtf8_bytes = &self.bytes;
        let mut pos = 0;
        loop {
            match self.next_surrogate(pos) {
                Some((surrogate_pos, _)) => {
                    // SAFETY: `next_surrogate` 给出的范围在当前代理项之前，只包含合法 UTF-8 字节。
                    formatter.write_str(unsafe {
                        str::from_utf8_unchecked(&wtf8_bytes[pos..surrogate_pos])
                    })?;
                    formatter.write_char(char::REPLACEMENT_CHARACTER)?;
                    pos = surrogate_pos + 3;
                }
                None => {
                    // SAFETY: 未再找到代理项时，剩余字节按 WTF-8 规则就是合法 UTF-8。
                    let s = unsafe { str::from_utf8_unchecked(&wtf8_bytes[pos..]) };
                    if pos == 0 { return s.fmt(formatter) } else { return formatter.write_str(s) }
                }
            }
        }
    }
}

impl Wtf8 {
    /// 从 UTF-8 `&str` 切片创建 WTF-8 切片。
    #[inline]
    pub fn from_str(value: &str) -> &Wtf8 {
        // SAFETY: WTF-8 是 UTF-8 的超集，任何已经由 `&str` 保证有效的 UTF-8 字节也满足 WTF-8。
        unsafe { Wtf8::from_bytes_unchecked(value.as_bytes()) }
    }

    /// 从 WTF-8 字节切片创建 WTF-8 切片。
    ///
    /// 字节切片不会在这里重新校验。调用方必须保证 `value` 已经满足 WTF-8 well-formedness：
    /// 普通片段必须是合法 UTF-8，代理项只能以 WTF-8 允许的形式出现，且不会制造公开 API 禁止的切分状态。
    #[inline]
    pub unsafe fn from_bytes_unchecked(value: &[u8]) -> &Wtf8 {
        // SAFETY: `Wtf8` 是对 `[u8]` 的透明包装；引用的地址、长度和生命周期保持不变，
        // 编码有效性由该 unsafe 函数的调用方负责维护。
        unsafe { &*(value as *const [u8] as *const Wtf8) }
    }

    /// 从可变 WTF-8 字节切片创建可变 WTF-8 切片。
    ///
    /// 字节切片不会在这里重新校验。调用方必须保证初始内容满足 WTF-8，并且通过返回的
    /// `&mut Wtf8` 暴露出去时仍维护该不变量。
    #[inline]
    pub unsafe fn from_mut_bytes_unchecked(value: &mut [u8]) -> &mut Wtf8 {
        // SAFETY: `Wtf8` 与 `[u8]` 具有相同布局；独占借用保持不变，编码不变量由调用方承诺。
        unsafe { &mut *(value as *mut [u8] as *mut Wtf8) }
    }

    /// 返回以 WTF-8 字节为单位的长度。
    #[inline]
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// 如果 `position` 处的 code point 位于 ASCII 范围内，则返回对应字节；
    /// 否则返回 `b'\xFF'` 作为非 ASCII 哨兵值。
    ///
    /// # Panics
    ///
    /// 当 `position` 超出字符串末尾时 panic。
    #[inline]
    pub fn ascii_byte_at(&self, position: usize) -> u8 {
        match self.bytes[position] {
            ascii_byte @ 0x00..=0x7F => ascii_byte,
            _ => 0xFF,
        }
    }

    /// 返回遍历该字符串 code point 的迭代器。
    #[inline]
    pub fn code_points(&self) -> Wtf8CodePoints<'_> {
        Wtf8CodePoints { bytes: self.bytes.iter() }
    }

    /// 访问 WTF-8 数据的原始字节。
    ///
    /// 该接口只暴露只读切片，调用方不能借此破坏 WTF-8 不变量；它主要供内部桥接和格式化逻辑使用。
    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// 尝试把该字符串转换为 UTF-8，并返回 `&str` 切片。
    ///
    /// 如果字符串包含代理项，则返回 `Err`，因为代理项不是 Unicode 标量值，不能出现在 `&str` 中。
    ///
    /// 成功时不会复制数据；返回的 `&str` 与原始 WTF-8 字节共享同一段存储。
    #[inline]
    pub fn as_str(&self) -> Result<&str, str::Utf8Error> {
        str::from_utf8(&self.bytes)
    }

    /// 把 WTF-8 字符串转换为可能非良构的 UTF-16，并返回 16 位 code unit 迭代器。
    ///
    /// 这是无损转换：对生成的 code unit 再调用
    /// `Wtf8Buf::from_ill_formed_utf16`，总能还原出原始 WTF-8 字符串。
    #[inline]
    pub fn encode_wide(&self) -> EncodeWide<'_> {
        EncodeWide { code_points: self.code_points(), extra: 0 }
    }

    #[inline]
    pub fn next_surrogate(&self, mut pos: usize) -> Option<(usize, u16)> {
        let mut iter = self.bytes[pos..].iter();
        loop {
            let b = *iter.next()?;
            if b < 0x80 {
                pos += 1;
            } else if b < 0xE0 {
                iter.next();
                pos += 2;
            } else if b == 0xED {
                match (iter.next(), iter.next()) {
                    (Some(&b2), Some(&b3)) if b2 >= 0xA0 => {
                        return Some((pos, decode_surrogate(b2, b3)));
                    }
                    _ => pos += 3,
                }
            } else if b < 0xF0 {
                iter.next();
                iter.next();
                pos += 3;
            } else {
                iter.next();
                iter.next();
                iter.next();
                pos += 4;
            }
        }
    }

    #[inline]
    pub fn final_lead_surrogate(&self) -> Option<u16> {
        match self.bytes {
            [.., 0xED, b2 @ 0xA0..=0xAF, b3] => Some(decode_surrogate(b2, b3)),
            _ => None,
        }
    }

    #[inline]
    pub fn initial_trail_surrogate(&self) -> Option<u16> {
        match self.bytes {
            [0xED, b2 @ 0xB0..=0xBF, b3, ..] => Some(decode_surrogate(b2, b3)),
            _ => None,
        }
    }

    #[inline]
    pub fn make_ascii_lowercase(&mut self) {
        self.bytes.make_ascii_lowercase()
    }

    #[inline]
    pub fn make_ascii_uppercase(&mut self) {
        self.bytes.make_ascii_uppercase()
    }

    #[inline]
    pub fn is_ascii(&self) -> bool {
        self.bytes.is_ascii()
    }

    #[inline]
    pub fn eq_ignore_ascii_case(&self, other: &Self) -> bool {
        self.bytes.eq_ignore_ascii_case(&other.bytes)
    }
}

/// 返回给定字符串在字节范围 \[`begin`..`end`) 内的 WTF-8 切片。
///
/// 与 `str` 一样，这里的索引是字节索引而不是字符序号；范围两端必须落在允许的
/// code point 边界上，否则会把一个 UTF-8/WTF-8 编码单元切开并破坏后续解码。
///
/// # Panics
///
/// 当 `begin` 或 `end` 没有指向 code point 边界，或者越过字符串末尾时 panic。
impl ops::Index<ops::Range<usize>> for Wtf8 {
    type Output = Wtf8;

    #[inline]
    fn index(&self, range: ops::Range<usize>) -> &Wtf8 {
        if range.start <= range.end
            && self.is_code_point_boundary(range.start)
            && self.is_code_point_boundary(range.end)
        {
            // SAFETY: `is_code_point_boundary` 已确认起止索引是有效边界，且上面的比较保证范围非负。
            unsafe { slice_unchecked(self, range.start, range.end) }
        } else {
            slice_error_fail(self, range.start, range.end)
        }
    }
}

/// 返回给定字符串从字节位置 `begin` 到末尾的 WTF-8 切片。
///
/// `begin` 必须是允许的 code point 边界；字节索引不能落在 UTF-8 continuation byte
/// 或公开 API 禁止切分的代理项组合中。
///
/// # Panics
///
/// 当 `begin` 不是 code point 边界，或者越过字符串末尾时 panic。
impl ops::Index<ops::RangeFrom<usize>> for Wtf8 {
    type Output = Wtf8;

    #[inline]
    fn index(&self, range: ops::RangeFrom<usize>) -> &Wtf8 {
        if self.is_code_point_boundary(range.start) {
            // SAFETY: `is_code_point_boundary` 已确认 `range.start` 可作为切片起点，终点使用当前长度。
            unsafe { slice_unchecked(self, range.start, self.len()) }
        } else {
            slice_error_fail(self, range.start, self.len())
        }
    }
}

/// 返回给定字符串从开头到字节位置 `end` 的 WTF-8 切片。
///
/// `end` 同样是字节边界要求：它必须位于完整 code point 之后，不能把多字节序列切开。
///
/// # Panics
///
/// 当 `end` 不是 code point 边界，或者越过字符串末尾时 panic。
impl ops::Index<ops::RangeTo<usize>> for Wtf8 {
    type Output = Wtf8;

    #[inline]
    fn index(&self, range: ops::RangeTo<usize>) -> &Wtf8 {
        if self.is_code_point_boundary(range.end) {
            // SAFETY: `is_code_point_boundary` 已确认 `range.end` 可作为切片终点，起点使用 0。
            unsafe { slice_unchecked(self, 0, range.end) }
        } else {
            slice_error_fail(self, 0, range.end)
        }
    }
}

impl ops::Index<ops::RangeFull> for Wtf8 {
    type Output = Wtf8;

    #[inline]
    fn index(&self, _range: ops::RangeFull) -> &Wtf8 {
        self
    }
}

#[inline]
fn decode_surrogate(second_byte: u8, third_byte: u8) -> u16 {
    // 这里的三字节序列首字节已由调用方判断为 0xED。
    0xD800 | (second_byte as u16 & 0x3F) << 6 | third_byte as u16 & 0x3F
}

impl Wtf8 {
    /// 逻辑来自 `str::is_char_boundary`。
    ///
    /// WTF-8 与 UTF-8 在非代理项 code point 的边界判定上相同：开头、末尾以及非 continuation
    /// byte 的位置都是候选边界。更严格的公开边界规则由 `check_utf8_boundary` 补充处理。
    #[inline]
    pub fn is_code_point_boundary(&self, index: usize) -> bool {
        if index == 0 {
            return true;
        }
        match self.bytes.get(index) {
            None => index == self.len(),
            Some(&b) => (b as i8) >= -0x40,
        }
    }

    /// 验证 `index` 位于合法 UTF-8 code point（也就是非代理项 code point）的边缘，
    /// 或者位于整个字符串的开头/末尾。
    ///
    /// 这些正是 `OsStr::self_encoded_bytes` 当前允许公开暴露的切分位置。单从 WTF-8
    /// 编码规则看，代理项之间切分可以是良构的；但 WTF-8 在标准库中只是 OS 字符串桥接的实现细节，
    /// 所以公开 API 不允许用户观察或依赖这种内部代理项边界。
    #[track_caller]
    #[inline]
    pub fn check_utf8_boundary(&self, index: usize) {
        if index == 0 {
            return;
        }
        match self.bytes.get(index) {
            Some(0xED) => (), // 可能是代理项编码的起始字节，需要继续检查。
            Some(&b) if (b as i8) >= -0x40 => return,
            Some(_) => panic!("byte index {index} is not a codepoint boundary"),
            None if index == self.len() => return,
            None => panic!("byte index {index} is out of bounds"),
        }
        if self.bytes[index + 1] >= 0xA0 {
            // `index` 后面有代理项；继续检查前面是否也紧邻代理项，避免公开切在代理项之间的位置。
            if index >= 3 && self.bytes[index - 3] == 0xED && self.bytes[index - 2] >= 0xA0 {
                panic!("byte index {index} lies between surrogate codepoints");
            }
        }
    }
}

/// 逻辑来自 `core::str::raw::slice_unchecked`。
#[inline]
unsafe fn slice_unchecked(s: &Wtf8, begin: usize, end: usize) -> &Wtf8 {
    // SAFETY: `Wtf8` 是 `[u8]` 的透明包装；调用方已保证 `begin..end` 位于有效边界内。
    // `from_raw_parts` 只重建同一分配内的子切片，随后按同布局转换回 `&Wtf8`。
    unsafe {
        let len = end - begin;
        let start = s.as_bytes().as_ptr().add(begin);
        Wtf8::from_bytes_unchecked(slice::from_raw_parts(start, len))
    }
}

/// 逻辑来自 `core::str::raw::slice_error_fail`。
#[inline(never)]
fn slice_error_fail(s: &Wtf8, begin: usize, end: usize) -> ! {
    assert!(begin <= end);
    panic!("index {begin} and/or {end} in `{s:?}` do not lie on character boundary");
}

/// WTF-8 字符串的 code point 迭代器。
///
/// 由 `.code_points()` 方法创建。它逐个解码 WTF-8 code point，可能产生代理项，
/// 因而迭代项使用 `CodePoint` 而不是 `char`。
#[derive(Clone)]
#[doc(hidden)]
pub struct Wtf8CodePoints<'a> {
    bytes: slice::Iter<'a, u8>,
}

impl Iterator for Wtf8CodePoints<'_> {
    type Item = CodePoint;

    #[inline]
    fn next(&mut self) -> Option<CodePoint> {
        // SAFETY: `self.bytes` 来源于满足 WTF-8 不变量的字符串；`next_code_point`
        // 读出的值必然位于 Unicode code point 范围内。
        unsafe { next_code_point(&mut self.bytes).map(|c| CodePoint::from_u32_unchecked(c)) }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.bytes.len();
        (len.saturating_add(3) / 4, Some(len))
    }
}

impl fmt::Debug for Wtf8CodePoints<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Wtf8CodePoints")
            // SAFETY: 迭代器每次只从前端消费完整 code point，剩余字节仍保持 WTF-8 有效状态。
            .field(&unsafe { Wtf8::from_bytes_unchecked(self.bytes.as_slice()) })
            .finish()
    }
}

/// 为可能非良构的 UTF-16 生成宽字符序列。
///
/// 该迭代器用于 OS 字符串桥接：合法标量值按 UTF-16 编码，WTF-8 中保留的代理项则作为原始
/// code unit 输出，从而保证往返转换不丢失信息。
#[stable(feature = "rust1", since = "1.0.0")]
#[derive(Clone)]
#[doc(hidden)]
pub struct EncodeWide<'a> {
    code_points: Wtf8CodePoints<'a>,
    extra: u16,
}

// 逻辑来自 libunicode/u_str.rs。
#[stable(feature = "rust1", since = "1.0.0")]
impl Iterator for EncodeWide<'_> {
    type Item = u16;

    #[inline]
    fn next(&mut self) -> Option<u16> {
        if self.extra != 0 {
            let tmp = self.extra;
            self.extra = 0;
            return Some(tmp);
        }

        let mut buf = [0; char::MAX_LEN_UTF16];
        self.code_points.next().map(|code_point| {
            let n = encode_utf16_raw(code_point.to_u32(), &mut buf).len();
            if n == 2 {
                self.extra = buf[1];
            }
            buf[0]
        })
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let (low, high) = self.code_points.size_hint();
        let ext = (self.extra != 0) as usize;
        // 每个 code point 会产生一个或两个 u16，因此该迭代器长度介于底层迭代器长度的
        // 1 倍到 2 倍之间；`extra` 保存代理对第二个 code unit 时还需额外计入。
        (low + ext, high.and_then(|n| n.checked_mul(2)).and_then(|n| n.checked_add(ext)))
    }
}

#[stable(feature = "encode_wide_fused_iterator", since = "1.62.0")]
impl FusedIterator for EncodeWide<'_> {}

#[stable(feature = "encode_wide_debug", since = "1.92.0")]
impl fmt::Debug for EncodeWide<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        struct CodeUnit(u16);
        impl fmt::Debug for CodeUnit {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                // 该输出在可读性和精确性之间折中：只需一个 WTF-16 code unit 的字符使用
                // `char` 语法显示，其余内容使用十六进制整数形式显示 code unit，
                // 包括成对和未配对的代理项半区。Rust 没有面向 WTF-16 的 `char` 等价类型，
                // 因而这种表示并不完美；如果后续不适合调试需求，可以调整（见 #140153）。
                match char::from_u32(self.0 as u32) {
                    Some(c) => write!(f, "{c:?}"),
                    None => write!(f, "0x{:04X}", self.0),
                }
            }
        }

        write!(f, "EncodeWide(")?;
        f.debug_list().entries(self.clone().map(CodeUnit)).finish()?;
        write!(f, ")")?;
        Ok(())
    }
}

impl Hash for CodePoint {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state)
    }
}

impl Hash for Wtf8 {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write(&self.bytes);
        0xfeu8.hash(state)
    }
}

#[unstable(feature = "clone_to_uninit", issue = "126799")]
unsafe impl CloneToUninit for Wtf8 {
    #[inline]
    #[cfg_attr(debug_assertions, track_caller)]
    unsafe fn clone_to_uninit(&self, dst: *mut u8) {
        // SAFETY: `Wtf8` 只是 `[u8]` 的透明包装，克隆未初始化目标时实际拷贝的是同一字节序列。
        unsafe { self.bytes.clone_to_uninit(dst) }
    }
}
