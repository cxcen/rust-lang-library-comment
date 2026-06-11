//! 针对 Unix 平台、对 `std` 的平台特有扩展。
//!
//! 在 Unix 平台上提供对平台级信息的访问，并暴露那些若作为核心 `std` 库的一部分
//! 则并不合适的 Unix 特有函数。
//!
//! 它暴露了更多处理平台特有字符串（[`OsStr`]、[`OsString`]）的方式，允许更细粒度地
//! 设置权限、从文件与套接字中提取底层文件描述符，并提供了用于派生进程的平台特有辅助工具。
//!
//! # 示例
//!
//! ```no_run
//! use std::fs::File;
//! use std::os::unix::prelude::*;
//!
//! fn main() -> std::io::Result<()> {
//!     let f = File::create("foo.txt")?;
//!     let fd = f.as_raw_fd();
//!
//!     // 配合原生的 unix 绑定使用 fd
//!
//!     Ok(())
//! }
//! ```
//!
//! [`OsStr`]: crate::ffi::OsStr
//! [`OsString`]: crate::ffi::OsString

#![stable(feature = "rust1", since = "1.0.0")]
#![doc(cfg(unix))]

// 在诸如 Windows 等其他平台上生成文档时，使用 linux 作为默认平台
#[cfg(doc)]
use crate::os::linux as platform;

#[cfg(not(doc))]
mod platform {
    #[cfg(target_os = "aix")]
    pub use crate::os::aix::*;
    #[cfg(target_os = "android")]
    pub use crate::os::android::*;
    #[cfg(target_os = "cygwin")]
    pub use crate::os::cygwin::*;
    #[cfg(target_vendor = "apple")]
    pub use crate::os::darwin::*;
    #[cfg(target_os = "dragonfly")]
    pub use crate::os::dragonfly::*;
    #[cfg(target_os = "emscripten")]
    pub use crate::os::emscripten::*;
    #[cfg(target_os = "espidf")]
    pub use crate::os::espidf::*;
    #[cfg(target_os = "freebsd")]
    pub use crate::os::freebsd::*;
    #[cfg(target_os = "fuchsia")]
    pub use crate::os::fuchsia::*;
    #[cfg(target_os = "haiku")]
    pub use crate::os::haiku::*;
    #[cfg(target_os = "horizon")]
    pub use crate::os::horizon::*;
    #[cfg(target_os = "hurd")]
    pub use crate::os::hurd::*;
    #[cfg(target_os = "illumos")]
    pub use crate::os::illumos::*;
    #[cfg(target_os = "l4re")]
    pub use crate::os::l4re::*;
    #[cfg(target_os = "linux")]
    pub use crate::os::linux::*;
    #[cfg(target_os = "netbsd")]
    pub use crate::os::netbsd::*;
    #[cfg(target_os = "nto")]
    pub use crate::os::nto::*;
    #[cfg(target_os = "nuttx")]
    pub use crate::os::nuttx::*;
    #[cfg(target_os = "openbsd")]
    pub use crate::os::openbsd::*;
    #[cfg(target_os = "redox")]
    pub use crate::os::redox::*;
    #[cfg(target_os = "rtems")]
    pub use crate::os::rtems::*;
    #[cfg(target_os = "solaris")]
    pub use crate::os::solaris::*;
    #[cfg(target_os = "vita")]
    pub use crate::os::vita::*;
    #[cfg(target_os = "vxworks")]
    pub use crate::os::vxworks::*;
}

pub mod ffi;
pub mod fs;
pub mod io;
pub mod net;
pub mod process;
pub mod raw;
pub mod thread;

/// 一个 prelude，方便编写平台特有代码。
///
/// 包含所有扩展 trait，以及一些重要的类型定义。
#[stable(feature = "rust1", since = "1.0.0")]
pub mod prelude {
    #[doc(no_inline)]
    #[stable(feature = "rust1", since = "1.0.0")]
    pub use super::ffi::{OsStrExt, OsStringExt};
    #[doc(no_inline)]
    #[stable(feature = "rust1", since = "1.0.0")]
    pub use super::fs::DirEntryExt;
    #[doc(no_inline)]
    #[stable(feature = "file_offset", since = "1.15.0")]
    pub use super::fs::FileExt;
    #[doc(no_inline)]
    #[stable(feature = "rust1", since = "1.0.0")]
    pub use super::fs::{FileTypeExt, MetadataExt, OpenOptionsExt, PermissionsExt};
    #[doc(no_inline)]
    #[stable(feature = "rust1", since = "1.0.0")]
    pub use super::io::{AsFd, AsRawFd, BorrowedFd, FromRawFd, IntoRawFd, OwnedFd, RawFd};
    #[doc(no_inline)]
    #[unstable(feature = "unix_send_signal", issue = "141975")]
    pub use super::process::ChildExt;
    #[doc(no_inline)]
    #[stable(feature = "rust1", since = "1.0.0")]
    pub use super::process::{CommandExt, ExitStatusExt};
    #[doc(no_inline)]
    #[stable(feature = "rust1", since = "1.0.0")]
    pub use super::thread::JoinHandleExt;
}
