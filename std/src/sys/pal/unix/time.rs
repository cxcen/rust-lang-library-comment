use core::num::niche_types::Nanoseconds;

use crate::sys::AsInner;
use crate::time::Duration;
use crate::{fmt, io};

const NSEC_PER_SEC: u64 = 1_000_000_000;
pub const UNIX_EPOCH: SystemTime = SystemTime { t: Timespec::zero() };
#[allow(dead_code)] // 用于 pthread condvar 超时
pub const TIMESPEC_MAX: libc::timespec =
    libc::timespec { tv_sec: <libc::time_t>::MAX, tv_nsec: 1_000_000_000 - 1 };

// 这个额外的常量仅在调用
// `libc::pthread_cond_timedwait` 时使用。
#[cfg(target_os = "nto")]
pub(in crate::sys) const TIMESPEC_MAX_CAPPED: libc::timespec = libc::timespec {
    tv_sec: (u64::MAX / NSEC_PER_SEC) as i64,
    tv_nsec: (u64::MAX % NSEC_PER_SEC) as i64,
};

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SystemTime {
    pub(crate) t: Timespec,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct Timespec {
    tv_sec: i64,
    tv_nsec: Nanoseconds,
}

impl SystemTime {
    pub const MAX: SystemTime = SystemTime { t: Timespec::MAX };

    pub const MIN: SystemTime = SystemTime { t: Timespec::MIN };

    #[cfg_attr(any(target_os = "horizon", target_os = "hurd"), allow(unused))]
    pub fn new(tv_sec: i64, tv_nsec: i64) -> Result<SystemTime, io::Error> {
        Ok(SystemTime { t: Timespec::new(tv_sec, tv_nsec)? })
    }

    pub fn now() -> SystemTime {
        SystemTime { t: Timespec::now(libc::CLOCK_REALTIME) }
    }

    pub fn sub_time(&self, other: &SystemTime) -> Result<Duration, Duration> {
        self.t.sub_timespec(&other.t)
    }

    pub fn checked_add_duration(&self, other: &Duration) -> Option<SystemTime> {
        Some(SystemTime { t: self.t.checked_add_duration(other)? })
    }

    pub fn checked_sub_duration(&self, other: &Duration) -> Option<SystemTime> {
        Some(SystemTime { t: self.t.checked_sub_duration(other)? })
    }
}

impl fmt::Debug for SystemTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SystemTime")
            .field("tv_sec", &self.t.tv_sec)
            .field("tv_nsec", &self.t.tv_nsec)
            .finish()
    }
}

impl Timespec {
    const MAX: Timespec = unsafe { Self::new_unchecked(i64::MAX, 1_000_000_000 - 1) };

    // 如下文所述，在 Apple OS 上，纪元（epoch）之前的日期表示方式不同。
    // 不过这在这里并不构成问题，因为我们使用的是 tv_sec = i64::MIN，
    // 它会导致那个兼容性包装逻辑根本不会被执行。
    const MIN: Timespec = unsafe { Self::new_unchecked(i64::MIN, 0) };

    const unsafe fn new_unchecked(tv_sec: i64, tv_nsec: i64) -> Timespec {
        Timespec { tv_sec, tv_nsec: unsafe { Nanoseconds::new_unchecked(tv_nsec as u32) } }
    }

    pub const fn zero() -> Timespec {
        unsafe { Self::new_unchecked(0, 0) }
    }

    const fn new(tv_sec: i64, tv_nsec: i64) -> Result<Timespec, io::Error> {
        // 在 Apple OS 上，纪元（epoch）之前的日期表示方式与其他 Unix 平台不同：
        // 例如纪元前 1/10 秒，在其他平台上表示为 `seconds=-1` 且
        // `nanoseconds=100_000_000`，但在 Apple OS 上表示为 `seconds=0` 且
        // `nanoseconds=-900_000_000`。
        //
        // 为补偿这一点，我们先通过检查 seconds 和 nanoseconds 是否都在范围内来检测
        // 这一特殊情况，然后再校正 seconds 和 nanoseconds 的值，使其与常见的 unix
        // 表示一致。
        //
        // 请注意，Apple OS 在设置文件时间时同样接受标准 unix 格式，这使得本补偿可以
        // 往返一致（round-trippable），并且通常是透明的。
        #[cfg(target_vendor = "apple")]
        let (tv_sec, tv_nsec) =
            if (tv_sec <= 0 && tv_sec > i64::MIN) && (tv_nsec < 0 && tv_nsec > -1_000_000_000) {
                (tv_sec - 1, tv_nsec + 1_000_000_000)
            } else {
                (tv_sec, tv_nsec)
            };
        if tv_nsec >= 0 && tv_nsec < NSEC_PER_SEC as i64 {
            Ok(unsafe { Self::new_unchecked(tv_sec, tv_nsec) })
        } else {
            Err(io::const_error!(io::ErrorKind::InvalidData, "invalid timestamp"))
        }
    }

    pub fn now(clock: libc::clockid_t) -> Timespec {
        use crate::mem::MaybeUninit;
        use crate::sys::cvt;

        // 为应对 Y2038 问题，尽量使用 64 位时间。
        #[cfg(all(
            target_os = "linux",
            target_env = "gnu",
            target_pointer_width = "32",
            not(target_arch = "riscv32")
        ))]
        {
            use crate::sys::weak::weak;

            // __clock_gettime64 在 glibc 2.34 中被加入到 32 位架构，
            // 它会自行处理 vDSO 调用以及 ENOSYS 回退。
            weak!(
                fn __clock_gettime64(
                    clockid: libc::clockid_t,
                    tp: *mut __timespec64,
                ) -> libc::c_int;
            );

            if let Some(clock_gettime64) = __clock_gettime64.get() {
                let mut t = MaybeUninit::uninit();
                cvt(unsafe { clock_gettime64(clock, t.as_mut_ptr()) }).unwrap();
                let t = unsafe { t.assume_init() };
                return Timespec::new(t.tv_sec as i64, t.tv_nsec as i64).unwrap();
            }
        }

        let mut t = MaybeUninit::uninit();
        cvt(unsafe { libc::clock_gettime(clock, t.as_mut_ptr()) }).unwrap();
        let t = unsafe { t.assume_init() };
        Timespec::new(t.tv_sec as i64, t.tv_nsec as i64).unwrap()
    }

    pub fn sub_timespec(&self, other: &Timespec) -> Result<Duration, Duration> {
        // 当 a >= b 时，差值能放进 u64。
        fn sub_ge_to_unsigned(a: i64, b: i64) -> u64 {
            debug_assert!(a >= b);
            a.wrapping_sub(b).cast_unsigned()
        }

        if self >= other {
            let (secs, nsec) = if self.tv_nsec.as_inner() >= other.tv_nsec.as_inner() {
                (
                    sub_ge_to_unsigned(self.tv_sec, other.tv_sec),
                    self.tv_nsec.as_inner() - other.tv_nsec.as_inner(),
                )
            } else {
                // 下面这一连串断言解释了为何 `self.tv_sec - 1` 不会下溢。
                debug_assert!(self.tv_nsec < other.tv_nsec);
                debug_assert!(self.tv_sec > other.tv_sec);
                debug_assert!(self.tv_sec > i64::MIN);
                (
                    sub_ge_to_unsigned(self.tv_sec - 1, other.tv_sec),
                    self.tv_nsec.as_inner() + (NSEC_PER_SEC as u32) - other.tv_nsec.as_inner(),
                )
            };

            Ok(Duration::new(secs, nsec))
        } else {
            match other.sub_timespec(self) {
                Ok(d) => Err(d),
                Err(d) => Ok(d),
            }
        }
    }

    pub fn checked_add_duration(&self, other: &Duration) -> Option<Timespec> {
        let mut secs = self.tv_sec.checked_add_unsigned(other.as_secs())?;

        // 纳秒的计算不会溢出，因为纳秒数小于 10 亿，能放进 u32。
        let mut nsec = other.subsec_nanos() + self.tv_nsec.as_inner();
        if nsec >= NSEC_PER_SEC as u32 {
            nsec -= NSEC_PER_SEC as u32;
            secs = secs.checked_add(1)?;
        }
        Some(unsafe { Timespec::new_unchecked(secs, nsec.into()) })
    }

    pub fn checked_sub_duration(&self, other: &Duration) -> Option<Timespec> {
        let mut secs = self.tv_sec.checked_sub_unsigned(other.as_secs())?;

        // 与上面类似，纳秒不会溢出。
        let mut nsec = self.tv_nsec.as_inner() as i32 - other.subsec_nanos() as i32;
        if nsec < 0 {
            nsec += NSEC_PER_SEC as i32;
            secs = secs.checked_sub(1)?;
        }
        Some(unsafe { Timespec::new_unchecked(secs, nsec.into()) })
    }

    #[allow(dead_code)]
    pub fn to_timespec(&self) -> Option<libc::timespec> {
        Some(libc::timespec {
            tv_sec: self.tv_sec.try_into().ok()?,
            tv_nsec: self.tv_nsec.as_inner().try_into().ok()?,
        })
    }

    // 在 QNX Neutrino 上，例如 pthread_cond_timedwait 的最大 timespec
    // 是 2^64 纳秒
    #[cfg(target_os = "nto")]
    pub(in crate::sys) fn to_timespec_capped(&self) -> Option<libc::timespec> {
        // 检查以纳秒为单位的超时是否能放进 u64
        if (self.tv_nsec.as_inner() as u64)
            .checked_add((self.tv_sec as u64).checked_mul(NSEC_PER_SEC)?)
            .is_none()
        {
            return None;
        }
        self.to_timespec()
    }

    #[cfg(all(
        target_os = "linux",
        target_env = "gnu",
        target_pointer_width = "32",
        not(target_arch = "riscv32")
    ))]
    pub fn to_timespec64(&self) -> __timespec64 {
        __timespec64::new(self.tv_sec, self.tv_nsec.as_inner() as _)
    }
}

#[cfg(all(
    target_os = "linux",
    target_env = "gnu",
    target_pointer_width = "32",
    not(target_arch = "riscv32")
))]
#[repr(C)]
pub(crate) struct __timespec64 {
    pub(crate) tv_sec: i64,
    #[cfg(target_endian = "big")]
    _padding: i32,
    pub(crate) tv_nsec: i32,
    #[cfg(target_endian = "little")]
    _padding: i32,
}

#[cfg(all(
    target_os = "linux",
    target_env = "gnu",
    target_pointer_width = "32",
    not(target_arch = "riscv32")
))]
impl __timespec64 {
    pub(crate) fn new(tv_sec: i64, tv_nsec: i32) -> Self {
        Self { tv_sec, tv_nsec, _padding: 0 }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Instant {
    t: Timespec,
}

impl Instant {
    #[cfg(target_vendor = "apple")]
    pub(crate) const CLOCK_ID: libc::clockid_t = libc::CLOCK_UPTIME_RAW;
    #[cfg(not(target_vendor = "apple"))]
    pub(crate) const CLOCK_ID: libc::clockid_t = libc::CLOCK_MONOTONIC;
    pub fn now() -> Instant {
        // https://pubs.opengroup.org/onlinepubs/9799919799/functions/clock_getres.html
        //
        // CLOCK_UPTIME_RAW   一个单调递增的时钟，与 CLOCK_MONOTONIC_RAW 的方式相同，
        //                    但在系统处于睡眠状态时不会递增。其返回值与对
        //                    mach_absolute_time() 的结果施加适当的 mach_timebase
        //                    转换后的结果一致。
        //
        // macos 上的 Instant 历史上是用 mach_absolute_time 实现的；
        // 出于格外谨慎，我们保留这个值域（value domain）。
        Instant { t: Timespec::now(Self::CLOCK_ID) }
    }

    pub fn checked_sub_instant(&self, other: &Instant) -> Option<Duration> {
        self.t.sub_timespec(&other.t).ok()
    }

    pub fn checked_add_duration(&self, other: &Duration) -> Option<Instant> {
        Some(Instant { t: self.t.checked_add_duration(other)? })
    }

    pub fn checked_sub_duration(&self, other: &Duration) -> Option<Instant> {
        Some(Instant { t: self.t.checked_sub_duration(other)? })
    }

    #[cfg_attr(
        not(target_os = "linux"),
        allow(unused, reason = "needed by the `sleep_until` on some unix platforms")
    )]
    pub(crate) fn into_timespec(self) -> Timespec {
        self.t
    }
}

impl AsInner<Timespec> for Instant {
    fn as_inner(&self) -> &Timespec {
        &self.t
    }
}

impl fmt::Debug for Instant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Instant")
            .field("tv_sec", &self.t.tv_sec)
            .field("tv_nsec", &self.t.tv_nsec)
            .finish()
    }
}
