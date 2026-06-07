use crate::fmt;
use crate::iter::FusedIterator;

/// 创建一个迭代器，从初始项开始，用前一项计算后一项。
///
/// 该迭代器保存一个可选项(`Option<T>`)和一个后继闭包
/// (`impl FnMut(&T) -> Option<T>`)。它的 `next` 方法返回当前保存的可选项；如果该项是
/// `Some(val)`，则会对 `&val` 调用后继闭包，计算并保存下一项。迭代器会持续把闭包
/// 应用于已保存 option 中的值，直到 option 变为 `None`。
///
/// 这也意味着一旦保存的 option 是 `None`，它就会保持为 `None`，因为闭包不会再被
/// 调用。因此创建出来的迭代器是 [`FusedIterator`]。该迭代器的元素包括初始项，以及
/// 后继闭包计算出的全部后继项。
///
/// ```
/// use std::iter::successors;
///
/// let powers_of_10 = successors(Some(1_u16), |n| n.checked_mul(10));
/// assert_eq!(powers_of_10.collect::<Vec<_>>(), &[1, 10, 100, 1_000, 10_000]);
/// ```
#[stable(feature = "iter_successors", since = "1.34.0")]
pub fn successors<T, F>(first: Option<T>, succ: F) -> Successors<T, F>
where
    F: FnMut(&T) -> Option<T>,
{
    // 如果该函数返回 `impl Iterator<Item=T>`，它可以基于 `from_fn` 实现而不需要专用类型。
    // 但具名的 `Successors<T, F>` 类型可以在 `T` 和 `F` 均为 Clone 时实现 Clone。
    Successors { next: first, succ }
}

/// 从初始项开始、用前一项计算后一项的迭代器。
///
/// 该 `struct` 由 [`iter::successors()`] 函数创建。更多信息见该函数文档。
///
/// [`iter::successors()`]: successors
#[derive(Clone)]
#[stable(feature = "iter_successors", since = "1.34.0")]
pub struct Successors<T, F> {
    next: Option<T>,
    succ: F,
}

#[stable(feature = "iter_successors", since = "1.34.0")]
impl<T, F> Iterator for Successors<T, F>
where
    F: FnMut(&T) -> Option<T>,
{
    type Item = T;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let item = self.next.take()?;
        self.next = (self.succ)(&item);
        Some(item)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.next.is_some() { (1, None) } else { (0, Some(0)) }
    }
}

#[stable(feature = "iter_successors", since = "1.34.0")]
impl<T, F> FusedIterator for Successors<T, F> where F: FnMut(&T) -> Option<T> {}

#[stable(feature = "iter_successors", since = "1.34.0")]
impl<T: fmt::Debug, F> fmt::Debug for Successors<T, F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Successors").field("next", &self.next).finish()
    }
}
