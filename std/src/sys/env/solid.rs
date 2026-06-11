use core::slice::memchr;

pub use super::common::Env;
use crate::ffi::{CStr, OsStr, OsString};
use crate::io;
use crate::os::raw::{c_char, c_int};
use crate::os::solid::ffi::{OsStrExt, OsStringExt};
use crate::sync::{PoisonError, RwLock};
use crate::sys::helpers::run_with_cstr;

static ENV_LOCK: RwLock<()> = RwLock::new(());

pub fn env_read_lock() -> impl Drop {
    ENV_LOCK.read().unwrap_or_else(PoisonError::into_inner)
}

/// 返回当前进程所有环境变量的 (变量名, 值) 字节向量对组成的向量。
pub fn env() -> Env {
    unsafe extern "C" {
        static mut environ: *const *const c_char;
    }

    unsafe {
        let _guard = env_read_lock();
        let mut result = Vec::new();
        if !environ.is_null() {
            while !(*environ).is_null() {
                if let Some(key_value) = parse(CStr::from_ptr(*environ).to_bytes()) {
                    result.push(key_value);
                }
                environ = environ.add(1);
            }
        }
        return Env::new(result);
    }

    fn parse(input: &[u8]) -> Option<(OsString, OsString)> {
        // 策略（抄自 glibc）：变量名与值之间以 ASCII 等号 '=' 分隔。
        // 由于变量名不能为空，所以允许变量名以等号开头。
        // 跳过所有格式错误的行。
        if input.is_empty() {
            return None;
        }
        let pos = memchr::memchr(b'=', &input[1..]).map(|p| p + 1);
        pos.map(|p| {
            (
                OsStringExt::from_vec(input[..p].to_vec()),
                OsStringExt::from_vec(input[p + 1..].to_vec()),
            )
        })
    }
}

pub fn getenv(k: &OsStr) -> Option<OsString> {
    // 含有 nul 字节的环境变量无法被设置，因此它们的值
    // 也始终为 None
    run_with_cstr(k.as_bytes(), &|k| {
        let _guard = env_read_lock();
        let v = unsafe { libc::getenv(k.as_ptr()) } as *const libc::c_char;

        if v.is_null() {
            Ok(None)
        } else {
            // SAFETY: 由于我们持有读锁，执行这一行期间 `v` 不会被改动
            let bytes = unsafe { CStr::from_ptr(v) }.to_bytes().to_vec();

            Ok(Some(OsStringExt::from_vec(bytes)))
        }
    })
    .ok()
    .flatten()
}

pub unsafe fn setenv(k: &OsStr, v: &OsStr) -> io::Result<()> {
    run_with_cstr(k.as_bytes(), &|k| {
        run_with_cstr(v.as_bytes(), &|v| {
            let _guard = ENV_LOCK.write();
            cvt_env(unsafe { libc::setenv(k.as_ptr(), v.as_ptr(), 1) }).map(drop)
        })
    })
}

pub unsafe fn unsetenv(n: &OsStr) -> io::Result<()> {
    run_with_cstr(n.as_bytes(), &|nbuf| {
        let _guard = ENV_LOCK.write();
        cvt_env(unsafe { libc::unsetenv(nbuf.as_ptr()) }).map(drop)
    })
}

/// 在 kmclib 中，`setenv` 和 `unsetenv` 并不总是设置 `errno`，因此该
/// 函数只返回一个通用错误。
fn cvt_env(t: c_int) -> io::Result<c_int> {
    if t == -1 { Err(io::const_error!(io::ErrorKind::Uncategorized, "failure")) } else { Ok(t) }
}
