use crate::ops::DerefMut;
use crate::pin::Pin;
use crate::task::{Context, Poll};

/// 一个用于处理异步迭代器的 trait。
///
/// 这是主要的异步迭代器 trait。关于异步迭代器这一概念的总体介绍,请参阅[模块级文档]。特别地,
/// 你可能想了解如何[实现 `AsyncIterator`][impl]。
///
/// [模块级文档]: index.html
/// [impl]: index.html#implementing-async-iterator
#[unstable(feature = "async_iterator", issue = "79024")]
#[must_use = "async iterators do nothing unless polled"]
#[doc(alias = "Stream")]
#[lang = "async_iterator"]
pub trait AsyncIterator {
    /// 本异步迭代器产出的元素的类型。
    type Item;

    /// 尝试取出本异步迭代器的下一个值;如果该值尚不可用,则登记当前任务以便后续被唤醒;如果异步
    /// 迭代器已耗尽,则返回 `None`。
    ///
    /// # 返回值
    ///
    /// 有几种可能的返回值,各自表示一种不同的异步迭代器状态:
    ///
    /// - `Poll::Pending` 表示本异步迭代器的下一个值尚未就绪。实现会确保:当下一个值可能就绪时,
    ///   当前任务会被通知(唤醒)。这与 [`Future::poll`](crate::future::Future::poll) 的契约一致——
    ///   返回 `Pending` 前必须已安排好唤醒,否则任务将永久挂起。若多次 `poll_next` 都返回
    ///   `Pending`,实现应以最近一次调用传入的 [`Context`](crate::task::Context) 中的 waker 为准。
    ///
    /// - `Poll::Ready(Some(val))` 表示异步迭代器已成功产出一个值 `val`,并且在后续的 `poll_next`
    ///   调用中可能继续产出更多的值。
    ///
    /// - `Poll::Ready(None)` 表示异步迭代器已终止。调用方看到这一结果后应停止轮询;它不应假定后续
    ///   再次调用 `poll_next` 还能恢复产出元素。
    ///
    /// # Panics
    ///
    /// 一旦异步迭代器已结束(即 `poll_next` 返回过 `Ready(None)`),再次调用它的 `poll_next`
    /// 方法可能 panic、永久阻塞或引发其他各种问题;`AsyncIterator` trait 对这种调用的后果不作
    /// 任何约束。但由于 `poll_next` 方法并未标记为 `unsafe`,Rust 的常规规则仍然适用:无论异步
    /// 迭代器处于何种状态,这类调用**绝不能导致未定义行为**(内存破坏、错误使用 `unsafe` 函数等)。
    #[lang = "async_iterator_poll_next"]
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>>;

    /// 返回异步迭代器剩余长度的上下界。
    ///
    /// 具体来说,`size_hint()` 返回一个元组,其中第一个元素是下界,第二个元素是上界。
    ///
    /// 返回元组的后半部分是一个 <code>[Option]<[usize]></code>。这里的 [`None`] 意味着:要么不存在
    /// 已知的上界,要么上界比 [`usize`] 还大。
    ///
    /// # 实现注意事项
    ///
    /// 并不强制要求异步迭代器的实现恰好产出它所声明的元素数量。一个有缺陷的异步迭代器可能产出
    /// 少于下界或多于上界的元素。
    ///
    /// `size_hint()` 主要用于诸如“为异步迭代器的元素预留空间”这类优化,但**绝不能**被信任到例如
    /// 在 unsafe 代码中省略边界检查的程度。`size_hint()` 的错误实现不应导致内存安全违规。
    ///
    /// 话虽如此,实现仍应给出正确的估计,否则就违反了该 trait 的协议。
    ///
    /// 默认实现返回 <code>(0, [None])</code>,这对任何异步迭代器都是正确的。
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, None)
    }
}

#[unstable(feature = "async_iterator", issue = "79024")]
impl<S: ?Sized + AsyncIterator + Unpin> AsyncIterator for &mut S {
    type Item = S::Item;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        S::poll_next(Pin::new(&mut **self), cx)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (**self).size_hint()
    }
}

#[unstable(feature = "async_iterator", issue = "79024")]
impl<P> AsyncIterator for Pin<P>
where
    P: DerefMut,
    P::Target: AsyncIterator,
{
    type Item = <P::Target as AsyncIterator>::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        <P::Target as AsyncIterator>::poll_next(self.as_deref_mut(), cx)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (**self).size_hint()
    }
}

#[unstable(feature = "async_gen_internals", issue = "none")]
impl<T> Poll<Option<T>> {
    /// 供内部脱糖使用的辅助函数——产生 `Ready(Some(t))`,对应于异步迭代器产出一个值。
    #[doc(hidden)]
    #[unstable(feature = "async_gen_internals", issue = "none")]
    #[lang = "AsyncGenReady"]
    pub fn async_gen_ready(t: T) -> Self {
        Poll::Ready(Some(t))
    }

    /// 供内部脱糖使用的辅助常量——产生 `Pending`,对应于异步迭代器在某个 `.await` 处挂起。
    #[doc(hidden)]
    #[unstable(feature = "async_gen_internals", issue = "none")]
    #[lang = "AsyncGenPending"]
    // FIXME(gen_blocks): 这一项或许可以去重。
    pub const PENDING: Self = Poll::Pending;

    /// 供内部脱糖使用的辅助常量——产生 `Ready(None)`,对应于异步迭代器结束其迭代。
    #[doc(hidden)]
    #[unstable(feature = "async_gen_internals", issue = "none")]
    #[lang = "AsyncGenFinished"]
    pub const FINISHED: Self = Poll::Ready(None);
}

/// 把某个东西转换为异步迭代器
#[unstable(feature = "async_iterator", issue = "79024")]
pub trait IntoAsyncIterator {
    /// 迭代器所产出的元素的类型
    type Item;
    /// 转换得到的迭代器的类型
    type IntoAsyncIter: AsyncIterator<Item = Self::Item>;

    /// 把 `self` 转换为一个异步迭代器
    #[lang = "into_async_iter_into_iter"]
    fn into_async_iter(self) -> Self::IntoAsyncIter;
}

#[unstable(feature = "async_iterator", issue = "79024")]
impl<I: AsyncIterator> IntoAsyncIterator for I {
    type Item = I::Item;
    type IntoAsyncIter = I;

    fn into_async_iter(self) -> Self::IntoAsyncIter {
        self
    }
}
