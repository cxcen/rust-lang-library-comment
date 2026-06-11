//! 裸（raw）类 Unix 文件描述符。

#![stable(feature = "rust1", since = "1.0.0")]

#[cfg(target_os = "hermit")]
use hermit_abi as libc;
#[cfg(target_os = "motor")]
use moto_rt::libc;

#[cfg(target_os = "motor")]
use super::owned::OwnedFd;
#[cfg(not(target_os = "trusty"))]
use crate::fs;
use crate::io;
#[cfg(target_os = "hermit")]
use crate::os::hermit::io::OwnedFd;
#[cfg(all(not(target_os = "hermit"), not(target_os = "motor")))]
use crate::os::raw;
#[cfg(all(doc, not(target_arch = "wasm32")))]
use crate::os::unix::io::AsFd;
#[cfg(unix)]
use crate::os::unix::io::OwnedFd;
#[cfg(target_os = "wasi")]
use crate::os::wasi::io::OwnedFd;
#[cfg(not(target_os = "trusty"))]
use crate::sys::{AsInner, FromInner, IntoInner};

/// 裸文件描述符。
#[stable(feature = "rust1", since = "1.0.0")]
#[cfg(all(not(target_os = "hermit"), not(target_os = "motor")))]
pub type RawFd = raw::c_int;
#[stable(feature = "rust1", since = "1.0.0")]
#[cfg(any(target_os = "hermit", target_os = "motor"))]
pub type RawFd = i32;

/// 用于从底层对象中提取裸文件描述符的 trait。
///
/// 该 trait 仅在 unix 与 WASI 平台上可用，且必须导入后才能调用其方法。
/// Windows 平台有对应的 `AsRawHandle` 与 `AsRawSocket` 系列 trait。
#[stable(feature = "rust1", since = "1.0.0")]
pub trait AsRawFd {
    /// 提取裸文件描述符。
    ///
    /// 该函数通常用于**借用**一个拥有所有权的文件描述符。以这种方式使用时，
    /// 本方法**不会**把裸文件描述符的所有权转移给调用方，且该文件描述符
    /// 仅在原对象尚未被销毁期间保证有效。
    ///
    /// 然而，借用并非严格必需。若需一个严格执行借用语义的 API，参见
    /// [`AsFd::as_fd`]。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs::File;
    /// # use std::io;
    /// #[cfg(any(unix, target_os = "wasi"))]
    /// use std::os::fd::{AsRawFd, RawFd};
    ///
    /// let mut f = File::open("foo.txt")?;
    /// // 注意 `raw_fd` 仅在 `f` 存在期间有效。
    /// #[cfg(any(unix, target_os = "wasi"))]
    /// let raw_fd: RawFd = f.as_raw_fd();
    /// # Ok::<(), io::Error>(())
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    fn as_raw_fd(&self) -> RawFd;
}

/// 用于表达“能够从裸文件描述符构造对象”这一能力的 trait。
#[stable(feature = "from_raw_os", since = "1.1.0")]
pub trait FromRawFd {
    /// 从给定的裸文件描述符构造一个新的 `Self` 实例。
    ///
    /// 该函数通常用于**取得（consume ownership）**指定文件描述符的所有权。
    /// 以这种方式使用时，返回的对象将负责在其离开作用域时关闭该描述符。
    ///
    /// 然而，取得所有权并非严格必需。若需一个严格执行所有权取得语义的 API，
    /// 请使用 [`From<OwnedFd>::from`] 实现。
    ///
    /// # Safety
    ///
    /// 传入的 `fd` 必须是一个[拥有所有权的文件描述符][io-safety]；
    /// 尤其是，它必须处于打开状态。
    ///
    /// [io-safety]: io#io-safety
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs::File;
    /// # use std::io;
    /// #[cfg(any(unix, target_os = "wasi"))]
    /// use std::os::fd::{FromRawFd, IntoRawFd, RawFd};
    ///
    /// let f = File::open("foo.txt")?;
    /// # #[cfg(any(unix, target_os = "wasi"))]
    /// let raw_fd: RawFd = f.into_raw_fd();
    /// // SAFETY: 不应有其他函数调用 `from_raw_fd`，因此该文件描述符
    /// // 只有一个所有者。
    /// # #[cfg(any(unix, target_os = "wasi"))]
    /// let f = unsafe { File::from_raw_fd(raw_fd) };
    /// # Ok::<(), io::Error>(())
    /// ```
    #[stable(feature = "from_raw_os", since = "1.1.0")]
    unsafe fn from_raw_fd(fd: RawFd) -> Self;
}

/// 用于表达“能够消耗一个对象并获取其裸文件描述符所有权”这一能力的 trait。
#[stable(feature = "into_raw_os", since = "1.4.0")]
pub trait IntoRawFd {
    /// 消耗此对象，返回其底层的裸文件描述符。
    ///
    /// 该函数通常用于把底层文件描述符的所有权**转移（transfer ownership）**给
    /// 调用方。以这种方式使用时，调用方随即成为该文件描述符的唯一所有者，
    /// 并必须在不再需要它时将其关闭。
    ///
    /// 然而，转移所有权并非严格必需。若需一个严格执行所有权转移语义的 API，
    /// 请使用 [`Into<OwnedFd>::into`] 实现。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::fs::File;
    /// # use std::io;
    /// #[cfg(any(unix, target_os = "wasi"))]
    /// use std::os::fd::{IntoRawFd, RawFd};
    ///
    /// let f = File::open("foo.txt")?;
    /// #[cfg(any(unix, target_os = "wasi"))]
    /// let raw_fd: RawFd = f.into_raw_fd();
    /// # Ok::<(), io::Error>(())
    /// ```
    #[must_use = "losing the raw file descriptor may leak resources"]
    #[stable(feature = "into_raw_os", since = "1.4.0")]
    fn into_raw_fd(self) -> RawFd;
}

#[stable(feature = "raw_fd_reflexive_traits", since = "1.48.0")]
impl AsRawFd for RawFd {
    #[inline]
    fn as_raw_fd(&self) -> RawFd {
        *self
    }
}
#[stable(feature = "raw_fd_reflexive_traits", since = "1.48.0")]
impl IntoRawFd for RawFd {
    #[inline]
    fn into_raw_fd(self) -> RawFd {
        self
    }
}
#[stable(feature = "raw_fd_reflexive_traits", since = "1.48.0")]
impl FromRawFd for RawFd {
    #[inline]
    unsafe fn from_raw_fd(fd: RawFd) -> RawFd {
        fd
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
#[cfg(not(target_os = "trusty"))]
impl AsRawFd for fs::File {
    #[inline]
    fn as_raw_fd(&self) -> RawFd {
        self.as_inner().as_raw_fd()
    }
}
#[stable(feature = "from_raw_os", since = "1.1.0")]
#[cfg(not(target_os = "trusty"))]
impl FromRawFd for fs::File {
    #[inline]
    unsafe fn from_raw_fd(fd: RawFd) -> fs::File {
        unsafe { fs::File::from(OwnedFd::from_raw_fd(fd)) }
    }
}
#[stable(feature = "into_raw_os", since = "1.4.0")]
#[cfg(not(target_os = "trusty"))]
impl IntoRawFd for fs::File {
    #[inline]
    fn into_raw_fd(self) -> RawFd {
        self.into_inner().into_inner().into_raw_fd()
    }
}

#[stable(feature = "asraw_stdio", since = "1.21.0")]
#[cfg(not(target_os = "trusty"))]
impl AsRawFd for io::Stdin {
    #[inline]
    fn as_raw_fd(&self) -> RawFd {
        libc::STDIN_FILENO
    }
}

#[stable(feature = "asraw_stdio", since = "1.21.0")]
impl AsRawFd for io::Stdout {
    #[inline]
    fn as_raw_fd(&self) -> RawFd {
        libc::STDOUT_FILENO
    }
}

#[stable(feature = "asraw_stdio", since = "1.21.0")]
impl AsRawFd for io::Stderr {
    #[inline]
    fn as_raw_fd(&self) -> RawFd {
        libc::STDERR_FILENO
    }
}

#[stable(feature = "asraw_stdio_locks", since = "1.35.0")]
#[cfg(not(target_os = "trusty"))]
impl<'a> AsRawFd for io::StdinLock<'a> {
    #[inline]
    fn as_raw_fd(&self) -> RawFd {
        libc::STDIN_FILENO
    }
}

#[stable(feature = "asraw_stdio_locks", since = "1.35.0")]
impl<'a> AsRawFd for io::StdoutLock<'a> {
    #[inline]
    fn as_raw_fd(&self) -> RawFd {
        libc::STDOUT_FILENO
    }
}

#[stable(feature = "asraw_stdio_locks", since = "1.35.0")]
impl<'a> AsRawFd for io::StderrLock<'a> {
    #[inline]
    fn as_raw_fd(&self) -> RawFd {
        libc::STDERR_FILENO
    }
}

/// 此 impl 使得可以在 Arc 上实现那些要求 `AsRawFd` 的 trait。
/// ```
/// # #[cfg(any(unix, target_os = "wasi"))] mod group_cfg {
/// # #[cfg(target_os = "wasi")]
/// # use std::os::wasi::io::AsRawFd;
/// # #[cfg(unix)]
/// # use std::os::unix::io::AsRawFd;
/// use std::net::UdpSocket;
/// use std::sync::Arc;
/// trait MyTrait: AsRawFd {
/// }
/// impl MyTrait for Arc<UdpSocket> {}
/// impl MyTrait for Box<UdpSocket> {}
/// # }
/// ```
#[stable(feature = "asrawfd_ptrs", since = "1.63.0")]
impl<T: AsRawFd> AsRawFd for crate::sync::Arc<T> {
    #[inline]
    fn as_raw_fd(&self) -> RawFd {
        (**self).as_raw_fd()
    }
}

#[stable(feature = "asfd_rc", since = "1.69.0")]
impl<T: AsRawFd> AsRawFd for crate::rc::Rc<T> {
    #[inline]
    fn as_raw_fd(&self) -> RawFd {
        (**self).as_raw_fd()
    }
}

#[unstable(feature = "unique_rc_arc", issue = "112566")]
impl<T: AsRawFd + ?Sized> AsRawFd for crate::rc::UniqueRc<T> {
    #[inline]
    fn as_raw_fd(&self) -> RawFd {
        (**self).as_raw_fd()
    }
}

#[stable(feature = "asrawfd_ptrs", since = "1.63.0")]
impl<T: AsRawFd> AsRawFd for Box<T> {
    #[inline]
    fn as_raw_fd(&self) -> RawFd {
        (**self).as_raw_fd()
    }
}

#[stable(feature = "anonymous_pipe", since = "1.87.0")]
#[cfg(not(target_os = "trusty"))]
impl AsRawFd for io::PipeReader {
    fn as_raw_fd(&self) -> RawFd {
        self.0.as_raw_fd()
    }
}

#[stable(feature = "anonymous_pipe", since = "1.87.0")]
#[cfg(not(target_os = "trusty"))]
impl FromRawFd for io::PipeReader {
    unsafe fn from_raw_fd(raw_fd: RawFd) -> Self {
        Self::from_inner(unsafe { FromRawFd::from_raw_fd(raw_fd) })
    }
}

#[stable(feature = "anonymous_pipe", since = "1.87.0")]
#[cfg(not(target_os = "trusty"))]
impl IntoRawFd for io::PipeReader {
    fn into_raw_fd(self) -> RawFd {
        self.0.into_raw_fd()
    }
}

#[stable(feature = "anonymous_pipe", since = "1.87.0")]
#[cfg(not(target_os = "trusty"))]
impl AsRawFd for io::PipeWriter {
    fn as_raw_fd(&self) -> RawFd {
        self.0.as_raw_fd()
    }
}

#[stable(feature = "anonymous_pipe", since = "1.87.0")]
#[cfg(not(target_os = "trusty"))]
impl FromRawFd for io::PipeWriter {
    unsafe fn from_raw_fd(raw_fd: RawFd) -> Self {
        Self::from_inner(unsafe { FromRawFd::from_raw_fd(raw_fd) })
    }
}

#[stable(feature = "anonymous_pipe", since = "1.87.0")]
#[cfg(not(target_os = "trusty"))]
impl IntoRawFd for io::PipeWriter {
    fn into_raw_fd(self) -> RawFd {
        self.0.into_raw_fd()
    }
}
