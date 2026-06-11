//! 实用的同步原语（synchronization primitives）。
//!
//! ## 为什么需要同步（The need for synchronization）
//!
//! 从概念上讲，一个 Rust 程序是一连串将在计算机上执行的操作。程序中
//! 各事件发生的时间线，与代码中各操作的先后顺序是一致的。
//!
//! 考虑下面这段对若干全局静态变量进行操作的代码：
//!
//! ```rust
//! // FIXME(static_mut_refs): 不要触发 `static_mut_refs` lint
//! #![allow(static_mut_refs)]
//!
//! static mut A: u32 = 0;
//! static mut B: u32 = 0;
//! static mut C: u32 = 0;
//!
//! fn main() {
//!     unsafe {
//!         A = 3;
//!         B = 4;
//!         A = A + B;
//!         C = B;
//!         println!("{A} {B} {C}");
//!         C = A;
//!     }
//! }
//! ```
//!
//! 表面上看，似乎是：内存中存储的某些变量被改写，执行了一次加法，
//! 结果存入 `A`，而变量 `C` 被修改了两次。
//!
//! 当只涉及单个线程时，结果符合预期：打印出 `7 4 4` 这一行。
//!
//! 至于幕后发生了什么：当启用优化后，最终生成的机器码可能与源代码
//! 看起来大相径庭：
//!
//! - 对 `C` 的第一次存储可能被移动到对 `A` 或 `B` 的存储之前，
//!   _仿佛_ 我们写的是 `C = 4; A = 3; B = 4`。
//!
//! - 把 `A + B` 赋值给 `A` 的操作可能被消除，因为这个和可以一直存放在
//!   某个临时位置直到被打印出来，而那个全局变量从始至终都没被更新过。
//!
//! - 最终结果完全可以在编译期仅凭代码就确定下来，于是 [constant folding]
//!   （常量折叠）可能把整个代码块变成一句简单的 `println!("7 4 4")`。
//!
//! 编译器允许执行上述任意优化组合，只要最终优化后的代码在执行时
//! 产生的结果与未经优化的版本一致即可。
//!
//! 由于现代计算机所涉及的 [concurrency]（并发性），关于程序执行顺序的
//! 假设往往是错误的。对全局变量的访问可能导致不确定的结果，**即便**
//! 关闭了编译器优化也是如此，而且引入同步 bug **仍然是可能的**。
//!
//! 注意，得益于 Rust 的安全保证，访问全局（静态）变量需要 `unsafe`
//! 代码——前提是我们没有使用本模块中任何同步原语。
//!
//! [constant folding]: https://en.wikipedia.org/wiki/Constant_folding
//! [concurrency]: https://en.wikipedia.org/wiki/Concurrency_(computer_science)
//!
//! ## 乱序执行（Out-of-order execution）
//!
//! 出于种种原因，指令可能以不同于我们书写的顺序执行：
//!
//! - **编译器** 重排指令：如果编译器能把某条指令提前发射，它就会尝试
//!   这么做。例如，它可能把内存加载操作提升到代码块顶部，好让 CPU
//!   提前从内存 [prefetching]（预取）这些值。
//!
//!   在单线程场景下，这在编写信号处理函数或某些底层代码时可能引发问题。
//!   可使用 [compiler fences]（编译器栅栏）来阻止这种重排。
//!
//! - **单个处理器** [out-of-order]（乱序）执行指令：现代 CPU 具备
//!   [superscalar]（超标量）执行能力，即多条指令可以同时执行，
//!   尽管机器码描述的是一个顺序过程。
//!
//!   这种重排由 CPU 透明地处理。
//!
//! - **多处理器** 系统同时执行多个硬件线程：在多线程场景下，你可以
//!   使用两类原语来处理同步：
//!   - [memory fences]（内存栅栏）以确保内存访问按正确顺序对其他
//!   CPU 可见。
//!   - [atomic operations]（原子操作）以确保对同一内存位置的并发访问
//!   不会导致未定义行为。
//!
//! [prefetching]: https://en.wikipedia.org/wiki/Cache_prefetching
//! [compiler fences]: crate::sync::atomic::compiler_fence
//! [out-of-order]: https://en.wikipedia.org/wiki/Out-of-order_execution
//! [superscalar]: https://en.wikipedia.org/wiki/Superscalar_processor
//! [memory fences]: crate::sync::atomic::fence
//! [atomic operations]: crate::sync::atomic
//!
//! ## 更高层的同步对象（Higher-level synchronization objects）
//!
//! 大多数底层同步原语相当易错且使用不便，这正是标准库还额外暴露了
//! 一些更高层同步对象的原因。
//!
//! 这些抽象可以由更底层的原语构建而成。出于效率考虑，标准库中的同步
//! 对象通常借助操作系统内核来实现——内核能够在线程因获取锁而阻塞时
//! 重新调度它们。
//!
//! 下面是可用同步对象的概览：
//!
//! - [`Arc`]：Atomically Reference-Counted（原子引用计数）指针，可在
//!   多线程环境中延长某些数据的生命周期，直到所有线程都用完它为止。
//!
//! - [`Barrier`]：确保多个线程相互等待，都到达程序中某一点后，再一起
//!   继续执行。
//!
//! - [`Condvar`]：Condition Variable（条件变量），提供在等待某事件发生
//!   时阻塞线程的能力。
//!
//! - [`mpsc`]：Multi-producer, single-consumer（多生产者、单消费者）
//!   队列，用于基于消息的通信。能以一些额外内存为代价，提供轻量的
//!   线程间同步机制。
//!
//! - [`mpmc`]：Multi-producer, multi-consumer（多生产者、多消费者）
//!   队列，用于基于消息的通信。能以一些额外内存为代价，提供轻量的
//!   线程间同步机制。
//!
//! - [`Mutex`]：Mutual Exclusion（互斥）机制，确保任一时刻至多一个
//!   线程能访问某些数据。
//!
//! - [`Once`]：用于线程安全的一次性全局初始化例程。主要用于实现
//!   [`OnceLock`] 这类其他类型。
//!
//! - [`OnceLock`]：用于变量的线程安全一次性初始化，不同调用方可以
//!   提供各自不同的初始化器。
//!
//! - [`LazyLock`]：用于变量的线程安全一次性初始化，使用创建时提供的
//!   一个无参（nullary）初始化函数。
//!
//! - [`RwLock`]：提供一种互斥机制，允许多个读者同时读，而同一时刻
//!   只允许一个写者。在某些情况下，这比互斥锁（mutex）更高效。
//!
//! [`Arc`]: crate::sync::Arc
//! [`Barrier`]: crate::sync::Barrier
//! [`Condvar`]: crate::sync::Condvar
//! [`mpmc`]: crate::sync::mpmc
//! [`mpsc`]: crate::sync::mpsc
//! [`Mutex`]: crate::sync::Mutex
//! [`Once`]: crate::sync::Once
//! [`OnceLock`]: crate::sync::OnceLock
//! [`RwLock`]: crate::sync::RwLock

#![stable(feature = "rust1", since = "1.0.0")]

// 不做格式化：本文件只是一堆重导出（re-exports），它们的顺序值得保留。
#![cfg_attr(rustfmt, rustfmt::skip)]

// 以下这些来自 `core` 与 `alloc`，且只有一种风味（flavor）：不带中毒（no poisoning）。
#[unstable(feature = "exclusive_wrapper", issue = "98407")]
pub use core::sync::Exclusive;
#[stable(feature = "rust1", since = "1.0.0")]
pub use core::sync::atomic;

#[unstable(feature = "unique_rc_arc", issue = "112566")]
pub use alloc_crate::sync::UniqueArc;
#[stable(feature = "rust1", since = "1.0.0")]
pub use alloc_crate::sync::{Arc, Weak};

#[unstable(feature = "mpmc_channel", issue = "126840")]
pub mod mpmc;
pub mod mpsc;
#[unstable(feature = "oneshot_channel", issue = "143674")]
pub mod oneshot;

pub(crate) mod once; // 标为 `pub(crate)` 以供 `sys::sync::once` 的具体实现以及 `LazyLock` 使用。

#[stable(feature = "rust1", since = "1.0.0")]
pub use self::once::{Once, OnceState};

#[stable(feature = "rust1", since = "1.0.0")]
#[doc(inline)]
#[expect(deprecated)]
pub use self::once::ONCE_INIT;

mod barrier;
mod lazy_lock;
mod once_lock;
mod reentrant_lock;

// 以下这些只有一种风味：不带中毒（no poisoning）。
#[stable(feature = "rust1", since = "1.0.0")]
pub use self::barrier::{Barrier, BarrierWaitResult};
#[stable(feature = "lazy_cell", since = "1.80.0")]
pub use self::lazy_lock::LazyLock;
#[stable(feature = "once_cell", since = "1.70.0")]
pub use self::once_lock::OnceLock;
#[unstable(feature = "reentrant_lock", issue = "121440")]
pub use self::reentrant_lock::{ReentrantLock, ReentrantLockGuard};

// 注意：将来我们会在某个 edition 上把 `std::sync` 的默认版本切换为不带中毒
// （non-poisoning）的版本。
// 详见 https://github.com/rust-lang/rust/issues/134645#issuecomment-3324577500

#[unstable(feature = "sync_nonpoison", issue = "134645")]
pub mod nonpoison;
#[unstable(feature = "sync_poison_mod", issue = "134646")]
pub mod poison;

// FIXME(sync_poison_mod): 一旦这些模块稳定下来，移除所有 `#[doc(inline)]`。

// 以下这些只存在于带中毒（poisoning）的版本中。
#[stable(feature = "rust1", since = "1.0.0")]
#[doc(inline)]
pub use self::poison::{LockResult, PoisonError};

// 以下这些两种风味都存在：带中毒与不带中毒。
// 历史上的默认值是带中毒的版本。
#[stable(feature = "rust1", since = "1.0.0")]
#[doc(inline)]
pub use self::poison::{
    TryLockError, TryLockResult,
    Mutex, MutexGuard,
    RwLock, RwLockReadGuard, RwLockWriteGuard,
    Condvar,
};

#[unstable(feature = "mapped_lock_guards", issue = "117108")]
#[doc(inline)]
pub use self::poison::{MappedMutexGuard, MappedRwLockReadGuard, MappedRwLockWriteGuard};

/// 用于表明在条件变量上的限时等待（timed wait）是否因超时而返回的类型。
///
/// 它由 [`wait_timeout`] 方法返回。
///
/// [`wait_timeout`]: Condvar::wait_timeout
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
#[stable(feature = "wait_timeout", since = "1.5.0")]
pub struct WaitTimeoutResult(bool);

impl WaitTimeoutResult {
    /// 如果可以确定本次等待已超时，则返回 `true`。
    ///
    /// # 示例
    ///
    /// 本例派生一个线程，它会先睡眠 20 毫秒，然后更新一个布尔值，
    /// 接着通知条件变量。
    ///
    /// 主线程将以 10 毫秒的超时在条件变量上等待，并在超时后离开循环。
    ///
    /// ```
    /// use std::sync::{Arc, Condvar, Mutex};
    /// use std::thread;
    /// use std::time::Duration;
    ///
    /// let pair = Arc::new((Mutex::new(false), Condvar::new()));
    /// let pair2 = Arc::clone(&pair);
    ///
    /// # let handle =
    /// thread::spawn(move || {
    ///     let (lock, cvar) = &*pair2;
    ///
    ///     // 让我们先等待 20 毫秒，再通知条件变量。
    ///     thread::sleep(Duration::from_millis(20));
    ///
    ///     let mut started = lock.lock().unwrap();
    ///     // 我们更新这个布尔值。
    ///     *started = true;
    ///     cvar.notify_one();
    /// });
    ///
    /// // 等待该线程启动。
    /// let (lock, cvar) = &*pair;
    /// loop {
    ///     // 让我们给条件变量的 wait 加上超时。
    ///     let result = cvar.wait_timeout(lock.lock().unwrap(), Duration::from_millis(10)).unwrap();
    ///     // 10 毫秒已经过去。
    ///     if result.1.timed_out() {
    ///         // 现在已超时，我们可以离开。
    ///         break
    ///     }
    /// }
    /// # // 防止在 Miri 下发生内存泄漏。
    /// # let _ = handle.join();
    /// ```
    #[must_use]
    #[stable(feature = "wait_timeout", since = "1.5.0")]
    pub fn timed_out(&self) -> bool {
        self.0
    }
}
