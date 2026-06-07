#![stable(feature = "futures_api", since = "1.36.0")]

use crate::convert;
use crate::ops::{self, ControlFlow};

/// 表示某个值是否已经就绪,或者当前任务是否已被安排在将来接收一次唤醒。
///
/// 它由 [`Future::poll`](core::future::Future::poll) 返回,是整个异步协议中传递“就绪 /
/// 未就绪”状态的核心返回类型——`poll` 不靠 panic 表达“尚未算完”,而是返回 `Pending`。
#[must_use = "this `Poll` may be a `Pending` variant, which should be handled"]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
#[lang = "Poll"]
#[stable(feature = "futures_api", since = "1.36.0")]
pub enum Poll<T> {
    /// 表示值已经立即就绪。
    #[lang = "Ready"]
    #[stable(feature = "futures_api", since = "1.36.0")]
    Ready(#[stable(feature = "futures_api", since = "1.36.0")] T),

    /// 表示值尚未就绪。
    ///
    /// 当一个函数返回 `Pending` 时,它**必须**同时确保:当能够继续推进时,当前任务已被安排好
    /// 接收唤醒(即已把 [`Waker`](crate::task::Waker) 注册到合适的地方)。否则任务将永久挂起。
    #[lang = "Pending"]
    #[stable(feature = "futures_api", since = "1.36.0")]
    Pending,
}

impl<T> Poll<T> {
    /// 通过对 `Poll<T>` 中包含的值施加一个函数,把它映射为 `Poll<U>`。
    ///
    /// # 示例
    ///
    /// 把一个 <code>Poll<[String]></code> 转换为 <code>Poll<[usize]></code>,并消耗掉原值:
    ///
    /// [String]: ../../std/string/struct.String.html "String"
    /// ```
    /// # use core::task::Poll;
    /// let poll_some_string = Poll::Ready(String::from("Hello, World!"));
    /// // `Poll::map` 按值接收 self,会消耗掉 `poll_some_string`
    /// let poll_some_len = poll_some_string.map(|s| s.len());
    ///
    /// assert_eq!(poll_some_len, Poll::Ready(13));
    /// ```
    #[stable(feature = "futures_api", since = "1.36.0")]
    #[inline]
    pub fn map<U, F>(self, f: F) -> Poll<U>
    where
        F: FnOnce(T) -> U,
    {
        match self {
            Poll::Ready(t) => Poll::Ready(f(t)),
            Poll::Pending => Poll::Pending,
        }
    }

    /// 如果该 poll 是一个 [`Poll::Ready`] 值,返回 `true`。
    ///
    /// # 示例
    ///
    /// ```
    /// # use core::task::Poll;
    /// let x: Poll<u32> = Poll::Ready(2);
    /// assert_eq!(x.is_ready(), true);
    ///
    /// let x: Poll<u32> = Poll::Pending;
    /// assert_eq!(x.is_ready(), false);
    /// ```
    #[inline]
    #[rustc_const_stable(feature = "const_poll", since = "1.49.0")]
    #[stable(feature = "futures_api", since = "1.36.0")]
    pub const fn is_ready(&self) -> bool {
        matches!(*self, Poll::Ready(_))
    }

    /// 如果该 poll 是一个 [`Pending`] 值,返回 `true`。
    ///
    /// [`Pending`]: Poll::Pending
    ///
    /// # 示例
    ///
    /// ```
    /// # use core::task::Poll;
    /// let x: Poll<u32> = Poll::Ready(2);
    /// assert_eq!(x.is_pending(), false);
    ///
    /// let x: Poll<u32> = Poll::Pending;
    /// assert_eq!(x.is_pending(), true);
    /// ```
    #[inline]
    #[rustc_const_stable(feature = "const_poll", since = "1.49.0")]
    #[stable(feature = "futures_api", since = "1.36.0")]
    pub const fn is_pending(&self) -> bool {
        !self.is_ready()
    }
}

impl<T, E> Poll<Result<T, E>> {
    /// 通过对 `Poll::Ready(Ok)` 中包含的值施加一个函数,把 `Poll<Result<T, E>>` 映射为
    /// `Poll<Result<U, E>>`,其余各变体保持不变。
    ///
    /// 此函数可用于组合两个函数的结果。
    ///
    /// # 示例
    ///
    /// ```
    /// # use core::task::Poll;
    /// let res: Poll<Result<u8, _>> = Poll::Ready("12".parse());
    /// let squared = res.map_ok(|n| n * n);
    /// assert_eq!(squared, Poll::Ready(Ok(144)));
    /// ```
    #[stable(feature = "futures_api", since = "1.36.0")]
    #[inline]
    pub fn map_ok<U, F>(self, f: F) -> Poll<Result<U, E>>
    where
        F: FnOnce(T) -> U,
    {
        match self {
            Poll::Ready(Ok(t)) => Poll::Ready(Ok(f(t))),
            Poll::Ready(Err(e)) => Poll::Ready(Err(e)),
            Poll::Pending => Poll::Pending,
        }
    }

    /// 通过对 `Poll::Ready(Err)` 中包含的值施加一个函数,把 `Poll::Ready<Result<T, E>>`
    /// 映射为 `Poll::Ready<Result<T, U>>`,其余各变体保持不变。
    ///
    /// 此函数可用于在处理错误的同时,让成功的结果原样透传。
    ///
    /// # 示例
    ///
    /// ```
    /// # use core::task::Poll;
    /// let res: Poll<Result<u8, _>> = Poll::Ready("oops".parse());
    /// let res = res.map_err(|_| 0_u8);
    /// assert_eq!(res, Poll::Ready(Err(0)));
    /// ```
    #[stable(feature = "futures_api", since = "1.36.0")]
    #[inline]
    pub fn map_err<U, F>(self, f: F) -> Poll<Result<T, U>>
    where
        F: FnOnce(E) -> U,
    {
        match self {
            Poll::Ready(Ok(t)) => Poll::Ready(Ok(t)),
            Poll::Ready(Err(e)) => Poll::Ready(Err(f(e))),
            Poll::Pending => Poll::Pending,
        }
    }
}

impl<T, E> Poll<Option<Result<T, E>>> {
    /// 通过对 `Poll::Ready(Some(Ok))` 中包含的值施加一个函数,把
    /// `Poll<Option<Result<T, E>>>` 映射为 `Poll<Option<Result<U, E>>>`,其余各变体保持不变。
    ///
    /// 此函数可用于组合两个函数的结果。
    ///
    /// # 示例
    ///
    /// ```
    /// # use core::task::Poll;
    /// let res: Poll<Option<Result<u8, _>>> = Poll::Ready(Some("12".parse()));
    /// let squared = res.map_ok(|n| n * n);
    /// assert_eq!(squared, Poll::Ready(Some(Ok(144))));
    /// ```
    #[stable(feature = "poll_map", since = "1.51.0")]
    #[inline]
    pub fn map_ok<U, F>(self, f: F) -> Poll<Option<Result<U, E>>>
    where
        F: FnOnce(T) -> U,
    {
        match self {
            Poll::Ready(Some(Ok(t))) => Poll::Ready(Some(Ok(f(t)))),
            Poll::Ready(Some(Err(e))) => Poll::Ready(Some(Err(e))),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }

    /// 通过对 `Poll::Ready(Some(Err))` 中包含的值施加一个函数,把
    /// `Poll::Ready<Option<Result<T, E>>>` 映射为 `Poll::Ready<Option<Result<T, F>>>`,
    /// 其余各变体保持不变。
    ///
    /// 此函数可用于在处理错误的同时,让成功的结果原样透传。
    ///
    /// # 示例
    ///
    /// ```
    /// # use core::task::Poll;
    /// let res: Poll<Option<Result<u8, _>>> = Poll::Ready(Some("oops".parse()));
    /// let res = res.map_err(|_| 0_u8);
    /// assert_eq!(res, Poll::Ready(Some(Err(0))));
    /// ```
    #[stable(feature = "poll_map", since = "1.51.0")]
    #[inline]
    pub fn map_err<U, F>(self, f: F) -> Poll<Option<Result<T, U>>>
    where
        F: FnOnce(E) -> U,
    {
        match self {
            Poll::Ready(Some(Ok(t))) => Poll::Ready(Some(Ok(t))),
            Poll::Ready(Some(Err(e))) => Poll::Ready(Some(Err(f(e)))),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

#[stable(feature = "futures_api", since = "1.36.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl<T> const From<T> for Poll<T> {
    /// 把值移动进一个 [`Poll::Ready`],从而构造出 `Poll<T>`。
    ///
    /// # 示例
    ///
    /// ```
    /// # use core::task::Poll;
    /// assert_eq!(Poll::from(true), Poll::Ready(true));
    /// ```
    fn from(t: T) -> Poll<T> {
        Poll::Ready(t)
    }
}

#[unstable(feature = "try_trait_v2", issue = "84277", old_name = "try_trait")]
impl<T, E> ops::Try for Poll<Result<T, E>> {
    type Output = Poll<T>;
    type Residual = Result<convert::Infallible, E>;

    #[inline]
    fn from_output(c: Self::Output) -> Self {
        c.map(Ok)
    }

    #[inline]
    fn branch(self) -> ControlFlow<Self::Residual, Self::Output> {
        match self {
            Poll::Ready(Ok(x)) => ControlFlow::Continue(Poll::Ready(x)),
            Poll::Ready(Err(e)) => ControlFlow::Break(Err(e)),
            Poll::Pending => ControlFlow::Continue(Poll::Pending),
        }
    }
}

#[unstable(feature = "try_trait_v2", issue = "84277", old_name = "try_trait")]
impl<T, E, F: From<E>> ops::FromResidual<Result<convert::Infallible, E>> for Poll<Result<T, F>> {
    #[inline]
    fn from_residual(x: Result<convert::Infallible, E>) -> Self {
        match x {
            Err(e) => Poll::Ready(Err(From::from(e))),
        }
    }
}

#[unstable(feature = "try_trait_v2", issue = "84277", old_name = "try_trait")]
impl<T, E> ops::Try for Poll<Option<Result<T, E>>> {
    type Output = Poll<Option<T>>;
    type Residual = Result<convert::Infallible, E>;

    #[inline]
    fn from_output(c: Self::Output) -> Self {
        c.map(|x| x.map(Ok))
    }

    #[inline]
    fn branch(self) -> ControlFlow<Self::Residual, Self::Output> {
        match self {
            Poll::Ready(Some(Ok(x))) => ControlFlow::Continue(Poll::Ready(Some(x))),
            Poll::Ready(Some(Err(e))) => ControlFlow::Break(Err(e)),
            Poll::Ready(None) => ControlFlow::Continue(Poll::Ready(None)),
            Poll::Pending => ControlFlow::Continue(Poll::Pending),
        }
    }
}

#[unstable(feature = "try_trait_v2", issue = "84277", old_name = "try_trait")]
impl<T, E, F: From<E>> ops::FromResidual<Result<convert::Infallible, E>>
    for Poll<Option<Result<T, F>>>
{
    #[inline]
    fn from_residual(x: Result<convert::Infallible, E>) -> Self {
        match x {
            Err(e) => Poll::Ready(Some(Err(From::from(e)))),
        }
    }
}
