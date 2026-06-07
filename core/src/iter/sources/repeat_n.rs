use crate::fmt;
use crate::iter::{FusedIterator, TrustedLen, UncheckedIterator};
use crate::num::NonZero;
use crate::ops::Try;

/// 创建一个新的迭代器，把单个元素重复给定次数。
///
/// `repeat_n()` 函数会把单个值恰好重复 `n` 次。
///
/// 这与把 [`repeat()`] 和 [`Iterator::take()`] 组合起来很相似，但 `repeat_n()` 可以
/// 返回原始值，而不是始终克隆。
///
/// [`repeat()`]: crate::iter::repeat
///
/// # 示例
///
/// 基本用法:
///
/// ```
/// use std::iter;
///
/// // 四个数字四:
/// let mut four_fours = iter::repeat_n(4, 4);
///
/// assert_eq!(Some(4), four_fours.next());
/// assert_eq!(Some(4), four_fours.next());
/// assert_eq!(Some(4), four_fours.next());
/// assert_eq!(Some(4), four_fours.next());
///
/// // 不再有四。
/// assert_eq!(None, four_fours.next());
/// ```
///
/// 对非 `Copy` 类型:
///
/// ```
/// use std::iter;
///
/// let v: Vec<i32> = Vec::with_capacity(123);
/// let mut it = iter::repeat_n(v, 5);
///
/// for i in 0..4 {
///     // 开始时通过克隆产出值。
///     let cloned = it.next().unwrap();
///     assert_eq!(cloned.len(), 0);
///     assert_eq!(cloned.capacity(), 0);
/// }
///
/// // ...但最后一项是原始值。
/// let last = it.next().unwrap();
/// assert_eq!(last.len(), 0);
/// assert_eq!(last.capacity(), 123);
///
/// // ...现在结束。
/// assert_eq!(None, it.next());
/// ```
#[inline]
#[stable(feature = "iter_repeat_n", since = "1.82.0")]
pub fn repeat_n<T: Clone>(element: T, count: usize) -> RepeatN<T> {
    RepeatN { inner: RepeatNInner::new(element, count) }
}

#[derive(Clone, Copy)]
struct RepeatNInner<T> {
    count: NonZero<usize>,
    element: T,
}

impl<T> RepeatNInner<T> {
    fn new(element: T, count: usize) -> Option<Self> {
        let count = NonZero::<usize>::new(count)?;
        Some(Self { element, count })
    }
}

/// 将元素重复精确次数的迭代器。
///
/// 该 `struct` 由 [`repeat_n()`] 函数创建。更多信息见该函数文档。
#[stable(feature = "iter_repeat_n", since = "1.82.0")]
#[derive(Clone)]
pub struct RepeatN<A> {
    inner: Option<RepeatNInner<A>>,
}

impl<A> RepeatN<A> {
    /// 如果还没有 drop 该元素，则把它包在 option 中返回。
    #[inline]
    fn take_element(&mut self) -> Option<A> {
        self.inner.take().map(|inner| inner.element)
    }
}

#[stable(feature = "iter_repeat_n", since = "1.82.0")]
impl<A: fmt::Debug> fmt::Debug for RepeatN<A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (count, element) = match self.inner.as_ref() {
            Some(inner) => (inner.count.get(), Some(&inner.element)),
            None => (0, None),
        };
        f.debug_struct("RepeatN").field("count", &count).field("element", &element).finish()
    }
}

#[stable(feature = "iter_repeat_n", since = "1.82.0")]
impl<A: Clone> Iterator for RepeatN<A> {
    type Item = A;

    #[inline]
    fn next(&mut self) -> Option<A> {
        let inner = self.inner.as_mut()?;
        let count = inner.count.get();

        if let Some(decremented) = NonZero::<usize>::new(count - 1) {
            // 这些操作的顺序对优化很重要。
            let tmp = inner.element.clone();
            inner.count = decremented;
            return Some(tmp);
        }

        return self.take_element();
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }

    #[inline]
    fn advance_by(&mut self, skip: usize) -> Result<(), NonZero<usize>> {
        let Some(inner) = self.inner.as_mut() else {
            return NonZero::<usize>::new(skip).map(Err).unwrap_or(Ok(()));
        };

        let len = inner.count.get();

        if let Some(new_len) = len.checked_sub(skip).and_then(NonZero::<usize>::new) {
            inner.count = new_len;
            return Ok(());
        }

        self.inner = None;
        return NonZero::<usize>::new(skip - len).map(Err).unwrap_or(Ok(()));
    }

    #[inline]
    fn last(mut self) -> Option<A> {
        self.take_element()
    }

    #[inline]
    fn count(self) -> usize {
        self.len()
    }
}

#[stable(feature = "iter_repeat_n", since = "1.82.0")]
impl<A: Clone> ExactSizeIterator for RepeatN<A> {
    fn len(&self) -> usize {
        self.inner.as_ref().map(|inner| inner.count.get()).unwrap_or(0)
    }
}

#[stable(feature = "iter_repeat_n", since = "1.82.0")]
impl<A: Clone> DoubleEndedIterator for RepeatN<A> {
    #[inline]
    fn next_back(&mut self) -> Option<A> {
        self.next()
    }

    #[inline]
    fn advance_back_by(&mut self, n: usize) -> Result<(), NonZero<usize>> {
        self.advance_by(n)
    }

    #[inline]
    fn nth_back(&mut self, n: usize) -> Option<A> {
        self.nth(n)
    }

    #[inline]
    fn try_rfold<B, F, R>(&mut self, init: B, f: F) -> R
    where
        F: FnMut(B, A) -> R,
        R: Try<Output = B>,
    {
        self.try_fold(init, f)
    }

    #[inline]
    fn rfold<B, F>(self, init: B, f: F) -> B
    where
        F: FnMut(B, A) -> B,
    {
        self.fold(init, f)
    }
}

#[stable(feature = "iter_repeat_n", since = "1.82.0")]
impl<A: Clone> FusedIterator for RepeatN<A> {}

#[unstable(feature = "trusted_len", issue = "37572")]
unsafe impl<A: Clone> TrustedLen for RepeatN<A> {}
#[stable(feature = "iter_repeat_n", since = "1.82.0")]
impl<A: Clone> UncheckedIterator for RepeatN<A> {}
