#![doc = include_str!("../../core/src/error.md")]
#![stable(feature = "rust1", since = "1.0.0")]

//! 实现说明：`Error` trait 本身以及泛型成员访问相关的 `Request`/`request_ref`/
//! `request_value` 都定义在 `core::error` 中，这里直接重导出（re-export），因此
//! `std::error::Error` 与 `core::error::Error` 是同一个 trait。std 在此基础上做的
//! “扩展”是新增了依赖运行时（如分配、回溯 `Backtrace`）的能力——本文件中定义的
//! [`Report`] 错误报告器即属于这部分：它沿着 `Error::source` 形成的错误源链向下
//! 遍历，并能整合 `Backtrace`，这些都无法在不依赖 std 的 `core` 中实现。

#[stable(feature = "rust1", since = "1.0.0")]
pub use core::error::Error;
#[unstable(feature = "error_generic_member_access", issue = "99301")]
pub use core::error::{Request, request_ref, request_value};

use crate::backtrace::Backtrace;
use crate::fmt::{self, Write};

/// 一个错误报告器，负责打印某个错误及其错误源链（sources）。
///
/// `Report` 还提供了一些格式化配置项，可以把错误源链全部排在同一行，或者采用
/// 多行格式、每个错误源单独占一行。
///
/// `Report` 只要求被包裹的错误实现 `Error`，并不要求它是 `Send`、`Sync` 或
/// `'static`。
///
/// # 示例
///
/// ```rust
/// #![feature(error_reporter)]
/// use std::error::{Error, Report};
/// use std::fmt;
///
/// #[derive(Debug)]
/// struct SuperError {
///     source: SuperErrorSideKick,
/// }
///
/// impl fmt::Display for SuperError {
///     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
///         write!(f, "SuperError is here!")
///     }
/// }
///
/// impl Error for SuperError {
///     fn source(&self) -> Option<&(dyn Error + 'static)> {
///         Some(&self.source)
///     }
/// }
///
/// #[derive(Debug)]
/// struct SuperErrorSideKick;
///
/// impl fmt::Display for SuperErrorSideKick {
///     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
///         write!(f, "SuperErrorSideKick is here!")
///     }
/// }
///
/// impl Error for SuperErrorSideKick {}
///
/// fn get_super_error() -> Result<(), SuperError> {
///     Err(SuperError { source: SuperErrorSideKick })
/// }
///
/// fn main() {
///     match get_super_error() {
///         Err(e) => println!("Error: {}", Report::new(e)),
///         _ => println!("No error"),
///     }
/// }
/// ```
///
/// 这个例子产生如下输出：
///
/// ```console
/// Error: SuperError is here!: SuperErrorSideKick is here!
/// ```
///
/// ## 输出一致性（Output consistency）
///
/// `Report` 通过 `Display` 和 `Debug` 产生完全相同的输出，因此它能与
/// [`Result::unwrap`]/[`Result::expect`] 很好地配合——后者通过 `Debug` 打印其
/// `Err` 变体：
///
/// ```should_panic
/// #![feature(error_reporter)]
/// use std::error::Report;
/// # use std::error::Error;
/// # use std::fmt;
/// # #[derive(Debug)]
/// # struct SuperError {
/// #     source: SuperErrorSideKick,
/// # }
/// # impl fmt::Display for SuperError {
/// #     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
/// #         write!(f, "SuperError is here!")
/// #     }
/// # }
/// # impl Error for SuperError {
/// #     fn source(&self) -> Option<&(dyn Error + 'static)> {
/// #         Some(&self.source)
/// #     }
/// # }
/// # #[derive(Debug)]
/// # struct SuperErrorSideKick;
/// # impl fmt::Display for SuperErrorSideKick {
/// #     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
/// #         write!(f, "SuperErrorSideKick is here!")
/// #     }
/// # }
/// # impl Error for SuperErrorSideKick {}
/// # fn get_super_error() -> Result<(), SuperError> {
/// #     Err(SuperError { source: SuperErrorSideKick })
/// # }
///
/// get_super_error().map_err(Report::new).unwrap();
/// ```
///
/// 这个例子产生如下输出：
///
/// ```console
/// thread 'main' panicked at src/error.rs:34:40:
/// called `Result::unwrap()` on an `Err` value: SuperError is here!: SuperErrorSideKick is here!
/// note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace
/// ```
///
/// ## 从 `main` 返回（Return from `main`）
///
/// `Report` 还为所有实现了 [`Error`] 的类型实现了 `From`；这一点与它的 `Debug`
/// 输出结合起来，使 `Report` 成为格式化 `main` 返回错误的理想起点。
///
/// ```should_panic
/// #![feature(error_reporter)]
/// use std::error::Report;
/// # use std::error::Error;
/// # use std::fmt;
/// # #[derive(Debug)]
/// # struct SuperError {
/// #     source: SuperErrorSideKick,
/// # }
/// # impl fmt::Display for SuperError {
/// #     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
/// #         write!(f, "SuperError is here!")
/// #     }
/// # }
/// # impl Error for SuperError {
/// #     fn source(&self) -> Option<&(dyn Error + 'static)> {
/// #         Some(&self.source)
/// #     }
/// # }
/// # #[derive(Debug)]
/// # struct SuperErrorSideKick;
/// # impl fmt::Display for SuperErrorSideKick {
/// #     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
/// #         write!(f, "SuperErrorSideKick is here!")
/// #     }
/// # }
/// # impl Error for SuperErrorSideKick {}
/// # fn get_super_error() -> Result<(), SuperError> {
/// #     Err(SuperError { source: SuperErrorSideKick })
/// # }
///
/// fn main() -> Result<(), Report<SuperError>> {
///     get_super_error()?;
///     Ok(())
/// }
/// ```
///
/// 这个例子产生如下输出：
///
/// ```console
/// Error: SuperError is here!: SuperErrorSideKick is here!
/// ```
///
/// **注意**：通过 `?` 和 `From` 构造出来的 `Report` 会被配置为使用单行输出格式。
/// 如果你想确保 `Report` 以美化（pretty）方式打印并包含回溯信息，就需要手动进行
/// 转换并开启相应的标志位。
///
/// ```should_panic
/// #![feature(error_reporter)]
/// use std::error::Report;
/// # use std::error::Error;
/// # use std::fmt;
/// # #[derive(Debug)]
/// # struct SuperError {
/// #     source: SuperErrorSideKick,
/// # }
/// # impl fmt::Display for SuperError {
/// #     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
/// #         write!(f, "SuperError is here!")
/// #     }
/// # }
/// # impl Error for SuperError {
/// #     fn source(&self) -> Option<&(dyn Error + 'static)> {
/// #         Some(&self.source)
/// #     }
/// # }
/// # #[derive(Debug)]
/// # struct SuperErrorSideKick;
/// # impl fmt::Display for SuperErrorSideKick {
/// #     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
/// #         write!(f, "SuperErrorSideKick is here!")
/// #     }
/// # }
/// # impl Error for SuperErrorSideKick {}
/// # fn get_super_error() -> Result<(), SuperError> {
/// #     Err(SuperError { source: SuperErrorSideKick })
/// # }
///
/// fn main() -> Result<(), Report<SuperError>> {
///     get_super_error()
///         .map_err(Report::from)
///         .map_err(|r| r.pretty(true).show_backtrace(true))?;
///     Ok(())
/// }
/// ```
///
/// 这个例子产生如下输出：
///
/// ```console
/// Error: SuperError is here!
///
/// Caused by:
///       SuperErrorSideKick is here!
/// ```
#[unstable(feature = "error_reporter", issue = "90172")]
pub struct Report<E = Box<dyn Error>> {
    /// 正在被报告的那个错误。
    error: E,
    /// 报告中是否应当包含回溯（backtrace）信息。
    show_backtrace: bool,
    /// 报告是否应当以美化（pretty-printed）方式输出。
    pretty: bool,
}

impl<E> Report<E>
where
    Report<E>: From<E>,
{
    /// 由一个输入错误创建一个新的 `Report`。
    #[unstable(feature = "error_reporter", issue = "90172")]
    pub fn new(error: E) -> Report<E> {
        Self::from(error)
    }
}

impl<E> Report<E> {
    /// 启用跨多行的美化打印（pretty-printing）。
    ///
    /// # 示例
    ///
    /// ```rust
    /// #![feature(error_reporter)]
    /// use std::error::Report;
    /// # use std::error::Error;
    /// # use std::fmt;
    /// # #[derive(Debug)]
    /// # struct SuperError {
    /// #     source: SuperErrorSideKick,
    /// # }
    /// # impl fmt::Display for SuperError {
    /// #     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    /// #         write!(f, "SuperError is here!")
    /// #     }
    /// # }
    /// # impl Error for SuperError {
    /// #     fn source(&self) -> Option<&(dyn Error + 'static)> {
    /// #         Some(&self.source)
    /// #     }
    /// # }
    /// # #[derive(Debug)]
    /// # struct SuperErrorSideKick;
    /// # impl fmt::Display for SuperErrorSideKick {
    /// #     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    /// #         write!(f, "SuperErrorSideKick is here!")
    /// #     }
    /// # }
    /// # impl Error for SuperErrorSideKick {}
    ///
    /// let error = SuperError { source: SuperErrorSideKick };
    /// let report = Report::new(error).pretty(true);
    /// eprintln!("Error: {report:?}");
    /// ```
    ///
    /// 这个例子产生如下输出：
    ///
    /// ```console
    /// Error: SuperError is here!
    ///
    /// Caused by:
    ///       SuperErrorSideKick is here!
    /// ```
    ///
    /// 当存在多个错误源时，这些“起因（causes）”会按照从最外层错误开始的遍历顺序
    /// 依次编号。
    ///
    /// ```rust
    /// #![feature(error_reporter)]
    /// use std::error::Report;
    /// # use std::error::Error;
    /// # use std::fmt;
    /// # #[derive(Debug)]
    /// # struct SuperError {
    /// #     source: SuperErrorSideKick,
    /// # }
    /// # impl fmt::Display for SuperError {
    /// #     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    /// #         write!(f, "SuperError is here!")
    /// #     }
    /// # }
    /// # impl Error for SuperError {
    /// #     fn source(&self) -> Option<&(dyn Error + 'static)> {
    /// #         Some(&self.source)
    /// #     }
    /// # }
    /// # #[derive(Debug)]
    /// # struct SuperErrorSideKick {
    /// #     source: SuperErrorSideKickSideKick,
    /// # }
    /// # impl fmt::Display for SuperErrorSideKick {
    /// #     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    /// #         write!(f, "SuperErrorSideKick is here!")
    /// #     }
    /// # }
    /// # impl Error for SuperErrorSideKick {
    /// #     fn source(&self) -> Option<&(dyn Error + 'static)> {
    /// #         Some(&self.source)
    /// #     }
    /// # }
    /// # #[derive(Debug)]
    /// # struct SuperErrorSideKickSideKick;
    /// # impl fmt::Display for SuperErrorSideKickSideKick {
    /// #     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    /// #         write!(f, "SuperErrorSideKickSideKick is here!")
    /// #     }
    /// # }
    /// # impl Error for SuperErrorSideKickSideKick { }
    ///
    /// let source = SuperErrorSideKickSideKick;
    /// let source = SuperErrorSideKick { source };
    /// let error = SuperError { source };
    /// let report = Report::new(error).pretty(true);
    /// eprintln!("Error: {report:?}");
    /// ```
    ///
    /// 这个例子产生如下输出：
    ///
    /// ```console
    /// Error: SuperError is here!
    ///
    /// Caused by:
    ///    0: SuperErrorSideKick is here!
    ///    1: SuperErrorSideKickSideKick is here!
    /// ```
    #[unstable(feature = "error_reporter", issue = "90172")]
    pub fn pretty(mut self, pretty: bool) -> Self {
        self.pretty = pretty;
        self
    }

    /// 在使用美化输出格式时，如果能取得回溯信息则将其展示出来。
    ///
    /// # 示例
    ///
    /// **注意**：`Report` 会从最外层错误开始查找它能找到的第一个 `Backtrace`。在
    /// 本例中，它将展示错误源链中第二个错误 `SuperErrorSideKick` 上的回溯。
    ///
    /// ```rust
    /// #![feature(error_reporter)]
    /// #![feature(error_generic_member_access)]
    /// # use std::error::Error;
    /// # use std::fmt;
    /// use std::error::Request;
    /// use std::error::Report;
    /// use std::backtrace::Backtrace;
    ///
    /// # #[derive(Debug)]
    /// # struct SuperError {
    /// #     source: SuperErrorSideKick,
    /// # }
    /// # impl fmt::Display for SuperError {
    /// #     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    /// #         write!(f, "SuperError is here!")
    /// #     }
    /// # }
    /// # impl Error for SuperError {
    /// #     fn source(&self) -> Option<&(dyn Error + 'static)> {
    /// #         Some(&self.source)
    /// #     }
    /// # }
    /// #[derive(Debug)]
    /// struct SuperErrorSideKick {
    ///     backtrace: Backtrace,
    /// }
    ///
    /// impl SuperErrorSideKick {
    ///     fn new() -> SuperErrorSideKick {
    ///         SuperErrorSideKick { backtrace: Backtrace::force_capture() }
    ///     }
    /// }
    ///
    /// impl Error for SuperErrorSideKick {
    ///     fn provide<'a>(&'a self, request: &mut Request<'a>) {
    ///         request.provide_ref::<Backtrace>(&self.backtrace);
    ///     }
    /// }
    ///
    /// // 本例的其余部分保持不变 ...
    /// # impl fmt::Display for SuperErrorSideKick {
    /// #     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    /// #         write!(f, "SuperErrorSideKick is here!")
    /// #     }
    /// # }
    ///
    /// let source = SuperErrorSideKick::new();
    /// let error = SuperError { source };
    /// let report = Report::new(error).pretty(true).show_backtrace(true);
    /// eprintln!("Error: {report:?}");
    /// ```
    ///
    /// 这个例子产生的输出类似于下面这样：
    ///
    /// ```console
    /// Error: SuperError is here!
    ///
    /// Caused by:
    ///       SuperErrorSideKick is here!
    ///
    /// Stack backtrace:
    ///    0: rust_out::main::_doctest_main_src_error_rs_1158_0::SuperErrorSideKick::new
    ///    1: rust_out::main::_doctest_main_src_error_rs_1158_0
    ///    2: rust_out::main
    ///    3: core::ops::function::FnOnce::call_once
    ///    4: std::sys::backtrace::__rust_begin_short_backtrace
    ///    5: std::rt::lang_start::{{closure}}
    ///    6: std::panicking::try
    ///    7: std::rt::lang_start_internal
    ///    8: std::rt::lang_start
    ///    9: main
    ///   10: __libc_start_main
    ///   11: _start
    /// ```
    #[unstable(feature = "error_reporter", issue = "90172")]
    pub fn show_backtrace(mut self, show_backtrace: bool) -> Self {
        self.show_backtrace = show_backtrace;
        self
    }
}

impl<E> Report<E>
where
    E: Error,
{
    fn backtrace(&self) -> Option<&Backtrace> {
        // 必须直接从第一个错误上抓取回溯，因为该错误可能不是 'static 的
        let backtrace = request_ref(&self.error);
        let backtrace = backtrace.or_else(|| {
            self.error
                .source()
                .map(|source| source.sources().find_map(|source| request_ref(source)))
                .flatten()
        });
        backtrace
    }

    /// 将报告格式化为单行。
    #[unstable(feature = "error_reporter", issue = "90172")]
    fn fmt_singleline(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.error)?;

        let sources = self.error.source().into_iter().flat_map(<dyn Error>::sources);

        for cause in sources {
            write!(f, ": {cause}")?;
        }

        Ok(())
    }

    /// 将报告格式化为多行，每个错误起因（cause）单独占一行。
    #[unstable(feature = "error_reporter", issue = "90172")]
    fn fmt_multiline(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let error = &self.error;

        write!(f, "{error}")?;

        if let Some(cause) = error.source() {
            write!(f, "\n\nCaused by:")?;

            let multiple = cause.source().is_some();

            for (ind, error) in cause.sources().enumerate() {
                writeln!(f)?;
                let mut indented = Indented { inner: f };
                if multiple {
                    write!(indented, "{ind: >4}: {error}")?;
                } else {
                    write!(indented, "      {error}")?;
                }
            }
        }

        if self.show_backtrace {
            if let Some(backtrace) = self.backtrace() {
                write!(f, "\n\nStack backtrace:\n{}", backtrace.to_string().trim_end())?;
            }
        }

        Ok(())
    }
}

#[unstable(feature = "error_reporter", issue = "90172")]
impl<E> From<E> for Report<E>
where
    E: Error,
{
    fn from(error: E) -> Self {
        Report { error, show_backtrace: false, pretty: false }
    }
}

#[unstable(feature = "error_reporter", issue = "90172")]
impl<E> fmt::Display for Report<E>
where
    E: Error,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.pretty { self.fmt_multiline(f) } else { self.fmt_singleline(f) }
    }
}

// 该类型有意让 `Display` 和 `Debug` 输出完全相同的格式，以适配你对 `Report`
// 做 unwrap、或把它从 main 返回的场景。
#[unstable(feature = "error_reporter", issue = "90172")]
impl<E> fmt::Debug for Report<E>
where
    Report<E>: fmt::Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

/// 包装类型：用于对内部错误源（source）的输出进行缩进。
struct Indented<'a, D> {
    inner: &'a mut D,
}

impl<T> Write for Indented<'_, T>
where
    T: Write,
{
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for (i, line) in s.split('\n').enumerate() {
            if i > 0 {
                self.inner.write_char('\n')?;
                self.inner.write_str("      ")?;
            }

            self.inner.write_str(line)?;
        }

        Ok(())
    }
}
