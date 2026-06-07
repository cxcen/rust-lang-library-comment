//! 可组合的异步迭代。
//!
//! 与 `future`、`task` 模块一样,这里也只定义**协议/契约**:[`AsyncIterator`] trait 描述了
//! “异步地逐个产出元素”这一接口,但 `core` 本身不含任何运行时或调度器。它建立在 [`Poll`] 与
//! [`Waker`](crate::task::Waker) 之上——`poll_next` 返回 `Poll<Option<Item>>`;`Pending` 表示
//! 下一个元素尚未就绪且实现必须安排唤醒,`Ready(Some(item))` 表示产出一个元素,`Ready(None)`
//! 表示异步迭代器终止。真正驱动它前进、在元素就绪时唤醒任务的是上层运行时。
//!
//! 如果你手头有某种异步的集合,并且需要对其中的元素执行某种操作,很快就会遇到“异步迭代器”。
//! 异步迭代器在符合习惯的异步 Rust 代码中被大量使用,因此值得花时间熟悉它们。
//!
//! 在进一步解释之前,先谈谈本模块的结构:
//!
//! # 组织结构
//!
//! 本模块在很大程度上是按类型来组织的:
//!
//! * [Traits] 是核心部分:这些 trait 定义了存在哪些种类的异步迭代器,以及你能用它们做什么。
//!   这些 trait 上的方法值得多花些时间研究。
//! * 函数提供了一些用来创建基础异步迭代器的便利方式。
//! * 结构体往往是本模块各 trait 上各个方法的返回类型。通常你会更想去看创建该 `struct` 的那个
//!   方法,而不是 `struct` 本身。至于原因,详见下面“实现 `AsyncIterator`”一节。
//!
//! [Traits]: #traits
//!
//! 就这些!下面我们深入异步迭代器。
//!
//! # 异步迭代器
//!
//! 本模块的核心与灵魂是 [`AsyncIterator`] trait。[`AsyncIterator`] 的核心看起来是这样的:
//!
//! ```
//! # use core::task::{Context, Poll};
//! # use core::pin::Pin;
//! trait AsyncIterator {
//!     type Item;
//!     fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>>;
//! }
//! ```
//!
//! 与 `Iterator` 不同,`AsyncIterator` 区分了两类方法:实现 `AsyncIterator` 时所用的
//! [`poll_next`] 方法,以及消费异步迭代器时所用的(尚待实现的)`next` 方法。`AsyncIterator`
//! 的消费者只需关心 `next`——调用它会返回一个 future,该 future 产出 `Option<AsyncIterator::Item>`。
//!
//! 只要还有元素,`next` 返回的 future 就会产出 `Some(Item)`;一旦元素全部耗尽,它就会产出 `None`
//! 以表明迭代结束。如果我们正等待某个异步操作解析完成,该 future 会一直等到异步迭代器准备好再次
//! 产出为止。
//!
//! 一旦底层 `poll_next` 返回过 `Ready(None)`,该异步迭代器就被视为已经终止;消费者应停止轮询。
//! 之后再次调用 `poll_next` 的行为由具体实现自行决定,可能 panic、永久挂起或返回任意结果,但由于
//! `poll_next` 不是 unsafe 方法,这类误用仍不得造成未定义行为。
//!
//! [`AsyncIterator`] 的完整定义还包含许多其它方法,但它们都是默认方法,构建在 [`poll_next`]
//! 之上,因此你无需额外实现就能直接获得。
//!
//! [`Poll`]: super::task::Poll
//! [`poll_next`]: AsyncIterator::poll_next
//!
//! # 实现 `AsyncIterator`
//!
//! 创建你自己的异步迭代器需要两步:先创建一个 `struct` 来保存异步迭代器的状态,然后为该 `struct`
//! 实现 [`AsyncIterator`]。
//!
//! 我们来做一个名为 `Counter` 的异步迭代器,它从 `1` 数到 `5`:
//!
//! ```no_run
//! #![feature(async_iterator)]
//! # use core::async_iter::AsyncIterator;
//! # use core::task::{Context, Poll};
//! # use core::pin::Pin;
//!
//! // 首先是 struct:
//!
//! /// 一个从一数到五的异步迭代器
//! struct Counter {
//!     count: usize,
//! }
//!
//! // 我们希望计数从一开始,因此添加一个 new() 方法来辅助。
//! // 这并非严格必要,但很方便。注意我们把 `count` 从零开始,
//! // 至于原因,我们会在下面 `poll_next()` 的实现里看到。
//! impl Counter {
//!     fn new() -> Counter {
//!         Counter { count: 0 }
//!     }
//! }
//!
//! // 然后,我们为 `Counter` 实现 `AsyncIterator`:
//!
//! impl AsyncIterator for Counter {
//!     // 我们将用 usize 来计数
//!     type Item = usize;
//!
//!     // poll_next() 是唯一必须实现的方法
//!     fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
//!         // 让计数加一。这就是我们从零开始的原因。
//!         self.count += 1;
//!
//!         // 检查我们是否已经数完。
//!         if self.count < 6 {
//!             Poll::Ready(Some(self.count))
//!         } else {
//!             Poll::Ready(None)
//!         }
//!     }
//! }
//! ```
//!
//! # 惰性
//!
//! 异步迭代器是*惰性的*。这意味着仅仅创建一个异步迭代器并不会*做*多少事情。在你调用 `poll_next`
//! 之前,实际上什么都不会发生。如果 `poll_next` 返回 `Pending`,实现还必须在未来可能产出元素时
//! 唤醒当前任务;如果没有这样的唤醒路径,消费方就会永久等待。当有人仅仅为了副作用而创建异步迭代器时,
//! 这有时会成为困惑的来源。
//! 编译器会就这类行为向我们发出警告:
//!
//! ```text
//! warning: unused result that must be used: async iterators do nothing unless polled
//! ```

mod async_iter;
mod from_iter;

pub use async_iter::{AsyncIterator, IntoAsyncIterator};
pub use from_iter::{FromIter, from_iter};
