//! 一个「一次性初始化」（once initialization）原语
//!
//! 该原语用于执行一次性初始化。一个典型用例是初始化某个 FFI 库。

use crate::fmt;
use crate::panic::{RefUnwindSafe, UnwindSafe};
use crate::sys::sync as sys;

/// 用于一次性全局执行的底层同步原语。
///
/// 在过去，这曾是 `std` 中唯一的「执行一次」（execute once）同步设施。
/// 在 [`OnceLock<T>`] 或 [`LazyLock<T, F>`] 被加入 `std` 之前，其他库曾用
/// `Once` 实现各种新颖的同步类型。其中 `OnceLock<T>` 在功能上取代了 `Once`，
/// 当 `Once` 与某份数据相关联这一常见场景下，应优先选用它。
///
/// 该类型只能通过 [`Once::new()`] 构造。
///
/// # 示例
///
/// ```
/// use std::sync::Once;
///
/// static START: Once = Once::new();
///
/// START.call_once(|| {
///     // 在此处运行初始化
/// });
/// ```
///
/// [`OnceLock<T>`]: crate::sync::OnceLock
/// [`LazyLock<T, F>`]: crate::sync::LazyLock
#[stable(feature = "rust1", since = "1.0.0")]
pub struct Once {
    inner: sys::Once,
}

#[stable(feature = "sync_once_unwind_safe", since = "1.59.0")]
impl UnwindSafe for Once {}

#[stable(feature = "sync_once_unwind_safe", since = "1.59.0")]
impl RefUnwindSafe for Once {}

/// 交给 [`Once::call_once_force()`] 闭包参数的状态。可用它来查询 [`Once`]
/// 的中毒（poison）状态。
#[stable(feature = "once_poison", since = "1.51.0")]
pub struct OnceState {
    pub(crate) inner: sys::OnceState,
}

/// 用于不同平台上 `sys::sync::once` 的内部实现，以及
/// [`LazyLock`](crate::sync::LazyLock) 的实现。
pub(crate) enum OnceExclusiveState {
    Incomplete,
    Poisoned,
    Complete,
}

/// 用于静态 [`Once`] 值的初始化值。
///
/// # 示例
///
/// ```
/// use std::sync::{Once, ONCE_INIT};
///
/// static START: Once = ONCE_INIT;
/// ```
#[stable(feature = "rust1", since = "1.0.0")]
#[deprecated(
    since = "1.38.0",
    note = "the `Once::new()` function is now preferred",
    suggestion = "Once::new()"
)]
pub const ONCE_INIT: Once = Once::new();

impl Once {
    /// 创建一个新的 `Once` 值。
    #[inline]
    #[stable(feature = "once_new", since = "1.2.0")]
    #[rustc_const_stable(feature = "const_once_new", since = "1.32.0")]
    #[must_use]
    pub const fn new() -> Once {
        Once { inner: sys::Once::new() }
    }

    /// 仅且只执行一次初始化例程。如果这是第一次调用 `call_once`，给定的闭包
    /// 就会被执行；否则该例程将 *不会* 被调用。
    ///
    /// 如果当前有另一个初始化例程正在运行，本方法会阻塞调用线程。
    ///
    /// 本函数返回时，保证某个初始化已经运行并完成（不一定是这次指定的那个
    /// 闭包）。同时还保证：所执行闭包做出的任何内存写入，此时都能被其他线程
    /// 可靠地观测到（在该闭包与返回之后执行的代码之间存在 happens-before
    /// 关系）。
    ///
    /// 如果给定的闭包对同一个 [`Once`] 实例递归调用 `call_once`，确切行为未作
    /// 规定：允许的结果是 panic 或死锁。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::sync::Once;
    ///
    /// static mut VAL: usize = 0;
    /// static INIT: Once = Once::new();
    ///
    /// // 访问 `static mut` 在大多数情况下是 unsafe 的，但如果我们以同步的
    /// // 方式访问（例如只写一次、或全部为读），那就没问题！
    /// //
    /// // 本函数只会调用 `expensive_computation` 一次，此后总是返回首次调用
    /// // 所返回的值。
    /// fn get_cached_val() -> usize {
    ///     unsafe {
    ///         INIT.call_once(|| {
    ///             VAL = expensive_computation();
    ///         });
    ///         VAL
    ///     }
    /// }
    ///
    /// fn expensive_computation() -> usize {
    ///     // ...
    /// # 2
    /// }
    /// ```
    ///
    /// # Panics
    ///
    /// 即便在众多线程间并发调用，闭包 `f` 也只会被执行一次。然而，如果该闭包
    /// panic，则它会使这个 [`Once`] 实例 *中毒*（poison），导致今后所有对
    /// `call_once` 的调用也都 panic。
    ///
    /// 这类似于 [互斥锁的中毒机制][poison]，但本机制保证绝不会跳过 `f` 内部
    /// 发生的 panic。
    ///
    /// [poison]: struct.Mutex.html#poisoning
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[track_caller]
    #[rustc_should_not_be_called_on_const_items]
    pub fn call_once<F>(&self, f: F)
    where
        F: FnOnce(),
    {
        // 快速路径检查：若已完成则直接返回，避免进入慢速的加锁路径。
        if self.inner.is_completed() {
            return;
        }

        // 把 `f` 包进 `Option` 里，以便在 `call` 选中本闭包时用 `take` 取出
        // 并按值调用（`FnOnce` 只能调用一次）。
        let mut f = Some(f);
        self.inner.call(false, &mut |_| f.take().unwrap()());
    }

    /// 执行与 [`call_once()`] 相同的功能，但忽略中毒（poisoning）。
    ///
    /// 与 [`call_once()`] 不同：如果这个 [`Once`] 已经中毒（即先前对
    /// [`call_once()`] 或 [`call_once_force()`] 的调用导致了 panic），调用
    /// [`call_once_force()`] 仍会执行闭包 `f`，并且 _不会_ 立即 panic。若 `f`
    /// panic，则该 [`Once`] 将保持中毒状态。若 `f` _未_ panic，则该 [`Once`]
    /// 将不再处于中毒状态，今后所有对 [`call_once()`] 或 [`call_once_force()`]
    /// 的调用都会成为空操作（no-op）。
    ///
    /// 闭包 `f` 会被交予一个 [`OnceState`] 结构体，可用它查询该 [`Once`] 的
    /// 中毒状态。
    ///
    /// [`call_once()`]: Once::call_once
    /// [`call_once_force()`]: Once::call_once_force
    ///
    /// # 示例
    ///
    /// ```
    /// use std::sync::Once;
    /// use std::thread;
    ///
    /// static INIT: Once = Once::new();
    ///
    /// // 使这个 once 中毒
    /// let handle = thread::spawn(|| {
    ///     INIT.call_once(|| panic!());
    /// });
    /// assert!(handle.join().is_err());
    ///
    /// // 中毒会传播
    /// let handle = thread::spawn(|| {
    ///     INIT.call_once(|| {});
    /// });
    /// assert!(handle.join().is_err());
    ///
    /// // call_once_force 仍会运行，并重置中毒状态
    /// INIT.call_once_force(|state| {
    ///     assert!(state.is_poisoned());
    /// });
    ///
    /// // 一旦有任何一次成功，我们就停止传播中毒
    /// INIT.call_once(|| {});
    /// ```
    #[inline]
    #[stable(feature = "once_poison", since = "1.51.0")]
    #[rustc_should_not_be_called_on_const_items]
    pub fn call_once_force<F>(&self, f: F)
    where
        F: FnOnce(&OnceState),
    {
        // 快速路径检查
        if self.inner.is_completed() {
            return;
        }

        let mut f = Some(f);
        self.inner.call(true, &mut |p| f.take().unwrap()(p));
    }

    /// 如果某次 [`call_once()`] 调用已成功完成，则返回 `true`。具体而言，
    /// `is_completed` 在以下情形会返回 false：
    ///   * [`call_once()`] 根本没被调用过，
    ///   * [`call_once()`] 被调用了，但尚未完成，
    ///   * 该 [`Once`] 实例已中毒
    ///
    /// 本函数返回 `false` 并不意味着 [`Once`] 没有被执行过。例如，它可能正好
    /// 在 `is_completed` 开始执行到返回之间被执行，这种情况下 `false` 返回值
    /// 就是过时的（但仍然是允许的）。
    ///
    /// [`call_once()`]: Once::call_once
    ///
    /// # 示例
    ///
    /// ```
    /// use std::sync::Once;
    ///
    /// static INIT: Once = Once::new();
    ///
    /// assert_eq!(INIT.is_completed(), false);
    /// INIT.call_once(|| {
    ///     assert_eq!(INIT.is_completed(), false);
    /// });
    /// assert_eq!(INIT.is_completed(), true);
    /// ```
    ///
    /// ```
    /// use std::sync::Once;
    /// use std::thread;
    ///
    /// static INIT: Once = Once::new();
    ///
    /// assert_eq!(INIT.is_completed(), false);
    /// let handle = thread::spawn(|| {
    ///     INIT.call_once(|| panic!());
    /// });
    /// assert!(handle.join().is_err());
    /// assert_eq!(INIT.is_completed(), false);
    /// ```
    #[stable(feature = "once_is_completed", since = "1.43.0")]
    #[inline]
    pub fn is_completed(&self) -> bool {
        self.inner.is_completed()
    }

    /// 阻塞当前线程，直到初始化完成为止。
    ///
    /// # 示例
    ///
    /// ```rust
    /// use std::sync::Once;
    /// use std::thread;
    ///
    /// static READY: Once = Once::new();
    ///
    /// let thread = thread::spawn(|| {
    ///     READY.wait();
    ///     println!("everything is ready");
    /// });
    ///
    /// READY.call_once(|| println!("performing setup"));
    /// ```
    ///
    /// # Panics
    ///
    /// 如果这个 [`Once`] 因某个初始化闭包发生 panic 而中毒，本方法也会 panic。
    /// 若不希望这种行为，请使用 [`wait_force`](Self::wait_force)。
    #[stable(feature = "once_wait", since = "1.86.0")]
    #[rustc_should_not_be_called_on_const_items]
    pub fn wait(&self) {
        if !self.inner.is_completed() {
            self.inner.wait(false);
        }
    }

    /// 阻塞当前线程，直到初始化完成为止，并忽略中毒（poisoning）。
    ///
    /// 如果这个 [`Once`] 已中毒，本函数会一直阻塞直到它完成；这与
    /// [`Once::wait()`] 不同——后者在此情况下会 panic。
    #[stable(feature = "once_wait", since = "1.86.0")]
    #[rustc_should_not_be_called_on_const_items]
    pub fn wait_force(&self) {
        if !self.inner.is_completed() {
            self.inner.wait(true);
        }
    }

    /// 返回该 `Once` 实例的当前状态。
    ///
    /// 由于本方法获取的是可变引用（mutable reference），当前不可能有任何
    /// 初始化正在运行，因此状态必定是 "incomplete"、"poisoned" 或 "complete"
    /// 三者之一。
    #[inline]
    pub(crate) fn state(&mut self) -> OnceExclusiveState {
        self.inner.state()
    }

    /// 设置该 `Once` 实例的当前状态。
    ///
    /// 由于本方法获取的是可变引用，当前不可能有任何初始化正在运行，因此状态
    /// 必定是 "incomplete"、"poisoned" 或 "complete" 三者之一。
    #[inline]
    pub(crate) fn set_state(&mut self, new_state: OnceExclusiveState) {
        self.inner.set_state(new_state);
    }
}

#[stable(feature = "std_debug", since = "1.16.0")]
impl fmt::Debug for Once {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Once").finish_non_exhaustive()
    }
}

impl OnceState {
    /// 如果关联的 [`Once`] 在传给 [`Once::call_once_force()`] 的闭包被调用之前
    /// 就已中毒，则返回 `true`。
    ///
    /// # 示例
    ///
    /// 一个已中毒的 [`Once`]：
    ///
    /// ```
    /// use std::sync::Once;
    /// use std::thread;
    ///
    /// static INIT: Once = Once::new();
    ///
    /// // 使这个 once 中毒
    /// let handle = thread::spawn(|| {
    ///     INIT.call_once(|| panic!());
    /// });
    /// assert!(handle.join().is_err());
    ///
    /// INIT.call_once_force(|state| {
    ///     assert!(state.is_poisoned());
    /// });
    /// ```
    ///
    /// 一个未中毒的 [`Once`]：
    ///
    /// ```
    /// use std::sync::Once;
    ///
    /// static INIT: Once = Once::new();
    ///
    /// INIT.call_once_force(|state| {
    ///     assert!(!state.is_poisoned());
    /// });
    #[stable(feature = "once_poison", since = "1.51.0")]
    #[inline]
    pub fn is_poisoned(&self) -> bool {
        self.inner.is_poisoned()
    }

    /// 使关联的 [`Once`] 中毒，而无需显式地 panic。
    // 注意：目前仅为 `OnceLock` 暴露此方法。
    #[inline]
    pub(crate) fn poison(&self) {
        self.inner.poison();
    }
}

#[stable(feature = "std_debug", since = "1.16.0")]
impl fmt::Debug for OnceState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OnceState").field("poisoned", &self.is_poisoned()).finish()
    }
}
