//! 本模块包含一些特化(specialization）实现，能够把作用于「持有文件描述符的类型」
//!（`File`、`TcpStream` 等）上的 `io::copy()` 操作卸载(offload）到比 `read(2)` 和
//! `write(2)` 更高效的系统调用上。
//!
//! 特化只应用于完全由 std 拥有的类型，这样用户代码就无法观察到 `Read` 和 `Write`
//! trait 没有被使用。
//!
//! 由于一次复制操作涉及一个读取端和一个写入端，而每一端都可能由不同的类型构成，
//! 还可能涉及泛型包装器(wrapper）（例如 `Take`、`BufReader`），因此对所有可能的
//! 组合去特化某个单一方法是不切实际的。
//!
//! 取而代之，读取器和写入器分别由 `CopyRead` 和 `CopyWrite` 特化 trait 来处理，
//! 然后再由 `Copier::copy` 方法进行特化。
//!
//! `Copier` 利用这些特化 trait 来解包底层的文件描述符，以及包装类型所施加的额外
//! 前提条件和约束。
//!
//! 一旦它获取了所有必要的部件，并把所有包装类型带入到一个可以被安全绕过的状态，
//! 它就会尝试使用 `copy_file_range(2)`、`sendfile(2)` 或 `splice(2)` 系统调用，
//! 在文件描述符之间直接搬运数据。
//! 由于这些系统调用有一些无法提前完全检查的要求，它会（在提示的引导下）一个接一个
//! 地尝试使用它们，以弄清楚哪一个可行，如果都不可行就回退到通用的读写复制循环。
//! 一旦为某一对文件描述符找到了一个可行的系统调用，它就会在循环中反复调用该系统调用
//! 直到复制操作完成。
//!
//! 使用这些系统调用的优点：
//!
//! * 更少的上下文切换，因为读和写被合并到了单个系统调用中，并且每次系统调用传输
//!   更多的字节。这会带来更高的吞吐量和更少的 CPU 周期，至少对于足够大的传输而言
//!   是如此，足以摊销最初的探测开销。
//! * `copy_file_range` 会在写时复制(CoW）文件系统上创建 reflink 复制，从而搬运
//!   更少的数据并占用更少的磁盘空间
//! * `sendfile` 和 `splice` 在某些情况下能够执行零拷贝(zero-copy）IO，而朴素的
//!   复制循环则会让每一个字节都经过 CPU。
//!
//! 缺点：
//!
//! * 小于默认缓冲区大小的复制操作，在某些情况下（尤其是在较旧的内核上）可能比朴素
//!   做法引发更多的系统调用。如上所述，系统调用的选择由提示来引导以尽量减小这种
//!   可能性，但它们并不完美。
//! * 这些优化只对 std 类型适用。如果用户添加了一个自定义的包装类型（例如用来报告
//!   进度），他们可能会遇到性能急剧下降。
//! * 复杂性

#[cfg(not(any(all(target_os = "linux", target_env = "gnu"), target_os = "hurd")))]
use libc::sendfile as sendfile64;
#[cfg(any(all(target_os = "linux", target_env = "gnu"), target_os = "hurd"))]
use libc::sendfile64;
use libc::{EBADF, EINVAL, ENOSYS, EOPNOTSUPP, EOVERFLOW, EPERM, EXDEV};

use super::CopyState;
use crate::cmp::min;
use crate::fs::{File, Metadata};
use crate::io::{
    BufRead, BufReader, BufWriter, Error, PipeReader, PipeWriter, Read, Result, StderrLock,
    StdinLock, StdoutLock, Take, Write,
};
use crate::mem::ManuallyDrop;
use crate::net::TcpStream;
use crate::os::unix::fs::FileTypeExt;
use crate::os::unix::io::{AsRawFd, FromRawFd, RawFd};
use crate::os::unix::net::UnixStream;
use crate::process::{ChildStderr, ChildStdin, ChildStdout};
use crate::ptr;
use crate::sync::atomic::{Atomic, AtomicBool, AtomicU8, Ordering};
use crate::sys::cvt;
use crate::sys::fs::CachedFileMetadata;
use crate::sys::weak::syscall;

#[cfg(test)]
mod tests;

pub fn kernel_copy<R: Read + ?Sized, W: Write + ?Sized>(
    read: &mut R,
    write: &mut W,
) -> Result<CopyState> {
    let copier = Copier { read, write };
    SpecCopy::copy(copier)
}

/// 这个类型表示要么是基于「提取该 `RawFd` 时所用的源类型」推断出的 `FileType`，
/// 要么是实际的元数据(metadata）
///
/// 由于 `AsRawFd` 和 `FromRawFd` 的存在，该类型上的方法只提供提示(hint），推断出
/// 的类型有可能是错的。
enum FdMeta {
    Metadata(Metadata),
    Socket,
    Pipe,
    /// 我们没有任何元数据，因为 stat 系统调用失败了
    NoneObtained,
}

#[derive(PartialEq)]
enum FdHandle {
    Input,
    Output,
}

impl FdMeta {
    fn maybe_fifo(&self) -> bool {
        match self {
            FdMeta::Metadata(meta) => meta.file_type().is_fifo(),
            FdMeta::Socket => false,
            FdMeta::Pipe => true,
            FdMeta::NoneObtained => true,
        }
    }

    fn potential_sendfile_source(&self) -> bool {
        match self {
            // procfs 会错误地把非空的可读文件显示为 0 长度。
            // 而如果一个文件确实是空的，那么 `read` 系统调用会判定这一点并跳过 write 系统调用
            // 因此尝试 sendfile 是有好处的
            FdMeta::Metadata(meta)
                if meta.file_type().is_file() && meta.len() > 0
                    || meta.file_type().is_block_device() =>
            {
                true
            }
            _ => false,
        }
    }

    fn copy_file_range_candidate(&self, f: FdHandle) -> bool {
        match self {
            // copy_file_range 在空的 procfs 文件上会失败。`read` 无需额外开销就能判断
            // 是否已到达 EOF 并跳过 write，因此尝试 copy_file_range 没有任何好处
            FdMeta::Metadata(meta) if f == FdHandle::Input && meta.is_file() && meta.len() > 0 => {
                true
            }
            FdMeta::Metadata(meta) if f == FdHandle::Output && meta.is_file() => true,
            _ => false,
        }
    }
}

/// 在以下任一情况下返回 true：在 sendfile/splice 调用之后对源所做的更改不会
/// 在汇(sink）端变得可见，或者源已经显式地选择接受这种行为（例如，通过把一个文件
/// splice 进一个管道，此时管道就是源）。
///
/// 这会阻止 File -> Pipe 和 File -> Socket 的 splice/sendfile 优化，以维护
/// io::copy 的 Read/Write API 语义。
///
/// 注意：这并非 100% 滴水不漏，调用方可以使用 RawFd 转换方法把一个普通文件变成
/// 一个 TcpSocket，那样它在这里就会被当作 socket 处理而不做检查。
fn safe_kernel_copy(source: &FdMeta, sink: &FdMeta) -> bool {
    match (source, sink) {
        // 从 socket 到达的数据是安全的，因为发送方无法修改 socket 缓冲区。
        // 从管道到达的数据（基本上）是安全的，因为发送方要么把字节*复制*进了管道，
        // 要么显式地执行了某种启用零拷贝的操作，从而承诺之后不会修改这些数据。
        (FdMeta::Socket, _) => true,
        (FdMeta::Pipe, _) => true,
        (FdMeta::Metadata(meta), _)
            if meta.file_type().is_fifo() || meta.file_type().is_socket() =>
        {
            true
        }
        // 进入非管道/非 socket 目标的数据是安全的，因为「之后的更改可能变得可见」
        // 这个问题只会发生在停留于发送缓冲区或管道中的页(page）上。
        (_, FdMeta::Metadata(meta))
            if !meta.file_type().is_fifo() && !meta.file_type().is_socket() =>
        {
            true
        }
        _ => false,
    }
}

struct CopyParams(FdMeta, Option<RawFd>);

struct Copier<'a, 'b, R: Read + ?Sized, W: Write + ?Sized> {
    read: &'a mut R,
    write: &'b mut W,
}

trait SpecCopy {
    fn copy(self) -> Result<CopyState>;
}

impl<R: Read + ?Sized, W: Write + ?Sized> SpecCopy for Copier<'_, '_, R, W> {
    default fn copy(self) -> Result<CopyState> {
        Ok(CopyState::Fallback(0))
    }
}

impl<R: CopyRead, W: CopyWrite> SpecCopy for Copier<'_, '_, R, W> {
    fn copy(self) -> Result<CopyState> {
        let (reader, writer) = (self.read, self.write);
        let r_cfg = reader.properties();
        let w_cfg = writer.properties();

        // 在直接对文件描述符进行操作之前，确保所有的源缓冲区和汇缓冲区都已清空
        let mut flush = || -> Result<u64> {
            let bytes = reader.drain_to(writer, u64::MAX)?;
            // BufWriter 中已缓冲的字节在之前的 write() 调用中已经被计入了
            writer.flush()?;
            Ok(bytes)
        };

        let mut written = 0u64;

        if let (CopyParams(input_meta, Some(readfd)), CopyParams(output_meta, Some(writefd))) =
            (r_cfg, w_cfg)
        {
            written += flush()?;
            let max_write = reader.min_limit();

            if input_meta.copy_file_range_candidate(FdHandle::Input)
                && output_meta.copy_file_range_candidate(FdHandle::Output)
            {
                let result = copy_regular_files(readfd, writefd, max_write);
                result.update_take(reader);

                match result {
                    CopyResult::Ended(bytes_copied) => {
                        return Ok(CopyState::Ended(bytes_copied + written));
                    }
                    CopyResult::Error(e, _) => return Err(e),
                    CopyResult::Fallback(bytes) => written += bytes,
                }
            }

            // 在现代内核上，sendfile 可以从任何可 mmap 的类型（部分但非全部的普通文件
            // 和块设备）复制到任何可写的文件描述符。在较旧的内核上，写入端只能是 socket。
            // 所以我们只是尝试一下，必要时回退。
            // 如果当前文件偏移量 + 写入大小发生溢出，它也可能失败，我们不去尝试修复这一点，
            // 而是回退到通用的复制循环。
            if input_meta.potential_sendfile_source() && safe_kernel_copy(&input_meta, &output_meta)
            {
                let result = sendfile_splice(SpliceMode::Sendfile, readfd, writefd, max_write);
                result.update_take(reader);

                match result {
                    CopyResult::Ended(bytes_copied) => {
                        return Ok(CopyState::Ended(bytes_copied + written));
                    }
                    CopyResult::Error(e, _) => return Err(e),
                    CopyResult::Fallback(bytes) => written += bytes,
                }
            }

            if (input_meta.maybe_fifo() || output_meta.maybe_fifo())
                && safe_kernel_copy(&input_meta, &output_meta)
            {
                let result = sendfile_splice(SpliceMode::Splice, readfd, writefd, max_write);
                result.update_take(reader);

                match result {
                    CopyResult::Ended(bytes_copied) => {
                        return Ok(CopyState::Ended(bytes_copied + written));
                    }
                    CopyResult::Error(e, _) => return Err(e),
                    CopyResult::Fallback(0) => { /* 使用下面的回退路径 */ }
                    CopyResult::Fallback(_) => {
                        unreachable!("splice should not return > 0 bytes on the fallback path")
                    }
                }
            }
        }

        // 如果那些更专门化的系统调用都不愿意处理这些文件描述符，则回退
        Ok(CopyState::Fallback(written))
    }
}

#[rustc_specialization_trait]
trait CopyRead: Read {
    /// 包含缓冲区的实现（即 `BufReader`）必须把数据从其内部缓冲区搬运到 `writer`，
    /// 直到缓冲区被清空或已传输 `limit` 个字节为止，以先发生者为准。
    /// 如果存在嵌套缓冲区，必须先排空外层的缓冲区。
    ///
    /// 当直接对底层文件描述符进行操作时，这是绕过包装类型同时又保持数据顺序所必需的。
    fn drain_to<W: Write>(&mut self, _writer: &mut W, _limit: u64) -> Result<u64> {
        Ok(0)
    }

    /// 更新 `Take` 包装器，以扣减已复制的字节数。
    fn taken(&mut self, _bytes: u64) {}

    /// 所有 `Take<_>` 包装器的限制中的最小值，否则为 `u64::MAX`。
    /// 该方法不会考虑 `BufReader` 缓冲区中的数据，会低估 `Take<BufReader<Take<_>>>`
    /// 类型的限制。因此，它的结果只有在通过 `drain_to` 排空缓冲区之后才有效。
    fn min_limit(&self) -> u64 {
        u64::MAX
    }

    /// 提取文件描述符以及提示/元数据，必要时通过包装器进行委托。
    fn properties(&self) -> CopyParams;
}

#[rustc_specialization_trait]
trait CopyWrite: Write {
    /// 提取文件描述符以及提示/元数据，必要时通过包装器进行委托。
    fn properties(&self) -> CopyParams;
}

impl<T> CopyRead for &mut T
where
    T: CopyRead,
{
    fn drain_to<W: Write>(&mut self, writer: &mut W, limit: u64) -> Result<u64> {
        (**self).drain_to(writer, limit)
    }

    fn taken(&mut self, bytes: u64) {
        (**self).taken(bytes);
    }

    fn min_limit(&self) -> u64 {
        (**self).min_limit()
    }

    fn properties(&self) -> CopyParams {
        (**self).properties()
    }
}

impl<T> CopyWrite for &mut T
where
    T: CopyWrite,
{
    fn properties(&self) -> CopyParams {
        (**self).properties()
    }
}

impl CopyRead for File {
    fn properties(&self) -> CopyParams {
        CopyParams(fd_to_meta(self), Some(self.as_raw_fd()))
    }
}

impl CopyRead for &File {
    fn properties(&self) -> CopyParams {
        CopyParams(fd_to_meta(*self), Some(self.as_raw_fd()))
    }
}

impl CopyWrite for File {
    fn properties(&self) -> CopyParams {
        CopyParams(fd_to_meta(self), Some(self.as_raw_fd()))
    }
}

impl CopyWrite for &File {
    fn properties(&self) -> CopyParams {
        CopyParams(fd_to_meta(*self), Some(self.as_raw_fd()))
    }
}

impl CopyRead for TcpStream {
    fn properties(&self) -> CopyParams {
        // 避免 stat 系统调用，因为我们可以相当确定它是一个 socket
        CopyParams(FdMeta::Socket, Some(self.as_raw_fd()))
    }
}

impl CopyRead for &TcpStream {
    fn properties(&self) -> CopyParams {
        // 避免 stat 系统调用，因为我们可以相当确定它是一个 socket
        CopyParams(FdMeta::Socket, Some(self.as_raw_fd()))
    }
}

impl CopyWrite for TcpStream {
    fn properties(&self) -> CopyParams {
        // 避免 stat 系统调用，因为我们可以相当确定它是一个 socket
        CopyParams(FdMeta::Socket, Some(self.as_raw_fd()))
    }
}

impl CopyWrite for &TcpStream {
    fn properties(&self) -> CopyParams {
        // 避免 stat 系统调用，因为我们可以相当确定它是一个 socket
        CopyParams(FdMeta::Socket, Some(self.as_raw_fd()))
    }
}

impl CopyRead for UnixStream {
    fn properties(&self) -> CopyParams {
        // 避免 stat 系统调用，因为我们可以相当确定它是一个 socket
        CopyParams(FdMeta::Socket, Some(self.as_raw_fd()))
    }
}

impl CopyRead for &UnixStream {
    fn properties(&self) -> CopyParams {
        // 避免 stat 系统调用，因为我们可以相当确定它是一个 socket
        CopyParams(FdMeta::Socket, Some(self.as_raw_fd()))
    }
}

impl CopyWrite for UnixStream {
    fn properties(&self) -> CopyParams {
        // 避免 stat 系统调用，因为我们可以相当确定它是一个 socket
        CopyParams(FdMeta::Socket, Some(self.as_raw_fd()))
    }
}

impl CopyWrite for &UnixStream {
    fn properties(&self) -> CopyParams {
        // 避免 stat 系统调用，因为我们可以相当确定它是一个 socket
        CopyParams(FdMeta::Socket, Some(self.as_raw_fd()))
    }
}

impl CopyRead for PipeReader {
    fn properties(&self) -> CopyParams {
        CopyParams(FdMeta::Pipe, Some(self.as_raw_fd()))
    }
}

impl CopyRead for &PipeReader {
    fn properties(&self) -> CopyParams {
        CopyParams(FdMeta::Pipe, Some(self.as_raw_fd()))
    }
}

impl CopyWrite for PipeWriter {
    fn properties(&self) -> CopyParams {
        CopyParams(FdMeta::Pipe, Some(self.as_raw_fd()))
    }
}

impl CopyWrite for &PipeWriter {
    fn properties(&self) -> CopyParams {
        CopyParams(FdMeta::Pipe, Some(self.as_raw_fd()))
    }
}

impl CopyWrite for ChildStdin {
    fn properties(&self) -> CopyParams {
        CopyParams(FdMeta::Pipe, Some(self.as_raw_fd()))
    }
}

impl CopyRead for ChildStdout {
    fn properties(&self) -> CopyParams {
        CopyParams(FdMeta::Pipe, Some(self.as_raw_fd()))
    }
}

impl CopyRead for ChildStderr {
    fn properties(&self) -> CopyParams {
        CopyParams(FdMeta::Pipe, Some(self.as_raw_fd()))
    }
}

impl CopyRead for StdinLock<'_> {
    fn drain_to<W: Write>(&mut self, writer: &mut W, outer_limit: u64) -> Result<u64> {
        let buf_reader = self.as_mut_buf();
        let buf = buf_reader.buffer();
        let buf = &buf[0..min(buf.len(), outer_limit.try_into().unwrap_or(usize::MAX))];
        let bytes_drained = buf.len();
        writer.write_all(buf)?;
        buf_reader.consume(bytes_drained);

        Ok(bytes_drained as u64)
    }

    fn properties(&self) -> CopyParams {
        CopyParams(fd_to_meta(self), Some(self.as_raw_fd()))
    }
}

impl CopyWrite for StdoutLock<'_> {
    fn properties(&self) -> CopyParams {
        CopyParams(fd_to_meta(self), Some(self.as_raw_fd()))
    }
}

impl CopyWrite for StderrLock<'_> {
    fn properties(&self) -> CopyParams {
        CopyParams(fd_to_meta(self), Some(self.as_raw_fd()))
    }
}

impl<T: CopyRead> CopyRead for Take<T> {
    fn drain_to<W: Write>(&mut self, writer: &mut W, outer_limit: u64) -> Result<u64> {
        let local_limit = self.limit();
        let combined_limit = min(outer_limit, local_limit);
        let bytes_drained = self.get_mut().drain_to(writer, combined_limit)?;
        // 由于 read() 被绕过了，需要更新 limit
        self.set_limit(local_limit - bytes_drained);

        Ok(bytes_drained)
    }

    fn taken(&mut self, bytes: u64) {
        self.set_limit(self.limit() - bytes);
        self.get_mut().taken(bytes);
    }

    fn min_limit(&self) -> u64 {
        min(Take::limit(self), self.get_ref().min_limit())
    }

    fn properties(&self) -> CopyParams {
        self.get_ref().properties()
    }
}

impl<T: ?Sized + CopyRead> CopyRead for BufReader<T> {
    fn drain_to<W: Write>(&mut self, writer: &mut W, outer_limit: u64) -> Result<u64> {
        let buf = self.buffer();
        let buf = &buf[0..min(buf.len(), outer_limit.try_into().unwrap_or(usize::MAX))];
        let bytes = buf.len();
        writer.write_all(buf)?;
        self.consume(bytes);

        let remaining = outer_limit - bytes as u64;

        // 如果是嵌套的 bufreader，我们还需要排空那些更靠近源的缓冲区
        let inner_bytes = self.get_mut().drain_to(writer, remaining)?;

        Ok(bytes as u64 + inner_bytes)
    }

    fn taken(&mut self, bytes: u64) {
        self.get_mut().taken(bytes);
    }

    fn min_limit(&self) -> u64 {
        self.get_ref().min_limit()
    }

    fn properties(&self) -> CopyParams {
        self.get_ref().properties()
    }
}

impl<T: ?Sized + CopyWrite> CopyWrite for BufWriter<T> {
    fn properties(&self) -> CopyParams {
        self.get_ref().properties()
    }
}

impl CopyRead for CachedFileMetadata {
    fn properties(&self) -> CopyParams {
        CopyParams(FdMeta::Metadata(self.1.clone()), Some(self.0.as_raw_fd()))
    }
}

impl CopyWrite for CachedFileMetadata {
    fn properties(&self) -> CopyParams {
        CopyParams(FdMeta::Metadata(self.1.clone()), Some(self.0.as_raw_fd()))
    }
}

fn fd_to_meta<T: AsRawFd>(fd: &T) -> FdMeta {
    let fd = fd.as_raw_fd();
    let file: ManuallyDrop<File> = ManuallyDrop::new(unsafe { File::from_raw_fd(fd) });
    match file.metadata() {
        Ok(meta) => FdMeta::Metadata(meta),
        Err(_) => FdMeta::NoneObtained,
    }
}

enum CopyResult {
    Ended(u64),
    Error(Error, u64),
    Fallback(u64),
}

impl CopyResult {
    fn update_take(&self, reader: &mut impl CopyRead) {
        match *self {
            CopyResult::Fallback(bytes)
            | CopyResult::Ended(bytes)
            | CopyResult::Error(_, bytes) => reader.taken(bytes),
        }
    }
}

/// 无效的文件描述符。
///
/// 有效的文件描述符保证是正数（参见 `open()` 手册页），
/// 而负值则用于表示错误。
/// 因此 -1 永远不会与一个有效的已打开文件重叠。
const INVALID_FD: RawFd = -1;

/// Linux 特有的实现，会尝试使用 copy_file_range 来进行复制卸载(copy offloading）。
/// 如其名所示，它只对普通文件起作用。
///
/// 调用方必须处理回退到通用复制循环的情况。
/// 如果某个文件的游标 + `max_len` 会超出 u64::MAX（`EOVERFLOW`），那么 `Fallback`
/// 可能表示已经写入了非零数量的字节。
fn copy_regular_files(reader: RawFd, writer: RawFd, max_len: u64) -> CopyResult {
    use crate::cmp;

    const NOT_PROBED: u8 = 0;
    const UNAVAILABLE: u8 = 1;
    const AVAILABLE: u8 = 2;

    // 4.5 之前的内核没有 copy_file_range
    // 我们把可用性存储在一个全局变量中，以避免不必要的系统调用
    static HAS_COPY_FILE_RANGE: Atomic<u8> = AtomicU8::new(NOT_PROBED);

    let mut have_probed = match HAS_COPY_FILE_RANGE.load(Ordering::Relaxed) {
        NOT_PROBED => false,
        UNAVAILABLE => return CopyResult::Fallback(0),
        _ => true,
    };

    syscall!(
        fn copy_file_range(
            fd_in: libc::c_int,
            off_in: *mut libc::loff_t,
            fd_out: libc::c_int,
            off_out: *mut libc::loff_t,
            len: libc::size_t,
            flags: libc::c_uint,
        ) -> libc::ssize_t;
    );

    fn probe_copy_file_range_support() -> u8 {
        // 在某些情况下，我们无法从第一次 `copy_file_range` 调用中确定可用性。
        // 此时我们用一个无效的文件描述符来探测，以便结果易于解读。
        match unsafe {
            cvt(copy_file_range(INVALID_FD, ptr::null_mut(), INVALID_FD, ptr::null_mut(), 1, 0))
                .map_err(|e| e.raw_os_error())
        } {
            Err(Some(EPERM | ENOSYS)) => UNAVAILABLE,
            Err(Some(EBADF)) => AVAILABLE,
            Ok(_) => panic!("unexpected copy_file_range probe success"),
            // 把其他错误当作该系统调用
            // 不可用来处理。
            Err(_) => UNAVAILABLE,
        }
    }

    let mut written = 0u64;
    while written < max_len {
        let bytes_to_copy = cmp::min(max_len - written, usize::MAX as u64);
        // 限制为 1GB 的块，以防 max_len 传入的是 u64::MAX 且文件具有非零的 seek 位置
        // 这让我们能够复制大块数据而不触发 EOVERFLOW，
        // 除非有人把文件偏移量设置到接近 u64::MAX - 1GB，那种情况下就需要回退
        let bytes_to_copy = cmp::min(bytes_to_copy as usize, 0x4000_0000usize);
        let copy_result = unsafe {
            // 我们实际上不必调整偏移量，
            // 因为 copy_file_range 会自动调整文件偏移量
            cvt(copy_file_range(reader, ptr::null_mut(), writer, ptr::null_mut(), bytes_to_copy, 0))
        };

        if !have_probed && copy_result.is_ok() {
            have_probed = true;
            HAS_COPY_FILE_RANGE.store(AVAILABLE, Ordering::Relaxed);
        }

        match copy_result {
            Ok(0) if written == 0 => {
                // 回退，以绕过若干内核 bug：在这些情况下 copy_file_range 会无法复制
                // 任何字节，并返回 0 而非一个错误，例如
                // - 读取 proc 文件系统中的虚拟文件，它们看起来大小为 0 但实际上并非为空。
                //   coreutils 中记录过这至少会影响到 5.6.19 之前的内核。
                // - 在 docker 中从一个 overlay 文件系统复制。据报告会在 fedora 32 上发生。
                return CopyResult::Fallback(0);
            }
            Ok(0) => return CopyResult::Ended(written), // 到达 EOF
            Ok(ret) => written += ret as u64,
            Err(err) => {
                return match err.raw_os_error() {
                    // 当 file offset + max_length > u64::MAX 时
                    Some(EOVERFLOW) => CopyResult::Fallback(written),
                    Some(raw_os_error @ (ENOSYS | EXDEV | EINVAL | EPERM | EOPNOTSUPP | EBADF))
                        if written == 0 =>
                    {
                        if !have_probed {
                            let available = if matches!(raw_os_error, ENOSYS | EOPNOTSUPP | EPERM) {
                                // EPERM 可能表示存在 seccomp 过滤器或一个不可变(immutable）
                                // 文件。为了区分这些情况，我们用无效的文件描述符进行探测：
                                // 如果该系统调用受支持，应当得到 EBADF；如果不可用，则得到
                                // EPERM 或 ENOSYS。
                                //
                                // 关于 EOPNOTSUPP，见下文。对于 ENOSYS 的情况，我们试图为
                                // 有缺陷的 FUSE 驱动做兜底。
                                probe_copy_file_range_support()
                            } else {
                                AVAILABLE
                            };
                            HAS_COPY_FILE_RANGE.store(available, Ordering::Relaxed);
                        }

                        // 在以下任一情况下尝试回退到 io::copy：
                        // - 内核版本 < 4.5（ENOSYS¹）
                        // - 文件挂载在不同的文件系统上（EXDEV）
                        // - copy_file_range 在 RHEL/CentOS 7 上以各种方式损坏（EOPNOTSUPP）
                        // - copy_file_range 操作的文件不可变，或该系统调用被 seccomp 阻止¹（EPERM）
                        // - copy_file_range 不能用于管道或设备节点（EINVAL）
                        // - 写入端 fd 是以 O_APPEND 打开的（EBADF²）
                        // 并且尚未成功写入任何字节。（如果已经写入了一些数据，这些 errno
                        // 都不应该被返回，但它们在现实中确实会出现，见 #91152。）
                        //
                        // ¹ 这些情况本应被最初的探测检测到，但我们在这里仍然处理它们
                        //   以防系统调用拦截在运行时发生变化
                        // ² 实际上无效的文件描述符也会导致这个错误，但那种情况下
                        //   预期回退代码路径会再次遇到同样的错误
                        CopyResult::Fallback(0)
                    }
                    _ => CopyResult::Error(err, written),
                };
            }
        }
    }
    CopyResult::Ended(written)
}

#[derive(PartialEq)]
enum SpliceMode {
    Sendfile,
    Splice,
}

/// 在文件描述符之间执行 splice 或 sendfile
/// _不会_ 回退到通用的复制循环。
fn sendfile_splice(mode: SpliceMode, reader: RawFd, writer: RawFd, len: u64) -> CopyResult {
    static HAS_SENDFILE: Atomic<bool> = AtomicBool::new(true);
    static HAS_SPLICE: Atomic<bool> = AtomicBool::new(true);

    // Android 构建使用 feature level 14，但 libc 对 splice 的封装受限于 feature
    // level 21+，所以我们必须直接调用该系统调用。
    #[cfg(target_os = "android")]
    syscall!(
        fn splice(
            srcfd: libc::c_int,
            src_offset: *const i64,
            dstfd: libc::c_int,
            dst_offset: *const i64,
            len: libc::size_t,
            flags: libc::c_int,
        ) -> libc::ssize_t;
    );

    #[cfg(target_os = "linux")]
    use libc::splice;

    match mode {
        SpliceMode::Sendfile if !HAS_SENDFILE.load(Ordering::Relaxed) => {
            return CopyResult::Fallback(0);
        }
        SpliceMode::Splice if !HAS_SPLICE.load(Ordering::Relaxed) => {
            return CopyResult::Fallback(0);
        }
        _ => (),
    }

    let mut written = 0u64;
    while written < len {
        // 根据其手册页，这是 sendfile() 每次调用所能复制的最大尺寸
        let chunk_size = crate::cmp::min(len - written, 0x7ffff000_u64) as usize;

        let result = match mode {
            SpliceMode::Sendfile => {
                cvt(unsafe { sendfile64(writer, reader, ptr::null_mut(), chunk_size) })
            }
            SpliceMode::Splice => cvt(unsafe {
                splice(reader, ptr::null_mut(), writer, ptr::null_mut(), chunk_size, 0)
            }),
        };

        match result {
            Ok(0) => break, // EOF
            Ok(ret) => written += ret as u64,
            Err(err) => {
                return match err.raw_os_error() {
                    Some(ENOSYS | EPERM) => {
                        // 系统调用不受支持（ENOSYS）
                        // 系统调用被禁止，例如被 seccomp 禁止（EPERM）
                        match mode {
                            SpliceMode::Sendfile => HAS_SENDFILE.store(false, Ordering::Relaxed),
                            SpliceMode::Splice => HAS_SPLICE.store(false, Ordering::Relaxed),
                        }
                        assert_eq!(written, 0);
                        CopyResult::Fallback(0)
                    }
                    Some(EINVAL) => {
                        // splice/sendfile 不支持这个特定的文件描述符（EINVAL）
                        assert_eq!(written, 0);
                        CopyResult::Fallback(0)
                    }
                    Some(os_err) if mode == SpliceMode::Sendfile && os_err == EOVERFLOW => {
                        CopyResult::Fallback(written)
                    }
                    _ => CopyResult::Error(err, written),
                };
            }
        }
    }
    CopyResult::Ended(written)
}
