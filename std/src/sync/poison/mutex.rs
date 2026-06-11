use crate::cell::UnsafeCell;
use crate::fmt;
use crate::marker::PhantomData;
use crate::mem::{self, ManuallyDrop};
use crate::ops::{Deref, DerefMut};
use crate::ptr::NonNull;
use crate::sync::{LockResult, PoisonError, TryLockError, TryLockResult, poison};
use crate::sys::sync as sys;

/// 一种互斥（mutual exclusion）原语，用于保护共享数据
///
/// 这个互斥锁会阻塞那些等待锁变为可用的线程。互斥锁可通过 [`new`] 构造函数
/// 创建。每个互斥锁都有一个类型参数，代表它所保护的数据。这份数据只能通过
/// 由 [`lock`] 与 [`try_lock`] 返回的 RAII 守卫来访问，从而保证：数据只在
/// 互斥锁被锁定时才可能被访问。
///
/// # 中毒（Poisoning）
///
/// 本模块中的互斥锁实现了一种名为「中毒」的策略：当一个互斥锁察觉到持有它的
/// 线程发生了 panic 时，它就会变成「中毒」状态。
///
/// 一旦某个互斥锁中毒，默认情况下所有其他线程都无法访问其数据，因为这份数据
/// 很可能已被污染（某些不变式 invariant 未被维持）。对互斥锁而言，这意味着
/// [`lock`] 与 [`try_lock`] 方法会返回一个 [`Result`]，用以表明该互斥锁是否
/// 已中毒。互斥锁的大多数用法会直接对这些结果 [`unwrap()`]，把 panic 在线程
/// 间传播，以确保不会有人观测到可能失效的不变式。
///
/// 中毒只是一种建议性（advisory）机制：[`PoisonError`] 类型有一个
/// [`into_inner`] 方法，它会返回那个本来会在加锁成功时返回的守卫。这就允许
/// 你在锁已中毒的情况下仍能访问数据。
///
/// 此外，对 panic 的检测并不完美，因此即便是未中毒的互斥锁也需谨慎对待，
/// 因为某些 panic 可能被漏掉了。下面是一份可能发生此类情况的（非详尽）清单：
///
/// - 如果在一次 panic 正在进行期间锁定某互斥锁，例如在某个 [`Drop`] 实现
///   或某个 [panic hook] 内部，那么在持锁期间第二次 panic 将使该互斥锁保持
///   不中毒（unpoisoned）。注意，虽然双重 panic 通常会中止（abort）程序，
///   但 [`catch_unwind`] 可以阻止这一点。
///
/// - 跨越不同的 panic 上下文来加锁和解锁互斥锁，例如在 [`Drop::drop`] 中把
///   守卫存入一个 [`Cell`] 并在外部访问它（或反之），都可能以出人意料的方式
///   影响中毒状态。
///
/// - 外来异常（foreign exceptions）目前即便在没有其他 panic 的情况下也不会
///   触发中毒。
///
/// 尽管这在现实代码中很少发生，但 `unsafe` 代码不能为了健全性（soundness）
/// 而依赖中毒，因为中毒的行为可能取决于外部上下文。下面是一个 **错误** 使用
/// 中毒的例子：
///
/// ```rust
/// use std::sync::Mutex;
///
/// struct MutexBox<T> {
///     data: Mutex<*mut T>,
/// }
///
/// impl<T> MutexBox<T> {
///     pub fn new(value: T) -> Self {
///         Self {
///             data: Mutex::new(Box::into_raw(Box::new(value))),
///         }
///     }
///
///     pub fn replace_with(&self, f: impl FnOnce(T) -> T) {
///         let ptr = self.data.lock().expect("poisoned");
///         // 在 `f` 运行期间，数据被移出 `*ptr`。如果 `f` panic，`*ptr`
///         // 就会一直指向一个已被 drop 的值。其本意是：这会使该互斥锁中毒，
///         // 这样后续对 `replace_with` 的调用就会 panic 而不去读取 `*ptr`。
///         // 但由于在从一个 panic hook 中运行时并不保证会发生中毒，
///         // 这可能导致 use-after-free（释放后使用）。
///         unsafe {
///             (*ptr).write(f((*ptr).read()));
///         }
///     }
/// }
/// ```
///
/// [`new`]: Self::new
/// [`lock`]: Self::lock
/// [`try_lock`]: Self::try_lock
/// [`unwrap()`]: Result::unwrap
/// [`PoisonError`]: super::PoisonError
/// [`into_inner`]: super::PoisonError::into_inner
/// [panic hook]: crate::panic::set_hook
/// [`catch_unwind`]: crate::panic::catch_unwind
/// [`Cell`]: crate::cell::Cell
///
/// # 示例
///
/// ```
/// use std::sync::{Arc, Mutex};
/// use std::thread;
/// use std::sync::mpsc::channel;
///
/// const N: usize = 10;
///
/// // 派生若干线程来（以非原子方式）递增一个共享变量，并在所有递增完成后
/// // 通知主线程。
/// //
/// // 这里我们用一个 Arc 在线程间共享内存，而 Arc 内部的数据则用互斥锁保护。
/// let data = Arc::new(Mutex::new(0));
///
/// let (tx, rx) = channel();
/// for _ in 0..N {
///     let (data, tx) = (Arc::clone(&data), tx.clone());
///     thread::spawn(move || {
///         // 只有在持有锁时才能访问共享状态。
///         // 我们的非原子递增是安全的，因为在持锁期间我们是唯一能访问
///         // 共享状态的线程。
///         //
///         // 我们对返回值 unwrap()，以此断言：我们不预期线程会在持锁期间失败。
///         let mut data = data.lock().unwrap();
///         *data += 1;
///         if *data == N {
///             tx.send(()).unwrap();
///         }
///         // 当 `data` 离开作用域时，锁在此处被解锁。
///     });
/// }
///
/// rx.recv().unwrap();
/// ```
///
/// 从一个已中毒的互斥锁中恢复：
///
/// ```
/// use std::sync::{Arc, Mutex};
/// use std::thread;
///
/// let lock = Arc::new(Mutex::new(0_u32));
/// let lock2 = Arc::clone(&lock);
///
/// let _ = thread::spawn(move || -> () {
///     // 这个线程会率先获取互斥锁，并对 `lock` 的结果 unwrap，因为此时锁
///     // 还未中毒。
///     let _guard = lock2.lock().unwrap();
///
///     // 在持锁期间（`_guard` 仍在作用域内）发生的这次 panic 会使该互斥锁
///     // 中毒。
///     panic!();
/// }).join();
///
/// // 此刻锁已中毒，但可以对返回的结果做模式匹配，在两条分支上都取回底层的
/// // 守卫。
/// let mut guard = match lock.lock() {
///     Ok(guard) => guard,
///     Err(poisoned) => poisoned.into_inner(),
/// };
///
/// *guard += 1;
/// ```
///
/// 若要早于外层作用域结束就解锁某个互斥锁守卫，可创建一个内部作用域，或者
/// 手动 drop 该守卫。
///
/// ```
/// use std::sync::{Arc, Mutex};
/// use std::thread;
///
/// const N: usize = 3;
///
/// let data_mutex = Arc::new(Mutex::new(vec![1, 2, 3, 4]));
/// let res_mutex = Arc::new(Mutex::new(0));
///
/// let mut threads = Vec::with_capacity(N);
/// (0..N).for_each(|_| {
///     let data_mutex_clone = Arc::clone(&data_mutex);
///     let res_mutex_clone = Arc::clone(&res_mutex);
///
///     threads.push(thread::spawn(move || {
///         // 这里我们用一个代码块来限定锁守卫的生命周期。
///         let result = {
///             let mut data = data_mutex_clone.lock().unwrap();
///             // 这是某项重要且耗时较长的工作的结果。
///             let result = data.iter().fold(0, |acc, x| acc + x * 2);
///             data.push(result);
///             result
///             // 互斥锁守卫在此处被 drop，同时被 drop 的还有临界区
///             // （critical section）内创建的任何其他值。
///         };
///         // 这里创建的守卫是一个临时量，在语句结束时即被 drop；也就是说，
///         // 即便该线程还做了一些额外工作，这把锁也不会一直被持有。
///         *res_mutex_clone.lock().unwrap() += result;
///     }));
/// });
///
/// let mut data = data_mutex.lock().unwrap();
/// // 这是某项重要且耗时较长的工作的结果。
/// let result = data.iter().fold(0, |acc, x| acc + x * 2);
/// data.push(result);
/// // 我们显式地 drop 掉 `data`，因为它已不再需要，而本线程仍有工作要做。
/// // 这样其他线程就能立即开始处理这份数据，而无需等待此处其余无关工作完成。
/// //
/// // 这一点在此处比在那些线程里更重要，因为我们随后要 `.join` 这些线程。
/// // 如果我们没有 drop 掉互斥锁守卫，某个线程可能会永远等待它，从而导致死锁。
/// // 和那些线程里一样，这里也可以用一个代码块来替代调用 `drop` 函数。
/// drop(data);
/// // 这里的互斥锁守卫没有被赋给任何变量，因此即便作用域在此行之后并未结束，
/// // 互斥锁仍会被释放：不会有死锁。
/// *res_mutex.lock().unwrap() += result;
///
/// threads.into_iter().for_each(|thread| {
///     thread
///         .join()
///         .expect("The thread creating or execution failed !")
/// });
///
/// assert_eq!(*res_mutex.lock().unwrap(), 800);
/// ```
///
#[stable(feature = "rust1", since = "1.0.0")]
#[cfg_attr(not(test), rustc_diagnostic_item = "Mutex")]
pub struct Mutex<T: ?Sized> {
    inner: sys::Mutex,
    poison: poison::Flag,
    data: UnsafeCell<T>,
}

/// `T` 必须是 `Send`，[`Mutex`] 才能是 `Send`，因为可以通过 [`into_inner`]
/// 从该 `Mutex` 取出被拥有的 `T`。
///
/// [`into_inner`]: Mutex::into_inner
#[stable(feature = "rust1", since = "1.0.0")]
unsafe impl<T: ?Sized + Send> Send for Mutex<T> {}

/// `T` 必须是 `Send`，[`Mutex`] 才能是 `Sync`。
/// 这确保了受保护的数据能从多个线程被安全访问，而不引发数据竞争或其他不安全
/// 行为。
///
/// [`Mutex<T>`] 一次只向一个线程提供对 `T` 的可变访问。然而，`T` 是 `Send`
/// 至关重要，因为以这种方式访问非 `Send` 的结构是不安全的。举例来说，考虑
/// [`Rc`]——一种非原子的引用计数智能指针，它不是 `Send`。借助 `Rc`，我们可以
/// 有多个副本以非原子的引用计数指向同一块堆分配。如果我们使用 `Mutex<Rc<_>>`，
/// 它将只能保护其中一个 `Rc` 实例免于共享访问，而其余副本仍可能遭遇数据竞争。
///
/// 还要注意，`T` 不必是 `Sync`，因为当 `T` 不是 `Sync` 时，`&T` 一次只会被
/// 提供给一个线程。
///
/// [`Rc`]: crate::rc::Rc
#[stable(feature = "rust1", since = "1.0.0")]
unsafe impl<T: ?Sized + Send> Sync for Mutex<T> {}

/// 互斥锁的「作用域锁」（scoped lock）的 RAII 实现。当该结构体被 drop
/// （离开作用域）时，锁会被解锁。
///
/// 受互斥锁保护的数据可通过该守卫的 [`Deref`] 与 [`DerefMut`] 实现来访问。
///
/// 该结构体由 [`Mutex`] 上的 [`lock`] 与 [`try_lock`] 方法创建。
///
/// [`lock`]: Mutex::lock
/// [`try_lock`]: Mutex::try_lock
#[must_use = "if unused the Mutex will immediately unlock"]
#[must_not_suspend = "holding a MutexGuard across suspend \
                      points can cause deadlocks, delays, \
                      and cause Futures to not implement `Send`"]
#[stable(feature = "rust1", since = "1.0.0")]
#[clippy::has_significant_drop]
#[cfg_attr(not(test), rustc_diagnostic_item = "MutexGuard")]
pub struct MutexGuard<'a, T: ?Sized + 'a> {
    lock: &'a Mutex<T>,
    poison: poison::Guard,
}

/// 为最大化平台可移植性，[`MutexGuard`] 不是 `Send`。
///
/// 在使用 POSIX 线程（通常称为 pthreads）的平台上，要求在获取互斥锁的同一
/// 线程上释放它。出于这个原因，[`MutexGuard`] 不可实现 `Send`，以防止它被
/// 从另一个线程 drop。
#[stable(feature = "rust1", since = "1.0.0")]
impl<T: ?Sized> !Send for MutexGuard<'_, T> {}

/// `T` 必须是 `Sync`，[`MutexGuard<T>`] 才能是 `Sync`，因为可以从
/// `&MutexGuard`（经由 `Deref`）得到一个 `&T`。
#[stable(feature = "mutexguard", since = "1.19.0")]
unsafe impl<T: ?Sized + Sync> Sync for MutexGuard<'_, T> {}

/// 由 `MutexGuard::map` 返回的 RAII 互斥守卫，它可以指向受保护数据的某个
/// 子字段。当该结构体被 drop（离开作用域）时，锁会被解锁。
///
/// `MappedMutexGuard` 与 [`MutexGuard`] 的主要区别在于：前者不能与
/// [`Condvar`] 一起使用，因为如果在 `Mutex` 解锁期间，被锁定的对象被另一个
/// 线程修改，那将引入健全性（soundness）问题。
///
/// 受互斥锁保护的数据可通过该守卫的 [`Deref`] 与 [`DerefMut`] 实现来访问。
///
/// 该结构体由 [`MutexGuard`] 上的 [`map`] 与 [`filter_map`] 方法创建。
///
/// [`map`]: MutexGuard::map
/// [`filter_map`]: MutexGuard::filter_map
/// [`Condvar`]: crate::sync::Condvar
#[must_use = "if unused the Mutex will immediately unlock"]
#[must_not_suspend = "holding a MappedMutexGuard across suspend \
                      points can cause deadlocks, delays, \
                      and cause Futures to not implement `Send`"]
#[unstable(feature = "mapped_lock_guards", issue = "117108")]
#[clippy::has_significant_drop]
pub struct MappedMutexGuard<'a, T: ?Sized + 'a> {
    // 注意（NB）：我们用裸指针而非 `&'a mut T`，以避免违反 `noalias`，因为
    // 一个 `MappedMutexGuard` 参数在其整个作用域内并不保持唯一性，只在它被
    // drop 之前保持。`NonNull` 对 `T` 是协变（covariant）的，所以我们在下面
    // 加了一个 `PhantomData<&'a mut T>` 字段以得到对 `T` 正确的型变（不变性
    // invariance）。
    data: NonNull<T>,
    inner: &'a sys::Mutex,
    poison_flag: &'a poison::Flag,
    poison: poison::Guard,
    _variance: PhantomData<&'a mut T>,
}

#[unstable(feature = "mapped_lock_guards", issue = "117108")]
impl<T: ?Sized> !Send for MappedMutexGuard<'_, T> {}
#[unstable(feature = "mapped_lock_guards", issue = "117108")]
unsafe impl<T: ?Sized + Sync> Sync for MappedMutexGuard<'_, T> {}

impl<T> Mutex<T> {
    /// 创建一个新的互斥锁，处于未锁定状态，可随时使用。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::sync::Mutex;
    ///
    /// let mutex = Mutex::new(0);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_locks", since = "1.63.0")]
    #[inline]
    pub const fn new(t: T) -> Mutex<T> {
        Mutex { inner: sys::Mutex::new(), poison: poison::Flag::new(), data: UnsafeCell::new(t) }
    }

    /// 通过克隆（cloning）返回其中所含的值。
    ///
    /// # Errors
    ///
    /// 如果本互斥锁的另一位用户在持锁期间发生了 panic，则本调用会改为返回一个
    /// 错误。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(lock_value_accessors)]
    ///
    /// use std::sync::Mutex;
    ///
    /// let mut mutex = Mutex::new(7);
    ///
    /// assert_eq!(mutex.get_cloned().unwrap(), 7);
    /// ```
    #[unstable(feature = "lock_value_accessors", issue = "133407")]
    pub fn get_cloned(&self) -> Result<T, PoisonError<()>>
    where
        T: Clone,
    {
        match self.lock() {
            Ok(guard) => Ok((*guard).clone()),
            Err(_) => Err(PoisonError::new(())),
        }
    }

    /// 设置其中所含的值。
    ///
    /// # Errors
    ///
    /// 如果本互斥锁的另一位用户在持锁期间发生了 panic，则本调用会改为返回一个
    /// 含有所提供 `value` 的错误。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(lock_value_accessors)]
    ///
    /// use std::sync::Mutex;
    ///
    /// let mut mutex = Mutex::new(7);
    ///
    /// assert_eq!(mutex.get_cloned().unwrap(), 7);
    /// mutex.set(11).unwrap();
    /// assert_eq!(mutex.get_cloned().unwrap(), 11);
    /// ```
    #[unstable(feature = "lock_value_accessors", issue = "133407")]
    #[rustc_should_not_be_called_on_const_items]
    pub fn set(&self, value: T) -> Result<(), PoisonError<T>> {
        if mem::needs_drop::<T>() {
            // 如果所含的值带有非平凡（non-trivial）的析构函数，我们就在锁
            // 已被释放之后再调用该析构函数。
            self.replace(value).map(drop)
        } else {
            match self.lock() {
                Ok(mut guard) => {
                    *guard = value;

                    Ok(())
                }
                Err(_) => Err(PoisonError::new(value)),
            }
        }
    }

    /// 用 `value` 替换其中所含的值，并返回旧的值。
    ///
    /// # Errors
    ///
    /// 如果本互斥锁的另一位用户在持锁期间发生了 panic，则本调用会改为返回一个
    /// 含有所提供 `value` 的错误。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(lock_value_accessors)]
    ///
    /// use std::sync::Mutex;
    ///
    /// let mut mutex = Mutex::new(7);
    ///
    /// assert_eq!(mutex.replace(11).unwrap(), 7);
    /// assert_eq!(mutex.get_cloned().unwrap(), 11);
    /// ```
    #[unstable(feature = "lock_value_accessors", issue = "133407")]
    #[rustc_should_not_be_called_on_const_items]
    pub fn replace(&self, value: T) -> LockResult<T> {
        match self.lock() {
            Ok(mut guard) => Ok(mem::replace(&mut *guard, value)),
            Err(_) => Err(PoisonError::new(value)),
        }
    }
}

impl<T: ?Sized> Mutex<T> {
    /// 获取互斥锁，阻塞当前线程直到能够成功获取为止。
    ///
    /// 本函数会阻塞本地线程，直到能够获取该互斥锁。返回时，该线程是唯一持有
    /// 该锁的线程。返回一个 RAII 守卫，以允许对锁进行作用域化的解锁。当该守卫
    /// 离开作用域时，互斥锁将被解锁。
    ///
    /// 在已经持有锁的线程上再次锁定该互斥锁，其确切行为未作规定。但本函数在
    /// 第二次调用时不会返回（举例来说，它可能 panic 或死锁）。
    ///
    /// # Errors
    ///
    /// 如果本互斥锁的另一位用户在持锁期间发生了 panic，则本调用会在获取到
    /// 互斥锁之后返回一个错误。获取到的互斥锁守卫将被包含在返回的错误中。
    ///
    /// # Panics
    ///
    /// 如果该锁已被当前线程持有，调用本函数时可能 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::sync::{Arc, Mutex};
    /// use std::thread;
    ///
    /// let mutex = Arc::new(Mutex::new(0));
    /// let c_mutex = Arc::clone(&mutex);
    ///
    /// thread::spawn(move || {
    ///     *c_mutex.lock().unwrap() = 10;
    /// }).join().expect("thread::spawn failed");
    /// assert_eq!(*mutex.lock().unwrap(), 10);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_should_not_be_called_on_const_items]
    pub fn lock(&self) -> LockResult<MutexGuard<'_, T>> {
        unsafe {
            self.inner.lock();
            MutexGuard::new(self)
        }
    }

    /// 尝试获取该锁。
    ///
    /// 如果此刻无法获取该锁，则返回 [`Err`]。否则返回一个 RAII 守卫。当该守卫
    /// 被 drop 时，锁将被解锁。
    ///
    /// 本函数不会阻塞。
    ///
    /// # Errors
    ///
    /// 如果本互斥锁的另一位用户在持锁期间发生了 panic，那么——若本来能获取到
    /// 互斥锁——本调用会返回 [`Poisoned`] 错误，且获取到的锁守卫将被包含在返回
    /// 的错误中。
    ///
    /// 如果因互斥锁已被锁定而无法获取它，则本调用会返回 [`WouldBlock`] 错误。
    ///
    /// [`Poisoned`]: TryLockError::Poisoned
    /// [`WouldBlock`]: TryLockError::WouldBlock
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::{Arc, Mutex};
    /// use std::thread;
    ///
    /// let mutex = Arc::new(Mutex::new(0));
    /// let c_mutex = Arc::clone(&mutex);
    ///
    /// thread::spawn(move || {
    ///     let mut lock = c_mutex.try_lock();
    ///     if let Ok(ref mut mutex) = lock {
    ///         **mutex = 10;
    ///     } else {
    ///         println!("try_lock failed");
    ///     }
    /// }).join().expect("thread::spawn failed");
    /// assert_eq!(*mutex.lock().unwrap(), 10);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_should_not_be_called_on_const_items]
    pub fn try_lock(&self) -> TryLockResult<MutexGuard<'_, T>> {
        unsafe {
            if self.inner.try_lock() {
                Ok(MutexGuard::new(self)?)
            } else {
                Err(TryLockError::WouldBlock)
            }
        }
    }

    /// 判定该互斥锁是否已中毒。
    ///
    /// 如果有另一个线程处于活动状态，那么该互斥锁随时仍可能变为中毒。在没有
    /// 额外同步的情况下，你不应为了程序正确性而信任 `false` 这一返回值。
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::{Arc, Mutex};
    /// use std::thread;
    ///
    /// let mutex = Arc::new(Mutex::new(0));
    /// let c_mutex = Arc::clone(&mutex);
    ///
    /// let _ = thread::spawn(move || {
    ///     let _lock = c_mutex.lock().unwrap();
    ///     panic!(); // 该互斥锁随之中毒
    /// }).join();
    /// assert_eq!(mutex.is_poisoned(), true);
    /// ```
    #[inline]
    #[stable(feature = "sync_poison", since = "1.2.0")]
    pub fn is_poisoned(&self) -> bool {
        self.poison.get()
    }

    /// 清除互斥锁的中毒状态。
    ///
    /// 如果互斥锁已中毒，它会一直保持中毒，直到本函数被调用为止。这就允许你从
    /// 中毒状态中恢复，并标记它已经恢复。举例来说，如果该值被一个已知良好
    /// （known-good）的值覆写，那么就可以把该互斥锁标记为未中毒。又或者，可以
    /// 检视该值以判断它是否处于一致（consistent）状态，若是则清除中毒。
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::{Arc, Mutex};
    /// use std::thread;
    ///
    /// let mutex = Arc::new(Mutex::new(0));
    /// let c_mutex = Arc::clone(&mutex);
    ///
    /// let _ = thread::spawn(move || {
    ///     let _lock = c_mutex.lock().unwrap();
    ///     panic!(); // 该互斥锁随之中毒
    /// }).join();
    ///
    /// assert_eq!(mutex.is_poisoned(), true);
    /// let x = mutex.lock().unwrap_or_else(|mut e| {
    ///     **e.get_mut() = 1;
    ///     mutex.clear_poison();
    ///     e.into_inner()
    /// });
    /// assert_eq!(mutex.is_poisoned(), false);
    /// assert_eq!(*x, 1);
    /// ```
    #[inline]
    #[stable(feature = "mutex_unpoison", since = "1.77.0")]
    #[rustc_should_not_be_called_on_const_items]
    pub fn clear_poison(&self) {
        self.poison.clear();
    }

    /// 消耗这个互斥锁，返回其底层数据。
    ///
    /// # Errors
    ///
    /// 如果本互斥锁的另一位用户在持锁期间发生了 panic，则本调用会改为返回一个
    /// 含有底层数据的错误。
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Mutex;
    ///
    /// let mutex = Mutex::new(0);
    /// assert_eq!(mutex.into_inner().unwrap(), 0);
    /// ```
    #[stable(feature = "mutex_into_inner", since = "1.6.0")]
    pub fn into_inner(self) -> LockResult<T>
    where
        T: Sized,
    {
        let data = self.data.into_inner();
        poison::map_result(self.poison.borrow(), |()| data)
    }

    /// 返回底层数据的可变引用。
    ///
    /// 由于本调用以可变方式借用 `Mutex`，无需进行任何实际的加锁——可变借用
    /// 在静态层面即保证：当这个引用存在期间，不可能获取任何新的锁。注意，本
    /// 方法不会清除任何先前被遗弃（abandoned）的锁（例如通过对 [`MutexGuard`]
    /// 调用 [`forget()`]）。
    ///
    /// # Errors
    ///
    /// 如果本互斥锁的另一位用户在持锁期间发生了 panic，则本调用会改为返回一个
    /// 含有底层数据可变引用的错误。
    ///
    /// # Examples
    ///
    /// ```
    /// use std::sync::Mutex;
    ///
    /// let mut mutex = Mutex::new(0);
    /// *mutex.get_mut().unwrap() = 10;
    /// assert_eq!(*mutex.lock().unwrap(), 10);
    /// ```
    ///
    /// [`forget()`]: mem::forget
    #[stable(feature = "mutex_get_mut", since = "1.6.0")]
    pub fn get_mut(&mut self) -> LockResult<&mut T> {
        let data = self.data.get_mut();
        poison::map_result(self.poison.borrow(), |()| data)
    }

    /// 返回底层数据的裸指针（raw pointer）。
    ///
    /// 返回的指针总是非空且对齐良好的，但用户有责任确保：通过它进行的任何
    /// 读写都已正确同步以避免数据竞争，并且在该互斥锁被 drop 之后不再通过它
    /// 读写。
    #[unstable(feature = "mutex_data_ptr", issue = "140368")]
    pub const fn data_ptr(&self) -> *mut T {
        self.data.get()
    }
}

#[stable(feature = "mutex_from", since = "1.24.0")]
impl<T> From<T> for Mutex<T> {
    /// 创建一个新的互斥锁，处于未锁定状态，可随时使用。
    /// 这等价于 [`Mutex::new`]。
    fn from(t: T) -> Self {
        Mutex::new(t)
    }
}

#[stable(feature = "mutex_default", since = "1.10.0")]
impl<T: Default> Default for Mutex<T> {
    /// 用 T 的 `Default` 值创建一个 `Mutex<T>`。
    fn default() -> Mutex<T> {
        Mutex::new(Default::default())
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T: ?Sized + fmt::Debug> fmt::Debug for Mutex<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut d = f.debug_struct("Mutex");
        match self.try_lock() {
            Ok(guard) => {
                d.field("data", &&*guard);
            }
            Err(TryLockError::Poisoned(err)) => {
                d.field("data", &&**err.get_ref());
            }
            Err(TryLockError::WouldBlock) => {
                d.field("data", &"<locked>");
            }
        }
        d.field("poisoned", &self.poison.get());
        d.finish_non_exhaustive()
    }
}

impl<'mutex, T: ?Sized> MutexGuard<'mutex, T> {
    unsafe fn new(lock: &'mutex Mutex<T>) -> LockResult<MutexGuard<'mutex, T>> {
        poison::map_result(lock.poison.guard(), |guard| MutexGuard { lock, poison: guard })
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T: ?Sized> Deref for MutexGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { &*self.lock.data.get() }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T: ?Sized> DerefMut for MutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { &mut *self.lock.data.get() }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T: ?Sized> Drop for MutexGuard<'_, T> {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            self.lock.poison.done(&self.poison);
            self.lock.inner.unlock();
        }
    }
}

#[stable(feature = "std_debug", since = "1.16.0")]
impl<T: ?Sized + fmt::Debug> fmt::Debug for MutexGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

#[stable(feature = "std_guard_impls", since = "1.20.0")]
impl<T: ?Sized + fmt::Display> fmt::Display for MutexGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}

/// 供 [`nonpoison::condvar`](super::condvar) 使用。
pub(super) fn guard_lock<'a, T: ?Sized>(guard: &MutexGuard<'a, T>) -> &'a sys::Mutex {
    &guard.lock.inner
}

/// 供 [`nonpoison::condvar`](super::condvar) 使用。
pub(super) fn guard_poison<'a, T: ?Sized>(guard: &MutexGuard<'a, T>) -> &'a poison::Flag {
    &guard.lock.poison
}

impl<'a, T: ?Sized> MutexGuard<'a, T> {
    /// 为被借用数据的某个组成部分（例如某个枚举变体）制作一个
    /// [`MappedMutexGuard`]。
    ///
    /// 此时 `Mutex` 已被锁定，因此本操作不会失败。
    ///
    /// 这是一个关联函数，需以 `MutexGuard::map(...)` 的形式使用。若设计为方法，
    /// 则会与通过 `Deref` 访问的 `MutexGuard` 内容上同名的方法相冲突。
    #[unstable(feature = "mapped_lock_guards", issue = "117108")]
    pub fn map<U, F>(orig: Self, f: F) -> MappedMutexGuard<'a, U>
    where
        F: FnOnce(&mut T) -> &mut U,
        U: ?Sized,
    {
        // SAFETY: 创建原始守卫时，`MutexGuard::new` 的各项条件均已满足，且在
        // 整个 `map` 与/或 `filter_map` 过程中始终得到维持。该闭包的签名保证
        // 它不会「泄漏」传给它的引用的生命周期。如果该闭包 panic，守卫会被 drop。
        let data = NonNull::from(f(unsafe { &mut *orig.lock.data.get() }));
        let orig = ManuallyDrop::new(orig);
        MappedMutexGuard {
            data,
            inner: &orig.lock.inner,
            poison_flag: &orig.lock.poison,
            poison: orig.poison.clone(),
            _variance: PhantomData,
        }
    }

    /// 为被借用数据的某个组成部分制作一个 [`MappedMutexGuard`]。如果该闭包
    /// 返回 `None`，则把原始守卫作为 `Err(...)` 返回。
    ///
    /// 此时 `Mutex` 已被锁定，因此本操作不会失败。
    ///
    /// 这是一个关联函数，需以 `MutexGuard::filter_map(...)` 的形式使用。若设计
    /// 为方法，则会与通过 `Deref` 访问的 `MutexGuard` 内容上同名的方法相冲突。
    #[unstable(feature = "mapped_lock_guards", issue = "117108")]
    pub fn filter_map<U, F>(orig: Self, f: F) -> Result<MappedMutexGuard<'a, U>, Self>
    where
        F: FnOnce(&mut T) -> Option<&mut U>,
        U: ?Sized,
    {
        // SAFETY: 创建原始守卫时，`MutexGuard::new` 的各项条件均已满足，且在
        // 整个 `map` 与/或 `filter_map` 过程中始终得到维持。该闭包的签名保证
        // 它不会「泄漏」传给它的引用的生命周期。如果该闭包 panic，守卫会被 drop。
        match f(unsafe { &mut *orig.lock.data.get() }) {
            Some(data) => {
                let data = NonNull::from(data);
                let orig = ManuallyDrop::new(orig);
                Ok(MappedMutexGuard {
                    data,
                    inner: &orig.lock.inner,
                    poison_flag: &orig.lock.poison,
                    poison: orig.poison.clone(),
                    _variance: PhantomData,
                })
            }
            None => Err(orig),
        }
    }
}

#[unstable(feature = "mapped_lock_guards", issue = "117108")]
impl<T: ?Sized> Deref for MappedMutexGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &T {
        unsafe { self.data.as_ref() }
    }
}

#[unstable(feature = "mapped_lock_guards", issue = "117108")]
impl<T: ?Sized> DerefMut for MappedMutexGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut T {
        unsafe { self.data.as_mut() }
    }
}

#[unstable(feature = "mapped_lock_guards", issue = "117108")]
impl<T: ?Sized> Drop for MappedMutexGuard<'_, T> {
    #[inline]
    fn drop(&mut self) {
        unsafe {
            self.poison_flag.done(&self.poison);
            self.inner.unlock();
        }
    }
}

#[unstable(feature = "mapped_lock_guards", issue = "117108")]
impl<T: ?Sized + fmt::Debug> fmt::Debug for MappedMutexGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

#[unstable(feature = "mapped_lock_guards", issue = "117108")]
impl<T: ?Sized + fmt::Display> fmt::Display for MappedMutexGuard<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}

impl<'a, T: ?Sized> MappedMutexGuard<'a, T> {
    /// 为被借用数据的某个组成部分（例如某个枚举变体）制作一个
    /// [`MappedMutexGuard`]。
    ///
    /// 此时 `Mutex` 已被锁定，因此本操作不会失败。
    ///
    /// 这是一个关联函数，需以 `MappedMutexGuard::map(...)` 的形式使用。若设计
    /// 为方法，则会与通过 `Deref` 访问的 `MutexGuard` 内容上同名的方法相冲突。
    #[unstable(feature = "mapped_lock_guards", issue = "117108")]
    pub fn map<U, F>(mut orig: Self, f: F) -> MappedMutexGuard<'a, U>
    where
        F: FnOnce(&mut T) -> &mut U,
        U: ?Sized,
    {
        // SAFETY: 创建原始守卫时，`MutexGuard::new` 的各项条件均已满足，且在
        // 整个 `map` 与/或 `filter_map` 过程中始终得到维持。该闭包的签名保证
        // 它不会「泄漏」传给它的引用的生命周期。如果该闭包 panic，守卫会被 drop。
        let data = NonNull::from(f(unsafe { orig.data.as_mut() }));
        let orig = ManuallyDrop::new(orig);
        MappedMutexGuard {
            data,
            inner: orig.inner,
            poison_flag: orig.poison_flag,
            poison: orig.poison.clone(),
            _variance: PhantomData,
        }
    }

    /// 为被借用数据的某个组成部分制作一个 [`MappedMutexGuard`]。如果该闭包
    /// 返回 `None`，则把原始守卫作为 `Err(...)` 返回。
    ///
    /// 此时 `Mutex` 已被锁定，因此本操作不会失败。
    ///
    /// 这是一个关联函数，需以 `MappedMutexGuard::filter_map(...)` 的形式使用。
    /// 若设计为方法，则会与通过 `Deref` 访问的 `MutexGuard` 内容上同名的方法
    /// 相冲突。
    #[unstable(feature = "mapped_lock_guards", issue = "117108")]
    pub fn filter_map<U, F>(mut orig: Self, f: F) -> Result<MappedMutexGuard<'a, U>, Self>
    where
        F: FnOnce(&mut T) -> Option<&mut U>,
        U: ?Sized,
    {
        // SAFETY: 创建原始守卫时，`MutexGuard::new` 的各项条件均已满足，且在
        // 整个 `map` 与/或 `filter_map` 过程中始终得到维持。该闭包的签名保证
        // 它不会「泄漏」传给它的引用的生命周期。如果该闭包 panic，守卫会被 drop。
        match f(unsafe { orig.data.as_mut() }) {
            Some(data) => {
                let data = NonNull::from(data);
                let orig = ManuallyDrop::new(orig);
                Ok(MappedMutexGuard {
                    data,
                    inner: orig.inner,
                    poison_flag: orig.poison_flag,
                    poison: orig.poison.clone(),
                    _variance: PhantomData,
                })
            }
            None => Err(orig),
        }
    }
}
