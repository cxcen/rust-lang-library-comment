//! 时间的量化（Temporal quantification）。
//!
//! 本模块在 `core::time::Duration` 的基础上，提供两种与操作系统时钟绑定的时间度量类型：
//! [`Instant`]（单调不减的时钟，用于测量经过的时长）与 [`SystemTime`]（系统挂钟时间，
//! 可被系统时钟调整而回拨）。两者都是不透明类型，只有借助 [`Duration`] 才有意义。
//! 它们各自维护的不变量、平台差异以及失败如何暴露，分别见两个类型的文档。
//!
//! # 示例
//!
//! 创建一个新的 [`Duration`] 有多种方式：
//!
//! ```
//! # use std::time::Duration;
//! let five_seconds = Duration::from_secs(5);
//! assert_eq!(five_seconds, Duration::from_millis(5_000));
//! assert_eq!(five_seconds, Duration::from_micros(5_000_000));
//! assert_eq!(five_seconds, Duration::from_nanos(5_000_000_000));
//!
//! let ten_seconds = Duration::from_secs(10);
//! let seven_nanos = Duration::from_nanos(7);
//! let total = ten_seconds + seven_nanos;
//! assert_eq!(total, Duration::new(10, 7));
//! ```
//!
//! 使用 [`Instant`] 计算一个函数运行了多久：
//!
//! ```ignore (incomplete)
//! let now = Instant::now();
//!
//! // 调用一个慢函数，它可能要执行一阵子
//! slow_function();
//!
//! let elapsed_time = now.elapsed();
//! println!("Running slow_function() took {} seconds.", elapsed_time.as_secs());
//! ```

#![stable(feature = "time", since = "1.3.0")]

#[stable(feature = "time", since = "1.3.0")]
pub use core::time::Duration;
#[stable(feature = "duration_checked_float", since = "1.66.0")]
pub use core::time::TryFromFloatSecsError;

use crate::error::Error;
use crate::fmt;
use crate::ops::{Add, AddAssign, Sub, SubAssign};
use crate::sys::{FromInner, IntoInner, time};

/// 对一个单调不减（monotonically nondecreasing）时钟的度量。
/// 它是不透明的，只有配合 [`Duration`] 使用才有意义。
///
/// 除非遇到[平台 bug][platform bugs]，`Instant` 总是保证：新创建的 instant 不小于此前测得的
/// 任何 instant。它常用于测量基准（benchmark）或对某个操作计时。
///
/// 但请注意，instant **不**保证是**稳定（steady）**的。换言之，底层时钟的每一次 tick
/// 长度未必相同（例如有些秒可能比其他秒更长）。instant 可能向前跳变，或经历时间膨胀
/// （time dilation，即变慢或变快），但它绝不会倒退。
/// 作为这一“非保证”的一部分，系统挂起（suspend）是否计入经过的时间也未作规定，
/// 该行为在不同平台和不同 Rust 版本之间各不相同。
///
/// instant 是不透明类型，只能彼此比较。没有任何方法能从 instant 取出“多少秒”这样的值。
/// 它只允许测量两个 instant 之间的时长（或比较两个 instant）。
///
/// `Instant` 结构体的大小可能因目标操作系统而异。
///
/// 示例：
///
/// ```no_run
/// use std::time::{Duration, Instant};
/// use std::thread::sleep;
///
/// fn main() {
///    let now = Instant::now();
///
///    // 我们睡眠 2 秒
///    sleep(Duration::new(2, 0));
///    // 打印 '2'
///    println!("{}", now.elapsed().as_secs());
/// }
/// ```
///
/// [platform bugs]: Instant#monotonicity
///
/// # 操作系统特定行为
///
/// `Instant` 是对各系统特定类型的封装，其行为可能因底层操作系统而异。例如，
/// 以下代码片段在 Linux 上没问题，但在 macOS 上会 panic：
///
/// ```no_run
/// use std::time::{Instant, Duration};
///
/// let now = Instant::now();
/// let days_per_10_millennia = 365_2425;
/// let solar_seconds_per_day = 60 * 60 * 24;
/// let millennium_in_solar_seconds = 31_556_952_000;
/// assert_eq!(millennium_in_solar_seconds, days_per_10_millennia * solar_seconds_per_day / 10);
///
/// let duration = Duration::new(millennium_in_solar_seconds, 0);
/// println!("{:?}", now + duration);
/// ```
///
/// 对于跨平台代码，可以放心使用最长约一百年的时长（duration）。
///
/// # 底层系统调用
///
/// `now()` [目前][currently]使用以下系统调用来获取当前时间：
///
/// |  Platform |               System call                                            |
/// |-----------|----------------------------------------------------------------------|
/// | SGX       | [`insecure_time` usercall]. More information on [timekeeping in SGX] |
/// | UNIX      | [clock_gettime] with `CLOCK_MONOTONIC`                               |
/// | Darwin    | [clock_gettime] with `CLOCK_UPTIME_RAW`                              |
/// | VXWorks   | [clock_gettime] with `CLOCK_MONOTONIC`                               |
/// | SOLID     | `get_tim`                                                            |
/// | WASI      | [__wasi_clock_time_get] with `monotonic`                             |
/// | Windows   | [QueryPerformanceCounter]                                            |
///
/// [currently]: crate::io#platform-specific-behavior
/// [QueryPerformanceCounter]: https://docs.microsoft.com/en-us/windows/win32/api/profileapi/nf-profileapi-queryperformancecounter
/// [`insecure_time` usercall]: https://edp.fortanix.com/docs/api/fortanix_sgx_abi/struct.Usercalls.html#method.insecure_time
/// [timekeeping in SGX]: https://edp.fortanix.com/docs/concepts/rust-std/#codestdtimecode
/// [__wasi_clock_time_get]: https://github.com/WebAssembly/WASI/blob/main/legacy/preview1/docs.md#clock_time_get
/// [clock_gettime]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/clock_getres.html
///
/// **免责声明（Disclaimer）：** 这些系统调用可能随时间变化。
///
/// > 注意：诸如 [`add`] 之类的数学运算可能会 panic，如果底层结构无法表示新的时间点的话。
///
/// [`add`]: Instant::add
///
/// ## 单调性（Monotonicity）
///
/// 在所有平台上，`Instant` 都会尽量使用保证单调行为的 OS API（如果存在的话），
/// 对所有 [tier 1] 平台而言确实存在这样的 API。
/// 实践中，这类保证在极少数情况下会被硬件、虚拟化或操作系统的 bug 打破。为了绕过这些 bug，
/// 以及应对不提供单调时钟的平台，[`duration_since`]、[`elapsed`] 和 [`sub`] 会饱和（saturate）到零。
/// 在较旧的 Rust 版本中，这种情形会导致 panic。可以用 [`checked_duration_since`] 来检测并处理
/// 单调性被违反、或 `Instant` 相减顺序写反的情况。
///
/// 这种绕过手段会掩盖一类编程错误：把较早和较晚的 instant 不小心写反。出于这个原因，
/// 未来的 Rust 版本可能会重新引入 panic。
///
/// [tier 1]: https://doc.rust-lang.org/rustc/platform-support.html
/// [`duration_since`]: Instant::duration_since
/// [`elapsed`]: Instant::elapsed
/// [`sub`]: Instant::sub
/// [`checked_duration_since`]: Instant::checked_duration_since
///
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[stable(feature = "time2", since = "1.8.0")]
#[cfg_attr(not(test), rustc_diagnostic_item = "Instant")]
pub struct Instant(time::Instant);

/// 对系统时钟的度量，适用于与外部实体（如文件系统或其他进程）交互。
///
/// 与 [`Instant`] 类型不同，这种时间度量**不是单调的**。这意味着：你可以先把一个文件保存到
/// 文件系统，再保存另一个文件，**而第二个文件的 `SystemTime` 度量值却可能早于第一个**。
/// 换言之，在真实时间里发生得更晚的操作，其 `SystemTime` 反而可能更早！
///
/// 因此，比较两个 `SystemTime` 实例以得知它们之间的时长时，返回的是 [`Result`] 而非
/// 不会失败的 [`Duration`]，以表明这种时间漂移（time drift）可能发生、需要被处理。
///
/// 虽然 `SystemTime` 不能被直接检视，但本模块提供了 [`UNIX_EPOCH`] 常量作为时间锚点，
/// 用于了解某个 `SystemTime` 的信息。通过计算相对这个固定时间点的时长，
/// 可以把 `SystemTime` 转换成人类可读的时间，或者某种字符串表示。
///
/// `SystemTime` 结构体的大小可能因目标操作系统而异。
///
/// `SystemTime` 不计入闰秒（leap second）。
/// `SystemTime::now()` 在闰秒附近的行为与操作系统的挂钟一致。
/// 闰秒附近的精确行为（例如时钟看起来变慢、变快、停止还是跳变）取决于平台和配置，
/// 因此不应依赖它。
///
/// 示例：
///
/// ```no_run
/// use std::time::{Duration, SystemTime};
/// use std::thread::sleep;
///
/// fn main() {
///    let now = SystemTime::now();
///
///    // 我们睡眠 2 秒
///    sleep(Duration::new(2, 0));
///    match now.elapsed() {
///        Ok(elapsed) => {
///            // 打印 '2'
///            println!("{}", elapsed.as_secs());
///        }
///        Err(e) => {
///            // 系统时钟倒退了！
///            println!("Great Scott! {e:?}");
///        }
///    }
/// }
/// ```
///
/// # 平台特定行为
///
/// `SystemTime` 的精度可能取决于底层 OS 特定的时间格式。例如，在 Windows 上时间以 100 纳秒
/// 为间隔表示，而 Linux 可以表示纳秒级间隔。
///
/// `now()` [目前][currently]使用以下系统调用来获取当前时间：
///
/// |  Platform |               System call                                            |
/// |-----------|----------------------------------------------------------------------|
/// | SGX       | [`insecure_time` usercall]. More information on [timekeeping in SGX] |
/// | UNIX      | [clock_gettime (Realtime Clock)]                                     |
/// | Darwin    | [clock_gettime (Realtime Clock)]                                     |
/// | VXWorks   | [clock_gettime (Realtime Clock)]                                     |
/// | SOLID     | `SOLID_RTC_ReadTime`                                                 |
/// | WASI      | [__wasi_clock_time_get (Realtime Clock)]                             |
/// | Windows   | [GetSystemTimePreciseAsFileTime] / [GetSystemTimeAsFileTime]         |
///
/// [currently]: crate::io#platform-specific-behavior
/// [`insecure_time` usercall]: https://edp.fortanix.com/docs/api/fortanix_sgx_abi/struct.Usercalls.html#method.insecure_time
/// [timekeeping in SGX]: https://edp.fortanix.com/docs/concepts/rust-std/#codestdtimecode
/// [clock_gettime (Realtime Clock)]: https://pubs.opengroup.org/onlinepubs/9799919799/functions/clock_getres.html
/// [__wasi_clock_time_get (Realtime Clock)]: https://github.com/WebAssembly/WASI/blob/main/legacy/preview1/docs.md#clock_time_get
/// [GetSystemTimePreciseAsFileTime]: https://docs.microsoft.com/en-us/windows/win32/api/sysinfoapi/nf-sysinfoapi-getsystemtimepreciseasfiletime
/// [GetSystemTimeAsFileTime]: https://docs.microsoft.com/en-us/windows/win32/api/sysinfoapi/nf-sysinfoapi-getsystemtimeasfiletime
///
/// **免责声明（Disclaimer）：** 这些系统调用可能随时间变化。
///
/// > 注意：诸如 [`add`] 之类的数学运算可能会 panic，如果底层结构无法表示新的时间点的话。
///
/// [`add`]: SystemTime::add
/// [`UNIX_EPOCH`]: SystemTime::UNIX_EPOCH
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[stable(feature = "time2", since = "1.8.0")]
pub struct SystemTime(time::SystemTime);

/// 由 `SystemTime` 上的 `duration_since` 和 `elapsed` 方法返回的错误，
/// 用于得知系统时间在反方向上偏离了多远。
///
/// # 示例
///
/// ```no_run
/// use std::thread::sleep;
/// use std::time::{Duration, SystemTime};
///
/// let sys_time = SystemTime::now();
/// sleep(Duration::from_secs(1));
/// let new_sys_time = SystemTime::now();
/// match sys_time.duration_since(new_sys_time) {
///     Ok(_) => {}
///     Err(e) => println!("SystemTimeError difference: {:?}", e.duration()),
/// }
/// ```
#[derive(Clone, Debug)]
#[stable(feature = "time2", since = "1.8.0")]
pub struct SystemTimeError(Duration);

impl Instant {
    /// 返回对应于“现在（now）”的 instant。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::time::Instant;
    ///
    /// let now = Instant::now();
    /// ```
    #[must_use]
    #[stable(feature = "time2", since = "1.8.0")]
    #[cfg_attr(not(test), rustc_diagnostic_item = "instant_now")]
    pub fn now() -> Instant {
        Instant(time::Instant::now())
    }

    /// 返回从另一个 instant 到本 instant 所经过的时长；
    /// 如果那个 instant 晚于本 instant，则返回零时长。
    ///
    /// # Panics
    ///
    /// 旧版 Rust 在 `earlier` 晚于 `self` 时会 panic。当前此方法改为饱和（saturate）处理。
    /// 未来版本可能在某些情形下重新引入 panic。参见[单调性][Monotonicity]。
    ///
    /// [Monotonicity]: Instant#monotonicity
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::time::{Duration, Instant};
    /// use std::thread::sleep;
    ///
    /// let now = Instant::now();
    /// sleep(Duration::new(1, 0));
    /// let new_now = Instant::now();
    /// println!("{:?}", new_now.duration_since(now));
    /// println!("{:?}", now.duration_since(new_now)); // 0ns
    /// ```
    #[must_use]
    #[stable(feature = "time2", since = "1.8.0")]
    pub fn duration_since(&self, earlier: Instant) -> Duration {
        self.checked_duration_since(earlier).unwrap_or_default()
    }

    /// 返回从另一个 instant 到本 instant 所经过的时长；
    /// 如果那个 instant 晚于本 instant，则返回 None。
    ///
    /// 由于[单调性 bug][monotonicity bugs]，即便传入的 `Instant` 在逻辑顺序上是正确的，
    /// 此方法也可能返回 `None`。
    ///
    /// [monotonicity bugs]: Instant#monotonicity
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::time::{Duration, Instant};
    /// use std::thread::sleep;
    ///
    /// let now = Instant::now();
    /// sleep(Duration::new(1, 0));
    /// let new_now = Instant::now();
    /// println!("{:?}", new_now.checked_duration_since(now));
    /// println!("{:?}", now.checked_duration_since(new_now)); // None
    /// ```
    #[must_use]
    #[stable(feature = "checked_duration_since", since = "1.39.0")]
    pub fn checked_duration_since(&self, earlier: Instant) -> Option<Duration> {
        self.0.checked_sub_instant(&earlier.0)
    }

    /// 返回从另一个 instant 到本 instant 所经过的时长；
    /// 如果那个 instant 晚于本 instant，则返回零时长。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::time::{Duration, Instant};
    /// use std::thread::sleep;
    ///
    /// let now = Instant::now();
    /// sleep(Duration::new(1, 0));
    /// let new_now = Instant::now();
    /// println!("{:?}", new_now.saturating_duration_since(now));
    /// println!("{:?}", now.saturating_duration_since(new_now)); // 0ns
    /// ```
    #[must_use]
    #[stable(feature = "checked_duration_since", since = "1.39.0")]
    pub fn saturating_duration_since(&self, earlier: Instant) -> Duration {
        self.checked_duration_since(earlier).unwrap_or_default()
    }

    /// 返回自本 instant 以来所经过的时长。
    ///
    /// # Panics
    ///
    /// 旧版 Rust 在当前时间早于 self 时会 panic。当前此方法在该情形下返回零时长。
    /// 未来版本可能重新引入 panic。参见[单调性][Monotonicity]。
    ///
    /// [Monotonicity]: Instant#monotonicity
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::thread::sleep;
    /// use std::time::{Duration, Instant};
    ///
    /// let instant = Instant::now();
    /// let three_secs = Duration::from_secs(3);
    /// sleep(three_secs);
    /// assert!(instant.elapsed() >= three_secs);
    /// ```
    #[must_use]
    #[stable(feature = "time2", since = "1.8.0")]
    pub fn elapsed(&self) -> Duration {
        Instant::now() - *self
    }

    /// 返回 `Some(t)`，其中 `t` 为时间 `self + duration`，前提是 `t` 能表示为
    /// `Instant`（即落在底层数据结构的边界之内）；否则返回 `None`。
    #[stable(feature = "time_checked_add", since = "1.34.0")]
    pub fn checked_add(&self, duration: Duration) -> Option<Instant> {
        self.0.checked_add_duration(&duration).map(Instant)
    }

    /// 返回 `Some(t)`，其中 `t` 为时间 `self - duration`，前提是 `t` 能表示为
    /// `Instant`（即落在底层数据结构的边界之内）；否则返回 `None`。
    #[stable(feature = "time_checked_add", since = "1.34.0")]
    pub fn checked_sub(&self, duration: Duration) -> Option<Instant> {
        self.0.checked_sub_duration(&duration).map(Instant)
    }

    // 被平台特定的 `sleep_until` 实现使用，例如 Linux 上所用的那个。
    #[cfg_attr(
        not(target_os = "linux"),
        allow(unused, reason = "not every platform has a specific `sleep_until`")
    )]
    pub(crate) fn into_inner(self) -> time::Instant {
        self.0
    }
}

#[stable(feature = "time2", since = "1.8.0")]
impl Add<Duration> for Instant {
    type Output = Instant;

    /// # Panics
    ///
    /// 如果结果时间点无法被底层数据结构表示，此函数可能 panic。
    /// 不会 panic 的版本见 [`Instant::checked_add`]。
    fn add(self, other: Duration) -> Instant {
        self.checked_add(other).expect("overflow when adding duration to instant")
    }
}

#[stable(feature = "time_augmented_assignment", since = "1.9.0")]
impl AddAssign<Duration> for Instant {
    fn add_assign(&mut self, other: Duration) {
        *self = *self + other;
    }
}

#[stable(feature = "time2", since = "1.8.0")]
impl Sub<Duration> for Instant {
    type Output = Instant;

    fn sub(self, other: Duration) -> Instant {
        self.checked_sub(other).expect("overflow when subtracting duration from instant")
    }
}

#[stable(feature = "time_augmented_assignment", since = "1.9.0")]
impl SubAssign<Duration> for Instant {
    fn sub_assign(&mut self, other: Duration) {
        *self = *self - other;
    }
}

#[stable(feature = "time2", since = "1.8.0")]
impl Sub<Instant> for Instant {
    type Output = Duration;

    /// 返回从另一个 instant 到本 instant 所经过的时长；
    /// 如果那个 instant 晚于本 instant，则返回零时长。
    ///
    /// # Panics
    ///
    /// 旧版 Rust 在 `other` 晚于 `self` 时会 panic。当前此方法改为饱和（saturate）处理。
    /// 未来版本可能在某些情形下重新引入 panic。参见[单调性][Monotonicity]。
    ///
    /// [Monotonicity]: Instant#monotonicity
    fn sub(self, other: Instant) -> Duration {
        self.duration_since(other)
    }
}

#[stable(feature = "time2", since = "1.8.0")]
impl fmt::Debug for Instant {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl SystemTime {
    /// 一个时间锚点，可用于创建新的 `SystemTime` 实例，或了解某个 `SystemTime` 处于时间轴上的位置。
    //
    // 注意！这段文档是重复的：此处与 std::time::UNIX_EPOCH 各有一份。
    // 由于命名不同，两份内容并不完全一致。
    ///
    /// 就系统时钟而言，该常量在所有系统上都被定义为 "1970-01-01 00:00:00 UTC"。
    /// 对一个已有的 `SystemTime` 实例调用 `duration_since`，可以得知它距离这个时间点有多远；
    /// 而用 `UNIX_EPOCH + duration` 则可以创建一个表示另一固定时间点的 `SystemTime` 实例。
    ///
    /// `duration_since(UNIX_EPOCH).unwrap().as_secs()` 返回自 1970 UTC 起始以来的非闰秒秒数。
    /// 这是一个 POSIX `time_t`（以 `u64` 表示），与许多互联网协议所用的时间表示相同。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::time::SystemTime;
    ///
    /// match SystemTime::now().duration_since(SystemTime::UNIX_EPOCH) {
    ///     Ok(n) => println!("1970-01-01 00:00:00 UTC was {} seconds ago!", n.as_secs()),
    ///     Err(_) => panic!("SystemTime before UNIX EPOCH!"),
    /// }
    /// ```
    #[stable(feature = "assoc_unix_epoch", since = "1.28.0")]
    pub const UNIX_EPOCH: SystemTime = UNIX_EPOCH;

    /// 表示本平台上 [`SystemTime`] 可表示的最大值。
    ///
    /// 该值在不同平台间差异很大，但始终满足：向 [`SystemTime::MAX`] 做任何正向加法，
    /// 只要所加 [`Duration`] 的值大于或等于操作系统的时间精度，就一定会失败。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// #![feature(time_systemtime_limits)]
    /// use std::time::{Duration, SystemTime};
    ///
    /// // 加上零不会改变任何东西。
    /// assert_eq!(SystemTime::MAX.checked_add(Duration::ZERO), Some(SystemTime::MAX));
    ///
    /// // 但仅仅加上一秒就已经会失败……
    /// //
    /// // 请记住：如果 Duration 小于操作系统的时间精度，这其实可能成功；
    /// // 在大多数操作系统上该精度恰好是 1ns，而 Windows 是个显著的例外，
    /// // 它使用 100ns，因此本示例使用 1s。
    /// assert_eq!(SystemTime::MAX.checked_add(Duration::new(1, 0)), None);
    ///
    /// // 利用它进行饱和算术以改进错误处理。
    /// // 这里我们以一个时间戳位于未来的证书作为实际例子。
    /// let configured_offset = Duration::from_secs(60 * 60 * 24);
    /// let valid_after =
    ///     SystemTime::now()
    ///         .checked_add(configured_offset)
    ///         .unwrap_or(SystemTime::MAX);
    /// ```
    #[unstable(feature = "time_systemtime_limits", issue = "149067")]
    pub const MAX: SystemTime = SystemTime(time::SystemTime::MAX);

    /// 表示本平台上 [`SystemTime`] 可表示的最小值。
    ///
    /// 该值在不同平台间差异很大，但始终满足：从 [`SystemTime::MIN`] 做任何正向减法，
    /// 只要所减 [`Duration`] 的值大于或等于操作系统的时间精度，就一定会失败。
    ///
    /// 取决于平台，该值可能小于或等于 [`SystemTime::UNIX_EPOCH`]，这取决于操作系统
    /// 是否支持表示 Unix epoch 之前的时间戳。不过始终保证 [`SystemTime::UNIX_EPOCH`]
    /// 落在 [`SystemTime::MIN`] 与 [`SystemTime::MAX`] 之间。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(time_systemtime_limits)]
    /// use std::time::{Duration, SystemTime};
    ///
    /// // 减去零不会改变任何东西。
    /// assert_eq!(SystemTime::MIN.checked_sub(Duration::ZERO), Some(SystemTime::MIN));
    ///
    /// // 但仅仅减去一秒就已经会失败。
    /// //
    /// // 请记住：如果 Duration 小于操作系统的时间精度，这其实可能成功；
    /// // 在大多数操作系统上该精度恰好是 1ns，而 Windows 是个显著的例外，
    /// // 它使用 100ns，因此本示例使用 1s。
    /// assert_eq!(SystemTime::MIN.checked_sub(Duration::new(1, 0)), None);
    ///
    /// // 利用它进行饱和算术以改进错误处理。
    /// // 这里我们以缓存过期（cache expiry）作为实际例子。
    /// let configured_expiry = Duration::from_secs(60 * 3);
    /// let expiry_threshold =
    ///     SystemTime::now()
    ///         .checked_sub(configured_expiry)
    ///         .unwrap_or(SystemTime::MIN);
    /// ```
    #[unstable(feature = "time_systemtime_limits", issue = "149067")]
    pub const MIN: SystemTime = SystemTime(time::SystemTime::MIN);

    /// 返回对应于“现在（now）”的系统时间。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::time::SystemTime;
    ///
    /// let sys_time = SystemTime::now();
    /// ```
    #[must_use]
    #[stable(feature = "time2", since = "1.8.0")]
    pub fn now() -> SystemTime {
        SystemTime(time::SystemTime::now())
    }

    /// 返回自某个更早时间点以来所经过的时长。
    ///
    /// 此函数可能失败，因为更早测得的时间不保证总是早于较晚测得的时间
    /// （由于诸如系统时钟被向前或向后调整之类的异常）。
    /// [`Instant`] 可用于测量经过的时间而没有这种失败风险。
    ///
    /// 如果成功，返回 <code>[Ok]\([Duration])</code>，其中 duration 表示从指定测量值到本测量值
    /// 所经过的时长。
    ///
    /// 如果 `earlier` 晚于 `self`，则返回 [`Err`]，错误中包含该时间距离 `self` 有多远。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::time::SystemTime;
    ///
    /// let sys_time = SystemTime::now();
    /// let new_sys_time = SystemTime::now();
    /// let difference = new_sys_time.duration_since(sys_time)
    ///     .expect("Clock may have gone backwards");
    /// println!("{difference:?}");
    /// ```
    #[stable(feature = "time2", since = "1.8.0")]
    pub fn duration_since(&self, earlier: SystemTime) -> Result<Duration, SystemTimeError> {
        self.0.sub_time(&earlier.0).map_err(SystemTimeError)
    }

    /// 返回从本系统时间到当前时钟时间的差值。
    ///
    /// 此函数可能失败，因为底层系统时钟易受漂移（drift）和更新影响（例如系统时钟可能倒退），
    /// 所以本函数未必总能成功。如果成功，返回 <code>[Ok]\([Duration])</code>，
    /// 其中 duration 表示从本次时间测量到当前时间所经过的时长。
    ///
    /// 要可靠地测量经过的时间，请改用 [`Instant`]。
    ///
    /// 如果 `self` 晚于当前系统时间，则返回 [`Err`]，错误中包含 `self` 距离当前系统时间有多远。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::thread::sleep;
    /// use std::time::{Duration, SystemTime};
    ///
    /// let sys_time = SystemTime::now();
    /// let one_sec = Duration::from_secs(1);
    /// sleep(one_sec);
    /// assert!(sys_time.elapsed().unwrap() >= one_sec);
    /// ```
    #[stable(feature = "time2", since = "1.8.0")]
    pub fn elapsed(&self) -> Result<Duration, SystemTimeError> {
        SystemTime::now().duration_since(*self)
    }

    /// 返回 `Some(t)`，其中 `t` 为时间 `self + duration`，前提是 `t` 能表示为
    /// `SystemTime`（即落在底层数据结构的边界之内）；否则返回 `None`。
    ///
    /// 如果 `duration` 小于操作系统的时间精度，则返回 `Some(self)`。
    #[stable(feature = "time_checked_add", since = "1.34.0")]
    pub fn checked_add(&self, duration: Duration) -> Option<SystemTime> {
        self.0.checked_add_duration(&duration).map(SystemTime)
    }

    /// 返回 `Some(t)`，其中 `t` 为时间 `self - duration`，前提是 `t` 能表示为
    /// `SystemTime`（即落在底层数据结构的边界之内）；否则返回 `None`。
    ///
    /// 如果 `duration` 小于操作系统的时间精度，则返回 `Some(self)`。
    #[stable(feature = "time_checked_add", since = "1.34.0")]
    pub fn checked_sub(&self, duration: Duration) -> Option<SystemTime> {
        self.0.checked_sub_duration(&duration).map(SystemTime)
    }
}

#[stable(feature = "time2", since = "1.8.0")]
impl Add<Duration> for SystemTime {
    type Output = SystemTime;

    /// # Panics
    ///
    /// 如果结果时间点无法被底层数据结构表示，此函数可能 panic。
    /// 不会 panic 的版本见 [`SystemTime::checked_add`]。
    fn add(self, dur: Duration) -> SystemTime {
        self.checked_add(dur).expect("overflow when adding duration to instant")
    }
}

#[stable(feature = "time_augmented_assignment", since = "1.9.0")]
impl AddAssign<Duration> for SystemTime {
    fn add_assign(&mut self, other: Duration) {
        *self = *self + other;
    }
}

#[stable(feature = "time2", since = "1.8.0")]
impl Sub<Duration> for SystemTime {
    type Output = SystemTime;

    fn sub(self, dur: Duration) -> SystemTime {
        self.checked_sub(dur).expect("overflow when subtracting duration from instant")
    }
}

#[stable(feature = "time_augmented_assignment", since = "1.9.0")]
impl SubAssign<Duration> for SystemTime {
    fn sub_assign(&mut self, other: Duration) {
        *self = *self - other;
    }
}

#[stable(feature = "time2", since = "1.8.0")]
impl fmt::Debug for SystemTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// 一个时间锚点，可用于创建新的 `SystemTime` 实例，或了解某个 `SystemTime` 处于时间轴上的位置。
//
// 注意！这段文档是重复的：此处与 SystemTime::UNIX_EPOCH 各有一份。
// 由于命名不同，两份内容并不完全一致。
///
/// 就系统时钟而言，该常量在所有系统上都被定义为 "1970-01-01 00:00:00 UTC"。
/// 对一个已有的 [`SystemTime`] 实例调用 `duration_since`，可以得知它距离这个时间点有多远；
/// 而用 `UNIX_EPOCH + duration` 则可以创建一个表示另一固定时间点的 [`SystemTime`] 实例。
///
/// `duration_since(UNIX_EPOCH).unwrap().as_secs()` 返回自 1970 UTC 起始以来的非闰秒秒数。
/// 这是一个 POSIX `time_t`（以 `u64` 表示），与许多互联网协议所用的时间表示相同。
///
/// # 示例
///
/// ```no_run
/// use std::time::{SystemTime, UNIX_EPOCH};
///
/// match SystemTime::now().duration_since(UNIX_EPOCH) {
///     Ok(n) => println!("1970-01-01 00:00:00 UTC was {} seconds ago!", n.as_secs()),
///     Err(_) => panic!("SystemTime before UNIX EPOCH!"),
/// }
/// ```
#[stable(feature = "time2", since = "1.8.0")]
pub const UNIX_EPOCH: SystemTime = SystemTime(time::UNIX_EPOCH);

impl SystemTimeError {
    /// 返回一个正的时长，表示第二个系统时间比第一个向前超出了多远。
    ///
    /// 每当第二个系统时间所表示的时间点晚于方法调用时的 `self`，
    /// [`SystemTime::duration_since`] 与 [`SystemTime::elapsed`] 方法就会返回一个
    /// `SystemTimeError`。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::thread::sleep;
    /// use std::time::{Duration, SystemTime};
    ///
    /// let sys_time = SystemTime::now();
    /// sleep(Duration::from_secs(1));
    /// let new_sys_time = SystemTime::now();
    /// match sys_time.duration_since(new_sys_time) {
    ///     Ok(_) => {}
    ///     Err(e) => println!("SystemTimeError difference: {:?}", e.duration()),
    /// }
    /// ```
    #[must_use]
    #[stable(feature = "time2", since = "1.8.0")]
    pub fn duration(&self) -> Duration {
        self.0
    }
}

#[stable(feature = "time2", since = "1.8.0")]
impl Error for SystemTimeError {}

#[stable(feature = "time2", since = "1.8.0")]
impl fmt::Display for SystemTimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "second time provided was later than self")
    }
}

impl FromInner<time::SystemTime> for SystemTime {
    fn from_inner(time: time::SystemTime) -> SystemTime {
        SystemTime(time)
    }
}

impl IntoInner<time::SystemTime> for SystemTime {
    fn into_inner(self) -> time::SystemTime {
        self.0
    }
}
