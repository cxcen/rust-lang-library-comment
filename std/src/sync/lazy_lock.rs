use super::once::OnceExclusiveState;
use crate::cell::UnsafeCell;
use crate::mem::ManuallyDrop;
use crate::ops::{Deref, DerefMut};
use crate::panic::{RefUnwindSafe, UnwindSafe};
use crate::sync::Once;
use crate::{fmt, ptr};

// 我们用一个 Once 的状态作为判别值（discriminant）。创建时状态为
// "incomplete"，此时 `f` 持有初始化闭包。在第一次调用 `call_once` 时，
// `f` 被取出并运行。若成功，则设置 `value` 并把状态改为 "complete"。
// 若它 panic，则该 Once 中毒，于是这两个字段都不会被初始化。
union Data<T, F> {
    value: ManuallyDrop<T>,
    f: ManuallyDrop<F>,
}

/// 一个在首次访问时才被初始化的值。
///
/// 该类型是线程安全版本的 [`LazyCell`]，可用于静态项（statics）。
/// 由于初始化可能从多个线程发起，若当前有另一个初始化例程正在运行，
/// 任何解引用调用都会阻塞调用线程。
///
/// [`LazyCell`]: crate::cell::LazyCell
///
/// # 中毒（Poisoning）
///
/// 如果传给 [`LazyLock::new`] 的初始化闭包 panic，该锁就会中毒。一旦锁中毒，
/// 任何试图访问它（通过解引用，或显式调用 [`force()`]）的线程都会 panic。
///
/// 这一概念类似于 [`std::sync::poison`] 模块中的中毒。但一个关键区别是：
/// `LazyLock` 中的中毒是 _不可恢复的_。今后来自其他线程的所有访问都会 panic；
/// 而 [`std::sync::poison`] 中的类型（如 [`std::sync::poison::Mutex`]）则允许
/// 通过 [`PoisonError::into_inner()`] 恢复。
///
/// [`force()`]: LazyLock::force
/// [`std::sync::poison`]: crate::sync::poison
/// [`std::sync::poison::Mutex`]: crate::sync::poison::Mutex
/// [`PoisonError::into_inner()`]: crate::sync::poison::PoisonError::into_inner
///
/// # 示例
///
/// 用 `LazyLock` 初始化静态变量。
/// ```
/// use std::sync::LazyLock;
///
/// // 注意：静态项在程序结束时不会调用 [`Drop`]，因此它不会被释放。
/// // 这没有问题，因为操作系统回收已结束程序的速度比我们释放内存更快；
/// // 不过像 valgrind 这样的工具可能会报告「内存泄漏」，因为这一意图并不显然。
/// static DEEP_THOUGHT: LazyLock<String> = LazyLock::new(|| {
/// # mod another_crate {
/// #     pub fn great_question() -> String { "42".to_string() }
/// # }
///     // 在 --release 配置下，M3 Ultra 大约要花 1600 万年
///     another_crate::great_question()
/// });
///
/// // 这个 `String` 被构建、存入 `LazyLock`，并作为 `&String` 返回。
/// let _ = &*DEEP_THOUGHT;
/// ```
///
/// 用 `LazyLock` 初始化字段。
/// ```
/// use std::sync::LazyLock;
///
/// #[derive(Debug)]
/// struct UseCellLock {
///     number: LazyLock<u32>,
/// }
/// fn main() {
///     let lock: LazyLock<u32> = LazyLock::new(|| 0u32);
///
///     let data = UseCellLock { number: lock };
///     println!("{}", *data.number);
/// }
/// ```
#[stable(feature = "lazy_cell", since = "1.80.0")]
pub struct LazyLock<T, F = fn() -> T> {
    // FIXME(nonpoison_once): 如果可能，一旦不中毒版本可用，就切换到该版本
    once: Once,
    data: UnsafeCell<Data<T, F>>,
}

impl<T, F: FnOnce() -> T> LazyLock<T, F> {
    /// 用给定的初始化函数创建一个新的惰性值（lazy value）。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::sync::LazyLock;
    ///
    /// let hello = "Hello, World!".to_string();
    ///
    /// let lazy = LazyLock::new(|| hello.to_uppercase());
    ///
    /// assert_eq!(&*lazy, "HELLO, WORLD!");
    /// ```
    #[inline]
    #[stable(feature = "lazy_cell", since = "1.80.0")]
    #[rustc_const_stable(feature = "lazy_cell", since = "1.80.0")]
    pub const fn new(f: F) -> LazyLock<T, F> {
        LazyLock { once: Once::new(), data: UnsafeCell::new(Data { f: ManuallyDrop::new(f) }) }
    }

    /// 创建一个已经初始化完毕的新惰性值。
    #[inline]
    #[cfg(test)]
    pub(crate) fn preinit(value: T) -> LazyLock<T, F> {
        let once = Once::new();
        once.call_once(|| {});
        LazyLock { once, data: UnsafeCell::new(Data { value: ManuallyDrop::new(value) }) }
    }

    /// 消耗这个 `LazyLock`，返回其中存储的值。
    ///
    /// 如果 `Lazy` 已初始化，返回 `Ok(value)`；否则返回 `Err(f)`。
    ///
    /// # Panics
    ///
    /// 如果锁已中毒，则 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(lazy_cell_into_inner)]
    ///
    /// use std::sync::LazyLock;
    ///
    /// let hello = "Hello, World!".to_string();
    ///
    /// let lazy = LazyLock::new(|| hello.to_uppercase());
    ///
    /// assert_eq!(&*lazy, "HELLO, WORLD!");
    /// assert_eq!(LazyLock::into_inner(lazy).ok(), Some("HELLO, WORLD!".to_string()));
    /// ```
    #[unstable(feature = "lazy_cell_into_inner", issue = "125623")]
    pub fn into_inner(mut this: Self) -> Result<T, F> {
        let state = this.once.state();
        match state {
            OnceExclusiveState::Poisoned => panic_poisoned(),
            state => {
                // 用 `ManuallyDrop` 包裹 `this` 以阻止其 `Drop` 运行，因为我们
                // 接下来要按位读出 `data`（ptr::read）并自行接管其所有权。
                let this = ManuallyDrop::new(this);
                let data = unsafe { ptr::read(&this.data) }.into_inner();
                match state {
                    // 尚未初始化：联合体中存的是闭包 `f`，取出返回。
                    OnceExclusiveState::Incomplete => {
                        Err(ManuallyDrop::into_inner(unsafe { data.f }))
                    }
                    // 已完成初始化：联合体中存的是 `value`，取出返回。
                    OnceExclusiveState::Complete => {
                        Ok(ManuallyDrop::into_inner(unsafe { data.value }))
                    }
                    OnceExclusiveState::Poisoned => unreachable!(),
                }
            }
        }
    }

    /// 强制对这个惰性值求值，并返回结果的可变引用。
    ///
    /// # Panics
    ///
    /// 如果初始化闭包（即传给 [`new()`] 方法的那个）panic，则该 panic 会传播
    /// 给调用方，并且锁会中毒。这会导致今后对该锁的所有访问（通过 [`force()`]
    /// 或解引用）都 panic。
    ///
    /// [`new()`]: LazyLock::new
    /// [`force()`]: LazyLock::force
    ///
    /// # 示例
    ///
    /// ```
    /// use std::sync::LazyLock;
    ///
    /// let mut lazy = LazyLock::new(|| 92);
    ///
    /// let p = LazyLock::force_mut(&mut lazy);
    /// assert_eq!(*p, 92);
    /// *p = 44;
    /// assert_eq!(*lazy, 44);
    /// ```
    #[inline]
    #[stable(feature = "lazy_get", since = "1.94.0")]
    pub fn force_mut(this: &mut LazyLock<T, F>) -> &mut T {
        #[cold]
        /// # Safety
        /// 只能在状态为 `Incomplete` 时调用。
        unsafe fn really_init_mut<T, F: FnOnce() -> T>(this: &mut LazyLock<T, F>) -> &mut T {
            // 一个守卫：若初始化闭包 panic，则在其 `Drop` 中把状态设为中毒。
            struct PoisonOnPanic<'a, T, F>(&'a mut LazyLock<T, F>);
            impl<T, F> Drop for PoisonOnPanic<'_, T, F> {
                #[inline]
                fn drop(&mut self) {
                    self.0.once.set_state(OnceExclusiveState::Poisoned);
                }
            }

            // SAFETY: 我们总是在初始化器 panic 时中毒（之后便绝不再检查该数据），
            // 或者在成功时设置该数据。
            let f = unsafe { ManuallyDrop::take(&mut this.data.get_mut().f) };
            // INVARIANT: 由可变引用发起，不要 drop，因为我们已把它读出来了。
            let guard = PoisonOnPanic(this);
            let data = f();
            guard.0.data.get_mut().value = ManuallyDrop::new(data);
            guard.0.once.set_state(OnceExclusiveState::Complete);
            // 初始化成功，撤销守卫以避免它把状态错误地置为中毒。
            core::mem::forget(guard);
            // SAFETY: 我们已在上面把值放进去了。
            unsafe { &mut this.data.get_mut().value }
        }

        let state = this.once.state();
        match state {
            OnceExclusiveState::Poisoned => panic_poisoned(),
            // SAFETY: `Once` 表明我们已完成初始化。
            OnceExclusiveState::Complete => unsafe { &mut this.data.get_mut().value },
            // SAFETY: 状态为 `Incomplete`。
            OnceExclusiveState::Incomplete => unsafe { really_init_mut(this) },
        }
    }

    /// 强制对这个惰性值求值，并返回结果的引用。等价于 `Deref` 的实现，
    /// 但写法更显式。
    ///
    /// 如果当前有另一个初始化例程正在运行，本方法会阻塞调用线程。
    ///
    /// # Panics
    ///
    /// 如果初始化闭包（即传给 [`new()`] 方法的那个）panic，则该 panic 会传播
    /// 给调用方，并且锁会中毒。这会导致今后对该锁的所有访问（通过 [`force()`]
    /// 或解引用）都 panic。
    ///
    /// [`new()`]: LazyLock::new
    /// [`force()`]: LazyLock::force
    ///
    /// # 示例
    ///
    /// ```
    /// use std::sync::LazyLock;
    ///
    /// let lazy = LazyLock::new(|| 92);
    ///
    /// assert_eq!(LazyLock::force(&lazy), &92);
    /// assert_eq!(&*lazy, &92);
    /// ```
    #[inline]
    #[stable(feature = "lazy_cell", since = "1.80.0")]
    #[rustc_should_not_be_called_on_const_items]
    pub fn force(this: &LazyLock<T, F>) -> &T {
        this.once.call_once_force(|state| {
            // 用 `call_once_force` 是为了能观察中毒状态：若已中毒就 panic，
            // 从而向调用方传播不可恢复的中毒。
            if state.is_poisoned() {
                panic_poisoned();
            }

            // SAFETY: `call_once` 这个闭包永远只运行一次。
            let data = unsafe { &mut *this.data.get() };
            let f = unsafe { ManuallyDrop::take(&mut data.f) };
            let value = f();
            data.value = ManuallyDrop::new(value);
        });

        // SAFETY:
        // 有四种可能的情形：
        // * 闭包被调用并初始化了 `value`。
        // * 闭包被调用但 panic 了，于是永远到不了这一点。
        // * 闭包没被调用，但先前某次调用已初始化了 `value`。
        // * 闭包没被调用，因为该 Once 已中毒——这一情形已在上面处理。
        // 因此 `value` 必定已被初始化，且不会再被修改。
        unsafe { &*(*this.data.get()).value }
    }
}

impl<T, F> LazyLock<T, F> {
    /// 如果已初始化，返回值的可变引用。否则（未初始化或已中毒）返回 `None`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::sync::LazyLock;
    ///
    /// let mut lazy = LazyLock::new(|| 92);
    ///
    /// assert_eq!(LazyLock::get_mut(&mut lazy), None);
    /// let _ = LazyLock::force(&lazy);
    /// *LazyLock::get_mut(&mut lazy).unwrap() = 44;
    /// assert_eq!(*lazy, 44);
    /// ```
    #[inline]
    #[stable(feature = "lazy_get", since = "1.94.0")]
    pub fn get_mut(this: &mut LazyLock<T, F>) -> Option<&mut T> {
        // `state()` 不执行原子加载（atomic load），因此优先用它而非 `is_complete()`。
        let state = this.once.state();
        match state {
            // SAFETY:
            // 闭包已成功运行，因此 `value` 已被初始化。
            OnceExclusiveState::Complete => Some(unsafe { &mut this.data.get_mut().value }),
            _ => None,
        }
    }

    /// 如果已初始化，返回值的引用。否则（未初始化或已中毒）返回 `None`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::sync::LazyLock;
    ///
    /// let lazy = LazyLock::new(|| 92);
    ///
    /// assert_eq!(LazyLock::get(&lazy), None);
    /// let _ = LazyLock::force(&lazy);
    /// assert_eq!(LazyLock::get(&lazy), Some(&92));
    /// ```
    #[inline]
    #[stable(feature = "lazy_get", since = "1.94.0")]
    #[rustc_should_not_be_called_on_const_items]
    pub fn get(this: &LazyLock<T, F>) -> Option<&T> {
        if this.once.is_completed() {
            // SAFETY:
            // 闭包已成功运行，因此 `value` 已被初始化，且不会再被修改。
            Some(unsafe { &(*this.data.get()).value })
        } else {
            None
        }
    }
}

#[stable(feature = "lazy_cell", since = "1.80.0")]
impl<T, F> Drop for LazyLock<T, F> {
    fn drop(&mut self) {
        // 依据 Once 的状态决定要 drop 联合体中的哪个字段：未完成时是闭包 `f`，
        // 已完成时是 `value`，中毒时两者皆未初始化，什么都不 drop。
        match self.once.state() {
            OnceExclusiveState::Incomplete => unsafe {
                ManuallyDrop::drop(&mut self.data.get_mut().f)
            },
            OnceExclusiveState::Complete => unsafe {
                ManuallyDrop::drop(&mut self.data.get_mut().value)
            },
            OnceExclusiveState::Poisoned => {}
        }
    }
}

#[stable(feature = "lazy_cell", since = "1.80.0")]
impl<T, F: FnOnce() -> T> Deref for LazyLock<T, F> {
    type Target = T;

    /// 解引用该值。
    ///
    /// 如果当前有另一个初始化例程正在运行，本方法会阻塞调用线程。
    ///
    /// # Panics
    ///
    /// 如果初始化闭包（即传给 [`new()`] 方法的那个）panic，则该 panic 会传播
    /// 给调用方，并且锁会中毒。这会导致今后对该锁的所有访问（通过 [`force()`]
    /// 或解引用）都 panic。
    ///
    /// [`new()`]: LazyLock::new
    /// [`force()`]: LazyLock::force
    #[inline]
    fn deref(&self) -> &T {
        LazyLock::force(self)
    }
}

#[stable(feature = "lazy_deref_mut", since = "1.89.0")]
impl<T, F: FnOnce() -> T> DerefMut for LazyLock<T, F> {
    /// # Panics
    ///
    /// 如果初始化闭包（即传给 [`new()`] 方法的那个）panic，则该 panic 会传播
    /// 给调用方，并且锁会中毒。这会导致今后对该锁的所有访问（通过 [`force()`]
    /// 或解引用）都 panic。
    ///
    /// [`new()`]: LazyLock::new
    /// [`force()`]: LazyLock::force
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        LazyLock::force_mut(self)
    }
}

#[stable(feature = "lazy_cell", since = "1.80.0")]
impl<T: Default> Default for LazyLock<T> {
    /// 用 `Default` 作为初始化函数创建一个新的惰性值。
    #[inline]
    fn default() -> LazyLock<T> {
        LazyLock::new(T::default)
    }
}

#[stable(feature = "lazy_cell", since = "1.80.0")]
impl<T: fmt::Debug, F> fmt::Debug for LazyLock<T, F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut d = f.debug_tuple("LazyLock");
        match LazyLock::get(self) {
            Some(v) => d.field(v),
            None => d.field(&format_args!("<uninit>")),
        };
        d.finish()
    }
}

#[cold]
#[inline(never)]
fn panic_poisoned() -> ! {
    panic!("LazyLock instance has previously been poisoned")
}

// 我们绝不会从 `&LazyLock<T, F>` 造出 `&F`，因此不为 `F` 实现 `Sync` 也没关系。
#[stable(feature = "lazy_cell", since = "1.80.0")]
unsafe impl<T: Sync + Send, F: Send> Sync for LazyLock<T, F> {}
// 自动派生（auto-derived）的 `Send` 实现是没问题的。

#[stable(feature = "lazy_cell", since = "1.80.0")]
impl<T: RefUnwindSafe + UnwindSafe, F: UnwindSafe> RefUnwindSafe for LazyLock<T, F> {}
#[stable(feature = "lazy_cell", since = "1.80.0")]
impl<T: UnwindSafe, F: UnwindSafe> UnwindSafe for LazyLock<T, F> {}
