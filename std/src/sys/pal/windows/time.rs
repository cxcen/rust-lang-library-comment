use core::hash::{Hash, Hasher};
use core::ops::Neg;

use crate::cmp::Ordering;
use crate::ptr::null;
use crate::sys::{IntoInner, c};
use crate::time::Duration;
use crate::{fmt, mem};

const NANOS_PER_SEC: u64 = 1_000_000_000;
const INTERVALS_PER_SEC: u64 = NANOS_PER_SEC / 100;

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Debug, Hash)]
pub struct Instant {
    // 这个 duration 是相对于一个任意的微秒纪元（epoch）而言的，
    // 该纪元来自 winapi 的 QueryPerformanceCounter 函数。
    t: Duration,
}

#[derive(Copy, Clone)]
pub struct SystemTime {
    t: c::FILETIME,
}

const INTERVALS_TO_UNIX_EPOCH: u64 = 11_644_473_600 * INTERVALS_PER_SEC;

pub const UNIX_EPOCH: SystemTime = SystemTime {
    t: c::FILETIME {
        dwLowDateTime: INTERVALS_TO_UNIX_EPOCH as u32,
        dwHighDateTime: (INTERVALS_TO_UNIX_EPOCH >> 32) as u32,
    },
};

impl Instant {
    pub fn now() -> Instant {
        // Windows 上的高精度计时以“性能计数器（Performance Counter）”为单位，
        // 由 WINAPI 的 QueryPerformanceCounter 函数返回。它们与秒之间的换算系数
        // 是 QueryPerformanceFrequency。为了让常规的时间间隔运算中不掺杂单位换算，
        // 我们以 QPC 为单位测量，并立即转换为纳秒。
        perf_counter::PerformanceCounterInstant::now().into()
    }

    pub fn checked_sub_instant(&self, other: &Instant) -> Option<Duration> {
        // 在 Windows 上存在一个阈值，低于该阈值时我们会因测量误差而认为两个时间戳
        // 是等价的。更多细节及文档链接，请参见 epsilon 上的文档。
        let epsilon = perf_counter::PerformanceCounterInstant::epsilon();
        if other.t > self.t && other.t - self.t <= epsilon {
            Some(Duration::new(0, 0))
        } else {
            self.t.checked_sub(other.t)
        }
    }

    pub fn checked_add_duration(&self, other: &Duration) -> Option<Instant> {
        Some(Instant { t: self.t.checked_add(*other)? })
    }

    pub fn checked_sub_duration(&self, other: &Duration) -> Option<Instant> {
        Some(Instant { t: self.t.checked_sub(*other)? })
    }
}

impl SystemTime {
    pub const MAX: SystemTime = SystemTime {
        t: c::FILETIME {
            dwLowDateTime: (i64::MAX & 0xFFFFFFFF) as u32,
            dwHighDateTime: (i64::MAX >> 32) as u32,
        },
    };

    pub const MIN: SystemTime =
        SystemTime { t: c::FILETIME { dwLowDateTime: 0, dwHighDateTime: 0 } };

    pub fn now() -> SystemTime {
        unsafe {
            let mut t: SystemTime = mem::zeroed();
            c::GetSystemTimePreciseAsFileTime(&mut t.t);
            t
        }
    }

    fn from_intervals(intervals: i64) -> SystemTime {
        SystemTime {
            t: c::FILETIME {
                dwLowDateTime: intervals as u32,
                dwHighDateTime: (intervals >> 32) as u32,
            },
        }
    }

    fn intervals(&self) -> i64 {
        (self.t.dwLowDateTime as i64) | ((self.t.dwHighDateTime as i64) << 32)
    }

    pub fn sub_time(&self, other: &SystemTime) -> Result<Duration, Duration> {
        let me = self.intervals();
        let other = other.intervals();
        if me >= other {
            Ok(intervals2dur((me - other) as u64))
        } else {
            Err(intervals2dur((other - me) as u64))
        }
    }

    pub fn checked_add_duration(&self, other: &Duration) -> Option<SystemTime> {
        let intervals = self.intervals().checked_add(checked_dur2intervals(other)?)?;
        Some(SystemTime::from_intervals(intervals))
    }

    pub fn checked_sub_duration(&self, other: &Duration) -> Option<SystemTime> {
        // Windows 不支持 1601 年之前的时间，这也是我们不支持负值的原因。
        // 为了解决这一点，我们尝试把计算结果转换为 u64，显然，如果该值
        // 小于零，转换就会失败。
        let intervals: u64 =
            self.intervals().checked_sub(checked_dur2intervals(other)?)?.try_into().ok()?;
        Some(SystemTime::from_intervals(intervals as i64))
    }
}

impl PartialEq for SystemTime {
    fn eq(&self, other: &SystemTime) -> bool {
        self.intervals() == other.intervals()
    }
}

impl Eq for SystemTime {}

impl PartialOrd for SystemTime {
    fn partial_cmp(&self, other: &SystemTime) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SystemTime {
    fn cmp(&self, other: &SystemTime) -> Ordering {
        self.intervals().cmp(&other.intervals())
    }
}

impl fmt::Debug for SystemTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SystemTime").field("intervals", &self.intervals()).finish()
    }
}

impl From<c::FILETIME> for SystemTime {
    fn from(t: c::FILETIME) -> SystemTime {
        SystemTime { t }
    }
}

impl IntoInner<c::FILETIME> for SystemTime {
    fn into_inner(self) -> c::FILETIME {
        self.t
    }
}

impl Hash for SystemTime {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.intervals().hash(state)
    }
}

fn checked_dur2intervals(dur: &Duration) -> Option<i64> {
    dur.as_secs()
        .checked_mul(INTERVALS_PER_SEC)?
        .checked_add(dur.subsec_nanos() as u64 / 100)?
        .try_into()
        .ok()
}

fn intervals2dur(intervals: u64) -> Duration {
    Duration::new(intervals / INTERVALS_PER_SEC, ((intervals % INTERVALS_PER_SEC) * 100) as u32)
}

mod perf_counter {
    use super::NANOS_PER_SEC;
    use crate::sync::atomic::{Atomic, AtomicU64, Ordering};
    use crate::sys::helpers::mul_div_u64;
    use crate::sys::{c, cvt};
    use crate::time::Duration;

    pub struct PerformanceCounterInstant {
        ts: i64,
    }
    impl PerformanceCounterInstant {
        pub fn now() -> Self {
            Self { ts: query() }
        }

        // 根据微软的文档，使用 QueryPerformanceCounter 进行跨线程时间比较时，
        // 误差范围是 1 个“tick”——定义为 1/frequency()。
        // 参考：https://docs.microsoft.com/en-us/windows/desktop/SysInfo
        //                   /acquiring-high-resolution-time-stamps
        pub fn epsilon() -> Duration {
            let epsilon = NANOS_PER_SEC / (frequency() as u64);
            Duration::from_nanos(epsilon)
        }
    }
    impl From<PerformanceCounterInstant> for super::Instant {
        fn from(other: PerformanceCounterInstant) -> Self {
            let freq = frequency() as u64;
            let instant_nsec = mul_div_u64(other.ts as u64, NANOS_PER_SEC, freq);
            Self { t: Duration::from_nanos(instant_nsec) }
        }
    }

    fn frequency() -> i64 {
        // 要么是 `QueryPerformanceFrequency` 的缓存结果，要么是表示未初始化的 `0`。
        // 将其存储为单个 `AtomicU64` 使我们能够使用 `Relaxed` 操作，因为我们只关心
        // 对单个内存位置的影响。
        static FREQUENCY: Atomic<u64> = AtomicU64::new(0);

        let cached = FREQUENCY.load(Ordering::Relaxed);
        // 如果之前有某个线程已经填好了这个全局状态，就用它。
        if cached != 0 {
            return cached as i64;
        }
        // ……否则我们自己来获取……
        let mut frequency = 0;
        unsafe {
            cvt(c::QueryPerformanceFrequency(&mut frequency)).unwrap();
        }

        FREQUENCY.store(frequency as u64, Ordering::Relaxed);
        frequency
    }

    fn query() -> i64 {
        let mut qpc_value: i64 = 0;
        cvt(unsafe { c::QueryPerformanceCounter(&mut qpc_value) }).unwrap();
        qpc_value
    }
}

/// 一个可供等待的定时器。
pub(crate) struct WaitableTimer {
    handle: c::HANDLE,
}
impl WaitableTimer {
    /// 创建一个高分辨率定时器。在 Windows 10 version 1803 之前会失败。
    pub fn high_resolution() -> Result<Self, ()> {
        let handle = unsafe {
            c::CreateWaitableTimerExW(
                null(),
                null(),
                c::CREATE_WAITABLE_TIMER_HIGH_RESOLUTION,
                c::TIMER_ALL_ACCESS,
            )
        };
        if !handle.is_null() { Ok(Self { handle }) } else { Err(()) }
    }
    pub fn set(&self, duration: Duration) -> Result<(), ()> {
        // 将 Duration 转换为类似 FILETIME 的格式。
        // 负值表示相对时间，正值表示绝对时间。
        // 因此我们对这个相对的 duration 取负。
        let time = checked_dur2intervals(&duration).ok_or(())?.neg();
        let result = unsafe { c::SetWaitableTimer(self.handle, &time, 0, None, null(), c::FALSE) };
        if result != 0 { Ok(()) } else { Err(()) }
    }
    pub fn wait(&self) -> Result<(), ()> {
        let result = unsafe { c::WaitForSingleObject(self.handle, c::INFINITE) };
        if result != c::WAIT_FAILED { Ok(()) } else { Err(()) }
    }
}
impl Drop for WaitableTimer {
    fn drop(&mut self) {
        unsafe { c::CloseHandle(self.handle) };
    }
}
