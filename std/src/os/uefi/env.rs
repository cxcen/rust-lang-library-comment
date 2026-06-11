//! 针对 `std::env` 模块中各类基础类型的 UEFI 平台特定扩展

#![unstable(feature = "uefi_std", issue = "100499")]

use crate::ffi::c_void;
use crate::ptr::NonNull;
use crate::sync::atomic::{Atomic, AtomicBool, AtomicPtr, Ordering};

static SYSTEM_TABLE: Atomic<*mut c_void> = AtomicPtr::new(crate::ptr::null_mut());
static IMAGE_HANDLE: Atomic<*mut c_void> = AtomicPtr::new(crate::ptr::null_mut());
// 用于检查 BootServices 是否仍然有效的标志。
// 初始假设它们不可用
static BOOT_SERVICES_FLAG: Atomic<bool> = AtomicBool::new(false);

/// 初始化全局的 System Table 与 Image Handle 指针。
///
/// 标准库的运作需要访问 UEFI System Table 和应用程序的 Image Handle。这些会在 UEFI
/// 应用程序的入口点处提供给它们。通过调用 `init_globals()`，标准库会保留这些指针以供
/// 将来使用。因此本函数必须在使用任何标准库服务之前被调用。
///
/// 这些指针绝不会暴露给本应用程序之外的任何实体，并且保证一旦应用程序退出，这些指针
/// 便绝不会再被解引用。
///
/// 调用方需要确保这些指针在本应用程序的整个生命周期内都有效。特别地，当一个使用了标准库
/// 的应用程序处于加载状态时，UEFI Boot Services 不得被退出。
///
/// # 安全性(SAFETY）
/// 多次调用本函数将会 panic。
pub(crate) unsafe fn init_globals(handle: NonNull<c_void>, system_table: NonNull<c_void>) {
    IMAGE_HANDLE
        .compare_exchange(
            crate::ptr::null_mut(),
            handle.as_ptr(),
            Ordering::Release,
            Ordering::Acquire,
        )
        .unwrap();
    SYSTEM_TABLE
        .compare_exchange(
            crate::ptr::null_mut(),
            system_table.as_ptr(),
            Ordering::Release,
            Ordering::Acquire,
        )
        .unwrap();
    BOOT_SERVICES_FLAG.store(true, Ordering::Release)
}

/// 获取 SystemTable 指针。
///
/// 如果你想使用 `BootServices`，请改用 [`boot_services`]，因为它会执行一些额外的检查。
///
/// 注意：如果 System Table 或 Image Handle 尚未初始化，本函数将会 panic。
pub fn system_table() -> NonNull<c_void> {
    try_system_table().unwrap()
}

/// 获取 ImageHandle 指针。
///
/// 注意：如果 System Table 或 Image Handle 尚未初始化，本函数将会 panic。
pub fn image_handle() -> NonNull<c_void> {
    try_image_handle().unwrap()
}

/// 获取 BootServices 指针。
///
/// 本函数还会检查 `ExitBootServices` 是否已经被调用过。
pub fn boot_services() -> Option<NonNull<c_void>> {
    if BOOT_SERVICES_FLAG.load(Ordering::Acquire) {
        let system_table: NonNull<r_efi::efi::SystemTable> = try_system_table()?.cast();
        let boot_services = unsafe { (*system_table.as_ptr()).boot_services };
        NonNull::new(boot_services).map(|x| x.cast())
    } else {
        None
    }
}

/// 获取 SystemTable 指针。
///
/// 本函数主要用于那些不允许 panic 的场景。
pub(crate) fn try_system_table() -> Option<NonNull<c_void>> {
    NonNull::new(SYSTEM_TABLE.load(Ordering::Acquire))
}

/// 获取 SystemHandle 指针。
///
/// 本函数主要用于那些不允许 panic 的场景。
pub(crate) fn try_image_handle() -> Option<NonNull<c_void>> {
    NonNull::new(IMAGE_HANDLE.load(Ordering::Acquire))
}

pub(crate) fn disable_boot_services() {
    BOOT_SERVICES_FLAG.store(false, Ordering::Release)
}
