use libc::{c_int, size_t};

use super::common::*;
use crate::num::NonZero;
use crate::process::StdioPipes;
use crate::sys::pal::fuchsia::*;
use crate::{fmt, io, mem, ptr};

////////////////////////////////////////////////////////////////////////////////
// 命令（Command）
////////////////////////////////////////////////////////////////////////////////

impl Command {
    pub fn spawn(
        &mut self,
        default: Stdio,
        needs_stdin: bool,
    ) -> io::Result<(Process, StdioPipes)> {
        let envp = self.capture_env();

        if self.saw_nul() {
            return Err(io::const_error!(
                io::ErrorKind::InvalidInput,
                "nul byte found in provided data",
            ));
        }

        let (ours, theirs) = self.setup_io(default, needs_stdin)?;

        let process_handle = unsafe { self.do_exec(theirs, envp.as_ref())? };

        Ok((Process { handle: Handle::new(process_handle) }, ours))
    }

    pub fn exec(&mut self, default: Stdio) -> io::Error {
        if self.saw_nul() {
            return io::const_error!(
                io::ErrorKind::InvalidInput,
                "nul byte found in provided data",
            );
        }

        match self.setup_io(default, true) {
            Ok((_, _)) => {
                // FIXME: 这很棘手，因为我们不支持 exec 系列系统调用
                unimplemented!();
            }
            Err(e) => e,
        }
    }

    unsafe fn do_exec(
        &mut self,
        stdio: ChildPipes,
        maybe_envp: Option<&CStringArray>,
    ) -> io::Result<zx_handle_t> {
        let envp = match maybe_envp {
            // None 表示克隆当前的环境，这是通过下面的 flags 来完成的。
            None => ptr::null(),
            Some(envp) => envp.as_ptr(),
        };

        let make_action = |local_io: &ChildStdio, target_fd| -> io::Result<fdio_spawn_action_t> {
            if let Some(local_fd) = local_io.fd() {
                Ok(fdio_spawn_action_t {
                    action: FDIO_SPAWN_ACTION_TRANSFER_FD,
                    local_fd,
                    target_fd,
                    ..Default::default()
                })
            } else {
                if let ChildStdio::Null = local_io {
                    // 充当空操作（no-op）
                    return Ok(Default::default());
                }

                let mut handle = ZX_HANDLE_INVALID;
                let status = fdio_fd_clone(target_fd, &mut handle);
                if status == ZX_ERR_INVALID_ARGS || status == ZX_ERR_NOT_SUPPORTED {
                    // 这个描述符已被关闭；跳过它，而不是生成一个错误。
                    return Ok(Default::default());
                }
                zx_cvt(status)?;

                let mut cloned_fd = 0;
                zx_cvt(fdio_fd_create(handle, &mut cloned_fd))?;

                Ok(fdio_spawn_action_t {
                    action: FDIO_SPAWN_ACTION_TRANSFER_FD,
                    local_fd: cloned_fd as i32,
                    target_fd,
                    ..Default::default()
                })
            }
        };

        // 克隆 stdin、stdout 和 stderr
        let action1 = make_action(&stdio.stdin, 0)?;
        let action2 = make_action(&stdio.stdout, 1)?;
        let action3 = make_action(&stdio.stderr, 2)?;
        let actions = [action1, action2, action3];

        // 我们不希望对任何 stdio 调用 FileDesc::drop。fdio_spawn_etc 总是会消耗（consume）
        // 被转移走的文件描述符。
        mem::forget(stdio);

        for callback in self.get_closures().iter_mut() {
            callback()?;
        }

        let mut process_handle: zx_handle_t = 0;
        zx_cvt(fdio_spawn_etc(
            ZX_HANDLE_INVALID,
            FDIO_SPAWN_CLONE_JOB
                | FDIO_SPAWN_CLONE_LDSVC
                | FDIO_SPAWN_CLONE_NAMESPACE
                | FDIO_SPAWN_CLONE_ENVIRON // 当 envp 非 null 时，这一项会被忽略
                | FDIO_SPAWN_CLONE_UTC_CLOCK,
            self.get_program_cstr().as_ptr(),
            self.get_argv().as_ptr(),
            envp,
            actions.len() as size_t,
            actions.as_ptr(),
            &mut process_handle,
            ptr::null_mut(),
        ))?;
        // FIXME: 看看我们是否想对那个 err_msg 做些什么

        Ok(process_handle)
    }
}

////////////////////////////////////////////////////////////////////////////////
// 进程（Processes）
////////////////////////////////////////////////////////////////////////////////

pub struct Process {
    handle: Handle,
}

impl Process {
    pub fn id(&self) -> u32 {
        self.handle.raw() as u32
    }

    pub fn kill(&mut self) -> io::Result<()> {
        unsafe {
            zx_cvt(zx_task_kill(self.handle.raw()))?;
        }

        Ok(())
    }

    pub fn send_signal(&self, _signal: i32) -> io::Result<()> {
        // Fuchsia 没有与信号（signals）直接对应的等价物
        unimplemented!()
    }

    pub fn wait(&mut self) -> io::Result<ExitStatus> {
        let mut proc_info: zx_info_process_t = Default::default();
        let mut actual: size_t = 0;
        let mut avail: size_t = 0;

        unsafe {
            zx_cvt(zx_object_wait_one(
                self.handle.raw(),
                ZX_TASK_TERMINATED,
                ZX_TIME_INFINITE,
                ptr::null_mut(),
            ))?;
            zx_cvt(zx_object_get_info(
                self.handle.raw(),
                ZX_INFO_PROCESS,
                (&raw mut proc_info) as *mut libc::c_void,
                size_of::<zx_info_process_t>(),
                &mut actual,
                &mut avail,
            ))?;
        }
        if actual != 1 {
            return Err(io::const_error!(
                io::ErrorKind::InvalidData,
                "failed to get exit status of process",
            ));
        }
        Ok(ExitStatus(proc_info.return_code))
    }

    pub fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        let mut proc_info: zx_info_process_t = Default::default();
        let mut actual: size_t = 0;
        let mut avail: size_t = 0;

        unsafe {
            let status =
                zx_object_wait_one(self.handle.raw(), ZX_TASK_TERMINATED, 0, ptr::null_mut());
            match status {
                0 => {} // Success
                x if x == ZX_ERR_TIMED_OUT => {
                    return Ok(None);
                }
                _ => {
                    panic!("Failed to wait on process handle: {status}");
                }
            }
            zx_cvt(zx_object_get_info(
                self.handle.raw(),
                ZX_INFO_PROCESS,
                (&raw mut proc_info) as *mut libc::c_void,
                size_of::<zx_info_process_t>(),
                &mut actual,
                &mut avail,
            ))?;
        }
        if actual != 1 {
            return Err(io::const_error!(
                io::ErrorKind::InvalidData,
                "failed to get exit status of process",
            ));
        }
        Ok(Some(ExitStatus(proc_info.return_code)))
    }
}

#[derive(PartialEq, Eq, Clone, Copy, Debug, Default)]
pub struct ExitStatus(i64);

impl ExitStatus {
    pub fn exit_ok(&self) -> Result<(), ExitStatusError> {
        match NonZero::try_from(self.0) {
            /* was nonzero */ Ok(failure) => Err(ExitStatusError(failure)),
            /* was zero, couldn't convert */ Err(_) => Ok(()),
        }
    }

    pub fn code(&self) -> Option<i32> {
        // FIXME: 支持把返回码（return code）提取为 i64
        self.0.try_into().ok()
    }

    pub fn signal(&self) -> Option<i32> {
        None
    }

    // FIXME: unix.rs 中真正的 Unix 实现使用了 WSTOPSIG、WCOREDUMP 等等。
    // 从上面 `success`、`code` 和 `signal` 的实现来看，我推断这些在 Fuchsia 上不可用。
    //
    // 看起来 Fuchsia 并没有 Unix-like 到足以正确实现 ExitStatus（或者说 std::os::unix 中
    // 许多其他东西）。这层贴皮（veneer）注定总是个权宜之计（bodge）。因此，尽管我不知道
    // 这些实现是否真的正确，但我认为它们至少目前够用了。
    pub fn core_dumped(&self) -> bool {
        false
    }
    pub fn stopped_signal(&self) -> Option<i32> {
        None
    }
    pub fn continued(&self) -> bool {
        false
    }

    pub fn into_raw(&self) -> c_int {
        // 我们不知道调用 into_raw() 的人会拿这个值做什么，但它应当具有惯常的
        // Unix 表示形式。尽管这一点在 SuS 或 POSIX 中并未标准化，但所有 Unix 系统
        // 都以相同的方式编码信号和退出状态。（即 WIFEXITED、WEXITSTATUS 等宏在每个
        // Unix 上都有完全相同的行为。）
        //
        // `std::os::unix::into_raw` 的调用者大概是想要一个 Unix 退出状态，并且可能会
        // 自己进行移位和掩码操作，甚至把该状态传给另一台运行不同 Unix 变体的计算机。
        //
        // 另一种看法是说：Fuchsia 上的调用者本应知道 `into_raw` 会给出一个原始的
        // Fuchsia 状态（不管那是什么——我个人也不清楚）。但这里做不到，因为我们必须返回
        // 一个 c_int，因为 Unix（包括 SuS 和 POSIX）规定 wait status 就是 c_int，
        // 而 Fuchsia 显然使用 u64，所以它不一定能放得下。
        //
        // 在我看来，正确的答案应该是为 std::os::fuchsia 提供它自己的 ExitStatusExt，
        // 而不是去尝试提供一个不那么令人信服的、对 Unix 的模仿。也就是说，
        // std::os::unix::process:ExitStatusExt 本不应在 Fuchsia 上存在。但把这一点修好
        // 已超出我现在的精力范围。
        let exit_status_as_if_unix: u8 = self.0.try_into().expect("Fuchsia process return code bigger than 8 bits, but std::os::unix::ExitStatusExt::into_raw() was called to try to convert the value into a traditional Unix-style wait status, which cannot represent values greater than 255.");
        let wait_status_as_if_unix = (exit_status_as_if_unix as c_int) << 8;
        wait_status_as_if_unix
    }
}

/// 通过包装一个原始的 `c_int`（不进行拷贝）来把它转换为类型安全的 `ExitStatus`。
impl From<c_int> for ExitStatus {
    fn from(a: c_int) -> ExitStatus {
        ExitStatus(a as i64)
    }
}

impl fmt::Display for ExitStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "exit code: {}", self.0)
    }
}

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub struct ExitStatusError(NonZero<i64>);

impl Into<ExitStatus> for ExitStatusError {
    fn into(self) -> ExitStatus {
        ExitStatus(self.0.into())
    }
}

impl ExitStatusError {
    pub fn code(self) -> Option<NonZero<i32>> {
        // fixme: 受与 ExitStatus::code() 相同的 bug 影响
        ExitStatus(self.0.into()).code().map(|st| st.try_into().unwrap())
    }
}
