use crate::iter::Step;
use crate::num::NonZero;

/// 与 `FusedIterator` 语义相同的内部可信标记。
///
/// # 安全性(Safety）
///
/// 该 trait 用于 specialization。实现不能依赖生命周期差异来改变 fused 语义；
/// 否则编译器和库内特化路径可能把某个实例当作“结束后永久返回 `None`”，而实际
/// 另一个生命周期实例又会恢复产出元素，导致优化分支的假设被破坏。
#[unstable(issue = "none", feature = "trusted_fused")]
#[doc(hidden)]
#[rustc_specialization_trait]
pub unsafe trait TrustedFused {}

/// 耗尽后会一直继续产出 `None` 的迭代器。
///
/// 对已经返回过一次 [`None`] 的 fused 迭代器再次调用 `next`，保证仍返回 [`None`]。
/// 这比普通 [`Iterator`] 的契约更强: 普通迭代器允许在 `None` 后重新返回
/// `Some(_)`，而 `FusedIterator` 明确禁止这种恢复。
///
/// 所有实际满足该语义的迭代器都应实现本 trait，因为它允许 [`Iterator::fuse()`]
/// 和其他适配器跳过额外状态检查。
///
/// 注意: 如果泛型代码需要一个 fused 迭代器，通常不应把 `FusedIterator` 写成泛型
/// bound，而应直接对传入迭代器调用 [`Iterator::fuse()`]。若迭代器本来已经 fused，
/// 额外的 [`Fuse`] 包装会被优化成无操作，没有性能开销。
///
/// [`Fuse`]: crate::iter::Fuse
#[stable(feature = "fused", since = "1.26.0")]
#[rustc_unsafe_specialization_marker]
// FIXME: 这里理论上应是 #[marker]，并为 T: TrustedFused 再提供一个 blanket impl，
// 但那会触发 iter::Fuse specialization 的 ICE。
#[lang = "fused_iterator"]
pub trait FusedIterator: Iterator {}

#[stable(feature = "fused", since = "1.26.0")]
impl<I: FusedIterator + ?Sized> FusedIterator for &mut I {}

/// 使用 `size_hint` 报告可信准确长度的迭代器。
///
/// 迭代器报告的 size hint 必须满足两种形式之一: 要么精确，即下界等于上界；
/// 要么上界为 [`None`]。上界只有在实际长度大于 [`usize::MAX`] 时才允许为
/// [`None`]；这种情况下下界必须是 [`usize::MAX`]，也就是
/// [`Iterator::size_hint()`] 返回 `(usize::MAX, None)`。
///
/// 迭代器必须精确地产出自己报告的元素数量，或者在到达末尾前发散。和
/// `ExactSizeIterator` 不同，`TrustedLen` 是 unsafe trait，消费者可以在 unsafe
/// 代码中依赖它来预留并初始化内存；错误实现可能让消费者越界写入、读取未初始化
/// 元素或错误地跳过检查，从而造成 UB。
///
/// # 适配器何时不应实现 `TrustedLen`?
///
/// 如果适配器会按某个数量让迭代器变短，它通常不应实现 `TrustedLen`。内层迭代器
/// 可能实际返回超过 `usize::MAX` 项，但它的 `size_hint` 已经饱和并丢失了精确信息；
/// 此时无法知道“少 `k` 个元素”后的真实长度。
///
/// 这就是 [`Skip<I>`](crate::iter::Skip) 即使在 `I` 实现 `TrustedLen` 时也不实现
/// `TrustedLen` 的原因。
///
/// # 安全性(Safety）
///
/// 只有在完全维护上述契约时才能实现该 trait。消费者必须检查
/// [`Iterator::size_hint()`] 的上界: `Some(upper)` 表示可以信任接下来至少且至多
/// 有 `upper` 项；`None` 只表示长度超过 [`usize::MAX`]，不能被当作“未知但有限”。
#[unstable(feature = "trusted_len", issue = "37572")]
#[rustc_unsafe_specialization_marker]
pub unsafe trait TrustedLen: Iterator {}

#[unstable(feature = "trusted_len", issue = "37572")]
unsafe impl<I: TrustedLen + ?Sized> TrustedLen for &mut I {}

/// 每产出一项时都至少从底层 [`SourceIter`] 取走一个元素的迭代器。
///
/// 调用任何会推进迭代器的方法，例如 [`next()`] 或 [`try_fold()`]，都保证每一步至少
/// 已经从迭代器底层 source 中移走一个值；如果 source 的结构允许，就可以把迭代器
/// 链产出的结果写回这个腾出的槽位。换句话说，该 trait 表示这条迭代器流水线可以
/// 被原地收集。
///
/// 该 trait 的主要用途是原地迭代。更多信息见 [`vec::in_place_collect`] 模块文档。
///
/// [`vec::in_place_collect`]: ../../../../alloc/vec/in_place_collect/index.html
/// [`SourceIter`]: crate::iter::SourceIter
/// [`next()`]: Iterator::next
/// [`try_fold()`]: Iterator::try_fold
#[unstable(issue = "none", feature = "inplace_iteration")]
#[doc(hidden)]
#[rustc_specialization_trait]
pub unsafe trait InPlaceIterable {
    /// 迭代器流水线中所有一对多展开倍数的乘积。
    /// 例如 [[u8; 4]; 4].iter().flatten().flatten() 的 `EXPAND_BY` 为 16。
    /// 这是上界，即每个输入最多会产生这么多输出项，用于布局计算。
    const EXPAND_BY: Option<NonZero<usize>>;
    /// 迭代器流水线中所有多对一归并倍数的乘积。
    /// 例如 [u8].iter().array_chunks::<4>().array_chunks::<4>() 的 `MERGE_BY` 为 16。
    /// 这是下界，即每个输出至少会消耗这么多输入项。
    const MERGE_BY: Option<NonZero<usize>>;
}

/// 维护 [`Step`] 全部不变量的类型。
///
/// [`Step::steps_between()`] 的不变量是 [`TrustedLen`] 不变量的超集。因此，使用同一
/// 泛型参数的所有 range 类型都可以基于 `TrustedStep` 实现 [`TrustedLen`]。
///
/// # 安全性(Safety）
///
/// 给定类型的 [`Step`] 实现必须保证所有方法的不变量都成立。具体要求见 [`Step`]
/// trait 文档。消费者可以在 unsafe 代码中依赖这些不变量，例如把 range 的长度当作
/// 可信精确长度来进行未检查访问或预分配。
#[unstable(feature = "trusted_step", issue = "85731")]
#[rustc_specialization_trait]
pub unsafe trait TrustedStep: Step + Copy {}
