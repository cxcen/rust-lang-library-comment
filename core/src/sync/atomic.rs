//! 原子类型(atomic types)。
//!
//! 原子类型提供线程间最底层的共享内存通信手段,是其他一切并发类型的构造基石
//! (`Mutex`、`RwLock`、`Arc` 的引用计数都建立在它们之上)。
//!
//! 本模块为若干基本类型定义了原子版本,包括 [`AtomicBool`]、[`AtomicIsize`]、
//! [`AtomicUsize`]、[`AtomicI8`]、[`AtomicU16`] 等。只要使用得当,这些原子操作
//! 能在线程之间同步更新。
//!
//! 原子变量本身可以安全地在线程间共享(它们实现了 [`Sync`]),但它们**不**自带
//! 共享机制,而是遵循 Rust 的[线程模型](../../../std/thread/index.html#the-threading-model)。
//! 共享一个原子变量最常见的方式是把它放进 [`Arc`][arc](一个原子引用计数的共享指针)。
//!
//! [arc]: ../../../std/sync/struct.Arc.html
//!
//! 原子类型可存放于 `static` 变量中,用 [`AtomicBool::new`] 这类常量构造器初始化。
//! 原子静态量常用于全局的惰性初始化。
//!
//! ## 原子访问的内存模型
//!
//! Rust 原子目前遵循与 [C++20 原子][cpp]相同的规则,具体是 [`intro.races`][cpp-intro.races]
//! 一节的规则,但**不含** "consume" 内存序。由于 C++ 采用基于对象(object-based)的内存
//! 模型,而 Rust 是基于访问(access-based)的,需要做一点翻译:凡 C++ 说“某对象的值”,
//! 在 Rust 中理解为“一次读取所得到的字节”;C++ 说“某原子对象的值”,指的是一次原子加载
//! (本模块提供的 load 操作)的结果;“对原子对象的修改”指一次原子存储(store)。
//!
//! 最终效果**几乎**等价于:为某个 Rust 原子类型创建一个**共享引用**,对应在 C++ 中
//! 创建一个 `atomic_ref`,该 `atomic_ref` 在共享引用的生命周期结束时销毁。主要差别在于:
//! Rust 允许对同一块内存并发地进行原子读与非原子读——这在 C++ 内存模型里没有任何问题,
//! C++ 之所以禁止,只是因为它把内存划分为“原子对象”和“非原子对象”(`atomic_ref` 临时把
//! 非原子对象转成原子对象)。
//!
//! 本模型最关键的一点是:**数据竞争(data race)是未定义行为(UB)**。数据竞争定义为:
//! 相互**冲突**且**未同步**的访问,且其中至少一个是非原子访问。这里,两个访问**冲突**
//! 指它们触及重叠的内存区域且至少一个是写(一个**未成功**的 `compare_exchange` /
//! `compare_exchange_weak` 不算写)。它们**未同步**指按内存模型的 happens-before 序,
//! 二者互不 *happens-before*。
//!
//! 内存模型中另一种 UB 来源是**混合尺寸访问**:Rust 继承了 C++ 的限制——未同步且冲突的
//! 原子访问不得**部分**重叠。换言之,任意一对未同步的原子访问,要么互不相交,要么访问
//! 完全相同的内存(包括访问尺寸相同),要么二者都是读。
//!
//! 每个原子访问都接受一个 [`Ordering`] 参数,用于规定该操作如何与 happens-before 序交互。
//! 这些内存序的行为与对应的 [C++20 原子内存序][cpp_memory_order]一致。更多说明见 [nomicon]。
//!
//! [cpp]: https://en.cppreference.com/w/cpp/atomic
//! [cpp-intro.races]: https://timsong-cpp.github.io/cppwp/n4868/intro.multithread#intro.races
//! [cpp_memory_order]: https://en.cppreference.com/w/cpp/atomic/memory_order
//! [nomicon]: ../../../nomicon/atomics.html
//!
//! ```rust,no_run undefined_behavior
//! use std::sync::atomic::{AtomicU16, AtomicU8, Ordering};
//! use std::mem::transmute;
//! use std::thread;
//!
//! let atomic = AtomicU16::new(0);
//!
//! thread::scope(|s| {
//!     // UB:相互冲突且未同步的访问,其中至少一个(这里是 write)是非原子的。
//!     s.spawn(|| atomic.store(1, Ordering::Relaxed)); // 原子存储
//!     s.spawn(|| unsafe { atomic.as_ptr().write(2) }); // 非原子写
//! });
//!
//! thread::scope(|s| {
//!     // 没问题:两个访问不冲突(都没有做修改)。在 C++ 中这是禁止的,因为创建
//!     // `atomic_ref` 后就不允许再做非原子访问;Rust 没有这个限制。
//!     s.spawn(|| atomic.load(Ordering::Relaxed)); // 原子加载
//!     s.spawn(|| unsafe { atomic.as_ptr().read() }); // 非原子读
//! });
//!
//! thread::scope(|s| {
//!     // 没问题:`join` 建立了同步,使原子存储 happens-before 那次非原子写。
//!     let handle = s.spawn(|| atomic.store(1, Ordering::Relaxed)); // 原子存储
//!     handle.join().expect("thread won't panic"); // 同步点
//!     s.spawn(|| unsafe { atomic.as_ptr().write(2) }); // 非原子写
//! });
//!
//! thread::scope(|s| {
//!     // UB:未同步、相互冲突、尺寸不同的原子访问。
//!     s.spawn(|| atomic.store(1, Ordering::Relaxed));
//!     s.spawn(|| unsafe {
//!         let differently_sized = transmute::<&AtomicU16, &AtomicU8>(&atomic);
//!         differently_sized.store(2, Ordering::Relaxed);
//!     });
//! });
//!
//! thread::scope(|s| {
//!     // 没问题:`join` 建立同步,使 1 字节存储 happens-before 那次 2 字节存储。
//!     let handle = s.spawn(|| atomic.store(1, Ordering::Relaxed));
//!     handle.join().expect("thread won't panic");
//!     s.spawn(|| unsafe {
//!         let differently_sized = transmute::<&AtomicU16, &AtomicU8>(&atomic);
//!         differently_sized.store(2, Ordering::Relaxed);
//!     });
//! });
//! ```
//!
//! # 可移植性(Portability)
//!
//! 本模块所有原子类型,只要在目标平台上可用,都保证是[无锁(lock-free)][lock-free]的——
//! 即内部不会获取全局互斥锁。但原子类型与操作**不**保证无等待(wait-free):像 `fetch_or`
//! 这样的操作可能用 compare-and-swap 循环实现。
//!
//! 在指令层面,原子操作可能借助更大尺寸的原子来实现。例如某些平台用 4 字节原子指令实现
//! `AtomicI8`。注意这种模拟不影响代码的正确性,只是需要知道有这回事。
//!
//! 本模块的原子类型未必在所有平台上都可用。这里列出的原子类型都很普遍、一般可依赖其存在,
//! 但有几个值得注意的例外:
//!
//! * 指针为 32 位的 PowerPC 与 MIPS 平台没有 `AtomicU64` 或 `AtomicI64` 类型。
//! * ARMv4T、ARMv5TE 等老旧 ARM 平台对原子的硬件支持非常有限。裸机目标会完全禁用本模块,
//!   而 Linux 目标[借助内核][use the kernel]来辅助实现(伴随性能损失)。要到 ARMv6K 之后,
//!   ARM CPU 才在硬件上支持 load/store 与 Compare-and-Swap(CAS)原子。
//! * ARMv6-M 与 ARMv8-M baseline 目标(`thumbv6m-*` 和 `thumbv8m.base-*`)只提供 `load`
//!   和 `store`,不支持 `swap`、`fetch_add` 等 CAS 操作。完整 CAS 支持见于 ARMv7-M 与
//!   ARMv8-M Mainline(`thumbv7m-*`、`thumbv7em*` 和 `thumbv8m.main-*`)。
//!
//! [use the kernel]: https://www.kernel.org/doc/Documentation/arm/kernel_user_helpers.txt
//!
//! 注意未来还可能加入同样缺少某些原子操作的平台。追求最大可移植性的代码应谨慎选择使用
//! 哪些原子类型。`AtomicUsize` 与 `AtomicIsize` 通常最可移植,但即便它们也并非处处可用。
//! 作为参考,`std` 库要求存在 `AtomicBool` 和指针大小的原子,而 `core` 不作此要求。
//!
//! 可用 `#[cfg(target_has_atomic)]` 属性,根据目标支持的位宽做条件编译。它是一组键值选项,
//! 为每个受支持的尺寸设置,取值有 "8"、"16"、"32"、"64"、"128" 以及表示指针大小原子的 "ptr"。
//!
//! [lock-free]: https://en.wikipedia.org/wiki/Non-blocking_algorithm
//!
//! # 对只读内存的原子访问
//!
//! 一般而言,对只读内存的**所有**原子访问都是 UB。例如,执行一个注定失败(因而概念上是
//! 只读操作)的 `compare_exchange`,如果底层内存页被映射为只读,仍可能触发段错误。由于原子
//! `load` 可能借助 compare-exchange 实现,即使是 `load` 也可能在只读内存上出错。
//!
//! 本节中,“只读内存”指在底层目标上即为只读的内存,即页面带只读标志、任何写入都会触发缺页。
//! 特别地,一个指向**读写**映射内存的 `&u128` 引用**不**被视为指向“只读内存”。在 Rust 中,
//! 几乎所有内存都是读写的;唯一的例外是 `const` 项、或不含内部可变性的 `static` 项所创建的
//! 内存,以及由操作系统通过平台特定 API 明确标记为只读的内存。
//!
//! 作为上述通则的例外:“足够小”的、使用 `Ordering::Relaxed` 的原子加载,其实现方式能在只读
//! 内存上工作,因而不是 UB。判定加载“足够小”的确切尺寸上限随目标而异:
//!
//! | `target_arch` | 尺寸上限 |
//! |---------------|---------|
//! | `x86`、`arm`、`loongarch32`、`mips`、`mips32r6`、`powerpc`、`riscv32`、`sparc`、`hexagon` | 4 字节 |
//! | `x86_64`、`aarch64`、`loongarch64`、`mips64`、`mips64r6`、`powerpc64`、`riscv64`、`sparc64`、`s390x` | 8 字节 |
//!
//! 超过此上限的原子加载、使用 `Relaxed` 以外内存序的原子加载,以及在表中未列出目标上的**所有**
//! 原子加载,在特定条件下也可能可用于只读内存,但这不是稳定保证,不应依赖。
//!
//! 如果需要在只读内存上做 acquire 加载,可改用“relaxed 加载 + acquire 栅栏(fence)”的组合。
//!
//! # 示例
//!
//! 一个简单的自旋锁(spinlock):
//!
//! ```ignore-wasm
//! use std::sync::Arc;
//! use std::sync::atomic::{AtomicUsize, Ordering};
//! use std::{hint, thread};
//!
//! fn main() {
//!     let spinlock = Arc::new(AtomicUsize::new(1));
//!
//!     let spinlock_clone = Arc::clone(&spinlock);
//!
//!     let thread = thread::spawn(move || {
//!         spinlock_clone.store(0, Ordering::Release);
//!     });
//!
//!     // 等待另一个线程释放锁
//!     while spinlock.load(Ordering::Acquire) != 0 {
//!         hint::spin_loop();
//!     }
//!
//!     if let Err(panic) = thread.join() {
//!         println!("Thread had an error: {panic:?}");
//!     }
//! }
//! ```
//!
//! 维护一个全局存活线程计数:
//!
//! ```
//! use std::sync::atomic::{AtomicUsize, Ordering};
//!
//! static GLOBAL_THREAD_COUNT: AtomicUsize = AtomicUsize::new(0);
//!
//! // 注意:Relaxed 内存序只保证全局线程计数器自身的原子更新,
//! // 不会同步其他内存。
//! let old_thread_count = GLOBAL_THREAD_COUNT.fetch_add(1, Ordering::Relaxed);
//! // 打印时这个数字可能已经过期,因为其他线程可能已经修改了静态值。
//! println!("live threads: {}", old_thread_count + 1);
//! ```

#![stable(feature = "rust1", since = "1.0.0")]
#![cfg_attr(not(target_has_atomic_load_store = "8"), allow(dead_code))]
#![cfg_attr(not(target_has_atomic_load_store = "8"), allow(unused_imports))]
#![rustc_diagnostic_item = "atomic_mod"]
// Clippy 会警告“安全函数调用接收指针的 unsafe 函数”这一模式。
// 这里发生在 AtomicPtr 的 intrinsic 调用上,但这些指针只是被当作普通位值做原子
// load/store,不会在该调用中解引用,因此不触及指针有效性问题。
#![allow(clippy::not_unsafe_ptr_arg_deref)]

use self::Ordering::*;
use crate::cell::UnsafeCell;
use crate::hint::spin_loop;
use crate::intrinsics::AtomicOrdering as AO;
use crate::{fmt, intrinsics};

trait Sealed {}

/// 标记可被原子修改的基本类型。
///
/// 这是 <code>[Atomic]\<T></code> 的实现细节,将来可能随时消失或被替换。
///
/// # 安全性(Safety）
///
/// 实现本 trait 的类型必须是可原子修改的基本类型。
///
/// 关联类型 `Self::AtomicInner` 必须与 `Self` 拥有相同大小和相同位有效性(bit validity)。
/// 它可以要求更高对齐,但必须保证下列重解释在类型层面是健全的:
///
/// - 把 `&mut Self::AtomicInner` 视作 `&mut Self`
/// - 在 `Self` 与 `Self::AtomicInner` 之间按值互相转换
#[unstable(
    feature = "atomic_internals",
    reason = "implementation detail which may disappear or be replaced at any time",
    issue = "none"
)]
#[expect(private_bounds)]
pub unsafe trait AtomicPrimitive: Sized + Copy + Sealed {
    /// 临时实现细节。
    type AtomicInner: Sized;
}

macro impl_atomic_primitive(
    $Atom:ident $(<$T:ident>)? ($Primitive:ty),
    size($size:literal),
    align($align:literal) $(,)?
) {
    impl $(<$T>)? Sealed for $Primitive {}

    #[unstable(
        feature = "atomic_internals",
        reason = "implementation detail which may disappear or be replaced at any time",
        issue = "none"
    )]
    #[cfg(target_has_atomic_load_store = $size)]
    unsafe impl $(<$T>)? AtomicPrimitive for $Primitive {
        type AtomicInner = $Atom $(<$T>)?;
    }
}

impl_atomic_primitive!(AtomicBool(bool), size("8"), align(1));
impl_atomic_primitive!(AtomicI8(i8), size("8"), align(1));
impl_atomic_primitive!(AtomicU8(u8), size("8"), align(1));
impl_atomic_primitive!(AtomicI16(i16), size("16"), align(2));
impl_atomic_primitive!(AtomicU16(u16), size("16"), align(2));
impl_atomic_primitive!(AtomicI32(i32), size("32"), align(4));
impl_atomic_primitive!(AtomicU32(u32), size("32"), align(4));
impl_atomic_primitive!(AtomicI64(i64), size("64"), align(8));
impl_atomic_primitive!(AtomicU64(u64), size("64"), align(8));
impl_atomic_primitive!(AtomicI128(i128), size("128"), align(16));
impl_atomic_primitive!(AtomicU128(u128), size("128"), align(16));

#[cfg(target_pointer_width = "16")]
impl_atomic_primitive!(AtomicIsize(isize), size("ptr"), align(2));
#[cfg(target_pointer_width = "32")]
impl_atomic_primitive!(AtomicIsize(isize), size("ptr"), align(4));
#[cfg(target_pointer_width = "64")]
impl_atomic_primitive!(AtomicIsize(isize), size("ptr"), align(8));

#[cfg(target_pointer_width = "16")]
impl_atomic_primitive!(AtomicUsize(usize), size("ptr"), align(2));
#[cfg(target_pointer_width = "32")]
impl_atomic_primitive!(AtomicUsize(usize), size("ptr"), align(4));
#[cfg(target_pointer_width = "64")]
impl_atomic_primitive!(AtomicUsize(usize), size("ptr"), align(8));

#[cfg(target_pointer_width = "16")]
impl_atomic_primitive!(AtomicPtr<T>(*mut T), size("ptr"), align(2));
#[cfg(target_pointer_width = "32")]
impl_atomic_primitive!(AtomicPtr<T>(*mut T), size("ptr"), align(4));
#[cfg(target_pointer_width = "64")]
impl_atomic_primitive!(AtomicPtr<T>(*mut T), size("ptr"), align(8));

/// 一块可以从多个线程安全修改的内存位置。
///
/// 它与底层类型 `T` 具有相同的大小与位有效性(bit validity)。但本类型的对齐
/// **始终等于其大小**,即使在 `T` 的对齐小于其大小的目标平台上也是如此。
///
/// 关于原子类型与非原子类型的区别、以及本类型的可移植性,详见[模块级文档]。
///
/// **注意:** 本类型仅在支持对 `T` 进行原子加载/存储的平台上可用。
///
/// [模块级文档]: crate::sync::atomic
#[unstable(feature = "generic_atomic", issue = "130539")]
pub type Atomic<T> = <T as AtomicPrimitive>::AtomicInner;

// 有些架构没有字节大小的原子,LLVM 会用 LL/SC(load-linked/store-conditional)循环
// 来模拟。但对 AtomicBool,可以利用它只可能是 0 或 1 这一事实,改用原子 OR/AND——
// LLVM 能用一个更大尺寸的原子 OR/AND 操作来模拟,效率更高。
//
// 此列表只应包含那些“有字大小的 atomic-or/atomic-and 指令、但原生不支持字节大小原子”
// 的架构。
#[cfg(target_has_atomic = "8")]
const EMULATE_ATOMIC_BOOL: bool = cfg!(any(
    target_arch = "riscv32",
    target_arch = "riscv64",
    target_arch = "loongarch32",
    target_arch = "loongarch64"
));

/// 一个可在线程间安全共享的布尔类型。
///
/// 它与 [`bool`] 具有相同的大小、对齐和位有效性。
///
/// **注意**:本类型仅在支持对 `u8` 进行原子加载/存储的平台上可用。
#[cfg(target_has_atomic_load_store = "8")]
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_diagnostic_item = "AtomicBool"]
#[repr(C, align(1))]
pub struct AtomicBool {
    v: UnsafeCell<u8>,
}

#[cfg(target_has_atomic_load_store = "8")]
#[stable(feature = "rust1", since = "1.0.0")]
impl Default for AtomicBool {
    /// 创建一个初始化为 `false` 的 `AtomicBool`。
    #[inline]
    fn default() -> Self {
        Self::new(false)
    }
}

// AtomicBool 的 Send 由编译器隐式实现。
// SAFETY:原子操作保证对内部 `u8` 的并发访问不构成数据竞争,故跨线程共享 `&AtomicBool`
// 是健全的。
#[cfg(target_has_atomic_load_store = "8")]
#[stable(feature = "rust1", since = "1.0.0")]
unsafe impl Sync for AtomicBool {}

/// 一个可在线程间安全共享的裸指针类型。
///
/// 它与 `*mut T` 具有相同的大小和位有效性。
///
/// **注意**:本类型仅在支持对指针进行原子加载/存储的平台上可用。其大小取决于目标平台
/// 的指针大小。
#[cfg(target_has_atomic_load_store = "ptr")]
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_diagnostic_item = "AtomicPtr"]
#[cfg_attr(target_pointer_width = "16", repr(C, align(2)))]
#[cfg_attr(target_pointer_width = "32", repr(C, align(4)))]
#[cfg_attr(target_pointer_width = "64", repr(C, align(8)))]
pub struct AtomicPtr<T> {
    p: UnsafeCell<*mut T>,
}

#[cfg(target_has_atomic_load_store = "ptr")]
#[stable(feature = "rust1", since = "1.0.0")]
impl<T> Default for AtomicPtr<T> {
    /// 创建一个空(null)的 `AtomicPtr<T>`。
    fn default() -> AtomicPtr<T> {
        AtomicPtr::new(crate::ptr::null_mut())
    }
}

// SAFETY:原子操作保证对内部指针的并发读写不构成数据竞争。注意 `AtomicPtr` 只原子地
// 搬运指针的“地址位”,解引用该指针仍需调用方自行保证其有效性与同步。
#[cfg(target_has_atomic_load_store = "ptr")]
#[stable(feature = "rust1", since = "1.0.0")]
unsafe impl<T> Send for AtomicPtr<T> {}
#[cfg(target_has_atomic_load_store = "ptr")]
#[stable(feature = "rust1", since = "1.0.0")]
unsafe impl<T> Sync for AtomicPtr<T> {}

/// 原子内存序(atomic memory orderings)。
///
/// 内存序规定原子操作如何同步内存。最弱的 [`Ordering::Relaxed`] 只同步操作直接触及的
/// 那块内存;而一对 [`Ordering::SeqCst`] 的 store-load,除了同步其他内存,还额外保证
/// 所有此类操作在所有线程间存在一个**全序**(total order)。
///
/// Rust 的内存序[与 C++20 的完全一致](https://en.cppreference.com/w/cpp/atomic/memory_order)。
///
/// 更多信息见 [nomicon]。
///
/// [nomicon]: ../../../nomicon/atomics.html
#[stable(feature = "rust1", since = "1.0.0")]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
#[non_exhaustive]
#[rustc_diagnostic_item = "Ordering"]
pub enum Ordering {
    /// 无任何顺序约束,仅保证操作本身的原子性。
    ///
    /// 对应 C++20 的 [`memory_order_relaxed`]。
    ///
    /// [`memory_order_relaxed`]: https://en.cppreference.com/w/cpp/atomic/memory_order#Relaxed_ordering
    #[stable(feature = "rust1", since = "1.0.0")]
    Relaxed,
    /// 与一次 store 搭配时:本次 store 之前的所有操作,都被排到“任何以 [`Acquire`]
    /// (或更强)序加载本值的操作”之前。特别地,本次 store 之前的所有写,对所有以
    /// [`Acquire`](或更强)序加载本值的线程都变为可见。
    ///
    /// 注意:若把本序用于一个同时含 load 与 store 的操作(如 fetch 系列),其 load
    /// 部分会退化为 [`Relaxed`] 加载!
    ///
    /// 本序只适用于能执行 store 的操作。
    ///
    /// 对应 C++20 的 [`memory_order_release`]。
    ///
    /// [`memory_order_release`]: https://en.cppreference.com/w/cpp/atomic/memory_order#Release-Acquire_ordering
    #[stable(feature = "rust1", since = "1.0.0")]
    Release,
    /// 与一次 load 搭配时:若所加载的值是由某次带 [`Release`](或更强)序的 store 写入的,
    /// 则本次 load 之后的所有操作都被排到那次 store 之后。特别地,本次 load 之后的所有
    /// load 都会看到那次 store 之前写入的数据。
    ///
    /// 注意:若把本序用于一个同时含 load 与 store 的操作,其 store 部分会退化为 [`Relaxed`]
    /// 存储!
    ///
    /// 本序只适用于能执行 load 的操作。
    ///
    /// 对应 C++20 的 [`memory_order_acquire`]。
    ///
    /// [`memory_order_acquire`]: https://en.cppreference.com/w/cpp/atomic/memory_order#Release-Acquire_ordering
    #[stable(feature = "rust1", since = "1.0.0")]
    Acquire,
    /// 同时具备 [`Acquire`] 与 [`Release`] 的效果:对 load 部分使用 [`Acquire`] 顺序,
    /// 对 store 部分使用 [`Release`] 顺序。
    ///
    /// 注意:在 `compare_and_swap` 这类操作中,比较失败时可能最终没有执行任何 store,
    /// 因而只剩 [`Acquire`] 顺序。但 `AcqRel` 永远不会退化到 [`Relaxed`] 访问。
    ///
    /// 本序只适用于同时含 load 与 store 的操作。
    ///
    /// 对应 C++20 的 [`memory_order_acq_rel`]。
    ///
    /// [`memory_order_acq_rel`]: https://en.cppreference.com/w/cpp/atomic/memory_order#Release-Acquire_ordering
    #[stable(feature = "rust1", since = "1.0.0")]
    AcqRel,
    /// 行为如同 [`Acquire`]/[`Release`]/[`AcqRel`](分别用于 load、store、含 store 的 load),
    /// 但额外保证:所有线程都以**相同的顺序**看到所有顺序一致(sequentially consistent)操作。
    /// 这是最强的内存序,也是唯一能在多个原子变量间建立全局全序的选项。
    ///
    /// 对应 C++20 的 [`memory_order_seq_cst`]。
    ///
    /// [`memory_order_seq_cst`]: https://en.cppreference.com/w/cpp/atomic/memory_order#Sequentially-consistent_ordering
    #[stable(feature = "rust1", since = "1.0.0")]
    SeqCst,
}

/// 一个初始化为 `false` 的 [`AtomicBool`]。
#[cfg(target_has_atomic_load_store = "8")]
#[stable(feature = "rust1", since = "1.0.0")]
#[deprecated(
    since = "1.34.0",
    note = "the `new` function is now preferred",
    suggestion = "AtomicBool::new(false)"
)]
pub const ATOMIC_BOOL_INIT: AtomicBool = AtomicBool::new(false);

#[cfg(target_has_atomic_load_store = "8")]
impl AtomicBool {
    /// 创建一个新的 `AtomicBool`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::sync::atomic::AtomicBool;
    ///
    /// let atomic_true = AtomicBool::new(true);
    /// let atomic_false = AtomicBool::new(false);
    /// ```
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_atomic_new", since = "1.24.0")]
    #[must_use]
    pub const fn new(v: bool) -> AtomicBool {
        AtomicBool { v: UnsafeCell::new(v as u8) }
    }

    /// 从一个指针创建 `AtomicBool` 视图。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::sync::atomic::{self, AtomicBool};
    ///
    /// // 取得一个已分配值的指针
    /// let ptr: *mut bool = Box::into_raw(Box::new(false));
    ///
    /// assert!(ptr.cast::<AtomicBool>().is_aligned());
    ///
    /// {
    ///     // 为这块已分配的值创建一个原子视图
    ///     let atomic = unsafe { AtomicBool::from_ptr(ptr) };
    ///
    ///     // 用 `atomic` 做原子操作,也可与其他线程共享
    ///     atomic.store(true, atomic::Ordering::Relaxed);
    /// }
    ///
    /// // 此时再非原子地访问 `ptr` 背后的值是 OK 的,
    /// // 因为对该原子的引用已在上面的代码块中结束生命周期
    /// assert_eq!(unsafe { *ptr }, true);
    ///
    /// // 释放该值
    /// unsafe { drop(Box::from_raw(ptr)) }
    /// ```
    ///
    /// # 安全性(Safety）
    ///
    /// 调用方必须保证以下前置条件,否则即为未定义行为:
    /// * `ptr` 必须对齐到 `align_of::<AtomicBool>()`(注:这总是成立,因为该对齐恒为 1)。
    /// * 在整个生命周期 `'a` 内,`ptr` 对读和写都必须[有效][valid]。
    /// * 必须遵守[原子访问的内存模型]。特别地,未经同步时,不得混用相互冲突的原子与非原子
    ///   访问,也不得混用不同尺寸的原子访问。
    ///
    /// [valid]: crate::ptr#safety
    /// [原子访问的内存模型]: self#memory-model-for-atomic-accesses
    #[inline]
    #[stable(feature = "atomic_from_ptr", since = "1.75.0")]
    #[rustc_const_stable(feature = "const_atomic_from_ptr", since = "1.84.0")]
    pub const unsafe fn from_ptr<'a>(ptr: *mut bool) -> &'a AtomicBool {
        // SAFETY: 由调用方保证(见上面的安全性契约)。
        unsafe { &*ptr.cast() }
    }

    /// 返回对底层 [`bool`] 的可变引用。
    ///
    /// 之所以安全,是因为这个可变引用本身就保证了没有其他线程正在并发访问该原子数据
    /// (`&mut self` 即独占访问,无需任何原子操作)。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::sync::atomic::{AtomicBool, Ordering};
    ///
    /// let mut some_bool = AtomicBool::new(true);
    /// assert_eq!(*some_bool.get_mut(), true);
    /// *some_bool.get_mut() = false;
    /// assert_eq!(some_bool.load(Ordering::SeqCst), false);
    /// ```
    #[inline]
    #[stable(feature = "atomic_access", since = "1.15.0")]
    pub fn get_mut(&mut self) -> &mut bool {
        // SAFETY: 可变引用保证了独占所有权,故可安全转成 `&mut bool`。
        unsafe { &mut *(self.v.get() as *mut bool) }
    }

    /// 为一个 `&mut bool` 取得原子访问视图。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(atomic_from_mut)]
    /// use std::sync::atomic::{AtomicBool, Ordering};
    ///
    /// let mut some_bool = true;
    /// let a = AtomicBool::from_mut(&mut some_bool);
    /// a.store(false, Ordering::Relaxed);
    /// assert_eq!(some_bool, false);
    /// ```
    #[inline]
    #[cfg(target_has_atomic_equal_alignment = "8")]
    #[unstable(feature = "atomic_from_mut", issue = "76314")]
    pub fn from_mut(v: &mut bool) -> &mut Self {
        // SAFETY: 可变引用保证独占所有权,且 `bool` 与 `Self` 的对齐都是 1,布局相容。
        unsafe { &mut *(v as *mut bool as *mut Self) }
    }

    /// 为一个 `&mut [AtomicBool]` 切片取得**非原子**访问视图。
    ///
    /// 之所以安全,是因为这个可变引用保证了没有其他线程正在并发访问这些原子数据。
    ///
    /// # 示例
    ///
    /// ```ignore-wasm
    /// #![feature(atomic_from_mut)]
    /// use std::sync::atomic::{AtomicBool, Ordering};
    ///
    /// let mut some_bools = [const { AtomicBool::new(false) }; 10];
    ///
    /// let view: &mut [bool] = AtomicBool::get_mut_slice(&mut some_bools);
    /// assert_eq!(view, [false; 10]);
    /// view[..5].copy_from_slice(&[true; 5]);
    ///
    /// std::thread::scope(|s| {
    ///     for t in &some_bools[..5] {
    ///         s.spawn(move || assert_eq!(t.load(Ordering::Relaxed), true));
    ///     }
    ///
    ///     for f in &some_bools[5..] {
    ///         s.spawn(move || assert_eq!(f.load(Ordering::Relaxed), false));
    ///     }
    /// });
    /// ```
    #[inline]
    #[unstable(feature = "atomic_from_mut", issue = "76314")]
    pub fn get_mut_slice(this: &mut [Self]) -> &mut [bool] {
        // SAFETY: 可变引用保证独占所有权,故无需原子操作即可重解释为 `&mut [bool]`。
        unsafe { &mut *(this as *mut [Self] as *mut [bool]) }
    }

    /// 为一个 `&mut [bool]` 切片取得原子访问视图。
    ///
    /// # 示例
    ///
    /// ```rust,ignore-wasm
    /// #![feature(atomic_from_mut)]
    /// use std::sync::atomic::{AtomicBool, Ordering};
    ///
    /// let mut some_bools = [false; 10];
    /// let a = &*AtomicBool::from_mut_slice(&mut some_bools);
    /// std::thread::scope(|s| {
    ///     for i in 0..a.len() {
    ///         s.spawn(move || a[i].store(true, Ordering::Relaxed));
    ///     }
    /// });
    /// assert_eq!(some_bools, [true; 10]);
    /// ```
    #[inline]
    #[cfg(target_has_atomic_equal_alignment = "8")]
    #[unstable(feature = "atomic_from_mut", issue = "76314")]
    pub fn from_mut_slice(v: &mut [bool]) -> &mut [Self] {
        // SAFETY: 可变引用保证独占所有权,且 `bool` 与 `Self` 的对齐都是 1,布局相容。
        unsafe { &mut *(v as *mut [bool] as *mut [Self]) }
    }

    /// 消费该原子,返回其中持有的值。
    ///
    /// 之所以安全,是因为按值传入 `self` 保证了没有其他线程正在并发访问该原子数据。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::sync::atomic::AtomicBool;
    ///
    /// let some_bool = AtomicBool::new(true);
    /// assert_eq!(some_bool.into_inner(), true);
    /// ```
    #[inline]
    #[stable(feature = "atomic_access", since = "1.15.0")]
    #[rustc_const_stable(feature = "const_atomic_into_inner", since = "1.79.0")]
    pub const fn into_inner(self) -> bool {
        self.v.into_inner() != 0
    }

    /// 从该 bool 中加载一个值。
    ///
    /// `load` 接受一个 [`Ordering`] 参数描述本操作的内存序。可取值为 [`SeqCst`]、
    /// [`Acquire`] 和 [`Relaxed`]。
    ///
    /// # Panics
    ///
    /// 当 `order` 为 [`Release`] 或 [`AcqRel`] 时 panic(load 不能带 release 语义)。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::sync::atomic::{AtomicBool, Ordering};
    ///
    /// let some_bool = AtomicBool::new(true);
    ///
    /// assert_eq!(some_bool.load(Ordering::Relaxed), true);
    /// ```
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[cfg_attr(miri, track_caller)] // 即使不 panic,这也有助于 Miri 的回溯信息
    pub fn load(&self, order: Ordering) -> bool {
        // SAFETY: 数据竞争由原子 intrinsic 防止;传入的裸指针来自一个引用,故必然有效。
        unsafe { atomic_load(self.v.get(), order) != 0 }
    }

    /// 向该 bool 存入一个值。
    ///
    /// `store` 接受一个 [`Ordering`] 参数描述本操作的内存序。可取值为 [`SeqCst`]、
    /// [`Release`] 和 [`Relaxed`]。
    ///
    /// # Panics
    ///
    /// 当 `order` 为 [`Acquire`] 或 [`AcqRel`] 时 panic(store 不能带 acquire 语义)。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::sync::atomic::{AtomicBool, Ordering};
    ///
    /// let some_bool = AtomicBool::new(true);
    ///
    /// some_bool.store(false, Ordering::Relaxed);
    /// assert_eq!(some_bool.load(Ordering::Relaxed), false);
    /// ```
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[cfg_attr(miri, track_caller)] // 即使不 panic,这也有助于 Miri 的回溯信息
    #[rustc_should_not_be_called_on_const_items]
    pub fn store(&self, val: bool, order: Ordering) {
        // SAFETY: 数据竞争由原子 intrinsic 防止;传入的裸指针来自一个引用,故必然有效。
        unsafe {
            atomic_store(self.v.get(), val as u8, order);
        }
    }

    /// 向该 bool 存入一个值,并返回此前的旧值。
    ///
    /// `swap` 接受一个 [`Ordering`] 参数描述本操作的内存序。所有内存序模式都可用。注意:
    /// 用 [`Acquire`] 会使本操作的 store 部分退化为 [`Relaxed`],用 [`Release`] 会使其
    /// load 部分退化为 [`Relaxed`]。
    ///
    /// **注意:** 本方法仅在支持对 `u8` 进行原子操作的平台上可用。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::sync::atomic::{AtomicBool, Ordering};
    ///
    /// let some_bool = AtomicBool::new(true);
    ///
    /// assert_eq!(some_bool.swap(false, Ordering::Relaxed), true);
    /// assert_eq!(some_bool.load(Ordering::Relaxed), false);
    /// ```
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[cfg(target_has_atomic = "8")]
    #[cfg_attr(miri, track_caller)] // 即使不 panic,这也有助于 Miri 的回溯信息
    #[rustc_should_not_be_called_on_const_items]
    pub fn swap(&self, val: bool, order: Ordering) -> bool {
        // 在缺少字节原子的架构上(见 EMULATE_ATOMIC_BOOL),用 OR/AND 模拟 swap:
        // 写 true 等价于 OR true,写 false 等价于 AND false。
        if EMULATE_ATOMIC_BOOL {
            if val { self.fetch_or(true, order) } else { self.fetch_and(false, order) }
        } else {
            // SAFETY: 数据竞争由原子 intrinsic 防止。
            unsafe { atomic_swap(self.v.get(), val as u8, order) != 0 }
        }
    }

    /// 仅当当前值等于 `current` 时,才把新值存入该 [`bool`]。
    ///
    /// 返回值**始终**是此前的旧值。若它等于 `current`,说明更新已发生。
    ///
    /// `compare_and_swap` 也接受一个 [`Ordering`] 参数描述本操作的内存序。注意:即便使用
    /// [`AcqRel`],操作也可能失败,此时只执行一次 `Acquire` 加载、不具备 `Release` 语义。
    /// 用 [`Acquire`] 会让(若发生的)store 部分退化为 [`Relaxed`],用 [`Release`] 会让
    /// load 部分退化为 [`Relaxed`]。
    ///
    /// **注意:** 本方法仅在支持对 `u8` 进行原子操作的平台上可用。
    ///
    /// # 迁移到 `compare_exchange` 与 `compare_exchange_weak`
    ///
    /// `compare_and_swap` 等价于按下表映射内存序的 `compare_exchange`:
    ///
    /// 原内存序 | 成功时 | 失败时
    /// -------- | ------- | -------
    /// Relaxed  | Relaxed | Relaxed
    /// Acquire  | Acquire | Acquire
    /// Release  | Release | Relaxed
    /// AcqRel   | AcqRel  | Acquire
    /// SeqCst   | SeqCst  | SeqCst
    ///
    /// 两者返回类型也不同。可用 `compare_exchange(...).unwrap_or_else(|x| x)` 复现
    /// `compare_and_swap` 的行为,但多数情况下更地道的做法是检查返回值是 `Ok` 还是 `Err`,
    /// 而非根据读到的值去推断成功与否。
    ///
    /// 迁移时还应考虑改用 `compare_exchange_weak`:它**允许在比较成功时仍偶发失败**
    /// (spurious failure),从而让编译器在“CAS 放在循环里”的场景下生成更优的汇编。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::sync::atomic::{AtomicBool, Ordering};
    ///
    /// let some_bool = AtomicBool::new(true);
    ///
    /// assert_eq!(some_bool.compare_and_swap(true, false, Ordering::Relaxed), true);
    /// assert_eq!(some_bool.load(Ordering::Relaxed), false);
    ///
    /// assert_eq!(some_bool.compare_and_swap(true, true, Ordering::Relaxed), false);
    /// assert_eq!(some_bool.load(Ordering::Relaxed), false);
    /// ```
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[deprecated(
        since = "1.50.0",
        note = "Use `compare_exchange` or `compare_exchange_weak` instead"
    )]
    #[cfg(target_has_atomic = "8")]
    #[cfg_attr(miri, track_caller)] // 即使不 panic,这也有助于 Miri 的回溯信息
    #[rustc_should_not_be_called_on_const_items]
    pub fn compare_and_swap(&self, current: bool, new: bool, order: Ordering) -> bool {
        match self.compare_exchange(current, new, order, strongest_failure_ordering(order)) {
            Ok(x) => x,
            Err(x) => x,
        }
    }

    /// 当且仅当当前值与 `current` 相同时,把 `new` 写入这个 [`bool`]。
    ///
    /// 返回值是一个 `Result`,指示新值是否被写入,并携带操作前的旧值。
    /// 成功时该值保证等于 `current`。
    ///
    /// `compare_exchange` 接受两个 [`Ordering`] 参数来描述本操作的内存顺序。
    /// `success` 描述比较成功时所执行的“读-改-写”操作所需的顺序;
    /// `failure` 描述比较失败时所执行的 load 操作所需的顺序。
    /// 把 `success` 设为 [`Acquire`] 会让本操作的 store 部分退化为 [`Relaxed`],
    /// 设为 [`Release`] 会让成功路径上的 load 部分退化为 [`Relaxed`]。
    /// `failure` 顺序只能是 [`SeqCst`]、[`Acquire`] 或 [`Relaxed`];
    /// 传入 [`Release`] 或 [`AcqRel`] 会触发 panic。
    ///
    /// **注意:** 本方法只在支持 `u8` 原子操作的平台上可用。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::sync::atomic::{AtomicBool, Ordering};
    ///
    /// let some_bool = AtomicBool::new(true);
    ///
    /// assert_eq!(some_bool.compare_exchange(true,
    ///                                       false,
    ///                                       Ordering::Acquire,
    ///                                       Ordering::Relaxed),
    ///            Ok(true));
    /// assert_eq!(some_bool.load(Ordering::Relaxed), false);
    ///
    /// assert_eq!(some_bool.compare_exchange(true, true,
    ///                                       Ordering::SeqCst,
    ///                                       Ordering::Acquire),
    ///            Err(false));
    /// assert_eq!(some_bool.load(Ordering::Relaxed), false);
    /// ```
    ///
    /// # 注意事项
    ///
    /// `compare_exchange` 是一个 [compare-and-swap operation],因此带有 CAS 操作惯有的缺陷。
    /// 特别地,“先 load 出一个值,再用这个旧值做一次成功的 `compare_exchange`” *并不能保证*
    /// 在这两步之间其他线程没有改动过该值。这一点在用 `compare_exchange` 里的*相等性*检查
    /// 来判断值的*同一性*时尤其重要 —— 相等并不必然意味着同一。在这种场景下,
    /// `compare_exchange` 可能导致 [ABA problem](即值被改成别的再改回原值,CAS 仍判定相等而通过)。
    ///
    /// [ABA Problem]: https://en.wikipedia.org/wiki/ABA_problem
    /// [compare-and-swap operation]: https://en.wikipedia.org/wiki/Compare-and-swap
    #[inline]
    #[stable(feature = "extended_compare_and_swap", since = "1.10.0")]
    #[doc(alias = "compare_and_swap")]
    #[cfg(target_has_atomic = "8")]
    #[cfg_attr(miri, track_caller)] // 即使不 panic,这也有助于 Miri 的回溯信息
    #[rustc_should_not_be_called_on_const_items]
    pub fn compare_exchange(
        &self,
        current: bool,
        new: bool,
        success: Ordering,
        failure: Ordering,
    ) -> Result<bool, bool> {
        if EMULATE_ATOMIC_BOOL {
            // 从 success 与 failure 中取较强的那个顺序。
            let order = match (success, failure) {
                (SeqCst, _) => SeqCst,
                (_, SeqCst) => SeqCst,
                (AcqRel, _) => AcqRel,
                (_, AcqRel) => {
                    panic!("there is no such thing as an acquire-release failure ordering")
                }
                (Release, Acquire) => AcqRel,
                (Acquire, _) => Acquire,
                (_, Acquire) => Acquire,
                (Release, Relaxed) => Release,
                (_, Release) => panic!("there is no such thing as a release failure ordering"),
                (Relaxed, Relaxed) => Relaxed,
            };
            let old = if current == new {
                // 这其实是个空操作,但为了内存顺序的正确性,我们仍然要执行这个操作。
                self.fetch_or(false, order)
            } else {
                // 把值设为新值,并返回旧值。
                self.swap(new, order)
            };
            if old == current { Ok(old) } else { Err(old) }
        } else {
            // SAFETY: 数据竞争由原子 intrinsic 防止。
            match unsafe {
                atomic_compare_exchange(self.v.get(), current as u8, new as u8, success, failure)
            } {
                Ok(x) => Ok(x != 0),
                Err(x) => Err(x != 0),
            }
        }
    }

    /// 当且仅当当前值与 `current` 相同时,把 `new` 写入这个 [`bool`]。
    ///
    /// 与 [`AtomicBool::compare_exchange`] 不同,本函数 **允许在比较成功时仍偶发失败
    /// (spurious failure)**,这能让某些平台生成更高效的代码。返回值是一个 `Result`,
    /// 指示新值是否被写入,并携带操作前的旧值。
    ///
    /// 之所以允许偶发失败:在 LL/SC(load-linked / store-conditional)架构上,
    /// 强 CAS 在底层往往需要一个重试循环;而 `compare_exchange_weak` 把单次 LL/SC 尝试
    /// 暴露出来,调用方通常本来就在循环里使用它,因此无需为强 CAS 内部那层重试付出代价。
    ///
    /// `compare_exchange_weak` 接受两个 [`Ordering`] 参数来描述本操作的内存顺序。
    /// `success` 描述比较成功时所执行的“读-改-写”操作所需的顺序;
    /// `failure` 描述比较失败时所执行的 load 操作所需的顺序。
    /// 把 `success` 设为 [`Acquire`] 会让本操作的 store 部分退化为 [`Relaxed`],
    /// 设为 [`Release`] 会让成功路径上的 load 部分退化为 [`Relaxed`]。
    /// `failure` 顺序只能是 [`SeqCst`]、[`Acquire`] 或 [`Relaxed`];
    /// 传入 [`Release`] 或 [`AcqRel`] 会触发 panic。
    ///
    /// **注意:** 本方法只在支持 `u8` 原子操作的平台上可用。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::sync::atomic::{AtomicBool, Ordering};
    ///
    /// let val = AtomicBool::new(false);
    ///
    /// let new = true;
    /// let mut old = val.load(Ordering::Relaxed);
    /// loop {
    ///     match val.compare_exchange_weak(old, new, Ordering::SeqCst, Ordering::Relaxed) {
    ///         Ok(_) => break,
    ///         Err(x) => old = x,
    ///     }
    /// }
    /// ```
    ///
    /// # 注意事项
    ///
    /// `compare_exchange` 是一个 [compare-and-swap operation],因此带有 CAS 操作惯有的缺陷。
    /// 特别地,“先 load 出一个值,再用这个旧值做一次成功的 `compare_exchange`” *并不能保证*
    /// 在这两步之间其他线程没有改动过该值。这一点在用 `compare_exchange` 里的*相等性*检查
    /// 来判断值的*同一性*时尤其重要 —— 相等并不必然意味着同一。在这种场景下,
    /// `compare_exchange` 可能导致 [ABA problem](即值被改成别的再改回原值,CAS 仍判定相等而通过)。
    ///
    /// [ABA Problem]: https://en.wikipedia.org/wiki/ABA_problem
    /// [compare-and-swap operation]: https://en.wikipedia.org/wiki/Compare-and-swap
    #[inline]
    #[stable(feature = "extended_compare_and_swap", since = "1.10.0")]
    #[doc(alias = "compare_and_swap")]
    #[cfg(target_has_atomic = "8")]
    #[cfg_attr(miri, track_caller)] // 即使不 panic,这也有助于 Miri 的回溯信息
    #[rustc_should_not_be_called_on_const_items]
    pub fn compare_exchange_weak(
        &self,
        current: bool,
        new: bool,
        success: Ordering,
        failure: Ordering,
    ) -> Result<bool, bool> {
        if EMULATE_ATOMIC_BOOL {
            return self.compare_exchange(current, new, success, failure);
        }

        // SAFETY: 数据竞争由原子 intrinsic 防止。
        match unsafe {
            atomic_compare_exchange_weak(self.v.get(), current as u8, new as u8, success, failure)
        } {
            Ok(x) => Ok(x != 0),
            Err(x) => Err(x != 0),
        }
    }

    /// 对一个布尔值做逻辑“与”(and)。
    ///
    /// 对当前值与参数 `val` 执行逻辑“与”操作,并把结果设为新值。
    ///
    /// 返回操作前的旧值。
    ///
    /// `fetch_and` 接受一个 [`Ordering`] 参数来描述本操作的内存顺序。
    /// 所有顺序模式都允许。注意:用 [`Acquire`] 会让本操作的 store 部分退化为 [`Relaxed`],
    /// 用 [`Release`] 会让 load 部分退化为 [`Relaxed`];只有 [`AcqRel`] 或 [`SeqCst`] 才同时具备两端语义。
    ///
    /// **注意:** 本方法只在支持 `u8` 原子操作的平台上可用。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::sync::atomic::{AtomicBool, Ordering};
    ///
    /// let foo = AtomicBool::new(true);
    /// assert_eq!(foo.fetch_and(false, Ordering::SeqCst), true);
    /// assert_eq!(foo.load(Ordering::SeqCst), false);
    ///
    /// let foo = AtomicBool::new(true);
    /// assert_eq!(foo.fetch_and(true, Ordering::SeqCst), true);
    /// assert_eq!(foo.load(Ordering::SeqCst), true);
    ///
    /// let foo = AtomicBool::new(false);
    /// assert_eq!(foo.fetch_and(false, Ordering::SeqCst), false);
    /// assert_eq!(foo.load(Ordering::SeqCst), false);
    /// ```
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[cfg(target_has_atomic = "8")]
    #[cfg_attr(miri, track_caller)] // 即使不 panic,这也有助于 Miri 的回溯信息
    #[rustc_should_not_be_called_on_const_items]
    pub fn fetch_and(&self, val: bool, order: Ordering) -> bool {
        // SAFETY: 数据竞争由原子 intrinsic 防止。
        unsafe { atomic_and(self.v.get(), val as u8, order) != 0 }
    }

    /// 对一个布尔值做逻辑“与非”(nand)。
    ///
    /// 对当前值与参数 `val` 执行逻辑“与非”操作,并把结果设为新值。
    ///
    /// 返回操作前的旧值。
    ///
    /// `fetch_nand` 接受一个 [`Ordering`] 参数来描述本操作的内存顺序。
    /// 所有顺序模式都允许。注意:用 [`Acquire`] 会让本操作的 store 部分退化为 [`Relaxed`],
    /// 用 [`Release`] 会让 load 部分退化为 [`Relaxed`];只有 [`AcqRel`] 或 [`SeqCst`] 才同时具备两端语义。
    ///
    /// **注意:** 本方法只在支持 `u8` 原子操作的平台上可用。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::sync::atomic::{AtomicBool, Ordering};
    ///
    /// let foo = AtomicBool::new(true);
    /// assert_eq!(foo.fetch_nand(false, Ordering::SeqCst), true);
    /// assert_eq!(foo.load(Ordering::SeqCst), true);
    ///
    /// let foo = AtomicBool::new(true);
    /// assert_eq!(foo.fetch_nand(true, Ordering::SeqCst), true);
    /// assert_eq!(foo.load(Ordering::SeqCst) as usize, 0);
    /// assert_eq!(foo.load(Ordering::SeqCst), false);
    ///
    /// let foo = AtomicBool::new(false);
    /// assert_eq!(foo.fetch_nand(false, Ordering::SeqCst), false);
    /// assert_eq!(foo.load(Ordering::SeqCst), true);
    /// ```
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[cfg(target_has_atomic = "8")]
    #[cfg_attr(miri, track_caller)] // 即使不 panic,这也有助于 Miri 的回溯信息
    #[rustc_should_not_be_called_on_const_items]
    pub fn fetch_nand(&self, val: bool, order: Ordering) -> bool {
        // 这里不能用 atomic_nand,因为它可能产生一个值非法的 bool。
        // 原因是底层是用一个 8 位整数来做原子操作的,与非运算会把高 7 位也置 1。
        // 所以我们改用 fetch_xor 或 swap 来实现。
        if val {
            // `!(x & true) == !x`,也就是对原 bool 取反。
            // 必须把这个 bool 取反。
            self.fetch_xor(true, order)
        } else {
            // `!(x & false) == true`,结果恒为 true。
            // 必须把这个 bool 设为 true。
            self.swap(true, order)
        }
    }

    /// 对一个布尔值做逻辑“或”(or)。
    ///
    /// 对当前值与参数 `val` 执行逻辑“或”操作,并把结果设为新值。
    ///
    /// 返回操作前的旧值。
    ///
    /// `fetch_or` 接受一个 [`Ordering`] 参数来描述本操作的内存顺序。
    /// 所有顺序模式都允许。注意:用 [`Acquire`] 会让本操作的 store 部分退化为 [`Relaxed`],
    /// 用 [`Release`] 会让 load 部分退化为 [`Relaxed`];只有 [`AcqRel`] 或 [`SeqCst`] 才同时具备两端语义。
    ///
    /// **注意:** 本方法只在支持 `u8` 原子操作的平台上可用。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::sync::atomic::{AtomicBool, Ordering};
    ///
    /// let foo = AtomicBool::new(true);
    /// assert_eq!(foo.fetch_or(false, Ordering::SeqCst), true);
    /// assert_eq!(foo.load(Ordering::SeqCst), true);
    ///
    /// let foo = AtomicBool::new(false);
    /// assert_eq!(foo.fetch_or(true, Ordering::SeqCst), false);
    /// assert_eq!(foo.load(Ordering::SeqCst), true);
    ///
    /// let foo = AtomicBool::new(false);
    /// assert_eq!(foo.fetch_or(false, Ordering::SeqCst), false);
    /// assert_eq!(foo.load(Ordering::SeqCst), false);
    /// ```
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[cfg(target_has_atomic = "8")]
    #[cfg_attr(miri, track_caller)] // 即使不 panic,这也有助于 Miri 的回溯信息
    #[rustc_should_not_be_called_on_const_items]
    pub fn fetch_or(&self, val: bool, order: Ordering) -> bool {
        // SAFETY: 数据竞争由原子 intrinsic 防止。
        unsafe { atomic_or(self.v.get(), val as u8, order) != 0 }
    }

    /// 对一个布尔值做逻辑“异或”(xor)。
    ///
    /// 对当前值与参数 `val` 执行逻辑“异或”操作,并把结果设为新值。
    ///
    /// 返回操作前的旧值。
    ///
    /// `fetch_xor` 接受一个 [`Ordering`] 参数来描述本操作的内存顺序。
    /// 所有顺序模式都允许。注意:用 [`Acquire`] 会让本操作的 store 部分退化为 [`Relaxed`],
    /// 用 [`Release`] 会让 load 部分退化为 [`Relaxed`];只有 [`AcqRel`] 或 [`SeqCst`] 才同时具备两端语义。
    ///
    /// **注意:** 本方法只在支持 `u8` 原子操作的平台上可用。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::sync::atomic::{AtomicBool, Ordering};
    ///
    /// let foo = AtomicBool::new(true);
    /// assert_eq!(foo.fetch_xor(false, Ordering::SeqCst), true);
    /// assert_eq!(foo.load(Ordering::SeqCst), true);
    ///
    /// let foo = AtomicBool::new(true);
    /// assert_eq!(foo.fetch_xor(true, Ordering::SeqCst), true);
    /// assert_eq!(foo.load(Ordering::SeqCst), false);
    ///
    /// let foo = AtomicBool::new(false);
    /// assert_eq!(foo.fetch_xor(false, Ordering::SeqCst), false);
    /// assert_eq!(foo.load(Ordering::SeqCst), false);
    /// ```
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[cfg(target_has_atomic = "8")]
    #[cfg_attr(miri, track_caller)] // 即使不 panic,这也有助于 Miri 的回溯信息
    #[rustc_should_not_be_called_on_const_items]
    pub fn fetch_xor(&self, val: bool, order: Ordering) -> bool {
        // SAFETY: 数据竞争由原子 intrinsic 防止。
        unsafe { atomic_xor(self.v.get(), val as u8, order) != 0 }
    }

    /// 对一个布尔值做逻辑“非”(not)。
    ///
    /// 对当前值执行逻辑“非”操作,并把结果设为新值。
    ///
    /// 返回操作前的旧值。
    ///
    /// `fetch_not` 接受一个 [`Ordering`] 参数来描述本操作的内存顺序。
    /// 所有顺序模式都允许。注意:用 [`Acquire`] 会让本操作的 store 部分退化为 [`Relaxed`],
    /// 用 [`Release`] 会让 load 部分退化为 [`Relaxed`];只有 [`AcqRel`] 或 [`SeqCst`] 才同时具备两端语义。
    ///
    /// **注意:** 本方法只在支持 `u8` 原子操作的平台上可用。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::sync::atomic::{AtomicBool, Ordering};
    ///
    /// let foo = AtomicBool::new(true);
    /// assert_eq!(foo.fetch_not(Ordering::SeqCst), true);
    /// assert_eq!(foo.load(Ordering::SeqCst), false);
    ///
    /// let foo = AtomicBool::new(false);
    /// assert_eq!(foo.fetch_not(Ordering::SeqCst), false);
    /// assert_eq!(foo.load(Ordering::SeqCst), true);
    /// ```
    #[inline]
    #[stable(feature = "atomic_bool_fetch_not", since = "1.81.0")]
    #[cfg(target_has_atomic = "8")]
    #[cfg_attr(miri, track_caller)] // 即使不 panic,这也有助于 Miri 的回溯信息
    #[rustc_should_not_be_called_on_const_items]
    pub fn fetch_not(&self, order: Ordering) -> bool {
        self.fetch_xor(true, order)
    }

    /// 返回指向底层 [`bool`] 的裸可变指针。
    ///
    /// 对返回的这个布尔值做非原子的读写可能造成数据竞争。
    /// 本方法主要用于 FFI 场景 —— 那里函数签名往往用 `*mut bool` 而非 `&AtomicBool`。
    ///
    /// 从对该原子的共享引用返回 `*mut` 指针是安全的,因为原子类型基于内部可变性工作:
    /// 对原子的所有修改都是通过共享引用进行的,而只要它们使用原子操作就能安全地这样做。
    /// 对返回的裸指针的任何使用都需要 `unsafe` 块,并且仍然必须满足 [memory model] 的要求。
    ///
    /// # 示例
    ///
    /// ```ignore (extern-declaration)
    /// # fn main() {
    /// use std::sync::atomic::AtomicBool;
    ///
    /// extern "C" {
    ///     fn my_atomic_op(arg: *mut bool);
    /// }
    ///
    /// let mut atomic = AtomicBool::new(true);
    /// unsafe {
    ///     my_atomic_op(atomic.as_ptr());
    /// }
    /// # }
    /// ```
    ///
    /// [memory model]: self#memory-model-for-atomic-accesses
    #[inline]
    #[stable(feature = "atomic_as_ptr", since = "1.70.0")]
    #[rustc_const_stable(feature = "atomic_as_ptr", since = "1.70.0")]
    #[rustc_never_returns_null_ptr]
    #[rustc_should_not_be_called_on_const_items]
    pub const fn as_ptr(&self) -> *mut bool {
        self.v.get().cast()
    }

    /// 取出当前值,并对它应用一个返回 `Option<新值>` 的函数。
    /// 若该函数返回 `Some(_)`,则返回 `Ok(previous_value)`(操作前的旧值);否则返回 `Err(previous_value)`。
    ///
    /// 注意:只要该函数持续返回 `Some(_)`,当期间该值被其他线程改动时,本方法可能会
    /// 多次调用该函数;但该函数最终只会对一个已存储的值生效一次。
    ///
    /// `fetch_update` 接受两个 [`Ordering`] 参数来描述本操作的内存顺序。
    /// 第一个描述操作最终成功时所需的顺序,第二个描述 load 所需的顺序。
    /// 它们分别对应 [`AtomicBool::compare_exchange`] 的 success 与 failure 顺序。
    ///
    /// 把成功顺序设为 [`Acquire`] 会让本操作的 store 部分退化为 [`Relaxed`],
    /// 设为 [`Release`] 会让最终成功路径上的 load 退化为 [`Relaxed`]。
    /// (失败的)load 顺序只能是 [`SeqCst`]、[`Acquire`] 或 [`Relaxed`]。
    ///
    /// **注意:** 本方法只在支持 `u8` 原子操作的平台上可用。
    ///
    /// # 注意事项
    ///
    /// 本方法并不神奇:它不是硬件直接提供的,也不像临界区或互斥锁那样工作。
    ///
    /// 它是在原子 [compare-and-swap operation] 之上实现的,因此带有 CAS 操作惯有的缺陷。
    /// 特别地,要当心 [ABA problem]。
    ///
    /// [ABA Problem]: https://en.wikipedia.org/wiki/ABA_problem
    /// [compare-and-swap operation]: https://en.wikipedia.org/wiki/Compare-and-swap
    ///
    /// # 示例
    ///
    /// ```rust
    /// use std::sync::atomic::{AtomicBool, Ordering};
    ///
    /// let x = AtomicBool::new(false);
    /// assert_eq!(x.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |_| None), Err(false));
    /// assert_eq!(x.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |x| Some(!x)), Ok(false));
    /// assert_eq!(x.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |x| Some(!x)), Ok(true));
    /// assert_eq!(x.load(Ordering::SeqCst), false);
    /// ```
    #[inline]
    #[stable(feature = "atomic_fetch_update", since = "1.53.0")]
    #[cfg(target_has_atomic = "8")]
    #[cfg_attr(miri, track_caller)] // 即使不 panic,这也有助于 Miri 的回溯信息
    #[rustc_should_not_be_called_on_const_items]
    pub fn fetch_update<F>(
        &self,
        set_order: Ordering,
        fetch_order: Ordering,
        mut f: F,
    ) -> Result<bool, bool>
    where
        F: FnMut(bool) -> Option<bool>,
    {
        let mut prev = self.load(fetch_order);
        while let Some(next) = f(prev) {
            match self.compare_exchange_weak(prev, next, set_order, fetch_order) {
                x @ Ok(_) => return x,
                Err(next_prev) => prev = next_prev,
            }
        }
        Err(prev)
    }

    /// 取出当前值,并对它应用一个返回 `Option<新值>` 的函数。
    /// 若该函数返回 `Some(_)`,则返回 `Ok(previous_value)`(操作前的旧值);否则返回 `Err(previous_value)`。
    ///
    /// 另见:[`update`](`AtomicBool::update`)。
    ///
    /// 注意:只要该函数持续返回 `Some(_)`,当期间该值被其他线程改动时,本方法可能会
    /// 多次调用该函数;但该函数最终只会对一个已存储的值生效一次。
    ///
    /// `try_update` 接受两个 [`Ordering`] 参数来描述本操作的内存顺序。
    /// 第一个描述操作最终成功时所需的顺序,第二个描述 load 所需的顺序。
    /// 它们分别对应 [`AtomicBool::compare_exchange`] 的 success 与 failure 顺序。
    ///
    /// 把成功顺序设为 [`Acquire`] 会让本操作的 store 部分退化为 [`Relaxed`],
    /// 设为 [`Release`] 会让最终成功路径上的 load 退化为 [`Relaxed`]。
    /// (失败的)load 顺序只能是 [`SeqCst`]、[`Acquire`] 或 [`Relaxed`]。
    ///
    /// **注意:** 本方法只在支持 `u8` 原子操作的平台上可用。
    ///
    /// # 注意事项
    ///
    /// 本方法并不神奇:它不是硬件直接提供的,也不像临界区或互斥锁那样工作。
    ///
    /// 它是在原子 [compare-and-swap operation] 之上实现的,因此带有 CAS 操作惯有的缺陷。
    /// 特别地,要当心 [ABA problem]。
    ///
    /// [ABA Problem]: https://en.wikipedia.org/wiki/ABA_problem
    /// [compare-and-swap operation]: https://en.wikipedia.org/wiki/Compare-and-swap
    ///
    /// # 示例
    ///
    /// ```rust
    /// #![feature(atomic_try_update)]
    /// use std::sync::atomic::{AtomicBool, Ordering};
    ///
    /// let x = AtomicBool::new(false);
    /// assert_eq!(x.try_update(Ordering::SeqCst, Ordering::SeqCst, |_| None), Err(false));
    /// assert_eq!(x.try_update(Ordering::SeqCst, Ordering::SeqCst, |x| Some(!x)), Ok(false));
    /// assert_eq!(x.try_update(Ordering::SeqCst, Ordering::SeqCst, |x| Some(!x)), Ok(true));
    /// assert_eq!(x.load(Ordering::SeqCst), false);
    /// ```
    #[inline]
    #[unstable(feature = "atomic_try_update", issue = "135894")]
    #[cfg(target_has_atomic = "8")]
    #[cfg_attr(miri, track_caller)] // 即使不 panic,这也有助于 Miri 的回溯信息
    #[rustc_should_not_be_called_on_const_items]
    pub fn try_update(
        &self,
        set_order: Ordering,
        fetch_order: Ordering,
        f: impl FnMut(bool) -> Option<bool>,
    ) -> Result<bool, bool> {
        // FIXME(atomic_try_update): 目前这是 `fetch_update` 的一个 unstable 别名;
        //      稳定化时,应把 `fetch_update` 改成 `try_update` 的 deprecated 别名。
        self.fetch_update(set_order, fetch_order, f)
    }

    /// 取出当前值,对它应用一个返回新值的函数。新值被存入,旧值被返回。
    ///
    /// 另见:[`try_update`](`AtomicBool::try_update`)。
    ///
    /// 注意:当期间该值被其他线程改动时,本方法可能会多次调用该函数;
    /// 但该函数最终只会对一个已存储的值生效一次。
    ///
    /// `update` 接受两个 [`Ordering`] 参数来描述本操作的内存顺序。
    /// 第一个描述操作最终成功时所需的顺序,第二个描述 load 所需的顺序。
    /// 它们分别对应 [`AtomicBool::compare_exchange`] 的 success 与 failure 顺序。
    ///
    /// 把成功顺序设为 [`Acquire`] 会让本操作的 store 部分退化为 [`Relaxed`],
    /// 设为 [`Release`] 会让最终成功路径上的 load 退化为 [`Relaxed`]。
    /// (失败的)load 顺序只能是 [`SeqCst`]、[`Acquire`] 或 [`Relaxed`]。
    ///
    /// **注意:** 本方法只在支持 `u8` 原子操作的平台上可用。
    ///
    /// # 注意事项
    ///
    /// 本方法并不神奇:它不是硬件直接提供的,也不像临界区或互斥锁那样工作。
    ///
    /// 它是在原子 [compare-and-swap operation] 之上实现的,因此带有 CAS 操作惯有的缺陷。
    /// 特别地,要当心 [ABA problem]。
    ///
    /// [ABA Problem]: https://en.wikipedia.org/wiki/ABA_problem
    /// [compare-and-swap operation]: https://en.wikipedia.org/wiki/Compare-and-swap
    ///
    /// # 示例
    ///
    /// ```rust
    /// #![feature(atomic_try_update)]
    ///
    /// use std::sync::atomic::{AtomicBool, Ordering};
    ///
    /// let x = AtomicBool::new(false);
    /// assert_eq!(x.update(Ordering::SeqCst, Ordering::SeqCst, |x| !x), false);
    /// assert_eq!(x.update(Ordering::SeqCst, Ordering::SeqCst, |x| !x), true);
    /// assert_eq!(x.load(Ordering::SeqCst), false);
    /// ```
    #[inline]
    #[unstable(feature = "atomic_try_update", issue = "135894")]
    #[cfg(target_has_atomic = "8")]
    #[cfg_attr(miri, track_caller)] // 即使不 panic,这也有助于 Miri 的回溯信息
    #[rustc_should_not_be_called_on_const_items]
    pub fn update(
        &self,
        set_order: Ordering,
        fetch_order: Ordering,
        mut f: impl FnMut(bool) -> bool,
    ) -> bool {
        let mut prev = self.load(fetch_order);
        loop {
            match self.compare_exchange_weak(prev, f(prev), set_order, fetch_order) {
                Ok(x) => break x,
                Err(next_prev) => prev = next_prev,
            }
        }
    }
}

#[cfg(target_has_atomic_load_store = "ptr")]
impl<T> AtomicPtr<T> {
    /// 创建一个新的 `AtomicPtr`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::sync::atomic::AtomicPtr;
    ///
    /// let ptr = &mut 5;
    /// let atomic_ptr = AtomicPtr::new(ptr);
    /// ```
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_atomic_new", since = "1.24.0")]
    pub const fn new(p: *mut T) -> AtomicPtr<T> {
        AtomicPtr { p: UnsafeCell::new(p) }
    }

    /// 从一个指针创建一个新的 `AtomicPtr`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::sync::atomic::{self, AtomicPtr};
    ///
    /// // 取得指向一块已分配值的指针
    /// let ptr: *mut *mut u8 = Box::into_raw(Box::new(std::ptr::null_mut()));
    ///
    /// assert!(ptr.cast::<AtomicPtr<u8>>().is_aligned());
    ///
    /// {
    ///     // 为这块已分配的值创建一个原子视图
    ///     let atomic = unsafe { AtomicPtr::from_ptr(ptr) };
    ///
    ///     // 用 `atomic` 做原子操作,也可以把它分享给其他线程
    ///     atomic.store(std::ptr::NonNull::dangling().as_ptr(), atomic::Ordering::Relaxed);
    /// }
    ///
    /// // 此时非原子地访问 `ptr` 背后的值是可以的,
    /// // 因为指向该原子的引用已在上面的代码块中结束了生命周期
    /// assert!(!unsafe { *ptr }.is_null());
    ///
    /// // 释放该值
    /// unsafe { drop(Box::from_raw(ptr)) }
    /// ```
    ///
    /// # 安全性(Safety)
    ///
    /// * `ptr` 必须按 `align_of::<AtomicPtr<T>>()` 对齐(注意:在某些平台上这可能比
    ///   `align_of::<*mut T>()` 更大)。
    /// * `ptr` 在整个生命周期 `'a` 内必须对读和写都 [valid]。
    /// * 你必须遵守 [Memory model for atomic accesses]。特别地,不允许在没有同步的情况下
    ///   混用冲突的原子访问与非原子访问,也不允许混用不同大小的原子访问。
    ///
    /// [valid]: crate::ptr#safety
    /// [Memory model for atomic accesses]: self#memory-model-for-atomic-accesses
    #[inline]
    #[stable(feature = "atomic_from_ptr", since = "1.75.0")]
    #[rustc_const_stable(feature = "const_atomic_from_ptr", since = "1.84.0")]
    pub const unsafe fn from_ptr<'a>(ptr: *mut *mut T) -> &'a AtomicPtr<T> {
        // SAFETY: 由调用方保证。
        unsafe { &*ptr.cast() }
    }

    /// 创建一个以空指针初始化的新 `AtomicPtr`。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(atomic_ptr_null)]
    /// use std::sync::atomic::{AtomicPtr, Ordering};
    ///
    /// let atomic_ptr = AtomicPtr::<()>::null();
    /// assert!(atomic_ptr.load(Ordering::Relaxed).is_null());
    /// ```
    #[inline]
    #[must_use]
    #[unstable(feature = "atomic_ptr_null", issue = "150733")]
    pub const fn null() -> AtomicPtr<T> {
        AtomicPtr::new(crate::ptr::null_mut())
    }

    /// 返回指向底层指针的可变引用。
    ///
    /// 这是安全的,因为可变引用保证了没有其他线程在并发访问这块原子数据。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::sync::atomic::{AtomicPtr, Ordering};
    ///
    /// let mut data = 10;
    /// let mut atomic_ptr = AtomicPtr::new(&mut data);
    /// let mut other_data = 5;
    /// *atomic_ptr.get_mut() = &mut other_data;
    /// assert_eq!(unsafe { *atomic_ptr.load(Ordering::SeqCst) }, 5);
    /// ```
    #[inline]
    #[stable(feature = "atomic_access", since = "1.15.0")]
    pub fn get_mut(&mut self) -> &mut *mut T {
        self.p.get_mut()
    }

    /// 取得对一个指针的原子访问。
    ///
    /// **注意:** 本函数只在 `AtomicPtr<T>` 与 `*const T` 对齐相同的目标平台上可用。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(atomic_from_mut)]
    /// use std::sync::atomic::{AtomicPtr, Ordering};
    ///
    /// let mut data = 123;
    /// let mut some_ptr = &mut data as *mut i32;
    /// let a = AtomicPtr::from_mut(&mut some_ptr);
    /// let mut other_data = 456;
    /// a.store(&mut other_data, Ordering::Relaxed);
    /// assert_eq!(unsafe { *some_ptr }, 456);
    /// ```
    #[inline]
    #[cfg(target_has_atomic_equal_alignment = "ptr")]
    #[unstable(feature = "atomic_from_mut", issue = "76314")]
    pub fn from_mut(v: &mut *mut T) -> &mut Self {
        let [] = [(); align_of::<AtomicPtr<()>>() - align_of::<*mut ()>()];
        // SAFETY:
        //  - 可变引用保证了独占所有权。
        //  - 在 rust 支持的所有平台上,`*mut T` 与 `Self` 的对齐都相同,如上面所验证。
        unsafe { &mut *(v as *mut *mut T as *mut Self) }
    }

    /// 取得对一个 `&mut [AtomicPtr]` 切片的非原子访问。
    ///
    /// 这是安全的,因为可变引用保证了没有其他线程在并发访问这块原子数据。
    ///
    /// # 示例
    ///
    /// ```ignore-wasm
    /// #![feature(atomic_from_mut)]
    /// use std::ptr::null_mut;
    /// use std::sync::atomic::{AtomicPtr, Ordering};
    ///
    /// let mut some_ptrs = [const { AtomicPtr::new(null_mut::<String>()) }; 10];
    ///
    /// let view: &mut [*mut String] = AtomicPtr::get_mut_slice(&mut some_ptrs);
    /// assert_eq!(view, [null_mut::<String>(); 10]);
    /// view
    ///     .iter_mut()
    ///     .enumerate()
    ///     .for_each(|(i, ptr)| *ptr = Box::into_raw(Box::new(format!("iteration#{i}"))));
    ///
    /// std::thread::scope(|s| {
    ///     for ptr in &some_ptrs {
    ///         s.spawn(move || {
    ///             let ptr = ptr.load(Ordering::Relaxed);
    ///             assert!(!ptr.is_null());
    ///
    ///             let name = unsafe { Box::from_raw(ptr) };
    ///             println!("Hello, {name}!");
    ///         });
    ///     }
    /// });
    /// ```
    #[inline]
    #[unstable(feature = "atomic_from_mut", issue = "76314")]
    pub fn get_mut_slice(this: &mut [Self]) -> &mut [*mut T] {
        // SAFETY: 可变引用保证了独占所有权。
        unsafe { &mut *(this as *mut [Self] as *mut [*mut T]) }
    }

    /// 取得对一个指针切片的原子访问。
    ///
    /// **注意:** 本函数只在 `AtomicPtr<T>` 与 `*const T` 对齐相同的目标平台上可用。
    ///
    /// # 示例
    ///
    /// ```ignore-wasm
    /// #![feature(atomic_from_mut)]
    /// use std::ptr::null_mut;
    /// use std::sync::atomic::{AtomicPtr, Ordering};
    ///
    /// let mut some_ptrs = [null_mut::<String>(); 10];
    /// let a = &*AtomicPtr::from_mut_slice(&mut some_ptrs);
    /// std::thread::scope(|s| {
    ///     for i in 0..a.len() {
    ///         s.spawn(move || {
    ///             let name = Box::new(format!("thread{i}"));
    ///             a[i].store(Box::into_raw(name), Ordering::Relaxed);
    ///         });
    ///     }
    /// });
    /// for p in some_ptrs {
    ///     assert!(!p.is_null());
    ///     let name = unsafe { Box::from_raw(p) };
    ///     println!("Hello, {name}!");
    /// }
    /// ```
    #[inline]
    #[cfg(target_has_atomic_equal_alignment = "ptr")]
    #[unstable(feature = "atomic_from_mut", issue = "76314")]
    pub fn from_mut_slice(v: &mut [*mut T]) -> &mut [Self] {
        // SAFETY:
        //  - 可变引用保证了独占所有权。
        //  - 在 rust 支持的所有平台上,`*mut T` 与 `Self` 的对齐都相同,如上面所验证。
        unsafe { &mut *(v as *mut [*mut T] as *mut [Self]) }
    }

    /// 消耗这个原子,返回它所包含的值。
    ///
    /// 这是安全的,因为按值传入 `self` 保证了没有其他线程在并发访问这块原子数据。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::sync::atomic::AtomicPtr;
    ///
    /// let mut data = 5;
    /// let atomic_ptr = AtomicPtr::new(&mut data);
    /// assert_eq!(unsafe { *atomic_ptr.into_inner() }, 5);
    /// ```
    #[inline]
    #[stable(feature = "atomic_access", since = "1.15.0")]
    #[rustc_const_stable(feature = "const_atomic_into_inner", since = "1.79.0")]
    pub const fn into_inner(self) -> *mut T {
        self.p.into_inner()
    }

    /// 从指针中 load 出一个值。
    ///
    /// `load` 接受一个 [`Ordering`] 参数来描述本操作的内存顺序。
    /// 可用的取值有 [`SeqCst`]、[`Acquire`] 和 [`Relaxed`]。
    ///
    /// # Panics
    ///
    /// 当 `order` 为 [`Release`] 或 [`AcqRel`] 时会 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::sync::atomic::{AtomicPtr, Ordering};
    ///
    /// let ptr = &mut 5;
    /// let some_ptr = AtomicPtr::new(ptr);
    ///
    /// let value = some_ptr.load(Ordering::Relaxed);
    /// ```
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[cfg_attr(miri, track_caller)] // 即使不 panic,这也有助于 Miri 的回溯信息
    pub fn load(&self, order: Ordering) -> *mut T {
        // SAFETY: 数据竞争由原子 intrinsic 防止。
        unsafe { atomic_load(self.p.get(), order) }
    }

    /// 把一个值 store 进指针。
    ///
    /// `store` 接受一个 [`Ordering`] 参数来描述本操作的内存顺序。
    /// 可用的取值有 [`SeqCst`]、[`Release`] 和 [`Relaxed`]。
    ///
    /// # Panics
    ///
    /// 当 `order` 为 [`Acquire`] 或 [`AcqRel`] 时会 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::sync::atomic::{AtomicPtr, Ordering};
    ///
    /// let ptr = &mut 5;
    /// let some_ptr = AtomicPtr::new(ptr);
    ///
    /// let other_ptr = &mut 10;
    ///
    /// some_ptr.store(other_ptr, Ordering::Relaxed);
    /// ```
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[cfg_attr(miri, track_caller)] // 即使不 panic,这也有助于 Miri 的回溯信息
    #[rustc_should_not_be_called_on_const_items]
    pub fn store(&self, ptr: *mut T, order: Ordering) {
        // SAFETY: 数据竞争由原子 intrinsic 防止。
        unsafe {
            atomic_store(self.p.get(), ptr, order);
        }
    }

    /// 把一个值 store 进指针,并返回操作前的旧值。
    ///
    /// `swap` 接受一个 [`Ordering`] 参数来描述本操作的内存顺序。
    /// 所有顺序模式都允许。注意:用 [`Acquire`] 会让本操作的 store 部分退化为 [`Relaxed`],
    /// 用 [`Release`] 会让 load 部分退化为 [`Relaxed`];只有 [`AcqRel`] 或 [`SeqCst`] 才同时具备两端语义。
    ///
    /// **注意:** 本方法只在支持指针原子操作的平台上可用。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::sync::atomic::{AtomicPtr, Ordering};
    ///
    /// let ptr = &mut 5;
    /// let some_ptr = AtomicPtr::new(ptr);
    ///
    /// let other_ptr = &mut 10;
    ///
    /// let value = some_ptr.swap(other_ptr, Ordering::Relaxed);
    /// ```
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[cfg(target_has_atomic = "ptr")]
    #[cfg_attr(miri, track_caller)] // 即使不 panic,这也有助于 Miri 的回溯信息
    #[rustc_should_not_be_called_on_const_items]
    pub fn swap(&self, ptr: *mut T, order: Ordering) -> *mut T {
        // SAFETY: 数据竞争由原子 intrinsic 防止。
        unsafe { atomic_swap(self.p.get(), ptr, order) }
    }

    /// 当且仅当当前值与 `current` 相同时,把 `new` 写入指针。
    ///
    /// 返回值始终是操作前的旧值。如果它等于 `current`,说明值已被更新。
    ///
    /// `compare_and_swap` 也接受一个 [`Ordering`] 参数来描述本操作的内存顺序。
    /// 注意:即便使用 [`AcqRel`],操作仍可能失败,此时它只执行一次 `Acquire` load,而不具备 `Release` 语义。
    /// 用 [`Acquire`] 会让本操作(若真的发生)的 store 部分退化为 [`Relaxed`],
    /// 用 [`Release`] 会让 load 部分退化为 [`Relaxed`]。
    ///
    /// **注意:** 本方法只在支持指针原子操作的平台上可用。
    ///
    /// # 迁移到 `compare_exchange` 和 `compare_exchange_weak`
    ///
    /// `compare_and_swap` 等价于按下表映射内存顺序的 `compare_exchange`:
    ///
    /// 原顺序   | Success | Failure
    /// -------- | ------- | -------
    /// Relaxed  | Relaxed | Relaxed
    /// Acquire  | Acquire | Acquire
    /// Release  | Release | Relaxed
    /// AcqRel   | AcqRel  | Acquire
    /// SeqCst   | SeqCst  | SeqCst
    ///
    /// `compare_and_swap` 与 `compare_exchange` 的返回类型也不同。你可以用
    /// `compare_exchange(...).unwrap_or_else(|x| x)` 来恢复 `compare_and_swap` 的行为,
    /// 但大多数情况下,更地道的做法是检查返回值是 `Ok` 还是 `Err`,
    /// 而不是根据读到的值去推断成功还是失败。
    ///
    /// 迁移期间,也可以考虑改用 `compare_exchange_weak` 是否更合适。
    /// `compare_exchange_weak` 允许在比较成功时仍偶发失败,这能让编译器在“比较并交换”被用于
    /// 循环中时生成更优的汇编代码。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::sync::atomic::{AtomicPtr, Ordering};
    ///
    /// let ptr = &mut 5;
    /// let some_ptr = AtomicPtr::new(ptr);
    ///
    /// let other_ptr = &mut 10;
    ///
    /// let value = some_ptr.compare_and_swap(ptr, other_ptr, Ordering::Relaxed);
    /// ```
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[deprecated(
        since = "1.50.0",
        note = "Use `compare_exchange` or `compare_exchange_weak` instead"
    )]
    #[cfg(target_has_atomic = "ptr")]
    #[cfg_attr(miri, track_caller)] // 即使不 panic,这也有助于 Miri 的回溯信息
    #[rustc_should_not_be_called_on_const_items]
    pub fn compare_and_swap(&self, current: *mut T, new: *mut T, order: Ordering) -> *mut T {
        match self.compare_exchange(current, new, order, strongest_failure_ordering(order)) {
            Ok(x) => x,
            Err(x) => x,
        }
    }

    /// 当且仅当当前值与 `current` 相同时,把 `new` 写入指针。
    ///
    /// 返回值是一个 `Result`,指示新值是否被写入,并携带操作前的旧值。
    /// 成功时该值保证等于 `current`。
    ///
    /// `compare_exchange` 接受两个 [`Ordering`] 参数来描述本操作的内存顺序。
    /// `success` 描述比较成功时所执行的“读-改-写”操作所需的顺序;
    /// `failure` 描述比较失败时所执行的 load 操作所需的顺序。
    /// 把 `success` 设为 [`Acquire`] 会让本操作的 store 部分退化为 [`Relaxed`],
    /// 设为 [`Release`] 会让成功路径上的 load 部分退化为 [`Relaxed`]。
    /// `failure` 顺序只能是 [`SeqCst`]、[`Acquire`] 或 [`Relaxed`];
    /// 传入 [`Release`] 或 [`AcqRel`] 会触发 panic。
    ///
    /// **注意:** 本方法只在支持指针原子操作的平台上可用。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::sync::atomic::{AtomicPtr, Ordering};
    ///
    /// let ptr = &mut 5;
    /// let some_ptr = AtomicPtr::new(ptr);
    ///
    /// let other_ptr = &mut 10;
    ///
    /// let value = some_ptr.compare_exchange(ptr, other_ptr,
    ///                                       Ordering::SeqCst, Ordering::Relaxed);
    /// ```
    ///
    /// # 注意事项
    ///
    /// `compare_exchange` 是一个 [compare-and-swap operation],因此带有 CAS 操作惯有的缺陷。
    /// 特别地,“先 load 出一个值,再用这个旧值做一次成功的 `compare_exchange`” *并不能保证*
    /// 在这两步之间其他线程没有改动过该值。这一点在用 `compare_exchange` 里的*相等性*检查
    /// 来判断值的*同一性*时尤其重要 —— 相等并不必然意味着同一。对指针来说这尤其常见:
    /// 一个指针持有相同的地址,并不意味着该地址上还存在同一个对象!在这种场景下,
    /// `compare_exchange` 可能导致 [ABA problem](即值被改成别的再改回原值,CAS 仍判定相等而通过)。
    ///
    /// [ABA Problem]: https://en.wikipedia.org/wiki/ABA_problem
    /// [compare-and-swap operation]: https://en.wikipedia.org/wiki/Compare-and-swap
    #[inline]
    #[stable(feature = "extended_compare_and_swap", since = "1.10.0")]
    #[cfg(target_has_atomic = "ptr")]
    #[cfg_attr(miri, track_caller)] // 即使不 panic,这也有助于 Miri 的回溯信息
    #[rustc_should_not_be_called_on_const_items]
    pub fn compare_exchange(
        &self,
        current: *mut T,
        new: *mut T,
        success: Ordering,
        failure: Ordering,
    ) -> Result<*mut T, *mut T> {
        // SAFETY: 数据竞争由原子 intrinsic 防止。
        unsafe { atomic_compare_exchange(self.p.get(), current, new, success, failure) }
    }

    /// 当且仅当当前值与 `current` 相同时,把 `new` 写入指针。
    ///
    /// 与 [`AtomicPtr::compare_exchange`] 不同,本函数 **允许在比较成功时仍偶发失败
    /// (spurious failure)**,这能让某些平台生成更高效的代码。返回值是一个 `Result`,
    /// 指示新值是否被写入,并携带操作前的旧值。
    ///
    /// `compare_exchange_weak` 接受两个 [`Ordering`] 参数来描述本操作的内存顺序。
    /// `success` 描述比较成功时所执行的“读-改-写”操作所需的顺序;
    /// `failure` 描述比较失败时所执行的 load 操作所需的顺序。
    /// 把 `success` 设为 [`Acquire`] 会让本操作的 store 部分退化为 [`Relaxed`],
    /// 设为 [`Release`] 会让成功路径上的 load 部分退化为 [`Relaxed`]。
    /// `failure` 顺序只能是 [`SeqCst`]、[`Acquire`] 或 [`Relaxed`];
    /// 传入 [`Release`] 或 [`AcqRel`] 会触发 panic。
    ///
    /// **注意:** 本方法只在支持指针原子操作的平台上可用。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::sync::atomic::{AtomicPtr, Ordering};
    ///
    /// let some_ptr = AtomicPtr::new(&mut 5);
    ///
    /// let new = &mut 10;
    /// let mut old = some_ptr.load(Ordering::Relaxed);
    /// loop {
    ///     match some_ptr.compare_exchange_weak(old, new, Ordering::SeqCst, Ordering::Relaxed) {
    ///         Ok(_) => break,
    ///         Err(x) => old = x,
    ///     }
    /// }
    /// ```
    ///
    /// # 注意事项
    ///
    /// `compare_exchange` 是一个 [compare-and-swap operation],因此带有 CAS 操作惯有的缺陷。
    /// 特别地,“先 load 出一个值,再用这个旧值做一次成功的 `compare_exchange`” *并不能保证*
    /// 在这两步之间其他线程没有改动过该值。这一点在用 `compare_exchange` 里的*相等性*检查
    /// 来判断值的*同一性*时尤其重要 —— 相等并不必然意味着同一。对指针来说这尤其常见:
    /// 一个指针持有相同的地址,并不意味着该地址上还存在同一个对象!在这种场景下,
    /// `compare_exchange` 可能导致 [ABA problem](即值被改成别的再改回原值,CAS 仍判定相等而通过)。
    ///
    /// [ABA Problem]: https://en.wikipedia.org/wiki/ABA_problem
    /// [compare-and-swap operation]: https://en.wikipedia.org/wiki/Compare-and-swap
    #[inline]
    #[stable(feature = "extended_compare_and_swap", since = "1.10.0")]
    #[cfg(target_has_atomic = "ptr")]
    #[cfg_attr(miri, track_caller)] // 即使不 panic,这也有助于 Miri 的回溯信息
    #[rustc_should_not_be_called_on_const_items]
    pub fn compare_exchange_weak(
        &self,
        current: *mut T,
        new: *mut T,
        success: Ordering,
        failure: Ordering,
    ) -> Result<*mut T, *mut T> {
        // SAFETY: 这个 intrinsic 是 unsafe 的,因为它在裸指针上操作;
        // 但我们确知该指针是有效的(它刚从一个我们按引用持有的 `UnsafeCell` 取得),
        // 而原子操作本身允许我们安全地修改 `UnsafeCell` 的内容。
        unsafe { atomic_compare_exchange_weak(self.p.get(), current, new, success, failure) }
    }

    /// 取出当前值,并对它应用一个返回 `Option<新值>` 的函数。
    /// 若该函数返回 `Some(_)`,则返回 `Ok(previous_value)`(操作前的旧值);否则返回 `Err(previous_value)`。
    ///
    /// 注意:只要该函数持续返回 `Some(_)`,当期间该值被其他线程改动时,本方法可能会
    /// 多次调用该函数;但该函数最终只会对一个已存储的值生效一次。
    ///
    /// `fetch_update` 接受两个 [`Ordering`] 参数来描述本操作的内存顺序。
    /// 第一个描述操作最终成功时所需的顺序,第二个描述 load 所需的顺序。
    /// 它们分别对应 [`AtomicPtr::compare_exchange`] 的 success 与 failure 顺序。
    ///
    /// 把成功顺序设为 [`Acquire`] 会让本操作的 store 部分退化为 [`Relaxed`],
    /// 设为 [`Release`] 会让最终成功路径上的 load 退化为 [`Relaxed`]。
    /// (失败的)load 顺序只能是 [`SeqCst`]、[`Acquire`] 或 [`Relaxed`]。
    ///
    /// **注意:** 本方法只在支持指针原子操作的平台上可用。
    ///
    /// # 注意事项
    ///
    /// 本方法并不神奇:它不是硬件直接提供的,也不像临界区或互斥锁那样工作。
    ///
    /// 它是在原子 [compare-and-swap operation] 之上实现的,因此带有 CAS 操作惯有的缺陷。
    /// 特别地,要当心 [ABA problem] —— 这对指针来说是一个尤其常见的陷阱!
    ///
    /// [ABA Problem]: https://en.wikipedia.org/wiki/ABA_problem
    /// [compare-and-swap operation]: https://en.wikipedia.org/wiki/Compare-and-swap
    ///
    /// # 示例
    ///
    /// ```rust
    /// use std::sync::atomic::{AtomicPtr, Ordering};
    ///
    /// let ptr: *mut _ = &mut 5;
    /// let some_ptr = AtomicPtr::new(ptr);
    ///
    /// let new: *mut _ = &mut 10;
    /// assert_eq!(some_ptr.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |_| None), Err(ptr));
    /// let result = some_ptr.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |x| {
    ///     if x == ptr {
    ///         Some(new)
    ///     } else {
    ///         None
    ///     }
    /// });
    /// assert_eq!(result, Ok(ptr));
    /// assert_eq!(some_ptr.load(Ordering::SeqCst), new);
    /// ```
    #[inline]
    #[stable(feature = "atomic_fetch_update", since = "1.53.0")]
    #[cfg(target_has_atomic = "ptr")]
    #[cfg_attr(miri, track_caller)] // 即使不 panic,这也有助于 Miri 的回溯信息
    #[rustc_should_not_be_called_on_const_items]
    pub fn fetch_update<F>(
        &self,
        set_order: Ordering,
        fetch_order: Ordering,
        mut f: F,
    ) -> Result<*mut T, *mut T>
    where
        F: FnMut(*mut T) -> Option<*mut T>,
    {
        let mut prev = self.load(fetch_order);
        while let Some(next) = f(prev) {
            match self.compare_exchange_weak(prev, next, set_order, fetch_order) {
                x @ Ok(_) => return x,
                Err(next_prev) => prev = next_prev,
            }
        }
        Err(prev)
    }
    /// 取出当前值,并对它应用一个返回 `Option<新值>` 的函数。
    /// 若该函数返回 `Some(_)`,则返回 `Ok(previous_value)`(操作前的旧值);否则返回 `Err(previous_value)`。
    ///
    /// 另见:[`update`](`AtomicPtr::update`)。
    ///
    /// 注意:只要该函数持续返回 `Some(_)`,当期间该值被其他线程改动时,本方法可能会
    /// 多次调用该函数;但该函数最终只会对一个已存储的值生效一次。
    ///
    /// `try_update` 接受两个 [`Ordering`] 参数来描述本操作的内存顺序。
    /// 第一个描述操作最终成功时所需的顺序,第二个描述 load 所需的顺序。
    /// 它们分别对应 [`AtomicPtr::compare_exchange`] 的 success 与 failure 顺序。
    ///
    /// 把成功顺序设为 [`Acquire`] 会让本操作的 store 部分退化为 [`Relaxed`],
    /// 设为 [`Release`] 会让最终成功路径上的 load 退化为 [`Relaxed`]。
    /// (失败的)load 顺序只能是 [`SeqCst`]、[`Acquire`] 或 [`Relaxed`]。
    ///
    /// **注意:** 本方法只在支持指针原子操作的平台上可用。
    ///
    /// # 注意事项
    ///
    /// 本方法并不神奇:它不是硬件直接提供的,也不像临界区或互斥锁那样工作。
    ///
    /// 它是在原子 [compare-and-swap operation] 之上实现的,因此带有 CAS 操作惯有的缺陷。
    /// 特别地,要当心 [ABA problem] —— 这对指针来说是一个尤其常见的陷阱!
    ///
    /// [ABA Problem]: https://en.wikipedia.org/wiki/ABA_problem
    /// [compare-and-swap operation]: https://en.wikipedia.org/wiki/Compare-and-swap
    ///
    /// # 示例
    ///
    /// ```rust
    /// #![feature(atomic_try_update)]
    /// use std::sync::atomic::{AtomicPtr, Ordering};
    ///
    /// let ptr: *mut _ = &mut 5;
    /// let some_ptr = AtomicPtr::new(ptr);
    ///
    /// let new: *mut _ = &mut 10;
    /// assert_eq!(some_ptr.try_update(Ordering::SeqCst, Ordering::SeqCst, |_| None), Err(ptr));
    /// let result = some_ptr.try_update(Ordering::SeqCst, Ordering::SeqCst, |x| {
    ///     if x == ptr {
    ///         Some(new)
    ///     } else {
    ///         None
    ///     }
    /// });
    /// assert_eq!(result, Ok(ptr));
    /// assert_eq!(some_ptr.load(Ordering::SeqCst), new);
    /// ```
    #[inline]
    #[unstable(feature = "atomic_try_update", issue = "135894")]
    #[cfg(target_has_atomic = "ptr")]
    #[cfg_attr(miri, track_caller)] // 即使不 panic,这也有助于 Miri 的回溯信息
    #[rustc_should_not_be_called_on_const_items]
    pub fn try_update(
        &self,
        set_order: Ordering,
        fetch_order: Ordering,
        f: impl FnMut(*mut T) -> Option<*mut T>,
    ) -> Result<*mut T, *mut T> {
        // FIXME(atomic_try_update): 目前这是 `fetch_update` 的一个 unstable 别名;
        //      稳定化时,应把 `fetch_update` 改成 `try_update` 的 deprecated 别名。
        self.fetch_update(set_order, fetch_order, f)
    }

    /// 取出当前值,对它应用一个返回新值的函数。新值被存入,旧值被返回。
    ///
    /// 另见:[`try_update`](`AtomicPtr::try_update`)。
    ///
    /// 注意:当期间该值被其他线程改动时,本方法可能会多次调用该函数;
    /// 但该函数最终只会对一个已存储的值生效一次。
    ///
    /// `update` 接受两个 [`Ordering`] 参数来描述本操作的内存顺序。
    /// 第一个描述操作最终成功时所需的顺序,第二个描述 load 所需的顺序。
    /// 它们分别对应 [`AtomicPtr::compare_exchange`] 的 success 与 failure 顺序。
    ///
    /// 把成功顺序设为 [`Acquire`] 会让本操作的 store 部分退化为 [`Relaxed`],
    /// 设为 [`Release`] 会让最终成功路径上的 load 退化为 [`Relaxed`]。
    /// (失败的)load 顺序只能是 [`SeqCst`]、[`Acquire`] 或 [`Relaxed`]。
    ///
    /// **注意:** 本方法只在支持指针原子操作的平台上可用。
    ///
    /// # 注意事项
    ///
    /// 本方法并不神奇:它不是硬件直接提供的,也不像临界区或互斥锁那样工作。
    ///
    /// 它是在原子 [compare-and-swap operation] 之上实现的,因此带有 CAS 操作惯有的缺陷。
    /// 特别地,要当心 [ABA problem] —— 这对指针来说是一个尤其常见的陷阱!
    ///
    /// [ABA Problem]: https://en.wikipedia.org/wiki/ABA_problem
    /// [compare-and-swap operation]: https://en.wikipedia.org/wiki/Compare-and-swap
    ///
    /// # 示例
    ///
    /// ```rust
    /// #![feature(atomic_try_update)]
    ///
    /// use std::sync::atomic::{AtomicPtr, Ordering};
    ///
    /// let ptr: *mut _ = &mut 5;
    /// let some_ptr = AtomicPtr::new(ptr);
    ///
    /// let new: *mut _ = &mut 10;
    /// let result = some_ptr.update(Ordering::SeqCst, Ordering::SeqCst, |_| new);
    /// assert_eq!(result, ptr);
    /// assert_eq!(some_ptr.load(Ordering::SeqCst), new);
    /// ```
    #[inline]
    #[unstable(feature = "atomic_try_update", issue = "135894")]
    #[cfg(target_has_atomic = "8")]
    #[cfg_attr(miri, track_caller)] // 即使不 panic,这也有助于 Miri 的回溯信息
    #[rustc_should_not_be_called_on_const_items]
    pub fn update(
        &self,
        set_order: Ordering,
        fetch_order: Ordering,
        mut f: impl FnMut(*mut T) -> *mut T,
    ) -> *mut T {
        let mut prev = self.load(fetch_order);
        loop {
            match self.compare_exchange_weak(prev, f(prev), set_order, fetch_order) {
                Ok(x) => break x,
                Err(next_prev) => prev = next_prev,
            }
        }
    }

    /// 通过加上 `val`(以 `T` 为单位)来偏移指针的地址,并返回操作前的旧指针。
    ///
    /// 这等价于用 [`wrapping_add`] 原子地执行 `ptr = ptr.wrapping_add(val);`。
    ///
    /// 本方法以 `T` 为单位操作,这意味着它无法把指针偏移一个不是 `size_of::<T>()`
    /// 整数倍的量。有时这会带来不便 —— 比如你想刻意构造一个未对齐的指针。
    /// 在那种情况下,可以改用 [`fetch_byte_add`](Self::fetch_byte_add) 方法。
    ///
    /// `fetch_ptr_add` 接受一个 [`Ordering`] 参数来描述本操作的内存顺序。
    /// 所有顺序模式都允许。注意:用 [`Acquire`] 会让本操作的 store 部分退化为 [`Relaxed`],
    /// 用 [`Release`] 会让 load 部分退化为 [`Relaxed`];只有 [`AcqRel`] 或 [`SeqCst`] 才同时具备两端语义。
    ///
    /// **注意**:本方法只在支持 [`AtomicPtr`] 原子操作的平台上可用。
    ///
    /// [`wrapping_add`]: pointer::wrapping_add
    ///
    /// # 示例
    ///
    /// ```
    /// use core::sync::atomic::{AtomicPtr, Ordering};
    ///
    /// let atom = AtomicPtr::<i64>::new(core::ptr::null_mut());
    /// assert_eq!(atom.fetch_ptr_add(1, Ordering::Relaxed).addr(), 0);
    /// // 注意:这里的单位是 `size_of::<i64>()`。
    /// assert_eq!(atom.load(Ordering::Relaxed).addr(), 8);
    /// ```
    #[inline]
    #[cfg(target_has_atomic = "ptr")]
    #[stable(feature = "strict_provenance_atomic_ptr", since = "1.91.0")]
    #[cfg_attr(miri, track_caller)] // 即使不 panic,这也有助于 Miri 的回溯信息
    #[rustc_should_not_be_called_on_const_items]
    pub fn fetch_ptr_add(&self, val: usize, order: Ordering) -> *mut T {
        self.fetch_byte_add(val.wrapping_mul(size_of::<T>()), order)
    }

    /// 通过减去 `val`(以 `T` 为单位)来偏移指针的地址,并返回操作前的旧指针。
    ///
    /// 这等价于用 [`wrapping_sub`] 原子地执行 `ptr = ptr.wrapping_sub(val);`。
    ///
    /// 本方法以 `T` 为单位操作,这意味着它无法把指针偏移一个不是 `size_of::<T>()`
    /// 整数倍的量。有时这会带来不便 —— 比如你想刻意构造一个未对齐的指针。
    /// 在那种情况下,可以改用 [`fetch_byte_sub`](Self::fetch_byte_sub) 方法。
    ///
    /// `fetch_ptr_sub` 接受一个 [`Ordering`] 参数来描述本操作的内存顺序。
    /// 所有顺序模式都允许。注意:用 [`Acquire`] 会让本操作的 store 部分退化为 [`Relaxed`],
    /// 用 [`Release`] 会让 load 部分退化为 [`Relaxed`];只有 [`AcqRel`] 或 [`SeqCst`] 才同时具备两端语义。
    ///
    /// **注意**:本方法只在支持 [`AtomicPtr`] 原子操作的平台上可用。
    ///
    /// [`wrapping_sub`]: pointer::wrapping_sub
    ///
    /// # 示例
    ///
    /// ```
    /// use core::sync::atomic::{AtomicPtr, Ordering};
    ///
    /// let array = [1i32, 2i32];
    /// let atom = AtomicPtr::new(array.as_ptr().wrapping_add(1) as *mut _);
    ///
    /// assert!(core::ptr::eq(
    ///     atom.fetch_ptr_sub(1, Ordering::Relaxed),
    ///     &array[1],
    /// ));
    /// assert!(core::ptr::eq(atom.load(Ordering::Relaxed), &array[0]));
    /// ```
    #[inline]
    #[cfg(target_has_atomic = "ptr")]
    #[stable(feature = "strict_provenance_atomic_ptr", since = "1.91.0")]
    #[cfg_attr(miri, track_caller)] // 即使不 panic,这也有助于 Miri 的回溯信息
    #[rustc_should_not_be_called_on_const_items]
    pub fn fetch_ptr_sub(&self, val: usize, order: Ordering) -> *mut T {
        self.fetch_byte_sub(val.wrapping_mul(size_of::<T>()), order)
    }

    /// 通过加上 `val` *字节* 来偏移指针的地址,并返回操作前的旧指针。
    ///
    /// 这等价于用 [`wrapping_byte_add`] 原子地执行 `ptr = ptr.wrapping_byte_add(val)`。
    ///
    /// `fetch_byte_add` 接受一个 [`Ordering`] 参数来描述本操作的内存顺序。
    /// 所有顺序模式都允许。注意:用 [`Acquire`] 会让本操作的 store 部分退化为 [`Relaxed`],
    /// 用 [`Release`] 会让 load 部分退化为 [`Relaxed`];只有 [`AcqRel`] 或 [`SeqCst`] 才同时具备两端语义。
    ///
    /// **注意**:本方法只在支持 [`AtomicPtr`] 原子操作的平台上可用。
    ///
    /// [`wrapping_byte_add`]: pointer::wrapping_byte_add
    ///
    /// # 示例
    ///
    /// ```
    /// use core::sync::atomic::{AtomicPtr, Ordering};
    ///
    /// let atom = AtomicPtr::<i64>::new(core::ptr::null_mut());
    /// assert_eq!(atom.fetch_byte_add(1, Ordering::Relaxed).addr(), 0);
    /// // 注意:这里的单位是字节,而不是 `size_of::<i64>()`。
    /// assert_eq!(atom.load(Ordering::Relaxed).addr(), 1);
    /// ```
    #[inline]
    #[cfg(target_has_atomic = "ptr")]
    #[stable(feature = "strict_provenance_atomic_ptr", since = "1.91.0")]
    #[cfg_attr(miri, track_caller)] // 即使不 panic,这也有助于 Miri 的回溯信息
    #[rustc_should_not_be_called_on_const_items]
    pub fn fetch_byte_add(&self, val: usize, order: Ordering) -> *mut T {
        // SAFETY: 数据竞争由原子 intrinsic 防止。
        unsafe { atomic_add(self.p.get(), val, order).cast() }
    }

    /// 通过减去 `val` *字节* 来偏移指针的地址,并返回操作前的旧指针。
    ///
    /// 这等价于用 [`wrapping_byte_sub`] 原子地执行 `ptr = ptr.wrapping_byte_sub(val)`。
    ///
    /// `fetch_byte_sub` 接受一个 [`Ordering`] 参数来描述本操作的内存顺序。
    /// 所有顺序模式都允许。注意:用 [`Acquire`] 会让本操作的 store 部分退化为 [`Relaxed`],
    /// 用 [`Release`] 会让 load 部分退化为 [`Relaxed`];只有 [`AcqRel`] 或 [`SeqCst`] 才同时具备两端语义。
    ///
    /// **注意**:本方法只在支持 [`AtomicPtr`] 原子操作的平台上可用。
    ///
    /// [`wrapping_byte_sub`]: pointer::wrapping_byte_sub
    ///
    /// # 示例
    ///
    /// ```
    /// use core::sync::atomic::{AtomicPtr, Ordering};
    ///
    /// let mut arr = [0i64, 1];
    /// let atom = AtomicPtr::<i64>::new(&raw mut arr[1]);
    /// assert_eq!(atom.fetch_byte_sub(8, Ordering::Relaxed).addr(), (&raw const arr[1]).addr());
    /// assert_eq!(atom.load(Ordering::Relaxed).addr(), (&raw const arr[0]).addr());
    /// ```
    #[inline]
    #[cfg(target_has_atomic = "ptr")]
    #[stable(feature = "strict_provenance_atomic_ptr", since = "1.91.0")]
    #[cfg_attr(miri, track_caller)] // 即使不 panic,这也有助于 Miri 的回溯信息
    #[rustc_should_not_be_called_on_const_items]
    pub fn fetch_byte_sub(&self, val: usize, order: Ordering) -> *mut T {
        // SAFETY: 数据竞争由原子 intrinsic 防止。
        unsafe { atomic_sub(self.p.get(), val, order).cast() }
    }

    /// 对当前指针的地址与参数 `val` 做按位“或”(or)运算,
    /// 并存入一个指针 —— 该指针带有当前指针的 provenance(出处)和运算得到的新地址。
    ///
    /// 这等价于用 [`map_addr`] 原子地执行 `ptr = ptr.map_addr(|a| a | val)`。
    /// 可用于带标记位的指针(tagged pointer)方案中原子地设置标记位。
    ///
    /// **告诫**:本操作返回的是操作前的旧值。若想在不丢失 provenance 的前提下计算出已存入的值,
    /// 可以使用 [`map_addr`],例如:`a.fetch_or(val).map_addr(|a| a | val)`。
    ///
    /// `fetch_or` 接受一个 [`Ordering`] 参数来描述本操作的内存顺序。
    /// 所有顺序模式都允许。注意:用 [`Acquire`] 会让本操作的 store 部分退化为 [`Relaxed`],
    /// 用 [`Release`] 会让 load 部分退化为 [`Relaxed`];只有 [`AcqRel`] 或 [`SeqCst`] 才同时具备两端语义。
    ///
    /// **注意**:本方法只在支持 [`AtomicPtr`] 原子操作的平台上可用。
    ///
    /// 本 API 及其所声称的语义是 Strict Provenance 实验的一部分,
    /// 详见 [`ptr` 的模块文档][crate::ptr]。
    ///
    /// [`map_addr`]: pointer::map_addr
    ///
    /// # 示例
    ///
    /// ```
    /// use core::sync::atomic::{AtomicPtr, Ordering};
    ///
    /// let pointer = &mut 3i64 as *mut i64;
    ///
    /// let atom = AtomicPtr::<i64>::new(pointer);
    /// // 给指针的最低位打标记。
    /// assert_eq!(atom.fetch_or(1, Ordering::Relaxed).addr() & 1, 0);
    /// // 取出并去除标记。
    /// let tagged = atom.load(Ordering::Relaxed);
    /// assert_eq!(tagged.addr() & 1, 1);
    /// assert_eq!(tagged.map_addr(|p| p & !1), pointer);
    /// ```
    #[inline]
    #[cfg(target_has_atomic = "ptr")]
    #[stable(feature = "strict_provenance_atomic_ptr", since = "1.91.0")]
    #[cfg_attr(miri, track_caller)] // 即使不 panic,这也有助于 Miri 的回溯信息
    #[rustc_should_not_be_called_on_const_items]
    pub fn fetch_or(&self, val: usize, order: Ordering) -> *mut T {
        // SAFETY: 数据竞争由原子 intrinsic 防止。
        unsafe { atomic_or(self.p.get(), val, order).cast() }
    }

    /// 对当前指针的地址与参数 `val` 做按位“与”(and)运算,
    /// 并存入一个指针 —— 该指针带有当前指针的 provenance(出处)和运算得到的新地址。
    ///
    /// 这等价于用 [`map_addr`] 原子地执行 `ptr = ptr.map_addr(|a| a & val)`。
    /// 可用于带标记位的指针(tagged pointer)方案中原子地清除标记位。
    ///
    /// **告诫**:本操作返回的是操作前的旧值。若想在不丢失 provenance 的前提下计算出已存入的值,
    /// 可以使用 [`map_addr`],例如:`a.fetch_and(val).map_addr(|a| a & val)`。
    ///
    /// `fetch_and` 接受一个 [`Ordering`] 参数来描述本操作的内存顺序。
    /// 所有顺序模式都允许。注意:用 [`Acquire`] 会让本操作的 store 部分退化为 [`Relaxed`],
    /// 用 [`Release`] 会让 load 部分退化为 [`Relaxed`];只有 [`AcqRel`] 或 [`SeqCst`] 才同时具备两端语义。
    ///
    /// **注意**:本方法只在支持 [`AtomicPtr`] 原子操作的平台上可用。
    ///
    /// 本 API 及其所声称的语义是 Strict Provenance 实验的一部分,
    /// 详见 [`ptr` 的模块文档][crate::ptr]。
    ///
    /// [`map_addr`]: pointer::map_addr
    ///
    /// # 示例
    ///
    /// ```
    /// use core::sync::atomic::{AtomicPtr, Ordering};
    ///
    /// let pointer = &mut 3i64 as *mut i64;
    /// // 一个带标记的指针
    /// let atom = AtomicPtr::<i64>::new(pointer.map_addr(|a| a | 1));
    /// assert_eq!(atom.fetch_or(1, Ordering::Relaxed).addr() & 1, 1);
    /// // 去除标记,并取出之前被打了标记的指针。
    /// let untagged = atom.fetch_and(!1, Ordering::Relaxed)
    ///     .map_addr(|a| a & !1);
    /// assert_eq!(untagged, pointer);
    /// ```
    #[inline]
    #[cfg(target_has_atomic = "ptr")]
    #[stable(feature = "strict_provenance_atomic_ptr", since = "1.91.0")]
    #[cfg_attr(miri, track_caller)] // 即使不 panic,这也有助于 Miri 的回溯信息
    #[rustc_should_not_be_called_on_const_items]
    pub fn fetch_and(&self, val: usize, order: Ordering) -> *mut T {
        // SAFETY: 数据竞争由原子 intrinsic 防止。
        unsafe { atomic_and(self.p.get(), val, order).cast() }
    }

    /// 对当前指针的地址与参数 `val` 做按位“异或”(xor)运算,
    /// 并存入一个指针 —— 该指针带有当前指针的 provenance(出处)和运算得到的新地址。
    ///
    /// 这等价于用 [`map_addr`] 原子地执行 `ptr = ptr.map_addr(|a| a ^ val)`。
    /// 可用于带标记位的指针(tagged pointer)方案中原子地翻转标记位。
    ///
    /// **告诫**:本操作返回的是操作前的旧值。若想在不丢失 provenance 的前提下计算出已存入的值,
    /// 可以使用 [`map_addr`],例如:`a.fetch_xor(val).map_addr(|a| a ^ val)`。
    ///
    /// `fetch_xor` 接受一个 [`Ordering`] 参数来描述本操作的内存顺序。
    /// 所有顺序模式都允许。注意:用 [`Acquire`] 会让本操作的 store 部分退化为 [`Relaxed`],
    /// 用 [`Release`] 会让 load 部分退化为 [`Relaxed`];只有 [`AcqRel`] 或 [`SeqCst`] 才同时具备两端语义。
    ///
    /// **注意**:本方法只在支持 [`AtomicPtr`] 原子操作的平台上可用。
    ///
    /// 本 API 及其所声称的语义是 Strict Provenance 实验的一部分,
    /// 详见 [`ptr` 的模块文档][crate::ptr]。
    ///
    /// [`map_addr`]: pointer::map_addr
    ///
    /// # 示例
    ///
    /// ```
    /// use core::sync::atomic::{AtomicPtr, Ordering};
    ///
    /// let pointer = &mut 3i64 as *mut i64;
    /// let atom = AtomicPtr::<i64>::new(pointer);
    ///
    /// // 翻转指针上的一个标记位。
    /// atom.fetch_xor(1, Ordering::Relaxed);
    /// assert_eq!(atom.load(Ordering::Relaxed).addr() & 1, 1);
    /// ```
    #[inline]
    #[cfg(target_has_atomic = "ptr")]
    #[stable(feature = "strict_provenance_atomic_ptr", since = "1.91.0")]
    #[cfg_attr(miri, track_caller)] // 即使不 panic,这也有助于 Miri 的回溯信息
    #[rustc_should_not_be_called_on_const_items]
    pub fn fetch_xor(&self, val: usize, order: Ordering) -> *mut T {
        // SAFETY: 数据竞争由原子 intrinsic 防止。
        unsafe { atomic_xor(self.p.get(), val, order).cast() }
    }

    /// 返回指向底层指针的裸可变指针。
    ///
    /// 对返回的这个指针做非原子的读写可能造成数据竞争。
    /// 本方法主要用于 FFI 场景 —— 那里函数签名往往用 `*mut *mut T` 而非 `&AtomicPtr<T>`。
    ///
    /// 从对该原子的共享引用返回 `*mut` 指针是安全的,因为原子类型基于内部可变性工作:
    /// 对原子的所有修改都是通过共享引用进行的,而只要它们使用原子操作就能安全地这样做。
    /// 对返回的裸指针的任何使用都需要 `unsafe` 块,并且仍然必须满足 [memory model] 的要求。
    ///
    /// # 示例
    ///
    /// ```ignore (extern-declaration)
    /// use std::sync::atomic::AtomicPtr;
    ///
    /// extern "C" {
    ///     fn my_atomic_op(arg: *mut *mut u32);
    /// }
    ///
    /// let mut value = 17;
    /// let atomic = AtomicPtr::new(&mut value);
    ///
    /// // SAFETY: 只要 `my_atomic_op` 是原子的,这就是安全的。
    /// unsafe {
    ///     my_atomic_op(atomic.as_ptr());
    /// }
    /// ```
    ///
    /// [memory model]: self#memory-model-for-atomic-accesses
    #[inline]
    #[stable(feature = "atomic_as_ptr", since = "1.70.0")]
    #[rustc_const_stable(feature = "atomic_as_ptr", since = "1.70.0")]
    #[rustc_never_returns_null_ptr]
    pub const fn as_ptr(&self) -> *mut *mut T {
        self.p.get()
    }
}

#[cfg(target_has_atomic_load_store = "8")]
#[stable(feature = "atomic_bool_from", since = "1.24.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl const From<bool> for AtomicBool {
    /// 把一个 `bool` 转换为 `AtomicBool`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::sync::atomic::AtomicBool;
    /// let atomic_bool = AtomicBool::from(true);
    /// assert_eq!(format!("{atomic_bool:?}"), "true")
    /// ```
    #[inline]
    fn from(b: bool) -> Self {
        Self::new(b)
    }
}

#[cfg(target_has_atomic_load_store = "ptr")]
#[stable(feature = "atomic_from", since = "1.23.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl<T> const From<*mut T> for AtomicPtr<T> {
    /// 把一个 `*mut T` 转换为 `AtomicPtr<T>`。
    #[inline]
    fn from(p: *mut T) -> Self {
        Self::new(p)
    }
}

#[allow(unused_macros)] // 在某些架构上,这个宏最终是用不到的。
macro_rules! if_8_bit {
    (u8, $( yes = [$($yes:tt)*], )? $( no = [$($no:tt)*], )? ) => { concat!("", $($($yes)*)?) };
    (i8, $( yes = [$($yes:tt)*], )? $( no = [$($no:tt)*], )? ) => { concat!("", $($($yes)*)?) };
    ($_:ident, $( yes = [$($yes:tt)*], )? $( no = [$($no:tt)*], )? ) => { concat!("", $($($no)*)?) };
}

#[cfg(target_has_atomic_load_store)]
macro_rules! atomic_int {
    ($cfg_cas:meta,
     $cfg_align:meta,
     $stable:meta,
     $stable_cxchg:meta,
     $stable_debug:meta,
     $stable_access:meta,
     $stable_from:meta,
     $stable_nand:meta,
     $const_stable_new:meta,
     $const_stable_into_inner:meta,
     $diagnostic_item:meta,
     $s_int_type:literal,
     $extra_feature:expr,
     $min_fn:ident, $max_fn:ident,
     $align:expr,
     $int_type:ident $atomic_type:ident) => {
        /// 一种可以在线程之间安全共享的整数类型。
        ///
        /// 它与底层整数类型 [`
        #[doc = $s_int_type]
        /// `] 拥有相同的
        #[doc = if_8_bit!(
            $int_type,
            yes = ["大小、对齐和位有效性(bit validity)"],
            no = ["大小和位有效性(bit validity)"],
        )]
        /// 。
        #[doc = if_8_bit! {
            $int_type,
            no = [
                "不过,本类型的对齐始终等于其大小,",
                "即使在 [`", $s_int_type, "`] 的对齐更小的目标平台上也是如此。"
            ],
        }]
        ///
        /// 关于原子类型与非原子类型之间的差异,以及本类型可移植性方面的更多内容,
        /// 请参阅 [module-level documentation]。
        ///
        /// **注意:** 本类型只在支持对 [`
        #[doc = $s_int_type]
        /// `] 进行原子 load 与 store 的平台上可用。
        ///
        /// [module-level documentation]: crate::sync::atomic
        #[$stable]
        #[$diagnostic_item]
        #[repr(C, align($align))]
        pub struct $atomic_type {
            v: UnsafeCell<$int_type>,
        }

        #[$stable]
        impl Default for $atomic_type {
            #[inline]
            fn default() -> Self {
                Self::new(Default::default())
            }
        }

        #[$stable_from]
        #[rustc_const_unstable(feature = "const_convert", issue = "143773")]
        impl const From<$int_type> for $atomic_type {
            #[doc = concat!("把一个 `", stringify!($int_type), "` 转换为 `", stringify!($atomic_type), "`。")]
            #[inline]
            fn from(v: $int_type) -> Self { Self::new(v) }
        }

        #[$stable_debug]
        impl fmt::Debug for $atomic_type {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                fmt::Debug::fmt(&self.load(Ordering::Relaxed), f)
            }
        }

        // Send 是隐式实现的。
        #[$stable]
        unsafe impl Sync for $atomic_type {}

        impl $atomic_type {
            /// 创建一个新的原子整数。
            ///
            /// # 示例
            ///
            /// ```
            #[doc = concat!($extra_feature, "use std::sync::atomic::", stringify!($atomic_type), ";")]
            ///
            #[doc = concat!("let atomic_forty_two = ", stringify!($atomic_type), "::new(42);")]
            /// ```
            #[inline]
            #[$stable]
            #[$const_stable_new]
            #[must_use]
            pub const fn new(v: $int_type) -> Self {
                Self {v: UnsafeCell::new(v)}
            }

            /// 从一个指针创建一个指向原子整数的新引用。
            ///
            /// # 示例
            ///
            /// ```
            #[doc = concat!($extra_feature, "use std::sync::atomic::{self, ", stringify!($atomic_type), "};")]
            ///
            /// // 取得指向一块已分配值的指针
            #[doc = concat!("let ptr: *mut ", stringify!($int_type), " = Box::into_raw(Box::new(0));")]
            ///
            #[doc = concat!("assert!(ptr.cast::<", stringify!($atomic_type), ">().is_aligned());")]
            ///
            /// {
            ///     // 为这块已分配的值创建一个原子视图
            // SAFETY: 这是一条文档注释,tidy,它伤害不到你(同时也由 `ptr` 的构造方式与上面的断言所保证)
            #[doc = concat!("    let atomic = unsafe {", stringify!($atomic_type), "::from_ptr(ptr) };")]
            ///
            ///     // 用 `atomic` 做原子操作,也可以把它分享给其他线程
            ///     atomic.store(1, atomic::Ordering::Relaxed);
            /// }
            ///
            /// // 此时非原子地访问 `ptr` 背后的值是可以的,
            /// // 因为指向该原子的引用已在上面的代码块中结束了生命周期
            /// assert_eq!(unsafe { *ptr }, 1);
            ///
            /// // 释放该值
            /// unsafe { drop(Box::from_raw(ptr)) }
            /// ```
            ///
            /// # 安全性(Safety)
            ///
            /// * `ptr` 必须按
            #[doc = concat!("  `align_of::<", stringify!($atomic_type), ">()`")]
            /// 对齐
            #[doc = if_8_bit!{
                $int_type,
                yes = [
                    "(注意:这总是成立的,因为 `align_of::<",
                    stringify!($atomic_type), ">() == 1`)。"
                ],
                no = [
                    "(注意:在某些平台上这可能比 `align_of::<",
                    stringify!($int_type), ">()` 更大)。"
                ],
            }]
            /// * `ptr` 在整个生命周期 `'a` 内必须对读和写都 [valid]。
            /// * 你必须遵守 [Memory model for atomic accesses]。特别地,不允许在没有同步的情况下
            ///   混用冲突的原子访问与非原子访问,也不允许混用不同大小的原子访问。
            ///
            /// [valid]: crate::ptr#safety
            /// [Memory model for atomic accesses]: self#memory-model-for-atomic-accesses
            #[inline]
            #[stable(feature = "atomic_from_ptr", since = "1.75.0")]
            #[rustc_const_stable(feature = "const_atomic_from_ptr", since = "1.84.0")]
            pub const unsafe fn from_ptr<'a>(ptr: *mut $int_type) -> &'a $atomic_type {
                // SAFETY: 由调用方保证。
                unsafe { &*ptr.cast() }
            }


            /// 返回指向底层整数的可变引用。
            ///
            /// 这是安全的,因为可变引用保证了没有其他线程在并发访问这块原子数据。
            ///
            /// # 示例
            ///
            /// ```
            #[doc = concat!($extra_feature, "use std::sync::atomic::{", stringify!($atomic_type), ", Ordering};")]
            ///
            #[doc = concat!("let mut some_var = ", stringify!($atomic_type), "::new(10);")]
            /// assert_eq!(*some_var.get_mut(), 10);
            /// *some_var.get_mut() = 5;
            /// assert_eq!(some_var.load(Ordering::SeqCst), 5);
            /// ```
            #[inline]
            #[$stable_access]
            pub fn get_mut(&mut self) -> &mut $int_type {
                self.v.get_mut()
            }

            #[doc = concat!("取得对一个 `&mut ", stringify!($int_type), "` 的原子访问。")]
            ///
            #[doc = if_8_bit! {
                $int_type,
                no = [
                    "**注意:** 本函数只在 `",
                    stringify!($atomic_type), "` 与 `", stringify!($int_type), "` 对齐相同的目标平台上可用。"
                ],
            }]
            ///
            /// # 示例
            ///
            /// ```
            /// #![feature(atomic_from_mut)]
            #[doc = concat!($extra_feature, "use std::sync::atomic::{", stringify!($atomic_type), ", Ordering};")]
            ///
            /// let mut some_int = 123;
            #[doc = concat!("let a = ", stringify!($atomic_type), "::from_mut(&mut some_int);")]
            /// a.store(100, Ordering::Relaxed);
            /// assert_eq!(some_int, 100);
            /// ```
            ///
            #[inline]
            #[$cfg_align]
            #[unstable(feature = "atomic_from_mut", issue = "76314")]
            pub fn from_mut(v: &mut $int_type) -> &mut Self {
                let [] = [(); align_of::<Self>() - align_of::<$int_type>()];
                // SAFETY:
                //  - 可变引用保证了独占所有权。
                //  - `$int_type` 与 `Self` 的对齐相同,这由 $cfg_align 保证并在上面验证。
                unsafe { &mut *(v as *mut $int_type as *mut Self) }
            }

            #[doc = concat!("取得对一个 `&mut [", stringify!($atomic_type), "]` 切片的非原子访问")]
            ///
            /// 这是安全的,因为可变引用保证了没有其他线程在并发访问这块原子数据。
            ///
            /// # 示例
            ///
            /// ```ignore-wasm
            /// #![feature(atomic_from_mut)]
            #[doc = concat!($extra_feature, "use std::sync::atomic::{", stringify!($atomic_type), ", Ordering};")]
            ///
            #[doc = concat!("let mut some_ints = [const { ", stringify!($atomic_type), "::new(0) }; 10];")]
            ///
            #[doc = concat!("let view: &mut [", stringify!($int_type), "] = ", stringify!($atomic_type), "::get_mut_slice(&mut some_ints);")]
            /// assert_eq!(view, [0; 10]);
            /// view
            ///     .iter_mut()
            ///     .enumerate()
            ///     .for_each(|(idx, int)| *int = idx as _);
            ///
            /// std::thread::scope(|s| {
            ///     some_ints
            ///         .iter()
            ///         .enumerate()
            ///         .for_each(|(idx, int)| {
            ///             s.spawn(move || assert_eq!(int.load(Ordering::Relaxed), idx as _));
            ///         })
            /// });
            /// ```
            #[inline]
            #[unstable(feature = "atomic_from_mut", issue = "76314")]
            pub fn get_mut_slice(this: &mut [Self]) -> &mut [$int_type] {
                // SAFETY: 可变引用保证了独占所有权。
                unsafe { &mut *(this as *mut [Self] as *mut [$int_type]) }
            }

            #[doc = concat!("取得对一个 `&mut [", stringify!($int_type), "]` 切片的原子访问。")]
            ///
            #[doc = if_8_bit! {
                $int_type,
                no = [
                    "**注意:** 本函数只在 `",
                    stringify!($atomic_type), "` 与 `", stringify!($int_type), "` 对齐相同的目标平台上可用。"
                ],
            }]
            ///
            /// # 示例
            ///
            /// ```ignore-wasm
            /// #![feature(atomic_from_mut)]
            #[doc = concat!($extra_feature, "use std::sync::atomic::{", stringify!($atomic_type), ", Ordering};")]
            ///
            /// let mut some_ints = [0; 10];
            #[doc = concat!("let a = &*", stringify!($atomic_type), "::from_mut_slice(&mut some_ints);")]
            /// std::thread::scope(|s| {
            ///     for i in 0..a.len() {
            ///         s.spawn(move || a[i].store(i as _, Ordering::Relaxed));
            ///     }
            /// });
            /// for (i, n) in some_ints.into_iter().enumerate() {
            ///     assert_eq!(i, n as usize);
            /// }
            /// ```
            #[inline]
            #[$cfg_align]
            #[unstable(feature = "atomic_from_mut", issue = "76314")]
            pub fn from_mut_slice(v: &mut [$int_type]) -> &mut [Self] {
                let [] = [(); align_of::<Self>() - align_of::<$int_type>()];
                // SAFETY:
                //  - 可变引用保证了独占所有权。
                //  - `$int_type` 与 `Self` 的对齐相同,这由 $cfg_align 保证并在上面验证。
                unsafe { &mut *(v as *mut [$int_type] as *mut [Self]) }
            }

            /// 消耗这个原子,返回它所包含的值。
            ///
            /// 这是安全的,因为按值传入 `self` 保证了没有其他线程在并发访问这块原子数据。
            ///
            /// # 示例
            ///
            /// ```
            #[doc = concat!($extra_feature, "use std::sync::atomic::", stringify!($atomic_type), ";")]
            ///
            #[doc = concat!("let some_var = ", stringify!($atomic_type), "::new(5);")]
            /// assert_eq!(some_var.into_inner(), 5);
            /// ```
            #[inline]
            #[$stable_access]
            #[$const_stable_into_inner]
            pub const fn into_inner(self) -> $int_type {
                self.v.into_inner()
            }

            /// 从原子整数中 load 出一个值。
            ///
            /// `load` 接受一个 [`Ordering`] 参数来描述本操作的内存顺序。
            /// 可用的取值有 [`SeqCst`]、[`Acquire`] 和 [`Relaxed`]。
            ///
            /// # Panics
            ///
            /// 当 `order` 为 [`Release`] 或 [`AcqRel`] 时会 panic。
            ///
            /// # 示例
            ///
            /// ```
            #[doc = concat!($extra_feature, "use std::sync::atomic::{", stringify!($atomic_type), ", Ordering};")]
            ///
            #[doc = concat!("let some_var = ", stringify!($atomic_type), "::new(5);")]
            ///
            /// assert_eq!(some_var.load(Ordering::Relaxed), 5);
            /// ```
            #[inline]
            #[$stable]
            #[cfg_attr(miri, track_caller)] // 即使不 panic,这也有助于 Miri 的回溯信息
            pub fn load(&self, order: Ordering) -> $int_type {
                // SAFETY: 数据竞争由原子 intrinsic 防止。
                unsafe { atomic_load(self.v.get(), order) }
            }

            /// 把一个值 store 进原子整数。
            ///
            /// `store` 接受一个 [`Ordering`] 参数来描述本操作的内存顺序。
            /// 可用的取值有 [`SeqCst`]、[`Release`] 和 [`Relaxed`]。
            ///
            /// # Panics
            ///
            /// 当 `order` 为 [`Acquire`] 或 [`AcqRel`] 时会 panic。
            ///
            /// # 示例
            ///
            /// ```
            #[doc = concat!($extra_feature, "use std::sync::atomic::{", stringify!($atomic_type), ", Ordering};")]
            ///
            #[doc = concat!("let some_var = ", stringify!($atomic_type), "::new(5);")]
            ///
            /// some_var.store(10, Ordering::Relaxed);
            /// assert_eq!(some_var.load(Ordering::Relaxed), 10);
            /// ```
            #[inline]
            #[$stable]
            #[cfg_attr(miri, track_caller)] // 即使不 panic,这也有助于 Miri 的回溯信息
            #[rustc_should_not_be_called_on_const_items]
            pub fn store(&self, val: $int_type, order: Ordering) {
                // SAFETY: 数据竞争由原子 intrinsic 防止。
                unsafe { atomic_store(self.v.get(), val, order); }
            }

            /// 把一个值 store 进原子整数,并返回操作前的旧值。
            ///
            /// `swap` 接受一个 [`Ordering`] 参数来描述本操作的内存顺序。
            /// 所有顺序模式都允许。注意:用 [`Acquire`] 会让本操作的 store 部分退化为 [`Relaxed`],
            /// 用 [`Release`] 会让 load 部分退化为 [`Relaxed`];只有 [`AcqRel`] 或 [`SeqCst`] 才同时具备两端语义。
            ///
            /// **注意**:本方法只在支持对
            #[doc = concat!("[`", $s_int_type, "`]")]
            /// 进行原子操作的平台上可用。
            ///
            /// # 示例
            ///
            /// ```
            #[doc = concat!($extra_feature, "use std::sync::atomic::{", stringify!($atomic_type), ", Ordering};")]
            ///
            #[doc = concat!("let some_var = ", stringify!($atomic_type), "::new(5);")]
            ///
            /// assert_eq!(some_var.swap(10, Ordering::Relaxed), 5);
            /// ```
            #[inline]
            #[$stable]
            #[$cfg_cas]
            #[cfg_attr(miri, track_caller)] // 即使不 panic,这也有助于 Miri 的回溯信息
            #[rustc_should_not_be_called_on_const_items]
            pub fn swap(&self, val: $int_type, order: Ordering) -> $int_type {
                // SAFETY: 数据竞争由原子 intrinsic 防止。
                unsafe { atomic_swap(self.v.get(), val, order) }
            }

            /// 当且仅当当前值与 `current` 相同时,把 `new` 写入原子整数。
            ///
            /// 返回值始终是操作前的旧值。如果它等于 `current`,说明值已被更新。
            ///
            /// `compare_and_swap` 也接受一个 [`Ordering`] 参数来描述本操作的内存顺序。
            /// 注意:即便使用 [`AcqRel`],操作仍可能失败,此时它只执行一次 `Acquire` load,而不具备 `Release` 语义。
            /// 用 [`Acquire`] 会让本操作(若真的发生)的 store 部分退化为 [`Relaxed`],
            /// 用 [`Release`] 会让 load 部分退化为 [`Relaxed`]。
            ///
            /// **注意**:本方法只在支持对
            #[doc = concat!("[`", $s_int_type, "`]")]
            /// 进行原子操作的平台上可用。
            ///
            /// # 迁移到 `compare_exchange` 和 `compare_exchange_weak`
            ///
            /// `compare_and_swap` 等价于按下表映射内存顺序的 `compare_exchange`:
            ///
            /// 原顺序   | Success | Failure
            /// -------- | ------- | -------
            /// Relaxed  | Relaxed | Relaxed
            /// Acquire  | Acquire | Acquire
            /// Release  | Release | Relaxed
            /// AcqRel   | AcqRel  | Acquire
            /// SeqCst   | SeqCst  | SeqCst
            ///
            /// `compare_and_swap` 与 `compare_exchange` 的返回类型也不同。你可以用
            /// `compare_exchange(...).unwrap_or_else(|x| x)` 来恢复 `compare_and_swap` 的行为,
            /// 但大多数情况下,更地道的做法是检查返回值是 `Ok` 还是 `Err`,
            /// 而不是根据读到的值去推断成功还是失败。
            ///
            /// 迁移期间,也可以考虑改用 `compare_exchange_weak` 是否更合适。
            /// `compare_exchange_weak` 允许在比较成功时仍偶发失败,这能让编译器在“比较并交换”被用于
            /// 循环中时生成更优的汇编代码。
            ///
            /// # 示例
            ///
            /// ```
            #[doc = concat!($extra_feature, "use std::sync::atomic::{", stringify!($atomic_type), ", Ordering};")]
            ///
            #[doc = concat!("let some_var = ", stringify!($atomic_type), "::new(5);")]
            ///
            /// assert_eq!(some_var.compare_and_swap(5, 10, Ordering::Relaxed), 5);
            /// assert_eq!(some_var.load(Ordering::Relaxed), 10);
            ///
            /// assert_eq!(some_var.compare_and_swap(6, 12, Ordering::Relaxed), 10);
            /// assert_eq!(some_var.load(Ordering::Relaxed), 10);
            /// ```
            #[inline]
            #[$stable]
            #[deprecated(
                since = "1.50.0",
                note = "Use `compare_exchange` or `compare_exchange_weak` instead")
            ]
            #[$cfg_cas]
            #[cfg_attr(miri, track_caller)] // 即使不 panic,这也有助于 Miri 的回溯信息
            #[rustc_should_not_be_called_on_const_items]
            pub fn compare_and_swap(&self,
                                    current: $int_type,
                                    new: $int_type,
                                    order: Ordering) -> $int_type {
                match self.compare_exchange(current,
                                            new,
                                            order,
                                            strongest_failure_ordering(order)) {
                    Ok(x) => x,
                    Err(x) => x,
                }
            }

            /// 当且仅当当前值与 `current` 相同时,把 `new` 写入原子整数。
            ///
            /// 返回值是一个 `Result`,指示新值是否被写入,并携带操作前的旧值。
            /// 成功时该值保证等于 `current`。
            ///
            /// `compare_exchange` 接受两个 [`Ordering`] 参数来描述本操作的内存顺序。
            /// `success` 描述比较成功时所执行的“读-改-写”操作所需的顺序;
            /// `failure` 描述比较失败时所执行的 load 操作所需的顺序。
            /// 把 `success` 设为 [`Acquire`] 会让本操作的 store 部分退化为 [`Relaxed`],
            /// 设为 [`Release`] 会让成功路径上的 load 部分退化为 [`Relaxed`]。
            /// `failure` 顺序只能是 [`SeqCst`]、[`Acquire`] 或 [`Relaxed`];
            /// 传入 [`Release`] 或 [`AcqRel`] 会触发 panic。
            ///
            /// **注意**:本方法只在支持对
            #[doc = concat!("[`", $s_int_type, "`]")]
            /// 进行原子操作的平台上可用。
            ///
            /// # 示例
            ///
            /// ```
            #[doc = concat!($extra_feature, "use std::sync::atomic::{", stringify!($atomic_type), ", Ordering};")]
            ///
            #[doc = concat!("let some_var = ", stringify!($atomic_type), "::new(5);")]
            ///
            /// assert_eq!(some_var.compare_exchange(5, 10,
            ///                                      Ordering::Acquire,
            ///                                      Ordering::Relaxed),
            ///            Ok(5));
            /// assert_eq!(some_var.load(Ordering::Relaxed), 10);
            ///
            /// assert_eq!(some_var.compare_exchange(6, 12,
            ///                                      Ordering::SeqCst,
            ///                                      Ordering::Acquire),
            ///            Err(10));
            /// assert_eq!(some_var.load(Ordering::Relaxed), 10);
            /// ```
            ///
            /// # 注意事项
            ///
            /// `compare_exchange` 是一个 [compare-and-swap operation],因此带有 CAS 操作惯有的缺陷。
            /// 特别地,“先 load 出一个值,再用这个旧值做一次成功的 `compare_exchange`” *并不能保证*
            /// 在这两步之间其他线程没有改动过该值!这一点在用 `compare_exchange` 里的*相等性*检查
            /// 来判断值的*同一性*时尤其重要 —— 相等并不必然意味着同一。对指针来说这尤其常见:
            /// 一个指针持有相同的地址,并不意味着该地址上还存在同一个对象!在这种场景下,
            /// `compare_exchange` 可能导致 [ABA problem](即值被改成别的再改回原值,CAS 仍判定相等而通过)。
            ///
            /// [ABA Problem]: https://en.wikipedia.org/wiki/ABA_problem
            /// [compare-and-swap operation]: https://en.wikipedia.org/wiki/Compare-and-swap
            #[inline]
            #[$stable_cxchg]
            #[$cfg_cas]
            #[cfg_attr(miri, track_caller)] // 即使不 panic,这也有助于 Miri 的回溯信息
            #[rustc_should_not_be_called_on_const_items]
            pub fn compare_exchange(&self,
                                    current: $int_type,
                                    new: $int_type,
                                    success: Ordering,
                                    failure: Ordering) -> Result<$int_type, $int_type> {
                // SAFETY: 数据竞争由原子 intrinsic 防止。
                unsafe { atomic_compare_exchange(self.v.get(), current, new, success, failure) }
            }

            /// 当且仅当当前值与 `current` 相同时,把 `new` 写入原子整数。
            ///
            #[doc = concat!("与 [`", stringify!($atomic_type), "::compare_exchange`] 不同,")]
            /// 本函数 **允许在比较成功时仍偶发失败(spurious failure)**,
            /// 这能让某些平台生成更高效的代码。返回值是一个 `Result`,
            /// 指示新值是否被写入,并携带操作前的旧值。
            ///
            /// `compare_exchange_weak` 接受两个 [`Ordering`] 参数来描述本操作的内存顺序。
            /// `success` 描述比较成功时所执行的“读-改-写”操作所需的顺序;
            /// `failure` 描述比较失败时所执行的 load 操作所需的顺序。
            /// 把 `success` 设为 [`Acquire`] 会让本操作的 store 部分退化为 [`Relaxed`],
            /// 设为 [`Release`] 会让成功路径上的 load 部分退化为 [`Relaxed`]。
            /// `failure` 顺序只能是 [`SeqCst`]、[`Acquire`] 或 [`Relaxed`];
            /// 传入 [`Release`] 或 [`AcqRel`] 会触发 panic。
            ///
            /// **注意**:本方法只在支持对
            #[doc = concat!("[`", $s_int_type, "`]")]
            /// 进行原子操作的平台上可用。
            ///
            /// # 示例
            ///
            /// ```
            #[doc = concat!($extra_feature, "use std::sync::atomic::{", stringify!($atomic_type), ", Ordering};")]
            ///
            #[doc = concat!("let val = ", stringify!($atomic_type), "::new(4);")]
            ///
            /// let mut old = val.load(Ordering::Relaxed);
            /// loop {
            ///     let new = old * 2;
            ///     match val.compare_exchange_weak(old, new, Ordering::SeqCst, Ordering::Relaxed) {
            ///         Ok(_) => break,
            ///         Err(x) => old = x,
            ///     }
            /// }
            /// ```
            ///
            /// # 注意事项
            ///
            /// `compare_exchange` 是一个 [compare-and-swap operation],因此带有 CAS 操作惯有的缺陷。
            /// 特别地,“先 load 出一个值,再用这个旧值做一次成功的 `compare_exchange`” *并不能保证*
            /// 在这两步之间其他线程没有改动过该值。这一点在用 `compare_exchange` 里的*相等性*检查
            /// 来判断值的*同一性*时尤其重要 —— 相等并不必然意味着同一。对指针来说这尤其常见:
            /// 一个指针持有相同的地址,并不意味着该地址上还存在同一个对象!在这种场景下,
            /// `compare_exchange` 可能导致 [ABA problem](即值被改成别的再改回原值,CAS 仍判定相等而通过)。
            ///
            /// [ABA Problem]: https://en.wikipedia.org/wiki/ABA_problem
            /// [compare-and-swap operation]: https://en.wikipedia.org/wiki/Compare-and-swap
            #[inline]
            #[$stable_cxchg]
            #[$cfg_cas]
            #[cfg_attr(miri, track_caller)] // 即使不 panic,这也有助于 Miri 的回溯信息
            #[rustc_should_not_be_called_on_const_items]
            pub fn compare_exchange_weak(&self,
                                         current: $int_type,
                                         new: $int_type,
                                         success: Ordering,
                                         failure: Ordering) -> Result<$int_type, $int_type> {
                // SAFETY: 数据竞争由原子 intrinsic 防止。
                unsafe {
                    atomic_compare_exchange_weak(self.v.get(), current, new, success, failure)
                }
            }

            /// 在当前值上做加法,并返回操作前的旧值。
            ///
            /// 本操作在溢出时回绕(wrapping)。
            ///
            /// `fetch_add` 接受一个 [`Ordering`] 参数来描述本操作的内存顺序。
            /// 所有顺序模式都允许。注意:用 [`Acquire`] 会让本操作的 store 部分退化为 [`Relaxed`],
            /// 用 [`Release`] 会让 load 部分退化为 [`Relaxed`];只有 [`AcqRel`] 或 [`SeqCst`] 才同时具备两端语义。
            ///
            /// **注意**:本方法只在支持对
            #[doc = concat!("[`", $s_int_type, "`]")]
            /// 进行原子操作的平台上可用。
            ///
            /// # 示例
            ///
            /// ```
            #[doc = concat!($extra_feature, "use std::sync::atomic::{", stringify!($atomic_type), ", Ordering};")]
            ///
            #[doc = concat!("let foo = ", stringify!($atomic_type), "::new(0);")]
            /// assert_eq!(foo.fetch_add(10, Ordering::SeqCst), 0);
            /// assert_eq!(foo.load(Ordering::SeqCst), 10);
            /// ```
            #[inline]
            #[$stable]
            #[$cfg_cas]
            #[cfg_attr(miri, track_caller)] // 即使不 panic,这也有助于 Miri 的回溯信息
            #[rustc_should_not_be_called_on_const_items]
            pub fn fetch_add(&self, val: $int_type, order: Ordering) -> $int_type {
                // SAFETY: 数据竞争由原子 intrinsic 防止。
                unsafe { atomic_add(self.v.get(), val, order) }
            }

            /// 在当前值上做减法,并返回操作前的旧值。
            ///
            /// 本操作在溢出时回绕(wrapping)。
            ///
            /// `fetch_sub` 接受一个 [`Ordering`] 参数来描述本操作的内存顺序。
            /// 所有顺序模式都允许。注意:用 [`Acquire`] 会让本操作的 store 部分退化为 [`Relaxed`],
            /// 用 [`Release`] 会让 load 部分退化为 [`Relaxed`];只有 [`AcqRel`] 或 [`SeqCst`] 才同时具备两端语义。
            ///
            /// **注意**:本方法只在支持对
            #[doc = concat!("[`", $s_int_type, "`]")]
            /// 进行原子操作的平台上可用。
            ///
            /// # 示例
            ///
            /// ```
            #[doc = concat!($extra_feature, "use std::sync::atomic::{", stringify!($atomic_type), ", Ordering};")]
            ///
            #[doc = concat!("let foo = ", stringify!($atomic_type), "::new(20);")]
            /// assert_eq!(foo.fetch_sub(10, Ordering::SeqCst), 20);
            /// assert_eq!(foo.load(Ordering::SeqCst), 10);
            /// ```
            #[inline]
            #[$stable]
            #[$cfg_cas]
            #[cfg_attr(miri, track_caller)] // 即使不 panic,这也有助于 Miri 的回溯信息
            #[rustc_should_not_be_called_on_const_items]
            pub fn fetch_sub(&self, val: $int_type, order: Ordering) -> $int_type {
                // SAFETY: 数据竞争由原子 intrinsic 防止。
                unsafe { atomic_sub(self.v.get(), val, order) }
            }

            /// 与当前值做按位“与”(and)。
            ///
            /// 对当前值与参数 `val` 执行按位“与”操作,并把结果设为新值。
            ///
            /// 返回操作前的旧值。
            ///
            /// `fetch_and` 接受一个 [`Ordering`] 参数来描述本操作的内存顺序。
            /// 所有顺序模式都允许。注意:用 [`Acquire`] 会让本操作的 store 部分退化为 [`Relaxed`],
            /// 用 [`Release`] 会让 load 部分退化为 [`Relaxed`];只有 [`AcqRel`] 或 [`SeqCst`] 才同时具备两端语义。
            ///
            /// **注意**:本方法只在支持对
            #[doc = concat!("[`", $s_int_type, "`]")]
            /// 进行原子操作的平台上可用。
            ///
            /// # 示例
            ///
            /// ```
            #[doc = concat!($extra_feature, "use std::sync::atomic::{", stringify!($atomic_type), ", Ordering};")]
            ///
            #[doc = concat!("let foo = ", stringify!($atomic_type), "::new(0b101101);")]
            /// assert_eq!(foo.fetch_and(0b110011, Ordering::SeqCst), 0b101101);
            /// assert_eq!(foo.load(Ordering::SeqCst), 0b100001);
            /// ```
            #[inline]
            #[$stable]
            #[$cfg_cas]
            #[cfg_attr(miri, track_caller)] // 即使不 panic,这也有助于 Miri 的回溯信息
            #[rustc_should_not_be_called_on_const_items]
            pub fn fetch_and(&self, val: $int_type, order: Ordering) -> $int_type {
                // SAFETY: 数据竞争由原子 intrinsic 防止。
                unsafe { atomic_and(self.v.get(), val, order) }
            }

            /// 与当前值做按位“与非”(nand)。
            ///
            /// 对当前值与参数 `val` 执行按位“与非”操作,并把结果设为新值。
            ///
            /// 返回操作前的旧值。
            ///
            /// `fetch_nand` 接受一个 [`Ordering`] 参数来描述本操作的内存顺序。
            /// 所有顺序模式都允许。注意:用 [`Acquire`] 会让本操作的 store 部分退化为 [`Relaxed`],
            /// 用 [`Release`] 会让 load 部分退化为 [`Relaxed`];只有 [`AcqRel`] 或 [`SeqCst`] 才同时具备两端语义。
            ///
            /// **注意**:本方法只在支持对
            #[doc = concat!("[`", $s_int_type, "`]")]
            /// 进行原子操作的平台上可用。
            ///
            /// # 示例
            ///
            /// ```
            #[doc = concat!($extra_feature, "use std::sync::atomic::{", stringify!($atomic_type), ", Ordering};")]
            ///
            #[doc = concat!("let foo = ", stringify!($atomic_type), "::new(0x13);")]
            /// assert_eq!(foo.fetch_nand(0x31, Ordering::SeqCst), 0x13);
            /// assert_eq!(foo.load(Ordering::SeqCst), !(0x13 & 0x31));
            /// ```
            #[inline]
            #[$stable_nand]
            #[$cfg_cas]
            #[cfg_attr(miri, track_caller)] // 即使不 panic,这也有助于 Miri 的回溯信息
            #[rustc_should_not_be_called_on_const_items]
            pub fn fetch_nand(&self, val: $int_type, order: Ordering) -> $int_type {
                // SAFETY: 数据竞争由原子 intrinsic 防止。
                unsafe { atomic_nand(self.v.get(), val, order) }
            }

            /// 与当前值做按位“或”(or)。
            ///
            /// 对当前值与参数 `val` 执行按位“或”操作,并把结果设为新值。
            ///
            /// 返回操作前的旧值。
            ///
            /// `fetch_or` 接受一个 [`Ordering`] 参数来描述本操作的内存顺序。
            /// 所有顺序模式都允许。注意:用 [`Acquire`] 会让本操作的 store 部分退化为 [`Relaxed`],
            /// 用 [`Release`] 会让 load 部分退化为 [`Relaxed`];只有 [`AcqRel`] 或 [`SeqCst`] 才同时具备两端语义。
            ///
            /// **注意**:本方法只在支持对
            #[doc = concat!("[`", $s_int_type, "`]")]
            /// 进行原子操作的平台上可用。
            ///
            /// # 示例
            ///
            /// ```
            #[doc = concat!($extra_feature, "use std::sync::atomic::{", stringify!($atomic_type), ", Ordering};")]
            ///
            #[doc = concat!("let foo = ", stringify!($atomic_type), "::new(0b101101);")]
            /// assert_eq!(foo.fetch_or(0b110011, Ordering::SeqCst), 0b101101);
            /// assert_eq!(foo.load(Ordering::SeqCst), 0b111111);
            /// ```
            #[inline]
            #[$stable]
            #[$cfg_cas]
            #[cfg_attr(miri, track_caller)] // 即使不 panic,这也有助于 Miri 的回溯信息
            #[rustc_should_not_be_called_on_const_items]
            pub fn fetch_or(&self, val: $int_type, order: Ordering) -> $int_type {
                // SAFETY: 数据竞争由原子 intrinsic 防止。
                unsafe { atomic_or(self.v.get(), val, order) }
            }

            /// 与当前值做按位“异或”(xor)。
            ///
            /// 对当前值与参数 `val` 执行按位“异或”操作,并把结果设为新值。
            ///
            /// 返回操作前的旧值。
            ///
            /// `fetch_xor` 接受一个 [`Ordering`] 参数来描述本操作的内存顺序。
            /// 所有顺序模式都允许。注意:用 [`Acquire`] 会让本操作的 store 部分退化为 [`Relaxed`],
            /// 用 [`Release`] 会让 load 部分退化为 [`Relaxed`];只有 [`AcqRel`] 或 [`SeqCst`] 才同时具备两端语义。
            ///
            /// **注意**:本方法只在支持对
            #[doc = concat!("[`", $s_int_type, "`]")]
            /// 进行原子操作的平台上可用。
            ///
            /// # 示例
            ///
            /// ```
            #[doc = concat!($extra_feature, "use std::sync::atomic::{", stringify!($atomic_type), ", Ordering};")]
            ///
            #[doc = concat!("let foo = ", stringify!($atomic_type), "::new(0b101101);")]
            /// assert_eq!(foo.fetch_xor(0b110011, Ordering::SeqCst), 0b101101);
            /// assert_eq!(foo.load(Ordering::SeqCst), 0b011110);
            /// ```
            #[inline]
            #[$stable]
            #[$cfg_cas]
            #[cfg_attr(miri, track_caller)] // 即使不 panic,这也有助于 Miri 的回溯信息
            #[rustc_should_not_be_called_on_const_items]
            pub fn fetch_xor(&self, val: $int_type, order: Ordering) -> $int_type {
                // SAFETY: 数据竞争由原子 intrinsic 防止。
                unsafe { atomic_xor(self.v.get(), val, order) }
            }

            /// 取出当前值,并对它应用一个返回 `Option<新值>` 的函数。
            /// 若该函数返回 `Some(_)`,则返回 `Ok(previous_value)`(操作前的旧值);否则返回 `Err(previous_value)`。
            ///
            /// 注意:只要该函数持续返回 `Some(_)`,当期间该值被其他线程改动时,本方法可能会
            /// 多次调用该函数;但该函数最终只会对一个已存储的值生效一次。
            ///
            /// `fetch_update` 接受两个 [`Ordering`] 参数来描述本操作的内存顺序。
            /// 第一个描述操作最终成功时所需的顺序,第二个描述 load 所需的顺序。它们分别对应
            #[doc = concat!("[`", stringify!($atomic_type), "::compare_exchange`]")]
            /// 的 success 与 failure 顺序。
            ///
            /// 把成功顺序设为 [`Acquire`] 会让本操作的 store 部分退化为 [`Relaxed`],
            /// 设为 [`Release`] 会让最终成功路径上的 load 退化为 [`Relaxed`]。
            /// (失败的)load 顺序只能是 [`SeqCst`]、[`Acquire`] 或 [`Relaxed`]。
            ///
            /// **注意**:本方法只在支持对
            #[doc = concat!("[`", $s_int_type, "`]")]
            /// 进行原子操作的平台上可用。
            ///
            /// # 注意事项
            ///
            /// 本方法并不神奇:它不是硬件直接提供的,也不像临界区或互斥锁那样工作。
            ///
            /// 它是在原子 [compare-and-swap operation] 之上实现的,因此带有 CAS 操作惯有的缺陷。
            /// 特别地,如果这个原子整数是一个索引,或者更一般地说,如果仅凭原子的 *按位取值*
            /// 本身不足以确保所需的前置条件,那就要当心 [ABA problem]。
            ///
            /// [ABA Problem]: https://en.wikipedia.org/wiki/ABA_problem
            /// [compare-and-swap operation]: https://en.wikipedia.org/wiki/Compare-and-swap
            ///
            /// # 示例
            ///
            /// ```rust
            #[doc = concat!($extra_feature, "use std::sync::atomic::{", stringify!($atomic_type), ", Ordering};")]
            ///
            #[doc = concat!("let x = ", stringify!($atomic_type), "::new(7);")]
            /// assert_eq!(x.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |_| None), Err(7));
            /// assert_eq!(x.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |x| Some(x + 1)), Ok(7));
            /// assert_eq!(x.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |x| Some(x + 1)), Ok(8));
            /// assert_eq!(x.load(Ordering::SeqCst), 9);
            /// ```
            #[inline]
            #[stable(feature = "no_more_cas", since = "1.45.0")]
            #[$cfg_cas]
            #[cfg_attr(miri, track_caller)] // 即使不 panic,这也有助于 Miri 的回溯信息
            #[rustc_should_not_be_called_on_const_items]
            pub fn fetch_update<F>(&self,
                                   set_order: Ordering,
                                   fetch_order: Ordering,
                                   mut f: F) -> Result<$int_type, $int_type>
            where F: FnMut($int_type) -> Option<$int_type> {
                let mut prev = self.load(fetch_order);
                while let Some(next) = f(prev) {
                    match self.compare_exchange_weak(prev, next, set_order, fetch_order) {
                        x @ Ok(_) => return x,
                        Err(next_prev) => prev = next_prev
                    }
                }
                Err(prev)
            }

            /// 取出当前值,并对它应用一个返回 `Option<新值>` 的函数。
            /// 若该函数返回 `Some(_)`,则返回 `Ok(previous_value)`(操作前的旧值);否则返回 `Err(previous_value)`。
            ///
            #[doc = concat!("另见:[`update`](`", stringify!($atomic_type), "::update`)。")]
            ///
            /// 注意:只要该函数持续返回 `Some(_)`,当期间该值被其他线程改动时,本方法可能会
            /// 多次调用该函数;但该函数最终只会对一个已存储的值生效一次。
            ///
            /// `try_update` 接受两个 [`Ordering`] 参数来描述本操作的内存顺序。
            /// 第一个描述操作最终成功时所需的顺序,第二个描述 load 所需的顺序。它们分别对应
            #[doc = concat!("[`", stringify!($atomic_type), "::compare_exchange`]")]
            /// 的 success 与 failure 顺序。
            ///
            /// 把成功顺序设为 [`Acquire`] 会让本操作的 store 部分退化为 [`Relaxed`],
            /// 设为 [`Release`] 会让最终成功路径上的 load 退化为 [`Relaxed`]。
            /// (失败的)load 顺序只能是 [`SeqCst`]、[`Acquire`] 或 [`Relaxed`]。
            ///
            /// **注意**:本方法只在支持对
            #[doc = concat!("[`", $s_int_type, "`]")]
            /// 进行原子操作的平台上可用。
            ///
            /// # 注意事项
            ///
            /// 本方法并不神奇:它不是硬件直接提供的,也不像临界区或互斥锁那样工作。
            ///
            /// 它是在原子 [compare-and-swap operation] 之上实现的,因此带有 CAS 操作惯有的缺陷。
            /// 特别地,如果这个原子整数是一个索引,或者更一般地说,如果仅凭原子的 *按位取值*
            /// 本身不足以确保所需的前置条件,那就要当心 [ABA problem]。
            ///
            /// [ABA Problem]: https://en.wikipedia.org/wiki/ABA_problem
            /// [compare-and-swap operation]: https://en.wikipedia.org/wiki/Compare-and-swap
            ///
            /// # 示例
            ///
            /// ```rust
            /// #![feature(atomic_try_update)]
            #[doc = concat!($extra_feature, "use std::sync::atomic::{", stringify!($atomic_type), ", Ordering};")]
            ///
            #[doc = concat!("let x = ", stringify!($atomic_type), "::new(7);")]
            /// assert_eq!(x.try_update(Ordering::SeqCst, Ordering::SeqCst, |_| None), Err(7));
            /// assert_eq!(x.try_update(Ordering::SeqCst, Ordering::SeqCst, |x| Some(x + 1)), Ok(7));
            /// assert_eq!(x.try_update(Ordering::SeqCst, Ordering::SeqCst, |x| Some(x + 1)), Ok(8));
            /// assert_eq!(x.load(Ordering::SeqCst), 9);
            /// ```
            #[inline]
            #[unstable(feature = "atomic_try_update", issue = "135894")]
            #[$cfg_cas]
            #[cfg_attr(miri, track_caller)] // 即使不 panic,这也有助于 Miri 的回溯信息
            #[rustc_should_not_be_called_on_const_items]
            pub fn try_update(
                &self,
                set_order: Ordering,
                fetch_order: Ordering,
                f: impl FnMut($int_type) -> Option<$int_type>,
            ) -> Result<$int_type, $int_type> {
                // FIXME(atomic_try_update): 目前这是 `fetch_update` 的一个 unstable 别名;
                //      稳定化时,应把 `fetch_update` 改成 `try_update` 的 deprecated 别名。
                self.fetch_update(set_order, fetch_order, f)
            }

            /// 取出当前值,对它应用一个返回新值的函数。新值被存入,旧值被返回。
            ///
            #[doc = concat!("另见:[`try_update`](`", stringify!($atomic_type), "::try_update`)。")]
            ///
            /// 注意:当期间该值被其他线程改动时,本方法可能会多次调用该函数;
            /// 但该函数最终只会对一个已存储的值生效一次。
            ///
            /// `update` 接受两个 [`Ordering`] 参数来描述本操作的内存顺序。
            /// 第一个描述操作最终成功时所需的顺序,第二个描述 load 所需的顺序。它们分别对应
            #[doc = concat!("[`", stringify!($atomic_type), "::compare_exchange`]")]
            /// 的 success 与 failure 顺序。
            ///
            /// 把成功顺序设为 [`Acquire`] 会让本操作的 store 部分退化为 [`Relaxed`],
            /// 设为 [`Release`] 会让最终成功路径上的 load 退化为 [`Relaxed`]。
            /// (失败的)load 顺序只能是 [`SeqCst`]、[`Acquire`] 或 [`Relaxed`]。
            ///
            /// **注意**:本方法只在支持对
            #[doc = concat!("[`", $s_int_type, "`]")]
            /// 进行原子操作的平台上可用。
            ///
            /// # 注意事项
            ///
            /// [CAS operation]: https://en.wikipedia.org/wiki/Compare-and-swap
            /// 本方法并不神奇:它不是硬件直接提供的,也不像临界区或互斥锁那样工作。
            ///
            /// 它是在原子 [compare-and-swap operation] 之上实现的,因此带有 CAS 操作惯有的缺陷。
            /// 特别地,如果这个原子整数是一个索引,或者更一般地说,如果仅凭原子的 *按位取值*
            /// 本身不足以确保所需的前置条件,那就要当心 [ABA problem]。
            ///
            /// [ABA Problem]: https://en.wikipedia.org/wiki/ABA_problem
            /// [compare-and-swap operation]: https://en.wikipedia.org/wiki/Compare-and-swap
            ///
            /// # 示例
            ///
            /// ```rust
            /// #![feature(atomic_try_update)]
            #[doc = concat!($extra_feature, "use std::sync::atomic::{", stringify!($atomic_type), ", Ordering};")]
            ///
            #[doc = concat!("let x = ", stringify!($atomic_type), "::new(7);")]
            /// assert_eq!(x.update(Ordering::SeqCst, Ordering::SeqCst, |x| x + 1), 7);
            /// assert_eq!(x.update(Ordering::SeqCst, Ordering::SeqCst, |x| x + 1), 8);
            /// assert_eq!(x.load(Ordering::SeqCst), 9);
            /// ```
            #[inline]
            #[unstable(feature = "atomic_try_update", issue = "135894")]
            #[$cfg_cas]
            #[cfg_attr(miri, track_caller)] // 即使不 panic,这也有助于 Miri 的回溯信息
            #[rustc_should_not_be_called_on_const_items]
            pub fn update(
                &self,
                set_order: Ordering,
                fetch_order: Ordering,
                mut f: impl FnMut($int_type) -> $int_type,
            ) -> $int_type {
                let mut prev = self.load(fetch_order);
                loop {
                    match self.compare_exchange_weak(prev, f(prev), set_order, fetch_order) {
                        Ok(x) => break x,
                        Err(next_prev) => prev = next_prev,
                    }
                }
            }

            /// 与当前值取最大值。
            ///
            /// 求当前值与参数 `val` 中的较大者,并把结果设为新值。
            ///
            /// 返回操作前的旧值。
            ///
            /// `fetch_max` 接受一个 [`Ordering`] 参数来描述本操作的内存顺序。
            /// 所有顺序模式都允许。注意:用 [`Acquire`] 会让本操作的 store 部分退化为 [`Relaxed`],
            /// 用 [`Release`] 会让 load 部分退化为 [`Relaxed`];只有 [`AcqRel`] 或 [`SeqCst`] 才同时具备两端语义。
            ///
            /// **注意**:本方法只在支持对
            #[doc = concat!("[`", $s_int_type, "`]")]
            /// 进行原子操作的平台上可用。
            ///
            /// # 示例
            ///
            /// ```
            #[doc = concat!($extra_feature, "use std::sync::atomic::{", stringify!($atomic_type), ", Ordering};")]
            ///
            #[doc = concat!("let foo = ", stringify!($atomic_type), "::new(23);")]
            /// assert_eq!(foo.fetch_max(42, Ordering::SeqCst), 23);
            /// assert_eq!(foo.load(Ordering::SeqCst), 42);
            /// ```
            ///
            /// 如果你想一步就得到最大值,可以这样写:
            ///
            /// ```
            #[doc = concat!($extra_feature, "use std::sync::atomic::{", stringify!($atomic_type), ", Ordering};")]
            ///
            #[doc = concat!("let foo = ", stringify!($atomic_type), "::new(23);")]
            /// let bar = 42;
            /// let max_foo = foo.fetch_max(bar, Ordering::SeqCst).max(bar);
            /// assert!(max_foo == 42);
            /// ```
            #[inline]
            #[stable(feature = "atomic_min_max", since = "1.45.0")]
            #[$cfg_cas]
            #[cfg_attr(miri, track_caller)] // 即使不 panic,这也有助于 Miri 的回溯信息
            #[rustc_should_not_be_called_on_const_items]
            pub fn fetch_max(&self, val: $int_type, order: Ordering) -> $int_type {
                // SAFETY: 数据竞争由原子 intrinsic 防止。
                unsafe { $max_fn(self.v.get(), val, order) }
            }

            /// 与当前值取最小值。
            ///
            /// 求当前值与参数 `val` 中的较小者,并把结果设为新值。
            ///
            /// 返回操作前的旧值。
            ///
            /// `fetch_min` 接受一个 [`Ordering`] 参数来描述本操作的内存顺序。
            /// 所有顺序模式都允许。注意:用 [`Acquire`] 会让本操作的 store 部分退化为 [`Relaxed`],
            /// 用 [`Release`] 会让 load 部分退化为 [`Relaxed`];只有 [`AcqRel`] 或 [`SeqCst`] 才同时具备两端语义。
            ///
            /// **注意**:本方法只在支持对
            #[doc = concat!("[`", $s_int_type, "`]")]
            /// 进行原子操作的平台上可用。
            ///
            /// # 示例
            ///
            /// ```
            #[doc = concat!($extra_feature, "use std::sync::atomic::{", stringify!($atomic_type), ", Ordering};")]
            ///
            #[doc = concat!("let foo = ", stringify!($atomic_type), "::new(23);")]
            /// assert_eq!(foo.fetch_min(42, Ordering::Relaxed), 23);
            /// assert_eq!(foo.load(Ordering::Relaxed), 23);
            /// assert_eq!(foo.fetch_min(22, Ordering::Relaxed), 23);
            /// assert_eq!(foo.load(Ordering::Relaxed), 22);
            /// ```
            ///
            /// 如果你想一步就得到最小值,可以这样写:
            ///
            /// ```
            #[doc = concat!($extra_feature, "use std::sync::atomic::{", stringify!($atomic_type), ", Ordering};")]
            ///
            #[doc = concat!("let foo = ", stringify!($atomic_type), "::new(23);")]
            /// let bar = 12;
            /// let min_foo = foo.fetch_min(bar, Ordering::SeqCst).min(bar);
            /// assert_eq!(min_foo, 12);
            /// ```
            #[inline]
            #[stable(feature = "atomic_min_max", since = "1.45.0")]
            #[$cfg_cas]
            #[cfg_attr(miri, track_caller)] // 即使不 panic,这也有助于 Miri 的回溯信息
            #[rustc_should_not_be_called_on_const_items]
            pub fn fetch_min(&self, val: $int_type, order: Ordering) -> $int_type {
                // SAFETY: 数据竞争由原子 intrinsic 防止。
                unsafe { $min_fn(self.v.get(), val, order) }
            }

            /// 返回指向底层整数的裸可变指针。
            ///
            /// 对返回的这个整数做非原子的读写可能造成数据竞争。
            /// 本方法主要用于 FFI 场景 —— 那里函数签名往往用
            #[doc = concat!("`*mut ", stringify!($int_type), "` 而非 `&", stringify!($atomic_type), "`。")]
            ///
            /// 从对该原子的共享引用返回 `*mut` 指针是安全的,因为原子类型基于内部可变性工作:
            /// 对原子的所有修改都是通过共享引用进行的,而只要它们使用原子操作就能安全地这样做。
            /// 对返回的裸指针的任何使用都需要 `unsafe` 块,并且仍然必须满足 [memory model] 的要求。
            ///
            /// # 示例
            ///
            /// ```ignore (extern-declaration)
            /// # fn main() {
            #[doc = concat!($extra_feature, "use std::sync::atomic::", stringify!($atomic_type), ";")]
            ///
            /// extern "C" {
            #[doc = concat!("    fn my_atomic_op(arg: *mut ", stringify!($int_type), ");")]
            /// }
            ///
            #[doc = concat!("let atomic = ", stringify!($atomic_type), "::new(1);")]
            ///
            /// // SAFETY: 只要 `my_atomic_op` 是原子的,这就是安全的。
            /// unsafe {
            ///     my_atomic_op(atomic.as_ptr());
            /// }
            /// # }
            /// ```
            ///
            /// [memory model]: self#memory-model-for-atomic-accesses
            #[inline]
            #[stable(feature = "atomic_as_ptr", since = "1.70.0")]
            #[rustc_const_stable(feature = "atomic_as_ptr", since = "1.70.0")]
            #[rustc_never_returns_null_ptr]
            pub const fn as_ptr(&self) -> *mut $int_type {
                self.v.get()
            }
        }
    }
}

#[cfg(target_has_atomic_load_store = "8")]
atomic_int! {
    cfg(target_has_atomic = "8"),
    cfg(target_has_atomic_equal_alignment = "8"),
    stable(feature = "integer_atomics_stable", since = "1.34.0"),
    stable(feature = "integer_atomics_stable", since = "1.34.0"),
    stable(feature = "integer_atomics_stable", since = "1.34.0"),
    stable(feature = "integer_atomics_stable", since = "1.34.0"),
    stable(feature = "integer_atomics_stable", since = "1.34.0"),
    stable(feature = "integer_atomics_stable", since = "1.34.0"),
    rustc_const_stable(feature = "const_integer_atomics", since = "1.34.0"),
    rustc_const_stable(feature = "const_atomic_into_inner", since = "1.79.0"),
    rustc_diagnostic_item = "AtomicI8",
    "i8",
    "",
    atomic_min, atomic_max,
    1,
    i8 AtomicI8
}
#[cfg(target_has_atomic_load_store = "8")]
atomic_int! {
    cfg(target_has_atomic = "8"),
    cfg(target_has_atomic_equal_alignment = "8"),
    stable(feature = "integer_atomics_stable", since = "1.34.0"),
    stable(feature = "integer_atomics_stable", since = "1.34.0"),
    stable(feature = "integer_atomics_stable", since = "1.34.0"),
    stable(feature = "integer_atomics_stable", since = "1.34.0"),
    stable(feature = "integer_atomics_stable", since = "1.34.0"),
    stable(feature = "integer_atomics_stable", since = "1.34.0"),
    rustc_const_stable(feature = "const_integer_atomics", since = "1.34.0"),
    rustc_const_stable(feature = "const_atomic_into_inner", since = "1.79.0"),
    rustc_diagnostic_item = "AtomicU8",
    "u8",
    "",
    atomic_umin, atomic_umax,
    1,
    u8 AtomicU8
}
#[cfg(target_has_atomic_load_store = "16")]
atomic_int! {
    cfg(target_has_atomic = "16"),
    cfg(target_has_atomic_equal_alignment = "16"),
    stable(feature = "integer_atomics_stable", since = "1.34.0"),
    stable(feature = "integer_atomics_stable", since = "1.34.0"),
    stable(feature = "integer_atomics_stable", since = "1.34.0"),
    stable(feature = "integer_atomics_stable", since = "1.34.0"),
    stable(feature = "integer_atomics_stable", since = "1.34.0"),
    stable(feature = "integer_atomics_stable", since = "1.34.0"),
    rustc_const_stable(feature = "const_integer_atomics", since = "1.34.0"),
    rustc_const_stable(feature = "const_atomic_into_inner", since = "1.79.0"),
    rustc_diagnostic_item = "AtomicI16",
    "i16",
    "",
    atomic_min, atomic_max,
    2,
    i16 AtomicI16
}
#[cfg(target_has_atomic_load_store = "16")]
atomic_int! {
    cfg(target_has_atomic = "16"),
    cfg(target_has_atomic_equal_alignment = "16"),
    stable(feature = "integer_atomics_stable", since = "1.34.0"),
    stable(feature = "integer_atomics_stable", since = "1.34.0"),
    stable(feature = "integer_atomics_stable", since = "1.34.0"),
    stable(feature = "integer_atomics_stable", since = "1.34.0"),
    stable(feature = "integer_atomics_stable", since = "1.34.0"),
    stable(feature = "integer_atomics_stable", since = "1.34.0"),
    rustc_const_stable(feature = "const_integer_atomics", since = "1.34.0"),
    rustc_const_stable(feature = "const_atomic_into_inner", since = "1.79.0"),
    rustc_diagnostic_item = "AtomicU16",
    "u16",
    "",
    atomic_umin, atomic_umax,
    2,
    u16 AtomicU16
}
#[cfg(target_has_atomic_load_store = "32")]
atomic_int! {
    cfg(target_has_atomic = "32"),
    cfg(target_has_atomic_equal_alignment = "32"),
    stable(feature = "integer_atomics_stable", since = "1.34.0"),
    stable(feature = "integer_atomics_stable", since = "1.34.0"),
    stable(feature = "integer_atomics_stable", since = "1.34.0"),
    stable(feature = "integer_atomics_stable", since = "1.34.0"),
    stable(feature = "integer_atomics_stable", since = "1.34.0"),
    stable(feature = "integer_atomics_stable", since = "1.34.0"),
    rustc_const_stable(feature = "const_integer_atomics", since = "1.34.0"),
    rustc_const_stable(feature = "const_atomic_into_inner", since = "1.79.0"),
    rustc_diagnostic_item = "AtomicI32",
    "i32",
    "",
    atomic_min, atomic_max,
    4,
    i32 AtomicI32
}
#[cfg(target_has_atomic_load_store = "32")]
atomic_int! {
    cfg(target_has_atomic = "32"),
    cfg(target_has_atomic_equal_alignment = "32"),
    stable(feature = "integer_atomics_stable", since = "1.34.0"),
    stable(feature = "integer_atomics_stable", since = "1.34.0"),
    stable(feature = "integer_atomics_stable", since = "1.34.0"),
    stable(feature = "integer_atomics_stable", since = "1.34.0"),
    stable(feature = "integer_atomics_stable", since = "1.34.0"),
    stable(feature = "integer_atomics_stable", since = "1.34.0"),
    rustc_const_stable(feature = "const_integer_atomics", since = "1.34.0"),
    rustc_const_stable(feature = "const_atomic_into_inner", since = "1.79.0"),
    rustc_diagnostic_item = "AtomicU32",
    "u32",
    "",
    atomic_umin, atomic_umax,
    4,
    u32 AtomicU32
}
#[cfg(target_has_atomic_load_store = "64")]
atomic_int! {
    cfg(target_has_atomic = "64"),
    cfg(target_has_atomic_equal_alignment = "64"),
    stable(feature = "integer_atomics_stable", since = "1.34.0"),
    stable(feature = "integer_atomics_stable", since = "1.34.0"),
    stable(feature = "integer_atomics_stable", since = "1.34.0"),
    stable(feature = "integer_atomics_stable", since = "1.34.0"),
    stable(feature = "integer_atomics_stable", since = "1.34.0"),
    stable(feature = "integer_atomics_stable", since = "1.34.0"),
    rustc_const_stable(feature = "const_integer_atomics", since = "1.34.0"),
    rustc_const_stable(feature = "const_atomic_into_inner", since = "1.79.0"),
    rustc_diagnostic_item = "AtomicI64",
    "i64",
    "",
    atomic_min, atomic_max,
    8,
    i64 AtomicI64
}
#[cfg(target_has_atomic_load_store = "64")]
atomic_int! {
    cfg(target_has_atomic = "64"),
    cfg(target_has_atomic_equal_alignment = "64"),
    stable(feature = "integer_atomics_stable", since = "1.34.0"),
    stable(feature = "integer_atomics_stable", since = "1.34.0"),
    stable(feature = "integer_atomics_stable", since = "1.34.0"),
    stable(feature = "integer_atomics_stable", since = "1.34.0"),
    stable(feature = "integer_atomics_stable", since = "1.34.0"),
    stable(feature = "integer_atomics_stable", since = "1.34.0"),
    rustc_const_stable(feature = "const_integer_atomics", since = "1.34.0"),
    rustc_const_stable(feature = "const_atomic_into_inner", since = "1.79.0"),
    rustc_diagnostic_item = "AtomicU64",
    "u64",
    "",
    atomic_umin, atomic_umax,
    8,
    u64 AtomicU64
}
#[cfg(target_has_atomic_load_store = "128")]
atomic_int! {
    cfg(target_has_atomic = "128"),
    cfg(target_has_atomic_equal_alignment = "128"),
    unstable(feature = "integer_atomics", issue = "99069"),
    unstable(feature = "integer_atomics", issue = "99069"),
    unstable(feature = "integer_atomics", issue = "99069"),
    unstable(feature = "integer_atomics", issue = "99069"),
    unstable(feature = "integer_atomics", issue = "99069"),
    unstable(feature = "integer_atomics", issue = "99069"),
    rustc_const_unstable(feature = "integer_atomics", issue = "99069"),
    rustc_const_unstable(feature = "integer_atomics", issue = "99069"),
    rustc_diagnostic_item = "AtomicI128",
    "i128",
    "#![feature(integer_atomics)]\n\n",
    atomic_min, atomic_max,
    16,
    i128 AtomicI128
}
#[cfg(target_has_atomic_load_store = "128")]
atomic_int! {
    cfg(target_has_atomic = "128"),
    cfg(target_has_atomic_equal_alignment = "128"),
    unstable(feature = "integer_atomics", issue = "99069"),
    unstable(feature = "integer_atomics", issue = "99069"),
    unstable(feature = "integer_atomics", issue = "99069"),
    unstable(feature = "integer_atomics", issue = "99069"),
    unstable(feature = "integer_atomics", issue = "99069"),
    unstable(feature = "integer_atomics", issue = "99069"),
    rustc_const_unstable(feature = "integer_atomics", issue = "99069"),
    rustc_const_unstable(feature = "integer_atomics", issue = "99069"),
    rustc_diagnostic_item = "AtomicU128",
    "u128",
    "#![feature(integer_atomics)]\n\n",
    atomic_umin, atomic_umax,
    16,
    u128 AtomicU128
}

#[cfg(target_has_atomic_load_store = "ptr")]
macro_rules! atomic_int_ptr_sized {
    ( $($target_pointer_width:literal $align:literal)* ) => { $(
        #[cfg(target_pointer_width = $target_pointer_width)]
        atomic_int! {
            cfg(target_has_atomic = "ptr"),
            cfg(target_has_atomic_equal_alignment = "ptr"),
            stable(feature = "rust1", since = "1.0.0"),
            stable(feature = "extended_compare_and_swap", since = "1.10.0"),
            stable(feature = "atomic_debug", since = "1.3.0"),
            stable(feature = "atomic_access", since = "1.15.0"),
            stable(feature = "atomic_from", since = "1.23.0"),
            stable(feature = "atomic_nand", since = "1.27.0"),
            rustc_const_stable(feature = "const_ptr_sized_atomics", since = "1.24.0"),
            rustc_const_stable(feature = "const_atomic_into_inner", since = "1.79.0"),
            rustc_diagnostic_item = "AtomicIsize",
            "isize",
            "",
            atomic_min, atomic_max,
            $align,
            isize AtomicIsize
        }
        #[cfg(target_pointer_width = $target_pointer_width)]
        atomic_int! {
            cfg(target_has_atomic = "ptr"),
            cfg(target_has_atomic_equal_alignment = "ptr"),
            stable(feature = "rust1", since = "1.0.0"),
            stable(feature = "extended_compare_and_swap", since = "1.10.0"),
            stable(feature = "atomic_debug", since = "1.3.0"),
            stable(feature = "atomic_access", since = "1.15.0"),
            stable(feature = "atomic_from", since = "1.23.0"),
            stable(feature = "atomic_nand", since = "1.27.0"),
            rustc_const_stable(feature = "const_ptr_sized_atomics", since = "1.24.0"),
            rustc_const_stable(feature = "const_atomic_into_inner", since = "1.79.0"),
            rustc_diagnostic_item = "AtomicUsize",
            "usize",
            "",
            atomic_umin, atomic_umax,
            $align,
            usize AtomicUsize
        }

        /// 一个初始化为 `0` 的 [`AtomicIsize`]。
        #[cfg(target_pointer_width = $target_pointer_width)]
        #[stable(feature = "rust1", since = "1.0.0")]
        #[deprecated(
            since = "1.34.0",
            note = "the `new` function is now preferred",
            suggestion = "AtomicIsize::new(0)",
        )]
        pub const ATOMIC_ISIZE_INIT: AtomicIsize = AtomicIsize::new(0);

        /// 一个初始化为 `0` 的 [`AtomicUsize`]。
        #[cfg(target_pointer_width = $target_pointer_width)]
        #[stable(feature = "rust1", since = "1.0.0")]
        #[deprecated(
            since = "1.34.0",
            note = "the `new` function is now preferred",
            suggestion = "AtomicUsize::new(0)",
        )]
        pub const ATOMIC_USIZE_INIT: AtomicUsize = AtomicUsize::new(0);
    )* };
}

#[cfg(target_has_atomic_load_store = "ptr")]
atomic_int_ptr_sized! {
    "16" 2
    "32" 4
    "64" 8
}

#[inline]
#[cfg(target_has_atomic)]
fn strongest_failure_ordering(order: Ordering) -> Ordering {
    match order {
        Release => Relaxed,
        Relaxed => Relaxed,
        SeqCst => SeqCst,
        Acquire => Acquire,
        AcqRel => Acquire,
    }
}

#[inline]
#[cfg_attr(miri, track_caller)] // 即使不 panic,这也有助于 Miri 的回溯信息
unsafe fn atomic_store<T: Copy>(dst: *mut T, val: T, order: Ordering) {
    // SAFETY: 由调用方保证 `atomic_store` 的安全契约:dst 指向有效且对齐的可写内存,数据竞争由原子 intrinsic 防止。
    unsafe {
        match order {
            Relaxed => intrinsics::atomic_store::<T, { AO::Relaxed }>(dst, val),
            Release => intrinsics::atomic_store::<T, { AO::Release }>(dst, val),
            SeqCst => intrinsics::atomic_store::<T, { AO::SeqCst }>(dst, val),
            Acquire => panic!("there is no such thing as an acquire store"),
            AcqRel => panic!("there is no such thing as an acquire-release store"),
        }
    }
}

#[inline]
#[cfg_attr(miri, track_caller)] // 即使不 panic,这也有助于 Miri 的回溯信息
unsafe fn atomic_load<T: Copy>(dst: *const T, order: Ordering) -> T {
    // SAFETY: 由调用方保证 `atomic_load` 的安全契约:dst 指向有效且对齐的可读内存,数据竞争由原子 intrinsic 防止。
    unsafe {
        match order {
            Relaxed => intrinsics::atomic_load::<T, { AO::Relaxed }>(dst),
            Acquire => intrinsics::atomic_load::<T, { AO::Acquire }>(dst),
            SeqCst => intrinsics::atomic_load::<T, { AO::SeqCst }>(dst),
            Release => panic!("there is no such thing as a release load"),
            AcqRel => panic!("there is no such thing as an acquire-release load"),
        }
    }
}

#[inline]
#[cfg(target_has_atomic)]
#[cfg_attr(miri, track_caller)] // 即使不 panic,这也有助于 Miri 的回溯信息
unsafe fn atomic_swap<T: Copy>(dst: *mut T, val: T, order: Ordering) -> T {
    // SAFETY: 由调用方保证 `atomic_swap` 的安全契约:dst 指向有效且对齐的可读写内存,数据竞争由原子 intrinsic 防止。
    unsafe {
        match order {
            Relaxed => intrinsics::atomic_xchg::<T, { AO::Relaxed }>(dst, val),
            Acquire => intrinsics::atomic_xchg::<T, { AO::Acquire }>(dst, val),
            Release => intrinsics::atomic_xchg::<T, { AO::Release }>(dst, val),
            AcqRel => intrinsics::atomic_xchg::<T, { AO::AcqRel }>(dst, val),
            SeqCst => intrinsics::atomic_xchg::<T, { AO::SeqCst }>(dst, val),
        }
    }
}

/// 返回操作前的旧值(类似 __sync_fetch_and_add)。
#[inline]
#[cfg(target_has_atomic)]
#[cfg_attr(miri, track_caller)] // 即使不 panic,这也有助于 Miri 的回溯信息
unsafe fn atomic_add<T: Copy, U: Copy>(dst: *mut T, val: U, order: Ordering) -> T {
    // SAFETY: 由调用方保证 `atomic_add` 的安全契约:dst 指向有效且对齐的可读写内存,数据竞争由原子 intrinsic 防止。
    unsafe {
        match order {
            Relaxed => intrinsics::atomic_xadd::<T, U, { AO::Relaxed }>(dst, val),
            Acquire => intrinsics::atomic_xadd::<T, U, { AO::Acquire }>(dst, val),
            Release => intrinsics::atomic_xadd::<T, U, { AO::Release }>(dst, val),
            AcqRel => intrinsics::atomic_xadd::<T, U, { AO::AcqRel }>(dst, val),
            SeqCst => intrinsics::atomic_xadd::<T, U, { AO::SeqCst }>(dst, val),
        }
    }
}

/// 返回操作前的旧值(类似 __sync_fetch_and_sub)。
#[inline]
#[cfg(target_has_atomic)]
#[cfg_attr(miri, track_caller)] // 即使不 panic,这也有助于 Miri 的回溯信息
unsafe fn atomic_sub<T: Copy, U: Copy>(dst: *mut T, val: U, order: Ordering) -> T {
    // SAFETY: 由调用方保证 `atomic_sub` 的安全契约:dst 指向有效且对齐的可读写内存,数据竞争由原子 intrinsic 防止。
    unsafe {
        match order {
            Relaxed => intrinsics::atomic_xsub::<T, U, { AO::Relaxed }>(dst, val),
            Acquire => intrinsics::atomic_xsub::<T, U, { AO::Acquire }>(dst, val),
            Release => intrinsics::atomic_xsub::<T, U, { AO::Release }>(dst, val),
            AcqRel => intrinsics::atomic_xsub::<T, U, { AO::AcqRel }>(dst, val),
            SeqCst => intrinsics::atomic_xsub::<T, U, { AO::SeqCst }>(dst, val),
        }
    }
}

/// 为 stdarch 公开暴露;其他任何人都不应使用它。
#[inline]
#[cfg(target_has_atomic)]
#[cfg_attr(miri, track_caller)] // 即使不 panic,这也有助于 Miri 的回溯信息
#[unstable(feature = "core_intrinsics", issue = "none")]
#[doc(hidden)]
pub unsafe fn atomic_compare_exchange<T: Copy>(
    dst: *mut T,
    old: T,
    new: T,
    success: Ordering,
    failure: Ordering,
) -> Result<T, T> {
    // SAFETY: 由调用方保证 `atomic_compare_exchange` 的安全契约:dst 指向有效且对齐的可读写内存,数据竞争由原子 intrinsic 防止。
    let (val, ok) = unsafe {
        match (success, failure) {
            (Relaxed, Relaxed) => {
                intrinsics::atomic_cxchg::<T, { AO::Relaxed }, { AO::Relaxed }>(dst, old, new)
            }
            (Relaxed, Acquire) => {
                intrinsics::atomic_cxchg::<T, { AO::Relaxed }, { AO::Acquire }>(dst, old, new)
            }
            (Relaxed, SeqCst) => {
                intrinsics::atomic_cxchg::<T, { AO::Relaxed }, { AO::SeqCst }>(dst, old, new)
            }
            (Acquire, Relaxed) => {
                intrinsics::atomic_cxchg::<T, { AO::Acquire }, { AO::Relaxed }>(dst, old, new)
            }
            (Acquire, Acquire) => {
                intrinsics::atomic_cxchg::<T, { AO::Acquire }, { AO::Acquire }>(dst, old, new)
            }
            (Acquire, SeqCst) => {
                intrinsics::atomic_cxchg::<T, { AO::Acquire }, { AO::SeqCst }>(dst, old, new)
            }
            (Release, Relaxed) => {
                intrinsics::atomic_cxchg::<T, { AO::Release }, { AO::Relaxed }>(dst, old, new)
            }
            (Release, Acquire) => {
                intrinsics::atomic_cxchg::<T, { AO::Release }, { AO::Acquire }>(dst, old, new)
            }
            (Release, SeqCst) => {
                intrinsics::atomic_cxchg::<T, { AO::Release }, { AO::SeqCst }>(dst, old, new)
            }
            (AcqRel, Relaxed) => {
                intrinsics::atomic_cxchg::<T, { AO::AcqRel }, { AO::Relaxed }>(dst, old, new)
            }
            (AcqRel, Acquire) => {
                intrinsics::atomic_cxchg::<T, { AO::AcqRel }, { AO::Acquire }>(dst, old, new)
            }
            (AcqRel, SeqCst) => {
                intrinsics::atomic_cxchg::<T, { AO::AcqRel }, { AO::SeqCst }>(dst, old, new)
            }
            (SeqCst, Relaxed) => {
                intrinsics::atomic_cxchg::<T, { AO::SeqCst }, { AO::Relaxed }>(dst, old, new)
            }
            (SeqCst, Acquire) => {
                intrinsics::atomic_cxchg::<T, { AO::SeqCst }, { AO::Acquire }>(dst, old, new)
            }
            (SeqCst, SeqCst) => {
                intrinsics::atomic_cxchg::<T, { AO::SeqCst }, { AO::SeqCst }>(dst, old, new)
            }
            (_, AcqRel) => panic!("there is no such thing as an acquire-release failure ordering"),
            (_, Release) => panic!("there is no such thing as a release failure ordering"),
        }
    };
    if ok { Ok(val) } else { Err(val) }
}

#[inline]
#[cfg(target_has_atomic)]
#[cfg_attr(miri, track_caller)] // 即使不 panic,这也有助于 Miri 的回溯信息
unsafe fn atomic_compare_exchange_weak<T: Copy>(
    dst: *mut T,
    old: T,
    new: T,
    success: Ordering,
    failure: Ordering,
) -> Result<T, T> {
    // SAFETY: 由调用方保证 `atomic_compare_exchange_weak` 的安全契约:dst 指向有效且对齐的可读写内存,数据竞争由原子 intrinsic 防止。
    let (val, ok) = unsafe {
        match (success, failure) {
            (Relaxed, Relaxed) => {
                intrinsics::atomic_cxchgweak::<T, { AO::Relaxed }, { AO::Relaxed }>(dst, old, new)
            }
            (Relaxed, Acquire) => {
                intrinsics::atomic_cxchgweak::<T, { AO::Relaxed }, { AO::Acquire }>(dst, old, new)
            }
            (Relaxed, SeqCst) => {
                intrinsics::atomic_cxchgweak::<T, { AO::Relaxed }, { AO::SeqCst }>(dst, old, new)
            }
            (Acquire, Relaxed) => {
                intrinsics::atomic_cxchgweak::<T, { AO::Acquire }, { AO::Relaxed }>(dst, old, new)
            }
            (Acquire, Acquire) => {
                intrinsics::atomic_cxchgweak::<T, { AO::Acquire }, { AO::Acquire }>(dst, old, new)
            }
            (Acquire, SeqCst) => {
                intrinsics::atomic_cxchgweak::<T, { AO::Acquire }, { AO::SeqCst }>(dst, old, new)
            }
            (Release, Relaxed) => {
                intrinsics::atomic_cxchgweak::<T, { AO::Release }, { AO::Relaxed }>(dst, old, new)
            }
            (Release, Acquire) => {
                intrinsics::atomic_cxchgweak::<T, { AO::Release }, { AO::Acquire }>(dst, old, new)
            }
            (Release, SeqCst) => {
                intrinsics::atomic_cxchgweak::<T, { AO::Release }, { AO::SeqCst }>(dst, old, new)
            }
            (AcqRel, Relaxed) => {
                intrinsics::atomic_cxchgweak::<T, { AO::AcqRel }, { AO::Relaxed }>(dst, old, new)
            }
            (AcqRel, Acquire) => {
                intrinsics::atomic_cxchgweak::<T, { AO::AcqRel }, { AO::Acquire }>(dst, old, new)
            }
            (AcqRel, SeqCst) => {
                intrinsics::atomic_cxchgweak::<T, { AO::AcqRel }, { AO::SeqCst }>(dst, old, new)
            }
            (SeqCst, Relaxed) => {
                intrinsics::atomic_cxchgweak::<T, { AO::SeqCst }, { AO::Relaxed }>(dst, old, new)
            }
            (SeqCst, Acquire) => {
                intrinsics::atomic_cxchgweak::<T, { AO::SeqCst }, { AO::Acquire }>(dst, old, new)
            }
            (SeqCst, SeqCst) => {
                intrinsics::atomic_cxchgweak::<T, { AO::SeqCst }, { AO::SeqCst }>(dst, old, new)
            }
            (_, AcqRel) => panic!("there is no such thing as an acquire-release failure ordering"),
            (_, Release) => panic!("there is no such thing as a release failure ordering"),
        }
    };
    if ok { Ok(val) } else { Err(val) }
}

#[inline]
#[cfg(target_has_atomic)]
#[cfg_attr(miri, track_caller)] // 即使不 panic,这也有助于 Miri 的回溯信息
unsafe fn atomic_and<T: Copy, U: Copy>(dst: *mut T, val: U, order: Ordering) -> T {
    // SAFETY: 由调用方保证 `atomic_and` 的安全契约:dst 指向有效且对齐的可读写内存,数据竞争由原子 intrinsic 防止。
    unsafe {
        match order {
            Relaxed => intrinsics::atomic_and::<T, U, { AO::Relaxed }>(dst, val),
            Acquire => intrinsics::atomic_and::<T, U, { AO::Acquire }>(dst, val),
            Release => intrinsics::atomic_and::<T, U, { AO::Release }>(dst, val),
            AcqRel => intrinsics::atomic_and::<T, U, { AO::AcqRel }>(dst, val),
            SeqCst => intrinsics::atomic_and::<T, U, { AO::SeqCst }>(dst, val),
        }
    }
}

#[inline]
#[cfg(target_has_atomic)]
#[cfg_attr(miri, track_caller)] // 即使不 panic,这也有助于 Miri 的回溯信息
unsafe fn atomic_nand<T: Copy, U: Copy>(dst: *mut T, val: U, order: Ordering) -> T {
    // SAFETY: 由调用方保证 `atomic_nand` 的安全契约:dst 指向有效且对齐的可读写内存,数据竞争由原子 intrinsic 防止。
    unsafe {
        match order {
            Relaxed => intrinsics::atomic_nand::<T, U, { AO::Relaxed }>(dst, val),
            Acquire => intrinsics::atomic_nand::<T, U, { AO::Acquire }>(dst, val),
            Release => intrinsics::atomic_nand::<T, U, { AO::Release }>(dst, val),
            AcqRel => intrinsics::atomic_nand::<T, U, { AO::AcqRel }>(dst, val),
            SeqCst => intrinsics::atomic_nand::<T, U, { AO::SeqCst }>(dst, val),
        }
    }
}

#[inline]
#[cfg(target_has_atomic)]
#[cfg_attr(miri, track_caller)] // 即使不 panic,这也有助于 Miri 的回溯信息
unsafe fn atomic_or<T: Copy, U: Copy>(dst: *mut T, val: U, order: Ordering) -> T {
    // SAFETY: 由调用方保证 `atomic_or` 的安全契约:dst 指向有效且对齐的可读写内存,数据竞争由原子 intrinsic 防止。
    unsafe {
        match order {
            SeqCst => intrinsics::atomic_or::<T, U, { AO::SeqCst }>(dst, val),
            Acquire => intrinsics::atomic_or::<T, U, { AO::Acquire }>(dst, val),
            Release => intrinsics::atomic_or::<T, U, { AO::Release }>(dst, val),
            AcqRel => intrinsics::atomic_or::<T, U, { AO::AcqRel }>(dst, val),
            Relaxed => intrinsics::atomic_or::<T, U, { AO::Relaxed }>(dst, val),
        }
    }
}

#[inline]
#[cfg(target_has_atomic)]
#[cfg_attr(miri, track_caller)] // 即使不 panic,这也有助于 Miri 的回溯信息
unsafe fn atomic_xor<T: Copy, U: Copy>(dst: *mut T, val: U, order: Ordering) -> T {
    // SAFETY: 由调用方保证 `atomic_xor` 的安全契约:dst 指向有效且对齐的可读写内存,数据竞争由原子 intrinsic 防止。
    unsafe {
        match order {
            SeqCst => intrinsics::atomic_xor::<T, U, { AO::SeqCst }>(dst, val),
            Acquire => intrinsics::atomic_xor::<T, U, { AO::Acquire }>(dst, val),
            Release => intrinsics::atomic_xor::<T, U, { AO::Release }>(dst, val),
            AcqRel => intrinsics::atomic_xor::<T, U, { AO::AcqRel }>(dst, val),
            Relaxed => intrinsics::atomic_xor::<T, U, { AO::Relaxed }>(dst, val),
        }
    }
}

/// 把 `*dst` 更新为 `val` 与旧值中的较大者(有符号比较)。
#[inline]
#[cfg(target_has_atomic)]
#[cfg_attr(miri, track_caller)] // 即使不 panic,这也有助于 Miri 的回溯信息
unsafe fn atomic_max<T: Copy>(dst: *mut T, val: T, order: Ordering) -> T {
    // SAFETY: 由调用方保证 `atomic_max` 的安全契约:dst 指向有效且对齐的可读写内存,数据竞争由原子 intrinsic 防止。
    unsafe {
        match order {
            Relaxed => intrinsics::atomic_max::<T, { AO::Relaxed }>(dst, val),
            Acquire => intrinsics::atomic_max::<T, { AO::Acquire }>(dst, val),
            Release => intrinsics::atomic_max::<T, { AO::Release }>(dst, val),
            AcqRel => intrinsics::atomic_max::<T, { AO::AcqRel }>(dst, val),
            SeqCst => intrinsics::atomic_max::<T, { AO::SeqCst }>(dst, val),
        }
    }
}

/// 把 `*dst` 更新为 `val` 与旧值中的较小者(有符号比较)。
#[inline]
#[cfg(target_has_atomic)]
#[cfg_attr(miri, track_caller)] // 即使不 panic,这也有助于 Miri 的回溯信息
unsafe fn atomic_min<T: Copy>(dst: *mut T, val: T, order: Ordering) -> T {
    // SAFETY: 由调用方保证 `atomic_min` 的安全契约:dst 指向有效且对齐的可读写内存,数据竞争由原子 intrinsic 防止。
    unsafe {
        match order {
            Relaxed => intrinsics::atomic_min::<T, { AO::Relaxed }>(dst, val),
            Acquire => intrinsics::atomic_min::<T, { AO::Acquire }>(dst, val),
            Release => intrinsics::atomic_min::<T, { AO::Release }>(dst, val),
            AcqRel => intrinsics::atomic_min::<T, { AO::AcqRel }>(dst, val),
            SeqCst => intrinsics::atomic_min::<T, { AO::SeqCst }>(dst, val),
        }
    }
}

/// 把 `*dst` 更新为 `val` 与旧值中的较大者(无符号比较)。
#[inline]
#[cfg(target_has_atomic)]
#[cfg_attr(miri, track_caller)] // 即使不 panic,这也有助于 Miri 的回溯信息
unsafe fn atomic_umax<T: Copy>(dst: *mut T, val: T, order: Ordering) -> T {
    // SAFETY: 由调用方保证 `atomic_umax` 的安全契约:dst 指向有效且对齐的可读写内存,数据竞争由原子 intrinsic 防止。
    unsafe {
        match order {
            Relaxed => intrinsics::atomic_umax::<T, { AO::Relaxed }>(dst, val),
            Acquire => intrinsics::atomic_umax::<T, { AO::Acquire }>(dst, val),
            Release => intrinsics::atomic_umax::<T, { AO::Release }>(dst, val),
            AcqRel => intrinsics::atomic_umax::<T, { AO::AcqRel }>(dst, val),
            SeqCst => intrinsics::atomic_umax::<T, { AO::SeqCst }>(dst, val),
        }
    }
}

/// 把 `*dst` 更新为 `val` 与旧值中的较小者(无符号比较)。
#[inline]
#[cfg(target_has_atomic)]
#[cfg_attr(miri, track_caller)] // 即使不 panic,这也有助于 Miri 的回溯信息
unsafe fn atomic_umin<T: Copy>(dst: *mut T, val: T, order: Ordering) -> T {
    // SAFETY: 由调用方保证 `atomic_umin` 的安全契约:dst 指向有效且对齐的可读写内存,数据竞争由原子 intrinsic 防止。
    unsafe {
        match order {
            Relaxed => intrinsics::atomic_umin::<T, { AO::Relaxed }>(dst, val),
            Acquire => intrinsics::atomic_umin::<T, { AO::Acquire }>(dst, val),
            Release => intrinsics::atomic_umin::<T, { AO::Release }>(dst, val),
            AcqRel => intrinsics::atomic_umin::<T, { AO::AcqRel }>(dst, val),
            SeqCst => intrinsics::atomic_umin::<T, { AO::SeqCst }>(dst, val),
        }
    }
}

/// 一个原子屏障(fence)。
///
/// 屏障在自身与其他线程中的原子操作或屏障之间建立同步关系。为做到这一点,
/// 屏障会阻止编译器和 CPU 把某些类型的内存操作重排到它的另一侧。
///
/// 注意:与原子操作不同,屏障并不绑定到任何具体的内存位置;它建立的是不绑定特定地址的
/// happens-before 关系,需要与原子操作配对才能起作用。
///
/// 使用原子屏障有 3 种不同的方式:
///
/// - 原子 - 屏障 同步:一个具有(至少)[`Release`] 顺序语义的原子操作,
///   与一个具有(至少)[`Acquire`] 顺序语义的屏障同步。
/// - 屏障 - 原子 同步:一个具有(至少)[`Release`] 顺序语义的屏障,
///   与一个具有(至少)[`Acquire`] 顺序语义的原子操作同步。
/// - 屏障 - 屏障 同步:一个具有(至少)[`Release`] 顺序语义的屏障,
///   与一个具有(至少)[`Acquire`] 顺序语义的屏障同步。
///
/// 这 3 种方式是对常规的、无屏障的 原子 - 原子 同步的补充。
///
/// ## 原子 - 屏障(Atomic - Fence)
///
/// 当满足以下条件时,一个线程上的原子操作会与另一个线程上的屏障同步:
///
/// -   在线程 1 上:
///     -   对某个原子对象 'm' 执行一个具有(至少)[`Release`] 顺序语义的原子操作 'X',
///
/// -   在线程 2 上与之配对:
///     -   对 'm' 执行一个任意顺序的原子读 'Y',
///     -   其后跟随一个具有(至少)[`Acquire`] 顺序语义的屏障 'B'。
///
/// 这就在 X 与 B 之间提供了 happens-before 依赖。
///
/// ```text
///     Thread 1                                          Thread 2
///
/// m.store(3, Release); X ---------
///                                |
///                                |
///                                -------------> Y  if m.load(Relaxed) == 3 {
///                                               B      fence(Acquire);
///                                                      ...
///                                                  }
/// ```
///
/// ## 屏障 - 原子(Fence - Atomic)
///
/// 当满足以下条件时,一个线程上的屏障会与另一个线程上的原子操作同步:
///
/// -   在某线程上:
///     -   一个具有(至少)[`Release`] 顺序语义的屏障 'A',
///     -   其后跟随对某个原子对象 'm' 的一个任意顺序的原子写 'X',
///
/// -   在线程 2 上与之配对:
///     -   一个具有(至少)[`Acquire`] 顺序语义的原子操作 'Y'。
///
/// 这就在 A 与 Y 之间提供了 happens-before 依赖。
///
/// ```text
///     Thread 1                                          Thread 2
///
/// fence(Release);      A
/// m.store(3, Relaxed); X ---------
///                                |
///                                |
///                                -------------> Y  if m.load(Acquire) == 3 {
///                                                      ...
///                                                  }
/// ```
///
/// ## 屏障 - 屏障(Fence - Fence)
///
/// 当满足以下条件时,一个线程上的屏障会与另一个线程上的屏障同步:
///
/// -   在线程 1 上:
///     -   一个具有(至少)[`Release`] 顺序语义的屏障 'A',
///     -   其后跟随对某个原子对象 'm' 的一个任意顺序的原子写 'X',
///
/// -   在线程 2 上与之配对:
///     -   对 'm' 执行一个任意顺序的原子读 'Y',
///     -   其后跟随一个具有(至少)[`Acquire`] 顺序语义的屏障 'B'。
///
/// 这就在 A 与 B 之间提供了 happens-before 依赖。
///
/// ```text
///     Thread 1                                          Thread 2
///
/// fence(Release);      A --------------
/// m.store(3, Relaxed); X ---------    |
///                                |    |
///                                |    |
///                                -------------> Y  if m.load(Relaxed) == 3 {
///                                     |-------> B      fence(Acquire);
///                                                      ...
///                                                  }
/// ```
///
/// ## 必须使用原子操作(Mandatory Atomic)
///
/// 注意:在上面的例子中,对 `m` 的访问是原子的这一点至关重要。屏障无法用来在不同线程的
/// *非原子* 访问之间建立同步。不过,得益于 happens-before 关系,任何 happens-before 于
/// 那个具有(至少)[`Release`] 顺序语义的原子操作或屏障的非原子访问,如今也会与任何
/// happens-after 于那个具有(至少)[`Acquire`] 顺序语义的原子操作或屏障的非原子访问
/// 正确地同步起来。
///
/// ## 内存顺序(Memory Ordering)
///
/// 一个具有 [`SeqCst`] 顺序的屏障,除了同时具备 [`Acquire`] 和 [`Release`] 语义外,
/// 还会参与到其他 [`SeqCst`] 操作和/或屏障的全局程序顺序中。
///
/// 接受 [`Acquire`]、[`Release`]、[`AcqRel`] 和 [`SeqCst`] 顺序。
///
/// # Panics
///
/// 当 `order` 为 [`Relaxed`] 时会 panic。
///
/// # 示例
///
/// ```
/// use std::sync::atomic::AtomicBool;
/// use std::sync::atomic::fence;
/// use std::sync::atomic::Ordering;
///
/// // 一个基于自旋锁(spinlock)的互斥原语。
/// pub struct Mutex {
///     flag: AtomicBool,
/// }
///
/// impl Mutex {
///     pub fn new() -> Mutex {
///         Mutex {
///             flag: AtomicBool::new(false),
///         }
///     }
///
///     pub fn lock(&self) {
///         // 一直等待,直到旧值为 `false`。
///         while self
///             .flag
///             .compare_exchange_weak(false, true, Ordering::Relaxed, Ordering::Relaxed)
///             .is_err()
///         {}
///         // 这个屏障与 `unlock` 中的 store 建立 synchronizes-with 关系。
///         fence(Ordering::Acquire);
///     }
///
///     pub fn unlock(&self) {
///         self.flag.store(false, Ordering::Release);
///     }
/// }
/// ```
#[inline]
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_diagnostic_item = "fence"]
#[cfg_attr(miri, track_caller)] // 即使不 panic,这也有助于 Miri 的回溯信息
pub fn fence(order: Ordering) {
    // SAFETY: 使用原子屏障是安全的。
    unsafe {
        match order {
            Acquire => intrinsics::atomic_fence::<{ AO::Acquire }>(),
            Release => intrinsics::atomic_fence::<{ AO::Release }>(),
            AcqRel => intrinsics::atomic_fence::<{ AO::AcqRel }>(),
            SeqCst => intrinsics::atomic_fence::<{ AO::SeqCst }>(),
            Relaxed => panic!("there is no such thing as a relaxed fence"),
        }
    }
}

/// 一个“仅编译器层面”的原子屏障。
///
/// 与 [`fence`] 一样,本函数也在自身与其他原子操作和屏障之间建立同步关系。
/// 但与 [`fence`] 不同,`compiler_fence` 只与 *同一线程内* 的操作建立同步。
/// 这乍听起来似乎相当无用 —— 因为同一线程内的代码通常本就是全序的,无需任何额外的同步。
/// 然而,确实存在代码运行在同一线程上却没有被定序的情形:
/// - 最常见的就是 *信号处理函数(signal handler)* 的情况:信号处理函数运行在它所中断的代码
///   所在的同一线程上,但相对于那段代码它并没有被定序。可以用 `compiler_fence` 在一个线程
///   与它的信号处理函数之间建立同步,就像用 `fence` 在线程之间建立同步那样。
/// - 类似的情形也会出现在带中断处理函数的嵌入式编程中,或者在抢占式 green thread 的自定义实现中。
///   一般而言,`compiler_fence` 可以与那些保证运行在同一物理 CPU 上的代码建立同步。
///
/// 关于如何用屏障来达成同步,参见 [`fence`]。注意:正如 [`fence`] 一样,
/// 同步仍然要求在两侧都使用原子操作 —— 不可能仅凭屏障和非原子操作就完成同步。
///
/// `compiler_fence` 不会发出任何机器码,而只是限制编译器被允许做的内存重排种类。
/// `compiler_fence` 对应于 C 和 C++ 中的 [`atomic_signal_fence`]。
///
/// [`atomic_signal_fence`]: https://en.cppreference.com/w/cpp/atomic/atomic_signal_fence
///
/// # Panics
///
/// 当 `order` 为 [`Relaxed`] 时会 panic。
///
/// # 示例
///
/// 如果没有那两次 `compiler_fence` 调用,`signal_handler` 中对 `IMPORTANT_VARIABLE` 的读取
/// 就是由数据竞争导致的 *未定义行为(undefined behavior)*,尽管一切都发生在单个线程中。
/// 这是因为信号处理函数被认为与其关联线程并发运行,而在线程与其信号处理函数之间传递数据
/// 需要显式的同步。下面的代码用两次 `compiler_fence` 调用建立起常见的 release-acquire 同步模式
///(图示参见 [`fence`])。
///
/// ```
/// use std::sync::atomic::AtomicBool;
/// use std::sync::atomic::Ordering;
/// use std::sync::atomic::compiler_fence;
///
/// static mut IMPORTANT_VARIABLE: usize = 0;
/// static IS_READY: AtomicBool = AtomicBool::new(false);
///
/// fn main() {
///     unsafe { IMPORTANT_VARIABLE = 42 };
///     // 把前面的写标记为:相对于未来的 relaxed store 被释放(released)。
///     compiler_fence(Ordering::Release);
///     IS_READY.store(true, Ordering::Relaxed);
/// }
///
/// fn signal_handler() {
///     if IS_READY.load(Ordering::Relaxed) {
///         // 获取(acquire)那些通过我们读到的 relaxed store 释放出来的写。
///         compiler_fence(Ordering::Acquire);
///         assert_eq!(unsafe { IMPORTANT_VARIABLE }, 42);
///     }
/// }
/// ```
#[inline]
#[stable(feature = "compiler_fences", since = "1.21.0")]
#[rustc_diagnostic_item = "compiler_fence"]
#[cfg_attr(miri, track_caller)] // 即使不 panic,这也有助于 Miri 的回溯信息
pub fn compiler_fence(order: Ordering) {
    // SAFETY: 使用原子屏障是安全的。
    unsafe {
        match order {
            Acquire => intrinsics::atomic_singlethreadfence::<{ AO::Acquire }>(),
            Release => intrinsics::atomic_singlethreadfence::<{ AO::Release }>(),
            AcqRel => intrinsics::atomic_singlethreadfence::<{ AO::AcqRel }>(),
            SeqCst => intrinsics::atomic_singlethreadfence::<{ AO::SeqCst }>(),
            Relaxed => panic!("there is no such thing as a relaxed fence"),
        }
    }
}

#[cfg(target_has_atomic_load_store = "8")]
#[stable(feature = "atomic_debug", since = "1.3.0")]
impl fmt::Debug for AtomicBool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.load(Ordering::Relaxed), f)
    }
}

#[cfg(target_has_atomic_load_store = "ptr")]
#[stable(feature = "atomic_debug", since = "1.3.0")]
impl<T> fmt::Debug for AtomicPtr<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.load(Ordering::Relaxed), f)
    }
}

#[cfg(target_has_atomic_load_store = "ptr")]
#[stable(feature = "atomic_pointer", since = "1.24.0")]
impl<T> fmt::Pointer for AtomicPtr<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Pointer::fmt(&self.load(Ordering::Relaxed), f)
    }
}

/// 向处理器发出信号:它正处于一个忙等待的自旋循环(“spin lock”)之中。
///
/// 本函数已被弃用,请改用 [`hint::spin_loop`]。
///
/// [`hint::spin_loop`]: crate::hint::spin_loop
#[inline]
#[stable(feature = "spin_loop_hint", since = "1.24.0")]
#[deprecated(since = "1.51.0", note = "use hint::spin_loop instead")]
pub fn spin_loop_hint() {
    spin_loop()
}
