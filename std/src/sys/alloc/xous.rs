// FIXME(static_mut_refs): 不要放行 `static_mut_refs` lint
#![allow(static_mut_refs)]

use crate::alloc::{GlobalAlloc, Layout, System};

#[cfg(not(test))]
#[unsafe(export_name = "_ZN16__rust_internals3std3sys4xous5alloc8DLMALLOCE")]
static mut DLMALLOC: dlmalloc::Dlmalloc = dlmalloc::Dlmalloc::new();

#[cfg(test)]
unsafe extern "Rust" {
    #[link_name = "_ZN16__rust_internals3std3sys4xous5alloc8DLMALLOCE"]
    static mut DLMALLOC: dlmalloc::Dlmalloc;
}

#[stable(feature = "alloc_system_type", since = "1.28.0")]
unsafe impl GlobalAlloc for System {
    #[inline]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: 对 DLMALLOC 的访问保证是安全的，因为该锁给了我们独占且非重入的访问权。
        // 调用 malloc() 是安全的，因为本函数的前置条件与该 trait 方法的前置条件相匹配。
        let _lock = lock::lock();
        unsafe { DLMALLOC.malloc(layout.size(), layout.align()) }
    }

    #[inline]
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: 对 DLMALLOC 的访问保证是安全的，因为该锁给了我们独占且非重入的访问权。
        // 调用 calloc() 是安全的，因为本函数的前置条件与该 trait 方法的前置条件相匹配。
        let _lock = lock::lock();
        unsafe { DLMALLOC.calloc(layout.size(), layout.align()) }
    }

    #[inline]
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: 对 DLMALLOC 的访问保证是安全的，因为该锁给了我们独占且非重入的访问权。
        // 调用 free() 是安全的，因为本函数的前置条件与该 trait 方法的前置条件相匹配。
        let _lock = lock::lock();
        unsafe { DLMALLOC.free(ptr, layout.size(), layout.align()) }
    }

    #[inline]
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: 对 DLMALLOC 的访问保证是安全的，因为该锁给了我们独占且非重入的访问权。
        // 调用 realloc() 是安全的，因为本函数的前置条件与该 trait 方法的前置条件相匹配。
        let _lock = lock::lock();
        unsafe { DLMALLOC.realloc(ptr, layout.size(), layout.align(), new_size) }
    }
}

mod lock {
    use crate::sync::atomic::Ordering::{Acquire, Release};
    use crate::sync::atomic::{Atomic, AtomicI32};

    static LOCKED: Atomic<i32> = AtomicI32::new(0);

    pub struct DropLock;

    pub fn lock() -> DropLock {
        loop {
            if LOCKED.swap(1, Acquire) == 0 {
                return DropLock;
            }
            crate::os::xous::ffi::do_yield();
        }
    }

    impl Drop for DropLock {
        fn drop(&mut self) {
            let r = LOCKED.swap(0, Release);
            debug_assert_eq!(r, 1);
        }
    }
}
