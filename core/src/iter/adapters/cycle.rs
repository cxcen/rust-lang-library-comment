use crate::iter::FusedIterator;
use crate::num::NonZero;
use crate::ops::Try;

/// 无限重复底层迭代器的迭代器。
///
/// 该 `struct` 由 [`Iterator`] 上的 [`cycle`] 方法创建。更多信息见该方法文档。
///
/// [`cycle`]: Iterator::cycle
/// [`Iterator`]: trait.Iterator.html
#[derive(Clone, Debug)]
#[must_use = "iterators are lazy and do nothing unless consumed"]
#[stable(feature = "rust1", since = "1.0.0")]
pub struct Cycle<I> {
    orig: I,
    iter: I,
}

impl<I: Clone> Cycle<I> {
    pub(in crate::iter) fn new(iter: I) -> Cycle<I> {
        Cycle { orig: iter.clone(), iter }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<I> Iterator for Cycle<I>
where
    I: Clone + Iterator,
{
    type Item = <I as Iterator>::Item;

    #[inline]
    fn next(&mut self) -> Option<<I as Iterator>::Item> {
        match self.iter.next() {
            None => {
                self.iter = self.orig.clone();
                self.iter.next()
            }
            y => y,
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        // cycle 迭代器要么为空，要么无限。
        match self.orig.size_hint() {
            sz @ (0, Some(0)) => sz,
            (0, _) => (0, None),
            _ => (usize::MAX, None),
        }
    }

    #[inline]
    fn try_fold<Acc, F, R>(&mut self, mut acc: Acc, mut f: F) -> R
    where
        F: FnMut(Acc, Self::Item) -> R,
        R: Try<Output = Acc>,
    {
        // 先完整迭代当前迭代器。这是必要的，因为即使 `self.orig` 非空，
        // `self.iter` 也可能为空。
        acc = self.iter.try_fold(acc, &mut f)?;
        self.iter = self.orig.clone();

        // 完成一个完整循环，同时记录被循环的迭代器是否为空。若为空，需要提前返回以
        // 避免无限循环。
        let mut is_empty = true;
        acc = self.iter.try_fold(acc, |acc, x| {
            is_empty = false;
            f(acc, x)
        })?;

        if is_empty {
            return try { acc };
        }

        loop {
            self.iter = self.orig.clone();
            acc = self.iter.try_fold(acc, &mut f)?;
        }
    }

    #[inline]
    #[rustc_inherit_overflow_checks]
    fn advance_by(&mut self, n: usize) -> Result<(), NonZero<usize>> {
        let mut n = match self.iter.advance_by(n) {
            Ok(()) => return Ok(()),
            Err(rem) => rem.get(),
        };

        while n > 0 {
            self.iter = self.orig.clone();
            n = match self.iter.advance_by(n) {
                Ok(()) => return Ok(()),
                e @ Err(rem) if rem.get() == n => return e,
                Err(rem) => rem.get(),
            };
        }

        NonZero::new(n).map_or(Ok(()), Err)
    }

    // 不覆盖 `fold`，因为 `fold` 对 `Cycle` 意义不大，而且无法做得比默认实现更好。
}

#[stable(feature = "fused", since = "1.26.0")]
impl<I> FusedIterator for Cycle<I> where I: Clone + Iterator {}
