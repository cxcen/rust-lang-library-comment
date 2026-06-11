#![allow(missing_docs, nonstandard_style)]

use crate::io;

#[cfg(target_os = "fuchsia")]
pub mod fuchsia;
pub mod futex;
#[cfg(target_os = "linux")]
pub mod linux;
pub mod os;
pub mod stack_overflow;
pub mod sync;
pub mod thread_parking;
pub mod time;
pub mod weak;

#[cfg(target_os = "espidf")]
pub fn init(_argc: isize, _argv: *const *const u8, _sigpipe: u8) {}

#[cfg(not(target_os = "espidf"))]
#[cfg_attr(target_os = "vita", allow(unused_variables))]
// SAFETY: 只能在运行时初始化期间被调用一次。
// NOTE: 不保证一定会运行，例如当 Rust 代码被外部调用时。
// 关于 `sigpipe` 的文档，参见 `library/std/src/rt.rs` 中的 `fn init()`。
pub unsafe fn init(argc: isize, argv: *const *const u8, sigpipe: u8) {
    // 标准流（standard streams）在应用启动时可能处于已关闭状态。为防止
    // std::io::{stdin, stdout, stderr} 对象去使用后来打开的、其他无关的文件资源，
    // 我们在标准流处于关闭状态时重新打开它们。
    sanitize_standard_fds();

    // 默认情况下，某些平台在本应投递 EPIPE 错误时，会改为发送一个*信号*。本运行时
    // 并不安装 SIGPIPE 处理函数，于是它会杀死程序，而这并不完全是我们想要的！
    //
    // 因此，我们在程序启动时把 SIGPIPE 设为忽略，以防止这一问题。可使用
    // `-Zon-broken-pipe=...` 来改变这一行为。
    reset_sigpipe(sigpipe);

    stack_overflow::init();
    #[cfg(not(target_os = "vita"))]
    crate::sys::args::init(argc, argv);

    // 通常 `thread::spawn` 会调用 `Thread::set_name`，但由于本线程已经存在，我们必须
    // 自己来调用它。我们只在 Apple 目标上这么做，因为某些类 unix 操作系统（如
    // Linux）让主线程的 process-id 和 thread-id 共用同一个值，于是重命名主线程就会
    // 把进程也一并重命名；我们只想在那些我们测试过的平台上启用这一行为。
    if cfg!(target_vendor = "apple") {
        crate::sys::thread::set_name(c"main");
    }

    unsafe fn sanitize_standard_fds() {
        #[allow(dead_code, unused_variables, unused_mut)]
        let mut opened_devnull = -1;
        #[allow(dead_code, unused_variables, unused_mut)]
        let mut open_devnull = || {
            #[cfg(not(all(target_os = "linux", target_env = "gnu")))]
            use libc::open;
            #[cfg(all(target_os = "linux", target_env = "gnu"))]
            use libc::open64 as open;

            if opened_devnull != -1 {
                if libc::dup(opened_devnull) != -1 {
                    return;
                }
            }
            opened_devnull = open(c"/dev/null".as_ptr(), libc::O_RDWR, 0);
            if opened_devnull == -1 {
                // 如果该流已关闭、但我们又未能重新打开它，就 abort 进程。否则我们将
                // 无法保持对应 Rust 对象 Stdin、Stdout 或 Stderr 上各项操作的安全性。
                libc::abort();
            }
        };

        // 对于具备 poll() 的系统，走只需单次系统调用的快速路径
        #[cfg(not(any(
            miri,
            target_os = "emscripten",
            target_os = "fuchsia",
            target_os = "vxworks",
            target_os = "redox",
            target_os = "l4re",
            target_os = "horizon",
            target_os = "vita",
            target_os = "rtems",
            // Darwin 上的 poll 不会为已关闭的 fd 设置 POLLNVAL。
            target_vendor = "apple",
        )))]
        'poll: {
            use crate::sys::io::errno;
            let pfds: &mut [_] = &mut [
                libc::pollfd { fd: 0, events: 0, revents: 0 },
                libc::pollfd { fd: 1, events: 0, revents: 0 },
                libc::pollfd { fd: 2, events: 0, revents: 0 },
            ];

            while libc::poll(pfds.as_mut_ptr(), 3, 0) == -1 {
                match errno() {
                    libc::EINTR => continue,
                    #[cfg(target_vendor = "unikraft")]
                    libc::ENOSYS => {
                        // 并非所有 Unikraft 配置都启用了 `LIBPOSIX_EVENT`。
                        break 'poll;
                    }
                    libc::EINVAL | libc::EAGAIN | libc::ENOMEM => {
                        // RLIMIT_NOFILE 或临时性的分配失败都可能妨碍 poll() 的使用，
                        // 此时退回到 fcntl
                        break 'poll;
                    }
                    _ => libc::abort(),
                }
            }
            for pfd in pfds {
                if pfd.revents & libc::POLLNVAL == 0 {
                    continue;
                }
                open_devnull();
            }
            return;
        }

        // 当 poll 不可用、或受 RLIMIT_NOFILE 限制时，走兜底路径
        #[cfg(not(any(
            // 在 Miri 中标准 fd 始终可用。
            miri,
            target_os = "emscripten",
            target_os = "fuchsia",
            target_os = "vxworks",
            target_os = "l4re",
            target_os = "horizon",
            target_os = "vita",
        )))]
        {
            use crate::sys::io::errno;
            for fd in 0..3 {
                if libc::fcntl(fd, libc::F_GETFD) == -1 && errno() == libc::EBADF {
                    open_devnull();
                }
            }
        }
    }

    unsafe fn reset_sigpipe(#[allow(unused_variables)] sigpipe: u8) {
        #[cfg(not(any(
            target_os = "emscripten",
            target_os = "fuchsia",
            target_os = "horizon",
            target_os = "vxworks",
            target_os = "vita",
            // Unikraft 的 `signal` 实现目前是坏的：
            // https://github.com/unikraft/lib-musl/issues/57
            target_vendor = "unikraft",
        )))]
        {
            // 我们既不想把它作为一个公开类型加进 std，也不想从编译器 `include!`
            // 一个文件（那会破坏例如 Miri 和 xargo），所以我们选择把
            // `compiler/rustc_session/src/config/sigpipe.rs` 中的这些常量复制一份。
            // 文档请见那个文件。NOTE: 务必让二者保持同步！
            mod sigpipe {
                pub const DEFAULT: u8 = 0;
                pub const INHERIT: u8 = 1;
                pub const SIG_IGN: u8 = 2;
                pub const SIG_DFL: u8 = 3;
            }

            let (sigpipe_attr_specified, handler) = match sigpipe {
                sigpipe::DEFAULT => (false, Some(libc::SIG_IGN)),
                sigpipe::INHERIT => (true, None),
                sigpipe::SIG_IGN => (true, Some(libc::SIG_IGN)),
                sigpipe::SIG_DFL => (true, Some(libc::SIG_DFL)),
                _ => unreachable!(),
            };
            if sigpipe_attr_specified {
                ON_BROKEN_PIPE_FLAG_USED.store(true, crate::sync::atomic::Ordering::Relaxed);
            }
            if let Some(handler) = handler {
                rtassert!(signal(libc::SIGPIPE, handler) != libc::SIG_ERR);
                #[cfg(target_os = "hurd")]
                {
                    rtassert!(signal(libc::SIGLOST, handler) != libc::SIG_ERR);
                }
            }
        }
    }
}

// 这个在 reset_sigpipe 中被设置（至多一次）。
#[cfg(not(any(
    target_os = "espidf",
    target_os = "emscripten",
    target_os = "fuchsia",
    target_os = "horizon",
    target_os = "vxworks",
    target_os = "vita",
)))]
static ON_BROKEN_PIPE_FLAG_USED: crate::sync::atomic::Atomic<bool> =
    crate::sync::atomic::AtomicBool::new(false);

#[cfg(not(any(
    target_os = "espidf",
    target_os = "emscripten",
    target_os = "fuchsia",
    target_os = "horizon",
    target_os = "vxworks",
    target_os = "vita",
    target_os = "nuttx",
)))]
pub(crate) fn on_broken_pipe_flag_used() -> bool {
    ON_BROKEN_PIPE_FLAG_USED.load(crate::sync::atomic::Ordering::Relaxed)
}

// SAFETY: 只能在运行时清理（cleanup）期间被调用一次。
// NOTE: 不保证一定会运行，例如当程序 abort 时。
pub unsafe fn cleanup() {
    stack_overflow::cleanup();
}

#[allow(unused_imports)]
pub use libc::signal;

#[doc(hidden)]
pub trait IsMinusOne {
    fn is_minus_one(&self) -> bool;
}

macro_rules! impl_is_minus_one {
    ($($t:ident)*) => ($(impl IsMinusOne for $t {
        fn is_minus_one(&self) -> bool {
            *self == -1
        }
    })*)
}

impl_is_minus_one! { i8 i16 i32 i64 isize }

/// 按照 *-1 表示出错、错误码在 `errno` 中* 的约定，把原生返回值转换为 Result。
/// 非错误的值会被包进 `Ok`。
pub fn cvt<T: IsMinusOne>(t: T) -> io::Result<T> {
    if t.is_minus_one() { Err(io::Error::last_os_error()) } else { Ok(t) }
}

/// `-1` → 查看 `errno` → 遇到 `EINTR` 时重试。否则把闭包的返回值包进 `Ok()`。
pub fn cvt_r<T, F>(mut f: F) -> io::Result<T>
where
    T: IsMinusOne,
    F: FnMut() -> T,
{
    loop {
        match cvt(f()) {
            Err(ref e) if e.is_interrupted() => {}
            other => return other,
        }
    }
}

#[allow(dead_code)] // 并非在所有平台上都会用到。
/// 零表示 `Ok()`，所有其他值都被当作原始 OS 错误处理。不查看 `errno`。
pub fn cvt_nz(error: libc::c_int) -> io::Result<()> {
    if error == 0 { Ok(()) } else { Err(io::Error::from_raw_os_error(error)) }
}

// libc::abort() 会运行 SIGABRT 处理函数。这没问题，因为任何安装了 SIGABRT 处理函数的
// 人，本就必须预料到它会在“非常糟糕”的情形下运行（例如 malloc 崩溃）。
//
// 当前 glibc 的 abort() 函数会解除对 SIGABRT 的阻塞、raise SIGABRT、清除 SIGABRT
// 处理函数后再次 raise 它，然后开始“各显神通”。
//
// 进一步的讨论参见 `intrinsics::abort()` 和 `process::abort()` 的公开文档。
//
// 关于 libc::abort() 是否会刷新（flush）stdio 流，存在一些混淆。ISO C 99（7.14.1.1p5）
// 要求 libc::abort() 必须是 async-signal-safe 的，因此刷新流即便不是完全不可能，至少
// 也是极其困难的。
//
// 然而，某些版本的 POSIX（例如 IEEE Std 1003.1-2001）曾要求 abort 这么做。在
// 1003.1-2004 中这一点被修正了。
//
// 在 Florian Weimer 提交的 glibc commit 91e7cf982d01 `abort: Do not flush stdio
// streams [BZ #15436]` 之前，glibc 的实现确实会（不安全地）做刷新。按照 glibc 的
// NEWS：
//
//    abort 函数会立即终止进程，而不刷新 stdio 流。之前的 glibc 版本曾会刷新流，
//    从而导致死锁以及进一步的数据损坏。这一变更同样影响断言失败所导致的进程 abort。
//
// 这是对该问题的准确描述。对于以非平凡方式使用 C stdio 的程序，唯一的解决办法就是一个
// 已修复的 libc——即在 abort 中不尝试刷新的那种——因为即便是 libc 内部的错误、以及由
// C 产生的断言失败，也都会经由 abort() 走。
//
// 在使用了陈旧、有 bug 的 libc 的系统上，对一个多线程 C 程序而言其影响可能很严重。对
// Rust 来说则影响小得多，因为 Rust 标准库不使用 libc 的 stdio 缓冲。在一个不使用 C
// stdio 的典型 Rust 程序中，即便是有 bug 的 libc::abort()，实际上也是安全的。
#[cfg_attr(miri, track_caller)] // 即便没有 panic，这对 Miri 的回溯（backtrace）也有帮助
pub fn abort_internal() -> ! {
    unsafe { libc::abort() }
}

cfg_select! {
    target_os = "android" => {
        #[link(name = "dl", kind = "static", modifiers = "-bundle",
            cfg(target_feature = "crt-static"))]
        #[link(name = "dl", cfg(not(target_feature = "crt-static")))]
        #[link(name = "log", cfg(not(target_feature = "crt-static")))]
        unsafe extern "C" {}
    }
    target_os = "freebsd" => {
        #[link(name = "execinfo")]
        #[link(name = "pthread")]
        unsafe extern "C" {}
    }
    target_os = "netbsd" => {
        #[link(name = "execinfo")]
        #[link(name = "pthread")]
        #[link(name = "rt")]
        unsafe extern "C" {}
    }
    any(target_os = "dragonfly", target_os = "openbsd", target_os = "cygwin") => {
        #[link(name = "pthread")]
        unsafe extern "C" {}
    }
    target_os = "solaris" => {
        #[link(name = "socket")]
        #[link(name = "posix4")]
        #[link(name = "pthread")]
        #[link(name = "resolv")]
        unsafe extern "C" {}
    }
    target_os = "illumos" => {
        #[link(name = "socket")]
        #[link(name = "posix4")]
        #[link(name = "pthread")]
        #[link(name = "resolv")]
        #[link(name = "nsl")]
        // 使用 libumem 作为（与 malloc 兼容的）分配器
        #[link(name = "umem")]
        unsafe extern "C" {}
    }
    target_vendor = "apple" => {
        // 链接到 `libSystem.dylib`。
        //
        // 不要被 `System.framework` 的存在搞糊涂了，
        // 它是对该动态库的一个已弃用的包装层。
        #[link(name = "System")]
        unsafe extern "C" {}
    }
    target_os = "fuchsia" => {
        #[link(name = "zircon")]
        #[link(name = "fdio")]
        unsafe extern "C" {}
    }
    all(target_os = "linux", target_env = "uclibc") => {
        #[link(name = "dl")]
        unsafe extern "C" {}
    }
    target_os = "vita" => {
        #[link(name = "pthread", kind = "static", modifiers = "-bundle")]
        unsafe extern "C" {}
    }
    _ => {}
}

#[cfg(any(target_os = "espidf", target_os = "horizon", target_os = "vita", target_os = "nuttx"))]
pub mod unsupported {
    use crate::io;

    pub fn unsupported<T>() -> io::Result<T> {
        Err(unsupported_err())
    }

    pub fn unsupported_err() -> io::Error {
        io::Error::UNSUPPORTED_PLATFORM
    }
}
