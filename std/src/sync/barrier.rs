use crate::fmt;
use crate::panic::RefUnwindSafe;
use crate::sync::nonpoison::{Condvar, Mutex};

/// 屏障（barrier）使多个线程能够同步某项计算的开始时刻。
///
/// # 示例
///
/// ```
/// use std::sync::Barrier;
/// use std::thread;
///
/// let n = 10;
/// let barrier = Barrier::new(n);
/// thread::scope(|s| {
///     for _ in 0..n {
///         // 相同的消息会成组打印。
///         // 你不会看到任何交错（interleaving）。
///         s.spawn(|| {
///             println!("before wait");
///             barrier.wait();
///             println!("after wait");
///         });
///     }
/// });
/// ```
#[stable(feature = "rust1", since = "1.0.0")]
pub struct Barrier {
    lock: Mutex<BarrierState>,
    cvar: Condvar,
    num_threads: usize,
}

#[stable(feature = "unwind_safe_lock_refs", since = "1.12.0")]
impl RefUnwindSafe for Barrier {}

// 双重屏障（double barrier）的内部状态
struct BarrierState {
    count: usize,
    // 代数（generation）编号：屏障可重复使用，每凑齐一批线程就递增一次，
    // 用以区分不同批次，避免等待中的线程被错误地视为已属于下一代。
    generation_id: usize,
}

/// 当 [`Barrier`] 中的所有线程都汇合（rendezvous）之后，[`Barrier::wait()`]
/// 会返回一个 `BarrierWaitResult`。
///
/// # 示例
///
/// ```
/// use std::sync::Barrier;
///
/// let barrier = Barrier::new(1);
/// let barrier_wait_result = barrier.wait();
/// ```
#[stable(feature = "rust1", since = "1.0.0")]
pub struct BarrierWaitResult(bool);

#[stable(feature = "std_debug", since = "1.16.0")]
impl fmt::Debug for Barrier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Barrier").finish_non_exhaustive()
    }
}

impl Barrier {
    /// 创建一个能阻塞给定数量线程的新屏障。
    ///
    /// 屏障会阻塞所有调用 [`wait()`] 的线程，直到第 `n` 个线程调用 [`wait()`]，
    /// 然后一次性唤醒所有线程。
    ///
    /// [`wait()`]: Barrier::wait
    ///
    /// # 示例
    ///
    /// ```
    /// use std::sync::Barrier;
    ///
    /// let barrier = Barrier::new(10);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_barrier", since = "1.78.0")]
    #[must_use]
    #[inline]
    pub const fn new(n: usize) -> Barrier {
        Barrier {
            lock: Mutex::new(BarrierState { count: 0, generation_id: 0 }),
            cvar: Condvar::new(),
            num_threads: n,
        }
    }

    /// 阻塞当前线程，直到所有线程都在此处汇合（rendezvous）。
    ///
    /// 当所有线程汇合过一次之后，屏障可被重复使用，因而能持续地使用下去。
    ///
    /// 从本函数返回时，将有单个（任意挑选的）线程收到一个
    /// [`BarrierWaitResult`]，其 [`BarrierWaitResult::is_leader()`] 返回 `true`；
    /// 而所有其他线程收到的结果，其 [`BarrierWaitResult::is_leader()`] 返回 `false`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::sync::Barrier;
    /// use std::thread;
    ///
    /// let n = 10;
    /// let barrier = Barrier::new(n);
    /// thread::scope(|s| {
    ///     for _ in 0..n {
    ///         // 相同的消息会成组打印。
    ///         // 你不会看到任何交错（interleaving）。
    ///         s.spawn(|| {
    ///             println!("before wait");
    ///             barrier.wait();
    ///             println!("after wait");
    ///         });
    ///     }
    /// });
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn wait(&self) -> BarrierWaitResult {
        let mut lock = self.lock.lock();
        // 记录进入时所处的代数；之后用它来判断本代是否已经翻篇。
        let local_gen = lock.generation_id;
        lock.count += 1;
        if lock.count < self.num_threads {
            // 还没凑齐：在条件变量上等待，直到代数发生变化（即本代被领头线程
            // 翻篇）。`wait_while` 在循环中检查谓词，自动应对虚假唤醒
            // （spurious wakeup）。
            self.cvar.wait_while(&mut lock, |state| local_gen == state.generation_id);
            BarrierWaitResult(false)
        } else {
            // 本线程是凑齐这一批的最后一个，于是它成为「领头线程」（leader）：
            // 重置计数、推进代数（使用 `wrapping_add` 以容忍回绕），并唤醒
            // 所有等待者。
            lock.count = 0;
            lock.generation_id = lock.generation_id.wrapping_add(1);
            self.cvar.notify_all();
            BarrierWaitResult(true)
        }
    }
}

#[stable(feature = "std_debug", since = "1.16.0")]
impl fmt::Debug for BarrierWaitResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BarrierWaitResult").field("is_leader", &self.is_leader()).finish()
    }
}

impl BarrierWaitResult {
    /// 如果本线程是该次 [`Barrier::wait()`] 调用的「领头线程」（leader thread），
    /// 则返回 `true`。
    ///
    /// 只有一个线程的结果会返回 `true`，所有其他线程返回 `false`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::sync::Barrier;
    ///
    /// let barrier = Barrier::new(1);
    /// let barrier_wait_result = barrier.wait();
    /// println!("{:?}", barrier_wait_result.is_leader());
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[must_use]
    pub fn is_leader(&self) -> bool {
        self.0
    }
}
