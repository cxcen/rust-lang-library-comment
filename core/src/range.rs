//! # 实验性的替代 range 类型
//!
//! 本模块中的类型计划在未来 edition 中取代现有的 `Range`、
//! `RangeInclusive` 和 `RangeFrom` 类型。
//!
//! ```
//! #![feature(new_range_api)]
//! use core::range::{Range, RangeFrom, RangeInclusive};
//!
//! let arr = [0, 1, 2, 3, 4];
//! assert_eq!(arr[                      ..   ], [0, 1, 2, 3, 4]);
//! assert_eq!(arr[                      .. 3 ], [0, 1, 2      ]);
//! assert_eq!(arr[                      ..=3 ], [0, 1, 2, 3   ]);
//! assert_eq!(arr[     RangeFrom::from(1..  )], [   1, 2, 3, 4]);
//! assert_eq!(arr[         Range::from(1..3 )], [   1, 2      ]);
//! assert_eq!(arr[RangeInclusive::from(1..=3)], [   1, 2, 3   ]);
//! ```

use crate::fmt;
use crate::hash::Hash;

mod iter;

#[unstable(feature = "new_range_api", issue = "125687")]
pub mod legacy;

use Bound::{Excluded, Included, Unbounded};
#[doc(inline)]
pub use iter::{RangeFromIter, RangeInclusiveIter, RangeIter};

#[doc(inline)]
pub use crate::iter::Step;
#[doc(inline)]
pub use crate::ops::{Bound, IntoBounds, OneSidedRange, RangeBounds, RangeFull, RangeTo};

/// 一个下界包含、上界排除的半开区间
/// (未来 edition 中的 `start..end`)。
///
/// 区间 `start..end` 包含所有满足 `start <= x < end` 的值。
/// 如果 `start >= end`,则该区间为空。
///
/// # 示例
///
/// ```
/// #![feature(new_range_api)]
/// use core::range::Range;
///
/// assert_eq!(Range::from(3..5), Range { start: 3, end: 5 });
/// assert_eq!(3 + 4 + 5, Range::from(3..6).into_iter().sum());
/// ```
#[lang = "RangeCopy"]
#[derive(Copy, Hash)]
#[derive_const(Clone, Default, PartialEq, Eq)]
#[unstable(feature = "new_range_api", issue = "125687")]
pub struct Range<Idx> {
    /// 区间的下界(包含)。
    #[unstable(feature = "new_range_api", issue = "125687")]
    pub start: Idx,
    /// 区间的上界(排除)。
    #[unstable(feature = "new_range_api", issue = "125687")]
    pub end: Idx,
}

#[unstable(feature = "new_range_api", issue = "125687")]
impl<Idx: fmt::Debug> fmt::Debug for Range<Idx> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.start.fmt(fmt)?;
        write!(fmt, "..")?;
        self.end.fmt(fmt)?;
        Ok(())
    }
}

impl<Idx: Step> Range<Idx> {
    /// 创建一个遍历该区间内元素的迭代器。
    ///
    /// 这是 `.clone().into_iter()` 的简写。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(new_range_api)]
    /// use core::range::Range;
    ///
    /// let mut i = Range::from(3..9).iter().map(|n| n*n);
    /// assert_eq!(i.next(), Some(9));
    /// assert_eq!(i.next(), Some(16));
    /// assert_eq!(i.next(), Some(25));
    /// ```
    #[unstable(feature = "new_range_api", issue = "125687")]
    #[inline]
    pub fn iter(&self) -> RangeIter<Idx> {
        self.clone().into_iter()
    }
}

impl<Idx: PartialOrd<Idx>> Range<Idx> {
    /// 如果 `item` 包含在区间内,则返回 `true`。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(new_range_api)]
    /// use core::range::Range;
    ///
    /// assert!(!Range::from(3..5).contains(&2));
    /// assert!( Range::from(3..5).contains(&3));
    /// assert!( Range::from(3..5).contains(&4));
    /// assert!(!Range::from(3..5).contains(&5));
    ///
    /// assert!(!Range::from(3..3).contains(&3));
    /// assert!(!Range::from(3..2).contains(&3));
    ///
    /// assert!( Range::from(0.0..1.0).contains(&0.5));
    /// assert!(!Range::from(0.0..1.0).contains(&f32::NAN));
    /// assert!(!Range::from(0.0..f32::NAN).contains(&0.5));
    /// assert!(!Range::from(f32::NAN..1.0).contains(&0.5));
    /// ```
    #[inline]
    #[unstable(feature = "new_range_api", issue = "125687")]
    #[rustc_const_unstable(feature = "const_range", issue = "none")]
    pub const fn contains<U>(&self, item: &U) -> bool
    where
        Idx: [const] PartialOrd<U>,
        U: ?Sized + [const] PartialOrd<Idx>,
    {
        <Self as RangeBounds<Idx>>::contains(self, item)
    }

    /// 如果区间不包含任何元素,则返回 `true`。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(new_range_api)]
    /// use core::range::Range;
    ///
    /// assert!(!Range::from(3..5).is_empty());
    /// assert!( Range::from(3..3).is_empty());
    /// assert!( Range::from(3..2).is_empty());
    /// ```
    ///
    /// 如果任一边界不可比较,该区间也为空:
    ///
    /// ```
    /// #![feature(new_range_api)]
    /// use core::range::Range;
    ///
    /// assert!(!Range::from(3.0..5.0).is_empty());
    /// assert!( Range::from(3.0..f32::NAN).is_empty());
    /// assert!( Range::from(f32::NAN..5.0).is_empty());
    /// ```
    #[inline]
    #[unstable(feature = "new_range_api", issue = "125687")]
    #[rustc_const_unstable(feature = "const_range", issue = "none")]
    pub const fn is_empty(&self) -> bool
    where
        Idx: [const] PartialOrd,
    {
        !(self.start < self.end)
    }
}

#[unstable(feature = "new_range_api", issue = "125687")]
#[rustc_const_unstable(feature = "const_range", issue = "none")]
impl<T> const RangeBounds<T> for Range<T> {
    fn start_bound(&self) -> Bound<&T> {
        Included(&self.start)
    }
    fn end_bound(&self) -> Bound<&T> {
        Excluded(&self.end)
    }
}

// 这个 impl 有意不带 `T: ?Sized`;
// 相关原因参见 https://github.com/rust-lang/rust/pull/61584 中的讨论。
//
/// 如果需要在 `T` 为 unsized 的场景中使用这类边界,
/// 请考虑使用 [`Bound<&T>`][Bound] 二元组的 `RangeBounds` impl,
/// 也就是用 `(Bound::Included(start), Bound::Excluded(end))` 替代 `start..end`。
#[unstable(feature = "new_range_api", issue = "125687")]
#[rustc_const_unstable(feature = "const_range", issue = "none")]
impl<T> const RangeBounds<T> for Range<&T> {
    fn start_bound(&self) -> Bound<&T> {
        Included(self.start)
    }
    fn end_bound(&self) -> Bound<&T> {
        Excluded(self.end)
    }
}

// #[unstable(feature = "range_into_bounds", issue = "136903")]
#[unstable(feature = "new_range_api", issue = "125687")]
#[rustc_const_unstable(feature = "const_range", issue = "none")]
impl<T> const IntoBounds<T> for Range<T> {
    fn into_bounds(self) -> (Bound<T>, Bound<T>) {
        (Included(self.start), Excluded(self.end))
    }
}

#[unstable(feature = "new_range_api", issue = "125687")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl<T> const From<Range<T>> for legacy::Range<T> {
    #[inline]
    fn from(value: Range<T>) -> Self {
        Self { start: value.start, end: value.end }
    }
}

#[unstable(feature = "new_range_api", issue = "125687")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl<T> const From<legacy::Range<T>> for Range<T> {
    #[inline]
    fn from(value: legacy::Range<T>) -> Self {
        Self { start: value.start, end: value.end }
    }
}

/// 一个下界和上界都包含的区间(`start..=last`)。
///
/// `RangeInclusive` `start..=last` 包含所有满足 `x >= start`
/// 且 `x <= last` 的值。除非 `start <= last`,否则它为空。
///
/// # 示例
///
/// `start..=last` 语法会产生一个 `RangeInclusive`:
///
/// ```
/// #![feature(new_range_api)]
/// use core::range::RangeInclusive;
///
/// assert_eq!(RangeInclusive::from(3..=5), RangeInclusive { start: 3, last: 5 });
/// assert_eq!(3 + 4 + 5, RangeInclusive::from(3..=5).into_iter().sum());
/// ```
#[lang = "RangeInclusiveCopy"]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
#[unstable(feature = "new_range_api", issue = "125687")]
pub struct RangeInclusive<Idx> {
    /// 区间的下界(包含)。
    #[unstable(feature = "new_range_api", issue = "125687")]
    pub start: Idx,
    /// 区间的上界(包含)。
    #[unstable(feature = "new_range_api", issue = "125687")]
    pub last: Idx,
}

#[unstable(feature = "new_range_api", issue = "125687")]
impl<Idx: fmt::Debug> fmt::Debug for RangeInclusive<Idx> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.start.fmt(fmt)?;
        write!(fmt, "..=")?;
        self.last.fmt(fmt)?;
        Ok(())
    }
}

impl<Idx: PartialOrd<Idx>> RangeInclusive<Idx> {
    /// 如果 `item` 包含在区间内,则返回 `true`。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(new_range_api)]
    /// use core::range::RangeInclusive;
    ///
    /// assert!(!RangeInclusive::from(3..=5).contains(&2));
    /// assert!( RangeInclusive::from(3..=5).contains(&3));
    /// assert!( RangeInclusive::from(3..=5).contains(&4));
    /// assert!( RangeInclusive::from(3..=5).contains(&5));
    /// assert!(!RangeInclusive::from(3..=5).contains(&6));
    ///
    /// assert!( RangeInclusive::from(3..=3).contains(&3));
    /// assert!(!RangeInclusive::from(3..=2).contains(&3));
    ///
    /// assert!( RangeInclusive::from(0.0..=1.0).contains(&1.0));
    /// assert!(!RangeInclusive::from(0.0..=1.0).contains(&f32::NAN));
    /// assert!(!RangeInclusive::from(0.0..=f32::NAN).contains(&0.0));
    /// assert!(!RangeInclusive::from(f32::NAN..=1.0).contains(&1.0));
    /// ```
    #[inline]
    #[unstable(feature = "new_range_api", issue = "125687")]
    #[rustc_const_unstable(feature = "const_range", issue = "none")]
    pub const fn contains<U>(&self, item: &U) -> bool
    where
        Idx: [const] PartialOrd<U>,
        U: ?Sized + [const] PartialOrd<Idx>,
    {
        <Self as RangeBounds<Idx>>::contains(self, item)
    }

    /// 如果区间不包含任何元素,则返回 `true`。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(new_range_api)]
    /// use core::range::RangeInclusive;
    ///
    /// assert!(!RangeInclusive::from(3..=5).is_empty());
    /// assert!(!RangeInclusive::from(3..=3).is_empty());
    /// assert!( RangeInclusive::from(3..=2).is_empty());
    /// ```
    ///
    /// 如果任一边界不可比较,该区间也为空:
    ///
    /// ```
    /// #![feature(new_range_api)]
    /// use core::range::RangeInclusive;
    ///
    /// assert!(!RangeInclusive::from(3.0..=5.0).is_empty());
    /// assert!( RangeInclusive::from(3.0..=f32::NAN).is_empty());
    /// assert!( RangeInclusive::from(f32::NAN..=5.0).is_empty());
    /// ```
    #[unstable(feature = "new_range_api", issue = "125687")]
    #[inline]
    #[rustc_const_unstable(feature = "const_range", issue = "none")]
    pub const fn is_empty(&self) -> bool
    where
        Idx: [const] PartialOrd,
    {
        !(self.start <= self.last)
    }
}

impl<Idx: Step> RangeInclusive<Idx> {
    /// 创建一个遍历该区间内元素的迭代器。
    ///
    /// 这是 `.clone().into_iter()` 的简写。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(new_range_api)]
    /// use core::range::RangeInclusive;
    ///
    /// let mut i = RangeInclusive::from(3..=8).iter().map(|n| n*n);
    /// assert_eq!(i.next(), Some(9));
    /// assert_eq!(i.next(), Some(16));
    /// assert_eq!(i.next(), Some(25));
    /// ```
    #[unstable(feature = "new_range_api", issue = "125687")]
    #[inline]
    pub fn iter(&self) -> RangeInclusiveIter<Idx> {
        self.clone().into_iter()
    }
}

impl RangeInclusive<usize> {
    /// 为 `SliceIndex` 实现转换成上界排除的 `Range`。
    /// 调用者负责处理 `last == usize::MAX` 的情况。
    #[inline]
    pub(crate) const fn into_slice_range(self) -> Range<usize> {
        Range { start: self.start, end: self.last + 1 }
    }
}

#[unstable(feature = "new_range_api", issue = "125687")]
#[rustc_const_unstable(feature = "const_range", issue = "none")]
impl<T> const RangeBounds<T> for RangeInclusive<T> {
    fn start_bound(&self) -> Bound<&T> {
        Included(&self.start)
    }
    fn end_bound(&self) -> Bound<&T> {
        Included(&self.last)
    }
}

// 这个 impl 有意不带 `T: ?Sized`;
// 相关原因参见 https://github.com/rust-lang/rust/pull/61584 中的讨论。
//
/// 如果需要在 `T` 为 unsized 的场景中使用这类边界,
/// 请考虑使用 [`Bound<&T>`][Bound] 二元组的 `RangeBounds` impl,
/// 也就是用 `(Bound::Included(start), Bound::Included(end))` 替代 `start..=end`。
#[unstable(feature = "new_range_api", issue = "125687")]
#[rustc_const_unstable(feature = "const_range", issue = "none")]
impl<T> const RangeBounds<T> for RangeInclusive<&T> {
    fn start_bound(&self) -> Bound<&T> {
        Included(self.start)
    }
    fn end_bound(&self) -> Bound<&T> {
        Included(self.last)
    }
}

// #[unstable(feature = "range_into_bounds", issue = "136903")]
#[unstable(feature = "new_range_api", issue = "125687")]
#[rustc_const_unstable(feature = "const_range", issue = "none")]
impl<T> const IntoBounds<T> for RangeInclusive<T> {
    fn into_bounds(self) -> (Bound<T>, Bound<T>) {
        (Included(self.start), Included(self.last))
    }
}

#[unstable(feature = "new_range_api", issue = "125687")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl<T> const From<RangeInclusive<T>> for legacy::RangeInclusive<T> {
    #[inline]
    fn from(value: RangeInclusive<T>) -> Self {
        Self::new(value.start, value.last)
    }
}
#[unstable(feature = "new_range_api", issue = "125687")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl<T> const From<legacy::RangeInclusive<T>> for RangeInclusive<T> {
    #[inline]
    fn from(value: legacy::RangeInclusive<T>) -> Self {
        assert!(
            !value.exhausted,
            "attempted to convert from an exhausted `legacy::RangeInclusive` (unspecified behavior)"
        );

        let (start, last) = value.into_inner();
        RangeInclusive { start, last }
    }
}

/// 一个只有包含下界的区间(`start..`)。
///
/// `RangeFrom` `start..` 包含所有满足 `x >= start` 的值。
///
/// *注意*: [`Iterator`] 实现中的溢出(也就是内部数据类型到达其数值极限时)
/// 可以 panic、回绕或饱和。该行为由 [`Step`] trait 的实现定义。对于原始整数,
/// 它遵循常规规则,并服从溢出检查配置(debug 中 panic,release 中回绕)。还要注意,
/// 溢出发生得可能比直觉更早:它会发生在产出最大值的那次 `next` 调用中,
/// 因为区间必须同时更新到能够产出下一个值的状态。
///
/// [`Step`]: crate::iter::Step
///
/// # 示例
///
/// `start..` 语法会产生一个 `RangeFrom`:
///
/// ```
/// #![feature(new_range_api)]
/// use core::range::RangeFrom;
///
/// assert_eq!(RangeFrom::from(2..), core::range::RangeFrom { start: 2 });
/// assert_eq!(2 + 3 + 4, RangeFrom::from(2..).into_iter().take(3).sum());
/// ```
#[lang = "RangeFromCopy"]
#[derive(Copy, Hash)]
#[derive_const(Clone, PartialEq, Eq)]
#[unstable(feature = "new_range_api", issue = "125687")]
pub struct RangeFrom<Idx> {
    /// 区间的下界(包含)。
    #[unstable(feature = "new_range_api", issue = "125687")]
    pub start: Idx,
}

#[unstable(feature = "new_range_api", issue = "125687")]
impl<Idx: fmt::Debug> fmt::Debug for RangeFrom<Idx> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.start.fmt(fmt)?;
        write!(fmt, "..")?;
        Ok(())
    }
}

impl<Idx: Step> RangeFrom<Idx> {
    /// 创建一个遍历该区间内元素的迭代器。
    ///
    /// 这是 `.clone().into_iter()` 的简写。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(new_range_api)]
    /// use core::range::RangeFrom;
    ///
    /// let mut i = RangeFrom::from(3..).iter().map(|n| n*n);
    /// assert_eq!(i.next(), Some(9));
    /// assert_eq!(i.next(), Some(16));
    /// assert_eq!(i.next(), Some(25));
    /// ```
    #[unstable(feature = "new_range_api", issue = "125687")]
    #[inline]
    pub fn iter(&self) -> RangeFromIter<Idx> {
        self.clone().into_iter()
    }
}

impl<Idx: PartialOrd<Idx>> RangeFrom<Idx> {
    /// 如果 `item` 包含在区间内,则返回 `true`。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(new_range_api)]
    /// use core::range::RangeFrom;
    ///
    /// assert!(!RangeFrom::from(3..).contains(&2));
    /// assert!( RangeFrom::from(3..).contains(&3));
    /// assert!( RangeFrom::from(3..).contains(&1_000_000_000));
    ///
    /// assert!( RangeFrom::from(0.0..).contains(&0.5));
    /// assert!(!RangeFrom::from(0.0..).contains(&f32::NAN));
    /// assert!(!RangeFrom::from(f32::NAN..).contains(&0.5));
    /// ```
    #[inline]
    #[unstable(feature = "new_range_api", issue = "125687")]
    #[rustc_const_unstable(feature = "const_range", issue = "none")]
    pub const fn contains<U>(&self, item: &U) -> bool
    where
        Idx: [const] PartialOrd<U>,
        U: ?Sized + [const] PartialOrd<Idx>,
    {
        <Self as RangeBounds<Idx>>::contains(self, item)
    }
}

#[unstable(feature = "new_range_api", issue = "125687")]
#[rustc_const_unstable(feature = "const_range", issue = "none")]
impl<T> const RangeBounds<T> for RangeFrom<T> {
    fn start_bound(&self) -> Bound<&T> {
        Included(&self.start)
    }
    fn end_bound(&self) -> Bound<&T> {
        Unbounded
    }
}

// 这个 impl 有意不带 `T: ?Sized`;
// 相关原因参见 https://github.com/rust-lang/rust/pull/61584 中的讨论。
//
/// 如果需要在 `T` 为 unsized 的场景中使用这类边界,
/// 请考虑使用 [`Bound<&T>`][Bound] 二元组的 `RangeBounds` impl,
/// 也就是用 `(Bound::Included(start), Bound::Unbounded)` 替代 `start..`。
#[unstable(feature = "new_range_api", issue = "125687")]
#[rustc_const_unstable(feature = "const_range", issue = "none")]
impl<T> const RangeBounds<T> for RangeFrom<&T> {
    fn start_bound(&self) -> Bound<&T> {
        Included(self.start)
    }
    fn end_bound(&self) -> Bound<&T> {
        Unbounded
    }
}

// #[unstable(feature = "range_into_bounds", issue = "136903")]
#[unstable(feature = "new_range_api", issue = "125687")]
#[rustc_const_unstable(feature = "const_range", issue = "none")]
impl<T> const IntoBounds<T> for RangeFrom<T> {
    fn into_bounds(self) -> (Bound<T>, Bound<T>) {
        (Included(self.start), Unbounded)
    }
}

#[unstable(feature = "new_range_api", issue = "125687")]
#[rustc_const_unstable(feature = "const_index", issue = "143775")]
impl<T> const From<RangeFrom<T>> for legacy::RangeFrom<T> {
    #[inline]
    fn from(value: RangeFrom<T>) -> Self {
        Self { start: value.start }
    }
}
#[unstable(feature = "new_range_api", issue = "125687")]
#[rustc_const_unstable(feature = "const_index", issue = "143775")]
impl<T> const From<legacy::RangeFrom<T>> for RangeFrom<T> {
    #[inline]
    fn from(value: legacy::RangeFrom<T>) -> Self {
        Self { start: value.start }
    }
}

/// 一个只有包含上界的区间(`..=last`)。
///
/// `RangeToInclusive` `..=last` 包含所有满足 `x <= last` 的值。
/// 因为它没有起点,所以不能作为 [`Iterator`]。
///
/// # 示例
///
/// `..=last` 语法会产生一个 `RangeToInclusive`:
///
/// ```
/// #![feature(new_range_api)]
/// #![feature(new_range)]
/// assert_eq!((..=5), std::range::RangeToInclusive{ last: 5 });
/// ```
///
/// 它没有 [`IntoIterator`] 实现,因此不能直接用于 `for` 循环。
/// 下面的代码无法编译:
///
/// ```compile_fail,E0277
/// // error[E0277]: the trait bound `std::range::RangeToInclusive<{integer}>:
/// // std::iter::Iterator` is not satisfied
/// for i in ..=5 {
///     // ...
/// }
/// ```
///
/// 当用作[切片索引]时,`RangeToInclusive` 会生成一个切片,其中包含
/// 从数组开头直到 `last` 指示索引(包含该索引)的所有元素。
///
/// ```
/// let arr = [0, 1, 2, 3, 4];
/// assert_eq!(arr[ ..  ], [0, 1, 2, 3, 4]);
/// assert_eq!(arr[ .. 3], [0, 1, 2      ]);
/// assert_eq!(arr[ ..=3], [0, 1, 2, 3   ]); // 这是一个 `RangeToInclusive`
/// assert_eq!(arr[1..  ], [   1, 2, 3, 4]);
/// assert_eq!(arr[1.. 3], [   1, 2      ]);
/// assert_eq!(arr[1..=3], [   1, 2, 3   ]);
/// ```
///
/// [切片索引]: crate::slice::SliceIndex
#[lang = "RangeToInclusiveCopy"]
#[doc(alias = "..=")]
#[derive(Copy, Clone, PartialEq, Eq, Hash)]
#[unstable(feature = "new_range_api", issue = "125687")]
pub struct RangeToInclusive<Idx> {
    /// 区间的上界(包含)。
    #[unstable(feature = "new_range_api", issue = "125687")]
    pub last: Idx,
}

#[unstable(feature = "new_range_api", issue = "125687")]
impl<Idx: fmt::Debug> fmt::Debug for RangeToInclusive<Idx> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, "..=")?;
        self.last.fmt(fmt)?;
        Ok(())
    }
}

impl<Idx: PartialOrd<Idx>> RangeToInclusive<Idx> {
    /// 如果 `item` 包含在区间内,则返回 `true`。
    ///
    /// # 示例
    ///
    /// ```
    /// assert!( (..=5).contains(&-1_000_000_000));
    /// assert!( (..=5).contains(&5));
    /// assert!(!(..=5).contains(&6));
    ///
    /// assert!( (..=1.0).contains(&1.0));
    /// assert!(!(..=1.0).contains(&f32::NAN));
    /// assert!(!(..=f32::NAN).contains(&0.5));
    /// ```
    #[inline]
    #[unstable(feature = "new_range_api", issue = "125687")]
    #[rustc_const_unstable(feature = "const_range", issue = "none")]
    pub const fn contains<U>(&self, item: &U) -> bool
    where
        Idx: [const] PartialOrd<U>,
        U: ?Sized + [const] PartialOrd<Idx>,
    {
        <Self as RangeBounds<Idx>>::contains(self, item)
    }
}

impl<T> From<legacy::RangeToInclusive<T>> for RangeToInclusive<T> {
    fn from(value: legacy::RangeToInclusive<T>) -> Self {
        Self { last: value.end }
    }
}

impl<T> From<RangeToInclusive<T>> for legacy::RangeToInclusive<T> {
    fn from(value: RangeToInclusive<T>) -> Self {
        Self { end: value.last }
    }
}

// RangeToInclusive<Idx> 不能实现 From<RangeTo<Idx>>,
// 因为 (..0).into() 可能产生下溢。

#[unstable(feature = "new_range_api", issue = "125687")]
#[rustc_const_unstable(feature = "const_range", issue = "none")]
impl<T> const RangeBounds<T> for RangeToInclusive<T> {
    fn start_bound(&self) -> Bound<&T> {
        Unbounded
    }
    fn end_bound(&self) -> Bound<&T> {
        Included(&self.last)
    }
}

#[unstable(feature = "range_into_bounds", issue = "136903")]
#[rustc_const_unstable(feature = "const_range", issue = "none")]
impl<T> const IntoBounds<T> for RangeToInclusive<T> {
    fn into_bounds(self) -> (Bound<T>, Bound<T>) {
        (Unbounded, Included(self.last))
    }
}
