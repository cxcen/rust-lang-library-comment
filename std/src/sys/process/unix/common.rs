#[cfg(all(test, not(target_os = "emscripten")))]
mod tests;

use libc::{EXIT_FAILURE, EXIT_SUCCESS, c_int, gid_t, pid_t, uid_t};

pub use self::cstring_array::CStringArray;
use self::cstring_array::CStringIter;
use crate::collections::BTreeMap;
use crate::ffi::{CStr, CString, OsStr, OsString};
use crate::os::unix::prelude::*;
use crate::path::Path;
use crate::process::StdioPipes;
use crate::sys::fd::FileDesc;
use crate::sys::fs::File;
#[cfg(not(target_os = "fuchsia"))]
use crate::sys::fs::OpenOptions;
use crate::sys::pipe::pipe;
use crate::sys::process::env::{CommandEnv, CommandEnvs};
use crate::sys::{FromInner, IntoInner, cvt_r};
use crate::{fmt, io, mem};

mod cstring_array;

cfg_select! {
    target_os = "fuchsia" => {
        // fuchsia 没有 /dev/null
    }
    target_os = "vxworks" => {
        const DEV_NULL: &CStr = c"/null";
    }
    _ => {
        const DEV_NULL: &CStr = c"/dev/null";
    }
}

// api 版本低于 21 的 Android 把 sig* 函数定义为内联（inline），因此它无法用于动态链接。
// 实现 sigemptyset 和 sigaddset 使我们能够支持较旧的 Android 版本（与 libc 版本无关）。
// 以下实现基于
// https://github.com/aosp-mirror/platform_bionic/blob/ad8dcd6023294b646e5a8288c0ed431b0845da49/libc/include/android/legacy_signal_inlines.h
cfg_select! {
    target_os = "android" => {
        #[allow(dead_code)]
        pub unsafe fn sigemptyset(set: *mut libc::sigset_t) -> libc::c_int {
            set.write_bytes(0u8, 1);
            return 0;
        }

        #[allow(dead_code)]
        pub unsafe fn sigaddset(set: *mut libc::sigset_t, signum: libc::c_int) -> libc::c_int {
            use crate::slice;
            use libc::{c_ulong, sigset_t};

            // 来自 bionic（android libc）的实现把 `sigset_t` 当作一个 `c_ulong` 数组
            // 来进行类型双关（type pun）。这样可行，但我们加一个 smoke check
            // 来确保这一点没有改变。
            const _: () = assert!(
                align_of::<c_ulong>() == align_of::<sigset_t>()
                    && (size_of::<sigset_t>() % size_of::<c_ulong>()) == 0
            );

            let bit = (signum - 1) as usize;
            if set.is_null() || bit >= (8 * size_of::<sigset_t>()) {
                crate::sys::io::set_errno(libc::EINVAL);
                return -1;
            }
            let raw = slice::from_raw_parts_mut(
                set as *mut c_ulong,
                size_of::<sigset_t>() / size_of::<c_ulong>(),
            );
            const LONG_BIT: usize = size_of::<c_ulong>() * 8;
            raw[bit / LONG_BIT] |= 1 << (bit % LONG_BIT);
            return 0;
        }
    }
    _ => {
        #[allow(unused_imports)]
        pub use libc::{sigemptyset, sigaddset};
    }
}

////////////////////////////////////////////////////////////////////////////////
// 命令（Command）
////////////////////////////////////////////////////////////////////////////////

pub struct Command {
    program: CString,
    args: CStringArray,
    env: CommandEnv,

    program_kind: ProgramKind,
    cwd: Option<CString>,
    chroot: Option<CString>,
    uid: Option<uid_t>,
    gid: Option<gid_t>,
    saw_nul: bool,
    closures: Vec<Box<dyn FnMut() -> io::Result<()> + Send + Sync>>,
    groups: Option<Box<[gid_t]>>,
    stdin: Option<Stdio>,
    stdout: Option<Stdio>,
    stderr: Option<Stdio>,
    #[cfg(target_os = "linux")]
    create_pidfd: bool,
    pgroup: Option<pid_t>,
    setsid: bool,
}

// 连同关于子进程 stdio 应当是什么样子的配置，一起传给 do_exec()
#[cfg_attr(target_os = "vita", allow(dead_code))]
pub struct ChildPipes {
    pub stdin: ChildStdio,
    pub stdout: ChildStdio,
    pub stderr: ChildStdio,
}

pub enum ChildStdio {
    Inherit,
    Explicit(c_int),
    Owned(FileDesc),

    // 在 Fuchsia 上，null stdio 是默认值，因此我们在 spawn 时干脆不指定任何 action。
    #[cfg(target_os = "fuchsia")]
    Null,
}

#[derive(Debug)]
pub enum Stdio {
    Inherit,
    Null,
    MakePipe,
    Fd(FileDesc),
    StaticFd(BorrowedFd<'static>),
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ProgramKind {
    /// 一个会在 PATH 上查找的程序（例如 `ls`）
    PathLookup,
    /// 一个相对路径（例如 `my-dir/foo`、`../foo`、`./foo`）
    Relative,
    /// 一个绝对路径。
    Absolute,
}

impl ProgramKind {
    fn new(program: &OsStr) -> Self {
        if program.as_encoded_bytes().starts_with(b"/") {
            Self::Absolute
        } else if program.as_encoded_bytes().contains(&b'/') {
            // 如果程序名中含有不止一个组成部分（component），那它就是一个相对路径。
            Self::Relative
        } else {
            Self::PathLookup
        }
    }
}

impl Command {
    pub fn new(program: &OsStr) -> Command {
        let mut saw_nul = false;
        let program_kind = ProgramKind::new(program.as_ref());
        let program = os2c(program, &mut saw_nul);
        let mut args = CStringArray::with_capacity(1);
        args.push(program.clone());
        Command {
            program,
            args,
            env: Default::default(),
            program_kind,
            cwd: None,
            chroot: None,
            uid: None,
            gid: None,
            saw_nul,
            closures: Vec::new(),
            groups: None,
            stdin: None,
            stdout: None,
            stderr: None,
            #[cfg(target_os = "linux")]
            create_pidfd: false,
            pgroup: None,
            setsid: false,
        }
    }

    pub fn set_arg_0(&mut self, arg: &OsStr) {
        // 设置一个新的 arg0
        let arg = os2c(arg, &mut self.saw_nul);
        self.args.write(0, arg);
    }

    pub fn arg(&mut self, arg: &OsStr) {
        let arg = os2c(arg, &mut self.saw_nul);
        self.args.push(arg);
    }

    pub fn cwd(&mut self, dir: &OsStr) {
        self.cwd = Some(os2c(dir, &mut self.saw_nul));
    }
    pub fn uid(&mut self, id: uid_t) {
        self.uid = Some(id);
    }
    pub fn gid(&mut self, id: gid_t) {
        self.gid = Some(id);
    }
    pub fn groups(&mut self, groups: &[gid_t]) {
        self.groups = Some(Box::from(groups));
    }
    pub fn pgroup(&mut self, pgroup: pid_t) {
        self.pgroup = Some(pgroup);
    }
    pub fn chroot(&mut self, dir: &Path) {
        self.chroot = Some(os2c(dir.as_os_str(), &mut self.saw_nul));
        if self.cwd.is_none() {
            self.cwd(&OsStr::new("/"));
        }
    }
    pub fn setsid(&mut self, setsid: bool) {
        self.setsid = setsid;
    }

    #[cfg(target_os = "linux")]
    pub fn create_pidfd(&mut self, val: bool) {
        self.create_pidfd = val;
    }

    #[cfg(not(target_os = "linux"))]
    #[allow(dead_code)]
    pub fn get_create_pidfd(&self) -> bool {
        false
    }

    #[cfg(target_os = "linux")]
    pub fn get_create_pidfd(&self) -> bool {
        self.create_pidfd
    }

    pub fn saw_nul(&self) -> bool {
        self.saw_nul
    }

    pub fn get_program(&self) -> &OsStr {
        OsStr::from_bytes(self.program.as_bytes())
    }

    #[allow(dead_code)]
    pub fn get_program_kind(&self) -> ProgramKind {
        self.program_kind
    }

    pub fn get_args(&self) -> CommandArgs<'_> {
        let mut iter = self.args.iter();
        // argv[0] 包含程序名，但我们只对参数感兴趣，所以跳过它。
        iter.next();
        CommandArgs { iter }
    }

    pub fn get_envs(&self) -> CommandEnvs<'_> {
        self.env.iter()
    }

    pub fn get_env_clear(&self) -> bool {
        self.env.does_clear()
    }

    pub fn get_current_dir(&self) -> Option<&Path> {
        self.cwd.as_ref().map(|cs| Path::new(OsStr::from_bytes(cs.as_bytes())))
    }

    pub fn get_argv(&self) -> &CStringArray {
        &self.args
    }

    pub fn get_program_cstr(&self) -> &CStr {
        &self.program
    }

    #[allow(dead_code)]
    pub fn get_cwd(&self) -> Option<&CStr> {
        self.cwd.as_deref()
    }
    #[allow(dead_code)]
    pub fn get_uid(&self) -> Option<uid_t> {
        self.uid
    }
    #[allow(dead_code)]
    pub fn get_gid(&self) -> Option<gid_t> {
        self.gid
    }
    #[allow(dead_code)]
    pub fn get_groups(&self) -> Option<&[gid_t]> {
        self.groups.as_deref()
    }
    #[allow(dead_code)]
    pub fn get_pgroup(&self) -> Option<pid_t> {
        self.pgroup
    }
    #[allow(dead_code)]
    pub fn get_chroot(&self) -> Option<&CStr> {
        self.chroot.as_deref()
    }
    #[allow(dead_code)]
    pub fn get_setsid(&self) -> bool {
        self.setsid
    }

    pub fn get_closures(&mut self) -> &mut Vec<Box<dyn FnMut() -> io::Result<()> + Send + Sync>> {
        &mut self.closures
    }

    pub unsafe fn pre_exec(&mut self, f: Box<dyn FnMut() -> io::Result<()> + Send + Sync>) {
        self.closures.push(f);
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

    pub fn env_mut(&mut self) -> &mut CommandEnv {
        &mut self.env
    }

    pub fn capture_env(&mut self) -> Option<CStringArray> {
        let maybe_env = self.env.capture_if_changed();
        maybe_env.map(|env| construct_envp(env, &mut self.saw_nul))
    }

    #[allow(dead_code)]
    pub fn env_saw_path(&self) -> bool {
        self.env.have_changed_path()
    }

    #[allow(dead_code)]
    pub fn program_is_path(&self) -> bool {
        self.program.to_bytes().contains(&b'/')
    }

    pub fn setup_io(
        &self,
        default: Stdio,
        needs_stdin: bool,
    ) -> io::Result<(StdioPipes, ChildPipes)> {
        let null = Stdio::Null;
        let default_stdin = if needs_stdin { &default } else { &null };
        let stdin = self.stdin.as_ref().unwrap_or(default_stdin);
        let stdout = self.stdout.as_ref().unwrap_or(&default);
        let stderr = self.stderr.as_ref().unwrap_or(&default);
        let (their_stdin, our_stdin) = stdin.to_child_stdio(true)?;
        let (their_stdout, our_stdout) = stdout.to_child_stdio(false)?;
        let (their_stderr, our_stderr) = stderr.to_child_stdio(false)?;
        let ours = StdioPipes { stdin: our_stdin, stdout: our_stdout, stderr: our_stderr };
        let theirs = ChildPipes { stdin: their_stdin, stdout: their_stdout, stderr: their_stderr };
        Ok((ours, theirs))
    }
}

fn os2c(s: &OsStr, saw_nul: &mut bool) -> CString {
    CString::new(s.as_bytes()).unwrap_or_else(|_e| {
        *saw_nul = true;
        c"<string-with-nul>".to_owned()
    })
}

fn construct_envp(env: BTreeMap<OsString, OsString>, saw_nul: &mut bool) -> CStringArray {
    let mut result = CStringArray::with_capacity(env.len());
    for (mut k, v) in env {
        // 为 '=' 和 null 终止符预留额外空间
        k.reserve_exact(v.len() + 2);
        k.push("=");
        k.push(&v);

        // 把新条目添加进数组
        if let Ok(item) = CString::new(k.into_vec()) {
            result.push(item);
        } else {
            *saw_nul = true;
        }
    }

    result
}

impl Stdio {
    pub fn to_child_stdio(&self, readable: bool) -> io::Result<(ChildStdio, Option<ChildPipe>)> {
        match *self {
            Stdio::Inherit => Ok((ChildStdio::Inherit, None)),

            // 确保这些源描述符不是 stdio 描述符，否则我们设置子进程描述符的顺序
            // 可能会冲掉一个我们正打算保存的描述符。举例来说，假设我们想让子进程的
            // stderr 成为父进程的 stdout，让子进程的 stdout 成为父进程的 stderr。
            // 无论我们先 dup 哪一个，第二个都会被过早地覆盖掉。
            Stdio::Fd(ref fd) => {
                if fd.as_raw_fd() >= 0 && fd.as_raw_fd() <= libc::STDERR_FILENO {
                    Ok((ChildStdio::Owned(fd.duplicate()?), None))
                } else {
                    Ok((ChildStdio::Explicit(fd.as_raw_fd()), None))
                }
            }

            Stdio::StaticFd(fd) => {
                let fd = FileDesc::from_inner(fd.try_clone_to_owned()?);
                Ok((ChildStdio::Owned(fd), None))
            }

            Stdio::MakePipe => {
                let (reader, writer) = pipe()?;
                let (ours, theirs) = if readable { (writer, reader) } else { (reader, writer) };
                Ok((ChildStdio::Owned(theirs), Some(ours)))
            }

            #[cfg(not(target_os = "fuchsia"))]
            Stdio::Null => {
                let mut opts = OpenOptions::new();
                opts.read(readable);
                opts.write(!readable);
                let fd = File::open_c(DEV_NULL, &opts)?;
                Ok((ChildStdio::Owned(fd.into_inner()), None))
            }

            #[cfg(target_os = "fuchsia")]
            Stdio::Null => Ok((ChildStdio::Null, None)),
        }
    }
}

impl From<FileDesc> for Stdio {
    fn from(fd: FileDesc) -> Stdio {
        Stdio::Fd(fd)
    }
}

impl From<File> for Stdio {
    fn from(file: File) -> Stdio {
        Stdio::Fd(file.into_inner())
    }
}

impl From<io::Stdout> for Stdio {
    fn from(_: io::Stdout) -> Stdio {
        // 这里其实本应是 Stdio::StaticFd(input_argument.as_fd())。
        // 但 AsFd::as_fd 是按引用接收其参数的，并产出一个有界的（bounded）生命周期，
        // 所以在这里没用。也不存在 AsStaticFd。
        //
        // 此外，AsFd 只为 *locked*（已加锁）版本实现。
        // 我们不想在这里给它们加锁。（不加锁的影响与 process::Stdio::inherit() 的相同。）
        //
        // 可以说，假想中的 AsStaticFd 和 AsFd<'static> 本应为 io::Stdout 实现，
        // 而不仅仅是为 StdoutLocked 实现。
        Stdio::StaticFd(unsafe { BorrowedFd::borrow_raw(libc::STDOUT_FILENO) })
    }
}

impl From<io::Stderr> for Stdio {
    fn from(_: io::Stderr) -> Stdio {
        Stdio::StaticFd(unsafe { BorrowedFd::borrow_raw(libc::STDERR_FILENO) })
    }
}

impl ChildStdio {
    pub fn fd(&self) -> Option<c_int> {
        match *self {
            ChildStdio::Inherit => None,
            ChildStdio::Explicit(fd) => Some(fd),
            ChildStdio::Owned(ref fd) => Some(fd.as_raw_fd()),

            #[cfg(target_os = "fuchsia")]
            ChildStdio::Null => None,
        }
    }
}

impl fmt::Debug for Command {
    // 显示除 `self.closures`（它未实现 `Debug`）和 `self.argv`（它对调试没什么用）
    // 之外的所有属性
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if f.alternate() {
            let mut debug_command = f.debug_struct("Command");
            debug_command.field("program", &self.program).field("args", &self.args);
            if !self.env.is_unchanged() {
                debug_command.field("env", &self.env);
            }

            if self.cwd.is_some() {
                debug_command.field("cwd", &self.cwd);
            }
            if self.uid.is_some() {
                debug_command.field("uid", &self.uid);
            }
            if self.gid.is_some() {
                debug_command.field("gid", &self.gid);
            }

            if self.groups.is_some() {
                debug_command.field("groups", &self.groups);
            }

            if self.stdin.is_some() {
                debug_command.field("stdin", &self.stdin);
            }
            if self.stdout.is_some() {
                debug_command.field("stdout", &self.stdout);
            }
            if self.stderr.is_some() {
                debug_command.field("stderr", &self.stderr);
            }
            if self.pgroup.is_some() {
                debug_command.field("pgroup", &self.pgroup);
            }

            #[cfg(target_os = "linux")]
            {
                debug_command.field("create_pidfd", &self.create_pidfd);
            }

            debug_command.finish()
        } else {
            if let Some(ref cwd) = self.cwd {
                write!(f, "cd {cwd:?} && ")?;
            }
            if self.env.does_clear() {
                write!(f, "env -i ")?;
                // 被改动过的环境变量将紧接着打印出来，这应当完全如预期那样工作。
            } else {
                // 被移除的环境变量需要把命令用 `env` 包裹起来。
                let mut any_removed = false;
                for (key, value_opt) in self.get_envs() {
                    if value_opt.is_none() {
                        if !any_removed {
                            write!(f, "env ")?;
                            any_removed = true;
                        }
                        write!(f, "-u {} ", key.to_string_lossy())?;
                    }
                }
            }
            // 被改动过的环境变量可以直接添加到程序名前面。
            for (key, value_opt) in self.get_envs() {
                if let Some(value) = value_opt {
                    write!(f, "{}={value:?} ", key.to_string_lossy())?;
                }
            }

            if *self.program != self.args[0] {
                write!(f, "[{:?}] ", self.program)?;
            }
            write!(f, "{:?}", &self.args[0])?;

            for arg in self.get_args() {
                write!(f, " {:?}", arg)?;
            }

            Ok(())
        }
    }
}

#[derive(PartialEq, Eq, Clone, Copy)]
pub struct ExitCode(u8);

impl fmt::Debug for ExitCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("unix_exit_status").field(&self.0).finish()
    }
}

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
        Self(code)
    }
}

pub struct CommandArgs<'a> {
    iter: CStringIter<'a>,
}

impl<'a> Iterator for CommandArgs<'a> {
    type Item = &'a OsStr;

    fn next(&mut self) -> Option<&'a OsStr> {
        self.iter.next().map(|cs| OsStr::from_bytes(cs.to_bytes()))
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

pub type ChildPipe = crate::sys::pipe::Pipe;

pub fn read_output(
    out: ChildPipe,
    stdout: &mut Vec<u8>,
    err: ChildPipe,
    stderr: &mut Vec<u8>,
) -> io::Result<()> {
    // 把两个管道都设为非阻塞（nonblocking）模式，因为我们将在下面的 `select` 循环中
    // 从两者读取，而我们不希望其中一个阻塞住另一个！
    out.set_nonblocking(true)?;
    err.set_nonblocking(true)?;

    let mut fds: [libc::pollfd; 2] = unsafe { mem::zeroed() };
    fds[0].fd = out.as_raw_fd();
    fds[0].events = libc::POLLIN;
    fds[1].fd = err.as_raw_fd();
    fds[1].events = libc::POLLIN;
    loop {
        // 使用 `poll` 等待任一管道变得可读
        cvt_r(|| unsafe { libc::poll(fds.as_mut_ptr(), 2, -1) })?;

        if fds[0].revents != 0 && read(&out, stdout)? {
            err.set_nonblocking(false)?;
            return err.read_to_end(stderr).map(drop);
        }
        if fds[1].revents != 0 && read(&err, stderr)? {
            out.set_nonblocking(false)?;
            return out.read_to_end(stdout).map(drop);
        }
    }

    // 从每个管道尽可能多地读取，忽略 EWOULDBLOCK 或 EAGAIN。如果我们碰到 EOF，
    // 那是因为底层 reader 会返回 Ok(0)，在这种情况下我们自己会看到 `Ok`。
    // 此时我们把另一个 fd 切回阻塞模式，并读取该文件描述符上剩余的全部内容。
    fn read(fd: &FileDesc, dst: &mut Vec<u8>) -> Result<bool, io::Error> {
        match fd.read_to_end(dst) {
            Ok(_) => Ok(true),
            Err(e) => {
                if e.raw_os_error() == Some(libc::EWOULDBLOCK)
                    || e.raw_os_error() == Some(libc::EAGAIN)
                {
                    Ok(false)
                } else {
                    Err(e)
                }
            }
        }
    }
}
