use super::key::{Key, LazyKey, get, set};
use super::{abort_on_dtor_unwind, guard};
use crate::alloc::{self, GlobalAlloc, Layout, System};
use crate::cell::Cell;
use crate::marker::PhantomData;
use crate::mem::ManuallyDrop;
use crate::ops::Deref;
use crate::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use crate::ptr::{self, NonNull};

#[doc(hidden)]
#[allow_internal_unstable(thread_local_internals)]
#[allow_internal_unsafe]
#[unstable(feature = "thread_local_internals", issue = "none")]
#[rustc_macro_transparency = "semiopaque"]
pub macro thread_local_inner {
    // 注意：我们不能用 `use` 来导入 `Storage` 或 `LocalKey`，因为那可能会遮蔽
    // （shadow）用户提供的同名类型或类型别名。如果这些类型被重命名，请更新
    // `tests/thread.rs` 中的遮蔽（shadowing）测试。

    // 用于为 `thread_local!` 生成 `LocalKey` 值。
    (@key $t:ty, $($(#[$($align_attr:tt)*])+)?, $init:expr) => {{
        #[inline]
        fn __rust_std_internal_init_fn() -> $t { $init }

        // 注意：这里不能用 `use` 来导入 `LocalKey` 或 `Storage`，因为那可能会遮蔽
        // 用户提供的同名类型或类型别名。如果这些类型被重命名，请更新
        // `tests/thread.rs` 中的遮蔽（shadowing）测试。
        unsafe {
            $crate::thread::LocalKey::new(|__rust_std_internal_init| {
                static __RUST_STD_INTERNAL_VAL: $crate::thread::local_impl::Storage<$t, {
                    $({
                        // 确保各属性具有有效语法，且相应的 feature gate 已启用
                        $(#[$($align_attr)*])+
                        #[allow(unused)]
                        static DUMMY: () = ();
                    })?

                    #[allow(unused_mut)]
                    let mut final_align = $crate::thread::local_impl::value_align::<$t>();
                    $($($crate::thread::local_impl::thread_local_inner!(@align final_align, $($align_attr)*);)+)?
                    final_align
                }>
                    = $crate::thread::local_impl::Storage::new();
                __RUST_STD_INTERNAL_VAL.get(__rust_std_internal_init, __rust_std_internal_init_fn)
            })
        }
    }},

    // 处理单个 `rustc_align_static` 属性
    (@align $final_align:ident, rustc_align_static($($align:tt)*) $(, $($attr_rest:tt)+)?) => {
        let new_align: $crate::primitive::usize = $($align)*;
        if new_align > $final_align {
            $final_align = new_align;
        }

        $($crate::thread::local_impl::thread_local_inner!(@align $final_align, $($attr_rest)+);)?
    },

    // 处理单个 `cfg_attr` 属性：把它翻译成一个带 `cfg` 的块然后递归处理。
    // https://doc.rust-lang.org/reference/conditional-compilation.html#railroad-ConfigurationPredicate

    (@align $final_align:ident, cfg_attr(true, $($cfg_rhs:tt)*) $(, $($attr_rest:tt)+)?) => {
        #[cfg(true)]
        {
            $crate::thread::local_impl::thread_local_inner!(@align $final_align, $($cfg_rhs)*);
        }

        $($crate::thread::local_impl::thread_local_inner!(@align $final_align, $($attr_rest)+);)?
    },

    (@align $final_align:ident, cfg_attr(false, $($cfg_rhs:tt)*) $(, $($attr_rest:tt)+)?) => {
        #[cfg(false)]
        {
            $crate::thread::local_impl::thread_local_inner!(@align $final_align, $($cfg_rhs)*);
        }

        $($crate::thread::local_impl::thread_local_inner!(@align $final_align, $($attr_rest)+);)?
    },

    (@align $final_align:ident, cfg_attr($cfg_pred:meta, $($cfg_rhs:tt)*) $(, $($attr_rest:tt)+)?) => {
        #[cfg($cfg_pred)]
        {
            $crate::thread::local_impl::thread_local_inner!(@align $final_align, $($cfg_rhs)*);
        }

        $($crate::thread::local_impl::thread_local_inner!(@align $final_align, $($attr_rest)+);)?
    },
}

/// 使用一个常规的全局 static 来存储此 key；它所提供的状态因而是线程本地的。
/// 不变量(INVARIANT)：ALIGN 必须是一个有效的对齐值，且不小于 `value_align::<T>`。
#[allow(missing_debug_implementations)]
pub struct Storage<T, const ALIGN: usize> {
    key: LazyKey,
    marker: PhantomData<Cell<T>>,
}

unsafe impl<T, const ALIGN: usize> Sync for Storage<T, ALIGN> {}

#[repr(C)]
struct Value<T: 'static> {
    // 为了 `#[rustc_align_static]` 的正确性，此字段必须排在第一位
    value: T,
    // 不变量(INVARIANT)：如果此值被存储在某个 TLS key 下，则 `key` 必须就是那个 `key`。
    key: Key,
}

pub const fn value_align<T: 'static>() -> usize {
    crate::mem::align_of::<Value<T>>()
}

/// 等价于 `Box<Value<T>, System>`，但可能是超对齐（over-aligned）的。
struct AlignedSystemBox<T: 'static, const ALIGN: usize> {
    ptr: NonNull<Value<T>>,
}

impl<T: 'static, const ALIGN: usize> AlignedSystemBox<T, ALIGN> {
    #[inline]
    fn new(v: Value<T>) -> Self {
        let layout = Layout::new::<Value<T>>().align_to(ALIGN).unwrap();

        // 我们这里使用 System 分配器，以避免干扰某个可能使用线程本地存储的
        // Global 分配器。
        let ptr: *mut Value<T> = (unsafe { System.alloc(layout) }).cast();
        let Some(ptr) = NonNull::new(ptr) else {
            alloc::handle_alloc_error(layout);
        };
        unsafe { ptr.write(v) };
        Self { ptr }
    }

    #[inline]
    fn into_raw(b: Self) -> *mut Value<T> {
        let md = ManuallyDrop::new(b);
        md.ptr.as_ptr()
    }

    #[inline]
    unsafe fn from_raw(ptr: *mut Value<T>) -> Self {
        Self { ptr: unsafe { NonNull::new_unchecked(ptr) } }
    }
}

impl<T: 'static, const ALIGN: usize> Deref for AlignedSystemBox<T, ALIGN> {
    type Target = Value<T>;

    #[inline]
    fn deref(&self) -> &Self::Target {
        unsafe { &*(self.ptr.as_ptr()) }
    }
}

impl<T: 'static, const ALIGN: usize> Drop for AlignedSystemBox<T, ALIGN> {
    #[inline]
    fn drop(&mut self) {
        let layout = Layout::new::<Value<T>>().align_to(ALIGN).unwrap();

        unsafe {
            let unwind_result = catch_unwind(AssertUnwindSafe(|| self.ptr.drop_in_place()));
            System.dealloc(self.ptr.as_ptr().cast(), layout);
            if let Err(payload) = unwind_result {
                resume_unwind(payload);
            }
        }
    }
}

impl<T: 'static, const ALIGN: usize> Storage<T, ALIGN> {
    pub const fn new() -> Storage<T, ALIGN> {
        Storage { key: LazyKey::new(Some(destroy_value::<T, ALIGN>)), marker: PhantomData }
    }

    /// 获取一个指向 TLS 值的指针，必要时用所提供的参数对其进行初始化。
    /// 如果该 TLS 变量已被销毁，则返回空指针。
    ///
    /// 在发生重入式（reentrant）初始化或线程销毁之后，所得到的指针不可再使用。
    pub fn get(&'static self, i: Option<&mut Option<T>>, f: impl FnOnce() -> T) -> *const T {
        let key = self.key.force();
        let ptr = unsafe { get(key) as *mut Value<T> };
        if ptr.addr() > 1 {
            // SAFETY：该检查确保了指针是安全的（它的析构函数没有在运行），
            // 并且它来自一个可信来源（self）。
            unsafe { &(*ptr).value }
        } else {
            // SAFETY：显然正确。
            unsafe { Self::try_initialize(key, ptr, i, f) }
        }
    }

    /// # 安全性(Safety）
    /// * `key` 必须是调用 `self.key.force()` 的结果
    /// * `ptr` 必须是当前与 `key` 关联的值。
    unsafe fn try_initialize(
        key: Key,
        ptr: *mut Value<T>,
        i: Option<&mut Option<T>>,
        f: impl FnOnce() -> T,
    ) -> *const T {
        if ptr.addr() == 1 {
            // 析构函数正在运行
            return ptr::null();
        }

        let value = AlignedSystemBox::<T, ALIGN>::new(Value {
            value: i.and_then(Option::take).unwrap_or_else(f),
            key,
        });
        let ptr = AlignedSystemBox::into_raw(value);

        // SAFETY：
        // * key 来自一个 `LazyKey`，因此是正确的。
        // * `ptr` 是一个正确的指针，可被该 key 的析构函数销毁。
        // * 该值被存储在它自身所含有的 key 之下。
        let old = unsafe {
            let old = get(key) as *mut Value<T>;
            set(key, ptr as *mut u8);
            old
        };

        if !old.is_null() {
            // 如果该变量曾被递归初始化，则 drop 旧值。
            // SAFETY：我们不可能处于某个 `LocalKey::with` 作用域内部，因为初始化器
            // 已经返回，而下一个作用域只会在我们返回该指针之后才开始。因此，
            // 不可能存在指向旧值的引用。
            drop(unsafe { AlignedSystemBox::<T, ALIGN>::from_raw(old) });
        }

        // SAFETY：我们刚刚在上面创建了这个值。
        unsafe { &(*ptr).value }
    }
}

unsafe extern "C" fn destroy_value<T: 'static, const ALIGN: usize>(ptr: *mut u8) {
    // SAFETY：
    //
    // 当此析构函数开始运行时，OS 的 TLS 保证此 key 中含有一个空值。
    // 我们把它重新设置为哨兵值 1，以确保本线程未来对此 key 的任何 `get`
    // 调用都将返回 `None`。
    //
    // 注意，为了防止无限循环，我们在自己从析构函数返回之前会把它重新设回 null。
    abort_on_dtor_unwind(|| {
        let ptr = unsafe { AlignedSystemBox::<T, ALIGN>::from_raw(ptr as *mut Value<T>) };
        let key = ptr.key;
        // SAFETY：`key` 就是 `ptr` 被存储于其下的那个 TLS key。
        unsafe { set(key, ptr::without_provenance_mut(1)) };
        drop(ptr);
        // SAFETY：`key` 就是 `ptr` 被存储于其下的那个 TLS key。
        unsafe { set(key, ptr::null_mut()) };
        // 确保运行时清理会在下一轮 TLS 析构之后被执行。
        guard::enable();
    });
}

#[rustc_macro_transparency = "semiopaque"]
pub(crate) macro local_pointer {
    () => {},
    ($vis:vis static $name:ident; $($rest:tt)*) => {
        $vis static $name: $crate::sys::thread_local::LocalPointer = $crate::sys::thread_local::LocalPointer::__new();
        $crate::sys::thread_local::local_pointer! { $($rest)* }
    },
}

pub(crate) struct LocalPointer {
    key: LazyKey,
}

impl LocalPointer {
    pub const fn __new() -> LocalPointer {
        LocalPointer { key: LazyKey::new(None) }
    }

    pub fn get(&'static self) -> *mut () {
        unsafe { get(self.key.force()) as *mut () }
    }

    pub fn set(&'static self, p: *mut ()) {
        unsafe { set(self.key.force(), p as *mut u8) }
    }
}
