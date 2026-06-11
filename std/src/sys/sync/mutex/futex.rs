use crate::sync::atomic::Ordering::{Acquire, Relaxed, Release};
use crate::sys::futex::{self, futex_wait, futex_wake};

type Futex = futex::SmallFutex;
type State = futex::SmallPrimitive;

pub struct Mutex {
    futex: Futex,
}

const UNLOCKED: State = 0;
const LOCKED: State = 1; // 已锁定，没有其他线程在等待
const CONTENDED: State = 2; // 已锁定，且有其他线程在等待（处于争用状态 contended）

impl Mutex {
    #[inline]
    pub const fn new() -> Self {
        Self { futex: Futex::new(UNLOCKED) }
    }

    #[inline]
    // 将其标记为诊断项（diagnostic item），供 Miri 的并发模型检查器识别。
    #[cfg_attr(not(test), rustc_diagnostic_item = "sys_mutex_try_lock")]
    pub fn try_lock(&self) -> bool {
        self.futex.compare_exchange(UNLOCKED, LOCKED, Acquire, Relaxed).is_ok()
    }

    #[inline]
    // 将其标记为诊断项（diagnostic item），供 Miri 的并发模型检查器识别。
    #[cfg_attr(not(test), rustc_diagnostic_item = "sys_mutex_lock")]
    pub fn lock(&self) {
        if self.futex.compare_exchange(UNLOCKED, LOCKED, Acquire, Relaxed).is_err() {
            self.lock_contended();
        }
    }

    #[cold]
    fn lock_contended(&self) {
        // 先自旋（spin），以便在锁很快被释放时加快获取速度。
        let mut state = self.spin();

        // 如果此刻锁已是未锁定状态，则尝试拿锁，
        // 且不把它标记为争用（contended）状态。
        if state == UNLOCKED {
            match self.futex.compare_exchange(UNLOCKED, LOCKED, Acquire, Relaxed) {
                Ok(_) => return, // 加锁成功！
                Err(s) => state = s,
            }
        }

        loop {
            // 把锁置为争用（contended）状态。
            // 如果它已经是 CONTENDED，我们就避免一次不必要的写操作，
            // 以对缓存更友好。
            if state != CONTENDED && self.futex.swap(CONTENDED, Acquire) == UNLOCKED {
                // 我们把它从 UNLOCKED 改成了 CONTENDED，所以此刻已成功加锁。
                return;
            }

            // 在假定 futex 仍为 CONTENDED 的前提下，等待它改变状态。
            futex_wait(&self.futex, CONTENDED, None);

            // 被唤醒后再次自旋。
            state = self.spin();
        }
    }

    fn spin(&self) -> State {
        let mut spin = 100;
        loop {
            // 自旋期间我们只使用 `load`（而不是 `swap` 或 `compare_exchange`），
            // 以对缓存更友好。
            let state = self.futex.load(Relaxed);

            // 当 mutex 处于 UNLOCKED 时停止自旋，
            // 当它处于 CONTENDED 时也停止自旋。
            if state != LOCKED || spin == 0 {
                return state;
            }

            crate::hint::spin_loop();
            spin -= 1;
        }
    }

    #[inline]
    // 将其标记为诊断项（diagnostic item），供 Miri 的并发模型检查器识别。
    #[cfg_attr(not(test), rustc_diagnostic_item = "sys_mutex_unlock")]
    pub unsafe fn unlock(&self) {
        if self.futex.swap(UNLOCKED, Release) == CONTENDED {
            // 我们只唤醒一个线程。当那个线程拿到 mutex 时，它会把 mutex 标记为
            // CONTENDED（见上文的 lock_contended），从而确保其他正在等待的线程
            // 最终也都会被唤醒。
            self.wake();
        }
    }

    #[cold]
    fn wake(&self) {
        futex_wake(&self.futex);
    }
}
