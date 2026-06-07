//! 本模块包含作为不稳定 quicksort fallback 的无分支 heapsort。

use crate::{cmp, intrinsics, ptr};

/// 使用 heapsort 排序 `v`，它保证最坏情况为 *O*(*n* \* log(*n*))。
///
/// 不要内联本函数；它位于 `recurse` 的主热循环中，并且只作为低概率算法 fallback。
#[inline(never)]
pub(crate) fn heapsort<T, F>(v: &mut [T], is_less: &mut F)
where
    F: FnMut(&T, &T) -> bool,
{
    let len = v.len();

    for i in (0..len + len / 2).rev() {
        let sift_idx = if i >= len {
            i - len
        } else {
            v.swap(0, i);
            0
        };

        // SAFETY: 上面的计算保证 `sift_idx` 要么是 0，要么是
        // `(len..(len + (len / 2))) - len`，即 `0..(len / 2)`。
        // 因此满足所需的 `sift_idx <= len`。
        unsafe {
            sift_down(&mut v[..cmp::min(i, len)], sift_idx, is_less);
        }
    }
}

// 这个二叉堆维护不变量 `parent >= child`。
//
// SAFETY: 调用方必须保证 `node <= v.len()`。
#[inline(always)]
unsafe fn sift_down<T, F>(v: &mut [T], mut node: usize, is_less: &mut F)
where
    F: FnMut(&T, &T) -> bool,
{
    // SAFETY: 见函数安全性注释。
    unsafe {
        intrinsics::assume(node <= v.len());
    }

    let len = v.len();

    let v_base = v.as_mut_ptr();

    loop {
        // `node` 的子节点。
        let mut child = 2 * node + 1;
        if child >= len {
            break;
        }

        // SAFETY: 不变量与检查保证 node 和 child 都位于边界内。
        unsafe {
            // 选择较大的子节点。
            if child + 1 < len {
                // 需要一个分支来确保不会越界索引，但它高度可预测。
                // 比较本身则更适合无分支完成，尤其是对 primitive 类型。
                child += is_less(&*v_base.add(child), &*v_base.add(child + 1)) as usize;
            }

            // 如果 `node` 处不变量已经成立，则停止。
            if !is_less(&*v_base.add(node), &*v_base.add(child)) {
                break;
            }

            ptr::swap_nonoverlapping(v_base.add(node), v_base.add(child), 1);
        }

        node = child;
    }
}
