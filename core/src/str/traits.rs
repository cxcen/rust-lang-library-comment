//! `str` 的 trait 实现。

use super::ParseBoolError;
use crate::cmp::Ordering;
use crate::intrinsics::unchecked_sub;
use crate::slice::SliceIndex;
use crate::ub_checks::assert_unsafe_precondition;
use crate::{ops, ptr, range};

/// 实现字符串排序。
///
/// 字符串按字节值进行[字典序](Ord#lexicographical-comparison)排序。这等价于按 UTF-8
/// 编码后的字节序比较，通常会反映 Unicode code point 在码表中的位置，但不等同于自然语言中的
/// “字母顺序”。字母顺序会随语言和区域设置变化；按文化习惯排序需要 locale 特定数据，
/// 超出 `str` 类型的职责范围。
#[stable(feature = "rust1", since = "1.0.0")]
impl Ord for str {
    #[inline]
    fn cmp(&self, other: &str) -> Ordering {
        self.as_bytes().cmp(other.as_bytes())
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
impl const PartialEq for str {
    #[inline]
    fn eq(&self, other: &str) -> bool {
        self.as_bytes() == other.as_bytes()
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
impl const Eq for str {}

/// 实现字符串比较操作。
///
/// 字符串按字节值进行[字典序](Ord#lexicographical-comparison)比较。这是稳定、与 locale
/// 无关的二进制顺序；它不尝试实现自然语言中的大小写折叠、规范化或排序规则。
#[stable(feature = "rust1", since = "1.0.0")]
impl PartialOrd for str {
    #[inline]
    fn partial_cmp(&self, other: &str) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_unstable(feature = "const_index", issue = "143775")]
impl<I> const ops::Index<I> for str
where
    I: [const] SliceIndex<str>,
{
    type Output = I::Output;

    #[inline]
    fn index(&self, index: I) -> &I::Output {
        index.index(self)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_unstable(feature = "const_index", issue = "143775")]
impl<I> const ops::IndexMut<I> for str
where
    I: [const] SliceIndex<str>,
{
    #[inline]
    fn index_mut(&mut self, index: I) -> &mut I::Output {
        index.index_mut(self)
    }
}

#[inline(never)]
#[cold]
#[track_caller]
const fn str_index_overflow_fail() -> ! {
    panic!("attempted to index str up to maximum usize");
}

/// 实现 `&self[..]` 或 `&mut self[..]` 语法的子串切片。
///
/// 返回整个字符串切片，也就是返回 `&self` 或 `&mut self`。
/// 它等价于 `&self[0 .. len]` 或 `&mut self[0 .. len]`。
/// 与其他索引操作不同，该操作永远不会 panic。
///
/// 该操作是 *O*(1)。
///
/// 在 1.20.0 之前，这些索引操作仍通过直接实现 `Index` 和 `IndexMut` 支持。
///
/// 等价于 `&self[0 .. len]` 或 `&mut self[0 .. len]`。
#[stable(feature = "str_checked_slicing", since = "1.20.0")]
#[rustc_const_unstable(feature = "const_index", issue = "143775")]
unsafe impl const SliceIndex<str> for ops::RangeFull {
    type Output = str;
    #[inline]
    fn get(self, slice: &str) -> Option<&Self::Output> {
        Some(slice)
    }
    #[inline]
    fn get_mut(self, slice: &mut str) -> Option<&mut Self::Output> {
        Some(slice)
    }
    #[inline]
    unsafe fn get_unchecked(self, slice: *const str) -> *const Self::Output {
        slice
    }
    #[inline]
    unsafe fn get_unchecked_mut(self, slice: *mut str) -> *mut Self::Output {
        slice
    }
    #[inline]
    fn index(self, slice: &str) -> &Self::Output {
        slice
    }
    #[inline]
    fn index_mut(self, slice: &mut str) -> &mut Self::Output {
        slice
    }
}

/// 实现 `&self[begin .. end]` 或 `&mut self[begin .. end]` 语法的子串切片。
///
/// 返回给定字符串中字节范围 [`begin`, `end`) 对应的切片。
/// 注意这里的索引是字节索引，不是 `char` 序号；两端必须位于 UTF-8 字符边界。
///
/// 该操作是 *O*(1)。
///
/// 在 1.20.0 之前，这些索引操作仍通过直接实现 `Index` 和 `IndexMut` 支持。
///
/// # Panics
///
/// 当 `begin` 或 `end` 未指向字符起始字节偏移（由 `is_char_boundary` 定义）、
/// `begin > end`，或 `end > len` 时 panic。
///
/// # 示例
///
/// ```
/// let s = "Löwe 老虎 Léopard";
/// assert_eq!(&s[0 .. 1], "L");
///
/// assert_eq!(&s[1 .. 9], "öwe 老");
///
/// // 这些写法会 panic：
/// // 字节 2 位于 `ö` 的编码内部：
/// // &s[2 ..3];
///
/// // 字节 8 位于 `老` 的编码内部：
/// // &s[1 .. 8];
///
/// // 字节 100 超出字符串范围：
/// // &s[3 .. 100];
/// ```
#[stable(feature = "str_checked_slicing", since = "1.20.0")]
#[rustc_const_unstable(feature = "const_index", issue = "143775")]
unsafe impl const SliceIndex<str> for ops::Range<usize> {
    type Output = str;
    #[inline]
    fn get(self, slice: &str) -> Option<&Self::Output> {
        if self.start <= self.end
            && slice.is_char_boundary(self.start)
            && slice.is_char_boundary(self.end)
        {
            // SAFETY: 已检查 `start` 和 `end` 位于 UTF-8 字符边界上；
            // 输入是安全引用，返回的子切片仍在同一合法 `str` 内，因此也是合法 UTF-8。
            Some(unsafe { &*self.get_unchecked(slice) })
        } else {
            None
        }
    }
    #[inline]
    fn get_mut(self, slice: &mut str) -> Option<&mut Self::Output> {
        if self.start <= self.end
            && slice.is_char_boundary(self.start)
            && slice.is_char_boundary(self.end)
        {
            // SAFETY: 已检查 `start` 和 `end` 位于 UTF-8 字符边界上；
            // 指针来自 `&mut str`，因此具有独占访问权。
            Some(unsafe { &mut *self.get_unchecked_mut(slice) })
        } else {
            None
        }
    }
    #[inline]
    #[track_caller]
    unsafe fn get_unchecked(self, slice: *const str) -> *const Self::Output {
        let slice = slice as *const [u8];

        assert_unsafe_precondition!(
            // 理想情况下还应检查范围端点是否位于字符边界；但在这里只拿到原始指针，
            // 若为了检查去读取指针背后的字节，会引入 aliasing 影响。
            // 若不专门给 `SliceIndex` 增加特殊函数，也无法把该检查上移到 `str::get_unchecked`。
            check_library_ub,
            "str::get_unchecked requires that the range is within the string slice",
            (
                start: usize = self.start,
                end: usize = self.end,
                len: usize = slice.len()
            ) => end >= start && end <= len,
        );

        // SAFETY: 调用方保证 `self` 在 `slice` 边界内，满足 `add` 的全部条件。
        unsafe {
            let new_len = unchecked_sub(self.end, self.start);
            ptr::slice_from_raw_parts(slice.as_ptr().add(self.start), new_len) as *const str
        }
    }
    #[inline]
    #[track_caller]
    unsafe fn get_unchecked_mut(self, slice: *mut str) -> *mut Self::Output {
        let slice = slice as *mut [u8];

        assert_unsafe_precondition!(
            check_library_ub,
            "str::get_unchecked_mut requires that the range is within the string slice",
            (
                start: usize = self.start,
                end: usize = self.end,
                len: usize = slice.len()
            ) => end >= start && end <= len,
        );

        // SAFETY: 见 `get_unchecked` 中的说明。
        unsafe {
            let new_len = unchecked_sub(self.end, self.start);
            ptr::slice_from_raw_parts_mut(slice.as_mut_ptr().add(self.start), new_len) as *mut str
        }
    }
    #[inline]
    fn index(self, slice: &str) -> &Self::Output {
        let (start, end) = (self.start, self.end);
        match self.get(slice) {
            Some(s) => s,
            None => super::slice_error_fail(slice, start, end),
        }
    }
    #[inline]
    fn index_mut(self, slice: &mut str) -> &mut Self::Output {
        // `is_char_boundary` 会检查索引位于 [0, .len()] 内。
        // 由于 NLL 限制，不能像上面那样复用 `get`。
        if self.start <= self.end
            && slice.is_char_boundary(self.start)
            && slice.is_char_boundary(self.end)
        {
            // SAFETY: 已检查 `start` 和 `end` 位于 UTF-8 字符边界上；
            // 输入是安全的独占引用，返回的子切片也保持有效。
            unsafe { &mut *self.get_unchecked_mut(slice) }
        } else {
            super::slice_error_fail(slice, self.start, self.end)
        }
    }
}

#[unstable(feature = "new_range_api", issue = "125687")]
#[rustc_const_unstable(feature = "const_index", issue = "143775")]
unsafe impl const SliceIndex<str> for range::Range<usize> {
    type Output = str;
    #[inline]
    fn get(self, slice: &str) -> Option<&Self::Output> {
        if self.start <= self.end
            && slice.is_char_boundary(self.start)
            && slice.is_char_boundary(self.end)
        {
            // SAFETY: 已检查 `start` 和 `end` 位于 UTF-8 字符边界上；
            // 输入是安全引用，返回的子切片仍是合法 UTF-8。
            Some(unsafe { &*self.get_unchecked(slice) })
        } else {
            None
        }
    }
    #[inline]
    fn get_mut(self, slice: &mut str) -> Option<&mut Self::Output> {
        if self.start <= self.end
            && slice.is_char_boundary(self.start)
            && slice.is_char_boundary(self.end)
        {
            // SAFETY: 已检查 `start` 和 `end` 位于 UTF-8 字符边界上；
            // 指针来自 `&mut str`，因此具有独占访问权。
            Some(unsafe { &mut *self.get_unchecked_mut(slice) })
        } else {
            None
        }
    }
    #[inline]
    #[track_caller]
    unsafe fn get_unchecked(self, slice: *const str) -> *const Self::Output {
        let slice = slice as *const [u8];

        assert_unsafe_precondition!(
            // 理想情况下还应检查范围端点是否位于字符边界；但在这里只拿到原始指针，
            // 若为了检查去读取指针背后的字节，会引入 aliasing 影响。
            // 若不专门给 `SliceIndex` 增加特殊函数，也无法把该检查上移到 `str::get_unchecked`。
            check_library_ub,
            "str::get_unchecked requires that the range is within the string slice",
            (
                start: usize = self.start,
                end: usize = self.end,
                len: usize = slice.len()
            ) => end >= start && end <= len,
        );

        // SAFETY: 调用方保证 `self` 在 `slice` 边界内，满足 `add` 的全部条件。
        unsafe {
            let new_len = unchecked_sub(self.end, self.start);
            ptr::slice_from_raw_parts(slice.as_ptr().add(self.start), new_len) as *const str
        }
    }
    #[inline]
    #[track_caller]
    unsafe fn get_unchecked_mut(self, slice: *mut str) -> *mut Self::Output {
        let slice = slice as *mut [u8];

        assert_unsafe_precondition!(
            check_library_ub,
            "str::get_unchecked_mut requires that the range is within the string slice",
            (
                start: usize = self.start,
                end: usize = self.end,
                len: usize = slice.len()
            ) => end >= start && end <= len,
        );

        // SAFETY: 见 `get_unchecked` 中的说明。
        unsafe {
            let new_len = unchecked_sub(self.end, self.start);
            ptr::slice_from_raw_parts_mut(slice.as_mut_ptr().add(self.start), new_len) as *mut str
        }
    }
    #[inline]
    fn index(self, slice: &str) -> &Self::Output {
        let (start, end) = (self.start, self.end);
        match self.get(slice) {
            Some(s) => s,
            None => super::slice_error_fail(slice, start, end),
        }
    }
    #[inline]
    fn index_mut(self, slice: &mut str) -> &mut Self::Output {
        // `is_char_boundary` 会检查索引位于 [0, .len()] 内。
        // 由于 NLL 限制，不能像上面那样复用 `get`。
        if self.start <= self.end
            && slice.is_char_boundary(self.start)
            && slice.is_char_boundary(self.end)
        {
            // SAFETY: 已检查 `start` 和 `end` 位于 UTF-8 字符边界上；
            // 输入是安全的独占引用，返回的子切片也保持有效。
            unsafe { &mut *self.get_unchecked_mut(slice) }
        } else {
            super::slice_error_fail(slice, self.start, self.end)
        }
    }
}

/// 为任意边界组合实现子串切片。
///
/// 返回由各边界提供的字节索引限定的字符串切片。最终范围仍必须位于 UTF-8 字符边界上。
///
/// 该操作是 *O*(1)。
///
/// # Panics
///
/// 当 `begin` 或 `end`（若存在，并在处理包含/排除边界后）未指向字符起始字节偏移
///（由 `is_char_boundary` 定义）、`begin > end`，或 `end > len` 时 panic。
#[stable(feature = "slice_index_str_with_ops_bound_pair", since = "1.73.0")]
unsafe impl SliceIndex<str> for (ops::Bound<usize>, ops::Bound<usize>) {
    type Output = str;

    #[inline]
    fn get(self, slice: &str) -> Option<&str> {
        crate::slice::index::into_range(slice.len(), self)?.get(slice)
    }

    #[inline]
    fn get_mut(self, slice: &mut str) -> Option<&mut str> {
        crate::slice::index::into_range(slice.len(), self)?.get_mut(slice)
    }

    #[inline]
    unsafe fn get_unchecked(self, slice: *const str) -> *const str {
        let len = (slice as *const [u8]).len();
        // SAFETY: 调用方必须维护 `get_unchecked` 的安全契约，包括范围在界内且边界适合 `str`。
        unsafe { crate::slice::index::into_range_unchecked(len, self).get_unchecked(slice) }
    }

    #[inline]
    unsafe fn get_unchecked_mut(self, slice: *mut str) -> *mut str {
        let len = (slice as *mut [u8]).len();
        // SAFETY: 调用方必须维护 `get_unchecked_mut` 的安全契约，包括范围在界内且边界适合 `str`。
        unsafe { crate::slice::index::into_range_unchecked(len, self).get_unchecked_mut(slice) }
    }

    #[inline]
    fn index(self, slice: &str) -> &str {
        crate::slice::index::into_slice_range(slice.len(), self).index(slice)
    }

    #[inline]
    fn index_mut(self, slice: &mut str) -> &mut str {
        crate::slice::index::into_slice_range(slice.len(), self).index_mut(slice)
    }
}

/// 实现 `&self[.. end]` 或 `&mut self[.. end]` 语法的子串切片。
///
/// 返回给定字符串中字节范围 \[0, `end`) 对应的切片。
/// 等价于 `&self[0 .. end]` 或 `&mut self[0 .. end]`。
///
/// 该操作是 *O*(1)。
///
/// 在 1.20.0 之前，这些索引操作仍通过直接实现 `Index` 和 `IndexMut` 支持。
///
/// # Panics
///
/// 当 `end` 未指向字符起始字节偏移（由 `is_char_boundary` 定义），或 `end > len` 时 panic。
#[stable(feature = "str_checked_slicing", since = "1.20.0")]
#[rustc_const_unstable(feature = "const_index", issue = "143775")]
unsafe impl const SliceIndex<str> for ops::RangeTo<usize> {
    type Output = str;
    #[inline]
    fn get(self, slice: &str) -> Option<&Self::Output> {
        if slice.is_char_boundary(self.end) {
            // SAFETY: 已检查 `end` 位于 UTF-8 字符边界上；
            // 输入是安全引用，返回的子切片仍是合法 `str`。
            Some(unsafe { &*self.get_unchecked(slice) })
        } else {
            None
        }
    }
    #[inline]
    fn get_mut(self, slice: &mut str) -> Option<&mut Self::Output> {
        if slice.is_char_boundary(self.end) {
            // SAFETY: 已检查 `end` 位于 UTF-8 字符边界上；
            // 输入是安全的独占引用，返回的子切片仍是合法 `str`。
            Some(unsafe { &mut *self.get_unchecked_mut(slice) })
        } else {
            None
        }
    }
    #[inline]
    unsafe fn get_unchecked(self, slice: *const str) -> *const Self::Output {
        // SAFETY: 调用方必须维护 `get_unchecked` 的安全契约。
        unsafe { (0..self.end).get_unchecked(slice) }
    }
    #[inline]
    unsafe fn get_unchecked_mut(self, slice: *mut str) -> *mut Self::Output {
        // SAFETY: 调用方必须维护 `get_unchecked_mut` 的安全契约。
        unsafe { (0..self.end).get_unchecked_mut(slice) }
    }
    #[inline]
    fn index(self, slice: &str) -> &Self::Output {
        let end = self.end;
        match self.get(slice) {
            Some(s) => s,
            None => super::slice_error_fail(slice, 0, end),
        }
    }
    #[inline]
    fn index_mut(self, slice: &mut str) -> &mut Self::Output {
        if slice.is_char_boundary(self.end) {
            // SAFETY: 已检查 `end` 位于 UTF-8 字符边界上；
            // 输入是安全的独占引用，返回的子切片仍是合法 `str`。
            unsafe { &mut *self.get_unchecked_mut(slice) }
        } else {
            super::slice_error_fail(slice, 0, self.end)
        }
    }
}

/// 实现 `&self[begin ..]` 或 `&mut self[begin ..]` 语法的子串切片。
///
/// 返回给定字符串中字节范围 \[`begin`, `len`) 对应的切片。
/// 等价于 `&self[begin .. len]` 或 `&mut self[begin .. len]`。
///
/// 该操作是 *O*(1)。
///
/// 在 1.20.0 之前，这些索引操作仍通过直接实现 `Index` 和 `IndexMut` 支持。
///
/// # Panics
///
/// 当 `begin` 未指向字符起始字节偏移（由 `is_char_boundary` 定义），或 `begin > len` 时 panic。
#[stable(feature = "str_checked_slicing", since = "1.20.0")]
#[rustc_const_unstable(feature = "const_index", issue = "143775")]
unsafe impl const SliceIndex<str> for ops::RangeFrom<usize> {
    type Output = str;
    #[inline]
    fn get(self, slice: &str) -> Option<&Self::Output> {
        if slice.is_char_boundary(self.start) {
            // SAFETY: 已检查 `start` 位于 UTF-8 字符边界上；
            // 输入是安全引用，返回的子切片仍是合法 `str`。
            Some(unsafe { &*self.get_unchecked(slice) })
        } else {
            None
        }
    }
    #[inline]
    fn get_mut(self, slice: &mut str) -> Option<&mut Self::Output> {
        if slice.is_char_boundary(self.start) {
            // SAFETY: 已检查 `start` 位于 UTF-8 字符边界上；
            // 输入是安全的独占引用，返回的子切片仍是合法 `str`。
            Some(unsafe { &mut *self.get_unchecked_mut(slice) })
        } else {
            None
        }
    }
    #[inline]
    unsafe fn get_unchecked(self, slice: *const str) -> *const Self::Output {
        let len = (slice as *const [u8]).len();
        // SAFETY: 调用方必须维护 `get_unchecked` 的安全契约。
        unsafe { (self.start..len).get_unchecked(slice) }
    }
    #[inline]
    unsafe fn get_unchecked_mut(self, slice: *mut str) -> *mut Self::Output {
        let len = (slice as *mut [u8]).len();
        // SAFETY: 调用方必须维护 `get_unchecked_mut` 的安全契约。
        unsafe { (self.start..len).get_unchecked_mut(slice) }
    }
    #[inline]
    fn index(self, slice: &str) -> &Self::Output {
        let (start, end) = (self.start, slice.len());
        match self.get(slice) {
            Some(s) => s,
            None => super::slice_error_fail(slice, start, end),
        }
    }
    #[inline]
    fn index_mut(self, slice: &mut str) -> &mut Self::Output {
        if slice.is_char_boundary(self.start) {
            // SAFETY: 已检查 `start` 位于 UTF-8 字符边界上；
            // 输入是安全的独占引用，返回的子切片仍是合法 `str`。
            unsafe { &mut *self.get_unchecked_mut(slice) }
        } else {
            super::slice_error_fail(slice, self.start, slice.len())
        }
    }
}

#[unstable(feature = "new_range_api", issue = "125687")]
#[rustc_const_unstable(feature = "const_index", issue = "143775")]
unsafe impl const SliceIndex<str> for range::RangeFrom<usize> {
    type Output = str;
    #[inline]
    fn get(self, slice: &str) -> Option<&Self::Output> {
        if slice.is_char_boundary(self.start) {
            // SAFETY: 已检查 `start` 位于 UTF-8 字符边界上；
            // 输入是安全引用，返回的子切片仍是合法 `str`。
            Some(unsafe { &*self.get_unchecked(slice) })
        } else {
            None
        }
    }
    #[inline]
    fn get_mut(self, slice: &mut str) -> Option<&mut Self::Output> {
        if slice.is_char_boundary(self.start) {
            // SAFETY: 已检查 `start` 位于 UTF-8 字符边界上；
            // 输入是安全的独占引用，返回的子切片仍是合法 `str`。
            Some(unsafe { &mut *self.get_unchecked_mut(slice) })
        } else {
            None
        }
    }
    #[inline]
    unsafe fn get_unchecked(self, slice: *const str) -> *const Self::Output {
        let len = (slice as *const [u8]).len();
        // SAFETY: 调用方必须维护 `get_unchecked` 的安全契约。
        unsafe { (self.start..len).get_unchecked(slice) }
    }
    #[inline]
    unsafe fn get_unchecked_mut(self, slice: *mut str) -> *mut Self::Output {
        let len = (slice as *mut [u8]).len();
        // SAFETY: 调用方必须维护 `get_unchecked_mut` 的安全契约。
        unsafe { (self.start..len).get_unchecked_mut(slice) }
    }
    #[inline]
    fn index(self, slice: &str) -> &Self::Output {
        let (start, end) = (self.start, slice.len());
        match self.get(slice) {
            Some(s) => s,
            None => super::slice_error_fail(slice, start, end),
        }
    }
    #[inline]
    fn index_mut(self, slice: &mut str) -> &mut Self::Output {
        if slice.is_char_boundary(self.start) {
            // SAFETY: 已检查 `start` 位于 UTF-8 字符边界上；
            // 输入是安全的独占引用，返回的子切片仍是合法 `str`。
            unsafe { &mut *self.get_unchecked_mut(slice) }
        } else {
            super::slice_error_fail(slice, self.start, slice.len())
        }
    }
}

/// 实现 `&self[begin ..= end]` 或 `&mut self[begin ..= end]` 语法的子串切片。
///
/// 返回给定字符串中字节范围 [`begin`, `end`] 对应的切片。
/// 等价于 `&self[begin .. end + 1]` 或 `&mut self[begin .. end + 1]`，
/// 但 `end` 为 `usize` 最大值时除外。
///
/// 该操作是 *O*(1)。
///
/// # Panics
///
/// 当 `begin` 未指向字符起始字节偏移（由 `is_char_boundary` 定义）、
/// `end` 未指向字符结束字节偏移（`end + 1` 要么是字符起始字节偏移，要么等于 `len`）、
/// `begin > end`，或 `end >= len` 时 panic。
#[stable(feature = "inclusive_range", since = "1.26.0")]
#[rustc_const_unstable(feature = "const_index", issue = "143775")]
unsafe impl const SliceIndex<str> for ops::RangeInclusive<usize> {
    type Output = str;
    #[inline]
    fn get(self, slice: &str) -> Option<&Self::Output> {
        if *self.end() == usize::MAX { None } else { self.into_slice_range().get(slice) }
    }
    #[inline]
    fn get_mut(self, slice: &mut str) -> Option<&mut Self::Output> {
        if *self.end() == usize::MAX { None } else { self.into_slice_range().get_mut(slice) }
    }
    #[inline]
    unsafe fn get_unchecked(self, slice: *const str) -> *const Self::Output {
        // SAFETY: 调用方必须维护 `get_unchecked` 的安全契约。
        unsafe { self.into_slice_range().get_unchecked(slice) }
    }
    #[inline]
    unsafe fn get_unchecked_mut(self, slice: *mut str) -> *mut Self::Output {
        // SAFETY: 调用方必须维护 `get_unchecked_mut` 的安全契约。
        unsafe { self.into_slice_range().get_unchecked_mut(slice) }
    }
    #[inline]
    fn index(self, slice: &str) -> &Self::Output {
        if *self.end() == usize::MAX {
            str_index_overflow_fail();
        }
        self.into_slice_range().index(slice)
    }
    #[inline]
    fn index_mut(self, slice: &mut str) -> &mut Self::Output {
        if *self.end() == usize::MAX {
            str_index_overflow_fail();
        }
        self.into_slice_range().index_mut(slice)
    }
}

#[unstable(feature = "new_range_api", issue = "125687")]
#[rustc_const_unstable(feature = "const_index", issue = "143775")]
unsafe impl const SliceIndex<str> for range::RangeInclusive<usize> {
    type Output = str;
    #[inline]
    fn get(self, slice: &str) -> Option<&Self::Output> {
        if self.last == usize::MAX { None } else { self.into_slice_range().get(slice) }
    }
    #[inline]
    fn get_mut(self, slice: &mut str) -> Option<&mut Self::Output> {
        if self.last == usize::MAX { None } else { self.into_slice_range().get_mut(slice) }
    }
    #[inline]
    unsafe fn get_unchecked(self, slice: *const str) -> *const Self::Output {
        // SAFETY: 调用方必须维护 `get_unchecked` 的安全契约。
        unsafe { self.into_slice_range().get_unchecked(slice) }
    }
    #[inline]
    unsafe fn get_unchecked_mut(self, slice: *mut str) -> *mut Self::Output {
        // SAFETY: 调用方必须维护 `get_unchecked_mut` 的安全契约。
        unsafe { self.into_slice_range().get_unchecked_mut(slice) }
    }
    #[inline]
    fn index(self, slice: &str) -> &Self::Output {
        if self.last == usize::MAX {
            str_index_overflow_fail();
        }
        self.into_slice_range().index(slice)
    }
    #[inline]
    fn index_mut(self, slice: &mut str) -> &mut Self::Output {
        if self.last == usize::MAX {
            str_index_overflow_fail();
        }
        self.into_slice_range().index_mut(slice)
    }
}

/// 实现 `&self[..= end]` 或 `&mut self[..= end]` 语法的子串切片。
///
/// 返回给定字符串中字节范围 \[0, `end`] 对应的切片。
/// 等价于 `&self[0 .. end + 1]`，但 `end` 为 `usize` 最大值时除外。
///
/// 该操作是 *O*(1)。
///
/// # Panics
///
/// 当 `end` 未指向字符结束字节偏移（`end + 1` 要么是 `is_char_boundary`
/// 定义的字符起始字节偏移，要么等于 `len`），或 `end >= len` 时 panic。
#[stable(feature = "inclusive_range", since = "1.26.0")]
#[rustc_const_unstable(feature = "const_index", issue = "143775")]
unsafe impl const SliceIndex<str> for ops::RangeToInclusive<usize> {
    type Output = str;
    #[inline]
    fn get(self, slice: &str) -> Option<&Self::Output> {
        (0..=self.end).get(slice)
    }
    #[inline]
    fn get_mut(self, slice: &mut str) -> Option<&mut Self::Output> {
        (0..=self.end).get_mut(slice)
    }
    #[inline]
    unsafe fn get_unchecked(self, slice: *const str) -> *const Self::Output {
        // SAFETY: 调用方必须维护 `get_unchecked` 的安全契约。
        unsafe { (0..=self.end).get_unchecked(slice) }
    }
    #[inline]
    unsafe fn get_unchecked_mut(self, slice: *mut str) -> *mut Self::Output {
        // SAFETY: 调用方必须维护 `get_unchecked_mut` 的安全契约。
        unsafe { (0..=self.end).get_unchecked_mut(slice) }
    }
    #[inline]
    fn index(self, slice: &str) -> &Self::Output {
        (0..=self.end).index(slice)
    }
    #[inline]
    fn index_mut(self, slice: &mut str) -> &mut Self::Output {
        (0..=self.end).index_mut(slice)
    }
}

/// 从字符串解析出一个值。
///
/// `FromStr` 的 [`from_str`] 方法经常通过 [`str`] 的 [`parse`] 方法隐式使用。
/// 示例见 [`parse`] 文档。
///
/// [`from_str`]: FromStr::from_str
/// [`parse`]: str::parse
///
/// `FromStr` 没有生命周期参数，因此只能解析自身不包含生命周期参数的类型。
/// 换句话说，可以用 `FromStr` 解析 `i32`，但不能解析 `&i32`；
/// 可以解析包含 `i32` 的结构体，但不能解析包含 `&i32` 的结构体。
///
/// # 输入格式与往返转换
///
/// 某个类型的 `FromStr` 实现期望什么输入格式取决于该类型。请查看具体类型文档，
/// 了解它能解析哪些格式。注意，类型的 `FromStr` 输入格式不一定接受其 `Display`
/// 实现的输出格式；即使接受，`Display` 也不一定无损，往返转换仍可能丢失信息。
///
/// 不过，如果某个类型具有无损 `Display` 实现，且输出本来就设计为便于机器解析而不只是给人阅读，
/// 那么该类型可以在 `FromStr` 中接受相同格式，并把这种用法写入文档。
/// 同时实现 `Display` 与 `FromStr`，但 `Display` 结果无法被 `FromStr` 解析，可能会让用户意外。
///
/// # 示例
///
/// 在示例 `Point` 类型上实现 `FromStr`：
///
/// ```
/// use std::str::FromStr;
///
/// #[derive(Debug, PartialEq)]
/// struct Point {
///     x: i32,
///     y: i32
/// }
///
/// #[derive(Debug, PartialEq, Eq)]
/// struct ParsePointError;
///
/// impl FromStr for Point {
///     type Err = ParsePointError;
///
///     fn from_str(s: &str) -> Result<Self, Self::Err> {
///         let (x, y) = s
///             .strip_prefix('(')
///             .and_then(|s| s.strip_suffix(')'))
///             .and_then(|s| s.split_once(','))
///             .ok_or(ParsePointError)?;
///
///         let x_fromstr = x.parse::<i32>().map_err(|_| ParsePointError)?;
///         let y_fromstr = y.parse::<i32>().map_err(|_| ParsePointError)?;
///
///         Ok(Point { x: x_fromstr, y: y_fromstr })
///     }
/// }
///
/// let expected = Ok(Point { x: 1, y: 2 });
/// // 显式调用。
/// assert_eq!(Point::from_str("(1,2)"), expected);
/// // 通过 parse 隐式调用。
/// assert_eq!("(1,2)".parse(), expected);
/// assert_eq!("(1,2)".parse::<Point>(), expected);
/// // 非法输入字符串。
/// assert!(Point::from_str("(1 2)").is_err());
/// ```
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
pub const trait FromStr: Sized {
    /// 解析失败时可返回的关联错误类型。
    #[stable(feature = "rust1", since = "1.0.0")]
    type Err;

    /// 解析字符串 `s`，返回该类型的值。
    ///
    /// 解析成功时返回包在 [`Ok`] 中的值；如果字符串格式不正确，则返回该实现特定的
    /// [`Err`] 错误类型。
    ///
    /// # 示例
    ///
    /// 使用实现了 `FromStr` 的 [`i32`]：
    ///
    /// ```
    /// use std::str::FromStr;
    ///
    /// let s = "5";
    /// let x = i32::from_str(s).unwrap();
    ///
    /// assert_eq!(5, x);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_diagnostic_item = "from_str_method"]
    fn from_str(s: &str) -> Result<Self, Self::Err>;
}

#[stable(feature = "rust1", since = "1.0.0")]
impl FromStr for bool {
    type Err = ParseBoolError;

    /// 从字符串解析 `bool`。
    ///
    /// 唯一接受的值是 `"true"` 和 `"false"`；其他任何输入都会返回错误。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::str::FromStr;
    ///
    /// assert_eq!(FromStr::from_str("true"), Ok(true));
    /// assert_eq!(FromStr::from_str("false"), Ok(false));
    /// assert!(<bool as FromStr>::from_str("not even a boolean").is_err());
    /// ```
    ///
    /// 注意，很多情况下在 `str` 上调用 `.parse()` 更合适。
    ///
    /// ```
    /// assert_eq!("true".parse(), Ok(true));
    /// assert_eq!("false".parse(), Ok(false));
    /// assert!("not even a boolean".parse::<bool>().is_err());
    /// ```
    #[inline]
    fn from_str(s: &str) -> Result<bool, ParseBoolError> {
        match s {
            "true" => Ok(true),
            "false" => Ok(false),
            _ => Err(ParseBoolError),
        }
    }
}
