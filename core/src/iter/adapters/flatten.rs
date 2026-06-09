use crate::iter::adapters::SourceIter;
use crate::iter::{
    Cloned, Copied, Empty, Filter, FilterMap, Fuse, FusedIterator, Map, Once, OnceWith,
    TrustedFused, TrustedLen,
};
use crate::num::NonZero;
use crate::ops::{ControlFlow, Try};
use crate::{array, fmt, option, result};

/// 将每个元素映射为一个迭代器，并产出这些迭代器中元素的迭代器。
///
/// 该 `struct` 由 [`Iterator::flat_map`] 创建。更多信息见该方法文档。
#[must_use = "iterators are lazy and do nothing unless consumed"]
#[stable(feature = "rust1", since = "1.0.0")]
pub struct FlatMap<I, U: IntoIterator, F> {
    inner: FlattenCompat<Map<I, F>, <U as IntoIterator>::IntoIter>,
}

impl<I: Iterator, U: IntoIterator, F: FnMut(I::Item) -> U> FlatMap<I, U, F> {
    pub(in crate::iter) fn new(iter: I, f: F) -> FlatMap<I, U, F> {
        FlatMap { inner: FlattenCompat::new(iter.map(f)) }
    }

    pub(crate) fn into_parts(self) -> (Option<U::IntoIter>, Option<I>, Option<U::IntoIter>) {
        (
            self.inner.frontiter,
            self.inner.iter.into_inner().map(Map::into_inner),
            self.inner.backiter,
        )
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<I: Clone, U, F: Clone> Clone for FlatMap<I, U, F>
where
    U: Clone + IntoIterator<IntoIter: Clone>,
{
    fn clone(&self) -> Self {
        FlatMap { inner: self.inner.clone() }
    }
}

#[stable(feature = "core_impl_debug", since = "1.9.0")]
impl<I: fmt::Debug, U, F> fmt::Debug for FlatMap<I, U, F>
where
    U: IntoIterator<IntoIter: fmt::Debug>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FlatMap").field("inner", &self.inner).finish()
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<I: Iterator, U: IntoIterator, F> Iterator for FlatMap<I, U, F>
where
    F: FnMut(I::Item) -> U,
{
    type Item = U::Item;

    #[inline]
    fn next(&mut self) -> Option<U::Item> {
        self.inner.next()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }

    #[inline]
    fn try_fold<Acc, Fold, R>(&mut self, init: Acc, fold: Fold) -> R
    where
        Self: Sized,
        Fold: FnMut(Acc, Self::Item) -> R,
        R: Try<Output = Acc>,
    {
        self.inner.try_fold(init, fold)
    }

    #[inline]
    fn fold<Acc, Fold>(self, init: Acc, fold: Fold) -> Acc
    where
        Fold: FnMut(Acc, Self::Item) -> Acc,
    {
        self.inner.fold(init, fold)
    }

    #[inline]
    fn advance_by(&mut self, n: usize) -> Result<(), NonZero<usize>> {
        self.inner.advance_by(n)
    }

    #[inline]
    fn count(self) -> usize {
        self.inner.count()
    }

    #[inline]
    fn last(self) -> Option<Self::Item> {
        self.inner.last()
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<I: DoubleEndedIterator, U, F> DoubleEndedIterator for FlatMap<I, U, F>
where
    F: FnMut(I::Item) -> U,
    U: IntoIterator<IntoIter: DoubleEndedIterator>,
{
    #[inline]
    fn next_back(&mut self) -> Option<U::Item> {
        self.inner.next_back()
    }

    #[inline]
    fn try_rfold<Acc, Fold, R>(&mut self, init: Acc, fold: Fold) -> R
    where
        Self: Sized,
        Fold: FnMut(Acc, Self::Item) -> R,
        R: Try<Output = Acc>,
    {
        self.inner.try_rfold(init, fold)
    }

    #[inline]
    fn rfold<Acc, Fold>(self, init: Acc, fold: Fold) -> Acc
    where
        Fold: FnMut(Acc, Self::Item) -> Acc,
    {
        self.inner.rfold(init, fold)
    }

    #[inline]
    fn advance_back_by(&mut self, n: usize) -> Result<(), NonZero<usize>> {
        self.inner.advance_back_by(n)
    }
}

#[stable(feature = "fused", since = "1.26.0")]
impl<I, U, F> FusedIterator for FlatMap<I, U, F>
where
    I: FusedIterator,
    U: IntoIterator,
    F: FnMut(I::Item) -> U,
{
}

#[unstable(feature = "trusted_len", issue = "37572")]
unsafe impl<I, U, F> TrustedLen for FlatMap<I, U, F>
where
    I: Iterator,
    U: IntoIterator,
    F: FnMut(I::Item) -> U,
    FlattenCompat<Map<I, F>, <U as IntoIterator>::IntoIter>: TrustedLen,
{
}

#[unstable(issue = "none", feature = "inplace_iteration")]
unsafe impl<I, U, F> SourceIter for FlatMap<I, U, F>
where
    I: SourceIter + TrustedFused,
    U: IntoIterator,
{
    type Source = I::Source;

    #[inline]
    unsafe fn as_inner(&mut self) -> &mut I::Source {
        // SAFETY: 转发到具有相同要求的 unsafe 函数。
        unsafe { SourceIter::as_inner(&mut self.inner.iter) }
    }
}

/// 在“元素可转换为迭代器”的外层迭代器中，展平一层嵌套的迭代器。
///
/// 该 `struct` 由 [`Iterator`] 上的 [`flatten`] 方法创建。更多信息见该方法文档。
///
/// [`flatten`]: Iterator::flatten()
#[must_use = "iterators are lazy and do nothing unless consumed"]
#[stable(feature = "iterator_flatten", since = "1.29.0")]
pub struct Flatten<I: Iterator<Item: IntoIterator>> {
    inner: FlattenCompat<I, <I::Item as IntoIterator>::IntoIter>,
}

impl<I: Iterator<Item: IntoIterator>> Flatten<I> {
    pub(in super::super) fn new(iter: I) -> Flatten<I> {
        Flatten { inner: FlattenCompat::new(iter) }
    }
}

#[stable(feature = "iterator_flatten", since = "1.29.0")]
impl<I, U> fmt::Debug for Flatten<I>
where
    I: fmt::Debug + Iterator<Item: IntoIterator<IntoIter = U, Item = U::Item>>,
    U: fmt::Debug + Iterator,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Flatten").field("inner", &self.inner).finish()
    }
}

#[stable(feature = "iterator_flatten", since = "1.29.0")]
impl<I, U> Clone for Flatten<I>
where
    I: Clone + Iterator<Item: IntoIterator<IntoIter = U, Item = U::Item>>,
    U: Clone + Iterator,
{
    fn clone(&self) -> Self {
        Flatten { inner: self.inner.clone() }
    }
}

#[stable(feature = "iterator_flatten", since = "1.29.0")]
impl<I, U> Iterator for Flatten<I>
where
    I: Iterator<Item: IntoIterator<IntoIter = U, Item = U::Item>>,
    U: Iterator,
{
    type Item = U::Item;

    #[inline]
    fn next(&mut self) -> Option<U::Item> {
        self.inner.next()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }

    #[inline]
    fn try_fold<Acc, Fold, R>(&mut self, init: Acc, fold: Fold) -> R
    where
        Self: Sized,
        Fold: FnMut(Acc, Self::Item) -> R,
        R: Try<Output = Acc>,
    {
        self.inner.try_fold(init, fold)
    }

    #[inline]
    fn fold<Acc, Fold>(self, init: Acc, fold: Fold) -> Acc
    where
        Fold: FnMut(Acc, Self::Item) -> Acc,
    {
        self.inner.fold(init, fold)
    }

    #[inline]
    fn advance_by(&mut self, n: usize) -> Result<(), NonZero<usize>> {
        self.inner.advance_by(n)
    }

    #[inline]
    fn count(self) -> usize {
        self.inner.count()
    }

    #[inline]
    fn last(self) -> Option<Self::Item> {
        self.inner.last()
    }
}

#[stable(feature = "iterator_flatten", since = "1.29.0")]
impl<I, U> DoubleEndedIterator for Flatten<I>
where
    I: DoubleEndedIterator<Item: IntoIterator<IntoIter = U, Item = U::Item>>,
    U: DoubleEndedIterator,
{
    #[inline]
    fn next_back(&mut self) -> Option<U::Item> {
        self.inner.next_back()
    }

    #[inline]
    fn try_rfold<Acc, Fold, R>(&mut self, init: Acc, fold: Fold) -> R
    where
        Self: Sized,
        Fold: FnMut(Acc, Self::Item) -> R,
        R: Try<Output = Acc>,
    {
        self.inner.try_rfold(init, fold)
    }

    #[inline]
    fn rfold<Acc, Fold>(self, init: Acc, fold: Fold) -> Acc
    where
        Fold: FnMut(Acc, Self::Item) -> Acc,
    {
        self.inner.rfold(init, fold)
    }

    #[inline]
    fn advance_back_by(&mut self, n: usize) -> Result<(), NonZero<usize>> {
        self.inner.advance_back_by(n)
    }
}

#[stable(feature = "iterator_flatten", since = "1.29.0")]
impl<I, U> FusedIterator for Flatten<I>
where
    I: FusedIterator<Item: IntoIterator<IntoIter = U, Item = U::Item>>,
    U: Iterator,
{
}

#[unstable(feature = "trusted_len", issue = "37572")]
unsafe impl<I> TrustedLen for Flatten<I>
where
    I: Iterator<Item: IntoIterator>,
    FlattenCompat<I, <I::Item as IntoIterator>::IntoIter>: TrustedLen,
{
}

#[unstable(issue = "none", feature = "inplace_iteration")]
unsafe impl<I> SourceIter for Flatten<I>
where
    I: SourceIter + TrustedFused + Iterator,
    <I as Iterator>::Item: IntoIterator,
{
    type Source = I::Source;

    #[inline]
    unsafe fn as_inner(&mut self) -> &mut I::Source {
        // SAFETY: 转发到具有相同要求的 unsafe 函数。
        unsafe { SourceIter::as_inner(&mut self.inner.iter) }
    }
}

#[stable(feature = "default_iters", since = "1.70.0")]
impl<I> Default for Flatten<I>
where
    I: Default + Iterator<Item: IntoIterator>,
{
    /// 从 `I` 的默认值创建一个 `Flatten` 迭代器。
    ///
    /// ```
    /// # use core::slice;
    /// # use std::iter::Flatten;
    /// let iter: Flatten<slice::Iter<'_, [u8; 4]>> = Default::default();
    /// assert_eq!(iter.count(), 0);
    /// ```
    fn default() -> Self {
        Flatten::new(Default::default())
    }
}

/// `Flatten` 和 `FlatMap` 的真实逻辑；二者都只是委托给该类型。
#[derive(Clone, Debug)]
#[unstable(feature = "trusted_len", issue = "37572")]
struct FlattenCompat<I, U> {
    iter: Fuse<I>,
    frontiter: Option<U>,
    backiter: Option<U>,
}
impl<I, U> FlattenCompat<I, U>
where
    I: Iterator,
{
    /// 通过展平来适配迭代器，供 `flatten()` 和 `flat_map()` 使用。
    fn new(iter: I) -> FlattenCompat<I, U> {
        FlattenCompat { iter: iter.fuse(), frontiter: None, backiter: None }
    }
}

impl<I, U> FlattenCompat<I, U>
where
    I: Iterator<Item: IntoIterator<IntoIter = U>>,
{
    /// 对内层迭代器应用操作，把它们 fold 到 accumulator 中。
    ///
    /// 这里 fold 的对象是内层迭代器本身，而不是它们的元素。供 `fold`、`count` 和
    /// `last` 方法使用。
    #[inline]
    fn iter_fold<Acc, Fold>(self, mut acc: Acc, mut fold: Fold) -> Acc
    where
        Fold: FnMut(Acc, U) -> Acc,
    {
        #[inline]
        fn flatten<T: IntoIterator, Acc>(
            fold: &mut impl FnMut(Acc, T::IntoIter) -> Acc,
        ) -> impl FnMut(Acc, T) -> Acc + '_ {
            move |acc, iter| fold(acc, iter.into_iter())
        }

        if let Some(iter) = self.frontiter {
            acc = fold(acc, iter);
        }

        acc = self.iter.fold(acc, flatten(&mut fold));

        if let Some(iter) = self.backiter {
            acc = fold(acc, iter);
        }

        acc
    }

    /// 只要给定函数成功返回，就持续 fold 内层迭代器，并始终把最近的内层迭代器保存到
    /// `self.frontiter`。
    ///
    /// 这里 fold 的对象是内层迭代器本身，而不是它们的元素。供 `try_fold` 和
    /// `advance_by` 方法使用。
    #[inline]
    fn iter_try_fold<Acc, Fold, R>(&mut self, mut acc: Acc, mut fold: Fold) -> R
    where
        Fold: FnMut(Acc, &mut U) -> R,
        R: Try<Output = Acc>,
    {
        #[inline]
        fn flatten<'a, T: IntoIterator, Acc, R: Try<Output = Acc>>(
            frontiter: &'a mut Option<T::IntoIter>,
            fold: &'a mut impl FnMut(Acc, &mut T::IntoIter) -> R,
        ) -> impl FnMut(Acc, T) -> R + 'a {
            move |acc, iter| fold(acc, frontiter.insert(iter.into_iter()))
        }

        if let Some(iter) = &mut self.frontiter {
            acc = fold(acc, iter)?;
        }
        self.frontiter = None;

        acc = self.iter.try_fold(acc, flatten(&mut self.frontiter, &mut fold))?;
        self.frontiter = None;

        if let Some(iter) = &mut self.backiter {
            acc = fold(acc, iter)?;
        }
        self.backiter = None;

        try { acc }
    }
}

impl<I, U> FlattenCompat<I, U>
where
    I: DoubleEndedIterator<Item: IntoIterator<IntoIter = U>>,
{
    /// 从后端开始对内层迭代器应用操作，把它们 fold 到 accumulator 中。
    ///
    /// 这里 fold 的对象是内层迭代器本身，而不是它们的元素。供 `rfold` 方法使用。
    #[inline]
    fn iter_rfold<Acc, Fold>(self, mut acc: Acc, mut fold: Fold) -> Acc
    where
        Fold: FnMut(Acc, U) -> Acc,
    {
        #[inline]
        fn flatten<T: IntoIterator, Acc>(
            fold: &mut impl FnMut(Acc, T::IntoIter) -> Acc,
        ) -> impl FnMut(Acc, T) -> Acc + '_ {
            move |acc, iter| fold(acc, iter.into_iter())
        }

        if let Some(iter) = self.backiter {
            acc = fold(acc, iter);
        }

        acc = self.iter.rfold(acc, flatten(&mut fold));

        if let Some(iter) = self.frontiter {
            acc = fold(acc, iter);
        }

        acc
    }

    /// 只要给定函数成功返回，就按反向顺序持续 fold 内层迭代器，并始终把最近的内层
    /// 迭代器保存到 `self.backiter`。
    ///
    /// 这里 fold 的对象是内层迭代器本身，而不是它们的元素。供 `try_rfold` 和
    /// `advance_back_by` 方法使用。
    #[inline]
    fn iter_try_rfold<Acc, Fold, R>(&mut self, mut acc: Acc, mut fold: Fold) -> R
    where
        Fold: FnMut(Acc, &mut U) -> R,
        R: Try<Output = Acc>,
    {
        #[inline]
        fn flatten<'a, T: IntoIterator, Acc, R: Try>(
            backiter: &'a mut Option<T::IntoIter>,
            fold: &'a mut impl FnMut(Acc, &mut T::IntoIter) -> R,
        ) -> impl FnMut(Acc, T) -> R + 'a {
            move |acc, iter| fold(acc, backiter.insert(iter.into_iter()))
        }

        if let Some(iter) = &mut self.backiter {
            acc = fold(acc, iter)?;
        }
        self.backiter = None;

        acc = self.iter.try_rfold(acc, flatten(&mut self.backiter, &mut fold))?;
        self.backiter = None;

        if let Some(iter) = &mut self.frontiter {
            acc = fold(acc, iter)?;
        }
        self.frontiter = None;

        try { acc }
    }
}

// 另见下面的 `OneShot` 特化。
impl<I, U> Iterator for FlattenCompat<I, U>
where
    I: Iterator<Item: IntoIterator<IntoIter = U, Item = U::Item>>,
    U: Iterator,
{
    type Item = U::Item;

    #[inline]
    default fn next(&mut self) -> Option<U::Item> {
        loop {
            if let elt @ Some(_) = and_then_or_clear(&mut self.frontiter, Iterator::next) {
                return elt;
            }
            match self.iter.next() {
                None => return and_then_or_clear(&mut self.backiter, Iterator::next),
                Some(inner) => self.frontiter = Some(inner.into_iter()),
            }
        }
    }

    #[inline]
    default fn size_hint(&self) -> (usize, Option<usize>) {
        let (flo, fhi) = self.frontiter.as_ref().map_or((0, Some(0)), U::size_hint);
        let (blo, bhi) = self.backiter.as_ref().map_or((0, Some(0)), U::size_hint);
        let lo = flo.saturating_add(blo);

        if let Some(fixed_size) = <<I as Iterator>::Item as ConstSizeIntoIterator>::size() {
            let (lower, upper) = self.iter.size_hint();

            let lower = lower.saturating_mul(fixed_size).saturating_add(lo);
            let upper =
                try { fhi?.checked_add(bhi?)?.checked_add(fixed_size.checked_mul(upper?)?)? };

            return (lower, upper);
        }

        match (self.iter.size_hint(), fhi, bhi) {
            ((0, Some(0)), Some(a), Some(b)) => (lo, a.checked_add(b)),
            _ => (lo, None),
        }
    }

    #[inline]
    default fn try_fold<Acc, Fold, R>(&mut self, init: Acc, fold: Fold) -> R
    where
        Self: Sized,
        Fold: FnMut(Acc, Self::Item) -> R,
        R: Try<Output = Acc>,
    {
        #[inline]
        fn flatten<U: Iterator, Acc, R: Try<Output = Acc>>(
            mut fold: impl FnMut(Acc, U::Item) -> R,
        ) -> impl FnMut(Acc, &mut U) -> R {
            move |acc, iter| iter.try_fold(acc, &mut fold)
        }

        self.iter_try_fold(init, flatten(fold))
    }

    #[inline]
    default fn fold<Acc, Fold>(self, init: Acc, fold: Fold) -> Acc
    where
        Fold: FnMut(Acc, Self::Item) -> Acc,
    {
        #[inline]
        fn flatten<U: Iterator, Acc>(
            mut fold: impl FnMut(Acc, U::Item) -> Acc,
        ) -> impl FnMut(Acc, U) -> Acc {
            move |acc, iter| iter.fold(acc, &mut fold)
        }

        self.iter_fold(init, flatten(fold))
    }

    #[inline]
    #[rustc_inherit_overflow_checks]
    default fn advance_by(&mut self, n: usize) -> Result<(), NonZero<usize>> {
        #[inline]
        #[rustc_inherit_overflow_checks]
        fn advance<U: Iterator>(n: usize, iter: &mut U) -> ControlFlow<(), usize> {
            match iter.advance_by(n) {
                Ok(()) => ControlFlow::Break(()),
                Err(remaining) => ControlFlow::Continue(remaining.get()),
            }
        }

        match self.iter_try_fold(n, advance) {
            ControlFlow::Continue(remaining) => NonZero::new(remaining).map_or(Ok(()), Err),
            _ => Ok(()),
        }
    }

    #[inline]
    default fn count(self) -> usize {
        #[inline]
        #[rustc_inherit_overflow_checks]
        fn count<U: Iterator>(acc: usize, iter: U) -> usize {
            acc + iter.count()
        }

        self.iter_fold(0, count)
    }

    #[inline]
    default fn last(self) -> Option<Self::Item> {
        #[inline]
        fn last<U: Iterator>(last: Option<U::Item>, iter: U) -> Option<U::Item> {
            iter.last().or(last)
        }

        self.iter_fold(None, last)
    }
}

// 另见下面的 `OneShot` 特化。
impl<I, U> DoubleEndedIterator for FlattenCompat<I, U>
where
    I: DoubleEndedIterator<Item: IntoIterator<IntoIter = U, Item = U::Item>>,
    U: DoubleEndedIterator,
{
    #[inline]
    default fn next_back(&mut self) -> Option<U::Item> {
        loop {
            if let elt @ Some(_) = and_then_or_clear(&mut self.backiter, |b| b.next_back()) {
                return elt;
            }
            match self.iter.next_back() {
                None => return and_then_or_clear(&mut self.frontiter, |f| f.next_back()),
                Some(inner) => self.backiter = Some(inner.into_iter()),
            }
        }
    }

    #[inline]
    default fn try_rfold<Acc, Fold, R>(&mut self, init: Acc, fold: Fold) -> R
    where
        Self: Sized,
        Fold: FnMut(Acc, Self::Item) -> R,
        R: Try<Output = Acc>,
    {
        #[inline]
        fn flatten<U: DoubleEndedIterator, Acc, R: Try<Output = Acc>>(
            mut fold: impl FnMut(Acc, U::Item) -> R,
        ) -> impl FnMut(Acc, &mut U) -> R {
            move |acc, iter| iter.try_rfold(acc, &mut fold)
        }

        self.iter_try_rfold(init, flatten(fold))
    }

    #[inline]
    default fn rfold<Acc, Fold>(self, init: Acc, fold: Fold) -> Acc
    where
        Fold: FnMut(Acc, Self::Item) -> Acc,
    {
        #[inline]
        fn flatten<U: DoubleEndedIterator, Acc>(
            mut fold: impl FnMut(Acc, U::Item) -> Acc,
        ) -> impl FnMut(Acc, U) -> Acc {
            move |acc, iter| iter.rfold(acc, &mut fold)
        }

        self.iter_rfold(init, flatten(fold))
    }

    #[inline]
    #[rustc_inherit_overflow_checks]
    default fn advance_back_by(&mut self, n: usize) -> Result<(), NonZero<usize>> {
        #[inline]
        #[rustc_inherit_overflow_checks]
        fn advance<U: DoubleEndedIterator>(n: usize, iter: &mut U) -> ControlFlow<(), usize> {
            match iter.advance_back_by(n) {
                Ok(()) => ControlFlow::Break(()),
                Err(remaining) => ControlFlow::Continue(remaining.get()),
            }
        }

        match self.iter_try_rfold(n, advance) {
            ControlFlow::Continue(remaining) => NonZero::new(remaining).map_or(Ok(()), Err),
            _ => Ok(()),
        }
    }
}

unsafe impl<const N: usize, I, T> TrustedLen
    for FlattenCompat<I, <[T; N] as IntoIterator>::IntoIter>
where
    I: TrustedLen<Item = [T; N]>,
{
}

unsafe impl<'a, const N: usize, I, T> TrustedLen
    for FlattenCompat<I, <&'a [T; N] as IntoIterator>::IntoIter>
where
    I: TrustedLen<Item = &'a [T; N]>,
{
}

unsafe impl<'a, const N: usize, I, T> TrustedLen
    for FlattenCompat<I, <&'a mut [T; N] as IntoIterator>::IntoIter>
where
    I: TrustedLen<Item = &'a mut [T; N]>,
{
}

trait ConstSizeIntoIterator: IntoIterator {
    // FIXME(#31844): 特化机制支持后，把它改为关联常量
    fn size() -> Option<usize>;
}

impl<T> ConstSizeIntoIterator for T
where
    T: IntoIterator,
{
    #[inline]
    default fn size() -> Option<usize> {
        None
    }
}

impl<T, const N: usize> ConstSizeIntoIterator for [T; N] {
    #[inline]
    fn size() -> Option<usize> {
        Some(N)
    }
}

impl<T, const N: usize> ConstSizeIntoIterator for &[T; N] {
    #[inline]
    fn size() -> Option<usize> {
        Some(N)
    }
}

impl<T, const N: usize> ConstSizeIntoIterator for &mut [T; N] {
    #[inline]
    fn size() -> Option<usize> {
        Some(N)
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

/// 用于那些永远不会返回多于一个项的 iterator 类型的特化 trait。
///
/// 注意，仍然必须处理 iterator 在进入我们控制之前就已经耗尽的可能性。
#[rustc_specialization_trait]
trait OneShot {}

// 如果尚未被消耗，这些类型都恰好有一个项。
impl<T> OneShot for Once<T> {}
impl<F> OneShot for OnceWith<F> {}
impl<T> OneShot for array::IntoIter<T, 1> {}
impl<T> OneShot for option::IntoIter<T> {}
impl<T> OneShot for option::Iter<'_, T> {}
impl<T> OneShot for option::IterMut<'_, T> {}
impl<T> OneShot for result::IntoIter<T> {}
impl<T> OneShot for result::Iter<'_, T> {}
impl<T> OneShot for result::IterMut<'_, T> {}

// 这些类型总是空的，因此同样适合优化。
impl<T> OneShot for Empty<T> {}
impl<T> OneShot for array::IntoIter<T, 0> {}

// 这些适配器永远不会增加项数。
// （还有更多可能的类型，但目前这与上方的 `BoundedSize` 保持一致。）
impl<I: OneShot> OneShot for Cloned<I> {}
impl<I: OneShot> OneShot for Copied<I> {}
impl<I: OneShot, P> OneShot for Filter<I, P> {}
impl<I: OneShot, P> OneShot for FilterMap<I, P> {}
impl<I: OneShot, F> OneShot for Map<I, F> {}

// 这些泛型 impl 也会传递该性质
// （但除非把此 trait 暴露给 alloc，否则不能对 `Box<I>` 这样做）
impl<I: OneShot> OneShot for &mut I {}

#[inline]
fn into_item<I>(inner: I) -> Option<I::Item>
where
    I: IntoIterator<IntoIter: OneShot>,
{
    inner.into_iter().next()
}

#[inline]
fn flatten_one<I: IntoIterator<IntoIter: OneShot>, Acc>(
    mut fold: impl FnMut(Acc, I::Item) -> Acc,
) -> impl FnMut(Acc, I) -> Acc {
    move |acc, inner| match inner.into_iter().next() {
        Some(item) => fold(acc, item),
        None => acc,
    }
}

#[inline]
fn try_flatten_one<I: IntoIterator<IntoIter: OneShot>, Acc, R: Try<Output = Acc>>(
    mut fold: impl FnMut(Acc, I::Item) -> R,
) -> impl FnMut(Acc, I) -> R {
    move |acc, inner| match inner.into_iter().next() {
        Some(item) => fold(acc, item),
        None => try { acc },
    }
}

#[inline]
fn advance_by_one<I>(n: NonZero<usize>, inner: I) -> Option<NonZero<usize>>
where
    I: IntoIterator<IntoIter: OneShot>,
{
    match inner.into_iter().next() {
        Some(_) => NonZero::new(n.get() - 1),
        None => Some(n),
    }
}

// 特化: 当内层迭代器 `U` 永远不会返回超过一项时，`frontiter` 和 `backiter`
// 状态是浪费的，因为它们总是已经消耗了自己的项。因此在这个实现中，我们完全忽略它们，
// 只关注 `self.iter`，并且只调用一次内层 `U::next()`。
//
// 如果意外把它和更通用的实现混用，例如忘记特化某个方法，大多数情况下也没问题。
// 如果另一个实现设置了 front 或 back，这里不会看到它，但它无论如何也是空的；
// 如果另一个实现查找我们没有设置的 front 或 back，它只会看到 `None`(或之前的空迭代器)
// 并继续前进。
//
// 例外是 `advance_by(0)` 和 `advance_back_by(0)`，通用实现可能在不消耗项的情况下设置
// `frontiter` 或 `backiter`，因此这里 **必须** 覆盖这些方法。
impl<I, U> Iterator for FlattenCompat<I, U>
where
    I: Iterator<Item: IntoIterator<IntoIter = U, Item = U::Item>>,
    U: Iterator + OneShot,
{
    #[inline]
    fn next(&mut self) -> Option<U::Item> {
        while let Some(inner) = self.iter.next() {
            if let item @ Some(_) = inner.into_iter().next() {
                return item;
            }
        }
        None
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let (lower, upper) = self.iter.size_hint();
        match <I::Item as ConstSizeIntoIterator>::size() {
            Some(0) => (0, Some(0)),
            Some(1) => (lower, upper),
            _ => (0, upper),
        }
    }

    #[inline]
    fn try_fold<Acc, Fold, R>(&mut self, init: Acc, fold: Fold) -> R
    where
        Self: Sized,
        Fold: FnMut(Acc, Self::Item) -> R,
        R: Try<Output = Acc>,
    {
        self.iter.try_fold(init, try_flatten_one(fold))
    }

    #[inline]
    fn fold<Acc, Fold>(self, init: Acc, fold: Fold) -> Acc
    where
        Fold: FnMut(Acc, Self::Item) -> Acc,
    {
        self.iter.fold(init, flatten_one(fold))
    }

    #[inline]
    fn advance_by(&mut self, n: usize) -> Result<(), NonZero<usize>> {
        if let Some(n) = NonZero::new(n) {
            self.iter.try_fold(n, advance_by_one).map_or(Ok(()), Err)
        } else {
            // 只推进外层迭代器。
            self.iter.advance_by(0)
        }
    }

    #[inline]
    fn count(self) -> usize {
        self.iter.filter_map(into_item).count()
    }

    #[inline]
    fn last(self) -> Option<Self::Item> {
        self.iter.filter_map(into_item).last()
    }
}

// 注意: 实际上并不关心 `U: DoubleEndedIterator`，因为对一次性迭代器来说正向和
// 反向相同；但为了匹配默认特化，必须保留这个约束。
impl<I, U> DoubleEndedIterator for FlattenCompat<I, U>
where
    I: DoubleEndedIterator<Item: IntoIterator<IntoIter = U, Item = U::Item>>,
    U: DoubleEndedIterator + OneShot,
{
    #[inline]
    fn next_back(&mut self) -> Option<U::Item> {
        while let Some(inner) = self.iter.next_back() {
            if let item @ Some(_) = inner.into_iter().next() {
                return item;
            }
        }
        None
    }

    #[inline]
    fn try_rfold<Acc, Fold, R>(&mut self, init: Acc, fold: Fold) -> R
    where
        Self: Sized,
        Fold: FnMut(Acc, Self::Item) -> R,
        R: Try<Output = Acc>,
    {
        self.iter.try_rfold(init, try_flatten_one(fold))
    }

    #[inline]
    fn rfold<Acc, Fold>(self, init: Acc, fold: Fold) -> Acc
    where
        Fold: FnMut(Acc, Self::Item) -> Acc,
    {
        self.iter.rfold(init, flatten_one(fold))
    }

    #[inline]
    fn advance_back_by(&mut self, n: usize) -> Result<(), NonZero<usize>> {
        if let Some(n) = NonZero::new(n) {
            self.iter.try_rfold(n, advance_by_one).map_or(Ok(()), Err)
        } else {
            // 只推进外层迭代器。
            self.iter.advance_back_by(0)
        }
    }
}
