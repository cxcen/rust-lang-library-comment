//! C 定义的平台相关类型。
//!
//! 通过 FFI 交互的代码几乎一定会用到 C 提供的基本类型。与 Rust 基本类型不同,
//! 这些 C 类型的大小和符号性往往取决于平台 ABI。本模块提供与 C 定义匹配的类型,
//! 让同 C 交互的代码能引用正确的 ABI 类型。

#![stable(feature = "core_ffi", since = "1.30.0")]
#![allow(non_camel_case_types)]

#[doc(inline)]
#[stable(feature = "core_c_str", since = "1.64.0")]
pub use self::c_str::CStr;
#[doc(inline)]
#[stable(feature = "cstr_from_bytes_until_nul", since = "1.69.0")]
pub use self::c_str::FromBytesUntilNulError;
#[doc(inline)]
#[stable(feature = "core_c_str", since = "1.64.0")]
pub use self::c_str::FromBytesWithNulError;
use crate::fmt;

#[stable(feature = "c_str_module", since = "1.88.0")]
pub mod c_str;

#[unstable(
    feature = "c_variadic",
    issue = "44930",
    reason = "the `c_variadic` feature has not been properly tested on all supported platforms"
)]
pub use self::va_list::{VaArgSafe, VaList};

#[unstable(
    feature = "c_variadic",
    issue = "44930",
    reason = "the `c_variadic` feature has not been properly tested on all supported platforms"
)]
pub mod va_list;

mod primitives;
#[stable(feature = "core_ffi_c", since = "1.64.0")]
pub use self::primitives::{
    c_char, c_double, c_float, c_int, c_long, c_longlong, c_schar, c_short, c_uchar, c_uint,
    c_ulong, c_ulonglong, c_ushort,
};
#[unstable(feature = "c_size_t", issue = "88345")]
pub use self::primitives::{c_ptrdiff_t, c_size_t, c_ssize_t};

// 注意:为了让 LLVM 识别 void 指针类型,进而识别 malloc() 这类函数,需要在 LLVM bitcode
// 中把它表示为 i8*。这里使用 enum 可保证这一点,并通过只有私有变体来防止误用“原始”类型。
// 需要两个变体,因为否则编译器会抱怨 repr 属性;同时也至少需要一个变体,否则 enum 会无人居留,
// 至少解引用这类指针会成为 UB。
#[doc = include_str!("c_void.md")]
#[lang = "c_void"]
#[repr(u8)]
#[stable(feature = "core_c_void", since = "1.30.0")]
pub enum c_void {
    #[unstable(
        feature = "c_void_variant",
        reason = "temporary implementation detail",
        issue = "none"
    )]
    #[doc(hidden)]
    __variant1,
    #[unstable(
        feature = "c_void_variant",
        reason = "temporary implementation detail",
        issue = "none"
    )]
    #[doc(hidden)]
    __variant2,
}

#[stable(feature = "std_debug", since = "1.16.0")]
impl fmt::Debug for c_void {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("c_void").finish()
    }
}

// 链接 MSVC 默认库。
#[cfg(all(windows, target_env = "msvc"))]
#[link(
    name = "/defaultlib:msvcrt",
    modifiers = "+verbatim",
    cfg(not(target_feature = "crt-static"))
)]
#[link(name = "/defaultlib:libcmt", modifiers = "+verbatim", cfg(target_feature = "crt-static"))]
unsafe extern "C" {}
