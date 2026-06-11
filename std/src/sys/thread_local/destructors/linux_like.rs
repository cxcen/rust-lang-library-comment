//! 针对 Linux 类系统的析构函数（destructor）注册。
//!
//! 大约从 2.18 版本起，glibc 开始提供 `__cxa_thread_atexit_impl` 符号，
//! GCC 和 clang 都用它来为 C++ thread_local 全局变量调用析构函数。
//! 这个函数所做的事情正是我们想要的：它调度一个回调，该回调将在线程退出时
//! 以提供的参数被运行。
//!
//! 遗憾的是，（在撰写本文时）我们所支持的最低 glibc 版本是 2.17，
//! 所以我们只能弱链接（link weakly）此符号，并需要用
//! [`list`](super::list) 析构函数实现作为回退方案。

use crate::mem::transmute;

pub unsafe fn register(t: *mut u8, dtor: unsafe extern "C" fn(*mut u8)) {
    /// 这是必要的，因为 std 默认链接到的 __cxa_thread_atexit_impl 实现
    /// 可能是一个 C 或 C++ 实现，且它没有使用 Clang 的整数规范化
    ///（integer normalization）选项编译。
    #[cfg(sanitizer_cfi_normalize_integers)]
    use core::ffi::c_int;
    #[cfg(not(sanitizer_cfi_normalize_integers))]
    #[cfi_encoding = "i"]
    #[repr(transparent)]
    #[allow(non_camel_case_types)]
    pub struct c_int(#[allow(dead_code)] pub core::ffi::c_int);

    unsafe extern "C" {
        #[linkage = "extern_weak"]
        static __dso_handle: *mut u8;
        #[linkage = "extern_weak"]
        static __cxa_thread_atexit_impl: Option<
            extern "C" fn(
                unsafe extern "C" fn(*mut libc::c_void),
                *mut libc::c_void,
                *mut libc::c_void,
            ) -> c_int,
        >;
    }

    if let Some(f) = unsafe { __cxa_thread_atexit_impl } {
        unsafe {
            f(
                transmute::<unsafe extern "C" fn(*mut u8), unsafe extern "C" fn(*mut libc::c_void)>(
                    dtor,
                ),
                t.cast(),
                (&raw const __dso_handle) as *mut _,
            );
        }
    } else {
        unsafe {
            super::list::register(t, dtor);
        }
    }
}
