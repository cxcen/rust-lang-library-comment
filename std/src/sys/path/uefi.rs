#![forbid(unsafe_op_in_unsafe_fn)]
use crate::ffi::OsStr;
use crate::io;
use crate::path::{Path, PathBuf, Prefix};
use crate::sys::pal::helpers;
use crate::sys::unsupported_err;

const FORWARD_SLASH: u8 = b'/';
const COLON: u8 = b':';

#[inline]
pub fn is_sep_byte(b: u8) -> bool {
    b == b'\\'
}

#[inline]
pub fn is_verbatim_sep(b: u8) -> bool {
    b == b'\\'
}

pub fn parse_prefix(_: &OsStr) -> Option<Prefix<'_>> {
    None
}

pub const HAS_PREFIXES: bool = true;
pub const MAIN_SEP_STR: &str = "\\";
pub const MAIN_SEP: char = '\\';

/// UEFI 路径可以分为 4 种类型：
///
/// 1. 绝对 Shell 路径(Absolute Shell Path）：使用 shell 映射（例如：`FS0:`）。如果 UEFI shell 不存在则此类型不存在。
///    它可以通过 `:` 来识别。
///    例如：FS0:\abc\run.efi
///
/// 2. 绝对设备路径(Absolute Device Path）：这正是我们想要的
///    它可以通过 `/` 来识别。
///    例如：PciRoot(0x0)/Pci(0x1,0x1)/Ata(Secondary,Slave,0x0)/\abc\run.efi
///
/// 3：相对根路径(Relative root）：相对于当前卷的路径。
///    它会以 `\` 开头。
///    例如：\abc\run.efi
///
/// 4：相对路径(Relative）
///    例如：run.efi
///
/// 该算法大体上取自 edk2 的 UEFI shell 实现，并且相当简单。按顺序检查路径类型。
///
/// 绝对 Shell 路径中的卷映射部分（不包括路径的其余部分）可以使用
/// `EFI_SHELL->GetDevicePathFromMap` 转换为设备路径协议(Device Path Protocol）形式。
/// 路径的其余部分（相对根路径）可以直接附加到剩下的路径上。
///
/// 对于相对根路径，我们获取当前卷（以 Shell 映射或设备路径协议形式），并把它与
/// 相对根路径连接起来。然后我们递归调用本函数，以便在存在 Shell 映射时解析它。
///
/// 对于相对路径，我们使用当前工作目录来构造新路径，并递归调用本函数，
/// 以便在存在 Shell 映射时解析它。
///
/// 最后，我们得到第 2 种形式，即绝对设备路径，它可以在普通的 UEFI API（例如文件、
/// 进程等）中使用。
/// 例如：PciRoot(0x0)/Pci(0x1,0x1)/Ata(Secondary,Slave,0x0)/\abc\run.efi
pub(crate) fn absolute(path: &Path) -> io::Result<PathBuf> {
    // 绝对 Shell 路径
    if path.as_os_str().as_encoded_bytes().contains(&COLON) {
        let mut path_components = path.components();
        // 由于 path 非空，它至少有一个 Component
        let prefix = path_components.next().unwrap();

        let dev_path = helpers::get_device_path_from_map(prefix.as_ref())?;
        let mut dev_path_text = dev_path.to_text().map_err(|_| unsupported_err())?;

        // UEFI Shell 似乎不会以 `/` 结束设备路径
        if *dev_path_text.as_encoded_bytes().last().unwrap() != FORWARD_SLASH {
            dev_path_text.push("/");
        }

        let mut ans = PathBuf::from(dev_path_text);
        ans.push(path_components);

        return Ok(ans);
    }

    // 绝对设备路径
    if path.as_os_str().as_encoded_bytes().contains(&FORWARD_SLASH) {
        return Ok(path.to_path_buf());
    }

    // cur_dir() 总是会返回点什么
    let cur_dir = crate::env::current_dir().unwrap();
    let mut path_components = path.components();

    // 相对根路径
    if path_components.next().unwrap() == crate::path::Component::RootDir {
        let mut ans = PathBuf::new();
        ans.push(cur_dir.components().next().unwrap());
        ans.push(path_components);
        return absolute(&ans);
    }

    absolute(&cur_dir.join(path))
}

pub(crate) fn is_absolute(path: &Path) -> bool {
    let temp = path.as_os_str().as_encoded_bytes();
    temp.contains(&COLON) || temp.contains(&FORWARD_SLASH)
}
