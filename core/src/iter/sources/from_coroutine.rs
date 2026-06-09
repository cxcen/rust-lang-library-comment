use crate::fmt;
use crate::ops::{Coroutine, CoroutineState};
use crate::pin::Pin;

/// 创建一个新的迭代器，每次迭代都会调用给定 coroutine。
///
/// 类似 [`iter::from_fn`]。
///
/// [`iter::from_fn`]: crate::iter::from_fn
///
/// # 示例
///
/// ```
/// #![feature(coroutines)]
/// #![feature(iter_from_coroutine)]
///
/// let it = std::iter::from_coroutine(#[coroutine] || {
///     yield 1;
///     yield 2;
///     yield 3;
/// });
/// let v: Vec<_> = it.collect();
/// assert_eq!(v, [1, 2, 3]);
/// ```
#[inline]
#[unstable(feature = "iter_from_coroutine", issue = "43122", reason = "coroutines are unstable")]
pub fn from_coroutine<G: Coroutine<Return = ()> + Unpin>(coroutine: G) -> FromCoroutine<G> {
    FromCoroutine(coroutine)
}

/// 遍历底层 coroutine 所 yield 值的迭代器。
///
/// 该 `struct` 由 [`iter::from_coroutine()`] 函数创建。更多信息见该函数文档。
///
/// [`iter::from_coroutine()`]: from_coroutine
#[unstable(feature = "iter_from_coroutine", issue = "43122", reason = "coroutines are unstable")]
#[derive(Clone)]
pub struct FromCoroutine<G>(G);

#[unstable(feature = "iter_from_coroutine", issue = "43122", reason = "coroutines are unstable")]
impl<G: Coroutine<Return = ()> + Unpin> Iterator for FromCoroutine<G> {
    type Item = G::Yield;

    fn next(&mut self) -> Option<Self::Item> {
        match Pin::new(&mut self.0).resume(()) {
            CoroutineState::Yielded(n) => Some(n),
            CoroutineState::Complete(()) => None,
        }
    }
}

#[unstable(feature = "iter_from_coroutine", issue = "43122", reason = "coroutines are unstable")]
impl<G> fmt::Debug for FromCoroutine<G> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FromCoroutine").finish()
    }
}
