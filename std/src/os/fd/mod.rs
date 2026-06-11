//! 拥有所有权的、以及借用的类 Unix 文件描述符。
//!
//! 本模块在 Unix 平台与 WASI 上受支持，二者都使用类似的文件描述符体系来
//! 引用操作系统资源。

#![stable(feature = "os_fd", since = "1.66.0")]
#![deny(unsafe_op_in_unsafe_fn)]

// `RawFd`、`AsRawFd` 等。
mod raw;

// `OwnedFd`、`AsFd` 等。
mod owned;

// 为各种网络类型实现 `AsRawFd` 等 trait。
#[cfg(not(target_os = "trusty"))]
mod net;

#[cfg(test)]
mod tests;

// 导出类型与 trait 以构成公开 API。
#[stable(feature = "os_fd", since = "1.66.0")]
pub use owned::*;
#[stable(feature = "os_fd", since = "1.66.0")]
pub use raw::*;
