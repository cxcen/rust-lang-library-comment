//! 针对 [`std::process`] 模块中各基础类型的 Unix 特有扩展。
//!
//! [`std::process`]: crate::process

#![stable(feature = "rust1", since = "1.0.0")]

use crate::ffi::OsStr;
use crate::os::unix::io::{AsFd, AsRawFd, BorrowedFd, FromRawFd, IntoRawFd, OwnedFd, RawFd};
use crate::path::Path;
use crate::sealed::Sealed;
use crate::sys::process::ChildPipe;
use crate::sys::{AsInner, AsInnerMut, FromInner, IntoInner};
use crate::{io, process, sys};

cfg_select! {
    any(target_os = "vxworks", target_os = "espidf", target_os = "horizon", target_os = "vita") => {
        type UserId = u16;
        type GroupId = u16;
    }
    target_os = "nto" => {
        // 两个 ID 都是有符号的，参见 QNX Neutrino SDP 的 `sys/target_nto.h`。
        // 只应使用正值，参见例如
        // https://www.qnx.com/developers/docs/7.1/#com.qnx.doc.neutrino.lib_ref/topic/s/setuid.html
        type UserId = i32;
        type GroupId = i32;
    }
    _ => {
        type UserId = u32;
        type GroupId = u32;
    }
}

/// 针对 [`process::Command`] 构建器的 Unix 特有扩展。
///
/// 该 trait 是封闭的（sealed）：它不能在标准库之外被实现。
/// 这样做是为了让未来新增的方法不会成为破坏性变更（breaking changes）。
#[stable(feature = "rust1", since = "1.0.0")]
pub trait CommandExt: Sealed {
    /// 设置子进程的用户 ID。这会在子进程中转化为一次 `setuid` 调用。
    /// `setuid` 调用失败将导致派生（spawn）失败。
    ///
    /// # Notes
    ///
    /// 如果未指定任何组，这还会在子进程中触发一次 `setgroups(0, NULL)` 调用。
    /// 这会移除那些可能赋予子进程不期望权限的补充组（supplementary groups）。
    #[stable(feature = "rust1", since = "1.0.0")]
    fn uid(&mut self, id: UserId) -> &mut process::Command;

    /// 与 `uid` 类似，但设置子进程的组 ID。它与 `uid` 字段具有相同的语义。
    #[stable(feature = "rust1", since = "1.0.0")]
    fn gid(&mut self, id: GroupId) -> &mut process::Command;

    /// 设置调用进程的补充组 ID（supplementary group IDs）。这会在子进程中转化为
    /// 一次 `setgroups` 调用。
    #[unstable(feature = "setgroups", issue = "90747")]
    fn groups(&mut self, groups: &[GroupId]) -> &mut process::Command;

    /// 安排一个闭包，使其恰好在 `exec` 函数被调用之前运行。
    ///
    /// 该闭包允许返回一个 I/O 错误，其操作系统错误码将被回传给父进程，并在请求
    /// 派生（spawn）时作为错误返回。
    ///
    /// 可以注册多个闭包，它们将按注册顺序被调用。如果某个闭包返回 `Err`，则不会
    /// 再调用后续闭包，且派生操作会立即以失败返回。
    ///
    /// # Notes and Safety
    ///
    /// 该闭包将在 `fork` 之后于子进程的上下文中运行。这首先意味着，代表该闭包对内存
    /// 所做的任何修改对父进程都将**不**可见。这通常是一个受到严格约束的环境，诸如
    /// `malloc`、通过 [`std::env`] 访问环境变量，或获取互斥锁等常规操作都不保证能够正常
    /// 工作（因为在执行 `fork` 时，其他线程可能仍在运行）。
    ///
    /// 注意，会进行分配的函数列表包括 [`Error::new`] 与 [`Error::other`]。若要发出一个
    /// 非平凡的错误信号，请优先使用 [`panic!`]。
    ///
    /// 更多细节请参阅 [POSIX fork() specification] 以及任何目标平台的等价文档，
    /// 尤其是围绕*异步信号安全（async-signal-safety）*的各项要求。
    ///
    /// 这还意味着诸如文件描述符与内存映射区域之类的所有资源都被复制了一份。确保该闭包
    /// 不会通过对这些副本的无效使用而破坏库的不变量，是你的责任。
    ///
    /// 仅当 panic 消息的所有格式化参数都能被安全地格式化时，在该闭包中进行 panic 才是
    /// 安全的；这是因为尽管 `Command` 会在调用 pre_exec 钩子之前先调用
    /// [`std::panic::always_abort`](crate::panic::always_abort)，panic 仍会尝试
    /// 格式化 panic 消息。
    ///
    /// 当该闭包运行时，诸如标准 I/O 文件描述符与工作目录等方面已经被成功更改，因此
    /// 输出到这些位置的内容可能不会出现在预期的地方。
    ///
    /// [POSIX fork() specification]:
    ///     https://pubs.opengroup.org/onlinepubs/9699919799/functions/fork.html
    /// [`std::env`]: mod@crate::env
    /// [`Error::new`]: crate::io::Error::new
    /// [`Error::other`]: crate::io::Error::other
    #[stable(feature = "process_pre_exec", since = "1.34.0")]
    unsafe fn pre_exec<F>(&mut self, f: F) -> &mut process::Command
    where
        F: FnMut() -> io::Result<()> + Send + Sync + 'static;

    /// 安排一个闭包，使其恰好在 `exec` 函数被调用之前运行。
    ///
    /// `before_exec` 曾是一个安全方法，但由于该闭包只能执行*异步信号安全
    ///（async-signal-safe）*的操作，它需要是 unsafe 的。因此它被弃用，转而使用 unsafe 的
    /// [`pre_exec`]。与此同时，Rust 获得了在新的 edition 中将一个现有安全方法完全标记为
    /// unsafe 的能力，`before_exec` 正是借此变为 `unsafe`。它目前仍处于弃用状态；
    /// 应改用 `pre_exec`。
    ///
    /// [`pre_exec`]: CommandExt::pre_exec
    #[stable(feature = "process_exec", since = "1.15.0")]
    #[deprecated(since = "1.37.0", note = "should be unsafe, use `pre_exec` instead")]
    #[rustc_deprecated_safe_2024(audit_that = "the closure is async-signal-safe")]
    unsafe fn before_exec<F>(&mut self, f: F) -> &mut process::Command
    where
        F: FnMut() -> io::Result<()> + Send + Sync + 'static,
    {
        unsafe { self.pre_exec(f) }
    }

    /// 执行此 `Command` 所需的全部设置，随后调用 `execvp` 系统调用。
    ///
    /// 成功时此函数不会返回，否则它将返回一个错误，指明 exec（或 `Command` 设置的
    /// 其他某个环节）失败的原因。
    ///
    /// `exec` 不返回这一点与调用 [`process::exit`] 有相同的含义——当前栈或任何其他
    /// 线程栈上的析构函数都不会运行。因此，建议仅在“不运行任何析构函数也无妨”的位置
    /// 调用 `exec`。注意，`execvp` 系统调用独立地保证所有内存都会被释放，且所有带有
    /// `CLOEXEC` 选项（标准库打开的所有文件描述符默认都设置该选项）的文件描述符都会
    /// 被关闭。
    ///
    /// 与 `spawn` 不同，此函数**不会**对进程进行 `fork` 以创建新的子进程。不过，与 spawn
    /// 一样，标准 I/O 描述符的默认行为将是从当前进程继承它们。
    ///
    /// # Notes
    ///
    /// 如果此函数以错误返回，进程可能处于“损坏状态（broken state）”。例如工作目录、
    /// 环境变量、信号处理设置、各种用户/组信息，或标准 I/O 文件描述符的某些方面可能
    /// 已被更改。如果需要一次“事务式派生（transactional spawn）”来优雅地处理错误，
    /// 建议改用跨平台的 `spawn`。
    #[stable(feature = "process_exec2", since = "1.9.0")]
    #[must_use]
    fn exec(&mut self) -> io::Error;

    /// 设置可执行文件参数
    ///
    /// 把第一个进程参数 `argv[0]` 设置为默认可执行文件路径之外的某个值。
    #[stable(feature = "process_set_argv0", since = "1.45.0")]
    fn arg0<S>(&mut self, arg: S) -> &mut process::Command
    where
        S: AsRef<OsStr>;

    /// 设置子进程的进程组 ID（PGID）。等价于在子进程中进行一次 `setpgid` 调用，
    /// 但可能更高效。
    ///
    /// 进程组决定哪些进程会接收信号。
    ///
    /// # 示例
    ///
    /// 在终端中按下 Ctrl-C 会向当前前台进程组中的所有进程发送 SIGINT。通过在一个新的
    /// 进程组中派生 `sleep` 子进程，它将不会从终端接收到 SIGINT。
    ///
    /// 父进程可以安装一个信号处理器，并按自己的方式管理该子进程。
    ///
    /// 进程组 ID 为 0 将使用进程 ID 作为 PGID。
    ///
    /// ```no_run
    /// use std::process::Command;
    /// use std::os::unix::process::CommandExt;
    ///
    /// Command::new("sleep")
    ///     .arg("10")
    ///     .process_group(0)
    ///     .spawn()?
    ///     .wait()?;
    /// #
    /// # Ok::<_, Box<dyn std::error::Error>>(())
    /// ```
    #[stable(feature = "process_set_process_group", since = "1.64.0")]
    fn process_group(&mut self, pgroup: i32) -> &mut process::Command;

    /// 设置子进程的根目录。这会在执行该命令之前于子进程中调用 `chroot`。
    ///
    /// 这发生在切换到由 [`process::Command::current_dir`] 指定的目录之前，且该目录
    /// 将相对于新的根目录。
    ///
    /// 如果没有用 [`process::Command::current_dir`] 指定任何目录，这将把目录设置为 `/`，
    /// 以避免把当前目录留在 chroot 之外。（这是与底层 `chroot` 系统调用有意为之的差异。）
    #[unstable(feature = "process_chroot", issue = "141298")]
    fn chroot<P: AsRef<Path>>(&mut self, dir: P) -> &mut process::Command;

    #[unstable(feature = "process_setsid", issue = "105376")]
    fn setsid(&mut self, setsid: bool) -> &mut process::Command;
}

#[stable(feature = "rust1", since = "1.0.0")]
impl CommandExt for process::Command {
    fn uid(&mut self, id: UserId) -> &mut process::Command {
        self.as_inner_mut().uid(id);
        self
    }

    fn gid(&mut self, id: GroupId) -> &mut process::Command {
        self.as_inner_mut().gid(id);
        self
    }

    fn groups(&mut self, groups: &[GroupId]) -> &mut process::Command {
        self.as_inner_mut().groups(groups);
        self
    }

    unsafe fn pre_exec<F>(&mut self, f: F) -> &mut process::Command
    where
        F: FnMut() -> io::Result<()> + Send + Sync + 'static,
    {
        self.as_inner_mut().pre_exec(Box::new(f));
        self
    }

    fn exec(&mut self) -> io::Error {
        // 注意：在 `libc::fork` 之后调用它可能*不*安全，因为它可能进行分配。
        // 这一点也许在将来的某个时刻值得修复。
        self.as_inner_mut().exec(sys::process::Stdio::Inherit)
    }

    fn arg0<S>(&mut self, arg: S) -> &mut process::Command
    where
        S: AsRef<OsStr>,
    {
        self.as_inner_mut().set_arg_0(arg.as_ref());
        self
    }

    fn process_group(&mut self, pgroup: i32) -> &mut process::Command {
        self.as_inner_mut().pgroup(pgroup);
        self
    }

    fn chroot<P: AsRef<Path>>(&mut self, dir: P) -> &mut process::Command {
        self.as_inner_mut().chroot(dir.as_ref());
        self
    }

    fn setsid(&mut self, setsid: bool) -> &mut process::Command {
        self.as_inner_mut().setsid(setsid);
        self
    }
}

/// 针对 [`process::ExitStatus`] 与
/// [`ExitStatusError`](process::ExitStatusError) 的 Unix 特有扩展。
///
/// 在 Unix 上，`ExitStatus` **未必表示一个退出状态（exit status）**——退出状态指
/// 传给 `_exit` 系统调用、或由 [`ExitStatus::code()`](crate::process::ExitStatus::code)
/// 返回的那种值。它表示由 `wait` 系列系统调用之一返回的**任意等待状态（wait status）**。
///
/// 一个 Unix 等待状态（即 Rust 的 `ExitStatus`）可以表示一个 Unix 退出状态，但也可以
/// 表示其他种类的进程事件。
///
/// 该 trait 是封闭的（sealed）：它不能在标准库之外被实现。
/// 这样做是为了让未来新增的方法不会成为破坏性变更（breaking changes）。
#[stable(feature = "rust1", since = "1.0.0")]
pub trait ExitStatusExt: Sealed {
    /// 从 `wait` 返回的底层裸整数状态值创建一个新的 `ExitStatus` 或 `ExitStatusError`
    ///
    /// 该值应当是一个**等待状态（wait status），而非退出状态（exit status）**。
    ///
    /// # Panics
    ///
    /// 尝试从等待状态 `0` 构造 `ExitStatusError` 时会 panic。
    ///
    /// 构造 `ExitStatus` 总是成功，永远不会 panic。
    #[stable(feature = "exit_status_from", since = "1.12.0")]
    fn from_raw(raw: i32) -> Self;

    /// 如果进程是被信号终止的，则返回该信号。
    ///
    /// 换句话说，如果 `WIFSIGNALED`，则返回 `WTERMSIG`。
    #[stable(feature = "rust1", since = "1.0.0")]
    fn signal(&self) -> Option<i32>;

    /// 如果进程是被信号终止的，则给出它是否转储了核心（dumped core）。
    #[stable(feature = "unix_process_wait_more", since = "1.58.0")]
    fn core_dumped(&self) -> bool;

    /// 如果进程是被信号停止（stopped）的，则返回该信号。
    ///
    /// 换句话说，如果 `WIFSTOPPED`，则返回 `WSTOPSIG`。只有当该状态来自一个传入了
    /// `WUNTRACED` 的 `wait` 系统调用、并随后被转换为 `ExitStatus` 时，这才有可能。
    #[stable(feature = "unix_process_wait_more", since = "1.58.0")]
    fn stopped_signal(&self) -> Option<i32>;

    /// 进程是否从停止（stopped）状态被继续（continued）。
    ///
    /// 即 `WIFCONTINUED`。只有当该状态来自一个传入了 `WCONTINUED` 的 `wait` 系统调用、
    /// 并随后被转换为 `ExitStatus` 时，这才有可能。
    #[stable(feature = "unix_process_wait_more", since = "1.58.0")]
    fn continued(&self) -> bool;

    /// 返回底层的裸 `wait` 状态。
    ///
    /// 返回的整数是一个**等待状态（wait status），而非退出状态（exit status）**。
    #[stable(feature = "unix_process_wait_more", since = "1.58.0")]
    fn into_raw(self) -> i32;
}

#[stable(feature = "rust1", since = "1.0.0")]
impl ExitStatusExt for process::ExitStatus {
    fn from_raw(raw: i32) -> Self {
        process::ExitStatus::from_inner(From::from(raw))
    }

    fn signal(&self) -> Option<i32> {
        self.as_inner().signal()
    }

    fn core_dumped(&self) -> bool {
        self.as_inner().core_dumped()
    }

    fn stopped_signal(&self) -> Option<i32> {
        self.as_inner().stopped_signal()
    }

    fn continued(&self) -> bool {
        self.as_inner().continued()
    }

    fn into_raw(self) -> i32 {
        self.as_inner().into_raw().into()
    }
}

#[unstable(feature = "exit_status_error", issue = "84908")]
impl ExitStatusExt for process::ExitStatusError {
    fn from_raw(raw: i32) -> Self {
        process::ExitStatus::from_raw(raw)
            .exit_ok()
            .expect_err("<ExitStatusError as ExitStatusExt>::from_raw(0) but zero is not an error")
    }

    fn signal(&self) -> Option<i32> {
        self.into_status().signal()
    }

    fn core_dumped(&self) -> bool {
        self.into_status().core_dumped()
    }

    fn stopped_signal(&self) -> Option<i32> {
        self.into_status().stopped_signal()
    }

    fn continued(&self) -> bool {
        self.into_status().continued()
    }

    fn into_raw(self) -> i32 {
        self.into_status().into_raw()
    }
}

#[unstable(feature = "unix_send_signal", issue = "141975")]
pub trait ChildExt: Sealed {
    /// 向子进程发送一个信号。
    ///
    /// # Errors
    ///
    /// 如果信号无效，此函数将返回错误。与信号关联的整数值是实现特定的，因此鼓励使用
    /// 一个提供 posix 绑定的 crate。
    ///
    /// # 示例
    ///
    /// ```rust
    /// #![feature(unix_send_signal)]
    ///
    /// use std::{io, os::unix::process::ChildExt, process::{Command, Stdio}};
    ///
    /// use libc::SIGTERM;
    ///
    /// fn main() -> io::Result<()> {
    ///     # if cfg!(not(all(target_vendor = "apple", not(target_os = "macos")))) {
    ///     let child = Command::new("cat").stdin(Stdio::piped()).spawn()?;
    ///     child.send_signal(SIGTERM)?;
    ///     # }
    ///     Ok(())
    /// }
    /// ```
    fn send_signal(&self, signal: i32) -> io::Result<()>;
}

#[unstable(feature = "unix_send_signal", issue = "141975")]
impl ChildExt for process::Child {
    fn send_signal(&self, signal: i32) -> io::Result<()> {
        self.handle.send_signal(signal)
    }
}

#[stable(feature = "process_extensions", since = "1.2.0")]
impl FromRawFd for process::Stdio {
    #[inline]
    unsafe fn from_raw_fd(fd: RawFd) -> process::Stdio {
        let fd = sys::fd::FileDesc::from_raw_fd(fd);
        let io = sys::process::Stdio::Fd(fd);
        process::Stdio::from_inner(io)
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl From<OwnedFd> for process::Stdio {
    /// 取得一个文件描述符的所有权，并返回一个可以将流附加到它上的
    /// [`Stdio`](process::Stdio)。
    #[inline]
    fn from(fd: OwnedFd) -> process::Stdio {
        let fd = sys::fd::FileDesc::from_inner(fd);
        let io = sys::process::Stdio::Fd(fd);
        process::Stdio::from_inner(io)
    }
}

#[stable(feature = "process_extensions", since = "1.2.0")]
impl AsRawFd for process::ChildStdin {
    #[inline]
    fn as_raw_fd(&self) -> RawFd {
        self.as_inner().as_raw_fd()
    }
}

#[stable(feature = "process_extensions", since = "1.2.0")]
impl AsRawFd for process::ChildStdout {
    #[inline]
    fn as_raw_fd(&self) -> RawFd {
        self.as_inner().as_raw_fd()
    }
}

#[stable(feature = "process_extensions", since = "1.2.0")]
impl AsRawFd for process::ChildStderr {
    #[inline]
    fn as_raw_fd(&self) -> RawFd {
        self.as_inner().as_raw_fd()
    }
}

#[stable(feature = "into_raw_os", since = "1.4.0")]
impl IntoRawFd for process::ChildStdin {
    #[inline]
    fn into_raw_fd(self) -> RawFd {
        self.into_inner().into_inner().into_raw_fd()
    }
}

#[stable(feature = "into_raw_os", since = "1.4.0")]
impl IntoRawFd for process::ChildStdout {
    #[inline]
    fn into_raw_fd(self) -> RawFd {
        self.into_inner().into_inner().into_raw_fd()
    }
}

#[stable(feature = "into_raw_os", since = "1.4.0")]
impl IntoRawFd for process::ChildStderr {
    #[inline]
    fn into_raw_fd(self) -> RawFd {
        self.into_inner().into_inner().into_raw_fd()
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl AsFd for crate::process::ChildStdin {
    #[inline]
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.as_inner().as_fd()
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl From<crate::process::ChildStdin> for OwnedFd {
    /// 取得一个 [`ChildStdin`](crate::process::ChildStdin) 的文件描述符的所有权。
    #[inline]
    fn from(child_stdin: crate::process::ChildStdin) -> OwnedFd {
        child_stdin.into_inner().into_inner()
    }
}

/// 从所提供的 `OwnedFd` 创建一个 `ChildStdin`。
///
/// 所提供的文件描述符必须指向一个设置了 `CLOEXEC` 标志的管道（pipe）。
#[stable(feature = "child_stream_from_fd", since = "1.74.0")]
impl From<OwnedFd> for process::ChildStdin {
    #[inline]
    fn from(fd: OwnedFd) -> process::ChildStdin {
        let pipe = ChildPipe::from_inner(fd);
        process::ChildStdin::from_inner(pipe)
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl AsFd for crate::process::ChildStdout {
    #[inline]
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.as_inner().as_fd()
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl From<crate::process::ChildStdout> for OwnedFd {
    /// 取得一个 [`ChildStdout`](crate::process::ChildStdout) 的文件描述符的所有权。
    #[inline]
    fn from(child_stdout: crate::process::ChildStdout) -> OwnedFd {
        child_stdout.into_inner().into_inner()
    }
}

/// 从所提供的 `OwnedFd` 创建一个 `ChildStdout`。
///
/// 所提供的文件描述符必须指向一个设置了 `CLOEXEC` 标志的管道（pipe）。
#[stable(feature = "child_stream_from_fd", since = "1.74.0")]
impl From<OwnedFd> for process::ChildStdout {
    #[inline]
    fn from(fd: OwnedFd) -> process::ChildStdout {
        let pipe = ChildPipe::from_inner(fd);
        process::ChildStdout::from_inner(pipe)
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl AsFd for crate::process::ChildStderr {
    #[inline]
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.as_inner().as_fd()
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl From<crate::process::ChildStderr> for OwnedFd {
    /// 取得一个 [`ChildStderr`](crate::process::ChildStderr) 的文件描述符的所有权。
    #[inline]
    fn from(child_stderr: crate::process::ChildStderr) -> OwnedFd {
        child_stderr.into_inner().into_inner()
    }
}

/// 从所提供的 `OwnedFd` 创建一个 `ChildStderr`。
///
/// 所提供的文件描述符必须指向一个设置了 `CLOEXEC` 标志的管道（pipe）。
#[stable(feature = "child_stream_from_fd", since = "1.74.0")]
impl From<OwnedFd> for process::ChildStderr {
    #[inline]
    fn from(fd: OwnedFd) -> process::ChildStderr {
        let pipe = ChildPipe::from_inner(fd);
        process::ChildStderr::from_inner(pipe)
    }
}

/// 返回与此进程的父进程关联的、由操作系统分配的进程标识符。
#[must_use]
#[stable(feature = "unix_ppid", since = "1.27.0")]
pub fn parent_id() -> u32 {
    crate::sys::os::getppid()
}
