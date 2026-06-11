use crate::num::NonZero;
use crate::sync::atomic::{Atomic, Ordering};

/// 正在运行的线程的唯一标识符。
///
/// `ThreadId` 是一个不透明对象，它唯一地标识了进程生命周期内创建的每个线程。
/// 保证 `ThreadId` 不会被重用，即便线程已经终止。`ThreadId` 由 Rust 标准库
/// 控制，它与底层平台的线程标识符概念之间可能没有任何关系——因此这两个概念
/// 不能互换使用。`ThreadId` 可以通过 [`Thread`] 上的 [`id`] 方法获取。
///
/// # Examples
///
/// ```
/// use std::thread;
///
/// let other_thread = thread::spawn(|| {
///     thread::current().id()
/// });
///
/// let other_thread_id = other_thread.join().unwrap();
/// assert!(thread::current().id() != other_thread_id);
/// ```
///
/// [`Thread`]: super::Thread
/// [`id`]: super::Thread::id
#[stable(feature = "thread_id", since = "1.19.0")]
#[derive(Eq, PartialEq, Clone, Copy, Hash, Debug)]
pub struct ThreadId(NonZero<u64>);

impl ThreadId {
    // 生成一个新的唯一线程 ID。
    pub(crate) fn new() -> ThreadId {
        #[cold]
        fn exhausted() -> ! {
            panic!("failed to generate unique thread ID: bitspace exhausted")
        }

        cfg_select! {
            target_has_atomic = "64" => {
                use crate::sync::atomic::AtomicU64;

                static COUNTER: Atomic<u64> = AtomicU64::new(0);

                let mut last = COUNTER.load(Ordering::Relaxed);
                loop {
                    let Some(id) = last.checked_add(1) else {
                        exhausted();
                    };

                    match COUNTER.compare_exchange_weak(last, id, Ordering::Relaxed, Ordering::Relaxed) {
                        Ok(_) => return ThreadId(NonZero::new(id).unwrap()),
                        Err(id) => last = id,
                    }
                }
            }
            _ => {
                use crate::cell::SyncUnsafeCell;
                use crate::hint::spin_loop;
                use crate::sync::atomic::AtomicBool;
                use crate::thread::yield_now;

                // 如果没有 64 位原子类型，我们就使用一个小型自旋锁。这里不用 Mutex，
                // 因为我们可能正试图在全局分配器中获取当前线程 ID，而在某些平台上
                // Mutex 需要进行分配。
                static COUNTER_LOCKED: Atomic<bool> = AtomicBool::new(false);
                static COUNTER: SyncUnsafeCell<u64> = SyncUnsafeCell::new(0);

                // 获取锁。
                let mut spin = 0;
                // Miri 不喜欢我们在这里 yield，因为它会干扰对线程的确定性调度，
                // 所以避免使用 `compare_exchange_weak` 以免产生虚假的 yield。
                while COUNTER_LOCKED.swap(true, Ordering::Acquire) {
                    if spin <= 3 {
                        for _ in 0..(1 << spin) {
                            spin_loop();
                        }
                    } else {
                        yield_now();
                    }
                    spin += 1;
                }
                // 在这次 swap 之前它是 `false`，所以我们拿到了锁。

                // SAFETY: 我们对该计数器持有独占锁。
                unsafe {
                    if let Some(id) = (*COUNTER.get()).checked_add(1) {
                        *COUNTER.get() = id;
                        COUNTER_LOCKED.store(false, Ordering::Release);
                        ThreadId(NonZero::new(id).unwrap())
                    } else {
                        COUNTER_LOCKED.store(false, Ordering::Release);
                        exhausted()
                    }
                }
            }
        }
    }

    #[cfg(any(not(target_thread_local), target_has_atomic = "64"))]
    pub(super) fn from_u64(v: u64) -> Option<ThreadId> {
        NonZero::new(v).map(ThreadId)
    }

    /// 它返回由这个 `ThreadId` 所标识的线程的一个数值标识符。
    ///
    /// 正如该类型自身的文档所述，它本质上是一个不透明 ID，但保证对每个线程都是
    /// 唯一的。返回值是完全不透明的——只有相等性测试是稳定的。注意，并不保证新
    /// 线程会返回哪些值，并且这一点可能随 Rust 版本而变化。
    #[must_use]
    #[unstable(feature = "thread_id_value", issue = "67939")]
    pub fn as_u64(&self) -> NonZero<u64> {
        self.0
    }
}
