//! 针对 UEFI 的 `std` 平台特定扩展。

#![unstable(feature = "uefi_std", issue = "100499")]
#![doc(cfg(target_os = "uefi"))]
#![forbid(unsafe_op_in_unsafe_fn)]

pub mod env;
#[path = "../windows/ffi.rs"]
pub mod ffi;
