//! [`OsStr`] 和 [`OsString`] 类型及相关实用工具。

#[cfg(test)]
mod tests;

use core::clone::CloneToUninit;

use crate::borrow::{Borrow, Cow};
use crate::collections::TryReserveError;
use crate::hash::{Hash, Hasher};
use crate::ops::{self, Range};
use crate::rc::Rc;
use crate::str::FromStr;
use crate::sync::Arc;
use crate::sys::os_str::{Buf, Slice};
use crate::sys::{AsInner, FromInner, IntoInner};
use crate::{cmp, fmt, slice};

/// 一种能够表示拥有所有权、可变的平台原生字符串的类型，同时它与 Rust
/// 字符串之间可以廉价地相互转换。
///
/// 这种类型的需求源于以下事实：
///
/// * 在 Unix 系统上，字符串往往是任意的非零字节序列，在许多情况下被
///   解释为 UTF-8。
///
/// * 在 Windows 上，字符串往往是任意的非零 16 位值序列，在可以这样解释
///   时被解释为 UTF-16。
///
/// * 在 Rust 中，字符串始终是有效的 UTF-8，其中可能含有零。
///
/// `OsString` 和 [`OsStr`] 通过同时表示 Rust 字符串值和平台原生字符串值
/// 来弥合这一鸿沟，特别是在可能的情况下允许把 Rust 字符串零开销地转换成
/// 一个 "OS" 字符串。其结果之一是，`OsString` 实例*不是* `NUL` 结尾的；
/// 为了传给例如 Unix 系统调用，你应该创建一个 [`CStr`]。
///
/// `OsString` 之于 <code>&[OsStr]</code> 就如同 [`String`] 之于
/// <code>&[str]</code>：每一对中前者是拥有所有权的字符串；后者是借用的引用。
///
/// 注意，`OsString` 和 [`OsStr`] 在内部不一定以平台原生的形式持有字符串；
/// 在 Unix 上，字符串以一串 8 位值的形式存储，而在 Windows 上，如前所述
/// 字符串基于 16 位值，但实际上同样以一串 8 位值的形式存储，采用 UTF-8 的
/// 一个不那么严格的变体编码（即 WTF-8）。在处理容量和长度值时，理解这一点
/// 很有用。
///
/// # Capacity of `OsString`
///
/// 对于由有效 unicode 创建的 OS 字符串，容量以 UTF-8 字节为单位；对于其他
/// 内容，则以某种未指定编码下的字节为单位。在给定的目标平台上，所有
/// `OsString` 和 `OsStr` 值的容量都使用相同的单位，因此下面的代码可以正常
/// 工作：
/// ```
/// use std::ffi::{OsStr, OsString};
///
/// fn concat_os_strings(a: &OsStr, b: &OsStr) -> OsString {
///     let mut ret = OsString::with_capacity(a.len() + b.len()); // 这里会分配内存
///     ret.push(a); // 这里不会再分配
///     ret.push(b); // 这里不会再分配
///     ret
/// }
/// ```
///
/// # Creating an `OsString`
///
/// **From a Rust string**：`OsString` 实现了
/// <code>[From]<[String]></code>，因此你可以使用
/// <code>my_string.[into]\()</code> 从一个普通的 Rust 字符串创建一个
/// `OsString`。
///
/// **From slices:** 正如你可以从一个空的 Rust [`String`] 开始，然后用
/// [`String::push_str`] 把若干 <code>&[str]</code> 子串切片推入其中一样，
/// 你也可以用 [`OsString::new`] 方法创建一个空的 `OsString`，然后用
/// [`OsString::push`] 方法把字符串切片推入其中。
///
/// # Extracting a borrowed reference to the whole OS string
///
/// 你可以使用 [`OsString::as_os_str`] 方法从一个 `OsString` 得到一个
/// <code>&[OsStr]</code>；这实际上是对整个字符串的借用引用。
///
/// # Conversions
///
/// 关于 `OsString` 为从/到原生表示进行 [conversions] 而实现的那些 trait
/// 的讨论，参见[模块顶层关于转换的文档][conversions]。
///
/// [`CStr`]: crate::ffi::CStr
/// [conversions]: super#conversions
/// [into]: Into::into
#[cfg_attr(not(test), rustc_diagnostic_item = "OsString")]
#[stable(feature = "rust1", since = "1.0.0")]
pub struct OsString {
    inner: Buf,
}

/// 允许在 `std` 内部使用扩展 trait。
#[unstable(feature = "sealed", issue = "none")]
impl crate::sealed::Sealed for OsString {}

/// 对一个 OS 字符串的借用引用（参见 [`OsString`]）。
///
/// 这种类型表示对一个字符串的借用引用，该字符串采用操作系统偏好的表示形式。
///
/// `&OsStr` 之于 [`OsString`] 就如同 <code>&[str]</code> 之于 [`String`]：
/// 每一对中前者是借用的引用；后者是拥有所有权的字符串。
///
/// 关于 `OsStr` 为从/到原生表示进行 [conversions] 而实现的那些 trait 的
/// 讨论，参见[模块顶层关于转换的文档][conversions]。
///
/// [conversions]: super#conversions
#[cfg_attr(not(test), rustc_diagnostic_item = "OsStr")]
#[stable(feature = "rust1", since = "1.0.0")]
// `OsStr::from_inner` 以及 `impl CloneToUninit for OsStr` 当前的实现依赖于
// `OsStr` 与 `Slice` 在内存布局上兼容。
// 但是，`OsStr` 的布局被视为实现细节，绝不应被依赖。
#[repr(transparent)]
pub struct OsStr {
    inner: Slice,
}

/// 允许在 `std` 内部使用扩展 trait。
#[unstable(feature = "sealed", issue = "none")]
impl crate::sealed::Sealed for OsStr {}

impl OsString {
    /// 构造一个新的空 `OsString`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::ffi::OsString;
    ///
    /// let os_string = OsString::new();
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[must_use]
    #[inline]
    #[rustc_const_stable(feature = "const_pathbuf_osstring_new", since = "1.91.0")]
    pub const fn new() -> OsString {
        OsString { inner: Buf::from_string(String::new()) }
    }

    /// 把字节转换为 `OsString`，不检查这些字节是否含有有效的 [`OsStr`] 编码
    /// 数据。
    ///
    /// 该字节编码是 UTF-8 的一个未指定的、平台相关的、自同步（self-synchronizing）
    /// 超集。由于它是 UTF-8 的一个自同步超集，因此该编码也是 7 位 ASCII 的
    /// 超集。
    ///
    /// 关于从/到原生表示进行安全、跨平台的 [conversions]，参见[模块顶层关于
    /// 转换的文档][conversions]。
    ///
    /// # 安全性(Safety）
    ///
    /// 由于编码是未指定的，调用者必须传入这样的字节：它们源自经过校验的
    /// UTF-8 与来自 [`OsStr::as_encoded_bytes`] 的字节的混合，且这些字节是
    /// 在相同 Rust 版本、为相同目标平台构建的环境下产生的。例如，从经网络
    /// 发送或存储于文件中的字节重建一个 `OsString`，很可能会违反这些安全
    /// 规则。
    ///
    /// 由于该编码是自同步的，来自 [`OsStr::as_encoded_bytes`] 的字节可以在
    /// 任意有效的非空 UTF-8 子串的紧前或紧后被切分。
    ///
    /// # Example
    ///
    /// ```
    /// use std::ffi::OsStr;
    ///
    /// let os_str = OsStr::new("Mary had a little lamb");
    /// let bytes = os_str.as_encoded_bytes();
    /// let words = bytes.split(|b| *b == b' ');
    /// let words: Vec<&OsStr> = words.map(|word| {
    ///     // SAFETY：
    ///     // - 每个 `word` 只包含源自 `OsStr::as_encoded_bytes` 的内容
    ///     // - 仅以 ASCII 空白进行切分，而它是一个非空的 UTF-8 子串
    ///     unsafe { OsStr::from_encoded_bytes_unchecked(word) }
    /// }).collect();
    /// ```
    ///
    /// [conversions]: super#conversions
    #[inline]
    #[stable(feature = "os_str_bytes", since = "1.74.0")]
    pub unsafe fn from_encoded_bytes_unchecked(bytes: Vec<u8>) -> Self {
        OsString { inner: unsafe { Buf::from_encoded_bytes_unchecked(bytes) } }
    }

    /// 转换为一个 [`OsStr`] 切片。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::ffi::{OsString, OsStr};
    ///
    /// let os_string = OsString::from("foo");
    /// let os_str = OsStr::new("foo");
    /// assert_eq!(os_string.as_os_str(), os_str);
    /// ```
    #[cfg_attr(not(test), rustc_diagnostic_item = "os_string_as_os_str")]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[must_use]
    #[inline]
    pub fn as_os_str(&self) -> &OsStr {
        self
    }

    /// 把 `OsString` 转换为一个字节向量。要把该字节向量转换回 `OsString`，
    /// 请使用 [`OsString::from_encoded_bytes_unchecked`] 函数。
    ///
    /// 该字节编码是 UTF-8 的一个未指定的、平台相关的、自同步超集。由于它是
    /// UTF-8 的一个自同步超集，因此该编码也是 7 位 ASCII 的超集。
    ///
    /// 注意：由于编码是未指定的，任何并非有效 UTF-8 的字节子切片都应被视为
    /// 不透明的，且只能在相同 Rust 版本、为相同目标平台构建的环境内进行
    /// 比较。例如，把这些字节经网络发送或存储于文件中，很可能会得到不兼容的
    /// 数据。关于编码的更多细节参见 [`OsString`]，关于平台相关的、已明确指定
    /// 的转换参见 [`std::ffi`]。
    ///
    /// [`std::ffi`]: crate::ffi
    #[inline]
    #[stable(feature = "os_str_bytes", since = "1.74.0")]
    pub fn into_encoded_bytes(self) -> Vec<u8> {
        self.inner.into_encoded_bytes()
    }

    /// 如果 `OsString` 含有有效的 Unicode 数据，则把它转换成一个 [`String`]。
    ///
    /// 失败时，返回原始 `OsString` 的所有权。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::ffi::OsString;
    ///
    /// let os_string = OsString::from("foo");
    /// let string = os_string.into_string();
    /// assert_eq!(string, Ok(String::from("foo")));
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn into_string(self) -> Result<String, OsString> {
        self.inner.into_string().map_err(|buf| OsString { inner: buf })
    }

    /// 用给定的 <code>&[OsStr]</code> 切片扩展该字符串。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::ffi::OsString;
    ///
    /// let mut os_string = OsString::from("foo");
    /// os_string.push("bar");
    /// assert_eq!(&os_string, "foobar");
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    #[rustc_confusables("append", "put")]
    pub fn push<T: AsRef<OsStr>>(&mut self, s: T) {
        trait SpecPushTo {
            fn spec_push_to(&self, buf: &mut OsString);
        }

        impl<T: AsRef<OsStr>> SpecPushTo for T {
            #[inline]
            default fn spec_push_to(&self, buf: &mut OsString) {
                buf.inner.push_slice(&self.as_ref().inner);
            }
        }

        // 当字符串是 UTF-8 时，使用更高效的实现。
        macro spec_str($T:ty) {
            impl SpecPushTo for $T {
                #[inline]
                fn spec_push_to(&self, buf: &mut OsString) {
                    buf.inner.push_str(self);
                }
            }
        }
        spec_str!(str);
        spec_str!(String);

        s.spec_push_to(self)
    }

    /// 创建一个至少具有给定容量的新 `OsString`。
    ///
    /// 该字符串将能够容纳至少 `capacity` 个长度单位的其他 OS 字符串而无需
    /// 重新分配。本方法允许分配多于 `capacity` 的单位数。如果 `capacity`
    /// 为 0，该字符串将不会分配内存。
    ///
    /// 关于编码和容量单位的信息，参见 `OsString` 的主文档。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::ffi::OsString;
    ///
    /// let mut os_string = OsString::with_capacity(10);
    /// let capacity = os_string.capacity();
    ///
    /// // 这次 push 不会重新分配
    /// os_string.push("foo");
    ///
    /// assert_eq!(capacity, os_string.capacity());
    /// ```
    #[stable(feature = "osstring_simple_functions", since = "1.9.0")]
    #[must_use]
    #[inline]
    pub fn with_capacity(capacity: usize) -> OsString {
        OsString { inner: Buf::with_capacity(capacity) }
    }

    /// 把 `OsString` 截断到零长度。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::ffi::OsString;
    ///
    /// let mut os_string = OsString::from("foo");
    /// assert_eq!(&os_string, "foo");
    ///
    /// os_string.clear();
    /// assert_eq!(&os_string, "");
    /// ```
    #[stable(feature = "osstring_simple_functions", since = "1.9.0")]
    #[inline]
    pub fn clear(&mut self) {
        self.inner.clear()
    }

    /// 返回该 `OsString` 在不重新分配的情况下所能容纳的容量。
    ///
    /// 关于编码和容量单位的信息，参见 `OsString` 的主文档。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::ffi::OsString;
    ///
    /// let os_string = OsString::with_capacity(10);
    /// assert!(os_string.capacity() >= 10);
    /// ```
    #[stable(feature = "osstring_simple_functions", since = "1.9.0")]
    #[must_use]
    #[inline]
    pub fn capacity(&self) -> usize {
        self.inner.capacity()
    }

    /// 为该 `OsString` 预留至少能再插入 `additional` 个容量单位的空间。
    /// 如果容量已经足够，则什么也不做。
    ///
    /// 集合可能会预留比所需更多的空间，以推测性地避免频繁的重新分配。
    ///
    /// 关于编码和容量单位的信息，参见 `OsString` 的主文档。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::ffi::OsString;
    ///
    /// let mut s = OsString::new();
    /// s.reserve(10);
    /// assert!(s.capacity() >= 10);
    /// ```
    #[stable(feature = "osstring_simple_functions", since = "1.9.0")]
    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        self.inner.reserve(additional)
    }

    /// 尝试为该 `OsString` 预留至少能再插入 `additional` 个长度单位的空间。
    /// 该字符串可能会预留更多空间，以推测性地避免频繁的重新分配。调用
    /// `try_reserve` 之后，如果它返回 `Ok(())`，则容量将大于或等于
    /// `self.len() + additional`。如果容量已经足够，则什么也不做。即使发生
    /// 错误，本方法也会保留其内容。
    ///
    /// 关于编码和容量单位的信息，参见 `OsString` 的主文档。
    ///
    /// # Errors
    ///
    /// 如果容量溢出，或分配器报告失败，则返回一个错误。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::ffi::{OsStr, OsString};
    /// use std::collections::TryReserveError;
    ///
    /// fn process_data(data: &str) -> Result<OsString, TryReserveError> {
    ///     let mut s = OsString::new();
    ///
    ///     // 预先预留内存，如果无法预留则退出
    ///     s.try_reserve(OsStr::new(data).len())?;
    ///
    ///     // 现在我们知道在执行复杂工作的中途不会发生 OOM
    ///     s.push(data);
    ///
    ///     Ok(s)
    /// }
    /// # process_data("123").expect("why is the test harness OOMing on 3 bytes?");
    /// ```
    #[stable(feature = "try_reserve_2", since = "1.63.0")]
    #[inline]
    pub fn try_reserve(&mut self, additional: usize) -> Result<(), TryReserveError> {
        self.inner.try_reserve(additional)
    }

    /// 为该 `OsString` 预留最小的容量，使其至少能再插入 `additional` 个容量
    /// 单位。如果容量已经足够，则什么也不做。
    ///
    /// 注意，分配器给集合的空间可能比请求的多。因此，不能依赖容量恰好是
    /// 最小值。如果预期之后还会插入，更应优先使用 [`reserve`]。
    ///
    /// [`reserve`]: OsString::reserve
    ///
    /// 关于编码和容量单位的信息，参见 `OsString` 的主文档。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::ffi::OsString;
    ///
    /// let mut s = OsString::new();
    /// s.reserve_exact(10);
    /// assert!(s.capacity() >= 10);
    /// ```
    #[stable(feature = "osstring_simple_functions", since = "1.9.0")]
    #[inline]
    pub fn reserve_exact(&mut self, additional: usize) {
        self.inner.reserve_exact(additional)
    }

    /// 尝试为该 `OsString` 预留最小的容量，使其至少能再插入 `additional` 个
    /// 长度单位。调用 `try_reserve_exact` 之后，如果它返回 `Ok(())`，则容量将
    /// 大于或等于 `self.len() + additional`。如果容量已经足够，则什么也不做。
    ///
    /// 注意，分配器给 `OsString` 的空间可能比请求的多。因此，不能依赖容量
    /// 恰好是最小值。如果预期之后还会插入，更应优先使用 [`try_reserve`]。
    ///
    /// [`try_reserve`]: OsString::try_reserve
    ///
    /// 关于编码和容量单位的信息，参见 `OsString` 的主文档。
    ///
    /// # Errors
    ///
    /// 如果容量溢出，或分配器报告失败，则返回一个错误。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::ffi::{OsStr, OsString};
    /// use std::collections::TryReserveError;
    ///
    /// fn process_data(data: &str) -> Result<OsString, TryReserveError> {
    ///     let mut s = OsString::new();
    ///
    ///     // 预先预留内存，如果无法预留则退出
    ///     s.try_reserve_exact(OsStr::new(data).len())?;
    ///
    ///     // 现在我们知道在执行复杂工作的中途不会发生 OOM
    ///     s.push(data);
    ///
    ///     Ok(s)
    /// }
    /// # process_data("123").expect("why is the test harness OOMing on 3 bytes?");
    /// ```
    #[stable(feature = "try_reserve_2", since = "1.63.0")]
    #[inline]
    pub fn try_reserve_exact(&mut self, additional: usize) -> Result<(), TryReserveError> {
        self.inner.try_reserve_exact(additional)
    }

    /// 将该 `OsString` 的容量收缩到与其长度相匹配。
    ///
    /// 关于编码和容量单位的信息，参见 `OsString` 的主文档。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::ffi::OsString;
    ///
    /// let mut s = OsString::from("foo");
    ///
    /// s.reserve(100);
    /// assert!(s.capacity() >= 100);
    ///
    /// s.shrink_to_fit();
    /// assert_eq!(3, s.capacity());
    /// ```
    #[stable(feature = "osstring_shrink_to_fit", since = "1.19.0")]
    #[inline]
    pub fn shrink_to_fit(&mut self) {
        self.inner.shrink_to_fit()
    }

    /// 将该 `OsString` 的容量收缩到一个下界。
    ///
    /// 容量将至少保持与长度和所提供的值二者一样大。
    ///
    /// 如果当前容量小于该下限，则这是一个空操作（no-op）。
    ///
    /// 关于编码和容量单位的信息，参见 `OsString` 的主文档。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::ffi::OsString;
    ///
    /// let mut s = OsString::from("foo");
    ///
    /// s.reserve(100);
    /// assert!(s.capacity() >= 100);
    ///
    /// s.shrink_to(10);
    /// assert!(s.capacity() >= 10);
    /// s.shrink_to(0);
    /// assert!(s.capacity() >= 3);
    /// ```
    #[inline]
    #[stable(feature = "shrink_to", since = "1.56.0")]
    pub fn shrink_to(&mut self, min_capacity: usize) {
        self.inner.shrink_to(min_capacity)
    }

    /// 把该 `OsString` 转换成一个装箱（boxed）的 [`OsStr`]。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::ffi::{OsString, OsStr};
    ///
    /// let s = OsString::from("hello");
    ///
    /// let b: Box<OsStr> = s.into_boxed_os_str();
    /// ```
    #[must_use = "`self` will be dropped if the result is not used"]
    #[stable(feature = "into_boxed_os_str", since = "1.20.0")]
    pub fn into_boxed_os_str(self) -> Box<OsStr> {
        let rw = Box::into_raw(self.inner.into_box()) as *mut OsStr;
        unsafe { Box::from_raw(rw) }
    }

    /// 消耗并泄漏（leak）该 `OsString`，返回一个对其内容的可变引用
    /// `&'a mut OsStr`。
    ///
    /// 调用者可以自由选择返回引用的生命周期，包括 'static。事实上，本函数最
    /// 适合用于在程序剩余生命周期内一直存活的数据，因为丢弃返回的引用会导致
    /// 内存泄漏。
    ///
    /// 它不会重新分配或收缩该 `OsString`，因此被泄漏的分配可能包含不属于
    /// 返回切片的未使用容量。如果你想丢弃多余的容量，请改为调用
    /// [`into_boxed_os_str`]，然后再调用 [`Box::leak`]。不过要记住，裁剪容量
    /// 可能导致一次重新分配和拷贝。
    ///
    /// [`into_boxed_os_str`]: Self::into_boxed_os_str
    #[stable(feature = "os_string_pathbuf_leak", since = "1.89.0")]
    #[inline]
    pub fn leak<'a>(self) -> &'a mut OsStr {
        OsStr::from_inner_mut(self.inner.leak())
    }

    /// 把该 `OsString` 截断到指定的长度。
    ///
    /// # Panics
    /// 如果 `len` 没有落在有效的 `OsStr` 边界上（如
    /// [`OsStr::slice_encoded_bytes`] 所述），则 panic。
    #[inline]
    #[unstable(feature = "os_string_truncate", issue = "133262")]
    pub fn truncate(&mut self, len: usize) {
        self.as_os_str().inner.check_public_boundary(len);
        // SAFETY: 刚刚已检查过该长度落在有效边界上。
        unsafe { self.inner.truncate_unchecked(len) };
    }

    /// 提供通往 `Vec::extend_from_slice` 的内部通道，同时不暴露对该 `Vec` 的
    /// 完整可变访问。
    ///
    /// # 安全性(Safety）
    ///
    /// 该切片必须对平台编码有效（如
    /// [`OsStr::from_encoded_bytes_unchecked`] 所述）。
    ///
    /// 这会绕过依赖编码的代理项拼接（surrogate joining），因此要么 `self`
    /// 不能以一个前导代理项半部（leading surrogate half）结尾，要么 `other`
    /// 不能以一个后尾代理项半部（trailing surrogate half）开头。
    #[inline]
    pub(crate) unsafe fn extend_from_slice_unchecked(&mut self, other: &[u8]) {
        // SAFETY: 由调用者保证。
        unsafe { self.inner.extend_from_slice_unchecked(other) };
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl From<String> for OsString {
    /// 把一个 [`String`] 转换成一个 [`OsString`]。
    ///
    /// 这种转换不会分配或拷贝内存。
    #[inline]
    fn from(s: String) -> OsString {
        OsString { inner: Buf::from_string(s) }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T: ?Sized + AsRef<OsStr>> From<&T> for OsString {
    /// 把任何实现了 <code>[AsRef]&lt;[OsStr]&gt;</code> 的值拷贝进一个新分配的
    /// [`OsString`]。
    fn from(s: &T) -> OsString {
        trait SpecToOsString {
            fn spec_to_os_string(&self) -> OsString;
        }

        impl<T: AsRef<OsStr>> SpecToOsString for T {
            #[inline]
            default fn spec_to_os_string(&self) -> OsString {
                self.as_ref().to_os_string()
            }
        }

        // 为字符串保留其已知为 UTF-8 的属性。
        macro spec_str($T:ty) {
            impl SpecToOsString for $T {
                #[inline]
                fn spec_to_os_string(&self) -> OsString {
                    OsString::from(String::from(self))
                }
            }
        }
        spec_str!(str);
        spec_str!(String);

        s.spec_to_os_string()
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl ops::Index<ops::RangeFull> for OsString {
    type Output = OsStr;

    #[inline]
    fn index(&self, _index: ops::RangeFull) -> &OsStr {
        OsStr::from_inner(self.inner.as_slice())
    }
}

#[stable(feature = "mut_osstr", since = "1.44.0")]
impl ops::IndexMut<ops::RangeFull> for OsString {
    #[inline]
    fn index_mut(&mut self, _index: ops::RangeFull) -> &mut OsStr {
        OsStr::from_inner_mut(self.inner.as_mut_slice())
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl ops::Deref for OsString {
    type Target = OsStr;

    #[inline]
    fn deref(&self) -> &OsStr {
        &self[..]
    }
}

#[stable(feature = "mut_osstr", since = "1.44.0")]
impl ops::DerefMut for OsString {
    #[inline]
    fn deref_mut(&mut self) -> &mut OsStr {
        &mut self[..]
    }
}

#[stable(feature = "osstring_default", since = "1.9.0")]
impl Default for OsString {
    /// 构造一个空的 `OsString`。
    #[inline]
    fn default() -> OsString {
        OsString::new()
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl Clone for OsString {
    #[inline]
    fn clone(&self) -> Self {
        OsString { inner: self.inner.clone() }
    }

    /// 把 `source` 的内容克隆进 `self`。
    ///
    /// 相比简单地把 `source.clone()` 赋值给 `self`，更应优先使用本方法，
    /// 因为它在可能的情况下避免重新分配。
    #[inline]
    fn clone_from(&mut self, source: &Self) {
        self.inner.clone_from(&source.inner)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl fmt::Debug for OsString {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, formatter)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl PartialEq for OsString {
    #[inline]
    fn eq(&self, other: &OsString) -> bool {
        &**self == &**other
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl PartialEq<str> for OsString {
    #[inline]
    fn eq(&self, other: &str) -> bool {
        &**self == other
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl PartialEq<OsString> for str {
    #[inline]
    fn eq(&self, other: &OsString) -> bool {
        &**other == self
    }
}

#[stable(feature = "os_str_str_ref_eq", since = "1.29.0")]
impl PartialEq<&str> for OsString {
    #[inline]
    fn eq(&self, other: &&str) -> bool {
        **self == **other
    }
}

#[stable(feature = "os_str_str_ref_eq", since = "1.29.0")]
impl<'a> PartialEq<OsString> for &'a str {
    #[inline]
    fn eq(&self, other: &OsString) -> bool {
        **other == **self
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl Eq for OsString {}

#[stable(feature = "rust1", since = "1.0.0")]
impl PartialOrd for OsString {
    #[inline]
    fn partial_cmp(&self, other: &OsString) -> Option<cmp::Ordering> {
        (&**self).partial_cmp(&**other)
    }
    #[inline]
    fn lt(&self, other: &OsString) -> bool {
        &**self < &**other
    }
    #[inline]
    fn le(&self, other: &OsString) -> bool {
        &**self <= &**other
    }
    #[inline]
    fn gt(&self, other: &OsString) -> bool {
        &**self > &**other
    }
    #[inline]
    fn ge(&self, other: &OsString) -> bool {
        &**self >= &**other
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl PartialOrd<str> for OsString {
    #[inline]
    fn partial_cmp(&self, other: &str) -> Option<cmp::Ordering> {
        (&**self).partial_cmp(other)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl Ord for OsString {
    #[inline]
    fn cmp(&self, other: &OsString) -> cmp::Ordering {
        (&**self).cmp(&**other)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl Hash for OsString {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        (&**self).hash(state)
    }
}

#[stable(feature = "os_string_fmt_write", since = "1.64.0")]
impl fmt::Write for OsString {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.push(s);
        Ok(())
    }
}

impl OsStr {
    /// 强制转换为一个 `OsStr` 切片。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::ffi::OsStr;
    ///
    /// let os_str = OsStr::new("foo");
    /// ```
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_unstable(feature = "const_convert", issue = "143773")]
    pub const fn new<S: [const] AsRef<OsStr> + ?Sized>(s: &S) -> &OsStr {
        s.as_ref()
    }

    /// 把一个字节切片转换为一个 OS 字符串切片，不检查该字符串是否含有有效的
    /// `OsStr` 编码数据。
    ///
    /// 该字节编码是 UTF-8 的一个未指定的、平台相关的、自同步超集。由于它是
    /// UTF-8 的一个自同步超集，因此该编码也是 7 位 ASCII 的超集。
    ///
    /// 关于从/到原生表示进行安全、跨平台的 [conversions]，参见[模块顶层关于
    /// 转换的文档][conversions]。
    ///
    /// # 安全性(Safety）
    ///
    /// 由于编码是未指定的，调用者必须传入这样的字节：它们源自经过校验的
    /// UTF-8 与来自 [`OsStr::as_encoded_bytes`] 的字节的混合，且这些字节是
    /// 在相同 Rust 版本、为相同目标平台构建的环境下产生的。例如，从经网络
    /// 发送或存储于文件中的字节重建一个 `OsStr`，很可能会违反这些安全规则。
    ///
    /// 由于该编码是自同步的，来自 [`OsStr::as_encoded_bytes`] 的字节可以在
    /// 任意有效的非空 UTF-8 子串的紧前或紧后被切分。
    ///
    /// # Example
    ///
    /// ```
    /// use std::ffi::OsStr;
    ///
    /// let os_str = OsStr::new("Mary had a little lamb");
    /// let bytes = os_str.as_encoded_bytes();
    /// let words = bytes.split(|b| *b == b' ');
    /// let words: Vec<&OsStr> = words.map(|word| {
    ///     // SAFETY：
    ///     // - 每个 `word` 只包含源自 `OsStr::as_encoded_bytes` 的内容
    ///     // - 仅以 ASCII 空白进行切分，而它是一个非空的 UTF-8 子串
    ///     unsafe { OsStr::from_encoded_bytes_unchecked(word) }
    /// }).collect();
    /// ```
    ///
    /// [conversions]: super#conversions
    #[inline]
    #[stable(feature = "os_str_bytes", since = "1.74.0")]
    pub unsafe fn from_encoded_bytes_unchecked(bytes: &[u8]) -> &Self {
        Self::from_inner(unsafe { Slice::from_encoded_bytes_unchecked(bytes) })
    }

    #[inline]
    #[rustc_const_unstable(feature = "const_convert", issue = "143773")]
    const fn from_inner(inner: &Slice) -> &OsStr {
        // SAFETY: OsStr 只是 Slice 的一个包装，
        // 因此把 &Slice 转换为 &OsStr 是安全的。
        unsafe { &*(inner as *const Slice as *const OsStr) }
    }

    #[inline]
    #[rustc_const_unstable(feature = "const_convert", issue = "143773")]
    const fn from_inner_mut(inner: &mut Slice) -> &mut OsStr {
        // SAFETY: OsStr 只是 Slice 的一个包装，
        // 因此把 &mut Slice 转换为 &mut OsStr 是安全的。
        // 任何会修改 OsStr 的方法都必须小心，不要破坏平台相关的编码，
        // 特别是 Windows 上的 Wtf8。
        unsafe { &mut *(inner as *mut Slice as *mut OsStr) }
    }

    /// 如果该 `OsStr` 是有效的 Unicode，则产出一个 <code>&[str]</code> 切片。
    ///
    /// 这种转换可能需要做一次 UTF-8 有效性检查。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::ffi::OsStr;
    ///
    /// let os_str = OsStr::new("foo");
    /// assert_eq!(os_str.to_str(), Some("foo"));
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[must_use = "this returns the result of the operation, \
                  without modifying the original"]
    #[inline]
    pub fn to_str(&self) -> Option<&str> {
        self.inner.to_str().ok()
    }

    /// 把一个 `OsStr` 转换为 <code>[Cow]<[str]></code>。
    ///
    /// 任何非 UTF-8 序列都会被替换为
    /// [`U+FFFD REPLACEMENT CHARACTER`][U+FFFD]。
    ///
    /// [U+FFFD]: crate::char::REPLACEMENT_CHARACTER
    ///
    /// # 示例
    ///
    /// 在一个含有无效 unicode 的 `OsStr` 上调用 `to_string_lossy`：
    ///
    /// ```
    /// // 注意，由于 Unix 和 Windows 表示字符串的方式不同，
    /// // 我们不得不把这个示例复杂化，分别用不同的源数据、通过不同的平台
    /// // 扩展来构造示例 `OsStr`。
    /// // 要理解，在现实中你完全可能仅仅通过收集用户的命令行参数，就得到
    /// // 这样的无效序列示例。
    ///
    /// #[cfg(unix)] {
    ///     use std::ffi::OsStr;
    ///     use std::os::unix::ffi::OsStrExt;
    ///
    ///     // 这里，值 0x66 和 0x6f 分别对应 'f' 和 'o'。
    ///     // 值 0x80 是一个孤立的延续字节（continuation byte），在 UTF-8
    ///     // 序列中是无效的。
    ///     let source = [0x66, 0x6f, 0x80, 0x6f];
    ///     let os_str = OsStr::from_bytes(&source[..]);
    ///
    ///     assert_eq!(os_str.to_string_lossy(), "fo�o");
    /// }
    /// #[cfg(windows)] {
    ///     use std::ffi::OsString;
    ///     use std::os::windows::prelude::*;
    ///
    ///     // 这里，值 0x0066 和 0x006f 分别对应 'f' 和 'o'。
    ///     // 值 0xD800 是一个孤立的代理项半部（surrogate half），在 UTF-16
    ///     // 序列中是无效的。
    ///     let source = [0x0066, 0x006f, 0xD800, 0x006f];
    ///     let os_string = OsString::from_wide(&source[..]);
    ///     let os_str = os_string.as_os_str();
    ///
    ///     assert_eq!(os_str.to_string_lossy(), "fo�o");
    /// }
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[must_use = "this returns the result of the operation, \
                  without modifying the original"]
    #[inline]
    pub fn to_string_lossy(&self) -> Cow<'_, str> {
        self.inner.to_string_lossy()
    }

    /// 把该切片拷贝进一个拥有所有权的 [`OsString`]。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::ffi::{OsStr, OsString};
    ///
    /// let os_str = OsStr::new("foo");
    /// let os_string = os_str.to_os_string();
    /// assert_eq!(os_string, OsString::from("foo"));
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[must_use = "this returns the result of the operation, \
                  without modifying the original"]
    #[inline]
    #[cfg_attr(not(test), rustc_diagnostic_item = "os_str_to_os_string")]
    pub fn to_os_string(&self) -> OsString {
        OsString { inner: self.inner.to_owned() }
    }

    /// 检查该 `OsStr` 是否为空。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::ffi::OsStr;
    ///
    /// let os_str = OsStr::new("");
    /// assert!(os_str.is_empty());
    ///
    /// let os_str = OsStr::new("foo");
    /// assert!(!os_str.is_empty());
    /// ```
    #[stable(feature = "osstring_simple_functions", since = "1.9.0")]
    #[must_use]
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.inner.inner.is_empty()
    }

    /// 返回该 `OsStr` 的长度。
    ///
    /// 注意，这**不**返回该字符串在 OS 字符串形式下的字节数。
    ///
    /// 所返回的长度是 `OsStr` 所用底层存储的长度。如 [`OsString`] 引言中所
    /// 讨论的，[`OsString`] 和 `OsStr` 以一种最适合在平台原生形式与 Rust
    /// 字符串形式之间廉价相互转换的形式来存储字符串，这种形式在存储大小和
    /// 编码上都可能与两者有显著差异。
    ///
    /// 这个数值仅在传给其他方法（例如 [`OsString::with_capacity`] 以避免
    /// 重新分配）时有用。
    ///
    /// 关于编码和容量单位的信息，参见 `OsString` 的主文档。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::ffi::OsStr;
    ///
    /// let os_str = OsStr::new("");
    /// assert_eq!(os_str.len(), 0);
    ///
    /// let os_str = OsStr::new("foo");
    /// assert_eq!(os_str.len(), 3);
    /// ```
    #[stable(feature = "osstring_simple_functions", since = "1.9.0")]
    #[must_use]
    #[inline]
    pub fn len(&self) -> usize {
        self.inner.inner.len()
    }

    /// 把一个 <code>[Box]<[OsStr]></code> 转换成一个 [`OsString`]，不进行拷贝
    /// 或分配。
    #[stable(feature = "into_boxed_os_str", since = "1.20.0")]
    #[must_use = "`self` will be dropped if the result is not used"]
    pub fn into_os_string(self: Box<Self>) -> OsString {
        let boxed = unsafe { Box::from_raw(Box::into_raw(self) as *mut Slice) };
        OsString { inner: Buf::from_box(boxed) }
    }

    /// 把一个 OS 字符串切片转换为一个字节切片。要把该字节切片转换回 OS
    /// 字符串切片，请使用 [`OsStr::from_encoded_bytes_unchecked`] 函数。
    ///
    /// 该字节编码是 UTF-8 的一个未指定的、平台相关的、自同步超集。由于它是
    /// UTF-8 的一个自同步超集，因此该编码也是 7 位 ASCII 的超集。
    ///
    /// 注意：由于编码是未指定的，任何并非有效 UTF-8 的字节子切片都应被视为
    /// 不透明的，且只能在相同 Rust 版本、为相同目标平台构建的环境内进行
    /// 比较。例如，把该切片经网络发送或存储于文件中，很可能会得到不兼容的
    /// 字节切片。关于编码的更多细节参见 [`OsString`]，关于平台相关的、已明确
    /// 指定的转换参见 [`std::ffi`]。
    ///
    /// [`std::ffi`]: crate::ffi
    #[inline]
    #[stable(feature = "os_str_bytes", since = "1.74.0")]
    pub fn as_encoded_bytes(&self) -> &[u8] {
        self.inner.as_encoded_bytes()
    }

    /// 基于一个与 [`OsStr::as_encoded_bytes`] 返回值相对应的范围取出一个子串。
    ///
    /// 该范围的起点和终点必须落在有效的 `OsStr` 边界上。一个有效的 `OsStr`
    /// 边界是以下之一：
    /// - 字符串的起点
    /// - 字符串的终点
    /// - 紧位于一个有效的非空 UTF-8 子串之前
    /// - 紧位于一个有效的非空 UTF-8 子串之后
    ///
    /// # Panics
    ///
    /// 如果 `range` 没有落在有效的 `OsStr` 边界上，或者它超出了字符串的
    /// 末尾，则 panic。
    ///
    /// # Example
    ///
    /// ```
    /// #![feature(os_str_slice)]
    ///
    /// use std::ffi::OsStr;
    ///
    /// let os_str = OsStr::new("foo=bar");
    /// let bytes = os_str.as_encoded_bytes();
    /// if let Some(index) = bytes.iter().position(|b| *b == b'=') {
    ///     let key = os_str.slice_encoded_bytes(..index);
    ///     let value = os_str.slice_encoded_bytes(index + 1..);
    ///     assert_eq!(key, "foo");
    ///     assert_eq!(value, "bar");
    /// }
    /// ```
    #[unstable(feature = "os_str_slice", issue = "118485")]
    pub fn slice_encoded_bytes<R: ops::RangeBounds<usize>>(&self, range: R) -> &Self {
        let encoded_bytes = self.as_encoded_bytes();
        let Range { start, end } = slice::range(range, ..encoded_bytes.len());

        // 如果索引没有落在如上所述的 `OsStr` 边界上，`check_public_boundary`
        // 应当 panic。可以用一种与编码无关的方式来实现这一点，但内部编码的
        // 细节可能允许更高效的实现。
        self.inner.check_public_boundary(start);
        self.inner.check_public_boundary(end);

        // SAFETY: `slice::range` 保证 `start` 和 `end` 是有效的
        let slice = unsafe { encoded_bytes.get_unchecked(start..end) };

        // SAFETY: `slice` 来自 `self`，并且我们已校验过边界
        unsafe { Self::from_encoded_bytes_unchecked(slice) }
    }

    /// 就地（in-place）把该字符串转换为其 ASCII 小写等价形式。
    ///
    /// ASCII 字母 'A' 到 'Z' 被映射为 'a' 到 'z'，但非 ASCII 字母保持不变。
    ///
    /// 若要返回一个新的小写化值而不修改现有值，请使用
    /// [`OsStr::to_ascii_lowercase`]。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::ffi::OsString;
    ///
    /// let mut s = OsString::from("GRÜßE, JÜRGEN ❤");
    ///
    /// s.make_ascii_lowercase();
    ///
    /// assert_eq!("grÜße, jÜrgen ❤", s);
    /// ```
    #[stable(feature = "osstring_ascii", since = "1.53.0")]
    #[inline]
    pub fn make_ascii_lowercase(&mut self) {
        self.inner.make_ascii_lowercase()
    }

    /// 就地把该字符串转换为其 ASCII 大写等价形式。
    ///
    /// ASCII 字母 'a' 到 'z' 被映射为 'A' 到 'Z'，但非 ASCII 字母保持不变。
    ///
    /// 若要返回一个新的大写化值而不修改现有值，请使用
    /// [`OsStr::to_ascii_uppercase`]。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::ffi::OsString;
    ///
    /// let mut s = OsString::from("Grüße, Jürgen ❤");
    ///
    /// s.make_ascii_uppercase();
    ///
    /// assert_eq!("GRüßE, JüRGEN ❤", s);
    /// ```
    #[stable(feature = "osstring_ascii", since = "1.53.0")]
    #[inline]
    pub fn make_ascii_uppercase(&mut self) {
        self.inner.make_ascii_uppercase()
    }

    /// 返回该字符串的一个副本，其中每个字符都被映射为其 ASCII 小写等价形式。
    ///
    /// ASCII 字母 'A' 到 'Z' 被映射为 'a' 到 'z'，但非 ASCII 字母保持不变。
    ///
    /// 若要就地把该值小写化，请使用 [`OsStr::make_ascii_lowercase`]。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::ffi::OsString;
    /// let s = OsString::from("Grüße, Jürgen ❤");
    ///
    /// assert_eq!("grüße, jürgen ❤", s.to_ascii_lowercase());
    /// ```
    #[must_use = "to lowercase the value in-place, use `make_ascii_lowercase`"]
    #[stable(feature = "osstring_ascii", since = "1.53.0")]
    pub fn to_ascii_lowercase(&self) -> OsString {
        OsString::from_inner(self.inner.to_ascii_lowercase())
    }

    /// 返回该字符串的一个副本，其中每个字符都被映射为其 ASCII 大写等价形式。
    ///
    /// ASCII 字母 'a' 到 'z' 被映射为 'A' 到 'Z'，但非 ASCII 字母保持不变。
    ///
    /// 若要就地把该值大写化，请使用 [`OsStr::make_ascii_uppercase`]。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::ffi::OsString;
    /// let s = OsString::from("Grüße, Jürgen ❤");
    ///
    /// assert_eq!("GRüßE, JüRGEN ❤", s.to_ascii_uppercase());
    /// ```
    #[must_use = "to uppercase the value in-place, use `make_ascii_uppercase`"]
    #[stable(feature = "osstring_ascii", since = "1.53.0")]
    pub fn to_ascii_uppercase(&self) -> OsString {
        OsString::from_inner(self.inner.to_ascii_uppercase())
    }

    /// 检查该字符串中的所有字符是否都在 ASCII 范围内。
    ///
    /// 空字符串返回 `true`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::ffi::OsString;
    ///
    /// let ascii = OsString::from("hello!\n");
    /// let non_ascii = OsString::from("Grüße, Jürgen ❤");
    ///
    /// assert!(ascii.is_ascii());
    /// assert!(!non_ascii.is_ascii());
    /// ```
    #[stable(feature = "osstring_ascii", since = "1.53.0")]
    #[must_use]
    #[inline]
    pub fn is_ascii(&self) -> bool {
        self.inner.is_ascii()
    }

    /// 检查两个字符串是否为 ASCII 大小写不敏感的匹配。
    ///
    /// 等同于 `to_ascii_lowercase(a) == to_ascii_lowercase(b)`，
    /// 但不会分配和拷贝临时值。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::ffi::OsString;
    ///
    /// assert!(OsString::from("Ferris").eq_ignore_ascii_case("FERRIS"));
    /// assert!(OsString::from("Ferrös").eq_ignore_ascii_case("FERRöS"));
    /// assert!(!OsString::from("Ferrös").eq_ignore_ascii_case("FERRÖS"));
    /// ```
    #[stable(feature = "osstring_ascii", since = "1.53.0")]
    pub fn eq_ignore_ascii_case<S: AsRef<OsStr>>(&self, other: S) -> bool {
        self.inner.eq_ignore_ascii_case(&other.as_ref().inner)
    }

    /// 返回一个实现了 [`Display`] 的对象，用于安全地打印一个可能含有非 Unicode
    /// 数据的 [`OsStr`]。这可能会执行有损转换，取决于平台。如果你想要一个
    /// 对 [`OsStr`] 进行转义的实现，请改用 [`Debug`]。
    ///
    /// [`Display`]: fmt::Display
    /// [`Debug`]: fmt::Debug
    ///
    /// # 示例
    ///
    /// ```
    /// use std::ffi::OsStr;
    ///
    /// let s = OsStr::new("Hello, world!");
    /// println!("{}", s.display());
    /// ```
    #[stable(feature = "os_str_display", since = "1.87.0")]
    #[must_use = "this does not display the `OsStr`; \
                  it returns an object that can be displayed"]
    #[inline]
    pub fn display(&self) -> Display<'_> {
        Display { os_str: self }
    }

    /// 把该字符串作为一个字符串切片 `&OsStr` 原样返回。
    ///
    /// 当直接在 `&OsStr` 上使用时，本方法是多余的，但它有助于把其他类似
    /// 字符串的类型解引用为字符串切片，例如对 `Box<OsStr>` 或 `Arc<OsStr>`
    /// 的引用。
    #[inline]
    #[unstable(feature = "str_as_str", issue = "130366")]
    pub const fn as_os_str(&self) -> &OsStr {
        self
    }
}

#[stable(feature = "box_from_os_str", since = "1.17.0")]
impl From<&OsStr> for Box<OsStr> {
    /// 把该字符串拷贝进一个新分配的 <code>[Box]&lt;[OsStr]&gt;</code>。
    #[inline]
    fn from(s: &OsStr) -> Box<OsStr> {
        Box::clone_from_ref(s)
    }
}

#[stable(feature = "box_from_mut_slice", since = "1.84.0")]
impl From<&mut OsStr> for Box<OsStr> {
    /// 把该字符串拷贝进一个新分配的 <code>[Box]&lt;[OsStr]&gt;</code>。
    #[inline]
    fn from(s: &mut OsStr) -> Box<OsStr> {
        Self::from(&*s)
    }
}

#[stable(feature = "box_from_cow", since = "1.45.0")]
impl From<Cow<'_, OsStr>> for Box<OsStr> {
    /// 把一个 `Cow<'a, OsStr>` 转换成一个 <code>[Box]&lt;[OsStr]&gt;</code>，
    /// 如果内容是借用的则进行拷贝。
    #[inline]
    fn from(cow: Cow<'_, OsStr>) -> Box<OsStr> {
        match cow {
            Cow::Borrowed(s) => Box::from(s),
            Cow::Owned(s) => Box::from(s),
        }
    }
}

#[stable(feature = "os_string_from_box", since = "1.18.0")]
impl From<Box<OsStr>> for OsString {
    /// 把一个 <code>[Box]<[OsStr]></code> 转换成一个 [`OsString`]，不进行拷贝
    /// 或分配。
    #[inline]
    fn from(boxed: Box<OsStr>) -> OsString {
        boxed.into_os_string()
    }
}

#[stable(feature = "box_from_os_string", since = "1.20.0")]
impl From<OsString> for Box<OsStr> {
    /// 把一个 [`OsString`] 转换成一个 <code>[Box]<[OsStr]></code>，不进行拷贝
    /// 或分配。
    #[inline]
    fn from(s: OsString) -> Box<OsStr> {
        s.into_boxed_os_str()
    }
}

#[stable(feature = "more_box_slice_clone", since = "1.29.0")]
impl Clone for Box<OsStr> {
    #[inline]
    fn clone(&self) -> Self {
        self.to_os_string().into_boxed_os_str()
    }
}

#[unstable(feature = "clone_to_uninit", issue = "126799")]
unsafe impl CloneToUninit for OsStr {
    #[inline]
    #[cfg_attr(debug_assertions, track_caller)]
    unsafe fn clone_to_uninit(&self, dst: *mut u8) {
        // SAFETY: 我们只是平台相关的 Slice 之上的一个 transparent 包装
        unsafe { self.inner.clone_to_uninit(dst) }
    }
}

#[stable(feature = "shared_from_slice2", since = "1.24.0")]
impl From<OsString> for Arc<OsStr> {
    /// 通过把 [`OsString`] 的数据移入一个新的 [`Arc`] 缓冲区，把一个
    /// [`OsString`] 转换成一个 <code>[Arc]<[OsStr]></code>。
    #[inline]
    fn from(s: OsString) -> Arc<OsStr> {
        let arc = s.inner.into_arc();
        unsafe { Arc::from_raw(Arc::into_raw(arc) as *const OsStr) }
    }
}

#[stable(feature = "shared_from_slice2", since = "1.24.0")]
impl From<&OsStr> for Arc<OsStr> {
    /// 把该字符串拷贝进一个新分配的 <code>[Arc]&lt;[OsStr]&gt;</code>。
    #[inline]
    fn from(s: &OsStr) -> Arc<OsStr> {
        let arc = s.inner.into_arc();
        unsafe { Arc::from_raw(Arc::into_raw(arc) as *const OsStr) }
    }
}

#[stable(feature = "shared_from_mut_slice", since = "1.84.0")]
impl From<&mut OsStr> for Arc<OsStr> {
    /// 把该字符串拷贝进一个新分配的 <code>[Arc]&lt;[OsStr]&gt;</code>。
    #[inline]
    fn from(s: &mut OsStr) -> Arc<OsStr> {
        Arc::from(&*s)
    }
}

#[stable(feature = "shared_from_slice2", since = "1.24.0")]
impl From<OsString> for Rc<OsStr> {
    /// 通过把 [`OsString`] 的数据移入一个新的 [`Rc`] 缓冲区，把一个
    /// [`OsString`] 转换成一个 <code>[Rc]<[OsStr]></code>。
    #[inline]
    fn from(s: OsString) -> Rc<OsStr> {
        let rc = s.inner.into_rc();
        unsafe { Rc::from_raw(Rc::into_raw(rc) as *const OsStr) }
    }
}

#[stable(feature = "shared_from_slice2", since = "1.24.0")]
impl From<&OsStr> for Rc<OsStr> {
    /// 把该字符串拷贝进一个新分配的 <code>[Rc]&lt;[OsStr]&gt;</code>。
    #[inline]
    fn from(s: &OsStr) -> Rc<OsStr> {
        let rc = s.inner.into_rc();
        unsafe { Rc::from_raw(Rc::into_raw(rc) as *const OsStr) }
    }
}

#[stable(feature = "shared_from_mut_slice", since = "1.84.0")]
impl From<&mut OsStr> for Rc<OsStr> {
    /// 把该字符串拷贝进一个新分配的 <code>[Rc]&lt;[OsStr]&gt;</code>。
    #[inline]
    fn from(s: &mut OsStr) -> Rc<OsStr> {
        Rc::from(&*s)
    }
}

#[stable(feature = "cow_from_osstr", since = "1.28.0")]
impl<'a> From<OsString> for Cow<'a, OsStr> {
    /// 把该字符串移入一个 [`Cow::Owned`]。
    #[inline]
    fn from(s: OsString) -> Cow<'a, OsStr> {
        Cow::Owned(s)
    }
}

#[stable(feature = "cow_from_osstr", since = "1.28.0")]
impl<'a> From<&'a OsStr> for Cow<'a, OsStr> {
    /// 把该字符串引用转换成一个 [`Cow::Borrowed`]。
    #[inline]
    fn from(s: &'a OsStr) -> Cow<'a, OsStr> {
        Cow::Borrowed(s)
    }
}

#[stable(feature = "cow_from_osstr", since = "1.28.0")]
impl<'a> From<&'a OsString> for Cow<'a, OsStr> {
    /// 把该字符串引用转换成一个 [`Cow::Borrowed`]。
    #[inline]
    fn from(s: &'a OsString) -> Cow<'a, OsStr> {
        Cow::Borrowed(s.as_os_str())
    }
}

#[stable(feature = "osstring_from_cow_osstr", since = "1.28.0")]
impl<'a> From<Cow<'a, OsStr>> for OsString {
    /// 把一个 `Cow<'a, OsStr>` 转换成一个 [`OsString`]，如果内容是借用的则
    /// 进行拷贝。
    #[inline]
    fn from(s: Cow<'a, OsStr>) -> Self {
        s.into_owned()
    }
}

#[stable(feature = "str_tryfrom_osstr_impl", since = "1.72.0")]
impl<'a> TryFrom<&'a OsStr> for &'a str {
    type Error = crate::str::Utf8Error;

    /// 尝试把一个 `&OsStr` 转换成一个 `&str`。
    ///
    /// ```
    /// use std::ffi::OsStr;
    ///
    /// let os_str = OsStr::new("foo");
    /// let as_str = <&str>::try_from(os_str).unwrap();
    /// assert_eq!(as_str, "foo");
    /// ```
    fn try_from(value: &'a OsStr) -> Result<Self, Self::Error> {
        value.inner.to_str()
    }
}

#[stable(feature = "box_default_extra", since = "1.17.0")]
impl Default for Box<OsStr> {
    #[inline]
    fn default() -> Box<OsStr> {
        let rw = Box::into_raw(Slice::empty_box()) as *mut OsStr;
        unsafe { Box::from_raw(rw) }
    }
}

#[stable(feature = "osstring_default", since = "1.9.0")]
impl Default for &OsStr {
    /// 创建一个空的 `OsStr`。
    #[inline]
    fn default() -> Self {
        OsStr::new("")
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl PartialEq for OsStr {
    #[inline]
    fn eq(&self, other: &OsStr) -> bool {
        self.as_encoded_bytes().eq(other.as_encoded_bytes())
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl PartialEq<str> for OsStr {
    #[inline]
    fn eq(&self, other: &str) -> bool {
        *self == *OsStr::new(other)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl PartialEq<OsStr> for str {
    #[inline]
    fn eq(&self, other: &OsStr) -> bool {
        *other == *OsStr::new(self)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl Eq for OsStr {}

#[stable(feature = "rust1", since = "1.0.0")]
impl PartialOrd for OsStr {
    #[inline]
    fn partial_cmp(&self, other: &OsStr) -> Option<cmp::Ordering> {
        self.as_encoded_bytes().partial_cmp(other.as_encoded_bytes())
    }
    #[inline]
    fn lt(&self, other: &OsStr) -> bool {
        self.as_encoded_bytes().lt(other.as_encoded_bytes())
    }
    #[inline]
    fn le(&self, other: &OsStr) -> bool {
        self.as_encoded_bytes().le(other.as_encoded_bytes())
    }
    #[inline]
    fn gt(&self, other: &OsStr) -> bool {
        self.as_encoded_bytes().gt(other.as_encoded_bytes())
    }
    #[inline]
    fn ge(&self, other: &OsStr) -> bool {
        self.as_encoded_bytes().ge(other.as_encoded_bytes())
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl PartialOrd<str> for OsStr {
    #[inline]
    fn partial_cmp(&self, other: &str) -> Option<cmp::Ordering> {
        self.partial_cmp(OsStr::new(other))
    }
}

// FIXME (#19470): 在我们拥有更灵活的一致性（coherence）规则之前，
// 无法为 str 提供 PartialOrd<OsStr>。

#[stable(feature = "rust1", since = "1.0.0")]
impl Ord for OsStr {
    #[inline]
    fn cmp(&self, other: &OsStr) -> cmp::Ordering {
        self.as_encoded_bytes().cmp(other.as_encoded_bytes())
    }
}

macro_rules! impl_cmp {
    ($lhs:ty, $rhs: ty) => {
        #[stable(feature = "cmp_os_str", since = "1.8.0")]
        impl<'a, 'b> PartialEq<$rhs> for $lhs {
            #[inline]
            fn eq(&self, other: &$rhs) -> bool {
                <OsStr as PartialEq>::eq(self, other)
            }
        }

        #[stable(feature = "cmp_os_str", since = "1.8.0")]
        impl<'a, 'b> PartialEq<$lhs> for $rhs {
            #[inline]
            fn eq(&self, other: &$lhs) -> bool {
                <OsStr as PartialEq>::eq(self, other)
            }
        }

        #[stable(feature = "cmp_os_str", since = "1.8.0")]
        impl<'a, 'b> PartialOrd<$rhs> for $lhs {
            #[inline]
            fn partial_cmp(&self, other: &$rhs) -> Option<cmp::Ordering> {
                <OsStr as PartialOrd>::partial_cmp(self, other)
            }
        }

        #[stable(feature = "cmp_os_str", since = "1.8.0")]
        impl<'a, 'b> PartialOrd<$lhs> for $rhs {
            #[inline]
            fn partial_cmp(&self, other: &$lhs) -> Option<cmp::Ordering> {
                <OsStr as PartialOrd>::partial_cmp(self, other)
            }
        }
    };
}

impl_cmp!(OsString, OsStr);
impl_cmp!(OsString, &'a OsStr);
impl_cmp!(Cow<'a, OsStr>, OsStr);
impl_cmp!(Cow<'a, OsStr>, &'b OsStr);
impl_cmp!(Cow<'a, OsStr>, OsString);

#[stable(feature = "rust1", since = "1.0.0")]
impl Hash for OsStr {
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_encoded_bytes().hash(state)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl fmt::Debug for OsStr {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.inner, formatter)
    }
}

/// 用于配合 [`format!`] 和 `{}` 安全打印一个 [`OsStr`] 的辅助结构体。
///
/// 一个 [`OsStr`] 可能含有非 Unicode 数据。这个 `struct` 以一种缓解该问题的
/// 方式实现了 [`Display`] trait。它由 [`OsStr`] 上的 [`display`](OsStr::display)
/// 方法创建。这可能会执行有损转换，取决于平台。如果你想要一个对 [`OsStr`]
/// 进行转义的实现，请改用 [`Debug`]。
///
/// # 示例
///
/// ```
/// use std::ffi::OsStr;
///
/// let s = OsStr::new("Hello, world!");
/// println!("{}", s.display());
/// ```
///
/// [`Display`]: fmt::Display
/// [`format!`]: crate::format
#[stable(feature = "os_str_display", since = "1.87.0")]
pub struct Display<'a> {
    os_str: &'a OsStr,
}

#[stable(feature = "os_str_display", since = "1.87.0")]
impl fmt::Debug for Display<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.os_str, f)
    }
}

#[stable(feature = "os_str_display", since = "1.87.0")]
impl fmt::Display for Display<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.os_str.inner, f)
    }
}

#[unstable(feature = "slice_concat_ext", issue = "27747")]
impl<S: Borrow<OsStr>> alloc::slice::Join<&OsStr> for [S] {
    type Output = OsString;

    fn join(slice: &Self, sep: &OsStr) -> OsString {
        let Some((first, suffix)) = slice.split_first() else {
            return OsString::new();
        };
        let first_owned = first.borrow().to_owned();
        suffix.iter().fold(first_owned, |mut a, b| {
            a.push(sep);
            a.push(b.borrow());
            a
        })
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl Borrow<OsStr> for OsString {
    #[inline]
    fn borrow(&self) -> &OsStr {
        &self[..]
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl ToOwned for OsStr {
    type Owned = OsString;
    #[inline]
    fn to_owned(&self) -> OsString {
        self.to_os_string()
    }
    #[inline]
    fn clone_into(&self, target: &mut OsString) {
        self.inner.clone_into(&mut target.inner)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl const AsRef<OsStr> for OsStr {
    #[inline]
    fn as_ref(&self) -> &OsStr {
        self
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl AsRef<OsStr> for OsString {
    #[inline]
    fn as_ref(&self) -> &OsStr {
        self
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl AsRef<OsStr> for str {
    #[inline]
    fn as_ref(&self) -> &OsStr {
        OsStr::from_inner(Slice::from_str(self))
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl AsRef<OsStr> for String {
    #[inline]
    fn as_ref(&self) -> &OsStr {
        (&**self).as_ref()
    }
}

impl FromInner<Buf> for OsString {
    #[inline]
    fn from_inner(buf: Buf) -> OsString {
        OsString { inner: buf }
    }
}

impl IntoInner<Buf> for OsString {
    #[inline]
    fn into_inner(self) -> Buf {
        self.inner
    }
}

impl AsInner<Slice> for OsStr {
    #[inline]
    fn as_inner(&self) -> &Slice {
        &self.inner
    }
}

#[stable(feature = "osstring_from_str", since = "1.45.0")]
impl FromStr for OsString {
    type Err = core::convert::Infallible;

    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(OsString::from(s))
    }
}

#[stable(feature = "osstring_extend", since = "1.52.0")]
impl Extend<OsString> for OsString {
    #[inline]
    fn extend<T: IntoIterator<Item = OsString>>(&mut self, iter: T) {
        for s in iter {
            self.push(&s);
        }
    }
}

#[stable(feature = "osstring_extend", since = "1.52.0")]
impl<'a> Extend<&'a OsStr> for OsString {
    #[inline]
    fn extend<T: IntoIterator<Item = &'a OsStr>>(&mut self, iter: T) {
        for s in iter {
            self.push(s);
        }
    }
}

#[stable(feature = "osstring_extend", since = "1.52.0")]
impl<'a> Extend<Cow<'a, OsStr>> for OsString {
    #[inline]
    fn extend<T: IntoIterator<Item = Cow<'a, OsStr>>>(&mut self, iter: T) {
        for s in iter {
            self.push(&s);
        }
    }
}

#[stable(feature = "osstring_extend", since = "1.52.0")]
impl FromIterator<OsString> for OsString {
    #[inline]
    fn from_iter<I: IntoIterator<Item = OsString>>(iter: I) -> Self {
        let mut iterator = iter.into_iter();

        // 因为我们在遍历多个 `OsString`，所以可以通过从迭代器取出第一个
        // 字符串、再把后续所有字符串追加到它上面，从而至少省去一次分配。
        match iterator.next() {
            None => OsString::new(),
            Some(mut buf) => {
                buf.extend(iterator);
                buf
            }
        }
    }
}

#[stable(feature = "osstring_extend", since = "1.52.0")]
impl<'a> FromIterator<&'a OsStr> for OsString {
    #[inline]
    fn from_iter<I: IntoIterator<Item = &'a OsStr>>(iter: I) -> Self {
        let mut buf = Self::new();
        for s in iter {
            buf.push(s);
        }
        buf
    }
}

#[stable(feature = "osstring_extend", since = "1.52.0")]
impl<'a> FromIterator<Cow<'a, OsStr>> for OsString {
    #[inline]
    fn from_iter<I: IntoIterator<Item = Cow<'a, OsStr>>>(iter: I) -> Self {
        let mut iterator = iter.into_iter();

        // 因为我们在遍历多个 `OsString`，所以可以通过从迭代器取出第一个
        // 拥有所有权的字符串、再把后续所有字符串追加到它上面，从而至少省去
        // 一次分配。
        match iterator.next() {
            None => OsString::new(),
            Some(Cow::Owned(mut buf)) => {
                buf.extend(iterator);
                buf
            }
            Some(Cow::Borrowed(buf)) => {
                let mut buf = OsString::from(buf);
                buf.extend(iterator);
                buf
            }
        }
    }
}
