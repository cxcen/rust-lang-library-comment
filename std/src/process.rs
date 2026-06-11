//! 用于处理进程的模块。
//!
//! 本模块主要关注子进程的 spawn（衍生）与交互，但同时也提供了 [`abort`] 和 [`exit`]
//! 用于终止当前进程。
//!
//! # 衍生一个进程
//!
//! [`Command`] 结构体用于配置并 spawn 进程：
//!
//! ```no_run
//! use std::process::Command;
//!
//! let output = Command::new("echo")
//!     .arg("Hello world")
//!     .output()
//!     .expect("Failed to execute command");
//!
//! assert_eq!(b"Hello world\n", output.stdout.as_slice());
//! ```
//!
//! [`Command`] 上有若干方法（例如 [`spawn`] 或 [`output`]）可用于 spawn 一个进程。
//! 特别地，[`output`] 会 spawn 子进程并一直等待该进程终止，而 [`spawn`] 会返回一个
//! 表示已衍生子进程的 [`Child`]。
//!
//! # 处理 I/O
//!
//! 子进程的 [`stdout`]、[`stdin`] 和 [`stderr`] 可以通过向 [`Command`] 上对应的方法
//! 传入一个 [`Stdio`] 来配置。进程衍生之后，可以从 [`Child`] 上访问它们。例如，把一个
//! 命令的输出通过管道接到另一个命令，可以这样做：
//!
//! ```no_run
//! use std::process::{Command, Stdio};
//!
//! // 必须用 `Stdio::piped` 配置 stdout 才能使用
//! // `echo_child.stdout`
//! let echo_child = Command::new("echo")
//!     .arg("Oh no, a tpyo!")
//!     .stdout(Stdio::piped())
//!     .spawn()
//!     .expect("Failed to start echo process");
//!
//! // 注意 `echo_child` 在这里被 move 了，但我们之后
//! // 不再需要 `echo_child`
//! let echo_out = echo_child.stdout.expect("Failed to open echo stdout");
//!
//! let mut sed_child = Command::new("sed")
//!     .arg("s/tpyo/typo/")
//!     .stdin(Stdio::from(echo_out))
//!     .stdout(Stdio::piped())
//!     .spawn()
//!     .expect("Failed to start sed process");
//!
//! let output = sed_child.wait_with_output().expect("Failed to wait on sed");
//! assert_eq!(b"Oh no, a typo!\n", output.stdout.as_slice());
//! ```
//!
//! 注意 [`ChildStderr`] 和 [`ChildStdout`] 实现了 [`Read`]，而
//! [`ChildStdin`] 实现了 [`Write`]：
//!
//! ```no_run
//! use std::process::{Command, Stdio};
//! use std::io::Write;
//!
//! let mut child = Command::new("/bin/cat")
//!     .stdin(Stdio::piped())
//!     .stdout(Stdio::piped())
//!     .spawn()
//!     .expect("failed to execute child");
//!
//! // 如果子进程把它的 stdout 缓冲区填满了，它可能会一直
//! // 等待父进程读取 stdout，而在此期间无法读取 stdin，
//! // 从而造成死锁。
//! // 在另一个线程里写入可以确保 stdout 同时被读取，
//! // 从而避免该问题。
//! let mut stdin = child.stdin.take().expect("failed to get stdin");
//! std::thread::spawn(move || {
//!     stdin.write_all(b"test").expect("failed to write to stdin");
//! });
//!
//! let output = child
//!     .wait_with_output()
//!     .expect("failed to wait on child");
//!
//! assert_eq!(b"test", output.stdout.as_slice());
//! ```
//!
//! # Windows argument splitting
//!
//! 在 Unix 系统上参数会以字符串数组的形式传给新进程，但在 Windows 上参数是作为单个
//! 命令行字符串传递的，由子进程负责把它解析成数组。因此父进程和子进程必须就命令行
//! 字符串如何编码达成一致。
//!
//! 大多数程序使用标准 C 运行时的 `argv`，这在实践中能带来一致的参数处理。然而，有些
//! 程序有自己解析命令行字符串的方式。在这些情况下使用 [`arg`] 或 [`args`] 可能会导致
//! 子进程看到的参数数组与父进程意图传递的不同。
//!
//! 缓解这一问题的两种方式：
//!
//! * 校验不可信输入，使得只允许安全的子集。
//! * 使用 [`raw_arg`] 构建自定义命令行。这会绕过 [`arg`] 使用的转义规则，因此应当
//!   谨慎使用。
//!
//! `cmd.exe` 和 `.bat` 文件使用非标准的参数解析方式，且由于它们可能被用来运行任意
//! shell 命令，因此对恶意输入尤其脆弱。不可信参数应尽可能地加以限制。处理这一问题的
//! 示例见 [`raw_arg`]。
//!
//! ### 批处理文件的特殊处理
//!
//! 在 Windows 上，`Command` 使用 Windows API 函数 [`CreateProcessW`] 来 spawn 新进程。
//! 该函数有一个未文档化的特性：当给定一个 `.bat` 文件作为要运行的应用程序时，它会
//! 自动将其转换为运行 `cmd.exe /c`，并以该批处理文件作为下一个参数。
//!
//! 出于历史原因，Rust 目前在使用 [`Command::new`] 时保留了这一行为，并按照 `cmd.exe`
//! 的规则对参数进行转义。由于 `cmd.exe` 参数处理的复杂性，某些特殊字符可能无法被安全
//! 转义，使用它们将导致在进程 spawn 时返回一个错误。无法转义的特殊字符集合可能在不同
//! 版本之间发生变化。
//!
//! 另请注意，以这种方式运行批处理脚本的功能未来可能会被移除，因此不应依赖它。
//!
//! [`spawn`]: Command::spawn
//! [`output`]: Command::output
//!
//! [`stdout`]: Command::stdout
//! [`stdin`]: Command::stdin
//! [`stderr`]: Command::stderr
//!
//! [`Write`]: io::Write
//! [`Read`]: io::Read
//!
//! [`arg`]: Command::arg
//! [`args`]: Command::args
//! [`raw_arg`]: crate::os::windows::process::CommandExt::raw_arg
//!
//! [`CreateProcessW`]: https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-createprocessw

#![stable(feature = "process", since = "1.0.0")]
#![deny(unsafe_op_in_unsafe_fn)]

#[cfg(all(
    test,
    not(any(
        target_os = "emscripten",
        target_os = "wasi",
        target_env = "sgx",
        target_os = "xous",
        target_os = "trusty",
    ))
))]
mod tests;

use crate::convert::Infallible;
use crate::ffi::OsStr;
use crate::io::prelude::*;
use crate::io::{self, BorrowedCursor, IoSlice, IoSliceMut};
use crate::num::NonZero;
use crate::path::Path;
use crate::sys::{AsInner, AsInnerMut, FromInner, IntoInner, process as imp};
use crate::{fmt, format_args_nl, fs, str};

/// 一个正在运行或已退出的子进程的表示。
///
/// 该结构体用于表示并管理子进程。子进程通过 [`Command`] 结构体创建，[`Command`]
/// 负责配置 spawn 进程，并且其本身可以使用 builder 风格的接口来构造。
///
/// 子进程没有 [`Drop`] 的实现，因此如果你不确保 `Child` 已经退出，它就会继续运行，
/// 即使指向该子进程的 `Child` 句柄已经离开作用域也是如此。
///
/// 调用 [`wait`]（或其他包装它的函数）会使父进程一直等待，直到子进程真正退出后
/// 才继续往下执行。
///
/// # 警告（Warning）
///
/// 在某些系统上，调用 [`wait`] 或类似函数对于让操作系统释放资源是必要的。一个已经
/// 终止但尚未被 wait 的进程仍然作为“僵尸（zombie）”存在。留下太多僵尸进程可能会
/// 耗尽全局资源（例如进程 ID）。
///
/// 标准库**不会**自动 wait 子进程（即使 `Child` 被 drop 也不会），这需要由应用
/// 开发者自己来做。因此，在长期运行的应用中，不先 wait 就 drop 掉 `Child` 句柄
/// 是不推荐的做法。
///
/// # 示例
///
/// ```should_panic
/// use std::process::Command;
///
/// let mut child = Command::new("/bin/cat")
///     .arg("file.txt")
///     .spawn()
///     .expect("failed to execute child");
///
/// let ecode = child.wait().expect("failed to wait on child");
///
/// assert!(ecode.success());
/// ```
///
/// [`wait`]: Child::wait
#[stable(feature = "process", since = "1.0.0")]
#[cfg_attr(not(test), rustc_diagnostic_item = "Child")]
pub struct Child {
    pub(crate) handle: imp::Process,

    /// 用于写入子进程标准输入（stdin）的句柄，前提是它已被捕获。你可能会发现
    /// 这样做很有帮助
    ///
    /// ```ignore (incomplete)
    /// let stdin = child.stdin.take().expect("handle present");
    /// ```
    ///
    /// 以避免对 `child` 进行部分 move，从而避免在使用 `stdin` 时阻碍你在 `child`
    /// 上调用其他函数。
    #[stable(feature = "process", since = "1.0.0")]
    pub stdin: Option<ChildStdin>,

    /// 用于从子进程标准输出（stdout）读取的句柄，前提是它已被捕获。你可能会发现
    /// 这样做很有帮助
    ///
    /// ```ignore (incomplete)
    /// let stdout = child.stdout.take().expect("handle present");
    /// ```
    ///
    /// 以避免对 `child` 进行部分 move，从而避免在使用 `stdout` 时阻碍你在 `child`
    /// 上调用其他函数。
    #[stable(feature = "process", since = "1.0.0")]
    pub stdout: Option<ChildStdout>,

    /// 用于从子进程标准错误（stderr）读取的句柄，前提是它已被捕获。你可能会发现
    /// 这样做很有帮助
    ///
    /// ```ignore (incomplete)
    /// let stderr = child.stderr.take().expect("handle present");
    /// ```
    ///
    /// 以避免对 `child` 进行部分 move，从而避免在使用 `stderr` 时阻碍你在 `child`
    /// 上调用其他函数。
    #[stable(feature = "process", since = "1.0.0")]
    pub stderr: Option<ChildStderr>,
}

/// 允许在 `std` 内部定义扩展 trait。
#[unstable(feature = "sealed", issue = "none")]
impl crate::sealed::Sealed for Child {}

impl AsInner<imp::Process> for Child {
    #[inline]
    fn as_inner(&self) -> &imp::Process {
        &self.handle
    }
}

impl FromInner<(imp::Process, StdioPipes)> for Child {
    fn from_inner((handle, io): (imp::Process, StdioPipes)) -> Child {
        Child {
            handle,
            stdin: io.stdin.map(ChildStdin::from_inner),
            stdout: io.stdout.map(ChildStdout::from_inner),
            stderr: io.stderr.map(ChildStderr::from_inner),
        }
    }
}

impl IntoInner<imp::Process> for Child {
    fn into_inner(self) -> imp::Process {
        self.handle
    }
}

#[stable(feature = "std_debug", since = "1.16.0")]
impl fmt::Debug for Child {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Child")
            .field("stdin", &self.stdin)
            .field("stdout", &self.stdout)
            .field("stderr", &self.stderr)
            .finish_non_exhaustive()
    }
}

/// 连接到一个已衍生进程的管道集合。
///
/// 用于在本模块与 [`imp`] 之间传递管道句柄。
pub(crate) struct StdioPipes {
    pub stdin: Option<imp::ChildPipe>,
    pub stdout: Option<imp::ChildPipe>,
    pub stderr: Option<imp::ChildPipe>,
}

/// 指向子进程标准输入（stdin）的句柄。
///
/// 此结构体被用于 [`Child`] 的 [`stdin`] 字段中。
///
/// 当一个 `ChildStdin` 实例被 [dropped] 时，`ChildStdin` 底层的文件句柄将被关闭。
/// 如果子进程在被 drop 之前正阻塞于等待输入，drop 之后它会被解除阻塞。
///
/// [`stdin`]: Child::stdin
/// [dropped]: Drop
#[stable(feature = "process", since = "1.0.0")]
pub struct ChildStdin {
    inner: imp::ChildPipe,
}

// 除了这里的这些 `impl` 之外，`ChildStdin` 在 Unix 和 WASI 上还有
// `AsFd`/`From<OwnedFd>`/`Into<OwnedFd>` 以及
// `AsRawFd`/`IntoRawFd`/`FromRawFd` 的 `impl`，在 Windows 上还有
// `AsHandle`/`From<OwnedHandle>`/`Into<OwnedHandle>` 以及
// `AsRawHandle`/`IntoRawHandle`/`FromRawHandle` 的 `impl`。

#[stable(feature = "process", since = "1.0.0")]
impl Write for ChildStdin {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        (&*self).write(buf)
    }

    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        (&*self).write_vectored(bufs)
    }

    fn is_write_vectored(&self) -> bool {
        io::Write::is_write_vectored(&&*self)
    }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        (&*self).flush()
    }
}

#[stable(feature = "write_mt", since = "1.48.0")]
impl Write for &ChildStdin {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.inner.write(buf)
    }

    fn write_vectored(&mut self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        self.inner.write_vectored(bufs)
    }

    fn is_write_vectored(&self) -> bool {
        self.inner.is_write_vectored()
    }

    #[inline]
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl AsInner<imp::ChildPipe> for ChildStdin {
    #[inline]
    fn as_inner(&self) -> &imp::ChildPipe {
        &self.inner
    }
}

impl IntoInner<imp::ChildPipe> for ChildStdin {
    fn into_inner(self) -> imp::ChildPipe {
        self.inner
    }
}

impl FromInner<imp::ChildPipe> for ChildStdin {
    fn from_inner(pipe: imp::ChildPipe) -> ChildStdin {
        ChildStdin { inner: pipe }
    }
}

#[stable(feature = "std_debug", since = "1.16.0")]
impl fmt::Debug for ChildStdin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ChildStdin").finish_non_exhaustive()
    }
}

/// 指向子进程标准输出（stdout）的句柄。
///
/// 此结构体被用于 [`Child`] 的 [`stdout`] 字段中。
///
/// 当一个 `ChildStdout` 实例被 [dropped] 时，`ChildStdout` 底层的文件句柄将被关闭。
///
/// [`stdout`]: Child::stdout
/// [dropped]: Drop
#[stable(feature = "process", since = "1.0.0")]
pub struct ChildStdout {
    inner: imp::ChildPipe,
}

// 除了这里的这些 `impl` 之外，`ChildStdout` 在 Unix 和 WASI 上还有
// `AsFd`/`From<OwnedFd>`/`Into<OwnedFd>` 以及
// `AsRawFd`/`IntoRawFd`/`FromRawFd` 的 `impl`，在 Windows 上还有
// `AsHandle`/`From<OwnedHandle>`/`Into<OwnedHandle>` 以及
// `AsRawHandle`/`IntoRawHandle`/`FromRawHandle` 的 `impl`。

#[stable(feature = "process", since = "1.0.0")]
impl Read for ChildStdout {
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
}

impl AsInner<imp::ChildPipe> for ChildStdout {
    #[inline]
    fn as_inner(&self) -> &imp::ChildPipe {
        &self.inner
    }
}

impl IntoInner<imp::ChildPipe> for ChildStdout {
    fn into_inner(self) -> imp::ChildPipe {
        self.inner
    }
}

impl FromInner<imp::ChildPipe> for ChildStdout {
    fn from_inner(pipe: imp::ChildPipe) -> ChildStdout {
        ChildStdout { inner: pipe }
    }
}

#[stable(feature = "std_debug", since = "1.16.0")]
impl fmt::Debug for ChildStdout {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ChildStdout").finish_non_exhaustive()
    }
}

/// 指向子进程标准错误（stderr）的句柄。
///
/// 此结构体被用于 [`Child`] 的 [`stderr`] 字段中。
///
/// 当一个 `ChildStderr` 实例被 [dropped] 时，`ChildStderr` 底层的文件句柄将被关闭。
///
/// [`stderr`]: Child::stderr
/// [dropped]: Drop
#[stable(feature = "process", since = "1.0.0")]
pub struct ChildStderr {
    inner: imp::ChildPipe,
}

// 除了这里的这些 `impl` 之外，`ChildStderr` 在 Unix 和 WASI 上还有
// `AsFd`/`From<OwnedFd>`/`Into<OwnedFd>` 以及
// `AsRawFd`/`IntoRawFd`/`FromRawFd` 的 `impl`，在 Windows 上还有
// `AsHandle`/`From<OwnedHandle>`/`Into<OwnedHandle>` 以及
// `AsRawHandle`/`IntoRawHandle`/`FromRawHandle` 的 `impl`。

#[stable(feature = "process", since = "1.0.0")]
impl Read for ChildStderr {
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
}

impl AsInner<imp::ChildPipe> for ChildStderr {
    #[inline]
    fn as_inner(&self) -> &imp::ChildPipe {
        &self.inner
    }
}

impl IntoInner<imp::ChildPipe> for ChildStderr {
    fn into_inner(self) -> imp::ChildPipe {
        self.inner
    }
}

impl FromInner<imp::ChildPipe> for ChildStderr {
    fn from_inner(pipe: imp::ChildPipe) -> ChildStderr {
        ChildStderr { inner: pipe }
    }
}

#[stable(feature = "std_debug", since = "1.16.0")]
impl fmt::Debug for ChildStderr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ChildStderr").finish_non_exhaustive()
    }
}

/// 一个进程 builder，提供对新进程应如何被 spawn 的细粒度控制。
///
/// 默认配置可以使用 `Command::new(program)` 生成，其中 `program` 给出待执行程序的
/// 路径。额外的 builder 方法允许在 spawn 之前修改该配置（例如添加参数）：
///
/// ```
/// # if cfg!(not(all(target_vendor = "apple", not(target_os = "macos")))) {
/// use std::process::Command;
///
/// let output = if cfg!(target_os = "windows") {
///     Command::new("cmd")
///         .args(["/C", "echo hello"])
///         .output()
///         .expect("failed to execute process")
/// } else {
///     Command::new("sh")
///         .arg("-c")
///         .arg("echo hello")
///         .output()
///         .expect("failed to execute process")
/// };
///
/// let hello = output.stdout;
/// # }
/// ```
///
/// `Command` 可以被复用以 spawn 多个进程。这些 builder 方法只修改命令，而无需立即
/// spawn 进程。
///
/// ```no_run
/// use std::process::Command;
///
/// let mut echo_hello = Command::new("sh");
/// echo_hello.arg("-c").arg("echo hello");
/// let hello_1 = echo_hello.output().expect("failed to execute process");
/// let hello_2 = echo_hello.output().expect("failed to execute process");
/// ```
///
/// 类似地，你可以在 spawn 一个进程之后调用 builder 方法，然后以修改后的设置 spawn
/// 一个新进程。
///
/// ```no_run
/// use std::process::Command;
///
/// let mut list_dir = Command::new("ls");
///
/// // 在程序的当前目录下执行 `ls`。
/// list_dir.status().expect("process failed to execute");
///
/// println!();
///
/// // 修改 `ls`，使其在根目录下执行。
/// list_dir.current_dir("/");
///
/// // 然后在根目录下再次执行 `ls`。
/// list_dir.status().expect("process failed to execute");
/// ```
#[stable(feature = "process", since = "1.0.0")]
#[cfg_attr(not(test), rustc_diagnostic_item = "Command")]
pub struct Command {
    inner: imp::Command,
}

/// 允许在 `std` 内部定义扩展 trait。
#[unstable(feature = "sealed", issue = "none")]
impl crate::sealed::Sealed for Command {}

impl Command {
    /// 构造一个新的 `Command`，用于启动位于路径 `program` 处的程序，采用以下默认配置：
    ///
    /// * 不向程序传递任何参数
    /// * 继承当前进程的环境变量
    /// * 继承当前进程的工作目录
    /// * 对 [`spawn`] 或 [`status`] 继承 stdin/stdout/stderr，但对 [`output`] 创建管道
    ///
    /// [`spawn`]: Self::spawn
    /// [`status`]: Self::status
    /// [`output`]: Self::output
    ///
    /// 提供了若干 builder 方法用于修改这些默认值并对进程进行其他配置。
    ///
    /// 如果 `program` 不是绝对路径，则会以某种由 OS 定义的方式搜索 `PATH`。
    ///
    /// 用于搜索的路径可以通过在该 Command 上设置 `PATH` 环境变量来控制，但这在
    /// Windows 上有一些实现上的限制（见 issue #37519）。
    ///
    /// # 平台特定行为
    ///
    /// Windows 上的注意事项：对于带 .exe 扩展名的可执行文件，在为该 Command 指定
    /// 程序时可以省略该扩展名。然而，如果文件具有不同的扩展名，则需要提供包含扩展名
    /// 的文件名，否则该文件将无法被找到。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::process::Command;
    ///
    /// Command::new("sh")
    ///     .spawn()
    ///     .expect("sh command failed to start");
    /// ```
    ///
    /// # 注意事项（Caveats）
    ///
    /// [`Command::new`] 只接受程序的路径。如果你像 `Command::new("ls -l").spawn()`
    /// 这样在程序路径中附带了参数，它会按字面去搜索 `ls -l`。参数需要单独传递，例如
    /// 通过 [`arg`] 或 [`args`]。
    ///
    /// ```no_run
    /// use std::process::Command;
    ///
    /// Command::new("ls")
    ///     .arg("-l") // 参数单独传递
    ///     .spawn()
    ///     .expect("ls command failed to start");
    /// ```
    ///
    /// [`arg`]: Self::arg
    /// [`args`]: Self::args
    #[stable(feature = "process", since = "1.0.0")]
    pub fn new<S: AsRef<OsStr>>(program: S) -> Command {
        Command { inner: imp::Command::new(program.as_ref()) }
    }

    /// 添加一个要传给程序的参数。
    ///
    /// 每次调用只能传入一个参数。因此，不要写成：
    ///
    /// ```no_run
    /// # std::process::Command::new("sh")
    /// .arg("-C /path/to/repo")
    /// # ;
    /// ```
    ///
    /// 而应写成：
    ///
    /// ```no_run
    /// # std::process::Command::new("sh")
    /// .arg("-C")
    /// .arg("/path/to/repo")
    /// # ;
    /// ```
    ///
    /// 要一次传入多个参数，见 [`args`]。
    ///
    /// [`args`]: Command::args
    ///
    /// 注意参数不会经过 shell，而是按字面直接传给程序。这意味着诸如引号、转义字符、
    /// 单词拆分、glob 模式、变量替换等 shell 语法都不会生效。
    ///
    /// <div class="warning">
    ///
    /// 在 Windows 上，对不可信输入要格外小心。大多数应用程序使用解码传入参数的标准
    /// 约定，这些应用与 `arg` 一起使用是安全的。然而，有些应用程序（例如 `cmd.exe`
    /// 和 `.bat` 文件）使用非标准的方式解码参数。因此它们容易受到恶意输入的攻击。
    ///
    /// 就 `cmd.exe` 而言这一点尤其重要，因为一个恶意参数有可能运行任意 shell 命令。
    ///
    /// 更多细节见 [Windows argument splitting][windows-args]，或见 [`raw_arg`] 以手动
    /// 实现非标准的参数编码。
    ///
    /// [`raw_arg`]: crate::os::windows::process::CommandExt::raw_arg
    /// [windows-args]: crate::process#windows-argument-splitting
    ///
    /// </div>
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::process::Command;
    ///
    /// Command::new("ls")
    ///     .arg("-l")
    ///     .arg("-a")
    ///     .spawn()
    ///     .expect("ls command failed to start");
    /// ```
    #[stable(feature = "process", since = "1.0.0")]
    pub fn arg<S: AsRef<OsStr>>(&mut self, arg: S) -> &mut Command {
        self.inner.arg(arg.as_ref());
        self
    }

    /// 添加多个要传给程序的参数。
    ///
    /// 要传入单个参数，见 [`arg`]。
    ///
    /// [`arg`]: Command::arg
    ///
    /// 注意参数不会经过 shell，而是按字面直接传给程序。这意味着诸如引号、转义字符、
    /// 单词拆分、glob 模式、变量替换等 shell 语法都不会生效。
    ///
    /// <div class="warning">
    ///
    /// 在 Windows 上，对不可信输入要格外小心。大多数应用程序使用解码传入参数的标准
    /// 约定，这些应用与 `arg` 一起使用是安全的。然而，有些应用程序（例如 `cmd.exe`
    /// 和 `.bat` 文件）使用非标准的方式解码参数。因此它们容易受到恶意输入的攻击。
    ///
    /// 就 `cmd.exe` 而言这一点尤其重要，因为一个恶意参数有可能运行任意 shell 命令。
    ///
    /// 更多细节见 [Windows argument splitting][windows-args]，或见 [`raw_arg`] 以手动
    /// 实现非标准的参数编码。
    ///
    /// [`raw_arg`]: crate::os::windows::process::CommandExt::raw_arg
    /// [windows-args]: crate::process#windows-argument-splitting
    ///
    /// </div>
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::process::Command;
    ///
    /// Command::new("ls")
    ///     .args(["-l", "-a"])
    ///     .spawn()
    ///     .expect("ls command failed to start");
    /// ```
    #[stable(feature = "process", since = "1.0.0")]
    pub fn args<I, S>(&mut self, args: I) -> &mut Command
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        for arg in args {
            self.arg(arg.as_ref());
        }
        self
    }

    /// 插入或更新一个显式的环境变量映射。
    ///
    /// 此方法允许你向被 spawn 的进程添加一个环境变量映射，或覆盖先前设置的值。你可以
    /// 使用 [`Command::envs`] 同时设置多个环境变量。
    ///
    /// 子进程默认会从其父进程继承环境变量。使用 [`Command::env`] 显式设置的环境变量
    /// 优先于继承来的变量。你可以使用 [`Command::env_clear`] 完全禁用环境变量继承，
    /// 或使用 [`Command::env_remove`] 针对单个键禁用继承。
    ///
    /// 注意环境变量名在 Windows 上大小写不敏感（但保留大小写），而在其他所有平台上
    /// 大小写敏感。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::process::Command;
    ///
    /// Command::new("ls")
    ///     .env("PATH", "/bin")
    ///     .spawn()
    ///     .expect("ls command failed to start");
    /// ```
    #[stable(feature = "process", since = "1.0.0")]
    pub fn env<K, V>(&mut self, key: K, val: V) -> &mut Command
    where
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        self.inner.env_mut().set(key.as_ref(), val.as_ref());
        self
    }

    /// 插入或更新多个显式的环境变量映射。
    ///
    /// 此方法允许你向被 spawn 的进程添加多个环境变量映射，或覆盖先前设置的值。你可以
    /// 使用 [`Command::env`] 设置单个环境变量。
    ///
    /// 子进程默认会从其父进程继承环境变量。使用 [`Command::envs`] 显式设置的环境变量
    /// 优先于继承来的变量。你可以使用 [`Command::env_clear`] 完全禁用环境变量继承，
    /// 或使用 [`Command::env_remove`] 针对单个键禁用继承。
    ///
    /// 注意环境变量名在 Windows 上大小写不敏感（但保留大小写），而在其他所有平台上
    /// 大小写敏感。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::process::{Command, Stdio};
    /// use std::env;
    /// use std::collections::HashMap;
    ///
    /// let filtered_env : HashMap<String, String> =
    ///     env::vars().filter(|&(ref k, _)|
    ///         k == "TERM" || k == "TZ" || k == "LANG" || k == "PATH"
    ///     ).collect();
    ///
    /// Command::new("printenv")
    ///     .stdin(Stdio::null())
    ///     .stdout(Stdio::inherit())
    ///     .env_clear()
    ///     .envs(&filtered_env)
    ///     .spawn()
    ///     .expect("printenv failed to start");
    /// ```
    #[stable(feature = "command_envs", since = "1.19.0")]
    pub fn envs<I, K, V>(&mut self, vars: I) -> &mut Command
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<OsStr>,
        V: AsRef<OsStr>,
    {
        for (ref key, ref val) in vars {
            self.inner.env_mut().set(key.as_ref(), val.as_ref());
        }
        self
    }

    /// 移除一个显式设置的环境变量，并阻止从父进程继承它。
    ///
    /// 此方法会移除通过 [`Command::env`] 或 [`Command::envs`] 设置的某个环境变量的
    /// 显式值。此外，它还会阻止被 spawn 的子进程从其父进程继承该环境变量。
    ///
    /// 调用 [`Command::env_remove`] 之后，[`Command::get_envs`] 中与其键关联的值将为
    /// [`None`]。
    ///
    /// 要清除所有显式设置的环境变量并禁用所有环境变量继承，可以使用
    /// [`Command::env_clear`]。
    ///
    /// # 示例
    ///
    /// 阻止任何继承来的 `GIT_DIR` 变量改变 `git` 命令的目标，同时允许所有其他变量，
    /// 例如 `GIT_AUTHOR_NAME`。
    ///
    /// ```no_run
    /// use std::process::Command;
    ///
    /// Command::new("git")
    ///     .arg("commit")
    ///     .env_remove("GIT_DIR")
    ///     .spawn()?;
    /// # std::io::Result::Ok(())
    /// ```
    #[stable(feature = "process", since = "1.0.0")]
    pub fn env_remove<K: AsRef<OsStr>>(&mut self, key: K) -> &mut Command {
        self.inner.env_mut().remove(key.as_ref());
        self
    }

    /// 清除所有显式设置的环境变量，并阻止继承任何父进程的环境变量。
    ///
    /// 此方法会移除所有通过 [`Command::env`] 或 [`Command::envs`] 显式添加的环境变量。
    /// 此外，它还会阻止被 spawn 的子进程从其父进程继承任何环境变量。
    ///
    /// 调用 [`Command::env_clear`] 之后，[`Command::get_envs`] 返回的迭代器将为空。
    ///
    /// 你可以使用 [`Command::env_remove`] 清除单个映射。
    ///
    /// # 示例
    ///
    /// `sort` 的行为会受 `LANG` 和 `LC_*` 环境变量的影响。清除环境会使 `sort` 的行为
    /// 与父进程的语言设置无关。
    ///
    /// ```no_run
    /// use std::process::Command;
    ///
    /// Command::new("sort")
    ///     .arg("file.txt")
    ///     .env_clear()
    ///     .spawn()?;
    /// # std::io::Result::Ok(())
    /// ```
    #[stable(feature = "process", since = "1.0.0")]
    pub fn env_clear(&mut self) -> &mut Command {
        self.inner.env_mut().clear();
        self
    }

    /// 设置子进程的工作目录。
    ///
    /// # 平台特定行为
    ///
    /// 如果程序路径是相对的（例如 `"./script.sh"`），那么它究竟应当被解释为相对于
    /// 父进程的工作目录、还是相对于 `current_dir`，是有歧义的。这种情况下的行为是
    /// 平台特定且不稳定的，建议改用 [`canonicalize`] 获取绝对的程序路径。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::process::Command;
    ///
    /// Command::new("ls")
    ///     .current_dir("/bin")
    ///     .spawn()
    ///     .expect("ls command failed to start");
    /// ```
    ///
    /// [`canonicalize`]: crate::fs::canonicalize
    #[stable(feature = "process", since = "1.0.0")]
    pub fn current_dir<P: AsRef<Path>>(&mut self, dir: P) -> &mut Command {
        self.inner.cwd(dir.as_ref().as_ref());
        self
    }

    /// 配置子进程标准输入（stdin）的句柄。
    ///
    /// 与 [`spawn`] 或 [`status`] 一起使用时默认为 [`inherit`]，与 [`output`] 一起
    /// 使用时默认为 [`piped`]。
    ///
    /// [`inherit`]: Stdio::inherit
    /// [`piped`]: Stdio::piped
    /// [`spawn`]: Self::spawn
    /// [`status`]: Self::status
    /// [`output`]: Self::output
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::process::{Command, Stdio};
    ///
    /// Command::new("ls")
    ///     .stdin(Stdio::null())
    ///     .spawn()
    ///     .expect("ls command failed to start");
    /// ```
    #[stable(feature = "process", since = "1.0.0")]
    pub fn stdin<T: Into<Stdio>>(&mut self, cfg: T) -> &mut Command {
        self.inner.stdin(cfg.into().0);
        self
    }

    /// 配置子进程标准输出（stdout）的句柄。
    ///
    /// 与 [`spawn`] 或 [`status`] 一起使用时默认为 [`inherit`]，与 [`output`] 一起
    /// 使用时默认为 [`piped`]。
    ///
    /// [`inherit`]: Stdio::inherit
    /// [`piped`]: Stdio::piped
    /// [`spawn`]: Self::spawn
    /// [`status`]: Self::status
    /// [`output`]: Self::output
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::process::{Command, Stdio};
    ///
    /// Command::new("ls")
    ///     .stdout(Stdio::null())
    ///     .spawn()
    ///     .expect("ls command failed to start");
    /// ```
    #[stable(feature = "process", since = "1.0.0")]
    pub fn stdout<T: Into<Stdio>>(&mut self, cfg: T) -> &mut Command {
        self.inner.stdout(cfg.into().0);
        self
    }

    /// 配置子进程标准错误（stderr）的句柄。
    ///
    /// 与 [`spawn`] 或 [`status`] 一起使用时默认为 [`inherit`]，与 [`output`] 一起
    /// 使用时默认为 [`piped`]。
    ///
    /// [`inherit`]: Stdio::inherit
    /// [`piped`]: Stdio::piped
    /// [`spawn`]: Self::spawn
    /// [`status`]: Self::status
    /// [`output`]: Self::output
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::process::{Command, Stdio};
    ///
    /// Command::new("ls")
    ///     .stderr(Stdio::null())
    ///     .spawn()
    ///     .expect("ls command failed to start");
    /// ```
    #[stable(feature = "process", since = "1.0.0")]
    pub fn stderr<T: Into<Stdio>>(&mut self, cfg: T) -> &mut Command {
        self.inner.stderr(cfg.into().0);
        self
    }

    /// 将该命令作为子进程执行，返回指向它的句柄。
    ///
    /// 默认情况下，stdin、stdout 和 stderr 都从父进程继承。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::process::Command;
    ///
    /// Command::new("ls")
    ///     .spawn()
    ///     .expect("ls command failed to start");
    /// ```
    #[stable(feature = "process", since = "1.0.0")]
    pub fn spawn(&mut self) -> io::Result<Child> {
        self.inner.spawn(imp::Stdio::Inherit, true).map(Child::from_inner)
    }

    /// 将该命令作为子进程执行，等待它结束，并收集它的所有输出。
    ///
    /// 默认情况下，stdout 和 stderr 会被捕获（并用于提供返回的输出）。stdin 不从父进程
    /// 继承，子进程任何从 stdin 流读取的尝试都会导致该流立即关闭。
    ///
    /// # 示例
    ///
    /// ```should_panic
    /// use std::process::Command;
    /// use std::io::{self, Write};
    /// let output = Command::new("/bin/cat")
    ///     .arg("file.txt")
    ///     .output()?;
    ///
    /// println!("status: {}", output.status);
    /// io::stdout().write_all(&output.stdout)?;
    /// io::stderr().write_all(&output.stderr)?;
    ///
    /// assert!(output.status.success());
    /// # io::Result::Ok(())
    /// ```
    #[stable(feature = "process", since = "1.0.0")]
    pub fn output(&mut self) -> io::Result<Output> {
        let (status, stdout, stderr) = imp::output(&mut self.inner)?;
        Ok(Output { status: ExitStatus(status), stdout, stderr })
    }

    /// 将命令作为子进程执行，等待它结束，并收集它的状态。
    ///
    /// 默认情况下，stdin、stdout 和 stderr 都从父进程继承。
    ///
    /// # 示例
    ///
    /// ```should_panic
    /// use std::process::Command;
    ///
    /// let status = Command::new("/bin/cat")
    ///     .arg("file.txt")
    ///     .status()
    ///     .expect("failed to execute process");
    ///
    /// println!("process finished with: {status}");
    ///
    /// assert!(status.success());
    /// ```
    #[stable(feature = "process", since = "1.0.0")]
    pub fn status(&mut self) -> io::Result<ExitStatus> {
        self.inner
            .spawn(imp::Stdio::Inherit, true)
            .map(Child::from_inner)
            .and_then(|mut p| p.wait())
    }

    /// 返回传给 [`Command::new`] 的程序路径。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::process::Command;
    ///
    /// let cmd = Command::new("echo");
    /// assert_eq!(cmd.get_program(), "echo");
    /// ```
    #[must_use]
    #[stable(feature = "command_access", since = "1.57.0")]
    pub fn get_program(&self) -> &OsStr {
        self.inner.get_program()
    }

    /// 返回一个迭代器，遍历将传给程序的参数。
    ///
    /// 这不包括作为第一个参数的程序路径；它只包括通过 [`Command::arg`] 和
    /// [`Command::args`] 指定的参数。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::ffi::OsStr;
    /// use std::process::Command;
    ///
    /// let mut cmd = Command::new("echo");
    /// cmd.arg("first").arg("second");
    /// let args: Vec<&OsStr> = cmd.get_args().collect();
    /// assert_eq!(args, &["first", "second"]);
    /// ```
    #[stable(feature = "command_access", since = "1.57.0")]
    pub fn get_args(&self) -> CommandArgs<'_> {
        CommandArgs { inner: self.inner.get_args() }
    }

    /// 返回一个迭代器，遍历为子进程显式设置的环境变量。
    ///
    /// 通过 [`Command::env`]、[`Command::envs`] 和 [`Command::env_remove`] 显式设置的
    /// 环境变量都可以用此方法获取。
    ///
    /// 注意此输出不包括从父进程继承的环境变量。
    ///
    /// 每个元素都是一个键/值对元组 `(&OsStr, Option<&OsStr>)`。[`None`] 值表示其键
    /// 被通过 [`Command::env_remove`] 显式移除了。与该 [`None`] 值关联的键将不再从其
    /// 父进程继承。
    ///
    /// 空迭代器可能表示没有添加任何显式映射，或表示调用过 [`Command::env_clear`]。
    /// 调用 [`Command::env_clear`] 之后，子进程将不会从其父进程继承任何环境变量。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::ffi::OsStr;
    /// use std::process::Command;
    ///
    /// let mut cmd = Command::new("ls");
    /// cmd.env("TERM", "dumb").env_remove("TZ");
    /// let envs: Vec<(&OsStr, Option<&OsStr>)> = cmd.get_envs().collect();
    /// assert_eq!(envs, &[
    ///     (OsStr::new("TERM"), Some(OsStr::new("dumb"))),
    ///     (OsStr::new("TZ"), None)
    /// ]);
    /// ```
    #[stable(feature = "command_access", since = "1.57.0")]
    pub fn get_envs(&self) -> CommandEnvs<'_> {
        CommandEnvs { iter: self.inner.get_envs() }
    }

    /// 返回子进程的工作目录。
    ///
    /// 如果工作目录不会被改变，则返回 [`None`]。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::path::Path;
    /// use std::process::Command;
    ///
    /// let mut cmd = Command::new("ls");
    /// assert_eq!(cmd.get_current_dir(), None);
    /// cmd.current_dir("/bin");
    /// assert_eq!(cmd.get_current_dir(), Some(Path::new("/bin")));
    /// ```
    #[must_use]
    #[stable(feature = "command_access", since = "1.57.0")]
    pub fn get_current_dir(&self) -> Option<&Path> {
        self.inner.get_current_dir()
    }

    /// 返回是否将为子进程清除环境变量。
    ///
    /// 如果调用过 [`Command::env_clear`]，则返回 `true`，否则返回 `false`。当为
    /// `true` 时，子进程将不会从其父进程继承任何环境变量。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(command_resolved_envs)]
    /// use std::process::Command;
    ///
    /// let mut cmd = Command::new("ls");
    /// assert_eq!(cmd.get_env_clear(), false);
    ///
    /// cmd.env_clear();
    /// assert_eq!(cmd.get_env_clear(), true);
    /// ```
    #[must_use]
    #[unstable(feature = "command_resolved_envs", issue = "149070")]
    pub fn get_env_clear(&self) -> bool {
        self.inner.get_env_clear()
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl fmt::Debug for Command {
    /// 格式化一个 Command 的程序和参数以供显示。任何非 utf8 数据都会使用 utf8
    /// 替换字符进行有损转换。
    ///
    /// 默认格式近似于该程序连同其参数的一次 shell 调用。它不包含大多数其他命令属性。
    /// 输出不保证能正常工作（例如由于缺少 shell 转义或路径解析方式的差异）。在某些
    /// 平台上你可以使用[备用语法][the alternate syntax]来显示更多字段。
    ///
    /// 注意该 debug 实现是平台特定的。
    ///
    /// [the alternate syntax]: fmt#sign0
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}

impl AsInner<imp::Command> for Command {
    #[inline]
    fn as_inner(&self) -> &imp::Command {
        &self.inner
    }
}

impl AsInnerMut<imp::Command> for Command {
    #[inline]
    fn as_inner_mut(&mut self) -> &mut imp::Command {
        &mut self.inner
    }
}

/// 一个遍历命令参数的迭代器。
///
/// 该结构体由 [`Command::get_args`] 创建。更多信息见其文档。
#[must_use = "iterators are lazy and do nothing unless consumed"]
#[stable(feature = "command_access", since = "1.57.0")]
#[derive(Debug)]
pub struct CommandArgs<'a> {
    inner: imp::CommandArgs<'a>,
}

#[stable(feature = "command_access", since = "1.57.0")]
impl<'a> Iterator for CommandArgs<'a> {
    type Item = &'a OsStr;
    fn next(&mut self) -> Option<&'a OsStr> {
        self.inner.next()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

#[stable(feature = "command_access", since = "1.57.0")]
impl<'a> ExactSizeIterator for CommandArgs<'a> {
    fn len(&self) -> usize {
        self.inner.len()
    }
    fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

/// 一个遍历命令环境变量的迭代器。
///
/// 该结构体由 [`Command::get_envs`][crate::process::Command::get_envs] 创建。更多
/// 信息见其文档。
#[must_use = "iterators are lazy and do nothing unless consumed"]
#[stable(feature = "command_access", since = "1.57.0")]
pub struct CommandEnvs<'a> {
    iter: imp::CommandEnvs<'a>,
}

#[stable(feature = "command_access", since = "1.57.0")]
impl<'a> Iterator for CommandEnvs<'a> {
    type Item = (&'a OsStr, Option<&'a OsStr>);

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

#[stable(feature = "command_access", since = "1.57.0")]
impl<'a> ExactSizeIterator for CommandEnvs<'a> {
    fn len(&self) -> usize {
        self.iter.len()
    }

    fn is_empty(&self) -> bool {
        self.iter.is_empty()
    }
}

#[stable(feature = "command_access", since = "1.57.0")]
impl<'a> fmt::Debug for CommandEnvs<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.iter.fmt(f)
    }
}

/// 一个已结束进程的输出。
///
/// 它由 [`Command`] 的 [`output`] 方法或 [`Child`] 进程的 [`wait_with_output`]
/// 方法以 Result 的形式返回。
///
/// [`output`]: Command::output
/// [`wait_with_output`]: Child::wait_with_output
#[derive(PartialEq, Eq, Clone)]
#[stable(feature = "process", since = "1.0.0")]
pub struct Output {
    /// 进程的状态（退出码）。
    #[stable(feature = "process", since = "1.0.0")]
    pub status: ExitStatus,
    /// 进程写入 stdout 的数据。
    #[stable(feature = "process", since = "1.0.0")]
    pub stdout: Vec<u8>,
    /// 进程写入 stderr 的数据。
    #[stable(feature = "process", since = "1.0.0")]
    pub stderr: Vec<u8>,
}

impl Output {
    /// 如果收到了非零退出状态，则返回一个错误。
    ///
    /// 如果该 [`Command`] 成功退出，则返回 `self`。
    ///
    /// 这等价于在 [`Output.status`](Output::status) 上调用
    /// [`exit_ok`](ExitStatus::exit_ok)。
    ///
    /// 注意在错误情况下这会丢弃 [`Output::stderr`] 字段。如果子进程向 stderr 输出了
    /// 有用的信息，你可以：
    /// * 使用 `cmd.stderr(Stdio::inherit())` 把子进程的 stderr 转发到父进程的 stderr，
    ///   通常会将其打印到用户能看到的控制台。这对命令行应用程序通常是正确的做法。
    /// * 使用自定义错误类型来捕获 `stderr`。这对库来说通常是正确的做法。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(exit_status_error)]
    /// # #[cfg(all(unix, not(target_os = "android"), not(all(target_vendor = "apple", not(target_os = "macos")))))] {
    /// use std::process::Command;
    /// assert!(Command::new("false").output().unwrap().exit_ok().is_err());
    /// # }
    /// ```
    #[unstable(feature = "exit_status_error", issue = "84908")]
    pub fn exit_ok(self) -> Result<Self, ExitStatusError> {
        self.status.exit_ok()?;
        Ok(self)
    }
}

// 如果 stderr 或 stdout 是有效的 utf8 字符串，就打印这些有效字符串，否则改为打印
// 其字节序列
#[stable(feature = "process_output_debug", since = "1.7.0")]
impl fmt::Debug for Output {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        let stdout_utf8 = str::from_utf8(&self.stdout);
        let stdout_debug: &dyn fmt::Debug = match stdout_utf8 {
            Ok(ref s) => s,
            Err(_) => &self.stdout,
        };

        let stderr_utf8 = str::from_utf8(&self.stderr);
        let stderr_debug: &dyn fmt::Debug = match stderr_utf8 {
            Ok(ref s) => s,
            Err(_) => &self.stderr,
        };

        fmt.debug_struct("Output")
            .field("status", &self.status)
            .field("stdout", stdout_debug)
            .field("stderr", stderr_debug)
            .finish()
    }
}

/// 描述当传给 [`Command`] 的 [`stdin`]、[`stdout`] 和 [`stderr`] 方法时，对子进程的
/// 某个标准 I/O 流要做什么。
///
/// [`stdin`]: Command::stdin
/// [`stdout`]: Command::stdout
/// [`stderr`]: Command::stderr
#[stable(feature = "process", since = "1.0.0")]
pub struct Stdio(imp::Stdio);

impl Stdio {
    /// 应当安排一个新管道来连接父进程和子进程。
    ///
    /// # 示例
    ///
    /// 配合 stdout：
    ///
    /// ```no_run
    /// use std::process::{Command, Stdio};
    ///
    /// let output = Command::new("echo")
    ///     .arg("Hello, world!")
    ///     .stdout(Stdio::piped())
    ///     .output()
    ///     .expect("Failed to execute command");
    ///
    /// assert_eq!(String::from_utf8_lossy(&output.stdout), "Hello, world!\n");
    /// // 控制台上不会回显任何内容
    /// ```
    ///
    /// 配合 stdin：
    ///
    /// ```no_run
    /// use std::io::Write;
    /// use std::process::{Command, Stdio};
    ///
    /// let mut child = Command::new("rev")
    ///     .stdin(Stdio::piped())
    ///     .stdout(Stdio::piped())
    ///     .spawn()
    ///     .expect("Failed to spawn child process");
    ///
    /// let mut stdin = child.stdin.take().expect("Failed to open stdin");
    /// std::thread::spawn(move || {
    ///     stdin.write_all("Hello, world!".as_bytes()).expect("Failed to write to stdin");
    /// });
    ///
    /// let output = child.wait_with_output().expect("Failed to read stdout");
    /// assert_eq!(String::from_utf8_lossy(&output.stdout), "!dlrow ,olleH");
    /// ```
    ///
    /// 在不同时读取 stdout 和 stderr 的情况下，向 stdin 写入超过一个管道缓冲区大小的
    /// 输入可能造成死锁。这在运行任何不保证在写入超过一个管道缓冲区大小的输出之前
    /// 就读完其全部 stdin 的程序时是一个问题。管道缓冲区的大小因目标平台而异。
    ///
    #[must_use]
    #[stable(feature = "process", since = "1.0.0")]
    pub fn piped() -> Stdio {
        Stdio(imp::Stdio::MakePipe)
    }

    /// 子进程从父进程对应的描述符继承。
    ///
    /// # 示例
    ///
    /// 配合 stdout：
    ///
    /// ```no_run
    /// use std::process::{Command, Stdio};
    ///
    /// let output = Command::new("echo")
    ///     .arg("Hello, world!")
    ///     .stdout(Stdio::inherit())
    ///     .output()
    ///     .expect("Failed to execute command");
    ///
    /// assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    /// // "Hello, world!" 被回显到控制台
    /// ```
    ///
    /// 配合 stdin：
    ///
    /// ```no_run
    /// use std::process::{Command, Stdio};
    /// use std::io::{self, Write};
    ///
    /// let output = Command::new("rev")
    ///     .stdin(Stdio::inherit())
    ///     .stdout(Stdio::piped())
    ///     .output()?;
    ///
    /// print!("You piped in the reverse of: ");
    /// io::stdout().write_all(&output.stdout)?;
    /// # io::Result::Ok(())
    /// ```
    #[must_use]
    #[stable(feature = "process", since = "1.0.0")]
    pub fn inherit() -> Stdio {
        Stdio(imp::Stdio::Inherit)
    }

    /// 该流将被忽略。这等价于把该流连接到 `/dev/null`。
    ///
    /// # 示例
    ///
    /// 配合 stdout：
    ///
    /// ```no_run
    /// use std::process::{Command, Stdio};
    ///
    /// let output = Command::new("echo")
    ///     .arg("Hello, world!")
    ///     .stdout(Stdio::null())
    ///     .output()
    ///     .expect("Failed to execute command");
    ///
    /// assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    /// // 控制台上不会回显任何内容
    /// ```
    ///
    /// 配合 stdin：
    ///
    /// ```no_run
    /// use std::process::{Command, Stdio};
    ///
    /// let output = Command::new("rev")
    ///     .stdin(Stdio::null())
    ///     .stdout(Stdio::piped())
    ///     .output()
    ///     .expect("Failed to execute command");
    ///
    /// assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    /// // 忽略任何通过管道传入的输入
    /// ```
    #[must_use]
    #[stable(feature = "process", since = "1.0.0")]
    pub fn null() -> Stdio {
        Stdio(imp::Stdio::Null)
    }

    /// 如果这需要 [`Command`] 创建一个新管道，则返回 `true`。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(stdio_makes_pipe)]
    /// use std::process::Stdio;
    ///
    /// let io = Stdio::piped();
    /// assert_eq!(io.makes_pipe(), true);
    /// ```
    #[unstable(feature = "stdio_makes_pipe", issue = "98288")]
    pub fn makes_pipe(&self) -> bool {
        matches!(self.0, imp::Stdio::MakePipe)
    }
}

impl FromInner<imp::Stdio> for Stdio {
    fn from_inner(inner: imp::Stdio) -> Stdio {
        Stdio(inner)
    }
}

#[stable(feature = "std_debug", since = "1.16.0")]
impl fmt::Debug for Stdio {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Stdio").finish_non_exhaustive()
    }
}

#[stable(feature = "stdio_from", since = "1.20.0")]
impl From<ChildStdin> for Stdio {
    /// 将一个 [`ChildStdin`] 转换为 [`Stdio`]。
    ///
    /// # 示例
    ///
    /// `ChildStdin` 在底层会通过 `Stdio::from` 被转换为 `Stdio`。
    ///
    /// ```rust,no_run
    /// use std::process::{Command, Stdio};
    ///
    /// let reverse = Command::new("rev")
    ///     .stdin(Stdio::piped())
    ///     .spawn()
    ///     .expect("failed reverse command");
    ///
    /// let _echo = Command::new("echo")
    ///     .arg("Hello, world!")
    ///     .stdout(reverse.stdin.unwrap()) // 在此处被转换为 Stdio
    ///     .output()
    ///     .expect("failed echo command");
    ///
    /// // "!dlrow ,olleH" 被回显到控制台
    /// ```
    fn from(child: ChildStdin) -> Stdio {
        Stdio::from_inner(child.into_inner().into())
    }
}

#[stable(feature = "stdio_from", since = "1.20.0")]
impl From<ChildStdout> for Stdio {
    /// 将一个 [`ChildStdout`] 转换为 [`Stdio`]。
    ///
    /// # 示例
    ///
    /// `ChildStdout` 在底层会通过 `Stdio::from` 被转换为 `Stdio`。
    ///
    /// ```rust,no_run
    /// use std::process::{Command, Stdio};
    ///
    /// let hello = Command::new("echo")
    ///     .arg("Hello, world!")
    ///     .stdout(Stdio::piped())
    ///     .spawn()
    ///     .expect("failed echo command");
    ///
    /// let reverse = Command::new("rev")
    ///     .stdin(hello.stdout.unwrap())  // 在此处被转换为 Stdio
    ///     .output()
    ///     .expect("failed reverse command");
    ///
    /// assert_eq!(reverse.stdout, b"!dlrow ,olleH\n");
    /// ```
    fn from(child: ChildStdout) -> Stdio {
        Stdio::from_inner(child.into_inner().into())
    }
}

#[stable(feature = "stdio_from", since = "1.20.0")]
impl From<ChildStderr> for Stdio {
    /// 将一个 [`ChildStderr`] 转换为 [`Stdio`]。
    ///
    /// # 示例
    ///
    /// ```rust,no_run
    /// use std::process::{Command, Stdio};
    ///
    /// let reverse = Command::new("rev")
    ///     .arg("non_existing_file.txt")
    ///     .stderr(Stdio::piped())
    ///     .spawn()
    ///     .expect("failed reverse command");
    ///
    /// let cat = Command::new("cat")
    ///     .arg("-")
    ///     .stdin(reverse.stderr.unwrap()) // 在此处被转换为 Stdio
    ///     .output()
    ///     .expect("failed echo command");
    ///
    /// assert_eq!(
    ///     String::from_utf8_lossy(&cat.stdout),
    ///     "rev: cannot open non_existing_file.txt: No such file or directory\n"
    /// );
    /// ```
    fn from(child: ChildStderr) -> Stdio {
        Stdio::from_inner(child.into_inner().into())
    }
}

#[stable(feature = "stdio_from", since = "1.20.0")]
impl From<fs::File> for Stdio {
    /// 将一个 [`File`](fs::File) 转换为 [`Stdio`]。
    ///
    /// # 示例
    ///
    /// `File` 在底层会通过 `Stdio::from` 被转换为 `Stdio`。
    ///
    /// ```rust,no_run
    /// use std::fs::File;
    /// use std::process::Command;
    ///
    /// // `foo.txt` 文件中包含 "Hello, world!"
    /// let file = File::open("foo.txt")?;
    ///
    /// let reverse = Command::new("rev")
    ///     .stdin(file)  // File 隐式转换为 Stdio
    ///     .output()?;
    ///
    /// assert_eq!(reverse.stdout, b"!dlrow ,olleH");
    /// # std::io::Result::Ok(())
    /// ```
    fn from(file: fs::File) -> Stdio {
        Stdio::from_inner(file.into_inner().into())
    }
}

#[stable(feature = "stdio_from_stdio", since = "1.74.0")]
impl From<io::Stdout> for Stdio {
    /// 将命令的 stdout/stderr 重定向到我们的 stdout
    ///
    /// # 示例
    ///
    /// ```rust
    /// #![feature(exit_status_error)]
    /// use std::io;
    /// use std::process::Command;
    ///
    /// # fn test() -> Result<(), Box<dyn std::error::Error>> {
    /// let output = Command::new("whoami")
    // "whoami" 是一个在 Unix 和 Windows 上都存在的命令，
    // 并且会成功执行，产生一些 stdout 输出但没有 stderr。
    ///     .stdout(io::stdout())
    ///     .output()?;
    /// output.status.exit_ok()?;
    /// assert!(output.stdout.is_empty());
    /// # Ok(())
    /// # }
    /// #
    /// # if cfg!(all(unix, not(target_os = "android"), not(all(target_vendor = "apple", not(target_os = "macos"))))) {
    /// #     test().unwrap();
    /// # }
    /// ```
    fn from(inherit: io::Stdout) -> Stdio {
        Stdio::from_inner(inherit.into())
    }
}

#[stable(feature = "stdio_from_stdio", since = "1.74.0")]
impl From<io::Stderr> for Stdio {
    /// 将命令的 stdout/stderr 重定向到我们的 stderr
    ///
    /// # 示例
    ///
    /// ```rust
    /// #![feature(exit_status_error)]
    /// use std::io;
    /// use std::process::Command;
    ///
    /// # fn test() -> Result<(), Box<dyn std::error::Error>> {
    /// let output = Command::new("whoami")
    ///     .stdout(io::stderr())
    ///     .output()?;
    /// output.status.exit_ok()?;
    /// assert!(output.stdout.is_empty());
    /// # Ok(())
    /// # }
    /// #
    /// # if cfg!(all(unix, not(target_os = "android"), not(all(target_vendor = "apple", not(target_os = "macos"))))) {
    /// #     test().unwrap();
    /// # }
    /// ```
    fn from(inherit: io::Stderr) -> Stdio {
        Stdio::from_inner(inherit.into())
    }
}

#[stable(feature = "anonymous_pipe", since = "1.87.0")]
impl From<io::PipeWriter> for Stdio {
    fn from(pipe: io::PipeWriter) -> Self {
        Stdio::from_inner(pipe.into_inner().into())
    }
}

#[stable(feature = "anonymous_pipe", since = "1.87.0")]
impl From<io::PipeReader> for Stdio {
    fn from(pipe: io::PipeReader) -> Self {
        Stdio::from_inner(pipe.into_inner().into())
    }
}

/// 描述一个进程终止后的结果。
///
/// 该 `struct` 用于表示子进程的退出状态或其他形式的终止。子进程通过 [`Command`]
/// 结构体创建，其退出状态通过 [`status`] 方法、或 [`Child`] 进程的 [`wait`] 方法
/// 暴露出来。
///
/// `ExitStatus` 表示一个进程所有可能的处置情况。在 Unix 上它是 **wait status（等待
/// 状态）**。它*不*只是一个*退出状态*（传给 `exit` 的值）。
///
/// 为了对失败的进程进行恰当的错误报告，应使用 `ExitStatus` 或 `ExitStatusError` 各自
/// 的 [`Display`](crate::fmt::Display) 实现来打印其值。
///
/// # 与 `ExitCode` 的区别
///
/// [`ExitCode`] 旨在通过 `Termination` trait 终止当前正在运行的进程，这与表示子进程
/// 终止的 `ExitStatus` 形成对比。这两套 API 之所以分开，是由于平台兼容性差异以及它们
/// 各自的预期用途；事后通常无法为当前进程精确地重现来自某个子进程的 `ExitStatus`。
///
/// [`status`]: Command::status
/// [`wait`]: Child::wait
//
// 我们（在这里以及标准库文档的其他多处）在谈到 `exit` 与 `_exit` 时用词略有宽松。Unix
// 系统调用的命名在各个 Unix 之间并未标准化，所以术语是约定与传统的问题。为清晰起见，
// 我们通常说 `exit`，即便我们可能指的是底层的某个系统调用，例如 `_exit`。
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
#[stable(feature = "process", since = "1.0.0")]
pub struct ExitStatus(imp::ExitStatus);

/// 默认值是一个表示成功完成的值。
#[stable(feature = "process_exitstatus_default", since = "1.73.0")]
impl Default for ExitStatus {
    fn default() -> Self {
        // 理想情况下这应当通过 ExitCode::default().into() 来完成，但那比较复杂。
        ExitStatus::from_inner(imp::ExitStatus::default())
    }
}

/// 允许在 `std` 内部定义扩展 trait。
#[unstable(feature = "sealed", issue = "none")]
impl crate::sealed::Sealed for ExitStatus {}

impl ExitStatus {
    /// 终止是否成功？返回一个 `Result`。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(exit_status_error)]
    /// # if cfg!(all(unix, not(all(target_vendor = "apple", not(target_os = "macos"))))) {
    /// use std::process::Command;
    ///
    /// let status = Command::new("ls")
    ///     .arg("/dev/nonexistent")
    ///     .status()
    ///     .expect("ls could not be executed");
    ///
    /// println!("ls: {status}");
    /// status.exit_ok().expect_err("/dev/nonexistent could be listed!");
    /// # } // cfg!(unix)
    /// ```
    #[unstable(feature = "exit_status_error", issue = "84908")]
    pub fn exit_ok(&self) -> Result<(), ExitStatusError> {
        self.0.exit_ok().map_err(ExitStatusError)
    }

    /// 终止是否成功？信号导致的终止不被视为成功，成功被定义为零退出状态。
    ///
    /// # 示例
    ///
    /// ```rust,no_run
    /// use std::process::Command;
    ///
    /// let status = Command::new("mkdir")
    ///     .arg("projects")
    ///     .status()
    ///     .expect("failed to execute mkdir");
    ///
    /// if status.success() {
    ///     println!("'projects/' directory created");
    /// } else {
    ///     println!("failed to create 'projects/' directory: {status}");
    /// }
    /// ```
    #[must_use]
    #[stable(feature = "process", since = "1.0.0")]
    pub fn success(&self) -> bool {
        self.0.exit_ok().is_ok()
    }

    /// 返回进程的退出码（如果有的话）。
    ///
    /// 用 Unix 的术语来说，返回值是 **exit status（退出状态）**：如果进程是通过调用
    /// `exit` 结束的，则为传给 `exit` 的值。注意在 Unix 上退出状态被截断为 8 位，且
    /// 那些并非来自程序对 `exit` 调用的值可能是由运行时系统臆造的（例如常见的 255、
    /// 254、127 或 126）。
    ///
    /// 在 Unix 上，如果进程是被信号终止的，这将返回 `None`。
    /// [`ExitStatusExt`](crate::os::unix::process::ExitStatusExt) 是一个扩展 trait，
    /// 用于从 `ExitStatus` 中提取任何此类信号及其他细节。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::process::Command;
    ///
    /// let status = Command::new("mkdir")
    ///     .arg("projects")
    ///     .status()
    ///     .expect("failed to execute mkdir");
    ///
    /// match status.code() {
    ///     Some(code) => println!("Exited with status code: {code}"),
    ///     None => println!("Process terminated by signal")
    /// }
    /// ```
    #[must_use]
    #[stable(feature = "process", since = "1.0.0")]
    pub fn code(&self) -> Option<i32> {
        self.0.code()
    }
}

impl AsInner<imp::ExitStatus> for ExitStatus {
    #[inline]
    fn as_inner(&self) -> &imp::ExitStatus {
        &self.0
    }
}

impl FromInner<imp::ExitStatus> for ExitStatus {
    fn from_inner(s: imp::ExitStatus) -> ExitStatus {
        ExitStatus(s)
    }
}

#[stable(feature = "process", since = "1.0.0")]
impl fmt::Display for ExitStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// 允许在 `std` 内部定义扩展 trait。
#[unstable(feature = "sealed", issue = "none")]
impl crate::sealed::Sealed for ExitStatusError {}

/// 描述一个进程失败后的结果
///
/// 由 [`ExitStatus`] 上的 [`.exit_ok`](ExitStatus::exit_ok) 方法产生。
///
/// # 示例
///
/// ```
/// #![feature(exit_status_error)]
/// # if cfg!(all(unix, not(target_os = "android"), not(all(target_vendor = "apple", not(target_os = "macos"))))) {
/// use std::process::{Command, ExitStatusError};
///
/// fn run(cmd: &str) -> Result<(), ExitStatusError> {
///     Command::new(cmd).status().unwrap().exit_ok()?;
///     Ok(())
/// }
///
/// run("true").unwrap();
/// run("false").unwrap_err();
/// # } // cfg!(unix)
/// ```
#[derive(PartialEq, Eq, Clone, Copy, Debug)]
#[unstable(feature = "exit_status_error", issue = "84908")]
// imp::ExitStatusError 的定义在理想情况下应当使得
// Result<(), imp::ExitStatusError> 与 imp::ExitStatus 具有完全相同的表示。
pub struct ExitStatusError(imp::ExitStatusError);

#[unstable(feature = "exit_status_error", issue = "84908")]
impl ExitStatusError {
    /// 从 `ExitStatusError` 中报告退出码（如果适用的话）。
    ///
    /// 用 Unix 的术语来说，返回值是 **exit status（退出状态）**：如果进程是通过调用
    /// `exit` 结束的，则为传给 `exit` 的值。注意在 Unix 上退出状态被截断为 8 位，且
    /// 那些并非来自程序对 `exit` 调用的值可能是由运行时系统臆造的（例如常见的 255、
    /// 254、127 或 126）。
    ///
    /// 在 Unix 上，如果进程是被信号终止的，这将返回 `None`。如果你想特殊处理这类
    /// 情况，可以考虑使用 [`ExitStatusExt`](crate::os::unix::process::ExitStatusExt)
    /// 中的方法。
    ///
    /// 如果进程是通过以非零值调用 `exit` 结束的，这将返回那个退出状态。
    ///
    /// 如果错误是其他原因，则返回 `None`。
    ///
    /// 如果进程成功退出（即通过调用 `exit(0)`），则不存在 `ExitStatusError`。因此
    /// `ExitStatusError::code()` 的返回值总是非零的。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(exit_status_error)]
    /// # #[cfg(all(unix, not(target_os = "android"), not(all(target_vendor = "apple", not(target_os = "macos")))))] {
    /// use std::process::Command;
    ///
    /// let bad = Command::new("false").status().unwrap().exit_ok().unwrap_err();
    /// assert_eq!(bad.code(), Some(1));
    /// # } // #[cfg(unix)]
    /// ```
    #[must_use]
    pub fn code(&self) -> Option<i32> {
        self.code_nonzero().map(Into::into)
    }

    /// 从 `ExitStatusError` 中报告退出码（如果适用的话），以 [`NonZero`] 形式返回。
    ///
    /// 这与 [`code()`](Self::code) 完全相同，区别在于它返回一个
    /// <code>[NonZero]<[i32]></code>。
    ///
    /// 之所以提供返回普通整数的普通 `code`，是因为它通常更方便。`code()` 的返回值
    /// 确实也是非零的；当你想要一个类型层面的非零保证时，使用 `code_nonzero()`。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(exit_status_error)]
    ///
    /// # if cfg!(all(unix, not(target_os = "android"), not(all(target_vendor = "apple", not(target_os = "macos"))))) {
    /// use std::num::NonZero;
    /// use std::process::Command;
    ///
    /// let bad = Command::new("false").status().unwrap().exit_ok().unwrap_err();
    /// assert_eq!(bad.code_nonzero().unwrap(), NonZero::new(1).unwrap());
    /// # } // cfg!(unix)
    /// ```
    #[must_use]
    pub fn code_nonzero(&self) -> Option<NonZero<i32>> {
        self.0.code()
    }

    /// 将一个 `ExitStatusError`（重新）转换回 `ExitStatus`。
    #[must_use]
    pub fn into_status(&self) -> ExitStatus {
        ExitStatus(self.0.into())
    }
}

#[unstable(feature = "exit_status_error", issue = "84908")]
impl From<ExitStatusError> for ExitStatus {
    fn from(error: ExitStatusError) -> Self {
        Self(error.0.into())
    }
}

#[unstable(feature = "exit_status_error", issue = "84908")]
impl fmt::Display for ExitStatusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "process exited unsuccessfully: {}", self.into_status())
    }
}

#[unstable(feature = "exit_status_error", issue = "84908")]
impl crate::error::Error for ExitStatusError {}

/// 该类型表示当前进程在正常终止时可以返回给其父进程的状态码。
///
/// `ExitCode` 旨在仅由标准库（通过 [`Termination::report()`]）使用。为了对潜在的
/// 不寻常目标平台保持前向兼容，该类型当前不提供 `Eq`、`Hash`，也不提供对原始值的
/// 访问。该类型确实提供了用于比较的 `PartialEq`，但请注意可能存在多个失败码，其中
/// 有些**不会**与 `ExitCode::FAILURE` 比较相等。标准库提供了规范的 `SUCCESS` 和
/// `FAILURE` 退出码，以及用于构造其他任意退出码的 `From<u8> for ExitCode`。
///
/// # 可移植性（Portability）
///
/// 该类型中使用的数值并没有可移植的含义，不同平台可能屏蔽掉其中不同数量的位。
///
/// 关于平台的规范成功码与失败码，见 [`SUCCESS`] 和 [`FAILURE`] 这两个关联项。
///
/// [`SUCCESS`]: ExitCode::SUCCESS
/// [`FAILURE`]: ExitCode::FAILURE
///
/// # 与 `ExitStatus` 的区别
///
/// `ExitCode` 旨在通过 `Termination` trait 终止当前正在运行的进程，这与表示子进程
/// 终止的 [`ExitStatus`] 形成对比。这两套 API 之所以分开，是由于平台兼容性差异以及
/// 它们各自的预期用途；事后通常无法为当前进程精确地重现来自某个子进程的
/// `ExitStatus`。
///
/// # 示例
///
/// `ExitCode` 可以从一个 crate 的 `main` 函数返回，因为它实现了 [`Termination`]：
///
/// ```
/// use std::process::ExitCode;
/// # fn check_foo() -> bool { true }
///
/// fn main() -> ExitCode {
///     if !check_foo() {
///         return ExitCode::from(42);
///     }
///
///     ExitCode::SUCCESS
/// }
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
#[stable(feature = "process_exitcode", since = "1.61.0")]
pub struct ExitCode(imp::ExitCode);

/// 允许在 `std` 内部定义扩展 trait。
#[unstable(feature = "sealed", issue = "none")]
impl crate::sealed::Sealed for ExitCode {}

#[stable(feature = "process_exitcode", since = "1.61.0")]
impl ExitCode {
    /// 本平台上表示成功终止的规范 `ExitCode`。
    ///
    /// 注意一个返回 `()` 的 `main` 会隐式地导致成功终止，因此除非你还要返回其他可能的
    /// 状态码，否则无需从 `main` 返回它。
    #[stable(feature = "process_exitcode", since = "1.61.0")]
    pub const SUCCESS: ExitCode = ExitCode(imp::ExitCode::SUCCESS);

    /// 本平台上表示失败终止的规范 `ExitCode`。
    ///
    /// 如果你只打算从 `main` 返回它和 `SUCCESS`，可以考虑改为返回 `Err(_)` 和
    /// `Ok(())`，它们会返回相同的状态码（但同时还会 `eprintln!` 出错误）。
    #[stable(feature = "process_exitcode", since = "1.61.0")]
    pub const FAILURE: ExitCode = ExitCode(imp::ExitCode::FAILURE);

    /// 以给定的 `ExitCode` 退出当前进程。
    ///
    /// 注意这与 [`process::exit()`][exit] 有相同的注意事项，即该函数会立即终止进程，
    /// 因此不会运行当前栈或任何其他线程栈上的析构函数。另请参阅那些文档，了解关于与
    /// C 代码互操作的一些重要说明。如果需要干净的关闭，建议直接从 `main` 函数返回这个
    /// ExitCode，如[类型文档](#examples)中所演示的那样。
    ///
    /// # 与 `process::exit()` 的区别
    ///
    /// `process::exit()` 接受任何 `i32` 值作为进程的退出码；然而，有些平台只使用该值
    /// 的一个子集（见 [`process::exit` 的平台特定行为][exit#platform-specific-behavior]）。
    /// `ExitCode` 的存在正是因为这一点；只能创建出受我们大多数平台支持的 `ExitCode`，
    /// 因此用这个方法时那些问题（在很大程度上）不复存在。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(exitcode_exit_method)]
    /// # use std::process::ExitCode;
    /// # use std::fmt;
    /// # enum UhOhError { GenericProblem, Specific, WithCode { exit_code: ExitCode, _x: () } }
    /// # impl fmt::Display for UhOhError {
    /// #     fn fmt(&self, _: &mut fmt::Formatter<'_>) -> fmt::Result { unimplemented!() }
    /// # }
    /// // 无法从 UhOhError 优雅地恢复，所以我们只是
    /// // 打印一条消息并退出
    /// fn handle_unrecoverable_error(err: UhOhError) -> ! {
    ///     eprintln!("UH OH! {err}");
    ///     let code = match err {
    ///         UhOhError::GenericProblem => ExitCode::FAILURE,
    ///         UhOhError::Specific => ExitCode::from(3),
    ///         UhOhError::WithCode { exit_code, .. } => exit_code,
    ///     };
    ///     code.exit_process()
    /// }
    /// ```
    #[unstable(feature = "exitcode_exit_method", issue = "97100")]
    pub fn exit_process(self) -> ! {
        exit(self.to_i32())
    }
}

impl ExitCode {
    // 这是私有/永久不稳定的，因为 ExitCode 是不透明的；我们并不确定 i32 能服务于
    // 所有用例，例如 windows 似乎使用 u32，unix 使用一个 i32 的第 8-15 位，我们很可能
    // 想把用户与任何可能限制 ExitCode 平台特定表示的东西隔离开。
    //
    // 更多信息：https://internals.rust-lang.org/t/mini-pre-rfc-redesigning-process-exitstatus/5426
    /// 将一个 `ExitCode` 转换为 i32
    #[unstable(
        feature = "process_exitcode_internals",
        reason = "exposed only for libstd",
        issue = "none"
    )]
    #[inline]
    #[doc(hidden)]
    pub fn to_i32(self) -> i32 {
        self.0.as_i32()
    }
}

/// 默认值是 [`ExitCode::SUCCESS`]
#[stable(feature = "process_exitcode_default", since = "1.75.0")]
impl Default for ExitCode {
    fn default() -> Self {
        ExitCode::SUCCESS
    }
}

#[stable(feature = "process_exitcode", since = "1.61.0")]
impl From<u8> for ExitCode {
    /// 从一个任意的 u8 值构造一个 `ExitCode`。
    fn from(code: u8) -> Self {
        ExitCode(imp::ExitCode::from(code))
    }
}

impl AsInner<imp::ExitCode> for ExitCode {
    #[inline]
    fn as_inner(&self) -> &imp::ExitCode {
        &self.0
    }
}

impl FromInner<imp::ExitCode> for ExitCode {
    fn from_inner(s: imp::ExitCode) -> ExitCode {
        ExitCode(s)
    }
}

impl Child {
    /// 强制子进程退出。如果子进程已经退出，则返回 `Ok(())`。
    ///
    /// 到各种 [`ErrorKind`] 的映射不属于该函数的兼容性契约的一部分。
    ///
    /// 在 Unix 平台上，这等价于发送一个 SIGKILL。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::process::Command;
    ///
    /// let mut command = Command::new("yes");
    /// if let Ok(mut child) = command.spawn() {
    ///     child.kill().expect("command couldn't be killed");
    /// } else {
    ///     println!("yes command didn't start");
    /// }
    /// ```
    ///
    /// [`ErrorKind`]: io::ErrorKind
    /// [`InvalidInput`]: io::ErrorKind::InvalidInput
    #[stable(feature = "process", since = "1.0.0")]
    #[cfg_attr(not(test), rustc_diagnostic_item = "child_kill")]
    pub fn kill(&mut self) -> io::Result<()> {
        self.handle.kill()
    }

    /// 返回与该子进程关联的、由 OS 分配的进程标识符。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::process::Command;
    ///
    /// let mut command = Command::new("ls");
    /// if let Ok(child) = command.spawn() {
    ///     println!("Child's ID is {}", child.id());
    /// } else {
    ///     println!("ls command didn't start");
    /// }
    /// ```
    #[must_use]
    #[stable(feature = "process_id", since = "1.3.0")]
    #[cfg_attr(not(test), rustc_diagnostic_item = "child_id")]
    pub fn id(&self) -> u32 {
        self.handle.id()
    }

    /// 等待子进程完全退出，返回它退出时的状态。在至少被调用一次之后，此函数将持续
    /// 返回相同的值。
    ///
    /// 指向子进程的 stdin 句柄（如果有的话）会在等待之前被关闭。这有助于避免死锁：
    /// 它确保子进程不会因等待来自父进程的输入而阻塞，与此同时父进程又在等待子进程
    /// 退出。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::process::Command;
    ///
    /// let mut command = Command::new("ls");
    /// if let Ok(mut child) = command.spawn() {
    ///     child.wait().expect("command wasn't running");
    ///     println!("Child has finished its execution!");
    /// } else {
    ///     println!("ls command didn't start");
    /// }
    /// ```
    #[stable(feature = "process", since = "1.0.0")]
    pub fn wait(&mut self) -> io::Result<ExitStatus> {
        drop(self.stdin.take());
        self.handle.wait().map(ExitStatus)
    }

    /// 尝试收集子进程的退出状态（如果它已经退出的话）。
    ///
    /// 此函数不会阻塞调用线程，只会检查子进程是否已经退出。如果子进程已经退出，那么
    /// 在 Unix 上其进程 ID 会被回收（reap）。只要子进程已经退出，此函数就保证会重复
    /// 返回一个成功的退出状态。
    ///
    /// 如果子进程已退出，则返回 `Ok(Some(status))`。如果此刻退出状态尚不可得，则返回
    /// `Ok(None)`。如果发生错误，则返回该错误。
    ///
    /// 注意与 `wait` 不同，此函数不会尝试 drop stdin。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::process::Command;
    ///
    /// let mut child = Command::new("ls").spawn()?;
    ///
    /// match child.try_wait() {
    ///     Ok(Some(status)) => println!("exited with: {status}"),
    ///     Ok(None) => {
    ///         println!("status not ready yet, let's really wait");
    ///         let res = child.wait();
    ///         println!("result: {res:?}");
    ///     }
    ///     Err(e) => println!("error attempting to wait: {e}"),
    /// }
    /// # std::io::Result::Ok(())
    /// ```
    #[stable(feature = "process_try_wait", since = "1.18.0")]
    pub fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        Ok(self.handle.try_wait()?.map(ExitStatus))
    }

    /// 同时等待子进程退出并收集 stdout/stderr 句柄上剩余的全部输出，返回一个
    /// `Output` 实例。
    ///
    /// 指向子进程的 stdin 句柄（如果有的话）会在等待之前被关闭。这有助于避免死锁：
    /// 它确保子进程不会因等待来自父进程的输入而阻塞，与此同时父进程又在等待子进程
    /// 退出。
    ///
    /// 默认情况下，stdin、stdout 和 stderr 都从父进程继承。为了把输出捕获到这个
    /// `Result<Output>` 中，需要在父进程与子进程之间创建新的管道。请分别使用
    /// `stdout(Stdio::piped())` 或 `stderr(Stdio::piped())`。
    ///
    /// # 示例
    ///
    /// ```should_panic
    /// use std::process::{Command, Stdio};
    ///
    /// let child = Command::new("/bin/cat")
    ///     .arg("file.txt")
    ///     .stdout(Stdio::piped())
    ///     .spawn()
    ///     .expect("failed to execute child");
    ///
    /// let output = child
    ///     .wait_with_output()
    ///     .expect("failed to wait on child");
    ///
    /// assert!(output.status.success());
    /// ```
    ///
    #[stable(feature = "process", since = "1.0.0")]
    pub fn wait_with_output(mut self) -> io::Result<Output> {
        drop(self.stdin.take());

        let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
        match (self.stdout.take(), self.stderr.take()) {
            (None, None) => {}
            (Some(mut out), None) => {
                let res = out.read_to_end(&mut stdout);
                res.unwrap();
            }
            (None, Some(mut err)) => {
                let res = err.read_to_end(&mut stderr);
                res.unwrap();
            }
            (Some(out), Some(err)) => {
                let res = imp::read_output(out.inner, &mut stdout, err.inner, &mut stderr);
                res.unwrap();
            }
        }

        let status = self.wait()?;
        Ok(Output { status, stdout, stderr })
    }
}

/// 以指定的退出码终止当前进程。
///
/// 此函数永远不会返回，并将立即终止当前进程。退出码会被透传给底层 OS，并可供另一个
/// 进程消费。
///
/// 注意由于此函数永不返回且会终止进程，因此不会运行当前栈或任何其他线程栈上的析构
/// 函数。如果需要干净的关闭，建议只在一个已知没有更多析构函数待运行的点上调用此
/// 函数；或者，更好的做法是直接从 `main` 函数返回一个实现了 [`Termination`] 的类型
/// （例如 [`ExitCode`] 或 `Result`），从而完全避免使用此函数：
///
/// ```
/// # use std::io::Error as MyError;
/// fn main() -> Result<(), MyError> {
///     // ...
///     Ok(())
/// }
/// ```
///
/// 在其当前实现中，此函数将执行通过 `atexit` 注册的退出处理器，以及其他平台特定的
/// 退出处理器（例如 ELF 共享对象的 `fini` 段）。这意味着 Rust 要求所有退出处理器
/// 在任意时刻执行都是安全的。特别地，如果某个退出处理器清理了某些可能被其他线程
/// 并发访问的状态，那么就要求该退出处理器与那些线程进行适当的同步。（这一要求的替代
/// 方案将是完全不运行退出处理器，而这被认为是不可取的。注意从 `main` 返回也会调用
/// `exit`，因此把 `exit` 设为一个 unsafe 操作并不是一个可选项。）
///
/// ## Platform-specific behavior
///
/// **Unix**：在类 Unix 平台上，`exit` 的全部 32 位不太可能对一个检查退出码的父进程
/// 都可见。在大多数类 Unix 平台上，只考虑最低有效的 8 位。
///
/// 例如，下面这个示例的退出码在 Linux 上将是 `0`，但在 Windows 上是 `256`：
///
/// ```no_run
/// use std::process;
///
/// process::exit(0x0100);
/// ```
///
/// ### 与 C 代码的安全互操作
///
/// 在 Unix 上，此函数当前使用 C 函数 [`exit`][C-exit] 实现。截至 C23，C 标准不允许
/// 多个线程并发调用 `exit`。Rust 用一个锁来缓解这一点，但如果 C 代码调用 `exit`，
/// 那仍然可能导致未定义行为。注意从 `main` 返回等价于调用 `exit`。
///
/// 因此，如果两个并发线程在没有同步的情况下执行以下操作，就是未定义行为：
/// - 一个线程调用 Rust 的 `exit` 函数或从 Rust 的 `main` 函数返回
/// - 另一个线程调用 C 函数 `exit` 或 `quick_exit`，或从 C 的 `main` 函数返回
///
/// 注意如果一个二进制文件包含 Rust 运行时的多个副本（例如组合多个 `cdylib` 或
/// `staticlib` 时），它们各自拥有独立的锁，因此从运行在某个 Rust 运行时中的代码的
/// 视角看，“外部的”那些 Rust 代码基本上就是 C 代码，并发的 `exit` 同样会导致未定义
/// 行为。
///
/// 各个 C 实现可能提供比标准更强的保证并允许并发调用 `exit`；细节请查阅你的 C 实现
/// 的文档。
///
/// 关于使 C 中的 `exit` 线程安全的一些正在进行的讨论，见：
/// - [Rust issue #126600](https://github.com/rust-lang/rust/issues/126600)
/// - [Austin Group Bugzilla (for POSIX)](https://austingroupbugs.net/view.php?id=1845)
/// - [GNU C library Bugzilla](https://sourceware.org/bugzilla/show_bug.cgi?id=31997)
///
/// [C-exit]: https://en.cppreference.com/w/c/program/exit
#[stable(feature = "rust1", since = "1.0.0")]
#[cfg_attr(not(test), rustc_diagnostic_item = "process_exit")]
pub fn exit(code: i32) -> ! {
    crate::rt::cleanup();
    crate::sys::os::exit(code)
}

/// 以一种异常的方式终止进程。
///
/// 该函数永远不会返回，并将立即以一种平台特定的“异常”方式终止当前进程。因此，
/// 不会运行当前栈或任何其他线程栈上的析构函数，Rust 的 IO 缓冲区（例如来自
/// `BufWriter` 的）不会被刷新，且 C 的 stdio 缓冲区（在大多数平台上）也不会被刷新。
///
/// 这与 [`panic!`] 的默认行为形成对比，后者会展开当前线程的栈并调用所有析构函数。
/// 当设置了 `panic="abort"`（无论是作为 `rustc` 的参数还是在某个 crate 的 Cargo.toml
/// 中）时，[`panic!`] 和 `abort` 是类似的。不过，[`panic!`] 仍会调用 [panic hook]，
/// 而 `abort` 不会。
///
/// 如果需要干净的关闭，建议只在一个已知没有更多析构函数待运行的点上调用此函数。
///
/// 进程的终止将类似于 C 的 `abort()` 函数所导致的终止。在 Unix 上，进程将以信号
/// `SIGABRT` 终止，这通常意味着 shell 会打印 "Aborted"。
///
/// # 示例
///
/// ```no_run
/// use std::process;
///
/// fn main() {
///     println!("aborting");
///
///     process::abort();
///
///     // 执行永远到不了这里
/// }
/// ```
///
/// `abort` 函数会终止进程，因此下面这个示例中的析构函数不会被运行：
///
/// ```no_run
/// use std::process;
///
/// struct HasDrop;
///
/// impl Drop for HasDrop {
///     fn drop(&mut self) {
///         println!("This will never be printed!");
///     }
/// }
///
/// fn main() {
///     let _x = HasDrop;
///     process::abort();
///     // 为 HasDrop 实现的析构函数永远不会被运行
/// }
/// ```
///
/// [panic hook]: crate::panic::set_hook
#[stable(feature = "process_abort", since = "1.17.0")]
#[cold]
#[cfg_attr(not(test), rustc_diagnostic_item = "process_abort")]
#[cfg_attr(miri, track_caller)] // 即便没有 panic，这对 Miri 的回溯也有帮助
pub fn abort() -> ! {
    crate::sys::abort_internal();
}

/// 返回与本进程关联的、由 OS 分配的进程标识符。
///
/// # 示例
///
/// ```no_run
/// use std::process;
///
/// println!("My pid is {}", process::id());
/// ```
#[must_use]
#[stable(feature = "getpid", since = "1.26.0")]
pub fn id() -> u32 {
    crate::sys::os::getpid()
}

/// 一个用于在 `main` 函数中实现任意返回类型的 trait。
///
/// C 的 main 函数只支持返回整数。因此，每个实现了 `Termination` trait 的类型都必须
/// 被转换成一个整数。
///
/// 默认实现返回 `libc::EXIT_SUCCESS` 以表示一次成功的执行。在失败的情况下，则返回
/// `libc::EXIT_FAILURE`。
///
/// 由于不同的运行时对 `main` 函数返回值有不同的规范，因此为方便起见，该 trait 很可能
/// 只在标准库的运行时上可用。其他运行时并不要求提供类似的功能。
#[cfg_attr(not(any(test, doctest)), lang = "termination")]
#[stable(feature = "termination_trait_lib", since = "1.61.0")]
#[rustc_on_unimplemented(on(
    cause = "MainFunctionType",
    message = "`main` has invalid return type `{Self}`",
    label = "`main` can only return types that implement `{This}`"
))]
pub trait Termination {
    /// 被调用以获取该值作为状态码的表示。这个状态码会被返回给操作系统。
    #[stable(feature = "termination_trait_lib", since = "1.61.0")]
    fn report(self) -> ExitCode;
}

#[stable(feature = "termination_trait_lib", since = "1.61.0")]
impl Termination for () {
    #[inline]
    fn report(self) -> ExitCode {
        ExitCode::SUCCESS
    }
}

#[stable(feature = "termination_trait_lib", since = "1.61.0")]
impl Termination for ! {
    fn report(self) -> ExitCode {
        self
    }
}

#[stable(feature = "termination_trait_lib", since = "1.61.0")]
impl Termination for Infallible {
    fn report(self) -> ExitCode {
        match self {}
    }
}

#[stable(feature = "termination_trait_lib", since = "1.61.0")]
impl Termination for ExitCode {
    #[inline]
    fn report(self) -> ExitCode {
        self
    }
}

#[stable(feature = "termination_trait_lib", since = "1.61.0")]
impl<T: Termination, E: fmt::Debug> Termination for Result<T, E> {
    fn report(self) -> ExitCode {
        match self {
            Ok(val) => val.report(),
            Err(err) => {
                io::attempt_print_to_stderr(format_args_nl!("Error: {err:?}"));
                ExitCode::FAILURE
            }
        }
    }
}
