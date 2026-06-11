use crate::fmt;
use crate::io::buffered::LineWriterShim;
use crate::io::{self, BufWriter, IntoInnerError, IoSlice, Write};

/// 包装一个 writer 并对写向它的输出进行缓冲，每当检测到换行符（`0x0a`，即 `'\n'`）时就刷新。
///
/// [`BufWriter`] 结构体会包装一个 writer 并对其输出进行缓冲。但它只在自身离开作用域、或内部
/// 缓冲被填满时，才执行这种成批写出。有时你会更希望每完成一行就写出一行，而不是一次性写出整个
/// 缓冲。`LineWriter` 应运而生，它做的正是这件事。
///
/// 与 [`BufWriter`] 类似，`LineWriter` 的缓冲在 `LineWriter` 离开作用域、或其内部缓冲被
/// 填满时也会被刷新。
///
/// 如果 `LineWriter` 被 drop 时缓冲里还残留着一行的一部分（尚未遇到换行符），它会把这些内容
/// 刷新出去。
///
/// # 示例
///
/// 我们可以用 `LineWriter` 来逐行写入，从而显著减少对文件的实际写入次数。
///
/// ```no_run
/// use std::fs::{self, File};
/// use std::io::prelude::*;
/// use std::io::LineWriter;
///
/// fn main() -> std::io::Result<()> {
///     let road_not_taken = b"I shall be telling this with a sigh
/// Somewhere ages and ages hence:
/// Two roads diverged in a wood, and I -
/// I took the one less traveled by,
/// And that has made all the difference.";
///
///     let file = File::create("poem.txt")?;
///     let mut file = LineWriter::new(file);
///
///     file.write_all(b"I shall be telling this with a sigh")?;
///
///     // 在遇到换行符（或内部缓冲被填满）之前，不会写出任何字节。
///     assert_eq!(fs::read_to_string("poem.txt")?, "");
///     file.write_all(b"\n")?;
///     assert_eq!(
///         fs::read_to_string("poem.txt")?,
///         "I shall be telling this with a sigh\n",
///     );
///
///     // 写出这首诗的其余部分。
///     file.write_all(b"Somewhere ages and ages hence:
/// Two roads diverged in a wood, and I -
/// I took the one less traveled by,
/// And that has made all the difference.")?;
///
///     // 这首诗的最后一行不以换行符结尾，所以我们必须 flush 或 drop 这个 `LineWriter`
///     // 才能完成写入。
///     file.flush()?;
///
///     // 确认整首诗都已写出。
///     assert_eq!(fs::read("poem.txt")?, &road_not_taken[..]);
///     Ok(())
/// }
/// ```
#[stable(feature = "rust1", since = "1.0.0")]
pub struct LineWriter<W: ?Sized + Write> {
    inner: BufWriter<W>,
}

impl<W: Write> LineWriter<W> {
    /// 创建一个新的 `LineWriter`。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs::File;
    /// use std::io::LineWriter;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let file = File::create("poem.txt")?;
    ///     let file = LineWriter::new(file);
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn new(inner: W) -> LineWriter<W> {
        // 一行通常不会太长，所以不要用一个巨大的缓冲
        LineWriter::with_capacity(1024, inner)
    }

    /// 创建一个新的 `LineWriter`，其内部缓冲容量至少为指定的大小。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs::File;
    /// use std::io::LineWriter;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let file = File::create("poem.txt")?;
    ///     let file = LineWriter::with_capacity(100, file);
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn with_capacity(capacity: usize, inner: W) -> LineWriter<W> {
        LineWriter { inner: BufWriter::with_capacity(capacity, inner) }
    }

    /// 获取对底层 writer 的可变引用。
    ///
    /// 在所返回的可变引用上调用方法时必须谨慎，因为额外的写入可能会破坏输出流（打乱
    /// LineWriter 的行缓冲状态）。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs::File;
    /// use std::io::LineWriter;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let file = File::create("poem.txt")?;
    ///     let mut file = LineWriter::new(file);
    ///
    ///     // 我们可以像使用 file 一样使用这个引用
    ///     let reference = file.get_mut();
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn get_mut(&mut self) -> &mut W {
        self.inner.get_mut()
    }

    /// 拆开（unwrap）这个 `LineWriter`，返回其底层 writer。
    ///
    /// 在返回 writer 之前，内部缓冲会先被写出。
    ///
    /// # Errors
    ///
    /// 如果在刷新缓冲时发生错误，将返回一个 [`Err`]。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs::File;
    /// use std::io::LineWriter;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let file = File::create("poem.txt")?;
    ///
    ///     let writer: LineWriter<File> = LineWriter::new(file);
    ///
    ///     let file: File = writer.into_inner()?;
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn into_inner(self) -> Result<W, IntoInnerError<LineWriter<W>>> {
        self.inner.into_inner().map_err(|err| err.new_wrapped(|inner| LineWriter { inner }))
    }
}

impl<W: ?Sized + Write> LineWriter<W> {
    /// 获取对底层 writer 的不可变引用。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs::File;
    /// use std::io::LineWriter;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let file = File::create("poem.txt")?;
    ///     let file = LineWriter::new(file);
    ///
    ///     let reference = file.get_ref();
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn get_ref(&self) -> &W {
        self.inner.get_ref()
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<W: ?Sized + Write> Write for LineWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        LineWriterShim::new(&mut self.inner).write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }

    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        LineWriterShim::new(&mut self.inner).write_vectored(bufs)
    }

    fn is_write_vectored(&self) -> bool {
        self.inner.is_write_vectored()
    }

    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        LineWriterShim::new(&mut self.inner).write_all(buf)
    }

    fn write_all_vectored(&mut self, bufs: &mut [IoSlice<'_>]) -> io::Result<()> {
        LineWriterShim::new(&mut self.inner).write_all_vectored(bufs)
    }

    fn write_fmt(&mut self, fmt: fmt::Arguments<'_>) -> io::Result<()> {
        LineWriterShim::new(&mut self.inner).write_fmt(fmt)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<W: ?Sized + Write> fmt::Debug for LineWriter<W>
where
    W: fmt::Debug,
{
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("LineWriter")
            .field("writer", &self.get_ref())
            .field(
                "buffer",
                &format_args!("{}/{}", self.inner.buffer().len(), self.inner.capacity()),
            )
            .finish_non_exhaustive()
    }
}
