use crate::ffi::OsStr;
use crate::path::{Path, PathBuf, Prefix};
use crate::{env, io};

#[inline]
pub fn is_sep_byte(b: u8) -> bool {
    b == b'/'
}

#[inline]
pub fn is_verbatim_sep(b: u8) -> bool {
    b == b'/'
}

#[inline]
pub fn parse_prefix(_: &OsStr) -> Option<Prefix<'_>> {
    None
}

pub const HAS_PREFIXES: bool = false;
pub const MAIN_SEP_STR: &str = "/";
pub const MAIN_SEP: char = '/';

/// 在不改变语义的前提下，把一个 POSIX 路径变为绝对路径。
pub(crate) fn absolute(path: &Path) -> io::Result<PathBuf> {
    // 这基本上是对收集 `Path::components` 的一层包装，仅在与 POSIX 规范
    // 冲突的地方做了例外处理。
    // 参见 4.13 Pathname Resolution, IEEE Std 1003.1-2017
    // https://pubs.opengroup.org/onlinepubs/9699919799/basedefs/V1_chap04.html#tag_04_13

    // 获取各个组件，如果存在多余的前导 "." 组件则将其跳过。
    let mut components = path.strip_prefix(".").unwrap_or(path).components();
    let path_os = path.as_os_str().as_encoded_bytes();

    let mut normalized = if path.is_absolute() {
        // 「如果一个路径名以两个连续的 <slash> 字符开头，则跟在这些前导
        // <slash> 字符之后的第一个组件可以以实现自定义的方式来解释，不过
        // 超过两个的前导 <slash> 字符应当被当作单个 <slash> 字符来处理。」
        if path_os.starts_with(b"//") && !path_os.starts_with(b"///") {
            components.next();
            PathBuf::from("//")
        } else {
            PathBuf::new()
        }
    } else {
        env::current_dir()?
    };
    normalized.extend(components);

    // 「使用路径名解析的接口可以指定额外的约束：当一个并不命名某个已存在
    // 目录的路径名包含至少一个非 <slash> 字符、并且包含一个或多个尾部
    // <slash> 字符时。」
    // 如果「在路径名解析过程中遇到一个符号链接」，那么尾部的 <slash>
    // 也是有意义的。
    if path_os.ends_with(b"/") {
        normalized.push("");
    }

    Ok(normalized)
}

pub(crate) fn is_absolute(path: &Path) -> bool {
    if cfg!(any(unix, target_os = "hermit", target_os = "wasi", target_os = "motor")) {
        path.has_root()
    } else {
        path.has_root() && path.prefix().is_some()
    }
}
