use crate::iter::InPlaceIterable;
use crate::num::NonZero;
use crate::ops::{ChangeOutputType, ControlFlow, FromResidual, Residual, Try};

mod array_chunks;
mod by_ref_sized;
mod chain;
mod cloned;
mod copied;
mod cycle;
mod enumerate;
mod filter;
mod filter_map;
mod flatten;
mod fuse;
mod inspect;
mod intersperse;
mod map;
mod map_while;
mod map_windows;
mod peekable;
mod rev;
mod scan;
mod skip;
mod skip_while;
mod step_by;
mod take;
mod take_while;
mod zip;

#[unstable(feature = "iter_array_chunks", issue = "100450")]
pub use self::array_chunks::ArrayChunks;
#[unstable(feature = "std_internals", issue = "none")]
pub use self::by_ref_sized::ByRefSized;
#[stable(feature = "iter_chain", since = "1.91.0")]
pub use self::chain::chain;
#[stable(feature = "iter_cloned", since = "1.1.0")]
pub use self::cloned::Cloned;
#[stable(feature = "iter_copied", since = "1.36.0")]
pub use self::copied::Copied;
#[stable(feature = "iterator_flatten", since = "1.29.0")]
pub use self::flatten::Flatten;
#[unstable(feature = "iter_intersperse", issue = "79524")]
pub use self::intersperse::{Intersperse, IntersperseWith};
#[stable(feature = "iter_map_while", since = "1.57.0")]
pub use self::map_while::MapWhile;
#[unstable(feature = "iter_map_windows", issue = "87155")]
pub use self::map_windows::MapWindows;
#[stable(feature = "iterator_step_by", since = "1.28.0")]
pub use self::step_by::StepBy;
#[unstable(feature = "trusted_random_access", issue = "none")]
pub use self::zip::TrustedRandomAccess;
#[unstable(feature = "trusted_random_access", issue = "none")]
pub use self::zip::TrustedRandomAccessNoCoerce;
#[stable(feature = "iter_zip", since = "1.59.0")]
pub use self::zip::zip;
#[stable(feature = "rust1", since = "1.0.0")]
pub use self::{
    chain::Chain, cycle::Cycle, enumerate::Enumerate, filter::Filter, filter_map::FilterMap,
    flatten::FlatMap, fuse::Fuse, inspect::Inspect, map::Map, peekable::Peekable, rev::Rev,
    scan::Scan, skip::Skip, skip_while::SkipWhile, take::Take, take_while::TakeWhile, zip::Zip,
};

/// 在迭代器适配器流水线中，传递式访问 source 阶段的 trait。
///
/// 满足下面条件时，流水线消费者可以通过该 trait 找到底层 source:
///
/// * 迭代器 source `S` 自身实现 `SourceIter<Source = S>`。
/// * source 与流水线消费者之间的每个适配器都提供委托实现，把访问继续向内转发。
///
/// 当 source 是拥有元素的迭代器结构体(通常称为 `IntoIter`)时，这可用于特化
/// [`FromIterator`] 实现，或在迭代器被部分消耗后取回剩余元素。
///
/// 注意，实现不一定必须暴露流水线最内层的 source。有状态的中间适配器可能已经急切
/// 求值了流水线的一部分，并把自己的内部存储作为 source 暴露出去。
///
/// 该 trait 是 unsafe，因为实现者必须维护额外安全性质。细节见 [`as_inner`]。
///
/// 该 trait 的主要用途是原地迭代。更多信息见 [`vec::in_place_collect`] 模块文档。
///
/// [`vec::in_place_collect`]: ../../../../alloc/vec/in_place_collect/index.html
///
/// # 示例
///
/// 取回已被部分消耗的 source:
///
/// ```
/// # #![feature(inplace_iteration)]
/// # use std::iter::SourceIter;
///
/// let mut iter = vec![9, 9, 9].into_iter().map(|i| i * i);
/// let _ = iter.next();
/// let mut remainder = std::mem::replace(unsafe { iter.as_inner() }, Vec::new().into_iter());
/// println!("n = {} elements remaining", remainder.len());
/// ```
///
/// [`FromIterator`]: crate::iter::FromIterator
/// [`as_inner`]: SourceIter::as_inner
#[unstable(issue = "none", feature = "inplace_iteration")]
#[doc(hidden)]
#[rustc_specialization_trait]
pub unsafe trait SourceIter {
    /// 迭代器流水线中的 source 阶段。
    type Source;

    /// 取出迭代器流水线的 source。
    ///
    /// # 安全性(Safety）
    ///
    /// 除非调用方替换了该引用，实现在自身生命周期内必须返回同一个可变引用。
    ///
    /// 调用方只有在已经停止迭代，并且会在取出 source 后丢弃整条迭代器流水线时，
    /// 才能替换这个引用。
    ///
    /// 这意味着迭代器适配器可以在迭代过程中依赖 source 不会被替换，但不能在自己的
    /// `Drop` 实现中继续依赖这一点。
    ///
    /// 实现该方法意味着适配器放弃对 source 的私有独占访问；之后只能依赖由方法接收
    /// 者类型表达的保证。由于访问不再受私有限制，即使适配器能接触 source 内部，
    /// 也必须维护 source 公共 API 承诺的不变量。
    ///
    /// 反过来，调用方也必须接受 source 可能处于任何符合其公共 API 的状态，因为位于
    /// 调用方与 source 之间的适配器拥有同样的访问能力。特别是，某个适配器可能已经
    /// 消耗了比表面上严格需要更多的元素。
    ///
    /// 这些要求的总体目标是允许流水线消费者使用:
    ///
    /// * 迭代停止后 source 中剩余的元素。
    /// * 消耗型迭代器推进后变为空闲的内存。
    ///
    /// [`next()`]: Iterator::next()
    unsafe fn as_inner(&mut self) -> &mut Self::Source;
}

/// 一个迭代器适配器: 只要底层迭代器产出的值经 `Try::branch` 判断为
/// `ControlFlow::Continue`，它就继续产出结果。
///
/// 如果遇到 `ControlFlow::Break`，迭代器会停止，并保存 residual。
pub(crate) struct GenericShunt<'a, I, R> {
    iter: I,
    residual: &'a mut Option<R>,
}

/// 把给定迭代器当作产出元素的 `Try::Output` 类型来处理。遇到任何 `Try::Residual`
/// 都会停止内部迭代器，并传播回整体结果。
pub(crate) fn try_process<I, T, R, F, U>(iter: I, mut f: F) -> ChangeOutputType<I::Item, U>
where
    I: Iterator<Item: Try<Output = T, Residual = R>>,
    for<'a> F: FnMut(GenericShunt<'a, I, R>) -> U,
    R: Residual<U>,
{
    let mut residual = None;
    let shunt = GenericShunt { iter, residual: &mut residual };
    let value = f(shunt);
    match residual {
        Some(r) => FromResidual::from_residual(r),
        None => Try::from_output(value),
    }
}

impl<I, R> Iterator for GenericShunt<'_, I, R>
where
    I: Iterator<Item: Try<Residual = R>>,
{
    type Item = <I::Item as Try>::Output;

    fn next(&mut self) -> Option<Self::Item> {
        self.try_for_each(ControlFlow::Break).break_value()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.residual.is_some() {
            (0, Some(0))
        } else {
            let (_, upper) = self.iter.size_hint();
            (0, upper)
        }
    }

    fn try_fold<B, F, T>(&mut self, init: B, mut f: F) -> T
    where
        F: FnMut(B, Self::Item) -> T,
        T: Try<Output = B>,
    {
        self.iter
            .try_fold(init, |acc, x| match Try::branch(x) {
                ControlFlow::Continue(x) => ControlFlow::from_try(f(acc, x)),
                ControlFlow::Break(r) => {
                    *self.residual = Some(r);
                    ControlFlow::Break(try { acc })
                }
            })
            .into_try()
    }

    impl_fold_via_try_fold! { fold -> try_fold }
}

#[unstable(issue = "none", feature = "inplace_iteration")]
unsafe impl<I, R> SourceIter for GenericShunt<'_, I, R>
where
    I: SourceIter,
{
    type Source = I::Source;

    #[inline]
    unsafe fn as_inner(&mut self) -> &mut Self::Source {
        // SAFETY: 转发到具有相同要求的 unsafe 函数。
        unsafe { SourceIter::as_inner(&mut self.iter) }
    }
}

// SAFETY: GenericShunt::next 调用 `I::try_for_each`；若要返回 `Some(_)`，它必须推进
// `iter`。由于 `iter` 的类型满足 `I: InPlaceIterable`，每一步都保证至少从底层
// source 中移出一项。
#[unstable(issue = "none", feature = "inplace_iteration")]
unsafe impl<I, R> InPlaceIterable for GenericShunt<'_, I, R>
where
    I: InPlaceIterable,
{
    const EXPAND_BY: Option<NonZero<usize>> = I::EXPAND_BY;
    const MERGE_BY: Option<NonZero<usize>> = I::MERGE_BY;
}
