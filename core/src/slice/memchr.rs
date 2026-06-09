// 原始实现取自 rust-memchr。
// Copyright 2015 Andrew Gallant, bluss and Nicolas Koch

use crate::intrinsics::const_eval_select;

const LO_USIZE: usize = usize::repeat_u8(0x01);
const HI_USIZE: usize = usize::repeat_u8(0x80);
const USIZE_BYTES: usize = size_of::<usize>();

/// 如果 `x` 包含任意零字节，返回 `true`。
///
/// 来自 J. Arndt 的 *Matters Computational*：
///
/// “思路是从每个字节减一，然后寻找借位一路传播到最高有效位的字节。”
#[inline]
const fn contains_zero_byte(x: usize) -> bool {
    x.wrapping_sub(LO_USIZE) & !x & HI_USIZE != 0
}

/// 返回 `text` 中第一个等于字节 `x` 的索引。
#[inline]
#[must_use]
pub const fn memchr(x: u8, text: &[u8]) -> Option<usize> {
    // 小切片 fast path。
    if text.len() < 2 * USIZE_BYTES {
        return memchr_naive(x, text);
    }

    memchr_aligned(x, text)
}

#[inline]
const fn memchr_naive(x: u8, text: &[u8]) -> Option<usize> {
    let mut i = 0;

    // FIXME(const-hack): 替换成 `text.iter().pos(|c| *c == x)`。
    while i < text.len() {
        if text[i] == x {
            return Some(i);
        }

        i += 1;
    }

    None
}

#[rustc_allow_const_fn_unstable(const_eval_select)] // fallback 实现具有相同行为
const fn memchr_aligned(x: u8, text: &[u8]) -> Option<usize> {
    // 运行时版本与编译期版本行为相同，只是优化更多。
    const_eval_select!(
        @capture { x: u8, text: &[u8] } -> Option<usize>:
        if const {
            memchr_naive(x, text)
        } else {
            // 每次读取两个 `usize` 字来扫描单个字节值。
            //
            // 把 `text` 分成三段：
            // - 初始未对齐部分，即第一个按 word 对齐地址之前的内容；
            // - 主体部分，每次扫描 2 个 word；
            // - 最后剩余部分，长度小于 2 个 word。

            // 搜索到对齐边界。
            let len = text.len();
            let ptr = text.as_ptr();
            let mut offset = ptr.align_offset(USIZE_BYTES);

            if offset > 0 {
                offset = offset.min(len);
                let slice = &text[..offset];
                if let Some(index) = memchr_naive(x, slice) {
                    return Some(index);
                }
            }

            // 搜索 text 主体。
            let repeated_x = usize::repeat_u8(x);
            while offset <= len - 2 * USIZE_BYTES {
                // SAFETY: while 条件保证 offset 与切片末尾之间至少有 2 * usize_bytes 的距离。
                unsafe {
                    let u = *(ptr.add(offset) as *const usize);
                    let v = *(ptr.add(offset + USIZE_BYTES) as *const usize);

                    // 如果存在匹配字节则跳出。
                    let zu = contains_zero_byte(u ^ repeated_x);
                    let zv = contains_zero_byte(v ^ repeated_x);
                    if zu || zv {
                        break;
                    }
                }
                offset += USIZE_BYTES * 2;
            }

            // 在主体循环停止位置之后查找字节。
            // FIXME(const-hack): 改用 `?`。
            // FIXME(const-hack, fee1-dead): 使用范围切片。
            let slice =
            // SAFETY: offset 位于边界内。
                unsafe { super::from_raw_parts(text.as_ptr().add(offset), text.len() - offset) };
            if let Some(i) = memchr_naive(x, slice) { Some(offset + i) } else { None }
        }
    )
}

/// 返回 `text` 中最后一个等于字节 `x` 的索引。
#[must_use]
pub fn memrchr(x: u8, text: &[u8]) -> Option<usize> {
    // 每次读取两个 `usize` 字来扫描单个字节值。
    //
    // 把 `text` 分成三段：
    // - 尾部未对齐部分，即最后一个按 word 对齐地址之后的内容；
    // - 主体部分，每次扫描 2 个 word；
    // - 最前面剩余的字节，长度小于 2 个 word。
    let len = text.len();
    let ptr = text.as_ptr();
    type Chunk = usize;

    let (min_aligned_offset, max_aligned_offset) = {
        // 调用它只是为了获得前缀和后缀长度；中间部分始终一次处理两个 chunk。
        // SAFETY: 将 `[u8]` 转换为 `[usize]` 时，除大小差异外是安全的；大小差异由
        // `align_to` 处理。
        let (prefix, _, suffix) = unsafe { text.align_to::<(Chunk, Chunk)>() };
        (prefix.len(), len - suffix.len())
    };

    let mut offset = max_aligned_offset;
    if let Some(index) = text[offset..].iter().rposition(|elt| *elt == x) {
        return Some(offset + index);
    }

    // 搜索 text 主体，确保不越过 min_aligned_offset。offset 始终对齐，因此只测试 `>`
    // 就足够，并可避免潜在溢出。
    let repeated_x = usize::repeat_u8(x);
    let chunk_bytes = size_of::<Chunk>();

    while offset > min_aligned_offset {
        // SAFETY: offset 初始为 len - suffix.len()；只要它大于 min_aligned_offset
        // (prefix.len())，剩余距离就至少是 2 * chunk_bytes。
        unsafe {
            let u = *(ptr.add(offset - 2 * chunk_bytes) as *const Chunk);
            let v = *(ptr.add(offset - chunk_bytes) as *const Chunk);

            // 如果存在匹配字节则跳出。
            let zu = contains_zero_byte(u ^ repeated_x);
            let zv = contains_zero_byte(v ^ repeated_x);
            if zu || zv {
                break;
            }
        }
        offset -= 2 * chunk_bytes;
    }

    // 在主体循环停止位置之前查找字节。
    text[..offset].iter().rposition(|elt| *elt == x)
}
