//! 内存分配相关的 API。
//!
//! 本模块维护进程级的“全局内存分配”机制：在一个给定程序中，标准库拥有唯一一个
//! “全局（global）”内存分配器，例如 `Box<T>` 和 `Vec<T>` 都通过它来分配内存。
//! `alloc` crate（通过 `pub use alloc_crate::alloc::*` 重导出）提供 `GlobalAlloc`
//! trait 以及 `__rust_alloc` 等底层分配符号的声明，std 在此之上额外提供面向操作
//! 系统的 [`System`] 分配器实现，以及分配失败钩子（alloc error hook）等只能依赖
//! 运行时才能完成的功能。
//!
//! 目前默认的全局分配器是未指定（unspecified）的。不过对于 `cdylib`、`staticlib`
//! 这类库，可以保证默认使用 [`System`]。
//!
//! # `#[global_allocator]` 属性
//!
//! 该属性用于配置全局分配器的选择。你可以借助它实现一个完全自定义的全局分配器，
//! 把所有[^system-alloc]默认分配请求都路由到一个自定义对象上。
//!
//! ```rust
//! use std::alloc::{GlobalAlloc, System, Layout};
//!
//! struct MyAllocator;
//!
//! unsafe impl GlobalAlloc for MyAllocator {
//!     unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
//!         unsafe { System.alloc(layout) }
//!     }
//!
//!     unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
//!         unsafe { System.dealloc(ptr, layout) }
//!     }
//! }
//!
//! #[global_allocator]
//! static GLOBAL: MyAllocator = MyAllocator;
//!
//! fn main() {
//!     // 这个 `Vec` 会通过上面的 `GLOBAL` 来分配内存
//!     let mut v = Vec::new();
//!     v.push(1);
//! }
//! ```
//!
//! 该属性作用于一个 `static` 项，其类型需实现 [`GlobalAlloc`] trait。这个类型可以
//! 由外部库提供：
//!
//! ```rust,ignore (demonstrates crates.io usage)
//! use jemallocator::Jemalloc;
//!
//! #[global_allocator]
//! static GLOBAL: Jemalloc = Jemalloc;
//!
//! fn main() {}
//! ```
//!
//! `#[global_allocator]` 在一个 crate 及其递归依赖中只能使用一次。
//!
//! [^system-alloc]: 注意，Rust 标准库内部在必要时仍可能直接调用 [`System`]（例如
//! 用于实现全局分配器通常所需的那部分运行时支持，详见 [`GlobalAlloc`] 上的
//! [re-entrance]，即重入相关说明）。
//!
//! [re-entrance]: trait.GlobalAlloc.html#re-entrance

#![deny(unsafe_op_in_unsafe_fn)]
#![stable(feature = "alloc_module", since = "1.28.0")]

use core::ptr::NonNull;
use core::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
use core::{hint, mem, ptr};

#[stable(feature = "alloc_module", since = "1.28.0")]
#[doc(inline)]
pub use alloc_crate::alloc::*;

/// 由操作系统提供的默认内存分配器。
///
/// 在 Unix 平台上它基于 `malloc`，在 Windows 上基于 `HeapAlloc` 及相关函数。但
/// 是，把对底层系统分配器的直接使用与 `System` 混用是不合法的：因为本实现可能
/// 包含一些额外工作，比如为满足超过底层系统分配器直接提供的对齐能力的对齐请求而
/// 做的处理。
///
/// 该类型实现了 [`GlobalAlloc`] trait。目前默认的全局分配器是未指定的。不过对于
/// `cdylib`、`staticlib` 这类库，可以保证默认使用 [`System`]，其行为如同它们有
/// 如下定义一般：
///
/// ```rust
/// use std::alloc::System;
///
/// #[global_allocator]
/// static A: System = System;
///
/// fn main() {
///     let a = Box::new(4); // 从系统分配器分配内存。
///     println!("{a}");
/// }
/// ```
///
/// 如果你愿意，也可以围绕 `System` 定义你自己的包装器，例如用来记录已分配的总
/// 字节数：
///
/// ```rust
/// use std::alloc::{System, GlobalAlloc, Layout};
/// use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};
///
/// struct Counter;
///
/// static ALLOCATED: AtomicUsize = AtomicUsize::new(0);
///
/// unsafe impl GlobalAlloc for Counter {
///     unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
///         let ret = unsafe { System.alloc(layout) };
///         if !ret.is_null() {
///             ALLOCATED.fetch_add(layout.size(), Relaxed);
///         }
///         ret
///     }
///
///     unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
///         unsafe { System.dealloc(ptr, layout); }
///         ALLOCATED.fetch_sub(layout.size(), Relaxed);
///     }
/// }
///
/// #[global_allocator]
/// static A: Counter = Counter;
///
/// fn main() {
///     println!("allocated bytes before main: {}", ALLOCATED.load(Relaxed));
/// }
/// ```
///
/// 它也可以被直接使用，从而独立于某个 Rust 程序所选定的全局分配器来分配内存。
/// 举例来说，如果一个 Rust 程序选择使用 jemalloc 作为全局分配器，`System` 仍会
/// 通过 `malloc` 和 `HeapAlloc` 来分配内存。
#[stable(feature = "alloc_system_type", since = "1.28.0")]
#[derive(Debug, Default, Copy, Clone)]
pub struct System;

impl System {
    #[inline]
    fn alloc_impl(&self, layout: Layout, zeroed: bool) -> Result<NonNull<[u8]>, AllocError> {
        match layout.size() {
            0 => Ok(NonNull::slice_from_raw_parts(layout.dangling(), 0)),
            // SAFETY: 此分支中 `layout` 的 size 非零，
            size => unsafe {
                let raw_ptr = if zeroed {
                    GlobalAlloc::alloc_zeroed(self, layout)
                } else {
                    GlobalAlloc::alloc(self, layout)
                };
                let ptr = NonNull::new(raw_ptr).ok_or(AllocError)?;
                Ok(NonNull::slice_from_raw_parts(ptr, size))
            },
        }
    }

    // SAFETY: 与 `Allocator::grow` 相同
    #[inline]
    unsafe fn grow_impl(
        &self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
        zeroed: bool,
    ) -> Result<NonNull<[u8]>, AllocError> {
        debug_assert!(
            new_layout.size() >= old_layout.size(),
            "`new_layout.size()` must be greater than or equal to `old_layout.size()`"
        );

        match old_layout.size() {
            0 => self.alloc_impl(new_layout, zeroed),

            // SAFETY: `new_size` 非零，因为按安全条件要求 `new_size` 大于等于
            // `old_size`，而 `old_size == 0` 的情况已在上一个 match 分支处理。其余
            // 条件须由调用方维护
            old_size if old_layout.align() == new_layout.align() => unsafe {
                let new_size = new_layout.size();

                // `realloc` 大概会检查 `new_size >= old_layout.size()` 或类似条件。
                hint::assert_unchecked(new_size >= old_layout.size());

                let raw_ptr = GlobalAlloc::realloc(self, ptr.as_ptr(), old_layout, new_size);
                let ptr = NonNull::new(raw_ptr).ok_or(AllocError)?;
                if zeroed {
                    raw_ptr.add(old_size).write_bytes(0, new_size - old_size);
                }
                Ok(NonNull::slice_from_raw_parts(ptr, new_size))
            },

            // SAFETY: 因为 `new_layout.size()` 必须大于等于 `old_size`，所以新旧
            // 两块内存对于 `old_size` 个字节的读写都是有效的。又因为旧内存尚未被
            // 释放，它不会与 `new_ptr` 重叠。因此对 `copy_nonoverlapping` 的调用是
            // 安全的。`dealloc` 的安全契约须由调用方维护。
            old_size => unsafe {
                let new_ptr = self.alloc_impl(new_layout, zeroed)?;
                ptr::copy_nonoverlapping(ptr.as_ptr(), new_ptr.as_mut_ptr(), old_size);
                Allocator::deallocate(self, ptr, old_layout);
                Ok(new_ptr)
            },
        }
    }
}

// 这个 Allocator 实现会先检查 layout 的 size 非零，然后转发给 GlobalAlloc 实现，
// 后者位于 `std::sys::*::alloc` 中。
#[unstable(feature = "allocator_api", issue = "32838")]
unsafe impl Allocator for System {
    #[inline]
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        self.alloc_impl(layout, false)
    }

    #[inline]
    fn allocate_zeroed(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        self.alloc_impl(layout, true)
    }

    #[inline]
    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        if layout.size() != 0 {
            // SAFETY: 此分支中 `layout` 的 size 非零，
            // 其余条件须由调用方维护
            unsafe { GlobalAlloc::dealloc(self, ptr.as_ptr(), layout) }
        }
    }

    #[inline]
    unsafe fn grow(
        &self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, AllocError> {
        // SAFETY: 所有条件须由调用方维护
        unsafe { self.grow_impl(ptr, old_layout, new_layout, false) }
    }

    #[inline]
    unsafe fn grow_zeroed(
        &self,
        ptr: NonNull<u8>,
        old_layout: Layout,
        new_layout: Layout,
    ) -> Result<NonNull<[u8]>, AllocError> {
        // SAFETY: 所有条件须由调用方维护
        unsafe { self.grow_impl(ptr, old_layout, new_layout, true) }
    }

    #[inline]
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

        match new_layout.size() {
            // SAFETY: 条件须由调用方维护
            0 => unsafe {
                Allocator::deallocate(self, ptr, old_layout);
                Ok(NonNull::slice_from_raw_parts(new_layout.dangling(), 0))
            },

            // SAFETY: `new_size` 非零。其余条件须由调用方维护
            new_size if old_layout.align() == new_layout.align() => unsafe {
                // `realloc` 大概会检查 `new_size <= old_layout.size()` 或类似条件。
                hint::assert_unchecked(new_size <= old_layout.size());

                let raw_ptr = GlobalAlloc::realloc(self, ptr.as_ptr(), old_layout, new_size);
                let ptr = NonNull::new(raw_ptr).ok_or(AllocError)?;
                Ok(NonNull::slice_from_raw_parts(ptr, new_size))
            },

            // SAFETY: 因为 `new_size` 必须小于等于 `old_layout.size()`，所以新旧
            // 两块内存对于 `new_size` 个字节的读写都是有效的。又因为旧内存尚未被
            // 释放，它不会与 `new_ptr` 重叠。因此对 `copy_nonoverlapping` 的调用是
            // 安全的。`dealloc` 的安全契约须由调用方维护。
            new_size => unsafe {
                let new_ptr = Allocator::allocate(self, new_layout)?;
                ptr::copy_nonoverlapping(ptr.as_ptr(), new_ptr.as_mut_ptr(), new_size);
                Allocator::deallocate(self, ptr, old_layout);
                Ok(new_ptr)
            },
        }
    }
}

static HOOK: AtomicPtr<()> = AtomicPtr::new(ptr::null_mut());

/// 注册一个自定义的“分配失败钩子”（allocation error hook），并替换掉先前注册的
/// 任何钩子。
///
/// 当一次不可失败（infallible）的内存分配失败时——也就是作为调用
/// [`handle_alloc_error`] 的后果——会在运行时中止（abort）之前调用该分配失败钩子。
///
/// 分配失败钩子是一个全局资源。可以使用 [`take_alloc_error_hook`] 取回先前注册的
/// 钩子，从而对其进行包装或丢弃。
///
/// # 所提供的 `hook` 函数应当预期什么
///
/// 钩子函数会收到一个 [`Layout`] 结构体，其中包含了关于这次失败分配的信息。
///
/// 钩子函数可以选择 panic 或 abort；如果它正常返回，则会立即触发一次 abort。
///
/// 由于 [`take_alloc_error_hook`] 是一个允许取回钩子的安全函数，因此即便此前从未
/// 尝试过任何内存分配，调用该钩子函数也必须是 _健全的（sound）_。
///
/// # 默认钩子
///
/// 如果从未调用过 [`set_alloc_error_hook`]，则使用默认钩子，它会向标准错误输出
/// (standard error) 打印一条消息（然后返回，从而导致运行时中止进程）。编译器选项
/// 可能会使它改为 panic，并且在 Rust 未来的版本中默认行为也可能改为 panic。
///
/// # 示例
///
/// ```
/// #![feature(alloc_error_hook)]
///
/// use std::alloc::{Layout, set_alloc_error_hook};
///
/// fn custom_alloc_error_hook(layout: Layout) {
///    panic!("memory allocation of {} bytes failed", layout.size());
/// }
///
/// set_alloc_error_hook(custom_alloc_error_hook);
/// ```
#[unstable(feature = "alloc_error_hook", issue = "51245")]
pub fn set_alloc_error_hook(hook: fn(Layout)) {
    HOOK.store(hook as *mut (), Ordering::Release);
}

/// 注销当前的分配失败钩子，并将其返回。
///
/// *另见函数 [`set_alloc_error_hook`]。*
///
/// 如果没有注册任何自定义钩子，则返回默认钩子。
#[unstable(feature = "alloc_error_hook", issue = "51245")]
pub fn take_alloc_error_hook() -> fn(Layout) {
    let hook = HOOK.swap(ptr::null_mut(), Ordering::Acquire);
    if hook.is_null() { default_alloc_error_hook } else { unsafe { mem::transmute(hook) } }
}

#[optimize(size)]
fn default_alloc_error_hook(layout: Layout) {
    if cfg!(panic = "immediate-abort") {
        return;
    }

    // 这是发生 OOM（内存耗尽）时所走的默认路径，也是在 stable 上使用 std 时唯一
    // 会走的路径。关键在于，它*不会*调用任何用户自定义代码，因此用户无需担心分配
    // 失败会引发重入（reentrancy）问题。这一点使它有别于 alloc 中定义的默认
    // `__rdl_alloc_error_handler`（即在没有 `#[alloc_error_handler]` 时被调用的那个
    // 默认分配错误处理器）：后者会触发一次常规 panic，从而可能调用用户自定义的
    // panic 钩子，执行任意用户自定义代码。

    static PREV_ALLOC_FAILURE: AtomicBool = AtomicBool::new(false);
    if PREV_ALLOC_FAILURE.swap(true, Ordering::Relaxed) {
        // 如果先前已经发生过一次分配失败，就不要再尝试打印回溯。这很可能意味着已经
        // 没有足够内存来打印回溯，不过也可能意味着两个线程并发地耗尽了内存。
        rtprintpanic!(
            "memory allocation of {} bytes failed\nskipping backtrace printing to avoid potential recursion\n",
            layout.size()
        );
        return;
    } else {
        rtprintpanic!("memory allocation of {} bytes failed\n", layout.size());
    }

    let Some(mut out) = crate::sys::stdio::panic_output() else {
        return;
    };

    // 使用锁来防止多线程环境下输出相互混杂。
    // 某些平台在打印回溯时也需要这个锁，比如 Windows 上的 `SymFromAddr`。
    // 务必在检查完 PREV_ALLOC_FAILURE 之后再获取此锁，以避免在内存太少、无法打印
    // 回溯时发生死锁。
    let mut lock = crate::sys::backtrace::lock();

    match crate::panic::get_backtrace_style() {
        Some(crate::panic::BacktraceStyle::Short) => {
            drop(lock.print(&mut out, crate::backtrace_rs::PrintFmt::Short))
        }
        Some(crate::panic::BacktraceStyle::Full) => {
            drop(lock.print(&mut out, crate::backtrace_rs::PrintFmt::Full))
        }
        Some(crate::panic::BacktraceStyle::Off) => {
            use crate::io::Write;
            let _ = writeln!(
                out,
                "note: run with `RUST_BACKTRACE=1` environment variable to display a \
                             backtrace"
            );
            if cfg!(miri) {
                let _ = writeln!(
                    out,
                    "note: in Miri, you may have to set `MIRIFLAGS=-Zmiri-env-forward=RUST_BACKTRACE` \
                                for the environment variable to have an effect"
                );
            }
        }
        // 如果不支持回溯或回溯被强制关闭，则什么也不做。
        None => {}
    }
}

#[cfg(not(test))]
#[doc(hidden)]
#[alloc_error_handler]
#[unstable(feature = "alloc_internals", issue = "none")]
pub fn rust_oom(layout: Layout) -> ! {
    crate::sys::backtrace::__rust_end_short_backtrace(|| {
        let hook = HOOK.load(Ordering::Acquire);
        let hook: fn(Layout) =
            if hook.is_null() { default_alloc_error_hook } else { unsafe { mem::transmute(hook) } };
        hook(layout);
        crate::process::abort()
    })
}

#[cfg(not(test))]
#[doc(hidden)]
#[allow(unused_attributes)]
#[unstable(feature = "alloc_internals", issue = "none")]
pub mod __default_lib_allocator {
    use super::{GlobalAlloc, Layout, System};
    // 这些“魔法”符号名在没有 `#[global_allocator]` 属性时，被用作实现
    // `__rust_alloc` 等符号（参见 `src/liballoc/alloc.rs`）的后备（fallback）方案。

    // 符号名见 src/librustc_ast/expand/allocator.rs
    // 函数签名见 src/librustc_allocator/lib.rs

    // 链接指示（linkage directives）作为当前编译器分配器 ABI 的一部分提供

    #[rustc_std_internal_symbol]
    pub unsafe extern "C" fn __rdl_alloc(size: usize, align: usize) -> *mut u8 {
        // SAFETY: 见 `Layout::from_size_align` 和 `GlobalAlloc::alloc` 所要求的保证。
        unsafe {
            let layout = Layout::from_size_align_unchecked(size, align);
            System.alloc(layout)
        }
    }

    #[rustc_std_internal_symbol]
    pub unsafe extern "C" fn __rdl_dealloc(ptr: *mut u8, size: usize, align: usize) {
        // SAFETY: 见 `Layout::from_size_align` 和 `GlobalAlloc::dealloc` 所要求的保证。
        unsafe { System.dealloc(ptr, Layout::from_size_align_unchecked(size, align)) }
    }

    #[rustc_std_internal_symbol]
    pub unsafe extern "C" fn __rdl_realloc(
        ptr: *mut u8,
        old_size: usize,
        align: usize,
        new_size: usize,
    ) -> *mut u8 {
        // SAFETY: 见 `Layout::from_size_align` 和 `GlobalAlloc::realloc` 所要求的保证。
        unsafe {
            let old_layout = Layout::from_size_align_unchecked(old_size, align);
            System.realloc(ptr, old_layout, new_size)
        }
    }

    #[rustc_std_internal_symbol]
    pub unsafe extern "C" fn __rdl_alloc_zeroed(size: usize, align: usize) -> *mut u8 {
        // SAFETY: 见 `Layout::from_size_align` 和 `GlobalAlloc::alloc_zeroed` 所要求的保证。
        unsafe {
            let layout = Layout::from_size_align_unchecked(size, align);
            System.alloc_zeroed(layout)
        }
    }
}
