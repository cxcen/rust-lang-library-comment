//! 本模块包含不稳定 quicksort 以及两个分区实现。

#[cfg(not(feature = "optimize_for_size"))]
use crate::mem;
use crate::mem::ManuallyDrop;
#[cfg(not(feature = "optimize_for_size"))]
use crate::slice::sort::shared::pivot::choose_pivot;
#[cfg(not(feature = "optimize_for_size"))]
use crate::slice::sort::shared::smallsort::UnstableSmallSortTypeImpl;
#[cfg(not(feature = "optimize_for_size"))]
use crate::slice::sort::unstable::heapsort;
use crate::{cfg_select, intrinsics, ptr};

/// 递归排序 `v`。
///
/// 如果该切片在原始数组中有前驱元素，则通过 `ancestor_pivot` 指定它。
///
/// `limit` 是切换到 `heapsort` 之前允许的不平衡分区次数。
/// 如果它为零，本函数会立即切换到 heapsort。
#[cfg(not(feature = "optimize_for_size"))]
pub(crate) fn quicksort<'a, T, F>(
    mut v: &'a mut [T],
    mut ancestor_pivot: Option<&'a T>,
    mut limit: u32,
    is_less: &mut F,
) where
    F: FnMut(&T, &T) -> bool,
{
    loop {
        if v.len() <= T::small_sort_threshold() {
            T::small_sort(v, is_less);
            return;
        }

        // 如果已经选择了太多糟糕枢轴，就直接 fallback 到 heapsort，
        // 以保证最坏情况为 `O(N x log(N))`。
        if limit == 0 {
            heapsort::heapsort(v, is_less);
            return;
        }

        limit -= 1;

        // 选择枢轴，并尝试判断切片是否已经有序。
        let pivot_pos = choose_pivot(v, is_less);

        // 如果选中的枢轴等于前驱元素，则它是该切片中的最小元素。
        // 把切片分成等于枢轴的元素和大于枢轴的元素。
        // 当切片包含大量重复元素时，通常会命中这个分支。
        if let Some(p) = ancestor_pivot {
            if !is_less(p, &v[pivot_pos]) {
                let num_lt = partition(v, pivot_pos, &mut |a, b| !is_less(b, a));

                // 继续排序大于枢轴的元素。已知 `num_lt` 位置包含枢轴，
                // 因此可从 `num_lt` 之后继续。
                v = &mut v[(num_lt + 1)..];
                ancestor_pivot = None;
                continue;
            }
        }

        // 对切片分区。
        let num_lt = partition(v, pivot_pos, is_less);
        // SAFETY: partition 保证 `num_lt` 位于边界内。
        unsafe { intrinsics::assume(num_lt < v.len()) };

        // 把切片拆成 `left`、`pivot` 和 `right`。
        let (left, right) = v.split_at_mut(num_lt);
        let (pivot, right) = right.split_at_mut(1);
        let pivot = &pivot[0];

        // 递归处理左侧。这里有固定递归限制，测试显示递归处理较短侧没有实际收益。
        quicksort(left, ancestor_pivot, limit, is_less);

        // 继续处理右侧。
        v = right;
        ancestor_pivot = Some(pivot);
    }
}

/// 接收输入切片 `v` 并重排元素，使调用正常返回时，所有满足
/// `is_less(elem, pivot)` 为 true 的元素都位于 `v` 左侧，其中
/// `pivot == v[pivot_pos]`；其后是其它逻辑上被认为大于或等于 `pivot` 的元素。
///
/// 返回满足 `is_less(elem, pivot)` 为 true 的元素数量。
///
/// 如果 `is_less` 没有实现全序，则结果顺序和返回值未指定。
/// 所有原始元素仍会保留在 `v` 中，并且通过内部可变性产生的任何可能修改都可观察。
/// 当 `is_less` panic 或 `v.len()` 超过 `scratch.len()` 时也是如此。
pub(crate) fn partition<T, F>(v: &mut [T], pivot: usize, is_less: &mut F) -> usize
where
    F: FnMut(&T, &T) -> bool,
{
    let len = v.len();

    // 向编译器证明此性质，以允许生成无 panic 的代码。
    if len == 0 {
        return 0;
    }

    if pivot >= len {
        intrinsics::abort();
    }

    // SAFETY: 已检查 `pivot` 位于边界内。
    unsafe {
        // 把枢轴放到切片开头。
        v.swap_unchecked(0, pivot);
    }
    let (pivot, v_without_pivot) = v.split_at_mut(1);

    // 假设 Rust 会生成 noalias LLVM IR，则可确信形如 `(v: &mut [T], pivot: &T)`
    // 的分区函数签名能保证 pivot 和 v 不会 alias。这个保证对优化至关重要。
    // 也可以把枢轴值复制到栈上，但这会给具有内部可变性的类型制造问题，
    // 因而需要 drop guard。
    let pivot = &mut pivot[0];

    // 这个结构用于限制生成的 LLVM IR：只实例化需要的代码，可节省大量编译时间。
    // 思路来自 Frank Steffahn。
    let num_lt = (const { inst_partition::<T, F>() })(v_without_pivot, pivot, is_less);

    if num_lt >= len {
        intrinsics::abort();
    }

    // SAFETY: 已检查 `num_lt` 位于边界内。
    unsafe {
        // 把枢轴放在两个分区之间。
        v.swap_unchecked(0, num_lt);
    }

    num_lt
}

const fn inst_partition<T, F: FnMut(&T, &T) -> bool>() -> fn(&mut [T], &T, &mut F) -> usize {
    const MAX_BRANCHLESS_PARTITION_SIZE: usize = 96;
    if size_of::<T>() <= MAX_BRANCHLESS_PARTITION_SIZE {
        // 对复制成本相对较低、无分支优化收益较大的类型进行特化，
        // 例如 `u64` 和 `String`。
        cfg_select! {
            feature = "optimize_for_size" => {
                partition_lomuto_branchless_simple::<T, F>
            }
            _ => {
                partition_lomuto_branchless_cyclic::<T, F>
            }
        }
    } else {
        partition_hoare_branchy_cyclic::<T, F>
    }
}

/// 见 [`partition`]。
fn partition_hoare_branchy_cyclic<T, F>(v: &mut [T], pivot: &T, is_less: &mut F) -> usize
where
    F: FnMut(&T, &T) -> bool,
{
    let len = v.len();

    if len == 0 {
        return 0;
    }

    // 针对移动成本较高的大类型优化；不针对整数优化。
    // 这里偏向较小的 code-gen，并假设 is_less 是昂贵操作，可能生成大量代码或一次调用；
    // 同时假设复制元素很可能会变成 memcpy。使用两次 `ptr::copy_nonoverlapping`
    // 有机会比 `ptr::swap_nonoverlapping` 更快，因为 `memcpy` 可基于运行时特性检测
    // 使用宽 SIMD。benchmark 支持这一分析。

    let mut gap_opt: Option<GapGuard<T>> = None;

    // SAFETY: 从左到右的扫描循环会执行边界检查，此时已知
    // `left >= v_base && left < right && right <= v_base.add(len)`。
    // 从右到左的扫描循环也会执行边界检查，确保 `right` 在边界内。
    // 已检查 `len` 大于零，因此无条件执行 `right = right.sub(1)` 是安全的。
    // 退出检查确保 `left` 和 `right` 永不 alias，使 `ptr::copy_nonoverlapping`
    // 安全。drop guard `gap` 保证如果 `is_less` panic，总会覆盖输入中的重复值。
    // `gap.pos` 存储 `right` 的旧值，并从 `right` 开始，因此它同样在边界内。
    // 当保存的 `gap.value` 位于 `GapGuard` 中时，永远不会把它传给 `is_less`；
    // 因此通过内部可变性产生的任何修改都会被观察到。
    unsafe {
        let v_base = v.as_mut_ptr();

        let mut left = v_base;
        let mut right = v_base.add(len);

        loop {
            // 查找第一个大于枢轴的元素。
            while left < right && is_less(&*left, pivot) {
                left = left.add(1);
            }

            // 查找最后一个等于枢轴的元素。
            loop {
                right = right.sub(1);
                if left >= right || is_less(&*right, pivot) {
                    break;
                }
            }

            if left >= right {
                break;
            }

            // 通过循环置换交换找到的那对乱序元素。
            let is_first_swap_pair = gap_opt.is_none();

            if is_first_swap_pair {
                gap_opt = Some(GapGuard { pos: right, value: ManuallyDrop::new(ptr::read(left)) });
            }

            let gap = gap_opt.as_mut().unwrap_unchecked();

            // 分区中唯一实例化 ptr::copy_nonoverlapping 的位置。
            if !is_first_swap_pair {
                ptr::copy_nonoverlapping(left, gap.pos, 1);
            }
            gap.pos = right;
            ptr::copy_nonoverlapping(right, left, 1);

            left = left.add(1);
        }

        left.offset_from_unsigned(v_base)

        // `gap_opt` 离开作用域时，会用左侧第一个位于错误一侧且一开始被覆盖的元素，
        // 覆盖右侧最后一个位于错误一侧的元素。
    }
}

#[cfg(not(feature = "optimize_for_size"))]
struct PartitionState<T> {
    // 当前正在查看的元素；从左到右扫描切片。
    right: *mut T,
    // 记录比较结果为 less-than 的元素数量，同时绕开：
    // 相关问题：https://github.com/rust-lang/rust/issues/117128
    num_lt: usize,
    // 跟踪输入中临时重复元素的 gap guard。
    gap: GapGuardRaw<T>,
}

#[cfg(not(feature = "optimize_for_size"))]
fn partition_lomuto_branchless_cyclic<T, F>(v: &mut [T], pivot: &T, is_less: &mut F) -> usize
where
    F: FnMut(&T, &T) -> bool,
{
    // Lukas Bergdoll 和 Orson Peters 提出的新分区实现：无分支 Lomuto 分区，
    // 搭配循环置换。
    // 参考：https://github.com/Voultapher/sort-research-rs/blob/main/writeup/lomcyc_partition/text.md

    let len = v.len();
    let v_base = v.as_mut_ptr();

    if len == 0 {
        return 0;
    }

    // SAFETY: 已检查 `len` 大于零，因此读取 `v_base` 是安全的。
    // 随后使用有界循环，其中 `v_base.add(i)` 保证在边界内。
    // 根据类型系统规则，`v` 和 `pivot` 不能 alias。drop guard `gap`
    // 保证如果 `is_less` panic，总会覆盖输入中的重复值。`gap.pos`
    // 存储 `right` 的旧值，并从 `v_base` 开始，因此它同样在边界内。
    // 给定 `UNROLL_LEN == 2`，主循环之后会处于两种情况之一：
    // A) 由于 `len % 2 != 0`，`v` 中还有最后一个元素尚未处理；
    // B) 除了开始用 `ptr::read(v_base)` 保存的 gap 值之外，所有元素都已处理。
    // 在情况 A 中，循环会迭代两次，先执行 loop_body 处理未纳入展开的最后一个元素。
    // 之后行为与情况 B 相同：使用保存的值作为 `right` 来覆盖重复值。
    // 如果最后一次 `is_less` 调用 panic，保存的值会被复制回去，包括所有通过内部可变性
    // 产生的可能修改。如果 `is_less` 没有 panic 且代码继续执行，就覆盖重复值并执行
    // `right = right.add(1)`；对 `&mut *gap.value` 这样做是安全的，因为 `T`
    // 与 `[T; 1]` 相同，并且生成 allocation 末尾后一位指针是安全的。
    unsafe {
        let mut loop_body = |state: &mut PartitionState<T>| {
            let right_is_lt = is_less(&*state.right, pivot);
            let left = v_base.add(state.num_lt);

            ptr::copy(left, state.gap.pos, 1);
            ptr::copy_nonoverlapping(state.right, left, 1);

            state.gap.pos = state.right;
            state.num_lt += right_is_lt as usize;

            state.right = state.right.add(1);
        };

        // 理想情况下可直接在 PartitionState 中使用 GapGuard；但调用 `loop_body`
        // 时由 `&mut state` 物化出的引用，会创建一个指向包含 gap 值的父结构体的
        // 可变引用，从而使 cleanup 循环中由 gap 值引用创建的引用指针失效。
        // 这只在 Stacked Borrows 下是问题；Tree Borrows 接受使用 GapGuard 的直观代码。
        let mut gap_value = ManuallyDrop::new(ptr::read(v_base));

        let mut state = PartitionState {
            num_lt: 0,
            right: v_base.add(1),

            gap: GapGuardRaw { pos: v_base, value: &mut *gap_value },
        };

        // 手动展开在 x86、Arm 以及 opt-level=s 下效果良好，同时不会严重拖慢编译时间。
        // 让编译器自行决定时，结果从尚可到较差不等。
        let unroll_len = const { if size_of::<T>() <= 16 { 2 } else { 1 } };

        let unroll_end = v_base.add(len - (unroll_len - 1));
        while state.right < unroll_end {
            if unroll_len == 2 {
                loop_body(&mut state);
                loop_body(&mut state);
            } else {
                loop_body(&mut state);
            }
        }

        // 对展开后的 cleanup 和循环置换 cleanup 只实例化一次 `loop_body`，
        // 以优化二进制体积和编译时间。
        let end = v_base.add(len);
        loop {
            let is_done = state.right == end;
            state.right = if is_done { state.gap.value } else { state.right };

            loop_body(&mut state);

            if is_done {
                mem::forget(state.gap);
                break;
            }
        }

        state.num_lt
    }
}

#[cfg(feature = "optimize_for_size")]
fn partition_lomuto_branchless_simple<T, F: FnMut(&T, &T) -> bool>(
    v: &mut [T],
    pivot: &T,
    is_less: &mut F,
) -> usize {
    let mut left = 0;

    for right in 0..v.len() {
        // SAFETY: `left` 在每次循环迭代中最多递增 1，因此 left <= right，
        // 二者都位于边界内。
        unsafe {
            let right_is_lt = is_less(v.get_unchecked(right), pivot);
            v.swap_unchecked(left, right);
            left += right_is_lt as usize;
        }
    }

    left
}

struct GapGuard<T> {
    pos: *mut T,
    value: ManuallyDrop<T>,
}

impl<T> Drop for GapGuard<T> {
    fn drop(&mut self) {
        // SAFETY: `self` 必须以如下方式构造：把 gap 值复制到 `self.pos` 是健全的。
        unsafe {
            ptr::copy_nonoverlapping(&*self.value, self.pos, 1);
        }
    }
}

/// 理想情况下不需要这个类型，直接使用常规 GapGuard 即可。
/// 见 [`partition_lomuto_branchless_cyclic`] 中的注释。
#[cfg(not(feature = "optimize_for_size"))]
struct GapGuardRaw<T> {
    pos: *mut T,
    value: *mut T,
}

#[cfg(not(feature = "optimize_for_size"))]
impl<T> Drop for GapGuardRaw<T> {
    fn drop(&mut self) {
        // SAFETY: `self` 必须以如下方式构造：把 gap 值复制到 `self.pos` 是健全的。
        unsafe {
            ptr::copy_nonoverlapping(self.value, self.pos, 1);
        }
    }
}
