//! 一个用于同步原语的简单队列实现。
//!
//! 这个队列被用来实现条件变量与互斥锁。
//!
//! 本 API 的使用者应当使用 `WaitVariable<T>` 类型。由于该类型不是 `Sync`，
//! 它需要由例如 `SpinMutex` 之类的同步原语加以保护，以允许共享访问。
//!
//! 由于用户空间（userspace）可能发送虚假唤醒（spurious wake-up），唤醒事件状态
//! 被记录在 enclave 内。该唤醒事件状态由一个自旋锁（spinlock）保护。队列及其
//! 关联的等待状态存储在一个 `WaitVariable` 中。

#[cfg(test)]
mod tests;

mod spin_mutex;
mod unsafe_list;

use fortanix_sgx_abi::{EV_UNPARK, Tcs, WAIT_INDEFINITE};

pub use self::spin_mutex::{SpinMutex, SpinMutexGuard, try_lock_or_false};
use self::unsafe_list::{UnsafeList, UnsafeListEntry};
use super::abi::{thread, usercalls};
use crate::num::NonZero;
use crate::ops::{Deref, DerefMut};
use crate::panic::{self, AssertUnwindSafe};
use crate::time::Duration;

/// `WaitQueue` 中的一个队列条目（entry）。
struct WaitEntry {
    /// 正在等待的线程的 TCS 地址
    tcs: Tcs,
    /// 该线程是否已被通知唤醒
    wake: bool,
}

/// 与 `WaitQueue` 一同存储的数据。由于该类型本身不是 `Sync`，这确保了对队列与
/// 该数据的访问是同步的。
///
/// 本 API 的使用者应当使用一种同步原语（如 `SpinMutex`）来进行共享访问。
#[derive(Default)]
pub struct WaitVariable<T> {
    queue: WaitQueue,
    lock: T,
}

impl<T> WaitVariable<T> {
    pub const fn new(var: T) -> Self {
        WaitVariable { queue: WaitQueue::new(), lock: var }
    }

    pub fn lock_var(&self) -> &T {
        &self.lock
    }

    pub fn lock_var_mut(&mut self) -> &mut T {
        &mut self.lock
    }
}

#[derive(Copy, Clone)]
pub enum NotifiedTcs {
    Single(Tcs),
    All { _count: NonZero<usize> },
}

/// 一个 RAII 守卫，在 drop 时会通知一组目标线程，并同时解锁一个互斥锁。
pub struct WaitGuard<'a, T: 'a> {
    mutex_guard: Option<SpinMutexGuard<'a, WaitVariable<T>>>,
    notified_tcs: NotifiedTcs,
}

/// 一个正在某个同步原语上等待的线程队列。
///
/// `UnsafeList` 的条目（entry）被分配在等待线程的栈上。这避免了堆分配器中可能
/// 发生的任何全局加锁。这是安全的，因为等待线程在被通知之前不会从那个栈帧返回。
/// 通知线程会确保在发送唤醒事件之前清理掉对这些链表条目的任何引用。
pub struct WaitQueue {
    // 我们在这里使用一个内部 Mutex，以在面对虚假唤醒（spurious wakeup）时保护数据。
    inner: UnsafeList<SpinMutex<WaitEntry>>,
}
unsafe impl Send for WaitQueue {}

impl Default for WaitQueue {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a, T> Deref for WaitGuard<'a, T> {
    type Target = SpinMutexGuard<'a, WaitVariable<T>>;

    fn deref(&self) -> &Self::Target {
        self.mutex_guard.as_ref().unwrap()
    }
}

impl<'a, T> DerefMut for WaitGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.mutex_guard.as_mut().unwrap()
    }
}

impl<'a, T> Drop for WaitGuard<'a, T> {
    fn drop(&mut self) {
        drop(self.mutex_guard.take());
        let target_tcs = match self.notified_tcs {
            NotifiedTcs::Single(tcs) => Some(tcs),
            NotifiedTcs::All { .. } => None,
        };
        rtunwrap!(Ok, usercalls::send(EV_UNPARK, target_tcs));
    }
}

impl WaitQueue {
    pub const fn new() -> Self {
        WaitQueue { inner: UnsafeList::new() }
    }

    /// 将调用线程加入到 `WaitVariable` 的等待队列中，然后等待，直到出现唤醒事件。
    ///
    /// 在本线程被唤醒之前，此函数不会返回。当 `before_wait` panic 时，此函数将
    /// 中止（abort）。
    pub fn wait<T, F: FnOnce()>(mut guard: SpinMutexGuard<'_, WaitVariable<T>>, before_wait: F) {
        // 非常 unsafe：请核对 UnsafeList::push 的各项要求
        unsafe {
            let mut entry = UnsafeListEntry::new(SpinMutex::new(WaitEntry {
                tcs: thread::current(),
                wake: false,
            }));
            let entry = guard.queue.inner.push(&mut entry);
            drop(guard);
            if let Err(_e) = panic::catch_unwind(AssertUnwindSafe(|| before_wait())) {
                rtabort!("Panic before wait on wakeup event")
            }
            while !entry.lock().wake {
                // `entry.wake` 只在 `notify_one` 和 `notify_all` 函数中被设置。二者都
                // 确保在设置该 bool 之前先把该 entry 从队列中移除。不存在对 `entry`
                // 的其他引用。
                // 不要 panic，否则会在栈展开（unwinding）期间使 `entry` 失效
                let eventset = rtunwrap!(Ok, usercalls::wait(EV_UNPARK, WAIT_INDEFINITE));
                rtassert!(eventset & EV_UNPARK == EV_UNPARK);
            }
        }
    }

    /// 将调用线程加入到 `WaitVariable` 的等待队列中，然后等待，直到出现唤醒事件
    /// 或超时。如果观察到了事件，则返回 true。否则，它会把调用线程从等待队列中
    /// 移除。当 `before_wait` panic 时，此函数将中止（abort）。
    pub fn wait_timeout<T, F: FnOnce()>(
        lock: &SpinMutex<WaitVariable<T>>,
        timeout: Duration,
        before_wait: F,
    ) -> bool {
        // 非常 unsafe：请核对 UnsafeList::push 的各项要求
        unsafe {
            let mut entry = UnsafeListEntry::new(SpinMutex::new(WaitEntry {
                tcs: thread::current(),
                wake: false,
            }));
            let entry_lock = lock.lock().queue.inner.push(&mut entry);
            if let Err(_e) = panic::catch_unwind(AssertUnwindSafe(|| before_wait())) {
                rtabort!("Panic before wait on wakeup event or timeout")
            }
            usercalls::wait_timeout(EV_UNPARK, timeout, || entry_lock.lock().wake);
            // 先获取等待队列的锁，以避免死锁，并确保没有其他函数能够同时访问该链表
            // （例如 `notify_one` 或 `notify_all`）
            let mut guard = lock.lock();
            let success = entry_lock.lock().wake;
            if !success {
                // 没有人在唤醒我们，所以把我们的 entry 从等待队列中移除。
                guard.queue.inner.remove(&mut entry);
            }
            success
        }
    }

    /// 要么在等待队列上找到下一个等待者，要么原样返回该 mutex 守卫。
    ///
    /// 如果找到了一个等待者，则返回一个 `WaitGuard`，它会在被 drop 时通知该
    /// 等待者。
    pub fn notify_one<T>(
        mut guard: SpinMutexGuard<'_, WaitVariable<T>>,
    ) -> Result<WaitGuard<'_, T>, SpinMutexGuard<'_, WaitVariable<T>>> {
        // SAFETY: pop() 返回值的生命周期被限定在 map 闭包内（该闭包的返回值是
        // 'static 的）。在队列上的锁被释放（即 `guard` 被 drop）之前，底层栈帧
        // 不会被释放。
        unsafe {
            let tcs = guard.queue.inner.pop().map(|entry| -> Tcs {
                let mut entry_guard = entry.lock();
                entry_guard.wake = true;
                entry_guard.tcs
            });

            if let Some(tcs) = tcs {
                Ok(WaitGuard { mutex_guard: Some(guard), notified_tcs: NotifiedTcs::Single(tcs) })
            } else {
                Err(guard)
            }
        }
    }

    /// 要么找出等待队列上所有的等待者，要么原样返回该 mutex 守卫。
    ///
    /// 如果至少找到一个等待者，则返回一个 `WaitGuard`，它会在被 drop 时通知所有
    /// 等待者。
    pub fn notify_all<T>(
        mut guard: SpinMutexGuard<'_, WaitVariable<T>>,
    ) -> Result<WaitGuard<'_, T>, SpinMutexGuard<'_, WaitVariable<T>>> {
        // SAFETY: pop() 各返回值的生命周期被限定在 while 循环体内。在队列上的锁
        // 被释放（即 `guard` 被 drop）之前，底层栈帧不会被释放。
        unsafe {
            let mut count = 0;
            while let Some(entry) = guard.queue.inner.pop() {
                count += 1;
                let mut entry_guard = entry.lock();
                entry_guard.wake = true;
            }

            if let Some(count) = NonZero::new(count) {
                Ok(WaitGuard {
                    mutex_guard: Some(guard),
                    notified_tcs: NotifiedTcs::All { _count: count },
                })
            } else {
                Err(guard)
            }
        }
    }
}
