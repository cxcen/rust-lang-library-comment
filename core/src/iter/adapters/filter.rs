use core::array;
use core::mem::MaybeUninit;
use core::ops::ControlFlow;

use crate::fmt;
use crate::iter::adapters::SourceIter;
use crate::iter::{FusedIterator, InPlaceIterable, TrustedFused, TrustedLen};
use crate::num::NonZero;
use crate::ops::Try;

/// 使用 `predicate` 过滤 `iter` 元素的迭代器。
///
/// 该 `struct` 由 [`Iterator`] 上的 [`filter`] 方法创建。更多信息见该方法文档。
///
/// [`filter`]: Iterator::filter
/// [`Iterator`]: trait.Iterator.html
#[must_use = "iterators are lazy and do nothing unless consumed"]
#[stable(feature = "rust1", since = "1.0.0")]
#[derive(Clone)]
pub struct Filter<I, P> {
    // 供 `SplitWhitespace` 和 `SplitAsciiWhitespace` 的 `as_str` 方法使用。
    pub(crate) iter: I,
    predicate: P,
}
impl<I, P> Filter<I, P> {
    pub(in crate::iter) fn new(iter: I, predicate: P) -> Filter<I, P> {
        Filter { iter, predicate }
    }
}

impl<I, P> Filter<I, P>
where
    I: Iterator,
    P: FnMut(&I::Item) -> bool,
{
    #[inline]
    fn next_chunk_dropless<const N: usize>(
        &mut self,
    ) -> Result<[I::Item; N], array::IntoIter<I::Item, N>> {
        let mut array: [MaybeUninit<I::Item>; N] = [const { MaybeUninit::uninit() }; N];
        let mut initialized = 0;

        let result = self.iter.try_for_each(|element| {
            let idx = initialized;
            // 无分支索引更新配合“即使元素被过滤也无条件复制值”，可以减少循环中的
            // 分支和依赖。
            initialized = idx + (self.predicate)(&element) as usize;
            // SAFETY: 循环条件保证索引在边界内。
            unsafe { array.get_unchecked_mut(idx) }.write(element);

            if initialized < N { ControlFlow::Continue(()) } else { ControlFlow::Break(()) }
        });

        match result {
            ControlFlow::Break(()) => {
                // SAFETY: 只有在数组已经完全初始化时，上面的循环才会显式 break。
                Ok(unsafe { MaybeUninit::array_assume_init(array) })
            }
            ControlFlow::Continue(()) => {
                // SAFETY: 到达 N 个元素时循环会 break，因此该范围在边界内。
                Err(unsafe { array::IntoIter::new_unchecked(array, 0..initialized) })
            }
        }
    }
}

#[stable(feature = "core_impl_debug", since = "1.9.0")]
impl<I: fmt::Debug, P> fmt::Debug for Filter<I, P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Filter").field("iter", &self.iter).finish()
    }
}

fn filter_fold<T, Acc>(
    mut predicate: impl FnMut(&T) -> bool,
    mut fold: impl FnMut(Acc, T) -> Acc,
) -> impl FnMut(Acc, T) -> Acc {
    move |acc, item| if predicate(&item) { fold(acc, item) } else { acc }
}

fn filter_try_fold<'a, T, Acc, R: Try<Output = Acc>>(
    predicate: &'a mut impl FnMut(&T) -> bool,
    mut fold: impl FnMut(Acc, T) -> R + 'a,
) -> impl FnMut(Acc, T) -> R + 'a {
    move |acc, item| if predicate(&item) { fold(acc, item) } else { try { acc } }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<I: Iterator, P> Iterator for Filter<I, P>
where
    P: FnMut(&I::Item) -> bool,
{
    type Item = I::Item;

    #[inline]
    fn next(&mut self) -> Option<I::Item> {
        self.iter.find(&mut self.predicate)
    }

    #[inline]
    fn next_chunk<const N: usize>(
        &mut self,
    ) -> Result<[Self::Item; N], array::IntoIter<Self::Item, N>> {
        // 避免为死分支生成代码。
        let fun = const {
            if crate::mem::needs_drop::<I::Item>() {
                array::iter_next_chunk::<I::Item, N>
            } else {
                Self::next_chunk_dropless::<N>
            }
        };

        fun(self)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let (_, upper) = self.iter.size_hint();
        (0, upper) // 由于 predicate 的存在，无法知道下界。
    }

    // 这个特殊情况允许编译器把 `.filter(_).count()` 做成无分支实现。除非分支预测完美
    // (一般情况下不可达到)，否则它在超过 90% 的情况中会快得多(几乎覆盖所有真实工作负载)，
    // 在其余情况中也只会慢一点点。
    //
    // 因此，该 specialization 允许我们写 `.filter(p).count()`；否则可能会写成
    // `.map(|x| p(x) as usize).sum()`，后者可读性更差，也更不兼容 Rust 1.10 之前版本。
    //
    // 使用无分支版本也会简化 LLVM 字节码，从而给 LLVM 优化留下更多空间。
    #[inline]
    fn count(self) -> usize {
        #[inline]
        fn to_usize<T>(mut predicate: impl FnMut(&T) -> bool) -> impl FnMut(T) -> usize {
            move |x| predicate(&x) as usize
        }

        let before = self.iter.size_hint().1.unwrap_or(usize::MAX);
        let total = self.iter.map(to_usize(self.predicate)).sum();
        // SAFETY: `total` 和 `before` 来自同一个 `I` 类型迭代器。
        unsafe {
            <I as SpecAssumeCount>::assume_count_le_upper_bound(total, before);
        }
        total
    }

    #[inline]
    fn try_fold<Acc, Fold, R>(&mut self, init: Acc, fold: Fold) -> R
    where
        Self: Sized,
        Fold: FnMut(Acc, Self::Item) -> R,
        R: Try<Output = Acc>,
    {
        self.iter.try_fold(init, filter_try_fold(&mut self.predicate, fold))
    }

    #[inline]
    fn fold<Acc, Fold>(self, init: Acc, fold: Fold) -> Acc
    where
        Fold: FnMut(Acc, Self::Item) -> Acc,
    {
        self.iter.fold(init, filter_fold(self.predicate, fold))
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<I: DoubleEndedIterator, P> DoubleEndedIterator for Filter<I, P>
where
    P: FnMut(&I::Item) -> bool,
{
    #[inline]
    fn next_back(&mut self) -> Option<I::Item> {
        self.iter.rfind(&mut self.predicate)
    }

    #[inline]
    fn try_rfold<Acc, Fold, R>(&mut self, init: Acc, fold: Fold) -> R
    where
        Self: Sized,
        Fold: FnMut(Acc, Self::Item) -> R,
        R: Try<Output = Acc>,
    {
        self.iter.try_rfold(init, filter_try_fold(&mut self.predicate, fold))
    }

    #[inline]
    fn rfold<Acc, Fold>(self, init: Acc, fold: Fold) -> Acc
    where
        Fold: FnMut(Acc, Self::Item) -> Acc,
    {
        self.iter.rfold(init, filter_fold(self.predicate, fold))
    }
}

#[stable(feature = "fused", since = "1.26.0")]
impl<I: FusedIterator, P> FusedIterator for Filter<I, P> where P: FnMut(&I::Item) -> bool {}

#[unstable(issue = "none", feature = "trusted_fused")]
unsafe impl<I: TrustedFused, F> TrustedFused for Filter<I, F> {}

#[unstable(issue = "none", feature = "inplace_iteration")]
unsafe impl<P, I> SourceIter for Filter<I, P>
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
unsafe impl<I: InPlaceIterable, P> InPlaceIterable for Filter<I, P> {
    const EXPAND_BY: Option<NonZero<usize>> = I::EXPAND_BY;
    const MERGE_BY: Option<NonZero<usize>> = I::MERGE_BY;
}

trait SpecAssumeCount {
    /// # 安全性(Safety）
    ///
    /// `count` 必须是从迭代器中实际读取的项数。
    ///
    /// `upper` 必须满足以下之一:
    /// - 来自该迭代器的 `size_hint().1`；
    /// - 是 `usize::MAX`，这种情况下断言退化为空操作。
    unsafe fn assume_count_le_upper_bound(count: usize, upper: usize);
}

impl<I: Iterator> SpecAssumeCount for I {
    #[inline]
    #[rustc_inherit_overflow_checks]
    default unsafe fn assume_count_le_upper_bound(count: usize, upper: usize) {
        // 默认实现中不能为了 soundness 信任 `upper`，
        // 因为它来自不可信的 `size_hint`。

        // debug 模式下顺便检查 size_hint 没有过小。
        let _ = upper - count;
    }
}

impl<I: TrustedLen> SpecAssumeCount for I {
    #[inline]
    unsafe fn assume_count_le_upper_bound(count: usize, upper: usize) {
        // SAFETY: `upper` 可信，因为它来自 `TrustedLen` 迭代器。
        unsafe { crate::hint::assert_unchecked(count <= upper) }
    }
}
