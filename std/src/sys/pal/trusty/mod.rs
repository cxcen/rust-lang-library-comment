//! Trusty OS 的系统绑定。

#[path = "../unsupported/common.rs"]
#[deny(unsafe_op_in_unsafe_fn)]
mod common;
#[path = "../unsupported/os.rs"]
pub mod os;
#[path = "../unsupported/time.rs"]
pub mod time;

pub use common::*;
