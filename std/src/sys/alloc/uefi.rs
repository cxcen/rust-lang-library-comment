//! UEFI 的全局分配器（Global Allocator）。
//! 使用 [r-efi-alloc](https://crates.io/crates/r-efi-alloc)

use r_efi::protocols::loaded_image;

use crate::alloc::{GlobalAlloc, Layout, System};
use crate::sync::OnceLock;
use crate::sys::pal::helpers;

#[stable(feature = "alloc_system_type", since = "1.28.0")]
unsafe impl GlobalAlloc for System {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        static EFI_MEMORY_TYPE: OnceLock<u32> = OnceLock::new();

        // 如果 boot services 不可用则返回空指针
        if crate::os::uefi::env::boot_services().is_none() {
            return crate::ptr::null_mut();
        }

        // 如果 boot services 有效，那么 SystemTable 就不为 null。
        let system_table = crate::os::uefi::env::system_table().as_ptr().cast();

        // 每个被加载的映像（loaded image）都有一个支持 `EFI_LOADED_IMAGE_PROTOCOL`
        // 的映像句柄。因此这绝不会失败。
        let mem_type = EFI_MEMORY_TYPE.get_or_init(|| {
            let protocol = helpers::image_handle_protocol::<loaded_image::Protocol>(
                loaded_image::PROTOCOL_GUID,
            )
            .unwrap();
            // 让分配得到的内存类型与数据段被加载时所用的内存类型一致。
            unsafe { (*protocol.as_ptr()).image_data_type }
        });

        // 调用者必须确保 layout 非 0
        unsafe { r_efi_alloc::raw::alloc(system_table, layout, *mem_type) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // 如果 boot services 不可用则什么也不做
        if crate::os::uefi::env::boot_services().is_none() {
            return;
        }

        // 如果 boot services 有效，那么 SystemTable 就不为 null。
        let system_table = crate::os::uefi::env::system_table().as_ptr().cast();
        // 调用者必须确保 layout 非 0
        unsafe { r_efi_alloc::raw::dealloc(system_table, ptr, layout) }
    }
}
