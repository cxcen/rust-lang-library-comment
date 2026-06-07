//! 为二进制体积优化的 mergesort，灵感来自 https://github.com/voultapher/tiny-sort-rs。

use crate::mem::MaybeUninit;
use crate::ptr;
use crate::slice::sort::stable::merge;

/// 为二进制体积优化的小型递归自顶向下 merge sort。
/// 它完全没有自适应性，也不做 run 检测等优化。
#[inline(always)]
pub fn mergesort<T, F: FnMut(&T, &T) -> bool>(
    v: &mut [T],
    scratch: &mut [MaybeUninit<T>],
    is_less: &mut F,
) {
    let len = v.len();

    if len > 2 {
        let mid = len / 2;

        // SAFETY: mid 位于边界内。
        unsafe {
            // 递归排序左半部分。
            mergesort(v.get_unchecked_mut(..mid), scratch, is_less);
            // 递归排序右半部分。
            mergesort(v.get_unchecked_mut(mid..), scratch, is_less);
        }

        merge::merge(v, scratch, mid, is_less);
    } else if len == 2 {
        // SAFETY: 已检查 len；创建的指针有效且互不重叠。
        unsafe {
            let v_base = v.as_mut_ptr();
            let v_a = v_base;
            let v_b = v_base.add(1);

            if is_less(&*v_b, &*v_a) {
                ptr::swap_nonoverlapping(v_a, v_b, 1);
            }
        }
    }
}
