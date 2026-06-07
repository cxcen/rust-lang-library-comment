use crate::iter::{FusedIterator, TrustedLen};
use crate::num::NonZero;

/// 创建一个新的迭代器，永远重复产出单个元素。
///
/// `repeat()` 函数会一遍又一遍重复同一个值。
///
/// 像 `repeat()` 这样的无限迭代器通常会配合 [`Iterator::take()`] 等适配器使用，
/// 从而把它们限制为有限迭代器。
///
/// 如果预先知道重复次数，考虑改用 [`repeat_n()`]，因为它更高效，也能更清楚地表达意图。
///
/// 如果只是想把 char/string 重复 `n` 次，请使用 [`str::repeat()`] 而不是本函数。
///
/// 如果所需迭代器的元素类型没有实现 `Clone`，或者不想把被重复的元素保存在内存中，
/// 可以改用 [`repeat_with()`] 函数。
///
/// [`repeat_n()`]: crate::iter::repeat_n
/// [`repeat_with()`]: crate::iter::repeat_with
/// [`str::repeat()`]: ../../std/primitive.str.html#method.repeat
///
/// # 示例
///
/// 基本用法:
///
/// ```
/// use std::iter;
///
/// // 永远产出数字四:
/// let mut fours = iter::repeat(4);
///
/// assert_eq!(Some(4), fours.next());
/// assert_eq!(Some(4), fours.next());
/// assert_eq!(Some(4), fours.next());
/// assert_eq!(Some(4), fours.next());
/// assert_eq!(Some(4), fours.next());
///
/// // 没错，仍然是四。
/// assert_eq!(Some(4), fours.next());
/// ```
///
/// 使用 [`Iterator::take()`] 限制为有限迭代:
///
/// ```
/// use std::iter;
///
/// // 上个例子里的四太多了。这里只取四个四。
/// let mut four_fours = iter::repeat(4).take(4);
///
/// assert_eq!(Some(4), four_fours.next());
/// assert_eq!(Some(4), four_fours.next());
/// assert_eq!(Some(4), four_fours.next());
/// assert_eq!(Some(4), four_fours.next());
///
/// // ...现在结束。
/// assert_eq!(None, four_fours.next());
/// ```
#[inline]
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_diagnostic_item = "iter_repeat"]
pub fn repeat<T: Clone>(elt: T) -> Repeat<T> {
    Repeat { element: elt }
}

/// 无限重复某个元素的迭代器。
///
/// 该 `struct` 由 [`repeat()`] 函数创建。更多信息见该函数文档。
#[derive(Clone, Debug)]
#[stable(feature = "rust1", since = "1.0.0")]
pub struct Repeat<A> {
    element: A,
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<A: Clone> Iterator for Repeat<A> {
    type Item = A;

    #[inline]
    fn next(&mut self) -> Option<A> {
        Some(self.element.clone())
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (usize::MAX, None)
    }

    #[inline]
    fn advance_by(&mut self, n: usize) -> Result<(), NonZero<usize>> {
        // 推进单元素无限迭代器是无操作。
        let _ = n;
        Ok(())
    }

    #[inline]
    fn nth(&mut self, n: usize) -> Option<A> {
        let _ = n;
        Some(self.element.clone())
    }

    #[track_caller]
    fn last(self) -> Option<A> {
        panic!("iterator is infinite");
    }

    #[track_caller]
    fn count(self) -> usize {
        panic!("iterator is infinite");
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<A: Clone> DoubleEndedIterator for Repeat<A> {
    #[inline]
    fn next_back(&mut self) -> Option<A> {
        Some(self.element.clone())
    }

    #[inline]
    fn advance_back_by(&mut self, n: usize) -> Result<(), NonZero<usize>> {
        // 推进单元素无限迭代器是无操作。
        let _ = n;
        Ok(())
    }

    #[inline]
    fn nth_back(&mut self, n: usize) -> Option<A> {
        let _ = n;
        Some(self.element.clone())
    }
}

#[stable(feature = "fused", since = "1.26.0")]
impl<A: Clone> FusedIterator for Repeat<A> {}

#[unstable(feature = "trusted_len", issue = "37572")]
unsafe impl<A: Clone> TrustedLen for Repeat<A> {}
