use crate::future::Future;
use crate::pin::Pin;
use crate::task::{Context, Poll};

/// 一个立即就绪、携带一个值的 future。
///
/// 该 `struct` 由 [`ready()`] 创建。更多信息请参阅其文档。
#[stable(feature = "future_readiness_fns", since = "1.48.0")]
#[derive(Debug, Clone)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
pub struct Ready<T>(Option<T>);

#[stable(feature = "future_readiness_fns", since = "1.48.0")]
impl<T> Unpin for Ready<T> {}

#[stable(feature = "future_readiness_fns", since = "1.48.0")]
impl<T> Future for Ready<T> {
    type Output = T;

    #[inline]
    fn poll(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<T> {
        Poll::Ready(self.0.take().expect("`Ready` polled after completion"))
    }
}

impl<T> Ready<T> {
    /// 消耗这个 `Ready`,返回其中被包裹的值。
    ///
    /// # Panics
    ///
    /// 如果该 [`Ready`] 已经被 poll 至完成,则会 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::future;
    ///
    /// let a = future::ready(1);
    /// assert_eq!(a.into_inner(), 1);
    /// ```
    #[stable(feature = "ready_into_inner", since = "1.82.0")]
    #[must_use]
    #[inline]
    pub fn into_inner(self) -> T {
        self.0.expect("Called `into_inner()` on `Ready` after completion")
    }
}

/// 创建一个立即就绪、携带一个值的 future。
///
/// 通过本函数创建的 future,在功能上与通过 `async {}` 创建的 future 相似。主要区别在于:本函数
/// 创建的 future 是具名的,并且实现了 `Unpin`。
///
/// # 示例
///
/// ```
/// use std::future;
///
/// # async fn run() {
/// let a = future::ready(1);
/// assert_eq!(a.await, 1);
/// # }
/// ```
#[stable(feature = "future_readiness_fns", since = "1.48.0")]
pub fn ready<T>(t: T) -> Ready<T> {
    Ready(Some(t))
}
