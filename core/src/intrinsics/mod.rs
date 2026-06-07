//! 编译器 intrinsic（编译器内建操作）。
//!
//! 本模块中的函数是 `core` 的实现细节，不应在标准库之外使用。我们通常通过稳定的封装函数来提供
//! 对 intrinsic 的访问，请改用那些封装。
//!
//! 设计背景：intrinsic 是编译器内建操作，是 std/core 与编译器（rustc/LLVM）之间的**契约层**。
//! 它们大多不稳定（unstable），稳定的对外 API 由上层封装后才暴露给用户（例如 `u32::wrapping_add`、
//! `ptr::copy`）。许多 intrinsic 是 `unsafe`，带有严格的前置条件，**误用直接导致 UB（未定义行为）**；
//! 而且正因为它们是 intrinsic，编译器会基于这些前置条件做激进优化，一旦违反，后果往往更加隐蔽。
//!
//! 这里只是把 intrinsic 声明（import）出来供 Rust 代码调用，真正的实现位于编译器内部。
//! 其中一部分 intrinsic 会被降级（lower）为 MIR，见
//! <https://github.com/rust-lang/rust/blob/HEAD/compiler/rustc_mir_transform/src/lower_intrinsics.rs>。
//! 其余的 intrinsic，针对 LLVM 后端的实现见
//! <https://github.com/rust-lang/rust/blob/HEAD/compiler/rustc_codegen_ssa/src/mir/intrinsic.rs>
//! 与 <https://github.com/rust-lang/rust/blob/HEAD/compiler/rustc_codegen_llvm/src/intrinsic.rs>，
//! 针对 const 求值（编译期求值）的实现见
//! <https://github.com/rust-lang/rust/blob/HEAD/compiler/rustc_const_eval/src/interpret/intrinsics.rs>。
//!
//! # Const intrinsic（可在编译期使用的 intrinsic）
//!
//! 想让一个不稳定的 intrinsic 可在编译期使用，需要把实现从
//! <https://github.com/rust-lang/miri/blob/master/src/intrinsics> 拷贝到
//! <https://github.com/rust-lang/rust/blob/HEAD/compiler/rustc_const_eval/src/interpret/intrinsics.rs>，
//! 并把下面的 intrinsic 声明改成 `const fn`。这一步应当与 wg-const-eval 工作组协调进行。
//!
//! 如果某个 intrinsic 打算被一个带 `rustc_const_stable` 属性的 `const fn` 使用，
//! 则需要给该 intrinsic 加上 `#[rustc_intrinsic_const_stable_indirect]`。这类改动需要 T-lang 团队批准，
//! 因为它可能把一项特性固化进语言中，而用户代码在没有编译器支持的情况下无法复刻该特性。
//!
//! # Volatile（易变访问）
//!
//! volatile 系列 intrinsic 提供的操作意在作用于 I/O 内存（MMIO），它们保证不会被编译器跨其他
//! volatile intrinsic 重新排序。参见 [`read_volatile`][ptr::read_volatile] 与
//! [`write_volatile`][ptr::write_volatile]。注意：volatile 不保证原子性，也不提供多线程同步语义。
//!
//! # Atomics（原子操作）
//!
//! 原子系列 intrinsic 提供针对机器字的常见原子操作，并支持多种可能的内存序（memory ordering）。
//! 详见[原子类型][atomic]的文档。
//!
//! # Unwinding（栈展开）
//!
//! Rust 的 intrinsic 一般而言是可以栈展开（unwind）的。如果某个 intrinsic 永远不会展开，
//! 就给它加上 `#[rustc_nounwind]` 属性，以便编译器利用这一事实。
//!
//! 不过，即便对那些可能展开的 intrinsic，rustc 也会假定 Rust 的 intrinsic 永远不会发起一次
//! 外部（非 Rust）的展开；因此对于 panic=abort，我们总可以假定这些 intrinsic 不会展开。

#![unstable(
    feature = "core_intrinsics",
    reason = "intrinsics are unlikely to ever be stabilized, instead \
                      they should be used through stabilized interfaces \
                      in the rest of the standard library",
    issue = "none"
)]
#![allow(missing_docs)]

use crate::ffi::va_list::{VaArgSafe, VaList};
use crate::marker::{ConstParamTy, Destruct, DiscriminantKind, PointeeSized, Tuple};
use crate::{mem, ptr};

mod bounds;
pub mod fallback;
pub mod gpu;
pub mod mir;
pub mod simd;

// 这些 import 用于简化文档内的链接（intra-doc links）
#[allow(unused_imports)]
#[cfg(all(target_has_atomic = "8", target_has_atomic = "32", target_has_atomic = "ptr"))]
use crate::sync::atomic::{self, AtomicBool, AtomicI32, AtomicIsize, AtomicU32, Ordering};

/// 用作 intrinsic 内存序参数的类型。它与 `atomic::Ordering` 是两个不同的类型，
/// 这样我们就能把它标成 `ConstParamTy` 并固定这里使用的取值，而不必担心把这些细节泄漏到 stable 代码中。
#[derive(Debug, ConstParamTy, PartialEq, Eq)]
pub enum AtomicOrdering {
    // 这些取值必须与编译器中 `rustc_middle/src/ty/consts/int.rs` 里定义的 `AtomicOrdering` 保持一致！
    Relaxed = 0,
    Release = 1,
    Acquire = 2,
    AcqRel = 3,
    SeqCst = 4,
}

// 注意：这些 intrinsic 之所以接收裸指针，是因为它们会改写可能存在别名（aliased）的内存，
// 而这对 `&` 或 `&mut` 来说都是不合法的。

/// 当“当前值”与给定的 `old` 值相同时，才存入新值（比较并交换，compare-and-exchange）。
/// `T` 必须是整数或指针类型。
///
/// 本 intrinsic 的稳定版本可通过[原子类型][`atomic`]上的 `compare_exchange` 方法使用，
/// 例如 [`AtomicBool::compare_exchange`]。
#[rustc_intrinsic]
#[rustc_nounwind]
pub unsafe fn atomic_cxchg<
    T: Copy,
    const ORD_SUCC: AtomicOrdering,
    const ORD_FAIL: AtomicOrdering,
>(
    dst: *mut T,
    old: T,
    src: T,
) -> (T, bool);

/// 当“当前值”与给定的 `old` 值相同时，才存入新值。
/// `T` 必须是整数或指针类型。该比较可能伪失败（spuriously fail，即在值相等时也可能返回失败）。
///
/// 本 intrinsic 的稳定版本可通过[原子类型][`atomic`]上的 `compare_exchange_weak` 方法使用，
/// 例如 [`AtomicBool::compare_exchange_weak`]。
#[rustc_intrinsic]
#[rustc_nounwind]
pub unsafe fn atomic_cxchgweak<
    T: Copy,
    const ORD_SUCC: AtomicOrdering,
    const ORD_FAIL: AtomicOrdering,
>(
    _dst: *mut T,
    _old: T,
    _src: T,
) -> (T, bool);

/// 加载指针处的当前值。
/// `T` 必须是整数或指针类型。
///
/// 本 intrinsic 的稳定版本可通过[原子类型][`atomic`]上的 `load` 方法使用，例如 [`AtomicBool::load`]。
#[rustc_intrinsic]
#[rustc_nounwind]
pub unsafe fn atomic_load<T: Copy, const ORD: AtomicOrdering>(src: *const T) -> T;

/// 把值存入指定的内存位置。
/// `T` 必须是整数或指针类型。
///
/// 本 intrinsic 的稳定版本可通过[原子类型][`atomic`]上的 `store` 方法使用，例如 [`AtomicBool::store`]。
#[rustc_intrinsic]
#[rustc_nounwind]
pub unsafe fn atomic_store<T: Copy, const ORD: AtomicOrdering>(dst: *mut T, val: T);

/// 把值存入指定的内存位置，并返回旧值（原子交换，swap）。
/// `T` 必须是整数或指针类型。
///
/// 本 intrinsic 的稳定版本可通过[原子类型][`atomic`]上的 `swap` 方法使用，例如 [`AtomicBool::swap`]。
#[rustc_intrinsic]
#[rustc_nounwind]
pub unsafe fn atomic_xchg<T: Copy, const ORD: AtomicOrdering>(dst: *mut T, src: T) -> T;

/// 在当前值上做加法，并返回加法之前的旧值。
/// `T` 必须是整数或指针类型。
/// 若 `T` 是整数类型，则 `U` 必须与 `T` 相同；若 `T` 是指针类型，则 `U` 必须是 `usize`。
///
/// 本 intrinsic 的稳定版本可通过[原子类型][`atomic`]上的 `fetch_add` 方法使用，例如 [`AtomicIsize::fetch_add`]。
#[rustc_intrinsic]
#[rustc_nounwind]
pub unsafe fn atomic_xadd<T: Copy, U: Copy, const ORD: AtomicOrdering>(dst: *mut T, src: U) -> T;

/// 在当前值上做减法，并返回减法之前的旧值。
/// `T` 必须是整数或指针类型。
/// 若 `T` 是整数类型，则 `U` 必须与 `T` 相同；若 `T` 是指针类型，则 `U` 必须是 `usize`。
///
/// 本 intrinsic 的稳定版本可通过[原子类型][`atomic`]上的 `fetch_sub` 方法使用，例如 [`AtomicIsize::fetch_sub`]。
#[rustc_intrinsic]
#[rustc_nounwind]
pub unsafe fn atomic_xsub<T: Copy, U: Copy, const ORD: AtomicOrdering>(dst: *mut T, src: U) -> T;

/// 与当前值做按位与，并返回操作之前的旧值。
/// `T` 必须是整数或指针类型。
/// 若 `T` 是整数类型，则 `U` 必须与 `T` 相同；若 `T` 是指针类型，则 `U` 必须是 `usize`。
///
/// 本 intrinsic 的稳定版本可通过[原子类型][`atomic`]上的 `fetch_and` 方法使用，例如 [`AtomicBool::fetch_and`]。
#[rustc_intrinsic]
#[rustc_nounwind]
pub unsafe fn atomic_and<T: Copy, U: Copy, const ORD: AtomicOrdering>(dst: *mut T, src: U) -> T;

/// 与当前值做按位与非（nand），并返回操作之前的旧值。
/// `T` 必须是整数或指针类型。
/// 若 `T` 是整数类型，则 `U` 必须与 `T` 相同；若 `T` 是指针类型，则 `U` 必须是 `usize`。
///
/// 本 intrinsic 的稳定版本可通过 [`AtomicBool`] 类型上的 `fetch_nand` 方法使用，例如 [`AtomicBool::fetch_nand`]。
#[rustc_intrinsic]
#[rustc_nounwind]
pub unsafe fn atomic_nand<T: Copy, U: Copy, const ORD: AtomicOrdering>(dst: *mut T, src: U) -> T;

/// 与当前值做按位或，并返回操作之前的旧值。
/// `T` 必须是整数或指针类型。
/// 若 `T` 是整数类型，则 `U` 必须与 `T` 相同；若 `T` 是指针类型，则 `U` 必须是 `usize`。
///
/// 本 intrinsic 的稳定版本可通过[原子类型][`atomic`]上的 `fetch_or` 方法使用，例如 [`AtomicBool::fetch_or`]。
#[rustc_intrinsic]
#[rustc_nounwind]
pub unsafe fn atomic_or<T: Copy, U: Copy, const ORD: AtomicOrdering>(dst: *mut T, src: U) -> T;

/// 与当前值做按位异或，并返回操作之前的旧值。
/// `T` 必须是整数或指针类型。
/// 若 `T` 是整数类型，则 `U` 必须与 `T` 相同；若 `T` 是指针类型，则 `U` 必须是 `usize`。
///
/// 本 intrinsic 的稳定版本可通过[原子类型][`atomic`]上的 `fetch_xor` 方法使用，例如 [`AtomicBool::fetch_xor`]。
#[rustc_intrinsic]
#[rustc_nounwind]
pub unsafe fn atomic_xor<T: Copy, U: Copy, const ORD: AtomicOrdering>(dst: *mut T, src: U) -> T;

/// 用有符号比较取当前值与给定值的较大者（并写回），返回旧值。
/// `T` 必须是有符号整数类型。
///
/// 本 intrinsic 的稳定版本可通过有符号[原子整数类型][`atomic`]上的 `fetch_max` 方法使用，例如 [`AtomicI32::fetch_max`]。
#[rustc_intrinsic]
#[rustc_nounwind]
pub unsafe fn atomic_max<T: Copy, const ORD: AtomicOrdering>(dst: *mut T, src: T) -> T;

/// 用有符号比较取当前值与给定值的较小者（并写回），返回旧值。
/// `T` 必须是有符号整数类型。
///
/// 本 intrinsic 的稳定版本可通过有符号[原子整数类型][`atomic`]上的 `fetch_min` 方法使用，例如 [`AtomicI32::fetch_min`]。
#[rustc_intrinsic]
#[rustc_nounwind]
pub unsafe fn atomic_min<T: Copy, const ORD: AtomicOrdering>(dst: *mut T, src: T) -> T;

/// 用无符号比较取当前值与给定值的较小者（并写回），返回旧值。
/// `T` 必须是无符号整数类型。
///
/// 本 intrinsic 的稳定版本可通过无符号[原子整数类型][`atomic`]上的 `fetch_min` 方法使用，例如 [`AtomicU32::fetch_min`]。
#[rustc_intrinsic]
#[rustc_nounwind]
pub unsafe fn atomic_umin<T: Copy, const ORD: AtomicOrdering>(dst: *mut T, src: T) -> T;

/// 用无符号比较取当前值与给定值的较大者（并写回），返回旧值。
/// `T` 必须是无符号整数类型。
///
/// 本 intrinsic 的稳定版本可通过无符号[原子整数类型][`atomic`]上的 `fetch_max` 方法使用，例如 [`AtomicU32::fetch_max`]。
#[rustc_intrinsic]
#[rustc_nounwind]
pub unsafe fn atomic_umax<T: Copy, const ORD: AtomicOrdering>(dst: *mut T, src: T) -> T;

/// 一道原子栅栏（fence）。
///
/// 本 intrinsic 的稳定版本是 [`atomic::fence`]。
#[rustc_intrinsic]
#[rustc_nounwind]
pub unsafe fn atomic_fence<const ORD: AtomicOrdering>();

/// 一道仅用于单线程内部同步的原子栅栏（编译器栅栏）。
///
/// 本 intrinsic 的稳定版本是 [`atomic::compiler_fence`]。
#[rustc_intrinsic]
#[rustc_nounwind]
pub unsafe fn atomic_singlethreadfence<const ORD: AtomicOrdering>();

/// `prefetch`（预取）intrinsic 是给代码生成器的一个提示：若目标支持，就为给定地址插入一条预取指令；
/// 否则它是一个空操作（no-op）。
/// 预取不影响程序的行为，但可能改变其性能特征。
///
/// `LOCALITY` 参数是一个时间局部性（temporal locality）说明符，取值范围从 (0) — 无局部性，
/// 到 (3) — 极强局部性、应保留在缓存中。
///
/// 本 intrinsic 没有稳定的对应物。
#[rustc_intrinsic]
#[rustc_nounwind]
#[miri::intrinsic_fallback_is_spec]
pub const fn prefetch_read_data<T, const LOCALITY: i32>(data: *const T) {
    // 除非被后端覆盖实现，否则本操作是一个空操作（no-op）。
    let _ = data;
}

/// `prefetch`（预取）intrinsic 是给代码生成器的一个提示：若目标支持，就为给定地址插入一条预取指令；
/// 否则它是一个空操作（no-op）。
/// 预取不影响程序的行为，但可能改变其性能特征。
///
/// `LOCALITY` 参数是一个时间局部性（temporal locality）说明符，取值范围从 (0) — 无局部性，
/// 到 (3) — 极强局部性、应保留在缓存中。
///
/// 本 intrinsic 没有稳定的对应物。
#[rustc_intrinsic]
#[rustc_nounwind]
#[miri::intrinsic_fallback_is_spec]
pub const fn prefetch_write_data<T, const LOCALITY: i32>(data: *const T) {
    // 除非被后端覆盖实现，否则本操作是一个空操作（no-op）。
    let _ = data;
}

/// `prefetch`（预取）intrinsic 是给代码生成器的一个提示：若目标支持，就为给定地址插入一条预取指令；
/// 否则它是一个空操作（no-op）。
/// 预取不影响程序的行为，但可能改变其性能特征。
///
/// `LOCALITY` 参数是一个时间局部性（temporal locality）说明符，取值范围从 (0) — 无局部性，
/// 到 (3) — 极强局部性、应保留在缓存中。
///
/// 本 intrinsic 没有稳定的对应物。
#[rustc_intrinsic]
#[rustc_nounwind]
#[miri::intrinsic_fallback_is_spec]
pub const fn prefetch_read_instruction<T, const LOCALITY: i32>(data: *const T) {
    // 除非被后端覆盖实现，否则本操作是一个空操作（no-op）。
    let _ = data;
}

/// `prefetch`（预取）intrinsic 是给代码生成器的一个提示：若目标支持，就为给定地址插入一条预取指令；
/// 否则它是一个空操作（no-op）。
/// 预取不影响程序的行为，但可能改变其性能特征。
///
/// `LOCALITY` 参数是一个时间局部性（temporal locality）说明符，取值范围从 (0) — 无局部性，
/// 到 (3) — 极强局部性、应保留在缓存中。
///
/// 本 intrinsic 没有稳定的对应物。
#[rustc_intrinsic]
#[rustc_nounwind]
#[miri::intrinsic_fallback_is_spec]
pub const fn prefetch_write_instruction<T, const LOCALITY: i32>(data: *const T) {
    // 除非被后端覆盖实现，否则本操作是一个空操作（no-op）。
    let _ = data;
}

/// 执行一次断点陷阱（breakpoint trap），供调试器检查。
///
/// 本 intrinsic 没有稳定的对应物。
#[rustc_intrinsic]
#[rustc_nounwind]
pub fn breakpoint();

/// 一个“魔法” intrinsic，其含义来自附加在函数上的各种属性。
///
/// 例如，数据流分析（dataflow）用它来注入静态断言，这样 `rustc_peek(potentially_uninitialized)`
/// 就会真正地复核：在控制流的那个点上，数据流分析确实算出了该值是未初始化的。
///
/// 本 intrinsic 不应在编译器之外使用。
#[rustc_nounwind]
#[rustc_intrinsic]
pub fn rustc_peek<T>(_: T) -> T;

/// 中止（abort）进程的执行。
///
/// 注意，与大多数 intrinsic 不同，调用它是安全的；它不需要 `unsafe` 块。
/// 因此，实现绝不能要求用户去维护任何安全不变量。
///
/// 如果可能，应优先使用 [`std::process::abort`](../../std/process/fn.abort.html)，
/// 因为它的行为对用户更友好、也更稳定。
///
/// 在大多数平台上，`intrinsics::abort` 当前的实现是执行一条非法指令。
/// 在 Unix 上，进程多半会以 `SIGABRT`、`SIGILL`、`SIGTRAP`、`SIGSEGV` 或 `SIGBUS` 之类的信号终止。
/// 其确切行为既不被保证、也不稳定。
#[rustc_nounwind]
#[rustc_intrinsic]
pub fn abort() -> !;

/// 告知优化器：代码中的这个点不可达（not reachable），从而启用进一步的优化。
///
/// 注意，这与 `unreachable!()` 宏非常不同：那个宏在执行时会 panic，而抵达本函数所标记的代码
/// 则是*未定义行为（UB）*。
///
/// 本 intrinsic 的稳定版本是 [`core::hint::unreachable_unchecked`]。
#[rustc_intrinsic_const_stable_indirect]
#[rustc_nounwind]
#[rustc_intrinsic]
pub const unsafe fn unreachable() -> !;

/// 告知优化器：某个条件恒为真。
/// 如果该条件为假，行为即未定义（UB）。
///
/// 本 intrinsic 不生成任何代码，但优化器会试图在各 pass 之间保留它（及其条件），
/// 这可能干扰周围代码的优化、降低性能。如果该不变量本就能被优化器自行发现，
/// 或者它并不能启用任何显著的优化，就不应使用它。
///
/// 本 intrinsic 的稳定版本是 [`core::hint::assert_unchecked`]。
#[rustc_intrinsic_const_stable_indirect]
#[rustc_nounwind]
#[unstable(feature = "core_intrinsics", issue = "none")]
#[rustc_intrinsic]
pub const unsafe fn assume(b: bool) {
    if !b {
        // SAFETY: 调用方必须保证参数永远不为 `false`；若为 `false`，抵达此处即为 UB。
        unsafe { unreachable() }
    }
}

/// 向编译器提示：当前代码路径是冷路径（cold，很少被执行到）。
///
/// 注意，与大多数 intrinsic 不同，调用它是安全的；它不需要 `unsafe` 块。
/// 因此，实现绝不能要求用户去维护任何安全不变量。
///
/// 本 intrinsic 没有稳定的对应物。
#[unstable(feature = "core_intrinsics", issue = "none")]
#[rustc_intrinsic]
#[rustc_nounwind]
#[miri::intrinsic_fallback_is_spec]
#[cold]
pub const fn cold_path() {}

/// 向编译器提示：分支条件很可能为真。返回传入的那个值。
///
/// 在 `if` 语句以外的任何用法都很可能不起作用。
///
/// 注意，与大多数 intrinsic 不同，调用它是安全的；它不需要 `unsafe` 块。
/// 因此，实现绝不能要求用户去维护任何安全不变量。
///
/// 本 intrinsic 没有稳定的对应物。
#[unstable(feature = "core_intrinsics", issue = "none")]
#[rustc_nounwind]
#[inline(always)]
pub const fn likely(b: bool) -> bool {
    if b {
        true
    } else {
        cold_path();
        false
    }
}

/// 向编译器提示：分支条件很可能为假。返回传入的那个值。
///
/// 在 `if` 语句以外的任何用法都很可能不起作用。
///
/// 注意，与大多数 intrinsic 不同，调用它是安全的；它不需要 `unsafe` 块。
/// 因此，实现绝不能要求用户去维护任何安全不变量。
///
/// 本 intrinsic 没有稳定的对应物。
#[unstable(feature = "core_intrinsics", issue = "none")]
#[rustc_nounwind]
#[inline(always)]
pub const fn unlikely(b: bool) -> bool {
    if b {
        cold_path();
        true
    } else {
        false
    }
}

/// 根据条件 `b` 返回 `true_val` 或 `false_val`，并向编译器提示：该条件不太可能被 CPU 的分支预测器
/// 正确预测（例如二分查找中的判断）。
///
/// 在其他方面，它的功能等价于 `if b { true_val } else { false_val }`。
///
/// 注意，与大多数 intrinsic 不同，调用它是安全的；它不需要 `unsafe` 块。
/// 因此，实现绝不能要求用户去维护任何安全不变量。
///
/// 本 intrinsic 的公开形式是 [`core::hint::select_unpredictable`]。
/// 但与公开形式不同，本 intrinsic 不会 drop 那个未被选中的值。
#[unstable(feature = "core_intrinsics", issue = "none")]
#[rustc_const_unstable(feature = "const_select_unpredictable", issue = "145938")]
#[rustc_intrinsic]
#[rustc_nounwind]
#[miri::intrinsic_fallback_is_spec]
#[inline]
pub const fn select_unpredictable<T>(b: bool, true_val: T, false_val: T) -> T
where
    T: [const] Destruct,
{
    if b { true_val } else { false_val }
}

/// 一个守卫（guard）：当 `T` 是无人居住类型（uninhabited，没有任何合法值，如 `!` 或空枚举）时，
/// 对应的 unsafe 函数就永远不可能被执行。它要么在编译期 panic，要么什么也不做。
/// 它*不保证*一定会 panic，只应在“断言失败即意味着后续代码触发语言级 UB”的场合调用。
///
/// 本 intrinsic 没有稳定的对应物。
#[rustc_intrinsic_const_stable_indirect]
#[rustc_nounwind]
#[rustc_intrinsic]
pub const fn assert_inhabited<T>();

/// 一个守卫（guard）：当 `T` 不允许零初始化时，对应的 unsafe 函数就永远不可能被执行。
/// 它要么在编译期 panic，要么什么也不做。它*不保证*一定会 panic，
/// 只应在“断言失败即意味着后续代码触发语言级 UB”的场合调用。
///
/// 本 intrinsic 没有稳定的对应物。
#[rustc_intrinsic_const_stable_indirect]
#[rustc_nounwind]
#[rustc_intrinsic]
pub const fn assert_zero_valid<T>();

/// `std::mem::uninitialized` 的守卫（guard）。它要么在编译期 panic，要么什么也不做。
/// 它*不保证*一定会 panic，只应在“断言失败即意味着后续代码触发语言级 UB”的场合调用。
///
/// 本 intrinsic 没有稳定的对应物。
#[rustc_intrinsic_const_stable_indirect]
#[rustc_nounwind]
#[rustc_intrinsic]
pub const fn assert_mem_uninitialized_valid<T>();

/// 获取一个指向静态 `Location` 的引用，指明本函数是在何处被调用的。
///
/// 注意，与大多数 intrinsic 不同，调用它是安全的；它不需要 `unsafe` 块。
/// 因此，实现绝不能要求用户去维护任何安全不变量。
///
/// 请考虑改用 [`core::panic::Location::caller`]。
#[rustc_intrinsic_const_stable_indirect]
#[rustc_nounwind]
#[rustc_intrinsic]
pub const fn caller_location() -> &'static crate::panic::Location<'static>;

/// 把一个值移出作用域，但不运行其 drop 胶水代码（即不调用析构）。
///
/// 它的存在仅仅是为了 [`crate::mem::forget_unsized`]；普通的 `forget` 改用 `ManuallyDrop`。
///
/// 注意，与大多数 intrinsic 不同，调用它是安全的；它不需要 `unsafe` 块。
/// 因此，实现绝不能要求用户去维护任何安全不变量。
#[rustc_intrinsic_const_stable_indirect]
#[rustc_nounwind]
#[rustc_intrinsic]
pub const fn forget<T: ?Sized>(_: T);

/// 把一个类型的值的比特位，重新解释（reinterpret）为另一个类型。
///
/// 两个类型必须大小相同。如果无法保证这一点，编译将失败。
///
/// 在语义上，`transmute` 等价于把一个类型按位移动（bitwise move）到另一个类型。它把源值的比特位
/// 拷贝到目标值中，然后 forget（遗忘）掉原值。注意：源和目标都是按值传递的，这意味着如果 `Src` 或 `Dst`
/// 含有填充（padding）字节，那么这些填充*不*保证会被 `transmute` 保留。
///
/// 参数和结果都必须在各自给定的类型下是[有效的（valid）](../../nomicon/what-unsafe-does.html)。
/// 违反这一条件会导致[未定义行为][ub]。编译器生成代码时，会*假定你这个程序员已经确保了永远不会发生
/// 未定义行为*。因此，保证传给 `transmute` 的每个值在 `Src` 和 `Dst` 两个类型下都有效，是你的责任。
/// 不能维护这一条件可能导致意料之外、且不稳定的编译结果。这使得 `transmute` **极其不安全（unsafe）**。
/// `transmute` 应当是万不得已时的最后手段。
///
/// 因为 `transmute` 是按值（by-value）操作，所以*被转换的值本身*的对齐不成问题。和任何其他函数一样，
/// 编译器已经确保了 `Src` 和 `Dst` 都被正确对齐。然而，当转换的值是*指向别处*的（例如指针、引用、box 等）时，
/// 调用方必须自行确保被指向的那些值的对齐。
///
/// [nomicon](../../nomicon/transmutes.html) 中有补充文档。
///
/// [ub]: ../../reference/behavior-considered-undefined.html
///
/// # 指针与整数之间的转换（transmute）
///
/// 在指针与整数之间做 transmute 时必须格外小心，例如在 `*const ()` 与 `usize` 之间转换。
///
/// 在 `const`（编译期）上下文中把*指针 transmute 成整数*是[未定义行为][ub]，除非该指针最初就是
/// *从*一个整数创建出来的。（这包括本函数本身、整数到指针的 cast、以及像 [`dangling`][crate::ptr::dangling]
/// 这样的辅助函数，但也包括语义上等价的转换，比如通过 `repr(C)` union 字段做“类型双关”punning。）
/// 任何试图把转换结果用于整数运算的行为，都会中止 const 求值。（即便在 `const` 之外，
/// 这类转换也触及了 Rust 内存模型中许多未规定的方面，应当避免。替代方案见下文。）
///
/// 把*整数 transmute 成指针*则在很大程度上是一个未规定的操作。它很可能*不*等价于 `as` cast。
/// 用这样构造出来的指针去做非零大小的内存访问，目前被视为未定义行为。
///
/// 当整数嵌套在数组、元组、结构体或枚举内部时，上述规则同样适用。
/// 不过，就本节而言，`MaybeUninit<usize>` 不被视为整数类型。把 `*const ()` transmute 成
/// `MaybeUninit<usize>` 是没问题的——但随后对结果调用 `assume_init()` 则被视为完成了那次
/// “指针转整数”的 transmute，于是又会撞上上面讨论的问题。
///
/// 特别地，通过 `transmute` 做一次“指针→整数→指针”的往返*并非*无损过程。如果你想让指针经过整数往返一圈
/// 之后还能拿回原来的指针，就需要使用 `as` cast，或者把整数类型换成 `MaybeUninit<$int>`
/// （且永远不要调用 `assume_init()`）。如果你想找一种方式来存放任意类型的数据，也请使用 `MaybeUninit<T>`
/// （它还能处理因填充而产生的未初始化内存）。如果你确实需要存放“要么是整数、要么是指针”的东西，
/// 请使用 `*mut ()`：整数可以无损地与指针来回转换（通过 `as` cast 或通过 `transmute`）。
///
/// # 示例
///
/// `transmute` 有几样确实很有用的用途。
///
/// 把一个指针转成函数指针。这*不*能移植到“函数指针与数据指针大小不同”的机器上。
///
/// ```
/// fn foo() -> i32 {
///     0
/// }
/// // 关键之处：在 `transmute` 成函数指针之前，我们先 `as`-cast 成裸指针。
/// // 这避免了“整数转指针”的 `transmute`，那种转换可能引发问题。
/// // 在裸指针与函数指针之间（即两个指针类型之间）做 transmute 是没问题的。
/// let pointer = foo as fn() -> i32 as *const ();
/// let function = unsafe {
///     std::mem::transmute::<*const (), fn() -> i32>(pointer)
/// };
/// assert_eq!(function(), 0);
/// ```
///
/// 延长一个生命周期，或缩短一个不变（invariant）生命周期。这是高级而非常 unsafe 的 Rust！
///
/// ```
/// struct R<'a>(&'a i32);
/// unsafe fn extend_lifetime<'b>(r: R<'b>) -> R<'static> {
///     unsafe { std::mem::transmute::<R<'b>, R<'static>>(r) }
/// }
///
/// unsafe fn shorten_invariant_lifetime<'b, 'c>(r: &'b mut R<'static>)
///                                              -> &'b mut R<'c> {
///     unsafe { std::mem::transmute::<&'b mut R<'static>, &'b mut R<'c>>(r) }
/// }
/// ```
///
/// # 替代方案
///
/// 不必绝望：`transmute` 的许多用途都能通过其他手段达成。下面列出一些 `transmute` 的常见应用，
/// 它们都可以用更安全的构造来替代。
///
/// 把原始字节（`[u8; SZ]`）转成 `u32`、`f64` 等：
///
/// ```
/// # #![allow(unnecessary_transmutes)]
/// let raw_bytes = [0x78, 0x56, 0x34, 0x12];
///
/// let num = unsafe {
///     std::mem::transmute::<[u8; 4], u32>(raw_bytes)
/// };
///
/// // 改用 `u32::from_ne_bytes`
/// let num = u32::from_ne_bytes(raw_bytes);
/// // 或者用 `u32::from_le_bytes`、`u32::from_be_bytes` 来指定字节序
/// let num = u32::from_le_bytes(raw_bytes);
/// assert_eq!(num, 0x12345678);
/// let num = u32::from_be_bytes(raw_bytes);
/// assert_eq!(num, 0x78563412);
/// ```
///
/// 把一个指针转成 `usize`：
///
/// ```no_run
/// let ptr = &0;
/// let ptr_num_transmute = unsafe {
///     std::mem::transmute::<&i32, usize>(ptr)
/// };
///
/// // 改用 `as` cast
/// let ptr_num_cast = ptr as *const i32 as usize;
/// ```
///
/// 注意，用 `transmute` 把指针转成 `usize`（如上文所述）在 `const` 上下文中是[未定义行为][ub]。
/// 即便在 const 之外，这个操作也可能不会按预期行事——它触及了 Rust 内存模型中许多未规定的方面。
/// 视代码要做的事情而定，以下替代方案优于“指针转整数”的 transmute：
/// - 如果代码只是想把任意类型的数据存进某个缓冲区、并需要为该缓冲区选一个类型，
///   它可以使用 [`MaybeUninit`][crate::mem::MaybeUninit]。
/// - 如果代码实际上想处理的是指针所指向的那个地址，它可以使用 `as` cast 或 [`ptr.addr()`][pointer::addr]。
///
/// 把 `*mut T` 转成 `&mut T`：
///
/// ```
/// let ptr: *mut i32 = &mut 0;
/// let ref_transmuted = unsafe {
///     std::mem::transmute::<*mut i32, &mut i32>(ptr)
/// };
///
/// // 改用 reborrow（重新借用）
/// let ref_casted = unsafe { &mut *ptr };
/// ```
///
/// 把 `&mut T` 转成 `&mut U`：
///
/// ```
/// let ptr = &mut 0;
/// let val_transmuted = unsafe {
///     std::mem::transmute::<&mut i32, &mut u32>(ptr)
/// };
///
/// // 这次把 `as` 与 reborrow 组合起来——注意 `as` 的链式写法
/// // `as` 不具传递性
/// let val_casts = unsafe { &mut *(ptr as *mut i32 as *mut u32) };
/// ```
///
/// 把 `&str` 转成 `&[u8]`：
///
/// ```
/// // 这并不是做这件事的好办法。
/// let slice = unsafe { std::mem::transmute::<&str, &[u8]>("Rust") };
/// assert_eq!(slice, &[82, 117, 115, 116]);
///
/// // 你可以用 `str::as_bytes`
/// let slice = "Rust".as_bytes();
/// assert_eq!(slice, &[82, 117, 115, 116]);
///
/// // 或者，如果你能控制那个字符串字面量，干脆直接用字节串字面量
/// assert_eq!(b"Rust", &[82, 117, 115, 116]);
/// ```
///
/// 把 `Vec<&T>` 转成 `Vec<Option<&T>>`。
///
/// 要转换容器内容的内部类型，你必须确保不违反该容器的任何不变量。对 `Vec` 而言，
/// 这意味着内部类型的大小*和对齐*都必须匹配。其他容器可能依赖类型的大小、对齐，
/// 甚至 `TypeId`，那种情况下若不违反容器不变量就根本无法进行 transmute。
///
/// ```
/// let store = [0, 1, 2, 3];
/// let v_orig = store.iter().collect::<Vec<&i32>>();
///
/// // 克隆这个 vector，因为我们稍后还要复用它们
/// let v_clone = v_orig.clone();
///
/// // 使用 transmute：这依赖于 `Vec` 未规定的数据布局，是个糟糕的主意，可能引发未定义行为。
/// // 不过，它是零拷贝的。
/// let v_transmuted = unsafe {
///     std::mem::transmute::<Vec<&i32>, Vec<Option<&i32>>>(v_clone)
/// };
///
/// let v_clone = v_orig.clone();
///
/// // 这是推荐的、安全的方式。
/// // 不过它可能会把整个 vector 拷贝进一个新的 vector，也可能不会。
/// let v_collected = v_clone.into_iter()
///                          .map(Some)
///                          .collect::<Vec<Option<&i32>>>();
///
/// let v_clone = v_orig.clone();
///
/// // 这是正确的、零拷贝、不依赖数据布局的 unsafe 方式来“transmute”一个 `Vec`。
/// // 我们不字面调用 `transmute`，而是做一次指针 cast；但就把原内部类型（`&i32`）转换成
/// // 新内部类型（`Option<&i32>`）这一点而言，它具有完全相同的注意事项。除了上面提供的信息，
/// // 还请查阅 [`from_raw_parts`] 的文档。
/// let (ptr, len, capacity) = v_clone.into_raw_parts();
/// let v_from_raw = unsafe {
///     Vec::from_raw_parts(ptr.cast::<*mut Option<&i32>>(), len, capacity)
/// };
/// ```
///
/// [`from_raw_parts`]: ../../std/vec/struct.Vec.html#method.from_raw_parts
///
/// 实现 `split_at_mut`：
///
/// ```
/// use std::{slice, mem};
///
/// // 有多种方式可以做到这一点，而下面这种（transmute）方式有多个问题。
/// fn split_at_mut_transmute<T>(slice: &mut [T], mid: usize)
///                              -> (&mut [T], &mut [T]) {
///     let len = slice.len();
///     assert!(mid <= len);
///     unsafe {
///         let slice2 = mem::transmute::<&mut [T], &mut [T]>(slice);
///         // 第一，transmute 不是类型安全的；它只检查 T 和 U 大小相同。
///         // 第二，就在这里，你有了两个指向同一块内存的可变引用。
///         (&mut slice[0..mid], &mut slice2[mid..len])
///     }
/// }
///
/// // 这样就消除了类型安全问题；`&mut *` *只*会从 `&mut T` 或 `*mut T` 给你一个 `&mut T`。
/// fn split_at_mut_casts<T>(slice: &mut [T], mid: usize)
///                          -> (&mut [T], &mut [T]) {
///     let len = slice.len();
///     assert!(mid <= len);
///     unsafe {
///         let slice2 = &mut *(slice as *mut [T]);
///         // 然而，你仍然有两个指向同一块内存的可变引用。
///         (&mut slice[0..mid], &mut slice2[mid..len])
///     }
/// }
///
/// // 这是标准库的做法。如果你需要做类似的事情，这是最佳方法。
/// fn split_at_stdlib<T>(slice: &mut [T], mid: usize)
///                       -> (&mut [T], &mut [T]) {
///     let len = slice.len();
///     assert!(mid <= len);
///     unsafe {
///         let ptr = slice.as_mut_ptr();
///         // 现在有三个可变引用指向同一块内存：`slice`、右值 ret.0、以及右值 ret.1。
///         // 在 `let ptr = ...` 之后 `slice` 就再也没被用过，因此可以把它视为“已死”，
///         // 于是你实际上只有两个真正的可变切片。
///         (slice::from_raw_parts_mut(ptr, mid),
///          slice::from_raw_parts_mut(ptr.add(mid), len - mid))
///     }
/// }
/// ```
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_allowed_through_unstable_modules = "import this function via `std::mem` instead"]
#[rustc_const_stable(feature = "const_transmute", since = "1.56.0")]
#[rustc_diagnostic_item = "transmute"]
#[rustc_nounwind]
#[rustc_intrinsic]
pub const unsafe fn transmute<Src, Dst>(src: Src) -> Dst;

/// 与 [`transmute`] 类似，但在编译期检查得更少：对于 `size_of::<Src>() != size_of::<Dst>()`，
/// 它不会给出编译错误，而是在运行时构成**未定义行为（UB）**。
///
/// 在可能的情况下，优先使用常规的 `transmute`，以获得那项额外检查；因为只要两者都能编译通过，
/// 它们在运行时做的事情是完全一样的。
///
/// 预计它永远不会直接暴露给用户，而是最终可能通过某种约束更强的 API 暴露出来。
#[rustc_intrinsic_const_stable_indirect]
#[rustc_nounwind]
#[rustc_intrinsic]
pub const unsafe fn transmute_unchecked<Src, Dst>(src: Src) -> Dst;

/// 如果作为 `T` 传入的实际类型需要 drop 胶水代码（析构），返回 `true`；
/// 如果为 `T` 提供的实际类型实现了 `Copy`，返回 `false`。
///
/// 如果实际类型既不需要 drop 胶水代码、又没有实现 `Copy`，那么本函数的返回值未规定。
///
/// 注意，与大多数 intrinsic 不同，它只能在编译期调用，因为各后端并没有为它提供实现。
/// 它唯一的调用方（即它的稳定对应物）会把这次 intrinsic 调用包进一个 `const` 块里，
/// 这样后端看到的就只是一个已经求值好的常量。
///
/// 本 intrinsic 的稳定版本是 [`mem::needs_drop`](crate::mem::needs_drop)。
#[rustc_intrinsic_const_stable_indirect]
#[rustc_nounwind]
#[rustc_intrinsic]
pub const fn needs_drop<T: ?Sized>() -> bool;

/// 从一个指针计算偏移。
///
/// 之所以实现成 intrinsic，是为了避免在指针与整数之间来回转换，因为那种转换会丢弃别名（aliasing）信息。
///
/// 它只能用于：`Ptr` 是指向 `Sized` 被指物的裸指针类型（`*mut` 或 `*const`），且 `Delta` 是
/// `usize` 或 `isize`。任何其他实例化都可能任意地行为异常，而那*并非*编译器 bug。
///
/// # 安全性（Safety）
///
/// 如果计算出的偏移非零，那么起始指针与结果指针都必须位于某个分配（allocation）的边界内、
/// 或恰好位于其末尾。如果任一指针越界、或发生算术溢出，则本操作是未定义行为（UB）。
///
/// 本 intrinsic 的稳定版本是 [`pointer::offset`]。
#[must_use = "returns a new pointer rather than modifying its argument"]
#[rustc_intrinsic_const_stable_indirect]
#[rustc_nounwind]
#[rustc_intrinsic]
pub const unsafe fn offset<Ptr: bounds::BuiltinDeref, Delta>(dst: Ptr, offset: Delta) -> Ptr;

/// 从一个指针计算偏移，可能发生回绕（wrapping）。
///
/// 之所以实现成 intrinsic，是为了避免在指针与整数之间来回转换，因为那种转换会妨碍某些优化。
///
/// # 安全性（Safety）
///
/// 与 `offset` intrinsic 不同，本 intrinsic 不要求结果指针指向某个已分配对象的边界内或末尾，
/// 并且它按二进制补码算术进行回绕。其结果值不一定能被合法地用于真正访问内存。
///
/// 本 intrinsic 的稳定版本是 [`pointer::wrapping_offset`]。
#[must_use = "returns a new pointer rather than modifying its argument"]
#[rustc_intrinsic_const_stable_indirect]
#[rustc_nounwind]
#[rustc_intrinsic]
pub const unsafe fn arith_offset<T>(dst: *const T, offset: isize) -> *const T;

/// 投影到 `slice_ptr` 的第 `index` 个元素，并保持与传入切片相同种类的指针——
/// 也就是 `&mut [T] → &mut T`、`&[T] → &T`、`*mut [T] → *mut T` 或 `*const [T] → *const T`——
/// 且不做边界检查。
///
/// 它通过 `<usize as SliceIndex>::get(_unchecked)(_mut)` 暴露，不打算在别处使用。
///
/// 在 MIR 中视所涉类型展开为 `{&, &mut, &raw const, &raw mut} (*slice_ptr)[index]`，因此无需后端支持。
///
/// # 安全性（Safety）
///
/// - `index < PtrMetadata(slice_ptr)`，从而对该切片的索引落在边界内；
/// - 由此产生的偏移落在分配（allocation）的边界内——对引用而言这总是成立的，但对指针则需手动保证。
#[rustc_nounwind]
#[rustc_intrinsic]
pub const unsafe fn slice_get_unchecked<
    ItemPtr: bounds::ChangePointee<[T], Pointee = T, Output = SlicePtr>,
    SlicePtr,
    T,
>(
    slice_ptr: SlicePtr,
    index: usize,
) -> ItemPtr;

/// 按掩码（mask）把指针的某些 bit 掩掉。
///
/// 注意，与大多数 intrinsic 不同，调用它是安全的；它不需要 `unsafe` 块。
/// 因此，实现绝不能要求用户去维护任何安全不变量。
///
/// 请考虑改用 [`pointer::mask`]。
#[rustc_nounwind]
#[rustc_intrinsic]
pub fn ptr_mask<T>(ptr: *const T, mask: usize) -> *const T;

/// 等价于对应的 `llvm.memcpy.p0i8.0i8.*` intrinsic，其大小为 `count` * `size_of::<T>()`、
/// 对齐为 `align_of::<T>()`。
///
/// 本 intrinsic 没有稳定的对应物。
/// # 安全性（Safety）
///
/// 其安全要求与 [`copy_nonoverlapping`] 一致，但读写行为是 volatile（易变）的，
/// 这意味着除非 `_count` 或 `size_of::<T>()` 等于零，否则它不会被优化掉。
///
/// [`copy_nonoverlapping`]: ptr::copy_nonoverlapping
#[rustc_intrinsic]
#[rustc_nounwind]
pub unsafe fn volatile_copy_nonoverlapping_memory<T>(dst: *mut T, src: *const T, count: usize);
/// 等价于对应的 `llvm.memmove.p0i8.0i8.*` intrinsic，其大小为 `count * size_of::<T>()`、
/// 对齐为 `align_of::<T>()`。
///
/// volatile 参数被设为 `true`，所以除非大小等于零，否则它不会被优化掉。
///
/// 本 intrinsic 没有稳定的对应物。
#[rustc_intrinsic]
#[rustc_nounwind]
pub unsafe fn volatile_copy_memory<T>(dst: *mut T, src: *const T, count: usize);
/// 等价于对应的 `llvm.memset.p0i8.*` intrinsic，其大小为 `count * size_of::<T>()`、
/// 对齐为 `align_of::<T>()`。
///
/// 本 intrinsic 没有稳定的对应物。
/// # 安全性（Safety）
///
/// 其安全要求与 [`write_bytes`] 一致，但写行为是 volatile（易变）的，
/// 这意味着除非 `_count` 或 `size_of::<T>()` 等于零，否则它不会被优化掉。
///
/// [`write_bytes`]: ptr::write_bytes
#[rustc_intrinsic]
#[rustc_nounwind]
pub unsafe fn volatile_set_memory<T>(dst: *mut T, val: u8, count: usize);

/// 从 `src` 指针处执行一次 volatile（易变）加载。
///
/// 本 intrinsic 的稳定版本是 [`core::ptr::read_volatile`]。
#[rustc_intrinsic]
#[rustc_nounwind]
pub unsafe fn volatile_load<T>(src: *const T) -> T;
/// 向 `dst` 指针处执行一次 volatile（易变）存储。
///
/// 本 intrinsic 的稳定版本是 [`core::ptr::write_volatile`]。
#[rustc_intrinsic]
#[rustc_nounwind]
pub unsafe fn volatile_store<T>(dst: *mut T, val: T);

/// 从 `src` 指针处执行一次 volatile（易变）加载。
/// 该指针不要求对齐。
///
/// 本 intrinsic 没有稳定的对应物。
#[rustc_intrinsic]
#[rustc_nounwind]
#[rustc_diagnostic_item = "intrinsics_unaligned_volatile_load"]
pub unsafe fn unaligned_volatile_load<T>(src: *const T) -> T;
/// 向 `dst` 指针处执行一次 volatile（易变）存储。
/// 该指针不要求对齐。
///
/// 本 intrinsic 没有稳定的对应物。
#[rustc_intrinsic]
#[rustc_nounwind]
#[rustc_diagnostic_item = "intrinsics_unaligned_volatile_store"]
pub unsafe fn unaligned_volatile_store<T>(dst: *mut T, val: T);

/// 返回一个 `f16` 的平方根。
///
/// 本 intrinsic 的稳定版本是
/// [`f16::sqrt`](../../std/primitive.f16.html#method.sqrt)
#[rustc_intrinsic]
#[rustc_nounwind]
pub fn sqrtf16(x: f16) -> f16;
/// 返回一个 `f32` 的平方根。
///
/// 本 intrinsic 的稳定版本是
/// [`f32::sqrt`](../../std/primitive.f32.html#method.sqrt)
#[rustc_intrinsic]
#[rustc_nounwind]
pub fn sqrtf32(x: f32) -> f32;
/// 返回一个 `f64` 的平方根。
///
/// 本 intrinsic 的稳定版本是
/// [`f64::sqrt`](../../std/primitive.f64.html#method.sqrt)
#[rustc_intrinsic]
#[rustc_nounwind]
pub fn sqrtf64(x: f64) -> f64;
/// 返回一个 `f128` 的平方根。
///
/// 本 intrinsic 的稳定版本是
/// [`f128::sqrt`](../../std/primitive.f128.html#method.sqrt)
#[rustc_intrinsic]
#[rustc_nounwind]
pub fn sqrtf128(x: f128) -> f128;

/// 把一个 `f16` 提升到整数次幂。
///
/// 本 intrinsic 的稳定版本是
/// [`f16::powi`](../../std/primitive.f16.html#method.powi)
#[rustc_intrinsic]
#[rustc_nounwind]
pub fn powif16(a: f16, x: i32) -> f16;
/// 把一个 `f32` 提升到整数次幂。
///
/// 本 intrinsic 的稳定版本是
/// [`f32::powi`](../../std/primitive.f32.html#method.powi)
#[rustc_intrinsic]
#[rustc_nounwind]
pub fn powif32(a: f32, x: i32) -> f32;
/// 把一个 `f64` 提升到整数次幂。
///
/// 本 intrinsic 的稳定版本是
/// [`f64::powi`](../../std/primitive.f64.html#method.powi)
#[rustc_intrinsic]
#[rustc_nounwind]
pub fn powif64(a: f64, x: i32) -> f64;
/// 把一个 `f128` 提升到整数次幂。
///
/// 本 intrinsic 的稳定版本是
/// [`f128::powi`](../../std/primitive.f128.html#method.powi)
#[rustc_intrinsic]
#[rustc_nounwind]
pub fn powif128(a: f128, x: i32) -> f128;

/// 返回一个 `f16` 的正弦。
///
/// 本 intrinsic 的稳定版本是
/// [`f16::sin`](../../std/primitive.f16.html#method.sin)
#[rustc_intrinsic]
#[rustc_nounwind]
pub fn sinf16(x: f16) -> f16;
/// 返回一个 `f32` 的正弦。
///
/// 本 intrinsic 的稳定版本是
/// [`f32::sin`](../../std/primitive.f32.html#method.sin)
#[rustc_intrinsic]
#[rustc_nounwind]
pub fn sinf32(x: f32) -> f32;
/// 返回一个 `f64` 的正弦。
///
/// 本 intrinsic 的稳定版本是
/// [`f64::sin`](../../std/primitive.f64.html#method.sin)
#[rustc_intrinsic]
#[rustc_nounwind]
pub fn sinf64(x: f64) -> f64;
/// 返回一个 `f128` 的正弦。
///
/// 本 intrinsic 的稳定版本是
/// [`f128::sin`](../../std/primitive.f128.html#method.sin)
#[rustc_intrinsic]
#[rustc_nounwind]
pub fn sinf128(x: f128) -> f128;

/// 返回一个 `f16` 的余弦。
///
/// 本 intrinsic 的稳定版本是
/// [`f16::cos`](../../std/primitive.f16.html#method.cos)
#[rustc_intrinsic]
#[rustc_nounwind]
pub fn cosf16(x: f16) -> f16;
/// 返回一个 `f32` 的余弦。
///
/// 本 intrinsic 的稳定版本是
/// [`f32::cos`](../../std/primitive.f32.html#method.cos)
#[rustc_intrinsic]
#[rustc_nounwind]
pub fn cosf32(x: f32) -> f32;
/// 返回一个 `f64` 的余弦。
///
/// 本 intrinsic 的稳定版本是
/// [`f64::cos`](../../std/primitive.f64.html#method.cos)
#[rustc_intrinsic]
#[rustc_nounwind]
pub fn cosf64(x: f64) -> f64;
/// 返回一个 `f128` 的余弦。
///
/// 本 intrinsic 的稳定版本是
/// [`f128::cos`](../../std/primitive.f128.html#method.cos)
#[rustc_intrinsic]
#[rustc_nounwind]
pub fn cosf128(x: f128) -> f128;

/// 把一个 `f16` 提升到 `f16` 次幂。
///
/// 本 intrinsic 的稳定版本是
/// [`f16::powf`](../../std/primitive.f16.html#method.powf)
#[rustc_intrinsic]
#[rustc_nounwind]
pub fn powf16(a: f16, x: f16) -> f16;
/// 把一个 `f32` 提升到 `f32` 次幂。
///
/// 本 intrinsic 的稳定版本是
/// [`f32::powf`](../../std/primitive.f32.html#method.powf)
#[rustc_intrinsic]
#[rustc_nounwind]
pub fn powf32(a: f32, x: f32) -> f32;
/// 把一个 `f64` 提升到 `f64` 次幂。
///
/// 本 intrinsic 的稳定版本是
/// [`f64::powf`](../../std/primitive.f64.html#method.powf)
#[rustc_intrinsic]
#[rustc_nounwind]
pub fn powf64(a: f64, x: f64) -> f64;
/// 把一个 `f128` 提升到 `f128` 次幂。
///
/// 本 intrinsic 的稳定版本是
/// [`f128::powf`](../../std/primitive.f128.html#method.powf)
#[rustc_intrinsic]
#[rustc_nounwind]
pub fn powf128(a: f128, x: f128) -> f128;

/// 返回一个 `f16` 的指数（e 的幂）。
///
/// 本 intrinsic 的稳定版本是
/// [`f16::exp`](../../std/primitive.f16.html#method.exp)
#[rustc_intrinsic]
#[rustc_nounwind]
pub fn expf16(x: f16) -> f16;
/// 返回一个 `f32` 的指数（e 的幂）。
///
/// 本 intrinsic 的稳定版本是
/// [`f32::exp`](../../std/primitive.f32.html#method.exp)
#[rustc_intrinsic]
#[rustc_nounwind]
pub fn expf32(x: f32) -> f32;
/// 返回一个 `f64` 的指数（e 的幂）。
///
/// 本 intrinsic 的稳定版本是
/// [`f64::exp`](../../std/primitive.f64.html#method.exp)
#[rustc_intrinsic]
#[rustc_nounwind]
pub fn expf64(x: f64) -> f64;
/// 返回一个 `f128` 的指数（e 的幂）。
///
/// 本 intrinsic 的稳定版本是
/// [`f128::exp`](../../std/primitive.f128.html#method.exp)
#[rustc_intrinsic]
#[rustc_nounwind]
pub fn expf128(x: f128) -> f128;

/// 返回 2 的 `f16` 次幂。
///
/// 本 intrinsic 的稳定版本是
/// [`f16::exp2`](../../std/primitive.f16.html#method.exp2)
#[rustc_intrinsic]
#[rustc_nounwind]
pub fn exp2f16(x: f16) -> f16;
/// 返回 2 的 `f32` 次幂。
///
/// 本 intrinsic 的稳定版本是
/// [`f32::exp2`](../../std/primitive.f32.html#method.exp2)
#[rustc_intrinsic]
#[rustc_nounwind]
pub fn exp2f32(x: f32) -> f32;
/// 返回 2 的 `f64` 次幂。
///
/// 本 intrinsic 的稳定版本是
/// [`f64::exp2`](../../std/primitive.f64.html#method.exp2)
#[rustc_intrinsic]
#[rustc_nounwind]
pub fn exp2f64(x: f64) -> f64;
/// 返回 2 的 `f128` 次幂。
///
/// 本 intrinsic 的稳定版本是
/// [`f128::exp2`](../../std/primitive.f128.html#method.exp2)
#[rustc_intrinsic]
#[rustc_nounwind]
pub fn exp2f128(x: f128) -> f128;

/// 返回一个浮点值的自然对数（以 e 为底）。
///
/// 本 intrinsic 的稳定版本是
/// [`f16::ln`](../../std/primitive.f16.html#method.ln)
#[rustc_intrinsic]
#[rustc_nounwind]
pub fn logf16(x: f16) -> f16;
/// 返回一个浮点值的自然对数（以 e 为底）。
///
/// 本 intrinsic 的稳定版本是
/// [`f32::ln`](../../std/primitive.f32.html#method.ln)
#[rustc_intrinsic]
#[rustc_nounwind]
pub fn logf32(x: f32) -> f32;
/// 返回一个浮点值的自然对数（以 e 为底）。
///
/// 本 intrinsic 的稳定版本是
/// [`f64::ln`](../../std/primitive.f64.html#method.ln)
#[rustc_intrinsic]
#[rustc_nounwind]
pub fn logf64(x: f64) -> f64;
/// 返回一个浮点值的自然对数（以 e 为底）。
///
/// 本 intrinsic 的稳定版本是
/// [`f128::ln`](../../std/primitive.f128.html#method.ln)
#[rustc_intrinsic]
#[rustc_nounwind]
pub fn logf128(x: f128) -> f128;

/// 返回一个 `f16` 以 10 为底的对数。
///
/// 本 intrinsic 的稳定版本是
/// [`f16::log10`](../../std/primitive.f16.html#method.log10)
#[rustc_intrinsic]
#[rustc_nounwind]
pub fn log10f16(x: f16) -> f16;
/// 返回一个 `f32` 以 10 为底的对数。
///
/// 本 intrinsic 的稳定版本是
/// [`f32::log10`](../../std/primitive.f32.html#method.log10)
#[rustc_intrinsic]
#[rustc_nounwind]
pub fn log10f32(x: f32) -> f32;
/// 返回一个 `f64` 以 10 为底的对数。
///
/// 本 intrinsic 的稳定版本是
/// [`f64::log10`](../../std/primitive.f64.html#method.log10)
#[rustc_intrinsic]
#[rustc_nounwind]
pub fn log10f64(x: f64) -> f64;
/// 返回一个 `f128` 以 10 为底的对数。
///
/// 本 intrinsic 的稳定版本是
/// [`f128::log10`](../../std/primitive.f128.html#method.log10)
#[rustc_intrinsic]
#[rustc_nounwind]
pub fn log10f128(x: f128) -> f128;

/// 返回一个 `f16` 以 2 为底的对数。
///
/// 本 intrinsic 的稳定版本是
/// [`f16::log2`](../../std/primitive.f16.html#method.log2)
#[rustc_intrinsic]
#[rustc_nounwind]
pub fn log2f16(x: f16) -> f16;
/// 返回一个 `f32` 以 2 为底的对数。
///
/// 本 intrinsic 的稳定版本是
/// [`f32::log2`](../../std/primitive.f32.html#method.log2)
#[rustc_intrinsic]
#[rustc_nounwind]
pub fn log2f32(x: f32) -> f32;
/// 返回一个 `f64` 以 2 为底的对数。
///
/// 本 intrinsic 的稳定版本是
/// [`f64::log2`](../../std/primitive.f64.html#method.log2)
#[rustc_intrinsic]
#[rustc_nounwind]
pub fn log2f64(x: f64) -> f64;
/// 返回一个 `f128` 以 2 为底的对数。
///
/// 本 intrinsic 的稳定版本是
/// [`f128::log2`](../../std/primitive.f128.html#method.log2)
#[rustc_intrinsic]
#[rustc_nounwind]
pub fn log2f128(x: f128) -> f128;

/// 返回 `f16` 值的 `a * b + c`。
///
/// 本 intrinsic 的稳定版本是
/// [`f16::mul_add`](../../std/primitive.f16.html#method.mul_add)
#[rustc_intrinsic_const_stable_indirect]
#[rustc_intrinsic]
#[rustc_nounwind]
pub const fn fmaf16(a: f16, b: f16, c: f16) -> f16;
/// 返回 `f32` 值的 `a * b + c`。
///
/// 本 intrinsic 的稳定版本是
/// [`f32::mul_add`](../../std/primitive.f32.html#method.mul_add)
#[rustc_intrinsic_const_stable_indirect]
#[rustc_intrinsic]
#[rustc_nounwind]
pub const fn fmaf32(a: f32, b: f32, c: f32) -> f32;
/// 返回 `f64` 值的 `a * b + c`。
///
/// 本 intrinsic 的稳定版本是
/// [`f64::mul_add`](../../std/primitive.f64.html#method.mul_add)
#[rustc_intrinsic_const_stable_indirect]
#[rustc_intrinsic]
#[rustc_nounwind]
pub const fn fmaf64(a: f64, b: f64, c: f64) -> f64;
/// 返回 `f128` 值的 `a * b + c`。
///
/// 本 intrinsic 的稳定版本是
/// [`f128::mul_add`](../../std/primitive.f128.html#method.mul_add)
#[rustc_intrinsic_const_stable_indirect]
#[rustc_intrinsic]
#[rustc_nounwind]
pub const fn fmaf128(a: f128, b: f128, c: f128) -> f128;

/// 返回 `f16` 值的 `a * b + c`，
/// 以非确定性的方式，要么执行融合乘加（fused multiply-add），要么执行两步运算并对中间结果做舍入。
///
/// 当代码生成器判定目标指令集支持融合操作、且融合操作比等价的“分开的乘法 + 加法”两条指令更高效时，
/// 才会做融合。是否选择融合操作并未被规定，且可能取决于优化级别、上下文等因素。
#[rustc_intrinsic]
#[rustc_nounwind]
pub const fn fmuladdf16(a: f16, b: f16, c: f16) -> f16;
/// 返回 `f32` 值的 `a * b + c`，
/// 以非确定性的方式，要么执行融合乘加（fused multiply-add），要么执行两步运算并对中间结果做舍入。
///
/// 当代码生成器判定目标指令集支持融合操作、且融合操作比等价的“分开的乘法 + 加法”两条指令更高效时，
/// 才会做融合。是否选择融合操作并未被规定，且可能取决于优化级别、上下文等因素。
#[rustc_intrinsic]
#[rustc_nounwind]
pub const fn fmuladdf32(a: f32, b: f32, c: f32) -> f32;
/// 返回 `f64` 值的 `a * b + c`，
/// 以非确定性的方式，要么执行融合乘加（fused multiply-add），要么执行两步运算并对中间结果做舍入。
///
/// 当代码生成器判定目标指令集支持融合操作、且融合操作比等价的“分开的乘法 + 加法”两条指令更高效时，
/// 才会做融合。是否选择融合操作并未被规定，且可能取决于优化级别、上下文等因素。
#[rustc_intrinsic]
#[rustc_nounwind]
pub const fn fmuladdf64(a: f64, b: f64, c: f64) -> f64;
/// 返回 `f128` 值的 `a * b + c`，
/// 以非确定性的方式，要么执行融合乘加（fused multiply-add），要么执行两步运算并对中间结果做舍入。
///
/// 当代码生成器判定目标指令集支持融合操作、且融合操作比等价的“分开的乘法 + 加法”两条指令更高效时，
/// 才会做融合。是否选择融合操作并未被规定，且可能取决于优化级别、上下文等因素。
#[rustc_intrinsic]
#[rustc_nounwind]
pub const fn fmuladdf128(a: f128, b: f128, c: f128) -> f128;

/// 返回不大于该 `f16` 的最大整数（向下取整 floor）。
///
/// 本 intrinsic 的稳定版本是
/// [`f16::floor`](../../std/primitive.f16.html#method.floor)
#[rustc_intrinsic_const_stable_indirect]
#[rustc_intrinsic]
#[rustc_nounwind]
pub const fn floorf16(x: f16) -> f16;
/// 返回不大于该 `f32` 的最大整数（向下取整 floor）。
///
/// 本 intrinsic 的稳定版本是
/// [`f32::floor`](../../std/primitive.f32.html#method.floor)
#[rustc_intrinsic_const_stable_indirect]
#[rustc_intrinsic]
#[rustc_nounwind]
pub const fn floorf32(x: f32) -> f32;
/// 返回不大于该 `f64` 的最大整数（向下取整 floor）。
///
/// 本 intrinsic 的稳定版本是
/// [`f64::floor`](../../std/primitive.f64.html#method.floor)
#[rustc_intrinsic_const_stable_indirect]
#[rustc_intrinsic]
#[rustc_nounwind]
pub const fn floorf64(x: f64) -> f64;
/// 返回不大于该 `f128` 的最大整数（向下取整 floor）。
///
/// 本 intrinsic 的稳定版本是
/// [`f128::floor`](../../std/primitive.f128.html#method.floor)
#[rustc_intrinsic_const_stable_indirect]
#[rustc_intrinsic]
#[rustc_nounwind]
pub const fn floorf128(x: f128) -> f128;

/// 返回不小于该 `f16` 的最小整数（向上取整 ceil）。
///
/// 本 intrinsic 的稳定版本是
/// [`f16::ceil`](../../std/primitive.f16.html#method.ceil)
#[rustc_intrinsic_const_stable_indirect]
#[rustc_intrinsic]
#[rustc_nounwind]
pub const fn ceilf16(x: f16) -> f16;
/// 返回不小于该 `f32` 的最小整数（向上取整 ceil）。
///
/// 本 intrinsic 的稳定版本是
/// [`f32::ceil`](../../std/primitive.f32.html#method.ceil)
#[rustc_intrinsic_const_stable_indirect]
#[rustc_intrinsic]
#[rustc_nounwind]
pub const fn ceilf32(x: f32) -> f32;
/// 返回不小于该 `f64` 的最小整数（向上取整 ceil）。
///
/// 本 intrinsic 的稳定版本是
/// [`f64::ceil`](../../std/primitive.f64.html#method.ceil)
#[rustc_intrinsic_const_stable_indirect]
#[rustc_intrinsic]
#[rustc_nounwind]
pub const fn ceilf64(x: f64) -> f64;
/// 返回不小于该 `f128` 的最小整数（向上取整 ceil）。
///
/// 本 intrinsic 的稳定版本是
/// [`f128::ceil`](../../std/primitive.f128.html#method.ceil)
#[rustc_intrinsic_const_stable_indirect]
#[rustc_intrinsic]
#[rustc_nounwind]
pub const fn ceilf128(x: f128) -> f128;

/// 返回一个 `f16` 的整数部分（向零截断 trunc）。
///
/// 本 intrinsic 的稳定版本是
/// [`f16::trunc`](../../std/primitive.f16.html#method.trunc)
#[rustc_intrinsic_const_stable_indirect]
#[rustc_intrinsic]
#[rustc_nounwind]
pub const fn truncf16(x: f16) -> f16;
/// 返回一个 `f32` 的整数部分（向零截断 trunc）。
///
/// 本 intrinsic 的稳定版本是
/// [`f32::trunc`](../../std/primitive.f32.html#method.trunc)
#[rustc_intrinsic_const_stable_indirect]
#[rustc_intrinsic]
#[rustc_nounwind]
pub const fn truncf32(x: f32) -> f32;
/// 返回一个 `f64` 的整数部分（向零截断 trunc）。
///
/// 本 intrinsic 的稳定版本是
/// [`f64::trunc`](../../std/primitive.f64.html#method.trunc)
#[rustc_intrinsic_const_stable_indirect]
#[rustc_intrinsic]
#[rustc_nounwind]
pub const fn truncf64(x: f64) -> f64;
/// 返回一个 `f128` 的整数部分（向零截断 trunc）。
///
/// 本 intrinsic 的稳定版本是
/// [`f128::trunc`](../../std/primitive.f128.html#method.trunc)
#[rustc_intrinsic_const_stable_indirect]
#[rustc_intrinsic]
#[rustc_nounwind]
pub const fn truncf128(x: f128) -> f128;

/// 返回最接近 `f16` 的整数。平局时舍入到最低有效位为偶数的那个数。
///
/// 本 intrinsic 的稳定版本是
/// [`f16::round_ties_even`](../../std/primitive.f16.html#method.round_ties_even)
#[rustc_intrinsic_const_stable_indirect]
#[rustc_intrinsic]
#[rustc_nounwind]
pub const fn round_ties_even_f16(x: f16) -> f16;

/// 返回最接近 `f32` 的整数。平局时舍入到最低有效位为偶数的那个数。
///
/// 本 intrinsic 的稳定版本是
/// [`f32::round_ties_even`](../../std/primitive.f32.html#method.round_ties_even)
#[rustc_intrinsic_const_stable_indirect]
#[rustc_intrinsic]
#[rustc_nounwind]
pub const fn round_ties_even_f32(x: f32) -> f32;

/// 返回最接近 `f64` 的整数。平局时舍入到最低有效位为偶数的那个数。
///
/// 本 intrinsic 的稳定版本是
/// [`f64::round_ties_even`](../../std/primitive.f64.html#method.round_ties_even)
#[rustc_intrinsic_const_stable_indirect]
#[rustc_intrinsic]
#[rustc_nounwind]
pub const fn round_ties_even_f64(x: f64) -> f64;

/// 返回最接近 `f128` 的整数。平局时舍入到最低有效位为偶数的那个数。
///
/// 本 intrinsic 的稳定版本是
/// [`f128::round_ties_even`](../../std/primitive.f128.html#method.round_ties_even)
#[rustc_intrinsic_const_stable_indirect]
#[rustc_intrinsic]
#[rustc_nounwind]
pub const fn round_ties_even_f128(x: f128) -> f128;

/// 返回最接近 `f16` 的整数。平局时向远离 0 的方向舍入。
///
/// 本 intrinsic 的稳定版本是
/// [`f16::round`](../../std/primitive.f16.html#method.round)
#[rustc_intrinsic_const_stable_indirect]
#[rustc_intrinsic]
#[rustc_nounwind]
pub const fn roundf16(x: f16) -> f16;
/// 返回最接近 `f32` 的整数。平局时向远离 0 的方向舍入。
///
/// 本 intrinsic 的稳定版本是
/// [`f32::round`](../../std/primitive.f32.html#method.round)
#[rustc_intrinsic_const_stable_indirect]
#[rustc_intrinsic]
#[rustc_nounwind]
pub const fn roundf32(x: f32) -> f32;
/// 返回最接近 `f64` 的整数。平局时向远离 0 的方向舍入。
///
/// 本 intrinsic 的稳定版本是
/// [`f64::round`](../../std/primitive.f64.html#method.round)
#[rustc_intrinsic_const_stable_indirect]
#[rustc_intrinsic]
#[rustc_nounwind]
pub const fn roundf64(x: f64) -> f64;
/// 返回最接近 `f128` 的整数。平局时向远离 0 的方向舍入。
///
/// 本 intrinsic 的稳定版本是
/// [`f128::round`](../../std/primitive.f128.html#method.round)
#[rustc_intrinsic_const_stable_indirect]
#[rustc_intrinsic]
#[rustc_nounwind]
pub const fn roundf128(x: f128) -> f128;

/// 允许基于代数规则进行优化的浮点加法。
/// 要求该操作的输入与输出均为有限值，否则即 UB。
///
/// 本 intrinsic 没有稳定的对应物。
#[rustc_intrinsic]
#[rustc_nounwind]
pub unsafe fn fadd_fast<T: Copy>(a: T, b: T) -> T;

/// 允许基于代数规则进行优化的浮点减法。
/// 要求该操作的输入与输出均为有限值，否则即 UB。
///
/// 本 intrinsic 没有稳定的对应物。
#[rustc_intrinsic]
#[rustc_nounwind]
pub unsafe fn fsub_fast<T: Copy>(a: T, b: T) -> T;

/// 允许基于代数规则进行优化的浮点乘法。
/// 要求该操作的输入与输出均为有限值，否则即 UB。
///
/// 本 intrinsic 没有稳定的对应物。
#[rustc_intrinsic]
#[rustc_nounwind]
pub unsafe fn fmul_fast<T: Copy>(a: T, b: T) -> T;

/// 允许基于代数规则进行优化的浮点除法。
/// 要求该操作的输入与输出均为有限值，否则即 UB。
///
/// 本 intrinsic 没有稳定的对应物。
#[rustc_intrinsic]
#[rustc_nounwind]
pub unsafe fn fdiv_fast<T: Copy>(a: T, b: T) -> T;

/// 允许基于代数规则进行优化的浮点取余。
/// 要求该操作的输入与输出均为有限值，否则即 UB。
///
/// 本 intrinsic 没有稳定的对应物。
#[rustc_intrinsic]
#[rustc_nounwind]
pub unsafe fn frem_fast<T: Copy>(a: T, b: T) -> T;

/// 使用 LLVM 的 fptoui/fptosi 进行转换；对越界的值它可能返回 undef
/// （<https://github.com/rust-lang/rust/issues/10184>）。
///
/// 稳定版本为 [`f32::to_int_unchecked`] 与 [`f64::to_int_unchecked`]。
#[rustc_intrinsic]
#[rustc_nounwind]
pub unsafe fn float_to_int_unchecked<Float: Copy, Int: Copy>(value: Float) -> Int;

/// 允许基于代数规则进行优化的浮点加法。
///
/// 稳定版本为 [`f16::algebraic_add`]、[`f32::algebraic_add`]、[`f64::algebraic_add`] 与 [`f128::algebraic_add`]。
#[rustc_nounwind]
#[rustc_intrinsic]
pub const fn fadd_algebraic<T: Copy>(a: T, b: T) -> T;

/// 允许基于代数规则进行优化的浮点减法。
///
/// 稳定版本为 [`f16::algebraic_sub`]、[`f32::algebraic_sub`]、[`f64::algebraic_sub`] 与 [`f128::algebraic_sub`]。
#[rustc_nounwind]
#[rustc_intrinsic]
pub const fn fsub_algebraic<T: Copy>(a: T, b: T) -> T;

/// 允许基于代数规则进行优化的浮点乘法。
///
/// 稳定版本为 [`f16::algebraic_mul`]、[`f32::algebraic_mul`]、[`f64::algebraic_mul`] 与 [`f128::algebraic_mul`]。
#[rustc_nounwind]
#[rustc_intrinsic]
pub const fn fmul_algebraic<T: Copy>(a: T, b: T) -> T;

/// 允许基于代数规则进行优化的浮点除法。
///
/// 稳定版本为 [`f16::algebraic_div`]、[`f32::algebraic_div`]、[`f64::algebraic_div`] 与 [`f128::algebraic_div`]。
#[rustc_nounwind]
#[rustc_intrinsic]
pub const fn fdiv_algebraic<T: Copy>(a: T, b: T) -> T;

/// 允许基于代数规则进行优化的浮点取余。
///
/// 稳定版本为 [`f16::algebraic_rem`]、[`f32::algebraic_rem`]、[`f64::algebraic_rem`] 与 [`f128::algebraic_rem`]。
#[rustc_nounwind]
#[rustc_intrinsic]
pub const fn frem_algebraic<T: Copy>(a: T, b: T) -> T;

/// 返回整数类型 `T` 中被置 1 的位的个数。
///
/// 注意，与大多数 intrinsic 不同，调用它是安全的；它不需要 `unsafe` 块。
/// 因此，实现绝不能要求用户去维护任何安全不变量。
///
/// 本 intrinsic 的稳定版本可通过整数原始类型上的 `count_ones` 方法使用，例如
/// [`u32::count_ones`]
#[rustc_intrinsic_const_stable_indirect]
#[rustc_nounwind]
#[rustc_intrinsic]
pub const fn ctpop<T: Copy>(x: T) -> u32;

/// 返回整数类型 `T` 中前导未置位（即前导零）的个数。
///
/// 注意，与大多数 intrinsic 不同，调用它是安全的；它不需要 `unsafe` 块。
/// 因此，实现绝不能要求用户去维护任何安全不变量。
///
/// 本 intrinsic 的稳定版本可通过整数原始类型上的 `leading_zeros` 方法使用，例如
/// [`u32::leading_zeros`]
///
/// # 示例
///
/// ```
/// #![feature(core_intrinsics)]
/// # #![allow(internal_features)]
///
/// use std::intrinsics::ctlz;
///
/// let x = 0b0001_1100_u8;
/// let num_leading = ctlz(x);
/// assert_eq!(num_leading, 3);
/// ```
///
/// 值为 `0` 的 `x` 会返回 `T` 的位宽。
///
/// ```
/// #![feature(core_intrinsics)]
/// # #![allow(internal_features)]
///
/// use std::intrinsics::ctlz;
///
/// let x = 0u16;
/// let num_leading = ctlz(x);
/// assert_eq!(num_leading, 16);
/// ```
#[rustc_intrinsic_const_stable_indirect]
#[rustc_nounwind]
#[rustc_intrinsic]
pub const fn ctlz<T: Copy>(x: T) -> u32;

/// 与 `ctlz` 类似，但格外不安全：当传入值为 `0` 的 `x` 时它返回 `undef`。
///
/// 本 intrinsic 没有稳定的对应物。
///
/// # 示例
///
/// ```
/// #![feature(core_intrinsics)]
/// # #![allow(internal_features)]
///
/// use std::intrinsics::ctlz_nonzero;
///
/// let x = 0b0001_1100_u8;
/// let num_leading = unsafe { ctlz_nonzero(x) };
/// assert_eq!(num_leading, 3);
/// ```
#[rustc_intrinsic_const_stable_indirect]
#[rustc_nounwind]
#[rustc_intrinsic]
pub const unsafe fn ctlz_nonzero<T: Copy>(x: T) -> u32;

/// 返回整数类型 `T` 中尾随未置位（即尾随零）的个数。
///
/// 注意，与大多数 intrinsic 不同，调用它是安全的；它不需要 `unsafe` 块。
/// 因此，实现绝不能要求用户去维护任何安全不变量。
///
/// 本 intrinsic 的稳定版本可通过整数原始类型上的 `trailing_zeros` 方法使用，例如
/// [`u32::trailing_zeros`]
///
/// # 示例
///
/// ```
/// #![feature(core_intrinsics)]
/// # #![allow(internal_features)]
///
/// use std::intrinsics::cttz;
///
/// let x = 0b0011_1000_u8;
/// let num_trailing = cttz(x);
/// assert_eq!(num_trailing, 3);
/// ```
///
/// 值为 `0` 的 `x` 会返回 `T` 的位宽：
///
/// ```
/// #![feature(core_intrinsics)]
/// # #![allow(internal_features)]
///
/// use std::intrinsics::cttz;
///
/// let x = 0u16;
/// let num_trailing = cttz(x);
/// assert_eq!(num_trailing, 16);
/// ```
#[rustc_intrinsic_const_stable_indirect]
#[rustc_nounwind]
#[rustc_intrinsic]
pub const fn cttz<T: Copy>(x: T) -> u32;

/// 与 `cttz` 类似，但格外不安全：当传入值为 `0` 的 `x` 时它返回 `undef`。
///
/// 本 intrinsic 没有稳定的对应物。
///
/// # 示例
///
/// ```
/// #![feature(core_intrinsics)]
/// # #![allow(internal_features)]
///
/// use std::intrinsics::cttz_nonzero;
///
/// let x = 0b0011_1000_u8;
/// let num_trailing = unsafe { cttz_nonzero(x) };
/// assert_eq!(num_trailing, 3);
/// ```
#[rustc_intrinsic_const_stable_indirect]
#[rustc_nounwind]
#[rustc_intrinsic]
pub const unsafe fn cttz_nonzero<T: Copy>(x: T) -> u32;

/// 反转整数类型 `T` 中的字节顺序。
///
/// 注意，与大多数 intrinsic 不同，调用它是安全的；它不需要 `unsafe` 块。
/// 因此，实现绝不能要求用户去维护任何安全不变量。
///
/// 本 intrinsic 的稳定版本可通过整数原始类型上的 `swap_bytes` 方法使用，例如
/// [`u32::swap_bytes`]
#[rustc_intrinsic_const_stable_indirect]
#[rustc_nounwind]
#[rustc_intrinsic]
pub const fn bswap<T: Copy>(x: T) -> T;

/// 反转整数类型 `T` 中的比特位。
///
/// 注意，与大多数 intrinsic 不同，调用它是安全的；它不需要 `unsafe` 块。
/// 因此，实现绝不能要求用户去维护任何安全不变量。
///
/// 本 intrinsic 的稳定版本可通过整数原始类型上的 `reverse_bits` 方法使用，例如
/// [`u32::reverse_bits`]
#[rustc_intrinsic_const_stable_indirect]
#[rustc_nounwind]
#[rustc_intrinsic]
pub const fn bitreverse<T: Copy>(x: T) -> T;

/// 对两个参数做三路比较（three-way comparison）；这两个参数必须是字符或整数（有符号或无符号）类型。
///
/// 它最初被加入是因为它极大地简化了 `cmp` 各实现中的 MIR；后来 LLVM 20 也为它增加了一个后端 intrinsic。
///
/// 本 intrinsic 的稳定版本是 [`Ord::cmp`]。
#[rustc_intrinsic_const_stable_indirect]
#[rustc_nounwind]
#[rustc_intrinsic]
pub const fn three_way_compare<T: Copy>(lhs: T, rhss: T) -> crate::cmp::Ordering;

/// 合并两个没有任何公共置位 bit 的值。
///
/// 这允许后端把它实现为 `a + b` *或* `a | b`，取决于在特定目标上哪种更易实现。
///
/// # 安全性（Safety）
///
/// 要求 `(a & b) == 0`，或等价地要求 `(a | b) == (a + b)`。
///
/// 否则即为立即 UB。
#[rustc_const_unstable(feature = "disjoint_bitor", issue = "135758")]
#[rustc_nounwind]
#[rustc_intrinsic]
#[track_caller]
#[miri::intrinsic_fallback_is_spec] // 各 fallback 都用 `assume` 来告知 Miri
pub const unsafe fn disjoint_bitor<T: [const] fallback::DisjointBitOr>(a: T, b: T) -> T {
    // SAFETY: 与本函数的前置条件相同。
    unsafe { fallback::DisjointBitOr::disjoint_bitor(a, b) }
}

/// 执行带溢出检查的整数加法。
///
/// 注意，与大多数 intrinsic 不同，调用它是安全的；它不需要 `unsafe` 块。
/// 因此，实现绝不能要求用户去维护任何安全不变量。
///
/// 本 intrinsic 的稳定版本可通过整数原始类型上的 `overflowing_add` 方法使用，例如
/// [`u32::overflowing_add`]
#[rustc_intrinsic_const_stable_indirect]
#[rustc_nounwind]
#[rustc_intrinsic]
pub const fn add_with_overflow<T: Copy>(x: T, y: T) -> (T, bool);

/// 执行带溢出检查的整数减法。
///
/// 注意，与大多数 intrinsic 不同，调用它是安全的；它不需要 `unsafe` 块。
/// 因此，实现绝不能要求用户去维护任何安全不变量。
///
/// 本 intrinsic 的稳定版本可通过整数原始类型上的 `overflowing_sub` 方法使用，例如
/// [`u32::overflowing_sub`]
#[rustc_intrinsic_const_stable_indirect]
#[rustc_nounwind]
#[rustc_intrinsic]
pub const fn sub_with_overflow<T: Copy>(x: T, y: T) -> (T, bool);

/// 执行带溢出检查的整数乘法。
///
/// 注意，与大多数 intrinsic 不同，调用它是安全的；它不需要 `unsafe` 块。
/// 因此，实现绝不能要求用户去维护任何安全不变量。
///
/// 本 intrinsic 的稳定版本可通过整数原始类型上的 `overflowing_mul` 方法使用，例如
/// [`u32::overflowing_mul`]
#[rustc_intrinsic_const_stable_indirect]
#[rustc_nounwind]
#[rustc_intrinsic]
pub const fn mul_with_overflow<T: Copy>(x: T, y: T) -> (T, bool);

/// 执行全宽（full-width）的乘加并带进位（carry）：
/// `multiplier * multiplicand + addend + carry`。
///
/// 这能在不发生任何溢出的情况下完成。对 `uN`：
///    MAX * MAX + MAX + MAX
/// => (2ⁿ-1) × (2ⁿ-1) + (2ⁿ-1) + (2ⁿ-1)
/// => (2²ⁿ - 2ⁿ⁺¹ + 1) + (2ⁿ⁺¹ - 2)
/// => 2²ⁿ - 1
///
/// 对 `iN`，上界为 MIN * MIN + MAX + MAX => 2²ⁿ⁻² + 2ⁿ - 2，
/// 下界为 MAX * MIN + MIN + MIN => -2²ⁿ⁻² - 2ⁿ + 2ⁿ⁺¹。
///
/// 目前*仅*支持无符号整数，不支持有符号整数。
/// 本 intrinsic 的稳定版本可在各整数类型上使用。
#[unstable(feature = "core_intrinsics", issue = "none")]
#[rustc_const_unstable(feature = "const_carrying_mul_add", issue = "85532")]
#[rustc_nounwind]
#[rustc_intrinsic]
#[miri::intrinsic_fallback_is_spec]
pub const fn carrying_mul_add<T: [const] fallback::CarryingMulAdd<Unsigned = U>, U>(
    multiplier: T,
    multiplicand: T,
    addend: T,
    carry: T,
) -> (U, T) {
    multiplier.carrying_mul_add(multiplicand, addend, carry)
}

/// 执行精确除法（exact division）；当 `x % y != 0`、或 `y == 0`、或 `x == T::MIN && y == -1` 时，
/// 即为未定义行为（UB）。
///
/// 本 intrinsic 没有稳定的对应物。
#[rustc_intrinsic_const_stable_indirect]
#[rustc_nounwind]
#[rustc_intrinsic]
pub const unsafe fn exact_div<T: Copy>(x: T, y: T) -> T;

/// 执行不做检查的除法（unchecked division）；当 `y == 0` 或 `x == T::MIN && y == -1` 时，
/// 即为未定义行为（UB）。
///
/// 本 intrinsic 的安全封装可通过整数原始类型上的 `checked_div` 方法使用，例如
/// [`u32::checked_div`]
#[rustc_intrinsic_const_stable_indirect]
#[rustc_nounwind]
#[rustc_intrinsic]
pub const unsafe fn unchecked_div<T: Copy>(x: T, y: T) -> T;
/// 返回不做检查的除法（unchecked division）的余数；当 `y == 0` 或 `x == T::MIN && y == -1` 时，
/// 即为未定义行为（UB）。
///
/// 本 intrinsic 的安全封装可通过整数原始类型上的 `checked_rem` 方法使用，例如
/// [`u32::checked_rem`]
#[rustc_intrinsic_const_stable_indirect]
#[rustc_nounwind]
#[rustc_intrinsic]
pub const unsafe fn unchecked_rem<T: Copy>(x: T, y: T) -> T;

/// 执行不做检查的左移（unchecked left shift）；当 `y < 0` 或 `y >= N`（N 为 T 的位宽）时，
/// 即为未定义行为（UB）。
///
/// 本 intrinsic 的安全封装可通过整数原始类型上的 `checked_shl` 方法使用，例如
/// [`u32::checked_shl`]
#[rustc_intrinsic_const_stable_indirect]
#[rustc_nounwind]
#[rustc_intrinsic]
pub const unsafe fn unchecked_shl<T: Copy, U: Copy>(x: T, y: U) -> T;
/// 执行不做检查的右移（unchecked right shift）；当 `y < 0` 或 `y >= N`（N 为 T 的位宽）时，
/// 即为未定义行为（UB）。
///
/// 本 intrinsic 的安全封装可通过整数原始类型上的 `checked_shr` 方法使用，例如
/// [`u32::checked_shr`]
#[rustc_intrinsic_const_stable_indirect]
#[rustc_nounwind]
#[rustc_intrinsic]
pub const unsafe fn unchecked_shr<T: Copy, U: Copy>(x: T, y: U) -> T;

/// 返回不做检查的加法（unchecked）的结果；当 `x + y > T::MAX` 或 `x + y < T::MIN` 时，
/// 即为未定义行为（UB）。
///
/// 本 intrinsic 的稳定对应物是各整数类型上的 `unchecked_add`，例如 [`u16::unchecked_add`] 与 [`i64::unchecked_add`]。
#[rustc_intrinsic_const_stable_indirect]
#[rustc_nounwind]
#[rustc_intrinsic]
pub const unsafe fn unchecked_add<T: Copy>(x: T, y: T) -> T;

/// 返回不做检查的减法（unchecked）的结果；当 `x - y > T::MAX` 或 `x - y < T::MIN` 时，
/// 即为未定义行为（UB）。
///
/// 本 intrinsic 的稳定对应物是各整数类型上的 `unchecked_sub`，例如 [`u16::unchecked_sub`] 与 [`i64::unchecked_sub`]。
#[rustc_intrinsic_const_stable_indirect]
#[rustc_nounwind]
#[rustc_intrinsic]
pub const unsafe fn unchecked_sub<T: Copy>(x: T, y: T) -> T;

/// 返回不做检查的乘法（unchecked）的结果；当 `x * y > T::MAX` 或 `x * y < T::MIN` 时，
/// 即为未定义行为（UB）。
///
/// 本 intrinsic 的稳定对应物是各整数类型上的 `unchecked_mul`，例如 [`u16::unchecked_mul`] 与 [`i64::unchecked_mul`]。
#[rustc_intrinsic_const_stable_indirect]
#[rustc_nounwind]
#[rustc_intrinsic]
pub const unsafe fn unchecked_mul<T: Copy>(x: T, y: T) -> T;

/// 执行循环左移（rotate left）。
///
/// 注意，与大多数 intrinsic 不同，调用它是安全的；它不需要 `unsafe` 块。
/// 因此，实现绝不能要求用户去维护任何安全不变量。
///
/// 本 intrinsic 的稳定版本可通过整数原始类型上的 `rotate_left` 方法使用，例如
/// [`u32::rotate_left`]
#[rustc_intrinsic_const_stable_indirect]
#[rustc_nounwind]
#[rustc_intrinsic]
#[rustc_allow_const_fn_unstable(const_trait_impl, funnel_shifts)]
#[miri::intrinsic_fallback_is_spec]
pub const fn rotate_left<T: [const] fallback::FunnelShift>(x: T, shift: u32) -> T {
    // 确保调用的是 `funnel_shl` 对应的 intrinsic，而不是 fallback 实现。
    // SAFETY: 我们对 `shift` 取模，保证结果一定小于 `T` 的位宽。
    unsafe { unchecked_funnel_shl(x, x, shift % (mem::size_of::<T>() as u32 * 8)) }
}

/// 执行循环右移（rotate right）。
///
/// 注意，与大多数 intrinsic 不同，调用它是安全的；它不需要 `unsafe` 块。
/// 因此，实现绝不能要求用户去维护任何安全不变量。
///
/// 本 intrinsic 的稳定版本可通过整数原始类型上的 `rotate_right` 方法使用，例如
/// [`u32::rotate_right`]
#[rustc_intrinsic_const_stable_indirect]
#[rustc_nounwind]
#[rustc_intrinsic]
#[rustc_allow_const_fn_unstable(const_trait_impl, funnel_shifts)]
#[miri::intrinsic_fallback_is_spec]
pub const fn rotate_right<T: [const] fallback::FunnelShift>(x: T, shift: u32) -> T {
    // 确保调用的是 `funnel_shr` 对应的 intrinsic，而不是 fallback 实现。
    // SAFETY: 我们对 `shift` 取模，保证结果一定小于 `T` 的位宽。
    unsafe { unchecked_funnel_shr(x, x, shift % (mem::size_of::<T>() as u32 * 8)) }
}

/// 返回 (a + b) mod 2<sup>N</sup>，其中 N 为 T 的位宽（即回绕 wrapping 加法）。
///
/// 注意，与大多数 intrinsic 不同，调用它是安全的；它不需要 `unsafe` 块。
/// 因此，实现绝不能要求用户去维护任何安全不变量。
///
/// 本 intrinsic 的稳定版本可通过整数原始类型上的 `wrapping_add` 方法使用，例如
/// [`u32::wrapping_add`]
#[rustc_intrinsic_const_stable_indirect]
#[rustc_nounwind]
#[rustc_intrinsic]
pub const fn wrapping_add<T: Copy>(a: T, b: T) -> T;
/// 返回 (a - b) mod 2<sup>N</sup>，其中 N 为 T 的位宽（即回绕 wrapping 减法）。
///
/// 注意，与大多数 intrinsic 不同，调用它是安全的；它不需要 `unsafe` 块。
/// 因此，实现绝不能要求用户去维护任何安全不变量。
///
/// 本 intrinsic 的稳定版本可通过整数原始类型上的 `wrapping_sub` 方法使用，例如
/// [`u32::wrapping_sub`]
#[rustc_intrinsic_const_stable_indirect]
#[rustc_nounwind]
#[rustc_intrinsic]
pub const fn wrapping_sub<T: Copy>(a: T, b: T) -> T;
/// 返回 (a * b) mod 2<sup>N</sup>，其中 N 为 T 的位宽（即回绕 wrapping 乘法）。
///
/// 注意，与大多数 intrinsic 不同，调用它是安全的；它不需要 `unsafe` 块。
/// 因此，实现绝不能要求用户去维护任何安全不变量。
///
/// 本 intrinsic 的稳定版本可通过整数原始类型上的 `wrapping_mul` 方法使用，例如
/// [`u32::wrapping_mul`]
#[rustc_intrinsic_const_stable_indirect]
#[rustc_nounwind]
#[rustc_intrinsic]
pub const fn wrapping_mul<T: Copy>(a: T, b: T) -> T;

/// 计算 `a + b`，在数值边界处饱和（saturating）。
///
/// 注意，与大多数 intrinsic 不同，调用它是安全的；它不需要 `unsafe` 块。
/// 因此，实现绝不能要求用户去维护任何安全不变量。
///
/// 本 intrinsic 的稳定版本可通过整数原始类型上的 `saturating_add` 方法使用，例如
/// [`u32::saturating_add`]
#[rustc_intrinsic_const_stable_indirect]
#[rustc_nounwind]
#[rustc_intrinsic]
pub const fn saturating_add<T: Copy>(a: T, b: T) -> T;
/// 计算 `a - b`，在数值边界处饱和（saturating）。
///
/// 注意，与大多数 intrinsic 不同，调用它是安全的；它不需要 `unsafe` 块。
/// 因此，实现绝不能要求用户去维护任何安全不变量。
///
/// 本 intrinsic 的稳定版本可通过整数原始类型上的 `saturating_sub` 方法使用，例如
/// [`u32::saturating_sub`]
#[rustc_intrinsic_const_stable_indirect]
#[rustc_nounwind]
#[rustc_intrinsic]
pub const fn saturating_sub<T: Copy>(a: T, b: T) -> T;

/// 漏斗左移（funnel shift left）。
///
/// 把 `a` 与 `b` 拼接（`a` 位于高位的那一半），得到一个位宽翻倍的整数。然后把该整数左移 `shift` 位，
/// 再取出高位那一半。如果 `a` 与 `b` 相同，这就等价于一次循环左移（rotate left）。
///
/// 如果 `shift` 大于或等于 `T` 的位宽，即为未定义行为（UB）。
///
/// 本 intrinsic 的安全版本可通过整数原始类型上的 `funnel_shl` 方法使用，例如 [`u32::funnel_shl`]。
#[rustc_intrinsic]
#[rustc_nounwind]
#[rustc_const_unstable(feature = "funnel_shifts", issue = "145686")]
#[unstable(feature = "funnel_shifts", issue = "145686")]
#[track_caller]
#[miri::intrinsic_fallback_is_spec]
pub const unsafe fn unchecked_funnel_shl<T: [const] fallback::FunnelShift>(
    a: T,
    b: T,
    shift: u32,
) -> T {
    // SAFETY: 调用方保证 `shift` 在合法范围内。
    unsafe { a.unchecked_funnel_shl(b, shift) }
}

/// 漏斗右移（funnel shift right）。
///
/// 把 `a` 与 `b` 拼接（`a` 位于高位的那一半），得到一个位宽翻倍的整数。然后把该整数右移 `shift` 位
/// （对 `T` 的位宽取模），再取出低位那一半。如果 `a` 与 `b` 相同，这就等价于一次循环右移（rotate right）。
///
/// 如果 `shift` 大于或等于 `T` 的位宽，即为未定义行为（UB）。
///
/// 本 intrinsic 更安全的版本可通过整数原始类型上的 `funnel_shr` 方法使用，例如 [`u32::funnel_shr`]
#[rustc_intrinsic]
#[rustc_nounwind]
#[rustc_const_unstable(feature = "funnel_shifts", issue = "145686")]
#[unstable(feature = "funnel_shifts", issue = "145686")]
#[track_caller]
#[miri::intrinsic_fallback_is_spec]
pub const unsafe fn unchecked_funnel_shr<T: [const] fallback::FunnelShift>(
    a: T,
    b: T,
    shift: u32,
) -> T {
    // SAFETY: 调用方保证 `shift` 在合法范围内。
    unsafe { a.unchecked_funnel_shr(b, shift) }
}

/// 这是 [`crate::ptr::read`] 的一个实现细节，不应在别处使用。它存在的原因见那里的注释。
///
/// 本 intrinsic *只能*在“指针是一个不带投影（projection）的局部变量”的地方调用
/// （即 `read_via_copy(ptr)`，而非 `read_via_copy(*ptr)`），这样它就能平凡地遵守
/// 运行期 MIR 关于“操作数中的解引用”的规则。
#[rustc_intrinsic_const_stable_indirect]
#[rustc_nounwind]
#[rustc_intrinsic]
pub const unsafe fn read_via_copy<T>(ptr: *const T) -> T;

/// 这是 [`crate::ptr::write`] 的一个实现细节，不应在别处使用。它存在的原因见那里的注释。
///
/// 本 intrinsic *只能*在“指针是一个不带投影（projection）的局部变量”的地方调用
/// （即 `write_via_move(ptr, x)`，而非 `write_via_move(*ptr, x)`），这样它就能平凡地遵守
/// 运行期 MIR 关于“操作数中的解引用”的规则。
#[rustc_intrinsic_const_stable_indirect]
#[rustc_nounwind]
#[rustc_intrinsic]
pub const unsafe fn write_via_move<T>(ptr: *mut T, value: T);

/// 返回 'v' 中所属变体的判别值（discriminant）；如果 `T` 没有判别值，返回 `0`。
///
/// 注意，与大多数 intrinsic 不同，调用它是安全的；它不需要 `unsafe` 块。
/// 因此，实现绝不能要求用户去维护任何安全不变量。
///
/// 本 intrinsic 的稳定版本是 [`core::mem::discriminant`]。
#[rustc_intrinsic_const_stable_indirect]
#[rustc_nounwind]
#[rustc_intrinsic]
pub const fn discriminant_value<T>(v: &T) -> <T as DiscriminantKind>::Discriminant;

/// Rust 用于栈展开（unwinding）的“try catch”构造。它用数据指针 `data` 去调用函数指针 `try_fn`，
/// 如果在 `try_fn` 运行期间发生展开，就调用 `catch_fn`。
/// 若发生了展开并调用了 `catch_fn`，返回 `1`；否则返回 `0`。
///
/// `catch_fn` 绝不能展开。
///
/// 第三个参数是一个函数，当发生展开（包括 Rust 的 `panic` 和外部展开）时被调用。该函数接收数据指针，
/// 以及一个指向“被捕获的、目标平台与运行时特定的异常对象”的指针。
///
/// 注意，对于外部展开操作，异常对象数据可能无法在 Rust 中被安全使用，不应通过标准库直接暴露。
/// 为防止不安全的访问，库的实现可以选择中止进程，或者向用户呈现一个不透明的错误类型。
///
/// 更多信息见编译器源码，以及本 intrinsic 的稳定版本 `std::panic::catch_unwind` 的文档。
#[rustc_intrinsic]
#[rustc_nounwind]
pub unsafe fn catch_unwind(
    _try_fn: fn(*mut u8),
    _data: *mut u8,
    _catch_fn: fn(*mut u8, *mut u8),
) -> i32;

/// 发出一次 `nontemporal`（非时序）存储，它给 CPU 一个提示：该数据不应被保留在缓存中。
/// 除性能外，它与 `ptr.write(val)` 完全等价。
///
/// 并非所有架构都提供这样的操作。例如 x86 就没有：虽然有 `MOVNT`，但那个操作*不*等价于
/// `ptr.write(val)`（`MOVNT` 写入可能以常规写入所不允许的方式被重排序）。
#[rustc_intrinsic]
#[rustc_nounwind]
pub unsafe fn nontemporal_store<T>(ptr: *mut T, val: T);

/// 细节见 `<*const T>::offset_from` 的文档。
#[rustc_intrinsic_const_stable_indirect]
#[rustc_nounwind]
#[rustc_intrinsic]
pub const unsafe fn ptr_offset_from<T>(ptr: *const T, base: *const T) -> isize;

/// 细节见 `<*const T>::offset_from_unsigned` 的文档。
#[rustc_nounwind]
#[rustc_intrinsic]
#[rustc_intrinsic_const_stable_indirect]
pub const unsafe fn ptr_offset_from_unsigned<T>(ptr: *const T, base: *const T) -> usize;

/// 细节见 `<*const T>::guaranteed_eq` 的文档。
/// 如果结果未知，返回 `2`。
/// 如果两个指针保证相等，返回 `1`。
/// 如果两个指针保证不等，返回 `0`。
#[rustc_intrinsic]
#[rustc_nounwind]
#[rustc_do_not_const_check]
#[inline]
#[miri::intrinsic_fallback_is_spec]
pub const fn ptr_guaranteed_cmp<T>(ptr: *const T, other: *const T) -> u8 {
    (ptr == other) as u8
}

/// 判断两个值的原始字节是否相等。
///
/// 这对数组特别方便，因为它允许像“直接比较一个 `i96`”这样的做法，而不必为 `[6 x i16]` 强制使用 `alloca`。
///
/// 超过某个由后端决定的阈值后，它会像切片相等比较那样发出对 `memcmp` 的调用，而不会造成庞大的代码体积。
///
/// 由于它通过比较底层字节来工作，实际的 `T` 并不特别重要。它的大小和对齐会被用到，
/// 但任何有效性（validity）限制都会被忽略，而不会被强制执行。
///
/// # 安全性（Safety）
///
/// 如果 `*a` 或 `*b` 中任何*字节*是未初始化的，调用本函数即为 UB。
/// 注意这是比“仅仅*值*被完全初始化”更严格的标准：如果 `T` 含有填充（padding），调用本 intrinsic 即为 UB。
///
/// 在编译期，还有一条：如果 `*a` 或 `*b` 中任何字节带有 provenance（来源信息），调用本函数即为 UB。
///
/// （实现允许根据比较结果进行分支，而若其任一输入为 `undef`，这就是 UB。）
#[rustc_nounwind]
#[rustc_intrinsic]
pub const unsafe fn raw_eq<T>(a: &T, b: &T) -> bool;

/// 把 `[left, left + bytes)` 与 `[right, right + bytes)` 当作无符号字节做字典序比较：
/// 若 `left` 较小返回负数，若所有字节都匹配返回零，若 `left` 较大返回正数。
///
/// 它是 `<[u8]>::cmp` 之类操作的底层实现，通常会被降级（lower）为 `memcmp`。
///
/// # 安全性（Safety）
///
/// `left` 与 `right` 都必须对读取 `bytes` 个字节[有效][valid]。
///
/// 注意这适用于整个范围，而不仅仅是到第一个不同的字节为止。这样才允许“按大块读取”的优化。
///
/// [valid]: crate::ptr#safety
#[rustc_nounwind]
#[rustc_intrinsic]
#[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
pub const unsafe fn compare_bytes(left: *const u8, right: *const u8, bytes: usize) -> i32;

/// 细节见 [`std::hint::black_box`] 的文档。
///
/// [`std::hint::black_box`]: crate::hint::black_box
#[rustc_nounwind]
#[rustc_intrinsic]
#[rustc_intrinsic_const_stable_indirect]
pub const fn black_box<T>(dummy: T) -> T;

/// 根据上下文选择调用哪个函数。
///
/// 如果本函数在编译期被求值，那么对本 intrinsic 的调用会被替换为对 `called_in_const` 的调用；
/// 否则被替换为对 `called_at_rt` 的调用。
///
/// 调用本函数是安全的，但请注意下面关于稳定性的考量。
///
/// # 类型要求（Type Requirements）
///
/// 这两个函数都必须是函数项（function item），不能是函数指针或闭包。第一个函数必须是 `const fn`。
///
/// `arg` 是将被传给两个函数之一的、打包成元组的实参；因此两个函数必须接受相同类型的参数，
/// 且都必须返回 RET。
///
/// # 稳定性考量（Stability concerns）
///
/// Rust 尚未决定允许 `const fn` 去判断自己是在编译期还是运行期执行。因此，在任何能从 stable 触达的地方
/// 使用本 intrinsic 时，至关重要的一点是：那个稳定的 `const fn` 的端到端行为，在两种执行模式下必须相同。
/// （这里，未定义行为被视为与任何其他行为“相同”，所以如果该函数在运行期表现出 UB，那么它在编译期
/// 也可以为所欲为。）
///
/// 下面是一个说明这会如何引发问题的例子：
/// ```no_run
/// #![feature(const_eval_select)]
/// #![feature(core_intrinsics)]
/// # #![allow(internal_features)]
/// use std::intrinsics::const_eval_select;
///
/// // 标准库
/// pub const fn inconsistent() -> i32 {
///     fn runtime() -> i32 { 1 }
///     const fn compiletime() -> i32 { 2 }
///
///     // ⚠ 这段代码违反了 `compiletime` 与 `runtime` 之间必须保持的等价性。
///     const_eval_select((), compiletime, runtime)
/// }
///
/// // 用户 crate
/// const X: i32 = inconsistent();
/// let x = inconsistent();
/// assert_eq!(x, X);
/// ```
///
/// 目前这样的断言总会成功；在 Rust 另行决定之前，这一原则不应被违反。
#[rustc_const_unstable(feature = "const_eval_select", issue = "124625")]
#[rustc_intrinsic]
pub const fn const_eval_select<ARG: Tuple, F, G, RET>(
    _arg: ARG,
    _called_in_const: F,
    _called_at_rt: G,
) -> RET
where
    G: FnOnce<ARG, Output = RET>,
    F: const FnOnce<ARG, Output = RET>;

/// 一个让调用 const_eval_select 更方便的宏。用法如下：
/// ```rust,ignore (just a macro example)
/// const_eval_select!(
///     @capture { arg1: i32 = some_expr, arg2: T = other_expr } -> U:
///     if const #[attributes_for_const_arm] {
///         // 编译期代码写在这里。
///     } else #[attributes_for_runtime_arm] {
///         // 运行期代码写在这里。
///     }
/// )
/// ```
/// `@capture` 块声明了哪些周围的变量/表达式可以在 `if const` 内部使用。
/// 注意，这个 `if` 的两个分支臂实际上各自成为一个独立的函数，这正是该宏支持为这些函数设置属性的原因。
/// 两个函数都被标记为 `#[inline]`。
///
/// 关于该 intrinsic 的规则与要求，见 [`const_eval_select()`]。
pub(crate) macro const_eval_select {
    (
        @capture$([$($binders:tt)*])? { $($arg:ident : $ty:ty = $val:expr),* $(,)? } $( -> $ret:ty )? :
        if const
            $(#[$compiletime_attr:meta])* $compiletime:block
        else
            $(#[$runtime_attr:meta])* $runtime:block
    ) => {{
        #[inline]
        $(#[$runtime_attr])*
        fn runtime$(<$($binders)*>)?($($arg: $ty),*) $( -> $ret )? {
            $runtime
        }

        #[inline]
        $(#[$compiletime_attr])*
        const fn compiletime$(<$($binders)*>)?($($arg: $ty),*) $( -> $ret )? {
            // 如果某个参数未被使用，不要发出警告。
            $(let _ = $arg;)*

            $compiletime
        }

        const_eval_select(($($val,)*), compiletime, runtime)
    }},
    // 我们支持对*所有*参数省略 `val` 表达式
    // （但不支持只对*部分*参数省略，那太棘手了）。
    (
        @capture$([$($binders:tt)*])? { $($arg:ident : $ty:ty),* $(,)? } $( -> $ret:ty )? :
        if const
            $(#[$compiletime_attr:meta])* $compiletime:block
        else
            $(#[$runtime_attr:meta])* $runtime:block
    ) => {
        $crate::intrinsics::const_eval_select!(
            @capture$([$($binders)*])? { $($arg : $ty = $arg),* } $(-> $ret)? :
            if const
                $(#[$compiletime_attr])* $compiletime
            else
                $(#[$runtime_attr])* $runtime
        )
    },
}

/// 返回参数的值是否在编译期静态可知。
///
/// 当存在这样一种写法：当某些变量取已知值时代码会*更快*、但一般情形下*更慢*时，本函数很有用：
/// 可以用 `if is_val_statically_known(var)` 在这两种变体之间做选择。这个 `if` 会被优化掉，
/// 只留下所需的那个分支。
///
/// 严格来说，本函数以非确定性的方式返回 `true` 或 `false`，调用方必须确保两种情形下的行为都是合理的（sound）。
/// 换言之，下面这段代码具有*未定义行为（UB）*：
///
/// ```no_run
/// #![feature(core_intrinsics)]
/// # #![allow(internal_features)]
/// use std::hint::unreachable_unchecked;
/// use std::intrinsics::is_val_statically_known;
///
/// if !is_val_statically_known(0) { unsafe { unreachable_unchecked(); } }
/// ```
///
/// 这也意味着下面这段代码的行为是未规定的；它可能 panic，也可能不 panic：
///
/// ```no_run
/// #![feature(core_intrinsics)]
/// # #![allow(internal_features)]
/// use std::intrinsics::is_val_statically_known;
///
/// assert_eq!(is_val_statically_known(0), is_val_statically_known(0));
/// ```
///
/// unsafe 代码永远不可依赖 `is_val_statically_known` 返回任何特定的值。不过，通常只有当参数的值
/// 确实已知时，编译器才会让它返回 `true`。
///
/// # 稳定性考量（Stability concerns）
///
/// 虽然调用它是安全的，但本 intrinsic 在 `const` 上下文中的行为可能与其他情形不同。
/// 关于这会引发的问题的解释，见 [`const_eval_select()`] 的文档。与 `const_eval_select` 不同，
/// 本 intrinsic 即便在 `const` 上下文中也不保证表现得具有确定性。
///
/// # 类型要求（Type Requirements）
///
/// `T` 必须是 `bool`、`char`、某个原始数值类型（例如 `f32`，但不能是 `NonZeroISize`），
/// 或任意瘦指针（thin pointer，例如 `*mut String`）。任何其他参数类型*都可能*导致编译错误。
///
/// ## 指针
///
/// 当输入是指针时，只会考虑指针本身，被指物没有影响。目前，下面这两个函数行为完全相同：
///
/// ```
/// #![feature(core_intrinsics)]
/// # #![allow(internal_features)]
/// use std::intrinsics::is_val_statically_known;
///
/// fn foo(x: &i32) -> bool {
///     is_val_statically_known(x)
/// }
///
/// fn bar(x: &i32) -> bool {
///     is_val_statically_known(
///         (x as *const i32).addr()
///     )
/// }
/// # _ = foo(&5_i32);
/// # _ = bar(&5_i32);
/// ```
#[rustc_const_stable_indirect]
#[rustc_nounwind]
#[unstable(feature = "core_intrinsics", issue = "none")]
#[rustc_intrinsic]
pub const fn is_val_statically_known<T: Copy>(_arg: T) -> bool {
    false
}

/// 对单个值进行的不重叠（non-overlapping）*带类型（typed）*交换。
///
/// 当 `T` 是一个可作为立即数（immediate）加载和存储的简单类型时，代码生成后端会用更优的实现来替换它。
///
/// 本 intrinsic 的稳定形式是 [`crate::mem::swap`]。
///
/// # 安全性（Safety）
/// 若违反下列任一条件，行为即未定义（UB）：
///
/// * `x` 与 `y` 都必须对读和写都[有效][valid]。
///
/// * `x` 与 `y` 都必须正确对齐。
///
/// * 起始于 `x` 的那块内存区域*不*得与起始于 `y` 的那块内存区域重叠。
///
/// * `x` 与 `y` 所指向的内存都必须含有 `T` 类型的值。
///
/// [valid]: crate::ptr#safety
#[rustc_nounwind]
#[inline]
#[rustc_intrinsic]
#[rustc_intrinsic_const_stable_indirect]
pub const unsafe fn typed_swap_nonoverlapping<T>(x: *mut T, y: *mut T) {
    // SAFETY: 调用方在指针后面提供的是单个、互不重叠的项，所以用 `count: 1` 交换它们没有问题。
    unsafe { ptr::swap_nonoverlapping(x, y, 1) };
}

/// 返回我们是否应当在运行期进行某些 UB 检查。它最终会求值为 `cfg!(ub_checks)`，但在混用以不同编译标志
/// 构建的 crate 时，它的行为与 `cfg!` 不同：如果该 crate 启用了 UB 检查、或带有 `#[rustc_preserve_ub_checks]`
/// 属性，求值会被推迟到单态化（monomorphization）时（或推迟到该调用被内联进一个不再继续推迟求值的 crate 时）；
/// 否则求值可以在任何时候发生。
///
/// 这里的常见情形是：一个启用了 ub_checks 构建的用户程序，链接到分发的 sysroot，而后者是在不启用 ub_checks、
/// 但带有 `#[rustc_preserve_ub_checks]` 的情况下构建的。
/// 对于在用户 crate 中被单态化的代码（即泛型函数以及带 `#[inline]` 的函数），把断言的开关挂在 `ub_checks()`
/// 上而非 `cfg!(ub_checks)` 上，意味着只要*用户 crate*启用了 UB 检查，这些断言就会被启用。然而，
/// 如果用户禁用了 UB 检查，这些检查仍会被优化掉。本 intrinsic 主要被
/// [`crate::ub_checks::assert_unsafe_precondition`] 使用。
#[rustc_intrinsic_const_stable_indirect] // 仅用于 UB 检查
#[inline(always)]
#[rustc_intrinsic]
pub const fn ub_checks() -> bool {
    cfg!(ub_checks)
}

/// 返回我们是否应当在运行期进行某些溢出检查。它最终会求值为
/// `cfg!(overflow_checks)`，但在混用以不同编译标志构建的 crate 时，它的行为与 `cfg!` 不同：
/// 如果该 crate 启用了溢出检查、或带有 `#[rustc_inherit_overflow_checks]` 属性，求值会被推迟到
/// 单态化（monomorphization）时（或推迟到该调用被内联进一个不再继续推迟求值的 crate 时）；
/// 否则求值可以在任何时候发生。
///
/// 这里的常见情形是：一个启用了 overflow_checks 构建的用户程序，链接到分发的 sysroot，而后者是在
/// 不启用 overflow_checks、但带有 `#[rustc_inherit_overflow_checks]` 的情况下构建的。
/// 对于在用户 crate 中被单态化的代码（即泛型函数以及带 `#[inline]` 的函数），把断言的开关挂在
/// `overflow_checks()` 上而非 `cfg!(overflow_checks)` 上，意味着只要*用户 crate*启用了溢出检查，
/// 这些断言就会被启用。然而，如果用户禁用了溢出检查，这些检查仍会被优化掉。
#[inline(always)]
#[rustc_intrinsic]
pub const fn overflow_checks() -> bool {
    cfg!(debug_assertions)
}

/// 在编译期分配一块内存。
/// 在运行期，仅返回一个空指针。
///
/// # 安全性（Safety）
///
/// - `align` 参数必须是 2 的幂。
///    - 在编译期，若违反此约束会产生编译错误。
///    - 在运行期，不做检查。
#[rustc_const_unstable(feature = "const_heap", issue = "79597")]
#[rustc_nounwind]
#[rustc_intrinsic]
#[miri::intrinsic_fallback_is_spec]
pub const unsafe fn const_allocate(_size: usize, _align: usize) -> *mut u8 {
    // const 求值会覆盖本函数，但运行期代码目前只返回空指针。
    // 见 <https://github.com/rust-lang/rust/issues/93935>。
    crate::ptr::null_mut()
}

/// 在编译期释放一块由 `intrinsics::const_allocate` 分配的内存。
/// 在运行期，什么也不做。
///
/// # 安全性（Safety）
///
/// - `align` 参数必须是 2 的幂。
///    - 在编译期，若违反此约束会产生编译错误。
///    - 在运行期，不做检查。
/// - 如果 `ptr` 是在另一个 const 中创建的，本 intrinsic 不会释放它。
/// - 如果 `ptr` 指向一个局部变量，本 intrinsic 不会释放它。
#[rustc_const_unstable(feature = "const_heap", issue = "79597")]
#[unstable(feature = "core_intrinsics", issue = "none")]
#[rustc_nounwind]
#[rustc_intrinsic]
#[miri::intrinsic_fallback_is_spec]
pub const unsafe fn const_deallocate(_ptr: *mut u8, _size: usize, _align: usize) {
    // 运行期是空操作（NOP）。
}

#[rustc_const_unstable(feature = "const_heap", issue = "79597")]
#[rustc_nounwind]
#[rustc_intrinsic]
#[miri::intrinsic_fallback_is_spec]
pub const unsafe fn const_make_global(ptr: *mut u8) -> *const u8 {
    // const 求值会覆盖本函数；在运行期它是空操作（NOP）。
    ptr
}

/// 检查前置条件 `cond` 是否已满足。
///
/// 默认情况下，如果启用了 `contract_checks`，当条件返回 false 时它会以“不展开（no unwind）”的方式 panic。
///
/// 注意，本函数在常量求值（constant evaluation）期间是空操作（no-op）。
#[unstable(feature = "contracts_internals", issue = "128044")]
// 对本函数的调用由一个 AST 展开 pass 插入，该 pass 使用等价于 `#[allow_internal_unstable]` 的机制
// 来允许使用 `contracts_internals` 函数。const 检查不遵守 `#[allow_internal_unstable]`，所以对于
// const 特性门控，我们使用面向用户的 `contracts` 特性，而不是永久不稳定的 `contracts_internals`。
#[rustc_const_unstable(feature = "contracts", issue = "128044")]
#[lang = "contract_check_requires"]
#[rustc_intrinsic]
pub const fn contract_check_requires<C: Fn() -> bool + Copy>(cond: C) {
    const_eval_select!(
        @capture[C: Fn() -> bool + Copy] { cond: C } :
        if const {
                // 什么也不做
        } else {
            if !cond() {
                // 万一这是一项安全要求，发出不展开（no unwind）的 panic。
                crate::panicking::panic_nounwind("failed requires check");
            }
        }
    )
}

/// 检查后置条件 `cond` 是否已满足。
///
/// 默认情况下，如果启用了 `contract_checks`，当条件返回 false 时它会以“不展开（no unwind）”的方式 panic。
///
/// 如果 `cond` 是 `None`，则不进行后置条件检查。
///
/// 注意，本函数在常量求值（constant evaluation）期间是空操作（no-op）。
#[unstable(feature = "contracts_internals", issue = "128044")]
// 与 `contract_check_requires` 类似，我们需要使用面向用户的 `contracts` 特性，
// 而不是永久不稳定的 `contracts_internals`。const 检查不遵守 contract 展开所用的
// allow_internal_unstable 逻辑。
#[rustc_const_unstable(feature = "contracts", issue = "128044")]
#[lang = "contract_check_ensures"]
#[rustc_intrinsic]
pub const fn contract_check_ensures<C: Fn(&Ret) -> bool + Copy, Ret>(
    cond: Option<C>,
    ret: Ret,
) -> Ret {
    const_eval_select!(
        @capture[C: Fn(&Ret) -> bool + Copy, Ret] { cond: Option<C>, ret: Ret } -> Ret :
        if const {
            // 什么也不做
            ret
        } else {
            match cond {
                crate::option::Option::Some(cond) => {
                    if !cond(&ret) {
                        // 万一这是一项安全要求，发出不展开（no unwind）的 panic。
                        crate::panicking::panic_nounwind("failed ensures check");
                    }
                },
                crate::option::Option::None => {},
            }
            ret
        }
    )
}

/// 本 intrinsic 返回存储在该 vtable 中的大小（size）。
///
/// # 安全性（Safety）
///
/// `ptr` 必须指向一个 vtable。
#[rustc_nounwind]
#[unstable(feature = "core_intrinsics", issue = "none")]
#[rustc_intrinsic]
pub unsafe fn vtable_size(ptr: *const ()) -> usize;

/// 本 intrinsic 返回存储在该 vtable 中的对齐（alignment）。
///
/// # 安全性（Safety）
///
/// `ptr` 必须指向一个 vtable。
#[rustc_nounwind]
#[unstable(feature = "core_intrinsics", issue = "none")]
#[rustc_intrinsic]
pub unsafe fn vtable_align(ptr: *const ()) -> usize;

/// 如果 `T` 能被强制转换（coerce）为 trait 对象类型 `U`，本 intrinsic 返回 `T` 对应于 `U` 的 vtable。
///
/// # 编译期失败（Compile-time failures）
/// 判断 `T` 能否被强制转换为 trait 对象类型 `U`，需要编译器进行 trait 求解（resolution）。
/// 在某些情况下，该求解可能超出递归上限，于是编译会失败，而不是本函数返回 `None`。
#[rustc_nounwind]
#[unstable(feature = "core_intrinsics", issue = "none")]
#[rustc_intrinsic]
pub const fn vtable_for<T, U: ptr::Pointee<Metadata = ptr::DynMetadata<U>> + ?Sized>()
-> Option<ptr::DynMetadata<U>>;

/// 一个类型以字节为单位的大小。
///
/// 注意，与大多数 intrinsic 不同，调用它是安全的；它不需要 `unsafe` 块。
/// 因此，实现绝不能要求用户去维护任何安全不变量。
///
/// 更具体地说，这是同一类型相邻两项之间以字节为单位的偏移，包含对齐填充（alignment padding）在内。
///
/// 注意，与大多数 intrinsic 不同，它只能在编译期调用，因为各后端并没有为它提供实现。
/// 它唯一的调用方（即它的稳定对应物）会把这次 intrinsic 调用包进一个 `const` 块里，
/// 这样后端看到的就只是一个已经求值好的常量。
///
/// 本 intrinsic 的稳定版本是 [`core::mem::size_of`]。
#[rustc_nounwind]
#[unstable(feature = "core_intrinsics", issue = "none")]
#[rustc_intrinsic_const_stable_indirect]
#[rustc_intrinsic]
pub const fn size_of<T>() -> usize;

/// 一个类型的最小对齐。
///
/// 注意，与大多数 intrinsic 不同，调用它是安全的；它不需要 `unsafe` 块。
/// 因此，实现绝不能要求用户去维护任何安全不变量。
///
/// 注意，与大多数 intrinsic 不同，它只能在编译期调用，因为各后端并没有为它提供实现。
/// 它唯一的调用方（即它的稳定对应物）会把这次 intrinsic 调用包进一个 `const` 块里，
/// 这样后端看到的就只是一个已经求值好的常量。
///
/// 本 intrinsic 的稳定版本是 [`core::mem::align_of`]。
#[rustc_nounwind]
#[unstable(feature = "core_intrinsics", issue = "none")]
#[rustc_intrinsic_const_stable_indirect]
#[rustc_intrinsic]
pub const fn align_of<T>() -> usize;

/// 某个字段在其所属类型内部的偏移。
///
/// 注意，与大多数 intrinsic 不同，调用它是安全的；它不需要 `unsafe` 块。
/// 因此，实现绝不能要求用户去维护任何安全不变量。
///
/// 本 intrinsic 只能在编译期求值，并且只应出现在常量或内联 const 块中。
///
/// 本 intrinsic 的稳定版本是 [`core::mem::offset_of`]。
/// 本 intrinsic 同时也是一个 lang item，这样 `offset_of!` 就能脱糖（desugar）为对它的调用。
#[rustc_nounwind]
#[unstable(feature = "core_intrinsics", issue = "none")]
#[rustc_const_unstable(feature = "core_intrinsics", issue = "none")]
#[rustc_intrinsic_const_stable_indirect]
#[rustc_intrinsic]
#[lang = "offset_of"]
pub const fn offset_of<T: PointeeSized>(variant: u32, field: u32) -> usize;

/// 返回类型 `T` 的变体（variant）数量（转换为 `usize`）；如果 `T` 没有变体，返回 `0`。
/// 无人居住的（uninhabited）变体也会被计入。
///
/// 注意，与大多数 intrinsic 不同，它只能在编译期调用，因为各后端并没有为它提供实现。
/// 它唯一的调用方（即它的稳定对应物）会把这次 intrinsic 调用包进一个 `const` 块里，
/// 这样后端看到的就只是一个已经求值好的常量。
///
/// 本 intrinsic 即将稳定的版本是 [`crate::mem::variant_count`]。
#[rustc_nounwind]
#[unstable(feature = "core_intrinsics", issue = "none")]
#[rustc_intrinsic]
pub const fn variant_count<T>() -> usize;

/// 被引用值以字节为单位的大小。
///
/// 本 intrinsic 的稳定版本是 [`core::mem::size_of_val`]。
///
/// # 安全性（Safety）
///
/// 安全条件见 [`crate::mem::size_of_val_raw`]。
#[rustc_nounwind]
#[unstable(feature = "core_intrinsics", issue = "none")]
#[rustc_intrinsic]
#[rustc_intrinsic_const_stable_indirect]
pub const unsafe fn size_of_val<T: ?Sized>(ptr: *const T) -> usize;

/// 被引用值所要求的对齐。
///
/// 本 intrinsic 的稳定版本是 [`core::mem::align_of_val`]。
///
/// # 安全性（Safety）
///
/// 安全条件见 [`crate::mem::align_of_val_raw`]。
#[rustc_nounwind]
#[unstable(feature = "core_intrinsics", issue = "none")]
#[rustc_intrinsic]
#[rustc_intrinsic_const_stable_indirect]
pub const unsafe fn align_of_val<T: ?Sized>(ptr: *const T) -> usize;

/// 计算一个具体类型的类型信息。
/// 它只能在编译期调用，各后端并未实现它。
#[rustc_intrinsic]
#[unstable(feature = "core_intrinsics", issue = "none")]
pub const fn type_of(_id: crate::any::TypeId) -> crate::mem::type_info::Type {
    panic!("`TypeId::info` can only be called at compile-time")
}

/// 获取一个静态字符串切片，其内容是某个类型的名字。
///
/// 注意，与大多数 intrinsic 不同，它只能在编译期调用，因为各后端并没有为它提供实现。
/// 它唯一的调用方（即它的稳定对应物）会把这次 intrinsic 调用包进一个 `const` 块里，
/// 这样后端看到的就只是一个已经求值好的常量。
///
/// 本 intrinsic 的稳定版本是 [`core::any::type_name`]。
#[rustc_nounwind]
#[unstable(feature = "core_intrinsics", issue = "none")]
#[rustc_intrinsic]
pub const fn type_name<T: ?Sized>() -> &'static str;

/// 获取一个对指定类型而言全局唯一的标识符。无论在哪个 crate 中调用，本函数对同一类型都会返回相同的值。
///
/// 注意，与大多数 intrinsic 不同，它只能在编译期调用，因为各后端并没有为它提供实现。
/// 它唯一的调用方（即它的稳定对应物）会把这次 intrinsic 调用包进一个 `const` 块里，
/// 这样后端看到的就只是一个已经求值好的常量。
///
/// 本 intrinsic 的稳定版本是 [`core::any::TypeId::of`]。
#[rustc_nounwind]
#[unstable(feature = "core_intrinsics", issue = "none")]
#[rustc_intrinsic]
pub const fn type_id<T: ?Sized + 'static>() -> crate::any::TypeId;

/// （在编译期）测试两个 [`crate::any::TypeId`] 实例是否标识同一个类型。之所以需要它，是因为在 const 求值期间，
/// 真正用于区分的数据是不透明的，无法直接检视。
///
/// 本 intrinsic 的稳定版本是 [`core::any::TypeId`] 的 [PartialEq] 实现。
#[rustc_nounwind]
#[unstable(feature = "core_intrinsics", issue = "none")]
#[rustc_intrinsic]
#[rustc_do_not_const_check]
pub const fn type_id_eq(a: crate::any::TypeId, b: crate::any::TypeId) -> bool {
    a.data == b.data
}

/// 在 MIR 中降级（lower）为带 `AggregateKind::RawPtr` 的 `Rvalue::Aggregate`。
///
/// 它用于以一种“与编译器能够改变指针可能布局相兼容”的方式，实现 `slice::from_raw_parts_mut`
/// 和 `ptr::from_raw_parts` 之类的函数。
#[rustc_nounwind]
#[unstable(feature = "core_intrinsics", issue = "none")]
#[rustc_intrinsic_const_stable_indirect]
#[rustc_intrinsic]
pub const fn aggregate_raw_ptr<P: bounds::BuiltinDeref, D, M>(data: D, meta: M) -> P
where
    <P as bounds::BuiltinDeref>::Pointee: ptr::Pointee<Metadata = M>;

/// 在 MIR 中降级（lower）为带 `UnOp::PtrMetadata` 的 `Rvalue::UnaryOp`。
///
/// 它用于实现 `ptr::metadata` 之类的函数。
#[rustc_nounwind]
#[unstable(feature = "core_intrinsics", issue = "none")]
#[rustc_intrinsic_const_stable_indirect]
#[rustc_intrinsic]
pub const fn ptr_metadata<P: ptr::Pointee<Metadata = M> + PointeeSized, M>(ptr: *const P) -> M;

/// 这是 [`ptr::copy_nonoverlapping`] 一个意外稳定下来的别名；请改用那个。
// 注意（特意不放进文档注释里）：`ptr::copy_nonoverlapping` 会额外加入一些 debug 断言；如果你在写编译器测试、
// 或标准库内部那种想要避开这些 debug 断言的代码，请直接调用本 intrinsic。
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_allowed_through_unstable_modules = "import this function via `std::ptr` instead"]
#[rustc_const_stable(feature = "const_intrinsic_copy", since = "1.83.0")]
#[rustc_nounwind]
#[rustc_intrinsic]
pub const unsafe fn copy_nonoverlapping<T>(src: *const T, dst: *mut T, count: usize);

/// 这是 [`ptr::copy`] 一个意外稳定下来的别名；请改用那个。
// 注意（特意不放进文档注释里）：`ptr::copy` 会额外加入一些 debug 断言；如果你在写编译器测试、
// 或标准库内部那种想要避开这些 debug 断言的代码，请直接调用本 intrinsic。
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_allowed_through_unstable_modules = "import this function via `std::ptr` instead"]
#[rustc_const_stable(feature = "const_intrinsic_copy", since = "1.83.0")]
#[rustc_nounwind]
#[rustc_intrinsic]
pub const unsafe fn copy<T>(src: *const T, dst: *mut T, count: usize);

/// 这是 [`ptr::write_bytes`] 一个意外稳定下来的别名；请改用那个。
// 注意（特意不放进文档注释里）：`ptr::write_bytes` 会额外加入一些 debug 断言；如果你在写编译器测试、
// 或标准库内部那种想要避开这些 debug 断言的代码，请直接调用本 intrinsic。
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_allowed_through_unstable_modules = "import this function via `std::ptr` instead"]
#[rustc_const_stable(feature = "const_intrinsic_copy", since = "1.83.0")]
#[rustc_nounwind]
#[rustc_intrinsic]
pub const unsafe fn write_bytes<T>(dst: *mut T, val: u8, count: usize);

/// 返回两个 `f16` 值中的较小者，忽略 NaN。
///
/// 如果其中一个参数是 NaN（quiet 或 signaling），则返回另一个参数。如果两个参数都是 NaN，返回 NaN。
/// 如果两个输入比较相等（例如 `+0.0` 与 `-0.0` 的情形），则可能以非确定性的方式返回任意一个输入。
///
/// 注意，与大多数 intrinsic 不同，调用它是安全的；它不需要 `unsafe` 块。
/// 因此，实现绝不能要求用户去维护任何安全不变量。
///
/// 本 intrinsic 的稳定版本是 [`f16::min`]。
#[rustc_nounwind]
#[rustc_intrinsic]
pub const fn minnumf16(x: f16, y: f16) -> f16;

/// 返回两个 `f32` 值中的较小者，忽略 NaN。
///
/// 如果其中一个参数是 NaN（quiet 或 signaling），则返回另一个参数。如果两个参数都是 NaN，返回 NaN。
/// 如果两个输入比较相等（例如 `+0.0` 与 `-0.0` 的情形），则可能以非确定性的方式返回任意一个输入。
///
/// 注意，与大多数 intrinsic 不同，调用它是安全的；它不需要 `unsafe` 块。
/// 因此，实现绝不能要求用户去维护任何安全不变量。
///
/// 本 intrinsic 的稳定版本是 [`f32::min`]。
#[rustc_nounwind]
#[rustc_intrinsic_const_stable_indirect]
#[rustc_intrinsic]
pub const fn minnumf32(x: f32, y: f32) -> f32;

/// 返回两个 `f64` 值中的较小者，忽略 NaN。
///
/// 如果其中一个参数是 NaN（quiet 或 signaling），则返回另一个参数。如果两个参数都是 NaN，返回 NaN。
/// 如果两个输入比较相等（例如 `+0.0` 与 `-0.0` 的情形），则可能以非确定性的方式返回任意一个输入。
///
/// 注意，与大多数 intrinsic 不同，调用它是安全的；它不需要 `unsafe` 块。
/// 因此，实现绝不能要求用户去维护任何安全不变量。
///
/// 本 intrinsic 的稳定版本是 [`f64::min`]。
#[rustc_nounwind]
#[rustc_intrinsic_const_stable_indirect]
#[rustc_intrinsic]
pub const fn minnumf64(x: f64, y: f64) -> f64;

/// 返回两个 `f128` 值中的较小者，忽略 NaN。
///
/// 如果其中一个参数是 NaN（quiet 或 signaling），则返回另一个参数。如果两个参数都是 NaN，返回 NaN。
/// 如果两个输入比较相等（例如 `+0.0` 与 `-0.0` 的情形），则可能以非确定性的方式返回任意一个输入。
///
/// 注意，与大多数 intrinsic 不同，调用它是安全的；它不需要 `unsafe` 块。
/// 因此，实现绝不能要求用户去维护任何安全不变量。
///
/// 本 intrinsic 的稳定版本是 [`f128::min`]。
#[rustc_nounwind]
#[rustc_intrinsic]
pub const fn minnumf128(x: f128, y: f128) -> f128;

/// 返回两个 `f16` 值中的较小者，传播（propagate）NaN。
///
/// 它的行为类似 IEEE 754-2019 的 minimum。特别地：
/// 如果其中一个参数是 NaN，则按通常的 NaN 传播规则返回一个 NaN。
/// 对本操作而言，-0.0 被视为严格小于 +0.0。
///
/// 注意，与大多数 intrinsic 不同，调用它是安全的；它不需要 `unsafe` 块。
/// 因此，实现绝不能要求用户去维护任何安全不变量。
#[rustc_nounwind]
#[rustc_intrinsic]
pub const fn minimumf16(x: f16, y: f16) -> f16 {
    if x < y {
        x
    } else if y < x {
        y
    } else if x == y {
        if x.is_sign_negative() && y.is_sign_positive() { x } else { y }
    } else {
        // 至少有一个输入是 NaN。用 `+` 来执行 NaN 的传播与“安静化”（quieting）。
        x + y
    }
}

/// 返回两个 `f32` 值中的较小者，传播（propagate）NaN。
///
/// 它的行为类似 IEEE 754-2019 的 minimum。特别地：
/// 如果其中一个参数是 NaN，则按通常的 NaN 传播规则返回一个 NaN。
/// 对本操作而言，-0.0 被视为严格小于 +0.0。
///
/// 注意，与大多数 intrinsic 不同，调用它是安全的；它不需要 `unsafe` 块。
/// 因此，实现绝不能要求用户去维护任何安全不变量。
#[rustc_nounwind]
#[rustc_intrinsic]
pub const fn minimumf32(x: f32, y: f32) -> f32 {
    if x < y {
        x
    } else if y < x {
        y
    } else if x == y {
        if x.is_sign_negative() && y.is_sign_positive() { x } else { y }
    } else {
        // 至少有一个输入是 NaN。用 `+` 来执行 NaN 的传播与“安静化”（quieting）。
        x + y
    }
}

/// 返回两个 `f64` 值中的较小者，传播（propagate）NaN。
///
/// 它的行为类似 IEEE 754-2019 的 minimum。特别地：
/// 如果其中一个参数是 NaN，则按通常的 NaN 传播规则返回一个 NaN。
/// 对本操作而言，-0.0 被视为严格小于 +0.0。
///
/// 注意，与大多数 intrinsic 不同，调用它是安全的；它不需要 `unsafe` 块。
/// 因此，实现绝不能要求用户去维护任何安全不变量。
#[rustc_nounwind]
#[rustc_intrinsic]
pub const fn minimumf64(x: f64, y: f64) -> f64 {
    if x < y {
        x
    } else if y < x {
        y
    } else if x == y {
        if x.is_sign_negative() && y.is_sign_positive() { x } else { y }
    } else {
        // 至少有一个输入是 NaN。用 `+` 来执行 NaN 的传播与“安静化”（quieting）。
        x + y
    }
}

/// 返回两个 `f128` 值中的较小者，传播（propagate）NaN。
///
/// 它的行为类似 IEEE 754-2019 的 minimum。特别地：
/// 如果其中一个参数是 NaN，则按通常的 NaN 传播规则返回一个 NaN。
/// 对本操作而言，-0.0 被视为严格小于 +0.0。
///
/// 注意，与大多数 intrinsic 不同，调用它是安全的；它不需要 `unsafe` 块。
/// 因此，实现绝不能要求用户去维护任何安全不变量。
#[rustc_nounwind]
#[rustc_intrinsic]
pub const fn minimumf128(x: f128, y: f128) -> f128 {
    if x < y {
        x
    } else if y < x {
        y
    } else if x == y {
        if x.is_sign_negative() && y.is_sign_positive() { x } else { y }
    } else {
        // 至少有一个输入是 NaN。用 `+` 来执行 NaN 的传播与“安静化”（quieting）。
        x + y
    }
}

/// 返回两个 `f16` 值中的较大者，忽略 NaN。
///
/// 如果其中一个参数是 NaN（quiet 或 signaling），则返回另一个参数。如果两个参数都是 NaN，返回 NaN。
/// 如果两个输入比较相等（例如 `+0.0` 与 `-0.0` 的情形），则可能以非确定性的方式返回任意一个输入。
///
/// 注意，与大多数 intrinsic 不同，调用它是安全的；它不需要 `unsafe` 块。
/// 因此，实现绝不能要求用户去维护任何安全不变量。
///
/// 本 intrinsic 的稳定版本是 [`f16::max`]。
#[rustc_nounwind]
#[rustc_intrinsic]
pub const fn maxnumf16(x: f16, y: f16) -> f16;

/// 返回两个 `f32` 值中的较大者，忽略 NaN。
///
/// 如果其中一个参数是 NaN（quiet 或 signaling），则返回另一个参数。如果两个参数都是 NaN，返回 NaN。
/// 如果两个输入比较相等（例如 `+0.0` 与 `-0.0` 的情形），则可能以非确定性的方式返回任意一个输入。
///
/// 注意，与大多数 intrinsic 不同，调用它是安全的；它不需要 `unsafe` 块。
/// 因此，实现绝不能要求用户去维护任何安全不变量。
///
/// 本 intrinsic 的稳定版本是 [`f32::max`]。
#[rustc_nounwind]
#[rustc_intrinsic_const_stable_indirect]
#[rustc_intrinsic]
pub const fn maxnumf32(x: f32, y: f32) -> f32;

/// 返回两个 `f64` 值中的较大者，忽略 NaN。
///
/// 如果其中一个参数是 NaN（quiet 或 signaling），则返回另一个参数。如果两个参数都是 NaN，返回 NaN。
/// 如果两个输入比较相等（例如 `+0.0` 与 `-0.0` 的情形），则可能以非确定性的方式返回任意一个输入。
///
/// 注意，与大多数 intrinsic 不同，调用它是安全的；它不需要 `unsafe` 块。
/// 因此，实现绝不能要求用户去维护任何安全不变量。
///
/// 本 intrinsic 的稳定版本是 [`f64::max`]。
#[rustc_nounwind]
#[rustc_intrinsic_const_stable_indirect]
#[rustc_intrinsic]
pub const fn maxnumf64(x: f64, y: f64) -> f64;

/// 返回两个 `f128` 值中的较大者，忽略 NaN。
///
/// 如果其中一个参数是 NaN（quiet 或 signaling），则返回另一个参数。如果两个参数都是 NaN，返回 NaN。
/// 如果两个输入比较相等（例如 `+0.0` 与 `-0.0` 的情形），则可能以非确定性的方式返回任意一个输入。
///
/// 注意，与大多数 intrinsic 不同，调用它是安全的；它不需要 `unsafe` 块。
/// 因此，实现绝不能要求用户去维护任何安全不变量。
///
/// 本 intrinsic 的稳定版本是 [`f128::max`]。
#[rustc_nounwind]
#[rustc_intrinsic]
pub const fn maxnumf128(x: f128, y: f128) -> f128;

/// 返回两个 `f16` 值中的较大者，传播（propagate）NaN。
///
/// 它的行为类似 IEEE 754-2019 的 maximum。特别地：
/// 如果其中一个参数是 NaN，则按通常的 NaN 传播规则返回一个 NaN。
/// 对本操作而言，-0.0 被视为严格小于 +0.0。
///
/// 注意，与大多数 intrinsic 不同，调用它是安全的；它不需要 `unsafe` 块。
/// 因此，实现绝不能要求用户去维护任何安全不变量。
#[rustc_nounwind]
#[rustc_intrinsic]
pub const fn maximumf16(x: f16, y: f16) -> f16 {
    if x > y {
        x
    } else if y > x {
        y
    } else if x == y {
        if x.is_sign_positive() && y.is_sign_negative() { x } else { y }
    } else {
        x + y
    }
}

/// 返回两个 `f32` 值中的较大者，传播（propagate）NaN。
///
/// 它的行为类似 IEEE 754-2019 的 maximum。特别地：
/// 如果其中一个参数是 NaN，则按通常的 NaN 传播规则返回一个 NaN。
/// 对本操作而言，-0.0 被视为严格小于 +0.0。
///
/// 注意，与大多数 intrinsic 不同，调用它是安全的；它不需要 `unsafe` 块。
/// 因此，实现绝不能要求用户去维护任何安全不变量。
#[rustc_nounwind]
#[rustc_intrinsic]
pub const fn maximumf32(x: f32, y: f32) -> f32 {
    if x > y {
        x
    } else if y > x {
        y
    } else if x == y {
        if x.is_sign_positive() && y.is_sign_negative() { x } else { y }
    } else {
        x + y
    }
}

/// 返回两个 `f64` 值中的较大者，传播（propagate）NaN。
///
/// 它的行为类似 IEEE 754-2019 的 maximum。特别地：
/// 如果其中一个参数是 NaN，则按通常的 NaN 传播规则返回一个 NaN。
/// 对本操作而言，-0.0 被视为严格小于 +0.0。
///
/// 注意，与大多数 intrinsic 不同，调用它是安全的；它不需要 `unsafe` 块。
/// 因此，实现绝不能要求用户去维护任何安全不变量。
#[rustc_nounwind]
#[rustc_intrinsic]
pub const fn maximumf64(x: f64, y: f64) -> f64 {
    if x > y {
        x
    } else if y > x {
        y
    } else if x == y {
        if x.is_sign_positive() && y.is_sign_negative() { x } else { y }
    } else {
        x + y
    }
}

/// 返回两个 `f128` 值中的较大者，传播（propagate）NaN。
///
/// 它的行为类似 IEEE 754-2019 的 maximum。特别地：
/// 如果其中一个参数是 NaN，则按通常的 NaN 传播规则返回一个 NaN。
/// 对本操作而言，-0.0 被视为严格小于 +0.0。
///
/// 注意，与大多数 intrinsic 不同，调用它是安全的；它不需要 `unsafe` 块。
/// 因此，实现绝不能要求用户去维护任何安全不变量。
#[rustc_nounwind]
#[rustc_intrinsic]
pub const fn maximumf128(x: f128, y: f128) -> f128 {
    if x > y {
        x
    } else if y > x {
        y
    } else if x == y {
        if x.is_sign_positive() && y.is_sign_negative() { x } else { y }
    } else {
        x + y
    }
}

/// 返回一个 `f16` 的绝对值。
///
/// 本 intrinsic 的稳定版本是
/// [`f16::abs`](../../std/primitive.f16.html#method.abs)
#[rustc_nounwind]
#[rustc_intrinsic]
pub const fn fabsf16(x: f16) -> f16;

/// 返回一个 `f32` 的绝对值。
///
/// 本 intrinsic 的稳定版本是
/// [`f32::abs`](../../std/primitive.f32.html#method.abs)
#[rustc_nounwind]
#[rustc_intrinsic_const_stable_indirect]
#[rustc_intrinsic]
pub const fn fabsf32(x: f32) -> f32;

/// 返回一个 `f64` 的绝对值。
///
/// 本 intrinsic 的稳定版本是
/// [`f64::abs`](../../std/primitive.f64.html#method.abs)
#[rustc_nounwind]
#[rustc_intrinsic_const_stable_indirect]
#[rustc_intrinsic]
pub const fn fabsf64(x: f64) -> f64;

/// 返回一个 `f128` 的绝对值。
///
/// 本 intrinsic 的稳定版本是
/// [`f128::abs`](../../std/primitive.f128.html#method.abs)
#[rustc_nounwind]
#[rustc_intrinsic]
pub const fn fabsf128(x: f128) -> f128;

/// 对 `f16` 值，把 `y` 的符号复制到 `x` 上。
///
/// 本 intrinsic 的稳定版本是
/// [`f16::copysign`](../../std/primitive.f16.html#method.copysign)
#[rustc_nounwind]
#[rustc_intrinsic]
pub const fn copysignf16(x: f16, y: f16) -> f16;

/// 对 `f32` 值，把 `y` 的符号复制到 `x` 上。
///
/// 本 intrinsic 的稳定版本是
/// [`f32::copysign`](../../std/primitive.f32.html#method.copysign)
#[rustc_nounwind]
#[rustc_intrinsic_const_stable_indirect]
#[rustc_intrinsic]
pub const fn copysignf32(x: f32, y: f32) -> f32;
/// 对 `f64` 值，把 `y` 的符号复制到 `x` 上。
///
/// 本 intrinsic 的稳定版本是
/// [`f64::copysign`](../../std/primitive.f64.html#method.copysign)
#[rustc_nounwind]
#[rustc_intrinsic_const_stable_indirect]
#[rustc_intrinsic]
pub const fn copysignf64(x: f64, y: f64) -> f64;

/// 对 `f128` 值，把 `y` 的符号复制到 `x` 上。
///
/// 本 intrinsic 的稳定版本是
/// [`f128::copysign`](../../std/primitive.f128.html#method.copysign)
#[rustc_nounwind]
#[rustc_intrinsic]
pub const fn copysignf128(x: f128, y: f128) -> f128;

/// 使用 Enzyme 为 `f` 的自动微分（automatic differentiation）生成 LLVM 函数体，
/// 其中 `df` 是导函数、`args` 是它的参数。
///
/// 在展开 `#[autodiff_forward]` 与 `#[autodiff_reverse]` 属性宏时，内部用它作为 `df` 的函数体。
///
/// 类型参数：
/// - `F`：待微分的原函数。必须是函数项（function item）。
/// - `G`：导函数。必须是函数项。
/// - `T`：传给 `df` 的参数元组。
/// - `R`：导函数的返回类型。
///
/// 下面展示了宏展开期间 `autodiff` intrinsic 用在何处：
///
/// ```rust,ignore (macro example)
/// #[autodiff_forward(df1, Dual, Const, Dual)]
/// pub fn f1(x: &[f64], y: f64) -> f64 {
///     unimplemented!()
/// }
/// ```
///
/// 展开为：
///
/// ```rust,ignore (macro example)
/// #[rustc_autodiff]
/// #[inline(never)]
/// pub fn f1(x: &[f64], y: f64) -> f64 {
///     ::core::panicking::panic("not implemented")
/// }
/// #[rustc_autodiff(Forward, 1, Dual, Const, Dual)]
/// pub fn df1(x: &[f64], bx_0: &[f64], y: f64) -> (f64, f64) {
///     ::core::intrinsics::autodiff(f1::<>, df1::<>, (x, bx_0, y))
/// }
/// ```
#[rustc_nounwind]
#[rustc_intrinsic]
pub const fn autodiff<F, G, T: crate::marker::Tuple, R>(f: F, df: G, args: T) -> R;

/// 生成一个包装函数的 LLVM 函数体，用于将内核（kernel）`f` 卸载（offload）出去执行。
///
/// 类型参数：
/// - `F`：要卸载的内核。必须是函数项（function item）。
/// - `T`：传给 `f` 的参数元组。
/// - `R`：内核的返回类型。
///
/// 参数：
/// - `f`：要卸载的内核函数。
/// - `workgroup_dim`：一个三维尺寸，指定要启动的工作组（workgroup）数量。
/// - `thread_dim`：一个三维尺寸，指定每个工作组的线程数量。
/// - `args`：转发给 `f` 的参数元组。
///
/// 用法示例（伪代码）：
///
/// ```rust,ignore (pseudocode)
/// fn kernel(x: *mut [f64; 128]) {
///     core::intrinsics::offload(kernel_1, [256, 1, 1], [32, 1, 1], (x,))
/// }
///
/// #[cfg(target_os = "linux")]
/// extern "C" {
///     pub fn kernel_1(array_b: *mut [f64; 128]);
/// }
///
/// #[cfg(not(target_os = "linux"))]
/// #[rustc_offload_kernel]
/// extern "gpu-kernel" fn kernel_1(x: *mut [f64; 128]) {
///     unsafe { (*x)[0] = 21.0 };
/// }
/// ```
///
/// 作为参考，见 Clang 关于 offloading 的文档：
/// <https://clang.llvm.org/docs/OffloadingDesign.html>。
#[rustc_nounwind]
#[rustc_intrinsic]
pub const fn offload<F, T: crate::marker::Tuple, R>(
    f: F,
    workgroup_dim: [u32; 3],
    thread_dim: [u32; 3],
    args: T,
) -> R;

/// 告知 Miri：给定的指针一定具有某个对齐。
#[cfg(miri)]
#[rustc_allow_const_fn_unstable(const_eval_select)]
pub(crate) const fn miri_promise_symbolic_alignment(ptr: *const (), align: usize) {
    unsafe extern "Rust" {
        /// 由 Miri 提供的 extern 函数，用于承诺给定指针对“符号化（symbolic）”对齐检查而言已正确对齐。
        /// 如果该指针实际上未对齐、或 `align` 不是 2 的幂，将会失败。当对齐检查是具体（concrete）模式时
        /// （这是默认模式），它没有任何效果。
        fn miri_promise_symbolic_alignment(ptr: *const (), align: usize);
    }

    const_eval_select!(
        @capture { ptr: *const (), align: usize}:
        if const {
            // 什么也不做。
        } else {
            // SAFETY: 这个调用总是安全的。
            unsafe {
                miri_promise_symbolic_alignment(ptr, align);
            }
        }
    )
}

/// 把可变参数列表 `src` 的当前位置复制到可变参数列表 `dst`。
///
/// # 安全性（Safety）
///
/// 调用本函数前，你必须检查以下不变量：
///
/// - `dest` 必须非空，并指向有效、可写的内存。
/// - `dest` 不得与 `src` 存在别名（alias）。
///
#[rustc_intrinsic]
#[rustc_nounwind]
pub unsafe fn va_copy<'f>(dest: *mut VaList<'f>, src: &VaList<'f>);

/// 从 `va_list` `ap` 中加载一个类型为 `T` 的参数，并令 `ap` 指向的参数位置前进一步。
///
/// # 安全性（Safety）
///
/// 仅在满足以下条件时调用本函数才是合理的（sound）：
///
/// - 存在下一个可用的可变参数。
/// - 下一个参数的类型必须与类型 `T` 在 ABI 上兼容。
/// - 下一个参数必须持有一个正确初始化的、类型为 `T` 的值。
///
/// 用不兼容的类型、无效的值，或在已没有更多可变参数时调用本函数，都是不合理的（unsound）。
///
#[rustc_intrinsic]
#[rustc_nounwind]
pub unsafe fn va_arg<T: VaArgSafe>(ap: &mut VaList<'_>) -> T;

/// 在用 `va_start` 或 `va_copy` 初始化之后，销毁可变参数列表 `ap`。
///
/// # 安全性（Safety）
///
/// 本次调用之后，`ap` 不得再被用于访问可变参数。
///
#[rustc_intrinsic]
#[rustc_nounwind]
pub unsafe fn va_end(ap: &mut VaList<'_>);
