#![allow(unsafe_op_in_unsafe_fn)]

mod alloc;
mod configure_builtins;
mod helpers;
mod pal;
mod personality;

pub mod args;
pub mod backtrace;
pub mod cmath;
pub mod env;
pub mod env_consts;
pub mod exit_guard;
pub mod fd;
pub mod fs;
pub mod io;
pub mod net;
pub mod os_str;
pub mod path;
pub mod pipe;
pub mod platform_version;
pub mod process;
pub mod random;
pub mod stdio;
pub mod sync;
pub mod thread;
pub mod thread_local;

// FIXME(117276): 移除这一行，把各特性实现迁移到各自独立的
//                子模块中。
pub use pal::*;

/// 用于查看 std 类型内部表示的 trait（以共享引用方式借出内部表示）。
#[cfg_attr(not(target_os = "linux"), allow(unused))]
pub(crate) trait AsInner<Inner: ?Sized> {
    fn as_inner(&self) -> &Inner;
}

/// 用于查看 std 类型内部表示的 trait（以可变引用方式借出内部表示）。
#[cfg_attr(not(target_os = "linux"), allow(unused))]
pub(crate) trait AsInnerMut<Inner: ?Sized> {
    fn as_inner_mut(&mut self) -> &mut Inner;
}

/// 用于从 std 类型中提取（按值取出）内部表示的 trait。
pub(crate) trait IntoInner<Inner> {
    fn into_inner(self) -> Inner;
}

/// 用于从内部表示构造 std 类型的 trait。
pub(crate) trait FromInner<Inner> {
    fn from_inner(inner: Inner) -> Self;
}
