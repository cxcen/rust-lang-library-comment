use crate::cell::{Cell, UnsafeCell};
use crate::ptr::{self, drop_in_place};
use crate::sys::thread_local::{abort_on_dtor_unwind, destructors};

#[derive(Clone, Copy)]
enum State {
    Initial,
    Alive,
    Destroyed,
}

#[allow(missing_debug_implementations)]
#[repr(C)]
pub struct Storage<T> {
    // 为了 `#[rustc_align_static]` 的正确性，此字段必须排在第一位
    val: UnsafeCell<T>,
    state: Cell<State>,
}

impl<T> Storage<T> {
    pub const fn new(val: T) -> Storage<T> {
        Storage { state: Cell::new(State::Initial), val: UnsafeCell::new(val) }
    }

    /// 获取一个指向 TLS 值的指针。如果该 TLS 变量已被销毁，则返回空指针。
    ///
    /// 在线程销毁发生之后，所得到的指针不可再使用。
    ///
    /// # 安全性(Safety）
    /// `self` 引用必须在 TLS 析构函数运行之前一直保持有效。
    #[inline]
    pub unsafe fn get(&self) -> *const T {
        match self.state.get() {
            State::Alive => self.val.get(),
            State::Destroyed => ptr::null(),
            State::Initial => unsafe { self.initialize() },
        }
    }

    #[cold]
    unsafe fn initialize(&self) -> *const T {
        // 注册析构函数

        // SAFETY：
        // 调用方保证 `self` 在线程销毁之前会一直有效。
        unsafe {
            destructors::register(ptr::from_ref(self).cast_mut().cast(), destroy::<T>);
        }

        self.state.set(State::Alive);
        self.val.get()
    }
}

/// 把一个处于 `Alive` 状态的 TLS 变量转移到 `Destroyed` 状态，并 drop 它的值。
///
/// # 安全性(Safety）
/// * 只能在线程销毁时调用。
/// * `ptr` 必须指向一个处于 `Alive` 状态的 `Storage` 实例，且对访问该实例有效。
unsafe extern "C" fn destroy<T>(ptr: *mut u8) {
    // 如果发生 panic，则打印一条友好的 abort 消息。
    abort_on_dtor_unwind(|| {
        let storage = unsafe { &*(ptr as *const Storage<T>) };
        // 在运行析构函数之前先更新状态，因为析构函数可能会尝试访问该变量。
        storage.state.set(State::Destroyed);
        unsafe {
            drop_in_place(storage.val.get());
        }
    })
}
