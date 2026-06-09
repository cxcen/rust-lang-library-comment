//! 本模块包含把自底向上的 Mergesort 与自顶向下的 Quicksort 结合起来的顶层混合循环。

use crate::mem::MaybeUninit;
use crate::slice::sort::shared::find_existing_run;
use crate::slice::sort::shared::smallsort::StableSmallSortTypeImpl;
use crate::slice::sort::stable::merge::merge;
use crate::slice::sort::stable::quicksort::quicksort;
use crate::{cmp, intrinsics};

/// 根据比较函数 `is_less` 排序 `v`。
///
/// 如果 `eager_sort` 为 true，它只会执行 small-sort 和实际 merge，
/// 从而保证最坏情况复杂度为 O(N * log(N))。`scratch.len()` 必须至少为
/// `max(v.len() - v.len() / 2, SMALL_SORT_GENERAL_SCRATCH_LEN)`，
/// 否则实现可能会 abort。完全升序和完全降序的输入会用恰好 N - 1 次比较完成排序。
///
/// 这是 driftsort 的主循环，它使用 powersort 的启发式来决定 run 的合并顺序；
/// 细节见下方说明。
pub fn sort<T, F: FnMut(&T, &T) -> bool>(
    v: &mut [T],
    scratch: &mut [MaybeUninit<T>],
    eager_sort: bool,
    is_less: &mut F,
) {
    let len = v.len();
    if len < 2 {
        return; // 移除这个长度检查会增加代码体积。
    }
    let scale_factor = merge_tree_scale_factor(len);

    // 预排序 run 的进入门槛需要相对较高：只要存在一个这样的 run，
    // 平均就会强制触发数次 merge，并显著缩小 quicksort 可处理的最大规模。
    // 因此这里使用 sqrt(len) 作为预排序 run 阈值。
    const MIN_SQRT_RUN_LEN: usize = 64;
    let min_good_run_len = if len <= (MIN_SQRT_RUN_LEN * MIN_SQRT_RUN_LEN) {
        // 对较小输入长度，`MIN_SQRT_RUN_LEN` 会破坏完全有序或近乎有序输入的模式检测。
        cmp::min(len - len / 2, MIN_SQRT_RUN_LEN)
    } else {
        sqrt_approx(len)
    };

    // (stack_len, runs, desired_depths) 共同组成一个栈，用来维护 powersort
    // 启发式所需的 run 信息。desired_depths[i] 是合并节点的期望深度；
    // 该合并节点负责合并 runs[i] 与紧随其后的 run。
    let mut stack_len = 0;
    let mut run_storage = MaybeUninit::<[DriftsortRun; 66]>::uninit();
    let runs: *mut DriftsortRun = run_storage.as_mut_ptr().cast();
    let mut desired_depth_storage = MaybeUninit::<[u8; 66]>::uninit();
    let desired_depths: *mut u8 = desired_depth_storage.as_mut_ptr().cast();

    let mut scan_idx = 0;
    let mut prev_run = DriftsortRun::new_sorted(0); // 初始 dummy run。
    loop {
        // 计算下一个 run，以及 prev_run 与 next_run 之间合并节点的期望深度。
        // 在最后一次迭代中，创建一个具有根级期望深度的 dummy run，
        // 以便完全折叠合并树。
        let (next_run, desired_depth);
        if scan_idx < len {
            next_run =
                create_run(&mut v[scan_idx..], scratch, min_good_run_len, eager_sort, is_less);
            desired_depth = merge_tree_depth(
                scan_idx - prev_run.len(),
                scan_idx,
                scan_idx + next_run.len(),
                scale_factor,
            );
        } else {
            next_run = DriftsortRun::new_sorted(0);
            desired_depth = 0;
        };

        // 处理较早的 runs[i] 之间的合并节点：这些节点希望在合并树中位于比
        // prev_run 与 next_run 的分割点对应节点更深的位置。
        //
        // SAFETY: 首先注意，这是唯一会修改 stack_len、runs 或 desired_depths
        // 的地方。这里维护以下不变量：
        //  1. runs/desired_depths 的前 stack_len 个元素已经初始化。
        //  2. 对所有有效 i > 0，desired_depths[i] < desired_depths[i+1]。
        //  3. 所有有效 runs[i].len() 之和加上 prev_run.len() 等于 scan_idx。
        unsafe {
            while stack_len > 1 && *desired_depths.add(stack_len - 1) >= desired_depth {
                // 期望深度大于即将到来的期望深度；从栈中弹出左邻 run，
                // 并将其合并到 prev_run。
                let left = *runs.add(stack_len - 1);
                let merged_len = left.len() + prev_run.len();
                let merge_start_idx = scan_idx - merged_len;
                let merge_slice = v.get_unchecked_mut(merge_start_idx..scan_idx);
                prev_run = logical_merge(merge_slice, scratch, left, prev_run, is_less);
                stack_len -= 1;
            }

            // 现在已知 desired_depths[stack_len - 1] < desired_depth，
            // 因而维持了不变量。这也保证不会让栈溢出：merge_tree_depth(..) <= 64，
            // 因此 push 之前栈上最多有 64 个不同值，再加上初始 dummy run；
            // 而容量是 66。
            *runs.add(stack_len) = prev_run;
            *desired_depths.add(stack_len) = desired_depth;
            stack_len += 1;
        }

        // 在用 dummy run 覆盖最后一个 run 之前退出。
        if scan_idx >= len {
            break;
        }

        scan_idx += next_run.len();
        prev_run = next_run;
    }

    if !prev_run.sorted() {
        stable_quicksort(v, scratch, is_less);
    }
}

// 参考 J. Ian Munro 和 Sebastian Wild 的论文：
// 论文题为《Nearly-Optimal Mergesorts: Fast, Practical Sorting Methods That Optimally
// 标题续：Adapt to Existing Runs》。
//
// 该方法形成一棵二叉合并树，其中每个内部节点对应需要合并的相邻 run
// 之间的一个分割点。如果把数组可视化为从 0 到 1 的数轴，我们希望找到
// 位于待合并切片中点之间、分母最小的 dyadic fraction。dyadic fraction
// 中的指数表示该内部节点希望在二叉合并树中拥有的期望深度。
// 由于 run 本身天然可能不平衡，这不总是等于实际深度，但算法会尽量贴近它。
//
// 作为优化，把数轴从 [0, 1) 重新缩放到 [0, 2^62)。这样，寻找两个中点之间
// 最简单的 dyadic fraction，就对应寻找两个中点最高有效位上的差异。
// 保存 scale_factor = ceil(2^62 / n)，即可用乘法执行这个缩放，
// 避免重复做整数除法。当 n 不是 2 的幂时，由于这里使用整数而非实数，
// 这个缩放并不精确；但结果非常接近。事实上，当 n < 2^30 时，
// 近似误差完全落在低位，得到的树是等价的。
//
// 因此，对两个相邻切片 [a, b) 与 [b, c) 之间的分割点，对应合并节点的
// 期望深度是 CLZ((a+b)*f ^ (b+c)*f)，其中 CLZ 统计整数前导零数量，
// f 是 scale factor。注意，中点计算中省略了除以二，因为那只会把位移动一位
// （从而总是给结果加一）；这里关心的只是相对深度。
//
// 最后，若尝试对 x = (a+b)*f 求上界，可得 x = (n-1 + n) * ceil(2^62 / n)，于是
//    x < (2^62 / n + 1) * 2n
//    x < 2^63 + 2n
// 因此只要 n < 2^62，就有 x < 2^64，意味着这些运算不会溢出。
#[inline(always)]
fn merge_tree_scale_factor(n: usize) -> u64 {
    if usize::BITS > u64::BITS {
        panic!("Platform not supported");
    }

    (1u64 << 62).div_ceil(n as u64)
}

// 注意：当 left < right 时，merge_tree_depth 的输出小于 64，因为 f*x 与 f*y
// 必定在某一位上不同；它始终小于等于 64。
#[inline(always)]
fn merge_tree_depth(left: usize, mid: usize, right: usize, scale_factor: u64) -> u8 {
    let x = left as u64 + mid as u64;
    let y = mid as u64 + right as u64;
    ((scale_factor * x) ^ (scale_factor * y)).leading_zeros() as u8
}

fn sqrt_approx(n: usize) -> usize {
    // 注意 sqrt(n) = n^(1/2)，且 2^log2(n) = n。结合这两个事实，
    // 可把 sqrt(n) 近似为 2^(log2(n) / 2)。由于整数 log 会向下取整，
    // 这里希望平均补偿 0.5，因此初始近似是 2^((1 + floor(log2(n))) / 2)。
    //
    // 随后应用一次 Newton 方法迭代来改进近似；对 sqrt(n)，公式为
    // a1 = (a0 + n / a0) / 2。
    //
    // 最后，指数和除法可直接用移位完成。这里 OR 1 是为了避免整数 log 中的零检查。
    let ilog = (n | 1).ilog2();
    let shift = ilog.div_ceil(2);
    ((1 << shift) + (n >> shift)) / 2
}

// 与 Glidesort 中相同的惰性逻辑 run。
#[inline(always)]
fn logical_merge<T, F: FnMut(&T, &T) -> bool>(
    v: &mut [T],
    scratch: &mut [MaybeUninit<T>],
    left: DriftsortRun,
    right: DriftsortRun,
    is_less: &mut F,
) -> DriftsortRun {
    // 如果一个或两个 run 已经排序，则执行实际 merge；若存在未排序 run，
    // 则先用 quicksort 排序它。如果合并后的 run 已经无法放入 scratch 空间，
    // 也*必须*执行实际 merge，因为这意味着之后无法再对它们运行 quicksort。
    let len = v.len();
    let can_fit_in_scratch = len <= scratch.len();
    if !can_fit_in_scratch || left.sorted() || right.sorted() {
        if !left.sorted() {
            stable_quicksort(&mut v[..left.len()], scratch, is_less);
        }
        if !right.sorted() {
            stable_quicksort(&mut v[left.len()..], scratch, is_less);
        }
        merge(v, scratch, left.len(), is_less);

        DriftsortRun::new_sorted(len)
    } else {
        DriftsortRun::new_unsorted(len)
    }
}

/// 创建新的逻辑 run。
///
/// 逻辑 run 可以是已排序或未排序。如果存在一个已满足 `min_good_run_len`
/// 阈值的现有 run，就把它作为已排序 run 返回。否则，结果取决于 `eager_sort`：
/// 如果为 true，返回长度为 `T::SMALL_SORT_THRESHOLD` 的已排序 run；
/// 如果为 false，返回长度为 `min_good_run_len` 的未排序 run。
fn create_run<T, F: FnMut(&T, &T) -> bool>(
    v: &mut [T],
    scratch: &mut [MaybeUninit<T>],
    min_good_run_len: usize,
    eager_sort: bool,
    is_less: &mut F,
) -> DriftsortRun {
    let len = v.len();
    if len >= min_good_run_len {
        let (run_len, was_reversed) = find_existing_run(v, is_less);

        // SAFETY: find_existing_run 承诺返回有效的 run_len。
        unsafe { intrinsics::assume(run_len <= len) };

        if run_len >= min_good_run_len {
            if was_reversed {
                v[..run_len].reverse();
            }

            return DriftsortRun::new_sorted(run_len);
        }
    }

    if eager_sort {
        // 这里调用 quicksort，并传入会立即触发 small-sort 的长度。
        // 不在这里直接调用 small-sort，可让它始终内联进 quicksort 自身，
        // 使递归基准情形更快，通常也更节省二进制体积。
        let eager_run_len = cmp::min(T::small_sort_threshold(), len);
        quicksort(&mut v[..eager_run_len], scratch, 0, None, is_less);
        DriftsortRun::new_sorted(eager_run_len)
    } else {
        DriftsortRun::new_unsorted(cmp::min(min_good_run_len, len))
    }
}

fn stable_quicksort<T, F: FnMut(&T, &T) -> bool>(
    v: &mut [T],
    scratch: &mut [MaybeUninit<T>],
    is_less: &mut F,
) {
    // 将不平衡分区次数限制为 `2 * floor(log2(len))`。
    // 按位 OR 1 用于消除对数中的零检查。
    let limit = 2 * (v.len() | 1).ilog2();
    quicksort(v, scratch, limit, None, is_less);
}

/// 紧凑存储 run 的长度以及它是否已排序。
/// 由于切片最大长度为 [`isize::MAX`]，这些信息总能放入一个 `usize`。
#[derive(Copy, Clone)]
struct DriftsortRun(usize);

impl DriftsortRun {
    #[inline(always)]
    fn new_sorted(length: usize) -> Self {
        Self((length << 1) | 1)
    }

    #[inline(always)]
    fn new_unsorted(length: usize) -> Self {
        Self(length << 1)
    }

    #[inline(always)]
    fn sorted(self) -> bool {
        self.0 & 1 == 1
    }

    #[inline(always)]
    fn len(self) -> usize {
        self.0 >> 1
    }
}
