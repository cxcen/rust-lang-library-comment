use crate::marker::Tuple;

/// 接收不可变接收者(`&self`)的那个版本的调用运算符。
///
/// `Fn` 的实例可以被反复调用而不改变状态。
///
/// *请勿把这个 trait(`Fn`)与 [函数指针][function pointers](`fn`)混淆。*
///
/// `Fn` 会被那些只对捕获变量取不可变引用、或根本不捕获任何东西的闭包自动实现,
/// 也会被(安全的)[函数指针][function pointers] 自动实现(有一些注意事项,详见
/// 其文档)。此外,对任何实现了 `Fn` 的类型 `F`,`&F` 也实现 `Fn`。
///
/// 由于 [`FnMut`] 和 [`FnOnce`] 都是 `Fn` 的 supertrait(父 trait),任何 `Fn`
/// 的实例都可以用在期望 [`FnMut`] 或 [`FnOnce`] 的参数位置上。
///
/// 当你想要接受一个类函数类型的参数,并且需要反复调用它而不改变状态时(例如并发
/// 调用它),就用 `Fn` 作为约束。如果你不需要这么严格的要求,则改用 [`FnMut`] 或
/// [`FnOnce`] 作为约束。
///
/// 关于本主题的更多信息,见 [《Rust 程序设计语言》中关于闭包的章节][book]。
///
/// 另外值得一提的是 `Fn` 系列 trait 的特殊语法(例如 `Fn(usize, bool) -> usize`)。
/// 对其技术细节感兴趣的读者可参阅 [《Rustonomicon》中的相关章节][nomicon]。
///
/// [book]: ../../book/ch13-01-closures.html
/// [function pointers]: fn
/// [nomicon]: ../../nomicon/hrtb.html
///
/// # 示例
///
/// ## 调用一个闭包
///
/// ```
/// let square = |x| x * x;
/// assert_eq!(square(5), 25);
/// ```
///
/// ## 使用 `Fn` 参数
///
/// ```
/// fn call_with_one<F>(func: F) -> usize
///     where F: Fn(usize) -> usize {
///     func(1)
/// }
///
/// let double = |x| x * 2;
/// assert_eq!(call_with_one(double), 2);
/// ```
#[lang = "fn"]
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_paren_sugar]
#[rustc_on_unimplemented(
    on(
        Args = "()",
        note = "wrap the `{Self}` in a closure with no arguments: `|| {{ /* code */ }}`"
    ),
    on(
        Self = "unsafe fn",
        note = "unsafe function cannot be called generically without an unsafe block",
        // SAFETY: tidy 还不够聪明,无法判断下面这个 unsafe 代码块只是一个字符串
        label = "call the function in a closure: `|| unsafe {{ /* code */ }}`"
    ),
    message = "expected a `{Trait}` closure, found `{Self}`",
    label = "expected an `{Trait}` closure, found `{Self}`"
)]
#[fundamental] // 这样 regex 就能依赖 `&str: !FnMut`
#[must_use = "closures are lazy and do nothing unless called"]
#[rustc_const_unstable(feature = "const_trait_impl", issue = "143874")]
pub const trait Fn<Args: Tuple>: [const] FnMut<Args> {
    /// 执行调用操作。
    #[unstable(feature = "fn_traits", issue = "29625")]
    extern "rust-call" fn call(&self, args: Args) -> Self::Output;
}

/// 接收可变接收者(`&mut self`)的那个版本的调用运算符。
///
/// `FnMut` 的实例可以被反复调用,并且可能改变状态。
///
/// `FnMut` 会被那些对捕获变量取可变引用的闭包自动实现,也会被所有实现了 [`Fn`]
/// 的类型自动实现,例如(安全的)[函数指针][function pointers](因为 `FnMut` 是
/// [`Fn`] 的 supertrait)。此外,对任何实现了 `FnMut` 的类型 `F`,`&mut F` 也
/// 实现 `FnMut`。
///
/// 由于 [`FnOnce`] 是 `FnMut` 的 supertrait,任何 `FnMut` 的实例都可以用在期望
/// [`FnOnce`] 的地方;又因为 [`Fn`] 是 `FnMut` 的 subtrait(子 trait),任何
/// [`Fn`] 的实例都可以用在期望 `FnMut` 的地方。
///
/// 当你想要接受一个类函数类型的参数,需要反复调用它,同时又允许它改变状态时,
/// 就用 `FnMut` 作为约束。如果你不想让该参数改变状态,则用 [`Fn`] 作为约束;
/// 如果你不需要反复调用它,则用 [`FnOnce`]。
///
/// 关于本主题的更多信息,见 [《Rust 程序设计语言》中关于闭包的章节][book]。
///
/// 另外值得一提的是 `Fn` 系列 trait 的特殊语法(例如 `Fn(usize, bool) -> usize`)。
/// 对其技术细节感兴趣的读者可参阅 [《Rustonomicon》中的相关章节][nomicon]。
///
/// [book]: ../../book/ch13-01-closures.html
/// [function pointers]: fn
/// [nomicon]: ../../nomicon/hrtb.html
///
/// # 示例
///
/// ## 调用一个以可变方式捕获的闭包
///
/// ```
/// let mut x = 5;
/// {
///     let mut square_x = || x *= x;
///     square_x();
/// }
/// assert_eq!(x, 25);
/// ```
///
/// ## 使用 `FnMut` 参数
///
/// ```
/// fn do_twice<F>(mut func: F)
///     where F: FnMut()
/// {
///     func();
///     func();
/// }
///
/// let mut x: usize = 1;
/// {
///     let add_two_to_x = || x += 2;
///     do_twice(add_two_to_x);
/// }
///
/// assert_eq!(x, 5);
/// ```
#[lang = "fn_mut"]
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_paren_sugar]
#[rustc_on_unimplemented(
    on(
        Args = "()",
        note = "wrap the `{Self}` in a closure with no arguments: `|| {{ /* code */ }}`"
    ),
    on(
        Self = "unsafe fn",
        note = "unsafe function cannot be called generically without an unsafe block",
        // SAFETY: tidy 还不够聪明,无法判断下面这个 unsafe 代码块只是一个字符串
        label = "call the function in a closure: `|| unsafe {{ /* code */ }}`"
    ),
    message = "expected a `{Trait}` closure, found `{Self}`",
    label = "expected an `{Trait}` closure, found `{Self}`"
)]
#[fundamental] // 这样 regex 就能依赖 `&str: !FnMut`
#[must_use = "closures are lazy and do nothing unless called"]
#[rustc_const_unstable(feature = "const_trait_impl", issue = "143874")]
pub const trait FnMut<Args: Tuple>: FnOnce<Args> {
    /// 执行调用操作。
    #[unstable(feature = "fn_traits", issue = "29625")]
    extern "rust-call" fn call_mut(&mut self, args: Args) -> Self::Output;
}

/// 接收按值接收者(`self`)的那个版本的调用运算符。
///
/// `FnOnce` 的实例可以被调用,但有可能不能被多次调用。正因如此,如果对某个类型
/// 唯一已知的信息就是它实现了 `FnOnce`,那么它只能被调用一次。
///
/// `FnOnce` 会被那些可能消耗(consume)捕获变量的闭包自动实现,也会被所有实现了
/// [`FnMut`] 的类型自动实现,例如(安全的)[函数指针][function pointers](因为
/// `FnOnce` 是 [`FnMut`] 的 supertrait)。
///
/// 由于 [`Fn`] 和 [`FnMut`] 都是 `FnOnce` 的 subtrait,任何 [`Fn`] 或 [`FnMut`]
/// 的实例都可以用在期望 `FnOnce` 的地方。
///
/// 当你想要接受一个类函数类型的参数,并且只需要调用它一次时,就用 `FnOnce` 作为
/// 约束。如果你需要反复调用该参数,则用 [`FnMut`] 作为约束;如果你还需要它不改变
/// 状态,则用 [`Fn`]。
///
/// 关于本主题的更多信息,见 [《Rust 程序设计语言》中关于闭包的章节][book]。
///
/// 另外值得一提的是 `Fn` 系列 trait 的特殊语法(例如 `Fn(usize, bool) -> usize`)。
/// 对其技术细节感兴趣的读者可参阅 [《Rustonomicon》中的相关章节][nomicon]。
///
/// [book]: ../../book/ch13-01-closures.html
/// [function pointers]: fn
/// [nomicon]: ../../nomicon/hrtb.html
///
/// # 示例
///
/// ## 使用 `FnOnce` 参数
///
/// ```
/// fn consume_with_relish<F>(func: F)
///     where F: FnOnce() -> String
/// {
///     // `func` 会消耗它所捕获的变量,因此它不能被运行超过一次。
///     println!("Consumed: {}", func());
///
///     println!("Delicious!");
///
///     // 再次尝试调用 `func()` 会针对 `func` 抛出 “use of moved
///     // value”(使用了已移动的值)错误。
/// }
///
/// let x = String::from("x");
/// let consume_and_return_x = move || x;
/// consume_with_relish(consume_and_return_x);
///
/// // 至此 `consume_and_return_x` 不能再被调用
/// ```
#[lang = "fn_once"]
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_paren_sugar]
#[rustc_on_unimplemented(
    on(
        Args = "()",
        note = "wrap the `{Self}` in a closure with no arguments: `|| {{ /* code */ }}`"
    ),
    on(
        Self = "unsafe fn",
        note = "unsafe function cannot be called generically without an unsafe block",
        // SAFETY: tidy 还不够聪明,无法判断下面这个 unsafe 代码块只是一个字符串
        label = "call the function in a closure: `|| unsafe {{ /* code */ }}`"
    ),
    message = "expected a `{Trait}` closure, found `{Self}`",
    label = "expected an `{Trait}` closure, found `{Self}`"
)]
#[fundamental] // 这样 regex 就能依赖 `&str: !FnMut`
#[must_use = "closures are lazy and do nothing unless called"]
#[rustc_const_unstable(feature = "const_trait_impl", issue = "143874")]
pub const trait FnOnce<Args: Tuple> {
    /// 使用调用运算符之后返回的类型。
    #[lang = "fn_once_output"]
    #[stable(feature = "fn_once_output", since = "1.12.0")]
    type Output;

    /// 执行调用操作。
    #[unstable(feature = "fn_traits", issue = "29625")]
    extern "rust-call" fn call_once(self, args: Args) -> Self::Output;
}

mod impls {
    use crate::marker::Tuple;

    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_unstable(feature = "const_trait_impl", issue = "143874")]
    impl<A: Tuple, F: ?Sized> const Fn<A> for &F
    where
        F: [const] Fn<A>,
    {
        extern "rust-call" fn call(&self, args: A) -> F::Output {
            (**self).call(args)
        }
    }

    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_unstable(feature = "const_trait_impl", issue = "143874")]
    impl<A: Tuple, F: ?Sized> const FnMut<A> for &F
    where
        F: [const] Fn<A>,
    {
        extern "rust-call" fn call_mut(&mut self, args: A) -> F::Output {
            (**self).call(args)
        }
    }

    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_unstable(feature = "const_trait_impl", issue = "143874")]
    impl<A: Tuple, F: ?Sized> const FnOnce<A> for &F
    where
        F: [const] Fn<A>,
    {
        type Output = F::Output;

        extern "rust-call" fn call_once(self, args: A) -> F::Output {
            (*self).call(args)
        }
    }

    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_unstable(feature = "const_trait_impl", issue = "143874")]
    impl<A: Tuple, F: ?Sized> const FnMut<A> for &mut F
    where
        F: [const] FnMut<A>,
    {
        extern "rust-call" fn call_mut(&mut self, args: A) -> F::Output {
            (*self).call_mut(args)
        }
    }

    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_unstable(feature = "const_trait_impl", issue = "143874")]
    impl<A: Tuple, F: ?Sized> const FnOnce<A> for &mut F
    where
        F: [const] FnMut<A>,
    {
        type Output = F::Output;
        extern "rust-call" fn call_once(self, args: A) -> F::Output {
            (*self).call_mut(args)
        }
    }
}
