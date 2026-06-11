use crate::os::xous::ffi::{blocking_scalar, scalar};
use crate::os::xous::services::{TicktimerScalar, ticktimer_server};
use crate::pin::Pin;
use crate::ptr;
use crate::sync::atomic::Ordering::{Acquire, Release};
use crate::sync::atomic::{Atomic, AtomicI8};
use crate::time::Duration;

const NOTIFIED: i8 = 1;
const EMPTY: i8 = 0;
const PARKED: i8 = -1;

pub struct Parker {
    state: Atomic<i8>,
}

impl Parker {
    pub unsafe fn new_in_place(parker: *mut Parker) {
        unsafe { parker.write(Parker { state: AtomicI8::new(EMPTY) }) }
    }

    fn index(&self) -> usize {
        ptr::from_ref(self).addr()
    }

    pub unsafe fn park(self: Pin<&Self>) {
        // 把 NOTIFIED 改为 EMPTY，把 EMPTY 改为 PARKED。
        let state = self.state.fetch_sub(1, Acquire);
        if state == NOTIFIED {
            // 状态已从 NOTIFIED (1) 变为 EMPTY (0)
            return;
        }
        // 状态已从 EMPTY (0) 变为 PARKED (-1)
        assert!(state == EMPTY);

        // 现在状态是 PARKED (-1)。等待，直到 `unpark` 把我们唤醒。
        blocking_scalar(
            ticktimer_server(),
            TicktimerScalar::WaitForCondition(self.index(), 0).into(),
        )
        .expect("failed to send WaitForCondition command");

        let state = self.state.swap(EMPTY, Acquire);
        assert!(state == NOTIFIED || state == PARKED);
    }

    pub unsafe fn park_timeout(self: Pin<&Self>, timeout: Duration) {
        // 把 NOTIFIED 改为 EMPTY，把 EMPTY 改为 PARKED。
        let state = self.state.fetch_sub(1, Acquire);
        if state == NOTIFIED {
            // 状态已从 NOTIFIED (1) 变为 EMPTY (0)
            return;
        }
        // 状态已从 EMPTY (0) 变为 PARKED (-1)
        assert!(state == EMPTY);

        // 值为零表示无限期等待。把毫秒数限制（clamp）到允许的范围内。
        let millis = usize::max(timeout.as_millis().try_into().unwrap_or(usize::MAX), 1);

        // 现在状态是 PARKED (-1)。等待，直到 `unpark` 把我们唤醒，或者超时。
        let _was_timeout = blocking_scalar(
            ticktimer_server(),
            TicktimerScalar::WaitForCondition(self.index(), millis).into(),
        )
        .expect("failed to send WaitForCondition command")[0]
            != 0;

        let state = self.state.swap(EMPTY, Acquire);
        assert!(state == PARKED || state == NOTIFIED);
    }

    pub fn unpark(self: Pin<&Self>) {
        // 如果状态已经是 `NOTIFIED`，那么说明另一个线程已经表明它想要
        // 唤醒目标线程。
        //
        // 如果状态是 `EMPTY`，那么没有什么需要唤醒，目标线程下一次调用
        // `park()` 时会立即从中退出。
        if self.state.swap(NOTIFIED, Release) != PARKED {
            return;
        }

        // 线程处于 parked 状态，把它唤醒。一直尝试，直到我们唤醒了某个东西。
        // 当 `NotifyCondition` 调用返回「有 1 个 condition 被通知」这一事实时，
        // 就会发生这种情况。
        // 或者，一直进行下去，直到看到状态变为 `EMPTY`，表明该线程已醒来并继续运行。
        // 当 Park 在我们发出 NotifyCondition 消息之前就超时时，可能会出现这种情况。
        while blocking_scalar(
            ticktimer_server(),
            TicktimerScalar::NotifyCondition(self.index(), 1).into(),
        )
        .expect("failed to send NotifyCondition command")[0]
            == 0
            && self.state.load(Acquire) != EMPTY
        {
            // 目标线程尚未到达 `WaitForCondition` 调用。
            // 让出（yield），以便让目标线程多运行一会儿。
            crate::thread::yield_now();
        }
    }
}

impl Drop for Parker {
    fn drop(&mut self) {
        scalar(ticktimer_server(), TicktimerScalar::FreeCondition(self.index()).into()).ok();
    }
}
