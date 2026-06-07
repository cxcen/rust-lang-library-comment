use crate::marker::{Destruct, PhantomData};
use crate::ops::ControlFlow;

/// `?` 运算符与 `try {}` 代码块。
///
/// `try_*` 系列方法通常会涉及一个实现了这个 trait 的类型。例如,传给
/// [`Iterator::try_fold`] 和 [`Iterator::try_for_each`] 的闭包就必须返回这样
/// 一个类型。
///
/// `Try` 类型通常是那些含有两类或更多类别取值的类型,其中某个子集的取值实在太
/// 常通过提前返回(early return)来处理,以至于值得为之提供一套简洁(但仍然显眼)
/// 的语法来让这件事变得容易。
///
/// 这一点最常见于用 [`Result`] 和 [`Option`] 做错误处理的场合。这个 trait 最
/// 典型的实现是在 [`ControlFlow`] 上。
///
/// # 在泛型代码中使用 `Try`
///
/// `Iterator::try_fold` 早在 Rust 1.27 就已稳定化,但这个 trait 要新得多。为了
/// 说明各个关联类型和方法,我们来实现一个自己的版本。
///
/// 先回顾一下,一个不会失败(infallible)的 fold 大致长这样:
/// ```
/// fn simple_fold<A, T>(
///     iter: impl Iterator<Item = T>,
///     mut accum: A,
///     mut f: impl FnMut(A, T) -> A,
/// ) -> A {
///     for x in iter {
///         accum = f(accum, x);
///     }
///     accum
/// }
/// ```
///
/// 因此,我们需要让 `f` 不再只返回一个 `A`,而是返回某个别的类型——它在“不短路”
/// 的路径上会产出一个 `A`。方便的是,这个类型也正是我们需要从该函数返回的类型。
///
/// 让我们为这个类型添加一个新的泛型参数 `R`,并把它约束到我们想要的输出类型上:
/// ```
/// # #![feature(try_trait_v2)]
/// # use std::ops::Try;
/// fn simple_try_fold_1<A, T, R: Try<Output = A>>(
///     iter: impl Iterator<Item = T>,
///     mut accum: A,
///     mut f: impl FnMut(A, T) -> R,
/// ) -> R {
///     todo!()
/// }
/// ```
///
/// 如果我们走完了整个迭代器,就需要用 [`Try::from_output`] 把累加器(accumulator)
/// 包装进返回类型:
/// ```
/// # #![feature(try_trait_v2)]
/// # use std::ops::{ControlFlow, Try};
/// fn simple_try_fold_2<A, T, R: Try<Output = A>>(
///     iter: impl Iterator<Item = T>,
///     mut accum: A,
///     mut f: impl FnMut(A, T) -> R,
/// ) -> R {
///     for x in iter {
///         let cf = f(accum, x).branch();
///         match cf {
///             ControlFlow::Continue(a) => accum = a,
///             ControlFlow::Break(_) => todo!(),
///         }
///     }
///     R::from_output(accum)
/// }
/// ```
///
/// 我们还需要 [`FromResidual::from_residual`] 来把 residual(残值)转回原来的
/// 类型。但由于它是 `Try` 的 supertrait,我们无需在约束中提到它。所有实现了
/// `Try` 的类型都能从其对应的 residual 重建出来,所以我们直接调用它就好:
/// ```
/// # #![feature(try_trait_v2)]
/// # use std::ops::{ControlFlow, Try};
/// pub fn simple_try_fold_3<A, T, R: Try<Output = A>>(
///     iter: impl Iterator<Item = T>,
///     mut accum: A,
///     mut f: impl FnMut(A, T) -> R,
/// ) -> R {
///     for x in iter {
///         let cf = f(accum, x).branch();
///         match cf {
///             ControlFlow::Continue(a) => accum = a,
///             ControlFlow::Break(r) => return R::from_residual(r),
///         }
///     }
///     R::from_output(accum)
/// }
/// ```
///
/// 但这套“调用 `branch`,然后对它 `match`,如果是 `Break` 就 `return`”的流程,
/// 恰恰就是 `?` 运算符内部所做的事。因此与其手动写这一整套,我们直接用 `?`
/// 即可:
/// ```
/// # #![feature(try_trait_v2)]
/// # use std::ops::Try;
/// fn simple_try_fold<A, T, R: Try<Output = A>>(
///     iter: impl Iterator<Item = T>,
///     mut accum: A,
///     mut f: impl FnMut(A, T) -> R,
/// ) -> R {
///     for x in iter {
///         accum = f(accum, x)?;
///     }
///     R::from_output(accum)
/// }
/// ```
#[unstable(feature = "try_trait_v2", issue = "84277", old_name = "try_trait")]
#[rustc_on_unimplemented(
    on(
        all(from_desugaring = "TryBlock"),
        message = "a `try` block must return `Result` or `Option` \
                    (or another type that implements `{This}`)",
        label = "could not wrap the final value of the block as `{Self}` doesn't implement `Try`",
    ),
    on(
        all(from_desugaring = "QuestionMark"),
        message = "the `?` operator can only be applied to values that implement `{This}`",
        label = "the `?` operator cannot be applied to type `{Self}`"
    )
)]
#[doc(alias = "?")]
#[lang = "Try"]
#[rustc_const_unstable(feature = "const_try", issue = "74935")]
pub const trait Try: [const] FromResidual {
    /// `?` 在 *不* 短路时所产出的值的类型。
    #[unstable(feature = "try_trait_v2", issue = "84277", old_name = "try_trait")]
    type Output;

    /// 在 `?` 短路时,作为其一部分传给 [`FromResidual::from_residual`] 的值的类型。
    ///
    /// 它表示 `Self` 类型中那些 *不* 由 `Output` 类型表示的可能取值。
    ///
    /// # 给实现者的提示(Note to Implementors）
    ///
    /// 这个类型的选取对于相互转换(interconversion)至关重要。与 `Output` 类型
    /// (它往往是一个裸的泛型类型)不同,这个类型通常是某种 newtype(新类型),
    /// 用来给类型“染色”(color),使它能与其他类型的 residual 区分开来。
    ///
    /// 这正是为什么 `Result<T, E>::Residual` 不是 `E`,而是 `Result<Infallible, E>`。
    /// 这样一来,它就与(比如)`ControlFlow<E>::Residual` 有所区别,因此在一个
    /// 返回 `Result` 的方法里,不能对 `ControlFlow` 使用 `?`。
    ///
    /// 如果你正在编写一个实现了 `Try<Output = T>` 的泛型类型 `Foo<T>`,那么通常
    /// 你可以用 `Foo<std::convert::Infallible>` 作为它的 `Residual` 类型:该类型
    /// 会在恰当的位置留有一个“洞”(hole),并保持 residual 的“foo 性”(foo-ness),
    /// 从而要求其他类型必须显式选择加入(opt-in)才能与之相互转换。
    #[unstable(feature = "try_trait_v2", issue = "84277", old_name = "try_trait")]
    type Residual;

    /// 从该类型的 `Output` 类型构造出该类型。
    ///
    /// 它的实现应当与 `branch` 方法保持一致,使得应用 `?` 运算符能取回原来的值:
    /// `Try::from_output(x).branch() --> ControlFlow::Continue(x)`。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(try_trait_v2)]
    /// use std::ops::Try;
    ///
    /// assert_eq!(<Result<_, String> as Try>::from_output(3), Ok(3));
    /// assert_eq!(<Option<_> as Try>::from_output(4), Some(4));
    /// assert_eq!(
    ///     <std::ops::ControlFlow<String, _> as Try>::from_output(5),
    ///     std::ops::ControlFlow::Continue(5),
    /// );
    ///
    /// # fn make_question_mark_work() -> Option<()> {
    /// assert_eq!(Option::from_output(4)?, 4);
    /// # None }
    /// # make_question_mark_work();
    ///
    /// // 例如,在 `try_fold` 中的累加器上就会用到它:
    /// let r = std::iter::empty().try_fold(4, |_, ()| -> Option<_> { unreachable!() });
    /// assert_eq!(r, Some(4));
    /// ```
    #[lang = "from_output"]
    #[unstable(feature = "try_trait_v2", issue = "84277", old_name = "try_trait")]
    fn from_output(output: Self::Output) -> Self;

    /// 在 `?` 中用来决定:该运算符应当产出一个值(因为它返回了
    /// [`ControlFlow::Continue`]),还是应当把一个值向上传播回调用者(因为它返回了
    /// [`ControlFlow::Break`])。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(try_trait_v2)]
    /// use std::ops::{ControlFlow, Try};
    ///
    /// assert_eq!(Ok::<_, String>(3).branch(), ControlFlow::Continue(3));
    /// assert_eq!(Err::<String, _>(3).branch(), ControlFlow::Break(Err(3)));
    ///
    /// assert_eq!(Some(3).branch(), ControlFlow::Continue(3));
    /// assert_eq!(None::<String>.branch(), ControlFlow::Break(None));
    ///
    /// assert_eq!(ControlFlow::<String, _>::Continue(3).branch(), ControlFlow::Continue(3));
    /// assert_eq!(
    ///     ControlFlow::<_, String>::Break(3).branch(),
    ///     ControlFlow::Break(ControlFlow::Break(3)),
    /// );
    /// ```
    #[lang = "branch"]
    #[unstable(feature = "try_trait_v2", issue = "84277", old_name = "try_trait")]
    fn branch(self) -> ControlFlow<Self::Residual, Self::Output>;
}

/// 用来指定:哪些 residual 可以被转换成哪些 [`crate::ops::Try`] 类型。
///
/// 每个 `Try` 类型都需要能从它自己关联的 `Residual` 类型重建出来,但它还可以
/// 拥有额外的 `FromResidual` 实现,以支持与其他 `Try` 类型之间的相互转换。
#[rustc_on_unimplemented(
    on(
        all(
            from_desugaring = "QuestionMark",
            Self = "core::result::Result<T, E>",
            R = "core::option::Option<core::convert::Infallible>",
        ),
        message = "the `?` operator can only be used on `Result`s, not `Option`s, \
            in {ItemContext} that returns `Result`",
        label = "use `.ok_or(...)?` to provide an error compatible with `{Self}`",
        parent_label = "this function returns a `Result`"
    ),
    on(
        all(
            from_desugaring = "QuestionMark",
            Self = "core::result::Result<T, E>",
        ),
        // 在 trait 选择代码中,对于 `?` 里的 `From` 有一条专门的错误信息,
        // 因此这条信息不会在 result-in-result 错误中显示;也正因如此,它的措辞
        // 可以比 `ControlFlow` 的更强硬。
        message = "the `?` operator can only be used on `Result`s \
            in {ItemContext} that returns `Result`",
        label = "this `?` produces `{R}`, which is incompatible with `{Self}`",
        parent_label = "this function returns a `Result`"
    ),
    on(
        all(
            from_desugaring = "QuestionMark",
            Self = "core::option::Option<T>",
            R = "core::result::Result<T, E>",
        ),
        message = "the `?` operator can only be used on `Option`s, not `Result`s, \
            in {ItemContext} that returns `Option`",
        label = "use `.ok()?` if you want to discard the `{R}` error information",
        parent_label = "this function returns an `Option`"
    ),
    on(
        all(
            from_desugaring = "QuestionMark",
            Self = "core::option::Option<T>",
        ),
        // `Option`-in-`Option` 总是能工作,因为这里只有一种可能的 residual,
        // 所以这条信息也可以措辞强硬。
        message = "the `?` operator can only be used on `Option`s \
            in {ItemContext} that returns `Option`",
        label = "this `?` produces `{R}`, which is incompatible with `{Self}`",
        parent_label = "this function returns an `Option`"
    ),
    on(
        all(
            from_desugaring = "QuestionMark",
            Self = "core::ops::control_flow::ControlFlow<B, C>",
            R = "core::ops::control_flow::ControlFlow<B, C>",
        ),
        message = "the `?` operator in {ItemContext} that returns `ControlFlow<B, _>` \
            can only be used on other `ControlFlow<B, _>`s (with the same Break type)",
        label = "this `?` produces `{R}`, which is incompatible with `{Self}`",
        parent_label = "this function returns a `ControlFlow`",
        note = "unlike `Result`, there's no `From`-conversion performed for `ControlFlow`"
    ),
    on(
        all(
            from_desugaring = "QuestionMark",
            Self = "core::ops::control_flow::ControlFlow<B, C>",
            // `R` 不是 `ControlFlow`,因为那种情形在前面已经匹配过了
        ),
        message = "the `?` operator can only be used on `ControlFlow`s \
            in {ItemContext} that returns `ControlFlow`",
        label = "this `?` produces `{R}`, which is incompatible with `{Self}`",
        parent_label = "this function returns a `ControlFlow`",
    ),
    on(
        all(from_desugaring = "QuestionMark"),
        message = "the `?` operator can only be used in {ItemContext} \
                    that returns `Result` or `Option` \
                    (or another type that implements `{This}`)",
        label = "cannot use the `?` operator in {ItemContext} that returns `{Self}`",
        parent_label = "this function should return `Result` or `Option` to accept `?`"
    ),
)]
#[rustc_diagnostic_item = "FromResidual"]
#[unstable(feature = "try_trait_v2", issue = "84277", old_name = "try_trait")]
#[rustc_const_unstable(feature = "const_try", issue = "74935")]
pub const trait FromResidual<R = <Self as Try>::Residual> {
    /// 从一个兼容的 `Residual` 类型构造出该类型。
    ///
    /// 它的实现应当与 `branch` 方法保持一致,使得应用 `?` 运算符能取回一个等价
    /// 的 residual:`FromResidual::from_residual(r).branch() --> ControlFlow::Break(r)`。
    ///(当涉及相互转换时,并不强制要求 residual 与原值 *完全相同*。)
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(try_trait_v2)]
    /// use std::ops::{ControlFlow, FromResidual};
    ///
    /// assert_eq!(Result::<String, i64>::from_residual(Err(3_u8)), Err(3));
    /// assert_eq!(Option::<String>::from_residual(None), None);
    /// assert_eq!(
    ///     ControlFlow::<_, String>::from_residual(ControlFlow::Break(5)),
    ///     ControlFlow::Break(5),
    /// );
    /// ```
    #[lang = "from_residual"]
    #[unstable(feature = "try_trait_v2", issue = "84277", old_name = "try_trait")]
    fn from_residual(residual: R) -> Self;
}

#[unstable(
    feature = "yeet_desugar_details",
    issue = "none",
    reason = "just here to simplify the desugaring; will never be stabilized"
)]
#[inline]
#[track_caller] // 因为 `Result::from_residual` 带有它
#[lang = "from_yeet"]
#[allow(unreachable_pub)] // 未对外暴露,但仍通过 lang-item 被使用
pub fn from_yeet<T, Y>(yeeted: Y) -> T
where
    T: FromResidual<Yeet<Y>>,
{
    FromResidual::from_residual(Yeet(yeeted))
}

/// 允许取回这样一个规范的、实现了 [`Try`] 的类型:它以本类型作为自己的 residual,
/// 并允许它以 `O` 作为自己的 output。
///
/// 如果你把 `Try` trait 想象成把一个类型拆分成它的 [`Try::Output`] 和
/// [`Try::Residual`] 两部分,那么这个 trait 允许把它们重新拼回去。
///
/// 例如,`Result<T, E>: Try<Output = T, Residual = Result<Infallible, E>>`,
/// 而反方向则有
/// `<Result<Infallible, E> as Residual<T>>::TryType = Result<T, E>`。
#[unstable(feature = "try_trait_v2_residual", issue = "91285")]
#[rustc_const_unstable(feature = "const_try_residual", issue = "91285")]
pub const trait Residual<O>: Sized {
    /// 这个元函数(meta-function)的“返回”类型。
    #[unstable(feature = "try_trait_v2_residual", issue = "91285")]
    // FIXME: 本应是被隐含(implied)的
    type TryType: [const] Try<Output = O, Residual = Self>;
}

/// 用在 `try {}` 代码块中,使得 `?` 脱糖所产出的类型取决于 residual 类型 `R`
/// 和该代码块的 output 类型 `O`,而关键在于:它 *不* 像我们直接调用
/// `<_ as FromResidual>::from_residual(r)` 时那样依赖上下文类型。
#[unstable(feature = "try_trait_v2_residual", issue = "91285")]
#[rustc_const_unstable(feature = "const_try_residual", issue = "91285")]
// 需要是 `pub` 以避免 `private type`(私有类型)错误
#[expect(unreachable_pub)]
#[inline] // FIXME: 能用 force 就好了,但会失败 —— 见 #148915
#[lang = "into_try_type"]
pub const fn residual_into_try_type<R: [const] Residual<O>, O>(
    r: R,
) -> <R as Residual<O>>::TryType {
    FromResidual::from_residual(r)
}

#[unstable(feature = "pub_crate_should_not_need_unstable_attr", issue = "none")]
#[allow(type_alias_bounds)]
pub(crate) type ChangeOutputType<T: Try<Residual: Residual<V>>, V> =
    <T::Residual as Residual<V>>::TryType;

/// 一个适配器,用于借助 `Try` 实现来实现那些非 try 的方法。
///
/// 它在概念上与 `Result<T, !>` 相同,但由于它是一个一目了然的 newtype、且没有
/// 到处散落的 `From` 约束,因此在 trait 求解(trait solving)、可居留性
/// (inhabited-ness)检查等方面所需的工作更少。
///
/// 目前并不打算把它对外公开,所以只是 `pub(crate)`。
#[repr(transparent)]
pub(crate) struct NeverShortCircuit<T>(pub T);
// FIXME(const-hack): 等加入 const 闭包后,替换为 `|a| NeverShortCircuit(f(a))`。
pub(crate) struct Wrapped<T, A, F: FnMut(A) -> T> {
    f: F,
    p: PhantomData<(T, A)>,
}
#[rustc_const_unstable(feature = "const_never_short_circuit", issue = "none")]
impl<T, A, F: [const] FnMut(A) -> T + [const] Destruct> const FnOnce<(A,)> for Wrapped<T, A, F> {
    type Output = NeverShortCircuit<T>;

    extern "rust-call" fn call_once(mut self, args: (A,)) -> Self::Output {
        self.call_mut(args)
    }
}
#[rustc_const_unstable(feature = "const_never_short_circuit", issue = "none")]
impl<T, A, F: [const] FnMut(A) -> T> const FnMut<(A,)> for Wrapped<T, A, F> {
    extern "rust-call" fn call_mut(&mut self, (args,): (A,)) -> Self::Output {
        NeverShortCircuit((self.f)(args))
    }
}

impl<T> NeverShortCircuit<T> {
    /// 包装一个一元函数,产生出另一个把其输出包进 `NeverShortCircuit` 的函数。
    ///
    /// 这在借助 `try_` 系列函数来实现不会失败(infallible)的函数时很有用,且不会
    /// 意外地在闭包中捕获额外的泛型参数。
    #[inline]
    pub(crate) const fn wrap_mut_1<A, F>(f: F) -> Wrapped<T, A, F>
    where
        F: [const] FnMut(A) -> T,
    {
        Wrapped { f, p: PhantomData }
    }

    #[inline]
    pub(crate) fn wrap_mut_2<A, B>(mut f: impl FnMut(A, B) -> T) -> impl FnMut(A, B) -> Self {
        move |a, b| NeverShortCircuit(f(a, b))
    }
}

pub(crate) enum NeverShortCircuitResidual {}

#[rustc_const_unstable(feature = "const_never_short_circuit", issue = "none")]
impl<T> const Try for NeverShortCircuit<T> {
    type Output = T;
    type Residual = NeverShortCircuitResidual;

    #[inline]
    fn branch(self) -> ControlFlow<NeverShortCircuitResidual, T> {
        ControlFlow::Continue(self.0)
    }

    #[inline]
    fn from_output(x: T) -> Self {
        NeverShortCircuit(x)
    }
}
#[rustc_const_unstable(feature = "const_never_short_circuit", issue = "none")]
impl<T> const FromResidual for NeverShortCircuit<T> {
    #[inline]
    fn from_residual(never: NeverShortCircuitResidual) -> Self {
        match never {}
    }
}
#[rustc_const_unstable(feature = "const_never_short_circuit", issue = "none")]
impl<T: [const] Destruct> const Residual<T> for NeverShortCircuitResidual {
    type TryType = NeverShortCircuit<T>;
}

/// 在你的类型上实现 `FromResidual<Yeet<T>>`,即可在返回你这个类型的函数中启用
/// `do yeet expr` 语法。
#[unstable(feature = "try_trait_v2_yeet", issue = "96374")]
#[derive(Debug)]
pub struct Yeet<T>(pub T);
