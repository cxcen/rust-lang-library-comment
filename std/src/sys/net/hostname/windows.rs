use crate::ffi::OsString;
use crate::io::Result;
use crate::mem::MaybeUninit;
use crate::os::windows::ffi::OsStringExt;
use crate::sys::pal::c;
use crate::sys::pal::winsock::{self, cvt};

pub fn hostname() -> Result<OsString> {
    winsock::startup();

    // GetHostNameW 的文档说明缓冲区大小为 256 时总是足够的。
    let mut buffer = [const { MaybeUninit::<u16>::uninit() }; 256];
    // SAFETY: 这些参数指定了一块有效且可写的内存区域。
    cvt(unsafe { c::GetHostNameW(buffer.as_mut_ptr().cast(), buffer.len() as i32) })?;
    // 此处使用 `lstrlenW`，因为它不要求 nul 终止符之后的字节已被初始化。
    // SAFETY: 如果 `GetHostNameW` 成功返回，则名称是以 nul 结尾的。
    let len = unsafe { c::lstrlenW(buffer.as_ptr().cast()) };
    // SAFETY: 名称的长度为 `len`，因此 `GetHostNameW` 已初始化了 `len` 个字节。
    let name = unsafe { buffer[..len as usize].assume_init_ref() };
    Ok(OsString::from_wide(name))
}
