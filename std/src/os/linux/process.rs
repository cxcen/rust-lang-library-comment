//! 针对 [`std::process`] 模块中各类型的 Linux 平台特有扩展。
//!
//! [`std::process`]: crate::process

#![unstable(feature = "linux_pidfd", issue = "82971")]

use crate::io::Result;
use crate::os::unix::io::{AsFd, AsRawFd, BorrowedFd, FromRawFd, IntoRawFd, OwnedFd, RawFd};
use crate::process::{self, ExitStatus};
use crate::sealed::Sealed;
use crate::sys::{AsInner, AsInnerMut, FromInner, IntoInner};
#[cfg(not(doc))]
use crate::sys::{fd::FileDesc, linux::pidfd::PidFd as InnerPidFd};

#[cfg(doc)]
struct InnerPidFd;

/// 该类型表示一个引用某个进程的文件描述符（file descriptor）。
///
/// 可以通过在 [`Command`] 上调用 [`create_pidfd`] 设置相应选项来获得 `PidFd`。
/// 随后，创建出的 pidfd 可以通过对 [`Child`] 调用 [`pidfd`] 或 [`into_pidfd`] 取得。
///
/// 示例：
/// ```no_run
/// #![feature(linux_pidfd)]
/// use std::os::linux::process::{CommandExt, ChildExt};
/// use std::process::Command;
///
/// let mut child = Command::new("echo")
///     .create_pidfd(true)
///     .spawn()
///     .expect("Failed to spawn child");
///
/// let pidfd = child
///     .into_pidfd()
///     .expect("Failed to retrieve pidfd");
///
/// // 当 `pidfd` 被丢弃（drop）时，该文件描述符将被关闭。
/// ```
/// 更多细节请参阅 [`pidfd_open(2)`] 的手册页（man page）。
///
/// [`Command`]: process::Command
/// [`create_pidfd`]: CommandExt::create_pidfd
/// [`Child`]: process::Child
/// [`pidfd`]: fn@ChildExt::pidfd
/// [`into_pidfd`]: ChildExt::into_pidfd
/// [`pidfd_open(2)`]: https://man7.org/linux/man-pages/man2/pidfd_open.2.html
#[derive(Debug)]
#[repr(transparent)]
pub struct PidFd {
    inner: InnerPidFd,
}

impl PidFd {
    /// 强制子进程退出。
    ///
    /// 与 [`Child::kill`] 不同，对已被回收（reaped）的子进程也可以尝试 kill，
    /// 因为 PidFd 不会受 pid 复用（recycling）竞争的影响。但这样做会返回一个 Error。
    ///
    /// [`Child::kill`]: process::Child::kill
    pub fn kill(&self) -> Result<()> {
        self.inner.kill()
    }

    /// 等待子进程完全退出，并返回它退出时的状态。
    ///
    /// 与 [`Child::wait`] 不同，本方法不会确保 stdin 句柄被关闭。
    ///
    /// 此外，在 6.15 之前的内核上，只有第一次回收（reap）子进程的尝试
    /// 会返回 ExitStatus，后续尝试将返回 Error。
    ///
    /// [`Child::wait`]: process::Child::wait
    pub fn wait(&self) -> Result<ExitStatus> {
        self.inner.wait().map(FromInner::from_inner)
    }

    /// 若子进程已经退出，则尝试收集其退出状态。
    ///
    /// 在 6.15 之前的内核上，并且与 [`Child::try_wait`] 不同，只有第一次回收（reap）
    /// 子进程的尝试会返回 ExitStatus，后续尝试将返回 Error。
    ///
    /// [`Child::try_wait`]: process::Child::try_wait
    pub fn try_wait(&self) -> Result<Option<ExitStatus>> {
        Ok(self.inner.try_wait()?.map(FromInner::from_inner))
    }
}

impl AsInner<InnerPidFd> for PidFd {
    #[inline]
    fn as_inner(&self) -> &InnerPidFd {
        &self.inner
    }
}

impl FromInner<InnerPidFd> for PidFd {
    fn from_inner(inner: InnerPidFd) -> PidFd {
        PidFd { inner }
    }
}

impl IntoInner<InnerPidFd> for PidFd {
    fn into_inner(self) -> InnerPidFd {
        self.inner
    }
}

impl AsRawFd for PidFd {
    #[inline]
    fn as_raw_fd(&self) -> RawFd {
        self.as_inner().as_inner().as_raw_fd()
    }
}

impl FromRawFd for PidFd {
    unsafe fn from_raw_fd(fd: RawFd) -> Self {
        Self::from_inner(InnerPidFd::from_raw_fd(fd))
    }
}

impl IntoRawFd for PidFd {
    fn into_raw_fd(self) -> RawFd {
        self.into_inner().into_inner().into_raw_fd()
    }
}

impl AsFd for PidFd {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.as_inner().as_inner().as_fd()
    }
}

impl From<OwnedFd> for PidFd {
    fn from(fd: OwnedFd) -> Self {
        Self::from_inner(InnerPidFd::from_inner(FileDesc::from_inner(fd)))
    }
}

impl From<PidFd> for OwnedFd {
    fn from(pid_fd: PidFd) -> Self {
        pid_fd.into_inner().into_inner().into_inner()
    }
}

/// 针对 [`Child`] 的 OS 平台特有扩展。
///
/// [`Child`]: process::Child
pub trait ChildExt: Sealed {
    /// 获取为此 [`Child`] 创建的 [`PidFd`] 的引用（如果可用）。
    ///
    /// 仅当在创建对应的 [`Command`] 时通过 [`create_pidfd`] 请求过创建 pidfd，
    /// 该 pidfd 才会可用。
    ///
    /// 即便请求过，pidfd 也可能因为使用的 Linux 版本过旧、或发生了其他错误而不可用。
    ///
    /// [`Command`]: process::Command
    /// [`create_pidfd`]: CommandExt::create_pidfd
    /// [`Child`]: process::Child
    fn pidfd(&self) -> Result<&PidFd>;

    /// 返回为此 [`Child`] 创建的 [`PidFd`]（如果可用）。否则返回 self。
    ///
    /// 仅当在创建对应的 [`Command`] 时通过 [`create_pidfd`] 请求过创建 pidfd，
    /// 该 pidfd 才会可用。
    ///
    /// 取得 PidFd 的所有权会消耗（consume）掉 Child，以避免 pid 复用竞争。
    /// 如果你还不想拆解（disassemble）Child，可改用 [`pidfd`] 与
    /// [`BorrowedFd::try_clone_to_owned`]。
    ///
    /// 即便请求过，pidfd 也可能因为使用的 Linux 版本过旧、或发生了其他错误而不可用。
    ///
    /// [`Command`]: process::Command
    /// [`create_pidfd`]: CommandExt::create_pidfd
    /// [`pidfd`]: ChildExt::pidfd
    /// [`Child`]: process::Child
    fn into_pidfd(self) -> crate::result::Result<PidFd, Self>
    where
        Self: Sized;
}

/// 针对 [`Command`] 的 OS 平台特有扩展。
///
/// [`Command`]: process::Command
pub trait CommandExt: Sealed {
    /// 设置是否应为此 [`Command`] 派生（spawn）的 [`Child`] 创建一个
    /// [`PidFd`](struct@PidFd)。默认不创建任何 pidfd。
    ///
    /// 该 pidfd 可以通过 [`pidfd`] 或 [`into_pidfd`] 从子进程取得。
    ///
    /// 只有在能够以保证无竞争（race-free）的方式创建时才会创建 pidfd。
    /// 否则，[`pidfd`] 将返回一个错误。
    ///
    /// 如果 pidfd 已成功创建且未从 `Child` 中取走，那么对 `kill()`、`wait()`
    /// 和 `try_wait()` 的调用将使用该 pidfd 而非 pid。这可以避免 pid 复用竞争，
    /// 例如那些由同一进程内的流氓库（rogue libraries）通过 `waitpid(-1, ...)`
    /// 调用过早回收僵尸子进程（zombie children）所引发的竞争。
    ///
    /// [`Command`]: process::Command
    /// [`Child`]: process::Child
    /// [`pidfd`]: fn@ChildExt::pidfd
    /// [`into_pidfd`]: ChildExt::into_pidfd
    fn create_pidfd(&mut self, val: bool) -> &mut process::Command;
}

impl CommandExt for process::Command {
    fn create_pidfd(&mut self, val: bool) -> &mut process::Command {
        self.as_inner_mut().create_pidfd(val);
        self
    }
}
