//! `ByteStr` 类型及其 trait 实现。
//!
//! `ByteStr` 是面向“可读字节串”的借用类型：它通常承载 UTF-8，但不把 UTF-8
//! 有效性作为类型不变量，因此可用于用户输入、平台文件名或其他需要原样往返字节的数据。

mod traits;

#[unstable(feature = "bstr_internals", issue = "none")]
pub use traits::{impl_partial_eq, impl_partial_eq_n, impl_partial_eq_ord};

use crate::borrow::{Borrow, BorrowMut};
use crate::fmt;
use crate::ops::{Deref, DerefMut, DerefPure};

/// 对 `&[u8]` 的包装，表示按惯例通常是 UTF-8、但不强制要求 UTF-8 有效的人类可读字符串。
///
/// 与 `&str` 不同，`ByteStr` 允许非 UTF-8 内容；因此它适合保存用户输入、
/// 非本机编码文件名（`Path` 只支持本机文件名）以及任何必须把用户提供的字节无损往返的数据。
/// 这也意味着它不能提供 `str` 那类按 Unicode 标量值解释的索引保证。
///
/// 如果需要拥有所有权且可增长的字节字符串缓冲区，请使用
/// [`ByteString`](../../std/bstr/struct.ByteString.html).
///
/// `ByteStr` 实现了到 `[u8]` 的 `Deref`，因此 `[u8]` 上可用的方法也可用于 `ByteStr`。
///
/// # 表示
///
/// `&ByteStr` 与 `&str` 一样是宽指针表示：包含指向字节序列的指针和长度。
/// 不同之处在于 `&str` 还承诺字节是合法 UTF-8，而 `ByteStr` 只承诺底层是 `[u8]`。
///
/// # Trait 实现
///
/// `ByteStr` 提供多种 trait 实现，特别是为了方便使用，定义了 `&ByteStr`、
/// `&str` 与 `&[u8]` 之间的相等性和比较。比较语义基于原始字节，而不是 Unicode
/// 规范化、大小写折叠或用户可感知字符。
///
/// `Debug` 实现把字节尽量显示为普通字符串；无法作为 UTF-8 解码的片段会显示为十六进制转义序列。
///
/// `Display` 实现的效果类似先把 `ByteStr` 有损转换为 `str`：非法 UTF-8 会显示为
/// Unicode 替换字符（�）。
#[unstable(feature = "bstr", issue = "134915")]
#[repr(transparent)]
#[doc(alias = "BStr")]
pub struct ByteStr(pub [u8]);

impl ByteStr {
    /// 从任何可转换为字节切片的值创建 `ByteStr` 切片。
    ///
    /// 这是零成本转换，只改变借用类型，不复制或验证字节。
    ///
    /// # 示例
    ///
    /// 可以从字节数组、字节切片或字符串切片创建 `ByteStr`：
    ///
    /// ```
    /// # #![feature(bstr)]
    /// # use std::bstr::ByteStr;
    /// let a = ByteStr::new(b"abc");
    /// let b = ByteStr::new(&b"abc"[..]);
    /// let c = ByteStr::new("abc");
    ///
    /// assert_eq!(a, b);
    /// assert_eq!(a, c);
    /// ```
    #[inline]
    #[unstable(feature = "bstr", issue = "134915")]
    #[rustc_const_unstable(feature = "const_convert", issue = "143773")]
    pub const fn new<B: ?Sized + [const] AsRef<[u8]>>(bytes: &B) -> &Self {
        ByteStr::from_bytes(bytes.as_ref())
    }

    /// 以 `&ByteStr` 形式返回同一段字节串。
    ///
    /// 直接在 `&ByteStr` 上调用时该方法是冗余的；它的作用是帮助 `Box<ByteStr>`、
    /// `Arc<ByteStr>` 等容器类型在解引用后得到明确的 `&ByteStr`。
    #[inline]
    // #[unstable(feature = "str_as_str", issue = "130366")]
    #[unstable(feature = "bstr", issue = "134915")]
    pub const fn as_byte_str(&self) -> &ByteStr {
        self
    }

    /// 以 `&mut ByteStr` 形式返回同一段可变字节串。
    ///
    /// 直接在 `&mut ByteStr` 上调用时该方法是冗余的；它的作用是帮助 `Box<ByteStr>`、
    /// `MutexGuard<ByteStr>` 等容器类型在解引用后得到明确的 `&mut ByteStr`。
    #[inline]
    // #[unstable(feature = "str_as_str", issue = "130366")]
    #[unstable(feature = "bstr", issue = "134915")]
    pub const fn as_mut_byte_str(&mut self) -> &mut ByteStr {
        self
    }

    #[doc(hidden)]
    #[unstable(feature = "bstr_internals", issue = "none")]
    #[inline]
    #[rustc_const_unstable(feature = "bstr_internals", issue = "none")]
    pub const fn from_bytes(slice: &[u8]) -> &Self {
        // SAFETY: `ByteStr` 是 `[u8]` 的透明包装，没有额外有效性不变量；
        // 因此可以在保持地址、长度和生命周期不变的情况下把被包装类型引用转为包装类型引用。
        unsafe { &*(slice as *const [u8] as *const Self) }
    }

    #[doc(hidden)]
    #[unstable(feature = "bstr_internals", issue = "none")]
    #[inline]
    #[rustc_const_unstable(feature = "bstr_internals", issue = "none")]
    pub const fn from_bytes_mut(slice: &mut [u8]) -> &mut Self {
        // SAFETY: `ByteStr` 是 `[u8]` 的透明包装，没有额外有效性不变量；
        // 可变引用的独占性保持不变，因此可以把 `[u8]` 的可变引用转为 `ByteStr` 的可变引用。
        unsafe { &mut *(slice as *mut [u8] as *mut Self) }
    }

    #[doc(hidden)]
    #[unstable(feature = "bstr_internals", issue = "none")]
    #[inline]
    #[rustc_const_unstable(feature = "bstr_internals", issue = "none")]
    pub const fn as_bytes(&self) -> &[u8] {
        &self.0
    }

    #[doc(hidden)]
    #[unstable(feature = "bstr_internals", issue = "none")]
    #[inline]
    #[rustc_const_unstable(feature = "bstr_internals", issue = "none")]
    pub const fn as_bytes_mut(&mut self) -> &mut [u8] {
        &mut self.0
    }
}

#[unstable(feature = "bstr", issue = "134915")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl const Deref for ByteStr {
    type Target = [u8];

    #[inline]
    fn deref(&self) -> &[u8] {
        &self.0
    }
}

#[unstable(feature = "bstr", issue = "134915")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl const DerefMut for ByteStr {
    #[inline]
    fn deref_mut(&mut self) -> &mut [u8] {
        &mut self.0
    }
}

#[unstable(feature = "deref_pure_trait", issue = "87121")]
unsafe impl DerefPure for ByteStr {}

#[unstable(feature = "bstr", issue = "134915")]
impl fmt::Debug for ByteStr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "\"")?;
        for chunk in self.utf8_chunks() {
            for c in chunk.valid().chars() {
                match c {
                    '\0' => write!(f, "\\0")?,
                    '\x01'..='\x7f' => write!(f, "{}", (c as u8).escape_ascii())?,
                    _ => write!(f, "{}", c.escape_debug())?,
                }
            }
            write!(f, "{}", chunk.invalid().escape_ascii())?;
        }
        write!(f, "\"")?;
        Ok(())
    }
}

#[unstable(feature = "bstr", issue = "134915")]
impl fmt::Display for ByteStr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fn fmt_nopad(this: &ByteStr, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            for chunk in this.utf8_chunks() {
                f.write_str(chunk.valid())?;
                if !chunk.invalid().is_empty() {
                    f.write_str("\u{FFFD}")?;
                }
            }
            Ok(())
        }

        let Some(align) = f.align() else {
            return fmt_nopad(self, f);
        };
        let nchars: usize = self
            .utf8_chunks()
            .map(|chunk| {
                chunk.valid().chars().count() + if chunk.invalid().is_empty() { 0 } else { 1 }
            })
            .sum();
        let padding = f.width().unwrap_or(0).saturating_sub(nchars);
        let fill = f.fill();
        let (lpad, rpad) = match align {
            fmt::Alignment::Left => (0, padding),
            fmt::Alignment::Right => (padding, 0),
            fmt::Alignment::Center => {
                let half = padding / 2;
                (half, half + padding % 2)
            }
        };
        for _ in 0..lpad {
            write!(f, "{fill}")?;
        }
        fmt_nopad(self, f)?;
        for _ in 0..rpad {
            write!(f, "{fill}")?;
        }

        Ok(())
    }
}

#[unstable(feature = "bstr", issue = "134915")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl const AsRef<[u8]> for ByteStr {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        &self.0
    }
}

#[unstable(feature = "bstr", issue = "134915")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl const AsRef<ByteStr> for ByteStr {
    #[inline]
    fn as_ref(&self) -> &ByteStr {
        self
    }
}

// 省略 `impl AsRef<ByteStr> for [u8]`，避免在泛型调用中造成大范围类型推断失败。

#[unstable(feature = "bstr", issue = "134915")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl const AsRef<ByteStr> for str {
    #[inline]
    fn as_ref(&self) -> &ByteStr {
        ByteStr::new(self)
    }
}

#[unstable(feature = "bstr", issue = "134915")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl const AsMut<[u8]> for ByteStr {
    #[inline]
    fn as_mut(&mut self) -> &mut [u8] {
        &mut self.0
    }
}

// 省略 `impl AsMut<ByteStr> for [u8]`，避免在泛型调用中造成大范围类型推断失败。

// 省略 `impl Borrow<ByteStr> for [u8]`，避免在泛型调用中造成大范围类型推断失败。

// 省略 `impl Borrow<ByteStr> for str`，避免在泛型调用中造成大范围类型推断失败。

#[unstable(feature = "bstr", issue = "134915")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl const Borrow<[u8]> for ByteStr {
    #[inline]
    fn borrow(&self) -> &[u8] {
        &self.0
    }
}

// 省略 `impl BorrowMut<ByteStr> for [u8]`，避免在泛型调用中造成大范围类型推断失败。

#[unstable(feature = "bstr", issue = "134915")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl const BorrowMut<[u8]> for ByteStr {
    #[inline]
    fn borrow_mut(&mut self) -> &mut [u8] {
        &mut self.0
    }
}

#[unstable(feature = "bstr", issue = "134915")]
impl<'a> Default for &'a ByteStr {
    fn default() -> Self {
        ByteStr::from_bytes(b"")
    }
}

#[unstable(feature = "bstr", issue = "134915")]
impl<'a> Default for &'a mut ByteStr {
    fn default() -> Self {
        ByteStr::from_bytes_mut(&mut [])
    }
}

// 因类型推断失败风险而省略。
//
// #[unstable(feature = "bstr", issue = "134915")]
// impl<'a, const N: usize> From<&'a [u8; N]> for &'a ByteStr {
//     #[inline]
//     fn from(s: &'a [u8; N]) -> Self {
//         ByteStr::from_bytes(s)
//     }
// }
//
// #[unstable(feature = "bstr", issue = "134915")]
// impl<'a> From<&'a [u8]> for &'a ByteStr {
//     #[inline]
//     fn from(s: &'a [u8]) -> Self {
//         ByteStr::from_bytes(s)
//     }
// }

// 因 slice-from-array-issue-113238 而省略：
//
// #[unstable(feature = "bstr", issue = "134915")]
// impl<'a> From<&'a ByteStr> for &'a [u8] {
//     #[inline]
//     fn from(s: &'a ByteStr) -> Self {
//         &s.0
//     }
// }
//
// #[unstable(feature = "bstr", issue = "134915")]
// impl<'a> From<&'a mut ByteStr> for &'a mut [u8] {
//     #[inline]
//     fn from(s: &'a mut ByteStr) -> Self {
//         &mut s.0
//     }
// }

// 因类型推断失败风险而省略。
//
// #[unstable(feature = "bstr", issue = "134915")]
// impl<'a> From<&'a str> for &'a ByteStr {
//     #[inline]
//     fn from(s: &'a str) -> Self {
//         ByteStr::from_bytes(s.as_bytes())
//     }
// }

#[unstable(feature = "bstr", issue = "134915")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl<'a> const TryFrom<&'a ByteStr> for &'a str {
    type Error = crate::str::Utf8Error;

    #[inline]
    fn try_from(s: &'a ByteStr) -> Result<Self, Self::Error> {
        crate::str::from_utf8(&s.0)
    }
}

#[unstable(feature = "bstr", issue = "134915")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl<'a> const TryFrom<&'a mut ByteStr> for &'a mut str {
    type Error = crate::str::Utf8Error;

    #[inline]
    fn try_from(s: &'a mut ByteStr) -> Result<Self, Self::Error> {
        crate::str::from_utf8_mut(&mut s.0)
    }
}
