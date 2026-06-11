//! 标准库宏（Standard library macros）
//!
//! 本模块包含一组从标准库导出的宏。每个宏在链接标准库时都可供使用。
//!
//! 这些宏与 `core` 中的同名宏的关键区别在于：它们依赖 `std` 的运行时 I/O 设施。
//! `print!`/`println!` 写入 [`io::stdout`]，`eprint!`/`eprintln!`/`dbg!` 写入
//! [`io::stderr`]——这些设施只有在有 OS 的 `std` 中才存在。它们在每次调用时都会
//! 获取相应输出流的锁，因此在热循环中频繁调用可能成为瓶颈；写入失败会以 panic 暴露。
// ignore-tidy-dbg

#[doc = include_str!("../../core/src/macros/panic.md")]
#[macro_export]
#[rustc_builtin_macro(std_panic)]
#[stable(feature = "rust1", since = "1.0.0")]
#[allow_internal_unstable(edition_panic)]
#[cfg_attr(not(test), rustc_diagnostic_item = "std_panic_macro")]
macro_rules! panic {
    // 根据调用方所处的 edition，展开为 `$crate::panic::panic_2015` 或
    // `$crate::panic::panic_2021`。
    ($($arg:tt)*) => {
        /* compiler built-in */
    };
}

/// 打印到标准输出（standard output）。
///
/// 等同于 [`println!`] 宏，区别在于不会在消息末尾打印换行符。
///
/// 注意：stdout 默认通常是行缓冲（line-buffered）的，因此可能需要使用
/// [`io::stdout().flush()`][flush] 来确保输出被立即送出。
///
/// `print!` 宏会在每次调用时锁定标准输出。如果你在热循环（hot loop）中调用
/// `print!`，这一行为可能成为该循环的瓶颈。要避免这一点，请用
/// [`io::stdout().lock()`][lock] 锁定 stdout：
/// ```
/// use std::io::{stdout, Write};
///
/// let mut lock = stdout().lock();
/// write!(lock, "hello world").unwrap();
/// ```
///
/// 仅将 `print!` 用于程序的主要输出。打印错误和进度消息请改用 [`eprint!`]。
///
/// 宏参数语法的详情参见 [`std::fmt`](crate::fmt) 中的格式化文档。
///
/// [flush]: crate::io::Write::flush
/// [`println!`]: crate::println
/// [`eprint!`]: crate::eprint
/// [lock]: crate::io::Stdout
///
/// # Panics
///
/// 如果写入 `io::stdout()` 失败，则会 panic。
///
/// 写入非阻塞（non-blocking）的 stdout 可能产生错误，从而导致本宏 panic。
///
/// # 示例
///
/// ```
/// use std::io::{self, Write};
///
/// print!("this ");
/// print!("will ");
/// print!("be ");
/// print!("on ");
/// print!("the ");
/// print!("same ");
/// print!("line ");
///
/// io::stdout().flush().unwrap();
///
/// print!("this string has a newline, why not choose println! instead?\n");
///
/// io::stdout().flush().unwrap();
/// ```
#[macro_export]
#[stable(feature = "rust1", since = "1.0.0")]
#[cfg_attr(not(test), rustc_diagnostic_item = "print_macro")]
#[allow_internal_unstable(print_internals)]
macro_rules! print {
    ($($arg:tt)*) => {{
        $crate::io::_print($crate::format_args!($($arg)*));
    }};
}

/// 打印到标准输出，并附带一个换行符。
///
/// 在所有平台上，换行符都是单独的 LINE FEED 字符（`\n`/`U+000A`）
///（不附带额外的 CARRIAGE RETURN（`\r`/`U+000D`））。
///
/// 本宏使用与 [`format!`] 相同的语法，但写入标准输出而非返回字符串。
/// 更多信息参见 [`std::fmt`]。
///
/// `println!` 宏会在每次调用时锁定标准输出。如果你在热循环（hot loop）中调用
/// `println!`，这一行为可能成为该循环的瓶颈。要避免这一点，请用
/// [`io::stdout().lock()`][lock] 锁定 stdout：
/// ```
/// use std::io::{stdout, Write};
///
/// let mut lock = stdout().lock();
/// writeln!(lock, "hello world").unwrap();
/// ```
///
/// 仅将 `println!` 用于程序的主要输出。打印错误和进度消息请改用 [`eprintln!`]。
///
/// 宏参数语法的详情参见 [`std::fmt`](crate::fmt) 中的格式化文档。
///
/// [`std::fmt`]: crate::fmt
/// [`eprintln!`]: crate::eprintln
/// [lock]: crate::io::Stdout
///
/// # Panics
///
/// 如果写入 [`io::stdout`] 失败，则会 panic。
///
/// 写入非阻塞（non-blocking）的 stdout 可能产生错误，从而导致本宏 panic。
///
/// [`io::stdout`]: crate::io::stdout
///
/// # 示例
///
/// ```
/// println!(); // 仅打印一个换行符
/// println!("hello there!");
/// println!("format {} arguments", "some");
/// let local_variable = "some";
/// println!("format {local_variable} arguments");
/// ```
#[macro_export]
#[stable(feature = "rust1", since = "1.0.0")]
#[cfg_attr(not(test), rustc_diagnostic_item = "println_macro")]
#[allow_internal_unstable(print_internals, format_args_nl)]
macro_rules! println {
    () => {
        $crate::print!("\n")
    };
    ($($arg:tt)*) => {{
        $crate::io::_print($crate::format_args_nl!($($arg)*));
    }};
}

/// 打印到标准错误（standard error）。
///
/// 等同于 [`print!`] 宏，区别在于输出去往 [`io::stderr`] 而非 [`io::stdout`]。
/// 用法示例参见 [`print!`]。
///
/// 仅将 `eprint!` 用于错误和进度消息。程序的主要输出请改用 `print!`。
///
/// [`io::stderr`]: crate::io::stderr
/// [`io::stdout`]: crate::io::stdout
///
/// 宏参数语法的详情参见 [`std::fmt`](crate::fmt) 中的格式化文档。
///
/// # Panics
///
/// 如果写入 `io::stderr` 失败，则会 panic。
///
/// 写入非阻塞（non-blocking）的 stderr 可能产生错误，从而导致本宏 panic。
///
/// # 示例
///
/// ```
/// eprint!("Error: Could not complete task");
/// ```
#[macro_export]
#[stable(feature = "eprint", since = "1.19.0")]
#[cfg_attr(not(test), rustc_diagnostic_item = "eprint_macro")]
#[allow_internal_unstable(print_internals)]
macro_rules! eprint {
    ($($arg:tt)*) => {{
        $crate::io::_eprint($crate::format_args!($($arg)*));
    }};
}

/// 打印到标准错误，并附带一个换行符。
///
/// 等同于 [`println!`] 宏，区别在于输出去往 [`io::stderr`] 而非 [`io::stdout`]。
/// 用法示例参见 [`println!`]。
///
/// 仅将 `eprintln!` 用于错误和进度消息。程序的主要输出请改用 `println!`。
///
/// 宏参数语法的详情参见 [`std::fmt`](crate::fmt) 中的格式化文档。
///
/// [`io::stderr`]: crate::io::stderr
/// [`io::stdout`]: crate::io::stdout
/// [`println!`]: crate::println
///
/// # Panics
///
/// 如果写入 `io::stderr` 失败，则会 panic。
///
/// 写入非阻塞（non-blocking）的 stderr 可能产生错误，从而导致本宏 panic。
///
/// # 示例
///
/// ```
/// eprintln!("Error: Could not complete task");
/// ```
#[macro_export]
#[stable(feature = "eprint", since = "1.19.0")]
#[cfg_attr(not(test), rustc_diagnostic_item = "eprintln_macro")]
#[allow_internal_unstable(print_internals, format_args_nl)]
macro_rules! eprintln {
    () => {
        $crate::eprint!("\n")
    };
    ($($arg:tt)*) => {{
        $crate::io::_eprint($crate::format_args_nl!($($arg)*));
    }};
}

/// 打印并返回给定表达式的值，用于快速而粗略（quick and dirty）的调试。
///
/// 一个示例：
///
/// ```rust
/// let a = 2;
/// let b = dbg!(a * 2) + 1;
/// //      ^-- 打印出: [src/main.rs:2:9] a * 2 = 4
/// assert_eq!(b, 5);
/// ```
///
/// 本宏的工作方式是：使用给定表达式类型的 `Debug` 实现，把值连同该宏调用处的
/// 源代码位置以及该表达式的源代码一起打印到 [stderr]。
///
/// 在表达式上调用本宏会先移动并取得它的所有权，然后再原封不动地返回求值后的
/// 表达式。如果该表达式的类型没有实现 `Copy`，而你又不想交出所有权，那么对于
/// 某个表达式 `expr`，可以改用 `dbg!(&expr)` 借用它。
///
/// `dbg!` 宏在 release 构建中的工作方式完全相同。这在调试那些只在 release 构建中
/// 出现的问题，或在 release 模式下调试速度明显更快时很有用。
///
/// 注意，本宏意在作为调试工具，因此你应当避免让对它的使用长期留在版本控制中
///（测试等情形除外）。生产代码中的调试输出最好借助其它设施完成，例如 [`log`] crate
/// 中的 [`debug!`] 宏。
///
/// # Stability
///
/// 本宏打印出的确切输出不应被依赖，并且将来可能发生变化。
///
/// # Panics
///
/// 如果写入 `io::stderr` 失败，则会 panic。
///
/// # 更多示例
///
/// 配合方法调用：
///
/// ```rust
/// fn foo(n: usize) {
///     if let Some(_) = dbg!(n.checked_sub(4)) {
///         // ...
///     }
/// }
///
/// foo(3)
/// ```
///
/// 这会打印到 [stderr]：
///
/// ```text,ignore
/// [src/main.rs:2:22] n.checked_sub(4) = None
/// ```
///
/// 朴素的阶乘实现：
///
/// ```rust
/// fn factorial(n: u32) -> u32 {
///     if dbg!(n <= 1) {
///         dbg!(1)
///     } else {
///         dbg!(n * factorial(n - 1))
///     }
/// }
///
/// dbg!(factorial(4));
/// ```
///
/// 这会打印到 [stderr]：
///
/// ```text,ignore
/// [src/main.rs:2:8] n <= 1 = false
/// [src/main.rs:2:8] n <= 1 = false
/// [src/main.rs:2:8] n <= 1 = false
/// [src/main.rs:2:8] n <= 1 = true
/// [src/main.rs:3:9] 1 = 1
/// [src/main.rs:7:9] n * factorial(n - 1) = 2
/// [src/main.rs:7:9] n * factorial(n - 1) = 6
/// [src/main.rs:7:9] n * factorial(n - 1) = 24
/// [src/main.rs:9:1] factorial(4) = 24
/// ```
///
/// `dbg!(..)` 宏会移动其输入：
///
/// ```compile_fail
/// /// 一个对 `usize` 的包装，关键在于它不可 Copy。
/// #[derive(Debug)]
/// struct NoCopy(usize);
///
/// let a = NoCopy(42);
/// let _ = dbg!(a); // <-- `a` 在此处被移动。
/// let _ = dbg!(a); // <-- `a` 再次被移动；错误！
/// ```
///
/// 你也可以不带值地使用 `dbg!()`，以便每次到达该处时仅打印文件名和行号。
///
/// 最后，如果你想 `dbg!(..)` 多个值，它会把它们当作一个元组（tuple）来处理
///（并且也会返回该元组）：
///
/// ```
/// assert_eq!(dbg!(1usize, 2u32), (1, 2));
/// ```
///
/// 不过，带有尾随逗号的单个参数仍然不会被当作元组处理，这遵循“宏调用中忽略尾随逗号”
/// 的惯例。如果你确实需要一个 1 元组（1-tuple），可以直接使用它：
///
/// ```
/// assert_eq!(1, dbg!(1u32,)); // 尾随逗号被忽略
/// assert_eq!((1,), dbg!((1u32,))); // 1 元组
/// ```
///
/// [stderr]: https://en.wikipedia.org/wiki/Standard_streams#Standard_error_(stderr)
/// [`debug!`]: https://docs.rs/log/*/log/macro.debug.html
/// [`log`]: https://crates.io/crates/log
#[macro_export]
#[cfg_attr(not(test), rustc_diagnostic_item = "dbg_macro")]
#[stable(feature = "dbg_macro", since = "1.32.0")]
macro_rules! dbg {
    // NOTE: 我们不能用 `concat!` 把一个静态字符串拼成 `eprintln!` 的格式参数，
    // 因为 `file!` 可能包含 `{`，或者 `$val` 表达式可能是一个块（`{ .. }`），
    // 这两种情况都会导致 `eprintln!` 格式不正确（malformed）。
    () => {
        $crate::eprintln!("[{}:{}:{}]", $crate::file!(), $crate::line!(), $crate::column!())
    };
    ($val:expr $(,)?) => {
        // 这里有意使用 `match`，因为它会影响临时值（temporaries）的生命周期 —
        // https://stackoverflow.com/a/48732525/1063961
        match $val {
            tmp => {
                $crate::eprintln!("[{}:{}:{}] {} = {:#?}",
                    $crate::file!(),
                    $crate::line!(),
                    $crate::column!(),
                    $crate::stringify!($val),
                    // `&T: Debug` 检查在此处发生（而非在格式字面量的脱糖过程中），
                    // 以避免出现与格式字面量相关的消息和建议。
                    &&tmp as &dyn $crate::fmt::Debug,
                );
                tmp
            }
        }
    };
    ($($val:expr),+ $(,)?) => {
        ($($crate::dbg!($val)),+,)
    };
}
