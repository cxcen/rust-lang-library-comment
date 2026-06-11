#![forbid(unsafe_op_in_unsafe_fn)]
use crate::pin::Pin;
use crate::sync::atomic::Ordering::{Acquire, Release};
use crate::sys::futex::{self, futex_wait, futex_wake};
use crate::time::Duration;

type Futex = futex::SmallFutex;
type State = futex::SmallPrimitive;

const PARKED: State = State::MAX;
const EMPTY: State = 0;
const NOTIFIED: State = 1;

pub struct Parker {
    state: Futex,
}

// 关于内存序的说明：
//
// 内存序只对「不同变量之间各操作的相对顺序」有意义。即便是 Ordering::Relaxed，
// 在只看单个原子变量时也能保证一个单调/一致的顺序。
//
// 因此，既然这个 parker 只是一个单独的原子变量，我们只需要关注我们需要向「外部
// 世界」提供哪些顺序保证即可。
//
// park 和 unpark 提供的唯一内存序保证是：在 unpark() 之前发生的事情，对于随后从
// park() 返回的那个线程是可见的。否则，就相当于在 unpark() 被调用之前就已经被
// unpark 了，但同时仍然消费了那个「令牌（token）」。
//
// 换句话说，unpark() 需要与 park() 中「消费令牌并返回」的那一部分建立同步关系。
//
// 这是通过一次 release-acquire 同步实现的：在 unpark() 中写入 NOTIFIED（即那个
//「令牌」）时使用 Ordering::Release，而在 park() 中检查这个状态时使用
// Ordering::Acquire。
impl Parker {
    /// 构造 futex parker。UNIX 的 parker 实现要求这必须就地（in-place）进行。
    pub unsafe fn new_in_place(parker: *mut Parker) {
        unsafe { parker.write(Self { state: Futex::new(EMPTY) }) };
    }

    // 假定本函数只由拥有该 Parker 的线程调用，
    // 这意味着 `self.state != PARKED`。
    pub unsafe fn park(self: Pin<&Self>) {
        // 把 NOTIFIED=>EMPTY 或 EMPTY=>PARKED，并在前一种情况下直接返回。
        if self.state.fetch_sub(1, Acquire) == NOTIFIED {
            return;
        }
        loop {
            // 在假定它仍被设为 PARKED 的前提下，等待有事情发生。
            futex_wait(&self.state, PARKED, None);
            // 把 NOTIFIED=>EMPTY，并在那种情况下返回。
            if self.state.compare_exchange(NOTIFIED, EMPTY, Acquire, Acquire).is_ok() {
                return;
            } else {
                // 虚假唤醒（spurious wake up）。我们循环回去重试。
            }
        }
    }

    // 假定本函数只由拥有该 Parker 的线程调用，
    // 这意味着 `self.state != PARKED`。本实现不需要 `Pin`，但其他实现需要。
    pub unsafe fn park_timeout(self: Pin<&Self>, timeout: Duration) {
        // 把 NOTIFIED=>EMPTY 或 EMPTY=>PARKED，并在前一种情况下直接返回。
        if self.state.fetch_sub(1, Acquire) == NOTIFIED {
            return;
        }
        // 在假定它仍被设为 PARKED 的前提下，等待有事情发生。
        futex_wait(&self.state, PARKED, Some(timeout));
        // 这不仅仅是一次 store，因为我们需要与 unpark() 建立一个
        // release-acquire 顺序。
        if self.state.swap(EMPTY, Acquire) == NOTIFIED {
            // 因为 unpark() 而醒来。
        } else {
            // 超时或虚假唤醒。
            // 无论哪种情况我们都返回，因为我们无法轻易分辨到底是不是超时。
        }
    }

    // 本实现不需要 `Pin`，但其他实现需要。
    #[inline]
    pub fn unpark(self: Pin<&Self>) {
        // 把 PARKED=>NOTIFIED、EMPTY=>NOTIFIED 或 NOTIFIED=>NOTIFIED，
        // 并在第一种情况下唤醒该线程。
        //
        // 注意即便是 NOTIFIED=>NOTIFIED 也会产生一次写操作。这是有意为之，
        // 以确保每一次 unpark() 都与 park() 之间存在一个 release-acquire 顺序。
        if self.state.swap(NOTIFIED, Release) == PARKED {
            futex_wake(&self.state);
        }
    }
}
