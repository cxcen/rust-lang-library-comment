//! 本模块及其子模块包含高效且健壮的排序实现，也包含与排序领域相邻的
//! `select_nth_unstable` 实现。
//!
//! 这里的算法位于 `core` 中，不能依赖分配器；稳定排序需要的临时存储通过上层传入的
//! buffer 抽象提供。不稳定排序和选择算法则尽量原地工作，同时在最坏情况下保持明确复杂度。

pub mod stable;
pub mod unstable;

pub(crate) mod select;
pub(crate) mod shared;
