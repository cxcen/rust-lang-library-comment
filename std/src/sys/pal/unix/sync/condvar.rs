use super::Mutex;
use crate::cell::UnsafeCell;
use crate::pin::Pin;
#[cfg(not(target_os = "nto"))]
use crate::sys::pal::time::TIMESPEC_MAX;
#[cfg(target_os = "nto")]
use crate::sys::pal::time::TIMESPEC_MAX_CAPPED;
use crate::time::Duration;

pub struct Condvar {
    inner: UnsafeCell<libc::pthread_cond_t>,
}

impl Condvar {
    pub fn new() -> Condvar {
        Condvar { inner: UnsafeCell::new(libc::PTHREAD_COND_INITIALIZER) }
    }

    #[inline]
    fn raw(&self) -> *mut libc::pthread_cond_t {
        self.inner.get()
    }

    /// # 安全性(Safety）
    /// 必须已对该实例调用过 `init`。
    #[inline]
    pub unsafe fn notify_one(self: Pin<&Self>) {
        let r = unsafe { libc::pthread_cond_signal(self.raw()) };
        debug_assert_eq!(r, 0);
    }

    /// # 安全性(Safety）
    /// 必须已对该实例调用过 `init`。
    #[inline]
    pub unsafe fn notify_all(self: Pin<&Self>) {
        let r = unsafe { libc::pthread_cond_broadcast(self.raw()) };
        debug_assert_eq!(r, 0);
    }

    /// # 安全性(Safety）
    /// * 必须已对该实例调用过 `init`。
    /// * `mutex` 必须已被当前线程锁定。
    /// * 该条件变量只能与同一个 mutex 搭配使用。
    #[inline]
    pub unsafe fn wait(self: Pin<&Self>, mutex: Pin<&Mutex>) {
        let r = unsafe { libc::pthread_cond_wait(self.raw(), mutex.raw()) };
        debug_assert_eq!(r, 0);
    }
}

#[cfg(not(target_vendor = "apple"))]
impl Condvar {
    /// # 安全性(Safety）
    /// * 必须已对该实例调用过 `init`。
    /// * `mutex` 必须已被当前线程锁定。
    /// * 该条件变量只能与同一个 mutex 搭配使用。
    pub unsafe fn wait_timeout(&self, mutex: Pin<&Mutex>, dur: Duration) -> bool {
        use crate::sys::pal::time::Timespec;

        let mutex = mutex.raw();

        // Cygwin 的实现基于 NT API，后者以 100 ns 为单位来计量时间。遗憾的是，
        // Cygwin 在转换时间时没有妥善防止溢出，因此我们把这个时间间隔钳制
        // （clamp）到 1000 年；这只会在大约 27000 年后、当距离下一次回绕
        // （rollover）不足 1000 年时才会成为问题……
        #[cfg(target_os = "cygwin")]
        let dur = Duration::min(dur, Duration::from_secs(1000 * 365 * 86400));

        let timeout = Timespec::now(Self::CLOCK).checked_add_duration(&dur);

        #[cfg(not(target_os = "nto"))]
        let timeout = timeout.and_then(|t| t.to_timespec()).unwrap_or(TIMESPEC_MAX);

        #[cfg(target_os = "nto")]
        let timeout = timeout.and_then(|t| t.to_timespec_capped()).unwrap_or(TIMESPEC_MAX_CAPPED);

        let r = unsafe { libc::pthread_cond_timedwait(self.raw(), mutex, &timeout) };
        assert!(r == libc::ETIMEDOUT || r == 0);
        r == 0
    }
}

// Apple 平台（自 macOS 10.4 与 iOS 2.0 起）提供了
// `pthread_cond_timedwait_relative_np`——一个非标准扩展，它基于单调时钟
// （monotonic clock）来计量超时，因而不受墙上时钟（wall-clock）变动的影响。
#[cfg(target_vendor = "apple")]
impl Condvar {
    /// # 安全性(Safety）
    /// * 必须已对该实例调用过 `init`。
    /// * `mutex` 必须已被当前线程锁定。
    /// * 该条件变量只能与同一个 mutex 搭配使用。
    pub unsafe fn wait_timeout(&self, mutex: Pin<&Mutex>, dur: Duration) -> bool {
        let mutex = mutex.raw();

        // macOS 对 `pthread_cond_timedwait` 的实现，内部会把传给
        // `pthread_cond_timedwait_relative_np` 的超时转换为纳秒。遗憾的是，
        // condvar 的 "psynch" 变体在该转换过程中并不防止溢出[^1]，这意味着如果
        // 相对超时长于 `u64::MAX` 纳秒，`pthread_cond_timedwait_relative_np`
        // 会比预期早得多地返回 `ETIMEDOUT`。
        //
        // 即便在较新的平台上也能观察到这一点（方法是把环境变量
        // PTHREAD_MUTEX_USE_ULOCK 设为 "1" 以外的值），例如调用
        // ```
        // condvar.wait_timeout(..., Duration::from_secs(u64::MAX.div_ceil(1_000_000_000));
        // ```
        // （参见 #37440，尤其是
        // https://github.com/rust-lang/rust/issues/37440#issuecomment-3285958326）。
        //
        // 为绕开此问题，我们始终把超时钳制到 u64::MAX 纳秒，即便用的是
        // "ulock" 变体（它确实会防止溢出）也一样。
        //
        // [^1]: https://github.com/apple-oss-distributions/libpthread/blob/1ebf56b3a702df53213c2996e5e128a535d2577e/kern/kern_synch.c#L1269
        const MAX_DURATION: Duration = Duration::from_nanos(u64::MAX);

        let (dur, clamped) = if dur <= MAX_DURATION { (dur, false) } else { (MAX_DURATION, true) };

        // 在 32 位平台上这可能溢出，但由于上面的钳制，在 64 位上不会。
        let timeout = if let Ok(tv_sec) = dur.as_secs().try_into() {
            libc::timespec { tv_sec, tv_nsec: dur.subsec_nanos() as _ }
        } else {
            // 在 32 位平台上这小于 `MAX_DURATION`。
            TIMESPEC_MAX
        };

        let r = unsafe { libc::pthread_cond_timedwait_relative_np(self.raw(), mutex, &timeout) };
        assert!(r == libc::ETIMEDOUT || r == 0);
        // 把（发生了）钳制的情况当作一次伪唤醒（spurious wakeup）来上报。谁知道
        // 呢，也许某个星际空间探测器会指望这个行为 ;-)。
        r == 0 || clamped
    }
}

#[cfg(not(any(
    target_os = "android",
    target_vendor = "apple",
    target_os = "espidf",
    target_os = "horizon",
    target_os = "l4re",
    target_os = "redox",
    target_os = "teeos",
)))]
impl Condvar {
    pub const PRECISE_TIMEOUT: bool = true;
    const CLOCK: libc::clockid_t = libc::CLOCK_MONOTONIC;

    /// # 安全性(Safety）
    /// 对于每个 `Self` 实例只能调用一次。
    pub unsafe fn init(self: Pin<&mut Self>) {
        use crate::mem::MaybeUninit;

        struct AttrGuard<'a>(pub &'a mut MaybeUninit<libc::pthread_condattr_t>);
        impl Drop for AttrGuard<'_> {
            fn drop(&mut self) {
                unsafe {
                    let result = libc::pthread_condattr_destroy(self.0.as_mut_ptr());
                    assert_eq!(result, 0);
                }
            }
        }

        unsafe {
            let mut attr = MaybeUninit::<libc::pthread_condattr_t>::uninit();
            let r = libc::pthread_condattr_init(attr.as_mut_ptr());
            assert_eq!(r, 0);
            let attr = AttrGuard(&mut attr);
            let r = libc::pthread_condattr_setclock(attr.0.as_mut_ptr(), Self::CLOCK);
            assert_eq!(r, 0);
            let r = libc::pthread_cond_init(self.raw(), attr.0.as_ptr());
            assert_eq!(r, 0);
        }
    }
}

#[cfg(target_vendor = "apple")]
impl Condvar {
    // `pthread_cond_timedwait_relative_np` 基于单调时钟来计量超时。
    pub const PRECISE_TIMEOUT: bool = true;

    /// # 安全性(Safety）
    /// 对于每个 `Self` 实例只能调用一次。
    pub unsafe fn init(self: Pin<&mut Self>) {
        // `PTHREAD_COND_INITIALIZER` 已被完整支持，而且我们不需要更换时钟，所以
        // 这里无事可做。
    }
}

// 遗憾的是 `pthread_condattr_setclock` 在以下这些平台上并不受支持。
#[cfg(any(
    target_os = "android",
    target_os = "espidf",
    target_os = "horizon",
    target_os = "l4re",
    target_os = "redox",
    target_os = "teeos",
))]
impl Condvar {
    pub const PRECISE_TIMEOUT: bool = false;
    const CLOCK: libc::clockid_t = libc::CLOCK_REALTIME;

    /// # 安全性(Safety）
    /// 对于每个 `Self` 实例只能调用一次。
    pub unsafe fn init(self: Pin<&mut Self>) {
        if cfg!(any(target_os = "espidf", target_os = "horizon", target_os = "teeos")) {
            // NOTE: ESP-IDF 对 PTHREAD_COND_INITIALIZER 的支持尚未发布，
            // 所以在该平台上，init() 应当总是被调用。
            //
            // 3DS（horizon）以及 TEEOS 的情况也类似。
            let r = unsafe { libc::pthread_cond_init(self.raw(), crate::ptr::null()) };
            assert_eq!(r, 0);
        }
    }
}

impl !Unpin for Condvar {}

unsafe impl Sync for Condvar {}
unsafe impl Send for Condvar {}

impl Drop for Condvar {
    #[inline]
    fn drop(&mut self) {
        let r = unsafe { libc::pthread_cond_destroy(self.raw()) };
        if cfg!(target_os = "dragonfly") {
            // 在 DragonFly 上，如果对一个刚用 libc::PTHREAD_COND_INITIALIZER
            // 初始化过的 condvar 调用 pthread_cond_destroy()，它会返回 EINVAL。
            // 一旦该 condvar 被使用过、或调用过 pthread_cond_init()，这种行为便
            // 不再出现。
            debug_assert!(r == 0 || r == libc::EINVAL);
        } else {
            debug_assert_eq!(r, 0);
        }
    }
}
