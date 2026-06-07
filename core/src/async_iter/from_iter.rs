use crate::async_iter::AsyncIterator;
use crate::pin::Pin;
use crate::task::{Context, Poll};

/// 一个从普通迭代器创建出来的异步迭代器。
///
/// 该异步迭代器由 [`from_iter`] 函数创建。更多信息请参阅其文档。
///
/// [`from_iter`]: fn.from_iter.html
#[unstable(feature = "async_iter_from_iter", issue = "81798")]
#[derive(Clone, Debug)]
pub struct FromIter<I> {
    iter: I,
}

#[unstable(feature = "async_iter_from_iter", issue = "81798")]
impl<I> Unpin for FromIter<I> {}

/// 把一个普通迭代器转换为异步迭代器。
#[unstable(feature = "async_iter_from_iter", issue = "81798")]
pub fn from_iter<I: IntoIterator>(iter: I) -> FromIter<I::IntoIter> {
    FromIter { iter: iter.into_iter() }
}

#[unstable(feature = "async_iter_from_iter", issue = "81798")]
impl<I: Iterator> AsyncIterator for FromIter<I> {
    type Item = I::Item;

    fn poll_next(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Poll::Ready(self.iter.next())
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}
