//! 核心 I/O 功能的 trait、辅助工具和类型定义。
//!
//! `std::io` 模块包含了在进行输入输出时你会用到的许多常见东西。本模块最核心的部分是
//! [`Read`] 和 [`Write`] trait，它们为读写输入输出提供了最为通用的接口。
//!
//! ## Read 与 Write
//!
//! 因为它们是 trait，[`Read`] 和 [`Write`] 由许多其他类型实现，你也可以为你自己的类型
//! 实现它们。因此，在本模块的文档中你会看到几种不同类型的 I/O：[`File`]、[`TcpStream`]，
//! 有时甚至还有 [`Vec<T>`]。例如，[`Read`] 添加了一个 [`read`][`Read::read`] 方法，我们可以
//! 在 [`File`] 上使用它：
//!
//! ```no_run
//! use std::io;
//! use std::io::prelude::*;
//! use std::fs::File;
//!
//! fn main() -> io::Result<()> {
//!     let mut f = File::open("foo.txt")?;
//!     let mut buffer = [0; 10];
//!
//!     // 最多读取 10 个字节
//!     let n = f.read(&mut buffer)?;
//!
//!     println!("The bytes: {:?}", &buffer[..n]);
//!     Ok(())
//! }
//! ```
//!
//! [`Read`] 和 [`Write`] 是如此重要，以至于这两个 trait 的实现者有了一个绰号：reader（读取器）
//! 和 writer（写入器）。所以有时你会看到「a reader」（一个读取器），而不是「a type that
//! implements the [`Read`] trait」（一个实现了 [`Read`] trait 的类型）。简单多了！
//!
//! ## Seek 与 BufRead
//!
//! 除此之外，还提供了两个重要的 trait：[`Seek`] 和 [`BufRead`]。这两者都构建在一个 reader
//! 之上，用来控制读取是如何进行的。[`Seek`] 让你能够控制下一个字节从哪里来：
//!
//! ```no_run
//! use std::io;
//! use std::io::prelude::*;
//! use std::io::SeekFrom;
//! use std::fs::File;
//!
//! fn main() -> io::Result<()> {
//!     let mut f = File::open("foo.txt")?;
//!     let mut buffer = [0; 10];
//!
//!     // 跳到文件的最后 10 个字节
//!     f.seek(SeekFrom::End(-10))?;
//!
//!     // 最多读取 10 个字节
//!     let n = f.read(&mut buffer)?;
//!
//!     println!("The bytes: {:?}", &buffer[..n]);
//!     Ok(())
//! }
//! ```
//!
//! [`BufRead`] 使用一个内部缓冲区来提供若干其他的读取方式，但为了展示它，我们需要先泛泛地
//! 谈谈缓冲区。继续往下读吧！
//!
//! ## BufReader 与 BufWriter
//!
//! 基于字节的接口既笨拙又可能低效，因为我们需要近乎不间断地调用操作系统。为了帮助解决这点，
//! `std::io` 提供了两个结构体 [`BufReader`] 和 [`BufWriter`]，它们包装 reader 和 writer。
//! 这层包装使用一个缓冲区，从而减少调用次数，并提供更友好的方法来精确访问你想要的内容。
//!
//! 例如，[`BufReader`] 与 [`BufRead`] trait 协同工作，为任意 reader 添加额外的方法：
//!
//! ```no_run
//! use std::io;
//! use std::io::prelude::*;
//! use std::io::BufReader;
//! use std::fs::File;
//!
//! fn main() -> io::Result<()> {
//!     let f = File::open("foo.txt")?;
//!     let mut reader = BufReader::new(f);
//!     let mut buffer = String::new();
//!
//!     // 读取一行到 buffer 中
//!     reader.read_line(&mut buffer)?;
//!
//!     println!("{buffer}");
//!     Ok(())
//! }
//! ```
//!
//! [`BufWriter`] 不增加任何新的写入方式；它只是把每一次对 [`write`][`Write::write`] 的调用
//! 缓冲起来：
//!
//! ```no_run
//! use std::io;
//! use std::io::prelude::*;
//! use std::io::BufWriter;
//! use std::fs::File;
//!
//! fn main() -> io::Result<()> {
//!     let f = File::create("foo.txt")?;
//!     {
//!         let mut writer = BufWriter::new(f);
//!
//!         // 向缓冲区写入一个字节
//!         writer.write(&[42])?;
//!
//!     } // 一旦 writer 离开作用域，缓冲区就会被刷新（flush）
//!
//!     Ok(())
//! }
//! ```
//!
//! ## 标准输入与标准输出
//!
//! 一个非常常见的输入来源是标准输入：
//!
//! ```no_run
//! use std::io;
//!
//! fn main() -> io::Result<()> {
//!     let mut input = String::new();
//!
//!     io::stdin().read_line(&mut input)?;
//!
//!     println!("You typed: {}", input.trim());
//!     Ok(())
//! }
//! ```
//!
//! 注意，你不能在那些不返回 [`Result<T, E>`][`Result`] 的函数中使用 [`?` 运算符][`?` operator]。
//! 作为替代，你可以对返回值调用 [`.unwrap()`] 或对其使用 `match` 来捕获任何可能的错误：
//!
//! ```no_run
//! use std::io;
//!
//! let mut input = String::new();
//!
//! io::stdin().read_line(&mut input).unwrap();
//! ```
//!
//! 而一个非常常见的输出去处是标准输出：
//!
//! ```no_run
//! use std::io;
//! use std::io::prelude::*;
//!
//! fn main() -> io::Result<()> {
//!     io::stdout().write(&[42])?;
//!     Ok(())
//! }
//! ```
//!
//! 当然，直接使用 [`io::stdout`] 不如 [`println!`] 之类的方式常见。
//!
//! ## 迭代器类型
//!
//! `std::io` 提供的大量结构体都是用于以各种方式对 I/O 进行迭代的。例如，[`Lines`] 用于按行
//! 分割：
//!
//! ```no_run
//! use std::io;
//! use std::io::prelude::*;
//! use std::io::BufReader;
//! use std::fs::File;
//!
//! fn main() -> io::Result<()> {
//!     let f = File::open("foo.txt")?;
//!     let reader = BufReader::new(f);
//!
//!     for line in reader.lines() {
//!         println!("{}", line?);
//!     }
//!     Ok(())
//! }
//! ```
//!
//! ## 函数
//!
//! 有许多[函数][functions-list]提供了对各种功能的访问。例如，我们可以使用其中三个函数，把所有
//! 内容从标准输入复制到标准输出：
//!
//! ```no_run
//! use std::io;
//!
//! fn main() -> io::Result<()> {
//!     io::copy(&mut io::stdin(), &mut io::stdout())?;
//!     Ok(())
//! }
//! ```
//!
//! [functions-list]: #functions-1
//!
//! ## io::Result
//!
//! 最后但同样重要的是 [`io::Result`]。这个类型被用作许多可能出错的 `std::io` 函数的返回类型，
//! 也可以从你自己的函数中返回。本模块中的许多示例都使用了 [`?` 运算符][`?` operator]：
//!
//! ```
//! use std::io;
//!
//! fn read_input() -> io::Result<()> {
//!     let mut input = String::new();
//!
//!     io::stdin().read_line(&mut input)?;
//!
//!     println!("You typed: {}", input.trim());
//!
//!     Ok(())
//! }
//! ```
//!
//! `read_input()` 的返回类型 [`io::Result<()>`][`io::Result`] 是一种非常常见的类型，适用于那些
//! 没有「真正」返回值、但确实希望在发生错误时返回错误的函数。在这个例子中，该函数的唯一目的就是
//! 读取这一行并打印它，所以我们用 `()`。
//!
//! ## 平台相关行为
//!
//! 整个标准库中的许多 I/O 函数，其文档都会标明它们被委托给了哪些不同的库函数或系统调用。这样做
//! 是为了帮助应用程序既理解底层发生了什么，又能调查任何可能含糊不清的语义。不过请注意，这是
//! 信息性的说明，而非具有约束力的契约。这些函数中许多的实现可能会随时间而变化，可能调用更少或
//! 更多的系统调用/库函数。
//!
//! ## I/O 安全性
//!
//! Rust 遵循一套 I/O 安全性准则，它可与其内存安全性准则相类比。这意味着文件描述符可以被
//! *独占拥有*。（这里，「文件描述符」一词意在涵盖那些在各种操作系统中存在的相似概念，即便它们
//! 可能使用不同的名称，比如「句柄」（handle）。）一个被独占拥有的文件描述符是指：不允许任何其他
//! 代码以任何方式访问它，但其拥有者可以随时访问它、乃至关闭它。一个拥有其文件描述符的类型，通常
//! 应当在其 `drop` 函数中将其关闭。像 [`File`] 这样的类型就拥有它们的文件描述符。类似地，文件
//! 描述符也可以被*借用*，从而授予在该文件描述符上执行操作的临时权利。这表明在借用的生命周期内
//! 该文件描述符不会被关闭，但它*不*意味着拥有关闭该文件描述符的任何权利，因为它很可能由其他人
//! 拥有。
//!
//! Rust 标准库中与平台相关的部分暴露了反映这些概念的类型，参见 [`os::unix`] 和 [`os::windows`]。
//!
//! 为了维护 I/O 安全性，至关重要的一点是：任何代码都不得对它既不拥有也未借用的文件描述符进行操作，
//! 并且任何代码都不得关闭它并不拥有的文件描述符。换句话说，一个接受普通整数、把它当作文件描述符
//! 并对其进行操作的安全函数，是*不可靠的（unsound）*。
//!
//! 不维护 I/O 安全性、在没有所有权证明的情况下对文件描述符进行操作，可能会在那些依赖于其文件
//! 描述符所有权的代码中导致行为失常、乃至未定义行为：一个已关闭的文件描述符可能被重新分配，于是
//! 该文件描述符原本的拥有者现在就操作到了错误的文件上。某些代码甚至可能依赖于对其文件描述符的
//! 完全封装——即不允许程序的任何其他部分对其执行任何操作。
//!
//! 注意，对文件描述符的独占所有权*不*意味着对该文件描述符所引用的底层内核对象（在某些操作系统上
//! 也称为「打开文件描述（open file description）」）的独占所有权。文件描述符的工作方式基本上
//! 类似于 [`Arc`]：当你收到一个被拥有的文件描述符时，你无法知道是否还有其他文件描述符引用着同一个
//! 内核对象。然而，当你创建一个新的内核对象时，你知道自己持有对它的唯一引用。只是要小心别把它
//! 借给任何人，因为他们可以获得一个克隆，于是你就再也无法知道引用计数是多少了！从这个意义上说，
//! [`OwnedFd`] 类似于 `Arc`，而 [`BorrowedFd<'a>`] 类似于 `&'a Arc`（Windows 的类型也与此类似）。
//! 特别地，给定一个 `BorrowedFd<'a>`，你不被允许关闭该文件描述符——就好比给定一个 `&'a Arc`，
//! 你不被允许递减引用计数并可能释放底层对象一样。标准库中没有针对文件描述符的、与 `Box` 等价的
//! 东西（那会是一个保证引用计数为 `1` 的类型），然而，某个 crate 完全可以定义一个具有这种语义的
//! 类型。
//!
//! [`File`]: crate::fs::File
//! [`TcpStream`]: crate::net::TcpStream
//! [`io::stdout`]: stdout
//! [`io::Result`]: self::Result
//! [`?` operator]: ../../book/appendix-02-operators.html
//! [`Result`]: crate::result::Result
//! [`.unwrap()`]: crate::result::Result::unwrap
//! [`os::unix`]: ../os/unix/io/index.html
//! [`os::windows`]: ../os/windows/io/index.html
//! [`OwnedFd`]: ../os/fd/struct.OwnedFd.html
//! [`BorrowedFd<'a>`]: ../os/fd/struct.BorrowedFd.html
//! [`Arc`]: crate::sync::Arc

#![stable(feature = "rust1", since = "1.0.0")]

#[cfg(test)]
mod tests;

#[unstable(feature = "read_buf", issue = "78485")]
pub use core::io::{BorrowedBuf, BorrowedCursor};
use core::slice::memchr;

#[stable(feature = "bufwriter_into_parts", since = "1.56.0")]
pub use self::buffered::WriterPanicked;
#[unstable(feature = "raw_os_error_ty", issue = "107792")]
pub use self::error::RawOsError;
#[doc(hidden)]
#[unstable(feature = "io_const_error_internals", issue = "none")]
pub use self::error::SimpleMessage;
#[unstable(feature = "io_const_error", issue = "133448")]
pub use self::error::const_error;
#[stable(feature = "anonymous_pipe", since = "1.87.0")]
pub use self::pipe::{PipeReader, PipeWriter, pipe};
#[stable(feature = "is_terminal", since = "1.70.0")]
pub use self::stdio::IsTerminal;
pub(crate) use self::stdio::attempt_print_to_stderr;
#[unstable(feature = "print_internals", issue = "none")]
#[doc(hidden)]
pub use self::stdio::{_eprint, _print};
#[unstable(feature = "internal_output_capture", issue = "none")]
#[doc(no_inline, hidden)]
pub use self::stdio::{set_output_capture, try_set_output_capture};
#[stable(feature = "rust1", since = "1.0.0")]
pub use self::{
    buffered::{BufReader, BufWriter, IntoInnerError, LineWriter},
    copy::copy,
    cursor::Cursor,
    error::{Error, ErrorKind, Result},
    stdio::{Stderr, StderrLock, Stdin, StdinLock, Stdout, StdoutLock, stderr, stdin, stdout},
    util::{Empty, Repeat, Sink, empty, repeat, sink},
};
use crate::mem::{MaybeUninit, take};
use crate::ops::{Deref, DerefMut};
use crate::{cmp, fmt, slice, str, sys};

mod buffered;
pub(crate) mod copy;
mod cursor;
mod error;
mod impls;
mod pipe;
pub mod prelude;
mod stdio;
mod util;

const DEFAULT_BUF_SIZE: usize = crate::sys::io::DEFAULT_BUF_SIZE;

pub(crate) use stdio::cleanup;

struct Guard<'a> {
    buf: &'a mut Vec<u8>,
    len: usize,
}

impl Drop for Guard<'_> {
    fn drop(&mut self) {
        unsafe {
            self.buf.set_len(self.len);
        }
    }
}

// 标准库中有好几个 `read_to_string` 和 `read_line` 方法会把数据追加到一个 `String`
// 缓冲区里，但我们在这样做时必须相当小心。其实现只是调用 `.as_mut_vec()`，然后委托给一个
// 面向字节的读取方法，但我们必须确保：在返回时，绝不让 `buf` 处于一种「其有效范围内含有
// 无效 UTF-8」的状态。
//
// 为此，我们使用一个 RAII 守卫（以防范 panic），它会在被析构时更新字符串的长度。这个守卫
// 起初会把字符串截断到先前的长度，只有在我们验证了新内容是有效 UTF-8 之后，才允许它设置一个
// 更长的长度。
//
// 这个函数中的 unsafe 之处有两点：
//
// 1. 我们在查看 `buf` 的原始字节，所以我们承担起了 UTF-8 检查的负担。
// 2. 我们把一个原始缓冲区传给函数 `f`，并期望该函数只向缓冲区*追加*字节。如果已有的字节被
//    覆写为非 UTF-8 数据，我们就会得到未定义行为。
pub(crate) unsafe fn append_to_string<F>(buf: &mut String, f: F) -> Result<usize>
where
    F: FnOnce(&mut Vec<u8>) -> Result<usize>,
{
    let mut g = Guard { len: buf.len(), buf: unsafe { buf.as_mut_vec() } };
    let ret = f(g.buf);

    // 安全性：调用方承诺只会向 `buf` 追加数据
    let appended = unsafe { g.buf.get_unchecked(g.len..) };
    if str::from_utf8(appended).is_err() {
        ret.and_then(|_| Err(Error::INVALID_UTF8))
    } else {
        g.len = g.buf.len();
        ret
    }
}

// 这里我们必须同时伺候许多目标互相冲突的「主人」：
//
// - 除非必要，否则避免分配
// - 如果我们知道确切大小，避免过度分配（#89165）
// - 避免把大缓冲区传给那些「在执行短读时总会初始化空闲容量」的 reader（#23815、#23820）
// - 把大缓冲区传给那些「不初始化备用容量」的 reader。这能摊薄每次调用的开销
// - 最后，给 Windows 的读取 API 传不太小也不太大的缓冲区，因为它们居然同时受这两个问题困扰，
//   即：小读受系统调用开销之苦，而所有读取又都要付出与缓冲区大小成正比的代价（#110650）
//
pub(crate) fn default_read_to_end<R: Read + ?Sized>(
    r: &mut R,
    buf: &mut Vec<u8>,
    size_hint: Option<usize>,
) -> Result<usize> {
    let start_len = buf.len();
    let start_cap = buf.capacity();
    // 可选地限制每次迭代读取的最大字节数。
    // 这里加上一个随意的微调因子（fiddle factor），以容纳比我们预期更多的数据。
    let mut max_read_size = size_hint
        .and_then(|s| s.checked_add(1024)?.checked_next_multiple_of(DEFAULT_BUF_SIZE))
        .unwrap_or(DEFAULT_BUF_SIZE);

    let mut initialized = 0; // 来自上一轮循环迭代的、额外已初始化的字节

    const PROBE_SIZE: usize = 32;

    fn small_probe_read<R: Read + ?Sized>(r: &mut R, buf: &mut Vec<u8>) -> Result<usize> {
        let mut probe = [0u8; PROBE_SIZE];

        loop {
            match r.read(&mut probe) {
                Ok(n) => {
                    // 这里无法从分配失败中恢复，因为数据已经被读取了。
                    buf.extend_from_slice(&probe[..n]);
                    return Ok(n);
                }
                Err(ref e) if e.is_interrupted() => continue,
                Err(e) => return Err(e),
            }
        }
    }

    // 在我们确定确实有东西可读之前，避免给空的/小的 vec 扩容
    if (size_hint.is_none() || size_hint == Some(0)) && buf.capacity() - buf.len() < PROBE_SIZE {
        let read = small_probe_read(r, buf)?;

        if read == 0 {
            return Ok(0);
        }
    }

    let mut consecutive_short_reads = 0;

    loop {
        if buf.len() == buf.capacity() && buf.capacity() == start_cap {
            // 缓冲区可能恰好正好装满。我们读到一个探测缓冲区里，看看它是否返回 `Ok(0)`。
            // 如果是，我们就避免了一次不必要的容量翻倍。但如果不是，就把探测缓冲区追加到主
            // 缓冲区上，让其容量增长。
            let read = small_probe_read(r, buf)?;

            if read == 0 {
                return Ok(buf.len() - start_len);
            }
        }

        if buf.len() == buf.capacity() {
            // buf 已满，需要更多空间
            buf.try_reserve(PROBE_SIZE)?;
        }

        let mut spare = buf.spare_capacity_mut();
        let buf_len = cmp::min(spare.len(), max_read_size);
        spare = &mut spare[..buf_len];
        let mut read_buf: BorrowedBuf<'_> = spare.into();

        // 安全性：这些字节在上一轮循环中已被初始化但未被填充
        unsafe {
            read_buf.set_init(initialized);
        }

        let mut cursor = read_buf.unfilled();
        let result = loop {
            match r.read_buf(cursor.reborrow()) {
                Err(e) if e.is_interrupted() => continue,
                // 出错时不要立刻停止：我们可能同时收到了数据和一个错误
                res => break res,
            }
        };

        let unfilled_but_initialized = cursor.init_mut().len();
        let bytes_read = cursor.written();
        let was_fully_initialized = read_buf.init_len() == buf_len;

        // 安全性：BorrowedBuf 的不变量意味着这么多内存是已初始化的。
        unsafe {
            let new_len = bytes_read + buf.len();
            buf.set_len(new_len);
        }

        // 既然所有数据都已推入向量，我们现在可以在不丢失数据的情况下失败
        result?;

        if bytes_read == 0 {
            return Ok(buf.len() - start_len);
        }

        if bytes_read < buf_len {
            consecutive_short_reads += 1;
        } else {
            consecutive_short_reads = 0;
        }

        // 记录有多少字节被初始化了但未被填充
        initialized = unfilled_but_initialized;

        // 如果没有提供初始的大小提示，则使用启发式方法来确定最大读取大小
        if size_hint.is_none() {
            // 该 reader 在返回短读，但它不调用 ensure_init()。
            // 在这种情况下，我们不再需要为了避免初始化开销而限制读取大小。
            // 从磁盘读取时，除了在 EOF 处，我们通常不会遇到任何短读。
            // 所以我们在解除读取缓冲区上限之前，会至少等待 2 次短读；
            // 这有助于解决前述的 Windows 问题。
            if !was_fully_initialized && consecutive_short_reads > 1 {
                max_read_size = usize::MAX;
            }

            // 我们已经传入了比之前更大的缓冲区，而 reader 仍未返回短读
            if buf_len >= max_read_size && bytes_read == buf_len {
                max_read_size = max_read_size.saturating_mul(2);
            }
        }
    }
}

pub(crate) fn default_read_to_string<R: Read + ?Sized>(
    r: &mut R,
    buf: &mut String,
    size_hint: Option<usize>,
) -> Result<usize> {
    // 注意，这里我们*不*调用 `r.read_to_end()`。我们把一个 `&mut Vec<u8>`（即 `buf` 的原始
    // 内容）传给 `read_to_end` 方法来填充它。某个任意的实现可能会覆写该向量的全部内容，而不仅仅
    // 是向它追加（追加才是我们所期望的）。
    //
    // 为了避免多余地检查整个缓冲区的 UTF-8 合规性，我们把它传给我们硬编码的 `default_read_to_end`
    // 实现——我们知道它保证只会把数据读到缓冲区的末尾。
    unsafe { append_to_string(buf, |b| default_read_to_end(r, b, size_hint)) }
}

pub(crate) fn default_read_vectored<F>(read: F, bufs: &mut [IoSliceMut<'_>]) -> Result<usize>
where
    F: FnOnce(&mut [u8]) -> Result<usize>,
{
    let buf = bufs.iter_mut().find(|b| !b.is_empty()).map_or(&mut [][..], |b| &mut **b);
    read(buf)
}

pub(crate) fn default_write_vectored<F>(write: F, bufs: &[IoSlice<'_>]) -> Result<usize>
where
    F: FnOnce(&[u8]) -> Result<usize>,
{
    let buf = bufs.iter().find(|b| !b.is_empty()).map_or(&[][..], |b| &**b);
    write(buf)
}

pub(crate) fn default_read_exact<R: Read + ?Sized>(this: &mut R, mut buf: &mut [u8]) -> Result<()> {
    while !buf.is_empty() {
        match this.read(buf) {
            Ok(0) => break,
            Ok(n) => {
                buf = &mut buf[n..];
            }
            Err(ref e) if e.is_interrupted() => {}
            Err(e) => return Err(e),
        }
    }
    if !buf.is_empty() { Err(Error::READ_EXACT_EOF) } else { Ok(()) }
}

pub(crate) fn default_read_buf<F>(read: F, mut cursor: BorrowedCursor<'_>) -> Result<()>
where
    F: FnOnce(&mut [u8]) -> Result<usize>,
{
    let n = read(cursor.ensure_init().init_mut())?;
    cursor.advance(n);
    Ok(())
}

pub(crate) fn default_read_buf_exact<R: Read + ?Sized>(
    this: &mut R,
    mut cursor: BorrowedCursor<'_>,
) -> Result<()> {
    while cursor.capacity() > 0 {
        let prev_written = cursor.written();
        match this.read_buf(cursor.reborrow()) {
            Ok(()) => {}
            Err(e) if e.is_interrupted() => continue,
            Err(e) => return Err(e),
        }

        if cursor.written() == prev_written {
            return Err(Error::READ_EXACT_EOF);
        }
    }

    Ok(())
}

pub(crate) fn default_write_fmt<W: Write + ?Sized>(
    this: &mut W,
    args: fmt::Arguments<'_>,
) -> Result<()> {
    // 创建一个填充层（shim），它把一个 `Write` 翻译成一个 `fmt::Write`，并把 I/O 错误保存
    // 下来，而不是丢弃它们。
    struct Adapter<'a, T: ?Sized + 'a> {
        inner: &'a mut T,
        error: Result<()>,
    }

    impl<T: Write + ?Sized> fmt::Write for Adapter<'_, T> {
        fn write_str(&mut self, s: &str) -> fmt::Result {
            match self.inner.write_all(s.as_bytes()) {
                Ok(()) => Ok(()),
                Err(e) => {
                    self.error = Err(e);
                    Err(fmt::Error)
                }
            }
        }
    }

    let mut output = Adapter { inner: this, error: Ok(()) };
    match fmt::write(&mut output, args) {
        Ok(()) => Ok(()),
        Err(..) => {
            // 检查错误是否来自底层的 `Write`。
            if output.error.is_err() {
                output.error
            } else {
                // 这不应该发生：底层流并没有出错，但格式化器却莫名其妙地出错了？
                panic!(
                    "a formatting trait implementation returned an error when the underlying stream did not"
                );
            }
        }
    }
}

/// `Read` trait 允许从某个来源读取字节。
///
/// `Read` trait 的实现者被称为「reader」（读取器）。
///
/// reader 由一个必需方法 [`read()`] 定义。每次对 [`read()`] 的调用都会尝试把字节从这个来源
/// 拉取到一个提供的缓冲区里。许多其他方法都是基于 [`read()`] 实现的，这就给了实现者多种读取
/// 字节的方式，而只需实现单个方法。
///
/// reader 旨在彼此可组合。整个 [`std::io`] 中的许多实现者都接受并提供实现了 `Read` trait 的
/// 类型。
///
/// 请注意，每次对 [`read()`] 的调用都可能涉及一次系统调用，因此，使用某个实现了 [`BufRead`]
/// 的东西（例如 [`BufReader`]）会更高效。
///
/// 对 reader 的重复调用使用同一个游标（cursor），所以举例来说，在一个 [`File`] 上调用两次
/// `read_to_end` 只会返回该文件的内容一次。在那种情况下，建议先调用 `rewind()`。
///
/// # 示例
///
/// [`File`] 实现了 `Read`：
///
/// ```no_run
/// use std::io;
/// use std::io::prelude::*;
/// use std::fs::File;
///
/// fn main() -> io::Result<()> {
///     let mut f = File::open("foo.txt")?;
///     let mut buffer = [0; 10];
///
///     // 最多读取 10 个字节
///     f.read(&mut buffer)?;
///
///     let mut buffer = Vec::new();
///     // 读取整个文件
///     f.read_to_end(&mut buffer)?;
///
///     // 读取到一个 String 中，这样你就不需要做转换。
///     let mut buffer = String::new();
///     f.read_to_string(&mut buffer)?;
///
///     // 还有更多！更多细节请参见其他方法。
///     Ok(())
/// }
/// ```
///
/// 从 [`&str`] 读取，因为 [`&[u8]`][prim@slice] 实现了 `Read`：
///
/// ```no_run
/// # use std::io;
/// use std::io::prelude::*;
///
/// fn main() -> io::Result<()> {
///     let mut b = "This string will be read".as_bytes();
///     let mut buffer = [0; 10];
///
///     // 最多读取 10 个字节
///     b.read(&mut buffer)?;
///
///     // 等等……它的工作方式和 File 完全一样！
///     Ok(())
/// }
/// ```
///
/// [`read()`]: Read::read
/// [`&str`]: prim@str
/// [`std::io`]: self
/// [`File`]: crate::fs::File
#[stable(feature = "rust1", since = "1.0.0")]
#[doc(notable_trait)]
#[cfg_attr(not(test), rustc_diagnostic_item = "IoRead")]
pub trait Read {
    /// 从这个来源拉取一些字节到指定的缓冲区中，返回读取了多少字节。
    ///
    /// 这个函数不就「它是否会阻塞以等待数据」提供任何保证，但如果一个对象需要为读取而阻塞却
    /// 又无法阻塞，它通常会通过一个 [`Err`] 返回值来发出此信号。
    ///
    /// 如果这个方法的返回值是 [`Ok(n)`]，那么实现必须保证 `0 <= n <= buf.len()`。一个非零的
    /// `n` 值表示缓冲区 `buf` 已被填入了来自这个来源的 `n` 个字节的数据。如果 `n` 为 `0`，
    /// 那么它可以表示以下两种情形之一：
    ///
    /// 1. 这个 reader 已到达其「文件结尾」，很可能不再能产出字节。注意，这并不意味着该 reader
    ///    将*永远*不再能产出字节。举例来说，在 Linux 上，对于一个 [`TcpStream`]，这个方法会
    ///    调用 `recv` 系统调用，此时返回零表示连接已正确关闭。而对于 [`File`]，到达文件结尾并
    ///    得到结果零是可能的，但如果有更多数据被追加到该文件，那么未来对 `read` 的调用将返回
    ///    更多数据。
    /// 2. 指定的缓冲区长度为 0 字节。
    ///
    /// 即便 reader 尚未到达流的末尾，返回值 `n` 小于缓冲区大小也不是错误。
    /// 例如，这可能是因为此刻实际可用的字节较少（例如接近文件结尾），或者因为 read() 被某个
    /// 信号中断了。
    ///
    /// 由于这个 trait 实现起来是安全的，unsafe 代码中的调用方不能为了安全性而依赖
    /// `n <= buf.len()`。
    /// 当使用 `unsafe` 函数去访问读取到的字节时，需要格外小心。
    /// 调用方必须确保：即便 `n > buf.len()`，也不可能发生任何未经检查的越界访问。
    ///
    /// 这个方法的*实现*在该函数被调用时，不能对 `buf` 的内容做任何假设。建议实现只向 `buf`
    /// 写入数据，而不是读取它的内容。
    ///
    /// 然而相应地，unsafe 代码中这个方法的*调用方*不得就「实现如何使用 `buf`」假设任何保证。
    /// 该 trait 实现起来是安全的，所以那个本应向缓冲区写入的代码也可能从中读取。确保 `buf` 在
    /// 调用 `read` 之前已被初始化，是你的责任。用一个未初始化的 `buf`（例如通过
    /// [`MaybeUninit<T>`] 获得的那种）来调用 `read` 是不安全的，并可能导致未定义行为。
    ///
    /// [`MaybeUninit<T>`]: crate::mem::MaybeUninit
    ///
    /// # 错误(Errors）
    ///
    /// 如果这个函数遇到任何形式的 I/O 或其他错误，将返回一个错误变体。如果返回了错误，那么必须
    /// 保证没有读取任何字节。
    ///
    /// 一个 [`ErrorKind::Interrupted`] 种类的错误并非致命，如果没有别的事情要做，应当重试该读取
    /// 操作。
    ///
    /// # 示例
    ///
    /// [`File`] 实现了 `Read`：
    ///
    /// [`Ok(n)`]: Ok
    /// [`File`]: crate::fs::File
    /// [`TcpStream`]: crate::net::TcpStream
    ///
    /// ```no_run
    /// use std::io;
    /// use std::io::prelude::*;
    /// use std::fs::File;
    ///
    /// fn main() -> io::Result<()> {
    ///     let mut f = File::open("foo.txt")?;
    ///     let mut buffer = [0; 10];
    ///
    ///     // 最多读取 10 个字节
    ///     let n = f.read(&mut buffer[..])?;
    ///
    ///     println!("The bytes: {:?}", &buffer[..n]);
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    fn read(&mut self, buf: &mut [u8]) -> Result<usize>;

    /// 类似于 `read`，但它读取到一组缓冲区切片中。
    ///
    /// 数据按顺序复制以填充每个缓冲区，最后一个被写入的缓冲区可能只被部分填充。这个方法的行为
    /// 必须等价于以拼接后的缓冲区单次调用 `read`。
    ///
    /// 默认实现以「提供的第一个非空缓冲区」（若不存在则以一个空缓冲区）调用 `read`。
    #[stable(feature = "iovec", since = "1.36.0")]
    fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> Result<usize> {
        default_read_vectored(|b| self.read(b), bufs)
    }

    /// 判断这个 `Read`er 是否有一个高效的 `read_vectored` 实现。
    ///
    /// 如果一个 `Read`er 没有重写默认的 `read_vectored` 实现，那么使用它的代码可能想完全避开
    /// 这个方法，转而把多次写入合并到单个缓冲区中，以获得更高的性能。
    ///
    /// 默认实现返回 `false`。
    #[unstable(feature = "can_vector", issue = "69941")]
    fn is_read_vectored(&self) -> bool {
        false
    }

    /// 读取这个来源中直到 EOF 为止的所有字节，把它们放入 `buf`。
    ///
    /// 从这个来源读取到的所有字节都会被追加到指定的缓冲区 `buf` 上。这个函数将持续调用
    /// [`read()`] 以向 `buf` 追加更多数据，直到 [`read()`] 返回 [`Ok(0)`] 或者一个
    /// 非 [`ErrorKind::Interrupted`] 种类的错误为止。
    ///
    /// 如果成功，这个函数将返回读取到的字节总数。
    ///
    /// # 错误(Errors）
    ///
    /// 如果这个函数遇到一个 [`ErrorKind::Interrupted`] 种类的错误，那么该错误会被忽略，操作将
    /// 继续。
    ///
    /// 如果遇到任何其他读取错误，那么这个函数会立即返回。任何已经被读取的字节都会被追加到 `buf`
    /// 上。
    ///
    /// # 示例
    ///
    /// [`File`] 实现了 `Read`：
    ///
    /// [`read()`]: Read::read
    /// [`Ok(0)`]: Ok
    /// [`File`]: crate::fs::File
    ///
    /// ```no_run
    /// use std::io;
    /// use std::io::prelude::*;
    /// use std::fs::File;
    ///
    /// fn main() -> io::Result<()> {
    ///     let mut f = File::open("foo.txt")?;
    ///     let mut buffer = Vec::new();
    ///
    ///     // 读取整个文件
    ///     f.read_to_end(&mut buffer)?;
    ///     Ok(())
    /// }
    /// ```
    ///
    /// （另请参见用于从文件读取的便捷函数 [`std::fs::read`]。）
    ///
    /// [`std::fs::read`]: crate::fs::read
    ///
    /// ## 实现 `read_to_end`
    ///
    /// 在实现 `io::Read` trait 时，建议使用 [`Vec::try_reserve`] 来分配内存。然而，并非所有实现
    /// 都保证这种行为，`read_to_end` 也可能无法优雅地处理内存耗尽（out-of-memory）的情形。
    ///
    /// ```no_run
    /// # use std::io::{self, BufRead};
    /// # struct Example { example_datasource: io::Empty } impl Example {
    /// # fn get_some_data_for_the_example(&self) -> &'static [u8] { &[] }
    /// fn read_to_end(&mut self, dest_vec: &mut Vec<u8>) -> io::Result<usize> {
    ///     let initial_vec_len = dest_vec.len();
    ///     loop {
    ///         let src_buf = self.example_datasource.fill_buf()?;
    ///         if src_buf.is_empty() {
    ///             break;
    ///         }
    ///         dest_vec.try_reserve(src_buf.len())?;
    ///         dest_vec.extend_from_slice(src_buf);
    ///
    ///         // 任何不可逆的副作用都应当在 `try_reserve` 成功之后发生，
    ///         // 以避免在分配出错时丢失数据。
    ///         let read = src_buf.len();
    ///         self.example_datasource.consume(read);
    ///     }
    ///     Ok(dest_vec.len() - initial_vec_len)
    /// }
    /// # }
    /// ```
    ///
    /// # 使用须知
    ///
    /// `read_to_end` 会尝试读取一个来源直到 EOF，但许多来源是不发送 EOF 的连续流。在这些情况下，
    /// `read_to_end` 将无限期地阻塞。标准输入就是这样一种流：如果通过管道传入它可能是有限的，但
    /// 它通常是连续的。例如，`cat file | my-rust-program` 会在 cat 关闭时随着一个 `EOF` 正确终止。
    /// 读取用户输入、或运行那些无限期保持打开的程序，将永远不会以 `EOF` 终止该流
    ///（例如 `yes | my-rust-program`）。
    ///
    /// 对 [`BufReader`] 使用 `.lines()`，或者使用 [`read`]，都能提供一个更好的解决方案
    ///
    ///[`read`]: Read::read
    ///
    /// [`Vec::try_reserve`]: crate::vec::Vec::try_reserve
    #[stable(feature = "rust1", since = "1.0.0")]
    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> Result<usize> {
        default_read_to_end(self, buf, None)
    }

    /// 读取这个来源中直到 EOF 为止的所有字节，把它们追加到 `buf`。
    ///
    /// 如果成功，这个函数返回被读取并追加到 `buf` 的字节数。
    ///
    /// # 错误(Errors）
    ///
    /// 如果这个流中的数据*不是*有效的 UTF-8，那么会返回一个错误，且 `buf` 保持不变。
    ///
    /// 其他错误语义参见 [`read_to_end`]。
    ///
    /// [`read_to_end`]: Read::read_to_end
    ///
    /// # 示例
    ///
    /// [`File`] 实现了 `Read`：
    ///
    /// [`File`]: crate::fs::File
    ///
    /// ```no_run
    /// use std::io;
    /// use std::io::prelude::*;
    /// use std::fs::File;
    ///
    /// fn main() -> io::Result<()> {
    ///     let mut f = File::open("foo.txt")?;
    ///     let mut buffer = String::new();
    ///
    ///     f.read_to_string(&mut buffer)?;
    ///     Ok(())
    /// }
    /// ```
    ///
    /// （另请参见用于从文件读取的便捷函数 [`std::fs::read_to_string`]。）
    ///
    /// # 使用须知
    ///
    /// `read_to_string` 会尝试读取一个来源直到 EOF，但许多来源是不发送 EOF 的连续流。在这些情况下，
    /// `read_to_string` 将无限期地阻塞。标准输入就是这样一种流：如果通过管道传入它可能是有限的，但
    /// 它通常是连续的。例如，`cat file | my-rust-program` 会在 cat 关闭时随着一个 `EOF` 正确终止。
    /// 读取用户输入、或运行那些无限期保持打开的程序，将永远不会以 `EOF` 终止该流
    ///（例如 `yes | my-rust-program`）。
    ///
    /// 对 [`BufReader`] 使用 `.lines()`，或者使用 [`read`]，都能提供一个更好的解决方案
    ///
    ///[`read`]: Read::read
    ///
    /// [`std::fs::read_to_string`]: crate::fs::read_to_string
    #[stable(feature = "rust1", since = "1.0.0")]
    fn read_to_string(&mut self, buf: &mut String) -> Result<usize> {
        default_read_to_string(self, buf, None)
    }

    /// 读取恰好填满 `buf` 所需的字节数。
    ///
    /// 这个函数会读取尽可能多的字节，以完全填满指定的缓冲区 `buf`。
    ///
    /// 这个方法的*实现*在该函数被调用时，不能对 `buf` 的内容做任何假设。建议实现只向 `buf` 写入
    /// 数据，而不是读取它的内容。[`read`] 的文档对这个话题有更详细的解释。
    ///
    /// # 错误(Errors）
    ///
    /// 如果这个函数遇到一个 [`ErrorKind::Interrupted`] 种类的错误，那么该错误会被忽略，操作将继续。
    ///
    /// 如果这个函数在完全填满缓冲区之前遇到「文件结尾」，它会返回一个 [`ErrorKind::UnexpectedEof`]
    /// 种类的错误。在这种情况下，`buf` 的内容是未指定的。
    ///
    /// 如果遇到任何其他读取错误，那么这个函数会立即返回。在这种情况下，`buf` 的内容是未指定的。
    ///
    /// 如果这个函数返回了错误，那么它已读取了多少字节是未指定的，但它读取的字节数绝不会超过完全
    /// 填满缓冲区所需的数量。
    ///
    /// # 示例
    ///
    /// [`File`] 实现了 `Read`：
    ///
    /// [`read`]: Read::read
    /// [`File`]: crate::fs::File
    ///
    /// ```no_run
    /// use std::io;
    /// use std::io::prelude::*;
    /// use std::fs::File;
    ///
    /// fn main() -> io::Result<()> {
    ///     let mut f = File::open("foo.txt")?;
    ///     let mut buffer = [0; 10];
    ///
    ///     // 恰好读取 10 个字节
    ///     f.read_exact(&mut buffer)?;
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "read_exact", since = "1.6.0")]
    fn read_exact(&mut self, buf: &mut [u8]) -> Result<()> {
        default_read_exact(self, buf)
    }

    /// 从这个来源拉取一些字节到指定的缓冲区中。
    ///
    /// 这等价于 [`read`](Read::read) 方法，区别在于它接受一个 [`BorrowedCursor`] 而非 `[u8]`，
    /// 以便能配合未初始化的缓冲区使用。新数据将被追加到 `buf` 已有的任何内容之后。
    ///
    /// 默认实现委托给 `read`。
    ///
    /// 这个方法使得同时返回数据和错误成为可能，但不建议这样做。
    #[unstable(feature = "read_buf", issue = "78485")]
    fn read_buf(&mut self, buf: BorrowedCursor<'_>) -> Result<()> {
        default_read_buf(|b| self.read(b), buf)
    }

    /// 读取恰好填满 `cursor` 所需的字节数。
    ///
    /// 这类似于 [`read_exact`](Read::read_exact) 方法，区别在于它接受一个 [`BorrowedCursor`]
    /// 而非 `[u8]`，以便能配合未初始化的缓冲区使用。
    ///
    /// # 错误(Errors）
    ///
    /// 如果这个函数遇到一个 [`ErrorKind::Interrupted`] 种类的错误，那么该错误会被忽略，操作将继续。
    ///
    /// 如果这个函数在完全填满缓冲区之前遇到「文件结尾」，它会返回一个 [`ErrorKind::UnexpectedEof`]
    /// 种类的错误。
    ///
    /// 如果遇到任何其他读取错误，那么这个函数会立即返回。
    ///
    /// 如果这个函数返回了错误，那么所有已读取的字节都会被追加到 `cursor`。
    #[unstable(feature = "read_buf", issue = "78485")]
    fn read_buf_exact(&mut self, cursor: BorrowedCursor<'_>) -> Result<()> {
        default_read_buf_exact(self, cursor)
    }

    /// 为这个 `Read` 实例创建一个「按引用」（by reference）适配器。
    ///
    /// 返回的适配器也实现了 `Read`，它只是简单地借用当前这个 reader。
    ///
    /// # 示例
    ///
    /// [`File`] 实现了 `Read`：
    ///
    /// [`File`]: crate::fs::File
    ///
    /// ```no_run
    /// use std::io;
    /// use std::io::Read;
    /// use std::fs::File;
    ///
    /// fn main() -> io::Result<()> {
    ///     let mut f = File::open("foo.txt")?;
    ///     let mut buffer = Vec::new();
    ///     let mut other_buffer = Vec::new();
    ///
    ///     {
    ///         let reference = f.by_ref();
    ///
    ///         // 最多读取 5 个字节
    ///         reference.take(5).read_to_end(&mut buffer)?;
    ///
    ///     } // 丢弃我们的 &mut 引用，这样就能再次使用 f
    ///
    ///     // 原始文件仍然可用，读取剩余部分
    ///     f.read_to_end(&mut other_buffer)?;
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    fn by_ref(&mut self) -> &mut Self
    where
        Self: Sized,
    {
        self
    }

    /// 把这个 `Read` 实例转换成一个遍历其字节的 [`Iterator`]。
    ///
    /// 返回的类型实现了 [`Iterator`]，其中 [`Item`] 为
    /// <code>[Result]<[u8], [io::Error]></code>。
    /// 如果成功读取了一个字节，产出的项是 [`Ok`]，否则是 [`Err`]。EOF 被映射为从这个迭代器
    /// 返回 [`None`]。
    ///
    /// 默认实现对每个字节都调用 `read`，对于不在内存中的数据（例如 [`File`]）这可能非常低效。
    /// 在这种情况下，考虑使用 [`BufReader`]。
    ///
    /// # 示例
    ///
    /// [`File`] 实现了 `Read`：
    ///
    /// [`Item`]: Iterator::Item
    /// [`File`]: crate::fs::File "fs::File"
    /// [Result]: crate::result::Result "Result"
    /// [io::Error]: self::Error "io::Error"
    ///
    /// ```no_run
    /// use std::io;
    /// use std::io::prelude::*;
    /// use std::io::BufReader;
    /// use std::fs::File;
    ///
    /// fn main() -> io::Result<()> {
    ///     let f = BufReader::new(File::open("foo.txt")?);
    ///
    ///     for byte in f.bytes() {
    ///         println!("{}", byte?);
    ///     }
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    fn bytes(self) -> Bytes<Self>
    where
        Self: Sized,
    {
        Bytes { inner: self }
    }

    /// 创建一个适配器，它会把这个流与另一个流链接（chain）起来。
    ///
    /// 返回的 `Read` 实例会先从这个对象读取所有字节，直到遇到 EOF。此后其输出等价于 `next`
    /// 的输出。
    ///
    /// # 示例
    ///
    /// [`File`] 实现了 `Read`：
    ///
    /// [`File`]: crate::fs::File
    ///
    /// ```no_run
    /// use std::io;
    /// use std::io::prelude::*;
    /// use std::fs::File;
    ///
    /// fn main() -> io::Result<()> {
    ///     let f1 = File::open("foo.txt")?;
    ///     let f2 = File::open("bar.txt")?;
    ///
    ///     let mut handle = f1.chain(f2);
    ///     let mut buffer = String::new();
    ///
    ///     // 把值读入一个 String。这里我们可以用任何 Read 方法，
    ///     // 这只是一个例子。
    ///     handle.read_to_string(&mut buffer)?;
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    fn chain<R: Read>(self, next: R) -> Chain<Self, R>
    where
        Self: Sized,
    {
        Chain { first: self, second: next, done_first: false }
    }

    /// 创建一个适配器，它最多从中读取 `limit` 个字节。
    ///
    /// 这个函数返回一个新的 `Read` 实例，它最多读取 `limit` 个字节，此后将始终返回 EOF
    ///（[`Ok(0)`]）。任何读取错误都不会计入已读取的字节数，未来对 [`read()`] 的调用可能成功。
    ///
    /// # 示例
    ///
    /// [`File`] 实现了 `Read`：
    ///
    /// [`File`]: crate::fs::File
    /// [`Ok(0)`]: Ok
    /// [`read()`]: Read::read
    ///
    /// ```no_run
    /// use std::io;
    /// use std::io::prelude::*;
    /// use std::fs::File;
    ///
    /// fn main() -> io::Result<()> {
    ///     let f = File::open("foo.txt")?;
    ///     let mut buffer = [0; 5];
    ///
    ///     // 最多读取五个字节
    ///     let mut handle = f.take(5);
    ///
    ///     handle.read(&mut buffer)?;
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    fn take(self, limit: u64) -> Take<Self>
    where
        Self: Sized,
    {
        Take { inner: self, len: limit, limit }
    }

    /// 从这个来源读取并返回一个固定大小的字节数组。
    ///
    /// 这个函数使用一个大小基于编译期已知的 const 泛型尺寸的数组。你可以用 turbofish 指定大小
    ///（`reader.read_array::<8>()`），或者让类型推断根据返回值的使用方式来确定所需的字节数。
    /// 例如，这个函数与 [`u64::from_le_bytes`] 之类的函数配合良好，可把一个字节数组转换成一个
    /// 相同大小的整数。
    ///
    /// 与 `read_exact` 一样，如果这个函数在读取到所需字节数之前遇到「文件结尾」，它会返回一个
    /// [`ErrorKind::UnexpectedEof`] 种类的错误。
    ///
    /// ```
    /// #![feature(read_array)]
    /// use std::io::Cursor;
    /// use std::io::prelude::*;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let mut buf = Cursor::new([1, 2, 3, 4, 5, 6, 7, 8, 9, 8, 7, 6, 5, 4, 3, 2]);
    ///     let x = u64::from_le_bytes(buf.read_array()?);
    ///     let y = u32::from_be_bytes(buf.read_array()?);
    ///     let z = u16::from_be_bytes(buf.read_array()?);
    ///     assert_eq!(x, 0x807060504030201);
    ///     assert_eq!(y, 0x9080706);
    ///     assert_eq!(z, 0x504);
    ///     Ok(())
    /// }
    /// ```
    #[unstable(feature = "read_array", issue = "148848")]
    fn read_array<const N: usize>(&mut self) -> Result<[u8; N]>
    where
        Self: Sized,
    {
        let mut buf = [MaybeUninit::uninit(); N];
        let mut borrowed_buf = BorrowedBuf::from(buf.as_mut_slice());
        self.read_buf_exact(borrowed_buf.unfilled())?;
        // 防范不正确的 `read_buf_exact` 实现。
        assert_eq!(borrowed_buf.len(), N);
        Ok(unsafe { MaybeUninit::array_assume_init(buf) })
    }
}

/// 从一个 [reader][Read] 读取所有字节到一个新的 [`String`] 中。
///
/// 这是 [`Read::read_to_string`] 的一个便捷函数。使用这个函数可以免去先创建一个变量的麻烦，
/// 并提供更强的类型安全性，因为只有在没有错误时你才能取出缓冲区。（如果你使用
/// [`Read::read_to_string`]，你就必须记得检查读取是否成功，否则你的缓冲区会是空的或只被部分
/// 填满。）
///
/// # 性能
///
/// 这个函数在易用性和类型安全性上的提升，其代价是它让你对性能的掌控更少。例如，你无法像使用
/// [`String::with_capacity`] 和 [`Read::read_to_string`] 那样预分配内存。此外，如果在读取过程中
/// 发生错误，你也无法复用缓冲区。
///
/// 在许多情况下，这个函数的性能将是足够的，易用性和类型安全性方面的取舍也是值得的。然而，在某些
/// 你需要对性能有更多掌控的情况下，你绝对应当直接使用 [`Read::read_to_string`]。
///
/// 注意，在某些特殊情况下（例如读取文件时），这个函数会根据它所读取的输入的大小来预分配内存。在
/// 那些情况下，其性能应当和你「使用 [`Read::read_to_string`] 并手动预分配缓冲区」一样好。
///
/// # 错误(Errors）
///
/// 这个函数强制你处理错误，因为其输出（`String`）被包装在一个 [`Result`] 中。可能发生的错误参见
/// [`Read::read_to_string`]。如果发生任何错误，你将得到一个 [`Err`]，因此你不必担心你的缓冲区
/// 会是空的或被部分填满。
///
/// # 示例
///
/// ```no_run
/// # use std::io;
/// fn main() -> io::Result<()> {
///     let stdin = io::read_to_string(io::stdin())?;
///     println!("Stdin was:");
///     println!("{stdin}");
///     Ok(())
/// }
/// ```
///
/// # 使用须知
///
/// `read_to_string` 会尝试一直读取数据源直到 EOF，但许多数据源是连续的流，
/// 不会发送 EOF。在这些情况下，`read_to_string` 会无限期阻塞。标准输入就是
/// 这样一种流：如果通过管道传入它可能是有限的，但通常是连续的。例如，
/// `cat file | my-rust-program` 会在 cat 关闭时正确地以 `EOF` 终止。
/// 而读取用户输入或运行那些保持无限期打开的程序，则永远不会以 `EOF`
/// 终止该流（例如 `yes | my-rust-program`）。
///
/// 对 [`BufReader`] 使用 `.lines()`，或使用 [`read`]，可以提供更好的解决方案
///
///[`read`]: Read::read
///
#[stable(feature = "io_read_to_string", since = "1.65.0")]
pub fn read_to_string<R: Read>(mut reader: R) -> Result<String> {
    let mut buf = String::new();
    reader.read_to_string(&mut buf)?;
    Ok(buf)
}

/// 与 `Read::read_vectored` 搭配使用的缓冲区类型。
///
/// 在语义上它是对 `&mut [u8]` 的一层包装，但保证在 Unix 平台上与 `iovec` 类型、在 Windows 上
/// 与 `WSABUF` 在 ABI 上兼容。
#[stable(feature = "iovec", since = "1.36.0")]
#[repr(transparent)]
pub struct IoSliceMut<'a>(sys::io::IoSliceMut<'a>);

#[stable(feature = "iovec_send_sync", since = "1.44.0")]
unsafe impl<'a> Send for IoSliceMut<'a> {}

#[stable(feature = "iovec_send_sync", since = "1.44.0")]
unsafe impl<'a> Sync for IoSliceMut<'a> {}

#[stable(feature = "iovec", since = "1.36.0")]
impl<'a> fmt::Debug for IoSliceMut<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.0.as_slice(), fmt)
    }
}

impl<'a> IoSliceMut<'a> {
    /// 创建一个新的 `IoSliceMut`，包装一个字节切片。
    ///
    /// # Panics
    ///
    /// 在 Windows 上，如果该切片大于 4GB，则会 panic。
    #[stable(feature = "iovec", since = "1.36.0")]
    #[inline]
    pub fn new(buf: &'a mut [u8]) -> IoSliceMut<'a> {
        IoSliceMut(sys::io::IoSliceMut::new(buf))
    }

    /// 推进该切片的内部游标。
    ///
    /// 另请参见 [`IoSliceMut::advance_slices`] 以推进多个缓冲区的游标。
    ///
    /// # Panics
    ///
    /// 当尝试推进超过该切片的末尾时会 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::io::IoSliceMut;
    /// use std::ops::Deref;
    ///
    /// let mut data = [1; 8];
    /// let mut buf = IoSliceMut::new(&mut data);
    ///
    /// // 把 3 个字节标记为已读。
    /// buf.advance(3);
    /// assert_eq!(buf.deref(), [1; 5].as_ref());
    /// ```
    #[stable(feature = "io_slice_advance", since = "1.81.0")]
    #[inline]
    pub fn advance(&mut self, n: usize) {
        self.0.advance(n)
    }

    /// 推进「切片的切片」。
    ///
    /// 收缩这个切片，移除任何已被完全推进越过的 `IoSliceMut`。如果游标最终落在某个 `IoSliceMut`
    /// 的中间，则该 `IoSliceMut` 会被修改为从那个游标处开始。
    ///
    /// 例如，如果我们有一个含两个 8 字节 `IoSliceMut` 的切片，并推进 10 个字节，那么结果将只包含
    /// 第二个 `IoSliceMut`，且它被推进了 2 个字节。
    ///
    /// # Panics
    ///
    /// 当尝试推进超过这些切片的末尾时会 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::io::IoSliceMut;
    /// use std::ops::Deref;
    ///
    /// let mut buf1 = [1; 8];
    /// let mut buf2 = [2; 16];
    /// let mut buf3 = [3; 8];
    /// let mut bufs = &mut [
    ///     IoSliceMut::new(&mut buf1),
    ///     IoSliceMut::new(&mut buf2),
    ///     IoSliceMut::new(&mut buf3),
    /// ][..];
    ///
    /// // 把 10 个字节标记为已读。
    /// IoSliceMut::advance_slices(&mut bufs, 10);
    /// assert_eq!(bufs[0].deref(), [2; 14].as_ref());
    /// assert_eq!(bufs[1].deref(), [3; 8].as_ref());
    /// ```
    #[stable(feature = "io_slice_advance", since = "1.81.0")]
    #[inline]
    pub fn advance_slices(bufs: &mut &mut [IoSliceMut<'a>], n: usize) {
        // 要移除的缓冲区数量。
        let mut remove = 0;
        // 到达 n 之前剩余的长度。
        let mut left = n;
        for buf in bufs.iter() {
            if let Some(remainder) = left.checked_sub(buf.len()) {
                left = remainder;
                remove += 1;
            } else {
                break;
            }
        }

        *bufs = &mut take(bufs)[remove..];
        if bufs.is_empty() {
            assert!(left == 0, "advancing io slices beyond their length");
        } else {
            bufs[0].advance(left);
        }
    }

    /// 以原始的生命周期，把底层字节作为一个可变切片获取。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(io_slice_as_bytes)]
    /// use std::io::IoSliceMut;
    ///
    /// let mut data = *b"abcdef";
    /// let io_slice = IoSliceMut::new(&mut data);
    /// io_slice.into_slice()[0] = b'A';
    ///
    /// assert_eq!(&data, b"Abcdef");
    /// ```
    #[unstable(feature = "io_slice_as_bytes", issue = "132818")]
    pub const fn into_slice(self) -> &'a mut [u8] {
        self.0.into_slice()
    }
}

#[stable(feature = "iovec", since = "1.36.0")]
impl<'a> Deref for IoSliceMut<'a> {
    type Target = [u8];

    #[inline]
    fn deref(&self) -> &[u8] {
        self.0.as_slice()
    }
}

#[stable(feature = "iovec", since = "1.36.0")]
impl<'a> DerefMut for IoSliceMut<'a> {
    #[inline]
    fn deref_mut(&mut self) -> &mut [u8] {
        self.0.as_mut_slice()
    }
}

/// 与 `Write::write_vectored` 搭配使用的缓冲区类型。
///
/// 在语义上它是对 `&[u8]` 的一层包装，但保证在 Unix 平台上与 `iovec` 类型、在 Windows 上
/// 与 `WSABUF` 在 ABI 上兼容。
#[stable(feature = "iovec", since = "1.36.0")]
#[derive(Copy, Clone)]
#[repr(transparent)]
pub struct IoSlice<'a>(sys::io::IoSlice<'a>);

#[stable(feature = "iovec_send_sync", since = "1.44.0")]
unsafe impl<'a> Send for IoSlice<'a> {}

#[stable(feature = "iovec_send_sync", since = "1.44.0")]
unsafe impl<'a> Sync for IoSlice<'a> {}

#[stable(feature = "iovec", since = "1.36.0")]
impl<'a> fmt::Debug for IoSlice<'a> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.0.as_slice(), fmt)
    }
}

impl<'a> IoSlice<'a> {
    /// 创建一个新的 `IoSlice`，包装一个字节切片。
    ///
    /// # Panics
    ///
    /// 在 Windows 上，如果该切片大于 4GB，则会 panic。
    #[stable(feature = "iovec", since = "1.36.0")]
    #[must_use]
    #[inline]
    pub fn new(buf: &'a [u8]) -> IoSlice<'a> {
        IoSlice(sys::io::IoSlice::new(buf))
    }

    /// 推进该切片的内部游标。
    ///
    /// 另请参见 [`IoSlice::advance_slices`] 以推进多个缓冲区的游标。
    ///
    /// # Panics
    ///
    /// 当尝试推进超过该切片的末尾时会 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::io::IoSlice;
    /// use std::ops::Deref;
    ///
    /// let data = [1; 8];
    /// let mut buf = IoSlice::new(&data);
    ///
    /// // 把 3 个字节标记为已读。
    /// buf.advance(3);
    /// assert_eq!(buf.deref(), [1; 5].as_ref());
    /// ```
    #[stable(feature = "io_slice_advance", since = "1.81.0")]
    #[inline]
    pub fn advance(&mut self, n: usize) {
        self.0.advance(n)
    }

    /// 推进「切片的切片」。
    ///
    /// 收缩这个切片，移除任何已被完全推进越过的 `IoSlice`。如果游标最终落在某个 `IoSlice` 的中间，
    /// 则该 `IoSlice` 会被修改为从那个游标处开始。
    ///
    /// 例如，如果我们有一个含两个 8 字节 `IoSlice` 的切片，并推进 10 个字节，那么结果将只包含
    /// 第二个 `IoSlice`，且它被推进了 2 个字节。
    ///
    /// # Panics
    ///
    /// 当尝试推进超过这些切片的末尾时会 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::io::IoSlice;
    /// use std::ops::Deref;
    ///
    /// let buf1 = [1; 8];
    /// let buf2 = [2; 16];
    /// let buf3 = [3; 8];
    /// let mut bufs = &mut [
    ///     IoSlice::new(&buf1),
    ///     IoSlice::new(&buf2),
    ///     IoSlice::new(&buf3),
    /// ][..];
    ///
    /// // 把 10 个字节标记为已写入。
    /// IoSlice::advance_slices(&mut bufs, 10);
    /// assert_eq!(bufs[0].deref(), [2; 14].as_ref());
    /// assert_eq!(bufs[1].deref(), [3; 8].as_ref());
    #[stable(feature = "io_slice_advance", since = "1.81.0")]
    #[inline]
    pub fn advance_slices(bufs: &mut &mut [IoSlice<'a>], n: usize) {
        // 要移除的缓冲区数量。
        let mut remove = 0;
        // 到达 n 之前剩余的长度。这避免了「改为累加 `bufs` 中各切片的长度」时可能发生的溢出：
        // 那些切片可能彼此别名（alias），并且如果它们足够大，其累加的长度可能溢出一个 `usize`。
        let mut left = n;
        for buf in bufs.iter() {
            if let Some(remainder) = left.checked_sub(buf.len()) {
                left = remainder;
                remove += 1;
            } else {
                break;
            }
        }

        *bufs = &mut take(bufs)[remove..];
        if bufs.is_empty() {
            assert!(left == 0, "advancing io slices beyond their length");
        } else {
            bufs[0].advance(left);
        }
    }

    /// 以原始的生命周期，把底层字节作为一个切片获取。
    ///
    /// 这不从 `self` 借用，所以比调用 `.deref()`（后者会借用）的限制更少。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(io_slice_as_bytes)]
    /// use std::io::IoSlice;
    ///
    /// let data = b"abcdef";
    ///
    /// let mut io_slice = IoSlice::new(data);
    /// let tail = &io_slice.as_slice()[3..];
    ///
    /// // 这之所以可行，是因为 `tail` 并不借用 `io_slice`
    /// io_slice = IoSlice::new(tail);
    ///
    /// assert_eq!(io_slice.as_slice(), b"def");
    /// ```
    #[unstable(feature = "io_slice_as_bytes", issue = "132818")]
    pub const fn as_slice(self) -> &'a [u8] {
        self.0.as_slice()
    }
}

#[stable(feature = "iovec", since = "1.36.0")]
impl<'a> Deref for IoSlice<'a> {
    type Target = [u8];

    #[inline]
    fn deref(&self) -> &[u8] {
        self.0.as_slice()
    }
}

/// 用于「面向字节的接收端（sink）」对象的 trait。
///
/// `Write` trait 的实现者有时被称为「writer」（写入器）。
///
/// writer 由两个必需方法定义：[`write`] 和 [`flush`]：
///
/// * [`write`] 方法会尝试把一些数据写入对象，返回成功写入了多少字节。
///
/// * [`flush`] 方法对适配器以及显式缓冲区自身很有用，用于确保所有被缓冲的数据都已被推送到
///   「真正的接收端」。
///
/// writer 旨在彼此可组合。整个 [`std::io`] 中的许多实现者都接受并提供实现了 `Write` trait 的
/// 类型。
///
/// [`write`]: Write::write
/// [`flush`]: Write::flush
/// [`std::io`]: self
///
/// # 示例
///
/// ```no_run
/// use std::io::prelude::*;
/// use std::fs::File;
///
/// fn main() -> std::io::Result<()> {
///     let data = b"some bytes";
///
///     let mut pos = 0;
///     let mut buffer = File::create("foo.txt")?;
///
///     while pos < data.len() {
///         let bytes_written = buffer.write(&data[pos..])?;
///         pos += bytes_written;
///     }
///     Ok(())
/// }
/// ```
///
/// 这个 trait 还提供了诸如 [`write_all`] 之类的便捷方法，它在一个循环中调用 `write`，直到其全部
/// 输入都被写入。
///
/// [`write_all`]: Write::write_all
#[stable(feature = "rust1", since = "1.0.0")]
#[doc(notable_trait)]
#[cfg_attr(not(test), rustc_diagnostic_item = "IoWrite")]
pub trait Write {
    /// 把一个缓冲区写入这个 writer，返回写入了多少字节。
    ///
    /// 这个函数会尝试写入 `buf` 的全部内容，但整个写入可能不会成功，写入也可能产生一个错误。
    /// 通常，一次对 `write` 的调用表示对任何被包装对象的一次写入尝试。
    ///
    /// 对 `write` 的调用不保证会为等待数据被写入而阻塞，一个本应阻塞的写入可以通过一个 [`Err`]
    /// 变体来表示。
    ///
    /// 如果这个方法消耗了 `buf` 中 `n > 0` 个字节，它必须返回 [`Ok(n)`]。如果返回值是 `Ok(n)`，
    /// 那么 `n` 必须满足 `n <= buf.len()`。返回值 `Ok(0)` 通常意味着底层对象不再能接受字节、且
    /// 很可能将来也不能，或者意味着提供的缓冲区为空。
    ///
    /// # 错误(Errors）
    ///
    /// 每次对 `write` 的调用都可能产生一个 I/O 错误，表示操作无法完成。如果返回了错误，那么缓冲区
    /// 中没有任何字节被写入这个 writer。
    ///
    /// 整个缓冲区未能被写入这个 writer **不**被视为错误。
    ///
    /// 一个 [`ErrorKind::Interrupted`] 种类的错误并非致命，如果没有别的事情要做，应当重试该写入
    /// 操作。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::io::prelude::*;
    /// use std::fs::File;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let mut buffer = File::create("foo.txt")?;
    ///
    ///     // 写入该字节串的某个前缀，不一定是全部。
    ///     buffer.write(b"some bytes")?;
    ///     Ok(())
    /// }
    /// ```
    ///
    /// [`Ok(n)`]: Ok
    #[stable(feature = "rust1", since = "1.0.0")]
    fn write(&mut self, buf: &[u8]) -> Result<usize>;

    /// 类似于 [`write`]，但它从一组缓冲区切片写入。
    ///
    /// 数据按顺序从每个缓冲区复制，最后一个被读取的缓冲区可能只被部分消耗。这个方法的行为必须
    /// 与「以拼接后的缓冲区调用 [`write`]」一致。
    ///
    /// 默认实现以「提供的第一个非空缓冲区」（若不存在则以一个空缓冲区）调用 [`write`]。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::io::IoSlice;
    /// use std::io::prelude::*;
    /// use std::fs::File;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let data1 = [1; 8];
    ///     let data2 = [15; 8];
    ///     let io_slice1 = IoSlice::new(&data1);
    ///     let io_slice2 = IoSlice::new(&data2);
    ///
    ///     let mut buffer = File::create("foo.txt")?;
    ///
    ///     // 写入该字节串的某个前缀，不一定是全部。
    ///     buffer.write_vectored(&[io_slice1, io_slice2])?;
    ///     Ok(())
    /// }
    /// ```
    ///
    /// [`write`]: Write::write
    #[stable(feature = "iovec", since = "1.36.0")]
    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> Result<usize> {
        default_write_vectored(|b| self.write(b), bufs)
    }

    /// 判断这个 `Write`r 是否有一个高效的 [`write_vectored`] 实现。
    ///
    /// 如果一个 `Write`r 没有重写默认的 [`write_vectored`] 实现，那么使用它的代码可能想完全避开
    /// 这个方法，转而把多次写入合并到单个缓冲区中，以获得更高的性能。
    ///
    /// 默认实现返回 `false`。
    ///
    /// [`write_vectored`]: Write::write_vectored
    #[unstable(feature = "can_vector", issue = "69941")]
    fn is_write_vectored(&self) -> bool {
        false
    }

    /// 刷新（flush）这个输出流，确保所有中间被缓冲的内容都到达其目的地。
    ///
    /// # 错误(Errors）
    ///
    /// 如果由于 I/O 错误或到达 EOF 而未能写入所有字节，则被视为错误。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::io::prelude::*;
    /// use std::io::BufWriter;
    /// use std::fs::File;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let mut buffer = BufWriter::new(File::create("foo.txt")?);
    ///
    ///     buffer.write_all(b"some bytes")?;
    ///     buffer.flush()?;
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    fn flush(&mut self) -> Result<()>;

    /// 尝试把整个缓冲区写入这个 writer。
    ///
    /// 这个方法会持续调用 [`write`]，直到没有更多数据要写入、或者返回一个非
    /// [`ErrorKind::Interrupted`] 种类的错误为止。这个方法在整个缓冲区被成功写入、或发生上述错误
    /// 之前不会返回。这个方法产生的第一个非 [`ErrorKind::Interrupted`] 种类的错误将被返回。
    ///
    /// 如果缓冲区不含数据，这将永远不会调用 [`write`]。
    ///
    /// # 错误(Errors）
    ///
    /// 这个函数将返回 [`write`] 所返回的第一个非 [`ErrorKind::Interrupted`] 种类的错误。
    ///
    /// [`write`]: Write::write
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::io::prelude::*;
    /// use std::fs::File;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let mut buffer = File::create("foo.txt")?;
    ///
    ///     buffer.write_all(b"some bytes")?;
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    fn write_all(&mut self, mut buf: &[u8]) -> Result<()> {
        while !buf.is_empty() {
            match self.write(buf) {
                Ok(0) => {
                    return Err(Error::WRITE_ALL_EOF);
                }
                Ok(n) => buf = &buf[n..],
                Err(ref e) if e.is_interrupted() => {}
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    /// 尝试把多个缓冲区写入这个 writer。
    ///
    /// 这个方法会持续调用 [`write_vectored`]，直到没有更多数据要写入、或者返回一个非
    /// [`ErrorKind::Interrupted`] 种类的错误为止。这个方法在所有缓冲区都被成功写入、或发生上述
    /// 错误之前不会返回。这个方法产生的第一个非 [`ErrorKind::Interrupted`] 种类的错误将被返回。
    ///
    /// 如果缓冲区不含数据，这将永远不会调用 [`write_vectored`]。
    ///
    /// # 注意
    ///
    /// 与 [`write_vectored`] 不同，这个方法接受一个对 [`IoSlice`] 切片的*可变*引用，而非不可变
    /// 引用。这是因为我们需要修改该切片，以追踪已经写入的字节。
    ///
    /// 一旦这个函数返回，`bufs` 的内容是未指定的，因为这取决于需要多少次对 [`write_vectored`] 的
    /// 调用。最好把这个函数理解为获取了 `bufs` 的所有权，且此后不再使用 `bufs`。这些 [`IoSlice`]
    /// 所指向的底层缓冲区（但不是 [`IoSlice`] 本身）保持不变，可以复用。
    ///
    /// [`write_vectored`]: Write::write_vectored
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(write_all_vectored)]
    /// # fn main() -> std::io::Result<()> {
    ///
    /// use std::io::{Write, IoSlice};
    ///
    /// let mut writer = Vec::new();
    /// let bufs = &mut [
    ///     IoSlice::new(&[1]),
    ///     IoSlice::new(&[2, 3]),
    ///     IoSlice::new(&[4, 5, 6]),
    /// ];
    ///
    /// writer.write_all_vectored(bufs)?;
    /// // 注意：`bufs` 的内容现在是未定义的，参见「注意」一节。
    ///
    /// assert_eq!(writer, &[1, 2, 3, 4, 5, 6]);
    /// # Ok(()) }
    /// ```
    #[unstable(feature = "write_all_vectored", issue = "70436")]
    fn write_all_vectored(&mut self, mut bufs: &mut [IoSlice<'_>]) -> Result<()> {
        // 如果 bufs 不含数据，保证它为空，以避免在没有数据可写时调用 write_vectored。
        IoSlice::advance_slices(&mut bufs, 0);
        while !bufs.is_empty() {
            match self.write_vectored(bufs) {
                Ok(0) => {
                    return Err(Error::WRITE_ALL_EOF);
                }
                Ok(n) => IoSlice::advance_slices(&mut bufs, n),
                Err(ref e) if e.is_interrupted() => {}
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }

    /// 把一个格式化后的字符串写入这个 writer，返回遇到的任何错误。
    ///
    /// 这个方法主要用于与 [`format_args!()`] 宏对接，很少需要显式调用它。应优先使用 [`write!()`]
    /// 宏来调用这个方法。
    ///
    /// 这个函数在内部使用该 trait 上的 [`write_all`] 方法，因此只要不收到错误，它就会持续写入数据。
    /// 这也意味着部分写入（partial write）不会在这个签名中得到体现。
    ///
    /// [`write_all`]: Write::write_all
    ///
    /// # 错误(Errors）
    ///
    /// 这个函数将返回格式化过程中报告的任何 I/O 错误。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::io::prelude::*;
    /// use std::fs::File;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let mut buffer = File::create("foo.txt")?;
    ///
    ///     // 这一次调用
    ///     write!(buffer, "{:.*}", 2, 1.234567)?;
    ///     // 会变成这样：
    ///     buffer.write_fmt(format_args!("{:.*}", 2, 1.234567))?;
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    fn write_fmt(&mut self, args: fmt::Arguments<'_>) -> Result<()> {
        if let Some(s) = args.as_statically_known_str() {
            self.write_all(s.as_bytes())
        } else {
            default_write_fmt(self, args)
        }
    }

    /// 为这个 `Write` 实例创建一个「按引用」（by reference）适配器。
    ///
    /// 返回的适配器也实现了 `Write`，它只是简单地借用当前这个 writer。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::io::Write;
    /// use std::fs::File;
    ///
    /// fn main() -> std::io::Result<()> {
    ///     let mut buffer = File::create("foo.txt")?;
    ///
    ///     let reference = buffer.by_ref();
    ///
    ///     // 我们可以像使用原始的 buffer 一样使用 reference
    ///     reference.write_all(b"some bytes")?;
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    fn by_ref(&mut self) -> &mut Self
    where
        Self: Sized,
    {
        self
    }
}

/// `Seek` trait 提供了一个可以在字节流内移动的游标（cursor）。
///
/// 该流通常有固定的大小，从而允许相对于任一端或当前偏移量进行寻位（seek）。
///
/// # 示例
///
/// [`File`] 实现了 `Seek`：
///
/// [`File`]: crate::fs::File
///
/// ```no_run
/// use std::io;
/// use std::io::prelude::*;
/// use std::fs::File;
/// use std::io::SeekFrom;
///
/// fn main() -> io::Result<()> {
///     let mut f = File::open("foo.txt")?;
///
///     // 把游标移动到距文件起始 42 个字节处
///     f.seek(SeekFrom::Start(42))?;
///     Ok(())
/// }
/// ```
#[stable(feature = "rust1", since = "1.0.0")]
#[cfg_attr(not(test), rustc_diagnostic_item = "IoSeek")]
pub trait Seek {
    /// 寻位到流中以字节为单位的某个偏移量处。
    ///
    /// 允许寻位到超过流末尾的位置，但其行为由具体实现定义。
    ///
    /// 如果寻位操作成功完成，这个方法返回相对于流起始处的新位置。该位置之后可以与
    /// [`SeekFrom::Start`] 一起使用。
    ///
    /// # 错误(Errors）
    ///
    /// 寻位可能失败，例如因为它可能涉及刷新一个缓冲区。
    ///
    /// 寻位到一个负的偏移量被视为错误。
    #[stable(feature = "rust1", since = "1.0.0")]
    fn seek(&mut self, pos: SeekFrom) -> Result<u64>;

    /// 倒回（rewind）到流的开头。
    ///
    /// 这是一个便捷方法，等价于 `seek(SeekFrom::Start(0))`。
    ///
    /// # 错误(Errors）
    ///
    /// 倒回可能失败，例如因为它可能涉及刷新一个缓冲区。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::io::{Read, Seek, Write};
    /// use std::fs::OpenOptions;
    ///
    /// let mut f = OpenOptions::new()
    ///     .write(true)
    ///     .read(true)
    ///     .create(true)
    ///     .open("foo.txt")?;
    ///
    /// let hello = "Hello!\n";
    /// write!(f, "{hello}")?;
    /// f.rewind()?;
    ///
    /// let mut buf = String::new();
    /// f.read_to_string(&mut buf)?;
    /// assert_eq!(&buf, hello);
    /// # std::io::Result::Ok(())
    /// ```
    #[stable(feature = "seek_rewind", since = "1.55.0")]
    fn rewind(&mut self) -> Result<()> {
        self.seek(SeekFrom::Start(0))?;
        Ok(())
    }

    /// 返回这个流的长度（以字节为单位）。
    ///
    /// 默认实现最多使用三次寻位操作。如果这个方法成功返回，寻位位置保持不变（即调用本方法之前的
    /// 位置与之后相同）。然而，如果这个方法返回错误，寻位位置是未指定的。
    ///
    /// 如果你需要获取*许多*流的长度、并且不在意之后的寻位位置，你可以简单地调用
    /// `seek(SeekFrom::End(0))` 并使用其返回值（它也就是流的长度），从而减少寻位操作的次数。
    ///
    /// 注意，一个流的长度可以随时间变化（例如，当数据被追加到一个文件时）。所以多次调用这个方法
    /// 不一定每次都返回相同的长度。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// #![feature(seek_stream_len)]
    /// use std::{
    ///     io::{self, Seek},
    ///     fs::File,
    /// };
    ///
    /// fn main() -> io::Result<()> {
    ///     let mut f = File::open("foo.txt")?;
    ///
    ///     let len = f.stream_len()?;
    ///     println!("The file is currently {len} bytes long");
    ///     Ok(())
    /// }
    /// ```
    #[unstable(feature = "seek_stream_len", issue = "59359")]
    fn stream_len(&mut self) -> Result<u64> {
        stream_len_default(self)
    }

    /// 返回相对于流起始处的当前寻位位置。
    ///
    /// 这等价于 `self.seek(SeekFrom::Current(0))`。
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
    #[stable(feature = "seek_convenience", since = "1.51.0")]
    fn stream_position(&mut self) -> Result<u64> {
        self.seek(SeekFrom::Current(0))
    }

    /// 相对于当前位置进行寻位。
    ///
    /// 这等价于 `self.seek(SeekFrom::Current(offset))`，但它不返回新位置，这可以让某些实现
    ///（例如 [`BufReader`]）执行更高效的寻位。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::{
    ///     io::{self, Seek},
    ///     fs::File,
    /// };
    ///
    /// fn main() -> io::Result<()> {
    ///     let mut f = File::open("foo.txt")?;
    ///     f.seek_relative(10)?;
    ///     assert_eq!(f.stream_position()?, 10);
    ///     Ok(())
    /// }
    /// ```
    ///
    /// [`BufReader`]: crate::io::BufReader
    #[stable(feature = "seek_seek_relative", since = "1.80.0")]
    fn seek_relative(&mut self, offset: i64) -> Result<()> {
        self.seek(SeekFrom::Current(offset))?;
        Ok(())
    }
}

pub(crate) fn stream_len_default<T: Seek + ?Sized>(self_: &mut T) -> Result<u64> {
    let old_pos = self_.stream_position()?;
    let len = self_.seek(SeekFrom::End(0))?;

    // 当我们已经位于流末尾时，避免进行第三次寻位。这个分支通常比一次寻位操作便宜得多。
    if old_pos != len {
        self_.seek(SeekFrom::Start(old_pos))?;
    }

    Ok(len)
}

/// 在一个 I/O 对象内进行寻位的若干可能方式的枚举。
///
/// 它被 [`Seek`] trait 使用。
#[derive(Copy, PartialEq, Eq, Clone, Debug)]
#[stable(feature = "rust1", since = "1.0.0")]
#[cfg_attr(not(test), rustc_diagnostic_item = "SeekFrom")]
pub enum SeekFrom {
    /// 把偏移量设置为提供的字节数。
    #[stable(feature = "rust1", since = "1.0.0")]
    Start(#[stable(feature = "rust1", since = "1.0.0")] u64),

    /// 把偏移量设置为这个对象的大小加上指定的字节数。
    ///
    /// 寻位到超过对象末尾是可能的，但寻位到第 0 个字节之前是错误。
    #[stable(feature = "rust1", since = "1.0.0")]
    End(#[stable(feature = "rust1", since = "1.0.0")] i64),

    /// 把偏移量设置为当前位置加上指定的字节数。
    ///
    /// 寻位到超过对象末尾是可能的，但寻位到第 0 个字节之前是错误。
    #[stable(feature = "rust1", since = "1.0.0")]
    Current(#[stable(feature = "rust1", since = "1.0.0")] i64),
}

fn read_until<R: BufRead + ?Sized>(r: &mut R, delim: u8, buf: &mut Vec<u8>) -> Result<usize> {
    let mut read = 0;
    loop {
        let (done, used) = {
            let available = match r.fill_buf() {
                Ok(n) => n,
                Err(ref e) if e.is_interrupted() => continue,
                Err(e) => return Err(e),
            };
            match memchr::memchr(delim, available) {
                Some(i) => {
                    buf.extend_from_slice(&available[..=i]);
                    (true, i + 1)
                }
                None => {
                    buf.extend_from_slice(available);
                    (false, available.len())
                }
            }
        };
        r.consume(used);
        read += used;
        if done || used == 0 {
            return Ok(read);
        }
    }
}

fn skip_until<R: BufRead + ?Sized>(r: &mut R, delim: u8) -> Result<usize> {
    let mut read = 0;
    loop {
        let (done, used) = {
            let available = match r.fill_buf() {
                Ok(n) => n,
                Err(ref e) if e.kind() == ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            };
            match memchr::memchr(delim, available) {
                Some(i) => (true, i + 1),
                None => (false, available.len()),
            }
        };
        r.consume(used);
        read += used;
        if done || used == 0 {
            return Ok(read);
        }
    }
}

/// `BufRead` 是一种带内部缓冲区的 `Read`er，使它能够执行额外的读取方式。
///
/// 例如，不使用缓冲区时逐行读取是低效的，所以如果你想逐行读取，你会需要 `BufRead`，它包含一个
/// [`read_line`] 方法以及一个 [`lines`] 迭代器。
///
/// # 示例
///
/// 加锁后的标准输入实现了 `BufRead`：
///
/// ```no_run
/// use std::io;
/// use std::io::prelude::*;
///
/// let stdin = io::stdin();
/// for line in stdin.lock().lines() {
///     println!("{}", line?);
/// }
/// # std::io::Result::Ok(())
/// ```
///
/// 如果你有某个实现了 [`Read`] 的东西，你可以使用 [`BufReader` 类型][`BufReader`] 把它变成一个
/// `BufRead`。
///
/// 例如，[`File`] 实现了 [`Read`]，但没有实现 `BufRead`。这时 [`BufReader`] 来救场！
///
/// [`File`]: crate::fs::File
/// [`read_line`]: BufRead::read_line
/// [`lines`]: BufRead::lines
///
/// ```no_run
/// use std::io::{self, BufReader};
/// use std::io::prelude::*;
/// use std::fs::File;
///
/// fn main() -> io::Result<()> {
///     let f = File::open("foo.txt")?;
///     let f = BufReader::new(f);
///
///     for line in f.lines() {
///         let line = line?;
///         println!("{line}");
///     }
///
///     Ok(())
/// }
/// ```
#[stable(feature = "rust1", since = "1.0.0")]
#[cfg_attr(not(test), rustc_diagnostic_item = "IoBufRead")]
pub trait BufRead: Read {
    /// 返回内部缓冲区的内容；如果它为空，则通过 `Read` 方法用更多数据填充它。
    ///
    /// 这是一个较底层的方法，旨在与 [`consume`] 一起使用——后者可用于标记那些不应被后续 `read`
    /// 调用返回的字节。
    ///
    /// [`consume`]: BufRead::consume
    ///
    /// 当流到达 EOF 时返回一个空缓冲区。
    ///
    /// # 错误(Errors）
    ///
    /// 如果调用了某个 `Read` 方法但它返回了错误，那么这个函数将返回一个 I/O 错误。
    ///
    /// # 示例
    ///
    /// 加锁后的标准输入实现了 `BufRead`：
    ///
    /// ```no_run
    /// use std::io;
    /// use std::io::prelude::*;
    ///
    /// let stdin = io::stdin();
    /// let mut stdin = stdin.lock();
    ///
    /// let buffer = stdin.fill_buf()?;
    ///
    /// // 处理 buffer
    /// println!("{buffer:?}");
    ///
    /// // 把我们处理过的字节标记为已读
    /// let length = buffer.len();
    /// stdin.consume(length);
    /// # std::io::Result::Ok(())
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    fn fill_buf(&mut self) -> Result<&[u8]>;

    /// 把内部缓冲区中给定 `amount` 数量的额外字节标记为已被读取。
    /// 后续对 `read` 的调用只会返回那些尚未被标记为已读的字节。
    ///
    /// 这是一个较底层的方法，旨在与 [`fill_buf`] 一起使用——后者可用于通过 `Read` 方法填充内部
    /// 缓冲区。
    ///
    /// 如果 `amount` 超过了内部缓冲区中未读字节的数量（该数量由 [`fill_buf`] 返回），那是一个
    /// 逻辑错误。
    ///
    /// # 示例
    ///
    /// 由于 `consume()` 旨在与 [`fill_buf`] 一起使用，该方法的示例中已包含一个 `consume()` 的
    /// 示例。
    ///
    /// [`fill_buf`]: BufRead::fill_buf
    #[stable(feature = "rust1", since = "1.0.0")]
    fn consume(&mut self, amount: usize);

    /// 检查是否还有任何数据可供 `read`。
    ///
    /// 这个函数可能会填充缓冲区以检查数据，所以这个函数返回 `Result<bool>`，而非 `bool`。
    ///
    /// 默认实现调用 `fill_buf` 并检查返回的切片是否为空（这意味着已没有数据剩余，因为已到达 EOF）。
    ///
    /// # 错误(Errors）
    ///
    /// 如果调用了某个 `Read` 方法但它返回了错误，那么这个函数将返回一个 I/O 错误。
    ///
    /// 示例
    ///
    /// ```
    /// #![feature(buf_read_has_data_left)]
    /// use std::io;
    /// use std::io::prelude::*;
    ///
    /// let stdin = io::stdin();
    /// let mut stdin = stdin.lock();
    ///
    /// while stdin.has_data_left()? {
    ///     let mut line = String::new();
    ///     stdin.read_line(&mut line)?;
    ///     // 处理 line
    ///     println!("{line:?}");
    /// }
    /// # std::io::Result::Ok(())
    /// ```
    #[unstable(feature = "buf_read_has_data_left", issue = "86423")]
    fn has_data_left(&mut self) -> Result<bool> {
        self.fill_buf().map(|b| !b.is_empty())
    }

    /// 把所有字节读入 `buf`，直到到达分隔符 `byte` 或 EOF。
    ///
    /// 这个函数会从底层流读取字节，直到找到分隔符或 EOF。一旦找到，所有截止到（并包括）该分隔符
    ///（如果找到的话）的字节都会被追加到 `buf`。
    ///
    /// 如果成功，这个函数将返回读取到的字节总数。
    ///
    /// 这个函数是阻塞的，应当谨慎使用：攻击者有可能持续发送字节而永不发送分隔符或 EOF。
    ///
    /// # 错误(Errors）
    ///
    /// 这个函数将忽略所有 [`ErrorKind::Interrupted`] 的情况，否则将返回 [`fill_buf`] 返回的任何
    /// 错误。
    ///
    /// 如果遇到一个 I/O 错误，那么到目前为止读取到的所有字节都会出现在 `buf` 中，且其长度也已被
    /// 相应地调整。
    ///
    /// [`fill_buf`]: BufRead::fill_buf
    ///
    /// # 示例
    ///
    /// [`std::io::Cursor`][`Cursor`] 是一个实现了 `BufRead` 的类型。在这个例子中，我们使用
    /// [`Cursor`] 以连字符分隔的片段来读取一个字节切片中的所有字节：
    ///
    /// ```
    /// use std::io::{self, BufRead};
    ///
    /// let mut cursor = io::Cursor::new(b"lorem-ipsum");
    /// let mut buf = vec![];
    ///
    /// // cursor 位于 'l'
    /// let num_bytes = cursor.read_until(b'-', &mut buf)
    ///     .expect("reading from cursor won't fail");
    /// assert_eq!(num_bytes, 6);
    /// assert_eq!(buf, b"lorem-");
    /// buf.clear();
    ///
    /// // cursor 位于 'i'
    /// let num_bytes = cursor.read_until(b'-', &mut buf)
    ///     .expect("reading from cursor won't fail");
    /// assert_eq!(num_bytes, 5);
    /// assert_eq!(buf, b"ipsum");
    /// buf.clear();
    ///
    /// // cursor 位于 EOF
    /// let num_bytes = cursor.read_until(b'-', &mut buf)
    ///     .expect("reading from cursor won't fail");
    /// assert_eq!(num_bytes, 0);
    /// assert_eq!(buf, b"");
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    fn read_until(&mut self, byte: u8, buf: &mut Vec<u8>) -> Result<usize> {
        read_until(self, byte, buf)
    }

    /// 跳过所有字节，直到到达分隔符 `byte` 或 EOF。
    ///
    /// 这个函数会从底层流读取（并丢弃）字节，直到找到分隔符或 EOF。
    ///
    /// 如果成功，这个函数将返回读取到的字节总数，包括分隔符字节（如果找到的话）。
    ///
    /// 这对于高效地跳过数据很有用，例如在二进制文件格式中跳过 NUL 结尾的字符串而无需缓冲。
    ///
    /// 这个函数是阻塞的，应当谨慎使用：攻击者有可能持续发送字节而永不发送分隔符或 EOF。
    ///
    /// # 错误(Errors）
    ///
    /// 这个函数会忽略所有 [`ErrorKind::Interrupted`] 的情况，
    /// 除此之外则返回 [`fill_buf`] 所返回的任何错误。
    ///
    /// 如果遇到 I/O 错误，那么到目前为止已读取的所有字节都会
    /// 留在 `buf` 中，并且其长度会被相应地调整。
    ///
    /// [`fill_buf`]: BufRead::fill_buf
    ///
    /// # 示例
    ///
    /// [`std::io::Cursor`][`Cursor`] 是一个实现了 `BufRead` 的类型。在这个例子中，我们使用
    /// [`Cursor`] 从一个二进制字符串读取一些关于 Ferris 的 NUL 结尾信息，并跳过那条趣闻：
    ///
    /// ```
    /// use std::io::{self, BufRead};
    ///
    /// let mut cursor = io::Cursor::new(b"Ferris\0Likes long walks on the beach\0Crustacean\0!");
    ///
    /// // 读取名字
    /// let mut name = Vec::new();
    /// let num_bytes = cursor.read_until(b'\0', &mut name)
    ///     .expect("reading from cursor won't fail");
    /// assert_eq!(num_bytes, 7);
    /// assert_eq!(name, b"Ferris\0");
    ///
    /// // 跳过趣闻
    /// let num_bytes = cursor.skip_until(b'\0')
    ///     .expect("reading from cursor won't fail");
    /// assert_eq!(num_bytes, 30);
    ///
    /// // 读取动物类型
    /// let mut animal = Vec::new();
    /// let num_bytes = cursor.read_until(b'\0', &mut animal)
    ///     .expect("reading from cursor won't fail");
    /// assert_eq!(num_bytes, 11);
    /// assert_eq!(animal, b"Crustacean\0");
    ///
    /// // 到达 EOF
    /// let num_bytes = cursor.skip_until(b'\0')
    ///     .expect("reading from cursor won't fail");
    /// assert_eq!(num_bytes, 1);
    /// ```
    #[stable(feature = "bufread_skip_until", since = "1.83.0")]
    fn skip_until(&mut self, byte: u8) -> Result<usize> {
        skip_until(self, byte)
    }

    /// 读取所有字节直到到达一个换行符（`0xA` 字节），并把它们追加到提供的 `String` 缓冲区。
    ///
    /// 缓冲区先前的内容将被保留。要避免追加到缓冲区，你需要先 [`clear`] 它。
    ///
    /// 这个函数会从底层流读取字节，直到找到换行分隔符（`0xA` 字节）或 EOF。一旦找到，所有截止到
    ///（并包括）该分隔符（如果找到的话）的字节都会被追加到 `buf`。
    ///
    /// 如果成功，这个函数将返回读取到的字节总数。
    ///
    /// 如果这个函数返回 [`Ok(0)`]，则该流已到达 EOF。
    ///
    /// 这个函数是阻塞的，应当谨慎使用：攻击者有可能持续发送字节而永不发送换行符或 EOF。你可以
    /// 使用 [`take`] 来限制读取的最大字节数。
    ///
    /// [`Ok(0)`]: Ok
    /// [`clear`]: String::clear
    /// [`take`]: crate::io::Read::take
    ///
    /// # 错误(Errors）
    ///
    /// 这个函数与 [`read_until`] 有相同的错误语义，并且如果读取到的字节不是有效的 UTF-8，它也会
    /// 返回一个错误。如果遇到一个 I/O 错误，那么在「到目前为止读取的所有数据都是有效 UTF-8」的
    /// 情况下，`buf` 可能包含一些已经读取的字节。
    ///
    /// [`read_until`]: BufRead::read_until
    ///
    /// # 示例
    ///
    /// [`std::io::Cursor`][`Cursor`] 是一个实现了 `BufRead` 的类型。在这个例子中，我们使用
    /// [`Cursor`] 读取一个字节切片中的所有行：
    ///
    /// ```
    /// use std::io::{self, BufRead};
    ///
    /// let mut cursor = io::Cursor::new(b"foo\nbar");
    /// let mut buf = String::new();
    ///
    /// // cursor 位于 'f'
    /// let num_bytes = cursor.read_line(&mut buf)
    ///     .expect("reading from cursor won't fail");
    /// assert_eq!(num_bytes, 4);
    /// assert_eq!(buf, "foo\n");
    /// buf.clear();
    ///
    /// // 此时游标位于 'b' 处
    /// let num_bytes = cursor.read_line(&mut buf)
    ///     .expect("reading from cursor won't fail");
    /// assert_eq!(num_bytes, 3);
    /// assert_eq!(buf, "bar");
    /// buf.clear();
    ///
    /// // cursor 位于 EOF
    /// let num_bytes = cursor.read_line(&mut buf)
    ///     .expect("reading from cursor won't fail");
    /// assert_eq!(num_bytes, 0);
    /// assert_eq!(buf, "");
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    fn read_line(&mut self, buf: &mut String) -> Result<usize> {
        // 注意，这里我们没有调用 `.read_until` 方法，而是用了我们硬编码的实现。至于为什么，
        // 更多细节请参见 `default_read_to_string` 中的注释。
        unsafe { append_to_string(buf, |b| read_until(self, b'\n', b)) }
    }

    /// 返回一个迭代器，遍历这个 reader 中以字节 `byte` 分割后的内容。
    ///
    /// 从这个函数返回的迭代器将返回 <code>[io::Result]<[Vec]\<u8>></code> 的实例。返回的每个向量
    /// 末尾*不*会带有分隔符字节。
    ///
    /// 每当 [`read_until`] 本来会产出错误时，这个函数也会产出错误。
    ///
    /// [io::Result]: self::Result "io::Result"
    /// [`read_until`]: BufRead::read_until
    ///
    /// # 示例
    ///
    /// [`std::io::Cursor`][`Cursor`] 是一个实现了 `BufRead` 的类型。在这个例子中，我们使用
    /// [`Cursor`] 遍历一个字节切片中所有以连字符分隔的片段
    ///
    /// ```
    /// use std::io::{self, BufRead};
    ///
    /// let cursor = io::Cursor::new(b"lorem-ipsum-dolor");
    ///
    /// let mut split_iter = cursor.split(b'-').map(|l| l.unwrap());
    /// assert_eq!(split_iter.next(), Some(b"lorem".to_vec()));
    /// assert_eq!(split_iter.next(), Some(b"ipsum".to_vec()));
    /// assert_eq!(split_iter.next(), Some(b"dolor".to_vec()));
    /// assert_eq!(split_iter.next(), None);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    fn split(self, byte: u8) -> Split<Self>
    where
        Self: Sized,
    {
        Split { buf: self, delim: byte }
    }

    /// 返回一个迭代器，遍历这个 reader 的各行。
    ///
    /// 从这个函数返回的迭代器将产出 <code>[io::Result]<[String]></code> 的实例。返回的每个字符串
    /// 末尾*不*会带有换行字节（`0xA` 字节）或 `CRLF`（`0xD`、`0xA` 字节）。
    ///
    /// [io::Result]: self::Result "io::Result"
    ///
    /// # 示例
    ///
    /// [`std::io::Cursor`][`Cursor`] 是一个实现了 `BufRead` 的类型。在这个例子中，我们使用
    /// [`Cursor`] 遍历一个字节切片中的所有行。
    ///
    /// ```
    /// use std::io::{self, BufRead};
    ///
    /// let cursor = io::Cursor::new(b"lorem\nipsum\r\ndolor");
    ///
    /// let mut lines_iter = cursor.lines().map(|l| l.unwrap());
    /// assert_eq!(lines_iter.next(), Some(String::from("lorem")));
    /// assert_eq!(lines_iter.next(), Some(String::from("ipsum")));
    /// assert_eq!(lines_iter.next(), Some(String::from("dolor")));
    /// assert_eq!(lines_iter.next(), None);
    /// ```
    ///
    /// # 错误(Errors）
    ///
    /// 该迭代器的每一行都与 [`BufRead::read_line`] 有相同的错误语义。
    #[stable(feature = "rust1", since = "1.0.0")]
    fn lines(self) -> Lines<Self>
    where
        Self: Sized,
    {
        Lines { buf: self }
    }
}

/// 用于把两个 reader 链接（chain）在一起的适配器。
///
/// 这个结构体通常通过在一个 reader 上调用 [`chain`] 来创建。更多细节请参见 [`chain`] 的文档。
///
/// [`chain`]: Read::chain
#[stable(feature = "rust1", since = "1.0.0")]
#[derive(Debug)]
pub struct Chain<T, U> {
    first: T,
    second: U,
    done_first: bool,
}

impl<T, U> Chain<T, U> {
    /// 消耗这个 `Chain`，返回被包装的那两个 reader。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::io;
    /// use std::io::prelude::*;
    /// use std::fs::File;
    ///
    /// fn main() -> io::Result<()> {
    ///     let mut foo_file = File::open("foo.txt")?;
    ///     let mut bar_file = File::open("bar.txt")?;
    ///
    ///     let chain = foo_file.chain(bar_file);
    ///     let (foo_file, bar_file) = chain.into_inner();
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "more_io_inner_methods", since = "1.20.0")]
    pub fn into_inner(self) -> (T, U) {
        (self.first, self.second)
    }

    /// 获取这个 `Chain` 中底层那两个 reader 的引用。
    ///
    /// 应当注意避免修改底层 reader 的内部 I/O 状态，因为这样做可能会破坏这个 `Chain` 的内部状态。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::io;
    /// use std::io::prelude::*;
    /// use std::fs::File;
    ///
    /// fn main() -> io::Result<()> {
    ///     let mut foo_file = File::open("foo.txt")?;
    ///     let mut bar_file = File::open("bar.txt")?;
    ///
    ///     let chain = foo_file.chain(bar_file);
    ///     let (foo_file, bar_file) = chain.get_ref();
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "more_io_inner_methods", since = "1.20.0")]
    pub fn get_ref(&self) -> (&T, &U) {
        (&self.first, &self.second)
    }

    /// 获取这个 `Chain` 中底层那两个 reader 的可变引用。
    ///
    /// 应当注意避免修改底层 reader 的内部 I/O 状态，因为这样做可能会破坏这个 `Chain` 的内部状态。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::io;
    /// use std::io::prelude::*;
    /// use std::fs::File;
    ///
    /// fn main() -> io::Result<()> {
    ///     let mut foo_file = File::open("foo.txt")?;
    ///     let mut bar_file = File::open("bar.txt")?;
    ///
    ///     let mut chain = foo_file.chain(bar_file);
    ///     let (foo_file, bar_file) = chain.get_mut();
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "more_io_inner_methods", since = "1.20.0")]
    pub fn get_mut(&mut self) -> (&mut T, &mut U) {
        (&mut self.first, &mut self.second)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T: Read, U: Read> Read for Chain<T, U> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        if !self.done_first {
            match self.first.read(buf)? {
                0 if !buf.is_empty() => self.done_first = true,
                n => return Ok(n),
            }
        }
        self.second.read(buf)
    }

    fn read_vectored(&mut self, bufs: &mut [IoSliceMut<'_>]) -> Result<usize> {
        if !self.done_first {
            match self.first.read_vectored(bufs)? {
                0 if bufs.iter().any(|b| !b.is_empty()) => self.done_first = true,
                n => return Ok(n),
            }
        }
        self.second.read_vectored(bufs)
    }

    #[inline]
    fn is_read_vectored(&self) -> bool {
        self.first.is_read_vectored() || self.second.is_read_vectored()
    }

    fn read_to_end(&mut self, buf: &mut Vec<u8>) -> Result<usize> {
        let mut read = 0;
        if !self.done_first {
            read += self.first.read_to_end(buf)?;
            self.done_first = true;
        }
        read += self.second.read_to_end(buf)?;
        Ok(read)
    }

    // 这里我们不重写 `read_to_string`，因为一个 UTF-8 序列可能被分割在链的两个部分之间

    fn read_buf(&mut self, mut buf: BorrowedCursor<'_>) -> Result<()> {
        if buf.capacity() == 0 {
            return Ok(());
        }

        if !self.done_first {
            let old_len = buf.written();
            self.first.read_buf(buf.reborrow())?;

            if buf.written() != old_len {
                return Ok(());
            } else {
                self.done_first = true;
            }
        }
        self.second.read_buf(buf)
    }
}

#[stable(feature = "chain_bufread", since = "1.9.0")]
impl<T: BufRead, U: BufRead> BufRead for Chain<T, U> {
    fn fill_buf(&mut self) -> Result<&[u8]> {
        if !self.done_first {
            match self.first.fill_buf()? {
                buf if buf.is_empty() => self.done_first = true,
                buf => return Ok(buf),
            }
        }
        self.second.fill_buf()
    }

    fn consume(&mut self, amt: usize) {
        if !self.done_first { self.first.consume(amt) } else { self.second.consume(amt) }
    }

    fn read_until(&mut self, byte: u8, buf: &mut Vec<u8>) -> Result<usize> {
        let mut read = 0;
        if !self.done_first {
            let n = self.first.read_until(byte, buf)?;
            read += n;

            match buf.last() {
                Some(b) if *b == byte && n != 0 => return Ok(read),
                _ => self.done_first = true,
            }
        }
        read += self.second.read_until(byte, buf)?;
        Ok(read)
    }

    // 这里我们不重写 `read_line`，因为一个 UTF-8 序列可能被分割在链的两个部分之间
}

impl<T, U> SizeHint for Chain<T, U> {
    #[inline]
    fn lower_bound(&self) -> usize {
        SizeHint::lower_bound(&self.first) + SizeHint::lower_bound(&self.second)
    }

    #[inline]
    fn upper_bound(&self) -> Option<usize> {
        match (SizeHint::upper_bound(&self.first), SizeHint::upper_bound(&self.second)) {
            (Some(first), Some(second)) => first.checked_add(second),
            _ => None,
        }
    }
}

/// reader 适配器，它限制从底层 reader 读取的字节数。
///
/// 这个结构体通常通过在一个 reader 上调用 [`take`] 来创建。更多细节请参见 [`take`] 的文档。
///
/// [`take`]: Read::take
#[stable(feature = "rust1", since = "1.0.0")]
#[derive(Debug)]
pub struct Take<T> {
    inner: T,
    len: u64,
    limit: u64,
}

impl<T> Take<T> {
    /// 返回在这个实例返回 EOF 之前还能读取的字节数。
    ///
    /// # 注意
    ///
    /// 如果底层 [`Read`] 实例到达 EOF，那么这个实例可能在读取的字节数少于本方法所指示的数量时
    /// 就到达 `EOF`。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::io;
    /// use std::io::prelude::*;
    /// use std::fs::File;
    ///
    /// fn main() -> io::Result<()> {
    ///     let f = File::open("foo.txt")?;
    ///
    ///     // 最多读取五个字节
    ///     let handle = f.take(5);
    ///
    ///     println!("limit: {}", handle.limit());
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn limit(&self) -> u64 {
        self.limit
    }

    /// 返回到目前为止读取的字节数。
    #[unstable(feature = "seek_io_take_position", issue = "97227")]
    pub fn position(&self) -> u64 {
        self.len - self.limit
    }

    /// 设置在这个实例返回 EOF 之前还能读取的字节数。这与构造一个新的 `Take` 实例相同，所以调用
    /// 这个方法时，已读取的字节数和先前的 limit 值都无关紧要。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::io;
    /// use std::io::prelude::*;
    /// use std::fs::File;
    ///
    /// fn main() -> io::Result<()> {
    ///     let f = File::open("foo.txt")?;
    ///
    ///     // 最多读取五个字节
    ///     let mut handle = f.take(5);
    ///     handle.set_limit(10);
    ///
    ///     assert_eq!(handle.limit(), 10);
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "take_set_limit", since = "1.27.0")]
    pub fn set_limit(&mut self, limit: u64) {
        self.len = limit;
        self.limit = limit;
    }

    /// 消耗这个 `Take`，返回被包装的 reader。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::io;
    /// use std::io::prelude::*;
    /// use std::fs::File;
    ///
    /// fn main() -> io::Result<()> {
    ///     let mut file = File::open("foo.txt")?;
    ///
    ///     let mut buffer = [0; 5];
    ///     let mut handle = file.take(5);
    ///     handle.read(&mut buffer)?;
    ///
    ///     let file = handle.into_inner();
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "io_take_into_inner", since = "1.15.0")]
    pub fn into_inner(self) -> T {
        self.inner
    }

    /// 获取底层 reader 的一个引用。
    ///
    /// 应当注意避免修改底层 reader 的内部 I/O 状态，因为这样做可能会破坏这个 `Take` 的内部 limit。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::io;
    /// use std::io::prelude::*;
    /// use std::fs::File;
    ///
    /// fn main() -> io::Result<()> {
    ///     let mut file = File::open("foo.txt")?;
    ///
    ///     let mut buffer = [0; 5];
    ///     let mut handle = file.take(5);
    ///     handle.read(&mut buffer)?;
    ///
    ///     let file = handle.get_ref();
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "more_io_inner_methods", since = "1.20.0")]
    pub fn get_ref(&self) -> &T {
        &self.inner
    }

    /// 获取底层 reader 的一个可变引用。
    ///
    /// 应当注意避免修改底层 reader 的内部 I/O 状态，因为这样做可能会破坏这个 `Take` 的内部 limit。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::io;
    /// use std::io::prelude::*;
    /// use std::fs::File;
    ///
    /// fn main() -> io::Result<()> {
    ///     let mut file = File::open("foo.txt")?;
    ///
    ///     let mut buffer = [0; 5];
    ///     let mut handle = file.take(5);
    ///     handle.read(&mut buffer)?;
    ///
    ///     let file = handle.get_mut();
    ///     Ok(())
    /// }
    /// ```
    #[stable(feature = "more_io_inner_methods", since = "1.20.0")]
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.inner
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T: Read> Read for Take<T> {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize> {
        // 在 EOF 处完全不要调用内层 reader，因为它可能仍会阻塞
        if self.limit == 0 {
            return Ok(0);
        }

        let max = cmp::min(buf.len() as u64, self.limit) as usize;
        let n = self.inner.read(&mut buf[..max])?;
        assert!(n as u64 <= self.limit, "number of read bytes exceeds limit");
        self.limit -= n as u64;
        Ok(n)
    }

    fn read_buf(&mut self, mut buf: BorrowedCursor<'_>) -> Result<()> {
        // 在 EOF 时完全不调用内层 reader，因为它可能仍会阻塞
        if self.limit == 0 {
            return Ok(());
        }

        if self.limit < buf.capacity() as u64 {
            // 上面的条件保证了 `self.limit` 能放进 `usize`。
            let limit = self.limit as usize;

            let extra_init = cmp::min(limit, buf.init_mut().len());

            // 安全性：没有未初始化的数据被写入 ibuf
            let ibuf = unsafe { &mut buf.as_mut()[..limit] };

            let mut sliced_buf: BorrowedBuf<'_> = ibuf.into();

            // 安全性：已知 ibuf 的 extra_init 个字节是已初始化的
            unsafe {
                sliced_buf.set_init(extra_init);
            }

            let mut cursor = sliced_buf.unfilled();
            let result = self.inner.read_buf(cursor.reborrow());

            let new_init = cursor.init_mut().len();
            let filled = sliced_buf.len();

            // cursor / sliced_buf / ibuf 必须在此处析构

            unsafe {
                // 安全性：filled 个字节已被填充，因此也已初始化
                buf.advance_unchecked(filled);
                // 安全性：buf 未填充缓冲区的 new_init 个字节已被初始化
                buf.set_init(new_init);
            }

            self.limit -= filled as u64;

            result
        } else {
            let written = buf.written();
            let result = self.inner.read_buf(buf.reborrow());
            self.limit -= (buf.written() - written) as u64;
            result
        }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T: BufRead> BufRead for Take<T> {
    fn fill_buf(&mut self) -> Result<&[u8]> {
        // 在 EOF 时完全不调用内层 reader，因为它可能仍会阻塞
        if self.limit == 0 {
            return Ok(&[]);
        }

        let buf = self.inner.fill_buf()?;
        let cap = cmp::min(buf.len() as u64, self.limit) as usize;
        Ok(&buf[..cap])
    }

    fn consume(&mut self, amt: usize) {
        // 不允许调用方通过传入一个过大的值来重置 limit
        let amt = cmp::min(amt as u64, self.limit) as usize;
        self.limit -= amt as u64;
        self.inner.consume(amt);
    }
}

impl<T> SizeHint for Take<T> {
    #[inline]
    fn lower_bound(&self) -> usize {
        cmp::min(SizeHint::lower_bound(&self.inner) as u64, self.limit) as usize
    }

    #[inline]
    fn upper_bound(&self) -> Option<usize> {
        match SizeHint::upper_bound(&self.inner) {
            Some(upper_bound) => Some(cmp::min(upper_bound as u64, self.limit) as usize),
            None => self.limit.try_into().ok(),
        }
    }
}

#[stable(feature = "seek_io_take", since = "1.89.0")]
impl<T: Seek> Seek for Take<T> {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64> {
        let new_position = match pos {
            SeekFrom::Start(v) => Some(v),
            SeekFrom::Current(v) => self.position().checked_add_signed(v),
            SeekFrom::End(v) => self.len.checked_add_signed(v),
        };
        let new_position = match new_position {
            Some(v) if v <= self.len => v,
            _ => return Err(ErrorKind::InvalidInput.into()),
        };
        while new_position != self.position() {
            if let Some(offset) = new_position.checked_signed_diff(self.position()) {
                self.inner.seek_relative(offset)?;
                self.limit = self.limit.wrapping_sub(offset as u64);
                break;
            }
            let offset = if new_position > self.position() { i64::MAX } else { i64::MIN };
            self.inner.seek_relative(offset)?;
            self.limit = self.limit.wrapping_sub(offset as u64);
        }
        Ok(new_position)
    }

    fn stream_len(&mut self) -> Result<u64> {
        Ok(self.len)
    }

    fn stream_position(&mut self) -> Result<u64> {
        Ok(self.position())
    }

    fn seek_relative(&mut self, offset: i64) -> Result<()> {
        if !self.position().checked_add_signed(offset).is_some_and(|p| p <= self.len) {
            return Err(ErrorKind::InvalidInput.into());
        }
        self.inner.seek_relative(offset)?;
        self.limit = self.limit.wrapping_sub(offset as u64);
        Ok(())
    }
}

/// 遍历一个 reader 的 `u8` 值的迭代器。
///
/// 这个结构体通常通过在一个 reader 上调用 [`bytes`] 来创建。更多细节请参见 [`bytes`] 的文档。
///
/// [`bytes`]: Read::bytes
#[stable(feature = "rust1", since = "1.0.0")]
#[derive(Debug)]
pub struct Bytes<R> {
    inner: R,
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<R: Read> Iterator for Bytes<R> {
    type Item = Result<u8>;

    // 没有标 `#[inline]`。即便没有它，这个函数也会被内联，但显式标注它会导致编译时间显著退化。e inline annotation can result in worse code generation. See #116785.
    fn next(&mut self) -> Option<Result<u8>> {
        SpecReadByte::spec_read_byte(&mut self.inner)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        SizeHint::size_hint(&self.inner)
    }
}

/// 用于 `Bytes::next` 的特化（specialization）。
trait SpecReadByte {
    fn spec_read_byte(&mut self) -> Option<Result<u8>>;
}

impl<R> SpecReadByte for R
where
    Self: Read,
{
    #[inline]
    default fn spec_read_byte(&mut self) -> Option<Result<u8>> {
        inlined_slow_read_byte(self)
    }
}

/// 以一种慢速、通用的方式读取单个字节。这被默认的 `spec_read_byte` 使用。
#[inline]
fn inlined_slow_read_byte<R: Read>(reader: &mut R) -> Option<Result<u8>> {
    let mut byte = 0;
    loop {
        return match reader.read(slice::from_mut(&mut byte)) {
            Ok(0) => None,
            Ok(..) => Some(Ok(byte)),
            Err(ref e) if e.is_interrupted() => continue,
            Err(e) => Some(Err(e)),
        };
    }
}

// 被 `BufReader::spec_read_byte` 使用，对它而言 `inline(never)` 很重要。
#[inline(never)]
fn uninlined_slow_read_byte<R: Read>(reader: &mut R) -> Option<Result<u8>> {
    inlined_slow_read_byte(reader)
}

trait SizeHint {
    fn lower_bound(&self) -> usize;

    fn upper_bound(&self) -> Option<usize>;

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.lower_bound(), self.upper_bound())
    }
}

impl<T: ?Sized> SizeHint for T {
    #[inline]
    default fn lower_bound(&self) -> usize {
        0
    }

    #[inline]
    default fn upper_bound(&self) -> Option<usize> {
        None
    }
}

impl<T> SizeHint for &mut T {
    #[inline]
    fn lower_bound(&self) -> usize {
        SizeHint::lower_bound(*self)
    }

    #[inline]
    fn upper_bound(&self) -> Option<usize> {
        SizeHint::upper_bound(*self)
    }
}

impl<T> SizeHint for Box<T> {
    #[inline]
    fn lower_bound(&self) -> usize {
        SizeHint::lower_bound(&**self)
    }

    #[inline]
    fn upper_bound(&self) -> Option<usize> {
        SizeHint::upper_bound(&**self)
    }
}

impl SizeHint for &[u8] {
    #[inline]
    fn lower_bound(&self) -> usize {
        self.len()
    }

    #[inline]
    fn upper_bound(&self) -> Option<usize> {
        Some(self.len())
    }
}

/// 遍历某个 `BufRead` 实例内容、以特定字节分割的迭代器。
///
/// 这个结构体通常通过在一个 `BufRead` 上调用 [`split`] 来创建。更多细节请参见 [`split`] 的文档。
///
/// [`split`]: BufRead::split
#[stable(feature = "rust1", since = "1.0.0")]
#[derive(Debug)]
pub struct Split<B> {
    buf: B,
    delim: u8,
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<B: BufRead> Iterator for Split<B> {
    type Item = Result<Vec<u8>>;

    fn next(&mut self) -> Option<Result<Vec<u8>>> {
        let mut buf = Vec::new();
        match self.buf.read_until(self.delim, &mut buf) {
            Ok(0) => None,
            Ok(_n) => {
                if buf[buf.len() - 1] == self.delim {
                    buf.pop();
                }
                Some(Ok(buf))
            }
            Err(e) => Some(Err(e)),
        }
    }
}

/// 遍历某个 `BufRead` 实例各行的迭代器。
///
/// 这个结构体通常通过在 `BufRead` 上调用 [`lines`] 来创建。
/// 更多细节请参阅 [`lines`] 的文档。
///
/// [`lines`]: BufRead::lines
#[stable(feature = "rust1", since = "1.0.0")]
#[derive(Debug)]
#[cfg_attr(not(test), rustc_diagnostic_item = "IoLines")]
pub struct Lines<B> {
    buf: B,
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<B: BufRead> Iterator for Lines<B> {
    type Item = Result<String>;

    fn next(&mut self) -> Option<Result<String>> {
        let mut buf = String::new();
        match self.buf.read_line(&mut buf) {
            Ok(0) => None,
            Ok(_n) => {
                if buf.ends_with('\n') {
                    buf.pop();
                    if buf.ends_with('\r') {
                        buf.pop();
                    }
                }
                Some(Ok(buf))
            }
            Err(e) => Some(Err(e)),
        }
    }
}
