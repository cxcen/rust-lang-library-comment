// 对此文件看似无关紧要的代码改动也可能对编译时间产生可测量的性能影响；
// 至少部分原因是 layout 代码会被各种集合类型的许多实例化调用，导致必须多次优化掉多余 IR。
// 仅凭性能直觉不可靠。请运行 perf。

use crate::error::Error;
use crate::intrinsics::{unchecked_add, unchecked_mul, unchecked_sub};
use crate::mem::SizedTypeProperties;
use crate::ptr::{Alignment, NonNull};
use crate::{assert_unsafe_precondition, fmt, mem};

/// 一块内存的 Layout。
///
/// `Layout` 实例描述某种特定内存布局。
/// 构造 `Layout` 后可将其作为输入传给 allocator。
///
/// 所有 layout 都有关联的大小和二的幂对齐值。大小向上取整到最接近的 `align`
/// 倍数时不会溢出 `isize`（也就是说，取整后的值始终小于或等于 `isize::MAX`）。
///
/// （注意，layout *不*要求大小非零，尽管 `GlobalAlloc` 要求所有内存请求的大小都非零。
/// 调用者必须自行确保满足这类条件，或使用要求更宽松的特定 allocator，
/// 或使用更宽松的 `Allocator` 接口。）
#[stable(feature = "alloc_layout", since = "1.28.0")]
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
#[lang = "alloc_layout"]
pub struct Layout {
    // 请求的内存块大小，以字节为单位。
    size: usize,

    // 请求的内存块对齐值，以字节为单位。
    // 保证它始终是二的幂，因为 `posix_memalign` 等 API 要求如此，
    // 这也是对 Layout 构造器施加的合理约束。
    //
    // （不过，虽然 `posix_memalign` 同样要求 `align >= sizeof(void*)`，
    //  这里不会类比地施加该要求。）
    align: Alignment,
}

impl Layout {
    /// 根据给定的 `size` 和 `align` 构造 `Layout`；如果不满足以下任一条件，
    /// 则返回 `LayoutError`：
    ///
    /// * `align` 不能为零，
    ///
    /// * `align` 必须是二的幂，
    ///
    /// * `size` 向上取整到最接近的 `align` 倍数时不能溢出 `isize`
    ///   （也就是说，取整后的值必须小于或等于 `isize::MAX`）。
    #[stable(feature = "alloc_layout", since = "1.28.0")]
    #[rustc_const_stable(feature = "const_alloc_layout_size_align", since = "1.50.0")]
    #[inline]
    pub const fn from_size_align(size: usize, align: usize) -> Result<Self, LayoutError> {
        if Layout::is_size_align_valid(size, align) {
            // SAFETY: Layout::is_size_align_valid 已检查此调用所需的前置条件。
            unsafe { Ok(Layout { size, align: mem::transmute(align) }) }
        } else {
            Err(LayoutError)
        }
    }

    #[inline]
    const fn is_size_align_valid(size: usize, align: usize) -> bool {
        let Some(align) = Alignment::new(align) else { return false };
        if size > Self::max_size_for_align(align) {
            return false;
        }
        true
    }

    #[inline(always)]
    const fn max_size_for_align(align: Alignment) -> usize {
        // （二的幂意味着 align != 0。）

        // 向上取整后的大小为：
        //   size_rounded_up = (size + align - 1) & !(align - 1);
        //
        // 从上面可知 align != 0。如果加上 (align - 1) 不溢出，
        // 则向上取整也没问题。
        //
        // 反过来，用 !(align - 1) 做 & 掩码只会减去低位 bit。
        // 因此如果求和已经溢出，该 & 掩码不可能减去足够多的值来撤销该溢出。
        //
        // 由此可知，检查求和是否溢出既是必要条件也是充分条件。

        // SAFETY: 最大可能对齐值是 `isize::MAX + 1`，因此该减法不会溢出。
        unsafe { unchecked_sub(isize::MAX as usize + 1, align.as_usize()) }
    }

    /// 内部辅助构造器，用于跳过对齐有效性的重复校验。
    #[inline]
    const fn from_size_alignment(size: usize, align: Alignment) -> Result<Self, LayoutError> {
        if size > Self::max_size_for_align(align) {
            return Err(LayoutError);
        }

        // SAFETY: 上面已检查 Layout::size 的不变量。
        Ok(Layout { size, align })
    }

    /// 创建 layout，绕过所有检查。
    ///
    /// # 安全性(Safety）
    ///
    /// 此函数不会验证 [`Layout::from_size_align`] 的前置条件，因此是不安全的。
    /// 调用者必须保证 `align` 非零且为二的幂，并且 `size` 按 `align` 向上取整后
    /// 不超过 `isize::MAX`。
    #[stable(feature = "alloc_layout", since = "1.28.0")]
    #[rustc_const_stable(feature = "const_alloc_layout_unchecked", since = "1.36.0")]
    #[must_use]
    #[inline]
    #[track_caller]
    pub const unsafe fn from_size_align_unchecked(size: usize, align: usize) -> Self {
        assert_unsafe_precondition!(
            check_library_ub,
            "Layout::from_size_align_unchecked requires that align is a power of 2 \
            and the rounded-up allocation size does not exceed isize::MAX",
            (
                size: usize = size,
                align: usize = align,
            ) => Layout::is_size_align_valid(size, align)
        );
        // SAFETY: 调用者必须保证这些前置条件成立。
        unsafe { Layout { size, align: mem::transmute(align) } }
    }

    /// 此 layout 的内存块所需的最小字节数。
    #[stable(feature = "alloc_layout", since = "1.28.0")]
    #[rustc_const_stable(feature = "const_alloc_layout_size_align", since = "1.50.0")]
    #[must_use]
    #[inline]
    pub const fn size(&self) -> usize {
        self.size
    }

    /// 此 layout 的内存块所需的最小字节对齐值。
    ///
    /// 返回的对齐值保证是二的幂。
    #[stable(feature = "alloc_layout", since = "1.28.0")]
    #[rustc_const_stable(feature = "const_alloc_layout_size_align", since = "1.50.0")]
    #[must_use = "this returns the minimum alignment, \
                  without modifying the layout"]
    #[inline]
    pub const fn align(&self) -> usize {
        self.align.as_usize()
    }

    /// 构造适合容纳 `T` 类型值的 `Layout`。
    #[stable(feature = "alloc_layout", since = "1.28.0")]
    #[rustc_const_stable(feature = "alloc_layout_const_new", since = "1.42.0")]
    #[must_use]
    #[inline]
    pub const fn new<T>() -> Self {
        <T as SizedTypeProperties>::LAYOUT
    }

    /// 生成描述某条记录的 layout，可用于为 `T` 分配后备结构
    /// （`T` 可以是 trait 或 slice 等其他 unsized 类型）。
    #[stable(feature = "alloc_layout", since = "1.28.0")]
    #[rustc_const_stable(feature = "const_alloc_layout", since = "1.85.0")]
    #[must_use]
    #[inline]
    pub const fn for_value<T: ?Sized>(t: &T) -> Self {
        let (size, align) = (size_of_val(t), align_of_val(t));
        // SAFETY: 这里使用 unsafe 变体的理由见 `new` 中的说明。
        unsafe { Layout::from_size_align_unchecked(size, align) }
    }

    /// 生成描述某条记录的 layout，可用于为 `T` 分配后备结构
    /// （`T` 可以是 trait 或 slice 等其他 unsized 类型）。
    ///
    /// # 安全性(Safety）
    ///
    /// 只有满足以下条件时，调用此函数才是安全的：
    ///
    /// - 如果 `T` 是 `Sized`，此函数始终可以安全调用。
    /// - 如果 `T` 的 unsized 尾部是：
    ///     - [slice]，则 slice 尾部长度必须是已初始化的整数，且*整个值*的大小
    ///       （动态尾部长度 + 静态大小前缀）必须能放入 `isize`。
    ///       在动态尾部长度为 0 的特殊情况下，此函数可以安全调用。
    ///     - [trait object]，则指针的 vtable 部分必须指向通过 unsizing coercion
    ///       为类型 `T` 取得的有效 vtable，且*整个值*的大小
    ///       （动态尾部长度 + 静态大小前缀）必须能放入 `isize`。
    ///     - （不稳定的）[extern type]，则此函数始终可以安全调用，但由于 extern type
    ///       的 layout 未知，它可能 panic 或以其他方式返回错误值。这与
    ///       [`Layout::for_value`] 作用于带 extern type 尾部的引用时的行为相同。
    ///     - 其他情况，则保守地不允许调用此函数。
    ///
    /// [trait object]: ../../book/ch17-02-trait-objects.html
    /// [extern type]: ../../unstable-book/language-features/extern-types.html
    #[unstable(feature = "layout_for_ptr", issue = "69835")]
    #[must_use]
    pub const unsafe fn for_value_raw<T: ?Sized>(t: *const T) -> Self {
        // SAFETY: 这些函数的前置条件已作为本函数的安全前置条件转交给调用者。
        let (size, align) = unsafe { (mem::size_of_val_raw(t), mem::align_of_val_raw(t)) };
        // SAFETY: 这里使用 unsafe 变体的理由见 `new` 中的说明。
        unsafe { Layout::from_size_align_unchecked(size, align) }
    }

    /// 创建一个 dangling 但对此 Layout 对齐良好的 `NonNull`。
    ///
    /// 注意，返回指针的地址可能恰好也是某个有效指针的地址，因此不能把它用作
    /// “尚未初始化”的哨兵值。惰性分配的类型必须用其他方式跟踪初始化状态。
    #[unstable(feature = "alloc_layout_extra", issue = "55724")]
    #[must_use]
    #[inline]
    pub const fn dangling(&self) -> NonNull<u8> {
        NonNull::without_provenance(self.align.as_nonzero())
    }

    /// 创建一个描述记录的 layout：它能容纳与 `self` 相同 layout 的值，
    /// 同时还满足 `align`（以字节为单位）的对齐要求。
    ///
    /// 如果 `self` 已经满足指定对齐，则返回 `self`。
    ///
    /// 注意，无论返回的 layout 是否具有不同的对齐值，此方法都不会给总大小添加任何 padding。
    /// 换言之，如果 `K` 的大小为 16，`K.align_to(32)` 的大小*仍然*是 16。
    ///
    /// 如果 `self.size()` 与给定 `align` 的组合违反 [`Layout::from_size_align`]
    /// 中列出的条件，则返回错误。
    #[stable(feature = "alloc_layout_manipulation", since = "1.44.0")]
    #[rustc_const_stable(feature = "const_alloc_layout", since = "1.85.0")]
    #[inline]
    pub const fn align_to(&self, align: usize) -> Result<Self, LayoutError> {
        if let Some(align) = Alignment::new(align) {
            Layout::from_size_alignment(self.size, Alignment::max(self.align, align))
        } else {
            Err(LayoutError)
        }
    }

    /// 返回必须在 `self` 之后插入多少 padding，才能确保后续地址满足 `align`
    ///（以字节为单位）。
    ///
    /// 例如，如果 `self.size()` 为 9，则 `self.padding_needed_for(4)` 返回 3，
    /// 因为这是取得 4 对齐地址所需的最少 padding 字节数
    /// （假设对应内存块从 4 对齐地址开始）。
    ///
    /// 如果 `align` 不是二的幂，此函数的返回值没有意义。
    ///
    /// 注意，若要让返回值有用，`align` 必须小于或等于整个已分配内存块起始地址的对齐值。
    /// 满足此约束的一种方式是确保 `align <= self.align()`。
    #[unstable(feature = "alloc_layout_extra", issue = "55724")]
    #[must_use = "this returns the padding needed, \
                  without modifying the `Layout`"]
    #[inline]
    pub const fn padding_needed_for(&self, align: usize) -> usize {
        // FIXME: 这里能否直接把类型改成 `Alignment`？
        let Some(align) = Alignment::new(align) else { return usize::MAX };
        let len_rounded_up = self.size_rounded_up_to_custom_align(align);
        // SAFETY: 不会溢出，因为向上取整后的值永远不会小于原大小。
        unsafe { unchecked_sub(len_rounded_up, self.size) }
    }

    /// 返回大于或等于 `self.size()` 的最小 `align` 倍数。
    ///
    /// 因为原始大小至多为 `isize::MAX`，所以此函数返回值至多为
    /// `Alignment::MAX`（也就是 `isize::MAX + 1`）。
    #[inline]
    const fn size_rounded_up_to_custom_align(&self, align: Alignment) -> usize {
        // SAFETY:
        // 向上取整后的值为：
        //   size_rounded_up = (size + align - 1) & !(align - 1);
        //
        // 这里执行的算术永远不会溢出：
        //
        // 1. align 保证 > 0，因此 align - 1 始终有效。
        //
        // 2. size 至多为 `isize::MAX`，因此加上 `align - 1`
        //    （最大为 `isize::MAX`）永远不会溢出 `usize`。
        //
        // 3. 按对齐值做掩码最多移除 `align - 1`，也就是刚刚加上的值，
        //    因此返回值永远不会小于原始 `size`。
        //
        // （Size 0 Align MAX 已经对齐，所以保持不变；但 Size 1 Align MAX 或
        // Size isize::MAX Align 2 这类情况会向上取整到 `isize::MAX + 1`。）
        unsafe {
            let align_m1 = unchecked_sub(align.as_usize(), 1);
            unchecked_add(self.size, align_m1) & !align_m1
        }
    }

    /// 通过将此 layout 的大小向上取整到其对齐值的倍数来创建 layout。
    ///
    /// 这等价于把 `padding_needed_for` 的结果加到 layout 的当前大小上。
    #[stable(feature = "alloc_layout_manipulation", since = "1.44.0")]
    #[rustc_const_stable(feature = "const_alloc_layout", since = "1.85.0")]
    #[must_use = "this returns a new `Layout`, \
                  without modifying the original"]
    #[inline]
    pub const fn pad_to_align(&self) -> Layout {
        // 这不会溢出。引用 Layout 的不变量：
        // > `size` 向上取整到最接近的 `align` 倍数时不能溢出 isize
        // >（也就是说，取整后的值必须小于或等于 `isize::MAX`）
        let new_size = self.size_rounded_up_to_custom_align(self.align);

        // SAFETY: padding 后的大小保证不会超过 `isize::MAX`。
        unsafe { Layout::from_size_align_unchecked(new_size, self.align()) }
    }

    /// 创建一个描述 `self` 的 `n` 个实例的记录 layout，并在实例之间加入合适数量的
    /// padding，以确保每个实例都有请求的大小和对齐。成功时返回 `(k, offs)`，
    /// 其中 `k` 是数组的 layout，`offs` 是数组中每个元素起始位置之间的距离。
    ///
    /// （元素之间的这个距离有时称为 “stride”。）
    ///
    /// 算术溢出时返回 `LayoutError`。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(alloc_layout_extra)]
    /// use std::alloc::Layout;
    ///
    /// // 所有 Rust 类型的大小都是其对齐值的倍数。
    /// let normal = Layout::from_size_align(12, 4).unwrap();
    /// let repeated = normal.repeat(3).unwrap();
    /// assert_eq!(repeated, (Layout::from_size_align(36, 4).unwrap(), 12));
    ///
    /// // 但你可以手动构造不满足该规则的 layout。
    /// let padding_needed = Layout::from_size_align(6, 4).unwrap();
    /// let repeated = padding_needed.repeat(3).unwrap();
    /// assert_eq!(repeated, (Layout::from_size_align(24, 4).unwrap(), 8));
    /// ```
    #[unstable(feature = "alloc_layout_extra", issue = "55724")]
    #[inline]
    pub const fn repeat(&self, n: usize) -> Result<(Self, usize), LayoutError> {
        let padded = self.pad_to_align();
        if let Ok(repeated) = padded.repeat_packed(n) {
            Ok((repeated, padded.size()))
        } else {
            Err(LayoutError)
        }
    }

    /// 创建一个描述 `self` 后跟 `next` 的记录 layout，其中包含必要 padding
    /// 以确保 `next` 正确对齐，但*不包含尾部 padding*。
    ///
    /// 为了匹配 C 表示 layout `repr(C)`，应在用所有字段扩展 layout 后调用
    /// `pad_to_align`。（无法匹配默认 Rust 表示 layout `repr(Rust)`，
    /// 因为它未被指定。）
    ///
    /// 注意，为了确保两部分都对齐，结果 layout 的对齐值将是 `self` 和 `next`
    /// 对齐值中的较大者。
    ///
    /// 返回 `Ok((k, offset))`，其中 `k` 是拼接后记录的 layout，`offset` 是嵌入在拼接
    /// 记录中的 `next` 起始位置的相对字节位置（假设记录本身从偏移 0 开始）。
    ///
    /// 算术溢出时返回 `LayoutError`。
    ///
    /// # 示例
    ///
    /// 根据字段 layout 计算 `#[repr(C)]` 结构体的 layout 和字段偏移：
    ///
    /// ```rust
    /// # use std::alloc::{Layout, LayoutError};
    /// pub fn repr_c(fields: &[Layout]) -> Result<(Layout, Vec<usize>), LayoutError> {
    ///     let mut offsets = Vec::new();
    ///     let mut layout = Layout::from_size_align(0, 1)?;
    ///     for &field in fields {
    ///         let (new_layout, offset) = layout.extend(field)?;
    ///         layout = new_layout;
    ///         offsets.push(offset);
    ///     }
    ///     // 记得最后用 `pad_to_align` 收尾！
    ///     Ok((layout.pad_to_align(), offsets))
    /// }
    /// # // 测试它能工作
    /// # #[repr(C)] struct S { a: u64, b: u32, c: u16, d: u32 }
    /// # let s = Layout::new::<S>();
    /// # let u16 = Layout::new::<u16>();
    /// # let u32 = Layout::new::<u32>();
    /// # let u64 = Layout::new::<u64>();
    /// # assert_eq!(repr_c(&[u64, u32, u16, u32]), Ok((s, vec![0, 8, 12, 16])));
    /// ```
    #[stable(feature = "alloc_layout_manipulation", since = "1.44.0")]
    #[rustc_const_stable(feature = "const_alloc_layout", since = "1.85.0")]
    #[inline]
    pub const fn extend(&self, next: Self) -> Result<(Self, usize), LayoutError> {
        let new_align = Alignment::max(self.align, next.align);
        let offset = self.size_rounded_up_to_custom_align(next.align);

        // SAFETY: `offset` 至多为 `isize::MAX + 1`（例如对齐到 `Alignment::MAX`），
        // 而 `next.size` 根据 `Layout` 类型不变量至多为 `isize::MAX`。
        // 因此最大的可能 `new_size` 是 `isize::MAX + 1 + isize::MAX`，
        // 即 `usize::MAX`，不会溢出。
        let new_size = unsafe { unchecked_add(offset, next.size) };

        if let Ok(layout) = Layout::from_size_alignment(new_size, new_align) {
            Ok((layout, offset))
        } else {
            Err(LayoutError)
        }
    }

    /// 创建一个描述 `self` 的 `n` 个实例的记录 layout，实例之间没有 padding。
    ///
    /// 注意，不同于 `repeat`，`repeat_packed` 不保证重复的 `self` 实例会正确对齐，
    /// 即使某一个 `self` 实例本身是正确对齐的。换言之，如果使用 `repeat_packed`
    /// 返回的 layout 分配数组，不保证数组中的所有元素都正确对齐。
    ///
    /// 算术溢出时返回 `LayoutError`。
    #[unstable(feature = "alloc_layout_extra", issue = "55724")]
    #[inline]
    pub const fn repeat_packed(&self, n: usize) -> Result<Self, LayoutError> {
        if let Some(size) = self.size.checked_mul(n) {
            // 这里调用安全构造器以强制执行 isize 大小限制。
            Layout::from_size_alignment(size, self.align)
        } else {
            Err(LayoutError)
        }
    }

    /// 创建一个描述 `self` 后跟 `next` 的记录 layout，两者之间没有额外 padding。
    /// 由于没有插入 padding，`next` 的对齐值无关紧要，并且*完全不会*纳入结果 layout。
    ///
    /// 算术溢出时返回 `LayoutError`。
    #[unstable(feature = "alloc_layout_extra", issue = "55724")]
    #[inline]
    pub const fn extend_packed(&self, next: Self) -> Result<Self, LayoutError> {
        // SAFETY: 每个 `size` 至多为 `isize::MAX == usize::MAX/2`，
        // 因此和至多为 `usize::MAX/2*2 == usize::MAX - 1`，不会溢出。
        let new_size = unsafe { unchecked_add(self.size, next.size) };
        // 安全构造器会强制新大小相对于该对齐值不会过大。
        Layout::from_size_alignment(new_size, self.align)
    }

    /// 创建描述 `[T; n]` 记录的 layout。
    ///
    /// 算术溢出或总大小会超过 `isize::MAX` 时，返回 `LayoutError`。
    #[stable(feature = "alloc_layout_manipulation", since = "1.44.0")]
    #[rustc_const_stable(feature = "const_alloc_layout", since = "1.85.0")]
    #[inline]
    pub const fn array<T>(n: usize) -> Result<Self, LayoutError> {
        // 减少每个 `T` 需要单态化的代码量。
        return inner(T::LAYOUT, n);

        #[inline]
        const fn inner(element_layout: Layout, n: usize) -> Result<Layout, LayoutError> {
            let Layout { size: element_size, align } = element_layout;

            // 需要检查关于大小的两件事：
            //  - 总大小不会溢出 `usize`，
            //  - 总大小仍能放入 `isize`。
            // 使用除法可以用单个阈值同时检查二者。这通常不是好主意，
            // 但这里元素大小和对齐值都是常量，编译器会把它们全部折叠掉。
            if element_size != 0 && n > Layout::max_size_for_align(align) / element_size {
                return Err(LayoutError);
            }

            // SAFETY: 刚刚已经检查过，乘法不会溢出 `usize`。
            // 在此函数内部这是无用提示，但内联后，它有助于在该乘法前后去重
            // “总体容量是否为零”的检查（例如 RawVec 的分配路径中）。
            let array_size = unsafe { unchecked_mul(element_size, n) };

            // SAFETY: 上面已经检查过，即使按对齐值向上取整，`array_size`
            // 也不会超过 `isize::MAX`。并且 `Alignment` 保证它是二的幂。
            unsafe { Ok(Layout::from_size_align_unchecked(array_size, align.as_usize())) }
        }
    }

    /// 永久不稳定接口：以 `Alignment` 类型访问 `align`。
    #[unstable(issue = "none", feature = "std_internals")]
    #[doc(hidden)]
    #[inline]
    pub const fn alignment(&self) -> Alignment {
        self.align
    }
}

#[stable(feature = "alloc_layout", since = "1.28.0")]
#[deprecated(
    since = "1.52.0",
    note = "Name does not follow std convention, use LayoutError",
    suggestion = "LayoutError"
)]
pub type LayoutErr = LayoutError;

/// 当传给 `Layout::from_size_align` 或其他 `Layout` 构造器的参数不满足其文档约束时，
/// 会返回 `LayoutError`。
#[stable(feature = "alloc_layout_error", since = "1.50.0")]
#[non_exhaustive]
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct LayoutError;

#[stable(feature = "alloc_layout", since = "1.28.0")]
impl Error for LayoutError {}

// （下游的 trait Error impl 需要这个）
#[stable(feature = "alloc_layout", since = "1.28.0")]
impl fmt::Display for LayoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("invalid parameters to Layout::from_size_align")
    }
}
