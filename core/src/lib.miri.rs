//! 想了解这个文件的来龙去脉,请 grep 搜索 `MIRI_REPLACE_LIBRS_IF_NOT_TEST`。
#![no_std]
extern crate core as realcore;
pub use realcore::*;
