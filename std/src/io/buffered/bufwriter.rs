use crate::io::{
    self, DEFAULT_BUF_SIZE, ErrorKind, IntoInnerError, IoSlice, Seek, SeekFrom, Write,
};
use crate::mem::{self, ManuallyDrop};
use crate::{error, fmt, ptr};

/// 包装一个 writer 并对其输出进行缓冲。
///
/// 直接操作一个实现了 [`Write`] 的类型可能非常低效。举例来说，对 [`TcpStream`] 的每一次
/// [`write`][`TcpStream::write`] 调用都会引发一次系统调用。而 `BufWriter<W>` 会把数据
/// 保存在一块内存缓冲里，再以少量的大批次写出到底层 writer。
///
/// 对于那些对同一个文件或网络 socket 反复进行 *小量* 写入的程序，`BufWriter<W>` 能提升
/// 速度。但如果你一次写入的数据量非常大，或只写入一两次，它就帮不上忙。此外，当写入目标本身
/// 就在内存中时（例如 <code>[Vec]\<u8></code>），它也没有任何优势。
///
/// 在 `BufWriter<W>` 被 drop 之前调用 [`flush`] 至关重要。尽管 drop 时会尝试刷新缓冲
/// 内容，但 drop 过程中发生的任何错误都会被忽略（即“flush-on-drop 时错误被吞”）。显式调用
/// [`flush`] 能确保缓冲为空，这样 drop 时甚至都不会去尝试任何文件操作——也就不存在错误被
/// 静默吞掉的隐患。因此，正确的做法是始终显式 flush，而不要依赖 drop 来刷新。
///
/// # 示例
///
/// 我们把数字 1 到 10 写入一个 [`TcpStream`]：
///
/// ```no_run
/// use std::io::prelude::*;
/// use std::net::TcpStream;
///
/// let mut stream = TcpStream::connect("127.0.0.1:34254").unwrap();
///
/// for i in 0..10 {
///     stream.write(&[i+1]).unwrap();
/// }
/// ```
///
/// 由于没有缓冲，我们是逐个写入的，每写一个字节都要承担一次系统调用的开销。我们可以用
/// `BufWriter<W>` 来解决这个问题：
///
/// ```no_run
/// use std::io::prelude::*;
/// use std::io::BufWriter;
/// use std::net::TcpStream;
///
/// let mut stream = BufWriter::new(TcpStream::connect("127.0.0.1:34254").unwrap());
///
/// for i in 0..10 {
///     stream.write(&[i+1]).unwrap();
/// }
/// stream.flush().unwrap();
/// ```
///
/// 通过用 `BufWriter<W>` 包装这个流，这十次写入会被缓冲合并到一起，并在 `stream` 被刷新时
/// 用一次系统调用全部写出。
///
/// [`TcpStream::write`]: crate::net::TcpStream::write
/// [`TcpStream`]: crate::net::TcpStream
/// [`flush`]: BufWriter::flush
#[stable(feature = "rust1", since = "1.0.0")]
pub struct BufWriter<W: ?Sized + Write> {
    // 缓冲本身。在常见代码路径中，不要把它当作普通的 `Vec` 来用。也就是说，不要用
    // `buf.push`、`buf.extend_from_slice` 或任何需要边界检查之类的方法。这对性能影响
    // 巨大（我们甚至可能想完全弃用 `Vec`）。
    buf: Vec<u8>,
    // #30888：如果内部 writer 在某次 write 调用中 panic，我们不希望在 BufWriter 的析构
    // 函数里把已缓冲的数据再写一遍。这个标志告诉 Drop 实现是否应跳过那次 flush。
    panicked: bool,
    inner: W,
}

impl<W: Write> BufWriter<W> {
    /// 用默认缓冲容量创建一个新的 `BufWriter<W>`。当前默认值是 8 KiB，但将来可能会变。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::io::BufWriter;
    /// use std::net::TcpStream;
    ///
    /// let mut buffer = BufWriter::new(TcpStream::connect("127.0.0.1:34254").unwrap());
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn new(inner: W) -> BufWriter<W> {
        BufWriter::with_capacity(DEFAULT_BUF_SIZE, inner)
    }

    pub(crate) fn try_new_buffer() -> io::Result<Vec<u8>> {
        Vec::try_with_capacity(DEFAULT_BUF_SIZE).map_err(|_| {
            io::const_error!(ErrorKind::OutOfMemory, "failed to allocate write buffer")
        })
    }

    pub(crate) fn with_buffer(inner: W, buf: Vec<u8>) -> Self {
        Self { inner, buf, panicked: false }
    }

    /// 创建一个新的 `BufWriter<W>`，其缓冲容量至少为指定的大小。
    ///
    /// # 示例
    ///
    /// 创建一个缓冲容量至少为一百字节的缓冲。
    ///
    /// ```no_run
    /// use std::io::BufWriter;
    /// use std::net::TcpStream;
    ///
    /// let stream = TcpStream::connect("127.0.0.1:34254").unwrap();
    /// let mut buffer = BufWriter::with_capacity(100, stream);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn with_capacity(capacity: usize, inner: W) -> BufWriter<W> {
        BufWriter { inner, buf: Vec::with_capacity(capacity), panicked: false }
    }

    /// 拆开（unwrap）这个 `BufWriter<W>`，返回其底层 writer。
    ///
    /// 在返回 writer 之前，缓冲会先被写出。
    ///
    /// # Errors
    ///
    /// 如果在刷新缓冲时发生错误，将返回一个 [`Err`]。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::io::BufWriter;
    /// use std::net::TcpStream;
    ///
    /// let mut buffer = BufWriter::new(TcpStream::connect("127.0.0.1:34254").unwrap());
    ///
    /// // 拆出 TcpStream 并刷新缓冲
    /// let stream = buffer.into_inner().unwrap();
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn into_inner(mut self) -> Result<W, IntoInnerError<BufWriter<W>>> {
        match self.flush_buf() {
            Err(e) => Err(IntoInnerError::new(self, e)),
            Ok(()) => Ok(self.into_parts().0),
        }
    }

    /// 拆解这个 `BufWriter<W>`，返回底层 writer 以及任何已缓冲但尚未写出的数据。
    ///
    /// 如果底层 writer 发生了 panic，则无从得知数据中有多少已被写出。在这种情况下，对于
    /// 已缓冲的数据我们返回 `WriterPanicked`（仍可从中取回缓冲内容）。
    ///
    /// `into_parts` 不会尝试刷新数据，也不会失败。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::io::{BufWriter, Write};
    ///
    /// let mut buffer = [0u8; 10];
    /// let mut stream = BufWriter::new(buffer.as_mut());
    /// write!(stream, "too much data").unwrap();
    /// stream.flush().expect_err("it doesn't fit");
    /// let (recovered_writer, buffered_data) = stream.into_parts();
    /// assert_eq!(recovered_writer.len(), 0);
    /// assert_eq!(&buffered_data.unwrap(), b"ata");
    /// ```
    #[stable(feature = "bufwriter_into_parts", since = "1.56.0")]
    pub fn into_parts(self) -> (W, Result<Vec<u8>, WriterPanicked>) {
        let mut this = ManuallyDrop::new(self);
        let buf = mem::take(&mut this.buf);
        let buf = if !this.panicked { Ok(buf) } else { Err(WriterPanicked { buf }) };

        // SAFETY: 把 `this` 放进一个永远不会被 drop 的 ManuallyDrop，从而防止重复 drop（double-drop）
        let inner = unsafe { ptr::read(&this.inner) };

        (inner, buf)
    }
}

impl<W: ?Sized + Write> BufWriter<W> {
    /// 把本地缓冲中的数据发送到内部 writer，必要时循环写入，直到全部发送完毕或发生错误。
    ///
    /// 因为缓冲中的所有数据都已经（通过 `write` 返回非零的成功值）向上层报告为“成功写入”，
    /// 所以来自 `inner` 的任何 0 长度写入都必须由本方法报告为 i/o 错误。
    pub(in crate::io) fn flush_buf(&mut self) -> io::Result<()> {
        /// 辅助结构体，用于确保在所有写入完成后更新缓冲。它跟踪已写出的字节数，并在被 drop
        /// 时把这些字节全部从缓冲前端排空（drain）。
        struct BufGuard<'a> {
            buffer: &'a mut Vec<u8>,
            written: usize,
        }

        impl<'a> BufGuard<'a> {
            fn new(buffer: &'a mut Vec<u8>) -> Self {
                Self { buffer, written: 0 }
            }

            /// 缓冲中尚未写出的部分
            fn remaining(&self) -> &[u8] {
                &self.buffer[self.written..]
            }

            /// 标记一些字节已从缓冲前端移除
            fn consume(&mut self, amt: usize) {
                self.written += amt;
            }

            /// 若所有字节都已写出，则返回 true
            fn done(&self) -> bool {
                self.written >= self.buffer.len()
            }
        }

        impl Drop for BufGuard<'_> {
            fn drop(&mut self) {
                if self.written > 0 {
                    self.buffer.drain(..self.written);
                }
            }
        }

        let mut guard = BufGuard::new(&mut self.buf);
        while !guard.done() {
            self.panicked = true;
            let r = self.inner.write(guard.remaining());
            self.panicked = false;

            match r {
                Ok(0) => {
                    return Err(io::const_error!(
                        ErrorKind::WriteZero,
                        "failed to write the buffered data",
                    ));
                }
                Ok(n) => guard.consume(n),
                Err(ref e) if e.is_interrupted() => {}
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    /// 缓冲一些数据但不刷新它，不论数据大小如何。在不超出容量的前提下尽可能多地写入。
    /// 返回写入的字节数。
    pub(super) fn write_to_buf(&mut self, buf: &[u8]) -> usize {
        let available = self.spare_capacity();
        let amt_to_buffer = available.min(buf.len());

        // SAFETY: 由构造方式可知，`amt_to_buffer` <= 缓冲的空闲容量。
        unsafe {
            self.write_to_buffer_unchecked(&buf[..amt_to_buffer]);
        }

        amt_to_buffer
    }

    /// 获取对底层 writer 的不可变引用。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::io::BufWriter;
    /// use std::net::TcpStream;
    ///
    /// let mut buffer = BufWriter::new(TcpStream::connect("127.0.0.1:34254").unwrap());
    ///
    /// // 我们可以像使用 buffer 一样使用这个引用
    /// let reference = buffer.get_ref();
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn get_ref(&self) -> &W {
        &self.inner
    }

    /// 获取对底层 writer 的可变引用。
    ///
    /// 不建议直接向底层 writer 写入。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::io::BufWriter;
    /// use std::net::TcpStream;
    ///
    /// let mut buffer = BufWriter::new(TcpStream::connect("127.0.0.1:34254").unwrap());
    ///
    /// // 我们可以像使用 buffer 一样使用这个引用
    /// let reference = buffer.get_mut();
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn get_mut(&mut self) -> &mut W {
        &mut self.inner
    }

    /// 返回对内部已缓冲数据的引用。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::io::BufWriter;
    /// use std::net::TcpStream;
    ///
    /// let buf_writer = BufWriter::new(TcpStream::connect("127.0.0.1:34254").unwrap());
    ///
    /// // 查看当前缓冲了多少字节
    /// let bytes_buffered = buf_writer.buffer().len();
    /// ```
    #[stable(feature = "bufreader_buffer", since = "1.37.0")]
    pub fn buffer(&self) -> &[u8] {
        &self.buf
    }

    /// 返回对内部缓冲的可变引用。
    ///
    /// 它可用于把数据直接写入缓冲，而不会触发向底层 writer 的写出。
    ///
    /// “缓冲是一个 `Vec`”这一点属于实现细节。调用方不应修改其容量，因为目前没有公开 API
    /// 可做此事，因此任何容量变化都会让用户始料未及。
    pub(in crate::io) fn buffer_mut(&mut self) -> &mut Vec<u8> {
        &mut self.buf
    }

    /// 返回内部缓冲在无需刷新的情况下还能容纳的字节数。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::io::BufWriter;
    /// use std::net::TcpStream;
    ///
    /// let buf_writer = BufWriter::new(TcpStream::connect("127.0.0.1:34254").unwrap());
    ///
    /// // 查看内部缓冲的容量
    /// let capacity = buf_writer.capacity();
    /// // 计算在无需刷新的情况下还能写入多少字节
    /// let without_flush = capacity - buf_writer.buffer().len();
    /// ```
    #[stable(feature = "buffered_io_capacity", since = "1.46.0")]
    pub fn capacity(&self) -> usize {
        self.buf.capacity()
    }

    // 确保本函数不会被内联进 `write`，从而让 `write` 保持可内联、其常见路径保持尽可能短。
    // 如果本函数相对于 `write` 被调用得很频繁，那很可能是个信号，说明客户端用了大小不当的
    // 缓冲，或其写入模式有些病态。
    #[cold]
    #[inline(never)]
    fn write_cold(&mut self, buf: &[u8]) -> io::Result<usize> {
        if buf.len() > self.spare_capacity() {
            self.flush_buf()?;
        }

        // 为什么不是 len > capacity？为的是当输入恰好把缓冲填满时，避免一次毫无必要的“走一遍
        // 缓冲”。因为那样反正也只能再把它刷新到底层 writer 罢了。
        if buf.len() >= self.buf.capacity() {
            self.panicked = true;
            let r = self.get_mut().write(buf);
            self.panicked = false;
            r
        } else {
            // 写入缓冲。在这种情况下，即便输入恰好把缓冲填满，我们也仍然写入缓冲。否则就意味着
            // 要先刷新缓冲、再把这份输入写到内部 writer，而在很多情况下那是更差的策略。

            // SAFETY: 要么原本就有足够的空闲容量，要么没有、但我们已经刷新了缓冲以确保腾出空间。
            // 在后一种情况下，我们知道空间是够的，因为刷新已使整个缓冲都变为空闲容量，而我们进入
            // 这个分支正是由于输入缓冲的长度小于该容量。无论哪种情况，把输入缓冲写入我们的缓冲
            // 都是安全的。
            unsafe {
                self.write_to_buffer_unchecked(buf);
            }

            Ok(buf.len())
        }
    }

    // 确保本函数不会被内联进 `write_all`，从而让 `write_all` 保持可内联、其常见路径保持
    // 尽可能短。如果本函数相对于 `write_all` 被调用得很频繁，那很可能是个信号，说明客户端
    // 用了大小不当的缓冲，或其写入模式有些病态。
    #[cold]
    #[inline(never)]
    fn write_all_cold(&mut self, buf: &[u8]) -> io::Result<()> {
        // 通常 `write_all` 只是在循环里调用 `write`。我们可以做得更好：直接调用
        // `self.get_mut().write_all()`，在某些情形下当出现一连串部分写入时，这能避免反复
        // “走一遍缓冲”的往返。

        if buf.len() > self.spare_capacity() {
            self.flush_buf()?;
        }

        // 为什么不是 len > capacity？为的是当输入恰好把缓冲填满时，避免一次毫无必要的“走一遍
        // 缓冲”。因为那样反正也只能再把它刷新到底层 writer 罢了。
        if buf.len() >= self.buf.capacity() {
            self.panicked = true;
            let r = self.get_mut().write_all(buf);
            self.panicked = false;
            r
        } else {
            // 写入缓冲。在这种情况下，即便输入恰好把缓冲填满，我们也仍然写入缓冲。否则就意味着
            // 要先刷新缓冲、再把这份输入写到内部 writer，而在很多情况下那是更差的策略。

            // SAFETY: 要么原本就有足够的空闲容量，要么没有、但我们已经刷新了缓冲以确保腾出空间。
            // 在后一种情况下，我们知道空间是够的，因为刷新已使整个缓冲都变为空闲容量，而我们进入
            // 这个分支正是由于输入缓冲的长度小于该容量。无论哪种情况，把输入缓冲写入我们的缓冲
            // 都是安全的。
            unsafe {
                self.write_to_buffer_unchecked(buf);
            }

            Ok(())
        }
    }

    // SAFETY: 要求 `buf.len() <= self.buf.capacity() - self.buf.len()`，
    // 即输入缓冲的长度小于或等于空闲容量。
    #[inline]
    unsafe fn write_to_buffer_unchecked(&mut self, buf: &[u8]) {
        debug_assert!(buf.len() <= self.spare_capacity());
        let old_len = self.buf.len();
        let buf_len = buf.len();
        let src = buf.as_ptr();
        unsafe {
            let dst = self.buf.as_mut_ptr().add(old_len);
            ptr::copy_nonoverlapping(src, dst, buf_len);
            self.buf.set_len(old_len + buf_len);
        }
    }

    #[inline]
    fn spare_capacity(&self) -> usize {
        self.buf.capacity() - self.buf.len()
    }
}

#[stable(feature = "bufwriter_into_parts", since = "1.56.0")]
/// 当底层 writer 此前已经 panic 时，`BufWriter::into_parts` 为已缓冲的数据返回的错误类型。
/// 它包含那份（可能已被部分写出的）已缓冲数据。
///
/// # 示例
///
/// ```
/// use std::io::{self, BufWriter, Write};
/// use std::panic::{catch_unwind, AssertUnwindSafe};
///
/// struct PanickingWriter;
/// impl Write for PanickingWriter {
///   fn write(&mut self, buf: &[u8]) -> io::Result<usize> { panic!() }
///   fn flush(&mut self) -> io::Result<()> { panic!() }
/// }
///
/// let mut stream = BufWriter::new(PanickingWriter);
/// write!(stream, "some data").unwrap();
/// let result = catch_unwind(AssertUnwindSafe(|| {
///     stream.flush().unwrap()
/// }));
/// assert!(result.is_err());
/// let (recovered_writer, buffered_data) = stream.into_parts();
/// assert!(matches!(recovered_writer, PanickingWriter));
/// assert_eq!(buffered_data.unwrap_err().into_inner(), b"some data");
/// ```
pub struct WriterPanicked {
    buf: Vec<u8>,
}

impl WriterPanicked {
    /// 返回那份可能尚未写出的数据。其中一部分数据可能已经被那次（或那几次）发生 panic 的
    /// 对底层 writer 的调用写出了，所以简单地把它再写一遍并不是个好主意。
    #[must_use = "`self` will be dropped if the result is not used"]
    #[stable(feature = "bufwriter_into_parts", since = "1.56.0")]
    pub fn into_inner(self) -> Vec<u8> {
        self.buf
    }
}

#[stable(feature = "bufwriter_into_parts", since = "1.56.0")]
impl error::Error for WriterPanicked {}

#[stable(feature = "bufwriter_into_parts", since = "1.56.0")]
impl fmt::Display for WriterPanicked {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        "BufWriter inner writer panicked, what data remains unwritten is not known".fmt(f)
    }
}

#[stable(feature = "bufwriter_into_parts", since = "1.56.0")]
impl fmt::Debug for WriterPanicked {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WriterPanicked")
            .field("buffer", &format_args!("{}/{}", self.buf.len(), self.buf.capacity()))
            .finish()
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<W: ?Sized + Write> Write for BufWriter<W> {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // 用 < 而非 <=，以便在某些情况下避免一次毫无必要的“走一遍缓冲”。
        // 详见 `write_cold`。
        if buf.len() < self.spare_capacity() {
            // SAFETY: 由上面的条件判断可知是安全的。
            unsafe {
                self.write_to_buffer_unchecked(buf);
            }

            Ok(buf.len())
        } else {
            self.write_cold(buf)
        }
    }

    #[inline]
    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        // 用 < 而非 <=，以便在某些情况下避免一次毫无必要的“走一遍缓冲”。
        // 详见 `write_all_cold`。
        if buf.len() < self.spare_capacity() {
            // SAFETY: 由上面的条件判断可知是安全的。
            unsafe {
                self.write_to_buffer_unchecked(buf);
            }

            Ok(())
        } else {
            self.write_all_cold(buf)
        }
    }

    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        // FIXME: 考虑把已经应用于 `write` 和 `write_all` 的 `#[inline]` / `#[inline(never)]`
        // 优化也应用到这里。性能收益可能相当可观。见 #79930。
        if self.get_ref().is_write_vectored() {
            // 我们必须处理这些缓冲的总长度溢出 `usize` 的可能性（尽管这只有在多个 `IoSlice`
            // 引用同一块底层缓冲时才会发生，否则这些缓冲根本放不进内存）。如果该计算发生溢出，
            // 那么输入必定无法装进我们的缓冲，于是我们转发给内部 writer 的 `write_vectored`
            // 方法，让它来妥善处理。
            let mut saturated_total_len: usize = 0;

            for buf in bufs {
                saturated_total_len = saturated_total_len.saturating_add(buf.len());

                if saturated_total_len > self.spare_capacity() && !self.buf.is_empty() {
                    // 如果输入的总长度超过了缓冲的空闲容量，就刷新。如果发生了溢出，这个条件
                    // 同样成立，而我们也确实需要刷新。
                    self.flush_buf()?;
                }

                if saturated_total_len >= self.buf.capacity() {
                    // 如果输入的总长度大于或等于我们的缓冲容量，就转发给内部 writer。如果
                    // 发生了溢出，这个条件同样成立，于是我们把活儿甩给内部 writer。
                    self.panicked = true;
                    let r = self.get_mut().write_vectored(bufs);
                    self.panicked = false;
                    return r;
                }
            }

            // `saturated_total_len < self.buf.capacity()` 意味着我们没有发生饱和（溢出）。

            // SAFETY: 我们在上面已经检查过空闲容量是否足够大。如果足够，那本来就安全。如果不够，
            // 我们已经刷新，从而为任何 <= 缓冲大小的输入腾出了足够空间，而当前这份输入正属于此列。
            unsafe {
                bufs.iter().for_each(|b| self.write_to_buffer_unchecked(b));
            };

            Ok(saturated_total_len)
        } else {
            let mut iter = bufs.iter();
            let mut total_written = if let Some(buf) = iter.by_ref().find(|&buf| !buf.is_empty()) {
                // 这是要写出的第一个非空切片，所以如果它装不进缓冲，我们仍然可以先刷新再继续。
                if buf.len() > self.spare_capacity() {
                    self.flush_buf()?;
                }
                if buf.len() >= self.buf.capacity() {
                    // 这个切片的大小至少与缓冲容量相当，所以绕过缓冲、直接写出更划算。
                    self.panicked = true;
                    let r = self.get_mut().write(buf);
                    self.panicked = false;
                    return r;
                } else {
                    // SAFETY: 我们在上面已经检查过空闲容量是否足够大。如果足够，那本来就安全。
                    // 如果不够，我们已经刷新，从而为任何 <= 缓冲大小的输入腾出了足够空间，而当前
                    // 这份输入正属于此列。
                    unsafe {
                        self.write_to_buffer_unchecked(buf);
                    }

                    buf.len()
                }
            } else {
                return Ok(0);
            };
            debug_assert!(total_written != 0);
            for buf in iter {
                if buf.len() <= self.spare_capacity() {
                    // SAFETY: 由上面的条件判断可知是安全的。
                    unsafe {
                        self.write_to_buffer_unchecked(buf);
                    }

                    // 这不会让 `usize` 溢出。如果执行到这里，说明我们已经把到目前为止的所有字节
                    // 都写入了缓冲，并且我们已确保从不超过缓冲容量。因此
                    // `total_written` <= `self.buf.capacity()` <= `usize::MAX`。
                    total_written += buf.len();
                } else {
                    break;
                }
            }
            Ok(total_written)
        }
    }

    fn is_write_vectored(&self) -> bool {
        true
    }

    fn flush(&mut self) -> io::Result<()> {
        self.flush_buf().and_then(|()| self.get_mut().flush())
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<W: ?Sized + Write> fmt::Debug for BufWriter<W>
where
    W: fmt::Debug,
{
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("BufWriter")
            .field("writer", &&self.inner)
            .field("buffer", &format_args!("{}/{}", self.buf.len(), self.buf.capacity()))
            .finish()
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<W: ?Sized + Write + Seek> Seek for BufWriter<W> {
    /// 在底层 writer 中按字节偏移进行 seek。
    ///
    /// seek 操作总是会在 seek 之前先把内部缓冲写出。
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        self.flush_buf()?;
        self.get_mut().seek(pos)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<W: ?Sized + Write> Drop for BufWriter<W> {
    fn drop(&mut self) {
        if !self.panicked {
            // 析构函数不应 panic，所以我们忽略一次失败的 flush
            let _r = self.flush_buf();
        }
    }
}
