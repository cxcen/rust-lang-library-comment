//! 拥有所有权的、以及借用的类 Unix 文件描述符。

#![stable(feature = "io_safety", since = "1.63.0")]
#![deny(unsafe_op_in_unsafe_fn)]

#[cfg(target_os = "motor")]
use moto_rt::libc;

use super::raw::{AsRawFd, FromRawFd, IntoRawFd, RawFd};
#[cfg(not(target_os = "trusty"))]
use crate::fs;
use crate::marker::PhantomData;
use crate::mem::ManuallyDrop;
#[cfg(not(any(
    target_arch = "wasm32",
    target_env = "sgx",
    target_os = "hermit",
    target_os = "trusty",
    target_os = "motor"
)))]
use crate::sys::cvt;
#[cfg(not(target_os = "trusty"))]
use crate::sys::{AsInner, FromInner, IntoInner};
use crate::{fmt, io};

type ValidRawFd = core::num::niche_types::NotAllOnes<RawFd>;

/// 一个借用的文件描述符。
///
/// 它带有一个生命周期参数，用来把自身与某个拥有该文件描述符的对象的生命周期绑定在一起。
/// 在该生命周期持续期间，保证不会有任何人关闭这个文件描述符。
///
/// 它使用 `repr(transparent)` 且具有与宿主文件描述符相同的表示形式，因此可以在 FFI 中
/// 用于以下场景：文件描述符作为实参传入、不会被捕获或消耗、且其值永远不为 `-1`。
///
/// 该类型没有 [`ToOwned`][crate::borrow::ToOwned] 实现。对该类型的变量调用 `.to_owned()`
/// 会改为在 `&BorrowedFd` 上调用，并像 `ToOwned` 对所有实现了 `Clone` 的类型所做的那样
/// 使用 `Clone::clone()`。其结果将是一个在相同生命周期下借用的描述符。
///
/// 若要获得 [`OwnedFd`]，可改用 [`BorrowedFd::try_clone_to_owned`]，但并非所有平台都支持该方法。
#[derive(Copy, Clone)]
#[repr(transparent)]
#[rustc_nonnull_optimization_guaranteed]
#[stable(feature = "io_safety", since = "1.63.0")]
pub struct BorrowedFd<'fd> {
    fd: ValidRawFd,
    _phantom: PhantomData<&'fd OwnedFd>,
}

/// 一个拥有所有权的文件描述符。
///
/// 它会在 drop 时关闭该文件描述符。保证不会有其他任何人关闭这个文件描述符。
///
/// 它使用 `repr(transparent)` 且具有与宿主文件描述符相同的表示形式，因此可以在 FFI 中
/// 用于以下场景：文件描述符作为被消耗的实参传入、或作为拥有所有权的值返回，且其值永远不为 `-1`。
///
/// 你可以使用 [`AsFd::as_fd`] 来获得一个 [`BorrowedFd`]。
#[repr(transparent)]
#[rustc_nonnull_optimization_guaranteed]
#[stable(feature = "io_safety", since = "1.63.0")]
pub struct OwnedFd {
    fd: ValidRawFd,
}

impl BorrowedFd<'_> {
    /// 返回一个持有给定裸文件描述符的 `BorrowedFd`。
    ///
    /// # Safety
    ///
    /// `fd` 所指向的资源必须在返回的 `BorrowedFd` 的整个存续期间保持打开状态。
    ///
    /// # Panics
    ///
    /// 如果该裸文件描述符的值为 `-1`，则会 panic。
    #[inline]
    #[track_caller]
    #[rustc_const_stable(feature = "io_safety", since = "1.63.0")]
    #[stable(feature = "io_safety", since = "1.63.0")]
    pub const unsafe fn borrow_raw(fd: RawFd) -> Self {
        Self { fd: ValidRawFd::new(fd).expect("fd != -1"), _phantom: PhantomData }
    }
}

impl OwnedFd {
    /// 创建一个新的 `OwnedFd` 实例，它与现有的 `OwnedFd` 实例共享同一个底层的
    /// 打开文件描述（file description）。
    #[stable(feature = "io_safety", since = "1.63.0")]
    pub fn try_clone(&self) -> io::Result<Self> {
        self.as_fd().try_clone_to_owned()
    }
}

impl BorrowedFd<'_> {
    /// 创建一个新的 `OwnedFd` 实例，它与现有的 `BorrowedFd` 实例共享同一个底层的
    /// 打开文件描述（file description）。
    #[cfg(not(any(
        target_arch = "wasm32",
        target_os = "hermit",
        target_os = "trusty",
        target_os = "motor"
    )))]
    #[stable(feature = "io_safety", since = "1.63.0")]
    pub fn try_clone_to_owned(&self) -> io::Result<OwnedFd> {
        // 我们希望原子地复制此文件描述符并设置 CLOEXEC 标志，目前这通过
        // F_DUPFD_CLOEXEC 完成。这是一个 POSIX 标志，于 2.6.24 版本加入 Linux。
        #[cfg(not(any(target_os = "espidf", target_os = "vita")))]
        let cmd = libc::F_DUPFD_CLOEXEC;

        // 对于 ESP-IDF，改用 F_DUPFD，因为 CLOEXEC 语义永远不会被支持——
        // 它是一个没有多进程执行能力的裸机框架。虽然 F_DUPFD 目前也尚未支持，
        // 但将来可能会支持（目前它返回 ENOSYS）。
        #[cfg(any(target_os = "espidf", target_os = "vita"))]
        let cmd = libc::F_DUPFD;

        // 避免使用 3 以下的文件描述符，因为它们被 stdio 占用
        let fd = cvt(unsafe { libc::fcntl(self.as_raw_fd(), cmd, 3) })?;
        Ok(unsafe { OwnedFd::from_raw_fd(fd) })
    }

    /// 创建一个新的 `OwnedFd` 实例，它与现有的 `BorrowedFd` 实例共享同一个底层的
    /// 打开文件描述（file description）。
    #[cfg(any(target_arch = "wasm32", target_os = "hermit", target_os = "trusty"))]
    #[stable(feature = "io_safety", since = "1.63.0")]
    pub fn try_clone_to_owned(&self) -> io::Result<OwnedFd> {
        Err(io::Error::UNSUPPORTED_PLATFORM)
    }

    /// 创建一个新的 `OwnedFd` 实例，它与现有的 `BorrowedFd` 实例共享同一个底层的
    /// 打开文件描述（file description）。
    #[cfg(target_os = "motor")]
    #[stable(feature = "io_safety", since = "1.63.0")]
    pub fn try_clone_to_owned(&self) -> io::Result<OwnedFd> {
        let fd = moto_rt::fs::duplicate(self.as_raw_fd()).map_err(crate::sys::map_motor_error)?;
        Ok(unsafe { OwnedFd::from_raw_fd(fd) })
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl AsRawFd for BorrowedFd<'_> {
    #[inline]
    fn as_raw_fd(&self) -> RawFd {
        self.fd.as_inner()
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl AsRawFd for OwnedFd {
    #[inline]
    fn as_raw_fd(&self) -> RawFd {
        self.fd.as_inner()
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl IntoRawFd for OwnedFd {
    #[inline]
    fn into_raw_fd(self) -> RawFd {
        ManuallyDrop::new(self).fd.as_inner()
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl FromRawFd for OwnedFd {
    /// 从给定的裸文件描述符构造一个新的 `Self` 实例。
    ///
    /// # Safety
    ///
    /// `fd` 所指向的资源必须处于打开状态，且适合被假定[拥有所有权][io-safety]。
    /// 该资源除 `close` 之外不得要求任何其他清理操作。
    ///
    /// [io-safety]: io#io-safety
    ///
    /// # Panics
    ///
    /// 如果该裸文件描述符的值为 `-1`，则会 panic。
    #[inline]
    #[track_caller]
    unsafe fn from_raw_fd(fd: RawFd) -> Self {
        Self { fd: ValidRawFd::new(fd).expect("fd != -1") }
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl Drop for OwnedFd {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            // 注意，关闭文件描述符时错误会被忽略。根据 POSIX 2024，我们可以、
            // 而且确实应当在 `EINTR` 时重试 `close`
            // (https://pubs.opengroup.org/onlinepubs/9799919799.2024edition/functions/close.html)，
            // 但目前尚不清楚广泛使用的实现对这一要求的遵循程度如何，因为旧版 POSIX
            // 把 `EINTR` 之后 FD 的状态留作未指定。忽略错误是“可以接受的”，因为某些主要的
            // Unix 系统（特别是 Linux）确实保证总是关闭该 FD，即便 `close()` 被中断；
            // 而且这种情形本身就很罕见。如果我们在一个不符合 POSIX 的实现上重试，后果
            // 可能非常糟糕，因为我们可能会关闭错误的 FD。这里有一个指向 POSIX 工作组那场
            // 最终促成最新 POSIX 措辞的精彩讨论的有用链接：http://austingroupbugs.net/view.php?id=529
            #[cfg(not(target_os = "hermit"))]
            {
                #[cfg(unix)]
                crate::sys::fs::debug_assert_fd_is_open(self.fd.as_inner());

                let _ = libc::close(self.fd.as_inner());
            }
            #[cfg(target_os = "hermit")]
            let _ = hermit_abi::close(self.fd.as_inner());
        }
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl fmt::Debug for BorrowedFd<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BorrowedFd").field("fd", &self.fd).finish()
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
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

/// 用于从底层对象借用文件描述符的 trait。
///
/// 该 trait 仅在 unix 平台上可用，且必须导入后才能调用其方法。Windows 平台有对应的
/// `AsHandle` 与 `AsSocket` 系列 trait。
#[stable(feature = "io_safety", since = "1.63.0")]
pub trait AsFd {
    /// 借用该文件描述符。
    ///
    /// # 示例
    ///
    /// ```rust,no_run
    /// use std::fs::File;
    /// # use std::io;
    /// # #[cfg(any(unix, target_os = "wasi"))]
    /// # use std::os::fd::{AsFd, BorrowedFd};
    ///
    /// let mut f = File::open("foo.txt")?;
    /// # #[cfg(any(unix, target_os = "wasi"))]
    /// let borrowed_fd: BorrowedFd<'_> = f.as_fd();
    /// # Ok::<(), io::Error>(())
    /// ```
    #[stable(feature = "io_safety", since = "1.63.0")]
    fn as_fd(&self) -> BorrowedFd<'_>;
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl<T: AsFd + ?Sized> AsFd for &T {
    #[inline]
    fn as_fd(&self) -> BorrowedFd<'_> {
        T::as_fd(self)
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl<T: AsFd + ?Sized> AsFd for &mut T {
    #[inline]
    fn as_fd(&self) -> BorrowedFd<'_> {
        T::as_fd(self)
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl AsFd for BorrowedFd<'_> {
    #[inline]
    fn as_fd(&self) -> BorrowedFd<'_> {
        *self
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl AsFd for OwnedFd {
    #[inline]
    fn as_fd(&self) -> BorrowedFd<'_> {
        // Safety: `OwnedFd` 与 `BorrowedFd` 具有相同的有效性
        // 不变量，且该 `BorrowedFd` 受 `&self` 的生命周期约束。
        unsafe { BorrowedFd::borrow_raw(self.as_raw_fd()) }
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
#[cfg(not(target_os = "trusty"))]
impl AsFd for fs::File {
    #[inline]
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.as_inner().as_fd()
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
#[cfg(not(target_os = "trusty"))]
impl From<fs::File> for OwnedFd {
    /// 取得一个 [`File`](fs::File) 底层文件描述符的所有权。
    #[inline]
    fn from(file: fs::File) -> OwnedFd {
        file.into_inner().into_inner().into_inner()
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
#[cfg(not(target_os = "trusty"))]
impl From<OwnedFd> for fs::File {
    /// 返回一个取得给定文件描述符所有权的 [`File`](fs::File)。
    #[inline]
    fn from(owned_fd: OwnedFd) -> Self {
        Self::from_inner(FromInner::from_inner(FromInner::from_inner(owned_fd)))
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
#[cfg(not(target_os = "trusty"))]
impl AsFd for crate::net::TcpStream {
    #[inline]
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.as_inner().socket().as_fd()
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
#[cfg(not(target_os = "trusty"))]
impl From<crate::net::TcpStream> for OwnedFd {
    /// 取得一个 [`TcpStream`](crate::net::TcpStream) 的套接字文件描述符的所有权。
    #[inline]
    fn from(tcp_stream: crate::net::TcpStream) -> OwnedFd {
        tcp_stream.into_inner().into_socket().into_inner().into_inner().into()
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
#[cfg(not(target_os = "trusty"))]
impl From<OwnedFd> for crate::net::TcpStream {
    #[inline]
    fn from(owned_fd: OwnedFd) -> Self {
        Self::from_inner(FromInner::from_inner(FromInner::from_inner(FromInner::from_inner(
            owned_fd,
        ))))
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
#[cfg(not(target_os = "trusty"))]
impl AsFd for crate::net::TcpListener {
    #[inline]
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.as_inner().socket().as_fd()
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
#[cfg(not(target_os = "trusty"))]
impl From<crate::net::TcpListener> for OwnedFd {
    /// 取得一个 [`TcpListener`](crate::net::TcpListener) 的套接字文件描述符的所有权。
    #[inline]
    fn from(tcp_listener: crate::net::TcpListener) -> OwnedFd {
        tcp_listener.into_inner().into_socket().into_inner().into_inner().into()
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
#[cfg(not(target_os = "trusty"))]
impl From<OwnedFd> for crate::net::TcpListener {
    #[inline]
    fn from(owned_fd: OwnedFd) -> Self {
        Self::from_inner(FromInner::from_inner(FromInner::from_inner(FromInner::from_inner(
            owned_fd,
        ))))
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
#[cfg(not(target_os = "trusty"))]
impl AsFd for crate::net::UdpSocket {
    #[inline]
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.as_inner().socket().as_fd()
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
#[cfg(not(target_os = "trusty"))]
impl From<crate::net::UdpSocket> for OwnedFd {
    /// 取得一个 [`UdpSocket`](crate::net::UdpSocket) 的文件描述符的所有权。
    #[inline]
    fn from(udp_socket: crate::net::UdpSocket) -> OwnedFd {
        udp_socket.into_inner().into_socket().into_inner().into_inner().into()
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
#[cfg(not(target_os = "trusty"))]
impl From<OwnedFd> for crate::net::UdpSocket {
    #[inline]
    fn from(owned_fd: OwnedFd) -> Self {
        Self::from_inner(FromInner::from_inner(FromInner::from_inner(FromInner::from_inner(
            owned_fd,
        ))))
    }
}

#[stable(feature = "asfd_ptrs", since = "1.64.0")]
/// 此 impl 使得可以在 Arc 上实现那些要求 `AsFd` 的 trait。
/// ```
/// # #[cfg(any(unix, target_os = "wasi"))] mod group_cfg {
/// # #[cfg(target_os = "wasi")]
/// # use std::os::wasi::io::AsFd;
/// # #[cfg(unix)]
/// # use std::os::unix::io::AsFd;
/// use std::net::UdpSocket;
/// use std::sync::Arc;
///
/// trait MyTrait: AsFd {}
/// impl MyTrait for Arc<UdpSocket> {}
/// impl MyTrait for Box<UdpSocket> {}
/// # }
/// ```
impl<T: AsFd + ?Sized> AsFd for crate::sync::Arc<T> {
    #[inline]
    fn as_fd(&self) -> BorrowedFd<'_> {
        (**self).as_fd()
    }
}

#[stable(feature = "asfd_rc", since = "1.69.0")]
impl<T: AsFd + ?Sized> AsFd for crate::rc::Rc<T> {
    #[inline]
    fn as_fd(&self) -> BorrowedFd<'_> {
        (**self).as_fd()
    }
}

#[unstable(feature = "unique_rc_arc", issue = "112566")]
impl<T: AsFd + ?Sized> AsFd for crate::rc::UniqueRc<T> {
    #[inline]
    fn as_fd(&self) -> BorrowedFd<'_> {
        (**self).as_fd()
    }
}

#[stable(feature = "asfd_ptrs", since = "1.64.0")]
impl<T: AsFd + ?Sized> AsFd for Box<T> {
    #[inline]
    fn as_fd(&self) -> BorrowedFd<'_> {
        (**self).as_fd()
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl AsFd for io::Stdin {
    #[inline]
    fn as_fd(&self) -> BorrowedFd<'_> {
        unsafe { BorrowedFd::borrow_raw(0) }
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl<'a> AsFd for io::StdinLock<'a> {
    #[inline]
    fn as_fd(&self) -> BorrowedFd<'_> {
        // SAFETY: 用户代码不应在标准库底下把 stdin 关闭
        unsafe { BorrowedFd::borrow_raw(0) }
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl AsFd for io::Stdout {
    #[inline]
    fn as_fd(&self) -> BorrowedFd<'_> {
        unsafe { BorrowedFd::borrow_raw(1) }
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl<'a> AsFd for io::StdoutLock<'a> {
    #[inline]
    fn as_fd(&self) -> BorrowedFd<'_> {
        // SAFETY: 用户代码不应在标准库底下把 stdout 关闭
        unsafe { BorrowedFd::borrow_raw(1) }
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl AsFd for io::Stderr {
    #[inline]
    fn as_fd(&self) -> BorrowedFd<'_> {
        unsafe { BorrowedFd::borrow_raw(2) }
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl<'a> AsFd for io::StderrLock<'a> {
    #[inline]
    fn as_fd(&self) -> BorrowedFd<'_> {
        // SAFETY: 用户代码不应在标准库底下把 stderr 关闭
        unsafe { BorrowedFd::borrow_raw(2) }
    }
}

#[stable(feature = "anonymous_pipe", since = "1.87.0")]
#[cfg(not(target_os = "trusty"))]
impl AsFd for io::PipeReader {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0.as_fd()
    }
}

#[stable(feature = "anonymous_pipe", since = "1.87.0")]
#[cfg(not(target_os = "trusty"))]
impl From<io::PipeReader> for OwnedFd {
    fn from(pipe: io::PipeReader) -> Self {
        pipe.0.into_inner()
    }
}

#[stable(feature = "anonymous_pipe", since = "1.87.0")]
#[cfg(not(target_os = "trusty"))]
impl AsFd for io::PipeWriter {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0.as_fd()
    }
}

#[stable(feature = "anonymous_pipe", since = "1.87.0")]
#[cfg(not(target_os = "trusty"))]
impl From<io::PipeWriter> for OwnedFd {
    fn from(pipe: io::PipeWriter) -> Self {
        pipe.0.into_inner()
    }
}

#[stable(feature = "anonymous_pipe", since = "1.87.0")]
#[cfg(not(target_os = "trusty"))]
impl From<OwnedFd> for io::PipeReader {
    fn from(owned_fd: OwnedFd) -> Self {
        Self(FromInner::from_inner(owned_fd))
    }
}

#[stable(feature = "anonymous_pipe", since = "1.87.0")]
#[cfg(not(target_os = "trusty"))]
impl From<OwnedFd> for io::PipeWriter {
    fn from(owned_fd: OwnedFd) -> Self {
        Self(FromInner::from_inner(owned_fd))
    }
}
