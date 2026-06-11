#![cfg_attr(test, allow(unused))]

#[cfg(test)]
mod tests;

use crate::cell::{Cell, RefCell};
use crate::fmt;
use crate::fs::File;
use crate::io::prelude::*;
use crate::io::{
    self, BorrowedCursor, BufReader, IoSlice, IoSliceMut, LineWriter, Lines, SpecReadByte,
};
use crate::panic::{RefUnwindSafe, UnwindSafe};
use crate::sync::atomic::{Atomic, AtomicBool, Ordering};
use crate::sync::{Arc, Mutex, MutexGuard, OnceLock, ReentrantLock, ReentrantLockGuard};
use crate::sys::stdio;
use crate::thread::AccessError;

type LocalStream = Arc<Mutex<Vec<u8>>>;

thread_local! {
    /// 供 test crate 使用，用于捕获 print 宏和 panic 的输出。
    static OUTPUT_CAPTURE: Cell<Option<LocalStream>> = const {
        Cell::new(None)
    }
}

/// 用于指示 OUTPUT_CAPTURE 是否被使用的标志。
///
/// 如果它为 None 且从未在任何线程上被设置过，则该标志为 false，于是在所有线程上都可以安全地
/// 忽略 OUTPUT_CAPTURE，从而省去注册一个未被使用的线程局部变量所需的时间和内存。
///
/// 关于内存序（memory ordering）的说明：本标志携带的是“某个线程局部变量是否可能正在被使用”
/// 这一信息。尽管它是一个全局标志，但线程之间的内存序无关紧要：我们只希望该标志在
/// *同一个线程内部* 的 set_output_capture 与 print_to 之间保持一致的顺序。而在同一个线程
/// 内部，一切本来就有完全一致的顺序。所以用 Ordering::Relaxed 就够了。
static OUTPUT_CAPTURE_USED: Atomic<bool> = AtomicBool::new(false);

/// 指向本进程标准输入流的一个“原始（raw）”实例的句柄。
///
/// 这个句柄不做任何同步或缓冲。通过 `std::io::stdio::stdin_raw` 函数构造。
struct StdinRaw(stdio::Stdin);

/// 指向本进程标准输出流的一个“原始（raw）”实例的句柄。
///
/// 这个句柄不做任何同步或缓冲。通过 `std::io::stdio::stdout_raw` 函数构造。
struct StdoutRaw(stdio::Stdout);

/// 指向本进程标准输出流的一个“原始（raw）”实例的句柄。
///
/// 这个句柄不做任何同步或缓冲。通过 `std::io::stdio::stderr_raw` 函数构造。
struct StderrRaw(stdio::Stderr);

/// 构造一个指向本进程标准输入的新“原始”句柄。
///
/// 返回的句柄既不与任何其他已创建的句柄交互，也不与 `std::io::stdin` 返回的句柄交互。被
/// `std::io::stdin` 句柄缓冲的数据**不**会提供给本函数返回的原始句柄。
///
/// 返回的句柄没有任何外部同步或缓冲。
#[unstable(feature = "libstd_sys_internals", issue = "none")]
const fn stdin_raw() -> StdinRaw {
    StdinRaw(stdio::Stdin::new())
}

/// 构造一个指向本进程标准输出流的新“原始”句柄。
///
/// 返回的句柄既不与任何其他已创建的句柄交互，也不与 `std::io::stdout` 返回的句柄交互。注意
/// 数据会被 `std::io::stdout` 句柄缓冲，所以经由这个原始句柄发生的写入可能会出现在先前那些
/// 写入之前。
///
/// 返回的句柄在其之上没有叠加任何外部同步或缓冲。
#[unstable(feature = "libstd_sys_internals", issue = "none")]
const fn stdout_raw() -> StdoutRaw {
    StdoutRaw(stdio::Stdout::new())
}

/// 构造一个指向本进程标准错误流的新“原始”句柄。
///
/// 返回的句柄既不与任何其他已创建的句柄交互，也不与 `std::io::stderr` 返回的句柄交互。
///
/// 返回的句柄在其之上没有叠加任何外部同步或缓冲。
#[unstable(feature = "libstd_sys_internals", issue = "none")]
const fn stderr_raw() -> StderrRaw {
    StderrRaw(stdio::Stderr::new())
}

impl Read for StdinRaw {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        handle_ebadf(self.0.read(buf), || Ok(0))
    }

    fn read_buf(&mut self, buf: BorrowedCursor<'_>) -> io::Result<()> {
        handle_ebadf(self.0.read_buf(buf), || Ok(()))
    }

    fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        handle_ebadf(self.0.read_vectored(bufs), || Ok(0))
    }

    #[inline]
    fn is_read_vectored(&self) -> bool {
        self.0.is_read_vectored()
    }

    fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
        if buf.is_empty() {
            return Ok(());
        }
        handle_ebadf(self.0.read_exact(buf), || Err(io::Error::READ_EXACT_EOF))
    }

    fn read_buf_exact(&mut self, buf: BorrowedCursor<'_>) -> io::Result<()> {
        if buf.capacity() == 0 {
            return Ok(());
        }
        handle_ebadf(self.0.read_buf_exact(buf), || Err(io::Error::READ_EXACT_EOF))
    }

    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> io::Result<usize> {
        handle_ebadf(self.0.read_to_end(buf), || Ok(0))
    }

    fn read_to_string(&mut self, buf: &mut String) -> io::Result<usize> {
        handle_ebadf(self.0.read_to_string(buf), || Ok(0))
    }
}

impl Write for StdoutRaw {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        handle_ebadf(self.0.write(buf), || Ok(buf.len()))
    }

    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        let total = || Ok(bufs.iter().map(|b| b.len()).sum());
        handle_ebadf(self.0.write_vectored(bufs), total)
    }

    #[inline]
    fn is_write_vectored(&self) -> bool {
        self.0.is_write_vectored()
    }

    fn flush(&mut self) -> io::Result<()> {
        handle_ebadf(self.0.flush(), || Ok(()))
    }

    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        handle_ebadf(self.0.write_all(buf), || Ok(()))
    }

    fn write_all_vectored(&mut self, bufs: &mut [IoSlice<'_>]) -> io::Result<()> {
        handle_ebadf(self.0.write_all_vectored(bufs), || Ok(()))
    }

    fn write_fmt(&mut self, fmt: fmt::Arguments<'_>) -> io::Result<()> {
        handle_ebadf(self.0.write_fmt(fmt), || Ok(()))
    }
}

impl Write for StderrRaw {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        handle_ebadf(self.0.write(buf), || Ok(buf.len()))
    }

    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        let total = || Ok(bufs.iter().map(|b| b.len()).sum());
        handle_ebadf(self.0.write_vectored(bufs), total)
    }

    #[inline]
    fn is_write_vectored(&self) -> bool {
        self.0.is_write_vectored()
    }

    fn flush(&mut self) -> io::Result<()> {
        handle_ebadf(self.0.flush(), || Ok(()))
    }

    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        handle_ebadf(self.0.write_all(buf), || Ok(()))
    }

    fn write_all_vectored(&mut self, bufs: &mut [IoSlice<'_>]) -> io::Result<()> {
        handle_ebadf(self.0.write_all_vectored(bufs), || Ok(()))
    }

    fn write_fmt(&mut self, fmt: fmt::Arguments<'_>) -> io::Result<()> {
        handle_ebadf(self.0.write_fmt(fmt), || Ok(()))
    }
}

fn handle_ebadf<T>(r: io::Result<T>, default: impl FnOnce() -> io::Result<T>) -> io::Result<T> {
    match r {
        Err(ref e) if stdio::is_ebadf(e) => default(),
        r => r,
    }
}

/// 指向某个进程标准输入流的句柄。
///
/// 每个句柄都是对“本进程全局输入数据缓冲”的一个共享引用。可以对句柄调用 `lock`，以获得对
/// [`BufRead`] 方法（如 `.lines()`）的完整访问。除此之外，对这个句柄的读取相对于其他读取
/// 来说也是加锁的（即全局锁保护了并发读取）。
///
/// 这个句柄实现了 `Read` trait，但要当心：对 `Stdin` 的并发读取必须小心进行（多个读取者
/// 共享同一全局缓冲，且各自加锁，处理不当可能造成死锁或数据交错）。
///
/// 由 [`io::stdin`] 方法创建。
///
/// [`io::stdin`]: stdin
///
/// ### 注意：Windows 可移植性方面的考量
///
/// 在控制台中运行时，本流的 Windows 实现不支持非 UTF-8 的字节序列。尝试读取非合法 UTF-8 的
/// 字节将返回一个错误。
///
/// 在一个分离了控制台（detached console）的进程中——例如使用了
/// `#![windows_subsystem = "windows"]` 的进程，或从这样的进程派生出的子进程——其中所含的
/// 句柄将为 null。在这种情况下，标准库的 `Read` 和 `Write` 将什么都不做并静默地成功返回。
/// 而所有其他 I/O 操作，无论是经由标准库还是经由原始的 Windows API 调用，都将失败。
///
/// # 示例
///
/// ```no_run
/// use std::io;
///
/// fn main() -> io::Result<()> {
///     let mut buffer = String::new();
///     let stdin = io::stdin(); // 我们在这里得到 `Stdin`。
///     stdin.read_line(&mut buffer)?;
///     Ok(())
/// }
/// ```
#[stable(feature = "rust1", since = "1.0.0")]
#[cfg_attr(not(test), rustc_diagnostic_item = "Stdin")]
pub struct Stdin {
    inner: &'static Mutex<BufReader<StdinRaw>>,
}

/// 对 [`Stdin`] 句柄的一个已加锁引用。
///
/// 这个句柄同时实现了 [`Read`] 和 [`BufRead`] traits，通过 [`Stdin::lock`] 方法构造。
///
/// ### 注意：Windows 可移植性方面的考量
///
/// 在控制台中运行时，本流的 Windows 实现不支持非 UTF-8 的字节序列。尝试读取非合法 UTF-8 的
/// 字节将返回一个错误。
///
/// 在一个分离了控制台（detached console）的进程中——例如使用了
/// `#![windows_subsystem = "windows"]` 的进程，或从这样的进程派生出的子进程——其中所含的
/// 句柄将为 null。在这种情况下，标准库的 `Read` 和 `Write` 将什么都不做并静默地成功返回。
/// 而所有其他 I/O 操作，无论是经由标准库还是经由原始的 Windows API 调用，都将失败。
///
/// # 示例
///
/// ```no_run
/// use std::io::{self, BufRead};
///
/// fn main() -> io::Result<()> {
///     let mut buffer = String::new();
///     let stdin = io::stdin(); // 我们在这里得到 `Stdin`。
///     {
///         let mut handle = stdin.lock(); // 我们在这里得到 `StdinLock`。
///         handle.read_line(&mut buffer)?;
///     } // `StdinLock` 在这里被 drop。
///     Ok(())
/// }
/// ```
#[must_use = "if unused stdin will immediately unlock"]
#[stable(feature = "rust1", since = "1.0.0")]
pub struct StdinLock<'a> {
    inner: MutexGuard<'a, BufReader<StdinRaw>>,
}

/// 构造一个指向当前进程标准输入的新句柄。
///
/// 返回的每个句柄都是对一个共享全局缓冲的引用，对该缓冲的访问通过一个互斥锁（mutex）来同步。
/// 如果你需要对加锁有更明确的控制，请参见 [`Stdin::lock`] 方法。
///
/// ### 注意：Windows 可移植性方面的考量
///
/// 在控制台中运行时，本流的 Windows 实现不支持非 UTF-8 的字节序列。尝试读取非合法 UTF-8 的
/// 字节将返回一个错误。
///
/// 在一个分离了控制台（detached console）的进程中——例如使用了
/// `#![windows_subsystem = "windows"]` 的进程，或从这样的进程派生出的子进程——其中所含的
/// 句柄将为 null。在这种情况下，标准库的 `Read` 和 `Write` 将什么都不做并静默地成功返回。
/// 而所有其他 I/O 操作，无论是经由标准库还是经由原始的 Windows API 调用，都将失败。
///
/// # 示例
///
/// 使用隐式同步：
///
/// ```no_run
/// use std::io;
///
/// fn main() -> io::Result<()> {
///     let mut buffer = String::new();
///     io::stdin().read_line(&mut buffer)?;
///     Ok(())
/// }
/// ```
///
/// 使用显式同步：
///
/// ```no_run
/// use std::io::{self, BufRead};
///
/// fn main() -> io::Result<()> {
///     let mut buffer = String::new();
///     let stdin = io::stdin();
///     let mut handle = stdin.lock();
///
///     handle.read_line(&mut buffer)?;
///     Ok(())
/// }
/// ```
#[must_use]
#[stable(feature = "rust1", since = "1.0.0")]
pub fn stdin() -> Stdin {
    static INSTANCE: OnceLock<Mutex<BufReader<StdinRaw>>> = OnceLock::new();
    Stdin {
        inner: INSTANCE.get_or_init(|| {
            Mutex::new(BufReader::with_capacity(stdio::STDIN_BUF_SIZE, stdin_raw()))
        }),
    }
}

impl Stdin {
    /// 锁定这个指向标准输入流的句柄，返回一个可读的守卫（guard）。
    ///
    /// 当返回的锁离开作用域时，锁会被释放。返回的守卫同时也实现了 [`Read`] 和 [`BufRead`]
    /// traits，以便访问底层数据。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::io::{self, BufRead};
    ///
    /// fn main() -> io::Result<()> {
    ///     let mut buffer = String::new();
    ///     let stdin = io::stdin();
    ///     let mut handle = stdin.lock();
    ///
    ///     handle.read_line(&mut buffer)?;
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn lock(&self) -> StdinLock<'static> {
        // 以 'static 生命周期锁定这个句柄。这依赖于一个实现细节：底层的 `Mutex` 是 static 的。
        StdinLock { inner: self.inner.lock().unwrap_or_else(|e| e.into_inner()) }
    }

    /// 锁定这个句柄并读取一行输入，将其追加到指定的缓冲中。
    ///
    /// 关于本方法的详细语义，请参见 [`BufRead::read_line`] 的文档。特别地：
    /// * 缓冲中先前的内容会被保留。要避免追加到缓冲，你需要先 [`clear`] 它。
    /// * 末尾的换行符（如果有的话）会被包含在缓冲中。
    ///
    /// [`clear`]: String::clear
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::io;
    ///
    /// let mut input = String::new();
    /// match io::stdin().read_line(&mut input) {
    ///     Ok(n) => {
    ///         println!("{n} bytes read");
    ///         println!("{input}");
    ///     }
    ///     Err(error) => println!("error: {error}"),
    /// }
    /// ```
    ///
    /// 你可以用以下两种方式之一来运行这个示例：
    ///
    /// - 把一些文本通过管道喂给它，例如 `printf foo | path/to/executable`
    /// - 直接运行可执行文件、以交互方式给它输入文本，这种情况下它会一直等待，直到按下回车键
    ///   才继续
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_confusables("get_line")]
    pub fn read_line(&self, buf: &mut String) -> io::Result<usize> {
        self.lock().read_line(buf)
    }

    /// 消耗这个句柄，返回一个遍历输入各行的迭代器。
    ///
    /// 关于本方法的详细语义，请参见 [`BufRead::lines`] 的文档。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::io;
    ///
    /// let lines = io::stdin().lines();
    /// for line in lines {
    ///     println!("got a line: {}", line.unwrap());
    /// }
    /// ```
    #[must_use = "`self` will be dropped if the result is not used"]
    #[stable(feature = "stdin_forwarders", since = "1.62.0")]
    pub fn lines(self) -> Lines<StdinLock<'static>> {
        self.lock().lines()
    }
}

#[stable(feature = "std_debug", since = "1.16.0")]
impl fmt::Debug for Stdin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Stdin").finish_non_exhaustive()
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl Read for Stdin {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.lock().read(buf)
    }
    fn read_buf(&mut self, buf: BorrowedCursor<'_>) -> io::Result<()> {
        self.lock().read_buf(buf)
    }
    fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        self.lock().read_vectored(bufs)
    }
    #[inline]
    fn is_read_vectored(&self) -> bool {
        self.lock().is_read_vectored()
    }
    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> io::Result<usize> {
        self.lock().read_to_end(buf)
    }
    fn read_to_string(&mut self, buf: &mut String) -> io::Result<usize> {
        self.lock().read_to_string(buf)
    }
    fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
        self.lock().read_exact(buf)
    }
    fn read_buf_exact(&mut self, cursor: BorrowedCursor<'_>) -> io::Result<()> {
        self.lock().read_buf_exact(cursor)
    }
}

#[stable(feature = "read_shared_stdin", since = "1.78.0")]
impl Read for &Stdin {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.lock().read(buf)
    }
    fn read_buf(&mut self, buf: BorrowedCursor<'_>) -> io::Result<()> {
        self.lock().read_buf(buf)
    }
    fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        self.lock().read_vectored(bufs)
    }
    #[inline]
    fn is_read_vectored(&self) -> bool {
        self.lock().is_read_vectored()
    }
    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> io::Result<usize> {
        self.lock().read_to_end(buf)
    }
    fn read_to_string(&mut self, buf: &mut String) -> io::Result<usize> {
        self.lock().read_to_string(buf)
    }
    fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
        self.lock().read_exact(buf)
    }
    fn read_buf_exact(&mut self, cursor: BorrowedCursor<'_>) -> io::Result<()> {
        self.lock().read_buf_exact(cursor)
    }
}

// 仅被平台相关的 io::copy 特化使用，因此在某些平台上是未使用的
#[cfg(any(target_os = "linux", target_os = "android"))]
impl StdinLock<'_> {
    pub(crate) fn as_mut_buf(&mut self) -> &mut BufReader<impl Read> {
        &mut self.inner
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl Read for StdinLock<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.inner.read(buf)
    }

    fn read_buf(&mut self, buf: BorrowedCursor<'_>) -> io::Result<()> {
        self.inner.read_buf(buf)
    }

    fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        self.inner.read_vectored(bufs)
    }

    #[inline]
    fn is_read_vectored(&self) -> bool {
        self.inner.is_read_vectored()
    }

    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> io::Result<usize> {
        self.inner.read_to_end(buf)
    }

    fn read_to_string(&mut self, buf: &mut String) -> io::Result<usize> {
        self.inner.read_to_string(buf)
    }

    fn read_exact(&mut self, buf: &mut [u8]) -> io::Result<()> {
        self.inner.read_exact(buf)
    }

    fn read_buf_exact(&mut self, cursor: BorrowedCursor<'_>) -> io::Result<()> {
        self.inner.read_buf_exact(cursor)
    }
}

impl SpecReadByte for StdinLock<'_> {
    #[inline]
    fn spec_read_byte(&mut self) -> Option<io::Result<u8>> {
        BufReader::spec_read_byte(&mut *self.inner)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl BufRead for StdinLock<'_> {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        self.inner.fill_buf()
    }

    fn consume(&mut self, n: usize) {
        self.inner.consume(n)
    }

    fn read_until(&mut self, byte: u8, buf: &mut Vec<u8>) -> io::Result<usize> {
        self.inner.read_until(byte, buf)
    }

    fn read_line(&mut self, buf: &mut String) -> io::Result<usize> {
        self.inner.read_line(buf)
    }
}

#[stable(feature = "std_debug", since = "1.16.0")]
impl fmt::Debug for StdinLock<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StdinLock").finish_non_exhaustive()
    }
}

/// 指向当前进程全局标准输出流的句柄。
///
/// 每个句柄共享一块“待写入标准输出流的数据”的全局缓冲。访问同样通过一个锁来同步，并且可经由
/// [`lock`] 方法获得对加锁的显式控制。
///
/// 默认情况下，当连接到终端（terminal）时，该句柄是行缓冲（line-buffered）的，意味着每当
/// 遇到换行符（`\n`）时它就会自动刷新。若想立即输出，你可以手动调用 [`flush`] 方法。当句柄
/// 离开作用域时，缓冲会被自动刷新。
///
/// 由 [`io::stdout`] 方法创建。
///
/// ### 注意：Windows 可移植性方面的考量
///
/// 在控制台中运行时，本流的 Windows 实现不支持非 UTF-8 的字节序列。尝试写入非合法 UTF-8 的
/// 字节将返回一个错误。
///
/// 在一个分离了控制台（detached console）的进程中——例如使用了
/// `#![windows_subsystem = "windows"]` 的进程，或从这样的进程派生出的子进程——其中所含的
/// 句柄将为 null。在这种情况下，标准库的 `Read` 和 `Write` 将什么都不做并静默地成功返回。
/// 而所有其他 I/O 操作，无论是经由标准库还是经由原始的 Windows API 调用，都将失败。
///
/// [`lock`]: Stdout::lock
/// [`flush`]: Write::flush
/// [`io::stdout`]: stdout
#[stable(feature = "rust1", since = "1.0.0")]
pub struct Stdout {
    // FIXME: 这里应当根据 stdout 的状态（是否为 tty）来选用 LineWriter 或 BufWriter。
    //        注意，如果它不是行缓冲的，那它还应当做到 flush-on-panic 或某种形式的
    //        flush-on-abort（在 panic/abort 时刷新）。
    inner: &'static ReentrantLock<RefCell<LineWriter<StdoutRaw>>>,
}

/// 对 [`Stdout`] 句柄的一个已加锁引用。
///
/// 这个句柄实现了 [`Write`] trait，通过 [`Stdout::lock`] 方法构造。更多内容见其文档。
///
/// 默认情况下，当连接到终端（terminal）时，该句柄是行缓冲（line-buffered）的，意味着每当
/// 遇到换行符（`\n`）时它就会自动刷新。若想立即输出，你可以手动调用 [`flush`] 方法。当句柄
/// 离开作用域时，缓冲会被自动刷新。
///
/// ### 注意：Windows 可移植性方面的考量
///
/// 在控制台中运行时，本流的 Windows 实现不支持非 UTF-8 的字节序列。尝试写入非合法 UTF-8 的
/// 字节将返回一个错误。
///
/// 在一个分离了控制台（detached console）的进程中——例如使用了
/// `#![windows_subsystem = "windows"]` 的进程，或从这样的进程派生出的子进程——其中所含的
/// 句柄将为 null。在这种情况下，标准库的 `Read` 和 `Write` 将什么都不做并静默地成功返回。
/// 而所有其他 I/O 操作，无论是经由标准库还是经由原始的 Windows API 调用，都将失败。
///
/// [`flush`]: Write::flush
#[must_use = "if unused stdout will immediately unlock"]
#[stable(feature = "rust1", since = "1.0.0")]
pub struct StdoutLock<'a> {
    inner: ReentrantLockGuard<'a, RefCell<LineWriter<StdoutRaw>>>,
}

static STDOUT: OnceLock<ReentrantLock<RefCell<LineWriter<StdoutRaw>>>> = OnceLock::new();

/// 构造一个指向当前进程标准输出的新句柄。
///
/// 返回的每个句柄都是对一个共享全局缓冲的引用，对该缓冲的访问通过一个互斥锁（mutex）来同步。
/// 如果你需要对加锁有更明确的控制，请参见 [`Stdout::lock`] 方法。
///
/// 默认情况下，当连接到终端（terminal）时，该句柄是行缓冲（line-buffered）的，意味着每当
/// 遇到换行符（`\n`）时它就会自动刷新。若想立即输出，你可以手动调用 [`flush`] 方法。当句柄
/// 离开作用域时，缓冲会被自动刷新。
///
/// ### 注意：Windows 可移植性方面的考量
///
/// 在控制台中运行时，本流的 Windows 实现不支持非 UTF-8 的字节序列。尝试写入非合法 UTF-8 的
/// 字节将返回一个错误。
///
/// 在一个分离了控制台（detached console）的进程中——例如使用了
/// `#![windows_subsystem = "windows"]` 的进程，或从这样的进程派生出的子进程——其中所含的
/// 句柄将为 null。在这种情况下，标准库的 `Read` 和 `Write` 将什么都不做并静默地成功返回。
/// 而所有其他 I/O 操作，无论是经由标准库还是经由原始的 Windows API 调用，都将失败。
///
/// # 示例
///
/// 使用隐式同步：
///
/// ```no_run
/// use std::io::{self, Write};
///
/// fn main() -> io::Result<()> {
///     io::stdout().write_all(b"hello world")?;
///
///     Ok(())
/// }
/// ```
///
/// 使用显式同步：
///
/// ```no_run
/// use std::io::{self, Write};
///
/// fn main() -> io::Result<()> {
///     let stdout = io::stdout();
///     let mut handle = stdout.lock();
///
///     handle.write_all(b"hello world")?;
///
///     Ok(())
/// }
/// ```
///
/// 确保输出被立即刷新：
///
/// ```no_run
/// use std::io::{self, Write};
///
/// fn main() -> io::Result<()> {
///     let mut stdout = io::stdout();
///     stdout.write_all(b"hello, ")?;
///     stdout.flush()?;                // 手动刷新
///     stdout.write_all(b"world!\n")?; // 自动刷新（因为以换行符结尾）
///     Ok(())
/// }
/// ```
///
/// [`flush`]: Write::flush
#[must_use]
#[stable(feature = "rust1", since = "1.0.0")]
#[cfg_attr(not(test), rustc_diagnostic_item = "io_stdout")]
pub fn stdout() -> Stdout {
    Stdout {
        inner: STDOUT
            .get_or_init(|| ReentrantLock::new(RefCell::new(LineWriter::new(stdout_raw())))),
    }
}

// 在关闭（shutdown）期间，把数据刷新出去并禁用缓冲——做法是用一个缓冲容量为零的 line writer
// 替换掉原来的 line writer。
pub fn cleanup() {
    let mut initialized = false;
    let stdout = STDOUT.get_or_init(|| {
        initialized = true;
        ReentrantLock::new(RefCell::new(LineWriter::with_capacity(0, stdout_raw())))
    });

    if !initialized {
        // 缓冲此前已被初始化过，所以在这里把它覆盖掉。
        // 我们用 try_lock() 而非 lock()，因为可能有人泄漏（leak）了一个 StdoutLock，
        // 否则那将在这里造成死锁。
        if let Some(lock) = stdout.try_lock() {
            *lock.borrow_mut() = LineWriter::with_capacity(0, stdout_raw());
        }
    }
}

impl Stdout {
    /// 锁定这个指向标准输出流的句柄，返回一个可写的守卫（guard）。
    ///
    /// 当返回的锁离开作用域时，锁会被释放。返回的守卫同样实现了 `Write` trait 以便写入数据。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::io::{self, Write};
    ///
    /// fn main() -> io::Result<()> {
    ///     let mut stdout = io::stdout().lock();
    ///
    ///     stdout.write_all(b"hello world")?;
    ///
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn lock(&self) -> StdoutLock<'static> {
        // 以 'static 生命周期锁定这个句柄。这依赖于一个实现细节：底层的 `ReentrantMutex`
        // 是 static 的。
        StdoutLock { inner: self.inner.lock() }
    }
}

#[stable(feature = "catch_unwind", since = "1.9.0")]
impl UnwindSafe for Stdout {}

#[stable(feature = "catch_unwind", since = "1.9.0")]
impl RefUnwindSafe for Stdout {}

#[stable(feature = "std_debug", since = "1.16.0")]
impl fmt::Debug for Stdout {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Stdout").finish_non_exhaustive()
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl Write for Stdout {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        (&*self).write(buf)
    }
    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        (&*self).write_vectored(bufs)
    }
    #[inline]
    fn is_write_vectored(&self) -> bool {
        io::Write::is_write_vectored(&&*self)
    }
    fn flush(&mut self) -> io::Result<()> {
        (&*self).flush()
    }
    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        (&*self).write_all(buf)
    }
    fn write_all_vectored(&mut self, bufs: &mut [IoSlice<'_>]) -> io::Result<()> {
        (&*self).write_all_vectored(bufs)
    }
    fn write_fmt(&mut self, args: fmt::Arguments<'_>) -> io::Result<()> {
        (&*self).write_fmt(args)
    }
}

#[stable(feature = "write_mt", since = "1.48.0")]
impl Write for &Stdout {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.lock().write(buf)
    }
    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        self.lock().write_vectored(bufs)
    }
    #[inline]
    fn is_write_vectored(&self) -> bool {
        self.lock().is_write_vectored()
    }
    fn flush(&mut self) -> io::Result<()> {
        self.lock().flush()
    }
    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        self.lock().write_all(buf)
    }
    fn write_all_vectored(&mut self, bufs: &mut [IoSlice<'_>]) -> io::Result<()> {
        self.lock().write_all_vectored(bufs)
    }
    fn write_fmt(&mut self, args: fmt::Arguments<'_>) -> io::Result<()> {
        self.lock().write_fmt(args)
    }
}

#[stable(feature = "catch_unwind", since = "1.9.0")]
impl UnwindSafe for StdoutLock<'_> {}

#[stable(feature = "catch_unwind", since = "1.9.0")]
impl RefUnwindSafe for StdoutLock<'_> {}

#[stable(feature = "rust1", since = "1.0.0")]
impl Write for StdoutLock<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.borrow_mut().write(buf)
    }
    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        self.inner.borrow_mut().write_vectored(bufs)
    }
    #[inline]
    fn is_write_vectored(&self) -> bool {
        self.inner.borrow_mut().is_write_vectored()
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.borrow_mut().flush()
    }
    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        self.inner.borrow_mut().write_all(buf)
    }
    fn write_all_vectored(&mut self, bufs: &mut [IoSlice<'_>]) -> io::Result<()> {
        self.inner.borrow_mut().write_all_vectored(bufs)
    }
}

#[stable(feature = "std_debug", since = "1.16.0")]
impl fmt::Debug for StdoutLock<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StdoutLock").finish_non_exhaustive()
    }
}

/// 指向某个进程标准错误流的句柄。
///
/// 更多信息，请参见 [`io::stderr`] 方法。
///
/// [`io::stderr`]: stderr
///
/// ### 注意：Windows 可移植性方面的考量
///
/// 在控制台中运行时，本流的 Windows 实现不支持非 UTF-8 的字节序列。尝试写入非合法 UTF-8 的
/// 字节将返回一个错误。
///
/// 在一个分离了控制台（detached console）的进程中——例如使用了
/// `#![windows_subsystem = "windows"]` 的进程，或从这样的进程派生出的子进程——其中所含的
/// 句柄将为 null。在这种情况下，标准库的 `Read` 和 `Write` 将什么都不做并静默地成功返回。
/// 而所有其他 I/O 操作，无论是经由标准库还是经由原始的 Windows API 调用，都将失败。
#[stable(feature = "rust1", since = "1.0.0")]
pub struct Stderr {
    inner: &'static ReentrantLock<RefCell<StderrRaw>>,
}

/// 对 [`Stderr`] 句柄的一个已加锁引用。
///
/// 这个句柄实现了 [`Write`] trait，通过 [`Stderr::lock`] 方法构造。更多内容见其文档。
///
/// ### 注意：Windows 可移植性方面的考量
///
/// 在控制台中运行时，本流的 Windows 实现不支持非 UTF-8 的字节序列。尝试写入非合法 UTF-8 的
/// 字节将返回一个错误。
///
/// 在一个分离了控制台（detached console）的进程中——例如使用了
/// `#![windows_subsystem = "windows"]` 的进程，或从这样的进程派生出的子进程——其中所含的
/// 句柄将为 null。在这种情况下，标准库的 `Read` 和 `Write` 将什么都不做并静默地成功返回。
/// 而所有其他 I/O 操作，无论是经由标准库还是经由原始的 Windows API 调用，都将失败。
#[must_use = "if unused stderr will immediately unlock"]
#[stable(feature = "rust1", since = "1.0.0")]
pub struct StderrLock<'a> {
    inner: ReentrantLockGuard<'a, RefCell<StderrRaw>>,
}

/// 构造一个指向当前进程标准错误的新句柄。
///
/// 这个句柄是不带缓冲的（即 stderr 无缓冲）。
///
/// ### 注意：Windows 可移植性方面的考量
///
/// 在控制台中运行时，本流的 Windows 实现不支持非 UTF-8 的字节序列。尝试写入非合法 UTF-8 的
/// 字节将返回一个错误。
///
/// 在一个分离了控制台（detached console）的进程中——例如使用了
/// `#![windows_subsystem = "windows"]` 的进程，或从这样的进程派生出的子进程——其中所含的
/// 句柄将为 null。在这种情况下，标准库的 `Read` 和 `Write` 将什么都不做并静默地成功返回。
/// 而所有其他 I/O 操作，无论是经由标准库还是经由原始的 Windows API 调用，都将失败。
///
/// # 示例
///
/// 使用隐式同步：
///
/// ```no_run
/// use std::io::{self, Write};
///
/// fn main() -> io::Result<()> {
///     io::stderr().write_all(b"hello world")?;
///
///     Ok(())
/// }
/// ```
///
/// 使用显式同步：
///
/// ```no_run
/// use std::io::{self, Write};
///
/// fn main() -> io::Result<()> {
///     let stderr = io::stderr();
///     let mut handle = stderr.lock();
///
///     handle.write_all(b"hello world")?;
///
///     Ok(())
/// }
/// ```
#[must_use]
#[stable(feature = "rust1", since = "1.0.0")]
#[cfg_attr(not(test), rustc_diagnostic_item = "io_stderr")]
pub fn stderr() -> Stderr {
    // 注意，与 `stdout()` 不同，我们在这里不用 `at_exit` 来注册一个析构函数。stderr 是无缓冲
    // 的，所以没有必要为刷新缓冲而运行一个析构函数
    static INSTANCE: ReentrantLock<RefCell<StderrRaw>> =
        ReentrantLock::new(RefCell::new(stderr_raw()));

    Stderr { inner: &INSTANCE }
}

impl Stderr {
    /// 锁定这个指向标准错误流的句柄，返回一个可写的守卫（guard）。
    ///
    /// 当返回的锁离开作用域时，锁会被释放。返回的守卫同样实现了 [`Write`] trait 以便写入数据。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::io::{self, Write};
    ///
    /// fn foo() -> io::Result<()> {
    ///     let stderr = io::stderr();
    ///     let mut handle = stderr.lock();
    ///
    ///     handle.write_all(b"hello world")?;
    ///
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn lock(&self) -> StderrLock<'static> {
        // 以 'static 生命周期锁定这个句柄。这依赖于一个实现细节：底层的 `ReentrantMutex`
        // 是 static 的。
        StderrLock { inner: self.inner.lock() }
    }
}

#[stable(feature = "catch_unwind", since = "1.9.0")]
impl UnwindSafe for Stderr {}

#[stable(feature = "catch_unwind", since = "1.9.0")]
impl RefUnwindSafe for Stderr {}

#[stable(feature = "std_debug", since = "1.16.0")]
impl fmt::Debug for Stderr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Stderr").finish_non_exhaustive()
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl Write for Stderr {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        (&*self).write(buf)
    }
    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        (&*self).write_vectored(bufs)
    }
    #[inline]
    fn is_write_vectored(&self) -> bool {
        io::Write::is_write_vectored(&&*self)
    }
    fn flush(&mut self) -> io::Result<()> {
        (&*self).flush()
    }
    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        (&*self).write_all(buf)
    }
    fn write_all_vectored(&mut self, bufs: &mut [IoSlice<'_>]) -> io::Result<()> {
        (&*self).write_all_vectored(bufs)
    }
    fn write_fmt(&mut self, args: fmt::Arguments<'_>) -> io::Result<()> {
        (&*self).write_fmt(args)
    }
}

#[stable(feature = "write_mt", since = "1.48.0")]
impl Write for &Stderr {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.lock().write(buf)
    }
    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        self.lock().write_vectored(bufs)
    }
    #[inline]
    fn is_write_vectored(&self) -> bool {
        self.lock().is_write_vectored()
    }
    fn flush(&mut self) -> io::Result<()> {
        self.lock().flush()
    }
    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        self.lock().write_all(buf)
    }
    fn write_all_vectored(&mut self, bufs: &mut [IoSlice<'_>]) -> io::Result<()> {
        self.lock().write_all_vectored(bufs)
    }
    fn write_fmt(&mut self, args: fmt::Arguments<'_>) -> io::Result<()> {
        self.lock().write_fmt(args)
    }
}

#[stable(feature = "catch_unwind", since = "1.9.0")]
impl UnwindSafe for StderrLock<'_> {}

#[stable(feature = "catch_unwind", since = "1.9.0")]
impl RefUnwindSafe for StderrLock<'_> {}

#[stable(feature = "rust1", since = "1.0.0")]
impl Write for StderrLock<'_> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.borrow_mut().write(buf)
    }
    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        self.inner.borrow_mut().write_vectored(bufs)
    }
    #[inline]
    fn is_write_vectored(&self) -> bool {
        self.inner.borrow_mut().is_write_vectored()
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.borrow_mut().flush()
    }
    fn write_all(&mut self, buf: &[u8]) -> io::Result<()> {
        self.inner.borrow_mut().write_all(buf)
    }
    fn write_all_vectored(&mut self, bufs: &mut [IoSlice<'_>]) -> io::Result<()> {
        self.inner.borrow_mut().write_all_vectored(bufs)
    }
}

#[stable(feature = "std_debug", since = "1.16.0")]
impl fmt::Debug for StderrLock<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("StderrLock").finish_non_exhaustive()
    }
}

/// 设置线程局部的输出捕获缓冲（output capture buffer），并返回旧的那个。
#[unstable(
    feature = "internal_output_capture",
    reason = "this function is meant for use in the test crate \
        and may disappear in the future",
    issue = "none"
)]
#[doc(hidden)]
pub fn set_output_capture(sink: Option<LocalStream>) -> Option<LocalStream> {
    try_set_output_capture(sink).expect(
        "cannot access a Thread Local Storage value \
         during or after destruction",
    )
}

/// 尝试设置线程局部的输出捕获缓冲，并返回旧的那个。
/// 一旦线程局部的析构函数已被调用，这可能会失败。它在 panic 处理中被使用，用以替代
/// `set_output_capture`。
#[unstable(
    feature = "internal_output_capture",
    reason = "this function is meant for use in the test crate \
    and may disappear in the future",
    issue = "none"
)]
#[doc(hidden)]
pub fn try_set_output_capture(
    sink: Option<LocalStream>,
) -> Result<Option<LocalStream>, AccessError> {
    if sink.is_none() && !OUTPUT_CAPTURE_USED.load(Ordering::Relaxed) {
        // 由于 OUTPUT_CAPTURE_USED 为 false，OUTPUT_CAPTURE 必定为 None。
        return Ok(None);
    }
    OUTPUT_CAPTURE_USED.store(true, Ordering::Relaxed);
    OUTPUT_CAPTURE.try_with(move |slot| slot.replace(sink))
}

/// 如果输出捕获已启用且可用，就把 `args` 写入捕获缓冲；否则写入 `global_s`。
/// `label` 用于在 panic 消息中标识该流。
///
/// 本函数用于打印错误消息，因此它会格外小心，以避免在 `OUTPUT_CAPTURE` 不可用时引发 panic。
/// 例如，如果用于输出捕获的 TLS 键已被销毁，或者本地流正被另一个线程使用，它就会直接回退到
/// 全局流。
///
/// 不过，如果实际的 I/O 引发了错误，本函数确实会 panic。
///
/// 向非阻塞（non-blocking）的 stdout/stderr 写入可能引发错误，而那将导致本函数 panic。
fn print_to<T>(args: fmt::Arguments<'_>, global_s: fn() -> T, label: &str)
where
    T: Write,
{
    if print_to_buffer_if_capture_used(args) {
        // 已成功写入捕获缓冲。
        return;
    }

    if let Err(e) = global_s().write_fmt(args) {
        panic!("failed printing to {label}: {e}");
    }
}

fn print_to_buffer_if_capture_used(args: fmt::Arguments<'_>) -> bool {
    OUTPUT_CAPTURE_USED.load(Ordering::Relaxed)
        && OUTPUT_CAPTURE.try_with(|s| {
            // 注意，我们会把本地输出端（sink）完全取出，以防我们的打印在递归过程中发生
            // panic/打印——这样，递归的 panic/打印就会走向全局输出端，而不是我们的本地输出端。
            s.take().map(|w| {
                let _ = w.lock().unwrap_or_else(|e| e.into_inner()).write_fmt(args);
                s.set(Some(w));
            })
        }) == Ok(Some(()))
}

/// 供 `impl Termination for Result` 使用，用于在 `main` 或某个测试返回之后打印错误。本函数
/// 应当避免 panic——尽管如果 args 内部的某个 Display 实现非要 panic，我们也无能为力。
pub(crate) fn attempt_print_to_stderr(args: fmt::Arguments<'_>) {
    if print_to_buffer_if_capture_used(args) {
        return;
    }

    // 如果写入失败就忽略该错误，例如因为 stderr 已经被关闭。在这一时刻 panic 没有太大意义。
    let _ = stderr().write_fmt(args);
}

/// 用于判断某个描述符/句柄是否指向一个终端（terminal/tty）的 trait。
#[stable(feature = "is_terminal", since = "1.70.0")]
pub trait IsTerminal: crate::sealed::Sealed {
    /// 如果该描述符/句柄指向一个终端（terminal/tty），返回 `true`。
    ///
    /// 在 Rust 尚不知如何检测终端的平台上，本方法将返回 `false`。如果发生了意料之外的错误
    ///（例如传入了一个无效的文件描述符），它也会返回 `false`。
    ///
    /// # Platform-specific behavior
    ///
    /// 在 Windows 上，除了检测控制台之外，本方法目前还使用了一些基于设备名的启发式方法来检测
    /// 较旧的 msys/cygwin/mingw 伪终端（pseudo-terminal）：设备名以 `msys-` 或 `cygwin-`
    /// 开头、并以 `-pty` 结尾的，会被视为终端。注意这[将来可能会改变][changes]。
    ///
    /// # 示例
    ///
    /// 一个实现了 `IsTerminal` 的类型示例是 [`Stdin`]：
    ///
    /// ```no_run
    /// use std::io::{self, IsTerminal, Write};
    ///
    /// fn main() -> io::Result<()> {
    ///     let stdin = io::stdin();
    ///
    ///     // 如果这是一个终端，就提示用户输入。
    ///     if stdin.is_terminal() {
    ///         print!("> ");
    ///         io::stdout().flush()?;
    ///     }
    ///
    ///     let mut name = String::new();
    ///     let _ = stdin.read_line(&mut name)?;
    ///
    ///     println!("Hello {}", name.trim_end());
    ///
    ///     Ok(())
    /// }
    /// ```
    ///
    /// 这个示例可以用两种方式运行：
    ///
    /// - 如果你通过把一些文本管道喂给它来运行，例如 `echo "foo" | path/to/executable`，
    ///   它将打印：`Hello foo`。
    /// - 如果你改为直接运行 `path/to/executable`、以交互方式运行该示例，它将提示你输入。
    ///
    /// [changes]: io#platform-specific-behavior
    /// [`Stdin`]: crate::io::Stdin
    #[doc(alias = "isatty")]
    #[stable(feature = "is_terminal", since = "1.70.0")]
    fn is_terminal(&self) -> bool;
}

macro_rules! impl_is_terminal {
    ($($t:ty),*$(,)?) => {$(
        #[unstable(feature = "sealed", issue = "none")]
        impl crate::sealed::Sealed for $t {}

        #[stable(feature = "is_terminal", since = "1.70.0")]
        impl IsTerminal for $t {
            #[inline]
            fn is_terminal(&self) -> bool {
                crate::sys::io::is_terminal(self)
            }
        }
    )*}
}

impl_is_terminal!(File, Stdin, StdinLock<'_>, Stdout, StdoutLock<'_>, Stderr, StderrLock<'_>);

#[unstable(
    feature = "print_internals",
    reason = "implementation detail which may disappear or be replaced at any time",
    issue = "none"
)]
#[doc(hidden)]
#[cfg(not(test))]
pub fn _print(args: fmt::Arguments<'_>) {
    print_to(args, stdout, "stdout");
}

#[unstable(
    feature = "print_internals",
    reason = "implementation detail which may disappear or be replaced at any time",
    issue = "none"
)]
#[doc(hidden)]
#[cfg(not(test))]
pub fn _eprint(args: fmt::Arguments<'_>) {
    print_to(args, stderr, "stderr");
}

#[cfg(test)]
pub use realstd::io::{_eprint, _print};
