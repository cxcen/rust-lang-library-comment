//! 线程局部存储（Thread local storage）

#![unstable(feature = "thread_local_internals", issue = "none")]

use crate::cell::{Cell, RefCell};
use crate::error::Error;
use crate::fmt;

/// 一个拥有其内容的线程局部存储（TLS）键。
///
/// 这个键使用目标平台上可用的最快实现。它由 [`thread_local!`] 宏实例化，主要
/// 方法是 [`with`]，不过也有一些辅助方法让使用 [`Cell`] 类型更方便。
///
/// [`with`] 方法会交出对所含值的一个引用，该引用既不能比当前线程活得更久，也
/// 不能逃逸出给定的闭包。
///
/// [`thread_local!`]: crate::thread_local
///
/// # Initialization and Destruction
///
/// 初始化是在线程内首次调用某个 setter（例如 [`with`]）时动态进行的，而实现了
/// [`Drop`] 的值会在线程退出时被析构。其中有一些平台特定的注意事项，将在下文
/// 解释。
/// 注意，若析构函数发生 panic，整个进程将被[中止][aborted]。
/// 在初始化需要内存分配的平台上，这种分配直接通过 [`System`] 进行，从而允许
/// [全局分配器][global allocator]使用线程局部存储。
///
/// 一个 `LocalKey` 的初始化器不能递归地依赖它自身。以这种方式使用 `LocalKey`
/// 可能会在首次调用 `with` 时导致 panic、abort 或无限递归。
///
/// [`System`]: crate::alloc::System
/// [global allocator]: crate::alloc
/// [aborted]: crate::process::abort
///
/// # Single-thread Synchronization
///
/// 尽管不存在与其他线程的潜在竞争，但仍有可能在调用栈的不同位置获取到对同一份
/// 线程局部数据的多个引用。出于这个原因，只能获取共享引用（`&T`）。
///
/// 要想获取独占的可变引用（`&mut T`），通常会使用 [`Cell`] 或 [`RefCell`]
///（关于其具体工作方式的更多信息，请参阅 [`std::cell`]）。为了让这更方便，
/// 还为 [`LocalKey<Cell<T>>`] 和 [`LocalKey<RefCell<T>>`] 提供了专门的实现。
///
/// [`std::cell`]: `crate::cell`
/// [`LocalKey<Cell<T>>`]: struct.LocalKey.html#impl-LocalKey<Cell<T>>
/// [`LocalKey<RefCell<T>>`]: struct.LocalKey.html#impl-LocalKey<RefCell<T>>
///
///
/// # Examples
///
/// ```
/// use std::cell::Cell;
/// use std::thread;
///
/// // 显式的 `const {}` 块能够启用更高效的初始化
/// thread_local!(static FOO: Cell<u32> = const { Cell::new(1) });
///
/// assert_eq!(FOO.get(), 1);
/// FOO.set(2);
///
/// // 每个线程都以初始值 1 开始
/// let t = thread::spawn(move || {
///     assert_eq!(FOO.get(), 1);
///     FOO.set(3);
/// });
///
/// // 等待该线程完成，若 panic 则退出
/// t.join().unwrap();
///
/// // 尽管有子线程的存在，我们仍保留着自己原来的值 2
/// assert_eq!(FOO.get(), 2);
/// ```
///
/// # Platform-specific behavior
///
/// 注意，我们会“尽力（best effort）”确保存放在线程局部存储中的类型的析构函数
/// 得以运行，但并非所有平台都能保证线程局部存储中所有类型的析构函数都会运行。
/// 例如，已知有若干析构函数不会运行的注意事项：
///
/// 1. 在使用基于 pthread 的 TLS 的 Unix 系统上，主线程退出时，主线程上的 TLS
///    值的析构函数不会运行。注意，主线程退出后应用程序也会随即立刻退出。
/// 2. 在所有平台上，TLS 都有可能在析构期间重新初始化其他 TLS 槽。有些平台会
///    确保这不会无限发生，做法是禁止重新初始化任何已被析构的槽，但并非所有平台
///    都有这一防护。那些没有防护的平台通常设有一个人为的上限，超过该上限后就
///    不再运行任何析构函数。
/// 3. 在 Windows 系统上当进程退出时，TLS 析构函数可能只会在导致进程退出的那个
///    线程上运行。这是因为其他线程可能会被强制终止。
///
/// ## Synchronization in thread-local destructors
///
/// 在 Windows 上，应避免在线程局部析构函数中进行同步操作（例如
/// [`JoinHandle::join`]），因为它们容易导致死锁。这是因为在运行析构函数期间会
/// 持有[加载器锁（loader lock）][loader lock]。每当一个线程启动或退出、或一个
/// DLL 被加载或卸载时，都会获取该锁。因此，只要线程局部析构函数还在运行，这些
/// 事件就会被阻塞。
///
/// [loader lock]: https://docs.microsoft.com/en-us/windows/win32/dlls/dynamic-link-library-best-practices
/// [`JoinHandle::join`]: crate::thread::JoinHandle::join
/// [`with`]: LocalKey::with
#[cfg_attr(not(test), rustc_diagnostic_item = "LocalKey")]
#[stable(feature = "rust1", since = "1.0.0")]
pub struct LocalKey<T: 'static> {
    // 这个外层的 `LocalKey<T>` 类型才是会被存放在 static 中的东西，但其内部的
    // 实际数据有时会被打上 #[thread_local] 标记。
    // 一个真正的 static 引用一个 #[thread_local] static 是不合法的，因此我们
    // 通过暴露一个穿过一层函数间接（这个 thunk）的访问器来规避这一点。
    //
    // 注意这个 thunk 本身是不安全的，因为它所返回的、数据所在槽位的生命周期
    // `'static` 实际上并不有效。这里真正的生命周期其实比当前运行的线程稍短！
    //
    // 尽管这是一层额外的间接，但理论上它应当能被 LLVM 轻易地去虚化
    //（devirtualize），因为 `inner` 的值从不改变，并且该常量在一个 crate 内
    // 应当是只读的。这主要只有在 TLS static 被跨 crate 导出时才会遇到问题。
    inner: fn(Option<&mut Option<T>>) -> *const T,
}

#[stable(feature = "std_debug", since = "1.16.0")]
impl<T: 'static> fmt::Debug for LocalKey<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LocalKey").finish_non_exhaustive()
    }
}

#[doc(hidden)]
#[allow_internal_unstable(thread_local_internals)]
#[unstable(feature = "thread_local_internals", issue = "none")]
#[rustc_macro_transparency = "semiopaque"]
pub macro thread_local_process_attrs {

    // 解析 `cfg_attr` 以判断它是否为一个 `rustc_align_static`。
    // 每个 `cfg_attr` 的右侧（RHS）可以有零个或多个属性，并且可以嵌套。

    // `cfg_attr` 解析完毕，其中没有 `rustc_align_static`
    (
        [] [$(#[$($prev_other_attrs:tt)*])*];
        @processing_cfg_attr { pred: ($($predicate:tt)*), rhs: [] };
        [$($prev_align_attrs_ret:tt)*] [$($prev_other_attrs_ret:tt)*];
        $($rest:tt)*
    ) => (
        $crate::thread::local_impl::thread_local_process_attrs!(
            [$($prev_align_attrs_ret)*] [$($prev_other_attrs_ret)* #[cfg_attr($($predicate)*, $($($prev_other_attrs)*),*)]];
            $($rest)*
        );
    ),

    // `cfg_attr` 解析完毕，其中只有 `rustc_align_static` 而无其他
    (
        [$(#[$($prev_align_attrs:tt)*])+] [];
        @processing_cfg_attr { pred: ($($predicate:tt)*), rhs: [] };
        [$($prev_align_attrs_ret:tt)*] [$($prev_other_attrs_ret:tt)*];
        $($rest:tt)*
    ) => (
        $crate::thread::local_impl::thread_local_process_attrs!(
            [$($prev_align_attrs_ret)*  #[cfg_attr($($predicate)*, $($($prev_align_attrs)*),+)]] [$($prev_other_attrs_ret)*];
            $($rest)*
        );
    ),

    // `cfg_attr` 解析完毕，其中混有 `rustc_align_static` 和其他属性
    (
        [$(#[$($prev_align_attrs:tt)*])+] [$(#[$($prev_other_attrs:tt)*])+];
        @processing_cfg_attr { pred: ($($predicate:tt)*), rhs: [] };
        [$($prev_align_attrs_ret:tt)*] [$($prev_other_attrs_ret:tt)*];
        $($rest:tt)*
    ) => (
        $crate::thread::local_impl::thread_local_process_attrs!(
            [$($prev_align_attrs_ret)*  #[cfg_attr($($predicate)*, $($($prev_align_attrs)*),+)]] [$($prev_other_attrs_ret)* #[cfg_attr($($predicate)*, $($($prev_other_attrs)*),+)]];
            $($rest)*
        );
    ),

    // 它是一个 `rustc_align_static`
    (
        [$($prev_align_attrs:tt)*] [$($prev_other_attrs:tt)*];
        @processing_cfg_attr { pred: ($($predicate:tt)*), rhs: [rustc_align_static($($align_static_args:tt)*) $(, $($attr_rhs:tt)*)?] };
        $($rest:tt)*
    ) => (
        $crate::thread::local_impl::thread_local_process_attrs!(
            [$($prev_align_attrs)* #[rustc_align_static($($align_static_args)*)]] [$($prev_other_attrs)*];
            @processing_cfg_attr { pred: ($($predicate)*), rhs: [$($($attr_rhs)*)?] };
            $($rest)*
        );
    ),

    // 它是一个嵌套的 `cfg_attr(true, ...)`；递归进入其 RHS
    (
        [$($prev_align_attrs:tt)*] [$($prev_other_attrs:tt)*];
        @processing_cfg_attr { pred: ($($predicate:tt)*), rhs: [cfg_attr(true, $($cfg_rhs:tt)*) $(, $($attr_rhs:tt)*)?] };
        $($rest:tt)*
    ) => (
        $crate::thread::local_impl::thread_local_process_attrs!(
            [] [];
            @processing_cfg_attr { pred: (true), rhs: [$($cfg_rhs)*] };
            [$($prev_align_attrs)*] [$($prev_other_attrs)*];
            @processing_cfg_attr { pred: ($($predicate)*), rhs: [$($($attr_rhs)*)?] };
            $($rest)*
        );
    ),

    // 它是一个嵌套的 `cfg_attr(false, ...)`；递归进入其 RHS
    (
        [$($prev_align_attrs:tt)*] [$($prev_other_attrs:tt)*];
        @processing_cfg_attr { pred: ($($predicate:tt)*), rhs: [cfg_attr(false, $($cfg_rhs:tt)*) $(, $($attr_rhs:tt)*)?] };
        $($rest:tt)*
    ) => (
        $crate::thread::local_impl::thread_local_process_attrs!(
            [] [];
            @processing_cfg_attr { pred: (false), rhs: [$($cfg_rhs)*] };
            [$($prev_align_attrs)*] [$($prev_other_attrs)*];
            @processing_cfg_attr { pred: ($($predicate)*), rhs: [$($($attr_rhs)*)?] };
            $($rest)*
        );
    ),


    // 它是一个嵌套的 `cfg_attr(..., ...)`；递归进入其 RHS
    (
        [$($prev_align_attrs:tt)*] [$($prev_other_attrs:tt)*];
        @processing_cfg_attr { pred: ($($predicate:tt)*), rhs: [cfg_attr($cfg_lhs:meta, $($cfg_rhs:tt)*) $(, $($attr_rhs:tt)*)?] };
        $($rest:tt)*
    ) => (
        $crate::thread::local_impl::thread_local_process_attrs!(
            [] [];
            @processing_cfg_attr { pred: ($cfg_lhs), rhs: [$($cfg_rhs)*] };
            [$($prev_align_attrs)*] [$($prev_other_attrs)*];
            @processing_cfg_attr { pred: ($($predicate)*), rhs: [$($($attr_rhs)*)?] };
            $($rest)*
        );
    ),

    // 它是某种其他属性
    (
        [$($prev_align_attrs:tt)*] [$($prev_other_attrs:tt)*];
        @processing_cfg_attr { pred: ($($predicate:tt)*), rhs: [$meta:meta $(, $($attr_rhs:tt)*)?] };
        $($rest:tt)*
    ) => (
        $crate::thread::local_impl::thread_local_process_attrs!(
            [$($prev_align_attrs)*] [$($prev_other_attrs)* #[$meta]];
            @processing_cfg_attr { pred: ($($predicate)*), rhs: [$($($attr_rhs)*)?] };
            $($rest)*
        );
    ),


    // 把属性分为 `rustc_align_static` 和其余所有属性两类：

    // `rustc_align_static` 属性
    ([$($prev_align_attrs:tt)*] [$($prev_other_attrs:tt)*]; #[rustc_align_static $($attr_rest:tt)*] $($rest:tt)*) => (
        $crate::thread::local_impl::thread_local_process_attrs!(
            [$($prev_align_attrs)* #[rustc_align_static $($attr_rest)*]] [$($prev_other_attrs)*];
            $($rest)*
        );
    ),

    // `cfg_attr(true, ...)` 属性；解析它
    ([$($prev_align_attrs:tt)*] [$($prev_other_attrs:tt)*]; #[cfg_attr(true, $($cfg_rhs:tt)*)] $($rest:tt)*) => (
        $crate::thread::local_impl::thread_local_process_attrs!(
            [] [];
            @processing_cfg_attr { pred: (true), rhs: [$($cfg_rhs)*] };
            [$($prev_align_attrs)*] [$($prev_other_attrs)*];
            $($rest)*
        );
    ),

    // `cfg_attr(false, ...)` 属性；解析它
    ([$($prev_align_attrs:tt)*] [$($prev_other_attrs:tt)*]; #[cfg_attr(false, $($cfg_rhs:tt)*)] $($rest:tt)*) => (
        $crate::thread::local_impl::thread_local_process_attrs!(
            [] [];
            @processing_cfg_attr { pred: (false), rhs: [$($cfg_rhs)*] };
            [$($prev_align_attrs)*] [$($prev_other_attrs)*];
            $($rest)*
        );
    ),

    // `cfg_attr(..., ...)` 属性；解析它
    ([$($prev_align_attrs:tt)*] [$($prev_other_attrs:tt)*]; #[cfg_attr($cfg_pred:meta, $($cfg_rhs:tt)*)] $($rest:tt)*) => (
        $crate::thread::local_impl::thread_local_process_attrs!(
            [] [];
            @processing_cfg_attr { pred: ($cfg_pred), rhs: [$($cfg_rhs)*] };
            [$($prev_align_attrs)*] [$($prev_other_attrs)*];
            $($rest)*
        );
    ),

    // 不跟随任何其他属性的文档注释；一次性整体处理，以避免触及递归上限
    ([$($prev_align_attrs:tt)*] [$($prev_other_attrs:tt)*]; $(#[doc $($doc_rhs:tt)*])+ $vis:vis static $($rest:tt)*) => (
        $crate::thread::local_impl::thread_local_process_attrs!(
            [$($prev_align_attrs)*] [$($prev_other_attrs)* $(#[doc $($doc_rhs)*])+];
            $vis static $($rest)*
        );
    ),

    // 8 行文档注释；一次性整体处理，以避免触及递归上限
    ([$($prev_align_attrs:tt)*] [$($prev_other_attrs:tt)*];
     #[doc $($doc_rhs_1:tt)*] #[doc $($doc_rhs_2:tt)*] #[doc $($doc_rhs_3:tt)*] #[doc $($doc_rhs_4:tt)*]
     #[doc $($doc_rhs_5:tt)*] #[doc $($doc_rhs_6:tt)*] #[doc $($doc_rhs_7:tt)*] #[doc $($doc_rhs_8:tt)*]
     $($rest:tt)*) => (
        $crate::thread::local_impl::thread_local_process_attrs!(
            [$($prev_align_attrs)*] [$($prev_other_attrs)*
            #[doc $($doc_rhs_1)*] #[doc $($doc_rhs_2)*] #[doc $($doc_rhs_3)*] #[doc $($doc_rhs_4)*]
            #[doc $($doc_rhs_5)*] #[doc $($doc_rhs_6)*] #[doc $($doc_rhs_7)*] #[doc $($doc_rhs_8)*]];
            $($rest)*
        );
    ),

    // 其他属性
    ([$($prev_align_attrs:tt)*] [$($prev_other_attrs:tt)*]; #[$($attr:tt)*] $($rest:tt)*) => (
        $crate::thread::local_impl::thread_local_process_attrs!(
            [$($prev_align_attrs)*] [$($prev_other_attrs)* #[$($attr)*]];
            $($rest)*
        );
    ),


    // 在属性被完全归类之后，委托给 `thread_local_inner`：

    // 处理 `const` 声明并递归
    ([$($align_attrs:tt)*] [$($other_attrs:tt)*]; $vis:vis static $name:ident: $t:ty = const $init:block $(; $($($rest:tt)+)?)?) => (
        $($other_attrs)* $vis const $name: $crate::thread::LocalKey<$t> =
            $crate::thread::local_impl::thread_local_inner!(@key $t, $($align_attrs)*, const $init);

        $($($crate::thread::local_impl::thread_local_process_attrs!([] []; $($rest)+);)?)?
    ),

    // 处理非 `const` 声明并递归
    ([$($align_attrs:tt)*] [$($other_attrs:tt)*]; $vis:vis static $name:ident: $t:ty = $init:expr $(; $($($rest:tt)+)?)?) => (
        $($other_attrs)* $vis const $name: $crate::thread::LocalKey<$t> =
            $crate::thread::local_impl::thread_local_inner!(@key $t, $($align_attrs)*, $init);

        $($($crate::thread::local_impl::thread_local_process_attrs!([] []; $($rest)+);)?)?
    ),
}

/// 声明一个新的、类型为 [`std::thread::LocalKey`] 的线程局部存储键。
///
/// # Syntax
///
/// 这个宏可以包裹任意数量的 static 声明，并把它们变成线程局部的。允许为每个
/// static 指定可见性和属性。示例：
///
/// ```
/// use std::cell::{Cell, RefCell};
///
/// thread_local! {
///     pub static FOO: Cell<u32> = const { Cell::new(1) };
///
///     static BAR: RefCell<Vec<f32>> = RefCell::new(vec![1.0, 2.0]);
/// }
///
/// assert_eq!(FOO.get(), 1);
/// BAR.with_borrow(|v| assert_eq!(v[1], 2.0));
/// ```
///
/// 注意，只能获取对内部数据的共享引用（`&T`），所以通常会使用诸如 [`Cell`] 或
/// [`RefCell`] 这样的类型来允许可变访问。
///
/// 当初始化表达式可以作为常量求值时，这个宏支持一种特殊的 `const {}` 语法。
/// 它能够启用一种更高效的线程局部实现，可以避免惰性初始化。对于那些
/// [无需被丢弃][crate::mem::needs_drop]的类型，它甚至能够启用一种更高效的实现，
/// 无需追踪任何额外状态。
///
/// ```
/// use std::cell::RefCell;
///
/// thread_local! {
///     pub static FOO: RefCell<Vec<u32>> = const { RefCell::new(Vec::new()) };
/// }
///
/// FOO.with_borrow(|v| assert_eq!(v.len(), 0));
/// ```
///
/// 更多信息请参阅 [`LocalKey` 文档][`std::thread::LocalKey`]。
///
/// [`std::thread::LocalKey`]: crate::thread::LocalKey
#[macro_export]
#[stable(feature = "rust1", since = "1.0.0")]
#[cfg_attr(not(test), rustc_diagnostic_item = "thread_local_macro")]
#[allow_internal_unstable(thread_local_internals)]
macro_rules! thread_local {
    () => {};

    ($($tt:tt)+) => {
        $crate::thread::local_impl::thread_local_process_attrs!([] []; $($tt)+);
    };
}

/// 由 [`LocalKey::try_with`](struct.LocalKey.html#method.try_with) 返回的错误。
#[stable(feature = "thread_local_try_with", since = "1.26.0")]
#[non_exhaustive]
#[derive(Clone, Copy, Eq, PartialEq)]
pub struct AccessError;

#[stable(feature = "thread_local_try_with", since = "1.26.0")]
impl fmt::Debug for AccessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AccessError").finish()
    }
}

#[stable(feature = "thread_local_try_with", since = "1.26.0")]
impl fmt::Display for AccessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt("already destroyed", f)
    }
}

#[stable(feature = "thread_local_try_with", since = "1.26.0")]
impl Error for AccessError {}

// 这确保 panic 代码会从 `LocalKey` 的 `with` 中被外联（outline）出去。
#[cfg_attr(not(panic = "immediate-abort"), inline(never))]
#[track_caller]
#[cold]
fn panic_access_error(err: AccessError) -> ! {
    panic!("cannot access a Thread Local Storage value during or after destruction: {err:?}")
}

impl<T: 'static> LocalKey<T> {
    #[doc(hidden)]
    #[unstable(
        feature = "thread_local_internals",
        reason = "recently added to create a key",
        issue = "none"
    )]
    pub const unsafe fn new(inner: fn(Option<&mut Option<T>>) -> *const T) -> LocalKey<T> {
        LocalKey { inner }
    }

    /// 获取对这个 TLS 键中值的一个引用。
    ///
    /// 如果本线程尚未引用过这个键，这会惰性地初始化它的值。
    ///
    /// # Panics
    ///
    /// 如果该键当前正在运行它的析构函数，本函数将会 `panic!()`；并且，如果该键
    /// 此前在本线程上已经运行过析构函数，它**可能**会 panic。
    ///
    /// # Examples
    ///
    /// ```
    /// thread_local! {
    ///     pub static STATIC: String = String::from("I am");
    /// }
    ///
    /// assert_eq!(
    ///     STATIC.with(|original_value| format!("{original_value} initialized")),
    ///     "I am initialized",
    /// );
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn with<F, R>(&'static self, f: F) -> R
    where
        F: FnOnce(&T) -> R,
    {
        match self.try_with(f) {
            Ok(r) => r,
            Err(err) => panic_access_error(err),
        }
    }

    /// 获取对这个 TLS 键中值的一个引用。
    ///
    /// 如果本线程尚未引用过这个键，这会惰性地初始化它的值。如果该键已被销毁
    ///（这可能在析构函数中调用本函数时发生），本函数会返回一个 [`AccessError`]。
    ///
    /// # Panics
    ///
    /// 如果该键未初始化且其初始化器发生 panic，本函数仍然会 `panic!()`。
    ///
    /// # Examples
    ///
    /// ```
    /// thread_local! {
    ///     pub static STATIC: String = String::from("I am");
    /// }
    ///
    /// assert_eq!(
    ///     STATIC.try_with(|original_value| format!("{original_value} initialized")),
    ///     Ok(String::from("I am initialized")),
    /// );
    /// ```
    #[stable(feature = "thread_local_try_with", since = "1.26.0")]
    #[inline]
    pub fn try_with<F, R>(&'static self, f: F) -> Result<R, AccessError>
    where
        F: FnOnce(&T) -> R,
    {
        let thread_local = unsafe { (self.inner)(None).as_ref().ok_or(AccessError)? };
        Ok(f(thread_local))
    }

    /// 获取对这个 TLS 键中值的一个引用，如果它在本线程上尚未初始化，则用 `init`
    /// 来初始化它。
    ///
    /// 如果 `init` 被用来初始化了这个线程局部变量，则传给 `f` 的第一个参数为
    /// `None`。如果它此前已被初始化，则传给 `f` 的是 `Some(init)`。
    ///
    /// # Panics
    ///
    /// 如果该键当前正在运行它的析构函数，本函数将会 panic；并且，如果该键此前在
    /// 本线程上已经运行过析构函数，它**可能**会 panic。
    fn initialize_with<F, R>(&'static self, init: T, f: F) -> R
    where
        F: FnOnce(Option<T>, &T) -> R,
    {
        let mut init = Some(init);

        let reference = unsafe {
            match (self.inner)(Some(&mut init)).as_ref() {
                Some(r) => r,
                None => panic_access_error(AccessError),
            }
        };

        f(init, reference)
    }
}

impl<T: 'static> LocalKey<Cell<T>> {
    /// 设置或初始化所含的值。
    ///
    /// 与其他方法不同，这**不会**运行该线程局部的惰性初始化器。相反，如果它尚未
    /// 初始化，则会直接用给定的值进行初始化。
    ///
    /// # Panics
    ///
    /// 如果该键当前正在运行它的析构函数，则会 panic；并且，如果该键此前在本线程上
    /// 已经运行过析构函数，它**可能**会 panic。
    ///
    /// # Examples
    ///
    /// ```
    /// use std::cell::Cell;
    ///
    /// thread_local! {
    ///     static X: Cell<i32> = panic!("!");
    /// }
    ///
    /// // 在这里调用 X.get() 会导致 panic。
    ///
    /// X.set(123); // 但 X.set() 没问题，因为它会跳过上面的初始化器。
    ///
    /// assert_eq!(X.get(), 123);
    /// ```
    #[stable(feature = "local_key_cell_methods", since = "1.73.0")]
    pub fn set(&'static self, value: T) {
        self.initialize_with(Cell::new(value), |value, cell| {
            if let Some(value) = value {
                // 该 cell 已经被初始化过了，所以 `value` 并没有被用于初始化它。
                // 因此我们改为用新值覆盖当前值。
                cell.set(value.into_inner());
            }
        });
    }

    /// 返回所含值的一份副本。
    ///
    /// 如果本线程尚未引用过这个键，这会惰性地初始化它的值。
    ///
    /// # Panics
    ///
    /// 如果该键当前正在运行它的析构函数，则会 panic；并且，如果该键此前在本线程上
    /// 已经运行过析构函数，它**可能**会 panic。
    ///
    /// # Examples
    ///
    /// ```
    /// use std::cell::Cell;
    ///
    /// thread_local! {
    ///     static X: Cell<i32> = const { Cell::new(1) };
    /// }
    ///
    /// assert_eq!(X.get(), 1);
    /// ```
    #[stable(feature = "local_key_cell_methods", since = "1.73.0")]
    pub fn get(&'static self) -> T
    where
        T: Copy,
    {
        self.with(Cell::get)
    }

    /// 取走所含的值，并在原处留下 `Default::default()`。
    ///
    /// 如果本线程尚未引用过这个键，这会惰性地初始化它的值。
    ///
    /// # Panics
    ///
    /// 如果该键当前正在运行它的析构函数，则会 panic；并且，如果该键此前在本线程上
    /// 已经运行过析构函数，它**可能**会 panic。
    ///
    /// # Examples
    ///
    /// ```
    /// use std::cell::Cell;
    ///
    /// thread_local! {
    ///     static X: Cell<Option<i32>> = const { Cell::new(Some(1)) };
    /// }
    ///
    /// assert_eq!(X.take(), Some(1));
    /// assert_eq!(X.take(), None);
    /// ```
    #[stable(feature = "local_key_cell_methods", since = "1.73.0")]
    pub fn take(&'static self) -> T
    where
        T: Default,
    {
        self.with(Cell::take)
    }

    /// 替换所含的值，并返回旧值。
    ///
    /// 如果本线程尚未引用过这个键，这会惰性地初始化它的值。
    ///
    /// # Panics
    ///
    /// 如果该键当前正在运行它的析构函数，则会 panic；并且，如果该键此前在本线程上
    /// 已经运行过析构函数，它**可能**会 panic。
    ///
    /// # Examples
    ///
    /// ```
    /// use std::cell::Cell;
    ///
    /// thread_local! {
    ///     static X: Cell<i32> = const { Cell::new(1) };
    /// }
    ///
    /// assert_eq!(X.replace(2), 1);
    /// assert_eq!(X.replace(3), 2);
    /// ```
    #[stable(feature = "local_key_cell_methods", since = "1.73.0")]
    #[rustc_confusables("swap")]
    pub fn replace(&'static self, value: T) -> T {
        self.with(|cell| cell.replace(value))
    }

    /// 使用一个函数来更新所含的值。
    ///
    /// # Examples
    ///
    /// ```
    /// #![feature(local_key_cell_update)]
    /// use std::cell::Cell;
    ///
    /// thread_local! {
    ///     static X: Cell<i32> = const { Cell::new(5) };
    /// }
    ///
    /// X.update(|x| x + 1);
    /// assert_eq!(X.get(), 6);
    /// ```
    #[unstable(feature = "local_key_cell_update", issue = "143989")]
    pub fn update(&'static self, f: impl FnOnce(T) -> T)
    where
        T: Copy,
    {
        self.with(|cell| cell.update(f))
    }
}

impl<T: 'static> LocalKey<RefCell<T>> {
    /// 获取对所含值的一个引用。
    ///
    /// 如果本线程尚未引用过这个键，这会惰性地初始化它的值。
    ///
    /// # Panics
    ///
    /// 如果该值当前正被可变借用，则会 panic。
    ///
    /// 如果该键当前正在运行它的析构函数，则会 panic；并且，如果该键此前在本线程上
    /// 已经运行过析构函数，它**可能**会 panic。
    ///
    /// # Examples
    ///
    /// ```
    /// use std::cell::RefCell;
    ///
    /// thread_local! {
    ///     static X: RefCell<Vec<i32>> = RefCell::new(Vec::new());
    /// }
    ///
    /// X.with_borrow(|v| assert!(v.is_empty()));
    /// ```
    #[stable(feature = "local_key_cell_methods", since = "1.73.0")]
    pub fn with_borrow<F, R>(&'static self, f: F) -> R
    where
        F: FnOnce(&T) -> R,
    {
        self.with(|cell| f(&cell.borrow()))
    }

    /// 获取对所含值的一个可变引用。
    ///
    /// 如果本线程尚未引用过这个键，这会惰性地初始化它的值。
    ///
    /// # Panics
    ///
    /// 如果该值当前正被借用，则会 panic。
    ///
    /// 如果该键当前正在运行它的析构函数，则会 panic；并且，如果该键此前在本线程上
    /// 已经运行过析构函数，它**可能**会 panic。
    ///
    /// # Examples
    ///
    /// ```
    /// use std::cell::RefCell;
    ///
    /// thread_local! {
    ///     static X: RefCell<Vec<i32>> = RefCell::new(Vec::new());
    /// }
    ///
    /// X.with_borrow_mut(|v| v.push(1));
    ///
    /// X.with_borrow(|v| assert_eq!(*v, vec![1]));
    /// ```
    #[stable(feature = "local_key_cell_methods", since = "1.73.0")]
    pub fn with_borrow_mut<F, R>(&'static self, f: F) -> R
    where
        F: FnOnce(&mut T) -> R,
    {
        self.with(|cell| f(&mut cell.borrow_mut()))
    }

    /// 设置或初始化所含的值。
    ///
    /// 与其他方法不同，这**不会**运行该线程局部的惰性初始化器。相反，如果它尚未
    /// 初始化，则会直接用给定的值进行初始化。
    ///
    /// # Panics
    ///
    /// 如果该值当前正被借用，则会 panic。
    ///
    /// 如果该键当前正在运行它的析构函数，则会 panic；并且，如果该键此前在本线程上
    /// 已经运行过析构函数，它**可能**会 panic。
    ///
    /// # Examples
    ///
    /// ```
    /// use std::cell::RefCell;
    ///
    /// thread_local! {
    ///     static X: RefCell<Vec<i32>> = panic!("!");
    /// }
    ///
    /// // 在这里调用 X.with() 会导致 panic。
    ///
    /// X.set(vec![1, 2, 3]); // 但 X.set() 没问题，因为它会跳过上面的初始化器。
    ///
    /// X.with_borrow(|v| assert_eq!(*v, vec![1, 2, 3]));
    /// ```
    #[stable(feature = "local_key_cell_methods", since = "1.73.0")]
    pub fn set(&'static self, value: T) {
        self.initialize_with(RefCell::new(value), |value, cell| {
            if let Some(value) = value {
                // 该 cell 已经被初始化过了，所以 `value` 并没有被用于初始化它。
                // 因此我们改为用新值覆盖当前值。
                *cell.borrow_mut() = value.into_inner();
            }
        });
    }

    /// 取走所含的值，并在原处留下 `Default::default()`。
    ///
    /// 如果本线程尚未引用过这个键，这会惰性地初始化它的值。
    ///
    /// # Panics
    ///
    /// 如果该值当前正被借用，则会 panic。
    ///
    /// 如果该键当前正在运行它的析构函数，则会 panic；并且，如果该键此前在本线程上
    /// 已经运行过析构函数，它**可能**会 panic。
    ///
    /// # Examples
    ///
    /// ```
    /// use std::cell::RefCell;
    ///
    /// thread_local! {
    ///     static X: RefCell<Vec<i32>> = RefCell::new(Vec::new());
    /// }
    ///
    /// X.with_borrow_mut(|v| v.push(1));
    ///
    /// let a = X.take();
    ///
    /// assert_eq!(a, vec![1]);
    ///
    /// X.with_borrow(|v| assert!(v.is_empty()));
    /// ```
    #[stable(feature = "local_key_cell_methods", since = "1.73.0")]
    pub fn take(&'static self) -> T
    where
        T: Default,
    {
        self.with(RefCell::take)
    }

    /// 替换所含的值，并返回旧值。
    ///
    /// # Panics
    ///
    /// 如果该值当前正被借用，则会 panic。
    ///
    /// 如果该键当前正在运行它的析构函数，则会 panic；并且，如果该键此前在本线程上
    /// 已经运行过析构函数，它**可能**会 panic。
    ///
    /// # Examples
    ///
    /// ```
    /// use std::cell::RefCell;
    ///
    /// thread_local! {
    ///     static X: RefCell<Vec<i32>> = RefCell::new(Vec::new());
    /// }
    ///
    /// let prev = X.replace(vec![1, 2, 3]);
    /// assert!(prev.is_empty());
    ///
    /// X.with_borrow(|v| assert_eq!(*v, vec![1, 2, 3]));
    /// ```
    #[stable(feature = "local_key_cell_methods", since = "1.73.0")]
    #[rustc_confusables("swap")]
    pub fn replace(&'static self, value: T) -> T {
        self.with(|cell| cell.replace(value))
    }
}
