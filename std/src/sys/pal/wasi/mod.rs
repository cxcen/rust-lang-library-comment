//! WASI 平台的系统绑定（System bindings）。
//!
//! 本模块包含 WASI 平台上 OS 级功能的门面（facade，即平台特定）实现。
//! 目前这同时涵盖 WASIp1 与 WASIp2。

#[allow(unused)]
#[path = "../wasm/atomics/futex.rs"]
pub mod futex;

pub mod os;
pub mod stack_overflow;
#[path = "../unix/time.rs"]
pub mod time;

#[path = "../unsupported/common.rs"]
#[deny(unsafe_op_in_unsafe_fn)]
#[allow(unused)]
mod common;

pub use common::*;

mod helpers;

// 下面这些导出之所以逐个单独列出，是为了规避 Rust 的 glob 导入冲突规则。
// 如果我们把 `helpers` 和 `common` 一起 glob 导出，
// 编译器就会抱怨存在冲突。

pub(crate) use helpers::abort_internal;
#[cfg(target_env = "p1")]
pub(crate) use helpers::err2io;
#[cfg(not(target_env = "p1"))]
pub use os::IsMinusOne;
pub use os::{cvt, cvt_r};

#[cfg(not(target_env = "p1"))]
mod cabi_realloc;
