//! 定义数组的拥有型迭代器 `IntoIter`。

use crate::intrinsics::transmute_unchecked;
use crate::iter::{FusedIterator, TrustedLen, TrustedRandomAccessNoCoerce};
use crate::mem::{ManuallyDrop, MaybeUninit};
use crate::num::NonZero;
use crate::ops::{Deref as _, DerefMut as _, IndexRange, Range, Try};
use crate::{fmt, ptr};

mod iter_inner;

type InnerSized<T, const N: usize> = iter_inner::PolymorphicIter<[MaybeUninit<T>; N]>;
type InnerUnsized<T> = iter_inner::PolymorphicIter<[MaybeUninit<T>]>;

/// 按值消费[array] 的迭代器。
#[stable(feature = "array_value_iter", since = "1.51.0")]
#[rustc_insignificant_dtor]
#[rustc_diagnostic_item = "ArrayIntoIter"]
#[derive(Clone)]
pub struct IntoIter<T, const N: usize> {
    inner: ManuallyDrop<InnerSized<T, N>>,
}

impl<T, const N: usize> IntoIter<T, N> {
    #[inline]
    fn unsize(&self) -> &InnerUnsized<T> {
        self.inner.deref()
    }
    #[inline]
    fn unsize_mut(&mut self) -> &mut InnerUnsized<T> {
        self.inner.deref_mut()
    }
}

// 注意:`trait IntoIterator` 上的 `#[rustc_skip_during_method_dispatch(array)]`
// 会在 2021 之前的 edition 中,对显式 `.into_iter()` 调用隐藏该实现。
// 因此那些调用仍会解析到按引用工作的切片实现。
#[stable(feature = "array_into_iter_impl", since = "1.53.0")]
impl<T, const N: usize> IntoIterator for [T; N] {
    type Item = T;
    type IntoIter = IntoIter<T, N>;

    /// 创建消费型迭代器,也就是按从头到尾的顺序把每个值移出数组。
    ///
    /// 调用后原数组不能再使用;除非 `T` 实现了 `Copy`,此时整个数组会被复制。
    ///
    /// 2021 edition 之前,数组调用 `.into_iter()` 有特殊行为;更多信息见 [array] 的
    /// Editions 小节。
    ///
    /// [array]: prim@array
    #[inline]
    fn into_iter(self) -> Self::IntoIter {
        // SAFETY: 这里的 transmute 实际是安全的。`MaybeUninit` 文档承诺:
        //
        // > 保证 `MaybeUninit<T>` 与 `T` 具有相同的大小和对齐。
        //
        // 文档甚至展示了从 `MaybeUninit<T>` 数组 transmute 到 `T` 数组的例子。
        //
        // 基于该保证,这里的初始化满足不变量。
        //
        // FIXME: 如果普通 `transmute` 将来足够智能,能够直接允许这种转换,
        // 就改用它而不是 `transmute_unchecked`。
        let data: [MaybeUninit<T>; N] = unsafe { transmute_unchecked(self) };
        // SAFETY: 原数组已经完全初始化,这里传入的 alive 范围正表达了这一事实。
        let inner = unsafe { InnerSized::new_unchecked(IndexRange::zero_to(N), data) };
        IntoIter { inner: ManuallyDrop::new(inner) }
    }
}

impl<T, const N: usize> IntoIter<T, N> {
    /// 为给定 `array` 创建新的按值迭代器。
    #[stable(feature = "array_value_iter", since = "1.51.0")]
    #[deprecated(since = "1.59.0", note = "use `IntoIterator::into_iter` instead")]
    pub fn new(array: [T; N]) -> Self {
        IntoIterator::into_iter(array)
    }

    /// 为部分初始化缓冲区中的元素创建迭代器。
    ///
    /// 如果已有完全初始化的数组,应使用 [`IntoIterator`]。本函数主要用于 unsafe 代码
    /// 在失败或提前结束时返回部分结果。
    ///
    /// # 安全性(Safety）
    ///
    /// - `buffer[initialized]` 中的元素必须全部已初始化。
    /// - 范围必须是规范形式,即 `initialized.start <= initialized.end`。
    /// - 范围必须在缓冲区边界内,即 `initialized.end <= N`。
    ///   (这类似于 `[0][100..100]` 即使范围为空也会索引失败。)
    ///
    /// 实际初始化的元素多于 `initialized` 覆盖的范围仍是健全的,但额外元素很可能会泄漏。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(array_into_iter_constructors)]
    /// #![feature(maybe_uninit_uninit_array_transpose)]
    /// use std::array::IntoIter;
    /// use std::mem::MaybeUninit;
    ///
    /// # // 这里限制为 `Copy`,否则可能泄漏。完全通用的版本需要 drop guard
    /// # // 来处理迭代器 panic;但作为示例这样足够。
    /// fn next_chunk<T: Copy, const N: usize>(
    ///     it: &mut impl Iterator<Item = T>,
    /// ) -> Result<[T; N], IntoIter<T, N>> {
    ///     let mut buffer = [const { MaybeUninit::uninit() }; N];
    ///     let mut i = 0;
    ///     while i < N {
    ///         match it.next() {
    ///             Some(x) => {
    ///                 buffer[i].write(x);
    ///                 i += 1;
    ///             }
    ///             None => {
    ///                 // SAFETY: 已经初始化了前 `i` 个元素。
    ///                 unsafe {
    ///                     return Err(IntoIter::new_unchecked(buffer, 0..i));
    ///                 }
    ///             }
    ///         }
    ///     }
    ///
    ///     // SAFETY: 已经初始化了全部 N 个元素。
    ///     unsafe { Ok(buffer.transpose().assume_init()) }
    /// }
    ///
    /// let r: [_; 4] = next_chunk(&mut (10..16)).unwrap();
    /// assert_eq!(r, [10, 11, 12, 13]);
    /// let r: IntoIter<_, 40> = next_chunk(&mut (10..16)).unwrap_err();
    /// assert_eq!(r.collect::<Vec<_>>(), vec![10, 11, 12, 13, 14, 15]);
    /// ```
    #[unstable(feature = "array_into_iter_constructors", issue = "91583")]
    #[inline]
    pub const unsafe fn new_unchecked(
        buffer: [MaybeUninit<T>; N],
        initialized: Range<usize>,
    ) -> Self {
        // SAFETY: 本函数的安全条件之一保证范围是规范形式。
        let alive = unsafe { IndexRange::new_unchecked(initialized.start, initialized.end) };
        // SAFETY: 本函数的安全条件之一保证这些元素已经初始化。
        let inner = unsafe { InnerSized::new_unchecked(alive, buffer) };
        IntoIter { inner: ManuallyDrop::new(inner) }
    }

    /// 创建一个不会返回任何元素的 `T` 迭代器。
    ///
    /// 如果只需要空迭代器,请改用 [`iter::empty()`](crate::iter::empty)。
    /// 如果需要空数组,使用 `[]`。
    ///
    /// 但当你明确需要 `array::IntoIter<T, N>` 类型时,本函数很有用。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(array_into_iter_constructors)]
    /// use std::array::IntoIter;
    ///
    /// let empty = IntoIter::<i32, 3>::empty();
    /// assert_eq!(empty.len(), 0);
    /// assert_eq!(empty.as_slice(), &[]);
    ///
    /// let empty = IntoIter::<std::convert::Infallible, 200>::empty();
    /// assert_eq!(empty.len(), 0);
    /// ```
    ///
    /// `[1, 2].into_iter()` 与 `[].into_iter()` 拥有不同类型。
    /// ```should_fail,edition2021
    /// #![feature(array_into_iter_constructors)]
    /// use std::array::IntoIter;
    ///
    /// pub fn get_bytes(b: bool) -> IntoIter<i8, 4> {
    ///     if b {
    ///         [1, 2, 3, 4].into_iter()
    ///     } else {
    ///         [].into_iter() // error[E0308]: mismatched types
    ///     }
    /// }
    /// ```
    ///
    /// 使用本方法则可以取得元素类型和数组长度都合适的空迭代器:
    /// ```edition2021
    /// #![feature(array_into_iter_constructors)]
    /// use std::array::IntoIter;
    ///
    /// pub fn get_bytes(b: bool) -> IntoIter<i8, 4> {
    ///     if b {
    ///         [1, 2, 3, 4].into_iter()
    ///     } else {
    ///         IntoIter::empty()
    ///     }
    /// }
    ///
    /// assert_eq!(get_bytes(true).collect::<Vec<_>>(), vec![1, 2, 3, 4]);
    /// assert_eq!(get_bytes(false).collect::<Vec<_>>(), vec![]);
    /// ```
    #[unstable(feature = "array_into_iter_constructors", issue = "91583")]
    #[inline]
    pub const fn empty() -> Self {
        let inner = InnerSized::empty();
        IntoIter { inner: ManuallyDrop::new(inner) }
    }

    /// 返回包含所有尚未产出元素的不可变切片。
    #[stable(feature = "array_value_iter", since = "1.51.0")]
    #[inline]
    pub fn as_slice(&self) -> &[T] {
        self.unsize().as_slice()
    }

    /// 返回包含所有尚未产出元素的可变切片。
    #[stable(feature = "array_value_iter", since = "1.51.0")]
    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        self.unsize_mut().as_mut_slice()
    }
}

#[stable(feature = "array_value_iter_default", since = "1.89.0")]
impl<T, const N: usize> Default for IntoIter<T, N> {
    fn default() -> Self {
        IntoIter::empty()
    }
}

#[stable(feature = "array_value_iter_impls", since = "1.40.0")]
impl<T, const N: usize> Iterator for IntoIter<T, N> {
    type Item = T;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.unsize_mut().next()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.unsize().size_hint()
    }

    #[inline]
    fn fold<Acc, Fold>(mut self, init: Acc, fold: Fold) -> Acc
    where
        Fold: FnMut(Acc, Self::Item) -> Acc,
    {
        self.unsize_mut().fold(init, fold)
    }

    #[inline]
    fn try_fold<B, F, R>(&mut self, init: B, f: F) -> R
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> R,
        R: Try<Output = B>,
    {
        self.unsize_mut().try_fold(init, f)
    }

    #[inline]
    fn count(self) -> usize {
        self.len()
    }

    #[inline]
    fn last(mut self) -> Option<Self::Item> {
        self.next_back()
    }

    #[inline]
    fn advance_by(&mut self, n: usize) -> Result<(), NonZero<usize>> {
        self.unsize_mut().advance_by(n)
    }

    #[inline]
    unsafe fn __iterator_get_unchecked(&mut self, idx: usize) -> Self::Item {
        // SAFETY: 调用方必须提供仍未产出部分中的有效下标。
        let elem_ref = unsafe { self.as_mut_slice().get_unchecked_mut(idx) };
        // SAFETY: 我们只为实际是 `Copy` 的类型实现 `TrustedRandomAccessNoCoerce`,
        // 因而不会出现多次 drop 的问题。
        unsafe { ptr::read(elem_ref) }
    }
}

#[stable(feature = "array_value_iter_impls", since = "1.40.0")]
impl<T, const N: usize> DoubleEndedIterator for IntoIter<T, N> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        self.unsize_mut().next_back()
    }

    #[inline]
    fn rfold<Acc, Fold>(mut self, init: Acc, rfold: Fold) -> Acc
    where
        Fold: FnMut(Acc, Self::Item) -> Acc,
    {
        self.unsize_mut().rfold(init, rfold)
    }

    #[inline]
    fn try_rfold<B, F, R>(&mut self, init: B, f: F) -> R
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> R,
        R: Try<Output = B>,
    {
        self.unsize_mut().try_rfold(init, f)
    }

    #[inline]
    fn advance_back_by(&mut self, n: usize) -> Result<(), NonZero<usize>> {
        self.unsize_mut().advance_back_by(n)
    }
}

#[stable(feature = "array_value_iter_impls", since = "1.40.0")]
// 即使全部 Drop 逻辑都可以由 PolymorphicIter 处理,这个 impl 仍有两个作用:
// - Drop 已经是公开 API 的一部分,不能移除
// - 对 !Drop 类型,partial_drop 函数并不总能被完全优化掉,最终可能作为死代码进入二进制。
//   在调用链更高处按 needs_drop 分支,可以让更早的优化 pass 移除它。
impl<T, const N: usize> Drop for IntoIter<T, N> {
    #[inline]
    fn drop(&mut self) {
        if crate::mem::needs_drop::<T>() {
            // SAFETY: 这是唯一会 drop 该字段的位置。
            unsafe { ManuallyDrop::drop(&mut self.inner) }
        }
    }
}

#[stable(feature = "array_value_iter_impls", since = "1.40.0")]
impl<T, const N: usize> ExactSizeIterator for IntoIter<T, N> {
    #[inline]
    fn len(&self) -> usize {
        self.inner.len()
    }
    #[inline]
    fn is_empty(&self) -> bool {
        self.inner.len() == 0
    }
}

#[stable(feature = "array_value_iter_impls", since = "1.40.0")]
impl<T, const N: usize> FusedIterator for IntoIter<T, N> {}

// 该迭代器确实报告正确长度。“活跃”(仍会被产出)元素数量就是 `alive` 范围长度。
// `next` 或 `next_back` 会缩短这个范围;这些方法只有在返回 `Some(_)` 时才会
// 每次将长度减少 1。
#[stable(feature = "array_value_iter_impls", since = "1.40.0")]
unsafe impl<T, const N: usize> TrustedLen for IntoIter<T, N> {}

#[doc(hidden)]
#[unstable(issue = "none", feature = "std_internals")]
#[rustc_unsafe_specialization_marker]
pub trait NonDrop {}

// 用 T: Copy 近似 !Drop,因为 get_unchecked 不推进 self.alive,因此无法实现 drop 处理。
#[unstable(issue = "none", feature = "std_internals")]
impl<T: Copy> NonDrop for T {}

#[doc(hidden)]
#[unstable(issue = "none", feature = "std_internals")]
unsafe impl<T, const N: usize> TrustedRandomAccessNoCoerce for IntoIter<T, N>
where
    T: NonDrop,
{
    const MAY_HAVE_SIDE_EFFECT: bool = false;
}

#[stable(feature = "array_value_iter_impls", since = "1.40.0")]
impl<T: fmt::Debug, const N: usize> fmt::Debug for IntoIter<T, N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.unsize().fmt(f)
    }
}
