#![unstable(feature = "process_internals", issue = "none")]

#[cfg(test)]
mod tests;

use core::ffi::c_void;

use super::env::{CommandEnv, CommandEnvs};
use crate::collections::BTreeMap;
use crate::env::consts::{EXE_EXTENSION, EXE_SUFFIX};
use crate::ffi::{OsStr, OsString};
use crate::io::{self, Error};
use crate::num::NonZero;
use crate::os::windows::ffi::{OsStrExt, OsStringExt};
use crate::os::windows::io::{AsHandle, AsRawHandle, BorrowedHandle, FromRawHandle, IntoRawHandle};
use crate::os::windows::process::ProcThreadAttributeList;
use crate::path::{Path, PathBuf};
use crate::process::StdioPipes;
use crate::sync::Mutex;
use crate::sys::args::{self, Arg};
use crate::sys::c::{self, EXIT_FAILURE, EXIT_SUCCESS};
use crate::sys::fs::{File, OpenOptions};
use crate::sys::handle::Handle;
use crate::sys::pal::api::{self, WinError, utf16};
use crate::sys::pal::{ensure_no_nuls, fill_utf16_buf};
use crate::sys::{IntoInner, cvt, path, stdio};
use crate::{cmp, env, fmt, ptr};

mod child_pipe;

pub use self::child_pipe::{ChildPipe, read_output};

////////////////////////////////////////////////////////////////////////////////
// 命令（Command）
////////////////////////////////////////////////////////////////////////////////

#[derive(Clone, Debug, Eq)]
#[doc(hidden)]
pub struct EnvKey {
    os_string: OsString,
    // 它存储一个 UTF-16 编码的字符串，以绕开 Rust 的 OsString（WTF-8）
    // 与 Windows API 字符串类型（UTF-16）之间的不匹配。
    // 通常在每次 API 调用时进行转换是可以接受的，但在这里
    // `c::CompareStringOrdinal` 会在每次使用 `==` 时被调用。
    utf16: Vec<u16>,
}

impl EnvKey {
    fn new<T: Into<OsString>>(key: T) -> Self {
        EnvKey::from(key.into())
    }
}

// 比较 Windows 环境变量的键[1]，在行为上等价于两个操作的组合[2]：
//
// 1. 对两个字符串都做大小写折叠（case-fold）。这是使用一个与语言无关的、
// Windows 独有的大写映射来完成的（尽管它基于较旧的 Unicode 规范的数据）。
// 它只对单个 UTF-16 码元进行操作，因此代理项（surrogates）保持不变。
// 这个大写映射在不同的 Windows 版本之间有可能发生变化。
//
// 2. 对字符串执行序数（ordinal）比较。使用序数的比较，仅仅是基于每个 UTF-16 码元
// 的数值进行的比较[3]。
//
// 由于这个大小写折叠映射是 Windows 独有的、且不保证稳定，我们请求操作系统来替我们
// 比较这两个字符串。这是通过把 `bIgnoreCase` 设为 `TRUE` 来调用
// `CompareStringOrdinal`[4] 完成的。
//
// [1] https://docs.microsoft.com/en-us/dotnet/standard/base-types/best-practices-strings#choosing-a-stringcomparison-member-for-your-method-call
// [2] https://docs.microsoft.com/en-us/dotnet/standard/base-types/best-practices-strings#stringtoupper-and-stringtolower
// [3] https://docs.microsoft.com/en-us/dotnet/api/system.stringcomparison?view=net-5.0#System_StringComparison_Ordinal
// [4] https://docs.microsoft.com/en-us/windows/win32/api/stringapiset/nf-stringapiset-comparestringordinal
impl Ord for EnvKey {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        unsafe {
            let result = c::CompareStringOrdinal(
                self.utf16.as_ptr(),
                self.utf16.len() as _,
                other.utf16.as_ptr(),
                other.utf16.len() as _,
                c::TRUE,
            );
            match result {
                c::CSTR_LESS_THAN => cmp::Ordering::Less,
                c::CSTR_EQUAL => cmp::Ordering::Equal,
                c::CSTR_GREATER_THAN => cmp::Ordering::Greater,
                // 只要参数正确，`CompareStringOrdinal` 应当永远不会失败。
                _ => panic!("comparing environment keys failed: {}", Error::last_os_error()),
            }
        }
    }
}
impl PartialOrd for EnvKey {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl PartialEq for EnvKey {
    fn eq(&self, other: &Self) -> bool {
        if self.utf16.len() != other.utf16.len() {
            false
        } else {
            self.cmp(other) == cmp::Ordering::Equal
        }
    }
}
impl PartialOrd<str> for EnvKey {
    fn partial_cmp(&self, other: &str) -> Option<cmp::Ordering> {
        Some(self.cmp(&EnvKey::new(other)))
    }
}
impl PartialEq<str> for EnvKey {
    fn eq(&self, other: &str) -> bool {
        if self.os_string.len() != other.len() {
            false
        } else {
            self.cmp(&EnvKey::new(other)) == cmp::Ordering::Equal
        }
    }
}

// 环境变量的键应当保留其原始的大小写，即便它们是使用一种不区分大小写的字符串映射
// 来进行比较的。
impl From<OsString> for EnvKey {
    fn from(k: OsString) -> Self {
        EnvKey { utf16: k.encode_wide().collect(), os_string: k }
    }
}

impl From<EnvKey> for OsString {
    fn from(k: EnvKey) -> Self {
        k.os_string
    }
}

impl From<&OsStr> for EnvKey {
    fn from(k: &OsStr) -> Self {
        Self::from(k.to_os_string())
    }
}

impl AsRef<OsStr> for EnvKey {
    fn as_ref(&self) -> &OsStr {
        &self.os_string
    }
}

pub struct Command {
    program: OsString,
    args: Vec<Arg>,
    env: CommandEnv,
    cwd: Option<OsString>,
    flags: u32,
    show_window: Option<u16>,
    detach: bool, // not currently exposed in std::process
    stdin: Option<Stdio>,
    stdout: Option<Stdio>,
    stderr: Option<Stdio>,
    force_quotes_enabled: bool,
    startupinfo_fullscreen: bool,
    startupinfo_untrusted_source: bool,
    startupinfo_force_feedback: Option<bool>,
    inherit_handles: bool,
}

pub enum Stdio {
    Inherit,
    InheritSpecific { from_stdio_id: u32 },
    Null,
    MakePipe,
    Pipe(ChildPipe),
    Handle(Handle),
}

impl Command {
    pub fn new(program: &OsStr) -> Command {
        Command {
            program: program.to_os_string(),
            args: Vec::new(),
            env: Default::default(),
            cwd: None,
            flags: 0,
            show_window: None,
            detach: false,
            stdin: None,
            stdout: None,
            stderr: None,
            force_quotes_enabled: false,
            startupinfo_fullscreen: false,
            startupinfo_untrusted_source: false,
            startupinfo_force_feedback: None,
            inherit_handles: true,
        }
    }

    pub fn arg(&mut self, arg: &OsStr) {
        self.args.push(Arg::Regular(arg.to_os_string()))
    }
    pub fn env_mut(&mut self) -> &mut CommandEnv {
        &mut self.env
    }
    pub fn cwd(&mut self, dir: &OsStr) {
        self.cwd = Some(dir.to_os_string())
    }
    pub fn stdin(&mut self, stdin: Stdio) {
        self.stdin = Some(stdin);
    }
    pub fn stdout(&mut self, stdout: Stdio) {
        self.stdout = Some(stdout);
    }
    pub fn stderr(&mut self, stderr: Stdio) {
        self.stderr = Some(stderr);
    }
    pub fn creation_flags(&mut self, flags: u32) {
        self.flags = flags;
    }
    pub fn show_window(&mut self, cmd_show: Option<u16>) {
        self.show_window = cmd_show;
    }

    pub fn force_quotes(&mut self, enabled: bool) {
        self.force_quotes_enabled = enabled;
    }

    pub fn raw_arg(&mut self, command_str_to_append: &OsStr) {
        self.args.push(Arg::Raw(command_str_to_append.to_os_string()))
    }

    pub fn startupinfo_fullscreen(&mut self, enabled: bool) {
        self.startupinfo_fullscreen = enabled;
    }

    pub fn startupinfo_untrusted_source(&mut self, enabled: bool) {
        self.startupinfo_untrusted_source = enabled;
    }

    pub fn startupinfo_force_feedback(&mut self, enabled: Option<bool>) {
        self.startupinfo_force_feedback = enabled;
    }

    pub fn get_program(&self) -> &OsStr {
        &self.program
    }

    pub fn get_args(&self) -> CommandArgs<'_> {
        let iter = self.args.iter();
        CommandArgs { iter }
    }

    pub fn get_envs(&self) -> CommandEnvs<'_> {
        self.env.iter()
    }

    pub fn get_env_clear(&self) -> bool {
        self.env.does_clear()
    }

    pub fn get_current_dir(&self) -> Option<&Path> {
        self.cwd.as_ref().map(Path::new)
    }

    pub fn inherit_handles(&mut self, inherit_handles: bool) {
        self.inherit_handles = inherit_handles;
    }

    pub fn spawn(
        &mut self,
        default: Stdio,
        needs_stdin: bool,
    ) -> io::Result<(Process, StdioPipes)> {
        self.spawn_with_attributes(default, needs_stdin, None)
    }

    pub fn spawn_with_attributes(
        &mut self,
        default: Stdio,
        needs_stdin: bool,
        proc_thread_attribute_list: Option<&ProcThreadAttributeList<'_>>,
    ) -> io::Result<(Process, StdioPipes)> {
        let env_saw_path = self.env.have_changed_path();
        let maybe_env = self.env.capture_if_changed();

        let child_paths = if env_saw_path && let Some(env) = maybe_env.as_ref() {
            env.get(&EnvKey::new("PATH")).map(|s| s.as_os_str())
        } else {
            None
        };
        let program = resolve_exe(&self.program, || env::var_os("PATH"), child_paths)?;
        let has_bat_extension = |program: &[u16]| {
            matches!(
                // 对 UTF-16 编码的 ".bat" 或 ".cmd" 进行不区分大小写的 "ends_with"
                program.len().checked_sub(4).and_then(|i| program.get(i..)),
                Some([46, 98 | 66, 97 | 65, 116 | 84] | [46, 99 | 67, 109 | 77, 100 | 68])
            )
        };
        let is_batch_file = if path::is_verbatim(&program) {
            has_bat_extension(&program[..program.len() - 1])
        } else {
            fill_utf16_buf(
                |buffer, size| unsafe {
                    // 解析（resolve）该路径，以便我们可以测试最终的文件名。
                    c::GetFullPathNameW(program.as_ptr(), size, buffer, ptr::null_mut())
                },
                |program| has_bat_extension(program),
            )?
        };
        let (program, mut cmd_str) = if is_batch_file {
            (
                command_prompt()?,
                args::make_bat_command_line(&program, &self.args, self.force_quotes_enabled)?,
            )
        } else {
            let cmd_str = make_command_line(&self.program, &self.args, self.force_quotes_enabled)?;
            (program, cmd_str)
        };
        cmd_str.push(0); // 添加 null 终止符

        // 偷师自（stolen from）libuv 的代码。
        let mut flags = self.flags | c::CREATE_UNICODE_ENVIRONMENT;
        if self.detach {
            flags |= c::DETACHED_PROCESS | c::CREATE_NEW_PROCESS_GROUP;
        }

        let inherit_handles = self.inherit_handles as c::BOOL;
        let (envp, _data) = make_envp(maybe_env)?;
        let (dirp, _data) = make_dirp(self.cwd.as_ref())?;
        let mut pi = zeroed_process_information();

        // 准备好所有的 stdio 句柄，使其能被子进程继承。这目前涉及把任何已存在的句柄
        // 复制（duplicate）为可被子进程继承的句柄。然而要注意，一旦创建了一个可继承的
        // 句柄，*任何* 被 spawn 的子进程都会继承那个句柄。我们只希望我们自己的子进程继承
        // 这个句柄，因此我们把本次 spawn 的剩余部分包裹在一个互斥锁（mutex）里。
        //
        // 想了解更多信息，msdn 也有一篇关于这种竞态（race）的文章：
        // https://support.microsoft.com/kb/315939
        static CREATE_PROCESS_LOCK: Mutex<()> = Mutex::new(());

        let _guard = CREATE_PROCESS_LOCK.lock();

        let mut pipes = StdioPipes { stdin: None, stdout: None, stderr: None };
        let null = Stdio::Null;
        let default_stdin = if needs_stdin { &default } else { &null };
        let stdin = self.stdin.as_ref().unwrap_or(default_stdin);
        let stdout = self.stdout.as_ref().unwrap_or(&default);
        let stderr = self.stderr.as_ref().unwrap_or(&default);
        let stdin = stdin.to_handle(c::STD_INPUT_HANDLE, &mut pipes.stdin)?;
        let stdout = stdout.to_handle(c::STD_OUTPUT_HANDLE, &mut pipes.stdout)?;
        let stderr = stderr.to_handle(c::STD_ERROR_HANDLE, &mut pipes.stderr)?;

        let mut si = zeroed_startupinfo();

        // 如果 stdin、stdout 或 stderr 中至少有一个被设置了（即非 null），
        // 那么就设置 `STARTUPINFO` 中的 `hStd` 字段。
        // 否则就跳过这一步，让操作系统应用它的默认行为。
        // 这能在 Win7 与 Win8+ 之间提供更一致的行为。
        let is_set = |stdio: &Handle| !stdio.as_raw_handle().is_null();
        if is_set(&stderr) || is_set(&stdout) || is_set(&stdin) {
            si.dwFlags |= c::STARTF_USESTDHANDLES;
            si.hStdInput = stdin.as_raw_handle();
            si.hStdOutput = stdout.as_raw_handle();
            si.hStdError = stderr.as_raw_handle();
        }

        if let Some(cmd_show) = self.show_window {
            si.dwFlags |= c::STARTF_USESHOWWINDOW;
            si.wShowWindow = cmd_show;
        }

        if self.startupinfo_fullscreen {
            si.dwFlags |= c::STARTF_RUNFULLSCREEN;
        }

        if self.startupinfo_untrusted_source {
            si.dwFlags |= c::STARTF_UNTRUSTEDSOURCE;
        }

        match self.startupinfo_force_feedback {
            Some(true) => {
                si.dwFlags |= c::STARTF_FORCEONFEEDBACK;
            }
            Some(false) => {
                si.dwFlags |= c::STARTF_FORCEOFFFEEDBACK;
            }
            None => {}
        }

        let si_ptr: *mut c::STARTUPINFOW;

        let mut si_ex;

        if let Some(proc_thread_attribute_list) = proc_thread_attribute_list {
            si.cb = size_of::<c::STARTUPINFOEXW>() as u32;
            flags |= c::EXTENDED_STARTUPINFO_PRESENT;

            si_ex = c::STARTUPINFOEXW {
                StartupInfo: si,
                // SAFETY: 在这里把这个 `*const` 指针转换为 `*mut` 指针是“安全的”，
                // 因为 windows 在内部不会改变（mutate）该属性列表（attribute list）。
                // 理想情况下，这一点应当反映在 `windows-sys` crate 的接口中。
                lpAttributeList: proc_thread_attribute_list.as_ptr().cast::<c_void>().cast_mut(),
            };
            si_ptr = (&raw mut si_ex) as _;
        } else {
            si.cb = size_of::<c::STARTUPINFOW>() as u32;
            si_ptr = (&raw mut si) as _;
        }

        unsafe {
            cvt(c::CreateProcessW(
                program.as_ptr(),
                cmd_str.as_mut_ptr(),
                ptr::null_mut(),
                ptr::null_mut(),
                inherit_handles,
                flags,
                envp,
                dirp,
                si_ptr,
                &mut pi,
            ))
        }?;

        unsafe {
            Ok((
                Process {
                    handle: Handle::from_raw_handle(pi.hProcess),
                    main_thread_handle: Handle::from_raw_handle(pi.hThread),
                },
                pipes,
            ))
        }
    }
}

impl fmt::Debug for Command {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.program.fmt(f)?;
        for arg in &self.args {
            f.write_str(" ")?;
            match arg {
                Arg::Regular(s) => s.fmt(f),
                Arg::Raw(s) => f.write_str(&s.to_string_lossy()),
            }?;
        }
        Ok(())
    }
}

// 把 `exe_path` 解析为可执行文件名。
//
// * 如果该路径只是一个文件名，那么就使用 `search_paths` 给出的路径来查找该可执行文件。
// * 否则，按原样使用 `exe_path`。
//
// 本函数也可能会向名称追加 `.exe`。这样做的理由如下：
//
// Windows 可执行文件带有 `exe` 扩展名，这是一个非常强的约定。
// 在 Rust 中，省略这个扩展名很常见。
// 因此本函数首先假定用户本意是想带上 `.exe`。
// 如果给出的是完整路径但省略了扩展名，或者只给出文件名且它已经包含扩展名，
// 那么它会回退到使用纯文件名。
fn resolve_exe<'a>(
    exe_path: &'a OsStr,
    parent_paths: impl FnOnce() -> Option<OsString>,
    child_paths: Option<&OsStr>,
) -> io::Result<Vec<u16>> {
    // 如果没有文件名，则提前返回。
    if exe_path.is_empty() || path::has_trailing_slash(exe_path) {
        return Err(io::const_error!(io::ErrorKind::InvalidInput, "program path has no file name"));
    }
    // 测试文件名是否带有 `exe` 扩展名。
    // 它执行的是不区分大小写的 `ends_with`。
    let has_exe_suffix = if exe_path.len() >= EXE_SUFFIX.len() {
        exe_path.as_encoded_bytes()[exe_path.len() - EXE_SUFFIX.len()..]
            .eq_ignore_ascii_case(EXE_SUFFIX.as_bytes())
    } else {
        false
    };

    // 如果 `exe_path` 是绝对路径或子路径（sub-path），那么就不要在 `PATH` 中查找它。
    if !path::is_file_name(exe_path) {
        if has_exe_suffix {
            // 应用程序名称是一个指向 `.exe` 文件的路径。
            // 让 `CreateProcessW` 去判断它是否存在。
            return args::to_user_path(Path::new(exe_path));
        }
        let mut path = PathBuf::from(exe_path);

        // 如果还没有 `.exe`，就追加上去。
        path = path::append_suffix(path, EXE_SUFFIX.as_ref());
        if let Some(path) = program_exists(&path) {
            return Ok(path);
        } else {
            // 在这里使用 `set_extension` 是没问题的，因为意图就是
            // 移除刚刚添加上去的那个扩展名。
            path.set_extension("");
            return args::to_user_path(&path);
        }
    } else {
        ensure_no_nuls(exe_path)?;
        // 摘自 `CreateProcessW` 的文档：
        // > 如果文件名不包含扩展名，则会追加 .exe。
        // 注意，此规则仅在搜索路径时适用。
        let has_extension = exe_path.as_encoded_bytes().contains(&b'.');

        // 在 `search_paths` 给出的目录中进行搜索。
        let result = search_paths(parent_paths, child_paths, |mut path| {
            path.push(exe_path);
            if !has_extension {
                path.set_extension(EXE_EXTENSION);
            }
            program_exists(&path)
        });
        if let Some(path) = result {
            return Ok(path);
        }
    }
    // 如果我们走到了这里，那就说明找不到该可执行文件。
    Err(io::const_error!(io::ErrorKind::NotFound, "program not found"))
}

// 对每一个应当用于查找可执行文件的路径调用 `f`。
// 一旦 `f` 返回某个可执行文件的路径、或所有路径都已被搜索过，就返回。
fn search_paths<Paths, Exists>(
    parent_paths: Paths,
    child_paths: Option<&OsStr>,
    mut exists: Exists,
) -> Option<Vec<u16>>
where
    Paths: FnOnce() -> Option<OsString>,
    Exists: FnMut(PathBuf) -> Option<Vec<u16>>,
{
    // 1. 子路径（Child paths）
    // 这是为了与 Rust 历史上的行为保持一致。
    if let Some(paths) = child_paths {
        for path in env::split_paths(paths).filter(|p| !p.as_os_str().is_empty()) {
            if let Some(path) = exists(path) {
                return Some(path);
            }
        }
    }

    // 2. 应用程序路径（Application path）
    if let Ok(mut app_path) = env::current_exe() {
        app_path.pop();
        if let Some(path) = exists(app_path) {
            return Some(path);
        }
    }

    // 3 和 4. 系统路径（System paths）
    // SAFETY: 它使用 `fill_utf16_buf` 来安全地调用相应的操作系统函数。
    unsafe {
        if let Ok(Some(path)) = fill_utf16_buf(
            |buf, size| c::GetSystemDirectoryW(buf, size),
            |buf| exists(PathBuf::from(OsString::from_wide(buf))),
        ) {
            return Some(path);
        }
        #[cfg(not(target_vendor = "uwp"))]
        {
            if let Ok(Some(path)) = fill_utf16_buf(
                |buf, size| c::GetWindowsDirectoryW(buf, size),
                |buf| exists(PathBuf::from(OsString::from_wide(buf))),
            ) {
                return Some(path);
            }
        }
    }

    // 5. 父路径（Parent paths）
    if let Some(parent_paths) = parent_paths() {
        for path in env::split_paths(&parent_paths).filter(|p| !p.as_os_str().is_empty()) {
            if let Some(path) = exists(path) {
                return Some(path);
            }
        }
    }
    None
}

/// 在不跟随符号链接的情况下检查某个文件是否存在。
fn program_exists(path: &Path) -> Option<Vec<u16>> {
    unsafe {
        let path = args::to_user_path(path).ok()?;
        // 使用 `GetFileAttributesW` 获取属性不会跟随符号链接，
        // 而且只要链接存在，它几乎总会成功。
        // 对于某些特殊的系统文件（例如页面文件 pagefile）有一些例外，
        // 但那些文件不是可执行文件。
        if c::GetFileAttributesW(path.as_ptr()) == c::INVALID_FILE_ATTRIBUTES {
            None
        } else {
            Some(path)
        }
    }
}

impl Stdio {
    fn to_handle(&self, stdio_id: u32, pipe: &mut Option<ChildPipe>) -> io::Result<Handle> {
        let use_stdio_id = |stdio_id| match stdio::get_handle(stdio_id) {
            Ok(io) => unsafe {
                let io = Handle::from_raw_handle(io);
                let ret = io.duplicate(0, true, c::DUPLICATE_SAME_ACCESS);
                let _ = io.into_raw_handle(); // 不要关闭该句柄
                ret
            },
            // 如果没有可用的 stdio 句柄，则把这个 null 值传播下去。
            Err(..) => unsafe { Ok(Handle::from_raw_handle(ptr::null_mut())) },
        };
        match *self {
            Stdio::Inherit => use_stdio_id(stdio_id),
            Stdio::InheritSpecific { from_stdio_id } => use_stdio_id(from_stdio_id),

            Stdio::MakePipe => {
                let ours_readable = stdio_id != c::STD_INPUT_HANDLE;
                let pipes = child_pipe::child_pipe(ours_readable, true)?;
                *pipe = Some(pipes.ours);
                Ok(pipes.theirs.into_handle())
            }

            Stdio::Pipe(ref source) => {
                let ours_readable = stdio_id != c::STD_INPUT_HANDLE;
                child_pipe::spawn_pipe_relay(source, ours_readable, true)
                    .map(ChildPipe::into_handle)
            }

            Stdio::Handle(ref handle) => handle.duplicate(0, true, c::DUPLICATE_SAME_ACCESS),

            // 打开一个对 NUL 的引用，赋予其适当的读/写权限，以及可被子进程继承的能力
            //（因为它即将被继承）。
            Stdio::Null => {
                let mut opts = OpenOptions::new();
                opts.read(stdio_id == c::STD_INPUT_HANDLE);
                opts.write(stdio_id != c::STD_INPUT_HANDLE);
                opts.inherit_handle(true);
                File::open(Path::new(r"\\.\NUL"), &opts).map(|file| file.into_inner())
            }
        }
    }
}

impl From<ChildPipe> for Stdio {
    fn from(pipe: ChildPipe) -> Stdio {
        Stdio::Pipe(pipe)
    }
}

impl From<Handle> for Stdio {
    fn from(pipe: Handle) -> Stdio {
        Stdio::Handle(pipe)
    }
}

impl From<File> for Stdio {
    fn from(file: File) -> Stdio {
        Stdio::Handle(file.into_inner())
    }
}

impl From<io::Stdout> for Stdio {
    fn from(_: io::Stdout) -> Stdio {
        Stdio::InheritSpecific { from_stdio_id: c::STD_OUTPUT_HANDLE }
    }
}

impl From<io::Stderr> for Stdio {
    fn from(_: io::Stderr) -> Stdio {
        Stdio::InheritSpecific { from_stdio_id: c::STD_ERROR_HANDLE }
    }
}

////////////////////////////////////////////////////////////////////////////////
// 进程（Processes）
////////////////////////////////////////////////////////////////////////////////

/// 一个表示子进程的值。
///
/// 这个值的生命周期与实际进程的生命周期相关联——Process 的析构函数会调用
/// self.finish()，它会等待该进程终止。
pub struct Process {
    handle: Handle,
    main_thread_handle: Handle,
}

impl Process {
    pub fn kill(&mut self) -> io::Result<()> {
        let result = unsafe { c::TerminateProcess(self.handle.as_raw_handle(), 1) };
        if result == c::FALSE {
            let error = api::get_last_error();
            // 如果进程已经被终止（被我们终止，或出于任何其他原因），
            // TerminateProcess 会返回 ERROR_ACCESS_DENIED。因此要检查该进程是否确实
            // 已被终止；如果是，就不要返回错误。
            if error != WinError::ACCESS_DENIED || self.try_wait().is_err() {
                return Err(crate::io::Error::from_raw_os_error(error.code as i32));
            }
        }
        Ok(())
    }

    pub fn id(&self) -> u32 {
        unsafe { c::GetProcessId(self.handle.as_raw_handle()) }
    }

    pub fn main_thread_handle(&self) -> BorrowedHandle<'_> {
        self.main_thread_handle.as_handle()
    }

    pub fn wait(&mut self) -> io::Result<ExitStatus> {
        unsafe {
            let res = c::WaitForSingleObject(self.handle.as_raw_handle(), c::INFINITE);
            if res != c::WAIT_OBJECT_0 {
                return Err(Error::last_os_error());
            }
            let mut status = 0;
            cvt(c::GetExitCodeProcess(self.handle.as_raw_handle(), &mut status))?;
            Ok(ExitStatus(status))
        }
    }

    pub fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        unsafe {
            match c::WaitForSingleObject(self.handle.as_raw_handle(), 0) {
                c::WAIT_OBJECT_0 => {}
                c::WAIT_TIMEOUT => {
                    return Ok(None);
                }
                _ => return Err(io::Error::last_os_error()),
            }
            let mut status = 0;
            cvt(c::GetExitCodeProcess(self.handle.as_raw_handle(), &mut status))?;
            Ok(Some(ExitStatus(status)))
        }
    }

    pub fn handle(&self) -> &Handle {
        &self.handle
    }

    pub fn into_handle(self) -> Handle {
        self.handle
    }
}

#[derive(PartialEq, Eq, Clone, Copy, Debug, Default)]
pub struct ExitStatus(u32);

impl ExitStatus {
    pub fn exit_ok(&self) -> Result<(), ExitStatusError> {
        match NonZero::<u32>::try_from(self.0) {
            /* was nonzero */ Ok(failure) => Err(ExitStatusError(failure)),
            /* was zero, couldn't convert */ Err(_) => Ok(()),
        }
    }
    pub fn code(&self) -> Option<i32> {
        Some(self.0 as i32)
    }
}

/// 通过包装一个原始的 `u32`（不进行拷贝）来把它转换为类型安全的 `ExitStatus`。
impl From<u32> for ExitStatus {
    fn from(u: u32) -> ExitStatus {
        ExitStatus(u)
    }
}

impl fmt::Display for ExitStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // 设置了高位（high bit）的 Windows 退出码通常意味着某种未处理的异常或警告。
        // 在这种情况下，以十进制打印退出码并不总是合理，因为它是一个非常大、
        // 且有点像乱码的数字。十六进制码更易辨认、也更便于搜索，所以打印那个。
        if self.0 & 0x80000000 != 0 {
            write!(f, "exit code: {:#x}", self.0)
        } else {
            write!(f, "exit code: {}", self.0)
        }
    }
}

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub struct ExitStatusError(NonZero<u32>);

impl Into<ExitStatus> for ExitStatusError {
    fn into(self) -> ExitStatus {
        ExitStatus(self.0.into())
    }
}

impl ExitStatusError {
    pub fn code(self) -> Option<NonZero<i32>> {
        Some((u32::from(self.0) as i32).try_into().unwrap())
    }
}

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub struct ExitCode(u32);

impl ExitCode {
    pub const SUCCESS: ExitCode = ExitCode(EXIT_SUCCESS as _);
    pub const FAILURE: ExitCode = ExitCode(EXIT_FAILURE as _);

    #[inline]
    pub fn as_i32(&self) -> i32 {
        self.0 as i32
    }
}

impl From<u8> for ExitCode {
    fn from(code: u8) -> Self {
        ExitCode(u32::from(code))
    }
}

impl From<u32> for ExitCode {
    fn from(code: u32) -> Self {
        ExitCode(u32::from(code))
    }
}

fn zeroed_startupinfo() -> c::STARTUPINFOW {
    c::STARTUPINFOW {
        cb: 0,
        lpReserved: ptr::null_mut(),
        lpDesktop: ptr::null_mut(),
        lpTitle: ptr::null_mut(),
        dwX: 0,
        dwY: 0,
        dwXSize: 0,
        dwYSize: 0,
        dwXCountChars: 0,
        dwYCountChars: 0,
        dwFillAttribute: 0,
        dwFlags: 0,
        wShowWindow: 0,
        cbReserved2: 0,
        lpReserved2: ptr::null_mut(),
        hStdInput: ptr::null_mut(),
        hStdOutput: ptr::null_mut(),
        hStdError: ptr::null_mut(),
    }
}

fn zeroed_process_information() -> c::PROCESS_INFORMATION {
    c::PROCESS_INFORMATION {
        hProcess: ptr::null_mut(),
        hThread: ptr::null_mut(),
        dwProcessId: 0,
        dwThreadId: 0,
    }
}

// 生成一个宽字符串（wide string）*且不带末尾的 null*；如果 `prog` 或任何 `args`
// 包含 nul，则返回一个错误。
fn make_command_line(argv0: &OsStr, args: &[Arg], force_quotes: bool) -> io::Result<Vec<u16>> {
    // 把命令和参数编码到一个命令行字符串中，使得被 spawn 的进程
    // 可以用 CommandLineToArgvW 把它们还原（recover）出来。
    let mut cmd: Vec<u16> = Vec::new();

    // 始终给程序名加上引号，以避免子进程解析其参数时 CreateProcess 产生歧义。
    // 注意这里不对引号进行转义，因为引号不能用于 arg0。
    // 不过这没关系，因为文件路径不能包含引号。
    cmd.push(b'"' as u16);
    cmd.extend(argv0.encode_wide());
    cmd.push(b'"' as u16);

    for arg in args {
        cmd.push(' ' as u16);
        args::append_arg(&mut cmd, arg, force_quotes)?;
    }
    Ok(cmd)
}

// 获取用于运行 bat 脚本的 `cmd.exe`，编码为 UTF-16 字符串。
fn command_prompt() -> io::Result<Vec<u16>> {
    let mut system: Vec<u16> =
        fill_utf16_buf(|buf, size| unsafe { c::GetSystemDirectoryW(buf, size) }, |buf| buf.into())?;
    system.extend("\\cmd.exe".encode_utf16().chain([0]));
    Ok(system)
}

fn make_envp(maybe_env: Option<BTreeMap<EnvKey, OsString>>) -> io::Result<(*mut c_void, Vec<u16>)> {
    // 在 Windows 上，我们传入一个 "environment block"（环境块），它不是 char**，
    // 而是若干以 null 结尾的 k=v\0 序列的拼接，并以一个末尾的 \0 来终止。
    if let Some(env) = maybe_env {
        let mut blk = Vec::new();

        // 如果没有要设置的环境变量，就通过 push 一个 null 来表明这一点。
        if env.is_empty() {
            blk.push(0);
        }

        for (k, v) in env {
            ensure_no_nuls(k.os_string)?;
            blk.extend(k.utf16);
            blk.push('=' as u16);
            blk.extend(ensure_no_nuls(v)?.encode_wide());
            blk.push(0);
        }
        blk.push(0);
        Ok((blk.as_mut_ptr() as *mut c_void, blk))
    } else {
        Ok((ptr::null_mut(), Vec::new()))
    }
}

fn make_dirp(d: Option<&OsString>) -> io::Result<(*const u16, Vec<u16>)> {
    match d {
        Some(dir) => {
            let mut dir_str: Vec<u16> = ensure_no_nuls(dir)?.encode_wide().chain([0]).collect();
            // 尝试移除 `\\?\` 前缀（如果有的话）。
            // 这是必要的，因为当前目录（current directory）不支持 verbatim 路径。
            // 然而，只有在这样做不会改变路径的解析方式时，才能这么做。
            let ptr = if dir_str.starts_with(utf16!(r"\\?\UNC")) {
                // 把 `UNC` 中的 `C` 变成 `\`，这样我们就可以使用 `\\rest\of\path`。
                let start = r"\\?\UN".len();
                dir_str[start] = b'\\' as u16;
                if path::is_absolute_exact(&dir_str[start..]) {
                    dir_str[start..].as_ptr()
                } else {
                    // 撤销上面的更改。
                    dir_str[start] = b'C' as u16;
                    dir_str.as_ptr()
                }
            } else if dir_str.starts_with(utf16!(r"\\?\")) {
                // 去掉开头的 `\\?\`
                let start = r"\\?\".len();
                if path::is_absolute_exact(&dir_str[start..]) {
                    dir_str[start..].as_ptr()
                } else {
                    dir_str.as_ptr()
                }
            } else {
                dir_str.as_ptr()
            };
            Ok((ptr, dir_str))
        }
        None => Ok((ptr::null(), Vec::new())),
    }
}

pub struct CommandArgs<'a> {
    iter: crate::slice::Iter<'a, Arg>,
}

impl<'a> Iterator for CommandArgs<'a> {
    type Item = &'a OsStr;
    fn next(&mut self) -> Option<&'a OsStr> {
        self.iter.next().map(|arg| match arg {
            Arg::Regular(s) | Arg::Raw(s) => s.as_ref(),
        })
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

impl<'a> ExactSizeIterator for CommandArgs<'a> {
    fn len(&self) -> usize {
        self.iter.len()
    }
    fn is_empty(&self) -> bool {
        self.iter.is_empty()
    }
}

impl<'a> fmt::Debug for CommandArgs<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter.clone()).finish()
    }
}
