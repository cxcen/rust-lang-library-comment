use r_efi::protocols::loaded_image;

pub use super::common::Args;
use crate::env::current_exe;
use crate::ffi::OsString;
use crate::iter::Iterator;
use crate::sys::pal::helpers;

pub fn args() -> Args {
    let lazy_current_exe = || Vec::from([current_exe().map(Into::into).unwrap_or_default()]);

    // 每个被加载的映像（loaded image）都有一个支持 `EFI_LOADED_IMAGE_PROTOCOL`
    // 的映像句柄。因此这绝不会失败。
    let protocol =
        helpers::image_handle_protocol::<loaded_image::Protocol>(loaded_image::PROTOCOL_GUID)
            .unwrap();

    let lp_size = unsafe { (*protocol.as_ptr()).load_options_size } as usize;
    // 如果可以确定它不可能是 UTF-16，则中断
    if lp_size < size_of::<u16>() || lp_size % size_of::<u16>() != 0 {
        return Args::new(lazy_current_exe());
    }
    let lp_size = lp_size / size_of::<u16>();

    let lp_cmd_line = unsafe { (*protocol.as_ptr()).load_options as *const u16 };
    if !lp_cmd_line.is_aligned() {
        return Args::new(lazy_current_exe());
    }
    let lp_cmd_line = unsafe { crate::slice::from_raw_parts(lp_cmd_line, lp_size) };

    Args::new(parse_lp_cmd_line(lp_cmd_line).unwrap_or_else(lazy_current_exe))
}

/// 实现 UEFI 命令行参数解析算法。
///
/// 本实现基于
/// [UEFI Shell Specification](https://uefi.org/sites/default/files/resources/UEFI_Shell_Spec_2_0.pdf)
/// 第 3.4 节中所定义的内容
///
/// 在以下情况下返回 None：
/// - 无效的 UTF-16（未配对的代理项 surrogate）
/// - 空的/不规范的参数
fn parse_lp_cmd_line(code_units: &[u16]) -> Option<Vec<OsString>> {
    const QUOTE: char = '"';
    const SPACE: char = ' ';
    const CARET: char = '^';
    const NULL: char = '\0';

    let mut ret_val = Vec::new();
    let mut code_units_iter = char::decode_utf16(code_units.iter().cloned()).peekable();

    // 开头的可执行文件名是个特殊情况。
    let mut in_quotes = false;
    let mut cur = String::new();
    while let Some(w) = code_units_iter.next() {
        let w = w.ok()?;
        match w {
            // 遇到 NULL 则中断
            NULL => break,
            // 引号标记总是会切换 `in_quotes`，无论如何，
            // 因为解析可执行文件名时没有转义字符。
            QUOTE => in_quotes = !in_quotes,
            // 如果不在 `in_quotes` 中，则空白结束 argv[0]。
            SPACE if !in_quotes => break,
            // 在其他所有情况下，该码元都按字面取用。
            _ => cur.push(w),
        }
    }

    // 如果缺少 exe 名，则该命令行参数是无效的
    if cur.is_empty() {
        return None;
    }

    ret_val.push(OsString::from(cur));
    // 跳过空白。
    while code_units_iter.next_if_eq(&Ok(SPACE)).is_some() {}

    // 按照以下规则解析参数：
    // * 除空格、引号和插入符（caret）外，所有码元都按字面取用。
    // * 当不在 `in_quotes` 中时，空格分隔参数。连续的空格
    // 被视为单个分隔符。
    // * `in_quotes` 中的空格按字面取用。
    // * 引号会切换 `in_quotes` 模式，除非它被转义。被转义的引号按字面取用。
    // * 如果引号前面有插入符，则该引号可被转义。
    // * 如果插入符前面有插入符，则该插入符可被转义。
    let mut cur = String::new();
    let mut in_quotes = false;
    while let Some(w) = code_units_iter.next() {
        let w = w.ok()?;
        match w {
            // 遇到 NULL 则中断
            NULL => break,
            // 如果不在 `in_quotes` 中，空格或制表符结束该参数。
            SPACE if !in_quotes => {
                ret_val.push(OsString::from(&cur[..]));
                cur.truncate(0);

                // 跳过空白。
                while code_units_iter.next_if_eq(&Ok(SPACE)).is_some() {}
            }
            // 插入符可以转义引号或插入符
            CARET if in_quotes => {
                if let Some(x) = code_units_iter.next() {
                    cur.push(x.ok()?);
                }
            }
            // 如果是引号，则翻转 `in_quotes`
            QUOTE => in_quotes = !in_quotes,
            // 其他所有内容总是按字面取用。
            _ => cur.push(w),
        }
    }
    // 推入最后一个参数（如果有的话）。
    if !cur.is_empty() || in_quotes {
        ret_val.push(OsString::from(cur));
    }
    Some(ret_val)
}
