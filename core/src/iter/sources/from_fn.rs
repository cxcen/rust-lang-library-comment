use crate::fmt;

/// 创建一个迭代器，将给定闭包 `F: FnMut() -> Option<T>` 作为它的
/// [`next`](Iterator::next) 方法。
///
/// 该迭代器会产出闭包返回的 `T`。
///
/// 这允许用闭包快速创建任意行为的自定义迭代器，而不必先定义专用类型再为它实现
/// [`Iterator`] trait。
///
/// 注意，`FromFn` 不会假设闭包在返回 `None` 后是否还会再次返回 `Some(_)`，因此它
/// 保守地不实现 [`FusedIterator`]，也不覆盖 [`Iterator::size_hint()`] 的默认
/// `(0, None)`。
///
/// 闭包可以通过捕获和自身环境在多次迭代之间保存状态。根据迭代器的使用方式，
/// 这可能需要在闭包上指定 [`move`] 关键字。
///
/// [`move`]: ../../std/keyword.move.html
/// [`FusedIterator`]: crate::iter::FusedIterator
///
/// # 示例
///
/// 重新实现[模块级文档]中的计数器迭代器:
///
/// [模块级文档]: crate::iter
///
/// ```
/// let mut count = 0;
/// let counter = std::iter::from_fn(move || {
///     // 递增计数，这也是从零开始的原因。
///     count += 1;
///
///     // 检查是否已经计数结束。
///     if count < 6 {
///         Some(count)
///     } else {
///         None
///     }
/// });
/// assert_eq!(counter.collect::<Vec<_>>(), &[1, 2, 3, 4, 5]);
/// ```
#[inline]
#[stable(feature = "iter_from_fn", since = "1.34.0")]
pub fn from_fn<T, F>(f: F) -> FromFn<F>
where
    F: FnMut() -> Option<T>,
{
    FromFn(f)
}

/// 每次迭代都会调用给定闭包 `F: FnMut() -> Option<T>` 的迭代器。
///
/// 该 `struct` 由 [`iter::from_fn()`] 函数创建。更多信息见该函数文档。
///
/// [`iter::from_fn()`]: from_fn
#[derive(Clone)]
#[stable(feature = "iter_from_fn", since = "1.34.0")]
pub struct FromFn<F>(F);

#[stable(feature = "iter_from_fn", since = "1.34.0")]
impl<T, F> Iterator for FromFn<F>
where
    F: FnMut() -> Option<T>,
{
    type Item = T;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        (self.0)()
    }
}

#[stable(feature = "iter_from_fn", since = "1.34.0")]
impl<F> fmt::Debug for FromFn<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FromFn").finish()
    }
}
