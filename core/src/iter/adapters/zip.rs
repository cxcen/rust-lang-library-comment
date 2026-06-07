use crate::cmp;
use crate::fmt::{self, Debug};
use crate::iter::{
    FusedIterator, InPlaceIterable, SourceIter, TrustedFused, TrustedLen, UncheckedIterator,
};
use crate::num::NonZero;

/// 同时迭代另外两个 iterator 的 iterator。
///
/// 该 `struct` 由 [`zip`] 或 [`Iterator::zip`] 创建。更多信息见它们的文档。
#[derive(Clone)]
#[must_use = "iterators are lazy and do nothing unless consumed"]
#[stable(feature = "rust1", since = "1.0.0")]
pub struct Zip<A, B> {
    a: A,
    b: B,
    // index、len 和 a_len 只被 zip 的特化版本使用。
    index: usize,
    len: usize,
}
impl<A: Iterator, B: Iterator> Zip<A, B> {
    pub(in crate::iter) fn new(a: A, b: B) -> Zip<A, B> {
        ZipImpl::new(a, b)
    }
    fn super_nth(&mut self, mut n: usize) -> Option<(A::Item, B::Item)> {
        while let Some(x) = Iterator::next(self) {
            if n == 0 {
                return Some(x);
            }
            n -= 1;
        }
        None
    }
}

/// 将参数转换为 iterator，并把它们 zip 在一起。
///
/// 更多信息见 [`Iterator::zip`] 的文档。
///
/// # 示例
///
/// ```
/// use std::iter::zip;
///
/// let xs = [1, 2, 3];
/// let ys = [4, 5, 6];
///
/// let mut iter = zip(xs, ys);
///
/// assert_eq!(iter.next().unwrap(), (1, 4));
/// assert_eq!(iter.next().unwrap(), (2, 5));
/// assert_eq!(iter.next().unwrap(), (3, 6));
/// assert!(iter.next().is_none());
///
/// // 也可以嵌套 zip:
/// let zs = [7, 8, 9];
///
/// let mut iter = zip(zip(xs, ys), zs);
///
/// assert_eq!(iter.next().unwrap(), ((1, 4), 7));
/// assert_eq!(iter.next().unwrap(), ((2, 5), 8));
/// assert_eq!(iter.next().unwrap(), ((3, 6), 9));
/// assert!(iter.next().is_none());
/// ```
#[stable(feature = "iter_zip", since = "1.59.0")]
pub fn zip<A, B>(a: A, b: B) -> Zip<A::IntoIter, B::IntoIter>
where
    A: IntoIterator,
    B: IntoIterator,
{
    ZipImpl::new(a.into_iter(), b.into_iter())
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<A, B> Iterator for Zip<A, B>
where
    A: Iterator,
    B: Iterator,
{
    type Item = (A::Item, B::Item);

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        ZipImpl::next(self)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        ZipImpl::size_hint(self)
    }

    #[inline]
    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        ZipImpl::nth(self, n)
    }

    #[inline]
    fn fold<Acc, F>(self, init: Acc, f: F) -> Acc
    where
        F: FnMut(Acc, Self::Item) -> Acc,
    {
        ZipImpl::fold(self, init, f)
    }

    #[inline]
    unsafe fn __iterator_get_unchecked(&mut self, idx: usize) -> Self::Item
    where
        Self: TrustedRandomAccessNoCoerce,
    {
        // SAFETY: `ZipImpl::__iterator_get_unchecked` 与
        // `Iterator::__iterator_get_unchecked` 具有相同安全要求。
        unsafe { ZipImpl::get_unchecked(self, idx) }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<A, B> DoubleEndedIterator for Zip<A, B>
where
    A: DoubleEndedIterator + ExactSizeIterator,
    B: DoubleEndedIterator + ExactSizeIterator,
{
    #[inline]
    fn next_back(&mut self) -> Option<(A::Item, B::Item)> {
        ZipImpl::next_back(self)
    }
}

// Zip specialization trait。
#[doc(hidden)]
trait ZipImpl<A, B> {
    type Item;
    fn new(a: A, b: B) -> Self;
    fn next(&mut self) -> Option<Self::Item>;
    fn size_hint(&self) -> (usize, Option<usize>);
    fn nth(&mut self, n: usize) -> Option<Self::Item>;
    fn next_back(&mut self) -> Option<Self::Item>
    where
        A: DoubleEndedIterator + ExactSizeIterator,
        B: DoubleEndedIterator + ExactSizeIterator;
    fn fold<Acc, F>(self, init: Acc, f: F) -> Acc
    where
        F: FnMut(Acc, Self::Item) -> Acc;
    // 这与 `Iterator::__iterator_get_unchecked` 具有相同安全要求。
    unsafe fn get_unchecked(&mut self, idx: usize) -> <Self as Iterator>::Item
    where
        Self: Iterator + TrustedRandomAccessNoCoerce;
}

// 绕过 specialization 的限制: `default` impl 必须在中间 impl 中重复写出。
macro_rules! zip_impl_general_defaults {
    () => {
        default fn new(a: A, b: B) -> Self {
            Zip {
                a,
                b,
                index: 0, // unused
                len: 0,   // unused
            }
        }

        #[inline]
        default fn next(&mut self) -> Option<(A::Item, B::Item)> {
            let x = self.a.next()?;
            let y = self.b.next()?;
            Some((x, y))
        }

        #[inline]
        default fn nth(&mut self, n: usize) -> Option<Self::Item> {
            self.super_nth(n)
        }

        #[inline]
        default fn next_back(&mut self) -> Option<(A::Item, B::Item)>
        where
            A: DoubleEndedIterator + ExactSizeIterator,
            B: DoubleEndedIterator + ExactSizeIterator,
        {
            // 下面的函数体只使用 `self.a/b.len()` 和 `self.a/b.next_back()`，
            // 并且不会过多调用 `next_back`，因此该实现在
            // `TrustedRandomAccessNoCoerce` specialization 中是安全的。

            let a_sz = self.a.len();
            let b_sz = self.b.len();
            if a_sz != b_sz {
                // 调整 a、b，使二者长度相等。
                if a_sz > b_sz {
                    for _ in 0..a_sz - b_sz {
                        self.a.next_back();
                    }
                } else {
                    for _ in 0..b_sz - a_sz {
                        self.b.next_back();
                    }
                }
            }
            match (self.a.next_back(), self.b.next_back()) {
                (Some(x), Some(y)) => Some((x, y)),
                (None, None) => None,
                _ => unreachable!(),
            }
        }
    };
}

// 通用 Zip 实现。
#[doc(hidden)]
impl<A, B> ZipImpl<A, B> for Zip<A, B>
where
    A: Iterator,
    B: Iterator,
{
    type Item = (A::Item, B::Item);

    zip_impl_general_defaults! {}

    #[inline]
    default fn size_hint(&self) -> (usize, Option<usize>) {
        let (a_lower, a_upper) = self.a.size_hint();
        let (b_lower, b_upper) = self.b.size_hint();

        let lower = cmp::min(a_lower, b_lower);

        let upper = match (a_upper, b_upper) {
            (Some(x), Some(y)) => Some(cmp::min(x, y)),
            (Some(x), None) => Some(x),
            (None, Some(y)) => Some(y),
            (None, None) => None,
        };

        (lower, upper)
    }

    default unsafe fn get_unchecked(&mut self, _idx: usize) -> <Self as Iterator>::Item
    where
        Self: TrustedRandomAccessNoCoerce,
    {
        unreachable!("Always specialized");
    }

    #[inline]
    default fn fold<Acc, F>(self, init: Acc, f: F) -> Acc
    where
        F: FnMut(Acc, Self::Item) -> Acc,
    {
        SpecFold::spec_fold(self, init, f)
    }
}

#[doc(hidden)]
impl<A, B> ZipImpl<A, B> for Zip<A, B>
where
    A: TrustedRandomAccessNoCoerce + Iterator,
    B: TrustedRandomAccessNoCoerce + Iterator,
{
    zip_impl_general_defaults! {}

    #[inline]
    default fn size_hint(&self) -> (usize, Option<usize>) {
        let size = cmp::min(self.a.size(), self.b.size());
        (size, Some(size))
    }

    #[inline]
    unsafe fn get_unchecked(&mut self, idx: usize) -> <Self as Iterator>::Item {
        let idx = self.index + idx;
        // SAFETY: 调用方必须维护 `Iterator::__iterator_get_unchecked` 的契约。
        unsafe { (self.a.__iterator_get_unchecked(idx), self.b.__iterator_get_unchecked(idx)) }
    }

    #[inline]
    fn fold<Acc, F>(mut self, init: Acc, mut f: F) -> Acc
    where
        F: FnMut(Acc, Self::Item) -> Acc,
    {
        let mut accum = init;
        let len = ZipImpl::size_hint(&self).0;
        for i in 0..len {
            // SAFETY: 由于 Self: TrustedRandomAccessNoCoerce，可以信任 size-hint 来计算
            // 长度，并据此执行 unchecked 迭代。fold 会消耗 iterator，因此不需要修复任何状态。
            unsafe {
                accum = f(accum, self.get_unchecked(i));
            }
        }
        accum
    }
}

#[doc(hidden)]
impl<A, B> ZipImpl<A, B> for Zip<A, B>
where
    A: TrustedRandomAccess + Iterator,
    B: TrustedRandomAccess + Iterator,
{
    fn new(a: A, b: B) -> Self {
        let len = cmp::min(a.size(), b.size());
        Zip { a, b, index: 0, len }
    }

    #[inline]
    fn next(&mut self) -> Option<(A::Item, B::Item)> {
        if self.index < self.len {
            let i = self.index;
            // 由于 get_unchecked 会执行可能 panic 的代码，我们先递增计数器，
            // 以满足 TrustedRandomAccess 要求: 同一索引不会被访问两次。
            self.index += 1;
            // SAFETY: `i` 小于 `self.len`，因此也小于 `self.a.len()` 和 `self.b.len()`。
            unsafe {
                Some((self.a.__iterator_get_unchecked(i), self.b.__iterator_get_unchecked(i)))
            }
        } else {
            None
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len - self.index;
        (len, Some(len))
    }

    #[inline]
    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        let delta = cmp::min(n, self.len - self.index);
        let end = self.index + delta;
        while self.index < end {
            let i = self.index;
            // 由于 get_unchecked 会执行可能 panic 的代码，我们先递增计数器，
            // 以满足 TrustedRandomAccess 要求: 同一索引不会被访问两次。
            self.index += 1;
            if A::MAY_HAVE_SIDE_EFFECT {
                // SAFETY: 使用 `cmp::min` 计算 `delta` 可确保 `end` 小于等于 `self.len`，
                // 因此 `i` 也小于 `self.len`。
                unsafe {
                    self.a.__iterator_get_unchecked(i);
                }
            }
            if B::MAY_HAVE_SIDE_EFFECT {
                // SAFETY: 同上。
                unsafe {
                    self.b.__iterator_get_unchecked(i);
                }
            }
        }

        self.super_nth(n - delta)
    }

    #[inline]
    fn next_back(&mut self) -> Option<(A::Item, B::Item)>
    where
        A: DoubleEndedIterator + ExactSizeIterator,
        B: DoubleEndedIterator + ExactSizeIterator,
    {
        // iterator 耗尽后不再产生副作用，以减少 unsafe 代码必须处理的情况数量。
        // #137255 展示了过度复杂的情况如何导致 unsoundness。
        if self.index < self.len {
            let old_len = self.len;

            // 由于 get_unchecked 和带副作用的代码可能执行会 panic 的用户代码，
            // 我们先递减计数器，以满足 TrustedRandomAccess 要求: 同一索引不会被访问两次。
            // 此外，这也确保带副作用的代码不会第二次运行。
            self.len -= 1;

            // 如果正在反向迭代，则调整 a、b，使二者长度相等。
            if A::MAY_HAVE_SIDE_EFFECT || B::MAY_HAVE_SIDE_EFFECT {
                // 注意，如果已经发生过一些前向迭代，这些并不是内部 iterator 的真实剩余
                // 长度，因此必须把它们与 Zip 的内部长度跟踪关联起来。
                let sz_a = self.a.size();
                let sz_b = self.b.size();
                // 这个条件只能、也必须只在第一次 `next_back` 调用时为真；否则会破坏
                // 调用 `get_unchecked()` 后对 `self.next_back()` 调用次数的限制。
                if sz_a != sz_b && (old_len == sz_a || old_len == sz_b) {
                    if A::MAY_HAVE_SIDE_EFFECT && sz_a > old_len {
                        for _ in 0..sz_a - old_len {
                            self.a.next_back();
                        }
                    }
                    if B::MAY_HAVE_SIDE_EFFECT && sz_b > old_len {
                        for _ in 0..sz_b - old_len {
                            self.b.next_back();
                        }
                    }
                    debug_assert_eq!(self.a.size(), self.b.size());
                }
            }
            let i = self.len;
            // SAFETY: `i` 小于 `self.len` 之前的值，而该值也小于等于 `self.a.len()`
            // 和 `self.b.len()`。
            unsafe {
                Some((self.a.__iterator_get_unchecked(i), self.b.__iterator_get_unchecked(i)))
            }
        } else {
            None
        }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<A, B> ExactSizeIterator for Zip<A, B>
where
    A: ExactSizeIterator,
    B: ExactSizeIterator,
{
}

#[doc(hidden)]
#[unstable(feature = "trusted_random_access", issue = "none")]
unsafe impl<A, B> TrustedRandomAccess for Zip<A, B>
where
    A: TrustedRandomAccess,
    B: TrustedRandomAccess,
{
}

#[doc(hidden)]
#[unstable(feature = "trusted_random_access", issue = "none")]
unsafe impl<A, B> TrustedRandomAccessNoCoerce for Zip<A, B>
where
    A: TrustedRandomAccessNoCoerce,
    B: TrustedRandomAccessNoCoerce,
{
    const MAY_HAVE_SIDE_EFFECT: bool = A::MAY_HAVE_SIDE_EFFECT || B::MAY_HAVE_SIDE_EFFECT;
}

#[stable(feature = "fused", since = "1.26.0")]
impl<A, B> FusedIterator for Zip<A, B>
where
    A: FusedIterator,
    B: FusedIterator,
{
}

#[unstable(issue = "none", feature = "trusted_fused")]
unsafe impl<A, B> TrustedFused for Zip<A, B>
where
    A: TrustedFused,
    B: TrustedFused,
{
}

#[unstable(feature = "trusted_len", issue = "37572")]
unsafe impl<A, B> TrustedLen for Zip<A, B>
where
    A: TrustedLen,
    B: TrustedLen,
{
}

impl<A, B> UncheckedIterator for Zip<A, B>
where
    A: UncheckedIterator,
    B: UncheckedIterator,
{
}

// 任意选择 zip 迭代的左侧作为可提取的 "source"；若要两边都尝试，需要 negative trait bound。
#[unstable(issue = "none", feature = "inplace_iteration")]
unsafe impl<A, B> SourceIter for Zip<A, B>
where
    A: SourceIter,
{
    type Source = A::Source;

    #[inline]
    unsafe fn as_inner(&mut self) -> &mut A::Source {
        // SAFETY: 将 unsafe 函数转发到具有相同要求的 unsafe 函数。
        unsafe { SourceIter::as_inner(&mut self.a) }
    }
}

// 由于 SourceIter 转发左侧，这里也采用同样做法。
#[unstable(issue = "none", feature = "inplace_iteration")]
unsafe impl<A: InPlaceIterable, B> InPlaceIterable for Zip<A, B> {
    const EXPAND_BY: Option<NonZero<usize>> = A::EXPAND_BY;
    const MERGE_BY: Option<NonZero<usize>> = A::MERGE_BY;
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<A: Debug, B: Debug> Debug for Zip<A, B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        ZipFmt::fmt(self, f)
    }
}

trait ZipFmt<A, B> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result;
}

impl<A: Debug, B: Debug> ZipFmt<A, B> for Zip<A, B> {
    default fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Zip").field("a", &self.a).field("b", &self.b).finish()
    }
}

impl<A: Debug + TrustedRandomAccessNoCoerce, B: Debug + TrustedRandomAccessNoCoerce> ZipFmt<A, B>
    for Zip<A, B>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // 对内部迭代器调用 fmt 并不安全: 一旦开始迭代，它们可能处于特殊的、
        // 甚至对普通方法调用不安全的中间状态。
        f.debug_struct("Zip").finish()
    }
}

/// 可以高效随机访问元素的迭代器。
///
/// # 安全性(Safety）
///
/// 迭代器的 `size_hint` 必须精确，并且调用开销低。
///
/// 不允许覆盖 `TrustedRandomAccessNoCoerce::size`。
///
/// `Self` 的所有子类型和所有父类型也必须实现 `TrustedRandomAccess`。这尤其意味着，
/// 带有非 invariant 参数的类型通常不能让 `TrustedRandomAccess` 实现依赖这些参数上
/// 的 trait bound；例外是来自对应 struct/enum 定义本身的 bound，或来自具备类似保证
/// 的 trait 的 bound。
///
/// 如果 `Self: ExactSizeIterator`，则 `self.len()` 必须始终产生与 `self.size()` 一致的结果。
///
/// 如果 `Self: Iterator`，并且满足下面条件，则调用
/// `<Self as Iterator>::__iterator_get_unchecked(&mut self, idx)` 必须是安全的。
///
/// 1. `0 <= idx` 且 `idx < self.size()`。
/// 2. 如果 `Self: !Clone`，则不会在同一个 `self` 上用同一个索引重复调用
///    `self.__iterator_get_unchecked(idx)`。
/// 3. 调用 `self.__iterator_get_unchecked(idx)` 后，`self.next_back()` 最多只会再被调用
///    `self.size() - idx - 1` 次。如果 `Self: Clone` 且 `self` 被克隆，该次数会分别为
///    `self` 和它的 clone 计算；但 clone 之前已经发生的 `self.next_back()` 调用同时计入
///    `self` 和 clone。
/// 4. 调用 `self.__iterator_get_unchecked(idx)` 后，只会在 `self` 或 `self` 的新 clone 上
///    调用以下方法:
///     * `std::clone::Clone::clone`
///     * `std::iter::Iterator::size_hint`
///     * `std::iter::DoubleEndedIterator::next_back`
///     * `std::iter::ExactSizeIterator::len`
///     * `std::iter::Iterator::__iterator_get_unchecked`
///     * `std::iter::TrustedRandomAccessNoCoerce::size`
/// 5. 如果 `Self` 是 `T` 的子类型，则允许把 `self` coercion 到 `T`。如果在已经调用
///    `self.__iterator_get_unchecked(idx)` 后把 `self` coercion 到 `T`，那么得到的 `T`
///    类型值也只能调用第 4 条列出的方法。允许多次这样的 coercion。对于第 2 条和第
///    3 条，`self`、得到的 `T` 值以及进一步父类型 coercion 结果上的
///    `__iterator_get_unchecked(idx)` 或 `next_back()` 调用次数要合并计算，总和不能
///    超过规定边界。
///
/// 此外，在这些条件满足时，实现还必须保证:
///
/// * 不改变 `size_hint` 返回的值。
/// * 调用 `self.__iterator_get_unchecked(idx)` 后，在所需 trait 已实现的前提下，
///   继续调用上面列出的方法仍然安全。
/// * 调用 `self.__iterator_get_unchecked(idx)` 后，drop `self` 仍然安全。
/// * 如果 `Self` 是 `T` 的子类型，把 `self` coercion 到 `T` 仍然安全。
//
// FIXME: 需要澄清与 SourceIter/InPlaceIterable 的交互。按预期，
// `__iterator_get_unchecked` 之后应允许调用 `SourceIter::as_inner`。
#[doc(hidden)]
#[unstable(feature = "trusted_random_access", issue = "none")]
#[rustc_specialization_trait]
pub unsafe trait TrustedRandomAccess: TrustedRandomAccessNoCoerce {}

/// 类似 [`TrustedRandomAccess`]，但不包含
/// `__iterator_get_unchecked` 之后 coercion 到父类型的要求或保证(这里不允许这种
/// coercion)，也不要求子类型或父类型实现 `TrustedRandomAccessNoCoerce`。
///
/// 该 trait 创建于 PR #85874，用于在不引入性能回退的情况下修复 soundness issue
/// #85873。它仍可能变化，因为之后可能会构建一个更通用、更适合性能优化且更精细的
/// trait 或 trait 层级，用来替代或扩展 [`TrustedRandomAccess`] 和
/// `TrustedRandomAccessNoCoerce`。
#[doc(hidden)]
#[unstable(feature = "trusted_random_access", issue = "none")]
#[rustc_specialization_trait]
pub unsafe trait TrustedRandomAccessNoCoerce: Sized {
    // 便捷方法。
    fn size(&self) -> usize
    where
        Self: Iterator,
    {
        self.size_hint().0
    }
    /// 如果取得迭代器元素可能有副作用，则为 `true`。
    /// 记得把内部迭代器也纳入考虑。
    const MAY_HAVE_SIDE_EFFECT: bool;
}

/// 类似 `Iterator::__iterator_get_unchecked`，但不要求编译器知道
/// `U: TrustedRandomAccess`。
///
/// ## 安全性(Safety）
///
/// 要求与直接调用 `get_unchecked` 相同。
#[doc(hidden)]
#[inline]
pub(in crate::iter::adapters) unsafe fn try_get_unchecked<I>(it: &mut I, idx: usize) -> I::Item
where
    I: Iterator,
{
    // SAFETY: 调用方必须维护 `Iterator::__iterator_get_unchecked` 的契约。
    unsafe { it.try_get_unchecked(idx) }
}

unsafe trait SpecTrustedRandomAccess: Iterator {
    /// 如果 `Self: TrustedRandomAccess`，则调用
    /// `Iterator::__iterator_get_unchecked(self, index)` 必须是安全的。
    unsafe fn try_get_unchecked(&mut self, index: usize) -> Self::Item;
}

unsafe impl<I: Iterator> SpecTrustedRandomAccess for I {
    default unsafe fn try_get_unchecked(&mut self, _: usize) -> Self::Item {
        panic!("Should only be called on TrustedRandomAccess iterators");
    }
}

unsafe impl<I: Iterator + TrustedRandomAccessNoCoerce> SpecTrustedRandomAccess for I {
    #[inline]
    unsafe fn try_get_unchecked(&mut self, index: usize) -> Self::Item {
        // SAFETY: 调用方必须维护 `Iterator::__iterator_get_unchecked` 的契约。
        unsafe { self.__iterator_get_unchecked(index) }
    }
}

trait SpecFold: Iterator {
    fn spec_fold<B, F>(self, init: B, f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B;
}

impl<A: Iterator, B: Iterator> SpecFold for Zip<A, B> {
    // 改编自 Iterator trait 的默认实现。
    #[inline]
    default fn spec_fold<Acc, F>(mut self, init: Acc, mut f: F) -> Acc
    where
        F: FnMut(Acc, Self::Item) -> Acc,
    {
        let mut accum = init;
        while let Some(x) = ZipImpl::next(&mut self) {
            accum = f(accum, x);
        }
        accum
    }
}

impl<A: TrustedLen, B: TrustedLen> SpecFold for Zip<A, B> {
    #[inline]
    fn spec_fold<Acc, F>(mut self, init: Acc, mut f: F) -> Acc
    where
        F: FnMut(Acc, Self::Item) -> Acc,
    {
        let mut accum = init;
        loop {
            let (upper, more) = if let Some(upper) = ZipImpl::size_hint(&self).1 {
                (upper, false)
            } else {
                // 根据 TrustedLen 契约，None 上界表示元素数超过 usize::MAX。
                (usize::MAX, true)
            };

            for _ in 0..upper {
                let pair =
                    // SAFETY: TrustedLen 保证至少有 `upper` 个元素可用，
                    // 因此这里知道它们不会是 None。
                    unsafe { (self.a.next().unwrap_unchecked(), self.b.next().unwrap_unchecked()) };
                accum = f(accum, pair);
            }

            if !more {
                break;
            }
        }
        accum
    }
}
