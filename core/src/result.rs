//! 使用 `Result` 类型进行错误处理。
//!
//! [`Result<T, E>`][`Result`] 用来返回和传播可恢复错误。它有两个变体：[`Ok(T)`] 表示成功并携带
//! 成功值，[`Err(E)`] 表示错误并携带错误值。与异常或全局错误码相比，`Result` 把“调用可能失败”
//! 放在函数签名中，使调用方在类型层面必须处理成功和失败两条路径。
//!
//! ```
//! # #[allow(dead_code)]
//! enum Result<T, E> {
//!    Ok(T),
//!    Err(E),
//! }
//! ```
//!
//! 函数在错误可预期且可恢复时返回 [`Result`]。在 `std` 中最典型的例子是
//! [I/O](../../std/io/index.html)：读写可能失败，但调用方通常可以选择重试、记录、降级或继续向上
//! 传播错误。
//!
//! # `Result` 必须被使用
//!
//! 用返回值表示错误的常见问题是调用方可能忽略返回值，从而遗漏错误处理。[`Result`] 带有
//! `#[must_use]` 属性，忽略 `Result` 值时编译器会发出警告。这让 `Result` 特别适合“可能失败但
//! 成功时不返回额外有用值”的函数，例如 I/O 的 `write_all`。
//!
//! 如果只是想断言成功，可以使用 [`expect`]。当结果为 [`Err`] 时，`expect` 会 panic，并把调用方
//! 提供的消息与错误的 [`Debug`] 表示一起输出；如果希望继续把错误交给上层处理，应使用 `?`。
//!
//! # 问号运算符 `?`
//!
//! 在返回 [`Result`] 的函数中，`?` 对 [`Ok`] 取出内部成功值并继续执行；如果遇到 [`Err`]，则立即
//! 从外层函数返回错误。返回时会通过 [`FromResidual`] / [`Try`] 相关机制执行必要转换，因此错误类型
//! 可以在边界处按 trait 规则转换。
//!
//! ```
//! # use std::fs::File;
//! # use std::io::prelude::*;
//! # use std::io;
//! # #[allow(dead_code)]
//! fn write_message() -> io::Result<()> {
//!     let mut file = File::create("valuable_data.txt")?;
//!     file.write_all(b"important message")?;
//!     Ok(())
//! }
//! ```
//!
//! [`Debug`]: crate::fmt::Debug
//! [`FromResidual`]: crate::ops::FromResidual
//! [`Try`]: crate::ops::Try
//! [`expect`]: Result::expect
//! [`Write`]: ../../std/io/trait.Write.html "io::Write"
//! [`write_all`]: ../../std/io/trait.Write.html#method.write_all "io::Write::write_all"
//! [`io::Result`]: ../../std/io/type.Result.html "io::Result"
//! [`?`]: crate::ops::Try
//! [`Ok(T)`]: Ok
//! [`Err(E)`]: Err
//! [io::Error]: ../../std/io/struct.Error.html "io::Error"
//!
//! # 表示
//!
//! 某些 [`Result<T, E>`] 具有大小、对齐和 ABI 保证。具体地，`T` 或 `E` 之一必须满足 `Option`
//! 的 [表示保证][opt-rep]，记为 `I`；另一个类型必须是大小为 0 且对齐为 1 的类型（“1-ZST”）。
//! 在这种情况下，`Result<T, E>` 与 `I`（也就是与 `Option<I>`）具有相同大小、对齐和
//! [function call ABI]。
//!
//! 例如 `NonZeroI32` 满足 `Option` 表示保证，`()` 是 1-ZST，因此 `Result<NonZeroI32, ()>` 和
//! `Result<(), NonZeroI32>` 都与 `NonZeroI32`（以及 `Option<NonZeroI32>`）具有相同布局。三者差异
//! 只在语义上：`Option<NonZeroI32>` 表示“可能有一个非零 i32”，`Result<NonZeroI32, ()>` 表示
//! “可能成功并给出非零 i32”，而 `Result<(), NonZeroI32>` 表示“可能失败并给出非零 i32 错误码”。
//!
//! [opt-rep]: ../option/index.html#representation "Option Representation"
//! [function call ABI]: ../primitive.fn.html#abi-compatibility
//!
//! # 方法概览
//!
//! [`Result`] 的方法大致分为几类：查询变体（[`is_ok`]、[`is_err`]、[`is_ok_and`]、
//! [`is_err_and`]）、转换到 [`Option`]（[`ok`]、[`err`]）、引用适配（[`as_ref`]、[`as_mut`]）、
//! 转换成功或错误值（[`map`]、[`map_err`]、[`inspect`]、[`inspect_err`]）、提取成功或错误值
//! （[`unwrap`]、[`expect`]、[`unwrap_err`]、[`expect_err`]、`unwrap_or*`）、以及短路组合
//! （[`and_then`]、[`or_else`]）。
//!
//! [`unwrap`] 和 [`expect`] 只在 `self` 为 [`Ok`] 时返回成功值；当 `self` 为 [`Err`] 时会 panic。
//! [`expect`] 使用调用方提供的上下文消息，[`unwrap`] 使用通用消息，两者都要求错误类型实现
//! [`Debug`]。对称地，[`unwrap_err`] 和 [`expect_err`] 只在 `self` 为 [`Err`] 时返回错误值；当
//! `self` 为 [`Ok`] 时会 panic。`unwrap_unchecked` 与 `unwrap_err_unchecked` 不检查变体，调用方
//! 若承诺错误会造成 UB。
//!
//! 组合子遵循短路语义：[`map`] 只改变 [`Ok`]，[`map_err`] 只改变 [`Err`]，[`and_then`] 只在
//! [`Ok`] 时调用闭包继续成功路径，[`or_else`] 只在 [`Err`] 时调用闭包恢复或转换错误路径。
//! 这也是 `?` 能把失败路径短路向上传播的基础。
//!
//! [`and_then`]: Result::and_then
//! [`as_mut`]: Result::as_mut
//! [`as_ref`]: Result::as_ref
//! [`err`]: Result::err
//! [`expect_err`]: Result::expect_err
//! [`inspect`]: Result::inspect
//! [`inspect_err`]: Result::inspect_err
//! [`is_err`]: Result::is_err
//! [`is_err_and`]: Result::is_err_and
//! [`is_ok`]: Result::is_ok
//! [`is_ok_and`]: Result::is_ok_and
//! [`map`]: Result::map
//! [`map_err`]: Result::map_err
//! [`ok`]: Result::ok
//! [`or_else`]: Result::or_else
//! [`unwrap`]: Result::unwrap
//! [`unwrap_err`]: Result::unwrap_err
//!
//! # 示例
//!
//! ```
//! #[derive(Debug)]
//! enum Version { Version1, Version2 }
//!
//! fn parse_version(header: &[u8]) -> Result<Version, &'static str> {
//!     match header.get(0) {
//!         None => Err("invalid header length"),
//!         Some(&1) => Ok(Version::Version1),
//!         Some(&2) => Ok(Version::Version2),
//!         Some(_) => Err("invalid version"),
//!     }
//! }
//! ```

#![stable(feature = "rust1", since = "1.0.0")]

use crate::iter::{self, FusedIterator, TrustedLen};
use crate::marker::Destruct;
use crate::ops::{self, ControlFlow, Deref, DerefMut};
use crate::{convert, fmt, hint};

/// `Result` 类型，表示成功（[`Ok`]）或失败（[`Err`]）。
///
/// 更多错误传播、`?` 短路和布局保证见本模块级文档。
#[doc(search_unbox)]
#[derive(Copy, Debug, Hash)]
#[derive_const(PartialEq, PartialOrd, Eq, Ord)]
#[must_use = "this `Result` may be an `Err` variant, which should be handled"]
#[rustc_diagnostic_item = "Result"]
#[stable(feature = "rust1", since = "1.0.0")]
pub enum Result<T, E> {
    /// 携带成功值。
    ///
    /// 该变体表示计算成功，`?` 会从中取出值并继续执行。
    #[lang = "Ok"]
    #[stable(feature = "rust1", since = "1.0.0")]
    Ok(#[stable(feature = "rust1", since = "1.0.0")] T),

    /// 携带错误值。
    ///
    /// 该变体表示可恢复错误，`?` 会把它短路返回给调用方。
    #[lang = "Err"]
    #[stable(feature = "rust1", since = "1.0.0")]
    Err(#[stable(feature = "rust1", since = "1.0.0")] E),
}

/////////////////////////////////////////////////////////////////////////////
// 类型实现
/////////////////////////////////////////////////////////////////////////////

impl<T, E> Result<T, E> {
    /////////////////////////////////////////////////////////////////////////
    // 查询包含的值
    /////////////////////////////////////////////////////////////////////////

    /// 如果 `self` 是 [`Ok`]，返回 `true`。
    #[must_use = "if you intended to assert that this is ok, consider `.unwrap()` instead"]
    #[rustc_const_stable(feature = "const_result_basics", since = "1.48.0")]
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    pub const fn is_ok(&self) -> bool {
        matches!(*self, Ok(_))
    }

    /// 如果 `self` 是 [`Ok`] 且内部成功值满足谓词 `f`，返回 `true`。
    ///
    /// 当 `self` 为 [`Err`] 时闭包不会执行，并直接返回 `false`。
    #[must_use]
    #[inline]
    #[stable(feature = "is_some_and", since = "1.70.0")]
    #[rustc_const_unstable(feature = "const_result_trait_fn", issue = "144211")]
    pub const fn is_ok_and<F>(self, f: F) -> bool
    where
        F: [const] FnOnce(T) -> bool + [const] Destruct,
        T: [const] Destruct,
        E: [const] Destruct,
    {
        match self {
            Err(_) => false,
            Ok(x) => f(x),
        }
    }

    /// 如果 `self` 是 [`Err`]，返回 `true`。
    #[must_use = "if you intended to assert that this is err, consider `.unwrap_err()` instead"]
    #[rustc_const_stable(feature = "const_result_basics", since = "1.48.0")]
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    pub const fn is_err(&self) -> bool {
        !self.is_ok()
    }

    /// 如果 `self` 是 [`Err`] 且内部错误值满足谓词 `f`，返回 `true`。
    ///
    /// 当 `self` 为 [`Ok`] 时闭包不会执行，并直接返回 `false`。
    #[must_use]
    #[inline]
    #[stable(feature = "is_some_and", since = "1.70.0")]
    #[rustc_const_unstable(feature = "const_result_trait_fn", issue = "144211")]
    pub const fn is_err_and<F>(self, f: F) -> bool
    where
        F: [const] FnOnce(E) -> bool + [const] Destruct,
        E: [const] Destruct,
        T: [const] Destruct,
    {
        match self {
            Ok(_) => false,
            Err(e) => f(e),
        }
    }

    /////////////////////////////////////////////////////////////////////////
    // 各变体适配器
    /////////////////////////////////////////////////////////////////////////

    /// 把 `Result<T, E>` 转换为 `Option<T>`。
    ///
    /// [`Ok(v)`] 变为 [`Some(v)`]；[`Err`] 中的错误值被消费并丢弃，结果为 [`None`]。
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_unstable(feature = "const_result_trait_fn", issue = "144211")]
    #[rustc_diagnostic_item = "result_ok_method"]
    pub const fn ok(self) -> Option<T>
    where
        T: [const] Destruct,
        E: [const] Destruct,
    {
        match self {
            Ok(x) => Some(x),
            Err(_) => None,
        }
    }

    /// 把 `Result<T, E>` 转换为 `Option<E>`。
    ///
    /// [`Err(e)`] 变为 [`Some(e)`]；[`Ok`] 中的成功值被消费并丢弃，结果为 [`None`]。
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_unstable(feature = "const_result_trait_fn", issue = "144211")]
    pub const fn err(self) -> Option<E>
    where
        T: [const] Destruct,
        E: [const] Destruct,
    {
        match self {
            Ok(_) => None,
            Err(x) => Some(x),
        }
    }

    /////////////////////////////////////////////////////////////////////////
    // 引用适配器
    /////////////////////////////////////////////////////////////////////////

    /// 把 `&Result<T, E>` 转换为 `Result<&T, &E>`。
    ///
    /// 该方法保留原 `Result` 的所有权，只借用当前变体内的值。
    #[inline]
    #[rustc_const_stable(feature = "const_result_basics", since = "1.48.0")]
    #[stable(feature = "rust1", since = "1.0.0")]
    pub const fn as_ref(&self) -> Result<&T, &E> {
        match *self {
            Ok(ref x) => Ok(x),
            Err(ref x) => Err(x),
        }
    }

    /// 把 `&mut Result<T, E>` 转换为 `Result<&mut T, &mut E>`。
    ///
    /// 该方法保留原 `Result`，只在当前变体中提供内部值的可变借用。
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_result", since = "1.83.0")]
    pub const fn as_mut(&mut self) -> Result<&mut T, &mut E> {
        match *self {
            Ok(ref mut x) => Ok(x),
            Err(ref mut x) => Err(x),
        }
    }

    /////////////////////////////////////////////////////////////////////////
    // 转换包含的值
    /////////////////////////////////////////////////////////////////////////

    /// 如果 `self` 为 [`Ok`]，把成功值交给函数 `op` 并返回 `Ok(op(value))`；如果为 [`Err`]，保持错误不变。
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_unstable(feature = "const_result_trait_fn", issue = "144211")]
    pub const fn map<U, F>(self, op: F) -> Result<U, E>
    where
        F: [const] FnOnce(T) -> U + [const] Destruct,
    {
        match self {
            Ok(t) => Ok(op(t)),
            Err(e) => Err(e),
        }
    }

    /// 把 `Result<T, E>` 转换为普通值 `U`。
    ///
    /// 若为 [`Ok`]，对成功值执行 `f`；若为 [`Err`]，返回已经求值的 `default`。
    #[inline]
    #[stable(feature = "result_map_or", since = "1.41.0")]
    #[rustc_const_unstable(feature = "const_result_trait_fn", issue = "144211")]
    #[must_use = "if you don't need the returned value, use `if let` instead"]
    pub const fn map_or<U, F>(self, default: U, f: F) -> U
    where
        F: [const] FnOnce(T) -> U + [const] Destruct,
        T: [const] Destruct,
        E: [const] Destruct,
        U: [const] Destruct,
    {
        match self {
            Ok(t) => f(t),
            Err(_) => default,
        }
    }

    /// 把 `Result<T, E>` 转换为普通值 `U`，并分别处理成功和错误分支。
    ///
    /// 若为 [`Ok`]，对成功值执行 `f`；若为 [`Err`]，把错误值交给 `default` 闭包计算后备值。
    #[inline]
    #[stable(feature = "result_map_or_else", since = "1.41.0")]
    #[rustc_const_unstable(feature = "const_result_trait_fn", issue = "144211")]
    pub const fn map_or_else<U, D, F>(self, default: D, f: F) -> U
    where
        D: [const] FnOnce(E) -> U + [const] Destruct,
        F: [const] FnOnce(T) -> U + [const] Destruct,
    {
        match self {
            Ok(t) => f(t),
            Err(e) => default(e),
        }
    }

    /// 把 `Result<T, E>` 转换为 `U`。
    ///
    /// 若为 [`Ok`]，对成功值执行 `f`；若为 [`Err`]，返回 `U::default()`。
    #[inline]
    #[unstable(feature = "result_option_map_or_default", issue = "138099")]
    #[rustc_const_unstable(feature = "const_result_trait_fn", issue = "144211")]
    pub const fn map_or_default<U, F>(self, f: F) -> U
    where
        F: [const] FnOnce(T) -> U + [const] Destruct,
        U: [const] Default,
        T: [const] Destruct,
        E: [const] Destruct,
    {
        match self {
            Ok(t) => f(t),
            Err(_) => U::default(),
        }
    }

    /// 如果 `self` 为 [`Err`]，把错误值交给函数 `op` 并返回 `Err(op(error))`；如果为 [`Ok`]，保持成功值不变。
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_unstable(feature = "const_result_trait_fn", issue = "144211")]
    pub const fn map_err<F, O>(self, op: O) -> Result<T, F>
    where
        O: [const] FnOnce(E) -> F + [const] Destruct,
    {
        match self {
            Ok(t) => Ok(t),
            Err(e) => Err(op(e)),
        }
    }

    /// 如果 `self` 为 [`Ok`]，以共享引用形式把成功值传给闭包 `f`，然后返回原 `Result`。
    ///
    /// 适合插入日志或断言，不改变成功/错误状态。
    #[inline]
    #[stable(feature = "result_option_inspect", since = "1.76.0")]
    #[rustc_const_unstable(feature = "const_result_trait_fn", issue = "144211")]
    pub const fn inspect<F>(self, f: F) -> Self
    where
        F: [const] FnOnce(&T) + [const] Destruct,
    {
        if let Ok(ref t) = self {
            f(t);
        }

        self
    }

    /// 如果 `self` 为 [`Err`]，以共享引用形式把错误值传给闭包 `f`，然后返回原 `Result`。
    ///
    /// 适合在不消费错误的情况下记录或观察错误。
    #[inline]
    #[stable(feature = "result_option_inspect", since = "1.76.0")]
    #[rustc_const_unstable(feature = "const_result_trait_fn", issue = "144211")]
    pub const fn inspect_err<F>(self, f: F) -> Self
    where
        F: [const] FnOnce(&E) + [const] Destruct,
    {
        if let Err(ref e) = self {
            f(e);
        }

        self
    }

    /// 把 `Result<T, E>` 或 `&Result<T, E>` 转换为 `Result<&T::Target, &E>`。
    ///
    /// 该方法结合 [`Deref`]，只在 [`Ok`] 分支对成功值做目标借用。
    #[inline]
    #[stable(feature = "inner_deref", since = "1.47.0")]
    #[rustc_const_unstable(feature = "const_convert", issue = "143773")]
    pub const fn as_deref(&self) -> Result<&T::Target, &E>
    where
        T: [const] Deref,
    {
        self.as_ref().map(Deref::deref)
    }

    /// 把 `Result<T, E>` 或 `&mut Result<T, E>` 转换为 `Result<&mut T::Target, &mut E>`。
    ///
    /// 该方法结合 [`DerefMut`]，只在 [`Ok`] 分支对成功值做目标可变借用。
    #[inline]
    #[stable(feature = "inner_deref", since = "1.47.0")]
    #[rustc_const_unstable(feature = "const_convert", issue = "143773")]
    pub const fn as_deref_mut(&mut self) -> Result<&mut T::Target, &mut E>
    where
        T: [const] DerefMut,
    {
        self.as_mut().map(DerefMut::deref_mut)
    }

    /////////////////////////////////////////////////////////////////////////
    // 迭代器构造器
    /////////////////////////////////////////////////////////////////////////

    /// 返回遍历可能存在成功值的迭代器。
    ///
    /// 若为 [`Ok`]，迭代器产生一个 `&T`；若为 [`Err`]，迭代器为空。
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_unstable(feature = "const_result_trait_fn", issue = "144211")]
    pub const fn iter(&self) -> Iter<'_, T> {
        Iter { inner: self.as_ref().ok() }
    }

    /// 返回遍历可能存在成功值的可变迭代器。
    ///
    /// 若为 [`Ok`]，迭代器产生一个 `&mut T`；若为 [`Err`]，迭代器为空。
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_unstable(feature = "const_result_trait_fn", issue = "144211")]
    pub const fn iter_mut(&mut self) -> IterMut<'_, T> {
        IterMut { inner: self.as_mut().ok() }
    }

    /////////////////////////////////////////////////////////////////////////
    // 提取值
    /////////////////////////////////////////////////////////////////////////

    /// 消费 `self` 并返回其中的 [`Ok`] 值。
    ///
    /// # Panics
    ///
    /// 如果 `self` 为 [`Err`]，以调用方提供的 `msg` 触发 panic，并附带错误值的 [`Debug`] 表示。`#[track_caller]` 会把 panic 位置报告到调用点。
    #[inline]
    #[track_caller]
    #[stable(feature = "result_expect", since = "1.4.0")]
    pub fn expect(self, msg: &str) -> T
    where
        E: fmt::Debug,
    {
        match self {
            Ok(t) => t,
            Err(e) => unwrap_failed(msg, &e),
        }
    }

    /// 消费 `self` 并返回其中的 [`Ok`] 值。
    ///
    /// # Panics
    ///
    /// 如果 `self` 为 [`Err`]，以通用消息触发 panic，并附带错误值的 [`Debug`] 表示。需要业务上下文时应使用 [`expect`]。
    #[inline(always)]
    #[track_caller]
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn unwrap(self) -> T
    where
        E: fmt::Debug,
    {
        match self {
            Ok(t) => t,
            Err(e) => unwrap_failed("called `Result::unwrap()` on an `Err` value", &e),
        }
    }

    /// 消费 `self`；若为 [`Ok`]，返回成功值，否则返回 `T::default()`。
    ///
    /// 该方法要求 `T: Default`，并只在错误分支构造默认值。
    #[inline]
    #[stable(feature = "result_unwrap_or_default", since = "1.16.0")]
    #[rustc_const_unstable(feature = "const_result_trait_fn", issue = "144211")]
    pub const fn unwrap_or_default(self) -> T
    where
        T: [const] Default + [const] Destruct,
        E: [const] Destruct,
    {
        match self {
            Ok(x) => x,
            Err(_) => Default::default(),
        }
    }

    /// 消费 `self` 并返回其中的 [`Err`] 值。
    ///
    /// # Panics
    ///
    /// 如果 `self` 为 [`Ok`]，以调用方提供的 `msg` 触发 panic，并附带成功值的 [`Debug`] 表示。
    #[inline]
    #[track_caller]
    #[stable(feature = "result_expect_err", since = "1.17.0")]
    pub fn expect_err(self, msg: &str) -> E
    where
        T: fmt::Debug,
    {
        match self {
            Ok(t) => unwrap_failed(msg, &t),
            Err(e) => e,
        }
    }

    /// 消费 `self` 并返回其中的 [`Err`] 值。
    ///
    /// # Panics
    ///
    /// 如果 `self` 为 [`Ok`]，以通用消息触发 panic，并附带成功值的 [`Debug`] 表示。
    #[inline]
    #[track_caller]
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn unwrap_err(self) -> E
    where
        T: fmt::Debug,
    {
        match self {
            Ok(t) => unwrap_failed("called `Result::unwrap_err()` on an `Ok` value", &t),
            Err(e) => e,
        }
    }

    /// 返回内部 [`Ok`] 值，并且永不 panic。
    ///
    /// 该方法只在错误类型 `E` 无法构造时可用；类型系统保证 [`Err`] 分支不可能出现。
    #[unstable(feature = "unwrap_infallible", reason = "newly added", issue = "61695")]
    #[inline]
    #[rustc_allow_const_fn_unstable(const_precise_live_drops)]
    #[rustc_const_unstable(feature = "const_convert", issue = "143773")]
    pub const fn into_ok(self) -> T
    where
        E: [const] Into<!>,
    {
        match self {
            Ok(x) => x,
            Err(e) => e.into(),
        }
    }

    /// 返回内部 [`Err`] 值，并且永不 panic。
    ///
    /// 该方法只在成功类型 `T` 无法构造时可用；类型系统保证 [`Ok`] 分支不可能出现。
    #[unstable(feature = "unwrap_infallible", reason = "newly added", issue = "61695")]
    #[inline]
    #[rustc_allow_const_fn_unstable(const_precise_live_drops)]
    #[rustc_const_unstable(feature = "const_convert", issue = "143773")]
    pub const fn into_err(self) -> E
    where
        T: [const] Into<!>,
    {
        match self {
            Ok(x) => x.into(),
            Err(e) => e,
        }
    }

    ////////////////////////////////////////////////////////////////////////
    // 对值执行立即/惰性布尔组合
    /////////////////////////////////////////////////////////////////////////

    /// 如果 `self` 为 [`Ok`]，返回参数 `res`；如果为 [`Err`]，返回原错误。
    ///
    /// `res` 会在调用前求值；需要延迟继续成功路径时使用 [`and_then`]。
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_unstable(feature = "const_result_trait_fn", issue = "144211")]
    pub const fn and<U>(self, res: Result<U, E>) -> Result<U, E>
    where
        T: [const] Destruct,
        E: [const] Destruct,
        U: [const] Destruct,
    {
        match self {
            Ok(_) => res,
            Err(e) => Err(e),
        }
    }

    /// 如果 `self` 为 [`Ok`]，把成功值交给闭包 `op` 并返回其结果；如果为 [`Err`]，直接返回原错误。
    ///
    /// 这是 `Result` 的扁平映射组合子，适合串联多个可失败步骤，并在第一处错误短路。
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_unstable(feature = "const_result_trait_fn", issue = "144211")]
    #[rustc_confusables("flat_map", "flatmap")]
    pub const fn and_then<U, F>(self, op: F) -> Result<U, E>
    where
        F: [const] FnOnce(T) -> Result<U, E> + [const] Destruct,
    {
        match self {
            Ok(t) => op(t),
            Err(e) => Err(e),
        }
    }

    /// 如果 `self` 为 [`Err`]，返回参数 `res`；如果为 [`Ok`]，返回原成功值。
    ///
    /// `res` 会在调用前求值；需要延迟恢复错误路径时使用 [`or_else`]。
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_unstable(feature = "const_result_trait_fn", issue = "144211")]
    pub const fn or<F>(self, res: Result<T, F>) -> Result<T, F>
    where
        T: [const] Destruct,
        E: [const] Destruct,
        F: [const] Destruct,
    {
        match self {
            Ok(v) => Ok(v),
            Err(_) => res,
        }
    }

    /// 如果 `self` 为 [`Err`]，把错误值交给闭包 `op` 并返回其结果；如果为 [`Ok`]，直接返回原成功值。
    ///
    /// 闭包只在错误分支执行，可用于错误转换、重试或后备处理。
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_unstable(feature = "const_result_trait_fn", issue = "144211")]
    pub const fn or_else<F, O>(self, op: O) -> Result<T, F>
    where
        O: [const] FnOnce(E) -> Result<T, F> + [const] Destruct,
    {
        match self {
            Ok(t) => Ok(t),
            Err(e) => op(e),
        }
    }

    /// 消费 `self`；若为 [`Ok`]，返回成功值，否则返回给定的 `default`。
    ///
    /// `default` 会在调用前求值；需要延迟计算时使用 [`unwrap_or_else`]。
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_unstable(feature = "const_result_trait_fn", issue = "144211")]
    pub const fn unwrap_or(self, default: T) -> T
    where
        T: [const] Destruct,
        E: [const] Destruct,
    {
        match self {
            Ok(t) => t,
            Err(_) => default,
        }
    }

    /// 消费 `self`；若为 [`Ok`]，返回成功值，否则把错误值交给闭包 `op` 计算替代值。
    ///
    /// 闭包只在 [`Err`] 分支执行。
    #[inline]
    #[track_caller]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_unstable(feature = "const_result_trait_fn", issue = "144211")]
    pub const fn unwrap_or_else<F>(self, op: F) -> T
    where
        F: [const] FnOnce(E) -> T + [const] Destruct,
    {
        match self {
            Ok(t) => t,
            Err(e) => op(e),
        }
    }

    /// 消费 `self` 并在不检查变体的情况下返回内部 [`Ok`] 值。
    ///
    /// # 安全性(Safety）
    ///
    /// 调用方必须保证 `self` 确实是 [`Ok`]。如果传入 [`Err`]，会调用 `unreachable_unchecked` 路径；编译器可假设该分支不可能发生，违反此前置条件是未定义行为(UB)。
    #[inline]
    #[track_caller]
    #[stable(feature = "option_result_unwrap_unchecked", since = "1.58.0")]
    #[rustc_const_unstable(feature = "const_result_unwrap_unchecked", issue = "148714")]
    pub const unsafe fn unwrap_unchecked(self) -> T {
        match self {
            Ok(t) => t,
            Err(e) => {
                // FIXME(const-hack): 用于避免引入 `E: const Destruct` 约束。
                super::mem::forget(e);
                // SAFETY: 调用方必须保证 `self` 是 `Ok`，因此 `Err` 分支不可达。
                unsafe { hint::unreachable_unchecked() }
            }
        }
    }

    /// 消费 `self` 并在不检查变体的情况下返回内部 [`Err`] 值。
    ///
    /// # 安全性(Safety）
    ///
    /// 调用方必须保证 `self` 确实是 [`Err`]。如果传入 [`Ok`]，会调用 `unreachable_unchecked` 路径；违反此前置条件是未定义行为(UB)。
    #[inline]
    #[track_caller]
    #[stable(feature = "option_result_unwrap_unchecked", since = "1.58.0")]
    pub unsafe fn unwrap_err_unchecked(self) -> E {
        match self {
            // SAFETY: 调用方必须保证 `self` 是 `Err`，因此 `Ok` 分支不可达。
            Ok(_) => unsafe { hint::unreachable_unchecked() },
            Err(e) => e,
        }
    }
}

impl<T, E> Result<&T, E> {
    /// 把 `Result<&T, E>` 或 `Result<&mut T, E>` 转换为 `Result<T, E>`，通过复制成功值完成转换。
    ///
    /// 该方法要求 `T: Copy`，错误分支保持不变。
    #[inline]
    #[stable(feature = "result_copied", since = "1.59.0")]
    #[rustc_const_stable(feature = "const_result", since = "1.83.0")]
    #[rustc_allow_const_fn_unstable(const_precise_live_drops)]
    pub const fn copied(self) -> Result<T, E>
    where
        T: Copy,
    {
        // FIXME(const-hack): 这个实现绕开了尚未 const-ready 的 `Result::map`；
        // 等可行时应改回去以避免重复代码。
        match self {
            Ok(&v) => Ok(v),
            Err(e) => Err(e),
        }
    }

    /// 把 `Result<&T, E>` 或 `Result<&mut T, E>` 转换为 `Result<T, E>`，通过克隆成功值完成转换。
    ///
    /// 该方法要求 `T: Clone`，并只在 [`Ok`] 分支调用 `clone`。
    #[inline]
    #[stable(feature = "result_cloned", since = "1.59.0")]
    pub fn cloned(self) -> Result<T, E>
    where
        T: Clone,
    {
        self.map(|t| t.clone())
    }
}

impl<T, E> Result<&mut T, E> {
    /// 把 `Result<&T, E>` 或 `Result<&mut T, E>` 转换为 `Result<T, E>`，通过复制成功值完成转换。
    ///
    /// 该方法要求 `T: Copy`，错误分支保持不变。
    #[inline]
    #[stable(feature = "result_copied", since = "1.59.0")]
    #[rustc_const_stable(feature = "const_result", since = "1.83.0")]
    #[rustc_allow_const_fn_unstable(const_precise_live_drops)]
    pub const fn copied(self) -> Result<T, E>
    where
        T: Copy,
    {
        // FIXME(const-hack): 这个实现绕开了尚未 const-ready 的 `Result::map`；
        // 等可行时应改回去以避免重复代码。
        match self {
            Ok(&mut v) => Ok(v),
            Err(e) => Err(e),
        }
    }

    /// 把 `Result<&T, E>` 或 `Result<&mut T, E>` 转换为 `Result<T, E>`，通过克隆成功值完成转换。
    ///
    /// 该方法要求 `T: Clone`，并只在 [`Ok`] 分支调用 `clone`。
    #[inline]
    #[stable(feature = "result_cloned", since = "1.59.0")]
    pub fn cloned(self) -> Result<T, E>
    where
        T: Clone,
    {
        self.map(|t| t.clone())
    }
}

impl<T, E> Result<Option<T>, E> {
    /// 把 `Result<Option<T>, E>` 转换为 `Option<Result<T, E>>`。
    ///
    /// `Ok(Some(v))` 变为 `Some(Ok(v))`，`Ok(None)` 变为 [`None`]，`Err(e)` 变为 `Some(Err(e))`。
    #[inline]
    #[stable(feature = "transpose_result", since = "1.33.0")]
    #[rustc_const_stable(feature = "const_result", since = "1.83.0")]
    #[rustc_allow_const_fn_unstable(const_precise_live_drops)]
    pub const fn transpose(self) -> Option<Result<T, E>> {
        match self {
            Ok(Some(x)) => Some(Ok(x)),
            Ok(None) => None,
            Err(e) => Some(Err(e)),
        }
    }
}

impl<T, E> Result<Result<T, E>, E> {
    /// 移除一层 `Result` 嵌套。
    ///
    /// `Ok(Ok(v))` 变为 `Ok(v)`，`Ok(Err(e))` 和 `Err(e)` 都保留为错误。
    #[inline]
    #[stable(feature = "result_flattening", since = "1.89.0")]
    #[rustc_allow_const_fn_unstable(const_precise_live_drops)]
    #[rustc_const_stable(feature = "result_flattening", since = "1.89.0")]
    pub const fn flatten(self) -> Result<T, E> {
        // FIXME(const-hack): 可以改写为 `and_then`。
        match self {
            Ok(inner) => inner,
            Err(e) => Err(e),
        }
    }
}

// 单独拆出此函数，用于减小这些方法的代码体积。
#[cfg(not(panic = "immediate-abort"))]
#[inline(never)]
#[cold]
#[track_caller]
fn unwrap_failed(msg: &str, error: &dyn fmt::Debug) -> ! {
    panic!("{msg}: {error:?}");
}

// 单独拆出此函数，是为了避免构造一个随即被丢弃的 `dyn Debug`。
// 一旦构造了 trait object，即使之后完全未使用，dead code elimination 也不会清理它的 vtable。
#[cfg(panic = "immediate-abort")]
#[inline]
#[cold]
#[track_caller]
const fn unwrap_failed<T>(_msg: &str, _error: &T) -> ! {
    panic!()
}

/////////////////////////////////////////////////////////////////////////////
// trait 实现
/////////////////////////////////////////////////////////////////////////////

#[stable(feature = "rust1", since = "1.0.0")]
impl<T, E> Clone for Result<T, E>
where
    T: Clone,
    E: Clone,
{
    #[inline]
    fn clone(&self) -> Self {
        match self {
            Ok(x) => Ok(x.clone()),
            Err(x) => Err(x.clone()),
        }
    }

    #[inline]
    fn clone_from(&mut self, source: &Self) {
        match (self, source) {
            (Ok(to), Ok(from)) => to.clone_from(from),
            (Err(to), Err(from)) => to.clone_from(from),
            (to, from) => *to = from.clone(),
        }
    }
}

#[unstable(feature = "ergonomic_clones", issue = "132290")]
impl<T, E> crate::clone::UseCloned for Result<T, E>
where
    T: crate::clone::UseCloned,
    E: crate::clone::UseCloned,
{
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T, E> IntoIterator for Result<T, E> {
    type Item = T;
    type IntoIter = IntoIter<T>;

    /// 消费 `Result` 并返回迭代器。
    ///
    /// 若为 [`Ok`]，迭代器产生一个 `T`；若为 [`Err`]，迭代器为空。
    #[inline]
    fn into_iter(self) -> IntoIter<T> {
        IntoIter { inner: self.ok() }
    }
}

#[stable(since = "1.4.0", feature = "result_iter")]
impl<'a, T, E> IntoIterator for &'a Result<T, E> {
    type Item = &'a T;
    type IntoIter = Iter<'a, T>;

    fn into_iter(self) -> Iter<'a, T> {
        self.iter()
    }
}

#[stable(since = "1.4.0", feature = "result_iter")]
impl<'a, T, E> IntoIterator for &'a mut Result<T, E> {
    type Item = &'a mut T;
    type IntoIter = IterMut<'a, T>;

    fn into_iter(self) -> IterMut<'a, T> {
        self.iter_mut()
    }
}

/////////////////////////////////////////////////////////////////////////////
// Result 迭代器
/////////////////////////////////////////////////////////////////////////////

/// 遍历 [`Option`] 或 [`Result`] 中共享引用的迭代器。
///
/// 它最多产生一个元素：存在成功/有效值时产生引用，否则为空。
#[derive(Debug)]
#[stable(feature = "rust1", since = "1.0.0")]
pub struct Iter<'a, T: 'a> {
    inner: Option<&'a T>,
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<'a, T> Iterator for Iter<'a, T> {
    type Item = &'a T;

    #[inline]
    fn next(&mut self) -> Option<&'a T> {
        self.inner.take()
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = if self.inner.is_some() { 1 } else { 0 };
        (n, Some(n))
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<'a, T> DoubleEndedIterator for Iter<'a, T> {
    #[inline]
    fn next_back(&mut self) -> Option<&'a T> {
        self.inner.take()
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T> ExactSizeIterator for Iter<'_, T> {}

#[stable(feature = "fused", since = "1.26.0")]
impl<T> FusedIterator for Iter<'_, T> {}

#[unstable(feature = "trusted_len", issue = "37572")]
unsafe impl<A> TrustedLen for Iter<'_, A> {}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T> Clone for Iter<'_, T> {
    #[inline]
    fn clone(&self) -> Self {
        Iter { inner: self.inner }
    }
}

/// 遍历 [`Option`] 或 [`Result`] 中可变引用的迭代器。
///
/// 它最多产生一个元素：存在成功/有效值时产生可变引用，否则为空。
#[derive(Debug)]
#[stable(feature = "rust1", since = "1.0.0")]
pub struct IterMut<'a, T: 'a> {
    inner: Option<&'a mut T>,
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<'a, T> Iterator for IterMut<'a, T> {
    type Item = &'a mut T;

    #[inline]
    fn next(&mut self) -> Option<&'a mut T> {
        self.inner.take()
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = if self.inner.is_some() { 1 } else { 0 };
        (n, Some(n))
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<'a, T> DoubleEndedIterator for IterMut<'a, T> {
    #[inline]
    fn next_back(&mut self) -> Option<&'a mut T> {
        self.inner.take()
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T> ExactSizeIterator for IterMut<'_, T> {}

#[stable(feature = "fused", since = "1.26.0")]
impl<T> FusedIterator for IterMut<'_, T> {}

#[unstable(feature = "trusted_len", issue = "37572")]
unsafe impl<A> TrustedLen for IterMut<'_, A> {}

/// 消费 [`Option`] 或 [`Result`] 并遍历其中值的迭代器。
///
/// 它最多产生一个元素，缺值或错误分支会表现为空迭代器。
#[derive(Clone, Debug)]
#[stable(feature = "rust1", since = "1.0.0")]
pub struct IntoIter<T> {
    inner: Option<T>,
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T> Iterator for IntoIter<T> {
    type Item = T;

    #[inline]
    fn next(&mut self) -> Option<T> {
        self.inner.take()
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = if self.inner.is_some() { 1 } else { 0 };
        (n, Some(n))
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T> DoubleEndedIterator for IntoIter<T> {
    #[inline]
    fn next_back(&mut self) -> Option<T> {
        self.inner.take()
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T> ExactSizeIterator for IntoIter<T> {}

#[stable(feature = "fused", since = "1.26.0")]
impl<T> FusedIterator for IntoIter<T> {}

#[unstable(feature = "trusted_len", issue = "37572")]
unsafe impl<A> TrustedLen for IntoIter<A> {}

/////////////////////////////////////////////////////////////////////////////
// FromIterator 实现
/////////////////////////////////////////////////////////////////////////////

#[stable(feature = "rust1", since = "1.0.0")]
impl<A, E, V: FromIterator<A>> FromIterator<Result<A, E>> for Result<V, E> {
    /// 从 `Iterator<Item = Result<A, E>>` 收集出 `Result<V, E>`。
    ///
    /// 迭代过程中只要遇到第一个 [`Err`] 就短路并返回该错误；如果全部为 [`Ok`]，则把所有成功值收集进目标集合。
    #[inline]
    fn from_iter<I: IntoIterator<Item = Result<A, E>>>(iter: I) -> Result<V, E> {
        iter::try_process(iter.into_iter(), |i| i.collect())
    }
}

#[unstable(feature = "try_trait_v2", issue = "84277", old_name = "try_trait")]
#[rustc_const_unstable(feature = "const_try", issue = "74935")]
impl<T, E> const ops::Try for Result<T, E> {
    type Output = T;
    type Residual = Result<convert::Infallible, E>;

    #[inline]
    fn from_output(output: Self::Output) -> Self {
        Ok(output)
    }

    #[inline]
    fn branch(self) -> ControlFlow<Self::Residual, Self::Output> {
        match self {
            Ok(v) => ControlFlow::Continue(v),
            Err(e) => ControlFlow::Break(Err(e)),
        }
    }
}

#[unstable(feature = "try_trait_v2", issue = "84277", old_name = "try_trait")]
#[rustc_const_unstable(feature = "const_try", issue = "74935")]
impl<T, E, F: [const] From<E>> const ops::FromResidual<Result<convert::Infallible, E>>
    for Result<T, F>
{
    #[inline]
    #[track_caller]
    fn from_residual(residual: Result<convert::Infallible, E>) -> Self {
        match residual {
            Err(e) => Err(From::from(e)),
        }
    }
}
#[diagnostic::do_not_recommend]
#[unstable(feature = "try_trait_v2_yeet", issue = "96374")]
#[rustc_const_unstable(feature = "const_try", issue = "74935")]
impl<T, E, F: [const] From<E>> const ops::FromResidual<ops::Yeet<E>> for Result<T, F> {
    #[inline]
    fn from_residual(ops::Yeet(e): ops::Yeet<E>) -> Self {
        Err(From::from(e))
    }
}

#[unstable(feature = "try_trait_v2_residual", issue = "91285")]
#[rustc_const_unstable(feature = "const_try", issue = "74935")]
impl<T, E> const ops::Residual<T> for Result<convert::Infallible, E> {
    type TryType = Result<T, E>;
}
