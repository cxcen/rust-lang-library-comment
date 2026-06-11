#![allow(missing_copy_implementations)]

#[cfg(test)]
mod tests;

use crate::fmt;
use crate::io::{
    self, BorrowedCursor, BufRead, IoSlice, IoSliceMut, Read, Seek, SeekFrom, SizeHint, Write,
};

/// `Empty` 会忽略任何通过 [`Write`] 写入的数据，并且在通过 [`Read`] 读取时始终为空
///（返回零个字节）。
///
/// 这个结构体通常通过调用 [`empty()`] 来创建。更多细节请参见 [`empty()`] 的文档。
#[stable(feature = "rust1", since = "1.0.0")]
#[non_exhaustive]
#[derive(Copy, Clone, Debug, Default)]
pub struct Empty;

/// 创建一个值：对读取而言它始终处于 EOF，对写入而言它会忽略所有数据。
///
/// 对返回实例的所有 [`write`] 调用都将返回 [`Ok(buf.len())`]，且缓冲的内容不会被检查。
///
/// 对返回 reader 的所有 [`read`] 调用都将返回 [`Ok(0)`]。
///
/// [`Ok(buf.len())`]: Ok
/// [`Ok(0)`]: Ok
///
/// [`write`]: Write::write
/// [`read`]: Read::read
///
/// # 示例
///
/// ```rust
/// use std::io::{self, Write};
///
/// let buffer = vec![1, 2, 3, 5, 8];
/// let num_bytes = io::empty().write(&buffer).unwrap();
/// assert_eq!(num_bytes, 5);
/// ```
///
///
/// ```rust
/// use std::io::{self, Read};
///
/// let mut buffer = String::new();
/// io::empty().read_to_string(&mut buffer).unwrap();
/// assert!(buffer.is_empty());
/// ```
#[must_use]
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_stable(feature = "const_io_structs", since = "1.79.0")]
pub const fn empty() -> Empty {
    Empty
}

#[stable(feature = "rust1", since = "1.0.0")]
impl Read for Empty {
    #[inline]
    fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
        Ok(0)
    }

    #[inline]
    fn read_buf(&mut self, _cursor: BorrowedCursor<'_>) -> io::Result<()> {
        Ok(())
    }

    #[inline]
    fn read_vectored(&mut self, _bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        Ok(0)
    }

    #[inline]
    fn is_read_vectored(&self) -> bool {
        // 不要强迫 `Chain<Empty, T>` 或 `Chain<T, Empty>` 去使用向量化读取，除非另一个
        // reader 本身就是向量化的。
        false
    }

    #[inline]
    fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
        if !buf.is_empty() { Err(io::Error::READ_EXACT_EOF) } else { Ok(()) }
    }

    #[inline]
    fn read_buf_exact(&mut self, cursor: BorrowedCursor<'_>) -> io::Result<()> {
        if cursor.capacity() != 0 { Err(io::Error::READ_EXACT_EOF) } else { Ok(()) }
    }

    #[inline]
    fn read_to_end(&mut self, _buf: &mut Vec<u8>) -> io::Result<usize> {
        Ok(0)
    }

    #[inline]
    fn read_to_string(&mut self, _buf: &mut String) -> io::Result<usize> {
        Ok(0)
    }
}
#[stable(feature = "rust1", since = "1.0.0")]
impl BufRead for Empty {
    #[inline]
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        Ok(&[])
    }

    #[inline]
    fn consume(&mut self, _n: usize) {}

    #[inline]
    fn has_data_left(&mut self) -> io::Result<bool> {
        Ok(false)
    }

    #[inline]
    fn read_until(&mut self, _byte: u8, _buf: &mut Vec<u8>) -> io::Result<usize> {
        Ok(0)
    }

    #[inline]
    fn skip_until(&mut self, _byte: u8) -> io::Result<usize> {
        Ok(0)
    }

    #[inline]
    fn read_line(&mut self, _buf: &mut String) -> io::Result<usize> {
        Ok(0)
    }
}

#[stable(feature = "empty_seek", since = "1.51.0")]
impl Seek for Empty {
    #[inline]
    fn seek(&mut self, _pos: SeekFrom) -> io::Result<u64> {
        Ok(0)
    }

    #[inline]
    fn stream_len(&mut self) -> io::Result<u64> {
        Ok(0)
    }

    #[inline]
    fn stream_position(&mut self) -> io::Result<u64> {
        Ok(0)
    }
}

impl SizeHint for Empty {
    #[inline]
    fn upper_bound(&self) -> Option<usize> {
        Some(0)
    }
}

#[stable(feature = "empty_write", since = "1.73.0")]
impl Write for Empty {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        Ok(buf.len())
    }

    #[inline]
    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        let total_len = bufs.iter().map(|b| b.len()).sum();
        Ok(total_len)
    }

    #[inline]
    fn is_write_vectored(&self) -> bool {
        true
    }

    #[inline]
    fn write_all(&mut self, _buf: &[u8]) -> io::Result<()> {
        Ok(())
    }

    #[inline]
    fn write_all_vectored(&mut self, _bufs: &mut [IoSlice<'_>]) -> io::Result<()> {
        Ok(())
    }

    #[inline]
    fn write_fmt(&mut self, _args: fmt::Arguments<'_>) -> io::Result<()> {
        Ok(())
    }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[stable(feature = "empty_write", since = "1.73.0")]
impl Write for &Empty {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        Ok(buf.len())
    }

    #[inline]
    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        let total_len = bufs.iter().map(|b| b.len()).sum();
        Ok(total_len)
    }

    #[inline]
    fn is_write_vectored(&self) -> bool {
        true
    }

    #[inline]
    fn write_all(&mut self, _buf: &[u8]) -> io::Result<()> {
        Ok(())
    }

    #[inline]
    fn write_all_vectored(&mut self, _bufs: &mut [IoSlice<'_>]) -> io::Result<()> {
        Ok(())
    }

    #[inline]
    fn write_fmt(&mut self, _args: fmt::Arguments<'_>) -> io::Result<()> {
        Ok(())
    }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// 一个 reader，它会一遍又一遍又一遍又一遍又一遍地……不断产出同一个字节。
///
/// 这个结构体通常通过调用 [`repeat()`] 来创建。更多细节请参见 [`repeat()`] 的文档。
#[stable(feature = "rust1", since = "1.0.0")]
pub struct Repeat {
    byte: u8,
}

/// 创建一个 reader 实例，它会无限地重复同一个字节。
///
/// 对这个 reader 的所有读取都会成功，做法是用给定的字节填满指定的缓冲。
///
/// # 示例
///
/// ```
/// use std::io::{self, Read};
///
/// let mut buffer = [0; 3];
/// io::repeat(0b101).read_exact(&mut buffer).unwrap();
/// assert_eq!(buffer, [0b101, 0b101, 0b101]);
/// ```
#[must_use]
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_stable(feature = "const_io_structs", since = "1.79.0")]
pub const fn repeat(byte: u8) -> Repeat {
    Repeat { byte }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl Read for Repeat {
    #[inline]
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        buf.fill(self.byte);
        Ok(buf.len())
    }

    #[inline]
    fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
        buf.fill(self.byte);
        Ok(())
    }

    #[inline]
    fn read_buf(&mut self, mut buf: BorrowedCursor<'_>) -> io::Result<()> {
        // SAFETY: 没有写入任何未初始化的字节。
        unsafe { buf.as_mut() }.write_filled(self.byte);
        // SAFETY: buf 的整个未填充部分都已被初始化。
        unsafe { buf.advance_unchecked(buf.capacity()) };
        Ok(())
    }

    #[inline]
    fn read_buf_exact(&mut self, buf: BorrowedCursor<'_>) -> io::Result<()> {
        self.read_buf(buf)
    }

    /// `io::Repeat` 不支持这个函数，因为它的数据没有尽头
    fn read_to_end(&mut self, _: &mut Vec<u8>) -> io::Result<usize> {
        Err(io::Error::from(io::ErrorKind::OutOfMemory))
    }

    /// `io::Repeat` 不支持这个函数，因为它的数据没有尽头
    fn read_to_string(&mut self, _: &mut String) -> io::Result<usize> {
        Err(io::Error::from(io::ErrorKind::OutOfMemory))
    }

    #[inline]
    fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        let mut nwritten = 0;
        for buf in bufs {
            nwritten += self.read(buf)?;
        }
        Ok(nwritten)
    }

    #[inline]
    fn is_read_vectored(&self) -> bool {
        true
    }
}

impl SizeHint for Repeat {
    #[inline]
    fn lower_bound(&self) -> usize {
        usize::MAX
    }

    #[inline]
    fn upper_bound(&self) -> Option<usize> {
        None
    }
}

#[stable(feature = "std_debug", since = "1.16.0")]
impl fmt::Debug for Repeat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Repeat").finish_non_exhaustive()
    }
}

/// 一个 writer，它会把数据搬进虚空（即丢弃所有写入的数据）。
///
/// 这个结构体通常通过调用 [`sink()`] 来创建。更多细节请参见 [`sink()`] 的文档。
#[stable(feature = "rust1", since = "1.0.0")]
#[non_exhaustive]
#[derive(Copy, Clone, Debug, Default)]
pub struct Sink;

/// 创建一个 writer 实例，它会成功地消费掉所有数据。
///
/// 对返回实例的所有 [`write`] 调用都将返回 [`Ok(buf.len())`]，且缓冲的内容不会被检查。
///
/// [`write`]: Write::write
/// [`Ok(buf.len())`]: Ok
///
/// # 示例
///
/// ```rust
/// use std::io::{self, Write};
///
/// let buffer = vec![1, 2, 3, 5, 8];
/// let num_bytes = io::sink().write(&buffer).unwrap();
/// assert_eq!(num_bytes, 5);
/// ```
#[must_use]
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_stable(feature = "const_io_structs", since = "1.79.0")]
pub const fn sink() -> Sink {
    Sink
}

#[stable(feature = "rust1", since = "1.0.0")]
impl Write for Sink {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        Ok(buf.len())
    }

    #[inline]
    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        let total_len = bufs.iter().map(|b| b.len()).sum();
        Ok(total_len)
    }

    #[inline]
    fn is_write_vectored(&self) -> bool {
        true
    }

    #[inline]
    fn write_all(&mut self, _buf: &[u8]) -> io::Result<()> {
        Ok(())
    }

    #[inline]
    fn write_all_vectored(&mut self, _bufs: &mut [IoSlice<'_>]) -> io::Result<()> {
        Ok(())
    }

    #[inline]
    fn write_fmt(&mut self, _args: fmt::Arguments<'_>) -> io::Result<()> {
        Ok(())
    }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[stable(feature = "write_mt", since = "1.48.0")]
impl Write for &Sink {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        Ok(buf.len())
    }

    #[inline]
    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        let total_len = bufs.iter().map(|b| b.len()).sum();
        Ok(total_len)
    }

    #[inline]
    fn is_write_vectored(&self) -> bool {
        true
    }

    #[inline]
    fn write_all(&mut self, _buf: &[u8]) -> io::Result<()> {
        Ok(())
    }

    #[inline]
    fn write_all_vectored(&mut self, _bufs: &mut [IoSlice<'_>]) -> io::Result<()> {
        Ok(())
    }

    #[inline]
    fn write_fmt(&mut self, _args: fmt::Arguments<'_>) -> io::Result<()> {
        Ok(())
    }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
