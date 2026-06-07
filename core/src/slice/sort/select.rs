//! 本模块包含 `slice::select_nth_unstable` 的实现。
//! 它使用基于 Lukas Bergdoll 和 Orson Peters 的 ipnsort 的 introselect 算法，
//! 发布位置：<https://github.com/Voultapher/sort-research-rs/tree/main/ipnsort>
//!
//! introselect 的 fallback 算法是 Median of Medians，并用 Tukey's Ninther 选择枢轴。
//! 把它作为 fallback 可确保 O(n) 最坏运行时间，同时性能优于用 heapsort 作为 fallback。

use crate::cfg_select;
use crate::mem::{self, SizedTypeProperties};
#[cfg(not(feature = "optimize_for_size"))]
use crate::slice::sort::shared::pivot::choose_pivot;
use crate::slice::sort::shared::smallsort::insertion_sort_shift_left;
use crate::slice::sort::unstable::quicksort::partition;

/// 重排切片，使 `index` 处元素位于最终排序位置。
pub(crate) fn partition_at_index<T, F>(
    v: &mut [T],
    index: usize,
    mut is_less: F,
) -> (&mut [T], &mut T, &mut [T])
where
    F: FnMut(&T, &T) -> bool,
{
    let len = v.len();

    // 给 `len` 设置下限 1；越界或空切片会在这里 panic。
    if index >= len {
        panic!("partition_at_index index {} greater than length of slice {}", index, len);
    }

    if T::IS_ZST {
        // zero-sized type 上排序没有有意义的行为，直接不做事。
    } else if index == len - 1 {
        // 找到最大元素并放到数组最后位置。前面已经检查 `v` 非空，因此可使用 `unwrap()`。
        let max_idx = max_index(v, &mut is_less).unwrap();
        v.swap(max_idx, index);
    } else if index == 0 {
        // 找到最小元素并放到数组第一位置。前面已经检查 `v` 非空，因此可使用 `unwrap()`。
        let min_idx = min_index(v, &mut is_less).unwrap();
        v.swap(min_idx, index);
    } else {
        cfg_select! {
            feature = "optimize_for_size" => {
                median_of_medians(v, &mut is_less, index);
            }
            _ => {
                partition_at_index_loop(v, index, None, &mut is_less);
            }
        }
    }

    let (left, right) = v.split_at_mut(index);
    let (pivot, right) = right.split_at_mut(1);
    let pivot = &mut pivot[0];
    (left, pivot, right)
}

// 对小子切片，专用 small-sort 更快；但这里最多只调用一次，使用比插入排序更复杂的算法没有意义。
const INSERTION_SORT_THRESHOLD: usize = 16;

#[cfg(not(feature = "optimize_for_size"))]
fn partition_at_index_loop<'a, T, F>(
    mut v: &'a mut [T],
    mut index: usize,
    mut ancestor_pivot: Option<&'a T>,
    is_less: &mut F,
) where
    F: FnMut(&T, &T) -> bool,
{
    // 限制迭代次数，并在达到限制时退回到快速确定性选择，以确保 O(n) 最坏运行时间。
    // 该限制必须是常量；如果像 `sort` 一样使用 `ilog2(len)`，会导致 O(n log n) 复杂度。
    // 具体数值有一定经验性，但多数输入中坏枢轴选择应较少；达到限制时剩余子切片通常已缩小到
    // len / 2^limit 或更少，fallback 的相对工作量也就很小。
    let mut limit = 16;

    loop {
        if v.len() <= INSERTION_SORT_THRESHOLD {
            if v.len() >= 2 {
                insertion_sort_shift_left(v, 1, is_less);
            }
            return;
        }

        if limit == 0 {
            median_of_medians(v, is_less, index);
            return;
        }

        limit -= 1;

        // 选择枢轴。
        let pivot_pos = choose_pivot(v, is_less);

        // 如果选出的枢轴等于祖先枢轴，则它是当前切片中的最小元素。把切片分成等于枢轴的元素
        // 和大于枢轴的元素。切片包含大量重复元素时通常会进入该分支。
        if let Some(p) = ancestor_pivot {
            let pivot = &v[pivot_pos];

            if !is_less(p, pivot) {
                let num_lt = partition(v, pivot_pos, &mut |a, b| !is_less(b, a));

                // 继续处理大于枢轴的元素。已知 `mid` 含有枢轴，因此可以从 `mid` 之后继续。
                let mid = num_lt + 1;

                // 如果已经越过目标 index，说明目标位置已满足要求。
                if mid > index {
                    return;
                }

                v = &mut v[mid..];
                index = index - mid;
                ancestor_pivot = None;
                continue;
            }
        }

        let mid = partition(v, pivot_pos, is_less);

        // 把切片分成 `left`、`pivot` 和 `right`。
        let (left, right) = v.split_at_mut(mid);
        let (pivot, right) = right.split_at_mut(1);
        let pivot = &pivot[0];

        if mid < index {
            v = right;
            index = index - mid - 1;
            ancestor_pivot = Some(pivot);
        } else if mid > index {
            v = left;
        } else {
            // 如果 mid == index，就完成了；partition() 已保证 mid 之后所有元素都大于或等于 mid。
            return;
        }
    }
}

/// 使用给定比较函数返回切片最小元素索引的辅助函数。
fn min_index<T, F: FnMut(&T, &T) -> bool>(slice: &[T], is_less: &mut F) -> Option<usize> {
    slice
        .iter()
        .enumerate()
        .reduce(|acc, t| if is_less(t.1, acc.1) { t } else { acc })
        .map(|(i, _)| i)
}

/// 使用给定比较函数返回切片最大元素索引的辅助函数。
fn max_index<T, F: FnMut(&T, &T) -> bool>(slice: &[T], is_less: &mut F) -> Option<usize> {
    slice
        .iter()
        .enumerate()
        .reduce(|acc, t| if is_less(acc.1, t.1) { t } else { acc })
        .map(|(i, _)| i)
}

/// 从切片中选择第 k 个元素的选择算法，保证 O(n) 时间。
/// 它本质上是使用 Tukey's Ninther 选择枢轴的 quickselect。
fn median_of_medians<T, F: FnMut(&T, &T) -> bool>(mut v: &mut [T], is_less: &mut F, mut k: usize) {
    // 该函数不是公开 API，不应使用越界索引调用。
    debug_assert!(k < v.len());

    // 如果 T 是 ZST，`partition_at_index` 已经会提前返回。
    debug_assert!(!T::IS_ZST);

    // 现在已知 `k < v.len() <= isize::MAX`。
    loop {
        if v.len() <= INSERTION_SORT_THRESHOLD {
            if v.len() >= 2 {
                insertion_sort_shift_left(v, 1, is_less);
            }

            return;
        }

        // `median_of_{minima,maxima}` 无法处理第一个/最后一个元素这类极端情况，
        // 因此在这里捕获并直接做线性搜索。
        if k == v.len() - 1 {
            // 找到最大元素并放到数组最后位置。已知 v 非空，因此可使用 `unwrap()`。
            let max_idx = max_index(v, is_less).unwrap();
            v.swap(max_idx, k);
            return;
        } else if k == 0 {
            // 找到最小元素并放到数组第一位置。已知 v 非空，因此可使用 `unwrap()`。
            let min_idx = min_index(v, is_less).unwrap();
            v.swap(min_idx, k);
            return;
        }

        let p = median_of_ninthers(v, is_less);

        if p == k {
            return;
        } else if p > k {
            v = &mut v[..p];
        } else {
            // 因为 `p < k < v.len()`，`p + 1` 不会溢出，并且是切片内有效索引。
            v = &mut v[p + 1..];
            k -= p + 1;
        }
    }
}

// 针对 `k` 位于切片中部附近的情况优化。它选择尽可能接近切片中位数的枢轴。
// 算法细节见论文 <https://drops.dagstuhl.de/opus/volltexte/2017/7612/pdf/LIPIcs-SEA-2017-24.pdf>。
fn median_of_ninthers<T, F: FnMut(&T, &T) -> bool>(v: &mut [T], is_less: &mut F) -> usize {
    // 使用 `saturating_mul`，避免乘法在 16 位平台上溢出。
    let frac = if v.len() <= 1024 {
        v.len() / 12
    } else if v.len() <= 128_usize.saturating_mul(1024) {
        v.len() / 64
    } else {
        v.len() / 1024
    };

    let pivot = frac / 2;
    let lo = v.len() / 2 - pivot;
    let hi = frac + lo;
    let gap = (v.len() - 9 * frac) / 4;
    let mut a = lo - 4 * frac - gap;
    let mut b = hi + gap;
    for i in lo..hi {
        ninther(v, is_less, a, i - frac, b, a + 1, i, b + 1, a + 2, i + frac, b + 2);
        a += 3;
        b += 3;
    }

    median_of_medians(&mut v[lo..lo + frac], is_less, pivot);

    partition(v, lo + pivot, is_less)
}

/// 移动索引 a..i 指定的 9 个元素，使 `v[d]` 包含这 9 个元素的中位数，
/// 其它元素围绕它分区。
fn ninther<T, F: FnMut(&T, &T) -> bool>(
    v: &mut [T],
    is_less: &mut F,
    a: usize,
    mut b: usize,
    c: usize,
    mut d: usize,
    e: usize,
    mut f: usize,
    g: usize,
    mut h: usize,
    i: usize,
) {
    b = median_idx(v, is_less, a, b, c);
    h = median_idx(v, is_less, g, h, i);
    if is_less(&v[h], &v[b]) {
        mem::swap(&mut b, &mut h);
    }
    if is_less(&v[f], &v[d]) {
        mem::swap(&mut d, &mut f);
    }
    if is_less(&v[e], &v[d]) {
        // 不需要做任何事。
    } else if is_less(&v[f], &v[e]) {
        d = f;
    } else {
        if is_less(&v[e], &v[b]) {
            v.swap(e, b);
        } else if is_less(&v[h], &v[e]) {
            v.swap(e, h);
        }
        return;
    }
    if is_less(&v[d], &v[b]) {
        d = b;
    } else if is_less(&v[h], &v[d]) {
        d = h;
    }

    v.swap(d, e);
}

/// 返回指向 `v[a]`、`v[b]`、`v[c]` 三个元素中位数的索引。
fn median_idx<T, F: FnMut(&T, &T) -> bool>(
    v: &[T],
    is_less: &mut F,
    mut a: usize,
    b: usize,
    mut c: usize,
) -> usize {
    if is_less(&v[c], &v[a]) {
        mem::swap(&mut a, &mut c);
    }
    if is_less(&v[c], &v[b]) {
        return c;
    }
    if is_less(&v[b], &v[a]) {
        return a;
    }
    b
}
