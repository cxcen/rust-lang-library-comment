use super::abi;
use crate::cell::UnsafeCell;
use crate::mem::MaybeUninit;
use crate::sync::atomic::{Atomic, AtomicBool, AtomicUsize, Ordering};

/// 一种由 `dis_dsp`（用于核内同步）与自旋锁（用于核间同步）实现的互斥锁。
pub struct SpinMutex<T = ()> {
    locked: Atomic<bool>,
    data: UnsafeCell<T>,
}

impl<T> SpinMutex<T> {
    #[inline]
    pub const fn new(x: T) -> Self {
        Self { locked: AtomicBool::new(false), data: UnsafeCell::new(x) }
    }

    /// 获取一把锁。
    #[inline]
    pub fn with_locked<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
        struct SpinMutexGuard<'a>(&'a Atomic<bool>);

        impl Drop for SpinMutexGuard<'_> {
            #[inline]
            fn drop(&mut self) {
                self.0.store(false, Ordering::Release);
                unsafe { abi::ena_dsp() };
            }
        }

        let _guard;
        if unsafe { abi::sns_dsp() } == 0 {
            let er = unsafe { abi::dis_dsp() };
            debug_assert!(er >= 0);

            // 等待，直到当前处理器获取到锁。
            while self.locked.swap(true, Ordering::Acquire) {}

            _guard = SpinMutexGuard(&self.locked);
        }

        f(unsafe { &mut *self.data.get() })
    }
}

/// 由 `dis_dsp`（用于核内同步）与自旋锁（用于核间同步）实现的
/// `OnceCell<(abi::ID, T)>`。
///
/// 这里假定 `0` 不是一个有效的 ID，并且所有内核对象 ID 都落在
/// `1..=usize::MAX` 范围内。
pub struct SpinIdOnceCell<T = ()> {
    id: Atomic<usize>,
    spin: SpinMutex<()>,
    extra: UnsafeCell<MaybeUninit<T>>,
}

const ID_UNINIT: usize = 0;

impl<T> SpinIdOnceCell<T> {
    #[inline]
    pub const fn new() -> Self {
        Self {
            id: AtomicUsize::new(ID_UNINIT),
            extra: UnsafeCell::new(MaybeUninit::uninit()),
            spin: SpinMutex::new(()),
        }
    }

    #[inline]
    pub fn get(&self) -> Option<(abi::ID, &T)> {
        match self.id.load(Ordering::Acquire) {
            ID_UNINIT => None,
            id => Some((id as abi::ID, unsafe { (&*self.extra.get()).assume_init_ref() })),
        }
    }

    #[inline]
    pub fn get_mut(&mut self) -> Option<(abi::ID, &mut T)> {
        match *self.id.get_mut() {
            ID_UNINIT => None,
            id => Some((id as abi::ID, unsafe { (&mut *self.extra.get()).assume_init_mut() })),
        }
    }

    #[inline]
    pub unsafe fn get_unchecked(&self) -> (abi::ID, &T) {
        (self.id.load(Ordering::Acquire) as abi::ID, unsafe {
            (&*self.extra.get()).assume_init_ref()
        })
    }

    /// 在不检查内容是否已初始化或正在初始化的情况下赋值。
    pub unsafe fn set_unchecked(&self, (id, extra): (abi::ID, T)) {
        debug_assert!(self.get().is_none());

        // 假定：一个正的 `abi::ID` 能放进 `usize` 中。
        debug_assert!(id >= 0);
        debug_assert!(usize::try_from(id).is_ok());
        let id = id as usize;

        unsafe { *self.extra.get() = MaybeUninit::new(extra) };
        self.id.store(id, Ordering::Release);
    }

    /// 获取该 cell 的内容；如果 cell 为空，则用 `f` 进行初始化。如果 cell 为空
    /// 且 `f` 失败，则返回一个错误。
    ///
    /// 警告：`f` 不得执行阻塞操作，这也包括 panic。
    #[inline]
    pub fn get_or_try_init<F, E>(&self, f: F) -> Result<(abi::ID, &T), E>
    where
        F: FnOnce() -> Result<(abi::ID, T), E>,
    {
        // 快速路径
        if let Some(x) = self.get() {
            return Ok(x);
        }

        self.initialize(f)?;

        debug_assert!(self.get().is_some());

        // Safety: 内部值已被初始化
        Ok(unsafe { self.get_unchecked() })
    }

    fn initialize<F, E>(&self, f: F) -> Result<(), E>
    where
        F: FnOnce() -> Result<(abi::ID, T), E>,
    {
        self.spin.with_locked(|_| {
            if self.id.load(Ordering::Relaxed) == ID_UNINIT {
                let (initialized_id, initialized_extra) = f()?;

                // 假定：一个正的 `abi::ID` 能放进 `usize` 中。
                debug_assert!(initialized_id >= 0);
                debug_assert!(usize::try_from(initialized_id).is_ok());
                let initialized_id = initialized_id as usize;

                // 存储已初始化的内容。使用 release 顺序，以确保该写入对 `get`
                // 的调用方可见。
                unsafe { *self.extra.get() = MaybeUninit::new(initialized_extra) };
                self.id.store(initialized_id, Ordering::Release);
            }
            Ok(())
        })
    }
}

impl<T> Drop for SpinIdOnceCell<T> {
    #[inline]
    fn drop(&mut self) {
        if self.get_mut().is_some() {
            unsafe { (&mut *self.extra.get()).assume_init_drop() };
        }
    }
}
