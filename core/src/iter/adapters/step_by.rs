use crate::intrinsics;
use crate::iter::{TrustedLen, TrustedRandomAccess, from_fn};
use crate::num::NonZero;
use crate::ops::{Range, Try};

/// 按自定义步长跳步遍历的迭代器。
///
/// 该 `struct` 由 [`Iterator`] 上的 [`step_by`] 方法创建。更多公开语义见该方法文档。
///
/// [`step_by`]: Iterator::step_by
/// [`Iterator`]: trait.Iterator.html
#[must_use = "iterators are lazy and do nothing unless consumed"]
#[stable(feature = "iterator_step_by", since = "1.28.0")]
#[derive(Clone, Debug)]
pub struct StepBy<I> {
    /// 构造函数会保证该字段已经经过 specialized `SpecRangeSetup::setup` 预处理。
    /// 对大多数迭代器，该处理是无操作；但对 Range<{integer}> 类型，该处理会丢失
    /// 部分原始信息，因此不能再把内部迭代器返回给用户代码。这个依赖类型的预处理
    /// 也意味着不同 specialized 实现不能混用。
    iter: I,
    /// 该字段是 `step - 1`，也就是迭代时传给 `nth` 的正确数量。
    /// 它绝不能是 `usize::MAX`，因为 unsafe 代码依赖“加一不会溢出”这一事实。
    /// 例如，这让长度计算不需要额外检查除零风险。
    step_minus_one: usize,
    first_take: bool,
}

impl<I> StepBy<I> {
    #[inline]
    pub(in crate::iter) fn new(iter: I, step: usize) -> StepBy<I> {
        assert!(step != 0);
        let iter = <I as SpecRangeSetup<I>>::setup(iter, step);
        StepBy { iter, step_minus_one: step - 1, first_take: true }
    }

    /// 最初传给 `Iterator::step_by(step)` 的 `step`，也就是 `self.step_minus_one + 1`。
    #[inline]
    fn original_step(&self) -> NonZero<usize> {
        // SAFETY: 根据类型不变量，`step_minus_one` 不可能是 `MAX`，因此加法不会溢出，
        // 且结果不可能为零。
        unsafe { NonZero::new_unchecked(intrinsics::unchecked_add(self.step_minus_one, 1)) }
    }
}

#[stable(feature = "iterator_step_by", since = "1.28.0")]
impl<I> Iterator for StepBy<I>
where
    I: Iterator,
{
    type Item = I::Item;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        self.spec_next()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.spec_size_hint()
    }

    #[inline]
    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        self.spec_nth(n)
    }

    fn try_fold<Acc, F, R>(&mut self, acc: Acc, f: F) -> R
    where
        F: FnMut(Acc, Self::Item) -> R,
        R: Try<Output = Acc>,
    {
        self.spec_try_fold(acc, f)
    }

    #[inline]
    fn fold<Acc, F>(self, acc: Acc, f: F) -> Acc
    where
        F: FnMut(Acc, Self::Item) -> Acc,
    {
        self.spec_fold(acc, f)
    }
}

impl<I> StepBy<I>
where
    I: ExactSizeIterator,
{
    // 最后一个元素相对于迭代器末端的从零开始索引。供 `DoubleEndedIterator` 实现使用。
    fn next_back_index(&self) -> usize {
        let rem = self.iter.len() % self.original_step();
        if self.first_take { if rem == 0 { self.step_minus_one } else { rem - 1 } } else { rem }
    }
}

#[stable(feature = "double_ended_step_by_iterator", since = "1.38.0")]
impl<I> DoubleEndedIterator for StepBy<I>
where
    I: DoubleEndedIterator + ExactSizeIterator,
{
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        self.spec_next_back()
    }

    #[inline]
    fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
        self.spec_nth_back(n)
    }

    fn try_rfold<Acc, F, R>(&mut self, init: Acc, f: F) -> R
    where
        F: FnMut(Acc, Self::Item) -> R,
        R: Try<Output = Acc>,
    {
        self.spec_try_rfold(init, f)
    }

    #[inline]
    fn rfold<Acc, F>(self, init: Acc, f: F) -> Acc
    where
        Self: Sized,
        F: FnMut(Acc, Self::Item) -> Acc,
    {
        self.spec_rfold(init, f)
    }
}

// StepBy 只会让迭代器变短，因此 len 仍能放入 usize。
#[stable(feature = "iterator_step_by", since = "1.28.0")]
impl<I> ExactSizeIterator for StepBy<I> where I: ExactSizeIterator {}

// SAFETY: 该适配器会缩短迭代器。`TrustedLen` 要求上界计算正确；只有当内层迭代器的
// 上界永远不是 `None` 时，才能满足这个要求。`I: TrustedRandomAccess` 恰好提供该
// 保证，而单独的 `I: TrustedLen` 不提供。Range 特化也被覆盖，因为这些 range 同样
// 实现 TRA。
#[unstable(feature = "trusted_len", issue = "37572")]
unsafe impl<I> TrustedLen for StepBy<I> where I: Iterator + TrustedRandomAccess {}

trait SpecRangeSetup<T> {
    fn setup(inner: T, step: usize) -> T;
}

impl<T> SpecRangeSetup<T> for T {
    #[inline]
    default fn setup(inner: T, _step: usize) -> T {
        inner
    }
}

/// 用于优化 `StepBy<Range<{integer}>>` 迭代的 specialization trait。
///
/// # 安全性(Safety）
///
/// 从语法上看，实现该 trait 不一定需要写 unsafe 操作，但实际有大量 unsafe 代码依赖
/// 整数 range 的长度和步进计算正确。
///
/// 为了保持正确性，*所有* public `StepBy` 方法都必须一起特化。原因是 `setup` 会显著
/// 改变结构体字段的含义；如果前向、长度或折叠方法混用不同实现，就会得到错误结果。
unsafe trait StepByImpl<I> {
    type Item;

    fn spec_next(&mut self) -> Option<Self::Item>;

    fn spec_size_hint(&self) -> (usize, Option<usize>);

    fn spec_nth(&mut self, n: usize) -> Option<Self::Item>;

    fn spec_try_fold<Acc, F, R>(&mut self, acc: Acc, f: F) -> R
    where
        F: FnMut(Acc, Self::Item) -> R,
        R: Try<Output = Acc>;

    fn spec_fold<Acc, F>(self, acc: Acc, f: F) -> Acc
    where
        F: FnMut(Acc, Self::Item) -> Acc;
}

/// 用于双端迭代的 specialization trait。
///
/// 另见: `StepByImpl`
///
/// # 安全性(Safety）
///
/// 在适用时，这些特化必须与 `StepByImpl` 一起实现。也就是说，如果某个迭代器上的
/// `StepBy` 支持反向迭代，并且已经为前向迭代特化，那么它也必须为反向迭代特化，
/// 否则前后端会对字段含义作出不同解释。
unsafe trait StepByBackImpl<I> {
    type Item;

    fn spec_next_back(&mut self) -> Option<Self::Item>
    where
        I: DoubleEndedIterator + ExactSizeIterator;

    fn spec_nth_back(&mut self, n: usize) -> Option<Self::Item>
    where
        I: DoubleEndedIterator + ExactSizeIterator;

    fn spec_try_rfold<Acc, F, R>(&mut self, init: Acc, f: F) -> R
    where
        I: DoubleEndedIterator + ExactSizeIterator,
        F: FnMut(Acc, Self::Item) -> R,
        R: Try<Output = Acc>;

    fn spec_rfold<Acc, F>(self, init: Acc, f: F) -> Acc
    where
        I: DoubleEndedIterator + ExactSizeIterator,
        F: FnMut(Acc, Self::Item) -> Acc;
}

unsafe impl<I: Iterator> StepByImpl<I> for StepBy<I> {
    type Item = I::Item;

    #[inline]
    default fn spec_next(&mut self) -> Option<I::Item> {
        let step_size = if self.first_take { 0 } else { self.step_minus_one };
        self.first_take = false;
        self.iter.nth(step_size)
    }

    #[inline]
    default fn spec_size_hint(&self) -> (usize, Option<usize>) {
        #[inline]
        fn first_size(step: NonZero<usize>) -> impl Fn(usize) -> usize {
            move |n| if n == 0 { 0 } else { 1 + (n - 1) / step }
        }

        #[inline]
        fn other_size(step: NonZero<usize>) -> impl Fn(usize) -> usize {
            move |n| n / step
        }

        let (low, high) = self.iter.size_hint();

        if self.first_take {
            let f = first_size(self.original_step());
            (f(low), high.map(f))
        } else {
            let f = other_size(self.original_step());
            (f(low), high.map(f))
        }
    }

    #[inline]
    default fn spec_nth(&mut self, mut n: usize) -> Option<I::Item> {
        if self.first_take {
            self.first_take = false;
            let first = self.iter.next();
            if n == 0 {
                return first;
            }
            n -= 1;
        }
        // n 和 self.step_minus_one 都是下标，要加 1 才能得到元素个数
        // 调用 `.nth` 时又要减回 1 才能换算回下标
        let mut step = self.original_step().get();
        // n + 1 可能溢出
        // 因此当 n 为 usize::MAX 时，不再加一，而是直接调用 .nth(step)
        if n == usize::MAX {
            self.iter.nth(step - 1);
        } else {
            n += 1;
        }

        // overflow handling
        loop {
            let mul = n.checked_mul(step);
            {
                if intrinsics::likely(mul.is_some()) {
                    return self.iter.nth(mul.unwrap() - 1);
                }
            }
            let div_n = usize::MAX / n;
            let div_step = usize::MAX / step;
            let nth_n = div_n * n;
            let nth_step = div_step * step;
            let nth = if nth_n > nth_step {
                step -= div_n;
                nth_n
            } else {
                n -= div_step;
                nth_step
            };
            self.iter.nth(nth - 1);
        }
    }

    default fn spec_try_fold<Acc, F, R>(&mut self, mut acc: Acc, mut f: F) -> R
    where
        F: FnMut(Acc, Self::Item) -> R,
        R: Try<Output = Acc>,
    {
        #[inline]
        fn nth<I: Iterator>(
            iter: &mut I,
            step_minus_one: usize,
        ) -> impl FnMut() -> Option<I::Item> + '_ {
            move || iter.nth(step_minus_one)
        }

        if self.first_take {
            self.first_take = false;
            match self.iter.next() {
                None => return try { acc },
                Some(x) => acc = f(acc, x)?,
            }
        }
        from_fn(nth(&mut self.iter, self.step_minus_one)).try_fold(acc, f)
    }

    default fn spec_fold<Acc, F>(mut self, mut acc: Acc, mut f: F) -> Acc
    where
        F: FnMut(Acc, Self::Item) -> Acc,
    {
        #[inline]
        fn nth<I: Iterator>(
            iter: &mut I,
            step_minus_one: usize,
        ) -> impl FnMut() -> Option<I::Item> + '_ {
            move || iter.nth(step_minus_one)
        }

        if self.first_take {
            self.first_take = false;
            match self.iter.next() {
                None => return acc,
                Some(x) => acc = f(acc, x),
            }
        }
        from_fn(nth(&mut self.iter, self.step_minus_one)).fold(acc, f)
    }
}

unsafe impl<I: DoubleEndedIterator + ExactSizeIterator> StepByBackImpl<I> for StepBy<I> {
    type Item = I::Item;

    #[inline]
    default fn spec_next_back(&mut self) -> Option<Self::Item> {
        self.iter.nth_back(self.next_back_index())
    }

    #[inline]
    default fn spec_nth_back(&mut self, n: usize) -> Option<I::Item> {
        // 当 `n` 越界时，`self.iter.nth_back(usize::MAX)` 在这里会做正确的事:
        // `self.iter` 的长度不会超过 `usize::MAX`（因为 `I: ExactSizeIterator`），
        // 且 `nth_back` 使用从零开始的索引。
        let n = n.saturating_mul(self.original_step().get()).saturating_add(self.next_back_index());
        self.iter.nth_back(n)
    }

    default fn spec_try_rfold<Acc, F, R>(&mut self, init: Acc, mut f: F) -> R
    where
        F: FnMut(Acc, Self::Item) -> R,
        R: Try<Output = Acc>,
    {
        #[inline]
        fn nth_back<I: DoubleEndedIterator>(
            iter: &mut I,
            step_minus_one: usize,
        ) -> impl FnMut() -> Option<I::Item> + '_ {
            move || iter.nth_back(step_minus_one)
        }

        match self.next_back() {
            None => try { init },
            Some(x) => {
                let acc = f(init, x)?;
                from_fn(nth_back(&mut self.iter, self.step_minus_one)).try_fold(acc, f)
            }
        }
    }

    #[inline]
    default fn spec_rfold<Acc, F>(mut self, init: Acc, mut f: F) -> Acc
    where
        Self: Sized,
        F: FnMut(Acc, I::Item) -> Acc,
    {
        #[inline]
        fn nth_back<I: DoubleEndedIterator>(
            iter: &mut I,
            step_minus_one: usize,
        ) -> impl FnMut() -> Option<I::Item> + '_ {
            move || iter.nth_back(step_minus_one)
        }

        match self.next_back() {
            None => init,
            Some(x) => {
                let acc = f(init, x);
                from_fn(nth_back(&mut self.iter, self.step_minus_one)).fold(acc, f)
            }
        }
    }
}

/// 对这些实现来说，`SpecRangeSetup` 会计算所需迭代次数，并把它存入 `iter.end`。
///
/// 随后的各种 iterator 实现依赖这个值，从而不需要做溢出检查，只需按计数执行循环。
///
/// 这些实现只适用于无符号类型；如果要用它们为有符号类型做 specialization，
/// 就需要重新设计。
///
/// 目前这些实现只覆盖到 `usize` 宽度以内的整数，因为 16 位平台上的
/// `ExactSizeIterator` impl 存在正确性问题。又因为 `ExactSizeIterator` 是反向迭代的
/// 前提，而前向和反向迭代必须保持一致地 specialization，所以情况已经足够复杂，
/// 暂时不覆盖其他类型。
macro_rules! spec_int_ranges {
    ($($t:ty)*) => ($(

        const _: () = assert!(usize::BITS >= <$t>::BITS);

        impl SpecRangeSetup<Range<$t>> for Range<$t> {
            #[inline]
            fn setup(mut r: Range<$t>, step: usize) -> Range<$t> {
                let inner_len = r.size_hint().0;
                // 如果 step 超过 $t::MAX，则计数最多为 1，因此总能放入 $t。
                let yield_count = inner_len.div_ceil(step);
                // 把 range 的 end 转换成迭代计数器。
                r.end = yield_count as $t;
                r
            }
        }

        unsafe impl StepByImpl<Range<$t>> for StepBy<Range<$t>> {
            #[inline]
            fn spec_next(&mut self) -> Option<$t> {
                // 如果指定了大于该类型可表示范围的步长，则退回到 t::MAX；
                // 此时 remaining 最多为 1。
                let step = <$t>::try_from(self.original_step().get()).unwrap_or(<$t>::MAX);
                let remaining = self.iter.end;
                if remaining > 0 {
                    let val = self.iter.start;
                    // 这只可能在最后一步溢出，而之后该值不会再被使用。
                    self.iter.start = val.wrapping_add(step);
                    self.iter.end = remaining - 1;
                    Some(val)
                } else {
                    None
                }
            }

            #[inline]
            fn spec_size_hint(&self) -> (usize, Option<usize>) {
                let remaining = self.iter.end as usize;
                (remaining, Some(remaining))
            }

            // 下面的方法全部复制自 Iterator trait 的默认实现。
            // 必须在这里重复它们，以便 specialization 覆盖 StepByImpl 默认实现。

            #[inline]
            fn spec_nth(&mut self, n: usize) -> Option<Self::Item> {
                self.advance_by(n).ok()?;
                self.next()
            }

            #[inline]
            fn spec_try_fold<Acc, F, R>(&mut self, init: Acc, mut f: F) -> R
                where
                    F: FnMut(Acc, Self::Item) -> R,
                    R: Try<Output = Acc>
            {
                let mut accum = init;
                while let Some(x) = self.next() {
                    accum = f(accum, x)?;
                }
                try { accum }
            }

            #[inline]
            fn spec_fold<Acc, F>(self, init: Acc, mut f: F) -> Acc
                where
                    F: FnMut(Acc, Self::Item) -> Acc
            {
                // 如果指定了大于该类型可表示范围的步长，则退回到 t::MAX；
                // 此时 remaining 最多为 1。
                let step = <$t>::try_from(self.original_step().get()).unwrap_or(<$t>::MAX);
                let remaining = self.iter.end;
                let mut acc = init;
                let mut val = self.iter.start;
                for _ in 0..remaining {
                    acc = f(acc, val);
                    // 这只可能在最后一步溢出，而之后该值不会再被使用。
                    val = val.wrapping_add(step);
                }
                acc
            }
        }
    )*)
}

macro_rules! spec_int_ranges_r {
    ($($t:ty)*) => ($(
        const _: () = assert!(usize::BITS >= <$t>::BITS);

        unsafe impl StepByBackImpl<Range<$t>> for StepBy<Range<$t>> {

            #[inline]
            fn spec_next_back(&mut self) -> Option<Self::Item> {
                let step = self.original_step().get() as $t;
                let remaining = self.iter.end;
                if remaining > 0 {
                    let start = self.iter.start;
                    self.iter.end = remaining - 1;
                    Some(start + step * (remaining - 1))
                } else {
                    None
                }
            }

            // 下面的方法全部复制自 Iterator trait 的默认实现。
            // 必须在这里重复它们，以便 specialization 覆盖 StepByImplBack 默认实现。

            #[inline]
            fn spec_nth_back(&mut self, n: usize) -> Option<Self::Item> {
                if self.advance_back_by(n).is_err() {
                    return None;
                }
                self.next_back()
            }

            #[inline]
            fn spec_try_rfold<Acc, F, R>(&mut self, init: Acc, mut f: F) -> R
            where
                F: FnMut(Acc, Self::Item) -> R,
                R: Try<Output = Acc>
            {
                let mut accum = init;
                while let Some(x) = self.next_back() {
                    accum = f(accum, x)?;
                }
                try { accum }
            }

            #[inline]
            fn spec_rfold<Acc, F>(mut self, init: Acc, mut f: F) -> Acc
            where
                F: FnMut(Acc, Self::Item) -> Acc
            {
                let mut accum = init;
                while let Some(x) = self.next_back() {
                    accum = f(accum, x);
                }
                accum
            }
        }
    )*)
}

#[cfg(target_pointer_width = "64")]
spec_int_ranges!(u8 u16 u32 u64 usize);
// DoubleEndedIterator 要求 ExactSizeIterator，而后者并未为 Range<u64> 实现
#[cfg(target_pointer_width = "64")]
spec_int_ranges_r!(u8 u16 u32 usize);

#[cfg(target_pointer_width = "32")]
spec_int_ranges!(u8 u16 u32 usize);
#[cfg(target_pointer_width = "32")]
spec_int_ranges_r!(u8 u16 u32 usize);

#[cfg(target_pointer_width = "16")]
spec_int_ranges!(u8 u16 usize);
#[cfg(target_pointer_width = "16")]
spec_int_ranges_r!(u8 u16 usize);
