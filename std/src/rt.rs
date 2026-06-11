//! 运行时服务（Runtime services）
//!
//! `rt` 模块提供一组狭窄的运行时服务，包括全局堆（在 `heap` 中导出）以及
//! unwinding（栈展开）和 backtrace（回溯）支持。本模块中的 API 高度不稳定，
//! 目前应当被视为私有实现细节。
//!
//! 设计背景：`std` 构建在 `core`/`alloc` 之上，额外提供 OS 抽象与运行时。本模块
//! 正是“运行时”的落脚点——它负责在用户的 `fn main()` 执行前后做一次性的初始化和
//! 清理（设置 `SIGPIPE`、记录主线程 ID、安装栈溢出保护、刷新并关闭 stdio 缓冲等），
//! 并通过 `#[lang = "start"]` 入口被编译器生成的代码调用。这里的几乎所有内容都不属于
//! 稳定的公开契约，编译器与 `panic_unwind`/`panic_abort` 运行时是它真正的依赖方。

#![unstable(
    feature = "rt",
    reason = "this public module should not exist and is highly likely \
              to disappear",
    issue = "none"
)]
#![doc(hidden)]
#![deny(unsafe_op_in_unsafe_fn)]
#![allow(unused_macros)]

#[rustfmt::skip]
pub use crate::panicking::{begin_panic, panic_count};
pub use core::panicking::{panic_display, panic_fmt};

#[rustfmt::skip]
use crate::any::Any;
use crate::sync::Once;
use crate::thread::{self, main_thread};
use crate::{mem, panic, sys};

// 此函数为 panic 运行时（panic runtime）所需。
#[cfg(not(test))]
#[rustc_std_internal_symbol]
fn __rust_abort() {
    crate::process::abort();
}

// 打印到“panic 输出（panic output）”，依平台不同它可能是：
// - 标准错误输出（standard error output）
// - 某些平台专属的输出
// - 什么都没有（此时该宏是个 no-op，即空操作）
macro_rules! rtprintpanic {
    ($($t:tt)*) => {
        #[cfg(not(panic = "immediate-abort"))]
        if let Some(mut out) = crate::sys::stdio::panic_output() {
            let _ = crate::io::Write::write_fmt(&mut out, format_args!($($t)*));
        }
        #[cfg(panic = "immediate-abort")]
        {
            let _ = format_args!($($t)*);
        }
    }
}

macro_rules! rtabort {
    ($($t:tt)*) => {
        {
            rtprintpanic!("fatal runtime error: {}, aborting\n", format_args!($($t)*));
            crate::process::abort();
        }
    }
}

macro_rules! rtassert {
    ($e:expr) => {
        if !$e {
            rtabort!(concat!("assertion failed: ", stringify!($e)));
        }
    };
}

macro_rules! rtunwrap {
    ($ok:ident, $e:expr) => {
        match $e {
            $ok(v) => v,
            ref err => {
                let err = err.as_ref().map(drop); // 把 Ok/Some 映射掉，因为它们的内容可能没有实现 Debug
                rtabort!(concat!("unwrap failed: ", stringify!($e), " = {:?}"), err)
            }
        }
    };
}

fn handle_rt_panic<T>(e: Box<dyn Any + Send>) -> T {
    mem::forget(e);
    rtabort!("initialization or cleanup bug");
}

// 一次性的运行时初始化（One-time runtime initialization）。
// 在 `main` 之前运行。
// SAFETY: 必须在运行时初始化期间且仅调用一次。
// NOTE: 不保证一定会运行，例如当 Rust 代码被外部（其它语言）调用时就不会运行。
//
// # `sigpipe` 参数
//
// 自 2014 年起，Unix 上的 Rust 运行时就把 `SIGPIPE` 处理器（handler）设置为
// `SIG_IGN`。但应用程序有充分理由希望使用不同的行为，因此存在一个
// `-Zon-broken-pipe` 编译器标志，可用于选择在 `fn main()` 被调用前如何设置
// `SIGPIPE`（以及是否更改它）。更多信息参见
// <https://github.com/rust-lang/rust/issues/97889>。
//
// 本函数的 `sigpipe` 参数通过 rustc 生成的、用于调用 `fn lang_start()` 的代码取得其值。
// 我们之所以在所有平台（而不仅是 Unix）上都保留 `sigpipe`，是因为在这一高层
// 不允许 std 出现 `cfg` 指令。详见 `src/tools/tidy/src/pal.rs` 中的模块文档。
// 在其它所有平台上，`sigpipe` 也有取值，但其值会被忽略。
//
// 尽管它是 `u8`，但其取值始终只有 4 种。它们记录在
// `compiler/rustc_session/src/config/sigpipe.rs` 中。
#[cfg_attr(test, allow(dead_code))]
unsafe fn init(argc: isize, argv: *const *const u8, sigpipe: u8) {
    // 记住主线程的 ID，以便给它正确的名字。
    // SAFETY: 这是我们调用此函数的唯一一次、唯一一处。
    unsafe { main_thread::set(thread::current_id()) };

    #[cfg_attr(target_os = "teeos", allow(unused_unsafe))]
    unsafe {
        sys::init(argc, argv, sigpipe)
    };
}

/// 清理线程局部（thread-local）的运行时状态。它*应当*在所有其它由 Rust 运行时
/// 管理的代码之后运行，但即使不满足该条件也不会导致 UB（未定义行为）。另外注意，
/// 本函数不保证一定会被运行，但跳过它会造成内存泄漏，因此应当避免跳过。
pub(crate) fn thread_cleanup() {
    // 本函数会在“展开（unwinding）将导致 abort”的场景下运行（想想 `extern "C"`
    // 函数）。这里直接 abort，以便我们能打印一条友好的消息。
    panic::catch_unwind(|| {
        crate::thread::drop_current();
    })
    .unwrap_or_else(handle_rt_panic);
}

// 一次性的运行时清理（One-time runtime cleanup）。
// 在 `main` 之后或程序退出时运行。
// NOTE: 不保证一定会运行，例如当程序 abort 时就不会运行。
pub(crate) fn cleanup() {
    static CLEANUP: Once = Once::new();
    CLEANUP.call_once(|| unsafe {
        // 刷新 stdout 并禁用其缓冲。
        crate::io::cleanup();
        // SAFETY: 在运行时清理期间仅被调用一次。
        sys::cleanup();
    });
}

// 为减少新版 `lang_start` 生成的代码量，由本函数来完成真正的工作。
#[cfg(not(test))]
fn lang_start_internal(
    main: &(dyn Fn() -> i32 + Sync + crate::panic::RefUnwindSafe),
    argc: isize,
    argv: *const *const u8,
    sigpipe: u8,
) -> isize {
    // 防止本函数所调用的代码展开（unwind）到 Rust 受控代码之外——那样做属于 UB。
    // 这一要求源于 `#[lang="start"]` 属性的实现方式与 panic 机制本身实现方式的共同作用。
    //
    // 有几处地方可能开始展开。第一处是在 `rt::init`、`rt::cleanup` 等由 std 控制的
    // 函数内部。在这些地方发生 panic 属于 std 的实现 bug，而且这种 bug 相当可能出现，
    // 因为没有任何手段能阻止 std 在这些函数中意外引入 panic。另一处来自 `main` 中的
    // 用户代码，或者更隐蔽地，如 issue #86030 中所描述的情形。
    //
    // 我们使用 `catch_unwind` 搭配 `handle_rt_panic`（而非 `abort_unwind`），是为了
    // 在发生 panic 时让报错信息更友好一些。
    panic::catch_unwind(move || {
        // SAFETY: 在运行时初始化期间仅被调用一次。
        unsafe { init(argc, argv, sigpipe) };

        let ret_code = panic::catch_unwind(main).unwrap_or_else(move |payload| {
            // 小心地处置 panic 载荷（payload）。
            let payload = panic::AssertUnwindSafe(payload);
            panic::catch_unwind(move || drop({ payload }.0)).unwrap_or_else(move |e| {
                mem::forget(e); // *不要* drop 第二个 payload
                rtabort!("drop of the panic payload panicked");
            });
            // 为发生 panic 的程序返回错误码。
            101
        });
        let ret_code = ret_code as isize;

        cleanup();
        // 防止多个线程并发调用 `libc::exit`。
        // 更多信息参见 `unique_thread_exit` 的文档。
        crate::sys::exit_guard::unique_thread_exit();

        ret_code
    })
    .unwrap_or_else(handle_rt_panic)
}

#[cfg(not(any(test, doctest)))]
#[lang = "start"]
fn lang_start<T: crate::process::Termination + 'static>(
    main: fn() -> T,
    argc: isize,
    argv: *const *const u8,
    sigpipe: u8,
) -> isize {
    lang_start_internal(
        &move || crate::sys::backtrace::__rust_begin_short_backtrace(main).report().to_i32(),
        argc,
        argv,
        sigpipe,
    )
}
