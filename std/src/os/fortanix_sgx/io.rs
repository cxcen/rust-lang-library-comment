//! 针对通用 I/O 基础类型的 SGX 平台特定扩展
//!
//! SGX 文件描述符的行为与 Unix 文件描述符不同。更多细节请参阅 [`TryIntoRawFd`] 的说明。
#![unstable(feature = "sgx_platform", issue = "56975")]

use crate::net;
pub use crate::sys::abi::usercalls::raw::Fd as RawFd;
use crate::sys::{self, AsInner, FromInner, IntoInner, TryIntoInner};

/// 用于从底层对象中提取原始 SGX 文件描述符的 trait。
#[unstable(feature = "sgx_platform", issue = "56975")]
pub trait AsRawFd {
    /// 提取原始文件描述符。
    ///
    /// 本方法**不会**将原始文件描述符的所有权转交给调用方。仅在原始对象尚未被销毁期间，
    /// 该描述符才保证有效。
    #[unstable(feature = "sgx_platform", issue = "56975")]
    fn as_raw_fd(&self) -> RawFd;
}

/// 用于表达从原始文件描述符构造对象之能力的 trait。
#[unstable(feature = "sgx_platform", issue = "56975")]
pub trait FromRawFd {
    /// 一个关联类型，包含 `Self` 的相关元数据。
    type Metadata: Default;

    /// 从给定的原始文件描述符和元数据构造一个新的 `Self` 实例。
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
    // FIXME: 关于 `metadata` 应当补充一些说明。
    ///
    /// [io-safety]: io#io-safety
    #[unstable(feature = "sgx_platform", issue = "56975")]
    unsafe fn from_raw_fd(fd: RawFd, metadata: Self::Metadata) -> Self;
}

/// 用于表达消耗一个对象并取得其原始文件描述符所有权之能力的 trait。
#[unstable(feature = "sgx_platform", issue = "56975")]
pub trait TryIntoRawFd: Sized {
    /// 消耗此对象，在此对象未被克隆的情况下返回底层的原始文件描述符。
    ///
    /// 本函数将底层文件描述符的所有权**转移（transfer）**给调用方。此后调用方是该文件
    /// 描述符的唯一所有者，必须在不再需要时关闭该描述符。
    ///
    /// 与其他平台不同，在 SGX 上，文件描述符在一个对象的所有克隆之间共享。为避免竞态条件，
    /// 本函数仅在对最后一个克隆调用时才会返回 `Ok`。
    #[unstable(feature = "sgx_platform", issue = "56975")]
    fn try_into_raw_fd(self) -> Result<RawFd, Self>;
}

impl AsRawFd for net::TcpStream {
    #[inline]
    fn as_raw_fd(&self) -> RawFd {
        *self.as_inner().as_inner().as_inner().as_inner()
    }
}

impl AsRawFd for net::TcpListener {
    #[inline]
    fn as_raw_fd(&self) -> RawFd {
        *self.as_inner().as_inner().as_inner().as_inner()
    }
}

/// `TcpStream` 的元数据。
#[derive(Debug, Clone, Default)]
#[unstable(feature = "sgx_platform", issue = "56975")]
pub struct TcpStreamMetadata {
    /// TCP 流的本地地址
    pub local_addr: Option<String>,
    /// TCP 流的对端地址
    pub peer_addr: Option<String>,
}

impl FromRawFd for net::TcpStream {
    type Metadata = TcpStreamMetadata;

    #[inline]
    unsafe fn from_raw_fd(fd: RawFd, metadata: Self::Metadata) -> net::TcpStream {
        let fd = sys::fd::FileDesc::from_inner(fd);
        let socket = sys::net::Socket::from_inner((fd, metadata.local_addr));
        net::TcpStream::from_inner(sys::net::TcpStream::from_inner((socket, metadata.peer_addr)))
    }
}

/// `TcpListener` 的元数据。
#[derive(Debug, Clone, Default)]
#[unstable(feature = "sgx_platform", issue = "56975")]
pub struct TcpListenerMetadata {
    /// TCP 监听器的本地地址
    pub local_addr: Option<String>,
}

impl FromRawFd for net::TcpListener {
    type Metadata = TcpListenerMetadata;

    #[inline]
    unsafe fn from_raw_fd(fd: RawFd, metadata: Self::Metadata) -> net::TcpListener {
        let fd = sys::fd::FileDesc::from_inner(fd);
        let socket = sys::net::Socket::from_inner((fd, metadata.local_addr));
        net::TcpListener::from_inner(sys::net::TcpListener::from_inner(socket))
    }
}

impl TryIntoRawFd for net::TcpStream {
    #[inline]
    fn try_into_raw_fd(self) -> Result<RawFd, Self> {
        let (socket, peer_addr) = self.into_inner().into_inner();
        match socket.try_into_inner() {
            Ok(fd) => Ok(fd.into_inner()),
            Err(socket) => {
                let sys = sys::net::TcpStream::from_inner((socket, peer_addr));
                Err(net::TcpStream::from_inner(sys))
            }
        }
    }
}

impl TryIntoRawFd for net::TcpListener {
    #[inline]
    fn try_into_raw_fd(self) -> Result<RawFd, Self> {
        match self.into_inner().into_inner().try_into_inner() {
            Ok(fd) => Ok(fd.into_inner()),
            Err(socket) => {
                let sys = sys::net::TcpListener::from_inner(socket);
                Err(net::TcpListener::from_inner(sys))
            }
        }
    }
}
