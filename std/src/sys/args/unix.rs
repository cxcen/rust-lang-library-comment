//! 命令行参数的全局初始化与获取。
//!
//! 在某些平台上，这些参数在运行时启动（runtime startup）期间被保存下来；
//! 而在另一些平台上，则是按需从系统中获取。

#![allow(dead_code)] // 运行时初始化函数在测试期间不会被用到

pub use super::common::Args;
use crate::ffi::CStr;
#[cfg(target_os = "hermit")]
use crate::os::hermit::ffi::OsStringExt;
#[cfg(not(target_os = "hermit"))]
use crate::os::unix::ffi::OsStringExt;

/// 一次性的全局初始化。
pub unsafe fn init(argc: isize, argv: *const *const u8) {
    unsafe { imp::init(argc, argv) }
}

/// 返回命令行参数
pub fn args() -> Args {
    let (argc, argv) = imp::argc_argv();

    let mut vec = Vec::with_capacity(argc as usize);

    for i in 0..argc {
        // SAFETY: 如果 `argc` 为正，则 `argv` 非 null，且保证其长度
        // 至少与 `argc` 一样长，因此从中读取
        // 应当是安全的。
        let ptr = unsafe { argv.offset(i).read() };

        // 某些 C 命令行解析器（例如 GLib 和 Qt）会把 `argv` 中已经
        // 处理过的参数替换为 `NULL`，并将它们移到末尾。
        //
        // 由于它们无法同时直接确保 `argc` 也被更新，这就意味着此刻
        // `argc` 可能比 `argv` 中实际非 `NULL` 指针的数量
        // 要大。
        //
        // 为处理这一点，我们只需在遇到第一个 `NULL` 参数时停止
        // 迭代即可。`argv` 还保证以 `NULL` 结尾，所以第一个 `NULL`
        // 之后的任何非 `NULL` 参数都可以安全地忽略。
        if ptr.is_null() {
            // 注意：在 Apple 平台上，`-[NSProcessInfo arguments]` 在此处
            // 并不会停止迭代，而是 `continue`，始终一直迭代
            // 直到达到 `argc`。
            //
            // 这一差异只有在 `argc`/`argv` 被修改、且修改方式出人意料的
            // 那种非常特定的情形下才会有影响；但既然如此，我们选哪个
            // 选项大概也无所谓。
            // 进一步的讨论参见下面这个 PR：
            // <https://github.com/rust-lang/rust/pull/125225>
            break;
        }

        // SAFETY: 刚刚检查过该指针不为 NULL，而参数
        // 在其他方面也保证是有效的 C 字符串。
        let cstr = unsafe { CStr::from_ptr(ptr) };
        vec.push(OsStringExt::from_vec(cstr.to_bytes().to_vec()));
    }

    Args::new(vec)
}

#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "freebsd",
    target_os = "dragonfly",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "cygwin",
    target_os = "solaris",
    target_os = "illumos",
    target_os = "emscripten",
    target_os = "haiku",
    target_os = "hermit",
    target_os = "l4re",
    target_os = "fuchsia",
    target_os = "redox",
    target_os = "vxworks",
    target_os = "horizon",
    target_os = "aix",
    target_os = "nto",
    target_os = "hurd",
    target_os = "rtems",
    target_os = "nuttx",
))]
mod imp {
    use crate::ffi::c_char;
    use crate::ptr;
    use crate::sync::atomic::{Atomic, AtomicIsize, AtomicPtr, Ordering};

    // 系统提供的 argc 和 argv，我们在这里把它们存放在静态内存中，
    // 以便能把解析它们的工作推迟到真正需要时
    // 再做。
    //
    // 注意，我们从不修改 argv/argc、argv 数组本身、或 argv
    // 中的字符串，这使得本文件中的代码可以非常简单。
    static ARGC: Atomic<isize> = AtomicIsize::new(0);
    static ARGV: Atomic<*mut *const u8> = AtomicPtr::new(ptr::null_mut());

    unsafe fn really_init(argc: isize, argv: *const *const u8) {
        // 它们之间、以及与其他存储操作之间都不需要保证顺序，
        // 因为它们只保存未经修改的、系统提供的 argv/argc。
        ARGC.store(argc, Ordering::Relaxed);
        ARGV.store(argv as *mut _, Ordering::Relaxed);
    }

    #[inline(always)]
    pub unsafe fn init(argc: isize, argv: *const *const u8) {
        // 在 GNU/Linux 上，如果我们是 main，那么会把 argv 和 argc 初始化两次，这“重复了工作”，
        // 但边界情况是真实存在的：仅使用 .init_array 会破坏大多数模拟器、dlopen 等等。
        unsafe { really_init(argc, argv) };
    }

    /// glibc 会把 argc、argv 和 envp 传给 .init_array 中的函数，这是一种非标准扩展。
    /// 这使得 `std::env::args` 即便在 `cdylib` 中也能工作，正如它在 macOS 和 Windows 上那样。
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    #[used]
    #[unsafe(link_section = ".init_array.00099")]
    static ARGV_INIT_ARRAY: extern "C" fn(
        crate::os::raw::c_int,
        *const *const u8,
        *const *const u8,
    ) = {
        extern "C" fn init_wrapper(
            argc: crate::os::raw::c_int,
            argv: *const *const u8,
            _envp: *const *const u8,
        ) {
            unsafe { really_init(argc as isize, argv) };
        }
        init_wrapper
    };

    pub fn argc_argv() -> (isize, *const *const c_char) {
        // 加载 ARGC 和 ARGV，它们保存着未经修改的、系统提供的
        // argc/argv，因此我们无需原子操作或同步即可读取
        // 其所指向的内存。
        //
        // 如果 ARGC 或 ARGV 仍然为零或为 null，那么要么确实
        // 没有任何参数，要么有人在初始化尚未完成之前
        // 就请求 `args()`，此时我们返回一个空列表。
        let argv = ARGV.load(Ordering::Relaxed);
        let argc = if argv.is_null() { 0 } else { ARGC.load(Ordering::Relaxed) };

        // 从 `*mut *const u8` 强制转换为 `*const *const c_char`
        (argc, argv.cast())
    }
}

// 在 Apple 平台上使用 `_NSGetArgc` 和 `_NSGetArgv`。
//
// 尽管它们的名字里带有下划线，但自 macOS 和 iOS 的最初版本起
// 它们就一直可用，并且在头文件 `crt_externs.h` 中
// 有声明。
//
// 注意：该头文件是在 iOS 13.0 SDK 中添加的，这在过去曾就这些 API
// 的可用性造成了大量困惑。
//
// NOTE(madsmtm): 这一点尚未经过严格验证以确认不会导致 App Store
// 被拒；如果发现确实如此，那么之前该实现使用的是
// `[[NSProcessInfo processInfo] arguments]`。
#[cfg(target_vendor = "apple")]
mod imp {
    use crate::ffi::{c_char, c_int};

    pub unsafe fn init(_argc: isize, _argv: *const *const u8) {
        // 这里无需初始化任何东西，`libdyld.dylib` 已经
        // 替我们完成了这项工作。
    }

    pub fn argc_argv() -> (isize, *const *const c_char) {
        unsafe extern "C" {
            // 这些函数在 crt_externs.h 中。
            fn _NSGetArgc() -> *mut c_int;
            fn _NSGetArgv() -> *mut *mut *mut c_char;
        }

        // SAFETY: 返回的指针指向一个由 `libdyld.dylib` 在程序生命周期
        // 早期就初始化好的静态量，因此它始终
        // 有效。
        //
        // 注意：与 `_NSGetEnviron` 类似，从技术上讲并没有什么东西
        // 保护我们免受对它的并发修改，而且也不存在我们可以获取的
        // 锁。相反，一般的预期是它只会在 `main` 中 / 在其他代码
        // 运行之前被修改，因此在这里读取它应当没有问题。
        let argc = unsafe { _NSGetArgc().read() };
        // SAFETY: 同上。
        let argv = unsafe { _NSGetArgv().read() };

        // 从 `*mut *mut c_char` 强制转换为 `*const *const c_char`
        (argc as isize, argv.cast())
    }
}
