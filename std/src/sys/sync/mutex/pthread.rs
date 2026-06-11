#![forbid(unsafe_op_in_unsafe_fn)]

use crate::mem::forget;
use crate::pin::Pin;
use crate::sys::pal::sync as pal;
use crate::sys::sync::OnceBox;

pub struct Mutex {
    pub(in crate::sys::sync) pal: OnceBox<pal::Mutex>,
}

impl Mutex {
    #[inline]
    pub const fn new() -> Mutex {
        Mutex { pal: OnceBox::new() }
    }

    #[inline]
    fn get(&self) -> Pin<&pal::Mutex> {
        // 如果在初始化竞争中落败，新创建的 mutex 会被销毁。
        // 不过这是合理（sound）的，因为它不可能已经被加过锁。
        self.pal.get_or_init(|| {
            let mut pal = Box::pin(pal::Mutex::new());
            // SAFETY: 我们对每个 `pal::Mutex` 只调用一次 `init`，也就是在这里。
            unsafe { pal.as_mut().init() };
            pal
        })
    }

    #[inline]
    // 将其标记为诊断项（diagnostic item），供 Miri 的并发模型检查器识别。
    #[cfg_attr(not(test), rustc_diagnostic_item = "sys_mutex_lock")]
    pub fn lock(&self) {
        // SAFETY: 我们在上面调用了 `init`，因此可重入加锁是安全的。
        // 在 `drop` 中我们确保 mutex 不会在加锁状态下被销毁。
        unsafe { self.get().lock() }
    }

    #[inline]
    // 将其标记为诊断项（diagnostic item），供 Miri 的并发模型检查器识别。
    #[cfg_attr(not(test), rustc_diagnostic_item = "sys_mutex_unlock")]
    pub unsafe fn unlock(&self) {
        // SAFETY: 只有在 mutex 已经初始化的情况下才可能对其加锁，而既然我们观测到了
        // 加锁，那么也就观测到了这次初始化。
        unsafe { self.pal.get_unchecked().unlock() }
    }

    #[inline]
    // 将其标记为诊断项（diagnostic item），供 Miri 的并发模型检查器识别。
    #[cfg_attr(not(test), rustc_diagnostic_item = "sys_mutex_try_lock")]
    pub fn try_lock(&self) -> bool {
        // SAFETY: 我们在上面调用了 `init`，因此可重入加锁是安全的。
        // 在 `drop` 中我们确保 mutex 不会在加锁状态下被销毁。
        unsafe { self.get().try_lock() }
    }
}

impl Drop for Mutex {
    fn drop(&mut self) {
        let Some(pal) = self.pal.take() else { return };
        // 我们不被允许对一个已加锁的 mutex 调用 pthread_mutex_destroy，
        // 所以先检查它是否处于未锁定状态。
        if unsafe { pal.as_ref().try_lock() } {
            unsafe { pal.as_ref().unlock() };
            drop(pal)
        } else {
            // mutex 处于加锁状态。这发生在某个 MutexGuard 被泄漏（leak）时。
            // 这种情况下，我们也干脆把 Mutex 一并泄漏掉。
            forget(pal)
        }
    }
}
