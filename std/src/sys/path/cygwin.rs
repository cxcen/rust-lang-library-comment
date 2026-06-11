use crate::ffi::OsString;
use crate::os::unix::ffi::OsStringExt;
use crate::path::{Path, PathBuf};
use crate::sys::cvt;
use crate::sys::helpers::run_path_with_cstr;
use crate::{io, ptr};

#[inline]
pub fn is_sep_byte(b: u8) -> bool {
    b == b'/' || b == b'\\'
}

/// Cygwin 总是偏好 `/` 而非 `\`，并且在调用 Win32 API 时它内部总是会把所有 `/`
/// 转换为 `\`。因此，路径 `\\?\UNC\localhost/share` 的 server 组件在 Win32 上是
/// `localhost/share`，但在 Cygwin 上是 `localhost`。
#[inline]
pub fn is_verbatim_sep(b: u8) -> bool {
    b == b'/' || b == b'\\'
}

pub use super::windows_prefix::parse_prefix;

pub const HAS_PREFIXES: bool = true;
pub const MAIN_SEP_STR: &str = "/";
pub const MAIN_SEP: char = '/';

unsafe extern "C" {
    // 文档：https://cygwin.com/cygwin-api/func-cygwin-conv-path.html
    // 源码：https://github.com/cygwin/cygwin/blob/718a15ba50e0d01c79800bd658c2477f9a603540/winsup/cygwin/path.cc#L3902
    // 安全性：
    // * 这里 `what` 应当是 `CCP_WIN_A_TO_POSIX`
    // * `from` 是以 null 结尾的 UTF-8 路径
    // * `to` 是缓冲区，缓冲区大小为 `size`。
    //
    // 将一个路径转换为绝对 POSIX 路径，无论输入是 Win32 路径还是 POSIX 路径。
    fn cygwin_conv_path(
        what: libc::c_uint,
        from: *const libc::c_char,
        to: *mut u8,
        size: libc::size_t,
    ) -> libc::ssize_t;
}

const CCP_WIN_A_TO_POSIX: libc::c_uint = 2;

/// 把一个 POSIX 路径变为绝对路径。
pub(crate) fn absolute(path: &Path) -> io::Result<PathBuf> {
    run_path_with_cstr(path, &|path| {
        let conv = CCP_WIN_A_TO_POSIX;
        let size = cvt(unsafe { cygwin_conv_path(conv, path.as_ptr(), ptr::null_mut(), 0) })?;
        // 如果成功，size 应当不为 0。
        debug_assert!(size >= 1);
        let size = size as usize;
        let mut buffer = Vec::with_capacity(size);
        cvt(unsafe { cygwin_conv_path(conv, path.as_ptr(), buffer.as_mut_ptr(), size) })?;
        unsafe {
            buffer.set_len(size - 1);
        }
        Ok(PathBuf::from(OsString::from_vec(buffer)))
    })
    .map(|path| {
        if path.prefix().is_some() {
            return path;
        }

        // 来自 unix.rs
        let mut components = path.components();
        let path_os = path.as_os_str().as_encoded_bytes();

        let mut normalized = if path_os.starts_with(b"//") && !path_os.starts_with(b"///") {
            components.next();
            PathBuf::from("//")
        } else {
            PathBuf::new()
        };
        normalized.extend(components);

        if path_os.ends_with(b"/") {
            normalized.push("");
        }

        normalized
    })
}

pub(crate) fn is_absolute(path: &Path) -> bool {
    if path.as_os_str().as_encoded_bytes().starts_with(b"\\") {
        path.has_root() && path.prefix().is_some()
    } else {
        path.has_root()
    }
}
