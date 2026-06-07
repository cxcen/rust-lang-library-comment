use crate::async_iter::AsyncIterator;
use crate::cell::UnsafeCell;
use crate::fmt;
use crate::future::Future;
use crate::ops::{Deref, DerefMut};
use crate::pin::Pin;
use crate::ptr::{NonNull, Unique};
use crate::task::{Context, Poll};

/// 表示 Rust 中“panic 安全”类型的标记 trait。
///
/// 这个 trait 会默认为许多类型实现，在实现推导方式上类似 [`Send`] 和 [`Sync`]。它的用途是
/// 描述哪些类型可以跨过 [`catch_unwind`] 边界，而不容易让调用方在捕获 panic 后观察到已被
/// panic 打断的逻辑不变量。
///
/// [`catch_unwind`]: ../../std/panic/fn.catch_unwind.html
///
/// ## 什么是 unwind safety？
///
/// 在 Rust 中，如果函数自身 panic，或调用链中更深层函数发生 panic，函数都可能“提前返回”。
/// 这种控制流并不总是被业务逻辑预期到，并且可能通过两个关键因素组合出隐蔽 bug：
///
/// 1. 线程 panic 时，某个数据结构正处在临时无效状态。
/// 2. 后续代码又观察到了这个已经被破坏的不变量。
///
/// 通常在 Rust 中，第二步并不容易发生：捕获 panic 要么需要新建线程（这反而让后续观察同一组
/// 被破坏不变量变得困难），要么需要使用本模块中的 `catch_unwind` 函数。此外，即使确实观察到
/// 破坏的不变量，Rust 中通常也不会像 C/C++ 那样因为未初始化值而立刻造成内存不安全。
///
/// 但是，Rust 仍然可能破坏 **逻辑** 不变量，并最终导致行为 bug。Rust 的 unwind safety 还有
/// 一个关键点：在没有 `unsafe` 代码参与时，panic 不会导致内存不安全。
///
/// 上面只是 unwind safety 的快速概览。关于它在 Rust 中的适用方式，更多信息见
/// [相关 RFC][rfc]。
///
/// [rfc]: https://github.com/rust-lang/rfcs/blob/master/text/1236-stabilize-catch-panic.md
///
/// ## 什么是 `UnwindSafe`？
///
/// 理解 unwind safety 之后，还需要明确这个 trait 表示什么。如上所述，观察破坏不变量的一种
/// 方式是通过本模块的 `catch_unwind` 捕获 panic，然后继续复用闭包捕获的环境。
///
/// 简单说，如果类型 `T` 不容易让使用者通过 `catch_unwind`（捕获 panic）观察到破坏的不变量，
/// 那么它就实现 `UnwindSafe`。这是一个 auto trait，因此会自动为许多类型实现，并且按结构组合：
/// 例如结构体的所有字段都 unwind safe 时，结构体本身也会 unwind safe。
///
/// 需要注意，这不是 unsafe trait，因此它没有类似“实现者必须维护某些不变量，否则 UB”的简短
/// 契约。它更像一个“减速带”：提醒 `catch_unwind` 的使用者，跨 panic 边界后可能观察到破坏的
/// 逻辑不变量，需要主动纳入正确性分析。
///
/// ## 谁会实现 `UnwindSafe`？
///
/// `&mut T` 和 `&RefCell<T>` 这类类型是 **不** unwind safe 的例子。一般原则是：任何可以跨
/// `catch_unwind` 共享的可变状态，默认都不应视为 unwind safe。原因是捕获 panic 之后，外部代码
/// 可以像平常一样继续访问这些数据，从而很容易观察到中途被打断的不变量。
///
/// 另一方面，`&Mutex<T>` 这类类型通常是 unwind safe 的，因为它们默认实现 poisoning。它们仍然
/// 可能让调用方观察到破坏的不变量，但已经提供了自己的“减速带”来显式暴露这一风险。
///
/// ## 什么时候使用 `UnwindSafe`？
///
/// 大多数类型或函数并不需要关心这个 trait。它主要作为 `catch_unwind` 的约束使用；如前所述，
/// 它不是 `unsafe`，因此更多是建议性信号。包装类型 [`AssertUnwindSafe`] 可用于强制把传给
/// `catch_unwind` 的捕获变量视为实现了此 trait。
#[stable(feature = "catch_unwind", since = "1.9.0")]
#[rustc_diagnostic_item = "unwind_safe_trait"]
#[diagnostic::on_unimplemented(
    message = "the type `{Self}` may not be safely transferred across an unwind boundary",
    label = "`{Self}` may not be safely transferred across an unwind boundary"
)]
pub auto trait UnwindSafe {}

/// 标记某些类型的共享引用可被视为 unwind safe 的 trait。
///
/// 这个 trait 明确不会为 [`UnsafeCell`] 实现；[`UnsafeCell`] 是所有内部可变性的根基。
///
/// 它是辅助性的标记 trait，用于为 [`UnwindSafe`] 提供实现规则。更多背景见 [`UnwindSafe`]
/// 的文档。
#[stable(feature = "catch_unwind", since = "1.9.0")]
#[rustc_diagnostic_item = "ref_unwind_safe_trait"]
#[diagnostic::on_unimplemented(
    message = "the type `{Self}` may contain interior mutability and a reference may not be safely \
               transferable across a catch_unwind boundary",
    label = "`{Self}` may contain interior mutability and a reference may not be safely \
             transferable across a catch_unwind boundary"
)]
pub auto trait RefUnwindSafe {}

/// 一个简单包装器，用来断言某个类型应被视为 unwind safe。
///
/// 使用 [`catch_unwind`] 时，闭包捕获的某些变量可能不是 unwind safe。例如捕获 `&mut T` 时，
/// 编译器会警告它默认不 unwind safe。但是在某些特定用法中，如果调用方已经明确分析过 unwind
/// safety，这不一定是真正的问题。该包装结构可作为轻量注解，表达“这个变量在此处确实可以跨过
/// panic 捕获边界”。
///
/// [`catch_unwind`]: ../../std/panic/fn.catch_unwind.html
///
/// # 示例
///
/// `AssertUnwindSafe` 的一种用法是断言整个闭包本身 unwind safe，从而绕过对所有捕获变量的检查：
///
/// ```
/// use std::panic::{self, AssertUnwindSafe};
///
/// let mut variable = 4;
///
/// // 这段代码不能编译，因为闭包捕获了 `&mut variable`，
/// // 而 `&mut T` 默认不被视为 unwind safe。
///
/// // panic::catch_unwind(|| {
/// //     variable += 3;
/// // });
///
/// // 加上 `AssertUnwindSafe` 包装后，这段代码可以编译。
/// let result = panic::catch_unwind(AssertUnwindSafe(|| {
///     variable += 3;
/// }));
/// // ...
/// ```
///
/// 包装整个闭包等价于一次性断言所有捕获变量都 unwind safe。缺点是，如果将来新增了捕获变量，
/// 它们也会被自动视为 unwind safe。因此，更稳妥的做法可能是只包装单个捕获变量，如下所示。
/// 这样注解更多，但可以保证未来新增的非 unwind safe 捕获会继续触发编译错误，迫使你重新判断
/// 新捕获是否真的代表 bug。
///
/// ```
/// use std::panic::{self, AssertUnwindSafe};
///
/// let mut variable = 4;
/// let other_capture = 3;
///
/// let result = {
///     let mut wrapper = AssertUnwindSafe(&mut variable);
///     panic::catch_unwind(move || {
///         **wrapper += other_capture;
///     })
/// };
/// // ...
/// ```
#[stable(feature = "catch_unwind", since = "1.9.0")]
pub struct AssertUnwindSafe<T>(#[stable(feature = "catch_unwind", since = "1.9.0")] pub T);

// `UnwindSafe` trait 的实现规则：
//
// * 默认情况下，类型被视为 unwind safe。
// * 含有某种共享可变性的指针/引用默认不是 unwind safe。
// * Unique 作为拥有所有权的指针，会提升内部类型的实现。
// * Mutex/RwLock 这类显式提供 poisoning 的类型是 unwind safe。
// * 自定义的 AssertUnwindSafe 包装器明确是 unwind safe。

#[stable(feature = "catch_unwind", since = "1.9.0")]
impl<T: ?Sized> !UnwindSafe for &mut T {}
#[stable(feature = "catch_unwind", since = "1.9.0")]
impl<T: RefUnwindSafe + ?Sized> UnwindSafe for &T {}
#[stable(feature = "catch_unwind", since = "1.9.0")]
impl<T: RefUnwindSafe + ?Sized> UnwindSafe for *const T {}
#[stable(feature = "catch_unwind", since = "1.9.0")]
impl<T: RefUnwindSafe + ?Sized> UnwindSafe for *mut T {}
#[unstable(feature = "ptr_internals", issue = "none")]
impl<T: UnwindSafe + ?Sized> UnwindSafe for Unique<T> {}
#[stable(feature = "nonnull", since = "1.25.0")]
impl<T: RefUnwindSafe + ?Sized> UnwindSafe for NonNull<T> {}
#[stable(feature = "catch_unwind", since = "1.9.0")]
impl<T> UnwindSafe for AssertUnwindSafe<T> {}

// `RefUnwindSafe` 标记 trait 的实现相对直接：
// 基本含义是只有 `UnsafeCell` 不实现它，并且这个结论会传递到包含 `UnsafeCell`
// 的其他类型上。
#[stable(feature = "catch_unwind", since = "1.9.0")]
impl<T: ?Sized> !RefUnwindSafe for UnsafeCell<T> {}
#[stable(feature = "catch_unwind", since = "1.9.0")]
impl<T> RefUnwindSafe for AssertUnwindSafe<T> {}

#[cfg(target_has_atomic_load_store = "ptr")]
#[stable(feature = "unwind_safe_atomic_refs", since = "1.14.0")]
impl RefUnwindSafe for crate::sync::atomic::AtomicIsize {}
#[cfg(target_has_atomic_load_store = "8")]
#[stable(feature = "integer_atomics_stable", since = "1.34.0")]
impl RefUnwindSafe for crate::sync::atomic::AtomicI8 {}
#[cfg(target_has_atomic_load_store = "16")]
#[stable(feature = "integer_atomics_stable", since = "1.34.0")]
impl RefUnwindSafe for crate::sync::atomic::AtomicI16 {}
#[cfg(target_has_atomic_load_store = "32")]
#[stable(feature = "integer_atomics_stable", since = "1.34.0")]
impl RefUnwindSafe for crate::sync::atomic::AtomicI32 {}
#[cfg(target_has_atomic_load_store = "64")]
#[stable(feature = "integer_atomics_stable", since = "1.34.0")]
impl RefUnwindSafe for crate::sync::atomic::AtomicI64 {}
#[cfg(target_has_atomic_load_store = "128")]
#[unstable(feature = "integer_atomics", issue = "99069")]
impl RefUnwindSafe for crate::sync::atomic::AtomicI128 {}

#[cfg(target_has_atomic_load_store = "ptr")]
#[stable(feature = "unwind_safe_atomic_refs", since = "1.14.0")]
impl RefUnwindSafe for crate::sync::atomic::AtomicUsize {}
#[cfg(target_has_atomic_load_store = "8")]
#[stable(feature = "integer_atomics_stable", since = "1.34.0")]
impl RefUnwindSafe for crate::sync::atomic::AtomicU8 {}
#[cfg(target_has_atomic_load_store = "16")]
#[stable(feature = "integer_atomics_stable", since = "1.34.0")]
impl RefUnwindSafe for crate::sync::atomic::AtomicU16 {}
#[cfg(target_has_atomic_load_store = "32")]
#[stable(feature = "integer_atomics_stable", since = "1.34.0")]
impl RefUnwindSafe for crate::sync::atomic::AtomicU32 {}
#[cfg(target_has_atomic_load_store = "64")]
#[stable(feature = "integer_atomics_stable", since = "1.34.0")]
impl RefUnwindSafe for crate::sync::atomic::AtomicU64 {}
#[cfg(target_has_atomic_load_store = "128")]
#[unstable(feature = "integer_atomics", issue = "99069")]
impl RefUnwindSafe for crate::sync::atomic::AtomicU128 {}

#[cfg(target_has_atomic_load_store = "8")]
#[stable(feature = "unwind_safe_atomic_refs", since = "1.14.0")]
impl RefUnwindSafe for crate::sync::atomic::AtomicBool {}

#[cfg(target_has_atomic_load_store = "ptr")]
#[stable(feature = "unwind_safe_atomic_refs", since = "1.14.0")]
impl<T> RefUnwindSafe for crate::sync::atomic::AtomicPtr<T> {}

#[stable(feature = "catch_unwind", since = "1.9.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl<T> const Deref for AssertUnwindSafe<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.0
    }
}

#[stable(feature = "catch_unwind", since = "1.9.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl<T> const DerefMut for AssertUnwindSafe<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

#[stable(feature = "catch_unwind", since = "1.9.0")]
impl<R, F: FnOnce() -> R> FnOnce<()> for AssertUnwindSafe<F> {
    type Output = R;

    #[inline]
    extern "rust-call" fn call_once(self, _args: ()) -> R {
        (self.0)()
    }
}

#[stable(feature = "std_debug", since = "1.16.0")]
impl<T: fmt::Debug> fmt::Debug for AssertUnwindSafe<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("AssertUnwindSafe").field(&self.0).finish()
    }
}

#[stable(feature = "assertunwindsafe_default", since = "1.62.0")]
impl<T: Default> Default for AssertUnwindSafe<T> {
    fn default() -> Self {
        Self(Default::default())
    }
}

#[stable(feature = "futures_api", since = "1.36.0")]
impl<F: Future> Future for AssertUnwindSafe<F> {
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // SAFETY: 这是 pin 投影；AssertUnwindSafe 遵循结构化 pinning，投影到字段不会移动内部值。
        let pinned_field = unsafe { Pin::map_unchecked_mut(self, |x| &mut x.0) };
        F::poll(pinned_field, cx)
    }
}

#[unstable(feature = "async_iterator", issue = "79024")]
impl<S: AsyncIterator> AsyncIterator for AssertUnwindSafe<S> {
    type Item = S::Item;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<S::Item>> {
        // SAFETY: 这是 pin 投影；AssertUnwindSafe 遵循结构化 pinning，投影到字段不会移动内部值。
        unsafe { self.map_unchecked_mut(|x| &mut x.0) }.poll_next(cx)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.0.size_hint()
    }
}
