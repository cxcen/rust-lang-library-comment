use super::abi;
use super::error::expect_success;
use crate::mem::MaybeUninit;
use crate::time::Duration;

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub struct Instant(abi::SYSTIM);

impl Instant {
    pub fn now() -> Instant {
        // Safety: 提供的指针是有效的
        unsafe {
            let mut out = MaybeUninit::uninit();
            expect_success(abi::get_tim(out.as_mut_ptr()), &"get_tim");
            Instant(out.assume_init())
        }
    }

    pub fn checked_sub_instant(&self, other: &Instant) -> Option<Duration> {
        self.0.checked_sub(other.0).map(|ticks| {
            // `SYSTIM` 以微秒为单位
            Duration::from_micros(ticks)
        })
    }

    pub fn checked_add_duration(&self, other: &Duration) -> Option<Instant> {
        // `SYSTIM` 以微秒为单位
        let ticks = other.as_micros();

        Some(Instant(self.0.checked_add(ticks.try_into().ok()?)?))
    }

    pub fn checked_sub_duration(&self, other: &Duration) -> Option<Instant> {
        // `SYSTIM` 以微秒为单位
        let ticks = other.as_micros();

        Some(Instant(self.0.checked_sub(ticks.try_into().ok()?)?))
    }
}

/// 将 `Duration` 拆分为零个或多个 `RELTIM`。
#[inline]
pub fn dur2reltims(dur: Duration) -> impl Iterator<Item = abi::RELTIM> {
    // `RELTIM` 以微秒为单位
    let mut ticks = dur.as_micros();

    crate::iter::from_fn(move || {
        if ticks == 0 {
            None
        } else if ticks <= abi::TMAX_RELTIM as u128 {
            Some(crate::mem::replace(&mut ticks, 0) as abi::RELTIM)
        } else {
            ticks -= abi::TMAX_RELTIM as u128;
            Some(abi::TMAX_RELTIM)
        }
    })
}

/// 将 `Duration` 拆分为一个或多个 `TMO`。
#[inline]
fn dur2tmos(dur: Duration) -> impl Iterator<Item = abi::TMO> {
    // `TMO` 以微秒为单位
    let mut ticks = dur.as_micros();
    let mut end = false;

    crate::iter::from_fn(move || {
        if end {
            None
        } else if ticks <= abi::TMAX_RELTIM as u128 {
            end = true;
            Some(crate::mem::replace(&mut ticks, 0) as abi::TMO)
        } else {
            ticks -= abi::TMAX_RELTIM as u128;
            Some(abi::TMAX_RELTIM)
        }
    })
}

/// 将 `Duration` 拆分为一个或多个带超时的 API 调用。
#[inline]
pub fn with_tmos(dur: Duration, mut f: impl FnMut(abi::TMO) -> abi::ER) -> abi::ER {
    let mut er = abi::E_TMOUT;
    for tmo in dur2tmos(dur) {
        er = f(tmo);
        if er != abi::E_TMOUT {
            break;
        }
    }
    er
}

/// 将 `Duration` 拆分为一个或多个带超时的 API 调用。此函数能够处理虚假唤醒
/// （spurious wakeup）。
#[inline]
pub fn with_tmos_strong(dur: Duration, mut f: impl FnMut(abi::TMO) -> abi::ER) -> abi::ER {
    // `TMO` 与 `SYSTIM` 都以微秒为单位。
    // 出于性能原因，钳制在 `SYSTIM::MAX`。这在实践中不应造成问题。
    // （`u64::MAX` μs ≈ 584942 年）
    let ticks = dur.as_micros().min(abi::SYSTIM::MAX as u128) as abi::SYSTIM;

    let start = Instant::now().0;
    let mut elapsed = 0;
    let mut er = abi::E_TMOUT;
    while elapsed <= ticks {
        er = f(elapsed.min(abi::TMAX_RELTIM as abi::SYSTIM) as abi::TMO);
        if er != abi::E_TMOUT {
            break;
        }
        elapsed = Instant::now().0.wrapping_sub(start);
    }

    er
}

#[cfg(test)]
mod tests;
