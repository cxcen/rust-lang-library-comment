// FIXME(static_mut_refs): 不要放行 `static_mut_refs` lint
#![allow(static_mut_refs)]

use crate::alloc::{GlobalAlloc, Layout, System};
use crate::ptr;
use crate::sync::atomic::{AtomicBool, Ordering};

// 堆段（heap section）边界的符号，在目标平台的链接器脚本（linkerscript）中定义
unsafe extern "C" {
    static mut __heap_start: u8;
    static mut __heap_end: u8;
}

static mut DLMALLOC: dlmalloc::Dlmalloc<Vexos> = dlmalloc::Dlmalloc::new_with_allocator(Vexos);

struct Vexos;

unsafe impl dlmalloc::Allocator for Vexos {
    /// 分配系统资源
    fn alloc(&self, _size: usize) -> (*mut u8, usize, u32) {
        static INIT: AtomicBool = AtomicBool::new(false);

        if !INIT.swap(true, Ordering::Relaxed) {
            // 本目标平台没有可增长的堆，因为用户内存具有固定的
            // 大小/位置，而且 VEXos 不为我们管理分配。
            unsafe {
                (
                    (&raw mut __heap_start).cast::<u8>(),
                    (&raw const __heap_end).offset_from_unsigned(&raw const __heap_start),
                    0,
                )
            }
        } else {
            (ptr::null_mut(), 0, 0)
        }
    }

    fn remap(&self, _ptr: *mut u8, _oldsize: usize, _newsize: usize, _can_move: bool) -> *mut u8 {
        ptr::null_mut()
    }

    fn free_part(&self, _ptr: *mut u8, _oldsize: usize, _newsize: usize) -> bool {
        false
    }

    fn free(&self, _ptr: *mut u8, _size: usize) -> bool {
        return false;
    }

    fn can_release_part(&self, _flags: u32) -> bool {
        false
    }

    fn allocates_zeros(&self) -> bool {
        false
    }

    fn page_size(&self) -> usize {
        0x1000
    }
}

#[stable(feature = "alloc_system_type", since = "1.28.0")]
unsafe impl GlobalAlloc for System {
    #[inline]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: 对 DLMALLOC 的访问保证是安全的，因为我们是单线程目标平台，
        // 这保证了对分配器的独占且非重入的访问。因此不使用任何分配器锁。
        // 调用 malloc() 是安全的，因为本函数的前置条件与该 trait 方法的前置条件相匹配。
        unsafe { DLMALLOC.malloc(layout.size(), layout.align()) }
    }

    #[inline]
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: 对 DLMALLOC 的访问保证是安全的，因为我们是单线程目标平台，
        // 这保证了对分配器的独占且非重入的访问。因此不使用任何分配器锁。
        // 调用 calloc() 是安全的，因为本函数的前置条件与该 trait 方法的前置条件相匹配。
        unsafe { DLMALLOC.calloc(layout.size(), layout.align()) }
    }

    #[inline]
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: 对 DLMALLOC 的访问保证是安全的，因为我们是单线程目标平台，
        // 这保证了对分配器的独占且非重入的访问。因此不使用任何分配器锁。
        // 调用 free() 是安全的，因为本函数的前置条件与该 trait 方法的前置条件相匹配。
        unsafe { DLMALLOC.free(ptr, layout.size(), layout.align()) }
    }

    #[inline]
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: 对 DLMALLOC 的访问保证是安全的，因为我们是单线程目标平台，
        // 这保证了对分配器的独占且非重入的访问。因此不使用任何分配器锁。
        // 调用 realloc() 是安全的，因为本函数的前置条件与该 trait 方法的前置条件相匹配。
        unsafe { DLMALLOC.realloc(ptr, layout.size(), layout.align(), new_size) }
    }
}
