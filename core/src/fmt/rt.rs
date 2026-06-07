#![allow(missing_debug_implementations)]
#![unstable(feature = "fmt_internals", reason = "internal to format_args!", issue = "none")]

//! 本文件中的所有类型和方法都由编译器在展开/降级 `format_args!()` 时使用。
//!
//! `format_args!()` 生成的 `fmt::Arguments` 会直接引用这里的运行时表示。
//! 这些类型的布局、构造方式和不变量都属于编译器与 `core::fmt` 之间的内部契约;
//! 如果不了解对 `format_args!()` 宏展开的影响,不要修改它们。

use super::*;
use crate::hint::unreachable_unchecked;
use crate::ptr::NonNull;

#[derive(Copy, Clone)]
enum ArgumentType<'a> {
    Placeholder {
        // INVARIANT: 对某个 `T`,`formatter` 的原始类型是 `fn(&T, _) -> _`,
        // 且 `value` 来源于一个 `&'a T`。这保证擦除成 `NonNull<()>` 后,
        // 格式化函数仍会以正确的具体类型读取该引用。
        value: NonNull<()>,
        formatter: unsafe fn(NonNull<()>, &mut Formatter<'_>) -> Result,
        _lifetime: PhantomData<&'a ()>,
    },
    Count(u16),
}

/// 表示 `format_args!()` 接收的一个泛化“参数”。
///
/// 它可以是占位符参数,也可以是计数参数。
/// * 占位符参数保存一个用于格式化给定值的函数。编译期会保证该函数和值具有
///   匹配的具体类型,随后这个结构把不同类型的参数规范化为同一种运行时表示。
///   从语义上看,占位符参数就是一个优化过的、已部分应用的格式化函数,
///   近似等价于 `exists T.(&T, fn(&T, &mut Formatter<'_>) -> Result`。
/// * 计数参数保存动态格式化参数的数值,例如运行时提供的 precision 或 width。
///
/// `Argument` 的生命周期来自 `format_args!` 所借用的实参。`write!`、panic 消息
/// 和日志设施通常会立即消费这些 `Arguments`;除非格式字符串没有非静态实参,
/// 否则不应把它们长期保存。
#[lang = "format_argument"]
#[derive(Copy, Clone)]
#[repr(align(2))] // 确保指向该结构的指针最低位始终为 0。
pub struct Argument<'a> {
    ty: ArgumentType<'a>,
}

macro_rules! argument_new {
    ($t:ty, $x:expr, $f:expr) => {
        Argument {
            // INVARIANT: 这里由 `&'a T` 和 `fn(&T, ...)` 构造
            // `ArgumentType<'a>`,因此维持了上面对 `value` 与 `formatter`
            // 类型匹配关系的要求。
            ty: ArgumentType::Placeholder {
                value: NonNull::<$t>::from_ref($x).cast(),
                // Rust ABI 把所有指针视为等价表示,因此把 `fn(&T)` transmute 成
                // `fn(NonNull<()>)`,再用一个实际指向 `T` 的 `NonNull<()>` 调用它,
                // 在 Rust 语义下是允许的。不过 CFI sanitizer 不允许这种调用形态,
                // 遇到时会触发崩溃。
                //
                // 为避免这种崩溃,启用 CFI 时使用一个辅助函数。未启用 CFI 时则仍然
                // transmute 函数指针,避免为辅助函数付出额外成本,主要是代码体积成本。
                //
                // 这类似于启用 KCFI 时 Rust 编译器在 vtable 内部采取的做法:
                // 编译器会生成只负责调整实参期望类型的 trampoline 函数。
                // `ArgumentType::Placeholder` 有点像手工构造的 trait object,
                // 因而这里也需要采用同样策略并不意外。
                //
                // 从 Rust 角度看,CFI 拒绝完全合法的 Rust 程序仍然是个问题,
                // 因此这里的处理不构成稳定保证;在问题解决前,我们保留这个
                // workaround 以维持 Rust 与 CFI/KCFI 的兼容性。
                #[cfg(not(any(sanitize = "cfi", sanitize = "kcfi")))]
                formatter: {
                    let f: fn(&$t, &mut Formatter<'_>) -> Result = $f;
                    // SAFETY: 该函数只会用 `value` 调用,而 `value` 具有正确的具体类型。
                    unsafe { core::mem::transmute(f) }
                },
                #[cfg(any(sanitize = "cfi", sanitize = "kcfi"))]
                formatter: |ptr: NonNull<()>, fmt: &mut Formatter<'_>| {
                    let func = $f;
                    // SAFETY: `ptr` 与 `value` 字段来源于同一个 `&T`,具体类型相同。
                    let r = unsafe { ptr.cast::<$t>().as_ref() };
                    (func)(r, fmt)
                },
                _lifetime: PhantomData,
            },
        }
    };
}

impl Argument<'_> {
    #[inline]
    pub const fn new_display<T: Display>(x: &T) -> Argument<'_> {
        argument_new!(T, x, <T as Display>::fmt)
    }
    #[inline]
    pub const fn new_debug<T: Debug>(x: &T) -> Argument<'_> {
        argument_new!(T, x, <T as Debug>::fmt)
    }
    #[inline]
    pub const fn new_debug_noop<T: Debug>(x: &T) -> Argument<'_> {
        argument_new!(T, x, |_: &T, _| Ok(()))
    }
    #[inline]
    pub const fn new_octal<T: Octal>(x: &T) -> Argument<'_> {
        argument_new!(T, x, <T as Octal>::fmt)
    }
    #[inline]
    pub const fn new_lower_hex<T: LowerHex>(x: &T) -> Argument<'_> {
        argument_new!(T, x, <T as LowerHex>::fmt)
    }
    #[inline]
    pub const fn new_upper_hex<T: UpperHex>(x: &T) -> Argument<'_> {
        argument_new!(T, x, <T as UpperHex>::fmt)
    }
    #[inline]
    pub const fn new_pointer<T: Pointer>(x: &T) -> Argument<'_> {
        argument_new!(T, x, <T as Pointer>::fmt)
    }
    #[inline]
    pub const fn new_binary<T: Binary>(x: &T) -> Argument<'_> {
        argument_new!(T, x, <T as Binary>::fmt)
    }
    #[inline]
    pub const fn new_lower_exp<T: LowerExp>(x: &T) -> Argument<'_> {
        argument_new!(T, x, <T as LowerExp>::fmt)
    }
    #[inline]
    pub const fn new_upper_exp<T: UpperExp>(x: &T) -> Argument<'_> {
        argument_new!(T, x, <T as UpperExp>::fmt)
    }
    #[inline]
    #[track_caller]
    pub const fn from_usize(x: &usize) -> Argument<'_> {
        if *x > u16::MAX as usize {
            panic!("Formatting argument out of range");
        }
        Argument { ty: ArgumentType::Count(*x as u16) }
    }

    /// 格式化这个占位符参数。
    ///
    /// # 安全性(Safety）
    ///
    /// 调用方必须保证该 `Argument` 实际上是占位符参数,而不是用于动态 width /
    /// precision 的计数参数。否则会把计数值当成格式化函数和值来解释,破坏
    /// `format_args!` 编译期建立的类型不变量。
    #[inline]
    pub(super) unsafe fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        match self.ty {
            // SAFETY:
            // 根据不变量,如果 `formatter` 的原始类型是 `fn(&T, _) -> _`,
            // 则 `value` 的类型就是 `&'b T`,其中 `'b` 是 `ArgumentType`
            // 的生命周期。又因为引用和 `NonNull` 在 ABI 上兼容,这里完全等价于
            // 用传给 `new` 的原始引用调用原始函数,因此是健全的。
            ArgumentType::Placeholder { formatter, value, .. } => unsafe { formatter(value, f) },
            // SAFETY: 调用方已承诺这里一定不是 `Count`。
            ArgumentType::Count(_) => unsafe { unreachable_unchecked() },
        }
    }

    #[inline]
    pub(super) const fn as_u16(&self) -> Option<u16> {
        match self.ty {
            ArgumentType::Count(count) => Some(count),
            ArgumentType::Placeholder { .. } => None,
        }
    }
}
