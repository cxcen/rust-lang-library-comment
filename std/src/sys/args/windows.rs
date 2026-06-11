//! Windows 的命令行只是一个字符串
//! <https://docs.microsoft.com/en-us/archive/blogs/larryosterman/the-windows-command-line-is-just-a-string>
//!
//! 本模块实现了把那个字符串解析为参数列表所必需的解析逻辑。

#[cfg(test)]
mod tests;

pub use super::common::Args;
use crate::ffi::{OsStr, OsString};
use crate::num::NonZero;
use crate::os::windows::prelude::*;
use crate::path::{Path, PathBuf};
use crate::sys::helpers::WStrUnits;
use crate::sys::pal::os::current_exe;
use crate::sys::pal::{ensure_no_nuls, fill_utf16_buf};
use crate::sys::path::get_long_path;
use crate::sys::{AsInner, c, to_u16s};
use crate::{io, iter, ptr};

pub fn args() -> Args {
    // SAFETY: `GetCommandLineW` 返回一个指向以 null 结尾的 UTF-16
    // 字符串的指针，因此 `WStrUnits` 使用它是安全的。
    unsafe {
        let lp_cmd_line = c::GetCommandLineW();
        let parsed_args_list = parse_lp_cmd_line(WStrUnits::new(lp_cmd_line), || {
            current_exe().map(PathBuf::into_os_string).unwrap_or_else(|_| OsString::new())
        });

        Args::new(parsed_args_list)
    }
}

/// 实现 Windows 命令行参数解析算法。
///
/// 微软关于 Windows CLI 参数格式的文档见
/// <https://docs.microsoft.com/en-us/cpp/cpp/main-function-command-line-args?view=msvc-160#parsing-c-command-line-arguments>
///
/// 更深入的讲解在此：
/// <https://daviddeley.com/autohotkey/parameters/parameters.htm#WIN>
///
/// Windows 在 shell32.dll 中包含一个用于命令行解析的函数。
/// 然而出于两个原因我们没有使用它：
///
/// 1. 与该 DLL 链接会使进程被注册为 GUI 应用程序。
/// 即便不绘制任何窗口，GUI 应用程序也会增加一大堆开销。参见
/// <https://randomascii.wordpress.com/2018/12/03/a-not-called-function-can-cause-a-5x-slowdown/>。
///
/// 2. 它并不遵循上面前两个链接中所述的现代 C/C++ argv 规则。
///
/// 我们使用一个内容详尽的测试套件验证了本函数与 C/C++ 解析规则的等价性，
/// 该套件见
/// <https://github.com/ChrisDenton/winarg/tree/std>。
fn parse_lp_cmd_line<'a, F: Fn() -> OsString>(
    lp_cmd_line: Option<WStrUnits<'a>>,
    exe_name: F,
) -> Vec<OsString> {
    const BACKSLASH: NonZero<u16> = NonZero::new(b'\\' as u16).unwrap();
    const QUOTE: NonZero<u16> = NonZero::new(b'"' as u16).unwrap();
    const TAB: NonZero<u16> = NonZero::new(b'\t' as u16).unwrap();
    const SPACE: NonZero<u16> = NonZero::new(b' ' as u16).unwrap();

    let mut ret_val = Vec::new();
    // 如果命令行指针为 null，或它指向一个空字符串，则
    // 返回可执行文件的名字作为 argv[0]。
    if lp_cmd_line.as_ref().and_then(|cmd| cmd.peek()).is_none() {
        ret_val.push(exe_name());
        return ret_val;
    }
    let mut code_units = lp_cmd_line.unwrap();

    // 开头的可执行文件名是个特殊情况。
    let mut in_quotes = false;
    let mut cur = Vec::new();
    for w in &mut code_units {
        match w {
            // 引号标记总是会切换 `in_quotes`，无论如何，
            // 因为解析可执行文件名时没有转义字符。
            QUOTE => in_quotes = !in_quotes,
            // 如果不在 `in_quotes` 中，则空白结束 argv[0]。
            SPACE | TAB if !in_quotes => break,
            // 在其他所有情况下，该码元都按字面取用。
            _ => cur.push(w.get()),
        }
    }
    // 跳过空白。
    code_units.advance_while(|w| w == SPACE || w == TAB);
    ret_val.push(OsString::from_wide(&cur));

    // 按照以下规则解析参数：
    // * 除空格、制表符、引号和反斜杠外，所有码元都按字面取用。
    // * 当不在 `in_quotes` 中时，空格和制表符分隔参数。连续的空格和制表符
    // 被视为单个分隔符。
    // * `in_quotes` 中的空格或制表符按字面取用。
    // * 引号会切换 `in_quotes` 模式，除非它被转义。被转义的引号按字面取用。
    // * 如果引号前面有奇数个反斜杠，则该引号可被转义。
    // * 如果任意数量的反斜杠后紧跟一个引号，那么反斜杠的数量
    // 减半（向下取整）。
    // * 后面没有跟引号的反斜杠全部按字面取用。
    // * 如果处于 `in_quotes` 中，引号也可以用另一个引号来转义
    //（即两个连续的引号变成一个字面引号）。
    let mut cur = Vec::new();
    let mut in_quotes = false;
    while let Some(w) = code_units.next() {
        match w {
            // 如果不在 `in_quotes` 中，空格或制表符结束该参数。
            SPACE | TAB if !in_quotes => {
                ret_val.push(OsString::from_wide(&cur[..]));
                cur.truncate(0);

                // 跳过空白。
                code_units.advance_while(|w| w == SPACE || w == TAB);
            }
            // 反斜杠可以转义引号或反斜杠，但仅当连续的反斜杠后面跟着一个引号时才行。
            BACKSLASH => {
                let backslash_count = code_units.advance_while(|w| w == BACKSLASH) + 1;
                if code_units.peek() == Some(QUOTE) {
                    cur.extend(iter::repeat(BACKSLASH.get()).take(backslash_count / 2));
                    // 如果反斜杠的数量为奇数，则该引号被转义。
                    if backslash_count % 2 == 1 {
                        code_units.next();
                        cur.push(QUOTE.get());
                    }
                } else {
                    // 如果末尾没有引号，则不存在转义。
                    cur.extend(iter::repeat(BACKSLASH.get()).take(backslash_count));
                }
            }
            // 如果处于 `in_quotes` 中且未被反斜杠转义（见上），那么引号要么
            // 取消 `in_quote`，要么被另一个引号转义。
            QUOTE if in_quotes => match code_units.peek() {
                // `in_quotes` 中两个连续的引号产生一个字面引号。
                Some(QUOTE) => {
                    cur.push(QUOTE.get());
                    code_units.next();
                }
                // 否则取消设置 `in_quotes`。
                Some(_) => in_quotes = false,
                // 命令行结束。
                // 即便 `cur` 为空也要 push，做法是在 `in_quotes` 仍被设置时 break。
                None => break,
            },
            // 如果不在 `in_quotes` 中且未被 BACKSLASH 转义（见上），那么引号设置 `in_quote`。
            QUOTE => in_quotes = true,
            // 其他所有内容总是按字面取用。
            _ => cur.push(w.get()),
        }
    }
    // 推入最后一个参数（如果有的话）。
    if !cur.is_empty() || in_quotes {
        ret_val.push(OsString::from_wide(&cur[..]));
    }
    ret_val
}

#[derive(Debug)]
pub(crate) enum Arg {
    /// 添加引号（如果需要）
    Regular(OsString),
    /// 不加引号，按原样追加字符串
    Raw(OsString),
}

enum Quote {
    // 每个参数都加引号
    Always,
    // 含空白的参数和空参数才加引号
    Auto,
    // 不做任何改动直接追加参数（#29494）
    Never,
}

pub(crate) fn append_arg(cmd: &mut Vec<u16>, arg: &Arg, force_quotes: bool) -> io::Result<()> {
    let (arg, quote) = match arg {
        Arg::Regular(arg) => (arg, if force_quotes { Quote::Always } else { Quote::Auto }),
        Arg::Raw(arg) => (arg, Quote::Never),
    };

    // 如果某个参数有 0 个字符，那么我们需要给它加引号，以确保
    // 它确实能通过命令行被传递过去，否则在另一端解析时
    // 它会被整个丢弃。
    ensure_no_nuls(arg)?;
    let arg_bytes = arg.as_encoded_bytes();
    let (quote, escape) = match quote {
        Quote::Always => (true, true),
        Quote::Auto => {
            (arg_bytes.iter().any(|c| *c == b' ' || *c == b'\t') || arg_bytes.is_empty(), true)
        }
        Quote::Never => (false, false),
    };
    if quote {
        cmd.push('"' as u16);
    }

    let mut backslashes: usize = 0;
    for x in arg.encode_wide() {
        if escape {
            if x == '\\' as u16 {
                backslashes += 1;
            } else {
                if x == '"' as u16 {
                    // 在内部的 '"' 之前加 n+1 个反斜杠，使其总数达到 2n+1。
                    cmd.extend((0..=backslashes).map(|_| '\\' as u16));
                }
                backslashes = 0;
            }
        }
        cmd.push(x);
    }

    if quote {
        // 在结尾的 '"' 之前加 n 个反斜杠，使其总数达到 2n。
        cmd.extend((0..backslashes).map(|_| '\\' as u16));
        cmd.push('"' as u16);
    }
    Ok(())
}

fn append_bat_arg(cmd: &mut Vec<u16>, arg: &OsStr, mut quote: bool) -> io::Result<()> {
    ensure_no_nuls(arg)?;
    // 如果某个参数有 0 个字符，那么我们需要给它加引号，以确保
    // 它确实能通过命令行被传递过去，否则在另一端解析时
    // 它会被整个丢弃。
    //
    // 如果参数以 `\` 结尾，我们也需要给它加引号，以防范诸如
    // `"%~2"` 这样的 bat 用法（即强制给参数加引号），否则
    // 末尾的斜杠会转义掉收尾的引号。
    if arg.is_empty() || arg.as_encoded_bytes().last() == Some(&b'\\') {
        quote = true;
    }
    for cp in arg.as_inner().inner.code_points() {
        if let Some(cp) = cp.to_char() {
            // 与其试图找出每一个必须加引号的 ascii 符号，
            // 我们不如假定所有 ascii 符号都必须加引号，除非已知它们是安全的。
            // 为保险起见，我们也给 Unicode 控制块（control blocks）加引号。
            // 注意：只要参数在其他方面没有被加引号，一个未加引号的 `\` 是没问题的。
            static UNQUOTED: &str = r"#$*+-./:?@\_";
            let ascii_needs_quotes =
                cp.is_ascii() && !(cp.is_ascii_alphanumeric() || UNQUOTED.contains(cp));
            if ascii_needs_quotes || cp.is_control() {
                quote = true;
            }
        }
    }

    if quote {
        cmd.push('"' as u16);
    }
    // 遍历字符串，仅当 `\` 后面跟着 `"` 时才转义它。
    // 并通过将 `"` 翻倍来转义它们。
    let mut backslashes: usize = 0;
    for x in arg.encode_wide() {
        if x == '\\' as u16 {
            backslashes += 1;
        } else {
            if x == '"' as u16 {
                // 在内部的 `"` 之前加 n 个反斜杠，使其总数达到 2n。
                cmd.extend((0..backslashes).map(|_| '\\' as u16));
                // 追加一个额外的双引号充当转义。
                cmd.push(b'"' as u16)
            } else if x == '%' as u16 || x == '\r' as u16 {
                // yt-dlp 的 hack：把 `%` 替换为 `%%cd:~,%`，以阻止 %VAR% 被展开为环境变量。
                //
                // # 解释
                //
                // cmd 支持使用以下语法从变量中提取子串：
                //     %variable:~start_index,end_index%
                //
                // 在上面的命令里，`cd` 被用作变量，而 start_index 和 end_index 留空。
                // `cd` 是一个内建变量，会动态展开为当前目录，因此它总是可用的。
                // 显式地同时省略起止索引会创建一个零长度的子串。
                //
                // 因此这一切最终都归结为空。然而，通过这个空操作（no-op），我们让 cmd.exe
                // 不再去尝试展开参数中可能存在的 %variables%。
                cmd.extend_from_slice(&[
                    '%' as u16, '%' as u16, 'c' as u16, 'd' as u16, ':' as u16, '~' as u16,
                    ',' as u16,
                ]);
            }
            backslashes = 0;
        }
        cmd.push(x);
    }
    if quote {
        // 在结尾的 `"` 之前加 n 个反斜杠，使其总数达到 2n。
        cmd.extend((0..backslashes).map(|_| '\\' as u16));
        cmd.push('"' as u16);
    }
    Ok(())
}

pub(crate) fn make_bat_command_line(
    script: &[u16],
    args: &[Arg],
    force_quotes: bool,
) -> io::Result<Vec<u16>> {
    const INVALID_ARGUMENT_ERROR: io::Error =
        io::const_error!(io::ErrorKind::InvalidInput, r#"batch file arguments are invalid"#);
    // 把命令行的开头设为 `cmd.exe /c "`
    // 有必要用额外的一对引号把整条命令包起来，
    // 因此这里有一个尾随的引号。它会在所有参数
    // 都被添加之后再闭合。
    // 使用 /e:ON 启用“命令扩展（command extensions）”，这对于让 `%` hack 生效至关重要。
    let mut cmd: Vec<u16> = "cmd.exe /e:ON /v:OFF /d /c \"".encode_utf16().collect();

    // 推入用其引号对包裹起来的脚本名。
    cmd.push(b'"' as u16);
    // Windows 文件名不能包含 `"` 字符，也不能以 `\\` 结尾。
    // 如果脚本名违反了这一点，则返回一个错误。
    if script.contains(&(b'"' as u16)) || script.last() == Some(&(b'\\' as u16)) {
        return Err(io::const_error!(
            io::ErrorKind::InvalidInput,
            "Windows file names may not contain `\"` or end with `\\`"
        ));
    }
    cmd.extend_from_slice(script.strip_suffix(&[0]).unwrap_or(script));
    cmd.push(b'"' as u16);

    // 追加各参数。
    // FIXME: 这里需要测试来确保默认情况下批处理脚本能够
    // 正确地重建这些参数。
    for arg in args {
        cmd.push(' ' as u16);
        match arg {
            Arg::Regular(arg_os) => {
                let arg_bytes = arg_os.as_encoded_bytes();
                // 不允许 \r 和 \n，因为它们可能会截断参数。
                const DISALLOWED: &[u8] = b"\r\n";
                if arg_bytes.iter().any(|c| DISALLOWED.contains(c)) {
                    return Err(INVALID_ARGUMENT_ERROR);
                }
                append_bat_arg(&mut cmd, arg_os, force_quotes)?;
            }
            _ => {
                // Raw 参数按原样传递。
                // 在这种情况下，正确处理参数是用户的责任。
                append_arg(&mut cmd, arg, force_quotes)?;
            }
        };
    }

    // 闭合我们前面留着没关的引号。
    cmd.push(b'"' as u16);

    Ok(cmd)
}

/// 接收一个路径，并尝试返回一个非逐字（non-verbatim）路径。
///
/// 这是必需的，因为 cmd.exe 不支持逐字（verbatim）路径。
pub(crate) fn to_user_path(path: &Path) -> io::Result<Vec<u16>> {
    from_wide_to_user_path(to_u16s(path)?)
}
pub(crate) fn from_wide_to_user_path(mut path: Vec<u16>) -> io::Result<Vec<u16>> {
    // UTF-16 编码的码点，用于解析和构建 UTF-16 路径。
    // 这些全都在 ASCII 范围内，因此可以直接强制转换为 `u16`。
    const SEP: u16 = b'\\' as _;
    const QUERY: u16 = b'?' as _;
    const COLON: u16 = b':' as _;
    const U: u16 = b'U' as _;
    const N: u16 = b'N' as _;
    const C: u16 = b'C' as _;

    // 如果路径太长以致无法移除逐字前缀，则提前返回。
    const LEGACY_MAX_PATH: usize = 260;
    if path.len() > LEGACY_MAX_PATH {
        return Ok(path);
    }

    match &path[..] {
        // `\\?\C:\...` => `C:\...`
        [SEP, SEP, QUERY, SEP, _, COLON, SEP, ..] => unsafe {
            let lpfilename = path[4..].as_ptr();
            fill_utf16_buf(
                |buffer, size| c::GetFullPathNameW(lpfilename, size, buffer, ptr::null_mut()),
                |full_path: &[u16]| {
                    if full_path == &path[4..path.len() - 1] {
                        let mut path: Vec<u16> = full_path.into();
                        path.push(0);
                        path
                    } else {
                        path
                    }
                },
            )
        },
        // `\\?\UNC\...` => `\\...`
        [SEP, SEP, QUERY, SEP, U, N, C, SEP, ..] => unsafe {
            // 把 `UNC\` 中的 `C` 改成 `\`，这样我们就能得到一个以 `\\` 开头的切片。
            path[6] = b'\\' as u16;
            let lpfilename = path[6..].as_ptr();
            fill_utf16_buf(
                |buffer, size| c::GetFullPathNameW(lpfilename, size, buffer, ptr::null_mut()),
                |full_path: &[u16]| {
                    if full_path == &path[6..path.len() - 1] {
                        let mut path: Vec<u16> = full_path.into();
                        path.push(0);
                        path
                    } else {
                        // 把 "UNC" 中的 'C' 复原。
                        path[6] = b'C' as u16;
                        path
                    }
                },
            )
        },
        // 对于其他所有情况，保持路径不变。
        _ => get_long_path(path, false),
    }
}
