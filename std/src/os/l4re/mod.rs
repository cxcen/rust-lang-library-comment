//! L4Re 平台特有的定义。

#![stable(feature = "raw_ext", since = "1.1.0")]
#![doc(cfg(target_os = "l4re"))]

pub mod fs;
pub mod raw;
