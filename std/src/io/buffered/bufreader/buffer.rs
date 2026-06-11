//! 对 `BufReader` 缓冲管理逻辑的封装。
//!
//! 本模块把 `BufReader` 的基础功能单独抽离出来，目的是保护两条核心不变量：
//! * `buf` 中前 `filled` 个字节始终是已初始化的
//! * `pos` 始终满足 `pos <= filled`
//! 由于本模块封装了缓冲管理逻辑，我们可以保证 `pos..filled` 这个区间永远是缓冲中已初始化
//! 区域的合法下标范围。这意味着：用户代码若想通过 `buffer` + `consume` 的方式从 `BufReader`
//! 读取数据，可以在不触发任何运行期边界检查的情况下完成。

use crate::cmp;
use crate::io::{self, BorrowedBuf, ErrorKind, Read};
use crate::mem::MaybeUninit;

pub struct Buffer {
    // 缓冲本身。
    buf: Box<[MaybeUninit<u8>]>,
    // `buf` 中当前的读取偏移（seek offset），必须始终 <= `filled`。
    pos: usize,
    // 每次调用 `fill_buf` 都会设置 `filled`，用以表示 `buf` 起始处有多少字节已被某次读取填入
    // 实际数据。
    filled: usize,
    // 这是历次 `fill_buf` 调用所返回过的最大字节数。我们之所以跟踪它，是为了能准确地告诉
    // `read_buf` 缓冲中有多少字节已经初始化，从而尽可能跳过它内部那套防御性的初始化逻辑。
    // 注意：尽管这个值常常与 `filled` 相同，但二者并不必然相等。`fill_buf` 的调用并不要求
    // 真的把整个缓冲填满；而如果省略这个字段，对于那些不会填满缓冲的 `Read` 实现来说会造成
    // 巨大的性能退化。
    initialized: usize,
}

impl Buffer {
    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        let buf = Box::new_uninit_slice(capacity);
        Self { buf, pos: 0, filled: 0, initialized: 0 }
    }

    #[inline]
    pub fn try_with_capacity(capacity: usize) -> io::Result<Self> {
        match Box::try_new_uninit_slice(capacity) {
            Ok(buf) => Ok(Self { buf, pos: 0, filled: 0, initialized: 0 }),
            Err(_) => {
                Err(io::const_error!(ErrorKind::OutOfMemory, "failed to allocate read buffer"))
            }
        }
    }

    #[inline]
    pub fn buffer(&self) -> &[u8] {
        // SAFETY: self.pos 和 self.filled 都是合法的，且 self.filled >= self.pos，
        // 而那段区域是已初始化的——因为这些都是本类型的不变量。
        unsafe { self.buf.get_unchecked(self.pos..self.filled).assume_init_ref() }
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        self.buf.len()
    }

    #[inline]
    pub fn filled(&self) -> usize {
        self.filled
    }

    #[inline]
    pub fn pos(&self) -> usize {
        self.pos
    }

    // 这个方法仅被一个测试使用，该测试用于断言“初始化跟踪”逻辑是正确的。
    #[cfg(test)]
    pub fn initialized(&self) -> usize {
        self.initialized
    }

    #[inline]
    pub fn discard_buffer(&mut self) {
        self.pos = 0;
        self.filled = 0;
    }

    #[inline]
    pub fn consume(&mut self, amt: usize) {
        self.pos = cmp::min(self.pos + amt, self.filled);
    }

    /// 如果缓冲中有 `amt` 个可用字节，就把包含这些字节的切片传给 `visitor` 并返回 true。
    /// 如果可用字节不足，则返回 false。
    #[inline]
    pub fn consume_with<V>(&mut self, amt: usize, mut visitor: V) -> bool
    where
        V: FnMut(&[u8]),
    {
        if let Some(claimed) = self.buffer().get(..amt) {
            visitor(claimed);
            // 如果对 self.buffer() 的索引成功，那么 amt 必定是一个合法的增量。
            self.pos += amt;
            true
        } else {
            false
        }
    }

    #[inline]
    pub fn unconsume(&mut self, amt: usize) {
        self.pos = self.pos.saturating_sub(amt);
    }

    /// 在不丢弃缓冲已有内容的前提下，把更多字节读入缓冲
    pub fn read_more(&mut self, mut reader: impl Read) -> io::Result<usize> {
        let mut buf = BorrowedBuf::from(&mut self.buf[self.filled..]);
        let old_init = self.initialized - self.filled;
        unsafe {
            buf.set_init(old_init);
        }
        reader.read_buf(buf.unfilled())?;
        self.filled += buf.len();
        self.initialized += buf.init_len() - old_init;
        Ok(buf.len())
    }

    /// 把那些已经读取过的字节从缓冲中移除（通过把未读部分整体前移到缓冲头部）。
    pub fn backshift(&mut self) {
        self.buf.copy_within(self.pos..self.filled, 0);
        self.filled -= self.pos;
        self.pos = 0;
    }

    #[inline]
    pub fn fill_buf(&mut self, mut reader: impl Read) -> io::Result<&[u8]> {
        // 如果我们已经读到了内部缓冲的末尾，那么就需要从底层 reader 再获取一些数据。
        // 这里用 `>=` 而非更精确的 `==` 来分支判断，是为了告诉编译器 pos..cap 这段切片
        // 始终是合法的。
        if self.pos >= self.filled {
            debug_assert!(self.pos == self.filled);

            let mut buf = BorrowedBuf::from(&mut *self.buf);
            // SAFETY: `self.filled` 个字节始终是已经初始化过的。
            unsafe {
                buf.set_init(self.initialized);
            }

            let result = reader.read_buf(buf.unfilled());

            self.pos = 0;
            self.filled = buf.len();
            self.initialized = buf.init_len();

            result?;
        }
        Ok(self.buffer())
    }
}
