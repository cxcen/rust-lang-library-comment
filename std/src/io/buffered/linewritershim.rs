use core::slice::memchr;

use crate::io::{self, BufWriter, IoSlice, Write};

/// 用于实现行缓冲写入逻辑的私有辅助结构体。
///
/// 这个 shim（垫片）临时包装一个 BufWriter，并借助它的内部机制来实现一个行缓冲写入器
///（具体来说，是通过使用 write_to_buf、flush_buf 这类内部方法）。如此一来，相比一个只能
/// 访问 `write` 和 `flush` 的实现，就能构建出一个更高效的抽象，同时又无需把 BufWriter 的
/// 大量实现细节毫无必要地重复一遍。这还使得已有的 `BufWriter` 可以被临时赋予行缓冲逻辑；
/// 正是这一点让 Stdout 能够在行缓冲模式与块缓冲模式之间切换。
#[derive(Debug)]
pub struct LineWriterShim<'a, W: ?Sized + Write> {
    buffer: &'a mut BufWriter<W>,
}

impl<'a, W: ?Sized + Write> LineWriterShim<'a, W> {
    pub fn new(buffer: &'a mut BufWriter<W>) -> Self {
        Self { buffer }
    }

    /// 获取对内部 writer（也就是被 BufWriter 包装的那个 writer）的不可变引用。
    fn inner(&self) -> &W {
        self.buffer.get_ref()
    }

    /// 获取对内部 writer（也就是被 BufWriter 包装的那个 writer）的可变引用。使用这个
    /// writer 时要小心，因为对它的写入会绕过缓冲。
    fn inner_mut(&mut self) -> &mut W {
        self.buffer.get_mut()
    }

    /// 获取当前缓冲在 self.buffer 中的内容
    fn buffered(&self) -> &[u8] {
        self.buffer.buffer()
    }

    /// 当且仅当最后一个字节是换行符时刷新缓冲（这表明此前某次写入只取得了部分成功，我们希望
    /// 在继续后续写入之前，先重试把这一行已缓冲的内容刷新出去）。
    fn flush_if_completed_line(&mut self) -> io::Result<()> {
        match self.buffered().last().copied() {
            Some(b'\n') => self.buffer.flush_buf(),
            _ => Ok(()),
        }
    }
}

impl<'a, W: ?Sized + Write> Write for LineWriterShim<'a, W> {
    /// 以行缓冲的方式把一些数据写入这个 BufWriter。
    ///
    /// 这意味着：如果数据中存在任何换行符，那么直到最后一个换行符（含）为止的数据会被直接
    /// 发送给底层 writer，而它之后的数据则被缓冲起来。返回写入的字节数。
    ///
    /// 本函数以“尽力而为（best effort basis）”的方式运作；遵循 `Write::write` 的约定，
    /// 它至多对底层 writer 尝试一次写入新数据。如果那次写入只报告了部分成功，剩余的数据将
    /// 被缓冲。
    ///
    /// 因为本函数会尝试把完整的行发送给底层 writer，所以如果现有缓冲以换行符结尾，它也会刷新
    /// 该缓冲——即便传入的数据本身不含任何换行符。
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let newline_idx = match memchr::memrchr(b'\n', buf) {
            // 如果没有新的换行符（也就是说，这次写入不足一整行），就做一次普通的缓冲写入
            //（如果超出了内部缓冲的大小，这次写入可能会触发刷新）
            None => {
                self.flush_if_completed_line()?;
                return self.buffer.write(buf);
            }
            // 否则，安排把这些行直接写到内部 writer。
            Some(newline_idx) => newline_idx + 1,
        };

        // 刷新现有内容，为我们这次写入做准备。我们必须在尝试写入 `buf` 之前完成这一步，以便
        // 维持一致性；如果我们先把 `buf` 加进缓冲、再一次性把它们全部刷新，那我们就有义务返回
        // Ok()，而这将意味着刷新过程中发生的任何错误都会被压制（吞掉）。
        self.buffer.flush_buf()?;

        // 这就是我们打算直接写到内部 writer 的部分。如果一切顺利，其余部分将被缓冲。
        let lines = &buf[..newline_idx];

        // 把 `lines` 直接写到内部 writer。遵循 `write` 的约定，至多尝试一次添加新的（未缓冲的）
        // 数据。因为这次写入不会直接触及 BufWriter 的状态，且已知缓冲为空，所以这里我们无需
        // 操心 self.buffer.panicked。
        let flushed = self.inner_mut().write(lines)?;

        // 如果 buffer 返回 Ok(0)，就把它原样传播给调用方，不做额外的缓冲；否则我们只是
        // 在为之后的某个 "ErrorKind::WriteZero" 埋下伏笔罢了。
        if flushed == 0 {
            return Ok(0);
        }

        // 既然写入已经成功，就把其余部分缓冲起来（或尽可能多地缓冲）。如果还有未写出的换行符，
        // 我们只缓冲到“能装进缓冲的、最后一个未写出换行符”为止；这有助于避免在后续对
        // LineWriterShim::write 的调用中刷新出不完整的行。

        // 按从最常见到最不常见的顺序处理各种情形，前提假设是：大多数写入都能全部成功，且大多数
        // 写入都比缓冲要小。
        // - 这是不是一行的一部分（即未写出的尾部里已没有换行符）
        // - 如果不是，那么直到最后一个未写出换行符为止的数据，能否装进缓冲？
        // - 如果还是不行，就扫描出“确实能装进缓冲”的最后一个换行符
        let tail = if flushed >= newline_idx {
            let tail = &buf[flushed..];
            // 如果剩余字节比缓冲还大，就不要把它们拆分，从而避免不必要的短写（short write）。
            // 它们可以由下一次 write 调用一次性完整写出。
            if tail.len() >= self.buffer.capacity() {
                return Ok(flushed);
            }
            tail
        } else if newline_idx - flushed <= self.buffer.capacity() {
            &buf[flushed..newline_idx]
        } else {
            let scan_area = &buf[flushed..];
            let scan_area = &scan_area[..self.buffer.capacity()];
            match memchr::memrchr(b'\n', scan_area) {
                Some(newline_idx) => &scan_area[..newline_idx + 1],
                None => scan_area,
            }
        };

        let buffered = self.buffer.write_to_buf(tail);
        Ok(flushed + buffered)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.buffer.flush()
    }

    /// 以行缓冲的方式把一些向量化（vectored）数据写入这个 BufWriter。
    ///
    /// 这意味着：如果数据中存在任何换行符，那么直到“含有最后一个换行符的那个缓冲”（含）为止
    /// 的数据会被直接发送给内部 writer，而它之后的数据则被缓冲起来。返回写入的字节数。
    ///
    /// 本函数以“尽力而为（best effort basis）”的方式运作；遵循 `Write::write` 的约定，
    /// 它至多对底层 writer 尝试一次写入新数据。
    ///
    /// 因为本函数会尝试把完整的行发送给底层 writer，所以如果现有缓冲含有任何换行符，它也会
    /// 刷新该缓冲。
    ///
    /// 由于在一个 `IoSlice` 数组中梳理（分类处理）会有些繁琐，本方法在以下几个方面与 write
    /// 不同：
    ///
    /// - 它尝试写出“直到含有最后一个换行符的那个缓冲（含）”为止所有缓冲的全部内容。这意味着，
    ///   如果那个缓冲在换行符之后还有数据，它可能会写出一行的一部分。
    /// - 如果写入只报告了部分成功，它不会去定位已写出字节的精确位置、再缓冲剩余部分。
    ///
    /// 如果底层 vector 不支持向量化写入，我们就转而简单地用 `write` 写出第一个非空缓冲。
    /// 这样一来，我们既享受到了更细粒度的“部分行处理”的好处，又不会在效率上有任何损失。
    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        // 如果 write_vectored 没有特化的行为，就直接用 write。这样做的好处是能获得更细粒度的
        // “部分行处理”。
        if !self.is_write_vectored() {
            return match bufs.iter().find(|buf| !buf.is_empty()) {
                Some(buf) => self.write(buf),
                None => Ok(0),
            };
        }

        // 找出含有最后一个换行符的那个缓冲
        // FIXME: 如果缓冲非常多、且没有任何一个含换行符，这里就会过慢。例如，Linux 上的
        // writev() 至多只写出 1024 个切片，所以扫描其余部分是白费力气。这会使
        // write_all_vectored() 变成二次复杂度（quadratic）。
        let last_newline_buf_idx = bufs
            .iter()
            .enumerate()
            .rev()
            .find_map(|(i, buf)| memchr::memchr(b'\n', buf).map(|_| i));

        // 如果没有新的换行符（也就是说，这次写入不足一整行），就做一次普通的缓冲写入
        let last_newline_buf_idx = match last_newline_buf_idx {
            // 没有换行符；只做一次普通的缓冲写入
            None => {
                self.flush_if_completed_line()?;
                return self.buffer.write_vectored(bufs);
            }
            Some(i) => i,
        };

        // 刷新现有内容，为我们这次写入做准备
        self.buffer.flush_buf()?;

        // 这就是我们打算直接写到内部 writer 的部分。如果一切顺利，其余部分将被缓冲。
        let (lines, tail) = bufs.split_at(last_newline_buf_idx + 1);

        // 把 `lines` 直接写到内部 writer。遵循 `write` 的约定，至多尝试一次添加新的（未缓冲的）
        // 数据。因为这次写入不会直接触及 BufWriter 的状态，且已知缓冲为空，所以这里我们无需
        // 操心 self.panicked。
        let flushed = self.inner_mut().write_vectored(lines)?;

        // 如果 inner 返回 Ok(0)，就把它原样传播给调用方，不做额外的缓冲；否则我们只是
        // 在为之后的某个 "ErrorKind::WriteZero" 埋下伏笔罢了。
        if flushed == 0 {
            return Ok(0);
        }

        // 不要试图重建出精确的写出数量；一旦遇到部分写入就直接放弃（返回）。
        let mut lines_len: usize = 0;
        for buf in lines {
            // 在切片相互重叠/重复的情况下，理论上总长度可能超过 usize::MAX
            lines_len = lines_len.saturating_add(buf.len());
            if flushed < lines_len {
                return Ok(flushed);
            }
        }

        // 既然写入已经成功，就把其余部分缓冲起来（或尽可能多地缓冲）
        let buffered: usize = tail
            .iter()
            .filter(|buf| !buf.is_empty())
            .map(|buf| self.buffer.write_to_buf(buf))
            .take_while(|&n| n > 0)
            .sum();

        Ok(flushed + buffered)
    }

    fn is_write_vectored(&self) -> bool {
        self.inner().is_write_vectored()
    }

    /// 以行缓冲的方式把一些数据写入这个 BufWriter。
    ///
    /// 这意味着：如果数据中存在任何换行符，那么直到最后一个换行符为止的数据会被直接发送给
    /// 底层 writer，而它之后的数据则被缓冲起来。
    ///
    /// 因为本函数会尝试把完整的行发送给底层 writer，所以如果现有缓冲含有任何换行符，它也会
    /// 刷新该缓冲——即便传入的数据本身不含任何换行符。
    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        match memchr::memrchr(b'\n', buf) {
            // 如果没有新的换行符（也就是说，这次写入不足一整行），就做一次普通的缓冲写入
            //（如果超出了内部缓冲的大小，这次写入可能会触发刷新）
            None => {
                self.flush_if_completed_line()?;
                self.buffer.write_all(buf)
            }
            Some(newline_idx) => {
                let (lines, tail) = buf.split_at(newline_idx + 1);

                if self.buffered().is_empty() {
                    self.inner_mut().write_all(lines)?;
                } else {
                    // 如果已有任何已缓冲的数据，我们就先把传入的这些行加到那块缓冲里，再做刷新，
                    // 这样至少能省下一次 write 调用。在 `write` 中我们没法真的这么做，因为我们
                    // 无法做到：既这么做、又不压制错误、还能通过返回值向调用方报告一个一致的
                    // 状态——三者兼得。但在 write_all 这里，这么做没问题。
                    self.buffer.write_all(lines)?;
                    self.buffer.flush_buf()?;
                }

                self.buffer.write_all(tail)
            }
        }
    }
}
