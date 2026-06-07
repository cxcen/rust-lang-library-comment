use super::*;
use crate::cmp::Ordering::{Equal, Greater, Less};
use crate::intrinsics::const_eval_select;
use crate::mem::{self, SizedTypeProperties};
use crate::slice::{self, SliceIndex};

impl<T: PointeeSized> *const T {
    #[doc = include_str!("docs/is_null.md")]
    ///
    /// # 示例
    ///
    /// ```
    /// let s: &str = "Follow the rabbit";
    /// let ptr: *const u8 = s.as_ptr();
    /// assert!(!ptr.is_null());
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_ptr_is_null", since = "1.84.0")]
    #[rustc_diagnostic_item = "ptr_const_is_null"]
    #[inline]
    #[rustc_allow_const_fn_unstable(const_eval_select)]
    pub const fn is_null(self) -> bool {
        // 先转换为 thin 指针再比较，这样对于 fat 指针只考虑其 "data" 部分来判断是否为 null。
        let ptr = self as *const u8;
        const_eval_select!(
            @capture { ptr: *const u8 } -> bool:
            // 这里对 `const_raw_ptr_comparison` 的使用已经被 t-lang 团队明确批准。
            if const #[rustc_allow_const_fn_unstable(const_raw_ptr_comparison)] {
                match (ptr).guaranteed_eq(null_mut()) {
                    Some(res) => res,
                    // 为了保持最大程度的保守，当我们无法确定指针是否为 null 时就停止执行。
                    // 这里 *不能* 返回 `false`，否则会让 `NonNull::new` 变得不健全（unsound）！
                    None => panic!("null-ness of this pointer cannot be determined in const context"),
                }
            } else {
                ptr.addr() == 0
            }
        )
    }

    /// 转换为指向另一类型的指针。
    #[stable(feature = "ptr_cast", since = "1.38.0")]
    #[rustc_const_stable(feature = "const_ptr_cast", since = "1.38.0")]
    #[rustc_diagnostic_item = "const_ptr_cast"]
    #[inline(always)]
    pub const fn cast<U>(self) -> *const U {
        self as _
    }

    /// 通过检查对齐来尝试转换为指向另一类型的指针。
    ///
    /// 如果该指针对目标类型来说是正确对齐的，就会被转换为目标类型；否则返回 `None`。
    ///
    /// # 示例
    ///
    /// ```rust
    /// #![feature(pointer_try_cast_aligned)]
    ///
    /// let x = 0u64;
    ///
    /// let aligned: *const u64 = &x;
    /// let unaligned = unsafe { aligned.byte_add(1) };
    ///
    /// assert!(aligned.try_cast_aligned::<u32>().is_some());
    /// assert!(unaligned.try_cast_aligned::<u32>().is_none());
    /// ```
    #[unstable(feature = "pointer_try_cast_aligned", issue = "141221")]
    #[must_use = "this returns the result of the operation, \
                  without modifying the original"]
    #[inline]
    pub fn try_cast_aligned<U>(self) -> Option<*const U> {
        if self.is_aligned_to(align_of::<U>()) { Some(self.cast()) } else { None }
    }

    /// 在一个指向另一类型的新指针中复用本指针的地址值。
    ///
    /// 该操作会忽略其 `meta` 操作数的地址部分，并丢弃 `self` 现有的 metadata。对于指向 sized
    /// 类型的指针（thin 指针），其效果与一次简单的 cast 相同。对于指向 unsized 类型的指针（fat
    /// 指针），它会把地址与新的 metadata（例如切片长度或 `dyn`-vtable）重新组合起来。
    ///
    /// 结果指针将带有 `self` 的 provenance。该操作在语义上等同于创建一个新指针，其 data 指针值取
    /// 自 `self`，而 metadata 取自 `meta`；其为 fat 或 thin 取决于 `meta` 操作数。
    ///
    /// # 示例
    ///
    /// 本函数主要用于在可能是 fat 的指针上进行指针算术运算。先把指针转换为指向 sized 的 pointee
    /// 以便使用 offset 系列操作，然后再与它自己原本的 metadata 重新组合。
    ///
    /// ```
    /// #![feature(set_ptr_value)]
    /// # use core::fmt::Debug;
    /// let arr: [i32; 3] = [1, 2, 3];
    /// let mut ptr = arr.as_ptr() as *const dyn Debug;
    /// let thin = ptr as *const u8;
    /// unsafe {
    ///     ptr = thin.add(8).with_metadata_of(ptr);
    ///     # assert_eq!(*(ptr as *const i32), 3);
    ///     println!("{:?}", &*ptr); // 会打印 "3"
    /// }
    /// ```
    ///
    /// # *错误* 用法
    ///
    /// 来自各指针的 provenance *不会* 被合并。结果指针只能用于引用 `self` 所允许访问的地址。
    ///
    /// ```rust,no_run
    /// #![feature(set_ptr_value)]
    /// let x = 0u32;
    /// let y = 1u32;
    ///
    /// let x = (&x) as *const u32;
    /// let y = (&y) as *const u32;
    ///
    /// let offset = (x as usize - y as usize) / 4;
    /// let bad = x.wrapping_add(offset).with_metadata_of(y);
    ///
    /// // 这次解引用是 UB。该指针只拥有针对 `x` 的 provenance，却指向了 `y`。
    /// println!("{:?}", unsafe { &*bad });
    /// ```
    #[unstable(feature = "set_ptr_value", issue = "75091")]
    #[must_use = "returns a new pointer rather than modifying its argument"]
    #[inline]
    pub const fn with_metadata_of<U>(self, meta: *const U) -> *const U
    where
        U: PointeeSized,
    {
        from_raw_parts::<U>(self as *const (), metadata(meta))
    }

    /// 在不改变类型的前提下改变 constness（可变性限定）。
    ///
    /// 这比 `as` 稍微安全一些，因为如果代码被重构，它不会悄无声息地改变类型。
    #[stable(feature = "ptr_const_cast", since = "1.65.0")]
    #[rustc_const_stable(feature = "ptr_const_cast", since = "1.65.0")]
    #[rustc_diagnostic_item = "ptr_cast_mut"]
    #[inline(always)]
    pub const fn cast_mut(self) -> *mut T {
        self as _
    }

    #[doc = include_str!("./docs/addr.md")]
    #[must_use]
    #[inline(always)]
    #[stable(feature = "strict_provenance", since = "1.84.0")]
    pub fn addr(self) -> usize {
        // 指针到整数的 transmute 目前恰好具有正确的语义：它返回地址而不暴露 provenance。注意这
        // *不是* 关于 transmute 语义的稳定保证，它依赖于 sysroot 内的 crate 拥有特殊地位。
        // SAFETY: 指针到整数的 transmute 是有效的（前提是你能接受丢失 provenance）。
        unsafe { mem::transmute(self.cast::<()>()) }
    }

    /// 暴露指针的 ["provenance"][crate::ptr#provenance] 部分以供日后在
    /// [`with_exposed_provenance`] 中使用，并返回其 "address"（地址）部分。
    ///
    /// 这等价于 `self as usize`，在语义上会丢弃 provenance 信息。此外，它（与 `as` cast 一样）带
    /// 有一个隐式副作用：将该 provenance 标记为“已暴露（exposed）”，因此在支持的平台上，你之后
    /// 可以调用 [`with_exposed_provenance`] 来重建出包含其 provenance 的原始指针。
    ///
    /// 由于其固有的歧义性，[`with_exposed_provenance`] 可能不被那些帮助你遵循 Rust 内存模型的工
    /// 具所支持。建议尽可能使用 [Strict Provenance][crate::ptr#strict-provenance] API，例如
    /// [`with_addr`][pointer::with_addr]，在这种情况下应当使用 [`addr`][pointer::addr] 而非
    /// `expose_provenance`。
    ///
    /// 在大多数平台上，这会产生一个与原始指针字节相同的值，因为所有字节都用于描述地址。需要在指
    /// 针中存放额外信息的平台可能不支持该操作，因为 [`with_exposed_provenance`] 正常工作所需的
    /// 'expose' 副作用通常无法提供。
    ///
    /// 这是一个 [Exposed Provenance][crate::ptr#exposed-provenance] API。
    ///
    /// [`with_exposed_provenance`]: with_exposed_provenance
    #[inline(always)]
    #[stable(feature = "exposed_provenance", since = "1.84.0")]
    pub fn expose_provenance(self) -> usize {
        self.cast::<()>() as usize
    }

    /// 创建一个新指针，其地址为给定值，而 [provenance][crate::ptr#provenance] 取自 `self`。
    ///
    /// 这类似于 `addr as *const T` cast，但会把 `self` 的 *provenance* 复制到新指针上。这避免了一
    /// 元 cast 所固有的歧义。
    ///
    /// 它等价于使用 [`wrapping_offset`][pointer::wrapping_offset] 把 `self` 偏移到给定地址，因此
    /// 拥有与之完全相同的能力与限制。
    ///
    /// 这是一个 [Strict Provenance][crate::ptr#strict-provenance] API。
    #[must_use]
    #[inline]
    #[stable(feature = "strict_provenance", since = "1.84.0")]
    pub fn with_addr(self, addr: usize) -> Self {
        // 这本应是一个 intrinsic，以避免做任何算术运算；但在此之前，我们可以用 `wrapping_offset`
        // 来实现它，它会保留指针的 provenance。
        let self_addr = self.addr() as isize;
        let dest_addr = addr as isize;
        let offset = dest_addr.wrapping_sub(self_addr);
        self.wrapping_byte_offset(offset)
    }

    /// 通过把 `self` 的地址映射为一个新地址来创建一个新指针，同时保留 `self` 的
    /// [provenance][crate::ptr#provenance]。
    ///
    /// 这是 [`with_addr`][pointer::with_addr] 的便捷包装，详见该方法。
    ///
    /// 这是一个 [Strict Provenance][crate::ptr#strict-provenance] API。
    #[must_use]
    #[inline]
    #[stable(feature = "strict_provenance", since = "1.84.0")]
    pub fn map_addr(self, f: impl FnOnce(usize) -> usize) -> Self {
        self.with_addr(f(self.addr()))
    }

    /// 把一个（可能是宽指针的）指针分解为它的 data 指针与 metadata 两个部分。
    ///
    /// 之后可以用 [`from_raw_parts`] 重新构造出该指针。
    #[unstable(feature = "ptr_metadata", issue = "81513")]
    #[inline]
    pub const fn to_raw_parts(self) -> (*const (), <T as super::Pointee>::Metadata) {
        (self.cast(), metadata(self))
    }

    #[doc = include_str!("./docs/as_ref.md")]
    ///
    /// ```
    /// let ptr: *const u8 = &10u8 as *const u8;
    ///
    /// unsafe {
    ///     let val_back = &*ptr;
    ///     assert_eq!(val_back, &10);
    /// }
    /// ```
    ///
    /// # 示例
    ///
    /// ```
    /// let ptr: *const u8 = &10u8 as *const u8;
    ///
    /// unsafe {
    ///     if let Some(val_back) = ptr.as_ref() {
    ///         assert_eq!(val_back, &10);
    ///     }
    /// }
    /// ```
    ///
    ///
    /// [`is_null`]: #method.is_null
    /// [`as_uninit_ref`]: #method.as_uninit_ref
    #[stable(feature = "ptr_as_ref", since = "1.9.0")]
    #[rustc_const_stable(feature = "const_ptr_is_null", since = "1.84.0")]
    #[inline]
    pub const unsafe fn as_ref<'a>(self) -> Option<&'a T> {
        // SAFETY: 调用方必须保证：当 `self` 不为 null 时，它对于一个引用而言是有效的。
        if self.is_null() { None } else { unsafe { Some(&*self) } }
    }

    /// 返回指针所指向值的共享引用。
    /// 如果指针可能为 null，或所指向的值可能未初始化，则必须改用 [`as_uninit_ref`]。
    /// 如果指针可能为 null，但所指向的值已知已被初始化，则必须改用 [`as_ref`]。
    ///
    /// [`as_ref`]: #method.as_ref
    /// [`as_uninit_ref`]: #method.as_uninit_ref
    ///
    /// # 安全性(Safety）
    ///
    /// 调用该方法时，你必须确保该指针是
    /// [可转换为引用](crate::ptr#pointer-to-reference-conversion)的。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(ptr_as_ref_unchecked)]
    /// let ptr: *const u8 = &10u8 as *const u8;
    ///
    /// unsafe {
    ///     assert_eq!(ptr.as_ref_unchecked(), &10);
    /// }
    /// ```
    // FIXME: 稳定后，在 `as_ref` 和 `as_uninit_ref` 的文档中提到这一点。
    #[unstable(feature = "ptr_as_ref_unchecked", issue = "122034")]
    #[inline]
    #[must_use]
    pub const unsafe fn as_ref_unchecked<'a>(self) -> &'a T {
        // SAFETY: 调用方必须保证 `self` 对于一个引用而言是有效的。
        unsafe { &*self }
    }

    #[doc = include_str!("./docs/as_uninit_ref.md")]
    ///
    /// [`is_null`]: #method.is_null
    /// [`as_ref`]: #method.as_ref
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(ptr_as_uninit)]
    ///
    /// let ptr: *const u8 = &10u8 as *const u8;
    ///
    /// unsafe {
    ///     if let Some(val_back) = ptr.as_uninit_ref() {
    ///         assert_eq!(val_back.assume_init(), 10);
    ///     }
    /// }
    /// ```
    #[inline]
    #[unstable(feature = "ptr_as_uninit", issue = "75402")]
    pub const unsafe fn as_uninit_ref<'a>(self) -> Option<&'a MaybeUninit<T>>
    where
        T: Sized,
    {
        // SAFETY: 调用方必须保证 `self` 满足引用所要求的全部条件。
        if self.is_null() { None } else { Some(unsafe { &*(self as *const MaybeUninit<T>) }) }
    }

    #[doc = include_str!("./docs/offset.md")]
    ///
    /// # 示例
    ///
    /// ```
    /// let s: &str = "123";
    /// let ptr: *const u8 = s.as_ptr();
    ///
    /// unsafe {
    ///     assert_eq!(*ptr.offset(1) as char, '2');
    ///     assert_eq!(*ptr.offset(2) as char, '3');
    /// }
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[must_use = "returns a new pointer rather than modifying its argument"]
    #[rustc_const_stable(feature = "const_ptr_offset", since = "1.61.0")]
    #[inline(always)]
    #[track_caller]
    pub const unsafe fn offset(self, count: isize) -> *const T
    where
        T: Sized,
    {
        #[inline]
        #[rustc_allow_const_fn_unstable(const_eval_select)]
        const fn runtime_offset_nowrap(this: *const (), count: isize, size: usize) -> bool {
            // 这里可以使用 const_eval_select，因为它仅用于 UB 检查。
            const_eval_select!(
                @capture { this: *const (), count: isize, size: usize } -> bool:
                if const {
                    true
                } else {
                    // `size` 是某个 Rust 类型的大小，因此我们知道 `size <= isize::MAX`，于是这里
                    // 的 `as` cast 不会丢失信息。
                    let Some(byte_offset) = count.checked_mul(size as isize) else {
                        return false;
                    };
                    let (_, overflow) = this.addr().overflowing_add_signed(byte_offset);
                    !overflow
                }
            )
        }

        ub_checks::assert_unsafe_precondition!(
            check_language_ub,
            "ptr::offset requires the address calculation to not overflow",
            (
                this: *const () = self as *const (),
                count: isize = count,
                size: usize = size_of::<T>(),
            ) => runtime_offset_nowrap(this, count, size)
        );

        // SAFETY: 调用方必须维护 `offset` 的安全契约。
        unsafe { intrinsics::offset(self, count) }
    }

    /// 给指针加上一个以字节为单位的有符号偏移量。
    ///
    /// `count` 的单位是 **字节**。
    ///
    /// 这纯粹是一个便捷封装，相当于先转换为 `u8` 指针，再在其上调用 [offset][pointer::offset]。
    /// 文档与安全要求请参见该方法。
    ///
    /// 对于非 `Sized` 的 pointee，该操作只改变 data 指针，而保持 metadata 不变。
    #[must_use]
    #[inline(always)]
    #[stable(feature = "pointer_byte_offsets", since = "1.75.0")]
    #[rustc_const_stable(feature = "const_pointer_byte_offsets", since = "1.75.0")]
    #[track_caller]
    pub const unsafe fn byte_offset(self, count: isize) -> Self {
        // SAFETY: 调用方必须维护 `offset` 的安全契约。
        unsafe { self.cast::<u8>().offset(count).with_metadata_of(self) }
    }

    /// 使用回绕（wrapping）算术给指针加上一个有符号偏移量。
    ///
    /// `count` 的单位是 T；例如 `count` 为 3 表示偏移 `3 * size_of::<T>()` 个字节。
    ///
    /// # 安全性(Safety）
    ///
    /// 该操作本身始终是安全的，但使用其结果指针则不一定安全。
    ///
    /// 结果指针会“记住” `self` 所指向的那个 [allocation]（这被称为
    /// "[Provenance](ptr/index.html#provenance)"）。该指针不得用于读写其他 allocation。
    ///
    /// 换句话说，`let z = x.wrapping_offset((y as isize) - (x as isize))` *不会* 使 `z`
    /// 等同于 `y`，即便我们假设 `T` 的大小为 `1` 且没有溢出：`z` 仍然依附于 `x` 所依附的对象，
    /// 除非 `x` 和 `y` 指向同一个 allocation，否则解引用它就是未定义行为。
    ///
    /// 与 [`offset`] 相比，本方法基本上是把“必须停留在同一 allocation 内”这一要求推迟了：
    /// [`offset`] 在跨越对象边界时即刻构成未定义行为；而 `wrapping_offset` 会产生一个指针，但当该
    /// 指针在其所依附对象的边界之外被解引用时，仍会导致未定义行为。[`offset`] 能被更好地优化，因
    /// 此在性能敏感的代码中更可取。
    ///
    /// 这种被推迟的检查只考虑被解引用的那个指针的值，而不考虑计算最终结果过程中用到的中间值。例
    /// 如，`x.wrapping_offset(o).wrapping_offset(o.wrapping_neg())` 始终等同于 `x`。换言之，先离
    /// 开 allocation 再于稍后重新进入它是被允许的。
    ///
    /// [`offset`]: #method.offset
    /// [allocation]: crate::ptr#allocation
    ///
    /// # 示例
    ///
    /// ```
    /// # use std::fmt::Write;
    /// // 使用裸指针以每次两个元素的步长进行迭代
    /// let data = [1u8, 2, 3, 4, 5];
    /// let mut ptr: *const u8 = data.as_ptr();
    /// let step = 2;
    /// let end_rounded_up = ptr.wrapping_offset(6);
    ///
    /// let mut out = String::new();
    /// while ptr != end_rounded_up {
    ///     unsafe {
    ///         write!(&mut out, "{}, ", *ptr)?;
    ///     }
    ///     ptr = ptr.wrapping_offset(step);
    /// }
    /// assert_eq!(out.as_str(), "1, 3, 5, ");
    /// # std::fmt::Result::Ok(())
    /// ```
    #[stable(feature = "ptr_wrapping_offset", since = "1.16.0")]
    #[must_use = "returns a new pointer rather than modifying its argument"]
    #[rustc_const_stable(feature = "const_ptr_offset", since = "1.61.0")]
    #[inline(always)]
    pub const fn wrapping_offset(self, count: isize) -> *const T
    where
        T: Sized,
    {
        // SAFETY: 调用 `arith_offset` intrinsic 没有任何前置条件。
        unsafe { intrinsics::arith_offset(self, count) }
    }

    /// 使用回绕（wrapping）算术给指针加上一个以字节为单位的有符号偏移量。
    ///
    /// `count` 的单位是 **字节**。
    ///
    /// 这纯粹是一个便捷封装，相当于先转换为 `u8` 指针，再在其上调用
    /// [wrapping_offset][pointer::wrapping_offset]。文档请参见该方法。
    ///
    /// 对于非 `Sized` 的 pointee，该操作只改变 data 指针，而保持 metadata 不变。
    #[must_use]
    #[inline(always)]
    #[stable(feature = "pointer_byte_offsets", since = "1.75.0")]
    #[rustc_const_stable(feature = "const_pointer_byte_offsets", since = "1.75.0")]
    pub const fn wrapping_byte_offset(self, count: isize) -> Self {
        self.cast::<u8>().wrapping_offset(count).with_metadata_of(self)
    }

    /// 根据掩码屏蔽掉指针的某些比特位。
    ///
    /// 这是 `ptr.map_addr(|a| a & mask)` 的便捷封装。
    ///
    /// 对于非 `Sized` 的 pointee，该操作只改变 data 指针，而保持 metadata 不变。
    ///
    /// ## 示例
    ///
    /// ```
    /// #![feature(ptr_mask)]
    /// let v = 17_u32;
    /// let ptr: *const u32 = &v;
    ///
    /// // `u32` 按 4 字节对齐，
    /// // 这意味着低 2 位始终为 0。
    /// let tag_mask = 0b11;
    /// let ptr_mask = !tag_mask;
    ///
    /// // 我们可以在这些低位里存放一些东西
    /// let tagged_ptr = ptr.map_addr(|a| a | 0b10);
    ///
    /// // 把 "tag" 取回来
    /// let tag = tagged_ptr.addr() & tag_mask;
    /// assert_eq!(tag, 0b10);
    ///
    /// // 注意 `tagged_ptr` 是未对齐的，从它读取属于 UB。
    /// // 要取回原始指针，可以使用 `mask`：
    /// let masked_ptr = tagged_ptr.mask(ptr_mask);
    /// assert_eq!(unsafe { *masked_ptr }, 17);
    /// ```
    #[unstable(feature = "ptr_mask", issue = "98290")]
    #[must_use = "returns a new pointer rather than modifying its argument"]
    #[inline(always)]
    pub fn mask(self, mask: usize) -> *const T {
        intrinsics::ptr_mask(self.cast::<()>(), mask).with_metadata_of(self)
    }

    /// 计算同一 allocation 内两个指针之间的距离。返回值以 T 为单位：即以字节为单位的距离除以
    /// `size_of::<T>()`。
    ///
    /// 这等价于 `(self as isize - origin as isize) / (size_of::<T>() as isize)`，区别在于它有多得
    /// 多的产生 UB 的机会，作为交换，编译器能更好地理解你想做什么。
    ///
    /// 本方法的主要用途是：当你把一个数组/切片用一对“起始”指针与“结束”指针来表示（“结束”指的是该
    /// 数组“末尾后一位”）时，计算其 `len`。在这种情况下，`end.offset_from(start)` 就能得到该数组
    /// 的长度。
    ///
    /// 对于该用例，下面所有的安全要求都能被平凡地满足。
    ///
    /// [`offset`]: #method.offset
    ///
    /// # 安全性(Safety）
    ///
    /// 若违反以下任一条件，结果即为未定义行为：
    ///
    /// * `self` 与 `origin` 必须满足以下之一：
    ///
    ///   * 指向同一地址，或
    ///   * 两者都 [derived from][crate::ptr#provenance] 指向同一 [allocation] 的指针，并且这两个
    ///     指针之间的内存范围必须落在该对象的边界内。（见下方示例。）
    ///
    /// * 两指针之间以字节计的距离，必须是 `T` 大小的精确整数倍。
    ///
    /// 由此可得：在数学整数意义上（不发生“回绕”）计算的、两指针之间以字节计的绝对距离，不能溢出
    /// `isize`。这一点由 in-bounds 要求以及“任何 allocation 都不可能大于 `isize::MAX` 字节”这一事
    /// 实共同蕴含。
    ///
    /// “两指针必须派生自同一 allocation”这一要求，主要是为了 `const`-兼容性：指向 *不同* 已分配对
    /// 象的指针之间的距离在编译期是未知的。不过该要求在运行时同样存在，并且可能被优化所利用。如果
    /// 你想计算不保证来自同一 allocation 的两指针之间的差值，请使用 `(self as isize -
    /// origin as isize) / size_of::<T>()`。
    // FIXME: `addr()` 稳定后，建议使用它而不是 `as usize`。
    ///
    /// [`add`]: #method.add
    /// [allocation]: crate::ptr#allocation
    ///
    /// # Panics
    ///
    /// 当 `T` 是零大小类型（"ZST"）时，本函数会 panic。
    ///
    /// # 示例
    ///
    /// 基本用法：
    ///
    /// ```
    /// let a = [0; 5];
    /// let ptr1: *const i32 = &a[1];
    /// let ptr2: *const i32 = &a[3];
    /// unsafe {
    ///     assert_eq!(ptr2.offset_from(ptr1), 2);
    ///     assert_eq!(ptr1.offset_from(ptr2), -2);
    ///     assert_eq!(ptr1.offset(2), ptr2);
    ///     assert_eq!(ptr2.offset(-2), ptr1);
    /// }
    /// ```
    ///
    /// *错误* 用法：
    ///
    /// ```rust,no_run
    /// let ptr1 = Box::into_raw(Box::new(0u8)) as *const u8;
    /// let ptr2 = Box::into_raw(Box::new(1u8)) as *const u8;
    /// let diff = (ptr2 as isize).wrapping_sub(ptr1 as isize);
    /// // 使 ptr2_other 成为 ptr2.add(1) 的“别名”，但它派生自 ptr1。
    /// let ptr2_other = (ptr1 as *const u8).wrapping_offset(diff).wrapping_offset(1);
    /// assert_eq!(ptr2 as usize, ptr2_other as usize);
    /// // 由于 ptr2_other 与 ptr2 派生自指向不同对象的指针，
    /// // 计算它们的偏移量属于未定义行为，即便
    /// // 它们指向的地址落在同一对象的边界内！
    /// unsafe {
    ///     let one = ptr2_other.offset_from(ptr2); // 未定义行为！ ⚠️
    /// }
    /// ```
    #[stable(feature = "ptr_offset_from", since = "1.47.0")]
    #[rustc_const_stable(feature = "const_ptr_offset_from", since = "1.65.0")]
    #[inline]
    #[cfg_attr(miri, track_caller)] // even without panics, this helps for Miri backtraces
    pub const unsafe fn offset_from(self, origin: *const T) -> isize
    where
        T: Sized,
    {
        let pointee_size = size_of::<T>();
        assert!(0 < pointee_size && pointee_size <= isize::MAX as usize);
        // SAFETY: 调用方必须维护 `ptr_offset_from` 的安全契约。
        unsafe { intrinsics::ptr_offset_from(self, origin) }
    }

    /// 计算同一 allocation 内两个指针之间的距离。返回值以 **字节** 为单位。
    ///
    /// 这纯粹是一个便捷封装，相当于先转换为 `u8` 指针，再在其上调用
    /// [`offset_from`][pointer::offset_from]。文档与安全要求请参见该方法。
    ///
    /// 对于非 `Sized` 的 pointee，该操作只考虑 data 指针，忽略 metadata。
    #[inline(always)]
    #[stable(feature = "pointer_byte_offsets", since = "1.75.0")]
    #[rustc_const_stable(feature = "const_pointer_byte_offsets", since = "1.75.0")]
    #[cfg_attr(miri, track_caller)] // even without panics, this helps for Miri backtraces
    pub const unsafe fn byte_offset_from<U: ?Sized>(self, origin: *const U) -> isize {
        // SAFETY: 调用方必须维护 `offset_from` 的安全契约。
        unsafe { self.cast::<u8>().offset_from(origin.cast::<u8>()) }
    }

    /// 计算同一 allocation 内两个指针之间的距离，*前提是已知 `self` 大于或等于 `origin`*。返回值
    /// 以 T 为单位：即以字节为单位的距离除以 `size_of::<T>()`。
    ///
    /// 它计算的值与 [`offset_from`](#method.offset_from) 相同，但附加了一个前置条件：保证该偏移量
    /// 非负。本方法等价于 `usize::try_from(self.offset_from(origin)).unwrap_unchecked()`，但它能向
    /// 优化器提供略多一些的信息，在某些后端上有时能让优化效果稍好。
    ///
    /// 本方法可以理解为：恢复出当初传给 [`add`](#method.add) 的那个 `count`（或者，把参数顺序对调
    /// 后，恢复出传给 [`sub`](#method.sub) 的 `count`）。在满足各自安全前置条件的前提下，下面这些
    /// 写法全部等价：
    /// ```rust
    /// # unsafe fn blah(ptr: *const i32, origin: *const i32, count: usize) -> bool { unsafe {
    /// ptr.offset_from_unsigned(origin) == count
    /// # &&
    /// origin.add(count) == ptr
    /// # &&
    /// ptr.sub(count) == origin
    /// # } }
    /// ```
    ///
    /// # 安全性(Safety）
    ///
    /// - 两指针之间的距离必须非负（`self >= origin`）
    ///
    /// - [`offset_from`](#method.offset_from) 的 *所有* 安全条件也都适用于本方法；完整细节请参见该
    ///   方法。
    ///
    /// 重要提示：尽管本方法的返回类型能够表示更大的偏移量，但仍然 *不允许* 传入相差超过
    /// `isize::MAX` *字节* 的指针。因此，本方法的结果将始终小于或等于 `isize::MAX as usize`。
    ///
    /// # Panics
    ///
    /// 当 `T` 是零大小类型（"ZST"）时，本函数会 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// let a = [0; 5];
    /// let ptr1: *const i32 = &a[1];
    /// let ptr2: *const i32 = &a[3];
    /// unsafe {
    ///     assert_eq!(ptr2.offset_from_unsigned(ptr1), 2);
    ///     assert_eq!(ptr1.add(2), ptr2);
    ///     assert_eq!(ptr2.sub(2), ptr1);
    ///     assert_eq!(ptr2.offset_from_unsigned(ptr2), 0);
    /// }
    ///
    /// // 下面这样写是错误的，因为两指针的顺序不正确：
    /// // ptr1.offset_from_unsigned(ptr2)
    /// ```
    #[stable(feature = "ptr_sub_ptr", since = "1.87.0")]
    #[rustc_const_stable(feature = "const_ptr_sub_ptr", since = "1.87.0")]
    #[inline]
    #[track_caller]
    pub const unsafe fn offset_from_unsigned(self, origin: *const T) -> usize
    where
        T: Sized,
    {
        #[rustc_allow_const_fn_unstable(const_eval_select)]
        const fn runtime_ptr_ge(this: *const (), origin: *const ()) -> bool {
            const_eval_select!(
                @capture { this: *const (), origin: *const () } -> bool:
                if const {
                    true
                } else {
                    this >= origin
                }
            )
        }

        ub_checks::assert_unsafe_precondition!(
            check_language_ub,
            "ptr::offset_from_unsigned requires `self >= origin`",
            (
                this: *const () = self as *const (),
                origin: *const () = origin as *const (),
            ) => runtime_ptr_ge(this, origin)
        );

        let pointee_size = size_of::<T>();
        assert!(0 < pointee_size && pointee_size <= isize::MAX as usize);
        // SAFETY: 调用方必须维护 `ptr_offset_from_unsigned` 的安全契约。
        unsafe { intrinsics::ptr_offset_from_unsigned(self, origin) }
    }

    /// 计算同一 allocation 内两个指针之间的距离，*前提是已知 `self` 大于或等于 `origin`*。返回值
    /// 以 **字节** 为单位。
    ///
    /// 这纯粹是一个便捷封装，相当于先转换为 `u8` 指针，再在其上调用
    /// [`offset_from_unsigned`][pointer::offset_from_unsigned]。文档与安全要求请参见该方法。
    ///
    /// 对于非 `Sized` 的 pointee，该操作只考虑 data 指针，忽略 metadata。
    #[stable(feature = "ptr_sub_ptr", since = "1.87.0")]
    #[rustc_const_stable(feature = "const_ptr_sub_ptr", since = "1.87.0")]
    #[inline]
    #[track_caller]
    pub const unsafe fn byte_offset_from_unsigned<U: ?Sized>(self, origin: *const U) -> usize {
        // SAFETY: 调用方必须维护 `offset_from_unsigned` 的安全契约。
        unsafe { self.cast::<u8>().offset_from_unsigned(origin.cast::<u8>()) }
    }

    /// 返回两个指针是否被保证相等。
    ///
    /// 在运行时，本函数的行为类似 `Some(self == other)`。然而在某些上下文中（例如编译期求值），并
    /// 不总能确定两个指针是否相等，因此对于那些之后实际上能确定相等性的指针，本函数也可能虚假地返
    /// 回 `None`。但当它返回 `Some` 时，两指针的相等性就保证是确定的。
    ///
    /// 返回值可能随编译器版本不同而在 `Some` 与 `None` 之间变化，unsafe 代码绝不能依赖本函数的结
    /// 果来保证健全性（soundness）。建议仅在性能优化场景中使用本函数，即那些本函数虚假返回 `None`
    /// 也不影响最终结果、只影响性能的场景。利用本方法让运行期与编译期代码表现不同所带来的后果尚未
    /// 被探究过。不应使用本方法来引入此类差异，并且在我们对该问题有更深入理解之前，它也不应被稳定
    /// 化。
    #[unstable(feature = "const_raw_ptr_comparison", issue = "53020")]
    #[rustc_const_unstable(feature = "const_raw_ptr_comparison", issue = "53020")]
    #[inline]
    pub const fn guaranteed_eq(self, other: *const T) -> Option<bool>
    where
        T: Sized,
    {
        match intrinsics::ptr_guaranteed_cmp(self, other) {
            2 => None,
            other => Some(other == 1),
        }
    }

    /// 返回两个指针是否被保证不相等。
    ///
    /// 在运行时，本函数的行为类似 `Some(self != other)`。然而在某些上下文中（例如编译期求值），并
    /// 不总能确定两个指针是否不相等，因此对于那些之后实际上能确定不相等性的指针，本函数也可能虚假
    /// 地返回 `None`。但当它返回 `Some` 时，两指针的不相等性就保证是确定的。
    ///
    /// 返回值可能随编译器版本不同而在 `Some` 与 `None` 之间变化，unsafe 代码绝不能依赖本函数的结
    /// 果来保证健全性（soundness）。建议仅在性能优化场景中使用本函数，即那些本函数虚假返回 `None`
    /// 也不影响最终结果、只影响性能的场景。利用本方法让运行期与编译期代码表现不同所带来的后果尚未
    /// 被探究过。不应使用本方法来引入此类差异，并且在我们对该问题有更深入理解之前，它也不应被稳定
    /// 化。
    #[unstable(feature = "const_raw_ptr_comparison", issue = "53020")]
    #[rustc_const_unstable(feature = "const_raw_ptr_comparison", issue = "53020")]
    #[inline]
    pub const fn guaranteed_ne(self, other: *const T) -> Option<bool>
    where
        T: Sized,
    {
        match self.guaranteed_eq(other) {
            None => None,
            Some(eq) => Some(!eq),
        }
    }

    #[doc = include_str!("./docs/add.md")]
    ///
    /// # 示例
    ///
    /// ```
    /// let s: &str = "123";
    /// let ptr: *const u8 = s.as_ptr();
    ///
    /// unsafe {
    ///     assert_eq!(*ptr.add(1), b'2');
    ///     assert_eq!(*ptr.add(2), b'3');
    /// }
    /// ```
    #[stable(feature = "pointer_methods", since = "1.26.0")]
    #[must_use = "returns a new pointer rather than modifying its argument"]
    #[rustc_const_stable(feature = "const_ptr_offset", since = "1.61.0")]
    #[inline(always)]
    #[track_caller]
    pub const unsafe fn add(self, count: usize) -> Self
    where
        T: Sized,
    {
        #[cfg(debug_assertions)]
        #[inline]
        #[rustc_allow_const_fn_unstable(const_eval_select)]
        const fn runtime_add_nowrap(this: *const (), count: usize, size: usize) -> bool {
            const_eval_select!(
                @capture { this: *const (), count: usize, size: usize } -> bool:
                if const {
                    true
                } else {
                    let Some(byte_offset) = count.checked_mul(size) else {
                        return false;
                    };
                    let (_, overflow) = this.addr().overflowing_add(byte_offset);
                    byte_offset <= (isize::MAX as usize) && !overflow
                }
            )
        }

        #[cfg(debug_assertions)] // Expensive, and doesn't catch much in the wild.
        ub_checks::assert_unsafe_precondition!(
            check_language_ub,
            "ptr::add requires that the address calculation does not overflow",
            (
                this: *const () = self as *const (),
                count: usize = count,
                size: usize = size_of::<T>(),
            ) => runtime_add_nowrap(this, count, size)
        );

        // SAFETY: 调用方必须维护 `offset` 的安全契约。
        unsafe { intrinsics::offset(self, count) }
    }

    /// 给指针加上一个以字节为单位的无符号偏移量。
    ///
    /// `count` 的单位是字节。
    ///
    /// 这纯粹是一个便捷封装，相当于先转换为 `u8` 指针，再在其上调用 [add][pointer::add]。文档与安
    /// 全要求请参见该方法。
    ///
    /// 对于非 `Sized` 的 pointee，该操作只改变 data 指针，而保持 metadata 不变。
    #[must_use]
    #[inline(always)]
    #[stable(feature = "pointer_byte_offsets", since = "1.75.0")]
    #[rustc_const_stable(feature = "const_pointer_byte_offsets", since = "1.75.0")]
    #[track_caller]
    pub const unsafe fn byte_add(self, count: usize) -> Self {
        // SAFETY: 调用方必须维护 `add` 的安全契约。
        unsafe { self.cast::<u8>().add(count).with_metadata_of(self) }
    }

    /// 从指针中减去一个无符号偏移量。
    ///
    /// 这只能让指针向后移动（或保持不动）。如果你需要根据某个值来决定向前还是向后移动，那么你可能
    /// 想要的是接受有符号偏移量的 [`offset`](#method.offset)。
    ///
    /// `count` 的单位是 T；例如 `count` 为 3 表示偏移 `3 * size_of::<T>()` 个字节。
    ///
    /// # 安全性(Safety）
    ///
    /// 若违反以下任一条件，结果即为未定义行为：
    ///
    /// * 以字节计的偏移量 `count * size_of::<T>()`，在数学整数意义上（不发生“回绕”）计算时，必须
    ///   能装入一个 `isize`。
    ///
    /// * 如果计算出的偏移量非零，则 `self` 必须 [derived from][crate::ptr#provenance] 指向某个
    ///   [allocation] 的指针，并且 `self` 与结果之间的整个内存范围必须落在该 allocation 的边界
    ///   内。特别地，该范围不得“回绕”地址空间的边缘。
    ///
    /// Allocation 永远不可能大于 `isize::MAX` 字节，因此如果计算出的偏移量保持在该 allocation 的
    /// 边界内，就保证能满足上面的第一个要求。这意味着，举例来说，`vec.as_ptr().add(vec.len())`
    /// （对于 `vec: Vec<T>`）始终是安全的。
    ///
    /// 如果这些约束难以满足，可以考虑改用 [`wrapping_sub`]。本方法唯一的优势在于它能启用更激进的
    /// 编译器优化。
    ///
    /// [`wrapping_sub`]: #method.wrapping_sub
    /// [allocation]: crate::ptr#allocation
    ///
    /// # 示例
    ///
    /// ```
    /// let s: &str = "123";
    ///
    /// unsafe {
    ///     let end: *const u8 = s.as_ptr().add(3);
    ///     assert_eq!(*end.sub(1), b'3');
    ///     assert_eq!(*end.sub(2), b'2');
    /// }
    /// ```
    #[stable(feature = "pointer_methods", since = "1.26.0")]
    #[must_use = "returns a new pointer rather than modifying its argument"]
    #[rustc_const_stable(feature = "const_ptr_offset", since = "1.61.0")]
    #[inline(always)]
    #[track_caller]
    pub const unsafe fn sub(self, count: usize) -> Self
    where
        T: Sized,
    {
        #[cfg(debug_assertions)]
        #[inline]
        #[rustc_allow_const_fn_unstable(const_eval_select)]
        const fn runtime_sub_nowrap(this: *const (), count: usize, size: usize) -> bool {
            const_eval_select!(
                @capture { this: *const (), count: usize, size: usize } -> bool:
                if const {
                    true
                } else {
                    let Some(byte_offset) = count.checked_mul(size) else {
                        return false;
                    };
                    byte_offset <= (isize::MAX as usize) && this.addr() >= byte_offset
                }
            )
        }

        #[cfg(debug_assertions)] // Expensive, and doesn't catch much in the wild.
        ub_checks::assert_unsafe_precondition!(
            check_language_ub,
            "ptr::sub requires that the address calculation does not overflow",
            (
                this: *const () = self as *const (),
                count: usize = count,
                size: usize = size_of::<T>(),
            ) => runtime_sub_nowrap(this, count, size)
        );

        if T::IS_ZST {
            // 当 pointee 是 ZST 时，指针算术不做任何事情。
            self
        } else {
            // SAFETY: 调用方必须维护 `offset` 的安全契约。
            // 因为 pointee *不是* ZST，所以 `count` 至多为 `isize::MAX`，因此对它取负不会溢出。
            unsafe { intrinsics::offset(self, intrinsics::unchecked_sub(0, count as isize)) }
        }
    }

    /// 从指针中减去一个以字节为单位的无符号偏移量。
    ///
    /// `count` 的单位是字节。
    ///
    /// 这纯粹是一个便捷封装，相当于先转换为 `u8` 指针，再在其上调用 [sub][pointer::sub]。文档与安
    /// 全要求请参见该方法。
    ///
    /// 对于非 `Sized` 的 pointee，该操作只改变 data 指针，而保持 metadata 不变。
    #[must_use]
    #[inline(always)]
    #[stable(feature = "pointer_byte_offsets", since = "1.75.0")]
    #[rustc_const_stable(feature = "const_pointer_byte_offsets", since = "1.75.0")]
    #[track_caller]
    pub const unsafe fn byte_sub(self, count: usize) -> Self {
        // SAFETY: 调用方必须维护 `sub` 的安全契约。
        unsafe { self.cast::<u8>().sub(count).with_metadata_of(self) }
    }

    /// 使用回绕（wrapping）算术给指针加上一个无符号偏移量。
    ///
    /// `count` 的单位是 T；例如 `count` 为 3 表示偏移 `3 * size_of::<T>()` 个字节。
    ///
    /// # 安全性(Safety）
    ///
    /// 该操作本身始终是安全的，但使用其结果指针则不一定安全。
    ///
    /// 结果指针会“记住” `self` 所指向的那个 [allocation]；它不得用于读写其他 allocation。
    ///
    /// 换句话说，`let z = x.wrapping_add((y as usize) - (x as usize))` *不会* 使 `z` 等同于 `y`，
    /// 即便我们假设 `T` 的大小为 `1` 且没有溢出：`z` 仍然依附于 `x` 所依附的对象，除非 `x` 和 `y`
    /// 指向同一个 allocation，否则解引用它就是未定义行为。
    ///
    /// 与 [`add`] 相比，本方法基本上是把“必须停留在同一 allocation 内”这一要求推迟了：[`add`] 在
    /// 跨越对象边界时即刻构成未定义行为；而 `wrapping_add` 会产生一个指针，但当该指针在其所依附对
    /// 象的边界之外被解引用时，仍会导致未定义行为。[`add`] 能被更好地优化，因此在性能敏感的代码中
    /// 更可取。
    ///
    /// 这种被推迟的检查只考虑被解引用的那个指针的值，而不考虑计算最终结果过程中用到的中间值。例
    /// 如，`x.wrapping_add(o).wrapping_sub(o)` 始终等同于 `x`。换言之，先离开 allocation 再于稍后
    /// 重新进入它是被允许的。
    ///
    /// [`add`]: #method.add
    /// [allocation]: crate::ptr#allocation
    ///
    /// # 示例
    ///
    /// ```
    /// # use std::fmt::Write;
    /// // 使用裸指针以每次两个元素的步长进行迭代
    /// let data = [1u8, 2, 3, 4, 5];
    /// let mut ptr: *const u8 = data.as_ptr();
    /// let step = 2;
    /// let end_rounded_up = ptr.wrapping_add(6);
    ///
    /// let mut out = String::new();
    /// while ptr != end_rounded_up {
    ///     unsafe {
    ///         write!(&mut out, "{}, ", *ptr)?;
    ///     }
    ///     ptr = ptr.wrapping_add(step);
    /// }
    /// assert_eq!(out, "1, 3, 5, ");
    /// # std::fmt::Result::Ok(())
    /// ```
    #[stable(feature = "pointer_methods", since = "1.26.0")]
    #[must_use = "returns a new pointer rather than modifying its argument"]
    #[rustc_const_stable(feature = "const_ptr_offset", since = "1.61.0")]
    #[inline(always)]
    pub const fn wrapping_add(self, count: usize) -> Self
    where
        T: Sized,
    {
        self.wrapping_offset(count as isize)
    }

    /// 使用回绕（wrapping）算术给指针加上一个以字节为单位的无符号偏移量。
    ///
    /// `count` 的单位是字节。
    ///
    /// 这纯粹是一个便捷封装，相当于先转换为 `u8` 指针，再在其上调用
    /// [wrapping_add][pointer::wrapping_add]。文档请参见该方法。
    ///
    /// 对于非 `Sized` 的 pointee，该操作只改变 data 指针，而保持 metadata 不变。
    #[must_use]
    #[inline(always)]
    #[stable(feature = "pointer_byte_offsets", since = "1.75.0")]
    #[rustc_const_stable(feature = "const_pointer_byte_offsets", since = "1.75.0")]
    pub const fn wrapping_byte_add(self, count: usize) -> Self {
        self.cast::<u8>().wrapping_add(count).with_metadata_of(self)
    }

    /// 使用回绕（wrapping）算术从指针中减去一个无符号偏移量。
    ///
    /// `count` 的单位是 T；例如 `count` 为 3 表示偏移 `3 * size_of::<T>()` 个字节。
    ///
    /// # 安全性(Safety）
    ///
    /// 该操作本身始终是安全的，但使用其结果指针则不一定安全。
    ///
    /// 结果指针会“记住” `self` 所指向的那个 [allocation]；它不得用于读写其他 allocation。
    ///
    /// 换句话说，`let z = x.wrapping_sub((x as usize) - (y as usize))` *不会* 使 `z` 等同于 `y`，
    /// 即便我们假设 `T` 的大小为 `1` 且没有溢出：`z` 仍然依附于 `x` 所依附的对象，除非 `x` 和 `y`
    /// 指向同一个 allocation，否则解引用它就是未定义行为。
    ///
    /// 与 [`sub`] 相比，本方法基本上是把“必须停留在同一 allocation 内”这一要求推迟了：[`sub`] 在
    /// 跨越对象边界时即刻构成未定义行为；而 `wrapping_sub` 会产生一个指针，但当该指针在其所依附对
    /// 象的边界之外被解引用时，仍会导致未定义行为。[`sub`] 能被更好地优化，因此在性能敏感的代码中
    /// 更可取。
    ///
    /// 这种被推迟的检查只考虑被解引用的那个指针的值，而不考虑计算最终结果过程中用到的中间值。例
    /// 如，`x.wrapping_add(o).wrapping_sub(o)` 始终等同于 `x`。换言之，先离开 allocation 再于稍后
    /// 重新进入它是被允许的。
    ///
    /// [`sub`]: #method.sub
    /// [allocation]: crate::ptr#allocation
    ///
    /// # 示例
    ///
    /// ```
    /// # use std::fmt::Write;
    /// // 使用裸指针以每次两个元素的步长进行迭代（反向）
    /// let data = [1u8, 2, 3, 4, 5];
    /// let mut ptr: *const u8 = data.as_ptr();
    /// let start_rounded_down = ptr.wrapping_sub(2);
    /// ptr = ptr.wrapping_add(4);
    /// let step = 2;
    /// let mut out = String::new();
    /// while ptr != start_rounded_down {
    ///     unsafe {
    ///         write!(&mut out, "{}, ", *ptr)?;
    ///     }
    ///     ptr = ptr.wrapping_sub(step);
    /// }
    /// assert_eq!(out, "5, 3, 1, ");
    /// # std::fmt::Result::Ok(())
    /// ```
    #[stable(feature = "pointer_methods", since = "1.26.0")]
    #[must_use = "returns a new pointer rather than modifying its argument"]
    #[rustc_const_stable(feature = "const_ptr_offset", since = "1.61.0")]
    #[inline(always)]
    pub const fn wrapping_sub(self, count: usize) -> Self
    where
        T: Sized,
    {
        self.wrapping_offset((count as isize).wrapping_neg())
    }

    /// 使用回绕（wrapping）算术从指针中减去一个以字节为单位的无符号偏移量。
    ///
    /// `count` 的单位是字节。
    ///
    /// 这纯粹是一个便捷封装，相当于先转换为 `u8` 指针，再在其上调用
    /// [wrapping_sub][pointer::wrapping_sub]。文档请参见该方法。
    ///
    /// 对于非 `Sized` 的 pointee，该操作只改变 data 指针，而保持 metadata 不变。
    #[must_use]
    #[inline(always)]
    #[stable(feature = "pointer_byte_offsets", since = "1.75.0")]
    #[rustc_const_stable(feature = "const_pointer_byte_offsets", since = "1.75.0")]
    pub const fn wrapping_byte_sub(self, count: usize) -> Self {
        self.cast::<u8>().wrapping_sub(count).with_metadata_of(self)
    }

    /// 从 `self` 读取出其中的值，但不移动它。这会保持 `self` 处的内存不变。
    ///
    /// 关于安全性方面的考虑与示例，请参见 [`ptr::read`]。
    ///
    /// [`ptr::read`]: crate::ptr::read()
    #[stable(feature = "pointer_methods", since = "1.26.0")]
    #[rustc_const_stable(feature = "const_ptr_read", since = "1.71.0")]
    #[inline]
    #[track_caller]
    pub const unsafe fn read(self) -> T
    where
        T: Sized,
    {
        // SAFETY: 调用方必须维护 `read` 的安全契约。
        unsafe { read(self) }
    }

    /// 对 `self` 中的值执行一次 volatile（易变）读取，但不移动它。这会保持 `self` 处的内存不变。
    ///
    /// Volatile 操作意在作用于 I/O 内存，并且保证编译器不会将其消除，也不会与其他 volatile 操作之
    /// 间发生重排。
    ///
    /// 关于安全性方面的考虑与示例，请参见 [`ptr::read_volatile`]。
    ///
    /// [`ptr::read_volatile`]: crate::ptr::read_volatile()
    #[stable(feature = "pointer_methods", since = "1.26.0")]
    #[inline]
    #[track_caller]
    pub unsafe fn read_volatile(self) -> T
    where
        T: Sized,
    {
        // SAFETY: 调用方必须维护 `read_volatile` 的安全契约。
        unsafe { read_volatile(self) }
    }

    /// 从 `self` 读取出其中的值，但不移动它。这会保持 `self` 处的内存不变。
    ///
    /// 与 `read` 不同，该指针可以是未对齐的。
    ///
    /// 关于安全性方面的考虑与示例，请参见 [`ptr::read_unaligned`]。
    ///
    /// [`ptr::read_unaligned`]: crate::ptr::read_unaligned()
    #[stable(feature = "pointer_methods", since = "1.26.0")]
    #[rustc_const_stable(feature = "const_ptr_read", since = "1.71.0")]
    #[inline]
    #[track_caller]
    pub const unsafe fn read_unaligned(self) -> T
    where
        T: Sized,
    {
        // SAFETY: 调用方必须维护 `read_unaligned` 的安全契约。
        unsafe { read_unaligned(self) }
    }

    /// 把 `count * size_of::<T>()` 个字节从 `self` 复制到 `dest`。源与目标可以重叠。
    ///
    /// 注意：它的参数顺序与 [`ptr::copy`] *相同*。
    ///
    /// 关于安全性方面的考虑与示例，请参见 [`ptr::copy`]。
    ///
    /// [`ptr::copy`]: crate::ptr::copy()
    #[rustc_const_stable(feature = "const_intrinsic_copy", since = "1.83.0")]
    #[stable(feature = "pointer_methods", since = "1.26.0")]
    #[inline]
    #[track_caller]
    pub const unsafe fn copy_to(self, dest: *mut T, count: usize)
    where
        T: Sized,
    {
        // SAFETY: 调用方必须维护 `copy` 的安全契约。
        unsafe { copy(self, dest, count) }
    }

    /// 把 `count * size_of::<T>()` 个字节从 `self` 复制到 `dest`。源与目标 *不得* 重叠。
    ///
    /// 注意：它的参数顺序与 [`ptr::copy_nonoverlapping`] *相同*。
    ///
    /// 关于安全性方面的考虑与示例，请参见 [`ptr::copy_nonoverlapping`]。
    ///
    /// [`ptr::copy_nonoverlapping`]: crate::ptr::copy_nonoverlapping()
    #[rustc_const_stable(feature = "const_intrinsic_copy", since = "1.83.0")]
    #[stable(feature = "pointer_methods", since = "1.26.0")]
    #[inline]
    #[track_caller]
    pub const unsafe fn copy_to_nonoverlapping(self, dest: *mut T, count: usize)
    where
        T: Sized,
    {
        // SAFETY: 调用方必须维护 `copy_nonoverlapping` 的安全契约。
        unsafe { copy_nonoverlapping(self, dest, count) }
    }

    /// 计算为使指针对齐到 `align` 而需要施加给它的偏移量。
    ///
    /// 如果无法使该指针对齐，实现会返回 `usize::MAX`。
    ///
    /// 该偏移量以 `T` 元素的个数表示，而不是字节数。返回的值可与 `wrapping_add` 方法配合使用。
    ///
    /// 完全不保证对指针施加该偏移量后不会溢出、也不会越出该指针所指向的 allocation。除对齐之外的
    /// 一切正确性，都由调用方负责确保返回的偏移量是正确的。
    ///
    /// # Panics
    ///
    /// 如果 `align` 不是 2 的幂，本函数会 panic。
    ///
    /// # 示例
    ///
    /// 把相邻的若干个 `u8` 作为 `u16` 访问
    ///
    /// ```
    /// # unsafe {
    /// let x = [5_u8, 6, 7, 8, 9];
    /// let ptr = x.as_ptr();
    /// let offset = ptr.align_offset(align_of::<u16>());
    ///
    /// if offset < x.len() - 1 {
    ///     let u16_ptr = ptr.add(offset).cast::<u16>();
    ///     assert!(*u16_ptr == u16::from_ne_bytes([5, 6]) || *u16_ptr == u16::from_ne_bytes([6, 7]));
    /// } else {
    ///     // 虽然该指针可以通过 `offset` 对齐，但对齐后它会指向 allocation 之外
    /// }
    /// # }
    /// ```
    #[must_use]
    #[inline]
    #[stable(feature = "align_offset", since = "1.36.0")]
    pub fn align_offset(self, align: usize) -> usize
    where
        T: Sized,
    {
        if !align.is_power_of_two() {
            panic!("align_offset: align is not a power-of-two");
        }

        // SAFETY: 上面已经检查过 `align` 是 2 的幂
        let ret = unsafe { align_offset(self, align) };

        // 告知 Miri：我们希望把得到的结果指针视为已恰当对齐。
        #[cfg(miri)]
        if ret != usize::MAX {
            intrinsics::miri_promise_symbolic_alignment(self.wrapping_add(ret).cast(), align);
        }

        ret
    }

    /// 返回该指针对于 `T` 而言是否已恰当对齐。
    ///
    /// # 示例
    ///
    /// ```
    /// // 在某些平台上，i32 的对齐小于 4。
    /// #[repr(align(4))]
    /// struct AlignedI32(i32);
    ///
    /// let data = AlignedI32(42);
    /// let ptr = &data as *const AlignedI32;
    ///
    /// assert!(ptr.is_aligned());
    /// assert!(!ptr.wrapping_byte_add(1).is_aligned());
    /// ```
    #[must_use]
    #[inline]
    #[stable(feature = "pointer_is_aligned", since = "1.79.0")]
    pub fn is_aligned(self) -> bool
    where
        T: Sized,
    {
        self.is_aligned_to(align_of::<T>())
    }

    /// 返回该指针是否对齐到 `align`。
    ///
    /// 对于非 `Sized` 的 pointee，该操作只考虑 data 指针，忽略 metadata。
    ///
    /// # Panics
    ///
    /// 如果 `align` 不是 2 的幂（这也包括 0），本函数会 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(pointer_is_aligned_to)]
    ///
    /// // 在某些平台上，i32 的对齐小于 4。
    /// #[repr(align(4))]
    /// struct AlignedI32(i32);
    ///
    /// let data = AlignedI32(42);
    /// let ptr = &data as *const AlignedI32;
    ///
    /// assert!(ptr.is_aligned_to(1));
    /// assert!(ptr.is_aligned_to(2));
    /// assert!(ptr.is_aligned_to(4));
    ///
    /// assert!(ptr.wrapping_byte_add(2).is_aligned_to(2));
    /// assert!(!ptr.wrapping_byte_add(2).is_aligned_to(4));
    ///
    /// assert_ne!(ptr.is_aligned_to(8), ptr.wrapping_add(1).is_aligned_to(8));
    /// ```
    #[must_use]
    #[inline]
    #[unstable(feature = "pointer_is_aligned_to", issue = "96284")]
    pub fn is_aligned_to(self, align: usize) -> bool {
        if !align.is_power_of_two() {
            panic!("is_aligned_to: align is not a power-of-two");
        }

        self.addr() & (align - 1) == 0
    }
}

impl<T> *const T {
    /// 从某个类型转换为其 maybe-uninitialized（可能未初始化）版本。
    #[must_use]
    #[inline(always)]
    #[unstable(feature = "cast_maybe_uninit", issue = "145036")]
    pub const fn cast_uninit(self) -> *const MaybeUninit<T> {
        self as _
    }
}
impl<T> *const MaybeUninit<T> {
    /// 从 maybe-uninitialized（可能未初始化）类型转换为其已初始化版本。
    ///
    /// 这始终是安全的，因为 UB 只可能在指针被初始化之前就被读取时才会发生。
    #[must_use]
    #[inline(always)]
    #[unstable(feature = "cast_maybe_uninit", issue = "145036")]
    pub const fn cast_init(self) -> *const T {
        self as _
    }
}

impl<T> *const [T] {
    /// 返回一个裸切片的长度。
    ///
    /// 返回的值是 **元素** 的个数，而不是字节数。
    ///
    /// 即便由于指针为 null 或未对齐而无法把该裸切片转换为切片引用，本函数仍然是安全的。
    ///
    /// # 示例
    ///
    /// ```rust
    /// use std::ptr;
    ///
    /// let slice: *const [i8] = ptr::slice_from_raw_parts(ptr::null(), 3);
    /// assert_eq!(slice.len(), 3);
    /// ```
    #[inline]
    #[stable(feature = "slice_ptr_len", since = "1.79.0")]
    #[rustc_const_stable(feature = "const_slice_ptr_len", since = "1.79.0")]
    pub const fn len(self) -> usize {
        metadata(self)
    }

    /// 如果裸切片的长度为 0，则返回 `true`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::ptr;
    ///
    /// let slice: *const [i8] = ptr::slice_from_raw_parts(ptr::null(), 3);
    /// assert!(!slice.is_empty());
    /// ```
    #[inline(always)]
    #[stable(feature = "slice_ptr_len", since = "1.79.0")]
    #[rustc_const_stable(feature = "const_slice_ptr_len", since = "1.79.0")]
    pub const fn is_empty(self) -> bool {
        self.len() == 0
    }

    /// 返回一个指向该切片缓冲区的裸指针。
    ///
    /// 这等价于把 `self` 转换为 `*const T`，但更具类型安全性。
    ///
    /// # 示例
    ///
    /// ```rust
    /// #![feature(slice_ptr_get)]
    /// use std::ptr;
    ///
    /// let slice: *const [i8] = ptr::slice_from_raw_parts(ptr::null(), 3);
    /// assert_eq!(slice.as_ptr(), ptr::null());
    /// ```
    #[inline]
    #[unstable(feature = "slice_ptr_get", issue = "74265")]
    pub const fn as_ptr(self) -> *const T {
        self as *const T
    }

    /// 获取一个指向底层数组的裸指针。
    ///
    /// 如果 `N` 与 `self` 的长度不完全相等，则本方法返回 `None`。
    #[stable(feature = "core_slice_as_array", since = "1.93.0")]
    #[rustc_const_stable(feature = "core_slice_as_array", since = "1.93.0")]
    #[inline]
    #[must_use]
    pub const fn as_array<const N: usize>(self) -> Option<*const [T; N]> {
        if self.len() == N {
            let me = self.as_ptr() as *const [T; N];
            Some(me)
        } else {
            None
        }
    }

    /// 返回一个指向某个元素或子切片的裸指针，且不做边界检查。
    ///
    /// 当以越界的索引调用本方法，或在 `self` 不可解引用（dereferenceable）时调用本方法，即为
    /// *[未定义行为][undefined behavior]*，即便其结果指针并未被使用。
    ///
    /// [undefined behavior]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(slice_ptr_get)]
    ///
    /// let x = &[1, 2, 4] as *const [i32];
    ///
    /// unsafe {
    ///     assert_eq!(x.get_unchecked(1), x.as_ptr().add(1));
    /// }
    /// ```
    #[unstable(feature = "slice_ptr_get", issue = "74265")]
    #[rustc_const_unstable(feature = "const_index", issue = "143775")]
    #[inline]
    pub const unsafe fn get_unchecked<I>(self, index: I) -> *const I::Output
    where
        I: [const] SliceIndex<[T]>,
    {
        // SAFETY: 调用方确保 `self` 是可解引用的，且 `index` 在边界内。
        unsafe { index.get_unchecked(self) }
    }

    #[doc = include_str!("docs/as_uninit_slice.md")]
    #[inline]
    #[unstable(feature = "ptr_as_uninit", issue = "75402")]
    pub const unsafe fn as_uninit_slice<'a>(self) -> Option<&'a [MaybeUninit<T>]> {
        if self.is_null() {
            None
        } else {
            // SAFETY: 调用方必须维护 `as_uninit_slice` 的安全契约。
            Some(unsafe { slice::from_raw_parts(self as *const MaybeUninit<T>, self.len()) })
        }
    }
}

impl<T> *const T {
    /// 从指向 `T` 的指针转换为指向 `[T; N]` 的指针。
    #[inline]
    #[unstable(feature = "ptr_cast_array", issue = "144514")]
    pub const fn cast_array<const N: usize>(self) -> *const [T; N] {
        self.cast()
    }
}

impl<T, const N: usize> *const [T; N] {
    /// 返回一个指向该数组缓冲区的裸指针。
    ///
    /// 这等价于把 `self` 转换为 `*const T`，但更具类型安全性。
    ///
    /// # 示例
    ///
    /// ```rust
    /// #![feature(array_ptr_get)]
    /// use std::ptr;
    ///
    /// let arr: *const [i8; 3] = ptr::null();
    /// assert_eq!(arr.as_ptr(), ptr::null());
    /// ```
    #[inline]
    #[unstable(feature = "array_ptr_get", issue = "119834")]
    pub const fn as_ptr(self) -> *const T {
        self as *const T
    }

    /// 返回一个裸指针，指向包含整个数组的切片。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(array_ptr_get)]
    ///
    /// let arr: *const [i32; 3] = &[1, 2, 4] as *const [i32; 3];
    /// let slice: *const [i32] = arr.as_slice();
    /// assert_eq!(slice.len(), 3);
    /// ```
    #[inline]
    #[unstable(feature = "array_ptr_get", issue = "119834")]
    pub const fn as_slice(self) -> *const [T] {
        self
    }
}

/// 指针相等性是按地址来判断的，正如 [`<*const T>::addr`](pointer::addr) 方法所产生的地址。
#[stable(feature = "rust1", since = "1.0.0")]
#[diagnostic::on_const(
    message = "pointers cannot be reliably compared during const eval",
    note = "see issue #53020 <https://github.com/rust-lang/rust/issues/53020> for more information"
)]
impl<T: PointeeSized> PartialEq for *const T {
    #[inline]
    #[allow(ambiguous_wide_pointer_comparisons)]
    fn eq(&self, other: &*const T) -> bool {
        *self == *other
    }
}

/// 指针相等性是一个等价关系。
#[stable(feature = "rust1", since = "1.0.0")]
#[diagnostic::on_const(
    message = "pointers cannot be reliably compared during const eval",
    note = "see issue #53020 <https://github.com/rust-lang/rust/issues/53020> for more information"
)]
impl<T: PointeeSized> Eq for *const T {}

/// 指针比较是按地址来进行的，正如 `[`<*const T>::addr`](pointer::addr)` 方法所产生的地址。
#[stable(feature = "rust1", since = "1.0.0")]
#[diagnostic::on_const(
    message = "pointers cannot be reliably compared during const eval",
    note = "see issue #53020 <https://github.com/rust-lang/rust/issues/53020> for more information"
)]
impl<T: PointeeSized> Ord for *const T {
    #[inline]
    #[allow(ambiguous_wide_pointer_comparisons)]
    fn cmp(&self, other: &*const T) -> Ordering {
        if self < other {
            Less
        } else if self == other {
            Equal
        } else {
            Greater
        }
    }
}

/// 指针比较是按地址来进行的，正如 `[`<*const T>::addr`](pointer::addr)` 方法所产生的地址。
#[stable(feature = "rust1", since = "1.0.0")]
#[diagnostic::on_const(
    message = "pointers cannot be reliably compared during const eval",
    note = "see issue #53020 <https://github.com/rust-lang/rust/issues/53020> for more information"
)]
impl<T: PointeeSized> PartialOrd for *const T {
    #[inline]
    #[allow(ambiguous_wide_pointer_comparisons)]
    fn partial_cmp(&self, other: &*const T) -> Option<Ordering> {
        Some(self.cmp(other))
    }

    #[inline]
    #[allow(ambiguous_wide_pointer_comparisons)]
    fn lt(&self, other: &*const T) -> bool {
        *self < *other
    }

    #[inline]
    #[allow(ambiguous_wide_pointer_comparisons)]
    fn le(&self, other: &*const T) -> bool {
        *self <= *other
    }

    #[inline]
    #[allow(ambiguous_wide_pointer_comparisons)]
    fn gt(&self, other: &*const T) -> bool {
        *self > *other
    }

    #[inline]
    #[allow(ambiguous_wide_pointer_comparisons)]
    fn ge(&self, other: &*const T) -> bool {
        *self >= *other
    }
}

#[stable(feature = "raw_ptr_default", since = "1.88.0")]
impl<T: ?Sized + Thin> Default for *const T {
    /// 返回 [`null()`][crate::ptr::null] 的默认值。
    fn default() -> Self {
        crate::ptr::null()
    }
}
