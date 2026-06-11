//! 打印 backtrace（调用栈回溯）的公共代码。
#![forbid(unsafe_op_in_unsafe_fn)]

use crate::backtrace_rs::{self, BacktraceFmt, BytesOrWideString, PrintFmt};
use crate::borrow::Cow;
use crate::io::prelude::*;
use crate::path::{self, Path, PathBuf};
use crate::sync::{Mutex, MutexGuard, PoisonError};
use crate::{env, fmt, io};

/// 最多打印的栈帧数量。
const MAX_NB_FRAMES: usize = 100;

pub(crate) const FULL_BACKTRACE_DEFAULT: bool = cfg_select! {
    // Fuchsia 的组件默认使用完整 backtrace。
    target_os = "fuchsia" => true,
    _ => false,
};

pub(crate) struct BacktraceLock<'a>(#[allow(dead_code)] MutexGuard<'a, ()>);

pub(crate) fn lock<'a>() -> BacktraceLock<'a> {
    static LOCK: Mutex<()> = Mutex::new(());
    BacktraceLock(LOCK.lock().unwrap_or_else(PoisonError::into_inner))
}

impl BacktraceLock<'_> {
    /// 打印当前的 backtrace。
    pub(crate) fn print(&mut self, w: &mut dyn Write, format: PrintFmt) -> io::Result<()> {
        // 目前把 libbacktrace 链接进测试中存在问题，而且在 std 自身的单元测试里
        // 我们一般也不测试这条路径。因此在 test 模式下立即返回，以便优化掉对
        // libbacktrace 各符号的任何引用。
        if cfg!(test) {
            return Ok(());
        }

        struct DisplayBacktrace {
            format: PrintFmt,
        }
        impl fmt::Display for DisplayBacktrace {
            fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
                // SAFETY: backtrace 锁已被持有
                unsafe { _print_fmt(fmt, self.format) }
            }
        }
        write!(w, "{}", DisplayBacktrace { format })
    }
}

/// # 安全性(Safety）
///
/// 本函数不是 Sync 的。调用者必须持有一个互斥锁，或者保证程序中只有一个线程。
unsafe fn _print_fmt(fmt: &mut fmt::Formatter<'_>, print_fmt: PrintFmt) -> fmt::Result {
    // 在 Miri 下始终“故意失败”地获取 cwd ——
    // 这样可以让 Miri 在隔离（isolation）模式下也能显示 backtrace
    let cwd = if !cfg!(miri) { env::current_dir().ok() } else { None };

    let mut print_path = move |fmt: &mut fmt::Formatter<'_>, bows: BytesOrWideString<'_>| {
        output_filename(fmt, bows, print_fmt, cwd.as_ref())
    };
    writeln!(fmt, "stack backtrace:")?;
    let mut bt_fmt = BacktraceFmt::new(fmt, print_fmt, &mut print_path);
    bt_fmt.add_context()?;
    let mut idx = 0;
    let mut res = Ok(());
    let mut omitted_count: usize = 0;
    let mut first_omit = true;
    // 如果使用的是简短（short）backtrace，则在被告知开始打印之前忽略所有栈帧。
    let mut print = print_fmt != PrintFmt::Short;
    set_image_base();
    // SAFETY: 在这片地界我们自行实现锁机制
    unsafe {
        backtrace_rs::trace_unsynchronized(|frame| {
            if print_fmt == PrintFmt::Short && idx > MAX_NB_FRAMES {
                return false;
            }

            if cfg!(feature = "backtrace-trace-only") {
                const HEX_WIDTH: usize = 2 + 2 * size_of::<usize>();
                let frame_ip = frame.ip();
                res = writeln!(bt_fmt.formatter(), "{idx:4}: {frame_ip:HEX_WIDTH$?}");
            } else {
                let mut hit = false;
                backtrace_rs::resolve_frame_unsynchronized(frame, |symbol| {
                    hit = true;

                    // 出现 `__rust_end_short_backtrace` 表示我们暂时不再隐藏符号。
                    // 一直打印，直到看到 `__rust_begin_short_backtrace` 为止。
                    if print_fmt == PrintFmt::Short {
                        if let Some(sym) = symbol.name().and_then(|s| s.as_str()) {
                            if sym.contains("__rust_end_short_backtrace") {
                                print = true;
                                return;
                            }
                            if print && sym.contains("__rust_begin_short_backtrace") {
                                print = false;
                                return;
                            }
                            if !print {
                                omitted_count += 1;
                            }
                        }
                    }

                    if print {
                        if omitted_count > 0 {
                            debug_assert!(print_fmt == PrintFmt::Short);
                            // 只在两段栈帧之间打印该提示信息
                            if !first_omit {
                                let _ = writeln!(
                                    bt_fmt.formatter(),
                                    "      [... omitted {} frame{} ...]",
                                    omitted_count,
                                    if omitted_count > 1 { "s" } else { "" }
                                );
                            }
                            first_omit = false;
                            omitted_count = 0;
                        }
                        res = bt_fmt.frame().symbol(frame, symbol);
                    }
                });
                #[cfg(all(target_os = "nto", any(target_env = "nto70", target_env = "nto71")))]
                if libc::__my_thread_exit as *mut libc::c_void == frame.ip() {
                    if !hit && print {
                        use crate::backtrace_rs::SymbolName;
                        res = bt_fmt.frame().print_raw(
                            frame.ip(),
                            Some(SymbolName::new("__my_thread_exit".as_bytes())),
                            None,
                            None,
                        );
                    }
                    return false;
                }
                if !hit && print {
                    res = bt_fmt.frame().print_raw(frame.ip(), None, None, None);
                }
            }

            idx += 1;
            res.is_ok()
        })
    };
    res?;
    bt_fmt.finish()?;
    if print_fmt == PrintFmt::Short {
        writeln!(
            fmt,
            "note: Some details are omitted, \
             run with `RUST_BACKTRACE=full` for a verbose backtrace."
        )?;
    }
    Ok(())
}

/// 用于在 `RUST_BACKTRACE=1` 时清理 backtrace 的固定栈帧。注意，
/// 只有在 std 中启用了 backtrace 时它才是 inline(never)；否则
/// 把它优化掉是没问题的。
#[cfg_attr(feature = "backtrace", inline(never))]
pub fn __rust_begin_short_backtrace<F, T>(f: F) -> T
where
    F: FnOnce() -> T,
{
    let result = f();

    // 防止该栈帧被尾调用优化（tail-call optimisation）掉
    crate::hint::black_box(());

    result
}

/// 用于在 `RUST_BACKTRACE=1` 时清理 backtrace 的固定栈帧。注意，
/// 只有在 std 中启用了 backtrace 时它才是 inline(never)；否则
/// 把它优化掉是没问题的。
#[cfg_attr(feature = "backtrace", inline(never))]
pub fn __rust_end_short_backtrace<F, T>(f: F) -> T
where
    F: FnOnce() -> T,
{
    let result = f();

    // 防止该栈帧被尾调用优化（tail-call optimisation）掉
    crate::hint::black_box(());

    result
}

/// 打印 backtrace 栈帧的文件名。
///
/// 另见 `output`。
pub fn output_filename(
    fmt: &mut fmt::Formatter<'_>,
    bows: BytesOrWideString<'_>,
    print_fmt: PrintFmt,
    cwd: Option<&PathBuf>,
) -> fmt::Result {
    let file: Cow<'_, Path> = match bows {
        #[cfg(unix)]
        BytesOrWideString::Bytes(bytes) => {
            use crate::os::unix::prelude::*;
            Path::new(crate::ffi::OsStr::from_bytes(bytes)).into()
        }
        #[cfg(not(unix))]
        BytesOrWideString::Bytes(bytes) => {
            Path::new(crate::str::from_utf8(bytes).unwrap_or("<unknown>")).into()
        }
        #[cfg(windows)]
        BytesOrWideString::Wide(wide) => {
            use crate::os::windows::prelude::*;
            Cow::Owned(crate::ffi::OsString::from_wide(wide).into())
        }
        #[cfg(not(windows))]
        BytesOrWideString::Wide(_wide) => Path::new("<unknown>").into(),
    };
    if print_fmt == PrintFmt::Short && file.is_absolute() {
        if let Some(cwd) = cwd {
            if let Ok(stripped) = file.strip_prefix(&cwd) {
                if let Some(s) = stripped.to_str() {
                    return write!(fmt, ".{}{s}", path::MAIN_SEPARATOR);
                }
            }
        }
    }
    fmt::Display::fmt(&file.display(), fmt)
}

#[cfg(all(target_vendor = "fortanix", target_env = "sgx"))]
pub fn set_image_base() {
    let image_base = crate::os::fortanix_sgx::mem::image_base();
    backtrace_rs::set_image_base(crate::ptr::without_provenance_mut(image_base as _));
}

#[cfg(not(all(target_vendor = "fortanix", target_env = "sgx")))]
pub fn set_image_base() {
    // 对于 SGX 以外的平台无需任何操作
}
