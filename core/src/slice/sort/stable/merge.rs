//! 本模块包含合并两个已排序子切片的逻辑。

use crate::mem::MaybeUninit;
use crate::{cmp, ptr};

/// 使用 `scratch` 作为临时存储，合并非递减 run `v[..mid]` 和 `v[mid..]`，
/// 并把结果存回 `v[..]`。
pub fn merge<T, F: FnMut(&T, &T) -> bool>(
    v: &mut [T],
    scratch: &mut [MaybeUninit<T>],
    mid: usize,
    is_less: &mut F,
) {
    let len = v.len();

    if mid == 0 || mid >= len || scratch.len() < cmp::min(mid, len - mid) {
        return;
    }

    // SAFETY: 已检查两个切片均非空且 `mid` 位于边界内。
    // 也已检查 `scratch` 有足够容量容纳较短切片的副本。
    // `merge_up` 和 `merge_down` 的实现方式会维护 `MergeState::drop`
    // 中描述的契约。
    unsafe {
        // 合并过程先把较短 run 复制到 `buf`。随后沿正向（或反向）同时扫描
        // 新复制的 run 和较长 run，比较二者尚未消耗的下一个元素，并把较小
        // （或较大）的元素复制回 `v`。
        //
        // 一旦较短 run 被完全消耗，过程就结束。如果较长 run 先被消耗，
        // 则必须把较短 run 中剩余的元素复制到 `v` 中留下的空洞。
        //
        // 过程中的中间状态始终由 `gap` 跟踪，它有两个作用：
        //  1. 当 `is_less` panic 时保护 `v` 的完整性。
        //  2. 如果较长 run 先被消耗，则填补 `v` 中剩余的空洞。

        let buf = scratch.as_mut_ptr().cast_init();

        let v_base = v.as_mut_ptr();
        let v_mid = v_base.add(mid);
        let v_end = v_base.add(len);

        let left_len = mid;
        let right_len = len - mid;

        let left_is_shorter = left_len <= right_len;
        let save_base = if left_is_shorter { v_base } else { v_mid };
        let save_len = if left_is_shorter { left_len } else { right_len };

        ptr::copy_nonoverlapping(save_base, buf, save_len);

        let mut merge_state = MergeState { start: buf, end: buf.add(save_len), dst: save_base };

        if left_is_shorter {
            merge_state.merge_up(v_mid, v_end, is_less);
        } else {
            merge_state.merge_down(v_base, buf, v_end, is_less);
        }
        // 最后 `merge_state` 会被 drop。如果较短 run 尚未完全消耗，
        // 此时会把其剩余部分复制进 `v` 中的空洞。
    }
}

// drop 时把范围 `start..end` 复制到 `dst..`。
struct MergeState<T> {
    start: *mut T,
    end: *mut T,
    dst: *mut T,
}

impl<T> MergeState<T> {
    /// # 安全性(Safety）
    /// 调用方必须保证 `self` 以如下方式初始化：`start -> end` 是较长子切片，
    /// 并且 `dst` 至少能写入较短子切片长度那么多次。此外，`start -> end`
    /// 和 `right -> right_end` 都必须可读。本函数必须只调用一次。
    unsafe fn merge_up<F: FnMut(&T, &T) -> bool>(
        &mut self,
        mut right: *const T,
        right_end: *const T,
        is_less: &mut F,
    ) {
        // SAFETY: 见函数安全性注释。
        unsafe {
            let left = &mut self.start;
            let out = &mut self.dst;

            while *left != self.end && right as *const T != right_end {
                let consume_left = !is_less(&*right, &**left);

                let src = if consume_left { *left } else { right };
                ptr::copy_nonoverlapping(src, *out, 1);

                *left = left.add(consume_left as usize);
                right = right.add(!consume_left as usize);

                *out = out.add(1);
            }
        }
    }

    /// # 安全性(Safety）
    /// 调用方必须保证 `self` 以如下方式初始化：`left_end <- dst` 是较短子切片，
    /// 并且 `out` 至少能写入较短子切片长度那么多次。此外，`left_end <- dst`
    /// 和 `right_end <- end` 都必须可读。本函数必须只调用一次。
    unsafe fn merge_down<F: FnMut(&T, &T) -> bool>(
        &mut self,
        left_end: *const T,
        right_end: *const T,
        mut out: *mut T,
        is_less: &mut F,
    ) {
        // SAFETY: 见函数安全性注释。
        unsafe {
            loop {
                let left = self.dst.sub(1);
                let right = self.end.sub(1);
                out = out.sub(1);

                let consume_left = is_less(&*right, &*left);

                let src = if consume_left { left } else { right };
                ptr::copy_nonoverlapping(src, out, 1);

                self.dst = left.add(!consume_left as usize);
                self.end = right.add(consume_left as usize);

                if self.dst as *const T == left_end || self.end as *const T == right_end {
                    break;
                }
            }
        }
    }
}

impl<T> Drop for MergeState<T> {
    fn drop(&mut self) {
        // SAFETY: MergeState 的使用者必须保证，在该 drop 实现可能运行的任何时刻
        // （例如用户提供的 `is_less` panic 时），把 `start` 与 `end` 之间的
        // 连续区域复制到 `dst` 后，输入切片 `v` 仍包含每个原始元素，并保留所有
        // 已观察到的可能修改。
        unsafe {
            let len = self.end.offset_from_unsigned(self.start);
            ptr::copy_nonoverlapping(self.start, self.dst, len);
        }
    }
}
