//! I/O 预导入模块（Prelude）。
//!
//! 本模块的目的是减轻众多常用 I/O trait 的导入负担：在以 I/O 为主的模块顶部
//! 添加一行 glob 导入即可：
//!
//! ```
//! # #![allow(unused_imports)]
//! use std::io::prelude::*;
//! ```

#![stable(feature = "rust1", since = "1.0.0")]

#[stable(feature = "rust1", since = "1.0.0")]
pub use super::{BufRead, Read, Seek, Write};
