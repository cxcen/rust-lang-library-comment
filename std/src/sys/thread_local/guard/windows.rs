//! 对 Windows TLS 析构函数（destructors）的支持。
//!
//! 遗憾的是，Windows 并未提供一个友好的 API 来为 TLS 变量提供析构函数。
//! 因此，这里的解决方案最终显得有些晦涩，但不必担心，互联网告诉我 [1][2]
//! 这个方案并非独创（我自己也绝不可能想得出来！）。核心思路是在某处插入
//! 某种钩子（hook），以便在线程终止时运行任意代码。有了它，我们就能运行
//! 任何想运行的东西，包括所有的 TLS 析构函数！
//!
//! 为了实现这一点，所有 TLS 析构函数都由*我们*来追踪，而不是 Windows 运行时。
//! 这意味着我们为已知的每个 TLS key 或变量维护一个全局的析构函数列表。
//!
//! # CRT$XLB 是怎么回事？
//!
//! 要让任何关于 TLS 析构函数的机制在 Windows 上工作，我们必须能够在线程退出时
//! 运行*某些东西*。为此，我们把一个非常特殊的 static 放在一个非常特殊的位置。
//! 如果它以恰到好处的方式编码，内核的加载器（loader）显然会很友好地在任何线程
//! 退出时运行我们的某个函数！内核真是太贴心了！
//!
//! 大量详细信息可在上面的来源 [1] 中找到，但其要点是：这利用了 Microsoft 的
//! PE 格式（可执行文件格式）的一个特性，而该特性如今实际上没有任何编译器在使用。
//! 这显然意味着：`.CRT$XLB` 节（section）中的任何回调都会在某些事件发生时被运行。
//!
//! 所以在这一切之后，我们使用编译器的 `#[link_section]` 特性，把一个回调指针
//! 放进那个魔法节中，从而使它最终被调用。
//!
//! # 这个回调是怎么回事？
//!
//! 所指定的回调会从……某人那里！（内核？运行时？我也不太确定！）接收若干参数。
//! 它会因为几种事件而被调用，但我们目前只关心线程或进程“分离（detach）”（退出）
//! 的时机。进程部分针对最后一个线程发生，线程部分针对任何普通线程发生。
//!
//! # 那篇文章里提到了关于 "/INCLUDE" 的奇怪东西？
//!
//! 的确如此！具体来说，我们指的是这段引文：
//!
//! ```quote
//! The Microsoft run-time library facilitates this process by defining a
//! memory image of the TLS Directory and giving it the special name
//! “__tls_used” (Intel x86 platforms) or “_tls_used” (other platforms). The
//! linker looks for this memory image and uses the data there to create the
//! TLS Directory. Other compilers that support TLS and work with the
//! Microsoft linker must use this same technique.
//! ```
//!
//! 基本上这意味着：如果我们希望我们的 TLS 析构函数/我们的钩子能被调用，
//! 就需要确保链接器不会省略这个符号。否则它会被省略，我们的回调也就不会
//! 被接上。
//!
//! 我们这里实际上并没有像文章中提到的那样使用 `/INCLUDE` 链接器标志，
//! 因为 Rust 编译器不会传播链接器标志，而是改用一个 shim 函数，它对
//! _tls_used 符号的地址执行一次 volatile 的 1 字节加载，以确保该符号被保留。
//!
//! [1]: https://www.codeproject.com/Articles/8113/Thread-Local-Storage-The-C-Way
//! [2]: https://github.com/ChromiumWebApps/chromium/blob/master/base/threading/thread_local_storage_win.cc#L42

use core::ffi::c_void;

use crate::ptr;
use crate::sys::c;

unsafe extern "C" {
    #[link_name = "_tls_used"]
    static TLS_USED: u8;
}
pub fn enable() {
    // 当使用析构函数时，我们需要添加一个对 CRT 所提供的 _tls_used 符号的引用，
    // 否则 TLS 支持代码会被链接器当作垃圾回收（GC）掉，我们的回调就不会被调用。
    unsafe { ptr::from_ref(&TLS_USED).read_volatile() };
    // 我们还需要引用 CALLBACK，以确保它不会被编译器/LLVM 当作垃圾回收掉。
    // 通过链接器的“魔法操作”，该回调最终会进入由 _TLS_USED 指向的 TLS 回调数组中；
    // 但就编译器看来，这份数据像是未被使用的，所以我们需要这个 hack 来防止它消失。
    unsafe { ptr::from_ref(&CALLBACK).read_volatile() };
}

#[unsafe(link_section = ".CRT$XLB")]
#[cfg_attr(miri, used)] // Miri 在 `lookup_link_section` 时只考虑显式标注了 `#[used]` 的 static
pub static CALLBACK: unsafe extern "system" fn(*mut c_void, u32, *mut c_void) = tls_callback;

unsafe extern "system" fn tls_callback(_h: *mut c_void, dw_reason: u32, _pv: *mut c_void) {
    if dw_reason == c::DLL_THREAD_DETACH || dw_reason == c::DLL_PROCESS_DETACH {
        unsafe {
            #[cfg(target_thread_local)]
            super::super::destructors::run();
            #[cfg(not(target_thread_local))]
            super::super::key::run_dtors();

            crate::rt::thread_cleanup();
        }
    }
}
