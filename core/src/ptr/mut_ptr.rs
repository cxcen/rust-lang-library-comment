use super::*;
use crate::cmp::Ordering::{Equal, Greater, Less};
use crate::intrinsics::const_eval_select;
use crate::marker::{Destruct, PointeeSized};
use crate::mem::{self, SizedTypeProperties};
use crate::slice::{self, SliceIndex};

impl<T: PointeeSized> *mut T {
    #[doc = include_str!("docs/is_null.md")]
    ///
    /// # 示例
    ///
    /// ```
    /// let mut s = [1, 2, 3];
    /// let ptr: *mut u32 = s.as_mut_ptr();
    /// assert!(!ptr.is_null());
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_ptr_is_null", since = "1.84.0")]
    #[rustc_diagnostic_item = "ptr_is_null"]
    #[inline]
    pub const fn is_null(self) -> bool {
        self.cast_const().is_null()
    }

    /// 转换为指向另一类型的指针。
    #[stable(feature = "ptr_cast", since = "1.38.0")]
    #[rustc_const_stable(feature = "const_ptr_cast", since = "1.38.0")]
    #[rustc_diagnostic_item = "ptr_cast"]
    #[inline(always)]
    pub const fn cast<U>(self) -> *mut U {
        self as _
    }

    /// 通过检查对齐尝试转换为指向另一类型的指针。
    ///
    /// 如果该指针已针对目标类型正确对齐，就会转换为目标类型；否则返回 `None`。
    ///
    /// # 示例
    ///
    /// ```rust
    /// #![feature(pointer_try_cast_aligned)]
    ///
    /// let mut x = 0u64;
    ///
    /// let aligned: *mut u64 = &mut x;
    /// let unaligned = unsafe { aligned.byte_add(1) };
    ///
    /// assert!(aligned.try_cast_aligned::<u32>().is_some());
    /// assert!(unaligned.try_cast_aligned::<u32>().is_none());
    /// ```
    #[unstable(feature = "pointer_try_cast_aligned", issue = "141221")]
    #[must_use = "this returns the result of the operation, \
                  without modifying the original"]
    #[inline]
    pub fn try_cast_aligned<U>(self) -> Option<*mut U> {
        if self.is_aligned_to(align_of::<U>()) { Some(self.cast()) } else { None }
    }

    /// 在指向另一类型的新指针中复用本指针的地址值。
    ///
    /// 此操作会忽略 `meta` 操作数的地址部分，并丢弃 `self` 现有的 metadata。对于指向 sized 类型的
    /// 指针（thin 指针），其效果等同于一次简单的 cast；对于指向 unsized 类型的指针（fat 指针），它会
    /// 把地址与新的 metadata（如切片长度或 `dyn`-vtable）重新组合。
    ///
    /// 结果指针将拥有 `self` 的 provenance。从语义上讲，此操作等价于用 `self` 的数据指针值与 `meta`
    /// 的 metadata 新建一个指针，其为 fat 还是 thin 取决于 `meta` 操作数。
    ///
    /// # 示例
    ///
    /// 此函数主要用于在可能是 fat 指针的指针上启用指针算术。先把指针 cast 为 sized 的 pointee 以使用
    /// offset 系列操作，再与它自己原有的 metadata 重新组合。
    ///
    /// ```
    /// #![feature(set_ptr_value)]
    /// # use core::fmt::Debug;
    /// let mut arr: [i32; 3] = [1, 2, 3];
    /// let mut ptr = arr.as_mut_ptr() as *mut dyn Debug;
    /// let thin = ptr as *mut u8;
    /// unsafe {
    ///     ptr = thin.add(8).with_metadata_of(ptr);
    ///     # assert_eq!(*(ptr as *mut i32), 3);
    ///     println!("{:?}", &*ptr); // 将打印 "3"
    /// }
    /// ```
    ///
    /// # *错误* 用法
    ///
    /// 来自两个指针的 provenance *不会* 被合并。结果指针只能用于引用 `self` 所允许的地址。
    ///
    /// ```rust,no_run
    /// #![feature(set_ptr_value)]
    /// let mut x = 0u32;
    /// let mut y = 1u32;
    ///
    /// let x = (&mut x) as *mut u32;
    /// let y = (&mut y) as *mut u32;
    ///
    /// let offset = (x as usize - y as usize) / 4;
    /// let bad = x.wrapping_add(offset).with_metadata_of(y);
    ///
    /// // 此解引用是 UB。该指针只拥有 `x` 的 provenance，却指向 `y`。
    /// println!("{:?}", unsafe { &*bad });
    /// ```
    #[unstable(feature = "set_ptr_value", issue = "75091")]
    #[must_use = "returns a new pointer rather than modifying its argument"]
    #[inline]
    pub const fn with_metadata_of<U>(self, meta: *const U) -> *mut U
    where
        U: PointeeSized,
    {
        from_raw_parts_mut::<U>(self as *mut (), metadata(meta))
    }

    /// 在不改变类型的情况下改变 constness。
    ///
    /// 这比 `as` 略微安全一些，因为在代码被重构时它不会悄悄改变类型。
    ///
    /// 虽然并非严格必要（`*mut T` 可强转为 `*const T`），但提供它是为了与 `*const T` 上的
    /// [`cast_mut`] 保持对称，并且在用它替代隐式强转时可能具有文档价值。
    ///
    /// [`cast_mut`]: pointer::cast_mut
    #[stable(feature = "ptr_const_cast", since = "1.65.0")]
    #[rustc_const_stable(feature = "ptr_const_cast", since = "1.65.0")]
    #[rustc_diagnostic_item = "ptr_cast_const"]
    #[inline(always)]
    pub const fn cast_const(self) -> *const T {
        self as _
    }

    #[doc = include_str!("./docs/addr.md")]
    ///
    /// [without_provenance]: without_provenance_mut
    #[must_use]
    #[inline(always)]
    #[stable(feature = "strict_provenance", since = "1.84.0")]
    pub fn addr(self) -> usize {
        // A pointer-to-integer transmute currently has exactly the right semantics: it returns the
        // address without exposing the provenance. Note that this is *not* a stable guarantee about
        // transmute semantics, it relies on sysroot crates having special status.
        // 指针到整数的 transmute 目前恰好具备所需语义：它返回地址而不暴露 provenance。注意这 *不是*
        // 关于 transmute 语义的稳定保证，它依赖于 sysroot crate 拥有特殊地位。
        // SAFETY: 指针到整数的 transmute 是有效的（前提是你接受丢失 provenance）。
        unsafe { mem::transmute(self.cast::<()>()) }
    }

    /// 暴露指针的 ["provenance"][crate::ptr#provenance] 部分以便日后在
    /// [`with_exposed_provenance_mut`] 中使用，并返回其 "address"（地址）部分。
    ///
    /// 这等价于 `self as usize`，在语义上会丢弃 provenance 信息。此外，它（如同 `as` cast 一样）带有把
    /// provenance 标记为 'exposed'（已暴露）的隐式副作用，因此在支持的平台上，你之后可以调用
    /// [`with_exposed_provenance_mut`] 来重建包含其 provenance 的原始指针。
    ///
    /// 由于其固有的歧义，[`with_exposed_provenance_mut`] 可能不被那些帮助你保持符合 Rust 内存模型的工具
    /// 所支持。建议尽可能使用 [Strict Provenance][crate::ptr#strict-provenance] API，例如
    /// [`with_addr`][pointer::with_addr]，在那种情况下应使用 [`addr`][pointer::addr] 而非
    /// `expose_provenance`。
    ///
    /// 在大多数平台上这会产生与原始指针字节相同的值，因为所有字节都用于描述地址。需要在指针中存储额外信息的
    /// 平台可能不支持此操作，因为 [`with_exposed_provenance_mut`] 工作所需的 'expose' 副作用通常不可用。
    ///
    /// 这是一个 [Exposed Provenance][crate::ptr#exposed-provenance] API。
    ///
    /// [`with_exposed_provenance_mut`]: with_exposed_provenance_mut
    #[inline(always)]
    #[stable(feature = "exposed_provenance", since = "1.84.0")]
    pub fn expose_provenance(self) -> usize {
        self.cast::<()>() as usize
    }

    /// 以给定的地址和 `self` 的 [provenance][crate::ptr#provenance] 创建一个新指针。
    ///
    /// 这类似于 `addr as *mut T` cast，但会把 `self` 的 *provenance* 复制到新指针上。
    /// 这避免了一元 cast 所固有的歧义。
    ///
    /// 这等价于用 [`wrapping_offset`][pointer::wrapping_offset] 把 `self` 偏移到给定地址，
    /// 因此具有与之完全相同的能力和限制。
    ///
    /// 这是一个 [Strict Provenance][crate::ptr#strict-provenance] API。
    #[must_use]
    #[inline]
    #[stable(feature = "strict_provenance", since = "1.84.0")]
    pub fn with_addr(self, addr: usize) -> Self {
        // This should probably be an intrinsic to avoid doing any sort of arithmetic, but
        // meanwhile, we can implement it with `wrapping_offset`, which preserves the pointer's
        // provenance.
        // 为避免做任何算术，这本应是一个 intrinsic，但在此之前，我们可以用 `wrapping_offset` 来实现它，
        // 它会保留指针的 provenance。
        let self_addr = self.addr() as isize;
        let dest_addr = addr as isize;
        let offset = dest_addr.wrapping_sub(self_addr);
        self.wrapping_byte_offset(offset)
    }

    /// 通过把 `self` 的地址映射为一个新地址来创建新指针，同时保留原指针的
    /// [provenance][crate::ptr#provenance]。
    ///
    /// 这是 [`with_addr`][pointer::with_addr] 的便捷封装，详见该方法。
    ///
    /// 这是一个 [Strict Provenance][crate::ptr#strict-provenance] API。
    #[must_use]
    #[inline]
    #[stable(feature = "strict_provenance", since = "1.84.0")]
    pub fn map_addr(self, f: impl FnOnce(usize) -> usize) -> Self {
        self.with_addr(f(self.addr()))
    }

    /// 将一个（可能是宽指针的）指针分解为其数据指针和 metadata 两个组成部分。
    ///
    /// 之后可用 [`from_raw_parts_mut`] 重新构造该指针。
    #[unstable(feature = "ptr_metadata", issue = "81513")]
    #[inline]
    pub const fn to_raw_parts(self) -> (*mut (), <T as super::Pointee>::Metadata) {
        (self.cast(), super::metadata(self))
    }

    #[doc = include_str!("./docs/as_ref.md")]
    ///
    /// ```
    /// let ptr: *mut u8 = &mut 10u8 as *mut u8;
    ///
    /// unsafe {
    ///     let val_back = &*ptr;
    ///     println!("We got back the value: {val_back}!");
    /// }
    /// ```
    ///
    /// # 示例
    ///
    /// ```
    /// let ptr: *mut u8 = &mut 10u8 as *mut u8;
    ///
    /// unsafe {
    ///     if let Some(val_back) = ptr.as_ref() {
    ///         println!("We got back the value: {val_back}!");
    ///     }
    /// }
    /// ```
    ///
    /// # 另请参阅
    ///
    /// 可变版本见 [`as_mut`]。
    ///
    /// [`is_null`]: #method.is_null-1
    /// [`as_uninit_ref`]: pointer#method.as_uninit_ref-1
    /// [`as_mut`]: #method.as_mut

    #[stable(feature = "ptr_as_ref", since = "1.9.0")]
    #[rustc_const_stable(feature = "const_ptr_is_null", since = "1.84.0")]
    #[inline]
    pub const unsafe fn as_ref<'a>(self) -> Option<&'a T> {
        // SAFETY: 调用方必须保证：若 `self` 非空，则它对一个引用而言是有效的。
        if self.is_null() { None } else { unsafe { Some(&*self) } }
    }

    /// 返回指针所指向值的共享引用。
    /// 如果指针可能为空，或者值可能未初始化，则必须改用 [`as_uninit_ref`]。
    /// 如果指针可能为空，但已知值已被初始化，则必须改用 [`as_ref`]。
    ///
    /// 可变版本见 [`as_mut_unchecked`]。
    ///
    /// [`as_ref`]: #method.as_ref
    /// [`as_uninit_ref`]: #method.as_uninit_ref
    /// [`as_mut_unchecked`]: #method.as_mut_unchecked
    ///
    /// # 安全性(Safety）
    ///
    /// 调用此方法时，你必须确保该指针是[可转换为引用的](crate::ptr#pointer-to-reference-conversion)。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(ptr_as_ref_unchecked)]
    /// let ptr: *mut u8 = &mut 10u8 as *mut u8;
    ///
    /// unsafe {
    ///     println!("We got back the value: {}!", ptr.as_ref_unchecked());
    /// }
    /// ```
    // FIXME: mention it in the docs for `as_ref` and `as_uninit_ref` once stabilized.
    #[unstable(feature = "ptr_as_ref_unchecked", issue = "122034")]
    #[inline]
    #[must_use]
    pub const unsafe fn as_ref_unchecked<'a>(self) -> &'a T {
        // SAFETY: 调用方必须保证 `self` 对一个引用而言是有效的
        unsafe { &*self }
    }

    #[doc = include_str!("./docs/as_uninit_ref.md")]
    ///
    /// [`is_null`]: #method.is_null-1
    /// [`as_ref`]: pointer#method.as_ref-1
    ///
    /// # 另请参阅
    /// 可变版本见 [`as_uninit_mut`]。
    ///
    /// [`as_uninit_mut`]: #method.as_uninit_mut
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(ptr_as_uninit)]
    ///
    /// let ptr: *mut u8 = &mut 10u8 as *mut u8;
    ///
    /// unsafe {
    ///     if let Some(val_back) = ptr.as_uninit_ref() {
    ///         println!("We got back the value: {}!", val_back.assume_init());
    ///     }
    /// }
    /// ```
    #[inline]
    #[unstable(feature = "ptr_as_uninit", issue = "75402")]
    pub const unsafe fn as_uninit_ref<'a>(self) -> Option<&'a MaybeUninit<T>>
    where
        T: Sized,
    {
        // SAFETY: 调用方必须保证 `self` 满足一个引用的全部要求。
        if self.is_null() { None } else { Some(unsafe { &*(self as *const MaybeUninit<T>) }) }
    }

    #[doc = include_str!("./docs/offset.md")]
    ///
    /// # 示例
    ///
    /// ```
    /// let mut s = [1, 2, 3];
    /// let ptr: *mut u32 = s.as_mut_ptr();
    ///
    /// unsafe {
    ///     assert_eq!(2, *ptr.offset(1));
    ///     assert_eq!(3, *ptr.offset(2));
    /// }
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[must_use = "returns a new pointer rather than modifying its argument"]
    #[rustc_const_stable(feature = "const_ptr_offset", since = "1.61.0")]
    #[inline(always)]
    #[track_caller]
    pub const unsafe fn offset(self, count: isize) -> *mut T
    where
        T: Sized,
    {
        #[inline]
        #[rustc_allow_const_fn_unstable(const_eval_select)]
        const fn runtime_offset_nowrap(this: *const (), count: isize, size: usize) -> bool {
            // We can use const_eval_select here because this is only for UB checks.
            // 这里可以使用 const_eval_select，因为它仅用于 UB 检查。
            const_eval_select!(
                @capture { this: *const (), count: isize, size: usize } -> bool:
                if const {
                    true
                } else {
                    // `size` is the size of a Rust type, so we know that
                    // `size <= isize::MAX` and thus `as` cast here is not lossy.
                    // `size` 是某个 Rust 类型的大小，因此我们知道 `size <= isize::MAX`，
                    // 故此处的 `as` cast 不会丢失精度。
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
        // 由于调用方必须保证结果指针与 `self` 指向同一分配对象，
        // 因此得到的指针对写入而言是有效的。
        unsafe { intrinsics::offset(self, count) }
    }

    /// 以字节为单位为指针加上一个带符号偏移量。
    ///
    /// `count` 的单位是 **字节**。
    ///
    /// 这纯粹是把指针 cast 为 `u8` 指针再在其上使用 [offset][pointer::offset] 的便捷封装。文档与安全
    /// 要求详见该方法。
    ///
    /// 对于非 `Sized` 的 pointee，此操作只改变数据指针，metadata 保持不变。
    #[must_use]
    #[inline(always)]
    #[stable(feature = "pointer_byte_offsets", since = "1.75.0")]
    #[rustc_const_stable(feature = "const_pointer_byte_offsets", since = "1.75.0")]
    #[track_caller]
    pub const unsafe fn byte_offset(self, count: isize) -> Self {
        // SAFETY: 调用方必须维护 `offset` 的安全契约。
        unsafe { self.cast::<u8>().offset(count).with_metadata_of(self) }
    }

    /// 以 wrapping 算术为指针加上一个带符号偏移量。
    ///
    /// `count` 的单位是 T；例如 `count` 为 3 表示偏移 `3 * size_of::<T>()` 字节。
    ///
    /// # 安全性(Safety）
    ///
    /// 此操作本身总是安全的，但使用其结果指针则不然。
    ///
    /// 结果指针会“记住”`self` 指向的那个 [allocation]（这称为 "[Provenance](ptr/index.html#provenance)"）。
    /// 该指针不得用于读写其他分配对象。
    ///
    /// 换言之，`let z = x.wrapping_offset((y as isize) - (x as isize))` *不会* 使 `z` 等同于 `y`，
    /// 即便我们假设 `T` 大小为 `1` 且不发生溢出：`z` 仍然附属于 `x` 所附属的对象，除非 `x` 和 `y` 指向
    /// 同一分配对象，否则解引用它就是未定义行为。
    ///
    /// 与 [`offset`] 相比，此方法基本上推迟了“停留在同一分配对象内”这一要求：[`offset`] 在跨越对象边界时
    /// 立即构成未定义行为；而 `wrapping_offset` 会产出一个指针，但如果在指针越出其所附属对象的边界时被
    /// 解引用，仍会导致未定义行为。[`offset`] 能被更好地优化，因此在性能敏感的代码中更可取。
    ///
    /// 这个被推迟的检查只考虑被解引用的那个指针值，而不考虑计算最终结果过程中用到的中间值。例如，
    /// `x.wrapping_offset(o).wrapping_offset(o.wrapping_neg())` 总是等于 `x`。换言之，离开分配对象再于
    /// 稍后重新进入是允许的。
    ///
    /// [`offset`]: #method.offset
    /// [allocation]: crate::ptr#allocation
    ///
    /// # 示例
    ///
    /// ```
    /// // 以每次两个元素的步长用裸指针进行迭代
    /// let mut data = [1u8, 2, 3, 4, 5];
    /// let mut ptr: *mut u8 = data.as_mut_ptr();
    /// let step = 2;
    /// let end_rounded_up = ptr.wrapping_offset(6);
    ///
    /// while ptr != end_rounded_up {
    ///     unsafe {
    ///         *ptr = 0;
    ///     }
    ///     ptr = ptr.wrapping_offset(step);
    /// }
    /// assert_eq!(&data, &[0, 2, 0, 4, 0]);
    /// ```
    #[stable(feature = "ptr_wrapping_offset", since = "1.16.0")]
    #[must_use = "returns a new pointer rather than modifying its argument"]
    #[rustc_const_stable(feature = "const_ptr_offset", since = "1.61.0")]
    #[inline(always)]
    pub const fn wrapping_offset(self, count: isize) -> *mut T
    where
        T: Sized,
    {
        // SAFETY: `arith_offset` intrinsic 的调用没有任何前置条件。
        unsafe { intrinsics::arith_offset(self, count) as *mut T }
    }

    /// 以 wrapping 算术、以字节为单位为指针加上一个带符号偏移量。
    ///
    /// `count` 的单位是 **字节**。
    ///
    /// 这纯粹是把指针 cast 为 `u8` 指针再在其上使用 [wrapping_offset][pointer::wrapping_offset] 的便捷
    /// 封装。文档详见该方法。
    ///
    /// 对于非 `Sized` 的 pointee，此操作只改变数据指针，metadata 保持不变。
    #[must_use]
    #[inline(always)]
    #[stable(feature = "pointer_byte_offsets", since = "1.75.0")]
    #[rustc_const_stable(feature = "const_pointer_byte_offsets", since = "1.75.0")]
    pub const fn wrapping_byte_offset(self, count: isize) -> Self {
        self.cast::<u8>().wrapping_offset(count).with_metadata_of(self)
    }

    /// 按照掩码屏蔽掉指针的某些位。
    ///
    /// 这是 `ptr.map_addr(|a| a & mask)` 的便捷写法。
    ///
    /// 对于非 `Sized` 的 pointee，此操作只改变数据指针，metadata 保持不变。
    ///
    /// ## 示例
    ///
    /// ```
    /// #![feature(ptr_mask)]
    /// let mut v = 17_u32;
    /// let ptr: *mut u32 = &mut v;
    ///
    /// // `u32` 按 4 字节对齐，
    /// // 这意味着低 2 位始终为 0。
    /// let tag_mask = 0b11;
    /// let ptr_mask = !tag_mask;
    ///
    /// // 我们可以在这些低位中存放一些东西
    /// let tagged_ptr = ptr.map_addr(|a| a | 0b10);
    ///
    /// // 把 "tag" 取回来
    /// let tag = tagged_ptr.addr() & tag_mask;
    /// assert_eq!(tag, 0b10);
    ///
    /// // 注意 `tagged_ptr` 是未对齐的，对它读写是 UB。
    /// // 要取回原始指针，可以使用 `mask`：
    /// let masked_ptr = tagged_ptr.mask(ptr_mask);
    /// assert_eq!(unsafe { *masked_ptr }, 17);
    ///
    /// unsafe { *masked_ptr = 0 };
    /// assert_eq!(v, 0);
    /// ```
    #[unstable(feature = "ptr_mask", issue = "98290")]
    #[must_use = "returns a new pointer rather than modifying its argument"]
    #[inline(always)]
    pub fn mask(self, mask: usize) -> *mut T {
        intrinsics::ptr_mask(self.cast::<()>(), mask).cast_mut().with_metadata_of(self)
    }

    /// 如果指针为空则返回 `None`，否则返回包裹在 `Some` 中、指向该值的独占引用。如果值可能未初始化，
    /// 则必须改用 [`as_uninit_mut`]。
    ///
    /// 共享版本见 [`as_ref`]。
    ///
    /// [`as_uninit_mut`]: #method.as_uninit_mut
    /// [`as_ref`]: pointer#method.as_ref-1
    ///
    /// # 安全性(Safety）
    ///
    /// 调用此方法时，你必须确保 *要么* 指针为空，*要么*
    /// 指针是[可转换为引用的](crate::ptr#pointer-to-reference-conversion)。
    ///
    /// # Panics during const evaluation
    ///
    /// 如果在 const 求值期间无法确定指针是否为空，此方法将 panic。详见 [`is_null`]。
    ///
    /// [`is_null`]: #method.is_null-1
    ///
    /// # 示例
    ///
    /// ```
    /// let mut s = [1, 2, 3];
    /// let ptr: *mut u32 = s.as_mut_ptr();
    /// let first_value = unsafe { ptr.as_mut().unwrap() };
    /// *first_value = 4;
    /// # assert_eq!(s, [4, 2, 3]);
    /// println!("{s:?}"); // 将打印："[4, 2, 3]"。
    /// ```
    ///
    /// # 免空检查版本
    ///
    /// 如果你确信指针永远不会为空，并且想要某种返回 `&mut T`（而非 `Option<&mut T>`）的
    /// `as_mut_unchecked`，要知道你可以直接解引用该指针。
    ///
    /// ```
    /// let mut s = [1, 2, 3];
    /// let ptr: *mut u32 = s.as_mut_ptr();
    /// let first_value = unsafe { &mut *ptr };
    /// *first_value = 4;
    /// # assert_eq!(s, [4, 2, 3]);
    /// println!("{s:?}"); // 将打印："[4, 2, 3]"。
    /// ```
    #[stable(feature = "ptr_as_ref", since = "1.9.0")]
    #[rustc_const_stable(feature = "const_ptr_is_null", since = "1.84.0")]
    #[inline]
    pub const unsafe fn as_mut<'a>(self) -> Option<&'a mut T> {
        // SAFETY: 调用方必须保证：若 `self` 非空，则它对一个可变引用而言是有效的。
        if self.is_null() { None } else { unsafe { Some(&mut *self) } }
    }

    /// 返回指针所指向值的独占引用。
    /// 如果指针可能为空，或者值可能未初始化，则必须改用 [`as_uninit_mut`]。
    /// 如果指针可能为空，但已知值已被初始化，则必须改用 [`as_mut`]。
    ///
    /// 共享版本见 [`as_ref_unchecked`]。
    ///
    /// [`as_mut`]: #method.as_mut
    /// [`as_uninit_mut`]: #method.as_uninit_mut
    /// [`as_ref_unchecked`]: #method.as_mut_unchecked
    ///
    /// # 安全性(Safety）
    ///
    /// 调用此方法时，你必须确保该指针是[可转换为引用的](crate::ptr#pointer-to-reference-conversion)。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(ptr_as_ref_unchecked)]
    /// let mut s = [1, 2, 3];
    /// let ptr: *mut u32 = s.as_mut_ptr();
    /// let first_value = unsafe { ptr.as_mut_unchecked() };
    /// *first_value = 4;
    /// # assert_eq!(s, [4, 2, 3]);
    /// println!("{s:?}"); // 将打印："[4, 2, 3]"。
    /// ```
    // FIXME: mention it in the docs for `as_mut` and `as_uninit_mut` once stabilized.
    #[unstable(feature = "ptr_as_ref_unchecked", issue = "122034")]
    #[inline]
    #[must_use]
    pub const unsafe fn as_mut_unchecked<'a>(self) -> &'a mut T {
        // SAFETY: 调用方必须保证 `self` 对一个引用而言是有效的
        unsafe { &mut *self }
    }

    /// 如果指针为空则返回 `None`，否则返回包裹在 `Some` 中、指向该值的独占引用。与 [`as_mut`] 不同，
    /// 此方法不要求值必须已被初始化。
    ///
    /// 共享版本见 [`as_uninit_ref`]。
    ///
    /// [`as_mut`]: #method.as_mut
    /// [`as_uninit_ref`]: pointer#method.as_uninit_ref-1
    ///
    /// # 安全性(Safety）
    ///
    /// 调用此方法时，你必须确保 *要么* 指针为空，*要么*
    /// 指针是[可转换为引用的](crate::ptr#pointer-to-reference-conversion)。
    ///
    /// # Panics during const evaluation
    ///
    /// 如果在 const 求值期间无法确定指针是否为空，此方法将 panic。详见 [`is_null`]。
    ///
    /// [`is_null`]: #method.is_null-1
    #[inline]
    #[unstable(feature = "ptr_as_uninit", issue = "75402")]
    pub const unsafe fn as_uninit_mut<'a>(self) -> Option<&'a mut MaybeUninit<T>>
    where
        T: Sized,
    {
        // SAFETY: 调用方必须保证 `self` 满足一个引用的全部要求。
        if self.is_null() { None } else { Some(unsafe { &mut *(self as *mut MaybeUninit<T>) }) }
    }

    /// 返回两个指针是否被保证相等。
    ///
    /// 在运行时，此函数的行为类似 `Some(self == other)`。
    /// 然而在某些上下文中（例如编译期求值），并不总能确定两个指针的相等性，因此此函数对于之后实际上确实
    /// 已知相等性的指针，也可能虚假地返回 `None`。但当它返回 `Some` 时，指针的相等性保证是已知的。
    ///
    /// 返回值可能随编译器版本在 `Some` 与 `None` 之间来回变化，unsafe 代码不得依赖此函数的结果来保证
    /// 健全性。建议仅在性能优化中使用此函数，且要求此函数虚假返回 `None` 不会影响结果、只影响性能。
    /// 用此方法使运行时与编译期代码表现不同所带来的后果尚未被探明。本方法不应被用于引入此类差异，并且在我们
    /// 对该问题有更好理解之前也不应被稳定化。
    #[unstable(feature = "const_raw_ptr_comparison", issue = "53020")]
    #[rustc_const_unstable(feature = "const_raw_ptr_comparison", issue = "53020")]
    #[inline]
    pub const fn guaranteed_eq(self, other: *mut T) -> Option<bool>
    where
        T: Sized,
    {
        (self as *const T).guaranteed_eq(other as _)
    }

    /// 返回两个指针是否被保证不相等。
    ///
    /// 在运行时，此函数的行为类似 `Some(self != other)`。
    /// 然而在某些上下文中（例如编译期求值），并不总能确定两个指针的不相等性，因此此函数对于之后实际上确实
    /// 已知不相等性的指针，也可能虚假地返回 `None`。但当它返回 `Some` 时，指针的不相等性保证是已知的。
    ///
    /// 返回值可能随编译器版本在 `Some` 与 `None` 之间来回变化，unsafe 代码不得依赖此函数的结果来保证
    /// 健全性。建议仅在性能优化中使用此函数，且要求此函数虚假返回 `None` 不会影响结果、只影响性能。
    /// 用此方法使运行时与编译期代码表现不同所带来的后果尚未被探明。本方法不应被用于引入此类差异，并且在我们
    /// 对该问题有更好理解之前也不应被稳定化。
    #[unstable(feature = "const_raw_ptr_comparison", issue = "53020")]
    #[rustc_const_unstable(feature = "const_raw_ptr_comparison", issue = "53020")]
    #[inline]
    pub const fn guaranteed_ne(self, other: *mut T) -> Option<bool>
    where
        T: Sized,
    {
        (self as *const T).guaranteed_ne(other as _)
    }

    /// 计算同一分配对象内两个指针之间的距离。返回值以 T 为单位：即字节距离除以 `size_of::<T>()`。
    ///
    /// 这等价于 `(self as isize - origin as isize) / (size_of::<T>() as isize)`，区别在于它有多得多的
    /// UB 可能性，作为交换，编译器能更好地理解你在做什么。
    ///
    /// 此方法的主要动机在于：当你正用一个“起始”指针和一个“结束”指针（“结束”是数组的“末尾后一个位置”）来表示
    /// 某个 `T` 的数组/切片时，用它计算该数组/切片的 `len`。在那种情况下，`end.offset_from(start)`
    /// 会给出数组的长度。
    ///
    /// 对于这一用例，下面所有安全要求都是平凡满足的。
    ///
    /// [`offset`]: pointer#method.offset-1
    ///
    /// # 安全性(Safety）
    ///
    /// 如果违反以下任一条件，结果即为未定义行为：
    ///
    /// * `self` 和 `origin` 必须满足以下之一
    ///
    ///   * 指向同一地址，或
    ///   * 二者都[派生自][crate::ptr#provenance]指向同一 [allocation] 的指针，且两个指针之间的内存
    ///     范围必须在该对象的边界内。（示例见下文。）
    ///
    /// * 两个指针之间以字节计的距离，必须是 `T` 大小的精确整数倍。
    ///
    /// 由此可知，两个指针之间以字节计、按数学整数计算（不“回绕”）的绝对距离，不能溢出 `isize`。这一点由
    /// in-bounds 要求以及“任何分配对象都不能大于 `isize::MAX` 字节”这一事实所蕴含。
    ///
    /// 要求指针派生自同一分配对象，主要是出于 `const` 兼容性：指向 *不同* 分配对象的指针之间的距离在编译期
    /// 是未知的。然而该要求在运行时同样存在，并且可能被优化所利用。如果你希望计算不保证来自同一分配对象的
    /// 两个指针之间的差值，请使用 `(self as isize - origin as isize) / size_of::<T>()`。
    // FIXME: recommend `addr()` instead of `as usize` once that is stable.
    ///
    /// [`add`]: #method.add
    /// [allocation]: crate::ptr#allocation
    ///
    /// # Panics
    ///
    /// 如果 `T` 是零大小类型（"ZST"），此函数会 panic。
    ///
    /// # 示例
    ///
    /// 基本用法：
    ///
    /// ```
    /// let mut a = [0; 5];
    /// let ptr1: *mut i32 = &mut a[1];
    /// let ptr2: *mut i32 = &mut a[3];
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
    /// let ptr1 = Box::into_raw(Box::new(0u8));
    /// let ptr2 = Box::into_raw(Box::new(1u8));
    /// let diff = (ptr2 as isize).wrapping_sub(ptr1 as isize);
    /// // 让 ptr2_other 成为 ptr2.add(1) 的“别名”，但派生自 ptr1。
    /// let ptr2_other = (ptr1 as *mut u8).wrapping_offset(diff).wrapping_offset(1);
    /// assert_eq!(ptr2 as usize, ptr2_other as usize);
    /// // 由于 ptr2_other 和 ptr2 派生自指向不同对象的指针，
    /// // 计算它们之间的偏移是未定义行为，即使
    /// // 它们指向的地址都在同一对象的边界内！
    /// unsafe {
    ///     let one = ptr2_other.offset_from(ptr2); // 未定义行为！⚠️
    /// }
    /// ```
    #[stable(feature = "ptr_offset_from", since = "1.47.0")]
    #[rustc_const_stable(feature = "const_ptr_offset_from", since = "1.65.0")]
    #[inline(always)]
    #[cfg_attr(miri, track_caller)] // 即便不会 panic，这对 Miri 的回溯也有帮助
    pub const unsafe fn offset_from(self, origin: *const T) -> isize
    where
        T: Sized,
    {
        // SAFETY: 调用方必须维护 `offset_from` 的安全契约。
        unsafe { (self as *const T).offset_from(origin) }
    }

    /// 计算同一分配对象内两个指针之间的距离。返回值以 **字节** 为单位。
    ///
    /// 这纯粹是把指针 cast 为 `u8` 指针再在其上使用 [`offset_from`][pointer::offset_from] 的便捷封装。
    /// 文档与安全要求详见该方法。
    ///
    /// 对于非 `Sized` 的 pointee，此操作只考虑数据指针，忽略 metadata。
    #[inline(always)]
    #[stable(feature = "pointer_byte_offsets", since = "1.75.0")]
    #[rustc_const_stable(feature = "const_pointer_byte_offsets", since = "1.75.0")]
    #[cfg_attr(miri, track_caller)] // 即便不会 panic，这对 Miri 的回溯也有帮助
    pub const unsafe fn byte_offset_from<U: ?Sized>(self, origin: *const U) -> isize {
        // SAFETY: 调用方必须维护 `offset_from` 的安全契约。
        unsafe { self.cast::<u8>().offset_from(origin.cast::<u8>()) }
    }

    /// 计算同一分配对象内两个指针之间的距离，*前提是已知 `self` 大于或等于 `origin`*。返回值以 T 为单位：
    /// 即字节距离除以 `size_of::<T>()`。
    ///
    /// 它计算的值与 [`offset_from`](#method.offset_from) 相同，但附加了一个前置条件：偏移量保证非负。
    /// 此方法等价于 `usize::try_from(self.offset_from(origin)).unwrap_unchecked()`，但它向优化器提供了
    /// 略多一些信息，从而在某些后端上有时能优化得稍好一些。
    ///
    /// 此方法可以被看作恢复出当初传给 [`add`](#method.add) 的那个 `count`（或者，把两个参数顺序对调，
    /// 即传给 [`sub`](#method.sub) 的那个）。以下几种写法都是等价的，前提是其安全前置条件均被满足：
    /// ```rust
    /// # unsafe fn blah(ptr: *mut i32, origin: *mut i32, count: usize) -> bool { unsafe {
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
    /// - 两个指针之间的距离必须非负（`self >= origin`）
    ///
    /// - [`offset_from`](#method.offset_from) 的 *全部* 安全条件同样适用于此方法；完整细节见该方法。
    ///
    /// 重要的是，尽管此方法的返回类型能表示更大的偏移量，但仍 *不允许* 传入相差超过 `isize::MAX`
    /// *字节* 的指针。因此，此方法的结果将始终小于或等于 `isize::MAX as usize`。
    ///
    /// # Panics
    ///
    /// 如果 `T` 是零大小类型（"ZST"），此函数会 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// let mut a = [0; 5];
    /// let p: *mut i32 = a.as_mut_ptr();
    /// unsafe {
    ///     let ptr1: *mut i32 = p.add(1);
    ///     let ptr2: *mut i32 = p.add(3);
    ///
    ///     assert_eq!(ptr2.offset_from_unsigned(ptr1), 2);
    ///     assert_eq!(ptr1.add(2), ptr2);
    ///     assert_eq!(ptr2.sub(2), ptr1);
    ///     assert_eq!(ptr2.offset_from_unsigned(ptr2), 0);
    /// }
    ///
    /// // 下面这样是错误的，因为两个指针顺序不正确：
    /// // ptr1.offset_from(ptr2)
    /// ```
    #[stable(feature = "ptr_sub_ptr", since = "1.87.0")]
    #[rustc_const_stable(feature = "const_ptr_sub_ptr", since = "1.87.0")]
    #[inline]
    #[track_caller]
    pub const unsafe fn offset_from_unsigned(self, origin: *const T) -> usize
    where
        T: Sized,
    {
        // SAFETY: 调用方必须维护 `offset_from_unsigned` 的安全契约。
        unsafe { (self as *const T).offset_from_unsigned(origin) }
    }

    /// 计算同一分配对象内两个指针之间的距离，*前提是已知 `self` 大于或等于 `origin`*。返回值以 **字节**
    /// 为单位。
    ///
    /// 这纯粹是把指针 cast 为 `u8` 指针再在其上使用
    /// [`offset_from_unsigned`][pointer::offset_from_unsigned] 的便捷封装。
    /// 文档与安全要求详见该方法。
    ///
    /// 对于非 `Sized` 的 pointee，此操作只考虑数据指针，忽略 metadata。
    #[stable(feature = "ptr_sub_ptr", since = "1.87.0")]
    #[rustc_const_stable(feature = "const_ptr_sub_ptr", since = "1.87.0")]
    #[inline]
    #[track_caller]
    pub const unsafe fn byte_offset_from_unsigned<U: ?Sized>(self, origin: *mut U) -> usize {
        // SAFETY: 调用方必须维护 `byte_offset_from_unsigned` 的安全契约。
        unsafe { (self as *const T).byte_offset_from_unsigned(origin) }
    }

    #[doc = include_str!("./docs/add.md")]
    ///
    /// # 示例
    ///
    /// ```
    /// let mut s: String = "123".to_string();
    /// let ptr: *mut u8 = s.as_mut_ptr();
    ///
    /// unsafe {
    ///     assert_eq!('2', *ptr.add(1) as char);
    ///     assert_eq!('3', *ptr.add(2) as char);
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

    /// 以字节为单位为指针加上一个无符号偏移量。
    ///
    /// `count` 的单位是字节。
    ///
    /// 这纯粹是把指针 cast 为 `u8` 指针再在其上使用 [add][pointer::add] 的便捷封装。文档与安全要求详见
    /// 该方法。
    ///
    /// 对于非 `Sized` 的 pointee，此操作只改变数据指针，metadata 保持不变。
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
    /// 这只能让指针向后移动（或不移动）。如果你需要根据值来决定向前或向后移动，那么你可能想用接受带符号
    /// 偏移量的 [`offset`](#method.offset)。
    ///
    /// `count` 的单位是 T；例如 `count` 为 3 表示偏移 `3 * size_of::<T>()` 字节。
    ///
    /// # 安全性(Safety）
    ///
    /// 如果违反以下任一条件，结果即为未定义行为：
    ///
    /// * 以字节计的偏移量 `count * size_of::<T>()`，按数学整数计算（不“回绕”），必须能放入一个 `isize`。
    ///
    /// * 如果计算出的偏移量非零，则 `self` 必须[派生自][crate::ptr#provenance]指向某个 [allocation]
    ///   的指针，且 `self` 与结果之间的整个内存范围必须在该分配对象的边界内。特别地，该范围不得“回绕”地址
    ///   空间的边界。
    ///
    /// 分配对象的大小永远不会超过 `isize::MAX` 字节，因此如果计算出的偏移量停留在分配对象的边界内，那么它
    /// 保证满足上述第一个要求。例如，这意味着 `vec.as_ptr().add(vec.len())`（对于 `vec: Vec<T>`）总是
    /// 安全的。
    ///
    /// 如果这些约束难以满足，考虑改用 [`wrapping_sub`]。此方法唯一的优势是它能启用更激进的编译器优化。
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
    ///     assert_eq!('3', *end.sub(1) as char);
    ///     assert_eq!('2', *end.sub(2) as char);
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
            // 当 pointee 是 ZST 时，指针算术不做任何事。
            self
        } else {
            // SAFETY: 调用方必须维护 `offset` 的安全契约。
            // 由于 pointee *不是* ZST，这意味着 `count` 至多为 `isize::MAX`，
            // 因此取负不会溢出。
            unsafe { intrinsics::offset(self, intrinsics::unchecked_sub(0, count as isize)) }
        }
    }

    /// 以字节为单位从指针中减去一个无符号偏移量。
    ///
    /// `count` 的单位是字节。
    ///
    /// 这纯粹是把指针 cast 为 `u8` 指针再在其上使用 [sub][pointer::sub] 的便捷封装。文档与安全要求详见
    /// 该方法。
    ///
    /// 对于非 `Sized` 的 pointee，此操作只改变数据指针，metadata 保持不变。
    #[must_use]
    #[inline(always)]
    #[stable(feature = "pointer_byte_offsets", since = "1.75.0")]
    #[rustc_const_stable(feature = "const_pointer_byte_offsets", since = "1.75.0")]
    #[track_caller]
    pub const unsafe fn byte_sub(self, count: usize) -> Self {
        // SAFETY: 调用方必须维护 `sub` 的安全契约。
        unsafe { self.cast::<u8>().sub(count).with_metadata_of(self) }
    }

    /// 以 wrapping 算术为指针加上一个无符号偏移量。
    ///
    /// `count` 的单位是 T；例如 `count` 为 3 表示偏移 `3 * size_of::<T>()` 字节。
    ///
    /// # 安全性(Safety）
    ///
    /// 此操作本身总是安全的，但使用其结果指针则不然。
    ///
    /// 结果指针会“记住”`self` 指向的那个 [allocation]；它不得用于读写其他分配对象。
    ///
    /// 换言之，`let z = x.wrapping_add((y as usize) - (x as usize))` *不会* 使 `z` 等同于 `y`，
    /// 即便我们假设 `T` 大小为 `1` 且不发生溢出：`z` 仍然附属于 `x` 所附属的对象，除非 `x` 和 `y` 指向
    /// 同一分配对象，否则解引用它就是未定义行为。
    ///
    /// 与 [`add`] 相比，此方法基本上推迟了“停留在同一分配对象内”这一要求：[`add`] 在跨越对象边界时
    /// 立即构成未定义行为；而 `wrapping_add` 会产出一个指针，但如果在指针越出其所附属对象的边界时被
    /// 解引用，仍会导致未定义行为。[`add`] 能被更好地优化，因此在性能敏感的代码中更可取。
    ///
    /// 这个被推迟的检查只考虑被解引用的那个指针值，而不考虑计算最终结果过程中用到的中间值。例如，
    /// `x.wrapping_add(o).wrapping_sub(o)` 总是等于 `x`。换言之，离开分配对象再于稍后重新进入是允许的。
    ///
    /// [`add`]: #method.add
    /// [allocation]: crate::ptr#allocation
    ///
    /// # 示例
    ///
    /// ```
    /// // 以每次两个元素的步长用裸指针进行迭代
    /// let data = [1u8, 2, 3, 4, 5];
    /// let mut ptr: *const u8 = data.as_ptr();
    /// let step = 2;
    /// let end_rounded_up = ptr.wrapping_add(6);
    ///
    /// // 此循环打印 "1, 3, 5, "
    /// while ptr != end_rounded_up {
    ///     unsafe {
    ///         print!("{}, ", *ptr);
    ///     }
    ///     ptr = ptr.wrapping_add(step);
    /// }
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

    /// 以 wrapping 算术、以字节为单位为指针加上一个无符号偏移量。
    ///
    /// `count` 的单位是字节。
    ///
    /// 这纯粹是把指针 cast 为 `u8` 指针再在其上使用 [wrapping_add][pointer::wrapping_add] 的便捷封装。
    /// 文档详见该方法。
    ///
    /// 对于非 `Sized` 的 pointee，此操作只改变数据指针，metadata 保持不变。
    #[must_use]
    #[inline(always)]
    #[stable(feature = "pointer_byte_offsets", since = "1.75.0")]
    #[rustc_const_stable(feature = "const_pointer_byte_offsets", since = "1.75.0")]
    pub const fn wrapping_byte_add(self, count: usize) -> Self {
        self.cast::<u8>().wrapping_add(count).with_metadata_of(self)
    }

    /// 以 wrapping 算术从指针中减去一个无符号偏移量。
    ///
    /// `count` 的单位是 T；例如 `count` 为 3 表示偏移 `3 * size_of::<T>()` 字节。
    ///
    /// # 安全性(Safety）
    ///
    /// 此操作本身总是安全的，但使用其结果指针则不然。
    ///
    /// 结果指针会“记住”`self` 指向的那个 [allocation]；它不得用于读写其他分配对象。
    ///
    /// 换言之，`let z = x.wrapping_sub((x as usize) - (y as usize))` *不会* 使 `z` 等同于 `y`，
    /// 即便我们假设 `T` 大小为 `1` 且不发生溢出：`z` 仍然附属于 `x` 所附属的对象，除非 `x` 和 `y` 指向
    /// 同一分配对象，否则解引用它就是未定义行为。
    ///
    /// 与 [`sub`] 相比，此方法基本上推迟了“停留在同一分配对象内”这一要求：[`sub`] 在跨越对象边界时
    /// 立即构成未定义行为；而 `wrapping_sub` 会产出一个指针，但如果在指针越出其所附属对象的边界时被
    /// 解引用，仍会导致未定义行为。[`sub`] 能被更好地优化，因此在性能敏感的代码中更可取。
    ///
    /// 这个被推迟的检查只考虑被解引用的那个指针值，而不考虑计算最终结果过程中用到的中间值。例如，
    /// `x.wrapping_add(o).wrapping_sub(o)` 总是等于 `x`。换言之，离开分配对象再于稍后重新进入是允许的。
    ///
    /// [`sub`]: #method.sub
    /// [allocation]: crate::ptr#allocation
    ///
    /// # 示例
    ///
    /// ```
    /// // 以每次两个元素的步长（反向）用裸指针进行迭代
    /// let data = [1u8, 2, 3, 4, 5];
    /// let mut ptr: *const u8 = data.as_ptr();
    /// let start_rounded_down = ptr.wrapping_sub(2);
    /// ptr = ptr.wrapping_add(4);
    /// let step = 2;
    /// // 此循环打印 "5, 3, 1, "
    /// while ptr != start_rounded_down {
    ///     unsafe {
    ///         print!("{}, ", *ptr);
    ///     }
    ///     ptr = ptr.wrapping_sub(step);
    /// }
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

    /// 以 wrapping 算术、以字节为单位从指针中减去一个无符号偏移量。
    ///
    /// `count` 的单位是字节。
    ///
    /// 这纯粹是把指针 cast 为 `u8` 指针再在其上使用 [wrapping_sub][pointer::wrapping_sub] 的便捷封装。
    /// 文档详见该方法。
    ///
    /// 对于非 `Sized` 的 pointee，此操作只改变数据指针，metadata 保持不变。
    #[must_use]
    #[inline(always)]
    #[stable(feature = "pointer_byte_offsets", since = "1.75.0")]
    #[rustc_const_stable(feature = "const_pointer_byte_offsets", since = "1.75.0")]
    pub const fn wrapping_byte_sub(self, count: usize) -> Self {
        self.cast::<u8>().wrapping_sub(count).with_metadata_of(self)
    }

    /// 从 `self` 读取值而不移动它。这会让 `self` 处的内存保持不变。
    ///
    /// 安全性相关事项和示例见 [`ptr::read`]。
    ///
    /// [`ptr::read`]: crate::ptr::read()
    #[stable(feature = "pointer_methods", since = "1.26.0")]
    #[rustc_const_stable(feature = "const_ptr_read", since = "1.71.0")]
    #[inline(always)]
    #[track_caller]
    pub const unsafe fn read(self) -> T
    where
        T: Sized,
    {
        // SAFETY: 调用方必须维护 `read` 的安全契约。
        unsafe { read(self) }
    }

    /// 对 `self` 处的值执行一次 volatile 读取而不移动它。这会让 `self` 处的内存保持不变。
    ///
    /// volatile 操作意在作用于 I/O 内存，并且保证不会被编译器消除，也不会被相对于其他 volatile 操作重排。
    ///
    /// 安全性相关事项和示例见 [`ptr::read_volatile`]。
    ///
    /// [`ptr::read_volatile`]: crate::ptr::read_volatile()
    #[stable(feature = "pointer_methods", since = "1.26.0")]
    #[inline(always)]
    #[track_caller]
    pub unsafe fn read_volatile(self) -> T
    where
        T: Sized,
    {
        // SAFETY: 调用方必须维护 `read_volatile` 的安全契约。
        unsafe { read_volatile(self) }
    }

    /// 从 `self` 读取值而不移动它。这会让 `self` 处的内存保持不变。
    ///
    /// 与 `read` 不同，此处的指针可以是未对齐的。
    ///
    /// 安全性相关事项和示例见 [`ptr::read_unaligned`]。
    ///
    /// [`ptr::read_unaligned`]: crate::ptr::read_unaligned()
    #[stable(feature = "pointer_methods", since = "1.26.0")]
    #[rustc_const_stable(feature = "const_ptr_read", since = "1.71.0")]
    #[inline(always)]
    #[track_caller]
    pub const unsafe fn read_unaligned(self) -> T
    where
        T: Sized,
    {
        // SAFETY: 调用方必须维护 `read_unaligned` 的安全契约。
        unsafe { read_unaligned(self) }
    }

    /// 从 `self` 复制 `count * size_of::<T>()` 字节到 `dest`。源与目标可以重叠。
    ///
    /// 注意：这与 [`ptr::copy`] 的参数顺序 *相同*。
    ///
    /// 安全性相关事项和示例见 [`ptr::copy`]。
    ///
    /// [`ptr::copy`]: crate::ptr::copy()
    #[rustc_const_stable(feature = "const_intrinsic_copy", since = "1.83.0")]
    #[stable(feature = "pointer_methods", since = "1.26.0")]
    #[inline(always)]
    #[track_caller]
    pub const unsafe fn copy_to(self, dest: *mut T, count: usize)
    where
        T: Sized,
    {
        // SAFETY: 调用方必须维护 `copy` 的安全契约。
        unsafe { copy(self, dest, count) }
    }

    /// 从 `self` 复制 `count * size_of::<T>()` 字节到 `dest`。源与目标 *不可* 重叠。
    ///
    /// 注意：这与 [`ptr::copy_nonoverlapping`] 的参数顺序 *相同*。
    ///
    /// 安全性相关事项和示例见 [`ptr::copy_nonoverlapping`]。
    ///
    /// [`ptr::copy_nonoverlapping`]: crate::ptr::copy_nonoverlapping()
    #[rustc_const_stable(feature = "const_intrinsic_copy", since = "1.83.0")]
    #[stable(feature = "pointer_methods", since = "1.26.0")]
    #[inline(always)]
    #[track_caller]
    pub const unsafe fn copy_to_nonoverlapping(self, dest: *mut T, count: usize)
    where
        T: Sized,
    {
        // SAFETY: 调用方必须维护 `copy_nonoverlapping` 的安全契约。
        unsafe { copy_nonoverlapping(self, dest, count) }
    }

    /// 从 `src` 复制 `count * size_of::<T>()` 字节到 `self`。源与目标可以重叠。
    ///
    /// 注意：这与 [`ptr::copy`] 的参数顺序 *相反*。
    ///
    /// 安全性相关事项和示例见 [`ptr::copy`]。
    ///
    /// [`ptr::copy`]: crate::ptr::copy()
    #[rustc_const_stable(feature = "const_intrinsic_copy", since = "1.83.0")]
    #[stable(feature = "pointer_methods", since = "1.26.0")]
    #[inline(always)]
    #[track_caller]
    pub const unsafe fn copy_from(self, src: *const T, count: usize)
    where
        T: Sized,
    {
        // SAFETY: 调用方必须维护 `copy` 的安全契约。
        unsafe { copy(src, self, count) }
    }

    /// 从 `src` 复制 `count * size_of::<T>()` 字节到 `self`。源与目标 *不可* 重叠。
    ///
    /// 注意：这与 [`ptr::copy_nonoverlapping`] 的参数顺序 *相反*。
    ///
    /// 安全性相关事项和示例见 [`ptr::copy_nonoverlapping`]。
    ///
    /// [`ptr::copy_nonoverlapping`]: crate::ptr::copy_nonoverlapping()
    #[rustc_const_stable(feature = "const_intrinsic_copy", since = "1.83.0")]
    #[stable(feature = "pointer_methods", since = "1.26.0")]
    #[inline(always)]
    #[track_caller]
    pub const unsafe fn copy_from_nonoverlapping(self, src: *const T, count: usize)
    where
        T: Sized,
    {
        // SAFETY: 调用方必须维护 `copy_nonoverlapping` 的安全契约。
        unsafe { copy_nonoverlapping(src, self, count) }
    }

    /// 执行所指向值的析构函数（如果有的话）。
    ///
    /// 安全性相关事项和示例见 [`ptr::drop_in_place`]。
    ///
    /// [`ptr::drop_in_place`]: crate::ptr::drop_in_place()
    #[stable(feature = "pointer_methods", since = "1.26.0")]
    #[rustc_const_unstable(feature = "const_drop_in_place", issue = "109342")]
    #[inline(always)]
    pub const unsafe fn drop_in_place(self)
    where
        T: [const] Destruct,
    {
        // SAFETY: 调用方必须维护 `drop_in_place` 的安全契约。
        unsafe { drop_in_place(self) }
    }

    /// 用给定值覆写一个内存位置，既不读取也不 drop 旧值。
    ///
    /// 安全性相关事项和示例见 [`ptr::write`]。
    ///
    /// [`ptr::write`]: crate::ptr::write()
    #[stable(feature = "pointer_methods", since = "1.26.0")]
    #[rustc_const_stable(feature = "const_ptr_write", since = "1.83.0")]
    #[inline(always)]
    #[track_caller]
    pub const unsafe fn write(self, val: T)
    where
        T: Sized,
    {
        // SAFETY: 调用方必须维护 `write` 的安全契约。
        unsafe { write(self, val) }
    }

    /// 对指定指针调用 memset，把从 `self` 开始的 `count * size_of::<T>()` 字节内存设置为 `val`。
    ///
    /// 安全性相关事项和示例见 [`ptr::write_bytes`]。
    ///
    /// [`ptr::write_bytes`]: crate::ptr::write_bytes()
    #[doc(alias = "memset")]
    #[stable(feature = "pointer_methods", since = "1.26.0")]
    #[rustc_const_stable(feature = "const_ptr_write", since = "1.83.0")]
    #[inline(always)]
    #[track_caller]
    pub const unsafe fn write_bytes(self, val: u8, count: usize)
    where
        T: Sized,
    {
        // SAFETY: 调用方必须维护 `write_bytes` 的安全契约。
        unsafe { write_bytes(self, val, count) }
    }

    /// 用给定值对一个内存位置执行一次 volatile 写入，既不读取也不 drop 旧值。
    ///
    /// volatile 操作意在作用于 I/O 内存，并且保证不会被编译器消除，也不会被相对于其他 volatile 操作重排。
    ///
    /// 安全性相关事项和示例见 [`ptr::write_volatile`]。
    ///
    /// [`ptr::write_volatile`]: crate::ptr::write_volatile()
    #[stable(feature = "pointer_methods", since = "1.26.0")]
    #[inline(always)]
    #[track_caller]
    pub unsafe fn write_volatile(self, val: T)
    where
        T: Sized,
    {
        // SAFETY: 调用方必须维护 `write_volatile` 的安全契约。
        unsafe { write_volatile(self, val) }
    }

    /// 用给定值覆写一个内存位置，既不读取也不 drop 旧值。
    ///
    /// 与 `write` 不同，此处的指针可以是未对齐的。
    ///
    /// 安全性相关事项和示例见 [`ptr::write_unaligned`]。
    ///
    /// [`ptr::write_unaligned`]: crate::ptr::write_unaligned()
    #[stable(feature = "pointer_methods", since = "1.26.0")]
    #[rustc_const_stable(feature = "const_ptr_write", since = "1.83.0")]
    #[inline(always)]
    #[track_caller]
    pub const unsafe fn write_unaligned(self, val: T)
    where
        T: Sized,
    {
        // SAFETY: 调用方必须维护 `write_unaligned` 的安全契约。
        unsafe { write_unaligned(self, val) }
    }

    /// 用 `src` 替换 `self` 处的值，返回旧值，且二者都不 drop。
    ///
    /// 安全性相关事项和示例见 [`ptr::replace`]。
    ///
    /// [`ptr::replace`]: crate::ptr::replace()
    #[stable(feature = "pointer_methods", since = "1.26.0")]
    #[rustc_const_stable(feature = "const_inherent_ptr_replace", since = "1.88.0")]
    #[inline(always)]
    pub const unsafe fn replace(self, src: T) -> T
    where
        T: Sized,
    {
        // SAFETY: 调用方必须维护 `replace` 的安全契约。
        unsafe { replace(self, src) }
    }

    /// 交换两个同类型可变位置上的值，且二者都不会被反初始化。与在其余方面等价的 `mem::swap` 不同，
    /// 这两个位置可以重叠。
    ///
    /// 安全性相关事项和示例见 [`ptr::swap`]。
    ///
    /// [`ptr::swap`]: crate::ptr::swap()
    #[stable(feature = "pointer_methods", since = "1.26.0")]
    #[rustc_const_stable(feature = "const_swap", since = "1.85.0")]
    #[inline(always)]
    pub const unsafe fn swap(self, with: *mut T)
    where
        T: Sized,
    {
        // SAFETY: 调用方必须维护 `swap` 的安全契约。
        unsafe { swap(self, with) }
    }

    /// 计算为使指针对齐到 `align` 所需施加的偏移量。
    ///
    /// 如果无法使该指针对齐，实现会返回 `usize::MAX`。
    ///
    /// 偏移量以 `T` 元素的个数表示，而非字节。返回的值可与 `wrapping_add` 方法一起使用。
    ///
    /// 此处完全不保证偏移该指针不会溢出，也不保证不会越出该指针所指向的分配对象。确保返回的偏移量在对齐
    /// 以外的所有方面都正确，是调用方的责任。
    ///
    /// # Panics
    ///
    /// 如果 `align` 不是 2 的幂，此函数会 panic。
    ///
    /// # 示例
    ///
    /// 把相邻的 `u8` 当作 `u16` 访问
    ///
    /// ```
    /// # unsafe {
    /// let mut x = [5_u8, 6, 7, 8, 9];
    /// let ptr = x.as_mut_ptr();
    /// let offset = ptr.align_offset(align_of::<u16>());
    ///
    /// if offset < x.len() - 1 {
    ///     let u16_ptr = ptr.add(offset).cast::<u16>();
    ///     *u16_ptr = 0;
    ///
    ///     assert!(x == [0, 0, 7, 8, 9] || x == [5, 0, 0, 8, 9]);
    /// } else {
    ///     // 虽然该指针可以通过 `offset` 对齐，但那样它会指向
    ///     // 分配对象之外
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

        // SAFETY: 上面已检查 `align` 是 2 的幂
        let ret = unsafe { align_offset(self, align) };

        // Inform Miri that we want to consider the resulting pointer to be suitably aligned.
        // 告知 Miri 我们希望把结果指针视为已适当对齐。
        #[cfg(miri)]
        if ret != usize::MAX {
            intrinsics::miri_promise_symbolic_alignment(
                self.wrapping_add(ret).cast_const().cast(),
                align,
            );
        }

        ret
    }

    /// 返回该指针对于 `T` 而言是否已正确对齐。
    ///
    /// # 示例
    ///
    /// ```
    /// // 在某些平台上，i32 的对齐小于 4。
    /// #[repr(align(4))]
    /// struct AlignedI32(i32);
    ///
    /// let mut data = AlignedI32(42);
    /// let ptr = &mut data as *mut AlignedI32;
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
    /// 对于非 `Sized` 的 pointee，此操作只考虑数据指针，忽略 metadata。
    ///
    /// # Panics
    ///
    /// 如果 `align` 不是 2 的幂（这包括 0），此函数会 panic。
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
    /// let mut data = AlignedI32(42);
    /// let ptr = &mut data as *mut AlignedI32;
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

impl<T> *mut T {
    /// 从某类型转换为其 maybe-uninitialized（可能未初始化）的版本。
    ///
    /// 这总是安全的，因为只有在指针被初始化前就被读取时才会发生 UB。
    #[must_use]
    #[inline(always)]
    #[unstable(feature = "cast_maybe_uninit", issue = "145036")]
    pub const fn cast_uninit(self) -> *mut MaybeUninit<T> {
        self as _
    }
}
impl<T> *mut MaybeUninit<T> {
    /// 从 maybe-uninitialized（可能未初始化）的类型转换为其已初始化的版本。
    ///
    /// 这总是安全的，因为只有在指针被初始化前就被读取时才会发生 UB。
    #[must_use]
    #[inline(always)]
    #[unstable(feature = "cast_maybe_uninit", issue = "145036")]
    pub const fn cast_init(self) -> *mut T {
        self as _
    }
}

impl<T> *mut [T] {
    /// 返回裸切片的长度。
    ///
    /// 返回值是 **元素** 的个数，而不是字节数。
    ///
    /// 此函数是安全的，即便该裸切片因指针为空或未对齐而无法被 cast 为切片引用也是如此。
    ///
    /// # 示例
    ///
    /// ```rust
    /// use std::ptr;
    ///
    /// let slice: *mut [i8] = ptr::slice_from_raw_parts_mut(ptr::null_mut(), 3);
    /// assert_eq!(slice.len(), 3);
    /// ```
    #[inline(always)]
    #[stable(feature = "slice_ptr_len", since = "1.79.0")]
    #[rustc_const_stable(feature = "const_slice_ptr_len", since = "1.79.0")]
    pub const fn len(self) -> usize {
        metadata(self)
    }

    /// 如果裸切片的长度为 0 则返回 `true`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::ptr;
    ///
    /// let slice: *mut [i8] = ptr::slice_from_raw_parts_mut(ptr::null_mut(), 3);
    /// assert!(!slice.is_empty());
    /// ```
    #[inline(always)]
    #[stable(feature = "slice_ptr_len", since = "1.79.0")]
    #[rustc_const_stable(feature = "const_slice_ptr_len", since = "1.79.0")]
    pub const fn is_empty(self) -> bool {
        self.len() == 0
    }

    /// 获取指向底层数组的裸可变指针。
    ///
    /// 如果 `N` 不恰好等于 `self` 的长度，则此方法返回 `None`。
    #[stable(feature = "core_slice_as_array", since = "1.93.0")]
    #[rustc_const_stable(feature = "core_slice_as_array", since = "1.93.0")]
    #[inline]
    #[must_use]
    pub const fn as_mut_array<const N: usize>(self) -> Option<*mut [T; N]> {
        if self.len() == N {
            let me = self.as_mut_ptr() as *mut [T; N];
            Some(me)
        } else {
            None
        }
    }

    /// 在某个索引处把一个可变裸切片一分为二。
    ///
    /// 第一个将包含 `[0, mid)` 范围内的所有索引（不含索引 `mid` 本身），第二个将包含 `[mid, len)`
    /// 范围内的所有索引（不含索引 `len` 本身）。
    ///
    /// # Panics
    ///
    /// 如果 `mid > len` 则 panic。
    ///
    /// # 安全性(Safety）
    ///
    /// `mid` 必须在底层 [allocation] 的 [in-bounds] 范围内。这意味着 `self` 必须可解引用，且要跨越一个
    /// 单一分配对象、该对象至少有 `mid * size_of::<T>()` 字节长。不满足这些要求即为 *[undefined behavior]*，
    /// 即便结果指针未被使用。
    ///
    /// 由于 `len` 是否在边界内并不是 `*mut [T]` 的安全不变量，本方法的安全要求与 [`split_at_mut_unchecked`]
    /// 相同。显式的边界检查只在 `len` 正确时才有用。
    ///
    /// [`split_at_mut_unchecked`]: #method.split_at_mut_unchecked
    /// [in-bounds]: #method.add
    /// [allocation]: crate::ptr#allocation
    /// [undefined behavior]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(raw_slice_split)]
    /// #![feature(slice_ptr_get)]
    ///
    /// let mut v = [1, 0, 3, 0, 5, 6];
    /// let ptr = &mut v as *mut [_];
    /// unsafe {
    ///     let (left, right) = ptr.split_at_mut(2);
    ///     assert_eq!(&*left, [1, 0]);
    ///     assert_eq!(&*right, [3, 0, 5, 6]);
    /// }
    /// ```
    #[inline(always)]
    #[track_caller]
    #[unstable(feature = "raw_slice_split", issue = "95595")]
    pub unsafe fn split_at_mut(self, mid: usize) -> (*mut [T], *mut [T]) {
        assert!(mid <= self.len());
        // SAFETY: The assert above is only a safety-net as long as `self.len()` is correct
        // The actual safety requirements of this function are the same as for `split_at_mut_unchecked`
        // 只要 `self.len()` 正确，上面的 assert 就只是一道安全网。
        // 本函数实际的安全要求与 `split_at_mut_unchecked` 相同。
        unsafe { self.split_at_mut_unchecked(mid) }
    }

    /// 在某个索引处把一个可变裸切片一分为二，不做边界检查。
    ///
    /// 第一个将包含 `[0, mid)` 范围内的所有索引（不含索引 `mid` 本身），第二个将包含 `[mid, len)`
    /// 范围内的所有索引（不含索引 `len` 本身）。
    ///
    /// # 安全性(Safety）
    ///
    /// `mid` 必须在底层 [allocation] 的 [in-bounds] 范围内。这意味着 `self` 必须可解引用，且要跨越一个
    /// 单一分配对象、该对象至少有 `mid * size_of::<T>()` 字节长。不满足这些要求即为 *[undefined behavior]*，
    /// 即便结果指针未被使用。
    ///
    /// [in-bounds]: #method.add
    /// [out-of-bounds index]: #method.add
    /// [allocation]: crate::ptr#allocation
    /// [undefined behavior]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(raw_slice_split)]
    ///
    /// let mut v = [1, 0, 3, 0, 5, 6];
    /// // 用作用域来限制这些借用的生命周期
    /// unsafe {
    ///     let ptr = &mut v as *mut [_];
    ///     let (left, right) = ptr.split_at_mut_unchecked(2);
    ///     assert_eq!(&*left, [1, 0]);
    ///     assert_eq!(&*right, [3, 0, 5, 6]);
    ///     (&mut *left)[1] = 2;
    ///     (&mut *right)[1] = 4;
    /// }
    /// assert_eq!(v, [1, 2, 3, 4, 5, 6]);
    /// ```
    #[inline(always)]
    #[unstable(feature = "raw_slice_split", issue = "95595")]
    pub unsafe fn split_at_mut_unchecked(self, mid: usize) -> (*mut [T], *mut [T]) {
        let len = self.len();
        let ptr = self.as_mut_ptr();

        // SAFETY: Caller must pass a valid pointer and an index that is in-bounds.
        // SAFETY: 调用方必须传入一个有效指针和一个在边界内的索引。
        let tail = unsafe { ptr.add(mid) };
        (
            crate::ptr::slice_from_raw_parts_mut(ptr, mid),
            crate::ptr::slice_from_raw_parts_mut(tail, len - mid),
        )
    }

    /// Returns a raw pointer to the slice's buffer.
    ///
    /// This is equivalent to casting `self` to `*mut T`, but more type-safe.
    ///
    /// # Examples
    ///
    /// ```rust
    /// #![feature(slice_ptr_get)]
    /// use std::ptr;
    ///
    /// let slice: *mut [i8] = ptr::slice_from_raw_parts_mut(ptr::null_mut(), 3);
    /// assert_eq!(slice.as_mut_ptr(), ptr::null_mut());
    /// ```
    #[inline(always)]
    #[unstable(feature = "slice_ptr_get", issue = "74265")]
    pub const fn as_mut_ptr(self) -> *mut T {
        self as *mut T
    }

    /// Returns a raw pointer to an element or subslice, without doing bounds
    /// checking.
    ///
    /// Calling this method with an [out-of-bounds index] or when `self` is not dereferenceable
    /// is *[undefined behavior]* even if the resulting pointer is not used.
    ///
    /// [out-of-bounds index]: #method.add
    /// [undefined behavior]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(slice_ptr_get)]
    ///
    /// let x = &mut [1, 2, 4] as *mut [i32];
    ///
    /// unsafe {
    ///     assert_eq!(x.get_unchecked_mut(1), x.as_mut_ptr().add(1));
    /// }
    /// ```
    #[unstable(feature = "slice_ptr_get", issue = "74265")]
    #[rustc_const_unstable(feature = "const_index", issue = "143775")]
    #[inline(always)]
    pub const unsafe fn get_unchecked_mut<I>(self, index: I) -> *mut I::Output
    where
        I: [const] SliceIndex<[T]>,
    {
        // SAFETY: the caller ensures that `self` is dereferenceable and `index` in-bounds.
        unsafe { index.get_unchecked_mut(self) }
    }

    #[doc = include_str!("docs/as_uninit_slice.md")]
    ///
    /// # See Also
    /// For the mutable counterpart see [`as_uninit_slice_mut`](pointer::as_uninit_slice_mut).
    #[inline]
    #[unstable(feature = "ptr_as_uninit", issue = "75402")]
    pub const unsafe fn as_uninit_slice<'a>(self) -> Option<&'a [MaybeUninit<T>]> {
        if self.is_null() {
            None
        } else {
            // SAFETY: the caller must uphold the safety contract for `as_uninit_slice`.
            Some(unsafe { slice::from_raw_parts(self as *const MaybeUninit<T>, self.len()) })
        }
    }

    /// Returns `None` if the pointer is null, or else returns a unique slice to
    /// the value wrapped in `Some`. In contrast to [`as_mut`], this does not require
    /// that the value has to be initialized.
    ///
    /// For the shared counterpart see [`as_uninit_slice`].
    ///
    /// [`as_mut`]: #method.as_mut
    /// [`as_uninit_slice`]: #method.as_uninit_slice-1
    ///
    /// # Safety
    ///
    /// When calling this method, you have to ensure that *either* the pointer is null *or*
    /// all of the following is true:
    ///
    /// * The pointer must be [valid] for reads and writes for `ptr.len() * size_of::<T>()`
    ///   many bytes, and it must be properly aligned. This means in particular:
    ///
    ///     * The entire memory range of this slice must be contained within a single [allocation]!
    ///       Slices can never span across multiple allocations.
    ///
    ///     * The pointer must be aligned even for zero-length slices. One
    ///       reason for this is that enum layout optimizations may rely on references
    ///       (including slices of any length) being aligned and non-null to distinguish
    ///       them from other data. You can obtain a pointer that is usable as `data`
    ///       for zero-length slices using [`NonNull::dangling()`].
    ///
    /// * The total size `ptr.len() * size_of::<T>()` of the slice must be no larger than `isize::MAX`.
    ///   See the safety documentation of [`pointer::offset`].
    ///
    /// * You must enforce Rust's aliasing rules, since the returned lifetime `'a` is
    ///   arbitrarily chosen and does not necessarily reflect the actual lifetime of the data.
    ///   In particular, while this reference exists, the memory the pointer points to must
    ///   not get accessed (read or written) through any other pointer.
    ///
    /// This applies even if the result of this method is unused!
    ///
    /// See also [`slice::from_raw_parts_mut`][].
    ///
    /// [valid]: crate::ptr#safety
    /// [allocation]: crate::ptr#allocation
    ///
    /// # Panics during const evaluation
    ///
    /// This method will panic during const evaluation if the pointer cannot be
    /// determined to be null or not. See [`is_null`] for more information.
    ///
    /// [`is_null`]: #method.is_null-1
    #[inline]
    #[unstable(feature = "ptr_as_uninit", issue = "75402")]
    pub const unsafe fn as_uninit_slice_mut<'a>(self) -> Option<&'a mut [MaybeUninit<T>]> {
        if self.is_null() {
            None
        } else {
            // SAFETY: the caller must uphold the safety contract for `as_uninit_slice_mut`.
            Some(unsafe { slice::from_raw_parts_mut(self as *mut MaybeUninit<T>, self.len()) })
        }
    }
}

impl<T> *mut T {
    /// Casts from a pointer-to-`T` to a pointer-to-`[T; N]`.
    #[inline]
    #[unstable(feature = "ptr_cast_array", issue = "144514")]
    pub const fn cast_array<const N: usize>(self) -> *mut [T; N] {
        self.cast()
    }
}

impl<T, const N: usize> *mut [T; N] {
    /// Returns a raw pointer to the array's buffer.
    ///
    /// This is equivalent to casting `self` to `*mut T`, but more type-safe.
    ///
    /// # Examples
    ///
    /// ```rust
    /// #![feature(array_ptr_get)]
    /// use std::ptr;
    ///
    /// let arr: *mut [i8; 3] = ptr::null_mut();
    /// assert_eq!(arr.as_mut_ptr(), ptr::null_mut());
    /// ```
    #[inline]
    #[unstable(feature = "array_ptr_get", issue = "119834")]
    pub const fn as_mut_ptr(self) -> *mut T {
        self as *mut T
    }

    /// Returns a raw pointer to a mutable slice containing the entire array.
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(array_ptr_get)]
    ///
    /// let mut arr = [1, 2, 5];
    /// let ptr: *mut [i32; 3] = &mut arr;
    /// unsafe {
    ///     (&mut *ptr.as_mut_slice())[..2].copy_from_slice(&[3, 4]);
    /// }
    /// assert_eq!(arr, [3, 4, 5]);
    /// ```
    #[inline]
    #[unstable(feature = "array_ptr_get", issue = "119834")]
    pub const fn as_mut_slice(self) -> *mut [T] {
        self
    }
}

/// Pointer equality is by address, as produced by the [`<*mut T>::addr`](pointer::addr) method.
#[stable(feature = "rust1", since = "1.0.0")]
#[diagnostic::on_const(
    message = "pointers cannot be reliably compared during const eval",
    note = "see issue #53020 <https://github.com/rust-lang/rust/issues/53020> for more information"
)]
impl<T: PointeeSized> PartialEq for *mut T {
    #[inline(always)]
    #[allow(ambiguous_wide_pointer_comparisons)]
    fn eq(&self, other: &*mut T) -> bool {
        *self == *other
    }
}

/// Pointer equality is an equivalence relation.
#[stable(feature = "rust1", since = "1.0.0")]
#[diagnostic::on_const(
    message = "pointers cannot be reliably compared during const eval",
    note = "see issue #53020 <https://github.com/rust-lang/rust/issues/53020> for more information"
)]
impl<T: PointeeSized> Eq for *mut T {}

/// Pointer comparison is by address, as produced by the [`<*mut T>::addr`](pointer::addr) method.
#[stable(feature = "rust1", since = "1.0.0")]
#[diagnostic::on_const(
    message = "pointers cannot be reliably compared during const eval",
    note = "see issue #53020 <https://github.com/rust-lang/rust/issues/53020> for more information"
)]
impl<T: PointeeSized> Ord for *mut T {
    #[inline]
    #[allow(ambiguous_wide_pointer_comparisons)]
    fn cmp(&self, other: &*mut T) -> Ordering {
        if self < other {
            Less
        } else if self == other {
            Equal
        } else {
            Greater
        }
    }
}

/// Pointer comparison is by address, as produced by the [`<*mut T>::addr`](pointer::addr) method.
#[stable(feature = "rust1", since = "1.0.0")]
#[diagnostic::on_const(
    message = "pointers cannot be reliably compared during const eval",
    note = "see issue #53020 <https://github.com/rust-lang/rust/issues/53020> for more information"
)]
impl<T: PointeeSized> PartialOrd for *mut T {
    #[inline(always)]
    #[allow(ambiguous_wide_pointer_comparisons)]
    fn partial_cmp(&self, other: &*mut T) -> Option<Ordering> {
        Some(self.cmp(other))
    }

    #[inline(always)]
    #[allow(ambiguous_wide_pointer_comparisons)]
    fn lt(&self, other: &*mut T) -> bool {
        *self < *other
    }

    #[inline(always)]
    #[allow(ambiguous_wide_pointer_comparisons)]
    fn le(&self, other: &*mut T) -> bool {
        *self <= *other
    }

    #[inline(always)]
    #[allow(ambiguous_wide_pointer_comparisons)]
    fn gt(&self, other: &*mut T) -> bool {
        *self > *other
    }

    #[inline(always)]
    #[allow(ambiguous_wide_pointer_comparisons)]
    fn ge(&self, other: &*mut T) -> bool {
        *self >= *other
    }
}

#[stable(feature = "raw_ptr_default", since = "1.88.0")]
impl<T: ?Sized + Thin> Default for *mut T {
    /// Returns the default value of [`null_mut()`][crate::ptr::null_mut].
    fn default() -> Self {
        crate::ptr::null_mut()
    }
}
