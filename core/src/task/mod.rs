#![stable(feature = "futures_api", since = "1.36.0")]

//! 用于处理异步任务的类型与 trait。
//!
//! 与 `future` 模块一样,这里也只定义异步的**协议/契约**,不含任何运行时:
//! [`Poll`] 描述一次 `poll` 的两种结果(就绪/挂起),[`Waker`]/[`RawWaker`]/
//! [`RawWakerVTable`] 定义了“如何唤醒一个被挂起的任务”这一接口,而 [`Context`] 则是
//! `poll` 调用时承载 [`Waker`] 的载体。真正实现这些接口、负责调度与唤醒任务的是上层运行时
//! (如 tokio),`core` 本身不提供。

mod poll;
#[stable(feature = "futures_api", since = "1.36.0")]
pub use self::poll::Poll;

mod wake;
#[stable(feature = "futures_api", since = "1.36.0")]
pub use self::wake::{Context, ContextBuilder, LocalWaker, RawWaker, RawWakerVTable, Waker};

mod ready;
#[stable(feature = "ready_macro", since = "1.64.0")]
pub use ready::ready;
