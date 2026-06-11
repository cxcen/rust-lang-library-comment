//! 针对通用 I/O 基础类型的 SOLID 平台特定扩展
//!
//! 就像裸指针一样，原始 SOLID Sockets 文件描述符指向具有动态生命周期的资源，
//! 如果它们的存活时间超过了所指向的资源，便可能成为悬垂（dangle）描述符；
//! 如果它们由无效值创建，则可能是伪造（forged）的。
//!
//! 本模块提供了三种类型，用于表示具有不同所有权属性的原始文件描述符：raw（裸）、
//! borrowed（借用）和 owned（拥有），它们分别类比于用于表示指针的各类型：
//!
//! | 类型               | 类比于       |
//! | ------------------ | ------------ |
//! | [`RawFd`]          | `*const _`   |
//! | [`BorrowedFd<'a>`] | `&'a _`      |
//! | [`OwnedFd`]        | `Box<_>`     |
//!
//! 与裸指针一样，`RawFd` 值是基础值（primitive values）。在新代码中，对其进行 I/O
//! 操作应被视为不安全（类比于对它们解引用）。Rust 并非一直提供这一指导，因此 Rust
//! 生态系统中的现有代码常常没有将 `RawFd` 的使用标记为 unsafe。一旦 `io_safety`
//! 特性稳定，库将被鼓励进行迁移，方式或是为解引用 `RawFd` 值的 API 添加 `unsafe`，
//! 或是改用 `BorrowedFd` 或 `OwnedFd`。
//!
//! 与引用一样，`BorrowedFd` 值与某个生命周期绑定，以确保它们的存活时间不会超过其所
//! 指向的资源。这些值的使用是安全的。`BorrowedFd` 值可用于为以下系统调用之外的任意
//! 系统调用提供安全访问的 API：
//!
//!  - `close`，因为这会在不结束文件描述符生命周期的情况下结束资源的动态生命周期。
//!
//!  - `dup2`/`dup3` 的第二个参数，因为该参数会被关闭并被赋予一个新资源，这可能会破坏
//!    使用该文件描述符的其他代码所持有的假设。
//!
//! `BorrowedFd` 值可用于为 `dup` 系统调用提供安全访问的 API，因此实现了 `AsFd` 或
//! `From<OwnedFd>` 的类型不应假设它们始终对底层的文件描述（file description）拥有独占访问权。
//!
//! 与 box 一样，`OwnedFd` 值在概念上拥有它们所指向的资源，并在被丢弃（drop）时
//! 释放（关闭）该资源。
//!
//! [`BorrowedFd<'a>`]: crate::os::solid::io::BorrowedFd

#![unstable(feature = "solid_ext", issue = "none")]

use crate::marker::PhantomData;
use crate::mem::ManuallyDrop;
use crate::sys::{AsInner, FromInner, IntoInner};
use crate::{fmt, io, net, sys};

/// 原始文件描述符。
pub type RawFd = i32;

// 以二进制补码表示，它的最大值为 -2。-1 是 `SOLID_NET_INVALID_FD`。
type ValidRawFd = core::num::niche_types::NotAllOnes<RawFd>;

/// 一个借用的 SOLID Sockets 文件描述符。
///
/// 它带有一个生命周期参数，用以将其与拥有该套接字之物的生命周期绑定。
///
/// 它使用 `repr(transparent)`，且具有宿主文件描述符（host file descriptor）的表示形式，
/// 因此可在 FFI 中用于将套接字作为参数传递的场景 —— 此时它不会被捕获或消耗，且其值
/// 永远不会是 `SOLID_NET_INVALID_FD`。
///
/// 此类型的 `.to_owned()` 实现返回的是另一个 `BorrowedFd` 而非 `OwnedFd`。它只是对
/// 原始套接字做一次平凡的拷贝，随后在相同的生命周期下被借用。
#[derive(Copy, Clone)]
#[repr(transparent)]
#[rustc_nonnull_optimization_guaranteed]
pub struct BorrowedFd<'socket> {
    fd: ValidRawFd,
    _phantom: PhantomData<&'socket OwnedFd>,
}

/// 一个拥有所有权的 SOLID Sockets 文件描述符。
///
/// 它会在丢弃（drop）时关闭该文件描述符。
///
/// 它使用 `repr(transparent)`，且具有宿主文件描述符（host file descriptor）的表示形式，
/// 因此可在 FFI 中用于将套接字作为参数传递的场景 —— 此时它不会被捕获或消耗，且其值
/// 永远不会是 `SOLID_NET_INVALID_FD`。
#[repr(transparent)]
#[rustc_nonnull_optimization_guaranteed]
pub struct OwnedFd {
    fd: ValidRawFd,
}

impl BorrowedFd<'_> {
    /// 返回一个持有给定原始文件描述符的 `BorrowedFd`。
    ///
    /// # 安全性(Safety）
    ///
    /// 在返回的 `BorrowedFd` 存续期间，`fd` 所指向的资源必须保持打开状态，且其值
    /// 不能是 `SOLID_NET_INVALID_FD`。
    #[inline]
    #[track_caller]
    pub const unsafe fn borrow_raw(fd: RawFd) -> Self {
        Self { fd: ValidRawFd::new(fd).expect("fd != -1"), _phantom: PhantomData }
    }
}

impl OwnedFd {
    /// 创建一个新的 `OwnedFd` 实例，它与现有的 `OwnedFd` 实例共享同一个底层文件描述（file description）。
    pub fn try_clone(&self) -> io::Result<Self> {
        self.as_fd().try_clone_to_owned()
    }
}

impl BorrowedFd<'_> {
    /// 创建一个新的 `OwnedFd` 实例，它与现有的 `BorrowedFd` 实例共享同一个底层文件描述（file description）。
    pub fn try_clone_to_owned(&self) -> io::Result<OwnedFd> {
        let fd = sys::net::cvt(unsafe { crate::sys::abi::sockets::dup(self.as_raw_fd()) })?;
        Ok(unsafe { OwnedFd::from_raw_fd(fd) })
    }
}

impl AsRawFd for BorrowedFd<'_> {
    #[inline]
    fn as_raw_fd(&self) -> RawFd {
        self.fd.as_inner()
    }
}

impl AsRawFd for OwnedFd {
    #[inline]
    fn as_raw_fd(&self) -> RawFd {
        self.fd.as_inner()
    }
}

impl IntoRawFd for OwnedFd {
    #[inline]
    fn into_raw_fd(self) -> RawFd {
        ManuallyDrop::new(self).fd.as_inner()
    }
}

impl FromRawFd for OwnedFd {
    /// 从给定的原始文件描述符构造一个新的 `Self` 实例。
    ///
    /// # 安全性(Safety）
    ///
    /// `fd` 所指向的资源必须处于打开状态且适合取得其所有权。该资源除 `close` 之外
    /// 不得需要任何其他清理操作。
    #[inline]
    #[track_caller]
    unsafe fn from_raw_fd(fd: RawFd) -> Self {
        Self { fd: ValidRawFd::new(fd).expect("fd != -1") }
    }
}

impl Drop for OwnedFd {
    #[inline]
    fn drop(&mut self) {
        unsafe { crate::sys::abi::sockets::close(self.fd.as_inner()) };
    }
}

impl fmt::Debug for BorrowedFd<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BorrowedFd").field("fd", &self.fd).finish()
    }
}

impl fmt::Debug for OwnedFd {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OwnedFd").field("fd", &self.fd).finish()
    }
}

macro_rules! impl_is_terminal {
    ($($t:ty),*$(,)?) => {$(
        #[unstable(feature = "sealed", issue = "none")]
        impl crate::sealed::Sealed for $t {}

        #[stable(feature = "is_terminal", since = "1.70.0")]
        impl io::IsTerminal for $t {
            #[inline]
            fn is_terminal(&self) -> bool {
                crate::sys::io::is_terminal(self)
            }
        }
    )*}
}

impl_is_terminal!(BorrowedFd<'_>, OwnedFd);

/// 用于从底层对象中借用 SOLID Sockets 文件描述符的 trait。
pub trait AsFd {
    /// 借用该文件描述符。
    fn as_fd(&self) -> BorrowedFd<'_>;
}

impl<T: AsFd> AsFd for &T {
    #[inline]
    fn as_fd(&self) -> BorrowedFd<'_> {
        T::as_fd(self)
    }
}

impl<T: AsFd> AsFd for &mut T {
    #[inline]
    fn as_fd(&self) -> BorrowedFd<'_> {
        T::as_fd(self)
    }
}

impl AsFd for BorrowedFd<'_> {
    #[inline]
    fn as_fd(&self) -> BorrowedFd<'_> {
        *self
    }
}

impl AsFd for OwnedFd {
    #[inline]
    fn as_fd(&self) -> BorrowedFd<'_> {
        // 安全性：`OwnedFd` 与 `BorrowedFd` 具有相同的有效性
        // 不变式，并且该 `BorrowedFd` 受 `&self` 的生命周期约束。
        unsafe { BorrowedFd::borrow_raw(self.as_raw_fd()) }
    }
}

macro_rules! impl_owned_fd_traits {
    ($($t:ident)*) => {$(
        impl AsFd for net::$t {
            #[inline]
            fn as_fd(&self) -> BorrowedFd<'_> {
                self.as_inner().socket().as_fd()
            }
        }

        impl From<net::$t> for OwnedFd {
            #[inline]
            fn from(socket: net::$t) -> OwnedFd {
                socket.into_inner().into_socket().into_inner()
            }
        }

        impl From<OwnedFd> for net::$t {
            #[inline]
            fn from(owned_fd: OwnedFd) -> Self {
                Self::from_inner(FromInner::from_inner(FromInner::from_inner(owned_fd)))
            }
        }
    )*};
}
impl_owned_fd_traits! { TcpStream TcpListener UdpSocket }

/// 此 impl 允许在 Arc 上实现需要 `AsFd` 的 trait。
/// ```
/// # #[cfg(target_os = "solid_asp3")] mod group_cfg {
/// # use std::os::solid::io::AsFd;
/// use std::net::UdpSocket;
/// use std::sync::Arc;
///
/// trait MyTrait: AsFd {}
/// impl MyTrait for Arc<UdpSocket> {}
/// impl MyTrait for Box<UdpSocket> {}
/// # }
/// ```
impl<T: AsFd> AsFd for crate::sync::Arc<T> {
    #[inline]
    fn as_fd(&self) -> BorrowedFd<'_> {
        (**self).as_fd()
    }
}

impl<T: AsFd> AsFd for crate::rc::Rc<T> {
    #[inline]
    fn as_fd(&self) -> BorrowedFd<'_> {
        (**self).as_fd()
    }
}

impl<T: AsFd> AsFd for Box<T> {
    #[inline]
    fn as_fd(&self) -> BorrowedFd<'_> {
        (**self).as_fd()
    }
}

/// 用于从底层对象中提取原始 SOLID Sockets 文件描述符的 trait。
pub trait AsRawFd {
    /// 提取原始文件描述符。
    ///
    /// 本方法**不会**将原始文件描述符的所有权转交给调用方。仅在原始对象尚未被销毁期间，
    /// 该描述符才保证有效。
    fn as_raw_fd(&self) -> RawFd;
}

/// 用于表达从原始文件描述符构造对象之能力的 trait。
pub trait FromRawFd {
    /// 从给定的原始文件描述符构造一个新的 `Self` 实例。
    ///
    /// 本函数通常用于**取得（consume）**所指定文件描述符的所有权。以这种方式使用时，
    /// 返回的对象将负责在其离开作用域时关闭该描述符。
    ///
    /// 然而，取得所有权并非严格必需。对于严格取得所有权的 API，请使用
    /// [`From<OwnedFd>::from`] 实现。
    ///
    /// # 安全性(Safety）
    ///
    /// 传入的 `fd` 必须是一个[拥有所有权的文件描述符][io-safety]；
    /// 特别地，它必须处于打开状态。
    ///
    /// [io-safety]: io#io-safety
    unsafe fn from_raw_fd(fd: RawFd) -> Self;
}

/// 用于表达消耗一个对象并取得其原始文件描述符所有权之能力的 trait。
pub trait IntoRawFd {
    /// 消耗此对象，返回底层的原始文件描述符。
    ///
    /// 本函数将底层文件描述符的所有权**转移（transfer）**给调用方。此后调用方是该文件
    /// 描述符的唯一所有者，必须在不再需要时关闭该描述符。
    #[must_use = "losing the raw file descriptor may leak resources"]
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

macro_rules! impl_as_raw_fd {
    ($($t:ident)*) => {$(
        #[stable(feature = "rust1", since = "1.0.0")]
        impl AsRawFd for net::$t {
            #[inline]
            fn as_raw_fd(&self) -> RawFd {
                self.as_inner().socket().as_raw_fd()
            }
        }
    )*};
}
impl_as_raw_fd! { TcpStream TcpListener UdpSocket }

macro_rules! impl_from_raw_fd {
    ($($t:ident)*) => {$(
        #[stable(feature = "from_raw_os", since = "1.1.0")]
        impl FromRawFd for net::$t {
            #[inline]
            unsafe fn from_raw_fd(fd: RawFd) -> net::$t {
                let socket = unsafe { sys::net::Socket::from_raw_fd(fd) };
                net::$t::from_inner(sys::net::$t::from_inner(socket))
            }
        }
    )*};
}
impl_from_raw_fd! { TcpStream TcpListener UdpSocket }

macro_rules! impl_into_raw_fd {
    ($($t:ident)*) => {$(
        #[stable(feature = "into_raw_os", since = "1.4.0")]
        impl IntoRawFd for net::$t {
            #[inline]
            fn into_raw_fd(self) -> RawFd {
                self.into_inner().into_socket().into_raw_fd()
            }
        }
    )*};
}
impl_into_raw_fd! { TcpStream TcpListener UdpSocket }
