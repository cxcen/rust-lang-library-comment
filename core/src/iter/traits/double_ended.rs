use crate::num::NonZero;
use crate::ops::{ControlFlow, Try};

/// 能从两端产出元素的迭代器。
///
/// 实现 `DoubleEndedIterator` 的类型在 [`Iterator`] 的基础上增加了一项能力:
/// 除了从前端通过 [`next()`] 取元素，也可以从后端通过 [`next_back()`] 取元素。
///
/// 关键约束是两端操作的是同一个剩余区间。前端和后端不能交叉，不能重复产出同一
/// 个元素；当两端在中间相遇时，迭代结束。实现者必须让 [`next()`] 和
/// [`next_back()`] 共同维护同一份状态，例如一个前游标和一个后游标。
///
/// 与 [`Iterator`] 协议类似，`DoubleEndedIterator` 从 [`next_back()`] 返回
/// [`None`] 后，再次调用并不一定永久返回 [`None`]。从这个角度看，[`next()`] 和
/// [`next_back()`] 是同等的: 普通 trait 不提供 fused 保证，除非类型还实现
/// [`FusedIterator`](crate::iter::FusedIterator) 或被 [`Iterator::fuse`] 包装。
///
/// [`next_back()`]: DoubleEndedIterator::next_back
/// [`next()`]: Iterator::next
///
/// # 示例
///
/// 基本用法:
///
/// ```
/// let numbers = vec![1, 2, 3, 4, 5, 6];
///
/// let mut iter = numbers.iter();
///
/// assert_eq!(Some(&1), iter.next());
/// assert_eq!(Some(&6), iter.next_back());
/// assert_eq!(Some(&5), iter.next_back());
/// assert_eq!(Some(&2), iter.next());
/// assert_eq!(Some(&3), iter.next());
/// assert_eq!(Some(&4), iter.next());
/// assert_eq!(None, iter.next());
/// assert_eq!(None, iter.next_back());
/// ```
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_diagnostic_item = "DoubleEndedIterator"]
pub trait DoubleEndedIterator: Iterator {
    /// 从迭代器末端移除并返回一个元素。
    ///
    /// 没有更多元素时返回 `None`。
    ///
    /// trait 级文档说明了两端不重叠、结束后不默认 fused 等细节。
    ///
    /// [trait-level]: DoubleEndedIterator
    ///
    /// # 示例
    ///
    /// 基本用法:
    ///
    /// ```
    /// let numbers = vec![1, 2, 3, 4, 5, 6];
    ///
    /// let mut iter = numbers.iter();
    ///
    /// assert_eq!(Some(&1), iter.next());
    /// assert_eq!(Some(&6), iter.next_back());
    /// assert_eq!(Some(&5), iter.next_back());
    /// assert_eq!(Some(&2), iter.next());
    /// assert_eq!(Some(&3), iter.next());
    /// assert_eq!(Some(&4), iter.next());
    /// assert_eq!(None, iter.next());
    /// assert_eq!(None, iter.next_back());
    /// ```
    ///
    /// # 说明
    ///
    /// `DoubleEndedIterator` 方法产出的元素可能不同于只使用 [`Iterator`] 方法时的
    /// 结果。原因是适配器的状态可能依赖遍历方向；实现仍必须保证两端不会产出同一
    /// 个剩余元素。
    ///
    /// ```
    /// let vec = vec![(1, 'a'), (1, 'b'), (1, 'c'), (2, 'a'), (2, 'b')];
    /// let uniq_by_fst_comp = || {
    ///     let mut seen = std::collections::HashSet::new();
    ///     vec.iter().copied().filter(move |x| seen.insert(x.0))
    /// };
    ///
    /// assert_eq!(uniq_by_fst_comp().last(), Some((2, 'a')));
    /// assert_eq!(uniq_by_fst_comp().next_back(), Some((2, 'b')));
    ///
    /// assert_eq!(
    ///     uniq_by_fst_comp().fold(vec![], |mut v, x| {v.push(x); v}),
    ///     vec![(1, 'a'), (2, 'a')]
    /// );
    /// assert_eq!(
    ///     uniq_by_fst_comp().rfold(vec![], |mut v, x| {v.push(x); v}),
    ///     vec![(2, 'b'), (1, 'c')]
    /// );
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    fn next_back(&mut self) -> Option<Self::Item>;

    /// 从后端将迭代器推进 `n` 个元素。
    ///
    /// `advance_back_by` 是 [`advance_by`] 的反向版本。该方法会急切地从后端跳过
    /// 元素: 最多调用 [`next_back`] `n` 次，或在更早遇到 [`None`] 时停止。
    ///
    /// 如果成功推进 `n` 个元素，`advance_back_by(n)` 返回 `Ok(())`。如果中途遇到
    /// [`None`]，则返回 `Err(NonZero<usize>)`，其中 `k` 表示因为迭代器耗尽而未能
    /// 推进的剩余步数。若 `self` 为空且 `n` 非零，则返回 `Err(n)`；否则 `k` 总是
    /// 小于 `n`。
    ///
    /// 调用 `advance_back_by(0)` 也可能产生有意义的工作。例如 [`Flatten`] 可以推进
    /// 外层迭代器直到找到非空的内层迭代器，从而让之后的 `size_hint()` 比初始状态
    /// 更精确。
    ///
    /// [`advance_by`]: Iterator::advance_by
    /// [`Flatten`]: crate::iter::Flatten
    /// [`next_back`]: DoubleEndedIterator::next_back
    ///
    /// # 示例
    ///
    /// 基本用法:
    ///
    /// ```
    /// #![feature(iter_advance_by)]
    ///
    /// use std::num::NonZero;
    ///
    /// let a = [3, 4, 5, 6];
    /// let mut iter = a.iter();
    ///
    /// assert_eq!(iter.advance_back_by(2), Ok(()));
    /// assert_eq!(iter.next_back(), Some(&4));
    /// assert_eq!(iter.advance_back_by(0), Ok(()));
    /// assert_eq!(iter.advance_back_by(100), Err(NonZero::new(99).unwrap())); // 只跳过了 `&3`
    /// ```
    ///
    /// [`Ok(())`]: Ok
    /// [`Err(k)`]: Err
    #[inline]
    #[unstable(feature = "iter_advance_by", issue = "77404")]
    fn advance_back_by(&mut self, n: usize) -> Result<(), NonZero<usize>> {
        for i in 0..n {
            if self.next_back().is_none() {
                // SAFETY: `i` 始终小于 `n`。
                return Err(unsafe { NonZero::new_unchecked(n - i) });
            }
        }
        Ok(())
    }

    /// 返回从 iterator 末端数起第 `n` 个元素。
    ///
    /// 这本质上是 [`Iterator::nth()`] 的反向版本。和大多数索引操作一样，计数从
    /// 零开始，因此 `nth_back(0)` 返回末端的第一个值，`nth_back(1)` 返回末端的
    /// 第二个值，依此类推。
    ///
    /// 注意，从末端到被返回元素之间的所有元素都会被消耗，被返回的元素本身也会被
    /// 消耗。这也意味着，对同一个 iterator 多次调用 `nth_back(0)` 会返回不同元素。
    ///
    /// 如果 `n` 大于或等于 iterator 的长度，`nth_back()` 会返回 [`None`]。
    ///
    /// # 示例
    ///
    /// 基本用法:
    ///
    /// ```
    /// let a = [1, 2, 3];
    /// assert_eq!(a.iter().nth_back(2), Some(&1));
    /// ```
    ///
    /// 多次调用 `nth_back()` 不会倒回 iterator:
    ///
    /// ```
    /// let a = [1, 2, 3];
    ///
    /// let mut iter = a.iter();
    ///
    /// assert_eq!(iter.nth_back(1), Some(&2));
    /// assert_eq!(iter.nth_back(1), None);
    /// ```
    ///
    /// 元素少于 `n + 1` 个时返回 `None`:
    ///
    /// ```
    /// let a = [1, 2, 3];
    /// assert_eq!(a.iter().nth_back(10), None);
    /// ```
    #[inline]
    #[stable(feature = "iter_nth_back", since = "1.37.0")]
    fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
        if self.advance_back_by(n).is_err() {
            return None;
        }
        self.next_back()
    }

    /// 这是 [`Iterator::try_fold()`] 的反向版本: 它从 iterator 后端开始取得元素。
    ///
    /// # 示例
    ///
    /// 基本用法:
    ///
    /// ```
    /// let a = ["1", "2", "3"];
    /// let sum = a.iter()
    ///     .map(|&s| s.parse::<i32>())
    ///     .try_rfold(0, |acc, x| x.and_then(|y| Ok(acc + y)));
    /// assert_eq!(sum, Ok(6));
    /// ```
    ///
    /// 短路行为:
    ///
    /// ```
    /// let a = ["1", "rust", "3"];
    /// let mut it = a.iter();
    /// let sum = it
    ///     .by_ref()
    ///     .map(|&s| s.parse::<i32>())
    ///     .try_rfold(0, |acc, x| x.and_then(|y| Ok(acc + y)));
    /// assert!(sum.is_err());
    ///
    /// // 由于发生了短路，剩余元素仍可通过 iterator 取得。
    /// assert_eq!(it.next_back(), Some(&"1"));
    /// ```
    #[inline]
    #[stable(feature = "iterator_try_fold", since = "1.27.0")]
    fn try_rfold<B, F, R>(&mut self, init: B, mut f: F) -> R
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> R,
        R: Try<Output = B>,
    {
        let mut accum = init;
        while let Some(x) = self.next_back() {
            accum = f(accum, x)?;
        }
        try { accum }
    }

    /// 一个从后端开始把 iterator 元素归约为单个最终值的 iterator 方法。
    ///
    /// 这是 [`Iterator::fold()`] 的反向版本: 它从 iterator 后端开始取得元素。
    ///
    /// `rfold()` 接收两个参数: 一个初始值，以及一个带有两个参数的闭包。闭包的两个
    /// 参数分别是“累加器”和一个元素；闭包返回下一轮迭代中累加器应持有的值。
    ///
    /// 初始值就是第一次调用闭包时累加器持有的值。
    ///
    /// 将该闭包应用到 iterator 的每个元素之后，`rfold()` 返回累加器。
    ///
    /// 这个操作有时也称为“reduce”或“inject”。
    ///
    /// 当你有一组内容并希望从中产生单个值时，folding 很有用。
    ///
    /// 注意: `rfold()` 以*右结合*方式组合元素。对于 `+` 这类满足结合律的运算符，
    /// 元素组合顺序并不重要；但对于 `-` 这类不满足结合律的运算符，顺序会影响最终
    /// 结果。需要 `rfold()` 的*左结合*版本时，见 [`Iterator::fold()`]。
    ///
    /// # 示例
    ///
    /// 基本用法:
    ///
    /// ```
    /// let a = [1, 2, 3];
    ///
    /// // a 中所有元素的和
    /// let sum = a.iter()
    ///            .rfold(0, |acc, &x| acc + x);
    ///
    /// assert_eq!(sum, 6);
    /// ```
    ///
    /// 这个示例展示了 `rfold()` 的右结合性质: 它从初始值开始构造字符串，并从后往前
    /// 依次处理每个元素:
    ///
    /// ```
    /// let numbers = [1, 2, 3, 4, 5];
    ///
    /// let zero = "0".to_string();
    ///
    /// let result = numbers.iter().rfold(zero, |acc, &x| {
    ///     format!("({x} + {acc})")
    /// });
    ///
    /// assert_eq!(result, "(1 + (2 + (3 + (4 + (5 + 0)))))");
    /// ```
    #[doc(alias = "foldr")]
    #[inline]
    #[stable(feature = "iter_rfold", since = "1.27.0")]
    fn rfold<B, F>(mut self, init: B, mut f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        let mut accum = init;
        while let Some(x) = self.next_back() {
            accum = f(accum, x);
        }
        accum
    }

    /// 从后端开始搜索 iterator 中满足谓词的元素。
    ///
    /// `rfind()` 接收一个返回 `true` 或 `false` 的闭包。它从末端开始把该闭包应用到
    /// iterator 的每个元素；如果某个元素使闭包返回 `true`，则 `rfind()` 返回
    /// [`Some(element)`]。如果所有元素都返回 `false`，则返回 [`None`]。
    ///
    /// `rfind()` 会短路；换句话说，一旦闭包返回 `true`，它就会停止处理。
    ///
    /// 因为 `rfind()` 接收引用，而很多 iterator 本身就迭代引用，所以可能出现让人
    /// 困惑的“双重引用”参数。下面示例里的 `&&x` 展示了这种效果。
    ///
    /// [`Some(element)`]: Some
    ///
    /// # 示例
    ///
    /// 基本用法:
    ///
    /// ```
    /// let a = [1, 2, 3];
    ///
    /// assert_eq!(a.into_iter().rfind(|&x| x == 2), Some(2));
    /// assert_eq!(a.into_iter().rfind(|&x| x == 5), None);
    /// ```
    ///
    /// 迭代引用:
    ///
    /// ```
    /// let a = [1, 2, 3];
    ///
    /// // `iter()` 产出引用，即 `&i32`；而 `rfind()` 会取得每个元素的引用。
    /// assert_eq!(a.iter().rfind(|&&x| x == 2), Some(&2));
    /// assert_eq!(a.iter().rfind(|&&x| x == 5), None);
    /// ```
    ///
    /// 在第一个 `true` 处停止:
    ///
    /// ```
    /// let a = [1, 2, 3];
    ///
    /// let mut iter = a.iter();
    ///
    /// assert_eq!(iter.rfind(|&&x| x == 2), Some(&2));
    ///
    /// // 由于还有剩余元素，仍然可以继续使用 `iter`。
    /// assert_eq!(iter.next_back(), Some(&1));
    /// ```
    #[inline]
    #[stable(feature = "iter_rfind", since = "1.27.0")]
    fn rfind<P>(&mut self, predicate: P) -> Option<Self::Item>
    where
        Self: Sized,
        P: FnMut(&Self::Item) -> bool,
    {
        #[inline]
        fn check<T>(mut predicate: impl FnMut(&T) -> bool) -> impl FnMut((), T) -> ControlFlow<T> {
            move |(), x| {
                if predicate(&x) { ControlFlow::Break(x) } else { ControlFlow::Continue(()) }
            }
        }

        self.try_rfold((), check(predicate)).break_value()
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<'a, I: DoubleEndedIterator + ?Sized> DoubleEndedIterator for &'a mut I {
    fn next_back(&mut self) -> Option<I::Item> {
        (**self).next_back()
    }
    fn advance_back_by(&mut self, n: usize) -> Result<(), NonZero<usize>> {
        (**self).advance_back_by(n)
    }
    fn nth_back(&mut self, n: usize) -> Option<I::Item> {
        (**self).nth_back(n)
    }
    fn rfold<B, F>(self, init: B, f: F) -> B
    where
        F: FnMut(B, Self::Item) -> B,
    {
        self.spec_rfold(init, f)
    }
    fn try_rfold<B, F, R>(&mut self, init: B, f: F) -> R
    where
        F: FnMut(B, Self::Item) -> R,
        R: Try<Output = B>,
    {
        self.spec_try_rfold(init, f)
    }
}

/// 用于为 `&mut I where I: Sized` 特化 `rfold` 和 `rtry_fold` 的辅助 trait。
trait DoubleEndedIteratorRefSpec: DoubleEndedIterator {
    fn spec_rfold<B, F>(self, init: B, f: F) -> B
    where
        F: FnMut(B, Self::Item) -> B;

    fn spec_try_rfold<B, F, R>(&mut self, init: B, f: F) -> R
    where
        F: FnMut(B, Self::Item) -> R,
        R: Try<Output = B>;
}

impl<I: DoubleEndedIterator + ?Sized> DoubleEndedIteratorRefSpec for &mut I {
    default fn spec_rfold<B, F>(self, init: B, mut f: F) -> B
    where
        F: FnMut(B, Self::Item) -> B,
    {
        let mut accum = init;
        while let Some(x) = self.next_back() {
            accum = f(accum, x);
        }
        accum
    }

    default fn spec_try_rfold<B, F, R>(&mut self, init: B, mut f: F) -> R
    where
        F: FnMut(B, Self::Item) -> R,
        R: Try<Output = B>,
    {
        let mut accum = init;
        while let Some(x) = self.next_back() {
            accum = f(accum, x)?;
        }
        try { accum }
    }
}

impl<I: DoubleEndedIterator> DoubleEndedIteratorRefSpec for &mut I {
    impl_fold_via_try_fold! { spec_rfold -> spec_try_rfold }

    fn spec_try_rfold<B, F, R>(&mut self, init: B, f: F) -> R
    where
        F: FnMut(B, Self::Item) -> R,
        R: Try<Output = B>,
    {
        (**self).try_rfold(init, f)
    }
}
