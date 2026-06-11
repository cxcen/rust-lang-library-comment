use crate::io;
use crate::os::fd::FromRawFd;
use crate::sys::fd::FileDesc;
use crate::sys::pal::cvt;

pub type Pipe = FileDesc;

pub fn pipe() -> io::Result<(Pipe, Pipe)> {
    let mut fds = [0; 2];

    // 目前已知的、能在创建管道时原子地设置 CLOEXEC 标志的唯一办法，是使用 `pipe2`
    // 系统调用。它在 Linux 2.6.27、glibc 2.9 和 musl 0.9.3 中加入，其他一些目标平台
    // 也提供了它。
    cfg_select! {
        any(
            target_os = "android",
            target_os = "dragonfly",
            target_os = "freebsd",
            target_os = "hurd",
            target_os = "illumos",
            target_os = "linux",
            target_os = "netbsd",
            target_os = "openbsd",
            target_os = "cygwin",
            target_os = "redox"
        ) => {
            unsafe {
                cvt(libc::pipe2(fds.as_mut_ptr(), libc::O_CLOEXEC))?;
                Ok((Pipe::from_raw_fd(fds[0]), Pipe::from_raw_fd(fds[1])))
            }
        }
        _ => {
            unsafe {
                cvt(libc::pipe(fds.as_mut_ptr()))?;

                let fd0 = Pipe::from_raw_fd(fds[0]);
                let fd1 = Pipe::from_raw_fd(fds[1]);
                fd0.set_cloexec()?;
                fd1.set_cloexec()?;
                Ok((fd0, fd1))
            }
        }
    }
}
