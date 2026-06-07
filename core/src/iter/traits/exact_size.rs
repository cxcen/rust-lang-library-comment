/// 知道自身精确剩余长度的迭代器。
///
/// 很多 [`Iterator`] 无法在不执行的情况下知道还会产出多少项，但有些迭代器可以。
/// 当迭代器能维护精确长度时，把这个信息暴露出来有助于预分配、快速判断是否为空，
/// 以及实现需要知道末端位置的反向迭代。
///
/// 实现 `ExactSizeIterator` 时也必须实现 [`Iterator`]，并且
/// [`Iterator::size_hint`] *必须* 返回精确长度，即 `(n, Some(n))`。该精确长度表示
/// 接下来还会返回 `n` 次 [`Some`]，随后返回 [`None`]。
///
/// [`len`] 有默认实现，通常不需要手写。只有在能比默认的 `size_hint` 解包更高效时，
/// 覆盖它才有意义；覆盖后仍必须和 [`Iterator::size_hint`] 保持一致。
///
/// 需要特别注意: 这是 safe trait，因此它本身不能、也不会给 unsafe 代码提供可信
/// 保证。错误实现会违反 trait 协议，但 unsafe 代码 **不能** 仅凭
/// `ExactSizeIterator` 或 [`Iterator::size_hint`] 省略边界检查。若 unsafe 消费方
/// 需要把长度当作内存安全不变量，必须依赖 unstable 且 unsafe 的
/// [`TrustedLen`](super::marker::TrustedLen)。
///
/// [`len`]: ExactSizeIterator::len
///
/// # 适配器何时不应实现 `ExactSizeIterator`?
///
/// 如果某个适配器会让迭代器变得*更长*，它通常不应实现 `ExactSizeIterator`。
/// 内层精确长度迭代器本身可能已经长达 `usize::MAX`；再变长后，结果长度就无法再用
/// `usize` 精确表示。
///
/// 这就是 [`Chain<A, B>`](crate::iter::Chain) 即使在 `A` 和 `B` 都实现
/// `ExactSizeIterator` 时也不实现该 trait 的原因。
///
/// # 示例
///
/// 基本用法:
///
/// ```
/// // 有限 range 精确知道自己还会迭代多少次。
/// let five = 0..5;
///
/// assert_eq!(5, five.len());
/// ```
///
/// 在[模块级文档]中，我们实现了一个 [`Iterator`] `Counter`。现在也为它实现
/// `ExactSizeIterator`:
///
/// [模块级文档]: crate::iter
///
/// ```
/// # struct Counter {
/// #     count: usize,
/// # }
/// # impl Counter {
/// #     fn new() -> Counter {
/// #         Counter { count: 0 }
/// #     }
/// # }
/// # impl Iterator for Counter {
/// #     type Item = usize;
/// #     fn next(&mut self) -> Option<Self::Item> {
/// #         self.count += 1;
/// #         if self.count < 6 {
/// #             Some(self.count)
/// #         } else {
/// #             None
/// #         }
/// #     }
/// # }
/// impl ExactSizeIterator for Counter {
///     // 可以直接计算剩余迭代次数。
///     fn len(&self) -> usize {
///         5 - self.count
///     }
/// }
///
/// // 现在可以使用它。
///
/// let mut counter = Counter::new();
///
/// assert_eq!(5, counter.len());
/// let _ = counter.next();
/// assert_eq!(4, counter.len());
/// ```
#[stable(feature = "rust1", since = "1.0.0")]
pub trait ExactSizeIterator: Iterator {
    /// 返回迭代器的精确剩余长度。
    ///
    /// 正确实现保证在返回 [`None`] 前，迭代器还会恰好返回 `len()` 次 [`Some(T)`]。
    /// 该方法有默认实现，因此通常不应直接实现；如果可以提供更高效的实现，则可以
    /// 覆盖它。示例见 trait 级文档。
    ///
    /// 本函数与 [`Iterator::size_hint`] 具有相同的安全边界: safe 调用方可以把它作
    /// 为协议性信息使用，但 unsafe 代码不能仅凭它承担内存安全不变量。
    ///
    /// [trait-level]: ExactSizeIterator
    /// [`Some(T)`]: Some
    ///
    /// # 示例
    ///
    /// 基本用法:
    ///
    /// ```
    /// // 有限 range 精确知道自己还会迭代多少次。
    /// let mut range = 0..5;
    ///
    /// assert_eq!(5, range.len());
    /// let _ = range.next();
    /// assert_eq!(4, range.len());
    /// ```
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    fn len(&self) -> usize {
        let (lower, upper) = self.size_hint();
        // 注意: 这个断言偏防御性，但它检查了该 trait 承诺的不变量。
        // 如果这个 trait 只在 Rust 内部使用，可以用 debug_assert!；
        // assert_eq! 会同时检查所有用户实现。
        assert_eq!(upper, Some(lower));
        lower
    }

    /// 如果迭代器为空，返回 `true`。
    ///
    /// 该方法默认通过 [`ExactSizeIterator::len()`] 实现，因此通常不需要自行实现。
    ///
    /// # 示例
    ///
    /// 基本用法:
    ///
    /// ```
    /// #![feature(exact_size_is_empty)]
    ///
    /// let mut one_element = std::iter::once(0);
    /// assert!(!one_element.is_empty());
    ///
    /// assert_eq!(one_element.next(), Some(0));
    /// assert!(one_element.is_empty());
    ///
    /// assert_eq!(one_element.next(), None);
    /// ```
    #[inline]
    #[unstable(feature = "exact_size_is_empty", issue = "35428")]
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<I: ExactSizeIterator + ?Sized> ExactSizeIterator for &mut I {
    fn len(&self) -> usize {
        (**self).len()
    }
    fn is_empty(&self) -> bool {
        (**self).is_empty()
    }
}
