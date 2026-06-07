#![stable(feature = "duration_core", since = "1.25.0")]

//! 时间量度。
//!
//! # 示例
//!
//! 可以用多种方式创建新的 [`Duration`]：
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

use crate::fmt;
use crate::iter::Sum;
use crate::num::niche_types::Nanoseconds;
use crate::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Sub, SubAssign};

const NANOS_PER_SEC: u32 = 1_000_000_000;
const NANOS_PER_MILLI: u32 = 1_000_000;
const NANOS_PER_MICRO: u32 = 1_000;
const MILLIS_PER_SEC: u64 = 1_000;
const MICROS_PER_SEC: u64 = 1_000_000;
#[unstable(feature = "duration_units", issue = "120301")]
const SECS_PER_MINUTE: u64 = 60;
#[unstable(feature = "duration_units", issue = "120301")]
const MINS_PER_HOUR: u64 = 60;
#[unstable(feature = "duration_units", issue = "120301")]
const HOURS_PER_DAY: u64 = 24;
#[unstable(feature = "duration_units", issue = "120301")]
const DAYS_PER_WEEK: u64 = 7;

/// 表示一段时间长度的 `Duration` 类型，通常用于系统超时等需要表达
/// “经过了多少时间”的场景。
///
/// 每个 `Duration` 都由完整秒数和以纳秒表示的秒内小数部分组成。秒内小数部分会
/// 被规范化到 `0 <= nanos < NANOS_PER_SEC`，因此同一个时长只有一种内部表示。
/// 如果底层系统不支持纳秒级精度，绑定系统超时的 API 通常会把纳秒数向上取整；
/// 这样做是为了避免请求的等待时间被截短。
///
/// [`Duration`] 实现了许多常用 trait，包括 [`Add`]、[`Sub`] 以及其他 [`ops`] trait。
/// 它的 [`Default`] 实现会返回长度为零的 `Duration`。
///
/// [`ops`]: crate::ops
///
/// # 示例
///
/// ```
/// use std::time::Duration;
///
/// let five_seconds = Duration::new(5, 0);
/// let five_seconds_and_five_nanos = five_seconds + Duration::new(0, 5);
///
/// assert_eq!(five_seconds_and_five_nanos.as_secs(), 5);
/// assert_eq!(five_seconds_and_five_nanos.subsec_nanos(), 5);
///
/// let ten_millis = Duration::from_millis(10);
/// ```
///
/// # 格式化 `Duration` 值
///
/// `Duration` 有意不实现 `Display`，因为面向人的时长格式存在许多合理选择：
/// 可能需要本地化、固定单位、压缩显示，或者保留完整精度。`Duration` 提供的
/// `Debug` 实现会显示该值的完整精度，适合调试和精确检查。
///
/// `Debug` 输出会用非 ASCII 的 "µs" 后缀表示微秒。如果程序输出可能出现在无法
/// 可靠支持完整 Unicode 的环境中，应当自行格式化 `Duration`，或使用专门的 crate。
#[stable(feature = "duration", since = "1.3.0")]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[rustc_diagnostic_item = "Duration"]
pub struct Duration {
    secs: u64,
    nanos: Nanoseconds, // 始终满足 0 <= nanos < NANOS_PER_SEC
}

impl Duration {
    /// 一秒的时长。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(duration_constants)]
    /// use std::time::Duration;
    ///
    /// assert_eq!(Duration::SECOND, Duration::from_secs(1));
    /// ```
    #[unstable(feature = "duration_constants", issue = "57391")]
    pub const SECOND: Duration = Duration::from_secs(1);

    /// 一毫秒的时长。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(duration_constants)]
    /// use std::time::Duration;
    ///
    /// assert_eq!(Duration::MILLISECOND, Duration::from_millis(1));
    /// ```
    #[unstable(feature = "duration_constants", issue = "57391")]
    pub const MILLISECOND: Duration = Duration::from_millis(1);

    /// 一微秒的时长。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(duration_constants)]
    /// use std::time::Duration;
    ///
    /// assert_eq!(Duration::MICROSECOND, Duration::from_micros(1));
    /// ```
    #[unstable(feature = "duration_constants", issue = "57391")]
    pub const MICROSECOND: Duration = Duration::from_micros(1);

    /// 一纳秒的时长。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(duration_constants)]
    /// use std::time::Duration;
    ///
    /// assert_eq!(Duration::NANOSECOND, Duration::from_nanos(1));
    /// ```
    #[unstable(feature = "duration_constants", issue = "57391")]
    pub const NANOSECOND: Duration = Duration::from_nanos(1);

    /// 零时长。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::time::Duration;
    ///
    /// let duration = Duration::ZERO;
    /// assert!(duration.is_zero());
    /// assert_eq!(duration.as_nanos(), 0);
    /// ```
    #[stable(feature = "duration_zero", since = "1.53.0")]
    pub const ZERO: Duration = Duration::from_nanos(0);

    /// 可表示的最大时长。
    ///
    /// 必要时可以随平台而变化，但必须能够容纳两个 [`Instant`] 实例之间或两个
    /// [`SystemTime`] 实例之间的差值。这个约束在实践中给出了约
    /// 584,942,417,355 年的上限，目前所有平台都使用这个值。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::time::Duration;
    ///
    /// assert_eq!(Duration::MAX, Duration::new(u64::MAX, 1_000_000_000 - 1));
    /// ```
    /// [`Instant`]: ../../std/time/struct.Instant.html
    /// [`SystemTime`]: ../../std/time/struct.SystemTime.html
    #[stable(feature = "duration_saturating_ops", since = "1.53.0")]
    pub const MAX: Duration = Duration::new(u64::MAX, NANOS_PER_SEC - 1);

    /// 根据指定的完整秒数和额外纳秒数创建新的 `Duration`。
    ///
    /// 如果纳秒数大于等于 10 亿，也就是一秒包含的纳秒数，多出的部分会进位到
    /// 秒数中。构造后的值仍会保持 `Duration` 的规范化表示。
    ///
    /// # Panics
    ///
    /// 如果纳秒进位导致秒计数溢出，此构造函数会 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::time::Duration;
    ///
    /// let five_seconds = Duration::new(5, 0);
    /// ```
    #[stable(feature = "duration", since = "1.3.0")]
    #[inline]
    #[must_use]
    #[rustc_const_stable(feature = "duration_consts_2", since = "1.58.0")]
    pub const fn new(secs: u64, nanos: u32) -> Duration {
        if nanos < NANOS_PER_SEC {
            // SAFETY: nanos < NANOS_PER_SEC，因此 nanos 位于 Nanoseconds 的有效范围内
            Duration { secs, nanos: unsafe { Nanoseconds::new_unchecked(nanos) } }
        } else {
            let secs = secs
                .checked_add((nanos / NANOS_PER_SEC) as u64)
                .expect("overflow in Duration::new");
            let nanos = nanos % NANOS_PER_SEC;
            // SAFETY: nanos % NANOS_PER_SEC < NANOS_PER_SEC，因此 nanos 位于有效范围内
            Duration { secs, nanos: unsafe { Nanoseconds::new_unchecked(nanos) } }
        }
    }

    /// 根据指定的完整秒数创建新的 `Duration`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::time::Duration;
    ///
    /// let duration = Duration::from_secs(5);
    ///
    /// assert_eq!(5, duration.as_secs());
    /// assert_eq!(0, duration.subsec_nanos());
    /// ```
    #[stable(feature = "duration", since = "1.3.0")]
    #[must_use]
    #[inline]
    #[rustc_const_stable(feature = "duration_consts", since = "1.32.0")]
    pub const fn from_secs(secs: u64) -> Duration {
        Duration { secs, nanos: Nanoseconds::ZERO }
    }

    /// 根据指定的毫秒数创建新的 `Duration`。
    ///
    /// 完整的 1000 毫秒会转换为秒，剩余的毫秒会转换为纳秒并存入秒内小数部分。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::time::Duration;
    ///
    /// let duration = Duration::from_millis(2_569);
    ///
    /// assert_eq!(2, duration.as_secs());
    /// assert_eq!(569_000_000, duration.subsec_nanos());
    /// ```
    #[stable(feature = "duration", since = "1.3.0")]
    #[must_use]
    #[inline]
    #[rustc_const_stable(feature = "duration_consts", since = "1.32.0")]
    pub const fn from_millis(millis: u64) -> Duration {
        let secs = millis / MILLIS_PER_SEC;
        let subsec_millis = (millis % MILLIS_PER_SEC) as u32;
        // SAFETY: (x % 1_000) * 1_000_000 < 1_000_000_000
        //         因为 x % 1_000 < 1_000
        let subsec_nanos = unsafe { Nanoseconds::new_unchecked(subsec_millis * NANOS_PER_MILLI) };

        Duration { secs, nanos: subsec_nanos }
    }

    /// 根据指定的微秒数创建新的 `Duration`。
    ///
    /// 完整的 1,000,000 微秒会转换为秒，剩余的微秒会转换为纳秒并存入秒内小数部分。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::time::Duration;
    ///
    /// let duration = Duration::from_micros(1_000_002);
    ///
    /// assert_eq!(1, duration.as_secs());
    /// assert_eq!(2_000, duration.subsec_nanos());
    /// ```
    #[stable(feature = "duration_from_micros", since = "1.27.0")]
    #[must_use]
    #[inline]
    #[rustc_const_stable(feature = "duration_consts", since = "1.32.0")]
    pub const fn from_micros(micros: u64) -> Duration {
        let secs = micros / MICROS_PER_SEC;
        let subsec_micros = (micros % MICROS_PER_SEC) as u32;
        // SAFETY: (x % 1_000_000) * 1_000 < 1_000_000_000
        //         因为 x % 1_000_000 < 1_000_000
        let subsec_nanos = unsafe { Nanoseconds::new_unchecked(subsec_micros * NANOS_PER_MICRO) };

        Duration { secs, nanos: subsec_nanos }
    }

    /// 根据指定的纳秒数创建新的 `Duration`。
    ///
    /// 注意：不要直接把 `as_nanos()` 的返回值传给这个函数后再截断成 `u64`。
    /// `as_nanos()` 返回 `u128`，并且可能返回无法装入 `u64` 的值；例如约 585 年的
    /// 时长已经超过 `u64` 纳秒能表示的范围。如果不能直接复制或克隆原 `Duration`，
    /// 应考虑使用 `Duration::new(d.as_secs(), d.subsec_nanos())` 这种写法保留秒数
    /// 和秒内纳秒部分。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::time::Duration;
    ///
    /// let duration = Duration::from_nanos(1_000_000_123);
    ///
    /// assert_eq!(1, duration.as_secs());
    /// assert_eq!(123, duration.subsec_nanos());
    /// ```
    #[stable(feature = "duration_extras", since = "1.27.0")]
    #[must_use]
    #[inline]
    #[rustc_const_stable(feature = "duration_consts", since = "1.32.0")]
    pub const fn from_nanos(nanos: u64) -> Duration {
        const NANOS_PER_SEC: u64 = self::NANOS_PER_SEC as u64;
        let secs = nanos / NANOS_PER_SEC;
        let subsec_nanos = (nanos % NANOS_PER_SEC) as u32;
        // SAFETY: x % 1_000_000_000 < 1_000_000_000
        let subsec_nanos = unsafe { Nanoseconds::new_unchecked(subsec_nanos) };

        Duration { secs, nanos: subsec_nanos }
    }

    /// 根据指定的 `u128` 纳秒数创建新的 `Duration`。
    ///
    /// 这个构造函数适用于已经以 `u128` 保存总纳秒数的场景。它会把总纳秒数拆分为
    /// 完整秒数和秒内纳秒部分，并在超过 [`Duration::MAX`] 时显式报错。
    ///
    /// # Panics
    ///
    /// 如果给定的纳秒数大于 [`Duration::MAX`]，则会 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::time::Duration;
    ///
    /// let nanos = 10_u128.pow(24) + 321;
    /// let duration = Duration::from_nanos_u128(nanos);
    ///
    /// assert_eq!(10_u64.pow(15), duration.as_secs());
    /// assert_eq!(321, duration.subsec_nanos());
    /// ```
    #[stable(feature = "duration_from_nanos_u128", since = "1.93.0")]
    #[rustc_const_stable(feature = "duration_from_nanos_u128", since = "1.93.0")]
    #[must_use]
    #[inline]
    #[track_caller]
    #[rustc_allow_const_fn_unstable(const_trait_impl, const_convert)] // 用于 `u64::try_from`
    pub const fn from_nanos_u128(nanos: u128) -> Duration {
        const NANOS_PER_SEC: u128 = self::NANOS_PER_SEC as u128;
        let Ok(secs) = u64::try_from(nanos / NANOS_PER_SEC) else {
            panic!("overflow in `Duration::from_nanos_u128`");
        };
        let subsec_nanos = (nanos % NANOS_PER_SEC) as u32;
        // SAFETY: x % 1_000_000_000 < 1_000_000_000；并且 u128 与 u32 均非负，
        // 因此 subsec_nanos >= 0
        let subsec_nanos = unsafe { Nanoseconds::new_unchecked(subsec_nanos) };

        Duration { secs: secs as u64, nanos: subsec_nanos }
    }

    /// 根据指定的周数创建新的 `Duration`。
    ///
    /// 一周按 7 天、一天按 24 小时计算；这里表达的是固定长度的时间间隔，
    /// 不涉及日历、时区或夏令时规则。
    ///
    /// # Panics
    ///
    /// 如果给定的周数会使 `Duration` 的秒数表示溢出，则会 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(duration_constructors)]
    /// use std::time::Duration;
    ///
    /// let duration = Duration::from_weeks(4);
    ///
    /// assert_eq!(4 * 7 * 24 * 60 * 60, duration.as_secs());
    /// assert_eq!(0, duration.subsec_nanos());
    /// ```
    #[unstable(feature = "duration_constructors", issue = "120301")]
    #[must_use]
    #[inline]
    pub const fn from_weeks(weeks: u64) -> Duration {
        if weeks > u64::MAX / (SECS_PER_MINUTE * MINS_PER_HOUR * HOURS_PER_DAY * DAYS_PER_WEEK) {
            panic!("overflow in Duration::from_weeks");
        }

        Duration::from_secs(weeks * MINS_PER_HOUR * SECS_PER_MINUTE * HOURS_PER_DAY * DAYS_PER_WEEK)
    }

    /// 根据指定的天数创建新的 `Duration`。
    ///
    /// 一天按固定的 24 小时计算；这里表达的是固定长度的时间间隔，
    /// 不涉及日历日期、时区或夏令时规则。
    ///
    /// # Panics
    ///
    /// 如果给定的天数会使 `Duration` 的秒数表示溢出，则会 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(duration_constructors)]
    /// use std::time::Duration;
    ///
    /// let duration = Duration::from_days(7);
    ///
    /// assert_eq!(7 * 24 * 60 * 60, duration.as_secs());
    /// assert_eq!(0, duration.subsec_nanos());
    /// ```
    #[unstable(feature = "duration_constructors", issue = "120301")]
    #[must_use]
    #[inline]
    pub const fn from_days(days: u64) -> Duration {
        if days > u64::MAX / (SECS_PER_MINUTE * MINS_PER_HOUR * HOURS_PER_DAY) {
            panic!("overflow in Duration::from_days");
        }

        Duration::from_secs(days * MINS_PER_HOUR * SECS_PER_MINUTE * HOURS_PER_DAY)
    }

    /// 根据指定的小时数创建新的 `Duration`。
    ///
    /// 一小时按固定的 60 分钟计算。
    ///
    /// # Panics
    ///
    /// 如果给定的小时数会使 `Duration` 的秒数表示溢出，则会 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::time::Duration;
    ///
    /// let duration = Duration::from_hours(6);
    ///
    /// assert_eq!(6 * 60 * 60, duration.as_secs());
    /// assert_eq!(0, duration.subsec_nanos());
    /// ```
    #[stable(feature = "duration_constructors_lite", since = "1.91.0")]
    #[rustc_const_stable(feature = "duration_constructors_lite", since = "1.91.0")]
    #[must_use]
    #[inline]
    pub const fn from_hours(hours: u64) -> Duration {
        if hours > u64::MAX / (SECS_PER_MINUTE * MINS_PER_HOUR) {
            panic!("overflow in Duration::from_hours");
        }

        Duration::from_secs(hours * MINS_PER_HOUR * SECS_PER_MINUTE)
    }

    /// 根据指定的分钟数创建新的 `Duration`。
    ///
    /// 一分钟按固定的 60 秒计算。
    ///
    /// # Panics
    ///
    /// 如果给定的分钟数会使 `Duration` 的秒数表示溢出，则会 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::time::Duration;
    ///
    /// let duration = Duration::from_mins(10);
    ///
    /// assert_eq!(10 * 60, duration.as_secs());
    /// assert_eq!(0, duration.subsec_nanos());
    /// ```
    #[stable(feature = "duration_constructors_lite", since = "1.91.0")]
    #[rustc_const_stable(feature = "duration_constructors_lite", since = "1.91.0")]
    #[must_use]
    #[inline]
    pub const fn from_mins(mins: u64) -> Duration {
        if mins > u64::MAX / SECS_PER_MINUTE {
            panic!("overflow in Duration::from_mins");
        }

        Duration::from_secs(mins * SECS_PER_MINUTE)
    }

    /// 如果这个 `Duration` 表示零时长，则返回 true。
    ///
    /// 只有秒数和秒内纳秒部分都为零时才是零时长。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::time::Duration;
    ///
    /// assert!(Duration::ZERO.is_zero());
    /// assert!(Duration::new(0, 0).is_zero());
    /// assert!(Duration::from_nanos(0).is_zero());
    /// assert!(Duration::from_secs(0).is_zero());
    ///
    /// assert!(!Duration::new(1, 1).is_zero());
    /// assert!(!Duration::from_nanos(1).is_zero());
    /// assert!(!Duration::from_secs(1).is_zero());
    /// ```
    #[must_use]
    #[stable(feature = "duration_zero", since = "1.53.0")]
    #[rustc_const_stable(feature = "duration_zero", since = "1.53.0")]
    #[inline]
    pub const fn is_zero(&self) -> bool {
        self.secs == 0 && self.nanos.as_inner() == 0
    }

    /// 返回这个 `Duration` 中包含的_完整_秒数。
    ///
    /// 返回值不包含秒内小数部分。秒内纳秒部分可通过 [`subsec_nanos`] 获取。
    /// 如果需要包含小数部分的总秒数，请使用浮点转换方法。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::time::Duration;
    ///
    /// let duration = Duration::new(5, 730_023_852);
    /// assert_eq!(duration.as_secs(), 5);
    /// ```
    ///
    /// 若要取得包含小数部分的总秒数，请使用 [`as_secs_f64`] 或 [`as_secs_f32`]。
    ///
    /// [`as_secs_f64`]: Duration::as_secs_f64
    /// [`as_secs_f32`]: Duration::as_secs_f32
    /// [`subsec_nanos`]: Duration::subsec_nanos
    #[stable(feature = "duration", since = "1.3.0")]
    #[rustc_const_stable(feature = "duration_consts", since = "1.32.0")]
    #[must_use]
    #[inline]
    pub const fn as_secs(&self) -> u64 {
        self.secs
    }

    /// 以完整毫秒返回这个 `Duration` 的秒内小数部分。
    ///
    /// 这个方法**不会**返回整个时长按毫秒表示后的长度。返回值始终只表示一秒内的
    /// 小数部分，因此一定小于 1000。要获取总毫秒数，请使用 [`as_millis`]。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::time::Duration;
    ///
    /// let duration = Duration::from_millis(5_432);
    /// assert_eq!(duration.as_secs(), 5);
    /// assert_eq!(duration.subsec_millis(), 432);
    /// ```
    #[stable(feature = "duration_extras", since = "1.27.0")]
    #[rustc_const_stable(feature = "duration_consts", since = "1.32.0")]
    #[must_use]
    #[inline]
    pub const fn subsec_millis(&self) -> u32 {
        self.nanos.as_inner() / NANOS_PER_MILLI
    }

    /// 以完整微秒返回这个 `Duration` 的秒内小数部分。
    ///
    /// 这个方法**不会**返回整个时长按微秒表示后的长度。返回值始终只表示一秒内的
    /// 小数部分，因此一定小于 1,000,000。要获取总微秒数，请使用 [`as_micros`]。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::time::Duration;
    ///
    /// let duration = Duration::from_micros(1_234_567);
    /// assert_eq!(duration.as_secs(), 1);
    /// assert_eq!(duration.subsec_micros(), 234_567);
    /// ```
    #[stable(feature = "duration_extras", since = "1.27.0")]
    #[rustc_const_stable(feature = "duration_consts", since = "1.32.0")]
    #[must_use]
    #[inline]
    pub const fn subsec_micros(&self) -> u32 {
        self.nanos.as_inner() / NANOS_PER_MICRO
    }

    /// 以纳秒返回这个 `Duration` 的秒内小数部分。
    ///
    /// 这个方法**不会**返回整个时长按纳秒表示后的长度。返回值始终只表示一秒内的
    /// 小数部分，因此一定小于 1,000,000,000。要获取总纳秒数，请使用 [`as_nanos`]。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::time::Duration;
    ///
    /// let duration = Duration::from_millis(5_010);
    /// assert_eq!(duration.as_secs(), 5);
    /// assert_eq!(duration.subsec_nanos(), 10_000_000);
    /// ```
    #[stable(feature = "duration", since = "1.3.0")]
    #[rustc_const_stable(feature = "duration_consts", since = "1.32.0")]
    #[must_use]
    #[inline]
    pub const fn subsec_nanos(&self) -> u32 {
        self.nanos.as_inner()
    }

    /// 返回这个 `Duration` 中包含的完整毫秒总数。
    ///
    /// 秒内不足一毫秒的纳秒部分会被向零截断；返回类型为 `u128`，以容纳
    /// [`Duration::MAX`] 级别的总量。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::time::Duration;
    ///
    /// let duration = Duration::new(5, 730_023_852);
    /// assert_eq!(duration.as_millis(), 5_730);
    /// ```
    #[stable(feature = "duration_as_u128", since = "1.33.0")]
    #[rustc_const_stable(feature = "duration_as_u128", since = "1.33.0")]
    #[must_use]
    #[inline]
    pub const fn as_millis(&self) -> u128 {
        self.secs as u128 * MILLIS_PER_SEC as u128
            + (self.nanos.as_inner() / NANOS_PER_MILLI) as u128
    }

    /// 返回这个 `Duration` 中包含的完整微秒总数。
    ///
    /// 秒内不足一微秒的纳秒部分会被向零截断；返回类型为 `u128`，以容纳
    /// [`Duration::MAX`] 级别的总量。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::time::Duration;
    ///
    /// let duration = Duration::new(5, 730_023_852);
    /// assert_eq!(duration.as_micros(), 5_730_023);
    /// ```
    #[stable(feature = "duration_as_u128", since = "1.33.0")]
    #[rustc_const_stable(feature = "duration_as_u128", since = "1.33.0")]
    #[must_use]
    #[inline]
    pub const fn as_micros(&self) -> u128 {
        self.secs as u128 * MICROS_PER_SEC as u128
            + (self.nanos.as_inner() / NANOS_PER_MICRO) as u128
    }

    /// 返回这个 `Duration` 中包含的纳秒总数。
    ///
    /// 返回类型为 `u128`，因为最大 `Duration` 的总纳秒数无法装入 `u64`。
    /// 如果需要重新构造 `Duration`，应避免把该值截断成 `u64`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::time::Duration;
    ///
    /// let duration = Duration::new(5, 730_023_852);
    /// assert_eq!(duration.as_nanos(), 5_730_023_852);
    /// ```
    #[stable(feature = "duration_as_u128", since = "1.33.0")]
    #[rustc_const_stable(feature = "duration_as_u128", since = "1.33.0")]
    #[must_use]
    #[inline]
    pub const fn as_nanos(&self) -> u128 {
        self.secs as u128 * NANOS_PER_SEC as u128 + self.nanos.as_inner() as u128
    }

    /// 计算 `self` 与 `other` 之间的绝对差值。
    ///
    /// 返回值始终非负，因此无论两个 `Duration` 的先后顺序如何，都能得到可表示的
    /// 时间间隔。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::time::Duration;
    ///
    /// assert_eq!(Duration::new(100, 0).abs_diff(Duration::new(80, 0)), Duration::new(20, 0));
    /// assert_eq!(Duration::new(100, 400_000_000).abs_diff(Duration::new(110, 0)), Duration::new(9, 600_000_000));
    /// ```
    #[stable(feature = "duration_abs_diff", since = "1.81.0")]
    #[rustc_const_stable(feature = "duration_abs_diff", since = "1.81.0")]
    #[must_use = "this returns the result of the operation, \
                  without modifying the original"]
    #[inline]
    pub const fn abs_diff(self, other: Duration) -> Duration {
        if let Some(res) = self.checked_sub(other) { res } else { other.checked_sub(self).unwrap() }
    }

    /// checked 版 `Duration` 加法。
    ///
    /// 计算 `self + other`。如果秒数相加、纳秒进位或最终规范化过程发生溢出，
    /// 则返回 [`None`]；否则返回规范化后的 `Duration`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::time::Duration;
    ///
    /// assert_eq!(Duration::new(0, 0).checked_add(Duration::new(0, 1)), Some(Duration::new(0, 1)));
    /// assert_eq!(Duration::new(1, 0).checked_add(Duration::new(u64::MAX, 0)), None);
    /// ```
    #[stable(feature = "duration_checked_ops", since = "1.16.0")]
    #[must_use = "this returns the result of the operation, \
                  without modifying the original"]
    #[inline]
    #[rustc_const_stable(feature = "duration_consts_2", since = "1.58.0")]
    pub const fn checked_add(self, rhs: Duration) -> Option<Duration> {
        if let Some(mut secs) = self.secs.checked_add(rhs.secs) {
            let mut nanos = self.nanos.as_inner() + rhs.nanos.as_inner();
            if nanos >= NANOS_PER_SEC {
                nanos -= NANOS_PER_SEC;
                let Some(new_secs) = secs.checked_add(1) else {
                    return None;
                };
                secs = new_secs;
            }
            debug_assert!(nanos < NANOS_PER_SEC);
            Some(Duration::new(secs, nanos))
        } else {
            None
        }
    }

    /// saturating 版 `Duration` 加法。
    ///
    /// 计算 `self + other`。如果运算溢出，则返回 [`Duration::MAX`]，而不是 panic
    /// 或返回 [`None`]。这适合希望把超过上限的结果钳制到最大可表示时长的场景。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(duration_constants)]
    /// use std::time::Duration;
    ///
    /// assert_eq!(Duration::new(0, 0).saturating_add(Duration::new(0, 1)), Duration::new(0, 1));
    /// assert_eq!(Duration::new(1, 0).saturating_add(Duration::new(u64::MAX, 0)), Duration::MAX);
    /// ```
    #[stable(feature = "duration_saturating_ops", since = "1.53.0")]
    #[must_use = "this returns the result of the operation, \
                  without modifying the original"]
    #[inline]
    #[rustc_const_stable(feature = "duration_consts_2", since = "1.58.0")]
    pub const fn saturating_add(self, rhs: Duration) -> Duration {
        match self.checked_add(rhs) {
            Some(res) => res,
            None => Duration::MAX,
        }
    }

    /// checked 版 `Duration` 减法。
    ///
    /// 计算 `self - other`。如果结果为负，或借位、规范化过程中发生溢出，
    /// 则返回 [`None`]；否则返回规范化后的 `Duration`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::time::Duration;
    ///
    /// assert_eq!(Duration::new(0, 1).checked_sub(Duration::new(0, 0)), Some(Duration::new(0, 1)));
    /// assert_eq!(Duration::new(0, 0).checked_sub(Duration::new(0, 1)), None);
    /// ```
    #[stable(feature = "duration_checked_ops", since = "1.16.0")]
    #[must_use = "this returns the result of the operation, \
                  without modifying the original"]
    #[inline]
    #[rustc_const_stable(feature = "duration_consts_2", since = "1.58.0")]
    pub const fn checked_sub(self, rhs: Duration) -> Option<Duration> {
        if let Some(mut secs) = self.secs.checked_sub(rhs.secs) {
            let nanos = if self.nanos.as_inner() >= rhs.nanos.as_inner() {
                self.nanos.as_inner() - rhs.nanos.as_inner()
            } else if let Some(sub_secs) = secs.checked_sub(1) {
                secs = sub_secs;
                self.nanos.as_inner() + NANOS_PER_SEC - rhs.nanos.as_inner()
            } else {
                return None;
            };
            debug_assert!(nanos < NANOS_PER_SEC);
            Some(Duration::new(secs, nanos))
        } else {
            None
        }
    }

    /// saturating 版 `Duration` 减法。
    ///
    /// 计算 `self - other`。如果结果为负，或运算无法表示，则返回 [`Duration::ZERO`]。
    /// 这会把低于零的结果钳制为零时长。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::time::Duration;
    ///
    /// assert_eq!(Duration::new(0, 1).saturating_sub(Duration::new(0, 0)), Duration::new(0, 1));
    /// assert_eq!(Duration::new(0, 0).saturating_sub(Duration::new(0, 1)), Duration::ZERO);
    /// ```
    #[stable(feature = "duration_saturating_ops", since = "1.53.0")]
    #[must_use = "this returns the result of the operation, \
                  without modifying the original"]
    #[inline]
    #[rustc_const_stable(feature = "duration_consts_2", since = "1.58.0")]
    pub const fn saturating_sub(self, rhs: Duration) -> Duration {
        match self.checked_sub(rhs) {
            Some(res) => res,
            None => Duration::ZERO,
        }
    }

    /// checked 版 `Duration` 乘法。
    ///
    /// 计算 `self * other`。纳秒部分会先以 `u64` 计算并拆出额外秒数，再与秒数部分
    /// 合并；如果任一步骤溢出，则返回 [`None`]。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::time::Duration;
    ///
    /// assert_eq!(Duration::new(0, 500_000_001).checked_mul(2), Some(Duration::new(1, 2)));
    /// assert_eq!(Duration::new(u64::MAX - 1, 0).checked_mul(2), None);
    /// ```
    #[stable(feature = "duration_checked_ops", since = "1.16.0")]
    #[must_use = "this returns the result of the operation, \
                  without modifying the original"]
    #[inline]
    #[rustc_const_stable(feature = "duration_consts_2", since = "1.58.0")]
    pub const fn checked_mul(self, rhs: u32) -> Option<Duration> {
        // 以 u64 计算纳秒乘积；在该宽度下，u32 纳秒部分乘以 u32 标量不会溢出。
        let total_nanos = self.nanos.as_inner() as u64 * rhs as u64;
        let extra_secs = total_nanos / (NANOS_PER_SEC as u64);
        let nanos = (total_nanos % (NANOS_PER_SEC as u64)) as u32;
        // FIXME(const-hack): 一旦 const 中可行，就改用 `and_then`。
        if let Some(s) = self.secs.checked_mul(rhs as u64) {
            if let Some(secs) = s.checked_add(extra_secs) {
                debug_assert!(nanos < NANOS_PER_SEC);
                return Some(Duration::new(secs, nanos));
            }
        }
        None
    }

    /// saturating 版 `Duration` 乘法。
    ///
    /// 计算 `self * other`。如果结果溢出，则返回 [`Duration::MAX`]，将结果钳制到
    /// 最大可表示时长。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(duration_constants)]
    /// use std::time::Duration;
    ///
    /// assert_eq!(Duration::new(0, 500_000_001).saturating_mul(2), Duration::new(1, 2));
    /// assert_eq!(Duration::new(u64::MAX - 1, 0).saturating_mul(2), Duration::MAX);
    /// ```
    #[stable(feature = "duration_saturating_ops", since = "1.53.0")]
    #[must_use = "this returns the result of the operation, \
                  without modifying the original"]
    #[inline]
    #[rustc_const_stable(feature = "duration_consts_2", since = "1.58.0")]
    pub const fn saturating_mul(self, rhs: u32) -> Duration {
        match self.checked_mul(rhs) {
            Some(res) => res,
            None => Duration::MAX,
        }
    }

    /// checked 版 `Duration` 除法。
    ///
    /// 计算 `self / other`。如果 `other == 0`，则返回 [`None`]；否则按整数除法得到
    /// 规范化后的 `Duration`，不能表示的更小纳秒余数会被向零截断。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::time::Duration;
    ///
    /// assert_eq!(Duration::new(2, 0).checked_div(2), Some(Duration::new(1, 0)));
    /// assert_eq!(Duration::new(1, 0).checked_div(2), Some(Duration::new(0, 500_000_000)));
    /// assert_eq!(Duration::new(2, 0).checked_div(0), None);
    /// ```
    #[stable(feature = "duration_checked_ops", since = "1.16.0")]
    #[must_use = "this returns the result of the operation, \
                  without modifying the original"]
    #[inline]
    #[rustc_const_stable(feature = "duration_consts_2", since = "1.58.0")]
    pub const fn checked_div(self, rhs: u32) -> Option<Duration> {
        if rhs != 0 {
            let (secs, extra_secs) = (self.secs / (rhs as u64), self.secs % (rhs as u64));
            let (mut nanos, extra_nanos) =
                (self.nanos.as_inner() / rhs, self.nanos.as_inner() % rhs);
            nanos +=
                ((extra_secs * (NANOS_PER_SEC as u64) + extra_nanos as u64) / (rhs as u64)) as u32;
            debug_assert!(nanos < NANOS_PER_SEC);
            Some(Duration::new(secs, nanos))
        } else {
            None
        }
    }

    /// 以 `f64` 返回这个 `Duration` 中包含的秒数。
    ///
    /// 返回值包含秒内纳秒小数部分。由于 `f64` 精度有限，较大的 `Duration` 可能无法
    /// 精确表示每一个纳秒。
    ///
    /// # 示例
    /// ```
    /// use std::time::Duration;
    ///
    /// let dur = Duration::new(2, 700_000_000);
    /// assert_eq!(dur.as_secs_f64(), 2.7);
    /// ```
    #[stable(feature = "duration_float", since = "1.38.0")]
    #[must_use]
    #[inline]
    #[rustc_const_stable(feature = "duration_consts_float", since = "1.83.0")]
    pub const fn as_secs_f64(&self) -> f64 {
        (self.secs as f64) + (self.nanos.as_inner() as f64) / (NANOS_PER_SEC as f64)
    }

    /// 以 `f32` 返回这个 `Duration` 中包含的秒数。
    ///
    /// 返回值包含秒内纳秒小数部分。`f32` 的有效精度比 `f64` 更低，因此更容易丢失
    /// 纳秒级细节或在较大数值上出现舍入。
    ///
    /// # 示例
    /// ```
    /// use std::time::Duration;
    ///
    /// let dur = Duration::new(2, 700_000_000);
    /// assert_eq!(dur.as_secs_f32(), 2.7);
    /// ```
    #[stable(feature = "duration_float", since = "1.38.0")]
    #[must_use]
    #[inline]
    #[rustc_const_stable(feature = "duration_consts_float", since = "1.83.0")]
    pub const fn as_secs_f32(&self) -> f32 {
        (self.secs as f32) + (self.nanos.as_inner() as f32) / (NANOS_PER_SEC as f32)
    }

    /// 以 `f64` 返回这个 `Duration` 中包含的毫秒数。
    ///
    /// 返回值包含由纳秒换算出的毫秒小数部分。由于 `f64` 精度有限，结果可能经过
    /// 浮点舍入。
    ///
    /// # 示例
    /// ```
    /// #![feature(duration_millis_float)]
    /// use std::time::Duration;
    ///
    /// let dur = Duration::new(2, 345_678_000);
    /// assert_eq!(dur.as_millis_f64(), 2_345.678);
    /// ```
    #[unstable(feature = "duration_millis_float", issue = "122451")]
    #[must_use]
    #[inline]
    pub const fn as_millis_f64(&self) -> f64 {
        (self.secs as f64) * (MILLIS_PER_SEC as f64)
            + (self.nanos.as_inner() as f64) / (NANOS_PER_MILLI as f64)
    }

    /// 以 `f32` 返回这个 `Duration` 中包含的毫秒数。
    ///
    /// 返回值包含由纳秒换算出的毫秒小数部分。`f32` 精度有限，结果可能经过浮点
    /// 舍入，并且比 `f64` 更容易丢失细节。
    ///
    /// # 示例
    /// ```
    /// #![feature(duration_millis_float)]
    /// use std::time::Duration;
    ///
    /// let dur = Duration::new(2, 345_678_000);
    /// assert_eq!(dur.as_millis_f32(), 2_345.678);
    /// ```
    #[unstable(feature = "duration_millis_float", issue = "122451")]
    #[must_use]
    #[inline]
    pub const fn as_millis_f32(&self) -> f32 {
        (self.secs as f32) * (MILLIS_PER_SEC as f32)
            + (self.nanos.as_inner() as f32) / (NANOS_PER_MILLI as f32)
    }

    /// 根据以 `f64` 表示的秒数创建新的 `Duration`。
    ///
    /// 转换会把浮点秒数舍入到最接近的纳秒，并使用 IEEE-754 的 ties-to-even 规则
    /// 处理正好位于两个纳秒之间的值。由于浮点数并不能精确表示所有十进制小数，
    /// 调用者应预期结果可能包含浮点舍入带来的差异。
    ///
    /// # Panics
    /// 如果 `secs` 为负、不是有限值，或转换结果溢出 `Duration`，此构造函数会 panic。
    ///
    /// # 示例
    /// ```
    /// use std::time::Duration;
    ///
    /// let res = Duration::from_secs_f64(0.0);
    /// assert_eq!(res, Duration::new(0, 0));
    /// let res = Duration::from_secs_f64(1e-20);
    /// assert_eq!(res, Duration::new(0, 0));
    /// let res = Duration::from_secs_f64(4.2e-7);
    /// assert_eq!(res, Duration::new(0, 420));
    /// let res = Duration::from_secs_f64(2.7);
    /// assert_eq!(res, Duration::new(2, 700_000_000));
    /// let res = Duration::from_secs_f64(3e10);
    /// assert_eq!(res, Duration::new(30_000_000_000, 0));
    /// // subnormal float
    /// let res = Duration::from_secs_f64(f64::from_bits(1));
    /// assert_eq!(res, Duration::new(0, 0));
    /// // conversion uses rounding
    /// let res = Duration::from_secs_f64(0.999e-9);
    /// assert_eq!(res, Duration::new(0, 1));
    /// ```
    #[stable(feature = "duration_float", since = "1.38.0")]
    #[must_use]
    #[inline]
    pub fn from_secs_f64(secs: f64) -> Duration {
        match Duration::try_from_secs_f64(secs) {
            Ok(v) => v,
            Err(e) => panic!("{e}"),
        }
    }

    /// 根据以 `f32` 表示的秒数创建新的 `Duration`。
    ///
    /// 转换会把浮点秒数舍入到最接近的纳秒，并使用 IEEE-754 的 ties-to-even 规则
    /// 处理正好位于两个纳秒之间的值。`f32` 的精度低于 `f64`，因此示例中的结果可能
    /// 与直觉上的十进制计算略有差异。
    ///
    /// # Panics
    /// 如果 `secs` 为负、不是有限值，或转换结果溢出 `Duration`，此构造函数会 panic。
    ///
    /// # 示例
    /// ```
    /// use std::time::Duration;
    ///
    /// let res = Duration::from_secs_f32(0.0);
    /// assert_eq!(res, Duration::new(0, 0));
    /// let res = Duration::from_secs_f32(1e-20);
    /// assert_eq!(res, Duration::new(0, 0));
    /// let res = Duration::from_secs_f32(4.2e-7);
    /// assert_eq!(res, Duration::new(0, 420));
    /// let res = Duration::from_secs_f32(2.7);
    /// assert_eq!(res, Duration::new(2, 700_000_048));
    /// let res = Duration::from_secs_f32(3e10);
    /// assert_eq!(res, Duration::new(30_000_001_024, 0));
    /// // subnormal float
    /// let res = Duration::from_secs_f32(f32::from_bits(1));
    /// assert_eq!(res, Duration::new(0, 0));
    /// // conversion uses rounding
    /// let res = Duration::from_secs_f32(0.999e-9);
    /// assert_eq!(res, Duration::new(0, 1));
    /// ```
    #[stable(feature = "duration_float", since = "1.38.0")]
    #[must_use]
    #[inline]
    pub fn from_secs_f32(secs: f32) -> Duration {
        match Duration::try_from_secs_f32(secs) {
            Ok(v) => v,
            Err(e) => panic!("{e}"),
        }
    }

    /// 将 `Duration` 乘以 `f64`。
    ///
    /// 运算会先把时长转换为浮点秒数，乘以标量，再按 `from_secs_f64` 的规则舍入回
    /// `Duration`。因此结果受浮点精度和舍入规则影响。
    ///
    /// # Panics
    /// 如果结果为负、不是有限值，或溢出 `Duration`，此方法会 panic。
    ///
    /// # 示例
    /// ```
    /// use std::time::Duration;
    ///
    /// let dur = Duration::new(2, 700_000_000);
    /// assert_eq!(dur.mul_f64(3.14), Duration::new(8, 478_000_000));
    /// assert_eq!(dur.mul_f64(3.14e5), Duration::new(847_800, 0));
    /// ```
    #[stable(feature = "duration_float", since = "1.38.0")]
    #[must_use = "this returns the result of the operation, \
                  without modifying the original"]
    #[inline]
    pub fn mul_f64(self, rhs: f64) -> Duration {
        Duration::from_secs_f64(rhs * self.as_secs_f64())
    }

    /// 将 `Duration` 乘以 `f32`。
    ///
    /// 运算会先把时长转换为浮点秒数，乘以标量，再按 `from_secs_f32` 的规则舍入回
    /// `Duration`。`f32` 精度较低，结果可能更明显地受到舍入影响。
    ///
    /// # Panics
    /// 如果结果为负、不是有限值，或溢出 `Duration`，此方法会 panic。
    ///
    /// # 示例
    /// ```
    /// use std::time::Duration;
    ///
    /// let dur = Duration::new(2, 700_000_000);
    /// assert_eq!(dur.mul_f32(3.14), Duration::new(8, 478_000_641));
    /// assert_eq!(dur.mul_f32(3.14e5), Duration::new(847_800, 0));
    /// ```
    #[stable(feature = "duration_float", since = "1.38.0")]
    #[must_use = "this returns the result of the operation, \
                  without modifying the original"]
    #[inline]
    pub fn mul_f32(self, rhs: f32) -> Duration {
        Duration::from_secs_f32(rhs * self.as_secs_f32())
    }

    /// 将 `Duration` 除以 `f64`。
    ///
    /// 运算会先把时长转换为浮点秒数，除以标量，再按 `from_secs_f64` 的规则舍入回
    /// `Duration`。结果受浮点精度和舍入规则影响。
    ///
    /// # Panics
    /// 如果结果为负、不是有限值，或溢出 `Duration`，此方法会 panic。
    ///
    /// # 示例
    /// ```
    /// use std::time::Duration;
    ///
    /// let dur = Duration::new(2, 700_000_000);
    /// assert_eq!(dur.div_f64(3.14), Duration::new(0, 859_872_611));
    /// assert_eq!(dur.div_f64(3.14e5), Duration::new(0, 8_599));
    /// ```
    #[stable(feature = "duration_float", since = "1.38.0")]
    #[must_use = "this returns the result of the operation, \
                  without modifying the original"]
    #[inline]
    pub fn div_f64(self, rhs: f64) -> Duration {
        Duration::from_secs_f64(self.as_secs_f64() / rhs)
    }

    /// 将 `Duration` 除以 `f32`。
    ///
    /// 运算会先把时长转换为浮点秒数，除以标量，再按 `from_secs_f32` 的规则舍入回
    /// `Duration`。`f32` 精度较低，结果可能更明显地受到舍入影响。
    ///
    /// # Panics
    /// 如果结果为负、不是有限值，或溢出 `Duration`，此方法会 panic。
    ///
    /// # 示例
    /// ```
    /// use std::time::Duration;
    ///
    /// let dur = Duration::new(2, 700_000_000);
    /// // note that due to rounding errors result is slightly
    /// // different from 0.859_872_611
    /// assert_eq!(dur.div_f32(3.14), Duration::new(0, 859_872_580));
    /// assert_eq!(dur.div_f32(3.14e5), Duration::new(0, 8_599));
    /// ```
    #[stable(feature = "duration_float", since = "1.38.0")]
    #[must_use = "this returns the result of the operation, \
                  without modifying the original"]
    #[inline]
    pub fn div_f32(self, rhs: f32) -> Duration {
        Duration::from_secs_f32(self.as_secs_f32() / rhs)
    }

    /// 用一个 `Duration` 除以另一个 `Duration`，并返回 `f64` 比值。
    ///
    /// 返回值表示两个时长的比例，而不是新的时长。由于结果是浮点数，可能存在舍入。
    ///
    /// # 示例
    /// ```
    /// use std::time::Duration;
    ///
    /// let dur1 = Duration::new(2, 700_000_000);
    /// let dur2 = Duration::new(5, 400_000_000);
    /// assert_eq!(dur1.div_duration_f64(dur2), 0.5);
    /// ```
    #[stable(feature = "div_duration", since = "1.80.0")]
    #[must_use = "this returns the result of the operation, \
                  without modifying the original"]
    #[inline]
    #[rustc_const_stable(feature = "duration_consts_float", since = "1.83.0")]
    pub const fn div_duration_f64(self, rhs: Duration) -> f64 {
        let self_nanos =
            (self.secs as f64) * (NANOS_PER_SEC as f64) + (self.nanos.as_inner() as f64);
        let rhs_nanos = (rhs.secs as f64) * (NANOS_PER_SEC as f64) + (rhs.nanos.as_inner() as f64);
        self_nanos / rhs_nanos
    }

    /// 用一个 `Duration` 除以另一个 `Duration`，并返回 `f32` 比值。
    ///
    /// 返回值表示两个时长的比例，而不是新的时长。`f32` 精度有限，可能存在舍入。
    ///
    /// # 示例
    /// ```
    /// use std::time::Duration;
    ///
    /// let dur1 = Duration::new(2, 700_000_000);
    /// let dur2 = Duration::new(5, 400_000_000);
    /// assert_eq!(dur1.div_duration_f32(dur2), 0.5);
    /// ```
    #[stable(feature = "div_duration", since = "1.80.0")]
    #[must_use = "this returns the result of the operation, \
                  without modifying the original"]
    #[inline]
    #[rustc_const_stable(feature = "duration_consts_float", since = "1.83.0")]
    pub const fn div_duration_f32(self, rhs: Duration) -> f32 {
        let self_nanos =
            (self.secs as f32) * (NANOS_PER_SEC as f32) + (self.nanos.as_inner() as f32);
        let rhs_nanos = (rhs.secs as f32) * (NANOS_PER_SEC as f32) + (rhs.nanos.as_inner() as f32);
        self_nanos / rhs_nanos
    }

    /// 用一个 `Duration` 除以另一个 `Duration`，并返回向零舍入的 `u128` 商。
    ///
    /// 这个方法执行整数除法，只返回完整倍数；余数会被丢弃。
    ///
    /// # 示例
    /// ```
    /// #![feature(duration_integer_division)]
    /// use std::time::Duration;
    ///
    /// let dur = Duration::new(2, 0);
    /// assert_eq!(dur.div_duration_floor(Duration::new(1, 000_000_001)), 1);
    /// assert_eq!(dur.div_duration_floor(Duration::new(1, 000_000_000)), 2);
    /// assert_eq!(dur.div_duration_floor(Duration::new(0, 999_999_999)), 2);
    /// ```
    #[unstable(feature = "duration_integer_division", issue = "149573")]
    #[must_use = "this returns the result of the operation, \
                  without modifying the original"]
    #[inline]
    pub const fn div_duration_floor(self, rhs: Duration) -> u128 {
        self.as_nanos().div_floor(rhs.as_nanos())
    }

    /// 用一个 `Duration` 除以另一个 `Duration`，并返回向正无穷舍入的 `u128` 商。
    ///
    /// 只要存在非零余数，结果就会比向零舍入的商多一。
    ///
    /// # 示例
    /// ```
    /// #![feature(duration_integer_division)]
    /// use std::time::Duration;
    ///
    /// let dur = Duration::new(2, 0);
    /// assert_eq!(dur.div_duration_ceil(Duration::new(1, 000_000_001)), 2);
    /// assert_eq!(dur.div_duration_ceil(Duration::new(1, 000_000_000)), 2);
    /// assert_eq!(dur.div_duration_ceil(Duration::new(0, 999_999_999)), 3);
    /// ```
    #[unstable(feature = "duration_integer_division", issue = "149573")]
    #[must_use = "this returns the result of the operation, \
                  without modifying the original"]
    #[inline]
    pub const fn div_duration_ceil(self, rhs: Duration) -> u128 {
        self.as_nanos().div_ceil(rhs.as_nanos())
    }
}

#[stable(feature = "duration", since = "1.3.0")]
#[rustc_const_unstable(feature = "const_ops", issue = "143802")]
impl const Add for Duration {
    type Output = Duration;

    #[inline]
    fn add(self, rhs: Duration) -> Duration {
        self.checked_add(rhs).expect("overflow when adding durations")
    }
}

#[stable(feature = "time_augmented_assignment", since = "1.9.0")]
#[rustc_const_unstable(feature = "const_ops", issue = "143802")]
impl const AddAssign for Duration {
    #[inline]
    fn add_assign(&mut self, rhs: Duration) {
        *self = *self + rhs;
    }
}

#[stable(feature = "duration", since = "1.3.0")]
#[rustc_const_unstable(feature = "const_ops", issue = "143802")]
impl const Sub for Duration {
    type Output = Duration;

    #[inline]
    fn sub(self, rhs: Duration) -> Duration {
        self.checked_sub(rhs).expect("overflow when subtracting durations")
    }
}

#[stable(feature = "time_augmented_assignment", since = "1.9.0")]
#[rustc_const_unstable(feature = "const_ops", issue = "143802")]
impl const SubAssign for Duration {
    #[inline]
    fn sub_assign(&mut self, rhs: Duration) {
        *self = *self - rhs;
    }
}

#[stable(feature = "duration", since = "1.3.0")]
#[rustc_const_unstable(feature = "const_ops", issue = "143802")]
impl const Mul<u32> for Duration {
    type Output = Duration;

    #[inline]
    fn mul(self, rhs: u32) -> Duration {
        self.checked_mul(rhs).expect("overflow when multiplying duration by scalar")
    }
}

#[stable(feature = "symmetric_u32_duration_mul", since = "1.31.0")]
#[rustc_const_unstable(feature = "const_ops", issue = "143802")]
impl const Mul<Duration> for u32 {
    type Output = Duration;

    #[inline]
    fn mul(self, rhs: Duration) -> Duration {
        rhs * self
    }
}

#[stable(feature = "time_augmented_assignment", since = "1.9.0")]
#[rustc_const_unstable(feature = "const_ops", issue = "143802")]
impl const MulAssign<u32> for Duration {
    #[inline]
    fn mul_assign(&mut self, rhs: u32) {
        *self = *self * rhs;
    }
}

#[stable(feature = "duration", since = "1.3.0")]
#[rustc_const_unstable(feature = "const_ops", issue = "143802")]
impl const Div<u32> for Duration {
    type Output = Duration;

    #[inline]
    #[track_caller]
    fn div(self, rhs: u32) -> Duration {
        self.checked_div(rhs).expect("divide by zero error when dividing duration by scalar")
    }
}

#[stable(feature = "time_augmented_assignment", since = "1.9.0")]
#[rustc_const_unstable(feature = "const_ops", issue = "143802")]
impl const DivAssign<u32> for Duration {
    #[inline]
    #[track_caller]
    fn div_assign(&mut self, rhs: u32) {
        *self = *self / rhs;
    }
}

macro_rules! sum_durations {
    ($iter:expr) => {{
        let mut total_secs: u64 = 0;
        let mut total_nanos: u64 = 0;

        for entry in $iter {
            total_secs =
                total_secs.checked_add(entry.secs).expect("overflow in iter::sum over durations");
            total_nanos = match total_nanos.checked_add(entry.nanos.as_inner() as u64) {
                Some(n) => n,
                None => {
                    total_secs = total_secs
                        .checked_add(total_nanos / NANOS_PER_SEC as u64)
                        .expect("overflow in iter::sum over durations");
                    (total_nanos % NANOS_PER_SEC as u64) + entry.nanos.as_inner() as u64
                }
            };
        }
        total_secs = total_secs
            .checked_add(total_nanos / NANOS_PER_SEC as u64)
            .expect("overflow in iter::sum over durations");
        total_nanos = total_nanos % NANOS_PER_SEC as u64;
        Duration::new(total_secs, total_nanos as u32)
    }};
}

#[stable(feature = "duration_sum", since = "1.16.0")]
impl Sum for Duration {
    fn sum<I: Iterator<Item = Duration>>(iter: I) -> Duration {
        sum_durations!(iter)
    }
}

#[stable(feature = "duration_sum", since = "1.16.0")]
impl<'a> Sum<&'a Duration> for Duration {
    fn sum<I: Iterator<Item = &'a Duration>>(iter: I) -> Duration {
        sum_durations!(iter)
    }
}

#[stable(feature = "duration_debug_impl", since = "1.27.0")]
impl fmt::Debug for Duration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        /// 用十进制记法格式化带小数部分的数值。
        ///
        /// 输入数值被拆成 `integer_part` 和小数部分。小数部分的实际值为
        /// `fractional_part / divisor`；例如 `integer_part` = 3、
        /// `fractional_part` = 12 且 `divisor` = 100 表示数值 `3.012`。
        /// 尾随的零会被省略。
        ///
        /// `divisor` 不能大于 100_000_000，并且应当是 10 的幂；其他取值没有实际意义。
        /// `fractional_part` 必须小于 `10 * divisor`。
        ///
        /// 可以附加前缀和后缀。如果格式化器指定了 `width`，整体输出会被填充到该宽度。
        fn fmt_decimal(
            f: &mut fmt::Formatter<'_>,
            integer_part: u64,
            mut fractional_part: u32,
            mut divisor: u32,
            prefix: &str,
            postfix: &str,
        ) -> fmt::Result {
            // 将小数部分编码到临时缓冲区。`fractional_part` 必须小于 10^9，
            // 因此缓冲区只需要容纳 9 个数字。缓冲区预先填入字符 '0'，
            // 以简化下面的写入逻辑。
            let mut buf = [b'0'; 9];

            // 下一个数字会写入这个位置。
            let mut pos = 0;

            // 只要仍有非零数字，并且尚未写满请求的精度，就持续向缓冲区写入数字。
            while fractional_part > 0 && pos < f.precision().unwrap_or(9) {
                // 将新的数字写入缓冲区。
                buf[pos] = b'0' + (fractional_part / divisor) as u8;

                fractional_part %= divisor;
                divisor /= 10;
                pos += 1;
            }

            // 如果指定的 precision 小于 9，可能还存在没有写入缓冲区的非零数字。
            // 为了匹配普通浮点数打印语义，需要在这种情况下执行舍入。不过只有需要
            // 向上舍入时才要实际修改缓冲区：这发生在剩余数字的第一位 >= 5 时。
            // 当第一位正好是 5 时，舍入遵循 IEEE-754 round-ties-to-even 语义：
            // 只有最后一个已写入数字为奇数时才向上舍入。
            let integer_part = if fractional_part > 0 && fractional_part >= divisor * 5 {
                // 对平局值（fractional_part == divisor * 5），仅在最后一位为奇数时向上舍入。
                let is_tie = fractional_part == divisor * 5;
                let last_digit_is_odd = if pos > 0 {
                    (buf[pos - 1] - b'0') % 2 == 1
                } else {
                    // 没有小数数字时，检查整数部分。
                    (integer_part % 2) == 1
                };

                if is_tie && !last_digit_is_odd {
                    Some(integer_part)
                } else {
                    // 对缓冲区中的数字执行向上舍入。这里从后向前遍历缓冲区，
                    // 并跟踪进位。
                    let mut rev_pos = pos;
                    let mut carry = true;
                    while carry && rev_pos > 0 {
                        rev_pos -= 1;

                        // 如果缓冲区中的数字不是 '9'，只需递增它即可停止，
                        // 因为不再有进位。否则把它置为 '0'（该位溢出）并继续。
                        if buf[rev_pos] < b'9' {
                            buf[rev_pos] += 1;
                            carry = false;
                        } else {
                            buf[rev_pos] = b'0';
                        }
                    }

                    // 如果最后仍然存在进位，说明整个缓冲区都已经被置为 '0'，
                    // 需要把进位加到整数部分。
                    if carry {
                        // 如果 `integer_part == u64::MAX` 且 precision < 9，
                        // 小数部分舍入产生的任何进位都会使 `integer_part` 本身溢出。
                        // 这里用 `Option<u64>` 避免这种溢出，其中 `None` 表示
                        // `u64::MAX + 1`。
                        integer_part.checked_add(1)
                    } else {
                        Some(integer_part)
                    }
                }
            } else {
                Some(integer_part)
            };

            // 确定缓冲区的输出末尾：如果设置了 precision，就使用相应数量的数字
            // （最多 9 位）；如果没有设置，则只输出到最后一个非零数字为止。
            let end = f.precision().map(|p| crate::cmp::min(p, 9)).unwrap_or(pos);

            // 这个闭包负责写出未填充的格式化时长；填充宽度在下面单独计算。
            let emit_without_padding = |f: &mut fmt::Formatter<'_>| {
                if let Some(integer_part) = integer_part {
                    write!(f, "{}{}", prefix, integer_part)?;
                } else {
                    // u64::MAX + 1 == 18446744073709551616
                    write!(f, "{}18446744073709551616", prefix)?;
                }

                // 写入小数点以及小数部分（如果存在）。
                if end > 0 {
                    // SAFETY: 缓冲区只会写入 ASCII 数字，且初始化时已填充为 '0'，
                    // 因此其中包含有效的 UTF-8。
                    let s = unsafe { crate::str::from_utf8_unchecked(&buf[..end]) };

                    // 如果用户请求的 precision > 9，就在末尾补 '0'。
                    let w = f.precision().unwrap_or(pos);
                    write!(f, ".{:0<width$}", s, width = w)?;
                }

                write!(f, "{}", postfix)
            };

            match f.width() {
                None => {
                    // 没有指定 `width`。这种情况下不需要计算输出长度，直接写出即可。
                    emit_without_padding(f)
                }
                Some(requested_w) => {
                    // 指定了 `width`。先计算实际输出宽度，再据此计算需要的填充。
                    // 宽度由 4 个部分组成：
                    // 1. 前缀：只能是 "+" 或 ""，因此可直接使用 len()。
                    // 2. 后缀：可能是 "µs"，因此必须按 UTF-8 字符计数。
                    let mut actual_w = prefix.len() + postfix.chars().count();
                    // 3. 整数部分：
                    if let Some(integer_part) = integer_part {
                        if let Some(log) = integer_part.checked_ilog10() {
                            // integer_part > 0，因此长度为 log10(x)+1
                            actual_w += 1 + log as usize;
                        } else {
                            // integer_part 为 0，因此长度为 1。
                            actual_w += 1;
                        }
                    } else {
                        // integer_part 为 u64::MAX + 1，因此长度为 20。
                        actual_w += 20;
                    }
                    // 4. 小数部分（如果存在）：
                    if end > 0 {
                        let frac_part_w = f.precision().unwrap_or(pos);
                        actual_w += 1 + frac_part_w;
                    }

                    if requested_w <= actual_w {
                        // 输出已经长于 `width`，因此不进行填充。
                        emit_without_padding(f)
                    } else {
                        // 需要添加填充。使用 `Formatter::padding` 辅助函数完成。
                        let default_align = fmt::Alignment::Left;
                        let post_padding =
                            f.padding((requested_w - actual_w) as u16, default_align)?;
                        emit_without_padding(f)?;
                        post_padding.write(f)
                    }
                }
            }
        }

        // 如果请求了正号，则打印前导 '+'。
        let prefix = if f.sign_plus() { "+" } else { "" };

        if self.secs > 0 {
            fmt_decimal(f, self.secs, self.nanos.as_inner(), NANOS_PER_SEC / 10, prefix, "s")
        } else if self.nanos.as_inner() >= NANOS_PER_MILLI {
            fmt_decimal(
                f,
                (self.nanos.as_inner() / NANOS_PER_MILLI) as u64,
                self.nanos.as_inner() % NANOS_PER_MILLI,
                NANOS_PER_MILLI / 10,
                prefix,
                "ms",
            )
        } else if self.nanos.as_inner() >= NANOS_PER_MICRO {
            fmt_decimal(
                f,
                (self.nanos.as_inner() / NANOS_PER_MICRO) as u64,
                self.nanos.as_inner() % NANOS_PER_MICRO,
                NANOS_PER_MICRO / 10,
                prefix,
                "µs",
            )
        } else {
            fmt_decimal(f, self.nanos.as_inner() as u64, 0, 1, prefix, "ns")
        }
    }
}

/// 将以浮点数表示的秒数转换为 [`Duration`] 时可能返回的错误。
///
/// 此错误类型用于 [`Duration::try_from_secs_f32`] 和 [`Duration::try_from_secs_f64`]。
/// 它区分负数输入，以及过大或 `NaN` 等无法表示为 `Duration` 的输入。
///
/// # 示例
///
/// ```
/// use std::time::Duration;
///
/// if let Err(e) = Duration::try_from_secs_f32(-1.0) {
///     println!("Failed conversion to Duration: {e}");
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
#[stable(feature = "duration_checked_float", since = "1.66.0")]
pub struct TryFromFloatSecsError {
    kind: TryFromFloatSecsErrorKind,
}

#[stable(feature = "duration_checked_float", since = "1.66.0")]
impl fmt::Display for TryFromFloatSecsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            TryFromFloatSecsErrorKind::Negative => {
                "cannot convert float seconds to Duration: value is negative"
            }
            TryFromFloatSecsErrorKind::OverflowOrNan => {
                "cannot convert float seconds to Duration: value is either too big or NaN"
            }
        }
        .fmt(f)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TryFromFloatSecsErrorKind {
    // 输入值为负。
    Negative,
    // 输入值过大，无法表示为 `Duration`；或者输入值为 `NaN`。
    OverflowOrNan,
}

macro_rules! try_from_secs {
    (
        secs = $secs: expr,
        mantissa_bits = $mant_bits: literal,
        exponent_bits = $exp_bits: literal,
        offset = $offset: literal,
        bits_ty = $bits_ty:ty,
        double_ty = $double_ty:ty,
    ) => {{
        const MIN_EXP: i16 = 1 - (1i16 << $exp_bits) / 2;
        const MANT_MASK: $bits_ty = (1 << $mant_bits) - 1;
        const EXP_MASK: $bits_ty = (1 << $exp_bits) - 1;

        if $secs < 0.0 {
            return Err(TryFromFloatSecsError { kind: TryFromFloatSecsErrorKind::Negative });
        }

        let bits = $secs.to_bits();
        let mant = (bits & MANT_MASK) | (MANT_MASK + 1);
        let exp = ((bits >> $mant_bits) & EXP_MASK) as i16 + MIN_EXP;

        let (secs, nanos) = if exp < -31 {
            // 输入值小于 1ns，并且不足以舍入到 1ns。
            (0u64, 0u32)
        } else if exp < 0 {
            // 输入值小于 1 秒，需要全部转换到纳秒部分。
            let t = <$double_ty>::from(mant) << ($offset + exp);
            let nanos_offset = $mant_bits + $offset;
            let nanos_tmp = u128::from(NANOS_PER_SEC) * u128::from(t);
            let nanos = (nanos_tmp >> nanos_offset) as u32;

            let rem_mask = (1 << nanos_offset) - 1;
            let rem_msb_mask = 1 << (nanos_offset - 1);
            let rem = nanos_tmp & rem_mask;
            let is_tie = rem == rem_msb_mask;
            let is_even = (nanos & 1) == 0;
            let rem_msb = nanos_tmp & rem_msb_mask == 0;
            let add_ns = !(rem_msb || (is_even && is_tie));

            // f32 精度不足以触发第二个分支，因为它无法表示
            // 0.999_999_940_395 到 1.0 之间的数值。
            let nanos = nanos + add_ns as u32;
            if ($mant_bits == 23) || (nanos != NANOS_PER_SEC) { (0, nanos) } else { (1, 0) }
        } else if exp < $mant_bits {
            let secs = u64::from(mant >> ($mant_bits - exp));
            let t = <$double_ty>::from((mant << exp) & MANT_MASK);
            let nanos_offset = $mant_bits;
            let nanos_tmp = <$double_ty>::from(NANOS_PER_SEC) * t;
            let nanos = (nanos_tmp >> nanos_offset) as u32;

            let rem_mask = (1 << nanos_offset) - 1;
            let rem_msb_mask = 1 << (nanos_offset - 1);
            let rem = nanos_tmp & rem_mask;
            let is_tie = rem == rem_msb_mask;
            let is_even = (nanos & 1) == 0;
            let rem_msb = nanos_tmp & rem_msb_mask == 0;
            let add_ns = !(rem_msb || (is_even && is_tie));

            // f32 精度不足以触发第二个分支。例如，它无法表示
            // 1.999_999_880... 到 2.0 之间的数值；数值越大，小数部分精度越低。
            let nanos = nanos + add_ns as u32;
            if ($mant_bits == 23) || (nanos != NANOS_PER_SEC) {
                (secs, nanos)
            } else {
                (secs + 1, 0)
            }
        } else if exp < 64 {
            // 输入值没有小数部分。
            let secs = u64::from(mant) << (exp - $mant_bits);
            (secs, 0)
        } else {
            return Err(TryFromFloatSecsError { kind: TryFromFloatSecsErrorKind::OverflowOrNan });
        };

        Ok(Duration::new(secs, nanos))
    }};
}

impl Duration {
    /// [`from_secs_f32`] 的 checked 版本。
    ///
    /// [`from_secs_f32`]: Duration::from_secs_f32
    ///
    /// 如果 `secs` 为负、不是有限值，或转换结果溢出 `Duration`，此构造函数会返回
    /// `Err`。成功时会按与 [`from_secs_f32`] 相同的规则把浮点秒数舍入到最接近的
    /// 纳秒，并使用 IEEE-754 ties-to-even 规则处理平局值。
    ///
    /// # 示例
    /// ```
    /// use std::time::Duration;
    ///
    /// let res = Duration::try_from_secs_f32(0.0);
    /// assert_eq!(res, Ok(Duration::new(0, 0)));
    /// let res = Duration::try_from_secs_f32(1e-20);
    /// assert_eq!(res, Ok(Duration::new(0, 0)));
    /// let res = Duration::try_from_secs_f32(4.2e-7);
    /// assert_eq!(res, Ok(Duration::new(0, 420)));
    /// let res = Duration::try_from_secs_f32(2.7);
    /// assert_eq!(res, Ok(Duration::new(2, 700_000_048)));
    /// let res = Duration::try_from_secs_f32(3e10);
    /// assert_eq!(res, Ok(Duration::new(30_000_001_024, 0)));
    /// // subnormal float:
    /// let res = Duration::try_from_secs_f32(f32::from_bits(1));
    /// assert_eq!(res, Ok(Duration::new(0, 0)));
    ///
    /// let res = Duration::try_from_secs_f32(-5.0);
    /// assert!(res.is_err());
    /// let res = Duration::try_from_secs_f32(f32::NAN);
    /// assert!(res.is_err());
    /// let res = Duration::try_from_secs_f32(2e19);
    /// assert!(res.is_err());
    ///
    /// // the conversion uses rounding with tie resolution to even
    /// let res = Duration::try_from_secs_f32(0.999e-9);
    /// assert_eq!(res, Ok(Duration::new(0, 1)));
    ///
    /// // this float represents exactly 976562.5e-9
    /// let val = f32::from_bits(0x3A80_0000);
    /// let res = Duration::try_from_secs_f32(val);
    /// assert_eq!(res, Ok(Duration::new(0, 976_562)));
    ///
    /// // this float represents exactly 2929687.5e-9
    /// let val = f32::from_bits(0x3B40_0000);
    /// let res = Duration::try_from_secs_f32(val);
    /// assert_eq!(res, Ok(Duration::new(0, 2_929_688)));
    ///
    /// // this float represents exactly 1.000_976_562_5
    /// let val = f32::from_bits(0x3F802000);
    /// let res = Duration::try_from_secs_f32(val);
    /// assert_eq!(res, Ok(Duration::new(1, 976_562)));
    ///
    /// // this float represents exactly 1.002_929_687_5
    /// let val = f32::from_bits(0x3F806000);
    /// let res = Duration::try_from_secs_f32(val);
    /// assert_eq!(res, Ok(Duration::new(1, 2_929_688)));
    /// ```
    #[stable(feature = "duration_checked_float", since = "1.66.0")]
    #[inline]
    pub fn try_from_secs_f32(secs: f32) -> Result<Duration, TryFromFloatSecsError> {
        try_from_secs!(
            secs = secs,
            mantissa_bits = 23,
            exponent_bits = 8,
            offset = 41,
            bits_ty = u32,
            double_ty = u64,
        )
    }

    /// [`from_secs_f64`] 的 checked 版本。
    ///
    /// [`from_secs_f64`]: Duration::from_secs_f64
    ///
    /// 如果 `secs` 为负、不是有限值，或转换结果溢出 `Duration`，此构造函数会返回
    /// `Err`。成功时会按与 [`from_secs_f64`] 相同的规则把浮点秒数舍入到最接近的
    /// 纳秒，并使用 IEEE-754 ties-to-even 规则处理平局值。
    ///
    /// # 示例
    /// ```
    /// use std::time::Duration;
    ///
    /// let res = Duration::try_from_secs_f64(0.0);
    /// assert_eq!(res, Ok(Duration::new(0, 0)));
    /// let res = Duration::try_from_secs_f64(1e-20);
    /// assert_eq!(res, Ok(Duration::new(0, 0)));
    /// let res = Duration::try_from_secs_f64(4.2e-7);
    /// assert_eq!(res, Ok(Duration::new(0, 420)));
    /// let res = Duration::try_from_secs_f64(2.7);
    /// assert_eq!(res, Ok(Duration::new(2, 700_000_000)));
    /// let res = Duration::try_from_secs_f64(3e10);
    /// assert_eq!(res, Ok(Duration::new(30_000_000_000, 0)));
    /// // subnormal float
    /// let res = Duration::try_from_secs_f64(f64::from_bits(1));
    /// assert_eq!(res, Ok(Duration::new(0, 0)));
    ///
    /// let res = Duration::try_from_secs_f64(-5.0);
    /// assert!(res.is_err());
    /// let res = Duration::try_from_secs_f64(f64::NAN);
    /// assert!(res.is_err());
    /// let res = Duration::try_from_secs_f64(2e19);
    /// assert!(res.is_err());
    ///
    /// // the conversion uses rounding with tie resolution to even
    /// let res = Duration::try_from_secs_f64(0.999e-9);
    /// assert_eq!(res, Ok(Duration::new(0, 1)));
    /// let res = Duration::try_from_secs_f64(0.999_999_999_499);
    /// assert_eq!(res, Ok(Duration::new(0, 999_999_999)));
    /// let res = Duration::try_from_secs_f64(0.999_999_999_501);
    /// assert_eq!(res, Ok(Duration::new(1, 0)));
    /// let res = Duration::try_from_secs_f64(42.999_999_999_499);
    /// assert_eq!(res, Ok(Duration::new(42, 999_999_999)));
    /// let res = Duration::try_from_secs_f64(42.999_999_999_501);
    /// assert_eq!(res, Ok(Duration::new(43, 0)));
    ///
    /// // this float represents exactly 976562.5e-9
    /// let val = f64::from_bits(0x3F50_0000_0000_0000);
    /// let res = Duration::try_from_secs_f64(val);
    /// assert_eq!(res, Ok(Duration::new(0, 976_562)));
    ///
    /// // this float represents exactly 2929687.5e-9
    /// let val = f64::from_bits(0x3F68_0000_0000_0000);
    /// let res = Duration::try_from_secs_f64(val);
    /// assert_eq!(res, Ok(Duration::new(0, 2_929_688)));
    ///
    /// // this float represents exactly 1.000_976_562_5
    /// let val = f64::from_bits(0x3FF0_0400_0000_0000);
    /// let res = Duration::try_from_secs_f64(val);
    /// assert_eq!(res, Ok(Duration::new(1, 976_562)));
    ///
    /// // this float represents exactly 1.002_929_687_5
    /// let val = f64::from_bits(0x3_FF00_C000_0000_000);
    /// let res = Duration::try_from_secs_f64(val);
    /// assert_eq!(res, Ok(Duration::new(1, 2_929_688)));
    /// ```
    #[stable(feature = "duration_checked_float", since = "1.66.0")]
    #[inline]
    pub fn try_from_secs_f64(secs: f64) -> Result<Duration, TryFromFloatSecsError> {
        try_from_secs!(
            secs = secs,
            mantissa_bits = 52,
            exponent_bits = 11,
            offset = 44,
            bits_ty = u64,
            double_ty = u128,
        )
    }
}
