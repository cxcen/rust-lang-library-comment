//! Darwin / Apple 平台的 `core` 平台特定扩展。
//!
//! 这适用于以下操作系统:
//! - macOS
//! - iOS
//! - tvOS
//! - watchOS
//! - visionOS
//!
//! 注意: 本模块称为 "Darwin"，因为这是上述操作系统底层核心操作系统的名称，
//! 但不应与 `x86_64-apple-darwin` 和 `aarch64-apple-darwin` 目标名中的
//! `-darwin` 后缀混淆；这些目标名大多是出于历史原因这样命名。

#![unstable(feature = "darwin_objc", issue = "145496")]
#![doc(cfg(target_vendor = "apple"))]

pub mod objc;
