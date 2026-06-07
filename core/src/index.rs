#![unstable(feature = "sliceindex_wrappers", issue = "146179")]

//! 用于索引切片的辅助类型。

use crate::intrinsics::slice_get_unchecked;
use crate::slice::SliceIndex;
use crate::{cmp, ops, range};

/// 对索引做 clamp，保证只访问切片中的有效元素。
///
/// # 示例
///
/// ```
/// #![feature(sliceindex_wrappers)]
///
/// use core::index::Clamp;
///
/// let s: &[usize] = &[0, 1, 2, 3];
///
/// assert_eq!(&3, &s[Clamp(6)]);
/// assert_eq!(&[1, 2, 3], &s[Clamp(1..6)]);
/// assert_eq!(&[] as &[usize], &s[Clamp(5..6)]);
/// assert_eq!(&[0, 1, 2, 3], &s[Clamp(..6)]);
/// assert_eq!(&[0, 1, 2, 3], &s[Clamp(..=6)]);
/// assert_eq!(&[] as &[usize], &s[Clamp(6..)]);
/// ```
#[unstable(feature = "sliceindex_wrappers", issue = "146179")]
#[derive(Debug)]
pub struct Clamp<Idx>(pub Idx);

/// 始终访问切片的最后一个元素。
///
/// # 示例
///
/// ```
/// #![feature(sliceindex_wrappers)]
/// #![feature(slice_index_methods)]
///
/// use core::index::Last;
/// use core::slice::SliceIndex;
///
/// let s = &[0, 1, 2, 3];
///
/// assert_eq!(&3, &s[Last]);
/// assert_eq!(None, Last.get(&[] as &[usize]));
///
/// ```
#[unstable(feature = "sliceindex_wrappers", issue = "146179")]
#[derive(Debug)]
pub struct Last;

#[unstable(feature = "sliceindex_wrappers", issue = "146179")]
unsafe impl<T> SliceIndex<[T]> for Clamp<usize> {
    type Output = T;

    fn get(self, slice: &[T]) -> Option<&Self::Output> {
        slice.get(cmp::min(self.0, slice.len() - 1))
    }

    fn get_mut(self, slice: &mut [T]) -> Option<&mut Self::Output> {
        slice.get_mut(cmp::min(self.0, slice.len() - 1))
    }

    unsafe fn get_unchecked(self, slice: *const [T]) -> *const Self::Output {
        // SAFETY: `SliceIndex::get_unchecked` 的调用者保证切片非空；
        // 因此 `slice.len() - 1` 不会下溢，且 clamp 后的索引一定小于 `slice.len()`。
        unsafe { slice_get_unchecked(slice, cmp::min(self.0, slice.len() - 1)) }
    }

    unsafe fn get_unchecked_mut(self, slice: *mut [T]) -> *mut Self::Output {
        // SAFETY: `SliceIndex::get_unchecked_mut` 的调用者保证切片非空；
        // 因此 `slice.len() - 1` 不会下溢，且 clamp 后的索引一定小于 `slice.len()`。
        unsafe { slice_get_unchecked(slice, cmp::min(self.0, slice.len() - 1)) }
    }

    fn index(self, slice: &[T]) -> &Self::Output {
        &(*slice)[cmp::min(self.0, slice.len() - 1)]
    }

    fn index_mut(self, slice: &mut [T]) -> &mut Self::Output {
        &mut (*slice)[cmp::min(self.0, slice.len() - 1)]
    }
}

#[unstable(feature = "sliceindex_wrappers", issue = "146179")]
unsafe impl<T> SliceIndex<[T]> for Clamp<range::Range<usize>> {
    type Output = [T];

    fn get(self, slice: &[T]) -> Option<&Self::Output> {
        let start = cmp::min(self.0.start, slice.len());
        let end = cmp::min(self.0.end, slice.len());
        (start..end).get(slice)
    }

    fn get_mut(self, slice: &mut [T]) -> Option<&mut Self::Output> {
        let start = cmp::min(self.0.start, slice.len());
        let end = cmp::min(self.0.end, slice.len());
        (start..end).get_mut(slice)
    }

    unsafe fn get_unchecked(self, slice: *const [T]) -> *const Self::Output {
        let start = cmp::min(self.0.start, slice.len());
        let end = cmp::min(self.0.end, slice.len());
        // SAFETY: `SliceIndex::get_unchecked` 的调用者保证该 `Clamp` 索引对切片有效。
        // clamp 后 `start` 和 `end` 都不超过 `slice.len()`，并且有效性前置条件保证
        // `start <= end`，因此传给底层 `Range` 的范围有效。
        unsafe { (start..end).get_unchecked(slice) }
    }

    unsafe fn get_unchecked_mut(self, slice: *mut [T]) -> *mut Self::Output {
        let start = cmp::min(self.0.start, slice.len());
        let end = cmp::min(self.0.end, slice.len());
        // SAFETY: `SliceIndex::get_unchecked_mut` 的调用者保证该 `Clamp` 索引对切片有效。
        // clamp 后 `start` 和 `end` 都不超过 `slice.len()`，并且有效性前置条件保证
        // `start <= end`，因此传给底层 `Range` 的范围有效。
        unsafe { (start..end).get_unchecked_mut(slice) }
    }

    fn index(self, slice: &[T]) -> &Self::Output {
        let start = cmp::min(self.0.start, slice.len());
        let end = cmp::min(self.0.end, slice.len());
        (start..end).index(slice)
    }

    fn index_mut(self, slice: &mut [T]) -> &mut Self::Output {
        let start = cmp::min(self.0.start, slice.len());
        let end = cmp::min(self.0.end, slice.len());
        (start..end).index_mut(slice)
    }
}

#[unstable(feature = "sliceindex_wrappers", issue = "146179")]
unsafe impl<T> SliceIndex<[T]> for Clamp<ops::Range<usize>> {
    type Output = [T];

    fn get(self, slice: &[T]) -> Option<&Self::Output> {
        let start = cmp::min(self.0.start, slice.len());
        let end = cmp::min(self.0.end, slice.len());
        (start..end).get(slice)
    }

    fn get_mut(self, slice: &mut [T]) -> Option<&mut Self::Output> {
        let start = cmp::min(self.0.start, slice.len());
        let end = cmp::min(self.0.end, slice.len());
        (start..end).get_mut(slice)
    }

    unsafe fn get_unchecked(self, slice: *const [T]) -> *const Self::Output {
        let start = cmp::min(self.0.start, slice.len());
        let end = cmp::min(self.0.end, slice.len());
        // SAFETY: `SliceIndex::get_unchecked` 的调用者保证该 `Clamp` 索引对切片有效。
        // clamp 后 `start` 和 `end` 都不超过 `slice.len()`，并且有效性前置条件保证
        // `start <= end`，因此传给底层 `Range` 的范围有效。
        unsafe { (start..end).get_unchecked(slice) }
    }

    unsafe fn get_unchecked_mut(self, slice: *mut [T]) -> *mut Self::Output {
        let start = cmp::min(self.0.start, slice.len());
        let end = cmp::min(self.0.end, slice.len());
        // SAFETY: `SliceIndex::get_unchecked_mut` 的调用者保证该 `Clamp` 索引对切片有效。
        // clamp 后 `start` 和 `end` 都不超过 `slice.len()`，并且有效性前置条件保证
        // `start <= end`，因此传给底层 `Range` 的范围有效。
        unsafe { (start..end).get_unchecked_mut(slice) }
    }

    fn index(self, slice: &[T]) -> &Self::Output {
        let start = cmp::min(self.0.start, slice.len());
        let end = cmp::min(self.0.end, slice.len());
        (start..end).index(slice)
    }

    fn index_mut(self, slice: &mut [T]) -> &mut Self::Output {
        let start = cmp::min(self.0.start, slice.len());
        let end = cmp::min(self.0.end, slice.len());
        (start..end).index_mut(slice)
    }
}

#[unstable(feature = "sliceindex_wrappers", issue = "146179")]
unsafe impl<T> SliceIndex<[T]> for Clamp<range::RangeInclusive<usize>> {
    type Output = [T];

    fn get(self, slice: &[T]) -> Option<&Self::Output> {
        let start = cmp::min(self.0.start, slice.len() - 1);
        let end = cmp::min(self.0.last, slice.len() - 1);
        (start..=end).get(slice)
    }

    fn get_mut(self, slice: &mut [T]) -> Option<&mut Self::Output> {
        let start = cmp::min(self.0.start, slice.len() - 1);
        let end = cmp::min(self.0.last, slice.len() - 1);
        (start..=end).get_mut(slice)
    }

    unsafe fn get_unchecked(self, slice: *const [T]) -> *const Self::Output {
        let start = cmp::min(self.0.start, slice.len() - 1);
        let end = cmp::min(self.0.last, slice.len() - 1);
        // SAFETY: `SliceIndex::get_unchecked` 的调用者保证切片非空且范围有效。
        // 因此 `slice.len() - 1` 不会下溢，clamp 后两个端点都在切片内且满足范围顺序。
        unsafe { (start..=end).get_unchecked(slice) }
    }

    unsafe fn get_unchecked_mut(self, slice: *mut [T]) -> *mut Self::Output {
        let start = cmp::min(self.0.start, slice.len() - 1);
        let end = cmp::min(self.0.last, slice.len() - 1);
        // SAFETY: `SliceIndex::get_unchecked_mut` 的调用者保证切片非空且范围有效。
        // 因此 `slice.len() - 1` 不会下溢，clamp 后两个端点都在切片内且满足范围顺序。
        unsafe { (start..=end).get_unchecked_mut(slice) }
    }

    fn index(self, slice: &[T]) -> &Self::Output {
        let start = cmp::min(self.0.start, slice.len() - 1);
        let end = cmp::min(self.0.last, slice.len() - 1);
        (start..=end).index(slice)
    }

    fn index_mut(self, slice: &mut [T]) -> &mut Self::Output {
        let start = cmp::min(self.0.start, slice.len() - 1);
        let end = cmp::min(self.0.last, slice.len() - 1);
        (start..=end).index_mut(slice)
    }
}

#[unstable(feature = "sliceindex_wrappers", issue = "146179")]
unsafe impl<T> SliceIndex<[T]> for Clamp<ops::RangeInclusive<usize>> {
    type Output = [T];

    fn get(self, slice: &[T]) -> Option<&Self::Output> {
        let start = cmp::min(self.0.start, slice.len() - 1);
        let end = cmp::min(self.0.end, slice.len() - 1);
        (start..=end).get(slice)
    }

    fn get_mut(self, slice: &mut [T]) -> Option<&mut Self::Output> {
        let start = cmp::min(self.0.start, slice.len() - 1);
        let end = cmp::min(self.0.end, slice.len() - 1);
        (start..=end).get_mut(slice)
    }

    unsafe fn get_unchecked(self, slice: *const [T]) -> *const Self::Output {
        let start = cmp::min(self.0.start, slice.len() - 1);
        let end = cmp::min(self.0.end, slice.len() - 1);
        // SAFETY: `SliceIndex::get_unchecked` 的调用者保证切片非空且范围有效。
        // 因此 `slice.len() - 1` 不会下溢，clamp 后两个端点都在切片内且满足范围顺序。
        unsafe { (start..=end).get_unchecked(slice) }
    }

    unsafe fn get_unchecked_mut(self, slice: *mut [T]) -> *mut Self::Output {
        let start = cmp::min(self.0.start, slice.len() - 1);
        let end = cmp::min(self.0.end, slice.len() - 1);
        // SAFETY: `SliceIndex::get_unchecked_mut` 的调用者保证切片非空且范围有效。
        // 因此 `slice.len() - 1` 不会下溢，clamp 后两个端点都在切片内且满足范围顺序。
        unsafe { (start..=end).get_unchecked_mut(slice) }
    }

    fn index(self, slice: &[T]) -> &Self::Output {
        let start = cmp::min(self.0.start, slice.len() - 1);
        let end = cmp::min(self.0.end, slice.len() - 1);
        (start..=end).index(slice)
    }

    fn index_mut(self, slice: &mut [T]) -> &mut Self::Output {
        let start = cmp::min(self.0.start, slice.len() - 1);
        let end = cmp::min(self.0.end, slice.len() - 1);
        (start..=end).index_mut(slice)
    }
}

#[unstable(feature = "sliceindex_wrappers", issue = "146179")]
unsafe impl<T> SliceIndex<[T]> for Clamp<range::RangeFrom<usize>> {
    type Output = [T];

    fn get(self, slice: &[T]) -> Option<&Self::Output> {
        (cmp::min(self.0.start, slice.len())..).get(slice)
    }

    fn get_mut(self, slice: &mut [T]) -> Option<&mut Self::Output> {
        (cmp::min(self.0.start, slice.len())..).get_mut(slice)
    }

    unsafe fn get_unchecked(self, slice: *const [T]) -> *const Self::Output {
        // SAFETY: clamp 后起点不超过 `slice.len()`，所以 `start..` 是有效的切片范围。
        unsafe { (cmp::min(self.0.start, slice.len())..).get_unchecked(slice) }
    }

    unsafe fn get_unchecked_mut(self, slice: *mut [T]) -> *mut Self::Output {
        // SAFETY: clamp 后起点不超过 `slice.len()`，所以 `start..` 是有效的切片范围。
        unsafe { (cmp::min(self.0.start, slice.len())..).get_unchecked_mut(slice) }
    }

    fn index(self, slice: &[T]) -> &Self::Output {
        (cmp::min(self.0.start, slice.len())..).index(slice)
    }

    fn index_mut(self, slice: &mut [T]) -> &mut Self::Output {
        (cmp::min(self.0.start, slice.len())..).index_mut(slice)
    }
}

#[unstable(feature = "sliceindex_wrappers", issue = "146179")]
unsafe impl<T> SliceIndex<[T]> for Clamp<ops::RangeFrom<usize>> {
    type Output = [T];

    fn get(self, slice: &[T]) -> Option<&Self::Output> {
        (cmp::min(self.0.start, slice.len())..).get(slice)
    }

    fn get_mut(self, slice: &mut [T]) -> Option<&mut Self::Output> {
        (cmp::min(self.0.start, slice.len())..).get_mut(slice)
    }

    unsafe fn get_unchecked(self, slice: *const [T]) -> *const Self::Output {
        // SAFETY: clamp 后起点不超过 `slice.len()`，所以 `start..` 是有效的切片范围。
        unsafe { (cmp::min(self.0.start, slice.len())..).get_unchecked(slice) }
    }

    unsafe fn get_unchecked_mut(self, slice: *mut [T]) -> *mut Self::Output {
        // SAFETY: clamp 后起点不超过 `slice.len()`，所以 `start..` 是有效的切片范围。
        unsafe { (cmp::min(self.0.start, slice.len())..).get_unchecked_mut(slice) }
    }

    fn index(self, slice: &[T]) -> &Self::Output {
        (cmp::min(self.0.start, slice.len())..).index(slice)
    }

    fn index_mut(self, slice: &mut [T]) -> &mut Self::Output {
        (cmp::min(self.0.start, slice.len())..).index_mut(slice)
    }
}

#[unstable(feature = "sliceindex_wrappers", issue = "146179")]
unsafe impl<T> SliceIndex<[T]> for Clamp<range::RangeTo<usize>> {
    type Output = [T];

    fn get(self, slice: &[T]) -> Option<&Self::Output> {
        (..cmp::min(self.0.end, slice.len())).get(slice)
    }

    fn get_mut(self, slice: &mut [T]) -> Option<&mut Self::Output> {
        (..cmp::min(self.0.end, slice.len())).get_mut(slice)
    }

    unsafe fn get_unchecked(self, slice: *const [T]) -> *const Self::Output {
        // SAFETY: clamp 后终点不超过 `slice.len()`，所以 `..end` 是有效的切片范围。
        unsafe { (..cmp::min(self.0.end, slice.len())).get_unchecked(slice) }
    }

    unsafe fn get_unchecked_mut(self, slice: *mut [T]) -> *mut Self::Output {
        // SAFETY: clamp 后终点不超过 `slice.len()`，所以 `..end` 是有效的切片范围。
        unsafe { (..cmp::min(self.0.end, slice.len())).get_unchecked_mut(slice) }
    }

    fn index(self, slice: &[T]) -> &Self::Output {
        (..cmp::min(self.0.end, slice.len())).index(slice)
    }

    fn index_mut(self, slice: &mut [T]) -> &mut Self::Output {
        (..cmp::min(self.0.end, slice.len())).index_mut(slice)
    }
}

#[unstable(feature = "sliceindex_wrappers", issue = "146179")]
unsafe impl<T> SliceIndex<[T]> for Clamp<range::RangeToInclusive<usize>> {
    type Output = [T];

    fn get(self, slice: &[T]) -> Option<&Self::Output> {
        (..=cmp::min(self.0.last, slice.len() - 1)).get(slice)
    }

    fn get_mut(self, slice: &mut [T]) -> Option<&mut Self::Output> {
        (..=cmp::min(self.0.last, slice.len() - 1)).get_mut(slice)
    }

    unsafe fn get_unchecked(self, slice: *const [T]) -> *const Self::Output {
        // SAFETY: `SliceIndex::get_unchecked` 的调用者保证切片非空；
        // 因此 `slice.len() - 1` 不会下溢，clamp 后的闭区间终点在切片内。
        unsafe { (..=cmp::min(self.0.last, slice.len() - 1)).get_unchecked(slice) }
    }

    unsafe fn get_unchecked_mut(self, slice: *mut [T]) -> *mut Self::Output {
        // SAFETY: `SliceIndex::get_unchecked_mut` 的调用者保证切片非空；
        // 因此 `slice.len() - 1` 不会下溢，clamp 后的闭区间终点在切片内。
        unsafe { (..=cmp::min(self.0.last, slice.len() - 1)).get_unchecked_mut(slice) }
    }

    fn index(self, slice: &[T]) -> &Self::Output {
        (..=cmp::min(self.0.last, slice.len() - 1)).index(slice)
    }

    fn index_mut(self, slice: &mut [T]) -> &mut Self::Output {
        (..=cmp::min(self.0.last, slice.len() - 1)).index_mut(slice)
    }
}

#[unstable(feature = "sliceindex_wrappers", issue = "146179")]
unsafe impl<T> SliceIndex<[T]> for Clamp<ops::RangeToInclusive<usize>> {
    type Output = [T];

    fn get(self, slice: &[T]) -> Option<&Self::Output> {
        (..=cmp::min(self.0.end, slice.len() - 1)).get(slice)
    }

    fn get_mut(self, slice: &mut [T]) -> Option<&mut Self::Output> {
        (..=cmp::min(self.0.end, slice.len() - 1)).get_mut(slice)
    }

    unsafe fn get_unchecked(self, slice: *const [T]) -> *const Self::Output {
        // SAFETY: `SliceIndex::get_unchecked` 的调用者保证切片非空；
        // 因此 `slice.len() - 1` 不会下溢，clamp 后的闭区间终点在切片内。
        unsafe { (..=cmp::min(self.0.end, slice.len() - 1)).get_unchecked(slice) }
    }

    unsafe fn get_unchecked_mut(self, slice: *mut [T]) -> *mut Self::Output {
        // SAFETY: `SliceIndex::get_unchecked_mut` 的调用者保证切片非空；
        // 因此 `slice.len() - 1` 不会下溢，clamp 后的闭区间终点在切片内。
        unsafe { (..=cmp::min(self.0.end, slice.len() - 1)).get_unchecked_mut(slice) }
    }

    fn index(self, slice: &[T]) -> &Self::Output {
        (..=cmp::min(self.0.end, slice.len() - 1)).index(slice)
    }

    fn index_mut(self, slice: &mut [T]) -> &mut Self::Output {
        (..=cmp::min(self.0.end, slice.len() - 1)).index_mut(slice)
    }
}

#[unstable(feature = "sliceindex_wrappers", issue = "146179")]
unsafe impl<T> SliceIndex<[T]> for Clamp<range::RangeFull> {
    type Output = [T];

    fn get(self, slice: &[T]) -> Option<&Self::Output> {
        (..).get(slice)
    }

    fn get_mut(self, slice: &mut [T]) -> Option<&mut Self::Output> {
        (..).get_mut(slice)
    }

    unsafe fn get_unchecked(self, slice: *const [T]) -> *const Self::Output {
        // SAFETY: `RangeFull` 在这里直接返回整个 `slice`，不产生越界范围。
        unsafe { (..).get_unchecked(slice) }
    }

    unsafe fn get_unchecked_mut(self, slice: *mut [T]) -> *mut Self::Output {
        // SAFETY: `RangeFull` 在这里直接返回整个 `slice`，不产生越界范围。
        unsafe { (..).get_unchecked_mut(slice) }
    }

    fn index(self, slice: &[T]) -> &Self::Output {
        (..).index(slice)
    }

    fn index_mut(self, slice: &mut [T]) -> &mut Self::Output {
        (..).index_mut(slice)
    }
}

#[unstable(feature = "sliceindex_wrappers", issue = "146179")]
unsafe impl<T> SliceIndex<[T]> for Last {
    type Output = T;

    fn get(self, slice: &[T]) -> Option<&Self::Output> {
        slice.last()
    }

    fn get_mut(self, slice: &mut [T]) -> Option<&mut Self::Output> {
        slice.last_mut()
    }

    unsafe fn get_unchecked(self, slice: *const [T]) -> *const Self::Output {
        // SAFETY: `SliceIndex::get_unchecked` 的调用者保证切片非空；
        // 因此 `slice.len() - 1` 不会下溢，且它是最后一个有效元素的索引。
        unsafe { slice_get_unchecked(slice, slice.len() - 1) }
    }

    unsafe fn get_unchecked_mut(self, slice: *mut [T]) -> *mut Self::Output {
        // SAFETY: `SliceIndex::get_unchecked_mut` 的调用者保证切片非空；
        // 因此 `slice.len() - 1` 不会下溢，且它是最后一个有效元素的索引。
        unsafe { slice_get_unchecked(slice, slice.len() - 1) }
    }

    fn index(self, slice: &[T]) -> &Self::Output {
        // 注意：这里使用 intrinsic 索引。
        &(*slice)[slice.len() - 1]
    }

    fn index_mut(self, slice: &mut [T]) -> &mut Self::Output {
        // 注意：这里使用 intrinsic 索引。
        &mut (*slice)[slice.len() - 1]
    }
}
