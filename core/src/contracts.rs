//! 包含不稳定 contracts lang item 和属性宏的不稳定模块。

pub use crate::macros::builtin::{contracts_ensures as ensures, contracts_requires as requires};

/// 这是一个恒等函数，用作 `#[ensures]` 属性脱糖的一部分。
///
/// 这是现有的权宜做法，允许用户在 `ensures` 属性中省略返回值类型。
///
/// 理想情况下，rustc 应当能生成类型标注。
/// 现有 lowering 逻辑让添加显式类型标注相当困难，而函数调用相对直接。
#[unstable(feature = "contracts_internals", issue = "128044" /* compiler-team#759 */)]
// 与 `contract_check_requires` 类似，这里需要使用面向用户的 `contracts` feature，
// 而不是永久不稳定的 `contracts_internals`。const 检查不会遵守 contract 展开
// 所使用的 allow_internal_unstable 逻辑。
#[rustc_const_unstable(feature = "contracts", issue = "128044")]
#[lang = "contract_build_check_ensures"]
pub const fn build_check_ensures<Ret, C>(cond: C) -> C
where
    C: Fn(&Ret) -> bool + Copy + 'static,
{
    cond
}
