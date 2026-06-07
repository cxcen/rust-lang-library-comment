use crate::iter::FusedIterator;
use crate::mem::MaybeUninit;
use crate::{fmt, ptr};

/// 在另一个迭代器的窗口上执行映射后得到的迭代器。
///
/// 该 `struct` 由 [`Iterator::map_windows`] 创建。更多公开语义见该方法文档。
#[must_use = "iterators are lazy and do nothing unless consumed"]
#[unstable(feature = "iter_map_windows", issue = "87155")]
pub struct MapWindows<I: Iterator, F, const N: usize> {
    f: F,
    inner: MapWindowsInner<I, N>,
}

struct MapWindowsInner<I: Iterator, const N: usize> {
    // 这里会 fuse 内层迭代器，因为滑动窗口中不应出现“空洞”。一旦底层迭代器返回
    // `None`，`MapWindows` 就会永久返回 `None`。
    iter: Option<I>,
    // 迭代器默认是惰性的: 只有调用 `Iterator::next()` 时才产出元素。
    // `MapWindows` 也遵守这一点。
    //
    // 第一次迭代前，缓冲区保持为 `None`。用户第一次调用 `next` 或其他会推进迭代器
    // 的方法时，我们才从内层迭代器收集前 `N` 项并放入缓冲区。
    //
    // 当内层迭代器已经返回 `None`(即被 fused)时，我们取走这个 `buffer` 并保持为
    // `None`，以回收其资源。
    //
    // FIXME: 是否应该利用 niche 优化缩小 `buffer` 的大小?
    buffer: Option<Buffer<I::Item, N>>,
}

// `Buffer` 使用两倍空间来减少迭代过程中的元素移动。
// `Buffer<T, N>` 在语义上是 `[MaybeUninit<T>; 2 * N]`。但受 const generics 限制，
// 这里使用了不同类型；它具有相同的底层内存布局。
struct Buffer<T, const N: usize> {
    // 不变量: `self.buffer[self.start..self.start + N]` 已初始化，
    // 其他元素未初始化。这也推出 `self.start <= N`。
    buffer: [[MaybeUninit<T>; N]; 2],
    start: usize,
}

impl<I: Iterator, F, const N: usize> MapWindows<I, F, N> {
    pub(in crate::iter) fn new(iter: I, f: F) -> Self {
        assert!(N != 0, "array in `Iterator::map_windows` must contain more than 0 elements");

        // 只有 ZST 数组的长度才可能大到这里。
        if size_of::<I::Item>() == 0 {
            assert!(
                N.checked_mul(2).is_some(),
                "array size of `Iterator::map_windows` is too large"
            );
        }

        Self { inner: MapWindowsInner::new(iter), f }
    }
}

impl<I: Iterator, const N: usize> MapWindowsInner<I, N> {
    #[inline]
    fn new(iter: I) -> Self {
        Self { iter: Some(iter), buffer: None }
    }

    fn next_window(&mut self) -> Option<&[I::Item; N]> {
        let iter = self.iter.as_mut()?;
        match self.buffer {
            // 第一次推进。收集 `self.iter` 的前 `N` 项来初始化 `self.buffer`。
            None => self.buffer = Buffer::try_from_iter(iter),
            Some(ref mut buffer) => match iter.next() {
                None => {
                    // 内层迭代器已经产出 `None`，因此把它 fuse。
                    self.iter.take();
                    self.buffer.take();
                }
                // 推进迭代器。先调用 `next`，再修改缓冲区。这样如果 `next` panic，
                // 我们的不变量仍然成立，`Drop` 实现也会 drop 正确的元素。
                Some(item) => buffer.push(item),
            },
        }
        self.buffer.as_ref().map(Buffer::as_array_ref)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let Some(ref iter) = self.iter else { return (0, Some(0)) };
        let (lo, hi) = iter.size_hint();
        if self.buffer.is_some() {
            // 如果内层迭代器已经产出了前 `N` 项，则 size hint 等于内层迭代器的
            // size hint。
            (lo, hi)
        } else {
            // 如果内层迭代器尚未产出前 `N` 项，则前 `N` 个元素应计为一个窗口，
            // 因此两个边界都应减去 `N - 1`。
            (lo.saturating_sub(N - 1), hi.map(|hi| hi.saturating_sub(N - 1)))
        }
    }
}

impl<T, const N: usize> Buffer<T, N> {
    fn try_from_iter(iter: &mut impl Iterator<Item = T>) -> Option<Self> {
        let first_half = crate::array::iter_next_chunk(iter).ok()?;
        let buffer =
            [MaybeUninit::new(first_half).transpose(), [const { MaybeUninit::uninit() }; N]];
        Some(Self { buffer, start: 0 })
    }

    #[inline]
    fn buffer_ptr(&self) -> *const MaybeUninit<T> {
        self.buffer.as_ptr().cast()
    }

    #[inline]
    fn buffer_mut_ptr(&mut self) -> *mut MaybeUninit<T> {
        self.buffer.as_mut_ptr().cast()
    }

    #[inline]
    fn as_array_ref(&self) -> &[T; N] {
        debug_assert!(self.start + N <= 2 * N);

        // SAFETY: 不变量保证这些元素已经初始化。
        unsafe { &*self.buffer_ptr().add(self.start).cast() }
    }

    #[inline]
    fn as_uninit_array_mut(&mut self) -> &mut MaybeUninit<[T; N]> {
        debug_assert!(self.start + N <= 2 * N);

        // SAFETY: 不变量保证这些元素在边界内。
        unsafe { &mut *self.buffer_mut_ptr().add(self.start).cast() }
    }

    /// 把新项 `next` 推入后端，并弹出最前端的一项。
    ///
    /// 当推入位置到达后端时，所有元素都会被移动到前端。
    fn push(&mut self, next: T) {
        let buffer_mut_ptr = self.buffer_mut_ptr();
        debug_assert!(self.start + N <= 2 * N);

        let to_drop = if self.start == N {
            // 已经到达缓冲区末端，必须把所有内容复制回开头。N = 3 时布局如下。
            //
            //    0   1   2   3   4   5            0   1   2   3   4   5
            //  ┌───┬───┬───┬───┬───┬───┐        ┌───┬───┬───┬───┬───┬───┐
            //  │ - │ - │ - │ a │ b │ c │   ->   │ b │ c │ n │ - │ - │ - │
            //  └───┴───┴───┴───┴───┴───┘        └───┴───┴───┴───┴───┴───┘
            //                ↑                    ↑
            //              start                start

            // SAFETY: 两个指针对 N - 1 个元素的读写有效，因为数组语义大小是 2 * N。
            // 两个区域也因此不重叠。
            //
            // 旧元素留在原位。一旦 `start` 设为 0，就把它们视为未初始化，并把副本视为
            // 已初始化。
            let to_drop = unsafe {
                ptr::copy_nonoverlapping(buffer_mut_ptr.add(self.start + 1), buffer_mut_ptr, N - 1);
                (*buffer_mut_ptr.add(N - 1)).write(next);
                buffer_mut_ptr.add(self.start)
            };
            self.start = 0;
            to_drop
        } else {
            // SAFETY: 不变量加上上面的检查保证 `self.start < N`。
            // 即使末尾的 drop panic，不变量也仍然成立。
            //
            // N = 3 时的布局:
            //
            //    0   1   2   3   4   5            0   1   2   3   4   5
            //  ┌───┬───┬───┬───┬───┬───┐        ┌───┬───┬───┬───┬───┬───┐
            //  │ - │ a │ b │ c │ - │ - │   ->   │ - │ - │ b │ c │ n │ - │
            //  └───┴───┴───┴───┴───┴───┘        └───┴───┴───┴───┴───┴───┘
            //        ↑                                    ↑
            //      start                                start
            //
            let to_drop = unsafe {
                (*buffer_mut_ptr.add(self.start + N)).write(next);
                buffer_mut_ptr.add(self.start)
            };
            self.start += 1;
            to_drop
        };

        // SAFETY: 该索引有效，并且这是上图中的元素 `a`，尚未被 drop。
        unsafe { ptr::drop_in_place(to_drop.cast_init()) };
    }
}

impl<T: Clone, const N: usize> Clone for Buffer<T, N> {
    fn clone(&self) -> Self {
        let mut buffer = Buffer {
            buffer: [[const { MaybeUninit::uninit() }; N], [const { MaybeUninit::uninit() }; N]],
            start: self.start,
        };
        buffer.as_uninit_array_mut().write(self.as_array_ref().clone());
        buffer
    }
}

impl<I, const N: usize> Clone for MapWindowsInner<I, N>
where
    I: Iterator + Clone,
    I::Item: Clone,
{
    fn clone(&self) -> Self {
        Self { iter: self.iter.clone(), buffer: self.buffer.clone() }
    }
}

impl<T, const N: usize> Drop for Buffer<T, N> {
    fn drop(&mut self) {
        // SAFETY: 不变量保证从 `self.start` 开始的 N 个元素已经初始化。这里 drop 它们。
        unsafe {
            let initialized_part: *mut [T] = crate::ptr::slice_from_raw_parts_mut(
                self.buffer_mut_ptr().add(self.start).cast(),
                N,
            );
            ptr::drop_in_place(initialized_part);
        }
    }
}

#[unstable(feature = "iter_map_windows", issue = "87155")]
impl<I, F, R, const N: usize> Iterator for MapWindows<I, F, N>
where
    I: Iterator,
    F: FnMut(&[I::Item; N]) -> R,
{
    type Item = R;

    fn next(&mut self) -> Option<Self::Item> {
        let window = self.inner.next_window()?;
        let out = (self.f)(window);
        Some(out)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

// 注意，即使内层迭代器不是 fused，`MapWindows` 仍然是 fused，
// 因为映射窗口中不允许出现“空洞”。
#[unstable(feature = "iter_map_windows", issue = "87155")]
impl<I, F, R, const N: usize> FusedIterator for MapWindows<I, F, N>
where
    I: Iterator,
    F: FnMut(&[I::Item; N]) -> R,
{
}

#[unstable(feature = "iter_map_windows", issue = "87155")]
impl<I, F, R, const N: usize> ExactSizeIterator for MapWindows<I, F, N>
where
    I: ExactSizeIterator,
    F: FnMut(&[I::Item; N]) -> R,
{
}

#[unstable(feature = "iter_map_windows", issue = "87155")]
impl<I: Iterator + fmt::Debug, F, const N: usize> fmt::Debug for MapWindows<I, F, N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MapWindows").field("iter", &self.inner.iter).finish()
    }
}

#[unstable(feature = "iter_map_windows", issue = "87155")]
impl<I, F, const N: usize> Clone for MapWindows<I, F, N>
where
    I: Iterator + Clone,
    F: Clone,
    I::Item: Clone,
{
    fn clone(&self) -> Self {
        Self { f: self.f.clone(), inner: self.inner.clone() }
    }
}
