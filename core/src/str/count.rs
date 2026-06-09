//! 高效统计 UTF-8 编码字符串中 `char` 数量的代码。
//!
//! 从结构上看，UTF-8 把每个 `char` 编码为一个“起始”字节，
//! 后面跟随若干个（可能为 0 个）continuation byte。
//!
//! 起始字节有多种位模式，具体模式表示后面跟随多少 continuation byte；
//! continuation byte 的格式始终是 `0b10XX_XXXX`（`X` 可为任意值）。
//! 也就是说，最高位为 1，次高位为 0。
//!
//! 因此，在已经满足 UTF-8 有效性不变量的 `&str` 中，统计字符数等价于统计
//! “不是 continuation byte 的字节”数量。这个判定可以按机器字批量完成。
//!
//! 注意：“leading byte” 有时有歧义（例如也可能指切片的第一个字节），所以代码中通常使用
//! “non-continuation byte” 指代这类字节。

use core::intrinsics::unlikely;

const USIZE_SIZE: usize = size_of::<usize>();
const UNROLL_INNER: usize = 4;

#[inline]
pub(super) fn count_chars(s: &str) -> usize {
    if cfg!(feature = "optimize_for_size") || s.len() < USIZE_SIZE * UNROLL_INNER {
        // 对很短的字符串避免进入优化实现：此时差异不大，甚至可能更慢。
        // 这里的阈值没有复杂调参，只取了一个看起来合理的值。
        char_count_general_case(s.as_bytes())
    } else {
        do_count_chars(s)
    }
}

fn do_count_chars(s: &str) -> usize {
    // 为保证正确性，`CHUNK_SIZE` 必须满足：
    //
    // - 小于等于 255，否则 `counts` 中的字节计数会溢出。
    // - 是 `UNROLL_INNER` 的倍数，否则 `body.chunks(CHUNK_SIZE)` 循环内的 `break` 不再正确。
    //
    // 为保证性能，`CHUNK_SIZE` 应满足：
    // - 除法相对便宜（例如若干 2 的幂之和）。
    // - 足够大，避免过于频繁地支付 `sum_bytes_in_usize` 的成本。
    const CHUNK_SIZE: usize = 192;

    // 检查 `CHUNK_SIZE` 和 `UNROLL_INNER` 为正确性所需的性质。
    const _: () = assert!(CHUNK_SIZE < 256);
    const _: () = assert!(CHUNK_SIZE.is_multiple_of(UNROLL_INNER));

    // SAFETY: `[u8]` 按 `usize` 重新分段的对齐和长度差异由 `align_to` 处理；
    // 返回的 head/body/tail 分别覆盖原切片，不改变底层字节。
    let (head, body, tail) = unsafe { s.as_bytes().align_to::<usize>() };

    // 这应当很少发生，主要用于处理 `align_to` 退化失败的情况，
    // 以及 miri 在符号对齐模式下的情形。
    //
    // `unlikely` 会降低 LLVM 内联该分支主体的倾向；这样无需把
    // `char_count_general_case` 整个函数标为 cold。
    if unlikely(body.is_empty() || head.len() > USIZE_SIZE || tail.len() > USIZE_SIZE) {
        return char_count_general_case(s.as_bytes());
    }

    let mut total = char_count_general_case(head) + char_count_general_case(tail);
    // 将 `body` 切为 `CHUNK_SIZE` 大小的块，以降低调用 `sum_bytes_in_usize` 的频率。
    for chunk in body.chunks(CHUNK_SIZE) {
        // 中间和累积在 `counts` 中；其中每个字节都保存该块计数的一部分，
        // 概念上类似 `[u8; size_of::<usize>()]`。
        let mut counts = 0;

        let (unrolled_chunks, remainder) = chunk.as_chunks::<UNROLL_INNER>();
        for unrolled in unrolled_chunks {
            for &word in unrolled {
                // 因为 `CHUNK_SIZE < 256`，该加法不会让任一字节中的计数溢出到后续字节。
                counts += contains_non_continuation_byte(word);
            }
        }

        // 对 `counts` 中各字节的值求和（它概念上仍是 `[u8; size_of::<usize>()]`），
        // 并累加到 `total`。
        total += sum_bytes_in_usize(counts);

        // 如果 `remainder` 中还有数据，就处理它。由于 `CHUNK_SIZE` 可被 `UNROLL_INNER` 整除，
        // 这只会发生在 `body.chunks()` 的最后一个块中，所以末尾显式 `break`，这似乎有助于 LLVM。
        if !remainder.is_empty() {
            // 累加 remainder 中的全部数据。
            let mut counts = 0;
            for &word in remainder {
                counts += contains_non_continuation_byte(word);
            }
            total += sum_bytes_in_usize(counts);
            break;
        }
    }
    total
}

// 检查 `w` 的每个字节是否是 UTF-8 序列中的首字节。
// continuation byte 会留下 `0x00`（相当于 false），non-continuation byte
// 会留下 `0x01`（相当于 true）。
#[inline]
fn contains_non_continuation_byte(w: usize) -> usize {
    const LSB: usize = usize::repeat_u8(0x01);
    ((!w >> 7) | (w >> 6)) & LSB
}

// 语义上等价于 `values.to_ne_bytes().into_iter().sum::<usize>()`，但效率更高。
#[inline]
fn sum_bytes_in_usize(values: usize) -> usize {
    const LSB_SHORTS: usize = usize::repeat_u16(0x0001);
    const SKIP_BYTES: usize = usize::repeat_u16(0x00ff);

    let pair_sum: usize = (values & SKIP_BYTES) + ((values >> 8) & SKIP_BYTES);
    pair_sum.wrapping_mul(LSB_SHORTS) >> ((USIZE_SIZE - 2) * 8)
}

// 这是“统计字符串中不是 continuation byte 的字节数量”这一概念最直接的实现，
// 用于输入字符串的 head 和 tail（即 `slice::align_to` 返回元组中的第一项和最后一项）。
fn char_count_general_case(s: &[u8]) -> usize {
    s.iter().filter(|&&byte| !super::validations::utf8_is_cont_byte(byte)).count()
}
