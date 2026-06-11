use crate::fmt;
use crate::sync::WaitTimeoutResult;
use crate::sync::poison::{self, LockResult, MutexGuard, PoisonError, mutex};
use crate::sys::sync as sys;
use crate::time::{Duration, Instant};

/// 一个条件变量（Condition Variable）
///
/// 条件变量代表这样一种能力：阻塞一个线程，使它在等待某事件发生时不消耗
/// CPU 时间。条件变量通常与一个布尔谓词（一个条件）和一个互斥锁关联在一起。
/// 在判定一个线程必须阻塞之前，总是会在互斥锁内部检验该谓词。
///
/// 本模块中的函数会阻塞当前 **线程**（thread）的执行。注意，任何在同一个
/// 条件变量上使用多个互斥锁的尝试，都可能导致运行时 panic。
///
/// # 示例
///
/// ```
/// use std::sync::{Arc, Mutex, Condvar};
/// use std::thread;
///
/// let pair = Arc::new((Mutex::new(false), Condvar::new()));
/// let pair2 = Arc::clone(&pair);
///
/// // 在持锁状态下派生一个新线程，然后等待它启动。
/// thread::spawn(move || {
///     let (lock, cvar) = &*pair2;
///     let mut started = lock.lock().unwrap();
///     *started = true;
///     // 我们通知该条件变量：值已经改变。
///     cvar.notify_one();
/// });
///
/// // 等待该线程启动。
/// let (lock, cvar) = &*pair;
/// let mut started = lock.lock().unwrap();
/// while !*started {
///     started = cvar.wait(started).unwrap();
/// }
/// ```
#[stable(feature = "rust1", since = "1.0.0")]
pub struct Condvar {
    inner: sys::Condvar,
}

impl Condvar {
    /// 创建一个新的条件变量，可随时被等待（wait）和通知（notify）。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::sync::Condvar;
    ///
    /// let condvar = Condvar::new();
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_locks", since = "1.63.0")]
    #[must_use]
    #[inline]
    pub const fn new() -> Condvar {
        Condvar { inner: sys::Condvar::new() }
    }

    /// 阻塞当前线程，直到这个条件变量收到一个通知（notification）。
    ///
    /// 本函数会原子地解锁所指定的互斥锁（由 `guard` 表示）并阻塞当前线程。
    /// 这意味着：任何在逻辑上发生于该互斥锁解锁之后的 [`notify_one`] 或
    /// [`notify_all`] 调用，都有可能唤醒本线程。当本函数调用返回时，所指定的
    /// 锁将已被重新获取。
    ///
    /// 注意，本函数易受虚假唤醒（spurious wakeups）的影响。条件变量通常会
    /// 关联一个布尔谓词（predicate），每次本函数返回时都必须检查该谓词，以
    /// 防范虚假唤醒。
    ///
    /// # Errors
    ///
    /// 如果当本线程重新获取该锁时，所等待的互斥锁已中毒（poisoned），本函数
    /// 将返回一个错误。更多信息参见 [`Mutex`] 类型上关于 [poisoning] 的说明。
    ///
    /// # Panics
    ///
    /// 如果在不同时间把它和多个互斥锁一起使用，本函数可能 [`panic!`]。
    ///
    /// [`notify_one`]: Self::notify_one
    /// [`notify_all`]: Self::notify_all
    /// [poisoning]: super::Mutex#poisoning
    /// [`Mutex`]: super::Mutex
    ///
    /// # 示例
    ///
    /// ```
    /// use std::sync::{Arc, Mutex, Condvar};
    /// use std::thread;
    ///
    /// let pair = Arc::new((Mutex::new(false), Condvar::new()));
    /// let pair2 = Arc::clone(&pair);
    ///
    /// thread::spawn(move || {
    ///     let (lock, cvar) = &*pair2;
    ///     let mut started = lock.lock().unwrap();
    ///     *started = true;
    ///     // 我们通知该条件变量：值已经改变。
    ///     cvar.notify_one();
    /// });
    ///
    /// // 等待该线程启动。
    /// let (lock, cvar) = &*pair;
    /// let mut started = lock.lock().unwrap();
    /// // 只要 `Mutex<bool>` 内部的值是 `false`，我们就一直等待。
    /// while !*started {
    ///     started = cvar.wait(started).unwrap();
    /// }
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_should_not_be_called_on_const_items]
    pub fn wait<'a, T>(&self, guard: MutexGuard<'a, T>) -> LockResult<MutexGuard<'a, T>> {
        let poisoned = unsafe {
            let lock = mutex::guard_lock(&guard);
            self.inner.wait(lock);
            // 重新获取锁之后，检查这把互斥锁此刻是否已中毒。
            mutex::guard_poison(&guard).get()
        };
        // 若已中毒，则把守卫包进 `PoisonError` 一并返回，把中毒状态传播给调用方。
        if poisoned { Err(PoisonError::new(guard)) } else { Ok(guard) }
    }

    /// 阻塞当前线程，直到所提供的条件变为 false。
    ///
    /// `condition` 会被立即检查；如果未满足（返回 `true`），则本方法会
    /// [`wait`] 下一个通知，然后再次检查。如此重复，直到 `condition` 返回
    /// `false`，此时本函数返回。
    ///
    /// 本函数会原子地解锁所指定的互斥锁（由 `guard` 表示）并阻塞当前线程。
    /// 这意味着：任何在逻辑上发生于该互斥锁解锁之后的 [`notify_one`] 或
    /// [`notify_all`] 调用，都有可能唤醒本线程。当本函数调用返回时，所指定的
    /// 锁将已被重新获取。
    ///
    /// # Errors
    ///
    /// 如果当本线程重新获取该锁时，所等待的互斥锁已中毒，本函数将返回一个
    /// 错误。更多信息参见 [`Mutex`] 类型上关于 [poisoning] 的说明。
    ///
    /// [`wait`]: Self::wait
    /// [`notify_one`]: Self::notify_one
    /// [`notify_all`]: Self::notify_all
    /// [poisoning]: super::Mutex#poisoning
    /// [`Mutex`]: super::Mutex
    ///
    /// # 示例
    ///
    /// ```
    /// use std::sync::{Arc, Mutex, Condvar};
    /// use std::thread;
    ///
    /// let pair = Arc::new((Mutex::new(true), Condvar::new()));
    /// let pair2 = Arc::clone(&pair);
    ///
    /// thread::spawn(move || {
    ///     let (lock, cvar) = &*pair2;
    ///     let mut pending = lock.lock().unwrap();
    ///     *pending = false;
    ///     // 我们通知该条件变量：值已经改变。
    ///     cvar.notify_one();
    /// });
    ///
    /// // 等待该线程启动。
    /// let (lock, cvar) = &*pair;
    /// // 只要 `Mutex<bool>` 内部的值是 `true`，我们就一直等待。
    /// let _guard = cvar.wait_while(lock.lock().unwrap(), |pending| { *pending }).unwrap();
    /// ```
    #[stable(feature = "wait_until", since = "1.42.0")]
    #[rustc_should_not_be_called_on_const_items]
    pub fn wait_while<'a, T, F>(
        &self,
        mut guard: MutexGuard<'a, T>,
        mut condition: F,
    ) -> LockResult<MutexGuard<'a, T>>
    where
        F: FnMut(&mut T) -> bool,
    {
        // 在循环中检查谓词，自动应对虚假唤醒：只要条件仍为 true 就继续等待。
        while condition(&mut *guard) {
            guard = self.wait(guard)?;
        }
        Ok(guard)
    }

    /// 在这个条件变量上等待一个通知，并在指定的时长（duration）后超时。
    ///
    /// 本函数的语义等价于 [`wait`]，区别在于本线程被阻塞的时间大致不会超过
    /// `ms` 毫秒。本方法不应用于精确计时，因为诸如抢占（preemption）或平台
    /// 差异之类的异常情况，可能导致实际等待的最大时间并非恰好为 `ms`。
    ///
    /// 注意，已尽力确保所等待的时间用单调时钟（monotonic clock）来度量，
    /// 不受系统时间变更的影响。
    ///
    /// 返回的布尔值仅在可确定超时已经发生时才为 `false`。
    ///
    /// 与 [`wait`] 一样，无论超时是否发生，本函数返回时所指定的锁都将被重新
    /// 获取。
    ///
    /// [`wait`]: Self::wait
    ///
    /// # 示例
    ///
    /// ```
    /// use std::sync::{Arc, Mutex, Condvar};
    /// use std::thread;
    ///
    /// let pair = Arc::new((Mutex::new(false), Condvar::new()));
    /// let pair2 = Arc::clone(&pair);
    ///
    /// thread::spawn(move || {
    ///     let (lock, cvar) = &*pair2;
    ///     let mut started = lock.lock().unwrap();
    ///     *started = true;
    ///     // 我们通知该条件变量：值已经改变。
    ///     cvar.notify_one();
    /// });
    ///
    /// // 等待该线程启动。
    /// let (lock, cvar) = &*pair;
    /// let mut started = lock.lock().unwrap();
    /// // 只要 `Mutex<bool>` 内部的值是 `false`，我们就一直等待。
    /// loop {
    ///     let result = cvar.wait_timeout_ms(started, 10).unwrap();
    ///     // 10 毫秒已经过去，或者也许值已改变！
    ///     started = result.0;
    ///     if *started == true {
    ///         // 我们收到了通知且值已被更新，可以离开了。
    ///         break
    ///     }
    /// }
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_should_not_be_called_on_const_items]
    #[deprecated(since = "1.6.0", note = "replaced by `std::sync::Condvar::wait_timeout`")]
    pub fn wait_timeout_ms<'a, T>(
        &self,
        guard: MutexGuard<'a, T>,
        ms: u32,
    ) -> LockResult<(MutexGuard<'a, T>, bool)> {
        let res = self.wait_timeout(guard, Duration::from_millis(ms as u64));
        poison::map_result(res, |(a, b)| (a, !b.timed_out()))
    }

    /// 在这个条件变量上等待一个通知，并在指定的时长（duration）后超时。
    ///
    /// 本函数的语义等价于 [`wait`]，区别在于本线程被阻塞的时间大致不会超过
    /// `dur`。本方法不应用于精确计时，因为诸如抢占（preemption）或平台差异
    /// 之类的异常情况，可能导致实际等待的最大时间并非恰好为 `dur`。
    ///
    /// 注意，已尽力确保所等待的时间用单调时钟（monotonic clock）来度量，
    /// 不受系统时间变更的影响。本函数易受虚假唤醒的影响。条件变量通常会关联
    /// 一个布尔谓词，每次本函数返回时都必须检查该谓词，以防范虚假唤醒。此外，
    /// 由于超时是相对于本函数被调用的那一刻给出的，因此当本函数在循环中调用
    /// 时需要对其进行调整。[`wait_timeout_while`] 方法让你能够在谓词为 true
    /// 期间带超时地等待，并替你处理好上述所有这些顾虑。
    ///
    /// 返回的 [`WaitTimeoutResult`] 值表明是否可确定超时已经发生。
    ///
    /// 与 [`wait`] 一样，无论超时是否发生，本函数返回时所指定的锁都将被重新
    /// 获取。
    ///
    /// [`wait`]: Self::wait
    /// [`wait_timeout_while`]: Self::wait_timeout_while
    ///
    /// # 示例
    ///
    /// ```
    /// use std::sync::{Arc, Mutex, Condvar};
    /// use std::thread;
    /// use std::time::Duration;
    ///
    /// let pair = Arc::new((Mutex::new(false), Condvar::new()));
    /// let pair2 = Arc::clone(&pair);
    ///
    /// thread::spawn(move || {
    ///     let (lock, cvar) = &*pair2;
    ///     let mut started = lock.lock().unwrap();
    ///     *started = true;
    ///     // 我们通知该条件变量：值已经改变。
    ///     cvar.notify_one();
    /// });
    ///
    /// // 等待该线程启动
    /// let (lock, cvar) = &*pair;
    /// let mut started = lock.lock().unwrap();
    /// // 只要 `Mutex<bool>` 内部的值是 `false`，我们就一直等待
    /// loop {
    ///     let result = cvar.wait_timeout(started, Duration::from_millis(10)).unwrap();
    ///     // 10 毫秒已经过去，或者也许值已改变！
    ///     started = result.0;
    ///     if *started == true {
    ///         // 我们收到了通知且值已被更新，可以离开了。
    ///         break
    ///     }
    /// }
    /// ```
    #[stable(feature = "wait_timeout", since = "1.5.0")]
    #[rustc_should_not_be_called_on_const_items]
    pub fn wait_timeout<'a, T>(
        &self,
        guard: MutexGuard<'a, T>,
        dur: Duration,
    ) -> LockResult<(MutexGuard<'a, T>, WaitTimeoutResult)> {
        let (poisoned, result) = unsafe {
            let lock = mutex::guard_lock(&guard);
            let success = self.inner.wait_timeout(lock, dur);
            // 底层 `wait_timeout` 返回「是否被通知唤醒（成功）」；取反即「是否超时」。
            (mutex::guard_poison(&guard).get(), WaitTimeoutResult(!success))
        };
        if poisoned { Err(PoisonError::new((guard, result))) } else { Ok((guard, result)) }
    }

    /// 在这个条件变量上等待一个通知，并在指定的时长后超时。
    ///
    /// 本函数的语义等价于 [`wait_while`]，区别在于本线程被阻塞的时间大致不会
    /// 超过 `dur`。本方法不应用于精确计时，因为诸如抢占或平台差异之类的异常
    /// 情况，可能导致实际等待的最大时间并非恰好为 `dur`。
    ///
    /// 注意，已尽力确保所等待的时间用单调时钟来度量，不受系统时间变更的影响。
    ///
    /// 返回的 [`WaitTimeoutResult`] 值表明是否可确定：在条件未被满足的情况下
    /// 超时已经发生。
    ///
    /// 与 [`wait_while`] 一样，无论超时是否发生，本函数返回时所指定的锁都将被
    /// 重新获取。
    ///
    /// [`wait_while`]: Self::wait_while
    /// [`wait_timeout`]: Self::wait_timeout
    ///
    /// # 示例
    ///
    /// ```
    /// use std::sync::{Arc, Mutex, Condvar};
    /// use std::thread;
    /// use std::time::Duration;
    ///
    /// let pair = Arc::new((Mutex::new(true), Condvar::new()));
    /// let pair2 = Arc::clone(&pair);
    ///
    /// thread::spawn(move || {
    ///     let (lock, cvar) = &*pair2;
    ///     let mut pending = lock.lock().unwrap();
    ///     *pending = false;
    ///     // 我们通知该条件变量：值已经改变。
    ///     cvar.notify_one();
    /// });
    ///
    /// // 等待该线程启动
    /// let (lock, cvar) = &*pair;
    /// let result = cvar.wait_timeout_while(
    ///     lock.lock().unwrap(),
    ///     Duration::from_millis(100),
    ///     |&mut pending| pending,
    /// ).unwrap();
    /// if result.1.timed_out() {
    ///     // 超时了，且条件自始至终都没有取值为 false。
    /// }
    /// // 通过 result.0 访问已锁定的互斥锁
    /// ```
    #[stable(feature = "wait_timeout_until", since = "1.42.0")]
    #[rustc_should_not_be_called_on_const_items]
    pub fn wait_timeout_while<'a, T, F>(
        &self,
        mut guard: MutexGuard<'a, T>,
        dur: Duration,
        mut condition: F,
    ) -> LockResult<(MutexGuard<'a, T>, WaitTimeoutResult)>
    where
        F: FnMut(&mut T) -> bool,
    {
        let start = Instant::now();
        loop {
            if !condition(&mut *guard) {
                return Ok((guard, WaitTimeoutResult(false)));
            }
            // 每次循环都重新计算剩余超时：用总时长减去已流逝时间；若已耗尽
            // （checked_sub 返回 None），则判定为超时并返回。
            let timeout = match dur.checked_sub(start.elapsed()) {
                Some(timeout) => timeout,
                None => return Ok((guard, WaitTimeoutResult(true))),
            };
            guard = self.wait_timeout(guard, timeout)?.0;
        }
    }

    /// 唤醒这个条件变量上一个被阻塞的线程。
    ///
    /// 如果这个条件变量上有一个被阻塞的线程，那么它将从其对 [`wait`] 或
    /// [`wait_timeout`] 的调用中被唤醒。对 `notify_one` 的调用不会以任何方式
    /// 被缓冲（buffer）。
    ///
    /// 要唤醒所有线程，参见 [`notify_all`]。
    ///
    /// [`wait`]: Self::wait
    /// [`wait_timeout`]: Self::wait_timeout
    /// [`notify_all`]: Self::notify_all
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::{Arc, Mutex, Condvar};
    /// use std::thread;
    ///
    /// let pair = Arc::new((Mutex::new(false), Condvar::new()));
    /// let pair2 = Arc::clone(&pair);
    ///
    /// thread::spawn(move || {
    ///     let (lock, cvar) = &*pair2;
    ///     let mut started = lock.lock().unwrap();
    ///     *started = true;
    ///     // 我们通知该条件变量：值已经改变。
    ///     cvar.notify_one();
    /// });
    ///
    /// // 等待该线程启动。
    /// let (lock, cvar) = &*pair;
    /// let mut started = lock.lock().unwrap();
    /// // 只要 `Mutex<bool>` 内部的值是 `false`，我们就一直等待。
    /// while !*started {
    ///     started = cvar.wait(started).unwrap();
    /// }
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_should_not_be_called_on_const_items]
    pub fn notify_one(&self) {
        self.inner.notify_one()
    }

    /// 唤醒这个条件变量上所有被阻塞的线程。
    ///
    /// 本方法会确保该条件变量上当前所有的等待者都被唤醒。对 `notify_all()` 的
    /// 调用不会以任何方式被缓冲。
    ///
    /// 要只唤醒一个线程，参见 [`notify_one`]。
    ///
    /// [`notify_one`]: Self::notify_one
    ///
    /// # 示例
    ///
    /// ```
    /// use std::sync::{Arc, Mutex, Condvar};
    /// use std::thread;
    ///
    /// let pair = Arc::new((Mutex::new(false), Condvar::new()));
    /// let pair2 = Arc::clone(&pair);
    ///
    /// thread::spawn(move || {
    ///     let (lock, cvar) = &*pair2;
    ///     let mut started = lock.lock().unwrap();
    ///     *started = true;
    ///     // 我们通知该条件变量：值已经改变。
    ///     cvar.notify_all();
    /// });
    ///
    /// // Wait for the thread to start up.
    /// let (lock, cvar) = &*pair;
    /// let mut started = lock.lock().unwrap();
    /// // 只要 `Mutex<bool>` 内部的值是 `false`，我们就一直等待。
    /// while !*started {
    ///     started = cvar.wait(started).unwrap();
    /// }
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_should_not_be_called_on_const_items]
    pub fn notify_all(&self) {
        self.inner.notify_all()
    }
}

#[stable(feature = "std_debug", since = "1.16.0")]
impl fmt::Debug for Condvar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Condvar").finish_non_exhaustive()
    }
}

#[stable(feature = "condvar_default", since = "1.10.0")]
impl Default for Condvar {
    /// 创建一个可随时被等待和通知的 `Condvar`。
    fn default() -> Condvar {
        Condvar::new()
    }
}
