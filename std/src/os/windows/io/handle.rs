//! 拥有式（owned）与借用式（borrowed）的操作系统 handle。

#![stable(feature = "io_safety", since = "1.63.0")]

use super::raw::{AsRawHandle, FromRawHandle, IntoRawHandle, RawHandle};
use crate::marker::PhantomData;
use crate::mem::ManuallyDrop;
use crate::sys::{AsInner, FromInner, IntoInner, cvt};
use crate::{fmt, fs, io, ptr, sys};

/// 一个借用式的 handle。
///
/// 它带有一个生命周期参数，用以将自身绑定到拥有该 handle 的某个对象的生命周期上。
///
/// 它采用 `repr(transparent)`，与宿主机 handle 具有相同的表示，因此可以在 FFI 中用于
/// 那些以参数形式传入 handle、且 handle 不会被捕获或消耗的场合。
///
/// 注意它 *可能* 取值为 `-1`：在 `BorrowedHandle` 中该值始终代表一个有效的 handle 值，
/// 例如 [当前进程 handle]，而 *不是* `INVALID_HANDLE_VALUE`，尽管二者数值相同。
/// 完整来龙去脉见 [here]。
///
/// 此外它 *可能* 取值为 `NULL`（0），这会在控制台从进程脱离、或使用了
/// `windows_subsystem` 时发生。
///
/// 本类型的 `.to_owned()` 实现返回的是另一个 `BorrowedHandle` 而不是 `OwnedHandle`。
/// 它只是对裸 handle 做一次平凡的拷贝，随后在同一个生命周期下被借用。
///
/// [here]: https://devblogs.microsoft.com/oldnewthing/20040302-00/?p=40443
/// [当前进程 handle]: https://docs.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-getcurrentprocess#remarks
#[derive(Copy, Clone)]
#[repr(transparent)]
#[stable(feature = "io_safety", since = "1.63.0")]
pub struct BorrowedHandle<'handle> {
    handle: RawHandle,
    _phantom: PhantomData<&'handle OwnedHandle>,
}

/// 一个拥有式的 handle。
///
/// 它会在 drop 时关闭该 handle。
///
/// 注意它 *可能* 取值为 `-1`：在 `OwnedHandle` 中该值始终代表一个有效的 handle 值，
/// 例如 [当前进程 handle]，而 *不是* `INVALID_HANDLE_VALUE`，尽管二者数值相同。
/// 完整来龙去脉见 [here]。
///
/// 此外它 *可能* 取值为 `NULL`（0），这会在控制台从进程脱离、或使用了
/// `windows_subsystem` 时发生。
///
/// `OwnedHandle` 在 drop 时使用 [`CloseHandle`] 关闭其 handle。正因如此，它不得用于
/// 指向已打开注册表键的 handle——后者需要改用 [`RegCloseKey`] 来关闭。
///
/// [`CloseHandle`]: https://docs.microsoft.com/en-us/windows/win32/api/handleapi/nf-handleapi-closehandle
/// [`RegCloseKey`]: https://docs.microsoft.com/en-us/windows/win32/api/winreg/nf-winreg-regclosekey
///
/// [here]: https://devblogs.microsoft.com/oldnewthing/20040302-00/?p=40443
/// [当前进程 handle]: https://docs.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-getcurrentprocess#remarks
#[repr(transparent)]
#[stable(feature = "io_safety", since = "1.63.0")]
pub struct OwnedHandle {
    handle: RawHandle,
}

/// 用于返回值或输出参数中 handle 的 FFI 类型，适用于以 `NULL` 作为哨兵值来表示错误的场景，
/// 例如 `CreateThread` 的返回值。它采用 `repr(transparent)`，与宿主机 handle 具有相同的
/// 表示，因此可以用于这类 FFI 声明。
///
/// 对一个 `HandleOrNull` 唯一有用的操作，就是通过它的 [`TryFrom`] 实现把它转换成
/// `OwnedHandle`；该转换会负责完成对 `NULL` 的检查。这确保了这类 FFI 调用在检查
/// `NULL` 之前无法直接开始使用该 handle。
///
/// 本类型可持有 [`OwnedHandle`] 所能持有的任何 handle 值。与 `OwnedHandle` 一样，
/// 当它持有 `-1` 时，该值被解释为一个有效的 handle 值，例如 [当前进程 handle]，
/// 而不是 `INVALID_HANDLE_VALUE`。
///
/// 如果它持有的是非 null 的 handle，则会在 drop 时关闭该 handle。
///
/// [当前进程 handle]: https://docs.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-getcurrentprocess#remarks
#[repr(transparent)]
#[stable(feature = "io_safety", since = "1.63.0")]
#[derive(Debug)]
pub struct HandleOrNull(RawHandle);

/// 用于返回值或输出参数中 handle 的 FFI 类型，适用于以 `INVALID_HANDLE_VALUE` 作为哨兵值
/// 来表示错误的场景，例如 `CreateFileW` 的返回值。它采用 `repr(transparent)`，与宿主机
/// handle 具有相同的表示，因此可以用于这类 FFI 声明。
///
/// 对一个 `HandleOrInvalid` 唯一有用的操作，就是通过它的 [`TryFrom`] 实现把它转换成
/// `OwnedHandle`；该转换会负责完成对 `INVALID_HANDLE_VALUE` 的检查。这确保了这类 FFI
/// 调用在检查 `INVALID_HANDLE_VALUE` 之前无法直接开始使用该 handle。
///
/// 本类型可持有 [`OwnedHandle`] 所能持有的任何 handle 值，唯一的区别是：当它持有 `-1` 时，
/// 该值被解释为 `INVALID_HANDLE_VALUE`。
///
/// 如果它持有的不是 `INVALID_HANDLE_VALUE`，则会在 drop 时关闭该 handle。
#[repr(transparent)]
#[stable(feature = "io_safety", since = "1.63.0")]
#[derive(Debug)]
pub struct HandleOrInvalid(RawHandle);

// Windows 的 [`HANDLE`] 类型可以跨线程边界传递并在线程间共享（尽管它内部包含一个
// `*mut void`，而后者通常并不是 `Send` 或 `Sync`）。
//
// [`HANDLE`]: std::os::windows::raw::HANDLE
#[stable(feature = "io_safety", since = "1.63.0")]
unsafe impl Send for OwnedHandle {}
#[stable(feature = "io_safety", since = "1.63.0")]
unsafe impl Send for HandleOrNull {}
#[stable(feature = "io_safety", since = "1.63.0")]
unsafe impl Send for HandleOrInvalid {}
#[stable(feature = "io_safety", since = "1.63.0")]
unsafe impl Send for BorrowedHandle<'_> {}
#[stable(feature = "io_safety", since = "1.63.0")]
unsafe impl Sync for OwnedHandle {}
#[stable(feature = "io_safety", since = "1.63.0")]
unsafe impl Sync for HandleOrNull {}
#[stable(feature = "io_safety", since = "1.63.0")]
unsafe impl Sync for HandleOrInvalid {}
#[stable(feature = "io_safety", since = "1.63.0")]
unsafe impl Sync for BorrowedHandle<'_> {}

impl BorrowedHandle<'_> {
    /// 返回一个持有给定裸 handle 的 `BorrowedHandle`。
    ///
    /// # 安全性(Safety）
    ///
    /// `handle` 所指向的资源必须是一个有效的已打开 handle，并且在所返回的
    /// `BorrowedHandle` 的整个存续期间必须保持打开状态。
    ///
    /// 注意它 *可能* 取值为 `INVALID_HANDLE_VALUE`（-1），而该值有时是一个有效的
    /// handle 值。完整来龙去脉见 [here]。
    ///
    /// 此外它 *可能* 取值为 `NULL`（0），这会在控制台从进程脱离、或使用了
    /// `windows_subsystem` 时发生。
    ///
    /// [here]: https://devblogs.microsoft.com/oldnewthing/20040302-00/?p=40443
    #[inline]
    #[rustc_const_stable(feature = "io_safety", since = "1.63.0")]
    #[stable(feature = "io_safety", since = "1.63.0")]
    pub const unsafe fn borrow_raw(handle: RawHandle) -> Self {
        Self { handle, _phantom: PhantomData }
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl TryFrom<HandleOrNull> for OwnedHandle {
    type Error = NullHandleError;

    #[inline]
    fn try_from(handle_or_null: HandleOrNull) -> Result<Self, NullHandleError> {
        let handle_or_null = ManuallyDrop::new(handle_or_null);
        if handle_or_null.is_valid() {
            // SAFETY: 该 handle 不是 null。
            Ok(unsafe { OwnedHandle::from_raw_handle(handle_or_null.0) })
        } else {
            Err(NullHandleError(()))
        }
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl Drop for HandleOrNull {
    #[inline]
    fn drop(&mut self) {
        if self.is_valid() {
            unsafe {
                let _ = sys::c::CloseHandle(self.0);
            }
        }
    }
}

impl OwnedHandle {
    /// 创建一个新的 `OwnedHandle` 实例，它与现有的 `OwnedHandle` 实例共享同一个底层对象。
    #[stable(feature = "io_safety", since = "1.63.0")]
    pub fn try_clone(&self) -> io::Result<Self> {
        self.as_handle().try_clone_to_owned()
    }
}

impl BorrowedHandle<'_> {
    /// 创建一个新的 `OwnedHandle` 实例，它与现有的 `BorrowedHandle` 实例共享同一个底层对象。
    #[stable(feature = "io_safety", since = "1.63.0")]
    pub fn try_clone_to_owned(&self) -> io::Result<OwnedHandle> {
        self.duplicate(0, false, sys::c::DUPLICATE_SAME_ACCESS)
    }

    pub(crate) fn duplicate(
        &self,
        access: u32,
        inherit: bool,
        options: u32,
    ) -> io::Result<OwnedHandle> {
        let handle = self.as_raw_handle();

        // `Stdin`、`Stdout` 和 `Stderr` 都可能持有 null handle，例如在控制台已脱离的
        // 进程中。如果我们把 null handle 传给 `DuplicateHandle`，它会失败；但我们可以把
        // null 当作一个不做任何 I/O 的有效 handle 来对待，并允许它被复制。
        if handle.is_null() {
            return unsafe { Ok(OwnedHandle::from_raw_handle(handle)) };
        }

        let mut ret = ptr::null_mut();
        cvt(unsafe {
            let cur_proc = sys::c::GetCurrentProcess();
            sys::c::DuplicateHandle(
                cur_proc,
                handle,
                cur_proc,
                &mut ret,
                access,
                inherit as sys::c::BOOL,
                options,
            )
        })?;
        unsafe { Ok(OwnedHandle::from_raw_handle(ret)) }
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl TryFrom<HandleOrInvalid> for OwnedHandle {
    type Error = InvalidHandleError;

    #[inline]
    fn try_from(handle_or_invalid: HandleOrInvalid) -> Result<Self, InvalidHandleError> {
        let handle_or_invalid = ManuallyDrop::new(handle_or_invalid);
        if handle_or_invalid.is_valid() {
            // SAFETY: 该 handle 不是 invalid。
            Ok(unsafe { OwnedHandle::from_raw_handle(handle_or_invalid.0) })
        } else {
            Err(InvalidHandleError(()))
        }
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl Drop for HandleOrInvalid {
    #[inline]
    fn drop(&mut self) {
        if self.is_valid() {
            unsafe {
                let _ = sys::c::CloseHandle(self.0);
            }
        }
    }
}

/// 这是 [`HandleOrNull`] 在尝试转换为 handle 时所使用的错误类型，用于指示该值为 null。
// 这个空字段使得本类型无法被外部构造，同时也为将来扩展留有余地。
#[stable(feature = "io_safety", since = "1.63.0")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NullHandleError(());

#[stable(feature = "io_safety", since = "1.63.0")]
impl fmt::Display for NullHandleError {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        "A HandleOrNull could not be converted to a handle because it was null".fmt(fmt)
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl crate::error::Error for NullHandleError {}

/// 这是 [`HandleOrInvalid`] 在尝试转换为 handle 时所使用的错误类型，用于指示该值为
/// `INVALID_HANDLE_VALUE`。
// 这个空字段使得本类型无法被外部构造，同时也为将来扩展留有余地。
#[stable(feature = "io_safety", since = "1.63.0")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidHandleError(());

#[stable(feature = "io_safety", since = "1.63.0")]
impl fmt::Display for InvalidHandleError {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        "A HandleOrInvalid could not be converted to a handle because it was INVALID_HANDLE_VALUE"
            .fmt(fmt)
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl crate::error::Error for InvalidHandleError {}

#[stable(feature = "io_safety", since = "1.63.0")]
impl AsRawHandle for BorrowedHandle<'_> {
    #[inline]
    fn as_raw_handle(&self) -> RawHandle {
        self.handle
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl AsRawHandle for OwnedHandle {
    #[inline]
    fn as_raw_handle(&self) -> RawHandle {
        self.handle
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl IntoRawHandle for OwnedHandle {
    #[inline]
    fn into_raw_handle(self) -> RawHandle {
        ManuallyDrop::new(self).handle
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl FromRawHandle for OwnedHandle {
    #[inline]
    unsafe fn from_raw_handle(handle: RawHandle) -> Self {
        Self { handle }
    }
}

impl HandleOrNull {
    /// 从给定的 `RawHandle` 构造一个新的 `Self` 实例，该 handle 来自一个以 null 表示
    /// 失败的 Windows API，例如 `CreateThread`。
    ///
    /// 对于以 `INVALID_HANDLE_VALUE` 表示失败的 API，请改用 `HandleOrInvalid` 而非
    /// `HandleOrNull`。
    ///
    /// # 安全性(Safety）
    ///
    /// 传入的 `handle` 值必须要么满足 [`FromRawHandle::from_raw_handle`] 的安全性要求，
    /// 要么为 null。注意并非所有 Windows API 都用 null 表示错误；完整来龙去脉见 [here]。
    ///
    /// [here]: https://devblogs.microsoft.com/oldnewthing/20040302-00/?p=40443
    #[stable(feature = "io_safety", since = "1.63.0")]
    #[inline]
    pub unsafe fn from_raw_handle(handle: RawHandle) -> Self {
        Self(handle)
    }

    fn is_valid(&self) -> bool {
        !self.0.is_null()
    }
}

impl HandleOrInvalid {
    /// 从给定的 `RawHandle` 构造一个新的 `Self` 实例，该 handle 来自一个以
    /// `INVALID_HANDLE_VALUE` 表示失败的 Windows API，例如 `CreateFileW`。
    ///
    /// 对于以 null 表示失败的 API，请改用 `HandleOrNull` 而非 `HandleOrInvalid`。
    ///
    /// # 安全性(Safety）
    ///
    /// 传入的 `handle` 值必须要么满足 [`FromRawHandle::from_raw_handle`] 的安全性要求，
    /// 要么为 `INVALID_HANDLE_VALUE`（-1）。注意并非所有 Windows API 都用
    /// `INVALID_HANDLE_VALUE` 表示错误；完整来龙去脉见 [here]。
    ///
    /// [here]: https://devblogs.microsoft.com/oldnewthing/20040302-00/?p=40443
    #[stable(feature = "io_safety", since = "1.63.0")]
    #[inline]
    pub unsafe fn from_raw_handle(handle: RawHandle) -> Self {
        Self(handle)
    }

    fn is_valid(&self) -> bool {
        self.0 != sys::c::INVALID_HANDLE_VALUE
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl Drop for OwnedHandle {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            let _ = sys::c::CloseHandle(self.handle);
        }
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl fmt::Debug for BorrowedHandle<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BorrowedHandle").field("handle", &self.handle).finish()
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl fmt::Debug for OwnedHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OwnedHandle").field("handle", &self.handle).finish()
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

impl_is_terminal!(BorrowedHandle<'_>, OwnedHandle);

/// 用于从某个底层对象借出其 handle 的 trait。
#[stable(feature = "io_safety", since = "1.63.0")]
pub trait AsHandle {
    /// 借出该 handle。
    ///
    /// # 示例
    ///
    /// ```rust,no_run
    /// use std::fs::File;
    /// # use std::io;
    /// use std::os::windows::io::{AsHandle, BorrowedHandle};
    ///
    /// let mut f = File::open("foo.txt")?;
    /// let borrowed_handle: BorrowedHandle<'_> = f.as_handle();
    /// # Ok::<(), io::Error>(())
    /// ```
    #[stable(feature = "io_safety", since = "1.63.0")]
    fn as_handle(&self) -> BorrowedHandle<'_>;
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl<T: AsHandle + ?Sized> AsHandle for &T {
    #[inline]
    fn as_handle(&self) -> BorrowedHandle<'_> {
        T::as_handle(self)
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl<T: AsHandle + ?Sized> AsHandle for &mut T {
    #[inline]
    fn as_handle(&self) -> BorrowedHandle<'_> {
        T::as_handle(self)
    }
}

#[stable(feature = "as_windows_ptrs", since = "1.71.0")]
/// 这个 impl 使得可以在 Arc 上实现那些要求 `AsHandle` 的 trait。
/// ```
/// # #[cfg(windows)] mod group_cfg {
/// # use std::os::windows::io::AsHandle;
/// use std::fs::File;
/// use std::sync::Arc;
///
/// trait MyTrait: AsHandle {}
/// impl MyTrait for Arc<File> {}
/// impl MyTrait for Box<File> {}
/// # }
/// ```
impl<T: AsHandle + ?Sized> AsHandle for crate::sync::Arc<T> {
    #[inline]
    fn as_handle(&self) -> BorrowedHandle<'_> {
        (**self).as_handle()
    }
}

#[stable(feature = "as_windows_ptrs", since = "1.71.0")]
impl<T: AsHandle + ?Sized> AsHandle for crate::rc::Rc<T> {
    #[inline]
    fn as_handle(&self) -> BorrowedHandle<'_> {
        (**self).as_handle()
    }
}

#[unstable(feature = "unique_rc_arc", issue = "112566")]
impl<T: AsHandle + ?Sized> AsHandle for crate::rc::UniqueRc<T> {
    #[inline]
    fn as_handle(&self) -> BorrowedHandle<'_> {
        (**self).as_handle()
    }
}

#[stable(feature = "as_windows_ptrs", since = "1.71.0")]
impl<T: AsHandle + ?Sized> AsHandle for Box<T> {
    #[inline]
    fn as_handle(&self) -> BorrowedHandle<'_> {
        (**self).as_handle()
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl AsHandle for BorrowedHandle<'_> {
    #[inline]
    fn as_handle(&self) -> BorrowedHandle<'_> {
        *self
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl AsHandle for OwnedHandle {
    #[inline]
    fn as_handle(&self) -> BorrowedHandle<'_> {
        // Safety: `OwnedHandle` 与 `BorrowedHandle` 具有相同的有效性不变量，并且这个
        // `BorrowedHandle` 的生命周期受 `&self` 约束。
        unsafe { BorrowedHandle::borrow_raw(self.as_raw_handle()) }
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl AsHandle for fs::File {
    #[inline]
    fn as_handle(&self) -> BorrowedHandle<'_> {
        self.as_inner().as_handle()
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl From<fs::File> for OwnedHandle {
    /// 接管一个 [`File`](fs::File) 底层文件 handle 的所有权。
    #[inline]
    fn from(file: fs::File) -> OwnedHandle {
        file.into_inner().into_inner().into_inner()
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl From<OwnedHandle> for fs::File {
    /// 返回一个 [`File`](fs::File)，由它接管给定 handle 的所有权。
    #[inline]
    fn from(owned: OwnedHandle) -> Self {
        Self::from_inner(FromInner::from_inner(FromInner::from_inner(owned)))
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl AsHandle for io::Stdin {
    #[inline]
    fn as_handle(&self) -> BorrowedHandle<'_> {
        unsafe { BorrowedHandle::borrow_raw(self.as_raw_handle()) }
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl<'a> AsHandle for io::StdinLock<'a> {
    #[inline]
    fn as_handle(&self) -> BorrowedHandle<'_> {
        unsafe { BorrowedHandle::borrow_raw(self.as_raw_handle()) }
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl AsHandle for io::Stdout {
    #[inline]
    fn as_handle(&self) -> BorrowedHandle<'_> {
        unsafe { BorrowedHandle::borrow_raw(self.as_raw_handle()) }
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl<'a> AsHandle for io::StdoutLock<'a> {
    #[inline]
    fn as_handle(&self) -> BorrowedHandle<'_> {
        unsafe { BorrowedHandle::borrow_raw(self.as_raw_handle()) }
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl AsHandle for io::Stderr {
    #[inline]
    fn as_handle(&self) -> BorrowedHandle<'_> {
        unsafe { BorrowedHandle::borrow_raw(self.as_raw_handle()) }
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl<'a> AsHandle for io::StderrLock<'a> {
    #[inline]
    fn as_handle(&self) -> BorrowedHandle<'_> {
        unsafe { BorrowedHandle::borrow_raw(self.as_raw_handle()) }
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl AsHandle for crate::process::ChildStdin {
    #[inline]
    fn as_handle(&self) -> BorrowedHandle<'_> {
        unsafe { BorrowedHandle::borrow_raw(self.as_raw_handle()) }
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl From<crate::process::ChildStdin> for OwnedHandle {
    /// 接管一个 [`ChildStdin`](crate::process::ChildStdin) 文件 handle 的所有权。
    #[inline]
    fn from(child_stdin: crate::process::ChildStdin) -> OwnedHandle {
        unsafe { OwnedHandle::from_raw_handle(child_stdin.into_raw_handle()) }
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl AsHandle for crate::process::ChildStdout {
    #[inline]
    fn as_handle(&self) -> BorrowedHandle<'_> {
        unsafe { BorrowedHandle::borrow_raw(self.as_raw_handle()) }
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl From<crate::process::ChildStdout> for OwnedHandle {
    /// 接管一个 [`ChildStdout`](crate::process::ChildStdout) 文件 handle 的所有权。
    #[inline]
    fn from(child_stdout: crate::process::ChildStdout) -> OwnedHandle {
        unsafe { OwnedHandle::from_raw_handle(child_stdout.into_raw_handle()) }
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl AsHandle for crate::process::ChildStderr {
    #[inline]
    fn as_handle(&self) -> BorrowedHandle<'_> {
        unsafe { BorrowedHandle::borrow_raw(self.as_raw_handle()) }
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl From<crate::process::ChildStderr> for OwnedHandle {
    /// 接管一个 [`ChildStderr`](crate::process::ChildStderr) 文件 handle 的所有权。
    #[inline]
    fn from(child_stderr: crate::process::ChildStderr) -> OwnedHandle {
        unsafe { OwnedHandle::from_raw_handle(child_stderr.into_raw_handle()) }
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl<T> AsHandle for crate::thread::JoinHandle<T> {
    #[inline]
    fn as_handle(&self) -> BorrowedHandle<'_> {
        unsafe { BorrowedHandle::borrow_raw(self.as_raw_handle()) }
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl<T> From<crate::thread::JoinHandle<T>> for OwnedHandle {
    #[inline]
    fn from(join_handle: crate::thread::JoinHandle<T>) -> OwnedHandle {
        join_handle.into_inner().into_handle().into_inner()
    }
}

#[stable(feature = "anonymous_pipe", since = "1.87.0")]
impl AsHandle for io::PipeReader {
    fn as_handle(&self) -> BorrowedHandle<'_> {
        self.0.as_handle()
    }
}

#[stable(feature = "anonymous_pipe", since = "1.87.0")]
impl From<io::PipeReader> for OwnedHandle {
    fn from(pipe: io::PipeReader) -> Self {
        pipe.into_inner().into_inner()
    }
}

#[stable(feature = "anonymous_pipe", since = "1.87.0")]
impl AsHandle for io::PipeWriter {
    fn as_handle(&self) -> BorrowedHandle<'_> {
        self.0.as_handle()
    }
}

#[stable(feature = "anonymous_pipe", since = "1.87.0")]
impl From<io::PipeWriter> for OwnedHandle {
    fn from(pipe: io::PipeWriter) -> Self {
        pipe.into_inner().into_inner()
    }
}

#[stable(feature = "anonymous_pipe", since = "1.87.0")]
impl From<OwnedHandle> for io::PipeReader {
    fn from(owned_handle: OwnedHandle) -> Self {
        Self::from_inner(FromInner::from_inner(owned_handle))
    }
}

#[stable(feature = "anonymous_pipe", since = "1.87.0")]
impl From<OwnedHandle> for io::PipeWriter {
    fn from(owned_handle: OwnedHandle) -> Self {
        Self::from_inner(FromInner::from_inner(owned_handle))
    }
}
