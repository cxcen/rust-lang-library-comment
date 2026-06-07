//! `[T]` 的比较 trait 实现。
//!
//! 切片比较采用字典序：先逐元素比较，遇到第一个不相等元素就决定顺序；若公共前缀相等，
//! 则较短切片更小。部分实现会在元素类型允许时退化为按字节比较以获得 `memcmp` 级性能。

use super::{from_raw_parts, memchr};
use crate::ascii;
use crate::cmp::{self, BytewiseEq, Ordering};
use crate::intrinsics::compare_bytes;
use crate::num::NonZero;
use crate::ops::ControlFlow;

#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
impl<T, U> const PartialEq<[U]> for [T]
where
    T: [const] PartialEq<U>,
{
    #[inline]
    fn eq(&self, other: &[U]) -> bool {
        SlicePartialEq::equal(self, other)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
impl<T: [const] Eq> const Eq for [T] {}

/// 按 [lexicographically](Ord#lexicographical-comparison) 语义实现切片比较。
#[stable(feature = "rust1", since = "1.0.0")]
impl<T: Ord> Ord for [T] {
    fn cmp(&self, other: &[T]) -> Ordering {
        SliceOrd::compare(self, other)
    }
}

#[inline]
const fn as_underlying(x: ControlFlow<bool>) -> u8 {
    // SAFETY: 只有 `bool` 与 `ControlFlow<bool>` 大小相同时这里才会编译（这不是语言保证，
    // 但这里是 libcore）。大小相同意味着它使用 niche 表示；在一个字节里不会存在未初始化内存。
    // 调用方只检查这里得到的 `0` 或 `1`，它们必然对应 `Break` 变体；无论 `Continue(())`
    // 最终选用哪个值表示，都不会影响结果。
    unsafe { crate::mem::transmute(x) }
}

/// 按 [lexicographically](Ord#lexicographical-comparison) 语义实现切片部分比较。
#[stable(feature = "rust1", since = "1.0.0")]
impl<T: PartialOrd> PartialOrd for [T] {
    #[inline]
    fn partial_cmp(&self, other: &[T]) -> Option<Ordering> {
        SlicePartialOrd::partial_compare(self, other)
    }
    #[inline]
    fn lt(&self, other: &Self) -> bool {
        // 这显然不是实现这些方法的直观方式。不幸的是，只要使用会查看 discriminant 的写法，
        // LLVM 就会看到对 `2`（即 `ControlFlow<bool>::Continue(())`）的检查，并生成多余代码。
        // 等 LLVM 更聪明（见 <https://github.com/llvm/llvm-project/issues/132678>），或我们生成
        // 不会触发该问题的 niche discriminant 检查后，应改成更简单的实现。

        as_underlying(self.__chaining_lt(other)) == 1
    }
    #[inline]
    fn le(&self, other: &Self) -> bool {
        as_underlying(self.__chaining_le(other)) != 0
    }
    #[inline]
    fn gt(&self, other: &Self) -> bool {
        as_underlying(self.__chaining_gt(other)) == 1
    }
    #[inline]
    fn ge(&self, other: &Self) -> bool {
        as_underlying(self.__chaining_ge(other)) != 0
    }
    #[inline]
    fn __chaining_lt(&self, other: &Self) -> ControlFlow<bool> {
        SliceChain::chaining_lt(self, other)
    }
    #[inline]
    fn __chaining_le(&self, other: &Self) -> ControlFlow<bool> {
        SliceChain::chaining_le(self, other)
    }
    #[inline]
    fn __chaining_gt(&self, other: &Self) -> ControlFlow<bool> {
        SliceChain::chaining_gt(self, other)
    }
    #[inline]
    fn __chaining_ge(&self, other: &Self) -> ControlFlow<bool> {
        SliceChain::chaining_ge(self, other)
    }
}

#[doc(hidden)]
// 用于切片 PartialEq specialization 的中间 trait。
#[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
const trait SlicePartialEq<B> {
    fn equal(&self, other: &[B]) -> bool;
}

// 通用切片相等性。
#[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
impl<A, B> const SlicePartialEq<B> for [A]
where
    A: [const] PartialEq<B>,
{
    // 不值得尝试在 *MIR* 中内联下面的循环；阻止这里内联反而能促进上游更有用的内联，
    // 例如 `<str as PartialEq>::eq`。如有需要，codegen 后端之后仍可内联它。
    #[rustc_no_mir_inline]
    default fn equal(&self, other: &[B]) -> bool {
        if self.len() != other.len() {
            return false;
        }

        // 出于性能原因，这里使用显式索引，而不是 zip 迭代器。
        // 见 PR https://github.com/rust-lang/rust/pull/116846
        // FIXME(const_hack): 改成 `for idx in 0..self.len()` 循环。
        let mut idx = 0;
        while idx < self.len() {
            // 边界检查会被优化掉。
            if self[idx] != other[idx] {
                return false;
            }
            idx += 1;
        }

        true
    }
}

// 当每个元素都可按字节比较时，可以通过一次 intrinsic 调用比较整个字节区域。
#[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
impl<A, B> const SlicePartialEq<B> for [A]
where
    A: [const] BytewiseEq<B>,
{
    // 这通常是很好的后端内联候选，因为 intrinsic 往往只是 `memcmp`。不过截至 2025-12，
    // 让 MIR 内联这里会降低复用效果：例如 `String::eq` 会因此不再内联；阻止这里内联时，
    // 各层 wrapper 会持续内联，直到对本函数的调用消失。如果启发式未来改变且这不再有益，
    // 可以移除此限制。与此同时，不在 MIR 中内联是可以的，因为后端如果认为重要仍会内联。
    #[rustc_no_mir_inline]
    #[inline]
    fn equal(&self, other: &[B]) -> bool {
        if self.len() != other.len() {
            return false;
        }

        // SAFETY: `self` 和 `other` 都是引用，因此保证有效。上面已经检查两个切片大小相同。
        unsafe {
            let size = size_of_val(self);
            compare_bytes(self.as_ptr() as *const u8, other.as_ptr() as *const u8, size) == 0
        }
    }
}

#[doc(hidden)]
#[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
// 用于切片 PartialOrd specialization 的中间 trait。
const trait SlicePartialOrd: Sized {
    fn partial_compare(left: &[Self], right: &[Self]) -> Option<Ordering>;
}

#[doc(hidden)]
#[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
// 用于切片 PartialOrd 链式比较方法 specialization 的中间 trait。
const trait SliceChain: Sized {
    fn chaining_lt(left: &[Self], right: &[Self]) -> ControlFlow<bool>;
    fn chaining_le(left: &[Self], right: &[Self]) -> ControlFlow<bool>;
    fn chaining_gt(left: &[Self], right: &[Self]) -> ControlFlow<bool>;
    fn chaining_ge(left: &[Self], right: &[Self]) -> ControlFlow<bool>;
}

type AlwaysBreak<B> = ControlFlow<B, crate::convert::Infallible>;

impl<A: PartialOrd> SlicePartialOrd for A {
    default fn partial_compare(left: &[A], right: &[A]) -> Option<Ordering> {
        let elem_chain = |a, b| match PartialOrd::partial_cmp(a, b) {
            Some(Ordering::Equal) => ControlFlow::Continue(()),
            non_eq => ControlFlow::Break(non_eq),
        };
        let len_chain = |a: &_, b: &_| ControlFlow::Break(usize::partial_cmp(a, b));
        let AlwaysBreak::Break(b) = chaining_impl(left, right, elem_chain, len_chain);
        b
    }
}

impl<A: PartialOrd> SliceChain for A {
    default fn chaining_lt(left: &[Self], right: &[Self]) -> ControlFlow<bool> {
        chaining_impl(left, right, PartialOrd::__chaining_lt, usize::__chaining_lt)
    }
    default fn chaining_le(left: &[Self], right: &[Self]) -> ControlFlow<bool> {
        chaining_impl(left, right, PartialOrd::__chaining_le, usize::__chaining_le)
    }
    default fn chaining_gt(left: &[Self], right: &[Self]) -> ControlFlow<bool> {
        chaining_impl(left, right, PartialOrd::__chaining_gt, usize::__chaining_gt)
    }
    default fn chaining_ge(left: &[Self], right: &[Self]) -> ControlFlow<bool> {
        chaining_impl(left, right, PartialOrd::__chaining_ge, usize::__chaining_ge)
    }
}

#[inline]
fn chaining_impl<'l, 'r, A: PartialOrd, B, C>(
    left: &'l [A],
    right: &'r [A],
    elem_chain: impl Fn(&'l A, &'r A) -> ControlFlow<B>,
    len_chain: impl for<'a> FnOnce(&'a usize, &'a usize) -> ControlFlow<B, C>,
) -> ControlFlow<B, C> {
    let l = cmp::min(left.len(), right.len());

    // 先切到循环迭代范围，使编译器能消除边界检查。
    let lhs = &left[..l];
    let rhs = &right[..l];

    for i in 0..l {
        elem_chain(&lhs[i], &rhs[i])?;
    }

    len_chain(&left.len(), &right.len())
}

// 这是我们想要的 impl，但遗憾的是它不健全。见 `partial_ord_slice.rs`。
/*
impl<A> SlicePartialOrd for A
where
    A: Ord,
{
    default fn partial_compare(left: &[A], right: &[A]) -> Option<Ordering> {
        Some(SliceOrd::compare(left, right))
    }
}
*/

#[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
impl<A: [const] AlwaysApplicableOrd> const SlicePartialOrd for A {
    fn partial_compare(left: &[A], right: &[A]) -> Option<Ordering> {
        Some(SliceOrd::compare(left, right))
    }
}

#[rustc_specialization_trait]
#[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
const trait AlwaysApplicableOrd: [const] SliceOrd + [const] Ord {}

macro_rules! always_applicable_ord {
    ($([$($p:tt)*] $t:ty,)*) => {
        $(impl<$($p)*> AlwaysApplicableOrd for $t {})*
    }
}

always_applicable_ord! {
    [] u8, [] u16, [] u32, [] u64, [] u128, [] usize,
    [] i8, [] i16, [] i32, [] i64, [] i128, [] isize,
    [] bool, [] char,
    [T: ?Sized] *const T, [T: ?Sized] *mut T,
    [T: AlwaysApplicableOrd] &T,
    [T: AlwaysApplicableOrd] &mut T,
    [T: AlwaysApplicableOrd] Option<T>,
}

#[doc(hidden)]
#[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
// 用于切片 Ord specialization 的中间 trait。
const trait SliceOrd: Sized {
    fn compare(left: &[Self], right: &[Self]) -> Ordering;
}

impl<A: Ord> SliceOrd for A {
    default fn compare(left: &[Self], right: &[Self]) -> Ordering {
        let elem_chain = |a, b| match Ord::cmp(a, b) {
            Ordering::Equal => ControlFlow::Continue(()),
            non_eq => ControlFlow::Break(non_eq),
        };
        let len_chain = |a: &_, b: &_| ControlFlow::Break(usize::cmp(a, b));
        let AlwaysBreak::Break(b) = chaining_impl(left, right, elem_chain, len_chain);
        b
    }
}

/// 标记某个类型在比较时应被当作 unsigned byte 处理。
///
/// # 安全性(Safety）
/// * 该类型必须能作为 `u8` 读取，也就是布局与 `u8` 相同，并且始终已初始化。
/// * 对该类型的任意 `x` 和 `y`，`Ord(x, y)` 必须返回与
///   `Ord::cmp(transmute::<_, u8>(x), transmute::<_, u8>(y))` 相同的结果。
#[rustc_specialization_trait]
const unsafe trait UnsignedBytewiseOrd: [const] Ord {}

#[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
unsafe impl const UnsignedBytewiseOrd for bool {}
#[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
unsafe impl const UnsignedBytewiseOrd for u8 {}
#[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
unsafe impl const UnsignedBytewiseOrd for NonZero<u8> {}
#[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
unsafe impl const UnsignedBytewiseOrd for Option<NonZero<u8>> {}
#[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
unsafe impl const UnsignedBytewiseOrd for ascii::Char {}

// `compare_bytes` 按字典序比较 unsigned bytes 序列；满足 `UnsignedBytewiseOrd` 要求时可使用它。
#[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
impl<A: [const] Ord + [const] UnsignedBytewiseOrd> const SliceOrd for A {
    #[inline]
    fn compare(left: &[Self], right: &[Self]) -> Ordering {
        // 切片长度始终小于或等于 `isize::MAX`，因此这里不会下溢。
        let diff = left.len() as isize - right.len() as isize;
        // 该比较在 x86_64 和 ARM 上会被优化掉，因为减法会更新 flags。
        let len = if left.len() < right.len() { left.len() } else { right.len() };
        let left = left.as_ptr().cast();
        let right = right.as_ptr().cast();
        // SAFETY: `left` 和 `right` 都是引用，因此保证有效。`UnsignedBytewiseOrd` 只为
        // 可作为有效 u8 且比较语义一致的类型实现。这里使用两者长度的较小值，保证两个区域
        // 在该区间内都可读。
        let mut order = unsafe { compare_bytes(left, right, len) as isize };
        if order == 0 {
            order = diff;
        }
        order.cmp(&0)
    }
}

// 对可用 `memcmp` 比较的类型，也不要生成自定义链式循环。

#[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
impl<A: [const] PartialOrd + [const] UnsignedBytewiseOrd> const SliceChain for A {
    #[inline]
    fn chaining_lt(left: &[Self], right: &[Self]) -> ControlFlow<bool> {
        match SliceOrd::compare(left, right) {
            Ordering::Equal => ControlFlow::Continue(()),
            ne => ControlFlow::Break(ne.is_lt()),
        }
    }
    #[inline]
    fn chaining_le(left: &[Self], right: &[Self]) -> ControlFlow<bool> {
        match SliceOrd::compare(left, right) {
            Ordering::Equal => ControlFlow::Continue(()),
            ne => ControlFlow::Break(ne.is_le()),
        }
    }
    #[inline]
    fn chaining_gt(left: &[Self], right: &[Self]) -> ControlFlow<bool> {
        match SliceOrd::compare(left, right) {
            Ordering::Equal => ControlFlow::Continue(()),
            ne => ControlFlow::Break(ne.is_gt()),
        }
    }
    #[inline]
    fn chaining_ge(left: &[Self], right: &[Self]) -> ControlFlow<bool> {
        match SliceOrd::compare(left, right) {
            Ordering::Equal => ControlFlow::Continue(()),
            ne => ControlFlow::Break(ne.is_ge()),
        }
    }
}

pub(super) trait SliceContains: Sized {
    fn slice_contains(&self, x: &[Self]) -> bool;
}

impl<T> SliceContains for T
where
    T: PartialEq,
{
    default fn slice_contains(&self, x: &[Self]) -> bool {
        x.iter().any(|y| *y == *self)
    }
}

impl SliceContains for u8 {
    #[inline]
    fn slice_contains(&self, x: &[Self]) -> bool {
        memchr::memchr(*self, x).is_some()
    }
}

impl SliceContains for i8 {
    #[inline]
    fn slice_contains(&self, x: &[Self]) -> bool {
        let byte = *self as u8;
        // SAFETY: `i8` 和 `u8` 具有相同内存布局，因此把 `x.as_ptr()` 转成 `*const u8`
        // 是安全的。`x.as_ptr()` 来自引用，保证在 `x.len()` 长度内可读，且该长度不会超过
        // `isize::MAX`。返回的切片不会被修改。
        let bytes: &[u8] = unsafe { from_raw_parts(x.as_ptr() as *const u8, x.len()) };
        memchr::memchr(byte, bytes).is_some()
    }
}

macro_rules! impl_slice_contains {
    ($($t:ty),*) => {
        $(
            impl SliceContains for $t {
                #[inline]
                fn slice_contains(&self, arr: &[$t]) -> bool {
                    // 让 LANE_COUNT 成为普通 lane 数的 4 倍（目标是 128 bit vector）。
                    // 编译器会很好地展开它。
                    const LANE_COUNT: usize = 4 * (128 / (size_of::<$t>() * 8));
                    // SIMD 路径。
                    let mut chunks = arr.chunks_exact(LANE_COUNT);
                    for chunk in &mut chunks {
                        if chunk.iter().fold(false, |acc, x| acc | (*x == *self)) {
                            return true;
                        }
                    }
                    // 标量尾部。
                    return chunks.remainder().iter().any(|x| *x == *self);
                }
            }
        )*
    };
}

impl_slice_contains!(u16, u32, u64, i16, i32, i64, f32, f64, usize, isize, char);
