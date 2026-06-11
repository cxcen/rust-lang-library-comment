//! 运行时查询操作系统/平台的版本。
//!
//! 与 [RFC 3750](https://github.com/rust-lang/rfcs/pull/3750) 相关，该 RFC
//! 在编译期进行版本探测。
//!
//! 另见 `os_info` crate。

#[cfg(target_vendor = "apple")]
mod darwin;

// 未来我们可以扩展这个模块，例如加入：
// - Windows 上的 `RtlGetVersion`。
// - Android 上的 `__system_property_get`。
