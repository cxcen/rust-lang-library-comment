//! 字符串处理。
//!
//! 更多细节见 [`std::str`] 模块。
//!
//! [`std::str`]: ../../std/str/index.html

#![stable(feature = "rust1", since = "1.0.0")]

mod converts;
mod count;
mod error;
mod iter;
mod traits;
mod validations;

use self::pattern::{DoubleEndedSearcher, Pattern, ReverseSearcher, Searcher};
use crate::char::{self, EscapeDebugExtArgs};
use crate::ops::Range;
use crate::slice::{self, SliceIndex};
use crate::ub_checks::assert_unsafe_precondition;
use crate::{ascii, mem};

pub mod pattern;

mod lossy;
#[unstable(feature = "str_from_raw_parts", issue = "119206")]
pub use converts::{from_raw_parts, from_raw_parts_mut};
#[stable(feature = "rust1", since = "1.0.0")]
pub use converts::{from_utf8, from_utf8_unchecked};
#[stable(feature = "str_mut_extras", since = "1.20.0")]
pub use converts::{from_utf8_mut, from_utf8_unchecked_mut};
#[stable(feature = "rust1", since = "1.0.0")]
pub use error::{ParseBoolError, Utf8Error};
#[stable(feature = "encode_utf16", since = "1.8.0")]
pub use iter::EncodeUtf16;
#[stable(feature = "rust1", since = "1.0.0")]
#[allow(deprecated)]
pub use iter::LinesAny;
#[stable(feature = "split_ascii_whitespace", since = "1.34.0")]
pub use iter::SplitAsciiWhitespace;
#[stable(feature = "split_inclusive", since = "1.51.0")]
pub use iter::SplitInclusive;
#[stable(feature = "rust1", since = "1.0.0")]
pub use iter::{Bytes, CharIndices, Chars, Lines, SplitWhitespace};
#[stable(feature = "str_escape", since = "1.34.0")]
pub use iter::{EscapeDebug, EscapeDefault, EscapeUnicode};
#[stable(feature = "str_match_indices", since = "1.5.0")]
pub use iter::{MatchIndices, RMatchIndices};
use iter::{MatchIndicesInternal, MatchesInternal, SplitInternal, SplitNInternal};
#[stable(feature = "str_matches", since = "1.2.0")]
pub use iter::{Matches, RMatches};
#[stable(feature = "rust1", since = "1.0.0")]
pub use iter::{RSplit, RSplitTerminator, Split, SplitTerminator};
#[stable(feature = "rust1", since = "1.0.0")]
pub use iter::{RSplitN, SplitN};
#[stable(feature = "utf8_chunks", since = "1.79.0")]
pub use lossy::{Utf8Chunk, Utf8Chunks};
#[stable(feature = "rust1", since = "1.0.0")]
pub use traits::FromStr;
#[unstable(feature = "str_internals", issue = "none")]
pub use validations::{next_code_point, utf8_char_width};

#[inline(never)]
#[cold]
#[track_caller]
#[rustc_allow_const_fn_unstable(const_eval_select)]
#[cfg(not(panic = "immediate-abort"))]
const fn slice_error_fail(s: &str, begin: usize, end: usize) -> ! {
    crate::intrinsics::const_eval_select((s, begin, end), slice_error_fail_ct, slice_error_fail_rt)
}

#[cfg(panic = "immediate-abort")]
const fn slice_error_fail(s: &str, begin: usize, end: usize) -> ! {
    slice_error_fail_ct(s, begin, end)
}

#[track_caller]
const fn slice_error_fail_ct(_: &str, _: usize, _: usize) -> ! {
    panic!("failed to slice string");
}

#[track_caller]
fn slice_error_fail_rt(s: &str, begin: usize, end: usize) -> ! {
    const MAX_DISPLAY_LENGTH: usize = 256;
    let trunc_len = s.floor_char_boundary(MAX_DISPLAY_LENGTH);
    let s_trunc = &s[..trunc_len];
    let ellipsis = if trunc_len < s.len() { "[...]" } else { "" };

    // 1. 越界。
    if begin > s.len() || end > s.len() {
        let oob_index = if begin > s.len() { begin } else { end };
        panic!("byte index {oob_index} is out of bounds of `{s_trunc}`{ellipsis}");
    }

    // 2. begin <= end。
    assert!(
        begin <= end,
        "begin <= end ({} <= {}) when slicing `{}`{}",
        begin,
        end,
        s_trunc,
        ellipsis
    );

    // 3. 字符边界。
    let index = if !s.is_char_boundary(begin) { begin } else { end };
    // 找到对应的字符。
    let char_start = s.floor_char_boundary(index);
    // `char_start` 必须小于长度，且必须是字符边界。
    let ch = s[char_start..].chars().next().unwrap();
    let char_range = char_start..char_start + ch.len_utf8();
    panic!(
        "byte index {} is not a char boundary; it is inside {:?} (bytes {:?}) of `{}`{}",
        index, ch, char_range, s_trunc, ellipsis
    );
}

impl str {
    /// 返回 `self` 的长度。
    ///
    /// 这个长度以字节为单位，而不是以 [`char`] 或字素簇为单位。换句话说，
    /// 它不一定等同于人类直观理解的“字符串长度”。
    ///
    /// [`char`]: prim@char
    ///
    /// # 示例
    ///
    /// ```
    /// let len = "foo".len();
    /// assert_eq!(3, len);
    ///
    /// assert_eq!("ƒoo".len(), 4); // 花体 f！
    /// assert_eq!("ƒoo".chars().count(), 3);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_str_len", since = "1.39.0")]
    #[rustc_diagnostic_item = "str_len"]
    #[rustc_no_implicit_autorefs]
    #[must_use]
    #[inline]
    pub const fn len(&self) -> usize {
        self.as_bytes().len()
    }

    /// 如果 `self` 的字节长度为零，返回 `true`。
    ///
    /// # 示例
    ///
    /// ```
    /// let s = "";
    /// assert!(s.is_empty());
    ///
    /// let s = "not empty";
    /// assert!(!s.is_empty());
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_str_is_empty", since = "1.39.0")]
    #[rustc_no_implicit_autorefs]
    #[must_use]
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// 将字节切片转换为字符串切片。
    ///
    /// 字符串切片（[`&str`]）由字节（[`u8`]）组成，字节切片（[`&[u8]`][byteslice]）
    /// 同样由字节组成，因此本函数在二者之间转换。但并非所有字节切片都是有效字符串切片：
    /// [`&str`] 要求内容必须是有效 UTF-8。`from_utf8()` 会先检查字节满足 UTF-8
    /// 有效性不变量，再执行转换。
    ///
    /// [`&str`]: str
    /// [byteslice]: prim@slice
    ///
    /// 如果你确信字节切片是有效 UTF-8，并且不想承担有效性检查开销，可以使用本函数的
    /// unsafe 版本 [`from_utf8_unchecked`]。它行为相同，但会跳过检查；调用方必须自行保证
    /// `from_utf8_unchecked` 的前置条件。
    ///
    /// 如果你需要的是 `String` 而不是 `&str`，可以考虑 [`String::from_utf8`][string]。
    ///
    /// [string]: ../std/string/struct.String.html#method.from_utf8
    ///
    /// 因为 `[u8; N]` 可以分配在栈上，并且可以从中取得 [`&[u8]`][byteslice]，
    /// 所以本函数也是构造栈上字符串的一种方式。下方示例展示了这一用法。
    ///
    /// [byteslice]: slice
    ///
    /// # 错误
    ///
    /// 如果切片不是有效 UTF-8，则返回 `Err`，并附带说明为什么给定切片不满足
    /// UTF-8 要求的信息。
    ///
    /// # 示例
    ///
    /// 基本用法：
    ///
    /// ```
    /// // vector 中的一些字节
    /// let sparkle_heart = vec![240, 159, 146, 150];
    ///
    /// // 可以使用 ?（try）运算符检查这些字节是否有效
    /// let sparkle_heart = str::from_utf8(&sparkle_heart)?;
    ///
    /// assert_eq!("💖", sparkle_heart);
    /// # Ok::<_, std::str::Utf8Error>(())
    /// ```
    ///
    /// 错误字节：
    ///
    /// ```
    /// // vector 中的一些无效字节
    /// let sparkle_heart = vec![0, 159, 146, 150];
    ///
    /// assert!(str::from_utf8(&sparkle_heart).is_err());
    /// ```
    ///
    /// 关于可能返回的错误种类，更多细节见 [`Utf8Error`] 的文档。
    ///
    /// “栈上分配的字符串”：
    ///
    /// ```
    /// // 栈上分配数组中的一些字节
    /// let sparkle_heart = [240, 159, 146, 150];
    ///
    /// // 已知这些字节有效，因此直接使用 `unwrap()`。
    /// let sparkle_heart: &str = str::from_utf8(&sparkle_heart).unwrap();
    ///
    /// assert_eq!("💖", sparkle_heart);
    /// ```
    #[stable(feature = "inherent_str_constructors", since = "1.87.0")]
    #[rustc_const_stable(feature = "inherent_str_constructors", since = "1.87.0")]
    #[rustc_diagnostic_item = "str_inherent_from_utf8"]
    pub const fn from_utf8(v: &[u8]) -> Result<&str, Utf8Error> {
        converts::from_utf8(v)
    }

    /// 将可变字节切片转换为可变字符串切片。
    ///
    /// # 示例
    ///
    /// 基本用法：
    ///
    /// ```
    /// // 用可变 vector 表示 "Hello, Rust!"
    /// let mut hellorust = vec![72, 101, 108, 108, 111, 44, 32, 82, 117, 115, 116, 33];
    ///
    /// // 已知这些字节有效，因此可以使用 `unwrap()`
    /// let outstr = str::from_utf8_mut(&mut hellorust).unwrap();
    ///
    /// assert_eq!("Hello, Rust!", outstr);
    /// ```
    ///
    /// 错误字节：
    ///
    /// ```
    /// // 可变 vector 中的一些无效字节
    /// let mut invalid = vec![128, 223];
    ///
    /// assert!(str::from_utf8_mut(&mut invalid).is_err());
    /// ```
    /// 关于可能返回的错误种类，更多细节见 [`Utf8Error`] 的文档。
    #[stable(feature = "inherent_str_constructors", since = "1.87.0")]
    #[rustc_const_stable(feature = "const_str_from_utf8", since = "1.87.0")]
    #[rustc_diagnostic_item = "str_inherent_from_utf8_mut"]
    pub const fn from_utf8_mut(v: &mut [u8]) -> Result<&mut str, Utf8Error> {
        converts::from_utf8_mut(v)
    }

    /// 将字节切片转换为字符串切片，但不检查其中是否包含有效 UTF-8。
    ///
    /// 更多信息见安全版本 [`from_utf8`]。
    ///
    /// # 安全性(Safety）
    ///
    /// 传入的字节必须是有效 UTF-8。调用方必须在调用 `from_utf8_unchecked`
    /// 前确认每个字节序列都满足 UTF-8 编码规则；否则构造出的 `&str` 会破坏核心不变量。
    ///
    /// # 示例
    ///
    /// 基本用法：
    ///
    /// ```
    /// // vector 中的一些字节
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
    #[stable(feature = "inherent_str_constructors", since = "1.87.0")]
    #[rustc_const_stable(feature = "inherent_str_constructors", since = "1.87.0")]
    #[rustc_diagnostic_item = "str_inherent_from_utf8_unchecked"]
    pub const unsafe fn from_utf8_unchecked(v: &[u8]) -> &str {
        // SAFETY: converts::from_utf8_unchecked 与本函数具有相同安全要求。
        unsafe { converts::from_utf8_unchecked(v) }
    }

    /// 将字节切片转换为字符串切片，但不检查其中是否包含有效 UTF-8；这是可变版本。
    ///
    /// 文档和安全要求见不可变版本 [`from_utf8_unchecked()`]。
    ///
    /// # 示例
    ///
    /// 基本用法：
    ///
    /// ```
    /// let mut heart = vec![240, 159, 146, 150];
    /// let heart = unsafe { str::from_utf8_unchecked_mut(&mut heart) };
    ///
    /// assert_eq!("💖", heart);
    /// ```
    #[inline]
    #[must_use]
    #[stable(feature = "inherent_str_constructors", since = "1.87.0")]
    #[rustc_const_stable(feature = "inherent_str_constructors", since = "1.87.0")]
    #[rustc_diagnostic_item = "str_inherent_from_utf8_unchecked_mut"]
    pub const unsafe fn from_utf8_unchecked_mut(v: &mut [u8]) -> &mut str {
        // SAFETY: converts::from_utf8_unchecked_mut 与本函数具有相同安全要求。
        unsafe { converts::from_utf8_unchecked_mut(v) }
    }

    /// 检查第 `index` 个字节是否是某个 UTF-8 code point 序列的首字节，
    /// 或者是否正好位于字符串末尾。
    ///
    /// 字符串开头和字符串结尾（当 `index == self.len()` 时）都视为边界。
    ///
    /// 如果 `index` 大于 `self.len()`，返回 `false`。
    ///
    /// # 示例
    ///
    /// ```
    /// let s = "Löwe 老虎 Léopard";
    /// assert!(s.is_char_boundary(0));
    /// // `老` 的起始位置
    /// assert!(s.is_char_boundary(6));
    /// assert!(s.is_char_boundary(s.len()));
    ///
    /// // `ö` 的第二个字节
    /// assert!(!s.is_char_boundary(2));
    ///
    /// // `老` 的第三个字节
    /// assert!(!s.is_char_boundary(8));
    /// ```
    #[must_use]
    #[stable(feature = "is_char_boundary", since = "1.9.0")]
    #[rustc_const_stable(feature = "const_is_char_boundary", since = "1.86.0")]
    #[inline]
    pub const fn is_char_boundary(&self, index: usize) -> bool {
        // 0 永远是有效边界。
        // 显式检测 0 可让编译器轻松优化掉该分支的检查，并跳过读取字符串数据。
        // 注意，`self.get(..index)` 的优化依赖这一点。
        if index == 0 {
            return true;
        }

        if index >= self.len() {
            // 返回 `true` 的情况有两种：
            //
            // - index == self.len()
            //   空字符串是有效边界，因此返回 true。
            // - index > self.len()
            //   此时返回 false。
            //
            // 该检查刻意放在这里，因为它能改善高优化级别下生成的代码。
            // 更多细节见 PR #84751。
            index == self.len()
        } else {
            self.as_bytes()[index].is_utf8_char_boundary()
        }
    }

    /// 查找不超过 `index` 且满足 [`is_char_boundary(x)`] 为 `true` 的最近 `x`。
    ///
    /// 本方法可帮助你按给定字节数截断字符串，同时仍保持有效 UTF-8。注意，这只在字符层面
    /// 操作，因此即使底层字符没有被拆开，视觉上的字素簇仍可能被切开。例如，emoji 🧑‍🔬
    ///（科学家）可能被截成只包含 🧑（人）。
    ///
    /// [`is_char_boundary(x)`]: Self::is_char_boundary
    ///
    /// # 示例
    ///
    /// ```
    /// let s = "❤️🧡💛💚💙💜";
    /// assert_eq!(s.len(), 26);
    /// assert!(!s.is_char_boundary(13));
    ///
    /// let closest = s.floor_char_boundary(13);
    /// assert_eq!(closest, 10);
    /// assert_eq!(&s[..closest], "❤️🧡");
    /// ```
    #[stable(feature = "round_char_boundary", since = "1.91.0")]
    #[rustc_const_stable(feature = "round_char_boundary", since = "1.91.0")]
    #[inline]
    pub const fn floor_char_boundary(&self, index: usize) -> usize {
        if index >= self.len() {
            self.len()
        } else {
            let mut i = index;
            while i > 0 {
                if self.as_bytes()[i].is_utf8_char_boundary() {
                    break;
                }
                i -= 1;
            }

            // 字符边界一定位于 index 附近四个字节以内。
            debug_assert!(i >= index.saturating_sub(3));

            i
        }
    }

    /// 查找不小于 `index` 且满足 [`is_char_boundary(x)`] 为 `true` 的最近 `x`。
    ///
    /// 如果 `index` 大于字符串长度，则返回字符串长度。
    ///
    /// 这是 [`floor_char_boundary`] 的自然互补方法。更多细节见该方法。
    ///
    /// [`floor_char_boundary`]: str::floor_char_boundary
    /// [`is_char_boundary(x)`]: Self::is_char_boundary
    ///
    /// # 示例
    ///
    /// ```
    /// let s = "❤️🧡💛💚💙💜";
    /// assert_eq!(s.len(), 26);
    /// assert!(!s.is_char_boundary(13));
    ///
    /// let closest = s.ceil_char_boundary(13);
    /// assert_eq!(closest, 14);
    /// assert_eq!(&s[..closest], "❤️🧡💛");
    /// ```
    #[stable(feature = "round_char_boundary", since = "1.91.0")]
    #[rustc_const_stable(feature = "round_char_boundary", since = "1.91.0")]
    #[inline]
    pub const fn ceil_char_boundary(&self, index: usize) -> usize {
        if index >= self.len() {
            self.len()
        } else {
            let mut i = index;
            while i < self.len() {
                if self.as_bytes()[i].is_utf8_char_boundary() {
                    break;
                }
                i += 1;
            }

            // 字符边界一定位于 index 附近四个字节以内。
            debug_assert!(i <= index + 3);

            i
        }
    }

    /// 将字符串切片转换为字节切片。要把字节切片再转换回字符串切片，请使用
    /// [`from_utf8`] 函数。
    ///
    /// # 示例
    ///
    /// ```
    /// let bytes = "bors".as_bytes();
    /// assert_eq!(b"bors", bytes);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "str_as_bytes", since = "1.39.0")]
    #[must_use]
    #[inline(always)]
    #[allow(unused_attributes)]
    pub const fn as_bytes(&self) -> &[u8] {
        // SAFETY: const 中这是可靠的，因为这里转换的是两个布局相同的类型。
        unsafe { mem::transmute(self) }
    }

    /// 将可变字符串切片转换为可变字节切片。
    ///
    /// # 安全性(Safety）
    ///
    /// 调用方必须确保在借用结束且底层 `str` 再次被使用前，切片内容仍然是有效 UTF-8。
    ///
    /// 使用内容不是有效 UTF-8 的 `str` 会导致未定义行为。
    ///
    /// # 示例
    ///
    /// 基本用法：
    ///
    /// ```
    /// let mut s = String::from("Hello");
    /// let bytes = unsafe { s.as_bytes_mut() };
    ///
    /// assert_eq!(b"Hello", bytes);
    /// ```
    ///
    /// 可变性：
    ///
    /// ```
    /// let mut s = String::from("🗻∈🌏");
    ///
    /// unsafe {
    ///     let bytes = s.as_bytes_mut();
    ///
    ///     bytes[0] = 0xF0;
    ///     bytes[1] = 0x9F;
    ///     bytes[2] = 0x8D;
    ///     bytes[3] = 0x94;
    /// }
    ///
    /// assert_eq!("🍔∈🌏", s);
    /// ```
    #[stable(feature = "str_mut_extras", since = "1.20.0")]
    #[rustc_const_stable(feature = "const_str_as_mut", since = "1.83.0")]
    #[must_use]
    #[inline(always)]
    pub const unsafe fn as_bytes_mut(&mut self) -> &mut [u8] {
        // SAFETY: 从 `&str` 转换到 `&[u8]` 是安全的，因为 `str` 与 `&[u8]`
        // 具有相同布局（只有 std 可以作出这个保证）。
        // 指针解引用是安全的，因为它来自保证可写的可变引用。
        unsafe { &mut *(self as *mut str as *mut [u8]) }
    }

    /// 将字符串切片转换为裸指针。
    ///
    /// 由于字符串切片本质上是字节切片，裸指针指向 [`u8`]。该指针会指向字符串切片的
    /// 第一个字节。
    ///
    /// 调用方必须确保永远不通过返回的指针写入。如果需要修改字符串切片内容，请使用
    /// [`as_mut_ptr`]。
    ///
    /// [`as_mut_ptr`]: str::as_mut_ptr
    ///
    /// # 示例
    ///
    /// ```
    /// let s = "Hello";
    /// let ptr = s.as_ptr();
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "rustc_str_as_ptr", since = "1.32.0")]
    #[rustc_never_returns_null_ptr]
    #[rustc_as_ptr]
    #[must_use]
    #[inline(always)]
    pub const fn as_ptr(&self) -> *const u8 {
        self as *const str as *const u8
    }

    /// 将可变字符串切片转换为裸指针。
    ///
    /// 由于字符串切片本质上是字节切片，裸指针指向 [`u8`]。该指针会指向字符串切片的
    /// 第一个字节。
    ///
    /// 你必须确保对字符串切片的修改方式始终保持其内容为有效 UTF-8。
    #[stable(feature = "str_as_mut_ptr", since = "1.36.0")]
    #[rustc_const_stable(feature = "const_str_as_mut", since = "1.83.0")]
    #[rustc_never_returns_null_ptr]
    #[rustc_as_ptr]
    #[must_use]
    #[inline(always)]
    pub const fn as_mut_ptr(&mut self) -> *mut u8 {
        self as *mut str as *mut u8
    }

    /// 返回 `str` 的子切片。
    ///
    /// 这是索引 `str` 的非 panic 替代方案。只要等价的索引操作会 panic，
    /// 本方法就返回 [`None`]。
    ///
    /// # 示例
    ///
    /// ```
    /// let v = String::from("🗻∈🌏");
    ///
    /// assert_eq!(Some("🗻"), v.get(0..4));
    ///
    /// // 不在 UTF-8 序列边界上的索引
    /// assert!(v.get(1..).is_none());
    /// assert!(v.get(..8).is_none());
    ///
    /// // 越界
    /// assert!(v.get(..42).is_none());
    /// ```
    #[stable(feature = "str_checked_slicing", since = "1.20.0")]
    #[rustc_const_unstable(feature = "const_index", issue = "143775")]
    #[inline]
    pub const fn get<I: [const] SliceIndex<str>>(&self, i: I) -> Option<&I::Output> {
        i.get(self)
    }

    /// 返回 `str` 的可变子切片。
    ///
    /// 这是索引 `str` 的非 panic 替代方案。只要等价的索引操作会 panic，
    /// 本方法就返回 [`None`]。
    ///
    /// # 示例
    ///
    /// ```
    /// let mut v = String::from("hello");
    /// // 正确长度
    /// assert!(v.get_mut(0..5).is_some());
    /// // 越界
    /// assert!(v.get_mut(..42).is_none());
    /// assert_eq!(Some("he"), v.get_mut(0..2).map(|v| &*v));
    ///
    /// assert_eq!("hello", v);
    /// {
    ///     let s = v.get_mut(0..2);
    ///     let s = s.map(|s| {
    ///         s.make_ascii_uppercase();
    ///         &*s
    ///     });
    ///     assert_eq!(Some("HE"), s);
    /// }
    /// assert_eq!("HEllo", v);
    /// ```
    #[stable(feature = "str_checked_slicing", since = "1.20.0")]
    #[rustc_const_unstable(feature = "const_index", issue = "143775")]
    #[inline]
    pub const fn get_mut<I: [const] SliceIndex<str>>(&mut self, i: I) -> Option<&mut I::Output> {
        i.get_mut(self)
    }

    /// 返回 `str` 的未检查子切片。
    ///
    /// 这是索引 `str` 的未检查替代方案。
    ///
    /// # 安全性(Safety）
    ///
    /// 本函数调用方负责确保满足以下前置条件：
    ///
    /// * 起始索引不得超过结束索引；
    /// * 索引必须位于原始切片边界内；
    /// * 索引必须位于 UTF-8 序列边界上。
    ///
    /// 如果这些条件不成立，返回的字符串切片可能引用无效内存，或违反 `str`
    /// 类型承诺的 UTF-8 有效性不变量。
    ///
    /// # 示例
    ///
    /// ```
    /// let v = "🗻∈🌏";
    /// unsafe {
    ///     assert_eq!("🗻", v.get_unchecked(0..4));
    ///     assert_eq!("∈", v.get_unchecked(4..7));
    ///     assert_eq!("🌏", v.get_unchecked(7..11));
    /// }
    /// ```
    #[stable(feature = "str_checked_slicing", since = "1.20.0")]
    #[inline]
    pub unsafe fn get_unchecked<I: SliceIndex<str>>(&self, i: I) -> &I::Output {
        // SAFETY: 调用方必须遵守 `get_unchecked` 的安全契约；
        // 由于 `self` 是安全引用，该切片可解引用。
        // 返回的指针是安全的，因为 `SliceIndex` 的实现必须保证这一点。
        unsafe { &*i.get_unchecked(self) }
    }

    /// 返回 `str` 的可变、未检查子切片。
    ///
    /// 这是索引 `str` 的未检查替代方案。
    ///
    /// # 安全性(Safety）
    ///
    /// 本函数调用方负责确保满足以下前置条件：
    ///
    /// * 起始索引不得超过结束索引；
    /// * 索引必须位于原始切片边界内；
    /// * 索引必须位于 UTF-8 序列边界上。
    ///
    /// 如果这些条件不成立，返回的字符串切片可能引用无效内存，或违反 `str`
    /// 类型承诺的 UTF-8 有效性不变量。
    ///
    /// # 示例
    ///
    /// ```
    /// let mut v = String::from("🗻∈🌏");
    /// unsafe {
    ///     assert_eq!("🗻", v.get_unchecked_mut(0..4));
    ///     assert_eq!("∈", v.get_unchecked_mut(4..7));
    ///     assert_eq!("🌏", v.get_unchecked_mut(7..11));
    /// }
    /// ```
    #[stable(feature = "str_checked_slicing", since = "1.20.0")]
    #[inline]
    pub unsafe fn get_unchecked_mut<I: SliceIndex<str>>(&mut self, i: I) -> &mut I::Output {
        // SAFETY: 调用方必须遵守 `get_unchecked_mut` 的安全契约；
        // 由于 `self` 是安全引用，该切片可解引用。
        // 返回的指针是安全的，因为 `SliceIndex` 的实现必须保证这一点。
        unsafe { &mut *i.get_unchecked_mut(self) }
    }

    /// 从另一个字符串切片创建字符串切片，绕过安全检查。
    ///
    /// 通常不建议这样做，请谨慎使用。安全替代方案见 [`str`] 和 [`Index`]。
    ///
    /// [`Index`]: crate::ops::Index
    ///
    /// 新切片从 `begin` 到 `end`，包含 `begin`，但不包含 `end`。
    ///
    /// 如果要取得可变字符串切片，见 [`slice_mut_unchecked`] 方法。
    ///
    /// [`slice_mut_unchecked`]: str::slice_mut_unchecked
    ///
    /// # 安全性(Safety）
    ///
    /// 本函数调用方负责确保满足三个前置条件：
    ///
    /// * `begin` 不得超过 `end`。
    /// * `begin` 和 `end` 必须是字符串切片内的字节位置。
    /// * `begin` 和 `end` 必须位于 UTF-8 序列边界上。
    ///
    /// # 示例
    ///
    /// ```
    /// let s = "Löwe 老虎 Léopard";
    ///
    /// unsafe {
    ///     assert_eq!("Löwe 老虎 Léopard", s.slice_unchecked(0, 21));
    /// }
    ///
    /// let s = "Hello, world!";
    ///
    /// unsafe {
    ///     assert_eq!("world", s.slice_unchecked(7, 12));
    /// }
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[deprecated(since = "1.29.0", note = "use `get_unchecked(begin..end)` instead")]
    #[must_use]
    #[inline]
    pub unsafe fn slice_unchecked(&self, begin: usize, end: usize) -> &str {
        // SAFETY: 调用方必须遵守 `get_unchecked` 的安全契约；
        // 由于 `self` 是安全引用，该切片可解引用。
        // 返回的指针是安全的，因为 `SliceIndex` 的实现必须保证这一点。
        unsafe { &*(begin..end).get_unchecked(self) }
    }

    /// 从另一个字符串切片创建字符串切片，绕过安全检查。
    ///
    /// 通常不建议这样做，请谨慎使用。安全替代方案见 [`str`] 和 [`IndexMut`]。
    ///
    /// [`IndexMut`]: crate::ops::IndexMut
    ///
    /// 新切片从 `begin` 到 `end`，包含 `begin`，但不包含 `end`。
    ///
    /// 如果要取得不可变字符串切片，见 [`slice_unchecked`] 方法。
    ///
    /// [`slice_unchecked`]: str::slice_unchecked
    ///
    /// # 安全性(Safety）
    ///
    /// 本函数调用方负责确保满足三个前置条件：
    ///
    /// * `begin` 不得超过 `end`。
    /// * `begin` 和 `end` 必须是字符串切片内的字节位置。
    /// * `begin` 和 `end` 必须位于 UTF-8 序列边界上。
    #[stable(feature = "str_slice_mut", since = "1.5.0")]
    #[deprecated(since = "1.29.0", note = "use `get_unchecked_mut(begin..end)` instead")]
    #[inline]
    pub unsafe fn slice_mut_unchecked(&mut self, begin: usize, end: usize) -> &mut str {
        // SAFETY: 调用方必须遵守 `get_unchecked_mut` 的安全契约；
        // 由于 `self` 是安全引用，该切片可解引用。
        // 返回的指针是安全的，因为 `SliceIndex` 的实现必须保证这一点。
        unsafe { &mut *(begin..end).get_unchecked_mut(self) }
    }

    /// 在给定索引处将一个字符串切片拆成两个。
    ///
    /// 参数 `mid` 应当是从字符串开头算起的字节偏移，并且必须位于 UTF-8 code point
    /// 的边界上。
    ///
    /// 返回的两个切片分别覆盖从字符串切片开头到 `mid`，以及从 `mid` 到字符串切片结尾。
    ///
    /// 如果要取得可变字符串切片，见 [`split_at_mut`] 方法。
    ///
    /// [`split_at_mut`]: str::split_at_mut
    ///
    /// # Panics
    ///
    /// Panic 条件：`mid` 不在 UTF-8 code point 边界上，或者超过了字符串切片最后一个
    /// code point 的末尾。非 panic 替代方案见 [`split_at_checked`](str::split_at_checked)。
    ///
    /// # 示例
    ///
    /// ```
    /// let s = "Per Martin-Löf";
    ///
    /// let (first, last) = s.split_at(3);
    ///
    /// assert_eq!("Per", first);
    /// assert_eq!(" Martin-Löf", last);
    /// ```
    #[inline]
    #[must_use]
    #[stable(feature = "str_split_at", since = "1.4.0")]
    #[rustc_const_stable(feature = "const_str_split_at", since = "1.86.0")]
    pub const fn split_at(&self, mid: usize) -> (&str, &str) {
        match self.split_at_checked(mid) {
            None => slice_error_fail(self, 0, mid),
            Some(pair) => pair,
        }
    }

    /// 在给定索引处将一个可变字符串切片拆成两个。
    ///
    /// 参数 `mid` 应当是从字符串开头算起的字节偏移，并且必须位于 UTF-8 code point
    /// 的边界上。
    ///
    /// 返回的两个切片分别覆盖从字符串切片开头到 `mid`，以及从 `mid` 到字符串切片结尾。
    ///
    /// 如果要取得不可变字符串切片，见 [`split_at`] 方法。
    ///
    /// [`split_at`]: str::split_at
    ///
    /// # Panics
    ///
    /// Panic 条件：`mid` 不在 UTF-8 code point 边界上，或者超过了字符串切片最后一个
    /// code point 的末尾。非 panic 替代方案见
    /// [`split_at_mut_checked`](str::split_at_mut_checked)。
    ///
    /// # 示例
    ///
    /// ```
    /// let mut s = "Per Martin-Löf".to_string();
    /// {
    ///     let (first, last) = s.split_at_mut(3);
    ///     first.make_ascii_uppercase();
    ///     assert_eq!("PER", first);
    ///     assert_eq!(" Martin-Löf", last);
    /// }
    /// assert_eq!("PER Martin-Löf", s);
    /// ```
    #[inline]
    #[must_use]
    #[stable(feature = "str_split_at", since = "1.4.0")]
    #[rustc_const_stable(feature = "const_str_split_at", since = "1.86.0")]
    pub const fn split_at_mut(&mut self, mid: usize) -> (&mut str, &mut str) {
        // is_char_boundary 会检查索引位于 [0, .len()] 范围内。
        if self.is_char_boundary(mid) {
            // SAFETY: 刚刚已经检查 `mid` 位于 char 边界上。
            unsafe { self.split_at_mut_unchecked(mid) }
        } else {
            slice_error_fail(self, 0, mid)
        }
    }

    /// 在给定索引处将一个字符串切片拆成两个。
    ///
    /// 参数 `mid` 应当是从字符串开头算起的有效字节偏移，并且必须位于 UTF-8 code point
    /// 的边界上。如果不是这样，本方法返回 `None`。
    ///
    /// 返回的两个切片分别覆盖从字符串切片开头到 `mid`，以及从 `mid` 到字符串切片结尾。
    ///
    /// 如果要取得可变字符串切片，见 [`split_at_mut_checked`] 方法。
    ///
    /// [`split_at_mut_checked`]: str::split_at_mut_checked
    ///
    /// # 示例
    ///
    /// ```
    /// let s = "Per Martin-Löf";
    ///
    /// let (first, last) = s.split_at_checked(3).unwrap();
    /// assert_eq!("Per", first);
    /// assert_eq!(" Martin-Löf", last);
    ///
    /// assert_eq!(None, s.split_at_checked(13));  // 位于 “ö” 内部
    /// assert_eq!(None, s.split_at_checked(16));  // 超过字符串长度
    /// ```
    #[inline]
    #[must_use]
    #[stable(feature = "split_at_checked", since = "1.80.0")]
    #[rustc_const_stable(feature = "const_str_split_at", since = "1.86.0")]
    pub const fn split_at_checked(&self, mid: usize) -> Option<(&str, &str)> {
        // is_char_boundary 会检查索引位于 [0, .len()] 范围内。
        if self.is_char_boundary(mid) {
            // SAFETY: 刚刚已经检查 `mid` 位于 char 边界上。
            Some(unsafe { self.split_at_unchecked(mid) })
        } else {
            None
        }
    }

    /// 在给定索引处将一个可变字符串切片拆成两个。
    ///
    /// 参数 `mid` 应当是从字符串开头算起的有效字节偏移，并且必须位于 UTF-8 code point
    /// 的边界上。如果不是这样，本方法返回 `None`。
    ///
    /// 返回的两个切片分别覆盖从字符串切片开头到 `mid`，以及从 `mid` 到字符串切片结尾。
    ///
    /// 如果要取得不可变字符串切片，见 [`split_at_checked`] 方法。
    ///
    /// [`split_at_checked`]: str::split_at_checked
    ///
    /// # 示例
    ///
    /// ```
    /// let mut s = "Per Martin-Löf".to_string();
    /// if let Some((first, last)) = s.split_at_mut_checked(3) {
    ///     first.make_ascii_uppercase();
    ///     assert_eq!("PER", first);
    ///     assert_eq!(" Martin-Löf", last);
    /// }
    /// assert_eq!("PER Martin-Löf", s);
    ///
    /// assert_eq!(None, s.split_at_mut_checked(13));  // 位于 “ö” 内部
    /// assert_eq!(None, s.split_at_mut_checked(16));  // 超过字符串长度
    /// ```
    #[inline]
    #[must_use]
    #[stable(feature = "split_at_checked", since = "1.80.0")]
    #[rustc_const_stable(feature = "const_str_split_at", since = "1.86.0")]
    pub const fn split_at_mut_checked(&mut self, mid: usize) -> Option<(&mut str, &mut str)> {
        // is_char_boundary 会检查索引位于 [0, .len()] 范围内。
        if self.is_char_boundary(mid) {
            // SAFETY: 刚刚已经检查 `mid` 位于 char 边界上。
            Some(unsafe { self.split_at_mut_unchecked(mid) })
        } else {
            None
        }
    }

    /// 在给定索引处将一个字符串切片拆成两个。
    ///
    /// # 安全性(Safety）
    ///
    /// 调用方必须确保 `mid` 是从字符串开头算起的有效字节偏移，并且位于 UTF-8
    /// code point 边界上。
    #[inline]
    const unsafe fn split_at_unchecked(&self, mid: usize) -> (&str, &str) {
        let len = self.len();
        let ptr = self.as_ptr();
        // SAFETY: 调用方保证 `mid` 位于 char 边界上。
        unsafe {
            (
                from_utf8_unchecked(slice::from_raw_parts(ptr, mid)),
                from_utf8_unchecked(slice::from_raw_parts(ptr.add(mid), len - mid)),
            )
        }
    }

    /// 在给定索引处将一个字符串切片拆成两个。
    ///
    /// # 安全性(Safety）
    ///
    /// 调用方必须确保 `mid` 是从字符串开头算起的有效字节偏移，并且位于 UTF-8
    /// code point 边界上。
    const unsafe fn split_at_mut_unchecked(&mut self, mid: usize) -> (&mut str, &mut str) {
        let len = self.len();
        let ptr = self.as_mut_ptr();
        // SAFETY: 调用方保证 `mid` 位于 char 边界上。
        unsafe {
            (
                from_utf8_unchecked_mut(slice::from_raw_parts_mut(ptr, mid)),
                from_utf8_unchecked_mut(slice::from_raw_parts_mut(ptr.add(mid), len - mid)),
            )
        }
    }

    /// 返回遍历字符串切片中各个 [`char`] 的迭代器。
    ///
    /// 字符串切片由有效 UTF-8 组成，因此可以按 [`char`] 遍历。本方法返回这样的迭代器。
    ///
    /// 需要记住，[`char`] 表示 Unicode 标量值，可能并不等同于你直觉中的“字符”。
    /// 你真正需要的可能是按字素簇遍历。Rust 标准库不提供此功能，可在 crates.io
    /// 上寻找相关 crate。
    ///
    /// # 示例
    ///
    /// 基本用法：
    ///
    /// ```
    /// let word = "goodbye";
    ///
    /// let count = word.chars().count();
    /// assert_eq!(7, count);
    ///
    /// let mut chars = word.chars();
    ///
    /// assert_eq!(Some('g'), chars.next());
    /// assert_eq!(Some('o'), chars.next());
    /// assert_eq!(Some('o'), chars.next());
    /// assert_eq!(Some('d'), chars.next());
    /// assert_eq!(Some('b'), chars.next());
    /// assert_eq!(Some('y'), chars.next());
    /// assert_eq!(Some('e'), chars.next());
    ///
    /// assert_eq!(None, chars.next());
    /// ```
    ///
    /// 请记住，[`char`] 可能并不符合你对“字符”的直觉：
    ///
    /// [`char`]: prim@char
    ///
    /// ```
    /// let y = "y̆";
    ///
    /// let mut chars = y.chars();
    ///
    /// assert_eq!(Some('y'), chars.next()); // 不是 'y̆'
    /// assert_eq!(Some('\u{0306}'), chars.next());
    ///
    /// assert_eq!(None, chars.next());
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    #[rustc_diagnostic_item = "str_chars"]
    pub fn chars(&self) -> Chars<'_> {
        Chars { iter: self.as_bytes().iter() }
    }

    /// 返回遍历字符串切片中各个 [`char`] 及其位置的迭代器。
    ///
    /// 字符串切片由有效 UTF-8 组成，因此可以按 [`char`] 遍历。本方法返回的迭代器
    /// 同时产出这些 [`char`] 及其字节位置。
    ///
    /// 迭代器产生元组：第一个元素是位置，第二个元素是 [`char`]。
    ///
    /// # 示例
    ///
    /// 基本用法：
    ///
    /// ```
    /// let word = "goodbye";
    ///
    /// let count = word.char_indices().count();
    /// assert_eq!(7, count);
    ///
    /// let mut char_indices = word.char_indices();
    ///
    /// assert_eq!(Some((0, 'g')), char_indices.next());
    /// assert_eq!(Some((1, 'o')), char_indices.next());
    /// assert_eq!(Some((2, 'o')), char_indices.next());
    /// assert_eq!(Some((3, 'd')), char_indices.next());
    /// assert_eq!(Some((4, 'b')), char_indices.next());
    /// assert_eq!(Some((5, 'y')), char_indices.next());
    /// assert_eq!(Some((6, 'e')), char_indices.next());
    ///
    /// assert_eq!(None, char_indices.next());
    /// ```
    ///
    /// 请记住，[`char`] 可能并不符合你对“字符”的直觉：
    ///
    /// [`char`]: prim@char
    ///
    /// ```
    /// let yes = "y̆es";
    ///
    /// let mut char_indices = yes.char_indices();
    ///
    /// assert_eq!(Some((0, 'y')), char_indices.next()); // 不是 (0, 'y̆')
    /// assert_eq!(Some((1, '\u{0306}')), char_indices.next());
    ///
    /// // 注意这里是 3，因为前一个字符占用了两个字节
    /// assert_eq!(Some((3, 'e')), char_indices.next());
    /// assert_eq!(Some((4, 's')), char_indices.next());
    ///
    /// assert_eq!(None, char_indices.next());
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn char_indices(&self) -> CharIndices<'_> {
        CharIndices { front_offset: 0, iter: self.chars() }
    }

    /// 返回遍历字符串切片字节的迭代器。
    ///
    /// 字符串切片由字节序列组成，因此可以按字节遍历。本方法返回这样的迭代器。
    ///
    /// # 示例
    ///
    /// ```
    /// let mut bytes = "bors".bytes();
    ///
    /// assert_eq!(Some(b'b'), bytes.next());
    /// assert_eq!(Some(b'o'), bytes.next());
    /// assert_eq!(Some(b'r'), bytes.next());
    /// assert_eq!(Some(b's'), bytes.next());
    ///
    /// assert_eq!(None, bytes.next());
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn bytes(&self) -> Bytes<'_> {
        Bytes(self.as_bytes().iter().copied())
    }

    /// 按空白字符拆分字符串切片。
    ///
    /// 返回的迭代器会产出原字符串切片的子切片，子切片之间由任意数量的空白字符分隔。
    ///
    /// “Whitespace” 根据 Unicode Derived Core Property `White_Space` 定义。
    /// 如果只想按 ASCII 空白拆分，请改用 [`split_ascii_whitespace`]。
    ///
    /// [`split_ascii_whitespace`]: str::split_ascii_whitespace
    ///
    /// # 示例
    ///
    /// 基本用法：
    ///
    /// ```
    /// let mut iter = "A few words".split_whitespace();
    ///
    /// assert_eq!(Some("A"), iter.next());
    /// assert_eq!(Some("few"), iter.next());
    /// assert_eq!(Some("words"), iter.next());
    ///
    /// assert_eq!(None, iter.next());
    /// ```
    ///
    /// 会考虑各种空白字符：
    ///
    /// ```
    /// let mut iter = " Mary   had\ta\u{2009}little  \n\t lamb".split_whitespace();
    /// assert_eq!(Some("Mary"), iter.next());
    /// assert_eq!(Some("had"), iter.next());
    /// assert_eq!(Some("a"), iter.next());
    /// assert_eq!(Some("little"), iter.next());
    /// assert_eq!(Some("lamb"), iter.next());
    ///
    /// assert_eq!(None, iter.next());
    /// ```
    ///
    /// 如果字符串为空或全部为空白字符，迭代器不会产出任何字符串切片：
    /// ```
    /// assert_eq!("".split_whitespace().next(), None);
    /// assert_eq!("   ".split_whitespace().next(), None);
    /// ```
    #[must_use = "this returns the split string as an iterator, \
                  without modifying the original"]
    #[stable(feature = "split_whitespace", since = "1.1.0")]
    #[rustc_diagnostic_item = "str_split_whitespace"]
    #[inline]
    pub fn split_whitespace(&self) -> SplitWhitespace<'_> {
        SplitWhitespace { inner: self.split(IsWhitespace).filter(IsNotEmpty) }
    }

    /// 按 ASCII 空白字符拆分字符串切片。
    ///
    /// 返回的迭代器会产出原字符串切片的子切片，子切片之间由任意数量的 ASCII 空白字符分隔。
    ///
    /// 它使用与 [`char::is_ascii_whitespace`] 相同的定义。
    /// 如果要按 Unicode `Whitespace` 拆分，请改用 [`split_whitespace`]。
    ///
    /// [`split_whitespace`]: str::split_whitespace
    ///
    /// # 示例
    ///
    /// 基本用法：
    ///
    /// ```
    /// let mut iter = "A few words".split_ascii_whitespace();
    ///
    /// assert_eq!(Some("A"), iter.next());
    /// assert_eq!(Some("few"), iter.next());
    /// assert_eq!(Some("words"), iter.next());
    ///
    /// assert_eq!(None, iter.next());
    /// ```
    ///
    /// 会考虑多种 ASCII 空白字符（见 [`char::is_ascii_whitespace`]）：
    ///
    /// ```
    /// let mut iter = " Mary   had\ta little  \n\t lamb".split_ascii_whitespace();
    /// assert_eq!(Some("Mary"), iter.next());
    /// assert_eq!(Some("had"), iter.next());
    /// assert_eq!(Some("a"), iter.next());
    /// assert_eq!(Some("little"), iter.next());
    /// assert_eq!(Some("lamb"), iter.next());
    ///
    /// assert_eq!(None, iter.next());
    /// ```
    ///
    /// 如果字符串为空或全部为 ASCII 空白字符，迭代器不会产出任何字符串切片：
    /// ```
    /// assert_eq!("".split_ascii_whitespace().next(), None);
    /// assert_eq!("   ".split_ascii_whitespace().next(), None);
    /// ```
    #[must_use = "this returns the split string as an iterator, \
                  without modifying the original"]
    #[stable(feature = "split_ascii_whitespace", since = "1.34.0")]
    #[inline]
    pub fn split_ascii_whitespace(&self) -> SplitAsciiWhitespace<'_> {
        let inner =
            self.as_bytes().split(IsAsciiWhitespace).filter(BytesIsNotEmpty).map(UnsafeBytesToStr);
        SplitAsciiWhitespace { inner }
    }

    /// 返回按行遍历字符串的迭代器，产出字符串切片。
    ///
    /// 行会在换行符（`\n`）或回车后紧跟换行（`\r\n`）的序列处分隔。
    ///
    /// 迭代器返回的行中不包含行终止符。
    ///
    /// 注意，任何没有紧跟换行符（`\n`）的回车符（`\r`）都不会分隔行。
    /// 因此这些回车符会包含在产出的行中。
    ///
    /// 最后一行的行终止符是可选的。以最终行终止符结尾的字符串，会返回与去掉该最终行
    /// 终止符后相同的行。
    ///
    /// 空字符串返回空迭代器。
    ///
    /// # 示例
    ///
    /// 基本用法：
    ///
    /// ```
    /// let text = "foo\r\nbar\n\nbaz\r";
    /// let mut lines = text.lines();
    ///
    /// assert_eq!(Some("foo"), lines.next());
    /// assert_eq!(Some("bar"), lines.next());
    /// assert_eq!(Some(""), lines.next());
    /// // 末尾回车会包含在最后一行中
    /// assert_eq!(Some("baz\r"), lines.next());
    ///
    /// assert_eq!(None, lines.next());
    /// ```
    ///
    /// 最后一行不需要任何行终止符：
    ///
    /// ```
    /// let text = "foo\nbar\n\r\nbaz";
    /// let mut lines = text.lines();
    ///
    /// assert_eq!(Some("foo"), lines.next());
    /// assert_eq!(Some("bar"), lines.next());
    /// assert_eq!(Some(""), lines.next());
    /// assert_eq!(Some("baz"), lines.next());
    ///
    /// assert_eq!(None, lines.next());
    /// ```
    ///
    /// 空字符串返回空迭代器：
    ///
    /// ```
    /// let text = "";
    /// let mut lines = text.lines();
    ///
    /// assert_eq!(lines.next(), None);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn lines(&self) -> Lines<'_> {
        Lines(self.split_inclusive('\n').map(LinesMap))
    }

    /// 返回按行遍历字符串的迭代器。
    #[stable(feature = "rust1", since = "1.0.0")]
    #[deprecated(since = "1.4.0", note = "use lines() instead now", suggestion = "lines")]
    #[inline]
    #[allow(deprecated)]
    pub fn lines_any(&self) -> LinesAny<'_> {
        LinesAny(self.lines())
    }

    /// 返回遍历字符串的 `u16` 迭代器，内容按本机字节序编码为 UTF-16
    ///（不带字节序标记）。
    ///
    /// # 示例
    ///
    /// ```
    /// let text = "Zażółć gęślą jaźń";
    ///
    /// let utf8_len = text.len();
    /// let utf16_len = text.encode_utf16().count();
    ///
    /// assert!(utf16_len <= utf8_len);
    /// ```
    #[must_use = "this returns the encoded string as an iterator, \
                  without modifying the original"]
    #[stable(feature = "encode_utf16", since = "1.8.0")]
    pub fn encode_utf16(&self) -> EncodeUtf16<'_> {
        EncodeUtf16 { chars: self.chars(), extra: 0 }
    }

    /// 如果给定模式匹配该字符串切片的某个子切片，则返回 `true`。
    ///
    /// 如果不匹配，则返回 `false`。
    ///
    /// [pattern] 可以是 `&str`、[`char`]、[`char`] 切片，或者用于判断字符是否匹配的
    /// 函数或闭包。
    ///
    /// [`char`]: prim@char
    /// [pattern]: self::pattern
    ///
    /// # 示例
    ///
    /// ```
    /// let bananas = "bananas";
    ///
    /// assert!(bananas.contains("nana"));
    /// assert!(!bananas.contains("apples"));
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn contains<P: Pattern>(&self, pat: P) -> bool {
        pat.is_contained_in(self)
    }

    /// 如果给定模式匹配该字符串切片的前缀，则返回 `true`。
    ///
    /// 如果不匹配，则返回 `false`。
    ///
    /// [pattern] 可以是 `&str`；此时如果该 `&str` 是当前字符串切片的前缀，
    /// 本函数返回 true。
    ///
    /// [pattern] 也可以是 [`char`]、[`char`] 切片，或者用于判断字符是否匹配的函数或闭包。
    /// 这些模式只会与该字符串切片的第一个字符比较。关于 [`char`] 切片的行为，
    /// 见下面第二个示例。
    ///
    /// [`char`]: prim@char
    /// [pattern]: self::pattern
    ///
    /// # 示例
    ///
    /// ```
    /// let bananas = "bananas";
    ///
    /// assert!(bananas.starts_with("bana"));
    /// assert!(!bananas.starts_with("nana"));
    /// ```
    ///
    /// ```
    /// let bananas = "bananas";
    ///
    /// // 注意，这两个断言都会成功。
    /// assert!(bananas.starts_with(&['b', 'a', 'n', 'a']));
    /// assert!(bananas.starts_with(&['a', 'b', 'c', 'd']));
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_diagnostic_item = "str_starts_with"]
    pub fn starts_with<P: Pattern>(&self, pat: P) -> bool {
        pat.is_prefix_of(self)
    }

    /// 如果给定模式匹配该字符串切片的后缀，则返回 `true`。
    ///
    /// 如果不匹配，则返回 `false`。
    ///
    /// [pattern] 可以是 `&str`、[`char`]、[`char`] 切片，或者用于判断字符是否匹配的
    /// 函数或闭包。
    ///
    /// [`char`]: prim@char
    /// [pattern]: self::pattern
    ///
    /// # 示例
    ///
    /// ```
    /// let bananas = "bananas";
    ///
    /// assert!(bananas.ends_with("anas"));
    /// assert!(!bananas.ends_with("nana"));
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_diagnostic_item = "str_ends_with"]
    pub fn ends_with<P: Pattern>(&self, pat: P) -> bool
    where
        for<'a> P::Searcher<'a>: ReverseSearcher<'a>,
    {
        pat.is_suffix_of(self)
    }

    /// 返回该字符串切片中第一个匹配模式的字符的字节索引。
    ///
    /// 如果模式不匹配，返回 [`None`]。
    ///
    /// [pattern] 可以是 `&str`、[`char`]、[`char`] 切片，或者用于判断字符是否匹配的
    /// 函数或闭包。
    ///
    /// [`char`]: prim@char
    /// [pattern]: self::pattern
    ///
    /// # 示例
    ///
    /// 简单模式：
    ///
    /// ```
    /// let s = "Löwe 老虎 Léopard Gepardi";
    ///
    /// assert_eq!(s.find('L'), Some(0));
    /// assert_eq!(s.find('é'), Some(14));
    /// assert_eq!(s.find("pard"), Some(17));
    /// ```
    ///
    /// 使用 point-free 风格和闭包的更复杂模式：
    ///
    /// ```
    /// let s = "Löwe 老虎 Léopard";
    ///
    /// assert_eq!(s.find(char::is_whitespace), Some(5));
    /// assert_eq!(s.find(char::is_lowercase), Some(1));
    /// assert_eq!(s.find(|c: char| c.is_whitespace() || c.is_lowercase()), Some(1));
    /// assert_eq!(s.find(|c: char| (c < 'o') && (c > 'a')), Some(4));
    /// ```
    ///
    /// 找不到模式：
    ///
    /// ```
    /// let s = "Löwe 老虎 Léopard";
    /// let x: &[_] = &['1', '2'];
    ///
    /// assert_eq!(s.find(x), None);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn find<P: Pattern>(&self, pat: P) -> Option<usize> {
        pat.into_searcher(self).next_match().map(|(i, _)| i)
    }

    /// 返回该字符串切片中最后一个模式匹配的首字符字节索引。
    ///
    /// 如果模式不匹配，返回 [`None`]。
    ///
    /// [pattern] 可以是 `&str`、[`char`]、[`char`] 切片，或者用于判断字符是否匹配的
    /// 函数或闭包。
    ///
    /// [`char`]: prim@char
    /// [pattern]: self::pattern
    ///
    /// # 示例
    ///
    /// 简单模式：
    ///
    /// ```
    /// let s = "Löwe 老虎 Léopard Gepardi";
    ///
    /// assert_eq!(s.rfind('L'), Some(13));
    /// assert_eq!(s.rfind('é'), Some(14));
    /// assert_eq!(s.rfind("pard"), Some(24));
    /// ```
    ///
    /// 使用闭包的更复杂模式：
    ///
    /// ```
    /// let s = "Löwe 老虎 Léopard";
    ///
    /// assert_eq!(s.rfind(char::is_whitespace), Some(12));
    /// assert_eq!(s.rfind(char::is_lowercase), Some(20));
    /// ```
    ///
    /// 找不到模式：
    ///
    /// ```
    /// let s = "Löwe 老虎 Léopard";
    /// let x: &[_] = &['1', '2'];
    ///
    /// assert_eq!(s.rfind(x), None);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn rfind<P: Pattern>(&self, pat: P) -> Option<usize>
    where
        for<'a> P::Searcher<'a>: ReverseSearcher<'a>,
    {
        pat.into_searcher(self).next_match_back().map(|(i, _)| i)
    }

    /// 返回遍历该字符串切片子串的迭代器，子串由匹配模式的字符分隔。
    ///
    /// [pattern] 可以是 `&str`、[`char`]、[`char`] 切片，或者用于判断字符是否匹配的
    /// 函数或闭包。
    ///
    /// 如果没有任何匹配，整个字符串切片会作为迭代器唯一的项返回。
    ///
    /// [`char`]: prim@char
    /// [pattern]: self::pattern
    ///
    /// # 迭代器行为
    ///
    /// 如果模式允许反向搜索，并且正向/反向搜索会产生相同元素，则返回的迭代器实现
    /// [`DoubleEndedIterator`]。例如 [`char`] 满足这一点，但 `&str` 不满足。
    ///
    /// 如果模式允许反向搜索，但结果可能与正向搜索不同，可以使用 [`rsplit`] 方法。
    ///
    /// [`rsplit`]: str::rsplit
    ///
    /// # 示例
    ///
    /// 简单模式：
    ///
    /// ```
    /// let v: Vec<&str> = "Mary had a little lamb".split(' ').collect();
    /// assert_eq!(v, ["Mary", "had", "a", "little", "lamb"]);
    ///
    /// let v: Vec<&str> = "".split('X').collect();
    /// assert_eq!(v, [""]);
    ///
    /// let v: Vec<&str> = "lionXXtigerXleopard".split('X').collect();
    /// assert_eq!(v, ["lion", "", "tiger", "leopard"]);
    ///
    /// let v: Vec<&str> = "lion::tiger::leopard".split("::").collect();
    /// assert_eq!(v, ["lion", "tiger", "leopard"]);
    ///
    /// let v: Vec<&str> = "AABBCC".split("DD").collect();
    /// assert_eq!(v, ["AABBCC"]);
    ///
    /// let v: Vec<&str> = "abc1def2ghi".split(char::is_numeric).collect();
    /// assert_eq!(v, ["abc", "def", "ghi"]);
    ///
    /// let v: Vec<&str> = "lionXtigerXleopard".split(char::is_uppercase).collect();
    /// assert_eq!(v, ["lion", "tiger", "leopard"]);
    /// ```
    ///
    /// 如果模式是 char 切片，则在其中任一字符出现处拆分：
    ///
    /// ```
    /// let v: Vec<&str> = "2020-11-03 23:59".split(&['-', ' ', ':', '@'][..]).collect();
    /// assert_eq!(v, ["2020", "11", "03", "23", "59"]);
    /// ```
    ///
    /// 使用闭包的更复杂模式：
    ///
    /// ```
    /// let v: Vec<&str> = "abc1defXghi".split(|c| c == '1' || c == 'X').collect();
    /// assert_eq!(v, ["abc", "def", "ghi"]);
    /// ```
    ///
    /// 如果字符串包含多个连续分隔符，输出中会出现空字符串：
    ///
    /// ```
    /// let x = "||||a||b|c".to_string();
    /// let d: Vec<_> = x.split('|').collect();
    ///
    /// assert_eq!(d, &["", "", "", "", "a", "", "b", "c"]);
    /// ```
    ///
    /// 连续分隔符之间由空字符串隔开。
    ///
    /// ```
    /// let x = "(///)".to_string();
    /// let d: Vec<_> = x.split('/').collect();
    ///
    /// assert_eq!(d, &["(", "", "", ")"]);
    /// ```
    ///
    /// 字符串开头或结尾处的分隔符旁边会产生空字符串。
    ///
    /// ```
    /// let d: Vec<_> = "010".split("0").collect();
    /// assert_eq!(d, &["", "1", ""]);
    /// ```
    ///
    /// 当空字符串用作分隔符时，它会分隔字符串中的每个字符，同时也分隔字符串的开头和结尾。
    ///
    /// ```
    /// let f: Vec<_> = "rust".split("").collect();
    /// assert_eq!(f, &["", "r", "u", "s", "t", ""]);
    /// ```
    ///
    /// 当空格用作分隔符时，连续分隔符可能导致看起来意外的行为。下面代码是正确的：
    ///
    /// ```
    /// let x = "    a  b c".to_string();
    /// let d: Vec<_> = x.split(' ').collect();
    ///
    /// assert_eq!(d, &["", "", "", "", "a", "", "b", "c"]);
    /// ```
    ///
    /// 它并不会得到：
    ///
    /// ```,ignore
    /// assert_eq!(d, &["a", "b", "c"]);
    /// ```
    ///
    /// 如果需要这种行为，请使用 [`split_whitespace`]。
    ///
    /// [`split_whitespace`]: str::split_whitespace
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn split<P: Pattern>(&self, pat: P) -> Split<'_, P> {
        Split(SplitInternal {
            start: 0,
            end: self.len(),
            matcher: pat.into_searcher(self),
            allow_trailing_empty: true,
            finished: false,
        })
    }

    /// 返回遍历该字符串切片子串的迭代器，子串由匹配模式的字符分隔。
    ///
    /// 与 `split` 产生的迭代器不同，`split_inclusive` 会把匹配部分保留为子串的终止符。
    ///
    /// [pattern] 可以是 `&str`、[`char`]、[`char`] 切片，或者用于判断字符是否匹配的
    /// 函数或闭包。
    ///
    /// [`char`]: prim@char
    /// [pattern]: self::pattern
    ///
    /// # 示例
    ///
    /// ```
    /// let v: Vec<&str> = "Mary had a little lamb\nlittle lamb\nlittle lamb."
    ///     .split_inclusive('\n').collect();
    /// assert_eq!(v, ["Mary had a little lamb\n", "little lamb\n", "little lamb."]);
    /// ```
    ///
    /// 如果字符串的最后一个元素被匹配，该元素会被视为前一个子串的终止符。
    /// 该子串会成为迭代器返回的最后一项。
    ///
    /// ```
    /// let v: Vec<&str> = "Mary had a little lamb\nlittle lamb\nlittle lamb.\n"
    ///     .split_inclusive('\n').collect();
    /// assert_eq!(v, ["Mary had a little lamb\n", "little lamb\n", "little lamb.\n"]);
    /// ```
    #[stable(feature = "split_inclusive", since = "1.51.0")]
    #[inline]
    pub fn split_inclusive<P: Pattern>(&self, pat: P) -> SplitInclusive<'_, P> {
        SplitInclusive(SplitInternal {
            start: 0,
            end: self.len(),
            matcher: pat.into_searcher(self),
            allow_trailing_empty: false,
            finished: false,
        })
    }

    /// 返回遍历给定字符串切片子串的迭代器，子串由匹配模式的字符分隔，并按反向顺序产出。
    ///
    /// [pattern] 可以是 `&str`、[`char`]、[`char`] 切片，或者用于判断字符是否匹配的
    /// 函数或闭包。
    ///
    /// [`char`]: prim@char
    /// [pattern]: self::pattern
    ///
    /// # 迭代器行为
    ///
    /// 返回的迭代器要求模式支持反向搜索；如果正向/反向搜索会产生相同元素，
    /// 它会实现 [`DoubleEndedIterator`]。
    ///
    /// 如果要从前端迭代，可以使用 [`split`] 方法。
    ///
    /// [`split`]: str::split
    ///
    /// # 示例
    ///
    /// 简单模式：
    ///
    /// ```
    /// let v: Vec<&str> = "Mary had a little lamb".rsplit(' ').collect();
    /// assert_eq!(v, ["lamb", "little", "a", "had", "Mary"]);
    ///
    /// let v: Vec<&str> = "".rsplit('X').collect();
    /// assert_eq!(v, [""]);
    ///
    /// let v: Vec<&str> = "lionXXtigerXleopard".rsplit('X').collect();
    /// assert_eq!(v, ["leopard", "tiger", "", "lion"]);
    ///
    /// let v: Vec<&str> = "lion::tiger::leopard".rsplit("::").collect();
    /// assert_eq!(v, ["leopard", "tiger", "lion"]);
    /// ```
    ///
    /// 使用闭包的更复杂模式：
    ///
    /// ```
    /// let v: Vec<&str> = "abc1defXghi".rsplit(|c| c == '1' || c == 'X').collect();
    /// assert_eq!(v, ["ghi", "def", "abc"]);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn rsplit<P: Pattern>(&self, pat: P) -> RSplit<'_, P>
    where
        for<'a> P::Searcher<'a>: ReverseSearcher<'a>,
    {
        RSplit(self.split(pat).0)
    }

    /// 返回遍历给定字符串切片子串的迭代器，子串由匹配模式的字符分隔。
    ///
    /// [pattern] 可以是 `&str`、[`char`]、[`char`] 切片，或者用于判断字符是否匹配的
    /// 函数或闭包。
    ///
    /// [`char`]: prim@char
    /// [pattern]: self::pattern
    ///
    /// 等价于 [`split`]，但如果尾随子串为空，则跳过它。
    ///
    /// [`split`]: str::split
    ///
    /// 本方法可用于由模式_终止_而不是由模式_分隔_的字符串数据。
    ///
    /// # 迭代器行为
    ///
    /// 如果模式允许反向搜索，并且正向/反向搜索会产生相同元素，则返回的迭代器实现
    /// [`DoubleEndedIterator`]。例如 [`char`] 满足这一点，但 `&str` 不满足。
    ///
    /// 如果模式允许反向搜索，但结果可能与正向搜索不同，可以使用
    /// [`rsplit_terminator`] 方法。
    ///
    /// [`rsplit_terminator`]: str::rsplit_terminator
    ///
    /// # 示例
    ///
    /// ```
    /// let v: Vec<&str> = "A.B.".split_terminator('.').collect();
    /// assert_eq!(v, ["A", "B"]);
    ///
    /// let v: Vec<&str> = "A..B..".split_terminator(".").collect();
    /// assert_eq!(v, ["A", "", "B", ""]);
    ///
    /// let v: Vec<&str> = "A.B:C.D".split_terminator(&['.', ':'][..]).collect();
    /// assert_eq!(v, ["A", "B", "C", "D"]);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn split_terminator<P: Pattern>(&self, pat: P) -> SplitTerminator<'_, P> {
        SplitTerminator(SplitInternal { allow_trailing_empty: false, ..self.split(pat).0 })
    }

    /// 返回遍历 `self` 子串的迭代器，子串由匹配模式的字符分隔，并按反向顺序产出。
    ///
    /// [pattern] 可以是 `&str`、[`char`]、[`char`] 切片，或者用于判断字符是否匹配的
    /// 函数或闭包。
    ///
    /// [`char`]: prim@char
    /// [pattern]: self::pattern
    ///
    /// 等价于 [`split`]，但如果尾随子串为空，则跳过它。
    ///
    /// [`split`]: str::split
    ///
    /// 本方法可用于由模式_终止_而不是由模式_分隔_的字符串数据。
    ///
    /// # 迭代器行为
    ///
    /// 返回的迭代器要求模式支持反向搜索；如果正向/反向搜索会产生相同元素，
    /// 它会是双端迭代器。
    ///
    /// 如果要从前端迭代，可以使用 [`split_terminator`] 方法。
    ///
    /// [`split_terminator`]: str::split_terminator
    ///
    /// # 示例
    ///
    /// ```
    /// let v: Vec<&str> = "A.B.".rsplit_terminator('.').collect();
    /// assert_eq!(v, ["B", "A"]);
    ///
    /// let v: Vec<&str> = "A..B..".rsplit_terminator(".").collect();
    /// assert_eq!(v, ["", "B", "", "A"]);
    ///
    /// let v: Vec<&str> = "A.B:C.D".rsplit_terminator(&['.', ':'][..]).collect();
    /// assert_eq!(v, ["D", "C", "B", "A"]);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn rsplit_terminator<P: Pattern>(&self, pat: P) -> RSplitTerminator<'_, P>
    where
        for<'a> P::Searcher<'a>: ReverseSearcher<'a>,
    {
        RSplitTerminator(self.split_terminator(pat).0)
    }

    /// 返回遍历给定字符串切片子串的迭代器，子串由模式分隔，并限制最多返回 `n` 项。
    ///
    /// 如果返回了 `n` 个子串，则最后一个子串（第 `n` 个子串）会包含字符串剩余部分。
    ///
    /// [pattern] 可以是 `&str`、[`char`]、[`char`] 切片，或者用于判断字符是否匹配的
    /// 函数或闭包。
    ///
    /// [`char`]: prim@char
    /// [pattern]: self::pattern
    ///
    /// # 迭代器行为
    ///
    /// 返回的迭代器不是双端迭代器，因为支持这一点并不高效。
    ///
    /// 如果模式允许反向搜索，可以使用 [`rsplitn`] 方法。
    ///
    /// [`rsplitn`]: str::rsplitn
    ///
    /// # 示例
    ///
    /// 简单模式：
    ///
    /// ```
    /// let v: Vec<&str> = "Mary had a little lambda".splitn(3, ' ').collect();
    /// assert_eq!(v, ["Mary", "had", "a little lambda"]);
    ///
    /// let v: Vec<&str> = "lionXXtigerXleopard".splitn(3, "X").collect();
    /// assert_eq!(v, ["lion", "", "tigerXleopard"]);
    ///
    /// let v: Vec<&str> = "abcXdef".splitn(1, 'X').collect();
    /// assert_eq!(v, ["abcXdef"]);
    ///
    /// let v: Vec<&str> = "".splitn(1, 'X').collect();
    /// assert_eq!(v, [""]);
    /// ```
    ///
    /// 使用闭包的更复杂模式：
    ///
    /// ```
    /// let v: Vec<&str> = "abc1defXghi".splitn(2, |c| c == '1' || c == 'X').collect();
    /// assert_eq!(v, ["abc", "defXghi"]);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn splitn<P: Pattern>(&self, n: usize, pat: P) -> SplitN<'_, P> {
        SplitN(SplitNInternal { iter: self.split(pat).0, count: n })
    }

    /// 返回遍历该字符串切片子串的迭代器，子串由模式分隔，从字符串末尾开始，
    /// 并限制最多返回 `n` 项。
    ///
    /// 如果返回了 `n` 个子串，则最后一个子串（第 `n` 个子串）会包含字符串剩余部分。
    ///
    /// [pattern] 可以是 `&str`、[`char`]、[`char`] 切片，或者用于判断字符是否匹配的
    /// 函数或闭包。
    ///
    /// [`char`]: prim@char
    /// [pattern]: self::pattern
    ///
    /// # 迭代器行为
    ///
    /// 返回的迭代器不是双端迭代器，因为支持这一点并不高效。
    ///
    /// 如果要从前端拆分，可以使用 [`splitn`] 方法。
    ///
    /// [`splitn`]: str::splitn
    ///
    /// # 示例
    ///
    /// 简单模式：
    ///
    /// ```
    /// let v: Vec<&str> = "Mary had a little lamb".rsplitn(3, ' ').collect();
    /// assert_eq!(v, ["lamb", "little", "Mary had a"]);
    ///
    /// let v: Vec<&str> = "lionXXtigerXleopard".rsplitn(3, 'X').collect();
    /// assert_eq!(v, ["leopard", "tiger", "lionX"]);
    ///
    /// let v: Vec<&str> = "lion::tiger::leopard".rsplitn(2, "::").collect();
    /// assert_eq!(v, ["leopard", "lion::tiger"]);
    /// ```
    ///
    /// 使用闭包的更复杂模式：
    ///
    /// ```
    /// let v: Vec<&str> = "abc1defXghi".rsplitn(2, |c| c == '1' || c == 'X').collect();
    /// assert_eq!(v, ["ghi", "abc1def"]);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn rsplitn<P: Pattern>(&self, n: usize, pat: P) -> RSplitN<'_, P>
    where
        for<'a> P::Searcher<'a>: ReverseSearcher<'a>,
    {
        RSplitN(self.splitn(n, pat).0)
    }

    /// 在指定分隔符第一次出现的位置拆分字符串，并返回分隔符之前的前缀和之后的后缀。
    ///
    /// # 示例
    ///
    /// ```
    /// assert_eq!("cfg".split_once('='), None);
    /// assert_eq!("cfg=".split_once('='), Some(("cfg", "")));
    /// assert_eq!("cfg=foo".split_once('='), Some(("cfg", "foo")));
    /// assert_eq!("cfg=foo=bar".split_once('='), Some(("cfg", "foo=bar")));
    /// ```
    #[stable(feature = "str_split_once", since = "1.52.0")]
    #[inline]
    pub fn split_once<P: Pattern>(&self, delimiter: P) -> Option<(&'_ str, &'_ str)> {
        let (start, end) = delimiter.into_searcher(self).next_match()?;
        // SAFETY: `Searcher` 保证返回有效索引。
        unsafe { Some((self.get_unchecked(..start), self.get_unchecked(end..))) }
    }

    /// 在指定分隔符最后一次出现的位置拆分字符串，并返回分隔符之前的前缀和之后的后缀。
    ///
    /// # 示例
    ///
    /// ```
    /// assert_eq!("cfg".rsplit_once('='), None);
    /// assert_eq!("cfg=".rsplit_once('='), Some(("cfg", "")));
    /// assert_eq!("cfg=foo".rsplit_once('='), Some(("cfg", "foo")));
    /// assert_eq!("cfg=foo=bar".rsplit_once('='), Some(("cfg=foo", "bar")));
    /// ```
    #[stable(feature = "str_split_once", since = "1.52.0")]
    #[inline]
    pub fn rsplit_once<P: Pattern>(&self, delimiter: P) -> Option<(&'_ str, &'_ str)>
    where
        for<'a> P::Searcher<'a>: ReverseSearcher<'a>,
    {
        let (start, end) = delimiter.into_searcher(self).next_match_back()?;
        // SAFETY: `Searcher` 保证返回有效索引。
        unsafe { Some((self.get_unchecked(..start), self.get_unchecked(end..))) }
    }

    /// 返回遍历给定字符串切片中各个不重叠模式匹配的迭代器。
    ///
    /// [pattern] 可以是 `&str`、[`char`]、[`char`] 切片，或者用于判断字符是否匹配的
    /// 函数或闭包。
    ///
    /// [`char`]: prim@char
    /// [pattern]: self::pattern
    ///
    /// # 迭代器行为
    ///
    /// 如果模式允许反向搜索，并且正向/反向搜索会产生相同元素，则返回的迭代器实现
    /// [`DoubleEndedIterator`]。例如 [`char`] 满足这一点，但 `&str` 不满足。
    ///
    /// 如果模式允许反向搜索，但结果可能与正向搜索不同，可以使用 [`rmatches`] 方法。
    ///
    /// [`rmatches`]: str::rmatches
    ///
    /// # 示例
    ///
    /// ```
    /// let v: Vec<&str> = "abcXXXabcYYYabc".matches("abc").collect();
    /// assert_eq!(v, ["abc", "abc", "abc"]);
    ///
    /// let v: Vec<&str> = "1abc2abc3".matches(char::is_numeric).collect();
    /// assert_eq!(v, ["1", "2", "3"]);
    /// ```
    #[stable(feature = "str_matches", since = "1.2.0")]
    #[inline]
    pub fn matches<P: Pattern>(&self, pat: P) -> Matches<'_, P> {
        Matches(MatchesInternal(pat.into_searcher(self)))
    }

    /// 返回遍历该字符串切片中各个不重叠模式匹配的迭代器，并按反向顺序产出。
    ///
    /// [pattern] 可以是 `&str`、[`char`]、[`char`] 切片，或者用于判断字符是否匹配的
    /// 函数或闭包。
    ///
    /// [`char`]: prim@char
    /// [pattern]: self::pattern
    ///
    /// # 迭代器行为
    ///
    /// 返回的迭代器要求模式支持反向搜索；如果正向/反向搜索会产生相同元素，
    /// 它会实现 [`DoubleEndedIterator`]。
    ///
    /// 如果要从前端迭代，可以使用 [`matches`] 方法。
    ///
    /// [`matches`]: str::matches
    ///
    /// # 示例
    ///
    /// ```
    /// let v: Vec<&str> = "abcXXXabcYYYabc".rmatches("abc").collect();
    /// assert_eq!(v, ["abc", "abc", "abc"]);
    ///
    /// let v: Vec<&str> = "1abc2abc3".rmatches(char::is_numeric).collect();
    /// assert_eq!(v, ["3", "2", "1"]);
    /// ```
    #[stable(feature = "str_matches", since = "1.2.0")]
    #[inline]
    pub fn rmatches<P: Pattern>(&self, pat: P) -> RMatches<'_, P>
    where
        for<'a> P::Searcher<'a>: ReverseSearcher<'a>,
    {
        RMatches(self.matches(pat).0)
    }

    /// 返回遍历该字符串切片中各个不重叠模式匹配的迭代器，同时产出匹配开始处的索引。
    ///
    /// 对于 `self` 中彼此重叠的 `pat` 匹配，只返回第一个匹配对应的索引。
    ///
    /// [pattern] 可以是 `&str`、[`char`]、[`char`] 切片，或者用于判断字符是否匹配的
    /// 函数或闭包。
    ///
    /// [`char`]: prim@char
    /// [pattern]: self::pattern
    ///
    /// # 迭代器行为
    ///
    /// 如果模式允许反向搜索，并且正向/反向搜索会产生相同元素，则返回的迭代器实现
    /// [`DoubleEndedIterator`]。例如 [`char`] 满足这一点，但 `&str` 不满足。
    ///
    /// 如果模式允许反向搜索，但结果可能与正向搜索不同，可以使用
    /// [`rmatch_indices`] 方法。
    ///
    /// [`rmatch_indices`]: str::rmatch_indices
    ///
    /// # 示例
    ///
    /// ```
    /// let v: Vec<_> = "abcXXXabcYYYabc".match_indices("abc").collect();
    /// assert_eq!(v, [(0, "abc"), (6, "abc"), (12, "abc")]);
    ///
    /// let v: Vec<_> = "1abcabc2".match_indices("abc").collect();
    /// assert_eq!(v, [(1, "abc"), (4, "abc")]);
    ///
    /// let v: Vec<_> = "ababa".match_indices("aba").collect();
    /// assert_eq!(v, [(0, "aba")]); // 只有第一个 `aba`
    /// ```
    #[stable(feature = "str_match_indices", since = "1.5.0")]
    #[inline]
    pub fn match_indices<P: Pattern>(&self, pat: P) -> MatchIndices<'_, P> {
        MatchIndices(MatchIndicesInternal(pat.into_searcher(self)))
    }

    /// 返回遍历 `self` 中各个不重叠模式匹配的迭代器，并按反向顺序连同匹配索引一起产出。
    ///
    /// 对于 `self` 中彼此重叠的 `pat` 匹配，只返回最后一个匹配对应的索引。
    ///
    /// [pattern] 可以是 `&str`、[`char`]、[`char`] 切片，或者用于判断字符是否匹配的
    /// 函数或闭包。
    ///
    /// [`char`]: prim@char
    /// [pattern]: self::pattern
    ///
    /// # 迭代器行为
    ///
    /// 返回的迭代器要求模式支持反向搜索；如果正向/反向搜索会产生相同元素，
    /// 它会实现 [`DoubleEndedIterator`]。
    ///
    /// 如果要从前端迭代，可以使用 [`match_indices`] 方法。
    ///
    /// [`match_indices`]: str::match_indices
    ///
    /// # 示例
    ///
    /// ```
    /// let v: Vec<_> = "abcXXXabcYYYabc".rmatch_indices("abc").collect();
    /// assert_eq!(v, [(12, "abc"), (6, "abc"), (0, "abc")]);
    ///
    /// let v: Vec<_> = "1abcabc2".rmatch_indices("abc").collect();
    /// assert_eq!(v, [(4, "abc"), (1, "abc")]);
    ///
    /// let v: Vec<_> = "ababa".rmatch_indices("aba").collect();
    /// assert_eq!(v, [(2, "aba")]); // 只有最后一个 `aba`
    /// ```
    #[stable(feature = "str_match_indices", since = "1.5.0")]
    #[inline]
    pub fn rmatch_indices<P: Pattern>(&self, pat: P) -> RMatchIndices<'_, P>
    where
        for<'a> P::Searcher<'a>: ReverseSearcher<'a>,
    {
        RMatchIndices(self.match_indices(pat).0)
    }

    /// 返回去除了开头和结尾空白字符的字符串切片。
    ///
    /// “Whitespace” 根据 Unicode Derived Core Property `White_Space` 定义，
    /// 其中包括换行符。
    ///
    /// # 示例
    ///
    /// ```
    /// let s = "\n Hello\tworld\t\n";
    ///
    /// assert_eq!("Hello\tworld", s.trim());
    /// ```
    #[inline]
    #[must_use = "this returns the trimmed string as a slice, \
                  without modifying the original"]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_diagnostic_item = "str_trim"]
    pub fn trim(&self) -> &str {
        self.trim_matches(char::is_whitespace)
    }

    /// 返回去除了开头空白字符的字符串切片。
    ///
    /// “Whitespace” 根据 Unicode Derived Core Property `White_Space` 定义，
    /// 其中包括换行符。
    ///
    /// # 文本方向性
    ///
    /// 字符串是字节序列。此处的 `start` 表示该字节串的第一个位置；对于 English
    /// 或 Russian 这类从左到右的语言，它是左侧；对于 Arabic 或 Hebrew 这类从右到左的语言，
    /// 它是右侧。
    ///
    /// # 示例
    ///
    /// 基本用法：
    ///
    /// ```
    /// let s = "\n Hello\tworld\t\n";
    /// assert_eq!("Hello\tworld\t\n", s.trim_start());
    /// ```
    ///
    /// 方向性：
    ///
    /// ```
    /// let s = "  English  ";
    /// assert!(Some('E') == s.trim_start().chars().next());
    ///
    /// let s = "  עברית  ";
    /// assert!(Some('ע') == s.trim_start().chars().next());
    /// ```
    #[inline]
    #[must_use = "this returns the trimmed string as a new slice, \
                  without modifying the original"]
    #[stable(feature = "trim_direction", since = "1.30.0")]
    #[rustc_diagnostic_item = "str_trim_start"]
    pub fn trim_start(&self) -> &str {
        self.trim_start_matches(char::is_whitespace)
    }

    /// 返回去除了结尾空白字符的字符串切片。
    ///
    /// “Whitespace” 根据 Unicode Derived Core Property `White_Space` 定义，
    /// 其中包括换行符。
    ///
    /// # 文本方向性
    ///
    /// 字符串是字节序列。此处的 `end` 表示该字节串的最后一个位置；对于 English
    /// 或 Russian 这类从左到右的语言，它是右侧；对于 Arabic 或 Hebrew 这类从右到左的语言，
    /// 它是左侧。
    ///
    /// # 示例
    ///
    /// 基本用法：
    ///
    /// ```
    /// let s = "\n Hello\tworld\t\n";
    /// assert_eq!("\n Hello\tworld", s.trim_end());
    /// ```
    ///
    /// 方向性：
    ///
    /// ```
    /// let s = "  English  ";
    /// assert!(Some('h') == s.trim_end().chars().rev().next());
    ///
    /// let s = "  עברית  ";
    /// assert!(Some('ת') == s.trim_end().chars().rev().next());
    /// ```
    #[inline]
    #[must_use = "this returns the trimmed string as a new slice, \
                  without modifying the original"]
    #[stable(feature = "trim_direction", since = "1.30.0")]
    #[rustc_diagnostic_item = "str_trim_end"]
    pub fn trim_end(&self) -> &str {
        self.trim_end_matches(char::is_whitespace)
    }

    /// 返回去除了开头空白字符的字符串切片。
    ///
    /// “Whitespace” 根据 Unicode Derived Core Property `White_Space` 定义。
    ///
    /// # 文本方向性
    ///
    /// 字符串是字节序列。此处的 'Left' 表示该字节串的第一个位置；对于 Arabic 或 Hebrew
    /// 这类从右到左而不是从左到右的语言，它是_右_侧，而不是左侧。
    ///
    /// # 示例
    ///
    /// 基本用法：
    ///
    /// ```
    /// let s = " Hello\tworld\t";
    ///
    /// assert_eq!("Hello\tworld\t", s.trim_left());
    /// ```
    ///
    /// 方向性：
    ///
    /// ```
    /// let s = "  English";
    /// assert!(Some('E') == s.trim_left().chars().next());
    ///
    /// let s = "  עברית";
    /// assert!(Some('ע') == s.trim_left().chars().next());
    /// ```
    #[must_use = "this returns the trimmed string as a new slice, \
                  without modifying the original"]
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[deprecated(since = "1.33.0", note = "superseded by `trim_start`", suggestion = "trim_start")]
    pub fn trim_left(&self) -> &str {
        self.trim_start()
    }

    /// 返回去除了结尾空白字符的字符串切片。
    ///
    /// “Whitespace” 根据 Unicode Derived Core Property `White_Space` 定义。
    ///
    /// # 文本方向性
    ///
    /// 字符串是字节序列。此处的 'Right' 表示该字节串的最后一个位置；对于 Arabic 或 Hebrew
    /// 这类从右到左而不是从左到右的语言，它是_左_侧，而不是右侧。
    ///
    /// # 示例
    ///
    /// 基本用法：
    ///
    /// ```
    /// let s = " Hello\tworld\t";
    ///
    /// assert_eq!(" Hello\tworld", s.trim_right());
    /// ```
    ///
    /// 方向性：
    ///
    /// ```
    /// let s = "English  ";
    /// assert!(Some('h') == s.trim_right().chars().rev().next());
    ///
    /// let s = "עברית  ";
    /// assert!(Some('ת') == s.trim_right().chars().rev().next());
    /// ```
    #[must_use = "this returns the trimmed string as a new slice, \
                  without modifying the original"]
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[deprecated(since = "1.33.0", note = "superseded by `trim_end`", suggestion = "trim_end")]
    pub fn trim_right(&self) -> &str {
        self.trim_end()
    }

    /// 返回反复移除所有匹配模式的前缀和后缀后的字符串切片。
    ///
    /// [pattern] 可以是 [`char`]、[`char`] 切片，或者用于判断字符是否匹配的函数或闭包。
    ///
    /// [`char`]: prim@char
    /// [pattern]: self::pattern
    ///
    /// # 示例
    ///
    /// 简单模式：
    ///
    /// ```
    /// assert_eq!("11foo1bar11".trim_matches('1'), "foo1bar");
    /// assert_eq!("123foo1bar123".trim_matches(char::is_numeric), "foo1bar");
    ///
    /// let x: &[_] = &['1', '2'];
    /// assert_eq!("12foo1bar12".trim_matches(x), "foo1bar");
    /// ```
    ///
    /// 使用闭包的更复杂模式：
    ///
    /// ```
    /// assert_eq!("1foo1barXX".trim_matches(|c| c == '1' || c == 'X'), "foo1bar");
    /// ```
    #[must_use = "this returns the trimmed string as a new slice, \
                  without modifying the original"]
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn trim_matches<P: Pattern>(&self, pat: P) -> &str
    where
        for<'a> P::Searcher<'a>: DoubleEndedSearcher<'a>,
    {
        let mut i = 0;
        let mut j = 0;
        let mut matcher = pat.into_searcher(self);
        if let Some((a, b)) = matcher.next_reject() {
            i = a;
            j = b; // 记住最早已知匹配；如果最后一个匹配不同，会在下面修正。
        }
        if let Some((_, b)) = matcher.next_reject_back() {
            j = b;
        }
        // SAFETY: `Searcher` 保证返回有效索引。
        unsafe { self.get_unchecked(i..j) }
    }

    /// 返回反复移除所有匹配模式的前缀后的字符串切片。
    ///
    /// [pattern] 可以是 `&str`、[`char`]、[`char`] 切片，或者用于判断字符是否匹配的
    /// 函数或闭包。
    ///
    /// [`char`]: prim@char
    /// [pattern]: self::pattern
    ///
    /// # 文本方向性
    ///
    /// 字符串是字节序列。此处的 `start` 表示该字节串的第一个位置；对于 English
    /// 或 Russian 这类从左到右的语言，它是左侧；对于 Arabic 或 Hebrew 这类从右到左的语言，
    /// 它是右侧。
    ///
    /// # 示例
    ///
    /// ```
    /// assert_eq!("11foo1bar11".trim_start_matches('1'), "foo1bar11");
    /// assert_eq!("123foo1bar123".trim_start_matches(char::is_numeric), "foo1bar123");
    ///
    /// let x: &[_] = &['1', '2'];
    /// assert_eq!("12foo1bar12".trim_start_matches(x), "foo1bar12");
    /// ```
    #[must_use = "this returns the trimmed string as a new slice, \
                  without modifying the original"]
    #[stable(feature = "trim_direction", since = "1.30.0")]
    pub fn trim_start_matches<P: Pattern>(&self, pat: P) -> &str {
        let mut i = self.len();
        let mut matcher = pat.into_searcher(self);
        if let Some((a, _)) = matcher.next_reject() {
            i = a;
        }
        // SAFETY: `Searcher` 保证返回有效索引。
        unsafe { self.get_unchecked(i..self.len()) }
    }

    /// 返回移除了前缀后的字符串切片。
    ///
    /// 如果字符串以模式 `prefix` 开头，返回前缀之后的子串，并包裹在 `Some` 中。
    /// 与 [`trim_start_matches`] 不同，本方法只移除一次前缀。
    ///
    /// 如果字符串不以 `prefix` 开头，则返回 `None`。
    ///
    /// [pattern] 可以是 `&str`、[`char`]、[`char`] 切片，或者用于判断字符是否匹配的
    /// 函数或闭包。
    ///
    /// [`char`]: prim@char
    /// [pattern]: self::pattern
    /// [`trim_start_matches`]: Self::trim_start_matches
    ///
    /// # 示例
    ///
    /// ```
    /// assert_eq!("foo:bar".strip_prefix("foo:"), Some("bar"));
    /// assert_eq!("foo:bar".strip_prefix("bar"), None);
    /// assert_eq!("foofoo".strip_prefix("foo"), Some("foo"));
    /// ```
    #[must_use = "this returns the remaining substring as a new slice, \
                  without modifying the original"]
    #[stable(feature = "str_strip", since = "1.45.0")]
    pub fn strip_prefix<P: Pattern>(&self, prefix: P) -> Option<&str> {
        prefix.strip_prefix_of(self)
    }

    /// 返回移除了后缀后的字符串切片。
    ///
    /// 如果字符串以模式 `suffix` 结尾，返回后缀之前的子串，并包裹在 `Some` 中。
    /// 与 [`trim_end_matches`] 不同，本方法只移除一次后缀。
    ///
    /// 如果字符串不以 `suffix` 结尾，则返回 `None`。
    ///
    /// [pattern] 可以是 `&str`、[`char`]、[`char`] 切片，或者用于判断字符是否匹配的
    /// 函数或闭包。
    ///
    /// [`char`]: prim@char
    /// [pattern]: self::pattern
    /// [`trim_end_matches`]: Self::trim_end_matches
    ///
    /// # 示例
    ///
    /// ```
    /// assert_eq!("bar:foo".strip_suffix(":foo"), Some("bar"));
    /// assert_eq!("bar:foo".strip_suffix("bar"), None);
    /// assert_eq!("foofoo".strip_suffix("foo"), Some("foo"));
    /// ```
    #[must_use = "this returns the remaining substring as a new slice, \
                  without modifying the original"]
    #[stable(feature = "str_strip", since = "1.45.0")]
    pub fn strip_suffix<P: Pattern>(&self, suffix: P) -> Option<&str>
    where
        for<'a> P::Searcher<'a>: ReverseSearcher<'a>,
    {
        suffix.strip_suffix_of(self)
    }

    /// 返回同时移除了前缀和后缀后的字符串切片。
    ///
    /// 如果字符串以模式 `prefix` 开头且以模式 `suffix` 结尾，返回前缀之后、后缀之前的子串，
    /// 并包裹在 `Some` 中。与 [`trim_start_matches`] 和 [`trim_end_matches`] 不同，
    /// 本方法对前缀和后缀都只移除一次。
    ///
    /// 如果字符串不以 `prefix` 开头，或不以 `suffix` 结尾，则返回 `None`。
    ///
    /// 每个 [pattern] 都可以是 `&str`、[`char`]、[`char`] 切片，或者用于判断字符是否匹配的
    /// 函数或闭包。
    ///
    /// [`char`]: prim@char
    /// [pattern]: self::pattern
    /// [`trim_start_matches`]: Self::trim_start_matches
    /// [`trim_end_matches`]: Self::trim_end_matches
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(strip_circumfix)]
    ///
    /// assert_eq!("bar:hello:foo".strip_circumfix("bar:", ":foo"), Some("hello"));
    /// assert_eq!("bar:foo".strip_circumfix("foo", "foo"), None);
    /// assert_eq!("foo:bar;".strip_circumfix("foo:", ';'), Some("bar"));
    /// ```
    #[must_use = "this returns the remaining substring as a new slice, \
                  without modifying the original"]
    #[unstable(feature = "strip_circumfix", issue = "147946")]
    pub fn strip_circumfix<P: Pattern, S: Pattern>(&self, prefix: P, suffix: S) -> Option<&str>
    where
        for<'a> S::Searcher<'a>: ReverseSearcher<'a>,
    {
        self.strip_prefix(prefix)?.strip_suffix(suffix)
    }

    /// 返回移除了可选前缀后的字符串切片。
    ///
    /// 如果字符串以模式 `prefix` 开头，返回前缀之后的子串。
    /// 与 [`strip_prefix`] 不同，本方法总是返回 `&str`，便于链式调用，
    /// 而不是返回 [`Option<&str>`]。
    ///
    /// 如果字符串不以 `prefix` 开头，则原样返回原字符串。
    ///
    /// [pattern] 可以是 `&str`、[`char`]、[`char`] 切片，或者用于判断字符是否匹配的
    /// 函数或闭包。
    ///
    /// [`char`]: prim@char
    /// [pattern]: self::pattern
    /// [`strip_prefix`]: Self::strip_prefix
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(trim_prefix_suffix)]
    ///
    /// // 前缀存在，将其移除
    /// assert_eq!("foo:bar".trim_prefix("foo:"), "bar");
    /// assert_eq!("foofoo".trim_prefix("foo"), "foo");
    ///
    /// // 前缀不存在，返回原字符串
    /// assert_eq!("foo:bar".trim_prefix("bar"), "foo:bar");
    ///
    /// // 方法链示例
    /// assert_eq!("<https://example.com/>".trim_prefix('<').trim_suffix('>'), "https://example.com/");
    /// ```
    #[must_use = "this returns the remaining substring as a new slice, \
                  without modifying the original"]
    #[unstable(feature = "trim_prefix_suffix", issue = "142312")]
    pub fn trim_prefix<P: Pattern>(&self, prefix: P) -> &str {
        prefix.strip_prefix_of(self).unwrap_or(self)
    }

    /// 返回移除了可选后缀后的字符串切片。
    ///
    /// 如果字符串以模式 `suffix` 结尾，返回后缀之前的子串。
    /// 与 [`strip_suffix`] 不同，本方法总是返回 `&str`，便于链式调用，
    /// 而不是返回 [`Option<&str>`]。
    ///
    /// 如果字符串不以 `suffix` 结尾，则原样返回原字符串。
    ///
    /// [pattern] 可以是 `&str`、[`char`]、[`char`] 切片，或者用于判断字符是否匹配的
    /// 函数或闭包。
    ///
    /// [`char`]: prim@char
    /// [pattern]: self::pattern
    /// [`strip_suffix`]: Self::strip_suffix
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(trim_prefix_suffix)]
    ///
    /// // 后缀存在，将其移除
    /// assert_eq!("bar:foo".trim_suffix(":foo"), "bar");
    /// assert_eq!("foofoo".trim_suffix("foo"), "foo");
    ///
    /// // 后缀不存在，返回原字符串
    /// assert_eq!("bar:foo".trim_suffix("bar"), "bar:foo");
    ///
    /// // 方法链示例
    /// assert_eq!("<https://example.com/>".trim_prefix('<').trim_suffix('>'), "https://example.com/");
    /// ```
    #[must_use = "this returns the remaining substring as a new slice, \
                  without modifying the original"]
    #[unstable(feature = "trim_prefix_suffix", issue = "142312")]
    pub fn trim_suffix<P: Pattern>(&self, suffix: P) -> &str
    where
        for<'a> P::Searcher<'a>: ReverseSearcher<'a>,
    {
        suffix.strip_suffix_of(self).unwrap_or(self)
    }

    /// 返回反复移除所有匹配模式的后缀后的字符串切片。
    ///
    /// [pattern] 可以是 `&str`、[`char`]、[`char`] 切片，或者用于判断字符是否匹配的
    /// 函数或闭包。
    ///
    /// [`char`]: prim@char
    /// [pattern]: self::pattern
    ///
    /// # 文本方向性
    ///
    /// 字符串是字节序列。此处的 `end` 表示该字节串的最后一个位置；对于 English
    /// 或 Russian 这类从左到右的语言，它是右侧；对于 Arabic 或 Hebrew 这类从右到左的语言，
    /// 它是左侧。
    ///
    /// # 示例
    ///
    /// 简单模式：
    ///
    /// ```
    /// assert_eq!("11foo1bar11".trim_end_matches('1'), "11foo1bar");
    /// assert_eq!("123foo1bar123".trim_end_matches(char::is_numeric), "123foo1bar");
    ///
    /// let x: &[_] = &['1', '2'];
    /// assert_eq!("12foo1bar12".trim_end_matches(x), "12foo1bar");
    /// ```
    ///
    /// 使用闭包的更复杂模式：
    ///
    /// ```
    /// assert_eq!("1fooX".trim_end_matches(|c| c == '1' || c == 'X'), "1foo");
    /// ```
    #[must_use = "this returns the trimmed string as a new slice, \
                  without modifying the original"]
    #[stable(feature = "trim_direction", since = "1.30.0")]
    pub fn trim_end_matches<P: Pattern>(&self, pat: P) -> &str
    where
        for<'a> P::Searcher<'a>: ReverseSearcher<'a>,
    {
        let mut j = 0;
        let mut matcher = pat.into_searcher(self);
        if let Some((_, b)) = matcher.next_reject_back() {
            j = b;
        }
        // SAFETY: `Searcher` 保证返回有效索引。
        unsafe { self.get_unchecked(0..j) }
    }

    /// 返回反复移除所有匹配模式的前缀后的字符串切片。
    ///
    /// [pattern] 可以是 `&str`、[`char`]、[`char`] 切片，或者用于判断字符是否匹配的
    /// 函数或闭包。
    ///
    /// [`char`]: prim@char
    /// [pattern]: self::pattern
    ///
    /// # 文本方向性
    ///
    /// 字符串是字节序列。此处的 'Left' 表示该字节串的第一个位置；对于 Arabic 或 Hebrew
    /// 这类从右到左而不是从左到右的语言，它是_右_侧，而不是左侧。
    ///
    /// # 示例
    ///
    /// ```
    /// assert_eq!("11foo1bar11".trim_left_matches('1'), "foo1bar11");
    /// assert_eq!("123foo1bar123".trim_left_matches(char::is_numeric), "foo1bar123");
    ///
    /// let x: &[_] = &['1', '2'];
    /// assert_eq!("12foo1bar12".trim_left_matches(x), "foo1bar12");
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[deprecated(
        since = "1.33.0",
        note = "superseded by `trim_start_matches`",
        suggestion = "trim_start_matches"
    )]
    pub fn trim_left_matches<P: Pattern>(&self, pat: P) -> &str {
        self.trim_start_matches(pat)
    }

    /// 返回反复移除所有匹配模式的后缀后的字符串切片。
    ///
    /// [pattern] 可以是 `&str`、[`char`]、[`char`] 切片，或者用于判断字符是否匹配的
    /// 函数或闭包。
    ///
    /// [`char`]: prim@char
    /// [pattern]: self::pattern
    ///
    /// # 文本方向性
    ///
    /// 字符串是字节序列。此处的 'Right' 表示该字节串的最后一个位置；对于 Arabic 或 Hebrew
    /// 这类从右到左而不是从左到右的语言，它是_左_侧，而不是右侧。
    ///
    /// # 示例
    ///
    /// 简单模式：
    ///
    /// ```
    /// assert_eq!("11foo1bar11".trim_right_matches('1'), "11foo1bar");
    /// assert_eq!("123foo1bar123".trim_right_matches(char::is_numeric), "123foo1bar");
    ///
    /// let x: &[_] = &['1', '2'];
    /// assert_eq!("12foo1bar12".trim_right_matches(x), "12foo1bar");
    /// ```
    ///
    /// 使用闭包的更复杂模式：
    ///
    /// ```
    /// assert_eq!("1fooX".trim_right_matches(|c| c == '1' || c == 'X'), "1foo");
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[deprecated(
        since = "1.33.0",
        note = "superseded by `trim_end_matches`",
        suggestion = "trim_end_matches"
    )]
    pub fn trim_right_matches<P: Pattern>(&self, pat: P) -> &str
    where
        for<'a> P::Searcher<'a>: ReverseSearcher<'a>,
    {
        self.trim_end_matches(pat)
    }

    /// 将该字符串切片解析为另一种类型。
    ///
    /// 由于 `parse` 非常通用，它可能给类型推断带来问题。因此，`parse` 是少数你会看到
    /// 被称为 “turbofish” 的 `::<>` 语法的场景之一。该语法帮助推断算法明确你想解析成的
    /// 具体类型。
    ///
    /// `parse` 可以解析成任何实现 [`FromStr`] trait 的类型。
    ///
    /// # 错误
    ///
    /// 如果无法把该字符串切片解析为目标类型，则返回 [`Err`]。
    ///
    /// [`Err`]: FromStr::Err
    ///
    /// # 示例
    ///
    /// 基本用法：
    ///
    /// ```
    /// let four: u32 = "4".parse().unwrap();
    ///
    /// assert_eq!(4, four);
    /// ```
    ///
    /// 使用 “turbofish” 而不是给 `four` 添加类型标注：
    ///
    /// ```
    /// let four = "4".parse::<u32>();
    ///
    /// assert_eq!(Ok(4), four);
    /// ```
    ///
    /// 解析失败：
    ///
    /// ```
    /// let nope = "j".parse::<u32>();
    ///
    /// assert!(nope.is_err());
    /// ```
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn parse<F: FromStr>(&self) -> Result<F, F::Err> {
        FromStr::from_str(self)
    }

    /// 检查该字符串中的所有字符是否都在 ASCII 范围内。
    ///
    /// 空字符串返回 `true`。
    ///
    /// # 示例
    ///
    /// ```
    /// let ascii = "hello!\n";
    /// let non_ascii = "Grüße, Jürgen ❤";
    ///
    /// assert!(ascii.is_ascii());
    /// assert!(!non_ascii.is_ascii());
    /// ```
    #[stable(feature = "ascii_methods_on_intrinsics", since = "1.23.0")]
    #[rustc_const_stable(feature = "const_slice_is_ascii", since = "1.74.0")]
    #[must_use]
    #[inline]
    pub const fn is_ascii(&self) -> bool {
        // 这里可以把每个字节视为字符：所有多字节字符都以非 ASCII 范围内的字节开头，
        // 因此会在该处停止。
        self.as_bytes().is_ascii()
    }

    /// 如果该字符串切片满足 [`is_ascii`](Self::is_ascii)，则将其作为
    /// [ASCII 字符](`ascii::Char`)切片返回；否则返回 `None`。
    #[unstable(feature = "ascii_char", issue = "110998")]
    #[must_use]
    #[inline]
    pub const fn as_ascii(&self) -> Option<&[ascii::Char]> {
        // 与 `is_ascii` 一样，这里可以直接处理字节。
        self.as_bytes().as_ascii()
    }

    /// 将该字符串切片转换为 [ASCII 字符](ascii::Char)切片，不检查它们是否有效。
    ///
    /// # 安全性(Safety）
    ///
    /// 该字符串中的每个字符都必须是 ASCII，否则会触发 UB。
    #[unstable(feature = "ascii_char", issue = "110998")]
    #[must_use]
    #[inline]
    pub const unsafe fn as_ascii_unchecked(&self) -> &[ascii::Char] {
        assert_unsafe_precondition!(
            check_library_ub,
            "as_ascii_unchecked requires that the string is valid ASCII",
            (it: &str = self) => it.is_ascii()
        );

        // SAFETY: 调用方承诺该字符串切片中的每个字节都是 ASCII。
        unsafe { self.as_bytes().as_ascii_unchecked() }
    }

    /// 检查两个字符串在 ASCII 范围内忽略大小写后是否匹配。
    ///
    /// 等价于 `to_ascii_lowercase(a) == to_ascii_lowercase(b)`，
    /// 但不会分配和复制临时值。
    ///
    /// # 示例
    ///
    /// ```
    /// assert!("Ferris".eq_ignore_ascii_case("FERRIS"));
    /// assert!("Ferrös".eq_ignore_ascii_case("FERRöS"));
    /// assert!(!"Ferrös".eq_ignore_ascii_case("FERRÖS"));
    /// ```
    #[stable(feature = "ascii_methods_on_intrinsics", since = "1.23.0")]
    #[rustc_const_stable(feature = "const_eq_ignore_ascii_case", since = "1.89.0")]
    #[must_use]
    #[inline]
    pub const fn eq_ignore_ascii_case(&self, other: &str) -> bool {
        self.as_bytes().eq_ignore_ascii_case(other.as_bytes())
    }

    /// 原地将该字符串转换为对应的 ASCII 大写形式。
    ///
    /// ASCII 字母 'a' 到 'z' 会映射为 'A' 到 'Z'，但非 ASCII 字母保持不变。
    ///
    /// 如果想返回新的大写值而不修改现有值，请使用 [`to_ascii_uppercase()`]。
    ///
    /// [`to_ascii_uppercase()`]: #method.to_ascii_uppercase
    ///
    /// # 示例
    ///
    /// ```
    /// let mut s = String::from("Grüße, Jürgen ❤");
    ///
    /// s.make_ascii_uppercase();
    ///
    /// assert_eq!("GRüßE, JüRGEN ❤", s);
    /// ```
    #[stable(feature = "ascii_methods_on_intrinsics", since = "1.23.0")]
    #[rustc_const_stable(feature = "const_make_ascii", since = "1.84.0")]
    #[inline]
    pub const fn make_ascii_uppercase(&mut self) {
        // SAFETY: 只修改 ASCII 字母不会破坏 UTF-8 有效性。
        let me = unsafe { self.as_bytes_mut() };
        me.make_ascii_uppercase()
    }

    /// 原地将该字符串转换为对应的 ASCII 小写形式。
    ///
    /// ASCII 字母 'A' 到 'Z' 会映射为 'a' 到 'z'，但非 ASCII 字母保持不变。
    ///
    /// 如果想返回新的小写值而不修改现有值，请使用 [`to_ascii_lowercase()`]。
    ///
    /// [`to_ascii_lowercase()`]: #method.to_ascii_lowercase
    ///
    /// # 示例
    ///
    /// ```
    /// let mut s = String::from("GRÜßE, JÜRGEN ❤");
    ///
    /// s.make_ascii_lowercase();
    ///
    /// assert_eq!("grÜße, jÜrgen ❤", s);
    /// ```
    #[stable(feature = "ascii_methods_on_intrinsics", since = "1.23.0")]
    #[rustc_const_stable(feature = "const_make_ascii", since = "1.84.0")]
    #[inline]
    pub const fn make_ascii_lowercase(&mut self) {
        // SAFETY: 只修改 ASCII 字母不会破坏 UTF-8 有效性。
        let me = unsafe { self.as_bytes_mut() };
        me.make_ascii_lowercase()
    }

    /// 返回去除了开头 ASCII 空白字符的字符串切片。
    ///
    /// “Whitespace” 指 [`u8::is_ascii_whitespace`] 使用的定义。
    ///
    /// [`u8::is_ascii_whitespace`]: u8::is_ascii_whitespace
    ///
    /// # 示例
    ///
    /// ```
    /// assert_eq!(" \t \u{3000}hello world\n".trim_ascii_start(), "\u{3000}hello world\n");
    /// assert_eq!("  ".trim_ascii_start(), "");
    /// assert_eq!("".trim_ascii_start(), "");
    /// ```
    #[must_use = "this returns the trimmed string as a new slice, \
                  without modifying the original"]
    #[stable(feature = "byte_slice_trim_ascii", since = "1.80.0")]
    #[rustc_const_stable(feature = "byte_slice_trim_ascii", since = "1.80.0")]
    #[inline]
    pub const fn trim_ascii_start(&self) -> &str {
        // SAFETY: 从 `&str` 中移除 ASCII 字符不会破坏 UTF-8 有效性。
        unsafe { core::str::from_utf8_unchecked(self.as_bytes().trim_ascii_start()) }
    }

    /// 返回去除了结尾 ASCII 空白字符的字符串切片。
    ///
    /// “Whitespace” 指 [`u8::is_ascii_whitespace`] 使用的定义。
    ///
    /// [`u8::is_ascii_whitespace`]: u8::is_ascii_whitespace
    ///
    /// # 示例
    ///
    /// ```
    /// assert_eq!("\r hello world\u{3000}\n ".trim_ascii_end(), "\r hello world\u{3000}");
    /// assert_eq!("  ".trim_ascii_end(), "");
    /// assert_eq!("".trim_ascii_end(), "");
    /// ```
    #[must_use = "this returns the trimmed string as a new slice, \
                  without modifying the original"]
    #[stable(feature = "byte_slice_trim_ascii", since = "1.80.0")]
    #[rustc_const_stable(feature = "byte_slice_trim_ascii", since = "1.80.0")]
    #[inline]
    pub const fn trim_ascii_end(&self) -> &str {
        // SAFETY: 从 `&str` 中移除 ASCII 字符不会破坏 UTF-8 有效性。
        unsafe { core::str::from_utf8_unchecked(self.as_bytes().trim_ascii_end()) }
    }

    /// 返回去除了开头和结尾 ASCII 空白字符的字符串切片。
    ///
    /// “Whitespace” 指 [`u8::is_ascii_whitespace`] 使用的定义。
    ///
    /// [`u8::is_ascii_whitespace`]: u8::is_ascii_whitespace
    ///
    /// # 示例
    ///
    /// ```
    /// assert_eq!("\r hello world\n ".trim_ascii(), "hello world");
    /// assert_eq!("  ".trim_ascii(), "");
    /// assert_eq!("".trim_ascii(), "");
    /// ```
    #[must_use = "this returns the trimmed string as a new slice, \
                  without modifying the original"]
    #[stable(feature = "byte_slice_trim_ascii", since = "1.80.0")]
    #[rustc_const_stable(feature = "byte_slice_trim_ascii", since = "1.80.0")]
    #[inline]
    pub const fn trim_ascii(&self) -> &str {
        // SAFETY: 从 `&str` 中移除 ASCII 字符不会破坏 UTF-8 有效性。
        unsafe { core::str::from_utf8_unchecked(self.as_bytes().trim_ascii()) }
    }

    /// 返回一个迭代器，使用 [`char::escape_debug`] 转义 `self` 中的每个 char。
    ///
    /// 注意：只有出现在字符串开头的扩展字素 codepoint 会被转义。
    ///
    /// # 示例
    ///
    /// 作为迭代器：
    ///
    /// ```
    /// for c in "❤\n!".escape_debug() {
    ///     print!("{c}");
    /// }
    /// println!();
    /// ```
    ///
    /// 直接使用 `println!`：
    ///
    /// ```
    /// println!("{}", "❤\n!".escape_debug());
    /// ```
    ///
    ///
    /// 二者等价于：
    ///
    /// ```
    /// println!("❤\\n!");
    /// ```
    ///
    /// 使用 `to_string`：
    ///
    /// ```
    /// assert_eq!("❤\n!".escape_debug().to_string(), "❤\\n!");
    /// ```
    #[must_use = "this returns the escaped string as an iterator, \
                  without modifying the original"]
    #[stable(feature = "str_escape", since = "1.34.0")]
    pub fn escape_debug(&self) -> EscapeDebug<'_> {
        let mut chars = self.chars();
        EscapeDebug {
            inner: chars
                .next()
                .map(|first| first.escape_debug_ext(EscapeDebugExtArgs::ESCAPE_ALL))
                .into_iter()
                .flatten()
                .chain(chars.flat_map(CharEscapeDebugContinue)),
        }
    }

    /// 返回一个迭代器，使用 [`char::escape_default`] 转义 `self` 中的每个 char。
    ///
    /// # 示例
    ///
    /// 作为迭代器：
    ///
    /// ```
    /// for c in "❤\n!".escape_default() {
    ///     print!("{c}");
    /// }
    /// println!();
    /// ```
    ///
    /// 直接使用 `println!`：
    ///
    /// ```
    /// println!("{}", "❤\n!".escape_default());
    /// ```
    ///
    ///
    /// 二者等价于：
    ///
    /// ```
    /// println!("\\u{{2764}}\\n!");
    /// ```
    ///
    /// 使用 `to_string`：
    ///
    /// ```
    /// assert_eq!("❤\n!".escape_default().to_string(), "\\u{2764}\\n!");
    /// ```
    #[must_use = "this returns the escaped string as an iterator, \
                  without modifying the original"]
    #[stable(feature = "str_escape", since = "1.34.0")]
    pub fn escape_default(&self) -> EscapeDefault<'_> {
        EscapeDefault { inner: self.chars().flat_map(CharEscapeDefault) }
    }

    /// 返回一个迭代器，使用 [`char::escape_unicode`] 转义 `self` 中的每个 char。
    ///
    /// # 示例
    ///
    /// 作为迭代器：
    ///
    /// ```
    /// for c in "❤\n!".escape_unicode() {
    ///     print!("{c}");
    /// }
    /// println!();
    /// ```
    ///
    /// 直接使用 `println!`：
    ///
    /// ```
    /// println!("{}", "❤\n!".escape_unicode());
    /// ```
    ///
    ///
    /// 二者等价于：
    ///
    /// ```
    /// println!("\\u{{2764}}\\u{{a}}\\u{{21}}");
    /// ```
    ///
    /// 使用 `to_string`：
    ///
    /// ```
    /// assert_eq!("❤\n!".escape_unicode().to_string(), "\\u{2764}\\u{a}\\u{21}");
    /// ```
    #[must_use = "this returns the escaped string as an iterator, \
                  without modifying the original"]
    #[stable(feature = "str_escape", since = "1.34.0")]
    pub fn escape_unicode(&self) -> EscapeUnicode<'_> {
        EscapeUnicode { inner: self.chars().flat_map(CharEscapeUnicode) }
    }

    /// 返回某个子字符串指向的范围。
    ///
    /// 如果 `substr` 不指向 `self` 内部，则返回 `None`。
    ///
    /// 与 [`str::find`] 不同，**本方法不会在字符串中搜索**。它会使用指针运算判断
    /// `substr` 是从该字符串的哪个位置派生出来的。
    ///
    /// 这对扩展 [`str::split`] 及类似方法很有用。
    ///
    /// 注意，如果 `substr` 是零长度 `str`，并且指向另一个独立 `str` 的开头或结尾，
    /// 本方法可能返回误报（通常是 `Some(0..0)` 或 `Some(self.len()..self.len())`）。
    ///
    /// # 示例
    /// ```
    /// #![feature(substr_range)]
    ///
    /// let data = "a, b, b, a";
    /// let mut iter = data.split(", ").map(|s| data.substr_range(s).unwrap());
    ///
    /// assert_eq!(iter.next(), Some(0..1));
    /// assert_eq!(iter.next(), Some(3..4));
    /// assert_eq!(iter.next(), Some(6..7));
    /// assert_eq!(iter.next(), Some(9..10));
    /// ```
    #[must_use]
    #[unstable(feature = "substr_range", issue = "126769")]
    pub fn substr_range(&self, substr: &str) -> Option<Range<usize>> {
        self.as_bytes().subslice_range(substr.as_bytes())
    }

    /// 将同一个字符串作为字符串切片 `&str` 返回。
    ///
    /// 直接在 `&str` 上使用时，本方法是冗余的；但它有助于把其他类字符串类型解引用为
    /// 字符串切片，例如对 `Box<str>` 或 `Arc<str>` 的引用。
    #[inline]
    #[unstable(feature = "str_as_str", issue = "130366")]
    pub const fn as_str(&self) -> &str {
        self
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl const AsRef<[u8]> for str {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        self.as_bytes()
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_unstable(feature = "const_default", issue = "143894")]
impl const Default for &str {
    /// 创建空 str。
    #[inline]
    fn default() -> Self {
        ""
    }
}

#[stable(feature = "default_mut_str", since = "1.28.0")]
#[rustc_const_unstable(feature = "const_default", issue = "143894")]
impl const Default for &mut str {
    /// 创建空可变 str。
    #[inline]
    fn default() -> Self {
        // SAFETY: 空字符串是有效 UTF-8。
        unsafe { from_utf8_unchecked_mut(&mut []) }
    }
}

impl_fn_for_zst! {
    /// 可命名、可克隆的 fn 类型。
    #[derive(Clone)]
    struct LinesMap impl<'a> Fn = |line: &'a str| -> &'a str {
        let Some(line) = line.strip_suffix('\n') else { return line };
        let Some(line) = line.strip_suffix('\r') else { return line };
        line
    };

    #[derive(Clone)]
    struct CharEscapeDebugContinue impl Fn = |c: char| -> char::EscapeDebug {
        c.escape_debug_ext(EscapeDebugExtArgs {
            escape_grapheme_extended: false,
            escape_single_quote: true,
            escape_double_quote: true
        })
    };

    #[derive(Clone)]
    struct CharEscapeUnicode impl Fn = |c: char| -> char::EscapeUnicode {
        c.escape_unicode()
    };
    #[derive(Clone)]
    struct CharEscapeDefault impl Fn = |c: char| -> char::EscapeDefault {
        c.escape_default()
    };

    #[derive(Clone)]
    struct IsWhitespace impl Fn = |c: char| -> bool {
        c.is_whitespace()
    };

    #[derive(Clone)]
    struct IsAsciiWhitespace impl Fn = |byte: &u8| -> bool {
        byte.is_ascii_whitespace()
    };

    #[derive(Clone)]
    struct IsNotEmpty impl<'a, 'b> Fn = |s: &'a &'b str| -> bool {
        !s.is_empty()
    };

    #[derive(Clone)]
    struct BytesIsNotEmpty impl<'a, 'b> Fn = |s: &'a &'b [u8]| -> bool {
        !s.is_empty()
    };

    #[derive(Clone)]
    struct UnsafeBytesToStr impl<'a> Fn = |bytes: &'a [u8]| -> &'a str {
        // SAFETY: 调用点已经保证这些字节来自有效 UTF-8 的 `str`。
        unsafe { from_utf8_unchecked(bytes) }
    };
}

// 该负实现用于避免 `impl From<&str> for Box<dyn Error>` 与
// `impl<E> From<E> for Box<dyn Error>` 重叠。
#[stable(feature = "error_in_core_neg_impl", since = "1.65.0")]
impl !crate::error::Error for &str {}
