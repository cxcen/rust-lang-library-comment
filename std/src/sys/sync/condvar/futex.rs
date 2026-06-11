use crate::sync::atomic::Ordering::Relaxed;
use crate::sys::futex::{Futex, futex_wait, futex_wake, futex_wake_all};
use crate::sys::sync::Mutex;
use crate::time::Duration;

pub struct Condvar {
    // 这个原子量的值会在每次 notification（通知）时简单地自增。
    // `.wait()` 用它来确保不会漏掉在「解锁 mutex 之后、开始等待通知之前」
    // 这段窗口里发生的任何通知。
    futex: Futex,
}

impl Condvar {
    #[inline]
    pub const fn new() -> Self {
        Self { futex: Futex::new(0) }
    }

    // 这里所有的内存序都是 `Relaxed`，
    // 因为同步是由对 mutex 的解锁与加锁来完成的。

    pub fn notify_one(&self) {
        self.futex.fetch_add(1, Relaxed);
        futex_wake(&self.futex);
    }

    pub fn notify_all(&self) {
        self.futex.fetch_add(1, Relaxed);
        futex_wake_all(&self.futex);
    }

    pub unsafe fn wait(&self, mutex: &Mutex) {
        self.wait_optional_timeout(mutex, None);
    }

    pub unsafe fn wait_timeout(&self, mutex: &Mutex, timeout: Duration) -> bool {
        self.wait_optional_timeout(mutex, Some(timeout))
    }

    unsafe fn wait_optional_timeout(&self, mutex: &Mutex, timeout: Option<Duration>) -> bool {
        // 在解锁 mutex *之前* 先读取通知计数器的值。
        let futex_value = self.futex.load(Relaxed);

        // 进入休眠前先解锁 mutex。
        mutex.unlock();

        // 等待，但仅当自我们解锁 mutex 以来还没有发生过任何通知时才真正等待。
        let r = futex_wait(&self.futex, futex_value, timeout);

        // 重新对 mutex 加锁。
        mutex.lock();

        r
    }
}
