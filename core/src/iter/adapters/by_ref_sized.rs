use crate::num::NonZero;
use crate::ops::{NeverShortCircuit, Try};

/// 类似 `Iterator::by_ref`，但要求 `Sized`，因此可以转发泛型方法。
///
/// 理想情况下，最终不应再需要它；但从 benchmark 可见（至少截至 2022 年 2 月），
/// `by_ref` 可能带来性能开销。
#[unstable(feature = "std_internals", issue = "none")]
#[derive(Debug)]
pub struct ByRefSized<'a, I>(pub &'a mut I);

// 下面的实现使用 UFCS 风格，而不是依赖 autoderef，以避免意外调用 `&mut Iterator` 实现。

#[unstable(feature = "std_internals", issue = "none")]
impl<I: Iterator> Iterator for ByRefSized<'_, I> {
    type Item = I::Item;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        I::next(self.0)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        I::size_hint(self.0)
    }

    #[inline]
    fn advance_by(&mut self, n: usize) -> Result<(), NonZero<usize>> {
        I::advance_by(self.0, n)
    }

    #[inline]
    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        I::nth(self.0, n)
    }

    #[inline]
    fn fold<B, F>(self, init: B, f: F) -> B
    where
        F: FnMut(B, Self::Item) -> B,
    {
        // `fold` 需要所有权，因此这里不能直接转发。
        I::try_fold(self.0, init, NeverShortCircuit::wrap_mut_2(f)).0
    }

    #[inline]
    fn try_fold<B, F, R>(&mut self, init: B, f: F) -> R
    where
        F: FnMut(B, Self::Item) -> R,
        R: Try<Output = B>,
    {
        I::try_fold(self.0, init, f)
    }
}

#[unstable(feature = "std_internals", issue = "none")]
impl<I: DoubleEndedIterator> DoubleEndedIterator for ByRefSized<'_, I> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        I::next_back(self.0)
    }

    #[inline]
    fn advance_back_by(&mut self, n: usize) -> Result<(), NonZero<usize>> {
        I::advance_back_by(self.0, n)
    }

    #[inline]
    fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
        I::nth_back(self.0, n)
    }

    #[inline]
    fn rfold<B, F>(self, init: B, f: F) -> B
    where
        F: FnMut(B, Self::Item) -> B,
    {
        // `rfold` 需要所有权，所以这里无法直接转发。
        I::try_rfold(self.0, init, NeverShortCircuit::wrap_mut_2(f)).0
    }

    #[inline]
    fn try_rfold<B, F, R>(&mut self, init: B, f: F) -> R
    where
        F: FnMut(B, Self::Item) -> R,
        R: Try<Output = B>,
    {
        I::try_rfold(self.0, init, f)
    }
}
