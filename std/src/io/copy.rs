use super::{BorrowedBuf, BufReader, BufWriter, DEFAULT_BUF_SIZE, Read, Result, Write};
use crate::alloc::Allocator;
use crate::cmp;
use crate::collections::VecDeque;
use crate::io::IoSlice;
use crate::mem::MaybeUninit;
use crate::sys::io::{CopyState, kernel_copy};

#[cfg(test)]
mod tests;

/// 把一个 reader 的全部内容复制到一个 writer 中。
///
/// 本函数会以流式（streaming）的方式持续地从 `reader` 读取数据、再把它写入 `writer`，
/// 直到 `reader` 返回 EOF。
///
/// 成功时，返回从 `reader` 复制到 `writer` 的字节总数。
///
/// 如果你想把一个文件的内容复制到另一个文件、并且正在使用文件系统路径，请参见
/// [`fs::copy`] 函数。
///
/// [`fs::copy`]: crate::fs::copy
///
/// # Errors
///
/// 一旦对 [`read`] 或 [`write`] 的任何调用返回错误，本函数会立即返回该错误。所有
/// [`ErrorKind::Interrupted`] 都会由本函数处理，并对底层操作进行重试。
///
/// [`read`]: Read::read
/// [`write`]: Write::write
/// [`ErrorKind::Interrupted`]: crate::io::ErrorKind::Interrupted
///
/// # 示例
///
/// ```
/// use std::io;
///
/// fn main() -> io::Result<()> {
///     let mut reader: &[u8] = b"hello";
///     let mut writer: Vec<u8> = vec![];
///
///     io::copy(&mut reader, &mut writer)?;
///
///     assert_eq!(&b"hello"[..], &writer[..]);
///     Ok(())
/// }
/// ```
///
/// # 平台特定行为（Platform-specific behavior）
///
/// 在 Linux（包括 Android）上，如果可能，本函数会使用 `copy_file_range(2)`、
/// `sendfile(2)` 或 `splice(2)` 系统调用，在文件描述符之间直接搬运数据（即内核级的零拷贝
/// 优化路径，避免数据往返用户态）。
///
/// 注意：平台特定行为[将来可能会改变][changes]。
///
/// [changes]: crate::io#platform-specific-behavior
#[stable(feature = "rust1", since = "1.0.0")]
pub fn copy<R: ?Sized, W: ?Sized>(reader: &mut R, writer: &mut W) -> Result<u64>
where
    R: Read,
    W: Write,
{
    match kernel_copy(reader, writer)? {
        CopyState::Ended(copied) => Ok(copied),
        CopyState::Fallback(copied) => {
            generic_copy(reader, writer).map(|additional| copied + additional)
        }
    }
}

/// `io::copy` 的用户态“读-写循环”实现，当面向特定 OS 的“复制卸载（copy offloading）”特化
/// 不可用或不适用时，就使用它。
fn generic_copy<R: ?Sized, W: ?Sized>(reader: &mut R, writer: &mut W) -> Result<u64>
where
    R: Read,
    W: Write,
{
    let read_buf = BufferedReaderSpec::buffer_size(reader);
    let write_buf = BufferedWriterSpec::buffer_size(writer);

    if read_buf >= DEFAULT_BUF_SIZE && read_buf >= write_buf {
        return BufferedReaderSpec::copy_to(reader, writer);
    }

    BufferedWriterSpec::copy_from(writer, reader)
}

/// “读-写循环”的特化，它复用 BufReader 的内部缓冲。如果没有缓冲，就应当改用 writer 那一侧。
trait BufferedReaderSpec {
    fn buffer_size(&self) -> usize;

    fn copy_to(&mut self, to: &mut (impl Write + ?Sized)) -> Result<u64>;
}

impl<T> BufferedReaderSpec for T
where
    Self: Read,
    T: ?Sized,
{
    #[inline]
    default fn buffer_size(&self) -> usize {
        0
    }

    default fn copy_to(&mut self, _to: &mut (impl Write + ?Sized)) -> Result<u64> {
        unreachable!("only called from specializations")
    }
}

impl BufferedReaderSpec for &[u8] {
    fn buffer_size(&self) -> usize {
        // 优先选用这个特化，因为源“缓冲”就是我们所需的全部内容——哪怕它很小
        usize::MAX
    }

    fn copy_to(&mut self, to: &mut (impl Write + ?Sized)) -> Result<u64> {
        let len = self.len();
        to.write_all(self)?;
        *self = &self[len..];
        Ok(len as u64)
    }
}

impl<A: Allocator> BufferedReaderSpec for VecDeque<u8, A> {
    fn buffer_size(&self) -> usize {
        // 优先选用这个特化，因为源“缓冲”就是我们所需的全部内容——哪怕它很小
        usize::MAX
    }

    fn copy_to(&mut self, to: &mut (impl Write + ?Sized)) -> Result<u64> {
        let len = self.len();
        let (front, back) = self.as_slices();
        let bufs = &mut [IoSlice::new(front), IoSlice::new(back)];
        to.write_all_vectored(bufs)?;
        self.clear();
        Ok(len as u64)
    }
}

impl<I> BufferedReaderSpec for BufReader<I>
where
    Self: Read,
    I: ?Sized,
{
    fn buffer_size(&self) -> usize {
        self.capacity()
    }

    fn copy_to(&mut self, to: &mut (impl Write + ?Sized)) -> Result<u64> {
        let mut len = 0;

        loop {
            // Hack：这依赖于 `impl Read for BufReader` 在缓冲为空时总会调用 fill_buf
            // 这一行为——即便传入的是空切片也是如此。
            // 这里无法直接调用 fill_buf，因为特化（specialization）机制使我们无法再添加
            // I: Read 这一约束
            match self.read(&mut []) {
                Ok(_) => {}
                Err(e) if e.is_interrupted() => continue,
                Err(e) => return Err(e),
            }
            let buf = self.buffer();
            if self.buffer().len() == 0 {
                return Ok(len);
            }

            // 如果 writer 那一侧是 BufWriter，那么它的 write_all 实现了一项优化：会把大块
            // 缓冲直接透传给底层 writer。那条代码路径是 #[cold] 的，但我们在“缓冲输入”与
            // “缓冲输出”之间做复制时，仍然借此避免了多余的 memcopy。
            to.write_all(buf)?;
            len += buf.len() as u64;
            self.discard_buffer();
        }
    }
}

/// “读-写循环”的特化，它要么使用一块栈上缓冲，要么复用 BufWriter 的内部缓冲。
trait BufferedWriterSpec: Write {
    fn buffer_size(&self) -> usize;

    fn copy_from<R: Read + ?Sized>(&mut self, reader: &mut R) -> Result<u64>;
}

impl<W: Write + ?Sized> BufferedWriterSpec for W {
    #[inline]
    default fn buffer_size(&self) -> usize {
        0
    }

    default fn copy_from<R: Read + ?Sized>(&mut self, reader: &mut R) -> Result<u64> {
        stack_buffer_copy(reader, self)
    }
}

impl<I: Write + ?Sized> BufferedWriterSpec for BufWriter<I> {
    fn buffer_size(&self) -> usize {
        self.capacity()
    }

    fn copy_from<R: Read + ?Sized>(&mut self, reader: &mut R) -> Result<u64> {
        if self.capacity() < DEFAULT_BUF_SIZE {
            return stack_buffer_copy(reader, self);
        }

        let mut len = 0;
        let mut init = 0;

        loop {
            let buf = self.buffer_mut();
            let mut read_buf: BorrowedBuf<'_> = buf.spare_capacity_mut().into();

            unsafe {
                // SAFETY: init 要么是 0，要么是上一轮迭代得到的 init_len。
                read_buf.set_init(init);
            }

            if read_buf.capacity() >= DEFAULT_BUF_SIZE {
                let mut cursor = read_buf.unfilled();
                match reader.read_buf(cursor.reborrow()) {
                    Ok(()) => {
                        let bytes_read = cursor.written();

                        if bytes_read == 0 {
                            return Ok(len);
                        }

                        init = read_buf.init_len() - bytes_read;
                        len += bytes_read as u64;

                        // SAFETY: BorrowedBuf 保证它所有已填充的字节都是已初始化的
                        unsafe { buf.set_len(buf.len() + bytes_read) };

                        // 如果缓冲仍有足够容量，就再读一次，正如 BufWriter 自身会做的那样。
                        // 当 reader 返回短读（short reads）时就会发生这种情况
                    }
                    Err(ref e) if e.is_interrupted() => {}
                    Err(e) => return Err(e),
                }
            } else {
                // 缓冲中原本就有的所有字节都是已初始化的，在刷新缓冲时按此对待它们。
                init += buf.len();

                self.flush_buf()?;
            }
        }
    }
}

impl BufferedWriterSpec for Vec<u8> {
    fn buffer_size(&self) -> usize {
        cmp::max(DEFAULT_BUF_SIZE, self.capacity() - self.len())
    }

    fn copy_from<R: Read + ?Sized>(&mut self, reader: &mut R) -> Result<u64> {
        reader.read_to_end(self).map(|bytes| u64::try_from(bytes).expect("usize overflowed u64"))
    }
}

fn stack_buffer_copy<R: Read + ?Sized, W: Write + ?Sized>(
    reader: &mut R,
    writer: &mut W,
) -> Result<u64> {
    let buf: &mut [_] = &mut [MaybeUninit::uninit(); DEFAULT_BUF_SIZE];
    let mut buf: BorrowedBuf<'_> = buf.into();

    let mut len = 0;

    loop {
        match reader.read_buf(buf.unfilled()) {
            Ok(()) => {}
            Err(e) if e.is_interrupted() => continue,
            Err(e) => return Err(e),
        };

        if buf.filled().is_empty() {
            break;
        }

        len += buf.filled().len() as u64;
        writer.write_all(buf.filled())?;
        buf.clear();
    }

    Ok(len)
}
