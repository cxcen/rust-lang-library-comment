#[doc = include_str!("panic.md")]
#[macro_export]
#[rustc_builtin_macro(core_panic)]
#[allow_internal_unstable(edition_panic)]
#[stable(feature = "core", since = "1.6.0")]
#[rustc_diagnostic_item = "core_panic_macro"]
macro_rules! panic {
    // 根据调用者的 edition,展开为 `$crate::panic::panic_2015`
    // 或 `$crate::panic::panic_2021`。
    ($($arg:tt)*) => {
        /* compiler built-in */
    };
}

/// 断言两个表达式彼此相等(使用 [`PartialEq`])。
///
/// 断言在 debug 和 release 构建中都会始终检查,且无法禁用。若需要在
/// release 构建中默认禁用的断言,请参见 [`debug_assert_eq!`]。
///
/// [`debug_assert_eq!`]: crate::debug_assert_eq
///
/// panic 时,本宏会用表达式的 debug 表示打印其值。
///
/// 与 [`assert!`] 类似,本宏还有第二种形式,可提供自定义 panic 消息。
///
/// # 示例
///
/// ```
/// let a = 3;
/// let b = 1 + 2;
/// assert_eq!(a, b);
///
/// assert_eq!(a, b, "we are testing addition with {} and {}", a, b);
/// ```
#[macro_export]
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_diagnostic_item = "assert_eq_macro"]
#[allow_internal_unstable(panic_internals)]
macro_rules! assert_eq {
    ($left:expr, $right:expr $(,)?) => {
        match (&$left, &$right) {
            (left_val, right_val) => {
                if !(*left_val == *right_val) {
                    let kind = $crate::panicking::AssertKind::Eq;
                    // 下面的再借用是有意的。没有它们时,借用的栈槽会在值比较之前
                    // 就被初始化,导致明显变慢。
                    $crate::panicking::assert_failed(kind, &*left_val, &*right_val, $crate::option::Option::None);
                }
            }
        }
    };
    ($left:expr, $right:expr, $($arg:tt)+) => {
        match (&$left, &$right) {
            (left_val, right_val) => {
                if !(*left_val == *right_val) {
                    let kind = $crate::panicking::AssertKind::Eq;
                    // 下面的再借用是有意的。没有它们时,借用的栈槽会在值比较之前
                    // 就被初始化,导致明显变慢。
                    $crate::panicking::assert_failed(kind, &*left_val, &*right_val, $crate::option::Option::Some($crate::format_args!($($arg)+)));
                }
            }
        }
    };
}

/// 断言两个表达式彼此不相等(使用 [`PartialEq`])。
///
/// 断言在 debug 和 release 构建中都会始终检查,且无法禁用。若需要在
/// release 构建中默认禁用的断言,请参见 [`debug_assert_ne!`]。
///
/// [`debug_assert_ne!`]: crate::debug_assert_ne
///
/// panic 时,本宏会用表达式的 debug 表示打印其值。
///
/// 与 [`assert!`] 类似,本宏还有第二种形式,可提供自定义 panic 消息。
///
/// # 示例
///
/// ```
/// let a = 3;
/// let b = 2;
/// assert_ne!(a, b);
///
/// assert_ne!(a, b, "we are testing that the values are not equal");
/// ```
#[macro_export]
#[stable(feature = "assert_ne", since = "1.13.0")]
#[rustc_diagnostic_item = "assert_ne_macro"]
#[allow_internal_unstable(panic_internals)]
macro_rules! assert_ne {
    ($left:expr, $right:expr $(,)?) => {
        match (&$left, &$right) {
            (left_val, right_val) => {
                if *left_val == *right_val {
                    let kind = $crate::panicking::AssertKind::Ne;
                    // 下面的再借用是有意的。没有它们时,借用的栈槽会在值比较之前
                    // 就被初始化,导致明显变慢。
                    $crate::panicking::assert_failed(kind, &*left_val, &*right_val, $crate::option::Option::None);
                }
            }
        }
    };
    ($left:expr, $right:expr, $($arg:tt)+) => {
        match (&($left), &($right)) {
            (left_val, right_val) => {
                if *left_val == *right_val {
                    let kind = $crate::panicking::AssertKind::Ne;
                    // 下面的再借用是有意的。没有它们时,借用的栈槽会在值比较之前
                    // 就被初始化,导致明显变慢。
                    $crate::panicking::assert_failed(kind, &*left_val, &*right_val, $crate::option::Option::Some($crate::format_args!($($arg)+)));
                }
            }
        }
    };
}

/// 断言一个表达式匹配给定模式。
///
/// 相比 `assert!(matches!(value, pattern))`,通常更推荐使用本宏,因为它能够打印
/// 未满足期望的实际值形状的 debug 表示。相比之下,使用 [`assert!`] 只会打印
/// 期望未满足,而不会说明原因。
///
/// 模式语法与 match arm 和 `matches!` 宏中的语法完全相同。可选的 if guard
/// 可用于添加额外检查;这些检查必须对匹配到的值为 true,否则本宏会 panic。
///
/// 断言在 debug 和 release 构建中都会始终检查,且无法禁用。若需要在
/// release 构建中默认禁用的断言,请参见 [`debug_assert_matches!`]。
///
/// [`debug_assert_matches!`]: crate::assert_matches::debug_assert_matches
///
/// panic 时,本宏会用表达式的 debug 表示打印其值。
///
/// 与 [`assert!`] 类似,本宏还有第二种形式,可提供自定义 panic 消息。
///
/// # 示例
///
/// ```
/// #![feature(assert_matches)]
///
/// use std::assert_matches::assert_matches;
///
/// let a = Some(345);
/// let b = Some(56);
/// assert_matches!(a, Some(_));
/// assert_matches!(b, Some(_));
///
/// assert_matches!(a, Some(345));
/// assert_matches!(a, Some(345) | None);
///
/// // assert_matches!(a, None); // 会 panic
/// // assert_matches!(b, Some(345)); // 会 panic
/// // assert_matches!(b, Some(345) | None); // 会 panic
///
/// assert_matches!(a, Some(x) if x > 100);
/// // assert_matches!(a, Some(x) if x < 100); // 会 panic
/// ```
#[unstable(feature = "assert_matches", issue = "82775")]
#[allow_internal_unstable(panic_internals)]
#[rustc_macro_transparency = "semiopaque"]
pub macro assert_matches {
    ($left:expr, $(|)? $( $pattern:pat_param )|+ $( if $guard: expr )? $(,)?) => {
        match $left {
            $( $pattern )|+ $( if $guard )? => {}
            ref left_val => {
                $crate::panicking::assert_matches_failed(
                    left_val,
                    $crate::stringify!($($pattern)|+ $(if $guard)?),
                    $crate::option::Option::None
                );
            }
        }
    },
    ($left:expr, $(|)? $( $pattern:pat_param )|+ $( if $guard: expr )?, $($arg:tt)+) => {
        match $left {
            $( $pattern )|+ $( if $guard )? => {}
            ref left_val => {
                $crate::panicking::assert_matches_failed(
                    left_val,
                    $crate::stringify!($($pattern)|+ $(if $guard)?),
                    $crate::option::Option::Some($crate::format_args!($($arg)+))
                );
            }
        }
    },
}

/// 根据 `cfg` 谓词在编译期选择代码。
///
/// 本宏会在编译期求值一系列 `cfg` 谓词,选择第一个为 true 的谓词,
/// 并发出由该谓词守卫的代码。由其他谓词守卫的代码不会被发出。
///
/// 可选的尾随 `_` 通配符可用于指定 fallback。如果没有任何谓词为 true,
/// 则会发出 [`compile_error`]。
///
/// # 示例
///
/// ```
/// #![feature(cfg_select)]
///
/// cfg_select! {
///     unix => {
///         fn foo() { /* unix specific functionality */ }
///     }
///     target_pointer_width = "32" => {
///         fn foo() { /* non-unix, 32-bit functionality */ }
///     }
///     _ => {
///         fn foo() { /* fallback implementation */ }
///     }
/// }
/// ```
///
/// `cfg_select!` 宏也可以用于表达式位置,右侧可带花括号也可不带:
///
/// ```
/// #![feature(cfg_select)]
///
/// let _some_string = cfg_select! {
///     unix => "With great power comes great electricity bills",
///     _ => { "Behind every successful diet is an unwatched pizza" }
/// };
/// ```
#[unstable(feature = "cfg_select", issue = "115585")]
#[rustc_diagnostic_item = "cfg_select"]
#[rustc_builtin_macro]
pub macro cfg_select($($tt:tt)*) {
    /* compiler built-in */
}

/// 断言一个布尔表达式在运行时为 `true`。
///
/// 如果给定表达式在运行时无法求值为 `true`,则会调用 [`panic!`] 宏。
///
/// 与 [`assert!`] 类似,本宏也有第二种形式,可提供自定义 panic 消息。
///
/// # 用法
///
/// 不同于 [`assert!`],`debug_assert!` 语句默认只在非优化构建中启用。
/// 除非向编译器传入 `-C debug-assertions`,否则优化构建不会执行
/// `debug_assert!` 语句。这使得 `debug_assert!` 适合执行那些放进 release
/// 构建代价过高、但开发期间可能有帮助的检查。`debug_assert!` 展开后的结果
/// 始终会进行类型检查。
///
/// 未检查的断言会允许处于不一致状态的程序继续运行,这可能产生意外后果;
/// 但只要这种情况只发生在安全代码中,就不会引入不安全性。然而,断言的性能成本
/// 通常无法笼统衡量。因此,只有经过充分 profiling 后,才建议用 `debug_assert!`
/// 替换 [`assert!`];更重要的是,只应在安全代码中这样做!
///
/// # 示例
///
/// ```
/// // 这些断言的 panic 消息是给定表达式字符串化后的值。
/// debug_assert!(true);
///
/// fn some_expensive_computation() -> bool {
///     // 这里执行某些昂贵计算。
///     true
/// }
/// debug_assert!(some_expensive_computation());
///
/// // 使用自定义消息进行断言。
/// let x = true;
/// debug_assert!(x, "x wasn't true!");
///
/// let a = 3; let b = 27;
/// debug_assert!(a + b == 30, "a = {}, b = {}", a, b);
/// ```
#[macro_export]
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_diagnostic_item = "debug_assert_macro"]
#[allow_internal_unstable(edition_panic)]
macro_rules! debug_assert {
    ($($arg:tt)*) => {
        if $crate::cfg!(debug_assertions) {
            $crate::assert!($($arg)*);
        }
    };
}

/// 断言两个表达式彼此相等。
///
/// panic 时,本宏会用表达式的 debug 表示打印其值。
///
/// 不同于 [`assert_eq!`],`debug_assert_eq!` 语句默认只在非优化构建中启用。
/// 除非向编译器传入 `-C debug-assertions`,否则优化构建不会执行
/// `debug_assert_eq!` 语句。这使得 `debug_assert_eq!` 适合执行那些放进 release
/// 构建代价过高、但开发期间可能有帮助的检查。`debug_assert_eq!` 展开后的结果
/// 始终会进行类型检查。
///
/// # 示例
///
/// ```
/// let a = 3;
/// let b = 1 + 2;
/// debug_assert_eq!(a, b);
/// ```
#[macro_export]
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_diagnostic_item = "debug_assert_eq_macro"]
macro_rules! debug_assert_eq {
    ($($arg:tt)*) => {
        if $crate::cfg!(debug_assertions) {
            $crate::assert_eq!($($arg)*);
        }
    };
}

/// 断言两个表达式彼此不相等。
///
/// panic 时,本宏会用表达式的 debug 表示打印其值。
///
/// 不同于 [`assert_ne!`],`debug_assert_ne!` 语句默认只在非优化构建中启用。
/// 除非向编译器传入 `-C debug-assertions`,否则优化构建不会执行
/// `debug_assert_ne!` 语句。这使得 `debug_assert_ne!` 适合执行那些放进 release
/// 构建代价过高、但开发期间可能有帮助的检查。`debug_assert_ne!` 展开后的结果
/// 始终会进行类型检查。
///
/// # 示例
///
/// ```
/// let a = 3;
/// let b = 2;
/// debug_assert_ne!(a, b);
/// ```
#[macro_export]
#[stable(feature = "assert_ne", since = "1.13.0")]
#[rustc_diagnostic_item = "debug_assert_ne_macro"]
macro_rules! debug_assert_ne {
    ($($arg:tt)*) => {
        if $crate::cfg!(debug_assertions) {
            $crate::assert_ne!($($arg)*);
        }
    };
}

/// 断言一个表达式匹配给定模式。
///
/// 相比 `debug_assert!(matches!(value, pattern))`,通常更推荐使用本宏,因为它能够打印
/// 未满足期望的实际值形状的 debug 表示。相比之下,使用 [`debug_assert!`] 只会打印
/// 期望未满足,而不会说明原因。
///
/// 模式语法与 match arm 和 `matches!` 宏中的语法完全相同。可选的 if guard
/// 可用于添加额外检查;这些检查必须对匹配到的值为 true,否则本宏会 panic。
///
/// panic 时,本宏会用表达式的 debug 表示打印其值。
///
/// 与 [`assert!`] 类似,本宏还有第二种形式,可提供自定义 panic 消息。
///
/// 不同于 [`assert_matches!`],`debug_assert_matches!` 语句默认只在非优化构建中启用。
/// 除非向编译器传入 `-C debug-assertions`,否则优化构建不会执行
/// `debug_assert_matches!` 语句。这使得 `debug_assert_matches!` 适合执行那些放进
/// release 构建代价过高、但开发期间可能有帮助的检查。`debug_assert_matches!`
/// 展开后的结果始终会进行类型检查。
///
/// # 示例
///
/// ```
/// #![feature(assert_matches)]
///
/// use std::assert_matches::debug_assert_matches;
///
/// let a = Some(345);
/// let b = Some(56);
/// debug_assert_matches!(a, Some(_));
/// debug_assert_matches!(b, Some(_));
///
/// debug_assert_matches!(a, Some(345));
/// debug_assert_matches!(a, Some(345) | None);
///
/// // debug_assert_matches!(a, None); // 会 panic
/// // debug_assert_matches!(b, Some(345)); // 会 panic
/// // debug_assert_matches!(b, Some(345) | None); // 会 panic
///
/// debug_assert_matches!(a, Some(x) if x > 100);
/// // debug_assert_matches!(a, Some(x) if x < 100); // 会 panic
/// ```
#[unstable(feature = "assert_matches", issue = "82775")]
#[allow_internal_unstable(assert_matches)]
#[rustc_macro_transparency = "semiopaque"]
pub macro debug_assert_matches($($arg:tt)*) {
    if $crate::cfg!(debug_assertions) {
        $crate::assert_matches::assert_matches!($($arg)*);
    }
}

/// 返回给定表达式是否匹配给定模式。
///
/// 模式语法与 match arm 中的语法完全相同。可选的 if guard 可用于添加额外检查;
/// 这些检查必须对匹配到的值为 true,否则本宏会返回 `false`。
///
/// 测试某个值是否匹配某个模式时,通常更推荐使用 [`assert_matches!`],
/// 因为断言失败时它会打印该值的 debug 表示。
///
/// # 示例
///
/// ```
/// let foo = 'f';
/// assert!(matches!(foo, 'A'..='Z' | 'a'..='z'));
///
/// let bar = Some(4);
/// assert!(matches!(bar, Some(x) if x > 2));
/// ```
#[macro_export]
#[stable(feature = "matches_macro", since = "1.42.0")]
#[rustc_diagnostic_item = "matches_macro"]
#[allow_internal_unstable(non_exhaustive_omitted_patterns_lint, stmt_expr_attributes)]
macro_rules! matches {
    ($expression:expr, $pattern:pat $(if $guard:expr)? $(,)?) => {
        #[allow(non_exhaustive_omitted_patterns)]
        match $expression {
            $pattern $(if $guard)? => true,
            _ => false
        }
    };
}

/// 解包一个 result,或传播其错误。
///
/// [`?` 运算符][propagating-errors]被加入来取代 `try!`,
/// 应优先使用它。此外,`try` 在 Rust 2018 中是保留字,因此如果必须使用它,
/// 需要使用[原始标识符语法][ris]:`r#try`。
///
/// [propagating-errors]: https://doc.rust-lang.org/book/ch09-02-recoverable-errors-with-result.html#a-shortcut-for-propagating-errors-the--operator
/// [ris]: https://doc.rust-lang.org/nightly/rust-by-example/compatibility/raw_identifiers.html
///
/// `try!` 会匹配给定的 [`Result`]。如果是 `Ok` 变体,表达式的值就是被包裹的值。
///
/// 如果是 `Err` 变体,它会取出内部错误。随后 `try!` 使用 `From` 执行转换。
/// 这会在专门错误与更一般的错误之间提供自动转换。转换得到的错误会被立即返回。
///
/// 由于会提前返回,`try!` 只能用于返回 [`Result`] 的函数。
///
/// # 示例
///
/// ```
/// use std::io;
/// use std::fs::File;
/// use std::io::prelude::*;
///
/// enum MyError {
///     FileWriteError
/// }
///
/// impl From<io::Error> for MyError {
///     fn from(e: io::Error) -> MyError {
///         MyError::FileWriteError
///     }
/// }
///
/// // 快速返回错误的首选方式。
/// fn write_to_file_question() -> Result<(), MyError> {
///     let mut file = File::create("my_best_friends.txt")?;
///     file.write_all(b"This is a list of my best friends.")?;
///     Ok(())
/// }
///
/// // 过去快速返回错误的方式。
/// fn write_to_file_using_try() -> Result<(), MyError> {
///     let mut file = r#try!(File::create("my_best_friends.txt"));
///     r#try!(file.write_all(b"This is a list of my best friends."));
///     Ok(())
/// }
///
/// // 这等价于:
/// fn write_to_file_using_match() -> Result<(), MyError> {
///     let mut file = r#try!(File::create("my_best_friends.txt"));
///     match file.write_all(b"This is a list of my best friends.") {
///         Ok(v) => v,
///         Err(e) => return Err(From::from(e)),
///     }
///     Ok(())
/// }
/// ```
#[macro_export]
#[stable(feature = "rust1", since = "1.0.0")]
#[deprecated(since = "1.39.0", note = "use the `?` operator instead")]
#[doc(alias = "?")]
macro_rules! r#try {
    ($expr:expr $(,)?) => {
        match $expr {
            $crate::result::Result::Ok(val) => val,
            $crate::result::Result::Err(err) => {
                return $crate::result::Result::Err($crate::convert::From::from(err));
            }
        }
    };
}

/// 将格式化数据写入缓冲区。
///
/// 本宏接受一个“writer”、一个格式字符串以及一组参数。参数会按照指定格式字符串
/// 进行格式化,结果会传给 writer。writer 可以是任何带有 `write_fmt` 方法的值;
/// 通常该方法来自 [`fmt::Write`] 或 [`io::Write`] trait 的实现。本宏返回
/// `write_fmt` 方法的返回值;通常是 [`fmt::Result`] 或 [`io::Result`]。
///
/// 有关格式字符串语法的更多信息,请参见 [`std::fmt`]。
///
/// [`std::fmt`]: ../std/fmt/index.html
/// [`fmt::Write`]: crate::fmt::Write
/// [`io::Write`]: ../std/io/trait.Write.html
/// [`fmt::Result`]: crate::fmt::Result
/// [`io::Result`]: ../std/io/type.Result.html
///
/// # 示例
///
/// ```
/// use std::io::Write;
///
/// fn main() -> std::io::Result<()> {
///     let mut w = Vec::new();
///     write!(&mut w, "test")?;
///     write!(&mut w, "formatted {}", "arguments")?;
///
///     assert_eq!(w, b"testformatted arguments");
///     Ok(())
/// }
/// ```
///
/// 一个模块可以同时导入 `std::fmt::Write` 和 `std::io::Write`,并在实现其中任一
/// trait 的对象上调用 `write!`,因为对象通常不会同时实现这两个 trait。不过,
/// 模块必须避免 trait 名称冲突,例如把它们导入为 `_` 或以其他方式重命名:
///
/// ```
/// use std::fmt::Write as _;
/// use std::io::Write as _;
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let mut s = String::new();
///     let mut v = Vec::new();
///
///     write!(&mut s, "{} {}", "abc", 123)?; // 使用 fmt::Write::write_fmt
///     write!(&mut v, "s = {:?}", s)?; // 使用 io::Write::write_fmt
///     assert_eq!(v, b"s = \"abc 123\"");
///     Ok(())
/// }
/// ```
///
/// 如果还需要 trait 名称本身,例如要为你的类型实现其中一个或两个 trait,
/// 请导入包含它们的模块,然后使用带前缀的名称:
///
/// ```
/// # #![allow(unused_imports)]
/// use std::fmt::{self, Write as _};
/// use std::io::{self, Write as _};
///
/// struct Example;
///
/// impl fmt::Write for Example {
///     fn write_str(&mut self, _s: &str) -> core::fmt::Result {
///          unimplemented!();
///     }
/// }
/// ```
///
/// 注意:本宏同样可用于 `no_std` 环境。在 `no_std` 环境中,
/// 你需要负责相关组件的实现细节。
///
/// ```no_run
/// use core::fmt::Write;
///
/// struct Example;
///
/// impl Write for Example {
///     fn write_str(&mut self, _s: &str) -> core::fmt::Result {
///          unimplemented!();
///     }
/// }
///
/// let mut m = Example{};
/// write!(&mut m, "Hello World").expect("Not written");
/// ```
#[macro_export]
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_diagnostic_item = "write_macro"]
macro_rules! write {
    ($dst:expr, $($arg:tt)*) => {
        $dst.write_fmt($crate::format_args!($($arg)*))
    };
}

/// 将格式化数据写入缓冲区,并追加换行符。
///
/// 在所有平台上,换行符都只是 LINE FEED 字符(`\n`/`U+000A`),
/// 不会额外加入 CARRIAGE RETURN(`\r`/`U+000D`)。
///
/// 有关更多信息,请参见 [`write!`]。有关格式字符串语法的信息,请参见 [`std::fmt`]。
///
/// [`std::fmt`]: ../std/fmt/index.html
///
/// # 示例
///
/// ```
/// use std::io::{Write, Result};
///
/// fn main() -> Result<()> {
///     let mut w = Vec::new();
///     writeln!(&mut w)?;
///     writeln!(&mut w, "test")?;
///     writeln!(&mut w, "formatted {}", "arguments")?;
///
///     assert_eq!(&w[..], "\ntest\nformatted arguments\n".as_bytes());
///     Ok(())
/// }
/// ```
#[macro_export]
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_diagnostic_item = "writeln_macro"]
#[allow_internal_unstable(format_args_nl)]
macro_rules! writeln {
    ($dst:expr $(,)?) => {
        $crate::write!($dst, "\n")
    };
    ($dst:expr, $($arg:tt)*) => {
        $dst.write_fmt($crate::format_args_nl!($($arg)*))
    };
}

/// 标示不可达代码。
///
/// 当编译器无法判定某些代码不可达时,本宏都很有用。例如:
///
/// * 带有 guard 条件的 match arm。
/// * 动态终止的循环。
/// * 动态终止的迭代器。
///
/// 如果“代码不可达”的判定被证明是错误的,程序会立即以 [`panic!`] 终止。
///
/// 本宏的 unsafe 对应项是 [`unreachable_unchecked`] 函数;如果代码执行到那里,
/// 会导致未定义行为。
///
/// [`unreachable_unchecked`]: crate::hint::unreachable_unchecked
///
/// # Panics
///
/// 本宏总是会 [`panic!`],因为 `unreachable!` 只是带有固定特定消息的
/// `panic!` 简写。
///
/// 与 `panic!` 类似,本宏还有第二种形式,用于显示自定义值。
///
/// # 示例
///
/// Match 分支:
///
/// ```
/// # #[allow(dead_code)]
/// fn foo(x: Option<i32>) {
///     match x {
///         Some(n) if n >= 0 => println!("Some(Non-negative)"),
///         Some(n) if n <  0 => println!("Some(Negative)"),
///         Some(_)           => unreachable!(), // 如果注释掉这一行会导致编译错误
///         None              => println!("None")
///     }
/// }
/// ```
///
/// 迭代器:
///
/// ```
/// # #[allow(dead_code)]
/// fn divide_by_three(x: u32) -> u32 { // 最差的 x/3 实现之一
///     for i in 0.. {
///         if 3*i < i { panic!("u32 overflow"); }
///         if x < 3*i { return i-1; }
///     }
///     unreachable!("The loop should always return");
/// }
/// ```
#[macro_export]
#[rustc_builtin_macro(unreachable)]
#[allow_internal_unstable(edition_panic)]
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_diagnostic_item = "unreachable_macro"]
macro_rules! unreachable {
    // 根据调用者的 edition,展开为 `$crate::panic::unreachable_2015`
    // 或 `$crate::panic::unreachable_2021`。
    ($($arg:tt)*) => {
        /* compiler built-in */
    };
}

/// 通过以 "not implemented" 消息 panic 来标示未实现代码。
///
/// 这会让代码通过类型检查;在原型开发,或实现一个要求多个方法但你并不打算全部使用的
/// trait 时很有用。
///
/// `unimplemented!` 与 [`todo!`] 的区别在于:`todo!` 表达了稍后实现该功能的意图,
/// 消息为 "not yet implemented";而 `unimplemented!` 不做这样的声明,它的消息是
/// 消息为 "not implemented"。
///
/// 另外,某些 IDE 会标记 `todo!`。
///
/// # Panics
///
/// 本宏总是会 [`panic!`],因为 `unimplemented!` 只是带有固定特定消息的
/// `panic!` 简写。
///
/// 与 `panic!` 类似,本宏还有第二种形式,用于显示自定义值。
///
/// [`todo!`]: crate::todo
///
/// # 示例
///
/// 假设有一个 trait `Foo`:
///
/// ```
/// trait Foo {
///     fn bar(&self) -> u8;
///     fn baz(&self);
///     fn qux(&self) -> Result<u64, ()>;
/// }
/// ```
///
/// 我们想为 `MyStruct` 实现 `Foo`,但出于某种原因,只有实现 `bar()` 函数才有意义。
/// 在 `Foo` 的实现中仍然需要定义 `baz()` 和 `qux()`,但可以在它们的定义中使用
/// `unimplemented!`,让代码能够编译。
///
/// 如果执行到这些未实现方法,我们仍希望程序停止运行。
///
/// ```
/// # trait Foo {
/// #     fn bar(&self) -> u8;
/// #     fn baz(&self);
/// #     fn qux(&self) -> Result<u64, ()>;
/// # }
/// struct MyStruct;
///
/// impl Foo for MyStruct {
///     fn bar(&self) -> u8 {
///         1 + 1
///     }
///
///     fn baz(&self) {
///         // 对 `MyStruct` 执行 `baz` 没有意义,所以这里完全没有逻辑。
///         // 这会显示 "thread 'main' panicked at 'not implemented'"。
///         unimplemented!();
///     }
///
///     fn qux(&self) -> Result<u64, ()> {
///         // 这里有一些逻辑。
///         // 可以给 unimplemented! 添加消息来显示遗漏内容。
///         // 这会显示:
///         // 消息为 "thread 'main' panicked at 'not implemented: MyStruct isn't quxable'"。
///         unimplemented!("MyStruct isn't quxable");
///     }
/// }
///
/// fn main() {
///     let s = MyStruct;
///     s.bar();
/// }
/// ```
#[macro_export]
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_diagnostic_item = "unimplemented_macro"]
#[allow_internal_unstable(panic_internals)]
macro_rules! unimplemented {
    () => {
        $crate::panicking::panic("not implemented")
    };
    ($($arg:tt)+) => {
        $crate::panic!("not implemented: {}", $crate::format_args!($($arg)+))
    };
}

/// 标示尚未完成的代码。
///
/// 如果你正在原型开发,只想要一个占位符让代码通过类型分析,本宏会很有用。
///
/// [`unimplemented!`] 与 `todo!` 的区别在于:`todo!` 表达了稍后实现该功能的意图,
/// 消息为 "not yet implemented";而 `unimplemented!` 不做这样的声明,它的消息是
/// 消息为 "not implemented"。
///
/// 另外,某些 IDE 会标记 `todo!`。
///
/// # Panics
///
/// 本宏总是会 [`panic!`],因为 `todo!` 只是带有固定特定消息的 `panic!` 简写。
///
/// 与 `panic!` 类似,本宏还有第二种形式,用于显示自定义值。
///
/// # 示例
///
/// 下面是一个进行中代码的示例。我们有一个 trait `Foo`:
///
/// ```
/// trait Foo {
///     fn bar(&self) -> u8;
///     fn baz(&self);
///     fn qux(&self) -> Result<u64, ()>;
/// }
/// ```
///
/// 我们想在自己的某个类型上实现 `Foo`,但也想先只处理 `bar()`。
/// 为了让代码能够编译,需要实现 `baz()` 和 `qux()`,因此可以使用 `todo!`:
///
/// ```
/// # trait Foo {
/// #     fn bar(&self) -> u8;
/// #     fn baz(&self);
/// #     fn qux(&self) -> Result<u64, ()>;
/// # }
/// struct MyStruct;
///
/// impl Foo for MyStruct {
///     fn bar(&self) -> u8 {
///         1 + 1
///     }
///
///     fn baz(&self) {
///         // 先不用担心实现 baz()。
///         todo!();
///     }
///
///     fn qux(&self) -> Result<u64, ()> {
///         // 可以给 todo! 添加消息来显示遗漏内容。
///         // 这会显示:
///         // 消息为 "thread 'main' panicked at 'not yet implemented: MyStruct is not yet quxable'"。
///         todo!("MyStruct is not yet quxable");
///     }
/// }
///
/// fn main() {
///     let s = MyStruct;
///     s.bar();
///
///     // 我们甚至没有使用 baz() 或 qux(),所以这里没问题。
/// }
/// ```
#[macro_export]
#[stable(feature = "todo_macro", since = "1.40.0")]
#[rustc_diagnostic_item = "todo_macro"]
#[allow_internal_unstable(panic_internals)]
macro_rules! todo {
    () => {
        $crate::panicking::panic("not yet implemented")
    };
    ($($arg:tt)+) => {
        $crate::panic!("not yet implemented: {}", $crate::format_args!($($arg)+))
    };
}

/// 内建宏的定义。
///
/// 宏的大多数属性(稳定性、可见性等)都来自这里的源代码;例外是把宏输入转换为
/// 输出的展开函数,这些函数由编译器提供。
pub(crate) mod builtin {

    /// 遇到时用给定错误消息导致编译失败。
    ///
    /// 当 crate 使用条件编译策略并希望为错误条件提供更好的错误消息时,
    /// 应使用本宏。它是编译器级别的 [`panic!`] 形式,但会在*编译期*而不是
    /// *运行时*发出错误。
    ///
    /// # 示例
    ///
    /// 两类典型场景是宏和 `#[cfg]` 环境。
    ///
    /// 如果宏收到无效值,发出更好的编译器错误。没有最后一个分支时,
    /// 编译器仍会发出错误,但错误消息不会提到两个有效值。
    ///
    /// ```compile_fail
    /// macro_rules! give_me_foo_or_bar {
    ///     (foo) => {};
    ///     (bar) => {};
    ///     ($x:ident) => {
    ///         compile_error!("This macro only accepts `foo` or `bar`");
    ///     }
    /// }
    ///
    /// give_me_foo_or_bar!(neither);
    /// // ^ 会在编译期失败,消息为 "This macro only accepts `foo` or `bar`"
    /// ```
    ///
    /// 如果一组 feature 中没有任何一个可用,则发出编译器错误。
    ///
    /// ```compile_fail
    /// #[cfg(not(any(feature = "foo", feature = "bar")))]
    /// compile_error!("Either feature \"foo\" or \"bar\" must be enabled for this crate.");
    /// ```
    #[stable(feature = "compile_error_macro", since = "1.20.0")]
    #[rustc_builtin_macro]
    #[macro_export]
    macro_rules! compile_error {
        ($msg:expr $(,)?) => {{ /* compiler built-in */ }};
    }

    /// 为其他字符串格式化宏构造参数。
    ///
    /// 本宏接收一个格式化字符串字面量,其中为每个额外传入的参数包含 `{}`。
    /// `format_args!` 会准备额外参数,确保输出可被解释为字符串,并把参数规范化为
    /// 单一类型。任何实现 [`Display`] trait 的值都可以传给 `format_args!`;
    /// 任何 [`Debug`] 实现也都可以传给格式字符串中的 `{:?}`。
    ///
    /// 本宏会生成一个 [`fmt::Arguments`] 类型的值。该值可以传给 [`std::fmt`]
    /// 中的宏来执行有用的重定向。所有其他格式化宏([`format!`]、[`write!`]、
    /// [`println!`] 等)都通过它代理。不同于派生自它的宏,`format_args!`
    /// 会避免堆分配。
    ///
    /// 可以像下面这样在 `Debug` 和 `Display` 上下文中使用 `format_args!`
    /// 返回的 [`fmt::Arguments`] 值。示例还展示了 `Debug` 和 `Display`
    /// 会格式化为同一个东西:`format_args!` 中插值后的格式字符串。
    ///
    /// ```rust
    /// let args = format_args!("{} foo {:?}", 1, 2);
    /// let debug = format!("{args:?}");
    /// let display = format!("{args}");
    /// assert_eq!("1 foo 2", display);
    /// assert_eq!(display, debug);
    /// ```
    ///
    /// 有关宏参数语法的细节和更多信息,请参见
    /// [`std::fmt` 中的格式化文档](../std/fmt/index.html)。
    ///
    /// [`Display`]: crate::fmt::Display
    /// [`Debug`]: crate::fmt::Debug
    /// [`fmt::Arguments`]: crate::fmt::Arguments
    /// [`std::fmt`]: ../std/fmt/index.html
    /// [`format!`]: ../std/macro.format.html
    /// [`println!`]: ../std/macro.println.html
    ///
    /// # 示例
    ///
    /// ```
    /// use std::fmt;
    ///
    /// let s = fmt::format(format_args!("hello {}", "world"));
    /// assert_eq!(s, format!("hello {}", "world"));
    /// ```
    ///
    /// # 参数生命周期
    ///
    /// 除了没有使用格式化参数的情况外,生成的 `fmt::Arguments` 值会借用临时值。
    /// 为了允许将其存储起来供稍后使用,当 `format_args!` 出现在 `let` 语句的
    /// 初始化表达式中时,参数的生命周期以及它们所借用临时值的生命周期可能会被
    /// [延长][extended]。用于判定何时延长临时值生命周期的语法规则记录在
    /// [Reference] 中。
    ///
    /// [extended]: ../reference/destructors.html#temporary-lifetime-extension
    /// [Reference]: ../reference/destructors.html#extending-based-on-expressions
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_diagnostic_item = "format_args_macro"]
    #[allow_internal_unsafe]
    #[allow_internal_unstable(fmt_internals, fmt_arguments_from_str)]
    #[rustc_builtin_macro]
    #[macro_export]
    macro_rules! format_args {
        ($fmt:expr) => {{ /* compiler built-in */ }};
        ($fmt:expr, $($args:tt)*) => {{ /* compiler built-in */ }};
    }

    /// 与 [`format_args`] 相同,但可用于某些 const 上下文。
    ///
    /// panic 宏会为 `const_panic` feature 使用本宏。
    ///
    /// 一旦 `format_args` 被允许用于 const 上下文,本宏就会被移除。
    #[unstable(feature = "const_format_args", issue = "none")]
    #[allow_internal_unstable(fmt_internals, fmt_arguments_from_str)]
    #[rustc_builtin_macro]
    #[macro_export]
    macro_rules! const_format_args {
        ($fmt:expr) => {{ /* compiler built-in */ }};
        ($fmt:expr, $($args:tt)*) => {{ /* compiler built-in */ }};
    }

    /// 与 [`format_args`] 相同,但会在末尾添加换行符。
    #[unstable(
        feature = "format_args_nl",
        issue = "none",
        reason = "`format_args_nl` is only for internal \
                  language use and is subject to change"
    )]
    #[allow_internal_unstable(fmt_internals, fmt_arguments_from_str)]
    #[rustc_builtin_macro]
    #[doc(hidden)]
    #[macro_export]
    macro_rules! format_args_nl {
        ($fmt:expr) => {{ /* compiler built-in */ }};
        ($fmt:expr, $($args:tt)*) => {{ /* compiler built-in */ }};
    }

    /// 在编译期检查环境变量。
    ///
    /// 本宏会在编译期展开为指定环境变量的值,生成类型为 `&'static str` 的表达式。
    /// 如果想在运行时读取该值,请改用 [`std::env::var`]。
    ///
    /// [`std::env::var`]: ../std/env/fn.var.html
    ///
    /// 如果环境变量未定义,则会发出编译错误。若不想发出编译错误,请改用
    /// [`option_env!`] 宏。如果环境变量不是有效的 Unicode 字符串,
    /// 也会发出编译错误。
    ///
    /// # 示例
    ///
    /// ```
    /// let path: &'static str = env!("PATH");
    /// println!("the $PATH variable at the time of compiling was: {path}");
    /// ```
    ///
    /// 可以通过传入字符串作为第二个参数来自定义错误消息:
    ///
    /// ```compile_fail
    /// let doc: &'static str = env!("documentation", "what's that?!");
    /// ```
    ///
    /// 如果未定义 `documentation` 环境变量,会得到如下错误:
    ///
    /// ```text
    /// error: what's that?!
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_builtin_macro]
    #[macro_export]
    #[rustc_diagnostic_item = "env_macro"] // 对外部 lint 有用
    macro_rules! env {
        ($name:expr $(,)?) => {{ /* compiler built-in */ }};
        ($name:expr, $error_msg:expr $(,)?) => {{ /* compiler built-in */ }};
    }

    /// 在编译期可选地检查环境变量。
    ///
    /// 如果指定环境变量在编译期存在,本宏会展开为类型 `Option<&'static str>` 的表达式,
    /// 其值为环境变量值对应的 `Some`(如果环境变量不是有效的 Unicode 字符串,
    /// 会发出编译错误)。如果环境变量不存在,则展开为 `None`。有关该类型的更多信息,
    /// 请参见 [`Option<T>`][Option]。如果想在运行时读取该值,请改用 [`std::env::var`]。
    ///
    /// [`std::env::var`]: ../std/env/fn.var.html
    ///
    /// 使用本宏时,只有环境变量存在但不是有效 Unicode 字符串才会发出编译期错误。
    /// 如果还希望在环境变量不存在时发出编译错误,请改用 [`env!`] 宏。
    ///
    /// # 示例
    ///
    /// ```
    /// let key: Option<&'static str> = option_env!("SECRET_KEY");
    /// println!("the secret key might be: {key:?}");
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_builtin_macro]
    #[macro_export]
    #[rustc_diagnostic_item = "option_env_macro"] // 对外部 lint 有用
    macro_rules! option_env {
        ($name:expr $(,)?) => {{ /* compiler built-in */ }};
    }

    /// 将字面量拼接为字节切片。
    ///
    /// 本宏接收任意数量用逗号分隔的字面量,并把它们拼接成一个整体,生成类型为
    /// `&[u8; _]` 的表达式,表示所有字面量从左到右拼接后的内容。传入的字面量
    /// 可以是以下任意组合:
    ///
    /// - 字节字面量(`b'r'`)
    /// - 字节字符串(`b"Rust"`)
    /// - 字节/数字数组(`[b'A', 66, b'C']`)
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(concat_bytes)]
    ///
    /// # fn main() {
    /// let s: &[u8; 6] = concat_bytes!(b'A', b"BC", [68, b'E', 70]);
    /// assert_eq!(s, b"ABCDEF");
    /// # }
    /// ```
    #[unstable(feature = "concat_bytes", issue = "87555")]
    #[rustc_builtin_macro]
    #[macro_export]
    macro_rules! concat_bytes {
        ($($e:literal),+ $(,)?) => {{ /* compiler built-in */ }};
    }

    /// 将字面量拼接为静态字符串切片。
    ///
    /// 本宏接收任意数量用逗号分隔的字面量,生成类型为 `&'static str` 的表达式,
    /// 表示所有字面量从左到右拼接后的内容。
    ///
    /// 整数字面量和浮点字面量会先被[字符串化](core::stringify),再参与拼接。
    ///
    /// # 示例
    ///
    /// ```
    /// let s = concat!("test", 10, 'b', true);
    /// assert_eq!(s, "test10btrue");
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_builtin_macro]
    #[rustc_diagnostic_item = "macro_concat"]
    #[macro_export]
    macro_rules! concat {
        ($($e:expr),* $(,)?) => {{ /* compiler built-in */ }};
    }

    /// 展开为调用位置所在的行号。
    ///
    /// 与 [`column!`] 和 [`file!`] 一起,这些宏为开发者提供源码位置的调试信息。
    ///
    /// 展开的表达式类型为 `u32`,并且从 1 开始计数,因此每个文件的第一行求值为 1,
    /// 第二行为 2,依此类推。这与常见编译器或流行编辑器的错误消息一致。
    /// 返回的行号*不一定*是 `line!` 调用本身所在的行,而是通向 `line!` 宏调用的
    /// 第一个宏调用所在的行。
    ///
    /// # 示例
    ///
    /// ```
    /// let current_line = line!();
    /// println!("defined on line: {current_line}");
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_builtin_macro]
    #[macro_export]
    macro_rules! line {
        () => {
            /* compiler built-in */
        };
    }

    /// 展开为调用位置所在的列号。
    ///
    /// 与 [`line!`] 和 [`file!`] 一起,这些宏为开发者提供源码位置的调试信息。
    ///
    /// 展开的表达式类型为 `u32`,并且从 1 开始计数,因此每行的第一列求值为 1,
    /// 第二列为 2,依此类推。这与常见编译器或流行编辑器的错误消息一致。
    /// 返回的列号*不一定*是 `column!` 调用本身所在的列,而是通向 `column!` 宏调用的
    /// 第一个宏调用所在的列。
    ///
    /// # 示例
    ///
    /// ```
    /// let current_col = column!();
    /// println!("defined on column: {current_col}");
    /// ```
    ///
    /// `column!` 计数 Unicode 码点,而不是字节或字素簇。因此,前两次调用返回相同值,
    /// 第三次调用则不同。
    ///
    /// ```
    /// let a = ("foobar", column!()).1;
    /// let b = ("人之初性本善", column!()).1;
    /// let c = ("f̅o̅o̅b̅a̅r̅", column!()).1; // 使用组合上划线(U+0305)
    ///
    /// assert_eq!(a, b);
    /// assert_ne!(b, c);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_builtin_macro]
    #[macro_export]
    macro_rules! column {
        () => {
            /* compiler built-in */
        };
    }

    /// 展开为调用位置所在的文件名。
    ///
    /// 与 [`line!`] 和 [`column!`] 一起,这些宏为开发者提供源码位置的调试信息。
    ///
    /// 展开的表达式类型为 `&'static str`,返回的文件不是 `file!` 宏调用本身所在的
    /// 文件,而是通向 `file!` 宏调用的第一个宏调用所在的文件。
    ///
    /// 文件名来自传给 Rust 编译器的 crate root 源路径,以及编译器从 crate root
    /// 走到包含 `file!` 的模块时采用的路径序列;传给 Rust 编译器的标志
    /// (例如 `--remap-path-prefix`)也可能修改它。如果 crate 的源路径是相对路径,
    /// 初始基目录会是 Rust 编译器的工作目录。例如,如果传给编译器的源路径是
    /// `./src/lib.rs`,其中有一个 `mod foo;`,其源路径是 `src/foo/mod.rs`,
    /// 那么在 `mod foo;` 内调用 `file!` 将返回 `./src/foo/mod.rs`。
    ///
    /// 未来的编译器选项可能进一步改变 `file!` 的行为,包括可能让它完全为空。
    /// 依赖 `file!` 生成可打开文件路径的代码(例如测试库)会与这类选项不兼容,
    /// 可能需要建议不要使用这些选项。
    ///
    /// # 示例
    ///
    /// ```
    /// let this_file = file!();
    /// println!("defined in file: {this_file}");
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_builtin_macro]
    #[macro_export]
    macro_rules! file {
        () => {
            /* compiler built-in */
        };
    }

    /// 将参数字符串化。
    ///
    /// 本宏会生成类型为 `&'static str` 的表达式,内容是传给宏的所有 token
    /// 字符串化后的结果。宏调用本身的语法不受限制。
    ///
    /// 注意,输入 token 的展开结果未来可能发生变化。如果依赖输出,应当谨慎。
    ///
    /// # 示例
    ///
    /// ```
    /// let one_plus_one = stringify!(1 + 1);
    /// assert_eq!(one_plus_one, "1 + 1");
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_builtin_macro]
    #[macro_export]
    macro_rules! stringify {
        ($($t:tt)*) => {
            /* compiler built-in */
        };
    }

    /// 以字符串形式包含一个 UTF-8 编码文件。
    ///
    /// 文件位置相对于当前文件(类似模块查找方式)。给定路径会在编译期以平台特定方式解释。
    /// 因此,例如带有反斜杠 `\` 的 Windows 路径调用在 Unix 上无法正确编译。
    ///
    /// 本宏会生成类型为 `&'static str` 的表达式,内容是该文件的内容。
    ///
    /// # 示例
    ///
    /// 假设同一目录中有两个文件,内容如下:
    ///
    /// 文件 'spanish.in':
    ///
    /// ```text
    /// adiós
    /// ```
    ///
    /// 文件 'main.rs':
    ///
    /// ```ignore (cannot-doctest-external-file-dependency)
    /// fn main() {
    ///     let my_str = include_str!("spanish.in");
    ///     assert_eq!(my_str, "adiós\n");
    ///     print!("{my_str}");
    /// }
    /// ```
    ///
    /// 编译 'main.rs' 并运行所得二进制文件会打印 "adiós"。
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_builtin_macro]
    #[macro_export]
    #[rustc_diagnostic_item = "include_str_macro"]
    macro_rules! include_str {
        ($file:expr $(,)?) => {{ /* compiler built-in */ }};
    }

    /// 以字节数组引用的形式包含一个文件。
    ///
    /// 文件位置相对于当前文件(类似模块查找方式)。给定路径会在编译期以平台特定方式解释。
    /// 因此,例如带有反斜杠 `\` 的 Windows 路径调用在 Unix 上无法正确编译。
    ///
    /// 本宏会生成类型为 `&'static [u8; N]` 的表达式,内容是该文件的内容。
    ///
    /// # 示例
    ///
    /// 假设同一目录中有两个文件,内容如下:
    ///
    /// 文件 'spanish.in':
    ///
    /// ```text
    /// adiós
    /// ```
    ///
    /// 文件 'main.rs':
    ///
    /// ```ignore (cannot-doctest-external-file-dependency)
    /// fn main() {
    ///     let bytes = include_bytes!("spanish.in");
    ///     assert_eq!(bytes, b"adi\xc3\xb3s\n");
    ///     print!("{}", String::from_utf8_lossy(bytes));
    /// }
    /// ```
    ///
    /// 编译 'main.rs' 并运行所得二进制文件会打印 "adiós"。
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_builtin_macro]
    #[macro_export]
    #[rustc_diagnostic_item = "include_bytes_macro"]
    macro_rules! include_bytes {
        ($file:expr $(,)?) => {{ /* compiler built-in */ }};
    }

    /// 展开为表示当前模块路径的字符串。
    ///
    /// 当前模块路径可以理解为一路回到 crate root 的模块层级。返回路径的第一个
    /// 组成部分是当前正在编译的 crate 名称。
    ///
    /// # 示例
    ///
    /// ```
    /// mod test {
    ///     pub fn foo() {
    ///         assert!(module_path!().ends_with("test"));
    ///     }
    /// }
    ///
    /// test::foo();
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_builtin_macro]
    #[macro_export]
    macro_rules! module_path {
        () => {
            /* compiler built-in */
        };
    }

    /// 在编译期求值配置标志的布尔组合。
    ///
    /// 除 `#[cfg]` 属性外,本宏提供配置标志的布尔表达式求值能力。这通常能减少重复代码。
    ///
    /// 传给本宏的语法与 [`cfg`] 属性的语法相同。
    ///
    /// 不同于 `#[cfg]`,`cfg!` 不会移除任何代码,只会求值为 true 或 false。
    /// 例如,当 `cfg!` 用作条件时,if/else 表达式中的所有块都必须有效,
    /// 不论 `cfg!` 求值结果是什么。
    ///
    /// [`cfg`]: ../reference/conditional-compilation.html#the-cfg-attribute
    ///
    /// # 示例
    ///
    /// ```
    /// let my_directory = if cfg!(windows) {
    ///     "windows-specific-directory"
    /// } else {
    ///     "unix-directory"
    /// };
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_builtin_macro]
    #[macro_export]
    macro_rules! cfg {
        ($($cfg:tt)*) => {
            /* compiler built-in */
        };
    }

    /// 根据上下文把一个文件解析为表达式或条目。
    ///
    /// **警告**:对于多文件 Rust 项目,`include!` 宏很可能不是你要找的东西。
    /// 通常,多文件 Rust 项目使用
    /// [模块](https://doc.rust-lang.org/reference/items/modules.html)。多文件项目和模块在
    /// Rust-by-Example 一书的[这里](https://doc.rust-lang.org/rust-by-example/mod/split.html)
    /// 有解释,模块系统也在 Rust Book 的[这里](https://doc.rust-lang.org/book/ch07-02-defining-modules-to-control-scope-and-privacy.html)
    /// 有解释。
    ///
    /// 被包含的文件会以[非卫生](https://doc.rust-lang.org/reference/macros-by-example.html#hygiene)
    /// 的方式放入周围代码中。如果被包含文件被解析为表达式,并且两个文件之间共享变量
    /// 或函数名,可能导致变量或函数与被包含文件期望的不同。
    ///
    /// 被包含文件的位置相对于当前文件(类似模块查找方式)。给定路径会在编译期以
    /// 平台特定方式解释。因此,例如带有反斜杠 `\` 的 Windows 路径调用在 Unix 上
    /// 无法正确编译。
    ///
    /// # 用法
    ///
    /// `include!` 宏主要有两个用途。它用于包含写在单独文件中的文档,也用于包含
    /// [通常由 `build.rs` 脚本生成的构建产物](https://doc.rust-lang.org/cargo/reference/build-scripts.html#outputs-of-the-build-script)。
    ///
    /// 使用 `include` 宏包含文档片段时,请记住被包含文件仍需是有效 Rust 语法。
    /// 也可以在模块级使用 [`include_str`] 宏,例如 `#![doc = include_str!("...")]`,
    /// 或在条目级使用 `#[doc = include_str!("...")]`,从纯文本或 markdown 文件中包含文档。
    ///
    /// # 示例
    ///
    /// 假设同一目录中有两个文件,内容如下:
    ///
    /// 文件 'monkeys.in':
    ///
    /// ```ignore (only-for-syntax-highlight)
    /// ['🙈', '🙊', '🙉']
    ///     .iter()
    ///     .cycle()
    ///     .take(6)
    ///     .collect::<String>()
    /// ```
    ///
    /// 文件 'main.rs':
    ///
    /// ```ignore (cannot-doctest-external-file-dependency)
    /// fn main() {
    ///     let my_string = include!("monkeys.in");
    ///     assert_eq!("🙈🙊🙉🙈🙊🙉", my_string);
    ///     println!("{my_string}");
    /// }
    /// ```
    ///
    /// 编译 'main.rs' 并运行所得二进制文件会打印
    /// "🙈🙊🙉🙈🙊🙉".
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_builtin_macro]
    #[macro_export]
    #[rustc_diagnostic_item = "include_macro"] // 对外部 lint 有用
    macro_rules! include {
        ($file:expr $(,)?) => {{ /* compiler built-in */ }};
    }

    /// 本宏使用前向模式自动微分生成一个新函数。
    /// 它只能应用于函数。新函数会计算被应用该宏的函数的导数。
    ///
    /// 期望的使用语法是:
    /// `#[autodiff_forward(NAME, INPUT_ACTIVITIES, OUTPUT_ACTIVITY)]`
    ///
    /// - `NAME`: 表示有效函数名的字符串。
    /// - `INPUT_ACTIVITIES`: 为每个输入参数指定一个有效 activity。
    /// - `OUTPUT_ACTIVITY`: 如果函数隐式不返回任何内容(或显式返回 `-> ()`),
    ///   则不得设置。否则,它必须被设置为允许的 activity 之一。
    ///
    /// ACTIVITIES 可以是 `Dual` 或 `Const`,之后会暴露更多选项。
    ///
    /// `Const` 应用于非浮点参数;如果不关心相对于某个浮点参数的导数,
    /// 也可以把它作为优化用于基于浮点的参数。
    ///
    /// `Dual` 可用于浮点标量值,也可用于引用、原始指针或其他间接输入参数。
    /// 它也可用于浮点标量返回值。如果用于返回值,生成的函数会返回两个浮点标量组成的元组。
    /// 如果用于输入参数,会创建一个同类型的新 shadow 参数,紧跟在原始参数之后。
    ///
    /// ### 用法示例:
    ///
    /// ```rust,ignore (autodiff requires a -Z flag as well as fat-lto for testing)
    /// #![feature(autodiff)]
    /// use std::autodiff::*;
    /// #[autodiff_forward(rb_fwd1, Dual, Const, Dual)]
    /// #[autodiff_forward(rb_fwd2, Const, Dual, Dual)]
    /// #[autodiff_forward(rb_fwd3, Dual, Dual, Dual)]
    /// fn rosenbrock(x: f64, y: f64) -> f64 {
    ///     (1.0 - x).powi(2) + 100.0 * (y - x.powi(2)).powi(2)
    /// }
    /// #[autodiff_forward(rb_inp_fwd, Dual, Dual, Dual)]
    /// fn rosenbrock_inp(x: f64, y: f64, out: &mut f64) {
    ///     *out = (1.0 - x).powi(2) + 100.0 * (y - x.powi(2)).powi(2);
    /// }
    ///
    /// fn main() {
    ///   let x0 = rosenbrock(1.0, 3.0); // 400.0
    ///   let (x1, dx1) = rb_fwd1(1.0, 1.0, 3.0); // (400.0, -800.0)
    ///   let (x2, dy1) = rb_fwd2(1.0, 3.0, 1.0); // (400.0, 400.0)
    ///   // 同时为两个参数播种时,切向返回值是两者之和。
    ///   let (x3, dxy) = rb_fwd3(1.0, 1.0, 3.0, 1.0); // (400.0, -400.0)
    ///
    ///   let mut out = 0.0;
    ///   let mut dout = 0.0;
    ///   rb_inp_fwd(1.0, 1.0, 3.0, 1.0, &mut out, &mut dout);
    ///   // (out, dout) == (400.0, -400.0)
    /// }
    /// ```
    ///
    /// 我们可能想跟踪一个输入浮点数如何影响一个或多个输出浮点数。在这种情况下,
    /// 一个输入的 shadow 应初始化为 `1.0`,其他输入的 shadow 应初始化为 `0.0`。
    /// 输出的 shadow 应初始化为 `0.0`。调用生成的函数后,输入的 shadow 会被清零,
    /// 输出的 shadow 会包含导数。当前标记为 `Dual` 的输出浮点数多于输入浮点数时,
    /// 前向模式通常更高效。相关信息也可在术语 "Vector-Jacobian product" (VJP) 下找到。
    #[unstable(feature = "autodiff", issue = "124509")]
    #[allow_internal_unstable(rustc_attrs)]
    #[allow_internal_unstable(core_intrinsics)]
    #[rustc_builtin_macro]
    pub macro autodiff_forward($item:item) {
        /* compiler built-in */
    }

    /// 本宏使用反向模式自动微分生成一个新函数。
    /// 它只能应用于函数。新函数会计算被应用该宏的函数的导数。
    ///
    /// 期望的使用语法是:
    /// `#[autodiff_reverse(NAME, INPUT_ACTIVITIES, OUTPUT_ACTIVITY)]`
    ///
    /// - `NAME`: 表示有效函数名的字符串。
    /// - `INPUT_ACTIVITIES`: 为每个输入参数指定一个有效 activity。
    /// - `OUTPUT_ACTIVITY`: 如果函数隐式不返回任何内容(或显式返回 `-> ()`),
    ///   则不得设置。否则,它必须被设置为允许的 activity 之一。
    ///
    /// ACTIVITIES 可以是 `Active`、`Duplicated` 或 `Const`,之后会暴露更多选项。
    ///
    /// `Active` 可用于浮点标量值。如果用于输入,会在生成函数的返回元组中追加一个
    /// 新浮点数。如果函数返回浮点标量,`Active` 也可用于返回值。在这种情况下,
    /// 会向参数列表追加一个浮点标量,它充当 seed。
    ///
    /// `Duplicated` 可用于引用、原始指针或其他间接输入参数。它会创建一个同类型的新
    /// shadow 参数,跟在原始参数之后。const 引用或指针参数会接收可变引用或指针作为 shadow。
    ///
    /// `Const` 应用于非浮点参数;如果不关心相对于某个浮点参数的导数,
    /// 也可以把它作为优化用于基于浮点的参数。
    ///
    /// ### 用法示例:
    ///
    /// ```rust,ignore (autodiff requires a -Z flag as well as fat-lto for testing)
    /// #![feature(autodiff)]
    /// use std::autodiff::*;
    /// #[autodiff_reverse(rb_rev, Active, Active, Active)]
    /// fn rosenbrock(x: f64, y: f64) -> f64 {
    ///     (1.0 - x).powi(2) + 100.0 * (y - x.powi(2)).powi(2)
    /// }
    /// #[autodiff_reverse(rb_inp_rev, Active, Active, Duplicated)]
    /// fn rosenbrock_inp(x: f64, y: f64, out: &mut f64) {
    ///     *out = (1.0 - x).powi(2) + 100.0 * (y - x.powi(2)).powi(2);
    /// }
    ///
    /// fn main() {
    ///     let (output1, dx1, dy1) = rb_rev(1.0, 3.0, 1.0);
    ///     dbg!(output1, dx1, dy1); // (400.0, -800.0, 400.0)
    ///     let mut output2 = 0.0;
    ///     let mut seed = 1.0;
    ///     let (dx2, dy2) = rb_inp_rev(1.0, 3.0, &mut output2, &mut seed);
    ///     // 结果满足 (dx2, dy2, output2, seed) == (-800.0, 400.0, 400.0, 0.0)
    /// }
    /// ```
    ///
    ///
    /// 我们经常想跟踪一个或多个输入浮点数如何影响一个输出浮点数。这个输出可以是
    /// 标量返回值,也可以是可变引用或指针参数。在后一种情况下,可变输入应标记为
    /// duplicated,且其 shadow 初始化为 `0.0`。输出的 shadow 应标记为 active 或
    /// duplicated,并初始化为 `1.0`。调用生成的函数后,输入的 shadow 会包含导数。
    /// 输出的 shadow("seed") 会被重置为零。如果函数有多个标记为 active 或 duplicated
    /// 的输出浮点数,用户可能需要把其中一个设为 `1.0`,其他设为 `0.0` 来计算偏导数。
    /// 不同于前向模式,调用生成函数不会重置输入的 shadow。active/duplicated 输入多于
    /// 输出浮点数时,反向模式通常更高效。
    ///
    /// 相关信息也可在术语 "Jacobian-Vector Product" (JVP) 下找到。
    #[unstable(feature = "autodiff", issue = "124509")]
    #[allow_internal_unstable(rustc_attrs)]
    #[allow_internal_unstable(core_intrinsics)]
    #[rustc_builtin_macro]
    pub macro autodiff_reverse($item:item) {
        /* compiler built-in */
    }

    /// 断言一个布尔表达式在运行时为 `true`。
    ///
    /// 如果给定表达式在运行时无法求值为 `true`,则会调用 [`panic!`] 宏。
    ///
    /// # 用法
    ///
    /// 断言在 debug 和 release 构建中都会始终检查,且无法禁用。若需要在 release
    /// 构建中默认不启用的断言,请参见 [`debug_assert!`]。
    ///
    /// unsafe 代码可能依赖 `assert!` 强制执行运行时不变量;若这些不变量被违反,
    /// 可能导致不安全性。
    ///
    /// `assert!` 的其他用例包括在安全代码中测试和强制执行运行时不变量
    /// (这些不变量被违反不会导致不安全性)。
    ///
    /// # 自定义消息
    ///
    /// 本宏还有第二种形式,可以提供自定义 panic 消息,并可带或不带格式化参数。
    /// 这种形式的语法见 [`std::fmt`]。作为格式参数使用的表达式只有在断言失败时
    /// 才会求值。
    ///
    /// [`std::fmt`]: ../std/fmt/index.html
    ///
    /// # 示例
    ///
    /// ```
    /// // 这些断言的 panic 消息是给定表达式字符串化后的值。
    /// assert!(true);
    ///
    /// fn some_computation() -> bool {
    ///     // 这里执行某些昂贵计算。
    ///     true
    /// }
    ///
    /// assert!(some_computation());
    ///
    /// // 使用自定义消息进行断言。
    /// let x = true;
    /// assert!(x, "x wasn't true!");
    ///
    /// let a = 3; let b = 27;
    /// assert!(a + b == 30, "a = {}, b = {}", a, b);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_builtin_macro]
    #[macro_export]
    #[rustc_diagnostic_item = "assert_macro"]
    #[allow_internal_unstable(
        core_intrinsics,
        panic_internals,
        edition_panic,
        generic_assert_internals
    )]
    macro_rules! assert {
        ($cond:expr $(,)?) => {{ /* compiler built-in */ }};
        ($cond:expr, $($arg:tt)+) => {{ /* compiler built-in */ }};
    }

    /// 将传入的 token 打印到标准输出。
    #[unstable(
        feature = "log_syntax",
        issue = "29598",
        reason = "`log_syntax!` is not stable enough for use and is subject to change"
    )]
    #[rustc_builtin_macro]
    #[macro_export]
    macro_rules! log_syntax {
        ($($arg:tt)*) => {
            /* compiler built-in */
        };
    }

    /// 启用或禁用用于调试其他宏的跟踪功能。
    #[unstable(
        feature = "trace_macros",
        issue = "29598",
        reason = "`trace_macros` is not stable enough for use and is subject to change"
    )]
    #[rustc_builtin_macro]
    #[macro_export]
    macro_rules! trace_macros {
        (true) => {{ /* compiler built-in */ }};
        (false) => {{ /* compiler built-in */ }};
    }

    /// 用于应用 derive 宏的属性宏。
    ///
    /// 更多信息见 [the reference]。
    ///
    /// [the reference]: ../../../reference/attributes/derive.html
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_builtin_macro]
    pub macro derive($item:item) {
        /* compiler built-in */
    }

    /// 用于在 const 上下文中应用 derive 宏来实现 trait 的属性宏。
    ///
    /// 更多信息见 [the reference]。
    ///
    /// [the reference]: ../../../reference/attributes/derive.html
    #[unstable(feature = "derive_const", issue = "118304")]
    #[rustc_builtin_macro]
    pub macro derive_const($item:item) {
        /* compiler built-in */
    }

    /// 应用于函数的属性宏,用于把该函数变成单元测试。
    ///
    /// 更多信息见 [the reference]。
    ///
    /// [the reference]: ../../../reference/attributes/testing.html#the-test-attribute
    #[stable(feature = "rust1", since = "1.0.0")]
    #[allow_internal_unstable(test, rustc_attrs, coverage_attribute)]
    #[rustc_builtin_macro]
    pub macro test($item:item) {
        /* compiler built-in */
    }

    /// 应用于函数的属性宏,用于把该函数变成基准测试。
    #[unstable(
        feature = "test",
        issue = "50297",
        reason = "`bench` is a part of custom test frameworks which are unstable"
    )]
    #[allow_internal_unstable(test, rustc_attrs, coverage_attribute)]
    #[rustc_builtin_macro]
    pub macro bench($item:item) {
        /* compiler built-in */
    }

    /// `#[test]` 和 `#[bench]` 宏的实现细节。
    #[unstable(
        feature = "custom_test_frameworks",
        issue = "50297",
        reason = "custom test frameworks are an unstable feature"
    )]
    #[allow_internal_unstable(test, rustc_attrs)]
    #[rustc_builtin_macro]
    pub macro test_case($item:item) {
        /* compiler built-in */
    }

    /// 应用于 static 的属性宏,用于把它注册为全局分配器。
    ///
    /// 另请参见 [`std::alloc::GlobalAlloc`](../../../std/alloc/trait.GlobalAlloc.html)。
    #[stable(feature = "global_allocator", since = "1.28.0")]
    #[allow_internal_unstable(rustc_attrs)]
    #[rustc_builtin_macro]
    pub macro global_allocator($item:item) {
        /* compiler built-in */
    }

    /// 应用于函数的属性宏,用于给函数添加后置条件。
    ///
    /// 该属性携带一个参数 token-tree,最终会被解析为一元闭包表达式,
    /// 并在返回值引用上调用。
    #[unstable(feature = "contracts", issue = "128044")]
    #[allow_internal_unstable(contracts_internals)]
    #[rustc_builtin_macro]
    pub macro contracts_ensures($item:item) {
        /* compiler built-in */
    }

    /// 应用于函数的属性宏,用于给函数添加前置条件。
    ///
    /// 该属性携带一个参数 token-tree,最终会被解析为一个布尔表达式,
    /// 该表达式可以访问函数的形式参数。
    #[unstable(feature = "contracts", issue = "128044")]
    #[allow_internal_unstable(contracts_internals)]
    #[rustc_builtin_macro]
    pub macro contracts_requires($item:item) {
        /* compiler built-in */
    }

    /// 应用于函数的属性宏,用于把它注册为分配失败处理器。
    ///
    /// 另请参见 [`std::alloc::handle_alloc_error`](../../../std/alloc/fn.handle_alloc_error.html)。
    #[unstable(feature = "alloc_error_handler", issue = "51540")]
    #[allow_internal_unstable(rustc_attrs)]
    #[rustc_builtin_macro]
    pub macro alloc_error_handler($item:item) {
        /* compiler built-in */
    }

    /// 如果传入路径可访问,保留被应用的条目;否则将其移除。
    #[unstable(
        feature = "cfg_accessible",
        issue = "64797",
        reason = "`cfg_accessible` is not fully implemented"
    )]
    #[rustc_builtin_macro]
    pub macro cfg_accessible($item:item) {
        /* compiler built-in */
    }

    /// 展开被应用代码片段中的所有 `#[cfg]` 和 `#[cfg_attr]` 属性。
    #[unstable(
        feature = "cfg_eval",
        issue = "82679",
        reason = "`cfg_eval` is a recently implemented feature"
    )]
    #[rustc_builtin_macro]
    pub macro cfg_eval($($tt:tt)*) {
        /* compiler built-in */
    }

    /// 向带有函数体的条目提供一组类型别名和其他包含 opaque type 的类型定义。
    /// 该列表会在该函数体中用于定义 opaque type 的隐藏类型。
    /// 只能应用于带有函数体的东西。
    #[unstable(
        feature = "type_alias_impl_trait",
        issue = "63063",
        reason = "`type_alias_impl_trait` has open design concerns"
    )]
    #[rustc_builtin_macro]
    pub macro define_opaque($($tt:tt)*) {
        /* compiler built-in */
    }

    /// 类型 ascription 的不稳定占位符。
    #[allow_internal_unstable(builtin_syntax)]
    #[unstable(
        feature = "type_ascription",
        issue = "23416",
        reason = "placeholder syntax for type ascription"
    )]
    #[rustfmt::skip]
    pub macro type_ascribe($expr:expr, $ty:ty) {
        builtin # type_ascribe($expr, $ty)
    }

    /// deref pattern 的不稳定占位符。
    #[allow_internal_unstable(builtin_syntax)]
    #[unstable(
        feature = "deref_patterns",
        issue = "87121",
        reason = "placeholder syntax for deref patterns"
    )]
    pub macro deref($pat:pat) {
        builtin # deref($pat)
    }

    /// 生成 `From` trait impl 的 derive 宏。
    /// 目前它只能用于单字段结构体。
    // 注意,该宏位于与 `From` trait 不同的模块中,
    // 以避免有人导入 `std::convert::From` 时触发不稳定 feature 被使用。
    #[rustc_builtin_macro]
    #[unstable(feature = "derive_from", issue = "144889")]
    pub macro From($item: item) {
        /* compiler built-in */
    }

    /// Externally Implementable Item:定义一个可以覆盖被应用条目的属性宏。
    #[unstable(feature = "extern_item_impls", issue = "125418")]
    #[rustc_builtin_macro]
    #[allow_internal_unstable(eii_internals, decl_macro, rustc_attrs)]
    pub macro eii($item:item) {
        /* compiler built-in */
    }

    /// Unsafely Externally Implementable Item:定义一个可以覆盖被应用条目的 unsafe 属性宏。
    #[unstable(feature = "extern_item_impls", issue = "125418")]
    #[rustc_builtin_macro]
    #[allow_internal_unstable(eii_internals, decl_macro, rustc_attrs)]
    pub macro unsafe_eii($item:item) {
        /* compiler built-in */
    }

    /// EII 的实现细节。
    #[unstable(feature = "eii_internals", issue = "none")]
    #[rustc_builtin_macro]
    pub macro eii_declaration($item:item) {
        /* compiler built-in */
    }
}
