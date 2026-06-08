#![doc = include_str!("../../stdarch/crates/core_arch/src/core_arch_docs.md")]

#[allow(
    // 某些目标没有任何内容可重新导出，这会
    // 使 `pub use` 既未使用又不可达；允许
    // 这两个 lint，以免写出 `#[cfg]`
    //
    // 参见 https://github.com/rust-lang/rust/pull/116033#issuecomment-1760085575
    unused_imports,
    unreachable_pub
)]
#[stable(feature = "simd_arch", since = "1.27.0")]
pub use crate::core_arch::arch::*;

/// 内联汇编。
///
/// 用法指南见 [Rust By Example]，语法和可用选项的详细信息见
/// [Rust 参考手册][reference]。
///
/// [Rust By Example]: https://doc.rust-lang.org/nightly/rust-by-example/unsafe/asm.html
/// [reference]: https://doc.rust-lang.org/nightly/reference/inline-assembly.html
#[stable(feature = "asm", since = "1.59.0")]
#[rustc_builtin_macro]
pub macro asm("assembly template", $(operands,)* $(options($(option),*))?) {
    /* 编译器内建 */
}

/// 与 `#[naked]` 函数配合使用的内联汇编。
///
/// 用法指南见 [Rust By Example]，语法和可用选项的详细信息见
/// [Rust 参考手册][reference]。
///
/// [Rust By Example]: https://doc.rust-lang.org/nightly/rust-by-example/unsafe/asm.html
/// [reference]: https://doc.rust-lang.org/nightly/reference/inline-assembly.html
#[stable(feature = "naked_functions", since = "1.88.0")]
#[rustc_builtin_macro]
pub macro naked_asm("assembly template", $(operands,)* $(options($(option),*))?) {
    /* 编译器内建 */
}

/// 模块级内联汇编。
///
/// 用法指南见 [Rust By Example]，语法和可用选项的详细信息见
/// [Rust 参考手册][reference]。
///
/// [Rust By Example]: https://doc.rust-lang.org/nightly/rust-by-example/unsafe/asm.html
/// [reference]: https://doc.rust-lang.org/nightly/reference/inline-assembly.html
#[stable(feature = "global_asm", since = "1.59.0")]
#[rustc_builtin_macro]
pub macro global_asm("assembly template", $(operands,)* $(options($(option),*))?) {
    /* 编译器内建 */
}

/// 编译为特定目标的软件断点指令或等效形式。
///
/// 这通常会中止程序。它可能产生 core dump，和/或让系统记录调试信息。
/// 根据调试器或其它工具，可能存在额外的特定目标能力；特别是调试器可能能够恢复执行。
///
/// 如果可能，它会生成一段指令序列，使调试器能在断点*之后*恢复，
/// 而不是在断点*处*恢复；不过确切行为依赖目标和调试器，且不作保证。
///
/// 如果目标平台没有任何调试断点指令，它可能会改为编译成陷阱指令
/// （例如未定义指令），或某种其它特定目标的中止形式；这些形式可能支持方便地恢复执行，
/// 也可能不支持。
///
/// 不保证确切行为和生成的确切指令，除一点外：在没有调试工具参与的普通执行中，
/// 它不会继续执行。
///
/// - 在 x86 目标上，这会生成一条 `int3` 指令。
/// - 在 aarch64 目标上，这会生成一条 `brk #0xf000` 指令。
// 稳定此 API 时，请更新 `core::intrinsics::breakpoint` 上的注释。
#[unstable(feature = "breakpoint", issue = "133724")]
#[inline(always)]
pub fn breakpoint() {
    core::intrinsics::breakpoint();
}
