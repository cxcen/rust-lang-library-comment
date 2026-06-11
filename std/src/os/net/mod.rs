//! 操作系统相关的网络功能。

// 参见 `library/std/src/os/mod.rs` 中的 cfg 宏，了解为何在 rustdoc 生成过程中
// 必须对这些平台进行特殊处理。
#[cfg(not(all(
    doc,
    any(
        all(target_arch = "wasm32", not(target_os = "wasi")),
        all(target_vendor = "fortanix", target_env = "sgx")
    )
)))]
#[cfg(any(target_os = "linux", target_os = "android", target_os = "cygwin", doc))]
pub(super) mod linux_ext;
