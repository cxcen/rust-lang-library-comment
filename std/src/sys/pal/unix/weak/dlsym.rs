use crate::ffi::{CStr, c_char, c_void};
use crate::marker::{FnPtr, PhantomData};
use crate::sync::atomic::{Atomic, AtomicPtr, Ordering};
use crate::{mem, ptr};

#[cfg(test)]
#[path = "./tests.rs"]
mod tests;

pub(crate) macro weak {
    (fn $name:ident($($param:ident : $t:ty),* $(,)?) -> $ret:ty;) => (
        static DLSYM: DlsymWeak<unsafe extern "C" fn($($t),*) -> $ret> = {
            let Ok(name) = CStr::from_bytes_with_nul(concat!(stringify!($name), '\0').as_bytes()) else {
                panic!("symbol name may not contain NUL")
            };

            // SAFETY: 调用 `get()` 所返回的函数指针的人，有责任确保该签名是正确
            // 的。和 extern 块一样，这一点通过把函数指针声明为 unsafe，在语法层面
            // 上强制要求调用方履行。
            unsafe { DlsymWeak::new(name) }
        };

        let $name = &DLSYM;
    )
}

pub(crate) struct DlsymWeak<F> {
    /// 指向该符号以 nul 结尾的名字的指针。
    // 使用裸指针而非 `&'static CStr`，以节省空间。
    name: *const c_char,
    func: Atomic<*mut libc::c_void>,
    _marker: PhantomData<F>,
}

impl<F: FnPtr> DlsymWeak<F> {
    /// # 安全性(Safety）
    ///
    /// 如果 `F` 的签名与该符号（若其存在）的签名不一致，那么调用 `get()` 所返回
    /// 的函数指针即为未定义行为。
    pub const unsafe fn new(name: &'static CStr) -> Self {
        DlsymWeak {
            name: name.as_ptr(),
            func: AtomicPtr::new(ptr::without_provenance_mut(1)),
            _marker: PhantomData,
        }
    }

    #[inline]
    pub fn get(&self) -> Option<F> {
        // 调用方接下来大概率会通过这个值进行读取（即调用我们 dlsym 得到的那个
        // 函数）。这意味着我们需要至少以 C11 的 consume 序来加载它，才能保证我们
        // 从该指针读出的数据不是在该指针被存入之前的旧数据。Rust 没有与
        // memory_order_consume 等价的东西，所以我们使用 acquire 加载（抱歉了，
        // ARM）。
        //
        // 不过，实践中即便在 relaxed 与 consume 含义不同的 CPU 上，这大概也并非
        // 必需。我们要加载的这些符号很可能在初始化时就已经（要么存在、要么不
        // 存在）确定下来了；而且即便它们当时还没确定，运行时的动态加载器内部也极
        // 有可能已经具备了足够的内存屏障（可能是隐式的，例如调用 `mprotect` 所
        // 提供的那些）。
        //
        // 话虽如此，这些都不是*有保证的*，所以我们用 acquire。
        match self.func.load(Ordering::Acquire) {
            func if func.addr() == 1 => self.initialize(),
            func if func.is_null() => None,
            // SAFETY:
            // `func` 非空，且 `F` 实现了 `FnPtr`，因此这次 transmute 是良定义
            // 的。创建该 `DlsymWeak` 的人有责任确保调用最终得到的函数指针不会
            // 导致未定义行为（不过 `weak!` 宏通过使用 `unsafe` 函数指针，把这一
            // 责任转交给了该函数的调用方）。
            // FIXME: 一旦 `transmute` 不再对泛型报错，就改用它。
            func => Some(unsafe { mem::transmute_copy::<*mut c_void, F>(&func) }),
        }
    }

    // 标记为 cold，因为它只应在首次初始化时发生。
    #[cold]
    fn initialize(&self) -> Option<F> {
        // SAFETY: `self.name` 由一个 `&'static CStr` 创建而来，因此是一个有效的
        // C 字符串指针。
        let val = unsafe { libc::dlsym(libc::RTLD_DEFAULT, self.name) };
        // 这与 `get` 中的 acquire 加载形成同步关系。
        self.func.store(val, Ordering::Release);

        if val.is_null() {
            None
        } else {
            // SAFETY: 参见 `get` 中的注释。
            // FIXME: 一旦 `transmute` 不再对泛型报错，就改用它。
            Some(unsafe { mem::transmute_copy::<*mut libc::c_void, F>(&val) })
        }
    }
}

unsafe impl<F> Send for DlsymWeak<F> {}
unsafe impl<F> Sync for DlsymWeak<F> {}
