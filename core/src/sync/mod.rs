//! 同步原语(synchronization primitives）。
//!
//! 本模块是 `core`(无运行时、无堆分配、`no_std` 可用）层面的并发基石,
//! 只提供两类不依赖操作系统的原语:
//!
//! - [`atomic`]:原子类型(`AtomicUsize`/`AtomicBool`/`AtomicPtr` 等）。它们直接
//!   映射到目标平台的原子指令,是 `std::sync::Mutex`、`RwLock`、`Arc` 引用计数、
//!   `Once` 等上层同步设施的底层依赖。原子操作本身不会“失败”——没有 `Result`
//!   返回,数据竞争由调用方通过正确选择内存序(`Ordering`)来避免;选错内存序
//!   不会被编译器或运行时报错,而是表现为难以复现的并发 bug。
//! - [`Exclusive`]:编译期版本的“互斥”包装,借助借用检查器保证同一时刻只存在
//!   一个 `&mut`,从而无条件实现 `Sync`。
//!
//! 这两类原语都处于并发程序的热路径上(锁、引用计数、无锁数据结构都建立在
//! 原子之上),因此实现以零成本为目标:原子操作通常内联为单条机器指令,
//! `Exclusive` 是 `repr(transparent)` 的零开销包装。

#![stable(feature = "rust1", since = "1.0.0")]

pub mod atomic;
mod exclusive;
#[unstable(feature = "exclusive_wrapper", issue = "98407")]
pub use exclusive::Exclusive;
