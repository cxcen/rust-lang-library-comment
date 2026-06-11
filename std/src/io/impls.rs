#[cfg(test)]
mod tests;

use crate::alloc::Allocator;
use crate::collections::VecDeque;
use crate::io::{self, BorrowedCursor, BufRead, IoSlice, IoSliceMut, Read, Seek, SeekFrom, Write};
use crate::{cmp, fmt, mem, str};

// =============================================================================
// 转发（forwarding）实现

#[stable(feature = "rust1", since = "1.0.0")]
impl<R: Read + ?Sized> Read for &mut R {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        (**self).read(buf)
    }

    #[inline]
    fn read_buf(&mut self, cursor: BorrowedCursor<'_>) -> io::Result<()> {
        (**self).read_buf(cursor)
    }

    #[inline]
    fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        (**self).read_vectored(bufs)
    }

    #[inline]
    fn is_read_vectored(&self) -> bool {
        (**self).is_read_vectored()
    }

    #[inline]
    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> io::Result<usize> {
        (**self).read_to_end(buf)
    }

    #[inline]
    fn read_to_string(&mut self, buf: &mut String) -> io::Result<usize> {
        (**self).read_to_string(buf)
    }

    #[inline]
    fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
        (**self).read_exact(buf)
    }

    #[inline]
    fn read_buf_exact(&mut self, cursor: BorrowedCursor<'_>) -> io::Result<()> {
        (**self).read_buf_exact(cursor)
    }
}
#[stable(feature = "rust1", since = "1.0.0")]
impl<W: Write + ?Sized> Write for &mut W {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        (**self).write(buf)
    }

    #[inline]
    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        (**self).write_vectored(bufs)
    }

    #[inline]
    fn is_write_vectored(&self) -> bool {
        (**self).is_write_vectored()
    }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        (**self).flush()
    }

    #[inline]
    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        (**self).write_all(buf)
    }

    #[inline]
    fn write_all_vectored(&mut self, bufs: &mut [IoSlice<'_>]) -> io::Result<()> {
        (**self).write_all_vectored(bufs)
    }

    #[inline]
    fn write_fmt(&mut self, fmt: fmt::Arguments<'_>) -> io::Result<()> {
        (**self).write_fmt(fmt)
    }
}
#[stable(feature = "rust1", since = "1.0.0")]
impl<S: Seek + ?Sized> Seek for &mut S {
    #[inline]
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        (**self).seek(pos)
    }

    #[inline]
    fn rewind(&mut self) -> io::Result<()> {
        (**self).rewind()
    }

    #[inline]
    fn stream_len(&mut self) -> io::Result<u64> {
        (**self).stream_len()
    }

    #[inline]
    fn stream_position(&mut self) -> io::Result<u64> {
        (**self).stream_position()
    }

    #[inline]
    fn seek_relative(&mut self, offset: i64) -> io::Result<()> {
        (**self).seek_relative(offset)
    }
}
#[stable(feature = "rust1", since = "1.0.0")]
impl<B: BufRead + ?Sized> BufRead for &mut B {
    #[inline]
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        (**self).fill_buf()
    }

    #[inline]
    fn consume(&mut self, amt: usize) {
        (**self).consume(amt)
    }

    #[inline]
    fn has_data_left(&mut self) -> io::Result<bool> {
        (**self).has_data_left()
    }

    #[inline]
    fn read_until(&mut self, byte: u8, buf: &mut Vec<u8>) -> io::Result<usize> {
        (**self).read_until(byte, buf)
    }

    #[inline]
    fn skip_until(&mut self, byte: u8) -> io::Result<usize> {
        (**self).skip_until(byte)
    }

    #[inline]
    fn read_line(&mut self, buf: &mut String) -> io::Result<usize> {
        (**self).read_line(buf)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<R: Read + ?Sized> Read for Box<R> {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        (**self).read(buf)
    }

    #[inline]
    fn read_buf(&mut self, cursor: BorrowedCursor<'_>) -> io::Result<()> {
        (**self).read_buf(cursor)
    }

    #[inline]
    fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        (**self).read_vectored(bufs)
    }

    #[inline]
    fn is_read_vectored(&self) -> bool {
        (**self).is_read_vectored()
    }

    #[inline]
    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> io::Result<usize> {
        (**self).read_to_end(buf)
    }

    #[inline]
    fn read_to_string(&mut self, buf: &mut String) -> io::Result<usize> {
        (**self).read_to_string(buf)
    }

    #[inline]
    fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
        (**self).read_exact(buf)
    }

    #[inline]
    fn read_buf_exact(&mut self, cursor: BorrowedCursor<'_>) -> io::Result<()> {
        (**self).read_buf_exact(cursor)
    }
}
#[stable(feature = "rust1", since = "1.0.0")]
impl<W: Write + ?Sized> Write for Box<W> {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        (**self).write(buf)
    }

    #[inline]
    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        (**self).write_vectored(bufs)
    }

    #[inline]
    fn is_write_vectored(&self) -> bool {
        (**self).is_write_vectored()
    }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        (**self).flush()
    }

    #[inline]
    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        (**self).write_all(buf)
    }

    #[inline]
    fn write_all_vectored(&mut self, bufs: &mut [IoSlice<'_>]) -> io::Result<()> {
        (**self).write_all_vectored(bufs)
    }

    #[inline]
    fn write_fmt(&mut self, fmt: fmt::Arguments<'_>) -> io::Result<()> {
        (**self).write_fmt(fmt)
    }
}
#[stable(feature = "rust1", since = "1.0.0")]
impl<S: Seek + ?Sized> Seek for Box<S> {
    #[inline]
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        (**self).seek(pos)
    }

    #[inline]
    fn rewind(&mut self) -> io::Result<()> {
        (**self).rewind()
    }

    #[inline]
    fn stream_len(&mut self) -> io::Result<u64> {
        (**self).stream_len()
    }

    #[inline]
    fn stream_position(&mut self) -> io::Result<u64> {
        (**self).stream_position()
    }

    #[inline]
    fn seek_relative(&mut self, offset: i64) -> io::Result<()> {
        (**self).seek_relative(offset)
    }
}
#[stable(feature = "rust1", since = "1.0.0")]
impl<B: BufRead + ?Sized> BufRead for Box<B> {
    #[inline]
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        (**self).fill_buf()
    }

    #[inline]
    fn consume(&mut self, amt: usize) {
        (**self).consume(amt)
    }

    #[inline]
    fn has_data_left(&mut self) -> io::Result<bool> {
        (**self).has_data_left()
    }

    #[inline]
    fn read_until(&mut self, byte: u8, buf: &mut Vec<u8>) -> io::Result<usize> {
        (**self).read_until(byte, buf)
    }

    #[inline]
    fn skip_until(&mut self, byte: u8) -> io::Result<usize> {
        (**self).skip_until(byte)
    }

    #[inline]
    fn read_line(&mut self, buf: &mut String) -> io::Result<usize> {
        (**self).read_line(buf)
    }
}

// =============================================================================
// 内存缓冲区（in-memory buffer）实现

/// `&[u8]` 的 Read 实现是通过从该切片复制数据来完成的。
///
/// 注意，读取会更新该切片，使其指向尚未读取的部分。当到达 EOF 时，该切片将为空。
#[stable(feature = "rust1", since = "1.0.0")]
impl Read for &[u8] {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let amt = cmp::min(buf.len(), self.len());
        let (a, b) = self.split_at(amt);

        // 首先检查我们想读取的字节数是否很小：`copy_from_slice` 一般会展开为对 `memcpy` 的调用，
        // 而对于单个字节而言，这个开销是显著的。
        if amt == 1 {
            buf[0] = a[0];
        } else {
            buf[..amt].copy_from_slice(a);
        }

        *self = b;
        Ok(amt)
    }

    #[inline]
    fn read_buf(&mut self, mut cursor: BorrowedCursor<'_>) -> io::Result<()> {
        let amt = cmp::min(cursor.capacity(), self.len());
        let (a, b) = self.split_at(amt);

        cursor.append(a);

        *self = b;
        Ok(())
    }

    #[inline]
    fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        let mut nread = 0;
        for buf in bufs {
            nread += self.read(buf)?;
            if self.is_empty() {
                break;
            }
        }

        Ok(nread)
    }

    #[inline]
    fn is_read_vectored(&self) -> bool {
        true
    }

    #[inline]
    fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
        if buf.len() > self.len() {
            // 如果 `read_exact` 失败，它不对 `buf` 的内容做任何承诺，所以这里不必费心处理它。
            *self = &self[self.len()..];
            return Err(io::Error::READ_EXACT_EOF);
        }
        let (a, b) = self.split_at(buf.len());

        // 首先检查我们想读取的字节数是否很小：`copy_from_slice` 一般会展开为对 `memcpy` 的调用，
        // 而对于单个字节而言，这个开销是显著的。
        if buf.len() == 1 {
            buf[0] = a[0];
        } else {
            buf.copy_from_slice(a);
        }

        *self = b;
        Ok(())
    }

    #[inline]
    fn read_buf_exact(&mut self, mut cursor: BorrowedCursor<'_>) -> io::Result<()> {
        if cursor.capacity() > self.len() {
            // 把我们能追加的内容全部追加到 cursor 中。
            cursor.append(*self);
            *self = &self[self.len()..];
            return Err(io::Error::READ_EXACT_EOF);
        }
        let (a, b) = self.split_at(cursor.capacity());

        cursor.append(a);

        *self = b;
        Ok(())
    }

    #[inline]
    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> io::Result<usize> {
        let len = self.len();
        buf.try_reserve(len)?;
        buf.extend_from_slice(*self);
        *self = &self[len..];
        Ok(len)
    }

    #[inline]
    fn read_to_string(&mut self, buf: &mut String) -> io::Result<usize> {
        let content = str::from_utf8(self).map_err(|_| io::Error::INVALID_UTF8)?;
        let len = self.len();
        buf.try_reserve(len)?;
        buf.push_str(content);
        *self = &self[len..];
        Ok(len)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl BufRead for &[u8] {
    #[inline]
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        Ok(*self)
    }

    #[inline]
    fn consume(&mut self, amt: usize) {
        *self = &self[amt..];
    }
}

/// `&mut [u8]` 的 Write 实现是通过复制到该切片中、覆写其数据来完成的。
///
/// 注意，写入会更新该切片，使其指向尚未写入的部分。当切片被完全覆写后，它将为空。
///
/// 如果要写入的字节数超过了切片的大小，写操作将返回短写（short write）：最终返回 `Ok(0)`；
/// 在这种情况下，`write_all` 会返回一个种类为 `ErrorKind::WriteZero` 的错误。
#[stable(feature = "rust1", since = "1.0.0")]
impl Write for &mut [u8] {
    #[inline]
    fn write(&mut self, data: &[u8]) -> io::Result<usize> {
        let amt = cmp::min(data.len(), self.len());
        let (a, b) = mem::take(self).split_at_mut(amt);
        a.copy_from_slice(&data[..amt]);
        *self = b;
        Ok(amt)
    }

    #[inline]
    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        let mut nwritten = 0;
        for buf in bufs {
            nwritten += self.write(buf)?;
            if self.is_empty() {
                break;
            }
        }

        Ok(nwritten)
    }

    #[inline]
    fn is_write_vectored(&self) -> bool {
        true
    }

    #[inline]
    fn write_all(&mut self, data: &[u8]) -> io::Result<()> {
        if self.write(data)? < data.len() { Err(io::Error::WRITE_ALL_EOF) } else { Ok(()) }
    }

    #[inline]
    fn write_all_vectored(&mut self, bufs: &mut [IoSlice<'_>]) -> io::Result<()> {
        for buf in bufs {
            if self.write(buf)? < buf.len() {
                return Err(io::Error::WRITE_ALL_EOF);
            }
        }
        Ok(())
    }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// `Vec<u8>` 的 Write 实现是通过追加到该向量来完成的。该向量会按需增长。
#[stable(feature = "rust1", since = "1.0.0")]
impl<A: Allocator> Write for Vec<u8, A> {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.extend_from_slice(buf);
        Ok(buf.len())
    }

    #[inline]
    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        let len = bufs.iter().map(|b| b.len()).sum();
        self.reserve(len);
        for buf in bufs {
            self.extend_from_slice(buf);
        }
        Ok(len)
    }

    #[inline]
    fn is_write_vectored(&self) -> bool {
        true
    }

    #[inline]
    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        self.extend_from_slice(buf);
        Ok(())
    }

    #[inline]
    fn write_all_vectored(&mut self, bufs: &mut [IoSlice<'_>]) -> io::Result<()> {
        self.write_vectored(bufs)?;
        Ok(())
    }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// `VecDeque<u8>` 的 Read 实现是通过从 `VecDeque` 前端消耗字节来完成的。
#[stable(feature = "vecdeque_read_write", since = "1.63.0")]
impl<A: Allocator> Read for VecDeque<u8, A> {
    /// 用由 [`as_slices`][`VecDeque::as_slices`] 返回的「前端」切片的内容填充 `buf`。如果该
    /// `VecDeque` 所含的字节切片是不连续的，那么读取全部内容将需要多次调用 `read`。
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let (ref mut front, _) = self.as_slices();
        let n = Read::read(front, buf)?;
        self.drain(..n);
        Ok(n)
    }

    #[inline]
    fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
        let (front, back) = self.as_slices();

        // 如果前端缓冲区足够大，能填满 `buf`，就只用它；否则也要用上后端缓冲区。
        match buf.split_at_mut_checked(front.len()) {
            None => buf.copy_from_slice(&front[..buf.len()]),
            Some((buf_front, buf_back)) => match back.split_at_checked(buf_back.len()) {
                Some((back, _)) => {
                    buf_front.copy_from_slice(front);
                    buf_back.copy_from_slice(back);
                }
                None => {
                    self.clear();
                    return Err(io::Error::READ_EXACT_EOF);
                }
            },
        }

        self.drain(..buf.len());
        Ok(())
    }

    #[inline]
    fn read_buf(&mut self, cursor: BorrowedCursor<'_>) -> io::Result<()> {
        let (ref mut front, _) = self.as_slices();
        let n = cmp::min(cursor.capacity(), front.len());
        Read::read_buf(front, cursor)?;
        self.drain(..n);
        Ok(())
    }

    #[inline]
    fn read_buf_exact(&mut self, mut cursor: BorrowedCursor<'_>) -> io::Result<()> {
        let len = cursor.capacity();
        let (front, back) = self.as_slices();

        match front.split_at_checked(cursor.capacity()) {
            Some((front, _)) => cursor.append(front),
            None => {
                cursor.append(front);
                match back.split_at_checked(cursor.capacity()) {
                    Some((back, _)) => cursor.append(back),
                    None => {
                        cursor.append(back);
                        self.clear();
                        return Err(io::Error::READ_EXACT_EOF);
                    }
                }
            }
        }

        self.drain(..len);
        Ok(())
    }

    #[inline]
    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> io::Result<usize> {
        // 总长度事先已知，所以我们可以在单次调用中预留它。
        let len = self.len();
        buf.try_reserve(len)?;

        let (front, back) = self.as_slices();
        buf.extend_from_slice(front);
        buf.extend_from_slice(back);
        self.clear();
        Ok(len)
    }

    #[inline]
    fn read_to_string(&mut self, buf: &mut String) -> io::Result<usize> {
        // 安全性：我们只对缓冲区进行追加
        unsafe { io::append_to_string(buf, |buf| self.read_to_end(buf)) }
    }
}

/// `VecDeque<u8>` 的 BufRead 实现是通过从 `VecDeque` 前端读取字节来完成的。
#[stable(feature = "vecdeque_buf_read", since = "1.75.0")]
impl<A: Allocator> BufRead for VecDeque<u8, A> {
    /// 返回由 [`as_slices`][`VecDeque::as_slices`] 返回的「前端」切片的内容。如果该 `VecDeque`
    /// 所含的字节切片是不连续的，那么读取全部内容将需要多次调用 `fill_buf`。
    #[inline]
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        let (front, _) = self.as_slices();
        Ok(front)
    }

    #[inline]
    fn consume(&mut self, amt: usize) {
        self.drain(..amt);
    }
}

/// `VecDeque<u8>` 的 Write 实现是通过追加到该 `VecDeque`、按需让它增长来完成的。
#[stable(feature = "vecdeque_read_write", since = "1.63.0")]
impl<A: Allocator> Write for VecDeque<u8, A> {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.extend(buf);
        Ok(buf.len())
    }

    #[inline]
    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        let len = bufs.iter().map(|b| b.len()).sum();
        self.reserve(len);
        for buf in bufs {
            self.extend(&**buf);
        }
        Ok(len)
    }

    #[inline]
    fn is_write_vectored(&self) -> bool {
        true
    }

    #[inline]
    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        self.extend(buf);
        Ok(())
    }

    #[inline]
    fn write_all_vectored(&mut self, bufs: &mut [IoSlice<'_>]) -> io::Result<()> {
        self.write_vectored(bufs)?;
        Ok(())
    }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[unstable(feature = "read_buf", issue = "78485")]
impl<'a> io::Write for core::io::BorrowedCursor<'a> {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let amt = cmp::min(buf.len(), self.capacity());
        self.append(&buf[..amt]);
        Ok(amt)
    }

    #[inline]
    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        let mut nwritten = 0;
        for buf in bufs {
            let n = self.write(buf)?;
            nwritten += n;
            if n < buf.len() {
                break;
            }
        }
        Ok(nwritten)
    }

    #[inline]
    fn is_write_vectored(&self) -> bool {
        true
    }

    #[inline]
    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        if self.write(buf)? < buf.len() { Err(io::Error::WRITE_ALL_EOF) } else { Ok(()) }
    }

    #[inline]
    fn write_all_vectored(&mut self, bufs: &mut [IoSlice<'_>]) -> io::Result<()> {
        for buf in bufs {
            if self.write(buf)? < buf.len() {
                return Err(io::Error::WRITE_ALL_EOF);
            }
        }
        Ok(())
    }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
