//! `ByteStr` 和 `ByteString` 类型及其 trait 实现。
//!
//! 本模块本身不定义这两个类型，只是把它们从底层的 `alloc` crate 重导出到
//! `std` 命名空间。`ByteStr`（借用）与 `ByteString`（拥有所有权）用于表示
//! “大概率是 UTF-8、但不保证”的字节串：相比 `[u8]`/`Vec<u8>`，它们在
//! `Debug`/`Display` 等场景按字符串语义打印，又不像 `str`/`String` 那样强制
//! 要求合法 UTF-8。两者目前仍是 unstable，受 `feature = "bstr"` 控制。

#[unstable(feature = "bstr", issue = "134915")]
pub use alloc::bstr::{ByteStr, ByteString};
