mod buffer;

use buffer::Buffer;

use crate::fmt;
use crate::io::{
    self, BorrowedCursor, BufRead, DEFAULT_BUF_SIZE, IoSliceMut, Read, Seek, SeekFrom, SizeHint,
    SpecReadByte, uninlined_slow_read_byte,
};

/// `BufReader<R>` 结构体为任意 reader 添加缓冲能力。
///
/// 直接操作一个 [`Read`] 实例可能会非常低效。举例来说，对 [`TcpStream`] 的每一次
/// [`read`][`TcpStream::read`] 调用都会引发一次系统调用。而 `BufReader<R>` 会对底层
/// [`Read`] 执行少数几次大块读取，并把读到的结果保存在内存缓冲中。
///
/// 对于那些对同一个文件或网络 socket 反复进行 *小量* 读取的程序，`BufReader<R>` 能提升
/// 速度。但如果你一次读取的数据量非常大，或只读取一两次，它就帮不上忙。此外，当数据源本身
/// 就已经在内存中时（例如 <code>[Vec]\<u8></code>），它也没有任何优势。
///
/// 当 `BufReader<R>` 被 drop 时，其缓冲中的内容会被丢弃。在同一个流上创建多个
/// `BufReader<R>` 实例可能导致数据丢失。同样地，在用 [`BufReader::into_inner`] 拆出
/// 底层 reader 之后再从底层 reader 读取，也可能导致数据丢失（因为缓冲中已读入但尚未消费的
/// 数据会随 `BufReader` 一起被丢弃）。
///
/// [`TcpStream::read`]: crate::net::TcpStream::read
/// [`TcpStream`]: crate::net::TcpStream
///
/// # 示例
///
/// ```no_run
/// use std::io::prelude::*;
/// use std::io::BufReader;
/// use std::fs::File;
///
/// fn main() -> std::io::Result<()> {
///     let f = File::open("log.txt")?;
///     let mut reader = BufReader::new(f);
///
///     let mut line = String::new();
///     let len = reader.read_line(&mut line)?;
///     println!("First line is {len} bytes long");
///     Ok(())
/// }
/// ```
#[stable(feature = "rust1", since = "1.0.0")]
pub struct BufReader<R: ?Sized> {
    buf: Buffer,
    inner: R,
}

impl<R: Read> BufReader<R> {
    /// 用默认缓冲容量创建一个新的 `BufReader<R>`。当前默认值是 8 KiB，但将来可能会变。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::io::BufReader;
    /// use std::fs::File;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let f = File::open("log.txt")?;
    ///     let reader = BufReader::new(f);
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn new(inner: R) -> BufReader<R> {
        BufReader::with_capacity(DEFAULT_BUF_SIZE, inner)
    }

    pub(crate) fn try_new_buffer() -> io::Result<Buffer> {
        Buffer::try_with_capacity(DEFAULT_BUF_SIZE)
    }

    pub(crate) fn with_buffer(inner: R, buf: Buffer) -> Self {
        Self { inner, buf }
    }

    /// 用指定的缓冲容量创建一个新的 `BufReader<R>`。
    ///
    /// # 示例
    ///
    /// 创建一个容量为 10 字节的缓冲：
    ///
    /// ```no_run
    /// use std::io::BufReader;
    /// use std::fs::File;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let f = File::open("log.txt")?;
    ///     let reader = BufReader::with_capacity(10, f);
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn with_capacity(capacity: usize, inner: R) -> BufReader<R> {
        BufReader { inner, buf: Buffer::with_capacity(capacity) }
    }
}

impl<R: Read + ?Sized> BufReader<R> {
    /// 尝试向前预看（look ahead）`n` 个字节。
    ///
    /// `n` 必须小于或等于 `capacity`（缓冲容量）。
    ///
    /// 如果到达了文件末尾（EOF），返回的切片长度可能小于 `n`。
    ///
    /// 调用本方法之后，你可以再调用 [`consume`](BufRead::consume) 并传入一个不超过 `n`
    /// 的值，以便跳过（消费）返回字节中的一部分或全部。
    ///
    /// ## 示例
    ///
    /// ```rust
    /// #![feature(bufreader_peek)]
    /// use std::io::{Read, BufReader};
    ///
    /// let mut bytes = &b"oh, hello there"[..];
    /// let mut rdr = BufReader::with_capacity(6, &mut bytes);
    /// assert_eq!(rdr.peek(2).unwrap(), b"oh");
    /// let mut buf = [0; 4];
    /// rdr.read(&mut buf[..]).unwrap();
    /// assert_eq!(&buf, b"oh, ");
    /// assert_eq!(rdr.peek(5).unwrap(), b"hello");
    /// let mut s = String::new();
    /// rdr.read_to_string(&mut s).unwrap();
    /// assert_eq!(&s, "hello there");
    /// assert_eq!(rdr.peek(1).unwrap().len(), 0);
    /// ```
    #[unstable(feature = "bufreader_peek", issue = "128405")]
    pub fn peek(&mut self, n: usize) -> io::Result<&[u8]> {
        assert!(n <= self.capacity());
        while n > self.buf.buffer().len() {
            if self.buf.pos() > 0 {
                self.buf.backshift();
            }
            let new = self.buf.read_more(&mut self.inner)?;
            if new == 0 {
                // 到达文件末尾，没有更多字节可读
                return Ok(&self.buf.buffer()[..]);
            }
            debug_assert_eq!(self.buf.pos(), 0);
        }
        Ok(&self.buf.buffer()[..n])
    }
}

impl<R: ?Sized> BufReader<R> {
    /// 获取对底层 reader 的不可变引用。
    ///
    /// 不建议直接从底层 reader 读取。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::io::BufReader;
    /// use std::fs::File;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let f1 = File::open("log.txt")?;
    ///     let reader = BufReader::new(f1);
    ///
    ///     let f2 = reader.get_ref();
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn get_ref(&self) -> &R {
        &self.inner
    }

    /// 获取对底层 reader 的可变引用。
    ///
    /// 不建议直接从底层 reader 读取。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::io::BufReader;
    /// use std::fs::File;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let f1 = File::open("log.txt")?;
    ///     let mut reader = BufReader::new(f1);
    ///
    ///     let f2 = reader.get_mut();
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn get_mut(&mut self) -> &mut R {
        &mut self.inner
    }

    /// 返回对内部已缓冲数据的引用。
    ///
    /// 与 [`fill_buf`] 不同，如果缓冲为空，本方法不会尝试去填充它。
    ///
    /// [`fill_buf`]: BufRead::fill_buf
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::io::{BufReader, BufRead};
    /// use std::fs::File;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let f = File::open("log.txt")?;
    ///     let mut reader = BufReader::new(f);
    ///     assert!(reader.buffer().is_empty());
    ///
    ///     if reader.fill_buf()?.len() > 0 {
    ///         assert!(!reader.buffer().is_empty());
    ///     }
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "bufreader_buffer", since = "1.37.0")]
    pub fn buffer(&self) -> &[u8] {
        self.buf.buffer()
    }

    /// 返回内部缓冲一次最多能容纳的字节数。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::io::{BufReader, BufRead};
    /// use std::fs::File;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let f = File::open("log.txt")?;
    ///     let mut reader = BufReader::new(f);
    ///
    ///     let capacity = reader.capacity();
    ///     let buffer = reader.fill_buf()?;
    ///     assert!(buffer.len() <= capacity);
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "buffered_io_capacity", since = "1.46.0")]
    pub fn capacity(&self) -> usize {
        self.buf.capacity()
    }

    /// 拆开（unwrap）这个 `BufReader<R>`，返回其底层 reader。
    ///
    /// 注意：内部缓冲中残留的任何数据都会丢失。因此，之后再从底层 reader 读取可能会导致
    /// 数据丢失。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::io::BufReader;
    /// use std::fs::File;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let f1 = File::open("log.txt")?;
    ///     let reader = BufReader::new(f1);
    ///
    ///     let f2 = reader.into_inner();
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn into_inner(self) -> R
    where
        R: Sized,
    {
        self.inner
    }

    /// 使内部缓冲中的所有数据失效（清空）。
    #[inline]
    pub(in crate::io) fn discard_buffer(&mut self) {
        self.buf.discard_buffer()
    }
}

// 这个方法仅被一个测试使用，该测试用于断言“初始化跟踪”逻辑是正确的。
#[cfg(test)]
impl<R: ?Sized> BufReader<R> {
    #[allow(missing_docs)]
    pub fn initialized(&self) -> usize {
        self.buf.initialized()
    }
}

impl<R: ?Sized + Seek> BufReader<R> {
    /// 相对于当前位置进行 seek。如果新位置落在缓冲范围之内，缓冲就不会被刷新（丢弃），
    /// 从而实现更高效的 seek。
    /// 本方法不会返回底层 reader 的位置，因此如有需要，调用方必须自行跟踪这一信息。
    #[stable(feature = "bufreader_seek_relative", since = "1.53.0")]
    pub fn seek_relative(&mut self, offset: i64) -> io::Result<()> {
        let pos = self.buf.pos() as u64;
        if offset < 0 {
            if let Some(_) = pos.checked_sub((-offset) as u64) {
                self.buf.unconsume((-offset) as usize);
                return Ok(());
            }
        } else if let Some(new_pos) = pos.checked_add(offset as u64) {
            if new_pos <= self.buf.filled() as u64 {
                self.buf.consume(offset as usize);
                return Ok(());
            }
        }

        self.seek(SeekFrom::Current(offset)).map(drop)
    }
}

impl<R> SpecReadByte for BufReader<R>
where
    Self: Read,
{
    #[inline]
    fn spec_read_byte(&mut self) -> Option<io::Result<u8>> {
        let mut byte = 0;
        if self.buf.consume_with(1, |claimed| byte = claimed[0]) {
            return Some(Ok(byte));
        }

        // 回退（fallback）分支，每次缓冲被重新填满时才会到达这里一次。
        uninlined_slow_read_byte(self)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<R: ?Sized + Read> Read for BufReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        // 如果我们当前没有任何已缓冲的数据，且这次是一次超大读取（比内部缓冲还大），
        // 就完全绕过内部缓冲，直接对底层 reader 读取。
        if self.buf.pos() == self.buf.filled() && buf.len() >= self.capacity() {
            self.discard_buffer();
            return self.inner.read(buf);
        }
        let mut rem = self.fill_buf()?;
        let nread = rem.read(buf)?;
        self.consume(nread);
        Ok(nread)
    }

    fn read_buf(&mut self, mut cursor: BorrowedCursor<'_>) -> io::Result<()> {
        // 如果我们当前没有任何已缓冲的数据，且这次是一次超大读取（比内部缓冲还大），
        // 就完全绕过内部缓冲。
        if self.buf.pos() == self.buf.filled() && cursor.capacity() >= self.capacity() {
            self.discard_buffer();
            return self.inner.read_buf(cursor);
        }

        let prev = cursor.written();

        let mut rem = self.fill_buf()?;
        rem.read_buf(cursor.reborrow())?; // 实际上永远不会失败

        self.consume(cursor.written() - prev); // 切片对 read_buf 的实现已知绝不会“反填充”缓冲

        Ok(())
    }

    // 在配合反序列化器（deserializer）使用时，对 BufReader 进行大量小规模的 read_exact
    // 是极其常见的。默认实现会在循环中调用 read，而对于“缓冲中已有足够字节来填满传入缓冲”
    // 这一常见路径来说，这会产生出乎意料地糟糕的代码生成结果。
    fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
        if self.buf.consume_with(buf.len(), |claimed| buf.copy_from_slice(claimed)) {
            return Ok(());
        }

        crate::io::default_read_exact(self, buf)
    }

    fn read_buf_exact(&mut self, mut cursor: BorrowedCursor<'_>) -> io::Result<()> {
        if self.buf.consume_with(cursor.capacity(), |claimed| cursor.append(claimed)) {
            return Ok(());
        }

        crate::io::default_read_buf_exact(self, cursor)
    }

    fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        let total_len = bufs.iter().map(|b| b.len()).sum::<usize>();
        if self.buf.pos() == self.buf.filled() && total_len >= self.capacity() {
            self.discard_buffer();
            return self.inner.read_vectored(bufs);
        }
        let mut rem = self.fill_buf()?;
        let nread = rem.read_vectored(bufs)?;

        self.consume(nread);
        Ok(nread)
    }

    fn is_read_vectored(&self) -> bool {
        self.inner.is_read_vectored()
    }

    // 底层 reader 可能拥有经过优化的 `read_to_end`。先把我们自己的缓冲排空，再委托给
    // 底层实现。
    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> io::Result<usize> {
        let inner_buf = self.buffer();
        buf.try_reserve(inner_buf.len())?;
        buf.extend_from_slice(inner_buf);
        let nread = inner_buf.len();
        self.discard_buffer();
        Ok(nread + self.inner.read_to_end(buf)?)
    }

    // 底层 reader 可能拥有经过优化的 `read_to_end`。先把我们自己的缓冲排空，再委托给
    // 底层实现。
    fn read_to_string(&mut self, buf: &mut String) -> io::Result<usize> {
        // 在下面那个通用的 `else` 分支里，我们必须先把字节读入一个临时缓冲，检查它们是
        // 合法的 UTF-8，然后再追加到 `buf`。这会引入一次可能很大的 memcpy。
        //
        // 如果 `buf` 是空的——这是最常见的情形——我们就可以借助 `append_to_string`
        // 直接读入 `buf` 的内部字节缓冲，从而省去一次分配和一次 memcpy。
        if buf.is_empty() {
            // `append_to_string` 的安全性依赖于：缓冲只会被追加（append），因为它只检查新
            // 数据的 UTF-8 合法性。如果 `buf` 中原本就有内容，那么一个不可信的 reader
            //（即 `self.inner`）不仅能追加字节，还可能修改已有字节、使其变得非法。反之，
            // 若 `buf` 为空，则按定义任何写入都必然是追加，`append_to_string` 也就会校验
            // 所有新字节。
            unsafe { crate::io::append_to_string(buf, |b| self.read_to_end(b)) }
        } else {
            // 我们不能把字节缓冲直接追加到 `buf` 这个 String 上，因为其中可能存在一个只读
            // 取了一部分、尚不完整的 UTF-8 序列。我们必须先把所有内容读入一个临时缓冲，
            // 再对这个完整的缓冲调用 `from_utf8`。
            let mut bytes = Vec::new();
            self.read_to_end(&mut bytes)?;
            let string = crate::str::from_utf8(&bytes).map_err(|_| io::Error::INVALID_UTF8)?;
            *buf += string;
            Ok(string.len())
        }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<R: ?Sized + Read> BufRead for BufReader<R> {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        self.buf.fill_buf(&mut self.inner)
    }

    fn consume(&mut self, amt: usize) {
        self.buf.consume(amt)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<R> fmt::Debug for BufReader<R>
where
    R: ?Sized + fmt::Debug,
{
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_struct("BufReader")
            .field("reader", &&self.inner)
            .field(
                "buffer",
                &format_args!("{}/{}", self.buf.filled() - self.buf.pos(), self.capacity()),
            )
            .finish()
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<R: ?Sized + Seek> Seek for BufReader<R> {
    /// 在底层 reader 中按字节偏移进行 seek。
    ///
    /// 使用 <code>[SeekFrom::Current]\(_)</code> 进行 seek 时所依据的位置，是
    /// “假如 `BufReader<R>` 没有内部缓冲、底层 reader 本应所处”的那个位置。
    ///
    /// seek 操作总是会丢弃内部缓冲，即便目标 seek 位置原本就落在缓冲范围之内。这保证了
    /// 在 seek 之后立即调用 [`BufReader::into_inner()`] 取出的底层 reader 处于同一个
    /// 位置。
    ///
    /// 若想在不丢弃内部缓冲的前提下 seek，请使用 [`BufReader::seek_relative`]。
    ///
    /// 更多细节见 [`std::io::Seek`]。
    ///
    /// 注意：在一种边缘情况下——你以 <code>[SeekFrom::Current]\(n)</code> 进行 seek，
    /// 而 `n` 减去内部缓冲长度会让一个 `i64` 溢出——此时会执行两次 seek 而非一次。如果
    /// 第二次 seek 返回了 [`Err`]，底层 reader 将停留在“假如你以
    /// <code>[SeekFrom::Current]\(0)</code> 调用 `seek` 时本应所处”的同一位置。
    ///
    /// [`std::io::Seek`]: Seek
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let result: u64;
        if let SeekFrom::Current(n) = pos {
            let remainder = (self.buf.filled() - self.buf.pos()) as i64;
            // 可以安全地假设 remainder 能容纳在一个 i64 中，因为反过来就意味着我们竟然分配了
            // 8 EiB（艾字节）的缓冲，这是荒谬的。
            // 但某个古怪的底层 reader 支持按 i64::MIN 进行 seek 也并非完全不可能，所以在
            // 减去 remainder 时我们仍需处理下溢的情况。
            if let Some(offset) = n.checked_sub(remainder) {
                result = self.inner.seek(SeekFrom::Current(offset))?;
            } else {
                // 先按我们的 remainder 向后 seek，再按 offset seek
                self.inner.seek(SeekFrom::Current(-remainder))?;
                self.discard_buffer();
                result = self.inner.seek(SeekFrom::Current(n))?;
            }
        } else {
            // 用 Start/End 方式 seek 时不关心我们的缓冲长度。
            result = self.inner.seek(pos)?;
        }
        self.discard_buffer();
        Ok(result)
    }

    /// 返回从流起始处算起的当前 seek 位置。
    ///
    /// 返回的值等价于 `self.seek(SeekFrom::Current(0))`，但不会刷新（丢弃）内部缓冲。
    /// 由于这一优化，本函数并不保证在其后立即调用 `.into_inner()` 会得到处于同一位置的
    /// 底层 reader。如果你需要这一保证，请改用 [`BufReader::seek`]。
    ///
    /// # Panics
    ///
    /// 如果底层 reader 的位置小于已缓冲数据的数量，本函数会 panic。这可能发生在：底层
    /// reader 对 [`Seek::stream_position`] 的实现不正确，或者由于直接在底层 reader 上
    /// 调用 [`Seek::seek`] 而导致位置失去同步。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::{
    ///     io::{self, BufRead, BufReader, Seek},
    ///     fs::File,
    /// };
    ///
    /// fn main() -> io::Result<()> {
    ///     let mut f = BufReader::new(File::open("foo.txt")?);
    ///
    ///     let before = f.stream_position()?;
    ///     f.read_line(&mut String::new())?;
    ///     let after = f.stream_position()?;
    ///
    ///     println!("The first line was {} bytes long", after - before);
    ///     Ok(())
    /// }
    /// ```
    fn stream_position(&mut self) -> io::Result<u64> {
        let remainder = (self.buf.filled() - self.buf.pos()) as u64;
        self.inner.stream_position().map(|pos| {
            pos.checked_sub(remainder).expect(
                "overflow when subtracting remaining buffer size from inner stream position",
            )
        })
    }

    /// 相对于当前位置进行 seek。
    ///
    /// 如果新位置落在缓冲范围之内，缓冲就不会被刷新（丢弃），从而实现更高效的 seek。本方法
    /// 不会返回底层 reader 的位置，因此如有需要，调用方必须自行跟踪这一信息。
    fn seek_relative(&mut self, offset: i64) -> io::Result<()> {
        self.seek_relative(offset)
    }
}

impl<T: ?Sized> SizeHint for BufReader<T> {
    #[inline]
    fn lower_bound(&self) -> usize {
        SizeHint::lower_bound(self.get_ref()) + self.buffer().len()
    }

    #[inline]
    fn upper_bound(&self) -> Option<usize> {
        SizeHint::upper_bound(self.get_ref()).and_then(|up| self.buffer().len().checked_add(up))
    }
}
