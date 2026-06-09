use crate::iter::adapters::SourceIter;
use crate::iter::{FusedIterator, TrustedLen};
use crate::ops::{ControlFlow, Try};

/// 带有 `peek()` 的迭代器，可以返回下一元素的可选引用。
///
/// 该 `struct` 由 [`Iterator`] 上的 [`peekable`] 方法创建。更多信息见该方法文档。
///
/// [`peekable`]: Iterator::peekable
/// [`Iterator`]: trait.Iterator.html
#[derive(Clone, Debug)]
#[must_use = "iterators are lazy and do nothing unless consumed"]
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_diagnostic_item = "IterPeekable"]
pub struct Peekable<I: Iterator> {
    iter: I,
    /// 记住已经 peek 到的值，即使它是 None。
    peeked: Option<Option<I::Item>>,
}

impl<I: Iterator> Peekable<I> {
    pub(in crate::iter) fn new(iter: I) -> Peekable<I> {
        Peekable { iter, peeked: None }
    }
}

// Peekable 必须记住 `.peek()` 方法是否见过 None。
// 这保证 `.peek(); .peek();` 或 `.peek(); .next();` 最多只推进底层迭代器一次。
// 这个状态本身不会让迭代器变成 fused。
#[stable(feature = "rust1", since = "1.0.0")]
impl<I: Iterator> Iterator for Peekable<I> {
    type Item = I::Item;

    #[inline]
    fn next(&mut self) -> Option<I::Item> {
        match self.peeked.take() {
            Some(v) => v,
            None => self.iter.next(),
        }
    }

    #[inline]
    #[rustc_inherit_overflow_checks]
    fn count(mut self) -> usize {
        match self.peeked.take() {
            Some(None) => 0,
            Some(Some(_)) => 1 + self.iter.count(),
            None => self.iter.count(),
        }
    }

    #[inline]
    fn nth(&mut self, n: usize) -> Option<I::Item> {
        match self.peeked.take() {
            Some(None) => None,
            Some(v @ Some(_)) if n == 0 => v,
            Some(Some(_)) => self.iter.nth(n - 1),
            None => self.iter.nth(n),
        }
    }

    #[inline]
    fn last(mut self) -> Option<I::Item> {
        let peek_opt = match self.peeked.take() {
            Some(None) => return None,
            Some(v) => v,
            None => None,
        };
        self.iter.last().or(peek_opt)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let peek_len = match self.peeked {
            Some(None) => return (0, Some(0)),
            Some(Some(_)) => 1,
            None => 0,
        };
        let (lo, hi) = self.iter.size_hint();
        let lo = lo.saturating_add(peek_len);
        let hi = match hi {
            Some(x) => x.checked_add(peek_len),
            None => None,
        };
        (lo, hi)
    }

    #[inline]
    fn try_fold<B, F, R>(&mut self, init: B, mut f: F) -> R
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> R,
        R: Try<Output = B>,
    {
        let acc = match self.peeked.take() {
            Some(None) => return try { init },
            Some(Some(v)) => f(init, v)?,
            None => init,
        };
        self.iter.try_fold(acc, f)
    }

    #[inline]
    fn fold<Acc, Fold>(self, init: Acc, mut fold: Fold) -> Acc
    where
        Fold: FnMut(Acc, Self::Item) -> Acc,
    {
        let acc = match self.peeked {
            Some(None) => return init,
            Some(Some(v)) => fold(init, v),
            None => init,
        };
        self.iter.fold(acc, fold)
    }
}

#[stable(feature = "double_ended_peek_iterator", since = "1.38.0")]
impl<I> DoubleEndedIterator for Peekable<I>
where
    I: DoubleEndedIterator,
{
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        match self.peeked.as_mut() {
            Some(v @ Some(_)) => self.iter.next_back().or_else(|| v.take()),
            Some(None) => None,
            None => self.iter.next_back(),
        }
    }

    #[inline]
    fn try_rfold<B, F, R>(&mut self, init: B, mut f: F) -> R
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> R,
        R: Try<Output = B>,
    {
        match self.peeked.take() {
            Some(None) => try { init },
            Some(Some(v)) => match self.iter.try_rfold(init, &mut f).branch() {
                ControlFlow::Continue(acc) => f(acc, v),
                ControlFlow::Break(r) => {
                    self.peeked = Some(Some(v));
                    R::from_residual(r)
                }
            },
            None => self.iter.try_rfold(init, f),
        }
    }

    #[inline]
    fn rfold<Acc, Fold>(self, init: Acc, mut fold: Fold) -> Acc
    where
        Fold: FnMut(Acc, Self::Item) -> Acc,
    {
        match self.peeked {
            Some(None) => init,
            Some(Some(v)) => {
                let acc = self.iter.rfold(init, &mut fold);
                fold(acc, v)
            }
            None => self.iter.rfold(init, fold),
        }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<I: ExactSizeIterator> ExactSizeIterator for Peekable<I> {}

#[stable(feature = "fused", since = "1.26.0")]
impl<I: FusedIterator> FusedIterator for Peekable<I> {}

impl<I: Iterator> Peekable<I> {
    /// 返回对 next() 值的引用，但不推进迭代器。
    ///
    /// 和 [`next`] 一样，如果存在值，就把它包在 `Some(T)` 中；如果迭代结束，
    /// 则返回 `None`。
    ///
    /// [`next`]: Iterator::next
    ///
    /// 因为 `peek()` 返回引用，而许多迭代器本身也遍历引用，所以返回值可能出现双重
    /// 引用，看起来有些令人困惑。下面的示例展示了这种效果。
    ///
    /// # 示例
    ///
    /// 基本用法:
    ///
    /// ```
    /// let xs = [1, 2, 3];
    ///
    /// let mut iter = xs.iter().peekable();
    ///
    /// // peek() 让我们能看到下一项。
    /// assert_eq!(iter.peek(), Some(&&1));
    /// assert_eq!(iter.next(), Some(&1));
    ///
    /// assert_eq!(iter.next(), Some(&2));
    ///
    /// // 即使多次 `peek`，迭代器也不会推进。
    /// assert_eq!(iter.peek(), Some(&&3));
    /// assert_eq!(iter.peek(), Some(&&3));
    ///
    /// assert_eq!(iter.next(), Some(&3));
    ///
    /// // 迭代器结束后，`peek()` 也结束。
    /// assert_eq!(iter.peek(), None);
    /// assert_eq!(iter.next(), None);
    /// ```
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn peek(&mut self) -> Option<&I::Item> {
        let iter = &mut self.iter;
        self.peeked.get_or_insert_with(|| iter.next()).as_ref()
    }

    /// 返回对 next() 值的可变引用，但不推进迭代器。
    ///
    /// 和 [`next`] 一样，如果存在值，就把它包在 `Some(T)` 中；如果迭代结束，
    /// 则返回 `None`。
    ///
    /// 因为 `peek_mut()` 返回引用，而许多迭代器本身也遍历引用，所以返回值可能出现
    /// 双重引用。下面的示例展示了这种效果。
    ///
    /// [`next`]: Iterator::next
    ///
    /// # 示例
    ///
    /// 基本用法:
    ///
    /// ```
    /// let mut iter = [1, 2, 3].iter().peekable();
    ///
    /// // 和 `peek()` 一样，可以在不推进迭代器的情况下看到下一项。
    /// assert_eq!(iter.peek_mut(), Some(&mut &1));
    /// assert_eq!(iter.peek_mut(), Some(&mut &1));
    /// assert_eq!(iter.next(), Some(&1));
    ///
    /// // 查看迭代器下一项，并设置可变引用背后的值。
    /// if let Some(p) = iter.peek_mut() {
    ///     assert_eq!(*p, &2);
    ///     *p = &5;
    /// }
    ///
    /// // 放入的值会在迭代继续时重新出现。
    /// assert_eq!(iter.collect::<Vec<_>>(), vec![&5, &3]);
    /// ```
    #[inline]
    #[stable(feature = "peekable_peek_mut", since = "1.53.0")]
    pub fn peek_mut(&mut self) -> Option<&mut I::Item> {
        let iter = &mut self.iter;
        self.peeked.get_or_insert_with(|| iter.next()).as_mut()
    }

    /// 如果条件为 true，则消耗并返回该迭代器的下一个值。
    ///
    /// 如果 `func` 对该迭代器的下一个值返回 `true`，则消耗并返回该值；否则返回 `None`。
    ///
    /// # 示例
    /// 如果数字等于 0，则消耗它。
    /// ```
    /// let mut iter = (0..5).peekable();
    /// // 迭代器第一项是 0；消耗它。
    /// assert_eq!(iter.next_if(|&x| x == 0), Some(0));
    /// // 现在下一项是 1，因此 `next_if` 会返回 `None`。
    /// assert_eq!(iter.next_if(|&x| x == 0), None);
    /// // 如果 predicate 对下一项求值为 `false`，`next_if` 会保留该项。
    /// assert_eq!(iter.next(), Some(1));
    /// ```
    ///
    /// 消耗所有小于 10 的数字。
    /// ```
    /// let mut iter = (1..20).peekable();
    /// // 消耗所有小于 10 的数字。
    /// while iter.next_if(|&x| x < 10).is_some() {}
    /// // 下一个返回值将是 10。
    /// assert_eq!(iter.next(), Some(10));
    /// ```
    #[stable(feature = "peekable_next_if", since = "1.51.0")]
    pub fn next_if(&mut self, func: impl FnOnce(&I::Item) -> bool) -> Option<I::Item> {
        match self.next() {
            Some(matched) if func(&matched) => Some(matched),
            other => {
                // 因为调用了 `self.next()`，所以已经消耗了 `self.peeked`。
                assert!(self.peeked.is_none());
                self.peeked = Some(other);
                None
            }
        }
    }

    /// 如果下一项等于 `expected`，则消耗并返回该项。
    ///
    /// # 示例
    /// 如果数字等于 0，则消耗它。
    /// ```
    /// let mut iter = (0..5).peekable();
    /// // 迭代器第一项是 0；消耗它。
    /// assert_eq!(iter.next_if_eq(&0), Some(0));
    /// // 现在下一项是 1，因此 `next_if_eq` 会返回 `None`。
    /// assert_eq!(iter.next_if_eq(&0), None);
    /// // 如果下一项不等于 `expected`，`next_if_eq` 会保留该项。
    /// assert_eq!(iter.next(), Some(1));
    /// ```
    #[stable(feature = "peekable_next_if", since = "1.51.0")]
    pub fn next_if_eq<T>(&mut self, expected: &T) -> Option<I::Item>
    where
        T: ?Sized,
        I::Item: PartialEq<T>,
    {
        self.next_if(|next| next == expected)
    }

    /// 消耗该迭代器的下一个值并对其应用函数 `f`；如果闭包返回 `Ok`，则返回其中结果。
    ///
    /// 否则，如果闭包返回 `Err`，该值会被放回，供下一次迭代使用。
    ///
    /// `Err` 变体中的内容通常是传给闭包的原始值，但这不是强制要求。如果返回了不同值，
    /// 下一次 `peek()` 或 `next()` 调用将得到这个新值。这类似于修改 `peek_mut()` 的输出。
    ///
    /// 如果闭包 panic，下一个值总会被消耗并 drop；即使 panic 被捕获也是如此，因为闭包
    /// 没有返回可放回的 `Err` 值。
    ///
    /// 另见: [`next_if_map_mut`](Self::next_if_map_mut)。
    ///
    /// # 示例
    ///
    /// 从字符迭代器中解析开头的十进制数。
    /// ```
    /// let mut iter = "125 GOTO 10".chars().peekable();
    /// let mut line_num = 0_u32;
    /// while let Some(digit) = iter.next_if_map(|c| c.to_digit(10).ok_or(c)) {
    ///     line_num = line_num * 10 + digit;
    /// }
    /// assert_eq!(line_num, 125);
    /// assert_eq!(iter.collect::<String>(), " GOTO 10");
    /// ```
    ///
    /// 匹配自定义类型。
    /// ```
    ///
    /// #[derive(Debug, PartialEq, Eq)]
    /// enum Node {
    ///     Comment(String),
    ///     Red(String),
    ///     Green(String),
    ///     Blue(String),
    /// }
    ///
    /// /// 将所有连续的 `Comment` 节点合并成单个节点。
    /// fn combine_comments(nodes: Vec<Node>) -> Vec<Node> {
    ///     let mut result = Vec::with_capacity(nodes.len());
    ///     let mut iter = nodes.into_iter().peekable();
    ///     let mut comment_text = None::<String>;
    ///     loop {
    ///         // .next_if_map() 中的闭包通常会匹配输入，
    ///         //  把期望的模式提取到 `Ok` 中，
    ///         //  并把其余内容放入 `Err`。
    ///         while let Some(text) = iter.next_if_map(|node| match node {
    ///             Node::Comment(text) => Ok(text),
    ///             other => Err(other),
    ///         }) {
    ///             comment_text.get_or_insert_default().push_str(&text);
    ///         }
    ///
    ///         if let Some(text) = comment_text.take() {
    ///             result.push(Node::Comment(text));
    ///         }
    ///         if let Some(node) = iter.next() {
    ///             result.push(node);
    ///         } else {
    ///             break;
    ///         }
    ///     }
    ///     result
    /// }
    ///# assert_eq!( // 隐藏测试，避免文档过于拥挤。
    ///#     combine_comments(vec![
    ///#         Node::Comment("The".to_owned()),
    ///#         Node::Comment("Quick".to_owned()),
    ///#         Node::Comment("Brown".to_owned()),
    ///#         Node::Red("Fox".to_owned()),
    ///#         Node::Green("Jumped".to_owned()),
    ///#         Node::Comment("Over".to_owned()),
    ///#         Node::Blue("The".to_owned()),
    ///#         Node::Comment("Lazy".to_owned()),
    ///#         Node::Comment("Dog".to_owned()),
    ///#     ]),
    ///#     vec![
    ///#         Node::Comment("TheQuickBrown".to_owned()),
    ///#         Node::Red("Fox".to_owned()),
    ///#         Node::Green("Jumped".to_owned()),
    ///#         Node::Comment("Over".to_owned()),
    ///#         Node::Blue("The".to_owned()),
    ///#         Node::Comment("LazyDog".to_owned()),
    ///#     ],
    ///# )
    /// ```
    #[stable(feature = "peekable_next_if_map", since = "1.94.0")]
    pub fn next_if_map<R>(&mut self, f: impl FnOnce(I::Item) -> Result<R, I::Item>) -> Option<R> {
        let unpeek = if let Some(item) = self.next() {
            match f(item) {
                Ok(result) => return Some(result),
                Err(item) => Some(item),
            }
        } else {
            None
        };
        self.peeked = Some(unpeek);
        None
    }

    /// 给出迭代器下一个值的可变引用，并对其应用函数 `f`；如果 `f` 返回 `Some`，
    /// 则返回该结果并推进迭代器。
    ///
    /// 否则，如果 `f` 返回 `None`，下一个值会保留到下一次迭代。
    ///
    /// 如果 `f` panic，从迭代器中取出的项会像 `f` 返回了 `Some` 一样被消耗。
    /// 该值随后会被 drop。
    ///
    /// 这类似于 [`next_if_map`](Self::next_if_map)，区别是不会把项的所有权交给 `f`。
    /// 如果 `f` 本来就会复制该项，这种方式可能更合适。
    ///
    /// # 示例
    ///
    /// 从字符迭代器中解析开头的十进制数。
    /// ```
    /// let mut iter = "125 GOTO 10".chars().peekable();
    /// let mut line_num = 0_u32;
    /// while let Some(digit) = iter.next_if_map_mut(|c| c.to_digit(10)) {
    ///     line_num = line_num * 10 + digit;
    /// }
    /// assert_eq!(line_num, 125);
    /// assert_eq!(iter.collect::<String>(), " GOTO 10");
    /// ```
    #[stable(feature = "peekable_next_if_map", since = "1.94.0")]
    pub fn next_if_map_mut<R>(&mut self, f: impl FnOnce(&mut I::Item) -> Option<R>) -> Option<R> {
        let unpeek = if let Some(mut item) = self.next() {
            match f(&mut item) {
                Some(result) => return Some(result),
                None => Some(item),
            }
        } else {
            None
        };
        self.peeked = Some(unpeek);
        None
    }
}

#[unstable(feature = "trusted_len", issue = "37572")]
unsafe impl<I> TrustedLen for Peekable<I> where I: TrustedLen {}

#[unstable(issue = "none", feature = "inplace_iteration")]
unsafe impl<I: Iterator> SourceIter for Peekable<I>
where
    I: SourceIter,
{
    type Source = I::Source;

    #[inline]
    unsafe fn as_inner(&mut self) -> &mut I::Source {
        // SAFETY: 转发到具有相同要求的 unsafe 函数。
        unsafe { SourceIter::as_inner(&mut self.iter) }
    }
}
