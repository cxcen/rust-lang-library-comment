use crate::fmt;
use crate::future::Future;
use crate::pin::Pin;
use crate::task::{Context, Poll};

/// 创建一个 future,它包裹一个返回 [`Poll`] 的函数。
///
/// 对该 future 的轮询会被委派给被包裹的函数。如果返回的 future 被固定(pin),那么被包裹函数
/// 所捕获的环境也会被就地固定;因此只要闭包不把它捕获的内容移动出去,它就可以安全地为这些捕获
/// 创建被固定的引用。
///
/// # 示例
///
/// ```
/// # async fn run() {
/// use core::future::poll_fn;
/// use std::task::{Context, Poll};
///
/// fn read_line(_cx: &mut Context<'_>) -> Poll<String> {
///     Poll::Ready("Hello, World!".into())
/// }
///
/// let read_future = poll_fn(read_line);
/// assert_eq!(read_future.await, "Hello, World!".to_owned());
/// # }
/// ```
///
/// ## 捕获一个被固定的状态
///
/// 一个闭包包裹内层 future 的例子:
///
/// ```
/// # async fn run() {
/// use core::future::{self, Future};
/// use core::task::Poll;
///
/// /// 解析为最先完成的那个 future。若平局,则 `a` 胜出。
/// fn naive_select<T>(
///     a: impl Future<Output = T>,
///     b: impl Future<Output = T>,
/// ) -> impl Future<Output = T>
/// {
///     let (mut a, mut b) = (Box::pin(a), Box::pin(b));
///     future::poll_fn(move |cx| {
///         if let Poll::Ready(r) = a.as_mut().poll(cx) {
///             Poll::Ready(r)
///         } else if let Poll::Ready(r) = b.as_mut().poll(cx) {
///             Poll::Ready(r)
///         } else {
///             Poll::Pending
///         }
///     })
/// }
///
/// let a = async { 42 };
/// let b = future::pending();
/// let v = naive_select(a, b).await;
/// assert_eq!(v, 42);
///
/// let a = future::pending();
/// let b = async { 27 };
/// let v = naive_select(a, b).await;
/// assert_eq!(v, 27);
///
/// let a = async { 42 };
/// let b = async { 27 };
/// let v = naive_select(a, b).await;
/// assert_eq!(v, 42); // 平局时偏向 `a`!
/// # }
/// ```
///
/// 这次不使用 [`Box::pin`]:
///
/// [`Box::pin`]: ../../std/boxed/struct.Box.html#method.pin
///
/// ```
/// # async fn run() {
/// use core::future::{self, Future};
/// use core::pin::pin;
/// use core::task::Poll;
///
/// /// 解析为最先完成的那个 future。若平局,则 `a` 胜出。
/// fn naive_select<T>(
///     a: impl Future<Output = T>,
///     b: impl Future<Output = T>,
/// ) -> impl Future<Output = T>
/// {
///     async {
///         let (mut a, mut b) = (pin!(a), pin!(b));
///         future::poll_fn(move |cx| {
///             if let Poll::Ready(r) = a.as_mut().poll(cx) {
///                 Poll::Ready(r)
///             } else if let Poll::Ready(r) = b.as_mut().poll(cx) {
///                 Poll::Ready(r)
///             } else {
///                 Poll::Pending
///             }
///         }).await
///     }
/// }
///
/// let a = async { 42 };
/// let b = future::pending();
/// let v = naive_select(a, b).await;
/// assert_eq!(v, 42);
/// # }
/// ```
///
///   - 注意:正因为身处 `async` 上下文中,我们才得以使用 [`pin!`] 宏,从而无需借助 unsafe 的
///     <code>[Pin::new_unchecked](&mut fut)</code> 构造器。
///
/// [`pin!`]: crate::pin::pin!
#[stable(feature = "future_poll_fn", since = "1.64.0")]
pub fn poll_fn<T, F>(f: F) -> PollFn<F>
where
    F: FnMut(&mut Context<'_>) -> Poll<T>,
{
    PollFn { f }
}

/// 一个包裹了“返回 [`Poll`] 的函数”的 Future。
///
/// 该 `struct` 由 [`poll_fn()`] 创建。更多信息请参阅其文档。
#[must_use = "futures do nothing unless you `.await` or poll them"]
#[stable(feature = "future_poll_fn", since = "1.64.0")]
pub struct PollFn<F> {
    f: F,
}

#[stable(feature = "future_poll_fn", since = "1.64.0")]
impl<F: Unpin> Unpin for PollFn<F> {}

#[stable(feature = "future_poll_fn", since = "1.64.0")]
impl<F> fmt::Debug for PollFn<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PollFn").finish()
    }
}

#[stable(feature = "future_poll_fn", since = "1.64.0")]
impl<T, F> Future for PollFn<F>
where
    F: FnMut(&mut Context<'_>) -> Poll<T>,
{
    type Output = T;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<T> {
        // SAFETY: 我们没有把被固定字段中的值移动出来,只是取得对 `f` 的可变引用并调用它。
        (unsafe { &mut self.get_unchecked_mut().f })(cx)
    }
}
