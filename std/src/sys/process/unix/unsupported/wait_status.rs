//! 针对非 Unix 的 #[cfg(unix)] 平台的、模拟出来的 wait status（等待状态）
//!
//! 单独成一个模块，以便于针对真正的 Unix 实现进行测试。

use super::ExitStatusError;
use crate::ffi::c_int;
use crate::fmt;
use crate::num::NonZero;

/// 供 `unsupported.rs` 使用的、模拟出来的 wait status
///
/// 使用 "traditional unix"（传统 unix）编码。用于那些虽然是 `#[cfg(unix)]`、
/// 但实际上根本不支持子进程的平台。
///
/// 这些平台并不是 Unix，只是为了移植方便而假装成 Unix。
/// 因此，我们在这里提供一份忠实的伪装（pretence）。
#[derive(PartialEq, Eq, Clone, Copy, Debug, Default)]
pub struct ExitStatus {
    wait_status: c_int,
}

/// 通过包装一个原始的 `c_int` 来把它转换为类型安全的 `ExitStatus`
impl From<c_int> for ExitStatus {
    fn from(wait_status: c_int) -> ExitStatus {
        ExitStatus { wait_status }
    }
}

impl fmt::Display for ExitStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "emulated wait status: {}", self.wait_status)
    }
}

impl ExitStatus {
    pub fn code(&self) -> Option<i32> {
        // Linux 和 FreeBSD 都认为：linux 中值的第 0x80 位（high bit）也算作 "WIFEXITED"，
        // 尽管这相当离谱。
        // 同样地，这些宏会忽略所有高位，因此乐于把越界（out-of-range）的值
        // 声明为 WIFEXITED、WIFSTOPPED 等等。
        let w = self.wait_status;
        if (w & 0x7f) == 0 { Some((w & 0xff00) >> 8) } else { None }
    }

    #[allow(unused)]
    pub fn exit_ok(&self) -> Result<(), ExitStatusError> {
        // 它假定 WIFEXITED(status) && WEXITSTATUS==0 对应于 status==0。
        // 这在所有实际版本的 Unix 上都成立，被广泛假定，并在 SuS 中有明确规定
        // https://pubs.opengroup.org/onlinepubs/9699919799/functions/wait.html。
        // 如果对某个假装成 Unix 的平台而言它不成立，那么这些测试（我们的 doctests，
        // 以及 unix/tests.rs）会发现它。`ExitStatusError::code` 也做了同样的假定。
        match NonZero::try_from(self.wait_status) {
            /* was nonzero */ Ok(failure) => Err(ExitStatusError(failure)),
            /* was zero, couldn't convert */ Err(_) => Ok(()),
        }
    }

    pub fn signal(&self) -> Option<i32> {
        let signal = self.wait_status & 0x007f;
        if signal > 0 && signal < 0x7f { Some(signal) } else { None }
    }

    pub fn core_dumped(&self) -> bool {
        self.signal().is_some() && (self.wait_status & 0x80) != 0
    }

    pub fn stopped_signal(&self) -> Option<i32> {
        let w = self.wait_status;
        if (w & 0xff) == 0x7f { Some((w & 0xff00) >> 8) } else { None }
    }

    pub fn continued(&self) -> bool {
        self.wait_status == 0xffff
    }

    pub fn into_raw(&self) -> c_int {
        self.wait_status
    }
}

#[cfg(test)]
#[path = "wait_status/tests.rs"]
// 之所以需要它，是因为为了测试目的，本模块也会通过 #[path] 被导入
mod tests;
