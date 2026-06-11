cfg_select! {
    target_os = "linux" => {
        /// 针对 <https://github.com/rust-lang/rust/issues/126600> 的缓解措施。
        ///
        /// 在 glibc 上，曾观察到 `libc::exit` 并不总是线程安全的。
        /// 目前尚不清楚这究竟是 glibc 的 bug，还是标准所允许的行为。
        /// 为缓解该问题，我们在调用 `libc::exit`（或从 `main` 返回）之前先调用本函数，
        /// 以确保只有一个 Rust 线程会去调用 `libc::exit`（或从 `main` 返回）。
        ///
        /// 严格来说，这并不足以保证健全性（soundness），因为其他直接调用
        /// `libc::exit` 的代码仍会与本机制发生竞争。
        ///
        /// *本函数自身并不调用 `libc::exit`。* 这样设计是为了它也能用来
        /// 守卫从 `main` 返回的情形。
        ///
        /// 本函数在一个进程中只有第一次被调用时才会返回。
        ///
        /// * 如果它在首次调用的同一线程上再次被调用，将会 abort（中止进程）。
        /// * 如果它在另一个线程上再次被调用，将会在一个循环里等待
        ///   （等待进程退出）。
        #[cfg_attr(any(test, doctest), allow(dead_code))]
        pub(crate) fn unique_thread_exit() {
            use crate::ffi::c_int;
            use crate::ptr;
            use crate::sync::atomic::AtomicPtr;
            use crate::sync::atomic::Ordering::{Acquire, Relaxed};

            static EXITING_THREAD_ID: AtomicPtr<c_int> = AtomicPtr::new(ptr::null_mut());

            // 我们用 `errno` 的地址作为一种廉价且安全的线程标识方式。由于 C 标准
            // 规定 `errno` 必须具有线程存储期（thread storage duration），我们可以
            // 依赖它的地址在该线程的整个生命周期内保持不变。此外，对 `errno` 的访问
            // 是异步信号安全（async-signal-safe）的，因此本函数在任何可以想象到的
            // 情形下都可用。
            let this_thread_id = crate::sys::io::errno_location();
            match EXITING_THREAD_ID.compare_exchange(ptr::null_mut(), this_thread_id, Acquire, Relaxed) {
                Ok(_) => {
                    // 这是第一个调用 `unique_thread_exit` 的线程，
                    // 而且是第一次调用。继续执行退出流程。
                }
                Err(exiting_thread_id) if exiting_thread_id == this_thread_id => {
                    // 这是第一个调用 `unique_thread_exit` 的线程，
                    // 但已经是第二次调用了。
                    // 中止进程。
                    core::panicking::panic_nounwind("std::process::exit called re-entrantly")
                }
                Err(_) => {
                    // 这不是第一个调用 `unique_thread_exit` 的线程。
                    // 暂停（pause）直到进程退出。
                    loop {
                        // Safety: 调用 libc::pause 是安全的。
                        unsafe { libc::pause(); }
                    }
                }
            }
        }
    }
    _ => {
        /// 针对 <https://github.com/rust-lang/rust/issues/126600> 的缓解措施。
        ///
        /// 在本平台上 ***未*** 实现该缓解措施，要么是因为本平台
        /// 不受此问题影响，要么是因为尚未为本平台实现缓解措施。
        #[cfg_attr(any(test, doctest), allow(dead_code))]
        pub(crate) fn unique_thread_exit() {
            // 在 `exit` 本身就线程安全的平台上无需缓解措施。
        }
    }
}
