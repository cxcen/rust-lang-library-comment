use crate::sys::pal::waitqueue::{SpinMutex, WaitQueue, WaitVariable, try_lock_or_false};
use crate::sys::sync::OnceBox;

pub struct Mutex {
    // FIXME: `UnsafeList` 不可移动（not movable）。
    inner: OnceBox<SpinMutex<WaitVariable<bool>>>,
}

// 实现依据《Operating Systems: Three Easy Pieces》第 28 章
impl Mutex {
    pub const fn new() -> Mutex {
        Mutex { inner: OnceBox::new() }
    }

    fn get(&self) -> &SpinMutex<WaitVariable<bool>> {
        self.inner.get_or_init(|| Box::pin(SpinMutex::new(WaitVariable::new(false)))).get_ref()
    }

    #[inline]
    pub fn lock(&self) {
        let mut guard = self.get().lock();
        if *guard.lock_var() {
            // 另一个线程持有该锁，等待
            WaitQueue::wait(guard, || {})
        // 另一个线程已经把锁传递给了我们
        } else {
            // 我们正是此刻获得该锁
            *guard.lock_var_mut() = true;
        }
    }

    #[inline]
    pub unsafe fn unlock(&self) {
        // SAFETY: 该 mutex 是被当前线程加锁的，所以它已经初始化过了。
        let guard = unsafe { self.inner.get_unchecked().get_ref().lock() };
        if let Err(mut guard) = WaitQueue::notify_one(guard) {
            // 没有其他等待者，解锁
            *guard.lock_var_mut() = false;
        } else {
            // 当时有一个线程在等待，直接把锁传递过去
        }
    }

    #[inline]
    pub fn try_lock(&self) -> bool {
        let mut guard = try_lock_or_false!(self.get());
        if *guard.lock_var() {
            // 另一个线程持有该锁
            false
        } else {
            // 我们正是此刻获得该锁
            *guard.lock_var_mut() = true;
            true
        }
    }
}
