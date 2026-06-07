use crate::future::Future;
use crate::marker::Tuple;

/// [`Fn`](crate::ops::Fn) trait 的异步感知(async-aware)版本。
///
/// 所有 `async fn` 以及返回 future 的函数都实现这个 trait。
#[stable(feature = "async_closure", since = "1.85.0")]
#[rustc_paren_sugar]
#[must_use = "async closures are lazy and do nothing unless called"]
#[lang = "async_fn"]
pub trait AsyncFn<Args: Tuple>: AsyncFnMut<Args> {
    /// 调用该 [`AsyncFn`],返回一个可能借用了被调用闭包的 future。
    #[unstable(feature = "async_fn_traits", issue = "none")]
    extern "rust-call" fn async_call(&self, args: Args) -> Self::CallRefFuture<'_>;
}

/// [`FnMut`](crate::ops::FnMut) trait 的异步感知版本。
///
/// 所有 `async fn` 以及返回 future 的函数都实现这个 trait。
#[stable(feature = "async_closure", since = "1.85.0")]
#[rustc_paren_sugar]
#[must_use = "async closures are lazy and do nothing unless called"]
#[lang = "async_fn_mut"]
pub trait AsyncFnMut<Args: Tuple>: AsyncFnOnce<Args> {
    /// 由 [`AsyncFnMut::async_call_mut`] 和 [`AsyncFn::async_call`] 返回的 future。
    #[unstable(feature = "async_fn_traits", issue = "none")]
    #[lang = "call_ref_future"]
    type CallRefFuture<'a>: Future<Output = Self::Output>
    where
        Self: 'a;

    /// 调用该 [`AsyncFnMut`],返回一个可能借用了被调用闭包的 future。
    #[unstable(feature = "async_fn_traits", issue = "none")]
    extern "rust-call" fn async_call_mut(&mut self, args: Args) -> Self::CallRefFuture<'_>;
}

/// [`FnOnce`](crate::ops::FnOnce) trait 的异步感知版本。
///
/// 所有 `async fn` 以及返回 future 的函数都实现这个 trait。
#[stable(feature = "async_closure", since = "1.85.0")]
#[rustc_paren_sugar]
#[must_use = "async closures are lazy and do nothing unless called"]
#[lang = "async_fn_once"]
pub trait AsyncFnOnce<Args: Tuple> {
    /// 由 [`AsyncFnOnce::async_call_once`] 返回的 future。
    #[unstable(feature = "async_fn_traits", issue = "none")]
    #[lang = "call_once_future"]
    type CallOnceFuture: Future<Output = Self::Output>;

    /// 被调用闭包所返回 future 的输出类型。
    #[unstable(feature = "async_fn_traits", issue = "none")]
    #[lang = "async_fn_once_output"]
    type Output;

    /// 调用该 [`AsyncFnOnce`],返回一个可能从被调用闭包中移出值的 future。
    #[unstable(feature = "async_fn_traits", issue = "none")]
    extern "rust-call" fn async_call_once(self, args: Args) -> Self::CallOnceFuture;
}

mod impls {
    use super::{AsyncFn, AsyncFnMut, AsyncFnOnce};
    use crate::marker::Tuple;

    #[stable(feature = "async_closure", since = "1.85.0")]
    impl<A: Tuple, F: ?Sized> AsyncFn<A> for &F
    where
        F: AsyncFn<A>,
    {
        extern "rust-call" fn async_call(&self, args: A) -> Self::CallRefFuture<'_> {
            F::async_call(*self, args)
        }
    }

    #[stable(feature = "async_closure", since = "1.85.0")]
    impl<A: Tuple, F: ?Sized> AsyncFnMut<A> for &F
    where
        F: AsyncFn<A>,
    {
        type CallRefFuture<'a>
            = F::CallRefFuture<'a>
        where
            Self: 'a;

        extern "rust-call" fn async_call_mut(&mut self, args: A) -> Self::CallRefFuture<'_> {
            F::async_call(*self, args)
        }
    }

    #[stable(feature = "async_closure", since = "1.85.0")]
    impl<'a, A: Tuple, F: ?Sized> AsyncFnOnce<A> for &'a F
    where
        F: AsyncFn<A>,
    {
        type Output = F::Output;
        type CallOnceFuture = F::CallRefFuture<'a>;

        extern "rust-call" fn async_call_once(self, args: A) -> Self::CallOnceFuture {
            F::async_call(self, args)
        }
    }

    #[stable(feature = "async_closure", since = "1.85.0")]
    impl<A: Tuple, F: ?Sized> AsyncFnMut<A> for &mut F
    where
        F: AsyncFnMut<A>,
    {
        type CallRefFuture<'a>
            = F::CallRefFuture<'a>
        where
            Self: 'a;

        extern "rust-call" fn async_call_mut(&mut self, args: A) -> Self::CallRefFuture<'_> {
            F::async_call_mut(*self, args)
        }
    }

    #[stable(feature = "async_closure", since = "1.85.0")]
    impl<'a, A: Tuple, F: ?Sized> AsyncFnOnce<A> for &'a mut F
    where
        F: AsyncFnMut<A>,
    {
        type Output = F::Output;
        type CallOnceFuture = F::CallRefFuture<'a>;

        extern "rust-call" fn async_call_once(self, args: A) -> Self::CallOnceFuture {
            F::async_call_mut(self, args)
        }
    }
}

mod internal_implementation_detail {
    /// 一个辅助 trait,用来强制保证某个目标的 `ClosureKind`(闭包种类)处于某个
    /// `CoroutineClosure`(协程闭包)的能力范围之内;它还允许我们把对元组化的
    /// upvar(被捕获变量)类型的投影(projection)推迟到 upvar 分析完成之后再进行。
    ///
    /// `Self` 类型应当是该协程闭包的 `kind_ty`(种类类型),因此要么是 `?0`,要么
    /// 是 `i8`/`i16`/`i32`(关于这一点的解释参见 `ClosureKind` 的文档)。`GoalKind`
    /// 也是同样的类型,但它表示的是该闭包被以何种 trait 的种类来调用。
    #[lang = "async_fn_kind_helper"]
    trait AsyncFnKindHelper<GoalKind> {
        // 把一组闭包输入(参数)、一个区域(region)以及一组 upvar(按 move 和按
        // ref 两种方式)投影为:根据上面的 `GoalKind` 参数,我们期望该协程所拥有的
        // 那组 upvar。
        //
        // `Upvars` 参数应当是父协程闭包的 upvar,而 `BorrowedUpvarsAsFnPtr` 则是
        // 一个形如 `for<'env> fn() -> (&'env T, ...)` 的函数指针。这让我们得以表示
        // 该闭包自捕获(self-capture)的 binder,而这些 upvar 类型会用提供给该关联
        // 类型的 `'closure_env` 区域来实例化。
        #[lang = "async_fn_kind_upvars"]
        type Upvars<'closure_env, Inputs, Upvars, BorrowedUpvarsAsFnPtr>;
    }
}
