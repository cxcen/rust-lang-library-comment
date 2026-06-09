//! # Rust 核心库
//!
//! Rust Core Library 是[标准库](../std/index.html)无依赖[^free]的地基。它是语言与库之间
//! 可移植的粘合层,定义所有 Rust 代码都会用到的 intrinsic、基本类型和核心 trait。
//! 它不链接上游库、系统库或 libc。
//!
//! [^free]: 严格来说,有少量符号可能被需要,但并非在所有配置下都必须存在。
//!
//! core 库是*最小*的:它甚至不知道堆分配,也不提供并发或 I/O。这些能力都需要平台集成,
//! 而本库保持平台无关。
//!
//! # 如何使用 core library
//!
//! 请注意,以下细节目前都不被视为稳定保证。
//!
// FIXME: 接口稳定后补充更多细节。
//! 本库建立在少量既有符号存在的假设之上:
//!
//! * `memcpy`、`memmove`、`memset`、`memcmp`、`bcmp`、`strlen` - 这些是 Rust codegen
//!   后端可能生成的核心内存例程。此外,本库也可能显式调用 `strlen`。它们的签名与 C 中相同,
//!   但语义上有额外假设:对 `memcpy`、`memmove`、`memset`、`memcmp` 和 `bcmp`,
//!   若 `n` 参数为 0,即使指针为 NULL 或 dangling,也假设调用不构成 UB。(编译器对这些函数
//!   作额外假设很常见:[clang](https://reviews.llvm.org/D86993) 与
//!   [GCC](https://gcc.gnu.org/onlinedocs/gcc/Standards.html#C-Language) 也这样做。)
//!   这些函数通常由系统 libc 提供,也可由
//!   [compiler-builtins crate](https://crates.io/crates/compiler_builtins) 提供。
//!   注意,本库不保证永远都会作这些假设,因此 Rust 用户代码若直接调用这些 C 函数,
//!   仍应遵守 C 规范! 建议 Rust 用户代码调用本库提供的函数,例如 `ptr::copy`。
//!
//! * Panic handler - 该函数接收一个 `&panic::PanicInfo` 参数。使用 core 的最终 crate
//!   需要自行定义这个 panic 函数;它唯一必须满足的契约是永不返回。实现应使用
//!   `#[panic_handler]` 标记。
//!
//! * `rust_eh_personality` - 供编译器的失败处理机制使用。它通常映射到 GCC 的
//!   personality function;但不会触发 panic 的 crate 可以确信该函数永远不会被调用。
//!   对应的 `lang` 属性名为 `eh_personality`。

#![stable(feature = "core", since = "1.6.0")]
#![doc(
    html_playground_url = "https://play.rust-lang.org/",
    issue_tracker_base_url = "https://github.com/rust-lang/rust/issues/",
    test(no_crate_inject, attr(deny(warnings))),
    test(attr(allow(dead_code, deprecated, unused_variables, unused_mut)))
)]
#![doc(rust_logo)]
#![doc(auto_cfg(hide(
    no_fp_fmt_parse,
    target_pointer_width = "16",
    target_pointer_width = "32",
    target_pointer_width = "64",
    target_has_atomic = "8",
    target_has_atomic = "16",
    target_has_atomic = "32",
    target_has_atomic = "64",
    target_has_atomic = "ptr",
    target_has_atomic_equal_alignment = "8",
    target_has_atomic_equal_alignment = "16",
    target_has_atomic_equal_alignment = "32",
    target_has_atomic_equal_alignment = "64",
    target_has_atomic_equal_alignment = "ptr",
    target_has_atomic_load_store = "8",
    target_has_atomic_load_store = "16",
    target_has_atomic_load_store = "32",
    target_has_atomic_load_store = "64",
    target_has_atomic_load_store = "ptr",
)))]
#![no_core]
#![rustc_coherence_is_core]
#![rustc_preserve_ub_checks]
//
// Lint 配置:
#![deny(rust_2021_incompatible_or_patterns)]
#![deny(unsafe_op_in_unsafe_fn)]
#![deny(fuzzy_provenance_casts)]
#![warn(deprecated_in_future)]
#![warn(missing_debug_implementations)]
#![warn(missing_docs)]
#![allow(explicit_outlives_requirements)]
#![allow(incomplete_features)]
#![warn(multiple_supertrait_upcastable)]
#![allow(internal_features)]
#![deny(ffi_unwind_calls)]
#![warn(unreachable_pub)]
// 在 bootstrap 阶段不检查链接冗余。
#![allow(rustdoc::redundant_explicit_links)]
#![warn(rustdoc::unescaped_backticks)]
//
// 库特性:
// tidy-alphabetical-start
#![feature(array_ptr_get)]
#![feature(asm_experimental_arch)]
#![feature(bigint_helper_methods)]
#![feature(bstr)]
#![feature(bstr_internals)]
#![feature(cfg_select)]
#![feature(cfg_target_has_reliable_f16_f128)]
#![feature(const_carrying_mul_add)]
#![feature(const_cmp)]
#![feature(const_destruct)]
#![feature(const_eval_select)]
#![feature(const_select_unpredictable)]
#![feature(core_intrinsics)]
#![feature(coverage_attribute)]
#![feature(disjoint_bitor)]
#![feature(internal_impls_macro)]
#![feature(ip)]
#![feature(is_ascii_octdigit)]
#![feature(link_cfg)]
#![feature(offset_of_enum)]
#![feature(panic_internals)]
#![feature(pattern_type_macro)]
#![feature(ptr_alignment_type)]
#![feature(ptr_metadata)]
#![feature(set_ptr_value)]
#![feature(slice_ptr_get)]
#![feature(str_internals)]
#![feature(str_split_inclusive_remainder)]
#![feature(str_split_remainder)]
#![feature(type_info)]
#![feature(ub_checks)]
#![feature(unsafe_pinned)]
#![feature(utf16_extra)]
#![feature(variant_count)]
// tidy-alphabetical-end
//
// 语言特性:
// tidy-alphabetical-start
#![feature(abi_unadjusted)]
#![feature(adt_const_params)]
#![feature(allow_internal_unsafe)]
#![feature(allow_internal_unstable)]
#![feature(auto_traits)]
#![feature(cfg_sanitize)]
#![feature(cfg_target_has_atomic)]
#![feature(cfg_target_has_atomic_equal_alignment)]
#![feature(cfg_ub_checks)]
#![feature(const_precise_live_drops)]
#![feature(const_trait_impl)]
#![feature(decl_macro)]
#![feature(deprecated_suggestion)]
#![feature(derive_const)]
#![feature(diagnostic_on_const)]
#![feature(doc_cfg)]
#![feature(doc_notable_trait)]
#![feature(extern_types)]
#![feature(f16)]
#![feature(f128)]
#![feature(freeze_impls)]
#![feature(fundamental)]
#![feature(funnel_shifts)]
#![feature(if_let_guard)]
#![feature(intra_doc_pointers)]
#![feature(intrinsics)]
#![feature(lang_items)]
#![feature(link_llvm_intrinsics)]
#![feature(macro_metavar_expr)]
#![feature(macro_metavar_expr_concat)]
#![feature(marker_trait_attr)]
#![feature(min_specialization)]
#![feature(multiple_supertrait_upcastable)]
#![feature(must_not_suspend)]
#![feature(negative_impls)]
#![feature(never_type)]
#![feature(no_core)]
#![feature(optimize_attribute)]
#![feature(pattern_types)]
#![feature(prelude_import)]
#![feature(reborrow)]
#![feature(repr_simd)]
#![feature(rustc_allow_const_fn_unstable)]
#![feature(rustc_attrs)]
#![feature(rustdoc_internals)]
#![feature(simd_ffi)]
#![feature(staged_api)]
#![feature(stmt_expr_attributes)]
#![feature(strict_provenance_lints)]
#![feature(trait_alias)]
#![feature(transparent_unions)]
#![feature(try_blocks)]
#![feature(unboxed_closures)]
#![feature(unsized_fn_params)]
#![feature(with_negative_coherence)]
// tidy-alphabetical-end
//
// 目标特性:
// tidy-alphabetical-start
#![feature(aarch64_unstable_target_feature)]
#![feature(arm_target_feature)]
#![feature(avx10_target_feature)]
#![feature(hexagon_target_feature)]
#![feature(loongarch_target_feature)]
#![feature(mips_target_feature)]
#![feature(nvptx_target_feature)]
#![feature(powerpc_target_feature)]
#![feature(riscv_target_feature)]
#![feature(rtm_target_feature)]
#![feature(s390x_target_feature)]
#![feature(wasm_target_feature)]
#![feature(x86_amx_intrinsics)]
// tidy-alphabetical-end

// 允许在 intra-doc 链接中使用 `core::`。
#[allow(unused_extern_crates)]
extern crate self as core;

/* core prelude;覆盖范围不像 std prelude 那样全面。 */
// 编译器要求 prelude 定义出现在使用它的 use 语句之前。
pub mod prelude;

#[prelude_import]
#[allow(unused)]
use prelude::rust_2024::*;

#[macro_use]
mod macros;

#[unstable(feature = "assert_matches", issue = "82775")]
/// 包含不稳定 `assert_matches` 宏的不稳定模块。
pub mod assert_matches {
    #[unstable(feature = "assert_matches", issue = "82775")]
    pub use crate::macros::{assert_matches, debug_assert_matches};
}

#[unstable(feature = "derive_from", issue = "144889")]
/// 包含不稳定 `From` 派生宏的不稳定模块。
pub mod from {
    #[unstable(feature = "derive_from", issue = "144889")]
    pub use crate::macros::builtin::From;
}

// 为避免造成破坏,暂时不通过 #[macro_export] 导出它。
#[unstable(feature = "autodiff", issue = "124509")]
/// 包含不稳定 `autodiff` 宏的不稳定模块。
pub mod autodiff {
    #[unstable(feature = "autodiff", issue = "124509")]
    pub use crate::macros::builtin::{autodiff_forward, autodiff_reverse};
}

#[unstable(feature = "contracts", issue = "128044")]
pub mod contracts;

#[unstable(feature = "cfg_select", issue = "115585")]
pub use crate::macros::cfg_select;

#[macro_use]
mod internal_macros;

#[path = "num/shells/legacy_int_modules.rs"]
mod legacy_int_modules;
#[stable(feature = "rust1", since = "1.0.0")]
#[allow(clippy::useless_attribute)] // FIXME: clippy 误报(https://github.com/rust-lang/rust-clippy/issues/15636)
#[allow(deprecated_in_future)]
pub use legacy_int_modules::{i8, i16, i32, i64, isize, u8, u16, u32, u64, usize};
#[stable(feature = "i128", since = "1.26.0")]
#[allow(clippy::useless_attribute)] // FIXME: clippy 误报(https://github.com/rust-lang/rust-clippy/issues/15636)
#[allow(deprecated_in_future)]
pub use legacy_int_modules::{i128, u128};

#[path = "num/f128.rs"]
pub mod f128;
#[path = "num/f16.rs"]
pub mod f16;
#[path = "num/f32.rs"]
pub mod f32;
#[path = "num/f64.rs"]
pub mod f64;

#[macro_use]
pub mod num;

/* 用于所有权管理的核心模块。 */

pub mod hint;
pub mod intrinsics;
pub mod mem;
#[unstable(feature = "profiling_marker_api", issue = "148197")]
pub mod profiling;
pub mod ptr;
#[unstable(feature = "ub_checks", issue = "none")]
pub mod ub_checks;

/* 核心语言 trait。 */

pub mod borrow;
pub mod clone;
pub mod cmp;
pub mod convert;
pub mod default;
pub mod error;
pub mod index;
pub mod marker;
pub mod ops;

/* 核心类型与 primitive 上的方法。 */

pub mod any;
pub mod array;
pub mod ascii;
pub mod asserting;
#[unstable(feature = "async_iterator", issue = "79024")]
pub mod async_iter;
#[unstable(feature = "bstr", issue = "134915")]
pub mod bstr;
pub mod cell;
pub mod char;
pub mod ffi;
#[unstable(feature = "core_io_borrowed_buf", issue = "117693")]
pub mod io;
pub mod iter;
pub mod net;
pub mod option;
pub mod os;
pub mod panic;
pub mod panicking;
#[unstable(feature = "pattern_type_macro", issue = "123646")]
pub mod pat;
pub mod pin;
#[unstable(feature = "random", issue = "130703")]
pub mod random;
#[unstable(feature = "new_range_api", issue = "125687")]
pub mod range;
pub mod result;
pub mod sync;
#[unstable(feature = "unsafe_binders", issue = "130516")]
pub mod unsafe_binder;

pub mod fmt;
pub mod hash;
pub mod slice;
pub mod str;
pub mod time;

pub mod wtf8;

pub mod unicode;

/* 异步。 */
pub mod future;
pub mod task;

/* 堆内存分配器 trait。 */
#[allow(missing_docs)]
pub mod alloc;

// 注意:不需要公开。
mod bool;
mod escape;
mod tuple;
mod unit;

#[stable(feature = "core_primitive", since = "1.43.0")]
pub mod primitive;

// 将 `core_arch` crate 直接引入 core。`core_arch` 的内容位于另一个仓库:
// rust-lang/stdarch。
//
// `core_arch` 依赖 core,但这个模块的内容经过了相应安排,可以直接拉到这里,
// 并让该 crate 使用当前这个 crate 作为它的 core。
#[path = "../../stdarch/crates/core_arch/src/mod.rs"]
#[allow(
    missing_docs,
    missing_debug_implementations,
    dead_code,
    unused_imports,
    unsafe_op_in_unsafe_fn,
    ambiguous_glob_reexports,
    deprecated_in_future,
    unreachable_pub
)]
#[allow(rustdoc::bare_urls)]
mod core_arch;

#[stable(feature = "simd_arch", since = "1.27.0")]
pub mod arch;

// 将 `core_simd` crate 直接引入 core。`core_simd` 的内容位于另一个仓库:
// rust-lang/portable-simd。
//
// `core_simd` 依赖 core,但这个模块的内容经过了相应安排,可以直接拉到这里,
// 并让该 crate 使用当前这个 crate 作为它的 core。
#[path = "../../portable-simd/crates/core_simd/src/mod.rs"]
#[allow(missing_debug_implementations, dead_code, unsafe_op_in_unsafe_fn)]
#[allow(rustdoc::bare_urls)]
#[unstable(feature = "portable_simd", issue = "86656")]
mod core_simd;

#[unstable(feature = "portable_simd", issue = "86656")]
pub mod simd {
    #![doc = include_str!("../../portable-simd/crates/core_simd/src/core_simd_docs.md")]

    #[unstable(feature = "portable_simd", issue = "86656")]
    pub use crate::core_simd::simd::*;
}

include!("primitive_docs.rs");
