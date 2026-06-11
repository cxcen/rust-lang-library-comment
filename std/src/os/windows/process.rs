//! Windows 平台对 [`std::process`] 模块中各原语的特定扩展。
//!
//! [`std::process`]: crate::process

#![stable(feature = "process_extensions", since = "1.2.0")]

use crate::ffi::{OsStr, c_void};
use crate::mem::MaybeUninit;
use crate::os::windows::io::{
    AsHandle, AsRawHandle, BorrowedHandle, FromRawHandle, IntoRawHandle, OwnedHandle, RawHandle,
};
use crate::sealed::Sealed;
use crate::sys::{AsInner, AsInnerMut, FromInner, IntoInner};
use crate::{io, marker, process, ptr, sys};

#[stable(feature = "process_extensions", since = "1.2.0")]
impl FromRawHandle for process::Stdio {
    unsafe fn from_raw_handle(handle: RawHandle) -> process::Stdio {
        let handle = unsafe { sys::handle::Handle::from_raw_handle(handle as *mut _) };
        let io = sys::process::Stdio::Handle(handle);
        process::Stdio::from_inner(io)
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl From<OwnedHandle> for process::Stdio {
    /// 接管一个 handle 的所有权，返回一个能把流附着到它上面的 [`Stdio`](process::Stdio)。
    fn from(handle: OwnedHandle) -> process::Stdio {
        let handle = sys::handle::Handle::from_inner(handle);
        let io = sys::process::Stdio::Handle(handle);
        process::Stdio::from_inner(io)
    }
}

#[stable(feature = "process_extensions", since = "1.2.0")]
impl AsRawHandle for process::Child {
    #[inline]
    fn as_raw_handle(&self) -> RawHandle {
        self.as_inner().handle().as_raw_handle() as *mut _
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl AsHandle for process::Child {
    #[inline]
    fn as_handle(&self) -> BorrowedHandle<'_> {
        self.as_inner().handle().as_handle()
    }
}

#[stable(feature = "into_raw_os", since = "1.4.0")]
impl IntoRawHandle for process::Child {
    fn into_raw_handle(self) -> RawHandle {
        self.into_inner().into_handle().into_raw_handle() as *mut _
    }
}

#[stable(feature = "io_safety", since = "1.63.0")]
impl From<process::Child> for OwnedHandle {
    /// 接管一个 [`Child`](process::Child) 进程 handle 的所有权。
    fn from(child: process::Child) -> OwnedHandle {
        child.into_inner().into_handle().into_inner()
    }
}

#[stable(feature = "process_extensions", since = "1.2.0")]
impl AsRawHandle for process::ChildStdin {
    #[inline]
    fn as_raw_handle(&self) -> RawHandle {
        self.as_inner().handle().as_raw_handle() as *mut _
    }
}

#[stable(feature = "process_extensions", since = "1.2.0")]
impl AsRawHandle for process::ChildStdout {
    #[inline]
    fn as_raw_handle(&self) -> RawHandle {
        self.as_inner().handle().as_raw_handle() as *mut _
    }
}

#[stable(feature = "process_extensions", since = "1.2.0")]
impl AsRawHandle for process::ChildStderr {
    #[inline]
    fn as_raw_handle(&self) -> RawHandle {
        self.as_inner().handle().as_raw_handle() as *mut _
    }
}

#[stable(feature = "into_raw_os", since = "1.4.0")]
impl IntoRawHandle for process::ChildStdin {
    fn into_raw_handle(self) -> RawHandle {
        self.into_inner().into_handle().into_raw_handle() as *mut _
    }
}

#[stable(feature = "into_raw_os", since = "1.4.0")]
impl IntoRawHandle for process::ChildStdout {
    fn into_raw_handle(self) -> RawHandle {
        self.into_inner().into_handle().into_raw_handle() as *mut _
    }
}

#[stable(feature = "into_raw_os", since = "1.4.0")]
impl IntoRawHandle for process::ChildStderr {
    fn into_raw_handle(self) -> RawHandle {
        self.into_inner().into_handle().into_raw_handle() as *mut _
    }
}

/// 从所提供的 `OwnedHandle` 创建一个 `ChildStdin`。
///
/// 所提供的 handle 必须是异步（asynchronous）的，因为对它的读写是用异步 API 实现的。
#[stable(feature = "child_stream_from_fd", since = "1.74.0")]
impl From<OwnedHandle> for process::ChildStdin {
    fn from(handle: OwnedHandle) -> process::ChildStdin {
        let handle = sys::handle::Handle::from_inner(handle);
        let pipe = sys::process::ChildPipe::from_inner(handle);
        process::ChildStdin::from_inner(pipe)
    }
}

/// 从所提供的 `OwnedHandle` 创建一个 `ChildStdout`。
///
/// 所提供的 handle 必须是异步（asynchronous）的，因为对它的读写是用异步 API 实现的。
#[stable(feature = "child_stream_from_fd", since = "1.74.0")]
impl From<OwnedHandle> for process::ChildStdout {
    fn from(handle: OwnedHandle) -> process::ChildStdout {
        let handle = sys::handle::Handle::from_inner(handle);
        let pipe = sys::process::ChildPipe::from_inner(handle);
        process::ChildStdout::from_inner(pipe)
    }
}

/// 从所提供的 `OwnedHandle` 创建一个 `ChildStderr`。
///
/// 所提供的 handle 必须是异步（asynchronous）的，因为对它的读写是用异步 API 实现的。
#[stable(feature = "child_stream_from_fd", since = "1.74.0")]
impl From<OwnedHandle> for process::ChildStderr {
    fn from(handle: OwnedHandle) -> process::ChildStderr {
        let handle = sys::handle::Handle::from_inner(handle);
        let pipe = sys::process::ChildPipe::from_inner(handle);
        process::ChildStderr::from_inner(pipe)
    }
}

/// Windows 平台对 [`process::ExitStatus`] 的特定扩展。
///
/// 本 trait 是密封的（sealed）：无法在标准库之外被实现。这样一来，将来新增方法就不会
/// 构成破坏性变更（breaking change）。
#[stable(feature = "exit_status_from", since = "1.12.0")]
pub trait ExitStatusExt: Sealed {
    /// 从某个进程底层的裸 `u32` 返回值创建一个新的 `ExitStatus`。
    #[stable(feature = "exit_status_from", since = "1.12.0")]
    fn from_raw(raw: u32) -> Self;
}

#[stable(feature = "exit_status_from", since = "1.12.0")]
impl ExitStatusExt for process::ExitStatus {
    fn from_raw(raw: u32) -> Self {
        process::ExitStatus::from_inner(From::from(raw))
    }
}

/// Windows 平台对 [`process::Command`] 构造器（builder）的特定扩展。
///
/// 本 trait 是密封的（sealed）：无法在标准库之外被实现。这样一来，将来新增方法就不会
/// 构成破坏性变更（breaking change）。
#[stable(feature = "windows_process_extensions", since = "1.16.0")]
pub trait CommandExt: Sealed {
    /// 设置要传递给 `CreateProcess` 的[进程创建标志（process creation flags）][1]。
    ///
    /// 这些标志将始终与 `CREATE_UNICODE_ENVIRONMENT` 进行按位或（OR）运算。
    ///
    /// [1]: https://docs.microsoft.com/en-us/windows/win32/procthread/process-creation-flags
    #[stable(feature = "windows_process_extensions", since = "1.16.0")]
    fn creation_flags(&mut self, flags: u32) -> &mut process::Command;

    /// 设置传递给 `CreateProcess` 的 [STARTUPINFO][1] 的 `wShowWindow` 字段。
    /// 允许的取值是下列文档中所列的那些：
    /// <https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-showwindow>
    ///
    /// [1]: <https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/ns-processthreadsapi-startupinfow>
    #[unstable(feature = "windows_process_extensions_show_window", issue = "127544")]
    fn show_window(&mut self, cmd_show: u16) -> &mut process::Command;

    /// 强制把所有参数都用引号（`"`）字符包裹起来。
    ///
    /// 这在向基于 [MSYS2/Cygwin][1] 的可执行程序传递参数时很有用：这些程序会通过搜索任何
    /// 匹配该通配符模式的文件路径，来展开未加引号、含有通配符字符（`?` 和 `*`）的参数。
    ///
    /// 在向使用 [msvcrt][2] 的程序传递参数时，加引号没有任何效果。这包括用 MinGW 和 MSVC
    /// 构建的程序。
    ///
    /// [1]: <https://github.com/msys2/MSYS2-packages/issues/2176>
    /// [2]: <https://msdn.microsoft.com/en-us/library/17w5ykft.aspx>
    #[unstable(feature = "windows_process_extensions_force_quotes", issue = "82227")]
    fn force_quotes(&mut self, enabled: bool) -> &mut process::Command;

    /// 把字面文本（literal text）追加到命令行中，不做任何加引号或转义处理。
    ///
    /// 这在向那些不遵循标准 C 运行时转义规则的应用程序传递参数时很有用，例如 `cmd.exe /c`。
    ///
    /// # Batch files
    ///
    /// 注意 `cmd /c` 的命令行所用的转义规则与批处理文件（batch file）本身的略有不同。
    /// 如果可能，更好的做法或许是把复杂参数连同适当的转义一起写入一个临时的 `.bat` 文件，
    /// 然后直接用下面的方式运行它：
    ///
    /// ```no_run
    /// # use std::process::Command;
    /// # let temp_bat_file = "";
    /// # #[allow(unused)]
    /// let output = Command::new("cmd").args(["/c", &format!("\"{temp_bat_file}\"")]).output();
    /// ```
    ///
    /// # Example
    ///
    /// 同时使用可信参数和不可信参数来运行一个批处理脚本。
    ///
    /// ```no_run
    /// #[cfg(windows)]
    /// // `my_script_path` 是指向已知 bat 文件的路径。
    /// // `user_name` 是由用户给出的不可信名字。
    /// fn run_script(
    ///     my_script_path: &str,
    ///     user_name: &str,
    /// ) -> Result<std::process::Output, std::io::Error> {
    ///     use std::io::{Error, ErrorKind};
    ///     use std::os::windows::process::CommandExt;
    ///     use std::process::Command;
    ///
    ///     // 构造命令行，注意一定要把脚本路径用引号括起来。
    ///     // 这里假定那些固定参数已经过测试，确认能与我们所用的脚本配合工作。
    ///     let mut cmd_args = format!(r#""{my_script_path}" "--features=[a,b,c]""#);
    ///
    ///     // 确保用户名是安全的。尤其需要警惕那些 cmd 可能会做特殊解释的 ascii 符号。
    ///     // 这里我们只允许字母数字字符。
    ///     if !user_name.chars().all(|c| c.is_alphanumeric()) {
    ///         return Err(Error::new(ErrorKind::InvalidInput, "invalid user name"));
    ///     }
    ///
    ///     // 现在我们已校验了用户名，把它也加进去。
    ///     cmd_args.push_str(" --user ");
    ///     cmd_args.push_str(user_name);
    ///
    ///     // 调用 cmd.exe 并返回其输出
    ///     Command::new("cmd.exe")
    ///         .arg("/c")
    ///         // 按 cmd.exe 的要求，用额外一对引号把整条命令包裹起来。
    ///         .raw_arg(&format!("\"{cmd_args}\""))
    ///         .output()
    /// }
    /// ````
    #[stable(feature = "windows_process_extensions_raw_arg", since = "1.62.0")]
    fn raw_arg<S: AsRef<OsStr>>(&mut self, text_to_append_as_is: S) -> &mut process::Command;

    /// 当 [`process::Command`] 创建管道（pipe）时，要求我方这一端始终是异步的。
    ///
    /// 默认情况下，[`process::Command`] 可能会选择使用两端都以同步读写操作打开的管道。
    /// 通过使用 `async_pipes(true)`，这一行为会被覆盖，使得我方这一端始终是异步的。
    ///
    /// 这一点很重要，因为若要进行异步 I/O，管道或文件必须以异步访问方式打开。
    ///
    /// 无论本选项如何设置，发送给子进程的那一端管道将始终是同步的。
    ///
    /// # Example
    ///
    /// ```
    /// #![feature(windows_process_extensions_async_pipes)]
    /// use std::os::windows::process::CommandExt;
    /// use std::process::{Command, Stdio};
    ///
    /// # let program = "";
    ///
    /// Command::new(program)
    ///     .async_pipes(true)
    ///     .stdin(Stdio::piped())
    ///     .stdout(Stdio::piped())
    ///     .stderr(Stdio::piped());
    /// ```
    #[unstable(feature = "windows_process_extensions_async_pipes", issue = "98289")]
    fn async_pipes(&mut self, always_async: bool) -> &mut process::Command;

    /// 用给定的 [`ProcThreadAttributeList`] 把该命令作为子进程执行，返回指向它的 handle。
    ///
    /// 本方法允许在 Windows 系统上对所派生子进程的各项属性（attribute）进行定制。
    /// 属性为进程创建提供了扩展的可配置性，但其用法可能错综复杂，且有潜在的 unsafe 性。
    ///
    /// # Note
    ///
    /// 默认情况下，stdin、stdout 和 stderr 从父进程继承而来。
    ///
    /// # Example
    ///
    /// ```
    /// #![feature(windows_process_extensions_raw_attribute)]
    /// use std::os::windows::io::AsRawHandle;
    /// use std::os::windows::process::{CommandExt, ProcThreadAttributeList};
    /// use std::process::Command;
    ///
    /// # struct ProcessDropGuard(std::process::Child);
    /// # impl Drop for ProcessDropGuard {
    /// #     fn drop(&mut self) {
    /// #         let _ = self.0.kill();
    /// #     }
    /// # }
    /// #
    /// let parent = Command::new("cmd").spawn()?;
    /// let parent_process_handle = parent.as_raw_handle();
    /// # let parent = ProcessDropGuard(parent);
    ///
    /// const PROC_THREAD_ATTRIBUTE_PARENT_PROCESS: usize = 0x00020000;
    /// let mut attribute_list = ProcThreadAttributeList::build()
    ///     .attribute(PROC_THREAD_ATTRIBUTE_PARENT_PROCESS, &parent_process_handle)
    ///     .finish()
    ///     .unwrap();
    ///
    /// let mut child = Command::new("cmd").spawn_with_attributes(&attribute_list)?;
    /// #
    /// # child.kill()?;
    /// # Ok::<(), std::io::Error>(())
    /// ```
    #[unstable(feature = "windows_process_extensions_raw_attribute", issue = "114854")]
    fn spawn_with_attributes(
        &mut self,
        attribute_list: &ProcThreadAttributeList<'_>,
    ) -> io::Result<process::Child>;

    /// 当为 true 时，在把 [STARTUPINFO][1] 结构体传递给 `CreateProcess` 之前，在其上设置 `STARTF_RUNFULLSCREEN` 标志。
    ///
    /// [1]: https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/ns-processthreadsapi-startupinfoa
    #[unstable(feature = "windows_process_extensions_startupinfo", issue = "141010")]
    fn startupinfo_fullscreen(&mut self, enabled: bool) -> &mut process::Command;

    /// 当为 true 时，在把 [STARTUPINFO][1] 结构体传递给 `CreateProcess` 之前，在其上设置 `STARTF_UNTRUSTEDSOURCE` 标志。
    ///
    /// [1]: https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/ns-processthreadsapi-startupinfoa
    #[unstable(feature = "windows_process_extensions_startupinfo", issue = "141010")]
    fn startupinfo_untrusted_source(&mut self, enabled: bool) -> &mut process::Command;

    /// 当指定时，在把 [STARTUPINFO][1] 结构体传递给 `CreateProcess` 之前，在其上设置以下标志：
    /// - 如果是 `Some(true)`，设置 `STARTF_FORCEONFEEDBACK`
    /// - 如果是 `Some(false)`，设置 `STARTF_FORCEOFFFEEDBACK`
    /// - 如果是 `None`，不设置任何标志
    ///
    /// [1]: https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/ns-processthreadsapi-startupinfoa
    #[unstable(feature = "windows_process_extensions_startupinfo", issue = "141010")]
    fn startupinfo_force_feedback(&mut self, enabled: Option<bool>) -> &mut process::Command;

    /// 如果该标志被设置为 `true`，则调用进程中每个可继承（inheritable）的 handle 都会被新
    /// 进程继承。如果该标志为 `false`，则这些 handle 不会被继承。
    ///
    /// 该标志的默认值为 `true`。
    ///
    /// **注意**：被继承的 handle 与原始 handle 具有相同的值和访问权限。关于可继承 handle
    /// 的更多讨论，参见 `CreateProcessW` 文档的 [Remarks][1] 一节。
    ///
    /// [1]: https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-createprocessw#remarks
    #[unstable(feature = "windows_process_extensions_inherit_handles", issue = "146407")]
    fn inherit_handles(&mut self, inherit_handles: bool) -> &mut process::Command;
}

#[stable(feature = "windows_process_extensions", since = "1.16.0")]
impl CommandExt for process::Command {
    fn creation_flags(&mut self, flags: u32) -> &mut process::Command {
        self.as_inner_mut().creation_flags(flags);
        self
    }

    fn show_window(&mut self, cmd_show: u16) -> &mut process::Command {
        self.as_inner_mut().show_window(Some(cmd_show));
        self
    }

    fn force_quotes(&mut self, enabled: bool) -> &mut process::Command {
        self.as_inner_mut().force_quotes(enabled);
        self
    }

    fn raw_arg<S: AsRef<OsStr>>(&mut self, raw_text: S) -> &mut process::Command {
        self.as_inner_mut().raw_arg(raw_text.as_ref());
        self
    }

    fn async_pipes(&mut self, always_async: bool) -> &mut process::Command {
        // FIXME: 当前这是一个有意为之的空操作（no-op）实现。
        // 目前，我方这一端的管道将始终是异步的。
        // 一旦生态系统完成了相应调整，我们或许就能开始在标准库内部使用同步管道。
        let _ = always_async;
        self
    }

    fn spawn_with_attributes(
        &mut self,
        attribute_list: &ProcThreadAttributeList<'_>,
    ) -> io::Result<process::Child> {
        self.as_inner_mut()
            .spawn_with_attributes(sys::process::Stdio::Inherit, true, Some(attribute_list))
            .map(process::Child::from_inner)
    }

    fn startupinfo_fullscreen(&mut self, enabled: bool) -> &mut process::Command {
        self.as_inner_mut().startupinfo_fullscreen(enabled);
        self
    }

    fn startupinfo_untrusted_source(&mut self, enabled: bool) -> &mut process::Command {
        self.as_inner_mut().startupinfo_untrusted_source(enabled);
        self
    }

    fn startupinfo_force_feedback(&mut self, enabled: Option<bool>) -> &mut process::Command {
        self.as_inner_mut().startupinfo_force_feedback(enabled);
        self
    }

    fn inherit_handles(&mut self, inherit_handles: bool) -> &mut process::Command {
        self.as_inner_mut().inherit_handles(inherit_handles);
        self
    }
}

#[unstable(feature = "windows_process_extensions_main_thread_handle", issue = "96723")]
pub trait ChildExt: Sealed {
    /// 提取主线程的裸 handle，且不取得其所有权
    #[unstable(feature = "windows_process_extensions_main_thread_handle", issue = "96723")]
    fn main_thread_handle(&self) -> BorrowedHandle<'_>;
}

#[unstable(feature = "windows_process_extensions_main_thread_handle", issue = "96723")]
impl ChildExt for process::Child {
    fn main_thread_handle(&self) -> BorrowedHandle<'_> {
        self.handle.main_thread_handle()
    }
}

/// Windows 平台对 [`process::ExitCode`] 的特定扩展。
///
/// 本 trait 是密封的（sealed）：无法在标准库之外被实现。这样一来，将来新增方法就不会
/// 构成破坏性变更（breaking change）。
#[unstable(feature = "windows_process_exit_code_from", issue = "111688")]
pub trait ExitCodeExt: Sealed {
    /// 从某个进程底层的裸 `u32` 返回值创建一个新的 `ExitCode`。
    ///
    /// 该退出码不应为 259，因为这会与 `GetExitCodeProcess` 函数返回的 `STILL_ACTIVE`
    /// 宏相冲突——后者用于表示进程尚未运行至完成。
    #[unstable(feature = "windows_process_exit_code_from", issue = "111688")]
    fn from_raw(raw: u32) -> Self;
}

#[unstable(feature = "windows_process_exit_code_from", issue = "111688")]
impl ExitCodeExt for process::ExitCode {
    fn from_raw(raw: u32) -> Self {
        process::ExitCode::from_inner(From::from(raw))
    }
}

/// 对 windows 的 [`ProcThreadAttributeList`][1] 的封装。
///
/// [1]: <https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-initializeprocthreadattributelist>
#[derive(Debug)]
#[unstable(feature = "windows_process_extensions_raw_attribute", issue = "114854")]
pub struct ProcThreadAttributeList<'a> {
    attribute_list: Box<[MaybeUninit<u8>]>,
    _lifetime_marker: marker::PhantomData<&'a ()>,
}

#[unstable(feature = "windows_process_extensions_raw_attribute", issue = "114854")]
impl<'a> ProcThreadAttributeList<'a> {
    /// 创建一个用于构造 [`ProcThreadAttributeList`] 的新构造器（builder）。
    pub fn build() -> ProcThreadAttributeListBuilder<'a> {
        ProcThreadAttributeListBuilder::new()
    }

    /// 返回指向底层属性列表的指针。
    #[doc(hidden)]
    pub fn as_ptr(&self) -> *const MaybeUninit<u8> {
        self.attribute_list.as_ptr()
    }
}

#[unstable(feature = "windows_process_extensions_raw_attribute", issue = "114854")]
impl<'a> Drop for ProcThreadAttributeList<'a> {
    /// 删除该属性列表。
    ///
    /// 本方法调用 [`DeleteProcThreadAttributeList`][1] 来删除底层的属性列表。
    ///
    /// [1]: <https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-deleteprocthreadattributelist>
    fn drop(&mut self) {
        let lp_attribute_list = self.attribute_list.as_mut_ptr().cast::<c_void>();
        unsafe { sys::c::DeleteProcThreadAttributeList(lp_attribute_list) }
    }
}

/// 用于构造 [`ProcThreadAttributeList`] 的构造器（builder）。
#[derive(Clone, Debug)]
#[unstable(feature = "windows_process_extensions_raw_attribute", issue = "114854")]
pub struct ProcThreadAttributeListBuilder<'a> {
    attributes: alloc::collections::BTreeMap<usize, ProcThreadAttributeValue>,
    _lifetime_marker: marker::PhantomData<&'a ()>,
}

#[unstable(feature = "windows_process_extensions_raw_attribute", issue = "114854")]
impl<'a> ProcThreadAttributeListBuilder<'a> {
    fn new() -> Self {
        ProcThreadAttributeListBuilder {
            attributes: alloc::collections::BTreeMap::new(),
            _lifetime_marker: marker::PhantomData,
        }
    }

    /// 在属性列表上设置一个属性。
    ///
    /// `attribute` 参数指定要设置的裸属性，而 `value` 参数则持有与该属性关联的值。
    /// 关于有效属性的列表，请参阅 [Windows 文档][1]。
    ///
    /// # Note
    ///
    /// 属性的最大数量为 [`u32::MAX`] 的值。如果超出此上限，对 [`Self::finish`] 的调用将
    /// 返回一个 `Error`，表示已超出属性的最大数量。
    ///
    /// # Safety Note
    ///
    /// 请记住，对属性的不当使用可能导致未定义行为（undefined behavior）或安全漏洞。
    /// 务必查阅文档，并确保使用了正确的属性值。
    ///
    /// [1]: <https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-updateprocthreadattribute#parameters>
    pub fn attribute<T>(self, attribute: usize, value: &'a T) -> Self {
        unsafe {
            self.raw_attribute(attribute, ptr::addr_of!(*value).cast::<c_void>(), size_of::<T>())
        }
    }

    /// 在属性列表上设置一个裸属性。
    ///
    /// 本函数适用于设置那些指针或大小无法直接从其值推导出来的属性。
    ///
    /// # 安全性(Safety）
    ///
    /// 本函数被标记为 `unsafe`，因为它涉及裸指针和大小。调用者有责任确保该值的存活时间
    /// 长于所得到的 [`ProcThreadAttributeList`]，并确保 size 参数的有效性。
    ///
    /// # Example
    ///
    /// ```
    /// #![feature(windows_process_extensions_raw_attribute)]
    /// use std::ffi::c_void;
    /// use std::os::windows::process::{CommandExt, ProcThreadAttributeList};
    /// use std::os::windows::raw::HANDLE;
    /// use std::process::Command;
    ///
    /// #[repr(C)]
    /// pub struct COORD {
    ///     pub X: i16,
    ///     pub Y: i16,
    /// }
    ///
    /// unsafe extern "system" {
    ///     fn CreatePipe(
    ///         hreadpipe: *mut HANDLE,
    ///         hwritepipe: *mut HANDLE,
    ///         lppipeattributes: *const c_void,
    ///         nsize: u32,
    ///     ) -> i32;
    ///     fn CreatePseudoConsole(
    ///         size: COORD,
    ///         hinput: HANDLE,
    ///         houtput: HANDLE,
    ///         dwflags: u32,
    ///         phpc: *mut isize,
    ///     ) -> i32;
    ///     fn CloseHandle(hobject: HANDLE) -> i32;
    /// }
    ///
    /// let [mut input_read_side, mut output_write_side, mut output_read_side, mut input_write_side] =
    ///     [unsafe { std::mem::zeroed::<HANDLE>() }; 4];
    ///
    /// unsafe {
    ///     CreatePipe(&mut input_read_side, &mut input_write_side, std::ptr::null(), 0);
    ///     CreatePipe(&mut output_read_side, &mut output_write_side, std::ptr::null(), 0);
    /// }
    ///
    /// let size = COORD { X: 60, Y: 40 };
    /// let mut h_pc = unsafe { std::mem::zeroed() };
    /// unsafe { CreatePseudoConsole(size, input_read_side, output_write_side, 0, &mut h_pc) };
    ///
    /// unsafe { CloseHandle(input_read_side) };
    /// unsafe { CloseHandle(output_write_side) };
    ///
    /// const PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE: usize = 131094;
    ///
    /// let attribute_list = unsafe {
    ///     ProcThreadAttributeList::build()
    ///         .raw_attribute(
    ///             PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE,
    ///             h_pc as *const c_void,
    ///             size_of::<isize>(),
    ///         )
    ///         .finish()?
    /// };
    ///
    /// let mut child = Command::new("cmd").spawn_with_attributes(&attribute_list)?;
    /// #
    /// # child.kill()?;
    /// # Ok::<(), std::io::Error>(())
    /// ```
    pub unsafe fn raw_attribute<T>(
        mut self,
        attribute: usize,
        value_ptr: *const T,
        value_size: usize,
    ) -> Self {
        self.attributes.insert(
            attribute,
            ProcThreadAttributeValue { ptr: value_ptr.cast::<c_void>(), size: value_size },
        );
        self
    }

    /// 完成 `ProcThreadAttributeList` 的构造。
    ///
    /// # Errors
    ///
    /// 如果超出了属性的最大数量，或者在初始化过程中发生 I/O 错误，则返回一个错误。
    pub fn finish(&self) -> io::Result<ProcThreadAttributeList<'a>> {
        // 为了初始化我们的 ProcThreadAttributeList，需要确定要为它分配多少字节。
        // Windows API 简化了这一过程：它允许我们用一个 null 指针来调用
        // `InitializeProcThreadAttributeList`，从而取回所需的大小。
        let mut required_size = 0;
        let Ok(attribute_count) = self.attributes.len().try_into() else {
            return Err(io::const_error!(
                io::ErrorKind::InvalidInput,
                "maximum number of ProcThreadAttributes exceeded",
            ));
        };
        unsafe {
            sys::c::InitializeProcThreadAttributeList(
                ptr::null_mut(),
                attribute_count,
                0,
                &mut required_size,
            )
        };

        let mut attribute_list = vec![MaybeUninit::uninit(); required_size].into_boxed_slice();

        // 一旦分配了所需的内存，再调用 `InitializeProcThreadAttributeList` 来正确地初始化
        // 该列表就是安全的了。
        sys::cvt(unsafe {
            sys::c::InitializeProcThreadAttributeList(
                attribute_list.as_mut_ptr().cast::<c_void>(),
                attribute_count,
                0,
                &mut required_size,
            )
        })?;

        // # 把我们的属性加入到缓冲区中。
        // 理论上属性数量有可能超过一个 u32 的值。因此，我们要确保所添加的属性数量不超过
        // 缓冲区初始化时所针对的数量。
        for (&attribute, value) in self.attributes.iter().take(attribute_count as usize) {
            sys::cvt(unsafe {
                sys::c::UpdateProcThreadAttribute(
                    attribute_list.as_mut_ptr().cast::<c_void>(),
                    0,
                    attribute,
                    value.ptr,
                    value.size,
                    ptr::null_mut(),
                    ptr::null_mut(),
                )
            })?;
        }

        Ok(ProcThreadAttributeList { attribute_list, _lifetime_marker: marker::PhantomData })
    }
}

/// 对要用作进程线程属性（Process Thread Attribute）的值数据的封装。
#[derive(Clone, Debug)]
struct ProcThreadAttributeValue {
    ptr: *const c_void,
    size: usize,
}
