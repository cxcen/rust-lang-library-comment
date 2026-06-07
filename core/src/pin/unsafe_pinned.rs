use crate::cell::UnsafeCell;
use crate::marker::Unpin;
use crate::ops::{CoerceUnsized, DispatchFromDyn};
use crate::pin::Pin;
use crate::{fmt, ptr};

/// 此类型提供了一种完全 opt-out（退出）典型别名（aliasing）规则的方式；具体而言，
/// `&mut UnsafePinned<T>` 不保证是一个唯一指针。它还涵盖了 `UnsafeCell` 的效果，即
/// `&UnsafePinned<T>` 可能指向正在被修改的数据。
///
/// 然而，即便你把你的类型定义成 `pub struct Wrapper(UnsafePinned<...>)`，让一个 `&mut Wrapper`
/// 与任何其他东西形成别名仍然是非常危险的。许多在 `&mut T` 上泛型工作的函数都假定存储 `T` 的那块
/// 内存是被唯一拥有的（例如 `mem::swap`）。换句话说，虽然让 `&mut Wrapper` 形成别名不会立即构成
/// 未定义行为，但把这样一个可变引用暴露给你无法控制的代码仍然是不健全的！要确保健全性，需要诸如
/// 通过 [`Pin`] 进行固定之类的技术。
///
/// 与 [`UnsafeCell`](crate::cell::UnsafeCell) 类似，`UnsafePinned` 通常不会出现在某个库的公开
/// API 中。它是那些需要支持别名可变引用的库的内部实现细节。
///
/// 此类型像 `UnsafeCell` 一样阻止壁龛（niche）。
#[lang = "unsafe_pinned"]
#[repr(transparent)]
#[unstable(feature = "unsafe_pinned", issue = "125735")]
pub struct UnsafePinned<T: ?Sized> {
    value: UnsafeCell<T>,
}

// 覆盖（override）`UnsafeCell` 中那个手动的 `!Sync`。
#[unstable(feature = "unsafe_pinned", issue = "125735")]
unsafe impl<T: ?Sized + Sync> Sync for UnsafePinned<T> {}

/// 当此类型被使用时，几乎可以肯定这意味着安全 API 需要使用固定，以避免那些别名失效。因此让我们
/// 把它标记为 `!Unpin`。只要你的 API 在未固定时仍然是健全的，你总是可以用一个 `impl` 块来重新
/// opt-in（加入）`Unpin`。
#[unstable(feature = "unsafe_pinned", issue = "125735")]
impl<T: ?Sized> !Unpin for UnsafePinned<T> {}

// `Send` 和 `Sync` 从 `T` 继承。这与 `SyncUnsafeCell` 类似，因为我们最终得出结论：`UnsafeCell`
// 隐式地让东西变成 `!Sync` 有时在人体工程学上很糟糕。一个需要 `!Send`/`!Sync` 的类型，真正应当
// 自己显式地 opt-out，例如通过一个 `PhantomData<*mut T>`，或者（将来某天）通过
// `impl !Send`/`impl !Sync`。

impl<T> UnsafePinned<T> {
    /// 构造一个新的 `UnsafePinned` 实例，它将包装指定的值。
    ///
    /// 所有通过 `&UnsafePinned<T>`、`&mut UnsafePinned<T>` 或 `Pin<&mut UnsafePinned<T>>` 对内部值
    /// 的访问都需要 `unsafe` 代码。
    #[inline(always)]
    #[must_use]
    #[unstable(feature = "unsafe_pinned", issue = "125735")]
    pub const fn new(value: T) -> Self {
        UnsafePinned { value: UnsafeCell::new(value) }
    }

    /// 解包（unwrap）该值，消耗这个 `UnsafePinned`。
    #[inline(always)]
    #[must_use]
    #[unstable(feature = "unsafe_pinned", issue = "125735")]
    #[rustc_allow_const_fn_unstable(const_precise_live_drops)]
    pub const fn into_inner(self) -> T {
        self.value.into_inner()
    }
}

impl<T: ?Sized> UnsafePinned<T> {
    /// 获取对一个被固定的 `UnsafePinned` 内容的读写访问。
    #[inline(always)]
    #[must_use]
    #[unstable(feature = "unsafe_pinned", issue = "125735")]
    pub const fn get_mut_pinned(self: Pin<&mut Self>) -> *mut T {
        // SAFETY: 我们并没有用 `get_unchecked_mut` 来 unpin 任何东西
        unsafe { self.get_unchecked_mut() }.get_mut_unchecked()
    }

    /// 获取对一个 `UnsafePinned` 内容的读写访问。
    ///
    /// 你通常应当改用 `get_mut_pinned`，以显式地追踪“由于存在别名，这块内存是‘被固定的’”这一事实。
    #[inline(always)]
    #[must_use]
    #[unstable(feature = "unsafe_pinned", issue = "125735")]
    pub const fn get_mut_unchecked(&mut self) -> *mut T {
        ptr::from_mut(self) as *mut T
    }

    /// 获取对一个共享的 `UnsafePinned` 内容的可变访问。
    ///
    /// 这可以被转换（cast）为任意种类的指针。在创建引用时，你必须维护别名规则；更多讨论与注意事项
    /// 参见 [`UnsafeCell`]。
    ///
    /// [`UnsafeCell`]: crate::cell::UnsafeCell#aliasing-rules
    ///
    /// ```rust,no_run
    /// #![feature(unsafe_pinned)]
    /// use std::pin::UnsafePinned;
    ///
    /// unsafe {
    ///     let mut x = UnsafePinned::new(0);
    ///     let ptr = x.get();
    ///     x.get_mut_unchecked().write(1);
    ///     assert_eq!(ptr.read(), 1);
    /// }
    /// ```
    #[inline(always)]
    #[must_use]
    #[unstable(feature = "unsafe_pinned", issue = "125735")]
    pub const fn get(&self) -> *mut T {
        self.value.get()
    }

    /// 获取一个指向被包装值的不可变指针。
    ///
    /// 与 [`get`] 的区别在于，此函数接受一个裸指针，这对于避免创建临时引用很有用。
    ///
    /// [`get`]: UnsafePinned::get
    #[inline(always)]
    #[must_use]
    #[unstable(feature = "unsafe_pinned", issue = "125735")]
    pub const fn raw_get(this: *const Self) -> *mut T {
        this as *const T as *mut T
    }

    /// 获取一个指向被包装值的可变指针。
    ///
    /// 与 [`get_mut_pinned`] 和 [`get_mut_unchecked`] 的区别在于，此函数接受一个裸指针，这对于避免
    /// 创建临时引用很有用。
    ///
    /// [`get_mut_pinned`]: UnsafePinned::get_mut_pinned
    /// [`get_mut_unchecked`]: UnsafePinned::get_mut_unchecked
    #[inline(always)]
    #[must_use]
    #[unstable(feature = "unsafe_pinned", issue = "125735")]
    pub const fn raw_get_mut(this: *mut Self) -> *mut T {
        this as *mut T
    }
}

#[unstable(feature = "unsafe_pinned", issue = "125735")]
impl<T: Default> Default for UnsafePinned<T> {
    /// 用 T 的 `Default` 值创建一个 `UnsafePinned`。
    fn default() -> Self {
        UnsafePinned::new(T::default())
    }
}

#[unstable(feature = "unsafe_pinned", issue = "125735")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl<T> const From<T> for UnsafePinned<T> {
    /// 创建一个包含给定值的新 `UnsafePinned<T>`。
    fn from(value: T) -> Self {
        UnsafePinned::new(value)
    }
}

#[unstable(feature = "unsafe_pinned", issue = "125735")]
impl<T: ?Sized> fmt::Debug for UnsafePinned<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UnsafePinned").finish_non_exhaustive()
    }
}

#[unstable(feature = "coerce_unsized", issue = "18598")]
// #[unstable(feature = "unsafe_pinned", issue = "125735")]
impl<T: CoerceUnsized<U>, U> CoerceUnsized<UnsafePinned<U>> for UnsafePinned<T> {}

// 允许那些包装了 `UnsafePinned` 的类型也实现 `DispatchFromDyn`，从而成为 dyn 兼容的方法接收者
// （receiver）。
// 注意，目前 `UnsafePinned` 本身还不能作为方法接收者，因为它没有实现 Deref。
// 换句话说：
// `self: UnsafePinned<&Self>` 不行
// `self: UnsafePinned<Self>` 变得可行
// FIXME(unsafe_pinned) 这段逻辑是从 UnsafeCell 复制来的，它现在还健全吗？
#[unstable(feature = "dispatch_from_dyn", issue = "none")]
// #[unstable(feature = "unsafe_pinned", issue = "125735")]
impl<T: DispatchFromDyn<U>, U> DispatchFromDyn<UnsafePinned<U>> for UnsafePinned<T> {}

// FIXME(unsafe_pinned): 为 UnsafePinned<T> 实现 PinCoerceUnsized？
