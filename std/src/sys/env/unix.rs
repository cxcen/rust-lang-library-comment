use core::slice::memchr;

use libc::c_char;

pub use super::common::Env;
use crate::ffi::{CStr, OsStr, OsString};
use crate::io;
use crate::os::unix::prelude::*;
use crate::sync::{PoisonError, RwLock};
use crate::sys::cvt;
use crate::sys::helpers::run_with_cstr;

// 在 Apple 平台上使用 `_NSGetEnviron`。
//
// `_NSGetEnviron` 是有文档记载的替代方案（见 `man environ`），并且
// 自 macOS 和 iOS 的最初版本起就一直可用。
//
// 如今，具体说是自 macOS 10.8 起，`environ` 已通过
// `libdyld.dylib` 暴露出来，而后者经由 `libSystem.dylib` 链接：
// <https://github.com/apple-oss-distributions/dyld/blob/dyld-1160.6/libdyld/libdyldGlue.cpp#L913>
//
// 所以归根结底，我们用哪个选项大概都无所谓；但使用 `_NSGetEnviron`
// 的性能代价极其微小，而且它可能受支持的程度稍微高那么一点点，
// 所以我们就用它好了。
//
// 注意：定义它的那个头文件（`crt_externs.h`）是在 iOS 13.0 SDK 中
// 添加的，这在过去曾就该 API 的可用性造成了大量
// 困惑。
//
// NOTE(madsmtm): 无论是用本方案还是用 `environ`，都尚未经过验证以确认
// 不会导致 App Store 被拒；如果发现确实如此，那么可以用
// `[NSProcessInfo environment]` 来实现一个替代方案
// ——它内部使用 `_NSGetEnviron`，并对环境变量加了一把系统级的
// 锁来防范 `setenv`，所以无论如何用它或许都更可取？不过那也
// 意味着我们必须链接到 Foundation。
#[cfg(target_vendor = "apple")]
pub unsafe fn environ() -> *mut *const *const c_char {
    unsafe { libc::_NSGetEnviron() as *mut *const *const c_char }
}

// 使用属于 POSIX 一部分的 `environ` 静态量。
#[cfg(not(target_vendor = "apple"))]
pub unsafe fn environ() -> *mut *const *const c_char {
    unsafe extern "C" {
        static mut environ: *const *const c_char;
    }
    &raw mut environ
}

static ENV_LOCK: RwLock<()> = RwLock::new(());

pub fn env_read_lock() -> impl Drop {
    ENV_LOCK.read().unwrap_or_else(PoisonError::into_inner)
}

/// 返回一个由 (变量, 值) 字节向量对组成的向量，涵盖当前进程的
/// 所有环境变量。
pub fn env() -> Env {
    unsafe {
        let _guard = env_read_lock();
        let mut environ = *environ();
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
        // 策略（抄自 glibc）：变量名与变量值之间由一个 ASCII 等号
        // '=' 分隔。由于变量名不得为空，所以允许变量名以等号开头。
        // 跳过所有格式不规范的行。
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
    // 带有 nul 字节的环境变量无法被设置，因此它们的值
    // 也总是 None
    run_with_cstr(k.as_bytes(), &|k| {
        let _guard = env_read_lock();
        let v = unsafe { libc::getenv(k.as_ptr()) } as *const libc::c_char;

        if v.is_null() {
            Ok(None)
        } else {
            // SAFETY: 由于我们持有读锁，所以执行这一行期间 `v` 不会被修改
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
            cvt(unsafe { libc::setenv(k.as_ptr(), v.as_ptr(), 1) }).map(drop)
        })
    })
}

pub unsafe fn unsetenv(n: &OsStr) -> io::Result<()> {
    run_with_cstr(n.as_bytes(), &|nbuf| {
        let _guard = ENV_LOCK.write();
        cvt(unsafe { libc::unsetenv(nbuf.as_ptr()) }).map(drop)
    })
}
