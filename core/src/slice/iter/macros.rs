//! 切片迭代器使用的宏。

/// 便于高效读取 `end_or_len` 字段的宏。
///
/// 如果 `T` 是 ZST，就把该字段当作 `(&mut) usize` 使用；否则当作
/// `(&mut) NonNull<T>` 使用。内部通过指向 `NonNull` 的指针读取 `end`，
/// 让后端获得合适的非空元数据，而不需要手动调用 `assume`。
macro_rules! if_zst {
    (mut $this:ident, $len:ident => $zst_body:expr, $end:ident => $other_body:expr,) => {{
        #![allow(unused_unsafe)] // 本宏有时会在 unsafe 块内部展开。

        if T::IS_ZST {
            // SAFETY: 对 ZST，指针字段存放的是没有 provenance 的长度，
            // 因此把它作为 `usize` 读取和更新是有效的。
            let $len = unsafe { &mut *(&raw mut $this.end_or_len).cast::<usize>() };
            $zst_body
        } else {
            // SAFETY: 对非 ZST，类型不变量保证该字段不可能为空。
            let $end = unsafe { &mut *(&raw mut $this.end_or_len).cast::<NonNull<T>>() };
            $other_body
        }
    }};
    ($this:ident, $len:ident => $zst_body:expr, $end:ident => $other_body:expr,) => {{
        #![allow(unused_unsafe)] // 本宏有时会在 unsafe 块内部展开。

        if T::IS_ZST {
            let $len = $this.end_or_len.addr();
            $zst_body
        } else {
            // SAFETY: 对非 ZST，类型不变量保证该字段不可能为空。
            let $end = unsafe { mem::transmute::<*const T, NonNull<T>>($this.end_or_len) };
            $other_body
        }
    }};
}

// 内联 is_empty 和 len 对性能影响很大。
macro_rules! is_empty {
    ($self: ident) => {
        if_zst!($self,
            len => len == 0,
            end => $self.ptr == end,
        )
    };
}

macro_rules! len {
    ($self: ident) => {{
        if_zst!($self,
            len => len,
            end => {
                // 为消除一些边界检查（见 `position`），这里使用 ptr_sub 而不是
                // offset_from（由 `codegen/slice-position-bounds-check` 测试覆盖）。
                // SAFETY: 类型不变量保证指针对齐，并且 `start <= end`。
                unsafe { end.offset_from_unsigned($self.ptr) }
            },
        )
    }};
}

// `Iter` 和 `IterMut` 迭代器的共享定义。
macro_rules! iterator {
    (
        struct $name:ident -> $ptr:ty,
        $elem:ty,
        $raw_mut:tt,
        {$( $mut_:tt )?},
        $into_ref:ident,
        $array_ref:ident,
        {$($extra:tt)*}
    ) => {
        impl<'a, T> $name<'a, T> {
            /// 返回最后一个元素，并把迭代器尾部向前移动 1 个元素。
            ///
            /// # 安全性(Safety）
            ///
            /// 迭代器必须非空。
            #[inline]
            unsafe fn next_back_unchecked(&mut self) -> $elem {
                // SAFETY: 调用方承诺迭代器非空，因此尾部回退仍在边界内，
                // 并且确实有一个元素可返回。
                unsafe { self.pre_dec_end(1).$into_ref() }
            }

            // 从迭代器创建切片的辅助函数。
            #[inline(always)]
            fn make_slice(&self) -> &'a [T] {
                // SAFETY: 该迭代器由一个切片创建，当前起点为 `self.ptr`，
                // 剩余长度为 `len!(self)`；这保证满足 `from_raw_parts` 的全部前置条件。
                unsafe { from_raw_parts(self.ptr.as_ptr(), len!(self)) }
            }

            // 把迭代器起点向前移动 `offset` 个元素并返回旧起点的辅助函数。
            // 这是 unsafe，因为 `offset` 不能超过 `self.len()`。
            #[inline(always)]
            unsafe fn post_inc_start(&mut self, offset: usize) -> NonNull<T> {
                let old = self.ptr;

                // SAFETY: 调用方保证 `offset` 不超过 `self.len()`，
                // 因此新指针仍位于 `self` 覆盖的范围内，并且保证非空。
                unsafe {
                    if_zst!(mut self,
                        // 直接使用 intrinsic 可避免生成 UbCheck。
                        len => *len = crate::intrinsics::unchecked_sub(*len, offset),
                        _end => self.ptr = self.ptr.add(offset),
                    );
                }
                old
            }

            // 把迭代器尾部向前移动 `offset` 个元素并返回新尾部的辅助函数。
            // 这是 unsafe，因为 `offset` 不能超过 `self.len()`。
            #[inline(always)]
            unsafe fn pre_dec_end(&mut self, offset: usize) -> NonNull<T> {
                if_zst!(mut self,
                    // SAFETY: 根据前置条件，`offset` 最多等于当前长度，
                    // 因此这个减法不会下溢。
                    len => unsafe {
                        // 直接使用 intrinsic 可避免生成 UbCheck。
                        *len = crate::intrinsics::unchecked_sub(*len, offset);
                        self.ptr
                    },
                    // SAFETY: 调用方保证 `offset` 不超过 `self.len()`，
                    // 因而不会溢出 `isize`；得到的指针仍在 `slice` 边界内，
                    // 满足 `offset` 的其它要求。
                    end => unsafe {
                        *end = end.sub(offset);
                        *end
                    },
                )
            }
        }

        #[stable(feature = "rust1", since = "1.0.0")]
        impl<T> ExactSizeIterator for $name<'_, T> {
            #[inline(always)]
            fn len(&self) -> usize {
                len!(self)
            }

            #[inline(always)]
            fn is_empty(&self) -> bool {
                is_empty!(self)
            }
        }

        #[stable(feature = "rust1", since = "1.0.0")]
        impl<'a, T> Iterator for $name<'a, T> {
            type Item = $elem;

            #[inline]
            fn next(&mut self) -> Option<$elem> {
                // 故意不使用辅助函数，因为这是库中最常被单态化的路径之一。

                let ptr = self.ptr;
                let end_or_len = self.end_or_len;
                // SAFETY: 见内部注释。（不知为何，多个块会破坏这里的内联；
                // 如果能修复这一点，请这样做。）
                unsafe {
                    if T::IS_ZST {
                        let len = end_or_len.addr();
                        if len == 0 {
                            return None;
                        }
                        // SAFETY: 刚刚检查过长度非零，因此减一不会回绕。
                        // 理想上这里可用 `checked_sub`，其内部做同样的事；但截至
                        // 2025-02，它在 MIR 中优化出的体积还不够小。
                        self.end_or_len = without_provenance_mut(len.unchecked_sub(1));
                    } else {
                        // SAFETY: 根据类型不变量，非 ZST pointee 的 `end_or_len`
                        // 字段始终非空。（这个 transmute 确保读取字段时得到
                        // `!nonnull` 元数据。）
                        if ptr == crate::intrinsics::transmute::<$ptr, NonNull<T>>(end_or_len) {
                            return None;
                        }
                        // SAFETY: 上面的检查说明迭代器非空，因此向前移动一个元素
                        // 仍在切片内部，这是有效的。
                        self.ptr = ptr.add(1);
                    }
                    // SAFETY: 现在已知迭代器非空，并且已经越过第一个元素
                    // （避免下一次给出重复的 `&mut`），因此可以返回它的引用。
                    Some({ptr}.$into_ref())
                }
            }

            fn next_chunk<const N:usize>(&mut self) -> Result<[$elem; N], crate::array::IntoIter<$elem, N>> {
                if T::IS_ZST {
                    return crate::array::iter_next_chunk(self);
                }
                let len = len!(self);
                if len >= N {
                    // SAFETY: 这里仅取得一个 `[T; N]` 数组引用，并把指针前移 N 个元素。
                    let r = unsafe { self.post_inc_start(N).cast_array().$into_ref() }
                        .$array_ref(); // 必须把 &[T; N] 转成 [&T; N]
                    Ok(r)
                } else {
                    // 不能使用 $array_ref，因为没有 &mut [MU<T>; N] -> [&mut MU<T>; N] 的 builtin。
                    // 不能使用 copy_nonoverlapping，因为 $elem 的类型是 &{mut} T，而不是 T。
                    let mut a = [const { crate::mem::MaybeUninit::<$elem>::uninit() }; N];
                    for into in (&mut a).into_iter().take(len) {
                        // SAFETY: take(n) 把写入限制在剩余元素内（用切片会生成更差的代码）。
                        into.write(unsafe { self.post_inc_start(1).$into_ref() });
                    }
                    // SAFETY: 刚刚已经初始化元素 0..len。
                    unsafe { Err(crate::array::IntoIter::new_unchecked(a, 0..len)) }
                }
            }

            #[inline]
            fn size_hint(&self) -> (usize, Option<usize>) {
                let exact = len!(self);
                (exact, Some(exact))
            }

            #[inline]
            fn count(self) -> usize {
                len!(self)
            }

            #[inline]
            fn nth(&mut self, n: usize) -> Option<$elem> {
                if n >= len!(self) {
                    // 该迭代器现在为空。
                    if_zst!(mut self,
                        len => *len = 0,
                        end => self.ptr = *end,
                    );
                    return None;
                }
                // SAFETY: 这里在边界内；`post_inc_start` 对 ZST 也会做正确的事。
                unsafe {
                    self.post_inc_start(n);
                    Some(self.next_unchecked())
                }
            }

            #[inline]
            fn advance_by(&mut self, n: usize) -> Result<(), NonZero<usize>> {
                let advance = cmp::min(len!(self), n);
                // SAFETY: 根据构造，`advance` 不超过 `self.len()`。
                unsafe { self.post_inc_start(advance) };
                NonZero::new(n - advance).map_or(Ok(()), Err)
            }

            #[inline]
            fn last(mut self) -> Option<$elem> {
                self.next_back()
            }

            #[inline]
            fn fold<B, F>(self, init: B, mut f: F) -> B
                where
                    F: FnMut(B, Self::Item) -> B,
            {
                // 与默认实现相比，这个实现包含以下优化：
                // - 使用 do-while 形状的循环，这是 LLVM 更偏好的循环形态，
                //   见 https://releases.llvm.org/16.0.0/docs/LoopTerminology.html#more-canonical-loops
                // - 递增索引而不是指针，因为后者会阻碍一些优化，见 #111603
                // - 避免 Option 包装与匹配。
                if is_empty!(self) {
                    return init;
                }
                let mut acc = init;
                let mut i = 0;
                let len = len!(self);
                loop {
                    // SAFETY: 循环迭代的是 `i in 0..len`，始终位于切片 allocation 边界内。
                    acc = f(acc, unsafe { & $( $mut_ )? *self.ptr.add(i).as_ptr() });
                    // SAFETY: `i` 不会溢出；只有切片本身达到 usize::MAX 长度时
                    // 它才会达到该值，而那种情况下递增后会立即跳出循环。
                    i = unsafe { i.unchecked_add(1) };
                    if i == len {
                        break;
                    }
                }
                acc
            }

            // 覆盖使用 `try_fold` 的默认实现，因为这个简单实现生成更少 LLVM IR，
            // 编译也更快。
            #[inline]
            fn for_each<F>(mut self, mut f: F)
            where
                Self: Sized,
                F: FnMut(Self::Item),
            {
                while let Some(x) = self.next() {
                    f(x);
                }
            }

            // 覆盖使用 `try_fold` 的默认实现，因为这个简单实现生成更少 LLVM IR，
            // 编译也更快。
            #[inline]
            fn all<F>(&mut self, mut f: F) -> bool
            where
                Self: Sized,
                F: FnMut(Self::Item) -> bool,
            {
                while let Some(x) = self.next() {
                    if !f(x) {
                        return false;
                    }
                }
                true
            }

            // 覆盖使用 `try_fold` 的默认实现，因为这个简单实现生成更少 LLVM IR，
            // 编译也更快。
            #[inline]
            fn any<F>(&mut self, mut f: F) -> bool
            where
                Self: Sized,
                F: FnMut(Self::Item) -> bool,
            {
                while let Some(x) = self.next() {
                    if f(x) {
                        return true;
                    }
                }
                false
            }

            // 覆盖使用 `try_fold` 的默认实现，因为这个简单实现生成更少 LLVM IR，
            // 编译也更快。
            #[inline]
            fn find<P>(&mut self, mut predicate: P) -> Option<Self::Item>
            where
                Self: Sized,
                P: FnMut(&Self::Item) -> bool,
            {
                while let Some(x) = self.next() {
                    if predicate(&x) {
                        return Some(x);
                    }
                }
                None
            }

            // 覆盖使用 `try_fold` 的默认实现，因为这个简单实现生成更少 LLVM IR，
            // 编译也更快。
            #[inline]
            fn find_map<B, F>(&mut self, mut f: F) -> Option<B>
            where
                Self: Sized,
                F: FnMut(Self::Item) -> Option<B>,
            {
                while let Some(x) = self.next() {
                    if let Some(y) = f(x) {
                        return Some(y);
                    }
                }
                None
            }

            // 覆盖使用 `try_fold` 的默认实现，因为这个简单实现生成更少 LLVM IR，
            // 编译也更快。同时，`assume` 可避免一次边界检查。
            #[inline]
            fn position<P>(&mut self, mut predicate: P) -> Option<usize> where
                Self: Sized,
                P: FnMut(Self::Item) -> bool,
            {
                let n = len!(self);
                let mut i = 0;
                while let Some(x) = self.next() {
                    if predicate(x) {
                        // SAFETY: 循环不变量保证这里在边界内：
                        // 当 `i >= n` 时，`self.next()` 返回 `None`，循环结束。
                        unsafe { assert_unchecked(i < n) };
                        return Some(i);
                    }
                    i += 1;
                }
                None
            }

            // 覆盖使用 `try_fold` 的默认实现，因为这个简单实现生成更少 LLVM IR，
            // 编译也更快。同时，`assume` 可避免一次边界检查。
            #[inline]
            fn rposition<P>(&mut self, mut predicate: P) -> Option<usize> where
                P: FnMut(Self::Item) -> bool,
                Self: Sized + ExactSizeIterator + DoubleEndedIterator
            {
                let n = len!(self);
                let mut i = n;
                while let Some(x) = self.next_back() {
                    i -= 1;
                    if predicate(x) {
                        // SAFETY: `i` 从 `n` 开始并且只会递减，因此必定小于 `n`。
                        unsafe { assert_unchecked(i < n) };
                        return Some(i);
                    }
                }
                None
            }

            #[inline]
            unsafe fn __iterator_get_unchecked(&mut self, idx: usize) -> Self::Item {
                // SAFETY: 调用方必须保证 `i` 位于底层切片边界内，因此 `i`
                // 不可能溢出 `isize`，返回的引用也保证指向切片中的一个元素，
                // 因而是有效的。
                //
                // 还要注意，调用方也保证不会以同一索引再次调用本方法，
                // 并且不会调用其它会访问该子切片的方法；因此在 `IterMut`
                // 情况下，返回可变引用也是有效的。
                unsafe { & $( $mut_ )? * self.ptr.as_ptr().add(idx) }
            }

            $($extra)*
        }

        #[stable(feature = "rust1", since = "1.0.0")]
        impl<'a, T> DoubleEndedIterator for $name<'a, T> {
            #[inline]
            fn next_back(&mut self) -> Option<$elem> {
                // 也可以用切片实现，但这样可避免边界检查。

                // SAFETY: 调用 `next_back_unchecked` 是安全的，
                // 因为会先检查迭代器是否为空。
                unsafe {
                    if is_empty!(self) {
                        None
                    } else {
                        Some(self.next_back_unchecked())
                    }
                }
            }

            #[inline]
            fn nth_back(&mut self, n: usize) -> Option<$elem> {
                if n >= len!(self) {
                    // 该迭代器现在为空。
                    if_zst!(mut self,
                        len => *len = 0,
                        end => *end = self.ptr,
                    );
                    return None;
                }
                // SAFETY: 这里在边界内；`pre_dec_end` 对 ZST 也会做正确的事。
                unsafe {
                    self.pre_dec_end(n);
                    Some(self.next_back_unchecked())
                }
            }

            #[inline]
            fn advance_back_by(&mut self, n: usize) -> Result<(), NonZero<usize>> {
                let advance = cmp::min(len!(self), n);
                // SAFETY: 根据构造，`advance` 不超过 `self.len()`。
                unsafe { self.pre_dec_end(advance) };
                NonZero::new(n - advance).map_or(Ok(()), Err)
            }
        }

        #[stable(feature = "fused", since = "1.26.0")]
        impl<T> FusedIterator for $name<'_, T> {}

        #[unstable(feature = "trusted_len", issue = "37572")]
        unsafe impl<T> TrustedLen for $name<'_, T> {}

        impl<'a, T> UncheckedIterator for $name<'a, T> {
            #[inline]
            unsafe fn next_unchecked(&mut self) -> $elem {
                // SAFETY: 调用方承诺至少还有一个元素。
                unsafe {
                    self.post_inc_start(1).$into_ref()
                }
            }
        }

        #[stable(feature = "default_iters", since = "1.70.0")]
        impl<T> Default for $name<'_, T> {
            /// 创建一个空切片迭代器。
            ///
            /// ```
            #[doc = concat!("# use core::slice::", stringify!($name), ";")]
            #[doc = concat!("let iter: ", stringify!($name<'_, u8>), " = Default::default();")]
            /// assert_eq!(iter.len(), 0);
            /// ```
            fn default() -> Self {
                (& $( $mut_ )? []).into_iter()
            }
        }
    }
}

macro_rules! forward_iterator {
    ($name:ident: $elem:ident, $iter_of:ty) => {
        #[stable(feature = "rust1", since = "1.0.0")]
        impl<'a, $elem, P> Iterator for $name<'a, $elem, P>
        where
            P: FnMut(&T) -> bool,
        {
            type Item = $iter_of;

            #[inline]
            fn next(&mut self) -> Option<$iter_of> {
                self.inner.next()
            }

            #[inline]
            fn size_hint(&self) -> (usize, Option<usize>) {
                self.inner.size_hint()
            }
        }

        #[stable(feature = "fused", since = "1.26.0")]
        impl<'a, $elem, P> FusedIterator for $name<'a, $elem, P> where P: FnMut(&T) -> bool {}
    };
}
