//! 字符串 Pattern API。
//!
//! Pattern API 提供一套通用机制，让不同类型的模式都能参与字符串搜索。
//! 这些模式最终都在 `str` 的 UTF-8 字节序列上产生匹配范围。
//!
//! 更多细节见 [`Pattern`]、[`Searcher`]、[`ReverseSearcher`] 和
//! [`DoubleEndedSearcher`] 这些 trait。
//!
//! 虽然本 API 自身仍是不稳定的，但它通过 [`str`] 类型上的稳定方法暴露给用户。
//!
//! # 示例
//!
//! 在稳定 API 中，[`&str`][`str`]、[`char`]、[`char`] 切片以及实现
//! `FnMut(char) -> bool` 的函数或闭包都实现了 [`Pattern`][pattern-impls]。
//!
//! ```
//! let s = "Can you find a needle in a haystack?";
//!
//! // &str 模式
//! assert_eq!(s.find("you"), Some(4));
//! // char 模式
//! assert_eq!(s.find('n'), Some(2));
//! // char 数组模式
//! assert_eq!(s.find(&['a', 'e', 'i', 'o', 'u']), Some(1));
//! // char 切片模式
//! assert_eq!(s.find(&['a', 'e', 'i', 'o', 'u'][..]), Some(1));
//! // 闭包模式
//! assert_eq!(s.find(|c: char| c.is_ascii_punctuation()), Some(35));
//! ```
//!
//! [pattern-impls]: Pattern#implementors

#![unstable(
    feature = "pattern",
    reason = "API not fully fleshed out and ready to be stabilized",
    issue = "27721"
)]

use crate::cmp::Ordering;
use crate::convert::TryInto as _;
use crate::slice::memchr;
use crate::{cmp, fmt};

// Pattern trait。

/// 字符串模式。
///
/// `Pattern` 表示实现该 trait 的类型可以作为字符串模式，在 [`&str`][str]
/// 中进行搜索。搜索结果以字节索引表示，因此实现者必须遵守 `str` 的 UTF-8
/// 有效性和字符边界不变量。
///
/// 例如，`'a'` 和 `"aa"` 都是模式，并且都可以在字符串 `"baaaab"` 的索引
/// `1` 处产生匹配。
///
/// 这个 trait 本身相当于关联 [`Searcher`] 类型的构造器；实际在字符串中寻找模式
/// 出现位置的工作由对应的 [`Searcher`] 完成。
///
/// 根据模式类型的不同，[`str::find`] 和 [`str::contains`] 等方法的行为也会不同。
/// 下表概述了一些常见模式的匹配条件。
///
/// | Pattern 类型             | 匹配条件                                  |
/// |--------------------------|-------------------------------------------|
/// | `&str`                   | 是子字符串                                |
/// | `char`                   | 字符串中包含该 `char`                     |
/// | `&[char]`                | 切片中的任意 `char` 出现在字符串中        |
/// | `F: FnMut(char) -> bool` | `F` 对字符串中的某个 `char` 返回 `true`   |
/// | `&&str`                  | 是子字符串                                |
/// | `&String`                | 是子字符串                                |
///
/// # 示例
///
/// ```
/// // &str
/// assert_eq!("abaaa".find("ba"), Some(1));
/// assert_eq!("abaaa".find("bac"), None);
///
/// // char
/// assert_eq!("abaaa".find('a'), Some(0));
/// assert_eq!("abaaa".find('b'), Some(1));
/// assert_eq!("abaaa".find('c'), None);
///
/// // &[char; N]
/// assert_eq!("ab".find(&['b', 'a']), Some(0));
/// assert_eq!("abaaa".find(&['a', 'z']), Some(0));
/// assert_eq!("abaaa".find(&['c', 'd']), None);
///
/// // &[char]
/// assert_eq!("ab".find(&['b', 'a'][..]), Some(0));
/// assert_eq!("abaaa".find(&['a', 'z'][..]), Some(0));
/// assert_eq!("abaaa".find(&['c', 'd'][..]), None);
///
/// // FnMut(char) -> bool
/// assert_eq!("abcdef_z".find(|ch| ch > 'd' && ch < 'y'), Some(4));
/// assert_eq!("abcddd_z".find(|ch| ch > 'd' && ch < 'y'), None);
/// ```
pub trait Pattern: Sized {
    /// 该模式关联的搜索器类型。
    type Searcher<'a>: Searcher<'a>;

    /// 根据 `self` 和待搜索的 `haystack` 构造关联搜索器。
    fn into_searcher(self, haystack: &str) -> Self::Searcher<'_>;

    /// 检查模式是否在 `haystack` 的任意位置匹配。
    #[inline]
    fn is_contained_in(self, haystack: &str) -> bool {
        self.into_searcher(haystack).next_match().is_some()
    }

    /// 检查模式是否匹配 `haystack` 的开头。
    #[inline]
    fn is_prefix_of(self, haystack: &str) -> bool {
        matches!(self.into_searcher(haystack).next(), SearchStep::Match(0, _))
    }

    /// 检查模式是否匹配 `haystack` 的结尾。
    #[inline]
    fn is_suffix_of<'a>(self, haystack: &'a str) -> bool
    where
        Self::Searcher<'a>: ReverseSearcher<'a>,
    {
        matches!(self.into_searcher(haystack).next_back(), SearchStep::Match(_, j) if haystack.len() == j)
    }

    /// 如果模式匹配，则从 `haystack` 开头移除该模式。
    #[inline]
    fn strip_prefix_of(self, haystack: &str) -> Option<&str> {
        if let SearchStep::Match(start, len) = self.into_searcher(haystack).next() {
            debug_assert_eq!(
                start, 0,
                "The first search step from Searcher \
                 must include the first character"
            );
            // SAFETY: `Searcher` 保证返回位于字符边界上的有效索引。
            unsafe { Some(haystack.get_unchecked(len..)) }
        } else {
            None
        }
    }

    /// 如果模式匹配，则从 `haystack` 结尾移除该模式。
    #[inline]
    fn strip_suffix_of<'a>(self, haystack: &'a str) -> Option<&'a str>
    where
        Self::Searcher<'a>: ReverseSearcher<'a>,
    {
        if let SearchStep::Match(start, end) = self.into_searcher(haystack).next_back() {
            debug_assert_eq!(
                end,
                haystack.len(),
                "The first search step from ReverseSearcher \
                 must include the last character"
            );
            // SAFETY: `Searcher` 保证返回位于字符边界上的有效索引。
            unsafe { Some(haystack.get_unchecked(..start)) }
        } else {
            None
        }
    }

    /// 如果可行，将模式作为 UTF-8 字节返回。
    fn as_utf8_pattern(&self) -> Option<Utf8Pattern<'_>> {
        None
    }
}
/// 调用 [`Pattern::as_utf8_pattern()`] 的结果。
/// 当底层表示可以表达为 UTF-8 时，可用它检查 [`Pattern`] 的内容。
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum Utf8Pattern<'a> {
    /// `String` 和 `str` 类型返回的形式。
    StringPattern(&'a [u8]),
    /// `char` 类型返回的形式。
    CharPattern(char),
}

// Searcher trait。

/// 调用 [`Searcher::next()`] 或 [`ReverseSearcher::next_back()`] 的结果。
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum SearchStep {
    /// 表示在 `haystack[a..b]` 找到了模式匹配。
    Match(usize, usize),
    /// 表示 `haystack[a..b]` 已被排除，不可能成为该模式的匹配。
    ///
    /// 注意，两个 `Match` 之间可以有多个 `Reject`，实现不需要把它们合并成一个范围。
    Reject(usize, usize),
    /// 表示已经访问完 `haystack` 的每个字节，迭代结束。
    Done,
}

/// 字符串模式的搜索器。
///
/// 这个 trait 提供从字符串前端（左侧）开始搜索非重叠模式匹配的方法。
///
/// 它由 [`Pattern`] trait 的关联 `Searcher` 类型实现。
///
/// 该 trait 标记为 unsafe，因为 [`next()`][Searcher::next] 返回的索引必须落在
/// `haystack` 的有效 UTF-8 字符边界上。这样调用方才能在不额外做运行时检查的情况下
/// 对 `haystack` 切片，而不会破坏 `str` 的 UTF-8 有效性不变量。
pub unsafe trait Searcher<'a> {
    /// 取得底层待搜索字符串。
    ///
    /// 该方法总是返回同一个 [`&str`][str]。
    fn haystack(&self) -> &'a str;

    /// 从前端开始执行下一个搜索步骤。
    ///
    /// - 如果 `haystack[a..b]` 匹配模式，返回 [`Match(a, b)`][SearchStep::Match]。
    /// - 如果 `haystack[a..b]` 即使作为部分内容也不可能匹配模式，返回
    ///   [`Reject(a, b)`][SearchStep::Reject]。
    /// - 如果已经访问完 `haystack` 的每个字节，返回 [`Done`][SearchStep::Done]。
    ///
    /// 在到达 [`Done`][SearchStep::Done] 之前，[`Match`][SearchStep::Match] 和
    /// [`Reject`][SearchStep::Reject] 组成的流包含一组相邻、不重叠、覆盖整个
    /// `haystack` 且位于 UTF-8 字符边界上的索引范围。
    ///
    /// [`Match`][SearchStep::Match] 结果必须包含完整匹配的模式；而
    /// [`Reject`][SearchStep::Reject] 结果可以被拆分为任意多个相邻片段。
    /// 两类范围都可以是零长度。
    ///
    /// 例如，模式 `"aaa"` 和 `haystack` `"cbaaaaab"` 可能产生如下流：
    /// `[Reject(0, 1), Reject(1, 2), Match(2, 5), Reject(5, 8)]`
    fn next(&mut self) -> SearchStep;

    /// 查找下一个 [`Match`][SearchStep::Match] 结果。见 [`next()`][Searcher::next]。
    ///
    /// 与 [`next()`][Searcher::next] 不同，本方法和
    /// [`next_reject`][Searcher::next_reject] 返回的范围之间没有覆盖关系保证。
    /// 它返回 `(start_match, end_match)`，其中 `start_match` 是匹配开始的索引，
    /// `end_match` 是匹配结束后的索引。
    #[inline]
    fn next_match(&mut self) -> Option<(usize, usize)> {
        loop {
            match self.next() {
                SearchStep::Match(a, b) => return Some((a, b)),
                SearchStep::Done => return None,
                _ => continue,
            }
        }
    }

    /// 查找下一个 [`Reject`][SearchStep::Reject] 结果。见 [`next()`][Searcher::next]
    /// 和 [`next_match()`][Searcher::next_match]。
    ///
    /// 与 [`next()`][Searcher::next] 不同，本方法和
    /// [`next_match`][Searcher::next_match] 返回的范围之间没有覆盖关系保证。
    #[inline]
    fn next_reject(&mut self) -> Option<(usize, usize)> {
        loop {
            match self.next() {
                SearchStep::Reject(a, b) => return Some((a, b)),
                SearchStep::Done => return None,
                _ => continue,
            }
        }
    }
}

/// 字符串模式的反向搜索器。
///
/// 这个 trait 提供从字符串后端（右侧）开始搜索非重叠模式匹配的方法。
///
/// 如果模式支持从后向前搜索，则由 [`Pattern`] trait 的关联 [`Searcher`] 类型实现。
///
/// 该 trait 返回的索引范围不要求与正向搜索结果严格反向对应。
///
/// 关于该 trait 标记为 unsafe 的原因，见父 trait [`Searcher`]。
pub unsafe trait ReverseSearcher<'a>: Searcher<'a> {
    /// 从后端开始执行下一个搜索步骤。
    ///
    /// - 如果 `haystack[a..b]` 匹配模式，返回 [`Match(a, b)`][SearchStep::Match]。
    /// - 如果 `haystack[a..b]` 即使作为部分内容也不可能匹配模式，返回
    ///   [`Reject(a, b)`][SearchStep::Reject]。
    /// - 如果已经访问完 `haystack` 的每个字节，返回 [`Done`][SearchStep::Done]。
    ///
    /// 在到达 [`Done`][SearchStep::Done] 之前，[`Match`][SearchStep::Match] 和
    /// [`Reject`][SearchStep::Reject] 组成的流包含一组相邻、不重叠、覆盖整个
    /// `haystack` 且位于 UTF-8 字符边界上的索引范围。
    ///
    /// [`Match`][SearchStep::Match] 结果必须包含完整匹配的模式；而
    /// [`Reject`][SearchStep::Reject] 结果可以被拆分为任意多个相邻片段。
    /// 两类范围都可以是零长度。
    ///
    /// 例如，模式 `"aaa"` 和 `haystack` `"cbaaaaab"` 可能产生如下流：
    /// `[Reject(7, 8), Match(4, 7), Reject(1, 4), Reject(0, 1)]`.
    fn next_back(&mut self) -> SearchStep;

    /// 查找下一个 [`Match`][SearchStep::Match] 结果。
    /// 见 [`next_back()`][ReverseSearcher::next_back]。
    #[inline]
    fn next_match_back(&mut self) -> Option<(usize, usize)> {
        loop {
            match self.next_back() {
                SearchStep::Match(a, b) => return Some((a, b)),
                SearchStep::Done => return None,
                _ => continue,
            }
        }
    }

    /// 查找下一个 [`Reject`][SearchStep::Reject] 结果。
    /// 见 [`next_back()`][ReverseSearcher::next_back]。
    #[inline]
    fn next_reject_back(&mut self) -> Option<(usize, usize)> {
        loop {
            match self.next_back() {
                SearchStep::Reject(a, b) => return Some((a, b)),
                SearchStep::Done => return None,
                _ => continue,
            }
        }
    }
}

/// 标记某个 [`ReverseSearcher`] 可用于实现 [`DoubleEndedIterator`] 的 trait。
///
/// 为了满足这个标记，[`Searcher`] 和 [`ReverseSearcher`] 的实现必须满足这些条件：
///
/// - `next()` 的所有结果必须与 `next_back()` 的结果按相反顺序完全一致。
/// - `next()` 和 `next_back()` 必须像同一值域的两端一样推进，也就是说二者不能
///   “越过”彼此。
///
/// # 示例
///
/// `char::Searcher` 是 `DoubleEndedSearcher`，因为搜索单个 [`char`] 时每次只需
/// 查看一个 Unicode 标量值，从两端搜索的行为一致。
///
/// `(&str)::Searcher` 不是 `DoubleEndedSearcher`，因为在 `haystack` `"aaa"` 中搜索
/// 模式 `"aa"` 时，结果可能是 `"[aa]a"`，也可能是 `"a[aa]"`，取决于搜索方向。
pub trait DoubleEndedSearcher<'a>: ReverseSearcher<'a> {}

/////////////////////////////////////////////////////////////////////////////
// char 的实现。
/////////////////////////////////////////////////////////////////////////////

/// `<char as Pattern>::Searcher<'a>` 的关联类型。
#[derive(Clone, Debug)]
pub struct CharSearcher<'a> {
    haystack: &'a str,
    // 安全不变量：`finger`/`finger_back` 必须是 `haystack` 内有效的 UTF-8 字节索引。
    // `next_match` 和 `next_match_back` 内部可能暂时破坏该不变量，但退出这些方法时
    // 游标必须回到有效的 code point 边界上。
    /// `finger` 是正向搜索当前所在的字节索引。
    /// 可以把它理解为位于该索引字节之前，也就是说，正向搜索时
    /// `haystack[finger]` 是接下来必须检查的切片的第一个字节。
    finger: usize,
    /// `finger_back` 是反向搜索当前所在的字节索引。
    /// 可以把它理解为位于其索引前一个字节之后，也就是说，
    /// `haystack[finger_back - 1]` 是正向搜索时必须检查的切片的最后一个字节
    /// （也就是调用 `next_back()` 时首先检查的字节）。
    finger_back: usize,
    /// 正在搜索的字符。
    needle: char,

    // 安全不变量：`utf8_size` 必须小于 5。
    /// `needle` 编码为 UTF-8 后占用的字节数。
    utf8_size: u8,
    /// `needle` 的 UTF-8 编码副本。
    utf8_encoded: [u8; 4],
}

impl CharSearcher<'_> {
    fn utf8_size(&self) -> usize {
        self.utf8_size.into()
    }
}

unsafe impl<'a> Searcher<'a> for CharSearcher<'a> {
    #[inline]
    fn haystack(&self) -> &'a str {
        self.haystack
    }
    #[inline]
    fn next(&mut self) -> SearchStep {
        let old_finger = self.finger;
        // SAFETY: 以下 1-4 点共同保证 `get_unchecked` 安全。
        // 1. `self.finger` 和 `self.finger_back` 始终保持在 Unicode 边界上
        //    （这是本类型的不变量）。
        // 2. `self.finger >= 0`，因为它从 0 开始且只会递增。
        // 3. `self.finger < self.finger_back`，否则 `char` 迭代器会返回
        //    `SearchStep::Done`。
        // 4. `self.finger` 位于 haystack 结尾之前，因为 `self.finger_back`
        //    从结尾开始且只会递减。
        let slice = unsafe { self.haystack.get_unchecked(old_finger..self.finger_back) };
        let mut iter = slice.chars();
        let old_len = iter.iter.len();
        if let Some(ch) = iter.next() {
            // 添加当前字符的字节偏移，不重新编码为 UTF-8。
            self.finger += old_len - iter.iter.len();
            if ch == self.needle {
                SearchStep::Match(old_finger, self.finger)
            } else {
                SearchStep::Reject(old_finger, self.finger)
            }
        } else {
            SearchStep::Done
        }
    }
    #[inline]
    fn next_match(&mut self) -> Option<(usize, usize)> {
        loop {
            // 取得上一个已找到字符之后的 haystack 部分。
            let bytes = self.haystack.as_bytes().get(self.finger..self.finger_back)?;
            // UTF-8 编码后的 needle 的最后一个字节。
            // SAFETY: 本类型有 `utf8_size < 5` 的不变量。
            let last_byte = unsafe { *self.utf8_encoded.get_unchecked(self.utf8_size() - 1) };
            if let Some(index) = memchr::memchr(last_byte, bytes) {
                // 新的 finger 是找到的字节索引加一，因为 memchr 搜索的是该字符的最后一个字节。
                //
                // 注意，这并不总是让 finger 落在 UTF-8 边界上。如果实际没有找到目标字符，
                // 我们可能定位到了某个 3 字节或 4 字节字符的非最后字节。不能简单跳到下一个
                // 有效起始字节，因为像 ꁁ (U+A041 YI SYLLABLE PA)，UTF-8 为 `EA 81 81`
                // 这样的字符，会让我们在搜索第三个字节时总是先找到第二个字节。
                //
                // 但这在本方法内部是允许的。虽然外部不变量要求 `self.finger` 位于 UTF-8
                // 边界上，本方法内部并不依赖该不变量（`CharSearcher::next()` 才依赖它）。
                //
                // 本方法只会在到达字符串结尾或找到匹配时退出。找到匹配时，`finger`
                // 会被设置回 UTF-8 边界。
                self.finger += index + 1;
                if self.finger >= self.utf8_size() {
                    let found_char = self.finger - self.utf8_size();
                    if let Some(slice) = self.haystack.as_bytes().get(found_char..self.finger) {
                        if slice == &self.utf8_encoded[0..self.utf8_size()] {
                            return Some((found_char, self.finger));
                        }
                    }
                }
            } else {
                // 未找到任何内容，退出。
                self.finger = self.finger_back;
                return None;
            }
        }
    }

    // 让 next_reject 使用 Searcher trait 提供的默认实现。
}

unsafe impl<'a> ReverseSearcher<'a> for CharSearcher<'a> {
    #[inline]
    fn next_back(&mut self) -> SearchStep {
        let old_finger = self.finger_back;
        // SAFETY: 见上方 next() 的注释。
        let slice = unsafe { self.haystack.get_unchecked(self.finger..old_finger) };
        let mut iter = slice.chars();
        let old_len = iter.iter.len();
        if let Some(ch) = iter.next_back() {
            // 减去当前字符的字节偏移，不重新编码为 UTF-8。
            self.finger_back -= old_len - iter.iter.len();
            if ch == self.needle {
                SearchStep::Match(self.finger_back, old_finger)
            } else {
                SearchStep::Reject(self.finger_back, old_finger)
            }
        } else {
            SearchStep::Done
        }
    }
    #[inline]
    fn next_match_back(&mut self) -> Option<(usize, usize)> {
        let haystack = self.haystack.as_bytes();
        loop {
            // 取得到上一个已搜索字符之前为止的 haystack 部分，不包含该字符。
            let bytes = haystack.get(self.finger..self.finger_back)?;
            // UTF-8 编码后的 needle 的最后一个字节。
            // SAFETY: 本类型有 `utf8_size < 5` 的不变量。
            let last_byte = unsafe { *self.utf8_encoded.get_unchecked(self.utf8_size() - 1) };
            if let Some(index) = memchr::memrchr(last_byte, bytes) {
                // 我们搜索的是以 self.finger 为偏移的切片，因此加回 self.finger
                // 来恢复原始索引。
                let index = self.finger + index;
                // memrchr 会返回我们想找的字节索引。对于 ASCII 字符，这正是新 finger
                // 应在的位置（按反向迭代的模型，即位于找到的 char “之后”）。对于多字节
                // char，还需要按其相对 ASCII 多出的字节数向下调整。
                let shift = self.utf8_size() - 1;
                if index >= shift {
                    let found_char = index - shift;
                    if let Some(slice) = haystack.get(found_char..(found_char + self.utf8_size())) {
                        if slice == &self.utf8_encoded[0..self.utf8_size()] {
                            // 将 finger 移到找到的字符之前，即该字符的起始索引。
                            self.finger_back = found_char;
                            return Some((self.finger_back, self.finger_back + self.utf8_size()));
                        }
                    }
                }
                // 这里不能使用 finger_back = index - size + 1。如果找到的是另一种长度字符的
                // 最后一个字节（或另一个字符的中间字节），就需要把 finger_back 下调到
                // `index`。这同样可能让 `finger_back` 暂时不在边界上，但这是可以接受的，
                // 因为本函数只会在边界上退出，或者在 haystack 已完整搜索后退出。
                //
                // 与 next_match 不同，这里不会遇到 UTF-8 重复字节的问题，因为我们搜索的是
                // 最后一个字节，并且反向搜索时只可能先找到最后一个字节。
                self.finger_back = index;
            } else {
                self.finger_back = self.finger;
                // 未找到任何内容，退出。
                return None;
            }
        }
    }

    // 让 next_reject_back 使用 Searcher trait 提供的默认实现。
}

impl<'a> DoubleEndedSearcher<'a> for CharSearcher<'a> {}

/// 搜索等于给定 [`char`] 的字符。
///
/// # 示例
///
/// ```
/// assert_eq!("Hello world".find('o'), Some(4));
/// ```
impl Pattern for char {
    type Searcher<'a> = CharSearcher<'a>;

    #[inline]
    fn into_searcher<'a>(self, haystack: &'a str) -> Self::Searcher<'a> {
        let mut utf8_encoded = [0; char::MAX_LEN_UTF8];
        let utf8_size = self
            .encode_utf8(&mut utf8_encoded)
            .len()
            .try_into()
            .expect("char len should be less than 255");

        CharSearcher {
            haystack,
            finger: 0,
            finger_back: haystack.len(),
            needle: self,
            utf8_size,
            utf8_encoded,
        }
    }

    #[inline]
    fn is_contained_in(self, haystack: &str) -> bool {
        if (self as u32) < 128 {
            haystack.as_bytes().contains(&(self as u8))
        } else {
            let mut buffer = [0u8; 4];
            self.encode_utf8(&mut buffer).is_contained_in(haystack)
        }
    }

    #[inline]
    fn is_prefix_of(self, haystack: &str) -> bool {
        self.encode_utf8(&mut [0u8; 4]).is_prefix_of(haystack)
    }

    #[inline]
    fn strip_prefix_of(self, haystack: &str) -> Option<&str> {
        self.encode_utf8(&mut [0u8; 4]).strip_prefix_of(haystack)
    }

    #[inline]
    fn is_suffix_of<'a>(self, haystack: &'a str) -> bool
    where
        Self::Searcher<'a>: ReverseSearcher<'a>,
    {
        self.encode_utf8(&mut [0u8; 4]).is_suffix_of(haystack)
    }

    #[inline]
    fn strip_suffix_of<'a>(self, haystack: &'a str) -> Option<&'a str>
    where
        Self::Searcher<'a>: ReverseSearcher<'a>,
    {
        self.encode_utf8(&mut [0u8; 4]).strip_suffix_of(haystack)
    }

    #[inline]
    fn as_utf8_pattern(&self) -> Option<Utf8Pattern<'_>> {
        Some(Utf8Pattern::CharPattern(*self))
    }
}

/////////////////////////////////////////////////////////////////////////////
// MultiCharEq 包装器的实现。
/////////////////////////////////////////////////////////////////////////////

#[doc(hidden)]
trait MultiCharEq {
    fn matches(&mut self, c: char) -> bool;
}

impl<F> MultiCharEq for F
where
    F: FnMut(char) -> bool,
{
    #[inline]
    fn matches(&mut self, c: char) -> bool {
        (*self)(c)
    }
}

impl<const N: usize> MultiCharEq for [char; N] {
    #[inline]
    fn matches(&mut self, c: char) -> bool {
        self.contains(&c)
    }
}

impl<const N: usize> MultiCharEq for &[char; N] {
    #[inline]
    fn matches(&mut self, c: char) -> bool {
        self.contains(&c)
    }
}

impl MultiCharEq for &[char] {
    #[inline]
    fn matches(&mut self, c: char) -> bool {
        self.contains(&c)
    }
}

struct MultiCharEqPattern<C: MultiCharEq>(C);

#[derive(Clone, Debug)]
struct MultiCharEqSearcher<'a, C: MultiCharEq> {
    char_eq: C,
    haystack: &'a str,
    char_indices: super::CharIndices<'a>,
}

impl<C: MultiCharEq> Pattern for MultiCharEqPattern<C> {
    type Searcher<'a> = MultiCharEqSearcher<'a, C>;

    #[inline]
    fn into_searcher(self, haystack: &str) -> MultiCharEqSearcher<'_, C> {
        MultiCharEqSearcher { haystack, char_eq: self.0, char_indices: haystack.char_indices() }
    }
}

unsafe impl<'a, C: MultiCharEq> Searcher<'a> for MultiCharEqSearcher<'a, C> {
    #[inline]
    fn haystack(&self) -> &'a str {
        self.haystack
    }

    #[inline]
    fn next(&mut self) -> SearchStep {
        let s = &mut self.char_indices;
        // 比较内部字节切片迭代器的长度，计算当前 char 的字节长度。
        let pre_len = s.iter.iter.len();
        if let Some((i, c)) = s.next() {
            let len = s.iter.iter.len();
            let char_len = pre_len - len;
            if self.char_eq.matches(c) {
                return SearchStep::Match(i, i + char_len);
            } else {
                return SearchStep::Reject(i, i + char_len);
            }
        }
        SearchStep::Done
    }
}

unsafe impl<'a, C: MultiCharEq> ReverseSearcher<'a> for MultiCharEqSearcher<'a, C> {
    #[inline]
    fn next_back(&mut self) -> SearchStep {
        let s = &mut self.char_indices;
        // 比较内部字节切片迭代器的长度，计算当前 char 的字节长度。
        let pre_len = s.iter.iter.len();
        if let Some((i, c)) = s.next_back() {
            let len = s.iter.iter.len();
            let char_len = pre_len - len;
            if self.char_eq.matches(c) {
                return SearchStep::Match(i, i + char_len);
            } else {
                return SearchStep::Reject(i, i + char_len);
            }
        }
        SearchStep::Done
    }
}

impl<'a, C: MultiCharEq> DoubleEndedSearcher<'a> for MultiCharEqSearcher<'a, C> {}

/////////////////////////////////////////////////////////////////////////////

macro_rules! pattern_methods {
    ($a:lifetime, $t:ty, $pmap:expr, $smap:expr) => {
        type Searcher<$a> = $t;

        #[inline]
        fn into_searcher<$a>(self, haystack: &$a str) -> $t {
            ($smap)(($pmap)(self).into_searcher(haystack))
        }

        #[inline]
        fn is_contained_in<$a>(self, haystack: &$a str) -> bool {
            ($pmap)(self).is_contained_in(haystack)
        }

        #[inline]
        fn is_prefix_of<$a>(self, haystack: &$a str) -> bool {
            ($pmap)(self).is_prefix_of(haystack)
        }

        #[inline]
        fn strip_prefix_of<$a>(self, haystack: &$a str) -> Option<&$a str> {
            ($pmap)(self).strip_prefix_of(haystack)
        }

        #[inline]
        fn is_suffix_of<$a>(self, haystack: &$a str) -> bool
        where
            $t: ReverseSearcher<$a>,
        {
            ($pmap)(self).is_suffix_of(haystack)
        }

        #[inline]
        fn strip_suffix_of<$a>(self, haystack: &$a str) -> Option<&$a str>
        where
            $t: ReverseSearcher<$a>,
        {
            ($pmap)(self).strip_suffix_of(haystack)
        }
    };
}

macro_rules! searcher_methods {
    (forward) => {
        #[inline]
        fn haystack(&self) -> &'a str {
            self.0.haystack()
        }
        #[inline]
        fn next(&mut self) -> SearchStep {
            self.0.next()
        }
        #[inline]
        fn next_match(&mut self) -> Option<(usize, usize)> {
            self.0.next_match()
        }
        #[inline]
        fn next_reject(&mut self) -> Option<(usize, usize)> {
            self.0.next_reject()
        }
    };
    (reverse) => {
        #[inline]
        fn next_back(&mut self) -> SearchStep {
            self.0.next_back()
        }
        #[inline]
        fn next_match_back(&mut self) -> Option<(usize, usize)> {
            self.0.next_match_back()
        }
        #[inline]
        fn next_reject_back(&mut self) -> Option<(usize, usize)> {
            self.0.next_reject_back()
        }
    };
}

/// `<[char; N] as Pattern>::Searcher<'a>` 的关联类型。
#[derive(Clone, Debug)]
pub struct CharArraySearcher<'a, const N: usize>(
    <MultiCharEqPattern<[char; N]> as Pattern>::Searcher<'a>,
);

/// `<&[char; N] as Pattern>::Searcher<'a>` 的关联类型。
#[derive(Clone, Debug)]
pub struct CharArrayRefSearcher<'a, 'b, const N: usize>(
    <MultiCharEqPattern<&'b [char; N]> as Pattern>::Searcher<'a>,
);

/// 搜索与数组中任意 [`char`] 相等的字符。
///
/// # 示例
///
/// ```
/// assert_eq!("Hello world".find(['o', 'l']), Some(2));
/// assert_eq!("Hello world".find(['h', 'w']), Some(6));
/// ```
impl<const N: usize> Pattern for [char; N] {
    pattern_methods!('a, CharArraySearcher<'a, N>, MultiCharEqPattern, CharArraySearcher);
}

unsafe impl<'a, const N: usize> Searcher<'a> for CharArraySearcher<'a, N> {
    searcher_methods!(forward);
}

unsafe impl<'a, const N: usize> ReverseSearcher<'a> for CharArraySearcher<'a, N> {
    searcher_methods!(reverse);
}

impl<'a, const N: usize> DoubleEndedSearcher<'a> for CharArraySearcher<'a, N> {}

/// 搜索与数组中任意 [`char`] 相等的字符。
///
/// # 示例
///
/// ```
/// assert_eq!("Hello world".find(&['o', 'l']), Some(2));
/// assert_eq!("Hello world".find(&['h', 'w']), Some(6));
/// ```
impl<'b, const N: usize> Pattern for &'b [char; N] {
    pattern_methods!('a, CharArrayRefSearcher<'a, 'b, N>, MultiCharEqPattern, CharArrayRefSearcher);
}

unsafe impl<'a, 'b, const N: usize> Searcher<'a> for CharArrayRefSearcher<'a, 'b, N> {
    searcher_methods!(forward);
}

unsafe impl<'a, 'b, const N: usize> ReverseSearcher<'a> for CharArrayRefSearcher<'a, 'b, N> {
    searcher_methods!(reverse);
}

impl<'a, 'b, const N: usize> DoubleEndedSearcher<'a> for CharArrayRefSearcher<'a, 'b, N> {}

/////////////////////////////////////////////////////////////////////////////
// &[char] 的实现。
/////////////////////////////////////////////////////////////////////////////

// Todo: 由于含义存在歧义，后续需要修改或移除。

/// `<&[char] as Pattern>::Searcher<'a>` 的关联类型。
#[derive(Clone, Debug)]
pub struct CharSliceSearcher<'a, 'b>(<MultiCharEqPattern<&'b [char]> as Pattern>::Searcher<'a>);

unsafe impl<'a, 'b> Searcher<'a> for CharSliceSearcher<'a, 'b> {
    searcher_methods!(forward);
}

unsafe impl<'a, 'b> ReverseSearcher<'a> for CharSliceSearcher<'a, 'b> {
    searcher_methods!(reverse);
}

impl<'a, 'b> DoubleEndedSearcher<'a> for CharSliceSearcher<'a, 'b> {}

/// 搜索与切片中任意 [`char`] 相等的字符。
///
/// # 示例
///
/// ```
/// assert_eq!("Hello world".find(&['o', 'l'][..]), Some(2));
/// assert_eq!("Hello world".find(&['h', 'w'][..]), Some(6));
/// ```
impl<'b> Pattern for &'b [char] {
    pattern_methods!('a, CharSliceSearcher<'a, 'b>, MultiCharEqPattern, CharSliceSearcher);
}

/////////////////////////////////////////////////////////////////////////////
// F: FnMut(char) -> bool 的实现。
/////////////////////////////////////////////////////////////////////////////

/// `<F as Pattern>::Searcher<'a>` 的关联类型。
#[derive(Clone)]
pub struct CharPredicateSearcher<'a, F>(<MultiCharEqPattern<F> as Pattern>::Searcher<'a>)
where
    F: FnMut(char) -> bool;

impl<F> fmt::Debug for CharPredicateSearcher<'_, F>
where
    F: FnMut(char) -> bool,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CharPredicateSearcher")
            .field("haystack", &self.0.haystack)
            .field("char_indices", &self.0.char_indices)
            .finish()
    }
}
unsafe impl<'a, F> Searcher<'a> for CharPredicateSearcher<'a, F>
where
    F: FnMut(char) -> bool,
{
    searcher_methods!(forward);
}

unsafe impl<'a, F> ReverseSearcher<'a> for CharPredicateSearcher<'a, F>
where
    F: FnMut(char) -> bool,
{
    searcher_methods!(reverse);
}

impl<'a, F> DoubleEndedSearcher<'a> for CharPredicateSearcher<'a, F> where F: FnMut(char) -> bool {}

/// 搜索满足给定谓词的 [`char`]。
///
/// # 示例
///
/// ```
/// assert_eq!("Hello world".find(char::is_uppercase), Some(0));
/// assert_eq!("Hello world".find(|c| "aeiou".contains(c)), Some(1));
/// ```
impl<F> Pattern for F
where
    F: FnMut(char) -> bool,
{
    pattern_methods!('a, CharPredicateSearcher<'a, F>, MultiCharEqPattern, CharPredicateSearcher);
}

/////////////////////////////////////////////////////////////////////////////
// &&str 的实现。
/////////////////////////////////////////////////////////////////////////////

/// 委托给 `&str` 的实现。
impl<'b, 'c> Pattern for &'c &'b str {
    pattern_methods!('a, StrSearcher<'a, 'b>, |&s| s, |s| s);
}

/////////////////////////////////////////////////////////////////////////////
// &str 的实现。
/////////////////////////////////////////////////////////////////////////////

/// 不分配内存的子字符串搜索。
///
/// 对模式 `""` 的处理方式是在每个字符边界返回空匹配。
///
/// # 示例
///
/// ```
/// assert_eq!("Hello world".find("world"), Some(6));
/// ```
impl<'b> Pattern for &'b str {
    type Searcher<'a> = StrSearcher<'a, 'b>;

    #[inline]
    fn into_searcher(self, haystack: &str) -> StrSearcher<'_, 'b> {
        StrSearcher::new(haystack, self)
    }

    /// 检查模式是否匹配 `haystack` 的开头。
    #[inline]
    fn is_prefix_of(self, haystack: &str) -> bool {
        haystack.as_bytes().starts_with(self.as_bytes())
    }

    /// 检查模式是否在 `haystack` 的任意位置匹配。
    #[inline]
    fn is_contained_in(self, haystack: &str) -> bool {
        if self.len() == 0 {
            return true;
        }

        match self.len().cmp(&haystack.len()) {
            Ordering::Less => {
                if self.len() == 1 {
                    return haystack.as_bytes().contains(&self.as_bytes()[0]);
                }

                #[cfg(any(
                    all(target_arch = "x86_64", target_feature = "sse2"),
                    all(target_arch = "loongarch64", target_feature = "lsx")
                ))]
                if self.len() <= 32 {
                    if let Some(result) = simd_contains(self, haystack) {
                        return result;
                    }
                }

                self.into_searcher(haystack).next_match().is_some()
            }
            _ => self == haystack,
        }
    }

    /// 如果模式匹配，则从 `haystack` 开头移除该模式。
    #[inline]
    fn strip_prefix_of(self, haystack: &str) -> Option<&str> {
        if self.is_prefix_of(haystack) {
            // SAFETY: 刚刚已经确认 prefix 存在。
            unsafe { Some(haystack.get_unchecked(self.as_bytes().len()..)) }
        } else {
            None
        }
    }

    /// 检查模式是否匹配 `haystack` 的结尾。
    #[inline]
    fn is_suffix_of<'a>(self, haystack: &'a str) -> bool
    where
        Self::Searcher<'a>: ReverseSearcher<'a>,
    {
        haystack.as_bytes().ends_with(self.as_bytes())
    }

    /// 如果模式匹配，则从 `haystack` 结尾移除该模式。
    #[inline]
    fn strip_suffix_of<'a>(self, haystack: &'a str) -> Option<&'a str>
    where
        Self::Searcher<'a>: ReverseSearcher<'a>,
    {
        if self.is_suffix_of(haystack) {
            let i = haystack.len() - self.as_bytes().len();
            // SAFETY: 刚刚已经确认 suffix 存在。
            unsafe { Some(haystack.get_unchecked(..i)) }
        } else {
            None
        }
    }

    #[inline]
    fn as_utf8_pattern(&self) -> Option<Utf8Pattern<'_>> {
        Some(Utf8Pattern::StringPattern(self.as_bytes()))
    }
}

/////////////////////////////////////////////////////////////////////////////
// Two-Way 子字符串搜索器。
/////////////////////////////////////////////////////////////////////////////

#[derive(Clone, Debug)]
/// `<&str as Pattern>::Searcher<'a>` 的关联类型。
pub struct StrSearcher<'a, 'b> {
    haystack: &'a str,
    needle: &'b str,

    searcher: StrSearcherImpl,
}

#[derive(Clone, Debug)]
enum StrSearcherImpl {
    Empty(EmptyNeedle),
    TwoWay(TwoWaySearcher),
}

#[derive(Clone, Debug)]
struct EmptyNeedle {
    position: usize,
    end: usize,
    is_match_fw: bool,
    is_match_bw: bool,
    // 空 haystack 的情况下需要该标记，见 #85462。
    is_finished: bool,
}

impl<'a, 'b> StrSearcher<'a, 'b> {
    fn new(haystack: &'a str, needle: &'b str) -> StrSearcher<'a, 'b> {
        if needle.is_empty() {
            StrSearcher {
                haystack,
                needle,
                searcher: StrSearcherImpl::Empty(EmptyNeedle {
                    position: 0,
                    end: haystack.len(),
                    is_match_fw: true,
                    is_match_bw: true,
                    is_finished: false,
                }),
            }
        } else {
            StrSearcher {
                haystack,
                needle,
                searcher: StrSearcherImpl::TwoWay(TwoWaySearcher::new(
                    needle.as_bytes(),
                    haystack.len(),
                )),
            }
        }
    }
}

unsafe impl<'a, 'b> Searcher<'a> for StrSearcher<'a, 'b> {
    #[inline]
    fn haystack(&self) -> &'a str {
        self.haystack
    }

    #[inline]
    fn next(&mut self) -> SearchStep {
        match self.searcher {
            StrSearcherImpl::Empty(ref mut searcher) => {
                if searcher.is_finished {
                    return SearchStep::Done;
                }
                // 空 needle 会拒绝每个 char，同时匹配它们之间的每个空字符串。
                let is_match = searcher.is_match_fw;
                searcher.is_match_fw = !searcher.is_match_fw;
                let pos = searcher.position;
                match self.haystack[pos..].chars().next() {
                    _ if is_match => SearchStep::Match(pos, pos),
                    None => {
                        searcher.is_finished = true;
                        SearchStep::Done
                    }
                    Some(ch) => {
                        searcher.position += ch.len_utf8();
                        SearchStep::Reject(pos, searcher.position)
                    }
                }
            }
            StrSearcherImpl::TwoWay(ref mut searcher) => {
                // 只要匹配逻辑正确且 haystack 与 needle 都是有效 UTF-8，
                // TwoWaySearcher 产生的 *Match* 索引就会位于字符边界上。
                // 算法产生的 *Reject* 可能落在任意字节索引上，因此这里会手动推进到
                // 下一个字符边界，保证返回给 Searcher 用户的范围对 UTF-8 是安全的。
                if searcher.position == self.haystack.len() {
                    return SearchStep::Done;
                }
                let is_long = searcher.memory == usize::MAX;
                match searcher.next::<RejectAndMatch>(
                    self.haystack.as_bytes(),
                    self.needle.as_bytes(),
                    is_long,
                ) {
                    SearchStep::Reject(a, mut b) => {
                        // 跳到下一个字符边界。
                        while !self.haystack.is_char_boundary(b) {
                            b += 1;
                        }
                        searcher.position = cmp::max(b, searcher.position);
                        SearchStep::Reject(a, b)
                    }
                    otherwise => otherwise,
                }
            }
        }
    }

    #[inline]
    fn next_match(&mut self) -> Option<(usize, usize)> {
        match self.searcher {
            StrSearcherImpl::Empty(..) => loop {
                match self.next() {
                    SearchStep::Match(a, b) => return Some((a, b)),
                    SearchStep::Done => return None,
                    SearchStep::Reject(..) => {}
                }
            },
            StrSearcherImpl::TwoWay(ref mut searcher) => {
                let is_long = searcher.memory == usize::MAX;
                // 显式写出 `true` 和 `false` 两种情况，鼓励编译器分别专门化。
                if is_long {
                    searcher.next::<MatchOnly>(
                        self.haystack.as_bytes(),
                        self.needle.as_bytes(),
                        true,
                    )
                } else {
                    searcher.next::<MatchOnly>(
                        self.haystack.as_bytes(),
                        self.needle.as_bytes(),
                        false,
                    )
                }
            }
        }
    }
}

unsafe impl<'a, 'b> ReverseSearcher<'a> for StrSearcher<'a, 'b> {
    #[inline]
    fn next_back(&mut self) -> SearchStep {
        match self.searcher {
            StrSearcherImpl::Empty(ref mut searcher) => {
                if searcher.is_finished {
                    return SearchStep::Done;
                }
                let is_match = searcher.is_match_bw;
                searcher.is_match_bw = !searcher.is_match_bw;
                let end = searcher.end;
                match self.haystack[..end].chars().next_back() {
                    _ if is_match => SearchStep::Match(end, end),
                    None => {
                        searcher.is_finished = true;
                        SearchStep::Done
                    }
                    Some(ch) => {
                        searcher.end -= ch.len_utf8();
                        SearchStep::Reject(searcher.end, end)
                    }
                }
            }
            StrSearcherImpl::TwoWay(ref mut searcher) => {
                if searcher.end == 0 {
                    return SearchStep::Done;
                }
                let is_long = searcher.memory == usize::MAX;
                match searcher.next_back::<RejectAndMatch>(
                    self.haystack.as_bytes(),
                    self.needle.as_bytes(),
                    is_long,
                ) {
                    SearchStep::Reject(mut a, b) => {
                        // 跳到前一个字符边界。
                        while !self.haystack.is_char_boundary(a) {
                            a -= 1;
                        }
                        searcher.end = cmp::min(a, searcher.end);
                        SearchStep::Reject(a, b)
                    }
                    otherwise => otherwise,
                }
            }
        }
    }

    #[inline]
    fn next_match_back(&mut self) -> Option<(usize, usize)> {
        match self.searcher {
            StrSearcherImpl::Empty(..) => loop {
                match self.next_back() {
                    SearchStep::Match(a, b) => return Some((a, b)),
                    SearchStep::Done => return None,
                    SearchStep::Reject(..) => {}
                }
            },
            StrSearcherImpl::TwoWay(ref mut searcher) => {
                let is_long = searcher.memory == usize::MAX;
                // 像 `next_match` 一样显式写出 `true` 和 `false`。
                if is_long {
                    searcher.next_back::<MatchOnly>(
                        self.haystack.as_bytes(),
                        self.needle.as_bytes(),
                        true,
                    )
                } else {
                    searcher.next_back::<MatchOnly>(
                        self.haystack.as_bytes(),
                        self.needle.as_bytes(),
                        false,
                    )
                }
            }
        }
    }
}

/// Two-Way 子字符串搜索算法的内部状态。
#[derive(Clone, Debug)]
struct TwoWaySearcher {
    // 常量状态。
    /// 临界分解索引。
    crit_pos: usize,
    /// 反向 needle 的临界分解索引。
    crit_pos_back: usize,
    period: usize,
    /// `byteset` 是扩展项（不是 Two-Way 算法本身的一部分）；
    /// 它是 64 位“指纹”，其中每个置位的 bit `j` 表示 needle 中存在
    /// 满足 `(byte & 63) == j` 的字节。
    byteset: u64,

    // 可变状态。
    position: usize,
    end: usize,
    /// needle 中的索引；该索引之前的内容已经匹配。
    memory: usize,
    /// needle 中的索引；该索引之后的内容已经匹配。
    memory_back: usize,
}

/*
    这里实现的是 Two-Way 搜索算法，该算法最早见于论文：
    Crochemore, M., Perrin, D., 1991, Two-way string-matching, Journal of the ACM 38(3):651-675.

    下面是一些背景定义。

    *word* 是由符号组成的串。word 的*长度*是通常意义上的长度，这里对任意 word x
    记为 |x|。（这里也允许*空 word*，即长度为零的 word。）

    如果 x 是任意非空 word，那么满足 0 < p <= |x| 的整数 p 被称为 x 的一个
    *period*，当且仅当对所有满足 0 <= i <= |x| - p - 1 的 i，都有
    x[i] == x[i+p]。例如，1 和 2 都是字符串 "aa" 的 period；而字符串 "abcd"
    唯一的 period 是 4。

    对非空 x，用 period(x) 表示 x 的*最小* period。这个值总是有定义的，因为每个
    非空 word x 至少有一个 period，也就是 |x|。有时也把它称为 x 的*周期*。

    如果 u、v 和 x 都是 word，且 x = uv，其中 uv 表示 u 和 v 的拼接，那么称
    (u, v) 是 x 的一个 *factorization*。

    设 (u, v) 是 word x 的一个 factorization。如果存在一个非空 word w，并且同时满足：

      - w 是 u 的后缀，或 u 是 w 的后缀
      - w 是 v 的前缀，或 v 是 w 的前缀

    则称 w 是该 factorization (u, v) 的一个 *repetition*。

    展开来看，这里有四种可能。令 w = "abc"，则可能出现：

      - w 是 u 的后缀，且 w 是 v 的前缀。例如：("lolabc", "abcde")
      - w 是 u 的后缀，且 v 是 w 的前缀。例如：("lolabc", "ab")
      - u 是 w 的后缀，且 w 是 v 的前缀。例如：("bc", "abchi")
      - u 是 w 的后缀，且 v 是 w 的前缀。例如：("bc", "a")

    注意，对于 x = uv 的任意 factorization (u, v)，word vu 都是一个 repetition，
    因此每个 factorization 至少有一个 repetition。

    如果 x 是字符串，且 (u, v) 是 x 的一个 factorization，那么 (u, v) 的
    *local period* 是某个整数 r，使得存在 word w 满足 |w| = r，且 w 是
    (u, v) 的 repetition。

    用 local_period(u, v) 表示 (u, v) 的最小 local period。有时也把它称为
    (u, v) 的*局部周期*。只要 x = uv 非空，这个值就是良定义的（因为如上所述，
    每个非空 word 至少有一个 factorization）。

    可以证明，对 factorization (u, v) 而言，local period 也可以等价地定义为满足以下
    条件的任意正整数 r：对所有满足 |u| - r <= i <= |u| - 1 且 x[i] 与 x[i+r]
    都有定义的 i，都有 x[i] == x[i+r]。（也就是 i > 0 且 i + r < |x|。）

    使用上述等价表述，很容易证明：

        1 <= local_period(u, v) <= period(uv)

    如果 x 的 factorization (u, v) 满足 local_period(u, v) = period(x)，则称其为
    *critical factorization*。

    该算法依赖如下定理，这里不展开证明：

    **Critical Factorization Theorem** 任意 word x 至少存在一个 critical
    factorization (u, v)，使得 |u| < period(x)。

    maximal_suffix 的目的就是找到这样的 critical factorization。

    如果 period 较短，则另外计算一个用于反向搜索的 factorization x = u' v'，
    并选择满足 |v'| < period(x) 的分解。

*/
impl TwoWaySearcher {
    fn new(needle: &[u8], end: usize) -> TwoWaySearcher {
        let (crit_pos_false, period_false) = TwoWaySearcher::maximal_suffix(needle, false);
        let (crit_pos_true, period_true) = TwoWaySearcher::maximal_suffix(needle, true);

        let (crit_pos, period) = if crit_pos_false > crit_pos_true {
            (crit_pos_false, period_false)
        } else {
            (crit_pos_true, period_true)
        };

        // Crochemore 和 Rytter 的《Text Algorithms》第 13 章对这里发生的事情有一段
        // 很清晰的解释，尤其可参见第 323 页的 "Algorithm CP" 代码。
        //
        // 这里已有 needle 的某个 critical factorization (u, v)，接下来要判断 u 是否是
        // &v[..period] 的后缀。如果是，使用 "Algorithm CP1"；否则使用针对 needle
        // period 较大的情况优化过的 "Algorithm CP2"。
        if needle[..crit_pos] == needle[period..period + crit_pos] {
            // 短 period 情况：period 是精确值。
            // 为反向 needle 单独计算 critical factorization：x = u' v'，
            // 其中 |v'| < period(x)。
            //
            // 已知 period 可以加速这个过程。注意，像 x = "acba" 这样的情况正向可能得到
            // 精确分解（crit_pos = 1, period = 3），但反向只能得到近似 period 的分解
            //（crit_pos = 2, period = 2）。这里使用给出的反向分解，但保留精确 period。
            let crit_pos_back = needle.len()
                - cmp::max(
                    TwoWaySearcher::reverse_maximal_suffix(needle, period, false),
                    TwoWaySearcher::reverse_maximal_suffix(needle, period, true),
                );

            TwoWaySearcher {
                crit_pos,
                crit_pos_back,
                period,
                byteset: Self::byteset_create(&needle[..period]),

                position: 0,
                end,
                memory: 0,
                memory_back: needle.len(),
            }
        } else {
            // 长 period 情况：这里只有实际 period 的近似值，并且不使用记忆化。
            //
            // 用下界 max(|u|, |v|) + 1 来近似 period。
            // 该 critical factorization 对正向和反向搜索都足够高效。

            TwoWaySearcher {
                crit_pos,
                crit_pos_back: crit_pos,
                period: cmp::max(crit_pos, needle.len() - crit_pos) + 1,
                byteset: Self::byteset_create(needle),

                position: 0,
                end,
                memory: usize::MAX, // 哑值，用来表示 period 较长。
                memory_back: usize::MAX,
            }
        }
    }

    #[inline]
    fn byteset_create(bytes: &[u8]) -> u64 {
        bytes.iter().fold(0, |a, &b| (1 << (b & 0x3f)) | a)
    }

    #[inline]
    fn byteset_contains(&self, byte: u8) -> bool {
        (self.byteset >> ((byte & 0x3f) as usize)) & 1 != 0
    }

    // Two-Way 的核心思路之一是把 needle 分解为两半 (u, v)，先从左到右扫描 haystack
    // 尝试寻找 v。如果 v 匹配，再从右到左尝试匹配 u。遇到不匹配时能跳多远，
    // 完全基于 (u, v) 是 needle 的 critical factorization 这一事实。
    #[inline]
    fn next<S>(&mut self, haystack: &[u8], needle: &[u8], long_period: bool) -> S::Output
    where
        S: TwoWayStrategy,
    {
        // `next()` 使用 `self.position` 作为游标。
        let old_pos = self.position;
        let needle_last = needle.len() - 1;
        'search: loop {
            // 检查剩余空间是否足够搜索。
            // 如果假设切片受 isize 范围限制，position + needle_last 就不会溢出。
            let tail_byte = match haystack.get(self.position + needle_last) {
                Some(&b) => b,
                None => {
                    self.position = haystack.len();
                    return S::rejecting(old_pos, self.position);
                }
            };

            if S::use_early_reject() && old_pos != self.position {
                return S::rejecting(old_pos, self.position);
            }

            // 快速跳过与目标子字符串无关的大块内容。
            if !self.byteset_contains(tail_byte) {
                self.position += needle.len();
                if !long_period {
                    self.memory = 0;
                }
                continue 'search;
            }

            // 检查 needle 右半部分是否匹配。
            let start =
                if long_period { self.crit_pos } else { cmp::max(self.crit_pos, self.memory) };
            for i in start..needle.len() {
                if needle[i] != haystack[self.position + i] {
                    self.position += i - self.crit_pos + 1;
                    if !long_period {
                        self.memory = 0;
                    }
                    continue 'search;
                }
            }

            // 检查 needle 左半部分是否匹配。
            let start = if long_period { 0 } else { self.memory };
            for i in (start..self.crit_pos).rev() {
                if needle[i] != haystack[self.position + i] {
                    self.position += self.period;
                    if !long_period {
                        self.memory = needle.len() - self.period;
                    }
                    continue 'search;
                }
            }

            // 找到了一个匹配。
            let match_pos = self.position;

            // 注意：如果要支持重叠匹配，应加 self.period 而不是 needle.len()。
            self.position += needle.len();
            if !long_period {
                self.memory = 0; // 如果要支持重叠匹配，则设置为 needle.len() - self.period。
            }

            return S::matching(match_pos, match_pos + needle.len());
        }
    }

    // 遵循 `next()` 中的思路。
    //
    // 这些定义是对称的：period(x) = period(reverse(x))，
    // local_period(u, v) = local_period(reverse(v), reverse(u))。因此如果 (u, v)
    // 是 critical factorization，那么 (reverse(v), reverse(u)) 也是。
    //
    // 对反向情况，已经计算了 critical factorization x = u' v'（字段 `crit_pos_back`）。
    // 正向搜索需要 |u| < period(x)，因此反向搜索需要 |v'| < period(x)。
    //
    // 要反向搜索 haystack，可以等价地在反转的 haystack 上用反转的 needle 做正向搜索，
    // 先匹配 u'，再匹配 v'。
    #[inline]
    fn next_back<S>(&mut self, haystack: &[u8], needle: &[u8], long_period: bool) -> S::Output
    where
        S: TwoWayStrategy,
    {
        // `next_back()` 使用 `self.end` 作为游标，使其与 `next()` 相互独立。
        let old_end = self.end;
        'search: loop {
            // 检查剩余空间是否足够搜索。
            // 当没有更多空间时，end - needle.len() 会回绕；但受切片长度限制，
            // 它不可能一路回绕到 haystack 的长度范围内。
            let front_byte = match haystack.get(self.end.wrapping_sub(needle.len())) {
                Some(&b) => b,
                None => {
                    self.end = 0;
                    return S::rejecting(0, old_end);
                }
            };

            if S::use_early_reject() && old_end != self.end {
                return S::rejecting(self.end, old_end);
            }

            // 快速跳过与目标子字符串无关的大块内容。
            if !self.byteset_contains(front_byte) {
                self.end -= needle.len();
                if !long_period {
                    self.memory_back = needle.len();
                }
                continue 'search;
            }

            // 检查 needle 左半部分是否匹配。
            let crit = if long_period {
                self.crit_pos_back
            } else {
                cmp::min(self.crit_pos_back, self.memory_back)
            };
            for i in (0..crit).rev() {
                if needle[i] != haystack[self.end - needle.len() + i] {
                    self.end -= self.crit_pos_back - i;
                    if !long_period {
                        self.memory_back = needle.len();
                    }
                    continue 'search;
                }
            }

            // 检查 needle 右半部分是否匹配。
            let needle_end = if long_period { needle.len() } else { self.memory_back };
            for i in self.crit_pos_back..needle_end {
                if needle[i] != haystack[self.end - needle.len() + i] {
                    self.end -= self.period;
                    if !long_period {
                        self.memory_back = self.period;
                    }
                    continue 'search;
                }
            }

            // 找到了一个匹配。
            let match_pos = self.end - needle.len();
            // 注意：如果要支持重叠匹配，应减 self.period 而不是 needle.len()。
            self.end -= needle.len();
            if !long_period {
                self.memory_back = needle.len();
            }

            return S::matching(match_pos, match_pos + needle.len());
        }
    }

    // 计算 `arr` 的 maximal suffix。
    //
    // maximal suffix 是 `arr` 的一个可能的 critical factorization (u, v)。
    //
    // 返回 (`i`, `p`)，其中 `i` 是 v 的起始索引，`p` 是 v 的 period。
    //
    // `order_greater` 决定词典序使用 `<` 还是 `>`。两种顺序都必须计算；
    // 产生最大 `i` 的顺序给出一个 critical factorization。
    //
    // 对长 period 情况，得到的 period 不是精确值（它偏短）。
    #[inline]
    fn maximal_suffix(arr: &[u8], order_greater: bool) -> (usize, usize) {
        let mut left = 0; // 对应论文中的 i。
        let mut right = 1; // 对应论文中的 j。
        let mut offset = 0; // 对应论文中的 k，但从 0 开始以匹配 0 基索引。
        let mut period = 1; // 对应论文中的 p。

        while let Some(&a) = arr.get(right + offset) {
            // 只要 `right` 在界内，`left` 也在界内。
            let b = arr[left + offset];
            if (a < b && !order_greater) || (a > b && order_greater) {
                // suffix 较小，period 是目前整个前缀。
                right += offset + 1;
                offset = 0;
                period = right - left;
            } else if a == b {
                // 沿着当前 period 的重复部分前进。
                if offset + 1 == period {
                    right += offset + 1;
                    offset = 0;
                } else {
                    offset += 1;
                }
            } else {
                // suffix 较大，从当前位置重新开始。
                left = right;
                right += 1;
                offset = 0;
                period = 1;
            }
        }
        (left, period)
    }

    // 计算 `arr` 反转后的 maximal suffix。
    //
    // maximal suffix 是 `arr` 的一个可能的 critical factorization (u', v')。
    //
    // 返回 `i`，其中 `i` 是从后向前看 v' 的起始索引；
    // 一旦达到 `known_period` 这个 period 就立即返回。
    //
    // `order_greater` 决定词典序使用 `<` 还是 `>`。两种顺序都必须计算；
    // 产生最大 `i` 的顺序给出一个 critical factorization。
    //
    // 对长 period 情况，得到的 period 不是精确值（它偏短）。
    fn reverse_maximal_suffix(arr: &[u8], known_period: usize, order_greater: bool) -> usize {
        let mut left = 0; // 对应论文中的 i。
        let mut right = 1; // 对应论文中的 j。
        let mut offset = 0; // 对应论文中的 k，但从 0 开始以匹配 0 基索引。
        let mut period = 1; // 对应论文中的 p。
        let n = arr.len();

        while right + offset < n {
            let a = arr[n - (1 + right + offset)];
            let b = arr[n - (1 + left + offset)];
            if (a < b && !order_greater) || (a > b && order_greater) {
                // suffix 较小，period 是目前整个前缀。
                right += offset + 1;
                offset = 0;
                period = right - left;
            } else if a == b {
                // 沿着当前 period 的重复部分前进。
                if offset + 1 == period {
                    right += offset + 1;
                    offset = 0;
                } else {
                    offset += 1;
                }
            } else {
                // suffix 较大，从当前位置重新开始。
                left = right;
                right += 1;
                offset = 0;
                period = 1;
            }
            if period == known_period {
                break;
            }
        }
        debug_assert!(period <= known_period);
        left
    }
}

// TwoWayStrategy 让算法可以在两种模式间切换：尽快跳过非匹配区间，或者较快地产生 Reject。
trait TwoWayStrategy {
    type Output;
    fn use_early_reject() -> bool;
    fn rejecting(a: usize, b: usize) -> Self::Output;
    fn matching(a: usize, b: usize) -> Self::Output;
}

/// 尽快跳到匹配区间。
enum MatchOnly {}

impl TwoWayStrategy for MatchOnly {
    type Output = Option<(usize, usize)>;

    #[inline]
    fn use_early_reject() -> bool {
        false
    }
    #[inline]
    fn rejecting(_a: usize, _b: usize) -> Self::Output {
        None
    }
    #[inline]
    fn matching(a: usize, b: usize) -> Self::Output {
        Some((a, b))
    }
}

/// 定期产生 Reject。
enum RejectAndMatch {}

impl TwoWayStrategy for RejectAndMatch {
    type Output = SearchStep;

    #[inline]
    fn use_early_reject() -> bool {
        true
    }
    #[inline]
    fn rejecting(a: usize, b: usize) -> Self::Output {
        SearchStep::Reject(a, b)
    }
    #[inline]
    fn matching(a: usize, b: usize) -> Self::Output {
        SearchStep::Match(a, b)
    }
}

/// 基于 Wojciech Muła 的 "SIMD-friendly algorithms for substring searching"[0]
/// 对短 needle 进行 SIMD 搜索。
///
/// 它每轮按向量宽度向前跳（而不是像 Two-Way 那样按 needle 长度跳）：先在整个向量宽度
/// 上探测 needle 的首字节和末字节，只有向量化探测显示可能匹配时，才做完整 needle 比较。
///
/// 由于 x86_64 基线只提供 SSE2，这里只使用 u8x16。如果未来 std 面向 x86-64-v3
/// 发布，或将该实现适配到其他平台，则应重新评估更宽的向量。
///
/// 类似地，在 LoongArch 上，128 位 LSX 向量扩展是基线，因此那里也使用 `u8x16`。
/// 对未来的 LoongArch 扩展（例如 LASX），可以考虑更宽的向量宽度。
///
/// 对小于“向量大小 + needle 长度”的 haystack，它会回退到朴素的 O(n*m) 搜索，
/// 因此该实现不应在更长 needle 上调用。
///
/// [0]: http://0x80.pl/articles/simd-strfind.html#sse-avx2
#[cfg(any(
    all(target_arch = "x86_64", target_feature = "sse2"),
    all(target_arch = "loongarch64", target_feature = "lsx")
))]
#[inline]
fn simd_contains(needle: &str, haystack: &str) -> Option<bool> {
    let needle = needle.as_bytes();
    let haystack = haystack.as_bytes();

    debug_assert!(needle.len() > 1);

    use crate::ops::BitAnd;
    use crate::simd::cmp::SimdPartialEq;
    use crate::simd::{mask8x16 as Mask, u8x16 as Block};

    let first_probe = needle[0];
    let last_byte_offset = needle.len() - 1;

    // 第二个向量使用的偏移。
    let second_probe_offset = if needle.len() == 2 {
        // 对 len=2 的 needle 永不提前放弃，因为探测会完整覆盖它们，不存在退化情况。
        1
    } else {
        // 尝试几个字节，以处理 needle 首字节和末字节相同的情况。
        let Some(second_probe_offset) =
            (needle.len().saturating_sub(4)..needle.len()).rfind(|&idx| needle[idx] != first_probe)
        else {
            // 如果找不到任何不同字节，则回退到其他搜索方法，否则可能遇到退化情况。
            return None;
        };
        second_probe_offset
    };

    // 如果 haystack 太小，容不下向量探测，则执行朴素搜索。
    if haystack.len() < Block::LEN + last_byte_offset {
        return Some(haystack.windows(needle.len()).any(|c| c == needle));
    }

    let first_probe: Block = Block::splat(first_probe);
    let second_probe: Block = Block::splat(needle[second_probe_offset]);
    // 首字节已经由外层循环检查；要确认匹配，只需比较剩余部分。
    let trimmed_needle = &needle[1..];

    // 这里的 #[cold] 对性能有实际影响，移除前需要跑基准。
    let check_mask = #[cold]
    |idx, mask: u16, skip: bool| -> bool {
        if skip {
            return false;
        }

        // 这里同样如此，优化器行为比较微妙。
        let mut mask = mask;

        while mask != 0 {
            let trailing = mask.trailing_zeros();
            let offset = idx + trailing as usize + 1;
            // SAFETY: mask 的 trailing zeroes 介于 0 到 15；这里额外跳过一个已比较的字节，
            // 然后取 trimmed_needle.len() 个字节。该范围位于外层循环定义的边界内。
            unsafe {
                let sub = haystack.get_unchecked(offset..).get_unchecked(..trimmed_needle.len());
                if small_slice_eq(sub, trimmed_needle) {
                    return true;
                }
            }
            mask &= !(1 << trailing);
        }
        false
    };

    let test_chunk = |idx| -> u16 {
        // SAFETY: 这里要求从 idx 开始至少有 LANES 个可读字节；
        // 这由循环范围保证（见下方注释）。
        let a: Block = unsafe { haystack.as_ptr().add(idx).cast::<Block>().read_unaligned() };
        // SAFETY: 这里要求从 idx 开始有 LANES + block_offset 个可读字节。
        let b: Block = unsafe {
            haystack.as_ptr().add(idx).add(second_probe_offset).cast::<Block>().read_unaligned()
        };
        let eq_first: Mask = a.simd_eq(first_probe);
        let eq_last: Mask = b.simd_eq(second_probe);
        let both = eq_first.bitand(eq_last);
        let mask = both.to_bitmask() as u16;

        mask
    };

    let mut i = 0;
    let mut result = false;
    // 循环条件必须确保有足够余量读取 LANE 个字节；
    // 不只当前索引如此，按 block_offset 偏移后的索引也必须如此。
    const UNROLL: usize = 4;
    while i + last_byte_offset + UNROLL * Block::LEN < haystack.len() && !result {
        let mut masks = [0u16; UNROLL];
        for j in 0..UNROLL {
            masks[j] = test_chunk(i + j * Block::LEN);
        }
        for j in 0..UNROLL {
            let mask = masks[j];
            if mask != 0 {
                result |= check_mask(i + j * Block::LEN, mask, result);
            }
        }
        i += UNROLL * Block::LEN;
    }
    while i + last_byte_offset + Block::LEN < haystack.len() && !result {
        let mask = test_chunk(i);
        if mask != 0 {
            result |= check_mask(i, mask, result);
        }
        i += Block::LEN;
    }

    // 处理无法纳入 LANES 大小步进的尾部。
    // 这里重复同样流程，但使用右对齐块而不是左对齐块。最后一个字节必须正好贴住字符串结尾，
    // 这样既不会漏掉单个字节，也不会越界读取。
    let i = haystack.len() - last_byte_offset - Block::LEN;
    let mask = test_chunk(i);
    if mask != 0 {
        result |= check_mask(i, mask, result);
    }

    Some(result)
}

/// 比较短切片是否相等。
///
/// 该实现避免调用 libc 的 memcmp。memcmp 因 SIMD 优化在长切片上更快，
/// 但会引入一次函数调用开销。
///
/// # 安全性(Safety）
///
/// 两个切片必须具有相同长度。
#[cfg(any(
    all(target_arch = "x86_64", target_feature = "sse2"),
    all(target_arch = "loongarch64", target_feature = "lsx")
))]
#[inline]
unsafe fn small_slice_eq(x: &[u8], y: &[u8]) -> bool {
    debug_assert_eq!(x.len(), y.len());
    // 该函数改编自：
    // https://github.com/BurntSushi/memchr/blob/8037d11b4357b0f07be2bb66dc2659d9cf28ad32/src/memmem/util.rs#L32

    // 如果字节数不足以每次加载 4 字节，则回退到朴素慢速版本。
    //
    // 潜在替代方案：可以使用 copy_nonoverlapping 加 mask 来替代循环，但需要跑基准。
    if x.len() < 4 {
        for (&b1, &b2) in x.iter().zip(y) {
            if b1 != b2 {
                return false;
            }
        }
        return true;
    }
    // 当有 4 个或更多字节要比较时，使用非对齐加载，每次按 4 字节块推进。
    //
    // 为什么是 4 字节加载而不是 8 字节加载？原因是这个特化版 memcmp 很可能用于很小的
    // needle。如果使用 8 字节加载，更多 memcmp 调用会落到上面的慢速分支。话虽如此，
    // 这只是一个假设，目前只有基准结果的弱支持；这里仍可能有改进空间。当前主要目标是
    // 优化延迟，而不是吞吐。

    // SAFETY: 通过上方条件可知 `px` 与 `py` 长度相同，因此 `px < pxend`
    // 蕴含 `py < pyend`。所以下面的循环中解引用 `px` 和 `py` 都是安全的。
    //
    // 此外，`pxend` 和 `pyend` 被设置到 `px` 与 `py` 实际结尾之前 4 字节的位置。
    // 因此循环外的最终解引用一定有效。（当长度不是 4 的倍数时，最终比较会与循环中
    // 最后一次比较发生重叠。）
    //
    // 最后，这里执行的是非对齐加载，因此无需担心对齐。
    unsafe {
        let (mut px, mut py) = (x.as_ptr(), y.as_ptr());
        let (pxend, pyend) = (px.add(x.len() - 4), py.add(y.len() - 4));
        while px < pxend {
            let vx = (px as *const u32).read_unaligned();
            let vy = (py as *const u32).read_unaligned();
            if vx != vy {
                return false;
            }
            px = px.add(4);
            py = py.add(4);
        }
        let vx = (pxend as *const u32).read_unaligned();
        let vy = (pyend as *const u32).read_unaligned();
        vx == vy
    }
}
