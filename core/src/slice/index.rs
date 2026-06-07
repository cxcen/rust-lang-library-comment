//! `[T]` 的索引实现。
//!
//! 这里把 `usize`、各种 `Range` 和边界对统一到 `SliceIndex`。安全索引路径会做边界检查
//! 并在越界时 panic，`get`/`get_mut` 用 `Option` 表达失败，而 `get_unchecked` 系列把
//! 边界不变量交给调用方；后者一旦越界，即使返回的指针没有被解引用，也会是 UB。

use crate::intrinsics::slice_get_unchecked;
use crate::marker::Destruct;
use crate::panic::const_panic;
use crate::ub_checks::assert_unsafe_precondition;
use crate::{ops, range};

#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_unstable(feature = "const_index", issue = "143775")]
impl<T, I> const ops::Index<I> for [T]
where
    I: [const] SliceIndex<[T]>,
{
    type Output = I::Output;

    #[inline(always)]
    fn index(&self, index: I) -> &I::Output {
        index.index(self)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_unstable(feature = "const_index", issue = "143775")]
impl<T, I> const ops::IndexMut<I> for [T]
where
    I: [const] SliceIndex<[T]>,
{
    #[inline(always)]
    fn index_mut(&mut self, index: I) -> &mut I::Output {
        index.index_mut(self)
    }
}

#[cfg_attr(not(panic = "immediate-abort"), inline(never), cold)]
#[cfg_attr(panic = "immediate-abort", inline)]
#[track_caller]
const fn slice_index_fail(start: usize, end: usize, len: usize) -> ! {
    if start > len {
        const_panic!(
            "slice start index is out of range for slice",
            "range start index {start} out of range for slice of length {len}",
            start: usize,
            len: usize,
        )
    }

    if end > len {
        const_panic!(
            "slice end index is out of range for slice",
            "range end index {end} out of range for slice of length {len}",
            end: usize,
            len: usize,
        )
    }

    if start > end {
        const_panic!(
            "slice index start is larger than end",
            "slice index starts at {start} but ends at {end}",
            start: usize,
            end: usize,
        )
    }

    // 只有 `RangeInclusive` 或 `RangeToInclusive` 且 `end == len` 时才会走到这里；
    // 对闭区间来说，包含末尾索引等价于越过切片最后一个元素。
    const_panic!(
        "slice end index is out of range for slice",
        "range end index {end} out of range for slice of length {len}",
        end: usize,
        len: usize,
    )
}

// UbChecks 对捕捉 unsafe 方法里的调用方错误很有用，但安全索引本身已经完成边界检查，
// 再把这些检查放进热路径会损害内联和 debug 运行时性能。安全与 unsafe 的公开方法共享
// 这些 helper；helper 直接使用 intrinsics，确保这里不会额外插入检查。

#[inline(always)]
const unsafe fn get_offset_len_noubcheck<T>(
    ptr: *const [T],
    offset: usize,
    len: usize,
) -> *const [T] {
    let ptr = ptr as *const T;
    // SAFETY: 调用方已经检查 `offset` 位于同一切片内，且得到的 `[ptr, ptr + len)` 范围有效。
    let ptr = unsafe { crate::intrinsics::offset(ptr, offset) };
    crate::intrinsics::aggregate_raw_ptr(ptr, len)
}

#[inline(always)]
const unsafe fn get_offset_len_mut_noubcheck<T>(
    ptr: *mut [T],
    offset: usize,
    len: usize,
) -> *mut [T] {
    let ptr = ptr as *mut T;
    // SAFETY: 调用方已经检查 `offset` 位于同一可变切片内，且得到的范围有效且唯一可访问。
    let ptr = unsafe { crate::intrinsics::offset(ptr, offset) };
    crate::intrinsics::aggregate_raw_ptr(ptr, len)
}

mod private_slice_index {
    use super::{ops, range};

    #[stable(feature = "slice_get_slice", since = "1.28.0")]
    pub trait Sealed {}

    #[stable(feature = "slice_get_slice", since = "1.28.0")]
    impl Sealed for usize {}
    #[stable(feature = "slice_get_slice", since = "1.28.0")]
    impl Sealed for ops::Range<usize> {}
    #[stable(feature = "slice_get_slice", since = "1.28.0")]
    impl Sealed for ops::RangeTo<usize> {}
    #[stable(feature = "slice_get_slice", since = "1.28.0")]
    impl Sealed for ops::RangeFrom<usize> {}
    #[stable(feature = "slice_get_slice", since = "1.28.0")]
    impl Sealed for ops::RangeFull {}
    #[stable(feature = "slice_get_slice", since = "1.28.0")]
    impl Sealed for ops::RangeInclusive<usize> {}
    #[stable(feature = "slice_get_slice", since = "1.28.0")]
    impl Sealed for ops::RangeToInclusive<usize> {}
    #[stable(feature = "slice_index_with_ops_bound_pair", since = "1.53.0")]
    impl Sealed for (ops::Bound<usize>, ops::Bound<usize>) {}

    #[unstable(feature = "new_range_api", issue = "125687")]
    impl Sealed for range::Range<usize> {}
    #[unstable(feature = "new_range_api", issue = "125687")]
    impl Sealed for range::RangeInclusive<usize> {}
    #[unstable(feature = "new_range_api", issue = "125687")]
    impl Sealed for range::RangeToInclusive<usize> {}
    #[unstable(feature = "new_range_api", issue = "125687")]
    impl Sealed for range::RangeFrom<usize> {}

    impl Sealed for ops::IndexRange {}

    #[unstable(feature = "sliceindex_wrappers", issue = "146179")]
    impl Sealed for crate::index::Last {}
    #[unstable(feature = "sliceindex_wrappers", issue = "146179")]
    impl<T> Sealed for crate::index::Clamp<T> where T: Sealed {}
}

/// 索引操作使用的辅助 trait。
///
/// 实现者必须承诺：如果传给 `get_unchecked(_mut)` 的切片指针来自一个安全引用，
/// 且索引值满足该实现声明的边界条件，那么返回的指针也能形成相同共享性或可变性的安全引用。
/// 这个 trait 是 unsafe 的原因正在于此：实现错误会让安全的 `get`、`index` 或内部
/// `get_unchecked` 路径产生越界、错长度或违反 aliasing 的引用。
#[stable(feature = "slice_get_slice", since = "1.28.0")]
#[rustc_diagnostic_item = "SliceIndex"]
#[rustc_on_unimplemented(
    on(T = "str", label = "string indices are ranges of `usize`",),
    on(
        all(any(T = "str", T = "&str", T = "alloc::string::String"), Self = "{integer}"),
        note = "you can use `.chars().nth()` or `.bytes().nth()`\n\
                for more information, see chapter 8 in The Book: \
                <https://doc.rust-lang.org/book/ch08-02-strings.html#indexing-into-strings>"
    ),
    message = "the type `{T}` cannot be indexed by `{Self}`",
    label = "slice indices are of type `usize` or ranges of `usize`"
)]
#[rustc_const_unstable(feature = "const_index", issue = "143775")]
pub const unsafe trait SliceIndex<T: ?Sized>: private_slice_index::Sealed {
    /// 各索引方法返回的输出类型。
    #[stable(feature = "slice_get_slice", since = "1.28.0")]
    type Output: ?Sized;

    /// 如果索引在边界内，返回该位置对应输出的共享引用。
    #[unstable(feature = "slice_index_methods", issue = "none")]
    fn get(self, slice: &T) -> Option<&Self::Output>;

    /// 如果索引在边界内，返回该位置对应输出的可变引用。
    #[unstable(feature = "slice_index_methods", issue = "none")]
    fn get_mut(self, slice: &mut T) -> Option<&mut Self::Output>;

    /// 不做边界检查，直接返回该位置对应输出的指针。
///
    /// 使用越界索引或悬垂的 `slice` 指针调用本方法是 *[undefined behavior]*，
    /// 即使返回的指针之后没有被使用。原因是实现通常会把边界条件传给 `assume`、
    /// `offset` 或引用构造，编译器可据此进行优化。
    ///
    /// [undefined behavior]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    #[unstable(feature = "slice_index_methods", issue = "none")]
    unsafe fn get_unchecked(self, slice: *const T) -> *const Self::Output;

    /// 不做边界检查，直接返回该位置对应输出的可变指针。
///
    /// 使用越界索引或悬垂的 `slice` 指针调用本方法是 *[undefined behavior]*，
    /// 即使返回的指针之后没有被使用；可变版本还必须保持最终引用的唯一访问权。
    ///
    /// [undefined behavior]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    #[unstable(feature = "slice_index_methods", issue = "none")]
    unsafe fn get_unchecked_mut(self, slice: *mut T) -> *mut Self::Output;

    /// 返回该位置对应输出的共享引用；越界时 panic。
    #[unstable(feature = "slice_index_methods", issue = "none")]
    #[track_caller]
    fn index(self, slice: &T) -> &Self::Output;

    /// 返回该位置对应输出的可变引用；越界时 panic。
    #[unstable(feature = "slice_index_methods", issue = "none")]
    #[track_caller]
    fn index_mut(self, slice: &mut T) -> &mut Self::Output;
}

/// 如果索引越界，`index` 和 `index_mut` 会 panic。
#[stable(feature = "slice_get_slice_impls", since = "1.15.0")]
#[rustc_const_unstable(feature = "const_index", issue = "143775")]
unsafe impl<T> const SliceIndex<[T]> for usize {
    type Output = T;

    #[inline]
    fn get(self, slice: &[T]) -> Option<&T> {
        if self < slice.len() {
            // SAFETY: 上面的 `self < slice.len()` 已证明单个元素索引在边界内。
            unsafe { Some(slice_get_unchecked(slice, self)) }
        } else {
            None
        }
    }

    #[inline]
    fn get_mut(self, slice: &mut [T]) -> Option<&mut T> {
        if self < slice.len() {
            // SAFETY: 上面的 `self < slice.len()` 已证明单个元素索引在边界内。
            unsafe { Some(slice_get_unchecked(slice, self)) }
        } else {
            None
        }
    }

    #[inline]
    #[track_caller]
    unsafe fn get_unchecked(self, slice: *const [T]) -> *const T {
        assert_unsafe_precondition!(
            check_language_ub, // okay because of the `assume` below
            "slice::get_unchecked requires that the index is within the slice",
            (this: usize = self, len: usize = slice.len()) => this < len
        );
        // SAFETY: 调用方保证 `slice` 不是悬垂指针，因此其长度不会超过 `isize::MAX`；
        // 调用方还保证 `self` 位于 `slice` 边界内，所以 `self` 可表示为同一 allocation 内
        // 的 `isize` 偏移，传给底层 unchecked 索引不会越界。
        unsafe {
            // 使用 intrinsics::assume 而不是 hint::assert_unchecked，避免把本函数的
            // 前置条件检查两次；这也是越界调用会立即成为 UB 的原因之一。
            crate::intrinsics::assume(self < slice.len());
            slice_get_unchecked(slice, self)
        }
    }

    #[inline]
    #[track_caller]
    unsafe fn get_unchecked_mut(self, slice: *mut [T]) -> *mut T {
        assert_unsafe_precondition!(
            check_library_ub,
            "slice::get_unchecked_mut requires that the index is within the slice",
            (this: usize = self, len: usize = slice.len()) => this < len
        );
        // SAFETY: 与上面的 `get_unchecked` 相同；调用方还必须维护可变访问的唯一性。
        unsafe { slice_get_unchecked(slice, self) }
    }

    #[inline]
    fn index(self, slice: &[T]) -> &T {
        // N.B. 使用 intrinsic indexing，让编译器生成标准的切片边界检查。
        &(*slice)[self]
    }

    #[inline]
    fn index_mut(self, slice: &mut [T]) -> &mut T {
        // N.B. 使用 intrinsic indexing，让编译器生成标准的切片边界检查。
        &mut (*slice)[self]
    }
}

/// 因为 `IndexRange` 自身保证 `start <= end`，这里比普通 `Range<usize>` 需要更少检查；
/// 普通范围可能是 `100..3` 这种反向范围。
#[rustc_const_unstable(feature = "const_index", issue = "143775")]
unsafe impl<T> const SliceIndex<[T]> for ops::IndexRange {
    type Output = [T];

    #[inline]
    fn get(self, slice: &[T]) -> Option<&[T]> {
        if self.end() <= slice.len() {
            // SAFETY: `self.end() <= slice.len()` 且 `IndexRange` 保证 `start <= end`。
            unsafe { Some(&*get_offset_len_noubcheck(slice, self.start(), self.len())) }
        } else {
            None
        }
    }

    #[inline]
    fn get_mut(self, slice: &mut [T]) -> Option<&mut [T]> {
        if self.end() <= slice.len() {
            // SAFETY: `self.end() <= slice.len()` 且 `IndexRange` 保证 `start <= end`。
            unsafe { Some(&mut *get_offset_len_mut_noubcheck(slice, self.start(), self.len())) }
        } else {
            None
        }
    }

    #[inline]
    #[track_caller]
    unsafe fn get_unchecked(self, slice: *const [T]) -> *const [T] {
        assert_unsafe_precondition!(
            check_library_ub,
            "slice::get_unchecked requires that the index is within the slice",
            (end: usize = self.end(), len: usize = slice.len()) => end <= len
        );
        // SAFETY: 调用方保证 `slice` 有效且 `self.end() <= slice.len()`；再加上
        // `IndexRange` 的 `start <= end`，可构造位于同一切片内的子切片。
        unsafe { get_offset_len_noubcheck(slice, self.start(), self.len()) }
    }

    #[inline]
    #[track_caller]
    unsafe fn get_unchecked_mut(self, slice: *mut [T]) -> *mut [T] {
        assert_unsafe_precondition!(
            check_library_ub,
            "slice::get_unchecked_mut requires that the index is within the slice",
            (end: usize = self.end(), len: usize = slice.len()) => end <= len
        );

        // SAFETY: 与上面的 `get_unchecked` 相同；可变返回值的唯一访问权由调用方保证。
        unsafe { get_offset_len_mut_noubcheck(slice, self.start(), self.len()) }
    }

    #[inline]
    fn index(self, slice: &[T]) -> &[T] {
        if self.end() <= slice.len() {
            // SAFETY: `self.end() <= slice.len()` 且 `IndexRange` 保证 `start <= end`。
            unsafe { &*get_offset_len_noubcheck(slice, self.start(), self.len()) }
        } else {
            slice_index_fail(self.start(), self.end(), slice.len())
        }
    }

    #[inline]
    fn index_mut(self, slice: &mut [T]) -> &mut [T] {
        if self.end() <= slice.len() {
            // SAFETY: `self.end() <= slice.len()` 且 `IndexRange` 保证 `start <= end`。
            unsafe { &mut *get_offset_len_mut_noubcheck(slice, self.start(), self.len()) }
        } else {
            slice_index_fail(self.start(), self.end(), slice.len())
        }
    }
}

/// 如果出现下列情况，`index` 和 `index_mut` 会 panic：
/// - 范围起点大于终点；或
/// - 范围终点越过切片边界。
#[stable(feature = "slice_get_slice_impls", since = "1.15.0")]
#[rustc_const_unstable(feature = "const_index", issue = "143775")]
unsafe impl<T> const SliceIndex<[T]> for ops::Range<usize> {
    type Output = [T];

    #[inline]
    fn get(self, slice: &[T]) -> Option<&[T]> {
        // 使用 checked_sub 是在 MIR 中得到 `SubUnchecked` 优化信息的安全方式。
        if let Some(new_len) = usize::checked_sub(self.end, self.start)
            && self.end <= slice.len()
        {
            // SAFETY: `checked_sub` 证明 `end >= start`，同时 `end <= slice.len()`。
            unsafe { Some(&*get_offset_len_noubcheck(slice, self.start, new_len)) }
        } else {
            None
        }
    }

    #[inline]
    fn get_mut(self, slice: &mut [T]) -> Option<&mut [T]> {
        if let Some(new_len) = usize::checked_sub(self.end, self.start)
            && self.end <= slice.len()
        {
            // SAFETY: `checked_sub` 证明 `end >= start`，同时 `end <= slice.len()`。
            unsafe { Some(&mut *get_offset_len_mut_noubcheck(slice, self.start, new_len)) }
        } else {
            None
        }
    }

    #[inline]
    #[track_caller]
    unsafe fn get_unchecked(self, slice: *const [T]) -> *const [T] {
        assert_unsafe_precondition!(
            check_library_ub,
            "slice::get_unchecked requires that the range is within the slice",
            (
                start: usize = self.start,
                end: usize = self.end,
                len: usize = slice.len()
            ) => end >= start && end <= len
        );

        // SAFETY: 调用方保证 `slice` 有效，且 `start <= end <= len`。因此起点和长度都在
        // 同一切片内，`add` 不会越界，长度计算也不会下溢或溢出。
        unsafe {
            // 使用 intrinsic 可避免多余的 UB 检查；本方法的前置条件已经检查了
            // `end >= start`。
            let new_len = crate::intrinsics::unchecked_sub(self.end, self.start);
            get_offset_len_noubcheck(slice, self.start, new_len)
        }
    }

    #[inline]
    #[track_caller]
    unsafe fn get_unchecked_mut(self, slice: *mut [T]) -> *mut [T] {
        assert_unsafe_precondition!(
            check_library_ub,
            "slice::get_unchecked_mut requires that the range is within the slice",
            (
                start: usize = self.start,
                end: usize = self.end,
                len: usize = slice.len()
            ) => end >= start && end <= len
        );
        // SAFETY: 与上面的 `get_unchecked` 相同；调用方还必须维护可变访问的唯一性。
        unsafe {
            let new_len = crate::intrinsics::unchecked_sub(self.end, self.start);
            get_offset_len_mut_noubcheck(slice, self.start, new_len)
        }
    }

    #[inline(always)]
    fn index(self, slice: &[T]) -> &[T] {
        // 使用 checked_sub 是在 MIR 中得到 `SubUnchecked` 优化信息的安全方式。
        if let Some(new_len) = usize::checked_sub(self.end, self.start)
            && self.end <= slice.len()
        {
            // SAFETY: `checked_sub` 证明 `end >= start`，同时 `end <= slice.len()`。
            unsafe { &*get_offset_len_noubcheck(slice, self.start, new_len) }
        } else {
            slice_index_fail(self.start, self.end, slice.len())
        }
    }

    #[inline]
    fn index_mut(self, slice: &mut [T]) -> &mut [T] {
        // 使用 checked_sub 是在 MIR 中得到 `SubUnchecked` 优化信息的安全方式。
        if let Some(new_len) = usize::checked_sub(self.end, self.start)
            && self.end <= slice.len()
        {
            // SAFETY: `checked_sub` 证明 `end >= start`，同时 `end <= slice.len()`。
            unsafe { &mut *get_offset_len_mut_noubcheck(slice, self.start, new_len) }
        } else {
            slice_index_fail(self.start, self.end, slice.len())
        }
    }
}

#[unstable(feature = "new_range_api", issue = "125687")]
#[rustc_const_unstable(feature = "const_index", issue = "143775")]
unsafe impl<T> const SliceIndex<[T]> for range::Range<usize> {
    type Output = [T];

    #[inline]
    fn get(self, slice: &[T]) -> Option<&[T]> {
        ops::Range::from(self).get(slice)
    }

    #[inline]
    fn get_mut(self, slice: &mut [T]) -> Option<&mut [T]> {
        ops::Range::from(self).get_mut(slice)
    }

    #[inline]
    unsafe fn get_unchecked(self, slice: *const [T]) -> *const [T] {
        // SAFETY: 调用方必须维护 `get_unchecked` 的边界契约；这里只转换范围类型。
        unsafe { ops::Range::from(self).get_unchecked(slice) }
    }

    #[inline]
    unsafe fn get_unchecked_mut(self, slice: *mut [T]) -> *mut [T] {
        // SAFETY: 调用方必须维护 `get_unchecked_mut` 的边界和唯一访问契约。
        unsafe { ops::Range::from(self).get_unchecked_mut(slice) }
    }

    #[inline(always)]
    fn index(self, slice: &[T]) -> &[T] {
        ops::Range::from(self).index(slice)
    }

    #[inline]
    fn index_mut(self, slice: &mut [T]) -> &mut [T] {
        ops::Range::from(self).index_mut(slice)
    }
}

/// 如果范围终点越过切片边界，`index` 和 `index_mut` 会 panic。
#[stable(feature = "slice_get_slice_impls", since = "1.15.0")]
#[rustc_const_unstable(feature = "const_index", issue = "143775")]
unsafe impl<T> const SliceIndex<[T]> for ops::RangeTo<usize> {
    type Output = [T];

    #[inline]
    fn get(self, slice: &[T]) -> Option<&[T]> {
        (0..self.end).get(slice)
    }

    #[inline]
    fn get_mut(self, slice: &mut [T]) -> Option<&mut [T]> {
        (0..self.end).get_mut(slice)
    }

    #[inline]
    unsafe fn get_unchecked(self, slice: *const [T]) -> *const [T] {
        // SAFETY: 调用方必须保证 `0..self.end` 位于 `slice` 内。
        unsafe { (0..self.end).get_unchecked(slice) }
    }

    #[inline]
    unsafe fn get_unchecked_mut(self, slice: *mut [T]) -> *mut [T] {
        // SAFETY: 调用方必须保证 `0..self.end` 位于 `slice` 内，并维护可变访问唯一性。
        unsafe { (0..self.end).get_unchecked_mut(slice) }
    }

    #[inline(always)]
    fn index(self, slice: &[T]) -> &[T] {
        (0..self.end).index(slice)
    }

    #[inline]
    fn index_mut(self, slice: &mut [T]) -> &mut [T] {
        (0..self.end).index_mut(slice)
    }
}

/// 如果范围起点越过切片边界，`index` 和 `index_mut` 会 panic。
#[stable(feature = "slice_get_slice_impls", since = "1.15.0")]
#[rustc_const_unstable(feature = "const_index", issue = "143775")]
unsafe impl<T> const SliceIndex<[T]> for ops::RangeFrom<usize> {
    type Output = [T];

    #[inline]
    fn get(self, slice: &[T]) -> Option<&[T]> {
        (self.start..slice.len()).get(slice)
    }

    #[inline]
    fn get_mut(self, slice: &mut [T]) -> Option<&mut [T]> {
        (self.start..slice.len()).get_mut(slice)
    }

    #[inline]
    unsafe fn get_unchecked(self, slice: *const [T]) -> *const [T] {
        // SAFETY: 调用方必须保证 `self.start..slice.len()` 是有效子范围。
        unsafe { (self.start..slice.len()).get_unchecked(slice) }
    }

    #[inline]
    unsafe fn get_unchecked_mut(self, slice: *mut [T]) -> *mut [T] {
        // SAFETY: 调用方必须保证范围有效，并维护返回可变子切片的唯一访问权。
        unsafe { (self.start..slice.len()).get_unchecked_mut(slice) }
    }

    #[inline]
    fn index(self, slice: &[T]) -> &[T] {
        if self.start > slice.len() {
            slice_index_fail(self.start, slice.len(), slice.len())
        }
        // SAFETY: 上面已经排除 `self.start > slice.len()`，长度计算位于边界内。
        unsafe {
            let new_len = crate::intrinsics::unchecked_sub(slice.len(), self.start);
            &*get_offset_len_noubcheck(slice, self.start, new_len)
        }
    }

    #[inline]
    fn index_mut(self, slice: &mut [T]) -> &mut [T] {
        if self.start > slice.len() {
            slice_index_fail(self.start, slice.len(), slice.len())
        }
        // SAFETY: 上面已经排除 `self.start > slice.len()`，长度计算位于边界内。
        unsafe {
            let new_len = crate::intrinsics::unchecked_sub(slice.len(), self.start);
            &mut *get_offset_len_mut_noubcheck(slice, self.start, new_len)
        }
    }
}

#[unstable(feature = "new_range_api", issue = "125687")]
#[rustc_const_unstable(feature = "const_index", issue = "143775")]
unsafe impl<T> const SliceIndex<[T]> for range::RangeFrom<usize> {
    type Output = [T];

    #[inline]
    fn get(self, slice: &[T]) -> Option<&[T]> {
        ops::RangeFrom::from(self).get(slice)
    }

    #[inline]
    fn get_mut(self, slice: &mut [T]) -> Option<&mut [T]> {
        ops::RangeFrom::from(self).get_mut(slice)
    }

    #[inline]
    unsafe fn get_unchecked(self, slice: *const [T]) -> *const [T] {
        // SAFETY: 调用方必须维护 `get_unchecked` 的边界契约；这里只转换范围类型。
        unsafe { ops::RangeFrom::from(self).get_unchecked(slice) }
    }

    #[inline]
    unsafe fn get_unchecked_mut(self, slice: *mut [T]) -> *mut [T] {
        // SAFETY: 调用方必须维护 `get_unchecked_mut` 的边界和唯一访问契约。
        unsafe { ops::RangeFrom::from(self).get_unchecked_mut(slice) }
    }

    #[inline]
    fn index(self, slice: &[T]) -> &[T] {
        ops::RangeFrom::from(self).index(slice)
    }

    #[inline]
    fn index_mut(self, slice: &mut [T]) -> &mut [T] {
        ops::RangeFrom::from(self).index_mut(slice)
    }
}

#[stable(feature = "slice_get_slice_impls", since = "1.15.0")]
#[rustc_const_unstable(feature = "const_index", issue = "143775")]
unsafe impl<T> const SliceIndex<[T]> for ops::RangeFull {
    type Output = [T];

    #[inline]
    fn get(self, slice: &[T]) -> Option<&[T]> {
        Some(slice)
    }

    #[inline]
    fn get_mut(self, slice: &mut [T]) -> Option<&mut [T]> {
        Some(slice)
    }

    #[inline]
    unsafe fn get_unchecked(self, slice: *const [T]) -> *const [T] {
        slice
    }

    #[inline]
    unsafe fn get_unchecked_mut(self, slice: *mut [T]) -> *mut [T] {
        slice
    }

    #[inline]
    fn index(self, slice: &[T]) -> &[T] {
        slice
    }

    #[inline]
    fn index_mut(self, slice: &mut [T]) -> &mut [T] {
        slice
    }
}

/// 如果出现下列情况，`index` 和 `index_mut` 会 panic：
/// - 范围终点是 `usize::MAX`，闭区间无法转换成排他上界；
/// - 范围起点大于终点；或
/// - 范围终点越过切片边界。
#[stable(feature = "inclusive_range", since = "1.26.0")]
#[rustc_const_unstable(feature = "const_index", issue = "143775")]
unsafe impl<T> const SliceIndex<[T]> for ops::RangeInclusive<usize> {
    type Output = [T];

    #[inline]
    fn get(self, slice: &[T]) -> Option<&[T]> {
        if *self.end() == usize::MAX { None } else { self.into_slice_range().get(slice) }
    }

    #[inline]
    fn get_mut(self, slice: &mut [T]) -> Option<&mut [T]> {
        if *self.end() == usize::MAX { None } else { self.into_slice_range().get_mut(slice) }
    }

    #[inline]
    unsafe fn get_unchecked(self, slice: *const [T]) -> *const [T] {
        // SAFETY: 调用方必须保证闭区间转换后的排他范围位于 `slice` 内。
        unsafe { self.into_slice_range().get_unchecked(slice) }
    }

    #[inline]
    unsafe fn get_unchecked_mut(self, slice: *mut [T]) -> *mut [T] {
        // SAFETY: 调用方必须保证闭区间转换后的排他范围有效，并维护唯一访问权。
        unsafe { self.into_slice_range().get_unchecked_mut(slice) }
    }

    #[inline]
    fn index(self, slice: &[T]) -> &[T] {
        let Self { mut start, mut end, exhausted } = self;
        let len = slice.len();
        if end < len {
            end = end + 1;
            start = if exhausted { end } else { start };
            if let Some(new_len) = usize::checked_sub(end, start) {
                // SAFETY: `end < len` 允许闭区间上界加一，`checked_sub` 证明 `start <= end`。
                unsafe { return &*get_offset_len_noubcheck(slice, start, new_len) }
            }
        }
        slice_index_fail(start, end, slice.len())
    }

    #[inline]
    fn index_mut(self, slice: &mut [T]) -> &mut [T] {
        let Self { mut start, mut end, exhausted } = self;
        let len = slice.len();
        if end < len {
            end = end + 1;
            start = if exhausted { end } else { start };
            if let Some(new_len) = usize::checked_sub(end, start) {
                // SAFETY: `end < len` 允许闭区间上界加一，`checked_sub` 证明 `start <= end`。
                unsafe { return &mut *get_offset_len_mut_noubcheck(slice, start, new_len) }
            }
        }
        slice_index_fail(start, end, slice.len())
    }
}

#[unstable(feature = "new_range_api", issue = "125687")]
#[rustc_const_unstable(feature = "const_index", issue = "143775")]
unsafe impl<T> const SliceIndex<[T]> for range::RangeInclusive<usize> {
    type Output = [T];

    #[inline]
    fn get(self, slice: &[T]) -> Option<&[T]> {
        ops::RangeInclusive::from(self).get(slice)
    }

    #[inline]
    fn get_mut(self, slice: &mut [T]) -> Option<&mut [T]> {
        ops::RangeInclusive::from(self).get_mut(slice)
    }

    #[inline]
    unsafe fn get_unchecked(self, slice: *const [T]) -> *const [T] {
        // SAFETY: 调用方必须维护 `get_unchecked` 的边界契约；这里只转换范围类型。
        unsafe { ops::RangeInclusive::from(self).get_unchecked(slice) }
    }

    #[inline]
    unsafe fn get_unchecked_mut(self, slice: *mut [T]) -> *mut [T] {
        // SAFETY: 调用方必须维护 `get_unchecked_mut` 的边界和唯一访问契约。
        unsafe { ops::RangeInclusive::from(self).get_unchecked_mut(slice) }
    }

    #[inline]
    fn index(self, slice: &[T]) -> &[T] {
        ops::RangeInclusive::from(self).index(slice)
    }

    #[inline]
    fn index_mut(self, slice: &mut [T]) -> &mut [T] {
        ops::RangeInclusive::from(self).index_mut(slice)
    }
}

/// 如果范围终点越过切片边界，`index` 和 `index_mut` 会 panic。
#[stable(feature = "inclusive_range", since = "1.26.0")]
#[rustc_const_unstable(feature = "const_index", issue = "143775")]
unsafe impl<T> const SliceIndex<[T]> for ops::RangeToInclusive<usize> {
    type Output = [T];

    #[inline]
    fn get(self, slice: &[T]) -> Option<&[T]> {
        (0..=self.end).get(slice)
    }

    #[inline]
    fn get_mut(self, slice: &mut [T]) -> Option<&mut [T]> {
        (0..=self.end).get_mut(slice)
    }

    #[inline]
    unsafe fn get_unchecked(self, slice: *const [T]) -> *const [T] {
        // SAFETY: 调用方必须保证 `0..=self.end` 位于 `slice` 内，且闭区间上界可加一。
        unsafe { (0..=self.end).get_unchecked(slice) }
    }

    #[inline]
    unsafe fn get_unchecked_mut(self, slice: *mut [T]) -> *mut [T] {
        // SAFETY: 调用方必须保证 `0..=self.end` 有效，并维护返回可变子切片的唯一访问权。
        unsafe { (0..=self.end).get_unchecked_mut(slice) }
    }

    #[inline]
    fn index(self, slice: &[T]) -> &[T] {
        (0..=self.end).index(slice)
    }

    #[inline]
    fn index_mut(self, slice: &mut [T]) -> &mut [T] {
        (0..=self.end).index_mut(slice)
    }
}

/// 如果范围终点越过切片边界，`index` 和 `index_mut` 会 panic。
#[stable(feature = "inclusive_range", since = "1.26.0")]
#[rustc_const_unstable(feature = "const_index", issue = "143775")]
unsafe impl<T> const SliceIndex<[T]> for range::RangeToInclusive<usize> {
    type Output = [T];

    #[inline]
    fn get(self, slice: &[T]) -> Option<&[T]> {
        (0..=self.last).get(slice)
    }

    #[inline]
    fn get_mut(self, slice: &mut [T]) -> Option<&mut [T]> {
        (0..=self.last).get_mut(slice)
    }

    #[inline]
    unsafe fn get_unchecked(self, slice: *const [T]) -> *const [T] {
        // SAFETY: 调用方必须保证 `0..=self.last` 位于 `slice` 内，且闭区间上界可加一。
        unsafe { (0..=self.last).get_unchecked(slice) }
    }

    #[inline]
    unsafe fn get_unchecked_mut(self, slice: *mut [T]) -> *mut [T] {
        // SAFETY: 调用方必须保证 `0..=self.last` 有效，并维护返回可变子切片的唯一访问权。
        unsafe { (0..=self.last).get_unchecked_mut(slice) }
    }

    #[inline]
    fn index(self, slice: &[T]) -> &[T] {
        (0..=self.last).index(slice)
    }

    #[inline]
    fn index_mut(self, slice: &mut [T]) -> &mut [T] {
        (0..=self.last).index_mut(slice)
    }
}

/// 对范围执行边界检查。
///
/// 这个方法类似切片上的 [`Index::index`]，但返回与 `range` 等价的 [`Range`]。
/// 调用方可以用它把任意范围语法规范化为 `start` 与排他 `end`。
///
/// `bounds` 是用于边界检查的切片范围；它应当是一个以切片长度为终点的 [`RangeTo`]。
///
/// 对同一个长度的切片，返回的 [`Range`] 可以安全地传给 [`slice::get_unchecked`] 和
/// [`slice::get_unchecked_mut`]；它已经排除了越界、反向范围和闭区间上界溢出。
///
/// [`Range`]: ops::Range
/// [`RangeTo`]: ops::RangeTo
/// [`slice::get_unchecked`]: slice::get_unchecked
/// [`slice::get_unchecked_mut`]: slice::get_unchecked_mut
///
/// # Panics
///
/// 如果 `range` 会越过 `bounds` 表示的切片边界，本函数会 panic。
///
/// # 示例
///
/// ```
/// #![feature(slice_range)]
///
/// use std::slice;
///
/// let v = [10, 40, 30];
/// assert_eq!(1..2, slice::range(1..2, ..v.len()));
/// assert_eq!(0..2, slice::range(..2, ..v.len()));
/// assert_eq!(1..3, slice::range(1.., ..v.len()));
/// ```
///
/// 当 [`Index::index`] 会 panic 时，本函数也会 panic：
///
/// ```should_panic
/// #![feature(slice_range)]
///
/// use std::slice;
///
/// let _ = slice::range(2..1, ..3);
/// ```
///
/// ```should_panic
/// #![feature(slice_range)]
///
/// use std::slice;
///
/// let _ = slice::range(1..4, ..3);
/// ```
///
/// ```should_panic
/// #![feature(slice_range)]
///
/// use std::slice;
///
/// let _ = slice::range(1..=usize::MAX, ..3);
/// ```
///
/// [`Index::index`]: ops::Index::index
#[track_caller]
#[unstable(feature = "slice_range", issue = "76393")]
#[must_use]
#[rustc_const_unstable(feature = "const_range", issue = "none")]
pub const fn range<R>(range: R, bounds: ops::RangeTo<usize>) -> ops::Range<usize>
where
    R: [const] ops::RangeBounds<usize> + [const] Destruct,
{
    let len = bounds.end;

    let end = match range.end_bound() {
        ops::Bound::Included(&end) if end >= len => slice_index_fail(0, end, len),
        // 因为 `end < len` 推出 `end < usize::MAX`，这里加一不会溢出。
        ops::Bound::Included(&end) => end + 1,

        ops::Bound::Excluded(&end) if end > len => slice_index_fail(0, end, len),
        ops::Bound::Excluded(&end) => end,
        ops::Bound::Unbounded => len,
    };

    let start = match range.start_bound() {
        ops::Bound::Excluded(&start) if start >= end => slice_index_fail(start, end, len),
        // 因为 `start < end` 推出 `start < usize::MAX`，这里加一不会溢出。
        ops::Bound::Excluded(&start) => start + 1,

        ops::Bound::Included(&start) if start > end => slice_index_fail(start, end, len),
        ops::Bound::Included(&start) => start,

        ops::Bound::Unbounded => 0,
    };

    ops::Range { start, end }
}

/// 对范围执行边界检查，但不 panic。
///
/// 这是 [`range()`] 的非 panic 版本；范围非法时返回 [`None`]。
///
/// # 示例
///
/// ```
/// #![feature(slice_range)]
///
/// use std::slice;
///
/// let v = [10, 40, 30];
/// assert_eq!(Some(1..2), slice::try_range(1..2, ..v.len()));
/// assert_eq!(Some(0..2), slice::try_range(..2, ..v.len()));
/// assert_eq!(Some(1..3), slice::try_range(1.., ..v.len()));
/// ```
///
/// 当 [`Index::index`] 会 panic 时返回 [`None`]：
///
/// ```
/// #![feature(slice_range)]
///
/// use std::slice;
///
/// assert_eq!(None, slice::try_range(2..1, ..3));
/// assert_eq!(None, slice::try_range(1..4, ..3));
/// assert_eq!(None, slice::try_range(1..=usize::MAX, ..3));
/// ```
///
/// [`Index::index`]: ops::Index::index
#[unstable(feature = "slice_range", issue = "76393")]
#[must_use]
pub fn try_range<R>(range: R, bounds: ops::RangeTo<usize>) -> Option<ops::Range<usize>>
where
    R: ops::RangeBounds<usize>,
{
    let len = bounds.end;

    let start = match range.start_bound() {
        ops::Bound::Included(&start) => start,
        ops::Bound::Excluded(start) => start.checked_add(1)?,
        ops::Bound::Unbounded => 0,
    };

    let end = match range.end_bound() {
        ops::Bound::Included(end) => end.checked_add(1)?,
        ops::Bound::Excluded(&end) => end,
        ops::Bound::Unbounded => len,
    };

    if start > end || end > len { None } else { Some(ops::Range { start, end }) }
}

/// 将一对 `ops::Bound` 转换成 `ops::Range`，不做任何边界检查，也不做 debug 溢出检查。
///
/// 调用方必须保证排除上界加一和下界加一不会溢出，并且后续会用相应的 `SliceIndex`
/// 实现检查范围是否合法；否则转换出的范围可能在 unsafe 路径中造成 UB。
pub(crate) const fn into_range_unchecked(
    len: usize,
    (start, end): (ops::Bound<usize>, ops::Bound<usize>),
) -> ops::Range<usize> {
    use ops::Bound;
    let start = match start {
        Bound::Included(i) => i,
        Bound::Excluded(i) => i + 1,
        Bound::Unbounded => 0,
    };
    let end = match end {
        Bound::Included(i) => i + 1,
        Bound::Excluded(i) => i,
        Bound::Unbounded => len,
    };
    start..end
}

/// 将一对 `ops::Bound` 转换成 `ops::Range`。
/// 如果包含上界或排除下界加一时发生溢出，返回 `None`。
#[rustc_const_unstable(feature = "const_range", issue = "none")]
pub(crate) const fn into_range(
    len: usize,
    (start, end): (ops::Bound<usize>, ops::Bound<usize>),
) -> Option<ops::Range<usize>> {
    use ops::Bound;
    let start = match start {
        Bound::Included(start) => start,
        Bound::Excluded(start) => start.checked_add(1)?,
        Bound::Unbounded => 0,
    };

    let end = match end {
        Bound::Included(end) => end.checked_add(1)?,
        Bound::Excluded(end) => end,
        Bound::Unbounded => len,
    };

    // 不在这里检查 `start < end` 和 `end <= len`，因为这些检查由 `Range` 的
    // `SliceIndex` 实现统一处理。

    Some(start..end)
}

/// 将一对 `ops::Bound` 转换成 `ops::Range`。
/// 如果索引溢出或越过给定切片长度，直接 panic。
pub(crate) fn into_slice_range(
    len: usize,
    (start, end): (ops::Bound<usize>, ops::Bound<usize>),
) -> ops::Range<usize> {
    let end = match end {
        ops::Bound::Included(end) if end >= len => slice_index_fail(0, end, len),
        // 因为 `end < len` 推出 `end < usize::MAX`，这里加一不会溢出。
        ops::Bound::Included(end) => end + 1,

        ops::Bound::Excluded(end) if end > len => slice_index_fail(0, end, len),
        ops::Bound::Excluded(end) => end,

        ops::Bound::Unbounded => len,
    };

    let start = match start {
        ops::Bound::Excluded(start) if start >= end => slice_index_fail(start, end, len),
        // 因为 `start < end` 推出 `start < usize::MAX`，这里加一不会溢出。
        ops::Bound::Excluded(start) => start + 1,

        ops::Bound::Included(start) if start > end => slice_index_fail(start, end, len),
        ops::Bound::Included(start) => start,

        ops::Bound::Unbounded => 0,
    };

    start..end
}

#[stable(feature = "slice_index_with_ops_bound_pair", since = "1.53.0")]
unsafe impl<T> SliceIndex<[T]> for (ops::Bound<usize>, ops::Bound<usize>) {
    type Output = [T];

    #[inline]
    fn get(self, slice: &[T]) -> Option<&Self::Output> {
        into_range(slice.len(), self)?.get(slice)
    }

    #[inline]
    fn get_mut(self, slice: &mut [T]) -> Option<&mut Self::Output> {
        into_range(slice.len(), self)?.get_mut(slice)
    }

    #[inline]
    unsafe fn get_unchecked(self, slice: *const [T]) -> *const Self::Output {
        // SAFETY: 调用方必须保证边界对转换后的范围位于 `slice` 内。
        unsafe { into_range_unchecked(slice.len(), self).get_unchecked(slice) }
    }

    #[inline]
    unsafe fn get_unchecked_mut(self, slice: *mut [T]) -> *mut Self::Output {
        // SAFETY: 调用方必须保证范围有效，并维护返回可变子切片的唯一访问权。
        unsafe { into_range_unchecked(slice.len(), self).get_unchecked_mut(slice) }
    }

    #[inline]
    fn index(self, slice: &[T]) -> &Self::Output {
        into_slice_range(slice.len(), self).index(slice)
    }

    #[inline]
    fn index_mut(self, slice: &mut [T]) -> &mut Self::Output {
        into_slice_range(slice.len(), self).index_mut(slice)
    }
}
