//! Windows 平台对通用 I/O 原语的特定扩展。

#![stable(feature = "rust1", since = "1.0.0")]

#[cfg(doc)]
use crate::os::windows::io::{AsHandle, AsSocket};
use crate::os::windows::io::{OwnedHandle, OwnedSocket};
use crate::os::windows::raw;
use crate::sys::{AsInner, FromInner, IntoInner};
use crate::{fs, io, net, ptr, sys};

/// 裸 HANDLE。
#[stable(feature = "rust1", since = "1.0.0")]
pub type RawHandle = raw::HANDLE;

/// 裸 SOCKET。
#[stable(feature = "rust1", since = "1.0.0")]
pub type RawSocket = raw::SOCKET;

/// 提取裸 handle。
#[stable(feature = "rust1", since = "1.0.0")]
pub trait AsRawHandle {
    /// 提取裸 handle。
    ///
    /// 本函数通常用于 **借用** 一个拥有式 handle。以这种方式使用时，本方法 **不会**
    /// 把裸 handle 的所有权转交给调用者，并且只有在原对象尚未被销毁期间，该 handle
    /// 才保证有效。
    ///
    /// 本函数可能返回 null，例如在控制台已脱离时对 [`Stdin`]、[`Stdout`] 或 [`Stderr`]
    /// 调用时。
    ///
    /// 不过，借用并非严格必需。若需要严格借用 handle 的 API，参见
    /// [`AsHandle::as_handle`]。
    ///
    /// [`Stdin`]: io::Stdin
    /// [`Stdout`]: io::Stdout
    /// [`Stderr`]: io::Stderr
    #[stable(feature = "rust1", since = "1.0.0")]
    fn as_raw_handle(&self) -> RawHandle;
}

/// 从裸 handle 构造 I/O 对象。
#[stable(feature = "from_raw_os", since = "1.1.0")]
pub trait FromRawHandle {
    /// 从指定的裸 handle 构造一个新的 I/O 对象。
    ///
    /// 本函数通常用于 **消耗（获取）** 所给 handle 的所有权，把关闭该 handle 的责任
    /// 转交给所返回的对象。以这种方式使用时，所返回的对象将在其离开作用域时负责关闭它。
    ///
    /// 不过，消耗所有权并非严格必需。若需要严格消耗所有权的 API，请使用
    /// `From<OwnedHandle>::from` 实现。
    ///
    /// # 安全性(Safety）
    ///
    /// 传入的 `handle` 必须：
    ///   - 是一个 [拥有式 handle][io-safety]；特别地，它必须处于打开状态。
    ///   - 是一个可经由 [`CloseHandle`] 释放的资源 handle（而不是需要 `RegCloseKey`
    ///     或其他关闭函数的那种）。
    ///
    /// 注意该 handle *可能* 取值为 `INVALID_HANDLE_VALUE`（-1），而该值有时是一个有效的
    /// handle 值。完整来龙去脉见 [here]。
    ///
    /// [`CloseHandle`]: https://docs.microsoft.com/en-us/windows/win32/api/handleapi/nf-handleapi-closehandle
    /// [here]: https://devblogs.microsoft.com/oldnewthing/20040302-00/?p=40443
    /// [io-safety]: io#io-safety
    #[stable(feature = "from_raw_os", since = "1.1.0")]
    unsafe fn from_raw_handle(handle: RawHandle) -> Self;
}

/// 一个 trait，用于表达消耗某个对象并获取其裸 `HANDLE` 所有权的能力。
#[stable(feature = "into_raw_os", since = "1.4.0")]
pub trait IntoRawHandle {
    /// 消耗本对象，返回其底层的裸 handle。
    ///
    /// 本函数通常用于把底层 handle 的 **所有权转交** 给调用者。以这种方式使用时，
    /// 调用者随后即成为该 handle 的唯一所有者，并且必须在不再需要它时将其关闭。
    ///
    /// 不过，转交所有权并非严格必需。若需要严格转交所有权的 API，请使用
    /// `Into<OwnedHandle>::into` 实现。
    #[must_use = "losing the raw handle may leak resources"]
    #[stable(feature = "into_raw_os", since = "1.4.0")]
    fn into_raw_handle(self) -> RawHandle;
}

#[stable(feature = "rust1", since = "1.0.0")]
impl AsRawHandle for fs::File {
    #[inline]
    fn as_raw_handle(&self) -> RawHandle {
        self.as_inner().as_raw_handle() as RawHandle
    }
}

#[stable(feature = "asraw_stdio", since = "1.21.0")]
impl AsRawHandle for io::Stdin {
    fn as_raw_handle(&self) -> RawHandle {
        stdio_handle(unsafe { sys::c::GetStdHandle(sys::c::STD_INPUT_HANDLE) as RawHandle })
    }
}

#[stable(feature = "asraw_stdio", since = "1.21.0")]
impl AsRawHandle for io::Stdout {
    fn as_raw_handle(&self) -> RawHandle {
        stdio_handle(unsafe { sys::c::GetStdHandle(sys::c::STD_OUTPUT_HANDLE) as RawHandle })
    }
}

#[stable(feature = "asraw_stdio", since = "1.21.0")]
impl AsRawHandle for io::Stderr {
    fn as_raw_handle(&self) -> RawHandle {
        stdio_handle(unsafe { sys::c::GetStdHandle(sys::c::STD_ERROR_HANDLE) as RawHandle })
    }
}

#[stable(feature = "asraw_stdio_locks", since = "1.35.0")]
impl<'a> AsRawHandle for io::StdinLock<'a> {
    fn as_raw_handle(&self) -> RawHandle {
        stdio_handle(unsafe { sys::c::GetStdHandle(sys::c::STD_INPUT_HANDLE) as RawHandle })
    }
}

#[stable(feature = "asraw_stdio_locks", since = "1.35.0")]
impl<'a> AsRawHandle for io::StdoutLock<'a> {
    fn as_raw_handle(&self) -> RawHandle {
        stdio_handle(unsafe { sys::c::GetStdHandle(sys::c::STD_OUTPUT_HANDLE) as RawHandle })
    }
}

#[stable(feature = "asraw_stdio_locks", since = "1.35.0")]
impl<'a> AsRawHandle for io::StderrLock<'a> {
    fn as_raw_handle(&self) -> RawHandle {
        stdio_handle(unsafe { sys::c::GetStdHandle(sys::c::STD_ERROR_HANDLE) as RawHandle })
    }
}

// 把从 `GetStdHandle` 返回的 handle 转换为要返回给用户的 handle。
fn stdio_handle(raw: RawHandle) -> RawHandle {
    // `GetStdHandle` 预期实际上不会失败，因此当它返回 `INVALID_HANDLE_VALUE` 时，
    // 意味着我们是被某个未向我们提供 stdio handle 的父进程启动的，例如一个控制台已脱离的
    // 父进程。在那种情况下，我们向用户返回 null——这与用户在父进程中得到的结果一致，
    // 同时也避免了 `INVALID_HANDLE_VALUE` 与当前进程 handle 取值相同（别名）所带来的问题。
    if raw == sys::c::INVALID_HANDLE_VALUE { ptr::null_mut() } else { raw }
}

#[stable(feature = "from_raw_os", since = "1.1.0")]
impl FromRawHandle for fs::File {
    #[inline]
    unsafe fn from_raw_handle(handle: RawHandle) -> fs::File {
        unsafe {
            let handle = handle as sys::c::HANDLE;
            fs::File::from_inner(sys::fs::File::from_inner(FromInner::from_inner(
                OwnedHandle::from_raw_handle(handle),
            )))
        }
    }
}

#[stable(feature = "into_raw_os", since = "1.4.0")]
impl IntoRawHandle for fs::File {
    #[inline]
    fn into_raw_handle(self) -> RawHandle {
        self.into_inner().into_raw_handle() as *mut _
    }
}

/// 提取裸 socket。
#[stable(feature = "rust1", since = "1.0.0")]
pub trait AsRawSocket {
    /// 提取裸 socket。
    ///
    /// 本函数通常用于 **借用** 一个拥有式 socket。以这种方式使用时，本方法 **不会**
    /// 把裸 socket 的所有权转交给调用者，并且只有在原对象尚未被销毁期间，该 socket
    /// 才保证有效。
    ///
    /// 不过，借用并非严格必需。若需要严格借用 socket 的 API，参见
    /// [`AsSocket::as_socket`]。
    #[stable(feature = "rust1", since = "1.0.0")]
    fn as_raw_socket(&self) -> RawSocket;
}

/// 从裸 socket 创建 I/O 对象。
#[stable(feature = "from_raw_os", since = "1.1.0")]
pub trait FromRawSocket {
    /// 从指定的裸 socket 构造一个新的 I/O 对象。
    ///
    /// 本函数通常用于 **消耗（获取）** 所给 socket 的所有权，把关闭该 socket 的责任
    /// 转交给所返回的对象。以这种方式使用时，所返回的对象将在其离开作用域时负责关闭它。
    ///
    /// 不过，消耗所有权并非严格必需。若需要严格消耗所有权的 API，请使用
    /// `From<OwnedSocket>::from` 实现。
    ///
    /// # 安全性(Safety）
    ///
    /// 传入的 `socket` 必须：
    ///   - 是一个 [拥有式 socket][io-safety]；特别地，它必须处于打开状态。
    ///   - 是一个可经由 [`closesocket`] 释放的 socket。
    ///
    /// [`closesocket`]: https://docs.microsoft.com/en-us/windows/win32/api/winsock2/nf-winsock2-closesocket
    /// [io-safety]: io#io-safety
    #[stable(feature = "from_raw_os", since = "1.1.0")]
    unsafe fn from_raw_socket(sock: RawSocket) -> Self;
}

/// 一个 trait，用于表达消耗某个对象并获取其裸 `SOCKET` 所有权的能力。
#[stable(feature = "into_raw_os", since = "1.4.0")]
pub trait IntoRawSocket {
    /// 消耗本对象，返回其底层的裸 socket。
    ///
    /// 本函数通常用于把底层 socket 的 **所有权转交** 给调用者。以这种方式使用时，
    /// 调用者随后即成为该 socket 的唯一所有者，并且必须在不再需要它时将其关闭。
    ///
    /// 不过，转交所有权并非严格必需。若需要严格转交所有权的 API，请使用
    /// `Into<OwnedSocket>::into` 实现。
    #[must_use = "losing the raw socket may leak resources"]
    #[stable(feature = "into_raw_os", since = "1.4.0")]
    fn into_raw_socket(self) -> RawSocket;
}

#[stable(feature = "rust1", since = "1.0.0")]
impl AsRawSocket for net::TcpStream {
    #[inline]
    fn as_raw_socket(&self) -> RawSocket {
        self.as_inner().socket().as_raw_socket()
    }
}
#[stable(feature = "rust1", since = "1.0.0")]
impl AsRawSocket for net::TcpListener {
    #[inline]
    fn as_raw_socket(&self) -> RawSocket {
        self.as_inner().socket().as_raw_socket()
    }
}
#[stable(feature = "rust1", since = "1.0.0")]
impl AsRawSocket for net::UdpSocket {
    #[inline]
    fn as_raw_socket(&self) -> RawSocket {
        self.as_inner().socket().as_raw_socket()
    }
}

#[stable(feature = "from_raw_os", since = "1.1.0")]
impl FromRawSocket for net::TcpStream {
    #[inline]
    unsafe fn from_raw_socket(sock: RawSocket) -> net::TcpStream {
        unsafe {
            let sock = sys::net::Socket::from_inner(OwnedSocket::from_raw_socket(sock));
            net::TcpStream::from_inner(sys::net::TcpStream::from_inner(sock))
        }
    }
}
#[stable(feature = "from_raw_os", since = "1.1.0")]
impl FromRawSocket for net::TcpListener {
    #[inline]
    unsafe fn from_raw_socket(sock: RawSocket) -> net::TcpListener {
        unsafe {
            let sock = sys::net::Socket::from_inner(OwnedSocket::from_raw_socket(sock));
            net::TcpListener::from_inner(sys::net::TcpListener::from_inner(sock))
        }
    }
}
#[stable(feature = "from_raw_os", since = "1.1.0")]
impl FromRawSocket for net::UdpSocket {
    #[inline]
    unsafe fn from_raw_socket(sock: RawSocket) -> net::UdpSocket {
        unsafe {
            let sock = sys::net::Socket::from_inner(OwnedSocket::from_raw_socket(sock));
            net::UdpSocket::from_inner(sys::net::UdpSocket::from_inner(sock))
        }
    }
}

#[stable(feature = "into_raw_os", since = "1.4.0")]
impl IntoRawSocket for net::TcpStream {
    #[inline]
    fn into_raw_socket(self) -> RawSocket {
        self.into_inner().into_socket().into_inner().into_raw_socket()
    }
}

#[stable(feature = "into_raw_os", since = "1.4.0")]
impl IntoRawSocket for net::TcpListener {
    #[inline]
    fn into_raw_socket(self) -> RawSocket {
        self.into_inner().into_socket().into_inner().into_raw_socket()
    }
}

#[stable(feature = "into_raw_os", since = "1.4.0")]
impl IntoRawSocket for net::UdpSocket {
    #[inline]
    fn into_raw_socket(self) -> RawSocket {
        self.into_inner().into_socket().into_inner().into_raw_socket()
    }
}

#[stable(feature = "anonymous_pipe", since = "1.87.0")]
impl AsRawHandle for io::PipeReader {
    fn as_raw_handle(&self) -> RawHandle {
        self.0.as_raw_handle()
    }
}

#[stable(feature = "anonymous_pipe", since = "1.87.0")]
impl FromRawHandle for io::PipeReader {
    unsafe fn from_raw_handle(raw_handle: RawHandle) -> Self {
        unsafe { Self::from_inner(FromRawHandle::from_raw_handle(raw_handle)) }
    }
}

#[stable(feature = "anonymous_pipe", since = "1.87.0")]
impl IntoRawHandle for io::PipeReader {
    fn into_raw_handle(self) -> RawHandle {
        self.0.into_raw_handle()
    }
}

#[stable(feature = "anonymous_pipe", since = "1.87.0")]
impl AsRawHandle for io::PipeWriter {
    fn as_raw_handle(&self) -> RawHandle {
        self.0.as_raw_handle()
    }
}

#[stable(feature = "anonymous_pipe", since = "1.87.0")]
impl FromRawHandle for io::PipeWriter {
    unsafe fn from_raw_handle(raw_handle: RawHandle) -> Self {
        unsafe { Self::from_inner(FromRawHandle::from_raw_handle(raw_handle)) }
    }
}

#[stable(feature = "anonymous_pipe", since = "1.87.0")]
impl IntoRawHandle for io::PipeWriter {
    fn into_raw_handle(self) -> RawHandle {
        self.0.into_raw_handle()
    }
}
