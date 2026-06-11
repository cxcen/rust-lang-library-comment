use crate::alloc::{GlobalAlloc, Layout, System};
use crate::ptr;
use crate::sync::atomic::{Atomic, AtomicBool, Ordering};
use crate::sys::pal::abi::mem as sgx_mem;
use crate::sys::pal::waitqueue::SpinMutex;

// 使用 SpinMutex，因为我们绝不希望为了等待分配器而退出 enclave（飞地）。
//
// 这里目前使用的分配器是 `dlmalloc` crate，它作为子模块（submodule）被收录在
// rust-lang/rust 仓库中。该 crate 是把 dlmalloc.c 从 C 移植到 Rust 的产物。
//
// 指定 linkage/符号名，纯粹是为了确保本 crate 与其单元测试之间共用同一个实例
#[cfg_attr(test, linkage = "available_externally")]
#[unsafe(export_name = "_ZN16__rust_internals3std3sys5alloc3sgx8DLMALLOCE")]
static DLMALLOC: SpinMutex<dlmalloc::Dlmalloc<Sgx>> =
    SpinMutex::new(dlmalloc::Dlmalloc::new_with_allocator(Sgx {}));

struct Sgx;

unsafe impl dlmalloc::Allocator for Sgx {
    /// 分配系统资源
    fn alloc(&self, _size: usize) -> (*mut u8, usize, u32) {
        static INIT: Atomic<bool> = AtomicBool::new(false);

        // 无需顺序（ordering）要求，因为本函数受全局锁保护。
        if !INIT.swap(true, Ordering::Relaxed) {
            (sgx_mem::heap_base() as _, sgx_mem::heap_size(), 0)
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
        // SAFETY: 调用者必须遵守 `malloc` 的安全契约
        unsafe { DLMALLOC.lock().malloc(layout.size(), layout.align()) }
    }

    #[inline]
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: 调用者必须遵守 `malloc` 的安全契约
        unsafe { DLMALLOC.lock().calloc(layout.size(), layout.align()) }
    }

    #[inline]
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: 调用者必须遵守 `malloc` 的安全契约
        unsafe { DLMALLOC.lock().free(ptr, layout.size(), layout.align()) }
    }

    #[inline]
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: 调用者必须遵守 `malloc` 的安全契约
        unsafe { DLMALLOC.lock().realloc(ptr, layout.size(), layout.align(), new_size) }
    }
}

// 以下这些函数是 libunwind 所需要的。这些符号名在目标平台规格说明
// （target specification）的 pre-link args 中被引用，所以要保持二者同步。
#[cfg(not(test))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn __rust_c_alloc(size: usize, align: usize) -> *mut u8 {
    unsafe { crate::alloc::alloc(Layout::from_size_align_unchecked(size, align)) }
}

#[cfg(not(test))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn __rust_c_dealloc(ptr: *mut u8, size: usize, align: usize) {
    unsafe { crate::alloc::dealloc(ptr, Layout::from_size_align_unchecked(size, align)) }
}
