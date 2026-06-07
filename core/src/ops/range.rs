use crate::fmt;
use crate::hash::Hash;
use crate::marker::Destruct;
/// 一个无界区间(`..`)。
///
/// `RangeFull` 主要用作[切片索引][slicing index],其简写形式是 `..`。它不能充当
/// [`Iterator`],因为它没有起点。
///
/// # 示例
///
/// `..` 语法就是一个 `RangeFull`:
///
/// ```
/// assert_eq!(.., std::ops::RangeFull);
/// ```
///
/// 它没有 [`IntoIterator`] 实现,所以你不能直接在 `for` 循环里使用它。下面这段
/// 不会通过编译:
///
/// ```compile_fail,E0277
/// for i in .. {
///     // ...
/// }
/// ```
///
/// 用作[切片索引][slicing index]时,`RangeFull` 会产出整个数组构成的切片。
///
/// ```
/// let arr = [0, 1, 2, 3, 4];
/// assert_eq!(arr[ ..  ], [0, 1, 2, 3, 4]); // 这就是 `RangeFull`
/// assert_eq!(arr[ .. 3], [0, 1, 2      ]);
/// assert_eq!(arr[ ..=3], [0, 1, 2, 3   ]);
/// assert_eq!(arr[1..  ], [   1, 2, 3, 4]);
/// assert_eq!(arr[1.. 3], [   1, 2      ]);
/// assert_eq!(arr[1..=3], [   1, 2, 3   ]);
/// ```
///
/// [slicing index]: crate::slice::SliceIndex
#[lang = "RangeFull"]
#[doc(alias = "..")]
#[derive(Copy, Hash)]
#[derive_const(Clone, Default, Eq, PartialEq)]
#[stable(feature = "rust1", since = "1.0.0")]
pub struct RangeFull;

#[stable(feature = "rust1", since = "1.0.0")]
impl fmt::Debug for RangeFull {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, "..")
    }
}

/// 一个下界包含、上界不包含的(半开)区间(`start..end`)。
///
/// 区间 `start..end` 包含所有满足 `start <= x < end` 的值。当 `start >= end`
/// 时它为空。
///
/// # 示例
///
/// `start..end` 语法就是一个 `Range`:
///
/// ```
/// assert_eq!((3..5), std::ops::Range { start: 3, end: 5 });
/// assert_eq!(3 + 4 + 5, (3..6).sum());
/// ```
///
/// ```
/// let arr = [0, 1, 2, 3, 4];
/// assert_eq!(arr[ ..  ], [0, 1, 2, 3, 4]);
/// assert_eq!(arr[ .. 3], [0, 1, 2      ]);
/// assert_eq!(arr[ ..=3], [0, 1, 2, 3   ]);
/// assert_eq!(arr[1..  ], [   1, 2, 3, 4]);
/// assert_eq!(arr[1.. 3], [   1, 2      ]); // 这就是一个 `Range`
/// assert_eq!(arr[1..=3], [   1, 2, 3   ]);
/// ```
#[lang = "Range"]
#[doc(alias = "..")]
#[derive(Eq, Hash)]
#[derive_const(Clone, Default, PartialEq)] // 不是 Copy —— 见 #27186
#[stable(feature = "rust1", since = "1.0.0")]
pub struct Range<Idx> {
    /// 区间的下界(包含)。
    #[stable(feature = "rust1", since = "1.0.0")]
    pub start: Idx,
    /// 区间的上界(不包含)。
    #[stable(feature = "rust1", since = "1.0.0")]
    pub end: Idx,
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<Idx: fmt::Debug> fmt::Debug for Range<Idx> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.start.fmt(fmt)?;
        write!(fmt, "..")?;
        self.end.fmt(fmt)?;
        Ok(())
    }
}

impl<Idx: PartialOrd<Idx>> Range<Idx> {
    /// 如果 `item` 包含在该区间内,返回 `true`。
    ///
    /// # 示例
    ///
    /// ```
    /// assert!(!(3..5).contains(&2));
    /// assert!( (3..5).contains(&3));
    /// assert!( (3..5).contains(&4));
    /// assert!(!(3..5).contains(&5));
    ///
    /// assert!(!(3..3).contains(&3));
    /// assert!(!(3..2).contains(&3));
    ///
    /// assert!( (0.0..1.0).contains(&0.5));
    /// assert!(!(0.0..1.0).contains(&f32::NAN));
    /// assert!(!(0.0..f32::NAN).contains(&0.5));
    /// assert!(!(f32::NAN..1.0).contains(&0.5));
    /// ```
    #[inline]
    #[stable(feature = "range_contains", since = "1.35.0")]
    #[rustc_const_unstable(feature = "const_range", issue = "none")]
    pub const fn contains<U>(&self, item: &U) -> bool
    where
        Idx: [const] PartialOrd<U>,
        U: ?Sized + [const] PartialOrd<Idx>,
    {
        <Self as RangeBounds<Idx>>::contains(self, item)
    }

    /// 如果该区间不含任何元素,返回 `true`。
    ///
    /// # 示例
    ///
    /// ```
    /// assert!(!(3..5).is_empty());
    /// assert!( (3..3).is_empty());
    /// assert!( (3..2).is_empty());
    /// ```
    ///
    /// 如果两端中的任意一端不可比较(incomparable),该区间即为空:
    ///
    /// ```
    /// assert!(!(3.0..5.0).is_empty());
    /// assert!( (3.0..f32::NAN).is_empty());
    /// assert!( (f32::NAN..5.0).is_empty());
    /// ```
    #[inline]
    #[stable(feature = "range_is_empty", since = "1.47.0")]
    #[rustc_const_unstable(feature = "const_range", issue = "none")]
    pub const fn is_empty(&self) -> bool
    where
        Idx: [const] PartialOrd<Idx>,
    {
        !(self.start < self.end)
    }
}

/// 一个仅下界包含的区间(`start..`)。
///
/// `RangeFrom`(即 `start..`)包含所有满足 `x >= start` 的值。
///
/// *注意*:[`Iterator`] 实现中的溢出(当所含数据类型达到其数值上限时)允许 panic、
/// 回绕(wrap)或饱和(saturate)。这一行为由 [`Step`] trait 的实现决定。对于
/// 原生整数,它遵循通常的规则,并尊重溢出检查的配置(debug 下 panic,release 下
/// 回绕)。还要注意溢出发生得比你可能以为的更早:溢出发生在那次产出最大值的
/// `next` 调用之中,因为此时区间必须被设置到能产出下一个值的状态。
///
/// [`Step`]: crate::iter::Step
///
/// # 示例
///
/// `start..` 语法就是一个 `RangeFrom`:
///
/// ```
/// assert_eq!((2..), std::ops::RangeFrom { start: 2 });
/// assert_eq!(2 + 3 + 4, (2..).take(3).sum());
/// ```
///
/// ```
/// let arr = [0, 1, 2, 3, 4];
/// assert_eq!(arr[ ..  ], [0, 1, 2, 3, 4]);
/// assert_eq!(arr[ .. 3], [0, 1, 2      ]);
/// assert_eq!(arr[ ..=3], [0, 1, 2, 3   ]);
/// assert_eq!(arr[1..  ], [   1, 2, 3, 4]); // 这就是一个 `RangeFrom`
/// assert_eq!(arr[1.. 3], [   1, 2      ]);
/// assert_eq!(arr[1..=3], [   1, 2, 3   ]);
/// ```
#[lang = "RangeFrom"]
#[doc(alias = "..")]
#[derive(Eq, Hash)]
#[derive_const(Clone, PartialEq)] // 不是 Copy —— 见 #27186
#[stable(feature = "rust1", since = "1.0.0")]
pub struct RangeFrom<Idx> {
    /// 区间的下界(包含)。
    #[stable(feature = "rust1", since = "1.0.0")]
    pub start: Idx,
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<Idx: fmt::Debug> fmt::Debug for RangeFrom<Idx> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.start.fmt(fmt)?;
        write!(fmt, "..")?;
        Ok(())
    }
}

impl<Idx: PartialOrd<Idx>> RangeFrom<Idx> {
    /// 如果 `item` 包含在该区间内,返回 `true`。
    ///
    /// # 示例
    ///
    /// ```
    /// assert!(!(3..).contains(&2));
    /// assert!( (3..).contains(&3));
    /// assert!( (3..).contains(&1_000_000_000));
    ///
    /// assert!( (0.0..).contains(&0.5));
    /// assert!(!(0.0..).contains(&f32::NAN));
    /// assert!(!(f32::NAN..).contains(&0.5));
    /// ```
    #[inline]
    #[stable(feature = "range_contains", since = "1.35.0")]
    #[rustc_const_unstable(feature = "const_range", issue = "none")]
    pub const fn contains<U>(&self, item: &U) -> bool
    where
        Idx: [const] PartialOrd<U>,
        U: ?Sized + [const] PartialOrd<Idx>,
    {
        <Self as RangeBounds<Idx>>::contains(self, item)
    }
}

/// 一个仅上界不包含的区间(`..end`)。
///
/// `RangeTo`(即 `..end`)包含所有满足 `x < end` 的值。它不能充当 [`Iterator`],
/// 因为它没有起点。
///
/// # 示例
///
/// `..end` 语法就是一个 `RangeTo`:
///
/// ```
/// assert_eq!((..5), std::ops::RangeTo { end: 5 });
/// ```
///
/// 它没有 [`IntoIterator`] 实现,所以你不能直接在 `for` 循环里使用它。下面这段
/// 不会通过编译:
///
/// ```compile_fail,E0277
/// // error[E0277]: the trait bound `std::ops::RangeTo<{integer}>:
/// // std::iter::Iterator` is not satisfied
/// for i in ..5 {
///     // ...
/// }
/// ```
///
/// 用作[切片索引][slicing index]时,`RangeTo` 会产出由 `end` 所指位置之前的所有
/// 数组元素构成的切片。
///
/// ```
/// let arr = [0, 1, 2, 3, 4];
/// assert_eq!(arr[ ..  ], [0, 1, 2, 3, 4]);
/// assert_eq!(arr[ .. 3], [0, 1, 2      ]); // 这就是一个 `RangeTo`
/// assert_eq!(arr[ ..=3], [0, 1, 2, 3   ]);
/// assert_eq!(arr[1..  ], [   1, 2, 3, 4]);
/// assert_eq!(arr[1.. 3], [   1, 2      ]);
/// assert_eq!(arr[1..=3], [   1, 2, 3   ]);
/// ```
///
/// [slicing index]: crate::slice::SliceIndex
#[lang = "RangeTo"]
#[doc(alias = "..")]
#[derive(Copy, Eq, Hash)]
#[derive_const(Clone, PartialEq)]
#[stable(feature = "rust1", since = "1.0.0")]
pub struct RangeTo<Idx> {
    /// 区间的上界(不包含)。
    #[stable(feature = "rust1", since = "1.0.0")]
    pub end: Idx,
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<Idx: fmt::Debug> fmt::Debug for RangeTo<Idx> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, "..")?;
        self.end.fmt(fmt)?;
        Ok(())
    }
}

impl<Idx: PartialOrd<Idx>> RangeTo<Idx> {
    /// 如果 `item` 包含在该区间内,返回 `true`。
    ///
    /// # 示例
    ///
    /// ```
    /// assert!( (..5).contains(&-1_000_000_000));
    /// assert!( (..5).contains(&4));
    /// assert!(!(..5).contains(&5));
    ///
    /// assert!( (..1.0).contains(&0.5));
    /// assert!(!(..1.0).contains(&f32::NAN));
    /// assert!(!(..f32::NAN).contains(&0.5));
    /// ```
    #[inline]
    #[stable(feature = "range_contains", since = "1.35.0")]
    #[rustc_const_unstable(feature = "const_range", issue = "none")]
    pub const fn contains<U>(&self, item: &U) -> bool
    where
        Idx: [const] PartialOrd<U>,
        U: ?Sized + [const] PartialOrd<Idx>,
    {
        <Self as RangeBounds<Idx>>::contains(self, item)
    }
}

/// 一个下界与上界都包含的区间(`start..=end`)。
///
/// `RangeInclusive`(即 `start..=end`)包含所有满足 `x >= start` 且 `x <= end`
/// 的值。除非 `start <= end`,否则它为空。
///
/// 这个迭代器是 [fused(熔断)][fused] 的,但在迭代结束之后,`start` 和 `end` 的
/// 具体取值是**未指定的**(unspecified);唯一能保证的是:一旦不再产出任何值,
/// [`.is_empty()`] 就会返回 `true`。
///
/// [fused]: crate::iter::FusedIterator
/// [`.is_empty()`]: RangeInclusive::is_empty
///
/// # 示例
///
/// `start..=end` 语法就是一个 `RangeInclusive`:
///
/// ```
/// assert_eq!((3..=5), std::ops::RangeInclusive::new(3, 5));
/// assert_eq!(3 + 4 + 5, (3..=5).sum());
/// ```
///
/// ```
/// let arr = [0, 1, 2, 3, 4];
/// assert_eq!(arr[ ..  ], [0, 1, 2, 3, 4]);
/// assert_eq!(arr[ .. 3], [0, 1, 2      ]);
/// assert_eq!(arr[ ..=3], [0, 1, 2, 3   ]);
/// assert_eq!(arr[1..  ], [   1, 2, 3, 4]);
/// assert_eq!(arr[1.. 3], [   1, 2      ]);
/// assert_eq!(arr[1..=3], [   1, 2, 3   ]); // 这就是一个 `RangeInclusive`
/// ```
#[lang = "RangeInclusive"]
#[doc(alias = "..=")]
#[derive(Clone, Hash)]
#[derive_const(Eq, PartialEq)] // 不是 Copy —— 见 #27186
#[stable(feature = "inclusive_range", since = "1.26.0")]
pub struct RangeInclusive<Idx> {
    // 注意这里的字段不是公开的,这是为了将来能够修改其表示(representation);
    // 特别是,虽然我们大可以暴露 start/end,但在不改动(将来/现有)私有字段的
    // 情况下修改它们可能导致错误行为,所以我们不想支持那种用法。
    pub(crate) start: Idx,
    pub(crate) end: Idx,

    // 这个字段:
    //  - 在构造时为 `false`
    //  - 当迭代已产出一个元素、且迭代器尚未耗尽时为 `false`
    //  - 当迭代已被用来耗尽该迭代器时为 `true`
    //
    // 这是为了在不引入 PartialOrd 约束、也不使用特化(specialization)的前提下,
    // 支持 PartialEq 和 Hash 所必需的。
    pub(crate) exhausted: bool,
}

impl<Idx> RangeInclusive<Idx> {
    /// 创建一个新的闭区间(inclusive range)。等价于写 `start..=end`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::ops::RangeInclusive;
    ///
    /// assert_eq!(3..=5, RangeInclusive::new(3, 5));
    /// ```
    #[lang = "range_inclusive_new"]
    #[stable(feature = "inclusive_range_methods", since = "1.27.0")]
    #[inline]
    #[rustc_promotable]
    #[rustc_const_stable(feature = "const_range_new", since = "1.32.0")]
    pub const fn new(start: Idx, end: Idx) -> Self {
        Self { start, end, exhausted: false }
    }

    /// 返回区间的下界(包含)。
    ///
    /// 当把闭区间用于迭代时,在迭代结束之后 `start()` 与 [`end()`] 的取值都是未
    /// 指定的。要判断闭区间是否为空,请使用 [`is_empty()`] 方法,而不要去比较
    /// `start() > end()`。
    ///
    /// 注意:在该区间被迭代至耗尽之后,本方法返回的值是未指定的。
    ///
    /// [`end()`]: RangeInclusive::end
    /// [`is_empty()`]: RangeInclusive::is_empty
    ///
    /// # 示例
    ///
    /// ```
    /// assert_eq!((3..=5).start(), &3);
    /// ```
    #[stable(feature = "inclusive_range_methods", since = "1.27.0")]
    #[rustc_const_stable(feature = "const_inclusive_range_methods", since = "1.32.0")]
    #[inline]
    pub const fn start(&self) -> &Idx {
        &self.start
    }

    /// 返回区间的上界(包含)。
    ///
    /// 当把闭区间用于迭代时,在迭代结束之后 [`start()`] 与 `end()` 的取值都是未
    /// 指定的。要判断闭区间是否为空,请使用 [`is_empty()`] 方法,而不要去比较
    /// `start() > end()`。
    ///
    /// 注意:在该区间被迭代至耗尽之后,本方法返回的值是未指定的。
    ///
    /// [`start()`]: RangeInclusive::start
    /// [`is_empty()`]: RangeInclusive::is_empty
    ///
    /// # 示例
    ///
    /// ```
    /// assert_eq!((3..=5).end(), &5);
    /// ```
    #[stable(feature = "inclusive_range_methods", since = "1.27.0")]
    #[rustc_const_stable(feature = "const_inclusive_range_methods", since = "1.32.0")]
    #[inline]
    pub const fn end(&self) -> &Idx {
        &self.end
    }

    /// 把 `RangeInclusive` 解构为(下界,上界(包含))。
    ///
    /// 注意:在该区间被迭代至耗尽之后,本方法返回的值是未指定的。
    ///
    /// # 示例
    ///
    /// ```
    /// assert_eq!((3..=5).into_inner(), (3, 5));
    /// ```
    #[stable(feature = "inclusive_range_methods", since = "1.27.0")]
    #[inline]
    #[rustc_const_unstable(feature = "const_range_bounds", issue = "108082")]
    pub const fn into_inner(self) -> (Idx, Idx) {
        (self.start, self.end)
    }
}

impl RangeInclusive<usize> {
    /// 为 `SliceIndex` 实现转换为一个开区间(exclusive)`Range`。调用者需要自行
    /// 处理 `end == usize::MAX` 的情况。
    #[inline]
    pub(crate) const fn into_slice_range(self) -> Range<usize> {
        // 如果尚未耗尽,我们只是想切片 `start..end + 1`。
        // 如果已经耗尽,那么用 `end + 1..end + 1` 切片会得到一个空区间,但它对那个
        // 端点仍然要接受边界检查(bounds-check)。
        let exclusive_end = self.end + 1;
        let start = if self.exhausted { exclusive_end } else { self.start };
        start..exclusive_end
    }
}

#[stable(feature = "inclusive_range", since = "1.26.0")]
impl<Idx: fmt::Debug> fmt::Debug for RangeInclusive<Idx> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.start.fmt(fmt)?;
        write!(fmt, "..=")?;
        self.end.fmt(fmt)?;
        if self.exhausted {
            write!(fmt, " (exhausted)")?;
        }
        Ok(())
    }
}

impl<Idx: PartialOrd<Idx>> RangeInclusive<Idx> {
    /// 如果 `item` 包含在该区间内,返回 `true`。
    ///
    /// # 示例
    ///
    /// ```
    /// assert!(!(3..=5).contains(&2));
    /// assert!( (3..=5).contains(&3));
    /// assert!( (3..=5).contains(&4));
    /// assert!( (3..=5).contains(&5));
    /// assert!(!(3..=5).contains(&6));
    ///
    /// assert!( (3..=3).contains(&3));
    /// assert!(!(3..=2).contains(&3));
    ///
    /// assert!( (0.0..=1.0).contains(&1.0));
    /// assert!(!(0.0..=1.0).contains(&f32::NAN));
    /// assert!(!(0.0..=f32::NAN).contains(&0.0));
    /// assert!(!(f32::NAN..=1.0).contains(&1.0));
    /// ```
    ///
    /// 在迭代结束之后,本方法总是返回 `false`:
    ///
    /// ```
    /// let mut r = 3..=5;
    /// assert!(r.contains(&3) && r.contains(&5));
    /// for _ in r.by_ref() {}
    /// // 此处各字段的精确取值是未指定的
    /// assert!(!r.contains(&3) && !r.contains(&5));
    /// ```
    #[inline]
    #[stable(feature = "range_contains", since = "1.35.0")]
    #[rustc_const_unstable(feature = "const_range", issue = "none")]
    pub const fn contains<U>(&self, item: &U) -> bool
    where
        Idx: [const] PartialOrd<U>,
        U: ?Sized + [const] PartialOrd<Idx>,
    {
        <Self as RangeBounds<Idx>>::contains(self, item)
    }

    /// 如果该区间不含任何元素,返回 `true`。
    ///
    /// # 示例
    ///
    /// ```
    /// assert!(!(3..=5).is_empty());
    /// assert!(!(3..=3).is_empty());
    /// assert!( (3..=2).is_empty());
    /// ```
    ///
    /// 如果两端中的任意一端不可比较(incomparable),该区间即为空:
    ///
    /// ```
    /// assert!(!(3.0..=5.0).is_empty());
    /// assert!( (3.0..=f32::NAN).is_empty());
    /// assert!( (f32::NAN..=5.0).is_empty());
    /// ```
    ///
    /// 在迭代结束之后,本方法返回 `true`:
    ///
    /// ```
    /// let mut r = 3..=5;
    /// for _ in r.by_ref() {}
    /// // 此处各字段的精确取值是未指定的
    /// assert!(r.is_empty());
    /// ```
    #[stable(feature = "range_is_empty", since = "1.47.0")]
    #[inline]
    #[rustc_const_unstable(feature = "const_range", issue = "none")]
    pub const fn is_empty(&self) -> bool
    where
        Idx: [const] PartialOrd,
    {
        self.exhausted || !(self.start <= self.end)
    }
}

/// 一个仅上界包含的区间(`..=end`)。
///
/// `RangeToInclusive`(即 `..=end`)包含所有满足 `x <= end` 的值。它不能充当
/// [`Iterator`],因为它没有起点。
///
/// # 示例
///
/// `..=end` 语法就是一个 `RangeToInclusive`:
///
/// ```
/// assert_eq!((..=5), std::ops::RangeToInclusive{ end: 5 });
/// ```
///
/// 它没有 [`IntoIterator`] 实现,所以你不能直接在 `for` 循环里使用它。下面这段
/// 不会通过编译:
///
/// ```compile_fail,E0277
/// // error[E0277]: the trait bound `std::ops::RangeToInclusive<{integer}>:
/// // std::iter::Iterator` is not satisfied
/// for i in ..=5 {
///     // ...
/// }
/// ```
///
/// 用作[切片索引][slicing index]时,`RangeToInclusive` 会产出由 `end` 所指位置
/// 及其之前的所有数组元素构成的切片。
///
/// ```
/// let arr = [0, 1, 2, 3, 4];
/// assert_eq!(arr[ ..  ], [0, 1, 2, 3, 4]);
/// assert_eq!(arr[ .. 3], [0, 1, 2      ]);
/// assert_eq!(arr[ ..=3], [0, 1, 2, 3   ]); // 这就是一个 `RangeToInclusive`
/// assert_eq!(arr[1..  ], [   1, 2, 3, 4]);
/// assert_eq!(arr[1.. 3], [   1, 2      ]);
/// assert_eq!(arr[1..=3], [   1, 2, 3   ]);
/// ```
///
/// [slicing index]: crate::slice::SliceIndex
#[lang = "RangeToInclusive"]
#[doc(alias = "..=")]
#[derive(Copy, Hash)]
#[derive(Clone, PartialEq, Eq)]
#[stable(feature = "inclusive_range", since = "1.26.0")]
pub struct RangeToInclusive<Idx> {
    /// 区间的上界(包含)
    #[stable(feature = "inclusive_range", since = "1.26.0")]
    pub end: Idx,
}

#[stable(feature = "inclusive_range", since = "1.26.0")]
impl<Idx: fmt::Debug> fmt::Debug for RangeToInclusive<Idx> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(fmt, "..=")?;
        self.end.fmt(fmt)?;
        Ok(())
    }
}

impl<Idx: PartialOrd<Idx>> RangeToInclusive<Idx> {
    /// 如果 `item` 包含在该区间内,返回 `true`。
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
    #[stable(feature = "range_contains", since = "1.35.0")]
    #[rustc_const_unstable(feature = "const_range", issue = "none")]
    pub const fn contains<U>(&self, item: &U) -> bool
    where
        Idx: [const] PartialOrd<U>,
        U: ?Sized + [const] PartialOrd<Idx>,
    {
        <Self as RangeBounds<Idx>>::contains(self, item)
    }
}

// RangeToInclusive<Idx> 不能实现 From<RangeTo<Idx>>,
// 因为 (..0).into() 可能导致下溢(underflow)

/// 键区间(range of keys)的一个端点。
///
/// # 示例
///
/// `Bound` 就是区间的端点:
///
/// ```
/// use std::ops::Bound::*;
/// use std::ops::RangeBounds;
///
/// assert_eq!((..100).start_bound(), Unbounded);
/// assert_eq!((1..12).start_bound(), Included(&1));
/// assert_eq!((1..12).end_bound(), Excluded(&12));
/// ```
///
/// 把一个由 `Bound` 组成的元组用作 [`BTreeMap::range`] 的参数。注意在大多数情况下,
/// 改用区间语法(`1..5`)会更好。
///
/// ```
/// use std::collections::BTreeMap;
/// use std::ops::Bound::{Excluded, Included, Unbounded};
///
/// let mut map = BTreeMap::new();
/// map.insert(3, "a");
/// map.insert(5, "b");
/// map.insert(8, "c");
///
/// for (key, value) in map.range((Excluded(3), Included(8))) {
///     println!("{key}: {value}");
/// }
///
/// assert_eq!(Some((&3, &"a")), map.range((Unbounded, Included(5))).next());
/// ```
///
/// [`BTreeMap::range`]: ../../std/collections/btree_map/struct.BTreeMap.html#method.range
#[stable(feature = "collections_bound", since = "1.17.0")]
#[derive(Copy, Debug, Hash)]
#[derive_const(Clone, Eq, PartialEq)]
pub enum Bound<T> {
    /// 一个包含端点(inclusive bound)。
    #[stable(feature = "collections_bound", since = "1.17.0")]
    Included(#[stable(feature = "collections_bound", since = "1.17.0")] T),
    /// 一个不包含端点(exclusive bound)。
    #[stable(feature = "collections_bound", since = "1.17.0")]
    Excluded(#[stable(feature = "collections_bound", since = "1.17.0")] T),
    /// 一个无限端点。表示在这个方向上没有边界。
    #[stable(feature = "collections_bound", since = "1.17.0")]
    Unbounded,
}

impl<T> Bound<T> {
    /// 从 `&Bound<T>` 转换为 `Bound<&T>`。
    #[inline]
    #[stable(feature = "bound_as_ref_shared", since = "1.65.0")]
    #[rustc_const_unstable(feature = "const_range", issue = "none")]
    pub const fn as_ref(&self) -> Bound<&T> {
        match *self {
            Included(ref x) => Included(x),
            Excluded(ref x) => Excluded(x),
            Unbounded => Unbounded,
        }
    }

    /// 从 `&mut Bound<T>` 转换为 `Bound<&mut T>`。
    #[inline]
    #[unstable(feature = "bound_as_ref", issue = "80996")]
    pub const fn as_mut(&mut self) -> Bound<&mut T> {
        match *self {
            Included(ref mut x) => Included(x),
            Excluded(ref mut x) => Excluded(x),
            Unbounded => Unbounded,
        }
    }

    /// 通过对所含的值(`Included` 和 `Excluded` 两种情形都包括在内)应用一个函数,
    /// 把 `Bound<T>` 映射为 `Bound<U>`,并返回一个同种类的 `Bound`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::ops::Bound::*;
    ///
    /// let bound_string = Included("Hello, World!");
    ///
    /// assert_eq!(bound_string.map(|s| s.len()), Included(13));
    /// ```
    ///
    /// ```
    /// use std::ops::Bound;
    /// use Bound::*;
    ///
    /// let unbounded_string: Bound<String> = Unbounded;
    ///
    /// assert_eq!(unbounded_string.map(|s| s.len()), Unbounded);
    /// ```
    #[inline]
    #[stable(feature = "bound_map", since = "1.77.0")]
    pub fn map<U, F: FnOnce(T) -> U>(self, f: F) -> Bound<U> {
        match self {
            Unbounded => Unbounded,
            Included(x) => Included(f(x)),
            Excluded(x) => Excluded(f(x)),
        }
    }
}

impl<T: Copy> Bound<&T> {
    /// 通过复制(copy)端点的内容,把 `Bound<&T>` 映射为 `Bound<T>`。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(bound_copied)]
    ///
    /// use std::ops::Bound::*;
    /// use std::ops::RangeBounds;
    ///
    /// assert_eq!((1..12).start_bound(), Included(&1));
    /// assert_eq!((1..12).start_bound().copied(), Included(1));
    /// ```
    #[unstable(feature = "bound_copied", issue = "145966")]
    #[must_use]
    pub fn copied(self) -> Bound<T> {
        match self {
            Bound::Unbounded => Bound::Unbounded,
            Bound::Included(x) => Bound::Included(*x),
            Bound::Excluded(x) => Bound::Excluded(*x),
        }
    }
}

impl<T: Clone> Bound<&T> {
    /// 通过克隆(clone)端点的内容,把 `Bound<&T>` 映射为 `Bound<T>`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::ops::Bound::*;
    /// use std::ops::RangeBounds;
    ///
    /// let a1 = String::from("a");
    /// let (a2, a3, a4) = (a1.clone(), a1.clone(), a1.clone());
    ///
    /// assert_eq!(Included(&a1), (a2..).start_bound());
    /// assert_eq!(Included(a3), (a4..).start_bound().cloned());
    /// ```
    #[must_use = "`self` will be dropped if the result is not used"]
    #[stable(feature = "bound_cloned", since = "1.55.0")]
    #[rustc_const_unstable(feature = "const_range", issue = "none")]
    pub const fn cloned(self) -> Bound<T>
    where
        T: [const] Clone,
    {
        match self {
            Bound::Unbounded => Bound::Unbounded,
            Bound::Included(x) => Bound::Included(x.clone()),
            Bound::Excluded(x) => Bound::Excluded(x.clone()),
        }
    }
}

/// `RangeBounds` 由 Rust 内建的各种区间类型实现,这些类型由形如 `..`、`a..`、
/// `..b`、`..=c`、`d..e` 或 `f..=g` 的区间语法产生。
#[stable(feature = "collections_range", since = "1.28.0")]
#[rustc_diagnostic_item = "RangeBounds"]
#[rustc_const_unstable(feature = "const_range", issue = "none")]
pub const trait RangeBounds<T: ?Sized> {
    /// 起始索引端点。
    ///
    /// 以 `Bound` 的形式返回起始值。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::ops::Bound::*;
    /// use std::ops::RangeBounds;
    ///
    /// assert_eq!((..10).start_bound(), Unbounded);
    /// assert_eq!((3..10).start_bound(), Included(&3));
    /// ```
    #[stable(feature = "collections_range", since = "1.28.0")]
    fn start_bound(&self) -> Bound<&T>;

    /// 末尾索引端点。
    ///
    /// 以 `Bound` 的形式返回末尾值。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::ops::Bound::*;
    /// use std::ops::RangeBounds;
    ///
    /// assert_eq!((3..).end_bound(), Unbounded);
    /// assert_eq!((3..10).end_bound(), Excluded(&10));
    /// ```
    #[stable(feature = "collections_range", since = "1.28.0")]
    fn end_bound(&self) -> Bound<&T>;

    /// Returns `true` if `item` is contained in the range.
    ///
    /// # Examples
    ///
    /// ```
    /// assert!( (3..5).contains(&4));
    /// assert!(!(3..5).contains(&2));
    ///
    /// assert!( (0.0..1.0).contains(&0.5));
    /// assert!(!(0.0..1.0).contains(&f32::NAN));
    /// assert!(!(0.0..f32::NAN).contains(&0.5));
    /// assert!(!(f32::NAN..1.0).contains(&0.5));
    /// ```
    #[inline]
    #[stable(feature = "range_contains", since = "1.35.0")]
    fn contains<U>(&self, item: &U) -> bool
    where
        T: [const] PartialOrd<U>,
        U: ?Sized + [const] PartialOrd<T>,
    {
        (match self.start_bound() {
            Included(start) => start <= item,
            Excluded(start) => start < item,
            Unbounded => true,
        }) && (match self.end_bound() {
            Included(end) => item <= end,
            Excluded(end) => item < end,
            Unbounded => true,
        })
    }

    /// Returns `true` if the range contains no items.
    /// One-sided ranges (`RangeFrom`, etc) always return `false`.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(range_bounds_is_empty)]
    /// use std::ops::RangeBounds;
    ///
    /// assert!(!(3..).is_empty());
    /// assert!(!(..2).is_empty());
    /// assert!(!RangeBounds::is_empty(&(3..5)));
    /// assert!( RangeBounds::is_empty(&(3..3)));
    /// assert!( RangeBounds::is_empty(&(3..2)));
    /// ```
    ///
    /// The range is empty if either side is incomparable:
    ///
    /// ```
    /// #![feature(range_bounds_is_empty)]
    /// use std::ops::RangeBounds;
    ///
    /// assert!(!RangeBounds::is_empty(&(3.0..5.0)));
    /// assert!( RangeBounds::is_empty(&(3.0..f32::NAN)));
    /// assert!( RangeBounds::is_empty(&(f32::NAN..5.0)));
    /// ```
    ///
    /// But never empty if either side is unbounded:
    ///
    /// ```
    /// #![feature(range_bounds_is_empty)]
    /// use std::ops::RangeBounds;
    ///
    /// assert!(!(..0).is_empty());
    /// assert!(!(i32::MAX..).is_empty());
    /// assert!(!RangeBounds::<u8>::is_empty(&(..)));
    /// ```
    ///
    /// `(Excluded(a), Excluded(b))` is only empty if `a >= b`:
    ///
    /// ```
    /// #![feature(range_bounds_is_empty)]
    /// use std::ops::Bound::*;
    /// use std::ops::RangeBounds;
    ///
    /// assert!(!(Excluded(1), Excluded(3)).is_empty());
    /// assert!(!(Excluded(1), Excluded(2)).is_empty());
    /// assert!( (Excluded(1), Excluded(1)).is_empty());
    /// assert!( (Excluded(2), Excluded(1)).is_empty());
    /// assert!( (Excluded(3), Excluded(1)).is_empty());
    /// ```
    #[unstable(feature = "range_bounds_is_empty", issue = "137300")]
    fn is_empty(&self) -> bool
    where
        T: [const] PartialOrd,
    {
        !match (self.start_bound(), self.end_bound()) {
            (Unbounded, _) | (_, Unbounded) => true,
            (Included(start), Excluded(end))
            | (Excluded(start), Included(end))
            | (Excluded(start), Excluded(end)) => start < end,
            (Included(start), Included(end)) => start <= end,
        }
    }
}

/// Used to convert a range into start and end bounds, consuming the
/// range by value.
///
/// `IntoBounds` is implemented by Rust’s built-in range types, produced
/// by range syntax like `..`, `a..`, `..b`, `..=c`, `d..e`, or `f..=g`.
#[unstable(feature = "range_into_bounds", issue = "136903")]
#[rustc_const_unstable(feature = "const_range", issue = "none")]
pub const trait IntoBounds<T>: [const] RangeBounds<T> {
    /// Convert this range into the start and end bounds.
    /// Returns `(start_bound, end_bound)`.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(range_into_bounds)]
    /// use std::ops::Bound::*;
    /// use std::ops::IntoBounds;
    ///
    /// assert_eq!((0..5).into_bounds(), (Included(0), Excluded(5)));
    /// assert_eq!((..=7).into_bounds(), (Unbounded, Included(7)));
    /// ```
    fn into_bounds(self) -> (Bound<T>, Bound<T>);

    /// Compute the intersection of  `self` and `other`.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(range_into_bounds)]
    /// use std::ops::Bound::*;
    /// use std::ops::IntoBounds;
    ///
    /// assert_eq!((3..).intersect(..5), (Included(3), Excluded(5)));
    /// assert_eq!((-12..387).intersect(0..256), (Included(0), Excluded(256)));
    /// assert_eq!((1..5).intersect(..), (Included(1), Excluded(5)));
    /// assert_eq!((1..=9).intersect(0..10), (Included(1), Included(9)));
    /// assert_eq!((7..=13).intersect(8..13), (Included(8), Excluded(13)));
    /// ```
    ///
    /// Combine with `is_empty` to determine if two ranges overlap.
    ///
    /// ```
    /// #![feature(range_into_bounds)]
    /// #![feature(range_bounds_is_empty)]
    /// use std::ops::{RangeBounds, IntoBounds};
    ///
    /// assert!(!(3..).intersect(..5).is_empty());
    /// assert!(!(-12..387).intersect(0..256).is_empty());
    /// assert!((1..5).intersect(6..).is_empty());
    /// ```
    fn intersect<R>(self, other: R) -> (Bound<T>, Bound<T>)
    where
        Self: Sized,
        T: [const] Ord + [const] Destruct,
        R: Sized + [const] IntoBounds<T>,
    {
        let (self_start, self_end) = IntoBounds::into_bounds(self);
        let (other_start, other_end) = IntoBounds::into_bounds(other);

        let start = match (self_start, other_start) {
            (Included(a), Included(b)) => Included(Ord::max(a, b)),
            (Excluded(a), Excluded(b)) => Excluded(Ord::max(a, b)),
            (Unbounded, Unbounded) => Unbounded,

            (x, Unbounded) | (Unbounded, x) => x,

            (Included(i), Excluded(e)) | (Excluded(e), Included(i)) => {
                if i > e {
                    Included(i)
                } else {
                    Excluded(e)
                }
            }
        };
        let end = match (self_end, other_end) {
            (Included(a), Included(b)) => Included(Ord::min(a, b)),
            (Excluded(a), Excluded(b)) => Excluded(Ord::min(a, b)),
            (Unbounded, Unbounded) => Unbounded,

            (x, Unbounded) | (Unbounded, x) => x,

            (Included(i), Excluded(e)) | (Excluded(e), Included(i)) => {
                if i < e {
                    Included(i)
                } else {
                    Excluded(e)
                }
            }
        };

        (start, end)
    }
}

use self::Bound::{Excluded, Included, Unbounded};

#[stable(feature = "collections_range", since = "1.28.0")]
#[rustc_const_unstable(feature = "const_range", issue = "none")]
impl<T: ?Sized> const RangeBounds<T> for RangeFull {
    fn start_bound(&self) -> Bound<&T> {
        Unbounded
    }
    fn end_bound(&self) -> Bound<&T> {
        Unbounded
    }
}

#[unstable(feature = "range_into_bounds", issue = "136903")]
#[rustc_const_unstable(feature = "const_range", issue = "none")]
impl<T> const IntoBounds<T> for RangeFull {
    fn into_bounds(self) -> (Bound<T>, Bound<T>) {
        (Unbounded, Unbounded)
    }
}

#[stable(feature = "collections_range", since = "1.28.0")]
#[rustc_const_unstable(feature = "const_range", issue = "none")]
impl<T> const RangeBounds<T> for RangeFrom<T> {
    fn start_bound(&self) -> Bound<&T> {
        Included(&self.start)
    }
    fn end_bound(&self) -> Bound<&T> {
        Unbounded
    }
}

#[unstable(feature = "range_into_bounds", issue = "136903")]
#[rustc_const_unstable(feature = "const_range", issue = "none")]
impl<T> const IntoBounds<T> for RangeFrom<T> {
    fn into_bounds(self) -> (Bound<T>, Bound<T>) {
        (Included(self.start), Unbounded)
    }
}

#[stable(feature = "collections_range", since = "1.28.0")]
#[rustc_const_unstable(feature = "const_range", issue = "none")]
impl<T> const RangeBounds<T> for RangeTo<T> {
    fn start_bound(&self) -> Bound<&T> {
        Unbounded
    }
    fn end_bound(&self) -> Bound<&T> {
        Excluded(&self.end)
    }
}

#[unstable(feature = "range_into_bounds", issue = "136903")]
#[rustc_const_unstable(feature = "const_range", issue = "none")]
impl<T> const IntoBounds<T> for RangeTo<T> {
    fn into_bounds(self) -> (Bound<T>, Bound<T>) {
        (Unbounded, Excluded(self.end))
    }
}

#[stable(feature = "collections_range", since = "1.28.0")]
#[rustc_const_unstable(feature = "const_range", issue = "none")]
impl<T> const RangeBounds<T> for Range<T> {
    fn start_bound(&self) -> Bound<&T> {
        Included(&self.start)
    }
    fn end_bound(&self) -> Bound<&T> {
        Excluded(&self.end)
    }
}

#[unstable(feature = "range_into_bounds", issue = "136903")]
#[rustc_const_unstable(feature = "const_range", issue = "none")]
impl<T> const IntoBounds<T> for Range<T> {
    fn into_bounds(self) -> (Bound<T>, Bound<T>) {
        (Included(self.start), Excluded(self.end))
    }
}

#[stable(feature = "collections_range", since = "1.28.0")]
#[rustc_const_unstable(feature = "const_range", issue = "none")]
impl<T> const RangeBounds<T> for RangeInclusive<T> {
    fn start_bound(&self) -> Bound<&T> {
        Included(&self.start)
    }
    fn end_bound(&self) -> Bound<&T> {
        if self.exhausted {
            // When the iterator is exhausted, we usually have start == end,
            // but we want the range to appear empty, containing nothing.
            Excluded(&self.end)
        } else {
            Included(&self.end)
        }
    }
}

#[unstable(feature = "range_into_bounds", issue = "136903")]
#[rustc_const_unstable(feature = "const_range", issue = "none")]
impl<T> const IntoBounds<T> for RangeInclusive<T> {
    fn into_bounds(self) -> (Bound<T>, Bound<T>) {
        (
            Included(self.start),
            if self.exhausted {
                // When the iterator is exhausted, we usually have start == end,
                // but we want the range to appear empty, containing nothing.
                Excluded(self.end)
            } else {
                Included(self.end)
            },
        )
    }
}

#[stable(feature = "collections_range", since = "1.28.0")]
#[rustc_const_unstable(feature = "const_range", issue = "none")]
impl<T> const RangeBounds<T> for RangeToInclusive<T> {
    fn start_bound(&self) -> Bound<&T> {
        Unbounded
    }
    fn end_bound(&self) -> Bound<&T> {
        Included(&self.end)
    }
}

#[unstable(feature = "range_into_bounds", issue = "136903")]
#[rustc_const_unstable(feature = "const_range", issue = "none")]
impl<T> const IntoBounds<T> for RangeToInclusive<T> {
    fn into_bounds(self) -> (Bound<T>, Bound<T>) {
        (Unbounded, Included(self.end))
    }
}

#[stable(feature = "collections_range", since = "1.28.0")]
#[rustc_const_unstable(feature = "const_range", issue = "none")]
impl<T> const RangeBounds<T> for (Bound<T>, Bound<T>) {
    fn start_bound(&self) -> Bound<&T> {
        match *self {
            (Included(ref start), _) => Included(start),
            (Excluded(ref start), _) => Excluded(start),
            (Unbounded, _) => Unbounded,
        }
    }

    fn end_bound(&self) -> Bound<&T> {
        match *self {
            (_, Included(ref end)) => Included(end),
            (_, Excluded(ref end)) => Excluded(end),
            (_, Unbounded) => Unbounded,
        }
    }
}

#[unstable(feature = "range_into_bounds", issue = "136903")]
#[rustc_const_unstable(feature = "const_range", issue = "none")]
impl<T> const IntoBounds<T> for (Bound<T>, Bound<T>) {
    fn into_bounds(self) -> (Bound<T>, Bound<T>) {
        self
    }
}

#[stable(feature = "collections_range", since = "1.28.0")]
#[rustc_const_unstable(feature = "const_range", issue = "none")]
impl<'a, T: ?Sized + 'a> const RangeBounds<T> for (Bound<&'a T>, Bound<&'a T>) {
    fn start_bound(&self) -> Bound<&T> {
        self.0
    }

    fn end_bound(&self) -> Bound<&T> {
        self.1
    }
}

// This impl intentionally does not have `T: ?Sized`;
// see https://github.com/rust-lang/rust/pull/61584 for discussion of why.
//
/// If you need to use this implementation where `T` is unsized,
/// consider using the `RangeBounds` impl for a 2-tuple of [`Bound<&T>`][Bound],
/// i.e. replace `start..` with `(Bound::Included(start), Bound::Unbounded)`.
#[stable(feature = "collections_range", since = "1.28.0")]
#[rustc_const_unstable(feature = "const_range", issue = "none")]
impl<T> const RangeBounds<T> for RangeFrom<&T> {
    fn start_bound(&self) -> Bound<&T> {
        Included(self.start)
    }
    fn end_bound(&self) -> Bound<&T> {
        Unbounded
    }
}

// This impl intentionally does not have `T: ?Sized`;
// see https://github.com/rust-lang/rust/pull/61584 for discussion of why.
//
/// If you need to use this implementation where `T` is unsized,
/// consider using the `RangeBounds` impl for a 2-tuple of [`Bound<&T>`][Bound],
/// i.e. replace `..end` with `(Bound::Unbounded, Bound::Excluded(end))`.
#[stable(feature = "collections_range", since = "1.28.0")]
#[rustc_const_unstable(feature = "const_range", issue = "none")]
impl<T> const RangeBounds<T> for RangeTo<&T> {
    fn start_bound(&self) -> Bound<&T> {
        Unbounded
    }
    fn end_bound(&self) -> Bound<&T> {
        Excluded(self.end)
    }
}

// This impl intentionally does not have `T: ?Sized`;
// see https://github.com/rust-lang/rust/pull/61584 for discussion of why.
//
/// If you need to use this implementation where `T` is unsized,
/// consider using the `RangeBounds` impl for a 2-tuple of [`Bound<&T>`][Bound],
/// i.e. replace `start..end` with `(Bound::Included(start), Bound::Excluded(end))`.
#[stable(feature = "collections_range", since = "1.28.0")]
#[rustc_const_unstable(feature = "const_range", issue = "none")]
impl<T> const RangeBounds<T> for Range<&T> {
    fn start_bound(&self) -> Bound<&T> {
        Included(self.start)
    }
    fn end_bound(&self) -> Bound<&T> {
        Excluded(self.end)
    }
}

// This impl intentionally does not have `T: ?Sized`;
// see https://github.com/rust-lang/rust/pull/61584 for discussion of why.
//
/// If you need to use this implementation where `T` is unsized,
/// consider using the `RangeBounds` impl for a 2-tuple of [`Bound<&T>`][Bound],
/// i.e. replace `start..=end` with `(Bound::Included(start), Bound::Included(end))`.
#[stable(feature = "collections_range", since = "1.28.0")]
#[rustc_const_unstable(feature = "const_range", issue = "none")]
impl<T> const RangeBounds<T> for RangeInclusive<&T> {
    fn start_bound(&self) -> Bound<&T> {
        Included(self.start)
    }
    fn end_bound(&self) -> Bound<&T> {
        Included(self.end)
    }
}

// This impl intentionally does not have `T: ?Sized`;
// see https://github.com/rust-lang/rust/pull/61584 for discussion of why.
//
/// If you need to use this implementation where `T` is unsized,
/// consider using the `RangeBounds` impl for a 2-tuple of [`Bound<&T>`][Bound],
/// i.e. replace `..=end` with `(Bound::Unbounded, Bound::Included(end))`.
#[stable(feature = "collections_range", since = "1.28.0")]
#[rustc_const_unstable(feature = "const_range", issue = "none")]
impl<T> const RangeBounds<T> for RangeToInclusive<&T> {
    fn start_bound(&self) -> Bound<&T> {
        Unbounded
    }
    fn end_bound(&self) -> Bound<&T> {
        Included(self.end)
    }
}

/// An internal helper for `split_off` functions indicating
/// which end a `OneSidedRange` is bounded on.
#[unstable(feature = "one_sided_range", issue = "69780")]
#[allow(missing_debug_implementations)]
pub enum OneSidedRangeBound {
    /// The range is bounded inclusively from below and is unbounded above.
    StartInclusive,
    /// The range is bounded exclusively from above and is unbounded below.
    End,
    /// The range is bounded inclusively from above and is unbounded below.
    EndInclusive,
}

/// `OneSidedRange` is implemented for built-in range types that are unbounded
/// on one side. For example, `a..`, `..b` and `..=c` implement `OneSidedRange`,
/// but `..`, `d..e`, and `f..=g` do not.
///
/// Types that implement `OneSidedRange<T>` must return `Bound::Unbounded`
/// from one of `RangeBounds::start_bound` or `RangeBounds::end_bound`.
#[unstable(feature = "one_sided_range", issue = "69780")]
#[rustc_const_unstable(feature = "const_range", issue = "none")]
pub const trait OneSidedRange<T>: RangeBounds<T> {
    /// An internal-only helper function for `split_off` and
    /// `split_off_mut` that returns the bound of the one-sided range.
    fn bound(self) -> (OneSidedRangeBound, T);
}

#[unstable(feature = "one_sided_range", issue = "69780")]
#[rustc_const_unstable(feature = "const_range", issue = "none")]
impl<T> const OneSidedRange<T> for RangeTo<T>
where
    Self: RangeBounds<T>,
{
    fn bound(self) -> (OneSidedRangeBound, T) {
        (OneSidedRangeBound::End, self.end)
    }
}

#[unstable(feature = "one_sided_range", issue = "69780")]
#[rustc_const_unstable(feature = "const_range", issue = "none")]
impl<T> const OneSidedRange<T> for RangeFrom<T>
where
    Self: RangeBounds<T>,
{
    fn bound(self) -> (OneSidedRangeBound, T) {
        (OneSidedRangeBound::StartInclusive, self.start)
    }
}

#[unstable(feature = "one_sided_range", issue = "69780")]
#[rustc_const_unstable(feature = "const_range", issue = "none")]
impl<T> const OneSidedRange<T> for RangeToInclusive<T>
where
    Self: RangeBounds<T>,
{
    fn bound(self) -> (OneSidedRangeBound, T) {
        (OneSidedRangeBound::EndInclusive, self.end)
    }
}
