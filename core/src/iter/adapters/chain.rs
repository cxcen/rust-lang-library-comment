use crate::iter::{FusedIterator, TrustedLen};
use crate::num::NonZero;
use crate::ops::Try;

/// 把两个迭代器按顺序链接起来的迭代器。
///
/// 该 `struct` 由 [`chain`] 或 [`Iterator::chain`] 创建。更多信息见它们的文档。
///
/// # 示例
///
/// ```
/// use std::iter::Chain;
/// use std::slice::Iter;
///
/// let a1 = [1, 2, 3];
/// let a2 = [4, 5, 6];
/// let iter: Chain<Iter<'_, _>, Iter<'_, _>> = a1.iter().chain(a2.iter());
/// ```
#[derive(Clone, Debug)]
#[must_use = "iterators are lazy and do nothing unless consumed"]
#[stable(feature = "rust1", since = "1.0.0")]
pub struct Chain<A, B> {
    // 这里用 `Option` 达成“fused”效果，因此不需要额外状态来跟踪哪一部分已经耗尽；
    // 也可能利用 `None` 获得 niche 布局。这里不使用真正的 `Fuse` 适配器，因为它对
    // `FusedIterator` 的 specialization 会无条件深入迭代器；对嵌套 chain 之类结构，
    // 反复访问会比较昂贵。给 `Chain` 增加更多迭代器层也会损害编译器性能。
    //
    // 根据正向或反向迭代，只有“第一个”被耗尽的迭代器会实际设为 `None`。
    // 如果混合两个方向，则两边都可能是 `None`。
    a: Option<A>,
    b: Option<B>,
}
impl<A, B> Chain<A, B> {
    pub(in super::super) fn new(a: A, b: B) -> Chain<A, B> {
        Chain { a: Some(a), b: Some(b) }
    }
}

/// 把参数转换为迭代器，并按顺序链接起来。
///
/// 更多信息见 [`Iterator::chain`] 文档。
///
/// # 示例
///
/// ```
/// use std::iter::chain;
///
/// let a = [1, 2, 3];
/// let b = [4, 5, 6];
///
/// let mut iter = chain(a, b);
///
/// assert_eq!(iter.next(), Some(1));
/// assert_eq!(iter.next(), Some(2));
/// assert_eq!(iter.next(), Some(3));
/// assert_eq!(iter.next(), Some(4));
/// assert_eq!(iter.next(), Some(5));
/// assert_eq!(iter.next(), Some(6));
/// assert_eq!(iter.next(), None);
/// ```
#[stable(feature = "iter_chain", since = "1.91.0")]
pub fn chain<A, B>(a: A, b: B) -> Chain<A::IntoIter, B::IntoIter>
where
    A: IntoIterator,
    B: IntoIterator<Item = A::Item>,
{
    Chain::new(a.into_iter(), b.into_iter())
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<A, B> Iterator for Chain<A, B>
where
    A: Iterator,
    B: Iterator<Item = A::Item>,
{
    type Item = A::Item;

    #[inline]
    fn next(&mut self) -> Option<A::Item> {
        and_then_or_clear(&mut self.a, Iterator::next).or_else(|| self.b.as_mut()?.next())
    }

    #[inline]
    #[rustc_inherit_overflow_checks]
    fn count(self) -> usize {
        let a_count = match self.a {
            Some(a) => a.count(),
            None => 0,
        };
        let b_count = match self.b {
            Some(b) => b.count(),
            None => 0,
        };
        a_count + b_count
    }

    fn try_fold<Acc, F, R>(&mut self, mut acc: Acc, mut f: F) -> R
    where
        Self: Sized,
        F: FnMut(Acc, Self::Item) -> R,
        R: Try<Output = Acc>,
    {
        if let Some(ref mut a) = self.a {
            acc = a.try_fold(acc, &mut f)?;
            self.a = None;
        }
        if let Some(ref mut b) = self.b {
            acc = b.try_fold(acc, f)?;
            // 不 fuse 第二个迭代器。
        }
        try { acc }
    }

    fn fold<Acc, F>(self, mut acc: Acc, mut f: F) -> Acc
    where
        F: FnMut(Acc, Self::Item) -> Acc,
    {
        if let Some(a) = self.a {
            acc = a.fold(acc, &mut f);
        }
        if let Some(b) = self.b {
            acc = b.fold(acc, f);
        }
        acc
    }

    #[inline]
    fn advance_by(&mut self, mut n: usize) -> Result<(), NonZero<usize>> {
        if let Some(ref mut a) = self.a {
            n = match a.advance_by(n) {
                Ok(()) => return Ok(()),
                Err(k) => k.get(),
            };
            self.a = None;
        }

        if let Some(ref mut b) = self.b {
            return b.advance_by(n);
            // 不 fuse 第二个迭代器。
        }

        NonZero::new(n).map_or(Ok(()), Err)
    }

    #[inline]
    fn nth(&mut self, mut n: usize) -> Option<Self::Item> {
        if let Some(ref mut a) = self.a {
            n = match a.advance_by(n) {
                Ok(()) => match a.next() {
                    None => 0,
                    x => return x,
                },
                Err(k) => k.get(),
            };

            self.a = None;
        }

        self.b.as_mut()?.nth(n)
    }

    #[inline]
    fn find<P>(&mut self, mut predicate: P) -> Option<Self::Item>
    where
        P: FnMut(&Self::Item) -> bool,
    {
        and_then_or_clear(&mut self.a, |a| a.find(&mut predicate))
            .or_else(|| self.b.as_mut()?.find(predicate))
    }

    #[inline]
    fn last(self) -> Option<A::Item> {
        // 必须先耗尽 a，再耗尽 b。
        let a_last = self.a.and_then(Iterator::last);
        let b_last = self.b.and_then(Iterator::last);
        b_last.or(a_last)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        match self {
            Chain { a: Some(a), b: Some(b) } => {
                let (a_lower, a_upper) = a.size_hint();
                let (b_lower, b_upper) = b.size_hint();

                let lower = a_lower.saturating_add(b_lower);

                let upper = match (a_upper, b_upper) {
                    (Some(x), Some(y)) => x.checked_add(y),
                    _ => None,
                };

                (lower, upper)
            }
            Chain { a: Some(a), b: None } => a.size_hint(),
            Chain { a: None, b: Some(b) } => b.size_hint(),
            Chain { a: None, b: None } => (0, Some(0)),
        }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<A, B> DoubleEndedIterator for Chain<A, B>
where
    A: DoubleEndedIterator,
    B: DoubleEndedIterator<Item = A::Item>,
{
    #[inline]
    fn next_back(&mut self) -> Option<A::Item> {
        and_then_or_clear(&mut self.b, |b| b.next_back()).or_else(|| self.a.as_mut()?.next_back())
    }

    #[inline]
    fn advance_back_by(&mut self, mut n: usize) -> Result<(), NonZero<usize>> {
        if let Some(ref mut b) = self.b {
            n = match b.advance_back_by(n) {
                Ok(()) => return Ok(()),
                Err(k) => k.get(),
            };
            self.b = None;
        }

        if let Some(ref mut a) = self.a {
            return a.advance_back_by(n);
            // 不 fuse 第二个迭代器。
        }

        NonZero::new(n).map_or(Ok(()), Err)
    }

    #[inline]
    fn nth_back(&mut self, mut n: usize) -> Option<Self::Item> {
        if let Some(ref mut b) = self.b {
            n = match b.advance_back_by(n) {
                Ok(()) => match b.next_back() {
                    None => 0,
                    x => return x,
                },
                Err(k) => k.get(),
            };

            self.b = None;
        }

        self.a.as_mut()?.nth_back(n)
    }

    #[inline]
    fn rfind<P>(&mut self, mut predicate: P) -> Option<Self::Item>
    where
        P: FnMut(&Self::Item) -> bool,
    {
        and_then_or_clear(&mut self.b, |b| b.rfind(&mut predicate))
            .or_else(|| self.a.as_mut()?.rfind(predicate))
    }

    fn try_rfold<Acc, F, R>(&mut self, mut acc: Acc, mut f: F) -> R
    where
        Self: Sized,
        F: FnMut(Acc, Self::Item) -> R,
        R: Try<Output = Acc>,
    {
        if let Some(ref mut b) = self.b {
            acc = b.try_rfold(acc, &mut f)?;
            self.b = None;
        }
        if let Some(ref mut a) = self.a {
            acc = a.try_rfold(acc, f)?;
            // 不 fuse 第二个迭代器。
        }
        try { acc }
    }

    fn rfold<Acc, F>(self, mut acc: Acc, mut f: F) -> Acc
    where
        F: FnMut(Acc, Self::Item) -> Acc,
    {
        if let Some(b) = self.b {
            acc = b.rfold(acc, &mut f);
        }
        if let Some(a) = self.a {
            acc = a.rfold(acc, f);
        }
        acc
    }
}

// 注意: 要正确处理双端迭代器，*两者*都必须 fused。
#[stable(feature = "fused", since = "1.26.0")]
impl<A, B> FusedIterator for Chain<A, B>
where
    A: FusedIterator,
    B: FusedIterator<Item = A::Item>,
{
}

#[unstable(feature = "trusted_len", issue = "37572")]
unsafe impl<A, B> TrustedLen for Chain<A, B>
where
    A: TrustedLen,
    B: TrustedLen<Item = A::Item>,
{
}

#[stable(feature = "default_iters", since = "1.70.0")]
impl<A: Default, B: Default> Default for Chain<A, B> {
    /// 从 `A` 和 `B` 的默认值创建一个 `Chain`。
    ///
    /// ```
    /// # use core::iter::Chain;
    /// # use core::slice;
    /// # use std::collections::{btree_set, BTreeSet};
    /// # use std::mem;
    /// struct Foo<'a>(Chain<slice::Iter<'a, u8>, btree_set::Iter<'a, u8>>);
    ///
    /// let set = BTreeSet::<u8>::new();
    /// let slice: &[u8] = &[];
    /// let mut foo = Foo(slice.iter().chain(set.iter()));
    ///
    /// // take 需要 `Default`。
    /// let _: Chain<_, _> = mem::take(&mut foo.0);
    /// ```
    fn default() -> Self {
        Chain::new(Default::default(), Default::default())
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
