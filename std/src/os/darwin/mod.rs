//! 针对 Darwin / Apple 平台的 `std` 平台特有扩展。
//!
//! 本模块在以下操作系统上可用：
//! - macOS
//! - iOS
//! - tvOS
//! - watchOS
//! - visionOS
//!
//! 注意：本模块之所以叫 “Darwin”，是因为它是上述操作系统底层核心 OS 的名称，
//! 但不应与 `x86_64-apple-darwin` 和 `aarch64-apple-darwin` 目标名称中的
//! `-darwin` 后缀相混淆——后者那样命名多半是出于历史遗留原因。

#![stable(feature = "os_darwin", since = "1.84.0")]
#![doc(cfg(target_vendor = "apple"))]

pub mod fs;
pub mod objc;

// 已弃用（deprecated），但用于在 `std::os::unix::raw` 下进行公开重导出，
// 以及 `std::os::macos`/`std::os::ios`，因为这些模块的出现早于移除这些定义的决定。
pub(super) mod raw;
