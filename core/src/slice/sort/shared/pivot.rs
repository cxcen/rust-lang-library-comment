//! 本模块包含枢轴选择逻辑。

use crate::{hint, intrinsics};

// 超过此阈值时递归选择伪中位数。
const PSEUDO_MEDIAN_REC_THRESHOLD: usize = 64;

/// 从 `v` 中选择一个枢轴。算法取自 Orson Peters 的 glidesort。
///
/// 它通过自适应数量的采样点选择枢轴，近似达到从 sqrt(n) 个元素中取中位数的质量。
#[inline]
pub fn choose_pivot<T, F: FnMut(&T, &T) -> bool>(v: &[T], is_less: &mut F) -> usize {
    // 这里使用 unsafe 代码和裸指针，因为该逻辑涉及较深递归。
    // 在递归中传递安全切片会带来大量分支和函数调用开销。

    let len = v.len();
    if len < 8 {
        intrinsics::abort();
    }

    // SAFETY: a、b、c 都指向包含 len_div_8 个元素的已初始化区域；
    // v_base 指向 n = len 个元素的已初始化区域，因此满足 median3
    // 和 median3_rec 的前置条件。
    let index = unsafe {
        let v_base = v.as_ptr();
        let len_div_8 = len / 8;

        let a = v_base; // [0, floor(n/8))
        let b = v_base.add(len_div_8 * 4); // [4*floor(n/8), 5*floor(n/8))
        let c = v_base.add(len_div_8 * 7); // [7*floor(n/8), 8*floor(n/8))

        if len < PSEUDO_MEDIAN_REC_THRESHOLD {
            median3(&*a, &*b, &*c, is_less).offset_from_unsigned(v_base)
        } else {
            median3_rec(a, b, c, len_div_8, is_less).offset_from_unsigned(v_base)
        }
    };
    // SAFETY: 前面必须已经满足 offset_from_unsigned() 的前置条件。
    unsafe {
        hint::assert_unchecked(index < v.len());
        index
    }
}

/// 从 a、b、c 三段中计算 3 个元素的近似中位数；如果这些段足够大，
/// 就先递归计算每段的近似值。
///
/// 每次递归把每段大小除以 8，因此递归深度为对数级；总体采样量满足
/// f(n) = 3*f(n/8)，即 f(n) = O(n^(log(3)/log(8))) ~= O(n^0.528) 个元素。
///
/// SAFETY: a、b、c 必须指向至少包含 n 个元素的已初始化内存区域开头。
unsafe fn median3_rec<T, F: FnMut(&T, &T) -> bool>(
    mut a: *const T,
    mut b: *const T,
    mut c: *const T,
    n: usize,
    is_less: &mut F,
) -> *const T {
    // SAFETY: 与 choose_pivot 中完全相同的推理可知，a、b、c 仍然指向
    // 包含 n / 8 个元素的已初始化区域。
    unsafe {
        if n * 8 >= PSEUDO_MEDIAN_REC_THRESHOLD {
            let n8 = n / 8;
            a = median3_rec(a, a.add(n8 * 4), a.add(n8 * 7), n8, is_less);
            b = median3_rec(b, b.add(n8 * 4), b.add(n8 * 7), n8, is_less);
            c = median3_rec(c, c.add(n8 * 4), c.add(n8 * 7), n8, is_less);
        }
        median3(&*a, &*b, &*c, is_less)
    }
}

/// 计算 3 个元素的中位数。
///
/// SAFETY: a、b、c 必须是有效且已初始化的元素。
#[inline(always)]
fn median3<T, F: FnMut(&T, &T) -> bool>(a: &T, b: &T, c: &T, is_less: &mut F) -> *const T {
    // 编译器通常会在合适时把这里变成无分支代码；否则也会避免第三次比较。
    let x = is_less(a, b);
    let y = is_less(a, c);
    if x == y {
        // 如果 x=y=0，则 b、c <= a，此时要返回 max(b, c)。
        // 如果 x=y=1，则 a < b、c，此时要返回 min(b, c)。
        // 用 XOR x 翻转 b < c 的结果即可得到这种行为。
        let z = is_less(b, c);
        if z ^ x { c } else { b }
    } else {
        // 要么 c <= a < b，要么 b <= a < c，因此 a 是中位数。
        a
    }
}
