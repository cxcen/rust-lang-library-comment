//! 用裸指针和长度构造 `&[T]` 与 `&mut [T]` 的自由函数。
//!
//! 这些函数是切片与 FFI、手写分配器、裸指针算法之间的边界。它们只把
//! “起始地址 + 元素个数” 重新解释成 Rust 引用，不会证明内存是否真实有效；
//! 调用方必须维护引用要求的有效性、对齐、生命周期和 aliasing 规则。

use crate::ops::Range;
use crate::{array, ptr, ub_checks};

/// 由裸指针和长度构造共享切片。
///
/// `len` 参数表示 **元素个数**，不是字节数。实际覆盖的字节范围是
/// `len * size_of::<T>()`，并且这个范围必须满足 Rust 对 `&[T]` 的所有不变量。
///
/// # 安全性(Safety）
///
/// 违反下面任一条件都会导致 undefined behavior，即使随后没有真正读取返回的切片：
///
/// * `data` 必须非空、按 `T` 正确对齐，并且在
///   `len * size_of::<T>()` 字节范围内对读取是 [valid] 的。这尤其意味着：
///
///     * 这个切片的整个内存范围必须位于 **同一个 allocation** 内。切片不能跨越两个
///       allocation，即使两个对象在数值地址上碰巧相邻也不行。见
///       [下面](#错误用法) 的错误示例。
///     * 即使 `len == 0`，或者 `T` 是 ZST，`data` 仍然必须非空且对齐。原因之一是
///       enum layout 优化可能依赖引用（包括任意长度的切片引用）总是非空且对齐，
///       用这些 niche 与其它数据区分。零长度切片可使用 [`NonNull::dangling()`]
///       产生一个可作为 `data` 的哨兵指针。
///
/// * `data` 必须指向 `len` 个连续、已经正确初始化的 `T` 值；不能把未初始化字节、
///   已经被移动走的值或不满足 `T` 有效性约束的位模式暴露成 `&[T]`。
///
/// * 在返回切片的生命周期 `'a` 内，返回切片所引用的内存不得被修改，除非修改发生在
///   `UnsafeCell` 内部。这是共享引用的 aliasing 契约：编译器可以假设普通
///   `&[T]` 背后的 `T` 在 `'a` 内不会被其它可变路径改写。
///
/// * 切片总大小 `len * size_of::<T>()` 不能超过 `isize::MAX`，把这个大小加到
///   `data` 上也不能让地址空间发生“回绕”。这是 [`pointer::offset`] 的安全前置，
///   也是 Rust 用 `isize` 表示同一 allocation 内偏移时的优化假设。
///
/// # 注意事项
///
/// 返回切片的生命周期由使用位置推断。为了避免不小心把裸指针扩展成过长的引用，
/// 通常应通过辅助函数把这个生命周期绑定到某个真实宿主值的生命周期上，或在调用点
/// 显式标注，使 `'a` 不会超过底层 allocation、初始化状态和 aliasing 保证的有效期。
///
/// # 示例
///
/// ```
/// use std::slice;
///
/// // 为单个元素构造长度为 1 的切片。
/// let x = 42;
/// let ptr = &x as *const _;
/// let slice = unsafe { slice::from_raw_parts(ptr, 1) };
/// assert_eq!(slice[0], 42);
/// ```
///
/// ### 错误用法
///
/// 下面的 `join_slices` 函数是 **unsound** 的，因为它只比较了数值地址是否连续，
/// 没有证明两个输入来自同一个 allocation。
///
/// ```rust,no_run
/// use std::slice;
///
/// fn join_slices<'a, T>(fst: &'a [T], snd: &'a [T]) -> &'a [T] {
///     let fst_end = fst.as_ptr().wrapping_add(fst.len());
///     let snd_start = snd.as_ptr();
///     assert_eq!(fst_end, snd_start, "Slices must be contiguous!");
///     unsafe {
///         // 上面的断言只说明 `fst` 与 `snd` 的地址数值连续；它们仍可能属于
///         // _不同 allocation_，此时把两者拼成一个切片就是 undefined behavior。
///         slice::from_raw_parts(fst.as_ptr(), fst.len() + snd.len())
///     }
/// }
///
/// fn main() {
///     // `a` 与 `b` 是不同 allocation...
///     let a = 42;
///     let b = 27;
///     // ... 但它们在内存中的数值地址仍可能碰巧连续：| a | b |
///     let _ = join_slices(slice::from_ref(&a), slice::from_ref(&b)); // UB
/// }
/// ```
///
/// ### FFI: 处理空指针
///
/// 在 C++ 等语言中，空集合的 `data()` 指针不保证非空。Rust 切片引用要求指针非空，
/// 因此外部接口传入 `(ptr, len)` 时必须先处理空指针，避免把 `NULL` 直接交给
/// `from_raw_parts`。
///
/// ```
/// use std::slice;
///
/// /// 对 FFI 传入的切片元素求和。
/// ///
/// /// # 安全性(Safety）
/// ///
/// /// 如果 ptr 不是 NULL，它必须正确对齐，并且指向 `len` 个已经初始化的 `f32`。
/// unsafe extern "C" fn sum_slice(ptr: *const f32, len: usize) -> f32 {
///     let data = if ptr.is_null() {
///         // 这里约定空指针只表示空切片，因此假定 `len` 为 0。
///         &[]
///     } else {
///         // SAFETY: 函数的安全文档要求非空分支中的 `ptr` 满足对齐、初始化和长度契约。
///         unsafe { slice::from_raw_parts(ptr, len) }
///     };
///     data.into_iter().sum()
/// }
///
/// // 这可能是 C++ 的 std::vector::data() 对空 vector 的返回值：
/// let ptr = std::ptr::null();
/// // 这可能是 std::vector::size()：
/// let len = 0;
/// assert_eq!(unsafe { sum_slice(ptr, len) }, 0.0);
/// ```
///
/// [valid]: ptr#safety
/// [`NonNull::dangling()`]: ptr::NonNull::dangling
#[inline]
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_stable(feature = "const_slice_from_raw_parts", since = "1.64.0")]
#[must_use]
#[rustc_diagnostic_item = "slice_from_raw_parts"]
#[track_caller]
pub const unsafe fn from_raw_parts<'a, T>(data: *const T, len: usize) -> &'a [T] {
    // SAFETY: 调用方必须维护 `from_raw_parts` 的完整安全契约；这里的 UB 检查只覆盖
    // 非空、对齐和总大小，初始化、同一 allocation、生命周期与 aliasing 仍由调用方保证。
    unsafe {
        ub_checks::assert_unsafe_precondition!(
            check_language_ub,
            "slice::from_raw_parts requires the pointer to be aligned and non-null, and the total size of the slice not to exceed `isize::MAX`",
            (
                data: *mut () = data as *mut (),
                size: usize = size_of::<T>(),
                align: usize = align_of::<T>(),
                len: usize = len,
            ) =>
            ub_checks::maybe_is_aligned_and_not_null(data, align, false)
                && ub_checks::is_valid_allocation_size(size, len)
        );
        &*ptr::slice_from_raw_parts(data, len)
    }
}

/// 与 [`from_raw_parts`] 功能相同，但返回可变切片。
///
/// `&mut [T]` 还会给编译器提供唯一访问权：在生命周期 `'a` 内，返回值覆盖的每个元素
/// 都必须只能通过这个可变切片（或从它派生出的引用/指针）访问。
///
/// # 安全性(Safety）
///
/// 违反下面任一条件都会导致 undefined behavior：
///
/// * `data` 必须非空、按 `T` 正确对齐，并且在 `len * size_of::<T>()` 字节范围内
///   对读取和写入都是 [valid] 的。这尤其意味着：
///
///     * 整个切片范围必须位于同一个 allocation 内，不能跨 allocation。
///     * 即使是零长度切片或 ZST 切片，`data` 也必须非空且对齐；可用
///       [`NonNull::dangling()`] 构造零长度情况下的有效哨兵指针。
///
/// * `data` 必须指向 `len` 个连续、已经正确初始化的 `T` 值。
///
/// * 在返回切片生命周期 `'a` 内，除从返回值派生出的引用或指针外，不能通过任何其它指针
///   访问这段内存；读和写都禁止。这是 `&mut` 独占 aliasing 契约，违反它会破坏编译器
///   对无别名可变引用的优化假设。
///
/// * 切片总大小 `len * size_of::<T>()` 不能超过 `isize::MAX`，把这个大小加到
///   `data` 上也不能让地址空间回绕。见 [`pointer::offset`] 的安全文档。
///
/// [valid]: ptr#safety
/// [`NonNull::dangling()`]: ptr::NonNull::dangling
#[inline]
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_stable(feature = "const_slice_from_raw_parts_mut", since = "1.83.0")]
#[must_use]
#[rustc_diagnostic_item = "slice_from_raw_parts_mut"]
#[track_caller]
pub const unsafe fn from_raw_parts_mut<'a, T>(data: *mut T, len: usize) -> &'a mut [T] {
    // SAFETY: 调用方必须维护 `from_raw_parts_mut` 的完整安全契约；这里仅把裸的
    // `(data, len)` 重新组成 `&mut [T]`，不会证明初始化、唯一访问权或生命周期。
    unsafe {
        ub_checks::assert_unsafe_precondition!(
            check_language_ub,
            "slice::from_raw_parts_mut requires the pointer to be aligned and non-null, and the total size of the slice not to exceed `isize::MAX`",
            (
                data: *mut () = data as *mut (),
                size: usize = size_of::<T>(),
                align: usize = align_of::<T>(),
                len: usize = len,
            ) =>
            ub_checks::maybe_is_aligned_and_not_null(data, align, false)
                && ub_checks::is_valid_allocation_size(size, len)
        );
        &mut *ptr::slice_from_raw_parts_mut(data, len)
    }
}

/// 将 `T` 的共享引用转换成长度为 1 的切片，不复制元素。
#[stable(feature = "from_ref", since = "1.28.0")]
#[rustc_const_stable(feature = "const_slice_from_ref_shared", since = "1.63.0")]
#[rustc_diagnostic_item = "slice_from_ref"]
#[must_use]
pub const fn from_ref<T>(s: &T) -> &[T] {
    array::from_ref(s)
}

/// 将 `T` 的可变引用转换成长度为 1 的可变切片，不复制元素。
#[stable(feature = "from_ref", since = "1.28.0")]
#[rustc_const_stable(feature = "const_slice_from_ref", since = "1.83.0")]
#[must_use]
pub const fn from_mut<T>(s: &mut T) -> &mut [T] {
    array::from_mut(s)
}

/// 由指针范围构造共享切片。
///
/// 这适合与用两个指针表示内存区间的外部接口交互，例如 C++ 中常见的
/// `[begin, end)` 约定。`end` 必须是一过末尾指针，范围长度由
/// `end.offset_from_unsigned(start)` 推导。
///
/// # 安全性(Safety）
///
/// 违反下面任一条件都会导致 undefined behavior：
///
/// * 范围的 `start` 指针必须非空、[valid]、正确对齐，并指向切片第一个元素。
///
/// * `end` 指针必须是 [valid] 且正确对齐的 *一过末尾* 指针；从 `start` 到 `end`
///   的偏移必须正好等于切片长度。
///
/// * 整个切片内存范围必须位于同一个 allocation 内，切片不能跨 allocation。
///
/// * 范围内必须包含 `N` 个连续、已经正确初始化的 `T` 值。
///
/// * 在返回切片生命周期 `'a` 内，这段内存不得被修改，除非修改发生在 `UnsafeCell` 内部。
///
/// * 范围总长度不能超过 `isize::MAX`，把对应字节大小加到 `start` 上不能让地址空间回绕。
///   见 [`pointer::offset`] 的安全文档。
///
/// 注意，由 [`slice::as_ptr_range`] 从同一个有效切片产生的范围满足这些要求。
///
/// # Panics
///
/// 如果 `T` 是 Zero-Sized Type（“ZST”），本函数会 panic。ZST 没有可用的字节距离来
/// 从两个指针恢复元素个数。
///
/// # 注意事项
///
/// 返回切片的生命周期由使用位置推断；应把它绑定到真实宿主对象或外部缓冲区的有效期上，
/// 避免把一对裸指针意外提升成过长的 `&[T]`。
///
/// # 示例
///
/// ```
/// #![feature(slice_from_ptr_range)]
///
/// use core::slice;
///
/// let x = [1, 2, 3];
/// let range = x.as_ptr_range();
///
/// unsafe {
///     assert_eq!(slice::from_ptr_range(range), &x);
/// }
/// ```
///
/// [valid]: ptr#safety
#[unstable(feature = "slice_from_ptr_range", issue = "89792")]
#[rustc_const_unstable(feature = "const_slice_from_ptr_range", issue = "89792")]
#[track_caller]
pub const unsafe fn from_ptr_range<'a, T>(range: Range<*const T>) -> &'a [T] {
    // SAFETY: 调用方保证 `range.start..range.end` 是同一 allocation 内的有效
    // `[begin, end)` 范围；`offset_from_unsigned` 得到的长度可安全交给 `from_raw_parts`。
    unsafe { from_raw_parts(range.start, range.end.offset_from_unsigned(range.start)) }
}

/// 由指针范围构造可变切片。
///
/// 这与 [`from_ptr_range`] 功能相同，但返回 `&mut [T]`，因此还要求整个范围在
/// 生命周期 `'a` 内具备唯一访问权。
///
/// 它适合与使用 `[begin, end)` 两指针约定的外部接口交互，例如 C++ 容器或手写缓冲区。
///
/// # 安全性(Safety）
///
/// 违反下面任一条件都会导致 undefined behavior：
///
/// * 范围的 `start` 指针必须非空、[valid]、正确对齐，并指向切片第一个元素。
///
/// * `end` 指针必须是 [valid] 且正确对齐的 *一过末尾* 指针；从 `start` 到 `end`
///   的偏移必须正好等于切片长度。
///
/// * 整个切片范围必须位于同一个 allocation 内，不能跨 allocation。
///
/// * 范围内必须包含 `N` 个连续、已经正确初始化的 `T` 值。
///
/// * 在返回切片生命周期 `'a` 内，不能通过任何非返回值派生的指针访问这段内存；
///   读和写都禁止，以维护 `&mut [T]` 的独占 aliasing 契约。
///
/// * 范围总长度不能超过 `isize::MAX`，把对应字节大小加到 `start` 上不能让地址空间回绕。
///   见 [`pointer::offset`] 的安全文档。
///
/// 注意，由 [`slice::as_mut_ptr_range`] 从同一个有效可变切片产生的范围满足这些要求。
///
/// # Panics
///
/// 如果 `T` 是 Zero-Sized Type（“ZST”），本函数会 panic。
///
/// # 注意事项
///
/// 返回切片的生命周期由使用位置推断；应把它显式绑定到真实缓冲区的生命周期，避免产生
/// 比底层唯一借用更长的 `&mut [T]`。
///
/// # 示例
///
/// ```
/// #![feature(slice_from_ptr_range)]
///
/// use core::slice;
///
/// let mut x = [1, 2, 3];
/// let range = x.as_mut_ptr_range();
///
/// unsafe {
///     assert_eq!(slice::from_mut_ptr_range(range), &mut [1, 2, 3]);
/// }
/// ```
///
/// [valid]: ptr#safety
#[unstable(feature = "slice_from_ptr_range", issue = "89792")]
#[rustc_const_unstable(feature = "const_slice_from_mut_ptr_range", issue = "89792")]
pub const unsafe fn from_mut_ptr_range<'a, T>(range: Range<*mut T>) -> &'a mut [T] {
    // SAFETY: 调用方保证该可变指针范围有效、同 allocation、已初始化且唯一可访问；
    // 得到的长度因此可安全交给 `from_raw_parts_mut`。
    unsafe { from_raw_parts_mut(range.start, range.end.offset_from_unsigned(range.start)) }
}
