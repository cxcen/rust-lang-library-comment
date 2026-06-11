//! 与 FFI 绑定相关的实用工具。
//!
//! 本模块提供了在非 Rust 接口（例如其他编程语言以及底层操作系统）之间
//! 处理数据的实用工具。它主要用于 FFI（Foreign Function Interface，外部
//! 函数接口）绑定，以及需要与其他语言交换 C 风格字符串的代码。
//!
//! # Overview
//!
//! Rust 用 [`String`] 类型表示拥有所有权的字符串，用 [`str`] 原生类型
//! 表示对字符串的借用切片。两者始终采用 UTF-8 编码，并且中间可以包含
//! nul 字节，也就是说，如果你查看组成字符串的那些字节，其中可能存在
//! `\0`。`String` 和 `str` 都显式存储自己的长度；它们不像 C 那样在字符串
//! 末尾使用 nul 终止符。
//!
//! C 字符串与 Rust 字符串有所不同：
//!
//! * **Encodings** - Rust 字符串是 UTF-8，但 C 字符串可能使用其他编码。
//! 如果你使用来自 C 的字符串，应当显式检查它的编码，而不是像在 Rust 中
//! 那样直接假设它是 UTF-8。
//!
//! * **Character size** - C 字符串可能使用 `char` 或 `wchar_t` 大小的字符；
//! 请**注意**：C 的 `char` 与 Rust 的 `char` 是不同的。C 标准并未规定这些
//! 类型的实际大小，而是留给具体实现去解释，但为由每种字符类型组成的字符串
//! 定义了不同的 API。Rust 字符串始终是 UTF-8，因此不同的 Unicode 字符各自
//! 会被编码为数量不等的若干字节。Rust 类型 [`char`] 表示一个 '[Unicode scalar
//! value]'（Unicode 标量值），它与 '[Unicode code point]'（Unicode 码点）
//! 相似，但并不相同。
//!
//! * **Nul terminators and implicit string lengths** - C 字符串通常以 nul
//! 结尾，也就是说它们在末尾有一个 `\0` 字符。字符串缓冲区的长度并不被
//! 存储，而必须计算得到；为了计算字符串的长度，C 代码必须手动调用类似
//! `strlen()`（针对基于 `char` 的字符串）或 `wcslen()`（针对基于 `wchar_t`
//! 的字符串）这样的函数。这些函数返回不包括 nul 终止符在内的字符数，因此
//! 缓冲区长度实际上是 `len+1` 个字符。Rust 字符串没有 nul 终止符；它们的
//! 长度始终被存储，无需计算。在 Rust 中访问字符串长度是一个 *O*(1) 操作
//! （因为长度被存储了）；而在 C 中则是 *O*(*n*) 操作，因为需要通过扫描
//! 字符串查找 nul 终止符来计算长度。
//!
//! * **Internal nul characters** - 当 C 字符串带有 nul 终止符时，这通常
//! 意味着它们中间不能含有 nul 字符——一个 nul 字符本质上会截断字符串。
//! Rust 字符串*可以*在中间含有 nul 字符，因为在 Rust 中 nul 不必标记
//! 字符串的结尾。
//!
//! # Representations of non-Rust strings
//!
//! 当你需要在具有 C ABI 的语言（例如 Python）之间传入和传出 UTF-8 字符串
//! 时，[`CString`] 和 [`CStr`] 很有用。
//!
//! * **From Rust to C:** [`CString`] 表示一个拥有所有权、对 C 友好的
//! 字符串：它以 nul 结尾，且内部没有 nul 字符。Rust 代码可以从普通字符串
//! 创建一个 [`CString`]（前提是该字符串中间没有 nul 字符），然后使用各种
//! 方法获得一个裸 <code>\*mut [u8]</code>，进而作为参数传给采用 C 字符串
//! 约定的函数。
//!
//! * **From C to Rust:** [`CStr`] 表示一个借用的 C 字符串；当你要包装一个
//! 从 C 函数得到的裸 <code>\*const [u8]</code> 时就会用到它。[`CStr`] 保证
//! 是一个以 nul 结尾的字节数组。一旦得到了 [`CStr`]，如果它是有效的 UTF-8，
//! 就可以把它转换成 Rust 的 <code>&[str]</code>，或者通过添加替换字符进行
//! 有损转换。
//!
//! 当你需要在操作系统自身之间传入和传出字符串，或者捕获外部命令的输出时，
//! [`OsString`] 和 [`OsStr`] 很有用。[`OsString`]、[`OsStr`] 与 Rust 字符串
//! 之间的转换，工作方式与 [`CString`] 和 [`CStr`] 的转换类似。
//!
//! * [`OsString`] 无损地表示一个拥有所有权的平台字符串。然而，这种表示
//! 不一定是平台原生的形式。在 Rust 标准库中，各种在操作系统之间传入/传出
//! 字符串的 API 使用 [`OsString`] 而非普通字符串。例如，[`env::var_os()`]
//! 用于查询环境变量；它返回一个 <code>[Option]<[OsString]></code>。如果该
//! 环境变量存在，你会得到一个 <code>[Some]\(os_string)</code>，*然后*你可以
//! 尝试把它转换成 Rust 字符串。该转换得到一个 [`Result`]，这样在环境变量
//! 实际上并不包含有效 Unicode 数据时，你的代码就能检测到错误。
//!
//! * [`OsStr`] 无损地表示对一个平台字符串的借用引用。然而，这种表示不一定
//! 是平台原生的形式。它能以类似于 [`OsString`] 的方式转换成 UTF-8 的 Rust
//! 字符串切片。
//!
//! # Conversions
//!
//! ## On Unix
//!
//! 在 Unix 上，[`OsStr`] 实现了
//! <code>std::os::unix::ffi::[OsStrExt][unix.OsStrExt]</code> trait，它为
//! [`OsStr`] 增加了两个方法：[`from_bytes`] 和 [`as_bytes`]。这些方法在
//! 字节切片与 [`OsStr`] 之间进行廉价的转换。
//!
//! 此外，在 Unix 上 [`OsString`] 实现了
//! <code>std::os::unix::ffi::[OsStringExt][unix.OsStringExt]</code> trait，
//! 它提供 [`from_vec`] 和 [`into_vec`] 方法，这两个方法会消耗其参数，
//! 并接受或产出 [`u8`] 向量。
//!
//! ## On Windows
//!
//! [`OsStr`] 可以无损地转换为原生 Windows 字符串。原生 Windows 字符串也
//! 可以无损地转换为 [`OsString`]。
//!
//! 在 Windows 上，[`OsStr`] 实现了
//! <code>std::os::windows::ffi::[OsStrExt][windows.OsStrExt]</code> trait，
//! 它提供一个 [`encode_wide`] 方法。该方法提供一个迭代器，可以被
//! [`collect`] 成一个 [`u16`] 向量。在追加一个 nul 字符之后，它就与原生
//! Windows 字符串相同了。
//!
//! 此外，在 Windows 上 [`OsString`] 实现了
//! <code>std::os::windows:ffi::[OsStringExt][windows.OsStringExt]</code>
//! trait，它提供一个 [`from_wide`] 方法，用于把一个原生 Windows 字符串
//! （不含结尾的 nul 字符）转换成 [`OsString`]。
//!
//! ## Other platforms
//!
//! 许多其他平台在 `std::os::*::ffi` 模块中提供它们各自的扩展 trait。
//!
//! ## On all platforms
//!
//! 在所有平台上，[`OsStr`] 都由一串字节组成，这串字节被编码为 UTF-8 的
//! 一个超集；关于它在不同平台上的编码的更多细节，参见 [`OsString`]。
//!
//! 关于字节与 [`OsStr`] 之间有限且廉价的转换，参见 [`OsStr::as_encoded_bytes`]
//! 和 [`OsStr::from_encoded_bytes_unchecked`]。
//!
//! 关于基本的字符串处理，参见 [`OsStr::slice_encoded_bytes`]。
//!
//! [Unicode scalar value]: https://www.unicode.org/glossary/#unicode_scalar_value
//! [Unicode code point]: https://www.unicode.org/glossary/#code_point
//! [`env::set_var()`]: crate::env::set_var "env::set_var"
//! [`env::var_os()`]: crate::env::var_os "env::var_os"
//! [unix.OsStringExt]: crate::os::unix::ffi::OsStringExt "os::unix::ffi::OsStringExt"
//! [`from_vec`]: crate::os::unix::ffi::OsStringExt::from_vec "os::unix::ffi::OsStringExt::from_vec"
//! [`into_vec`]: crate::os::unix::ffi::OsStringExt::into_vec "os::unix::ffi::OsStringExt::into_vec"
//! [unix.OsStrExt]: crate::os::unix::ffi::OsStrExt "os::unix::ffi::OsStrExt"
//! [`from_bytes`]: crate::os::unix::ffi::OsStrExt::from_bytes "os::unix::ffi::OsStrExt::from_bytes"
//! [`as_bytes`]: crate::os::unix::ffi::OsStrExt::as_bytes "os::unix::ffi::OsStrExt::as_bytes"
//! [`OsStrExt`]: crate::os::unix::ffi::OsStrExt "os::unix::ffi::OsStrExt"
//! [windows.OsStrExt]: crate::os::windows::ffi::OsStrExt "os::windows::ffi::OsStrExt"
//! [`encode_wide`]: crate::os::windows::ffi::OsStrExt::encode_wide "os::windows::ffi::OsStrExt::encode_wide"
//! [`collect`]: crate::iter::Iterator::collect "iter::Iterator::collect"
//! [windows.OsStringExt]: crate::os::windows::ffi::OsStringExt "os::windows::ffi::OsStringExt"
//! [`from_wide`]: crate::os::windows::ffi::OsStringExt::from_wide "os::windows::ffi::OsStringExt::from_wide"

#![stable(feature = "rust1", since = "1.0.0")]

#[stable(feature = "c_str_module", since = "1.88.0")]
pub mod c_str;

#[stable(feature = "core_c_void", since = "1.30.0")]
pub use core::ffi::c_void;
#[unstable(
    feature = "c_variadic",
    reason = "the `c_variadic` feature has not been properly tested on \
              all supported platforms",
    issue = "44930"
)]
pub use core::ffi::{VaArgSafe, VaList};
#[stable(feature = "core_ffi_c", since = "1.64.0")]
pub use core::ffi::{
    c_char, c_double, c_float, c_int, c_long, c_longlong, c_schar, c_short, c_uchar, c_uint,
    c_ulong, c_ulonglong, c_ushort,
};
#[unstable(feature = "c_size_t", issue = "88345")]
pub use core::ffi::{c_ptrdiff_t, c_size_t, c_ssize_t};

#[doc(inline)]
#[stable(feature = "cstr_from_bytes_until_nul", since = "1.69.0")]
pub use self::c_str::FromBytesUntilNulError;
#[doc(inline)]
#[stable(feature = "cstr_from_bytes", since = "1.10.0")]
pub use self::c_str::FromBytesWithNulError;
#[doc(inline)]
#[stable(feature = "cstring_from_vec_with_nul", since = "1.58.0")]
pub use self::c_str::FromVecWithNulError;
#[doc(inline)]
#[stable(feature = "cstring_into", since = "1.7.0")]
pub use self::c_str::IntoStringError;
#[doc(inline)]
#[stable(feature = "rust1", since = "1.0.0")]
pub use self::c_str::NulError;
#[doc(inline)]
#[stable(feature = "rust1", since = "1.0.0")]
pub use self::c_str::{CStr, CString};
#[stable(feature = "rust1", since = "1.0.0")]
#[doc(inline)]
pub use self::os_str::{OsStr, OsString};

#[stable(feature = "os_str_display", since = "1.87.0")]
pub mod os_str;
