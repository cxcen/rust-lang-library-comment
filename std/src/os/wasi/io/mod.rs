//! 针对通用 I/O 基础类型的 WASI 平台特定扩展。

#![stable(feature = "io_safety_wasi", since = "1.65.0")]

#[stable(feature = "io_safety_wasi", since = "1.65.0")]
pub use crate::os::fd::*;

// 本模块的测试
#[cfg(test)]
mod tests;
