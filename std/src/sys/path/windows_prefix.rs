//! 解析 Windows 前缀，同时供 Windows 和 Cygwin 使用。

use super::{is_sep_byte, is_verbatim_sep};
use crate::ffi::OsStr;
use crate::path::Prefix;

struct PrefixParser<'a, const LEN: usize> {
    path: &'a OsStr,
    prefix: [u8; LEN],
}

impl<'a, const LEN: usize> PrefixParser<'a, LEN> {
    #[inline]
    fn get_prefix(path: &OsStr) -> [u8; LEN] {
        let mut prefix = [0; LEN];
        // SAFETY: 只有 ASCII 字符会被修改。
        for (i, &ch) in path.as_encoded_bytes().iter().take(LEN).enumerate() {
            prefix[i] = if ch == b'/' { b'\\' } else { ch };
        }
        prefix
    }

    fn new(path: &'a OsStr) -> Self {
        Self { path, prefix: Self::get_prefix(path) }
    }

    fn as_slice(&self) -> PrefixParserSlice<'a, '_> {
        PrefixParserSlice {
            path: self.path,
            prefix: &self.prefix[..LEN.min(self.path.len())],
            index: 0,
        }
    }
}

struct PrefixParserSlice<'a, 'b> {
    path: &'a OsStr,
    prefix: &'b [u8],
    index: usize,
}

impl<'a> PrefixParserSlice<'a, '_> {
    fn strip_prefix(&self, prefix: &str) -> Option<Self> {
        self.prefix[self.index..]
            .starts_with(prefix.as_bytes())
            .then_some(Self { index: self.index + prefix.len(), ..*self })
    }

    fn prefix_bytes(&self) -> &'a [u8] {
        &self.path.as_encoded_bytes()[..self.index]
    }

    fn finish(self) -> &'a OsStr {
        // SAFETY: 这里的不安全性源于在 &OsStr 和 &[u8] 之间来回转换。这样做是安全的，
        // 因为 (1) 我们只查看编码中的 ASCII 内容，并且 (2) 新的 &OsStr 值只从已有
        // &OsStr 值的、以 ASCII 为边界的切片中产生。
        unsafe { OsStr::from_encoded_bytes_unchecked(&self.path.as_encoded_bytes()[self.index..]) }
    }
}

pub fn parse_prefix(path: &OsStr) -> Option<Prefix<'_>> {
    use Prefix::{DeviceNS, Disk, UNC, Verbatim, VerbatimDisk, VerbatimUNC};

    let parser = PrefixParser::<8>::new(path);
    let parser = parser.as_slice();
    if let Some(parser) = parser.strip_prefix(r"\\") {
        // \\

        // 这是一个 POSIX 路径。
        if cfg!(target_os = "cygwin") && !path.as_encoded_bytes().iter().any(|&x| x == b'\\') {
            return None;
        }

        // 当 verbatim 路径使用不同的分隔符时，其含义可能会改变。
        if let Some(parser) = parser.strip_prefix(r"?\")
            // Cygwin 允许在 verbatim 路径中使用 `/`。
            && (cfg!(target_os = "cygwin") || !parser.prefix_bytes().iter().any(|&x| x == b'/'))
        {
            // \\?\
            if let Some(parser) = parser.strip_prefix(r"UNC\") {
                // \\?\UNC\server\share

                let path = parser.finish();
                let (server, path) = parse_next_component(path, true);
                let (share, _) = parse_next_component(path, true);

                Some(VerbatimUNC(server, share))
            } else {
                let path = parser.finish();

                // 在 verbatim 路径中只识别精确的盘符前缀
                if let Some(drive) = parse_drive_exact(path) {
                    // \\?\C:
                    Some(VerbatimDisk(drive))
                } else {
                    // \\?\prefix
                    let (prefix, _) = parse_next_component(path, true);
                    Some(Verbatim(prefix))
                }
            }
        } else if let Some(parser) = parser.strip_prefix(r".\") {
            // \\.\COM42
            let path = parser.finish();
            let (prefix, _) = parse_next_component(path, false);
            Some(DeviceNS(prefix))
        } else {
            let path = parser.finish();
            let (server, path) = parse_next_component(path, false);
            let (share, _) = parse_next_component(path, false);

            if !server.is_empty() && !share.is_empty() {
                // \\server\share
                Some(UNC(server, share))
            } else {
                // 没有识别出以 "\\" 开头的有效前缀
                None
            }
        }
    } else {
        // 如果它带有像 `C:` 这样的盘符，那么它就是一个磁盘。
        // 否则就没有前缀。
        Some(Disk(parse_drive(path)?))
    }
}

// 解析盘符前缀，例如 "C:" 和 "C:\whatever"
fn parse_drive(path: &OsStr) -> Option<u8> {
    // 在大多数 DOS 系统中，不可能有超过 26 个盘符。
    // 参见 <https://en.wikipedia.org/wiki/Drive_letter_assignment#Common_assignments>。
    fn is_valid_drive_letter(drive: &u8) -> bool {
        drive.is_ascii_alphabetic()
    }

    match path.as_encoded_bytes() {
        [drive, b':', ..] if is_valid_drive_letter(drive) => Some(drive.to_ascii_uppercase()),
        _ => None,
    }
}

// 精确地解析盘符前缀，例如 "C:"
fn parse_drive_exact(path: &OsStr) -> Option<u8> {
    // 只解析两个字节：盘符字母和盘符分隔符
    if path.as_encoded_bytes().get(2).map(|&x| is_sep_byte(x)).unwrap_or(true) {
        parse_drive(path)
    } else {
        None
    }
}

// 解析下一个路径组件。
//
// 返回下一个组件，以及路径中剔除该组件和分隔符之后剩下的部分。
// 如果 `verbatim` 为 true，则在 Windows 上不把 `/` 识别为分隔符字符。
pub(crate) fn parse_next_component(path: &OsStr, verbatim: bool) -> (&OsStr, &OsStr) {
    let separator = if verbatim { is_verbatim_sep } else { is_sep_byte };

    match path.as_encoded_bytes().iter().position(|&x| separator(x)) {
        Some(separator_start) => {
            let separator_end = separator_start + 1;

            let component = &path.as_encoded_bytes()[..separator_start];

            // panic 安全
            // `separator_end` 的最大值是 `bytes.len()`，而 `bytes[bytes.len()..]` 是一个有效的索引。
            let path = &path.as_encoded_bytes()[separator_end..];

            // SAFETY: `path` 是一个有效的 wtf8 编码切片，并且每一个分隔符（'/'、'\'）
            // 都以单个字节编码，因此 `bytes[separator_start]` 和
            // `bytes[separator_end]` 必定是码点边界，从而
            // `bytes[..separator_start]` 和 `bytes[separator_end..]` 都是有效的 wtf8 切片。
            unsafe {
                (
                    OsStr::from_encoded_bytes_unchecked(component),
                    OsStr::from_encoded_bytes_unchecked(path),
                )
            }
        }
        None => (path, OsStr::new("")),
    }
}
