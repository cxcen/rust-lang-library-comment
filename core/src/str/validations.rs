//! 与 UTF-8 验证相关的内部操作。
//!
//! `str` 的所有安全 API 都依赖“底层字节是合法 UTF-8”这一不变量。这里的验证逻辑负责在从
//! `[u8]` 进入 `&str` 前拒绝越界 code point、代理项、过短编码和缺失 continuation byte。

use super::Utf8Error;
use crate::intrinsics::const_eval_select;

/// 根据首字节返回 code point 累加器的初始值。
///
/// 首字节携带了编码宽度标签，因此只保留真正属于 code point 的低位：
/// 2 字节序列保留低 5 位，3 字节序列保留低 4 位，4 字节序列保留低 3 位。
#[inline]
const fn utf8_first_byte(byte: u8, width: u32) -> u32 {
    (byte & (0x7F >> width)) as u32
}

/// 将 continuation byte `byte` 合并进当前累加值 `ch` 后返回新值。
#[inline]
const fn utf8_acc_cont_byte(ch: u32, byte: u8) -> u32 {
    (ch << 6) | (byte & CONT_MASK) as u32
}

/// 检查字节是否是 UTF-8 continuation byte，也就是是否以位模式 `10` 开头。
#[inline]
pub(super) const fn utf8_is_cont_byte(byte: u8) -> bool {
    (byte as i8) < -64
}

/// 从字节迭代器中读取下一个 code point，假定输入使用 UTF-8-like 编码。
///
/// # 安全性(Safety）
///
/// `bytes` 必须产生合法 UTF-8-like 字符串（UTF-8 或 WTF-8）。也就是说，
/// 多字节序列长度和 continuation byte 必须完整有效；调用方还要保证迭代器不会在一个
/// code point 中途结束。
#[unstable(feature = "str_internals", issue = "none")]
#[inline]
pub unsafe fn next_code_point<'a, I: Iterator<Item = &'a u8>>(bytes: &mut I) -> Option<u32> {
    // 解码 UTF-8。
    let x = *bytes.next()?;
    if x < 128 {
        return Some(x as u32);
    }

    // 后续处理多字节情形。
    // 按 [[[x y] z] w] 的组合顺序解码。
    // NOTE: 这里的具体写法对性能敏感。
    let init = utf8_first_byte(x, 2);
    // SAFETY: `bytes` 产生 UTF-8-like 字符串；首字节已表明这是多字节序列，
    // 因而迭代器这里必须还能产生下一个字节。
    let y = unsafe { *bytes.next().unwrap_unchecked() };
    let mut ch = utf8_acc_cont_byte(init, y);
    if x >= 0xE0 {
        // [[x y z] w] 情形。
        // 0xE0 .. 0xEF 中第 5 位始终为 0，因此 `init` 仍然有效。
        // SAFETY: `bytes` 产生 UTF-8-like 字符串；三字节/四字节序列在这里必须还有字节。
        let z = unsafe { *bytes.next().unwrap_unchecked() };
        let y_z = utf8_acc_cont_byte((y & CONT_MASK) as u32, z);
        ch = init << 12 | y_z;
        if x >= 0xF0 {
            // [x y z w] 情形。
            // 只使用 `init` 的低 3 位。
            // SAFETY: `bytes` 产生 UTF-8-like 字符串；四字节序列在这里必须还有第 4 个字节。
            let w = unsafe { *bytes.next().unwrap_unchecked() };
            ch = (init & 7) << 18 | utf8_acc_cont_byte(y_z, w);
        }
    }

    Some(ch)
}

/// 从双端字节迭代器尾部读取最后一个 code point，假定输入使用 UTF-8-like 编码。
///
/// # 安全性(Safety）
///
/// `bytes` 必须产生合法 UTF-8-like 字符串（UTF-8 或 WTF-8）。从尾部读取时，
/// 调用方同样要保证不会把一个 code point 的字节序列截断。
#[inline]
pub(super) unsafe fn next_code_point_reverse<'a, I>(bytes: &mut I) -> Option<u32>
where
    I: DoubleEndedIterator<Item = &'a u8>,
{
    // 解码 UTF-8。
    let w = match *bytes.next_back()? {
        next_byte if next_byte < 128 => return Some(next_byte as u32),
        back_byte => back_byte,
    };

    // 后续处理多字节情形。
    // 按 [x [y [z w]]] 的组合顺序反向解码。
    let mut ch;
    // SAFETY: `bytes` 产生 UTF-8-like 字符串；尾字节表明这是多字节序列，前面必须还有字节。
    let z = unsafe { *bytes.next_back().unwrap_unchecked() };
    ch = utf8_first_byte(z, 2);
    if utf8_is_cont_byte(z) {
        // SAFETY: `bytes` 产生 UTF-8-like 字符串；仍处于 continuation byte 链中，前面必须还有字节。
        let y = unsafe { *bytes.next_back().unwrap_unchecked() };
        ch = utf8_first_byte(y, 3);
        if utf8_is_cont_byte(y) {
            // SAFETY: `bytes` 产生 UTF-8-like 字符串；四字节序列的首字节必须存在。
            let x = unsafe { *bytes.next_back().unwrap_unchecked() };
            ch = utf8_first_byte(x, 4);
            ch = utf8_acc_cont_byte(ch, y);
        }
        ch = utf8_acc_cont_byte(ch, z);
    }
    ch = utf8_acc_cont_byte(ch, w);

    Some(ch)
}

const NONASCII_MASK: usize = usize::repeat_u8(0x80);

/// 如果机器字 `x` 中任一字节是非 ASCII（>= 128），则返回 `true`。
#[inline]
const fn contains_nonascii(x: usize) -> bool {
    (x & NONASCII_MASK) != 0
}

/// 遍历 `v` 并检查它是否是合法 UTF-8 字节序列。
///
/// 有效时返回 `Ok(())`；无效时返回 `Err(err)`，其中包含已验证的有效前缀位置和错误长度。
#[inline(always)]
#[rustc_allow_const_fn_unstable(const_eval_select)] // fallback 实现具有相同行为。
pub(super) const fn run_utf8_validation(v: &[u8]) -> Result<(), Utf8Error> {
    let mut index = 0;
    let len = v.len();

    const USIZE_BYTES: usize = size_of::<usize>();

    let ascii_block_size = 2 * USIZE_BYTES;
    let blocks_end = if len >= ascii_block_size { len - ascii_block_size + 1 } else { 0 };
    // 如果偏移量是 `usize::MAX`，下面会安全退回较慢路径；
    // 因而编译期和运行期的端到端行为保持一致。
    let align = const_eval_select!(
        @capture { v: &[u8] } -> usize:
        if const {
            usize::MAX
        } else {
            v.as_ptr().align_offset(USIZE_BYTES)
        }
    );

    while index < len {
        let old_offset = index;
        macro_rules! err {
            ($error_len: expr) => {
                return Err(Utf8Error { valid_up_to: old_offset, error_len: $error_len })
            };
        }

        macro_rules! next {
            () => {{
                index += 1;
                // 需要继续读取字节，但输入已结束：这是错误。
                if index >= len {
                    err!(None)
                }
                v[index]
            }};
        }

        let first = v[index];
        if first >= 128 {
            let w = utf8_char_width(first);
            // 2 字节编码覆盖 code point  \u{0080} 到  \u{07ff}
            //        首个合法字节 C2 80        最后合法字节 DF BF
            // 3 字节编码覆盖 code point  \u{0800} 到  \u{ffff}
            //        首个合法字节 E0 A0 80     最后合法字节 EF BF BF
            //   但排除代理项 code point  \u{d800} 到  \u{dfff}
            //               ED A0 80 到       ED BF BF
            // 4 字节编码覆盖 code point \u{10000} 到 \u{10ffff}
            //        首个合法字节 F0 90 80 80  最后合法字节 F4 8F BF BF
            //
            // 使用 RFC 中的 UTF-8 语法：
            //
            // https://tools.ietf.org/html/rfc3629
            // UTF8-1      = %x00-7F
            // UTF8-2      = %xC2-DF UTF8-tail
            // UTF8-3      = %xE0 %xA0-BF UTF8-tail / %xE1-EC 2( UTF8-tail ) /
            //               %xED %x80-9F UTF8-tail / %xEE-EF 2( UTF8-tail )
            // UTF8-4      = %xF0 %x90-BF 2( UTF8-tail ) / %xF1-F3 3( UTF8-tail ) /
            //               %xF4 %x80-8F 2( UTF8-tail )
            match w {
                2 => {
                    if next!() as i8 >= -64 {
                        err!(Some(1))
                    }
                }
                3 => {
                    match (first, next!()) {
                        (0xE0, 0xA0..=0xBF)
                        | (0xE1..=0xEC, 0x80..=0xBF)
                        | (0xED, 0x80..=0x9F)
                        | (0xEE..=0xEF, 0x80..=0xBF) => {}
                        _ => err!(Some(1)),
                    }
                    if next!() as i8 >= -64 {
                        err!(Some(2))
                    }
                }
                4 => {
                    match (first, next!()) {
                        (0xF0, 0x90..=0xBF) | (0xF1..=0xF3, 0x80..=0xBF) | (0xF4, 0x80..=0x8F) => {}
                        _ => err!(Some(1)),
                    }
                    if next!() as i8 >= -64 {
                        err!(Some(2))
                    }
                    if next!() as i8 >= -64 {
                        err!(Some(3))
                    }
                }
                _ => err!(Some(1)),
            }
            index += 1;
        } else {
            // ASCII 情形：尝试快速向前跳过。
            // 当指针对齐时，每次迭代读取 2 个机器字，直到发现包含非 ASCII 字节的机器字。
            if align != usize::MAX && align.wrapping_sub(index).is_multiple_of(USIZE_BYTES) {
                let ptr = v.as_ptr();
                while index < blocks_end {
                    // SAFETY: `align - index` 和 `ascii_block_size` 都是 `USIZE_BYTES` 的倍数，
                    // 因此 `block = ptr.add(index)` 始终按 `usize` 对齐；
                    // 同时循环边界保证可读取两个机器字，所以解引用 `block` 和 `block.add(1)` 是安全的。
                    unsafe {
                        let block = ptr.add(index) as *const usize;
                        // 发现非 ASCII 字节时退出快速路径。
                        let zu = contains_nonascii(*block);
                        let zv = contains_nonascii(*block.add(1));
                        if zu || zv {
                            break;
                        }
                    }
                    index += ascii_block_size;
                }
                // 从按机器字扫描停止的位置继续逐字节前进。
                while index < len && v[index] < 128 {
                    index += 1;
                }
            } else {
                index += 1;
            }
        }
    }

    Ok(())
}

// https://tools.ietf.org/html/rfc3629
const UTF8_CHAR_WIDTH: &[u8; 256] = &[
    // 1  2  3  4  5  6  7  8  9  A  B  C  D  E  F
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, // 0
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, // 1
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, // 2
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, // 3
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, // 4
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, // 5
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, // 6
    1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, // 7
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // 8
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // 9
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // A
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // B
    0, 0, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, // C
    2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, // D
    3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, // E
    4, 4, 4, 4, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, // F
];

/// 给定首字节，判断该 UTF-8 字符占用多少字节。
#[unstable(feature = "str_internals", issue = "none")]
#[must_use]
#[inline]
pub const fn utf8_char_width(b: u8) -> usize {
    UTF8_CHAR_WIDTH[b as usize] as usize
}

/// continuation byte 中数值位的掩码。
const CONT_MASK: u8 = 0b0011_1111;
