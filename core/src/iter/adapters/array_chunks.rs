use crate::array;
use crate::iter::adapters::SourceIter;
use crate::iter::{
    ByRefSized, FusedIterator, InPlaceIterable, TrustedFused, TrustedRandomAccessNoCoerce,
};
use crate::num::NonZero;
use crate::ops::{ControlFlow, NeverShortCircuit, Try};

/// 每次遍历底层迭代器 `N` 个元素的迭代器。
///
/// chunk 之间不重叠。如果 `N` 不能整除迭代器长度，则最后最多 `N - 1` 个元素会被省略。
///
/// 该 `struct` 由 [`Iterator`] 上的 [`array_chunks`][Iterator::array_chunks] 方法创建。
/// 更多信息见该方法文档。
#[derive(Debug, Clone)]
#[must_use = "iterators are lazy and do nothing unless consumed"]
#[unstable(feature = "iter_array_chunks", issue = "100450")]
pub struct ArrayChunks<I: Iterator, const N: usize> {
    iter: I,
    remainder: Option<array::IntoIter<I::Item, N>>,
}

impl<I, const N: usize> ArrayChunks<I, N>
where
    I: Iterator,
{
    #[track_caller]
    pub(in crate::iter) fn new(iter: I) -> Self {
        assert!(N != 0, "chunk size must be non-zero");
        Self { iter, remainder: None }
    }

    /// 返回一个迭代器，遍历原始迭代器中不会被本迭代器返回的剩余元素。
    /// 返回的迭代器最多产出 `N - 1` 个元素。
    ///
    /// # 示例
    /// ```
    /// # // Also serves as a regression test for https://github.com/rust-lang/rust/issues/123333
    /// # #![feature(iter_array_chunks)]
    /// let x = [1,2,3,4,5].into_iter().array_chunks::<2>();
    /// let mut rem = x.into_remainder();
    /// assert_eq!(rem.next(), Some(5));
    /// assert_eq!(rem.next(), None);
    /// ```
    #[unstable(feature = "iter_array_chunks", issue = "100450")]
    #[inline]
    pub fn into_remainder(mut self) -> array::IntoIter<I::Item, N> {
        if self.remainder.is_none() {
            while let Some(_) = self.next() {}
        }
        self.remainder.unwrap_or_default()
    }
}

#[unstable(feature = "iter_array_chunks", issue = "100450")]
impl<I, const N: usize> Iterator for ArrayChunks<I, N>
where
    I: Iterator,
{
    type Item = [I::Item; N];

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.try_for_each(ControlFlow::Break).break_value()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let (lower, upper) = self.iter.size_hint();

        (lower / N, upper.map(|n| n / N))
    }

    #[inline]
    fn count(self) -> usize {
        self.iter.count() / N
    }

    fn try_fold<B, F, R>(&mut self, init: B, mut f: F) -> R
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> R,
        R: Try<Output = B>,
    {
        let mut acc = init;
        loop {
            match self.iter.next_chunk() {
                Ok(chunk) => acc = f(acc, chunk)?,
                Err(remainder) => {
            // 确保 `ArrayChunks` 耗尽后再次调用 `next` 时，不会用空数组覆盖
            // `self.remainder`。
                    self.remainder.get_or_insert(remainder);

                    break try { acc };
                }
            }
        }
    }

    fn fold<B, F>(self, init: B, f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        <Self as SpecFold>::fold(self, init, f)
    }
}

#[unstable(feature = "iter_array_chunks", issue = "100450")]
impl<I, const N: usize> DoubleEndedIterator for ArrayChunks<I, N>
where
    I: DoubleEndedIterator + ExactSizeIterator,
{
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        self.try_rfold((), |(), x| ControlFlow::Break(x)).break_value()
    }

    fn try_rfold<B, F, R>(&mut self, init: B, mut f: F) -> R
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> R,
        R: Try<Output = B>,
    {
        // 从后端迭代时，需要先处理余数部分。
        self.next_back_remainder();

        let mut acc = init;
        let mut iter = ByRefSized(&mut self.iter).rev();

        // 注意: remainder 已由 `next_back_remainder` 处理，因此 `next_chunk` 不会返回
        // 带非空 remainder 的 `Err`(假设 `I as ExactSizeIterator` 实现正确)。
        while let Ok(mut chunk) = iter.next_chunk() {
            // FIXME: 不要做双重 reverse
            //        (例如可以改为添加 `next_chunk_back`)。
            chunk.reverse();
            acc = f(acc, chunk)?
        }

        try { acc }
    }

    impl_fold_via_try_fold! { rfold -> try_rfold }
}

impl<I, const N: usize> ArrayChunks<I, N>
where
    I: DoubleEndedIterator + ExactSizeIterator,
{
    /// 更新 `self.remainder`，使 `self.iter.len` 能被 `N` 整除。
    fn next_back_remainder(&mut self) {
        // 确保 `ArrayChunks` 耗尽后再次调用 `next_back` 时，不会用空数组覆盖
        // `self.remainder`。
        if self.remainder.is_some() {
            return;
        }

        // 使用底层迭代器的 `ExactSizeIterator` 实现来得知剩余元素数量。
        let rem = self.iter.len() % N;

        // 从 `self.iter` 中取出最后 `rem` 个元素。
        let mut remainder =
            // SAFETY: 对所有 x 都有 x % N < N，因此 `unwrap_err` 总是成功。
            unsafe { self.iter.by_ref().rev().take(rem).next_chunk().unwrap_err_unchecked() };

        // 上面使用了 `.rev()`，因此需要把 remainder 再反转回来。
        remainder.as_mut_slice().reverse();
        self.remainder = Some(remainder);
    }
}

#[unstable(feature = "iter_array_chunks", issue = "100450")]
impl<I, const N: usize> FusedIterator for ArrayChunks<I, N> where I: FusedIterator {}

#[unstable(issue = "none", feature = "trusted_fused")]
unsafe impl<I, const N: usize> TrustedFused for ArrayChunks<I, N> where I: TrustedFused + Iterator {}

#[unstable(feature = "iter_array_chunks", issue = "100450")]
impl<I, const N: usize> ExactSizeIterator for ArrayChunks<I, N>
where
    I: ExactSizeIterator,
{
    #[inline]
    fn len(&self) -> usize {
        self.iter.len() / N
    }

    #[inline]
    fn is_empty(&self) -> bool {
        self.iter.len() < N
    }
}

trait SpecFold: Iterator {
    fn fold<B, F>(self, init: B, f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B;
}

impl<I, const N: usize> SpecFold for ArrayChunks<I, N>
where
    I: Iterator,
{
    #[inline]
    default fn fold<B, F>(mut self, init: B, f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        self.try_fold(init, NeverShortCircuit::wrap_mut_2(f)).0
    }
}

impl<I, const N: usize> SpecFold for ArrayChunks<I, N>
where
    I: Iterator + TrustedRandomAccessNoCoerce,
{
    #[inline]
    fn fold<B, F>(mut self, init: B, mut f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        let mut accum = init;
        let inner_len = self.iter.size();
        let mut i = 0;
        // 使用 while 循环，因为 (0..len).step_by(N) 优化效果不好。
        while inner_len - i >= N {
            let chunk = crate::array::from_fn(|local| {
                // SAFETY: 该方法会消耗迭代器，且循环条件保证所有访问都在边界内并且
                // 只发生一次。
                unsafe {
                    let idx = i + local;
                    self.iter.__iterator_get_unchecked(idx)
                }
            });
            accum = f(accum, chunk);
            i += N;
        }

        // 不同于 try_fold，该方法不需要处理 remainder，因为 `self` 会被 drop。

        accum
    }
}

#[unstable(issue = "none", feature = "inplace_iteration")]
unsafe impl<I, const N: usize> SourceIter for ArrayChunks<I, N>
where
    I: SourceIter + Iterator,
{
    type Source = I::Source;

    #[inline]
    unsafe fn as_inner(&mut self) -> &mut I::Source {
        // SAFETY: 转发到具有相同要求的 unsafe 函数。
        unsafe { SourceIter::as_inner(&mut self.iter) }
    }
}

#[unstable(issue = "none", feature = "inplace_iteration")]
unsafe impl<I: InPlaceIterable + Iterator, const N: usize> InPlaceIterable for ArrayChunks<I, N> {
    const EXPAND_BY: Option<NonZero<usize>> = I::EXPAND_BY;
    const MERGE_BY: Option<NonZero<usize>> = const {
        match (I::MERGE_BY, NonZero::new(N)) {
            (Some(m), Some(n)) => m.checked_mul(n),
            _ => None,
        }
    };
}
