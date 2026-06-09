use crate::intrinsics;
use crate::iter::adapters::SourceIter;
use crate::iter::adapters::zip::try_get_unchecked;
use crate::iter::{
    FusedIterator, TrustedFused, TrustedLen, TrustedRandomAccess, TrustedRandomAccessNoCoerce,
};
use crate::ops::Try;

/// 底层迭代器第一次产出 `None` 后，就永远继续产出 `None` 的迭代器。
///
/// 该 `struct` 由 [`Iterator::fuse`] 创建。更多语义见该方法文档。
#[derive(Clone, Debug)]
#[must_use = "iterators are lazy and do nothing unless consumed"]
#[stable(feature = "rust1", since = "1.0.0")]
pub struct Fuse<I> {
    // 注意: 对 `I: FusedIterator`，我们通常不会主动把这里设成 `None`，
    // 但由于型变相关原因仍必须能处理该状态。见 rust-lang/rust#85863。
    iter: Option<I>,
}
impl<I> Fuse<I> {
    pub(in crate::iter) fn new(iter: I) -> Fuse<I> {
        Fuse { iter: Some(iter) }
    }

    pub(crate) fn into_inner(self) -> Option<I> {
        self.iter
    }
}

#[stable(feature = "fused", since = "1.26.0")]
impl<I> FusedIterator for Fuse<I> where I: Iterator {}

#[unstable(issue = "none", feature = "trusted_fused")]
unsafe impl<I> TrustedFused for Fuse<I> where I: TrustedFused {}

// 这里的所有特化实现都保持为内部细节，避免把 default fn 暴露到 trait 外部。
#[stable(feature = "rust1", since = "1.0.0")]
impl<I> Iterator for Fuse<I>
where
    I: Iterator,
{
    type Item = <I as Iterator>::Item;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        FuseImpl::next(self)
    }

    #[inline]
    fn nth(&mut self, n: usize) -> Option<I::Item> {
        FuseImpl::nth(self, n)
    }

    #[inline]
    fn last(self) -> Option<Self::Item> {
        match self.iter {
            Some(iter) => iter.last(),
            None => None,
        }
    }

    #[inline]
    fn count(self) -> usize {
        match self.iter {
            Some(iter) => iter.count(),
            None => 0,
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        match self.iter {
            Some(ref iter) => iter.size_hint(),
            None => (0, Some(0)),
        }
    }

    #[inline]
    fn try_fold<Acc, Fold, R>(&mut self, acc: Acc, fold: Fold) -> R
    where
        Self: Sized,
        Fold: FnMut(Acc, Self::Item) -> R,
        R: Try<Output = Acc>,
    {
        FuseImpl::try_fold(self, acc, fold)
    }

    #[inline]
    fn fold<Acc, Fold>(self, mut acc: Acc, fold: Fold) -> Acc
    where
        Fold: FnMut(Acc, Self::Item) -> Acc,
    {
        if let Some(iter) = self.iter {
            acc = iter.fold(acc, fold);
        }
        acc
    }

    #[inline]
    fn find<P>(&mut self, predicate: P) -> Option<Self::Item>
    where
        P: FnMut(&Self::Item) -> bool,
    {
        FuseImpl::find(self, predicate)
    }

    #[inline]
    unsafe fn __iterator_get_unchecked(&mut self, idx: usize) -> Self::Item
    where
        Self: TrustedRandomAccessNoCoerce,
    {
        match self.iter {
            // SAFETY: 调用方必须维护 `Iterator::__iterator_get_unchecked` 的契约。
            Some(ref mut iter) => unsafe { try_get_unchecked(iter, idx) },
            // SAFETY: 调用方断言索引处存在元素，因此当前不可能已耗尽。
            None => unsafe { intrinsics::unreachable() },
        }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<I> DoubleEndedIterator for Fuse<I>
where
    I: DoubleEndedIterator,
{
    #[inline]
    fn next_back(&mut self) -> Option<<I as Iterator>::Item> {
        FuseImpl::next_back(self)
    }

    #[inline]
    fn nth_back(&mut self, n: usize) -> Option<<I as Iterator>::Item> {
        FuseImpl::nth_back(self, n)
    }

    #[inline]
    fn try_rfold<Acc, Fold, R>(&mut self, acc: Acc, fold: Fold) -> R
    where
        Self: Sized,
        Fold: FnMut(Acc, Self::Item) -> R,
        R: Try<Output = Acc>,
    {
        FuseImpl::try_rfold(self, acc, fold)
    }

    #[inline]
    fn rfold<Acc, Fold>(self, mut acc: Acc, fold: Fold) -> Acc
    where
        Fold: FnMut(Acc, Self::Item) -> Acc,
    {
        if let Some(iter) = self.iter {
            acc = iter.rfold(acc, fold);
        }
        acc
    }

    #[inline]
    fn rfind<P>(&mut self, predicate: P) -> Option<Self::Item>
    where
        P: FnMut(&Self::Item) -> bool,
    {
        FuseImpl::rfind(self, predicate)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<I> ExactSizeIterator for Fuse<I>
where
    I: ExactSizeIterator,
{
    fn len(&self) -> usize {
        match self.iter {
            Some(ref iter) => iter.len(),
            None => 0,
        }
    }

    fn is_empty(&self) -> bool {
        match self.iter {
            Some(ref iter) => iter.is_empty(),
            None => true,
        }
    }
}

#[stable(feature = "default_iters", since = "1.70.0")]
impl<I: Default> Default for Fuse<I> {
    /// 从 `I` 的默认值创建一个 `Fuse` 迭代器。
    ///
    /// ```
    /// # use core::slice;
    /// # use std::iter::Fuse;
    /// let iter: Fuse<slice::Iter<'_, u8>> = Default::default();
    /// assert_eq!(iter.len(), 0);
    /// ```
    ///
    /// 这等同于 `I::default().fuse()`[^fuse_note]。例如，如果 `I::default()` 不是空
    /// 迭代器，那么这里得到的迭代器也不是空迭代器。
    ///
    /// ```
    /// # use std::iter::Fuse;
    /// #[derive(Default)]
    /// struct Fourever;
    ///
    /// impl Iterator for Fourever {
    ///     type Item = u32;
    ///     fn next(&mut self) -> Option<u32> {
    ///         Some(4)
    ///     }
    /// }
    ///
    /// let mut iter: Fuse<Fourever> = Default::default();
    /// assert_eq!(iter.next(), Some(4));
    /// ```
    ///
    /// [^fuse_note]: 前提是 `I` 没有覆盖 `Iterator::fuse` 的默认实现。
    fn default() -> Self {
        Fuse { iter: Some(I::default()) }
    }
}

#[unstable(feature = "trusted_len", issue = "37572")]
// SAFETY: `TrustedLen` 要求 `size_hint()` 报告准确长度。`Fuse` 只是把该信息转发给被
// 包装的迭代器 `I`；一旦自身记录为耗尽，则报告空迭代器。因此该性质被保留，可以
// 在这里实现 `TrustedLen`。
unsafe impl<I> TrustedLen for Fuse<I> where I: TrustedLen {}

#[doc(hidden)]
#[unstable(feature = "trusted_random_access", issue = "none")]
// SAFETY: `TrustedRandomAccess` 要求 `size_hint()` 精确且调用开销低，并且
// `Iterator::__iterator_get_unchecked()` 必须按相同长度契约实现。
//
// `Fuse` 只是把这些操作转发给被包装的迭代器 `I`，并在耗尽状态下阻止不可达访问，
// 因此这些性质被保留。
unsafe impl<I> TrustedRandomAccess for Fuse<I> where I: TrustedRandomAccess {}

#[doc(hidden)]
#[unstable(feature = "trusted_random_access", issue = "none")]
unsafe impl<I> TrustedRandomAccessNoCoerce for Fuse<I>
where
    I: TrustedRandomAccessNoCoerce,
{
    const MAY_HAVE_SIDE_EFFECT: bool = I::MAY_HAVE_SIDE_EFFECT;
}

/// `Fuse` 特化 trait。
///
/// 只需关注 `&mut self` 方法，因为这些方法可能在不消耗迭代器本身的情况下耗尽它。
#[doc(hidden)]
trait FuseImpl<I> {
    type Item;

    // 普通 `Iterator` 特有的函数。
    fn next(&mut self) -> Option<Self::Item>;
    fn nth(&mut self, n: usize) -> Option<Self::Item>;
    fn try_fold<Acc, Fold, R>(&mut self, acc: Acc, fold: Fold) -> R
    where
        Self: Sized,
        Fold: FnMut(Acc, Self::Item) -> R,
        R: Try<Output = Acc>;
    fn find<P>(&mut self, predicate: P) -> Option<Self::Item>
    where
        P: FnMut(&Self::Item) -> bool;

    // `DoubleEndedIterator` 特有的函数。
    fn next_back(&mut self) -> Option<Self::Item>
    where
        I: DoubleEndedIterator;
    fn nth_back(&mut self, n: usize) -> Option<Self::Item>
    where
        I: DoubleEndedIterator;
    fn try_rfold<Acc, Fold, R>(&mut self, acc: Acc, fold: Fold) -> R
    where
        Self: Sized,
        Fold: FnMut(Acc, Self::Item) -> R,
        R: Try<Output = Acc>,
        I: DoubleEndedIterator;
    fn rfind<P>(&mut self, predicate: P) -> Option<Self::Item>
    where
        P: FnMut(&Self::Item) -> bool,
        I: DoubleEndedIterator;
}

/// 通用 `Fuse` 实现: 耗尽后把 `iter` 设为 `None`。
#[doc(hidden)]
impl<I> FuseImpl<I> for Fuse<I>
where
    I: Iterator,
{
    type Item = <I as Iterator>::Item;

    #[inline]
    default fn next(&mut self) -> Option<<I as Iterator>::Item> {
        and_then_or_clear(&mut self.iter, Iterator::next)
    }

    #[inline]
    default fn nth(&mut self, n: usize) -> Option<I::Item> {
        and_then_or_clear(&mut self.iter, |iter| iter.nth(n))
    }

    #[inline]
    default fn try_fold<Acc, Fold, R>(&mut self, mut acc: Acc, fold: Fold) -> R
    where
        Self: Sized,
        Fold: FnMut(Acc, Self::Item) -> R,
        R: Try<Output = Acc>,
    {
        if let Some(ref mut iter) = self.iter {
            acc = iter.try_fold(acc, fold)?;
            self.iter = None;
        }
        try { acc }
    }

    #[inline]
    default fn find<P>(&mut self, predicate: P) -> Option<Self::Item>
    where
        P: FnMut(&Self::Item) -> bool,
    {
        and_then_or_clear(&mut self.iter, |iter| iter.find(predicate))
    }

    #[inline]
    default fn next_back(&mut self) -> Option<<I as Iterator>::Item>
    where
        I: DoubleEndedIterator,
    {
        and_then_or_clear(&mut self.iter, |iter| iter.next_back())
    }

    #[inline]
    default fn nth_back(&mut self, n: usize) -> Option<<I as Iterator>::Item>
    where
        I: DoubleEndedIterator,
    {
        and_then_or_clear(&mut self.iter, |iter| iter.nth_back(n))
    }

    #[inline]
    default fn try_rfold<Acc, Fold, R>(&mut self, mut acc: Acc, fold: Fold) -> R
    where
        Self: Sized,
        Fold: FnMut(Acc, Self::Item) -> R,
        R: Try<Output = Acc>,
        I: DoubleEndedIterator,
    {
        if let Some(ref mut iter) = self.iter {
            acc = iter.try_rfold(acc, fold)?;
            self.iter = None;
        }
        try { acc }
    }

    #[inline]
    default fn rfind<P>(&mut self, predicate: P) -> Option<Self::Item>
    where
        P: FnMut(&Self::Item) -> bool,
        I: DoubleEndedIterator,
    {
        and_then_or_clear(&mut self.iter, |iter| iter.rfind(predicate))
    }
}

/// 特化的 `Fuse` 实现: 耗尽时不会特意清空 `iter`。
/// 但仍必须准备好处理它已经被清空的可能性！
#[doc(hidden)]
impl<I> FuseImpl<I> for Fuse<I>
where
    I: FusedIterator,
{
    #[inline]
    fn next(&mut self) -> Option<<I as Iterator>::Item> {
        self.iter.as_mut()?.next()
    }

    #[inline]
    fn nth(&mut self, n: usize) -> Option<I::Item> {
        self.iter.as_mut()?.nth(n)
    }

    #[inline]
    fn try_fold<Acc, Fold, R>(&mut self, mut acc: Acc, fold: Fold) -> R
    where
        Self: Sized,
        Fold: FnMut(Acc, Self::Item) -> R,
        R: Try<Output = Acc>,
    {
        if let Some(ref mut iter) = self.iter {
            acc = iter.try_fold(acc, fold)?;
        }
        try { acc }
    }

    #[inline]
    fn find<P>(&mut self, predicate: P) -> Option<Self::Item>
    where
        P: FnMut(&Self::Item) -> bool,
    {
        self.iter.as_mut()?.find(predicate)
    }

    #[inline]
    fn next_back(&mut self) -> Option<<I as Iterator>::Item>
    where
        I: DoubleEndedIterator,
    {
        self.iter.as_mut()?.next_back()
    }

    #[inline]
    fn nth_back(&mut self, n: usize) -> Option<<I as Iterator>::Item>
    where
        I: DoubleEndedIterator,
    {
        self.iter.as_mut()?.nth_back(n)
    }

    #[inline]
    fn try_rfold<Acc, Fold, R>(&mut self, mut acc: Acc, fold: Fold) -> R
    where
        Self: Sized,
        Fold: FnMut(Acc, Self::Item) -> R,
        R: Try<Output = Acc>,
        I: DoubleEndedIterator,
    {
        if let Some(ref mut iter) = self.iter {
            acc = iter.try_rfold(acc, fold)?;
        }
        try { acc }
    }

    #[inline]
    fn rfind<P>(&mut self, predicate: P) -> Option<Self::Item>
    where
        P: FnMut(&Self::Item) -> bool,
        I: DoubleEndedIterator,
    {
        self.iter.as_mut()?.rfind(predicate)
    }
}

// 这被 Flatten 的 SourceIter impl 使用。
#[unstable(issue = "none", feature = "inplace_iteration")]
unsafe impl<I> SourceIter for Fuse<I>
where
    I: SourceIter + TrustedFused,
{
    type Source = I::Source;

    #[inline]
    unsafe fn as_inner(&mut self) -> &mut I::Source {
        // SAFETY: 将 unsafe 函数转发到具有相同要求的 unsafe 函数。
        // TrustedFused 保证永远不会遇到 `self.iter` 被设为 None 的情况。
        unsafe { SourceIter::as_inner(self.iter.as_mut().unwrap_unchecked()) }
    }
}

#[inline]
fn and_then_or_clear<T, U>(opt: &mut Option<T>, f: impl FnOnce(&mut T) -> Option<U>) -> Option<U> {
    let x = f(opt.as_mut()?);
    if x.is_none() {
        *opt = None;
    }
    x
}
