//! 拥有式（owned）与借用式（borrowed）的操作系统 socket。

#![stable(feature = "io_safety", since = "1.63.0")]

use super::raw::{AsRawSocket, FromRawSocket, IntoRawSocket, RawSocket};
use crate::marker::PhantomData;
use crate::mem::{self, ManuallyDrop};
#[cfg(not(target_vendor = "uwp"))]
use crate::sys::cvt;
use crate::{fmt, io, sys};

// 这里的最大值是 -2（以二进制补码表示）。-1 即 `INVALID_SOCKET`。
type ValidRawSocket = core::num::niche_types::NotAllOnes<RawSocket>;

/// 一个借用式的 socket。
///
/// 它带有一个生命周期参数，用以将自身绑定到拥有该 socket 的某个对象的生命周期上。
///
/// 它采用 `repr(transparent)`，与宿主机 socket 具有相同的表示，因此可以在 FFI 中用于
/// 那些以参数形式传入 socket、且 socket 不会被捕获或消耗的场合，并且它的取值永远不会是
/// `INVALID_SOCKET`。
///
/// 本类型的 `.to_owned()` 实现返回的是另一个 `BorrowedSocket` 而不是 `OwnedSocket`。
/// 它只是对裸 socket 做一次平凡的拷贝，随后在同一个生命周期下被借用。
#[derive(Copy, Clone)]
#[repr(transparent)]
#[rustc_nonnull_optimization_guaranteed]
#[stable(feature = "io_safety", since = "1.63.0")]
pub struct BorrowedSocket<'socket> {
    socket: ValidRawSocket,
    _phantom: PhantomData<&'socket OwnedSocket>,
}

/// 一个拥有式的 socket。
///
/// 它会在 drop 时关闭该 socket。
///
/// 它采用 `repr(transparent)`，与宿主机 socket 具有相同的表示，因此可以在 FFI 中用于
/// 那些以被消耗的参数形式传入 socket、或以拥有式值返回 socket 的场合，并且它的取值永远
/// 不会是 `INVALID_SOCKET`。
#[repr(transparent)]
#[rustc_nonnull_optimization_guaranteed]
#[stable(feature = "io_safety", since = "1.63.0")]
pub struct OwnedSocket {
    socket: ValidRawSocket,
}

impl BorrowedSocket<'_> {
    /// 返回一个持有给定裸 socket 的 `BorrowedSocket`。
    ///
    /// # 安全性(Safety）
    ///
    /// `socket` 所指向的资源必须在所返回的 `BorrowedSocket` 的整个存续期间保持打开状态，
    /// 并且它的取值不得为 `INVALID_SOCKET`。
    #[inline]
    #[track_caller]
    #[rustc_const_stable(feature = "io_safety", since = "1.63.0")]
    #[stable(feature = "io_safety", since = "1.63.0")]
    pub const unsafe fn borrow_raw(socket: RawSocket) -> Self {
        Self { socket: ValidRawSocket::new(socket).expect("socket != -1"), _phantom: PhantomData }
    }
}

impl OwnedSocket {
    /// 创建一个新的 `OwnedSocket` 实例，它与现有的 `OwnedSocket` 实例共享同一个底层对象。
    #[stable(feature = "io_safety", since = "1.63.0")]
    pub fn try_clone(&self) -> io::Result<Self> {
        self.as_socket().try_clone_to_owned()
    }

    // FIXME(strict_provenance_magic): 我们当初把 RawSocket 定义成了 u64 ;-;
    #[allow(fuzzy_provenance_casts)]
    #[cfg(not(target_vendor = "uwp"))]
    pub(crate) fn set_no_inherit(&self) -> io::Result<()> {
        cvt(unsafe {
            sys::c::SetHandleInformation(
                self.as_raw_socket() as sys::c::HANDLE,
                sys::c::HANDLE_FLAG_INHERIT,
                0,
            )
        })
        .map(drop)
    }

    #[cfg(target_vendor = "uwp")]
    pub(crate) fn set_no_inherit(&self) -> io::Result<()> {
        Err(io::const_error!(io::ErrorKind::Unsupported, "unavailable on UWP"))
    }
}

impl BorrowedSocket<'_> {
    /// 创建一个新的 `OwnedSocket` 实例，它与现有的 `BorrowedSocket` 实例共享同一个底层对象。
    #[stable(feature = "io_safety", since = "1.63.0")]
    pub fn try_clone_to_owned(&self) -> io::Result<OwnedSocket> {
        let mut info = unsafe { mem::zeroed::<sys::c::WSAPROTOCOL_INFOW>() };
        let result = unsafe {
            sys::c::WSADuplicateSocketW(
                self.as_raw_socket() as sys::c::SOCKET,
                sys::c::GetCurrentProcessId(),
                &mut info,
            )
        };
        sys::net::cvt(result)?;
        let socket = unsafe {
            sys::c::WSASocketW(
                info.iAddressFamily,
                info.iSocketType,
                info.iProtocol,
                &info,
                0,
                sys::c::WSA_FLAG_OVERLAPPED | sys::c::WSA_FLAG_NO_HANDLE_INHERIT,
            )
        };

        if socket != sys::c::INVALID_SOCKET {
            unsafe { Ok(OwnedSocket::from_raw_socket(socket as RawSocket)) }
        } else {
            let error = unsafe { sys::c::WSAGetLastError() };

            if error != sys::c::WSAEPROTOTYPE && error != sys::c::WSAEINVAL {
                return Err(io::Error::from_raw_os_error(error));
            }

            let socket = unsafe {
                sys::c::WSASocketW(
                    info.iAddressFamily,
                    info.iSocketType,
                    info.iProtocol,
                    &info,
                    0,
                    sys::c::WSA_FLAG_OVERLAPPED,
                )
            };

            if socket == sys::c::INVALID_SOCKET {
                return Err(last_error());
            }

            unsafe {
                let socket = OwnedSocket::from_raw_socket(socket as RawSocket);
                socket.set_no_inherit()?;
                Ok(socket)
            }
        }
    }
}

/// 返回来自 Windows socket 接口的最近一次错误。
fn last_error() -> io::Error {
    io::Error::from_raw_os_error(unsafe { sys::c::WSAGetLastError() })
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl AsRawSocket for BorrowedSocket<'_> {
    #[inline]
    fn as_raw_socket(&self) -> RawSocket {
        self.socket.as_inner()
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl AsRawSocket for OwnedSocket {
    #[inline]
    fn as_raw_socket(&self) -> RawSocket {
        self.socket.as_inner()
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl IntoRawSocket for OwnedSocket {
    #[inline]
    fn into_raw_socket(self) -> RawSocket {
        ManuallyDrop::new(self).socket.as_inner()
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl FromRawSocket for OwnedSocket {
    #[inline]
    #[track_caller]
    unsafe fn from_raw_socket(socket: RawSocket) -> Self {
        Self { socket: ValidRawSocket::new(socket).expect("socket != -1") }
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl Drop for OwnedSocket {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            let _ = sys::c::closesocket(self.socket.as_inner() as sys::c::SOCKET);
        }
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl fmt::Debug for BorrowedSocket<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BorrowedSocket").field("socket", &self.socket).finish()
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl fmt::Debug for OwnedSocket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OwnedSocket").field("socket", &self.socket).finish()
    }
}

/// 用于从某个底层对象借出其 socket 的 trait。
#[stable(feature = "io_safety", since = "1.63.0")]
pub trait AsSocket {
    /// 借出该 socket。
    #[stable(feature = "io_safety", since = "1.63.0")]
    fn as_socket(&self) -> BorrowedSocket<'_>;
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl<T: AsSocket> AsSocket for &T {
    #[inline]
    fn as_socket(&self) -> BorrowedSocket<'_> {
        T::as_socket(self)
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl<T: AsSocket> AsSocket for &mut T {
    #[inline]
    fn as_socket(&self) -> BorrowedSocket<'_> {
        T::as_socket(self)
    }
}

#[stable(feature = "as_windows_ptrs", since = "1.71.0")]
/// 这个 impl 使得可以在 Arc 上实现那些要求 `AsSocket` 的 trait。
/// ```
/// # #[cfg(windows)] mod group_cfg {
/// # use std::os::windows::io::AsSocket;
/// use std::net::UdpSocket;
/// use std::sync::Arc;
///
/// trait MyTrait: AsSocket {}
/// impl MyTrait for Arc<UdpSocket> {}
/// impl MyTrait for Box<UdpSocket> {}
/// # }
/// ```
impl<T: AsSocket> AsSocket for crate::sync::Arc<T> {
    #[inline]
    fn as_socket(&self) -> BorrowedSocket<'_> {
        (**self).as_socket()
    }
}

#[stable(feature = "as_windows_ptrs", since = "1.71.0")]
impl<T: AsSocket> AsSocket for crate::rc::Rc<T> {
    #[inline]
    fn as_socket(&self) -> BorrowedSocket<'_> {
        (**self).as_socket()
    }
}

#[unstable(feature = "unique_rc_arc", issue = "112566")]
impl<T: AsSocket + ?Sized> AsSocket for crate::rc::UniqueRc<T> {
    #[inline]
    fn as_socket(&self) -> BorrowedSocket<'_> {
        (**self).as_socket()
    }
}

#[stable(feature = "as_windows_ptrs", since = "1.71.0")]
impl<T: AsSocket> AsSocket for Box<T> {
    #[inline]
    fn as_socket(&self) -> BorrowedSocket<'_> {
        (**self).as_socket()
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl AsSocket for BorrowedSocket<'_> {
    #[inline]
    fn as_socket(&self) -> BorrowedSocket<'_> {
        *self
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl AsSocket for OwnedSocket {
    #[inline]
    fn as_socket(&self) -> BorrowedSocket<'_> {
        // Safety: `OwnedSocket` 与 `BorrowedSocket` 具有相同的有效性不变量，并且这个
        // `BorrowedSocket` 的生命周期受 `&self` 约束。
        unsafe { BorrowedSocket::borrow_raw(self.as_raw_socket()) }
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl AsSocket for crate::net::TcpStream {
    #[inline]
    fn as_socket(&self) -> BorrowedSocket<'_> {
        unsafe { BorrowedSocket::borrow_raw(self.as_raw_socket()) }
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl From<crate::net::TcpStream> for OwnedSocket {
    /// 接管一个 [`TcpStream`](crate::net::TcpStream) 的 socket 的所有权。
    #[inline]
    fn from(tcp_stream: crate::net::TcpStream) -> OwnedSocket {
        unsafe { OwnedSocket::from_raw_socket(tcp_stream.into_raw_socket()) }
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl From<OwnedSocket> for crate::net::TcpStream {
    #[inline]
    fn from(owned: OwnedSocket) -> Self {
        unsafe { Self::from_raw_socket(owned.into_raw_socket()) }
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl AsSocket for crate::net::TcpListener {
    #[inline]
    fn as_socket(&self) -> BorrowedSocket<'_> {
        unsafe { BorrowedSocket::borrow_raw(self.as_raw_socket()) }
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl From<crate::net::TcpListener> for OwnedSocket {
    /// 接管一个 [`TcpListener`](crate::net::TcpListener) 的 socket 的所有权。
    #[inline]
    fn from(tcp_listener: crate::net::TcpListener) -> OwnedSocket {
        unsafe { OwnedSocket::from_raw_socket(tcp_listener.into_raw_socket()) }
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl From<OwnedSocket> for crate::net::TcpListener {
    #[inline]
    fn from(owned: OwnedSocket) -> Self {
        unsafe { Self::from_raw_socket(owned.into_raw_socket()) }
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl AsSocket for crate::net::UdpSocket {
    #[inline]
    fn as_socket(&self) -> BorrowedSocket<'_> {
        unsafe { BorrowedSocket::borrow_raw(self.as_raw_socket()) }
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl From<crate::net::UdpSocket> for OwnedSocket {
    /// 接管一个 [`UdpSocket`](crate::net::UdpSocket) 底层 socket 的所有权。
    #[inline]
    fn from(udp_socket: crate::net::UdpSocket) -> OwnedSocket {
        unsafe { OwnedSocket::from_raw_socket(udp_socket.into_raw_socket()) }
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl From<OwnedSocket> for crate::net::UdpSocket {
    #[inline]
    fn from(owned: OwnedSocket) -> Self {
        unsafe { Self::from_raw_socket(owned.into_raw_socket()) }
    }
}
