use super::once::OnceExclusiveState;
use crate::cell::UnsafeCell;
use crate::fmt;
use crate::marker::PhantomData;
use crate::mem::MaybeUninit;
use crate::panic::{RefUnwindSafe, UnwindSafe};
use crate::sync::Once;

/// 一种名义上只能被写入一次的同步原语。
///
/// 该类型是线程安全版本的 [`OnceCell`]，可用于静态项（statics）。
/// 在许多简单场景下，你可以改用 [`LazyLock<T, F>`]，以更少的力气获得本类型的
/// 好处：`LazyLock<T, F>` 在解引用时用 `F` 初始化，因而「看起来就像」`&T`！
/// OnceLock 的过人之处在于：当 LazyLock 过于简单、无法支持某个场景时——因为
/// 一旦你调用了 [`LazyLock::new(|| ...)`]，LazyLock 就不允许再向它的函数提供
/// 额外输入。
///
/// 可以把 `OnceLock` 看作对「未初始化数据、一经写入即变为已初始化」的一种
/// 安全抽象。
///
/// 与 [`Mutex`](crate::sync::Mutex) 不同，`OnceLock` 在 panic 时绝不会中毒。
///
/// [`OnceCell`]: crate::cell::OnceCell
/// [`LazyLock<T, F>`]: crate::sync::LazyLock
/// [`LazyLock::new(|| ...)`]: crate::sync::LazyLock::new
///
/// # 示例
///
/// 从另一个线程写入 `OnceLock`：
///
/// ```
/// use std::sync::OnceLock;
///
/// static CELL: OnceLock<usize> = OnceLock::new();
///
/// // `OnceLock` 还没有被写入过。
/// assert!(CELL.get().is_none());
///
/// // 派生一个线程并写入 `OnceLock`。
/// std::thread::spawn(|| {
///     let value = CELL.get_or_init(|| 12345);
///     assert_eq!(value, &12345);
/// })
/// .join()
/// .unwrap();
///
/// // `OnceLock` 现在含有该值。
/// assert_eq!(
///     CELL.get(),
///     Some(&12345),
/// );
/// ```
///
/// 你可以用 `OnceLock` 实现一种需要「只追加」（append-only）逻辑的类型：
///
/// ```
/// use std::sync::{OnceLock, atomic::{AtomicU32, Ordering}};
/// use std::thread;
///
/// struct OnceList<T> {
///     data: OnceLock<T>,
///     next: OnceLock<Box<OnceList<T>>>,
/// }
/// impl<T> OnceList<T> {
///     const fn new() -> OnceList<T> {
///         OnceList { data: OnceLock::new(), next: OnceLock::new() }
///     }
///     fn push(&self, value: T) {
///         // FIXME: 这个实现很简洁，但对长列表或多线程而言也很慢。
///         // 作为练习，请思考如何在保持其行为的前提下加以改进
///         if let Err(value) = self.data.set(value) {
///             let next = self.next.get_or_init(|| Box::new(OnceList::new()));
///             next.push(value)
///         };
///     }
///     fn contains(&self, example: &T) -> bool
///     where
///         T: PartialEq,
///     {
///         self.data.get().map(|item| item == example).filter(|v| *v).unwrap_or_else(|| {
///             self.next.get().map(|next| next.contains(example)).unwrap_or(false)
///         })
///     }
/// }
///
/// // 让我们通过做点小小的计数来检验这个新的、Sync 的只追加列表
/// static LIST: OnceList<u32> = OnceList::new();
/// static COUNTER: AtomicU32 = AtomicU32::new(0);
///
/// # const LEN: u32 = if cfg!(miri) { 50 } else { 1000 };
/// # /*
/// const LEN: u32 = 1000;
/// # */
/// thread::scope(|s| {
///     for _ in 0..thread::available_parallelism().unwrap().get() {
///         s.spawn(|| {
///             while let i @ 0..LEN = COUNTER.fetch_add(1, Ordering::Relaxed) {
///                 LIST.push(i);
///             }
///         });
///     }
/// });
///
/// for i in 0..LEN {
///     assert!(LIST.contains(&i));
/// }
///
/// ```
#[stable(feature = "once_cell", since = "1.70.0")]
pub struct OnceLock<T> {
    // FIXME(nonpoison_once): 一旦不中毒版本可用，就切换到该版本
    once: Once,
    // 值是否已初始化，由 `once.is_completed()` 来追踪。
    value: UnsafeCell<MaybeUninit<T>>,
    /// 用 `PhantomData` 确保 dropck（drop 检查器）明白：我们在自己的 Drop
    /// 实现里会 drop 一个 T。
    ///
    /// ```compile_fail,E0597
    /// use std::sync::OnceLock;
    ///
    /// struct A<'a>(&'a str);
    ///
    /// impl<'a> Drop for A<'a> {
    ///     fn drop(&mut self) {}
    /// }
    ///
    /// let cell = OnceLock::new();
    /// {
    ///     let s = String::new();
    ///     let _ = cell.set(A(&s));
    /// }
    /// ```
    _marker: PhantomData<T>,
}

impl<T> OnceLock<T> {
    /// 创建一个新的、未初始化的单元（cell）。
    #[inline]
    #[must_use]
    #[stable(feature = "once_cell", since = "1.70.0")]
    #[rustc_const_stable(feature = "once_cell", since = "1.70.0")]
    pub const fn new() -> OnceLock<T> {
        OnceLock {
            once: Once::new(),
            value: UnsafeCell::new(MaybeUninit::uninit()),
            _marker: PhantomData,
        }
    }

    /// 获取底层值的引用。
    ///
    /// 如果该单元未初始化、或正在初始化中，则返回 `None`。本方法绝不阻塞。
    #[inline]
    #[stable(feature = "once_cell", since = "1.70.0")]
    #[rustc_should_not_be_called_on_const_items]
    pub fn get(&self) -> Option<&T> {
        if self.initialized() {
            // 安全：因为已检查过处于已初始化状态
            Some(unsafe { self.get_unchecked() })
        } else {
            None
        }
    }

    /// 获取底层值的可变引用。
    ///
    /// 如果该单元未初始化，则返回 `None`。
    ///
    /// 本方法绝不阻塞。由于它以可变方式借用 `OnceLock`，在静态层面即可保证
    /// 不存在对该 `OnceLock` 的任何活跃借用（包括来自其他线程的借用）。
    #[inline]
    #[stable(feature = "once_cell", since = "1.70.0")]
    pub fn get_mut(&mut self) -> Option<&mut T> {
        if self.initialized_mut() {
            // 安全：因为已检查过处于已初始化状态，且我们拥有独占访问权
            Some(unsafe { self.get_unchecked_mut() })
        } else {
            None
        }
    }

    /// 阻塞当前线程，直到该单元被初始化为止。
    ///
    /// # 示例
    ///
    /// 等待另一线程上的某项计算完成：
    /// ```rust
    /// use std::thread;
    /// use std::sync::OnceLock;
    ///
    /// let value = OnceLock::new();
    ///
    /// thread::scope(|s| {
    ///     s.spawn(|| value.set(1 + 1));
    ///
    ///     let result = value.wait();
    ///     assert_eq!(result, &2);
    /// })
    /// ```
    #[inline]
    #[stable(feature = "once_wait", since = "1.86.0")]
    #[rustc_should_not_be_called_on_const_items]
    pub fn wait(&self) -> &T {
        self.once.wait_force();

        unsafe { self.get_unchecked() }
    }

    /// 把该单元的内容初始化为 `value`。
    ///
    /// 如果另一个线程当前正尝试初始化该单元，本方法可能阻塞。当 `set` 返回时，
    /// 保证该单元含有一个值，尽管不一定是这里提供的那个。
    ///
    /// 如果该单元此前未初始化，返回 `Ok(())`；如果它已经初始化过，
    /// 返回 `Err(value)`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::sync::OnceLock;
    ///
    /// static CELL: OnceLock<i32> = OnceLock::new();
    ///
    /// fn main() {
    ///     assert!(CELL.get().is_none());
    ///
    ///     std::thread::spawn(|| {
    ///         assert_eq!(CELL.set(92), Ok(()));
    ///     }).join().unwrap();
    ///
    ///     assert_eq!(CELL.set(62), Err(62));
    ///     assert_eq!(CELL.get(), Some(&92));
    /// }
    /// ```
    #[inline]
    #[stable(feature = "once_cell", since = "1.70.0")]
    #[rustc_should_not_be_called_on_const_items]
    pub fn set(&self, value: T) -> Result<(), T> {
        match self.try_insert(value) {
            Ok(_) => Ok(()),
            Err((_, value)) => Err(value),
        }
    }

    /// 如果该单元此前未初始化，则把它的内容初始化为 `value`，然后返回其引用。
    ///
    /// 如果另一个线程当前正尝试初始化该单元，本方法可能阻塞。当 `try_insert`
    /// 返回时，保证该单元含有一个值，尽管不一定是这里提供的那个。
    ///
    /// 如果该单元此前未初始化，返回 `Ok(&value)`；如果它已经初始化过，
    /// 返回 `Err((&current_value, value))`。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(once_cell_try_insert)]
    ///
    /// use std::sync::OnceLock;
    ///
    /// static CELL: OnceLock<i32> = OnceLock::new();
    ///
    /// fn main() {
    ///     assert!(CELL.get().is_none());
    ///
    ///     std::thread::spawn(|| {
    ///         assert_eq!(CELL.try_insert(92), Ok(&92));
    ///     }).join().unwrap();
    ///
    ///     assert_eq!(CELL.try_insert(62), Err((&92, 62)));
    ///     assert_eq!(CELL.get(), Some(&92));
    /// }
    /// ```
    #[inline]
    #[unstable(feature = "once_cell_try_insert", issue = "116693")]
    #[rustc_should_not_be_called_on_const_items]
    pub fn try_insert(&self, value: T) -> Result<&T, (&T, T)> {
        // 把 value 暂存在 `Option` 中：若初始化闭包被实际执行，`take` 会取走它；
        // 否则（已被别处初始化）`value` 仍在，据此区分 Ok/Err 两种返回。
        let mut value = Some(value);
        let res = self.get_or_init(|| value.take().unwrap());
        match value {
            None => Ok(res),
            Some(value) => Err((res, value)),
        }
    }

    /// 获取该单元的内容；如果此前未初始化，则把它初始化为 `f()`。
    ///
    /// 多个线程可以带着各自不同的初始化函数并发调用 `get_or_init`，但保证：
    /// 只要函数不 panic，就只有一个函数会被执行。
    ///
    /// # Panics
    ///
    /// 如果 `f()` panic，则该 panic 会传播给调用方，且该单元保持未初始化。
    ///
    /// 从 `f` 中重入式地（reentrantly）初始化该单元是错误的。确切结果未作规定。
    /// 当前实现会死锁，但将来这可能改为 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::sync::OnceLock;
    ///
    /// let cell = OnceLock::new();
    /// let value = cell.get_or_init(|| 92);
    /// assert_eq!(value, &92);
    /// let value = cell.get_or_init(|| unreachable!());
    /// assert_eq!(value, &92);
    /// ```
    #[inline]
    #[stable(feature = "once_cell", since = "1.70.0")]
    #[rustc_should_not_be_called_on_const_items]
    pub fn get_or_init<F>(&self, f: F) -> &T
    where
        F: FnOnce() -> T,
    {
        // 借道不会失败的 `get_or_try_init`（错误类型为 `!`）来复用其实现。
        match self.get_or_try_init(|| Ok::<T, !>(f())) {
            Ok(val) => val,
        }
    }

    /// 获取该单元内容的可变引用；如果此前未初始化，则把它初始化为 `f()`。
    ///
    /// 本方法绝不阻塞。由于它以可变方式借用 `OnceLock`，在静态层面即可保证
    /// 不存在对该 `OnceLock` 的任何活跃借用（包括来自其他线程的借用）。
    ///
    /// # Panics
    ///
    /// 如果 `f()` panic，则该 panic 会传播给调用方，且该单元保持未初始化。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(once_cell_get_mut)]
    ///
    /// use std::sync::OnceLock;
    ///
    /// let mut cell = OnceLock::new();
    /// let value = cell.get_mut_or_init(|| 92);
    /// assert_eq!(*value, 92);
    ///
    /// *value += 2;
    /// assert_eq!(*value, 94);
    ///
    /// let value = cell.get_mut_or_init(|| unreachable!());
    /// assert_eq!(*value, 94);
    /// ```
    #[inline]
    #[unstable(feature = "once_cell_get_mut", issue = "121641")]
    pub fn get_mut_or_init<F>(&mut self, f: F) -> &mut T
    where
        F: FnOnce() -> T,
    {
        match self.get_mut_or_try_init(|| Ok::<T, !>(f())) {
            Ok(val) => val,
        }
    }

    /// 获取该单元的内容；如果此前未初始化，则把它初始化为 `f()`。如果该单元
    /// 此前未初始化且 `f()` 失败，则返回一个错误。
    ///
    /// # Panics
    ///
    /// 如果 `f()` panic，则该 panic 会传播给调用方，且该单元保持未初始化。
    ///
    /// 从 `f` 中重入式地初始化该单元是错误的。确切结果未作规定。当前实现会
    /// 死锁，但将来这可能改为 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(once_cell_try)]
    ///
    /// use std::sync::OnceLock;
    ///
    /// let cell = OnceLock::new();
    /// assert_eq!(cell.get_or_try_init(|| Err(())), Err(()));
    /// assert!(cell.get().is_none());
    /// let value = cell.get_or_try_init(|| -> Result<i32, ()> {
    ///     Ok(92)
    /// });
    /// assert_eq!(value, Ok(&92));
    /// assert_eq!(cell.get(), Some(&92))
    /// ```
    #[inline]
    #[unstable(feature = "once_cell_try", issue = "109737")]
    #[rustc_should_not_be_called_on_const_items]
    pub fn get_or_try_init<F, E>(&self, f: F) -> Result<&T, E>
    where
        F: FnOnce() -> Result<T, E>,
    {
        // 快速路径检查
        // 注意：本方法中我们需要对状态执行一次 acquire，以正确地与
        // `LazyLock::force` 同步。这目前是通过调用 `self.get()` 来完成的——
        // 它进而调用 `self.initialized()`，后者再执行 acquire。
        if let Some(value) = self.get() {
            return Ok(value);
        }
        self.initialize(f)?;

        // SAFETY: 内部的值已被初始化
        Ok(unsafe { self.get_unchecked() })
    }

    /// 获取该单元内容的可变引用；如果此前未初始化，则把它初始化为 `f()`。
    /// 如果该单元此前未初始化且 `f()` 失败，则返回一个错误。
    ///
    /// 本方法绝不阻塞。由于它以可变方式借用 `OnceLock`，在静态层面即可保证
    /// 不存在对该 `OnceLock` 的任何活跃借用（包括来自其他线程的借用）。
    ///
    /// # Panics
    ///
    /// 如果 `f()` panic，则该 panic 会传播给调用方，且该单元保持未初始化。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(once_cell_get_mut)]
    ///
    /// use std::sync::OnceLock;
    ///
    /// let mut cell: OnceLock<u32> = OnceLock::new();
    ///
    /// // 初始化该单元的失败尝试不会改变它的内容
    /// assert!(cell.get_mut_or_try_init(|| "not a number!".parse()).is_err());
    /// assert!(cell.get().is_none());
    ///
    /// let value = cell.get_mut_or_try_init(|| "1234".parse());
    /// assert_eq!(value, Ok(&mut 1234));
    /// *value.unwrap() += 2;
    /// assert_eq!(cell.get(), Some(&1236))
    /// ```
    #[inline]
    #[unstable(feature = "once_cell_get_mut", issue = "121641")]
    pub fn get_mut_or_try_init<F, E>(&mut self, f: F) -> Result<&mut T, E>
    where
        F: FnOnce() -> Result<T, E>,
    {
        if self.get_mut().is_none() {
            self.initialize(f)?;
        }

        // SAFETY: 内部的值已被初始化
        Ok(unsafe { self.get_unchecked_mut() })
    }

    /// 消耗该 `OnceLock`，返回被包裹的值。如果该单元未初始化，返回 `None`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::sync::OnceLock;
    ///
    /// let cell: OnceLock<String> = OnceLock::new();
    /// assert_eq!(cell.into_inner(), None);
    ///
    /// let cell = OnceLock::new();
    /// cell.set("hello".to_string()).unwrap();
    /// assert_eq!(cell.into_inner(), Some("hello".to_string()));
    /// ```
    #[inline]
    #[stable(feature = "once_cell", since = "1.70.0")]
    pub fn into_inner(mut self) -> Option<T> {
        self.take()
    }

    /// 把值从这个 `OnceLock` 中取出，使其退回未初始化状态。
    ///
    /// 如果该 `OnceLock` 未初始化，则无任何效果并返回 `None`。
    ///
    /// 由于本方法以可变方式借用 `OnceLock`，在静态层面即可保证不存在对该
    /// `OnceLock` 的任何活跃借用（包括来自其他线程的借用）。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::sync::OnceLock;
    ///
    /// let mut cell: OnceLock<String> = OnceLock::new();
    /// assert_eq!(cell.take(), None);
    ///
    /// let mut cell = OnceLock::new();
    /// cell.set("hello".to_string()).unwrap();
    /// assert_eq!(cell.take(), Some("hello".to_string()));
    /// assert_eq!(cell.get(), None);
    /// ```
    #[inline]
    #[stable(feature = "once_cell", since = "1.70.0")]
    pub fn take(&mut self) -> Option<T> {
        if self.initialized_mut() {
            self.once = Once::new();
            // SAFETY: `self.value` 已初始化，含有一个有效的 `T`。
            // `self.once` 被重置，因此 `initialized()` 将再次为 false，
            // 这可防止该值被读取两次。
            unsafe { Some(self.value.get_mut().assume_init_read()) }
        } else {
            None
        }
    }

    #[inline]
    fn initialized(&self) -> bool {
        self.once.is_completed()
    }

    #[inline]
    fn initialized_mut(&mut self) -> bool {
        // `state()` 不执行原子加载（atomic load），因此优先用它而非 `is_complete()`。
        let state = self.once.state();
        match state {
            OnceExclusiveState::Complete => true,
            _ => false,
        }
    }

    #[cold]
    #[optimize(size)]
    fn initialize<F, E>(&self, f: F) -> Result<(), E>
    where
        F: FnOnce() -> Result<T, E>,
    {
        let mut res: Result<(), E> = Ok(());
        let slot = &self.value;

        // 忽略来自其他线程的中毒
        // 即便另一个线程发生了 panic，我们也仍能运行自己的闭包
        self.once.call_once_force(|p| {
            match f() {
                Ok(value) => {
                    unsafe { (&mut *slot.get()).write(value) };
                }
                Err(e) => {
                    res = Err(e);

                    // 既然我们未能初始化自己的值，就把底层的 `Once` 视为中毒。
                    p.poison();
                }
            }
        });
        res
    }

    /// # Safety
    ///
    /// 该单元必须已初始化
    #[inline]
    unsafe fn get_unchecked(&self) -> &T {
        debug_assert!(self.initialized());
        unsafe { (&*self.value.get()).assume_init_ref() }
    }

    /// # Safety
    ///
    /// 该单元必须已初始化
    #[inline]
    unsafe fn get_unchecked_mut(&mut self) -> &mut T {
        debug_assert!(self.initialized_mut());
        unsafe { self.value.get_mut().assume_init_mut() }
    }
}

// 为什么我们需要 `T: Send`？
// 线程 A 创建一个 `OnceLock` 并与作用域线程（scoped thread）B 共享，
// B 填入该单元，随后该单元被 A 销毁。也就是说，析构函数观测到了一个被
// 发送过来的值。
#[stable(feature = "once_cell", since = "1.70.0")]
unsafe impl<T: Sync + Send> Sync for OnceLock<T> {}
#[stable(feature = "once_cell", since = "1.70.0")]
unsafe impl<T: Send> Send for OnceLock<T> {}

#[stable(feature = "once_cell", since = "1.70.0")]
impl<T: RefUnwindSafe + UnwindSafe> RefUnwindSafe for OnceLock<T> {}
#[stable(feature = "once_cell", since = "1.70.0")]
impl<T: UnwindSafe> UnwindSafe for OnceLock<T> {}

#[stable(feature = "once_cell", since = "1.70.0")]
#[rustc_const_unstable(feature = "const_default", issue = "143894")]
impl<T> const Default for OnceLock<T> {
    /// 创建一个新的、未初始化的单元。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::sync::OnceLock;
    ///
    /// fn main() {
    ///     assert_eq!(OnceLock::<()>::new(), OnceLock::default());
    /// }
    /// ```
    #[inline]
    fn default() -> OnceLock<T> {
        OnceLock::new()
    }
}

#[stable(feature = "once_cell", since = "1.70.0")]
impl<T: fmt::Debug> fmt::Debug for OnceLock<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut d = f.debug_tuple("OnceLock");
        match self.get() {
            Some(v) => d.field(v),
            None => d.field(&format_args!("<uninit>")),
        };
        d.finish()
    }
}

#[stable(feature = "once_cell", since = "1.70.0")]
impl<T: Clone> Clone for OnceLock<T> {
    #[inline]
    fn clone(&self) -> OnceLock<T> {
        let cell = Self::new();
        if let Some(value) = self.get() {
            match cell.set(value.clone()) {
                Ok(()) => (),
                Err(_) => unreachable!(),
            }
        }
        cell
    }
}

#[stable(feature = "once_cell", since = "1.70.0")]
impl<T> From<T> for OnceLock<T> {
    /// 创建一个新单元，其内容设为 `value`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::sync::OnceLock;
    ///
    /// # fn main() -> Result<(), i32> {
    /// let a = OnceLock::from(3);
    /// let b = OnceLock::new();
    /// b.set(3)?;
    /// assert_eq!(a, b);
    /// Ok(())
    /// # }
    /// ```
    #[inline]
    fn from(value: T) -> Self {
        let cell = Self::new();
        match cell.set(value) {
            Ok(()) => cell,
            Err(_) => unreachable!(),
        }
    }
}

#[stable(feature = "once_cell", since = "1.70.0")]
impl<T: PartialEq> PartialEq for OnceLock<T> {
    /// 两个 `OnceLock` 之间的相等性。
    ///
    /// 两个 `OnceLock` 相等的条件是：它们要么都含有值且两值相等，要么都不含值。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::sync::OnceLock;
    ///
    /// let five = OnceLock::new();
    /// five.set(5).unwrap();
    ///
    /// let also_five = OnceLock::new();
    /// also_five.set(5).unwrap();
    ///
    /// assert!(five == also_five);
    ///
    /// assert!(OnceLock::<u32>::new() == OnceLock::<u32>::new());
    /// ```
    #[inline]
    fn eq(&self, other: &OnceLock<T>) -> bool {
        self.get() == other.get()
    }
}

#[stable(feature = "once_cell", since = "1.70.0")]
impl<T: Eq> Eq for OnceLock<T> {}

#[stable(feature = "once_cell", since = "1.70.0")]
unsafe impl<#[may_dangle] T> Drop for OnceLock<T> {
    #[inline]
    fn drop(&mut self) {
        if self.initialized_mut() {
            // SAFETY: 该单元已初始化且正在被 drop，因此它不会再被访问。除了
            // drop 之外我们也不触碰那个 `T`，这印证了我们对 #[may_dangle] 的
            // 使用是正确的。
            unsafe { self.value.get_mut().assume_init_drop() };
        }
    }
}
