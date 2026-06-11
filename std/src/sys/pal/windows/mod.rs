#![allow(missing_docs, nonstandard_style)]
#![forbid(unsafe_op_in_unsafe_fn)]

use crate::ffi::{OsStr, OsString};
use crate::io;
use crate::mem::MaybeUninit;
use crate::os::windows::ffi::{OsStrExt, OsStringExt};
use crate::path::PathBuf;
use crate::sys::pal::windows::api::wide_str;
use crate::time::Duration;

#[macro_use]
pub mod compat;

pub mod api;

pub mod c;
#[cfg(not(target_vendor = "win7"))]
pub mod futex;
pub mod handle;
pub mod os;
pub mod time;
cfg_select! {
    // 在 panic=immediate-abort（立即中止）模式下，我们不关心是否能打印漂亮的错误信息
    all(not(target_vendor = "uwp"), not(panic = "immediate-abort")) => {
        pub mod stack_overflow;
    }
    _ => {
        pub mod stack_overflow_uwp;
        pub use self::stack_overflow_uwp as stack_overflow;
    }
}
pub mod winsock;

/// 将 [`Result<T, WinError>`] 映射为 [`io::Result<T>`]。
pub trait IoResult<T> {
    fn io_result(self) -> io::Result<T>;
}
impl<T> IoResult<T> for Result<T, api::WinError> {
    fn io_result(self) -> io::Result<T> {
        self.map_err(|e| io::Error::from_raw_os_error(e.code as i32))
    }
}

// SAFETY: 只能在运行时初始化期间调用一次。
// NOTE: 不保证一定会运行，例如当 Rust 代码被外部调用时就不会运行。
pub unsafe fn init(_argc: isize, _argv: *const *const u8, _sigpipe: u8) {
    unsafe {
        stack_overflow::init();

        // 通常情况下，`thread::spawn` 会调用 `set_name`，但由于该线程（主线程）
        // 已经存在，所以我们必须自己来调用它。
        crate::sys::thread::set_name_wide(wide_str!("main"));
    }
}

// SAFETY: 只能在运行时清理期间调用一次。
// NOTE: 不保证一定会运行，例如当程序中止（abort）时就不会运行。
pub unsafe fn cleanup() {
    winsock::cleanup();
}

pub fn unrolled_find_u16s(needle: u16, haystack: &[u16]) -> Option<usize> {
    let ptr = haystack.as_ptr();
    let mut start = haystack;

    // 出于性能考虑，将循环展开八次。
    while start.len() >= 8 {
        macro_rules! if_return {
            ($($n:literal,)+) => {
                $(
                    if start[$n] == needle {
                        return Some(((&start[$n] as *const u16).addr() - ptr.addr()) / 2);
                    }
                )+
            }
        }

        if_return!(0, 1, 2, 3, 4, 5, 6, 7,);

        start = &start[8..];
    }

    for c in start {
        if *c == needle {
            return Some(((c as *const u16).addr() - ptr.addr()) / 2);
        }
    }
    None
}

pub fn to_u16s<S: AsRef<OsStr>>(s: S) -> io::Result<Vec<u16>> {
    fn inner(s: &OsStr) -> io::Result<Vec<u16>> {
        // 大多数路径都是 ASCII，因此预留与 OsStr 字节数相同的容量，
        // 再加上一个用于存放 null 结尾字符的位置。这里并不会浪费字节，
        // 因为本函数创建的路径主要用于临时（ephemeral）用途。
        let mut maybe_result = Vec::with_capacity(s.len() + 1);
        maybe_result.extend(s.encode_wide());

        if unrolled_find_u16s(0, &maybe_result).is_some() {
            return Err(io::const_error!(
                io::ErrorKind::InvalidInput,
                "strings passed to WinAPI cannot contain NULs",
            ));
        }
        maybe_result.push(0);
        Ok(maybe_result)
    }
    inner(s.as_ref())
}

// 许多 Windows API 遵循这样一种模式：我们传入一个缓冲区，它们随后会告诉我们
// 缓冲区应该有多大，或者当前缓冲区中已存放了多少字节。本函数是对这类函数的抽象，
// 使它们更易于调用。
//
// 第一个回调 `f1` 会收到一个 (pointer, len) 二元组，可直接传给系统调用。
// 其中 `ptr` 在 `len` 个元素范围内有效（本例中为 u16）。该闭包应当：
// - 成功时，返回所写入数据的实际长度（*不含* null 结尾符）。该值可能为 0，
//   此时必须保持 last_error 不变。
// - 缓冲区空间不足时，
//   - 要么返回所需的长度（*包含* null 结尾符），
//   - 要么将 last-error 设为 ERROR_INSUFFICIENT_BUFFER 并返回 `len`。
// - 其他失败情况，返回 0 并设置 last_error。
//
// 大多数（但并非全部）系统调用以此方式表示所需的缓冲区大小。其他系统调用可能
// 需要做转换以符合该协议。
//
// 一旦系统调用完成（出错则提前返回），第二个闭包会收到从系统调用中读取到的数据。
// 该闭包的返回值即为本函数的返回值。
pub fn fill_utf16_buf<F1, F2, T>(mut f1: F1, f2: F2) -> io::Result<T>
where
    F1: FnMut(*mut u16, u32) -> u32,
    F2: FnOnce(&[u16]) -> T,
{
    // 先从一个栈上缓冲区开始，如果最终需要更多空间，再溢出到堆上。
    //
    // 这个初始大小同时也规避了 `GetFullPathNameW` 对某些短路径返回
    // 错误大小提示的问题：
    // https://github.com/dylni/normpath/issues/5
    let mut stack_buf: [MaybeUninit<u16>; 512] = [MaybeUninit::uninit(); 512];
    let mut heap_buf: Vec<MaybeUninit<u16>> = Vec::new();
    unsafe {
        let mut n = stack_buf.len();
        loop {
            let buf = if n <= stack_buf.len() {
                &mut stack_buf[..]
            } else {
                let extra = n - heap_buf.len();
                heap_buf.reserve(extra);
                // 我们用的是 `reserve` 而非 `reserve_exact`，因此理论上可能
                // 拿到比请求更多的容量。如果是这样，我们希望能用上这部分容量……
                // 只要不会引发溢出即可。
                n = heap_buf.capacity().min(u32::MAX as usize);
                // Safety: MaybeUninit<u16> 无需初始化
                heap_buf.set_len(n);
                &mut heap_buf[..]
            };

            // 本函数通常用于调用那些会返回字符串正确长度的 Windows API 函数，
            // 但这些函数同时也用返回 `0` 来表示出错。然而在某些情况下，
            // 返回的“正确长度”本身就可能是 0！
            //
            // 为了处理这种情况，我们调用 `SetLastError` 将其重置为 0，
            // 然后在拿到“0 错误值”时再次检查它。如果此时“last error”仍为 0，
            // 我们就将其解释为缓冲区长度为 0，而非真正的错误。
            c::SetLastError(0);
            let k = match f1(buf.as_mut_ptr().cast::<u16>(), n as u32) {
                0 if api::get_last_error().code == 0 => 0,
                0 => return Err(io::Error::last_os_error()),
                n => n,
            } as usize;
            if k == n && api::get_last_error().code == c::ERROR_INSUFFICIENT_BUFFER {
                n = n.saturating_mul(2).min(u32::MAX as usize);
            } else if k > n {
                n = k;
            } else if k == n {
                // 不可能执行到这一点。
                // 成功时，k 是返回的字符串长度（不含 null）。
                // 失败时，k 是所需的缓冲区长度（含 null）。
                // 因此 k 永远不会等于 n。
                unreachable!();
            } else {
                // Safety: 前 `k` 个值已被初始化。
                let slice: &[u16] = buf[..k].assume_init_ref();
                return Ok(f2(slice));
            }
        }
    }
}

pub fn os2path(s: &[u16]) -> PathBuf {
    PathBuf::from(OsString::from_wide(s))
}

pub fn truncate_utf16_at_nul(v: &[u16]) -> &[u16] {
    match unrolled_find_u16s(0, v) {
        // 不包含末尾的 0
        Some(i) => &v[..i],
        None => v,
    }
}

pub fn ensure_no_nuls<T: AsRef<OsStr>>(s: T) -> io::Result<T> {
    if s.as_ref().encode_wide().any(|b| b == 0) {
        Err(io::const_error!(io::ErrorKind::InvalidInput, "nul byte found in provided data"))
    } else {
        Ok(s)
    }
}

pub trait IsZero {
    fn is_zero(&self) -> bool;
}

macro_rules! impl_is_zero {
    ($($t:ident)*) => ($(impl IsZero for $t {
        fn is_zero(&self) -> bool {
            *self == 0
        }
    })*)
}

impl_is_zero! { i8 i16 i32 i64 isize u8 u16 u32 u64 usize }

pub fn cvt<I: IsZero>(i: I) -> io::Result<I> {
    if i.is_zero() { Err(io::Error::last_os_error()) } else { Ok(i) }
}

pub fn dur2timeout(dur: Duration) -> u32 {
    // 注意：Duration 是一个 (u64, u32)（秒，纳秒）二元组，而 Windows API 中的
    // 超时时间通常是以毫秒为单位的 u32。要完成转换，需处理两件事：
    //
    // * 纳秒精度向上取整
    // * 超过 u32::MAX 毫秒（50 天）的值向上取整为 INFINITE（永不超时）。
    dur.as_secs()
        .checked_mul(1000)
        .and_then(|ms| ms.checked_add((dur.subsec_nanos() as u64) / 1_000_000))
        .and_then(|ms| ms.checked_add(if dur.subsec_nanos() % 1_000_000 > 0 { 1 } else { 0 }))
        .map(|ms| if ms > <u32>::MAX as u64 { c::INFINITE } else { ms as u32 })
        .unwrap_or(c::INFINITE)
}

/// 使用 `__fastfail` 来中止进程
///
/// 在 Windows 8 及更高版本中，这会立即终止进程，而不运行任何进程内异常处理程序。
/// 在更早的 Windows 版本中，这一指令序列会被当作访问违例（access violation）处理，
/// 它仍然会终止进程，但可能会运行一些异常处理程序。
///
/// <https://docs.microsoft.com/en-us/cpp/intrinsics/fastfail>
#[cfg(not(miri))] // 内联汇编在 Miri 中无法工作
pub fn abort_internal() -> ! {
    unsafe {
        cfg_select! {
            any(target_arch = "x86", target_arch = "x86_64") => {
                core::arch::asm!("int $$0x29", in("ecx") c::FAST_FAIL_FATAL_APP_EXIT, options(noreturn, nostack));
            }
            all(target_arch = "arm", target_feature = "thumb-mode") => {
                core::arch::asm!(".inst 0xDEFB", in("r0") c::FAST_FAIL_FATAL_APP_EXIT, options(noreturn, nostack));
            }
            any(target_arch = "aarch64", target_arch = "arm64ec") => {
                core::arch::asm!("brk 0xF003", in("x0") c::FAST_FAIL_FATAL_APP_EXIT, options(noreturn, nostack));
            }
            _ => {
                core::intrinsics::abort();
            }
        }
    }
}

#[cfg(miri)]
#[track_caller] // 即使没有 panic，这也有助于改善 Miri 的回溯信息
pub fn abort_internal() -> ! {
    crate::intrinsics::abort();
}

/// 将内部值对齐到 8 字节。
///
/// 对于我们在所用 Windows API 中可能处理的几乎所有缓冲区而言，这一对齐已经足够。
#[repr(C, align(8))]
#[derive(Copy, Clone)]
pub(crate) struct Align8<T: ?Sized>(pub T);
