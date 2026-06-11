use super::{MIN_ALIGN, realloc_fallback};
use crate::alloc::{GlobalAlloc, Layout, System};
use crate::ptr;

#[stable(feature = "alloc_system_type", since = "1.28.0")]
unsafe impl GlobalAlloc for System {
    #[inline]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // 对于小块分配，jemalloc 提供的对齐可能小于 MIN_ALIGN。
        // 因此只有在 size >= align 时才依赖 MIN_ALIGN。
        // 另见 <https://github.com/rust-lang/rust/issues/45955> 与
        // <https://github.com/rust-lang/rust/issues/62251#issuecomment-507580914>。
        if layout.align() <= MIN_ALIGN && layout.align() <= layout.size() {
            unsafe { libc::malloc(layout.size()) as *mut u8 }
        } else {
            // 在 Apple 旧版本平台上，如果传入一个非常大的对齐值，
            // `posix_memalign` 会返回未对齐的值（具体是哪个版本区间不详，
            // 但该问题在 macOS 10.14 与 iOS 13.3 中确实存在）。
            //
            // <https://github.com/rust-lang/rust/issues/30170>
            #[cfg(target_vendor = "apple")]
            {
                if layout.align() > (1 << 31) {
                    return ptr::null_mut();
                }
            }
            unsafe { aligned_malloc(&layout) }
        }
    }

    #[inline]
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // 此处检查为何写成这样，参见上面 `alloc` 中的注释。
        if layout.align() <= MIN_ALIGN && layout.align() <= layout.size() {
            unsafe { libc::calloc(layout.size(), 1) as *mut u8 }
        } else {
            let ptr = unsafe { self.alloc(layout) };
            if !ptr.is_null() {
                unsafe { ptr::write_bytes(ptr, 0, layout.size()) };
            }
            ptr
        }
    }

    #[inline]
    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        unsafe { libc::free(ptr as *mut libc::c_void) }
    }

    #[inline]
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if layout.align() <= MIN_ALIGN && layout.align() <= new_size {
            unsafe { libc::realloc(ptr as *mut libc::c_void, new_size) as *mut u8 }
        } else {
            unsafe { realloc_fallback(self, ptr, layout, new_size) }
        }
    }
}

cfg_select! {
    // 我们尽可能使用 posix_memalign，但某些目标平台对 POSIX 的覆盖非常不完整，
    // 因此需要为它们准备一个回退方案。
    any(target_os = "horizon", target_os = "vita") => {
        #[inline]
        unsafe fn aligned_malloc(layout: &Layout) -> *mut u8 {
            unsafe { libc::memalign(layout.align(), layout.size()) as *mut u8 }
        }
    }
    _ => {
        #[inline]
        #[cfg_attr(target_os = "vxworks", allow(unused_unsafe))]
        unsafe fn aligned_malloc(layout: &Layout) -> *mut u8 {
            let mut out = ptr::null_mut();
            // 我们偏好 posix_memalign 而非 aligned_alloc，因为前者可用范围更广；而且
            // 对于 aligned_alloc，各实现对哪些对齐值“受支持”几乎是任意选择的，导致
            // 难以使用。例如，有的实现要求 size 必须是对齐的整数倍（wasi emmalloc），
            // 另一些则要求对齐至少为指针大小（Illumos、macOS）。
            // posix_memalign 只有一条清晰的要求：对齐必须是 `sizeof(void*)` 的整数倍。
            // 由于这些都是 2 的幂，我们直接取 max 即可。
            let align = layout.align().max(size_of::<usize>());
            let ret = unsafe { libc::posix_memalign(&mut out, align, layout.size()) };
            if ret != 0 { ptr::null_mut() } else { out as *mut u8 }
        }
    }
}
