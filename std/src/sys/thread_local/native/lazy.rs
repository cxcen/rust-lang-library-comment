use crate::cell::{Cell, UnsafeCell};
use crate::mem::MaybeUninit;
use crate::ptr;
use crate::sys::thread_local::{abort_on_dtor_unwind, destructors};

pub unsafe trait DestroyedState: Sized + Copy {
    fn register_dtor<T>(s: &Storage<T, Self>);
}

unsafe impl DestroyedState for ! {
    fn register_dtor<T>(_: &Storage<T, !>) {}
}

unsafe impl DestroyedState for () {
    fn register_dtor<T>(s: &Storage<T, ()>) {
        unsafe {
            destructors::register(ptr::from_ref(s).cast_mut().cast(), destroy::<T>);
        }
    }
}

#[derive(Copy, Clone)]
enum State<D> {
    Uninitialized,
    Alive,
    Destroyed(D),
}

#[allow(missing_debug_implementations)]
#[repr(C)]
pub struct Storage<T, D> {
    // 为了 `#[rustc_align_static]` 的正确性，此字段必须排在第一位
    value: UnsafeCell<MaybeUninit<T>>,
    state: Cell<State<D>>,
}

impl<T, D> Storage<T, D>
where
    D: DestroyedState,
{
    pub const fn new() -> Storage<T, D> {
        Storage {
            state: Cell::new(State::Uninitialized),
            value: UnsafeCell::new(MaybeUninit::uninit()),
        }
    }

    /// 获取一个指向 TLS 值的指针，必要时用所提供的参数对其进行初始化。
    /// 如果该 TLS 变量已被销毁，则返回空指针。
    ///
    /// 在发生重入式（reentrant）初始化或线程销毁之后，所得到的指针不可再使用。
    ///
    /// # 安全性(Safety）
    /// `self` 引用必须在 TLS 析构函数运行之前一直保持有效。
    #[inline]
    pub unsafe fn get_or_init(&self, i: Option<&mut Option<T>>, f: impl FnOnce() -> T) -> *const T {
        if let State::Alive = self.state.get() {
            self.value.get().cast()
        } else {
            unsafe { self.get_or_init_slow(i, f) }
        }
    }

    /// # 安全性(Safety）
    /// `self` 引用必须在 TLS 析构函数运行之前一直保持有效。
    #[cold]
    unsafe fn get_or_init_slow(
        &self,
        i: Option<&mut Option<T>>,
        f: impl FnOnce() -> T,
    ) -> *const T {
        match self.state.get() {
            State::Uninitialized => {}
            State::Alive => return self.value.get().cast(),
            State::Destroyed(_) => return ptr::null(),
        }

        let v = i.and_then(Option::take).unwrap_or_else(f);

        // SAFETY：我们不可能处于某个 `LocalKey::with` 作用域内部，因为初始化器
        // 已经返回，而下一个作用域只会在我们返回该指针之后才开始。因此，即使旧值
        // 曾被初始化，也不可能存在指向它的引用。于是，由于我们是 !Sync 的，
        // 我们对 self.value 拥有独占访问权，可以替换它。
        let mut old_value = unsafe { self.value.get().replace(MaybeUninit::new(v)) };
        match self.state.replace(State::Alive) {
            // 如果该变量不是正在被递归初始化，则注册析构函数。如果该值不需要
            // 销毁，这可能是个 noop（空操作）。
            State::Uninitialized => D::register_dtor(self),

            // 递归初始化的情形，我们只需要 drop 旧值，因为析构函数已经注册过了。
            State::Alive => unsafe { old_value.assume_init_drop() },

            State::Destroyed(_) => unreachable!(),
        }

        self.value.get().cast()
    }
}

/// 把一个处于 `Alive` 状态的 TLS 变量转移到 `Destroyed` 状态，并 drop 它的值。
///
/// # 安全性(Safety）
/// * 只能在线程销毁时调用。
/// * `ptr` 必须指向一个 `Storage<T, ()>` 实例，且对访问该实例有效。
unsafe extern "C" fn destroy<T>(ptr: *mut u8) {
    // 如果发生 panic，则打印一条友好的 abort 消息。
    abort_on_dtor_unwind(|| {
        let storage = unsafe { &*(ptr as *const Storage<T, ()>) };
        if let State::Alive = storage.state.replace(State::Destroyed(())) {
            // SAFETY：我们已确认状态曾为 Alive，所以该值是已初始化的。
            // 我们还把状态更新为 Destroyed，以防止析构函数访问该线程本地变量，
            // 因为那会违反 Drop::drop 中 &mut T 所提供的独占访问。
            unsafe { (*storage.value.get()).assume_init_drop() }
        }
    })
}
