//! 定义数组的拥有型迭代器 `IntoIter` 的内部实现。

use crate::mem::MaybeUninit;
use crate::num::NonZero;
use crate::ops::{IndexRange, NeverShortCircuit, Try};
use crate::{fmt, iter};

#[allow(private_bounds)]
trait PartialDrop {
    /// # 安全性(Safety）
    ///
    /// 调用前 `self[alive]` 中的元素必须全部已初始化;调用后这些元素在重新初始化前
    /// 不得再被使用。
    unsafe fn partial_drop(&mut self, alive: IndexRange);
}
impl<T> PartialDrop for [MaybeUninit<T>] {
    unsafe fn partial_drop(&mut self, alive: IndexRange) {
        // SAFETY: 调用方保证 `alive` 范围内的所有元素都已正确初始化。
        unsafe { self.get_unchecked_mut(alive).assume_init_drop() }
    }
}
impl<T, const N: usize> PartialDrop for [MaybeUninit<T>; N] {
    unsafe fn partial_drop(&mut self, alive: IndexRange) {
        let slice: &mut [MaybeUninit<T>] = self;
        // SAFETY: 数组中已初始化的元素在切片视图中同样已初始化。
        unsafe { slice.partial_drop(alive) }
    }
}

/// 按值数组迭代器的内部状态。
///
/// 真正的 `array::IntoIter<T, N>` 存储的是 `PolymorphicIter<[MaybeUninit<T>, N]>`,
/// 迭代时再将其 unsize 为 `PolymorphicIter<[MaybeUninit<T>]>`。
#[allow(private_bounds)]
pub(super) struct PolymorphicIter<DATA: ?Sized>
where
    DATA: PartialDrop,
{
    /// `data` 中尚未产出的元素范围。
    ///
    /// 不变量:
    /// - `alive.end <= N`
    ///
    /// (并且 `IndexRange` 类型本身要求 `alive.start <= alive.end`。)
    alive: IndexRange,

    /// 正在被迭代的数组。
    ///
    /// 满足 `alive.start <= i < alive.end` 的元素尚未产出,因此仍是有效数组项。
    /// 索引 `i < alive.start` 或 `i >= alive.end` 的元素已经产出,不得再访问!
    /// 这些“死亡”元素甚至可能处于完全未初始化状态。
    ///
    /// 因而不变量是:
    /// - `data[alive]` 是活跃区(即包含有效元素)
    /// - `data[..alive.start]` 和 `data[alive.end..]` 是死亡区(元素已经被读取,
    ///   不得再触碰!)
    data: DATA,
}

#[allow(private_bounds)]
impl<DATA: ?Sized> PolymorphicIter<DATA>
where
    DATA: PartialDrop,
{
    #[inline]
    pub(super) const fn len(&self) -> usize {
        self.alive.len()
    }
}

#[allow(private_bounds)]
impl<DATA: ?Sized> Drop for PolymorphicIter<DATA>
where
    DATA: PartialDrop,
{
    #[inline]
    fn drop(&mut self) {
        // SAFETY: 根据类型不变量,`self.alive` 精确覆盖仍初始化的元素;这里处于 Drop,
        // 之后不会再有代码使用这些元素。
        unsafe { self.data.partial_drop(self.alive.clone()) }
    }
}

impl<T, const N: usize> PolymorphicIter<[MaybeUninit<T>; N]> {
    #[inline]
    pub(super) const fn empty() -> Self {
        Self { alive: IndexRange::zero_to(0), data: [const { MaybeUninit::uninit() }; N] }
    }

    /// # 安全性(Safety）
    ///
    /// `data[alive]` 中的元素必须全部已初始化。
    #[inline]
    pub(super) const unsafe fn new_unchecked(alive: IndexRange, data: [MaybeUninit<T>; N]) -> Self {
        Self { alive, data }
    }
}

impl<T: Clone, const N: usize> Clone for PolymorphicIter<[MaybeUninit<T>; N]> {
    #[inline]
    fn clone(&self) -> Self {
        // 注意,克隆结果不需要保持完全相同的 alive 范围;无论 `self` 当前位于何处,
        // 都可以从偏移 0 开始克隆。
        let mut new = Self::empty();

        fn clone_into_new<U: Clone>(
            source: &PolymorphicIter<[MaybeUninit<U>]>,
            target: &mut PolymorphicIter<[MaybeUninit<U>]>,
        ) {
            // 克隆所有活跃元素。
            for (src, dst) in iter::zip(source.as_slice(), &mut target.data) {
                // 将克隆值写入新数组,随后更新其 alive 范围。
                // 若克隆过程 panic,此前写入的元素会被正确 drop。
                dst.write(src.clone());
                // 这里的加法不会溢出,因为我们正在迭代切片,而切片长度一定能放入 usize。
                target.alive = IndexRange::zero_to(target.alive.end() + 1);
            }
        }

        clone_into_new(self, &mut new);
        new
    }
}

impl<T> PolymorphicIter<[MaybeUninit<T>]> {
    #[inline]
    pub(super) fn as_slice(&self) -> &[T] {
        // SAFETY: 根据类型不变量,`alive` 范围内的所有元素都已正确初始化。
        unsafe {
            let slice = self.data.get_unchecked(self.alive.clone());
            slice.assume_init_ref()
        }
    }

    #[inline]
    pub(super) fn as_mut_slice(&mut self) -> &mut [T] {
        // SAFETY: 根据类型不变量,`alive` 范围内的所有元素都已正确初始化。
        unsafe {
            let slice = self.data.get_unchecked_mut(self.alive.clone());
            slice.assume_init_mut()
        }
    }
}

impl<T: fmt::Debug> fmt::Debug for PolymorphicIter<[MaybeUninit<T>]> {
    #[inline]
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // 只打印尚未产出的元素:已经产出的元素不得再访问。
        f.debug_tuple("IntoIter").field(&self.as_slice()).finish()
    }
}

/// 等价于迭代器 trait 的方法。
///
/// 这里没有直接实现真正的迭代器 trait,因为我们想实现 `try_fold` 这类要求
/// `Self: Sized` 的方法,而当前类型并不满足该要求。
impl<T> PolymorphicIter<[MaybeUninit<T>]> {
    #[inline]
    pub(super) fn next(&mut self) -> Option<T> {
        // 从前端取得下一个索引。
        //
        // 将 `alive.start` 加 1 会维持关于 `alive` 的不变量。但由于这次修改,
        // 在短暂期间,活跃区不再是 `data[alive]`,而是 `data[idx..alive.end]`。
        self.alive.next().map(|idx| {
            // 从数组中读出该元素。
            // SAFETY: `idx` 是数组原“活跃”区域内的索引。读取该元素意味着
            // `data[idx]` 现在被视为死亡(不得触碰)。由于 `idx` 原本就是活跃区起点,
            // 此后活跃区重新变为 `data[alive]`,所有不变量恢复成立。
            unsafe { self.data.get_unchecked(idx).assume_init_read() }
        })
    }

    #[inline]
    pub(super) fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }

    #[inline]
    pub(super) fn advance_by(&mut self, n: usize) -> Result<(), NonZero<usize>> {
        // 这也会移动起点,在概念上把这些元素标记为“已 drop”。因此即使后续出错,
        // Drop 实现也不会二次释放它们。
        let range_to_drop = self.alive.take_prefix(n);
        let remaining = n - range_to_drop.len();

        // SAFETY: 这些元素当前已初始化,因此可以 drop。
        unsafe {
            let slice = self.data.get_unchecked_mut(range_to_drop);
            slice.assume_init_drop();
        }

        NonZero::new(remaining).map_or(Ok(()), Err)
    }

    #[inline]
    pub(super) fn fold<B>(&mut self, init: B, f: impl FnMut(B, T) -> B) -> B {
        self.try_fold(init, NeverShortCircuit::wrap_mut_2(f)).0
    }

    #[inline]
    pub(super) fn try_fold<B, F, R>(&mut self, init: B, mut f: F) -> R
    where
        F: FnMut(B, T) -> R,
        R: Try<Output = B>,
    {
        // `alive` 是 `IndexRange`,不是任意迭代器;因此可以信任它的 `try_fold`
        // 不会做出对同一索引多次调用 fold 闭包之类的异常行为。
        let data = &mut self.data;
        self.alive.try_fold(init, move |accum, idx| {
            // SAFETY: `idx` 已从 alive 范围中移除,所以即使 `f` panic,我们也不会再 drop 它;
            // 因而可以把该元素的所有权交给 `f` 处理。
            let elem = unsafe { data.get_unchecked(idx).assume_init_read() };
            f(accum, elem)
        })
    }

    #[inline]
    pub(super) fn next_back(&mut self) -> Option<T> {
        // 从后端取得下一个索引。
        //
        // 将 `alive.end` 减 1 会维持关于 `alive` 的不变量。但由于这次修改,
        // 在短暂期间,活跃区不再是 `data[alive]`,而是 `data[alive.start..=idx]`。
        self.alive.next_back().map(|idx| {
            // 从数组中读出该元素。
            // SAFETY: `idx` 是数组原“活跃”区域内的索引。读取该元素意味着
            // `data[idx]` 现在被视为死亡(不得触碰)。由于 `idx` 原本就是活跃区终点,
            // 此后活跃区重新变为 `data[alive]`,所有不变量恢复成立。
            unsafe { self.data.get_unchecked(idx).assume_init_read() }
        })
    }

    #[inline]
    pub(super) fn advance_back_by(&mut self, n: usize) -> Result<(), NonZero<usize>> {
        // 这也会移动终点,在概念上把这些元素标记为“已 drop”。因此即使后续出错,
        // Drop 实现也不会二次释放它们。
        let range_to_drop = self.alive.take_suffix(n);
        let remaining = n - range_to_drop.len();

        // SAFETY: 这些元素当前已初始化,因此可以 drop。
        unsafe {
            let slice = self.data.get_unchecked_mut(range_to_drop);
            slice.assume_init_drop();
        }

        NonZero::new(remaining).map_or(Ok(()), Err)
    }

    #[inline]
    pub(super) fn rfold<B>(&mut self, init: B, f: impl FnMut(B, T) -> B) -> B {
        self.try_rfold(init, NeverShortCircuit::wrap_mut_2(f)).0
    }

    #[inline]
    pub(super) fn try_rfold<B, F, R>(&mut self, init: B, mut f: F) -> R
    where
        F: FnMut(B, T) -> R,
        R: Try<Output = B>,
    {
        // `alive` 是 `IndexRange`,不是任意迭代器;因此可以信任它的 `try_rfold`
        // 不会做出对同一索引多次调用 fold 闭包之类的异常行为。
        let data = &mut self.data;
        self.alive.try_rfold(init, move |accum, idx| {
            // SAFETY: `idx` 已从 alive 范围中移除,所以即使 `f` panic,我们也不会再 drop 它;
            // 因而可以把该元素的所有权交给 `f` 处理。
            let elem = unsafe { data.get_unchecked(idx).assume_init_read() };
            f(accum, elem)
        })
    }
}
