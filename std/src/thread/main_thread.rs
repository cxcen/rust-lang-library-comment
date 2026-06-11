//! 保存主线程的 ID。
//!
//! 主线程的线程句柄是惰性创建的，这甚至可能发生在 pre-main（main 之前）阶段。
//! 由于并非每个平台都有办法在那时识别出主线程——macOS 的 `pthread_main_np`
//! 函数是一个值得注意的例外——我们无法在那时就给它赋予正确的名字。因此，我们在
//! 运行时启动代码中记住主线程的线程 ID（通过本模块的 `set` 函数），并从那时起
//! 用它来识别主线程。这种做法可靠地工作，并且还有一个额外的好处：即便在主线程
//! 的线程句柄已被销毁之后，我们仍能报告 main 上正确的线程名。
//! 不过请注意，这也意味着在 pre-main 函数中报告的名字会是错误的，但这只是我们
//! 不得不接受的一点代价。

cfg_select! {
    target_has_atomic = "64" => {
        use super::id::ThreadId;
        use crate::sync::atomic::{Atomic, AtomicU64};
        use crate::sync::atomic::Ordering::Relaxed;

        static MAIN: Atomic<u64> = AtomicU64::new(0);

        pub(super) fn get() -> Option<ThreadId> {
            ThreadId::from_u64(MAIN.load(Relaxed))
        }

        /// # Safety
        /// 只能被调用一次。
        pub(crate) unsafe fn set(id: ThreadId) {
            MAIN.store(id.as_u64().get(), Relaxed)
        }
    }
    _ => {
        use super::id::ThreadId;
        use crate::mem::MaybeUninit;
        use crate::sync::atomic::{Atomic, AtomicBool};
        use crate::sync::atomic::Ordering::{Acquire, Release};

        static INIT: Atomic<bool> = AtomicBool::new(false);
        static mut MAIN: MaybeUninit<ThreadId> = MaybeUninit::uninit();

        pub(super) fn get() -> Option<ThreadId> {
            if INIT.load(Acquire) {
                Some(unsafe { MAIN.assume_init() })
            } else {
                None
            }
        }

        /// # Safety
        /// 只能被调用一次。
        pub(crate) unsafe fn set(id: ThreadId) {
            unsafe { MAIN = MaybeUninit::new(id) };
            INIT.store(true, Release);
        }
    }
}
