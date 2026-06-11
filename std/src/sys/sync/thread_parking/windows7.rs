// 针对 Windows 的线程 parker 实现。
//
// 如果可用（Windows 8+），它会使用 WaitOnAddress 和 WakeByAddressSingle。
// 这套现代 API 与 Linux 线程 parker 所用的 futex 系统调用完全相同。当这些 API
// 可用时，本线程 parker 的实现与 Linux 线程 parker 完全一致。
//
// 然而，当这套现代 API 不可用时，本实现会回退到 NT Keyed Events（NT 键控事件），
// 它们与之类似，但有一些重要的差异。这套机制自 Windows XP 起即可用。
//
// WaitOnAddress 会先检查线程 parker 的状态，以确保在更新 parker 状态与调用该函数
// 之间不会漏掉任何 WakeByAddressSingle 调用。
//
// NtWaitForKeyedEvent 没有这个选项，它会无条件阻塞，而不先检查 parker 状态。
// 作为代替，NtReleaseKeyedEvent（不同于 WakeByAddressSingle）会*阻塞*，
// 直到它唤醒了一个正通过 NtWaitForKeyedEvent 等待它的线程。这样，我们就能确保
// 没有事件被漏掉，但我们需要小心：如果 park_timeout() 是因超时而非 unpark() 被唤醒，
// 就不要阻塞 unpark()。
//
// 与 WaitOnAddress 不同，NtWaitForKeyedEvent/NtReleaseKeyedEvent 是在一个 HANDLE
//（由 NtCreateKeyedEvent 创建）上操作的。这意味着我们可以确信：一次成功被唤醒的
// park() 是被 unpark() 唤醒的，而不是被某段其他代码的 NtReleaseKeyedEvent 调用唤醒的，
// 因为这些事件不仅要按 key（parker（状态）的地址）匹配，还要按这个 HANDLE 匹配。
// 我们会在首次需要时惰性（lazily）分配这个 handle。
//
// 快速路径（在 unpark() 已经被调用之后再调用 park()）以及可能的各个状态，对两种实现
// 而言是相同的。这里利用了这一点，确保快速路径甚至无需检查使用哪套 API，
// 而可以立即返回，与所用的 API 无关。只有慢速路径（会真正阻塞/唤醒线程的路径）
// 才会检查哪套 API 可用，并采用不同的实现。
//
// 遗憾的是，NT Keyed Events 是一套未公开文档的 Windows API。然而：
// - 这套 API 相对简单、行为显而易见，并且有若干（非官方的）文章记录了其细节。[1]
// - `parking_lot` 已经使用这套 API 多年（用于 Windows 8 之前的 Windows 版本）。[2]
//   许多大型项目都大量使用 parking_lot，例如 servo 和 Rust 编译器本身。
// - 它是 Windows SRW 锁和 Windows 临界区（critical sections）底层所用的 API。[3] [4]
// - Wine、ReactOS 和 Windows XP 各自实现的源代码都可获取，并与预期行为相符。
// - 使用一套未公开文档的 API，主要风险在于它将来可能会改变。但由于我们只在较旧的
//   Windows 版本上使用它，所以这并不是问题。
// - 即便这些函数没有按我们预期的那样阻塞或唤醒（这不太可能，参见前面所有要点），
//   本实现仍然是内存安全的。NT Keyed Events API 只用于在正确的位置进行睡眠/阻塞。
//
// [1]: http://www.locklessinc.com/articles/keyed_events/
// [2]: https://github.com/Amanieu/parking_lot/commit/43abbc964e
// [3]: https://docs.microsoft.com/en-us/archive/msdn-magazine/2012/november/windows-with-c-the-evolution-of-synchronization-in-windows-and-c
// [4]: Windows Internals, Part 1, ISBN 9780735671300

use core::ffi::c_void;

use crate::pin::Pin;
use crate::sync::atomic::Ordering::{Acquire, Release};
use crate::sync::atomic::{Atomic, AtomicI8};
use crate::sys::{c, dur2timeout};
use crate::time::Duration;

pub struct Parker {
    state: Atomic<i8>,
}

const PARKED: i8 = -1;
const EMPTY: i8 = 0;
const NOTIFIED: i8 = 1;

// 关于内存序的说明：
//
// 内存序只与不同变量之间各操作的相对顺序相关。即便是 Ordering::Relaxed，
// 在只看单个原子变量时也能保证一个单调/一致的顺序。
//
// 因此，由于这个 parker 只是单个原子变量，我们只需要关注我们必须向“外部世界”
// 提供哪些顺序保证即可。
//
// parking 与 unparking 提供的唯一内存序保证是：在 unpark() 之前发生的事情，
// 在随后从 park() 返回的线程上是可见的。否则，那相当于线程在 unpark() 被调用之前
// 就已被 unpark 了，同时还消费了那个“令牌（token）”。
//
// 换句话说，unpark() 需要与 park() 中消费令牌并返回的那一部分相互同步。
//
// 这是通过一次 release-acquire 同步来实现的：在 unpark() 中写入 NOTIFIED
//（即“令牌”）时使用 Ordering::Release，而在被唤醒后于 park() 中读取该状态时
// 使用 Ordering::Acquire。
impl Parker {
    /// 构造 Windows parker。UNIX parker 实现要求它就地（in-place）发生。
    pub unsafe fn new_in_place(parker: *mut Parker) {
        parker.write(Self { state: AtomicI8::new(EMPTY) });
    }

    // 假定它只会被拥有该 Parker 的线程调用，这意味着 `self.state != PARKED`。
    // 本实现并不要求 `Pin`，但其他实现需要。
    pub unsafe fn park(self: Pin<&Self>) {
        // 把 NOTIFIED=>EMPTY 或 EMPTY=>PARKED，并在前一种情况下直接返回。
        if self.state.fetch_sub(1, Acquire) == NOTIFIED {
            return;
        }

        #[cfg(target_vendor = "win7")]
        if c::WaitOnAddress::option().is_none() {
            return keyed_events::park(self);
        }

        loop {
            // 等待有事情发生，前提是它仍被设为 PARKED。
            c::WaitOnAddress(self.ptr(), &PARKED as *const _ as *const c_void, 1, c::INFINITE);
            // 把 NOTIFIED=>EMPTY，但不动 PARKED。
            if self.state.compare_exchange(NOTIFIED, EMPTY, Acquire, Acquire).is_ok() {
                // 确实是被 unpark() 唤醒的。
                return;
            } else {
                // 虚假唤醒（spurious wake up）。我们循环重试。
            }
        }
    }

    // 假定它只会被拥有该 Parker 的线程调用，这意味着 `self.state != PARKED`。
    // 本实现并不要求 `Pin`，但其他实现需要。
    pub unsafe fn park_timeout(self: Pin<&Self>, timeout: Duration) {
        // 把 NOTIFIED=>EMPTY 或 EMPTY=>PARKED，并在前一种情况下直接返回。
        if self.state.fetch_sub(1, Acquire) == NOTIFIED {
            return;
        }

        #[cfg(target_vendor = "win7")]
        if c::WaitOnAddress::option().is_none() {
            return keyed_events::park_timeout(self, timeout);
        }

        // 等待有事情发生，前提是它仍被设为 PARKED。
        c::WaitOnAddress(self.ptr(), &PARKED as *const _ as *const c_void, 1, dur2timeout(timeout));
        // 把状态设回 EMPTY（来源可能是 PARKED 或 NOTIFIED）。
        // 注意我们并不是简单地写入 EMPTY，而是使用 swap()，从而同时包含一次
        // acquire 序的读取，以与 unpark() 的 release 序写入相互同步。
        if self.state.swap(EMPTY, Acquire) == NOTIFIED {
            // 确实是被 unpark() 唤醒的。
        } else {
            // 超时或虚假唤醒。
            // 我们无论如何都返回，因为我们无法轻易判断它是超时还是 unpark()。
        }
    }

    // 本实现并不要求 `Pin`，但其他实现需要。
    pub fn unpark(self: Pin<&Self>) {
        // 把 PARKED=>NOTIFIED、EMPTY=>NOTIFIED 或 NOTIFIED=>NOTIFIED，
        // 并在第一种情况下唤醒该线程。
        //
        // 注意，即使 NOTIFIED=>NOTIFIED 也会产生一次写入。这是有意为之，
        // 以确保每一次 unpark() 都与 park() 之间存在 release-acquire 顺序。
        if self.state.swap(NOTIFIED, Release) == PARKED {
            unsafe {
                #[cfg(target_vendor = "win7")]
                if c::WakeByAddressSingle::option().is_none() {
                    return keyed_events::unpark(self);
                }
                c::WakeByAddressSingle(self.ptr());
            }
        }
    }

    fn ptr(&self) -> *const c_void {
        (&raw const self.state).cast::<c_void>()
    }
}

#[cfg(target_vendor = "win7")]
mod keyed_events {
    use core::pin::Pin;
    use core::ptr;
    use core::sync::atomic::Ordering::{Acquire, Relaxed};
    use core::sync::atomic::{Atomic, AtomicPtr};
    use core::time::Duration;

    use super::{EMPTY, NOTIFIED, Parker};
    use crate::sys::c;

    pub unsafe fn park(parker: Pin<&Parker>) {
        // 等待 unpark() 产生这个事件。
        c::NtWaitForKeyedEvent(keyed_event_handle(), parker.ptr(), false, ptr::null_mut());
        // 把状态设回 EMPTY（来源可能是 PARKED 或 NOTIFIED）。
        // 注意我们并不是简单地写入 EMPTY，而是使用 swap()，从而同时包含一次
        // acquire 序的读取，以与 unpark() 的 release 序写入相互同步。
        parker.state.swap(EMPTY, Acquire);
        return;
    }
    pub unsafe fn park_timeout(parker: Pin<&Parker>, timeout: Duration) {
        // 需要使用 NtWaitForKeyedEvent 等待 unpark()。
        let handle = keyed_event_handle();

        // NtWaitForKeyedEvent 以 100ns 为单位，并使用负值表示相对于单调时钟
        //（monotonic clock）的相对时间。
        // 这一点在底层的 KeWaitForSingleObject 函数文档中有记载：
        // https://docs.microsoft.com/en-us/windows-hardware/drivers/ddi/wdm/nf-wdm-kewaitforsingleobject
        let mut timeout = match i64::try_from((timeout.as_nanos() + 99) / 100) {
            Ok(t) => -t,
            Err(_) => i64::MIN,
        };

        // 等待 unpark() 产生这个事件。
        let unparked =
            c::NtWaitForKeyedEvent(handle, parker.ptr(), false, &mut timeout) == c::STATUS_SUCCESS;

        // 把状态设回 EMPTY（来源可能是 PARKED 或 NOTIFIED）。
        let prev_state = parker.state.swap(EMPTY, Acquire);

        if !unparked && prev_state == NOTIFIED {
            // 我们是被超时唤醒的，而不是被 unpark() 唤醒；但状态已被设为 NOTIFIED，
            // 这意味着我们*刚好*错过了一次 unpark()，而那次 unpark() 现在正阻塞着
            // 等我们。等待它消费掉这个事件，从而解除那个线程的阻塞。
            c::NtWaitForKeyedEvent(handle, parker.ptr(), false, ptr::null_mut());
        }
    }
    pub unsafe fn unpark(parker: Pin<&Parker>) {
        // 如果我们在等待线程运行 NtWaitForKeyedEvent 之前就运行了 NtReleaseKeyedEvent，
        // 那么它会（短暂地）阻塞，直到我们能够把它唤醒。
        // 如果等待线程在我们运行 NtReleaseKeyedEvent 之前就醒来（例如因为超时），
        // 那么它会阻塞，直到我们确实唤醒了某个线程。
        // 为了防止本线程在那种情况下无限期阻塞，park_impl() 会在醒来后看到状态被设为
        // NOTIFIED 之后，再次调用 NtWaitForKeyedEvent 来解除我们的阻塞。
        c::NtReleaseKeyedEvent(keyed_event_handle(), parker.ptr(), false, ptr::null_mut());
    }

    fn keyed_event_handle() -> c::HANDLE {
        const INVALID: c::HANDLE = ptr::without_provenance_mut(!0);
        static HANDLE: Atomic<*mut crate::ffi::c_void> = AtomicPtr::new(INVALID);
        match HANDLE.load(Relaxed) {
            INVALID => {
                let mut handle = c::INVALID_HANDLE_VALUE;
                unsafe {
                    match c::NtCreateKeyedEvent(
                        &mut handle,
                        c::GENERIC_READ | c::GENERIC_WRITE,
                        ptr::null_mut(),
                        0,
                    ) {
                        c::STATUS_SUCCESS => {}
                        r => panic!("Unable to create keyed event handle: error {r}"),
                    }
                }
                match HANDLE.compare_exchange(INVALID, handle, Relaxed, Relaxed) {
                    Ok(_) => handle,
                    Err(h) => {
                        // 在竞争中输给了另一个先于我们初始化 HANDLE 的线程。
                        // 关闭我们的 handle，改用它们的。
                        unsafe {
                            c::CloseHandle(handle);
                        }
                        h
                    }
                }
            }
            handle => handle,
        }
    }
}
