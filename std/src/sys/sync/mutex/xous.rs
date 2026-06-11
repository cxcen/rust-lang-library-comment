use crate::os::xous::ffi::{blocking_scalar, do_yield};
use crate::os::xous::services::{TicktimerScalar, ticktimer_server};
use crate::sync::atomic::Ordering::{Acquire, Relaxed, Release};
use crate::sync::atomic::{Atomic, AtomicBool, AtomicUsize};

pub struct Mutex {
    /// "locked"（已锁定）的取值表示有多少个线程正在这个 Mutex 上等待。
    /// 可能的取值为：
    ///     0: 锁处于未锁定状态
    ///     1: 锁已锁定且无争用（uncontended）
    ///   >=2: 锁已锁定且存在争用（contended）
    ///
    /// 当有不止一个线程在等待锁，或者锁被长时间持有时，该锁就处于「争用」
    ///（contended）状态。这些锁不会自旋，而是向 ticktimer 服务器发送一条
    /// Message，请求在锁被解锁时唤醒它们。
    locked: Atomic<usize>,

    /// 这个 Mutex 是否曾经处于争用状态，因而曾经与 ticktimer 服务器打过交道。
    /// 如果这个标志从未被设置过，那么我们就从未进入过慢速路径，于是可以跳过
    /// 对该 mutex 的注销（deregister）。
    contended: Atomic<bool>,
}

impl Mutex {
    #[inline]
    pub const fn new() -> Mutex {
        Mutex { locked: AtomicUsize::new(0), contended: AtomicBool::new(false) }
    }

    fn index(&self) -> usize {
        core::ptr::from_ref(self).addr()
    }

    #[inline]
    pub unsafe fn lock(&self) {
        // 多次尝试获取锁，且不动用 ticktimer 服务器。对于只被短暂持有的锁，
        // 这会使 ticktimer 服务器永远不会被调用。此时 `locked` 的取值要么是 0
        // 要么是 1。
        for _attempts in 0..3 {
            if unsafe { self.try_lock() } {
                return;
            }
            do_yield();
        }

        // 再尝试加锁一次。如果在上一段代码与此处之间锁被释放了，那么到这一步结束时
        // 内部的 `locked` 值会是 1。如果它当时未被锁定，那么该值会大于 1，例如当有
        // 多个其他线程正在这个锁上等待时。
        if unsafe { self.try_lock_or_poison() } {
            return;
        }

        // 当这个 mutex 被 drop 时，我们将需要向服务器注销它。
        self.contended.store(true, Relaxed);

        // 现在锁处于「争用」（contended）状态。当锁被释放时，会有一条 Message 被发往
        // ticktimer 服务器以将其唤醒。注意这可能已经发生了，所以 `lock` 的实际取值
        // 可能是任意值（0、1、2、……）。
        blocking_scalar(
            ticktimer_server(),
            crate::os::xous::services::TicktimerScalar::LockMutex(self.index()).into(),
        )
        .expect("failure to send LockMutex command");
    }

    #[inline]
    pub unsafe fn unlock(&self) {
        let prev = self.locked.fetch_sub(1, Release);

        // 如果之前的值是 1，那么这是一次「快速路径」（fast path）解锁，因此无需
        // 牵涉 Ticktimer 服务器。
        if prev == 1 {
            return;
        }

        // 如果之前的值是 0，那么出了严重错误，计数器刚刚发生了环绕（wrap around）。
        if prev == 0 {
            panic!("mutex lock count underflowed");
        }

        // 解除阻塞一个正在等待这条 message 的线程。
        blocking_scalar(ticktimer_server(), TicktimerScalar::UnlockMutex(self.index()).into())
            .expect("failure to send UnlockMutex command");
    }

    #[inline]
    pub unsafe fn try_lock(&self) -> bool {
        self.locked.compare_exchange(0, 1, Acquire, Relaxed).is_ok()
    }

    #[inline]
    pub unsafe fn try_lock_or_poison(&self) -> bool {
        self.locked.fetch_add(1, Acquire) == 0
    }
}

impl Drop for Mutex {
    fn drop(&mut self) {
        // 如果曾经发生过 Mutex 争用，那么我们就牵涉过 ticktimer。在这个 Mutex 被
        // 释放（deallocate）时，释放与之关联的资源。
        if self.contended.load(Relaxed) {
            blocking_scalar(ticktimer_server(), TicktimerScalar::FreeMutex(self.index()).into())
                .ok();
        }
    }
}
