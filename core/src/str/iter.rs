//! `str` 方法使用的迭代器类型。

use super::pattern::{DoubleEndedSearcher, Pattern, ReverseSearcher, Searcher};
use super::validations::{next_code_point, next_code_point_reverse};
use super::{
    BytesIsNotEmpty, CharEscapeDebugContinue, CharEscapeDefault, CharEscapeUnicode,
    IsAsciiWhitespace, IsNotEmpty, IsWhitespace, LinesMap, UnsafeBytesToStr, from_utf8_unchecked,
};
use crate::fmt::{self, Write};
use crate::iter::{
    Chain, Copied, Filter, FlatMap, Flatten, FusedIterator, Map, TrustedLen, TrustedRandomAccess,
    TrustedRandomAccessNoCoerce,
};
use crate::num::NonZero;
use crate::ops::Try;
use crate::slice::{self, Split as SliceSplit};
use crate::{char as char_mod, option};

/// 遍历字符串切片中 [`char`] 的迭代器。
///
///
/// 该结构体由 [`str`] 上的 [`chars`] 方法创建；更多语义见该方法文档。
/// 由于 `str` 保证底层字节是合法 UTF-8，迭代器每次都能解码出合法 Unicode scalar value。
///
/// [`char`]: prim@char
/// [`chars`]: str::chars
#[derive(Clone)]
#[must_use = "iterators are lazy and do nothing unless consumed"]
#[stable(feature = "rust1", since = "1.0.0")]
pub struct Chars<'a> {
    pub(super) iter: slice::Iter<'a, u8>,
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<'a> Iterator for Chars<'a> {
    type Item = char;

    #[inline]
    fn next(&mut self) -> Option<char> {
        // SAFETY: `str` 不变量保证 `self.iter` 剩余字节是合法 UTF-8；
        // 解码得到的 `ch` 必然是合法 Unicode scalar value，可构造为 `char`。
        unsafe { next_code_point(&mut self.iter).map(|ch| char::from_u32_unchecked(ch)) }
    }

    #[inline]
    fn count(self) -> usize {
        super::count::count_chars(self.as_str())
    }

    #[inline]
    fn advance_by(&mut self, mut remainder: usize) -> Result<(), NonZero<usize>> {
        const CHUNK_SIZE: usize = 32;

        if remainder >= CHUNK_SIZE {
            let mut chunks = self.iter.as_slice().as_chunks::<CHUNK_SIZE>().0.iter();
            let mut bytes_skipped: usize = 0;

            while remainder > CHUNK_SIZE
                && let Some(chunk) = chunks.next()
            {
                bytes_skipped += CHUNK_SIZE;

                let mut start_bytes = [false; CHUNK_SIZE];

                for i in 0..CHUNK_SIZE {
                    start_bytes[i] = !super::validations::utf8_is_cont_byte(chunk[i]);
                }

                remainder -= start_bytes.into_iter().map(|i| i as u8).sum::<u8>() as usize;
            }

            // SAFETY: 刚才已经实际遍历过这些字节，说明它们存在；因此 `advance_by` 会成功。
            unsafe { self.iter.advance_by(bytes_skipped).unwrap_unchecked() };

            // 跳过尾随的 continuation byte，确保停在下一个字符边界上。
            while self.iter.len() > 0 {
                let b = self.iter.as_slice()[0];
                if !super::validations::utf8_is_cont_byte(b) {
                    break;
                }
                // SAFETY: 刚读取过该字节，说明它存在。
                unsafe { self.iter.advance_by(1).unwrap_unchecked() };
            }
        }

        while (remainder > 0) && (self.iter.len() > 0) {
            remainder -= 1;
            let b = self.iter.as_slice()[0];
            let slurp = super::validations::utf8_char_width(b);
            // SAFETY: UTF-8 有效性保证该字符所需的 continuation byte（若有）都存在。
            unsafe { self.iter.advance_by(slurp).unwrap_unchecked() };
        }

        NonZero::new(remainder).map_or(Ok(()), Err)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.iter.len();
        // `(len + 3)` 不会溢出，因为 `slice::Iter` 属于内存中的切片，
        // 切片最大长度为 `isize::MAX`，远小于 `usize::MAX`。
        (len.div_ceil(4), Some(len))
    }

    #[inline]
    fn last(mut self) -> Option<char> {
        // 不需要遍历整个字符串。
        self.next_back()
    }
}

#[stable(feature = "chars_debug_impl", since = "1.38.0")]
impl fmt::Debug for Chars<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Chars(")?;
        f.debug_list().entries(self.clone()).finish()?;
        write!(f, ")")?;
        Ok(())
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<'a> DoubleEndedIterator for Chars<'a> {
    #[inline]
    fn next_back(&mut self) -> Option<char> {
        // SAFETY: `str` 不变量保证 `self.iter` 剩余字节是合法 UTF-8；
        // 反向解码得到的 `ch` 必然是合法 Unicode scalar value。
        unsafe { next_code_point_reverse(&mut self.iter).map(|ch| char::from_u32_unchecked(ch)) }
    }
}

#[stable(feature = "fused", since = "1.26.0")]
impl FusedIterator for Chars<'_> {}

impl<'a> Chars<'a> {
    /// 将底层剩余数据视为原始数据的子切片。
    ///
    /// 返回值与原始切片具有相同生命周期；持有该借用时，迭代器本身仍可继续使用。
    ///
    /// # 示例
    ///
    /// ```
    /// let mut chars = "abc".chars();
    ///
    /// assert_eq!(chars.as_str(), "abc");
    /// chars.next();
    /// assert_eq!(chars.as_str(), "bc");
    /// chars.next();
    /// chars.next();
    /// assert_eq!(chars.as_str(), "");
    /// ```
    #[stable(feature = "iter_to_slice", since = "1.4.0")]
    #[must_use]
    #[inline]
    pub fn as_str(&self) -> &'a str {
        // SAFETY: `Chars` 只能由 `str` 创建，`str` 保证该剩余字节切片仍是合法 UTF-8。
        unsafe { from_utf8_unchecked(self.iter.as_slice()) }
    }
}

/// 遍历字符串切片中 [`char`] 及其字节位置的迭代器。
///
/// 该结构体由 [`str`] 上的 [`char_indices`] 方法创建；更多语义见该方法文档。
/// 返回的位置是字节索引，且总是位于 UTF-8 字符边界。
///
/// [`char`]: prim@char
/// [`char_indices`]: str::char_indices
#[derive(Clone, Debug)]
#[must_use = "iterators are lazy and do nothing unless consumed"]
#[stable(feature = "rust1", since = "1.0.0")]
pub struct CharIndices<'a> {
    pub(super) front_offset: usize,
    pub(super) iter: Chars<'a>,
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<'a> Iterator for CharIndices<'a> {
    type Item = (usize, char);

    #[inline]
    fn next(&mut self) -> Option<(usize, char)> {
        let pre_len = self.iter.iter.len();
        match self.iter.next() {
            None => None,
            Some(ch) => {
                let index = self.front_offset;
                let len = self.iter.iter.len();
                self.front_offset += pre_len - len;
                Some((index, ch))
            }
        }
    }

    #[inline]
    fn count(self) -> usize {
        self.iter.count()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }

    #[inline]
    fn last(mut self) -> Option<(usize, char)> {
        // 不需要遍历整个字符串。
        self.next_back()
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<'a> DoubleEndedIterator for CharIndices<'a> {
    #[inline]
    fn next_back(&mut self) -> Option<(usize, char)> {
        self.iter.next_back().map(|ch| {
            let index = self.front_offset + self.iter.iter.len();
            (index, ch)
        })
    }
}

#[stable(feature = "fused", since = "1.26.0")]
impl FusedIterator for CharIndices<'_> {}

impl<'a> CharIndices<'a> {
    /// 将底层剩余数据视为原始数据的子切片。
    ///
    /// 返回值与原始切片具有相同生命周期；持有该借用时，迭代器本身仍可继续使用。
    #[stable(feature = "iter_to_slice", since = "1.4.0")]
    #[must_use]
    #[inline]
    pub fn as_str(&self) -> &'a str {
        self.iter.as_str()
    }

    /// 返回下一个字符的字节位置；如果没有更多字符，则返回底层字符串长度。
    ///
    /// 这意味着只要迭代器尚未完全消费，返回值就会等于下一次调用
    /// [`next()`](Self::next) 时返回的索引。
    ///
    /// # 示例
    ///
    /// ```
    /// let mut chars = "a楽".char_indices();
    ///
    /// // 还没有调用过 `next()`，因此 `offset()` 返回字符串首字符的字节索引，恒为 0。
    /// assert_eq!(chars.offset(), 0);
    /// // 如预期一样，第一次调用 `next()` 也返回索引 0。
    /// assert_eq!(chars.next(), Some((0, 'a')));
    ///
    /// // 已调用过一次 `next()`，因此 `offset()` 返回第二个字符的字节索引……
    /// assert_eq!(chars.offset(), 1);
    /// // ……并与下一次调用 `next()` 返回的索引一致。
    /// assert_eq!(chars.next(), Some((1, '楽')));
    ///
    /// // 迭代器消费完后，`offset()` 返回字符串的字节长度。
    /// assert_eq!(chars.offset(), 4);
    /// assert_eq!(chars.next(), None);
    /// ```
    #[inline]
    #[must_use]
    #[stable(feature = "char_indices_offset", since = "1.82.0")]
    pub fn offset(&self) -> usize {
        self.front_offset
    }
}

/// 遍历字符串切片中字节的迭代器。
///
/// 该结构体由 [`str`] 上的 [`bytes`] 方法创建；更多语义见该方法文档。
/// 它按原始 UTF-8 字节产生元素，不解码为 `char`。
///
/// [`bytes`]: str::bytes
#[must_use = "iterators are lazy and do nothing unless consumed"]
#[stable(feature = "rust1", since = "1.0.0")]
#[derive(Clone, Debug)]
pub struct Bytes<'a>(pub(super) Copied<slice::Iter<'a, u8>>);

#[stable(feature = "rust1", since = "1.0.0")]
impl Iterator for Bytes<'_> {
    type Item = u8;

    #[inline]
    fn next(&mut self) -> Option<u8> {
        self.0.next()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }

    #[inline]
    fn count(self) -> usize {
        self.0.count()
    }

    #[inline]
    fn last(self) -> Option<Self::Item> {
        self.0.last()
    }

    #[inline]
    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        self.0.nth(n)
    }

    #[inline]
    fn all<F>(&mut self, f: F) -> bool
    where
        F: FnMut(Self::Item) -> bool,
    {
        self.0.all(f)
    }

    #[inline]
    fn any<F>(&mut self, f: F) -> bool
    where
        F: FnMut(Self::Item) -> bool,
    {
        self.0.any(f)
    }

    #[inline]
    fn find<P>(&mut self, predicate: P) -> Option<Self::Item>
    where
        P: FnMut(&Self::Item) -> bool,
    {
        self.0.find(predicate)
    }

    #[inline]
    fn position<P>(&mut self, predicate: P) -> Option<usize>
    where
        P: FnMut(Self::Item) -> bool,
    {
        self.0.position(predicate)
    }

    #[inline]
    fn rposition<P>(&mut self, predicate: P) -> Option<usize>
    where
        P: FnMut(Self::Item) -> bool,
    {
        self.0.rposition(predicate)
    }

    #[inline]
    unsafe fn __iterator_get_unchecked(&mut self, idx: usize) -> u8 {
        // SAFETY: 调用方必须维护 `Iterator::__iterator_get_unchecked` 的安全契约。
        unsafe { self.0.__iterator_get_unchecked(idx) }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl DoubleEndedIterator for Bytes<'_> {
    #[inline]
    fn next_back(&mut self) -> Option<u8> {
        self.0.next_back()
    }

    #[inline]
    fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
        self.0.nth_back(n)
    }

    #[inline]
    fn rfind<P>(&mut self, predicate: P) -> Option<Self::Item>
    where
        P: FnMut(&Self::Item) -> bool,
    {
        self.0.rfind(predicate)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl ExactSizeIterator for Bytes<'_> {
    #[inline]
    fn len(&self) -> usize {
        self.0.len()
    }

    #[inline]
    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[stable(feature = "fused", since = "1.26.0")]
impl FusedIterator for Bytes<'_> {}

#[unstable(feature = "trusted_len", issue = "37572")]
unsafe impl TrustedLen for Bytes<'_> {}

#[doc(hidden)]
#[unstable(feature = "trusted_random_access", issue = "none")]
unsafe impl TrustedRandomAccess for Bytes<'_> {}

#[doc(hidden)]
#[unstable(feature = "trusted_random_access", issue = "none")]
unsafe impl TrustedRandomAccessNoCoerce for Bytes<'_> {
    const MAY_HAVE_SIDE_EFFECT: bool = false;
}

/// 该宏为字符串 pattern API 中形如 X<'a, P> 的包装类型生成 Clone 实现。
macro_rules! derive_pattern_clone {
    (clone $t:ident with |$s:ident| $e:expr) => {
        impl<'a, P> Clone for $t<'a, P>
        where
            P: Pattern<Searcher<'a>: Clone>,
        {
            fn clone(&self) -> Self {
                let $s = self;
                $e
            }
        }
    };
}

/// 该宏生成两个公开迭代器结构体，它们包装一个使用 `Pattern` API 的私有内部迭代器。
///
/// 对所有 `P: Pattern`，会生成以下项目（省略泛型）：
///
/// struct $forward_iterator($internal_iterator);
/// struct $reverse_iterator($internal_iterator);
///
/// impl Iterator for $forward_iterator
/// { /* 内部实现最终会调用 Searcher::next_match() */ }
///
/// impl DoubleEndedIterator for $forward_iterator
///       where P::Searcher: DoubleEndedSearcher
/// { /* 内部实现最终会调用 Searcher::next_match_back() */ }
///
/// impl Iterator for $reverse_iterator
///       where P::Searcher: ReverseSearcher
/// { /* 内部实现最终会调用 Searcher::next_match_back() */ }
///
/// impl DoubleEndedIterator for $reverse_iterator
///       where P::Searcher: DoubleEndedSearcher
/// { /* 内部实现最终会调用 Searcher::next_match() */ }
///
/// 内部迭代器在宏外定义，通过把正向和反向迭代委托给 `pattern::Searcher`
/// 与 `pattern::ReverseSearcher`，语义上几乎等同于 DoubleEndedIterator。
///
/// 之所以说“几乎”，是因为同一个 `Pattern` 的 `Searcher` 和 `ReverseSearcher`
/// 可能返回不同元素；因此直接为内部类型实现 `DoubleEndedIterator` 会不正确。
/// 详情见 `str::pattern` 文档。
///
/// 不过，内部结构体仍表示可从任一端单向推进的迭代器；在某些 pattern 下也确实是合法双端迭代器。
/// 因此两个包装结构体会根据具体 pattern 类型分别实现 `Iterator` 和 `DoubleEndedIterator`，
/// 这正是上面复杂 impl 的来源。
macro_rules! generate_pattern_iterators {
    {
        // 正向迭代器。
        forward:
            $(#[$forward_iterator_attribute:meta])*
            struct $forward_iterator:ident;

        // 反向迭代器。
        reverse:
            $(#[$reverse_iterator_attribute:meta])*
            struct $reverse_iterator:ident;

        // 所有生成项的稳定性属性。
        stability:
            $(#[$common_stability_attribute:meta])*

        // 被委托的内部“近似迭代器”。
        internal:
            $internal_iterator:ident yielding ($iterty:ty);

        // 委托种类：单端或双端。
        delegate $($t:tt)*
    } => {
        $(#[$forward_iterator_attribute])*
        $(#[$common_stability_attribute])*
        pub struct $forward_iterator<'a, P: Pattern>(pub(super) $internal_iterator<'a, P>);

        $(#[$common_stability_attribute])*
        impl<'a, P> fmt::Debug for $forward_iterator<'a, P>
        where
            P: Pattern<Searcher<'a>: fmt::Debug>,
        {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_tuple(stringify!($forward_iterator))
                    .field(&self.0)
                    .finish()
            }
        }

        $(#[$common_stability_attribute])*
        impl<'a, P: Pattern> Iterator for $forward_iterator<'a, P> {
            type Item = $iterty;

            #[inline]
            fn next(&mut self) -> Option<$iterty> {
                self.0.next()
            }
        }

        $(#[$common_stability_attribute])*
        impl<'a, P> Clone for $forward_iterator<'a, P>
        where
            P: Pattern<Searcher<'a>: Clone>,
        {
            fn clone(&self) -> Self {
                $forward_iterator(self.0.clone())
            }
        }

        $(#[$reverse_iterator_attribute])*
        $(#[$common_stability_attribute])*
        pub struct $reverse_iterator<'a, P: Pattern>(pub(super) $internal_iterator<'a, P>);

        $(#[$common_stability_attribute])*
        impl<'a, P> fmt::Debug for $reverse_iterator<'a, P>
        where
            P: Pattern<Searcher<'a>: fmt::Debug>,
        {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_tuple(stringify!($reverse_iterator))
                    .field(&self.0)
                    .finish()
            }
        }

        $(#[$common_stability_attribute])*
        impl<'a, P> Iterator for $reverse_iterator<'a, P>
        where
            P: Pattern<Searcher<'a>: ReverseSearcher<'a>>,
        {
            type Item = $iterty;

            #[inline]
            fn next(&mut self) -> Option<$iterty> {
                self.0.next_back()
            }
        }

        $(#[$common_stability_attribute])*
        impl<'a, P> Clone for $reverse_iterator<'a, P>
        where
            P: Pattern<Searcher<'a>: Clone>,
        {
            fn clone(&self) -> Self {
                $reverse_iterator(self.0.clone())
            }
        }

        #[stable(feature = "fused", since = "1.26.0")]
        impl<'a, P: Pattern> FusedIterator for $forward_iterator<'a, P> {}

        #[stable(feature = "fused", since = "1.26.0")]
        impl<'a, P> FusedIterator for $reverse_iterator<'a, P>
        where
            P: Pattern<Searcher<'a>: ReverseSearcher<'a>>,
        {}

        generate_pattern_iterators!($($t)* with $(#[$common_stability_attribute])*,
                                                $forward_iterator,
                                                $reverse_iterator, $iterty);
    };
    {
        double ended; with $(#[$common_stability_attribute:meta])*,
                           $forward_iterator:ident,
                           $reverse_iterator:ident, $iterty:ty
    } => {
        $(#[$common_stability_attribute])*
        impl<'a, P> DoubleEndedIterator for $forward_iterator<'a, P>
        where
            P: Pattern<Searcher<'a>: DoubleEndedSearcher<'a>>,
        {
            #[inline]
            fn next_back(&mut self) -> Option<$iterty> {
                self.0.next_back()
            }
        }

        $(#[$common_stability_attribute])*
        impl<'a, P> DoubleEndedIterator for $reverse_iterator<'a, P>
        where
            P: Pattern<Searcher<'a>: DoubleEndedSearcher<'a>>,
        {
            #[inline]
            fn next_back(&mut self) -> Option<$iterty> {
                self.0.next()
            }
        }
    };
    {
        single ended; with $(#[$common_stability_attribute:meta])*,
                           $forward_iterator:ident,
                           $reverse_iterator:ident, $iterty:ty
    } => {}
}

derive_pattern_clone! {
    clone SplitInternal
    with |s| SplitInternal { matcher: s.matcher.clone(), ..*s }
}

pub(super) struct SplitInternal<'a, P: Pattern> {
    pub(super) start: usize,
    pub(super) end: usize,
    pub(super) matcher: P::Searcher<'a>,
    pub(super) allow_trailing_empty: bool,
    pub(super) finished: bool,
}

impl<'a, P> fmt::Debug for SplitInternal<'a, P>
where
    P: Pattern<Searcher<'a>: fmt::Debug>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SplitInternal")
            .field("start", &self.start)
            .field("end", &self.end)
            .field("matcher", &self.matcher)
            .field("allow_trailing_empty", &self.allow_trailing_empty)
            .field("finished", &self.finished)
            .finish()
    }
}

impl<'a, P: Pattern> SplitInternal<'a, P> {
    #[inline]
    fn get_end(&mut self) -> Option<&'a str> {
        if !self.finished {
            self.finished = true;

            if self.allow_trailing_empty || self.end - self.start > 0 {
                // SAFETY: `self.start` 和 `self.end` 始终位于 UTF-8 字符边界上。
                let string = unsafe { self.matcher.haystack().get_unchecked(self.start..self.end) };
                return Some(string);
            }
        }

        None
    }

    #[inline]
    fn next(&mut self) -> Option<&'a str> {
        if self.finished {
            return None;
        }

        let haystack = self.matcher.haystack();
        match self.matcher.next_match() {
            // SAFETY: `Searcher` 保证 `a` 和 `b` 都位于 UTF-8 字符边界上。
            Some((a, b)) => unsafe {
                let elt = haystack.get_unchecked(self.start..a);
                self.start = b;
                Some(elt)
            },
            None => self.get_end(),
        }
    }

    #[inline]
    fn next_inclusive(&mut self) -> Option<&'a str> {
        if self.finished {
            return None;
        }

        let haystack = self.matcher.haystack();
        match self.matcher.next_match() {
            // SAFETY: `Searcher` 保证 `b` 位于 UTF-8 字符边界上；
            // `self.start` 要么是原始字符串开头，要么曾被赋值为某个 `b`，因此同样位于边界。
            Some((_, b)) => unsafe {
                let elt = haystack.get_unchecked(self.start..b);
                self.start = b;
                Some(elt)
            },
            None => self.get_end(),
        }
    }

    #[inline]
    fn next_back(&mut self) -> Option<&'a str>
    where
        P::Searcher<'a>: ReverseSearcher<'a>,
    {
        if self.finished {
            return None;
        }

        if !self.allow_trailing_empty {
            self.allow_trailing_empty = true;
            match self.next_back() {
                Some(elt) if !elt.is_empty() => return Some(elt),
                _ => {
                    if self.finished {
                        return None;
                    }
                }
            }
        }

        let haystack = self.matcher.haystack();
        match self.matcher.next_match_back() {
            // SAFETY: `Searcher` 保证 `a` 和 `b` 都位于 UTF-8 字符边界上。
            Some((a, b)) => unsafe {
                let elt = haystack.get_unchecked(b..self.end);
                self.end = a;
                Some(elt)
            },
            // SAFETY: `self.start` 和 `self.end` 始终位于 UTF-8 字符边界上。
            None => unsafe {
                self.finished = true;
                Some(haystack.get_unchecked(self.start..self.end))
            },
        }
    }

    #[inline]
    fn next_back_inclusive(&mut self) -> Option<&'a str>
    where
        P::Searcher<'a>: ReverseSearcher<'a>,
    {
        if self.finished {
            return None;
        }

        if !self.allow_trailing_empty {
            self.allow_trailing_empty = true;
            match self.next_back_inclusive() {
                Some(elt) if !elt.is_empty() => return Some(elt),
                _ => {
                    if self.finished {
                        return None;
                    }
                }
            }
        }

        let haystack = self.matcher.haystack();
        match self.matcher.next_match_back() {
            // SAFETY: `Searcher` 保证 `b` 位于 UTF-8 字符边界上；
            // `self.end` 要么是原始字符串末尾，要么曾被赋值为某个 `b`，因此同样位于边界。
            Some((_, b)) => unsafe {
                let elt = haystack.get_unchecked(b..self.end);
                self.end = b;
                Some(elt)
            },
            // SAFETY: `self.start` 要么是原始字符串开头，要么是尚未迭代部分的子串起点；
            // 两种情况下都保证位于 UTF-8 字符边界上。`self.end` 要么是原始字符串末尾，
            // 要么曾被赋值为某个 `b`，因此也位于字符边界上。
            None => unsafe {
                self.finished = true;
                Some(haystack.get_unchecked(self.start..self.end))
            },
        }
    }

    #[inline]
    fn remainder(&self) -> Option<&'a str> {
        // `Self::get_end` 不会改变 `self.start`。
        if self.finished {
            return None;
        }

        // SAFETY: `self.start` 和 `self.end` 始终位于 UTF-8 字符边界上。
        Some(unsafe { self.matcher.haystack().get_unchecked(self.start..self.end) })
    }
}

generate_pattern_iterators! {
    forward:
        /// 由 [`split`] 方法创建。
        ///
        /// [`split`]: str::split
        struct Split;
    reverse:
        /// 由 [`rsplit`] 方法创建。
        ///
        /// [`rsplit`]: str::rsplit
        struct RSplit;
    stability:
        #[stable(feature = "rust1", since = "1.0.0")]
    internal:
        SplitInternal yielding (&'a str);
    delegate double ended;
}

impl<'a, P: Pattern> Split<'a, P> {
    /// 返回被分割字符串中尚未迭代的剩余部分。
    ///
    /// 如果迭代器已为空，则返回 `None`。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(str_split_remainder)]
    /// let mut split = "Mary had a little lamb".split(' ');
    /// assert_eq!(split.remainder(), Some("Mary had a little lamb"));
    /// split.next();
    /// assert_eq!(split.remainder(), Some("had a little lamb"));
    /// split.by_ref().for_each(drop);
    /// assert_eq!(split.remainder(), None);
    /// ```
    #[inline]
    #[unstable(feature = "str_split_remainder", issue = "77998")]
    pub fn remainder(&self) -> Option<&'a str> {
        self.0.remainder()
    }
}

impl<'a, P: Pattern> RSplit<'a, P> {
    /// 返回被分割字符串中尚未反向迭代的剩余部分。
    ///
    /// 如果迭代器已为空，则返回 `None`。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(str_split_remainder)]
    /// let mut split = "Mary had a little lamb".rsplit(' ');
    /// assert_eq!(split.remainder(), Some("Mary had a little lamb"));
    /// split.next();
    /// assert_eq!(split.remainder(), Some("Mary had a little"));
    /// split.by_ref().for_each(drop);
    /// assert_eq!(split.remainder(), None);
    /// ```
    #[inline]
    #[unstable(feature = "str_split_remainder", issue = "77998")]
    pub fn remainder(&self) -> Option<&'a str> {
        self.0.remainder()
    }
}

generate_pattern_iterators! {
    forward:
        /// 由 [`split_terminator`] 方法创建。
        ///
        /// [`split_terminator`]: str::split_terminator
        struct SplitTerminator;
    reverse:
        /// 由 [`rsplit_terminator`] 方法创建。
        ///
        /// [`rsplit_terminator`]: str::rsplit_terminator
        struct RSplitTerminator;
    stability:
        #[stable(feature = "rust1", since = "1.0.0")]
    internal:
        SplitInternal yielding (&'a str);
    delegate double ended;
}

impl<'a, P: Pattern> SplitTerminator<'a, P> {
    /// 返回被分割字符串中尚未迭代的剩余部分。
    ///
    /// 如果迭代器已为空，则返回 `None`。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(str_split_remainder)]
    /// let mut split = "A..B..".split_terminator('.');
    /// assert_eq!(split.remainder(), Some("A..B.."));
    /// split.next();
    /// assert_eq!(split.remainder(), Some(".B.."));
    /// split.by_ref().for_each(drop);
    /// assert_eq!(split.remainder(), None);
    /// ```
    #[inline]
    #[unstable(feature = "str_split_remainder", issue = "77998")]
    pub fn remainder(&self) -> Option<&'a str> {
        self.0.remainder()
    }
}

impl<'a, P: Pattern> RSplitTerminator<'a, P> {
    /// 返回被分割字符串中尚未反向迭代的剩余部分。
    ///
    /// 如果迭代器已为空，则返回 `None`。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(str_split_remainder)]
    /// let mut split = "A..B..".rsplit_terminator('.');
    /// assert_eq!(split.remainder(), Some("A..B.."));
    /// split.next();
    /// assert_eq!(split.remainder(), Some("A..B"));
    /// split.by_ref().for_each(drop);
    /// assert_eq!(split.remainder(), None);
    /// ```
    #[inline]
    #[unstable(feature = "str_split_remainder", issue = "77998")]
    pub fn remainder(&self) -> Option<&'a str> {
        self.0.remainder()
    }
}

derive_pattern_clone! {
    clone SplitNInternal
    with |s| SplitNInternal { iter: s.iter.clone(), ..*s }
}

pub(super) struct SplitNInternal<'a, P: Pattern> {
    pub(super) iter: SplitInternal<'a, P>,
    /// 剩余可执行的分割次数。
    pub(super) count: usize,
}

impl<'a, P> fmt::Debug for SplitNInternal<'a, P>
where
    P: Pattern<Searcher<'a>: fmt::Debug>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SplitNInternal")
            .field("iter", &self.iter)
            .field("count", &self.count)
            .finish()
    }
}

impl<'a, P: Pattern> SplitNInternal<'a, P> {
    #[inline]
    fn next(&mut self) -> Option<&'a str> {
        match self.count {
            0 => None,
            1 => {
                self.count = 0;
                self.iter.get_end()
            }
            _ => {
                self.count -= 1;
                self.iter.next()
            }
        }
    }

    #[inline]
    fn next_back(&mut self) -> Option<&'a str>
    where
        P::Searcher<'a>: ReverseSearcher<'a>,
    {
        match self.count {
            0 => None,
            1 => {
                self.count = 0;
                self.iter.get_end()
            }
            _ => {
                self.count -= 1;
                self.iter.next_back()
            }
        }
    }

    #[inline]
    fn remainder(&self) -> Option<&'a str> {
        self.iter.remainder()
    }
}

generate_pattern_iterators! {
    forward:
        /// 由 [`splitn`] 方法创建。
        ///
        /// [`splitn`]: str::splitn
        struct SplitN;
    reverse:
        /// 由 [`rsplitn`] 方法创建。
        ///
        /// [`rsplitn`]: str::rsplitn
        struct RSplitN;
    stability:
        #[stable(feature = "rust1", since = "1.0.0")]
    internal:
        SplitNInternal yielding (&'a str);
    delegate single ended;
}

impl<'a, P: Pattern> SplitN<'a, P> {
    /// 返回被分割字符串中尚未迭代的剩余部分。
    ///
    /// 如果迭代器已为空，则返回 `None`。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(str_split_remainder)]
    /// let mut split = "Mary had a little lamb".splitn(3, ' ');
    /// assert_eq!(split.remainder(), Some("Mary had a little lamb"));
    /// split.next();
    /// assert_eq!(split.remainder(), Some("had a little lamb"));
    /// split.by_ref().for_each(drop);
    /// assert_eq!(split.remainder(), None);
    /// ```
    #[inline]
    #[unstable(feature = "str_split_remainder", issue = "77998")]
    pub fn remainder(&self) -> Option<&'a str> {
        self.0.remainder()
    }
}

impl<'a, P: Pattern> RSplitN<'a, P> {
    /// 返回被分割字符串中尚未反向迭代的剩余部分。
    ///
    /// 如果迭代器已为空，则返回 `None`。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(str_split_remainder)]
    /// let mut split = "Mary had a little lamb".rsplitn(3, ' ');
    /// assert_eq!(split.remainder(), Some("Mary had a little lamb"));
    /// split.next();
    /// assert_eq!(split.remainder(), Some("Mary had a little"));
    /// split.by_ref().for_each(drop);
    /// assert_eq!(split.remainder(), None);
    /// ```
    #[inline]
    #[unstable(feature = "str_split_remainder", issue = "77998")]
    pub fn remainder(&self) -> Option<&'a str> {
        self.0.remainder()
    }
}

derive_pattern_clone! {
    clone MatchIndicesInternal
    with |s| MatchIndicesInternal(s.0.clone())
}

pub(super) struct MatchIndicesInternal<'a, P: Pattern>(pub(super) P::Searcher<'a>);

impl<'a, P> fmt::Debug for MatchIndicesInternal<'a, P>
where
    P: Pattern<Searcher<'a>: fmt::Debug>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("MatchIndicesInternal").field(&self.0).finish()
    }
}

impl<'a, P: Pattern> MatchIndicesInternal<'a, P> {
    #[inline]
    fn next(&mut self) -> Option<(usize, &'a str)> {
        self.0
            .next_match()
            // SAFETY: `Searcher` 保证 `start` 和 `end` 都位于 UTF-8 字符边界上。
            .map(|(start, end)| unsafe { (start, self.0.haystack().get_unchecked(start..end)) })
    }

    #[inline]
    fn next_back(&mut self) -> Option<(usize, &'a str)>
    where
        P::Searcher<'a>: ReverseSearcher<'a>,
    {
        self.0
            .next_match_back()
            // SAFETY: `Searcher` 保证 `start` 和 `end` 都位于 UTF-8 字符边界上。
            .map(|(start, end)| unsafe { (start, self.0.haystack().get_unchecked(start..end)) })
    }
}

generate_pattern_iterators! {
    forward:
        /// 由 [`match_indices`] 方法创建。
        ///
        /// [`match_indices`]: str::match_indices
        struct MatchIndices;
    reverse:
        /// 由 [`rmatch_indices`] 方法创建。
        ///
        /// [`rmatch_indices`]: str::rmatch_indices
        struct RMatchIndices;
    stability:
        #[stable(feature = "str_match_indices", since = "1.5.0")]
    internal:
        MatchIndicesInternal yielding ((usize, &'a str));
    delegate double ended;
}

derive_pattern_clone! {
    clone MatchesInternal
    with |s| MatchesInternal(s.0.clone())
}

pub(super) struct MatchesInternal<'a, P: Pattern>(pub(super) P::Searcher<'a>);

impl<'a, P> fmt::Debug for MatchesInternal<'a, P>
where
    P: Pattern<Searcher<'a>: fmt::Debug>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("MatchesInternal").field(&self.0).finish()
    }
}

impl<'a, P: Pattern> MatchesInternal<'a, P> {
    #[inline]
    fn next(&mut self) -> Option<&'a str> {
        // SAFETY: `Searcher` 保证 `start` 和 `end` 都位于 UTF-8 字符边界上。
        self.0.next_match().map(|(a, b)| unsafe {
            // 索引已知位于 UTF-8 字符边界上。
            self.0.haystack().get_unchecked(a..b)
        })
    }

    #[inline]
    fn next_back(&mut self) -> Option<&'a str>
    where
        P::Searcher<'a>: ReverseSearcher<'a>,
    {
        // SAFETY: `Searcher` 保证 `start` 和 `end` 都位于 UTF-8 字符边界上。
        self.0.next_match_back().map(|(a, b)| unsafe {
            // 索引已知位于 UTF-8 字符边界上。
            self.0.haystack().get_unchecked(a..b)
        })
    }
}

generate_pattern_iterators! {
    forward:
        /// 由 [`matches`] 方法创建。
        ///
        /// [`matches`]: str::matches
        struct Matches;
    reverse:
        /// 由 [`rmatches`] 方法创建。
        ///
        /// [`rmatches`]: str::rmatches
        struct RMatches;
    stability:
        #[stable(feature = "str_matches", since = "1.2.0")]
    internal:
        MatchesInternal yielding (&'a str);
    delegate double ended;
}

/// 以字符串切片形式遍历字符串中各行的迭代器。
///
/// 该结构体由 [`str`] 上的 [`lines`] 方法创建；更多语义见该方法文档。
///
/// [`lines`]: str::lines
#[stable(feature = "rust1", since = "1.0.0")]
#[must_use = "iterators are lazy and do nothing unless consumed"]
#[derive(Clone, Debug)]
pub struct Lines<'a>(pub(super) Map<SplitInclusive<'a, char>, LinesMap>);

#[stable(feature = "rust1", since = "1.0.0")]
impl<'a> Iterator for Lines<'a> {
    type Item = &'a str;

    #[inline]
    fn next(&mut self) -> Option<&'a str> {
        self.0.next()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }

    #[inline]
    fn last(mut self) -> Option<&'a str> {
        self.next_back()
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<'a> DoubleEndedIterator for Lines<'a> {
    #[inline]
    fn next_back(&mut self) -> Option<&'a str> {
        self.0.next_back()
    }
}

#[stable(feature = "fused", since = "1.26.0")]
impl FusedIterator for Lines<'_> {}

impl<'a> Lines<'a> {
    /// 返回按行分割后尚未迭代的剩余字符串。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(str_lines_remainder)]
    ///
    /// let mut lines = "a\nb\nc\nd".lines();
    /// assert_eq!(lines.remainder(), Some("a\nb\nc\nd"));
    ///
    /// lines.next();
    /// assert_eq!(lines.remainder(), Some("b\nc\nd"));
    ///
    /// lines.by_ref().for_each(drop);
    /// assert_eq!(lines.remainder(), None);
    /// ```
    #[inline]
    #[must_use]
    #[unstable(feature = "str_lines_remainder", issue = "77998")]
    pub fn remainder(&self) -> Option<&'a str> {
        self.0.iter.remainder()
    }
}

/// 由 [`lines_any`] 方法创建。
///
/// [`lines_any`]: str::lines_any
#[stable(feature = "rust1", since = "1.0.0")]
#[deprecated(since = "1.4.0", note = "use lines()/Lines instead now")]
#[must_use = "iterators are lazy and do nothing unless consumed"]
#[derive(Clone, Debug)]
#[allow(deprecated)]
pub struct LinesAny<'a>(pub(super) Lines<'a>);

#[stable(feature = "rust1", since = "1.0.0")]
#[allow(deprecated)]
impl<'a> Iterator for LinesAny<'a> {
    type Item = &'a str;

    #[inline]
    fn next(&mut self) -> Option<&'a str> {
        self.0.next()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
#[allow(deprecated)]
impl<'a> DoubleEndedIterator for LinesAny<'a> {
    #[inline]
    fn next_back(&mut self) -> Option<&'a str> {
        self.0.next_back()
    }
}

#[stable(feature = "fused", since = "1.26.0")]
#[allow(deprecated)]
impl FusedIterator for LinesAny<'_> {}

/// 遍历字符串中非 whitespace 子串的迭代器，子串之间可由任意数量的 whitespace 分隔。
///
/// 该结构体由 [`str`] 上的 [`split_whitespace`] 方法创建；更多语义见该方法文档。
///
/// [`split_whitespace`]: str::split_whitespace
#[stable(feature = "split_whitespace", since = "1.1.0")]
#[derive(Clone, Debug)]
pub struct SplitWhitespace<'a> {
    pub(super) inner: Filter<Split<'a, IsWhitespace>, IsNotEmpty>,
}

/// 遍历字符串中非 ASCII whitespace 子串的迭代器，子串之间可由任意数量的 ASCII whitespace 分隔。
///
/// 该结构体由 [`str`] 上的 [`split_ascii_whitespace`] 方法创建；更多语义见该方法文档。
///
/// [`split_ascii_whitespace`]: str::split_ascii_whitespace
#[stable(feature = "split_ascii_whitespace", since = "1.34.0")]
#[derive(Clone, Debug)]
pub struct SplitAsciiWhitespace<'a> {
    pub(super) inner:
        Map<Filter<SliceSplit<'a, u8, IsAsciiWhitespace>, BytesIsNotEmpty>, UnsafeBytesToStr>,
}

/// 遍历字符串中各子串的迭代器，每个子串以匹配 predicate 的片段结尾。
///
/// 与 `Split` 不同，它会把匹配到的分隔部分也包含在返回子切片的结尾。
///
/// 该结构体由 [`str`] 上的 [`split_inclusive`] 方法创建；更多语义见该方法文档。
///
/// [`split_inclusive`]: str::split_inclusive
#[stable(feature = "split_inclusive", since = "1.51.0")]
pub struct SplitInclusive<'a, P: Pattern>(pub(super) SplitInternal<'a, P>);

#[stable(feature = "split_whitespace", since = "1.1.0")]
impl<'a> Iterator for SplitWhitespace<'a> {
    type Item = &'a str;

    #[inline]
    fn next(&mut self) -> Option<&'a str> {
        self.inner.next()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }

    #[inline]
    fn last(mut self) -> Option<&'a str> {
        self.next_back()
    }
}

#[stable(feature = "split_whitespace", since = "1.1.0")]
impl<'a> DoubleEndedIterator for SplitWhitespace<'a> {
    #[inline]
    fn next_back(&mut self) -> Option<&'a str> {
        self.inner.next_back()
    }
}

#[stable(feature = "fused", since = "1.26.0")]
impl FusedIterator for SplitWhitespace<'_> {}

impl<'a> SplitWhitespace<'a> {
    /// 返回按 whitespace 分割后尚未迭代的剩余字符串。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(str_split_whitespace_remainder)]
    ///
    /// let mut split = "Mary had a little lamb".split_whitespace();
    /// assert_eq!(split.remainder(), Some("Mary had a little lamb"));
    ///
    /// split.next();
    /// assert_eq!(split.remainder(), Some("had a little lamb"));
    ///
    /// split.by_ref().for_each(drop);
    /// assert_eq!(split.remainder(), None);
    /// ```
    #[inline]
    #[must_use]
    #[unstable(feature = "str_split_whitespace_remainder", issue = "77998")]
    pub fn remainder(&self) -> Option<&'a str> {
        self.inner.iter.remainder()
    }
}

#[stable(feature = "split_ascii_whitespace", since = "1.34.0")]
impl<'a> Iterator for SplitAsciiWhitespace<'a> {
    type Item = &'a str;

    #[inline]
    fn next(&mut self) -> Option<&'a str> {
        self.inner.next()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }

    #[inline]
    fn last(mut self) -> Option<&'a str> {
        self.next_back()
    }
}

#[stable(feature = "split_ascii_whitespace", since = "1.34.0")]
impl<'a> DoubleEndedIterator for SplitAsciiWhitespace<'a> {
    #[inline]
    fn next_back(&mut self) -> Option<&'a str> {
        self.inner.next_back()
    }
}

#[stable(feature = "split_ascii_whitespace", since = "1.34.0")]
impl FusedIterator for SplitAsciiWhitespace<'_> {}

impl<'a> SplitAsciiWhitespace<'a> {
    /// 返回按 ASCII whitespace 分割后尚未迭代的剩余字符串。
    ///
    /// 如果迭代器已为空，则返回 `None`。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(str_split_whitespace_remainder)]
    ///
    /// let mut split = "Mary had a little lamb".split_ascii_whitespace();
    /// assert_eq!(split.remainder(), Some("Mary had a little lamb"));
    ///
    /// split.next();
    /// assert_eq!(split.remainder(), Some("had a little lamb"));
    ///
    /// split.by_ref().for_each(drop);
    /// assert_eq!(split.remainder(), None);
    /// ```
    #[inline]
    #[must_use]
    #[unstable(feature = "str_split_whitespace_remainder", issue = "77998")]
    pub fn remainder(&self) -> Option<&'a str> {
        if self.inner.iter.iter.finished {
            return None;
        }

        // SAFETY: 该字节切片来源于 `str`，因此仍是合法 UTF-8。
        Some(unsafe { crate::str::from_utf8_unchecked(&self.inner.iter.iter.v) })
    }
}

#[stable(feature = "split_inclusive", since = "1.51.0")]
impl<'a, P: Pattern> Iterator for SplitInclusive<'a, P> {
    type Item = &'a str;

    #[inline]
    fn next(&mut self) -> Option<&'a str> {
        self.0.next_inclusive()
    }
}

#[stable(feature = "split_inclusive", since = "1.51.0")]
impl<'a, P: Pattern<Searcher<'a>: fmt::Debug>> fmt::Debug for SplitInclusive<'a, P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SplitInclusive").field("0", &self.0).finish()
    }
}

// FIXME(#26925): 改用 `#[derive(Clone)]` 后移除该手写实现。
#[stable(feature = "split_inclusive", since = "1.51.0")]
impl<'a, P: Pattern<Searcher<'a>: Clone>> Clone for SplitInclusive<'a, P> {
    fn clone(&self) -> Self {
        SplitInclusive(self.0.clone())
    }
}

#[stable(feature = "split_inclusive", since = "1.51.0")]
impl<'a, P: Pattern<Searcher<'a>: DoubleEndedSearcher<'a>>> DoubleEndedIterator
    for SplitInclusive<'a, P>
{
    #[inline]
    fn next_back(&mut self) -> Option<&'a str> {
        self.0.next_back_inclusive()
    }
}

#[stable(feature = "split_inclusive", since = "1.51.0")]
impl<'a, P: Pattern> FusedIterator for SplitInclusive<'a, P> {}

impl<'a, P: Pattern> SplitInclusive<'a, P> {
    /// 返回 inclusive split 后尚未迭代的剩余字符串。
    ///
    /// 如果迭代器已为空，则返回 `None`。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(str_split_inclusive_remainder)]
    /// let mut split = "Mary had a little lamb".split_inclusive(' ');
    /// assert_eq!(split.remainder(), Some("Mary had a little lamb"));
    /// split.next();
    /// assert_eq!(split.remainder(), Some("had a little lamb"));
    /// split.by_ref().for_each(drop);
    /// assert_eq!(split.remainder(), None);
    /// ```
    #[inline]
    #[unstable(feature = "str_split_inclusive_remainder", issue = "77998")]
    pub fn remainder(&self) -> Option<&'a str> {
        self.0.remainder()
    }
}

/// 遍历字符串按 UTF-16 编码后得到的 [`u16`] code unit 的迭代器。
///
/// 该结构体由 [`str`] 上的 [`encode_utf16`] 方法创建；更多语义见该方法文档。
/// 由于 `str` 是合法 UTF-8，迭代出的 `char` 都是 Unicode scalar value；
/// 编码为 UTF-16 时，补充平面字符会产生代理对。
///
/// [`encode_utf16`]: str::encode_utf16
#[derive(Clone)]
#[stable(feature = "encode_utf16", since = "1.8.0")]
pub struct EncodeUtf16<'a> {
    pub(super) chars: Chars<'a>,
    pub(super) extra: u16,
}

#[stable(feature = "collection_debug", since = "1.17.0")]
impl fmt::Debug for EncodeUtf16<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EncodeUtf16").finish_non_exhaustive()
    }
}

#[stable(feature = "encode_utf16", since = "1.8.0")]
impl<'a> Iterator for EncodeUtf16<'a> {
    type Item = u16;

    #[inline]
    fn next(&mut self) -> Option<u16> {
        if self.extra != 0 {
            let tmp = self.extra;
            self.extra = 0;
            return Some(tmp);
        }

        let mut buf = [0; 2];
        self.chars.next().map(|ch| {
            let n = ch.encode_utf16(&mut buf).len();
            if n == 2 {
                self.extra = buf[1];
            }
            buf[0]
        })
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.chars.iter.len();
        // 最高的 bytes:code units 比例出现在 3 字节 UTF-8 序列；
        // 4 字节 UTF-8 序列会产生 2 个 UTF-16 code unit。
        // 因此下界假设剩余字节尽可能多地组成 3 字节序列。
        // 上界使用 1 字节序列，因为它们的 bytes:code units 比例最高。
        // `(len + 2)` 不会溢出，因为 `slice::Iter` 属于内存中的切片，
        // 切片最大长度为 `isize::MAX`，远小于 `usize::MAX`。
        if self.extra == 0 {
            (len.div_ceil(3), Some(len))
        } else {
            // 当前位于代理对中间，因此需要把剩余代理项加入上下界。
            (len.div_ceil(3) + 1, Some(len + 1))
        }
    }
}

#[stable(feature = "fused", since = "1.26.0")]
impl FusedIterator for EncodeUtf16<'_> {}

/// [`str::escape_debug`] 的返回类型。
#[stable(feature = "str_escape", since = "1.34.0")]
#[derive(Clone, Debug)]
pub struct EscapeDebug<'a> {
    pub(super) inner: Chain<
        Flatten<option::IntoIter<char_mod::EscapeDebug>>,
        FlatMap<Chars<'a>, char_mod::EscapeDebug, CharEscapeDebugContinue>,
    >,
}

/// [`str::escape_default`] 的返回类型。
#[stable(feature = "str_escape", since = "1.34.0")]
#[derive(Clone, Debug)]
pub struct EscapeDefault<'a> {
    pub(super) inner: FlatMap<Chars<'a>, char_mod::EscapeDefault, CharEscapeDefault>,
}

/// [`str::escape_unicode`] 的返回类型。
#[stable(feature = "str_escape", since = "1.34.0")]
#[derive(Clone, Debug)]
pub struct EscapeUnicode<'a> {
    pub(super) inner: FlatMap<Chars<'a>, char_mod::EscapeUnicode, CharEscapeUnicode>,
}

macro_rules! escape_types_impls {
    ($( $Name: ident ),+) => {$(
        #[stable(feature = "str_escape", since = "1.34.0")]
        impl<'a> fmt::Display for $Name<'a> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.clone().try_for_each(|c| f.write_char(c))
            }
        }

        #[stable(feature = "str_escape", since = "1.34.0")]
        impl<'a> Iterator for $Name<'a> {
            type Item = char;

            #[inline]
            fn next(&mut self) -> Option<char> { self.inner.next() }

            #[inline]
            fn size_hint(&self) -> (usize, Option<usize>) { self.inner.size_hint() }

            #[inline]
            fn try_fold<Acc, Fold, R>(&mut self, init: Acc, fold: Fold) -> R where
                Self: Sized, Fold: FnMut(Acc, Self::Item) -> R, R: Try<Output = Acc>
            {
                self.inner.try_fold(init, fold)
            }

            #[inline]
            fn fold<Acc, Fold>(self, init: Acc, fold: Fold) -> Acc
                where Fold: FnMut(Acc, Self::Item) -> Acc,
            {
                self.inner.fold(init, fold)
            }
        }

        #[stable(feature = "str_escape", since = "1.34.0")]
        impl<'a> FusedIterator for $Name<'a> {}
    )+}
}

escape_types_impls!(EscapeDebug, EscapeDefault, EscapeUnicode);
