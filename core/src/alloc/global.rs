use crate::alloc::Layout;
use crate::{cmp, ptr};

/// 可通过 `#[global_allocator]` 属性注册为标准库默认分配器的内存分配器。
///
/// 某些方法要求内存块是由某个分配器*当前已分配*的。这意味着：
///
/// * 该内存块的起始地址先前由 `alloc` 等分配方法返回，并且
///
/// * 该内存块之后尚未被释放。内存块传给 `dealloc` 等释放方法，
///   或传给返回非空指针的重新分配方法，都会被视为已经释放。
///
///
/// # 示例
///
/// ```
/// use std::alloc::{GlobalAlloc, Layout};
/// use std::cell::UnsafeCell;
/// use std::ptr::null_mut;
/// use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};
///
/// const ARENA_SIZE: usize = 128 * 1024;
/// const MAX_SUPPORTED_ALIGN: usize = 4096;
/// #[repr(C, align(4096))] // 4096 == MAX_SUPPORTED_ALIGN
/// struct SimpleAllocator {
///     arena: UnsafeCell<[u8; ARENA_SIZE]>,
///     remaining: AtomicUsize, // 从顶部开始分配，向下递减计数
/// }
///
/// #[global_allocator]
/// static ALLOCATOR: SimpleAllocator = SimpleAllocator {
///     arena: UnsafeCell::new([0x55; ARENA_SIZE]),
///     remaining: AtomicUsize::new(ARENA_SIZE),
/// };
///
/// unsafe impl Sync for SimpleAllocator {}
///
/// unsafe impl GlobalAlloc for SimpleAllocator {
///     unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
///         let size = layout.size();
///         let align = layout.align();
///
///         // `Layout` 的约定禁止构造 align=0 或 align 不是 2 的幂的 `Layout`。
///         // 因此我们可以安全地用掩码来保证对齐,而不必担心 UB。
///         let align_mask_to_round_down = !(align - 1);
///
///         if align > MAX_SUPPORTED_ALIGN {
///             return null_mut();
///         }
///
///         let mut allocated = 0;
///         if self
///             .remaining
///             .fetch_update(Relaxed, Relaxed, |mut remaining| {
///                 if size > remaining {
///                     return None;
///                 }
///                 remaining -= size;
///                 remaining &= align_mask_to_round_down;
///                 allocated = remaining;
///                 Some(remaining)
///             })
///             .is_err()
///         {
///             return null_mut();
///         };
///         unsafe { self.arena.get().cast::<u8>().add(allocated) }
///     }
///     unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {}
/// }
///
/// fn main() {
///     let _s = format!("allocating a string!");
///     let currently = ALLOCATOR.remaining.load(Relaxed);
///     println!("allocated so far: {}", ARENA_SIZE - currently);
/// }
/// ```
///
/// # 安全性(Safety）
///
/// `GlobalAlloc` 是 `unsafe` trait，有多方面原因；实现者必须确保遵守以下契约：
///
/// * 全局分配器发生 unwind 是未定义行为。这个限制将来可能会放宽，
///   但目前这些函数中的任何一次 panic 都可能导致内存不安全。
///
/// * 对 `Layout` 的查询和计算一般都必须正确。调用者可以依赖每个方法定义的契约，
///   因而实现者必须保证这些契约持续成立，包括 `Layout` 的大小、对齐以及
///   “大小按对齐向上取整后不超过 `isize::MAX`”等不变量。
///
/// * 即使源码中有显式堆分配，也不能依赖分配确实发生。优化器可能发现未使用的分配，
///   并将其完全消除，或移到栈上，从而根本不调用分配器。优化器还可能假设分配不会失败；
///   因此，原本会因分配失败而失败的代码，可能会因为优化器绕开了分配需求而突然成功。
///   更具体地说，无论你的自定义分配器是否能统计已经发生的分配次数，下面的代码示例都是
///   不健全的。
///
///   ```rust,ignore (unsound and has placeholders)
///   drop(Box::new(42));
///   let number_of_heap_allocs = /* 调用私有的分配器 API */;
///   unsafe { std::hint::assert_unchecked(number_of_heap_allocs > 0); }
///   ```
///
///   注意，上面提到的并不是唯一可能应用的优化。一般来说，如果某次堆分配可以在不改变
///   程序行为的前提下被移除，就不能依赖它确实发生。分配是否发生并不是程序行为的一部分，
///   即使分配器可以通过打印或其他副作用来跟踪分配并观察到这一点也是如此。
///
/// # Re-entrance
///
/// 实现全局分配器时必须小心，避免意外写出无限递归的实现，因为 Rust 标准库中的许多构造
/// 在自身实现中可能会分配内存。例如，在某些平台上 [`std::sync::Mutex`] 可能会分配内存，
/// 因此在全局分配器中使用它会非常成问题。
///
/// 因此，一般来说应坚持只使用 [`core`] 提供的库功能，并避免在全局分配器中使用 [`std`]。
/// [`std`] 中有少数功能保证不会使用 `#[global_allocator]` 进行分配：
///
///  - [`std::thread_local`],
///  - [`std::thread::current`],
///  - [`std::thread::park`] 以及 [`std::thread::Thread`] 的 [`unpark`] 方法和
/// [`Clone`] 实现。
///
/// [`std`]: ../../std/index.html
/// [`std::sync::Mutex`]: ../../std/sync/struct.Mutex.html
/// [`std::thread_local`]: ../../std/macro.thread_local.html
/// [`std::thread::current`]: ../../std/thread/fn.current.html
/// [`std::thread::park`]: ../../std/thread/fn.park.html
/// [`std::thread::Thread`]: ../../std/thread/struct.Thread.html
/// [`unpark`]: ../../std/thread/struct.Thread.html#method.unpark

#[stable(feature = "global_alloc", since = "1.28.0")]
pub unsafe trait GlobalAlloc {
    /// 按给定的 `layout` 描述分配内存。
    ///
    /// 返回指向新分配内存的指针，或返回 null 表示分配失败。返回非空指针时，
    /// 新内存块的所有权转移给调用者。
    ///
    /// # 安全性(Safety）
    ///
    /// `layout` 的大小必须非零。尝试为零大小的 `layout` 分配内存会导致未定义行为。
    ///
    /// （扩展子 trait 可能会对行为提供更具体的界限，例如保证对零大小分配请求返回
    /// 哨兵地址或 null 指针。）
    ///
    /// 分配得到的内存块可能已经初始化，也可能尚未初始化。
    ///
    /// # Errors
    ///
    /// 返回 null 指针表示内存已经耗尽，或 `layout` 不满足此分配器的大小或对齐约束。
    ///
    /// 鼓励实现在内存耗尽时返回 null，而不是中止进程，但这不是严格要求。
    /// （具体来说：在一个会于内存耗尽时中止的底层原生分配库之上实现此 trait 是*合法*的。）
    ///
    /// 希望在分配错误发生时中止计算的客户端，建议调用 [`handle_alloc_error`] 函数，
    /// 而不是直接调用 `panic!` 或类似机制。
    ///
    /// [`handle_alloc_error`]: ../../alloc/alloc/fn.handle_alloc_error.html
    #[stable(feature = "global_alloc", since = "1.28.0")]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8;

    /// 使用给定的 `layout` 释放给定 `ptr` 指针处的内存块。
    ///
    /// # 安全性(Safety）
    ///
    /// 调用者必须确保：
    ///
    /// * `ptr` 是由此分配器当前已分配的内存块，并且
    ///
    /// * `layout` 与分配该内存块时使用的布局相同。
    ///
    /// 调用成功后，该内存块的所有权交还给此分配器；调用者不得再访问 `ptr`，
    /// 也不得再次释放同一内存块。否则行为是未定义的。
    #[stable(feature = "global_alloc", since = "1.28.0")]
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout);

    /// 行为类似于 `alloc`，但还会确保返回前将内容置零。
    ///
    /// # 安全性(Safety）
    ///
    /// 调用者必须确保 `layout` 的大小非零。与 `alloc` 一样，零大小的 `layout`
    /// 会导致未定义行为。不过，分配得到的内存块保证已经初始化。
    ///
    /// # Errors
    ///
    /// 与 `alloc` 一样，返回非空指针时，新内存块的所有权转移给调用者；返回 null
    /// 指针表示内存已经耗尽，或 `layout` 不满足分配器的大小或对齐约束。
    ///
    /// 希望在分配错误发生时中止计算的客户端，建议调用 [`handle_alloc_error`] 函数，
    /// 而不是直接调用 `panic!` 或类似机制。
    ///
    /// [`handle_alloc_error`]: ../../alloc/alloc/fn.handle_alloc_error.html
    #[stable(feature = "global_alloc", since = "1.28.0")]
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let size = layout.size();
        // SAFETY: 调用者必须满足 `alloc` 的安全契约，尤其是 `layout` 的大小非零。
        let ptr = unsafe { self.alloc(layout) };
        if !ptr.is_null() {
            // SAFETY: 分配已经成功，因此从 `ptr` 开始、长度为 `size` 的区域保证可写。
            unsafe { ptr::write_bytes(ptr, 0, size) };
        }
        ptr
    }

    /// 将一个内存块收缩或增长到给定的 `new_size` 字节。
    /// 该内存块由给定的 `ptr` 指针和 `layout` 描述。
    ///
    /// 如果此方法返回非空指针，则 `ptr` 所引用内存块的所有权已经转移给此分配器。
    /// 任何对旧 `ptr` 的访问都是未定义行为，即使分配仍然保留在原地也是如此。
    /// 新返回的指针现在是访问这块内存的唯一有效指针，并且新内存块的所有权转移给调用者。
    ///
    /// 新内存块使用 `layout` 分配，但其 `size` 更新为 `new_size` 字节。
    /// 使用 `dealloc` 释放新内存块时，必须使用这个新布局。新内存块的
    /// `0..min(layout.size(), new_size)` 范围保证与原内存块具有相同的值。
    ///
    /// 如果此方法返回 null，则该内存块的所有权没有转移给此分配器，
    /// 并且该内存块的内容保持不变。
    ///
    /// # 安全性(Safety）
    ///
    /// 调用者必须确保：
    ///
    /// * `ptr` 由此分配器分配，
    ///
    /// * `layout` 与分配该内存块时使用的布局相同，
    ///
    /// * `new_size` 大于零。
    ///
    /// * `new_size` 向上取整到最接近的 `layout.align()` 倍数时不会溢出 `isize`
    ///   （也就是说，取整后的值必须小于或等于 `isize::MAX`）。
    ///
    /// 如果不满足这些条件，行为是未定义的。
    ///
    /// （扩展子 trait 可能会对行为提供更具体的界限，例如保证对零大小分配请求返回
    /// 哨兵地址或 null 指针。）
    ///
    /// # Errors
    ///
    /// 如果新布局不满足分配器的大小和对齐约束，或重新分配因其他原因失败，则返回 null。
    ///
    /// 鼓励实现在内存耗尽时返回 null，而不是 panic 或中止进程，但这不是严格要求。
    /// （具体来说：在一个会于内存耗尽时中止的底层原生分配库之上实现此 trait 是*合法*的。）
    ///
    /// 希望在重新分配错误发生时中止计算的客户端，建议调用 [`handle_alloc_error`] 函数，
    /// 而不是直接调用 `panic!` 或类似机制。
    ///
    /// [`handle_alloc_error`]: ../../alloc/alloc/fn.handle_alloc_error.html
    #[stable(feature = "global_alloc", since = "1.28.0")]
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: 调用者必须保证 `new_size` 按 `layout.align()` 向上取整后不溢出；
        // `layout.align()` 来自一个 `Layout`，因此保证是有效对齐。
        let new_layout = unsafe { Layout::from_size_align_unchecked(new_size, layout.align()) };
        // SAFETY: 调用者必须保证 `new_size` 大于零，因此 `new_layout` 满足 `alloc` 的非零要求。
        let new_ptr = unsafe { self.alloc(new_layout) };
        if !new_ptr.is_null() {
            // SAFETY: 调用者保证 `ptr` 和 `layout` 描述一个由此分配器当前分配的块；
            // 新分配的块不会与该旧块重叠，且复制长度不超过两个块的大小。
            // 随后按原 `layout` 释放旧块，把旧块所有权交还给分配器。
            unsafe {
                ptr::copy_nonoverlapping(ptr, new_ptr, cmp::min(layout.size(), new_size));
                self.dealloc(ptr, layout);
            }
        }
        new_ptr
    }
}
