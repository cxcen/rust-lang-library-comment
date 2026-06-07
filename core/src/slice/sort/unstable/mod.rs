//! 本模块包含 `slice::sort_unstable` 的内部入口。

use crate::mem::SizedTypeProperties;
use crate::ops::{Range, RangeBounds};
use crate::slice::sort::select::partition_at_index;
#[cfg(not(any(feature = "optimize_for_size", target_pointer_width = "16")))]
use crate::slice::sort::shared::find_existing_run;
#[cfg(not(any(feature = "optimize_for_size", target_pointer_width = "16")))]
use crate::slice::sort::shared::smallsort::insertion_sort_shift_left;
use crate::{cfg_select, intrinsics, slice};

pub(crate) mod heapsort;
pub(crate) mod quicksort;

/// Lukas Bergdoll 和 Orson Peters 称为 ipnsort 的不稳定排序。
/// 设计文档：
/// <https://github.com/Voultapher/sort-research-rs/blob/main/writeup/ipnsort_introduction/text.md>
///
/// 维护下列文档中列出的全部安全性质：
/// <https://github.com/Voultapher/sort-research-rs/blob/main/writeup/sort_safety/text.md>
#[inline(always)]
pub fn sort<T, F>(v: &mut [T], is_less: &mut F)
where
    F: FnMut(&T, &T) -> bool,
{
    // zero-sized type 数组中的元素总是全相等，因此已排序。
    if T::IS_ZST {
        return;
    }

    // 对标准库插桩显示，rustc 调用 sort 时 90% 以上输入长度为 0 或 1。
    let len = v.len();
    if intrinsics::likely(len < 2) {
        return;
    }

    cfg_select! {
        any(feature = "optimize_for_size", target_pointer_width = "16") => {
            heapsort::heapsort(v, is_less);
        }
        _ => {
            // 对小输入而言，比插入排序更复杂的算法如果在热循环中调用会更快；但对通用代码，
            // 插入排序较小的二进制体积更重要。现代处理器的指令缓存很宝贵，通用代码中单次
            // sort 调用从复杂算法得到的收益，往往会被排序期间的 i-cache miss 以及对周边代码
            // 指令缓存的扰动抵消。
            const MAX_LEN_ALWAYS_INSERTION_SORT: usize = 20;
            if intrinsics::likely(len <= MAX_LEN_ALWAYS_INSERTION_SORT) {
                insertion_sort_shift_left(v, 1, is_less);
                return;
            }

            ipnsort(v, is_less);
        }
    }
}

/// 对范围 `start..end` 做不稳定部分排序，完成后保证：
///
/// 1. `v[..start]` 中的每个元素都小于或等于
/// 2. 已排序的 `v[start..end]` 中每个元素，并且这些元素又小于或等于
/// 3. `v[end..]` 中的每个元素。
#[inline]
pub fn partial_sort<T, F, R>(v: &mut [T], range: R, mut is_less: F)
where
    F: FnMut(&T, &T) -> bool,
    R: RangeBounds<usize>,
{
    // zero-sized type 数组中的元素总是全相等，因此已排序。
    if T::IS_ZST {
        return;
    }

    let len = v.len();
    let Range { start, end } = slice::range(range, ..len);

    if end - start <= 1 {
        // 空范围或单元素范围最多只需要一次 partition_at_index 调用，无需继续排序。

        if end == 0 || start == len {
            // 如果是开头或末尾的空范围，什么都不用做：所有保证已经成立。
            return;
        }

        partition_at_index(v, start, &mut is_less);
        return;
    }

    // 用于决定是否先分区的启发式因子。如果范围边界靠近切片边缘，先分区不值得。
    const PARTITION_THRESHOLD: usize = 8;
    let mut v = v;
    if end + PARTITION_THRESHOLD <= len {
        v = partition_at_index(v, end - 1, &mut is_less).0;
    }
    if start >= PARTITION_THRESHOLD {
        v = partition_at_index(v, start, &mut is_less).2;
    }

    sort(v, &mut is_less);
}

/// 见 [`sort`]。
///
/// 故意不内联主排序例程入口，以确保内联插入排序的 i-cache 占用保持最小。
#[cfg(not(any(feature = "optimize_for_size", target_pointer_width = "16")))]
#[inline(never)]
fn ipnsort<T, F>(v: &mut [T], is_less: &mut F)
where
    F: FnMut(&T, &T) -> bool,
{
    let len = v.len();
    let (run_len, was_reversed) = find_existing_run(v, is_less);

    // SAFETY: find_existing_run 承诺返回有效的 run_len。
    unsafe { intrinsics::assume(run_len <= len) };

    if run_len == len {
        if was_reversed {
            v.reverse();
        }

        // 这里可以对较长的已有有序段做原地合并，但会显著增大实现；这种用例用户可使用
        // `slice::sort`。
        return;
    }

    // 将不平衡分区次数限制为 `2 * floor(log2(len))`。
    // 与 1 做按位或用于消除对数计算中的零检查。
    let limit = 2 * (len | 1).ilog2();
    crate::slice::sort::unstable::quicksort::quicksort(v, None, limit, is_less);
}
