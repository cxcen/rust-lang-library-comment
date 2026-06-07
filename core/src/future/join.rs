#![allow(unused_imports, unused_macros)] // items are used by the macro

use crate::cell::UnsafeCell;
use crate::future::{Future, poll_fn};
use crate::mem;
use crate::pin::Pin;
use crate::task::{Context, Poll, ready};

/// 同时轮询多个 future,在它们全部完成后返回一个包含所有结果的元组。
///
/// 虽然 `join!(a, b).await` 与 `(a.await, b.await)` 相似,但 `join!` 会**并发地**轮询两个
/// future,因此更高效:`(a.await, b.await)` 必须先等 `a` 完成才会开始 `b`,而 `join!` 一开始
/// 就同时推进二者,从而把各自的等待时间重叠起来。
///
/// # 示例
///
/// ```
/// #![feature(future_join)]
///
/// use std::future::join;
///
/// async fn one() -> usize { 1 }
/// async fn two() -> usize { 2 }
///
/// # let _ =  async {
/// let x = join!(one(), two()).await;
/// assert_eq!(x, (1, 2));
/// # };
/// ```
///
/// `join!` 是变长的,可以传入任意数量的 future:
///
/// ```
/// #![feature(future_join)]
///
/// use std::future::join;
///
/// async fn one() -> usize { 1 }
/// async fn two() -> usize { 2 }
/// async fn three() -> usize { 3 }
///
/// # let _ = async {
/// let x = join!(one(), two(), three()).await;
/// assert_eq!(x, (1, 2, 3));
/// # };
/// ```
#[unstable(feature = "future_join", issue = "91642")]
pub macro join( $($fut:expr),+ $(,)? ) {
    // 通过一个内部宏来漏斗式处理,以免泄露实现细节。
    join_internal! {
        current_position: []
        futures_and_positions: []
        munching: [ $($fut)+ ]
    }
}

// FIXME(danielhenrymantilla): 私有宏理论上不应需要稳定性保证。
#[unstable(feature = "future_join", issue = "91642")]
/// 为了能够*命名*元组中第 i 个 future(比如想取第 .4 个),用了如下技巧:
/// `let (_, _, _, _, it, ..) = tuple;`。要做到这一点,需要为第 i 个 future 生成一段
/// 长度为 `i` 的 `_` 重复序列。因此采用递归“逐个取出”的宏展开方式。
macro join_internal {
    // 递归步骤:把每个 future 映射到它的“位置”(下划线的个数)。
    (
        // 为每个已展开的 future 累积一个 token:"_ _ _"。
        current_position: [
            $($underscores:tt)*
        ]
        // 累积 future 及其在元组中的位置:`_0th ()   _1st ( _ ) …`。
        futures_and_positions: [
            $($acc:tt)*
        ]
        // 蚕食(取出)一个 future。
        munching: [
            $current:tt
            $($rest:tt)*
        ]
    ) => (
        join_internal! {
            current_position: [
                $($underscores)*
                _
            ]
            futures_and_positions: [
                $($acc)*
                $current ( $($underscores)* )
            ]
            munching: [
                $($rest)*
            ]
        }
    ),

    // 递归终点:生成最终的输出 future。
    (
        current_position: $_:tt
        futures_and_positions: [
            $(
                $fut_expr:tt ( $($pos:tt)* )
            )*
        ]
        // 没有要蚕食的内容了。
        munching: []
    ) => (
        match ( $( MaybeDone::Future($fut_expr), )* ) { futures => async {
            let mut futures = futures;
            // SAFETY: 这等价于 `pin_mut!`:`futures` 是局部变量,此后不会再被移动,
            // 因此对它栈上的位置做固定(pin)是合法的。
            let mut futures = unsafe { Pin::new_unchecked(&mut futures) };
            poll_fn(move |cx| {
                let mut done = true;
                // 对每个 `fut`,做 pin 投影后轮询它。
                $(
                    // SAFETY: pin 投影——把对元组的固定结构性地投影到其中的字段 `fut`,
                    // 字段不会被移动,因而固定不变量得以保持。
                    let fut = unsafe {
                        futures.as_mut().map_unchecked_mut(|it| {
                            let ( $($pos,)* fut, .. ) = it;
                            fut
                        })
                    };
                    // 尽管写成 `let () = ready!(fut.poll(cx));` 看上去很诱人,但那样会破坏
                    // `join!` 的初衷:对所有 future 急切地开始轮询,从而让各自的等待并行起来。
                    done &= fut.poll(cx).is_ready();
                )*
                if !done {
                    return Poll::Pending;
                }
                // 全部就绪;现在该把所有输出取出来了。

                // SAFETY: `.take_output()` 不会破坏对应 `fut` 的 `Pin` 不变量
                // (只是把已完成的输出取走,不移动尚处于固定状态的 future)。
                let futures = unsafe {
                    futures.as_mut().get_unchecked_mut()
                };
                Poll::Ready(
                    ($(
                        {
                            let ( $($pos,)* fut, .. ) = &mut *futures;
                            fut.take_output().unwrap()
                        }
                    ),*) // <- 不加尾随逗号,因为我们不希望得到 1 元组。
                )
            }).await
        }}
    ),
}

/// 供 `join!` 使用的 future:它会把自身的输出存下来以便之后取走,并且在 ready 之后被再次
/// 轮询也不会 panic。
///
/// 该类型放在私有模块中但声明为 public,仅供上面的宏使用。
#[allow(missing_debug_implementations)]
#[unstable(feature = "future_join", issue = "91642")]
pub enum MaybeDone<F: Future> {
    Future(F),
    Done(F::Output),
    Taken,
}

#[unstable(feature = "future_join", issue = "91642")]
impl<F: Future> MaybeDone<F> {
    pub fn take_output(&mut self) -> Option<F::Output> {
        match *self {
            MaybeDone::Done(_) => match mem::replace(self, Self::Taken) {
                MaybeDone::Done(val) => Some(val),
                _ => unreachable!(),
            },
            _ => None,
        }
    }
}

#[unstable(feature = "future_join", issue = "91642")]
impl<F: Future> Future for MaybeDone<F> {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // SAFETY: 对 `f` 做结构性固定:`self` 已被固定,这里只是把
        // 固定向内投影到 `Future` 变体中的 `f`,并通过 `Pin::new_unchecked` 轮询它,
        // 全程不移动 `f`。
        unsafe {
            // 不要把匹配人体工学与 unsafe 混用。
            match *self.as_mut().get_unchecked_mut() {
                MaybeDone::Future(ref mut f) => {
                    let val = ready!(Pin::new_unchecked(f).poll(cx));
                    self.set(Self::Done(val));
                }
                MaybeDone::Done(_) => {}
                MaybeDone::Taken => unreachable!(),
            }
        }

        Poll::Ready(())
    }
}
