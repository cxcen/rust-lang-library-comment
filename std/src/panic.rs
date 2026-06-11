//! 标准库中的 panic（恐慌）支持。
//!
//! 本模块对外暴露 panic 机制的“公共面”：
//! - [`catch_unwind`] / [`resume_unwind`]：在某个调用边界处捕获正在 unwind（栈展开）的
//!   panic，并把它原样重新抛出。两者配合可以让 panic 跨越一段 C 代码或线程边界传递。
//! - [`UnwindSafe`] / [`RefUnwindSafe`] / [`AssertUnwindSafe`]：在类型系统层面编码
//!   “异常安全（exception safety）”的概念，约束哪些值可以安全地穿越 [`catch_unwind`] 边界。
//! - [`set_hook`] / [`take_hook`] / [`update_hook`]：注册、取出、更新全局 panic 钩子（hook）。
//! - [`PanicHookInfo`] / [`Location`]：panic 钩子收到的信息载体（payload、源码位置等）。
//! - [`always_abort`]：把后续所有 panic 都切换为直接 abort（用于 `fork` 之后等场景）。
//!
//! 真正的 panic 运行时（hook 调用流程、unwind/abort 决策、panic 计数等）实现在
//! [`crate::panicking`] 模块；本模块主要是其稳定的对外接口与类型定义，并维护
//! “backtrace 是否捕获/如何显示”这一全局配置（见 `SHOULD_CAPTURE` 与 [`BacktraceStyle`]）。
//! panic 的失败（例如不可 unwind 场景下仍尝试 unwind）最终会通过 abort 暴露给进程。

#![stable(feature = "std_panic", since = "1.9.0")]

use crate::any::Any;
use crate::sync::atomic::{Atomic, AtomicU8, Ordering};
use crate::sync::{Condvar, Mutex, RwLock};
use crate::thread::Result;
use crate::{collections, fmt, panicking};

#[stable(feature = "panic_hooks", since = "1.10.0")]
#[deprecated(
    since = "1.82.0",
    note = "use `PanicHookInfo` instead",
    suggestion = "std::panic::PanicHookInfo"
)]
/// 携带 panic 相关信息的结构体。
///
/// `PanicInfo` 已被重命名为 [`PanicHookInfo`]，以避免与
/// [`core::panic::PanicInfo`] 混淆。
pub type PanicInfo<'a> = PanicHookInfo<'a>;

/// 携带 panic 相关信息的结构体。
///
/// `PanicHookInfo` 结构体会被传递给通过 [`set_hook`] 函数设置的 panic 钩子（hook）。
///
/// # 示例
///
/// ```should_panic
/// use std::panic;
///
/// panic::set_hook(Box::new(|panic_info| {
///     println!("panic occurred: {panic_info}");
/// }));
///
/// panic!("critical system failure");
/// ```
///
/// [`set_hook`]: ../../std/panic/fn.set_hook.html
#[stable(feature = "panic_hook_info", since = "1.81.0")]
#[derive(Debug)]
pub struct PanicHookInfo<'a> {
    payload: &'a (dyn Any + Send),
    location: &'a Location<'a>,
    can_unwind: bool,
    force_no_backtrace: bool,
}

impl<'a> PanicHookInfo<'a> {
    #[inline]
    pub(crate) fn new(
        location: &'a Location<'a>,
        payload: &'a (dyn Any + Send),
        can_unwind: bool,
        force_no_backtrace: bool,
    ) -> Self {
        PanicHookInfo { payload, location, can_unwind, force_no_backtrace }
    }

    /// 返回与本次 panic 关联的 payload（载荷）。
    ///
    /// 它通常（但并非总是）是一个 `&'static str` 或 [`String`]。
    /// 如果你只关心这类字符串 payload，请改用 [`payload_as_str`]。
    ///
    /// 在 Rust 2021 及更新的版本中，调用 `panic!()` 宏总是会产生类型为
    /// `&'static str` 或 `String` 的 panic payload。
    ///
    /// 只有调用 [`panic_any`]
    /// （或在 Rust 2018 及更早版本中写 `panic!(x)` 且 `x` 不是字符串）
    /// 才可能产生非 `&'static str` 或 `String` 的 panic payload。
    ///
    /// [`String`]: ../../std/string/struct.String.html
    /// [`payload_as_str`]: PanicHookInfo::payload_as_str
    ///
    /// # 示例
    ///
    /// ```should_panic
    /// use std::panic;
    ///
    /// panic::set_hook(Box::new(|panic_info| {
    ///     if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
    ///         println!("panic occurred: {s:?}");
    ///     } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
    ///         println!("panic occurred: {s:?}");
    ///     } else {
    ///         println!("panic occurred");
    ///     }
    /// }));
    ///
    /// panic!("Normal panic");
    /// ```
    #[must_use]
    #[inline]
    #[stable(feature = "panic_hooks", since = "1.10.0")]
    pub fn payload(&self) -> &(dyn Any + Send) {
        self.payload
    }

    /// 返回与本次 panic 关联的 payload（如果它是字符串）。
    ///
    /// 当 payload 的类型为 `&'static str` 或 `String` 时返回该字符串。
    ///
    /// 在 Rust 2021 及更新的版本中，调用 `panic!()` 宏总是会产生使得
    /// `payload_as_str` 返回 `Some` 的 panic payload。
    ///
    /// 只有调用 [`panic_any`]
    /// （或在 Rust 2018 及更早版本中写 `panic!(x)` 且 `x` 不是字符串）
    /// 才可能产生使得 `payload_as_str` 返回 `None` 的 panic payload。
    ///
    /// # 示例
    ///
    /// ```should_panic
    /// std::panic::set_hook(Box::new(|panic_info| {
    ///     if let Some(s) = panic_info.payload_as_str() {
    ///         println!("panic occurred: {s:?}");
    ///     } else {
    ///         println!("panic occurred");
    ///     }
    /// }));
    ///
    /// panic!("Normal panic");
    /// ```
    #[must_use]
    #[inline]
    #[stable(feature = "panic_payload_as_str", since = "1.91.0")]
    pub fn payload_as_str(&self) -> Option<&str> {
        if let Some(s) = self.payload.downcast_ref::<&str>() {
            Some(s)
        } else if let Some(s) = self.payload.downcast_ref::<String>() {
            Some(s)
        } else {
            None
        }
    }

    /// 返回 panic 起源处的源码位置信息（如果可用）。
    ///
    /// 当前实现总是返回 [`Some`]，但这一行为在未来版本中可能改变。
    ///
    /// # 示例
    ///
    /// ```should_panic
    /// use std::panic;
    ///
    /// panic::set_hook(Box::new(|panic_info| {
    ///     if let Some(location) = panic_info.location() {
    ///         println!("panic occurred in file '{}' at line {}",
    ///             location.file(),
    ///             location.line(),
    ///         );
    ///     } else {
    ///         println!("panic occurred but can't get location information...");
    ///     }
    /// }));
    ///
    /// panic!("Normal panic");
    /// ```
    #[must_use]
    #[inline]
    #[stable(feature = "panic_hooks", since = "1.10.0")]
    pub fn location(&self) -> Option<&Location<'_>> {
        // 注意：如果将来改成有时返回 None，
        // 需要在 std::panicking::default_hook 和 core::panicking::panic_fmt 中处理那种情况。
        Some(&self.location)
    }

    /// 返回 panic 处理程序是否被允许从 panic 发生点开始展开（unwind）栈。
    ///
    /// 对大多数种类的 panic 这都为 true，例外是：当 panic 是由“试图从 `Drop`
    /// 实现中 unwind 出去”，或“从一个其 ABI 不支持 unwind 的函数中 unwind 出去”
    /// 所引发时，此值为 false。
    ///
    /// 即使本函数返回 false，panic 处理程序进行 unwind 也是安全的；只不过这样
    /// 做会导致 panic 处理程序被再次调用。
    #[must_use]
    #[inline]
    #[unstable(feature = "panic_can_unwind", issue = "92988")]
    pub fn can_unwind(&self) -> bool {
        self.can_unwind
    }

    #[unstable(
        feature = "panic_internals",
        reason = "internal details of the implementation of the `panic!` and related macros",
        issue = "none"
    )]
    #[doc(hidden)]
    #[inline]
    pub fn force_no_backtrace(&self) -> bool {
        self.force_no_backtrace
    }
}

#[stable(feature = "panic_hook_display", since = "1.26.0")]
impl fmt::Display for PanicHookInfo<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("panicked at ")?;
        self.location.fmt(formatter)?;
        if let Some(payload) = self.payload_as_str() {
            formatter.write_str(":\n")?;
            formatter.write_str(payload)?;
        }
        Ok(())
    }
}

#[doc(hidden)]
#[unstable(feature = "edition_panic", issue = "none", reason = "use panic!() instead")]
#[allow_internal_unstable(libstd_sys_internals, const_format_args, panic_internals, rt)]
#[cfg_attr(not(test), rustc_diagnostic_item = "std_panic_2015_macro")]
#[rustc_macro_transparency = "semiopaque"]
pub macro panic_2015 {
    () => ({
        $crate::rt::begin_panic("explicit panic")
    }),
    ($msg:expr $(,)?) => ({
        $crate::rt::begin_panic($msg);
    }),
    // 针对 const_panic 对单参数情况做特殊处理。
    ("{}", $arg:expr $(,)?) => ({
        $crate::rt::panic_display(&$arg);
    }),
    ($fmt:expr, $($arg:tt)+) => ({
        // 这里的分号用于防止格式化机制中产生的临时变量在 panic_fmt 调用之后
        // 仍被视为在调用方处存活。
        $crate::rt::panic_fmt($crate::const_format_args!($fmt, $($arg)+));
    }),
}

#[stable(feature = "panic_hooks", since = "1.10.0")]
pub use core::panic::Location;
#[doc(hidden)]
#[unstable(feature = "edition_panic", issue = "none", reason = "use panic!() instead")]
pub use core::panic::panic_2021;
#[stable(feature = "catch_unwind", since = "1.9.0")]
pub use core::panic::{AssertUnwindSafe, RefUnwindSafe, UnwindSafe};

#[unstable(feature = "panic_update_hook", issue = "92649")]
pub use crate::panicking::update_hook;
#[stable(feature = "panic_hooks", since = "1.10.0")]
pub use crate::panicking::{set_hook, take_hook};

/// 以给定的消息作为 panic payload，令当前线程 panic。
///
/// 消息可以是任意 (`Any + Send`) 类型，而不仅仅是字符串。
///
/// 消息会被包装进 `Box<'static + Any + Send>`，之后可以通过
/// [`PanicHookInfo::payload`] 访问到。
///
/// 关于 panic 的更多信息，请参阅 [`panic!`] 宏。
#[stable(feature = "panic_any", since = "1.51.0")]
#[inline]
#[track_caller]
#[cfg_attr(not(test), rustc_diagnostic_item = "panic_any")]
pub fn panic_any<M: 'static + Any + Send>(msg: M) -> ! {
    crate::panicking::begin_panic(msg);
}

#[stable(feature = "catch_unwind", since = "1.9.0")]
impl<T: ?Sized> UnwindSafe for Mutex<T> {}
#[stable(feature = "catch_unwind", since = "1.9.0")]
impl<T: ?Sized> UnwindSafe for RwLock<T> {}
#[stable(feature = "catch_unwind", since = "1.9.0")]
impl UnwindSafe for Condvar {}

#[stable(feature = "unwind_safe_lock_refs", since = "1.12.0")]
impl<T: ?Sized> RefUnwindSafe for Mutex<T> {}
#[stable(feature = "unwind_safe_lock_refs", since = "1.12.0")]
impl<T: ?Sized> RefUnwindSafe for RwLock<T> {}
#[stable(feature = "unwind_safe_lock_refs", since = "1.12.0")]
impl RefUnwindSafe for Condvar {}

// https://github.com/rust-lang/rust/issues/62301
#[stable(feature = "hashbrown", since = "1.36.0")]
impl<K, V, S> UnwindSafe for collections::HashMap<K, V, S>
where
    K: UnwindSafe,
    V: UnwindSafe,
    S: UnwindSafe,
{
}

#[unstable(feature = "abort_unwind", issue = "130338")]
pub use core::panic::abort_unwind;

/// 调用一个闭包，并在其因 panic 而 unwind 时捕获导致 unwind 的原因。
///
/// 如果闭包没有 panic，本函数返回 `Ok`，其中包含闭包的返回值；如果闭包 panic，
/// 则返回 `Err(cause)`。返回的 `cause` 就是当初触发 panic 时所用的那个对象。
///
/// 那些期望被“不支持 unwind 的外部代码”（例如用 `-fno-exceptions` 编译的 C 代码）
/// 调用的 Rust 函数，应当定义为 `extern "C"`，这能确保 Rust 代码一旦 panic 就会
/// 被自动捕获并令进程 abort。如果这正是你想要的行为，就无需显式使用
/// `catch_unwind`。只有在需要更优雅的错误处理时才应使用本函数。
///
/// **不**建议把本函数当作通用的 try/catch 机制使用。对于会按常规失败的函数，
/// [`Result`] 类型才是更合适的选择。此外，本函数并不保证能捕获所有 panic，详见
/// 下文 “Notes” 一节。
///
/// 提供的闭包必须满足 [`UnwindSafe`] trait，以确保所有被捕获的变量都能安全地
/// 穿越这道边界。该约束的目的，是在类型系统中编码 [异常安全（exception safety）][rfc]
/// 这一概念。大多数对本函数的使用都无需关心此约束，因为在没有 `unsafe` 代码的
/// 情况下程序天然就是 unwind safe 的。若它成为障碍，可以用 [`AssertUnwindSafe`]
/// 包装结构体来快速断言此处的用法确实是 unwind safe 的。
///
/// [rfc]: https://github.com/rust-lang/rfcs/blob/master/text/1236-stabilize-catch-panic.md
///
/// # Notes
///
/// 本函数 **可能并不会捕获所有 Rust panic**。Rust 的 panic 并不总是通过 unwind
/// 实现，也可以通过令进程 abort 来实现。本函数 *只* 捕获 unwind 形式的 panic，
/// 不会捕获那些直接令进程 abort 的 panic。
///
/// 如果已经设置了自定义 panic 钩子，它会在 panic 被捕获之前、在 unwind 之前被调用。
///
/// 虽然允许通过合适的 ABI（如 `"C-unwind"`）让外部异常（例如 C++ 代码抛出的异常，
/// 或在用其他运行时编译/链接的 Rust 代码中触发的 `panic!`）unwind 进入 Rust 代码，
/// 但用本函数去捕获这样的异常会有以下两种行为之一，且具体是哪一种是未指定的：
///
/// * 进程 abort，但会先执行 `f` 及其所调用函数的全部析构函数。
/// * 函数返回一个 `Result::Err`，其中包含一个不透明类型。
///
/// 最后，**务必小心你如何 drop 本函数的返回值**。如果它是 `Err`，则其中包含 panic
/// payload，而 drop 该 payload 本身又可能引发 panic！
///
/// # 示例
///
/// ```
/// use std::panic;
///
/// let result = panic::catch_unwind(|| {
///     println!("hello!");
/// });
/// assert!(result.is_ok());
///
/// let result = panic::catch_unwind(|| {
///     panic!("oh no!");
/// });
/// assert!(result.is_err());
/// ```
#[stable(feature = "catch_unwind", since = "1.9.0")]
pub fn catch_unwind<F: FnOnce() -> R + UnwindSafe, R>(f: F) -> Result<R> {
    unsafe { panicking::catch_unwind(f) }
}

/// 触发一次 panic，但不调用 panic 钩子。
///
/// 它被设计为与 [`catch_unwind`] 配合使用，例如把一个 panic 跨越一层 C 代码
/// 继续向上传递。
///
/// # Notes
///
/// 注意，Rust 中的 panic 并不总是通过 unwind 实现，也可能通过令进程 abort 来实现。
/// 如果在 panic 以这种方式实现时调用本函数，那么本函数会令进程 abort，而不会
/// 触发一次 unwind。
///
/// # 示例
///
/// ```should_panic
/// use std::panic;
///
/// let result = panic::catch_unwind(|| {
///     if 1 != 2 {
///         panic!("oh no!");
///     }
/// });
///
/// if let Err(err) = result {
///     panic::resume_unwind(err);
/// }
/// ```
#[stable(feature = "resume_unwind", since = "1.9.0")]
pub fn resume_unwind(payload: Box<dyn Any + Send>) -> ! {
    panicking::resume_unwind(payload)
}

/// 使今后所有的 panic 都直接 abort，既不运行 panic 钩子，也不进行 unwind。
///
/// 这一设置无法撤销；其效果会一直持续到进程退出或 exec（或与之等价的操作）。
///
/// # 在 fork 之后使用
///
/// 本函数特别适合在 `libc::fork` 之后调用。在多线程程序中，`fork` 之后（在很多平台上）
/// 调用分配器是不安全的。此外，让一次 unwind 跨越 `fork` 继续传播通常也是极不可取的，
/// 因为这会让 unwind 传播到那些原本只期望在父进程中运行的代码里。
///
/// `panic::always_abort()` 有助于避免上述两种问题。它直接杜绝任何进一步的 unwind；
/// 并且当发生 panic 时，只要传给 panic 的参数能够在不分配内存的情况下完成格式化，
/// abort 就会在不分配内存的前提下发生。
///
/// Examples
///
/// ```no_run
/// #![feature(panic_always_abort)]
/// use std::panic;
///
/// panic::always_abort();
///
/// let _ = panic::catch_unwind(|| {
///     panic!("inside the catch");
/// });
///
/// // 由于发生了 panic，我们此时早已 abort。
/// unreachable!();
/// ```
#[unstable(feature = "panic_always_abort", issue = "84438")]
pub fn always_abort() {
    crate::panicking::panic_count::set_always_abort();
}

/// 用于配置默认 panic 钩子是否捕获 backtrace，以及如何显示它。
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[unstable(feature = "panic_backtrace_config", issue = "93346")]
#[non_exhaustive]
pub enum BacktraceStyle {
    /// 打印较为精简的 backtrace，理想情况下只包含相关信息。
    Short,
    /// 打印包含尽可能多信息的 backtrace。
    Full,
    /// 禁止收集与显示 backtrace。
    Off,
}

impl BacktraceStyle {
    pub(crate) fn full() -> Option<Self> {
        if cfg!(feature = "backtrace") { Some(BacktraceStyle::Full) } else { None }
    }

    fn as_u8(self) -> u8 {
        match self {
            BacktraceStyle::Short => 1,
            BacktraceStyle::Full => 2,
            BacktraceStyle::Off => 3,
        }
    }

    fn from_u8(s: u8) -> Option<Self> {
        match s {
            1 => Some(BacktraceStyle::Short),
            2 => Some(BacktraceStyle::Full),
            3 => Some(BacktraceStyle::Off),
            _ => None,
        }
    }
}

// 跟踪我们是否应该/能够捕获 backtrace，以及应当如何显示该 backtrace。
//
// 内部存储的是等价于 Option<BacktraceStyle> 的值。
static SHOULD_CAPTURE: Atomic<u8> = AtomicU8::new(0);

/// 配置默认 panic 钩子是否捕获并显示 backtrace。
///
/// 该设置的默认值可由 `RUST_BACKTRACE` 环境变量设定；详见 [`get_backtrace_style`]。
#[unstable(feature = "panic_backtrace_config", issue = "93346")]
pub fn set_backtrace_style(style: BacktraceStyle) {
    if cfg!(feature = "backtrace") {
        // 如果本 crate 启用了 `backtrace` feature，则设置 backtrace 样式。
        SHOULD_CAPTURE.store(style.as_u8(), Ordering::Relaxed);
    }
}

/// 检查标准库的 panic 钩子是否会捕获并打印 backtrace。
///
/// 如果尚未通过 [`set_backtrace_style`] 设置 backtrace 样式，本函数会读取环境
/// 变量 `RUST_BACKTRACE`，以确定 backtrace 格式化的默认值：
///
/// 在 `set_backtrace_style` 尚未被调用以覆盖默认值的前提下，第一次调用
/// `get_backtrace_style` 可能会读取 `RUST_BACKTRACE` 环境变量。在调用过一次
/// `set_backtrace_style` 或 `get_backtrace_style` 之后，对 `RUST_BACKTRACE` 的
/// 任何更改都将不再生效。
///
/// `RUST_BACKTRACE` 按以下规则读取：
///
/// * `0` 对应 `BacktraceStyle::Off`
/// * `full` 对应 `BacktraceStyle::Full`
/// * `1` 对应 `BacktraceStyle::Short`
/// * 其他值目前都对应 `BacktraceStyle::Short`，但这在未来可能改变
///
/// 如果当前不支持 backtrace，则返回 `None`。
#[unstable(feature = "panic_backtrace_config", issue = "93346")]
pub fn get_backtrace_style() -> Option<BacktraceStyle> {
    if !cfg!(feature = "backtrace") {
        // 如果本 crate 没有启用 `backtrace` feature，则快速返回 `Unsupported`，
        // 以便这一结果可以被常量传播到各处，从而优化掉相关调用方。
        return None;
    }

    let current = SHOULD_CAPTURE.load(Ordering::Relaxed);
    if let Some(style) = BacktraceStyle::from_u8(current) {
        return Some(style);
    }

    let format = match crate::env::var_os("RUST_BACKTRACE") {
        Some(x) if &x == "0" => BacktraceStyle::Off,
        Some(x) if &x == "full" => BacktraceStyle::Full,
        Some(_) => BacktraceStyle::Short,
        None if crate::sys::backtrace::FULL_BACKTRACE_DEFAULT => BacktraceStyle::Full,
        None => BacktraceStyle::Off,
    };

    match SHOULD_CAPTURE.compare_exchange(0, format.as_u8(), Ordering::Relaxed, Ordering::Relaxed) {
        Ok(_) => Some(format),
        Err(new) => BacktraceStyle::from_u8(new),
    }
}
