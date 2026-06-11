use super::{MIN_ALIGN, realloc_fallback};
use crate::alloc::{GlobalAlloc, Layout, System};
use crate::ffi::c_void;
use crate::mem::MaybeUninit;
use crate::ptr;
use crate::sys::c;

#[cfg(test)]
mod tests;

// Windows 上的堆内存管理是通过系统 Heap API（heapapi.h）完成的
// 参见 https://docs.microsoft.com/windows/win32/api/heapapi/

// 用于指示由 `HeapAlloc` 返回的内存应被清零的标志。
const HEAP_ZERO_MEMORY: u32 = 0x00000008;

// 获取当前进程默认堆的句柄；若操作失败则返回 null。
//
// SAFETY: 假定在同一进程内对本函数的成功调用总是返回相同的句柄，
// 且该句柄在进程的整个生命周期内始终有效。
//
// 参见 https://docs.microsoft.com/windows/win32/api/heapapi/nf-heapapi-getprocessheap
windows_targets::link!("kernel32.dll" "system" fn GetProcessHeap() -> c::HANDLE);

// 从给定的堆 `hHeap` 中分配一块 `dwBytes` 字节的内存。
// 所分配的内存可能是未初始化的；若 `dwFlags` 设为
// `HEAP_ZERO_MEMORY`，则会被清零。
//
// 返回指向新分配内存的指针；若操作失败则返回 null。
// 返回的指针至少对齐到 `MIN_ALIGN`。
//
// SAFETY:
//  - `hHeap` 必须是由 `GetProcessHeap` 返回的非 null 句柄。
//  - `dwFlags` 必须设为 0 或 `HEAP_ZERO_MEMORY`。
//
// 注意，与某些其他分配器不同，这里允许 `dwBytes` 为 0。
//
// 参见 https://docs.microsoft.com/windows/win32/api/heapapi/nf-heapapi-heapalloc
windows_targets::link!("kernel32.dll" "system" fn HeapAlloc(hheap: c::HANDLE, dwflags: u32, dwbytes: usize) -> *mut c_void);

// 将给定指针 `lpMem` 背后、属于给定堆 `hHeap` 的一块内存重新分配为
// 至少 `dwBytes` 字节的块：要么原地收缩该块，
// 要么在新位置分配、复制内存，并释放原位置。
//
// 返回指向重新分配后内存的指针；若操作失败则返回 null。
// 返回的指针至少对齐到 `MIN_ALIGN`。
// 若操作失败，则给定的块绝不会被释放。
//
// SAFETY:
//  - `hHeap` 必须是由 `GetProcessHeap` 返回的非 null 句柄。
//  - `dwFlags` 必须设为 0。
//  - `lpMem` 必须是一个非 null 指针，指向由 `HeapAlloc` 或
//     `HeapReAlloc` 返回、且尚未被释放的已分配块。
// 如果该块被成功地重新分配到了新位置，那么指向已释放内存的指针
// （例如 `lpMem`）就绝不能再被解引用。
//
// 注意，与某些其他分配器不同，这里允许 `dwBytes` 为 0。
//
// 参见 https://docs.microsoft.com/windows/win32/api/heapapi/nf-heapapi-heaprealloc
windows_targets::link!("kernel32.dll" "system" fn HeapReAlloc(
    hheap: c::HANDLE,
    dwflags : u32,
    lpmem: *const c_void,
    dwbytes: usize
) -> *mut c_void);

// 从给定堆 `hHeap` 中释放给定指针 `lpMem` 背后的一块内存。
// 操作成功返回非零值，失败返回零。
//
// SAFETY:
//  - `hHeap` 必须是由 `GetProcessHeap` 返回的非 null 句柄。
//  - `dwFlags` 必须设为 0。
//  - `lpMem` 必须是指向由 `HeapAlloc` 或 `HeapReAlloc` 返回、
//     且尚未被释放的已分配块的指针。
// 如果该块被成功释放，那么指向已释放内存的指针（例如 `lpMem`）
// 就绝不能再被解引用。
//
// 注意，允许 `lpMem` 为 null，这并不会导致操作失败。
//
// 参见 https://docs.microsoft.com/windows/win32/api/heapapi/nf-heapapi-heapfree
windows_targets::link!("kernel32.dll" "system" fn HeapFree(hheap: c::HANDLE, dwflags: u32, lpmem: *const c_void) -> c::BOOL);

fn get_process_heap() -> *mut c_void {
    // SAFETY: GetProcessHeap 只是返回一个有效句柄或 NULL，因此调用它总是安全的。
    unsafe { GetProcessHeap() }
}

#[inline(never)]
fn process_heap_alloc(
    _heap: MaybeUninit<c::HANDLE>, // 传入此参数是为了匹配 `HeapAlloc` 的 ABI，
    flags: u32,
    bytes: usize,
) -> *mut c_void {
    let heap = get_process_heap();
    if core::intrinsics::unlikely(heap.is_null()) {
        return ptr::null_mut();
    }
    // SAFETY: `heap` 是由 `GetProcessHeap` 返回的非 null 句柄。
    unsafe { HeapAlloc(heap, flags, bytes) }
}

// 包含指向已分配块起始位置的指针的头部（header）。
// SAFETY: 其大小和对齐必须 <= `MIN_ALIGN`。
#[repr(C)]
struct Header(*mut u8);

// 为给定的 `layout` 分配一块内存，可选择是否清零。
// SAFETY: 返回一个满足 `System` 对已分配指针所作保证的指针，
// 或在操作失败时返回 null。若本函数返回非 null，则 `HEAP` 已被成功
// 初始化。
#[inline]
unsafe fn allocate(layout: Layout, zeroed: bool) -> *mut u8 {
    // 分配的内存要么被清零，要么是未初始化的。
    let flags = if zeroed { HEAP_ZERO_MEMORY } else { 0 };

    if layout.align() <= MIN_ALIGN {
        // 返回的指针指向已分配块的起始位置。
        process_heap_alloc(MaybeUninit::uninit(), flags, layout.size()) as *mut u8
    } else {
        // 额外分配一些填充（padding），以便能够满足对齐要求。
        let total = layout.align() + layout.size();

        let ptr = process_heap_alloc(MaybeUninit::uninit(), flags, total) as *mut u8;
        if ptr.is_null() {
            // 分配失败。
            return ptr::null_mut();
        }

        // 在已分配块起始处的偏移位置上构造一个正确对齐的指针，
        // 并在它之前写入一个头部。

        let offset = layout.align() - (ptr.addr() & (layout.align() - 1));
        // SAFETY: `MIN_ALIGN` <= `offset` <= `layout.align()`，且所分配块的大小为
        // `layout.align() + layout.size()`。因此 `aligned` 将是位于已分配块内部、
        // 正确对齐的指针，其后至少有 `layout.size()` 字节、其前至少有
        // `MIN_ALIGN` 字节的填充。
        let aligned = unsafe { ptr.add(offset) };
        // SAFETY: 由于头部的大小和对齐 <= `MIN_ALIGN`，而 `aligned`
        // 至少对齐到 `MIN_ALIGN` 且其前至少有 `MIN_ALIGN` 字节的填充，
        // 因此在它正前方写入一个头部是安全的。
        unsafe { ptr::write((aligned as *mut Header).sub(1), Header(ptr)) };

        // SAFETY: 返回的指针并不指向已分配块的起始位置，
        // 但在它正前方可以读到一个头部，其中存有该块起始位置的地址。
        aligned
    }
}

// 除了 `GlobalAlloc` 的各项保证之外，本分配器返回的所有指针还具有
// 以下性质：
//
// 如果该指针是用对齐 <= `MIN_ALIGN` 的 `layout` 分配或重新分配的，
// 那么它将至少对齐到 `MIN_ALIGN`，并指向已分配块的起始位置。
//
// 如果该指针是用对齐 > `MIN_ALIGN` 的 `layout` 分配或重新分配的，
// 那么它将对齐到所指定的对齐值，且不指向已分配块的起始位置。
// 而是在返回指针的正前方可读到一个头部，其中存有该块起始位置的实际地址。
#[stable(feature = "alloc_system_type", since = "1.28.0")]
unsafe impl GlobalAlloc for System {
    #[inline]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: `allocate` 返回的指针满足 `System` 的各项保证
        let zeroed = false;
        unsafe { allocate(layout, zeroed) }
    }

    #[inline]
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: `allocate` 返回的指针满足 `System` 的各项保证
        let zeroed = true;
        unsafe { allocate(layout, zeroed) }
    }

    #[inline]
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let block = {
            if layout.align() <= MIN_ALIGN {
                ptr
            } else {
                // 块起始位置的地址存放在 `ptr` 之前的填充里。

                // SAFETY: 根据 `System` 的契约，`ptr` 保证非 null，
                // 且在它正前方可读到一个头部。
                unsafe { ptr::read((ptr as *mut Header).sub(1)).0 }
            }
        };

        // 因为 `ptr` 已通过本分配器成功分配，
        // 所以必然存在一个有效的进程堆。
        let heap = get_process_heap();

        // SAFETY: `heap` 是由 `GetProcessHeap` 返回的非 null 句柄，
        // `block` 是指向已分配块起始位置的指针。
        unsafe { HeapFree(heap, 0, block.cast::<c_void>()) };
    }

    #[inline]
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if layout.align() <= MIN_ALIGN {
            // 因为 `ptr` 已通过本分配器成功分配，
            // 所以必然存在一个有效的进程堆。
            let heap = get_process_heap();

            // SAFETY: `heap` 是由 `GetProcessHeap` 返回的非 null 句柄，
            // `ptr` 是指向已分配块起始位置的指针。
            // 返回的指针指向已分配块的起始位置。
            unsafe { HeapReAlloc(heap, 0, ptr.cast::<c_void>(), new_size).cast::<u8>() }
        } else {
            // SAFETY: `realloc_fallback` 是用 `dealloc` 和 `alloc` 实现的，它们会
            // 正确处理 `ptr`，并返回满足 `System` 各项保证的指针
            unsafe { realloc_fallback(self, ptr, layout, new_size) }
        }
    }
}
