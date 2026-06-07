#![stable(feature = "futures_api", since = "1.36.0")]

//! 异步编程的基础设施。
//!
//! 这里定义的是异步的**协议/契约**而非运行时:`core` 只提供 [`Future`]、[`IntoFuture`]
//! 等 trait 以及 [`pending`]/[`ready`]/[`poll_fn`] 这类极小的适配器,本身**不含任何
//! executor / 运行时 / 反应器(reactor)**。真正驱动 future 前进、监听 I/O 事件、调度
//! 唤醒的运行时由上层库提供(如 tokio、async-std,或 std 中的极简实现)。这些类型位于
//! 异步程序的热路径上,设计目标是零成本抽象。
//!
//! future 的失败或完成都通过返回值 [`Poll<T>`](crate::task::Poll) 暴露,`poll` 本身不靠
//! panic 传递正常的“尚未就绪”状态。
//!
//! 关于 [`async`] 和 [`await`] 关键字以及异步编程的更多内容,请参阅 [async book]。
//!
//! [`async`]: ../../std/keyword.async.html
//! [`await`]: ../../std/keyword.await.html
//! [async book]: https://rust-lang.github.io/async-book/

use crate::ptr::NonNull;
use crate::task::Context;

mod async_drop;
mod future;
mod into_future;
mod join;
mod pending;
mod poll_fn;
mod ready;

#[unstable(feature = "async_drop", issue = "126482")]
pub use async_drop::{AsyncDrop, async_drop_in_place};
#[stable(feature = "into_future", since = "1.64.0")]
pub use into_future::IntoFuture;
#[stable(feature = "future_readiness_fns", since = "1.48.0")]
pub use pending::{Pending, pending};
#[stable(feature = "future_poll_fn", since = "1.64.0")]
pub use poll_fn::{PollFn, poll_fn};
#[stable(feature = "future_readiness_fns", since = "1.48.0")]
pub use ready::{Ready, ready};

#[stable(feature = "futures_api", since = "1.36.0")]
pub use self::future::Future;
#[unstable(feature = "future_join", issue = "91642")]
pub use self::join::join;

/// 之所以需要这个类型,是因为:
///
/// a) 协程(coroutine)无法实现 `for<'a, 'b> Coroutine<&'a mut Context<'b>>`,所以必须改为传递
///    一个裸指针(见 <https://github.com/rust-lang/rust/issues/68923>)。
/// b) 裸指针和 `NonNull` 既不是 `Send` 也不是 `Sync`,若直接使用会让每一个 future 都丢掉
///    Send/Sync,这并非我们想要的结果。
///
/// 同时它也简化了 `.await` 在 HIR 阶段的脱糖(lowering)。
#[lang = "ResumeTy"]
#[doc(hidden)]
#[unstable(feature = "gen_future", issue = "none")]
#[derive(Debug, Copy, Clone)]
pub struct ResumeTy(NonNull<Context<'static>>);

#[unstable(feature = "gen_future", issue = "none")]
unsafe impl Send for ResumeTy {}

#[unstable(feature = "gen_future", issue = "none")]
unsafe impl Sync for ResumeTy {}

#[lang = "get_context"]
#[doc(hidden)]
#[unstable(feature = "gen_future", issue = "none")]
#[must_use]
#[inline]
pub unsafe fn get_context<'a, 'b>(cx: ResumeTy) -> &'a mut Context<'b> {
    // SAFETY: 调用者必须保证 `cx.0` 是一个有效指针,且满足构造可变引用的全部要求
    // (对齐、非空、指向有效且未被别名借用的 `Context`)。
    unsafe { &mut *cx.0.as_ptr().cast() }
}
