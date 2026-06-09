#![cfg_attr(any(feature = "optimize_for_size", target_pointer_width = "16"), allow(dead_code))]

use crate::marker::Freeze;

pub(crate) mod pivot;
pub(crate) mod smallsort;

/// SAFETY: 这是安全相关标记；它与 specialization 中已知的健全性漏洞如何交互？
#[rustc_unsafe_specialization_marker]
pub(crate) trait FreezeMarker {}

impl<T: Freeze> FreezeMarker for T {}

/// 查找从切片开头开始的一段已经有序的 run。
///
/// 返回 run 的长度，以及一个布尔值：`false` 表示 run 为升序，`true`
/// 表示 run 为严格降序。
#[inline(always)]
pub(crate) fn find_existing_run<T, F: FnMut(&T, &T) -> bool>(
    v: &[T],
    is_less: &mut F,
) -> (usize, bool) {
    let len = v.len();
    if len < 2 {
        return (len, false);
    }

    // SAFETY: 已检查 len >= 2，因此 0 和 1 是有效索引。
    // 这也意味着当 run_len < len 时，run_len 和 run_len - 1
    // 都是有效索引。
    unsafe {
        let mut run_len = 2;
        let strictly_descending = is_less(v.get_unchecked(1), v.get_unchecked(0));
        if strictly_descending {
            while run_len < len && is_less(v.get_unchecked(run_len), v.get_unchecked(run_len - 1)) {
                run_len += 1;
            }
        } else {
            while run_len < len && !is_less(v.get_unchecked(run_len), v.get_unchecked(run_len - 1))
            {
                run_len += 1;
            }
        }
        (run_len, strictly_descending)
    }
}
