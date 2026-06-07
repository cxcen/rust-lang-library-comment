//! 可选值。
//!
//! [`Option`] 表示“可能有值，也可能没有值”的类型：每个 [`Option`] 要么是 [`Some`]，并且
//! 携带一个 `T`，要么是 [`None`]，并且不携带值。它把 C/C++ 中常用的空指针、哨兵值和
//! “调用者自己记住是否初始化”的约定显式放进类型系统，使编译器能强制调用方处理没有值的分支。
//!
//! 常见用途包括：延迟初始化的字段、部分函数的返回值、用 [`None`] 表示简单失败、可选结构体字段、
//! 可被暂时借出或 `take` 的字段、可选参数、可空指针抽象，以及在复杂控制流中把值安全地移出。
//!
//! ```
//! fn divide(numerator: f64, denominator: f64) -> Option<f64> {
//!     if denominator == 0.0 {
//!         None
//!     } else {
//!         Some(numerator / denominator)
//!     }
//! }
//!
//! let result = divide(2.0, 3.0);
//! match result {
//!     Some(x) => println!("Result: {x}"),
//!     None => println!("Cannot divide by 0"),
//! }
//! ```
//!
// FIXME: 展示 `Option` 在真实代码中的更多方法组合用法。
//
//! # 指针和可空值
//!
//! Rust 的引用必须总是指向有效位置，语言层没有“空引用”。需要表达“可能没有指向对象”时，使用
//! [`Option`] 包住指针或拥有所有权的指针，例如 <code>[Option]<[Box\<T>]></code>、`Option<&T>`
//! 或 `Option<NonNull<T>>`。这样调用方必须通过模式匹配、组合子或 `?` 明确处理 [`None`]，而不是
//! 在某个运行时路径上偶然解引用空指针。
//!
//! [Box\<T>]: ../../std/boxed/struct.Box.html
//!
//! # 问号运算符 `?`
//!
//! 当函数返回 [`Option`] 时，`?` 会把 [`Some`] 中的值取出继续计算；如果表达式结果是 [`None`]，
//! 则立即从外层函数返回 [`None`]。这是一种短路语义：后续表达式不会执行，也不会构造额外错误信息。
//!
//! ```
//! # #![allow(dead_code)]
//! fn add_last_numbers(stack: &mut Vec<i32>) -> Option<i32> {
//!     Some(stack.pop()? + stack.pop()?)
//! }
//! ```
//!
//! [`?`]: crate::ops::Try
//! [`Some`]: Some
//! [`None`]: None
//!
//! # 表示
//!
//! Rust 保证对若干类型 `T` 做空指针优化（null pointer optimization，NPO）：[`Option<T>`]
//! 与 `T` 具有相同大小、对齐和 [function call ABI]。这些保证依赖 `T` 的某个 niche 值，例如
//! 非空指针类型的空地址、[`num::NonZero*`] 的 0 值等；该 niche 被用来表示 [`None`]，因此
//! `Option<NonZeroUsize>`、`Option<&T>` 等在内存布局上可以做到零额外开销。
//!
//! 对下列类型，若 `T` 满足表中条件，则 `Option<T>` 与 `T` 有相同布局；在这些情况下，把有效的
//! `T` 转成 `Option<T>` 会得到 `Some(t)`，把 `Some(t)` 转回 `T` 会得到 `t`。
//!
//! | `T`                                                                 | `[0u8; size_of::<T>()]` 与 `Option::<T>::None` 互转是否 sound |
//! |---------------------------------------------------------------------|----------------------------------------------------------------|
//! | [`Box<U>`]（仅 `Box<U, Global>`）                                   | 当 `U: Sized` 时                                               |
//! | `&U`                                                                | 当 `U: Sized` 时                                               |
//! | `&mut U`                                                            | 当 `U: Sized` 时                                               |
//! | `fn`, `extern "C" fn`[^extern_fn]                                  | 总是                                                           |
//! | [`num::NonZero*`]                                                   | 总是                                                           |
//! | [`ptr::NonNull<U>`]                                                 | 当 `U: Sized` 时                                               |
//! | 包住这些类型之一的 `#[repr(transparent)]` 结构体                    | 当内部类型满足条件时                                           |
//!
//! [^extern_fn]: 对 `unsafe` 变体、任意参数/返回类型以及任意 ABI 都成立：`[unsafe] extern "abi" fn`，例如 `extern "system" fn`。
//!
//! 在某些条件下，上述类型 `T` 包在 [`Result`][result_repr] 中也能获得类似优化。需要特别注意：
//! 对上述类型，可以从所有有效的 `T` 通过 [`mem::transmute`] 得到 `Option<T>`，也可以从
//! `Some::<T>(_)` 转回 `T`；但把 `None::<T>` 转成 `T` 是未定义行为(UB)，因为那会构造出
//! `T` 明确禁止的 niche 值。
//!
//! [`Box<U>`]: ../../std/boxed/struct.Box.html
//! [`num::NonZero*`]: crate::num
//! [`ptr::NonNull<U>`]: crate::ptr::NonNull
//! [function call ABI]: ../primitive.fn.html#abi-compatibility
//! [result_repr]: crate::result#representation
//!
//! # 方法概览
//!
//! [`Option`] 的方法大致分为几类：查询变体（[`is_some`]、[`is_none`]、[`is_some_and`]、
//! [`is_none_or`]）、把 `Option<T>` 变成引用或切片（[`as_ref`]、[`as_mut`]、[`as_slice`]、
//! [`as_mut_slice`]、pin 相关适配器）、提取值（[`unwrap`]、[`expect`]、`unwrap_or*`）、
//! 转换值（[`map`]、[`and_then`]、[`filter`]、[`transpose`]）、以及原地插入/取出（[`insert`]、
//! [`take`]、[`replace`]）。
//!
//! [`unwrap`] 和 [`expect`] 只在 `self` 为 [`Some`] 时返回内部值；当 `self` 为 [`None`] 时会
//! panic，并通过 `#[track_caller]` 把位置报告到调用点。[`expect`] 使用调用方提供的消息，
//! [`unwrap`] 使用通用消息。相对地，[`unwrap_unchecked`] 不检查变体；调用方必须保证值为
//! [`Some`]，否则立即违反 unsafe 前置条件并造成 UB。
//!
//! 组合子遵循短路规则：[`map`] 只处理 [`Some`]，[`and_then`] 只在 [`Some`] 时调用闭包并可改变
//! 内部类型，[`or_else`] 只在 [`None`] 时延迟计算替代值，[`ok_or_else`] 只在 [`None`] 时构造
//! 错误。理解这些立即/惰性求值区别有助于避免无谓计算或意外移动。
//!
//! [`and_then`]: Option::and_then
//! [`as_mut`]: Option::as_mut
//! [`as_mut_slice`]: Option::as_mut_slice
//! [`as_ref`]: Option::as_ref
//! [`as_slice`]: Option::as_slice
//! [`expect`]: Option::expect
//! [`filter`]: Option::filter
//! [`insert`]: Option::insert
//! [`is_none`]: Option::is_none
//! [`is_none_or`]: Option::is_none_or
//! [`is_some`]: Option::is_some
//! [`is_some_and`]: Option::is_some_and
//! [`map`]: Option::map
//! [`ok_or_else`]: Option::ok_or_else
//! [`or_else`]: Option::or_else
//! [`replace`]: Option::replace
//! [`take`]: Option::take
//! [`transpose`]: Option::transpose
//! [`unwrap`]: Option::unwrap
//! [`unwrap_unchecked`]: Option::unwrap_unchecked
//!
//! # 示例
//!
//! ```
//! let msg = Some("howdy");
//! if let Some(m) = &msg {
//!     println!("{}", *m);
//! }
//! let unwrapped_msg = msg.unwrap_or("default message");
//! ```

#![stable(feature = "rust1", since = "1.0.0")]

use crate::clone::TrivialClone;
use crate::iter::{self, FusedIterator, TrustedLen};
use crate::marker::Destruct;
use crate::ops::{self, ControlFlow, Deref, DerefMut};
use crate::panicking::{panic, panic_display};
use crate::pin::Pin;
use crate::{cmp, convert, hint, mem, slice};

/// `Option` 类型。更多设计背景见本模块级文档。
///
/// `Option<T>` 是编译器识别的基础枚举，`Some(T)` 表示有值，`None` 表示无值。
#[doc(search_unbox)]
#[derive(Copy, Debug, Hash)]
#[derive_const(Eq)]
#[rustc_diagnostic_item = "Option"]
#[lang = "Option"]
#[stable(feature = "rust1", since = "1.0.0")]
#[allow(clippy::derived_hash_with_manual_eq)] // PartialEq 已用等价方式手动实现。
pub enum Option<T> {
    /// 没有值。
    ///
    /// 该变体表示缺失、失败或尚未初始化；它不携带 `T`，也不会构造或 drop `T`。
    #[lang = "None"]
    #[stable(feature = "rust1", since = "1.0.0")]
    None,
    /// 携带一个类型为 `T` 的值。
    ///
    /// 该变体表示值存在，提取、映射和组合子通常只在此分支继续处理内部值。
    #[lang = "Some"]
    #[stable(feature = "rust1", since = "1.0.0")]
    Some(#[stable(feature = "rust1", since = "1.0.0")] T),
}

/////////////////////////////////////////////////////////////////////////////
// 类型实现
/////////////////////////////////////////////////////////////////////////////

impl<T> Option<T> {
    /////////////////////////////////////////////////////////////////////////
    // 查询包含的值
    /////////////////////////////////////////////////////////////////////////

    /// 如果 `self` 是 [`Some`]，返回 `true`。
    ///
    /// 此方法只查询变体，不移动内部值；需要断言一定有值时，应使用模式匹配、[`expect`] 或 [`unwrap`]。
    #[must_use = "if you intended to assert that this has a value, consider `.unwrap()` instead"]
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_option_basics", since = "1.48.0")]
    pub const fn is_some(&self) -> bool {
        matches!(*self, Some(_))
    }

    /// 如果 `self` 是 [`Some`] 且内部值满足谓词 `f`，返回 `true`。
    ///
    /// 当 `self` 为 [`None`] 时闭包不会执行，并直接返回 `false`；这是按需求值的查询组合子。
    #[must_use]
    #[inline]
    #[stable(feature = "is_some_and", since = "1.70.0")]
    #[rustc_const_unstable(feature = "const_option_ops", issue = "143956")]
    pub const fn is_some_and(self, f: impl [const] FnOnce(T) -> bool + [const] Destruct) -> bool {
        match self {
            None => false,
            Some(x) => f(x),
        }
    }

    /// 如果 `self` 是 [`None`]，返回 `true`。
    ///
    /// 此方法只查询变体，不移动内部值。
    #[must_use = "if you intended to assert that this doesn't have a value, consider \
                  wrapping this in an `assert!()` instead"]
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_option_basics", since = "1.48.0")]
    pub const fn is_none(&self) -> bool {
        !self.is_some()
    }

    /// 如果 `self` 是 [`None`]，或 `self` 是 [`Some`] 且内部值满足谓词 `f`，返回 `true`。
    ///
    /// 当 `self` 为 [`None`] 时闭包不会执行；当有值时才把 `T` 交给闭包检查。
    #[must_use]
    #[inline]
    #[stable(feature = "is_none_or", since = "1.82.0")]
    #[rustc_const_unstable(feature = "const_option_ops", issue = "143956")]
    pub const fn is_none_or(self, f: impl [const] FnOnce(T) -> bool + [const] Destruct) -> bool {
        match self {
            None => true,
            Some(x) => f(x),
        }
    }

    /////////////////////////////////////////////////////////////////////////
    // 引用适配器
    /////////////////////////////////////////////////////////////////////////

    /// 把 `&Option<T>` 转换为 `Option<&T>`。
    ///
    /// 该方法保留原 `Option` 的所有权，只把内部值重新借用出来，适合在不移动 `T` 的情况下调用组合子。
    #[inline]
    #[rustc_const_stable(feature = "const_option_basics", since = "1.48.0")]
    #[stable(feature = "rust1", since = "1.0.0")]
    pub const fn as_ref(&self) -> Option<&T> {
        match *self {
            Some(ref x) => Some(x),
            None => None,
        }
    }

    /// 把 `&mut Option<T>` 转换为 `Option<&mut T>`。
    ///
    /// 该方法保留外层 `Option`，只在为 [`Some`] 时提供内部值的唯一可变借用。
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_option", since = "1.83.0")]
    pub const fn as_mut(&mut self) -> Option<&mut T> {
        match *self {
            Some(ref mut x) => Some(x),
            None => None,
        }
    }

    /// 把 `Pin<&Option<T>>` 转换为 `Option<Pin<&T>>`。
    ///
    /// 如果有值，返回指向内部值的 pinned 共享引用；如果为 [`None`]，返回 [`None`]。
    #[inline]
    #[must_use]
    #[stable(feature = "pin", since = "1.33.0")]
    #[rustc_const_stable(feature = "const_option_ext", since = "1.84.0")]
    pub const fn as_pin_ref(self: Pin<&Self>) -> Option<Pin<&T>> {
        // FIXME(const-hack): 等 `map` 可用于这里后，改回使用 `map`。
        match Pin::get_ref(self).as_ref() {
            // SAFETY: `x` 来自已经 pinned 的 `self`，因此它同样保证处于 pinned 状态。
            Some(x) => unsafe { Some(Pin::new_unchecked(x)) },
            None => None,
        }
    }

    /// 把 `Pin<&mut Option<T>>` 转换为 `Option<Pin<&mut T>>`。
    ///
    /// 该投影不会移动内部值，因此在 `Option` 已经被 pin 住时可以安全地访问其中的 `T`。
    #[inline]
    #[must_use]
    #[stable(feature = "pin", since = "1.33.0")]
    #[rustc_const_stable(feature = "const_option_ext", since = "1.84.0")]
    pub const fn as_pin_mut(self: Pin<&mut Self>) -> Option<Pin<&mut T>> {
        // SAFETY: `get_unchecked_mut` 不会用来移动 `self` 内部的 `Option`。
        // `x` 来自已经 pinned 的 `self`，因此它同样保证处于 pinned 状态。
        unsafe {
            // FIXME(const-hack): 等 `map` 可用于这里后，改回使用 `map`。
            match Pin::get_unchecked_mut(self).as_mut() {
                Some(x) => Some(Pin::new_unchecked(x)),
                None => None,
            }
        }
    }

    #[inline]
    const fn len(&self) -> usize {
        // 使用 intrinsic 可避免为了得到 0 或 1 而生成分支。
        let discriminant: isize = crate::intrinsics::discriminant_value(self);
        discriminant as usize
    }

    /// 以切片形式查看可能存在的单个值。
    ///
    /// 当 `self` 为 [`Some`] 时返回长度为 1 的切片；为 [`None`] 时返回空切片。这样可把 `Option<T>` 当作 0 或 1 个元素的序列处理。
    #[inline]
    #[must_use]
    #[stable(feature = "option_as_slice", since = "1.75.0")]
    #[rustc_const_stable(feature = "const_option_ext", since = "1.84.0")]
    pub const fn as_slice(&self) -> &[T] {
        // SAFETY: 当 `Option` 为 `Some` 时，这里使用的是指向 payload 的真实指针，
        // 且长度为 1，因此等价于 `slice::from_ref`。
        // 当 `Option` 为 `None` 时，长度为 0；此时只需要指针对齐即可。`&self`
        // 已经对齐，并且所用偏移是对齐量的倍数。
        //
        // 这里假设 `offset_of!` 总是返回一个对 `T` 来说位于对象范围内且正确对齐的位置；
        // 即使在 `None` 分支中该位置只是 padding。
        unsafe {
            slice::from_raw_parts(
                (self as *const Self).byte_add(core::mem::offset_of!(Self, Some.0)).cast(),
                self.len(),
            )
        }
    }

    /// 以可变切片形式查看可能存在的单个值。
    ///
    /// 当 `self` 为 [`Some`] 时返回长度为 1 的可变切片；为 [`None`] 时返回空切片。
    #[inline]
    #[must_use]
    #[stable(feature = "option_as_slice", since = "1.75.0")]
    #[rustc_const_stable(feature = "const_option_ext", since = "1.84.0")]
    pub const fn as_mut_slice(&mut self) -> &mut [T] {
        // SAFETY: 当 `Option` 为 `Some` 时，这里使用的是指向 payload 的真实指针，
        // 且长度为 1，因此等价于 `slice::from_mut`。
        // 当 `Option` 为 `None` 时，长度为 0；此时只需要指针对齐即可。`&self`
        // 已经对齐，并且所用偏移是对齐量的倍数。
        //
        // 在当前实现中，intrinsic 会从可变引用创建 `*const T`，因此这里把它转回
        // 可变指针是安全的。与 `as_slice` 一样，intrinsic 总是返回一个对 `T` 来说
        // 位于对象范围内且正确对齐的位置；即使在 `None` 分支中该位置只是 padding。
        unsafe {
            slice::from_raw_parts_mut(
                (self as *mut Self).byte_add(core::mem::offset_of!(Self, Some.0)).cast(),
                self.len(),
            )
        }
    }

    /////////////////////////////////////////////////////////////////////////
    // 取得包含的值
    /////////////////////////////////////////////////////////////////////////

    /// 消费 `self` 并返回其中的 [`Some`] 值。
    ///
    /// # Panics
    ///
    /// 如果 `self` 为 [`None`]，以调用方提供的 `msg` 触发 panic。`#[track_caller]` 会把 panic 位置报告到调用 `expect` 的位置，而不是库内部。
    #[inline]
    #[track_caller]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_diagnostic_item = "option_expect"]
    #[rustc_allow_const_fn_unstable(const_precise_live_drops)]
    #[rustc_const_stable(feature = "const_option", since = "1.83.0")]
    pub const fn expect(self, msg: &str) -> T {
        match self {
            Some(val) => val,
            None => expect_failed(msg),
        }
    }

    /// 消费 `self` 并返回其中的 [`Some`] 值。
    ///
    /// # Panics
    ///
    /// 如果 `self` 为 [`None`]，以通用消息触发 panic。需要给 panic 消息补充业务上下文时，应使用 [`expect`]。
    #[inline(always)]
    #[track_caller]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_diagnostic_item = "option_unwrap"]
    #[rustc_allow_const_fn_unstable(const_precise_live_drops)]
    #[rustc_const_stable(feature = "const_option", since = "1.83.0")]
    pub const fn unwrap(self) -> T {
        match self {
            Some(val) => val,
            None => unwrap_failed(),
        }
    }

    /// 消费 `self`；若为 [`Some`]，返回内部值，否则返回给定的 `default`。
    ///
    /// `default` 会在调用前被求值；如果默认值构造有成本，优先使用 [`unwrap_or_else`]。
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_allow_const_fn_unstable(const_precise_live_drops)]
    #[rustc_const_unstable(feature = "const_option_ops", issue = "143956")]
    pub const fn unwrap_or(self, default: T) -> T
    where
        T: [const] Destruct,
    {
        match self {
            Some(x) => x,
            None => default,
        }
    }

    /// 消费 `self`；若为 [`Some`]，返回内部值，否则调用闭包 `f` 计算默认值。
    ///
    /// 闭包只在 [`None`] 分支执行，适合把昂贵计算或需要移动的后备值延迟到确实需要时。
    #[inline]
    #[track_caller]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_unstable(feature = "const_option_ops", issue = "143956")]
    pub const fn unwrap_or_else<F>(self, f: F) -> T
    where
        F: [const] FnOnce() -> T + [const] Destruct,
    {
        match self {
            Some(x) => x,
            None => f(),
        }
    }

    /// 消费 `self`；若为 [`Some`]，返回内部值，否则返回 `T::default()`。
    ///
    /// 该方法要求 `T: Default`，并只在 [`None`] 分支构造默认值。
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_unstable(feature = "const_option_ops", issue = "143956")]
    pub const fn unwrap_or_default(self) -> T
    where
        T: [const] Default,
    {
        match self {
            Some(x) => x,
            None => T::default(),
        }
    }

    /// 消费 `self` 并在不检查变体的情况下返回内部 [`Some`] 值。
    ///
    /// # 安全性(Safety）
    ///
    /// 调用方必须保证 `self` 确实是 [`Some`]。如果传入 [`None`]，会调用 `unreachable_unchecked` 路径，编译器可据此假设该分支不可能发生；违反此前置条件是未定义行为(UB)。
    #[inline]
    #[track_caller]
    #[stable(feature = "option_result_unwrap_unchecked", since = "1.58.0")]
    #[rustc_allow_const_fn_unstable(const_precise_live_drops)]
    #[rustc_const_stable(feature = "const_option", since = "1.83.0")]
    pub const unsafe fn unwrap_unchecked(self) -> T {
        match self {
            Some(val) => val,
            // SAFETY: 调用方必须保证 `self` 是 `Some`，因此 `None` 分支不可达。
            None => unsafe { hint::unreachable_unchecked() },
        }
    }

    /////////////////////////////////////////////////////////////////////////
    // 转换包含的值
    /////////////////////////////////////////////////////////////////////////

    /// 如果 `self` 为 [`Some`]，把内部值交给函数 `f` 并返回 `Some(f(value))`；如果为 [`None`]，保持 [`None`]。
    ///
    /// 该方法消费原 `Option`，常用于只转换存在的值而保留缺失状态。
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_unstable(feature = "const_option_ops", issue = "143956")]
    pub const fn map<U, F>(self, f: F) -> Option<U>
    where
        F: [const] FnOnce(T) -> U + [const] Destruct,
    {
        match self {
            Some(x) => Some(f(x)),
            None => None,
        }
    }

    /// 如果 `self` 为 [`Some`]，以共享引用形式把内部值传给闭包 `f`，然后返回原 `Option`。
    ///
    /// 该方法适合插入调试、日志或断言逻辑，不改变成功/缺失状态，也不把内部值交给闭包所有权。
    #[inline]
    #[stable(feature = "result_option_inspect", since = "1.76.0")]
    #[rustc_const_unstable(feature = "const_option_ops", issue = "143956")]
    pub const fn inspect<F>(self, f: F) -> Self
    where
        F: [const] FnOnce(&T) + [const] Destruct,
    {
        if let Some(ref x) = self {
            f(x);
        }

        self
    }

    /// 把 `Option<T>` 转换为普通值 `U`。
    ///
    /// 若为 [`Some`]，对内部值执行 `f`；若为 [`None`]，返回已经求值的 `default`。默认值总会在调用前求值。
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[must_use = "if you don't need the returned value, use `if let` instead"]
    #[rustc_const_unstable(feature = "const_option_ops", issue = "143956")]
    pub const fn map_or<U, F>(self, default: U, f: F) -> U
    where
        F: [const] FnOnce(T) -> U + [const] Destruct,
        U: [const] Destruct,
    {
        match self {
            Some(t) => f(t),
            None => default,
        }
    }

    /// 把 `Option<T>` 转换为普通值 `U`，并延迟计算默认值。
    ///
    /// 若为 [`Some`]，对内部值执行 `f`；若为 [`None`]，才调用 `default` 闭包。
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_unstable(feature = "const_option_ops", issue = "143956")]
    pub const fn map_or_else<U, D, F>(self, default: D, f: F) -> U
    where
        D: [const] FnOnce() -> U + [const] Destruct,
        F: [const] FnOnce(T) -> U + [const] Destruct,
    {
        match self {
            Some(t) => f(t),
            None => default(),
        }
    }

    /// 把 `Option<T>` 转换为 `U`。
    ///
    /// 若为 [`Some`]，对内部值执行 `f`；若为 [`None`]，返回 `U::default()`。
    #[inline]
    #[unstable(feature = "result_option_map_or_default", issue = "138099")]
    #[rustc_const_unstable(feature = "const_option_ops", issue = "143956")]
    pub const fn map_or_default<U, F>(self, f: F) -> U
    where
        U: [const] Default,
        F: [const] FnOnce(T) -> U + [const] Destruct,
    {
        match self {
            Some(t) => f(t),
            None => U::default(),
        }
    }

    /// 把 `Option<T>` 转换为 [`Result<T, E>`]。
    ///
    /// [`Some(v)`] 变为 [`Ok(v)`]；[`None`] 变为使用给定 `err` 的 [`Err(err)`]。`err` 会在调用前求值。
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_unstable(feature = "const_option_ops", issue = "143956")]
    pub const fn ok_or<E: [const] Destruct>(self, err: E) -> Result<T, E> {
        match self {
            Some(v) => Ok(v),
            None => Err(err),
        }
    }

    /// 把 `Option<T>` 转换为 [`Result<T, E>`]，并延迟构造错误。
    ///
    /// [`Some(v)`] 变为 [`Ok(v)`]；[`None`] 时才调用闭包生成 [`Err`]，适合错误构造有成本的场景。
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_unstable(feature = "const_option_ops", issue = "143956")]
    pub const fn ok_or_else<E, F>(self, err: F) -> Result<T, E>
    where
        F: [const] FnOnce() -> E + [const] Destruct,
    {
        match self {
            Some(v) => Ok(v),
            None => Err(err()),
        }
    }

    /// 把 `Option<T>` 或 `&Option<T>` 转换为 `Option<&T::Target>`。
    ///
    /// 该方法结合 [`Deref`]，常用于把 `Option<String>`、`Option<Box<T>>` 等转换为目标类型的借用。
    #[inline]
    #[stable(feature = "option_deref", since = "1.40.0")]
    #[rustc_const_unstable(feature = "const_convert", issue = "143773")]
    pub const fn as_deref(&self) -> Option<&T::Target>
    where
        T: [const] Deref,
    {
        self.as_ref().map(Deref::deref)
    }

    /// 把 `Option<T>` 或 `&mut Option<T>` 转换为 `Option<&mut T::Target>`。
    ///
    /// 该方法结合 [`DerefMut`]，在存在值时返回目标类型的可变借用。
    #[inline]
    #[stable(feature = "option_deref", since = "1.40.0")]
    #[rustc_const_unstable(feature = "const_convert", issue = "143773")]
    pub const fn as_deref_mut(&mut self) -> Option<&mut T::Target>
    where
        T: [const] DerefMut,
    {
        self.as_mut().map(DerefMut::deref_mut)
    }

    /////////////////////////////////////////////////////////////////////////
    // 迭代器构造器
    /////////////////////////////////////////////////////////////////////////

    /// 返回遍历可能存在值的迭代器。
    ///
    /// 若为 [`Some`]，迭代器产生一个 `&T`；若为 [`None`]，迭代器为空。
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn iter(&self) -> Iter<'_, T> {
        Iter { inner: Item { opt: self.as_ref() } }
    }

    /// 返回遍历可能存在值的可变迭代器。
    ///
    /// 若为 [`Some`]，迭代器产生一个 `&mut T`；若为 [`None`]，迭代器为空。
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn iter_mut(&mut self) -> IterMut<'_, T> {
        IterMut { inner: Item { opt: self.as_mut() } }
    }

    /////////////////////////////////////////////////////////////////////////
    // 对值执行立即/惰性布尔组合
    /////////////////////////////////////////////////////////////////////////

    /// 如果 `self` 为 [`None`]，返回 [`None`]；否则返回参数 `optb`。
    ///
    /// `optb` 会在调用前求值；需要在 [`Some`] 分支延迟计算时使用 [`and_then`]。
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_unstable(feature = "const_option_ops", issue = "143956")]
    pub const fn and<U>(self, optb: Option<U>) -> Option<U>
    where
        T: [const] Destruct,
        U: [const] Destruct,
    {
        match self {
            Some(_) => optb,
            None => None,
        }
    }

    /// 如果 `self` 为 [`Some`]，把内部值交给闭包 `f` 并返回其结果；如果为 [`None`]，直接返回 [`None`]。
    ///
    /// 这是 `Option` 的扁平映射组合子，适合串联多个可能失败的计算，并在任一步为 [`None`] 时短路。
    #[doc(alias = "flatmap")]
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_confusables("flat_map", "flatmap")]
    #[rustc_const_unstable(feature = "const_option_ops", issue = "143956")]
    pub const fn and_then<U, F>(self, f: F) -> Option<U>
    where
        F: [const] FnOnce(T) -> Option<U> + [const] Destruct,
    {
        match self {
            Some(x) => f(x),
            None => None,
        }
    }

    /// 如果 `self` 为 [`Some`]，用谓词检查内部值；谓词返回 `true` 时保留该值，否则返回 [`None`]。
    ///
    /// 如果 `self` 为 [`None`]，谓词不会执行。
    #[inline]
    #[stable(feature = "option_filter", since = "1.27.0")]
    #[rustc_const_unstable(feature = "const_option_ops", issue = "143956")]
    pub const fn filter<P>(self, predicate: P) -> Self
    where
        P: [const] FnOnce(&T) -> bool + [const] Destruct,
        T: [const] Destruct,
    {
        if let Some(x) = self {
            if predicate(&x) {
                return Some(x);
            }
        }
        None
    }

    /// 如果 `self` 为 [`Some`]，返回 `self`；否则返回参数 `optb`。
    ///
    /// `optb` 会在调用前求值；需要延迟构造替代值时使用 [`or_else`]。
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_unstable(feature = "const_option_ops", issue = "143956")]
    pub const fn or(self, optb: Option<T>) -> Option<T>
    where
        T: [const] Destruct,
    {
        match self {
            x @ Some(_) => x,
            None => optb,
        }
    }

    /// 如果 `self` 为 [`Some`]，返回 `self`；否则调用闭包 `f` 生成替代 `Option`。
    ///
    /// 闭包只在 [`None`] 分支执行。
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_unstable(feature = "const_option_ops", issue = "143956")]
    pub const fn or_else<F>(self, f: F) -> Option<T>
    where
        F: [const] FnOnce() -> Option<T> + [const] Destruct,
        // FIXME(const_hack): 这里的 `T: [const] Destruct` 实际不必要，但即使精确 live-drop
        // 分析也还无法证明此处不会 drop 任何 `T` 类型的值。
        T: [const] Destruct,
    {
        match self {
            x @ Some(_) => x,
            None => f(),
        }
    }

    /// 当 `self` 和 `optb` 中恰好一个是 [`Some`] 时返回那个 [`Some`]；两者都为 [`Some`] 或都为 [`None`] 时返回 [`None`]。
    #[inline]
    #[stable(feature = "option_xor", since = "1.37.0")]
    #[rustc_const_unstable(feature = "const_option_ops", issue = "143956")]
    pub const fn xor(self, optb: Option<T>) -> Option<T>
    where
        T: [const] Destruct,
    {
        match (self, optb) {
            (a @ Some(_), None) => a,
            (None, b @ Some(_)) => b,
            _ => None,
        }
    }

    /////////////////////////////////////////////////////////////////////////
    // 类 entry 操作：插入值并返回引用
    /////////////////////////////////////////////////////////////////////////

    /// 把 `value` 插入到 `Option` 中，并返回指向新值的可变引用。
    ///
    /// 如果原来已经是 [`Some`]，旧值会先被 drop。
    #[must_use = "if you intended to set a value, consider assignment instead"]
    #[inline]
    #[stable(feature = "option_insert", since = "1.53.0")]
    #[rustc_const_unstable(feature = "const_option_ops", issue = "143956")]
    pub const fn insert(&mut self, value: T) -> &mut T
    where
        T: [const] Destruct,
    {
        *self = Some(value);

        // SAFETY: 上面的赋值刚刚把该 `Option` 填成 `Some`。
        unsafe { self.as_mut().unwrap_unchecked() }
    }

    /// 如果 `self` 为 [`None`]，插入给定 `value`；随后返回内部值的可变引用。
    ///
    /// `value` 会在调用前求值；需要延迟构造时使用 [`get_or_insert_with`]。
    #[inline]
    #[stable(feature = "option_entry", since = "1.20.0")]
    pub fn get_or_insert(&mut self, value: T) -> &mut T {
        self.get_or_insert_with(|| value)
    }

    /// 如果 `self` 为 [`None`]，插入 `T::default()`；随后返回内部值的可变引用。
    #[inline]
    #[stable(feature = "option_get_or_insert_default", since = "1.83.0")]
    #[rustc_const_unstable(feature = "const_option_ops", issue = "143956")]
    pub const fn get_or_insert_default(&mut self) -> &mut T
    where
        T: [const] Default + [const] Destruct,
    {
        self.get_or_insert_with(T::default)
    }

    /// 如果 `self` 为 [`None`]，调用闭包 `f` 生成并插入值；随后返回内部值的可变引用。
    ///
    /// 闭包只在缺值时执行。
    #[inline]
    #[stable(feature = "option_entry", since = "1.20.0")]
    #[rustc_const_unstable(feature = "const_option_ops", issue = "143956")]
    pub const fn get_or_insert_with<F>(&mut self, f: F) -> &mut T
    where
        F: [const] FnOnce() -> T + [const] Destruct,
        T: [const] Destruct,
    {
        if let None = self {
            *self = Some(f());
        }

        // SAFETY: 如果 `self` 原本是 `None`，上面的代码已经把它替换为 `Some`。
        unsafe { self.as_mut().unwrap_unchecked() }
    }

    /////////////////////////////////////////////////////////////////////////
    // 其他操作
    /////////////////////////////////////////////////////////////////////////

    /// 从 `Option` 中取出值，并在原位置留下 [`None`]。
    ///
    /// 若原来为 [`Some`]，返回原值；若原来为 [`None`]，返回 [`None`]。
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_option", since = "1.83.0")]
    pub const fn take(&mut self) -> Option<T> {
        // FIXME(const-hack): 等 `mem::take` 支持 const 后，用它替换 `mem::replace`。
        mem::replace(self, None)
    }

    /// 仅当谓词对内部值返回 `true` 时，从 `Option` 中取出值并留下 [`None`]。
    ///
    /// 谓词接收 `&mut T`，因此可以在决定是否取出前检查或修改内部值。
    #[inline]
    #[stable(feature = "option_take_if", since = "1.80.0")]
    #[rustc_const_unstable(feature = "const_option_ops", issue = "143956")]
    pub const fn take_if<P>(&mut self, predicate: P) -> Option<T>
    where
        P: [const] FnOnce(&mut T) -> bool + [const] Destruct,
    {
        if self.as_mut().map_or(false, predicate) { self.take() } else { None }
    }

    /// 用给定 `value` 替换 `Option` 当前内容，并返回旧内容。
    ///
    /// 替换后 `self` 一定为 [`Some(value)`]；旧值如果存在会作为返回值交还给调用方，而不是在此处 drop。
    #[inline]
    #[stable(feature = "option_replace", since = "1.31.0")]
    #[rustc_const_stable(feature = "const_option", since = "1.83.0")]
    pub const fn replace(&mut self, value: T) -> Option<T> {
        mem::replace(self, Some(value))
    }

    /// 把两个 `Option` 合并。
    ///
    /// 只有当两者都是 [`Some`] 时返回 `Some((a, b))`；任一为 [`None`] 时返回 [`None`]。
    #[stable(feature = "option_zip_option", since = "1.46.0")]
    #[rustc_const_unstable(feature = "const_option_ops", issue = "143956")]
    pub const fn zip<U>(self, other: Option<U>) -> Option<(T, U)>
    where
        T: [const] Destruct,
        U: [const] Destruct,
    {
        match (self, other) {
            (Some(a), Some(b)) => Some((a, b)),
            _ => None,
        }
    }

    /// 把两个 `Option` 合并，并在两者都是 [`Some`] 时用函数 `f` 计算结果。
    ///
    /// 如果任一输入为 [`None`]，函数不会执行并返回 [`None`]。
    #[unstable(feature = "option_zip", issue = "70086")]
    #[rustc_const_unstable(feature = "const_option_ops", issue = "143956")]
    pub const fn zip_with<U, F, R>(self, other: Option<U>, f: F) -> Option<R>
    where
        F: [const] FnOnce(T, U) -> R + [const] Destruct,
        T: [const] Destruct,
        U: [const] Destruct,
    {
        match (self, other) {
            (Some(a), Some(b)) => Some(f(a, b)),
            _ => None,
        }
    }

    /// 把两个 `Option` 归约为一个。
    ///
    /// 两者都是 [`Some`] 时调用函数合并；只有一边是 [`Some`] 时返回那一边；两边都是 [`None`] 时返回 [`None`]。
    #[unstable(feature = "option_reduce", issue = "144273")]
    pub fn reduce<U, R, F>(self, other: Option<U>, f: F) -> Option<R>
    where
        T: Into<R>,
        U: Into<R>,
        F: FnOnce(T, U) -> R,
    {
        match (self, other) {
            (Some(a), Some(b)) => Some(f(a, b)),
            (Some(a), _) => Some(a.into()),
            (_, Some(b)) => Some(b.into()),
            _ => None,
        }
    }
}

impl<T: IntoIterator> Option<T> {
    /// 把可选迭代器转换为迭代器。
    ///
    /// 当 `self` 为 [`Some(iterable)`] 时迭代其中元素；当为 [`None`] 时产生空迭代器。
    #[unstable(feature = "option_into_flat_iter", issue = "148441")]
    pub fn into_flat_iter<A>(self) -> OptionFlatten<A>
    where
        T: IntoIterator<IntoIter = A>,
    {
        OptionFlatten { iter: self.map(IntoIterator::into_iter) }
    }
}

impl<T, U> Option<(T, U)> {
    /// 把包含二元组的 `Option<(T, U)>` 拆成 `(Option<T>, Option<U>)`。
    ///
    /// 若原值为 [`Some((a, b))`]，结果为 `(Some(a), Some(b))`；若为 [`None`]，两边都为 [`None`]。
    #[inline]
    #[stable(feature = "unzip_option", since = "1.66.0")]
    pub fn unzip(self) -> (Option<T>, Option<U>) {
        match self {
            Some((a, b)) => (Some(a), Some(b)),
            None => (None, None),
        }
    }
}

impl<T> Option<&T> {
    /// 把 `Option<&T>` 或 `Option<&mut T>` 转换为 `Option<T>`，通过复制内部值完成转换。
    ///
    /// 该方法要求 `T: Copy`。
    #[must_use = "`self` will be dropped if the result is not used"]
    #[stable(feature = "copied", since = "1.35.0")]
    #[rustc_const_stable(feature = "const_option", since = "1.83.0")]
    pub const fn copied(self) -> Option<T>
    where
        T: Copy,
    {
        // FIXME(const-hack): 这个实现绕开了尚未 const-ready 的 `Option::map`；
        // 等可行时应改回去以避免重复代码。
        match self {
            Some(&v) => Some(v),
            None => None,
        }
    }

    /// 把 `Option<&T>` 或 `Option<&mut T>` 转换为 `Option<T>`，通过克隆内部值完成转换。
    ///
    /// 该方法要求 `T: Clone`，并只在 [`Some`] 分支调用 `clone`。
    #[must_use = "`self` will be dropped if the result is not used"]
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn cloned(self) -> Option<T>
    where
        T: Clone,
    {
        match self {
            Some(t) => Some(t.clone()),
            None => None,
        }
    }
}

impl<T> Option<&mut T> {
    /// 把 `Option<&T>` 或 `Option<&mut T>` 转换为 `Option<T>`，通过复制内部值完成转换。
    ///
    /// 该方法要求 `T: Copy`。
    #[must_use = "`self` will be dropped if the result is not used"]
    #[stable(feature = "copied", since = "1.35.0")]
    #[rustc_const_stable(feature = "const_option", since = "1.83.0")]
    pub const fn copied(self) -> Option<T>
    where
        T: Copy,
    {
        match self {
            Some(&mut t) => Some(t),
            None => None,
        }
    }

    /// 把 `Option<&T>` 或 `Option<&mut T>` 转换为 `Option<T>`，通过克隆内部值完成转换。
    ///
    /// 该方法要求 `T: Clone`，并只在 [`Some`] 分支调用 `clone`。
    #[must_use = "`self` will be dropped if the result is not used"]
    #[stable(since = "1.26.0", feature = "option_ref_mut_cloned")]
    pub fn cloned(self) -> Option<T>
    where
        T: Clone,
    {
        match self {
            Some(t) => Some(t.clone()),
            None => None,
        }
    }
}

impl<T, E> Option<Result<T, E>> {
    /// 在 `Option` 与另一层容器之间交换嵌套顺序。
    ///
    /// 对 `Option<Result<T, E>>`，`Some(Ok(v))` 变为 `Ok(Some(v))`，`Some(Err(e))` 变为 `Err(e)`，`None` 变为 `Ok(None)`。对数组形式则把 `[Option<T>; N]` 转为 `Option<[T; N]>`，任一元素为 [`None`] 即整体为 [`None`]。
    #[inline]
    #[stable(feature = "transpose_result", since = "1.33.0")]
    #[rustc_allow_const_fn_unstable(const_precise_live_drops)]
    #[rustc_const_stable(feature = "const_option", since = "1.83.0")]
    pub const fn transpose(self) -> Result<Option<T>, E> {
        match self {
            Some(Ok(x)) => Ok(Some(x)),
            Some(Err(e)) => Err(e),
            None => Ok(None),
        }
    }
}

#[cfg_attr(not(panic = "immediate-abort"), inline(never))]
#[cfg_attr(panic = "immediate-abort", inline)]
#[cold]
#[track_caller]
const fn unwrap_failed() -> ! {
    panic("called `Option::unwrap()` on a `None` value")
}

// 单独拆出此函数，用于减小 .expect() 自身的代码体积。
#[cfg_attr(not(panic = "immediate-abort"), inline(never))]
#[cfg_attr(panic = "immediate-abort", inline)]
#[cold]
#[track_caller]
const fn expect_failed(msg: &str) -> ! {
    panic_display(&msg)
}

/////////////////////////////////////////////////////////////////////////////
// trait 实现
/////////////////////////////////////////////////////////////////////////////

#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_unstable(feature = "const_clone", issue = "142757")]
impl<T> const Clone for Option<T>
where
    // FIXME(const_hack): `clone_from` 中的 `T: [const] Destruct` 应能从
    // `Self: [const] Destruct` 推导出来。参见 https://github.com/rust-lang/rust/issues/144207
    T: [const] Clone + [const] Destruct,
{
    #[inline]
    fn clone(&self) -> Self {
        match self {
            Some(x) => Some(x.clone()),
            None => None,
        }
    }

    #[inline]
    fn clone_from(&mut self, source: &Self) {
        match (self, source) {
            (Some(to), Some(from)) => to.clone_from(from),
            (to, from) => *to = from.clone(),
        }
    }
}

#[unstable(feature = "ergonomic_clones", issue = "132290")]
impl<T> crate::clone::UseCloned for Option<T> where T: crate::clone::UseCloned {}

#[doc(hidden)]
#[unstable(feature = "trivial_clone", issue = "none")]
#[rustc_const_unstable(feature = "const_clone", issue = "142757")]
unsafe impl<T> const TrivialClone for Option<T> where T: [const] TrivialClone + [const] Destruct {}

#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_unstable(feature = "const_default", issue = "143894")]
impl<T> const Default for Option<T> {
    /// 返回 [`None`][Option::None]。
    ///
    /// `Option<T>` 的默认值表示“尚无值”，不会构造任何 `T`。
    #[inline]
    fn default() -> Option<T> {
        None
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T> IntoIterator for Option<T> {
    type Item = T;
    type IntoIter = IntoIter<T>;

    /// 消费 `Option` 并返回迭代器。
    ///
    /// 若为 [`Some`]，迭代器产生一个 `T`；若为 [`None`]，迭代器为空。
    #[inline]
    fn into_iter(self) -> IntoIter<T> {
        IntoIter { inner: Item { opt: self } }
    }
}

#[stable(since = "1.4.0", feature = "option_iter")]
impl<'a, T> IntoIterator for &'a Option<T> {
    type Item = &'a T;
    type IntoIter = Iter<'a, T>;

    fn into_iter(self) -> Iter<'a, T> {
        self.iter()
    }
}

#[stable(since = "1.4.0", feature = "option_iter")]
impl<'a, T> IntoIterator for &'a mut Option<T> {
    type Item = &'a mut T;
    type IntoIter = IterMut<'a, T>;

    fn into_iter(self) -> IterMut<'a, T> {
        self.iter_mut()
    }
}

#[stable(since = "1.12.0", feature = "option_from")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl<T> const From<T> for Option<T> {
    /// 执行 `Option` 的标准转换。
    ///
    /// 从 `T` 转换会得到 [`Some(val)`]；从 `&Option<T>` 或 `&mut Option<T>` 转换会借用内部值，而不移动原 `Option`。
    fn from(val: T) -> Option<T> {
        Some(val)
    }
}

#[stable(feature = "option_ref_from_ref_option", since = "1.30.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl<'a, T> const From<&'a Option<T>> for Option<&'a T> {
    /// 执行 `Option` 的标准转换。
    ///
    /// 从 `T` 转换会得到 [`Some(val)`]；从 `&Option<T>` 或 `&mut Option<T>` 转换会借用内部值，而不移动原 `Option`。
    fn from(o: &'a Option<T>) -> Option<&'a T> {
        o.as_ref()
    }
}

#[stable(feature = "option_ref_from_ref_option", since = "1.30.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl<'a, T> const From<&'a mut Option<T>> for Option<&'a mut T> {
    /// 执行 `Option` 的标准转换。
    ///
    /// 从 `T` 转换会得到 [`Some(val)`]；从 `&Option<T>` 或 `&mut Option<T>` 转换会借用内部值，而不移动原 `Option`。
    fn from(o: &'a mut Option<T>) -> Option<&'a mut T> {
        o.as_mut()
    }
}

// 理想情况下，LLVM 应能把 derive 生成的代码优化到这种形式。
// 等 https://github.com/llvm/llvm-project/issues/52622 修复后，可以改回 derive `PartialEq`。
#[stable(feature = "rust1", since = "1.0.0")]
impl<T> crate::marker::StructuralPartialEq for Option<T> {}
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
impl<T: [const] PartialEq> const PartialEq for Option<T> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        // 显式写出各个分支比 `_ => false` 优化效果更好。
        match (self, other) {
            (Some(l), Some(r)) => *l == *r,
            (Some(_), None) => false,
            (None, Some(_)) => false,
            (None, None) => true,
        }
    }
}

// 这里手动实现可以稍微改善 https://github.com/rust-lang/rust/issues/49892 的 codegen，
// 尽管结果仍非最优。
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
impl<T: [const] PartialOrd> const PartialOrd for Option<T> {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        match (self, other) {
            (Some(l), Some(r)) => l.partial_cmp(r),
            (Some(_), None) => Some(cmp::Ordering::Greater),
            (None, Some(_)) => Some(cmp::Ordering::Less),
            (None, None) => Some(cmp::Ordering::Equal),
        }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
impl<T: [const] Ord> const Ord for Option<T> {
    #[inline]
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        match (self, other) {
            (Some(l), Some(r)) => l.cmp(r),
            (Some(_), None) => cmp::Ordering::Greater,
            (None, Some(_)) => cmp::Ordering::Less,
            (None, None) => cmp::Ordering::Equal,
        }
    }
}

/////////////////////////////////////////////////////////////////////////////
// Option 迭代器
/////////////////////////////////////////////////////////////////////////////

#[derive(Clone, Debug)]
struct Item<A> {
    opt: Option<A>,
}

impl<A> Iterator for Item<A> {
    type Item = A;

    #[inline]
    fn next(&mut self) -> Option<A> {
        self.opt.take()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}

impl<A> DoubleEndedIterator for Item<A> {
    #[inline]
    fn next_back(&mut self) -> Option<A> {
        self.opt.take()
    }
}

impl<A> ExactSizeIterator for Item<A> {
    #[inline]
    fn len(&self) -> usize {
        self.opt.len()
    }
}
impl<A> FusedIterator for Item<A> {}
unsafe impl<A> TrustedLen for Item<A> {}

/// 遍历 [`Option`] 或 [`Result`] 中共享引用的迭代器。
///
/// 它最多产生一个元素：存在成功/有效值时产生引用，否则为空。
#[stable(feature = "rust1", since = "1.0.0")]
#[derive(Debug)]
pub struct Iter<'a, A: 'a> {
    inner: Item<&'a A>,
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<'a, A> Iterator for Iter<'a, A> {
    type Item = &'a A;

    #[inline]
    fn next(&mut self) -> Option<&'a A> {
        self.inner.next()
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<'a, A> DoubleEndedIterator for Iter<'a, A> {
    #[inline]
    fn next_back(&mut self) -> Option<&'a A> {
        self.inner.next_back()
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<A> ExactSizeIterator for Iter<'_, A> {}

#[stable(feature = "fused", since = "1.26.0")]
impl<A> FusedIterator for Iter<'_, A> {}

#[unstable(feature = "trusted_len", issue = "37572")]
unsafe impl<A> TrustedLen for Iter<'_, A> {}

#[stable(feature = "rust1", since = "1.0.0")]
impl<A> Clone for Iter<'_, A> {
    #[inline]
    fn clone(&self) -> Self {
        Iter { inner: self.inner.clone() }
    }
}

/// 遍历 [`Option`] 或 [`Result`] 中可变引用的迭代器。
///
/// 它最多产生一个元素：存在成功/有效值时产生可变引用，否则为空。
#[stable(feature = "rust1", since = "1.0.0")]
#[derive(Debug)]
pub struct IterMut<'a, A: 'a> {
    inner: Item<&'a mut A>,
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<'a, A> Iterator for IterMut<'a, A> {
    type Item = &'a mut A;

    #[inline]
    fn next(&mut self) -> Option<&'a mut A> {
        self.inner.next()
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<'a, A> DoubleEndedIterator for IterMut<'a, A> {
    #[inline]
    fn next_back(&mut self) -> Option<&'a mut A> {
        self.inner.next_back()
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<A> ExactSizeIterator for IterMut<'_, A> {}

#[stable(feature = "fused", since = "1.26.0")]
impl<A> FusedIterator for IterMut<'_, A> {}
#[unstable(feature = "trusted_len", issue = "37572")]
unsafe impl<A> TrustedLen for IterMut<'_, A> {}

/// 消费 [`Option`] 或 [`Result`] 并遍历其中值的迭代器。
///
/// 它最多产生一个元素，缺值或错误分支会表现为空迭代器。
#[derive(Clone, Debug)]
#[stable(feature = "rust1", since = "1.0.0")]
pub struct IntoIter<A> {
    inner: Item<A>,
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<A> Iterator for IntoIter<A> {
    type Item = A;

    #[inline]
    fn next(&mut self) -> Option<A> {
        self.inner.next()
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<A> DoubleEndedIterator for IntoIter<A> {
    #[inline]
    fn next_back(&mut self) -> Option<A> {
        self.inner.next_back()
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<A> ExactSizeIterator for IntoIter<A> {}

#[stable(feature = "fused", since = "1.26.0")]
impl<A> FusedIterator for IntoIter<A> {}

#[unstable(feature = "trusted_len", issue = "37572")]
unsafe impl<A> TrustedLen for IntoIter<A> {}

/// 由 [`Option::into_flat_iter`] 产生的迭代器。
///
/// 它在外层 [`Option`] 为 [`Some`] 时委托给内部迭代器，为 [`None`] 时为空。
#[derive(Clone, Debug)]
#[unstable(feature = "option_into_flat_iter", issue = "148441")]
pub struct OptionFlatten<A> {
    iter: Option<A>,
}

#[unstable(feature = "option_into_flat_iter", issue = "148441")]
impl<A: Iterator> Iterator for OptionFlatten<A> {
    type Item = A::Item;

    fn next(&mut self) -> Option<Self::Item> {
        self.iter.as_mut()?.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.as_ref().map(|i| i.size_hint()).unwrap_or((0, Some(0)))
    }
}

#[unstable(feature = "option_into_flat_iter", issue = "148441")]
impl<A: DoubleEndedIterator> DoubleEndedIterator for OptionFlatten<A> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.iter.as_mut()?.next_back()
    }
}

#[unstable(feature = "option_into_flat_iter", issue = "148441")]
impl<A: ExactSizeIterator> ExactSizeIterator for OptionFlatten<A> {}

#[unstable(feature = "option_into_flat_iter", issue = "148441")]
impl<A: FusedIterator> FusedIterator for OptionFlatten<A> {}

#[unstable(feature = "option_into_flat_iter", issue = "148441")]
unsafe impl<A: TrustedLen> TrustedLen for OptionFlatten<A> {}

/////////////////////////////////////////////////////////////////////////////
// FromIterator 实现
/////////////////////////////////////////////////////////////////////////////

#[stable(feature = "rust1", since = "1.0.0")]
impl<A, V: FromIterator<A>> FromIterator<Option<A>> for Option<V> {
    /// 从 `Iterator<Item = Option<A>>` 收集出 `Option<V>`。
    ///
    /// 迭代过程中只要遇到 [`None`] 就短路并返回 [`None`]；如果全部为 [`Some`]，则把所有内部值收集进目标集合。
    #[inline]
    fn from_iter<I: IntoIterator<Item = Option<A>>>(iter: I) -> Option<V> {
        // FIXME(#11084): 等这个性能问题修复后，可以用 Iterator::scan 替换这里。

        iter::try_process(iter.into_iter(), |i| i.collect())
    }
}

#[unstable(feature = "try_trait_v2", issue = "84277", old_name = "try_trait")]
#[rustc_const_unstable(feature = "const_try", issue = "74935")]
impl<T> const ops::Try for Option<T> {
    type Output = T;
    type Residual = Option<convert::Infallible>;

    #[inline]
    fn from_output(output: Self::Output) -> Self {
        Some(output)
    }

    #[inline]
    fn branch(self) -> ControlFlow<Self::Residual, Self::Output> {
        match self {
            Some(v) => ControlFlow::Continue(v),
            None => ControlFlow::Break(None),
        }
    }
}

#[unstable(feature = "try_trait_v2", issue = "84277", old_name = "try_trait")]
#[rustc_const_unstable(feature = "const_try", issue = "74935")]
// 注意：这里手动指定 residual 类型，而不是使用默认值，以绕过
// https://github.com/rust-lang/rust/issues/99940
impl<T> const ops::FromResidual<Option<convert::Infallible>> for Option<T> {
    #[inline]
    fn from_residual(residual: Option<convert::Infallible>) -> Self {
        match residual {
            None => None,
        }
    }
}

#[diagnostic::do_not_recommend]
#[unstable(feature = "try_trait_v2_yeet", issue = "96374")]
#[rustc_const_unstable(feature = "const_try", issue = "74935")]
impl<T> const ops::FromResidual<ops::Yeet<()>> for Option<T> {
    #[inline]
    fn from_residual(ops::Yeet(()): ops::Yeet<()>) -> Self {
        None
    }
}

#[unstable(feature = "try_trait_v2_residual", issue = "91285")]
#[rustc_const_unstable(feature = "const_try", issue = "74935")]
impl<T> const ops::Residual<T> for Option<convert::Infallible> {
    type TryType = Option<T>;
}

impl<T> Option<Option<T>> {
    /// 移除一层 `Option` 嵌套。
    ///
    /// `Some(Some(v))` 变为 `Some(v)`，`Some(None)` 和 `None` 都变为 [`None`]。
    #[inline]
    #[stable(feature = "option_flattening", since = "1.40.0")]
    #[rustc_allow_const_fn_unstable(const_precise_live_drops)]
    #[rustc_const_stable(feature = "const_option", since = "1.83.0")]
    pub const fn flatten(self) -> Option<T> {
        // FIXME(const-hack): 可以改写为 `and_then`。
        match self {
            Some(inner) => inner,
            None => None,
        }
    }
}

impl<'a, T> Option<&'a Option<T>> {
    /// 把嵌套在引用中的 `Option` 展平为内部值的共享引用。
    ///
    /// 该方法不移动被借用的 `Option` 或内部值。
    #[inline]
    #[unstable(feature = "option_reference_flattening", issue = "149221")]
    pub const fn flatten_ref(self) -> Option<&'a T> {
        match self {
            Some(inner) => inner.as_ref(),
            None => None,
        }
    }
}

impl<'a, T> Option<&'a mut Option<T>> {
    /// 把嵌套在引用中的 `Option` 展平为内部值的共享引用。
    ///
    /// 该方法不移动被借用的 `Option` 或内部值。
    #[inline]
    #[unstable(feature = "option_reference_flattening", issue = "149221")]
    pub const fn flatten_ref(self) -> Option<&'a T> {
        match self {
            Some(inner) => inner.as_ref(),
            None => None,
        }
    }

    /// 把嵌套在可变引用中的 `Option` 展平为内部值的可变引用。
    ///
    /// 该方法不移动被借用的 `Option`，只在存在内部值时给出唯一可变借用。
    #[inline]
    #[unstable(feature = "option_reference_flattening", issue = "149221")]
    pub const fn flatten_mut(self) -> Option<&'a mut T> {
        match self {
            Some(inner) => inner.as_mut(),
            None => None,
        }
    }
}

impl<T, const N: usize> [Option<T>; N] {
    /// 在 `Option` 与另一层容器之间交换嵌套顺序。
    ///
    /// 对 `Option<Result<T, E>>`，`Some(Ok(v))` 变为 `Ok(Some(v))`，`Some(Err(e))` 变为 `Err(e)`，`None` 变为 `Ok(None)`。对数组形式则把 `[Option<T>; N]` 转为 `Option<[T; N]>`，任一元素为 [`None`] 即整体为 [`None`]。
    #[inline]
    #[unstable(feature = "option_array_transpose", issue = "130828")]
    pub fn transpose(self) -> Option<[T; N]> {
        self.try_map(core::convert::identity)
    }
}
