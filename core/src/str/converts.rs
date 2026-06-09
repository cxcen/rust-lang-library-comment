//! 从字节切片创建 `str` 的入口。
//!
//! `&str` 的核心类型不变量是：底层字节必须始终是合法 UTF-8。安全构造函数会先验证字节；
//! unchecked 构造函数则把这个责任转交给调用方，一旦前置条件不成立，后续所有按 UTF-8
//! 解码的操作都可能触发 UB。

use super::Utf8Error;
use super::validations::run_utf8_validation;
use crate::{mem, ptr};

/// 将字节切片转换为字符串切片。
///
/// 这是 [`str::from_utf8`] 的别名。
///
/// 字符串切片（[`&str`]）和字节切片（[`&[u8]`][byteslice]）底层都由 [`u8`] 组成，
/// 但 `&str` 额外承诺其字节是合法 UTF-8。并非所有字节切片都满足该不变量；
/// `from_utf8()` 会先完整验证 UTF-8，再在不复制数据的情况下改变借用类型。
///
/// [`&str`]: str
/// [byteslice]: slice
///
/// 如果已经确信字节切片是合法 UTF-8，并且不想承担验证开销，可以使用 unsafe 版本
/// [`from_utf8_unchecked`]。它行为相同但跳过检查；调用方必须维护 `&str`
/// 的 UTF-8 有效性不变量。
///
/// 如果需要的是拥有所有权的 `String` 而不是借用的 `&str`，可考虑
/// [`String::from_utf8`][string]。
///
/// [string]: ../../std/string/struct.String.html#method.from_utf8
///
/// 由于 `[u8; N]` 可以分配在栈上，并可借用为 [`&[u8]`][byteslice]，
/// 该函数也能用于创建借用栈上字节数组的字符串切片。下面的示例展示了这种用法。
///
/// [byteslice]: slice
///
/// # 错误
///
/// 如果切片不是合法 UTF-8，则返回 `Err`；错误中会记录已验证的有效前缀长度，
/// 以及失败的字节序列长度（若可确定）。
///
/// # 示例
///
/// 基本用法：
///
/// ```
/// use std::str;
///
/// // vector 中的一些字节。
/// let sparkle_heart = vec![240, 159, 146, 150];
///
/// // 可以使用 ?（try）运算符检查这些字节是否是合法 UTF-8。
/// let sparkle_heart = str::from_utf8(&sparkle_heart)?;
///
/// assert_eq!("💖", sparkle_heart);
/// # Ok::<_, str::Utf8Error>(())
/// ```
///
/// 非法字节：
///
/// ```
/// use std::str;
///
/// // vector 中的一些非法字节。
/// let sparkle_heart = vec![0, 159, 146, 150];
///
/// assert!(str::from_utf8(&sparkle_heart).is_err());
/// ```
///
/// 可返回的错误种类详见 [`Utf8Error`] 文档。
///
/// “栈上分配的字符串”：
///
/// ```
/// use std::str;
///
/// // 栈上数组中的一些字节。
/// let sparkle_heart = [240, 159, 146, 150];
///
/// // 已知这些字节合法，因此直接使用 `unwrap()`。
/// let sparkle_heart: &str = str::from_utf8(&sparkle_heart).unwrap();
///
/// assert_eq!("💖", sparkle_heart);
/// ```
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_stable(feature = "const_str_from_utf8_shared", since = "1.63.0")]
#[rustc_diagnostic_item = "str_from_utf8"]
pub const fn from_utf8(v: &[u8]) -> Result<&str, Utf8Error> {
    // FIXME(const-hack): 当 `?` 可再次用于 const 后，恢复使用它。
    match run_utf8_validation(v) {
        Ok(_) => {
            // SAFETY: `run_utf8_validation` 已验证 `v` 是合法 UTF-8，满足 `from_utf8_unchecked` 前置条件。
            Ok(unsafe { from_utf8_unchecked(v) })
        }
        Err(err) => Err(err),
    }
}

/// 将可变字节切片转换为可变字符串切片。
///
/// 这是 [`str::from_utf8_mut`] 的别名。成功后，返回的 `&mut str` 仍指向同一段字节，
/// 但类型保证调用方只能通过维护 UTF-8 有效性的字符串接口使用它。
///
/// # 示例
///
/// 基本用法：
///
/// ```
/// use std::str;
///
/// // 以可变 vector 表示的 "Hello, Rust!"。
/// let mut hellorust = vec![72, 101, 108, 108, 111, 44, 32, 82, 117, 115, 116, 33];
///
/// // 已知这些字节合法，因此可以使用 `unwrap()`。
/// let outstr = str::from_utf8_mut(&mut hellorust).unwrap();
///
/// assert_eq!("Hello, Rust!", outstr);
/// ```
///
/// 非法字节：
///
/// ```
/// use std::str;
///
/// // 可变 vector 中的一些非法字节。
/// let mut invalid = vec![128, 223];
///
/// assert!(str::from_utf8_mut(&mut invalid).is_err());
/// ```
/// 可返回的错误种类详见 [`Utf8Error`] 文档。
#[stable(feature = "str_mut_extras", since = "1.20.0")]
#[rustc_const_stable(feature = "const_str_from_utf8", since = "1.87.0")]
#[rustc_diagnostic_item = "str_from_utf8_mut"]
pub const fn from_utf8_mut(v: &mut [u8]) -> Result<&mut str, Utf8Error> {
    // FIXME(const-hack): 当 `?` 可再次用于 const 后，恢复使用它。
    match run_utf8_validation(v) {
        Ok(_) => {
            // SAFETY: `run_utf8_validation` 已验证 `v` 是合法 UTF-8，满足可变 unchecked 转换前置条件。
            Ok(unsafe { from_utf8_unchecked_mut(v) })
        }
        Err(err) => Err(err),
    }
}

/// 不检查 UTF-8 有效性，直接把字节切片转换为字符串切片。
///
/// 这是 [`str::from_utf8_unchecked`] 的别名。
///
/// 安全版本见 [`from_utf8`]。
///
/// # 安全性(Safety）
///
/// 传入字节必须是合法 UTF-8，并且在返回的 `&str` 生命周期内保持该不变量。
/// 这不是普通逻辑错误：构造无效 `&str` 会破坏 Rust 对字符串切片的类型假设，
/// 使后续按字符边界索引、迭代 `char` 或调用 `str` 方法时可能发生 UB。
///
/// # 示例
///
/// 基本用法：
///
/// ```
/// use std::str;
///
/// // vector 中的一些字节。
/// let sparkle_heart = vec![240, 159, 146, 150];
///
/// let sparkle_heart = unsafe {
///     str::from_utf8_unchecked(&sparkle_heart)
/// };
///
/// assert_eq!("💖", sparkle_heart);
/// ```
#[inline]
#[must_use]
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_stable(feature = "const_str_from_utf8_unchecked", since = "1.55.0")]
#[rustc_diagnostic_item = "str_from_utf8_unchecked"]
pub const unsafe fn from_utf8_unchecked(v: &[u8]) -> &str {
    // SAFETY: 调用方必须保证 `v` 的字节是合法 UTF-8；`&str` 与 `&[u8]`
    // 的胖指针布局相同，因此可在保持地址和长度不变的情况下转换引用类型。
    unsafe { mem::transmute(v) }
}

/// 不检查 UTF-8 有效性，直接把可变字节切片转换为可变字符串切片。
///
/// 这是 [`str::from_utf8_unchecked_mut`] 的别名。
///
/// 文档和安全要求见不可变版本 [`from_utf8_unchecked()`]。
///
/// # 示例
///
/// 基本用法：
///
/// ```
/// use std::str;
///
/// let mut heart = vec![240, 159, 146, 150];
/// let heart = unsafe { str::from_utf8_unchecked_mut(&mut heart) };
///
/// assert_eq!("💖", heart);
/// ```
#[inline]
#[must_use]
#[stable(feature = "str_mut_extras", since = "1.20.0")]
#[rustc_const_stable(feature = "const_str_from_utf8_unchecked_mut", since = "1.83.0")]
#[rustc_diagnostic_item = "str_from_utf8_unchecked_mut"]
pub const unsafe fn from_utf8_unchecked_mut(v: &mut [u8]) -> &mut str {
    // SAFETY: 调用方必须保证 `v` 的字节是合法 UTF-8，因此转换为 `*mut str` 不会破坏
    // `str` 的有效性不变量。该指针来自有效的 `&mut [u8]`，所以在原长度范围内可写。
    unsafe { &mut *(v as *mut [u8] as *mut str) }
}

/// 从指针和长度创建 `&str`。
///
/// 指针指向的 `len` 个字节必须是合法 UTF-8。如果不能确定这一点，请使用
/// `str::from_utf8(slice::from_raw_parts(ptr, len))`，它会在数据不是合法 UTF-8 时返回 `Err`。
///
/// 该函数是 [`slice::from_raw_parts`](crate::slice::from_raw_parts) 的 `str` 版本。
/// 除了 UTF-8 不变量外，还必须满足该函数关于指针有效性、对齐、长度和生命周期的全部安全要求。
///
/// 可变版本是 [`from_raw_parts_mut`]。
#[inline]
#[must_use]
#[unstable(feature = "str_from_raw_parts", issue = "119206")]
pub const unsafe fn from_raw_parts<'a>(ptr: *const u8, len: usize) -> &'a str {
    // SAFETY: 调用方必须维护 `from_raw_parts` 的完整安全契约，包括 UTF-8 有效性和原始切片有效性。
    unsafe { &*ptr::from_raw_parts(ptr, len) }
}

/// 从指针和长度创建 `&mut str`。
///
/// 指针指向的 `len` 个字节必须是合法 UTF-8。如果不能确定这一点，请使用
/// `str::from_utf8_mut(slice::from_raw_parts_mut(ptr, len))`，
/// 它会在数据不是合法 UTF-8 时返回 `Err`。
///
/// 该函数是 [`slice::from_raw_parts_mut`](crate::slice::from_raw_parts_mut) 的 `str` 版本。
/// 除了 UTF-8 不变量外，还必须满足该函数关于独占访问、指针有效性、长度和生命周期的全部安全要求。
///
/// 不可变版本是 [`from_raw_parts`]。
#[inline]
#[must_use]
#[unstable(feature = "str_from_raw_parts", issue = "119206")]
pub const unsafe fn from_raw_parts_mut<'a>(ptr: *mut u8, len: usize) -> &'a mut str {
    // SAFETY: 调用方必须维护 `from_raw_parts_mut` 的完整安全契约，包括 UTF-8 有效性和独占可写访问。
    unsafe { &mut *ptr::from_raw_parts_mut(ptr, len) }
}
