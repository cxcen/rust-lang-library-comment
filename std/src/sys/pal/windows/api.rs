//! # 对 Windows API 函数的安全（或更安全）封装。
//!
//! 本模块包含对 Windows API 函数的相当薄的封装，目的是把安全性集中到一处，
//! 而不是让 unsafe 块散布在更上层的代码中。这使得审计 FFI 安全性变得容易得多。
//!
//! 并非所有函数都能在缺乏更多上下文的情况下做到完全安全，但即便如此，
//! 我们仍应尽可能减轻调用方在安全性方面的负担。
//!
//! ## 封装指南
//!
//! 这里的条目应当以与其对应的原始 Windows API 名称相似的方式命名，
//! 区别仅在于遵循 Rust 的命名约定。例如函数名采用 lower_snake_case。
//! 这样做的意图是：让 Windows C/C++ 程序员可以轻松辨认出被封装的底层函数，
//! 同时又不至于在 Rust 代码中显得格格不入。
//!
//! 每一处 `unsafe` 块都必须配有相应的 SAFETY 注释，即便它显然是安全的
//! （例如，参见 `get_last_error`）。公开的 unsafe 函数必须说明调用方需要
//! 做些什么才能安全地调用它们。
//!
//! 避免未经检查的 `as` 转换。对于整数，要么断言该整数在取值范围内，
//! 要么改用 `try_into`。对于指针，尽可能使用 `ptr.cast::<Type>()`。
//!
//! 本模块只能依赖 core，而不能依赖 std 类型，因为最终的目标是让 std 依赖 sys，
//! 而非反过来。不过，目前可能仍然需要一些粘合代码（glue code），这类代码
//! 应放到 sys/pal/windows/mod.rs 中，而非这里。可参见 `IoResult` 这个例子。

use core::ffi::c_void;
use core::marker::PhantomData;

use super::c;

/// 从一个 str 创建以 null 结尾的 UTF-16 字符串。
pub macro wide_str($str:literal) {{
    const _: () = {
        if core::slice::memchr::memchr(0, $str.as_bytes()).is_some() {
            panic!("null terminated strings cannot contain interior nulls");
        }
    };
    crate::sys::pal::windows::api::utf16!(concat!($str, '\0'))
}}

/// 从一个 str 创建 UTF-16 字符串，不带 null 结尾符。
pub macro utf16($str:expr) {{
    const UTF8: &str = $str;
    const UTF16_LEN: usize = crate::sys::pal::windows::api::utf16_len(UTF8);
    const UTF16: [u16; UTF16_LEN] = crate::sys::pal::windows::api::to_utf16(UTF8);
    &UTF16
}}

#[cfg(test)]
mod tests;

/// 获取 UTF-8 字符串对应的 UTF-16 长度，供 wide_str 宏使用。
pub const fn utf16_len(s: &str) -> usize {
    let s = s.as_bytes();
    let mut i = 0;
    let mut len = 0;
    while i < s.len() {
        // 一个 UTF-8 编码码点的长度由其前导 1 的个数给出，ASCII 情况除外。
        let utf8_len = match s[i].leading_ones() {
            0 => 1,
            n => n as usize,
        };
        i += utf8_len;
        // 注意：UTF-16 代理项（U+D800 至 U+DFFF）无法用 UTF-8 编码，
        // 因此（不同于 WTF-8）我们无需担心它们会被如何重新编码。
        len += if utf8_len < 4 { 1 } else { 2 };
    }
    len
}

/// 在 const 上下文中把 UTF-8 转换为 UTF-16，供 wide_str 宏使用。
///
/// 注意：这是为在 const 上下文中使用而设计的，因此没有做优化。
pub const fn to_utf16<const UTF16_LEN: usize>(s: &str) -> [u16; UTF16_LEN] {
    let mut output = [0_u16; UTF16_LEN];
    let mut pos = 0;
    let s = s.as_bytes();
    let mut i = 0;
    while i < s.len() {
        match s[i].leading_ones() {
            // 根据 UTF-8 的长度进行解码。
            // 参见 https://en.wikipedia.org/wiki/UTF-8
            0 => {
                // ASCII 在两种编码中是一样的
                output[pos] = s[i] as u16;
                i += 1;
                pos += 1;
            }
            2 => {
                // 位布局：110xxxxx 10xxxxxx
                output[pos] = ((s[i] as u16 & 0b11111) << 6) | (s[i + 1] as u16 & 0b111111);
                i += 2;
                pos += 1;
            }
            3 => {
                // 位布局：1110xxxx 10xxxxxx 10xxxxxx
                output[pos] = ((s[i] as u16 & 0b1111) << 12)
                    | ((s[i + 1] as u16 & 0b111111) << 6)
                    | (s[i + 2] as u16 & 0b111111);
                i += 3;
                pos += 1;
            }
            4 => {
                // 位布局：11110xxx 10xxxxxx 10xxxxxx 10xxxxxx
                let mut c = ((s[i] as u32 & 0b111) << 18)
                    | ((s[i + 1] as u32 & 0b111111) << 12)
                    | ((s[i + 2] as u32 & 0b111111) << 6)
                    | (s[i + 3] as u32 & 0b111111);
                // 重新编码为 UTF-16（参见 https://en.wikipedia.org/wiki/UTF-16）
                // - 从码点中减去 0x10000
                // - 高位代理项：右移 10 位后加上 0xD800
                // - 低位代理项：取低 10 位后加上 0xDC00
                c -= 0x10000;
                output[pos] = ((c >> 10) + 0xD800) as u16;
                output[pos + 1] = ((c & 0b1111111111) + 0xDC00) as u16;
                i += 4;
                pos += 2;
            }
            // 合法的 UTF-8 不可能有其他取值
            _ => unreachable!(),
        }
    }
    output
}

/// 用于获取 `T` 的大小（以 u32 表示）的辅助方法。
/// 如果该大小会发生溢出，则在编译期报错。
///
/// 虽然类型大于 u32::MAX 不太可能发生，但确有可能，哪怕只是因为出现了 bug。
/// 不过，本函数的一个关键动机是避免动辄使用 `as` 转换的诱惑。这样做有风险，
/// 因为 `as` 太过强大了。例如，下面这行代码今天就能编译通过：
///
/// `size_of::<u64> as u32`
///
/// 注意 `size_of` 其实从未被真正调用，实际上是把一个函数指针转换成了 `u32`。
/// Clippy 会对此发出警告，但很遗憾，它并不会在标准库上运行。
const fn win32_size_of<T: Sized>() -> u32 {
    // 在 const 上下文中断言其大小不超过 u32::MAX。
    // 使用一个 trait 来绕开“不能在内层 item 中使用泛型类型”的限制。
    trait Win32SizeOf: Sized {
        const WIN32_SIZE_OF: u32 = {
            let size = size_of::<Self>();
            assert!(size <= u32::MAX as usize);
            size as u32
        };
    }
    impl<T: Sized> Win32SizeOf for T {}

    T::WIN32_SIZE_OF
}

/// `SetFileInformationByHandle` 函数是泛型的：它要求使用者指定类型（class）、
/// 一个指向数据的指针以及数据的大小。该 trait 允许把这些信息附加到某个 Rust
/// 类型上，从而能够安全地调用 [`set_file_information_by_handle`]。
///
/// 该 trait 在设计上可以支持可变大小的类型。不过目前 Rust 的 std 只使用
/// 固定大小的结构体。
///
/// # 安全性(Safety）
///
/// * `as_ptr` 必须返回一个指向可读取至多 `size` 字节内存的指针。
/// * `CLASS` 必须准确反映 `as_ptr` 所指向的类型。例如，`FILE_BASIC_INFO`
/// 结构体对应的 class 是 `FileBasicInfo`。
pub unsafe trait SetFileInformation {
    /// 要设置的信息类型。
    const CLASS: i32;
    /// 指向要设置的文件信息的指针。
    fn as_ptr(&self) -> *const c_void;
    /// `as_ptr` 所指向类型的大小。
    fn size(&self) -> u32;
}
/// 为静态大小类型实现 `SetFileInformation` 的辅助 trait。
unsafe trait SizedSetFileInformation: Sized {
    const CLASS: i32;
}
unsafe impl<T: SizedSetFileInformation> SetFileInformation for T {
    const CLASS: i32 = T::CLASS;
    fn as_ptr(&self) -> *const c_void {
        (&raw const *self).cast::<c_void>()
    }
    fn size(&self) -> u32 {
        win32_size_of::<Self>()
    }
}

// SAFETY: FILE_BASIC_INFO、FILE_END_OF_FILE_INFO、FILE_ALLOCATION_INFO、
// FILE_DISPOSITION_INFO、FILE_DISPOSITION_INFO_EX 和 FILE_IO_PRIORITY_HINT_INFO
// 都是普通的 `repr(C)` 结构体，且仅包含原始类型。
// 所给定的信息 class 与各结构体正确对应。
unsafe impl SizedSetFileInformation for c::FILE_BASIC_INFO {
    const CLASS: i32 = c::FileBasicInfo;
}
unsafe impl SizedSetFileInformation for c::FILE_END_OF_FILE_INFO {
    const CLASS: i32 = c::FileEndOfFileInfo;
}
unsafe impl SizedSetFileInformation for c::FILE_ALLOCATION_INFO {
    const CLASS: i32 = c::FileAllocationInfo;
}
unsafe impl SizedSetFileInformation for c::FILE_DISPOSITION_INFO {
    const CLASS: i32 = c::FileDispositionInfo;
}
unsafe impl SizedSetFileInformation for c::FILE_DISPOSITION_INFO_EX {
    const CLASS: i32 = c::FileDispositionInfoEx;
}
unsafe impl SizedSetFileInformation for c::FILE_IO_PRIORITY_HINT_INFO {
    const CLASS: i32 = c::FileIoPriorityHintInfo;
}

#[inline]
pub fn set_file_information_by_handle<T: SetFileInformation>(
    handle: c::HANDLE,
    info: &T,
) -> Result<(), WinError> {
    unsafe fn set_info(
        handle: c::HANDLE,
        class: i32,
        info: *const c_void,
        size: u32,
    ) -> Result<(), WinError> {
        unsafe {
            let result = c::SetFileInformationByHandle(handle, class, info, size);
            (result != 0).then_some(()).ok_or_else(get_last_error)
        }
    }
    // SAFETY: `SetFileInformation` trait 确保了这是安全的。
    unsafe { set_info(handle, T::CLASS, info.as_ptr(), info.size()) }
}

/// 获取最近一次函数调用所产生的错误。
/// 必须在设置该错误的函数之后立即调用本函数，以避免另一个函数覆盖它的风险。
pub fn get_last_error() -> WinError {
    // SAFETY: 这只是返回一个线程局部的 u32，没有任何其他副作用。
    unsafe { WinError { code: c::GetLastError() } }
}

/// 由 [`get_last_error`] 返回的错误码。
///
/// 它通常是一个 16 位的 Win32 错误码，但也可能是 32 位的 HRESULT 或 NTSTATUS。
/// 请查阅所调用 Windows API 函数的文档，以了解可能出现的错误。
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct WinError {
    pub code: u32,
}
impl WinError {
    pub const fn new(code: u32) -> Self {
        Self { code }
    }
}

// 错误码常量。
// 这些常量名应当与 winapi 中的常量一致，只是去掉了前导的 `ERROR_`。
// 由于错误码数量庞大，仅应在确有需要时才往这里添加错误码。
// 但是，已添加的错误码永远不应被删除，因为我们假设它们将来可能会再次有用。
#[allow(unused)]
impl WinError {
    /// 成功并不是错误。
    /// 某些 Windows API 确实用它来区分“返回零”和“返回错误”这两种情况，
    /// 但我们绝不应把它作为错误返回给用户。
    pub const SUCCESS: Self = Self::new(c::ERROR_SUCCESS);
    // tidy-alphabetical-start
    pub const ACCESS_DENIED: Self = Self::new(c::ERROR_ACCESS_DENIED);
    pub const ALREADY_EXISTS: Self = Self::new(c::ERROR_ALREADY_EXISTS);
    pub const BAD_NETPATH: Self = Self::new(c::ERROR_BAD_NETPATH);
    pub const BAD_NET_NAME: Self = Self::new(c::ERROR_BAD_NET_NAME);
    pub const CANT_ACCESS_FILE: Self = Self::new(c::ERROR_CANT_ACCESS_FILE);
    pub const DELETE_PENDING: Self = Self::new(c::ERROR_DELETE_PENDING);
    pub const DIRECTORY: Self = Self::new(c::ERROR_DIRECTORY);
    pub const DIR_NOT_EMPTY: Self = Self::new(c::ERROR_DIR_NOT_EMPTY);
    pub const FILE_NOT_FOUND: Self = Self::new(c::ERROR_FILE_NOT_FOUND);
    pub const INSUFFICIENT_BUFFER: Self = Self::new(c::ERROR_INSUFFICIENT_BUFFER);
    pub const INVALID_FUNCTION: Self = Self::new(c::ERROR_INVALID_FUNCTION);
    pub const INVALID_HANDLE: Self = Self::new(c::ERROR_INVALID_HANDLE);
    pub const INVALID_PARAMETER: Self = Self::new(c::ERROR_INVALID_PARAMETER);
    pub const NOT_FOUND: Self = Self::new(c::ERROR_NOT_FOUND);
    pub const NOT_SUPPORTED: Self = Self::new(c::ERROR_NOT_SUPPORTED);
    pub const NO_MORE_FILES: Self = Self::new(c::ERROR_NO_MORE_FILES);
    pub const OPERATION_ABORTED: Self = Self::new(c::ERROR_OPERATION_ABORTED);
    pub const PATH_NOT_FOUND: Self = Self::new(c::ERROR_PATH_NOT_FOUND);
    pub const SHARING_VIOLATION: Self = Self::new(c::ERROR_SHARING_VIOLATION);
    pub const TIMEOUT: Self = Self::new(c::ERROR_TIMEOUT);
    // tidy-alphabetical-end
}

/// 对 UNICODE_STRING 的封装，等价于 `&[u16]`。
///
/// 更推荐使用 `unicode_str!` 宏，因为它包含针对 #143078 的缓解措施。
///
/// 如果底层 UNICODE_STRING 的 MaximumLength 字段大于 Length 字段，
/// 那么你可以通过检查紧跟在字符串之后的那个 u16 来测试该字符串是否以 null 结尾。
/// 除此之外，你不能依赖于它以 null 结尾。
#[derive(Copy, Clone)]
pub struct UnicodeStrRef<'a> {
    s: c::UNICODE_STRING,
    lifetime: PhantomData<&'a [u16]>,
}

static EMPTY_STRING_NULL_TERMINATED: &[u16] = &[0];

impl UnicodeStrRef<'_> {
    const fn new(slice: &[u16], is_null_terminated: bool) -> Self {
        let (len, max_len, ptr) = if slice.is_empty() {
            (0, 2, EMPTY_STRING_NULL_TERMINATED.as_ptr().cast_mut())
        } else {
            let len = slice.len() - (is_null_terminated as usize);
            (len * 2, size_of_val(slice), slice.as_ptr().cast_mut())
        };
        Self {
            s: c::UNICODE_STRING { Length: len as _, MaximumLength: max_len as _, Buffer: ptr },
            lifetime: PhantomData,
        }
    }

    pub const fn from_slice_with_nul(slice: &[u16]) -> Self {
        if !slice.is_empty() {
            debug_assert!(slice[slice.len() - 1] == 0);
        }
        Self::new(slice, true)
    }

    pub const fn from_slice(slice: &[u16]) -> Self {
        Self::new(slice, false)
    }

    /// 返回指向底层 UNICODE_STRING 的指针
    pub const fn as_ptr(&self) -> *const c::UNICODE_STRING {
        &self.s
    }
}

/// 从一个字面量 str 或一个 u16 数组创建 UnicodeStringRef。
///
/// 为缓解 #143078，当使用字面量 str 时，所创建的 UNICODE_STRING 会以 null 结尾。
/// 该 UNICODE_STRING 的 MaximumLength 字段会被设置为大于 Length 字段，
/// 以表明其后可能存在一个 null。
///
/// 如果使用 u16 数组，则该数组会被原样使用，你不能指望该字符串以 null 结尾。
/// 这种用法通常适用于来自操作系统的字符串。
///
/// **NOTE:** 我们没有提供 UNICODE_STRING 的构建器类型，因为目前还用不到它。
/// 如果需要动态构建 UNICODE_STRING，构建器应尽量确保在字符串末尾的下一个位置
/// 存在一个 null。
pub macro unicode_str {
    ($str:literal) => {const {
        crate::sys::pal::windows::api::UnicodeStrRef::from_slice_with_nul(
            crate::sys::pal::windows::api::wide_str!($str),
        )
    }},
    ($array:expr) => {
        crate::sys::pal::windows::api::UnicodeStrRef::from_slice(
            $array,
        )
    }
}
