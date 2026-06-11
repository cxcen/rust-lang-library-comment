//! 这是在 wasm 目标平台上、且未使用 emscripten 或 wasi 时的全局分配器实现。
//! 在那种情况下并没有真正的运行时可供我们依赖来做分配，
//! 因此我们就自己提供一个！
//!
//! wasm 指令集有两条指令用于获取当前内存量以及增长内存量。
//! 这两条指令是我们得以构建分配器的基础，所以我们就用它们来构建！
//! 注意，这两条指令本身也相当“全局”——这毕竟是“全局”分配器嘛！
//!
//! 这里目前使用的分配器是 `dlmalloc` crate，它作为子模块（submodule）被收录在
//! rust-lang/rust 仓库中。该 crate 是把 dlmalloc.c 从 C 移植到 Rust 的产物，
//! 基本上只是为了让我们目前能拥有“纯 Rust”的实现——而这在当前从技术上讲是必需的
//!（因为还不能与 C 链接）。
//!
//! 该 crate 自身提供了一个全局分配器，在 wasm 上它没有任何同步机制，
//! 因为这里没有线程！

use core::cell::SyncUnsafeCell;

use crate::alloc::{GlobalAlloc, Layout, System};

struct SyncDlmalloc(dlmalloc::Dlmalloc);
unsafe impl Sync for SyncDlmalloc {}

static DLMALLOC: SyncUnsafeCell<SyncDlmalloc> =
    SyncUnsafeCell::new(SyncDlmalloc(dlmalloc::Dlmalloc::new()));

#[stable(feature = "alloc_system_type", since = "1.28.0")]
unsafe impl GlobalAlloc for System {
    #[inline]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: 对 DLMALLOC 的访问保证是安全的，因为该锁给了我们独占且非重入的访问权。
        // 调用 malloc() 是安全的，因为本函数的前置条件与该 trait 方法的前置条件相匹配。
        let _lock = lock::lock();
        unsafe { (*DLMALLOC.get()).0.malloc(layout.size(), layout.align()) }
    }

    #[inline]
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        // SAFETY: 对 DLMALLOC 的访问保证是安全的，因为该锁给了我们独占且非重入的访问权。
        // 调用 calloc() 是安全的，因为本函数的前置条件与该 trait 方法的前置条件相匹配。
        let _lock = lock::lock();
        unsafe { (*DLMALLOC.get()).0.calloc(layout.size(), layout.align()) }
    }

    #[inline]
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: 对 DLMALLOC 的访问保证是安全的，因为该锁给了我们独占且非重入的访问权。
        // 调用 free() 是安全的，因为本函数的前置条件与该 trait 方法的前置条件相匹配。
        let _lock = lock::lock();
        unsafe { (*DLMALLOC.get()).0.free(ptr, layout.size(), layout.align()) }
    }

    #[inline]
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: 对 DLMALLOC 的访问保证是安全的，因为该锁给了我们独占且非重入的访问权。
        // 调用 realloc() 是安全的，因为本函数的前置条件与该 trait 方法的前置条件相匹配。
        let _lock = lock::lock();
        unsafe { (*DLMALLOC.get()).0.realloc(ptr, layout.size(), layout.align(), new_size) }
    }
}

#[cfg(target_feature = "atomics")]
mod lock {
    use crate::sync::atomic::Ordering::{Acquire, Release};
    use crate::sync::atomic::{Atomic, AtomicI32};

    static LOCKED: Atomic<i32> = AtomicI32::new(0);

    pub struct DropLock;

    pub fn lock() -> DropLock {
        loop {
            if LOCKED.swap(1, Acquire) == 0 {
                return DropLock;
            }
            // 好的，事情从这里开始变得有点令人沮丧了。此刻我们需要
            // 同步地获取一把锁，但我们正与某个其他线程竞争。通常我们会
            // 执行某种形式的 `i32.atomic.wait`，像这样：
            //
            //     unsafe {
            //         let r = core::arch::wasm32::i32_atomic_wait(
            //             LOCKED.as_mut_ptr(),
            //             1,  //     期望值（expected value）
            //             -1, //     超时（timeout）
            //         );
            //         debug_assert!(r == 0 || r == 1);
            //     }
            //
            // 但遗憾的是，这样做会给主线程带来问题。Web 浏览器中的主线程
            // *永远不能阻塞*，没有例外。这意味着主线程实际上无法
            // 执行 `i32.atomic.wait` 指令。
            //
            // 因此，如果我们想在浏览器的语境下工作，就需要为主线程
            // 设计出某种分配方案：当全局 malloc 锁出现竞争时，
            // 主线程要……做点什么。
            //
            // 可能的思路包括：
            //
            // 1. 尝试获取全局锁。如果失败，则退回到通过 `memory.grow`
            //    进行内存分配。之后再……以某种方式……在这块裸页（raw page）
            //    随着时间被逐步切分使用时，把它重新注入到主分配器中。
            //    这种策略的缺点是：每当主线程与其他线程发生竞争时，
            //    都会强制分配一个页，这很不理想。
            //
            // 2. 维护某种“两级（two level）”分配器方案，让主线程
            //    拥有自己的分配器。这个分配器还要以某种方式与一个全局分配器
            //    保持平衡，既要让分配能够在线程间往来，也要确保两个分配器
            //    在已释放内存等方面保持“平衡”。然而，这看起来要复杂得多。
            //
            // 由于实在没有别的好办法，这里当前实现的策略就是简单地自旋（spin）。
            // 典型的自旋循环算法在此处会向 CPU 给出某种“提示（hint）”，
            // 表明我们正在做什么，以确保 CPU 不会过热，但 wasm 并没有
            // 这样的指令。
            //
            // 说清楚：在这里自旋并不是一个好的解决方案。
            // 持有锁的另一个线程可能要过相当长的时间才会醒来。例如它可能正处于
            // `memory.grow` 之中，或者被从 CPU 上换出（evicted）一个时间片，
            // 比如 10ms。在这些时间段里，我们的线程会“热心地”坐在这儿吃掉 CPU 时间，
            // 直到它自己也被换出，或锁的持有者完成为止。这意味着我们只是在
            // 白白燃烧、浪费 CPU 时间，对谁都没有好处。
            //
            // 不过自旋确实也有不错的特性：它在语义上是正确的，对所有线程的内存
            // 分配都是公平的，而且简单到足以实现。
            //
            // 这（但愿）今后一定会被一个能够处理主线程这一限制的
            // 真正的内存分配器所取代。
            //
            //
            // FIXME: 我们或许还可以在这里加一个优化：检测某个线程
            // 是否为主线程，并对所有非主线程进行阻塞。然而目前我们
            // 没有办法知道哪个 wasm 线程位于浏览器的主线程上，但
            // 如果我们能弄清楚，至少就能在一定程度上缓解这种自旋的代价。
        }
    }

    impl Drop for DropLock {
        fn drop(&mut self) {
            let r = LOCKED.swap(0, Release);
            debug_assert_eq!(r, 1);

            // 注意，由于上面所述的逻辑，我们实际上并不需要唤醒
            // 任何线程；但假如真要唤醒，那大概会像这样：
            //
            //     unsafe {
            //         core::arch::wasm32::atomic_notify(
            //             LOCKED.as_mut_ptr(),
            //             1, //     只唤醒一个线程
            //         );
            //     }
        }
    }
}

#[cfg(not(target_feature = "atomics"))]
mod lock {
    #[inline]
    pub fn lock() {} // 没有原子操作，没有线程，那就轻松了！
}
