use crate::iter::adapters::SourceIter;
use crate::iter::{FusedIterator, InPlaceIterable, TrustedFused};
use crate::mem::{ManuallyDrop, MaybeUninit};
use crate::num::NonZero;
use crate::ops::{ControlFlow, Try};
use crate::{array, fmt};

/// 使用 `f` 对 `iter` 中元素同时执行过滤和映射的迭代器。
///
/// 该 `struct` 由 [`Iterator`] 上的 [`filter_map`] 方法创建。更多信息见该方法文档。
///
/// [`filter_map`]: Iterator::filter_map
/// [`Iterator`]: trait.Iterator.html
#[must_use = "iterators are lazy and do nothing unless consumed"]
#[stable(feature = "rust1", since = "1.0.0")]
#[derive(Clone)]
pub struct FilterMap<I, F> {
    iter: I,
    f: F,
}
impl<I, F> FilterMap<I, F> {
    pub(in crate::iter) fn new(iter: I, f: F) -> FilterMap<I, F> {
        FilterMap { iter, f }
    }
}

#[stable(feature = "core_impl_debug", since = "1.9.0")]
impl<I: fmt::Debug, F> fmt::Debug for FilterMap<I, F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FilterMap").field("iter", &self.iter).finish()
    }
}

fn filter_map_fold<T, B, Acc>(
    mut f: impl FnMut(T) -> Option<B>,
    mut fold: impl FnMut(Acc, B) -> Acc,
) -> impl FnMut(Acc, T) -> Acc {
    move |acc, item| match f(item) {
        Some(x) => fold(acc, x),
        None => acc,
    }
}

fn filter_map_try_fold<'a, T, B, Acc, R: Try<Output = Acc>>(
    f: &'a mut impl FnMut(T) -> Option<B>,
    mut fold: impl FnMut(Acc, B) -> R + 'a,
) -> impl FnMut(Acc, T) -> R + 'a {
    move |acc, item| match f(item) {
        Some(x) => fold(acc, x),
        None => try { acc },
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<B, I: Iterator, F> Iterator for FilterMap<I, F>
where
    F: FnMut(I::Item) -> Option<B>,
{
    type Item = B;

    #[inline]
    fn next(&mut self) -> Option<B> {
        self.iter.find_map(&mut self.f)
    }

    #[inline]
    fn next_chunk<const N: usize>(
        &mut self,
    ) -> Result<[Self::Item; N], array::IntoIter<Self::Item, N>> {
        let mut array: [MaybeUninit<Self::Item>; N] = [const { MaybeUninit::uninit() }; N];

        struct Guard<'a, T> {
            array: &'a mut [MaybeUninit<T>],
            initialized: usize,
        }

        impl<T> Drop for Guard<'_, T> {
            #[inline]
            fn drop(&mut self) {
                if const { crate::mem::needs_drop::<T>() } {
                    // SAFETY: self.initialized 始终 <= N，而 N 也是数组长度。
                    unsafe {
                        self.array.get_unchecked_mut(..self.initialized).assume_init_drop();
                    }
                }
            }
        }

        let mut guard = Guard { array: &mut array, initialized: 0 };

        let result = self.iter.try_for_each(|element| {
            let idx = guard.initialized;
            let val = (self.f)(element);
            guard.initialized = idx + val.is_some() as usize;

            // SAFETY: 循环条件保证索引在边界内。

            unsafe {
                let opt_payload_at: *const MaybeUninit<B> =
                    (&raw const val).byte_add(core::mem::offset_of!(Option<B>, Some.0)).cast();
                let dst = guard.array.as_mut_ptr().add(idx);
                crate::ptr::copy_nonoverlapping(opt_payload_at, dst, 1);
                crate::mem::forget(val);
            };

            if guard.initialized < N { ControlFlow::Continue(()) } else { ControlFlow::Break(()) }
        });

        let guard = ManuallyDrop::new(guard);

        match result {
            ControlFlow::Break(()) => {
                // SAFETY: 只有在数组已经完全初始化时，上面的循环才会显式 break。
                Ok(unsafe { MaybeUninit::array_assume_init(array) })
            }
            ControlFlow::Continue(()) => {
                let initialized = guard.initialized;
                // SAFETY: 到达 N 个元素时循环会 break，因此该范围在边界内。
                Err(unsafe { array::IntoIter::new_unchecked(array, 0..initialized) })
            }
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let (_, upper) = self.iter.size_hint();
        (0, upper) // 由于 predicate 的存在，无法知道下界。
    }

    #[inline]
    fn try_fold<Acc, Fold, R>(&mut self, init: Acc, fold: Fold) -> R
    where
        Self: Sized,
        Fold: FnMut(Acc, Self::Item) -> R,
        R: Try<Output = Acc>,
    {
        self.iter.try_fold(init, filter_map_try_fold(&mut self.f, fold))
    }

    #[inline]
    fn fold<Acc, Fold>(self, init: Acc, fold: Fold) -> Acc
    where
        Fold: FnMut(Acc, Self::Item) -> Acc,
    {
        self.iter.fold(init, filter_map_fold(self.f, fold))
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<B, I: DoubleEndedIterator, F> DoubleEndedIterator for FilterMap<I, F>
where
    F: FnMut(I::Item) -> Option<B>,
{
    #[inline]
    fn next_back(&mut self) -> Option<B> {
        #[inline]
        fn find<T, B>(
            f: &mut impl FnMut(T) -> Option<B>,
        ) -> impl FnMut((), T) -> ControlFlow<B> + '_ {
            move |(), x| match f(x) {
                Some(x) => ControlFlow::Break(x),
                None => ControlFlow::Continue(()),
            }
        }

        self.iter.try_rfold((), find(&mut self.f)).break_value()
    }

    #[inline]
    fn try_rfold<Acc, Fold, R>(&mut self, init: Acc, fold: Fold) -> R
    where
        Self: Sized,
        Fold: FnMut(Acc, Self::Item) -> R,
        R: Try<Output = Acc>,
    {
        self.iter.try_rfold(init, filter_map_try_fold(&mut self.f, fold))
    }

    #[inline]
    fn rfold<Acc, Fold>(self, init: Acc, fold: Fold) -> Acc
    where
        Fold: FnMut(Acc, Self::Item) -> Acc,
    {
        self.iter.rfold(init, filter_map_fold(self.f, fold))
    }
}

#[stable(feature = "fused", since = "1.26.0")]
impl<B, I: FusedIterator, F> FusedIterator for FilterMap<I, F> where F: FnMut(I::Item) -> Option<B> {}

#[unstable(issue = "none", feature = "trusted_fused")]
unsafe impl<I: TrustedFused, F> TrustedFused for FilterMap<I, F> {}

#[unstable(issue = "none", feature = "inplace_iteration")]
unsafe impl<I, F> SourceIter for FilterMap<I, F>
where
    I: SourceIter,
{
    type Source = I::Source;

    #[inline]
    unsafe fn as_inner(&mut self) -> &mut I::Source {
        // SAFETY: 将 unsafe 函数转发到具有相同要求的 unsafe 函数。
        unsafe { SourceIter::as_inner(&mut self.iter) }
    }
}

#[unstable(issue = "none", feature = "inplace_iteration")]
unsafe impl<I: InPlaceIterable, F> InPlaceIterable for FilterMap<I, F> {
    const EXPAND_BY: Option<NonZero<usize>> = I::EXPAND_BY;
    const MERGE_BY: Option<NonZero<usize>> = I::MERGE_BY;
}
