//! 为 I/O traits 提供缓冲功能的包装器
//!
//! 本模块汇集了标准库中与缓冲相关的几个核心类型：
//! - [`BufReader`]：为实现了 `Read` 的类型添加输入缓冲，减少底层系统调用次数。
//! - [`BufWriter`]：为实现了 `Write` 的类型添加输出缓冲，把多次小写入合并为少量大写入。
//! - [`LineWriter`]：基于行缓冲的写入器，遇到换行符时才把数据刷新到底层。
//!
//! 这些包装器的共同目的，是减少与底层（文件、socket 等）实际交互的次数，从而提升性能。

mod bufreader;
mod bufwriter;
mod linewriter;
mod linewritershim;

#[cfg(test)]
mod tests;

#[stable(feature = "bufwriter_into_parts", since = "1.56.0")]
pub use bufwriter::WriterPanicked;
use linewritershim::LineWriterShim;

#[stable(feature = "rust1", since = "1.0.0")]
pub use self::{bufreader::BufReader, bufwriter::BufWriter, linewriter::LineWriter};
use crate::io::Error;
use crate::{error, fmt};

/// [`BufWriter::into_inner`] 返回的错误类型，它把两部分组合在一起：写出缓冲时发生的
/// 错误，以及那个缓冲写入器对象本身（可用于从该错误状态中恢复）。
///
/// 也就是说，当你想从 `BufWriter` 中取回底层写入器、但刷新缓冲失败时，并不会直接丢失
/// 缓冲写入器；它会连同错误一起被打包进 `IntoInnerError`，从而给你机会检查缓冲内容、
/// 重试或做其他恢复处理。
///
/// # 示例
///
/// ```no_run
/// use std::io::BufWriter;
/// use std::net::TcpStream;
///
/// let mut stream = BufWriter::new(TcpStream::connect("127.0.0.1:34254").unwrap());
///
/// // 对 stream 做一些操作
///
/// // 我们想取回自己的 `TcpStream`，于是尝试：
///
/// let stream = match stream.into_inner() {
///     Ok(s) => s,
///     Err(e) => {
///         // 这里的 e 就是一个 IntoInnerError
///         panic!("An error occurred");
///     }
/// };
/// ```
#[derive(Debug)]
#[stable(feature = "rust1", since = "1.0.0")]
pub struct IntoInnerError<W>(W, Error);

impl<W> IntoInnerError<W> {
    /// 构造一个新的 IntoInnerError
    fn new(writer: W, error: Error) -> Self {
        Self(writer, error)
    }

    /// 用于构造新的 IntoInnerError 的辅助方法；其目的是帮助那些包装了其他适配器的
    /// 适配器（即一层套一层的写入器）做错误转换。
    fn new_wrapped<W2>(self, f: impl FnOnce(W) -> W2) -> IntoInnerError<W2> {
        let Self(writer, error) = self;
        IntoInnerError::new(f(writer), error)
    }

    /// 返回导致 [`BufWriter::into_inner()`] 调用失败的那个错误。
    ///
    /// 该错误是在尝试写出内部缓冲时返回的。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::io::BufWriter;
    /// use std::net::TcpStream;
    ///
    /// let mut stream = BufWriter::new(TcpStream::connect("127.0.0.1:34254").unwrap());
    ///
    /// // 对 stream 做一些操作
    ///
    /// // 我们想取回自己的 `TcpStream`，于是尝试：
    ///
    /// let stream = match stream.into_inner() {
    ///     Ok(s) => s,
    ///     Err(e) => {
    ///         // 这里的 e 是一个 IntoInnerError，我们把内部的错误记录下来。
    ///         //
    ///         // 在本例中我们只是简单地“记录”到 stdout。
    ///         println!("{}", e.error());
    ///
    ///         panic!("An unexpected error occurred.");
    ///     }
    /// };
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn error(&self) -> &Error {
        &self.1
    }

    /// 返回产生该错误的缓冲写入器实例。
    ///
    /// 返回的对象可用于错误恢复，例如重新检查缓冲中的内容。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::io::BufWriter;
    /// use std::net::TcpStream;
    ///
    /// let mut stream = BufWriter::new(TcpStream::connect("127.0.0.1:34254").unwrap());
    ///
    /// // 对 stream 做一些操作
    ///
    /// // 我们想取回自己的 `TcpStream`，于是尝试：
    ///
    /// let stream = match stream.into_inner() {
    ///     Ok(s) => s,
    ///     Err(e) => {
    ///         // 这里的 e 是一个 IntoInnerError，我们重新检查一下缓冲：
    ///         let buffer = e.into_inner();
    ///
    ///         // 做一些尝试恢复的操作
    ///
    ///         // 之后，我们直接把 stream 返回
    ///         buffer.into_inner().unwrap()
    ///     }
    /// };
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn into_inner(self) -> W {
        self.0
    }

    /// 消耗 [`IntoInnerError`] 并返回导致 [`BufWriter::into_inner()`] 调用失败的错误。
    /// 与 `error` 不同，这个方法可以让你取得底层错误的所有权。
    ///
    /// # 示例
    /// ```
    /// use std::io::{BufWriter, ErrorKind, Write};
    ///
    /// let mut not_enough_space = [0u8; 10];
    /// let mut stream = BufWriter::new(not_enough_space.as_mut());
    /// write!(stream, "this cannot be actually written").unwrap();
    /// let into_inner_err = stream.into_inner().expect_err("now we discover it's too small");
    /// let err = into_inner_err.into_error();
    /// assert_eq!(err.kind(), ErrorKind::WriteZero);
    /// ```
    #[stable(feature = "io_into_inner_error_parts", since = "1.55.0")]
    pub fn into_error(self) -> Error {
        self.1
    }

    /// 消耗 [`IntoInnerError`] 并同时返回导致 [`BufWriter::into_inner()`] 调用失败的
    /// 错误，以及底层的写入器。
    ///
    /// 这个方法既可用于单纯地取得底层错误的所有权，也可用于高级的错误恢复场景。
    ///
    /// # 示例
    /// ```
    /// use std::io::{BufWriter, ErrorKind, Write};
    ///
    /// let mut not_enough_space = [0u8; 10];
    /// let mut stream = BufWriter::new(not_enough_space.as_mut());
    /// write!(stream, "this cannot be actually written").unwrap();
    /// let into_inner_err = stream.into_inner().expect_err("now we discover it's too small");
    /// let (err, recovered_writer) = into_inner_err.into_parts();
    /// assert_eq!(err.kind(), ErrorKind::WriteZero);
    /// assert_eq!(recovered_writer.buffer(), b"t be actually written");
    /// ```
    #[stable(feature = "io_into_inner_error_parts", since = "1.55.0")]
    pub fn into_parts(self) -> (Error, W) {
        (self.1, self.0)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<W> From<IntoInnerError<W>> for Error {
    fn from(iie: IntoInnerError<W>) -> Error {
        iie.1
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<W: Send + fmt::Debug> error::Error for IntoInnerError<W> {}

#[stable(feature = "rust1", since = "1.0.0")]
impl<W> fmt::Display for IntoInnerError<W> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error().fmt(f)
    }
}
