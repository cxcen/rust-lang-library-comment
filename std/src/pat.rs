//! 辅助模块：用于把 `pattern_type` 宏重导出到 `std`。
//!
//! 该宏本身定义在 `core` 中，这里仅做转发，使其在 `std` 命名空间下可见。

pub use core::pattern_type;
