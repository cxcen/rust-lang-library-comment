use crate::ffi::{OsStr, OsString};
use crate::path::{Path, PathBuf};
use crate::sys::api::utf16;
use crate::sys::pal::{c, fill_utf16_buf, os2path, to_u16s};
use crate::{io, ptr};

#[cfg(test)]
mod tests;

pub use super::windows_prefix::parse_prefix;

pub const HAS_PREFIXES: bool = true;
pub const MAIN_SEP_STR: &str = "\\";
pub const MAIN_SEP: char = '\\';

/// 一个以 null 结尾的宽字符串(wide string）。
#[repr(transparent)]
pub struct WCStr([u16]);

impl WCStr {
    /// 不做检查地把一个切片转换为 WCStr。
    ///
    /// 尽管它是内存安全的，该切片也不应包含内部的 null，
    /// 因为这可能导致意料之外的截断。
    ///
    /// # 安全性(Safety）
    ///
    /// 该切片必须以一个 null 结尾。
    pub unsafe fn from_wchars_with_null_unchecked(s: &[u16]) -> &Self {
        unsafe { &*(s as *const [u16] as *const Self) }
    }

    pub fn as_ptr(&self) -> *const u16 {
        self.0.as_ptr()
    }

    pub fn count_bytes(&self) -> usize {
        self.0.len()
    }
}

#[inline]
pub fn with_native_path<T>(path: &Path, f: &dyn Fn(&WCStr) -> io::Result<T>) -> io::Result<T> {
    let path = maybe_verbatim(path)?;
    // SAFETY: maybe_verbatim 返回以 null 结尾的字符串
    let path = unsafe { WCStr::from_wchars_with_null_unchecked(&path) };
    f(path)
}

#[inline]
pub fn is_sep_byte(b: u8) -> bool {
    b == b'/' || b == b'\\'
}

#[inline]
pub fn is_verbatim_sep(b: u8) -> bool {
    b == b'\\'
}

pub fn is_verbatim(path: &[u16]) -> bool {
    path.starts_with(utf16!(r"\\?\")) || path.starts_with(utf16!(r"\??\"))
}

/// 如果 `path` 看起来像一个单独的文件名，则返回 true。
pub(crate) fn is_file_name(path: &OsStr) -> bool {
    !path.as_encoded_bytes().iter().copied().any(is_sep_byte)
}
pub(crate) fn has_trailing_slash(path: &OsStr) -> bool {
    let is_verbatim = path.as_encoded_bytes().starts_with(br"\\?\");
    let is_separator = if is_verbatim { is_verbatim_sep } else { is_sep_byte };
    if let Some(&c) = path.as_encoded_bytes().last() { is_separator(c) } else { false }
}

/// 把一个后缀追加到路径上。
///
/// 可用于在不移除已有扩展名的情况下追加一个扩展名。
pub(crate) fn append_suffix(path: PathBuf, suffix: &OsStr) -> PathBuf {
    let mut path = OsString::from(path);
    path.push(suffix);
    path.into()
}

/// 返回一个 UTF-16 编码的路径，它能够绕过传统的 `MAX_PATH` 限制。
///
/// 该路径可能带有也可能不带有 verbatim 前缀。
pub(crate) fn maybe_verbatim(path: &Path) -> io::Result<Vec<u16>> {
    let path = to_u16s(path)?;
    get_long_path(path, true)
}

/// 获取一个能够绕过路径长度限制的、规范化的绝对路径。
///
/// 把 prefer_verbatim 设为 true 表示更强烈地偏好 verbatim 路径，即使并非严格必要。
/// 这能让 Windows API 避免重复我们已经做过的工作。然而，如果该路径可能被返回给用户，
/// 或被传递给其他应用程序，那么在可能的情况下最好使用非 verbatim 路径。非 verbatim
/// 路径更容易被用户理解，也能被更多软件处理。
pub(crate) fn get_long_path(mut path: Vec<u16>, prefer_verbatim: bool) -> io::Result<Vec<u16>> {
    // 通常 MAX_PATH 是 260 个 UTF-16 码元（包含 NULL）。
    // 然而，对于诸如 CreateDirectory[1] 这样的 API，限制是 248。
    //
    // [1]: https://docs.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-createdirectorya#parameters
    const LEGACY_MAX_PATH: usize = 248;
    // UTF-16 编码的码点，用于解析和构建 UTF-16 路径。
    // 它们全都在 ASCII 范围内，所以可以直接转换(cast）为 `u16`。
    const SEP: u16 = b'\\' as _;
    const ALT_SEP: u16 = b'/' as _;
    const QUERY: u16 = b'?' as _;
    const COLON: u16 = b':' as _;
    const DOT: u16 = b'.' as _;
    const U: u16 = b'U' as _;
    const N: u16 = b'N' as _;
    const C: u16 = b'C' as _;

    // \\?\
    const VERBATIM_PREFIX: &[u16] = &[SEP, SEP, QUERY, SEP];
    // \??\
    const NT_PREFIX: &[u16] = &[SEP, QUERY, QUERY, SEP];
    // \\?\UNC\
    const UNC_PREFIX: &[u16] = &[SEP, SEP, QUERY, SEP, U, N, C, SEP];

    if path.starts_with(VERBATIM_PREFIX) || path.starts_with(NT_PREFIX) || path == [0] {
        // 对于已经是 verbatim 或为空的路径，提前返回。
        return Ok(path);
    } else if path.len() < LEGACY_MAX_PATH {
        // 如果一个绝对路径少于 260 个 UTF-16 码元，则提前返回。
        // 这是一项优化，用于避免不必要地调用 `GetFullPathNameW`。
        match path.as_slice() {
            // 以 `D:`、`D:\`、`D:/` 等开头。
            // 如果路径以 `\` 或 `/` 开头，则不匹配。
            [drive, COLON, 0] | [drive, COLON, SEP | ALT_SEP, ..]
                if *drive != SEP && *drive != ALT_SEP =>
            {
                return Ok(path);
            }
            // 以 `\\`、`//` 等开头
            [SEP | ALT_SEP, SEP | ALT_SEP, ..] => return Ok(path),
            _ => {}
        }
    }

    // 首先，使用 `GetFullPathNameW` 获取绝对路径。
    // https://docs.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getfullpathnamew
    let lpfilename = path.as_ptr();
    fill_utf16_buf(
        // SAFETY: `fill_utf16_buf` 确保 `buffer` 和 `size` 是有效的。
        // `lpfilename` 是指向一个以 null 结尾的字符串的指针，该字符串在
        // `GetFullPathNameW` 成功返回之前都不会失效。
        |buffer, size| unsafe { c::GetFullPathNameW(lpfilename, size, buffer, ptr::null_mut()) },
        |mut absolute| {
            path.clear();

            // 仅在需要时才添加前缀。
            if prefer_verbatim || absolute.len() + 1 >= LEGACY_MAX_PATH {
                // 其次，添加 verbatim 前缀。在这里这样做更容易，因为我们知道此时
                // 路径已经是绝对的并且已完全规范化（例如 `/` 已被改为 `\`）。
                let prefix = match absolute {
                    // C:\ => \\?\C:\
                    [_, COLON, SEP, ..] => VERBATIM_PREFIX,
                    // \\.\ => \\?\
                    [SEP, SEP, DOT, SEP, ..] => {
                        absolute = &absolute[4..];
                        VERBATIM_PREFIX
                    }
                    // 保持 \\?\ 和 \??\ 原样不变。
                    [SEP, SEP, QUERY, SEP, ..] | [SEP, QUERY, QUERY, SEP, ..] => &[],
                    // \\ => \\?\UNC\
                    [SEP, SEP, ..] => {
                        absolute = &absolute[2..];
                        UNC_PREFIX
                    }
                    // 其他任何情况我们都不去动它。
                    _ => &[],
                };

                path.reserve_exact(prefix.len() + absolute.len() + 1);
                path.extend_from_slice(prefix);
            } else {
                path.reserve_exact(absolute.len() + 1);
            }
            path.extend_from_slice(absolute);
            path.push(0);
        },
    )?;
    Ok(path)
}

/// 把一个 Windows 路径变为绝对路径。
pub(crate) fn absolute(path: &Path) -> io::Result<PathBuf> {
    let path = path.as_os_str();
    let prefix = parse_prefix(path);
    // verbatim 路径不应被修改。
    if prefix.map(|x| x.is_verbatim()).unwrap_or(false) {
        // 为了保持一致性，verbatim 路径中的 NUL 会被拒绝。
        if path.as_encoded_bytes().contains(&0) {
            return Err(io::const_error!(
                io::ErrorKind::InvalidInput,
                "strings passed to WinAPI cannot contain NULs",
            ));
        }
        return Ok(path.to_owned().into());
    }

    let path = to_u16s(path)?;
    let lpfilename = path.as_ptr();
    fill_utf16_buf(
        // SAFETY: `fill_utf16_buf` 确保 `buffer` 和 `size` 是有效的。
        // `lpfilename` 是指向一个以 null 结尾的字符串的指针，该字符串在
        // `GetFullPathNameW` 成功返回之前都不会失效。
        |buffer, size| unsafe { c::GetFullPathNameW(lpfilename, size, buffer, ptr::null_mut()) },
        os2path,
    )
}

pub(crate) fn is_absolute(path: &Path) -> bool {
    path.has_root() && path.prefix().is_some()
}

/// 测试该路径是否为绝对路径、是否完全限定(fully qualified），以及经过 Windows API 处理后是否保持不变。
///
/// 例如：
///
/// - `C:\path\to\file` 会返回 true。
/// - `C:\path\to\nul` 返回 false，因为 Windows API 会把它转换为 \\.\NUL
/// - `C:\path\to\..\file` 返回 false，因为它会被解析为 `C:\path\file`。
///
/// 这是一个有用的性质，因为它意味着只需更改前缀，该路径就可以在普通路径和 verbatim
/// 路径之间相互转换。
pub(crate) fn is_absolute_exact(path: &[u16]) -> bool {
    // 这是通过检查把路径传给 GetFullPathNameW 后路径是否以任何方式发生改变来实现的。

    // Windows 路径的长度被限制在 i16::MAX，
    // 尽管这里的 API 接受一个 u32 作为长度。
    if path.is_empty() || path.len() > u32::MAX as usize || path.last() != Some(&0) {
        return false;
    }
    // `GetFullPathNameW` 返回的路径长度必须与给定的路径相同，
    // 否则它们就不相等。
    let buffer_len = path.len();
    let mut new_path = Vec::with_capacity(buffer_len);
    let result = unsafe {
        c::GetFullPathNameW(
            path.as_ptr(),
            new_path.capacity() as u32,
            new_path.as_mut_ptr(),
            crate::ptr::null_mut(),
        )
    };
    // 注意：如果非零，返回的结果是缓冲区的长度，不含 null 终止符
    if result == 0 || result as usize != buffer_len - 1 {
        false
    } else {
        // SAFETY: `GetFullPathNameW` 初始化了 `result` 个字节，且不超过 `nBufferLength - 1`（容量）。
        unsafe {
            new_path.set_len((result as usize) + 1);
        }
        path == &new_path
    }
}
