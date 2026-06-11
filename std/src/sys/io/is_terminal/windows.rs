use crate::ffi::c_void;
use crate::os::windows::io::{AsHandle, AsRawHandle, BorrowedHandle};
use crate::sys::c;

pub fn is_terminal(h: &impl AsHandle) -> bool {
    handle_is_console(h.as_handle())
}

fn handle_is_console(handle: BorrowedHandle<'_>) -> bool {
    // null 句柄意味着该进程没有控制台。
    if handle.as_raw_handle().is_null() {
        return false;
    }

    let mut out = 0;
    if unsafe { c::GetConsoleMode(handle.as_raw_handle(), &mut out) != 0 } {
        // 不可能出现误报。如果我们拿到了一个控制台，那我们就肯定有一个控制台。
        return true;
    }

    // 否则，我们回退到一种 msys 的 hack，看看能否检测到 pty 的存在。
    msys_tty_on(handle)
}

fn msys_tty_on(handle: BorrowedHandle<'_>) -> bool {
    // 如果该句柄不是一个管道，则提前返回。
    if unsafe { c::GetFileType(handle.as_raw_handle()) != c::FILE_TYPE_PIPE } {
        return false;
    }

    /// 对 [`FILE_NAME_INFO`] 的镜像，赋予它一个我们可以在栈上分配的固定长度
    ///
    /// [`FILE_NAME_INFO`]: https://learn.microsoft.com/en-us/windows/win32/api/winbase/ns-winbase-file_name_info
    #[repr(C)]
    #[allow(non_snake_case)]
    struct FILE_NAME_INFO {
        FileNameLength: u32,
        FileName: [u16; c::MAX_PATH as usize],
    }
    let mut name_info = FILE_NAME_INFO { FileNameLength: 0, FileName: [0; c::MAX_PATH as usize] };
    // 安全性：缓冲区长度是固定的。
    let res = unsafe {
        c::GetFileInformationByHandleEx(
            handle.as_raw_handle(),
            c::FileNameInfo,
            (&raw mut name_info) as *mut c_void,
            size_of::<FILE_NAME_INFO>() as u32,
        )
    };
    if res == 0 {
        return false;
    }

    // 使用 `get`，因为 `FileNameLength` 可能超出范围。
    let s = match name_info.FileName.get(..name_info.FileNameLength as usize / 2) {
        None => return false,
        Some(s) => s,
    };
    let name = String::from_utf16_lossy(s);
    // 只获取文件名部分。
    let name = name.rsplit('\\').next().unwrap_or(&name);
    // 这会检查文件名中是否存在 'pty'，它表明附加了一个伪终端(pseudo-terminal）。
    // 为了减少误报（例如，一个实际包含 'pty' 的文件名），我们还要求文件名
    // 以字符串 'msys-' 或 'cygwin-' 之一开头。
    let is_msys = name.starts_with("msys-") || name.starts_with("cygwin-");
    let is_pty = name.contains("-pty");
    is_msys && is_pty
}
