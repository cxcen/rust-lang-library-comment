//! 本模块包含 `slice::sort` 的内部入口。

#[cfg(not(any(feature = "optimize_for_size", target_pointer_width = "16")))]
use crate::cmp;
use crate::mem::{MaybeUninit, SizedTypeProperties};
#[cfg(not(any(feature = "optimize_for_size", target_pointer_width = "16")))]
use crate::slice::sort::shared::smallsort::{
    SMALL_SORT_GENERAL_SCRATCH_LEN, StableSmallSortTypeImpl, insertion_sort_shift_left,
};
use crate::{cfg_select, intrinsics};

pub(crate) mod merge;

#[cfg(not(any(feature = "optimize_for_size", target_pointer_width = "16")))]
pub(crate) mod drift;
#[cfg(not(any(feature = "optimize_for_size", target_pointer_width = "16")))]
pub(crate) mod quicksort;

#[cfg(any(feature = "optimize_for_size", target_pointer_width = "16"))]
pub(crate) mod tiny;

/// Orson Peters 和 Lukas Bergdoll 称为 driftsort 的稳定排序。
/// 设计文档：
/// <https://github.com/Voultapher/sort-research-rs/blob/main/writeup/driftsort_introduction/text.md>
///
/// 维护下列文档中列出的全部安全性质：
/// <https://github.com/Voultapher/sort-research-rs/blob/main/writeup/sort_safety/text.md>
#[inline(always)]
pub fn sort<T, F: FnMut(&T, &T) -> bool, BufT: BufGuard<T>>(v: &mut [T], is_less: &mut F) {
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
            // 与 driftsort 不同，mergesort 只需要 len / 2，而不是 len - len / 2。
            let alloc_len = len / 2;

            cfg_select! {
                target_pointer_width = "16" => {
                    let mut heap_buf = BufT::with_capacity(alloc_len);
                    let scratch = heap_buf.as_uninit_slice_mut();
                }
                _ => {
                    // 对小输入，4KiB 栈存储已经足够，可避免调用分配/释放器。
                    // 基准测试显示这很有收益。
                    let mut stack_buf = AlignedStorage::<T, 4096>::new();
                    let stack_scratch = stack_buf.as_uninit_slice_mut();
                    let mut heap_buf;
                    let scratch = if stack_scratch.len() >= alloc_len {
                        stack_scratch
                    } else {
                        heap_buf = BufT::with_capacity(alloc_len);
                        heap_buf.as_uninit_slice_mut()
                    };
                }
            }

            tiny::mergesort(v, scratch, is_less);
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

            driftsort_main::<T, F, BufT>(v, is_less);
        }
    }
}

/// 见 [`sort`]。
///
/// 故意不内联主排序例程入口，以确保内联插入排序的 i-cache 占用保持最小。
#[cfg(not(any(feature = "optimize_for_size", target_pointer_width = "16")))]
#[inline(never)]
fn driftsort_main<T, F: FnMut(&T, &T) -> bool, BufT: BufGuard<T>>(v: &mut [T], is_less: &mut F) {
    // 分配 n 个元素的临时内存可保证整个输入能用 stable quicksort 排序，这对随机分布和
    // 低基数分布性能更好。不过对大输入，我们仍希望把内存用量降到 n - n / 2。
    // 因此按 max(n - n / 2, min(n, 8MB)) 缩放分配：小输入按 n 缩放，大输入按
    // n - n / 2 缩放，避免突然下降。还必须保证分配量 >= SMALL_SORT_GENERAL_SCRATCH_LEN，
    // 因为 small-sort 总是需要这么多临时内存。
    //
    // driftsort 会产生长度最高为 min_good_run_len 的未排序 run；该值至多是 len - len / 2。
    // 未排序 run 需要由 quicksort 处理，而 quicksort 需要与 run 长度相同的 scratch 空间，
    // 因此 scratch 至少必须为 len - len / 2。如果以后修改 min_good_run_len，
    // 这里也必须同步更新以分配正确的 scratch 大小。
    const MAX_FULL_ALLOC_BYTES: usize = 8_000_000; // 8MB
    let max_full_alloc = MAX_FULL_ALLOC_BYTES / size_of::<T>();
    let len = v.len();
    let alloc_len = cmp::max(
        cmp::max(len - len / 2, cmp::min(len, max_full_alloc)),
        SMALL_SORT_GENERAL_SCRATCH_LEN,
    );

    // 对小输入，4KiB 栈存储已经足够，可避免调用分配/释放器。基准测试显示这很有收益。
    let mut stack_buf = AlignedStorage::<T, 4096>::new();
    let stack_scratch = stack_buf.as_uninit_slice_mut();
    let mut heap_buf;
    let scratch = if stack_scratch.len() >= alloc_len {
        stack_scratch
    } else {
        heap_buf = BufT::with_capacity(alloc_len);
        heap_buf.as_uninit_slice_mut()
    };

    // 对小输入，使用 quicksort 还不划算；单次 small-sort 或两次 small-sort 加一次 merge
    // 表现更好，因此使用 eager mode。
    let eager_sort = len <= T::small_sort_threshold() * 2;
    crate::slice::sort::stable::drift::sort(v, scratch, eager_sort, is_less);
}

#[doc(hidden)]
/// 抽象拥有所有权的内存缓冲区，使排序代码可以留在无法分配的 core 中。
/// 有分配能力的地方再实现该 trait。
pub trait BufGuard<T> {
    /// 创建至少能容纳 `capacity` 个元素的缓冲区。
    fn with_capacity(capacity: usize) -> Self;
    /// 返回该缓冲区拥有的未初始化内存的可变访问。
    fn as_uninit_slice_mut(&mut self) -> &mut [MaybeUninit<T>];
}

#[repr(C)]
struct AlignedStorage<T, const N: usize> {
    _align: [T; 0],
    storage: [MaybeUninit<u8>; N],
}

impl<T, const N: usize> AlignedStorage<T, N> {
    fn new() -> Self {
        Self { _align: [], storage: [const { MaybeUninit::uninit() }; N] }
    }

    fn as_uninit_slice_mut(&mut self) -> &mut [MaybeUninit<T>] {
        let len = N / size_of::<T>();

        // SAFETY: `_align` 保证存储对 `T` 正确对齐。
        unsafe { core::slice::from_raw_parts_mut(self.storage.as_mut_ptr().cast(), len) }
    }
}
