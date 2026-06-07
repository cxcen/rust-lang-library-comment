//! 内存分配 API

#![stable(feature = "alloc_module", since = "1.28.0")]

mod global;
mod layout;

#[stable(feature = "global_alloc", since = "1.28.0")]
pub use self::global::GlobalAlloc;
#[stable(feature = "alloc_layout", since = "1.28.0")]
pub use self::layout::Layout;
#[stable(feature = "alloc_layout", since = "1.28.0")]
#[deprecated(
    since = "1.52.0",
    note = "Name does not follow std convention, use LayoutError",
    suggestion = "LayoutError"
)]
#[allow(deprecated, deprecated_in_future)]
pub use self::layout::LayoutErr;
#[stable(feature = "alloc_layout_error", since = "1.50.0")]
pub use self::layout::LayoutError;
use crate::error::Error;
use crate::fmt;
use crate::ptr::{self, NonNull};

/// `AllocError` 错误表示分配失败；失败原因可能是资源耗尽，
/// 也可能是给定输入参数与此 allocator 的组合存在问题。
#[unstable(feature = "allocator_api", issue = "32838")]
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub struct AllocError;

#[unstable(
    feature = "allocator_api",
    reason = "the precise API and guarantees it provides may be tweaked.",
    issue = "32838"
)]
impl Error for AllocError {}

// （下游的 trait Error impl 需要这个）
#[unstable(feature = "allocator_api", issue = "32838")]
impl fmt::Display for AllocError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("memory allocation failed")
    }
}

/// `Allocator` 的实现可以分配、扩展、收缩和释放由 [`Layout`][] 描述的任意数据块。
///
/// `Allocator` 设计为可在 ZST、引用或智能指针上实现。
/// `MyAlloc([u8; N])` 这类 allocator 不能在不更新已分配内存指针的情况下被移动。
///
/// 与 [`GlobalAlloc`][] 不同，`Allocator` 允许零大小分配。如果底层 allocator
/// 不支持零大小分配（如 jemalloc），或通过返回空指针响应（如 `libc::malloc`），
/// 实现必须捕获并处理这种情况。
///
/// ### 当前已分配的内存 {#currently-allocated-memory}
///
/// 某些方法要求内存块由 allocator *当前分配*。这意味着：
///  * 该内存块的起始地址先前由 [`allocate`]、[`grow`] 或 [`shrink`] 返回，并且
///  * 该内存块随后尚未被释放。
///
/// 调用 [`deallocate`] 会释放内存块；调用 [`grow`] 或 [`shrink`] 且返回 `Ok`
/// 也会释放原内存块。返回 `Err` 的 `grow` 或 `shrink` 调用不会释放传入的内存块。
///
/// [`allocate`]: Allocator::allocate
/// [`grow`]: Allocator::grow
/// [`shrink`]: Allocator::shrink
/// [`deallocate`]: Allocator::deallocate
///
/// ### 内存匹配 {#memory-fitting}
///
/// 某些方法要求 `layout` 与内存块*匹配*，或反过来要求内存块与 `layout` 匹配。
/// 这表示必须满足以下条件：
///  * 内存块必须以 [`layout.align()`] 的对齐值*当前分配*，并且
///  * [`layout.size()`] 必须落在 `min ..= max` 范围内，其中：
///    - `min` 是用于分配该块的 layout 的大小，
///    - `max` 是 [`allocate`]、[`grow`] 或 [`shrink`] 返回的实际大小。
///
/// [`layout.align()`]: Layout::align
/// [`layout.size()`]: Layout::size
///
/// # 安全性(Safety）
///
/// allocator [*当前分配*]的内存块必须指向有效内存，并且在以下任一事件发生前保持有效：
///  - 该内存块被释放，或
///  - 该 allocator 被 drop。
///
/// 复制、克隆或移动 allocator 不得使它返回过的内存块失效。
/// 复制或克隆出的 allocator 必须表现得像原 allocator 一样。
///
/// [*当前分配*]的内存块可以传给该 allocator 中任何接受此类参数的方法。
///
/// [*当前分配*]: #currently-allocated-memory
#[unstable(feature = "allocator_api", issue = "32838")]
#[rustc_const_unstable(feature = "const_heap", issue = "79597")]
pub const unsafe trait Allocator {
    /// 尝试分配一块内存。
    ///
    /// 成功时返回满足 `layout` 大小和对齐保证的 [`NonNull<[u8]>`][NonNull]。
    ///
    /// 返回的内存块大小可能大于 `layout.size()` 指定的大小，其内容可能已初始化，也可能未初始化。
    ///
    /// 返回的内存块只要仍[*当前分配*]，且未超过以下两者中较短者，就保持有效：
    ///   - allocator 类型自身的 borrow-checker 生命周期。
    ///   - allocator 及其所有 clone 尚未被 drop 的时间。
    ///
    /// [*当前分配*]: #currently-allocated-memory
    ///
    /// # 错误
    ///
    /// 返回 `Err` 表示内存耗尽，或 `layout` 不满足 allocator 的大小或对齐约束。
    ///
    /// 鼓励实现方在内存耗尽时返回 `Err`，而不是 panic 或 abort，但这不是严格要求。
    /// （具体而言，在内存耗尽时会 abort 的底层原生分配库之上实现此 trait 是*合法的*。）
    ///
    /// 希望在分配错误时终止计算的客户端应调用 [`handle_alloc_error`] 函数，
    /// 而不是直接调用 `panic!` 或类似机制。
    ///
    /// [`handle_alloc_error`]: ../../alloc/alloc/fn.handle_alloc_error.html
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError>;

    /// 行为类似 `allocate`，但还会确保返回的内存被零初始化。
    ///
    /// # 错误
    ///
    /// 返回 `Err` 表示内存耗尽，或 `layout` 不满足 allocator 的大小或对齐约束。
    ///
    /// 鼓励实现方在内存耗尽时返回 `Err`，而不是 panic 或 abort，但这不是严格要求。
    /// （具体而言，在内存耗尽时会 abort 的底层原生分配库之上实现此 trait 是*合法的*。）
    ///
    /// 希望在分配错误时终止计算的客户端应调用 [`handle_alloc_error`] 函数，
    /// 而不是直接调用 `panic!` 或类似机制。
    ///
    /// [`handle_alloc_error`]: ../../alloc/alloc/fn.handle_alloc_error.html
    fn allocate_zeroed(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        let ptr = self.allocate(layout)?;
        // SAFETY: `allocate` 返回的内存块对 `ptr.len()` 个字节写入有效。
        unsafe { ptr.as_non_null_ptr().as_ptr().write_bytes(0, ptr.len()) }
        Ok(ptr)
    }

    /// 释放 `ptr` 引用的内存。
    ///
    /// # 安全性(Safety）
    ///
    /// * `ptr` 必须表示通过此 allocator [*当前分配*]的一块内存，并且
    /// * `layout` 必须与该内存块[*匹配*]。
    ///
    /// [*当前分配*]: #currently-allocated-memory
    /// [*匹配*]: #memory-fitting
    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout);

    /// 尝试扩展内存块。
    ///
    /// 返回新的 [`NonNull<[u8]>`][NonNull]，其中包含指针和已分配内存的实际大小。
    /// 该指针适合保存 `new_layout` 描述的数据。为此，allocator 可以扩展 `ptr`
    /// 引用的分配，使其匹配新 layout。
    ///
    /// 如果此方法返回 `Ok`，则 `ptr` 引用的内存块所有权已经转移给此 allocator。
    /// 任何对旧 `ptr` 的访问都是未定义行为(Undefined Behavior)，即使该分配是在原地扩展的。
    /// 新返回的指针现在是访问这块内存的唯一有效指针。
    ///
    /// 如果此方法返回 `Err`，则内存块所有权没有转移给此 allocator，
    /// 且该内存块的内容保持不变。
    ///
    /// # 安全性(Safety）
    ///
    /// * `ptr` 必须表示通过此 allocator [*当前分配*]的一块内存。
    /// * `old_layout` 必须与该内存块[*匹配*]（`new_layout` 参数不需要匹配它）。
    /// * `new_layout.size()` 必须大于或等于 `old_layout.size()`。
    ///
    /// 注意，`new_layout.align()` 不必与 `old_layout.align()` 相同。
    ///
    /// [*当前分配*]: #currently-allocated-memory
    /// [*匹配*]: #memory-fitting
    ///
    /// # 错误
    ///
    /// 如果新 layout 不满足 allocator 的大小和对齐约束，或扩展因其他原因失败，
    /// 则返回 `Err`。
    ///
    /// 鼓励实现方在内存耗尽时返回 `Err`，而不是 panic 或 abort，但这不是严格要求。
    /// （具体而言，在内存耗尽时会 abort 的底层原生分配库之上实现此 trait 是*合法的*。）
    ///
    /// 希望在分配错误时终止计算的客户端应调用 [`handle_alloc_error`] 函数，
    /// 而不是直接调用 `panic!` 或类似机制。
    ///
    /// [`handle_alloc_error`]: ../../alloc/alloc/fn.handle_alloc_error.html
    unsafe fn grow(
        &self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, AllocError> {
        debug_assert!(
            new_layout.size() >= old_layout.size(),
            "`new_layout.size()` must be greater than or equal to `old_layout.size()`"
        );

        let new_ptr = self.allocate(new_layout)?;

        // SAFETY: 因为 `new_layout.size()` 必须大于或等于 `old_layout.size()`，
        // 所以旧分配和新分配都可对 `old_layout.size()` 个字节进行读写。
        // 另外，旧分配尚未释放，因此不能与 `new_ptr` 重叠。所以调用
        // `copy_nonoverlapping` 是安全的。`deallocate` 的安全契约由调用者保证。
        unsafe {
            ptr::copy_nonoverlapping(ptr.as_ptr(), new_ptr.as_mut_ptr(), old_layout.size());
            self.deallocate(ptr, old_layout);
        }

        Ok(new_ptr)
    }

    /// 行为类似 `grow`，但还会确保返回前新内容被设置为零。
    ///
    /// 成功调用 `grow_zeroed` 后，内存块将包含以下内容：
    ///   * 字节 `0..old_layout.size()` 从原分配中保留。
    ///   * 字节 `old_layout.size()..old_size` 会被保留或置零，取决于 allocator 实现。
    ///     `old_size` 指 `grow_zeroed` 调用前内存块的大小，它可能大于分配时最初请求的大小。
    ///   * 字节 `old_size..new_size` 被置零。`new_size` 指 `grow_zeroed` 调用返回的内存块大小。
    ///
    /// # 安全性(Safety）
    ///
    /// * `ptr` 必须表示通过此 allocator [*当前分配*]的一块内存。
    /// * `old_layout` 必须与该内存块[*匹配*]（`new_layout` 参数不需要匹配它）。
    /// * `new_layout.size()` 必须大于或等于 `old_layout.size()`。
    ///
    /// 注意，`new_layout.align()` 不必与 `old_layout.align()` 相同。
    ///
    /// [*当前分配*]: #currently-allocated-memory
    /// [*匹配*]: #memory-fitting
    ///
    /// # 错误
    ///
    /// 如果新 layout 不满足 allocator 的大小和对齐约束，或扩展因其他原因失败，
    /// 则返回 `Err`。
    ///
    /// 鼓励实现方在内存耗尽时返回 `Err`，而不是 panic 或 abort，但这不是严格要求。
    /// （具体而言，在内存耗尽时会 abort 的底层原生分配库之上实现此 trait 是*合法的*。）
    ///
    /// 希望在分配错误时终止计算的客户端应调用 [`handle_alloc_error`] 函数，
    /// 而不是直接调用 `panic!` 或类似机制。
    ///
    /// [`handle_alloc_error`]: ../../alloc/alloc/fn.handle_alloc_error.html
    unsafe fn grow_zeroed(
        &self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, AllocError> {
        debug_assert!(
            new_layout.size() >= old_layout.size(),
            "`new_layout.size()` must be greater than or equal to `old_layout.size()`"
        );

        let new_ptr = self.allocate_zeroed(new_layout)?;

        // SAFETY: 因为 `new_layout.size()` 必须大于或等于 `old_layout.size()`，
        // 所以旧分配和新分配都可对 `old_layout.size()` 个字节进行读写。
        // 另外，旧分配尚未释放，因此不能与 `new_ptr` 重叠。所以调用
        // `copy_nonoverlapping` 是安全的。`deallocate` 的安全契约由调用者保证。
        unsafe {
            ptr::copy_nonoverlapping(ptr.as_ptr(), new_ptr.as_mut_ptr(), old_layout.size());
            self.deallocate(ptr, old_layout);
        }

        Ok(new_ptr)
    }

    /// 尝试收缩内存块。
    ///
    /// 返回新的 [`NonNull<[u8]>`][NonNull]，其中包含指针和已分配内存的实际大小。
    /// 该指针适合保存 `new_layout` 描述的数据。为此，allocator 可以收缩 `ptr`
    /// 引用的分配，使其匹配新 layout。
    ///
    /// 如果此方法返回 `Ok`，则 `ptr` 引用的内存块所有权已经转移给此 allocator。
    /// 任何对旧 `ptr` 的访问都是未定义行为(Undefined Behavior)，即使该分配是在原地收缩的。
    /// 新返回的指针现在是访问这块内存的唯一有效指针。
    ///
    /// 如果此方法返回 `Err`，则内存块所有权没有转移给此 allocator，
    /// 且该内存块的内容保持不变。
    ///
    /// # 安全性(Safety）
    ///
    /// * `ptr` 必须表示通过此 allocator [*当前分配*]的一块内存。
    /// * `old_layout` 必须与该内存块[*匹配*]（`new_layout` 参数不需要匹配它）。
    /// * `new_layout.size()` 必须小于或等于 `old_layout.size()`。
    ///
    /// 注意，`new_layout.align()` 不必与 `old_layout.align()` 相同。
    ///
    /// [*当前分配*]: #currently-allocated-memory
    /// [*匹配*]: #memory-fitting
    ///
    /// # 错误
    ///
    /// 如果新 layout 不满足 allocator 的大小和对齐约束，或收缩因其他原因失败，
    /// 则返回 `Err`。
    ///
    /// 鼓励实现方在内存耗尽时返回 `Err`，而不是 panic 或 abort，但这不是严格要求。
    /// （具体而言，在内存耗尽时会 abort 的底层原生分配库之上实现此 trait 是*合法的*。）
    ///
    /// 希望在分配错误时终止计算的客户端应调用 [`handle_alloc_error`] 函数，
    /// 而不是直接调用 `panic!` 或类似机制。
    ///
    /// [`handle_alloc_error`]: ../../alloc/alloc/fn.handle_alloc_error.html
    unsafe fn shrink(
        &self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, AllocError> {
        debug_assert!(
            new_layout.size() <= old_layout.size(),
            "`new_layout.size()` must be smaller than or equal to `old_layout.size()`"
        );

        let new_ptr = self.allocate(new_layout)?;

        // SAFETY: 因为 `new_layout.size()` 必须小于或等于 `old_layout.size()`，
        // 所以旧分配和新分配都可对 `new_layout.size()` 个字节进行读写。
        // 另外，旧分配尚未释放，因此不能与 `new_ptr` 重叠。所以调用
        // `copy_nonoverlapping` 是安全的。`deallocate` 的安全契约由调用者保证。
        unsafe {
            ptr::copy_nonoverlapping(ptr.as_ptr(), new_ptr.as_mut_ptr(), new_layout.size());
            self.deallocate(ptr, old_layout);
        }

        Ok(new_ptr)
    }

    /// 为此 `Allocator` 实例创建“按引用”适配器。
    ///
    /// 返回的适配器也实现 `Allocator`，并且只会借用此实例。
    #[inline(always)]
    fn by_ref(&self) -> &Self
    where
        Self: Sized,
    {
        self
    }
}

#[unstable(feature = "allocator_api", issue = "32838")]
#[rustc_const_unstable(feature = "const_heap", issue = "79597")]
unsafe impl<A> const Allocator for &A
where
    A: [const] Allocator + ?Sized,
{
    #[inline]
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        (**self).allocate(layout)
    }

    #[inline]
    fn allocate_zeroed(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        (**self).allocate_zeroed(layout)
    }

    #[inline]
    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        // SAFETY: 安全契约必须由调用者保证。
        unsafe { (**self).deallocate(ptr, layout) }
    }

    #[inline]
    unsafe fn grow(
        &self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, AllocError> {
        // SAFETY: 安全契约必须由调用者保证。
        unsafe { (**self).grow(ptr, old_layout, new_layout) }
    }

    #[inline]
    unsafe fn grow_zeroed(
        &self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, AllocError> {
        // SAFETY: 安全契约必须由调用者保证。
        unsafe { (**self).grow_zeroed(ptr, old_layout, new_layout) }
    }

    #[inline]
    unsafe fn shrink(
        &self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, AllocError> {
        // SAFETY: 安全契约必须由调用者保证。
        unsafe { (**self).shrink(ptr, old_layout, new_layout) }
    }
}

#[unstable(feature = "allocator_api", issue = "32838")]
unsafe impl<A> Allocator for &mut A
where
    A: Allocator + ?Sized,
{
    #[inline]
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        (**self).allocate(layout)
    }

    #[inline]
    fn allocate_zeroed(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        (**self).allocate_zeroed(layout)
    }

    #[inline]
    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        // SAFETY: 安全契约必须由调用者保证。
        unsafe { (**self).deallocate(ptr, layout) }
    }

    #[inline]
    unsafe fn grow(
        &self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, AllocError> {
        // SAFETY: 安全契约必须由调用者保证。
        unsafe { (**self).grow(ptr, old_layout, new_layout) }
    }

    #[inline]
    unsafe fn grow_zeroed(
        &self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, AllocError> {
        // SAFETY: 安全契约必须由调用者保证。
        unsafe { (**self).grow_zeroed(ptr, old_layout, new_layout) }
    }

    #[inline]
    unsafe fn shrink(
        &self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, AllocError> {
        // SAFETY: 安全契约必须由调用者保证。
        unsafe { (**self).shrink(ptr, old_layout, new_layout) }
    }
}
