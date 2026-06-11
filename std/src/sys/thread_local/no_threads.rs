//! 在像 wasm 这样的某些 target 上没有线程，因此无需生成线程本地变量，
//! 我们可以转而直接使用普通的 static！

use crate::cell::{Cell, UnsafeCell};
use crate::mem::MaybeUninit;
use crate::ptr;

#[doc(hidden)]
#[allow_internal_unstable(thread_local_internals)]
#[allow_internal_unsafe]
#[unstable(feature = "thread_local_internals", issue = "none")]
#[rustc_macro_transparency = "semiopaque"]
pub macro thread_local_inner {
    // 用于为 const 初始化的线程本地变量生成 `LocalKey` 值
    (@key $t:ty, $(#[$align_attr:meta])*, const $init:expr) => {{
        const __RUST_STD_INTERNAL_INIT: $t = $init;

        // 注意：如果这些类型被重命名，请更新 `tests/thread.rs` 中的遮蔽（shadowing）测试。
        unsafe {
            $crate::thread::LocalKey::new(|_| {
                $(#[$align_attr])*
                static __RUST_STD_INTERNAL_VAL: $crate::thread::local_impl::EagerStorage<$t> =
                    $crate::thread::local_impl::EagerStorage { value: __RUST_STD_INTERNAL_INIT };
                &__RUST_STD_INTERNAL_VAL.value
            })
        }
    }},

    // 用于为 `thread_local!` 生成 `LocalKey` 值
    (@key $t:ty, $(#[$align_attr:meta])*, $init:expr) => {{
        #[inline]
        fn __rust_std_internal_init_fn() -> $t { $init }

        unsafe {
            $crate::thread::LocalKey::new(|__rust_std_internal_init| {
                $(#[$align_attr])*
                static __RUST_STD_INTERNAL_VAL: $crate::thread::local_impl::LazyStorage<$t> = $crate::thread::local_impl::LazyStorage::new();
                __RUST_STD_INTERNAL_VAL.get(__rust_std_internal_init, __rust_std_internal_init_fn)
            })
        }
    }},
}

#[allow(missing_debug_implementations)]
#[repr(transparent)] // 为了 `#[rustc_align_static]` 的正确性，这是必需的
pub struct EagerStorage<T> {
    pub value: T,
}

// SAFETY：该 target 没有线程。
unsafe impl<T> Sync for EagerStorage<T> {}

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Initial,
    Alive,
    Destroying,
}

#[allow(missing_debug_implementations)]
#[repr(C)]
pub struct LazyStorage<T> {
    // 为了 `#[rustc_align_static]` 的正确性，此字段必须排在第一位
    value: UnsafeCell<MaybeUninit<T>>,
    state: Cell<State>,
}

impl<T> LazyStorage<T> {
    pub const fn new() -> LazyStorage<T> {
        LazyStorage {
            value: UnsafeCell::new(MaybeUninit::uninit()),
            state: Cell::new(State::Initial),
        }
    }

    /// 获取一个指向 TLS 值的指针，必要时用所提供的参数对其进行初始化。
    ///
    /// 在发生重入式（reentrant）初始化之后，所得到的指针不可再使用。
    #[inline]
    pub fn get(&'static self, i: Option<&mut Option<T>>, f: impl FnOnce() -> T) -> *const T {
        if self.state.get() == State::Alive {
            self.value.get() as *const T
        } else {
            self.initialize(i, f)
        }
    }

    #[cold]
    fn initialize(&'static self, i: Option<&mut Option<T>>, f: impl FnOnce() -> T) -> *const T {
        let value = i.and_then(Option::take).unwrap_or_else(f);

        // 如果旧值已被初始化，则销毁它
        // FIXME(#110897)：也许应当在递归初始化时 panic。
        if self.state.get() == State::Alive {
            self.state.set(State::Destroying);
            // Safety：下面我们会检查 drop 期间没有发生初始化
            unsafe {
                ptr::drop_in_place(self.value.get() as *mut T);
            }
            self.state.set(State::Initial);
        }

        // 防范 drop 期间发生初始化
        if self.state.get() == State::Destroying {
            panic!("Attempted to initialize thread-local while it is being dropped");
        }

        unsafe {
            self.value.get().write(MaybeUninit::new(value));
        }
        self.state.set(State::Alive);

        self.value.get() as *const T
    }
}

// SAFETY：该 target 没有线程。
unsafe impl<T> Sync for LazyStorage<T> {}

#[rustc_macro_transparency = "semiopaque"]
pub(crate) macro local_pointer {
    () => {},
    ($vis:vis static $name:ident; $($rest:tt)*) => {
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

// SAFETY：该 target 没有线程。
unsafe impl Sync for LocalPointer {}
