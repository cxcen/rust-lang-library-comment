//! 针对基于 Darwin 的系统的线程驻留（thread parking）。
//!
//! Darwin 其实有 futex 系统调用（`__ulock_wait`/`__ulock_wake`），但它们不能
//! 在 `std` 中使用，因为它们是非公开的（使用它们会导致被 App Store 拒绝）。
//!
//! 因此，我们需要寻找其他同步原语。幸运的是，Darwin 支持信号量（semaphores），
//! 这让我们只用一个原语（而非一对 mutex-condvar）就能实现所需的行为。我们使用
//! 由 libdispatch 提供的信号量，因为底层的 Mach 信号量其公开性颇为可疑。

#![allow(non_camel_case_types)]

use crate::pin::Pin;
use crate::sync::atomic::Ordering::{Acquire, Release};
use crate::sync::atomic::{Atomic, AtomicI8};
use crate::time::Duration;

type dispatch_semaphore_t = *mut crate::ffi::c_void;
type dispatch_time_t = u64;

const DISPATCH_TIME_NOW: dispatch_time_t = 0;
const DISPATCH_TIME_FOREVER: dispatch_time_t = !0;

// 包含在 libSystem.dylib 中，该库默认会被链接。
unsafe extern "C" {
    fn dispatch_time(when: dispatch_time_t, delta: i64) -> dispatch_time_t;
    fn dispatch_semaphore_create(val: isize) -> dispatch_semaphore_t;
    fn dispatch_semaphore_wait(dsema: dispatch_semaphore_t, timeout: dispatch_time_t) -> isize;
    fn dispatch_semaphore_signal(dsema: dispatch_semaphore_t) -> isize;
    fn dispatch_release(object: *mut crate::ffi::c_void);
}

const EMPTY: i8 = 0;
const NOTIFIED: i8 = 1;
const PARKED: i8 = -1;

pub struct Parker {
    semaphore: dispatch_semaphore_t,
    state: Atomic<i8>,
}

unsafe impl Sync for Parker {}
unsafe impl Send for Parker {}

impl Parker {
    pub unsafe fn new_in_place(parker: *mut Parker) {
        let semaphore = dispatch_semaphore_create(0);
        assert!(
            !semaphore.is_null(),
            "failed to create dispatch semaphore for thread synchronization"
        );
        parker.write(Parker { semaphore, state: AtomicI8::new(EMPTY) })
    }

    // 不需要 `Pin`，但其他实现需要。
    pub unsafe fn park(self: Pin<&Self>) {
        // 此刻信号量计数必须为零，因为在我们发出「正在等待」的信号之前，
        // 执行 unpark 的线程并不会真正增加它。

        // 把 NOTIFIED 改为 EMPTY，把 EMPTY 改为 PARKED。
        if self.state.fetch_sub(1, Acquire) == NOTIFIED {
            return;
        }

        // 从这一刻起，另一个线程可能会增加信号量计数。
        // 如果它比我们快，我们会在下面立即把它再减回去。
        // 如果我们更快，我们就等待。

        // 确保信号量计数确实已被减少，即使该调用因某种原因超时了也是如此。
        while dispatch_semaphore_wait(self.semaphore, DISPATCH_TIME_FOREVER) != 0 {}

        // 到这一点，信号量计数又重新变回了零。

        // 我们肯定是被唤醒的，所以不需要检查 state。
        // 不过，我们仍然需要用一次 swap 来重置 state，以便用 acquire 内存序
        // 观测到状态变化。
        self.state.swap(EMPTY, Acquire);
    }

    // 不需要 `Pin`，但其他实现需要。
    pub unsafe fn park_timeout(self: Pin<&Self>, dur: Duration) {
        if self.state.fetch_sub(1, Acquire) == NOTIFIED {
            return;
        }

        let nanos = dur.as_nanos().try_into().unwrap_or(i64::MAX);
        let timeout = dispatch_time(DISPATCH_TIME_NOW, nanos);

        let timeout = dispatch_semaphore_wait(self.semaphore, timeout) != 0;

        let state = self.state.swap(EMPTY, Acquire);
        if state == NOTIFIED && timeout {
            // 如果 state 曾是 NOTIFIED，但 semaphore_wait 因超时而返回、且没有
            // 减少计数，这意味着另一个线程即将调用 semaphore_signal。我们必须
            // 等待那次调用发生，以确保信号量计数被重置。
            while dispatch_semaphore_wait(self.semaphore, DISPATCH_TIME_FOREVER) != 0 {}
        } else {
            // 要么是发生了超时、且我们在任何线程试图唤醒我们之前就重置了 state；
            // 要么是我们被唤醒并重置了 state，并确保用 acquire 内存序观测到了
            // 状态变化。无论哪种情况，信号量计数现在都重新变回了零。
        }
    }

    // 不需要 `Pin`，但其他实现需要。
    pub fn unpark(self: Pin<&Self>) {
        let state = self.state.swap(NOTIFIED, Release);
        if state == PARKED {
            unsafe {
                dispatch_semaphore_signal(self.semaphore);
            }
        }
    }
}

impl Drop for Parker {
    fn drop(&mut self) {
        // SAFETY:
        // 我们总是确保信号量计数被重置，所以这绝不会引发异常。
        unsafe {
            dispatch_release(self.semaphore);
        }
    }
}
