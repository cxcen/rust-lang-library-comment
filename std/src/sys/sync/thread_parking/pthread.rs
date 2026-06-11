//! 不使用 `futex`、改用 `pthread` 同步原语实现的线程驻留（thread parking）。

use crate::pin::Pin;
use crate::sync::atomic::Ordering::{Acquire, Relaxed, Release};
use crate::sync::atomic::{Atomic, AtomicUsize};
use crate::sys::pal::sync::{Condvar, Mutex};
use crate::time::Duration;

const EMPTY: usize = 0;
const PARKED: usize = 1;
const NOTIFIED: usize = 2;

pub struct Parker {
    state: Atomic<usize>,
    lock: Mutex,
    cvar: Condvar,
}

impl Parker {
    /// 就地（in-place）构造 UNIX parker。
    ///
    /// # 安全性(Safety）
    /// 构造出来的 parker 绝不能被移动。
    pub unsafe fn new_in_place(parker: *mut Parker) {
        parker.write(Parker {
            state: AtomicUsize::new(EMPTY),
            lock: Mutex::new(),
            cvar: Condvar::new(),
        });

        Pin::new_unchecked(&mut (*parker).cvar).init();
    }

    fn lock(self: Pin<&Self>) -> Pin<&Mutex> {
        unsafe { self.map_unchecked(|p| &p.lock) }
    }

    fn cvar(self: Pin<&Self>) -> Pin<&Condvar> {
        unsafe { self.map_unchecked(|p| &p.cvar) }
    }

    // 本实现并不要求 `unsafe`，但其他实现可能假定它只会被拥有该 Parker 的线程调用。
    //
    // 关于内存序，参见 futex.rs
    pub unsafe fn park(self: Pin<&Self>) {
        // 如果我们此前已被通知，那么就消费掉这次通知并快速返回。
        if self.state.compare_exchange(NOTIFIED, EMPTY, Acquire, Relaxed).is_ok() {
            return;
        }

        // 否则我们需要协调进入睡眠
        self.lock().lock();
        match self.state.compare_exchange(EMPTY, PARKED, Relaxed, Relaxed) {
            Ok(_) => {}
            Err(NOTIFIED) => {
                // 即便我们知道它将会是 `NOTIFIED`，这里也必须读取一次。
                // 这是因为自从我们在上面的 `compare_exchange` 中读到 `NOTIFIED` 之后，
                // `unpark` 可能又被调用了。我们必须执行一次与那个 `unpark` 同步的
                // acquire 操作，以观测到它在调用 unpark 之前所做的任何写入。
                // 为此，我们必须从它对 `state` 所做的写入中读取。
                let old = self.state.swap(EMPTY, Acquire);

                self.lock().unlock();

                assert_eq!(old, NOTIFIED, "park state changed unexpectedly");
                return;
            } // 应当消费这次通知，从而禁止下一次 park 中的虚假唤醒（spurious wakeups）。
            Err(_) => {
                self.lock().unlock();

                panic!("inconsistent park state")
            }
        }

        loop {
            self.cvar().wait(self.lock());

            match self.state.compare_exchange(NOTIFIED, EMPTY, Acquire, Relaxed) {
                Ok(_) => break, // 收到了一次通知
                Err(_) => {}    // 虚假唤醒（spurious wakeup），回去继续睡
            }
        }

        self.lock().unlock();
    }

    // 本实现并不要求 `unsafe`，但其他实现可能假定它只会被拥有该 Parker 的线程调用。
    // 使用 `Pin` 以保证 mutex 和条件变量（condition variable）拥有稳定的地址。
    pub unsafe fn park_timeout(self: Pin<&Self>, dur: Duration) {
        // 与上面的 `park` 一样，对于已被通知的线程我们有一条快速路径，
        // 之后我们开始协调进入睡眠。
        // 快速返回。
        if self.state.compare_exchange(NOTIFIED, EMPTY, Acquire, Relaxed).is_ok() {
            return;
        }

        self.lock().lock();
        match self.state.compare_exchange(EMPTY, PARKED, Relaxed, Relaxed) {
            Ok(_) => {}
            Err(NOTIFIED) => {
                // 这里我们必须再次读取，参见 `park`。
                let old = self.state.swap(EMPTY, Acquire);
                self.lock().unlock();

                assert_eq!(old, NOTIFIED, "park state changed unexpectedly");
                return;
            } // 应当消费这次通知，从而禁止下一次 park 中的虚假唤醒（spurious wakeups）。
            Err(_) => {
                self.lock().unlock();
                panic!("inconsistent park_timeout state")
            }
        }

        // 带超时地等待；如果我们发生虚假唤醒、或以其他方式从一次通知中醒来，
        // 我们都只想无条件地把 state 设回 empty，要么消费一次通知，
        // 要么把我们自己“处于 parked”的标志清除。
        self.cvar().wait_timeout(self.lock(), dur);

        match self.state.swap(EMPTY, Acquire) {
            NOTIFIED => self.lock().unlock(), // 收到了一次通知，万岁！
            PARKED => self.lock().unlock(),   // 没有通知，唉
            n => {
                self.lock().unlock();
                panic!("inconsistent park_timeout state: {n}")
            }
        }
    }

    pub fn unpark(self: Pin<&Self>) {
        // 为了确保被唤醒的线程能观测到我们在本次调用之前所做的任何写入，
        // 我们必须执行一次 `park` 可以与之同步的 release 操作。为此，即使 `state`
        // 已经是 `NOTIFIED`，我们也必须写入 `NOTIFIED`。这就是为什么这里必须是一次
        // swap，而不是一次「在读到 `NOTIFIED` 失败时直接返回」的 compare-and-swap。
        match self.state.swap(NOTIFIED, Release) {
            EMPTY => return,    // 没有人在等待
            NOTIFIED => return, // 已经被 unpark 过了
            PARKED => {}        // 得去唤醒某人
            _ => panic!("inconsistent state in unpark"),
        }

        // 在被 park 的线程把 `state` 设为 `PARKED`（或在虚假唤醒的情况下最后一次
        // 检查 `state`）的时刻，与它真正在 `cvar` 上等待的时刻之间，存在一个时间窗口。
        // 如果我们在这个窗口期间发出通知，它会被忽略，于是当被 park 的线程进入睡眠后
        // 就再也不会醒来。所幸，在此阶段它持有着 `lock` 的锁，所以我们可以去获取 `lock`，
        // 一直等到它准备好接收通知为止。
        //
        // 在调用 `notify_one` 之前释放 `lock`，意味着当被 park 的线程醒来时，
        // 不会出现「刚被唤醒却又得等我们释放 `lock`」的情况。
        unsafe {
            self.lock().lock();
            self.lock().unlock();
            self.cvar().notify_one();
        }
    }
}
