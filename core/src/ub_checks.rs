//! 提供 [`assert_unsafe_precondition`] 宏，以及若干覆盖常见 unsafe 前置条件的工具函数。
//!
//! 这里的检查只服务于调试期的 UB 暴露：它们让标准库在调用方开启 debug assertions 时，
//! 尽早发现“不满足 unsafe 契约”的调用，而不是把问题留给后续优化或解释器诊断。检查
//! 本身不能成为安全性的依据，调用方仍必须在所有构建模式下维护文档列出的不变量。

use crate::intrinsics::{self, const_eval_select};

/// 检查 unsafe 函数的调用方是否满足了该函数声明的前置条件。
///
/// 当调用方单态化时启用了 debug assertions，这个检查会在运行期启用。对于语言层 UB，
/// 通过此宏实现的检查在 const-eval/Miri 中总是被忽略，因为解释器自己能给出更精确的
/// 语言 UB 诊断。
///
/// 这个宏的调用形式应为
/// `assert_unsafe_precondition!(check_{library,language}_ub, "message", (ident: type = expr, ident: type = expr) => check_expr)`
/// 其中每个 `expr` 会先求值，并以 `ident: type` 函数参数的形式传入；所有这些参数随后
/// 传给一个函数，该函数的函数体就是 `check_expr`。如果要防守的是语言层 UB，即按照
/// Rust Abstract Machine 规则会立即构成 UB 的情况，应选择 `check_language_ub`。如果
/// 要防守的是文档化的库前置条件，并且违反它不会 *立即* 造成语言层 UB，应选择
/// `check_library_ub`。
///
/// 如果实际检查的是语言层 UB，却误用 `check_library_ub`，const-eval/Miri 会变慢，并且
/// 我们会得到普通 panic 消息而不是解释器更友好的诊断，但发现 UB 的能力不变。反过来，
/// 如果实际只是库级 UB，却误用 `check_language_ub`，该检查会在 const-eval/Miri 中被
/// 省略，UB 因而可能漏检。即使后续确实执行了依赖该库级 UB 的语言层 UB，Miri 报告的
/// 回溯也可能已经离最初原因很远。
///
/// 这些检查背后有一个在 codegen 阶段求值的条件，而不是像 [`debug_assert`] 那样在宏
/// 展开阶段决定。这意味着，预编译标准库若以优化开启且 debug assertions 关闭的方式构建，
/// 这些检查会从它自己的单态化代码中优化掉；但如果标准库调用方开启了 debug assertions，
/// 并且单态化了此宏的一次展开，那么那份单态化代码会包含检查。
///
/// 由于这些检查不能在 MIR 中优化掉，调用和实现都必须注意编译期开销。此宏的调用总会
/// 展开成如下结构：
/// ```ignore (伪代码)
/// if ::core::intrinsics::check_language_ub() {
///     precondition_check(args)
/// }
/// ```
/// 其中 `precondition_check` 是带有 `#[rustc_nounwind]`、`#[inline]` 和
/// `#[rustc_no_mir_inline]` 属性的单态函数。这组属性保证实际检查逻辑只编译一次，并
/// 生成尽量少的 IR：检查不会被 MIR inliner 内联，但 *可以* 由 codegen 后端内联并完全优化。
///
/// 调用方为了调用此宏，应避免额外引入 `let` 绑定或宏外辅助代码。预编译标准库带完整
/// debuginfo 构建，而这些变量无法在 MIR 中优化掉；一个看似无害的 `let` 也可能产生足够多的
/// debuginfo，从而对 debug 构建的编译时间造成可测量的影响。
#[macro_export]
#[unstable(feature = "ub_checks", issue = "none")]
macro_rules! assert_unsafe_precondition {
    ($kind:ident, $message:expr, ($($name:ident:$ty:ty = $arg:expr),*$(,)?) => $e:expr $(,)?) => {
        {
            // 这个检查可以内联，但不能由 MIR inliner 内联。
            // 原因是 MIR inliner 很难在这里做出划算判断：在 MIR 中，此调用受
            // `debug_assertions` 控制，而 release 构建的 codegen 会把它变成 `false`。
            // 此时内联检查逻辑只会浪费工作，并拖慢编译时间。
            //
            // 另一方面，LLVM 能看到这个常量分支；如果它是 `false`，LLVM 可以不内联检查就直接删掉。
            // 如果它是 `true`，LLVM 又可以内联检查逻辑，从而得到明显更好的运行期性能。
            #[rustc_no_mir_inline]
            #[inline]
            #[rustc_nounwind]
            #[track_caller]
            const fn precondition_check($($name:$ty),*) {
                if !$e {
                    let msg = concat!("unsafe precondition(s) violated: ", $message,
                        "\n\nThis indicates a bug in the program. \
                        This Undefined Behavior check is optional, and cannot be relied on for safety.");
                    ::core::panicking::panic_nounwind_fmt(::core::fmt::Arguments::from_str(msg), false);
                }
            }

            if ::core::ub_checks::$kind() {
                precondition_check($($arg,)*);
            }
        }
    };
}
#[unstable(feature = "ub_checks", issue = "none")]
pub use assert_unsafe_precondition;
/// 只要启用了 UB 检查，库级 UB 检查就始终启用。
/// 这里使用重导出，避免生成没有必要的包装函数。
#[unstable(feature = "ub_checks", issue = "none")]
pub use intrinsics::ub_checks as check_library_ub;

/// 判断当前是否应检查语言层 UB。
///
/// 设计意图是在解释器中不执行这类检查，因为 Miri/const-eval 解释器已经拥有自己的语言 UB
/// 检测，通常能给出更准确、更贴近根因的错误信息。
#[inline]
#[rustc_allow_const_fn_unstable(const_eval_select)]
pub(crate) const fn check_language_ub() -> bool {
    // 只用于 UB 检查，因此可以使用 const_eval_select。
    const_eval_select!(
        @capture { } -> bool:
        if const {
            // const 分支中总是关闭这里的 UB 检查。
            false
        } else {
            // 在 Miri 中关闭这里的 UB 检查，交给解释器自身诊断。
            !cfg!(miri)
        }
    ) && intrinsics::ub_checks()
}

/// 检查 `ptr` 是否满足给定对齐要求；并且在 `is_zst == false` 时，检查 `ptr` 非空。
///
/// 在 `const` 中此判断只是近似值，可能出现伪失败。它主要供带 `check_language_ub` 的
/// `assert_unsafe_precondition!` 使用，而这种检查本来就不会在 `const` 中执行。
#[inline]
#[rustc_allow_const_fn_unstable(const_eval_select)]
pub(crate) const fn maybe_is_aligned_and_not_null(
    ptr: *const (),
    align: usize,
    is_zst: bool,
) -> bool {
    // 这只服务于安全前置检查，因此可以使用 const_eval_select。
    maybe_is_aligned(ptr, align) && (is_zst || !ptr.is_null())
}

/// 检查 `ptr` 是否满足给定对齐要求。
///
/// 在 `const` 中此判断只是近似值，可能出现伪失败。它主要供带 `check_language_ub` 的
/// `assert_unsafe_precondition!` 使用，而这种检查本来就不会在 `const` 中执行。
#[inline]
#[rustc_allow_const_fn_unstable(const_eval_select)]
pub(crate) const fn maybe_is_aligned(ptr: *const (), align: usize) -> bool {
    // 这只服务于安全前置检查，因此可以使用 const_eval_select。
    const_eval_select!(
        @capture { ptr: *const (), align: usize } -> bool:
        if const {
            true
        } else {
            ptr.is_aligned_to(align)
        }
    )
}

#[inline]
pub(crate) const fn is_valid_allocation_size(size: usize, len: usize) -> bool {
    let max_len = if size == 0 { usize::MAX } else { isize::MAX as usize / size };
    len <= max_len
}

/// 检查从 `src` 和 `dst` 开始、大小为 `count * size` 的两段内存区域是否 *不* 重叠。
///
/// 注意，在 const-eval 中这个函数只返回 `true`，因此只能和 `assert_unsafe_precondition!`
/// 搭配使用，语义上类似 `is_aligned_and_not_null` 一类近似检查。
#[inline]
#[rustc_allow_const_fn_unstable(const_eval_select)]
pub(crate) const fn maybe_is_nonoverlapping(
    src: *const (),
    dst: *const (),
    size: usize,
    count: usize,
) -> bool {
    // 这只服务于安全前置检查，因此可以使用 const_eval_select。
    const_eval_select!(
        @capture { src: *const (), dst: *const (), size: usize, count: usize } -> bool:
        if const {
            true
        } else {
            let src_usize = src.addr();
            let dst_usize = dst.addr();
            let Some(size) = size.checked_mul(count) else {
                crate::panicking::panic_nounwind(
                    "is_nonoverlapping: `size_of::<T>() * count` overflows a usize",
                )
            };
            let diff = src_usize.abs_diff(dst_usize);
            // 如果两个指针地址之间的绝对距离至少等于缓冲区大小，
            // 那么两段区域不可能重叠。
            diff >= size
        }
    )
}
