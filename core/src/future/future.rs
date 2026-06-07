#![stable(feature = "futures_api", since = "1.36.0")]

use crate::ops;
use crate::pin::Pin;
use crate::task::{Context, Poll};

/// future 代表一次异步计算,通常通过 [`async`] 块或 `async fn` 获得。
///
/// future 是一个“可能尚未算完”的值。这种“异步值”使得线程在等待结果可用期间,可以转去做
/// 其他有用的工作。
///
/// # `poll` 方法
///
/// future 的核心方法 `poll` 会*尝试*把 future 推进到最终结果。该方法在值还没就绪时**不会
/// 阻塞**,而是把当前任务登记好,以便将来能继续推进时被唤醒、再次 `poll`。传入 `poll` 的
/// `context` 提供了一个 [`Waker`],它是唤醒当前任务的句柄。
///
/// 使用 future 时通常不直接调用 `poll`,而是对其值使用 `.await`。
///
/// future 没有单独的“取消”方法。调用方若在 future 返回 `Poll::Ready` 之前丢弃它,就等价于取消
/// 这次异步计算:状态机会被正常 drop,但运行时不会再承诺继续调用 `poll`,也不会额外发送取消通知。
/// 因此实现者需要把释放资源、注销 I/O 兴趣、减少引用计数等清理逻辑放在自身的 drop 路径中,
/// 而不能假定取消时还会再收到一次 `poll`。
///
/// [`async`]: ../../std/keyword.async.html
/// [`Waker`]: crate::task::Waker
#[doc(notable_trait)]
#[doc(search_unbox)]
#[must_use = "futures do nothing unless you `.await` or poll them"]
#[stable(feature = "futures_api", since = "1.36.0")]
#[lang = "future_trait"]
#[diagnostic::on_unimplemented(
    label = "`{Self}` is not a future",
    message = "`{Self}` is not a future"
)]
pub trait Future {
    /// 计算完成时产生的值的类型。
    #[stable(feature = "futures_api", since = "1.36.0")]
    #[lang = "future_output"]
    type Output;

    /// 尝试把 future 推进到最终结果;若结果尚不可用,则登记当前任务以便后续被唤醒。
    ///
    /// # 返回值
    ///
    /// 本函数返回:
    ///
    /// - [`Poll::Pending`]:表示 future 还没就绪。
    /// - [`Poll::Ready(val)`]:表示 future 已成功完成,`val` 是它的结果。
    ///
    /// **完成契约**:一旦 future 返回了 `Poll::Ready`(即已完成),调用方就**不得再次
    /// `poll` 它**(即“完成后再次 poll”)。再次 `poll` 的行为不被 `Future`
    /// trait 约束,可能 panic、永久阻塞或产生其他问题(详见下方 `# Panics`)。
    ///
    /// **唤醒契约**:当 future 尚未就绪时,`poll` 返回 `Poll::Pending`,并且在返回 `Pending`
    /// **之前必须确保已经安排好唤醒**——通常做法是从当前 [`Context`] 中取出 [`Waker`] 并
    /// `clone()` 一份保存到某处,等到能继续推进时再调用 [`Waker::wake`]。如果 future 返回了
    /// `Pending` 却没有安排任何唤醒途径,那么它将**永久挂起**,因为运行时不会知道何时该重新
    /// 调度它。举例来说,一个等待某个套接字变为可读的 future,会 `.clone()` 这个 [`Waker`]
    /// 并保存下来;当别处的信号表明该套接字已可读时,就调用 [`Waker::wake`],于是该 future
    /// 所属的任务被唤醒。任务被唤醒后应当再次 `poll` 这个 future,这一次 `poll` 可能产生最终
    /// 值,也可能仍然返回 `Pending`。
    ///
    /// 注意:`poll` 接收的是 [`Pin<&mut Self>`](crate::pin::Pin),因为 future(尤其是由
    /// `async` 块生成的状态机)可能是自引用的、因而不可移动;`Pin` 保证了在 `poll` 期间
    /// 其内存地址不会改变。
    ///
    /// 还要注意:在多次 `poll` 调用中,只应安排最近一次调用所传入 [`Context`] 中的 [`Waker`]
    /// 去接收唤醒(见下文 `Context` 不保证跨 `poll` 相同的说明)。
    ///
    /// **取消契约**:丢弃一个尚未完成的 future 就是取消它。取消不会通过 `Poll` 返回值表达,也不会
    /// 要求执行器再次调用 `poll`;它只表现为 future 的字段按 Rust 的析构规则被 drop。已经克隆并
    /// 注册到外部事件源中的 [`Waker`] 可能仍然存在,但这些唤醒最多只能让执行器发现任务已不再需要
    /// 继续推进,不能复活已经被 drop 的 future。
    ///
    /// # 运行时特征
    ///
    /// future 自身是*惰性的*:必须被*主动* `poll` 底层计算才会前进。这意味着每当当前任务被
    /// 唤醒,它都应主动对自己仍然关心的、处于 `Pending` 状态的 future 重新 `poll`。
    ///
    /// 话虽如此,某些 future 代表的值实际上是在另一个任务里计算的。此时该 future 的底层计算
    /// 只是作为一个“管道”,传递由那个独立运行的任务算出的值。这类 future 通常是在把新任务
    /// spawn 到某个异步运行时时获得的。
    ///
    /// 不应在一个紧凑的循环里反复调用 `poll`,而应仅在 future 表明自己已准备好继续推进时
    /// (即它调用了 `wake()`)才去 `poll`。如果你熟悉 Unix 上的 `poll(2)` 或 `select(2)`
    /// 系统调用,值得一提的是:future 通常**不会**遇到“每次唤醒都得轮询所有事件”的问题,
    /// 它们更接近 `epoll(4)` 的语义。
    ///
    /// `poll` 的实现应当力求快速返回,不要阻塞。快速返回可以避免不必要地占满线程或事件循环。
    /// 如果事先就知道某次 `poll` 可能会耗时较久,应当把这部分工作下放到线程池(或类似机制),
    /// 以确保 `poll` 能尽快返回。
    ///
    /// # Panics
    ///
    /// 一旦 future 已完成(`poll` 返回过 `Ready`),再次调用它的 `poll` 方法可能 panic、永久
    /// 阻塞或引发其他各种问题;`Future` trait 对这种调用的后果不作任何约束。但由于 `poll`
    /// 方法并未标记为 `unsafe`,Rust 的常规规则仍然适用:无论 future 处于何种状态,这类调用
    /// **绝不能导致未定义行为**(内存破坏、错误使用 `unsafe` 函数等)。
    ///
    /// [`Poll::Ready(val)`]: Poll::Ready
    /// [`Context`]: crate::task::Context
    /// [`Waker`]: crate::task::Waker
    /// [`Waker::wake`]: crate::task::Waker::wake
    #[lang = "poll"]
    #[stable(feature = "futures_api", since = "1.36.0")]
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output>;
}

#[stable(feature = "futures_api", since = "1.36.0")]
impl<F: ?Sized + Future + Unpin> Future for &mut F {
    type Output = F::Output;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        F::poll(Pin::new(&mut **self), cx)
    }
}

#[stable(feature = "futures_api", since = "1.36.0")]
impl<P> Future for Pin<P>
where
    P: ops::DerefMut<Target: Future>,
{
    type Output = <<P as ops::Deref>::Target as Future>::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        <P::Target as Future>::poll(self.as_deref_mut(), cx)
    }
}
