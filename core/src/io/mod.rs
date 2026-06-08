//! `core` I/O 功能的 trait、辅助工具和类型定义。

mod borrowed_buf;

#[unstable(feature = "core_io_borrowed_buf", issue = "117693")]
pub use self::borrowed_buf::{BorrowedBuf, BorrowedCursor};
