use super::UnsafeCell;
use crate::hint::unreachable_unchecked;
use crate::ops::{Deref, DerefMut};
use crate::{fmt, mem};

enum State<T, F> {
    Uninit(F),
    Init(T),
    Poisoned,
}

/// 一种在首次被访问时才进行初始化的值。
///
/// 本结构体的线程安全版本,参见 [`std::sync::LazyLock`]。
///
/// [`std::sync::LazyLock`]: ../../std/sync/struct.LazyLock.html
///
/// # 中毒(Poisoning）
///
/// 如果传给 [`LazyCell::new`] 的初始化闭包发生 panic,该 cell 就会被“毒化(poisoned)”。
/// 一旦 cell 被毒化,任何试图访问它的线程(无论是经由解引用,还是显式调用 [`force()`])都会 panic。
///
/// 这一概念类似于 [`std::sync::poison`] 模块中的中毒。但一个关键区别在于:`LazyCell` 中的中毒是
/// _不可恢复_ 的。此后,所有来自其他线程对该 cell 的访问都会 panic;而 [`std::sync::poison`] 中
/// 像 [`std::sync::poison::Mutex`] 这样的类型,则允许通过 [`PoisonError::into_inner()`] 进行恢复。
///
/// [`force()`]: LazyCell::force
/// [`std::sync::poison`]: ../../std/sync/poison/index.html
/// [`std::sync::poison::Mutex`]: ../../std/sync/poison/struct.Mutex.html
/// [`PoisonError::into_inner()`]: ../../std/sync/poison/struct.PoisonError.html#method.into_inner
///
/// # 示例
///
/// ```
/// use std::cell::LazyCell;
///
/// let lazy: LazyCell<i32> = LazyCell::new(|| {
///     println!("initializing");
///     92
/// });
/// println!("ready");
/// println!("{}", *lazy);
/// println!("{}", *lazy);
///
/// // 打印结果:
/// //   ready
/// //   initializing
/// //   92
/// //   92
/// ```
#[stable(feature = "lazy_cell", since = "1.80.0")]
pub struct LazyCell<T, F = fn() -> T> {
    state: UnsafeCell<State<T, F>>,
}

impl<T, F: FnOnce() -> T> LazyCell<T, F> {
    /// 用给定的初始化函数创建一个新的惰性值。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::cell::LazyCell;
    ///
    /// let hello = "Hello, World!".to_string();
    ///
    /// let lazy = LazyCell::new(|| hello.to_uppercase());
    ///
    /// assert_eq!(&*lazy, "HELLO, WORLD!");
    /// ```
    #[inline]
    #[stable(feature = "lazy_cell", since = "1.80.0")]
    #[rustc_const_stable(feature = "lazy_cell", since = "1.80.0")]
    pub const fn new(f: F) -> LazyCell<T, F> {
        LazyCell { state: UnsafeCell::new(State::Uninit(f)) }
    }

    /// 消耗该 `LazyCell`,返回其所存储的值。
    ///
    /// 如果 `Lazy` 已初始化,则返回 `Ok(value)`;否则返回 `Err(f)`。
    ///
    /// # Panics
    ///
    /// 如果该 cell 已被毒化,则 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(lazy_cell_into_inner)]
    ///
    /// use std::cell::LazyCell;
    ///
    /// let hello = "Hello, World!".to_string();
    ///
    /// let lazy = LazyCell::new(|| hello.to_uppercase());
    ///
    /// assert_eq!(&*lazy, "HELLO, WORLD!");
    /// assert_eq!(LazyCell::into_inner(lazy).ok(), Some("HELLO, WORLD!".to_string()));
    /// ```
    #[unstable(feature = "lazy_cell_into_inner", issue = "125623")]
    #[rustc_const_unstable(feature = "lazy_cell_into_inner", issue = "125623")]
    pub const fn into_inner(this: Self) -> Result<T, F> {
        match this.state.into_inner() {
            State::Init(data) => Ok(data),
            State::Uninit(f) => Err(f),
            State::Poisoned => panic_poisoned(),
        }
    }

    /// 强制求值这个惰性值,并返回一个指向求值结果的引用。
    ///
    /// 它等价于 `Deref` 的实现,只不过是显式的。
    ///
    /// # Panics
    ///
    /// 如果初始化闭包(即传给 [`new()`] 方法的那个)发生 panic,该 panic 会被传播给调用者,
    /// 而该 cell 会变为已毒化状态。这将导致此后对该 cell 的所有访问(经由 [`force()`] 或解引用)
    /// 都 panic。
    ///
    /// [`new()`]: LazyCell::new
    /// [`force()`]: LazyCell::force
    ///
    /// # 示例
    ///
    /// ```
    /// use std::cell::LazyCell;
    ///
    /// let lazy = LazyCell::new(|| 92);
    ///
    /// assert_eq!(LazyCell::force(&lazy), &92);
    /// assert_eq!(&*lazy, &92);
    /// ```
    #[inline]
    #[stable(feature = "lazy_cell", since = "1.80.0")]
    #[rustc_should_not_be_called_on_const_items]
    pub fn force(this: &LazyCell<T, F>) -> &T {
        // SAFETY:
        // 这会使任何指向该数据的可变引用失效。所得到的引用要么一直存活到对 `this` 的借用结束
        // (在已初始化的情形下),要么会在 `really_init` 中被失效(在未初始化的情形下;
        // `really_init` 会创建并返回一个全新的引用)。
        let state = unsafe { &*this.state.get() };
        match state {
            State::Init(data) => data,
            // SAFETY:此时状态是未初始化的。
            State::Uninit(_) => unsafe { LazyCell::really_init(this) },
            State::Poisoned => panic_poisoned(),
        }
    }

    /// 强制求值这个惰性值,并返回一个指向求值结果的可变引用。
    ///
    /// # Panics
    ///
    /// 如果初始化闭包(即传给 [`new()`] 方法的那个)发生 panic,该 panic 会被传播给调用者,
    /// 而该 cell 会变为已毒化状态。这将导致此后对该 cell 的所有访问(经由 [`force()`] 或解引用)
    /// 都 panic。
    ///
    /// [`new()`]: LazyCell::new
    /// [`force()`]: LazyCell::force
    ///
    /// # 示例
    ///
    /// ```
    /// use std::cell::LazyCell;
    ///
    /// let mut lazy = LazyCell::new(|| 92);
    ///
    /// let p = LazyCell::force_mut(&mut lazy);
    /// assert_eq!(*p, 92);
    /// *p = 44;
    /// assert_eq!(*lazy, 44);
    /// ```
    #[inline]
    #[stable(feature = "lazy_get", since = "1.94.0")]
    pub fn force_mut(this: &mut LazyCell<T, F>) -> &mut T {
        #[cold]
        /// # 安全性(Safety）
        /// 只能在状态为 `Uninit` 时调用。
        unsafe fn really_init_mut<T, F: FnOnce() -> T>(state: &mut State<T, F>) -> &mut T {
            // 不变量:始终有效,但其中的值可能不会被 drop。
            struct PoisonOnPanic<T, F>(*mut State<T, F>);
            impl<T, F> Drop for PoisonOnPanic<T, F> {
                #[inline]
                fn drop(&mut self) {
                    // SAFETY:不变量声明它是有效的,而且我们不会 drop 掉旧值。
                    unsafe {
                        self.0.write(State::Poisoned);
                    }
                }
            }

            let State::Uninit(f) = state else {
                // 这里的 `unreachable!()` 不会被优化掉,因为本函数是 cold 的。
                // SAFETY:前置条件。
                unsafe { unreachable_unchecked() };
            };
            // SAFETY:在读取 `f` 之后我们绝不会 drop 该状态,而且无论 panic 还是成功,我们都会
            // 写回一个有效的值。`f` 无法访问该 `LazyCell`,因为它正被可变借用。
            let f = unsafe { core::ptr::read(f) };
            // 不变量:由可变引用发起,不要 drop,因为我们已经把它读取出来了。
            let guard = PoisonOnPanic(state);
            let data = f();
            // SAFETY:依据 `PoisonOnPanic` 的不变量,而且我们不会 drop 掉旧值。
            unsafe {
                core::ptr::write(guard.0, State::Init(data));
            }
            core::mem::forget(guard);
            let State::Init(data) = state else { unreachable!() };
            data
        }

        let state = this.state.get_mut();
        match state {
            State::Init(data) => data,
            // SAFETY:`state` 是 `Uninit`。
            State::Uninit(_) => unsafe { really_init_mut(state) },
            State::Poisoned => panic_poisoned(),
        }
    }

    /// # 安全性(Safety）
    /// 只能在状态为 `Uninit` 时调用。
    #[cold]
    unsafe fn really_init(this: &LazyCell<T, F>) -> &T {
        // SAFETY:
        // 本函数只会在状态为未初始化时被调用,因此除了 `force` 中的那个引用之外,不存在任何
        // 指向 `state` 的引用;而那个引用在此处会被失效,且之后不会再被访问。
        let state = unsafe { &mut *this.state.get() };
        // 暂时把状态标记为已毒化。这既能阻止重入式访问,又能在闭包 panic 时正确地把该 cell 毒化。
        let State::Uninit(f) = mem::replace(state, State::Poisoned) else { unreachable!() };

        let data = f();

        // SAFETY:
        // 如果闭包通过类似可重入互斥锁的东西访问了该 cell,但捕获了因状态被毒化而产生的 panic,
        // 那么对 `state` 的可变借用就会被失效,所以这里我们需要改走 `UnsafeCell` 指针。此时状态
        // 只可能是已毒化的,因此用 `write` 来跳过 `State` 的析构函数应当有助于优化器。
        unsafe { this.state.get().write(State::Init(data)) };

        // SAFETY:
        // 之前那些引用已被上面的 `write` 调用失效,所以这里改为对该状态做一次新的共享借用。
        let state = unsafe { &*this.state.get() };
        let State::Init(data) = state else { unreachable!() };
        data
    }
}

impl<T, F> LazyCell<T, F> {
    /// 如果已初始化,则返回一个指向该值的可变引用。否则(未初始化或已毒化)返回 `None`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::cell::LazyCell;
    ///
    /// let mut lazy = LazyCell::new(|| 92);
    ///
    /// assert_eq!(LazyCell::get_mut(&mut lazy), None);
    /// let _ = LazyCell::force(&lazy);
    /// *LazyCell::get_mut(&mut lazy).unwrap() = 44;
    /// assert_eq!(*lazy, 44);
    /// ```
    #[inline]
    #[stable(feature = "lazy_get", since = "1.94.0")]
    pub fn get_mut(this: &mut LazyCell<T, F>) -> Option<&mut T> {
        let state = this.state.get_mut();
        match state {
            State::Init(data) => Some(data),
            _ => None,
        }
    }

    /// 如果已初始化,则返回一个指向该值的引用。否则(未初始化或已毒化)返回 `None`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::cell::LazyCell;
    ///
    /// let lazy = LazyCell::new(|| 92);
    ///
    /// assert_eq!(LazyCell::get(&lazy), None);
    /// let _ = LazyCell::force(&lazy);
    /// assert_eq!(LazyCell::get(&lazy), Some(&92));
    /// ```
    #[inline]
    #[stable(feature = "lazy_get", since = "1.94.0")]
    pub fn get(this: &LazyCell<T, F>) -> Option<&T> {
        // SAFETY:
        // 此处之所以健全,与 `force` 中的理由相同:状态一旦初始化,就不会再被可变地访问,所以
        // 这个引用在对 `self` 的整个借用期间都会保持有效。
        let state = unsafe { &*this.state.get() };
        match state {
            State::Init(data) => Some(data),
            _ => None,
        }
    }
}

#[stable(feature = "lazy_cell", since = "1.80.0")]
impl<T, F: FnOnce() -> T> Deref for LazyCell<T, F> {
    type Target = T;

    /// # Panics
    ///
    /// 如果初始化闭包(即传给 [`new()`] 方法的那个)发生 panic,该 panic 会被传播给调用者,
    /// 而该 cell 会变为已毒化状态。这将导致此后对该 cell 的所有访问(经由 [`force()`] 或解引用)
    /// 都 panic。
    ///
    /// [`new()`]: LazyCell::new
    /// [`force()`]: LazyCell::force
    #[inline]
    fn deref(&self) -> &T {
        LazyCell::force(self)
    }
}

#[stable(feature = "lazy_deref_mut", since = "1.89.0")]
impl<T, F: FnOnce() -> T> DerefMut for LazyCell<T, F> {
    /// # Panics
    ///
    /// 如果初始化闭包(即传给 [`new()`] 方法的那个)发生 panic,该 panic 会被传播给调用者,
    /// 而该 cell 会变为已毒化状态。这将导致此后对该 cell 的所有访问(经由 [`force()`] 或解引用)
    /// 都 panic。
    ///
    /// [`new()`]: LazyCell::new
    /// [`force()`]: LazyCell::force
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        LazyCell::force_mut(self)
    }
}

#[stable(feature = "lazy_cell", since = "1.80.0")]
impl<T: Default> Default for LazyCell<T> {
    /// 用 `Default` 作为初始化函数,创建一个新的惰性值。
    #[inline]
    fn default() -> LazyCell<T> {
        LazyCell::new(T::default)
    }
}

#[stable(feature = "lazy_cell", since = "1.80.0")]
impl<T: fmt::Debug, F> fmt::Debug for LazyCell<T, F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut d = f.debug_tuple("LazyCell");
        match LazyCell::get(self) {
            Some(data) => d.field(data),
            None => d.field(&format_args!("<uninit>")),
        };
        d.finish()
    }
}

#[cold]
#[inline(never)]
const fn panic_poisoned() -> ! {
    panic!("LazyCell instance has previously been poisoned")
}
