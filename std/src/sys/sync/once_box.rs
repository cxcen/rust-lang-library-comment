//! 一个可竞争初始化（racily-initialized）的 `OnceLock<Box<T>>` 替代实现。
//!
//! 它被用来实现那些需要分配内存的同步原语，例如基于 pthread 的版本。

#![allow(dead_code)] // 仅在部分平台上使用。

use crate::mem::replace;
use crate::pin::Pin;
use crate::ptr::null_mut;
use crate::sync::atomic::Ordering::{Acquire, Relaxed, Release};
use crate::sync::atomic::{Atomic, AtomicPtr};

pub(crate) struct OnceBox<T> {
    ptr: Atomic<*mut T>,
}

impl<T> OnceBox<T> {
    #[inline]
    pub const fn new() -> Self {
        Self { ptr: AtomicPtr::new(null_mut()) }
    }

    /// 在「值已经初始化，且该初始化已被当前线程观测到」的前提下访问该值。
    ///
    /// 由于对指针的所有修改都已经被观测到，本函数中的指针 load 可以使用
    /// relaxed 内存序，从而让优化器有机会把这样的代码：
    /// ```rust, ignore
    /// once_box.get_or_init(|| Box::pin(42));
    /// unsafe { once_box.get_unchecked() }
    /// ```
    /// 优化成：
    /// ```rust, ignore
    /// once_box.get_or_init(|| Box::pin(42))
    /// ```
    ///
    /// # 安全性(Safety）
    /// 若上述前提被违反，则会导致未定义行为。
    #[inline]
    pub unsafe fn get_unchecked(&self) -> Pin<&T> {
        unsafe { Pin::new_unchecked(&*self.ptr.load(Relaxed)) }
    }

    #[inline]
    pub fn get_or_init(&self, f: impl FnOnce() -> Pin<Box<T>>) -> Pin<&T> {
        let ptr = self.ptr.load(Acquire);
        match unsafe { ptr.as_ref() } {
            Some(val) => unsafe { Pin::new_unchecked(val) },
            None => self.initialize(f),
        }
    }

    #[inline]
    pub fn take(&mut self) -> Option<Pin<Box<T>>> {
        let ptr = replace(self.ptr.get_mut(), null_mut());
        if !ptr.is_null() { Some(unsafe { Pin::new_unchecked(Box::from_raw(ptr)) }) } else { None }
    }

    #[cold]
    fn initialize(&self, f: impl FnOnce() -> Pin<Box<T>>) -> Pin<&T> {
        let new_ptr = Box::into_raw(unsafe { Pin::into_inner_unchecked(f()) });
        match self.ptr.compare_exchange(null_mut(), new_ptr, Release, Acquire) {
            Ok(_) => unsafe { Pin::new_unchecked(&*new_ptr) },
            Err(ptr) => {
                // 在与另一个线程的竞争中落败。
                // 丢弃我们自己创建的值，转而使用另一个线程创建的那个。
                drop(unsafe { Box::from_raw(new_ptr) });
                unsafe { Pin::new_unchecked(&*ptr) }
            }
        }
    }
}

unsafe impl<T: Send> Send for OnceBox<T> {}
unsafe impl<T: Send + Sync> Sync for OnceBox<T> {}

impl<T> Drop for OnceBox<T> {
    fn drop(&mut self) {
        self.take();
    }
}
