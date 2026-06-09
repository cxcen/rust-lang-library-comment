use crate::iter::{FusedIterator, TrustedLen};
use crate::{fmt, marker};

/// 创建一个不产出任何元素的迭代器。
///
/// # 示例
///
/// 基本用法:
///
/// ```
/// use std::iter;
///
/// // 它本可以是一个 i32 迭代器，但实际上什么都不产出。
/// let mut nope = iter::empty::<i32>();
///
/// assert_eq!(None, nope.next());
/// ```
#[stable(feature = "iter_empty", since = "1.2.0")]
#[rustc_const_stable(feature = "const_iter_empty", since = "1.32.0")]
pub const fn empty<T>() -> Empty<T> {
    Empty(marker::PhantomData)
}

/// 不产出任何元素的迭代器。
///
/// 该 `struct` 由 [`empty()`] 函数创建。更多信息见该函数文档。
#[must_use = "iterators are lazy and do nothing unless consumed"]
#[stable(feature = "iter_empty", since = "1.2.0")]
#[rustc_diagnostic_item = "IterEmpty"]
pub struct Empty<T>(marker::PhantomData<fn() -> T>);

#[stable(feature = "core_impl_debug", since = "1.9.0")]
impl<T> fmt::Debug for Empty<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Empty").finish()
    }
}

#[stable(feature = "iter_empty", since = "1.2.0")]
impl<T> Iterator for Empty<T> {
    type Item = T;

    fn next(&mut self) -> Option<T> {
        None
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some(0))
    }
}

#[stable(feature = "iter_empty", since = "1.2.0")]
impl<T> DoubleEndedIterator for Empty<T> {
    fn next_back(&mut self) -> Option<T> {
        None
    }
}

#[stable(feature = "iter_empty", since = "1.2.0")]
impl<T> ExactSizeIterator for Empty<T> {
    fn len(&self) -> usize {
        0
    }
}

#[unstable(feature = "trusted_len", issue = "37572")]
unsafe impl<T> TrustedLen for Empty<T> {}

#[stable(feature = "fused", since = "1.26.0")]
impl<T> FusedIterator for Empty<T> {}

// 不使用 #[derive]，因为那会给 T 增加不必要的 Clone bound。
#[stable(feature = "iter_empty", since = "1.2.0")]
impl<T> Clone for Empty<T> {
    fn clone(&self) -> Empty<T> {
        Empty(marker::PhantomData)
    }
}

// 不使用 #[derive]，因为那会给 T 增加不必要的 Default bound。
#[stable(feature = "iter_empty", since = "1.2.0")]
#[rustc_const_unstable(feature = "const_default", issue = "143894")]
impl<T> const Default for Empty<T> {
    fn default() -> Empty<T> {
        Empty(marker::PhantomData)
    }
}
