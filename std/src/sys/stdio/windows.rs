#![unstable(issue = "none", feature = "windows_stdio")]

use core::str::utf8_char_width;

use crate::mem::MaybeUninit;
use crate::os::windows::io::{FromRawHandle, IntoRawHandle};
use crate::sys::handle::Handle;
use crate::sys::pal::api::{self, WinError};
use crate::sys::{c, cvt};
use crate::{cmp, io, ptr, str};

#[cfg(test)]
mod tests;

// 不要缓存句柄，而是在每次读/写时都重新获取它们。这使得我们能够跟踪该值随时间发生的变化
//（例如某个进程在运行期间调用了 `SetStdHandle`）。参见 #40490。
pub struct Stdin {
    surrogate: u16,
    incomplete_utf8: IncompleteUtf8,
}

pub struct Stdout {
    incomplete_utf8: IncompleteUtf8,
}

pub struct Stderr {
    incomplete_utf8: IncompleteUtf8,
}

struct IncompleteUtf8 {
    bytes: [u8; 4],
    len: u8,
}

impl IncompleteUtf8 {
    // 实现它是为了在 Stdin::read 中使用。
    fn read(&mut self, buf: &mut [u8]) -> usize {
        // 持续写入缓冲区，直到缓冲区写满，或者我们的字节用完。
        let to_write = cmp::min(buf.len(), self.len as usize);
        buf[..to_write].copy_from_slice(&self.bytes[..to_write]);

        // 如果缓冲区中剩余空间不足，则旋转（rotate）剩余的字节。
        if usize::from(self.len) > buf.len() {
            self.bytes.copy_within(to_write.., 0);
            self.len -= to_write as u8;
        } else {
            self.len = 0;
        }

        to_write
    }
}

// 显然 Windows 在 stdin 上的大读取、或对 stdout/stderr 的大写入处理得不好
//（细节参见 #13304）。
//
// 摘自 MSDN（2011）："该缓冲区的存储是从进程的一个共享堆（shared heap）中分配的，
// 其大小为 64 KB。缓冲区的最大尺寸将取决于堆的使用情况。"
//
// 我们把上限选为 8 KiB，因为 libuv 也是这么做的，而且到目前为止它似乎是可以接受的。
const MAX_BUFFER_SIZE: usize = 8192;

// Stdin 的 BufReader 的标准缓冲区大小应当能够容纳的字节数，是 MAX_BUFFER_SIZE 中
// `u16` 个数的 3 倍以上。这确保读到的数据总是能够从 UTF-16 完整地解码为 UTF-8。
pub const STDIN_BUF_SIZE: usize = MAX_BUFFER_SIZE / 2 * 3;

pub fn get_handle(handle_id: u32) -> io::Result<c::HANDLE> {
    let handle = unsafe { c::GetStdHandle(handle_id) };
    if handle == c::INVALID_HANDLE_VALUE {
        Err(io::Error::last_os_error())
    } else if handle.is_null() {
        Err(io::Error::from_raw_os_error(c::ERROR_INVALID_HANDLE as i32))
    } else {
        Ok(handle)
    }
}

fn is_console(handle: c::HANDLE) -> bool {
    // 如果这是一个管道（pipe），`GetConsoleMode` 会返回 false (0)（我们并不关心其报告的
    // mode）。它只会检测 Windows Console，而不会检测其他通过管道连接的终端，比如 MSYS。
    // 这正是我们所需要的，因为只有 Windows Console 才需要转换为 UTF-16。
    let mut mode = 0;
    unsafe { c::GetConsoleMode(handle, &mut mode) != 0 }
}

/// 如果所连接控制台的代码页（code page）当前为 UTF-8，则返回 true。
#[cfg(not(target_vendor = "win7"))]
fn is_utf8_console() -> bool {
    unsafe { c::GetConsoleOutputCP() == c::CP_UTF8 }
}

#[cfg(target_vendor = "win7")]
fn is_utf8_console() -> bool {
    // Windows 7 有一个有趣的“特性”：对一个控制台句柄调用 WriteFile 时，
    // 它返回的是写入的 UTF-16 码元（code units）数量，而不是输入字符串中的字节数。
    // 因此我们总是声称该控制台不是 UTF-8，从而触发 WriteConsole 回退代码。
    false
}

fn write(handle_id: u32, data: &[u8], incomplete_utf8: &mut IncompleteUtf8) -> io::Result<usize> {
    if data.is_empty() {
        return Ok(0);
    }

    let handle = get_handle(handle_id)?;
    if !is_console(handle) || is_utf8_console() {
        unsafe {
            let handle = Handle::from_raw_handle(handle);
            let ret = handle.write(data);
            let _ = handle.into_raw_handle(); // 不要关闭该句柄
            return ret;
        }
    } else {
        write_console_utf16(data, incomplete_utf8, handle)
    }
}

fn write_console_utf16(
    data: &[u8],
    incomplete_utf8: &mut IncompleteUtf8,
    handle: c::HANDLE,
) -> io::Result<usize> {
    if incomplete_utf8.len > 0 {
        assert!(
            incomplete_utf8.len < 4,
            "Unexpected number of bytes for incomplete UTF-8 codepoint."
        );
        if data[0] >> 6 != 0b10 {
            // 不是延续字节（continuation byte）——拒绝
            incomplete_utf8.len = 0;
            return Err(io::const_error!(
                io::ErrorKind::InvalidData,
                "Windows stdio in console mode does not support writing non-UTF-8 byte sequences",
            ));
        }
        incomplete_utf8.bytes[incomplete_utf8.len as usize] = data[0];
        incomplete_utf8.len += 1;
        let char_width = utf8_char_width(incomplete_utf8.bytes[0]);
        if (incomplete_utf8.len as usize) < char_width {
            // 还需要更多字节
            return Ok(1);
        }
        let s = str::from_utf8(&incomplete_utf8.bytes[0..incomplete_utf8.len as usize]);
        incomplete_utf8.len = 0;
        match s {
            Ok(s) => {
                assert_eq!(char_width, s.len());
                let written = write_valid_utf8_to_console(handle, s)?;
                assert_eq!(written, s.len()); // 对于单个码点（codepoint）的写入，这由 write_valid_utf8_to_console() 保证
                return Ok(1);
            }
            Err(_) => {
                return Err(io::const_error!(
                    io::ErrorKind::InvalidData,
                    "Windows stdio in console mode does not support writing non-UTF-8 byte sequences",
                ));
            }
        }
    }

    // 由于控制台用于呈现文本，我们假定 `data` 中的字节是以 UTF-8 编码的，
    // 而这需要被编码为 UTF-16。
    //
    // 如果数据不是有效的 UTF-8，我们就只写出其中有效的那部分字节。
    // 如果第一个字节就无效，那么它要么是某个多字节序列的首字节但所提供的字节切片太短，
    // 要么是某个无效多字节序列的首字节。
    let len = cmp::min(data.len(), MAX_BUFFER_SIZE / 2);
    let utf8 = match str::from_utf8(&data[..len]) {
        Ok(s) => s,
        Err(ref e) if e.valid_up_to() == 0 => {
            let first_byte_char_width = utf8_char_width(data[0]);
            if first_byte_char_width > 1 && data.len() < first_byte_char_width {
                incomplete_utf8.bytes[0] = data[0];
                incomplete_utf8.len = 1;
                return Ok(1);
            } else {
                return Err(io::const_error!(
                    io::ErrorKind::InvalidData,
                    "Windows stdio in console mode does not support writing non-UTF-8 byte sequences",
                ));
            }
        }
        Err(e) => str::from_utf8(&data[..e.valid_up_to()]).unwrap(),
    };

    write_valid_utf8_to_console(handle, utf8)
}

fn write_valid_utf8_to_console(handle: c::HANDLE, utf8: &str) -> io::Result<usize> {
    debug_assert!(!utf8.is_empty());

    let mut utf16 = [MaybeUninit::<u16>::uninit(); MAX_BUFFER_SIZE / 2];
    let utf8 = &utf8[..utf8.floor_char_boundary(utf16.len())];

    let utf16: &[u16] = unsafe {
        // 注意：在底层字节序列是有效 utf-8 的（最常见）情况下（鉴于 `write()` 中已有检查），
        // 这在理论上会重复进行两次有效性检查。
        let result = c::MultiByteToWideChar(
            c::CP_UTF8,                          // CodePage
            c::MB_ERR_INVALID_CHARS,             // dwFlags
            utf8.as_ptr(),                       // lpMultiByteStr
            utf8.len() as i32,                   // cbMultiByte
            utf16.as_mut_ptr() as *mut c::WCHAR, // lpWideCharStr
            utf16.len() as i32,                  // cchWideChar
        );
        assert!(result != 0, "Unexpected error in MultiByteToWideChar");

        // Safety: MultiByteToWideChar 会初始化 `result` 个值。
        utf16[..result as usize].assume_init_ref()
    };

    let mut written = write_u16s(handle, utf16)?;

    // 计算以 UTF-16 形式写出去的内容对应了多少字节的 UTF-8。
    if written == utf16.len() {
        Ok(utf8.len())
    } else {
        // 确保我们没有最终只写出了代理对（surrogate pair）的一半（尽管这种几率非常小）。
        // 由于用户代码不可能以某种方式重新切片（re-slice）`data` 而产生出缺失的代理项
        //（同时也由于上面的 UTF-8 验证），现在就把缺失的代理项写出去。
        // 对它进行缓冲将意味着我们不得不在已写入字节数上撒谎。
        let first_code_unit_remaining = utf16[written];
        if matches!(first_code_unit_remaining, 0xDCEE..=0xDFFF) {
            // 低位代理项（low surrogate）
            // 我们只能寄希望于它能成功，否则就放弃
            let _ = write_u16s(handle, &utf16[written..written + 1]);
            written += 1;
        }
        // 计算 `utf8` 中实际被写入的字节数。
        let mut count = 0;
        for ch in utf16[..written].iter() {
            count += match ch {
                0x0000..=0x007F => 1,
                0x0080..=0x07FF => 2,
                0xDCEE..=0xDFFF => 1, // 低位代理项（Low surrogate）。我们已经为另一半计入了 3 个字节。
                _ => 3,
            };
        }
        debug_assert!(String::from_utf16(&utf16[..written]).unwrap() == utf8[..count]);
        Ok(count)
    }
}

fn write_u16s(handle: c::HANDLE, data: &[u16]) -> io::Result<usize> {
    debug_assert!(data.len() < u32::MAX as usize);
    let mut written = 0;
    cvt(unsafe {
        c::WriteConsoleW(handle, data.as_ptr(), data.len() as u32, &mut written, ptr::null_mut())
    })?;
    Ok(written as usize)
}

impl Stdin {
    pub const fn new() -> Stdin {
        Stdin { surrogate: 0, incomplete_utf8: IncompleteUtf8::new() }
    }
}

impl io::Read for Stdin {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let handle = get_handle(c::STD_INPUT_HANDLE)?;
        if !is_console(handle) {
            unsafe {
                let handle = Handle::from_raw_handle(handle);
                let ret = handle.read(buf);
                let _ = handle.into_raw_handle(); // 不要关闭该句柄
                return ret;
            }
        }

        // 如果不完整的 utf-8 缓冲区中还有字节，就先从那些字节开始。
        //（如果缓冲区中什么都没有，则为空操作。）
        let mut bytes_copied = self.incomplete_utf8.read(buf);

        if bytes_copied == buf.len() {
            Ok(bytes_copied)
        } else if buf.len() - bytes_copied < 4 {
            // 空间不足以取得一个 UTF-8 字节。我们将使用 incomplete UTF8。
            let mut utf16_buf = [MaybeUninit::new(0); 1];
            // 读取一个 u16 字符。
            let read = read_u16s_fixup_surrogates(handle, &mut utf16_buf, 1, &mut self.surrogate)?;
            // 读取字节，把（现在已为空的）self.incomplete_utf8 用作额外空间。
            let read_bytes = utf16_to_utf8(
                unsafe { utf16_buf[..read].assume_init_ref() },
                &mut self.incomplete_utf8.bytes,
            )?;

            // 从 incomplete_utf8 中读入字节，直到缓冲区被填满。
            self.incomplete_utf8.len = read_bytes as u8;
            // 如果没有字节，则为空操作。
            bytes_copied += self.incomplete_utf8.read(&mut buf[bytes_copied..]);
            Ok(bytes_copied)
        } else {
            let mut utf16_buf = [MaybeUninit::<u16>::uninit(); MAX_BUFFER_SIZE / 2];

            // 在最坏情况下，一个 UTF-8 字符串中每个 UTF-16 的 `u16` 可能会占用 3 个字节。
            // 因此我们最多只能读取 `buf.len()` 的三分之一个字符，并且坚守不丢失任何数据的保证。
            let amount = cmp::min(buf.len() / 3, utf16_buf.len());
            let read =
                read_u16s_fixup_surrogates(handle, &mut utf16_buf, amount, &mut self.surrogate)?;
            // Safety: `read_u16s_fixup_surrogates` 返回已初始化的项数。
            let utf16s = unsafe { utf16_buf[..read].assume_init_ref() };
            match utf16_to_utf8(utf16s, buf) {
                Ok(value) => return Ok(bytes_copied + value),
                Err(e) => return Err(e),
            }
        }
    }
}

// 我们假定：如果最后一个 `u16` 是一个未配对的代理项（unpaired surrogate），
// 那是因为它们被我们的缓冲区大小切开了；于是把它保留下来，留到下一次读取时，
// 寄希望于能把它们重新拼到一起。
// 这是一种尽力而为（best effort）的做法，如果我们不是 Stdin 上唯一的 reader，它可能不起作用。
fn read_u16s_fixup_surrogates(
    handle: c::HANDLE,
    buf: &mut [MaybeUninit<u16>],
    mut amount: usize,
    surrogate: &mut u16,
) -> io::Result<usize> {
    // 插入上一次读取中可能残留的未配对代理项。
    let mut start = 0;
    if *surrogate != 0 {
        buf[0] = MaybeUninit::new(*surrogate);
        *surrogate = 0;
        start = 1;
        if amount == 1 {
            // 特殊情况：`Stdin::read` 保证我们总能读到至少一个新的 `u16`，
            // 并把它与一个未配对的代理项组合起来，因为 UTF-8 缓冲区至少有 4 个字节。
            amount = 2;
        }
    }
    let mut amount = read_u16s(handle, &mut buf[start..amount])? + start;

    if amount > 0 {
        // Safety: 返回的 `amount` 是已初始化的值的数量，并且它不为 0，
        // 因此我们知道 `buf[amount - 1]` 已经被初始化。
        let last_char = unsafe { buf[amount - 1].assume_init() };
        if matches!(last_char, 0xD800..=0xDBFF) {
            // 高位代理项（high surrogate）
            *surrogate = last_char;
            amount -= 1;
        }
    }
    Ok(amount)
}

// 如果它在 `buf` 中初始化了 `n` 个值，则返回 `Ok(n)`。
fn read_u16s(handle: c::HANDLE, buf: &mut [MaybeUninit<u16>]) -> io::Result<usize> {
    // 配置 `pInputControl` 参数，使其不仅在遇到 `\r\n` 时返回，还在遇到 Ctrl-Z 时返回；
    // Ctrl-Z 是 DOS 中用来表示字符流结束 / 用户输入结束的传统方式（SUB）。
    // 参见 #38274 和 https://stackoverflow.com/questions/43836040/win-api-readconsole。
    const CTRL_Z: u16 = 0x1A;
    const CTRL_Z_MASK: u32 = 1 << CTRL_Z;
    let input_control = c::CONSOLE_READCONSOLE_CONTROL {
        nLength: size_of::<c::CONSOLE_READCONSOLE_CONTROL>() as u32,
        nInitialChars: 0,
        dwCtrlWakeupMask: CTRL_Z_MASK,
        dwControlKeyState: 0,
    };

    let mut amount = 0;
    loop {
        cvt(unsafe {
            c::SetLastError(0);
            c::ReadConsoleW(
                handle,
                buf.as_mut_ptr() as *mut core::ffi::c_void,
                buf.len() as u32,
                &mut amount,
                &input_control,
            )
        })?;

        // 对于 Ctrl-C 或 Ctrl-Break，ReadConsoleW 会返回成功，但伴随 ERROR_OPERATION_ABORTED。
        // 这里显式检查这种情况并重试。
        if amount == 0 && api::get_last_error() == WinError::OPERATION_ABORTED {
            continue;
        }
        break;
    }
    // Safety: 如果 `amount > 0`，则说明写入了那么多字节，
    // 因此 `buf[amount as usize - 1]` 已经被初始化。
    if amount > 0 && unsafe { buf[amount as usize - 1].assume_init() } == CTRL_Z {
        amount -= 1;
    }
    Ok(amount as usize)
}

fn utf16_to_utf8(utf16: &[u16], utf8: &mut [u8]) -> io::Result<usize> {
    debug_assert!(utf16.len() <= i32::MAX as usize);
    debug_assert!(utf8.len() <= i32::MAX as usize);

    if utf16.is_empty() {
        return Ok(0);
    }

    let result = unsafe {
        c::WideCharToMultiByte(
            c::CP_UTF8,              // CodePage
            c::WC_ERR_INVALID_CHARS, // dwFlags
            utf16.as_ptr(),          // lpWideCharStr
            utf16.len() as i32,      // cchWideChar
            utf8.as_mut_ptr(),       // lpMultiByteStr
            utf8.len() as i32,       // cbMultiByte
            ptr::null(),             // lpDefaultChar
            ptr::null_mut(),         // lpUsedDefaultChar
        )
    };
    if result == 0 {
        // 除了丢弃所有数据并返回一个错误之外，我们实在没有更好的办法了。
        Err(io::const_error!(
            io::ErrorKind::InvalidData,
            "Windows stdin in console mode does not support non-UTF-16 input; \
            encountered unpaired surrogate",
        ))
    } else {
        Ok(result as usize)
    }
}

impl IncompleteUtf8 {
    pub const fn new() -> IncompleteUtf8 {
        IncompleteUtf8 { bytes: [0; char::MAX_LEN_UTF8], len: 0 }
    }
}

impl Stdout {
    pub const fn new() -> Stdout {
        Stdout { incomplete_utf8: IncompleteUtf8::new() }
    }
}

impl io::Write for Stdout {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        write(c::STD_OUTPUT_HANDLE, buf, &mut self.incomplete_utf8)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl Stderr {
    pub const fn new() -> Stderr {
        Stderr { incomplete_utf8: IncompleteUtf8::new() }
    }
}

impl io::Write for Stderr {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        write(c::STD_ERROR_HANDLE, buf, &mut self.incomplete_utf8)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub fn is_ebadf(err: &io::Error) -> bool {
    err.raw_os_error() == Some(c::ERROR_INVALID_HANDLE as i32)
}

pub fn panic_output() -> Option<impl io::Write> {
    Some(Stderr::new())
}
