//! 针对 UEFI 平台、对 `std` 的平台特定扩展。
//!
//! 提供对 UEFI 平台上平台级信息的访问，并暴露一些 UEFI 特定的函数——这些
//! 函数若放进核心 `std` 库则并不合适。
//!
//! 它暴露了更多处理平台特定字符串（[`OsStr`]、[`OsString`]）的方式，允许更
//! 细粒度地设置权限，从文件和套接字中提取底层文件描述符，并带有用于派生进程
//! 的平台特定辅助工具。
//!
//! [`OsStr`]: crate::ffi::OsStr
//! [`OsString`]: crate::ffi::OsString
#![forbid(unsafe_op_in_unsafe_fn)]

pub mod helpers;
pub mod os;
pub mod time;

#[cfg(test)]
mod tests;

use crate::io;
use crate::os::uefi;
use crate::ptr::NonNull;
use crate::sync::atomic::{Atomic, AtomicPtr, Ordering};

static EXIT_BOOT_SERVICE_EVENT: Atomic<*mut crate::ffi::c_void> =
    AtomicPtr::new(crate::ptr::null_mut());

/// # 安全性(Safety）
/// - 只能在运行时初始化期间调用一次。
/// - argc 必须为 2。
/// - argv 必须为 &[Handle, *mut SystemTable]。
pub(crate) unsafe fn init(argc: isize, argv: *const *const u8, _sigpipe: u8) {
    assert_eq!(argc, 2);
    let image_handle = unsafe { NonNull::new(*argv as *mut crate::ffi::c_void).unwrap() };
    let system_table = unsafe { NonNull::new(*argv.add(1) as *mut crate::ffi::c_void).unwrap() };
    unsafe { uefi::env::init_globals(image_handle, system_table) };

    // 注册退出 boot services 的处理器
    match helpers::OwnedEvent::new(
        r_efi::efi::EVT_SIGNAL_EXIT_BOOT_SERVICES,
        r_efi::efi::TPL_NOTIFY,
        Some(exit_boot_service_handler),
        None,
    ) {
        Ok(x) => {
            if EXIT_BOOT_SERVICE_EVENT
                .compare_exchange(
                    crate::ptr::null_mut(),
                    x.into_raw(),
                    Ordering::Release,
                    Ordering::Acquire,
                )
                .is_err()
            {
                abort_internal();
            };
        }
        Err(_) => abort_internal(),
    }
}

/// # 安全性(Safety）
/// 它不保证一定会运行，例如当程序中止（abort）时。
/// - 只能在运行时清理期间调用一次。
pub unsafe fn cleanup() {
    if let Some(exit_boot_service_event) =
        NonNull::new(EXIT_BOOT_SERVICE_EVENT.swap(crate::ptr::null_mut(), Ordering::Acquire))
    {
        let _ = unsafe { helpers::OwnedEvent::from_raw(exit_boot_service_event.as_ptr()) };
    }
}

#[inline]
pub const fn unsupported<T>() -> io::Result<T> {
    Err(unsupported_err())
}

#[inline]
pub const fn unsupported_err() -> io::Error {
    io::const_error!(io::ErrorKind::Unsupported, "operation not supported on UEFI")
}

pub fn abort_internal() -> ! {
    if let Some(exit_boot_service_event) =
        NonNull::new(EXIT_BOOT_SERVICE_EVENT.load(Ordering::Acquire))
    {
        let _ = unsafe { helpers::OwnedEvent::from_raw(exit_boot_service_event.as_ptr()) };
    }

    if let (Some(boot_services), Some(handle)) =
        (uefi::env::boot_services(), uefi::env::try_image_handle())
    {
        let boot_services: NonNull<r_efi::efi::BootServices> = boot_services.cast();
        let _ = unsafe {
            ((*boot_services.as_ptr()).exit)(
                handle.as_ptr(),
                r_efi::efi::Status::ABORTED,
                0,
                crate::ptr::null_mut(),
            )
        };
    }

    // 万一 SystemTable 和 ImageHandle 都无法访问，则使用 `core::intrinsics::abort`
    core::intrinsics::abort();
}

/// 如果 `EVT_SIGNAL_EXIT_BOOT_SERVICES` 被触发，则禁用对 BootServices 的访问
extern "efiapi" fn exit_boot_service_handler(_e: r_efi::efi::Event, _ctx: *mut crate::ffi::c_void) {
    uefi::env::disable_boot_services();
}
