//! 本模块包含稳定 quicksort 和稳定分区实现。

use crate::mem::{ManuallyDrop, MaybeUninit};
use crate::slice::sort::shared::FreezeMarker;
use crate::slice::sort::shared::pivot::choose_pivot;
use crate::slice::sort::shared::smallsort::StableSmallSortTypeImpl;
use crate::{intrinsics, ptr};

/// 使用 quicksort 递归排序 `v`。
///
/// `scratch.len()` 必须至少为
/// `max(v.len() - v.len() / 2, SMALL_SORT_GENERAL_SCRATCH_LEN)`，
/// 否则实现可能会 abort。
///
/// 当 `limit` 被初始化为某个 c 对应的 `c*log(v.len())` 时，可以确保不会栈溢出，
/// 也不会退化为平方复杂度。
#[inline(never)]
pub fn quicksort<T, F: FnMut(&T, &T) -> bool>(
    mut v: &mut [T],
    scratch: &mut [MaybeUninit<T>],
    mut limit: u32,
    mut left_ancestor_pivot: Option<&T>,
    is_less: &mut F,
) {
    loop {
        let len = v.len();

        if len <= T::small_sort_threshold() {
            T::small_sort(v, scratch, is_less);
            return;
        }

        if limit == 0 {
            // 已经遇到太多糟糕枢轴，切换到 O(n log n) fallback 算法。
            // 在这里 fallback 是 eager mode 的 driftsort。
            crate::slice::sort::stable::drift::sort(v, scratch, true, is_less);
            return;
        }
        limit -= 1;

        let pivot_pos = choose_pivot(v, is_less);

        // SAFETY: 只有对 Freeze 类型才访问临时副本；否则通过 `is_less`
        // 发生的自修改无法被观察到，这会不健全。临时副本不会逃出本作用域。
        let pivot_copy = unsafe { ManuallyDrop::new(ptr::read(&v[pivot_pos])) };
        let pivot_ref = (!has_direct_interior_mutability::<T>()).then_some(&*pivot_copy);

        // 选择枢轴后，检查该枢轴是否等于左侧祖先枢轴。如果相等，
        // 就把等于枢轴的元素分区到左侧，并且不再递归处理它们。
        // 对 k 个不同值，这给出 O(n log k) 的排序复杂度；该策略借鉴自 pdqsort。
        // 对带内部可变性的类型，不能健全地创建祖先枢轴的临时副本，
        // 因此使用 left_partition_len == 0 来检测是否复用了某个枢轴。
        // 这意味着对同一枢轴 p 最多执行三次分区，而不是最优的两次。
        let mut perform_equal_partition = false;
        if let Some(la_pivot) = left_ancestor_pivot {
            perform_equal_partition = !is_less(la_pivot, &v[pivot_pos]);
        }

        let mut left_partition_len = 0;
        if !perform_equal_partition {
            left_partition_len = stable_partition(v, scratch, pivot_pos, false, is_less);
            perform_equal_partition = left_partition_len == 0;
        }

        if perform_equal_partition {
            let mid_eq = stable_partition(v, scratch, pivot_pos, true, &mut |a, b| !is_less(b, a));
            v = &mut v[mid_eq..];
            left_ancestor_pivot = None;
            continue;
        }

        // 下一轮循环处理左侧，递归处理右侧。
        let (left, right) = v.split_at_mut(left_partition_len);
        quicksort(right, scratch, limit, pivot_ref, is_less);
        v = left;
    }
}

/// 使用枢轴 `p = v[pivot_pos]` 对 `v` 分区，并返回小于 `p` 的元素个数。
/// 比较结果为 < p 的元素之间、以及比较结果为 >= p 的元素之间都会保留相对顺序，
/// 因此这是稳定分区。
///
/// 如果 `is_less` 不是严格全序或发生 panic，或者 `scratch.len() < v.len()`，
/// 或者 `pivot_pos >= v.len()`，结果和 `v` 的状态仍保持健全，但具体内容未指定。
fn stable_partition<T, F: FnMut(&T, &T) -> bool>(
    v: &mut [T],
    scratch: &mut [MaybeUninit<T>],
    pivot_pos: usize,
    pivot_goes_left: bool,
    is_less: &mut F,
) -> usize {
    let len = v.len();

    if intrinsics::unlikely(scratch.len() < len || pivot_pos >= len) {
        core::intrinsics::abort()
    }

    let v_base = v.as_ptr();
    let scratch_base = scratch.as_mut_ptr().cast_init();

    // 核心思路是：把比较结果为 less-than 的值写到 `scratch` 左侧，
    // 把与 `v[pivot_pos]` 比较后为 greater-or-equal 的值按反向写到 `scratch`
    // 右侧。细节见 PartitionState。

    // SAFETY: 见各处局部注释。
    unsafe {
        // SAFETY: 已确保 scratch 长度 >= len，且 pivot_pos 在边界内。
        // v 和 scratch 是互不相交的切片。
        let pivot = v_base.add(pivot_pos);
        let mut state = PartitionState::new(v_base, scratch_base, len);

        let mut pivot_in_scratch = ptr::null_mut();
        let mut loop_end_pos = pivot_pos;

        // SAFETY: 这个循环等价于准确调用 state.partition_one len 次。
        loop {
            // 理想情况下外层循环不应展开，以节省二进制体积；
            // 但对小类型，内层循环展开在 benchmark 中带来显著性能提升。
            // 通过 for _ in 0..UNROLL_LEN { .. } 而不是手动展开可改善编译时间，
            // 但在 opt-level=s 下会有约 10-20% 的性能损失。
            if const { size_of::<T>() <= 16 } {
                const UNROLL_LEN: usize = 4;
                let unroll_end = v_base.add(loop_end_pos.saturating_sub(UNROLL_LEN - 1));
                while state.scan < unroll_end {
                    state.partition_one(is_less(&*state.scan, &*pivot));
                    state.partition_one(is_less(&*state.scan, &*pivot));
                    state.partition_one(is_less(&*state.scan, &*pivot));
                    state.partition_one(is_less(&*state.scan, &*pivot));
                }
            }

            let loop_end = v_base.add(loop_end_pos);
            while state.scan < loop_end {
                state.partition_one(is_less(&*state.scan, &*pivot));
            }

            if loop_end_pos == len {
                break;
            }

            // 避免将枢轴与自身比较，因为某些比较运算可能因此死锁。
            // 同时记录它稍后所在的位置。
            pivot_in_scratch = state.partition_one(pivot_goes_left);

            loop_end_pos = len;
        }

        // 必须把 `pivot` 再次复制到正确位置，因为比较运算可能已经修改了它。
        if has_direct_interior_mutability::<T>() {
            ptr::copy_nonoverlapping(pivot, pivot_in_scratch, 1);
        }

        // SAFETY: partition_one 被准确调用 len 次，保证 scratch 中已初始化出
        // `v` 的一个排列副本，并且 num_left <= v.len()。因此把
        // scratch[0..num_left] 和 scratch[num_left..v.len()] 复制回去是健全的；
        // scratch 中的值之后不会再读取，所以这些复制在语义上相当于移动并重排 `v`。

        // 把所有 < p 的元素直接从 scratch 复制到 v。
        let v_base = v.as_mut_ptr();
        ptr::copy_nonoverlapping(scratch_base, v_base, state.num_left);

        // 按反向复制所有 >= p 的元素。
        for i in 0..len - state.num_left {
            ptr::copy_nonoverlapping(
                scratch_base.add(len - 1 - i),
                v_base.add(state.num_left + i),
                1,
            );
        }

        state.num_left
    }
}

struct PartitionState<T> {
    // scratch 辅助内存的起点。
    scratch_base: *mut T,
    // 当前正在查看的元素；从左到右扫描切片。
    scan: *const T,
    // 记录进入左侧的元素数量，同时绕开：
    // 相关问题：https://github.com/rust-lang/rust/issues/117128
    num_left: usize,
    // 反向 scratch 输出指针。
    scratch_rev: *mut T,
}

impl<T> PartitionState<T> {
    /// # 安全性(Safety）
    ///
    /// `scan` 和 `scratch` 必须指向长度为 `len` 的有效且互不相交的缓冲区。
    /// scan 缓冲区必须已经初始化。
    unsafe fn new(scan: *const T, scratch: *mut T, len: usize) -> Self {
        // SAFETY: 见函数安全性注释。
        unsafe { Self { scratch_base: scratch, scan, num_left: 0, scratch_rev: scratch.add(len) } }
    }

    /// 根据 `towards_left` 的值，把一个值写到 scratch 内存中正在增长的左侧或右侧。
    /// 这构成分区算法的无分支核心。
    ///
    /// # 安全性(Safety）
    ///
    /// 本函数最多可调用 `len` 次。如果正好调用 `len` 次，则 scratch 缓冲区中
    /// 会恰好包含 scan 缓冲区每个元素的一份副本，即一个排列，并且 num_left <= len。
    unsafe fn partition_one(&mut self, towards_left: bool) -> *mut T {
        // SAFETY: 见各处局部注释。
        unsafe {
            // SAFETY: 本函数最多调用 len 次，因此 right 到目前为止最多递增 len - 1 次，
            // 仍在边界内。类似地，num_left < len 且 num_right < len；
            // 在第 i 次迭代开始时（从零计数），num_right == i - num_left。
            self.scratch_rev = self.scratch_rev.sub(1);

            // SAFETY: 此时 scratch_rev == base + len - (i + 1)。
            // 因此 scratch_rev + num_left == base + len - 1 - num_right < base + len。
            let dst_base = if towards_left { self.scratch_base } else { self.scratch_rev };
            let dst = dst_base.add(self.num_left);
            ptr::copy_nonoverlapping(self.scan, dst, 1);

            self.num_left += towards_left as usize;
            self.scan = self.scan.add(1);
            dst
        }
    }
}

trait IsFreeze {
    fn is_freeze() -> bool;
}

impl<T> IsFreeze for T {
    default fn is_freeze() -> bool {
        false
    }
}
impl<T: FreezeMarker> IsFreeze for T {
    fn is_freeze() -> bool {
        true
    }
}

#[must_use]
fn has_direct_interior_mutability<T>() -> bool {
    // 如果类型具有内部可变性，它可能在比较期间修改自身；
    // 这些修改必须在排序操作结束后仍被保留。否则像 Mutex<Option<Box<str>>>
    // 这样的类型可能导致 double free。
    !T::is_freeze()
}
