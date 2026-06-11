//! 定义用于 Objective-C 互操作（interoperability）的类型与宏。
//!
//! 本模块重导出 [`core::os::darwin::objc`] 中的所有项。
//!
//! [`core::os::darwin::objc`]: ../../../../core/os/darwin/objc/index.html "mod core::os::darwin::objc"

#![unstable(feature = "darwin_objc", issue = "145496")]

// 我们无法为此自动生成 intra-doc 链接，因为 `core::os::darwin` 并非在每个平台上
// 都会被编译进 `core`，尽管它在每个平台上都有文档。
// 我们改为在上方的模块文档中直接链接到它。
#[cfg(not(doc))]
pub use core::os::darwin::objc::*;
