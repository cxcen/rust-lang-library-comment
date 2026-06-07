use crate::pin::Pin;

/// 协程一次 resume(恢复执行)的结果。
///
/// 这个枚举由 `Coroutine::resume` 方法返回,表示一个协程可能的返回值。目前它
/// 对应于两种情形之一:挂起点(`Yielded`)或终止点(`Complete`)。
#[derive(Clone, Copy, PartialEq, PartialOrd, Eq, Ord, Debug, Hash)]
#[lang = "coroutine_state"]
#[unstable(feature = "coroutine_trait", issue = "43122")]
pub enum CoroutineState<Y, R> {
    /// 协程带着一个值挂起了。
    ///
    /// 这个状态表示协程已被挂起,通常对应于一条 `yield` 语句。该变体中携带的值
    /// 对应于传给 `yield` 的表达式,从而允许协程在每次 yield 时都提供一个值。
    Yielded(Y),

    /// 协程带着一个返回值执行完毕了。
    ///
    /// 这个状态表示协程已携带所提供的值结束执行。一旦协程返回了 `Complete`,
    /// 再对它调用 `resume` 就被视为程序员错误。
    Complete(R),
}

/// 由内建协程类型实现的 trait。
///
/// 协程目前是 Rust 中的一项实验性语言特性。它在 [RFC 2033] 中被引入,当前主要
/// 意在为 async/await 语法提供基础构件,但很可能还会扩展为给迭代器及其他原语
/// 提供符合人体工程学的定义方式。
///
/// 协程的语法与语义都是不稳定的,要稳定化还需要进一步的 RFC。不过目前其语法
/// 与闭包类似:
///
/// ```rust
/// #![feature(coroutines)]
/// #![feature(coroutine_trait)]
/// #![feature(stmt_expr_attributes)]
///
/// use std::ops::{Coroutine, CoroutineState};
/// use std::pin::Pin;
///
/// fn main() {
///     let mut coroutine = #[coroutine] || {
///         yield 1;
///         "foo"
///     };
///
///     match Pin::new(&mut coroutine).resume(()) {
///         CoroutineState::Yielded(1) => {}
///         _ => panic!("unexpected return from resume"),
///     }
///     match Pin::new(&mut coroutine).resume(()) {
///         CoroutineState::Complete("foo") => {}
///         _ => panic!("unexpected return from resume"),
///     }
/// }
/// ```
///
/// 关于协程的更多文档可在 [unstable book] 中找到。
///
/// [RFC 2033]: https://github.com/rust-lang/rfcs/pull/2033
/// [unstable book]: ../../unstable-book/language-features/coroutines.html
#[lang = "coroutine"]
#[unstable(feature = "coroutine_trait", issue = "43122")]
#[fundamental]
#[must_use = "coroutines are lazy and do nothing unless resumed"]
pub trait Coroutine<R = ()> {
    /// 该协程 yield 出来的值的类型。
    ///
    /// 这个关联类型对应于 `yield` 表达式,也就是协程每次 yield 时允许返回的值
    /// 的类型。例如,把迭代器实现为协程时,这个类型很可能就是被迭代的元素类型 `T`。
    #[lang = "coroutine_yield"]
    type Yield;

    /// 该协程返回的值的类型。
    ///
    /// 这对应于协程通过 `return` 语句、或作为协程字面量最后一个表达式隐式返回的
    /// 值的类型。例如,future 会把它用作 `Result<T, E>`,因为它代表一个已完成的
    /// future。
    #[lang = "coroutine_return"]
    type Return;

    /// 恢复(resume)该协程的执行。
    ///
    /// 此函数会恢复协程的执行,如果它尚未开始执行则启动它。这次调用会回到协程
    /// 上一次的挂起点,从最近的 `yield` 处继续执行。协程会一直执行,直到它再次
    /// yield 或返回为止,届时此函数返回。
    ///
    /// # Return value
    ///
    /// 此函数返回的 `CoroutineState` 枚举表示协程返回时所处的状态。如果返回的是
    /// `Yielded` 变体,则说明协程到达了一个挂起点并 yield 出了一个值。处于这种
    /// 状态的协程可供之后再次恢复执行。
    ///
    /// 如果返回的是 `Complete`,则说明协程已携带所提供的值彻底结束。此时再次恢复
    /// 该协程是无效的。
    ///
    /// # Panics
    ///
    /// 如果在此前已返回过 `Complete` 变体之后再调用此函数,它可能会 panic。语言
    /// 中的协程字面量在 `Complete` 之后再被 resume 时一定会 panic,但这一点对
    /// `Coroutine` trait 的所有实现并不做保证。
    ///
    /// 关于设计:`resume` 接收 `self: Pin<&mut Self>` 而非普通的 `&mut self`,
    /// 是因为协程会被编译器编译成一个状态机,它在挂起点之间需要把局部变量保存在
    /// 自身内部,这些变量之间可能存在内部引用(即“自引用”,self-referential)。
    /// 一旦协程开始执行,把它移动到别处就会让这些内部指针失效;`Pin` 正是用来在
    /// 类型系统层面保证协程在被 resume 后不再被移动,从而维护这些自引用指针的有效性。
    #[lang = "coroutine_resume"]
    fn resume(self: Pin<&mut Self>, arg: R) -> CoroutineState<Self::Yield, Self::Return>;
}

#[unstable(feature = "coroutine_trait", issue = "43122")]
impl<G: ?Sized + Coroutine<R>, R> Coroutine<R> for Pin<&mut G> {
    type Yield = G::Yield;
    type Return = G::Return;

    fn resume(mut self: Pin<&mut Self>, arg: R) -> CoroutineState<Self::Yield, Self::Return> {
        G::resume((*self).as_mut(), arg)
    }
}

#[unstable(feature = "coroutine_trait", issue = "43122")]
impl<G: ?Sized + Coroutine<R> + Unpin, R> Coroutine<R> for &mut G {
    type Yield = G::Yield;
    type Return = G::Return;

    fn resume(mut self: Pin<&mut Self>, arg: R) -> CoroutineState<Self::Yield, Self::Return> {
        G::resume(Pin::new(&mut *self), arg)
    }
}
