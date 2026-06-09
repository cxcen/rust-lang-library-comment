use super::super::{
    ArrayChunks, ByRefSized, Chain, Cloned, Copied, Cycle, Enumerate, Filter, FilterMap, FlatMap,
    Flatten, Fuse, Inspect, Intersperse, IntersperseWith, Map, MapWhile, MapWindows, Peekable,
    Product, Rev, Scan, Skip, SkipWhile, StepBy, Sum, Take, TakeWhile, TrustedRandomAccessNoCoerce,
    Zip, try_process,
};
use super::TrustedLen;
use crate::array;
use crate::cmp::{self, Ordering};
use crate::num::NonZero;
use crate::ops::{ChangeOutputType, ControlFlow, FromResidual, Residual, Try};

fn _assert_is_dyn_compatible(_: &dyn Iterator<Item = ()>) {}

/// 处理迭代器的核心 trait。
///
/// 这是最主要的迭代器 trait。它描述“每次请求下一项时如何推进状态”，并把
/// 许多常见遍历操作建立在同一个 [`next`](Iterator::next) 契约之上。关于迭代器
/// 概念、`for` 循环脱糖、适配器和惰性求值的背景，请参阅[模块级文档]。如果要
/// 编写自己的迭代器，尤其应先阅读[实现 `Iterator`][impl] 的说明。
///
/// [模块级文档]: crate::iter
/// [impl]: crate::iter#implementing-iterator
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_on_unimplemented(
    on(
        Self = "core::ops::range::RangeTo<Idx>",
        note = "you might have meant to use a bounded `Range`"
    ),
    on(
        Self = "core::ops::range::RangeToInclusive<Idx>",
        note = "you might have meant to use a bounded `RangeInclusive`"
    ),
    label = "`{Self}` is not an iterator",
    message = "`{Self}` is not an iterator"
)]
#[doc(notable_trait)]
#[lang = "iterator"]
#[rustc_diagnostic_item = "Iterator"]
#[must_use = "iterators are lazy and do nothing unless consumed"]
pub trait Iterator {
    /// 迭代器逐项产出的元素类型。
    ///
    /// `Item` 是关联类型而不是泛型参数，因此同一个迭代器类型只能有一个
    /// 明确的产出类型。这让 `next`、`fold`、`collect` 等方法能围绕同一项类型
    /// 建立一致的协议。
    #[rustc_diagnostic_item = "IteratorItem"]
    #[stable(feature = "rust1", since = "1.0.0")]
    type Item;

    /// 推进迭代器并返回下一项。
    ///
    /// 每次调用都会给实现一次推进内部状态的机会。仍有元素时返回
    /// [`Some(Item)`]；当前序列结束时返回 [`None`]。普通 [`Iterator`] 的契约只
    /// 要求“本次调用没有下一项”时返回 [`None`]，并不要求第一次 [`None`] 之后
    /// 永远返回 [`None`]。也就是说，之后再次调用 `next()` 可能继续返回 [`None`]，
    /// 也可能在某些非 fused 迭代器上重新返回 [`Some(Item)`]。
    ///
    /// 需要“结束后永久结束”语义的调用方不应从 [`Iterator`] 本身推断该性质，
    /// 而应要求 [`FusedIterator`](crate::iter::FusedIterator)，或对迭代器调用
    /// [`fuse`](Iterator::fuse)。适配器实现也必须尊重这个边界: 除非自身显式
    /// 维护 fused 状态，否则不能把底层迭代器的一次 [`None`] 当成永久耗尽。
    ///
    /// [`Some(Item)`]: Some
    ///
    /// # 示例
    ///
    /// ```
    /// let a = [1, 2, 3];
    ///
    /// let mut iter = a.into_iter();
    ///
    /// // 调用 next() 会返回下一项...
    /// assert_eq!(Some(1), iter.next());
    /// assert_eq!(Some(2), iter.next());
    /// assert_eq!(Some(3), iter.next());
    ///
    /// // ...结束后返回 None。
    /// assert_eq!(None, iter.next());
    ///
    /// // 之后的调用不一定都返回 `None`。这个迭代器会一直返回 `None`。
    /// assert_eq!(None, iter.next());
    /// assert_eq!(None, iter.next());
    /// ```
    #[lang = "next"]
    #[stable(feature = "rust1", since = "1.0.0")]
    fn next(&mut self) -> Option<Self::Item>;

    /// 推进迭代器，并返回包含接下来 `N` 个值的数组。
    ///
    /// 如果剩余元素不足以填满数组，则返回 `Err`，其中包含还能取出的剩余元素
    /// 迭代器。成功时这 `N` 个元素已经从原迭代器中消耗；失败时也不会假装数组
    /// 已经完整构造，而是把实际取得的尾部元素交还给调用方继续处理。
    ///
    /// # 示例
    ///
    /// 基本用法:
    ///
    /// ```
    /// #![feature(iter_next_chunk)]
    ///
    /// let mut iter = "lorem".chars();
    ///
    /// assert_eq!(iter.next_chunk().unwrap(), ['l', 'o']);              // N 被推断为 2
    /// assert_eq!(iter.next_chunk().unwrap(), ['r', 'e', 'm']);         // N 被推断为 3
    /// assert_eq!(iter.next_chunk::<4>().unwrap_err().as_slice(), &[]); // N 被显式指定为 4
    /// ```
    ///
    /// 拆分字符串并取得前三项。
    ///
    /// ```
    /// #![feature(iter_next_chunk)]
    ///
    /// let quote = "not all those who wander are lost";
    /// let [first, second, third] = quote.split_whitespace().next_chunk().unwrap();
    /// assert_eq!(first, "not");
    /// assert_eq!(second, "all");
    /// assert_eq!(third, "those");
    /// ```
    #[inline]
    #[unstable(feature = "iter_next_chunk", issue = "98326")]
    fn next_chunk<const N: usize>(
        &mut self,
    ) -> Result<[Self::Item; N], array::IntoIter<Self::Item, N>>
    where
        Self: Sized,
    {
        array::iter_next_chunk(self)
    }

    /// 返回迭代器剩余长度的上下界。
    ///
    /// `size_hint()` 返回 `(lower, upper)`。`lower` 是实现承诺的下界: 后续最多
    /// 只能更准确，实际剩余元素数量不能少于它。`upper` 是可选上界:
    /// `Some(n)` 表示实际剩余元素数量不能大于 `n`；[`None`] 表示没有已知上界，
    /// 或上界无法用 [`usize`] 表示。
    ///
    /// # 实现说明
    ///
    /// [`Iterator`] 是 safe trait，类型系统不会强制实现真的产出声明数量的元素。
    /// 有 bug 的实现可能少于下界或多于上界。正因为如此，`size_hint()` 主要用于
    /// 预分配、选择算法路径等优化，unsafe 代码不能仅凭普通 `size_hint()` 省略
    /// 边界检查或假定内存已经初始化。错误的 `size_hint()` 是 trait 协议违规，
    /// 但不应单独导致内存安全问题。
    ///
    /// 实现者仍有责任尽可能给出正确估计。适配器组合时必须用饱和加法、
    /// `checked_add` 等方式维护上下界，不能因为溢出而把过小的值报告为精确上界。
    /// 如果能证明剩余长度精确，应返回 `(n, Some(n))`；如果只能证明下界，则返回
    /// 合理下界和 [`None`] 或保守上界。
    ///
    /// 默认实现返回 <code>(0, [None])</code>，这对任何迭代器都是保守且正确的。
    ///
    /// # 示例
    ///
    /// 基本用法:
    ///
    /// ```
    /// let a = [1, 2, 3];
    /// let mut iter = a.iter();
    ///
    /// assert_eq!((3, Some(3)), iter.size_hint());
    /// let _ = iter.next();
    /// assert_eq!((2, Some(2)), iter.size_hint());
    /// ```
    ///
    /// 更复杂的例子:
    ///
    /// ```
    /// // 零到九范围内的偶数。
    /// let iter = (0..10).filter(|x| x % 2 == 0);
    ///
    /// // 可能迭代零到十次。不实际执行 filter() 就无法知道精确值是五。
    /// assert_eq!((0, Some(10)), iter.size_hint());
    ///
    /// // 再用 chain() 接上五个数。
    /// let iter = (0..10).filter(|x| x % 2 == 0).chain(15..20);
    ///
    /// // 现在两个边界都增加五。
    /// assert_eq!((5, Some(15)), iter.size_hint());
    /// ```
    ///
    /// 上界返回 `None`:
    ///
    /// ```
    /// // 无限迭代器没有有限上界，并且下界饱和到最大 usize。
    /// let iter = 0..;
    ///
    /// assert_eq!((usize::MAX, None), iter.size_hint());
    /// ```
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, None)
    }

    /// 消耗迭代器，统计迭代次数并返回计数。
    ///
    /// 该方法会反复调用 [`next`]，直到遇到 [`None`]，并返回看到 [`Some`] 的次数。
    /// 即使迭代器没有任何元素，也至少需要调用一次 [`next`] 才能确认结束。
    ///
    /// [`next`]: Iterator::next
    ///
    /// # 溢出行为
    ///
    /// 该方法不会额外防护计数溢出。因此，如果迭代器元素数超过 [`usize::MAX`]，
    /// 结果要么错误，要么 panic；启用溢出检查时保证会 panic。
    ///
    /// # Panics
    ///
    /// 如果迭代器拥有超过 [`usize::MAX`] 个元素，本函数可能 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// let a = [1, 2, 3];
    /// assert_eq!(a.iter().count(), 3);
    ///
    /// let a = [1, 2, 3, 4, 5];
    /// assert_eq!(a.iter().count(), 5);
    /// ```
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    fn count(self) -> usize
    where
        Self: Sized,
    {
        self.fold(
            0,
            #[rustc_inherit_overflow_checks]
            |count, _| count + 1,
        )
    }

    /// 消耗 iterator，并返回最后一个元素。
    ///
    /// 该方法会一直求值 iterator，直到它返回 [`None`]。在此过程中，它会记录当前
    /// 元素。遇到 [`None`] 后，`last()` 返回它见到的最后一个元素。
    ///
    /// # Panics
    ///
    /// 如果 iterator 是无限的，该函数可能 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// let a = [1, 2, 3];
    /// assert_eq!(a.into_iter().last(), Some(3));
    ///
    /// let a = [1, 2, 3, 4, 5];
    /// assert_eq!(a.into_iter().last(), Some(5));
    /// ```
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    fn last(self) -> Option<Self::Item>
    where
        Self: Sized,
    {
        #[inline]
        fn some<T>(_: Option<T>, x: T) -> Option<T> {
            Some(x)
        }

        self.fold(None, some)
    }

    /// 将迭代器向前推进 `n` 个元素。
    ///
    /// 该方法会急切地跳过元素: 最多调用 [`next`] `n` 次，或在更早遇到 [`None`] 时
    /// 停止。与大多数适配器不同，调用 `advance_by` 本身就是消费操作，会立即推进
    /// 底层状态。
    ///
    /// 如果成功推进 `n` 个元素，`advance_by(n)` 返回 `Ok(())`。如果中途遇到
    /// [`None`]，则返回 `Err(NonZero<usize>)`，其中的 `k` 表示还剩多少步未能推进，
    /// 因为迭代器已经耗尽。若 `self` 为空且 `n` 非零，则返回 `Err(n)`；否则
    /// `k` 总是小于 `n`。
    ///
    /// 调用 `advance_by(0)` 也可能产生有意义的工作。例如 [`Flatten`] 可以推进外层
    /// 迭代器直到找到非空的内层迭代器，从而让之后的 `size_hint()` 比初始状态更精确。
    ///
    /// [`Flatten`]: crate::iter::Flatten
    /// [`next`]: Iterator::next
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(iter_advance_by)]
    ///
    /// use std::num::NonZero;
    ///
    /// let a = [1, 2, 3, 4];
    /// let mut iter = a.into_iter();
    ///
    /// assert_eq!(iter.advance_by(2), Ok(()));
    /// assert_eq!(iter.next(), Some(3));
    /// assert_eq!(iter.advance_by(0), Ok(()));
    /// assert_eq!(iter.advance_by(100), Err(NonZero::new(99).unwrap())); // 只跳过了 `4`
    /// ```
    #[inline]
    #[unstable(feature = "iter_advance_by", issue = "77404")]
    fn advance_by(&mut self, n: usize) -> Result<(), NonZero<usize>> {
        /// 辅助 trait: 为 `Sized` 迭代器通过 `try_fold` 特化 `advance_by`。
        trait SpecAdvanceBy {
            fn spec_advance_by(&mut self, n: usize) -> Result<(), NonZero<usize>>;
        }

        impl<I: Iterator + ?Sized> SpecAdvanceBy for I {
            default fn spec_advance_by(&mut self, n: usize) -> Result<(), NonZero<usize>> {
                for i in 0..n {
                    if self.next().is_none() {
                        // SAFETY: `i` 始终小于 `n`。
                        return Err(unsafe { NonZero::new_unchecked(n - i) });
                    }
                }
                Ok(())
            }
        }

        impl<I: Iterator> SpecAdvanceBy for I {
            fn spec_advance_by(&mut self, n: usize) -> Result<(), NonZero<usize>> {
                let Some(n) = NonZero::new(n) else {
                    return Ok(());
                };

                let res = self.try_fold(n, |n, _| NonZero::new(n.get() - 1));

                match res {
                    None => Ok(()),
                    Some(n) => Err(n),
                }
            }
        }

        self.spec_advance_by(n)
    }

    /// 返回 iterator 的第 `n` 个元素。
    ///
    /// 和大多数索引操作一样，计数从零开始，因此 `nth(0)` 返回第一个值，`nth(1)`
    /// 返回第二个值，依此类推。
    ///
    /// 注意，所有前置元素以及被返回的元素都会从 iterator 中消耗掉。这意味着前置
    /// 元素会被丢弃，也意味着对同一个 iterator 多次调用 `nth(0)` 会返回不同元素。
    ///
    /// 如果 `n` 大于或等于 iterator 的长度，`nth()` 会返回 [`None`]。
    ///
    /// # 示例
    ///
    /// 基本用法:
    ///
    /// ```
    /// let a = [1, 2, 3];
    /// assert_eq!(a.into_iter().nth(1), Some(2));
    /// ```
    ///
    /// 多次调用 `nth()` 不会倒回 iterator:
    ///
    /// ```
    /// let a = [1, 2, 3];
    ///
    /// let mut iter = a.into_iter();
    ///
    /// assert_eq!(iter.nth(1), Some(2));
    /// assert_eq!(iter.nth(1), None);
    /// ```
    ///
    /// 元素少于 `n + 1` 个时返回 `None`:
    ///
    /// ```
    /// let a = [1, 2, 3];
    /// assert_eq!(a.into_iter().nth(10), None);
    /// ```
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        self.advance_by(n).ok()?;
        self.next()
    }

    /// 创建一个从同一点开始、每次迭代按给定步长前进的 iterator。
    ///
    /// 注意 1: 无论给定步长是多少，iterator 的第一个元素总会被返回。
    ///
    /// 注意 2: 被忽略元素实际被拉取的时机并不固定。`StepBy` 可以表现得像
    /// `self.next()`、`self.nth(step-1)`、`self.nth(step-1)`、... 这样的序列；
    /// 也可以表现得像 `advance_n_and_return_first(&mut self, step)`、
    /// `advance_n_and_return_first(&mut self, step)`、... 这样的序列。出于性能原因，
    /// 某些 iterator 采用哪种方式可能会变化。第二种方式会更早推进 iterator，
    /// 因而可能消耗更多元素。
    ///
    /// `advance_n_and_return_first` 等价于:
    /// ```
    /// fn advance_n_and_return_first<I>(iter: &mut I, n: usize) -> Option<I::Item>
    /// where
    ///     I: Iterator,
    /// {
    ///     let next = iter.next();
    ///     if n > 1 {
    ///         iter.nth(n - 2);
    ///     }
    ///     next
    /// }
    /// ```
    ///
    /// # Panics
    ///
    /// 如果给定步长为 `0`，该方法会 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// let a = [0, 1, 2, 3, 4, 5];
    /// let mut iter = a.into_iter().step_by(2);
    ///
    /// assert_eq!(iter.next(), Some(0));
    /// assert_eq!(iter.next(), Some(2));
    /// assert_eq!(iter.next(), Some(4));
    /// assert_eq!(iter.next(), None);
    /// ```
    #[inline]
    #[stable(feature = "iterator_step_by", since = "1.28.0")]
    fn step_by(self, step: usize) -> StepBy<Self>
    where
        Self: Sized,
    {
        StepBy::new(self, step)
    }

    /// 接收两个 iterator，并创建一个按顺序遍历二者的新 iterator。
    ///
    /// `chain()` 返回的新 iterator 会先遍历第一个 iterator 产出的值，再遍历第二个
    /// iterator 产出的值。
    ///
    /// 换句话说，它把两个 iterator 首尾相接成一条链。
    ///
    /// [`once`] 常用于把单个值适配成可接到其他迭代过程中的一段 iterator。
    ///
    /// # 示例
    ///
    /// 基本用法:
    ///
    /// ```
    /// let s1 = "abc".chars();
    /// let s2 = "def".chars();
    ///
    /// let mut iter = s1.chain(s2);
    ///
    /// assert_eq!(iter.next(), Some('a'));
    /// assert_eq!(iter.next(), Some('b'));
    /// assert_eq!(iter.next(), Some('c'));
    /// assert_eq!(iter.next(), Some('d'));
    /// assert_eq!(iter.next(), Some('e'));
    /// assert_eq!(iter.next(), Some('f'));
    /// assert_eq!(iter.next(), None);
    /// ```
    ///
    /// 由于 `chain()` 的参数使用 [`IntoIterator`]，可以传入任何能转换成
    /// [`Iterator`] 的值，而不只是 [`Iterator`] 本身。例如数组 (`[T]`) 实现了
    /// [`IntoIterator`]，因此可以直接传给 `chain()`:
    ///
    /// ```
    /// let a1 = [1, 2, 3];
    /// let a2 = [4, 5, 6];
    ///
    /// let mut iter = a1.into_iter().chain(a2);
    ///
    /// assert_eq!(iter.next(), Some(1));
    /// assert_eq!(iter.next(), Some(2));
    /// assert_eq!(iter.next(), Some(3));
    /// assert_eq!(iter.next(), Some(4));
    /// assert_eq!(iter.next(), Some(5));
    /// assert_eq!(iter.next(), Some(6));
    /// assert_eq!(iter.next(), None);
    /// ```
    ///
    /// 如果需要配合 Windows API，可能会想把 [`OsStr`] 转换成 `Vec<u16>`:
    ///
    /// ```
    /// #[cfg(windows)]
    /// fn os_str_to_utf16(s: &std::ffi::OsStr) -> Vec<u16> {
    ///     use std::os::windows::ffi::OsStrExt;
    ///     s.encode_wide().chain(std::iter::once(0)).collect()
    /// }
    /// ```
    ///
    /// [`once`]: crate::iter::once
    /// [`OsStr`]: ../../std/ffi/struct.OsStr.html
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    fn chain<U>(self, other: U) -> Chain<Self, U::IntoIter>
    where
        Self: Sized,
        U: IntoIterator<Item = Self::Item>,
    {
        Chain::new(self, other.into_iter())
    }

    /// 将两个 iterator “zip” 成一个产出成对元素的 iterator。
    ///
    /// `zip()` 返回一个新的 iterator，它会同时遍历另外两个 iterator，并返回元组:
    /// 元组第一个元素来自第一个 iterator，第二个元素来自第二个 iterator。
    ///
    /// 换句话说，它把两个 iterator 合并成一个 iterator。
    ///
    /// 如果任一 iterator 返回 [`None`]，zip 后 iterator 的 [`next`] 就会返回
    /// [`None`]。如果 zip 后的 iterator 已经没有更多元素可返回，之后每次尝试推进它时，
    /// 会先至多尝试推进第一个 iterator 一次；如果第一个 iterator 仍产出了元素，
    /// 再至多尝试推进第二个 iterator 一次。
    ///
    /// 如果要“撤销”两个 iterator zip 后的结果，见 [`unzip`]。
    ///
    /// [`unzip`]: Iterator::unzip
    ///
    /// # 示例
    ///
    /// 基本用法:
    ///
    /// ```
    /// let s1 = "abc".chars();
    /// let s2 = "def".chars();
    ///
    /// let mut iter = s1.zip(s2);
    ///
    /// assert_eq!(iter.next(), Some(('a', 'd')));
    /// assert_eq!(iter.next(), Some(('b', 'e')));
    /// assert_eq!(iter.next(), Some(('c', 'f')));
    /// assert_eq!(iter.next(), None);
    /// ```
    ///
    /// 由于 `zip()` 的参数使用 [`IntoIterator`]，可以传入任何能转换成
    /// [`Iterator`] 的值，而不只是 [`Iterator`] 本身。例如数组 (`[T]`) 实现了
    /// [`IntoIterator`]，因此可以直接传给 `zip()`:
    ///
    /// ```
    /// let a1 = [1, 2, 3];
    /// let a2 = [4, 5, 6];
    ///
    /// let mut iter = a1.into_iter().zip(a2);
    ///
    /// assert_eq!(iter.next(), Some((1, 4)));
    /// assert_eq!(iter.next(), Some((2, 5)));
    /// assert_eq!(iter.next(), Some((3, 6)));
    /// assert_eq!(iter.next(), None);
    /// ```
    ///
    /// `zip()` 常用于把一个无限 iterator 和一个有限 iterator 配对。这可以正常工作，
    /// 因为有限 iterator 最终会返回 [`None`]，从而结束 zip。与 `(0..)` 一起 zip
    /// 看起来很像 [`enumerate`]:
    ///
    /// ```
    /// let enumerate: Vec<_> = "foo".chars().enumerate().collect();
    ///
    /// let zipper: Vec<_> = (0..).zip("foo".chars()).collect();
    ///
    /// assert_eq!((0, 'f'), enumerate[0]);
    /// assert_eq!((0, 'f'), zipper[0]);
    ///
    /// assert_eq!((1, 'o'), enumerate[1]);
    /// assert_eq!((1, 'o'), zipper[1]);
    ///
    /// assert_eq!((2, 'o'), enumerate[2]);
    /// assert_eq!((2, 'o'), zipper[2]);
    /// ```
    ///
    /// 如果两个 iterator 的表达式结构大致对称，使用 [`zip`] 可能更易读:
    ///
    /// ```
    /// use std::iter::zip;
    ///
    /// let a = [1, 2, 3];
    /// let b = [2, 3, 4];
    ///
    /// let mut zipped = zip(
    ///     a.into_iter().map(|x| x * 2).skip(1),
    ///     b.into_iter().map(|x| x * 2).skip(1),
    /// );
    ///
    /// assert_eq!(zipped.next(), Some((4, 6)));
    /// assert_eq!(zipped.next(), Some((6, 8)));
    /// assert_eq!(zipped.next(), None);
    /// ```
    ///
    /// 与下面写法相比:
    ///
    /// ```
    /// # let a = [1, 2, 3];
    /// # let b = [2, 3, 4];
    /// #
    /// let mut zipped = a
    ///     .into_iter()
    ///     .map(|x| x * 2)
    ///     .skip(1)
    ///     .zip(b.into_iter().map(|x| x * 2).skip(1));
    /// #
    /// # assert_eq!(zipped.next(), Some((4, 6)));
    /// # assert_eq!(zipped.next(), Some((6, 8)));
    /// # assert_eq!(zipped.next(), None);
    /// ```
    ///
    /// [`enumerate`]: Iterator::enumerate
    /// [`next`]: Iterator::next
    /// [`zip`]: crate::iter::zip
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    fn zip<U>(self, other: U) -> Zip<Self, U::IntoIter>
    where
        Self: Sized,
        U: IntoIterator,
    {
        Zip::new(self, other.into_iter())
    }

    /// 创建一个新的 iterator，在原 iterator 的相邻元素之间放入 `separator` 的副本。
    ///
    /// 如果 `separator` 没有实现 [`Clone`]，或者需要每次重新计算分隔元素，请使用
    /// [`intersperse_with`]。
    ///
    /// # 示例
    ///
    /// 基本用法:
    ///
    /// ```
    /// #![feature(iter_intersperse)]
    ///
    /// let mut a = [0, 1, 2].into_iter().intersperse(100);
    /// assert_eq!(a.next(), Some(0));   // `a` 的第一个元素。
    /// assert_eq!(a.next(), Some(100)); // 分隔元素。
    /// assert_eq!(a.next(), Some(1));   // `a` 的下一个元素。
    /// assert_eq!(a.next(), Some(100)); // 分隔元素。
    /// assert_eq!(a.next(), Some(2));   // `a` 的最后一个元素。
    /// assert_eq!(a.next(), None);       // iterator 已结束。
    /// ```
    ///
    /// `intersperse` 对于用共同元素连接 iterator 的各项很有用:
    /// ```
    /// #![feature(iter_intersperse)]
    ///
    /// let words = ["Hello", "World", "!"];
    /// let hello: String = words.into_iter().intersperse(" ").collect();
    /// assert_eq!(hello, "Hello World !");
    /// ```
    ///
    /// [`Clone`]: crate::clone::Clone
    /// [`intersperse_with`]: Iterator::intersperse_with
    #[inline]
    #[unstable(feature = "iter_intersperse", issue = "79524")]
    fn intersperse(self, separator: Self::Item) -> Intersperse<Self>
    where
        Self: Sized,
        Self::Item: Clone,
    {
        Intersperse::new(self, separator)
    }

    /// 创建一个新的 iterator，在原 iterator 的相邻元素之间放入由 `separator` 生成的项。
    ///
    /// 每当需要在底层 iterator 的两个相邻项之间放入一个项时，该闭包都会被恰好调用
    /// 一次。具体来说，如果底层 iterator 产出少于两个项，或最后一个项已经产出之后，
    /// 闭包不会被调用。
    ///
    /// 如果 iterator 的项实现了 [`Clone`]，使用 [`intersperse`] 可能更简单。
    ///
    /// # 示例
    ///
    /// 基本用法:
    ///
    /// ```
    /// #![feature(iter_intersperse)]
    ///
    /// #[derive(PartialEq, Debug)]
    /// struct NotClone(usize);
    ///
    /// let v = [NotClone(0), NotClone(1), NotClone(2)];
    /// let mut it = v.into_iter().intersperse_with(|| NotClone(99));
    ///
    /// assert_eq!(it.next(), Some(NotClone(0)));  // `v` 的第一个元素。
    /// assert_eq!(it.next(), Some(NotClone(99))); // 分隔元素。
    /// assert_eq!(it.next(), Some(NotClone(1)));  // `v` 的下一个元素。
    /// assert_eq!(it.next(), Some(NotClone(99))); // 分隔元素。
    /// assert_eq!(it.next(), Some(NotClone(2)));  // `v` 的最后一个元素。
    /// assert_eq!(it.next(), None);               // iterator 已结束。
    /// ```
    ///
    /// 当分隔元素需要计算时，可以使用 `intersperse_with`:
    /// ```
    /// #![feature(iter_intersperse)]
    ///
    /// let src = ["Hello", "to", "all", "people", "!!"].iter().copied();
    ///
    /// // 该闭包可变借用其上下文来生成一个项。
    /// let mut happy_emojis = [" ❤️ ", " 😀 "].into_iter();
    /// let separator = || happy_emojis.next().unwrap_or(" 🦀 ");
    ///
    /// let result = src.intersperse_with(separator).collect::<String>();
    /// assert_eq!(result, "Hello ❤️ to 😀 all 🦀 people 🦀 !!");
    /// ```
    /// [`Clone`]: crate::clone::Clone
    /// [`intersperse`]: Iterator::intersperse
    #[inline]
    #[unstable(feature = "iter_intersperse", issue = "79524")]
    fn intersperse_with<G>(self, separator: G) -> IntersperseWith<Self, G>
    where
        Self: Sized,
        G: FnMut() -> Self::Item,
    {
        IntersperseWith::new(self, separator)
    }

    /// 接收一个闭包，并创建一个会在每个元素上调用该闭包的 iterator。
    ///
    /// `map()` 通过它的参数，也就是某个实现 [`FnMut`] 的值，把一个 iterator 转换为
    /// 另一个 iterator。它产生的新 iterator 会在原 iterator 的每个元素上调用该闭包。
    ///
    /// 如果习惯从类型角度思考，可以这样理解 `map()`: 你有一个产出某种类型 `A`
    /// 元素的 iterator，而你想得到一个产出另一种类型 `B` 的 iterator；此时可以使用
    /// `map()`，并传入一个接收 `A`、返回 `B` 的闭包。
    ///
    /// 从概念上说，`map()` 类似一个 [`for`] 循环。不过 `map()` 是惰性的，因此最适合
    /// 已经在组合其他 iterator 时使用。如果只是为了副作用而循环，使用 [`for`] 通常
    /// 更符合惯用写法。
    ///
    /// [`for`]: ../../book/ch03-05-control-flow.html#looping-through-a-collection-with-for
    ///
    /// # 示例
    ///
    /// 基本用法:
    ///
    /// ```
    /// let a = [1, 2, 3];
    ///
    /// let mut iter = a.iter().map(|x| 2 * x);
    ///
    /// assert_eq!(iter.next(), Some(2));
    /// assert_eq!(iter.next(), Some(4));
    /// assert_eq!(iter.next(), Some(6));
    /// assert_eq!(iter.next(), None);
    /// ```
    ///
    /// 如果只是为了执行某种副作用，请优先使用 [`for`] 而不是 `map()`:
    ///
    /// ```
    /// # #![allow(unused_must_use)]
    /// // 不要这样做:
    /// (0..5).map(|x| println!("{x}"));
    ///
    /// // 它甚至不会执行，因为它是惰性的。Rust 会对此发出警告。
    ///
    /// // 应改用 for 循环:
    /// for x in 0..5 {
    ///     println!("{x}");
    /// }
    /// ```
    #[rustc_diagnostic_item = "IteratorMap"]
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    fn map<B, F>(self, f: F) -> Map<Self, F>
    where
        Self: Sized,
        F: FnMut(Self::Item) -> B,
    {
        Map::new(self, f)
    }

    /// 在 iterator 的每个元素上调用闭包。
    ///
    /// 这等价于在 iterator 上使用 [`for`] 循环，不过闭包中不能使用 `break` 和
    /// `continue`。通常使用 `for` 循环更符合惯用写法，但在较长 iterator 链的末端
    /// 处理元素时，`for_each` 可能更清楚。在某些情况下 `for_each` 也可能比循环更快，
    /// 因为它会在 `Chain` 等适配器上使用内部迭代。
    ///
    /// [`for`]: ../../book/ch03-05-control-flow.html#looping-through-a-collection-with-for
    ///
    /// # 示例
    ///
    /// 基本用法:
    ///
    /// ```
    /// use std::sync::mpsc::channel;
    ///
    /// let (tx, rx) = channel();
    /// (0..5).map(|x| x * 2 + 1)
    ///       .for_each(move |x| tx.send(x).unwrap());
    ///
    /// let v: Vec<_> = rx.iter().collect();
    /// assert_eq!(v, vec![1, 3, 5, 7, 9]);
    /// ```
    ///
    /// 对这样的小例子来说，`for` 循环可能更清楚；但对更长的 iterator 链，`for_each`
    /// 可能更适合保持函数式风格:
    ///
    /// ```
    /// (0..5).flat_map(|x| (x * 100)..(x * 110))
    ///       .enumerate()
    ///       .filter(|&(i, x)| (i + x) % 3 == 0)
    ///       .for_each(|(i, x)| println!("{i}:{x}"));
    /// ```
    #[inline]
    #[stable(feature = "iterator_for_each", since = "1.21.0")]
    fn for_each<F>(self, f: F)
    where
        Self: Sized,
        F: FnMut(Self::Item),
    {
        #[inline]
        fn call<T>(mut f: impl FnMut(T)) -> impl FnMut((), T) {
            move |(), item| f(item)
        }

        self.fold((), call(f));
    }

    /// 创建一个 iterator，使用闭包决定某个元素是否应该被产出。
    ///
    /// 对给定元素，闭包必须返回 `true` 或 `false`。返回的 iterator 只会产出那些让
    /// 闭包返回 `true` 的元素。
    ///
    /// # 示例
    ///
    /// 基本用法:
    ///
    /// ```
    /// let a = [0i32, 1, 2];
    ///
    /// let mut iter = a.into_iter().filter(|x| x.is_positive());
    ///
    /// assert_eq!(iter.next(), Some(1));
    /// assert_eq!(iter.next(), Some(2));
    /// assert_eq!(iter.next(), None);
    /// ```
    ///
    /// 因为传给 `filter()` 的闭包接收引用，而很多 iterator 本身就迭代引用，所以
    /// 可能出现让人困惑的情况: 闭包参数类型是双重引用。
    ///
    /// ```
    /// let s = &[0, 1, 2];
    ///
    /// let mut iter = s.iter().filter(|x| **x > 1); // 需要两个 *！
    ///
    /// assert_eq!(iter.next(), Some(&2));
    /// assert_eq!(iter.next(), None);
    /// ```
    ///
    /// 常见写法是在参数上使用解构，去掉一层引用:
    ///
    /// ```
    /// let s = &[0, 1, 2];
    ///
    /// let mut iter = s.iter().filter(|&x| *x > 1); // 同时使用 & 和 *
    ///
    /// assert_eq!(iter.next(), Some(&2));
    /// assert_eq!(iter.next(), None);
    /// ```
    ///
    /// 或者去掉两层:
    ///
    /// ```
    /// let s = &[0, 1, 2];
    ///
    /// let mut iter = s.iter().filter(|&&x| x > 1); // 两个 &
    ///
    /// assert_eq!(iter.next(), Some(&2));
    /// assert_eq!(iter.next(), None);
    /// ```
    ///
    /// 用于剥离这些引用层级。
    ///
    /// 注意，`iter.filter(f).next()` 等价于 `iter.find(f)`。
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_diagnostic_item = "iter_filter"]
    fn filter<P>(self, predicate: P) -> Filter<Self, P>
    where
        Self: Sized,
        P: FnMut(&Self::Item) -> bool,
    {
        Filter::new(self, predicate)
    }

    /// 创建一个同时执行过滤和映射的 iterator。
    ///
    /// 返回的 iterator 只会产出那些让所提供闭包返回 `Some(value)` 的 `value`。
    ///
    /// `filter_map` 可用于让 [`filter`] 和 [`map`] 组成的链更简洁。下面的示例展示了
    /// 如何把 `map().filter().map()` 缩短为一次 `filter_map` 调用。
    ///
    /// [`filter`]: Iterator::filter
    /// [`map`]: Iterator::map
    ///
    /// # 示例
    ///
    /// 基本用法:
    ///
    /// ```
    /// let a = ["1", "two", "NaN", "four", "5"];
    ///
    /// let mut iter = a.iter().filter_map(|s| s.parse().ok());
    ///
    /// assert_eq!(iter.next(), Some(1));
    /// assert_eq!(iter.next(), Some(5));
    /// assert_eq!(iter.next(), None);
    /// ```
    ///
    /// 下面是同一个例子，但使用 [`filter`] 和 [`map`]:
    ///
    /// ```
    /// let a = ["1", "two", "NaN", "four", "5"];
    /// let mut iter = a.iter().map(|s| s.parse()).filter(|s| s.is_ok()).map(|s| s.unwrap());
    /// assert_eq!(iter.next(), Some(1));
    /// assert_eq!(iter.next(), Some(5));
    /// assert_eq!(iter.next(), None);
    /// ```
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    fn filter_map<B, F>(self, f: F) -> FilterMap<Self, F>
    where
        Self: Sized,
        F: FnMut(Self::Item) -> Option<B>,
    {
        FilterMap::new(self, f)
    }

    /// 创建一个同时给出当前迭代计数和下一个值的 iterator。
    ///
    /// 返回的 iterator 会产出 `(i, val)` 对，其中 `i` 是当前迭代索引，`val` 是
    /// iterator 返回的值。
    ///
    /// `enumerate()` 使用 [`usize`] 保存计数。如果想用其他大小的整数计数，
    /// [`zip`] 函数可以提供类似功能。
    ///
    /// # 溢出行为
    ///
    /// 该方法不会额外防护溢出，因此枚举超过 [`usize::MAX`] 个元素时，要么产生错误
    /// 结果，要么 panic。如果启用了溢出检查，则保证会 panic。
    ///
    /// # Panics
    ///
    /// 如果将要返回的索引会使 [`usize`] 溢出，返回的 iterator 可能 panic。
    ///
    /// [`zip`]: Iterator::zip
    ///
    /// # 示例
    ///
    /// ```
    /// let a = ['a', 'b', 'c'];
    ///
    /// let mut iter = a.into_iter().enumerate();
    ///
    /// assert_eq!(iter.next(), Some((0, 'a')));
    /// assert_eq!(iter.next(), Some((1, 'b')));
    /// assert_eq!(iter.next(), Some((2, 'c')));
    /// assert_eq!(iter.next(), None);
    /// ```
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_diagnostic_item = "enumerate_method"]
    fn enumerate(self) -> Enumerate<Self>
    where
        Self: Sized,
    {
        Enumerate::new(self)
    }

    /// 创建一个 iterator，它可以用 [`peek`] 和 [`peek_mut`] 方法在不消耗元素的情况下
    /// 查看下一个元素。更多信息见这些方法各自的文档。
    ///
    /// 注意，第一次调用 [`peek`] 或 [`peek_mut`] 时，底层 iterator 仍会被推进:
    /// 为了取得下一个元素，会在底层 iterator 上调用 [`next`]，因此 [`next`] 方法的
    /// 任何副作用（也就是“取得下一个值”以外的行为）都会发生。
    ///
    ///
    /// # 示例
    ///
    /// 基本用法:
    ///
    /// ```
    /// let xs = [1, 2, 3];
    ///
    /// let mut iter = xs.into_iter().peekable();
    ///
    /// // peek() 让我们可以预看下一个元素
    /// assert_eq!(iter.peek(), Some(&1));
    /// assert_eq!(iter.next(), Some(1));
    ///
    /// assert_eq!(iter.next(), Some(2));
    ///
    /// // 可以多次 peek()，iterator 不会继续推进
    /// assert_eq!(iter.peek(), Some(&3));
    /// assert_eq!(iter.peek(), Some(&3));
    ///
    /// assert_eq!(iter.next(), Some(3));
    ///
    /// // iterator 结束后，peek() 也结束
    /// assert_eq!(iter.peek(), None);
    /// assert_eq!(iter.next(), None);
    /// ```
    ///
    /// 使用 [`peek_mut`] 在不推进 iterator 的情况下修改下一项:
    ///
    /// ```
    /// let xs = [1, 2, 3];
    ///
    /// let mut iter = xs.into_iter().peekable();
    ///
    /// // `peek_mut()` 让我们可以预看下一个元素
    /// assert_eq!(iter.peek_mut(), Some(&mut 1));
    /// assert_eq!(iter.peek_mut(), Some(&mut 1));
    /// assert_eq!(iter.next(), Some(1));
    ///
    /// if let Some(p) = iter.peek_mut() {
    ///     assert_eq!(*p, 2);
    ///     // 向 iterator 中放入一个值
    ///     *p = 1000;
    /// }
    ///
    /// // iterator 继续时，该值会再次出现
    /// assert_eq!(iter.collect::<Vec<_>>(), vec![1000, 3]);
    /// ```
    /// [`peek`]: Peekable::peek
    /// [`peek_mut`]: Peekable::peek_mut
    /// [`next`]: Iterator::next
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    fn peekable(self) -> Peekable<Self>
    where
        Self: Sized,
    {
        Peekable::new(self)
    }

    /// 创建一个根据谓词 [`skip`] 元素的 iterator。
    ///
    /// [`skip`]: Iterator::skip
    ///
    /// `skip_while()` 接收一个闭包作为参数。它会在 iterator 的每个元素上调用该闭包，
    /// 并忽略元素，直到闭包返回 `false`。
    ///
    /// 一旦返回 `false`，`skip_while()` 的工作就结束，剩余元素都会被产出。
    ///
    /// # 示例
    ///
    /// 基本用法:
    ///
    /// ```
    /// let a = [-1i32, 0, 1];
    ///
    /// let mut iter = a.into_iter().skip_while(|x| x.is_negative());
    ///
    /// assert_eq!(iter.next(), Some(0));
    /// assert_eq!(iter.next(), Some(1));
    /// assert_eq!(iter.next(), None);
    /// ```
    ///
    /// 因为传给 `skip_while()` 的闭包接收引用，而很多 iterator 本身就迭代引用，
    /// 所以可能出现让人困惑的情况: 闭包参数类型是双重引用。
    ///
    /// ```
    /// let s = &[-1, 0, 1];
    ///
    /// let mut iter = s.iter().skip_while(|x| **x < 0); // 需要两个 *！
    ///
    /// assert_eq!(iter.next(), Some(&0));
    /// assert_eq!(iter.next(), Some(&1));
    /// assert_eq!(iter.next(), None);
    /// ```
    ///
    /// 遇到第一个 `false` 后停止跳过:
    ///
    /// ```
    /// let a = [-1, 0, 1, -2];
    ///
    /// let mut iter = a.into_iter().skip_while(|&x| x < 0);
    ///
    /// assert_eq!(iter.next(), Some(0));
    /// assert_eq!(iter.next(), Some(1));
    ///
    /// // 虽然这个元素也会让谓词为 false，但由于前面已经遇到过 false，
    /// // skip_while() 不再继续使用谓词
    /// assert_eq!(iter.next(), Some(-2));
    ///
    /// assert_eq!(iter.next(), None);
    /// ```
    #[inline]
    #[doc(alias = "drop_while")]
    #[stable(feature = "rust1", since = "1.0.0")]
    fn skip_while<P>(self, predicate: P) -> SkipWhile<Self, P>
    where
        Self: Sized,
        P: FnMut(&Self::Item) -> bool,
    {
        SkipWhile::new(self, predicate)
    }

    /// 创建一个根据谓词产出元素的 iterator。
    ///
    /// `take_while()` 接收一个闭包作为参数。它会在 iterator 的每个元素上调用该闭包，
    /// 并在闭包返回 `true` 时产出元素。
    ///
    /// 一旦返回 `false`，`take_while()` 的工作就结束，剩余元素都会被忽略。
    ///
    /// # 示例
    ///
    /// 基本用法:
    ///
    /// ```
    /// let a = [-1i32, 0, 1];
    ///
    /// let mut iter = a.into_iter().take_while(|x| x.is_negative());
    ///
    /// assert_eq!(iter.next(), Some(-1));
    /// assert_eq!(iter.next(), None);
    /// ```
    ///
    /// 因为传给 `take_while()` 的闭包接收引用，而很多 iterator 本身就迭代引用，
    /// 所以可能出现让人困惑的情况: 闭包参数类型是双重引用。
    ///
    /// ```
    /// let s = &[-1, 0, 1];
    ///
    /// let mut iter = s.iter().take_while(|x| **x < 0); // 需要两个 *！
    ///
    /// assert_eq!(iter.next(), Some(&-1));
    /// assert_eq!(iter.next(), None);
    /// ```
    ///
    /// 遇到第一个 `false` 后停止:
    ///
    /// ```
    /// let a = [-1, 0, 1, -2];
    ///
    /// let mut iter = a.into_iter().take_while(|&x| x < 0);
    ///
    /// assert_eq!(iter.next(), Some(-1));
    ///
    /// // 后面还有小于零的元素，但由于前面已经遇到 false，
    /// // take_while() 会忽略剩余元素。
    /// assert_eq!(iter.next(), None);
    /// ```
    ///
    /// 因为 `take_while()` 需要查看值才能判断它是否应该被包含，所以消费原 iterator
    /// 时会发现该值已经被移除:
    ///
    /// ```
    /// let a = [1, 2, 3, 4];
    /// let mut iter = a.into_iter();
    ///
    /// let result: Vec<i32> = iter.by_ref().take_while(|&n| n != 3).collect();
    ///
    /// assert_eq!(result, [1, 2]);
    ///
    /// let result: Vec<i32> = iter.collect();
    ///
    /// assert_eq!(result, [4]);
    /// ```
    ///
    /// `3` 已经不在其中，因为它为了判断迭代是否应停止而被消耗，并且没有被放回
    /// iterator。
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    fn take_while<P>(self, predicate: P) -> TakeWhile<Self, P>
    where
        Self: Sized,
        P: FnMut(&Self::Item) -> bool,
    {
        TakeWhile::new(self, predicate)
    }

    /// 创建一个同时按谓词产出元素并执行映射的 iterator。
    ///
    /// `map_while()` 接收一个闭包作为参数。它会在 iterator 的每个元素上调用该闭包，
    /// 并在闭包返回 [`Some(_)`][`Some`] 时产出元素。
    ///
    /// # 示例
    ///
    /// 基本用法:
    ///
    /// ```
    /// let a = [-1i32, 4, 0, 1];
    ///
    /// let mut iter = a.into_iter().map_while(|x| 16i32.checked_div(x));
    ///
    /// assert_eq!(iter.next(), Some(-16));
    /// assert_eq!(iter.next(), Some(4));
    /// assert_eq!(iter.next(), None);
    /// ```
    ///
    /// 下面是同一个例子，但使用 [`take_while`] 和 [`map`]:
    ///
    /// [`take_while`]: Iterator::take_while
    /// [`map`]: Iterator::map
    ///
    /// ```
    /// let a = [-1i32, 4, 0, 1];
    ///
    /// let mut iter = a.into_iter()
    ///                 .map(|x| 16i32.checked_div(x))
    ///                 .take_while(|x| x.is_some())
    ///                 .map(|x| x.unwrap());
    ///
    /// assert_eq!(iter.next(), Some(-16));
    /// assert_eq!(iter.next(), Some(4));
    /// assert_eq!(iter.next(), None);
    /// ```
    ///
    /// 遇到第一个 [`None`] 后停止:
    ///
    /// ```
    /// let a = [0, 1, 2, -3, 4, 5, -6];
    ///
    /// let iter = a.into_iter().map_while(|x| u32::try_from(x).ok());
    /// let vec: Vec<_> = iter.collect();
    ///
    /// // 后面还有能放入 u32 的元素（例如 4、5），但 `map_while` 对 `-3` 返回了 `None`
    /// //（因为 `predicate` 返回 `None`），而 `collect` 会在遇到第一个 `None` 时停止。
    /// assert_eq!(vec, [0, 1, 2]);
    /// ```
    ///
    /// 因为 `map_while()` 需要查看值才能判断它是否应该被包含，所以消费原 iterator
    /// 时会发现该值已经被移除:
    ///
    /// ```
    /// let a = [1, 2, -3, 4];
    /// let mut iter = a.into_iter();
    ///
    /// let result: Vec<u32> = iter.by_ref()
    ///                            .map_while(|n| u32::try_from(n).ok())
    ///                            .collect();
    ///
    /// assert_eq!(result, [1, 2]);
    ///
    /// let result: Vec<i32> = iter.collect();
    ///
    /// assert_eq!(result, [4]);
    /// ```
    ///
    /// `-3` 已经不在其中，因为它为了判断迭代是否应停止而被消耗，并且没有被放回
    /// iterator。
    ///
    /// 注意，与 [`take_while`] 不同，该 iterator **不是** fused。第一次返回
    /// [`None`] 之后它还会返回什么并未被指定。如果需要 fused iterator，请使用
    /// [`fuse`]。
    ///
    /// [`fuse`]: Iterator::fuse
    #[inline]
    #[stable(feature = "iter_map_while", since = "1.57.0")]
    fn map_while<B, P>(self, predicate: P) -> MapWhile<Self, P>
    where
        Self: Sized,
        P: FnMut(Self::Item) -> Option<B>,
    {
        MapWhile::new(self, predicate)
    }

    /// 创建一个跳过前 `n` 个元素的 iterator。
    ///
    /// `skip(n)` 会跳过元素，直到跳过了 `n` 个元素或到达 iterator 末尾（二者以先
    /// 发生者为准）。之后，所有剩余元素都会被产出。特别地，如果原 iterator 太短，
    /// 返回的 iterator 就是空的。
    ///
    /// 与其直接覆盖该方法，更应覆盖 `nth` 方法。
    ///
    /// # 示例
    ///
    /// ```
    /// let a = [1, 2, 3];
    ///
    /// let mut iter = a.into_iter().skip(2);
    ///
    /// assert_eq!(iter.next(), Some(3));
    /// assert_eq!(iter.next(), None);
    /// ```
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    fn skip(self, n: usize) -> Skip<Self>
    where
        Self: Sized,
    {
        Skip::new(self, n)
    }

    /// 创建一个产出前 `n` 个元素的 iterator；如果底层 iterator 更早结束，则产出更少。
    ///
    /// `take(n)` 会产出元素，直到已经产出 `n` 个元素或到达 iterator 末尾（二者以
    /// 先发生者为准）。如果原 iterator 至少包含 `n` 个元素，返回的 iterator 就是长度
    /// 为 `n` 的前缀；否则它包含原 iterator 的全部元素（数量少于 `n`）。
    ///
    /// # 示例
    ///
    /// 基本用法:
    ///
    /// ```
    /// let a = [1, 2, 3];
    ///
    /// let mut iter = a.into_iter().take(2);
    ///
    /// assert_eq!(iter.next(), Some(1));
    /// assert_eq!(iter.next(), Some(2));
    /// assert_eq!(iter.next(), None);
    /// ```
    ///
    /// `take()` 常用于无限 iterator，把它限制为有限 iterator:
    ///
    /// ```
    /// let mut iter = (0..).take(3);
    ///
    /// assert_eq!(iter.next(), Some(0));
    /// assert_eq!(iter.next(), Some(1));
    /// assert_eq!(iter.next(), Some(2));
    /// assert_eq!(iter.next(), None);
    /// ```
    ///
    /// 如果可用元素少于 `n` 个，`take` 会把自身限制到底层 iterator 的长度:
    ///
    /// ```
    /// let v = [1, 2];
    /// let mut iter = v.into_iter().take(5);
    /// assert_eq!(iter.next(), Some(1));
    /// assert_eq!(iter.next(), Some(2));
    /// assert_eq!(iter.next(), None);
    /// ```
    ///
    /// 使用 [`by_ref`] 可以在不消耗 iterator 所有权的情况下从中取元素，然后继续使用
    /// 原 iterator:
    ///
    /// ```
    /// let mut words = ["hello", "world", "of", "Rust"].into_iter();
    ///
    /// // 取出前两个单词。
    /// let hello_world: Vec<_> = words.by_ref().take(2).collect();
    /// assert_eq!(hello_world, vec!["hello", "world"]);
    ///
    /// // 收集剩余单词。
    /// // 能这样做是因为前面使用了 `by_ref`。
    /// let of_rust: Vec<_> = words.collect();
    /// assert_eq!(of_rust, vec!["of", "Rust"]);
    /// ```
    ///
    /// [`by_ref`]: Iterator::by_ref
    #[doc(alias = "limit")]
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    fn take(self, n: usize) -> Take<Self>
    where
        Self: Sized,
    {
        Take::new(self, n)
    }

    /// 一个 iterator 适配器，它和 [`fold`] 一样持有内部状态，但不同于 [`fold`]，
    /// 它会产生新的 iterator。
    ///
    /// [`fold`]: Iterator::fold
    ///
    /// `scan()` 接收两个参数: 一个作为内部状态初始种子的初始值，以及一个带两个参数
    /// 的闭包。闭包第一个参数是指向内部状态的可变引用，第二个参数是 iterator 元素。
    /// 闭包可以赋值给内部状态，从而在多次迭代之间共享状态。
    ///
    /// 迭代时，该闭包会应用到 iterator 的每个元素；闭包的返回值是一个 [`Option`]，
    /// 并会由 `next` 方法返回。因此闭包可以返回 `Some(value)` 来产出 `value`，
    /// 或返回 `None` 来结束迭代。
    ///
    /// # 示例
    ///
    /// ```
    /// let a = [1, 2, 3, 4];
    ///
    /// let mut iter = a.into_iter().scan(1, |state, x| {
    ///     // 每次迭代都把状态乘以当前元素 ...
    ///     *state = *state * x;
    ///
    ///     // ... 如果状态超过 6，则终止
    ///     if *state > 6 {
    ///         return None;
    ///     }
    ///     // ... 否则产出状态的相反数
    ///     Some(-*state)
    /// });
    ///
    /// assert_eq!(iter.next(), Some(-1));
    /// assert_eq!(iter.next(), Some(-2));
    /// assert_eq!(iter.next(), Some(-6));
    /// assert_eq!(iter.next(), None);
    /// ```
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    fn scan<St, B, F>(self, initial_state: St, f: F) -> Scan<Self, St, F>
    where
        Self: Sized,
        F: FnMut(&mut St, Self::Item) -> Option<B>,
    {
        Scan::new(self, initial_state, f)
    }

    /// 创建一个类似 map、但会展平嵌套结构的 iterator。
    ///
    /// [`map`] 适配器很有用，但它最适合闭包参数直接产生值的情况。如果闭包产生的是
    /// iterator，就会多出一层间接结构。`flat_map()` 会自行移除这一额外层级。
    ///
    /// 可以把 `flat_map(f)` 理解为先执行 [`map`]，再像 `map(f).flatten()` 那样执行
    /// [`flatten`] 的语义等价形式。
    ///
    /// 另一种理解 `flat_map()` 的方式是: [`map`] 的闭包为每个元素返回一个项，
    /// 而 `flat_map()` 的闭包为每个元素返回一个 iterator。
    ///
    /// [`map`]: Iterator::map
    /// [`flatten`]: Iterator::flatten
    ///
    /// # 示例
    ///
    /// ```
    /// let words = ["alpha", "beta", "gamma"];
    ///
    /// // chars() 返回一个 iterator
    /// let merged: String = words.iter()
    ///                           .flat_map(|s| s.chars())
    ///                           .collect();
    /// assert_eq!(merged, "alphabetagamma");
    /// ```
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    fn flat_map<U, F>(self, f: F) -> FlatMap<Self, U, F>
    where
        Self: Sized,
        U: IntoIterator,
        F: FnMut(Self::Item) -> U,
    {
        FlatMap::new(self, f)
    }

    /// 创建一个展平嵌套结构的 iterator。
    ///
    /// 当你有一个由 iterator 组成的 iterator，或者一个由可转换为 iterator 的值组成的
    /// iterator，并希望移除一层间接结构时，该方法很有用。
    ///
    /// # 示例
    ///
    /// 基本用法:
    ///
    /// ```
    /// let data = vec![vec![1, 2, 3, 4], vec![5, 6]];
    /// let flattened: Vec<_> = data.into_iter().flatten().collect();
    /// assert_eq!(flattened, [1, 2, 3, 4, 5, 6]);
    /// ```
    ///
    /// 先映射再展平:
    ///
    /// ```
    /// let words = ["alpha", "beta", "gamma"];
    ///
    /// // chars() 返回一个 iterator
    /// let merged: String = words.iter()
    ///                           .map(|s| s.chars())
    ///                           .flatten()
    ///                           .collect();
    /// assert_eq!(merged, "alphabetagamma");
    /// ```
    ///
    /// 也可以用 [`flat_map()`] 重写；在这个场景下它更推荐，因为能更清楚地表达意图:
    ///
    /// ```
    /// let words = ["alpha", "beta", "gamma"];
    ///
    /// // chars() 返回一个 iterator
    /// let merged: String = words.iter()
    ///                           .flat_map(|s| s.chars())
    ///                           .collect();
    /// assert_eq!(merged, "alphabetagamma");
    /// ```
    ///
    /// 展平适用于任何 `IntoIterator` 类型，包括 `Option` 和 `Result`:
    ///
    /// ```
    /// let options = vec![Some(123), Some(321), None, Some(231)];
    /// let flattened_options: Vec<_> = options.into_iter().flatten().collect();
    /// assert_eq!(flattened_options, [123, 321, 231]);
    ///
    /// let results = vec![Ok(123), Ok(321), Err(456), Ok(231)];
    /// let flattened_results: Vec<_> = results.into_iter().flatten().collect();
    /// assert_eq!(flattened_results, [123, 321, 231]);
    /// ```
    ///
    /// 每次展平只会移除一层嵌套:
    ///
    /// ```
    /// let d3 = [[[1, 2], [3, 4]], [[5, 6], [7, 8]]];
    ///
    /// let d2: Vec<_> = d3.into_iter().flatten().collect();
    /// assert_eq!(d2, [[1, 2], [3, 4], [5, 6], [7, 8]]);
    ///
    /// let d1: Vec<_> = d3.into_iter().flatten().flatten().collect();
    /// assert_eq!(d1, [1, 2, 3, 4, 5, 6, 7, 8]);
    /// ```
    ///
    /// 这里可以看到，`flatten()` 不会执行“深度”展平。它一次只移除一层嵌套。也就是说，
    /// 如果对三维数组调用 `flatten()`，结果会是二维而不是一维。要得到一维结构，
    /// 必须再次调用 `flatten()`。
    ///
    /// [`flat_map()`]: Iterator::flat_map
    #[inline]
    #[stable(feature = "iterator_flatten", since = "1.29.0")]
    fn flatten(self) -> Flatten<Self>
    where
        Self: Sized,
        Self::Item: IntoIterator,
    {
        Flatten::new(self)
    }

    /// 对 `self` 中每个长度为 `N` 的连续窗口调用函数 `f`，并返回产出 `f` 结果的迭代器。
    ///
    /// 和 [`slice::windows()`] 一样，映射时的窗口会相互重叠。该适配器仍然是惰性的:
    /// 创建它时不会读取元素，只有调用 `next` 等消费方法时，才会维护内部窗口缓冲并
    /// 调用闭包。
    ///
    /// 下面的例子中，闭包会分别以 `&['a', 'b']`、`&['b', 'c']` 和
    /// `&['c', 'd']` 为参数调用三次。
    ///
    /// ```
    /// #![feature(iter_map_windows)]
    ///
    /// let strings = "abcd".chars()
    ///     .map_windows(|[x, y]| format!("{}+{}", x, y))
    ///     .collect::<Vec<String>>();
    ///
    /// assert_eq!(strings, vec!["a+b", "b+c", "c+d"]);
    /// ```
    ///
    /// 注意，const 参数 `N` 通常会根据闭包的解构参数推断出来。
    ///
    /// 返回的迭代器会产出 𝑘 - `N` + 1 项，其中 𝑘 是 `self` 实际产出的项数。
    /// 如果 𝑘 小于 `N`，则返回的迭代器为空。
    ///
    /// 返回的迭代器实现 [`FusedIterator`]。原因是窗口必须表示一段连续历史；一旦
    /// `self` 返回 [`None`]，即使底层非 fused 迭代器之后又返回 `Some(T)`，也无法把
    /// 这个新元素放回一段没有空洞的连续数组窗口中，因此 `map_windows` 会把第一次
    /// [`None`] 固化为永久结束。
    ///
    /// [`slice::windows()`]: slice::windows
    /// [`FusedIterator`]: crate::iter::FusedIterator
    ///
    /// # Panics
    ///
    /// 如果 `N` 为零会 panic。该检查在方法稳定前很可能改成编译期错误。
    ///
    /// ```should_panic
    /// #![feature(iter_map_windows)]
    ///
    /// let iter = std::iter::repeat(0).map_windows(|&[]| ());
    /// ```
    ///
    /// # 示例
    ///
    /// 计算相邻数字之和。
    ///
    /// ```
    /// #![feature(iter_map_windows)]
    ///
    /// let mut it = [1, 3, 8, 1].iter().map_windows(|&[a, b]| a + b);
    /// assert_eq!(it.next(), Some(4));  // 1 + 3
    /// assert_eq!(it.next(), Some(11)); // 3 + 8
    /// assert_eq!(it.next(), Some(9));  // 8 + 1
    /// assert_eq!(it.next(), None);
    /// ```
    ///
    /// 因为下面例子中的元素实现了 `Copy`，可以直接复制数组来取得窗口迭代结果。
    ///
    /// ```
    /// #![feature(iter_map_windows)]
    ///
    /// let mut it = "ferris".chars().map_windows(|w: &[_; 3]| *w);
    /// assert_eq!(it.next(), Some(['f', 'e', 'r']));
    /// assert_eq!(it.next(), Some(['e', 'r', 'r']));
    /// assert_eq!(it.next(), Some(['r', 'r', 'i']));
    /// assert_eq!(it.next(), Some(['r', 'i', 's']));
    /// assert_eq!(it.next(), None);
    /// ```
    ///
    /// 也可以用这个函数检查迭代器是否有序。简单场景更建议直接使用
    /// [`Iterator::is_sorted`]。
    ///
    /// ```
    /// #![feature(iter_map_windows)]
    ///
    /// let mut it = [0.5, 1.0, 3.5, 3.0, 8.5, 8.5, f32::NAN].iter()
    ///     .map_windows(|[a, b]| a <= b);
    ///
    /// assert_eq!(it.next(), Some(true));  // 0.5 <= 1.0
    /// assert_eq!(it.next(), Some(true));  // 1.0 <= 3.5
    /// assert_eq!(it.next(), Some(false)); // 3.5 <= 3.0
    /// assert_eq!(it.next(), Some(true));  // 3.0 <= 8.5
    /// assert_eq!(it.next(), Some(true));  // 8.5 <= 8.5
    /// assert_eq!(it.next(), Some(false)); // 8.5 <= NAN
    /// assert_eq!(it.next(), None);
    /// ```
    ///
    /// 非 fused 迭代器经过 `map_windows` 后会变成 fused。
    ///
    /// ```
    /// #![feature(iter_map_windows)]
    ///
    /// #[derive(Default)]
    /// struct NonFusedIterator {
    ///     state: i32,
    /// }
    ///
    /// impl Iterator for NonFusedIterator {
    ///     type Item = i32;
    ///
    ///     fn next(&mut self) -> Option<i32> {
    ///         let val = self.state;
    ///         self.state = self.state + 1;
    ///
    ///         // 先产出 `0..5`，之后从 `6..` 起只产出偶数。
    ///         if val < 5 || val % 2 == 0 {
    ///             Some(val)
    ///         } else {
    ///             None
    ///         }
    ///     }
    /// }
    ///
    ///
    /// let mut iter = NonFusedIterator::default();
    ///
    /// // 先产出 0..5。
    /// assert_eq!(iter.next(), Some(0));
    /// assert_eq!(iter.next(), Some(1));
    /// assert_eq!(iter.next(), Some(2));
    /// assert_eq!(iter.next(), Some(3));
    /// assert_eq!(iter.next(), Some(4));
    /// // 然后可以看到该迭代器会在 Some 和 None 之间来回切换。
    /// assert_eq!(iter.next(), None);
    /// assert_eq!(iter.next(), Some(6));
    /// assert_eq!(iter.next(), None);
    /// assert_eq!(iter.next(), Some(8));
    /// assert_eq!(iter.next(), None);
    ///
    /// // 但是，经过 `.map_windows()` 后它会变成 fused。
    /// let mut iter = NonFusedIterator::default()
    ///     .map_windows(|arr: &[_; 2]| *arr);
    ///
    /// assert_eq!(iter.next(), Some([0, 1]));
    /// assert_eq!(iter.next(), Some([1, 2]));
    /// assert_eq!(iter.next(), Some([2, 3]));
    /// assert_eq!(iter.next(), Some([3, 4]));
    /// assert_eq!(iter.next(), None);
    ///
    /// // 第一次返回 `None` 后，它会一直返回 `None`。
    /// assert_eq!(iter.next(), None);
    /// assert_eq!(iter.next(), None);
    /// assert_eq!(iter.next(), None);
    /// ```
    #[inline]
    #[unstable(feature = "iter_map_windows", issue = "87155")]
    fn map_windows<F, R, const N: usize>(self, f: F) -> MapWindows<Self, F, N>
    where
        Self: Sized,
        F: FnMut(&[Self::Item; N]) -> R,
    {
        MapWindows::new(self, f)
    }

    /// 创建一个在第一次 [`None`] 之后永久结束的迭代器。
    ///
    /// 普通迭代器返回 [`None`] 后，后续调用可能也可能不再产出 [`Some(T)`]。
    /// `fuse()` 会包装迭代器，记录第一次 [`None`]，并保证之后永远返回 [`None`]。
    ///
    /// 如果迭代器已经实现 [`FusedIterator`]，[`Fuse`] 包装器会被视为无操作以便优化。
    /// 因此，错误实现 [`FusedIterator`] 会让 `fuse()` 的语义也变得错误: 调用方会
    /// 信任该 trait 宣称的“结束后永久结束”契约。
    ///
    /// [`Some(T)`]: Some
    /// [`FusedIterator`]: crate::iter::FusedIterator
    ///
    /// # 示例
    ///
    /// ```
    /// // 一个在 Some 和 None 之间交替的迭代器
    /// struct Alternate {
    ///     state: i32,
    /// }
    ///
    /// impl Iterator for Alternate {
    ///     type Item = i32;
    ///
    ///     fn next(&mut self) -> Option<i32> {
    ///         let val = self.state;
    ///         self.state = self.state + 1;
    ///
    ///         // 偶数返回 Some(i32)，奇数返回 None
    ///         (val % 2 == 0).then_some(val)
    ///     }
    /// }
    ///
    /// let mut iter = Alternate { state: 0 };
    ///
    /// // 可以看到这个迭代器会来回切换。
    /// assert_eq!(iter.next(), Some(0));
    /// assert_eq!(iter.next(), None);
    /// assert_eq!(iter.next(), Some(2));
    /// assert_eq!(iter.next(), None);
    ///
    /// // 但是，一旦对它调用 fuse()...
    /// let mut iter = iter.fuse();
    ///
    /// assert_eq!(iter.next(), Some(4));
    /// assert_eq!(iter.next(), None);
    ///
    /// // 第一次返回 `None` 后，它会一直返回 `None`。
    /// assert_eq!(iter.next(), None);
    /// assert_eq!(iter.next(), None);
    /// assert_eq!(iter.next(), None);
    /// ```
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    fn fuse(self) -> Fuse<Self>
    where
        Self: Sized,
    {
        Fuse::new(self)
    }

    /// 对 iterator 的每个元素执行某个操作，并继续传递该值。
    ///
    /// 使用 iterator 时，常会把多个适配器串成一条链。在编写这类代码时，可能希望查看
    /// 管道中不同位置正在发生什么。要做到这一点，可以插入一次 `inspect()` 调用。
    ///
    /// `inspect()` 更常作为调试工具使用，而不是留在最终代码中。不过在某些场景下，
    /// 应用需要在丢弃错误前记录错误，此时它也可能很有用。
    ///
    /// # 示例
    ///
    /// 基本用法:
    ///
    /// ```
    /// let a = [1, 4, 2, 3];
    ///
    /// // 这条 iterator 序列比较复杂。
    /// let sum = a.iter()
    ///     .cloned()
    ///     .filter(|x| x % 2 == 0)
    ///     .fold(0, |sum, i| sum + i);
    ///
    /// println!("{sum}");
    ///
    /// // 加入一些 inspect() 调用来观察发生了什么
    /// let sum = a.iter()
    ///     .cloned()
    ///     .inspect(|x| println!("about to filter: {x}"))
    ///     .filter(|x| x % 2 == 0)
    ///     .inspect(|x| println!("made it through filter: {x}"))
    ///     .fold(0, |sum, i| sum + i);
    ///
    /// println!("{sum}");
    /// ```
    ///
    /// 这会打印:
    ///
    /// ```text
    /// 6
    /// about to filter: 1
    /// about to filter: 4
    /// made it through filter: 4
    /// about to filter: 2
    /// made it through filter: 2
    /// about to filter: 3
    /// 6
    /// ```
    ///
    /// 在丢弃错误前记录错误:
    ///
    /// ```
    /// let lines = ["1", "2", "a"];
    ///
    /// let sum: i32 = lines
    ///     .iter()
    ///     .map(|line| line.parse::<i32>())
    ///     .inspect(|num| {
    ///         if let Err(ref e) = *num {
    ///             println!("Parsing error: {e}");
    ///         }
    ///     })
    ///     .filter_map(Result::ok)
    ///     .sum();
    ///
    /// println!("Sum: {sum}");
    /// ```
    ///
    /// 这会打印:
    ///
    /// ```text
    /// Parsing error: invalid digit found in string
    /// Sum: 3
    /// ```
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    fn inspect<F>(self, f: F) -> Inspect<Self, F>
    where
        Self: Sized,
        F: FnMut(&Self::Item),
    {
        Inspect::new(self, f)
    }

    /// 为这个 `Iterator` 实例创建一个“按引用”适配器。
    ///
    /// 在“按引用”适配器上调用消费性方法（直接或间接调用 `next` 的方法）会消费原
    /// iterator；但取得所有权的方法（带 `self` 参数的方法）只会取得这个“按引用”
    /// iterator 的所有权。
    ///
    /// 这对于在不放弃原 iterator 所有权的情况下调用取得所有权的方法很有用（例如
    /// 下面示例中的 `take`），因此之后仍可继续使用原 iterator。
    ///
    /// 该方法使用
    /// [`impl<I: Iterator + ?Sized> Iterator for &mut I { type Item = I::Item; ...}`](https://doc.rust-lang.org/nightly/std/iter/trait.Iterator.html#impl-Iterator-for-%26mut+I)。
    ///
    /// # 示例
    ///
    /// ```
    /// let mut words = ["hello", "world", "of", "Rust"].into_iter();
    ///
    /// // 取出前两个单词。
    /// let hello_world: Vec<_> = words.by_ref().take(2).collect();
    /// assert_eq!(hello_world, vec!["hello", "world"]);
    ///
    /// // 收集剩余单词。
    /// // 能这样做是因为前面使用了 `by_ref`。
    /// let of_rust: Vec<_> = words.collect();
    /// assert_eq!(of_rust, vec!["of", "Rust"]);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    fn by_ref(&mut self) -> &mut Self
    where
        Self: Sized,
    {
        self
    }

    /// 将 iterator 转换为集合。
    ///
    /// `collect()` 获取 iterator 的所有权，并产生你请求的集合类型。iterator 本身并不
    /// 知道最终容器是什么；目标集合完全由你要求 `collect()` 返回的类型决定。这使
    /// `collect()` 成为标准库中能力很强的方法之一，并在大量场景中出现。
    ///
    /// `collect()` 最基本的使用模式，是把一个集合转换成另一个集合。你取一个集合，
    /// 在其上调用 [`iter`]，执行一系列转换，最后调用 `collect()`。
    ///
    /// `collect()` 也可以创建那些并非典型集合的类型实例。例如，可以从 [`char`] 构造
    /// [`String`]，也可以把产出 [`Result<T, E>`][`Result`] 项的 iterator 收集为
    /// `Result<Collection<T>, E>`。更多内容见下面的示例。
    ///
    /// 因为 `collect()` 非常通用，它可能给类型推断带来问题。因此，`collect()` 是少数
    /// 经常需要看到被亲切称为“turbofish”的语法 `::<>` 的地方之一。该语法帮助推断
    /// 算法明确理解你想收集到哪一种集合中。
    ///
    /// # 示例
    ///
    /// 基本用法:
    ///
    /// ```
    /// let a = [1, 2, 3];
    ///
    /// let doubled: Vec<i32> = a.iter()
    ///                          .map(|x| x * 2)
    ///                          .collect();
    ///
    /// assert_eq!(vec![2, 4, 6], doubled);
    /// ```
    ///
    /// 注意，左侧需要写出 `: Vec<i32>`。这是因为也可以收集到其他类型中，例如
    /// [`VecDeque<T>`]:
    ///
    /// [`VecDeque<T>`]: ../../std/collections/struct.VecDeque.html
    ///
    /// ```
    /// use std::collections::VecDeque;
    ///
    /// let a = [1, 2, 3];
    ///
    /// let doubled: VecDeque<i32> = a.iter().map(|x| x * 2).collect();
    ///
    /// assert_eq!(2, doubled[0]);
    /// assert_eq!(4, doubled[1]);
    /// assert_eq!(6, doubled[2]);
    /// ```
    ///
    /// 使用 “turbofish” 而不是给 `doubled` 写类型标注:
    ///
    /// ```
    /// let a = [1, 2, 3];
    ///
    /// let doubled = a.iter().map(|x| x * 2).collect::<Vec<i32>>();
    ///
    /// assert_eq!(vec![2, 4, 6], doubled);
    /// ```
    ///
    /// 因为 `collect()` 只关心要收集到什么类型中，所以仍可在 turbofish 中使用部分
    /// 类型提示 `_`:
    ///
    /// ```
    /// let a = [1, 2, 3];
    ///
    /// let doubled = a.iter().map(|x| x * 2).collect::<Vec<_>>();
    ///
    /// assert_eq!(vec![2, 4, 6], doubled);
    /// ```
    ///
    /// 使用 `collect()` 创建 [`String`]:
    ///
    /// ```
    /// let chars = ['g', 'd', 'k', 'k', 'n'];
    ///
    /// let hello: String = chars.into_iter()
    ///     .map(|x| x as u8)
    ///     .map(|x| (x + 1) as char)
    ///     .collect();
    ///
    /// assert_eq!("hello", hello);
    /// ```
    ///
    /// 如果有一组 [`Result<T, E>`][`Result`]，可以使用 `collect()` 检查其中是否有
    /// 失败项:
    ///
    /// ```
    /// let results = [Ok(1), Err("nope"), Ok(3), Err("bad")];
    ///
    /// let result: Result<Vec<_>, &str> = results.into_iter().collect();
    ///
    /// // 得到第一个错误
    /// assert_eq!(Err("nope"), result);
    ///
    /// let results = [Ok(1), Ok(3)];
    ///
    /// let result: Result<Vec<_>, &str> = results.into_iter().collect();
    ///
    /// // 得到结果列表
    /// assert_eq!(Ok(vec![1, 3]), result);
    /// ```
    ///
    /// [`iter`]: Iterator::next
    /// [`String`]: ../../std/string/struct.String.html
    /// [`char`]: type@char
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[must_use = "if you really need to exhaust the iterator, consider `.for_each(drop)` instead"]
    #[rustc_diagnostic_item = "iterator_collect_fn"]
    fn collect<B: FromIterator<Self::Item>>(self) -> B
    where
        Self: Sized,
    {
        // 对所有场景一直启用这个检查过于激进，但 PR#137908 意外发现一些 rustc
        // iterator 的 `size_hint` 格式不正确，因此该检查可以帮助
        // debug-assertions-std runner 捕获这类问题，即使用户实际上不会看到它。
        if cfg!(debug_assertions) {
            let hint = self.size_hint();
            assert!(hint.1.is_none_or(|high| high >= hint.0), "Malformed size_hint {hint:?}");
        }

        FromIterator::from_iter(self)
    }

    /// 以可失败方式将 iterator 转换为集合，遇到失败时短路。
    ///
    /// `try_collect()` 是 [`collect()`][`collect`] 的变体，允许收集过程中发生可失败转换。
    /// 它的主要用途是简化从产出 [`Option<T>`][`Option`] 的 iterator 到
    /// `Option<Collection<T>>` 的转换；其他 [`Try`] 类型（例如 [`Result`]）也类似。
    ///
    /// 重要的是，`try_collect()` 不要求外层 [`Try`] 类型也实现 [`FromIterator`]；只有
    /// `Try::Output` 上产生的内层类型必须实现它。具体来说，这意味着收集到
    /// `ControlFlow<_, Vec<i32>>` 是有效的，因为 `Vec<i32>` 实现了 [`FromIterator`]，
    /// 即使 [`ControlFlow`] 没有实现。
    ///
    /// 此外，如果 `try_collect()` 期间遇到失败，iterator 仍然有效，并且可以继续使用。
    /// 在这种情况下，它会从触发失败的元素之后继续迭代。最后一个示例展示了这种行为。
    ///
    /// # 示例
    /// 成功把产出 `Option<i32>` 的 iterator 收集为 `Option<Vec<i32>>`:
    /// ```
    /// #![feature(iterator_try_collect)]
    ///
    /// let u = vec![Some(1), Some(2), Some(3)];
    /// let v = u.into_iter().try_collect::<Vec<i32>>();
    /// assert_eq!(v, Some(vec![1, 2, 3]));
    /// ```
    ///
    /// 以相同方式收集但失败:
    /// ```
    /// #![feature(iterator_try_collect)]
    ///
    /// let u = vec![Some(1), Some(2), None, Some(3)];
    /// let v = u.into_iter().try_collect::<Vec<i32>>();
    /// assert_eq!(v, None);
    /// ```
    ///
    /// 类似示例，但使用 `Result`:
    /// ```
    /// #![feature(iterator_try_collect)]
    ///
    /// let u: Vec<Result<i32, ()>> = vec![Ok(1), Ok(2), Ok(3)];
    /// let v = u.into_iter().try_collect::<Vec<i32>>();
    /// assert_eq!(v, Ok(vec![1, 2, 3]));
    ///
    /// let u = vec![Ok(1), Ok(2), Err(()), Ok(3)];
    /// let v = u.into_iter().try_collect::<Vec<i32>>();
    /// assert_eq!(v, Err(()));
    /// ```
    ///
    /// 最后，即使 [`ControlFlow`] 没有实现 [`FromIterator`]，它也可以工作。还要注意，
    /// 即使遇到失败，iterator 也可以继续使用:
    ///
    /// ```
    /// #![feature(iterator_try_collect)]
    ///
    /// use core::ops::ControlFlow::{Break, Continue};
    ///
    /// let u = [Continue(1), Continue(2), Break(3), Continue(4), Continue(5)];
    /// let mut it = u.into_iter();
    ///
    /// let v = it.try_collect::<Vec<_>>();
    /// assert_eq!(v, Break(3));
    ///
    /// let v = it.try_collect::<Vec<_>>();
    /// assert_eq!(v, Continue(vec![4, 5]));
    /// ```
    ///
    /// [`collect`]: Iterator::collect
    #[inline]
    #[unstable(feature = "iterator_try_collect", issue = "94047")]
    fn try_collect<B>(&mut self) -> ChangeOutputType<Self::Item, B>
    where
        Self: Sized,
        Self::Item: Try<Residual: Residual<B>>,
        B: FromIterator<<Self::Item as Try>::Output>,
    {
        try_process(ByRefSized(self), |i| i.collect())
    }

    /// 将 iterator 的所有项收集到一个集合中。
    ///
    /// 该方法会消耗 iterator，并把它的所有项加入传入的集合。随后返回该集合的可变
    /// 引用，因此调用链可以继续。
    ///
    /// 当你已经有一个集合，并希望把 iterator 的项加入其中时，该方法很有用。
    ///
    /// 该方法是调用 [Extend::extend](trait.Extend.html) 的便利形式；不同之处在于，
    /// 它是在 iterator 上调用，而不是在集合上调用。
    ///
    /// # 示例
    ///
    /// 基本用法:
    ///
    /// ```
    /// #![feature(iter_collect_into)]
    ///
    /// let a = [1, 2, 3];
    /// let mut vec: Vec::<i32> = vec![0, 1];
    ///
    /// a.iter().map(|x| x * 2).collect_into(&mut vec);
    /// a.iter().map(|x| x * 10).collect_into(&mut vec);
    ///
    /// assert_eq!(vec, vec![0, 1, 2, 4, 6, 10, 20, 30]);
    /// ```
    ///
    /// 可以手动设置 `Vec` 容量，以避免重新分配:
    ///
    /// ```
    /// #![feature(iter_collect_into)]
    ///
    /// let a = [1, 2, 3];
    /// let mut vec: Vec::<i32> = Vec::with_capacity(6);
    ///
    /// a.iter().map(|x| x * 2).collect_into(&mut vec);
    /// a.iter().map(|x| x * 10).collect_into(&mut vec);
    ///
    /// assert_eq!(6, vec.capacity());
    /// assert_eq!(vec, vec![2, 4, 6, 10, 20, 30]);
    /// ```
    ///
    /// 返回的可变引用可用于继续调用链:
    ///
    /// ```
    /// #![feature(iter_collect_into)]
    ///
    /// let a = [1, 2, 3];
    /// let mut vec: Vec::<i32> = Vec::with_capacity(6);
    ///
    /// let count = a.iter().collect_into(&mut vec).iter().count();
    ///
    /// assert_eq!(count, vec.len());
    /// assert_eq!(vec, vec![1, 2, 3]);
    ///
    /// let count = a.iter().collect_into(&mut vec).iter().count();
    ///
    /// assert_eq!(count, vec.len());
    /// assert_eq!(vec, vec![1, 2, 3, 1, 2, 3]);
    /// ```
    #[inline]
    #[unstable(feature = "iter_collect_into", issue = "94780")]
    fn collect_into<E: Extend<Self::Item>>(self, collection: &mut E) -> &mut E
    where
        Self: Sized,
    {
        collection.extend(self);
        collection
    }

    /// 消耗 iterator，并从中创建两个集合。
    ///
    /// 传给 `partition()` 的谓词可以返回 `true` 或 `false`。`partition()` 返回一对
    /// 集合: 第一个包含所有让谓词返回 `true` 的元素，第二个包含所有让谓词返回
    /// `false` 的元素。
    ///
    /// 另见 [`is_partitioned()`] 和 [`partition_in_place()`]。
    ///
    /// [`is_partitioned()`]: Iterator::is_partitioned
    /// [`partition_in_place()`]: Iterator::partition_in_place
    ///
    /// # 示例
    ///
    /// ```
    /// let a = [1, 2, 3];
    ///
    /// let (even, odd): (Vec<_>, Vec<_>) = a
    ///     .into_iter()
    ///     .partition(|n| n % 2 == 0);
    ///
    /// assert_eq!(even, [2]);
    /// assert_eq!(odd, [1, 3]);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    fn partition<B, F>(self, f: F) -> (B, B)
    where
        Self: Sized,
        B: Default + Extend<Self::Item>,
        F: FnMut(&Self::Item) -> bool,
    {
        #[inline]
        fn extend<'a, T, B: Extend<T>>(
            mut f: impl FnMut(&T) -> bool + 'a,
            left: &'a mut B,
            right: &'a mut B,
        ) -> impl FnMut((), T) + 'a {
            move |(), x| {
                if f(&x) {
                    left.extend_one(x);
                } else {
                    right.extend_one(x);
                }
            }
        }

        let mut left: B = Default::default();
        let mut right: B = Default::default();

        self.fold((), extend(f, &mut left, &mut right));

        (left, right)
    }

    /// 按给定谓词*原地*重排此 iterator 的元素，使所有返回 `true` 的元素排在所有返回
    /// `false` 的元素之前。返回找到的 `true` 元素数量。
    ///
    /// 分区后各项的相对顺序不会被保持。
    ///
    /// # 当前实现
    ///
    /// 当前算法会尝试找到第一个让谓词求值为 false 的元素，以及最后一个让谓词求值为
    /// true 的元素，并反复交换它们。
    ///
    /// 时间复杂度: *O*(*n*)
    ///
    /// 另见 [`is_partitioned()`] 和 [`partition()`]。
    ///
    /// [`is_partitioned()`]: Iterator::is_partitioned
    /// [`partition()`]: Iterator::partition
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(iter_partition_in_place)]
    ///
    /// let mut a = [1, 2, 3, 4, 5, 6, 7];
    ///
    /// // 在偶数和奇数之间原地分区
    /// let i = a.iter_mut().partition_in_place(|n| n % 2 == 0);
    ///
    /// assert_eq!(i, 3);
    /// assert!(a[..i].iter().all(|n| n % 2 == 0)); // 偶数
    /// assert!(a[i..].iter().all(|n| n % 2 == 1)); // 奇数
    /// ```
    #[unstable(feature = "iter_partition_in_place", issue = "62543")]
    fn partition_in_place<'a, T: 'a, P>(mut self, ref mut predicate: P) -> usize
    where
        Self: Sized + DoubleEndedIterator<Item = &'a mut T>,
        P: FnMut(&T) -> bool,
    {
        // FIXME: 是否应担心计数溢出？拥有超过 `usize::MAX` 个可变引用的唯一方式是
        // ZST，而对 ZST 进行分区并没有什么用途...

        // 这些闭包“工厂”函数用于避免在 `Self` 上引入泛型性。

        #[inline]
        fn is_false<'a, T>(
            predicate: &'a mut impl FnMut(&T) -> bool,
            true_count: &'a mut usize,
        ) -> impl FnMut(&&mut T) -> bool + 'a {
            move |x| {
                let p = predicate(&**x);
                *true_count += p as usize;
                !p
            }
        }

        #[inline]
        fn is_true<T>(predicate: &mut impl FnMut(&T) -> bool) -> impl FnMut(&&mut T) -> bool + '_ {
            move |x| predicate(&**x)
        }

        // 反复找到第一个 `false`，并与最后一个 `true` 交换。
        let mut true_count = 0;
        while let Some(head) = self.find(is_false(predicate, &mut true_count)) {
            if let Some(tail) = self.rfind(is_true(predicate)) {
                crate::mem::swap(head, tail);
                true_count += 1;
            } else {
                break;
            }
        }
        true_count
    }

    /// 检查此 iterator 的元素是否已按给定谓词分区，也就是所有返回 `true` 的元素都排在
    /// 所有返回 `false` 的元素之前。
    ///
    /// 另见 [`partition()`] 和 [`partition_in_place()`]。
    ///
    /// [`partition()`]: Iterator::partition
    /// [`partition_in_place()`]: Iterator::partition_in_place
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(iter_is_partitioned)]
    ///
    /// assert!("Iterator".chars().is_partitioned(char::is_uppercase));
    /// assert!(!"IntoIterator".chars().is_partitioned(char::is_uppercase));
    /// ```
    #[unstable(feature = "iter_is_partitioned", issue = "62544")]
    fn is_partitioned<P>(mut self, mut predicate: P) -> bool
    where
        Self: Sized,
        P: FnMut(Self::Item) -> bool,
    {
        // 要么所有项测试结果都是 `true`，要么第一个子句停在 `false` 处，
        // 然后检查其后不再有 `true` 项。
        self.all(&mut predicate) || !self.any(predicate)
    }

    /// 一个 iterator 方法，会在函数持续成功返回时不断应用它，并产生单个最终值。
    ///
    /// `try_fold()` 接收两个参数: 一个初始值，以及一个带两个参数的闭包。闭包参数为
    /// “累加器”和一个元素。闭包要么成功返回下一轮迭代中累加器应持有的值，要么返回
    /// 失败，并把错误值立即传播给调用方（短路）。
    ///
    /// 初始值就是第一次调用闭包时累加器持有的值。如果对 iterator 的每个元素应用闭包
    /// 都成功，`try_fold()` 会把最终累加器作为成功结果返回。
    ///
    /// 当你有一组内容并希望从中产生单个值时，folding 很有用。
    ///
    /// # 给实现者的说明
    ///
    /// 其他多个（前向）方法的默认实现都基于该方法，因此如果能比默认 `for` 循环实现
    /// 做得更好，请尽量显式实现它。
    ///
    /// 尤其应尽量让它在组成此 iterator 的内部部件上调用 `try_fold()`。如果需要多次
    /// 调用，`?` 运算符便于串接累加器值；但要注意在这些提前返回之前必须维护好的
    /// 任何不变量。该方法接收 `&mut self`，因此这里遇到错误之后，迭代仍必须可以恢复。
    ///
    /// # 示例
    ///
    /// 基本用法:
    ///
    /// ```
    /// let a = [1, 2, 3];
    ///
    /// // 对数组的所有元素执行带检查的求和
    /// let sum = a.into_iter().try_fold(0i8, |acc, x| acc.checked_add(x));
    ///
    /// assert_eq!(sum, Some(6));
    /// ```
    ///
    /// 短路行为:
    ///
    /// ```
    /// let a = [10, 20, 30, 100, 40, 50];
    /// let mut iter = a.into_iter();
    ///
    /// // 加到 100 这个元素时，该求和会溢出
    /// let sum = iter.try_fold(0i8, |acc, x| acc.checked_add(x));
    /// assert_eq!(sum, None);
    ///
    /// // 由于发生了短路，剩余元素仍可通过 iterator 取得。
    /// assert_eq!(iter.len(), 2);
    /// assert_eq!(iter.next(), Some(40));
    /// ```
    ///
    /// 虽然不能从闭包中直接 `break`，但 [`ControlFlow`] 类型允许表达类似想法:
    ///
    /// ```
    /// use std::ops::ControlFlow;
    ///
    /// let triangular = (1..30).try_fold(0_i8, |prev, x| {
    ///     if let Some(next) = prev.checked_add(x) {
    ///         ControlFlow::Continue(next)
    ///     } else {
    ///         ControlFlow::Break(prev)
    ///     }
    /// });
    /// assert_eq!(triangular, ControlFlow::Break(120));
    ///
    /// let triangular = (1..30).try_fold(0_u64, |prev, x| {
    ///     if let Some(next) = prev.checked_add(x) {
    ///         ControlFlow::Continue(next)
    ///     } else {
    ///         ControlFlow::Break(prev)
    ///     }
    /// });
    /// assert_eq!(triangular, ControlFlow::Continue(435));
    /// ```
    #[inline]
    #[stable(feature = "iterator_try_fold", since = "1.27.0")]
    fn try_fold<B, F, R>(&mut self, init: B, mut f: F) -> R
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> R,
        R: Try<Output = B>,
    {
        let mut accum = init;
        while let Some(x) = self.next() {
            accum = f(accum, x)?;
        }
        try { accum }
    }

    /// 一个 iterator 方法，会在每个项上应用可失败函数，并在第一个错误处停止且返回该错误。
    ///
    /// 也可以把它看作 [`for_each()`] 的可失败形式，或 [`try_fold()`] 的无状态版本。
    ///
    /// [`for_each()`]: Iterator::for_each
    /// [`try_fold()`]: Iterator::try_fold
    ///
    /// # 示例
    ///
    /// ```
    /// use std::fs::rename;
    /// use std::io::{stdout, Write};
    /// use std::path::Path;
    ///
    /// let data = ["no_tea.txt", "stale_bread.json", "torrential_rain.png"];
    ///
    /// let res = data.iter().try_for_each(|x| writeln!(stdout(), "{x}"));
    /// assert!(res.is_ok());
    ///
    /// let mut it = data.iter().cloned();
    /// let res = it.try_for_each(|x| rename(x, Path::new(x).with_extension("old")));
    /// assert!(res.is_err());
    /// // 它发生了短路，因此剩余项仍留在 iterator 中:
    /// assert_eq!(it.next(), Some("stale_bread.json"));
    /// ```
    ///
    /// 在普通循环中会使用 `break` 和 `continue` 的场景，也可以在该方法中配合
    /// [`ControlFlow`] 类型表达:
    ///
    /// ```
    /// use std::ops::ControlFlow;
    ///
    /// let r = (2..100).try_for_each(|x| {
    ///     if 323 % x == 0 {
    ///         return ControlFlow::Break(x)
    ///     }
    ///
    ///     ControlFlow::Continue(())
    /// });
    /// assert_eq!(r, ControlFlow::Break(17));
    /// ```
    #[inline]
    #[stable(feature = "iterator_try_fold", since = "1.27.0")]
    fn try_for_each<F, R>(&mut self, f: F) -> R
    where
        Self: Sized,
        F: FnMut(Self::Item) -> R,
        R: Try<Output = ()>,
    {
        #[inline]
        fn call<T, R>(mut f: impl FnMut(T) -> R) -> impl FnMut((), T) -> R {
            move |(), x| f(x)
        }

        self.try_fold((), call(f))
    }

    /// 通过应用一个操作，把每个元素折叠进累加器，并返回最终结果。
    ///
    /// `fold()` 接收两个参数: 一个初始值，以及一个带两个参数的闭包。闭包参数为
    /// “累加器”和一个元素；闭包返回下一轮迭代中累加器应持有的值。
    ///
    /// 初始值就是第一次调用闭包时累加器持有的值。
    ///
    /// 将该闭包应用到 iterator 的每个元素之后，`fold()` 返回累加器。
    ///
    /// 这个操作有时也称为“reduce”或“inject”。
    ///
    /// 当你有一组内容并希望从中产生单个值时，folding 很有用。
    ///
    /// 注意: `fold()` 以及其他会遍历整个 iterator 的类似方法，在无限 iterator 上可能
    /// 不会终止；即使对某些 trait 来说，结果本可在有限时间内确定，也是如此。
    ///
    /// 注意: 如果累加器类型和项类型相同，可以使用 [`reduce()`] 把第一个元素作为初始值。
    ///
    /// 注意: `fold()` 以*左结合*方式组合元素。对于 `+` 这类满足结合律的运算符，
    /// 元素组合顺序并不重要；但对于 `-` 这类不满足结合律的运算符，顺序会影响最终
    /// 结果。需要 `fold()` 的*右结合*版本时，见 [`DoubleEndedIterator::rfold()`]。
    ///
    /// # 给实现者的说明
    ///
    /// 其他多个（前向）方法的默认实现都基于该方法，因此如果能比默认 `for` 循环实现
    /// 做得更好，请尽量显式实现它。
    ///
    /// 尤其应尽量让它在组成此 iterator 的内部部件上调用 `fold()`。
    ///
    /// # 示例
    ///
    /// 基本用法:
    ///
    /// ```
    /// let a = [1, 2, 3];
    ///
    /// // 数组中所有元素的和
    /// let sum = a.iter().fold(0, |acc, x| acc + x);
    ///
    /// assert_eq!(sum, 6);
    /// ```
    ///
    /// 下面逐步查看这里的每一步迭代:
    ///
    /// | element | acc | x | result |
    /// |---------|-----|---|--------|
    /// |         | 0   |   |        |
    /// | 1       | 0   | 1 | 1      |
    /// | 2       | 1   | 2 | 3      |
    /// | 3       | 3   | 3 | 6      |
    ///
    /// 因此，最终结果为 `6`。
    ///
    /// 该示例展示 `fold()` 的左结合性质: 它从初始值开始构造字符串，并从前往后依次
    /// 处理每个元素:
    ///
    /// ```
    /// let numbers = [1, 2, 3, 4, 5];
    ///
    /// let zero = "0".to_string();
    ///
    /// let result = numbers.iter().fold(zero, |acc, &x| {
    ///     format!("({acc} + {x})")
    /// });
    ///
    /// assert_eq!(result, "(((((0 + 1) + 2) + 3) + 4) + 5)");
    /// ```
    /// 不常使用 iterator 的人，经常会用 `for` 循环遍历一组内容并逐步构造结果。这类
    /// 代码可以转换成 `fold()`:
    ///
    /// [`for`]: ../../book/ch03-05-control-flow.html#looping-through-a-collection-with-for
    ///
    /// ```
    /// let numbers = [1, 2, 3, 4, 5];
    ///
    /// let mut result = 0;
    ///
    /// // for 循环:
    /// for i in &numbers {
    ///     result = result + i;
    /// }
    ///
    /// // fold:
    /// let result2 = numbers.iter().fold(0, |acc, &x| acc + x);
    ///
    /// // 二者相同
    /// assert_eq!(result, result2);
    /// ```
    ///
    /// [`reduce()`]: Iterator::reduce
    #[doc(alias = "inject", alias = "foldl")]
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    fn fold<B, F>(mut self, init: B, mut f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        let mut accum = init;
        while let Some(x) = self.next() {
            accum = f(accum, x);
        }
        accum
    }

    /// 通过反复应用归约操作，把元素归约为单个元素。
    ///
    /// 如果 iterator 为空，则返回 [`None`]；否则返回归约结果。
    ///
    /// 归约函数是一个带两个参数的闭包: “累加器”和一个元素。对于至少有一个元素的
    /// iterator，这等价于以 iterator 的第一个元素作为初始累加器值调用 [`fold()`]，
    /// 并把后续每个元素折叠进去。
    ///
    /// [`fold()`]: Iterator::fold
    ///
    /// # 示例
    ///
    /// ```
    /// let reduced: i32 = (1..10).reduce(|acc, e| acc + e).unwrap_or(0);
    /// assert_eq!(reduced, 45);
    ///
    /// // 这等价于用 `fold` 完成:
    /// let folded: i32 = (1..10).fold(0, |acc, e| acc + e);
    /// assert_eq!(reduced, folded);
    /// ```
    #[inline]
    #[stable(feature = "iterator_fold_self", since = "1.51.0")]
    fn reduce<F>(mut self, f: F) -> Option<Self::Item>
    where
        Self: Sized,
        F: FnMut(Self::Item, Self::Item) -> Self::Item,
    {
        let first = self.next()?;
        Some(self.fold(first, f))
    }

    /// 通过反复应用归约操作，把元素归约为单个元素。如果闭包返回失败，该失败会立即
    /// 传播回调用方。
    ///
    /// 该方法的返回类型取决于闭包的返回类型。如果闭包返回 `Result<Self::Item, E>`，
    /// 则该函数返回 `Result<Option<Self::Item>, E>`。如果闭包返回
    /// `Option<Self::Item>`，则该函数返回 `Option<Option<Self::Item>>`。
    ///
    /// 在空 iterator 上调用时，该函数会根据所提供闭包的类型返回 `Some(None)` 或
    /// `Ok(None)`。
    ///
    /// 对于至少有一个元素的 iterator，这本质上等价于以 iterator 的第一个元素作为
    /// 初始累加器值调用 [`try_fold()`]。
    ///
    /// [`try_fold()`]: Iterator::try_fold
    ///
    /// # 示例
    ///
    /// 安全地计算一串数字之和:
    ///
    /// ```
    /// #![feature(iterator_try_reduce)]
    ///
    /// let numbers: Vec<usize> = vec![10, 20, 5, 23, 0];
    /// let sum = numbers.into_iter().try_reduce(|x, y| x.checked_add(y));
    /// assert_eq!(sum, Some(Some(58)));
    /// ```
    ///
    /// 判断归约何时发生短路:
    ///
    /// ```
    /// #![feature(iterator_try_reduce)]
    ///
    /// let numbers = vec![1, 2, 3, usize::MAX, 4, 5];
    /// let sum = numbers.into_iter().try_reduce(|x, y| x.checked_add(y));
    /// assert_eq!(sum, None);
    /// ```
    ///
    /// 判断因为没有元素而未执行归约的情况:
    ///
    /// ```
    /// #![feature(iterator_try_reduce)]
    ///
    /// let numbers: Vec<usize> = Vec::new();
    /// let sum = numbers.into_iter().try_reduce(|x, y| x.checked_add(y));
    /// assert_eq!(sum, Some(None));
    /// ```
    ///
    /// 使用 [`Result`] 而不是 [`Option`]:
    ///
    /// ```
    /// #![feature(iterator_try_reduce)]
    ///
    /// let numbers = vec!["1", "2", "3", "4", "5"];
    /// let max: Result<Option<_>, <usize as std::str::FromStr>::Err> =
    ///     numbers.into_iter().try_reduce(|x, y| {
    ///         if x.parse::<usize>()? > y.parse::<usize>()? { Ok(x) } else { Ok(y) }
    ///     });
    /// assert_eq!(max, Ok(Some("5")));
    /// ```
    #[inline]
    #[unstable(feature = "iterator_try_reduce", issue = "87053")]
    fn try_reduce<R>(
        &mut self,
        f: impl FnMut(Self::Item, Self::Item) -> R,
    ) -> ChangeOutputType<R, Option<R::Output>>
    where
        Self: Sized,
        R: Try<Output = Self::Item, Residual: Residual<Option<Self::Item>>>,
    {
        let first = match self.next() {
            Some(i) => i,
            None => return Try::from_output(None),
        };

        match self.try_fold(first, f).branch() {
            ControlFlow::Break(r) => FromResidual::from_residual(r),
            ControlFlow::Continue(i) => Try::from_output(Some(i)),
        }
    }

    /// 测试 iterator 的每个元素是否都匹配谓词。
    ///
    /// `all()` 接收一个返回 `true` 或 `false` 的闭包。它把该闭包应用到 iterator 的
    /// 每个元素；如果所有元素都返回 `true`，`all()` 也返回 `true`。如果任一元素返回
    /// `false`，则返回 `false`。
    ///
    /// `all()` 会短路；换句话说，一旦找到 `false`，它就会停止处理，因为无论之后
    /// 发生什么，结果都已经必然为 `false`。
    ///
    /// 空 iterator 返回 `true`。
    ///
    /// # 示例
    ///
    /// 基本用法:
    ///
    /// ```
    /// let a = [1, 2, 3];
    ///
    /// assert!(a.into_iter().all(|x| x > 0));
    ///
    /// assert!(!a.into_iter().all(|x| x > 2));
    /// ```
    ///
    /// 在第一个 `false` 处停止:
    ///
    /// ```
    /// let a = [1, 2, 3];
    ///
    /// let mut iter = a.into_iter();
    ///
    /// assert!(!iter.all(|x| x != 2));
    ///
    /// // 由于还有剩余元素，仍然可以继续使用 `iter`。
    /// assert_eq!(iter.next(), Some(3));
    /// ```
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    fn all<F>(&mut self, f: F) -> bool
    where
        Self: Sized,
        F: FnMut(Self::Item) -> bool,
    {
        #[inline]
        fn check<T>(mut f: impl FnMut(T) -> bool) -> impl FnMut((), T) -> ControlFlow<()> {
            move |(), x| {
                if f(x) { ControlFlow::Continue(()) } else { ControlFlow::Break(()) }
            }
        }
        self.try_fold((), check(f)) == ControlFlow::Continue(())
    }

    /// 测试 iterator 是否有任一元素匹配谓词。
    ///
    /// `any()` 接收一个返回 `true` 或 `false` 的闭包。它把该闭包应用到 iterator 的
    /// 每个元素；如果任一元素返回 `true`，`any()` 也返回 `true`。如果全部返回
    /// `false`，则返回 `false`。
    ///
    /// `any()` 会短路；换句话说，一旦找到 `true`，它就会停止处理，因为无论之后
    /// 发生什么，结果都已经必然为 `true`。
    ///
    /// 空 iterator 返回 `false`。
    ///
    /// # 示例
    ///
    /// 基本用法:
    ///
    /// ```
    /// let a = [1, 2, 3];
    ///
    /// assert!(a.into_iter().any(|x| x > 0));
    ///
    /// assert!(!a.into_iter().any(|x| x > 5));
    /// ```
    ///
    /// 在第一个 `true` 处停止:
    ///
    /// ```
    /// let a = [1, 2, 3];
    ///
    /// let mut iter = a.into_iter();
    ///
    /// assert!(iter.any(|x| x != 2));
    ///
    /// // 由于还有剩余元素，仍然可以继续使用 `iter`。
    /// assert_eq!(iter.next(), Some(2));
    /// ```
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    fn any<F>(&mut self, f: F) -> bool
    where
        Self: Sized,
        F: FnMut(Self::Item) -> bool,
    {
        #[inline]
        fn check<T>(mut f: impl FnMut(T) -> bool) -> impl FnMut((), T) -> ControlFlow<()> {
            move |(), x| {
                if f(x) { ControlFlow::Break(()) } else { ControlFlow::Continue(()) }
            }
        }

        self.try_fold((), check(f)) == ControlFlow::Break(())
    }

    /// 搜索 iterator 中满足谓词的元素。
    ///
    /// `find()` 接收一个返回 `true` 或 `false` 的闭包。它把该闭包应用到 iterator 的
    /// 每个元素；如果任一元素返回 `true`，`find()` 返回 [`Some(element)`]。如果全部
    /// 返回 `false`，则返回 [`None`]。
    ///
    /// `find()` 会短路；换句话说，一旦闭包返回 `true`，它就会停止处理。
    ///
    /// 因为 `find()` 接收引用，而很多 iterator 本身就迭代引用，所以可能出现让人困惑的
    /// 双重引用参数。下面示例中的 `&&x` 展示了这种效果。
    ///
    /// 如果需要元素索引，见 [`position()`]。
    ///
    /// [`Some(element)`]: Some
    /// [`position()`]: Iterator::position
    ///
    /// # 示例
    ///
    /// 基本用法:
    ///
    /// ```
    /// let a = [1, 2, 3];
    ///
    /// assert_eq!(a.into_iter().find(|&x| x == 2), Some(2));
    /// assert_eq!(a.into_iter().find(|&x| x == 5), None);
    /// ```
    ///
    /// 迭代引用:
    ///
    /// ```
    /// let a = [1, 2, 3];
    ///
    /// // `iter()` 产出引用，即 `&i32`；而 `find()` 会取得每个元素的引用。
    /// assert_eq!(a.iter().find(|&&x| x == 2), Some(&2));
    /// assert_eq!(a.iter().find(|&&x| x == 5), None);
    /// ```
    ///
    /// 在第一个 `true` 处停止:
    ///
    /// ```
    /// let a = [1, 2, 3];
    ///
    /// let mut iter = a.into_iter();
    ///
    /// assert_eq!(iter.find(|&x| x == 2), Some(2));
    ///
    /// // 由于还有剩余元素，仍然可以继续使用 `iter`。
    /// assert_eq!(iter.next(), Some(3));
    /// ```
    ///
    /// 注意，`iter.find(f)` 等价于 `iter.filter(f).next()`。
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    fn find<P>(&mut self, predicate: P) -> Option<Self::Item>
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

        self.try_fold((), check(predicate)).break_value()
    }

    /// 将函数应用到 iterator 的元素，并返回第一个非 none 的结果。
    ///
    /// `iter.find_map(f)` 等价于 `iter.filter_map(f).next()`。
    ///
    /// # 示例
    ///
    /// ```
    /// let a = ["lol", "NaN", "2", "5"];
    ///
    /// let first_number = a.iter().find_map(|s| s.parse().ok());
    ///
    /// assert_eq!(first_number, Some(2));
    /// ```
    #[inline]
    #[stable(feature = "iterator_find_map", since = "1.30.0")]
    fn find_map<B, F>(&mut self, f: F) -> Option<B>
    where
        Self: Sized,
        F: FnMut(Self::Item) -> Option<B>,
    {
        #[inline]
        fn check<T, B>(mut f: impl FnMut(T) -> Option<B>) -> impl FnMut((), T) -> ControlFlow<B> {
            move |(), x| match f(x) {
                Some(x) => ControlFlow::Break(x),
                None => ControlFlow::Continue(()),
            }
        }

        self.try_fold((), check(f)).break_value()
    }

    /// 将函数应用到 iterator 的元素，并返回第一个 true 结果或第一个错误。
    ///
    /// 该方法的返回类型取决于闭包的返回类型。如果闭包返回 `Result<bool, E>`，
    /// 则得到 `Result<Option<Self::Item>, E>`；如果闭包返回 `Option<bool>`，
    /// 则得到 `Option<Option<Self::Item>>`。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(try_find)]
    ///
    /// let a = ["1", "2", "lol", "NaN", "5"];
    ///
    /// let is_my_num = |s: &str, search: i32| -> Result<bool, std::num::ParseIntError> {
    ///     Ok(s.parse::<i32>()? == search)
    /// };
    ///
    /// let result = a.into_iter().try_find(|&s| is_my_num(s, 2));
    /// assert_eq!(result, Ok(Some("2")));
    ///
    /// let result = a.into_iter().try_find(|&s| is_my_num(s, 5));
    /// assert!(result.is_err());
    /// ```
    ///
    /// 这也支持其他实现 [`Try`] 的类型，而不只是 [`Result`]。
    ///
    /// ```
    /// #![feature(try_find)]
    ///
    /// use std::num::NonZero;
    ///
    /// let a = [3, 5, 7, 4, 9, 0, 11u32];
    /// let result = a.into_iter().try_find(|&x| NonZero::new(x).map(|y| y.is_power_of_two()));
    /// assert_eq!(result, Some(Some(4)));
    /// let result = a.into_iter().take(3).try_find(|&x| NonZero::new(x).map(|y| y.is_power_of_two()));
    /// assert_eq!(result, Some(None));
    /// let result = a.into_iter().rev().try_find(|&x| NonZero::new(x).map(|y| y.is_power_of_two()));
    /// assert_eq!(result, None);
    /// ```
    #[inline]
    #[unstable(feature = "try_find", issue = "63178")]
    fn try_find<R>(
        &mut self,
        f: impl FnMut(&Self::Item) -> R,
    ) -> ChangeOutputType<R, Option<Self::Item>>
    where
        Self: Sized,
        R: Try<Output = bool, Residual: Residual<Option<Self::Item>>>,
    {
        #[inline]
        fn check<I, V, R>(
            mut f: impl FnMut(&I) -> V,
        ) -> impl FnMut((), I) -> ControlFlow<R::TryType>
        where
            V: Try<Output = bool, Residual = R>,
            R: Residual<Option<I>>,
        {
            move |(), x| match f(&x).branch() {
                ControlFlow::Continue(false) => ControlFlow::Continue(()),
                ControlFlow::Continue(true) => ControlFlow::Break(Try::from_output(Some(x))),
                ControlFlow::Break(r) => ControlFlow::Break(FromResidual::from_residual(r)),
            }
        }

        match self.try_fold((), check(f)) {
            ControlFlow::Break(x) => x,
            ControlFlow::Continue(()) => Try::from_output(None),
        }
    }

    /// 在 iterator 中搜索元素，并返回其索引。
    ///
    /// `position()` 接收一个返回 `true` 或 `false` 的闭包。它把该闭包应用到 iterator
    /// 的每个元素；如果某个元素返回 `true`，`position()` 返回 [`Some(index)`]。
    /// 如果全部返回 `false`，则返回 [`None`]。
    ///
    /// `position()` 会短路；换句话说，一旦找到 `true`，它就会停止处理。
    ///
    /// # 溢出行为
    ///
    /// 该方法不会额外防护溢出，因此如果不匹配元素数量超过 [`usize::MAX`]，它要么产生
    /// 错误结果，要么 panic。如果启用了溢出检查，则保证会 panic。
    ///
    /// # Panics
    ///
    /// 如果 iterator 中不匹配元素超过 `usize::MAX` 个，本函数可能 panic。
    ///
    /// [`Some(index)`]: Some
    ///
    /// # 示例
    ///
    /// 基本用法:
    ///
    /// ```
    /// let a = [1, 2, 3];
    ///
    /// assert_eq!(a.into_iter().position(|x| x == 2), Some(1));
    ///
    /// assert_eq!(a.into_iter().position(|x| x == 5), None);
    /// ```
    ///
    /// 在第一个 `true` 处停止:
    ///
    /// ```
    /// let a = [1, 2, 3, 4];
    ///
    /// let mut iter = a.into_iter();
    ///
    /// assert_eq!(iter.position(|x| x >= 2), Some(1));
    ///
    /// // 由于还有剩余元素，仍然可以继续使用 `iter`。
    /// assert_eq!(iter.next(), Some(3));
    ///
    /// // 返回索引取决于 iterator 当前状态
    /// assert_eq!(iter.position(|x| x == 4), Some(0));
    ///
    /// ```
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    fn position<P>(&mut self, predicate: P) -> Option<usize>
    where
        Self: Sized,
        P: FnMut(Self::Item) -> bool,
    {
        #[inline]
        fn check<'a, T>(
            mut predicate: impl FnMut(T) -> bool + 'a,
            acc: &'a mut usize,
        ) -> impl FnMut((), T) -> ControlFlow<usize, ()> + 'a {
            #[rustc_inherit_overflow_checks]
            move |_, x| {
                if predicate(x) {
                    ControlFlow::Break(*acc)
                } else {
                    *acc += 1;
                    ControlFlow::Continue(())
                }
            }
        }

        let mut acc = 0;
        self.try_fold((), check(predicate, &mut acc)).break_value()
    }

    /// 从右侧开始在 iterator 中搜索元素，并返回其索引。
    ///
    /// `rposition()` 接收一个返回 `true` 或 `false` 的闭包。它从末端开始把该闭包应用到
    /// iterator 的每个元素；如果某个元素返回 `true`，`rposition()` 返回
    /// [`Some(index)`]。如果全部返回 `false`，则返回 [`None`]。
    ///
    /// `rposition()` 会短路；换句话说，一旦找到 `true`，它就会停止处理。
    ///
    /// [`Some(index)`]: Some
    ///
    /// # 示例
    ///
    /// 基本用法:
    ///
    /// ```
    /// let a = [1, 2, 3];
    ///
    /// assert_eq!(a.into_iter().rposition(|x| x == 3), Some(2));
    ///
    /// assert_eq!(a.into_iter().rposition(|x| x == 5), None);
    /// ```
    ///
    /// 在第一个 `true` 处停止:
    ///
    /// ```
    /// let a = [-1, 2, 3, 4];
    ///
    /// let mut iter = a.into_iter();
    ///
    /// assert_eq!(iter.rposition(|x| x >= 2), Some(3));
    ///
    /// // 由于还有剩余元素，仍然可以继续使用 `iter`。
    /// assert_eq!(iter.next(), Some(-1));
    /// assert_eq!(iter.next_back(), Some(3));
    /// ```
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    fn rposition<P>(&mut self, predicate: P) -> Option<usize>
    where
        P: FnMut(Self::Item) -> bool,
        Self: Sized + ExactSizeIterator + DoubleEndedIterator,
    {
        // 这里不需要溢出检查，因为 `ExactSizeIterator` 意味着元素数量能放入 `usize`。
        #[inline]
        fn check<T>(
            mut predicate: impl FnMut(T) -> bool,
        ) -> impl FnMut(usize, T) -> ControlFlow<usize, usize> {
            move |i, x| {
                let i = i - 1;
                if predicate(x) { ControlFlow::Break(i) } else { ControlFlow::Continue(i) }
            }
        }

        let n = self.len();
        self.try_rfold(n, check(predicate)).break_value()
    }

    /// 返回 iterator 中的最大元素。
    ///
    /// 如果多个元素并列最大，则返回最后一个。如果 iterator 为空，则返回 [`None`]。
    ///
    /// 注意，由于 NaN 不可比较，[`f32`]/[`f64`] 没有实现 [`Ord`]。可以使用
    /// [`Iterator::reduce`] 绕过这一点:
    /// ```
    /// assert_eq!(
    ///     [2.4, f32::NAN, 1.3]
    ///         .into_iter()
    ///         .reduce(f32::max)
    ///         .unwrap_or(0.),
    ///     2.4
    /// );
    /// ```
    ///
    /// # 示例
    ///
    /// ```
    /// let a = [1, 2, 3];
    /// let b: [u32; 0] = [];
    ///
    /// assert_eq!(a.into_iter().max(), Some(3));
    /// assert_eq!(b.into_iter().max(), None);
    /// ```
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    fn max(self) -> Option<Self::Item>
    where
        Self: Sized,
        Self::Item: Ord,
    {
        self.max_by(Ord::cmp)
    }

    /// 返回 iterator 中的最小元素。
    ///
    /// 如果多个元素并列最小，则返回第一个。如果 iterator 为空，则返回 [`None`]。
    ///
    /// 注意，由于 NaN 不可比较，[`f32`]/[`f64`] 没有实现 [`Ord`]。可以使用
    /// [`Iterator::reduce`] 绕过这一点:
    /// ```
    /// assert_eq!(
    ///     [2.4, f32::NAN, 1.3]
    ///         .into_iter()
    ///         .reduce(f32::min)
    ///         .unwrap_or(0.),
    ///     1.3
    /// );
    /// ```
    ///
    /// # 示例
    ///
    /// ```
    /// let a = [1, 2, 3];
    /// let b: [u32; 0] = [];
    ///
    /// assert_eq!(a.into_iter().min(), Some(1));
    /// assert_eq!(b.into_iter().min(), None);
    /// ```
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    fn min(self) -> Option<Self::Item>
    where
        Self: Sized,
        Self::Item: Ord,
    {
        self.min_by(Ord::cmp)
    }

    /// 返回使指定函数产生最大值的元素。
    ///
    /// 如果多个元素并列最大，则返回最后一个。如果 iterator 为空，则返回 [`None`]。
    ///
    /// # 示例
    ///
    /// ```
    /// let a = [-3_i32, 0, 1, 5, -10];
    /// assert_eq!(a.into_iter().max_by_key(|x| x.abs()).unwrap(), -10);
    /// ```
    #[inline]
    #[stable(feature = "iter_cmp_by_key", since = "1.6.0")]
    fn max_by_key<B: Ord, F>(self, f: F) -> Option<Self::Item>
    where
        Self: Sized,
        F: FnMut(&Self::Item) -> B,
    {
        #[inline]
        fn key<T, B>(mut f: impl FnMut(&T) -> B) -> impl FnMut(T) -> (B, T) {
            move |x| (f(&x), x)
        }

        #[inline]
        fn compare<T, B: Ord>((x_p, _): &(B, T), (y_p, _): &(B, T)) -> Ordering {
            x_p.cmp(y_p)
        }

        let (_, x) = self.map(key(f)).max_by(compare)?;
        Some(x)
    }

    /// 按指定比较函数返回最大元素。
    ///
    /// 如果多个元素并列最大，则返回最后一个。如果 iterator 为空，则返回 [`None`]。
    ///
    /// # 示例
    ///
    /// ```
    /// let a = [-3_i32, 0, 1, 5, -10];
    /// assert_eq!(a.into_iter().max_by(|x, y| x.cmp(y)).unwrap(), 5);
    /// ```
    #[inline]
    #[stable(feature = "iter_max_by", since = "1.15.0")]
    fn max_by<F>(self, compare: F) -> Option<Self::Item>
    where
        Self: Sized,
        F: FnMut(&Self::Item, &Self::Item) -> Ordering,
    {
        #[inline]
        fn fold<T>(mut compare: impl FnMut(&T, &T) -> Ordering) -> impl FnMut(T, T) -> T {
            move |x, y| cmp::max_by(x, y, &mut compare)
        }

        self.reduce(fold(compare))
    }

    /// 返回使指定函数产生最小值的元素。
    ///
    /// 如果多个元素并列最小，则返回第一个。如果 iterator 为空，则返回 [`None`]。
    ///
    /// # 示例
    ///
    /// ```
    /// let a = [-3_i32, 0, 1, 5, -10];
    /// assert_eq!(a.into_iter().min_by_key(|x| x.abs()).unwrap(), 0);
    /// ```
    #[inline]
    #[stable(feature = "iter_cmp_by_key", since = "1.6.0")]
    fn min_by_key<B: Ord, F>(self, f: F) -> Option<Self::Item>
    where
        Self: Sized,
        F: FnMut(&Self::Item) -> B,
    {
        #[inline]
        fn key<T, B>(mut f: impl FnMut(&T) -> B) -> impl FnMut(T) -> (B, T) {
            move |x| (f(&x), x)
        }

        #[inline]
        fn compare<T, B: Ord>((x_p, _): &(B, T), (y_p, _): &(B, T)) -> Ordering {
            x_p.cmp(y_p)
        }

        let (_, x) = self.map(key(f)).min_by(compare)?;
        Some(x)
    }

    /// 按指定比较函数返回最小元素。
    ///
    /// 如果多个元素并列最小，则返回第一个。如果 iterator 为空，则返回 [`None`]。
    ///
    /// # 示例
    ///
    /// ```
    /// let a = [-3_i32, 0, 1, 5, -10];
    /// assert_eq!(a.into_iter().min_by(|x, y| x.cmp(y)).unwrap(), -10);
    /// ```
    #[inline]
    #[stable(feature = "iter_min_by", since = "1.15.0")]
    fn min_by<F>(self, compare: F) -> Option<Self::Item>
    where
        Self: Sized,
        F: FnMut(&Self::Item, &Self::Item) -> Ordering,
    {
        #[inline]
        fn fold<T>(mut compare: impl FnMut(&T, &T) -> Ordering) -> impl FnMut(T, T) -> T {
            move |x, y| cmp::min_by(x, y, &mut compare)
        }

        self.reduce(fold(compare))
    }

    /// 反转 iterator 的方向。
    ///
    /// 通常 iterator 从左到右迭代。使用 `rev()` 后，iterator 会改为从右到左迭代。
    ///
    /// 只有 iterator 有末端时才可能做到这一点，因此 `rev()` 只适用于
    /// [`DoubleEndedIterator`]。
    ///
    /// # 示例
    ///
    /// ```
    /// let a = [1, 2, 3];
    ///
    /// let mut iter = a.into_iter().rev();
    ///
    /// assert_eq!(iter.next(), Some(3));
    /// assert_eq!(iter.next(), Some(2));
    /// assert_eq!(iter.next(), Some(1));
    ///
    /// assert_eq!(iter.next(), None);
    /// ```
    #[inline]
    #[doc(alias = "reverse")]
    #[stable(feature = "rust1", since = "1.0.0")]
    fn rev(self) -> Rev<Self>
    where
        Self: Sized + DoubleEndedIterator,
    {
        Rev::new(self)
    }

    /// 将由成对元素组成的 iterator 转换为一对容器。
    ///
    /// `unzip()` 会消耗整个成对元素 iterator，并产生两个集合: 一个来自每对元素的左侧，
    /// 另一个来自右侧。
    ///
    /// 从某种意义上说，该函数与 [`zip`] 相反。
    ///
    /// [`zip`]: Iterator::zip
    ///
    /// # 示例
    ///
    /// ```
    /// let a = [(1, 2), (3, 4), (5, 6)];
    ///
    /// let (left, right): (Vec<_>, Vec<_>) = a.into_iter().unzip();
    ///
    /// assert_eq!(left, [1, 3, 5]);
    /// assert_eq!(right, [2, 4, 6]);
    ///
    /// // 也可以一次 unzip 多层嵌套元组
    /// let a = [(1, (2, 3)), (4, (5, 6))];
    ///
    /// let (x, (y, z)): (Vec<_>, (Vec<_>, Vec<_>)) = a.into_iter().unzip();
    /// assert_eq!(x, [1, 4]);
    /// assert_eq!(y, [2, 5]);
    /// assert_eq!(z, [3, 6]);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    fn unzip<A, B, FromA, FromB>(self) -> (FromA, FromB)
    where
        FromA: Default + Extend<A>,
        FromB: Default + Extend<B>,
        Self: Sized + Iterator<Item = (A, B)>,
    {
        let mut unzipped: (FromA, FromB) = Default::default();
        unzipped.extend(self);
        unzipped
    }

    /// 创建一个会复制其所有元素的 iterator。
    ///
    /// 当你有一个遍历 `&T` 的 iterator，但需要遍历 `T` 的 iterator 时，该方法很有用。
    ///
    /// # 示例
    ///
    /// ```
    /// let a = [1, 2, 3];
    ///
    /// let v_copied: Vec<_> = a.iter().copied().collect();
    ///
    /// // copied 等同于 .map(|&x| x)
    /// let v_map: Vec<_> = a.iter().map(|&x| x).collect();
    ///
    /// assert_eq!(v_copied, [1, 2, 3]);
    /// assert_eq!(v_map, [1, 2, 3]);
    /// ```
    #[stable(feature = "iter_copied", since = "1.36.0")]
    #[rustc_diagnostic_item = "iter_copied"]
    fn copied<'a, T>(self) -> Copied<Self>
    where
        T: Copy + 'a,
        Self: Sized + Iterator<Item = &'a T>,
    {
        Copied::new(self)
    }

    /// 创建一个会 [`clone`] 其所有元素的 iterator。
    ///
    /// 当你有一个遍历 `&T` 的 iterator，但需要遍历 `T` 的 iterator 时，该方法很有用。
    ///
    /// 完全不保证 `clone` 方法一定会实际被调用，也不保证一定会被优化掉。因此代码不应
    /// 依赖这两种情况中的任何一种。
    ///
    /// [`clone`]: Clone::clone
    ///
    /// # 示例
    ///
    /// 基本用法:
    ///
    /// ```
    /// let a = [1, 2, 3];
    ///
    /// let v_cloned: Vec<_> = a.iter().cloned().collect();
    ///
    /// // 对整数来说，cloned 等同于 .map(|&x| x)
    /// let v_map: Vec<_> = a.iter().map(|&x| x).collect();
    ///
    /// assert_eq!(v_cloned, [1, 2, 3]);
    /// assert_eq!(v_map, [1, 2, 3]);
    /// ```
    ///
    /// 为获得最佳性能，尽量延后 clone:
    ///
    /// ```
    /// let a = [vec![0_u8, 1, 2], vec![3, 4], vec![23]];
    /// // 不要这样做:
    /// let slower: Vec<_> = a.iter().cloned().filter(|s| s.len() == 1).collect();
    /// assert_eq!(&[vec![23]], &slower[..]);
    /// // 改为更晚调用 `cloned`
    /// let faster: Vec<_> = a.iter().filter(|s| s.len() == 1).cloned().collect();
    /// assert_eq!(&[vec![23]], &faster[..]);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_diagnostic_item = "iter_cloned"]
    fn cloned<'a, T>(self) -> Cloned<Self>
    where
        T: Clone + 'a,
        Self: Sized + Iterator<Item = &'a T>,
    {
        Cloned::new(self)
    }

    /// 无限重复一个 iterator。
    ///
    /// iterator 不会在 [`None`] 处停止，而是会从头再次开始。再次迭代完后，它又会从头
    /// 开始。如此反复，永不结束。注意，如果原 iterator 为空，结果 iterator 也为空。
    ///
    /// # 示例
    ///
    /// ```
    /// let a = [1, 2, 3];
    ///
    /// let mut iter = a.into_iter().cycle();
    ///
    /// loop {
    ///     assert_eq!(iter.next(), Some(1));
    ///     assert_eq!(iter.next(), Some(2));
    ///     assert_eq!(iter.next(), Some(3));
    /// #   break;
    /// }
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    fn cycle(self) -> Cycle<Self>
    where
        Self: Sized + Clone,
    {
        Cycle::new(self)
    }

    /// 返回一个每次遍历原 iterator 中 `N` 个元素的 iterator。
    ///
    /// 这些块不会重叠。如果 `N` 不能整除 iterator 的长度，那么最后最多 `N-1` 个元素
    /// 会被省略，并可通过该 iterator 的
    /// [`.into_remainder()`][ArrayChunks::into_remainder] 函数取回。
    ///
    /// # Panics
    ///
    /// 如果 `N` 为零则 panic。
    ///
    /// # 示例
    ///
    /// 基本用法:
    ///
    /// ```
    /// #![feature(iter_array_chunks)]
    ///
    /// let mut iter = "lorem".chars().array_chunks();
    /// assert_eq!(iter.next(), Some(['l', 'o']));
    /// assert_eq!(iter.next(), Some(['r', 'e']));
    /// assert_eq!(iter.next(), None);
    /// assert_eq!(iter.into_remainder().as_slice(), &['m']);
    /// ```
    ///
    /// ```
    /// #![feature(iter_array_chunks)]
    ///
    /// let data = [1, 1, 2, -2, 6, 0, 3, 1];
    /// //          ^-----^  ^------^
    /// for [x, y, z] in data.iter().array_chunks() {
    ///     assert_eq!(x + y + z, 4);
    /// }
    /// ```
    #[track_caller]
    #[unstable(feature = "iter_array_chunks", issue = "100450")]
    fn array_chunks<const N: usize>(self) -> ArrayChunks<Self, N>
    where
        Self: Sized,
    {
        ArrayChunks::new(self)
    }

    /// 对 iterator 的元素求和。
    ///
    /// 取出每个元素，将它们相加，并返回结果。
    ///
    /// 空 iterator 返回该类型的*加法单位元*（“零”），也就是整数的 `0` 和浮点数的
    /// `-0.0`。
    ///
    /// `sum()` 可用于对任何实现 [`Sum`][`core::iter::Sum`] 的类型求和，包括
    /// [`Option`][`Option::sum`] 和 [`Result`][`Result::sum`]。
    ///
    /// # Panics
    ///
    /// 调用 `sum()` 且返回的是基本整数类型时，如果计算溢出并且启用了溢出检查，该方法
    /// 会 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// let a = [1, 2, 3];
    /// let sum: i32 = a.iter().sum();
    ///
    /// assert_eq!(sum, 6);
    ///
    /// let b: Vec<f32> = vec![];
    /// let sum: f32 = b.iter().sum();
    /// assert_eq!(sum, -0.0_f32);
    /// ```
    #[stable(feature = "iter_arith", since = "1.11.0")]
    fn sum<S>(self) -> S
    where
        Self: Sized,
        S: Sum<Self::Item>,
    {
        Sum::sum(self)
    }

    /// 遍历整个 iterator，并把所有元素相乘。
    ///
    /// 空 iterator 返回该类型的一值。
    ///
    /// `product()` 可用于对任何实现 [`Product`][`core::iter::Product`] 的类型求积，
    /// 包括 [`Option`][`Option::product`] 和 [`Result`][`Result::product`]。
    ///
    /// # Panics
    ///
    /// 调用 `product()` 且返回的是基本整数类型时，如果计算溢出并且启用了溢出检查，
    /// 该方法会 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// fn factorial(n: u32) -> u32 {
    ///     (1..=n).product()
    /// }
    /// assert_eq!(factorial(0), 1);
    /// assert_eq!(factorial(1), 1);
    /// assert_eq!(factorial(5), 120);
    /// ```
    #[stable(feature = "iter_arith", since = "1.11.0")]
    fn product<P>(self) -> P
    where
        Self: Sized,
        P: Product<Self::Item>,
    {
        Product::product(self)
    }

    /// 按[字典序](Ord#lexicographical-comparison)比较此 [`Iterator`] 的元素与另一
    /// iterator 的元素。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::cmp::Ordering;
    ///
    /// assert_eq!([1].iter().cmp([1].iter()), Ordering::Equal);
    /// assert_eq!([1].iter().cmp([1, 2].iter()), Ordering::Less);
    /// assert_eq!([1, 2].iter().cmp([1].iter()), Ordering::Greater);
    /// ```
    #[stable(feature = "iter_order", since = "1.5.0")]
    fn cmp<I>(self, other: I) -> Ordering
    where
        I: IntoIterator<Item = Self::Item>,
        Self::Item: Ord,
        Self: Sized,
    {
        self.cmp_by(other, |x, y| x.cmp(&y))
    }

    /// 使用指定比较函数，按[字典序](Ord#lexicographical-comparison)比较此 [`Iterator`]
    /// 的元素与另一 iterator 的元素。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(iter_order_by)]
    ///
    /// use std::cmp::Ordering;
    ///
    /// let xs = [1, 2, 3, 4];
    /// let ys = [1, 4, 9, 16];
    ///
    /// assert_eq!(xs.into_iter().cmp_by(ys, |x, y| x.cmp(&y)), Ordering::Less);
    /// assert_eq!(xs.into_iter().cmp_by(ys, |x, y| (x * x).cmp(&y)), Ordering::Equal);
    /// assert_eq!(xs.into_iter().cmp_by(ys, |x, y| (2 * x).cmp(&y)), Ordering::Greater);
    /// ```
    #[unstable(feature = "iter_order_by", issue = "64295")]
    fn cmp_by<I, F>(self, other: I, cmp: F) -> Ordering
    where
        Self: Sized,
        I: IntoIterator,
        F: FnMut(Self::Item, I::Item) -> Ordering,
    {
        #[inline]
        fn compare<X, Y, F>(mut cmp: F) -> impl FnMut(X, Y) -> ControlFlow<Ordering>
        where
            F: FnMut(X, Y) -> Ordering,
        {
            move |x, y| match cmp(x, y) {
                Ordering::Equal => ControlFlow::Continue(()),
                non_eq => ControlFlow::Break(non_eq),
            }
        }

        match iter_compare(self, other.into_iter(), compare(cmp)) {
            ControlFlow::Continue(ord) => ord,
            ControlFlow::Break(ord) => ord,
        }
    }

    /// 按[字典序](Ord#lexicographical-comparison)比较此 [`Iterator`] 中实现
    /// [`PartialOrd`] 的元素与另一 iterator 的元素。比较过程类似短路求值，会在不比较
    /// 剩余元素的情况下返回结果。一旦能够确定顺序，求值就会停止并返回结果。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::cmp::Ordering;
    ///
    /// assert_eq!([1.].iter().partial_cmp([1.].iter()), Some(Ordering::Equal));
    /// assert_eq!([1.].iter().partial_cmp([1., 2.].iter()), Some(Ordering::Less));
    /// assert_eq!([1., 2.].iter().partial_cmp([1.].iter()), Some(Ordering::Greater));
    /// ```
    ///
    /// 对浮点数来说，NaN 没有全序，比较时会得到 `None`:
    ///
    /// ```
    /// assert_eq!([f64::NAN].iter().partial_cmp([1.].iter()), None);
    /// ```
    ///
    /// 结果由求值顺序决定。
    ///
    /// ```
    /// use std::cmp::Ordering;
    ///
    /// assert_eq!([1.0, f64::NAN].iter().partial_cmp([2.0, f64::NAN].iter()), Some(Ordering::Less));
    /// assert_eq!([2.0, f64::NAN].iter().partial_cmp([1.0, f64::NAN].iter()), Some(Ordering::Greater));
    /// assert_eq!([f64::NAN, 1.0].iter().partial_cmp([f64::NAN, 2.0].iter()), None);
    /// ```
    ///
    #[stable(feature = "iter_order", since = "1.5.0")]
    fn partial_cmp<I>(self, other: I) -> Option<Ordering>
    where
        I: IntoIterator,
        Self::Item: PartialOrd<I::Item>,
        Self: Sized,
    {
        self.partial_cmp_by(other, |x, y| x.partial_cmp(&y))
    }

    /// 使用指定比较函数，按[字典序](Ord#lexicographical-comparison)比较此 [`Iterator`]
    /// 的元素与另一 iterator 的元素。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(iter_order_by)]
    ///
    /// use std::cmp::Ordering;
    ///
    /// let xs = [1.0, 2.0, 3.0, 4.0];
    /// let ys = [1.0, 4.0, 9.0, 16.0];
    ///
    /// assert_eq!(
    ///     xs.iter().partial_cmp_by(ys, |x, y| x.partial_cmp(&y)),
    ///     Some(Ordering::Less)
    /// );
    /// assert_eq!(
    ///     xs.iter().partial_cmp_by(ys, |x, y| (x * x).partial_cmp(&y)),
    ///     Some(Ordering::Equal)
    /// );
    /// assert_eq!(
    ///     xs.iter().partial_cmp_by(ys, |x, y| (2.0 * x).partial_cmp(&y)),
    ///     Some(Ordering::Greater)
    /// );
    /// ```
    #[unstable(feature = "iter_order_by", issue = "64295")]
    fn partial_cmp_by<I, F>(self, other: I, partial_cmp: F) -> Option<Ordering>
    where
        Self: Sized,
        I: IntoIterator,
        F: FnMut(Self::Item, I::Item) -> Option<Ordering>,
    {
        #[inline]
        fn compare<X, Y, F>(mut partial_cmp: F) -> impl FnMut(X, Y) -> ControlFlow<Option<Ordering>>
        where
            F: FnMut(X, Y) -> Option<Ordering>,
        {
            move |x, y| match partial_cmp(x, y) {
                Some(Ordering::Equal) => ControlFlow::Continue(()),
                non_eq => ControlFlow::Break(non_eq),
            }
        }

        match iter_compare(self, other.into_iter(), compare(partial_cmp)) {
            ControlFlow::Continue(ord) => Some(ord),
            ControlFlow::Break(ord) => ord,
        }
    }

    /// 判断此 [`Iterator`] 的元素是否等于另一 iterator 的元素。
    ///
    /// # 示例
    ///
    /// ```
    /// assert_eq!([1].iter().eq([1].iter()), true);
    /// assert_eq!([1].iter().eq([1, 2].iter()), false);
    /// ```
    #[stable(feature = "iter_order", since = "1.5.0")]
    fn eq<I>(self, other: I) -> bool
    where
        I: IntoIterator,
        Self::Item: PartialEq<I::Item>,
        Self: Sized,
    {
        self.eq_by(other, |x, y| x == y)
    }

    /// 使用指定相等性函数，判断此 [`Iterator`] 的元素是否等于另一 iterator 的元素。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(iter_order_by)]
    ///
    /// let xs = [1, 2, 3, 4];
    /// let ys = [1, 4, 9, 16];
    ///
    /// assert!(xs.iter().eq_by(ys, |x, y| x * x == y));
    /// ```
    #[unstable(feature = "iter_order_by", issue = "64295")]
    fn eq_by<I, F>(self, other: I, eq: F) -> bool
    where
        Self: Sized,
        I: IntoIterator,
        F: FnMut(Self::Item, I::Item) -> bool,
    {
        #[inline]
        fn compare<X, Y, F>(mut eq: F) -> impl FnMut(X, Y) -> ControlFlow<()>
        where
            F: FnMut(X, Y) -> bool,
        {
            move |x, y| {
                if eq(x, y) { ControlFlow::Continue(()) } else { ControlFlow::Break(()) }
            }
        }

        SpecIterEq::spec_iter_eq(self, other.into_iter(), compare(eq))
    }

    /// 判断此 [`Iterator`] 的元素是否不等于另一 iterator 的元素。
    ///
    /// # 示例
    ///
    /// ```
    /// assert_eq!([1].iter().ne([1].iter()), false);
    /// assert_eq!([1].iter().ne([1, 2].iter()), true);
    /// ```
    #[stable(feature = "iter_order", since = "1.5.0")]
    fn ne<I>(self, other: I) -> bool
    where
        I: IntoIterator,
        Self::Item: PartialEq<I::Item>,
        Self: Sized,
    {
        !self.eq(other)
    }

    /// 判断此 [`Iterator`] 的元素是否按[字典序](Ord#lexicographical-comparison)小于另一
    /// iterator 的元素。
    ///
    /// # 示例
    ///
    /// ```
    /// assert_eq!([1].iter().lt([1].iter()), false);
    /// assert_eq!([1].iter().lt([1, 2].iter()), true);
    /// assert_eq!([1, 2].iter().lt([1].iter()), false);
    /// assert_eq!([1, 2].iter().lt([1, 2].iter()), false);
    /// ```
    #[stable(feature = "iter_order", since = "1.5.0")]
    fn lt<I>(self, other: I) -> bool
    where
        I: IntoIterator,
        Self::Item: PartialOrd<I::Item>,
        Self: Sized,
    {
        self.partial_cmp(other) == Some(Ordering::Less)
    }

    /// 判断此 [`Iterator`] 的元素是否按[字典序](Ord#lexicographical-comparison)小于或
    /// 等于另一 iterator 的元素。
    ///
    /// # 示例
    ///
    /// ```
    /// assert_eq!([1].iter().le([1].iter()), true);
    /// assert_eq!([1].iter().le([1, 2].iter()), true);
    /// assert_eq!([1, 2].iter().le([1].iter()), false);
    /// assert_eq!([1, 2].iter().le([1, 2].iter()), true);
    /// ```
    #[stable(feature = "iter_order", since = "1.5.0")]
    fn le<I>(self, other: I) -> bool
    where
        I: IntoIterator,
        Self::Item: PartialOrd<I::Item>,
        Self: Sized,
    {
        matches!(self.partial_cmp(other), Some(Ordering::Less | Ordering::Equal))
    }

    /// 判断此 [`Iterator`] 的元素是否按[字典序](Ord#lexicographical-comparison)大于另一
    /// iterator 的元素。
    ///
    /// # 示例
    ///
    /// ```
    /// assert_eq!([1].iter().gt([1].iter()), false);
    /// assert_eq!([1].iter().gt([1, 2].iter()), false);
    /// assert_eq!([1, 2].iter().gt([1].iter()), true);
    /// assert_eq!([1, 2].iter().gt([1, 2].iter()), false);
    /// ```
    #[stable(feature = "iter_order", since = "1.5.0")]
    fn gt<I>(self, other: I) -> bool
    where
        I: IntoIterator,
        Self::Item: PartialOrd<I::Item>,
        Self: Sized,
    {
        self.partial_cmp(other) == Some(Ordering::Greater)
    }

    /// 判断此 [`Iterator`] 的元素是否按[字典序](Ord#lexicographical-comparison)大于或
    /// 等于另一 iterator 的元素。
    ///
    /// # 示例
    ///
    /// ```
    /// assert_eq!([1].iter().ge([1].iter()), true);
    /// assert_eq!([1].iter().ge([1, 2].iter()), false);
    /// assert_eq!([1, 2].iter().ge([1].iter()), true);
    /// assert_eq!([1, 2].iter().ge([1, 2].iter()), true);
    /// ```
    #[stable(feature = "iter_order", since = "1.5.0")]
    fn ge<I>(self, other: I) -> bool
    where
        I: IntoIterator,
        Self::Item: PartialOrd<I::Item>,
        Self: Sized,
    {
        matches!(self.partial_cmp(other), Some(Ordering::Greater | Ordering::Equal))
    }

    /// 检查此 iterator 的元素是否已排序。
    ///
    /// 也就是说，对每个元素 `a` 及其后继元素 `b`，都必须满足 `a <= b`。如果 iterator
    /// 恰好产出零个或一个元素，则返回 `true`。
    ///
    /// 注意，如果 `Self::Item` 只有 `PartialOrd` 而没有 `Ord`，上述定义意味着只要任意
    /// 两个连续项不可比较，该函数就会返回 `false`。
    ///
    /// # 示例
    ///
    /// ```
    /// assert!([1, 2, 2, 9].iter().is_sorted());
    /// assert!(![1, 3, 2, 4].iter().is_sorted());
    /// assert!([0].iter().is_sorted());
    /// assert!(std::iter::empty::<i32>().is_sorted());
    /// assert!(![0.0, 1.0, f32::NAN].iter().is_sorted());
    /// ```
    #[inline]
    #[stable(feature = "is_sorted", since = "1.82.0")]
    fn is_sorted(self) -> bool
    where
        Self: Sized,
        Self::Item: PartialOrd,
    {
        self.is_sorted_by(|a, b| a <= b)
    }

    /// 使用给定比较函数检查此 iterator 的元素是否已排序。
    ///
    /// 该函数不使用 `PartialOrd::partial_cmp`，而是使用给定的 `compare` 函数判断两个元素
    /// 是否应被视为已按顺序排列。
    ///
    /// # 示例
    ///
    /// ```
    /// assert!([1, 2, 2, 9].iter().is_sorted_by(|a, b| a <= b));
    /// assert!(![1, 2, 2, 9].iter().is_sorted_by(|a, b| a < b));
    ///
    /// assert!([0].iter().is_sorted_by(|a, b| true));
    /// assert!([0].iter().is_sorted_by(|a, b| false));
    ///
    /// assert!(std::iter::empty::<i32>().is_sorted_by(|a, b| false));
    /// assert!(std::iter::empty::<i32>().is_sorted_by(|a, b| true));
    /// ```
    #[stable(feature = "is_sorted", since = "1.82.0")]
    fn is_sorted_by<F>(mut self, compare: F) -> bool
    where
        Self: Sized,
        F: FnMut(&Self::Item, &Self::Item) -> bool,
    {
        #[inline]
        fn check<'a, T>(
            last: &'a mut T,
            mut compare: impl FnMut(&T, &T) -> bool + 'a,
        ) -> impl FnMut(T) -> bool + 'a {
            move |curr| {
                if !compare(&last, &curr) {
                    return false;
                }
                *last = curr;
                true
            }
        }

        let mut last = match self.next() {
            Some(e) => e,
            None => return true,
        };

        self.all(check(&mut last, compare))
    }

    /// 使用给定键提取函数检查此 iterator 的元素是否已排序。
    ///
    /// 该函数不直接比较 iterator 的元素，而是比较由 `f` 决定的元素键。除此之外，它
    /// 等价于 [`is_sorted`]；更多信息见其文档。
    ///
    /// [`is_sorted`]: Iterator::is_sorted
    ///
    /// # 示例
    ///
    /// ```
    /// assert!(["c", "bb", "aaa"].iter().is_sorted_by_key(|s| s.len()));
    /// assert!(![-2i32, -1, 0, 3].iter().is_sorted_by_key(|n| n.abs()));
    /// ```
    #[inline]
    #[stable(feature = "is_sorted", since = "1.82.0")]
    fn is_sorted_by_key<F, K>(self, f: F) -> bool
    where
        Self: Sized,
        F: FnMut(Self::Item) -> K,
        K: PartialOrd,
    {
        self.map(f).is_sorted()
    }

    /// 见 [TrustedRandomAccess][super::super::TrustedRandomAccess]
    // 这个不寻常的名字用于避免方法解析中的名称冲突，见 #76479。
    #[inline]
    #[doc(hidden)]
    #[unstable(feature = "trusted_random_access", issue = "none")]
    unsafe fn __iterator_get_unchecked(&mut self, _idx: usize) -> Self::Item
    where
        Self: TrustedRandomAccessNoCoerce,
    {
        unreachable!("Always specialized");
    }
}

trait SpecIterEq<B: Iterator>: Iterator {
    fn spec_iter_eq<F>(self, b: B, f: F) -> bool
    where
        F: FnMut(Self::Item, <B as Iterator>::Item) -> ControlFlow<()>;
}

impl<A: Iterator, B: Iterator> SpecIterEq<B> for A {
    #[inline]
    default fn spec_iter_eq<F>(self, b: B, f: F) -> bool
    where
        F: FnMut(Self::Item, <B as Iterator>::Item) -> ControlFlow<()>,
    {
        iter_eq(self, b, f)
    }
}

impl<A: Iterator + TrustedLen, B: Iterator + TrustedLen> SpecIterEq<B> for A {
    #[inline]
    fn spec_iter_eq<F>(self, b: B, f: F) -> bool
    where
        F: FnMut(Self::Item, <B as Iterator>::Item) -> ControlFlow<()>,
    {
        // 在以下情况下*不能*短路:
        match (self.size_hint(), b.size_hint()) {
            // ... 两个 iterator 长度相同
            ((_, Some(a)), (_, Some(b))) if a == b => {}
            // ... 或者二者都长于 `usize::MAX`（即长度未知）。
            ((_, None), (_, None)) => {}
            // 否则，无需实际比较元素即可确定二者不相等
            _ => return false,
        }

        iter_eq(self, b, f)
    }
}

/// 使用给定函数逐元素比较两个 iterator。
///
/// 如果函数返回 `ControlFlow::Continue(())`，比较会继续推进到两个 iterator 的下一组
/// 元素。返回 `ControlFlow::Break(x)` 会短路迭代，并返回 `ControlFlow::Break(x)`。
/// 如果其中一个 iterator 耗尽，则返回 `ControlFlow::Continue(ord)`，其中 `ord` 是
/// 比较两个 iterator 长度得到的结果。
///
/// 该函数隔离出 ['cmp_by'](Iterator::cmp_by)、['partial_cmp_by'](Iterator::partial_cmp_by)
/// 和 ['eq_by'](Iterator::eq_by) 共享的逻辑。
#[inline]
fn iter_compare<A, B, F, T>(mut a: A, mut b: B, f: F) -> ControlFlow<T, Ordering>
where
    A: Iterator,
    B: Iterator,
    F: FnMut(A::Item, B::Item) -> ControlFlow<T>,
{
    #[inline]
    fn compare<'a, B, X, T>(
        b: &'a mut B,
        mut f: impl FnMut(X, B::Item) -> ControlFlow<T> + 'a,
    ) -> impl FnMut(X) -> ControlFlow<ControlFlow<T, Ordering>> + 'a
    where
        B: Iterator,
    {
        move |x| match b.next() {
            None => ControlFlow::Break(ControlFlow::Continue(Ordering::Greater)),
            Some(y) => f(x, y).map_break(ControlFlow::Break),
        }
    }

    match a.try_for_each(compare(&mut b, f)) {
        ControlFlow::Continue(()) => ControlFlow::Continue(match b.next() {
            None => Ordering::Equal,
            Some(_) => Ordering::Less,
        }),
        ControlFlow::Break(x) => x,
    }
}

#[inline]
fn iter_eq<A, B, F>(a: A, b: B, f: F) -> bool
where
    A: Iterator,
    B: Iterator,
    F: FnMut(A::Item, B::Item) -> ControlFlow<()>,
{
    iter_compare(a, b, f).continue_value().is_some_and(|ord| ord == Ordering::Equal)
}

/// 为 iterator 的可变引用实现 `Iterator`，例如 [`Iterator::by_ref`] 产生的引用。
///
/// 该实现会把所有方法调用转发给原 iterator。
#[stable(feature = "rust1", since = "1.0.0")]
impl<I: Iterator + ?Sized> Iterator for &mut I {
    type Item = I::Item;
    #[inline]
    fn next(&mut self) -> Option<I::Item> {
        (**self).next()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        (**self).size_hint()
    }
    fn advance_by(&mut self, n: usize) -> Result<(), NonZero<usize>> {
        (**self).advance_by(n)
    }
    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        (**self).nth(n)
    }
    fn fold<B, F>(self, init: B, f: F) -> B
    where
        F: FnMut(B, Self::Item) -> B,
    {
        self.spec_fold(init, f)
    }
    fn try_fold<B, F, R>(&mut self, init: B, f: F) -> R
    where
        F: FnMut(B, Self::Item) -> R,
        R: Try<Output = B>,
    {
        self.spec_try_fold(init, f)
    }
}

/// 用于为 `&mut I where I: Sized` 特化 `fold` 和 `try_fold` 的辅助 trait。
trait IteratorRefSpec: Iterator {
    fn spec_fold<B, F>(self, init: B, f: F) -> B
    where
        F: FnMut(B, Self::Item) -> B;

    fn spec_try_fold<B, F, R>(&mut self, init: B, f: F) -> R
    where
        F: FnMut(B, Self::Item) -> R,
        R: Try<Output = B>;
}

impl<I: Iterator + ?Sized> IteratorRefSpec for &mut I {
    default fn spec_fold<B, F>(self, init: B, mut f: F) -> B
    where
        F: FnMut(B, Self::Item) -> B,
    {
        let mut accum = init;
        while let Some(x) = self.next() {
            accum = f(accum, x);
        }
        accum
    }

    default fn spec_try_fold<B, F, R>(&mut self, init: B, mut f: F) -> R
    where
        F: FnMut(B, Self::Item) -> R,
        R: Try<Output = B>,
    {
        let mut accum = init;
        while let Some(x) = self.next() {
            accum = f(accum, x)?;
        }
        try { accum }
    }
}

impl<I: Iterator> IteratorRefSpec for &mut I {
    impl_fold_via_try_fold! { spec_fold -> spec_try_fold }

    fn spec_try_fold<B, F, R>(&mut self, init: B, f: F) -> R
    where
        F: FnMut(B, Self::Item) -> R,
        R: Try<Output = B>,
    {
        (**self).try_fold(init, f)
    }
}
