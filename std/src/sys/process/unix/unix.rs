#[cfg(target_os = "vxworks")]
use libc::RTP_ID as pid_t;
#[cfg(not(target_os = "vxworks"))]
use libc::{c_int, pid_t};
#[cfg(not(any(
    target_os = "vxworks",
    target_os = "l4re",
    target_os = "tvos",
    target_os = "watchos",
)))]
use libc::{gid_t, uid_t};

use super::common::*;
use crate::io::{self, Error, ErrorKind};
use crate::num::NonZero;
use crate::process::StdioPipes;
use crate::sys::cvt;
#[cfg(target_os = "linux")]
use crate::sys::pal::linux::pidfd::PidFd;
use crate::{fmt, mem, sys};

cfg_select! {
    target_os = "nto" => {
        use crate::thread;
        use libc::{c_char, posix_spawn_file_actions_t, posix_spawnattr_t};
        use crate::time::Duration;
        use crate::sync::LazyLock;
        // 获取我们能够睡眠的最小时间量。
        // 如果无法确定，则返回一个通用值。
        fn get_clock_resolution() -> Duration {
            static MIN_DELAY: LazyLock<Duration, fn() -> Duration> = LazyLock::new(|| {
                let mut mindelay = libc::timespec { tv_sec: 0, tv_nsec: 0 };
                if unsafe { libc::clock_getres(libc::CLOCK_MONOTONIC, &mut mindelay) } == 0
                {
                    Duration::from_nanos(mindelay.tv_nsec as u64)
                } else {
                    Duration::from_millis(1)
                }
            });
            *MIN_DELAY
        }
        // 重试 fork/spawn 时使用的任意（arbitrary）最小睡眠时长
        const MIN_FORKSPAWN_SLEEP: Duration = Duration::from_nanos(1);
        // 在放弃并返回错误之前睡眠的最大时长
        const MAX_FORKSPAWN_SLEEP: Duration = Duration::from_millis(1000);
    }
    _ => {}
}

////////////////////////////////////////////////////////////////////////////////
// 命令（Command）
////////////////////////////////////////////////////////////////////////////////

impl Command {
    pub fn spawn(
        &mut self,
        default: Stdio,
        needs_stdin: bool,
    ) -> io::Result<(Process, StdioPipes)> {
        const CLOEXEC_MSG_FOOTER: [u8; 4] = *b"NOEX";

        let envp = self.capture_env();

        if self.saw_nul() {
            return Err(io::const_error!(
                ErrorKind::InvalidInput,
                "nul byte found in provided data",
            ));
        }

        let (ours, theirs) = self.setup_io(default, needs_stdin)?;

        if let Some(ret) = self.posix_spawn(&theirs, envp.as_ref())? {
            return Ok((ret, ours));
        }

        #[cfg(target_os = "linux")]
        let (input, output) = sys::net::Socket::new_pair(libc::AF_UNIX, libc::SOCK_SEQPACKET)?;

        #[cfg(not(target_os = "linux"))]
        let (input, output) = sys::pipe::pipe()?;

        // fork 之后无论发生什么，几乎肯定都会以某种方式触碰或查看环境
        //（在 `execvp` 中查看 PATH，或我们自己访问 `environ` 指针）。
        // 因此要确保在我们执行 fork 本身时没有其他线程正在访问环境。
        //
        // 注意，一旦我们完成了 fork，就不再需要持有锁了，因为父进程不会再做任何事情，
        // 而子进程身处它自己的进程之中。因此父进程会立即丢弃（drop）这个锁守卫（lock guard）。
        // 子进程则调用 `mem::forget` 来泄漏（leak）这个锁，这一点至关重要，
        // 因为释放锁不是 async-signal-safe（异步信号安全）的。
        let env_lock = sys::env::env_read_lock();
        let pid = unsafe { self.do_fork()? };

        if pid == 0 {
            crate::panic::always_abort();
            mem::forget(env_lock); // 避免非 async-signal-safe 的解锁操作
            drop(input);
            #[cfg(target_os = "linux")]
            if self.get_create_pidfd() {
                self.send_pidfd(&output);
            }
            let Err(err) = unsafe { self.do_exec(theirs, envp.as_ref()) };
            let errno = err.raw_os_error().unwrap_or(libc::EINVAL) as u32;
            let errno = errno.to_be_bytes();
            let bytes = [
                errno[0],
                errno[1],
                errno[2],
                errno[3],
                CLOEXEC_MSG_FOOTER[0],
                CLOEXEC_MSG_FOOTER[1],
                CLOEXEC_MSG_FOOTER[2],
                CLOEXEC_MSG_FOOTER[3],
            ];
            // 不超过 PIPE_BUF 字节的管道 I/O 应当是原子的，而且
            // 我们想确保自己 *不* 运行 at_exit 析构函数（destructors），
            // 因为无论如何我们都正在被销毁（torn down）
            rtassert!(output.write(&bytes).is_ok());
            unsafe { libc::_exit(1) }
        }

        drop(env_lock);
        drop(output);

        #[cfg(target_os = "linux")]
        let pidfd = if self.get_create_pidfd() { self.recv_pidfd(&input) } else { -1 };

        #[cfg(not(target_os = "linux"))]
        let pidfd = -1;

        // Safety: 我们（在 Linux 上）是使用 SOCK_SEQPACKET 获取该 pidfd 的，所以它是有效的。
        let mut p = unsafe { Process::new(pid, pidfd) };
        let mut bytes = [0; 8];

        // 循环以处理 EINTR
        loop {
            match input.read(&mut bytes) {
                Ok(0) => return Ok((p, ours)),
                Ok(8) => {
                    let (errno, footer) = bytes.split_at(4);
                    assert_eq!(
                        CLOEXEC_MSG_FOOTER, footer,
                        "Validation on the CLOEXEC pipe failed: {:?}",
                        bytes
                    );
                    let errno = i32::from_be_bytes(errno.try_into().unwrap());
                    assert!(p.wait().is_ok(), "wait() should either return Ok or panic");
                    return Err(Error::from_raw_os_error(errno));
                }
                Err(ref e) if e.is_interrupted() => {}
                Err(e) => {
                    assert!(p.wait().is_ok(), "wait() should either return Ok or panic");
                    panic!("the CLOEXEC pipe failed: {e:?}")
                }
                Ok(..) => {
                    // 不超过 PIPE_BUF 字节的管道 I/O 应当是原子的
                    // 类似地，SOCK_SEQPACKET 消息也应当整条地到达
                    assert!(p.wait().is_ok(), "wait() should either return Ok or panic");
                    panic!("short read on the CLOEXEC pipe")
                }
            }
        }
    }

    // WatchOS 和 TVOS 的头文件用 `__WATCHOS_PROHIBITED __TVOS_PROHIBITED`
    // 标记了 `fork`/`exec*` 函数，并指出应改用 `posix_spawn*` 函数。
    // 这里的 `PROHIBITED` 究竟是什么意思尚不完全清楚（例如，是否允许这些函数的调用
    // 存在于死代码（dead code）中），但听起来不妙，所以我们竭力避免一切相关用法。
    #[cfg(any(target_os = "tvos", target_os = "watchos"))]
    const ERR_APPLE_TV_WATCH_NO_FORK_EXEC: Error = io::const_error!(
        ErrorKind::Unsupported,
        "`fork`+`exec`-based process spawning is not supported on this target",
    );

    #[cfg(any(target_os = "tvos", target_os = "watchos"))]
    unsafe fn do_fork(&mut self) -> Result<pid_t, io::Error> {
        return Err(Self::ERR_APPLE_TV_WATCH_NO_FORK_EXEC);
    }

    // 尝试 fork 该进程。如果成功，在子进程中返回 Ok((0, -1))，
    // 在父进程中返回 Ok((child_pid, -1))。
    #[cfg(not(any(target_os = "watchos", target_os = "tvos", target_os = "nto")))]
    unsafe fn do_fork(&mut self) -> Result<pid_t, io::Error> {
        cvt(libc::fork())
    }

    // 在 QNX Neutrino 上，fork 可能以 EBADF 失败，原因是“在 fork() 进行期间，
    // 另一个线程可能打开或关闭了某个文件描述符”。
    // 文档说“……或者尝试再次调用 fork()”。这正是我们在这里所做的。
    // 另见 https://www.qnx.com/developers/docs/7.1/#com.qnx.doc.neutrino.lib_ref/topic/f/fork.html
    #[cfg(target_os = "nto")]
    unsafe fn do_fork(&mut self) -> Result<pid_t, io::Error> {
        use crate::sys::io::errno;

        let mut delay = MIN_FORKSPAWN_SLEEP;

        loop {
            let r = libc::fork();
            if r == -1 as libc::pid_t && errno() as libc::c_int == libc::EBADF {
                if delay < get_clock_resolution() {
                    // 我们无法睡眠这么短的时间（实际睡眠会更长）。
                    // 改为让出（Yield）。
                    thread::yield_now();
                } else if delay < MAX_FORKSPAWN_SLEEP {
                    thread::sleep(delay);
                } else {
                    return Err(io::const_error!(
                        ErrorKind::WouldBlock,
                        "forking returned EBADF too often",
                    ));
                }
                delay *= 2;
                continue;
            } else {
                return cvt(r);
            }
        }
    }

    pub fn exec(&mut self, default: Stdio) -> io::Error {
        let envp = self.capture_env();

        if self.saw_nul() {
            return io::const_error!(ErrorKind::InvalidInput, "nul byte found in provided data");
        }

        match self.setup_io(default, true) {
            Ok((_, theirs)) => {
                unsafe {
                    // 和 fork 时类似，我们想确保对环境的访问是同步的，
                    // 因此在尝试 exec 之前务必先拿到环境锁（environment lock）。
                    let _lock = sys::env::env_read_lock();

                    let Err(e) = self.do_exec(theirs, envp.as_ref());
                    e
                }
            }
            Err(e) => e,
        }
    }

    // 而此刻，我们已经到达了子进程生命中的一个特殊时刻。现在必须认为子进程已被严重削弱
    // （hamstrung），除了系统调用（syscalls）之外几乎做不了任何事情。考虑如下场景：
    //
    //      1. 进程 1 的线程 A 抓住了 malloc() 互斥锁
    //      2. 进程 1 的线程 B 调用 forks()，创建出线程 C
    //      3. 进程 2 的线程 C 接着尝试 malloc()
    //      4. 进程 2 的内存与进程 1 的内存相同，所以那个互斥锁是被锁住的。
    //
    // 这种情形看起来很像死锁（deadlock），对吧？事实证明，这正是 pthread_atfork()
    // 所要处理的问题，而它大概是在各个平台上都有实现的。线程在 fork *之前* 要做的
    // 第一件事，就是去做诸如抓住 malloc 互斥锁之类的事情，然后在 fork 之后再把它解锁。
    //
    // 尽管有这些信息，libnative 的 spawn 仍被观察到在 macOS 和 FreeBSD 上都发生过死锁。
    // 我不完全确定原因，但收集到的所有回溯（backtraces）都指向子 spawn 进程中的
    // malloc/free 活动。
    //
    // 出于这个原因，下面这段代码应当包含 0 次对 malloc 或 free（及其相关伙伴）的调用。
    //
    // 作为一个没有 malloc/free 活动的例子：我们不会通过丢弃 FileDesc（它含有一处分配）
    // 来关闭这个文件描述符。相反，我们只是手动地关闭它。反正这里永远不会有 drop glue，
    // 因为这段代码永远不会返回（子进程要么 exec()，要么调用 libc::exit）
    #[cfg(not(any(target_os = "tvos", target_os = "watchos")))]
    unsafe fn do_exec(
        &mut self,
        stdio: ChildPipes,
        maybe_envp: Option<&CStringArray>,
    ) -> Result<!, io::Error> {
        use crate::sys::{self, cvt_r};

        if let Some(fd) = stdio.stdin.fd() {
            cvt_r(|| libc::dup2(fd, libc::STDIN_FILENO))?;
        }
        if let Some(fd) = stdio.stdout.fd() {
            cvt_r(|| libc::dup2(fd, libc::STDOUT_FILENO))?;
        }
        if let Some(fd) = stdio.stderr.fd() {
            cvt_r(|| libc::dup2(fd, libc::STDERR_FILENO))?;
        }

        #[cfg(not(target_os = "l4re"))]
        {
            if let Some(_g) = self.get_groups() {
                //FIXME: Redox kernel 目前还不支持 setgroups
                #[cfg(not(target_os = "redox"))]
                cvt(libc::setgroups(_g.len().try_into().unwrap(), _g.as_ptr()))?;
            }
            if let Some(u) = self.get_gid() {
                cvt(libc::setgid(u as gid_t))?;
            }
            if let Some(u) = self.get_uid() {
                // 当从 root 降权（drop privileges）时，`setgroups` 调用会移除任何
                // 多余的（extraneous）用户组。只有当我们拥有 CAP_SETGID 且没有被显式
                // 指定一组用户组时，我们才会丢弃这些用户组。如果我们不调用它，那么即便
                // 我们的 uid 已经降权，我们可能仍然拥有某些足以让我们做超级用户（super-user）
                // 事情的用户组。
                //FIXME: Redox kernel 目前还不支持 setgroups
                #[cfg(not(target_os = "redox"))]
                if self.get_groups().is_none() {
                    let res = cvt(libc::setgroups(0, crate::ptr::null()));
                    if let Err(e) = res {
                        // 这里我们忽略没有 CAP_SETGID 的情况。
                        // 一种替代方案是：要求在设置 UID 时除了 CAP_SETUID 之外
                        // 还必须拥有 CAP_SETGID。
                        if e.raw_os_error() != Some(libc::EPERM) {
                            return Err(e.into());
                        }
                    }
                }
                cvt(libc::setuid(u as uid_t))?;
            }
        }
        if let Some(chroot) = self.get_chroot() {
            #[cfg(not(target_os = "fuchsia"))]
            cvt(libc::chroot(chroot.as_ptr()))?;
            #[cfg(target_os = "fuchsia")]
            return Err(io::const_error!(
                io::ErrorKind::Unsupported,
                "chroot not supported by fuchsia"
            ));
        }
        if let Some(cwd) = self.get_cwd() {
            cvt(libc::chdir(cwd.as_ptr()))?;
        }

        if let Some(pgroup) = self.get_pgroup() {
            cvt(libc::setpgid(0, pgroup))?;
        }

        if self.get_setsid() {
            cvt(libc::setsid())?;
        }

        // emscripten 没有信号支持。
        #[cfg(not(target_os = "emscripten"))]
        {
            // 继承父进程的信号掩码（signal mask），而不是重置它（即不调用 pthread_sigmask）。

            // 如果使用了 -Zon-broken-pipe，则不要把 SIGPIPE 重置为 SIG_DFL。
            // 如果未使用 -Zon-broken-pipe，则为了向后兼容，把 SIGPIPE 重置为 SIG_DFL。
            //
            // -Zon-broken-pipe 提供了一个在此处改变默认行为的契机。
            if !crate::sys::pal::on_broken_pipe_flag_used() {
                #[cfg(target_os = "android")] // see issue #88585
                {
                    let mut action: libc::sigaction = mem::zeroed();
                    action.sa_sigaction = libc::SIG_DFL;
                    cvt(libc::sigaction(libc::SIGPIPE, &action, crate::ptr::null_mut()))?;
                }
                #[cfg(not(target_os = "android"))]
                {
                    let ret = sys::signal(libc::SIGPIPE, libc::SIG_DFL);
                    if ret == libc::SIG_ERR {
                        return Err(io::Error::last_os_error());
                    }
                }
                #[cfg(target_os = "hurd")]
                {
                    let ret = sys::signal(libc::SIGLOST, libc::SIG_DFL);
                    if ret == libc::SIG_ERR {
                        return Err(io::Error::last_os_error());
                    }
                }
            }
        }

        for callback in self.get_closures().iter_mut() {
            callback()?;
        }

        // 尽管我们在这里执行的是 exec，但本函数也可能带着一个错误返回（而并未真正 exec），
        // 在这种情况下，我们想确保把全局环境恢复到它原先的样子，从而保证我们的临时覆盖
        // （temporary override）在被释放（free'd）时不会破坏本进程的环境。
        let mut _reset = None;
        if let Some(envp) = maybe_envp {
            struct Reset(*const *const libc::c_char);

            impl Drop for Reset {
                fn drop(&mut self) {
                    unsafe {
                        *sys::env::environ() = self.0;
                    }
                }
            }

            _reset = Some(Reset(*sys::env::environ()));
            *sys::env::environ() = envp.as_ptr();
        }

        libc::execvp(self.get_program_cstr().as_ptr(), self.get_argv().as_ptr());
        Err(io::Error::last_os_error())
    }

    #[cfg(any(target_os = "tvos", target_os = "watchos"))]
    unsafe fn do_exec(
        &mut self,
        _stdio: ChildPipes,
        _maybe_envp: Option<&CStringArray>,
    ) -> Result<!, io::Error> {
        return Err(Self::ERR_APPLE_TV_WATCH_NO_FORK_EXEC);
    }

    #[cfg(not(any(
        target_os = "freebsd",
        target_os = "illumos",
        all(target_os = "linux", target_env = "gnu"),
        all(target_os = "linux", target_env = "musl"),
        target_os = "nto",
        target_vendor = "apple",
        target_os = "cygwin",
    )))]
    fn posix_spawn(
        &mut self,
        _: &ChildPipes,
        _: Option<&CStringArray>,
    ) -> io::Result<Option<Process>> {
        Ok(None)
    }

    // 只支持那些 posix_spawn() 可以直接返回 ENOENT 的平台。
    #[cfg(any(
        target_os = "freebsd",
        target_os = "illumos",
        all(target_os = "linux", target_env = "gnu"),
        all(target_os = "linux", target_env = "musl"),
        target_os = "nto",
        target_vendor = "apple",
        target_os = "cygwin",
    ))]
    fn posix_spawn(
        &mut self,
        stdio: &ChildPipes,
        envp: Option<&CStringArray>,
    ) -> io::Result<Option<Process>> {
        #[cfg(target_os = "linux")]
        use core::sync::atomic::{Atomic, AtomicU8, Ordering};

        use crate::mem::MaybeUninit;
        use crate::sys::{self, cvt_nz, on_broken_pipe_flag_used};

        if self.get_gid().is_some()
            || self.get_uid().is_some()
            || (self.env_saw_path() && !self.program_is_path())
            || !self.get_closures().is_empty()
            || self.get_groups().is_some()
            || self.get_chroot().is_some()
        {
            return Ok(None);
        }

        cfg_select! {
            target_os = "linux" => {
                use crate::sys::weak::weak;

                weak!(
                    fn pidfd_spawnp(
                        pidfd: *mut libc::c_int,
                        path: *const libc::c_char,
                        file_actions: *const libc::posix_spawn_file_actions_t,
                        attrp: *const libc::posix_spawnattr_t,
                        argv: *const *mut libc::c_char,
                        envp: *const *mut libc::c_char,
                    ) -> libc::c_int;
                );

                static PIDFD_SUPPORTED: Atomic<u8> = AtomicU8::new(0);
                const UNKNOWN: u8 = 0;
                const SPAWN: u8 = 1;
                // 通过 fork+exec 路径来获取 pidfd 可能可行
                const FORK_EXEC: u8 = 2;
                // pidfd_spawn 和 fork/exec 都无法给我们一个 pidfd。
                // 于是，如果其他前提条件都满足，我们就只做 posix_spawn。
                const NO: u8 = 3;

                if self.get_create_pidfd() {
                    let mut support = PIDFD_SUPPORTED.load(Ordering::Relaxed);
                    if support == FORK_EXEC {
                        return Ok(None);
                    }
                    if support == UNKNOWN {
                        support = NO;

                        match PidFd::current_process() {
                            Ok(pidfd) => {
                                // 如果 pidfd_open 可用，那么我们至少知道 fork 路径是可用的。
                                support = FORK_EXEC;
                                // 但对于快速路径，我们需要 spawnp 以及
                                // pidfd -> pid 的转换都能正常工作。
                                if pidfd_spawnp.get().is_some() && let Ok(pid) = pidfd.pid() {
                                    assert_eq!(pid, crate::process::id(), "sanity check");
                                    support = SPAWN;
                                }
                            }
                            Err(e) if e.raw_os_error() == Some(libc::EMFILE) => {
                                // 我们（暂时？）耗尽了文件描述符。在这种情况下 pidfd_spawnp 同样会失败。
                                // 不要更新 support 标志，以便我们稍后可以再次探测（probe）。
                                return Err(e)
                            }
                            _ => {
                                // pidfd_open 不可用？很可能是一个不支持 pidfd 的旧内核。
                            }
                        }
                        PIDFD_SUPPORTED.store(support, Ordering::Relaxed);
                        if support == FORK_EXEC {
                            return Ok(None);
                        }
                    }
                    core::assert_matches::debug_assert_matches!(support, SPAWN | NO);
                }
            }
            _ => {
                if self.get_create_pidfd() {
                    unreachable!("only implemented on linux")
                }
            }
        }

        // 只有 glibc 2.24+ 的 posix_spawn() 才支持直接返回 ENOENT。
        #[cfg(all(target_os = "linux", target_env = "gnu"))]
        {
            if let Some(version) = sys::os::glibc_version() {
                if version < (2, 24) {
                    return Ok(None);
                }
            } else {
                return Ok(None);
            }
        }

        // 在 QNX Neutrino 上，posix_spawnp 可能以 EBADF 失败，原因是“在 posix_spawn() 进行期间，
        // 另一个线程可能打开或关闭了某个文件描述符”。
        // 文档说“……或者尝试再次调用 posix_spawn()”。这正是我们在这里所做的。
        // See also http://www.qnx.com/developers/docs/7.1/#com.qnx.doc.neutrino.lib_ref/topic/p/posix_spawn.html
        #[cfg(target_os = "nto")]
        unsafe fn retrying_libc_posix_spawnp(
            pid: *mut pid_t,
            file: *const c_char,
            file_actions: *const posix_spawn_file_actions_t,
            attrp: *const posix_spawnattr_t,
            argv: *const *mut c_char,
            envp: *const *mut c_char,
        ) -> io::Result<i32> {
            let mut delay = MIN_FORKSPAWN_SLEEP;
            loop {
                match libc::posix_spawnp(pid, file, file_actions, attrp, argv, envp) {
                    libc::EBADF => {
                        if delay < get_clock_resolution() {
                            // 我们无法睡眠这么短的时间（实际睡眠会更长）。
                            // 改为让出（Yield）。
                            thread::yield_now();
                        } else if delay < MAX_FORKSPAWN_SLEEP {
                            thread::sleep(delay);
                        } else {
                            return Err(io::const_error!(
                                ErrorKind::WouldBlock,
                                "posix_spawnp returned EBADF too often",
                            ));
                        }
                        delay *= 2;
                        continue;
                    }
                    r => {
                        return Ok(r);
                    }
                }
            }
        }

        type PosixSpawnAddChdirFn = unsafe extern "C" fn(
            *mut libc::posix_spawn_file_actions_t,
            *const libc::c_char,
        ) -> libc::c_int;

        /// 在假定使用动态 libc 的前提下，获取用于向 `posix_spawn_file_actions_t` 添加
        /// 一个 chdir action 的函数指针（若可用）。
        ///
        /// 某些平台能够在 `posix_spawn` 路径中为被 spawn 的进程设置一个新的工作目录。
        /// 本函数查找用于向 `posix_spawn_file_actions_t` 结构体添加这样一个 action 的
        /// 函数指针。
        #[cfg(not(any(all(target_os = "linux", target_env = "musl"), target_os = "cygwin")))]
        fn get_posix_spawn_addchdir() -> Option<PosixSpawnAddChdirFn> {
            use crate::sys::weak::weak;

            // POSIX.1-2024 把这个函数标准化了：
            // https://pubs.opengroup.org/onlinepubs/9799919799/functions/posix_spawn_file_actions_addchdir.html。
            // 不过 _np 版本的可用范围更广，因此先尝试它。

            weak!(
                fn posix_spawn_file_actions_addchdir_np(
                    file_actions: *mut libc::posix_spawn_file_actions_t,
                    path: *const libc::c_char,
                ) -> libc::c_int;
            );

            weak!(
                fn posix_spawn_file_actions_addchdir(
                    file_actions: *mut libc::posix_spawn_file_actions_t,
                    path: *const libc::c_char,
                ) -> libc::c_int;
            );

            posix_spawn_file_actions_addchdir_np
                .get()
                .or_else(|| posix_spawn_file_actions_addchdir.get())
        }

        /// 在已知该函数存在的平台上，获取用于向 `posix_spawn_file_actions_t` 添加
        /// 一个 chdir action 的函数指针（若可用）。
        ///
        /// 弱符号（Weak symbol）查找对静态链接的 libc 不起作用，因此在可能进行静态链接的
        /// 情况下，我们要么需要在编译期检查该符号是否存在，要么需要事先就知道它。
        ///
        /// Cygwin 不支持弱符号，所以直接把它链接进来。
        #[cfg(any(all(target_os = "linux", target_env = "musl"), target_os = "cygwin"))]
        fn get_posix_spawn_addchdir() -> Option<PosixSpawnAddChdirFn> {
            // 我们所要求的最低 musl 版本就支持此函数，因此可以直接使用它。
            Some(libc::posix_spawn_file_actions_addchdir_np)
        }

        let addchdir = match self.get_cwd() {
            Some(cwd) => {
                if cfg!(target_vendor = "apple") {
                    // macOS 上有一个 bug：像 "../myprogram" 这样的相对可执行路径
                    // 会导致 `posix_spawn` 成功启动该程序，但当与
                    // posix_spawn_file_actions_addchdir_np（它在 macOS 10.15 中引入）
                    // 一起使用时，却错误地返回 ENOENT。
                    if self.get_program_kind() == ProgramKind::Relative {
                        return Ok(None);
                    }
                }
                // 现在检查 posix_spawn 的 addchdir 函数是否可用。
                // 如果它不可用，就放弃（bail）并改用 fork/exec 路径。
                match get_posix_spawn_addchdir() {
                    Some(f) => Some((f, cwd)),
                    None => return Ok(None),
                }
            }
            None => None,
        };

        let pgroup = self.get_pgroup();

        struct PosixSpawnFileActions<'a>(&'a mut MaybeUninit<libc::posix_spawn_file_actions_t>);

        impl Drop for PosixSpawnFileActions<'_> {
            fn drop(&mut self) {
                unsafe {
                    libc::posix_spawn_file_actions_destroy(self.0.as_mut_ptr());
                }
            }
        }

        struct PosixSpawnattr<'a>(&'a mut MaybeUninit<libc::posix_spawnattr_t>);

        impl Drop for PosixSpawnattr<'_> {
            fn drop(&mut self) {
                unsafe {
                    libc::posix_spawnattr_destroy(self.0.as_mut_ptr());
                }
            }
        }

        unsafe {
            let mut attrs = MaybeUninit::uninit();
            cvt_nz(libc::posix_spawnattr_init(attrs.as_mut_ptr()))?;
            let attrs = PosixSpawnattr(&mut attrs);

            let mut flags = 0;

            let mut file_actions = MaybeUninit::uninit();
            cvt_nz(libc::posix_spawn_file_actions_init(file_actions.as_mut_ptr()))?;
            let file_actions = PosixSpawnFileActions(&mut file_actions);

            if let Some(fd) = stdio.stdin.fd() {
                cvt_nz(libc::posix_spawn_file_actions_adddup2(
                    file_actions.0.as_mut_ptr(),
                    fd,
                    libc::STDIN_FILENO,
                ))?;
            }
            if let Some(fd) = stdio.stdout.fd() {
                cvt_nz(libc::posix_spawn_file_actions_adddup2(
                    file_actions.0.as_mut_ptr(),
                    fd,
                    libc::STDOUT_FILENO,
                ))?;
            }
            if let Some(fd) = stdio.stderr.fd() {
                cvt_nz(libc::posix_spawn_file_actions_adddup2(
                    file_actions.0.as_mut_ptr(),
                    fd,
                    libc::STDERR_FILENO,
                ))?;
            }
            if let Some((f, cwd)) = addchdir {
                cvt_nz(f(file_actions.0.as_mut_ptr(), cwd.as_ptr()))?;
            }

            if let Some(pgroup) = pgroup {
                flags |= libc::POSIX_SPAWN_SETPGROUP;
                cvt_nz(libc::posix_spawnattr_setpgroup(attrs.0.as_mut_ptr(), pgroup))?;
            }

            // 继承本进程的信号掩码（signal mask），而不是重置它（即不调用
            // posix_spawnattr_setsigmask）。

            // 如果使用了 -Zon-broken-pipe，则不要把 SIGPIPE 重置为 SIG_DFL。
            // 如果未使用 -Zon-broken-pipe，则为了向后兼容，把 SIGPIPE 重置为 SIG_DFL。
            //
            // -Zon-broken-pipe 提供了一个在此处改变默认行为的契机。
            if !on_broken_pipe_flag_used() {
                let mut default_set = MaybeUninit::<libc::sigset_t>::uninit();
                cvt(sigemptyset(default_set.as_mut_ptr()))?;
                cvt(sigaddset(default_set.as_mut_ptr(), libc::SIGPIPE))?;
                #[cfg(target_os = "hurd")]
                {
                    cvt(sigaddset(default_set.as_mut_ptr(), libc::SIGLOST))?;
                }
                cvt_nz(libc::posix_spawnattr_setsigdefault(
                    attrs.0.as_mut_ptr(),
                    default_set.as_ptr(),
                ))?;
                flags |= libc::POSIX_SPAWN_SETSIGDEF;
            }

            if self.get_setsid() {
                cfg_select! {
                    all(target_os = "linux", target_env = "gnu") => {
                        flags |= libc::POSIX_SPAWN_SETSID;
                    }
                    _ => {
                        return Ok(None);
                    }
                }
            }

            cvt_nz(libc::posix_spawnattr_setflags(attrs.0.as_mut_ptr(), flags as _))?;

            // 确保我们对全局 `environ` 资源的访问是同步的
            let _env_lock = sys::env::env_read_lock();
            let envp = envp.map(|c| c.as_ptr()).unwrap_or_else(|| *sys::env::environ() as *const _);

            #[cfg(not(target_os = "nto"))]
            let spawn_fn = libc::posix_spawnp;
            #[cfg(target_os = "nto")]
            let spawn_fn = retrying_libc_posix_spawnp;

            #[cfg(target_os = "linux")]
            if self.get_create_pidfd() && PIDFD_SUPPORTED.load(Ordering::Relaxed) == SPAWN {
                let mut pidfd: libc::c_int = -1;
                let spawn_res = pidfd_spawnp.get().unwrap()(
                    &mut pidfd,
                    self.get_program_cstr().as_ptr(),
                    file_actions.0.as_ptr(),
                    attrs.0.as_ptr(),
                    self.get_argv().as_ptr() as *const _,
                    envp as *const _,
                );

                let spawn_res = cvt_nz(spawn_res);
                if let Err(ref e) = spawn_res
                    && e.raw_os_error() == Some(libc::ENOSYS)
                {
                    PIDFD_SUPPORTED.store(FORK_EXEC, Ordering::Relaxed);
                    return Ok(None);
                }
                spawn_res?;

                use crate::os::fd::{FromRawFd, IntoRawFd};

                let pidfd = PidFd::from_raw_fd(pidfd);
                let pid = match pidfd.pid() {
                    Ok(pid) => pid,
                    Err(e) => {
                        // 子进程已被 spawn，并且我们持有它的 pidfd。
                        // 但即便前面已经验证过 pidfd_spawnp 和 getpid 的支持，
                        // 我们仍然无法获取它的 pid。
                        // 这相当不太可能发生，但如果该 ioctl 不受支持、glibc 尝试改用 procfs，
                        // 而我们又耗尽了文件描述符，就可能出现这种情况。
                        return Err(Error::new(
                            e.kind(),
                            "pidfd_spawnp succeeded but the child's PID could not be obtained",
                        ));
                    }
                };

                return Ok(Some(Process::new(pid as i32, pidfd.into_raw_fd())));
            }

            // Safety: -1 表示我们没有 pidfd。
            let mut p = Process::new(0, -1);

            let spawn_res = spawn_fn(
                &mut p.pid,
                self.get_program_cstr().as_ptr(),
                file_actions.0.as_ptr(),
                attrs.0.as_ptr(),
                self.get_argv().as_ptr() as *const _,
                envp as *const _,
            );

            #[cfg(target_os = "nto")]
            let spawn_res = spawn_res?;

            cvt_nz(spawn_res)?;
            Ok(Some(p))
        }
    }

    #[cfg(target_os = "linux")]
    fn send_pidfd(&self, sock: &crate::sys::net::Socket) {
        use libc::{CMSG_DATA, CMSG_FIRSTHDR, CMSG_LEN, CMSG_SPACE, SCM_RIGHTS, SOL_SOCKET};

        use crate::io::IoSlice;
        use crate::os::fd::RawFd;
        use crate::sys::cvt_r;

        unsafe {
            let child_pid = libc::getpid();
            // pidfd_open 默认会设置 CLOEXEC
            let pidfd = libc::syscall(libc::SYS_pidfd_open, child_pid, 0);

            let fds: [c_int; 1] = [pidfd as RawFd];

            const SCM_MSG_LEN: usize = size_of::<[c_int; 1]>();

            #[repr(C)]
            union Cmsg {
                buf: [u8; unsafe { CMSG_SPACE(SCM_MSG_LEN as u32) as usize }],
                _align: libc::cmsghdr,
            }

            let mut cmsg: Cmsg = mem::zeroed();

            // 一条 0 长度的消息，通过 socket 发送，这样我们就能借此把 fd 传递过去
            let mut iov = [IoSlice::new(b"")];
            let mut msg: libc::msghdr = mem::zeroed();

            msg.msg_iov = (&raw mut iov) as *mut _;
            msg.msg_iovlen = 1;

            // 只有在我们成功获取到 pidfd 时才附加 cmsg
            if pidfd >= 0 {
                msg.msg_controllen = size_of_val(&cmsg.buf) as _;
                msg.msg_control = (&raw mut cmsg.buf) as *mut _;

                let hdr = CMSG_FIRSTHDR((&raw mut msg) as *mut _);
                (*hdr).cmsg_level = SOL_SOCKET;
                (*hdr).cmsg_type = SCM_RIGHTS;
                (*hdr).cmsg_len = CMSG_LEN(SCM_MSG_LEN as _) as _;
                let data = CMSG_DATA(hdr);
                crate::ptr::copy_nonoverlapping(
                    fds.as_ptr().cast::<u8>(),
                    data as *mut _,
                    SCM_MSG_LEN,
                );
            }

            // 即使我们未能获取到 pidfd，也仍然发送这条 0 长度的消息，
            // 以便我们得到一个一致的 SEQPACKET 顺序
            match cvt_r(|| libc::sendmsg(sock.as_raw(), &msg, 0)) {
                Ok(0) => {}
                other => rtabort!("failed to communicate with parent process. {:?}", other),
            }
        }
    }

    #[cfg(target_os = "linux")]
    fn recv_pidfd(&self, sock: &crate::sys::net::Socket) -> pid_t {
        use libc::{CMSG_DATA, CMSG_FIRSTHDR, CMSG_LEN, CMSG_SPACE, SCM_RIGHTS, SOL_SOCKET};

        use crate::io::IoSliceMut;
        use crate::sys::cvt_r;

        unsafe {
            const SCM_MSG_LEN: usize = size_of::<[c_int; 1]>();

            #[repr(C)]
            union Cmsg {
                _buf: [u8; unsafe { CMSG_SPACE(SCM_MSG_LEN as u32) as usize }],
                _align: libc::cmsghdr,
            }
            let mut cmsg: Cmsg = mem::zeroed();
            // 一次 0 长度的读取，以获取该 fd
            let mut iov = [IoSliceMut::new(&mut [])];

            let mut msg: libc::msghdr = mem::zeroed();

            msg.msg_iov = (&raw mut iov) as *mut _;
            msg.msg_iovlen = 1;
            msg.msg_controllen = size_of::<Cmsg>() as _;
            msg.msg_control = (&raw mut cmsg) as *mut _;

            match cvt_r(|| libc::recvmsg(sock.as_raw(), &mut msg, libc::MSG_CMSG_CLOEXEC)) {
                Err(_) => return -1,
                Ok(_) => {}
            }

            let hdr = CMSG_FIRSTHDR((&raw mut msg) as *mut _);
            if hdr.is_null()
                || (*hdr).cmsg_level != SOL_SOCKET
                || (*hdr).cmsg_type != SCM_RIGHTS
                || (*hdr).cmsg_len != CMSG_LEN(SCM_MSG_LEN as _) as _
            {
                return -1;
            }
            let data = CMSG_DATA(hdr);

            let mut fds = [-1 as c_int];

            crate::ptr::copy_nonoverlapping(
                data as *const _,
                fds.as_mut_ptr().cast::<u8>(),
                SCM_MSG_LEN,
            );

            fds[0]
        }
    }
}

////////////////////////////////////////////////////////////////////////////////
// 进程（Processes）
////////////////////////////////////////////////////////////////////////////////

/// 进程的唯一 ID（它绝不应为负数）。
pub struct Process {
    pid: pid_t,
    status: Option<ExitStatus>,
    // 在 Linux 上，存储为该子进程创建的 pidfd。
    // 如果用户没有请求创建 pidfd，或者由于某种原因 pidfd 无法被创建
    //（例如 `pidfd_open` 系统调用不可用），则它为 None。
    #[cfg(target_os = "linux")]
    pidfd: Option<PidFd>,
}

impl Process {
    #[cfg(target_os = "linux")]
    /// # 安全性(Safety）
    ///
    /// `pidfd` 必须要么是 -1（表示没有文件描述符），要么是一个有效的、被独占拥有
    /// （exclusively owned）的文件描述符（参见 [I/O Safety]）。
    ///
    /// [I/O Safety]: crate::io#io-safety
    unsafe fn new(pid: pid_t, pidfd: pid_t) -> Self {
        use crate::os::unix::io::FromRawFd;
        use crate::sys::FromInner;
        // Safety: 如果 `pidfd` 为非负值，我们就假定它有效，并且在其他情况下是无主的（unowned）。
        let pidfd = (pidfd >= 0).then(|| PidFd::from_inner(sys::fd::FileDesc::from_raw_fd(pidfd)));
        Process { pid, status: None, pidfd }
    }

    #[cfg(not(target_os = "linux"))]
    unsafe fn new(pid: pid_t, _pidfd: pid_t) -> Self {
        Process { pid, status: None }
    }

    pub fn id(&self) -> u32 {
        self.pid as u32
    }

    pub fn kill(&self) -> io::Result<()> {
        self.send_signal(libc::SIGKILL)
    }

    pub(crate) fn send_signal(&self, signal: i32) -> io::Result<()> {
        // 如果我们已经对该进程 wait 过了，那么这个 pid 可能会被回收（recycled）并用于
        // 另一个进程，而我们大概不应该向随机的进程发送信号，因此返回 Ok，
        // 因为该进程已经退出了。
        if self.status.is_some() {
            return Ok(());
        }
        #[cfg(target_os = "linux")]
        if let Some(pid_fd) = self.pidfd.as_ref() {
            // pidfd_send_signal 出现得比 pidfd_open 早。因此，如果我们能拿到一个 fd，那么发送信号也能正常工作
            return pid_fd.send_signal(signal);
        }
        cvt(unsafe { libc::kill(self.pid, signal) }).map(drop)
    }

    pub fn wait(&mut self) -> io::Result<ExitStatus> {
        use crate::sys::cvt_r;
        if let Some(status) = self.status {
            return Ok(status);
        }
        #[cfg(target_os = "linux")]
        if let Some(pid_fd) = self.pidfd.as_ref() {
            let status = pid_fd.wait()?;
            self.status = Some(status);
            return Ok(status);
        }
        let mut status = 0 as c_int;
        cvt_r(|| unsafe { libc::waitpid(self.pid, &mut status, 0) })?;
        self.status = Some(ExitStatus::new(status));
        Ok(ExitStatus::new(status))
    }

    pub fn try_wait(&mut self) -> io::Result<Option<ExitStatus>> {
        if let Some(status) = self.status {
            return Ok(Some(status));
        }
        #[cfg(target_os = "linux")]
        if let Some(pid_fd) = self.pidfd.as_ref() {
            let status = pid_fd.try_wait()?;
            if let Some(status) = status {
                self.status = Some(status)
            }
            return Ok(status);
        }
        let mut status = 0 as c_int;
        let pid = cvt(unsafe { libc::waitpid(self.pid, &mut status, libc::WNOHANG) })?;
        if pid == 0 {
            Ok(None)
        } else {
            self.status = Some(ExitStatus::new(status));
            Ok(Some(ExitStatus::new(status)))
        }
    }
}

/// Unix 退出状态（exit statuses）
//
// 在 Unix 术语中，这其实并不是一个 "exit status"（退出状态）。确切地说，它是一个
// "wait status"（等待状态）。
// 参见 `std::process::ExitStatus` 的注释和文档注释中的讨论。
#[derive(PartialEq, Eq, Clone, Copy, Default)]
pub struct ExitStatus(c_int);

impl fmt::Debug for ExitStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("unix_wait_status").field(&self.0).finish()
    }
}

impl ExitStatus {
    pub fn new(status: c_int) -> ExitStatus {
        ExitStatus(status)
    }

    #[cfg(target_os = "linux")]
    pub fn from_waitid_siginfo(siginfo: libc::siginfo_t) -> ExitStatus {
        let status = unsafe { siginfo.si_status() };

        match siginfo.si_code {
            libc::CLD_EXITED => ExitStatus((status & 0xff) << 8),
            libc::CLD_KILLED => ExitStatus(status),
            libc::CLD_DUMPED => ExitStatus(status | 0x80),
            libc::CLD_CONTINUED => ExitStatus(0xffff),
            libc::CLD_STOPPED | libc::CLD_TRAPPED => ExitStatus(((status & 0xff) << 8) | 0x7f),
            _ => unreachable!("waitid() should only return the above codes"),
        }
    }

    fn exited(&self) -> bool {
        libc::WIFEXITED(self.0)
    }

    pub fn exit_ok(&self) -> Result<(), ExitStatusError> {
        // 它假定 WIFEXITED(status) && WEXITSTATUS==0 对应于 status==0。
        // 这在所有实际版本的 Unix 上都成立，被广泛假定，并在 SuS 中有明确规定
        // https://pubs.opengroup.org/onlinepubs/9699919799/functions/wait.html。
        // 如果对某个假装成 Unix 的平台而言它不成立，那么这些测试（我们的 doctests，
        // 以及 unix/tests.rs）会发现它。`ExitStatusError::code` 也做了同样的假定。
        match NonZero::try_from(self.0) {
            /* was nonzero */ Ok(failure) => Err(ExitStatusError(failure)),
            /* was zero, couldn't convert */ Err(_) => Ok(()),
        }
    }

    pub fn code(&self) -> Option<i32> {
        self.exited().then(|| libc::WEXITSTATUS(self.0))
    }

    pub fn signal(&self) -> Option<i32> {
        libc::WIFSIGNALED(self.0).then(|| libc::WTERMSIG(self.0))
    }

    pub fn core_dumped(&self) -> bool {
        libc::WIFSIGNALED(self.0) && libc::WCOREDUMP(self.0)
    }

    pub fn stopped_signal(&self) -> Option<i32> {
        libc::WIFSTOPPED(self.0).then(|| libc::WSTOPSIG(self.0))
    }

    pub fn continued(&self) -> bool {
        libc::WIFCONTINUED(self.0)
    }

    pub fn into_raw(&self) -> c_int {
        self.0
    }
}

/// 通过包装一个原始的 `c_int`（不进行拷贝）来把它转换为类型安全的 `ExitStatus`。
impl From<c_int> for ExitStatus {
    fn from(a: c_int) -> ExitStatus {
        ExitStatus(a)
    }
}

/// 把一个信号编号（signal number）转换为一个可读、可搜索的名称。
///
/// 这个字符串应当紧接在信号编号之后显示。
/// 如果某个信号无法识别，它会返回空字符串，这样你就只会得到像 "0" 这样的数字。
/// 如果它能被识别，你会得到类似 "9 (SIGKILL)" 这样的结果。
fn signal_string(signal: i32) -> &'static str {
    match signal {
        libc::SIGHUP => " (SIGHUP)",
        libc::SIGINT => " (SIGINT)",
        libc::SIGQUIT => " (SIGQUIT)",
        libc::SIGILL => " (SIGILL)",
        libc::SIGTRAP => " (SIGTRAP)",
        libc::SIGABRT => " (SIGABRT)",
        #[cfg(not(target_os = "l4re"))]
        libc::SIGBUS => " (SIGBUS)",
        libc::SIGFPE => " (SIGFPE)",
        libc::SIGKILL => " (SIGKILL)",
        #[cfg(not(target_os = "l4re"))]
        libc::SIGUSR1 => " (SIGUSR1)",
        libc::SIGSEGV => " (SIGSEGV)",
        #[cfg(not(target_os = "l4re"))]
        libc::SIGUSR2 => " (SIGUSR2)",
        libc::SIGPIPE => " (SIGPIPE)",
        libc::SIGALRM => " (SIGALRM)",
        libc::SIGTERM => " (SIGTERM)",
        #[cfg(not(target_os = "l4re"))]
        libc::SIGCHLD => " (SIGCHLD)",
        #[cfg(not(target_os = "l4re"))]
        libc::SIGCONT => " (SIGCONT)",
        #[cfg(not(target_os = "l4re"))]
        libc::SIGSTOP => " (SIGSTOP)",
        #[cfg(not(target_os = "l4re"))]
        libc::SIGTSTP => " (SIGTSTP)",
        #[cfg(not(target_os = "l4re"))]
        libc::SIGTTIN => " (SIGTTIN)",
        #[cfg(not(target_os = "l4re"))]
        libc::SIGTTOU => " (SIGTTOU)",
        #[cfg(not(target_os = "l4re"))]
        libc::SIGURG => " (SIGURG)",
        #[cfg(not(target_os = "l4re"))]
        libc::SIGXCPU => " (SIGXCPU)",
        #[cfg(not(any(target_os = "l4re", target_os = "rtems")))]
        libc::SIGXFSZ => " (SIGXFSZ)",
        #[cfg(not(any(target_os = "l4re", target_os = "rtems")))]
        libc::SIGVTALRM => " (SIGVTALRM)",
        #[cfg(not(target_os = "l4re"))]
        libc::SIGPROF => " (SIGPROF)",
        #[cfg(not(any(target_os = "l4re", target_os = "rtems")))]
        libc::SIGWINCH => " (SIGWINCH)",
        #[cfg(not(any(target_os = "haiku", target_os = "l4re")))]
        libc::SIGIO => " (SIGIO)",
        #[cfg(target_os = "haiku")]
        libc::SIGPOLL => " (SIGPOLL)",
        #[cfg(not(target_os = "l4re"))]
        libc::SIGSYS => " (SIGSYS)",
        // 关于 Linux 信号的信息，运行 `man 7 signal`
        #[cfg(all(
            target_os = "linux",
            any(
                target_arch = "x86_64",
                target_arch = "x86",
                target_arch = "arm",
                target_arch = "aarch64"
            )
        ))]
        libc::SIGSTKFLT => " (SIGSTKFLT)",
        #[cfg(any(target_os = "linux", target_os = "nto", target_os = "cygwin"))]
        libc::SIGPWR => " (SIGPWR)",
        #[cfg(any(
            target_os = "freebsd",
            target_os = "netbsd",
            target_os = "openbsd",
            target_os = "dragonfly",
            target_os = "nto",
            target_vendor = "apple",
            target_os = "cygwin",
        ))]
        libc::SIGEMT => " (SIGEMT)",
        #[cfg(any(
            target_os = "freebsd",
            target_os = "netbsd",
            target_os = "openbsd",
            target_os = "dragonfly",
            target_vendor = "apple",
        ))]
        libc::SIGINFO => " (SIGINFO)",
        #[cfg(target_os = "hurd")]
        libc::SIGLOST => " (SIGLOST)",
        #[cfg(target_os = "freebsd")]
        libc::SIGTHR => " (SIGTHR)",
        #[cfg(target_os = "freebsd")]
        libc::SIGLIBRT => " (SIGLIBRT)",
        _ => "",
    }
}

impl fmt::Display for ExitStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(code) = self.code() {
            write!(f, "exit status: {code}")
        } else if let Some(signal) = self.signal() {
            let signal_string = signal_string(signal);
            if self.core_dumped() {
                write!(f, "signal: {signal}{signal_string} (core dumped)")
            } else {
                write!(f, "signal: {signal}{signal_string}")
            }
        } else if let Some(signal) = self.stopped_signal() {
            let signal_string = signal_string(signal);
            write!(f, "stopped (not terminated) by signal: {signal}{signal_string}")
        } else if self.continued() {
            write!(f, "continued (WIFCONTINUED)")
        } else {
            write!(f, "unrecognised wait status: {} {:#x}", self.0, self.0)
        }
    }
}

#[derive(PartialEq, Eq, Clone, Copy)]
pub struct ExitStatusError(NonZero<c_int>);

impl Into<ExitStatus> for ExitStatusError {
    fn into(self) -> ExitStatus {
        ExitStatus(self.0.into())
    }
}

impl fmt::Debug for ExitStatusError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("unix_wait_status").field(&self.0).finish()
    }
}

impl ExitStatusError {
    pub fn code(self) -> Option<NonZero<i32>> {
        ExitStatus(self.0.into()).code().map(|st| st.try_into().unwrap())
    }
}

#[cfg(target_os = "linux")]
mod linux_child_ext {
    use crate::io::ErrorKind;
    use crate::os::linux::process as os;
    use crate::sys::FromInner;
    use crate::sys::pal::linux::pidfd as imp;
    use crate::{io, mem};

    #[unstable(feature = "linux_pidfd", issue = "82971")]
    impl crate::os::linux::process::ChildExt for crate::process::Child {
        fn pidfd(&self) -> io::Result<&os::PidFd> {
            self.handle
                .pidfd
                .as_ref()
                // SAFETY: 该 os 类型是一个透明包装器（transparent wrapper），因此我们可以对引用做 transmute
                .map(|fd| unsafe { mem::transmute::<&imp::PidFd, &os::PidFd>(fd) })
                .ok_or_else(|| io::const_error!(ErrorKind::Uncategorized, "no pidfd was created."))
        }

        fn into_pidfd(mut self) -> Result<os::PidFd, Self> {
            self.handle
                .pidfd
                .take()
                .map(|fd| <os::PidFd as FromInner<imp::PidFd>>::from_inner(fd))
                .ok_or_else(|| self)
        }
    }
}

#[cfg(test)]
mod tests;

// 参见 [`unsupported_wait_status::compare_with_linux`]；
#[cfg(all(test, target_os = "linux"))]
#[path = "unsupported/wait_status.rs"]
mod unsupported_wait_status;
