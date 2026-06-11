//! `panic!` 宏及其相关运行时部件的各种实现细节。
//!
//! 具体来说，本模块包含以下内容的实现：
//!
//! * panic 钩子（hook）
//! * 执行一次 panic，直到进入真正的实现为止
//! * 围绕 “try” 的各种 shim（垫片）
//!
//! 维护的核心不变量与依赖关系：
//! - 本模块通过 `#[panic_handler]`（`panic_handler` 函数）与 `core` crate 的 panic 入口对接：
//!   `core::panicking` 把 panic 转发到这里，再由这里走 hook -> unwind/abort 的流程。
//! - panic 计数（见 `panic_count`）维护着“当前线程是否正在 panic”这一信息，
//!   [`panicking`] 函数据此判断；它还负责识别递归 panic（double panic）并强制 abort。
//! - 是否 unwind 还是 abort 的决策集中在 `panic_with_hook`：根据 `can_unwind`、
//!   `always_abort`、以及 hook 内是否再次 panic 来决定。
//! - 真正发起 unwind 的工作交给外部 panic 运行时（通过 `__rust_start_panic` 等
//!   `rustc_std_internal_symbol` 符号对接，见下方 `extern` 块）。
//! - 失败暴露：无法发起 panic、不可 unwind 场景下仍尝试 unwind、或 hook 内再次 panic，
//!   都会通过 `rtabort!` / `process::abort()` 令进程直接终止。

#![deny(unsafe_op_in_unsafe_fn)]

use core::panic::{Location, PanicPayload};

// 确保使用由 libtest 在真正的那份 std 拷贝中配置好的 stderr 输出
#[cfg(test)]
use realstd::io::try_set_output_capture;

use crate::any::Any;
#[cfg(not(test))]
use crate::io::try_set_output_capture;
use crate::mem::{self, ManuallyDrop};
use crate::panic::{BacktraceStyle, PanicHookInfo};
use crate::sync::atomic::{Atomic, AtomicBool, Ordering};
use crate::sync::nonpoison::RwLock;
use crate::sys::backtrace;
use crate::sys::stdio::panic_output;
use crate::{fmt, intrinsics, process, thread};

// 确保由 panic!() 调用的函数在 std crate 内部完成代码生成（codegen），而不是在下游
// crate 中。这主要对 rustc 的 codegen 测试有用，那些测试依赖于“能注意到 panic 已被
// 从生成的 IR 中完全移除”。由于 begin_panic 标注了 inline(never)，它在整个 crate 图中
// 只会被 codegen 一次，因此这样做能把这次 codegen 推到 std 中，而非我们的 codegen 测试
// crate 中。
//
// （关于为何如此，更多信息见 https://github.com/rust-lang/rust/pull/123244）。
//
// 如果这会造成问题，我们也可以改用诸如 cdylib 之类的 crate type 来修改那些 codegen
// 测试——cdylib 不会向下游链接单元导出 "Rust" 符号。
#[unstable(feature = "libstd_sys_internals", reason = "used by the panic! macro", issue = "none")]
#[doc(hidden)]
#[allow(dead_code)]
#[used(compiler)]
pub static EMPTY_PANIC: fn(&'static str) -> ! =
    begin_panic::<&'static str> as fn(&'static str) -> !;

// 与标准库所依赖的 panic 运行时之间的二进制接口（binary interface）。
//
// 标准库被标记了 `#![needs_panic_runtime]`（在 RFC 1513 中引入），表明它需要
// 在某处存在另一个标记了 `#![panic_runtime]` 的 crate。每个 panic 运行时都应当
// 实现这些符号（且签名一致），这样我们才能与它们匹配对接。
//
// 也许有朝一日编译器会帮忙把这些函数挂接起来，使这一切不再这么临时拼凑（ad-hoc），
// 但那一天还没到来！
#[allow(improper_ctypes)]
unsafe extern "C" {
    #[rustc_std_internal_symbol]
    fn __rust_panic_cleanup(payload: *mut u8) -> *mut (dyn Any + Send + 'static);
}

unsafe extern "Rust" {
    /// `PanicPayload` 仅在需要时才惰性进行分配（这样在使用 "abort" panic 运行时
    /// 时就能避免分配内存）。
    #[rustc_std_internal_symbol]
    fn __rust_start_panic(payload: &mut dyn PanicPayload) -> u32;
}

/// 当 FFI 代码捕获了一个 Rust panic 却没有重新抛出它时，panic 运行时会调用本函数。
/// 我们不支持这种情况，因为它会扰乱我们的 panic 计数。
#[cfg(not(test))]
#[rustc_std_internal_symbol]
extern "C" fn __rust_drop_panic() -> ! {
    rtabort!("Rust panics must be rethrown");
}

/// 当 panic 运行时捕获到一个并不对应于 Rust panic 的异常对象时，会调用本函数。
#[cfg(not(test))]
#[rustc_std_internal_symbol]
extern "C" fn __rust_foreign_exception() -> ! {
    rtabort!("Rust cannot catch foreign exceptions");
}

#[derive(Default)]
enum Hook {
    #[default]
    Default,
    Custom(Box<dyn Fn(&PanicHookInfo<'_>) + 'static + Sync + Send>),
}

impl Hook {
    #[inline]
    fn into_box(self) -> Box<dyn Fn(&PanicHookInfo<'_>) + 'static + Sync + Send> {
        match self {
            Hook::Default => Box::new(default_hook),
            Hook::Custom(hook) => hook,
        }
    }
}

static HOOK: RwLock<Hook> = RwLock::new(Hook::Default);

/// 注册一个自定义 panic 钩子，替换先前注册的钩子。
///
/// panic 钩子会在某个线程 panic 时、但在调用 panic 运行时之前被触发。因此，
/// 无论使用的是 abort 还是 unwind 运行时，该钩子都会运行。
///
/// 默认钩子在启动时注册，它会向标准错误打印一条消息，并在被请求时生成 backtrace。
/// 可以使用 `set_hook` 函数自定义这一行为。可以用 [`take_hook`] 函数在恢复默认钩子
/// 的同时取出当前的钩子。
///
/// [`take_hook`]: ./fn.take_hook.html
///
/// 钩子会收到一个 `PanicHookInfo` 结构体，其中包含关于 panic 来源的信息，包括传给
/// `panic!` 的 payload 以及 panic 起源处的源码位置。
///
/// panic 钩子是一项全局资源。
///
/// # Panics
///
/// 如果从一个正在 panic 的线程中调用，则会 panic。
///
/// # 示例
///
/// 下面这段代码会打印 "Custom panic hook"：
///
/// ```should_panic
/// use std::panic;
///
/// panic::set_hook(Box::new(|_| {
///     println!("Custom panic hook");
/// }));
///
/// panic!("Normal panic");
/// ```
#[stable(feature = "panic_hooks", since = "1.10.0")]
pub fn set_hook(hook: Box<dyn Fn(&PanicHookInfo<'_>) + 'static + Sync + Send>) {
    if thread::panicking() {
        panic!("cannot modify the panic hook from a panicking thread");
    }

    // 在更换钩子之后再 drop 旧钩子，以避免在其析构函数 panic 时发生死锁。
    drop(HOOK.replace(Hook::Custom(hook)));
}

/// 注销当前的 panic 钩子并将其返回，同时在原位注册默认钩子。
///
/// *另请参阅函数 [`set_hook`]。*
///
/// [`set_hook`]: ./fn.set_hook.html
///
/// 如果当前注册的是默认钩子，则返回它，但它仍保持注册状态。
///
/// # Panics
///
/// 如果从一个正在 panic 的线程中调用，则会 panic。
///
/// # 示例
///
/// 下面这段代码会打印 "Normal panic"：
///
/// ```should_panic
/// use std::panic;
///
/// panic::set_hook(Box::new(|_| {
///     println!("Custom panic hook");
/// }));
///
/// let _ = panic::take_hook();
///
/// panic!("Normal panic");
/// ```
#[must_use]
#[stable(feature = "panic_hooks", since = "1.10.0")]
pub fn take_hook() -> Box<dyn Fn(&PanicHookInfo<'_>) + 'static + Sync + Send> {
    if thread::panicking() {
        panic!("cannot modify the panic hook from a panicking thread");
    }

    HOOK.replace(Hook::Default).into_box()
}

/// [`take_hook`] 与 [`set_hook`] 的原子组合。用它来把 panic 处理程序替换为一个
/// 新的处理程序——该新处理程序先做某些事情，然后再执行旧的处理程序。
///
/// [`take_hook`]: ./fn.take_hook.html
/// [`set_hook`]: ./fn.set_hook.html
///
/// # Panics
///
/// 如果从一个正在 panic 的线程中调用，则会 panic。
///
/// # 示例
///
/// 下面这段代码会先打印自定义消息，然后再打印 panic 的正常输出。
///
/// ```should_panic
/// #![feature(panic_update_hook)]
/// use std::panic;
///
/// // 等价于
/// // let prev = panic::take_hook();
/// // panic::set_hook(Box::new(move |info| {
/// //     println!("...");
/// //     prev(info);
/// // }));
/// panic::update_hook(move |prev, info| {
///     println!("Print custom message and execute panic handler as usual");
///     prev(info);
/// });
///
/// panic!("Custom and then normal");
/// ```
#[unstable(feature = "panic_update_hook", issue = "92649")]
pub fn update_hook<F>(hook_fn: F)
where
    F: Fn(&(dyn Fn(&PanicHookInfo<'_>) + Send + Sync + 'static), &PanicHookInfo<'_>)
        + Sync
        + Send
        + 'static,
{
    if thread::panicking() {
        panic!("cannot modify the panic hook from a panicking thread");
    }

    let mut hook = HOOK.write();
    let prev = mem::take(&mut *hook).into_box();
    *hook = Hook::Custom(Box::new(move |info| hook_fn(&prev, info)));
}

/// 默认的 panic 处理程序。
#[optimize(size)]
fn default_hook(info: &PanicHookInfo<'_>) {
    // 如果这是一次 double panic（panic 套 panic），就确保为本次 panic 打印 backtrace。
    // 否则只有在启用了日志时才打印它。
    let backtrace = if info.force_no_backtrace() {
        None
    } else if panic_count::get_count() >= 2 {
        BacktraceStyle::full()
    } else {
        crate::panic::get_backtrace_style()
    };

    // 当前实现总是返回 `Some`。
    let location = info.location().unwrap();

    let msg = payload_as_str(info.payload());

    let write = #[optimize(size)]
    |err: &mut dyn crate::io::Write| {
        // 使用锁来防止多线程环境下输出相互混杂。
        // 某些平台在打印 backtrace 时也需要它，比如 Windows 上的 `SymFromAddr`。
        let mut lock = backtrace::lock();

        thread::with_current_name(|name| {
            let name = name.unwrap_or("<unnamed>");
            let tid = thread::current_os_id();

            // 先尝试把 panic 消息写入一个缓冲区，以防止其他并发输出与之交错。
            let mut buffer = [0u8; 512];
            let mut cursor = crate::io::Cursor::new(&mut buffer[..]);

            let write_msg = |dst: &mut dyn crate::io::Write| {
                // 我们加一个换行符，以确保 panic 消息出现在某一行的开头。
                writeln!(dst, "\nthread '{name}' ({tid}) panicked at {location}:\n{msg}")
            };

            if write_msg(&mut cursor).is_ok() {
                let pos = cursor.position() as usize;
                let _ = err.write_all(&buffer[0..pos]);
            } else {
                // 消息没能装进缓冲区，那就直接把它写出去。
                let _ = write_msg(err);
            };
        });

        static FIRST_PANIC: Atomic<bool> = AtomicBool::new(true);

        match backtrace {
            Some(BacktraceStyle::Short) => {
                drop(lock.print(err, crate::backtrace_rs::PrintFmt::Short))
            }
            Some(BacktraceStyle::Full) => {
                drop(lock.print(err, crate::backtrace_rs::PrintFmt::Full))
            }
            Some(BacktraceStyle::Off) => {
                if FIRST_PANIC.swap(false, Ordering::Relaxed) {
                    let _ = writeln!(
                        err,
                        "note: run with `RUST_BACKTRACE=1` environment variable to display a \
                             backtrace"
                    );
                    if cfg!(miri) {
                        let _ = writeln!(
                            err,
                            "note: in Miri, you may have to set `MIRIFLAGS=-Zmiri-env-forward=RUST_BACKTRACE` \
                                for the environment variable to have an effect"
                        );
                    }
                }
            }
            // 如果不支持 backtrace，或者 backtrace 被强制关闭，则什么也不做。
            None => {}
        }
    };

    if let Ok(Some(local)) = try_set_output_capture(None) {
        write(&mut *local.lock().unwrap_or_else(|e| e.into_inner()));
        try_set_output_capture(Some(local)).ok();
    } else if let Some(mut out) = panic_output() {
        write(&mut out);
    }
}

#[cfg(not(test))]
#[doc(hidden)]
#[cfg(panic = "immediate-abort")]
#[unstable(feature = "update_panic_count", issue = "none")]
pub mod panic_count {
    /// 强制立即 abort 的原因。
    #[derive(Debug)]
    pub enum MustAbort {
        AlwaysAbort,
        PanicInHook,
    }

    #[inline]
    pub fn increase(run_panic_hook: bool) -> Option<MustAbort> {
        None
    }

    #[inline]
    pub fn finished_panic_hook() {}

    #[inline]
    pub fn decrease() {}

    #[inline]
    pub fn set_always_abort() {}

    // 忽略 ALWAYS_ABORT_FLAG
    #[inline]
    #[must_use]
    pub fn get_count() -> usize {
        0
    }

    #[must_use]
    #[inline]
    pub fn count_is_zero() -> bool {
        true
    }
}

#[cfg(not(test))]
#[doc(hidden)]
#[cfg(not(panic = "immediate-abort"))]
#[unstable(feature = "update_panic_count", issue = "none")]
pub mod panic_count {
    use crate::cell::Cell;
    use crate::sync::atomic::{Atomic, AtomicUsize, Ordering};

    const ALWAYS_ABORT_FLAG: usize = 1 << (usize::BITS - 1);

    /// 强制立即 abort 的原因。
    #[derive(Debug)]
    pub enum MustAbort {
        AlwaysAbort,
        PanicInHook,
    }

    // 当前线程的 panic 计数，以及当前是否正在执行某个 panic 钩子。
    thread_local! {
        static LOCAL_PANIC_COUNT: Cell<(usize, bool)> = const { Cell::new((0, false)) }
    }

    // 所有线程 panic 计数之和。它的用途是给 `count_is_zero`（被 `panicking` 使用）提供
    // 一条快速路径。对任意某个线程而言，如果该线程当前看到的 `GLOBAL_PANIC_COUNT` 为零，
    // 那么该线程中的 `LOCAL_PANIC_COUNT` 也为零。这一不变量在 increase 与 decrease 执行
    // 前后都成立，但在它们执行过程中不一定成立。
    //
    // 此外，GLOBAL_PANIC_COUNT 的最高位（GLOBAL_ALWAYS_ABORT_FLAG）记录 panic::always_abort()
    // 是否已被调用。该位只能被置位，永远不会被清除。
    // panic::always_abort() 通常被用来防止在 `libc::fork` 创建的子进程中、panic 处理过程
    // 所做的内存分配。
    // 在用 `libc::fork` 创建的子进程中执行内存分配，在大多数操作系统上都是未定义行为。
    // 在 `libc::fork` 创建的子进程中访问 LOCAL_PANIC_COUNT 会导致一次内存分配。在这种
    // 情形下只能访问 GLOBAL_PANIC_COUNT。这已经足够，因为子进程总是恰好只有一个线程。
    // 详情另见 #85261。
    //
    // 它可以被看作一个包含“一个 bit”和“一个 n-1 位的值”的结构体，但如果真这么写，
    // 它会超过一个字（word）的大小；而即便是围绕 usize 的 newtype 也会很笨拙，因为我们
    // 需要原子操作。不过我们确实在 increase() 的返回类型中使用了这样的元组。
    //
    // “偷用”一个 bit 是没问题的，因为这相当于假设每个正在 panic 的线程至少占用 2 字节的
    // 地址空间。
    static GLOBAL_PANIC_COUNT: Atomic<usize> = AtomicUsize::new(0);

    // 增加全局和本地的 panic 计数，并返回是否需要立即 abort。
    //
    // 它还会更新线程局部状态，以跟踪当前是否有某个 panic 钩子正在执行。
    pub fn increase(run_panic_hook: bool) -> Option<MustAbort> {
        let global_count = GLOBAL_PANIC_COUNT.fetch_add(1, Ordering::Relaxed);
        if global_count & ALWAYS_ABORT_FLAG != 0 {
            // *不要* 访问线程局部状态，我们此刻可能正处于一次 `fork` 之后。
            return Some(MustAbort::AlwaysAbort);
        }

        LOCAL_PANIC_COUNT.with(|c| {
            let (count, in_panic_hook) = c.get();
            if in_panic_hook {
                return Some(MustAbort::PanicInHook);
            }
            c.set((count + 1, run_panic_hook));
            None
        })
    }

    pub fn finished_panic_hook() {
        LOCAL_PANIC_COUNT.with(|c| {
            let (count, _) = c.get();
            c.set((count, false));
        });
    }

    pub fn decrease() {
        GLOBAL_PANIC_COUNT.fetch_sub(1, Ordering::Relaxed);
        LOCAL_PANIC_COUNT.with(|c| {
            let (count, _) = c.get();
            c.set((count - 1, false));
        });
    }

    pub fn set_always_abort() {
        GLOBAL_PANIC_COUNT.fetch_or(ALWAYS_ABORT_FLAG, Ordering::Relaxed);
    }

    // 忽略 ALWAYS_ABORT_FLAG
    #[must_use]
    pub fn get_count() -> usize {
        LOCAL_PANIC_COUNT.with(|c| c.get().0)
    }

    // 忽略 ALWAYS_ABORT_FLAG
    #[must_use]
    #[inline]
    pub fn count_is_zero() -> bool {
        if GLOBAL_PANIC_COUNT.load(Ordering::Relaxed) & !ALWAYS_ABORT_FLAG == 0 {
            // 快速路径：如果 `GLOBAL_PANIC_COUNT` 为零，那么所有线程（包括当前线程）的
            // `LOCAL_PANIC_COUNT` 都将等于零，于是可以避免 TLS 访问。
            //
            // 就性能而言，一次 relaxed 原子加载与一次普通的对齐内存读取（例如 x86 上的
            // mov 指令）相近，只是带有一些编译器优化方面的限制。而另一方面，一次 TLS 访问
            // 可能需要调用一个不可内联的函数（例如使用 GD TLS 模型时的 `__tls_get_addr`）。
            true
        } else {
            is_zero_slow_path()
        }
    }

    // 慢速路径放在一个单独的函数里，以减少从 `count_is_zero` 内联进来的代码量。
    #[inline(never)]
    #[cold]
    fn is_zero_slow_path() -> bool {
        LOCAL_PANIC_COUNT.with(|c| c.get().0 == 0)
    }
}

#[cfg(test)]
pub use realstd::rt::panic_count;

/// 调用一个闭包，并在其因 panic 而 unwind 时捕获导致 unwind 的原因。
#[cfg(panic = "immediate-abort")]
pub unsafe fn catch_unwind<R, F: FnOnce() -> R>(f: F) -> Result<R, Box<dyn Any + Send>> {
    Ok(f())
}

/// 调用一个闭包，并在其因 panic 而 unwind 时捕获导致 unwind 的原因。
#[cfg(not(panic = "immediate-abort"))]
pub unsafe fn catch_unwind<R, F: FnOnce() -> R>(f: F) -> Result<R, Box<dyn Any + Send>> {
    union Data<F, R> {
        f: ManuallyDrop<F>,
        r: ManuallyDrop<R>,
        p: ManuallyDrop<Box<dyn Any + Send>>,
    }

    // 出于性能考虑，我们在这里对所有权做了一些“取巧”的操作。我们只能把指针传给
    // `do_call`（无法按值传递对象），所以我们在这里用一个 union 手动完成全部的
    // 所有权跟踪。
    //
    // 我们会经历这样一个状态转移过程：
    //
    // * 首先，把 data 的 `f` 字段设为我们将要调用的那个无参闭包。
    // * 当我们发起函数调用时（下面的 `do_call` 函数），它会取得该函数指针的所有权。
    //   此时 `data` union 完全处于未初始化状态。
    // * 如果闭包成功返回，我们把返回值写入 data 的返回值槽（字段 `r`）。
    // * 如果闭包 panic（下面的 `do_catch`），我们把 panic payload 写入字段 `p`。
    // * 最后，当我们从 `try` 内建（intrinsic）返回时，会处于以下两种状态之一：
    //
    //      1. 闭包没有 panic，此时返回值已被填好。我们把它从 `data.r` 中移出并返回。
    //      2. 闭包 panic 了，此时 panic payload 已被填好。我们把它从 `data.p` 中移出并返回。
    //
    // 把上面这些组合到一起，我们就得到了在捕获 panic 的同时兼顾所有权管理的“最高效”
    // 的方法。
    let mut data = Data { f: ManuallyDrop::new(f) };

    let data_ptr = (&raw mut data) as *mut u8;
    // SAFETY:
    //
    // 对 union 各字段的访问：这里是 `std`，我们知道 `catch_unwind` 内建会根据其返回值
    // 来填充 union 的 `r` 或 `p` 字段。
    //
    // 对 `intrinsics::catch_unwind` 的调用之所以安全，是因为：
    // - 第一个参数 `do_call` 可以用最初的 `data_ptr` 来调用。
    // - 第二个参数 `do_catch` 同样可以用 `data_ptr` 来调用。
    // 更多信息见它们各自的安全前置条件。
    unsafe {
        return if intrinsics::catch_unwind(do_call::<F, R>, data_ptr, do_catch::<F, R>) == 0 {
            Ok(ManuallyDrop::into_inner(data.r))
        } else {
            Err(ManuallyDrop::into_inner(data.p))
        };
    }

    // 我们认为 unwind 是少见情形，因此把本函数标记为 cold。不过不把它标记为 no-inline——
    // 那个决定最好留给优化器（在大多数情况下，即便作为普通的、非 cold 的函数，本函数也
    // 不会被内联——截至撰写本注释时如此）。
    #[cold]
    #[optimize(size)]
    unsafe fn cleanup(payload: *mut u8) -> Box<dyn Any + Send + 'static> {
        // SAFETY: 整个 unsafe 块的成立都取决于 panic 处理程序 `__rust_panic_cleanup`
        // 被正确实现。因此我们只能假定它返回的东西是正确的，从而 `Box::from_raw`
        // 可以在不引发未定义行为的前提下工作。
        let obj = unsafe { Box::from_raw(__rust_panic_cleanup(payload)) };
        panic_count::decrease();
        obj
    }

    // SAFETY:
    // data 必须非 NUL、正确对齐，并且是一个指向 `Data<F, R>` 的指针。
    // 它必须包含一个有效的 `f`（类型为 F）值，可用来填充 `data.r`。
    //
    // 本函数不能被标记为 `unsafe`，因为 `intrinsics::catch_unwind` 期望的是普通函数指针。
    #[inline]
    fn do_call<F: FnOnce() -> R, R>(data: *mut u8) {
        // SAFETY: 这是调用方的责任，见上文。
        unsafe {
            let data = data as *mut Data<F, R>;
            let data = &mut (*data);
            let f = ManuallyDrop::take(&mut data.f);
            data.r = ManuallyDrop::new(f());
        }
    }

    // 我们 *确实* 希望 catch 的这一部分被内联：这能让编译器恰当地跟踪对 Data union
    // 的访问，并在大多数情况下把它优化掉。
    //
    // SAFETY:
    // data 必须非 NUL、正确对齐，并且是一个指向 `Data<F, R>` 的指针。
    // 由于它使用了 `cleanup`，所以也取决于 `__rustc_panic_cleanup` 被正确实现。
    //
    // 本函数不能被标记为 `unsafe`，因为 `intrinsics::catch_unwind` 期望的是普通函数指针。
    #[inline]
    #[rustc_nounwind] // `intrinsic::catch_unwind` 要求 catch 函数必须是 nounwind
    fn do_catch<F: FnOnce() -> R, R>(data: *mut u8, payload: *mut u8) {
        // SAFETY: 这是调用方的责任，见上文。
        //
        // 当 `__rustc_panic_cleaner` 被正确实现时，我们可以依赖 `obj` 是正确的、
        // 可传给 `data.p` 的东西（在用 `ManuallyDrop` 包装之后）。
        unsafe {
            let data = data as *mut Data<F, R>;
            let data = &mut (*data);
            let obj = cleanup(payload);
            data.p = ManuallyDrop::new(obj);
        }
    }
}

/// 判断当前线程是否正因 panic 而处于 unwind 状态。
#[inline]
pub fn panicking() -> bool {
    !panic_count::count_is_zero()
}

/// 来自 core crate 的 panic 入口点（`panic_impl` lang item）。
#[cfg(not(any(test, doctest)))]
#[panic_handler]
pub fn panic_handler(info: &core::panic::PanicInfo<'_>) -> ! {
    struct FormatStringPayload<'a> {
        inner: &'a core::panic::PanicMessage<'a>,
        string: Option<String>,
    }

    impl FormatStringPayload<'_> {
        fn fill(&mut self) -> &mut String {
            let inner = self.inner;
            // 惰性处理：在本方法第一次被调用时，才真正执行字符串格式化。
            self.string.get_or_insert_with(|| {
                let mut s = String::new();
                let mut fmt = fmt::Formatter::new(&mut s, fmt::FormattingOptions::new());
                let _err = fmt::Display::fmt(&inner, &mut fmt);
                s
            })
        }
    }

    unsafe impl PanicPayload for FormatStringPayload<'_> {
        fn take_box(&mut self) -> *mut (dyn Any + Send) {
            // 很遗憾，我们在这里做了两次分配。但是 (a) 在当前方案下这是必需的，
            // 且 (b) 我们本来也没有正确处理 panic + OOM 的情况（见下方 begin_panic
            // 中的注释）。
            let contents = mem::take(self.fill());
            Box::into_raw(Box::new(contents))
        }

        fn get(&mut self) -> &(dyn Any + Send) {
            self.fill()
        }
    }

    impl fmt::Display for FormatStringPayload<'_> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            if let Some(s) = &self.string {
                f.write_str(s)
            } else {
                fmt::Display::fmt(&self.inner, f)
            }
        }
    }

    struct StaticStrPayload(&'static str);

    unsafe impl PanicPayload for StaticStrPayload {
        fn take_box(&mut self) -> *mut (dyn Any + Send) {
            Box::into_raw(Box::new(self.0))
        }

        fn get(&mut self) -> &(dyn Any + Send) {
            &self.0
        }

        fn as_str(&mut self) -> Option<&str> {
            Some(self.0)
        }
    }

    impl fmt::Display for StaticStrPayload {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str(self.0)
        }
    }

    let loc = info.location().unwrap(); // 当前实现总是返回 Some
    let msg = info.message();
    crate::sys::backtrace::__rust_end_short_backtrace(move || {
        if let Some(s) = msg.as_str() {
            panic_with_hook(
                &mut StaticStrPayload(s),
                loc,
                info.can_unwind(),
                info.force_no_backtrace(),
            );
        } else {
            panic_with_hook(
                &mut FormatStringPayload { inner: &msg, string: None },
                loc,
                info.can_unwind(),
                info.force_no_backtrace(),
            );
        }
    })
}

/// 这是 `panic!()` 和 `assert!()` 中非“格式化字符串”变体的 panic 入口点。
/// 特别地，它是唯一支持任意 payload（而不只是格式化字符串）的入口点。
#[unstable(feature = "libstd_sys_internals", reason = "used by the panic! macro", issue = "none")]
#[cfg_attr(not(any(test, doctest)), lang = "begin_panic")]
// 用于 CTFE（编译期求值）panic 支持的 lang item
// 除非 panic=immediate-abort，否则绝不内联，以尽可能减少调用点处的代码膨胀
#[cfg_attr(not(panic = "immediate-abort"), inline(never), cold, optimize(size))]
#[cfg_attr(panic = "immediate-abort", inline)]
#[track_caller]
#[rustc_do_not_const_check] // 由 const-eval 挂接接管
pub const fn begin_panic<M: Any + Send>(msg: M) -> ! {
    if cfg!(panic = "immediate-abort") {
        intrinsics::abort()
    }

    struct Payload<A> {
        inner: Option<A>,
    }

    unsafe impl<A: Send + 'static> PanicPayload for Payload<A> {
        fn take_box(&mut self) -> *mut (dyn Any + Send) {
            // 注意：这应当是本代码路径中执行的唯一一次分配。当前这意味着在 OOM 时
            // 调用 panic!() 会走到这条代码路径，不过话说回来，我们本来也还没准备好
            // 处理 OOM 时的 panic。如果我们将来真要这么做，那么就应当把这次分配改为
            // 在当前线程的父级中执行，而不是在正在 panic 的这个线程中执行。
            let data = match self.inner.take() {
                Some(a) => Box::new(a) as Box<dyn Any + Send>,
                None => process::abort(),
            };
            Box::into_raw(data)
        }

        fn get(&mut self) -> &(dyn Any + Send) {
            match self.inner {
                Some(ref a) => a,
                None => process::abort(),
            }
        }
    }

    impl<A: 'static> fmt::Display for Payload<A> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match &self.inner {
                Some(a) => f.write_str(payload_as_str(a)),
                None => process::abort(),
            }
        }
    }

    let loc = Location::caller();
    crate::sys::backtrace::__rust_end_short_backtrace(move || {
        panic_with_hook(
            &mut Payload { inner: Some(msg) },
            loc,
            /* can_unwind */ true,
            /* force_no_backtrace */ false,
        )
    })
}

fn payload_as_str(payload: &dyn Any) -> &str {
    if let Some(&s) = payload.downcast_ref::<&'static str>() {
        s
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.as_str()
    } else {
        "Box<dyn Any>"
    }
}

/// 派发 panic 的中央枢纽。
///
/// 执行一次 panic 的主要逻辑，包括检查递归 panic、调用 panic 钩子，以及最终派发给
/// panic 运行时，由其决定是 abort 还是 unwind。
#[optimize(size)]
fn panic_with_hook(
    payload: &mut dyn PanicPayload,
    location: &Location<'_>,
    can_unwind: bool,
    force_no_backtrace: bool,
) -> ! {
    let must_abort = panic_count::increase(true);

    // 检查我们是否需要立即 abort。
    if let Some(must_abort) = must_abort {
        match must_abort {
            panic_count::MustAbort::PanicInHook => {
                // 在这种情况下不要尝试格式化消息，也许正是格式化在引发这些递归 panic。
                // 不过，如果消息只是一个字符串，那么打印它不涉及任何用户定义的代码，
                // 因此是无风险的。
                let message: &str = payload.as_str().unwrap_or_default();
                rtprintpanic!(
                    "panicked at {location}:\n{message}\nthread panicked while processing panic. aborting.\n"
                );
            }
            panic_count::MustAbort::AlwaysAbort => {
                // 很遗憾，这里不会打印 backtrace，因为创建一个 `Backtrace` 会分配内存，
                // 而我们在这里必须避免分配。
                rtprintpanic!("aborting due to panic at {location}:\n{payload}\n");
            }
        }
        crate::process::abort();
    }

    match *HOOK.read() {
        // 某些平台（比如 wasm）知道向 stderr 打印实际上不会输出任何东西，若是这种情况，
        // 我们就可以跳过默认钩子。由于字符串格式化是在调用 `payload` 的各方法时才惰性
        // 发生的，这意味着我们可以完全避免格式化该字符串！
        // （不过 panic 运行时仍然可能调用 `payload.take_box()` 从而触发格式化。）
        Hook::Default if panic_output().is_none() => {}
        Hook::Default => {
            default_hook(&PanicHookInfo::new(
                location,
                payload.get(),
                can_unwind,
                force_no_backtrace,
            ));
        }
        Hook::Custom(ref hook) => {
            hook(&PanicHookInfo::new(location, payload.get(), can_unwind, force_no_backtrace));
        }
    }

    // 表明我们已经执行完 panic 钩子。在此之后，即便在执行析构函数期间发生 panic 也没
    // 关系，只要它被包含在某个 `catch_unwind` 之内即可。
    panic_count::finished_panic_hook();

    if !can_unwind {
        // 如果一个线程在运行析构函数期间 panic，或试图穿过一个 nounwind 函数
        // （例如 extern "C"）进行 unwind，那么我们就无法继续 unwind，只能立即 abort。
        rtprintpanic!("thread caused non-unwinding panic. aborting.\n");
        crate::process::abort();
    }

    rust_panic(payload)
}

/// 这是 `resume_unwind` 的入口点。
/// 它只是把 payload 转发给 panic 运行时。
#[cfg_attr(panic = "immediate-abort", inline)]
pub fn resume_unwind(payload: Box<dyn Any + Send>) -> ! {
    panic_count::increase(false);

    struct RewrapBox(Box<dyn Any + Send>);

    unsafe impl PanicPayload for RewrapBox {
        fn take_box(&mut self) -> *mut (dyn Any + Send) {
            Box::into_raw(mem::replace(&mut self.0, Box::new(())))
        }

        fn get(&mut self) -> &(dyn Any + Send) {
            &*self.0
        }
    }

    impl fmt::Display for RewrapBox {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str(payload_as_str(&self.0))
        }
    }

    rust_panic(&mut RewrapBox(payload))
}

/// 一个带有固定后缀（通过 `rustc_std_internal_symbol`）的函数，
/// 方便你往上拍断点（breakpoint）。
#[inline(never)]
#[cfg_attr(not(test), rustc_std_internal_symbol)]
#[cfg(not(panic = "immediate-abort"))]
fn rust_panic(msg: &mut dyn PanicPayload) -> ! {
    let code = unsafe { __rust_start_panic(msg) };
    rtabort!("failed to initiate panic, error {code}")
}

#[cfg_attr(not(test), rustc_std_internal_symbol)]
#[cfg(panic = "immediate-abort")]
fn rust_panic(_: &mut dyn PanicPayload) -> ! {
    crate::intrinsics::abort();
}
