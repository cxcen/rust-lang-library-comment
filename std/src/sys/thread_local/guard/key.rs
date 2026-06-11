//! 很多 UNIX 平台都没有为原生 TLS 注册 TLS 析构函数（destructors）的专门办法。
//! 因此，我们改用一个带析构函数的 TLS key，由该析构函数运行析构函数列表中
//! 所有的原生 TLS 析构函数。

use crate::ptr;
use crate::sys::thread_local::key::{LazyKey, set};

#[cfg(target_thread_local)]
pub fn enable() {
    use crate::sys::thread_local::destructors;

    static DTORS: LazyKey = LazyKey::new(Some(run));

    // 把 key 的值设置为 NULL 以外的某个值，会导致该析构函数在线程退出时被运行。
    unsafe {
        set(DTORS.force(), ptr::without_provenance_mut(1));
    }

    unsafe extern "C" fn run(_: *mut u8) {
        unsafe {
            destructors::run();
            // 在拥有 `__cxa_thread_atexit_impl` 的平台上，由于 TLS 析构函数
            // 已经向系统注册，因此在较新的系统上 `destructors::run` 什么也不做。
            // 但因为所有这些平台都会在已注册的析构函数之后才调用 TLS key 的
            // 析构函数，所以（在撰写本文时）此函数仍会被最后运行。
            crate::rt::thread_cleanup();
        }
    }
}

/// 在采用基于 key 的 TLS 的平台上，系统会替我们运行析构函数。
/// 不过，我们仍然必须确保 [`crate::rt::thread_cleanup`] 被调用。
/// 这是通过把一个 TLS 析构函数的执行推迟到 TLS 析构过程的下一轮来实现的。
#[cfg(not(target_thread_local))]
pub fn enable() {
    const DEFER: *mut u8 = ptr::without_provenance_mut(1);
    const RUN: *mut u8 = ptr::without_provenance_mut(2);

    static CLEANUP: LazyKey = LazyKey::new(Some(run));

    unsafe { set(CLEANUP.force(), DEFER) }

    unsafe extern "C" fn run(state: *mut u8) {
        if state == DEFER {
            // 确保此函数在 TLS 析构的下一轮中会再次被运行。如果没有下一轮，
            // 就会发生泄漏，但这没关系，`thread_cleanup` 并不保证一定会被调用。
            unsafe { set(CLEANUP.force(), RUN) }
        } else {
            debug_assert_eq!(state, RUN);
            // 如果在 TLS 析构的下一轮中状态仍然是 RUN，就意味着本运行时定义的
            // 其他 TLS 析构函数都没有被运行过，否则它们会把状态设置为 DEFER。
            crate::rt::thread_cleanup();
        }
    }
}
