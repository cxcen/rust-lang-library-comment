//! 本模块包含多种针对小长度优化的排序实现。

use crate::mem::{self, ManuallyDrop, MaybeUninit};
use crate::slice::sort::shared::FreezeMarker;
use crate::{hint, intrinsics, ptr, slice};

// 必须区分两类性能：直接排序小切片时的 SMALL_SORT_THRESHOLD 表现，
// 以及作为 quicksort 主循环的一部分排序小子切片时的 small-sort 表现。
// 对前者，测试表明更能代表真实世界性能的是冷 CPU 状态下的 benchmark，
// 而不是单一长度的热 benchmark。对后者，CPU 会多次调用这些例程，
// 因而热 benchmark 是合理且更真实的。也正因为如此，
// 用比 insertion sort 更复杂的方案优化小子切片排序是值得的。

/// 使用 trait 允许在 `Freeze` 上做 specialization，从而构造安全抽象。
pub(crate) trait StableSmallSortTypeImpl: Sized {
    /// 当输入长度 <= 本函数返回值时，调用 `small_sort` 是有效的。
    fn small_sort_threshold() -> usize;

    /// 使用针对小尺寸优化的策略排序 `v`。
    fn small_sort<F: FnMut(&Self, &Self) -> bool>(
        v: &mut [Self],
        scratch: &mut [MaybeUninit<Self>],
        is_less: &mut F,
    );
}

impl<T> StableSmallSortTypeImpl for T {
    #[inline(always)]
    default fn small_sort_threshold() -> usize {
        // 比较次数最优，并且性能良好。
        SMALL_SORT_FALLBACK_THRESHOLD
    }

    #[inline(always)]
    default fn small_sort<F: FnMut(&T, &T) -> bool>(
        v: &mut [T],
        _scratch: &mut [MaybeUninit<T>],
        is_less: &mut F,
    ) {
        if v.len() >= 2 {
            insertion_sort_shift_left(v, 1, is_less);
        }
    }
}

impl<T: FreezeMarker> StableSmallSortTypeImpl for T {
    #[inline(always)]
    fn small_sort_threshold() -> usize {
        SMALL_SORT_GENERAL_THRESHOLD
    }

    #[inline(always)]
    fn small_sort<F: FnMut(&T, &T) -> bool>(
        v: &mut [T],
        scratch: &mut [MaybeUninit<T>],
        is_less: &mut F,
    ) {
        small_sort_general_with_scratch(v, scratch, is_less);
    }
}

/// 使用 trait 允许在 `Freeze` 上做 specialization，从而构造安全抽象。
pub(crate) trait UnstableSmallSortTypeImpl: Sized {
    /// 当输入长度 <= 本函数返回值时，调用 `small_sort` 是有效的。
    fn small_sort_threshold() -> usize;

    /// 使用针对小尺寸优化的策略排序 `v`。
    fn small_sort<F: FnMut(&Self, &Self) -> bool>(v: &mut [Self], is_less: &mut F);
}

impl<T> UnstableSmallSortTypeImpl for T {
    #[inline(always)]
    default fn small_sort_threshold() -> usize {
        SMALL_SORT_FALLBACK_THRESHOLD
    }

    #[inline(always)]
    default fn small_sort<F>(v: &mut [T], is_less: &mut F)
    where
        F: FnMut(&T, &T) -> bool,
    {
        small_sort_fallback(v, is_less);
    }
}

impl<T: FreezeMarker> UnstableSmallSortTypeImpl for T {
    #[inline(always)]
    fn small_sort_threshold() -> usize {
        <T as UnstableSmallSortFreezeTypeImpl>::small_sort_threshold()
    }

    #[inline(always)]
    fn small_sort<F>(v: &mut [T], is_less: &mut F)
    where
        F: FnMut(&T, &T) -> bool,
    {
        <T as UnstableSmallSortFreezeTypeImpl>::small_sort(v, is_less);
    }
}

/// FIXME(const_trait_impl) 使用原始 ipnsort 中带 choose_unstable_small_sort 的方案，
/// 见 <https://github.com/Voultapher/sort-research-rs/blob/438fad5d0495f65d4b72aa87f0b62fc96611dff3/ipnsort/src/smallsort.rs#L83C10-L83C36>。
pub(crate) trait UnstableSmallSortFreezeTypeImpl: Sized + FreezeMarker {
    fn small_sort_threshold() -> usize;

    fn small_sort<F: FnMut(&Self, &Self) -> bool>(v: &mut [Self], is_less: &mut F);
}

impl<T: FreezeMarker> UnstableSmallSortFreezeTypeImpl for T {
    #[inline(always)]
    default fn small_sort_threshold() -> usize {
        if (size_of::<T>() * SMALL_SORT_GENERAL_SCRATCH_LEN) <= MAX_STACK_ARRAY_SIZE {
            SMALL_SORT_GENERAL_THRESHOLD
        } else {
            SMALL_SORT_FALLBACK_THRESHOLD
        }
    }

    #[inline(always)]
    default fn small_sort<F>(v: &mut [T], is_less: &mut F)
    where
        F: FnMut(&T, &T) -> bool,
    {
        if (size_of::<T>() * SMALL_SORT_GENERAL_SCRATCH_LEN) <= MAX_STACK_ARRAY_SIZE {
            small_sort_general(v, is_less);
        } else {
            small_sort_fallback(v, is_less);
        }
    }
}

/// SAFETY: 仅用于运行时优化启发式。
#[rustc_unsafe_specialization_marker]
trait CopyMarker {}

impl<T: Copy> CopyMarker for T {}

impl<T: FreezeMarker + CopyMarker> UnstableSmallSortFreezeTypeImpl for T {
    #[inline(always)]
    fn small_sort_threshold() -> usize {
        if has_efficient_in_place_swap::<T>()
            && (size_of::<T>() * SMALL_SORT_NETWORK_SCRATCH_LEN) <= MAX_STACK_ARRAY_SIZE
        {
            SMALL_SORT_NETWORK_THRESHOLD
        } else if (size_of::<T>() * SMALL_SORT_GENERAL_SCRATCH_LEN) <= MAX_STACK_ARRAY_SIZE {
            SMALL_SORT_GENERAL_THRESHOLD
        } else {
            SMALL_SORT_FALLBACK_THRESHOLD
        }
    }

    #[inline(always)]
    fn small_sort<F>(v: &mut [T], is_less: &mut F)
    where
        F: FnMut(&T, &T) -> bool,
    {
        if has_efficient_in_place_swap::<T>()
            && (size_of::<T>() * SMALL_SORT_NETWORK_SCRATCH_LEN) <= MAX_STACK_ARRAY_SIZE
        {
            small_sort_network(v, is_less);
        } else if (size_of::<T>() * SMALL_SORT_GENERAL_SCRATCH_LEN) <= MAX_STACK_ARRAY_SIZE {
            small_sort_general(v, is_less);
        } else {
            small_sort_fallback(v, is_less);
        }
    }
}

/// 比较次数最优，并且性能良好。
const SMALL_SORT_FALLBACK_THRESHOLD: usize = 16;

/// 从比较次数角度看，20 对完全随机输入约高效 2%；但从实际耗时看，选择 32
/// 的整体性能更好。
///
/// SAFETY: 如果修改这个值，必须同步调整 [`small_sort_general`]！
const SMALL_SORT_GENERAL_THRESHOLD: usize = 32;

/// [`small_sort_general`] 使用 [`sort8_stable`] 作为 primitive，并执行一种 ping-pong
/// merge：前两次 [`sort8_stable`] 调用的输出存放在 scratch 缓冲区末尾。
/// 这简化了 panic 处理并避免额外复制，也会影响所需的 scratch 缓冲区大小。
///
/// SAFETY: 如果修改这个值，必须同步调整 [`small_sort_general`]！
pub(crate) const SMALL_SORT_GENERAL_SCRATCH_LEN: usize = SMALL_SORT_GENERAL_THRESHOLD + 16;

/// SAFETY: 如果修改这个值，必须同步调整 [`small_sort_network`]！
const SMALL_SORT_NETWORK_THRESHOLD: usize = 32;
const SMALL_SORT_NETWORK_SCRATCH_LEN: usize = SMALL_SORT_NETWORK_THRESHOLD;

/// 使用栈数组时，如果类型 `T` 非常大，可能导致栈溢出。
/// 出于保守考虑，需要栈数组的 small-sort 只用于能落在此限制内的类型。
const MAX_STACK_ARRAY_SIZE: usize = 4096;

fn small_sort_fallback<T, F: FnMut(&T, &T) -> bool>(v: &mut [T], is_less: &mut F) {
    if v.len() >= 2 {
        insertion_sort_shift_left(v, 1, is_less);
    }
}

fn small_sort_general<T: FreezeMarker, F: FnMut(&T, &T) -> bool>(v: &mut [T], is_less: &mut F) {
    let mut stack_array = MaybeUninit::<[T; SMALL_SORT_GENERAL_SCRATCH_LEN]>::uninit();

    // SAFETY: 该内存由 `stack_array` 支撑；只要长度保持相同，这个操作就是安全的。
    let scratch = unsafe {
        slice::from_raw_parts_mut(
            stack_array.as_mut_ptr() as *mut MaybeUninit<T>,
            SMALL_SORT_GENERAL_SCRATCH_LEN,
        )
    };

    small_sort_general_with_scratch(v, scratch, is_less);
}

fn small_sort_general_with_scratch<T: FreezeMarker, F: FnMut(&T, &T) -> bool>(
    v: &mut [T],
    scratch: &mut [MaybeUninit<T>],
    is_less: &mut F,
) {
    let len = v.len();
    if len < 2 {
        return;
    }

    if scratch.len() < len + 16 {
        intrinsics::abort();
    }

    let v_base = v.as_mut_ptr();
    let len_div_2 = len / 2;

    // SAFETY: 见各处局部注释。
    unsafe {
        let scratch_base = scratch.as_mut_ptr() as *mut T;

        let presorted_len = if const { size_of::<T>() <= 16 } && len >= 16 {
            // SAFETY: scratch_base 有效且空间足够。
            sort8_stable(v_base, scratch_base, scratch_base.add(len), is_less);
            sort8_stable(
                v_base.add(len_div_2),
                scratch_base.add(len_div_2),
                scratch_base.add(len + 8),
                is_less,
            );

            8
        } else if len >= 8 {
            // SAFETY: scratch_base 有效且空间足够。
            sort4_stable(v_base, scratch_base, is_less);
            sort4_stable(v_base.add(len_div_2), scratch_base.add(len_div_2), is_less);

            4
        } else {
            ptr::copy_nonoverlapping(v_base, scratch_base, 1);
            ptr::copy_nonoverlapping(v_base.add(len_div_2), scratch_base.add(len_div_2), 1);

            1
        };

        for offset in [0, len_div_2] {
            // SAFETY: 此时 dst 已初始化 presorted_len 个元素。
            // 这里把它扩展到 desired_len；src 对 desired_len 个元素有效。
            let src = v_base.add(offset);
            let dst = scratch_base.add(offset);
            let desired_len = if offset == 0 { len_div_2 } else { len - len_div_2 };

            for i in presorted_len..desired_len {
                ptr::copy_nonoverlapping(src.add(i), dst.add(i), 1);
                insert_tail(dst, dst.add(i), is_less);
            }
        }

        // SAFETY: 见 `CopyOnDrop::drop` 中的注释。
        let drop_guard = CopyOnDrop { src: scratch_base, dst: v_base, len };

        // SAFETY: 此时 scratch_base 已完全初始化，因此可作为源数据合并回原数组。
        // 如果发生 panic，drop_guard 会确保原数组恢复为输入的一个有效排列。
        // 这种技术类似 ping-pong merging。
        bidirectional_merge(
            &*ptr::slice_from_raw_parts(drop_guard.src, drop_guard.len),
            drop_guard.dst,
            is_less,
        );
        mem::forget(drop_guard);
    }
}

struct CopyOnDrop<T> {
    src: *const T,
    dst: *mut T,
    len: usize,
}

impl<T> Drop for CopyOnDrop<T> {
    fn drop(&mut self) {
        // SAFETY: `src` 必须包含 `len` 个已初始化元素，dst 必须可写入 `len` 个元素。
        unsafe {
            ptr::copy_nonoverlapping(self.src, self.dst, self.len);
        }
    }
}

fn small_sort_network<T, F>(v: &mut [T], is_less: &mut F)
where
    T: FreezeMarker,
    F: FnMut(&T, &T) -> bool,
{
    // 这个实现针对整数类型的效率做了调优。

    let len = v.len();
    if len < 2 {
        return;
    }

    if len > SMALL_SORT_NETWORK_SCRATCH_LEN {
        intrinsics::abort();
    }

    let mut stack_array = MaybeUninit::<[T; SMALL_SORT_NETWORK_SCRATCH_LEN]>::uninit();

    let len_div_2 = len / 2;
    let no_merge = len < 18;

    let v_base = v.as_mut_ptr();
    let initial_region_len = if no_merge { len } else { len_div_2 };
    // SAFETY: `initial_region_len` 的两个可能值都位于边界内。
    let mut region = unsafe { &mut *ptr::slice_from_raw_parts_mut(v_base, initial_region_len) };

    // 避免编译器展开；出于二进制体积原因，这里确实不希望发生展开。
    loop {
        let presorted_len = if region.len() >= 13 {
            sort13_optimal(region, is_less);
            13
        } else if region.len() >= 9 {
            sort9_optimal(region, is_less);
            9
        } else {
            1
        };

        insertion_sort_shift_left(region, presorted_len, is_less);

        if no_merge {
            return;
        }

        if region.as_ptr() != v_base {
            break;
        }

        // SAFETY: 基于 `len_div_2` 得到的 `v` 右侧保证在边界内。
        unsafe {
            region = &mut *ptr::slice_from_raw_parts_mut(v_base.add(len_div_2), len - len_div_2)
        };
    }

    // SAFETY: 已检查 T 是 Freeze，因此观察是安全的。
    // 如果 is_less panic，v 在 parity_merge 中未被修改，仍保留原始输入。
    // scratch 与 v 不能 alias，并且 scratch 有 v.len() 的空间。
    unsafe {
        let scratch_base = stack_array.as_mut_ptr() as *mut T;
        bidirectional_merge(
            &mut *ptr::slice_from_raw_parts_mut(v_base, len),
            scratch_base,
            is_less,
        );
        ptr::copy_nonoverlapping(scratch_base, v_base, len);
    }
}

/// 如果 `b_pos` 位置的值小于 `a_pos` 位置的值，则交换 `v_base`
/// 指向的切片中这两个位置的值。
///
/// 尽管对整数类类型希望它被内联，这里仍故意不标记 `#[inline]`。
/// `is_less` 可能是非常大的函数，因此要给编译器不内联本函数的选择。
/// 由于本函数对性能非常关键，它应与使用它的函数位于同一模块中。
unsafe fn swap_if_less<T, F>(v_base: *mut T, a_pos: usize, b_pos: usize, is_less: &mut F)
where
    F: FnMut(&T, &T) -> bool,
{
    // SAFETY: 调用方必须保证 `a_pos` 和 `b_pos` 分别加到 `v_base` 后，
    // 会得到指向 `v_base` 所在区域内的有效指针；这些指针正确对齐，
    // 并且属于同一个 allocation。
    unsafe {
        let v_a = v_base.add(a_pos);
        let v_b = v_base.add(b_pos);

        // PANIC SAFETY: 如果 is_less panic，这里尚未创建 scratch 内存，
        // 切片仍应处于定义良好的状态，且没有重复元素。

        // 重要的是，仅当左侧更大时才交换，相等时不交换。is_less 对相等元素应返回 false，
        // 因此不会交换。
        let should_swap = is_less(&*v_b, &*v_a);

        // 这是 swap-if 的无分支版本。带分支的等价代码是：
        //
        // if should_swap {
        //     ptr::swap(v_a, v_b, 1);
        // }

        // 目标是在这里生成 cmov 指令。
        let v_a_swap = hint::select_unpredictable(should_swap, v_b, v_a);
        let v_b_swap = hint::select_unpredictable(should_swap, v_a, v_b);

        let v_b_swap_tmp = ManuallyDrop::new(ptr::read(v_b_swap));
        ptr::copy(v_a_swap, v_a, 1);
        ptr::copy_nonoverlapping(&*v_b_swap_tmp, v_b, 1);
    }
}

/// 使用快速固定函数排序 `v` 的前 9 个元素。
///
/// 如果 `is_less` 生成大量代码，编译器可以选择不内联 `swap_if_less`。
/// 如果某个排序实现的代码变化导致多处调用本函数，建议使用 `#[inline(never)]`
/// 来控制二进制体积。`small_sort_network` 当前设计保证只调用它一次。
fn sort9_optimal<T, F>(v: &mut [T], is_less: &mut F)
where
    F: FnMut(&T, &T) -> bool,
{
    if v.len() < 9 {
        intrinsics::abort();
    }

    let v_base = v.as_mut_ptr();

    // 最优 sorting network 见：
    // 参考：https://bertdobbelaere.github.io/sorting_networks.html.

    // SAFETY: 已检查 len。
    unsafe {
        swap_if_less(v_base, 0, 3, is_less);
        swap_if_less(v_base, 1, 7, is_less);
        swap_if_less(v_base, 2, 5, is_less);
        swap_if_less(v_base, 4, 8, is_less);
        swap_if_less(v_base, 0, 7, is_less);
        swap_if_less(v_base, 2, 4, is_less);
        swap_if_less(v_base, 3, 8, is_less);
        swap_if_less(v_base, 5, 6, is_less);
        swap_if_less(v_base, 0, 2, is_less);
        swap_if_less(v_base, 1, 3, is_less);
        swap_if_less(v_base, 4, 5, is_less);
        swap_if_less(v_base, 7, 8, is_less);
        swap_if_less(v_base, 1, 4, is_less);
        swap_if_less(v_base, 3, 6, is_less);
        swap_if_less(v_base, 5, 7, is_less);
        swap_if_less(v_base, 0, 1, is_less);
        swap_if_less(v_base, 2, 4, is_less);
        swap_if_less(v_base, 3, 5, is_less);
        swap_if_less(v_base, 6, 8, is_less);
        swap_if_less(v_base, 2, 3, is_less);
        swap_if_less(v_base, 4, 5, is_less);
        swap_if_less(v_base, 6, 7, is_less);
        swap_if_less(v_base, 1, 2, is_less);
        swap_if_less(v_base, 3, 4, is_less);
        swap_if_less(v_base, 5, 6, is_less);
    }
}

/// 使用快速固定函数排序 `v` 的前 13 个元素。
///
/// 如果 `is_less` 生成大量代码，编译器可以选择不内联 `swap_if_less`。
/// 如果某个排序实现的代码变化导致多处调用本函数，建议使用 `#[inline(never)]`
/// 来控制二进制体积。`small_sort_network` 当前设计保证只调用它一次。
fn sort13_optimal<T, F>(v: &mut [T], is_less: &mut F)
where
    F: FnMut(&T, &T) -> bool,
{
    if v.len() < 13 {
        intrinsics::abort();
    }

    let v_base = v.as_mut_ptr();

    // 最优 sorting network 见：
    // 参考：https://bertdobbelaere.github.io/sorting_networks.html.

    // SAFETY: 已检查 len。
    unsafe {
        swap_if_less(v_base, 0, 12, is_less);
        swap_if_less(v_base, 1, 10, is_less);
        swap_if_less(v_base, 2, 9, is_less);
        swap_if_less(v_base, 3, 7, is_less);
        swap_if_less(v_base, 5, 11, is_less);
        swap_if_less(v_base, 6, 8, is_less);
        swap_if_less(v_base, 1, 6, is_less);
        swap_if_less(v_base, 2, 3, is_less);
        swap_if_less(v_base, 4, 11, is_less);
        swap_if_less(v_base, 7, 9, is_less);
        swap_if_less(v_base, 8, 10, is_less);
        swap_if_less(v_base, 0, 4, is_less);
        swap_if_less(v_base, 1, 2, is_less);
        swap_if_less(v_base, 3, 6, is_less);
        swap_if_less(v_base, 7, 8, is_less);
        swap_if_less(v_base, 9, 10, is_less);
        swap_if_less(v_base, 11, 12, is_less);
        swap_if_less(v_base, 4, 6, is_less);
        swap_if_less(v_base, 5, 9, is_less);
        swap_if_less(v_base, 8, 11, is_less);
        swap_if_less(v_base, 10, 12, is_less);
        swap_if_less(v_base, 0, 5, is_less);
        swap_if_less(v_base, 3, 8, is_less);
        swap_if_less(v_base, 4, 7, is_less);
        swap_if_less(v_base, 6, 11, is_less);
        swap_if_less(v_base, 9, 10, is_less);
        swap_if_less(v_base, 0, 1, is_less);
        swap_if_less(v_base, 2, 5, is_less);
        swap_if_less(v_base, 6, 9, is_less);
        swap_if_less(v_base, 7, 8, is_less);
        swap_if_less(v_base, 10, 11, is_less);
        swap_if_less(v_base, 1, 3, is_less);
        swap_if_less(v_base, 2, 4, is_less);
        swap_if_less(v_base, 5, 6, is_less);
        swap_if_less(v_base, 9, 10, is_less);
        swap_if_less(v_base, 1, 2, is_less);
        swap_if_less(v_base, 3, 4, is_less);
        swap_if_less(v_base, 5, 7, is_less);
        swap_if_less(v_base, 6, 8, is_less);
        swap_if_less(v_base, 2, 3, is_less);
        swap_if_less(v_base, 4, 5, is_less);
        swap_if_less(v_base, 6, 7, is_less);
        swap_if_less(v_base, 8, 9, is_less);
        swap_if_less(v_base, 3, 4, is_less);
        swap_if_less(v_base, 5, 6, is_less);
    }
}

/// 在假设 [begin, tail) 已排序的前提下，排序范围 [begin, tail]。
///
/// # 安全性(Safety）
/// 必须满足 begin < tail，并且对所有 begin <= p <= tail，p 都必须有效且已初始化。
unsafe fn insert_tail<T, F: FnMut(&T, &T) -> bool>(begin: *mut T, tail: *mut T, is_less: &mut F) {
    // SAFETY: 见各处局部注释。
    unsafe {
        // SAFETY: tail > begin，因此在边界内。
        let mut sift = tail.sub(1);
        if !is_less(&*tail, &*sift) {
            return;
        }

        // SAFETY: 这次读取之后不会再读取 tail，因为之后只会读取 sift；
        // sift < tail 且只会递减。因此这在语义上是移动而非复制。
        // 如果发生 panic，或者找到了正确插入位置，gap_guard 会确保该元素移回数组。
        let tmp = ManuallyDrop::new(tail.read());
        let mut gap_guard = CopyOnDrop { src: &*tmp, dst: tail, len: 1 };

        loop {
            // SAFETY: 把 sift 移入 gap（该 gap 有效），并把 gap guard 的目标指向 sift，
            // 确保如果发生 panic，gap 会再次被填上。
            ptr::copy_nonoverlapping(sift, gap_guard.dst, 1);
            gap_guard.dst = sift;

            if sift == begin {
                break;
            }

            // SAFETY: 已检查 sift != begin，因此这里在边界内。
            sift = sift.sub(1);
            if !is_less(&tmp, &*sift) {
                break;
            }
        }
    }
}

/// 在假设 `v[..offset]` 已排序的前提下排序 `v`。
pub fn insertion_sort_shift_left<T, F: FnMut(&T, &T) -> bool>(
    v: &mut [T],
    offset: usize,
    is_less: &mut F,
) {
    let len = v.len();
    if offset == 0 || offset > len {
        intrinsics::abort();
    }

    // SAFETY: 见各处局部注释。
    unsafe {
        // 直接用指针写这个基础循环；如果使用 for 循环，LLVM 喜欢展开它，
        // 而这里并不希望这样。
        // SAFETY: v_end 是末尾后一位指针，且已检查 offset <= len，
        // 因此 tail 也在边界内。
        let v_base = v.as_mut_ptr();
        let v_end = v_base.add(len);
        let mut tail = v_base.add(offset);
        while tail != v_end {
            // SAFETY: v_base 和 tail 都是指向元素的有效指针；
            // 已检查 offset != 0，因此 v_base < tail。
            insert_tail(v_base, tail, is_less);

            // SAFETY: 已检查 tail 还不是末尾后一位指针。
            tail = tail.add(1);
        }
    }
}

/// SAFETY: 调用方必须保证 `v_base` 对 4 次读取有效，`dst` 对 4 次写入有效。
/// 结果会存储在 `dst[0..4]` 中。
pub unsafe fn sort4_stable<T, F: FnMut(&T, &T) -> bool>(
    v_base: *const T,
    dst: *mut T,
    is_less: &mut F,
) {
    // 通过把 select 限制为选择指针，不论类型 T 的大小如何，都能保证良好的 cmov code-gen。
    // 此外，相比稳定 transposition 4 元素 sorting-network，这里只做 5 次而不是 6 次比较，
    // 并且始终恰好复制每个元素一次。

    // SAFETY: 所有指针相对 v_base 和 dst 的偏移最多为 3；
    // 根据前置条件，它们都在边界内。
    unsafe {
        // 稳定地创建两对元素：a <= b 和 c <= d。
        let c1 = is_less(&*v_base.add(1), &*v_base);
        let c2 = is_less(&*v_base.add(3), &*v_base.add(2));
        let a = v_base.add(c1 as usize);
        let b = v_base.add(!c1 as usize);
        let c = v_base.add(2 + c2 as usize);
        let d = v_base.add(2 + (!c2 as usize));

        // 比较 (a, c) 和 (b, d) 以识别 max/min。此时还剩两个未知元素；
        // 由于这是稳定排序，必须知道哪个在更左侧、哪个在更右侧。
        // c3, c4 | min max unknown_left unknown_right
        //  0,  0 |  a   d    b         c
        //  0,  1 |  a   b    c         d
        //  1,  0 |  c   d    a         b
        //  1,  1 |  c   b    a         d
        let c3 = is_less(&*c, &*a);
        let c4 = is_less(&*d, &*b);
        let min = hint::select_unpredictable(c3, c, a);
        let max = hint::select_unpredictable(c4, b, d);
        let unknown_left = hint::select_unpredictable(c3, a, hint::select_unpredictable(c4, c, b));
        let unknown_right = hint::select_unpredictable(c4, d, hint::select_unpredictable(c3, b, c));

        // 排序最后两个未知元素。
        let c5 = is_less(&*unknown_right, &*unknown_left);
        let lo = hint::select_unpredictable(c5, unknown_right, unknown_left);
        let hi = hint::select_unpredictable(c5, unknown_left, unknown_right);

        ptr::copy_nonoverlapping(min, dst, 1);
        ptr::copy_nonoverlapping(lo, dst.add(1), 1);
        ptr::copy_nonoverlapping(hi, dst.add(2), 1);
        ptr::copy_nonoverlapping(max, dst.add(3), 1);
    }
}

/// SAFETY: 调用方必须保证 `v_base` 对 8 次读写有效，
/// `scratch_base` 和 `dst` 对 8 次写入有效。结果会存储在 `dst[0..8]` 中。
unsafe fn sort8_stable<T: FreezeMarker, F: FnMut(&T, &T) -> bool>(
    v_base: *mut T,
    dst: *mut T,
    scratch_base: *mut T,
    is_less: &mut F,
) {
    // SAFETY: 根据本函数前置条件，这些指针都在边界内。
    unsafe {
        sort4_stable(v_base, scratch_base, is_less);
        sort4_stable(v_base.add(4), scratch_base.add(4), is_less);
    }

    // SAFETY: scratch_base[0..8] 现在已经初始化，因此可合并回 dst。
    unsafe {
        bidirectional_merge(&*ptr::slice_from_raw_parts(scratch_base, 8), dst, is_less);
    }
}

#[inline(always)]
unsafe fn merge_up<T, F: FnMut(&T, &T) -> bool>(
    mut left_src: *const T,
    mut right_src: *const T,
    mut dst: *mut T,
    is_less: &mut F,
) -> (*const T, *const T, *mut T) {
    // 这是无分支 merge 辅助函数。带分支的等价代码是：
    //
    // if !is_less(&*right_src, &*left_src) {
    //     ptr::copy_nonoverlapping(left_src, dst, 1);
    //     left_src = left_src.add(1);
    // } else {
    //     ptr::copy_nonoverlapping(right_src, dst, 1);
    //     right_src = right_src.add(1);
    // }
    // dst = dst.add(1);

    // SAFETY: 调用方必须保证 `left_src`、`right_src` 可读，`dst` 可写，
    // 并且三者不 alias。
    unsafe {
        let is_l = !is_less(&*right_src, &*left_src);
        let src = if is_l { left_src } else { right_src };
        ptr::copy_nonoverlapping(src, dst, 1);
        right_src = right_src.add(!is_l as usize);
        left_src = left_src.add(is_l as usize);
        dst = dst.add(1);
    }

    (left_src, right_src, dst)
}

#[inline(always)]
unsafe fn merge_down<T, F: FnMut(&T, &T) -> bool>(
    mut left_src: *const T,
    mut right_src: *const T,
    mut dst: *mut T,
    is_less: &mut F,
) -> (*const T, *const T, *mut T) {
    // 这是无分支 merge 辅助函数。带分支的等价代码是：
    //
    // if !is_less(&*right_src, &*left_src) {
    //     ptr::copy_nonoverlapping(right_src, dst, 1);
    //     right_src = right_src.wrapping_sub(1);
    // } else {
    //     ptr::copy_nonoverlapping(left_src, dst, 1);
    //     left_src = left_src.wrapping_sub(1);
    // }
    // dst = dst.sub(1);

    // SAFETY: 调用方必须保证 `left_src`、`right_src` 可读，`dst` 可写，
    // 并且三者不 alias。
    unsafe {
        let is_l = !is_less(&*right_src, &*left_src);
        let src = if is_l { right_src } else { left_src };
        ptr::copy_nonoverlapping(src, dst, 1);
        right_src = right_src.wrapping_sub(is_l as usize);
        left_src = left_src.wrapping_sub(!is_l as usize);
        dst = dst.sub(1);
    }

    (left_src, right_src, dst)
}

/// 在假设 v[..len / 2] 和 v[len / 2..] 已排序的前提下合并 v。
///
/// 双向合并的原始思路来自 Igor van den Hoven（quadsort），这里改写为只使用
/// merge up 和 merge down。与原始 parity_merge 函数相比，它每次迭代执行 2 次写入，
/// 而不是 4 次。
///
/// # 安全性(Safety）
/// 调用方必须保证 `dst` 对 v.len() 次写入有效。
/// 同时，`v.as_ptr()` 和 `dst` 不能 alias，且 v.len() 必须 >= 2。
///
/// 注意 T 必须是 Freeze；比较函数会在过时的临时“副本”上求值，
/// 这些副本不一定最终出现在数组中。
unsafe fn bidirectional_merge<T: FreezeMarker, F: FnMut(&T, &T) -> bool>(
    v: &[T],
    dst: *mut T,
    is_less: &mut F,
) {
    // 把合并过程画出来更容易理解：
    //
    // 初始状态：
    //
    //  |dst (in dst)
    //  |left               |right
    //  v                   v
    // [xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx]
    //                     ^                   ^
    //                     |left_rev           |right_rev
    //                                         |dst_rev (in dst)
    //
    // 之后：
    //
    //                      |dst (in dst)
    //        |left         |           |right
    //        v             v           v
    // [xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx]
    //       ^             ^           ^
    //       |left_rev     |           |right_rev
    //                     |dst_rev (in dst)
    //
    // 每次迭代中，left 或 right 之一向上移动一个位置，left_rev 或 right_rev
    // 之一向下移动一个位置；而 dst 始终向上移动一个位置，dst_rev 始终向下移动一个位置。
    // 假设输入已排序且比较函数实现正确，最终会有 left == left_rev + 1
    // 且 right == right_rev + 1，表示输入被完整消耗并写入 dst。

    let len = v.len();
    let src = v.as_ptr();

    let len_div_2 = len / 2;

    // SAFETY: 调用方必须保证 len >= 2。
    unsafe {
        intrinsics::assume(len_div_2 != 0); // 这可以避免无用的 code-gen。
    }

    // SAFETY: 无论用户提供的比较函数返回什么结果，4 个读取指针都会始终在边界内。
    // 如果调用方保证 `dst` 对 `v.len()` 次写入有效，则写入 `dst` 和 `dst_rev`
    // 也始终在边界内。
    unsafe {
        let mut left = src;
        let mut right = src.add(len_div_2);
        let mut dst = dst;

        let mut left_rev = src.add(len_div_2 - 1);
        let mut right_rev = src.add(len - 1);
        let mut dst_rev = dst.add(len - 1);

        for _ in 0..len_div_2 {
            (left, right, dst) = merge_up(left, right, dst, is_less);
            (left_rev, right_rev, dst_rev) = merge_down(left_rev, right_rev, dst_rev, is_less);
        }

        let left_end = left_rev.wrapping_add(1);
        let right_end = right_rev.wrapping_add(1);

        // 长度为奇数，因此输入中会剩下一个尚未消耗的元素。
        if !len.is_multiple_of(2) {
            let left_nonempty = left < left_end;
            let last_src = if left_nonempty { left } else { right };
            ptr::copy_nonoverlapping(last_src, dst, 1);
            left = left.add(left_nonempty as usize);
            right = right.add((!left_nonempty) as usize);
        }

        // 现在应当已经恰好完整消耗了一次输入。只有当用户提供的比较函数没有实现
        // strict weak ordering 时，这才可能失败。在这种情况下会 panic，
        // 且不会访问 dst 中的不一致状态。
        if left != left_end || right != right_end {
            panic_on_ord_violation();
        }
    }
}

#[cfg_attr(not(panic = "immediate-abort"), inline(never), cold)]
#[cfg_attr(panic = "immediate-abort", inline)]
fn panic_on_ord_violation() -> ! {
    // 这表示用户提供的比较函数或 Ord 实现中存在逻辑错误。
    // 如 Ord 文档所述，它们应当实现全序。
    //
    // 通过 panic，可以告知用户其程序中存在逻辑错误。如果没有 strict weak ordering，
    // 基于比较的排序这个概念无法产生已排序结果。例如：a < b < c < a。
    //
    // Ord 文档要求用户实现全序。就排序语境而言，这可以说过于严格；
    // 只有当更弱的 strict weak ordering 要求被违反时，问题才会出现。
    //
    // panic 消息提到 total order，是因为 Ord 文档讨论并要求的就是它，
    // 这样可以避免让用户困惑。
    panic!("user-provided comparison function does not correctly implement a total order");
}

#[must_use]
pub(crate) const fn has_efficient_in_place_swap<T>() -> bool {
    // 该启发式在所有已测试的 64-bit capable 架构上都成立。
    size_of::<T>() <= 8 // size_of::<u64>()
}
