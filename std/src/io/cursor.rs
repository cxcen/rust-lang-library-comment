#[cfg(test)]
mod tests;

use crate::alloc::Allocator;
use crate::cmp;
use crate::io::prelude::*;
use crate::io::{self, BorrowedCursor, ErrorKind, IoSlice, IoSliceMut, SeekFrom};

/// `Cursor` 包装一个内存中的缓冲，并为它提供一个 [`Seek`] 实现。
///
/// `Cursor` 与内存缓冲（任何实现了 <code>[AsRef]<\[u8]></code> 的类型）配合使用，使这些
/// 缓冲能够实现 [`Read`] 和/或 [`Write`]，从而让它们可以用在任何你本会使用“做真实 I/O 的
/// reader 或 writer”的地方。
///
/// 标准库为若干常被用作缓冲的类型实现了一些 I/O traits，例如 <code>Cursor<[Vec]\<u8>></code>
/// 和 <code>Cursor<[&\[u8\]][bytes]></code>。
///
/// # 示例
///
/// 在生产代码中我们可能想把字节写入一个 [`File`]，但在测试中却想用一块内存缓冲。借助
/// `Cursor` 我们可以做到这一点：
///
/// [bytes]: crate::slice "slice"
/// [`File`]: crate::fs::File
///
/// ```no_run
/// use std::io::prelude::*;
/// use std::io::{self, SeekFrom};
/// use std::fs::File;
///
/// // 我们写的一个库函数
/// fn write_ten_bytes_at_end<W: Write + Seek>(mut writer: W) -> io::Result<()> {
///     writer.seek(SeekFrom::End(-10))?;
///
///     for i in 0..10 {
///         writer.write(&[i])?;
///     }
///
///     // 一切顺利
///     Ok(())
/// }
///
/// # fn foo() -> io::Result<()> {
/// // 这是一段使用上述库函数的代码。
/// //
/// // 出于效率考虑我们这里也许会想用一个 BufReader，但为了让示例保持聚焦，就不用了。
/// let mut file = File::create("foo.txt")?;
/// // 首先，我们需要分配 10 个字节以便能向其中写入。
/// file.set_len(10)?;
///
/// write_ten_bytes_at_end(&mut file)?;
/// # Ok(())
/// # }
///
/// // 现在我们来写一个测试
/// #[test]
/// fn test_writes_bytes() {
///     // 创建一个真实的 File 比内存缓冲慢得多，所以我们改用 cursor
///     use std::io::Cursor;
///     let mut buff = Cursor::new(vec![0; 15]);
///
///     write_ten_bytes_at_end(&mut buff).unwrap();
///
///     assert_eq!(&buff.get_ref()[5..15], &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
/// }
/// ```
#[stable(feature = "rust1", since = "1.0.0")]
#[derive(Debug, Default, Eq, PartialEq)]
pub struct Cursor<T> {
    inner: T,
    pos: u64,
}

impl<T> Cursor<T> {
    /// 创建一个新的 cursor，包装所提供的底层内存缓冲。
    ///
    /// 即便底层缓冲（例如 [`Vec`]）非空，cursor 的初始位置也是 `0`。因此，向 cursor 写入
    /// 是从覆写 [`Vec`] 的内容开始的，而不是追加到其末尾。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::io::Cursor;
    ///
    /// let buff = Cursor::new(Vec::new());
    /// # fn force_inference(_: &Cursor<Vec<u8>>) {}
    /// # force_inference(&buff);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_io_structs", since = "1.79.0")]
    pub const fn new(inner: T) -> Cursor<T> {
        Cursor { pos: 0, inner }
    }

    /// 消耗这个 cursor，返回其底层值。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::io::Cursor;
    ///
    /// let buff = Cursor::new(Vec::new());
    /// # fn force_inference(_: &Cursor<Vec<u8>>) {}
    /// # force_inference(&buff);
    ///
    /// let vec = buff.into_inner();
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn into_inner(self) -> T {
        self.inner
    }

    /// 获取对这个 cursor 中底层值的不可变引用。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::io::Cursor;
    ///
    /// let buff = Cursor::new(Vec::new());
    /// # fn force_inference(_: &Cursor<Vec<u8>>) {}
    /// # force_inference(&buff);
    ///
    /// let reference = buff.get_ref();
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_io_structs", since = "1.79.0")]
    pub const fn get_ref(&self) -> &T {
        &self.inner
    }

    /// 获取对这个 cursor 中底层值的可变引用。
    ///
    /// 应当注意避免修改底层值的内部 I/O 状态，因为那可能会破坏（搞乱）这个 cursor 的位置。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::io::Cursor;
    ///
    /// let mut buff = Cursor::new(Vec::new());
    /// # fn force_inference(_: &Cursor<Vec<u8>>) {}
    /// # force_inference(&buff);
    ///
    /// let reference = buff.get_mut();
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_mut_cursor", since = "1.86.0")]
    pub const fn get_mut(&mut self) -> &mut T {
        &mut self.inner
    }

    /// 返回这个 cursor 的当前位置。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::io::Cursor;
    /// use std::io::prelude::*;
    /// use std::io::SeekFrom;
    ///
    /// let mut buff = Cursor::new(vec![1, 2, 3, 4, 5]);
    ///
    /// assert_eq!(buff.position(), 0);
    ///
    /// buff.seek(SeekFrom::Current(2)).unwrap();
    /// assert_eq!(buff.position(), 2);
    ///
    /// buff.seek(SeekFrom::Current(-1)).unwrap();
    /// assert_eq!(buff.position(), 1);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_io_structs", since = "1.79.0")]
    pub const fn position(&self) -> u64 {
        self.pos
    }

    /// 设置这个 cursor 的位置。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::io::Cursor;
    ///
    /// let mut buff = Cursor::new(vec![1, 2, 3, 4, 5]);
    ///
    /// assert_eq!(buff.position(), 0);
    ///
    /// buff.set_position(2);
    /// assert_eq!(buff.position(), 2);
    ///
    /// buff.set_position(4);
    /// assert_eq!(buff.position(), 4);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_mut_cursor", since = "1.86.0")]
    pub const fn set_position(&mut self, pos: u64) {
        self.pos = pos;
    }
}

impl<T> Cursor<T>
where
    T: AsRef<[u8]>,
{
    /// 在 cursor 位置处把底层切片一分为二，并返回这两部分。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(cursor_split)]
    /// use std::io::Cursor;
    ///
    /// let mut buff = Cursor::new(vec![1, 2, 3, 4, 5]);
    ///
    /// assert_eq!(buff.split(), ([].as_slice(), [1, 2, 3, 4, 5].as_slice()));
    ///
    /// buff.set_position(2);
    /// assert_eq!(buff.split(), ([1, 2].as_slice(), [3, 4, 5].as_slice()));
    ///
    /// buff.set_position(6);
    /// assert_eq!(buff.split(), ([1, 2, 3, 4, 5].as_slice(), [].as_slice()));
    /// ```
    #[unstable(feature = "cursor_split", issue = "86369")]
    pub fn split(&self) -> (&[u8], &[u8]) {
        let slice = self.inner.as_ref();
        let pos = self.pos.min(slice.len() as u64);
        slice.split_at(pos as usize)
    }
}

impl<T> Cursor<T>
where
    T: AsMut<[u8]>,
{
    /// 在 cursor 位置处把底层切片一分为二，并以可变方式返回这两部分。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(cursor_split)]
    /// use std::io::Cursor;
    ///
    /// let mut buff = Cursor::new(vec![1, 2, 3, 4, 5]);
    ///
    /// assert_eq!(buff.split_mut(), ([].as_mut_slice(), [1, 2, 3, 4, 5].as_mut_slice()));
    ///
    /// buff.set_position(2);
    /// assert_eq!(buff.split_mut(), ([1, 2].as_mut_slice(), [3, 4, 5].as_mut_slice()));
    ///
    /// buff.set_position(6);
    /// assert_eq!(buff.split_mut(), ([1, 2, 3, 4, 5].as_mut_slice(), [].as_mut_slice()));
    /// ```
    #[unstable(feature = "cursor_split", issue = "86369")]
    pub fn split_mut(&mut self) -> (&mut [u8], &mut [u8]) {
        let slice = self.inner.as_mut();
        let pos = self.pos.min(slice.len() as u64);
        slice.split_at_mut(pos as usize)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T> Clone for Cursor<T>
where
    T: Clone,
{
    #[inline]
    fn clone(&self) -> Self {
        Cursor { inner: self.inner.clone(), pos: self.pos }
    }

    #[inline]
    fn clone_from(&mut self, other: &Self) {
        self.inner.clone_from(&other.inner);
        self.pos = other.pos;
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T> io::Seek for Cursor<T>
where
    T: AsRef<[u8]>,
{
    fn seek(&mut self, style: SeekFrom) -> io::Result<u64> {
        let (base_pos, offset) = match style {
            SeekFrom::Start(n) => {
                self.pos = n;
                return Ok(n);
            }
            SeekFrom::End(n) => (self.inner.as_ref().len() as u64, n),
            SeekFrom::Current(n) => (self.pos, n),
        };
        match base_pos.checked_add_signed(offset) {
            Some(n) => {
                self.pos = n;
                Ok(self.pos)
            }
            None => Err(io::const_error!(
                ErrorKind::InvalidInput,
                "invalid seek to a negative or overflowing position",
            )),
        }
    }

    fn stream_len(&mut self) -> io::Result<u64> {
        Ok(self.inner.as_ref().len() as u64)
    }

    fn stream_position(&mut self) -> io::Result<u64> {
        Ok(self.pos)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T> Read for Cursor<T>
where
    T: AsRef<[u8]>,
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = Read::read(&mut Cursor::split(self).1, buf)?;
        self.pos += n as u64;
        Ok(n)
    }

    fn read_buf(&mut self, mut cursor: BorrowedCursor<'_>) -> io::Result<()> {
        let prev_written = cursor.written();

        Read::read_buf(&mut Cursor::split(self).1, cursor.reborrow())?;

        self.pos += (cursor.written() - prev_written) as u64;

        Ok(())
    }

    fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        let mut nread = 0;
        for buf in bufs {
            let n = self.read(buf)?;
            nread += n;
            if n < buf.len() {
                break;
            }
        }
        Ok(nread)
    }

    fn is_read_vectored(&self) -> bool {
        true
    }

    fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
        let result = Read::read_exact(&mut Cursor::split(self).1, buf);

        match result {
            Ok(_) => self.pos += buf.len() as u64,
            // 唯一可能出现的错误情形是 EOF，所以把 cursor 置于“EOF”处
            Err(_) => self.pos = self.inner.as_ref().len() as u64,
        }

        result
    }

    fn read_buf_exact(&mut self, mut cursor: BorrowedCursor<'_>) -> io::Result<()> {
        let prev_written = cursor.written();

        let result = Read::read_buf_exact(&mut Cursor::split(self).1, cursor.reborrow());
        self.pos += (cursor.written() - prev_written) as u64;

        result
    }

    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> io::Result<usize> {
        let content = Cursor::split(self).1;
        let len = content.len();
        buf.try_reserve(len)?;
        buf.extend_from_slice(content);
        self.pos += len as u64;

        Ok(len)
    }

    fn read_to_string(&mut self, buf: &mut String) -> io::Result<usize> {
        let content =
            crate::str::from_utf8(Cursor::split(self).1).map_err(|_| io::Error::INVALID_UTF8)?;
        let len = content.len();
        buf.try_reserve(len)?;
        buf.push_str(content);
        self.pos += len as u64;

        Ok(len)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T> BufRead for Cursor<T>
where
    T: AsRef<[u8]>,
{
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        Ok(Cursor::split(self).1)
    }
    fn consume(&mut self, amt: usize) {
        self.pos += amt as u64;
    }
}

// 不会改变大小（non-resizing）的 write 实现
#[inline]
fn slice_write(pos_mut: &mut u64, slice: &mut [u8], buf: &[u8]) -> io::Result<usize> {
    let pos = cmp::min(*pos_mut, slice.len() as u64);
    let amt = (&mut slice[(pos as usize)..]).write(buf)?;
    *pos_mut += amt as u64;
    Ok(amt)
}

#[inline]
fn slice_write_vectored(
    pos_mut: &mut u64,
    slice: &mut [u8],
    bufs: &[IoSlice<'_>],
) -> io::Result<usize> {
    let mut nwritten = 0;
    for buf in bufs {
        let n = slice_write(pos_mut, slice, buf)?;
        nwritten += n;
        if n < buf.len() {
            break;
        }
    }
    Ok(nwritten)
}

#[inline]
fn slice_write_all(pos_mut: &mut u64, slice: &mut [u8], buf: &[u8]) -> io::Result<()> {
    let n = slice_write(pos_mut, slice, buf)?;
    if n < buf.len() { Err(io::Error::WRITE_ALL_EOF) } else { Ok(()) }
}

#[inline]
fn slice_write_all_vectored(
    pos_mut: &mut u64,
    slice: &mut [u8],
    bufs: &[IoSlice<'_>],
) -> io::Result<()> {
    for buf in bufs {
        let n = slice_write(pos_mut, slice, buf)?;
        if n < buf.len() {
            return Err(io::Error::WRITE_ALL_EOF);
        }
    }
    Ok(())
}

/// 预留所需的空间，并在必要时用 0 对 vec 进行填充（padding）。
fn reserve_and_pad<A: Allocator>(
    pos_mut: &mut u64,
    vec: &mut Vec<u8, A>,
    buf_len: usize,
) -> io::Result<usize> {
    let pos: usize = (*pos_mut).try_into().map_err(|_| {
        io::const_error!(
            ErrorKind::InvalidInput,
            "cursor position exceeds maximum possible vector length",
        )
    })?;

    // 出于安全考虑，我们不希望这些数值发生溢出，否则我们的分配就会不够用
    let desired_cap = pos.saturating_add(buf_len);
    if desired_cap > vec.capacity() {
        // 我们希望 vec 的总容量能为 (pos+buf_len) 个字节腾出空间。reserve 是基于“在现有
        // 长度之外再增加多少元素”来分配的，所以我们需要预留两者之间的差值
        vec.reserve(desired_cap - vec.len());
    }
    // 如果 pos 超出了当前的 len，就进行填充。
    if pos > vec.len() {
        let diff = pos - vec.len();
        // 遗憾的是，`resize()` 本可以胜任，但优化器没意识到它内部所做的 `reserve` 是可以被
        // 消除的。所以我们手动来做，以消除那一个多余的分支
        let spare = vec.spare_capacity_mut();
        debug_assert!(spare.len() >= diff);
        // Safety: 我们已经为此分配了足够的容量。
        // 而且我们只是在写入，并不读取
        unsafe {
            spare.get_unchecked_mut(..diff).fill(core::mem::MaybeUninit::new(0));
            vec.set_len(pos);
        }
    }

    Ok(pos)
}

/// 在不分配的前提下把切片写入 vec。
///
/// # Safety
///
/// `vec` 必须拥有 `buf.len()` 这么多的空闲容量。
unsafe fn vec_write_all_unchecked<A>(pos: usize, vec: &mut Vec<u8, A>, buf: &[u8]) -> usize
where
    A: Allocator,
{
    debug_assert!(vec.capacity() >= pos + buf.len());
    unsafe { vec.as_mut_ptr().add(pos).copy_from(buf.as_ptr(), buf.len()) };
    pos + buf.len()
}

/// 为 [`Cursor`] 实现的、会改变大小（resizing）的 `write_all`。
///
/// 允许 Cursor 拥有一个已预先分配并初始化的 vector 内容、但位置却为 0。这意味着 [`Write`]
/// 将覆写 vec 的内容。
///
/// 这也允许 vec 的内容为空、但位置却为 N。这意味着 [`Write`] 会先用 0 对 vec 进行填充，
/// 然后再从那个位置开始写入任何东西
fn vec_write_all<A>(pos_mut: &mut u64, vec: &mut Vec<u8, A>, buf: &[u8]) -> io::Result<usize>
where
    A: Allocator,
{
    let buf_len = buf.len();
    let mut pos = reserve_and_pad(pos_mut, vec, buf_len)?;

    // 写入 buf，然后在必要时让 vec 向前推进
    // Safety: 我们已确保容量可用，且直到 pos 为止的所有字节都会被写入
    unsafe {
        pos = vec_write_all_unchecked(pos, vec, buf);
        if pos > vec.len() {
            vec.set_len(pos);
        }
    };

    // 让我们向前推进
    *pos_mut += buf_len as u64;
    Ok(buf_len)
}

/// 为 [`Cursor`] 实现的、会改变大小（resizing）的 `write_all_vectored`。
///
/// 允许 Cursor 拥有一个已预先分配并初始化的 vector 内容、但位置却为 0。这意味着 [`Write`]
/// 将覆写 vec 的内容。
///
/// 这也允许 vec 的内容为空、但位置却为 N。这意味着 [`Write`] 会先用 0 对 vec 进行填充，
/// 然后再从那个位置开始写入任何东西
fn vec_write_all_vectored<A>(
    pos_mut: &mut u64,
    vec: &mut Vec<u8, A>,
    bufs: &[IoSlice<'_>],
) -> io::Result<usize>
where
    A: Allocator,
{
    // 出于安全考虑，我们绝不希望这个累加和发生溢出。
    // 如果它发生饱和（saturate），reserve 应当 panic，以避免任何不可靠（unsound）的写入。
    let buf_len = bufs.iter().fold(0usize, |a, b| a.saturating_add(b.len()));
    let mut pos = reserve_and_pad(pos_mut, vec, buf_len)?;

    // 写入 buf，然后在必要时让 vec 向前推进
    // Safety: 我们已确保容量可用，且直到最后一个 pos 为止的所有字节都会被写入
    unsafe {
        for buf in bufs {
            pos = vec_write_all_unchecked(pos, vec, buf);
        }
        if pos > vec.len() {
            vec.set_len(pos);
        }
    }

    // 让我们向前推进
    *pos_mut += buf_len as u64;
    Ok(buf_len)
}

#[stable(feature = "rust1", since = "1.0.0")]
impl Write for Cursor<&mut [u8]> {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        slice_write(&mut self.pos, self.inner, buf)
    }

    #[inline]
    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        slice_write_vectored(&mut self.pos, self.inner, bufs)
    }

    #[inline]
    fn is_write_vectored(&self) -> bool {
        true
    }

    #[inline]
    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        slice_write_all(&mut self.pos, self.inner, buf)
    }

    #[inline]
    fn write_all_vectored(&mut self, bufs: &mut [IoSlice<'_>]) -> io::Result<()> {
        slice_write_all_vectored(&mut self.pos, self.inner, bufs)
    }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[stable(feature = "cursor_mut_vec", since = "1.25.0")]
impl<A> Write for Cursor<&mut Vec<u8, A>>
where
    A: Allocator,
{
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        vec_write_all(&mut self.pos, self.inner, buf)
    }

    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        vec_write_all_vectored(&mut self.pos, self.inner, bufs)
    }

    #[inline]
    fn is_write_vectored(&self) -> bool {
        true
    }

    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        vec_write_all(&mut self.pos, self.inner, buf)?;
        Ok(())
    }

    fn write_all_vectored(&mut self, bufs: &mut [IoSlice<'_>]) -> io::Result<()> {
        vec_write_all_vectored(&mut self.pos, self.inner, bufs)?;
        Ok(())
    }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<A> Write for Cursor<Vec<u8, A>>
where
    A: Allocator,
{
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        vec_write_all(&mut self.pos, &mut self.inner, buf)
    }

    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        vec_write_all_vectored(&mut self.pos, &mut self.inner, bufs)
    }

    #[inline]
    fn is_write_vectored(&self) -> bool {
        true
    }

    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        vec_write_all(&mut self.pos, &mut self.inner, buf)?;
        Ok(())
    }

    fn write_all_vectored(&mut self, bufs: &mut [IoSlice<'_>]) -> io::Result<()> {
        vec_write_all_vectored(&mut self.pos, &mut self.inner, bufs)?;
        Ok(())
    }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[stable(feature = "cursor_box_slice", since = "1.5.0")]
impl<A> Write for Cursor<Box<[u8], A>>
where
    A: Allocator,
{
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        slice_write(&mut self.pos, &mut self.inner, buf)
    }

    #[inline]
    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        slice_write_vectored(&mut self.pos, &mut self.inner, bufs)
    }

    #[inline]
    fn is_write_vectored(&self) -> bool {
        true
    }

    #[inline]
    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        slice_write_all(&mut self.pos, &mut self.inner, buf)
    }

    #[inline]
    fn write_all_vectored(&mut self, bufs: &mut [IoSlice<'_>]) -> io::Result<()> {
        slice_write_all_vectored(&mut self.pos, &mut self.inner, bufs)
    }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[stable(feature = "cursor_array", since = "1.61.0")]
impl<const N: usize> Write for Cursor<[u8; N]> {
    #[inline]
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        slice_write(&mut self.pos, &mut self.inner, buf)
    }

    #[inline]
    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        slice_write_vectored(&mut self.pos, &mut self.inner, bufs)
    }

    #[inline]
    fn is_write_vectored(&self) -> bool {
        true
    }

    #[inline]
    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        slice_write_all(&mut self.pos, &mut self.inner, buf)
    }

    #[inline]
    fn write_all_vectored(&mut self, bufs: &mut [IoSlice<'_>]) -> io::Result<()> {
        slice_write_all_vectored(&mut self.pos, &mut self.inner, bufs)
    }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
