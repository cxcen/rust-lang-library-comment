//! 对捕获某个 OS 线程的栈 backtrace（调用栈回溯）提供支持
//!
//! 本模块包含从运行中的 OS 线程自身捕获该线程栈 backtrace 所需的支持。`Backtrace`
//! 类型支持通过 `Backtrace::capture` 与 `Backtrace::force_capture` 函数来捕获栈轨迹。
//!
//! backtrace 通常很适合附加到错误上（例如实现了 `std::error::Error` 的类型），以便
//! 得到一条说明错误在何处产生的因果链。
//!
//! ## 准确性（Accuracy）
//!
//! backtrace 会尽量做到准确，但并不对其确切的准确性作任何保证。指令指针、符号名、
//! 文件名、行号等都可能报告错误。准确性是在尽力而为（best-effort）的基础上努力达成的；
//! 不过，任何指出可改进之处的 bug 报告都始终欢迎！
//!
//! 在大多数平台上，要让 backtrace 带有文件名/行号，要求程序在编译时带有调试信息。
//! 没有调试信息时，文件名/行号将不会被报告。
//!
//! ## 平台支持（Platform support）
//!
//! 并非 std 能编译到的所有平台都支持捕获 backtrace。某些平台在捕获 backtrace 时只是
//! 什么都不做。要检查某平台是否支持捕获 backtrace，你可以查看 `Backtrace::status`
//! 返回的 `BacktraceStatus` 枚举。
//!
//! 和上面的准确性一样，平台支持也是在尽力而为的基础上完成的。有时运行时可能没有相应
//! 的库可用，或者出了某些差错，从而导致 backtrace 没能被捕获。如果你遇到无法捕获
//! backtrace 的平台，也请尽管提交 issue！
//!
//! ## 环境变量（Environment Variables）
//!
//! `Backtrace::capture` 函数在默认情况下可能并不会真的捕获 backtrace。它的行为受两个
//! 环境变量控制：
//!
//! * `RUST_LIB_BACKTRACE` —— 如果它被设为 `0`，那么 `Backtrace::capture` 将永不捕获
//!   backtrace。设为任何其他值都会启用 `Backtrace::capture`。
//!
//! * `RUST_BACKTRACE` —— 如果 `RUST_LIB_BACKTRACE` 未设置，则会按照与 `RUST_LIB_BACKTRACE`
//!   相同的规则来查阅这个变量。
//!
//! * 如果上述两个环境变量都未设置，那么 `Backtrace::capture` 将被禁用。
//!
//! 捕获 backtrace 可能是一项相当昂贵的运行时操作，因此这些环境变量允许你要么强制禁用
//! 这一运行时性能开销，要么在某些程序中选择性地启用它。
//!
//! 注意，`Backtrace::force_capture` 函数可用于忽略这些环境变量。还需注意，环境变量的
//! 状态会在第一个 backtrace 被创建时就被缓存下来，因此在运行时修改 `RUST_LIB_BACKTRACE`
//! 或 `RUST_BACKTRACE` 可能并不会真的改变 backtrace 的捕获方式。

#![stable(feature = "backtrace", since = "1.65.0")]

#[cfg(test)]
mod tests;

// 注意：关于 backtrace 的解析（resolution）：
//
// backtrace 主要分两步进行。第一步是真正去捕获栈 backtrace，得到一系列与各个栈帧
// 对应的指令指针。接下来我们把这些指令指针逐个转换成人类可读的名字（比如 `main`）。
//
// 第一步可能有些昂贵（要遍历栈），在 MSVC 上尤其如此——那里会查阅调试信息，从而把
// 内联帧（inline frame）各自作为独立的一帧返回。然而第二步几乎总是极其昂贵的（有时
// 达到毫秒数量级），因为它要查阅调试信息。
//
// 我们尽量摊薄这一开销：把“将地址解析为人类可读名字”这件事尽可能往后推迟。当调用
// `Backtrace::create` 来捕获一个 backtrace 时，它并不会真的执行任何符号解析，而是
// 直到这些符号即将被用于打印之前才惰性地解析它们。这样一来，捕获一个 backtrace 然后
// 把它丢弃就能便宜得多，而真正打印一个 backtrace 的开销则基本不变。
//
// 这一策略的代价是在 `Backtrace` 内部需要一些同步，但相对于捕获一个 backtrace 或真正
// 对其符号化（symbolize）而言，这个代价相对较小。

use crate::backtrace_rs::{self, BytesOrWideString};
use crate::ffi::c_void;
use crate::panic::UnwindSafe;
use crate::sync::LazyLock;
use crate::sync::atomic::Ordering::Relaxed;
use crate::sync::atomic::{Atomic, AtomicU8};
use crate::sys::backtrace::{lock, output_filename, set_image_base};
use crate::{env, fmt};

/// 已捕获的某个 OS 线程的栈 backtrace。
///
/// 该类型表示某个 OS 线程在过去某一时刻被捕获的栈 backtrace。在某些情形下，由于配置
/// 原因，`Backtrace` 内部可能是空的。更多信息见 `Backtrace::capture`。
#[stable(feature = "backtrace", since = "1.65.0")]
#[must_use]
pub struct Backtrace {
    inner: Inner,
}

/// backtrace 的当前状态，指示它是已被捕获，还是因某种其他原因而为空。
#[stable(feature = "backtrace", since = "1.65.0")]
#[non_exhaustive]
#[derive(Debug, PartialEq, Eq)]
pub enum BacktraceStatus {
    /// 不支持捕获 backtrace，很可能是因为当前平台尚未实现这一功能。
    #[stable(feature = "backtrace", since = "1.65.0")]
    Unsupported,
    /// 捕获 backtrace 已通过 `RUST_LIB_BACKTRACE` 或 `RUST_BACKTRACE` 环境变量被禁用。
    #[stable(feature = "backtrace", since = "1.65.0")]
    Disabled,
    /// 已经捕获到一个 backtrace，渲染该 `Backtrace` 时应当能打印出合理的信息。
    #[stable(feature = "backtrace", since = "1.65.0")]
    Captured,
}

enum Inner {
    Unsupported,
    Disabled,
    Captured(LazyLock<Capture, LazyResolve>),
}

struct Capture {
    actual_start: usize,
    frames: Vec<BacktraceFrame>,
}

fn _assert_send_sync() {
    fn _assert<T: Send + Sync>() {}
    _assert::<Backtrace>();
}

/// backtrace 中的单个栈帧（frame）。
#[unstable(feature = "backtrace_frames", issue = "79676")]
pub struct BacktraceFrame {
    frame: RawFrame,
    symbols: Vec<BacktraceSymbol>,
}

#[derive(Debug)]
enum RawFrame {
    Actual(backtrace_rs::Frame),
    #[cfg(test)]
    Fake,
}

struct BacktraceSymbol {
    name: Option<Vec<u8>>,
    filename: Option<BytesOrWide>,
    lineno: Option<u32>,
    colno: Option<u32>,
}

enum BytesOrWide {
    Bytes(Vec<u8>),
    Wide(Vec<u16>),
}

#[stable(feature = "backtrace", since = "1.65.0")]
impl fmt::Debug for Backtrace {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        let capture = match &self.inner {
            Inner::Unsupported => return fmt.write_str("<unsupported>"),
            Inner::Disabled => return fmt.write_str("<disabled>"),
            Inner::Captured(c) => &**c,
        };

        let frames = &capture.frames[capture.actual_start..];

        write!(fmt, "Backtrace ")?;

        let mut dbg = fmt.debug_list();

        for frame in frames {
            if frame.frame.ip().is_null() {
                continue;
            }

            dbg.entries(&frame.symbols);
        }

        dbg.finish()
    }
}

#[unstable(feature = "backtrace_frames", issue = "79676")]
impl fmt::Debug for BacktraceFrame {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut dbg = fmt.debug_list();
        dbg.entries(&self.symbols);
        dbg.finish()
    }
}

impl fmt::Debug for BacktraceSymbol {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        // FIXME: 改进格式化：https://github.com/rust-lang/rust/issues/65280
        // FIXME: 另外，把列号也纳入 debug 格式中，因为 Display 已经包含了它们。
        // 在出现稳定的逐帧访问器（per-frame accessor）之前，这个格式不应被改动：
        // https://github.com/rust-lang/rust/issues/65280#issuecomment-638966585
        write!(fmt, "{{ ")?;

        if let Some(fn_name) = self.name.as_ref().map(|b| backtrace_rs::SymbolName::new(b)) {
            write!(fmt, "fn: \"{:#}\"", fn_name)?;
        } else {
            write!(fmt, "fn: <unknown>")?;
        }

        if let Some(fname) = self.filename.as_ref() {
            write!(fmt, ", file: \"{:?}\"", fname)?;
        }

        if let Some(line) = self.lineno {
            write!(fmt, ", line: {:?}", line)?;
        }

        write!(fmt, " }}")
    }
}

impl fmt::Debug for BytesOrWide {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        output_filename(
            fmt,
            match self {
                BytesOrWide::Bytes(w) => BytesOrWideString::Bytes(w),
                BytesOrWide::Wide(w) => BytesOrWideString::Wide(w),
            },
            backtrace_rs::PrintFmt::Short,
            crate::env::current_dir().as_ref().ok(),
        )
    }
}

impl Backtrace {
    /// 返回是否通过环境变量启用了 backtrace 捕获。
    fn enabled() -> bool {
        // 缓存读取环境变量的结果，以便让 backtrace 捕获变快，否则每次都读取环境变量
        // 可能会有些慢。
        static ENABLED: Atomic<u8> = AtomicU8::new(0);
        match ENABLED.load(Relaxed) {
            0 => {}
            1 => return false,
            _ => return true,
        }
        let enabled = match env::var("RUST_LIB_BACKTRACE") {
            Ok(s) => s != "0",
            Err(_) => match env::var("RUST_BACKTRACE") {
                Ok(s) => s != "0",
                Err(_) => false,
            },
        };
        ENABLED.store(enabled as u8 + 1, Relaxed);
        enabled
    }

    /// 捕获当前线程的栈 backtrace。
    ///
    /// 本函数会捕获当前正在执行的 OS 线程的栈 backtrace，返回一个 `Backtrace` 类型，
    /// 之后可用它打印整个栈轨迹，或将其渲染为字符串。
    ///
    /// 如果 `RUST_BACKTRACE` 与 `RUST_LIB_BACKTRACE` 这两个 backtrace 变量都未设置，
    /// 本函数将是一个空操作（noop）。如果其中任一环境变量被设置并启用，那么本函数就会
    /// 真正捕获一个 backtrace。捕获 backtrace 既可能占用大量内存，也可能很慢，因此这些
    /// 环境变量允许你放心地大量使用 `Backtrace::capture`，仅在设置了环境变量时才承受
    /// 这一性能下降。
    ///
    /// 若要不管环境变量如何都强制捕获 backtrace，请使用 `Backtrace::force_capture`
    /// 函数。
    #[stable(feature = "backtrace", since = "1.65.0")]
    #[inline(never)] // 想确保这里有一个栈帧可供移除
    pub fn capture() -> Backtrace {
        if !Backtrace::enabled() {
            return Backtrace { inner: Inner::Disabled };
        }
        Backtrace::create(Backtrace::capture as fn() -> Backtrace as usize)
    }

    /// 强制捕获一个完整的 backtrace，无视环境变量的配置。
    ///
    /// 本函数的行为与 `capture` 相同，区别在于它会忽略 `RUST_BACKTRACE` 与
    /// `RUST_LIB_BACKTRACE` 环境变量的值，总是捕获一个 backtrace。
    ///
    /// 注意，在某些平台上捕获 backtrace 可能是一项昂贵的操作，因此在性能敏感的代码
    /// 部分应当谨慎使用它。
    #[stable(feature = "backtrace", since = "1.65.0")]
    #[inline(never)] // 想确保这里有一个栈帧可供移除
    pub fn force_capture() -> Backtrace {
        Backtrace::create(Backtrace::force_capture as fn() -> Backtrace as usize)
    }

    /// 强制捕获一个被禁用的 backtrace，无视环境变量的配置。
    #[stable(feature = "backtrace", since = "1.65.0")]
    #[rustc_const_stable(feature = "backtrace", since = "1.65.0")]
    pub const fn disabled() -> Backtrace {
        Backtrace { inner: Inner::Disabled }
    }

    // 捕获一个 backtrace，其起点恰好位于由 `ip` 所定位的那个函数之前
    fn create(ip: usize) -> Backtrace {
        let _lock = lock();
        let mut frames = Vec::new();
        let mut actual_start = None;
        set_image_base();
        unsafe {
            backtrace_rs::trace_unsynchronized(|frame| {
                frames.push(BacktraceFrame {
                    frame: RawFrame::Actual(frame.clone()),
                    symbols: Vec::new(),
                });
                if frame.symbol_address().addr() == ip && actual_start.is_none() {
                    actual_start = Some(frames.len());
                }
                true
            });
        }

        // 如果没有产生任何栈帧，就假定这是一个不受支持的平台，因为 `backtrace` 目前
        // 没有提供获知这一点的方式，而这应当是一个足够好的近似判断。
        let inner = if frames.is_empty() {
            Inner::Unsupported
        } else {
            Inner::Captured(LazyLock::new(lazy_resolve(Capture {
                actual_start: actual_start.unwrap_or(0),
                frames,
            })))
        };

        Backtrace { inner }
    }

    /// 返回本 backtrace 的状态，指示这次 backtrace 请求是不受支持、被禁用，还是确实
    /// 捕获到了一个栈轨迹。
    #[stable(feature = "backtrace", since = "1.65.0")]
    #[must_use]
    pub fn status(&self) -> BacktraceStatus {
        match self.inner {
            Inner::Unsupported => BacktraceStatus::Unsupported,
            Inner::Disabled => BacktraceStatus::Disabled,
            Inner::Captured(_) => BacktraceStatus::Captured,
        }
    }
}

impl<'a> Backtrace {
    /// 返回一个遍历各个 backtrace 栈帧的迭代器。
    #[must_use]
    #[unstable(feature = "backtrace_frames", issue = "79676")]
    pub fn frames(&'a self) -> &'a [BacktraceFrame] {
        if let Inner::Captured(c) = &self.inner { &c.frames } else { &[] }
    }
}

#[stable(feature = "backtrace", since = "1.65.0")]
impl fmt::Display for Backtrace {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        let capture = match &self.inner {
            Inner::Unsupported => return fmt.write_str("unsupported backtrace"),
            Inner::Disabled => return fmt.write_str("disabled backtrace"),
            Inner::Captured(c) => &**c,
        };

        let full = fmt.alternate();
        let (frames, style) = if full {
            (&capture.frames[..], backtrace_rs::PrintFmt::Full)
        } else {
            (&capture.frames[capture.actual_start..], backtrace_rs::PrintFmt::Short)
        };

        // 在打印路径时，如果当前工作目录（cwd）存在，我们会尝试把它从路径前缀中剥离，
        // 否则就原样打印路径。注意我们只在 short 格式下这么做，因为如果是 full 格式，
        // 我们大概是想把所有信息都打印出来。
        let cwd = crate::env::current_dir();
        let mut print_path = move |fmt: &mut fmt::Formatter<'_>, path: BytesOrWideString<'_>| {
            output_filename(fmt, path, style, cwd.as_ref().ok())
        };

        let mut f = backtrace_rs::BacktraceFmt::new(fmt, style, &mut print_path);
        f.add_context()?;
        for frame in frames {
            if frame.symbols.is_empty() {
                f.frame().print_raw(frame.frame.ip(), None, None, None)?;
            } else {
                for symbol in frame.symbols.iter() {
                    f.frame().print_raw_with_column(
                        frame.frame.ip(),
                        symbol.name.as_ref().map(|b| backtrace_rs::SymbolName::new(b)),
                        symbol.filename.as_ref().map(|b| match b {
                            BytesOrWide::Bytes(w) => BytesOrWideString::Bytes(w),
                            BytesOrWide::Wide(w) => BytesOrWideString::Wide(w),
                        }),
                        symbol.lineno,
                        symbol.colno,
                    )?;
                }
            }
        }
        f.finish()?;
        Ok(())
    }
}

mod helper {
    use super::*;
    pub(super) type LazyResolve = impl (FnOnce() -> Capture) + Send + Sync + UnwindSafe;

    #[define_opaque(LazyResolve)]
    pub(super) fn lazy_resolve(mut capture: Capture) -> LazyResolve {
        move || {
            // 使用全局 backtrace 锁来对此进行同步，因为这是 `backtrace` crate 的要求，
            // 然后再真正把所有内容解析出来。
            let _lock = lock();
            for frame in capture.frames.iter_mut() {
                let symbols = &mut frame.symbols;
                let frame = match &frame.frame {
                    RawFrame::Actual(frame) => frame,
                    #[cfg(test)]
                    RawFrame::Fake => unimplemented!(),
                };
                unsafe {
                    backtrace_rs::resolve_frame_unsynchronized(frame, |symbol| {
                        symbols.push(BacktraceSymbol {
                            name: symbol.name().map(|m| m.as_bytes().to_vec()),
                            filename: symbol.filename_raw().map(|b| match b {
                                BytesOrWideString::Bytes(b) => BytesOrWide::Bytes(b.to_owned()),
                                BytesOrWideString::Wide(b) => BytesOrWide::Wide(b.to_owned()),
                            }),
                            lineno: symbol.lineno(),
                            colno: symbol.colno(),
                        });
                    });
                }
            }

            capture
        }
    }
}
use helper::*;

impl RawFrame {
    fn ip(&self) -> *mut c_void {
        match self {
            RawFrame::Actual(frame) => frame.ip(),
            #[cfg(test)]
            RawFrame::Fake => crate::ptr::without_provenance_mut(1),
        }
    }
}
