/// 从 [`Poll<T>`] 中提取出成功(就绪)的那个值。
///
/// 该宏内建了对 [`Pending`] 信号的传播:一旦遇到 `Pending` 就提前 `return`。
///
/// [`Poll<T>`]: crate::task::Poll
/// [`Pending`]: crate::task::Poll::Pending
///
/// # 示例
///
/// ```
/// use std::task::{ready, Context, Poll};
/// use std::future::{self, Future};
/// use std::pin::Pin;
///
/// pub fn do_poll(cx: &mut Context<'_>) -> Poll<()> {
///     let mut fut = future::ready(42);
///     let fut = Pin::new(&mut fut);
///
///     let num = ready!(fut.poll(cx));
///     # let _ = num;
///     // ... 使用 num
///
///     Poll::Ready(())
/// }
/// ```
///
/// 这一句 `ready!` 调用会展开为:
///
/// ```
/// # use std::task::{Context, Poll};
/// # use std::future::{self, Future};
/// # use std::pin::Pin;
/// #
/// # pub fn do_poll(cx: &mut Context<'_>) -> Poll<()> {
///     # let mut fut = future::ready(42);
///     # let fut = Pin::new(&mut fut);
///     #
/// let num = match fut.poll(cx) {
///     Poll::Ready(t) => t,
///     Poll::Pending => return Poll::Pending,
/// };
///     # let _ = num; // 消除“未使用”警告
///     # // ... 使用 num
///     #
///     # Poll::Ready(())
/// # }
/// ```
#[stable(feature = "ready_macro", since = "1.64.0")]
#[rustc_macro_transparency = "semiopaque"]
pub macro ready($e:expr) {
    match $e {
        $crate::task::Poll::Ready(t) => t,
        $crate::task::Poll::Pending => {
            return $crate::task::Poll::Pending;
        }
    }
}
