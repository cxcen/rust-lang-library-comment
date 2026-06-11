use core::slice::memchr;

pub use super::common::Env;
use crate::ffi::{CStr, OsStr, OsString};
use crate::io;
use crate::os::wasi::prelude::*;
use crate::sys::helpers::run_with_cstr;
use crate::sys::pal::os::{cvt, libc};

cfg_select! {
    target_feature = "atomics" => {
        // 多线程场景下，对环境的访问必须由锁保护。
        use crate::sync::{PoisonError, RwLock};
        static ENV_LOCK: RwLock<()> = RwLock::new(());
        pub fn env_read_lock() -> impl Drop {
            ENV_LOCK.read().unwrap_or_else(PoisonError::into_inner)
        }
        pub fn env_write_lock() -> impl Drop {
            ENV_LOCK.write().unwrap_or_else(PoisonError::into_inner)
        }
    }
    _ => {
        // 单线程时无需加锁。
        pub fn env_read_lock() -> impl Drop {
            Box::new(())
        }
        pub fn env_write_lock() -> impl Drop {
            Box::new(())
        }
    }
}

pub fn env() -> Env {
    unsafe {
        let _guard = env_read_lock();

        // 这里使用 `__wasilibc_get_environ` 而不是 `environ`，这样我们就
        // 不要求 wasi-libc 提前（eagerly）初始化环境变量。
        let mut environ = libc::__wasilibc_get_environ();

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

    // 参见 src/libstd/sys/pal/unix/os.rs，与那里相同
    fn parse(input: &[u8]) -> Option<(OsString, OsString)> {
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
        run_with_cstr(v.as_bytes(), &|v| unsafe {
            let _guard = env_write_lock();
            cvt(libc::setenv(k.as_ptr(), v.as_ptr(), 1)).map(drop)
        })
    })
}

pub unsafe fn unsetenv(n: &OsStr) -> io::Result<()> {
    run_with_cstr(n.as_bytes(), &|nbuf| unsafe {
        let _guard = env_write_lock();
        cvt(libc::unsetenv(nbuf.as_ptr())).map(drop)
    })
}
