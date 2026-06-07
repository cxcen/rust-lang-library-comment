//! 切片管理与操作。
//!
//! 切片是 Rust 中“指向连续元素序列的胖指针”：指针部分指向第一个元素，metadata 是元素个数。
//! 本模块提供不拥有内存的视图操作、索引、分割、迭代、排序和与裸指针之间的转换。
//! 安全 API 会把越界暴露为 panic 或 `None`，unsafe API 则把边界、初始化、对齐和 aliasing
//! 不变量交给调用方；违反这些契约会破坏编译器对引用的基本假设并导致 UB。
//!
//! 更多细节见 [`std::slice`]。
//!
//! [`std::slice`]: ../../std/slice/index.html

#![stable(feature = "rust1", since = "1.0.0")]

use crate::clone::TrivialClone;
use crate::cmp::Ordering::{self, Equal, Greater, Less};
use crate::intrinsics::{exact_div, unchecked_sub};
use crate::marker::Destruct;
use crate::mem::{self, MaybeUninit, SizedTypeProperties};
use crate::num::NonZero;
use crate::ops::{OneSidedRange, OneSidedRangeBound, Range, RangeBounds, RangeInclusive};
use crate::panic::const_panic;
use crate::simd::{self, Simd};
use crate::ub_checks::assert_unsafe_precondition;
use crate::{fmt, hint, ptr, range, slice};

#[unstable(
    feature = "slice_internals",
    issue = "none",
    reason = "exposed from core to be reused in std; use the memchr crate"
)]
#[doc(hidden)]
/// 纯 Rust 的 memchr 实现，来源于 rust-memchr。
pub mod memchr;

#[unstable(
    feature = "slice_internals",
    issue = "none",
    reason = "exposed from core to be reused in std;"
)]
#[doc(hidden)]
pub mod sort;

mod ascii;
mod cmp;
pub(crate) mod index;
mod iter;
mod raw;
mod rotate;
mod specialize;

#[stable(feature = "inherent_ascii_escape", since = "1.60.0")]
pub use ascii::EscapeAscii;
#[unstable(feature = "str_internals", issue = "none")]
#[doc(hidden)]
pub use ascii::is_ascii_simple;
#[stable(feature = "slice_get_slice", since = "1.28.0")]
pub use index::SliceIndex;
#[unstable(feature = "slice_range", issue = "76393")]
pub use index::{range, try_range};
#[stable(feature = "array_windows", since = "1.94.0")]
pub use iter::ArrayWindows;
#[stable(feature = "slice_group_by", since = "1.77.0")]
pub use iter::{ChunkBy, ChunkByMut};
#[stable(feature = "rust1", since = "1.0.0")]
pub use iter::{Chunks, ChunksMut, Windows};
#[stable(feature = "chunks_exact", since = "1.31.0")]
pub use iter::{ChunksExact, ChunksExactMut};
#[stable(feature = "rust1", since = "1.0.0")]
pub use iter::{Iter, IterMut};
#[stable(feature = "rchunks", since = "1.31.0")]
pub use iter::{RChunks, RChunksExact, RChunksExactMut, RChunksMut};
#[stable(feature = "slice_rsplit", since = "1.27.0")]
pub use iter::{RSplit, RSplitMut};
#[stable(feature = "rust1", since = "1.0.0")]
pub use iter::{RSplitN, RSplitNMut, Split, SplitMut, SplitN, SplitNMut};
#[stable(feature = "split_inclusive", since = "1.51.0")]
pub use iter::{SplitInclusive, SplitInclusiveMut};
#[stable(feature = "from_ref", since = "1.28.0")]
pub use raw::{from_mut, from_ref};
#[unstable(feature = "slice_from_ptr_range", issue = "89792")]
pub use raw::{from_mut_ptr_range, from_ptr_range};
#[stable(feature = "rust1", since = "1.0.0")]
pub use raw::{from_raw_parts, from_raw_parts_mut};

/// 计算单侧范围的分割方向和分割点。
///
/// 这是 `split_off` 与 `split_off_mut` 的辅助函数：它返回从前端还是后端分割，
/// 以及实际的分割索引。如果闭合终点加一会溢出，则返回 `None`。
#[inline]
fn split_point_of(range: impl OneSidedRange<usize>) -> Option<(Direction, usize)> {
    use OneSidedRangeBound::{End, EndInclusive, StartInclusive};

    Some(match range.bound() {
        (StartInclusive, i) => (Direction::Back, i),
        (End, i) => (Direction::Front, i),
        (EndInclusive, i) => (Direction::Front, i.checked_add(1)?),
    })
}

enum Direction {
    Front,
    Back,
}

impl<T> [T] {
    /// 返回切片中的元素个数。
    ///
    /// 这是胖指针 metadata 中保存的长度，不是字节数；切片覆盖的字节数取决于
    /// `size_of::<T>()`。
    ///
    /// # 示例
    ///
    /// ```
    /// let a = [1, 2, 3];
    /// assert_eq!(a.len(), 3);
    /// ```
    #[lang = "slice_len_fn"]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_slice_len", since = "1.39.0")]
    #[rustc_no_implicit_autorefs]
    #[inline]
    #[must_use]
    pub const fn len(&self) -> usize {
        ptr::metadata(self)
    }

    /// 如果切片长度为 0，返回 `true`。
    ///
    /// # 示例
    ///
    /// ```
    /// let a = [1, 2, 3];
    /// assert!(!a.is_empty());
    ///
    /// let b: &[i32] = &[];
    /// assert!(b.is_empty());
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_slice_is_empty", since = "1.39.0")]
    #[rustc_no_implicit_autorefs]
    #[inline]
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// 返回切片第一个元素；如果切片为空则返回 `None`。
    ///
    /// # 示例
    ///
    /// ```
    /// let v = [10, 40, 30];
    /// assert_eq!(Some(&10), v.first());
    ///
    /// let w: &[i32] = &[];
    /// assert_eq!(None, w.first());
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_slice_first_last_not_mut", since = "1.56.0")]
    #[inline]
    #[must_use]
    pub const fn first(&self) -> Option<&T> {
        if let [first, ..] = self { Some(first) } else { None }
    }

    /// 返回切片第一个元素的可变引用；如果切片为空则返回 `None`。
    ///
    /// # 示例
    ///
    /// ```
    /// let x = &mut [0, 1, 2];
    ///
    /// if let Some(first) = x.first_mut() {
    ///     *first = 5;
    /// }
    /// assert_eq!(x, &[5, 1, 2]);
    ///
    /// let y: &mut [i32] = &mut [];
    /// assert_eq!(None, y.first_mut());
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_slice_first_last", since = "1.83.0")]
    #[inline]
    #[must_use]
    pub const fn first_mut(&mut self) -> Option<&mut T> {
        if let [first, ..] = self { Some(first) } else { None }
    }

    /// 返回第一个元素以及剩余元素切片；如果切片为空则返回 `None`。
    ///
    /// # 示例
    ///
    /// ```
    /// let x = &[0, 1, 2];
    ///
    /// if let Some((first, elements)) = x.split_first() {
    ///     assert_eq!(first, &0);
    ///     assert_eq!(elements, &[1, 2]);
    /// }
    /// ```
    #[stable(feature = "slice_splits", since = "1.5.0")]
    #[rustc_const_stable(feature = "const_slice_first_last_not_mut", since = "1.56.0")]
    #[inline]
    #[must_use]
    pub const fn split_first(&self) -> Option<(&T, &[T])> {
        if let [first, tail @ ..] = self { Some((first, tail)) } else { None }
    }

    /// 返回第一个元素的可变引用以及剩余元素的可变切片；如果切片为空则返回 `None`。
    ///
    /// # 示例
    ///
    /// ```
    /// let x = &mut [0, 1, 2];
    ///
    /// if let Some((first, elements)) = x.split_first_mut() {
    ///     *first = 3;
    ///     elements[0] = 4;
    ///     elements[1] = 5;
    /// }
    /// assert_eq!(x, &[3, 4, 5]);
    /// ```
    #[stable(feature = "slice_splits", since = "1.5.0")]
    #[rustc_const_stable(feature = "const_slice_first_last", since = "1.83.0")]
    #[inline]
    #[must_use]
    pub const fn split_first_mut(&mut self) -> Option<(&mut T, &mut [T])> {
        if let [first, tail @ ..] = self { Some((first, tail)) } else { None }
    }

    /// 返回最后一个元素以及它之前的所有元素；如果切片为空则返回 `None`。
    ///
    /// # 示例
    ///
    /// ```
    /// let x = &[0, 1, 2];
    ///
    /// if let Some((last, elements)) = x.split_last() {
    ///     assert_eq!(last, &2);
    ///     assert_eq!(elements, &[0, 1]);
    /// }
    /// ```
    #[stable(feature = "slice_splits", since = "1.5.0")]
    #[rustc_const_stable(feature = "const_slice_first_last_not_mut", since = "1.56.0")]
    #[inline]
    #[must_use]
    pub const fn split_last(&self) -> Option<(&T, &[T])> {
        if let [init @ .., last] = self { Some((last, init)) } else { None }
    }

    /// 返回最后一个元素的可变引用以及它之前所有元素的可变切片；如果切片为空则返回 `None`。
    ///
    /// # 示例
    ///
    /// ```
    /// let x = &mut [0, 1, 2];
    ///
    /// if let Some((last, elements)) = x.split_last_mut() {
    ///     *last = 3;
    ///     elements[0] = 4;
    ///     elements[1] = 5;
    /// }
    /// assert_eq!(x, &[4, 5, 3]);
    /// ```
    #[stable(feature = "slice_splits", since = "1.5.0")]
    #[rustc_const_stable(feature = "const_slice_first_last", since = "1.83.0")]
    #[inline]
    #[must_use]
    pub const fn split_last_mut(&mut self) -> Option<(&mut T, &mut [T])> {
        if let [init @ .., last] = self { Some((last, init)) } else { None }
    }

    /// 返回切片最后一个元素；如果切片为空则返回 `None`。
    ///
    /// # 示例
    ///
    /// ```
    /// let v = [10, 40, 30];
    /// assert_eq!(Some(&30), v.last());
    ///
    /// let w: &[i32] = &[];
    /// assert_eq!(None, w.last());
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_slice_first_last_not_mut", since = "1.56.0")]
    #[inline]
    #[must_use]
    pub const fn last(&self) -> Option<&T> {
        if let [.., last] = self { Some(last) } else { None }
    }

    /// 返回切片最后一个元素的可变引用；如果切片为空则返回 `None`。
    ///
    /// # 示例
    ///
    /// ```
    /// let x = &mut [0, 1, 2];
    ///
    /// if let Some(last) = x.last_mut() {
    ///     *last = 10;
    /// }
    /// assert_eq!(x, &[0, 1, 10]);
    ///
    /// let y: &mut [i32] = &mut [];
    /// assert_eq!(None, y.last_mut());
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_slice_first_last", since = "1.83.0")]
    #[inline]
    #[must_use]
    pub const fn last_mut(&mut self) -> Option<&mut T> {
        if let [.., last] = self { Some(last) } else { None }
    }

    /// 返回切片前 `N` 个元素组成的数组引用。
///
    /// 如果切片长度小于 `N`，返回 `None`。返回的是 `[T; N]` 引用，因此长度在类型层面固定。
    ///
    /// # 示例
    ///
    /// ```
    /// let u = [10, 40, 30];
    /// assert_eq!(Some(&[10, 40]), u.first_chunk::<2>());
    ///
    /// let v: &[i32] = &[10];
    /// assert_eq!(None, v.first_chunk::<2>());
    ///
    /// let w: &[i32] = &[];
    /// assert_eq!(Some(&[]), w.first_chunk::<0>());
    /// ```
    #[inline]
    #[stable(feature = "slice_first_last_chunk", since = "1.77.0")]
    #[rustc_const_stable(feature = "slice_first_last_chunk", since = "1.77.0")]
    pub const fn first_chunk<const N: usize>(&self) -> Option<&[T; N]> {
        if self.len() < N {
            None
        } else {
            // SAFETY: 已显式检查元素个数至少为 `N`，并且返回引用不会超过原切片生命周期。
            Some(unsafe { &*(self.as_ptr().cast_array()) })
        }
    }

    /// 返回切片前 `N` 个元素组成的可变数组引用。
///
    /// 如果切片长度小于 `N`，返回 `None`。
    ///
    /// # 示例
    ///
    /// ```
    /// let x = &mut [0, 1, 2];
    ///
    /// if let Some(first) = x.first_chunk_mut::<2>() {
    ///     first[0] = 5;
    ///     first[1] = 4;
    /// }
    /// assert_eq!(x, &[5, 4, 2]);
    ///
    /// assert_eq!(None, x.first_chunk_mut::<4>());
    /// ```
    #[inline]
    #[stable(feature = "slice_first_last_chunk", since = "1.77.0")]
    #[rustc_const_stable(feature = "const_slice_first_last_chunk", since = "1.83.0")]
    pub const fn first_chunk_mut<const N: usize>(&mut self) -> Option<&mut [T; N]> {
        if self.len() < N {
            None
        } else {
            // SAFETY: 已显式检查元素个数至少为 `N`；返回引用不超过原切片生命周期；
            // `&mut self` 提供整个切片的独占访问权，因此可安全地产生可变数组引用。
            Some(unsafe { &mut *(self.as_mut_ptr().cast_array()) })
        }
    }

    /// 返回切片前 `N` 个元素组成的数组引用，以及剩余元素切片。
///
    /// 如果切片长度小于 `N`，返回 `None`。
    ///
    /// # 示例
    ///
    /// ```
    /// let x = &[0, 1, 2];
    ///
    /// if let Some((first, elements)) = x.split_first_chunk::<2>() {
    ///     assert_eq!(first, &[0, 1]);
    ///     assert_eq!(elements, &[2]);
    /// }
    ///
    /// assert_eq!(None, x.split_first_chunk::<4>());
    /// ```
    #[inline]
    #[stable(feature = "slice_first_last_chunk", since = "1.77.0")]
    #[rustc_const_stable(feature = "slice_first_last_chunk", since = "1.77.0")]
    pub const fn split_first_chunk<const N: usize>(&self) -> Option<(&[T; N], &[T])> {
        let Some((first, tail)) = self.split_at_checked(N) else { return None };

        // SAFETY: `split_at_checked(N)` 已证明前缀恰有 `N` 个元素，返回引用不超过原切片生命周期。
        Some((unsafe { &*(first.as_ptr().cast_array()) }, tail))
    }

    /// 返回切片前 `N` 个元素组成的可变数组引用，以及剩余元素的可变切片。
///
    /// 如果切片长度小于 `N`，返回 `None`。
    ///
    /// # 示例
    ///
    /// ```
    /// let x = &mut [0, 1, 2];
    ///
    /// if let Some((first, elements)) = x.split_first_chunk_mut::<2>() {
    ///     first[0] = 3;
    ///     first[1] = 4;
    ///     elements[0] = 5;
    /// }
    /// assert_eq!(x, &[3, 4, 5]);
    ///
    /// assert_eq!(None, x.split_first_chunk_mut::<4>());
    /// ```
    #[inline]
    #[stable(feature = "slice_first_last_chunk", since = "1.77.0")]
    #[rustc_const_stable(feature = "const_slice_first_last_chunk", since = "1.83.0")]
    pub const fn split_first_chunk_mut<const N: usize>(
        &mut self,
    ) -> Option<(&mut [T; N], &mut [T])> {
        let Some((first, tail)) = self.split_at_mut_checked(N) else { return None };

        // SAFETY: `split_at_mut_checked(N)` 已证明前缀恰有 `N` 个元素，并把两个可变切片
        // 分成不重叠区域；返回引用不超过原切片生命周期。
        Some((unsafe { &mut *(first.as_mut_ptr().cast_array()) }, tail))
    }

    /// 返回切片最后 `N` 个元素组成的数组引用，以及剩余的前缀切片。
///
    /// 如果切片长度小于 `N`，返回 `None`。
    ///
    /// # 示例
    ///
    /// ```
    /// let x = &[0, 1, 2];
    ///
    /// if let Some((elements, last)) = x.split_last_chunk::<2>() {
    ///     assert_eq!(elements, &[0]);
    ///     assert_eq!(last, &[1, 2]);
    /// }
    ///
    /// assert_eq!(None, x.split_last_chunk::<4>());
    /// ```
    #[inline]
    #[stable(feature = "slice_first_last_chunk", since = "1.77.0")]
    #[rustc_const_stable(feature = "slice_first_last_chunk", since = "1.77.0")]
    pub const fn split_last_chunk<const N: usize>(&self) -> Option<(&[T], &[T; N])> {
        let Some(index) = self.len().checked_sub(N) else { return None };
        let (init, last) = self.split_at(index);

        // SAFETY: `checked_sub(N)` 与 `split_at(index)` 已证明后缀恰有 `N` 个元素，
        // 返回引用不超过原切片生命周期。
        Some((init, unsafe { &*(last.as_ptr().cast_array()) }))
    }

    /// 返回切片最后 `N` 个元素组成的可变数组引用，以及剩余前缀的可变切片。
///
    /// 如果切片长度小于 `N`，返回 `None`。
    ///
    /// # 示例
    ///
    /// ```
    /// let x = &mut [0, 1, 2];
    ///
    /// if let Some((elements, last)) = x.split_last_chunk_mut::<2>() {
    ///     last[0] = 3;
    ///     last[1] = 4;
    ///     elements[0] = 5;
    /// }
    /// assert_eq!(x, &[5, 3, 4]);
    ///
    /// assert_eq!(None, x.split_last_chunk_mut::<4>());
    /// ```
    #[inline]
    #[stable(feature = "slice_first_last_chunk", since = "1.77.0")]
    #[rustc_const_stable(feature = "const_slice_first_last_chunk", since = "1.83.0")]
    pub const fn split_last_chunk_mut<const N: usize>(
        &mut self,
    ) -> Option<(&mut [T], &mut [T; N])> {
        let Some(index) = self.len().checked_sub(N) else { return None };
        let (init, last) = self.split_at_mut(index);

        // SAFETY: `split_at_mut(index)` 把前缀和后缀分成不重叠区域，后缀长度恰为 `N`；
        // 返回引用不超过原切片生命周期。
        Some((init, unsafe { &mut *(last.as_mut_ptr().cast_array()) }))
    }

    /// 返回切片最后 `N` 个元素组成的数组引用。
///
    /// 如果切片长度小于 `N`，返回 `None`。
    ///
    /// # 示例
    ///
    /// ```
    /// let u = [10, 40, 30];
    /// assert_eq!(Some(&[40, 30]), u.last_chunk::<2>());
    ///
    /// let v: &[i32] = &[10];
    /// assert_eq!(None, v.last_chunk::<2>());
    ///
    /// let w: &[i32] = &[];
    /// assert_eq!(Some(&[]), w.last_chunk::<0>());
    /// ```
    #[inline]
    #[stable(feature = "slice_first_last_chunk", since = "1.77.0")]
    #[rustc_const_stable(feature = "const_slice_last_chunk", since = "1.80.0")]
    pub const fn last_chunk<const N: usize>(&self) -> Option<&[T; N]> {
        // FIXME(const-hack): 缺少 const traits 时，需要用这种写法代替 `get`。
        let Some(index) = self.len().checked_sub(N) else { return None };
        let (_, last) = self.split_at(index);

        // SAFETY: `checked_sub(N)` 与 `split_at(index)` 已证明后缀恰有 `N` 个元素，
        // 返回引用不超过原切片生命周期。
        Some(unsafe { &*(last.as_ptr().cast_array()) })
    }

    /// 返回切片最后 `N` 个元素组成的可变数组引用。
///
    /// 如果切片长度小于 `N`，返回 `None`。
    ///
    /// # 示例
    ///
    /// ```
    /// let x = &mut [0, 1, 2];
    ///
    /// if let Some(last) = x.last_chunk_mut::<2>() {
    ///     last[0] = 10;
    ///     last[1] = 20;
    /// }
    /// assert_eq!(x, &[0, 10, 20]);
    ///
    /// assert_eq!(None, x.last_chunk_mut::<4>());
    /// ```
    #[inline]
    #[stable(feature = "slice_first_last_chunk", since = "1.77.0")]
    #[rustc_const_stable(feature = "const_slice_first_last_chunk", since = "1.83.0")]
    pub const fn last_chunk_mut<const N: usize>(&mut self) -> Option<&mut [T; N]> {
        // FIXME(const-hack): 缺少 const traits 时，需要用这种写法代替 `get`。
        let Some(index) = self.len().checked_sub(N) else { return None };
        let (_, last) = self.split_at_mut(index);

        // SAFETY: 已证明后缀恰有 `N` 个元素；`&mut self` 提供独占访问权，返回引用不超过原切片生命周期。
        Some(unsafe { &mut *(last.as_mut_ptr().cast_array()) })
    }

    /// 根据索引类型返回某个元素或子切片的共享引用。
    ///
    /// - 如果传入单个位置，位置在边界内则返回对应元素引用，越界则返回 `None`。
    /// - 如果传入范围，范围在边界内则返回对应子切片，越界或反向范围则返回 `None`。
    ///
    /// # 示例
    ///
    /// ```
    /// let v = [10, 40, 30];
    /// assert_eq!(Some(&40), v.get(1));
    /// assert_eq!(Some(&[10, 40][..]), v.get(0..2));
    /// assert_eq!(None, v.get(3));
    /// assert_eq!(None, v.get(0..4));
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_no_implicit_autorefs]
    #[inline]
    #[must_use]
    #[rustc_const_unstable(feature = "const_index", issue = "143775")]
    pub const fn get<I>(&self, index: I) -> Option<&I::Output>
    where
        I: [const] SliceIndex<Self>,
    {
        index.get(self)
    }

    /// 根据索引类型返回某个元素或子切片的可变引用；索引越界时返回 `None`。
    ///
    /// 语义与 [`get`] 相同，但返回值带有可变借用，因此成功时会把对应元素或子切片的
    /// 可变访问权借出。
    ///
    /// [`get`]: slice::get
    ///
    /// # 示例
    ///
    /// ```
    /// let x = &mut [0, 1, 2];
    ///
    /// if let Some(elem) = x.get_mut(1) {
    ///     *elem = 42;
    /// }
    /// assert_eq!(x, &[0, 42, 2]);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_no_implicit_autorefs]
    #[inline]
    #[must_use]
    #[rustc_const_unstable(feature = "const_index", issue = "143775")]
    pub const fn get_mut<I>(&mut self, index: I) -> Option<&mut I::Output>
    where
        I: [const] SliceIndex<Self>,
    {
        index.get_mut(self)
    }

    /// 不做边界检查，直接返回某个元素或子切片的共享引用。
///
    /// 安全替代方案见 [`get`]。
    ///
    /// # 安全性(Safety）
    ///
    /// 使用越界索引调用本方法是 *[undefined behavior]*，即使得到的引用之后没有被使用。
///
    /// 可以把它理解为 `.get(index).unwrap_unchecked()`：调用方必须先证明索引满足
    /// 对应 `SliceIndex` 的边界条件。调用 `.get_unchecked(len)` 是 UB，哪怕立刻把
    /// 结果转换成裸指针；调用 `.get_unchecked(..len + 1)`、`.get_unchecked(..=len)`
    /// 或类似越界范围同样是 UB。`get_unchecked` 不只是“省略 panic”，它把越界不存在
    /// 作为优化前提交给编译器。
    ///
    /// [`get`]: slice::get
    /// [undefined behavior]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    ///
    /// # 示例
    ///
    /// ```
    /// let x = &[1, 2, 4];
    ///
    /// unsafe {
    ///     assert_eq!(x.get_unchecked(1), &2);
    /// }
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_no_implicit_autorefs]
    #[inline]
    #[must_use]
    #[track_caller]
    #[rustc_const_unstable(feature = "const_index", issue = "143775")]
    pub const unsafe fn get_unchecked<I>(&self, index: I) -> &I::Output
    where
        I: [const] SliceIndex<Self>,
    {
        // SAFETY: 调用方必须维护 `get_unchecked` 的索引边界契约；`self` 是安全引用，
        // 因而切片本身可解引用。`SliceIndex` 的 unsafe 实现负责保证返回指针对应
        // 一个可形成共享引用的有效输出。
        unsafe { &*index.get_unchecked(self) }
    }

    /// 不做边界检查，直接返回某个元素或子切片的可变引用。
///
    /// 安全替代方案见 [`get_mut`]。
    ///
    /// # 安全性(Safety）
    ///
    /// 使用越界索引调用本方法是 *[undefined behavior]*，即使得到的引用之后没有被使用。
///
    /// 可以把它理解为 `.get_mut(index).unwrap_unchecked()`。调用
    /// `.get_unchecked_mut(len)` 是 UB，哪怕立刻把结果转换成裸指针；
    /// 调用 `.get_unchecked_mut(..len + 1)`、`.get_unchecked_mut(..=len)` 或类似
    /// 越界范围同样是 UB。可变版本还要求返回区域与任何其它活跃引用不发生 aliasing。
    ///
    /// [`get_mut`]: slice::get_mut
    /// [undefined behavior]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    ///
    /// # 示例
    ///
    /// ```
    /// let x = &mut [1, 2, 4];
    ///
    /// unsafe {
    ///     let elem = x.get_unchecked_mut(1);
    ///     *elem = 13;
    /// }
    /// assert_eq!(x, &[1, 13, 4]);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_no_implicit_autorefs]
    #[inline]
    #[must_use]
    #[track_caller]
    #[rustc_const_unstable(feature = "const_index", issue = "143775")]
    pub const unsafe fn get_unchecked_mut<I>(&mut self, index: I) -> &mut I::Output
    where
        I: [const] SliceIndex<Self>,
    {
        // SAFETY: 调用方必须维护 `get_unchecked_mut` 的边界与唯一访问契约；`self`
        // 是安全可变引用，切片本身可解引用。`SliceIndex` 的 unsafe 实现负责保证
        // 返回指针对应一个有效且可唯一借用的输出。
        unsafe { &mut *index.get_unchecked_mut(self) }
    }

    /// 返回指向切片缓冲区起始位置的裸共享指针。
///
    /// 调用方必须保证切片比返回指针活得更久，否则该指针会悬垂。
///
    /// 调用方还必须保证：不能通过这个指针或从它直接派生出的指针写入其指向的内存，
    /// 除非写入发生在 `UnsafeCell` 内部。需要修改切片内容时应使用 [`as_mut_ptr`]，
    /// 以免破坏共享引用的 aliasing 契约。
///
    /// 如果切片背后的容器被修改并导致缓冲区重新分配，先前取得的所有指针都会失效。
    ///
    /// # 示例
    ///
    /// ```
    /// let x = &[1, 2, 4];
    /// let x_ptr = x.as_ptr();
    ///
    /// unsafe {
    ///     for i in 0..x.len() {
    ///         assert_eq!(x.get_unchecked(i), &*x_ptr.add(i));
    ///     }
    /// }
    /// ```
    ///
    /// [`as_mut_ptr`]: slice::as_mut_ptr
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_slice_as_ptr", since = "1.32.0")]
    #[rustc_never_returns_null_ptr]
    #[rustc_as_ptr]
    #[inline(always)]
    #[must_use]
    pub const fn as_ptr(&self) -> *const T {
        self as *const [T] as *const T
    }

    /// 返回指向切片缓冲区起始位置的裸可变指针。
///
    /// 调用方必须保证切片比返回指针活得更久，否则该指针会悬垂。
///
    /// 如果切片背后的容器被修改并导致缓冲区重新分配，先前取得的所有指针都会失效。
    /// 使用该指针写入时仍需遵守裸指针和 `&mut [T]` 的 aliasing 规则。
    ///
    /// # 示例
    ///
    /// ```
    /// let x = &mut [1, 2, 4];
    /// let x_ptr = x.as_mut_ptr();
    ///
    /// unsafe {
    ///     for i in 0..x.len() {
    ///         *x_ptr.add(i) += 2;
    ///     }
    /// }
    /// assert_eq!(x, &[3, 4, 6]);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_ptr_offset", since = "1.61.0")]
    #[rustc_never_returns_null_ptr]
    #[rustc_as_ptr]
    #[inline(always)]
    #[must_use]
    pub const fn as_mut_ptr(&mut self) -> *mut T {
        self as *mut [T] as *mut T
    }

    /// 返回横跨整个切片的两个裸共享指针。
///
    /// 返回范围是半开区间，表示 `start..end`。其中 `end` 指向最后一个元素之后的
    /// *一过末尾* 位置。这样空切片可表示为两个相等指针，两个指针之间的距离表示切片长度。
///
    /// 使用这些指针时请参阅 [`as_ptr`] 的注意事项。`end` 指针需要额外谨慎，
    /// 因为它不指向切片中的有效元素，不能被解引用。
///
    /// 该函数适合与使用两个指针表示元素范围的外部接口交互，例如 C++ 常见的
    /// `[begin, end)` 约定。
///
    /// 它也可用于检查某个元素指针是否落在此切片范围内：
    ///
    /// ```
    /// let a = [1, 2, 3];
    /// let x = &a[1] as *const _;
    /// let y = &5 as *const _;
    ///
    /// assert!(a.as_ptr_range().contains(&x));
    /// assert!(!a.as_ptr_range().contains(&y));
    /// ```
    ///
    /// [`as_ptr`]: slice::as_ptr
    #[stable(feature = "slice_ptr_range", since = "1.48.0")]
    #[rustc_const_stable(feature = "const_ptr_offset", since = "1.61.0")]
    #[inline]
    #[must_use]
    pub const fn as_ptr_range(&self) -> Range<*const T> {
        let start = self.as_ptr();
        // SAFETY: 这里调用 `add` 是安全的，原因如下：
        //
        //   - 起始指针和一过末尾指针都属于同一个对象；直接指向对象之后的位置也算同一范围。
        //
        //   - 切片大小不会超过 `isize::MAX` 字节，相关约定见：
        //       - https://github.com/rust-lang/unsafe-code-guidelines/issues/102#issuecomment-473340447
        //       - https://doc.rust-lang.org/reference/behavior-considered-undefined.html
        //       - https://doc.rust-lang.org/core/slice/fn.from_raw_parts.html#safety
        //     （这看起来尚未完全规范化，但切片的 Index 实现等多处代码都依赖同一假设。）
        //
        //   - 切片不会绕过地址空间末尾，因此这里不存在地址回绕。
        //
        // 另见 [`pointer::add`] 的文档。
        let end = unsafe { start.add(self.len()) };
        start..end
    }

    /// 返回横跨整个切片的两个裸可变指针。
///
    /// 返回范围是半开区间，`end` 指向最后一个元素之后的 *一过末尾* 位置。
    /// 空切片会得到两个相等指针，两个指针的距离表示切片长度。
///
    /// 使用这些指针时请参阅 [`as_mut_ptr`] 的注意事项。`end` 不指向有效元素，不能解引用；
    /// 对范围内元素写入时还必须维护可变访问的唯一性。
///
    /// 该函数适合与使用 `[begin, end)` 两指针约定的外部接口交互，例如 C++。
    ///
    /// [`as_mut_ptr`]: slice::as_mut_ptr
    #[stable(feature = "slice_ptr_range", since = "1.48.0")]
    #[rustc_const_stable(feature = "const_ptr_offset", since = "1.61.0")]
    #[inline]
    #[must_use]
    pub const fn as_mut_ptr_range(&mut self) -> Range<*mut T> {
        let start = self.as_mut_ptr();
        // SAFETY: 见上方 as_ptr_range() 对这里调用 `add` 安全性的说明。
        let end = unsafe { start.add(self.len()) };
        start..end
    }

    /// 获取底层数组的共享引用。
///
    /// 只有当 `N` 与 `self.len()` 完全相等时才返回 `Some(&[T; N])`，否则返回 `None`。
    #[stable(feature = "core_slice_as_array", since = "1.93.0")]
    #[rustc_const_stable(feature = "core_slice_as_array", since = "1.93.0")]
    #[inline]
    #[must_use]
    pub const fn as_array<const N: usize>(&self) -> Option<&[T; N]> {
        if self.len() == N {
            let ptr = self.as_ptr().cast_array();

            // SAFETY: 已检查 `N == self.len()`，因此切片覆盖的元素序列可重新解释为
            // 同长度的实际数组 `[T; N]`，且引用生命周期不超过原切片。
            let me = unsafe { &*ptr };
            Some(me)
        } else {
            None
        }
    }

    /// 获取底层数组的可变引用。
///
    /// 只有当 `N` 与 `self.len()` 完全相等时才返回 `Some(&mut [T; N])`，否则返回 `None`。
    #[stable(feature = "core_slice_as_array", since = "1.93.0")]
    #[rustc_const_stable(feature = "core_slice_as_array", since = "1.93.0")]
    #[inline]
    #[must_use]
    pub const fn as_mut_array<const N: usize>(&mut self) -> Option<&mut [T; N]> {
        if self.len() == N {
            let ptr = self.as_mut_ptr().cast_array();

            // SAFETY: 已检查 `N == self.len()`，且 `&mut self` 保证独占访问，因此可把
            // 同长度切片重新解释为 `[T; N]` 的可变引用。
            let me = unsafe { &mut *ptr };
            Some(me)
        } else {
            None
        }
    }

    /// 交换切片中的两个元素。
///
    /// 如果 `a == b`，元素值保证不会发生变化。
///
    /// # 参数
///
    /// * a - 第一个元素的索引
    /// * b - 第二个元素的索引
    ///
    /// # Panics
    ///
    /// 如果 `a` 或 `b` 越界，本函数会 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// let mut v = ["a", "b", "c", "d", "e"];
    /// v.swap(2, 4);
    /// assert!(v == ["a", "b", "e", "d", "c"]);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_swap", since = "1.85.0")]
    #[inline]
    #[track_caller]
    pub const fn swap(&mut self, a: usize, b: usize) {
        // FIXME: 在这里使用 swap_unchecked（https://github.com/rust-lang/rust/pull/88540#issuecomment-944344343）。
        // 不能从同一个切片同时取得两个可变借用，因此改用裸指针。
        let pa = &raw mut self[a];
        let pb = &raw mut self[b];
        // SAFETY: `pa` 和 `pb` 来自安全的可变引用，指向切片内元素，因此有效且对齐。
        // 对 `a` 与 `b` 的元素访问已经经过边界检查，越界时会先 panic。
        unsafe {
            ptr::swap(pa, pb);
        }
    }

    /// 不做边界检查，交换切片中的两个元素。
    ///
    /// 安全替代方案见 [`swap`]。
    ///
    /// # 参数
    ///
    /// * a - 第一个元素的索引
    /// * b - 第二个元素的索引
    ///
    /// # 安全性(Safety）
    ///
    /// 使用越界索引调用本方法是 *[undefined behavior]*。调用方必须保证
    /// `a < self.len()` 且 `b < self.len()`；这里不会做 panic 边界检查。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(slice_swap_unchecked)]
    ///
    /// let mut v = ["a", "b", "c", "d"];
    /// // SAFETY: 我们知道 1 和 3 都是该切片内的有效索引。
    /// unsafe { v.swap_unchecked(1, 3) };
    /// assert!(v == ["a", "d", "c", "b"]);
    /// ```
    ///
    /// [`swap`]: slice::swap
    /// [undefined behavior]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    #[unstable(feature = "slice_swap_unchecked", issue = "88539")]
    #[track_caller]
    pub const unsafe fn swap_unchecked(&mut self, a: usize, b: usize) {
        assert_unsafe_precondition!(
            check_library_ub,
            "slice::swap_unchecked requires that the indices are within the slice",
            (
                len: usize = self.len(),
                a: usize = a,
                b: usize = b,
            ) => a < len && b < len,
        );

        let ptr = self.as_mut_ptr();
        // SAFETY: 调用方必须保证 `a < self.len()` 且 `b < self.len()`。
        unsafe {
            ptr::swap(ptr.add(a), ptr.add(b));
        }
    }

    /// 原地反转切片中的元素顺序。
    ///
    /// # 示例
    ///
    /// ```
    /// let mut v = [1, 2, 3];
    /// v.reverse();
    /// assert!(v == [3, 2, 1]);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_slice_reverse", since = "1.90.0")]
    #[inline]
    pub const fn reverse(&mut self) {
        let half_len = self.len() / 2;
        let Range { start, end } = self.as_mut_ptr_range();

        // 如果长度为奇数，这两个切片会跳过中间元素，因为中间元素不需要移动。
        let (front_half, back_half) =
            // SAFETY: 两者都是原切片的子区域，因此内存范围有效；它们各自至多占原切片一半，
            // 所以不会重叠。
            unsafe {
                (
                    slice::from_raw_parts_mut(start, half_len),
                    slice::from_raw_parts_mut(end.sub(half_len), half_len),
                )
            };

        // 在这里引入函数边界，可以让两个半区获得 `noalias` 标记；LLVM 因而知道它们互不重叠，
        // 相比直接在原切片上操作更利于优化。
        revswap(front_half, back_half, half_len);

        #[inline]
        const fn revswap<T>(a: &mut [T], b: &mut [T], n: usize) {
            debug_assert!(a.len() == n);
            debug_assert!(b.len() == n);

            // 这个函数会先独立编译；该检查告诉 LLVM 下面的索引位于边界内。
            // 内联之后，实际切片长度已知，这个检查会被移除。
            // FIXME(const_trait_impl): 替换为 let (a, b) = (&mut a[..n], &mut b[..n]);
            let (a, _) = a.split_at_mut(n);
            let (b, _) = b.split_at_mut(n);

            let mut i = 0;
            while i < n {
                mem::swap(&mut a[i], &mut b[n - 1 - i]);
                i += 1;
            }
        }
    }

    /// 返回遍历切片的迭代器。
///
    /// 迭代器按从头到尾的顺序产出所有元素的共享引用。
    ///
    /// # 示例
    ///
    /// ```
    /// let x = &[1, 2, 4];
    /// let mut iterator = x.iter();
    ///
    /// assert_eq!(iterator.next(), Some(&1));
    /// assert_eq!(iterator.next(), Some(&2));
    /// assert_eq!(iterator.next(), Some(&4));
    /// assert_eq!(iterator.next(), None);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_unstable(feature = "const_slice_make_iter", issue = "137737")]
    #[inline]
    #[rustc_diagnostic_item = "slice_iter"]
    pub const fn iter(&self) -> Iter<'_, T> {
        Iter::new(self)
    }

    /// 返回允许逐个修改元素的迭代器。
///
    /// 迭代器按从头到尾的顺序产出所有元素的可变引用；这些可变引用互不重叠。
    ///
    /// # 示例
    ///
    /// ```
    /// let x = &mut [1, 2, 4];
    /// for elem in x.iter_mut() {
    ///     *elem += 2;
    /// }
    /// assert_eq!(x, &[3, 4, 6]);
    /// ```
    #[rustc_const_unstable(feature = "const_slice_make_iter", issue = "137737")]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub const fn iter_mut(&mut self) -> IterMut<'_, T> {
        IterMut::new(self)
    }

    /// 返回遍历所有长度为 `size` 的连续窗口的迭代器。
    ///
    /// 相邻窗口会重叠。如果切片短于 `size`，迭代器不产生任何值。
    ///
    /// # Panics
    ///
    /// 如果 `size` 为 0，本函数会 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// let slice = ['l', 'o', 'r', 'e', 'm'];
    /// let mut iter = slice.windows(3);
    /// assert_eq!(iter.next().unwrap(), &['l', 'o', 'r']);
    /// assert_eq!(iter.next().unwrap(), &['o', 'r', 'e']);
    /// assert_eq!(iter.next().unwrap(), &['r', 'e', 'm']);
    /// assert!(iter.next().is_none());
    /// ```
    ///
    /// 当切片短于 `size` 时：
    ///
    /// ```
    /// let slice = ['f', 'o', 'o'];
    /// let mut iter = slice.windows(4);
    /// assert!(iter.next().is_none());
    /// ```
    ///
    /// 因为 [Iterator] trait 无法表达这种重叠可变窗口所需的生命周期，标准切片没有
    /// `windows_mut`。例如 `[0,1,2].windows_mut(2).collect()` 会同时持有重叠的
    /// `&mut`，违反 [the rules of references]。类似 [LendingIterator] 的抽象可以表达
    /// 这种模式；在某些场景下，也可以结合
    /// [`Cell::as_slice_of_cells`](crate::cell::Cell::as_slice_of_cells) 与 `windows` 使用。
    ///
    /// [the rules of references]: https://doc.rust-lang.org/book/ch04-02-references-and-borrowing.html#the-rules-of-references
    /// [LendingIterator]: https://blog.rust-lang.org/2022/10/28/gats-stabilization.html
    /// ```
    /// use std::cell::Cell;
    ///
    /// let mut array = ['R', 'u', 's', 't', ' ', '2', '0', '1', '5'];
    /// let slice = &mut array[..];
    /// let slice_of_cells: &[Cell<char>] = Cell::from_mut(slice).as_slice_of_cells();
    /// for w in slice_of_cells.windows(3) {
    ///     Cell::swap(&w[0], &w[2]);
    /// }
    /// assert_eq!(array, ['s', 't', ' ', '2', '0', '1', '5', 'u', 'R']);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_unstable(feature = "const_slice_make_iter", issue = "137737")]
    #[inline]
    #[track_caller]
    pub const fn windows(&self, size: usize) -> Windows<'_, T> {
        let size = NonZero::new(size).expect("window size must be non-zero");
        Windows::new(self, size)
    }

    /// 返回从切片开头开始、每次遍历 `chunk_size` 个元素的迭代器。
///
    /// 每个 chunk 都是一个不重叠的子切片。如果 `chunk_size` 不能整除切片长度，
    /// 最后一个 chunk 的长度会小于 `chunk_size`。
///
    /// 如果需要只返回长度恰好为 `chunk_size` 的 chunk，见 [`chunks_exact`]；
    /// 如果需要从切片末尾开始的同类迭代器，见 [`rchunks`]。
///
    /// 如果 `chunk_size` 是常量，考虑使用 [`as_chunks`]；它返回固定长度数组引用，
    /// 而不是普通切片引用。
    ///
    /// # Panics
    ///
    /// 如果 `chunk_size` 为 0，本函数会 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// let slice = ['l', 'o', 'r', 'e', 'm'];
    /// let mut iter = slice.chunks(2);
    /// assert_eq!(iter.next().unwrap(), &['l', 'o']);
    /// assert_eq!(iter.next().unwrap(), &['r', 'e']);
    /// assert_eq!(iter.next().unwrap(), &['m']);
    /// assert!(iter.next().is_none());
    /// ```
    ///
    /// [`chunks_exact`]: slice::chunks_exact
    /// [`rchunks`]: slice::rchunks
    /// [`as_chunks`]: slice::as_chunks
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_unstable(feature = "const_slice_make_iter", issue = "137737")]
    #[inline]
    #[track_caller]
    pub const fn chunks(&self, chunk_size: usize) -> Chunks<'_, T> {
        assert!(chunk_size != 0, "chunk size must be non-zero");
        Chunks::new(self, chunk_size)
    }

    /// 返回从切片开头开始、每次遍历 `chunk_size` 个元素的可变迭代器。
///
    /// 每个 chunk 都是互不重叠的可变子切片。如果 `chunk_size` 不能整除切片长度，
    /// 最后一个 chunk 的长度会小于 `chunk_size`。
///
    /// 如果需要只返回长度恰好为 `chunk_size` 的可变 chunk，见 [`chunks_exact_mut`]；
    /// 如果需要从切片末尾开始的同类迭代器，见 [`rchunks_mut`]。
///
    /// 如果 `chunk_size` 是常量，考虑使用 [`as_chunks_mut`]；它返回固定长度数组的可变引用，
    /// 而不是普通切片引用。
    ///
    /// # Panics
    ///
    /// 如果 `chunk_size` 为 0，本函数会 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// let v = &mut [0, 0, 0, 0, 0];
    /// let mut count = 1;
    ///
    /// for chunk in v.chunks_mut(2) {
    ///     for elem in chunk.iter_mut() {
    ///         *elem += count;
    ///     }
    ///     count += 1;
    /// }
    /// assert_eq!(v, &[1, 1, 2, 2, 3]);
    /// ```
    ///
    /// [`chunks_exact_mut`]: slice::chunks_exact_mut
    /// [`rchunks_mut`]: slice::rchunks_mut
    /// [`as_chunks_mut`]: slice::as_chunks_mut
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_unstable(feature = "const_slice_make_iter", issue = "137737")]
    #[inline]
    #[track_caller]
    pub const fn chunks_mut(&mut self, chunk_size: usize) -> ChunksMut<'_, T> {
        assert!(chunk_size != 0, "chunk size must be non-zero");
        ChunksMut::new(self, chunk_size)
    }

    /// 返回从切片开头开始、每次遍历 `chunk_size` 个元素的迭代器。
    ///
    /// 每个 chunk 都是互不重叠的子切片。如果 `chunk_size` 不能整除切片长度，
    /// 最后最多 `chunk_size-1` 个元素会被省略，并可通过迭代器的 `remainder`
    /// 函数取回。
    ///
    /// 由于每个 chunk 的长度都恰好是 `chunk_size`，编译器通常能比 [`chunks`]
    /// 的情况更好地优化生成代码。
    ///
    /// 若需要把 remainder 也作为较小 chunk 返回，见 [`chunks`]；若需要从切片末尾
    /// 开始的同类迭代器，见 [`rchunks_exact`]。
    ///
    /// 如果 `chunk_size` 是常量，考虑使用 [`as_chunks`]；它返回固定长度数组引用，
    /// 而不是普通切片引用。
    ///
    /// # Panics
    ///
    /// 如果 `chunk_size` 为 0，本函数会 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// let slice = ['l', 'o', 'r', 'e', 'm'];
    /// let mut iter = slice.chunks_exact(2);
    /// assert_eq!(iter.next().unwrap(), &['l', 'o']);
    /// assert_eq!(iter.next().unwrap(), &['r', 'e']);
    /// assert!(iter.next().is_none());
    /// assert_eq!(iter.remainder(), &['m']);
    /// ```
    ///
    /// [`chunks`]: slice::chunks
    /// [`rchunks_exact`]: slice::rchunks_exact
    /// [`as_chunks`]: slice::as_chunks
    #[stable(feature = "chunks_exact", since = "1.31.0")]
    #[rustc_const_unstable(feature = "const_slice_make_iter", issue = "137737")]
    #[inline]
    #[track_caller]
    pub const fn chunks_exact(&self, chunk_size: usize) -> ChunksExact<'_, T> {
        assert!(chunk_size != 0, "chunk size must be non-zero");
        ChunksExact::new(self, chunk_size)
    }

    /// 返回从切片开头开始、每次遍历 `chunk_size` 个元素的可变迭代器。
    ///
    /// 每个 chunk 都是互不重叠的可变子切片。如果 `chunk_size` 不能整除切片长度，
    /// 最后最多 `chunk_size-1` 个元素会被省略，并可通过迭代器的 `into_remainder`
    /// 函数取回。
    ///
    /// 由于每个 chunk 的长度都恰好是 `chunk_size`，编译器通常能比 [`chunks_mut`]
    /// 的情况更好地优化生成代码。
    ///
    /// 若需要把 remainder 也作为较小 chunk 返回，见 [`chunks_mut`]；若需要从切片末尾
    /// 开始的同类迭代器，见 [`rchunks_exact_mut`]。
    ///
    /// 如果 `chunk_size` 是常量，考虑使用 [`as_chunks_mut`]；它返回固定长度数组的可变引用，
    /// 而不是普通切片引用。
    ///
    /// # Panics
    ///
    /// 如果 `chunk_size` 为 0，本函数会 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// let v = &mut [0, 0, 0, 0, 0];
    /// let mut count = 1;
    ///
    /// for chunk in v.chunks_exact_mut(2) {
    ///     for elem in chunk.iter_mut() {
    ///         *elem += count;
    ///     }
    ///     count += 1;
    /// }
    /// assert_eq!(v, &[1, 1, 2, 2, 0]);
    /// ```
    ///
    /// [`chunks_mut`]: slice::chunks_mut
    /// [`rchunks_exact_mut`]: slice::rchunks_exact_mut
    /// [`as_chunks_mut`]: slice::as_chunks_mut
    #[stable(feature = "chunks_exact", since = "1.31.0")]
    #[rustc_const_unstable(feature = "const_slice_make_iter", issue = "137737")]
    #[inline]
    #[track_caller]
    pub const fn chunks_exact_mut(&mut self, chunk_size: usize) -> ChunksExactMut<'_, T> {
        assert!(chunk_size != 0, "chunk size must be non-zero");
        ChunksExactMut::new(self, chunk_size)
    }

    /// 在假设没有 remainder 的前提下，把切片拆成 `N` 元素数组切片。
    ///
    /// 这是 [`as_flattened`] 的逆操作。
    ///
    /// [`as_flattened`]: slice::as_flattened
    ///
    /// 由于它是 `unsafe`，应考虑是否可改用 [`as_chunks`] 或 [`as_rchunks`]，
    /// 例如通过类似下面的写法：
    /// `if let (chunks, []) = slice.as_chunks()` 或
    /// `let (chunks, []) = slice.as_chunks() else { unreachable!() };`.
    ///
    /// [`as_chunks`]: slice::as_chunks
    /// [`as_rchunks`]: slice::as_rchunks
    ///
    /// # 安全性(Safety）
    ///
    /// 只能在满足以下条件时调用：
    /// - 切片能被精确分成 `N` 元素 chunk（即 `self.len() % N == 0`）。
    /// - `N != 0`。
    ///
    /// # 示例
    ///
    /// ```
    /// let slice: &[char] = &['l', 'o', 'r', 'e', 'm', '!'];
    /// let chunks: &[[char; 1]] =
    ///     // SAFETY: 1 元素 chunk 永远没有 remainder。
    ///     unsafe { slice.as_chunks_unchecked() };
    /// assert_eq!(chunks, &[['l'], ['o'], ['r'], ['e'], ['m'], ['!']]);
    /// let chunks: &[[char; 3]] =
    ///     // SAFETY: 切片长度 (6) 是 3 的倍数。
    ///     unsafe { slice.as_chunks_unchecked() };
    /// assert_eq!(chunks, &[['l', 'o', 'r'], ['e', 'm', '!']]);
    ///
    /// // 这些调用都是不健全的：
    /// // let chunks: &[[_; 5]] = slice.as_chunks_unchecked() // 切片长度不是 5 的倍数。
    /// // let chunks: &[[_; 0]] = slice.as_chunks_unchecked() // 永远不允许零长度 chunk。
    /// ```
    #[stable(feature = "slice_as_chunks", since = "1.88.0")]
    #[rustc_const_stable(feature = "slice_as_chunks", since = "1.88.0")]
    #[inline]
    #[must_use]
    #[track_caller]
    pub const unsafe fn as_chunks_unchecked<const N: usize>(&self) -> &[[T; N]] {
        assert_unsafe_precondition!(
            check_language_ub,
            "slice::as_chunks_unchecked requires `N != 0` and the slice to split exactly into `N`-element chunks",
            (n: usize = N, len: usize = self.len()) => n != 0 && len.is_multiple_of(n),
        );
        // SAFETY: 调用方必须保证 `N` 非零，并且能整除切片长度。
        let new_len = unsafe { exact_div(self.len(), N) };
        // SAFETY: 把一个包含 `new_len * N` 个元素的切片转换成
        // 一个包含 `new_len` 个 `N` 元素 chunk 的切片。
        unsafe { from_raw_parts(self.as_ptr().cast(), new_len) }
    }

    /// 从切片开头开始，把切片拆成 `N` 元素数组切片和一个长度严格小于 `N` 的 remainder 切片。
    ///
    /// remainder 在除法意义上有定义。给定
    /// `let (chunks, remainder) = slice.as_chunks()`，则：
    /// - `chunks.len()` 等于 `slice.len() / N`；
    /// - `remainder.len()` 等于 `slice.len() % N`；
    /// - `slice.len()` 等于 `chunks.len() * N + remainder.len()`。
    ///
    /// 可使用 [`as_flattened`] 把 chunk 重新展平成 `T` 切片。
    ///
    /// [`as_flattened`]: slice::as_flattened
    ///
    /// # Panics
    ///
    /// 如果 `N` 为零则 panic。
    ///
    /// 注意，该检查针对的是 const generic 参数，而不是运行时值；
    /// 因此某个特定单态化要么总是 panic，要么永不 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// let slice = ['l', 'o', 'r', 'e', 'm'];
    /// let (chunks, remainder) = slice.as_chunks();
    /// assert_eq!(chunks, &[['l', 'o'], ['r', 'e']]);
    /// assert_eq!(remainder, &['m']);
    /// ```
    ///
    /// 如果预期切片长度正好是倍数，可将 `let`-`else` 与空切片模式结合使用：
    /// ```
    /// let slice = ['R', 'u', 's', 't'];
    /// let (chunks, []) = slice.as_chunks::<2>() else {
    ///     panic!("slice didn't have even length")
    /// };
    /// assert_eq!(chunks, &[['R', 'u'], ['s', 't']]);
    /// ```
    #[stable(feature = "slice_as_chunks", since = "1.88.0")]
    #[rustc_const_stable(feature = "slice_as_chunks", since = "1.88.0")]
    #[inline]
    #[track_caller]
    #[must_use]
    pub const fn as_chunks<const N: usize>(&self) -> (&[[T; N]], &[T]) {
        assert!(N != 0, "chunk size must be non-zero");
        let len_rounded_down = self.len() / N * N;
        // SAFETY: 向下取整后的值始终小于或等于原始长度，因此必定位于切片边界内。
        let (multiple_of_n, remainder) = unsafe { self.split_at_unchecked(len_rounded_down) };
        // SAFETY: 已经对零值 panic，并通过构造保证子切片长度是 N 的倍数。
        let array_slice = unsafe { multiple_of_n.as_chunks_unchecked() };
        (array_slice, remainder)
    }

    /// 从切片末尾开始，把切片拆成 `N` 元素数组切片和一个长度严格小于 `N` 的 remainder 切片。
    ///
    /// remainder 在除法意义上有定义。给定
    /// `let (remainder, chunks) = slice.as_rchunks()`，则：
    /// - `remainder.len()` 等于 `slice.len() % N`；
    /// - `chunks.len()` 等于 `slice.len() / N`；
    /// - `slice.len()` 等于 `chunks.len() * N + remainder.len()`。
    ///
    /// 可使用 [`as_flattened`] 把 chunk 重新展平成 `T` 切片。
    ///
    /// [`as_flattened`]: slice::as_flattened
    ///
    /// # Panics
    ///
    /// 如果 `N` 为零则 panic。
    ///
    /// 注意，该检查针对的是 const generic 参数，而不是运行时值；
    /// 因此某个特定单态化要么总是 panic，要么永不 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// let slice = ['l', 'o', 'r', 'e', 'm'];
    /// let (remainder, chunks) = slice.as_rchunks();
    /// assert_eq!(remainder, &['l']);
    /// assert_eq!(chunks, &[['o', 'r'], ['e', 'm']]);
    /// ```
    #[stable(feature = "slice_as_chunks", since = "1.88.0")]
    #[rustc_const_stable(feature = "slice_as_chunks", since = "1.88.0")]
    #[inline]
    #[track_caller]
    #[must_use]
    pub const fn as_rchunks<const N: usize>(&self) -> (&[T], &[[T; N]]) {
        assert!(N != 0, "chunk size must be non-zero");
        let len = self.len() / N;
        let (remainder, multiple_of_n) = self.split_at(self.len() - len * N);
        // SAFETY: 已经对零值 panic，并通过构造保证子切片长度是 N 的倍数。
        let array_slice = unsafe { multiple_of_n.as_chunks_unchecked() };
        (remainder, array_slice)
    }

    /// 在假设没有 remainder 的前提下，把切片拆成 `N` 元素数组切片。
    ///
    /// 这是 [`as_flattened_mut`] 的逆操作。
    ///
    /// [`as_flattened_mut`]: slice::as_flattened_mut
    ///
    /// 由于它是 `unsafe`，应考虑是否可改用 [`as_chunks_mut`] 或 [`as_rchunks_mut`]，
    /// 例如通过类似下面的写法：
    /// `if let (chunks, []) = slice.as_chunks_mut()` 或
    /// `let (chunks, []) = slice.as_chunks_mut() else { unreachable!() };`.
    ///
    /// [`as_chunks_mut`]: slice::as_chunks_mut
    /// [`as_rchunks_mut`]: slice::as_rchunks_mut
    ///
    /// # 安全性(Safety）
    ///
    /// 只能在满足以下条件时调用：
    /// - 切片能被精确分成 `N` 元素 chunk（即 `self.len() % N == 0`）。
    /// - `N != 0`。
    ///
    /// # 示例
    ///
    /// ```
    /// let slice: &mut [char] = &mut ['l', 'o', 'r', 'e', 'm', '!'];
    /// let chunks: &mut [[char; 1]] =
    ///     // SAFETY: 1 元素 chunk 永远没有 remainder。
    ///     unsafe { slice.as_chunks_unchecked_mut() };
    /// chunks[0] = ['L'];
    /// assert_eq!(chunks, &[['L'], ['o'], ['r'], ['e'], ['m'], ['!']]);
    /// let chunks: &mut [[char; 3]] =
    ///     // SAFETY: 切片长度 (6) 是 3 的倍数。
    ///     unsafe { slice.as_chunks_unchecked_mut() };
    /// chunks[1] = ['a', 'x', '?'];
    /// assert_eq!(slice, &['L', 'o', 'r', 'a', 'x', '?']);
    ///
    /// // 这些调用都是不健全的：
    /// // let chunks: &[[_; 5]] = slice.as_chunks_unchecked_mut() // 切片长度不是 5 的倍数。
    /// // let chunks: &[[_; 0]] = slice.as_chunks_unchecked_mut() // 永远不允许零长度 chunk。
    /// ```
    #[stable(feature = "slice_as_chunks", since = "1.88.0")]
    #[rustc_const_stable(feature = "slice_as_chunks", since = "1.88.0")]
    #[inline]
    #[must_use]
    #[track_caller]
    pub const unsafe fn as_chunks_unchecked_mut<const N: usize>(&mut self) -> &mut [[T; N]] {
        assert_unsafe_precondition!(
            check_language_ub,
            "slice::as_chunks_unchecked requires `N != 0` and the slice to split exactly into `N`-element chunks",
            (n: usize = N, len: usize = self.len()) => n != 0 && len.is_multiple_of(n)
        );
        // SAFETY: 调用方必须保证 `N` 非零，并且能整除切片长度。
        let new_len = unsafe { exact_div(self.len(), N) };
        // SAFETY: 把一个包含 `new_len * N` 个元素的切片转换成
        // 一个包含 `new_len` 个 `N` 元素 chunk 的切片。
        unsafe { from_raw_parts_mut(self.as_mut_ptr().cast(), new_len) }
    }

    /// 从切片开头开始，把切片拆成 `N` 元素数组切片和一个长度严格小于 `N` 的
    /// remainder 切片。
    ///
    /// remainder 在除法意义上有定义。给定
    /// `let (chunks, remainder) = slice.as_chunks_mut()`，则：
    /// - `chunks.len()` 等于 `slice.len() / N`；
    /// - `remainder.len()` 等于 `slice.len() % N`；
    /// - `slice.len()` 等于 `chunks.len() * N + remainder.len()`。
    ///
    /// 可使用 [`as_flattened_mut`] 把 chunk 重新展平成 `T` 切片。
    ///
    /// [`as_flattened_mut`]: slice::as_flattened_mut
    ///
    /// # Panics
    ///
    /// 如果 `N` 为零则 panic。
    ///
    /// 注意，该检查针对的是 const generic 参数，而不是运行时值；
    /// 因此某个特定单态化要么总是 panic，要么永不 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// let v = &mut [0, 0, 0, 0, 0];
    /// let mut count = 1;
    ///
    /// let (chunks, remainder) = v.as_chunks_mut();
    /// remainder[0] = 9;
    /// for chunk in chunks {
    ///     *chunk = [count; 2];
    ///     count += 1;
    /// }
    /// assert_eq!(v, &[1, 1, 2, 2, 9]);
    /// ```
    #[stable(feature = "slice_as_chunks", since = "1.88.0")]
    #[rustc_const_stable(feature = "slice_as_chunks", since = "1.88.0")]
    #[inline]
    #[track_caller]
    #[must_use]
    pub const fn as_chunks_mut<const N: usize>(&mut self) -> (&mut [[T; N]], &mut [T]) {
        assert!(N != 0, "chunk size must be non-zero");
        let len_rounded_down = self.len() / N * N;
        // SAFETY: 向下取整后的值始终小于或等于原始长度，因此必定位于切片边界内。
        let (multiple_of_n, remainder) = unsafe { self.split_at_mut_unchecked(len_rounded_down) };
        // SAFETY: 已经对零值 panic，并通过构造保证子切片长度是 N 的倍数。
        let array_slice = unsafe { multiple_of_n.as_chunks_unchecked_mut() };
        (array_slice, remainder)
    }

    /// 从切片末尾开始，把切片拆成 `N` 元素数组切片和一个长度严格小于 `N` 的
    /// remainder 切片。
    ///
    /// remainder 在除法意义上有定义。给定
    /// `let (remainder, chunks) = slice.as_rchunks_mut()`，则：
    /// - `remainder.len()` 等于 `slice.len() % N`；
    /// - `chunks.len()` 等于 `slice.len() / N`；
    /// - `slice.len()` 等于 `chunks.len() * N + remainder.len()`。
    ///
    /// 可使用 [`as_flattened_mut`] 把 chunk 重新展平成 `T` 切片。
    ///
    /// [`as_flattened_mut`]: slice::as_flattened_mut
    ///
    /// # Panics
    ///
    /// 如果 `N` 为零则 panic。
    ///
    /// 注意，该检查针对的是 const generic 参数，而不是运行时值；
    /// 因此某个特定单态化要么总是 panic，要么永不 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// let v = &mut [0, 0, 0, 0, 0];
    /// let mut count = 1;
    ///
    /// let (remainder, chunks) = v.as_rchunks_mut();
    /// remainder[0] = 9;
    /// for chunk in chunks {
    ///     *chunk = [count; 2];
    ///     count += 1;
    /// }
    /// assert_eq!(v, &[9, 1, 1, 2, 2]);
    /// ```
    #[stable(feature = "slice_as_chunks", since = "1.88.0")]
    #[rustc_const_stable(feature = "slice_as_chunks", since = "1.88.0")]
    #[inline]
    #[track_caller]
    #[must_use]
    pub const fn as_rchunks_mut<const N: usize>(&mut self) -> (&mut [T], &mut [[T; N]]) {
        assert!(N != 0, "chunk size must be non-zero");
        let len = self.len() / N;
        let (remainder, multiple_of_n) = self.split_at_mut(self.len() - len * N);
        // SAFETY: 已经对零值 panic，并通过构造保证子切片长度是 N 的倍数。
        let array_slice = unsafe { multiple_of_n.as_chunks_unchecked_mut() };
        (remainder, array_slice)
    }

    /// 返回从切片开头开始、遍历所有重叠 `N` 元素窗口的迭代器。
    ///
    /// 这是 [`windows`] 的 const generic 等价版本。
    ///
    /// 如果 `N` 大于切片长度，则不返回任何窗口。
    ///
    /// # Panics
    ///
    /// 如果 `N` 为零则 panic。
    ///
    /// 注意，该检查针对的是 const generic 参数，而不是运行时值；
    /// 因此某个特定单态化要么总是 panic，要么永不 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// let slice = [0, 1, 2, 3];
    /// let mut iter = slice.array_windows();
    /// assert_eq!(iter.next().unwrap(), &[0, 1]);
    /// assert_eq!(iter.next().unwrap(), &[1, 2]);
    /// assert_eq!(iter.next().unwrap(), &[2, 3]);
    /// assert!(iter.next().is_none());
    /// ```
    ///
    /// [`windows`]: slice::windows
    #[stable(feature = "array_windows", since = "1.94.0")]
    #[rustc_const_unstable(feature = "const_slice_make_iter", issue = "137737")]
    #[inline]
    #[track_caller]
    pub const fn array_windows<const N: usize>(&self) -> ArrayWindows<'_, T, N> {
        assert!(N != 0, "window size must be non-zero");
        ArrayWindows::new(self)
    }

    /// 返回从切片末尾开始、每次遍历 `chunk_size` 个元素的迭代器。
    ///
    /// 每个 chunk 都是互不重叠的子切片。如果 `chunk_size` 不能整除切片长度，
    /// 最后一个 chunk 的长度会小于 `chunk_size`。
    ///
    /// 若需要只返回长度恰好为 `chunk_size` 的 chunk，见 [`rchunks_exact`]；
    /// 若需要从切片开头开始的同类迭代器，见 [`chunks`]。
    ///
    /// 如果 `chunk_size` 是常量，考虑使用 [`as_rchunks`]；它返回固定长度数组引用，
    /// 而不是普通切片引用。
    ///
    /// # Panics
    ///
    /// 如果 `chunk_size` 为 0，本函数会 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// let slice = ['l', 'o', 'r', 'e', 'm'];
    /// let mut iter = slice.rchunks(2);
    /// assert_eq!(iter.next().unwrap(), &['e', 'm']);
    /// assert_eq!(iter.next().unwrap(), &['o', 'r']);
    /// assert_eq!(iter.next().unwrap(), &['l']);
    /// assert!(iter.next().is_none());
    /// ```
    ///
    /// [`rchunks_exact`]: slice::rchunks_exact
    /// [`chunks`]: slice::chunks
    /// [`as_rchunks`]: slice::as_rchunks
    #[stable(feature = "rchunks", since = "1.31.0")]
    #[rustc_const_unstable(feature = "const_slice_make_iter", issue = "137737")]
    #[inline]
    #[track_caller]
    pub const fn rchunks(&self, chunk_size: usize) -> RChunks<'_, T> {
        assert!(chunk_size != 0, "chunk size must be non-zero");
        RChunks::new(self, chunk_size)
    }

    /// 返回从切片末尾开始、每次遍历 `chunk_size` 个元素的可变迭代器。
    ///
    /// 每个 chunk 都是互不重叠的可变子切片。如果 `chunk_size` 不能整除切片长度，
    /// 最后一个 chunk 的长度会小于 `chunk_size`。
    ///
    /// 若需要只返回长度恰好为 `chunk_size` 的可变 chunk，见 [`rchunks_exact_mut`]；
    /// 若需要从切片开头开始的同类迭代器，见 [`chunks_mut`]。
    ///
    /// 如果 `chunk_size` 是常量，考虑使用 [`as_rchunks_mut`]；它返回固定长度数组的可变引用，
    /// 而不是普通切片引用。
    ///
    /// # Panics
    ///
    /// 如果 `chunk_size` 为 0，本函数会 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// let v = &mut [0, 0, 0, 0, 0];
    /// let mut count = 1;
    ///
    /// for chunk in v.rchunks_mut(2) {
    ///     for elem in chunk.iter_mut() {
    ///         *elem += count;
    ///     }
    ///     count += 1;
    /// }
    /// assert_eq!(v, &[3, 2, 2, 1, 1]);
    /// ```
    ///
    /// [`rchunks_exact_mut`]: slice::rchunks_exact_mut
    /// [`chunks_mut`]: slice::chunks_mut
    /// [`as_rchunks_mut`]: slice::as_rchunks_mut
    #[stable(feature = "rchunks", since = "1.31.0")]
    #[rustc_const_unstable(feature = "const_slice_make_iter", issue = "137737")]
    #[inline]
    #[track_caller]
    pub const fn rchunks_mut(&mut self, chunk_size: usize) -> RChunksMut<'_, T> {
        assert!(chunk_size != 0, "chunk size must be non-zero");
        RChunksMut::new(self, chunk_size)
    }

    /// 返回从切片末尾开始、每次遍历 `chunk_size` 个元素的迭代器。
    ///
    /// 每个 chunk 都是互不重叠的子切片。如果 `chunk_size` 不能整除切片长度，
    /// 最后最多 `chunk_size-1` 个元素会被省略，并可通过迭代器的 `remainder`
    /// 函数取回。
    ///
    /// 由于每个 chunk 的长度都恰好是 `chunk_size`，编译器通常能比 [`rchunks`]
    /// 的情况更好地优化生成代码。
    ///
    /// 若需要把 remainder 也作为较小 chunk 返回，见 [`rchunks`]；若需要从切片开头
    /// 开始的同类迭代器，见 [`chunks_exact`]。
    ///
    /// 如果 `chunk_size` 是常量，考虑使用 [`as_rchunks`]；它返回固定长度数组引用，
    /// 而不是普通切片引用。
    ///
    /// # Panics
    ///
    /// 如果 `chunk_size` 为 0，本函数会 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// let slice = ['l', 'o', 'r', 'e', 'm'];
    /// let mut iter = slice.rchunks_exact(2);
    /// assert_eq!(iter.next().unwrap(), &['e', 'm']);
    /// assert_eq!(iter.next().unwrap(), &['o', 'r']);
    /// assert!(iter.next().is_none());
    /// assert_eq!(iter.remainder(), &['l']);
    /// ```
    ///
    /// [`chunks`]: slice::chunks
    /// [`rchunks`]: slice::rchunks
    /// [`chunks_exact`]: slice::chunks_exact
    /// [`as_rchunks`]: slice::as_rchunks
    #[stable(feature = "rchunks", since = "1.31.0")]
    #[rustc_const_unstable(feature = "const_slice_make_iter", issue = "137737")]
    #[inline]
    #[track_caller]
    pub const fn rchunks_exact(&self, chunk_size: usize) -> RChunksExact<'_, T> {
        assert!(chunk_size != 0, "chunk size must be non-zero");
        RChunksExact::new(self, chunk_size)
    }

    /// 返回从切片末尾开始、每次遍历 `chunk_size` 个元素的可变迭代器。
    ///
    /// 每个 chunk 都是互不重叠的可变子切片。如果 `chunk_size` 不能整除切片长度，
    /// 最后最多 `chunk_size-1` 个元素会被省略，并可通过迭代器的 `into_remainder`
    /// 函数取回。
    ///
    /// 由于每个 chunk 的长度都恰好是 `chunk_size`，编译器通常能比 [`chunks_mut`]
    /// 的情况更好地优化生成代码。
    ///
    /// 若需要把 remainder 也作为较小 chunk 返回，见 [`rchunks_mut`]；若需要从切片开头
    /// 开始的同类迭代器，见 [`chunks_exact_mut`]。
    ///
    /// 如果 `chunk_size` 是常量，考虑使用 [`as_rchunks_mut`]；它返回固定长度数组的可变引用，
    /// 而不是普通切片引用。
    ///
    /// # Panics
    ///
    /// 如果 `chunk_size` 为 0，本函数会 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// let v = &mut [0, 0, 0, 0, 0];
    /// let mut count = 1;
    ///
    /// for chunk in v.rchunks_exact_mut(2) {
    ///     for elem in chunk.iter_mut() {
    ///         *elem += count;
    ///     }
    ///     count += 1;
    /// }
    /// assert_eq!(v, &[0, 2, 2, 1, 1]);
    /// ```
    ///
    /// [`chunks_mut`]: slice::chunks_mut
    /// [`rchunks_mut`]: slice::rchunks_mut
    /// [`chunks_exact_mut`]: slice::chunks_exact_mut
    /// [`as_rchunks_mut`]: slice::as_rchunks_mut
    #[stable(feature = "rchunks", since = "1.31.0")]
    #[rustc_const_unstable(feature = "const_slice_make_iter", issue = "137737")]
    #[inline]
    #[track_caller]
    pub const fn rchunks_exact_mut(&mut self, chunk_size: usize) -> RChunksExactMut<'_, T> {
        assert!(chunk_size != 0, "chunk size must be non-zero");
        RChunksExactMut::new(self, chunk_size)
    }

    /// 返回一个迭代器，它按谓词分隔切片，产出互不重叠的连续 run。
    ///
    /// 谓词会对每一对相邻元素调用：先传入 `slice[0]` 与 `slice[1]`，
    /// 然后是 `slice[1]` 与 `slice[2]`，依此类推。
    ///
    /// # 示例
    ///
    /// ```
    /// let slice = &[1, 1, 1, 3, 3, 2, 2, 2];
    ///
    /// let mut iter = slice.chunk_by(|a, b| a == b);
    ///
    /// assert_eq!(iter.next(), Some(&[1, 1, 1][..]));
    /// assert_eq!(iter.next(), Some(&[3, 3][..]));
    /// assert_eq!(iter.next(), Some(&[2, 2, 2][..]));
    /// assert_eq!(iter.next(), None);
    /// ```
    ///
    /// 该方法可用于提取已排序的子切片：
    ///
    /// ```
    /// let slice = &[1, 1, 2, 3, 2, 3, 2, 3, 4];
    ///
    /// let mut iter = slice.chunk_by(|a, b| a <= b);
    ///
    /// assert_eq!(iter.next(), Some(&[1, 1, 2, 3][..]));
    /// assert_eq!(iter.next(), Some(&[2, 3][..]));
    /// assert_eq!(iter.next(), Some(&[2, 3, 4][..]));
    /// assert_eq!(iter.next(), None);
    /// ```
    #[stable(feature = "slice_group_by", since = "1.77.0")]
    #[rustc_const_unstable(feature = "const_slice_make_iter", issue = "137737")]
    #[inline]
    pub const fn chunk_by<F>(&self, pred: F) -> ChunkBy<'_, T, F>
    where
        F: FnMut(&T, &T) -> bool,
    {
        ChunkBy::new(self, pred)
    }

    /// 返回一个迭代器，它按谓词分隔切片，产出互不重叠的可变连续 run。
    ///
    /// 谓词会对每一对相邻元素调用：先传入 `slice[0]` 与 `slice[1]`，
    /// 然后是 `slice[1]` 与 `slice[2]`，依此类推。
    ///
    /// # 示例
    ///
    /// ```
    /// let slice = &mut [1, 1, 1, 3, 3, 2, 2, 2];
    ///
    /// let mut iter = slice.chunk_by_mut(|a, b| a == b);
    ///
    /// assert_eq!(iter.next(), Some(&mut [1, 1, 1][..]));
    /// assert_eq!(iter.next(), Some(&mut [3, 3][..]));
    /// assert_eq!(iter.next(), Some(&mut [2, 2, 2][..]));
    /// assert_eq!(iter.next(), None);
    /// ```
    ///
    /// 该方法可用于提取已排序的子切片：
    ///
    /// ```
    /// let slice = &mut [1, 1, 2, 3, 2, 3, 2, 3, 4];
    ///
    /// let mut iter = slice.chunk_by_mut(|a, b| a <= b);
    ///
    /// assert_eq!(iter.next(), Some(&mut [1, 1, 2, 3][..]));
    /// assert_eq!(iter.next(), Some(&mut [2, 3][..]));
    /// assert_eq!(iter.next(), Some(&mut [2, 3, 4][..]));
    /// assert_eq!(iter.next(), None);
    /// ```
    #[stable(feature = "slice_group_by", since = "1.77.0")]
    #[rustc_const_unstable(feature = "const_slice_make_iter", issue = "137737")]
    #[inline]
    pub const fn chunk_by_mut<F>(&mut self, pred: F) -> ChunkByMut<'_, T, F>
    where
        F: FnMut(&T, &T) -> bool,
    {
        ChunkByMut::new(self, pred)
    }

    /// 在指定索引处把一个切片分成两个共享切片。
///
    /// 第一个切片包含 `[0, mid)` 的所有索引，不包含 `mid` 本身；第二个切片包含
    /// `[mid, len)` 的所有索引，不包含 `len` 本身。`mid == 0` 或 `mid == len`
    /// 时其中一侧为空切片。
    ///
    /// # Panics
    ///
    /// 如果 `mid > len`，本函数会 panic。非 panic 替代方案见
    /// [`split_at_checked`](slice::split_at_checked)。
    ///
    /// # 示例
    ///
    /// ```
    /// let v = ['a', 'b', 'c'];
    ///
    /// {
    ///    let (left, right) = v.split_at(0);
    ///    assert_eq!(left, []);
    ///    assert_eq!(right, ['a', 'b', 'c']);
    /// }
    ///
    /// {
    ///     let (left, right) = v.split_at(2);
    ///     assert_eq!(left, ['a', 'b']);
    ///     assert_eq!(right, ['c']);
    /// }
    ///
    /// {
    ///     let (left, right) = v.split_at(3);
    ///     assert_eq!(left, ['a', 'b', 'c']);
    ///     assert_eq!(right, []);
    /// }
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_slice_split_at_not_mut", since = "1.71.0")]
    #[inline]
    #[track_caller]
    #[must_use]
    pub const fn split_at(&self, mid: usize) -> (&[T], &[T]) {
        match self.split_at_checked(mid) {
            Some(pair) => pair,
            None => panic!("mid > len"),
        }
    }

    /// 在指定索引处把一个可变切片分成两个互不重叠的可变切片。
///
    /// 第一个切片包含 `[0, mid)` 的所有索引，不包含 `mid` 本身；第二个切片包含
    /// `[mid, len)` 的所有索引，不包含 `len` 本身。两个返回值覆盖原切片的不同区域，
    /// 因而可以同时作为 `&mut [T]` 暴露。
    ///
    /// # Panics
    ///
    /// 如果 `mid > len`，本函数会 panic。非 panic 替代方案见
    /// [`split_at_mut_checked`](slice::split_at_mut_checked)。
    ///
    /// # 示例
    ///
    /// ```
    /// let mut v = [1, 0, 3, 0, 5, 6];
    /// let (left, right) = v.split_at_mut(2);
    /// assert_eq!(left, [1, 0]);
    /// assert_eq!(right, [3, 0, 5, 6]);
    /// left[1] = 2;
    /// right[1] = 4;
    /// assert_eq!(v, [1, 2, 3, 4, 5, 6]);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    #[track_caller]
    #[must_use]
    #[rustc_const_stable(feature = "const_slice_split_at_mut", since = "1.83.0")]
    pub const fn split_at_mut(&mut self, mid: usize) -> (&mut [T], &mut [T]) {
        match self.split_at_mut_checked(mid) {
            Some(pair) => pair,
            None => panic!("mid > len"),
        }
    }

    /// 不做边界检查，在指定索引处把一个切片分成两个共享切片。
///
    /// 第一个切片包含 `[0, mid)` 的所有索引，不包含 `mid` 本身；第二个切片包含
    /// `[mid, len)` 的所有索引，不包含 `len` 本身。
///
    /// 安全替代方案见 [`split_at`]。
    ///
    /// # 安全性(Safety）
    ///
    /// 使用越界索引调用本方法是 *[undefined behavior]*，即使得到的引用之后没有被使用。
    /// 调用方必须保证 `mid <= self.len()`。由于 `mid` 是 `usize`，`0 <= mid` 自动成立。
    /// 这个不变量同时保证 `ptr.add(mid)` 位于同一 allocation 内，且右侧长度
    /// `self.len() - mid` 不会下溢。
    ///
    /// [`split_at`]: slice::split_at
    /// [undefined behavior]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    ///
    /// # 示例
    ///
    /// ```
    /// let v = ['a', 'b', 'c'];
    ///
    /// unsafe {
    ///    let (left, right) = v.split_at_unchecked(0);
    ///    assert_eq!(left, []);
    ///    assert_eq!(right, ['a', 'b', 'c']);
    /// }
    ///
    /// unsafe {
    ///     let (left, right) = v.split_at_unchecked(2);
    ///     assert_eq!(left, ['a', 'b']);
    ///     assert_eq!(right, ['c']);
    /// }
    ///
    /// unsafe {
    ///     let (left, right) = v.split_at_unchecked(3);
    ///     assert_eq!(left, ['a', 'b', 'c']);
    ///     assert_eq!(right, []);
    /// }
    /// ```
    #[stable(feature = "slice_split_at_unchecked", since = "1.79.0")]
    #[rustc_const_stable(feature = "const_slice_split_at_unchecked", since = "1.77.0")]
    #[inline]
    #[must_use]
    #[track_caller]
    pub const unsafe fn split_at_unchecked(&self, mid: usize) -> (&[T], &[T]) {
        // FIXME(const-hack): 为了让本函数成为 const fn，这里使用 const 版本的
        // `from_raw_parts`；此前实现使用 `(self.get_unchecked(..mid), self.get_unchecked(mid..))`。

        let len = self.len();
        let ptr = self.as_ptr();

        assert_unsafe_precondition!(
            check_library_ub,
            "slice::split_at_unchecked requires the index to be within the slice",
            (mid: usize = mid, len: usize = len) => mid <= len,
        );

        // SAFETY: 调用方必须保证 `mid <= self.len()`；`self` 是有效切片引用，因此
        // `ptr..ptr.add(len)` 位于同一 allocation 内，左右两个共享切片范围有效。
        unsafe { (from_raw_parts(ptr, mid), from_raw_parts(ptr.add(mid), unchecked_sub(len, mid))) }
    }

    /// 不做边界检查，在指定索引处把一个可变切片分成两个可变切片。
///
    /// 第一个切片包含 `[0, mid)` 的所有索引，不包含 `mid` 本身；第二个切片包含
    /// `[mid, len)` 的所有索引，不包含 `len` 本身。
///
    /// 安全替代方案见 [`split_at_mut`]。
    ///
    /// # 安全性(Safety）
    ///
    /// 使用越界索引调用本方法是 *[undefined behavior]*，即使得到的引用之后没有被使用。
    /// 调用方必须保证 `mid <= self.len()`。这个不变量保证左右两个范围都位于原切片内；
    /// 可变版本还依赖 `[0, mid)` 与 `[mid, len)` 不重叠，才能同时返回两个 `&mut [T]`。
    ///
    /// [`split_at_mut`]: slice::split_at_mut
    /// [undefined behavior]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    ///
    /// # 示例
    ///
    /// ```
    /// let mut v = [1, 0, 3, 0, 5, 6];
    /// // 用作用域限制两个可变借用的生命周期。
    /// unsafe {
    ///     let (left, right) = v.split_at_mut_unchecked(2);
    ///     assert_eq!(left, [1, 0]);
    ///     assert_eq!(right, [3, 0, 5, 6]);
    ///     left[1] = 2;
    ///     right[1] = 4;
    /// }
    /// assert_eq!(v, [1, 2, 3, 4, 5, 6]);
    /// ```
    #[stable(feature = "slice_split_at_unchecked", since = "1.79.0")]
    #[rustc_const_stable(feature = "const_slice_split_at_mut", since = "1.83.0")]
    #[inline]
    #[must_use]
    #[track_caller]
    pub const unsafe fn split_at_mut_unchecked(&mut self, mid: usize) -> (&mut [T], &mut [T]) {
        let len = self.len();
        let ptr = self.as_mut_ptr();

        assert_unsafe_precondition!(
            check_library_ub,
            "slice::split_at_mut_unchecked requires the index to be within the slice",
            (mid: usize = mid, len: usize = len) => mid <= len,
        );

        // SAFETY: 调用方必须保证 `mid <= self.len()`。
        //
        // `[ptr, ptr + mid)` 与 `[ptr + mid, ptr + len)` 不重叠，因此可以同时返回
        // 两个可变切片引用。
        unsafe {
            (
                from_raw_parts_mut(ptr, mid),
                from_raw_parts_mut(ptr.add(mid), unchecked_sub(len, mid)),
            )
        }
    }

    /// 在指定索引处把切片分成两个共享切片；如果切片太短则返回 `None`。
///
    /// 如果 `mid <= len`，返回一对子切片：第一个包含 `[0, mid)` 的所有索引，
    /// 第二个包含 `[mid, len)` 的所有索引。
///
    /// 如果 `mid > len`，返回 `None`。
    ///
    /// # 示例
    ///
    /// ```
    /// let v = [1, -2, 3, -4, 5, -6];
    ///
    /// {
    ///    let (left, right) = v.split_at_checked(0).unwrap();
    ///    assert_eq!(left, []);
    ///    assert_eq!(right, [1, -2, 3, -4, 5, -6]);
    /// }
    ///
    /// {
    ///     let (left, right) = v.split_at_checked(2).unwrap();
    ///     assert_eq!(left, [1, -2]);
    ///     assert_eq!(right, [3, -4, 5, -6]);
    /// }
    ///
    /// {
    ///     let (left, right) = v.split_at_checked(6).unwrap();
    ///     assert_eq!(left, [1, -2, 3, -4, 5, -6]);
    ///     assert_eq!(right, []);
    /// }
    ///
    /// assert_eq!(None, v.split_at_checked(7));
    /// ```
    #[stable(feature = "split_at_checked", since = "1.80.0")]
    #[rustc_const_stable(feature = "split_at_checked", since = "1.80.0")]
    #[inline]
    #[must_use]
    pub const fn split_at_checked(&self, mid: usize) -> Option<(&[T], &[T])> {
        if mid <= self.len() {
            // SAFETY: `[0, mid)` 和 `[mid, len)` 都位于 `self` 内，满足
            // `split_at_unchecked` 的 `mid <= len` 前置条件。
            Some(unsafe { self.split_at_unchecked(mid) })
        } else {
            None
        }
    }

    /// 在指定索引处把可变切片分成两个可变切片；如果切片太短则返回 `None`。
///
    /// 如果 `mid <= len`，返回一对互不重叠的可变子切片：第一个包含 `[0, mid)`，
    /// 第二个包含 `[mid, len)`。
///
    /// 如果 `mid > len`，返回 `None`。
    ///
    /// # 示例
    ///
    /// ```
    /// let mut v = [1, 0, 3, 0, 5, 6];
    ///
    /// if let Some((left, right)) = v.split_at_mut_checked(2) {
    ///     assert_eq!(left, [1, 0]);
    ///     assert_eq!(right, [3, 0, 5, 6]);
    ///     left[1] = 2;
    ///     right[1] = 4;
    /// }
    /// assert_eq!(v, [1, 2, 3, 4, 5, 6]);
    ///
    /// assert_eq!(None, v.split_at_mut_checked(7));
    /// ```
    #[stable(feature = "split_at_checked", since = "1.80.0")]
    #[rustc_const_stable(feature = "const_slice_split_at_mut", since = "1.83.0")]
    #[inline]
    #[must_use]
    pub const fn split_at_mut_checked(&mut self, mid: usize) -> Option<(&mut [T], &mut [T])> {
        if mid <= self.len() {
            // SAFETY: `[0, mid)` 和 `[mid, len)` 都位于 `self` 内且不重叠，满足
            // `split_at_mut_unchecked` 的前置条件。
            Some(unsafe { self.split_at_mut_unchecked(mid) })
        } else {
            None
        }
    }

    /// 返回按满足 `pred` 的元素分隔出的子切片迭代器。
    ///
    /// 匹配到的分隔元素本身不会包含在任何返回的子切片中。
    ///
    /// # 示例
    ///
    /// ```
    /// let slice = [10, 40, 33, 20];
    /// let mut iter = slice.split(|num| num % 3 == 0);
    ///
    /// assert_eq!(iter.next().unwrap(), &[10, 40]);
    /// assert_eq!(iter.next().unwrap(), &[20]);
    /// assert!(iter.next().is_none());
    /// ```
    ///
    /// 如果第一个元素就匹配，迭代器返回的第一个条目为空切片。类似地，如果最后一个元素匹配，
    /// 迭代器返回的最后一个条目为空切片：
    ///
    /// ```
    /// let slice = [10, 40, 33];
    /// let mut iter = slice.split(|num| num % 3 == 0);
    ///
    /// assert_eq!(iter.next().unwrap(), &[10, 40]);
    /// assert_eq!(iter.next().unwrap(), &[]);
    /// assert!(iter.next().is_none());
    /// ```
    ///
    /// 如果两个匹配元素直接相邻，它们之间也会产生一个空切片：
    ///
    /// ```
    /// let slice = [10, 6, 33, 20];
    /// let mut iter = slice.split(|num| num % 3 == 0);
    ///
    /// assert_eq!(iter.next().unwrap(), &[10]);
    /// assert_eq!(iter.next().unwrap(), &[]);
    /// assert_eq!(iter.next().unwrap(), &[20]);
    /// assert!(iter.next().is_none());
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn split<F>(&self, pred: F) -> Split<'_, T, F>
    where
        F: FnMut(&T) -> bool,
    {
        Split::new(self, pred)
    }

    /// 返回按满足 `pred` 的元素分隔出的可变子切片迭代器。
    ///
    /// 匹配到的分隔元素本身不会包含在任何返回的可变子切片中；各子切片互不重叠。
    ///
    /// # 示例
    ///
    /// ```
    /// let mut v = [10, 40, 30, 20, 60, 50];
    ///
    /// for group in v.split_mut(|num| *num % 3 == 0) {
    ///     group[0] = 1;
    /// }
    /// assert_eq!(v, [1, 40, 30, 1, 60, 1]);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn split_mut<F>(&mut self, pred: F) -> SplitMut<'_, T, F>
    where
        F: FnMut(&T) -> bool,
    {
        SplitMut::new(self, pred)
    }

    /// 返回按满足 `pred` 的元素分隔出的子切片迭代器，并把匹配元素包含在前一个子切片末尾。
    ///
    /// 匹配元素在语义上作为前一段的终止符。
    ///
    /// # 示例
    ///
    /// ```
    /// let slice = [10, 40, 33, 20];
    /// let mut iter = slice.split_inclusive(|num| num % 3 == 0);
    ///
    /// assert_eq!(iter.next().unwrap(), &[10, 40, 33]);
    /// assert_eq!(iter.next().unwrap(), &[20]);
    /// assert!(iter.next().is_none());
    /// ```
    ///
    /// 如果切片最后一个元素匹配，该元素会被视为前一段的终止符；那一段会成为迭代器返回的
    /// 最后一个条目。
    ///
    /// ```
    /// let slice = [3, 10, 40, 33];
    /// let mut iter = slice.split_inclusive(|num| num % 3 == 0);
    ///
    /// assert_eq!(iter.next().unwrap(), &[3]);
    /// assert_eq!(iter.next().unwrap(), &[10, 40, 33]);
    /// assert!(iter.next().is_none());
    /// ```
    #[stable(feature = "split_inclusive", since = "1.51.0")]
    #[inline]
    pub fn split_inclusive<F>(&self, pred: F) -> SplitInclusive<'_, T, F>
    where
        F: FnMut(&T) -> bool,
    {
        SplitInclusive::new(self, pred)
    }

    /// 返回按满足 `pred` 的元素分隔出的可变子切片迭代器，并把匹配元素包含在前一个
    /// 子切片末尾。
    ///
    /// 匹配元素在语义上作为前一段的终止符。
    ///
    /// # 示例
    ///
    /// ```
    /// let mut v = [10, 40, 30, 20, 60, 50];
    ///
    /// for group in v.split_inclusive_mut(|num| *num % 3 == 0) {
    ///     let terminator_idx = group.len()-1;
    ///     group[terminator_idx] = 1;
    /// }
    /// assert_eq!(v, [10, 40, 1, 20, 1, 1]);
    /// ```
    #[stable(feature = "split_inclusive", since = "1.51.0")]
    #[inline]
    pub fn split_inclusive_mut<F>(&mut self, pred: F) -> SplitInclusiveMut<'_, T, F>
    where
        F: FnMut(&T) -> bool,
    {
        SplitInclusiveMut::new(self, pred)
    }

    /// 返回按满足 `pred` 的元素分隔出的子切片迭代器，从切片末尾开始反向工作。
    ///
    /// 匹配到的分隔元素本身不会包含在任何返回的子切片中。
    ///
    /// # 示例
    ///
    /// ```
    /// let slice = [11, 22, 33, 0, 44, 55];
    /// let mut iter = slice.rsplit(|num| *num == 0);
    ///
    /// assert_eq!(iter.next().unwrap(), &[44, 55]);
    /// assert_eq!(iter.next().unwrap(), &[11, 22, 33]);
    /// assert_eq!(iter.next(), None);
    /// ```
    ///
    /// 与 `split()` 一样，如果第一个或最后一个元素匹配，迭代器返回的第一个
    /// （或最后一个）条目会是空切片。
    ///
    /// ```
    /// let v = &[0, 1, 1, 2, 3, 5, 8];
    /// let mut it = v.rsplit(|n| *n % 2 == 0);
    /// assert_eq!(it.next().unwrap(), &[]);
    /// assert_eq!(it.next().unwrap(), &[3, 5]);
    /// assert_eq!(it.next().unwrap(), &[1, 1]);
    /// assert_eq!(it.next().unwrap(), &[]);
    /// assert_eq!(it.next(), None);
    /// ```
    #[stable(feature = "slice_rsplit", since = "1.27.0")]
    #[inline]
    pub fn rsplit<F>(&self, pred: F) -> RSplit<'_, T, F>
    where
        F: FnMut(&T) -> bool,
    {
        RSplit::new(self, pred)
    }

    /// 返回按满足 `pred` 的元素分隔出的可变子切片迭代器，从切片末尾开始反向工作。
    ///
    /// 匹配到的分隔元素本身不会包含在任何返回的可变子切片中；各子切片互不重叠。
    ///
    /// # 示例
    ///
    /// ```
    /// let mut v = [100, 400, 300, 200, 600, 500];
    ///
    /// let mut count = 0;
    /// for group in v.rsplit_mut(|num| *num % 3 == 0) {
    ///     count += 1;
    ///     group[0] = count;
    /// }
    /// assert_eq!(v, [3, 400, 300, 2, 600, 1]);
    /// ```
    ///
    #[stable(feature = "slice_rsplit", since = "1.27.0")]
    #[inline]
    pub fn rsplit_mut<F>(&mut self, pred: F) -> RSplitMut<'_, T, F>
    where
        F: FnMut(&T) -> bool,
    {
        RSplitMut::new(self, pred)
    }

    /// 返回按满足 `pred` 的元素分隔出的子切片迭代器，最多返回 `n` 个条目。
    ///
    /// 匹配到的分隔元素本身不会包含在任何返回的子切片中。
    ///
    /// 如果存在最后一个返回项，它会包含切片中剩余未分隔的部分。
    ///
    /// # 示例
    ///
    /// 按能被 3 整除的数字只分割一次并打印结果（即 `[10, 40]`、
    /// `[20, 60, 50]`）：
    ///
    /// ```
    /// let v = [10, 40, 30, 20, 60, 50];
    ///
    /// for group in v.splitn(2, |num| *num % 3 == 0) {
    ///     println!("{group:?}");
    /// }
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn splitn<F>(&self, n: usize, pred: F) -> SplitN<'_, T, F>
    where
        F: FnMut(&T) -> bool,
    {
        SplitN::new(self.split(pred), n)
    }

    /// 返回按满足 `pred` 的元素分隔出的可变子切片迭代器，最多返回 `n` 个条目。
    ///
    /// 匹配到的分隔元素本身不会包含在任何返回的可变子切片中；各子切片互不重叠。
    ///
    /// 如果存在最后一个返回项，它会包含切片中剩余未分隔的部分。
    ///
    /// # 示例
    ///
    /// ```
    /// let mut v = [10, 40, 30, 20, 60, 50];
    ///
    /// for group in v.splitn_mut(2, |num| *num % 3 == 0) {
    ///     group[0] = 1;
    /// }
    /// assert_eq!(v, [1, 40, 30, 1, 60, 50]);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn splitn_mut<F>(&mut self, n: usize, pred: F) -> SplitNMut<'_, T, F>
    where
        F: FnMut(&T) -> bool,
    {
        SplitNMut::new(self.split_mut(pred), n)
    }

    /// 返回按满足 `pred` 的元素分隔出的子切片迭代器，最多返回 `n` 个条目；
    /// 它从切片末尾开始反向工作。
    ///
    /// 匹配到的分隔元素本身不会包含在任何返回的子切片中。
    ///
    /// 如果存在最后一个返回项，它会包含切片中剩余未分隔的部分。
    ///
    /// # 示例
    ///
    /// 从末尾开始，按能被 3 整除的数字只分割一次并打印结果
    /// （即 `[50]`、`[10, 40, 30, 20]`）：
    ///
    /// ```
    /// let v = [10, 40, 30, 20, 60, 50];
    ///
    /// for group in v.rsplitn(2, |num| *num % 3 == 0) {
    ///     println!("{group:?}");
    /// }
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn rsplitn<F>(&self, n: usize, pred: F) -> RSplitN<'_, T, F>
    where
        F: FnMut(&T) -> bool,
    {
        RSplitN::new(self.rsplit(pred), n)
    }

    /// 返回按满足 `pred` 的元素分隔出的可变子切片迭代器，最多返回 `n` 个条目；
    /// 它从切片末尾开始反向工作。
    ///
    /// 匹配到的分隔元素本身不会包含在任何返回的可变子切片中；各子切片互不重叠。
    ///
    /// 如果存在最后一个返回项，它会包含切片中剩余未分隔的部分。
    ///
    /// # 示例
    ///
    /// ```
    /// let mut s = [10, 40, 30, 20, 60, 50];
    ///
    /// for group in s.rsplitn_mut(2, |num| *num % 3 == 0) {
    ///     group[0] = 1;
    /// }
    /// assert_eq!(s, [1, 40, 30, 20, 60, 1]);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn rsplitn_mut<F>(&mut self, n: usize, pred: F) -> RSplitNMut<'_, T, F>
    where
        F: FnMut(&T) -> bool,
    {
        RSplitNMut::new(self.rsplit_mut(pred), n)
    }

    /// 在第一个满足指定谓词的元素处分割切片。
    ///
    /// 如果切片中存在匹配元素，则返回匹配元素之前的前缀和之后的后缀；
    /// 匹配元素本身不包含在返回值中。如果没有元素匹配，则返回 `None`。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(slice_split_once)]
    /// let s = [1, 2, 3, 2, 4];
    /// assert_eq!(s.split_once(|&x| x == 2), Some((
    ///     &[1][..],
    ///     &[3, 2, 4][..]
    /// )));
    /// assert_eq!(s.split_once(|&x| x == 0), None);
    /// ```
    #[unstable(feature = "slice_split_once", reason = "newly added", issue = "112811")]
    #[inline]
    pub fn split_once<F>(&self, pred: F) -> Option<(&[T], &[T])>
    where
        F: FnMut(&T) -> bool,
    {
        let index = self.iter().position(pred)?;
        Some((&self[..index], &self[index + 1..]))
    }

    /// 在最后一个满足指定谓词的元素处分割切片。
    ///
    /// 如果切片中存在匹配元素，则返回匹配元素之前的前缀和之后的后缀；
    /// 匹配元素本身不包含在返回值中。如果没有元素匹配，则返回 `None`。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(slice_split_once)]
    /// let s = [1, 2, 3, 2, 4];
    /// assert_eq!(s.rsplit_once(|&x| x == 2), Some((
    ///     &[1, 2, 3][..],
    ///     &[4][..]
    /// )));
    /// assert_eq!(s.rsplit_once(|&x| x == 0), None);
    /// ```
    #[unstable(feature = "slice_split_once", reason = "newly added", issue = "112811")]
    #[inline]
    pub fn rsplit_once<F>(&self, pred: F) -> Option<(&[T], &[T])>
    where
        F: FnMut(&T) -> bool,
    {
        let index = self.iter().rposition(pred)?;
        Some((&self[..index], &self[index + 1..]))
    }

    /// 如果切片包含给定值的元素，返回 `true`。
    ///
    /// 该操作是 *O*(*n*)。
    ///
    /// 注意，如果切片已经排序，[`binary_search`] 可能更快。
    ///
    /// [`binary_search`]: slice::binary_search
    ///
    /// # 示例
    ///
    /// ```
    /// let v = [10, 40, 30];
    /// assert!(v.contains(&30));
    /// assert!(!v.contains(&50));
    /// ```
    ///
    /// 如果你没有 `&T`，而是有其它可与 `T` 比较的值（例如 `String` 实现了
    /// `PartialEq<str>`），可以使用 `iter().any`：
    ///
    /// ```
    /// let v = [String::from("hello"), String::from("world")]; // `String` 切片
    /// assert!(v.iter().any(|e| e == "hello")); // 用 `&str` 搜索
    /// assert!(!v.iter().any(|e| e == "hi"));
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    #[must_use]
    pub fn contains(&self, x: &T) -> bool
    where
        T: PartialEq,
    {
        cmp::SliceContains::slice_contains(x, self)
    }

    /// 如果 `needle` 是切片的前缀，或与切片相等，返回 `true`。
    ///
    /// # 示例
    ///
    /// ```
    /// let v = [10, 40, 30];
    /// assert!(v.starts_with(&[10]));
    /// assert!(v.starts_with(&[10, 40]));
    /// assert!(v.starts_with(&v));
    /// assert!(!v.starts_with(&[50]));
    /// assert!(!v.starts_with(&[10, 50]));
    /// ```
    ///
    /// 如果 `needle` 是空切片，总是返回 `true`：
    ///
    /// ```
    /// let v = &[10, 40, 30];
    /// assert!(v.starts_with(&[]));
    /// let v: &[u8] = &[];
    /// assert!(v.starts_with(&[]));
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[must_use]
    pub fn starts_with(&self, needle: &[T]) -> bool
    where
        T: PartialEq,
    {
        let n = needle.len();
        self.len() >= n && needle == &self[..n]
    }

    /// 如果 `needle` 是切片的后缀，或与切片相等，返回 `true`。
    ///
    /// # 示例
    ///
    /// ```
    /// let v = [10, 40, 30];
    /// assert!(v.ends_with(&[30]));
    /// assert!(v.ends_with(&[40, 30]));
    /// assert!(v.ends_with(&v));
    /// assert!(!v.ends_with(&[50]));
    /// assert!(!v.ends_with(&[50, 30]));
    /// ```
    ///
    /// 如果 `needle` 是空切片，总是返回 `true`：
    ///
    /// ```
    /// let v = &[10, 40, 30];
    /// assert!(v.ends_with(&[]));
    /// let v: &[u8] = &[];
    /// assert!(v.ends_with(&[]));
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[must_use]
    pub fn ends_with(&self, needle: &[T]) -> bool
    where
        T: PartialEq,
    {
        let (m, n) = (self.len(), needle.len());
        m >= n && needle == &self[m - n..]
    }

    /// 返回移除前缀后的子切片。
    ///
    /// 如果切片以 `prefix` 开头，返回前缀之后的子切片并包在 `Some` 中。
    /// 如果 `prefix` 为空，直接返回原切片。如果 `prefix` 等于整个原切片，则返回空切片。
    ///
    /// 如果切片并不以 `prefix` 开头，则返回 `None`。
    ///
    /// # 示例
    ///
    /// ```
    /// let v = &[10, 40, 30];
    /// assert_eq!(v.strip_prefix(&[10]), Some(&[40, 30][..]));
    /// assert_eq!(v.strip_prefix(&[10, 40]), Some(&[30][..]));
    /// assert_eq!(v.strip_prefix(&[10, 40, 30]), Some(&[][..]));
    /// assert_eq!(v.strip_prefix(&[50]), None);
    /// assert_eq!(v.strip_prefix(&[10, 50]), None);
    ///
    /// let prefix : &str = "he";
    /// assert_eq!(b"hello".strip_prefix(prefix.as_bytes()),
    ///            Some(b"llo".as_ref()));
    /// ```
    #[must_use = "returns the subslice without modifying the original"]
    #[stable(feature = "slice_strip", since = "1.51.0")]
    pub fn strip_prefix<P: SlicePattern<Item = T> + ?Sized>(&self, prefix: &P) -> Option<&[T]>
    where
        T: PartialEq,
    {
        // 如果 SlicePattern 之后变得更复杂，本函数就需要重写。
        let prefix = prefix.as_slice();
        let n = prefix.len();
        if n <= self.len() {
            let (head, tail) = self.split_at(n);
            if head == prefix {
                return Some(tail);
            }
        }
        None
    }

    /// 返回移除后缀后的子切片。
    ///
    /// 如果切片以 `suffix` 结尾，返回后缀之前的子切片并包在 `Some` 中。
    /// 如果 `suffix` 为空，直接返回原切片。如果 `suffix` 等于整个原切片，则返回空切片。
    ///
    /// 如果切片并不以 `suffix` 结尾，则返回 `None`。
    ///
    /// # 示例
    ///
    /// ```
    /// let v = &[10, 40, 30];
    /// assert_eq!(v.strip_suffix(&[30]), Some(&[10, 40][..]));
    /// assert_eq!(v.strip_suffix(&[40, 30]), Some(&[10][..]));
    /// assert_eq!(v.strip_suffix(&[10, 40, 30]), Some(&[][..]));
    /// assert_eq!(v.strip_suffix(&[50]), None);
    /// assert_eq!(v.strip_suffix(&[50, 30]), None);
    /// ```
    #[must_use = "returns the subslice without modifying the original"]
    #[stable(feature = "slice_strip", since = "1.51.0")]
    pub fn strip_suffix<P: SlicePattern<Item = T> + ?Sized>(&self, suffix: &P) -> Option<&[T]>
    where
        T: PartialEq,
    {
        // 如果 SlicePattern 之后变得更复杂，本函数就需要重写。
        let suffix = suffix.as_slice();
        let (len, n) = (self.len(), suffix.len());
        if n <= len {
            let (head, tail) = self.split_at(len - n);
            if tail == suffix {
                return Some(head);
            }
        }
        None
    }

    /// 返回同时移除前缀和后缀后的子切片。
    ///
    /// 如果切片以 `prefix` 开头且以 `suffix` 结尾，返回前缀之后、后缀之前的子切片，
    /// 并包在 `Some` 中。
    ///
    /// 如果切片不以 `prefix` 开头，或不以 `suffix` 结尾，则返回 `None`。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(strip_circumfix)]
    ///
    /// let v = &[10, 50, 40, 30];
    /// assert_eq!(v.strip_circumfix(&[10], &[30]), Some(&[50, 40][..]));
    /// assert_eq!(v.strip_circumfix(&[10], &[40, 30]), Some(&[50][..]));
    /// assert_eq!(v.strip_circumfix(&[10, 50], &[40, 30]), Some(&[][..]));
    /// assert_eq!(v.strip_circumfix(&[50], &[30]), None);
    /// assert_eq!(v.strip_circumfix(&[10], &[40]), None);
    /// assert_eq!(v.strip_circumfix(&[], &[40, 30]), Some(&[10, 50][..]));
    /// assert_eq!(v.strip_circumfix(&[10, 50], &[]), Some(&[40, 30][..]));
    /// ```
    #[must_use = "returns the subslice without modifying the original"]
    #[unstable(feature = "strip_circumfix", issue = "147946")]
    pub fn strip_circumfix<S, P>(&self, prefix: &P, suffix: &S) -> Option<&[T]>
    where
        T: PartialEq,
        S: SlicePattern<Item = T> + ?Sized,
        P: SlicePattern<Item = T> + ?Sized,
    {
        self.strip_prefix(prefix)?.strip_suffix(suffix)
    }

    /// 返回去掉可选前缀后的子切片。
    ///
    /// 如果切片以 `prefix` 开头，返回前缀之后的子切片。如果 `prefix` 为空，
    /// 或切片并不以 `prefix` 开头，则直接返回原切片。如果 `prefix` 等于整个原切片，
    /// 则返回空切片。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(trim_prefix_suffix)]
    ///
    /// let v = &[10, 40, 30];
    ///
    /// // 前缀存在，移除它。
    /// assert_eq!(v.trim_prefix(&[10]), &[40, 30][..]);
    /// assert_eq!(v.trim_prefix(&[10, 40]), &[30][..]);
    /// assert_eq!(v.trim_prefix(&[10, 40, 30]), &[][..]);
    ///
    /// // 前缀不存在，返回原切片。
    /// assert_eq!(v.trim_prefix(&[50]), &[10, 40, 30][..]);
    /// assert_eq!(v.trim_prefix(&[10, 50]), &[10, 40, 30][..]);
    ///
    /// let prefix : &str = "he";
    /// assert_eq!(b"hello".trim_prefix(prefix.as_bytes()), b"llo".as_ref());
    /// ```
    #[must_use = "returns the subslice without modifying the original"]
    #[unstable(feature = "trim_prefix_suffix", issue = "142312")]
    pub fn trim_prefix<P: SlicePattern<Item = T> + ?Sized>(&self, prefix: &P) -> &[T]
    where
        T: PartialEq,
    {
        // 如果将来 SlicePattern 变得更复杂，这个函数需要随之重写。
        let prefix = prefix.as_slice();
        let n = prefix.len();
        if n <= self.len() {
            let (head, tail) = self.split_at(n);
            if head == prefix {
                return tail;
            }
        }
        self
    }

    /// 返回去掉可选后缀后的子切片。
///
    /// 如果切片以 `suffix` 结尾，返回后缀之前的子切片。如果 `suffix` 为空，或切片并不以
    /// `suffix` 结尾，则直接返回原切片。如果 `suffix` 等于整个原切片，则返回空切片。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(trim_prefix_suffix)]
    ///
    /// let v = &[10, 40, 30];
    ///
    /// // 后缀存在，移除它。
    /// assert_eq!(v.trim_suffix(&[30]), &[10, 40][..]);
    /// assert_eq!(v.trim_suffix(&[40, 30]), &[10][..]);
    /// assert_eq!(v.trim_suffix(&[10, 40, 30]), &[][..]);
    ///
    /// // 后缀不存在，返回原切片。
    /// assert_eq!(v.trim_suffix(&[50]), &[10, 40, 30][..]);
    /// assert_eq!(v.trim_suffix(&[50, 30]), &[10, 40, 30][..]);
    /// ```
    #[must_use = "returns the subslice without modifying the original"]
    #[unstable(feature = "trim_prefix_suffix", issue = "142312")]
    pub fn trim_suffix<P: SlicePattern<Item = T> + ?Sized>(&self, suffix: &P) -> &[T]
    where
        T: PartialEq,
    {
        // 如果将来 SlicePattern 变得更复杂，这个函数需要随之重写。
        let suffix = suffix.as_slice();
        let (len, n) = (self.len(), suffix.len());
        if n <= len {
            let (head, tail) = self.split_at(len - n);
            if tail == suffix {
                return head;
            }
        }
        self
    }

    /// 在已排序切片中用二分查找给定元素。
    ///
    /// 调用前提是切片已经按照 `T: Ord` 的顺序排序。若切片未排序，返回结果没有指定含义；
    /// 它不会保证是正确位置，也不能作为插入位置使用。
///
    /// 如果找到该值，返回 [`Result::Ok`]，其中包含某个匹配元素的索引。若存在多个匹配项，
    /// 可以返回其中任意一个；当前选择是确定性的，但未来 Rust 版本可能改变具体选择。
    /// 如果未找到，返回 [`Result::Err`]，其中包含可插入匹配元素且仍保持排序顺序的位置。
///
    /// 另见 [`binary_search_by`]、[`binary_search_by_key`] 和 [`partition_point`]。
    ///
    /// [`binary_search_by`]: slice::binary_search_by
    /// [`binary_search_by_key`]: slice::binary_search_by_key
    /// [`partition_point`]: slice::partition_point
    ///
    /// # 示例
    ///
    /// 查找四个元素：第一个存在且位置唯一；第二、第三个不存在；第四个有多个相等元素，
    /// 可以匹配 `[1, 4]` 中的任意位置。
    ///
    /// ```
    /// let s = [0, 1, 1, 1, 1, 2, 3, 5, 8, 13, 21, 34, 55];
    ///
    /// assert_eq!(s.binary_search(&13),  Ok(9));
    /// assert_eq!(s.binary_search(&4),   Err(7));
    /// assert_eq!(s.binary_search(&100), Err(13));
    /// let r = s.binary_search(&1);
    /// assert!(match r { Ok(1..=4) => true, _ => false, });
    /// ```
    ///
    /// 如果需要找到全部匹配项的完整 *范围*，而不是任意一个匹配位置，可以使用
    /// [`partition_point`]：
    /// ```
    /// let s = [0, 1, 1, 1, 1, 2, 3, 5, 8, 13, 21, 34, 55];
    ///
    /// let low = s.partition_point(|x| x < &1);
    /// assert_eq!(low, 1);
    /// let high = s.partition_point(|x| x <= &1);
    /// assert_eq!(high, 5);
    /// let r = s.binary_search(&1);
    /// assert!((low..high).contains(&r.unwrap()));
    ///
    /// assert!(s[..low].iter().all(|&x| x < 1));
    /// assert!(s[low..high].iter().all(|&x| x == 1));
    /// assert!(s[high..].iter().all(|&x| x > 1));
    ///
    /// // 对不存在的元素，相等项“范围”为空。
    /// assert_eq!(s.partition_point(|x| x < &11), 9);
    /// assert_eq!(s.partition_point(|x| x <= &11), 9);
    /// assert_eq!(s.binary_search(&11), Err(9));
    /// ```
    ///
    /// 如果需要向已排序 vector 插入元素并保持排序顺序，可以考虑使用 [`partition_point`]：
    ///
    /// ```
    /// let mut s = vec![0, 1, 1, 1, 1, 2, 3, 5, 8, 13, 21, 34, 55];
    /// let num = 42;
    /// let idx = s.partition_point(|&x| x <= num);
    /// // 如果 `num` 唯一，使用 `<` 的 `s.partition_point(|&x| x < num)` 等价于
    /// // `s.binary_search(&num).unwrap_or_else(|x| x)`；但使用 `<=` 可让 `insert`
    /// // 移动更少元素。
    /// s.insert(idx, num);
    /// assert_eq!(s, [0, 1, 1, 1, 1, 2, 3, 5, 8, 13, 21, 34, 42, 55]);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn binary_search(&self, x: &T) -> Result<usize, usize>
    where
        T: Ord,
    {
        self.binary_search_by(|p| p.cmp(x))
    }

    /// 使用比较函数在切片中执行二分查找。
///
    /// 比较函数应返回一个 `Ordering`，表示它的参数相对于目标值是 `Less`、`Equal`
    /// 还是 `Greater`。调用前提是切片已经按同一个比较关系排序；如果切片未排序，
    /// 或比较函数与底层排序顺序不一致，返回结果没有指定含义。
///
    /// 如果找到该值，返回 [`Result::Ok`]，其中包含某个匹配元素的索引。若存在多个匹配项，
    /// 可以返回其中任意一个；当前选择是确定性的，但未来 Rust 版本可能改变具体选择。
    /// 如果未找到，返回 [`Result::Err`]，其中包含可插入匹配元素且仍保持排序顺序的位置。
///
    /// 另见 [`binary_search`]、[`binary_search_by_key`] 和 [`partition_point`]。
    ///
    /// [`binary_search`]: slice::binary_search
    /// [`binary_search_by_key`]: slice::binary_search_by_key
    /// [`partition_point`]: slice::partition_point
    ///
    /// # 示例
    ///
    /// 查找四个元素：第一个存在且位置唯一；第二、第三个不存在；第四个有多个相等元素，
    /// 可以匹配 `[1, 4]` 中的任意位置。
    ///
    /// ```
    /// let s = [0, 1, 1, 1, 1, 2, 3, 5, 8, 13, 21, 34, 55];
    ///
    /// let seek = 13;
    /// assert_eq!(s.binary_search_by(|probe| probe.cmp(&seek)), Ok(9));
    /// let seek = 4;
    /// assert_eq!(s.binary_search_by(|probe| probe.cmp(&seek)), Err(7));
    /// let seek = 100;
    /// assert_eq!(s.binary_search_by(|probe| probe.cmp(&seek)), Err(13));
    /// let seek = 1;
    /// let r = s.binary_search_by(|probe| probe.cmp(&seek));
    /// assert!(match r { Ok(1..=4) => true, _ => false, });
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn binary_search_by<'a, F>(&'a self, mut f: F) -> Result<usize, usize>
    where
        F: FnMut(&'a T) -> Ordering,
    {
        let mut size = self.len();
        if size == 0 {
            return Err(0);
        }
        let mut base = 0usize;

        // 即使比较结果为 Equal，这个循环也故意不提前退出。我们希望循环迭代次数
        // *只* 取决于输入切片大小，这样 CPU 可以可靠预测循环次数。
        while size > 1 {
            let half = size / 2;
            let mid = base + half;

            // SAFETY: 下列不变量保证调用安全：
            // - `mid >= 0`：由类型定义保证；
            // - `mid < self.len()`：`base` 始终位于剩余搜索区间起点，`half < size`，
            //   因而 `base + half` 仍在原切片内。
            let cmp = f(unsafe { self.get_unchecked(mid) });

            // 二分查找与分支预测配合较差，因此在目标架构支持时强制编译器使用条件移动。
            base = hint::select_unpredictable(cmp == Greater, base, mid);

            // 当 `size` 为奇数且比较结果为 Greater 时，这里的范围更新并不精确：
            // mid 元素虽已知大于目标，仍会被 `size` 计入下一轮。
            //
            // 这是有意的权衡：保持循环次数不变且可预测带来的性能收益，高于多考虑一个元素的成本。
            size -= half;
        }

        // SAFETY: `base` 始终位于原切片内；空切片已在函数开头返回。
        let cmp = f(unsafe { self.get_unchecked(base) });
        if cmp == Equal {
            // SAFETY: 与上面的 `get_unchecked` 相同，`base` 已知在边界内。
            unsafe { hint::assert_unchecked(base < self.len()) };
            Ok(base)
        } else {
            let result = base + (cmp == Less) as usize;
            // SAFETY: 与上面的 `get_unchecked` 相同。注意这里是 `<=`，因为插入点可以等于
            // `self.len()`，不同于 `Ok` 路径中的索引。
            unsafe { hint::assert_unchecked(result <= self.len()) };
            Err(result)
        }
    }

    /// 使用键提取函数在切片中执行二分查找。
    ///
    /// 调用前提是切片已经按该键排序，例如使用同一个键提取函数调用过 [`sort_by_key`]。
    /// 如果切片未按该键排序，返回结果没有指定含义。
    ///
    /// 如果找到该值，返回 [`Result::Ok`]，其中包含某个匹配元素的索引。若存在多个匹配项，
    /// 可以返回其中任意一个；当前选择是确定性的，但未来 Rust 版本可能改变具体选择。
    /// 如果未找到，返回 [`Result::Err`]，其中包含可插入匹配元素且仍保持排序顺序的位置。
    ///
    /// 另见 [`binary_search`]、[`binary_search_by`] 和 [`partition_point`]。
    ///
    /// [`sort_by_key`]: slice::sort_by_key
    /// [`binary_search`]: slice::binary_search
    /// [`binary_search_by`]: slice::binary_search_by
    /// [`partition_point`]: slice::partition_point
    ///
    /// # 示例
    ///
    /// 在按第二个元素排序的 pair 切片中查找四个元素：第一个存在且位置唯一；
    /// 第二、第三个不存在；第四个有多个相等键，可以匹配 `[1, 4]` 中的任意位置。
    ///
    /// ```
    /// let s = [(0, 0), (2, 1), (4, 1), (5, 1), (3, 1),
    ///          (1, 2), (2, 3), (4, 5), (5, 8), (3, 13),
    ///          (1, 21), (2, 34), (4, 55)];
    ///
    /// assert_eq!(s.binary_search_by_key(&13, |&(a, b)| b),  Ok(9));
    /// assert_eq!(s.binary_search_by_key(&4, |&(a, b)| b),   Err(7));
    /// assert_eq!(s.binary_search_by_key(&100, |&(a, b)| b), Err(13));
    /// let r = s.binary_search_by_key(&1, |&(a, b)| b);
    /// assert!(match r { Ok(1..=4) => true, _ => false, });
    /// ```
    // 允许 rustdoc::broken_intra_doc_links，因为 `slice::sort_by_key` 位于 crate `alloc`，
    // 构建 `core` 时它还不存在：#74481。切片文档在 core 中显示时链接会断，但改成相对链接
    // 又会破坏该 item 被 re-export 后的链接，因此暂时允许 core 链接断开。
    #[allow(rustdoc::broken_intra_doc_links)]
    #[stable(feature = "slice_binary_search_by_key", since = "1.10.0")]
    #[inline]
    pub fn binary_search_by_key<'a, B, F>(&'a self, b: &B, mut f: F) -> Result<usize, usize>
    where
        F: FnMut(&'a T) -> B,
        B: Ord,
    {
        self.binary_search_by(|k| f(k).cmp(b))
    }

    /// 按升序排序切片，**不** 保留相等元素的原始相对顺序。
///
    /// 这是不稳定排序（可能重排相等元素）、原地排序（不分配），最坏时间复杂度为
    /// *O*(*n* \* log(*n*))。
///
    /// 如果 `T` 的 [`Ord`] 实现不是 [total order]，本函数可能 panic；即使正常返回，
    /// 切片中元素的最终顺序也没有指定含义。另见下面关于 panic 的说明。
///
    /// 例如 `|a, b| (a - b).cmp(a)` 既不传递、也不自反、更不是全序；
    /// 当 `a = 1, b = 2, c = 3` 时会出现 `a < b < c < a`。更多信息和示例见 [`Ord`] 文档。
///
///
    /// 即使 [`Ord`] 实现 panic，所有原始元素仍会留在切片中；通过内部可变性发生的修改也会
    /// 反映在输入切片里。
///
    /// 对只实现 [`PartialOrd`] 的类型（例如 [`f32`] 和 [`f64`]）排序需要额外谨慎。
    /// 例如 `f32::NAN != f32::NAN`，不满足 [`Ord`] 的自反性要求。若要排序包含浮点值的
    /// 切片，可用 `slice::sort_unstable_by` 搭配定义了 [total order] 的比较函数，
    /// 例如 [`f32::total_cmp`] 或 [`f64::total_cmp`]。如果能保证切片中所有值都落在
    /// [`PartialOrd::partial_cmp`] 构成 [total order] 的子集内，也可使用
    /// `sort_unstable_by(|a, b| a.partial_cmp(b).unwrap())`。
///
    /// # 当前实现
///
    /// 当前实现基于 Lukas Bergdoll 和 Orson Peters 的 [ipnsort]。它结合了 quicksort 的
    /// 快速平均情况和 heapsort 的快速最坏情况，在完全有序和完全逆序输入上可达到线性时间。
    /// 对只有 k 个不同元素的输入，期望排序时间为 *O*(*n* \* log(*k*))。
///
    /// 除少数特殊情况（例如切片已经部分有序）外，它通常比稳定排序更快。
    ///
    /// # Panics
    ///
    /// 如果 `T` 的 [`Ord`] 实现不是 [total order]，或 [`Ord`] 实现自身 panic，本函数可能 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// let mut v = [4, -5, 1, -3, 2];
    ///
    /// v.sort_unstable();
    /// assert_eq!(v, [-5, -3, 1, 2, 4]);
    /// ```
    ///
    /// [ipnsort]: https://github.com/Voultapher/sort-research-rs/tree/main/ipnsort
    /// [total order]: https://en.wikipedia.org/wiki/Total_order
    #[stable(feature = "sort_unstable", since = "1.20.0")]
    #[inline]
    pub fn sort_unstable(&mut self)
    where
        T: Ord,
    {
        sort::unstable::sort(self, &mut T::lt);
    }

    /// 使用比较函数按升序排序切片，**不** 保留相等元素的原始相对顺序。
///
    /// 这是不稳定排序（可能重排相等元素）、原地排序（不分配），最坏时间复杂度为
    /// *O*(*n* \* log(*n*))。
///
    /// 如果比较函数 `compare` 不构成 [total order]，本函数可能 panic；即使正常返回，
    /// 切片中元素的最终顺序也没有指定含义。另见下面关于 panic 的说明。
///
    /// 例如 `|a, b| (a - b).cmp(a)` 既不传递、也不自反、更不是全序；
    /// 当 `a = 1, b = 2, c = 3` 时会出现 `a < b < c < a`。更多信息和示例见 [`Ord`] 文档。
///
    /// 即使 `compare` panic，所有原始元素仍会留在切片中；通过内部可变性发生的修改也会
    /// 反映在输入切片里。
///
    /// # 当前实现
///
    /// 当前实现基于 Lukas Bergdoll 和 Orson Peters 的 [ipnsort]。它结合了 quicksort 的
    /// 快速平均情况和 heapsort 的快速最坏情况，在完全有序和完全逆序输入上可达到线性时间。
    /// 对只有 k 个不同元素的输入，期望排序时间为 *O*(*n* \* log(*k*))。
///
    /// 除少数特殊情况（例如切片已经部分有序）外，它通常比稳定排序更快。
    ///
    /// # Panics
    ///
    /// 如果 `compare` 不构成 [total order]，或 `compare` 自身 panic，本函数可能 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// let mut v = [4, -5, 1, -3, 2];
    /// v.sort_unstable_by(|a, b| a.cmp(b));
    /// assert_eq!(v, [-5, -3, 1, 2, 4]);
    ///
    /// // 反向排序。
    /// v.sort_unstable_by(|a, b| b.cmp(a));
    /// assert_eq!(v, [4, 2, 1, -3, -5]);
    /// ```
    ///
    /// [ipnsort]: https://github.com/Voultapher/sort-research-rs/tree/main/ipnsort
    /// [total order]: https://en.wikipedia.org/wiki/Total_order
    #[stable(feature = "sort_unstable", since = "1.20.0")]
    #[inline]
    pub fn sort_unstable_by<F>(&mut self, mut compare: F)
    where
        F: FnMut(&T, &T) -> Ordering,
    {
        sort::unstable::sort(self, &mut |a, b| compare(a, b) == Ordering::Less);
    }

    /// 使用键提取函数按升序排序切片，**不** 保留相等元素的原始相对顺序。
///
    /// 这是不稳定排序（可能重排相等键元素）、原地排序（不分配），最坏时间复杂度为
    /// *O*(*n* \* log(*n*))。
///
    /// 如果 `K` 的 [`Ord`] 实现不是 [total order]，本函数可能 panic；即使正常返回，
    /// 切片中元素的最终顺序也没有指定含义。另见下面关于 panic 的说明。
///
    /// 例如 `|a, b| (a - b).cmp(a)` 既不传递、也不自反、更不是全序；
    /// 当 `a = 1, b = 2, c = 3` 时会出现 `a < b < c < a`。更多信息和示例见 [`Ord`] 文档。
///
    /// 即使 `K` 的 [`Ord`] 实现 panic，所有原始元素仍会留在切片中；通过内部可变性发生的
    /// 修改也会反映在输入切片里。
///
    /// # 当前实现
///
    /// 当前实现基于 Lukas Bergdoll 和 Orson Peters 的 [ipnsort]。它结合了 quicksort 的
    /// 快速平均情况和 heapsort 的快速最坏情况，在完全有序和完全逆序输入上可达到线性时间。
    /// 对只有 k 个不同键的输入，期望排序时间为 *O*(*n* \* log(*k*))。
///
    /// 除少数特殊情况（例如切片已经部分有序）外，它通常比稳定排序更快。
    ///
    /// # Panics
    ///
    /// 如果 `K` 的 [`Ord`] 实现不是 [total order]，或 [`Ord`] 实现自身 panic，本函数可能 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// let mut v = [4i32, -5, 1, -3, 2];
    ///
    /// v.sort_unstable_by_key(|k| k.abs());
    /// assert_eq!(v, [1, 2, -3, 4, -5]);
    /// ```
    ///
    /// [ipnsort]: https://github.com/Voultapher/sort-research-rs/tree/main/ipnsort
    /// [total order]: https://en.wikipedia.org/wiki/Total_order
    #[stable(feature = "sort_unstable", since = "1.20.0")]
    #[inline]
    pub fn sort_unstable_by_key<K, F>(&mut self, mut f: F)
    where
        F: FnMut(&T) -> K,
        K: Ord,
    {
        sort::unstable::sort(self, &mut |a, b| f(a).lt(&f(b)));
    }

    /// 按升序对切片的一段做部分排序，**不** 保留相等元素的原始相对顺序。
///
    /// 完成后，对指定范围 `start..end` 保证：
///
    /// 1. `self[..start]` 中的每个元素都小于或等于
    /// 2. 已排序的 `self[start..end]` 中的每个元素，并且这些元素又小于或等于
    /// 3. `self[end..]` 中的每个元素。
///
    /// 该部分排序是不稳定的，可能重排指定范围内的相等元素；它也可能重排指定范围之外的元素，
    /// 但上面的分区与范围内有序保证仍然成立。
///
    /// 该部分排序是原地的（不分配），最坏时间复杂度为 *O*(*n* + *k* \* log(*k*))，
    /// 其中 *n* 是切片长度，*k* 是指定范围长度。
///
    /// 实现说明见 [`sort_unstable`] 的文档。
    ///
    /// # Panics
    ///
    /// 如果 `T` 的 [`Ord`] 实现不是 total order、[`Ord`] 实现自身 panic，或指定范围越界，
    /// 本函数可能 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(slice_partial_sort_unstable)]
    ///
    /// let mut v = [4, -5, 1, -3, 2];
    ///
    /// // 开头处的空范围，不改变内容。
    /// v.partial_sort_unstable(0..0);
    /// assert_eq!(v, [4, -5, 1, -3, 2]);
    ///
    /// // 中间的空范围，只对切片进行分区。
    /// v.partial_sort_unstable(2..2);
    /// for i in 0..2 {
    ///    assert!(v[i] <= v[2]);
    /// }
    /// for i in 3..v.len() {
    ///   assert!(v[2] <= v[i]);
    /// }
    ///
    /// // 单元素范围，效果类似 select_nth_unstable。
    /// v.partial_sort_unstable(2..3);
    /// for i in 0..2 {
    ///    assert!(v[i] <= v[2]);
    /// }
    /// for i in 3..v.len() {
    ///   assert!(v[2] <= v[i]);
    /// }
    ///
    /// // 对子范围做部分排序。
    /// v.partial_sort_unstable(1..4);
    /// assert_eq!(&v[1..4], [-3, 1, 2]);
    ///
    /// // 对整个范围做部分排序，等同 sort_unstable。
    /// v.partial_sort_unstable(..);
    /// assert_eq!(v, [-5, -3, 1, 2, 4]);
    /// ```
    ///
    /// [`sort_unstable`]: slice::sort_unstable
    #[unstable(feature = "slice_partial_sort_unstable", issue = "149046")]
    #[inline]
    pub fn partial_sort_unstable<R>(&mut self, range: R)
    where
        T: Ord,
        R: RangeBounds<usize>,
    {
        sort::unstable::partial_sort(self, range, T::lt);
    }

    /// 使用比较函数按升序对切片的一段做部分排序，**不** 保留相等元素的原始相对顺序。
///
    /// 完成后，对指定范围 `start..end` 保证：
///
    /// 1. `self[..start]` 中的每个元素都小于或等于
    /// 2. 已排序的 `self[start..end]` 中的每个元素，并且这些元素又小于或等于
    /// 3. `self[end..]` 中的每个元素。
///
    /// 该部分排序是不稳定的，可能重排指定范围内的相等元素；它也可能重排指定范围之外的元素，
    /// 但上面的保证仍然成立。
///
    /// 该部分排序是原地的（不分配），最坏时间复杂度为 *O*(*n* + *k* \* log(*k*))，
    /// 其中 *n* 是切片长度，*k* 是指定范围长度。
///
    /// 实现说明见 [`sort_unstable_by`] 的文档。
    ///
    /// # Panics
    ///
    /// 如果 `compare` 不构成 total order、`compare` 自身 panic，或指定范围越界，
    /// 本函数可能 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(slice_partial_sort_unstable)]
    ///
    /// let mut v = [4, -5, 1, -3, 2];
    ///
    /// // 开头处的空范围，不改变内容。
    /// v.partial_sort_unstable_by(0..0, |a, b| b.cmp(a));
    /// assert_eq!(v, [4, -5, 1, -3, 2]);
    ///
    /// // 中间的空范围，只对切片进行分区。
    /// v.partial_sort_unstable_by(2..2, |a, b| b.cmp(a));
    /// for i in 0..2 {
    ///    assert!(v[i] >= v[2]);
    /// }
    /// for i in 3..v.len() {
    ///   assert!(v[2] >= v[i]);
    /// }
    ///
    /// // 单元素范围，效果类似 select_nth_unstable。
    /// v.partial_sort_unstable_by(2..3, |a, b| b.cmp(a));
    /// for i in 0..2 {
    ///    assert!(v[i] >= v[2]);
    /// }
    /// for i in 3..v.len() {
    ///   assert!(v[2] >= v[i]);
    /// }
    ///
    /// // 对子范围做部分排序。
    /// v.partial_sort_unstable_by(1..4, |a, b| b.cmp(a));
    /// assert_eq!(&v[1..4], [2, 1, -3]);
    ///
    /// // 对整个范围做部分排序，等同 sort_unstable。
    /// v.partial_sort_unstable_by(.., |a, b| b.cmp(a));
    /// assert_eq!(v, [4, 2, 1, -3, -5]);
    /// ```
    ///
    /// [`sort_unstable_by`]: slice::sort_unstable_by
    #[unstable(feature = "slice_partial_sort_unstable", issue = "149046")]
    #[inline]
    pub fn partial_sort_unstable_by<F, R>(&mut self, range: R, mut compare: F)
    where
        F: FnMut(&T, &T) -> Ordering,
        R: RangeBounds<usize>,
    {
        sort::unstable::partial_sort(self, range, |a, b| compare(a, b) == Less);
    }

    /// 使用键提取函数按升序对切片的一段做部分排序，**不** 保留相等元素的原始相对顺序。
///
    /// 完成后，对指定范围 `start..end` 保证：
///
    /// 1. `self[..start]` 中每个元素的键都小于或等于
    /// 2. 已排序的 `self[start..end]` 中每个元素的键，并且这些键又小于或等于
    /// 3. `self[end..]` 中每个元素的键。
///
    /// 该部分排序是不稳定的，可能重排指定范围内键相等的元素；它也可能重排指定范围之外的元素，
    /// 但上面的保证仍然成立。
///
    /// 该部分排序是原地的（不分配），最坏时间复杂度为 *O*(*n* + *k* \* log(*k*))，
    /// 其中 *n* 是切片长度，*k* 是指定范围长度。
///
    /// 实现说明见 [`sort_unstable_by_key`] 的文档。
    ///
    /// # Panics
    ///
    /// 如果 `K` 的 [`Ord`] 实现不是 total order、[`Ord`] 实现自身 panic，或指定范围越界，
    /// 本函数可能 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(slice_partial_sort_unstable)]
    ///
    /// let mut v = [4i32, -5, 1, -3, 2];
    ///
    /// // 开头处的空范围，不改变内容。
    /// v.partial_sort_unstable_by_key(0..0, |k| k.abs());
    /// assert_eq!(v, [4, -5, 1, -3, 2]);
    ///
    /// // 中间的空范围，只对切片进行分区。
    /// v.partial_sort_unstable_by_key(2..2, |k| k.abs());
    /// for i in 0..2 {
    ///    assert!(v[i].abs() <= v[2].abs());
    /// }
    /// for i in 3..v.len() {
    ///   assert!(v[2].abs() <= v[i].abs());
    /// }
    ///
    /// // 单元素范围，效果类似 select_nth_unstable。
    /// v.partial_sort_unstable_by_key(2..3, |k| k.abs());
    /// for i in 0..2 {
    ///    assert!(v[i].abs() <= v[2].abs());
    /// }
    /// for i in 3..v.len() {
    ///   assert!(v[2].abs() <= v[i].abs());
    /// }
    ///
    /// // 对子范围做部分排序。
    /// v.partial_sort_unstable_by_key(1..4, |k| k.abs());
    /// assert_eq!(&v[1..4], [2, -3, 4]);
    ///
    /// // 对整个范围做部分排序，等同 sort_unstable。
    /// v.partial_sort_unstable_by_key(.., |k| k.abs());
    /// assert_eq!(v, [1, 2, -3, 4, -5]);
    /// ```
    ///
    /// [`sort_unstable_by_key`]: slice::sort_unstable_by_key
    #[unstable(feature = "slice_partial_sort_unstable", issue = "149046")]
    #[inline]
    pub fn partial_sort_unstable_by_key<K, F, R>(&mut self, range: R, mut f: F)
    where
        F: FnMut(&T) -> K,
        K: Ord,
        R: RangeBounds<usize>,
    {
        sort::unstable::partial_sort(self, range, |a, b| f(a).lt(&f(b)));
    }

    /// 重排切片，使 `index` 处元素处在其排序后应在的位置。
    ///
    /// `index` 之前的所有元素都小于或等于该值，`index` 之后的所有元素都大于或等于该值。
///
    /// 该重排是不稳定的（任何与第 n 个元素比较相等的元素都可能落在该位置）、原地的
    /// （不分配），运行时间为 *O*(*n*)。其它库中常称为 “kth element”。
///
    /// 返回一个三元组，对重排后的切片进行分区：
///
    /// * `index` 之前的未排序子切片，其中所有元素都满足 `x <= self[index]`。
///
    /// * `index` 处的元素。
///
    /// * `index` 之后的未排序子切片，其中所有元素都满足 `x >= self[index]`。
///
    /// # 当前实现
///
    /// 当前算法是基于 Lukas Bergdoll 和 Orson Peters 的 [ipnsort] 的 introselect 实现，
    /// [`sort_unstable`] 也以它为基础。fallback 算法是 Median of Medians，并用
    /// Tukey's Ninther 选择枢轴，从而对所有输入保证线性运行时间。
    ///
    /// [`sort_unstable`]: slice::sort_unstable
    ///
    /// # Panics
    ///
    /// 当 `index >= len()` 时 panic，因此空切片上总会 panic。
///
    /// 如果 `T` 的 [`Ord`] 实现不是 [total order]，本函数可能 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// let mut v = [-5i32, 4, 2, -3, 1];
    ///
    /// // 找到小于等于中位数的元素、中位数本身，以及大于等于中位数的元素。
    /// let (lesser, median, greater) = v.select_nth_unstable(2);
    ///
    /// assert!(lesser == [-3, -5] || lesser == [-5, -3]);
    /// assert_eq!(median, &mut 1);
    /// assert!(greater == [4, 2] || greater == [2, 4]);
    ///
    /// // 根据围绕指定索引分区的方式，只保证切片会是下面几种形式之一。
    /// assert!(v == [-3, -5, 1, 2, 4] ||
    ///         v == [-5, -3, 1, 2, 4] ||
    ///         v == [-3, -5, 1, 4, 2] ||
    ///         v == [-5, -3, 1, 4, 2]);
    /// ```
    ///
    /// [ipnsort]: https://github.com/Voultapher/sort-research-rs/tree/main/ipnsort
    /// [total order]: https://en.wikipedia.org/wiki/Total_order
    #[stable(feature = "slice_select_nth_unstable", since = "1.49.0")]
    #[inline]
    pub fn select_nth_unstable(&mut self, index: usize) -> (&mut [T], &mut T, &mut [T])
    where
        T: Ord,
    {
        sort::select::partition_at_index(self, index, T::lt)
    }

    /// 使用比较函数重排切片，使 `index` 处元素处在其排序后应在的位置。
    ///
    /// 按该比较函数，`index` 之前的所有元素都小于或等于该值，`index` 之后的所有元素都
    /// 大于或等于该值。
///
    /// 该重排是不稳定的（任何与第 n 个元素比较相等的元素都可能落在该位置）、原地的
    /// （不分配），运行时间为 *O*(*n*)。其它库中常称为 “kth element”。
///
    /// 返回一个三元组，对重排后的切片进行分区：
///
    /// * `index` 之前的未排序子切片，其中所有元素都满足
    ///   `compare(x, self[index]).is_le()`.
///
    /// * `index` 处的元素。
///
    /// * `index` 之后的未排序子切片，其中所有元素都满足
    ///   `compare(x, self[index]).is_ge()`.
///
    /// # 当前实现
///
    /// 当前算法是基于 Lukas Bergdoll 和 Orson Peters 的 [ipnsort] 的 introselect 实现，
    /// [`sort_unstable`] 也以它为基础。fallback 算法是 Median of Medians，并用
    /// Tukey's Ninther 选择枢轴，从而对所有输入保证线性运行时间。
    ///
    /// [`sort_unstable`]: slice::sort_unstable
    ///
    /// # Panics
    ///
    /// 当 `index >= len()` 时 panic，因此空切片上总会 panic。
///
    /// 如果 `compare` 不构成 [total order]，本函数可能 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// let mut v = [-5i32, 4, 2, -3, 1];
    ///
    /// // 通过反向比较器，找到大于等于中位数的元素、中位数本身，以及小于等于中位数的元素。
    /// let (before, median, after) = v.select_nth_unstable_by(2, |a, b| b.cmp(a));
    ///
    /// assert!(before == [4, 2] || before == [2, 4]);
    /// assert_eq!(median, &mut 1);
    /// assert!(after == [-3, -5] || after == [-5, -3]);
    ///
    /// // 根据围绕指定索引分区的方式，只保证切片会是下面几种形式之一。
    /// assert!(v == [2, 4, 1, -5, -3] ||
    ///         v == [2, 4, 1, -3, -5] ||
    ///         v == [4, 2, 1, -5, -3] ||
    ///         v == [4, 2, 1, -3, -5]);
    /// ```
    ///
    /// [ipnsort]: https://github.com/Voultapher/sort-research-rs/tree/main/ipnsort
    /// [total order]: https://en.wikipedia.org/wiki/Total_order
    #[stable(feature = "slice_select_nth_unstable", since = "1.49.0")]
    #[inline]
    pub fn select_nth_unstable_by<F>(
        &mut self,
        index: usize,
        mut compare: F,
    ) -> (&mut [T], &mut T, &mut [T])
    where
        F: FnMut(&T, &T) -> Ordering,
    {
        sort::select::partition_at_index(self, index, |a: &T, b: &T| compare(a, b) == Less)
    }

    /// 使用键提取函数重排切片，使 `index` 处元素处在按键排序后应在的位置。
    ///
    /// `index` 之前所有元素的键都小于或等于 `index` 处元素的键，之后所有元素的键都
    /// 大于或等于它。
///
    /// 该重排是不稳定的（任何与第 n 个元素键相等的元素都可能落在该位置）、原地的
    /// （不分配），运行时间为 *O*(*n*)。其它库中常称为 “kth element”。
///
    /// 返回一个三元组，对重排后的切片进行分区：
///
    /// * `index` 之前的未排序子切片，其中所有元素都满足 `f(x) <= f(self[index])`。
///
    /// * `index` 处的元素。
///
    /// * `index` 之后的未排序子切片，其中所有元素都满足 `f(x) >= f(self[index])`。
///
    /// # 当前实现
///
    /// 当前算法是基于 Lukas Bergdoll 和 Orson Peters 的 [ipnsort] 的 introselect 实现，
    /// [`sort_unstable`] 也以它为基础。fallback 算法是 Median of Medians，并用
    /// Tukey's Ninther 选择枢轴，从而对所有输入保证线性运行时间。
    ///
    /// [`sort_unstable`]: slice::sort_unstable
    ///
    /// # Panics
    ///
    /// 当 `index >= len()` 时 panic，因此空切片上总会 panic。
///
    /// 如果 `K: Ord` 不构成 total order，本函数可能 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// let mut v = [-5i32, 4, 1, -3, 2];
    ///
    /// // 找到绝对值小于等于绝对中位数的元素、绝对中位数本身，以及绝对值大于等于它的元素。
    /// let (lesser, median, greater) = v.select_nth_unstable_by_key(2, |a| a.abs());
    ///
    /// assert!(lesser == [1, 2] || lesser == [2, 1]);
    /// assert_eq!(median, &mut -3);
    /// assert!(greater == [4, -5] || greater == [-5, 4]);
    ///
    /// // 根据围绕指定索引分区的方式，只保证切片会是下面几种形式之一。
    /// assert!(v == [1, 2, -3, 4, -5] ||
    ///         v == [1, 2, -3, -5, 4] ||
    ///         v == [2, 1, -3, 4, -5] ||
    ///         v == [2, 1, -3, -5, 4]);
    /// ```
    ///
    /// [ipnsort]: https://github.com/Voultapher/sort-research-rs/tree/main/ipnsort
    /// [total order]: https://en.wikipedia.org/wiki/Total_order
    #[stable(feature = "slice_select_nth_unstable", since = "1.49.0")]
    #[inline]
    pub fn select_nth_unstable_by_key<K, F>(
        &mut self,
        index: usize,
        mut f: F,
    ) -> (&mut [T], &mut T, &mut [T])
    where
        F: FnMut(&T) -> K,
        K: Ord,
    {
        sort::select::partition_at_index(self, index, |a: &T, b: &T| f(a).lt(&f(b)))
    }

    /// 根据 [`PartialEq`] trait 实现，把所有连续重复元素移动到切片末尾。
    ///
    /// 返回两个切片。第一个切片不包含连续重复元素；第二个切片包含所有重复元素，
    /// 顺序不作规定。
    ///
    /// 如果切片已经排序，第一个返回切片不包含任何重复元素。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(slice_partition_dedup)]
    ///
    /// let mut slice = [1, 2, 2, 3, 3, 2, 1, 1];
    ///
    /// let (dedup, duplicates) = slice.partition_dedup();
    ///
    /// assert_eq!(dedup, [1, 2, 3, 2, 1]);
    /// assert_eq!(duplicates, [2, 3, 1]);
    /// ```
    #[unstable(feature = "slice_partition_dedup", issue = "54279")]
    #[inline]
    pub fn partition_dedup(&mut self) -> (&mut [T], &mut [T])
    where
        T: PartialEq,
    {
        self.partition_dedup_by(|a, b| a == b)
    }

    /// 按给定相等关系，把连续相等元素中除第一个以外的元素移动到切片末尾。
    ///
    /// 返回两个切片。第一个切片不包含连续重复元素；第二个切片包含所有重复元素，
    /// 顺序不作规定。
    ///
    /// `same_bucket` 函数会收到来自切片的两个元素引用，并判断它们是否应视为相等。
    /// 两个元素的传入顺序与它们在切片中的顺序相反，因此如果 `same_bucket(a, b)`
    /// 返回 `true`，`a` 会被移动到切片末尾。
    ///
    /// 如果切片已经排序，第一个返回切片不包含任何重复元素。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(slice_partition_dedup)]
    ///
    /// let mut slice = ["foo", "Foo", "BAZ", "Bar", "bar", "baz", "BAZ"];
    ///
    /// let (dedup, duplicates) = slice.partition_dedup_by(|a, b| a.eq_ignore_ascii_case(b));
    ///
    /// assert_eq!(dedup, ["foo", "BAZ", "Bar", "baz"]);
    /// assert_eq!(duplicates, ["bar", "Foo", "BAZ"]);
    /// ```
    #[unstable(feature = "slice_partition_dedup", issue = "54279")]
    #[inline]
    pub fn partition_dedup_by<F>(&mut self, mut same_bucket: F) -> (&mut [T], &mut [T])
    where
        F: FnMut(&mut T, &mut T) -> bool,
    {
        // 虽然这里拥有 `self` 的可变引用，但不能做*任意*修改。
        // `same_bucket` 调用可能 panic，因此必须始终确保切片处于有效状态。
        //
        // 处理方式是使用交换：遍历所有元素，并在过程中交换，
        // 使最终希望保留的元素位于前部，希望剔除的元素位于后部。
        // 随后即可分割切片。该操作仍是 `O(n)`。
        //
        // 示例：从如下状态开始，其中 `r` 表示“下一个读取位置”，
        // `w` 表示“next_write”。
        //
        //           r
        //     +---+---+---+---+---+---+
        //     | 0 | 1 | 1 | 2 | 3 | 3 |
        //     +---+---+---+---+---+---+
        //           w
        //
        // 比较 self[r] 与 self[w-1]，这不是重复元素，
        // 因此交换 self[r] 和 self[w]（由于 r==w，没有实际效果），
        // 然后同时递增 r 和 w，得到：
        //
        //               r
        //     +---+---+---+---+---+---+
        //     | 0 | 1 | 1 | 2 | 3 | 3 |
        //     +---+---+---+---+---+---+
        //               w
        //
        // 比较 self[r] 与 self[w-1]，该值是重复元素，
        // 因此只递增 `r`，其余保持不变：
        //
        //                   r
        //     +---+---+---+---+---+---+
        //     | 0 | 1 | 1 | 2 | 3 | 3 |
        //     +---+---+---+---+---+---+
        //               w
        //
        // 比较 self[r] 与 self[w-1]，这不是重复元素，
        // 因此交换 self[r] 和 self[w]，并前进 r 和 w：
        //
        //                       r
        //     +---+---+---+---+---+---+
        //     | 0 | 1 | 2 | 1 | 3 | 3 |
        //     +---+---+---+---+---+---+
        //                   w
        //
        // 不是重复元素，重复上述步骤：
        //
        //                           r
        //     +---+---+---+---+---+---+
        //     | 0 | 1 | 2 | 3 | 1 | 3 |
        //     +---+---+---+---+---+---+
        //                       w
        //
        // 是重复元素，前进 r。到达切片末尾后，在 w 处分割。

        let len = self.len();
        if len <= 1 {
            return (self, &mut []);
        }

        let ptr = self.as_mut_ptr();
        let mut next_read: usize = 1;
        let mut next_write: usize = 1;

        // SAFETY: `while` 条件保证 `next_read` 和 `next_write` 都小于 `len`，
        // 因而位于 `self` 内。`prev_ptr_write` 指向 `ptr_write` 前一个元素，
        // 但 `next_write` 从 1 开始，因此 `prev_ptr_write` 永远不会小于 0，
        // 并且位于切片内。这满足解引用 `ptr_read`、`prev_ptr_write` 和 `ptr_write`
        // 以及使用 `ptr.add(next_read)`、`ptr.add(next_write - 1)` 和
        // `prev_ptr_write.offset(1)` 的要求。
        //
        // `next_write` 每轮循环也至多递增一次，这意味着需要交换的元素不会被跳过。
        //
        // `ptr_read` 和 `prev_ptr_write` 永远不会指向同一元素。
        // 这是 `&mut *ptr_read`、`&mut *prev_ptr_write` 安全所必需的。
        // 理由很简单：`next_read >= next_write` 始终为真，
        // 因此 `next_read > next_write - 1` 也为真。
        unsafe {
            // 使用裸指针避免边界检查。
            while next_read < len {
                let ptr_read = ptr.add(next_read);
                let prev_ptr_write = ptr.add(next_write - 1);
                if !same_bucket(&mut *ptr_read, &mut *prev_ptr_write) {
                    if next_read != next_write {
                        let ptr_write = prev_ptr_write.add(1);
                        mem::swap(&mut *ptr_read, &mut *ptr_write);
                    }
                    next_write += 1;
                }
                next_read += 1;
            }
        }

        self.split_at_mut(next_write)
    }

    /// 按键提取结果，把连续相同 key 的元素中除第一个以外的元素移动到切片末尾。
    ///
    /// 返回两个切片。第一个切片不包含连续重复元素；第二个切片包含所有重复元素，
    /// 顺序不作规定。
    ///
    /// 如果切片已经排序，第一个返回切片不包含任何重复元素。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(slice_partition_dedup)]
    ///
    /// let mut slice = [10, 20, 21, 30, 30, 20, 11, 13];
    ///
    /// let (dedup, duplicates) = slice.partition_dedup_by_key(|i| *i / 10);
    ///
    /// assert_eq!(dedup, [10, 20, 30, 20, 11]);
    /// assert_eq!(duplicates, [21, 30, 13]);
    /// ```
    #[unstable(feature = "slice_partition_dedup", issue = "54279")]
    #[inline]
    pub fn partition_dedup_by_key<K, F>(&mut self, mut key: F) -> (&mut [T], &mut [T])
    where
        F: FnMut(&mut T) -> K,
        K: PartialEq,
    {
        self.partition_dedup_by(|a, b| key(a) == key(b))
    }

    /// 原地旋转切片，使前 `mid` 个元素移动到末尾，而后 `self.len() - mid`
    /// 个元素移动到开头。
    ///
    /// 调用 `rotate_left` 后，原先位于索引 `mid` 的元素会成为切片中的第一个元素。
    ///
    /// # Panics
    ///
    /// 如果 `mid` 大于切片长度，本函数会 panic。注意 `mid == self.len()` 不会 panic，
    /// 并且是不做任何事的旋转。
    ///
    /// # 复杂度
    ///
    /// 耗时与 `self.len()` 线性相关。
    ///
    /// # 示例
    ///
    /// ```
    /// let mut a = ['a', 'b', 'c', 'd', 'e', 'f'];
    /// a.rotate_left(2);
    /// assert_eq!(a, ['c', 'd', 'e', 'f', 'a', 'b']);
    /// ```
    ///
    /// 旋转子切片：
    ///
    /// ```
    /// let mut a = ['a', 'b', 'c', 'd', 'e', 'f'];
    /// a[1..5].rotate_left(1);
    /// assert_eq!(a, ['a', 'c', 'd', 'e', 'b', 'f']);
    /// ```
    #[stable(feature = "slice_rotate", since = "1.26.0")]
    #[rustc_const_stable(feature = "const_slice_rotate", since = "1.92.0")]
    pub const fn rotate_left(&mut self, mid: usize) {
        assert!(mid <= self.len());
        let k = self.len() - mid;
        let p = self.as_mut_ptr();

        // SAFETY: 范围 `[p.add(mid) - mid, p.add(mid) + k)` 显然对读写有效，
        // 满足 `ptr_rotate` 的要求。
        unsafe {
            rotate::ptr_rotate(mid, p.add(mid), k);
        }
    }

    /// 原地旋转切片，使前 `self.len() - k` 个元素移动到末尾，而后 `k` 个元素
    /// 移动到开头。
    ///
    /// 调用 `rotate_right` 后，原先位于索引 `self.len() - k` 的元素会成为
    /// 切片中的第一个元素。
    ///
    /// # Panics
    ///
    /// 如果 `k` 大于切片长度，本函数会 panic。注意 `k == self.len()` 不会 panic，
    /// 并且是不做任何事的旋转。
    ///
    /// # 复杂度
    ///
    /// 耗时与 `self.len()` 线性相关。
    ///
    /// # 示例
    ///
    /// ```
    /// let mut a = ['a', 'b', 'c', 'd', 'e', 'f'];
    /// a.rotate_right(2);
    /// assert_eq!(a, ['e', 'f', 'a', 'b', 'c', 'd']);
    /// ```
    ///
    /// 旋转子切片：
    ///
    /// ```
    /// let mut a = ['a', 'b', 'c', 'd', 'e', 'f'];
    /// a[1..5].rotate_right(1);
    /// assert_eq!(a, ['a', 'e', 'b', 'c', 'd', 'f']);
    /// ```
    #[stable(feature = "slice_rotate", since = "1.26.0")]
    #[rustc_const_stable(feature = "const_slice_rotate", since = "1.92.0")]
    pub const fn rotate_right(&mut self, k: usize) {
        assert!(k <= self.len());
        let mid = self.len() - k;
        let p = self.as_mut_ptr();

        // SAFETY: 范围 `[p.add(mid) - mid, p.add(mid) + k)` 显然对读写有效，
        // 满足 `ptr_rotate` 的要求。
        unsafe {
            rotate::ptr_rotate(mid, p.add(mid), k);
        }
    }

    /// 通过克隆 `value` 填充 `self` 的所有元素。
    ///
    /// # 示例
    ///
    /// ```
    /// let mut buf = vec![0; 10];
    /// buf.fill(1);
    /// assert_eq!(buf, vec![1; 10]);
    /// ```
    #[doc(alias = "memset")]
    #[stable(feature = "slice_fill", since = "1.50.0")]
    pub fn fill(&mut self, value: T)
    where
        T: Clone,
    {
        specialize::SpecFill::spec_fill(self, value);
    }

    /// 通过反复调用闭包返回的元素填充 `self`。
    ///
    /// 本方法使用闭包创建新值。如果更想 [`Clone`] 某个给定值，请使用 [`fill`]。
    /// 如果想用 [`Default`] trait 生成值，可以把 [`Default::default`] 作为参数传入。
    ///
    /// [`fill`]: slice::fill
    ///
    /// # 示例
    ///
    /// ```
    /// let mut buf = vec![1; 10];
    /// buf.fill_with(Default::default);
    /// assert_eq!(buf, vec![0; 10]);
    /// ```
    #[stable(feature = "slice_fill_with", since = "1.51.0")]
    pub fn fill_with<F>(&mut self, mut f: F)
    where
        F: FnMut() -> T,
    {
        for el in self {
            *el = f();
        }
    }

    /// 把 `src` 中的元素复制到 `self`。
    ///
    /// `src` 的长度必须与 `self` 相同。
    ///
    /// # Panics
    ///
    /// 如果两个切片长度不同，本函数会 panic。
    ///
    /// # 示例
    ///
    /// 从一个切片向另一个切片 clone 两个元素：
    ///
    /// ```
    /// let src = [1, 2, 3, 4];
    /// let mut dst = [0, 0];
    ///
    /// // 两个切片长度必须相同，因此把源切片从四个元素裁成两个。
    /// // 如果不这样做，会 panic。
    /// dst.clone_from_slice(&src[2..]);
    ///
    /// assert_eq!(src, [1, 2, 3, 4]);
    /// assert_eq!(dst, [3, 4]);
    /// ```
    ///
    /// Rust 要求某一作用域内同一块数据只能存在一个可变引用，且不能同时存在不可变引用。
    /// 因此，尝试在同一个切片上使用 `clone_from_slice` 会导致编译失败：
    ///
    /// ```compile_fail
    /// let mut slice = [1, 2, 3, 4, 5];
    ///
    /// slice[..2].clone_from_slice(&slice[3..]); // 编译失败！
    /// ```
    ///
    /// 为绕过这一点，可以使用 [`split_at_mut`] 从一个切片创建两个互不重叠的子切片：
    ///
    /// ```
    /// let mut slice = [1, 2, 3, 4, 5];
    ///
    /// {
    ///     let (left, right) = slice.split_at_mut(2);
    ///     left.clone_from_slice(&right[1..]);
    /// }
    ///
    /// assert_eq!(slice, [4, 5, 3, 4, 5]);
    /// ```
    ///
    /// [`copy_from_slice`]: slice::copy_from_slice
    /// [`split_at_mut`]: slice::split_at_mut
    #[stable(feature = "clone_from_slice", since = "1.7.0")]
    #[track_caller]
    #[rustc_const_unstable(feature = "const_clone", issue = "142757")]
    pub const fn clone_from_slice(&mut self, src: &[T])
    where
        T: [const] Clone + [const] Destruct,
    {
        self.spec_clone_from(src);
    }

    /// 使用 memcpy，把 `src` 中的所有元素复制到 `self`。
    ///
    /// `src` 的长度必须与 `self` 相同。
    ///
    /// 如果 `T` 未实现 `Copy`，请使用 [`clone_from_slice`]。
    ///
    /// # Panics
    ///
    /// 如果两个切片长度不同，本函数会 panic。
    ///
    /// # 示例
    ///
    /// 从一个切片向另一个切片复制两个元素：
    ///
    /// ```
    /// let src = [1, 2, 3, 4];
    /// let mut dst = [0, 0];
    ///
    /// // 两个切片长度必须相同，因此把源切片从四个元素裁成两个。
    /// // 如果不这样做，会 panic。
    /// dst.copy_from_slice(&src[2..]);
    ///
    /// assert_eq!(src, [1, 2, 3, 4]);
    /// assert_eq!(dst, [3, 4]);
    /// ```
    ///
    /// Rust 要求某一作用域内同一块数据只能存在一个可变引用，且不能同时存在不可变引用。
    /// 因此，尝试在同一个切片上使用 `copy_from_slice` 会导致编译失败：
    ///
    /// ```compile_fail
    /// let mut slice = [1, 2, 3, 4, 5];
    ///
    /// slice[..2].copy_from_slice(&slice[3..]); // 编译失败！
    /// ```
    ///
    /// 为绕过这一点，可以使用 [`split_at_mut`] 从一个切片创建两个互不重叠的子切片：
    ///
    /// ```
    /// let mut slice = [1, 2, 3, 4, 5];
    ///
    /// {
    ///     let (left, right) = slice.split_at_mut(2);
    ///     left.copy_from_slice(&right[1..]);
    /// }
    ///
    /// assert_eq!(slice, [4, 5, 3, 4, 5]);
    /// ```
    ///
    /// [`clone_from_slice`]: slice::clone_from_slice
    /// [`split_at_mut`]: slice::split_at_mut
    #[doc(alias = "memcpy")]
    #[inline]
    #[stable(feature = "copy_from_slice", since = "1.9.0")]
    #[rustc_const_stable(feature = "const_copy_from_slice", since = "1.87.0")]
    #[track_caller]
    pub const fn copy_from_slice(&mut self, src: &[T])
    where
        T: Copy,
    {
        // SAFETY: `T` 实现 `Copy`。
        unsafe { copy_from_slice_impl(self, src) }
    }

    /// 使用 memmove，把切片中某一部分的元素复制到它自身的另一部分。
    ///
    /// `src` 是 `self` 内要复制的源范围。`dest` 是 `self` 内目标范围的起始索引；
    /// 目标范围长度与 `src` 相同。两个范围可以重叠。两个范围的末端都必须小于或等于
    /// `self.len()`。
    ///
    /// # Panics
    ///
    /// 如果任一范围超出切片末尾，或 `src` 的结束位置位于起始位置之前，本函数会 panic。
    ///
    /// # 示例
    ///
    /// 在一个切片内部复制四个字节：
    ///
    /// ```
    /// let mut bytes = *b"Hello, World!";
    ///
    /// bytes.copy_within(1..5, 8);
    ///
    /// assert_eq!(&bytes, b"Hello, Wello!");
    /// ```
    #[stable(feature = "copy_within", since = "1.37.0")]
    #[track_caller]
    pub fn copy_within<R: RangeBounds<usize>>(&mut self, src: R, dest: usize)
    where
        T: Copy,
    {
        let Range { start: src_start, end: src_end } = slice::range(src, ..self.len());
        let count = src_end - src_start;
        assert!(dest <= self.len() - count, "dest is out of bounds");
        // SAFETY: 上面已经检查过 `ptr::copy` 的条件，
        // 也检查过 `ptr::add` 的条件。
        unsafe {
            // 从同一个 loan 派生 `src_ptr` 和 `dest_ptr`。
            let ptr = self.as_mut_ptr();
            let src_ptr = ptr.add(src_start);
            let dest_ptr = ptr.add(dest);
            ptr::copy(src_ptr, dest_ptr, count);
        }
    }

    /// 将 `self` 中的所有元素与 `other` 中的元素交换。
    ///
    /// `other` 的长度必须与 `self` 相同。
    ///
    /// # Panics
    ///
    /// 如果两个切片长度不同，本函数会 panic。
    ///
    /// # 示例
    ///
    /// 在两个切片之间交换两个元素：
    ///
    /// ```
    /// let mut slice1 = [0, 0];
    /// let mut slice2 = [1, 2, 3, 4];
    ///
    /// slice1.swap_with_slice(&mut slice2[2..]);
    ///
    /// assert_eq!(slice1, [3, 4]);
    /// assert_eq!(slice2, [1, 2, 0, 0]);
    /// ```
    ///
    /// Rust 要求某一作用域内同一块数据只能存在一个可变引用。因此，尝试在同一个切片上
    /// 使用 `swap_with_slice` 会导致编译失败：
    ///
    /// ```compile_fail
    /// let mut slice = [1, 2, 3, 4, 5];
    /// slice[..2].swap_with_slice(&mut slice[3..]); // 编译失败！
    /// ```
    ///
    /// 为绕过这一点，可以使用 [`split_at_mut`] 从一个切片创建两个互不重叠的可变子切片：
    ///
    /// ```
    /// let mut slice = [1, 2, 3, 4, 5];
    ///
    /// {
    ///     let (left, right) = slice.split_at_mut(2);
    ///     left.swap_with_slice(&mut right[1..]);
    /// }
    ///
    /// assert_eq!(slice, [4, 5, 3, 1, 2]);
    /// ```
    ///
    /// [`split_at_mut`]: slice::split_at_mut
    #[stable(feature = "swap_with_slice", since = "1.27.0")]
    #[rustc_const_unstable(feature = "const_swap_with_slice", issue = "142204")]
    #[track_caller]
    pub const fn swap_with_slice(&mut self, other: &mut [T]) {
        assert!(self.len() == other.len(), "destination and source slices have different lengths");
        // SAFETY: 根据定义，`self` 对 `self.len()` 个元素有效；另一个切片也已检查
        // 为相同长度。两个切片不能重叠，因为可变引用具有排他性。
        unsafe {
            ptr::swap_nonoverlapping(self.as_mut_ptr(), other.as_mut_ptr(), self.len());
        }
    }

    /// 计算 `align_to{,_mut}` 中间切片和尾部切片长度的函数。
    fn align_to_offsets<U>(&self) -> (usize, usize) {
        // 对 `rest` 要做的是：找出最少多少个 `T` 可容纳若干个完整 `U`，
        // 以及每个这样的“倍数”需要多少个 `T`。
        //
        // 例如 T=u8、U=u16，则 2 个 T 可容纳 1 个 U，很简单。
        // 再考虑 size_of::<T> = 16、size_of::<U> = 24 的情况：
        // 在 `rest` 切片中，每 3 个 T 的位置可放入 2 个 U，稍微复杂一些。
        //
        // 计算公式是：
        //
        // Us = lcm(size_of::<T>, size_of::<U>) / size_of::<U>
        // Ts = lcm(size_of::<T>, size_of::<U>) / size_of::<T>
        //
        // 展开并简化后：
        //
        // Us = size_of::<T> / gcd(size_of::<T>, size_of::<U>)
        // Ts = size_of::<U> / gcd(size_of::<T>, size_of::<U>)
        //
        // 由于这些都会被常量求值，性能在这里并不重要。
        const fn gcd(a: usize, b: usize) -> usize {
            if b == 0 { a } else { gcd(b, a % b) }
        }

        // 显式把函数调用包进 const block，使其即使在 debug 模式下也会被常量求值。
        let gcd: usize = const { gcd(size_of::<T>(), size_of::<U>()) };
        let ts: usize = size_of::<U>() / gcd;
        let us: usize = size_of::<T>() / gcd;

        // 有了上述比例，就能算出中间段能容纳多少个 `U`。
        let us_len = self.len() / ts * us;
        // 同时算出尾部切片还会留下多少个 `T`。
        let ts_len = self.len() % ts;
        (us_len, ts_len)
    }

    /// 把切片中对齐合适的一段重新解释成另一种元素类型的切片。
///
    /// 本方法把原切片分成三个互不重叠的切片：前缀、正确对齐且元素类型为 `U` 的中间切片、
    /// 以及后缀。在给定对齐和元素大小约束下，中间部分会尽可能大。
///
    /// 当输入元素 `T` 或输出元素 `U` 是 zero-sized 时，本方法没有实际意义，并会原样返回
    /// 原切片，不做任何拆分。
    ///
    /// # 安全性(Safety）
    ///
    /// 对返回的中间切片而言，本方法本质上是 `transmute::<T, U>`。调用方必须保证中间段的
    /// 每个 `U` 位模式都是有效的 `U`，并且把同一字节范围视为 `U` 不会违反 `U` 的布局、
    /// 对齐、初始化和 aliasing 规则。`align_to` 只负责寻找满足 `U` 对齐的位置，不会证明
    /// `T` 的字节序列能合法表示 `U`。
    ///
    /// # 示例
    ///
    /// 基本用法：
    ///
    /// ```
    /// unsafe {
    ///     let bytes: [u8; 7] = [1, 2, 3, 4, 5, 6, 7];
    ///     let (prefix, shorts, suffix) = bytes.align_to::<u16>();
    ///     // less_efficient_algorithm_for_bytes(prefix);
    ///     // more_efficient_algorithm_for_aligned_shorts(shorts);
    ///     // less_efficient_algorithm_for_bytes(suffix);
    /// }
    /// ```
    #[stable(feature = "slice_align_to", since = "1.30.0")]
    #[must_use]
    pub unsafe fn align_to<U>(&self) -> (&[T], &[U], &[T]) {
        // 注意，本函数的大部分计算都会被常量求值。
        if U::IS_ZST || T::IS_ZST {
            // 对 ZST 做特殊处理：不尝试重新解释它们。
            return (self, &[], &[]);
        }

        // 首先找到前缀和中间切片之间的分割点；这正是 ptr.align_offset 的用途。
        let ptr = self.as_ptr();
        // SAFETY: 详细安全理由见 `align_to_mut` 中对 `align_offset` 的说明。
        let offset = unsafe { crate::ptr::align_offset(ptr, align_of::<U>()) };
        if offset > self.len() {
            (self, &[], &[])
        } else {
            let (left, rest) = self.split_at(offset);
            let (us_len, ts_len) = rest.align_to_offsets::<U>();
            // 告知 Miri：我们希望把“中间”指针视为已经满足所需对齐。
            #[cfg(miri)]
            crate::intrinsics::miri_promise_symbolic_alignment(
                rest.as_ptr().cast(),
                align_of::<U>(),
            );
            // SAFETY: 此时 `rest` 起始地址已满足 `U` 的对齐，`align_to_offsets` 计算出的
            // `us_len` 与 `ts_len` 保证中间 `U` 切片和尾部 `T` 切片都落在原切片范围内。
            // 调用方还必须保证把中间字节重新解释为 `U` 是合法的。
            unsafe {
                (
                    left,
                    from_raw_parts(rest.as_ptr() as *const U, us_len),
                    from_raw_parts(rest.as_ptr().add(rest.len() - ts_len), ts_len),
                )
            }
        }
    }

    /// 把可变切片中对齐合适的一段重新解释成另一种元素类型的可变切片。
///
    /// 本方法把原切片分成三个互不重叠的可变切片：前缀、正确对齐且元素类型为 `U` 的中间切片、
    /// 以及后缀。在给定对齐和元素大小约束下，中间部分会尽可能大。
///
    /// 当输入元素 `T` 或输出元素 `U` 是 zero-sized 时，本方法没有实际意义，并会原样返回
    /// 原切片，不做任何拆分。
    ///
    /// # 安全性(Safety）
    ///
    /// 对返回的中间可变切片而言，本方法本质上是 `transmute::<T, U>`。调用方必须保证中间段
    /// 的每个 `U` 位模式有效，并且后续通过 `&mut [U]` 读写不会破坏 `T` 或 `U` 的有效性、
    /// drop 语义、对齐要求或 aliasing 规则。可变版本还要求中间 `&mut [U]` 与前缀/后缀
    /// `&mut [T]` 互不重叠，且没有其它活跃引用访问同一字节范围。
    ///
    /// # 示例
    ///
    /// 基本用法：
    ///
    /// ```
    /// unsafe {
    ///     let mut bytes: [u8; 7] = [1, 2, 3, 4, 5, 6, 7];
    ///     let (prefix, shorts, suffix) = bytes.align_to_mut::<u16>();
    ///     // less_efficient_algorithm_for_bytes(prefix);
    ///     // more_efficient_algorithm_for_aligned_shorts(shorts);
    ///     // less_efficient_algorithm_for_bytes(suffix);
    /// }
    /// ```
    #[stable(feature = "slice_align_to", since = "1.30.0")]
    #[must_use]
    pub unsafe fn align_to_mut<U>(&mut self) -> (&mut [T], &mut [U], &mut [T]) {
        // 注意，本函数的大部分计算都会被常量求值。
        if U::IS_ZST || T::IS_ZST {
            // 对 ZST 做特殊处理：不尝试重新解释它们。
            return (self, &mut [], &mut []);
        }

        // 首先找到前缀和中间切片之间的分割点；这正是 ptr.align_offset 的用途。
        let ptr = self.as_ptr();
        // SAFETY: 这里通过以 `U` 的对齐为目标调用 `align_offset`，确保后续中间段使用的
        // `U` 指针满足对齐要求。`ptr` 来自 `self` 引用，因此是有效且按 `T` 对齐的指针；
        // `align_of::<U>()` 是 2 的幂，满足 `align_offset` 的安全约束。
        let offset = unsafe { crate::ptr::align_offset(ptr, align_of::<U>()) };
        if offset > self.len() {
            (self, &mut [], &mut [])
        } else {
            let (left, rest) = self.split_at_mut(offset);
            let (us_len, ts_len) = rest.align_to_offsets::<U>();
            let rest_len = rest.len();
            let mut_ptr = rest.as_mut_ptr();
            // 告知 Miri：我们希望把“中间”指针视为已经满足所需对齐。
            #[cfg(miri)]
            crate::intrinsics::miri_promise_symbolic_alignment(
                mut_ptr.cast() as *const (),
                align_of::<U>(),
            );
            // 之后不能再使用 `rest`，否则会使它与 `mut_ptr` 形成冲突 alias。
            // SAFETY: 见 `align_to` 的说明；这里还依赖 `split_at_mut` 保证三个返回区域互不重叠。
            unsafe {
                (
                    left,
                    from_raw_parts_mut(mut_ptr as *mut U, us_len),
                    from_raw_parts_mut(mut_ptr.add(rest_len - ts_len), ts_len),
                )
            }
        }
    }

    /// 把切片拆成前缀、对齐的 SIMD 类型中间段和后缀。
///
    /// 这是 [`slice::align_to`] 的安全包装，因此继承该方法的布局和对齐保证。
    /// SIMD 类型与 `[T; LANES]` 布局一致，只是可能有更高对齐要求。
    ///
    /// # Panics
    ///
    /// 如果 SIMD 类型大小不同于标量大小乘以 `LANES`，本函数会 panic。
///
    /// 撰写本文档时，`Simd<T, LANES>` 的 trait 限制会防止这种情况发生，因为只支持
    /// 2 的幂数量的 lanes。未来如果放宽这些限制，像 `LANES == 3` 这样的情况可能让
    /// 本方法出现 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(portable_simd)]
    /// use core::simd::prelude::*;
    ///
    /// let short = &[1, 2, 3];
    /// let (prefix, middle, suffix) = short.as_simd::<4>();
    /// assert_eq!(middle, []); // 元素不够，无法形成任何中间 SIMD 段。
    ///
    /// // 这些元素可能以任意方式分布在 prefix 和 suffix 之间。
    /// let it = prefix.iter().chain(suffix).copied();
    /// assert_eq!(it.collect::<Vec<_>>(), vec![1, 2, 3]);
    ///
    /// fn basic_simd_sum(x: &[f32]) -> f32 {
    ///     use std::ops::Add;
    ///     let (prefix, middle, suffix) = x.as_simd();
    ///     let sums = f32x4::from_array([
    ///         prefix.iter().copied().sum(),
    ///         0.0,
    ///         0.0,
    ///         suffix.iter().copied().sum(),
    ///     ]);
    ///     let sums = middle.iter().copied().fold(sums, f32x4::add);
    ///     sums.reduce_sum()
    /// }
    ///
    /// let numbers: Vec<f32> = (1..101).map(|x| x as _).collect();
    /// assert_eq!(basic_simd_sum(&numbers[1..99]), 4949.0);
    /// ```
    #[unstable(feature = "portable_simd", issue = "86656")]
    #[must_use]
    pub fn as_simd<const LANES: usize>(&self) -> (&[T], &[Simd<T, LANES>], &[T])
    where
        Simd<T, LANES>: AsRef<[T; LANES]>,
        T: simd::SimdElement,
        simd::LaneCount<LANES>: simd::SupportedLaneCount,
    {
        // 根据 <https://llvm.org/docs/LangRef.html#vector-type>，vector 类型按数组形式布局，
        // 因而这些大小预期总是匹配；这里再检查一次，优化后也会被消除。
        assert_eq!(size_of::<Simd<T, LANES>>(), size_of::<[T; LANES]>());

        // SAFETY: simd 类型与数组布局相同，只是可能有更高对齐；`align_to` 会处理对齐，
        // 因而这里事实上的 transmute 是健全的。
        unsafe { self.align_to() }
    }

    /// 把可变切片拆成可变前缀、对齐的 SIMD 类型中间段和可变后缀。
///
    /// 这是 [`slice::align_to_mut`] 的安全包装，因此继承该方法的布局、对齐和不重叠保证。
///
    /// 这是 [`slice::as_simd`] 的可变版本；示例见该方法。
    ///
    /// # Panics
    ///
    /// 如果 SIMD 类型大小不同于标量大小乘以 `LANES`，本函数会 panic。
///
    /// 撰写本文档时，`Simd<T, LANES>` 的 trait 限制会防止这种情况发生，因为只支持
    /// 2 的幂数量的 lanes。未来如果放宽这些限制，像 `LANES == 3` 这样的情况可能让
    /// 本方法出现 panic。
    #[unstable(feature = "portable_simd", issue = "86656")]
    #[must_use]
    pub fn as_simd_mut<const LANES: usize>(&mut self) -> (&mut [T], &mut [Simd<T, LANES>], &mut [T])
    where
        Simd<T, LANES>: AsMut<[T; LANES]>,
        T: simd::SimdElement,
        simd::LaneCount<LANES>: simd::SupportedLaneCount,
    {
        // 根据 <https://llvm.org/docs/LangRef.html#vector-type>，vector 类型按数组形式布局，
        // 因而这些大小预期总是匹配；这里再检查一次，优化后也会被消除。
        assert_eq!(size_of::<Simd<T, LANES>>(), size_of::<[T; LANES]>());

        // SAFETY: simd 类型与数组布局相同，只是可能有更高对齐；`align_to_mut` 会处理对齐
        // 并返回不重叠区域，因而这里事实上的 transmute 是健全的。
        unsafe { self.align_to_mut() }
    }

    /// 检查切片元素是否已经排序。
///
    /// 也就是说，对于每个元素 `a` 及其后继元素 `b`，都必须满足 `a <= b`。
    /// 如果切片长度为 0 或 1，返回 `true`。
///
    /// 注意，如果元素只实现 [`PartialOrd`] 而不是 [`Ord`]，上述定义意味着只要任意两个
    /// 相邻元素不可比较，本函数就会返回 `false`。
    ///
    /// # 示例
    ///
    /// ```
    /// let empty: [i32; 0] = [];
    ///
    /// assert!([1, 2, 2, 9].is_sorted());
    /// assert!(![1, 3, 2, 4].is_sorted());
    /// assert!([0].is_sorted());
    /// assert!(empty.is_sorted());
    /// assert!(![0.0, 1.0, f32::NAN].is_sorted());
    /// ```
    #[inline]
    #[stable(feature = "is_sorted", since = "1.82.0")]
    #[must_use]
    pub fn is_sorted(&self) -> bool
    where
        T: PartialOrd,
    {
        // 这个奇数效果最好：32 个元素，加 1 个用于重叠 chunk 边界。
        const CHUNK_SIZE: usize = 33;
        if self.len() < CHUNK_SIZE {
            return self.windows(2).all(|w| w[0] <= w[1]);
        }
        let mut i = 0;
        // 分块检查以利于自动向量化。
        while i < self.len() - CHUNK_SIZE {
            let chunk = &self[i..i + CHUNK_SIZE];
            if !chunk.windows(2).fold(true, |acc, w| acc & (w[0] <= w[1])) {
                return false;
            }
            // 还需要确保 chunk 边界也是有序的，因此让下一个 chunk 与上一个 chunk 的最后元素重叠。
            i += CHUNK_SIZE - 1;
        }
        self[i..].windows(2).all(|w| w[0] <= w[1])
    }

    /// 使用给定比较函数检查切片元素是否已经排序。
///
    /// 本函数不使用 `PartialOrd::partial_cmp`，而是用给定 `compare` 函数判断相邻元素是否
    /// 应被视为有序。
    ///
    /// # 示例
    ///
    /// ```
    /// assert!([1, 2, 2, 9].is_sorted_by(|a, b| a <= b));
    /// assert!(![1, 2, 2, 9].is_sorted_by(|a, b| a < b));
    ///
    /// assert!([0].is_sorted_by(|a, b| true));
    /// assert!([0].is_sorted_by(|a, b| false));
    ///
    /// let empty: [i32; 0] = [];
    /// assert!(empty.is_sorted_by(|a, b| false));
    /// assert!(empty.is_sorted_by(|a, b| true));
    /// ```
    #[stable(feature = "is_sorted", since = "1.82.0")]
    #[must_use]
    pub fn is_sorted_by<'a, F>(&'a self, mut compare: F) -> bool
    where
        F: FnMut(&'a T, &'a T) -> bool,
    {
        self.array_windows().all(|[a, b]| compare(a, b))
    }

    /// 使用给定键提取函数检查切片元素是否已经按键排序。
///
    /// 本函数不直接比较切片元素，而是比较由 `f` 提取出的键。除此之外，它等价于
    /// [`is_sorted`]；更多信息见该方法文档。
    ///
    /// [`is_sorted`]: slice::is_sorted
    ///
    /// # 示例
    ///
    /// ```
    /// assert!(["c", "bb", "aaa"].is_sorted_by_key(|s| s.len()));
    /// assert!(![-2i32, -1, 0, 3].is_sorted_by_key(|n| n.abs()));
    /// ```
    #[inline]
    #[stable(feature = "is_sorted", since = "1.82.0")]
    #[must_use]
    pub fn is_sorted_by_key<'a, F, K>(&'a self, f: F) -> bool
    where
        F: FnMut(&'a T) -> K,
        K: PartialOrd,
    {
        self.iter().is_sorted_by_key(f)
    }

    /// 按给定谓词返回分区点索引，也就是第二个分区第一个元素的索引。
///
    /// 调用前提是切片已经按该谓词分区：所有让谓词返回 true 的元素位于切片开头，
    /// 所有让谓词返回 false 的元素位于切片末尾。例如 `[7, 15, 3, 5, 4, 12, 6]`
    /// 按谓词 `x % 2 != 0` 分区（所有奇数在前，所有偶数在后）。
///
    /// 如果切片没有按该谓词分区，返回结果没有指定含义；本方法本质上执行一种二分查找，
    /// 因而依赖“先 true 后 false”的前置条件。
///
    /// 另见 [`binary_search`]、[`binary_search_by`] 和 [`binary_search_by_key`]。
    ///
    /// [`binary_search`]: slice::binary_search
    /// [`binary_search_by`]: slice::binary_search_by
    /// [`binary_search_by_key`]: slice::binary_search_by_key
    ///
    /// # 示例
    ///
    /// ```
    /// let v = [1, 2, 3, 3, 5, 6, 7];
    /// let i = v.partition_point(|&x| x < 5);
    ///
    /// assert_eq!(i, 4);
    /// assert!(v[..i].iter().all(|&x| x < 5));
    /// assert!(v[i..].iter().all(|&x| !(x < 5)));
    /// ```
    ///
    /// 如果切片中所有元素都满足谓词，包括切片为空的情况，则返回切片长度：
    ///
    /// ```
    /// let a = [2, 4, 8];
    /// assert_eq!(a.partition_point(|x| x < &100), a.len());
    /// let a: [i32; 0] = [];
    /// assert_eq!(a.partition_point(|x| x < &100), 0);
    /// ```
    ///
    /// 如果需要向已排序 vector 插入元素并保持排序顺序：
    ///
    /// ```
    /// let mut s = vec![0, 1, 1, 1, 1, 2, 3, 5, 8, 13, 21, 34, 55];
    /// let num = 42;
    /// let idx = s.partition_point(|&x| x <= num);
    /// s.insert(idx, num);
    /// assert_eq!(s, [0, 1, 1, 1, 1, 2, 3, 5, 8, 13, 21, 34, 42, 55]);
    /// ```
    #[stable(feature = "partition_point", since = "1.52.0")]
    #[must_use]
    pub fn partition_point<P>(&self, mut pred: P) -> usize
    where
        P: FnMut(&T) -> bool,
    {
        self.binary_search_by(|x| if pred(x) { Less } else { Greater }).unwrap_or_else(|i| i)
    }

    /// 移除给定范围对应的子切片，并返回它的引用。
    ///
    /// 如果给定范围越界，返回 `None`，且不修改切片。
    ///
    /// 注意，本方法只接受 `2..` 或 `..6` 这样的单侧范围，而不接受 `2..6`。
    ///
    /// # 示例
    ///
    /// 分离切片中的前三个元素：
    ///
    /// ```
    /// let mut slice: &[_] = &['a', 'b', 'c', 'd'];
    /// let mut first_three = slice.split_off(..3).unwrap();
    ///
    /// assert_eq!(slice, &['d']);
    /// assert_eq!(first_three, &['a', 'b', 'c']);
    /// ```
    ///
    /// 分离从第三个元素开始的切片：
    ///
    /// ```
    /// let mut slice: &[_] = &['a', 'b', 'c', 'd'];
    /// let mut tail = slice.split_off(2..).unwrap();
    ///
    /// assert_eq!(slice, &['a', 'b']);
    /// assert_eq!(tail, &['c', 'd']);
    /// ```
    ///
    /// 当 `range` 越界时得到 `None`：
    ///
    /// ```
    /// let mut slice: &[_] = &['a', 'b', 'c', 'd'];
    ///
    /// assert_eq!(None, slice.split_off(5..));
    /// assert_eq!(None, slice.split_off(..5));
    /// assert_eq!(None, slice.split_off(..=4));
    /// let expected: &[char] = &['a', 'b', 'c', 'd'];
    /// assert_eq!(Some(expected), slice.split_off(..4));
    /// ```
    #[inline]
    #[must_use = "method does not modify the slice if the range is out of bounds"]
    #[stable(feature = "slice_take", since = "1.87.0")]
    pub fn split_off<'a, R: OneSidedRange<usize>>(
        self: &mut &'a Self,
        range: R,
    ) -> Option<&'a Self> {
        let (direction, split_index) = split_point_of(range)?;
        if split_index > self.len() {
            return None;
        }
        let (front, back) = self.split_at(split_index);
        match direction {
            Direction::Front => {
                *self = back;
                Some(front)
            }
            Direction::Back => {
                *self = front;
                Some(back)
            }
        }
    }

    /// 移除给定范围对应的子切片，并返回它的可变引用。
    ///
    /// 如果给定范围越界，返回 `None`，且不修改切片。
    ///
    /// 注意，本方法只接受 `2..` 或 `..6` 这样的单侧范围，而不接受 `2..6`。
    ///
    /// # 示例
    ///
    /// 分离切片中的前三个元素：
    ///
    /// ```
    /// let mut slice: &mut [_] = &mut ['a', 'b', 'c', 'd'];
    /// let mut first_three = slice.split_off_mut(..3).unwrap();
    ///
    /// assert_eq!(slice, &mut ['d']);
    /// assert_eq!(first_three, &mut ['a', 'b', 'c']);
    /// ```
    ///
    /// 分离从第三个元素开始的切片：
    ///
    /// ```
    /// let mut slice: &mut [_] = &mut ['a', 'b', 'c', 'd'];
    /// let mut tail = slice.split_off_mut(2..).unwrap();
    ///
    /// assert_eq!(slice, &mut ['a', 'b']);
    /// assert_eq!(tail, &mut ['c', 'd']);
    /// ```
    ///
    /// 当 `range` 越界时得到 `None`：
    ///
    /// ```
    /// let mut slice: &mut [_] = &mut ['a', 'b', 'c', 'd'];
    ///
    /// assert_eq!(None, slice.split_off_mut(5..));
    /// assert_eq!(None, slice.split_off_mut(..5));
    /// assert_eq!(None, slice.split_off_mut(..=4));
    /// let expected: &mut [_] = &mut ['a', 'b', 'c', 'd'];
    /// assert_eq!(Some(expected), slice.split_off_mut(..4));
    /// ```
    #[inline]
    #[must_use = "method does not modify the slice if the range is out of bounds"]
    #[stable(feature = "slice_take", since = "1.87.0")]
    pub fn split_off_mut<'a, R: OneSidedRange<usize>>(
        self: &mut &'a mut Self,
        range: R,
    ) -> Option<&'a mut Self> {
        let (direction, split_index) = split_point_of(range)?;
        if split_index > self.len() {
            return None;
        }
        let (front, back) = mem::take(self).split_at_mut(split_index);
        match direction {
            Direction::Front => {
                *self = back;
                Some(front)
            }
            Direction::Back => {
                *self = front;
                Some(back)
            }
        }
    }

    /// 移除切片的第一个元素，并返回它的引用。
    ///
    /// 如果切片为空，返回 `None`。
    ///
    /// # 示例
    ///
    /// ```
    /// let mut slice: &[_] = &['a', 'b', 'c'];
    /// let first = slice.split_off_first().unwrap();
    ///
    /// assert_eq!(slice, &['b', 'c']);
    /// assert_eq!(first, &'a');
    /// ```
    #[inline]
    #[stable(feature = "slice_take", since = "1.87.0")]
    #[rustc_const_unstable(feature = "const_split_off_first_last", issue = "138539")]
    pub const fn split_off_first<'a>(self: &mut &'a Self) -> Option<&'a T> {
        // FIXME(const-hack): const 中可用 `?` 后，用它替代 `let-else`。
        let Some((first, rem)) = self.split_first() else { return None };
        *self = rem;
        Some(first)
    }

    /// 移除切片的第一个元素，并返回它的可变引用。
    ///
    /// 如果切片为空，返回 `None`。
    ///
    /// # 示例
    ///
    /// ```
    /// let mut slice: &mut [_] = &mut ['a', 'b', 'c'];
    /// let first = slice.split_off_first_mut().unwrap();
    /// *first = 'd';
    ///
    /// assert_eq!(slice, &['b', 'c']);
    /// assert_eq!(first, &'d');
    /// ```
    #[inline]
    #[stable(feature = "slice_take", since = "1.87.0")]
    #[rustc_const_unstable(feature = "const_split_off_first_last", issue = "138539")]
    pub const fn split_off_first_mut<'a>(self: &mut &'a mut Self) -> Option<&'a mut T> {
        // FIXME(const-hack): const 中可用 `mem::take` 和 `?` 后改用它们。
        // 原始写法：`mem::take(self).split_first_mut()?`
        let Some((first, rem)) = mem::replace(self, &mut []).split_first_mut() else { return None };
        *self = rem;
        Some(first)
    }

    /// 移除切片的最后一个元素，并返回它的引用。
    ///
    /// 如果切片为空，返回 `None`。
    ///
    /// # 示例
    ///
    /// ```
    /// let mut slice: &[_] = &['a', 'b', 'c'];
    /// let last = slice.split_off_last().unwrap();
    ///
    /// assert_eq!(slice, &['a', 'b']);
    /// assert_eq!(last, &'c');
    /// ```
    #[inline]
    #[stable(feature = "slice_take", since = "1.87.0")]
    #[rustc_const_unstable(feature = "const_split_off_first_last", issue = "138539")]
    pub const fn split_off_last<'a>(self: &mut &'a Self) -> Option<&'a T> {
        // FIXME(const-hack): const 中可用 `?` 后，用它替代 `let-else`。
        let Some((last, rem)) = self.split_last() else { return None };
        *self = rem;
        Some(last)
    }

    /// 移除切片的最后一个元素，并返回它的可变引用。
    ///
    /// 如果切片为空，返回 `None`。
    ///
    /// # 示例
    ///
    /// ```
    /// let mut slice: &mut [_] = &mut ['a', 'b', 'c'];
    /// let last = slice.split_off_last_mut().unwrap();
    /// *last = 'd';
    ///
    /// assert_eq!(slice, &['a', 'b']);
    /// assert_eq!(last, &'d');
    /// ```
    #[inline]
    #[stable(feature = "slice_take", since = "1.87.0")]
    #[rustc_const_unstable(feature = "const_split_off_first_last", issue = "138539")]
    pub const fn split_off_last_mut<'a>(self: &mut &'a mut Self) -> Option<&'a mut T> {
        // FIXME(const-hack): const 中可用 `mem::take` 和 `?` 后改用它们。
        // 原始写法：`mem::take(self).split_last_mut()?`
        let Some((last, rem)) = mem::replace(self, &mut []).split_last_mut() else { return None };
        *self = rem;
        Some(last)
    }

    /// 不做任何检查，一次性返回多个索引对应的可变引用。
    ///
    /// 索引可以是 `usize`、[`Range`] 或 [`RangeInclusive`]。注意，本方法接收数组，
    /// 因而所有索引必须具有相同类型。
    /// 如果传入 `usize` 数组，本方法返回单个元素的可变引用数组；如果传入范围数组，
    /// 则返回可变子切片引用数组。
///
    /// 安全替代方案见 [`get_disjoint_mut`]。
    ///
    /// # 安全性(Safety）
    ///
    /// 使用重叠或越界索引调用本方法是 *[undefined behavior]*，即使得到的引用之后没有被使用。
    /// 调用方必须保证所有索引都在边界内，且任意两个返回的可变引用覆盖的内存不重叠。
    ///
    /// # 示例
    ///
    /// ```
    /// let x = &mut [1, 2, 4];
    ///
    /// unsafe {
    ///     let [a, b] = x.get_disjoint_unchecked_mut([0, 2]);
    ///     *a *= 10;
    ///     *b *= 100;
    /// }
    /// assert_eq!(x, &[10, 2, 400]);
    ///
    /// unsafe {
    ///     let [a, b] = x.get_disjoint_unchecked_mut([0..1, 1..3]);
    ///     a[0] = 8;
    ///     b[0] = 88;
    ///     b[1] = 888;
    /// }
    /// assert_eq!(x, &[8, 88, 888]);
    ///
    /// unsafe {
    ///     let [a, b] = x.get_disjoint_unchecked_mut([1..=2, 0..=0]);
    ///     a[0] = 11;
    ///     a[1] = 111;
    ///     b[0] = 1;
    /// }
    /// assert_eq!(x, &[1, 11, 111]);
    /// ```
    ///
    /// [`get_disjoint_mut`]: slice::get_disjoint_mut
    /// [undefined behavior]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    #[stable(feature = "get_many_mut", since = "1.86.0")]
    #[inline]
    #[track_caller]
    pub unsafe fn get_disjoint_unchecked_mut<I, const N: usize>(
        &mut self,
        indices: [I; N],
    ) -> [&mut I::Output; N]
    where
        I: GetDisjointMutIndex + SliceIndex<Self>,
    {
        // NB: 这个实现保持当前写法，是因为任何 `indices.map(|i| self.get_unchecked_mut(i))`
        // 的变体都会让 Miri 不满意，或者生成更差的代码。这也是这里需要经过裸指针的原因。
        let slice: *mut [T] = self;
        let mut arr: MaybeUninit<[&mut I::Output; N]> = MaybeUninit::uninit();
        let arr_ptr = arr.as_mut_ptr();

        // SAFETY: 调用方必须保证 `indices` 中的每个索引都在 `self` 边界内，并且彼此不重叠。
        unsafe {
            for i in 0..N {
                let idx = indices.get_unchecked(i).clone();
                arr_ptr.cast::<&mut I::Output>().add(i).write(&mut *slice.get_unchecked_mut(idx));
            }
            arr.assume_init()
        }
    }

    /// 一次性返回多个索引对应位置的可变引用。
///
    /// 索引可以是 `usize`、[`Range`] 或 [`RangeInclusive`]。注意本方法接收数组，
    /// 因而所有索引必须具有同一种类型。传入 `usize` 数组时返回多个元素的可变引用；
    /// 传入范围数组时返回多个可变子切片引用。
///
    /// 如果任一索引越界，或任意两个索引范围重叠，返回错误。空范围位于另一个范围开头或末尾时
    /// 不视为重叠，但位于另一个范围中间时视为重叠。
///
    /// 本方法使用 O(n^2) 检查确认索引之间没有重叠；传入大量索引时需要注意成本。
    ///
    /// # 示例
    ///
    /// ```
    /// let v = &mut [1, 2, 3];
    /// if let Ok([a, b]) = v.get_disjoint_mut([0, 2]) {
    ///     *a = 413;
    ///     *b = 612;
    /// }
    /// assert_eq!(v, &[413, 2, 612]);
    ///
    /// if let Ok([a, b]) = v.get_disjoint_mut([0..1, 1..3]) {
    ///     a[0] = 8;
    ///     b[0] = 88;
    ///     b[1] = 888;
    /// }
    /// assert_eq!(v, &[8, 88, 888]);
    ///
    /// if let Ok([a, b]) = v.get_disjoint_mut([1..=2, 0..=0]) {
    ///     a[0] = 11;
    ///     a[1] = 111;
    ///     b[0] = 1;
    /// }
    /// assert_eq!(v, &[1, 11, 111]);
    /// ```
    #[stable(feature = "get_many_mut", since = "1.86.0")]
    #[inline]
    pub fn get_disjoint_mut<I, const N: usize>(
        &mut self,
        indices: [I; N],
    ) -> Result<[&mut I::Output; N], GetDisjointMutError>
    where
        I: GetDisjointMutIndex + SliceIndex<Self>,
    {
        get_disjoint_check_valid(&indices, self.len())?;
        // SAFETY: `get_disjoint_check_valid()` 已检查所有索引都互不重叠且在边界内。
        unsafe { Ok(self.get_disjoint_unchecked_mut(indices)) }
    }

    /// 返回某个元素引用在本切片中指向的索引。
///
    /// 如果 `element` 并未指向本切片内某个元素的起始位置，返回 `None`。
///
    /// 该方法适合用于扩展 [`slice::split`] 这类切片迭代器。
///
    /// 注意，本方法使用指针算术，**不会比较元素值**。如果想通过值比较查找元素索引，
    /// 应改用 [`.iter().position()`](crate::iter::Iterator::position)。
    ///
    /// # Panics
    /// 如果 `T` 是 zero-sized，本函数会 panic。
    ///
    /// # 示例
    /// 基本用法：
    /// ```
    /// let nums: &[u32] = &[1, 7, 1, 1];
    /// let num = &nums[2];
    ///
    /// assert_eq!(num, &1);
    /// assert_eq!(nums.element_offset(num), Some(2));
    /// ```
    /// 对未与原切片元素边界对齐的引用返回 `None`：
    /// ```
    /// let arr: &[[u32; 2]] = &[[0, 1], [2, 3]];
    /// let flat_arr: &[u32] = arr.as_flattened();
    ///
    /// let ok_elm: &[u32; 2] = flat_arr[0..2].try_into().unwrap();
    /// let weird_elm: &[u32; 2] = flat_arr[1..3].try_into().unwrap();
    ///
    /// assert_eq!(ok_elm, &[0, 1]);
    /// assert_eq!(weird_elm, &[1, 2]);
    ///
    /// assert_eq!(arr.element_offset(ok_elm), Some(0)); // 指向元素 0
    /// assert_eq!(arr.element_offset(weird_elm), None); // 指向元素 0 和 1 之间
    /// ```
    #[must_use]
    #[stable(feature = "element_offset", since = "1.94.0")]
    pub fn element_offset(&self, element: &T) -> Option<usize> {
        if T::IS_ZST {
            panic!("elements are zero-sized");
        }

        let self_start = self.as_ptr().addr();
        let elem_start = ptr::from_ref(element).addr();

        let byte_offset = elem_start.wrapping_sub(self_start);

        if !byte_offset.is_multiple_of(size_of::<T>()) {
            return None;
        }

        let offset = byte_offset / size_of::<T>();

        if offset < self.len() { Some(offset) } else { None }
    }

    /// 返回某个子切片在本切片中指向的索引范围。
///
    /// 如果 `subslice` 不指向本切片内部，或未与本切片的元素边界对齐，返回 `None`。
///
    /// 本方法 **不会比较元素值**。它只通过指针算术找出 `subslice` 来源于本切片的哪个位置。
    /// 如果想通过值比较查找子切片索引，应改用
    /// [`.windows()`](slice::windows)[`.position()`](crate::iter::Iterator::position)。
///
    /// 该方法适合用于扩展 [`slice::split`] 这类切片迭代器。
///
    /// 注意，如果 `subslice` 长度为 0，且指向另一个独立切片的开头或末尾，本方法可能返回
    /// 假阳性（`Some(0..0)` 或 `Some(self.len()..self.len())`）。
    ///
    /// # Panics
    /// 如果 `T` 是 zero-sized，本函数会 panic。
    ///
    /// # 示例
    /// 基本用法：
    /// ```
    /// #![feature(substr_range)]
    ///
    /// let nums = &[0, 5, 10, 0, 0, 5];
    ///
    /// let mut iter = nums
    ///     .split(|t| *t == 0)
    ///     .map(|n| nums.subslice_range(n).unwrap());
    ///
    /// assert_eq!(iter.next(), Some(0..0));
    /// assert_eq!(iter.next(), Some(1..3));
    /// assert_eq!(iter.next(), Some(4..4));
    /// assert_eq!(iter.next(), Some(5..6));
    /// ```
    #[must_use]
    #[unstable(feature = "substr_range", issue = "126769")]
    pub fn subslice_range(&self, subslice: &[T]) -> Option<Range<usize>> {
        if T::IS_ZST {
            panic!("elements are zero-sized");
        }

        let self_start = self.as_ptr().addr();
        let subslice_start = subslice.as_ptr().addr();

        let byte_start = subslice_start.wrapping_sub(self_start);

        if !byte_start.is_multiple_of(size_of::<T>()) {
            return None;
        }

        let start = byte_start / size_of::<T>();
        let end = start.wrapping_add(subslice.len());

        if start <= self.len() && end <= self.len() { Some(start..end) } else { None }
    }

    /// 返回同一个切片 `&[T]`。
///
    /// 直接在 `&[T]` 上调用时该方法是冗余的，但它有助于把其它“容器”类型解引用为切片，
    /// 例如 `Box<[T]>` 或 `Arc<[T]>`。
    #[inline]
    #[unstable(feature = "str_as_str", issue = "130366")]
    pub const fn as_slice(&self) -> &[T] {
        self
    }

    /// 返回同一个可变切片 `&mut [T]`。
///
    /// 直接在 `&mut [T]` 上调用时该方法是冗余的，但它有助于把其它“容器”类型解引用为
    /// 可变切片，例如 `Box<[T]>` 或 `MutexGuard<[T]>`。
    #[inline]
    #[unstable(feature = "str_as_str", issue = "130366")]
    pub const fn as_mut_slice(&mut self) -> &mut [T] {
        self
    }
}

impl<T> [MaybeUninit<T>] {
    /// 把可变未初始化切片中对齐合适的一段重新解释成另一种类型的可变未初始化切片。
///
    /// 这是 [`slice::align_to_mut`] 的安全包装，因此继承该方法的对齐和不重叠保证。
    /// 安全性的额外来源是 `MaybeUninit` 对任意位模式都有效，所以中间段不需要证明已经
    /// 初始化为 `U`。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(align_to_uninit_mut)]
    /// use std::mem::MaybeUninit;
    ///
    /// pub struct BumpAllocator<'scope> {
    ///     memory: &'scope mut [MaybeUninit<u8>],
    /// }
    ///
    /// impl<'scope> BumpAllocator<'scope> {
    ///     pub fn new(memory: &'scope mut [MaybeUninit<u8>]) -> Self {
    ///         Self { memory }
    ///     }
    ///     pub fn try_alloc_uninit<T>(&mut self) -> Option<&'scope mut MaybeUninit<T>> {
    ///         let first_end = self.memory.as_ptr().align_offset(align_of::<T>()) + size_of::<T>();
    ///         let prefix = self.memory.split_off_mut(..first_end)?;
    ///         Some(&mut prefix.align_to_uninit_mut::<T>().1[0])
    ///     }
    ///     pub fn try_alloc_u32(&mut self, value: u32) -> Option<&'scope mut u32> {
    ///         let uninit = self.try_alloc_uninit()?;
    ///         Some(uninit.write(value))
    ///     }
    /// }
    ///
    /// let mut memory = [MaybeUninit::<u8>::uninit(); 10];
    /// let mut allocator = BumpAllocator::new(&mut memory);
    /// let v = allocator.try_alloc_u32(42);
    /// assert_eq!(v, Some(&mut 42));
    /// ```
    #[unstable(feature = "align_to_uninit_mut", issue = "139062")]
    #[inline]
    #[must_use]
    pub fn align_to_uninit_mut<U>(&mut self) -> (&mut Self, &mut [MaybeUninit<U>], &mut Self) {
        // SAFETY: `MaybeUninit` 是透明包装。正确大小和对齐由 `align_to_mut` 自身保证。
        // 因此安全 transmute 还需证明的只剩“值对目标类型有效”；而对 `MaybeUninit` 来说，
        // 任意位模式都是有效值，所以该操作安全。
        unsafe { self.align_to_mut() }
    }
}

impl<T, const N: usize> [[T; N]] {
    /// 接收 `&[[T; N]]`，并将其展平成 `&[T]`。
///
    /// 反向操作见 [`as_chunks`] 和 [`as_rchunks`]。
    ///
    /// [`as_chunks`]: slice::as_chunks
    /// [`as_rchunks`]: slice::as_rchunks
    ///
    /// # Panics
    ///
    /// 如果结果切片长度会溢出 `usize`，本函数会 panic。
///
    /// 这只可能发生在展平 zero-sized 类型数组切片时，因此实践中通常无关紧要。
    /// 如果 `size_of::<T>() > 0`，本函数永远不会 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// assert_eq!([[1, 2, 3], [4, 5, 6]].as_flattened(), &[1, 2, 3, 4, 5, 6]);
    ///
    /// assert_eq!(
    ///     [[1, 2, 3], [4, 5, 6]].as_flattened(),
    ///     [[1, 2], [3, 4], [5, 6]].as_flattened(),
    /// );
    ///
    /// let slice_of_empty_arrays: &[[i32; 0]] = &[[], [], [], [], []];
    /// assert!(slice_of_empty_arrays.as_flattened().is_empty());
    ///
    /// let empty_slice_of_arrays: &[[u32; 10]] = &[];
    /// assert!(empty_slice_of_arrays.as_flattened().is_empty());
    /// ```
    #[stable(feature = "slice_flatten", since = "1.80.0")]
    #[rustc_const_stable(feature = "const_slice_flatten", since = "1.87.0")]
    pub const fn as_flattened(&self) -> &[T] {
        let len = if T::IS_ZST {
            self.len().checked_mul(N).expect("slice len overflow")
        } else {
            // SAFETY: `self.len() * N` 不会溢出，因为 `self` 已经位于地址空间内。
            unsafe { self.len().unchecked_mul(N) }
        };
        // SAFETY: `[T; N]` 的连续元素布局与 `[T]` 对应前缀布局一致。
        unsafe { from_raw_parts(self.as_ptr().cast(), len) }
    }

    /// 接收 `&mut [[T; N]]`，并将其展平成 `&mut [T]`。
///
    /// 反向操作见 [`as_chunks_mut`] 和 [`as_rchunks_mut`]。
    ///
    /// [`as_chunks_mut`]: slice::as_chunks_mut
    /// [`as_rchunks_mut`]: slice::as_rchunks_mut
    ///
    /// # Panics
    ///
    /// 如果结果切片长度会溢出 `usize`，本函数会 panic。
///
    /// 这只可能发生在展平 zero-sized 类型数组切片时，因此实践中通常无关紧要。
    /// 如果 `size_of::<T>() > 0`，本函数永远不会 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// fn add_5_to_all(slice: &mut [i32]) {
    ///     for i in slice {
    ///         *i += 5;
    ///     }
    /// }
    ///
    /// let mut array = [[1, 2, 3], [4, 5, 6], [7, 8, 9]];
    /// add_5_to_all(array.as_flattened_mut());
    /// assert_eq!(array, [[6, 7, 8], [9, 10, 11], [12, 13, 14]]);
    /// ```
    #[stable(feature = "slice_flatten", since = "1.80.0")]
    #[rustc_const_stable(feature = "const_slice_flatten", since = "1.87.0")]
    pub const fn as_flattened_mut(&mut self) -> &mut [T] {
        let len = if T::IS_ZST {
            self.len().checked_mul(N).expect("slice len overflow")
        } else {
            // SAFETY: `self.len() * N` 不会溢出，因为 `self` 已经位于地址空间内。
            unsafe { self.len().unchecked_mul(N) }
        };
        // SAFETY: `[T; N]` 的连续元素布局与 `[T]` 对应前缀布局一致，且 `&mut self` 保证独占访问。
        unsafe { from_raw_parts_mut(self.as_mut_ptr().cast(), len) }
    }
}

impl [f32] {
    /// 对浮点切片排序。
///
    /// 这是原地排序（不分配），最坏时间复杂度为 *O*(*n* \* log(*n*))，使用
    /// [`f32::total_cmp`] 定义的全序。该顺序能处理 NaN、正负零和无穷大。
///
    /// # 当前实现
///
    /// 本方法使用与 [`sort_unstable_by`](slice::sort_unstable_by) 相同的排序算法。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(sort_floats)]
    /// let mut v = [2.6, -5e-8, f32::NAN, 8.29, f32::INFINITY, -1.0, 0.0, -f32::INFINITY, -0.0];
    ///
    /// v.sort_floats();
    /// let sorted = [-f32::INFINITY, -1.0, -5e-8, -0.0, 0.0, 2.6, 8.29, f32::INFINITY, f32::NAN];
    /// assert_eq!(&v[..8], &sorted[..8]);
    /// assert!(v[8].is_nan());
    /// ```
    #[unstable(feature = "sort_floats", issue = "93396")]
    #[inline]
    pub fn sort_floats(&mut self) {
        self.sort_unstable_by(f32::total_cmp);
    }
}

impl [f64] {
    /// 对浮点切片排序。
///
    /// 这是原地排序（不分配），最坏时间复杂度为 *O*(*n* \* log(*n*))，使用
    /// [`f64::total_cmp`] 定义的全序。该顺序能处理 NaN、正负零和无穷大。
///
    /// # 当前实现
///
    /// 本方法使用与 [`sort_unstable_by`](slice::sort_unstable_by) 相同的排序算法。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(sort_floats)]
    /// let mut v = [2.6, -5e-8, f64::NAN, 8.29, f64::INFINITY, -1.0, 0.0, -f64::INFINITY, -0.0];
    ///
    /// v.sort_floats();
    /// let sorted = [-f64::INFINITY, -1.0, -5e-8, -0.0, 0.0, 2.6, 8.29, f64::INFINITY, f64::NAN];
    /// assert_eq!(&v[..8], &sorted[..8]);
    /// assert!(v[8].is_nan());
    /// ```
    #[unstable(feature = "sort_floats", issue = "93396")]
    #[inline]
    pub fn sort_floats(&mut self) {
        self.sort_unstable_by(f64::total_cmp);
    }
}

/// 将 `src` 复制到 `dest`。
///
/// # 安全性(Safety）
/// `T` 必须实现 `Copy` 或 `TrivialClone` 之一；否则按位复制会绕过 `Clone` 语义。
#[track_caller]
const unsafe fn copy_from_slice_impl<T: Clone>(dest: &mut [T], src: &[T]) {
    // 将 panic 路径放入 cold 函数，避免膨胀正常调用点。
    #[cfg_attr(not(panic = "immediate-abort"), inline(never), cold)]
    #[cfg_attr(panic = "immediate-abort", inline)]
    #[track_caller]
    const fn len_mismatch_fail(dst_len: usize, src_len: usize) -> ! {
        const_panic!(
            "copy_from_slice: source slice length does not match destination slice length",
            "copy_from_slice: source slice length ({src_len}) does not match destination slice length ({dst_len})",
            src_len: usize,
            dst_len: usize,
        )
    }

    if dest.len() != src.len() {
        len_mismatch_fail(dest.len(), src.len());
    }

    // SAFETY: 根据定义，`dest` 对 `dest.len()` 个元素有效，且已检查 `src` 长度相同。
    // 两个切片不能重叠，因为可变引用具有独占性。
    unsafe {
        ptr::copy_nonoverlapping(src.as_ptr(), dest.as_mut_ptr(), dest.len());
    }
}

#[rustc_const_unstable(feature = "const_clone", issue = "142757")]
const trait CloneFromSpec<T> {
    fn spec_clone_from(&mut self, src: &[T])
    where
        T: [const] Destruct;
}

#[rustc_const_unstable(feature = "const_clone", issue = "142757")]
impl<T> const CloneFromSpec<T> for [T]
where
    T: [const] Clone + [const] Destruct,
{
    #[track_caller]
    default fn spec_clone_from(&mut self, src: &[T]) {
        assert!(self.len() == src.len(), "destination and source slices have different lengths");
        // NOTE: 需要显式把两者切到相同长度，便于优化器消除边界检查。
        // 但不能完全依赖这一点，因此对 T: Copy 仍有显式 specialization。
        let len = self.len();
        let src = &src[..len];
        // FIXME(const_hack): 改成 `for idx in 0..self.len()` 循环。
        let mut idx = 0;
        while idx < self.len() {
            self[idx].clone_from(&src[idx]);
            idx += 1;
        }
    }
}

#[rustc_const_unstable(feature = "const_clone", issue = "142757")]
impl<T> const CloneFromSpec<T> for [T]
where
    T: [const] TrivialClone + [const] Destruct,
{
    #[track_caller]
    fn spec_clone_from(&mut self, src: &[T]) {
        // SAFETY: `T` 实现了 `TrivialClone`，按位复制等价于 clone。
        unsafe {
            copy_from_slice_impl(self, src);
        }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_unstable(feature = "const_default", issue = "143894")]
impl<T> const Default for &[T] {
    /// 创建空切片。
    fn default() -> Self {
        &[]
    }
}

#[stable(feature = "mut_slice_default", since = "1.5.0")]
#[rustc_const_unstable(feature = "const_default", issue = "143894")]
impl<T> const Default for &mut [T] {
    /// 创建可变空切片。
    fn default() -> Self {
        &mut []
    }
}

#[unstable(feature = "slice_pattern", reason = "stopgap trait for slice patterns", issue = "56345")]
/// 切片中的模式。目前只供 `strip_prefix` 与 `strip_suffix` 使用。
///
/// 未来如果能把 `core::str::Pattern`（撰写时仅限 `str`）泛化到切片，本 trait 将被替换
/// 或移除。
pub trait SlicePattern {
    /// 被匹配切片的元素类型。
    type Item;

    /// 当前 `SlicePattern` 的使用方需要拿到一个切片视图。
    fn as_slice(&self) -> &[Self::Item];
}

#[stable(feature = "slice_strip", since = "1.51.0")]
impl<T> SlicePattern for [T] {
    type Item = T;

    #[inline]
    fn as_slice(&self) -> &[Self::Item] {
        self
    }
}

#[stable(feature = "slice_strip", since = "1.51.0")]
impl<T, const N: usize> SlicePattern for [T; N] {
    type Item = T;

    #[inline]
    fn as_slice(&self) -> &[Self::Item] {
        self
    }
}

/// 检查每个索引是否在 `len` 内，并与其它索引两两检查是否重叠。
///
/// 这会执行 `binomial(N + 1, 2) = N * (N + 1) / 2 = 0, 1, 3, 6, 10, ..`
/// 次比较操作。
#[inline]
fn get_disjoint_check_valid<I: GetDisjointMutIndex, const N: usize>(
    indices: &[I; N],
    len: usize,
) -> Result<(), GetDisjointMutError> {
    // NB: 优化器应当把这些循环内联成一串没有额外分支的指令。
    for (i, idx) in indices.iter().enumerate() {
        if !idx.is_in_bounds(len) {
            return Err(GetDisjointMutError::IndexOutOfBounds);
        }
        for idx2 in &indices[..i] {
            if idx.is_overlapping(idx2) {
                return Err(GetDisjointMutError::OverlappingIndices);
            }
        }
    }
    Ok(())
}

/// [`get_disjoint_mut`][`slice::get_disjoint_mut`] 返回的错误类型。
///
/// 它表示两类可能错误之一：
/// - 某个索引越过切片边界。
/// - 同一个索引在数组中出现多次；传入范围时，也可能是不同范围发生重叠。
///
/// # 示例
///
/// ```
/// use std::slice::GetDisjointMutError;
///
/// let v = &mut [1, 2, 3];
/// assert_eq!(v.get_disjoint_mut([0, 999]), Err(GetDisjointMutError::IndexOutOfBounds));
/// assert_eq!(v.get_disjoint_mut([1, 1]), Err(GetDisjointMutError::OverlappingIndices));
/// ```
#[stable(feature = "get_many_mut", since = "1.86.0")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GetDisjointMutError {
    /// 提供的某个索引越过切片边界。
    IndexOutOfBounds,
    /// 提供的两个索引范围发生重叠。
    OverlappingIndices,
}

#[stable(feature = "get_many_mut", since = "1.86.0")]
impl fmt::Display for GetDisjointMutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let msg = match self {
            GetDisjointMutError::IndexOutOfBounds => "an index is out of bounds",
            GetDisjointMutError::OverlappingIndices => "there were overlapping indices",
        };
        fmt::Display::fmt(msg, f)
    }
}

mod private_get_disjoint_mut_index {
    use super::{Range, RangeInclusive, range};

    #[unstable(feature = "get_disjoint_mut_helpers", issue = "none")]
    pub trait Sealed {}

    #[unstable(feature = "get_disjoint_mut_helpers", issue = "none")]
    impl Sealed for usize {}
    #[unstable(feature = "get_disjoint_mut_helpers", issue = "none")]
    impl Sealed for Range<usize> {}
    #[unstable(feature = "get_disjoint_mut_helpers", issue = "none")]
    impl Sealed for RangeInclusive<usize> {}
    #[unstable(feature = "get_disjoint_mut_helpers", issue = "none")]
    impl Sealed for range::Range<usize> {}
    #[unstable(feature = "get_disjoint_mut_helpers", issue = "none")]
    impl Sealed for range::RangeInclusive<usize> {}
}

/// `<[T]>::get_disjoint_mut()` 使用的辅助 trait。
///
/// # 安全性(Safety）
///
/// 如果 `is_in_bounds()` 返回 `true` 且 `is_overlapping()` 返回 `false`，
/// 则必须可以用这些索引安全地索引切片，并同时返回互不重叠的 `&mut`。
#[unstable(feature = "get_disjoint_mut_helpers", issue = "none")]
pub unsafe trait GetDisjointMutIndex:
    Clone + private_get_disjoint_mut_index::Sealed
{
    /// 如果 `self` 对长度为 `len` 的切片来说位于边界内，返回 `true`。
    #[unstable(feature = "get_disjoint_mut_helpers", issue = "none")]
    fn is_in_bounds(&self, len: usize) -> bool;

    /// 如果 `self` 与 `other` 覆盖范围重叠，返回 `true`。
///
    /// 注意：位于另一个范围开头或末尾的零长度范围不视为重叠；位于中间的零长度范围视为重叠。
    #[unstable(feature = "get_disjoint_mut_helpers", issue = "none")]
    fn is_overlapping(&self, other: &Self) -> bool;
}

#[unstable(feature = "get_disjoint_mut_helpers", issue = "none")]
// SAFETY: `is_in_bounds()` 与 `is_overlapping()` 按 `usize` 单点索引语义正确实现。
unsafe impl GetDisjointMutIndex for usize {
    #[inline]
    fn is_in_bounds(&self, len: usize) -> bool {
        *self < len
    }

    #[inline]
    fn is_overlapping(&self, other: &Self) -> bool {
        *self == *other
    }
}

#[unstable(feature = "get_disjoint_mut_helpers", issue = "none")]
// SAFETY: `is_in_bounds()` 与 `is_overlapping()` 按 `Range<usize>` 半开范围语义正确实现。
unsafe impl GetDisjointMutIndex for Range<usize> {
    #[inline]
    fn is_in_bounds(&self, len: usize) -> bool {
        (self.start <= self.end) & (self.end <= len)
    }

    #[inline]
    fn is_overlapping(&self, other: &Self) -> bool {
        (self.start < other.end) & (other.start < self.end)
    }
}

#[unstable(feature = "get_disjoint_mut_helpers", issue = "none")]
// SAFETY: `is_in_bounds()` 与 `is_overlapping()` 按 `RangeInclusive<usize>` 闭区间语义正确实现。
unsafe impl GetDisjointMutIndex for RangeInclusive<usize> {
    #[inline]
    fn is_in_bounds(&self, len: usize) -> bool {
        (self.start <= self.end) & (self.end < len)
    }

    #[inline]
    fn is_overlapping(&self, other: &Self) -> bool {
        (self.start <= other.end) & (other.start <= self.end)
    }
}

#[unstable(feature = "get_disjoint_mut_helpers", issue = "none")]
// SAFETY: `is_in_bounds()` 与 `is_overlapping()` 按 `range::Range<usize>` 半开范围语义正确实现。
unsafe impl GetDisjointMutIndex for range::Range<usize> {
    #[inline]
    fn is_in_bounds(&self, len: usize) -> bool {
        Range::from(*self).is_in_bounds(len)
    }

    #[inline]
    fn is_overlapping(&self, other: &Self) -> bool {
        Range::from(*self).is_overlapping(&Range::from(*other))
    }
}

#[unstable(feature = "get_disjoint_mut_helpers", issue = "none")]
// SAFETY: `is_in_bounds()` 与 `is_overlapping()` 按 `range::RangeInclusive<usize>` 闭区间语义正确实现。
unsafe impl GetDisjointMutIndex for range::RangeInclusive<usize> {
    #[inline]
    fn is_in_bounds(&self, len: usize) -> bool {
        RangeInclusive::from(*self).is_in_bounds(len)
    }

    #[inline]
    fn is_overlapping(&self, other: &Self) -> bool {
        RangeInclusive::from(*self).is_overlapping(&RangeInclusive::from(*other))
    }
}
