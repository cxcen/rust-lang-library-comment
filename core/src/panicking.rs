//! core 的 panic 支持。
//!
//! 在 core 中，panic 总是携带一条消息，得到的 `core::panic::PanicInfo`
//! 包含 `fmt::Arguments`。而在 std 中，panic 可以通过 `panic_any` 触发，
//! 它会抛出一个 `Box<dyn Any>`，其中可以保存任意类型的值。因此，
//! `std::panic::PanicHookInfo` 是另一种类型，它包含 `&dyn Any`，
//! 而不是 `fmt::Arguments`。std 的 panic handler 会把 `fmt::Arguments`
//! 转换成一个 `&dyn Any`，其中保存格式化消息对应的 `&'static str`
//! 或 `String`。
//!
//! core 库不能定义任何 panic handler，但可以调用它。这意味着 core 内部的
//! 函数允许 panic，但若要真正有用，上游 crate 必须为 core 定义可用的
//! panic 处理方式。当前的 panic 接口是：
//!
//! ```
//! fn panic_impl(pi: &core::panic::PanicInfo<'_>) -> !
//! # { loop {} }
//! ```
//!
//! 本模块还包含其他几个触发 panic 的函数，但它们只是编译器所需的 lang item。
//! 所有 panic 最终都会汇入这个函数。实际符号通过 `#[panic_handler]`
//! 属性声明。

#![allow(dead_code, missing_docs)]
#![unstable(
    feature = "panic_internals",
    reason = "internal details of the implementation of the `panic!` and related macros",
    issue = "none"
)]

use crate::fmt;
use crate::intrinsics::const_eval_select;
use crate::panic::{Location, PanicInfo};

#[cfg(feature = "panic_immediate_abort")]
compile_error!(
    "panic_immediate_abort is now a real panic strategy! \
    Enable it with `panic = \"immediate-abort\"` in Cargo.toml, \
    or with the compiler flags `-Zunstable-options -Cpanic=immediate-abort`. \
    In both cases, you still need to build core, e.g. with `-Zbuild-std`"
);

// 首先定义所有 panic 都会流经的两个主入口。最终二者都只是围绕 `panic_impl`
// 的便利包装。

/// 使用格式化消息触发 panic 的入口点。
///
/// 它的设计目的是尽可能降低调用点所需的代码量（让 `panic!()` 对其他函数
/// 内联等场景的影响尽量低），把实际格式化工作移到这个共享位置。
// 如果 panic=immediate-abort，则内联 abort 调用；否则避免内联，因为这是冷路径。
#[cfg_attr(not(panic = "immediate-abort"), inline(never), cold)]
#[cfg_attr(panic = "immediate-abort", inline)]
#[track_caller]
#[lang = "panic_fmt"] // const 求值的 panic 需要
#[rustc_do_not_const_check] // 由 const-eval 接管
#[rustc_const_stable_indirect] // 暴露给 stable，因此必须遵守 stable const 规则
pub const fn panic_fmt(fmt: fmt::Arguments<'_>) -> ! {
    if cfg!(panic = "immediate-abort") {
        super::intrinsics::abort()
    }

    // NOTE 该函数从不跨越 FFI 边界；它是一个 Rust 到 Rust 的调用，
    // 会解析到 `#[panic_handler]` 函数。
    unsafe extern "Rust" {
        #[lang = "panic_impl"]
        fn panic_impl(pi: &PanicInfo<'_>) -> !;
    }

    let pi = PanicInfo::new(
        &fmt,
        Location::caller(),
        /* can_unwind */ true,
        /* force_no_backtrace */ false,
    );

    // SAFETY: `panic_impl` 在安全 Rust 代码中定义，因此调用它是安全的。
    unsafe { panic_impl(&pi) }
}

/// 类似 `panic_fmt`，但用于不会 unwind 的 panic。
///
/// 它必须是单独函数，才能携带 `rustc_nounwind` 属性。
#[cfg_attr(not(panic = "immediate-abort"), inline(never), cold)]
#[cfg_attr(panic = "immediate-abort", inline)]
#[track_caller]
// 该属性有一个关键副作用：如果 panic handler 忽略 `can_unwind`
// 并且仍然 unwind，就会触发“从 nounwind 函数向外 unwind”的防护，
// 进而导致“在不能 unwind 的函数中 panic”。
#[rustc_nounwind]
#[rustc_const_stable_indirect] // 暴露给 stable，因此必须遵守 stable const 规则
#[rustc_allow_const_fn_unstable(const_eval_select)]
pub const fn panic_nounwind_fmt(fmt: fmt::Arguments<'_>, force_no_backtrace: bool) -> ! {
    const_eval_select!(
        @capture { fmt: fmt::Arguments<'_>, force_no_backtrace: bool } -> !:
        if const #[track_caller] {
            // 编译期本来也不会 unwind，因此可以调用普通的 `panic_fmt`。
            panic_fmt(fmt)
        } else #[track_caller] {
            if cfg!(panic = "immediate-abort") {
                super::intrinsics::abort()
            }

            // NOTE 该函数从不跨越 FFI 边界；它是一个 Rust 到 Rust 的调用，
            // 会解析到 `#[panic_handler]` 函数。
            unsafe extern "Rust" {
                #[lang = "panic_impl"]
                fn panic_impl(pi: &PanicInfo<'_>) -> !;
            }

            // 将 `can_unwind` 标志设为 false 的 PanicInfo 会强制 abort。
            let pi = PanicInfo::new(
                &fmt,
                Location::caller(),
                /* can_unwind */ false,
                force_no_backtrace,
            );

            // SAFETY: `panic_impl` 在安全 Rust 代码中定义，因此调用它是安全的。
            unsafe { panic_impl(&pi) }
        }
    )
}

// 接下来定义一组更高层包装器，它们最终都会落到上面两个核心函数。

/// core 的 `panic!` 宏在未使用格式化时的底层实现。
// 除 panic=immediate-abort 外绝不内联，以尽量避免调用点代码膨胀。
#[cfg_attr(not(panic = "immediate-abort"), inline(never), cold)]
#[cfg_attr(panic = "immediate-abort", inline)]
#[track_caller]
#[rustc_const_stable_indirect] // 暴露给 stable，因此必须遵守 stable const 规则
#[lang = "panic"] // lint 和 miri 处理 panic 时使用
pub const fn panic(expr: &'static str) -> ! {
    // 使用 Arguments::from_str 而不是 format_args!("{expr}")，可能降低体积开销。
    // format_args! 宏会使用 str 的 Display trait 写入 expr，这会调用 Formatter::pad，
    // 而该函数必须支持字符串截断和填充（即便此处都不会用到）。使用
    // Arguments::from_str 可能让编译器从输出二进制中省略 Formatter::pad，
    // 最多节省几 KB。
    // 不过，这个优化只适用于 `'static` 字符串：`from_str` 还会让这条消息的
    // `Arguments::as_str` 返回 `Some`，这意味着它可以在无需分配或复制的情况下
    // 成为 panic payload 的一部分。生命周期更短的字符串会随着 unwind 时栈帧
    // 弹出而失效，因此不能由 payload 直接引用。
    panic_fmt(fmt::Arguments::from_str(expr));
}

// 为编译器生成的断言生成可用的函数。
//
// 把这些函数放在 libcore 中，意味着所有 Rust 程序都可以生成跳转到这段代码的指令，
// 而不是展开为上面的 panic("...")，后者会给调用点增加额外代码体积（常量字符串参数的
// 指针和长度）。
//
// 当这段代码被频繁调用时（例如使用 -Coverflow-checks），这对降低二进制体积影响尤其重要。
macro_rules! panic_const {
    ($($lang:ident = $message:expr,)+) => {
        $(
            /// 这是用 MIR 生成的 Assert 所产生消息来调用的 panic。
            //
            // 除 panic=immediate-abort 外绝不内联，以尽量避免调用点代码膨胀
            #[cfg_attr(not(panic = "immediate-abort"), inline(never), cold)]
            #[cfg_attr(panic = "immediate-abort", inline)]
            #[track_caller]
            #[rustc_const_stable_indirect] // 暴露给 stable，因此必须遵守 stable const 规则
            #[lang = stringify!($lang)]
            pub const fn $lang() -> ! {
                // 关于这里为何使用 `Arguments::from_str`，参见 `panic(&'static str)` 中的注释。
                panic_fmt(fmt::Arguments::from_str($message));
            }
        )+
    }
}

// 遗憾的是，这组字符串在此处和编译器内几个位置以略有不同的形式重复存在。尚不清楚
// 有没有好办法在不向编译器添加特殊情况的前提下去重（例如 const 泛型函数不会在各
// crate 之间共享单一定义，而这正是我们在这里需要的）。
pub mod panic_const {
    use super::*;
    panic_const! {
        panic_const_add_overflow = "attempt to add with overflow",
        panic_const_sub_overflow = "attempt to subtract with overflow",
        panic_const_mul_overflow = "attempt to multiply with overflow",
        panic_const_div_overflow = "attempt to divide with overflow",
        panic_const_rem_overflow = "attempt to calculate the remainder with overflow",
        panic_const_neg_overflow = "attempt to negate with overflow",
        panic_const_shr_overflow = "attempt to shift right with overflow",
        panic_const_shl_overflow = "attempt to shift left with overflow",
        panic_const_div_by_zero = "attempt to divide by zero",
        panic_const_rem_by_zero = "attempt to calculate the remainder with a divisor of zero",
        panic_const_coroutine_resumed = "coroutine resumed after completion",
        panic_const_async_fn_resumed = "`async fn` resumed after completion",
        panic_const_async_gen_fn_resumed = "`async gen fn` resumed after completion",
        panic_const_gen_fn_none = "`gen fn` should just keep returning `None` after completion",
        panic_const_coroutine_resumed_panic = "coroutine resumed after panicking",
        panic_const_async_fn_resumed_panic = "`async fn` resumed after panicking",
        panic_const_async_gen_fn_resumed_panic = "`async gen fn` resumed after panicking",
        panic_const_gen_fn_none_panic = "`gen fn` should just keep returning `None` after panicking",
    }
    // async drop 功能使用的单独 panic 常量列表
    // （等对应 lang item 进入 bootstrap 后，可以把它们合并）
    panic_const! {
        panic_const_coroutine_resumed_drop = "coroutine resumed after async drop",
        panic_const_async_fn_resumed_drop = "`async fn` resumed after async drop",
        panic_const_async_gen_fn_resumed_drop = "`async gen fn` resumed after async drop",
        panic_const_gen_fn_none_drop = "`gen fn` resumed after async drop",
    }
}

/// 类似 `panic`，但不 unwind，也不使用 track_caller，以降低对调用方代码体积的影响。
/// 如果想要 `#[track_caller]` 以获得更好的错误位置，请直接调用 `panic_nounwind_fmt`。
#[cfg_attr(not(panic = "immediate-abort"), inline(never), cold)]
#[cfg_attr(panic = "immediate-abort", inline)]
#[lang = "panic_nounwind"] // codegen 处理不会 unwind 的 panic 时需要
#[rustc_nounwind]
#[rustc_const_stable_indirect] // 暴露给 stable，因此必须遵守 stable const 规则
pub const fn panic_nounwind(expr: &'static str) -> ! {
    panic_nounwind_fmt(fmt::Arguments::from_str(expr), /* force_no_backtrace */ false);
}

/// 类似 `panic_nounwind`，但还会禁止显示 backtrace。
#[cfg_attr(not(panic = "immediate-abort"), inline(never), cold)]
#[cfg_attr(panic = "immediate-abort", inline)]
#[rustc_nounwind]
pub fn panic_nounwind_nobacktrace(expr: &'static str) -> ! {
    panic_nounwind_fmt(fmt::Arguments::from_str(expr), /* force_no_backtrace */ true);
}

#[inline]
#[track_caller]
#[rustc_diagnostic_item = "unreachable_display"] // `non-fmt-panics` lint 需要
pub fn unreachable_display<T: fmt::Display>(x: &T) -> ! {
    panic_fmt(format_args!("internal error: entered unreachable code: {}", *x));
}

/// 它仅用于 2015 edition 的 `panic!` 宏，以触发针对
/// `panic!(my_str_variable);` 的 lint。
#[inline]
#[track_caller]
#[rustc_diagnostic_item = "panic_str_2015"]
#[rustc_const_stable_indirect] // 暴露给 stable，因此必须遵守 stable const 规则
pub const fn panic_str_2015(expr: &str) -> ! {
    panic_display(&expr);
}

#[inline]
#[track_caller]
#[lang = "panic_display"] // const 求值的 panic 需要
#[rustc_do_not_const_check] // 由 const-eval 接管
#[rustc_const_stable_indirect] // 暴露给 stable，因此必须遵守 stable const 规则
pub const fn panic_display<T: fmt::Display>(x: &T) -> ! {
    panic_fmt(format_args!("{}", *x));
}

#[cfg_attr(not(panic = "immediate-abort"), inline(never), cold, optimize(size))]
#[cfg_attr(panic = "immediate-abort", inline)]
#[track_caller]
#[lang = "panic_bounds_check"] // codegen 为数组/slice 越界访问触发 panic 时需要
fn panic_bounds_check(index: usize, len: usize) -> ! {
    if cfg!(panic = "immediate-abort") {
        super::intrinsics::abort()
    }

    panic!("index out of bounds: the len is {len} but the index is {index}")
}

#[cfg_attr(not(panic = "immediate-abort"), inline(never), cold, optimize(size))]
#[cfg_attr(panic = "immediate-abort", inline)]
#[track_caller]
#[lang = "panic_misaligned_pointer_dereference"] // codegen 为未对齐指针解引用触发 panic 时需要
#[rustc_nounwind] // `CheckAlignment` MIR pass 要求该函数绝不 unwind
fn panic_misaligned_pointer_dereference(required: usize, found: usize) -> ! {
    if cfg!(panic = "immediate-abort") {
        super::intrinsics::abort()
    }

    panic_nounwind_fmt(
        format_args!(
            "misaligned pointer dereference: address must be a multiple of {required:#x} but is {found:#x}"
        ),
        /* force_no_backtrace */ false,
    )
}

#[cfg_attr(not(panic = "immediate-abort"), inline(never), cold, optimize(size))]
#[cfg_attr(panic = "immediate-abort", inline)]
#[track_caller]
#[lang = "panic_null_pointer_dereference"] // codegen 为 null 指针解引用触发 panic 时需要
#[rustc_nounwind] // `CheckNull` MIR pass 要求该函数绝不 unwind
fn panic_null_pointer_dereference() -> ! {
    if cfg!(panic = "immediate-abort") {
        super::intrinsics::abort()
    }

    panic_nounwind_fmt(
        format_args!("null pointer dereference occurred"),
        /* force_no_backtrace */ false,
    )
}

#[cfg_attr(not(panic = "immediate-abort"), inline(never), cold, optimize(size))]
#[cfg_attr(panic = "immediate-abort", inline)]
#[track_caller]
#[lang = "panic_invalid_enum_construction"] // codegen 为无效 enum 构造触发 panic 时需要。
#[rustc_nounwind] // `CheckEnums` MIR pass 要求该函数绝不 unwind
fn panic_invalid_enum_construction(source: u128) -> ! {
    if cfg!(panic = "immediate-abort") {
        super::intrinsics::abort()
    }

    panic_nounwind_fmt(
        format_args!("trying to construct an enum from an invalid value {source:#x}"),
        /* force_no_backtrace */ false,
    )
}

/// 因无法从该函数向外 unwind 而触发 panic。
///
/// 这被拆成单独函数，是为了避免每个 crate 都内联包含要传给
/// `panic_nounwind` 的字符串，从而增加代码体积。
///
/// 该函数由 codegen backend 直接调用，因此不能拥有任何额外参数
/// （包括 `track_caller` 合成的参数）。
#[cfg_attr(not(panic = "immediate-abort"), inline(never), cold, optimize(size))]
#[cfg_attr(panic = "immediate-abort", inline)]
#[lang = "panic_cannot_unwind"] // codegen 在 nounwind 函数中处理 panic 时需要
#[rustc_nounwind]
fn panic_cannot_unwind() -> ! {
    // 保持该文本与 `rustc_middle` 中的 `UnwindTerminateReason::as_str` 同步。
    panic_nounwind("panic in a function that cannot unwind")
}

/// 因清理期间正从析构函数向外 unwind 而触发 panic。
///
/// 这被拆成单独函数，是为了避免每个 crate 都内联包含要传给
/// `panic_nounwind` 的字符串，从而增加代码体积。
///
/// 该函数由 codegen backend 直接调用，因此不能拥有任何额外参数
/// （包括 `track_caller` 合成的参数）。
#[cfg_attr(not(panic = "immediate-abort"), inline(never), cold, optimize(size))]
#[cfg_attr(panic = "immediate-abort", inline)]
#[lang = "panic_in_cleanup"] // codegen 在 nounwind 函数中处理 panic 时需要
#[rustc_nounwind]
fn panic_in_cleanup() -> ! {
    // 保持该文本与 `rustc_middle` 中的 `UnwindTerminateReason::as_str` 同步。
    panic_nounwind_nobacktrace("panic in a destructor during cleanup")
}

/// 该函数在 const eval 中替代 `panic_fmt` 使用。
#[lang = "const_panic_fmt"] // const-eval 机器用它替换对 `panic_fmt` lang item 的调用
#[rustc_const_stable_indirect] // 暴露给 stable，因此必须遵守 stable const 规则
pub const fn const_panic_fmt(fmt: fmt::Arguments<'_>) -> ! {
    if let Some(msg) = fmt.as_str() {
        // `panic_display` 函数由 const eval 接管。
        panic_display(&msg);
    } else {
        // SAFETY: 这里仅在编译期求值，const eval 能可靠地处理这个 UB
        // （以防这个分支不知为何变得可达）。
        unsafe { crate::hint::unreachable_unchecked() };
    }
}

#[derive(Debug)]
#[doc(hidden)]
pub enum AssertKind {
    Eq,
    Ne,
    Match,
}

/// 供 `assert_eq!` 和 `assert_ne!` 宏使用的内部函数。
#[cfg_attr(not(panic = "immediate-abort"), inline(never), cold, optimize(size))]
#[cfg_attr(panic = "immediate-abort", inline)]
#[track_caller]
#[doc(hidden)]
pub fn assert_failed<T, U>(
    kind: AssertKind,
    left: &T,
    right: &U,
    args: Option<fmt::Arguments<'_>>,
) -> !
where
    T: fmt::Debug + ?Sized,
    U: fmt::Debug + ?Sized,
{
    assert_failed_inner(kind, &left, &right, args)
}

/// `assert_match!` 使用的内部函数。
#[cfg_attr(not(panic = "immediate-abort"), inline(never), cold, optimize(size))]
#[cfg_attr(panic = "immediate-abort", inline)]
#[track_caller]
#[doc(hidden)]
pub fn assert_matches_failed<T: fmt::Debug + ?Sized>(
    left: &T,
    right: &str,
    args: Option<fmt::Arguments<'_>>,
) -> ! {
    // pattern 是字符串，因此可直接显示。
    struct Pattern<'a>(&'a str);
    impl fmt::Debug for Pattern<'_> {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str(self.0)
        }
    }
    assert_failed_inner(AssertKind::Match, &left, &Pattern(right), args);
}

/// 上面几个函数的非泛型版本，用于避免代码膨胀。
#[cfg_attr(not(panic = "immediate-abort"), inline(never), cold, optimize(size))]
#[cfg_attr(panic = "immediate-abort", inline)]
#[track_caller]
fn assert_failed_inner(
    kind: AssertKind,
    left: &dyn fmt::Debug,
    right: &dyn fmt::Debug,
    args: Option<fmt::Arguments<'_>>,
) -> ! {
    let op = match kind {
        AssertKind::Eq => "==",
        AssertKind::Ne => "!=",
        AssertKind::Match => "matches",
    };

    match args {
        Some(args) => panic!(
            r#"assertion `left {op} right` failed: {args}
  left: {left:?}
 right: {right:?}"#
        ),
        None => panic!(
            r#"assertion `left {op} right` failed
  left: {left:?}
 right: {right:?}"#
        ),
    }
}
