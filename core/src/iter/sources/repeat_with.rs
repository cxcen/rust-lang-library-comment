use crate::fmt;
use crate::iter::{FusedIterator, TrustedLen};
use crate::ops::Try;

/// 创建一个新的无限迭代器，通过反复调用给定闭包 `F: FnMut() -> A` 来产出 `A` 类型元素。
///
/// `repeat_with()` 会一遍又一遍调用这个 repeater。
///
/// 像 `repeat_with()` 这样的无限迭代器通常会配合 [`Iterator::take()`] 等适配器使用，
/// 从而把它们限制为有限迭代器。
///
/// 如果所需迭代器的元素类型实现了 [`Clone`]，并且把源元素保存在内存中是可接受的，
/// 应改用 [`repeat()`] 函数。
///
/// `repeat_with()` 产生的迭代器不是 [`DoubleEndedIterator`]。如果需要它返回
/// [`DoubleEndedIterator`]，请提交 GitHub issue 说明你的用例。
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
/// // 假设有一个值，它的类型不是 `Clone`，
/// // 或者构造成本较高，暂时不想把它保存在内存中:
/// #[derive(PartialEq, Debug)]
/// struct Expensive;
///
/// // 永远产出某个特定值:
/// let mut things = iter::repeat_with(|| Expensive);
///
/// assert_eq!(Some(Expensive), things.next());
/// assert_eq!(Some(Expensive), things.next());
/// assert_eq!(Some(Expensive), things.next());
/// assert_eq!(Some(Expensive), things.next());
/// assert_eq!(Some(Expensive), things.next());
/// ```
///
/// 使用可变状态并限制为有限迭代:
///
/// ```rust
/// use std::iter;
///
/// // 从 2 的零次方到三次方:
/// let mut curr = 1;
/// let mut pow2 = iter::repeat_with(|| { let tmp = curr; curr *= 2; tmp })
///                     .take(4);
///
/// assert_eq!(Some(1), pow2.next());
/// assert_eq!(Some(2), pow2.next());
/// assert_eq!(Some(4), pow2.next());
/// assert_eq!(Some(8), pow2.next());
///
/// // ...现在结束。
/// assert_eq!(None, pow2.next());
/// ```
#[inline]
#[stable(feature = "iterator_repeat_with", since = "1.28.0")]
pub fn repeat_with<A, F: FnMut() -> A>(repeater: F) -> RepeatWith<F> {
    RepeatWith { repeater }
}

/// 通过调用给定闭包 `F: FnMut() -> A` 无限重复产出 `A` 类型元素的迭代器。
///
/// 该 `struct` 由 [`repeat_with()`] 函数创建。更多信息见该函数文档。
#[derive(Copy, Clone)]
#[stable(feature = "iterator_repeat_with", since = "1.28.0")]
pub struct RepeatWith<F> {
    repeater: F,
}

#[stable(feature = "iterator_repeat_with_debug", since = "1.68.0")]
impl<F> fmt::Debug for RepeatWith<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RepeatWith").finish_non_exhaustive()
    }
}

#[stable(feature = "iterator_repeat_with", since = "1.28.0")]
impl<A, F: FnMut() -> A> Iterator for RepeatWith<F> {
    type Item = A;

    #[inline]
    fn next(&mut self) -> Option<A> {
        Some((self.repeater)())
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (usize::MAX, None)
    }

    #[inline]
    fn try_fold<Acc, Fold, R>(&mut self, mut init: Acc, mut fold: Fold) -> R
    where
        Fold: FnMut(Acc, Self::Item) -> R,
        R: Try<Output = Acc>,
    {
        // 这个覆盖并非严格必要，但它避免了依赖优化器消除“next 总是返回 Some”的分支，
        // 也强调 `?` 是退出循环的唯一方式。

        loop {
            let item = (self.repeater)();
            init = fold(init, item)?;
        }
    }
}

#[stable(feature = "iterator_repeat_with", since = "1.28.0")]
impl<A, F: FnMut() -> A> FusedIterator for RepeatWith<F> {}

#[unstable(feature = "trusted_len", issue = "37572")]
unsafe impl<A, F: FnMut() -> A> TrustedLen for RepeatWith<F> {}
