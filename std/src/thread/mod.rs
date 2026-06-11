//! 原生线程。
//!
//! ## 线程模型（The threading model）
//!
//! 一个正在执行的 Rust 程序由一组原生的操作系统线程组成，每个线程都拥有自己的
//! 栈和本地状态。线程可以被命名，并为底层同步提供一些内建支持。
//!
//! 线程之间的通信可以通过 [channels]（Rust 的消息传递类型）来完成，也可以通过
//! [其他形式的线程同步](../../std/sync/index.html)以及共享内存的数据结构来完成。
//! 特别地，那些保证线程安全的类型可以借助原子引用计数容器 [`Arc`]
//! 轻松地在线程之间共享。
//!
//! Rust 中致命的逻辑错误会引发 *线程 panic*，在 panic 期间线程会展开（unwind）
//! 自己的栈，运行析构函数并释放所拥有的资源。虽然 panic 并不打算作为一种
//! 'try/catch' 机制，但 Rust 中的 panic 仍然可以被
//! [`catch_unwind`](../../std/panic/fn.catch_unwind.html) 捕获（除非以
//! `panic=abort` 编译）并从中恢复，或者用
//! [`resume_unwind`](../../std/panic/fn.resume_unwind.html) 重新抛出。如果
//! panic 没有被捕获，线程就会退出，但这个 panic 可以选择性地由另一个线程通过
//! [`join`] 来检测到。如果主线程发生 panic 且 panic 未被捕获，应用程序将以
//! 非零退出码退出。
//!
//! 当 Rust 程序的主线程终止时，整个程序会随之关闭，即使其他线程仍在运行。不过，
//! 本模块提供了便利的设施，用于自动等待某个线程终止（即 join）。
//!
//! ## 派生线程（Spawning a thread）
//!
//! 可以使用 [`thread::spawn`][`spawn`] 函数派生一个新线程：
//!
//! ```rust
//! use std::thread;
//!
//! thread::spawn(move || {
//!     // 此处是一些工作
//! });
//! ```
//!
//! 在这个例子中，派生出来的线程是“分离的”（detached），意味着程序无法得知派生
//! 线程在何时完成或以其他方式终止。
//!
//! 要想知道线程何时完成，就需要捕获 [`spawn`] 调用返回的 [`JoinHandle`] 对象，
//! 它提供了一个 `join` 方法，让调用者可以等待派生线程的完成：
//!
//! ```rust
//! use std::thread;
//!
//! let thread_join_handle = thread::spawn(move || {
//!     // 此处是一些工作
//! });
//! // 此处是一些工作
//! let res = thread_join_handle.join();
//! ```
//!
//! [`join`] 方法返回一个 [`thread::Result`]：若线程正常完成，则为包含派生线程
//! 最终产生值的 [`Ok`]；若线程发生 panic，则为包含传给 [`panic!`] 调用的那个值
//! 的 [`Err`]。
//!
//! 注意，派生新线程的线程与被派生的线程之间没有父/子关系。特别地，除非派生方是
//! 主线程，否则被派生的线程既可能比派生它的线程活得更久，也可能更短。
//!
//! ## 配置线程（Configuring threads）
//!
//! 在派生线程之前，可以通过 [`Builder`] 类型对其进行配置，目前它允许你设置线程
//! 的名字和栈大小：
//!
//! ```rust
//! # #![allow(unused_must_use)]
//! use std::thread;
//!
//! thread::Builder::new().name("thread1".to_string()).spawn(move || {
//!     println!("Hello, world!");
//! });
//! ```
//!
//! ## `Thread` 类型（The `Thread` type）
//!
//! 线程通过 [`Thread`] 类型来表示，你可以通过以下两种方式之一获取它：
//!
//! * 通过派生一个新线程，例如使用 [`thread::spawn`][`spawn`] 函数，并在返回的
//!   [`JoinHandle`] 上调用 [`thread`][`JoinHandle::thread`]。
//! * 通过 [`thread::current`] 函数请求当前线程。
//!
//! [`thread::current`] 函数即便对于并非由本模块 API 派生的线程也是可用的。
//!
//! ## 线程局部存储（Thread-local storage）
//!
//! 本模块还为 Rust 程序提供了线程局部存储的一种实现。线程局部存储是一种将数据
//! 存入全局变量的方式，使得程序中每个线程都拥有它自己的一份副本。线程之间不共享
//! 这份数据，因此对它的访问无需同步。
//!
//! 线程局部键拥有它所包含的值，并会在线程退出时销毁该值。它由 [`thread_local!`]
//! 宏创建，可以容纳任何 `'static` 的值（不含借用指针）。它提供了一个访问器函数
//! [`with`]，会把对该值的共享引用交给指定的闭包。线程局部键只允许对值进行共享
//! 访问，因为如果允许可变借用，将无法保证唯一性。大多数值会希望借助 [`Cell`] 或
//! [`RefCell`] 类型以某种形式的**内部可变性（interior mutability）**来使用。
//!
//! ## 为线程命名（Naming threads）
//!
//! 线程可以拥有关联的名字，用于标识目的。默认情况下，派生出来的线程是未命名的。
//! 要为线程指定名字，请用 [`Builder`] 构建线程并把期望的线程名传给
//! [`Builder::name`]。要在线程内部获取线程名，请使用 [`Thread::name`]。下面是
//! 几个会用到线程名的例子：
//!
//! * 如果某个具名线程中发生 panic，线程名会被打印在 panic 消息中。
//! * 在适用的平台上，线程名会被提供给操作系统（例如类 unix 平台上的
//!   `pthread_setname_np`）。
//!
//! ## 栈大小（Stack size）
//!
//! 默认栈大小与平台相关，并且可能变化。目前在所有 Tier-1 平台上为 2 MiB。
//!
//! 有两种方式可以手动为派生线程指定栈大小：
//!
//! * 用 [`Builder`] 构建线程并把期望的栈大小传给 [`Builder::stack_size`]。
//! * 把 `RUST_MIN_STACK` 环境变量设置为一个整数，表示期望的栈大小（以字节为
//!   单位）。注意设置 [`Builder::stack_size`] 会覆盖它。还要注意，`RUST_MIN_STACK`
//!   的变更在程序启动之后可能会被忽略。
//!
//! 注意主线程的栈大小**不是**由 Rust 决定的。
//!
//! [channels]: crate::sync::mpsc
//! [`Arc`]: crate::sync::Arc
//! [`join`]: JoinHandle::join
//! [`Result`]: crate::result::Result
//! [`Ok`]: crate::result::Result::Ok
//! [`Err`]: crate::result::Result::Err
//! [`thread::current`]: current::current
//! [`thread::Result`]: Result
//! [`unpark`]: Thread::unpark
//! [`thread::park_timeout`]: park_timeout
//! [`Cell`]: crate::cell::Cell
//! [`RefCell`]: crate::cell::RefCell
//! [`with`]: LocalKey::with
//! [`thread_local!`]: crate::thread_local

#![stable(feature = "rust1", since = "1.0.0")]
#![deny(unsafe_op_in_unsafe_fn)]
// 在 `test` 下，`__FastLocalKeyInner` 似乎未被使用。
#![cfg_attr(test, allow(dead_code))]

use crate::any::Any;

#[macro_use]
mod local;
mod builder;
mod current;
mod functions;
mod id;
mod join_handle;
mod lifecycle;
mod scoped;
mod spawnhook;
mod thread;

pub(crate) mod main_thread;

#[cfg(all(test, not(any(target_os = "emscripten", target_os = "wasi"))))]
mod tests;

#[stable(feature = "rust1", since = "1.0.0")]
pub use builder::Builder;
#[stable(feature = "rust1", since = "1.0.0")]
pub use current::current;
#[unstable(feature = "current_thread_id", issue = "147194")]
pub use current::current_id;
pub(crate) use current::{current_or_unnamed, current_os_id, drop_current, with_current_name};
#[stable(feature = "available_parallelism", since = "1.59.0")]
pub use functions::available_parallelism;
#[stable(feature = "park_timeout", since = "1.4.0")]
pub use functions::park_timeout;
#[stable(feature = "thread_sleep", since = "1.4.0")]
pub use functions::sleep;
#[unstable(feature = "thread_sleep_until", issue = "113752")]
pub use functions::sleep_until;
#[expect(deprecated)]
#[stable(feature = "rust1", since = "1.0.0")]
pub use functions::{panicking, park, park_timeout_ms, sleep_ms, spawn, yield_now};
#[stable(feature = "thread_id", since = "1.19.0")]
pub use id::ThreadId;
#[stable(feature = "rust1", since = "1.0.0")]
pub use join_handle::JoinHandle;
pub(crate) use lifecycle::ThreadInit;
#[stable(feature = "rust1", since = "1.0.0")]
pub use local::{AccessError, LocalKey};
#[stable(feature = "scoped_threads", since = "1.63.0")]
pub use scoped::{Scope, ScopedJoinHandle, scope};
#[unstable(feature = "thread_spawn_hook", issue = "132951")]
pub use spawnhook::add_spawn_hook;
#[stable(feature = "rust1", since = "1.0.0")]
pub use thread::Thread;

// thread_local!{} 宏所使用的实现细节。
#[doc(hidden)]
#[unstable(feature = "thread_local_internals", issue = "none")]
pub mod local_impl {
    pub use super::local::thread_local_process_attrs;
    pub use crate::sys::thread_local::*;
}

/// 线程专用的一种特化 [`Result`] 类型。
///
/// 它表示线程退出的方式。
///
/// `Result::Err` 变体中所包含的值，就是线程 panic 时所携带的值；也就是说，
/// 是调用 `panic!` 宏时传入的参数。与普通错误不同，这个值并不实现
/// [`Error`](crate::error::Error) trait。
///
/// 因此，处理线程 panic 的一种合理做法是要么：
///
/// 1. 用 [`std::panic::resume_unwind`] 重新抛出该 panic；
/// 2. 或者，如果该线程被设计为一个子系统边界、用于隔离系统级故障，则对 `Err`
/// 变体进行 match 并以适当方式处理该 panic。
///
/// 一个没有发生 panic 就完成的线程被视为成功退出。
///
/// # Examples
///
/// 对一个被 join 的线程的结果进行 match：
///
/// ```no_run
/// use std::{fs, thread, panic};
///
/// fn copy_in_thread() -> thread::Result<()> {
///     thread::spawn(|| {
///         fs::copy("foo.txt", "bar.txt").unwrap();
///     }).join()
/// }
///
/// fn main() {
///     match copy_in_thread() {
///         Ok(_) => println!("copy succeeded"),
///         Err(e) => panic::resume_unwind(e),
///     }
/// }
/// ```
///
/// [`Result`]: crate::result::Result
/// [`std::panic::resume_unwind`]: crate::panic::resume_unwind
#[stable(feature = "rust1", since = "1.0.0")]
#[doc(search_unbox)]
pub type Result<T> = crate::result::Result<T, Box<dyn Any + Send + 'static>>;

fn _assert_sync_and_send() {
    fn _assert_both<T: Send + Sync>() {}
    _assert_both::<JoinHandle<()>>();
    _assert_both::<Thread>();
}
