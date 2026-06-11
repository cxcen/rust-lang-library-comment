use core::sync::atomic::{Atomic, AtomicUsize, Ordering};

use crate::os::xous::ffi::{blocking_scalar, scalar};
use crate::os::xous::services::{TicktimerScalar, ticktimer_server};
use crate::sys::sync::Mutex;
use crate::time::Duration;

// 本实现的灵感来自 Andrew D. Birrell 的论文
// "Implementing Condition Variables with Semaphores"

const NOTIFY_TRIES: usize = 3;

pub struct Condvar {
    counter: Atomic<usize>,
    timed_out: Atomic<usize>,
}

unsafe impl Send for Condvar {}
unsafe impl Sync for Condvar {}

impl Condvar {
    #[inline]
    pub const fn new() -> Condvar {
        Condvar { counter: AtomicUsize::new(0), timed_out: AtomicUsize::new(0) }
    }

    fn notify_some(&self, to_notify: usize) {
        // 假设：保护这个 condvar 的 Mutex 在本次调用的整个过程中都处于加锁状态，
        // 从而阻止对 `wait` 和 `wait_timeout` 的调用。

        // 逻辑检查：确保没有任何遗漏的等待者。移除所有已超时的等待者，并确保计数器
        // 不会下溢（underflow）。
        assert!(self.timed_out.load(Ordering::Relaxed) <= self.counter.load(Ordering::Relaxed));
        self.counter.fetch_sub(self.timed_out.swap(0, Ordering::Relaxed), Ordering::Relaxed);

        // 算出要通知多少个线程。注意：由于 Mutex 处于加锁状态，`counter` 在本次操作
        // 期间不可能增加。然而，`counter` 有可能因为某个 condvar 超时而减少，在那种
        // 情况下对应的 `timed_out` 会相应增加。
        let Ok(waiter_count) =
            self.counter.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |counter| {
                if counter == 0 {
                    return None;
                } else {
                    Some(counter - counter.min(to_notify))
                }
            })
        else {
            // 没有线程在这个 condvar 上等待
            return;
        };

        let mut remaining_to_wake = waiter_count.min(to_notify);
        if remaining_to_wake == 0 {
            return;
        }
        for _wake_tries in 0..NOTIFY_TRIES {
            let result = blocking_scalar(
                ticktimer_server(),
                TicktimerScalar::NotifyCondition(self.index(), remaining_to_wake).into(),
            )
            .expect("failure to send NotifyCondition command");

            // 减去已被通知的那部分等待者
            remaining_to_wake -= result[0];

            // 同时减去已超时的等待者数量。把它向下钳制（clamp）到 0，以确保我们不会
            // 永远等待下去——以防某个等待者在「我们统计剩余等待者」与「此刻」之间醒来。
            remaining_to_wake =
                remaining_to_wake.saturating_sub(self.timed_out.swap(0, Ordering::Relaxed));
            if remaining_to_wake == 0 {
                return;
            }
            crate::thread::yield_now();
        }
    }

    pub fn notify_one(&self) {
        self.notify_some(1)
    }

    pub fn notify_all(&self) {
        self.notify_some(self.counter.load(Ordering::Relaxed))
    }

    fn index(&self) -> usize {
        core::ptr::from_ref(self).addr()
    }

    /// 解锁给定的 Mutex 并等待通知。最多等待 `ms` 毫秒，传入 `0` 则永远等待。
    ///
    /// 如果收到了条件通知则返回 `true`，如果超时则返回 `false`
    fn wait_ms(&self, mutex: &Mutex, ms: usize) -> bool {
        self.counter.fetch_add(1, Ordering::Relaxed);
        unsafe { mutex.unlock() };

        // 线程安全考量：有可能 `notify` 线程会在我们还没来得及等待条件之前就在这里
        // 醒来。这没有问题，因为我们已经通过把计数器加一记录了「我们正在等待」这一
        // 事实。
        let result = blocking_scalar(
            ticktimer_server(),
            TicktimerScalar::WaitForCondition(self.index(), ms).into(),
        );
        let awoken = result.expect("Ticktimer: failure to send WaitForCondition command")[0] == 0;

        // 如果我们是因为超时而醒来的，则把 `timed_out` 计数器加一，以便 `notify` 的
        // 主循环知道发生了一次超时。
        //
        // 这是在 Mutex 仍处于解锁状态时完成的，因为 Mutex 可能仍被上面的 `notify`
        // 过程所持有。
        if !awoken {
            self.timed_out.fetch_add(1, Ordering::Relaxed);
        }

        unsafe { mutex.lock() };
        awoken
    }

    pub unsafe fn wait(&self, mutex: &Mutex) {
        // 等待 0 毫秒，这是表示「永远等待」的一个特殊情形
        self.wait_ms(mutex, 0);
    }

    pub unsafe fn wait_timeout(&self, mutex: &Mutex, dur: Duration) -> bool {
        let mut millis = dur.as_millis() as usize;
        // 确保我们不会等待 0 毫秒，因为那会导致我们永远等待下去
        if millis == 0 {
            millis = 1;
        }
        self.wait_ms(mutex, millis)
    }
}

impl Drop for Condvar {
    fn drop(&mut self) {
        let remaining_count = self.counter.load(Ordering::Relaxed);
        let timed_out = self.timed_out.load(Ordering::Relaxed);
        assert!(
            remaining_count - timed_out == 0,
            "counter was {} and timed_out was {} not 0",
            remaining_count,
            timed_out
        );
        scalar(ticktimer_server(), TicktimerScalar::FreeCondition(self.index()).into()).ok();
    }
}
