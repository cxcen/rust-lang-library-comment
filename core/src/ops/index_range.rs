use crate::iter::{FusedIterator, TrustedLen};
use crate::num::NonZero;
use crate::ops::{NeverShortCircuit, Try};
use crate::ub_checks;

/// 类似于 `Range<usize>`,但带有一个安全不变量:`start <= end`。
///
/// 这意味着 `end - start` 不会溢出,从而允许做一些微优化(μoptimization)。
///
/// (普通的 `Range` 代码需要处理像 `10..0` 这样的退化区间,与只处理规范形式相比,
///  这需要额外的检查。)
#[derive(Debug)]
#[derive_const(Clone, Eq, PartialEq)]
pub(crate) struct IndexRange {
    start: usize,
    end: usize,
}

impl IndexRange {
    /// # 安全性(Safety）
    /// - `start <= end`
    #[inline]
    #[track_caller]
    pub(crate) const unsafe fn new_unchecked(start: usize, end: usize) -> Self {
        ub_checks::assert_unsafe_precondition!(
            check_library_ub,
            "IndexRange::new_unchecked requires `start <= end`",
            (start: usize = start, end: usize = end) => start <= end,
        );
        IndexRange { start, end }
    }

    #[inline]
    pub(crate) const fn zero_to(end: usize) -> Self {
        IndexRange { start: 0, end }
    }

    #[inline]
    pub(crate) const fn start(&self) -> usize {
        self.start
    }

    #[inline]
    pub(crate) const fn end(&self) -> usize {
        self.end
    }

    #[inline]
    pub(crate) const fn len(&self) -> usize {
        // SAFETY: 根据不变量,这里不会发生回绕(wrap)
        // 这里使用 intrinsic,是因为在此处做 UB 检查会妨碍 LLVM 优化。(#131563)
        unsafe { crate::intrinsics::unchecked_sub(self.end, self.start) }
    }

    /// # 安全性(Safety）
    /// - 只能在 `start < end`(即 `len > 0`)时调用。
    #[inline]
    const unsafe fn next_unchecked(&mut self) -> usize {
        debug_assert!(self.start < self.end);

        let value = self.start;
        // SAFETY: 区间非空,所以这里不会溢出
        self.start = unsafe { value.unchecked_add(1) };
        value
    }

    /// # 安全性(Safety）
    /// - 只能在 `start < end`(即 `len > 0`)时调用。
    #[inline]
    const unsafe fn next_back_unchecked(&mut self) -> usize {
        debug_assert!(self.start < self.end);

        // SAFETY: 区间非空,所以这里不会溢出
        let value = unsafe { self.end.unchecked_sub(1) };
        self.end = value;
        value
    }

    /// 从该区间中移除前 `n` 个元素,并把它们作为一个 `IndexRange` 返回。如果不足
    /// `n` 个,则返回整个区间,并把 `self` 置空。
    ///
    /// 这是为了帮助实现 `Iterator::advance_by` 而设计的。
    #[inline]
    pub(crate) fn take_prefix(&mut self, n: usize) -> Self {
        let mid = if n <= self.len() {
            // SAFETY: 我们刚刚检查过这个值会落在 start 与 end 之间,
            // 因此该加法不会溢出。
            // 使用 intrinsic 可以避免一次多余的 UB 检查。
            unsafe { crate::intrinsics::unchecked_add(self.start, n) }
        } else {
            self.end
        };
        let prefix = Self { start: self.start, end: mid };
        self.start = mid;
        prefix
    }

    /// 从该区间中移除后 `n` 个元素,并把它们作为一个 `IndexRange` 返回。如果不足
    /// `n` 个,则返回整个区间,并把 `self` 置空。
    ///
    /// 这是为了帮助实现 `Iterator::advance_back_by` 而设计的。
    #[inline]
    pub(crate) fn take_suffix(&mut self, n: usize) -> Self {
        let mid = if n <= self.len() {
            // SAFETY: 我们刚刚检查过这个值会落在 start 与 end 之间,
            // 因此该减法不会溢出。
            // 使用 intrinsic 可以避免一次多余的 UB 检查。
            unsafe { crate::intrinsics::unchecked_sub(self.end, n) }
        } else {
            self.start
        };
        let suffix = Self { start: mid, end: self.end };
        self.end = mid;
        suffix
    }

    #[inline]
    const fn assume_range(&self) {
        // SAFETY: 这正是该类型的不变量
        unsafe { crate::hint::assert_unchecked(self.start <= self.end) }
    }
}

impl Iterator for IndexRange {
    type Item = usize;

    #[inline]
    fn next(&mut self) -> Option<usize> {
        if self.len() > 0 {
            // SAFETY: 我们刚刚检查过区间非空
            unsafe { Some(self.next_unchecked()) }
        } else {
            None
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }

    #[inline]
    fn advance_by(&mut self, n: usize) -> Result<(), NonZero<usize>> {
        let taken = self.take_prefix(n);
        NonZero::new(n - taken.len()).map_or(Ok(()), Err)
    }

    #[inline]
    fn fold<B, F: FnMut(B, usize) -> B>(mut self, init: B, f: F) -> B {
        self.try_fold(init, NeverShortCircuit::wrap_mut_2(f)).0
    }

    #[inline]
    fn try_fold<B, F, R>(&mut self, mut accum: B, mut f: F) -> R
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> R,
        R: Try<Output = B>,
    {
        // `Range` 需要检查 `start < end`,但得益于我们的类型不变量,
        // 我们可以基于更严格的 `start != end` 来循环。

        self.assume_range();
        while self.start != self.end {
            // SAFETY: 我们刚刚检查过区间非空
            let i = unsafe { self.next_unchecked() };
            accum = f(accum, i)?;
        }
        try { accum }
    }
}

impl DoubleEndedIterator for IndexRange {
    #[inline]
    fn next_back(&mut self) -> Option<usize> {
        if self.len() > 0 {
            // SAFETY: 我们刚刚检查过区间非空
            unsafe { Some(self.next_back_unchecked()) }
        } else {
            None
        }
    }

    #[inline]
    fn advance_back_by(&mut self, n: usize) -> Result<(), NonZero<usize>> {
        let taken = self.take_suffix(n);
        NonZero::new(n - taken.len()).map_or(Ok(()), Err)
    }

    #[inline]
    fn rfold<B, F: FnMut(B, usize) -> B>(mut self, init: B, f: F) -> B {
        self.try_rfold(init, NeverShortCircuit::wrap_mut_2(f)).0
    }

    #[inline]
    fn try_rfold<B, F, R>(&mut self, mut accum: B, mut f: F) -> R
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> R,
        R: Try<Output = B>,
    {
        // `Range` 需要检查 `start < end`,但得益于我们的类型不变量,
        // 我们可以基于更严格的 `start != end` 来循环。

        self.assume_range();
        while self.start != self.end {
            // SAFETY: 我们刚刚检查过区间非空
            let i = unsafe { self.next_back_unchecked() };
            accum = f(accum, i)?;
        }
        try { accum }
    }
}

impl ExactSizeIterator for IndexRange {
    #[inline]
    fn len(&self) -> usize {
        self.len()
    }
}

// SAFETY: 由于我们只处理 `usize`,我们的 `len` 总是精确无误的。
unsafe impl TrustedLen for IndexRange {}

impl FusedIterator for IndexRange {}
