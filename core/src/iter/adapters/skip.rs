use crate::intrinsics::unlikely;
use crate::iter::adapters::SourceIter;
use crate::iter::adapters::zip::try_get_unchecked;
use crate::iter::{
    FusedIterator, InPlaceIterable, TrustedFused, TrustedLen, TrustedRandomAccess,
    TrustedRandomAccessNoCoerce,
};
use crate::num::NonZero;
use crate::ops::{ControlFlow, Try};

/// 跳过 `iter` 前 `n` 个元素的迭代器。
///
/// 该 `struct` 由 [`Iterator`] 上的 [`skip`] 方法创建。更多信息见该方法文档。
///
/// [`skip`]: Iterator::skip
/// [`Iterator`]: trait.Iterator.html
#[derive(Clone, Debug)]
#[must_use = "iterators are lazy and do nothing unless consumed"]
#[stable(feature = "rust1", since = "1.0.0")]
pub struct Skip<I> {
    iter: I,
    n: usize,
}

impl<I> Skip<I> {
    pub(in crate::iter) fn new(iter: I, n: usize) -> Skip<I> {
        Skip { iter, n }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<I> Iterator for Skip<I>
where
    I: Iterator,
{
    type Item = <I as Iterator>::Item;

    #[inline]
    fn next(&mut self) -> Option<I::Item> {
        if unlikely(self.n > 0) {
            self.iter.nth(crate::mem::take(&mut self.n))
        } else {
            self.iter.next()
        }
    }

    #[inline]
    fn nth(&mut self, n: usize) -> Option<I::Item> {
        if self.n > 0 {
            let skip: usize = crate::mem::take(&mut self.n);
            // 使用 checked add 处理溢出情况。
            let n = match skip.checked_add(n) {
                Some(nth) => nth,
                None => {
                    // 发生溢出时，先加载 skip 值，再加载 `n`。由于要迭代的元素数量超过
                    // `usize::MAX`，这里拆成两次 `nth` 调用，其中 `skip` 的 `nth` 结果会被丢弃。
                    self.iter.nth(skip - 1)?;
                    n
                }
            };
            // 加载包含 skip 后的第 n 个元素。
            self.iter.nth(n)
        } else {
            self.iter.nth(n)
        }
    }

    #[inline]
    fn count(mut self) -> usize {
        if self.n > 0 {
            // nth(n) 会跳过 n+1 项。
            if self.iter.nth(self.n - 1).is_none() {
                return 0;
            }
        }
        self.iter.count()
    }

    #[inline]
    fn last(mut self) -> Option<I::Item> {
        if self.n > 0 {
            // nth(n) 会跳过 n+1 项。
            self.iter.nth(self.n - 1)?;
        }
        self.iter.last()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let (lower, upper) = self.iter.size_hint();

        let lower = lower.saturating_sub(self.n);
        let upper = match upper {
            Some(x) => Some(x.saturating_sub(self.n)),
            None => None,
        };

        (lower, upper)
    }

    #[inline]
    fn try_fold<Acc, Fold, R>(&mut self, init: Acc, fold: Fold) -> R
    where
        Self: Sized,
        Fold: FnMut(Acc, Self::Item) -> R,
        R: Try<Output = Acc>,
    {
        let n = self.n;
        self.n = 0;
        if n > 0 {
            // nth(n) 会跳过 n+1 项。
            if self.iter.nth(n - 1).is_none() {
                return try { init };
            }
        }
        self.iter.try_fold(init, fold)
    }

    #[inline]
    fn fold<Acc, Fold>(mut self, init: Acc, fold: Fold) -> Acc
    where
        Fold: FnMut(Acc, Self::Item) -> Acc,
    {
        if self.n > 0 {
            // nth(n) skips n+1
            if self.iter.nth(self.n - 1).is_none() {
                return init;
            }
        }
        self.iter.fold(init, fold)
    }

    #[inline]
    #[rustc_inherit_overflow_checks]
    fn advance_by(&mut self, mut n: usize) -> Result<(), NonZero<usize>> {
        let skip_inner = self.n;
        let skip_and_advance = skip_inner.saturating_add(n);

        let remainder = match self.iter.advance_by(skip_and_advance) {
            Ok(()) => 0,
            Err(n) => n.get(),
        };
        let advanced_inner = skip_and_advance - remainder;
        n -= advanced_inner.saturating_sub(skip_inner);
        self.n = self.n.saturating_sub(advanced_inner);

        // skip_and_advance 可能已经饱和。
        if unlikely(remainder == 0 && n > 0) {
            n = match self.iter.advance_by(n) {
                Ok(()) => 0,
                Err(n) => n.get(),
            }
        }

        NonZero::new(n).map_or(Ok(()), Err)
    }

    #[doc(hidden)]
    unsafe fn __iterator_get_unchecked(&mut self, idx: usize) -> Self::Item
    where
        Self: TrustedRandomAccessNoCoerce,
    {
        // SAFETY: 调用方必须维护 `Iterator::__iterator_get_unchecked` 的契约。
        //
        // 传入索引 0 时 drop 被跳过的前缀是安全的，因为:
        // * 调用方传入索引 0 意味着内层迭代器拥有超过 `self.n` 个元素；
        // * TRA 契约要求 get_unchecked 只会被调用一次(除非元素可复制)；
        // * 这不会与原地迭代冲突，因为在向前缀使用的存储写入内容之前，必须先访问索引 0。
        unsafe {
            if Self::MAY_HAVE_SIDE_EFFECT && idx == 0 {
                for skipped_idx in 0..self.n {
                    drop(try_get_unchecked(&mut self.iter, skipped_idx));
                }
            }

            try_get_unchecked(&mut self.iter, idx + self.n)
        }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<I> ExactSizeIterator for Skip<I> where I: ExactSizeIterator {}

#[stable(feature = "double_ended_skip_iterator", since = "1.9.0")]
impl<I> DoubleEndedIterator for Skip<I>
where
    I: DoubleEndedIterator + ExactSizeIterator,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.len() > 0 { self.iter.next_back() } else { None }
    }

    #[inline]
    fn nth_back(&mut self, n: usize) -> Option<I::Item> {
        let len = self.len();
        if n < len {
            self.iter.nth_back(n)
        } else {
            if len > 0 {
                // 消耗原始迭代器。
                self.iter.nth_back(len - 1);
            }
            None
        }
    }

    fn try_rfold<Acc, Fold, R>(&mut self, init: Acc, fold: Fold) -> R
    where
        Self: Sized,
        Fold: FnMut(Acc, Self::Item) -> R,
        R: Try<Output = Acc>,
    {
        fn check<T, Acc, R: Try<Output = Acc>>(
            mut n: usize,
            mut fold: impl FnMut(Acc, T) -> R,
        ) -> impl FnMut(Acc, T) -> ControlFlow<R, Acc> {
            move |acc, x| {
                n -= 1;
                let r = fold(acc, x);
                if n == 0 { ControlFlow::Break(r) } else { ControlFlow::from_try(r) }
            }
        }

        let n = self.len();
        if n == 0 { try { init } } else { self.iter.try_rfold(init, check(n, fold)).into_try() }
    }

    impl_fold_via_try_fold! { rfold -> try_rfold }

    #[inline]
    fn advance_back_by(&mut self, n: usize) -> Result<(), NonZero<usize>> {
        let min = crate::cmp::min(self.len(), n);
        let rem = self.iter.advance_back_by(min);
        assert!(rem.is_ok(), "ExactSizeIterator contract violation");
        NonZero::new(n - min).map_or(Ok(()), Err)
    }
}

#[stable(feature = "fused", since = "1.26.0")]
impl<I> FusedIterator for Skip<I> where I: FusedIterator {}

#[unstable(issue = "none", feature = "trusted_fused")]
unsafe impl<I: TrustedFused> TrustedFused for Skip<I> {}

#[unstable(issue = "none", feature = "inplace_iteration")]
unsafe impl<I> SourceIter for Skip<I>
where
    I: SourceIter,
{
    type Source = I::Source;

    #[inline]
    unsafe fn as_inner(&mut self) -> &mut I::Source {
        // SAFETY: 转发到具有相同要求的 unsafe 函数。
        unsafe { SourceIter::as_inner(&mut self.iter) }
    }
}

#[unstable(issue = "none", feature = "inplace_iteration")]
unsafe impl<I: InPlaceIterable> InPlaceIterable for Skip<I> {
    const EXPAND_BY: Option<NonZero<usize>> = I::EXPAND_BY;
    const MERGE_BY: Option<NonZero<usize>> = I::MERGE_BY;
}

#[doc(hidden)]
#[unstable(feature = "trusted_random_access", issue = "none")]
unsafe impl<I> TrustedRandomAccess for Skip<I> where I: TrustedRandomAccess {}

#[doc(hidden)]
#[unstable(feature = "trusted_random_access", issue = "none")]
unsafe impl<I> TrustedRandomAccessNoCoerce for Skip<I>
where
    I: TrustedRandomAccessNoCoerce,
{
    const MAY_HAVE_SIDE_EFFECT: bool = I::MAY_HAVE_SIDE_EFFECT;
}

// SAFETY: 该适配器只会缩短长度。TrustedLen 要求正确计算上界。
// 只有当内部 iterator 的上界永远不是 `None` 时，才能满足这些要求。
// I: TrustedRandomAccess 正好提供这个保证，而 I: TrustedLen 不提供。
#[unstable(feature = "trusted_len", issue = "37572")]
unsafe impl<I> TrustedLen for Skip<I> where I: Iterator + TrustedRandomAccess {}
