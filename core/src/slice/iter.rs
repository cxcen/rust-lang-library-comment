//! `[T]` 上各类迭代器的定义。

#[macro_use] // 导入 iterator! 和 forward_iterator!。
mod macros;

use super::{from_raw_parts, from_raw_parts_mut};
use crate::hint::assert_unchecked;
use crate::iter::{
    FusedIterator, TrustedLen, TrustedRandomAccess, TrustedRandomAccessNoCoerce, UncheckedIterator,
};
use crate::marker::PhantomData;
use crate::mem::{self, SizedTypeProperties};
use crate::num::NonZero;
use crate::ptr::{NonNull, without_provenance, without_provenance_mut};
use crate::{cmp, fmt};

#[stable(feature = "boxed_slice_into_iter", since = "1.80.0")]
impl<T> !Iterator for [T] {}

#[stable(feature = "rust1", since = "1.0.0")]
impl<'a, T> IntoIterator for &'a [T] {
    type Item = &'a T;
    type IntoIter = Iter<'a, T>;

    fn into_iter(self) -> Iter<'a, T> {
        self.iter()
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<'a, T> IntoIterator for &'a mut [T] {
    type Item = &'a mut T;
    type IntoIter = IterMut<'a, T>;

    fn into_iter(self) -> IterMut<'a, T> {
        self.iter_mut()
    }
}

/// 不可变切片迭代器。
///
/// 该结构体由 [切片][slices] 上的 [`iter`] 方法创建。
///
/// # 示例
///
/// 基本用法：
///
/// ```
/// // 首先，需要一个可调用 `iter` 方法的切片：
/// let slice = &[1, 2, 3];
///
/// // 然后在切片上调用 `iter` 得到 `Iter` 迭代器，
/// // 并遍历它：
/// for element in slice.iter() {
///     println!("{element}");
/// }
///
/// // 实际上，不调用 `iter` 时这个 for 循环也已经可用：
/// for element in slice {
///     println!("{element}");
/// }
/// ```
///
/// [`iter`]: slice::iter
/// [slices]: slice
#[stable(feature = "rust1", since = "1.0.0")]
#[must_use = "iterators are lazy and do nothing unless consumed"]
#[rustc_diagnostic_item = "SliceIter"]
pub struct Iter<'a, T: 'a> {
    /// 指向下一个要返回的元素；如果迭代器为空，则指向末尾后一位位置。
    ///
    /// 对所有 ZST 元素都会使用这个地址，并且它不会改变。
    ptr: NonNull<T>,
    /// 对非 ZST，这是指向末尾后一位元素的非空指针。
    ///
    /// 对 ZST，这是 `ptr::without_provenance_mut(len)`。
    end_or_len: *const T,
    _marker: PhantomData<&'a T>,
}

#[stable(feature = "core_impl_debug", since = "1.9.0")]
impl<T: fmt::Debug> fmt::Debug for Iter<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Iter").field(&self.as_slice()).finish()
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
unsafe impl<T: Sync> Sync for Iter<'_, T> {}
#[stable(feature = "rust1", since = "1.0.0")]
unsafe impl<T: Sync> Send for Iter<'_, T> {}

impl<'a, T> Iter<'a, T> {
    #[inline]
    pub(super) const fn new(slice: &'a [T]) -> Self {
        let len = slice.len();
        let ptr: NonNull<T> = NonNull::from_ref(slice).cast();
        // SAFETY: 与 `IterMut::new` 类似。
        unsafe {
            let end_or_len =
                if T::IS_ZST { without_provenance(len) } else { ptr.as_ptr().add(len) };

            Self { ptr, end_or_len, _marker: PhantomData }
        }
    }

    /// 把底层数据视为原始数据的一个子切片。
    ///
    /// # 示例
    ///
    /// 基本用法：
    ///
    /// ```
    /// // 首先，需要一个可调用 `iter` 方法的切片：
    /// let slice = &[1, 2, 3];
    ///
    /// // 然后在切片上调用 `iter` 得到 `Iter` 迭代器：
    /// let mut iter = slice.iter();
    /// // 这里 `as_slice` 仍返回整个切片，因此会打印 "[1, 2, 3]"：
    /// println!("{:?}", iter.as_slice());
    ///
    /// // 现在，调用 `next` 方法从迭代器中移除第一个元素：
    /// iter.next();
    /// // 这里迭代器不再包含切片的第一个元素，
    /// // 因而 `as_slice` 只返回切片最后两个元素，
    /// // 所以会打印 "[2, 3]"：
    /// println!("{:?}", iter.as_slice());
    ///
    /// // 底层切片没有被修改，仍包含三个元素，
    /// // 因此会打印 "[1, 2, 3]"：
    /// println!("{:?}", slice);
    /// ```
    #[must_use]
    #[stable(feature = "iter_to_slice", since = "1.4.0")]
    #[inline]
    pub fn as_slice(&self) -> &'a [T] {
        self.make_slice()
    }
}

iterator! {struct Iter -> *const T, &'a T, const, {/* no mut */}, as_ref, each_ref, {
    fn is_sorted_by<F>(self, mut compare: F) -> bool
    where
        Self: Sized,
        F: FnMut(&Self::Item, &Self::Item) -> bool,
    {
        self.as_slice().is_sorted_by(|a, b| compare(&a, &b))
    }
}}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T> Clone for Iter<'_, T> {
    #[inline]
    fn clone(&self) -> Self {
        Iter { ptr: self.ptr, end_or_len: self.end_or_len, _marker: self._marker }
    }
}

#[stable(feature = "slice_iter_as_ref", since = "1.13.0")]
impl<T> AsRef<[T]> for Iter<'_, T> {
    #[inline]
    fn as_ref(&self) -> &[T] {
        self.as_slice()
    }
}

/// 可变切片迭代器。
///
/// 该结构体由 [切片][slices] 上的 [`iter_mut`] 方法创建。
///
/// # 示例
///
/// 基本用法：
///
/// ```
/// // 首先，需要一个可调用 `iter_mut` 方法的切片：
/// let slice = &mut [1, 2, 3];
///
/// // 然后在切片上调用 `iter_mut` 得到 `IterMut` 迭代器，
/// // 遍历它并递增每个元素的值：
/// for element in slice.iter_mut() {
///     *element += 1;
/// }
///
/// // 现在得到 "[2, 3, 4]"：
/// println!("{slice:?}");
/// ```
///
/// [`iter_mut`]: slice::iter_mut
/// [slices]: slice
#[stable(feature = "rust1", since = "1.0.0")]
#[must_use = "iterators are lazy and do nothing unless consumed"]
pub struct IterMut<'a, T: 'a> {
    /// 指向下一个要返回的元素；如果迭代器为空，则指向末尾后一位位置。
    ///
    /// 对所有 ZST 元素都会使用这个地址，并且它不会改变。
    ptr: NonNull<T>,
    /// 对非 ZST，这是指向末尾后一位元素的非空指针。
    ///
    /// 对 ZST，这是 `ptr::without_provenance_mut(len)`。
    end_or_len: *mut T,
    _marker: PhantomData<&'a mut T>,
}

#[stable(feature = "core_impl_debug", since = "1.9.0")]
impl<T: fmt::Debug> fmt::Debug for IterMut<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("IterMut").field(&self.make_slice()).finish()
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
unsafe impl<T: Sync> Sync for IterMut<'_, T> {}
#[stable(feature = "rust1", since = "1.0.0")]
unsafe impl<T: Send> Send for IterMut<'_, T> {}

impl<'a, T> IterMut<'a, T> {
    #[inline]
    pub(super) const fn new(slice: &'a mut [T]) -> Self {
        let len = slice.len();
        let ptr: NonNull<T> = NonNull::from_mut(slice).cast();
        // SAFETY: 这里有几层理由：
        //
        // `ptr` 来自 `slice.as_ptr()`，而 `slice` 是有效引用，
        // 因此它非空，可安全使用并传给 `NonNull::new_unchecked`。
        //
        // 将 `slice.len()` 加到起始指针上，会得到切片末尾后一位指针。
        // `end` 永远不会被解引用，只会与 `ptr` 做直接指针相等性检查，
        // 用于判断迭代器是否结束。
        //
        // 对 ZST，end 指针只是长度。它完全不会作为指针使用，
        // 因此没有 provenance 也是可以的。
        //
        // 更多信息见 `next_unchecked!`、`is_empty!` 宏以及 `post_inc_start` 方法。
        unsafe {
            let end_or_len =
                if T::IS_ZST { without_provenance_mut(len) } else { ptr.as_ptr().add(len) };

            Self { ptr, end_or_len, _marker: PhantomData }
        }
    }

    /// 把底层数据视为原始数据的一个子切片。
    ///
    /// 为避免创建相互 alias 的 `&mut` 引用，这个方法被迫消费迭代器。
    ///
    /// # 示例
    ///
    /// 基本用法：
    ///
    /// ```
    /// // 首先，需要一个可调用 `iter_mut` 方法的切片：
    /// let mut slice = &mut [1, 2, 3];
    ///
    /// // 然后在切片上调用 `iter_mut` 得到 `IterMut` 结构体：
    /// let mut iter = slice.iter_mut();
    /// // 现在，调用 `next` 方法移除迭代器的第一个元素，
    /// // 对 `next` 得到的值执行 unwrap 和解引用，并把它的值加 1：
    /// *iter.next().unwrap() += 1;
    /// // 这里迭代器不再包含切片的第一个元素，
    /// // 因而 `into_slice` 只返回切片最后两个元素，
    /// // 所以会打印 "[2, 3]"：
    /// println!("{:?}", iter.into_slice());
    /// // 底层切片仍包含三个元素，但第一个元素已增加 1，
    /// // 因此会打印 "[2, 2, 3]"：
    /// println!("{:?}", slice);
    /// ```
    #[must_use = "`self` will be dropped if the result is not used"]
    #[stable(feature = "iter_to_slice", since = "1.4.0")]
    pub fn into_slice(self) -> &'a mut [T] {
        // SAFETY: 该迭代器由一个可变切片创建，当前起点为 `self.ptr`，
        // 剩余长度为 `len!(self)`；这保证满足 `from_raw_parts_mut` 的全部前置条件。
        unsafe { from_raw_parts_mut(self.ptr.as_ptr(), len!(self)) }
    }

    /// 把底层数据视为原始数据的一个子切片。
    ///
    /// # 示例
    ///
    /// 基本用法：
    ///
    /// ```
    /// // 首先，需要一个可调用 `iter_mut` 方法的切片：
    /// let slice = &mut [1, 2, 3];
    ///
    /// // 然后在切片上调用 `iter_mut` 得到 `IterMut` 迭代器：
    /// let mut iter = slice.iter_mut();
    /// // 这里 `as_slice` 仍返回整个切片，因此会打印 "[1, 2, 3]"：
    /// println!("{:?}", iter.as_slice());
    ///
    /// // 现在，调用 `next` 方法从迭代器中移除第一个元素并递增它的值：
    /// *iter.next().unwrap() += 1;
    /// // 这里迭代器不再包含切片的第一个元素，
    /// // 因而 `as_slice` 只返回切片最后两个元素，
    /// // 所以会打印 "[2, 3]"：
    /// println!("{:?}", iter.as_slice());
    ///
    /// // 底层切片仍包含三个元素，但第一个元素已增加 1，
    /// // 因此会打印 "[2, 2, 3]"：
    /// println!("{:?}", slice);
    /// ```
    #[must_use]
    #[stable(feature = "slice_iter_mut_as_slice", since = "1.53.0")]
    #[inline]
    pub fn as_slice(&self) -> &[T] {
        self.make_slice()
    }

    /// 把底层数据视为原始数据的一个可变子切片。
    ///
    /// # 示例
    ///
    /// 基本用法：
    ///
    /// ```
    /// #![feature(slice_iter_mut_as_mut_slice)]
    ///
    /// let mut slice: &mut [usize] = &mut [1, 2, 3];
    ///
    /// // 首先取得迭代器：
    /// let mut iter = slice.iter_mut();
    /// // 然后从中取得一个可变切片：
    /// let mut_slice = iter.as_mut_slice();
    /// // 因此检查 `as_mut_slice` 方法的返回值时，会得到 "[1, 2, 3]"：
    /// assert_eq!(mut_slice, &mut [1, 2, 3]);
    ///
    /// // 可以用它修改切片：
    /// mut_slice[0] = 4;
    /// mut_slice[2] = 5;
    ///
    /// // 接着可以移动到切片第二个元素，并检查它产出刚才写入的值：
    /// assert_eq!(iter.next(), Some(&mut 4));
    /// // 现在 `as_mut_slice` 返回 "[2, 5]"：
    /// assert_eq!(iter.as_mut_slice(), &mut [2, 5]);
    /// ```
    #[must_use]
    // FIXME: 当它稳定后，取消注释 `AsMut<[T]>` impl。
    #[unstable(feature = "slice_iter_mut_as_mut_slice", issue = "93079")]
    pub fn as_mut_slice(&mut self) -> &mut [T] {
        // SAFETY: 该迭代器由一个可变切片创建，当前起点为 `self.ptr`，
        // 剩余长度为 `len!(self)`；这保证满足 `from_raw_parts_mut` 的全部前置条件。
        unsafe { from_raw_parts_mut(self.ptr.as_ptr(), len!(self)) }
    }
}

#[stable(feature = "slice_iter_mut_as_slice", since = "1.53.0")]
impl<T> AsRef<[T]> for IterMut<'_, T> {
    #[inline]
    fn as_ref(&self) -> &[T] {
        self.as_slice()
    }
}

// #[stable(feature = "slice_iter_mut_as_mut_slice", since = "FIXME")]
// impl<T> AsMut<[T]> for IterMut<'_, T> {
//     fn as_mut(&mut self) -> &mut [T] {
//         self.as_mut_slice()
//     }
// }

iterator! {struct IterMut -> *mut T, &'a mut T, mut, {mut}, as_mut, each_mut, {}}

/// 分割迭代器上的内部抽象，使 splitn、splitn_mut 等只需实现一次。
#[doc(hidden)]
pub(super) trait SplitIter: DoubleEndedIterator {
    /// 将底层迭代器标记为完成，并提取切片的剩余部分。
    fn finish(&mut self) -> Option<Self::Item>;
}

/// 按满足谓词函数的元素分隔子切片的迭代器。
///
/// 该结构体由 [切片][slices] 上的 [`split`] 方法创建。
///
/// # 示例
///
/// ```
/// let slice = [10, 40, 33, 20];
/// let mut iter = slice.split(|num| num % 3 == 0);
/// assert_eq!(iter.next(), Some(&[10, 40][..]));
/// assert_eq!(iter.next(), Some(&[20][..]));
/// assert_eq!(iter.next(), None);
/// ```
///
/// [`split`]: slice::split
/// [slices]: slice
#[stable(feature = "rust1", since = "1.0.0")]
#[must_use = "iterators are lazy and do nothing unless consumed"]
pub struct Split<'a, T: 'a, P>
where
    P: FnMut(&T) -> bool,
{
    // 供 `SplitWhitespace` 和 `SplitAsciiWhitespace` 的 `as_str` 方法使用。
    pub(crate) v: &'a [T],
    pred: P,
    // 供 `SplitAsciiWhitespace` 的 `as_str` 方法使用。
    pub(crate) finished: bool,
}

impl<'a, T: 'a, P: FnMut(&T) -> bool> Split<'a, T, P> {
    #[inline]
    pub(super) fn new(slice: &'a [T], pred: P) -> Self {
        Self { v: slice, pred, finished: false }
    }
    /// 返回一个包含 split 尚未处理元素的切片。
    /// # 示例
    ///
    /// ```
    /// #![feature(split_as_slice)]
    /// let slice = [1,2,3,4,5];
    /// let mut split = slice.split(|v| v % 2 == 0);
    /// assert!(split.next().is_some());
    /// assert_eq!(split.as_slice(), &[3,4,5]);
    /// ```
    #[unstable(feature = "split_as_slice", issue = "96137")]
    pub fn as_slice(&self) -> &'a [T] {
        if self.finished { &[] } else { &self.v }
    }
}

#[stable(feature = "core_impl_debug", since = "1.9.0")]
impl<T: fmt::Debug, P> fmt::Debug for Split<'_, T, P>
where
    P: FnMut(&T) -> bool,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Split").field("v", &self.v).field("finished", &self.finished).finish()
    }
}

// FIXME(#26925) 改用 `#[derive(Clone)]` 后移除此实现。
#[stable(feature = "rust1", since = "1.0.0")]
impl<T, P> Clone for Split<'_, T, P>
where
    P: Clone + FnMut(&T) -> bool,
{
    fn clone(&self) -> Self {
        Split { v: self.v, pred: self.pred.clone(), finished: self.finished }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<'a, T, P> Iterator for Split<'a, T, P>
where
    P: FnMut(&T) -> bool,
{
    type Item = &'a [T];

    #[inline]
    fn next(&mut self) -> Option<&'a [T]> {
        if self.finished {
            return None;
        }

        match self.v.iter().position(|x| (self.pred)(x)) {
            None => self.finish(),
            Some(idx) => {
                let (left, right) =
                    // SAFETY: 如果 v.iter().position 返回 Some(idx)，
                    // 则 idx 必定是 v 的有效索引。
                    unsafe { (self.v.get_unchecked(..idx), self.v.get_unchecked(idx + 1..)) };
                let ret = Some(left);
                self.v = right;
                ret
            }
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.finished {
            (0, Some(0))
        } else {
            // 如果谓词不匹配任何元素，则产出一个切片。
            // 如果它匹配每个元素，则产出 `len() + 1` 个空切片。
            (1, Some(self.v.len() + 1))
        }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<'a, T, P> DoubleEndedIterator for Split<'a, T, P>
where
    P: FnMut(&T) -> bool,
{
    #[inline]
    fn next_back(&mut self) -> Option<&'a [T]> {
        if self.finished {
            return None;
        }

        match self.v.iter().rposition(|x| (self.pred)(x)) {
            None => self.finish(),
            Some(idx) => {
                let (left, right) =
                    // SAFETY: 如果 v.iter().rposition 返回 Some(idx)，
                    // 则 idx 必定是 v 的有效索引。
                    unsafe { (self.v.get_unchecked(..idx), self.v.get_unchecked(idx + 1..)) };
                let ret = Some(right);
                self.v = left;
                ret
            }
        }
    }
}

impl<'a, T, P> SplitIter for Split<'a, T, P>
where
    P: FnMut(&T) -> bool,
{
    #[inline]
    fn finish(&mut self) -> Option<&'a [T]> {
        if self.finished {
            None
        } else {
            self.finished = true;
            Some(self.v)
        }
    }
}

#[stable(feature = "fused", since = "1.26.0")]
impl<T, P> FusedIterator for Split<'_, T, P> where P: FnMut(&T) -> bool {}

/// 按满足谓词函数的元素分隔子切片的迭代器。
/// 与 `Split` 不同，它会把匹配的部分作为子切片的终止元素包含进去。
///
/// 该结构体由 [切片][slices] 上的 [`split_inclusive`] 方法创建。
///
/// # 示例
///
/// ```
/// let slice = [10, 40, 33, 20];
/// let mut iter = slice.split_inclusive(|num| num % 3 == 0);
/// assert_eq!(iter.next(), Some(&[10, 40, 33][..]));
/// assert_eq!(iter.next(), Some(&[20][..]));
/// assert_eq!(iter.next(), None);
/// ```
///
/// [`split_inclusive`]: slice::split_inclusive
/// [slices]: slice
#[stable(feature = "split_inclusive", since = "1.51.0")]
#[must_use = "iterators are lazy and do nothing unless consumed"]
pub struct SplitInclusive<'a, T: 'a, P>
where
    P: FnMut(&T) -> bool,
{
    v: &'a [T],
    pred: P,
    finished: bool,
}

impl<'a, T: 'a, P: FnMut(&T) -> bool> SplitInclusive<'a, T, P> {
    #[inline]
    pub(super) fn new(slice: &'a [T], pred: P) -> Self {
        let finished = slice.is_empty();
        Self { v: slice, pred, finished }
    }
}

#[stable(feature = "split_inclusive", since = "1.51.0")]
impl<T: fmt::Debug, P> fmt::Debug for SplitInclusive<'_, T, P>
where
    P: FnMut(&T) -> bool,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SplitInclusive")
            .field("v", &self.v)
            .field("finished", &self.finished)
            .finish()
    }
}

// FIXME(#26925) 改用 `#[derive(Clone)]` 后移除此实现。
#[stable(feature = "split_inclusive", since = "1.51.0")]
impl<T, P> Clone for SplitInclusive<'_, T, P>
where
    P: Clone + FnMut(&T) -> bool,
{
    fn clone(&self) -> Self {
        SplitInclusive { v: self.v, pred: self.pred.clone(), finished: self.finished }
    }
}

#[stable(feature = "split_inclusive", since = "1.51.0")]
impl<'a, T, P> Iterator for SplitInclusive<'a, T, P>
where
    P: FnMut(&T) -> bool,
{
    type Item = &'a [T];

    #[inline]
    fn next(&mut self) -> Option<&'a [T]> {
        if self.finished {
            return None;
        }

        let idx =
            self.v.iter().position(|x| (self.pred)(x)).map(|idx| idx + 1).unwrap_or(self.v.len());
        if idx == self.v.len() {
            self.finished = true;
        }
        let ret = Some(&self.v[..idx]);
        self.v = &self.v[idx..];
        ret
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.finished {
            (0, Some(0))
        } else {
            // 如果谓词不匹配任何元素，则产出一个切片。
            // 如果它匹配每个元素，则产出 `len()` 个单元素切片，或者一个空切片。
            (1, Some(cmp::max(1, self.v.len())))
        }
    }
}

#[stable(feature = "split_inclusive", since = "1.51.0")]
impl<'a, T, P> DoubleEndedIterator for SplitInclusive<'a, T, P>
where
    P: FnMut(&T) -> bool,
{
    #[inline]
    fn next_back(&mut self) -> Option<&'a [T]> {
        if self.finished {
            return None;
        }

        // self.v 的最后一个索引已经在上一次迭代中检查并发现匹配，
        // 因此从左移一个索引的位置开始搜索新的匹配项。
        let remainder = if self.v.is_empty() { &[] } else { &self.v[..(self.v.len() - 1)] };
        let idx = remainder.iter().rposition(|x| (self.pred)(x)).map(|idx| idx + 1).unwrap_or(0);
        if idx == 0 {
            self.finished = true;
        }
        let ret = Some(&self.v[idx..]);
        self.v = &self.v[..idx];
        ret
    }
}

#[stable(feature = "split_inclusive", since = "1.51.0")]
impl<T, P> FusedIterator for SplitInclusive<'_, T, P> where P: FnMut(&T) -> bool {}

/// 按匹配 `pred` 的元素分隔可变子切片的迭代器。
///
/// 该结构体由 [切片][slices] 上的 [`split_mut`] 方法创建。
///
/// # 示例
///
/// ```
/// let mut v = [10, 40, 30, 20, 60, 50];
/// let iter = v.split_mut(|num| *num % 3 == 0);
/// ```
///
/// [`split_mut`]: slice::split_mut
/// [slices]: slice
#[stable(feature = "rust1", since = "1.0.0")]
#[must_use = "iterators are lazy and do nothing unless consumed"]
pub struct SplitMut<'a, T: 'a, P>
where
    P: FnMut(&T) -> bool,
{
    v: &'a mut [T],
    pred: P,
    finished: bool,
}

impl<'a, T: 'a, P: FnMut(&T) -> bool> SplitMut<'a, T, P> {
    #[inline]
    pub(super) fn new(slice: &'a mut [T], pred: P) -> Self {
        Self { v: slice, pred, finished: false }
    }
}

#[stable(feature = "core_impl_debug", since = "1.9.0")]
impl<T: fmt::Debug, P> fmt::Debug for SplitMut<'_, T, P>
where
    P: FnMut(&T) -> bool,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SplitMut").field("v", &self.v).field("finished", &self.finished).finish()
    }
}

impl<'a, T, P> SplitIter for SplitMut<'a, T, P>
where
    P: FnMut(&T) -> bool,
{
    #[inline]
    fn finish(&mut self) -> Option<&'a mut [T]> {
        if self.finished {
            None
        } else {
            self.finished = true;
            Some(mem::take(&mut self.v))
        }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<'a, T, P> Iterator for SplitMut<'a, T, P>
where
    P: FnMut(&T) -> bool,
{
    type Item = &'a mut [T];

    #[inline]
    fn next(&mut self) -> Option<&'a mut [T]> {
        if self.finished {
            return None;
        }

        match self.v.iter().position(|x| (self.pred)(x)) {
            None => self.finish(),
            Some(idx) => {
                let tmp = mem::take(&mut self.v);
                // idx 是执行分割的元素索引。目标是把 self 设为 idx 之后的区域，
                // 并返回 idx 之前且不包含 idx 的子切片。因此先在 idx 之后分割。
                let (head, tail) = tmp.split_at_mut(idx + 1);
                self.v = tail;
                // 然后返回到找到的元素之前、且不包含该元素的子切片。
                Some(&mut head[..idx])
            }
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.finished {
            (0, Some(0))
        } else {
            // 如果谓词不匹配任何元素，则产出一个切片。
            // 如果它匹配每个元素，则产出 `len() + 1` 个空切片。
            (1, Some(self.v.len() + 1))
        }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<'a, T, P> DoubleEndedIterator for SplitMut<'a, T, P>
where
    P: FnMut(&T) -> bool,
{
    #[inline]
    fn next_back(&mut self) -> Option<&'a mut [T]> {
        if self.finished {
            return None;
        }

        let idx_opt = {
            // 绕开 borrowck 限制。
            let pred = &mut self.pred;
            self.v.iter().rposition(|x| (*pred)(x))
        };
        match idx_opt {
            None => self.finish(),
            Some(idx) => {
                let tmp = mem::take(&mut self.v);
                let (head, tail) = tmp.split_at_mut(idx);
                self.v = head;
                Some(&mut tail[1..])
            }
        }
    }
}

#[stable(feature = "fused", since = "1.26.0")]
impl<T, P> FusedIterator for SplitMut<'_, T, P> where P: FnMut(&T) -> bool {}

/// 按匹配 `pred` 的元素分隔可变子切片的迭代器。
/// 与 `SplitMut` 不同，它会把匹配部分包含在子切片末尾。
///
/// 该结构体由 [切片][slices] 上的 [`split_inclusive_mut`] 方法创建。
///
/// # 示例
///
/// ```
/// let mut v = [10, 40, 30, 20, 60, 50];
/// let iter = v.split_inclusive_mut(|num| *num % 3 == 0);
/// ```
///
/// [`split_inclusive_mut`]: slice::split_inclusive_mut
/// [slices]: slice
#[stable(feature = "split_inclusive", since = "1.51.0")]
#[must_use = "iterators are lazy and do nothing unless consumed"]
pub struct SplitInclusiveMut<'a, T: 'a, P>
where
    P: FnMut(&T) -> bool,
{
    v: &'a mut [T],
    pred: P,
    finished: bool,
}

impl<'a, T: 'a, P: FnMut(&T) -> bool> SplitInclusiveMut<'a, T, P> {
    #[inline]
    pub(super) fn new(slice: &'a mut [T], pred: P) -> Self {
        let finished = slice.is_empty();
        Self { v: slice, pred, finished }
    }
}

#[stable(feature = "split_inclusive", since = "1.51.0")]
impl<T: fmt::Debug, P> fmt::Debug for SplitInclusiveMut<'_, T, P>
where
    P: FnMut(&T) -> bool,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SplitInclusiveMut")
            .field("v", &self.v)
            .field("finished", &self.finished)
            .finish()
    }
}

#[stable(feature = "split_inclusive", since = "1.51.0")]
impl<'a, T, P> Iterator for SplitInclusiveMut<'a, T, P>
where
    P: FnMut(&T) -> bool,
{
    type Item = &'a mut [T];

    #[inline]
    fn next(&mut self) -> Option<&'a mut [T]> {
        if self.finished {
            return None;
        }

        let idx_opt = {
            // 绕开 borrowck 限制。
            let pred = &mut self.pred;
            self.v.iter().position(|x| (*pred)(x))
        };
        let idx = idx_opt.map(|idx| idx + 1).unwrap_or(self.v.len());
        if idx == self.v.len() {
            self.finished = true;
        }
        let tmp = mem::take(&mut self.v);
        let (head, tail) = tmp.split_at_mut(idx);
        self.v = tail;
        Some(head)
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.finished {
            (0, Some(0))
        } else {
            // 如果谓词不匹配任何元素，则产出一个切片。
            // 如果它匹配每个元素，则产出 `len()` 个单元素切片，或者一个空切片。
            (1, Some(cmp::max(1, self.v.len())))
        }
    }
}

#[stable(feature = "split_inclusive", since = "1.51.0")]
impl<'a, T, P> DoubleEndedIterator for SplitInclusiveMut<'a, T, P>
where
    P: FnMut(&T) -> bool,
{
    #[inline]
    fn next_back(&mut self) -> Option<&'a mut [T]> {
        if self.finished {
            return None;
        }

        let idx_opt = if self.v.is_empty() {
            None
        } else {
            // 绕开 borrowck 限制。
            let pred = &mut self.pred;

            // self.v 的最后一个索引已经在上一次迭代中检查并发现匹配，
            // 因此从左移一个索引的位置开始搜索新的匹配项。
            let remainder = &self.v[..(self.v.len() - 1)];
            remainder.iter().rposition(|x| (*pred)(x))
        };
        let idx = idx_opt.map(|idx| idx + 1).unwrap_or(0);
        if idx == 0 {
            self.finished = true;
        }
        let tmp = mem::take(&mut self.v);
        let (head, tail) = tmp.split_at_mut(idx);
        self.v = head;
        Some(tail)
    }
}

#[stable(feature = "split_inclusive", since = "1.51.0")]
impl<T, P> FusedIterator for SplitInclusiveMut<'_, T, P> where P: FnMut(&T) -> bool {}

/// 按满足谓词函数的元素分隔子切片，并从切片末尾开始迭代的迭代器。
///
/// 该结构体由 [切片][slices] 上的 [`rsplit`] 方法创建。
///
/// # 示例
///
/// ```
/// let slice = [11, 22, 33, 0, 44, 55];
/// let mut iter = slice.rsplit(|num| *num == 0);
/// assert_eq!(iter.next(), Some(&[44, 55][..]));
/// assert_eq!(iter.next(), Some(&[11, 22, 33][..]));
/// assert_eq!(iter.next(), None);
/// ```
///
/// [`rsplit`]: slice::rsplit
/// [slices]: slice
#[stable(feature = "slice_rsplit", since = "1.27.0")]
#[must_use = "iterators are lazy and do nothing unless consumed"]
pub struct RSplit<'a, T: 'a, P>
where
    P: FnMut(&T) -> bool,
{
    inner: Split<'a, T, P>,
}

impl<'a, T: 'a, P: FnMut(&T) -> bool> RSplit<'a, T, P> {
    #[inline]
    pub(super) fn new(slice: &'a [T], pred: P) -> Self {
        Self { inner: Split::new(slice, pred) }
    }
}

#[stable(feature = "slice_rsplit", since = "1.27.0")]
impl<T: fmt::Debug, P> fmt::Debug for RSplit<'_, T, P>
where
    P: FnMut(&T) -> bool,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RSplit")
            .field("v", &self.inner.v)
            .field("finished", &self.inner.finished)
            .finish()
    }
}

// FIXME(#26925) 改用 `#[derive(Clone)]` 后移除此实现。
#[stable(feature = "slice_rsplit", since = "1.27.0")]
impl<T, P> Clone for RSplit<'_, T, P>
where
    P: Clone + FnMut(&T) -> bool,
{
    fn clone(&self) -> Self {
        RSplit { inner: self.inner.clone() }
    }
}

#[stable(feature = "slice_rsplit", since = "1.27.0")]
impl<'a, T, P> Iterator for RSplit<'a, T, P>
where
    P: FnMut(&T) -> bool,
{
    type Item = &'a [T];

    #[inline]
    fn next(&mut self) -> Option<&'a [T]> {
        self.inner.next_back()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

#[stable(feature = "slice_rsplit", since = "1.27.0")]
impl<'a, T, P> DoubleEndedIterator for RSplit<'a, T, P>
where
    P: FnMut(&T) -> bool,
{
    #[inline]
    fn next_back(&mut self) -> Option<&'a [T]> {
        self.inner.next()
    }
}

#[stable(feature = "slice_rsplit", since = "1.27.0")]
impl<'a, T, P> SplitIter for RSplit<'a, T, P>
where
    P: FnMut(&T) -> bool,
{
    #[inline]
    fn finish(&mut self) -> Option<&'a [T]> {
        self.inner.finish()
    }
}

#[stable(feature = "slice_rsplit", since = "1.27.0")]
impl<T, P> FusedIterator for RSplit<'_, T, P> where P: FnMut(&T) -> bool {}

/// 按匹配 `pred` 的元素分隔子切片，并从切片末尾开始迭代的迭代器。
///
/// 该结构体由 [切片][slices] 上的 [`rsplit_mut`] 方法创建。
///
/// # 示例
///
/// ```
/// let mut slice = [11, 22, 33, 0, 44, 55];
/// let iter = slice.rsplit_mut(|num| *num == 0);
/// ```
///
/// [`rsplit_mut`]: slice::rsplit_mut
/// [slices]: slice
#[stable(feature = "slice_rsplit", since = "1.27.0")]
#[must_use = "iterators are lazy and do nothing unless consumed"]
pub struct RSplitMut<'a, T: 'a, P>
where
    P: FnMut(&T) -> bool,
{
    inner: SplitMut<'a, T, P>,
}

impl<'a, T: 'a, P: FnMut(&T) -> bool> RSplitMut<'a, T, P> {
    #[inline]
    pub(super) fn new(slice: &'a mut [T], pred: P) -> Self {
        Self { inner: SplitMut::new(slice, pred) }
    }
}

#[stable(feature = "slice_rsplit", since = "1.27.0")]
impl<T: fmt::Debug, P> fmt::Debug for RSplitMut<'_, T, P>
where
    P: FnMut(&T) -> bool,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RSplitMut")
            .field("v", &self.inner.v)
            .field("finished", &self.inner.finished)
            .finish()
    }
}

#[stable(feature = "slice_rsplit", since = "1.27.0")]
impl<'a, T, P> SplitIter for RSplitMut<'a, T, P>
where
    P: FnMut(&T) -> bool,
{
    #[inline]
    fn finish(&mut self) -> Option<&'a mut [T]> {
        self.inner.finish()
    }
}

#[stable(feature = "slice_rsplit", since = "1.27.0")]
impl<'a, T, P> Iterator for RSplitMut<'a, T, P>
where
    P: FnMut(&T) -> bool,
{
    type Item = &'a mut [T];

    #[inline]
    fn next(&mut self) -> Option<&'a mut [T]> {
        self.inner.next_back()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

#[stable(feature = "slice_rsplit", since = "1.27.0")]
impl<'a, T, P> DoubleEndedIterator for RSplitMut<'a, T, P>
where
    P: FnMut(&T) -> bool,
{
    #[inline]
    fn next_back(&mut self) -> Option<&'a mut [T]> {
        self.inner.next()
    }
}

#[stable(feature = "slice_rsplit", since = "1.27.0")]
impl<T, P> FusedIterator for RSplitMut<'_, T, P> where P: FnMut(&T) -> bool {}

/// 私有迭代器：按满足谓词函数的元素分隔子切片，最多分割固定次数。
#[derive(Debug)]
struct GenericSplitN<I> {
    iter: I,
    count: usize,
}

impl<T, I: SplitIter<Item = T>> Iterator for GenericSplitN<I> {
    type Item = T;

    #[inline]
    fn next(&mut self) -> Option<T> {
        match self.count {
            0 => None,
            1 => {
                self.count -= 1;
                self.iter.finish()
            }
            _ => {
                self.count -= 1;
                self.iter.next()
            }
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let (lower, upper_opt) = self.iter.size_hint();
        (
            cmp::min(self.count, lower),
            Some(upper_opt.map_or(self.count, |upper| cmp::min(self.count, upper))),
        )
    }
}

/// 按满足谓词函数的元素分隔子切片，并限制分割次数的迭代器。
///
/// 该结构体由 [切片][slices] 上的 [`splitn`] 方法创建。
///
/// # 示例
///
/// ```
/// let slice = [10, 40, 30, 20, 60, 50];
/// let mut iter = slice.splitn(2, |num| *num % 3 == 0);
/// assert_eq!(iter.next(), Some(&[10, 40][..]));
/// assert_eq!(iter.next(), Some(&[20, 60, 50][..]));
/// assert_eq!(iter.next(), None);
/// ```
///
/// [`splitn`]: slice::splitn
/// [slices]: slice
#[stable(feature = "rust1", since = "1.0.0")]
#[must_use = "iterators are lazy and do nothing unless consumed"]
pub struct SplitN<'a, T: 'a, P>
where
    P: FnMut(&T) -> bool,
{
    inner: GenericSplitN<Split<'a, T, P>>,
}

impl<'a, T: 'a, P: FnMut(&T) -> bool> SplitN<'a, T, P> {
    #[inline]
    pub(super) fn new(s: Split<'a, T, P>, n: usize) -> Self {
        Self { inner: GenericSplitN { iter: s, count: n } }
    }
}

#[stable(feature = "core_impl_debug", since = "1.9.0")]
impl<T: fmt::Debug, P> fmt::Debug for SplitN<'_, T, P>
where
    P: FnMut(&T) -> bool,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SplitN").field("inner", &self.inner).finish()
    }
}

/// 按满足谓词函数的元素分隔子切片、限制分割次数，
/// 并从切片末尾开始迭代的迭代器。
///
/// 该结构体由 [切片][slices] 上的 [`rsplitn`] 方法创建。
///
/// # 示例
///
/// ```
/// let slice = [10, 40, 30, 20, 60, 50];
/// let mut iter = slice.rsplitn(2, |num| *num % 3 == 0);
/// assert_eq!(iter.next(), Some(&[50][..]));
/// assert_eq!(iter.next(), Some(&[10, 40, 30, 20][..]));
/// assert_eq!(iter.next(), None);
/// ```
///
/// [`rsplitn`]: slice::rsplitn
/// [slices]: slice
#[stable(feature = "rust1", since = "1.0.0")]
#[must_use = "iterators are lazy and do nothing unless consumed"]
pub struct RSplitN<'a, T: 'a, P>
where
    P: FnMut(&T) -> bool,
{
    inner: GenericSplitN<RSplit<'a, T, P>>,
}

impl<'a, T: 'a, P: FnMut(&T) -> bool> RSplitN<'a, T, P> {
    #[inline]
    pub(super) fn new(s: RSplit<'a, T, P>, n: usize) -> Self {
        Self { inner: GenericSplitN { iter: s, count: n } }
    }
}

#[stable(feature = "core_impl_debug", since = "1.9.0")]
impl<T: fmt::Debug, P> fmt::Debug for RSplitN<'_, T, P>
where
    P: FnMut(&T) -> bool,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RSplitN").field("inner", &self.inner).finish()
    }
}

/// 按满足谓词函数的元素分隔子切片，并限制分割次数的迭代器。
///
/// 该结构体由 [切片][slices] 上的 [`splitn_mut`] 方法创建。
///
/// # 示例
///
/// ```
/// let mut slice = [10, 40, 30, 20, 60, 50];
/// let iter = slice.splitn_mut(2, |num| *num % 3 == 0);
/// ```
///
/// [`splitn_mut`]: slice::splitn_mut
/// [slices]: slice
#[stable(feature = "rust1", since = "1.0.0")]
#[must_use = "iterators are lazy and do nothing unless consumed"]
pub struct SplitNMut<'a, T: 'a, P>
where
    P: FnMut(&T) -> bool,
{
    inner: GenericSplitN<SplitMut<'a, T, P>>,
}

impl<'a, T: 'a, P: FnMut(&T) -> bool> SplitNMut<'a, T, P> {
    #[inline]
    pub(super) fn new(s: SplitMut<'a, T, P>, n: usize) -> Self {
        Self { inner: GenericSplitN { iter: s, count: n } }
    }
}

#[stable(feature = "core_impl_debug", since = "1.9.0")]
impl<T: fmt::Debug, P> fmt::Debug for SplitNMut<'_, T, P>
where
    P: FnMut(&T) -> bool,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SplitNMut").field("inner", &self.inner).finish()
    }
}

/// 按满足谓词函数的元素分隔子切片、限制分割次数，
/// 并从切片末尾开始迭代的迭代器。
///
/// 该结构体由 [切片][slices] 上的 [`rsplitn_mut`] 方法创建。
///
/// # 示例
///
/// ```
/// let mut slice = [10, 40, 30, 20, 60, 50];
/// let iter = slice.rsplitn_mut(2, |num| *num % 3 == 0);
/// ```
///
/// [`rsplitn_mut`]: slice::rsplitn_mut
/// [slices]: slice
#[stable(feature = "rust1", since = "1.0.0")]
#[must_use = "iterators are lazy and do nothing unless consumed"]
pub struct RSplitNMut<'a, T: 'a, P>
where
    P: FnMut(&T) -> bool,
{
    inner: GenericSplitN<RSplitMut<'a, T, P>>,
}

impl<'a, T: 'a, P: FnMut(&T) -> bool> RSplitNMut<'a, T, P> {
    #[inline]
    pub(super) fn new(s: RSplitMut<'a, T, P>, n: usize) -> Self {
        Self { inner: GenericSplitN { iter: s, count: n } }
    }
}

#[stable(feature = "core_impl_debug", since = "1.9.0")]
impl<T: fmt::Debug, P> fmt::Debug for RSplitNMut<'_, T, P>
where
    P: FnMut(&T) -> bool,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RSplitNMut").field("inner", &self.inner).finish()
    }
}

forward_iterator! { SplitN: T, &'a [T] }
forward_iterator! { RSplitN: T, &'a [T] }
forward_iterator! { SplitNMut: T, &'a mut [T] }
forward_iterator! { RSplitNMut: T, &'a mut [T] }

/// 长度为 `size` 的重叠子切片迭代器。
///
/// 该结构体由 [切片][slices] 上的 [`windows`] 方法创建。
///
/// # 示例
///
/// ```
/// let slice = ['r', 'u', 's', 't'];
/// let mut iter = slice.windows(2);
/// assert_eq!(iter.next(), Some(&['r', 'u'][..]));
/// assert_eq!(iter.next(), Some(&['u', 's'][..]));
/// assert_eq!(iter.next(), Some(&['s', 't'][..]));
/// assert_eq!(iter.next(), None);
/// ```
///
/// [`windows`]: slice::windows
/// [slices]: slice
#[derive(Debug)]
#[stable(feature = "rust1", since = "1.0.0")]
#[must_use = "iterators are lazy and do nothing unless consumed"]
pub struct Windows<'a, T: 'a> {
    v: &'a [T],
    size: NonZero<usize>,
}

impl<'a, T: 'a> Windows<'a, T> {
    #[inline]
    pub(super) const fn new(slice: &'a [T], size: NonZero<usize>) -> Self {
        Self { v: slice, size }
    }
}

// FIXME(#26925) 改用 `#[derive(Clone)]` 后移除此实现。
#[stable(feature = "rust1", since = "1.0.0")]
impl<T> Clone for Windows<'_, T> {
    fn clone(&self) -> Self {
        Windows { v: self.v, size: self.size }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<'a, T> Iterator for Windows<'a, T> {
    type Item = &'a [T];

    #[inline]
    fn next(&mut self) -> Option<&'a [T]> {
        if self.size.get() > self.v.len() {
            None
        } else {
            let ret = Some(&self.v[..self.size.get()]);
            self.v = &self.v[1..];
            ret
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.size.get() > self.v.len() {
            (0, Some(0))
        } else {
            let size = self.v.len() - self.size.get() + 1;
            (size, Some(size))
        }
    }

    #[inline]
    fn count(self) -> usize {
        self.len()
    }

    #[inline]
    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        let size = self.size.get();
        if let Some(rest) = self.v.get(n..)
            && let Some(nth) = rest.get(..size)
        {
            self.v = &rest[1..];
            Some(nth)
        } else {
            // 赋值为 &[] 时，设置长度为 0 比覆盖指针更便宜。
            self.v = &self.v[..0]; // cheaper than &[]
            None
        }
    }

    #[inline]
    fn last(self) -> Option<Self::Item> {
        if self.size.get() > self.v.len() {
            None
        } else {
            let start = self.v.len() - self.size.get();
            Some(&self.v[start..])
        }
    }

    unsafe fn __iterator_get_unchecked(&mut self, idx: usize) -> Self::Item {
        // SAFETY: 调用方保证 `i` 在边界内，这意味着 `i` 不可能溢出 `isize`；
        // `from_raw_parts` 创建的切片是 `self.v` 的子切片，
        // 因而保证在 `self.v` 的生命周期 `'a` 内有效。
        unsafe { from_raw_parts(self.v.as_ptr().add(idx), self.size.get()) }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<'a, T> DoubleEndedIterator for Windows<'a, T> {
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        self.nth_back(0)
    }

    #[inline]
    fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
        if let Some(end) = self.v.len().checked_sub(n)
            && let Some(start) = end.checked_sub(self.size.get())
        {
            let res = &self.v[start..end];
            self.v = &self.v[..end - 1];
            Some(res)
        } else {
            self.v = &self.v[..0]; // cheaper than &[]
            None
        }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T> ExactSizeIterator for Windows<'_, T> {}

#[unstable(feature = "trusted_len", issue = "37572")]
unsafe impl<T> TrustedLen for Windows<'_, T> {}

#[stable(feature = "fused", since = "1.26.0")]
impl<T> FusedIterator for Windows<'_, T> {}

#[doc(hidden)]
#[unstable(feature = "trusted_random_access", issue = "none")]
unsafe impl<'a, T> TrustedRandomAccess for Windows<'a, T> {}

#[doc(hidden)]
#[unstable(feature = "trusted_random_access", issue = "none")]
unsafe impl<'a, T> TrustedRandomAccessNoCoerce for Windows<'a, T> {
    const MAY_HAVE_SIDE_EFFECT: bool = false;
}

/// 从切片开头开始、按（不重叠的）chunk 迭代切片的迭代器；
/// 每个 chunk 包含 `chunk_size` 个元素。
///
/// 当切片长度不能被 chunk 大小整除时，迭代中的最后一个切片就是剩余部分。
///
/// 该结构体由 [切片][slices] 上的 [`chunks`] 方法创建。
///
/// # 示例
///
/// ```
/// let slice = ['l', 'o', 'r', 'e', 'm'];
/// let mut iter = slice.chunks(2);
/// assert_eq!(iter.next(), Some(&['l', 'o'][..]));
/// assert_eq!(iter.next(), Some(&['r', 'e'][..]));
/// assert_eq!(iter.next(), Some(&['m'][..]));
/// assert_eq!(iter.next(), None);
/// ```
///
/// [`chunks`]: slice::chunks
/// [slices]: slice
#[derive(Debug)]
#[stable(feature = "rust1", since = "1.0.0")]
#[must_use = "iterators are lazy and do nothing unless consumed"]
pub struct Chunks<'a, T: 'a> {
    v: &'a [T],
    chunk_size: usize,
}

impl<'a, T: 'a> Chunks<'a, T> {
    #[inline]
    pub(super) const fn new(slice: &'a [T], size: usize) -> Self {
        Self { v: slice, chunk_size: size }
    }
}

// FIXME(#26925) 改用 `#[derive(Clone)]` 后移除此实现。
#[stable(feature = "rust1", since = "1.0.0")]
impl<T> Clone for Chunks<'_, T> {
    fn clone(&self) -> Self {
        Chunks { v: self.v, chunk_size: self.chunk_size }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<'a, T> Iterator for Chunks<'a, T> {
    type Item = &'a [T];

    #[inline]
    fn next(&mut self) -> Option<&'a [T]> {
        if self.v.is_empty() {
            None
        } else {
            let chunksz = cmp::min(self.v.len(), self.chunk_size);
            let (fst, snd) = self.v.split_at(chunksz);
            self.v = snd;
            Some(fst)
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.v.is_empty() {
            (0, Some(0))
        } else {
            let n = self.v.len().div_ceil(self.chunk_size);
            (n, Some(n))
        }
    }

    #[inline]
    fn count(self) -> usize {
        self.len()
    }

    #[inline]
    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        if let Some(start) = n.checked_mul(self.chunk_size)
            && start < self.v.len()
        {
            let rest = &self.v[start..];
            let (chunk, rest) = rest.split_at(self.chunk_size.min(rest.len()));
            self.v = rest;
            Some(chunk)
        } else {
            self.v = &self.v[..0]; // cheaper than &[]
            None
        }
    }

    #[inline]
    fn last(self) -> Option<Self::Item> {
        if self.v.is_empty() {
            None
        } else {
            let start = (self.v.len() - 1) / self.chunk_size * self.chunk_size;
            Some(&self.v[start..])
        }
    }

    unsafe fn __iterator_get_unchecked(&mut self, idx: usize) -> Self::Item {
        let start = idx * self.chunk_size;
        // SAFETY: 调用方保证 `i` 在边界内，这意味着 `start` 必定位于底层
        // `self.v` 切片边界内；这里也确保 `len` 在 `self.v` 边界内。
        // 因此 `start` 不能溢出 `isize`，并且由 `from_raw_parts` 构造的切片
        // 是 `self.v` 的子切片，保证在 `self.v` 的生命周期 `'a` 内有效。
        unsafe {
            let len = cmp::min(self.v.len().unchecked_sub(start), self.chunk_size);
            from_raw_parts(self.v.as_ptr().add(start), len)
        }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<'a, T> DoubleEndedIterator for Chunks<'a, T> {
    #[inline]
    fn next_back(&mut self) -> Option<&'a [T]> {
        if self.v.is_empty() {
            None
        } else {
            let remainder = self.v.len() % self.chunk_size;
            let chunksz = if remainder != 0 { remainder } else { self.chunk_size };
            // SAFETY: split_at_unchecked 要求参数小于或等于长度。
            // 这里能保证这一点，但理由比较微妙：`chunksz` 要么是
            // `self.v.len() % self.chunk_size`，它始终严格小于 `self.v.len()`
            // （如果 `self.chunk_size` 为零，则会 panic）；要么在长度正好能被
            // chunk 大小整除时是 `self.chunk_size`。
            //
            // 这种情况下使用 `self.chunk_size` 看似可能产生大于 `self.v.len()`
            // 的值，但实际上不会：如果 `self.chunk_size` 大于 `self.v.len()`，
            // 那么 `self.v.len() % self.chunk_size` 会返回非零值（注意在该 `if`
            // 分支中，已经知道 `self.v` 非空）。
            let (fst, snd) = unsafe { self.v.split_at_unchecked(self.v.len() - chunksz) };
            self.v = fst;
            Some(snd)
        }
    }

    #[inline]
    fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
        let len = self.len();
        if n < len {
            let start = (len - 1 - n) * self.chunk_size;
            let end = start + (self.v.len() - start).min(self.chunk_size);
            let nth_back = &self.v[start..end];
            self.v = &self.v[..start];
            Some(nth_back)
        } else {
            self.v = &self.v[..0]; // cheaper than &[]
            None
        }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T> ExactSizeIterator for Chunks<'_, T> {}

#[unstable(feature = "trusted_len", issue = "37572")]
unsafe impl<T> TrustedLen for Chunks<'_, T> {}

#[stable(feature = "fused", since = "1.26.0")]
impl<T> FusedIterator for Chunks<'_, T> {}

#[doc(hidden)]
#[unstable(feature = "trusted_random_access", issue = "none")]
unsafe impl<'a, T> TrustedRandomAccess for Chunks<'a, T> {}

#[doc(hidden)]
#[unstable(feature = "trusted_random_access", issue = "none")]
unsafe impl<'a, T> TrustedRandomAccessNoCoerce for Chunks<'a, T> {
    const MAY_HAVE_SIDE_EFFECT: bool = false;
}

/// 从切片开头开始、按（不重叠的）可变 chunk 迭代切片的迭代器；
/// 每个 chunk 包含 `chunk_size` 个元素。
///
/// 当切片长度不能被 chunk 大小整除时，迭代中的最后一个切片就是剩余部分。
///
/// 该结构体由 [切片][slices] 上的 [`chunks_mut`] 方法创建。
///
/// # 示例
///
/// ```
/// let mut slice = ['l', 'o', 'r', 'e', 'm'];
/// let iter = slice.chunks_mut(2);
/// ```
///
/// [`chunks_mut`]: slice::chunks_mut
/// [slices]: slice
#[derive(Debug)]
#[stable(feature = "rust1", since = "1.0.0")]
#[must_use = "iterators are lazy and do nothing unless consumed"]
pub struct ChunksMut<'a, T: 'a> {
    /// # 安全性(Safety）
    /// 该切片指针必须指向至少有 `v.len()` 个 `T` 的有效区域。
    /// 通常这些要求意味着这里可以改用 `&mut [T]`，但实际不能这样做：
    /// `__iterator_get_unchecked` 需要返回 `&mut [T]`，而这保证了某些 aliasing
    /// 属性；如果继续持有完整的原始 `&mut [T]`，就无法维护这些属性。
    /// 包装裸切片则允许从被包装切片中分发互不重叠的 `&mut [T]` 子切片。
    v: *mut [T],
    chunk_size: usize,
    _marker: PhantomData<&'a mut T>,
}

impl<'a, T: 'a> ChunksMut<'a, T> {
    #[inline]
    pub(super) const fn new(slice: &'a mut [T], size: usize) -> Self {
        Self { v: slice, chunk_size: size, _marker: PhantomData }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<'a, T> Iterator for ChunksMut<'a, T> {
    type Item = &'a mut [T];

    #[inline]
    fn next(&mut self) -> Option<&'a mut [T]> {
        if self.v.is_empty() {
            None
        } else {
            let sz = cmp::min(self.v.len(), self.chunk_size);
            // SAFETY: self.v 的契约保证任意 split_at_mut 都有效。
            let (head, tail) = unsafe { self.v.split_at_mut(sz) };
            self.v = tail;
            // SAFETY: 没有其它东西指向、或将会指向该切片的内容。
            Some(unsafe { &mut *head })
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.v.is_empty() {
            (0, Some(0))
        } else {
            let n = self.v.len().div_ceil(self.chunk_size);
            (n, Some(n))
        }
    }

    #[inline]
    fn count(self) -> usize {
        self.len()
    }

    #[inline]
    fn nth(&mut self, n: usize) -> Option<&'a mut [T]> {
        if let Some(start) = n.checked_mul(self.chunk_size)
            && start < self.v.len()
        {
            // SAFETY: `start < self.v.len()` 保证这里在边界内。
            let (_, rest) = unsafe { self.v.split_at_mut(start) };
            // SAFETY: `.min(rest.len()` 保证这里在边界内。
            let (chunk, rest) = unsafe { rest.split_at_mut(self.chunk_size.min(rest.len())) };
            self.v = rest;
            // SAFETY: 没有其它东西指向、或将会指向该切片的内容。
            Some(unsafe { &mut *chunk })
        } else {
            self.v = &mut [];
            None
        }
    }

    #[inline]
    fn last(self) -> Option<Self::Item> {
        if self.v.is_empty() {
            None
        } else {
            let start = (self.v.len() - 1) / self.chunk_size * self.chunk_size;
            // SAFETY: 没有其它东西指向、或将会指向该切片的内容。
            Some(unsafe { &mut *self.v.get_unchecked_mut(start..) })
        }
    }

    unsafe fn __iterator_get_unchecked(&mut self, idx: usize) -> Self::Item {
        let start = idx * self.chunk_size;
        // SAFETY: 见 `Chunks::__iterator_get_unchecked` 和 `self.v` 的注释。
        //
        // 还要注意，调用方也保证不会以同一索引再次调用本方法，
        // 并且不会调用其它会访问该子切片的方法；因此返回可变切片是有效的。
        unsafe {
            let len = cmp::min(self.v.len().unchecked_sub(start), self.chunk_size);
            from_raw_parts_mut(self.v.as_mut_ptr().add(start), len)
        }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<'a, T> DoubleEndedIterator for ChunksMut<'a, T> {
    #[inline]
    fn next_back(&mut self) -> Option<&'a mut [T]> {
        if self.v.is_empty() {
            None
        } else {
            let remainder = self.v.len() % self.chunk_size;
            let sz = if remainder != 0 { remainder } else { self.chunk_size };
            let len = self.v.len();
            // SAFETY: 与 `Chunks::next_back` 类似。
            let (head, tail) = unsafe { self.v.split_at_mut_unchecked(len - sz) };
            self.v = head;
            // SAFETY: 没有其它东西指向、或将会指向该切片的内容。
            Some(unsafe { &mut *tail })
        }
    }

    #[inline]
    fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
        let len = self.len();
        if n < len {
            let start = (len - 1 - n) * self.chunk_size;
            let end = match start.checked_add(self.chunk_size) {
                Some(res) => cmp::min(self.v.len(), res),
                None => self.v.len(),
            };
            // SAFETY: self.v 的契约保证任意 split_at_mut 都有效。
            let (temp, _tail) = unsafe { self.v.split_at_mut(end) };
            // SAFETY: self.v 的契约保证任意 split_at_mut 都有效。
            let (head, nth_back) = unsafe { temp.split_at_mut(start) };
            self.v = head;
            // SAFETY: 没有其它东西指向、或将会指向该切片的内容。
            Some(unsafe { &mut *nth_back })
        } else {
            self.v = &mut [];
            None
        }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T> ExactSizeIterator for ChunksMut<'_, T> {}

#[unstable(feature = "trusted_len", issue = "37572")]
unsafe impl<T> TrustedLen for ChunksMut<'_, T> {}

#[stable(feature = "fused", since = "1.26.0")]
impl<T> FusedIterator for ChunksMut<'_, T> {}

#[doc(hidden)]
#[unstable(feature = "trusted_random_access", issue = "none")]
unsafe impl<'a, T> TrustedRandomAccess for ChunksMut<'a, T> {}

#[doc(hidden)]
#[unstable(feature = "trusted_random_access", issue = "none")]
unsafe impl<'a, T> TrustedRandomAccessNoCoerce for ChunksMut<'a, T> {
    const MAY_HAVE_SIDE_EFFECT: bool = false;
}

#[stable(feature = "rust1", since = "1.0.0")]
unsafe impl<T> Send for ChunksMut<'_, T> where T: Send {}

#[stable(feature = "rust1", since = "1.0.0")]
unsafe impl<T> Sync for ChunksMut<'_, T> where T: Sync {}

/// 从切片开头开始、按（不重叠的）chunk 迭代切片的迭代器；
/// 每个 chunk 包含 `chunk_size` 个元素。
///
/// 当切片长度不能被 chunk 大小整除时，最后最多 `chunk_size-1` 个元素会被省略，
/// 但可通过迭代器的 [`remainder`] 函数取回。
///
/// 该结构体由 [切片][slices] 上的 [`chunks_exact`] 方法创建。
///
/// # 示例
///
/// ```
/// let slice = ['l', 'o', 'r', 'e', 'm'];
/// let mut iter = slice.chunks_exact(2);
/// assert_eq!(iter.next(), Some(&['l', 'o'][..]));
/// assert_eq!(iter.next(), Some(&['r', 'e'][..]));
/// assert_eq!(iter.next(), None);
/// ```
///
/// [`chunks_exact`]: slice::chunks_exact
/// [`remainder`]: ChunksExact::remainder
/// [slices]: slice
#[derive(Debug)]
#[stable(feature = "chunks_exact", since = "1.31.0")]
#[must_use = "iterators are lazy and do nothing unless consumed"]
pub struct ChunksExact<'a, T: 'a> {
    v: &'a [T],
    rem: &'a [T],
    chunk_size: usize,
}

impl<'a, T> ChunksExact<'a, T> {
    #[inline]
    pub(super) const fn new(slice: &'a [T], chunk_size: usize) -> Self {
        let rem = slice.len() % chunk_size;
        let fst_len = slice.len() - rem;
        // SAFETY: 根据上面的构造，0 <= fst_len <= slice.len()。
        let (fst, snd) = unsafe { slice.split_at_unchecked(fst_len) };
        Self { v: fst, rem: snd, chunk_size }
    }

    /// 返回原始切片中不会被迭代器返回的剩余部分。
    /// 返回切片最多包含 `chunk_size-1` 个元素。
    ///
    /// # 示例
    ///
    /// ```
    /// let slice = ['l', 'o', 'r', 'e', 'm'];
    /// let mut iter = slice.chunks_exact(2);
    /// assert_eq!(iter.remainder(), &['m'][..]);
    /// assert_eq!(iter.next(), Some(&['l', 'o'][..]));
    /// assert_eq!(iter.remainder(), &['m'][..]);
    /// assert_eq!(iter.next(), Some(&['r', 'e'][..]));
    /// assert_eq!(iter.remainder(), &['m'][..]);
    /// assert_eq!(iter.next(), None);
    /// assert_eq!(iter.remainder(), &['m'][..]);
    /// ```
    #[must_use]
    #[stable(feature = "chunks_exact", since = "1.31.0")]
    pub fn remainder(&self) -> &'a [T] {
        self.rem
    }
}

// FIXME(#26925) 改用 `#[derive(Clone)]` 后移除此实现。
#[stable(feature = "chunks_exact", since = "1.31.0")]
impl<T> Clone for ChunksExact<'_, T> {
    fn clone(&self) -> Self {
        ChunksExact { v: self.v, rem: self.rem, chunk_size: self.chunk_size }
    }
}

#[stable(feature = "chunks_exact", since = "1.31.0")]
impl<'a, T> Iterator for ChunksExact<'a, T> {
    type Item = &'a [T];

    #[inline]
    fn next(&mut self) -> Option<&'a [T]> {
        self.v.split_at_checked(self.chunk_size).and_then(|(chunk, rest)| {
            self.v = rest;
            Some(chunk)
        })
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = self.v.len() / self.chunk_size;
        (n, Some(n))
    }

    #[inline]
    fn count(self) -> usize {
        self.len()
    }

    #[inline]
    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        if let Some(start) = n.checked_mul(self.chunk_size)
            && start < self.v.len()
        {
            self.v = &self.v[start..];
            self.next()
        } else {
            self.v = &self.v[..0]; // 比 &[] 更便宜。
            None
        }
    }

    #[inline]
    fn last(mut self) -> Option<Self::Item> {
        self.next_back()
    }

    unsafe fn __iterator_get_unchecked(&mut self, idx: usize) -> Self::Item {
        let start = idx * self.chunk_size;
        // SAFETY: 与 `Chunks::__iterator_get_unchecked` 基本相同。
        unsafe { from_raw_parts(self.v.as_ptr().add(start), self.chunk_size) }
    }
}

#[stable(feature = "chunks_exact", since = "1.31.0")]
impl<'a, T> DoubleEndedIterator for ChunksExact<'a, T> {
    #[inline]
    fn next_back(&mut self) -> Option<&'a [T]> {
        if self.v.len() < self.chunk_size {
            None
        } else {
            let (fst, snd) = self.v.split_at(self.v.len() - self.chunk_size);
            self.v = fst;
            Some(snd)
        }
    }

    #[inline]
    fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
        let len = self.len();
        if n < len {
            let start = (len - 1 - n) * self.chunk_size;
            let end = start + self.chunk_size;
            let nth_back = &self.v[start..end];
            self.v = &self.v[..start];
            Some(nth_back)
        } else {
            self.v = &self.v[..0]; // 比 &[] 更便宜。
            None
        }
    }
}

#[stable(feature = "chunks_exact", since = "1.31.0")]
impl<T> ExactSizeIterator for ChunksExact<'_, T> {
    fn is_empty(&self) -> bool {
        self.v.is_empty()
    }
}

#[unstable(feature = "trusted_len", issue = "37572")]
unsafe impl<T> TrustedLen for ChunksExact<'_, T> {}

#[stable(feature = "chunks_exact", since = "1.31.0")]
impl<T> FusedIterator for ChunksExact<'_, T> {}

#[doc(hidden)]
#[unstable(feature = "trusted_random_access", issue = "none")]
unsafe impl<'a, T> TrustedRandomAccess for ChunksExact<'a, T> {}

#[doc(hidden)]
#[unstable(feature = "trusted_random_access", issue = "none")]
unsafe impl<'a, T> TrustedRandomAccessNoCoerce for ChunksExact<'a, T> {
    const MAY_HAVE_SIDE_EFFECT: bool = false;
}

/// 从切片开头开始、按（不重叠的）可变 chunk 迭代切片的迭代器；
/// 每个 chunk 包含 `chunk_size` 个元素。
///
/// 当切片长度不能被 chunk 大小整除时，最后最多 `chunk_size-1` 个元素会被省略，
/// 但可通过迭代器的 [`into_remainder`] 函数取回。
///
/// 该结构体由 [切片][slices] 上的 [`chunks_exact_mut`] 方法创建。
///
/// # 示例
///
/// ```
/// let mut slice = ['l', 'o', 'r', 'e', 'm'];
/// let iter = slice.chunks_exact_mut(2);
/// ```
///
/// [`chunks_exact_mut`]: slice::chunks_exact_mut
/// [`into_remainder`]: ChunksExactMut::into_remainder
/// [slices]: slice
#[derive(Debug)]
#[stable(feature = "chunks_exact", since = "1.31.0")]
#[must_use = "iterators are lazy and do nothing unless consumed"]
pub struct ChunksExactMut<'a, T: 'a> {
    /// # 安全性(Safety）
    /// 该切片指针必须指向至少有 `v.len()` 个 `T` 的有效区域。
    /// 通常这些要求意味着这里可以改用 `&mut [T]`，但实际不能这样做：
    /// `__iterator_get_unchecked` 需要返回 `&mut [T]`，而这保证了某些 aliasing
    /// 属性；如果继续持有完整的原始 `&mut [T]`，就无法维护这些属性。
    /// 包装裸切片则允许从被包装切片中分发互不重叠的 `&mut [T]` 子切片。
    v: *mut [T],
    rem: &'a mut [T], // 迭代器永远不会从这里产出元素，因此它可以保持唯一。
    chunk_size: usize,
    _marker: PhantomData<&'a mut T>,
}

impl<'a, T> ChunksExactMut<'a, T> {
    #[inline]
    pub(super) const fn new(slice: &'a mut [T], chunk_size: usize) -> Self {
        let rem = slice.len() % chunk_size;
        let fst_len = slice.len() - rem;
        // SAFETY: 根据上面的构造，0 <= fst_len <= slice.len()。
        let (fst, snd) = unsafe { slice.split_at_mut_unchecked(fst_len) };
        Self { v: fst, rem: snd, chunk_size, _marker: PhantomData }
    }

    /// 返回原始切片中不会被迭代器返回的剩余部分。
    /// 返回切片最多包含 `chunk_size-1` 个元素。
    #[must_use = "`self` will be dropped if the result is not used"]
    #[stable(feature = "chunks_exact", since = "1.31.0")]
    pub fn into_remainder(self) -> &'a mut [T] {
        self.rem
    }
}

#[stable(feature = "chunks_exact", since = "1.31.0")]
impl<'a, T> Iterator for ChunksExactMut<'a, T> {
    type Item = &'a mut [T];

    #[inline]
    fn next(&mut self) -> Option<&'a mut [T]> {
        // SAFETY: 这里持有 `&mut self`，因此允许临时物化一个可变切片。
        unsafe { &mut *self.v }.split_at_mut_checked(self.chunk_size).and_then(|(chunk, rest)| {
            self.v = rest;
            Some(chunk)
        })
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = self.v.len() / self.chunk_size;
        (n, Some(n))
    }

    #[inline]
    fn count(self) -> usize {
        self.len()
    }

    #[inline]
    fn nth(&mut self, n: usize) -> Option<&'a mut [T]> {
        if let Some(start) = n.checked_mul(self.chunk_size)
            && start < self.v.len()
        {
            // SAFETY: `start < self.v.len()` 保证这里在边界内。
            self.v = unsafe { self.v.split_at_mut(start).1 };
            self.next()
        } else {
            self.v = &mut [];
            None
        }
    }

    #[inline]
    fn last(mut self) -> Option<Self::Item> {
        self.next_back()
    }

    unsafe fn __iterator_get_unchecked(&mut self, idx: usize) -> Self::Item {
        let start = idx * self.chunk_size;
        // SAFETY: 见 `Chunks::__iterator_get_unchecked` 和 `self.v` 的注释。
        unsafe { from_raw_parts_mut(self.v.as_mut_ptr().add(start), self.chunk_size) }
    }
}

#[stable(feature = "chunks_exact", since = "1.31.0")]
impl<'a, T> DoubleEndedIterator for ChunksExactMut<'a, T> {
    #[inline]
    fn next_back(&mut self) -> Option<&'a mut [T]> {
        if self.v.len() < self.chunk_size {
            None
        } else {
            // SAFETY: 由于上面的检查，这个减法在边界内。
            let (head, tail) = unsafe { self.v.split_at_mut(self.v.len() - self.chunk_size) };
            self.v = head;
            // SAFETY: 没有其它东西指向、或将会指向该切片的内容。
            Some(unsafe { &mut *tail })
        }
    }

    #[inline]
    fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
        let len = self.len();
        if n < len {
            let start = (len - 1 - n) * self.chunk_size;
            let end = start + self.chunk_size;
            // SAFETY: self.v 的契约保证任意 split_at_mut 都有效。
            let (temp, _tail) = unsafe { mem::replace(&mut self.v, &mut []).split_at_mut(end) };
            // SAFETY: self.v 的契约保证任意 split_at_mut 都有效。
            let (head, nth_back) = unsafe { temp.split_at_mut(start) };
            self.v = head;
            // SAFETY: 没有其它东西指向、或将会指向该切片的内容。
            Some(unsafe { &mut *nth_back })
        } else {
            self.v = &mut [];
            None
        }
    }
}

#[stable(feature = "chunks_exact", since = "1.31.0")]
impl<T> ExactSizeIterator for ChunksExactMut<'_, T> {
    fn is_empty(&self) -> bool {
        self.v.is_empty()
    }
}

#[unstable(feature = "trusted_len", issue = "37572")]
unsafe impl<T> TrustedLen for ChunksExactMut<'_, T> {}

#[stable(feature = "chunks_exact", since = "1.31.0")]
impl<T> FusedIterator for ChunksExactMut<'_, T> {}

#[doc(hidden)]
#[unstable(feature = "trusted_random_access", issue = "none")]
unsafe impl<'a, T> TrustedRandomAccess for ChunksExactMut<'a, T> {}

#[doc(hidden)]
#[unstable(feature = "trusted_random_access", issue = "none")]
unsafe impl<'a, T> TrustedRandomAccessNoCoerce for ChunksExactMut<'a, T> {
    const MAY_HAVE_SIDE_EFFECT: bool = false;
}

#[stable(feature = "chunks_exact", since = "1.31.0")]
unsafe impl<T> Send for ChunksExactMut<'_, T> where T: Send {}

#[stable(feature = "chunks_exact", since = "1.31.0")]
unsafe impl<T> Sync for ChunksExactMut<'_, T> where T: Sync {}

/// 从切片开头开始、按重叠窗口迭代切片的迭代器；每个窗口包含 `N` 个元素。
///
/// 该结构体由 [切片][slices] 上的 [`array_windows`] 方法创建。
///
/// # 示例
///
/// ```
/// let slice = [0, 1, 2, 3];
/// let mut iter = slice.array_windows::<2>();
/// assert_eq!(iter.next(), Some(&[0, 1]));
/// assert_eq!(iter.next(), Some(&[1, 2]));
/// assert_eq!(iter.next(), Some(&[2, 3]));
/// assert_eq!(iter.next(), None);
/// ```
///
/// [`array_windows`]: slice::array_windows
/// [slices]: slice
#[derive(Debug)]
#[stable(feature = "array_windows", since = "1.94.0")]
#[must_use = "iterators are lazy and do nothing unless consumed"]
pub struct ArrayWindows<'a, T: 'a, const N: usize> {
    v: &'a [T],
}

impl<'a, T: 'a, const N: usize> ArrayWindows<'a, T, N> {
    #[inline]
    pub(super) const fn new(slice: &'a [T]) -> Self {
        Self { v: slice }
    }
}

// FIXME(#26925) 改用 `#[derive(Clone)]` 后移除此实现。
#[stable(feature = "array_windows", since = "1.94.0")]
impl<T, const N: usize> Clone for ArrayWindows<'_, T, N> {
    fn clone(&self) -> Self {
        Self { v: self.v }
    }
}

#[stable(feature = "array_windows", since = "1.94.0")]
impl<'a, T, const N: usize> Iterator for ArrayWindows<'a, T, N> {
    type Item = &'a [T; N];

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let ret = self.v.first_chunk();
        if ret.is_some() {
            self.v = &self.v[1..];
        }
        ret
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let size = self.v.len().saturating_sub(N - 1);
        (size, Some(size))
    }

    #[inline]
    fn count(self) -> usize {
        self.len()
    }

    #[inline]
    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        let idx = n.min(self.v.len());
        self.v = &self.v[idx..];
        self.next()
    }

    #[inline]
    fn last(self) -> Option<Self::Item> {
        self.v.last_chunk()
    }

    unsafe fn __iterator_get_unchecked(&mut self, idx: usize) -> Self::Item {
        // SAFETY: 调用方保证 `idx` 在边界内，这意味着 `idx` 不可能溢出 `isize`；
        // `cast_array` 创建的“切片”是 `self.v` 的子切片，
        // 因而保证在 `self.v` 的生命周期 `'a` 内有效。
        unsafe { &*self.v.as_ptr().add(idx).cast_array() }
    }
}

#[stable(feature = "array_windows", since = "1.94.0")]
impl<'a, T, const N: usize> DoubleEndedIterator for ArrayWindows<'a, T, N> {
    #[inline]
    fn next_back(&mut self) -> Option<&'a [T; N]> {
        let ret = self.v.last_chunk();
        if ret.is_some() {
            self.v = &self.v[..self.v.len() - 1];
        }
        ret
    }

    #[inline]
    fn nth_back(&mut self, n: usize) -> Option<&'a [T; N]> {
        let idx = self.v.len().saturating_sub(n);
        self.v = &self.v[..idx];
        self.next_back()
    }
}

#[stable(feature = "array_windows", since = "1.94.0")]
impl<T, const N: usize> ExactSizeIterator for ArrayWindows<'_, T, N> {
    fn is_empty(&self) -> bool {
        self.v.len() < N
    }
}

#[unstable(feature = "trusted_len", issue = "37572")]
unsafe impl<T, const N: usize> TrustedLen for ArrayWindows<'_, T, N> {}

#[stable(feature = "array_windows", since = "1.94.0")]
impl<T, const N: usize> FusedIterator for ArrayWindows<'_, T, N> {}

#[doc(hidden)]
#[unstable(feature = "trusted_random_access", issue = "none")]
unsafe impl<T, const N: usize> TrustedRandomAccess for ArrayWindows<'_, T, N> {}

#[doc(hidden)]
#[unstable(feature = "trusted_random_access", issue = "none")]
unsafe impl<T, const N: usize> TrustedRandomAccessNoCoerce for ArrayWindows<'_, T, N> {
    const MAY_HAVE_SIDE_EFFECT: bool = false;
}

/// 从切片末尾开始、按（不重叠的）chunk 迭代切片的迭代器；
/// 每个 chunk 包含 `chunk_size` 个元素。
///
/// 当切片长度不能被 chunk 大小整除时，迭代中的最后一个切片就是剩余部分。
///
/// 该结构体由 [切片][slices] 上的 [`rchunks`] 方法创建。
///
/// # 示例
///
/// ```
/// let slice = ['l', 'o', 'r', 'e', 'm'];
/// let mut iter = slice.rchunks(2);
/// assert_eq!(iter.next(), Some(&['e', 'm'][..]));
/// assert_eq!(iter.next(), Some(&['o', 'r'][..]));
/// assert_eq!(iter.next(), Some(&['l'][..]));
/// assert_eq!(iter.next(), None);
/// ```
///
/// [`rchunks`]: slice::rchunks
/// [slices]: slice
#[derive(Debug)]
#[stable(feature = "rchunks", since = "1.31.0")]
#[must_use = "iterators are lazy and do nothing unless consumed"]
pub struct RChunks<'a, T: 'a> {
    v: &'a [T],
    chunk_size: usize,
}

impl<'a, T: 'a> RChunks<'a, T> {
    #[inline]
    pub(super) const fn new(slice: &'a [T], size: usize) -> Self {
        Self { v: slice, chunk_size: size }
    }
}

// FIXME(#26925) 改用 `#[derive(Clone)]` 后移除此实现。
#[stable(feature = "rchunks", since = "1.31.0")]
impl<T> Clone for RChunks<'_, T> {
    fn clone(&self) -> Self {
        RChunks { v: self.v, chunk_size: self.chunk_size }
    }
}

#[stable(feature = "rchunks", since = "1.31.0")]
impl<'a, T> Iterator for RChunks<'a, T> {
    type Item = &'a [T];

    #[inline]
    fn next(&mut self) -> Option<&'a [T]> {
        if self.v.is_empty() {
            None
        } else {
            let idx = self.v.len().saturating_sub(self.chunk_size);
            // SAFETY: self.chunk_size() > 0，因此 0 <= idx < self.v.len()。
            // 所以 `idx` 位于 `self.v` 边界内，可作为 `split_at_mut_unchecked` 的有效参数。
            let (rest, chunk) = unsafe { self.v.split_at_unchecked(idx) };
            self.v = rest;
            Some(chunk)
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.v.is_empty() {
            (0, Some(0))
        } else {
            let n = self.v.len().div_ceil(self.chunk_size);
            (n, Some(n))
        }
    }

    #[inline]
    fn count(self) -> usize {
        self.len()
    }

    #[inline]
    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        if let Some(end) = n.checked_mul(self.chunk_size)
            && end < self.v.len()
        {
            let end = self.v.len() - end;
            let rest = &self.v[..end];
            let (rest, chunk) = rest.split_at(end.saturating_sub(self.chunk_size));
            self.v = rest;
            Some(chunk)
        } else {
            self.v = &self.v[..0]; // 比 &[] 更便宜。
            None
        }
    }

    #[inline]
    fn last(self) -> Option<Self::Item> {
        if self.v.is_empty() {
            None
        } else {
            let rem = self.v.len() % self.chunk_size;
            let end = if rem == 0 { self.chunk_size } else { rem };
            Some(&self.v[0..end])
        }
    }

    unsafe fn __iterator_get_unchecked(&mut self, idx: usize) -> Self::Item {
        let end = self.v.len() - idx * self.chunk_size;
        let start = end.saturating_sub(self.chunk_size);
        // SAFETY: 与 `Chunks::__iterator_get_unchecked` 基本相同。
        unsafe { from_raw_parts(self.v.as_ptr().add(start), end - start) }
    }
}

#[stable(feature = "rchunks", since = "1.31.0")]
impl<'a, T> DoubleEndedIterator for RChunks<'a, T> {
    #[inline]
    fn next_back(&mut self) -> Option<&'a [T]> {
        if self.v.is_empty() {
            None
        } else {
            let remainder = self.v.len() % self.chunk_size;
            let chunksz = if remainder != 0 { remainder } else { self.chunk_size };
            // SAFETY: 与 Chunks::next_back 类似。
            let (fst, snd) = unsafe { self.v.split_at_unchecked(chunksz) };
            self.v = snd;
            Some(fst)
        }
    }

    #[inline]
    fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
        let len = self.len();
        if n < len {
            let offset_from_end = (len - 1 - n) * self.chunk_size;
            let end = self.v.len() - offset_from_end;
            let start = end.saturating_sub(self.chunk_size);
            let nth_back = &self.v[start..end];
            self.v = &self.v[end..];
            Some(nth_back)
        } else {
            self.v = &self.v[..0]; // 比 &[] 更便宜。
            None
        }
    }
}

#[stable(feature = "rchunks", since = "1.31.0")]
impl<T> ExactSizeIterator for RChunks<'_, T> {}

#[unstable(feature = "trusted_len", issue = "37572")]
unsafe impl<T> TrustedLen for RChunks<'_, T> {}

#[stable(feature = "rchunks", since = "1.31.0")]
impl<T> FusedIterator for RChunks<'_, T> {}

#[doc(hidden)]
#[unstable(feature = "trusted_random_access", issue = "none")]
unsafe impl<'a, T> TrustedRandomAccess for RChunks<'a, T> {}

#[doc(hidden)]
#[unstable(feature = "trusted_random_access", issue = "none")]
unsafe impl<'a, T> TrustedRandomAccessNoCoerce for RChunks<'a, T> {
    const MAY_HAVE_SIDE_EFFECT: bool = false;
}

/// 从切片末尾开始、按（不重叠的）可变 chunk 迭代切片的迭代器；
/// 每个 chunk 包含 `chunk_size` 个元素。
///
/// 当切片长度不能被 chunk 大小整除时，迭代中的最后一个切片就是剩余部分。
///
/// 该结构体由 [切片][slices] 上的 [`rchunks_mut`] 方法创建。
///
/// # 示例
///
/// ```
/// let mut slice = ['l', 'o', 'r', 'e', 'm'];
/// let iter = slice.rchunks_mut(2);
/// ```
///
/// [`rchunks_mut`]: slice::rchunks_mut
/// [slices]: slice
#[derive(Debug)]
#[stable(feature = "rchunks", since = "1.31.0")]
#[must_use = "iterators are lazy and do nothing unless consumed"]
pub struct RChunksMut<'a, T: 'a> {
    /// # 安全性(Safety）
    /// 该切片指针必须指向至少有 `v.len()` 个 `T` 的有效区域。
    /// 通常这些要求意味着这里可以改用 `&mut [T]`，但实际不能这样做：
    /// `__iterator_get_unchecked` 需要返回 `&mut [T]`，而这保证了某些 aliasing
    /// 属性；如果继续持有完整的原始 `&mut [T]`，就无法维护这些属性。
    /// 包装裸切片则允许从被包装切片中分发互不重叠的 `&mut [T]` 子切片。
    v: *mut [T],
    chunk_size: usize,
    _marker: PhantomData<&'a mut T>,
}

impl<'a, T: 'a> RChunksMut<'a, T> {
    #[inline]
    pub(super) const fn new(slice: &'a mut [T], size: usize) -> Self {
        Self { v: slice, chunk_size: size, _marker: PhantomData }
    }
}

#[stable(feature = "rchunks", since = "1.31.0")]
impl<'a, T> Iterator for RChunksMut<'a, T> {
    type Item = &'a mut [T];

    #[inline]
    fn next(&mut self) -> Option<&'a mut [T]> {
        if self.v.is_empty() {
            None
        } else {
            let idx = self.v.len().saturating_sub(self.chunk_size);
            // SAFETY: self.chunk_size() > 0，因此 0 <= idx < self.v.len()。
            // 所以 `idx` 位于 `self.v` 边界内，可作为 `split_at_mut_unchecked` 的有效参数。
            let (rest, chunk) = unsafe { self.v.split_at_mut_unchecked(idx) };
            self.v = rest;
            // SAFETY: 没有其它东西指向、或将会指向该切片的内容。
            Some(unsafe { &mut *chunk })
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.v.is_empty() {
            (0, Some(0))
        } else {
            let n = self.v.len().div_ceil(self.chunk_size);
            (n, Some(n))
        }
    }

    #[inline]
    fn count(self) -> usize {
        self.len()
    }

    #[inline]
    fn nth(&mut self, n: usize) -> Option<&'a mut [T]> {
        if let Some(end) = n.checked_mul(self.chunk_size)
            && end < self.v.len()
        {
            let end = self.v.len() - end;
            // SAFETY: self.v 的契约保证任意 split_at_mut 都有效。
            let (rest, _) = unsafe { self.v.split_at_mut(end) };
            // SAFETY: self.v 的契约保证任意 split_at_mut 都有效。
            let (rest, chunk) = unsafe { rest.split_at_mut(end.saturating_sub(self.chunk_size)) };
            self.v = rest;
            // SAFETY: 没有其它东西指向、或将会指向该切片的内容。
            Some(unsafe { &mut *chunk })
        } else {
            self.v = &mut [];
            None
        }
    }

    #[inline]
    fn last(self) -> Option<Self::Item> {
        if self.v.is_empty() {
            None
        } else {
            let rem = self.v.len() % self.chunk_size;
            let end = if rem == 0 { self.chunk_size } else { rem };
            // SAFETY: 没有其它东西指向、或将会指向该切片的内容。
            Some(unsafe { &mut *self.v.get_unchecked_mut(0..end) })
        }
    }

    unsafe fn __iterator_get_unchecked(&mut self, idx: usize) -> Self::Item {
        let end = self.v.len() - idx * self.chunk_size;
        let start = end.saturating_sub(self.chunk_size);
        // SAFETY: 见 `RChunks::__iterator_get_unchecked`、
        // `ChunksMut::__iterator_get_unchecked` 和 `self.v` 的注释。
        unsafe { from_raw_parts_mut(self.v.as_mut_ptr().add(start), end - start) }
    }
}

#[stable(feature = "rchunks", since = "1.31.0")]
impl<'a, T> DoubleEndedIterator for RChunksMut<'a, T> {
    #[inline]
    fn next_back(&mut self) -> Option<&'a mut [T]> {
        if self.v.is_empty() {
            None
        } else {
            let remainder = self.v.len() % self.chunk_size;
            let sz = if remainder != 0 { remainder } else { self.chunk_size };
            // SAFETY: 与 `Chunks::next_back` 类似。
            let (head, tail) = unsafe { self.v.split_at_mut_unchecked(sz) };
            self.v = tail;
            // SAFETY: 没有其它东西指向、或将会指向该切片的内容。
            Some(unsafe { &mut *head })
        }
    }

    #[inline]
    fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
        let len = self.len();
        if n < len {
            // 不会下溢，因为 `n < len`。
            let offset_from_end = (len - 1 - n) * self.chunk_size;
            let end = self.v.len() - offset_from_end;
            let start = end.saturating_sub(self.chunk_size);
            // SAFETY: self.v 的契约保证任意 split_at_mut 都有效。
            let (tmp, tail) = unsafe { self.v.split_at_mut(end) };
            // SAFETY: self.v 的契约保证任意 split_at_mut 都有效。
            let (_, nth_back) = unsafe { tmp.split_at_mut(start) };
            self.v = tail;
            // SAFETY: 没有其它东西指向、或将会指向该切片的内容。
            Some(unsafe { &mut *nth_back })
        } else {
            self.v = &mut [];
            None
        }
    }
}

#[stable(feature = "rchunks", since = "1.31.0")]
impl<T> ExactSizeIterator for RChunksMut<'_, T> {}

#[unstable(feature = "trusted_len", issue = "37572")]
unsafe impl<T> TrustedLen for RChunksMut<'_, T> {}

#[stable(feature = "rchunks", since = "1.31.0")]
impl<T> FusedIterator for RChunksMut<'_, T> {}

#[doc(hidden)]
#[unstable(feature = "trusted_random_access", issue = "none")]
unsafe impl<'a, T> TrustedRandomAccess for RChunksMut<'a, T> {}

#[doc(hidden)]
#[unstable(feature = "trusted_random_access", issue = "none")]
unsafe impl<'a, T> TrustedRandomAccessNoCoerce for RChunksMut<'a, T> {
    const MAY_HAVE_SIDE_EFFECT: bool = false;
}

#[stable(feature = "rchunks", since = "1.31.0")]
unsafe impl<T> Send for RChunksMut<'_, T> where T: Send {}

#[stable(feature = "rchunks", since = "1.31.0")]
unsafe impl<T> Sync for RChunksMut<'_, T> where T: Sync {}

/// 从切片末尾开始、按（不重叠的）chunk 迭代切片的迭代器；
/// 每个 chunk 包含 `chunk_size` 个元素。
///
/// 当切片长度不能被 chunk 大小整除时，最后最多 `chunk_size-1` 个元素会被省略，
/// 但可通过迭代器的 [`remainder`] 函数取回。
///
/// 该结构体由 [切片][slices] 上的 [`rchunks_exact`] 方法创建。
///
/// # 示例
///
/// ```
/// let slice = ['l', 'o', 'r', 'e', 'm'];
/// let mut iter = slice.rchunks_exact(2);
/// assert_eq!(iter.next(), Some(&['e', 'm'][..]));
/// assert_eq!(iter.next(), Some(&['o', 'r'][..]));
/// assert_eq!(iter.next(), None);
/// ```
///
/// [`rchunks_exact`]: slice::rchunks_exact
/// [`remainder`]: RChunksExact::remainder
/// [slices]: slice
#[derive(Debug)]
#[stable(feature = "rchunks", since = "1.31.0")]
#[must_use = "iterators are lazy and do nothing unless consumed"]
pub struct RChunksExact<'a, T: 'a> {
    v: &'a [T],
    rem: &'a [T],
    chunk_size: usize,
}

impl<'a, T> RChunksExact<'a, T> {
    #[inline]
    pub(super) const fn new(slice: &'a [T], chunk_size: usize) -> Self {
        let rem = slice.len() % chunk_size;
        // SAFETY: 根据上面的构造，0 <= rem <= slice.len()。
        let (fst, snd) = unsafe { slice.split_at_unchecked(rem) };
        Self { v: snd, rem: fst, chunk_size }
    }

    /// 返回原始切片中不会被迭代器返回的剩余部分。
    /// 返回切片最多包含 `chunk_size-1` 个元素。
    ///
    /// # 示例
    ///
    /// ```
    /// let slice = ['l', 'o', 'r', 'e', 'm'];
    /// let mut iter = slice.rchunks_exact(2);
    /// assert_eq!(iter.remainder(), &['l'][..]);
    /// assert_eq!(iter.next(), Some(&['e', 'm'][..]));
    /// assert_eq!(iter.remainder(), &['l'][..]);
    /// assert_eq!(iter.next(), Some(&['o', 'r'][..]));
    /// assert_eq!(iter.remainder(), &['l'][..]);
    /// assert_eq!(iter.next(), None);
    /// assert_eq!(iter.remainder(), &['l'][..]);
    /// ```
    #[must_use]
    #[stable(feature = "rchunks", since = "1.31.0")]
    #[rustc_const_unstable(feature = "const_slice_make_iter", issue = "137737")]
    pub const fn remainder(&self) -> &'a [T] {
        self.rem
    }
}

// FIXME(#26925) 改用 `#[derive(Clone)]` 后移除此实现。
#[stable(feature = "rchunks", since = "1.31.0")]
impl<'a, T> Clone for RChunksExact<'a, T> {
    fn clone(&self) -> RChunksExact<'a, T> {
        RChunksExact { v: self.v, rem: self.rem, chunk_size: self.chunk_size }
    }
}

#[stable(feature = "rchunks", since = "1.31.0")]
impl<'a, T> Iterator for RChunksExact<'a, T> {
    type Item = &'a [T];

    #[inline]
    fn next(&mut self) -> Option<&'a [T]> {
        if self.v.len() < self.chunk_size {
            None
        } else {
            let (fst, snd) = self.v.split_at(self.v.len() - self.chunk_size);
            self.v = fst;
            Some(snd)
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = self.v.len() / self.chunk_size;
        (n, Some(n))
    }

    #[inline]
    fn count(self) -> usize {
        self.len()
    }

    #[inline]
    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        if let Some(end) = n.checked_mul(self.chunk_size)
            && end < self.v.len()
        {
            self.v = &self.v[..self.v.len() - end];
            self.next()
        } else {
            self.v = &self.v[..0]; // 比 &[] 更便宜。
            None
        }
    }

    #[inline]
    fn last(mut self) -> Option<Self::Item> {
        self.next_back()
    }

    unsafe fn __iterator_get_unchecked(&mut self, idx: usize) -> Self::Item {
        let end = self.v.len() - idx * self.chunk_size;
        let start = end - self.chunk_size;
        // SAFETY: 与 `Chunks::__iterator_get_unchecked` 基本相同。
        unsafe { from_raw_parts(self.v.as_ptr().add(start), self.chunk_size) }
    }
}

#[stable(feature = "rchunks", since = "1.31.0")]
impl<'a, T> DoubleEndedIterator for RChunksExact<'a, T> {
    #[inline]
    fn next_back(&mut self) -> Option<&'a [T]> {
        if self.v.len() < self.chunk_size {
            None
        } else {
            let (fst, snd) = self.v.split_at(self.chunk_size);
            self.v = snd;
            Some(fst)
        }
    }

    #[inline]
    fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
        let len = self.len();
        if n < len {
            // 现在已知 `n` 对应一个 chunk，因此这些操作都不会下溢或溢出。
            let offset = (len - n) * self.chunk_size;
            let start = self.v.len() - offset;
            let end = start + self.chunk_size;
            let nth_back = &self.v[start..end];
            self.v = &self.v[end..];
            Some(nth_back)
        } else {
            self.v = &self.v[..0]; // 比 &[] 更便宜。
            None
        }
    }
}

#[stable(feature = "rchunks", since = "1.31.0")]
impl<'a, T> ExactSizeIterator for RChunksExact<'a, T> {
    fn is_empty(&self) -> bool {
        self.v.is_empty()
    }
}

#[unstable(feature = "trusted_len", issue = "37572")]
unsafe impl<T> TrustedLen for RChunksExact<'_, T> {}

#[stable(feature = "rchunks", since = "1.31.0")]
impl<T> FusedIterator for RChunksExact<'_, T> {}

#[doc(hidden)]
#[unstable(feature = "trusted_random_access", issue = "none")]
unsafe impl<'a, T> TrustedRandomAccess for RChunksExact<'a, T> {}

#[doc(hidden)]
#[unstable(feature = "trusted_random_access", issue = "none")]
unsafe impl<'a, T> TrustedRandomAccessNoCoerce for RChunksExact<'a, T> {
    const MAY_HAVE_SIDE_EFFECT: bool = false;
}

/// 从切片末尾开始、按（不重叠的）可变 chunk 迭代切片的迭代器；
/// 每个 chunk 包含 `chunk_size` 个元素。
///
/// 当切片长度不能被 chunk 大小整除时，最后最多 `chunk_size-1` 个元素会被省略，
/// 但可通过迭代器的 [`into_remainder`] 函数取回。
///
/// 该结构体由 [切片][slices] 上的 [`rchunks_exact_mut`] 方法创建。
///
/// # 示例
///
/// ```
/// let mut slice = ['l', 'o', 'r', 'e', 'm'];
/// let iter = slice.rchunks_exact_mut(2);
/// ```
///
/// [`rchunks_exact_mut`]: slice::rchunks_exact_mut
/// [`into_remainder`]: RChunksExactMut::into_remainder
/// [slices]: slice
#[derive(Debug)]
#[stable(feature = "rchunks", since = "1.31.0")]
#[must_use = "iterators are lazy and do nothing unless consumed"]
pub struct RChunksExactMut<'a, T: 'a> {
    /// # 安全性(Safety）
    /// 该切片指针必须指向至少有 `v.len()` 个 `T` 的有效区域。
    /// 通常这些要求意味着这里可以改用 `&mut [T]`，但实际不能这样做：
    /// `__iterator_get_unchecked` 需要返回 `&mut [T]`，而这保证了某些 aliasing
    /// 属性；如果继续持有完整的原始 `&mut [T]`，就无法维护这些属性。
    /// 包装裸切片则允许从被包装切片中分发互不重叠的 `&mut [T]` 子切片。
    v: *mut [T],
    rem: &'a mut [T],
    chunk_size: usize,
}

impl<'a, T> RChunksExactMut<'a, T> {
    #[inline]
    pub(super) const fn new(slice: &'a mut [T], chunk_size: usize) -> Self {
        let rem = slice.len() % chunk_size;
        // SAFETY: 根据上面的构造，0 <= rem <= slice.len()。
        let (fst, snd) = unsafe { slice.split_at_mut_unchecked(rem) };
        Self { v: snd, rem: fst, chunk_size }
    }

    /// 返回原始切片中不会由迭代器产出的 remainder。
    ///
    /// 返回的切片最多包含 `chunk_size-1` 个元素。
    #[must_use = "`self` will be dropped if the result is not used"]
    #[stable(feature = "rchunks", since = "1.31.0")]
    #[rustc_const_unstable(feature = "const_slice_make_iter", issue = "137737")]
    pub const fn into_remainder(self) -> &'a mut [T] {
        self.rem
    }
}

#[stable(feature = "rchunks", since = "1.31.0")]
impl<'a, T> Iterator for RChunksExactMut<'a, T> {
    type Item = &'a mut [T];

    #[inline]
    fn next(&mut self) -> Option<&'a mut [T]> {
        if self.v.len() < self.chunk_size {
            None
        } else {
            let len = self.v.len();
            // SAFETY: self.v 的契约保证任意 split_at_mut 都有效。
            let (head, tail) = unsafe { self.v.split_at_mut(len - self.chunk_size) };
            self.v = head;
            // SAFETY: 没有其它东西指向、或将会指向该切片的内容。
            Some(unsafe { &mut *tail })
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = self.v.len() / self.chunk_size;
        (n, Some(n))
    }

    #[inline]
    fn count(self) -> usize {
        self.len()
    }

    #[inline]
    fn nth(&mut self, n: usize) -> Option<&'a mut [T]> {
        if let Some(end) = n.checked_mul(self.chunk_size)
            && end < self.v.len()
        {
            let idx = self.v.len() - end;
            // SAFETY: self.v 的契约保证任意 split_at_mut 都有效。
            let (fst, _) = unsafe { self.v.split_at_mut(idx) };
            self.v = fst;
            self.next()
        } else {
            self.v = &mut [];
            None
        }
    }

    #[inline]
    fn last(mut self) -> Option<Self::Item> {
        self.next_back()
    }

    unsafe fn __iterator_get_unchecked(&mut self, idx: usize) -> Self::Item {
        let end = self.v.len() - idx * self.chunk_size;
        let start = end - self.chunk_size;
        // SAFETY: 见 `RChunksMut::__iterator_get_unchecked` 和 `self.v` 的注释。
        unsafe { from_raw_parts_mut(self.v.as_mut_ptr().add(start), self.chunk_size) }
    }
}

#[stable(feature = "rchunks", since = "1.31.0")]
impl<'a, T> DoubleEndedIterator for RChunksExactMut<'a, T> {
    #[inline]
    fn next_back(&mut self) -> Option<&'a mut [T]> {
        if self.v.len() < self.chunk_size {
            None
        } else {
            // SAFETY: self.v 的契约保证任意 split_at_mut 都有效。
            let (head, tail) = unsafe { self.v.split_at_mut(self.chunk_size) };
            self.v = tail;
            // SAFETY: 没有其它东西指向、或将会指向该切片的内容。
            Some(unsafe { &mut *head })
        }
    }

    #[inline]
    fn nth_back(&mut self, n: usize) -> Option<Self::Item> {
        let len = self.len();
        if n < len {
            // 现在已知 `n` 对应一个 chunk，因此这些操作都不会下溢或溢出。
            let offset = (len - n) * self.chunk_size;
            let start = self.v.len() - offset;
            let end = start + self.chunk_size;
            // SAFETY: self.v 的契约保证任意 split_at_mut 都有效。
            let (tmp, tail) = unsafe { self.v.split_at_mut(end) };
            // SAFETY: self.v 的契约保证任意 split_at_mut 都有效。
            let (_, nth_back) = unsafe { tmp.split_at_mut(start) };
            self.v = tail;
            // SAFETY: 没有其它东西指向、或将会指向该切片的内容。
            Some(unsafe { &mut *nth_back })
        } else {
            self.v = &mut [];
            None
        }
    }
}

#[stable(feature = "rchunks", since = "1.31.0")]
impl<T> ExactSizeIterator for RChunksExactMut<'_, T> {
    fn is_empty(&self) -> bool {
        self.v.is_empty()
    }
}

#[unstable(feature = "trusted_len", issue = "37572")]
unsafe impl<T> TrustedLen for RChunksExactMut<'_, T> {}

#[stable(feature = "rchunks", since = "1.31.0")]
impl<T> FusedIterator for RChunksExactMut<'_, T> {}

#[doc(hidden)]
#[unstable(feature = "trusted_random_access", issue = "none")]
unsafe impl<'a, T> TrustedRandomAccess for RChunksExactMut<'a, T> {}

#[doc(hidden)]
#[unstable(feature = "trusted_random_access", issue = "none")]
unsafe impl<'a, T> TrustedRandomAccessNoCoerce for RChunksExactMut<'a, T> {
    const MAY_HAVE_SIDE_EFFECT: bool = false;
}

#[stable(feature = "rchunks", since = "1.31.0")]
unsafe impl<T> Send for RChunksExactMut<'_, T> where T: Send {}

#[stable(feature = "rchunks", since = "1.31.0")]
unsafe impl<T> Sync for RChunksExactMut<'_, T> where T: Sync {}

#[doc(hidden)]
#[unstable(feature = "trusted_random_access", issue = "none")]
unsafe impl<'a, T> TrustedRandomAccess for Iter<'a, T> {}

#[doc(hidden)]
#[unstable(feature = "trusted_random_access", issue = "none")]
unsafe impl<'a, T> TrustedRandomAccessNoCoerce for Iter<'a, T> {
    const MAY_HAVE_SIDE_EFFECT: bool = false;
}

#[doc(hidden)]
#[unstable(feature = "trusted_random_access", issue = "none")]
unsafe impl<'a, T> TrustedRandomAccess for IterMut<'a, T> {}

#[doc(hidden)]
#[unstable(feature = "trusted_random_access", issue = "none")]
unsafe impl<'a, T> TrustedRandomAccessNoCoerce for IterMut<'a, T> {
    const MAY_HAVE_SIDE_EFFECT: bool = false;
}

/// 按谓词分隔切片、产出（不重叠）chunk 的迭代器。
///
/// 该结构体由 [切片][slices] 上的 [`chunk_by`] 方法创建。
///
/// [`chunk_by`]: slice::chunk_by
/// [slices]: slice
#[stable(feature = "slice_group_by", since = "1.77.0")]
#[must_use = "iterators are lazy and do nothing unless consumed"]
pub struct ChunkBy<'a, T: 'a, P> {
    slice: &'a [T],
    predicate: P,
}

#[stable(feature = "slice_group_by", since = "1.77.0")]
impl<'a, T: 'a, P> ChunkBy<'a, T, P> {
    pub(super) const fn new(slice: &'a [T], predicate: P) -> Self {
        ChunkBy { slice, predicate }
    }
}

#[stable(feature = "slice_group_by", since = "1.77.0")]
impl<'a, T: 'a, P> Iterator for ChunkBy<'a, T, P>
where
    P: FnMut(&T, &T) -> bool,
{
    type Item = &'a [T];

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.slice.is_empty() {
            None
        } else {
            let mut len = 1;
            let mut iter = self.slice.windows(2);
            while let Some([l, r]) = iter.next() {
                if (self.predicate)(l, r) { len += 1 } else { break }
            }
            let (head, tail) = self.slice.split_at(len);
            self.slice = tail;
            Some(head)
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.slice.is_empty() { (0, Some(0)) } else { (1, Some(self.slice.len())) }
    }

    #[inline]
    fn last(mut self) -> Option<Self::Item> {
        self.next_back()
    }
}

#[stable(feature = "slice_group_by", since = "1.77.0")]
impl<'a, T: 'a, P> DoubleEndedIterator for ChunkBy<'a, T, P>
where
    P: FnMut(&T, &T) -> bool,
{
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.slice.is_empty() {
            None
        } else {
            let mut len = 1;
            let mut iter = self.slice.windows(2);
            while let Some([l, r]) = iter.next_back() {
                if (self.predicate)(l, r) { len += 1 } else { break }
            }
            let (head, tail) = self.slice.split_at(self.slice.len() - len);
            self.slice = head;
            Some(tail)
        }
    }
}

#[stable(feature = "slice_group_by", since = "1.77.0")]
impl<'a, T: 'a, P> FusedIterator for ChunkBy<'a, T, P> where P: FnMut(&T, &T) -> bool {}

#[stable(feature = "slice_group_by_clone", since = "1.89.0")]
impl<'a, T: 'a, P: Clone> Clone for ChunkBy<'a, T, P> {
    fn clone(&self) -> Self {
        Self { slice: self.slice, predicate: self.predicate.clone() }
    }
}

#[stable(feature = "slice_group_by", since = "1.77.0")]
impl<'a, T: 'a + fmt::Debug, P> fmt::Debug for ChunkBy<'a, T, P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ChunkBy").field("slice", &self.slice).finish()
    }
}

/// 按谓词分隔切片、产出（不重叠）可变 chunk 的迭代器。
///
/// 该结构体由 [切片][slices] 上的 [`chunk_by_mut`] 方法创建。
///
/// [`chunk_by_mut`]: slice::chunk_by_mut
/// [slices]: slice
#[stable(feature = "slice_group_by", since = "1.77.0")]
#[must_use = "iterators are lazy and do nothing unless consumed"]
pub struct ChunkByMut<'a, T: 'a, P> {
    slice: &'a mut [T],
    predicate: P,
}

#[stable(feature = "slice_group_by", since = "1.77.0")]
impl<'a, T: 'a, P> ChunkByMut<'a, T, P> {
    pub(super) const fn new(slice: &'a mut [T], predicate: P) -> Self {
        ChunkByMut { slice, predicate }
    }
}

#[stable(feature = "slice_group_by", since = "1.77.0")]
impl<'a, T: 'a, P> Iterator for ChunkByMut<'a, T, P>
where
    P: FnMut(&T, &T) -> bool,
{
    type Item = &'a mut [T];

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        if self.slice.is_empty() {
            None
        } else {
            let mut len = 1;
            let mut iter = self.slice.windows(2);
            while let Some([l, r]) = iter.next() {
                if (self.predicate)(l, r) { len += 1 } else { break }
            }
            let slice = mem::take(&mut self.slice);
            let (head, tail) = slice.split_at_mut(len);
            self.slice = tail;
            Some(head)
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        if self.slice.is_empty() { (0, Some(0)) } else { (1, Some(self.slice.len())) }
    }

    #[inline]
    fn last(mut self) -> Option<Self::Item> {
        self.next_back()
    }
}

#[stable(feature = "slice_group_by", since = "1.77.0")]
impl<'a, T: 'a, P> DoubleEndedIterator for ChunkByMut<'a, T, P>
where
    P: FnMut(&T, &T) -> bool,
{
    #[inline]
    fn next_back(&mut self) -> Option<Self::Item> {
        if self.slice.is_empty() {
            None
        } else {
            let mut len = 1;
            let mut iter = self.slice.windows(2);
            while let Some([l, r]) = iter.next_back() {
                if (self.predicate)(l, r) { len += 1 } else { break }
            }
            let slice = mem::take(&mut self.slice);
            let (head, tail) = slice.split_at_mut(slice.len() - len);
            self.slice = head;
            Some(tail)
        }
    }
}

#[stable(feature = "slice_group_by", since = "1.77.0")]
impl<'a, T: 'a, P> FusedIterator for ChunkByMut<'a, T, P> where P: FnMut(&T, &T) -> bool {}

#[stable(feature = "slice_group_by", since = "1.77.0")]
impl<'a, T: 'a + fmt::Debug, P> fmt::Debug for ChunkByMut<'a, T, P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ChunkByMut").field("slice", &self.slice).finish()
    }
}
