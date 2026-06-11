use core::slice::memchr;

pub use super::common::Env;
use crate::collections::HashMap;
use crate::ffi::{CStr, OsStr, OsString, c_char};
use crate::io;
use crate::os::hermit::ffi::OsStringExt;
use crate::sync::Mutex;

static ENV: Mutex<Option<HashMap<OsString, OsString>>> = Mutex::new(None);

pub fn init(env: *const *const c_char) {
    let mut guard = ENV.lock().unwrap();
    let map = guard.insert(HashMap::new());

    if env.is_null() {
        return;
    }

    unsafe {
        let mut environ = env;
        while !(*environ).is_null() {
            if let Some((key, value)) = parse(CStr::from_ptr(*environ).to_bytes()) {
                map.insert(key, value);
            }
            environ = environ.add(1);
        }
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

/// 返回一个由 (变量, 值) 字节向量对组成的向量，涵盖当前进程的
/// 所有环境变量。
pub fn env() -> Env {
    let guard = ENV.lock().unwrap();
    let env = guard.as_ref().unwrap();

    let result = env.iter().map(|(key, value)| (key.clone(), value.clone())).collect();

    Env::new(result)
}

pub fn getenv(k: &OsStr) -> Option<OsString> {
    ENV.lock().unwrap().as_ref().unwrap().get(k).cloned()
}

pub unsafe fn setenv(k: &OsStr, v: &OsStr) -> io::Result<()> {
    let (k, v) = (k.to_owned(), v.to_owned());
    ENV.lock().unwrap().as_mut().unwrap().insert(k, v);
    Ok(())
}

pub unsafe fn unsetenv(k: &OsStr) -> io::Result<()> {
    ENV.lock().unwrap().as_mut().unwrap().remove(k);
    Ok(())
}
