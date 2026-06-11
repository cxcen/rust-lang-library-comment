#![forbid(unsafe_op_in_unsafe_fn)]

use crate::pin::Pin;
use crate::ptr;
use crate::sync::atomic::Ordering::Relaxed;
use crate::sync::atomic::{Atomic, AtomicUsize};
use crate::sys::pal::sync as pal;
use crate::sys::sync::{Mutex, OnceBox};
use crate::time::{Duration, Instant};

pub struct Condvar {
    cvar: OnceBox<pal::Condvar>,
    mutex: Atomic<usize>,
}

impl Condvar {
    pub const fn new() -> Condvar {
        Condvar { cvar: OnceBox::new(), mutex: AtomicUsize::new(0) }
    }

    #[inline]
    fn get(&self) -> Pin<&pal::Condvar> {
        self.cvar.get_or_init(|| {
            let mut cvar = Box::pin(pal::Condvar::new());
            // SAFETY: 我们对每个 `pal::Condvar` 只调用一次 `init`，也就是在这里。
            unsafe { cvar.as_mut().init() };
            cvar
        })
    }

    #[inline]
    fn verify(&self, mutex: Pin<&pal::Mutex>) {
        let addr = ptr::from_ref::<pal::Mutex>(&mutex).addr();
        // 这里用 Relaxed 没问题，因为我们从不通过 `self.mutex` 进行读取，仅用它来
        // 比较地址。
        match self.mutex.compare_exchange(0, addr, Relaxed, Relaxed) {
            Ok(_) => {}               // 成功存入了该地址
            Err(n) if n == addr => {} // 在存入相同地址的竞争中落败
            _ => panic!("attempted to use a condition variable with two mutexes"),
        }
    }

    #[inline]
    pub fn notify_one(&self) {
        // SAFETY: 我们在上面调用了 `init`。
        unsafe { self.get().notify_one() }
    }

    #[inline]
    pub fn notify_all(&self) {
        // SAFETY: 我们在上面调用了 `init`。
        unsafe { self.get().notify_all() }
    }

    #[inline]
    pub unsafe fn wait(&self, mutex: &Mutex) {
        // SAFETY: 调用者保证该锁已被持有，因此 mutex 必定已经初始化过了。
        let mutex = unsafe { mutex.pal.get_unchecked() };
        self.verify(mutex);
        // SAFETY: 我们在上面调用了 `init`，并验证了这个条件变量只与 `mutex` 一起
        // 使用，而调用者保证 `mutex` 已被当前线程加锁。
        unsafe { self.get().wait(mutex) }
    }

    pub unsafe fn wait_timeout(&self, mutex: &Mutex, dur: Duration) -> bool {
        // SAFETY: 调用者保证该锁已被持有，因此 mutex 必定已经初始化过了。
        let mutex = unsafe { mutex.pal.get_unchecked() };
        self.verify(mutex);

        if pal::Condvar::PRECISE_TIMEOUT {
            // SAFETY: 我们在上面调用了 `init`，并验证了这个条件变量只与 `mutex` 一起
            // 使用，而调用者保证 `mutex` 已被当前线程加锁。
            unsafe { self.get().wait_timeout(mutex, dur) }
        } else {
            // 超时报告并不可靠，所以我们自己来做检查。
            let now = Instant::now();
            // SAFETY: 我们在上面调用了 `init`，并验证了这个条件变量只与 `mutex` 一起
            // 使用，而调用者保证 `mutex` 已被当前线程加锁。
            let woken = unsafe { self.get().wait_timeout(mutex, dur) };
            woken || now.elapsed() < dur
        }
    }
}
