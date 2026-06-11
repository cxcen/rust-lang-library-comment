//! 对拥有原生 TLS 的平台的线程本地（thread local）支持。
//!
//! 为了获得最佳性能，我们根据所用的初始化方式（`const` 还是惰性 lazy）
//! 以及所存储类型的 drop 需求，从四种不同的类型中为 TLS 变量做选择：
//!
//! |         | `Drop`               | `!Drop`             |
//! |--------:|:--------------------:|:-------------------:|
//! | `const` | `EagerStorage<T>`    | `T`                 |
//! | lazy    | `LazyStorage<T, ()>` | `LazyStorage<T, !>` |
//!
//! 对于 `const` 初始化且 `!Drop` 的类型，我们直接使用 `T`；但对于其他情形，
//! 我们实现了一个状态机来处理该变量的初始化、它的析构函数以及销毁。
//! 在访问该 TLS 变量时，会比较当前的状态：
//!
//! 1. 如果状态是 `Initial`，则初始化存储、把状态转移为 `Alive`，并（在适用时）
//!    注册析构函数，然后返回一个指向该值的引用。
//! 2. 如果状态是 `Alive`，则说明此前已完成初始化，于是返回一个指向该值的引用。
//! 3. 如果状态是 `Destroyed`，则说明析构函数已经运行过，于是返回 [`None`]。
//!
//! TLS 析构函数会把状态设置为 `Destroyed` 并 drop 当前的值。
//!
//! 为简化代码，我们让 `LazyStorage` 在“已销毁状态”上是泛型的，并对 `!Drop`
//! 类型使用 `!` 类型（never type）作为类型参数。这样就为这些值消除了
//! `Destroyed` 状态，从而可以让 `State` 枚举发生更多的 niche 优化。
//! 对于 `Drop` 类型，则使用 `()`。

use crate::cell::Cell;
use crate::ptr;

mod eager;
mod lazy;

pub use eager::Storage as EagerStorage;
pub use lazy::Storage as LazyStorage;

#[doc(hidden)]
#[allow_internal_unstable(
    thread_local_internals,
    cfg_target_thread_local,
    thread_local,
    never_type
)]
#[allow_internal_unsafe]
#[unstable(feature = "thread_local_internals", issue = "none")]
#[rustc_macro_transparency = "semiopaque"]
pub macro thread_local_inner {
    // 注意：我们不能用 `use` 来导入 `LocalKey`、`LazyStorage` 或 `EagerStorage`，
    // 因为那可能会遮蔽（shadow）用户提供的同名类型或类型别名。如果这些类型被重命名，
    // 请更新 `tests/thread.rs` 中的遮蔽（shadowing）测试。

    // 用于为 const 初始化的线程本地变量生成 `LocalKey` 值。
    (@key $t:ty, $(#[$align_attr:meta])*, const $init:expr) => {{
        const __RUST_STD_INTERNAL_INIT: $t = $init;

        unsafe {
            $crate::thread::LocalKey::new(const {
                if $crate::mem::needs_drop::<$t>() {
                    |_| {
                        #[thread_local]
                        $(#[$align_attr])*
                        static __RUST_STD_INTERNAL_VAL: $crate::thread::local_impl::EagerStorage<$t>
                            = $crate::thread::local_impl::EagerStorage::new(__RUST_STD_INTERNAL_INIT);
                        __RUST_STD_INTERNAL_VAL.get()
                    }
                } else {
                    |_| {
                        #[thread_local]
                        $(#[$align_attr])*
                        static __RUST_STD_INTERNAL_VAL: $t = __RUST_STD_INTERNAL_INIT;
                        &__RUST_STD_INTERNAL_VAL
                    }
                }
            })
        }
    }},

    // 用于为 `thread_local!` 生成 `LocalKey` 值
    (@key $t:ty, $(#[$align_attr:meta])*, $init:expr) => {{
        #[inline]
        fn __rust_std_internal_init_fn() -> $t {
            $init
        }

        unsafe {
            $crate::thread::LocalKey::new(const {
                if $crate::mem::needs_drop::<$t>() {
                    |__rust_std_internal_init| {
                        #[thread_local]
                        $(#[$align_attr])*
                        static __RUST_STD_INTERNAL_VAL: $crate::thread::local_impl::LazyStorage<$t, ()>
                            = $crate::thread::local_impl::LazyStorage::new();
                        __RUST_STD_INTERNAL_VAL.get_or_init(__rust_std_internal_init, __rust_std_internal_init_fn)
                    }
                } else {
                    |__rust_std_internal_init| {
                        #[thread_local]
                        $(#[$align_attr])*
                        static __RUST_STD_INTERNAL_VAL: $crate::thread::local_impl::LazyStorage<$t, !>
                            = $crate::thread::local_impl::LazyStorage::new();
                        __RUST_STD_INTERNAL_VAL.get_or_init(__rust_std_internal_init, __rust_std_internal_init_fn)
                    }
                }
            })
        }
    }},
}

#[rustc_macro_transparency = "semiopaque"]
pub(crate) macro local_pointer {
    () => {},
    ($vis:vis static $name:ident; $($rest:tt)*) => {
        #[thread_local]
        $vis static $name: $crate::sys::thread_local::LocalPointer = $crate::sys::thread_local::LocalPointer::__new();
        $crate::sys::thread_local::local_pointer! { $($rest)* }
    },
}

pub(crate) struct LocalPointer {
    p: Cell<*mut ()>,
}

impl LocalPointer {
    pub const fn __new() -> LocalPointer {
        LocalPointer { p: Cell::new(ptr::null_mut()) }
    }

    pub fn get(&self) -> *mut () {
        self.p.get()
    }

    pub fn set(&self, p: *mut ()) {
        self.p.set(p)
    }
}
