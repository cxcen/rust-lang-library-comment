//! 标准库的 panic 支撑接口。
//!
//! `core` 只定义 panic 宏、panic 元信息和 unwind-safety 标记等底层协议；真正的默认
//! panic hook、线程边界上的捕获、以及 unwind 运行时由 `std` 或目标平台的 panic runtime
//! 负责。在 `no_std` 程序中，用户通常通过 `#[panic_handler]` 接收 [`PanicInfo`]，并决定是
//! 记录信息、停机、重启，还是进入无限循环。

#![stable(feature = "core_panic_info", since = "1.41.0")]

mod location;
mod panic_info;
mod unwind_safe;

#[stable(feature = "panic_hooks", since = "1.10.0")]
pub use self::location::Location;
#[stable(feature = "panic_hooks", since = "1.10.0")]
pub use self::panic_info::PanicInfo;
#[stable(feature = "panic_info_message", since = "1.81.0")]
pub use self::panic_info::PanicMessage;
#[stable(feature = "catch_unwind", since = "1.9.0")]
pub use self::unwind_safe::{AssertUnwindSafe, RefUnwindSafe, UnwindSafe};
use crate::any::Any;

#[doc(hidden)]
#[unstable(feature = "edition_panic", issue = "none", reason = "use panic!() instead")]
#[allow_internal_unstable(panic_internals, const_format_args)]
#[rustc_diagnostic_item = "core_panic_2015_macro"]
#[rustc_macro_transparency = "semiopaque"]
pub macro panic_2015 {
    () => (
        $crate::panicking::panic("explicit panic")
    ),
    ($msg:literal $(,)?) => (
        $crate::panicking::panic($msg)
    ),
    // 为了配合 non_fmt_panic lint，对字符串表达式使用 `panic_str_2015`，
    // 而不是走 `panic_display::<&str>`。
    ($msg:expr $(,)?) => ({
        $crate::panicking::panic_str_2015($msg);
    }),
    // 单参数格式化形式为 const_panic 做特殊处理。
    ("{}", $arg:expr $(,)?) => ({
        $crate::panicking::panic_display(&$arg);
    }),
    ($fmt:expr, $($arg:tt)+) => ({
        // 分号用于阻止格式化机制内部的临时值在 `panic_fmt` 调用后
        // 被认为仍活在调用方作用域中。
        $crate::panicking::panic_fmt($crate::const_format_args!($fmt, $($arg)+));
    }),
}

#[doc(hidden)]
#[unstable(feature = "edition_panic", issue = "none", reason = "use panic!() instead")]
#[allow_internal_unstable(panic_internals, const_format_args)]
#[rustc_diagnostic_item = "core_panic_2021_macro"]
#[rustc_macro_transparency = "semiopaque"]
pub macro panic_2021 {
    () => (
        $crate::panicking::panic("explicit panic")
    ),
    // 单参数格式化形式为 const_panic 做特殊处理。
    ("{}", $arg:expr $(,)?) => ({
        $crate::panicking::panic_display(&$arg);
    }),
    ($($t:tt)+) => ({
        // 分号用于阻止格式化机制内部的临时值在 `panic_fmt` 调用后
        // 被认为仍活在调用方作用域中。
        $crate::panicking::panic_fmt($crate::const_format_args!($($t)+));
    }),
}

#[doc(hidden)]
#[unstable(feature = "edition_panic", issue = "none", reason = "use unreachable!() instead")]
#[allow_internal_unstable(panic_internals)]
#[rustc_diagnostic_item = "unreachable_2015_macro"]
#[rustc_macro_transparency = "semiopaque"]
pub macro unreachable_2015 {
    () => (
        $crate::panicking::panic("internal error: entered unreachable code")
    ),
    // 为 non_fmt_panic lint 使用 `unreachable_display`。
    // NOTE: 消息文本 ("internal error ...") 直接嵌入在 unreachable_display 中。
    ($msg:expr $(,)?) => ({
        $crate::panicking::unreachable_display(&$msg);
    }),
    ($fmt:expr, $($arg:tt)*) => (
        $crate::panic!($crate::concat!("internal error: entered unreachable code: ", $fmt), $($arg)*)
    ),
}

#[doc(hidden)]
#[unstable(feature = "edition_panic", issue = "none", reason = "use unreachable!() instead")]
#[allow_internal_unstable(panic_internals)]
#[rustc_macro_transparency = "semiopaque"]
pub macro unreachable_2021 {
    () => (
        $crate::panicking::panic("internal error: entered unreachable code")
    ),
    ($($t:tt)+) => (
        $crate::panic!("internal error: entered unreachable code: {}", $crate::format_args!($($t)+))
    ),
}

/// 调用闭包；如果该闭包尝试 unwind，则中止进程。
///
/// 在以 aborting panics 编译时，这个函数实质上不需要额外工作。在以 unwinding panics
/// 编译时，闭包发生 unwind 会导致再次进入 panic hook，然后执行进程 abort。这使调用点能
/// 明确建立“这里不允许栈展开跨出”的边界。
///
/// # 说明
///
/// 普通代码应优先尝试支持 unwinding，而不是直接使用此函数。实现 [`Drop`] 可以让类型在
/// 正常返回路径和 unwind 路径上统一恢复不变量。
///
/// 如果 unwind 只可能导致逻辑正确性问题，而不会造成健全性问题，应允许 unwind 继续发生。
/// 不实现 [`UnwindSafe`] 是给调用者的提示：跨 `catch_unwind` 边界观察该值时，需要额外考虑
/// panic 中断后的逻辑不变量。
///
/// 如果 unwind 会让程序不健全，则应使用此函数阻止 unwind 穿过该边界。需要注意的是，
/// `extern "C" fn` 已经会自动把 unwind 转成 abort，因此 FFI 边界通常不必再额外包一层此函数。
#[unstable(feature = "abort_unwind", issue = "130338")]
#[rustc_nounwind]
pub fn abort_unwind<F: FnOnce() -> R, R>(f: F) -> R {
    f()
}

/// `std` 内部用来把数据传给 `panic_unwind` 和其他 panic runtime 的 trait。
///
/// 这个 trait 描述的是运行时之间的内部载荷传递协议，不打算在短期内稳定；库代码不应依赖它。
#[unstable(feature = "std_internals", issue = "none")]
#[doc(hidden)]
pub unsafe trait PanicPayload: crate::fmt::Display {
    /// 取得内容的完整所有权。
    /// 返回类型实际表示 `Box<dyn Any + Send>`，但 `core` 中不能使用 `Box`。
    ///
    /// 调用此方法后，`self` 中只留下某个占位默认值。重复调用此方法，或者在调用后再调用
    /// `get`，都违反该内部协议。
    ///
    /// 参数使用借用形式，是因为 panic runtime (`__rust_start_panic`) 只能拿到借用的
    /// `dyn PanicPayload`。
    fn take_box(&mut self) -> *mut (dyn Any + Send);

    /// 只借用 panic 载荷内容，不转移所有权。
    fn get(&mut self) -> &(dyn Any + Send);

    /// 在不需要分配的前提下，尝试把载荷内容借用为 `&str`。
    fn as_str(&mut self) -> Option<&str> {
        None
    }
}

/// 在 `const fn` 中触发 panic 的辅助宏。
///
/// 调用形式如下：
/// ```rust,ignore (只是示例)
/// core::macros::const_panic!("boring message", "flavored message {a} {b:?}", a: u32 = foo.len(), b: Something = bar);
/// ```
/// 其中第一条消息会在 const-eval 中打印，第二条消息会在运行期打印。这样可以让 const 诊断
/// 保持简单，同时在运行期保留带参数的完整格式化信息。
// 此宏的所有使用位置都是 FIXME(const-hack)。
#[unstable(feature = "panic_internals", issue = "none")]
#[doc(hidden)]
pub macro const_panic {
    ($const_msg:literal, $runtime_msg:literal, $($arg:ident : $ty:ty = $val:expr),* $(,)?) => {{
        // 把 `const_eval_select` 调用包进函数中，方便添加
        // `rustc_allow_const_fn_unstable`。这样做是可接受的，因为两个分支都会 panic，
        // 只是消息不同。
        #[rustc_allow_const_fn_unstable(const_eval_select)]
        #[inline(always)] // 内联这个包装函数
        #[track_caller]
        const fn do_panic($($arg: $ty),*) -> ! {
            $crate::intrinsics::const_eval_select!(
                @capture { $($arg: $ty = $arg),* } -> !:
                if const #[track_caller] {
                    $crate::panic!($const_msg)
                } else #[track_caller] {
                    $crate::panic!($runtime_msg)
                }
            )
        }

        do_panic($($val),*)
    }},
    // 支持为 *所有* 参数省略 `val` 表达式；
    // 但不支持只给 *部分* 参数省略，因为那会让宏匹配过于复杂。
    ($const_msg:literal, $runtime_msg:literal, $($arg:ident : $ty:ty),* $(,)?) => {
        $crate::panic::const_panic!(
            $const_msg,
            $runtime_msg,
            $($arg: $ty = $arg),*
        )
    },
}

/// `assert` 的 const 版本，在 const 上下文中打印非格式化消息。
///
/// 参见 [`const_panic!`]。
#[unstable(feature = "panic_internals", issue = "none")]
#[doc(hidden)]
pub macro const_assert {
    ($condition: expr, $const_msg:literal, $runtime_msg:literal, $($arg:tt)*) => {{
        if !($condition) {
            $crate::panic::const_panic!($const_msg, $runtime_msg, $($arg)*)
        }
    }}
}
