use crate::io;
use crate::sys::{FromInner, IntoInner, pipe as imp};

/// 创建一个匿名管道（anonymous pipe）。
///
/// # Behavior
///
/// 管道是由操作系统提供的单向数据通道，可跨进程工作。管道通常用于在两个或多个相互独立的进程
/// 之间通信，因为在单个进程内部有更好、更快的通信方式。
///
/// 具体来说：
///
/// * 对 [`PipeReader`] 的读取会阻塞，直到管道非空。
/// * 对 [`PipeWriter`] 的写入会在管道已满时阻塞。
/// * 当某个 [`PipeWriter`] 的所有副本都被关闭后，对相应 [`PipeReader`] 的读取会返回 EOF。
/// * [`PipeWriter`] 可以被共享，多个进程或线程可以同时向它写入，但写入（超过某个由目标平台
///   决定的阈值时）的数据可能会相互交错（interleaved）。
/// * [`PipeReader`] 可以被共享，多个进程或线程可以同时读取它。任意给定的字节只会被一个
///   reader 消费。对于数据交错不作任何保证。
/// * 可移植的应用程序不能假设大于单个字节的消息具有任何原子性（atomicity）。
///
/// # Platform-specific behavior
///
/// 本函数当前在 Unix 上对应 `pipe` 函数，在 Windows 上对应 `CreatePipe` 函数。
///
/// 注意：这[将来可能会改变][changes]。
///
/// # Capacity
///
/// 管道容量与平台相关。引用 Linux 的 [man page]：
///
/// > 不同的实现对管道容量有不同的限制。应用程序不应依赖某个特定的容量：应用程序的设计应当做到
/// > 让“读取进程”一旦有数据可用就立即消费，从而使“写入进程”不会一直被阻塞。
///
/// # Example
///
/// ```no_run
/// # #[cfg(miri)] fn main() {}
/// # #[cfg(not(miri))]
/// # fn main() -> std::io::Result<()> {
/// use std::io::{Read, Write, pipe};
/// use std::process::Command;
/// let (ping_reader, mut ping_writer) = pipe()?;
/// let (mut pong_reader, pong_writer) = pipe()?;
///
/// // 派生（spawn）一个子进程，它会回显（echo）自己的输入。
/// let mut echo_command = Command::new("cat");
/// echo_command.stdin(ping_reader);
/// echo_command.stdout(pong_writer);
/// let mut echo_child = echo_command.spawn()?;
///
/// // 向子进程发送输入。注意：由于我们是在读取任何输出之前就把全部输入写出，如果子进程的输入
/// // 和输出管道缓冲都被填满，这里可能会死锁。这些缓冲通常至少有几 KB，所以 "hello" 没问题，
/// // 但对于更长的输入，我们就需要同时进行读和写，例如借助线程。
/// ping_writer.write_all(b"hello")?;
///
/// // `cat` 在从 stdin 读到 EOF 时退出，但只要还有任何 ping writer 处于打开状态，EOF 就
/// // 不会发生。我们需要 drop 掉自己的 ping writer，否则下面的 read_to_string 会死锁。
/// drop(ping_writer);
///
/// // 只要还有任何 pong writer 处于打开状态，pong reader 就无法报告 EOF。我们的 Command
/// // 对象正持有一个 pong writer，同样地，如果我们不 drop 它，read_to_string 就会死锁。
/// drop(echo_command);
///
/// let mut buf = String::new();
/// // 阻塞，直到 `cat` 关闭它的 stdout（一个 pong writer）。
/// pong_reader.read_to_string(&mut buf)?;
/// assert_eq!(&buf, "hello");
///
/// // 此时我们知道 `cat` 已经退出，但我们仍需 wait 以清理掉那个“僵尸（zombie）”进程。
/// echo_child.wait()?;
/// # Ok(())
/// # }
/// ```
/// [changes]: io#platform-specific-behavior
/// [man page]: https://man7.org/linux/man-pages/man7/pipe.7.html
#[stable(feature = "anonymous_pipe", since = "1.87.0")]
#[inline]
pub fn pipe() -> io::Result<(PipeReader, PipeWriter)> {
    imp::pipe().map(|(reader, writer)| (PipeReader(reader), PipeWriter(writer)))
}

/// 匿名管道的读取端。
#[stable(feature = "anonymous_pipe", since = "1.87.0")]
#[derive(Debug)]
pub struct PipeReader(pub(crate) imp::Pipe);

/// 匿名管道的写入端。
#[stable(feature = "anonymous_pipe", since = "1.87.0")]
#[derive(Debug)]
pub struct PipeWriter(pub(crate) imp::Pipe);

impl FromInner<imp::Pipe> for PipeReader {
    fn from_inner(inner: imp::Pipe) -> Self {
        Self(inner)
    }
}

impl IntoInner<imp::Pipe> for PipeReader {
    fn into_inner(self) -> imp::Pipe {
        self.0
    }
}

impl FromInner<imp::Pipe> for PipeWriter {
    fn from_inner(inner: imp::Pipe) -> Self {
        Self(inner)
    }
}

impl IntoInner<imp::Pipe> for PipeWriter {
    fn into_inner(self) -> imp::Pipe {
        self.0
    }
}

impl PipeReader {
    /// 创建一个新的 [`PipeReader`] 实例，它与原实例共享同一个底层文件描述（file description）。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// # #[cfg(miri)] fn main() {}
    /// # #[cfg(not(miri))]
    /// # fn main() -> std::io::Result<()> {
    /// use std::fs;
    /// use std::io::{pipe, Write};
    /// use std::process::Command;
    /// const NUM_SLOT: u8 = 2;
    /// const NUM_PROC: u8 = 5;
    /// const OUTPUT: &str = "work.txt";
    ///
    /// let mut jobs = vec![];
    /// let (reader, mut writer) = pipe()?;
    ///
    /// // 向管道写入 NUM_SLOT 个字符。
    /// writer.write_all(&[b'|'; NUM_SLOT as usize])?;
    ///
    /// // 派生若干进程，它们从管道读取一个字符、做一些工作、再写回管道。当管道为空时，这些
    /// // 进程会阻塞，因此在任意时刻最多只有 NUM_SLOT 个进程在工作。
    /// for _ in 0..NUM_PROC {
    ///     jobs.push(
    ///         Command::new("bash")
    ///             .args(["-c",
    ///                 &format!(
    ///                      "read -n 1\n\
    ///                       echo -n 'x' >> '{OUTPUT}'\n\
    ///                       echo -n '|'",
    ///                 ),
    ///             ])
    ///             .stdin(reader.try_clone()?)
    ///             .stdout(writer.try_clone()?)
    ///             .spawn()?,
    ///     );
    /// }
    ///
    /// // 等待所有 job 完成。
    /// for mut job in jobs {
    ///     job.wait()?;
    /// }
    ///
    /// // 检查工作结果并做清理。
    /// let xs = fs::read_to_string(OUTPUT)?;
    /// fs::remove_file(OUTPUT)?;
    /// assert_eq!(xs, "x".repeat(NUM_PROC.into()));
    /// # Ok(())
    /// # }
    /// ```
    #[stable(feature = "anonymous_pipe", since = "1.87.0")]
    pub fn try_clone(&self) -> io::Result<Self> {
        self.0.try_clone().map(Self)
    }
}

impl PipeWriter {
    /// 创建一个新的 [`PipeWriter`] 实例，它与原实例共享同一个底层文件描述（file description）。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// # #[cfg(miri)] fn main() {}
    /// # #[cfg(not(miri))]
    /// # fn main() -> std::io::Result<()> {
    /// use std::process::Command;
    /// use std::io::{pipe, Read};
    /// let (mut reader, writer) = pipe()?;
    ///
    /// // 派生一个进程，它会向 stdout 和 stderr 写入数据。
    /// let mut peer = Command::new("bash")
    ///     .args([
    ///         "-c",
    ///         "echo -n foo\n\
    ///          echo -n bar >&2"
    ///     ])
    ///     .stdout(writer.try_clone()?)
    ///     .stderr(writer)
    ///     .spawn()?;
    ///
    /// // 读取并检查结果。
    /// let mut msg = String::new();
    /// reader.read_to_string(&mut msg)?;
    /// assert_eq!(&msg, "foobar");
    ///
    /// peer.wait()?;
    /// # Ok(())
    /// # }
    /// ```
    #[stable(feature = "anonymous_pipe", since = "1.87.0")]
    pub fn try_clone(&self) -> io::Result<Self> {
        self.0.try_clone().map(Self)
    }
}

#[stable(feature = "anonymous_pipe", since = "1.87.0")]
impl io::Read for &PipeReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.read(buf)
    }
    fn read_vectored(&mut self, bufs: &mut [io::IoSliceMut<'_>]) -> io::Result<usize> {
        self.0.read_vectored(bufs)
    }
    #[inline]
    fn is_read_vectored(&self) -> bool {
        self.0.is_read_vectored()
    }
    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> io::Result<usize> {
        self.0.read_to_end(buf)
    }
    fn read_buf(&mut self, buf: io::BorrowedCursor<'_>) -> io::Result<()> {
        self.0.read_buf(buf)
    }
}

#[stable(feature = "anonymous_pipe", since = "1.87.0")]
impl io::Read for PipeReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.read(buf)
    }
    fn read_vectored(&mut self, bufs: &mut [io::IoSliceMut<'_>]) -> io::Result<usize> {
        self.0.read_vectored(bufs)
    }
    #[inline]
    fn is_read_vectored(&self) -> bool {
        self.0.is_read_vectored()
    }
    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> io::Result<usize> {
        self.0.read_to_end(buf)
    }
    fn read_buf(&mut self, buf: io::BorrowedCursor<'_>) -> io::Result<()> {
        self.0.read_buf(buf)
    }
}

#[stable(feature = "anonymous_pipe", since = "1.87.0")]
impl io::Write for &PipeWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }
    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
    fn write_vectored(&mut self, bufs: &[io::IoSlice<'_>]) -> io::Result<usize> {
        self.0.write_vectored(bufs)
    }
    #[inline]
    fn is_write_vectored(&self) -> bool {
        self.0.is_write_vectored()
    }
}

#[stable(feature = "anonymous_pipe", since = "1.87.0")]
impl io::Write for PipeWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }
    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
    fn write_vectored(&mut self, bufs: &[io::IoSlice<'_>]) -> io::Result<usize> {
        self.0.write_vectored(bufs)
    }
    #[inline]
    fn is_write_vectored(&self) -> bool {
        self.0.is_write_vectored()
    }
}
