use crate::clone::TrivialClone;
use crate::fmt;
use crate::marker::{PhantomData, PointeeSized, Unsize};
use crate::ops::{CoerceUnsized, DispatchFromDyn};
use crate::pin::PinCoerceUnsized;
use crate::ptr::NonNull;

/// 对裸的、非空 `*mut T` 的包装器,表示该包装器的持有者**拥有**(own)其
/// 所指向的对象(referent)。用于构建 `Box<T>`、`Vec<T>`、`String`、
/// `HashMap<K, V>` 这类抽象的内部实现。
///
/// 与 `*mut T` 不同,`Unique<T>` 在语义上"仿佛"(as if)它本身就是一个
/// `T` 的实例:
/// - 当 `T` 满足 `Send`/`Sync` 时,它也实现 `Send`/`Sync`(裸指针默认既不
///   `Send` 也不 `Sync`)。
/// - 它还隐含了一个 `T` 实例所能期望的那种强别名(aliasing)保证:除非通过
///   指向该 `Unique` 的唯一路径,否则不应修改指针所指向的对象。这是一种独占
///   所有权语义——同一时刻逻辑上只存在一条访问该内存的通路。
///
/// 如果你不确定在自己的场景中使用 `Unique` 是否正确,请改用 `NonNull`,它的
/// 语义更弱(不附带上述独占别名保证),因而更难用错。
///
/// 与 `*mut T` 不同,该指针必须**始终非空**,即便它从不被解引用。这样做是为了
/// 让枚举可以把这个被禁止的取值(空指针)用作判别值(discriminant,即 niche
/// 优化)——因此 `Option<Unique<T>>` 与 `Unique<T>` 大小相同。不过,只要不解
/// 引用,该指针仍然可以悬垂(dangle)。
///
/// 与 `*mut T` 不同,`Unique<T>` 对 `T` 是**协变的**(covariant)。对于任何
/// 维护了 Unique 别名要求的类型,协变都应当是正确的。
#[unstable(
    feature = "ptr_internals",
    issue = "none",
    reason = "use `NonNull` instead and consider `PhantomData<T>` \
              (if you also use `#[may_dangle]`), `Send`, and/or `Sync`"
)]
#[doc(hidden)]
#[repr(transparent)]
pub struct Unique<T: PointeeSized> {
    pointer: NonNull<T>,
    // 注意:此标记对协变性没有影响,但它对于 dropck(drop 检查)而言是必要的,
    // 它让编译器理解我们在逻辑上拥有一个 `T`。
    //
    // 详情参见:
    // https://github.com/rust-lang/rfcs/blob/master/text/0769-sound-generic-drop.md#phantom-data
    _marker: PhantomData<T>,
}

/// 当 `T` 是 `Send` 时,`Unique` 指针也是 `Send`,因为它们所引用的数据是
/// 无别名的(unaliased,独占)。注意:这条别名不变量并不由类型系统强制保证;
/// 使用 `Unique` 的抽象必须自行维护它。
#[unstable(feature = "ptr_internals", issue = "none")]
unsafe impl<T: Send + PointeeSized> Send for Unique<T> {}

/// 当 `T` 是 `Sync` 时,`Unique` 指针也是 `Sync`,因为它们所引用的数据是
/// 无别名的(unaliased,独占)。注意:这条别名不变量并不由类型系统强制保证;
/// 使用 `Unique` 的抽象必须自行维护它。
#[unstable(feature = "ptr_internals", issue = "none")]
unsafe impl<T: Sync + PointeeSized> Sync for Unique<T> {}

#[unstable(feature = "ptr_internals", issue = "none")]
impl<T: Sized> Unique<T> {
    /// 创建一个新的、悬垂(dangling)但已正确对齐(well-aligned)的 `Unique`。
    ///
    /// 这在初始化那些惰性分配(lazily allocate)的类型时很有用,例如
    /// `Vec::new` 就是这么做的:此时还没有真正分配内存,但需要一个非空且对齐的
    /// 指针占位。
    ///
    /// 注意:返回指针的地址有可能恰好与某个有效指针的地址相同,这意味着它**不能**
    /// 被当作"尚未初始化"的哨兵值(sentinel value)来使用。惰性分配的类型必须
    /// 通过其他手段来追踪是否已初始化。
    #[must_use]
    #[inline]
    pub const fn dangling() -> Self {
        // FIXME(const-hack): 替换为 `From`。
        Unique { pointer: NonNull::dangling(), _marker: PhantomData }
    }
}

#[unstable(feature = "ptr_internals", issue = "none")]
impl<T: PointeeSized> Unique<T> {
    /// 创建一个新的 `Unique`。
    ///
    /// # 安全性(Safety）
    ///
    /// 调用方必须保证:`ptr` 非空。其余的别名/有效性不变量虽然不在此处强制,
    /// 但使用 `Unique` 的抽象有义务维护(见类型级文档中的独占别名语义)。
    #[inline]
    pub const unsafe fn new_unchecked(ptr: *mut T) -> Self {
        // SAFETY: 调用方必须保证 `ptr` 非空。
        unsafe { Unique { pointer: NonNull::new_unchecked(ptr), _marker: PhantomData } }
    }

    /// 当 `ptr` 非空时创建一个新的 `Unique`;若为空则返回 `None`。
    #[inline]
    pub const fn new(ptr: *mut T) -> Option<Self> {
        if let Some(pointer) = NonNull::new(ptr) {
            Some(Unique { pointer, _marker: PhantomData })
        } else {
            None
        }
    }

    /// 在 const 上下文中,从一个 `NonNull` 创建新的 `Unique`。
    #[inline]
    pub const fn from_non_null(pointer: NonNull<T>) -> Self {
        Unique { pointer, _marker: PhantomData }
    }

    /// 取出底层的 `*mut` 裸指针。
    #[must_use = "`self` will be dropped if the result is not used"]
    #[inline]
    pub const fn as_ptr(self) -> *mut T {
        self.pointer.as_ptr()
    }

    /// 取出底层的 `*mut` 指针,以 `NonNull` 形式返回。
    #[must_use = "`self` will be dropped if the result is not used"]
    #[inline]
    pub const fn as_non_null_ptr(self) -> NonNull<T> {
        self.pointer
    }

    /// 解引用其内容,返回共享引用。
    ///
    /// 返回引用的生命周期被绑定到 `self`,因此其行为"仿佛"(as if)真的是在借用
    /// 一个 `T` 的实例。如果需要一个更长(未绑定)的生命周期,请改用
    /// `&*my_ptr.as_ptr()`。
    ///
    /// # 安全性(Safety）
    ///
    /// 调用方必须保证 `self` 满足构造一个引用所需的全部要求:指针非空、已对齐、
    /// 指向一个已初始化且类型为 `T` 的有效值、在返回引用的整个生命周期内该内存
    /// 保持存活且不被可变别名修改。
    #[must_use]
    #[inline]
    pub const unsafe fn as_ref(&self) -> &T {
        // SAFETY: 调用方必须保证 `self` 满足构造引用的全部要求。
        unsafe { self.pointer.as_ref() }
    }

    /// 可变地解引用其内容,返回可变引用。
    ///
    /// 返回引用的生命周期被绑定到 `self`,因此其行为"仿佛"(as if)真的是在可变
    /// 借用一个 `T` 的实例。如果需要一个更长(未绑定)的生命周期,请改用
    /// `&mut *my_ptr.as_ptr()`。
    ///
    /// # 安全性(Safety）
    ///
    /// 调用方必须保证 `self` 满足构造一个可变引用所需的全部要求:指针非空、已对齐、
    /// 指向一个已初始化且类型为 `T` 的有效值,并且在返回引用的整个生命周期内,
    /// 不存在任何对该内存的其他活动引用(独占访问)。
    #[must_use]
    #[inline]
    pub const unsafe fn as_mut(&mut self) -> &mut T {
        // SAFETY: 调用方必须保证 `self` 满足构造可变引用的全部要求。
        unsafe { self.pointer.as_mut() }
    }

    /// 转换为指向另一种类型的指针。
    #[must_use = "`self` will be dropped if the result is not used"]
    #[inline]
    pub const fn cast<U>(self) -> Unique<U> {
        // FIXME(const-hack):应替换为 `From`
        // SAFETY: 底层就是 `NonNull`,非空不变量在 cast 后依然成立。
        Unique { pointer: self.pointer.cast(), _marker: PhantomData }
    }
}

#[unstable(feature = "ptr_internals", issue = "none")]
impl<T: PointeeSized> Clone for Unique<T> {
    #[inline]
    fn clone(&self) -> Self {
        *self
    }
}

#[unstable(feature = "ptr_internals", issue = "none")]
impl<T: PointeeSized> Copy for Unique<T> {}

#[doc(hidden)]
#[unstable(feature = "trivial_clone", issue = "none")]
unsafe impl<T: PointeeSized> TrivialClone for Unique<T> {}

#[unstable(feature = "ptr_internals", issue = "none")]
impl<T: PointeeSized, U: PointeeSized> CoerceUnsized<Unique<U>> for Unique<T> where T: Unsize<U> {}

#[unstable(feature = "ptr_internals", issue = "none")]
impl<T: PointeeSized, U: PointeeSized> DispatchFromDyn<Unique<U>> for Unique<T> where T: Unsize<U> {}

#[unstable(feature = "pin_coerce_unsized_trait", issue = "150112")]
unsafe impl<T: PointeeSized> PinCoerceUnsized for Unique<T> {}

#[unstable(feature = "ptr_internals", issue = "none")]
impl<T: PointeeSized> fmt::Debug for Unique<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Pointer::fmt(&self.as_ptr(), f)
    }
}

#[unstable(feature = "ptr_internals", issue = "none")]
impl<T: PointeeSized> fmt::Pointer for Unique<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Pointer::fmt(&self.as_ptr(), f)
    }
}

#[unstable(feature = "ptr_internals", issue = "none")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl<T: PointeeSized> const From<&mut T> for Unique<T> {
    /// 将 `&mut T` 转换为 `Unique<T>`。
    ///
    /// 此转换不会失败,因为引用不可能为空。
    #[inline]
    fn from(reference: &mut T) -> Self {
        Self::from(NonNull::from(reference))
    }
}

#[unstable(feature = "ptr_internals", issue = "none")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl<T: PointeeSized> const From<NonNull<T>> for Unique<T> {
    /// 将 `NonNull<T>` 转换为 `Unique<T>`。
    ///
    /// 此转换不会失败,因为 `NonNull` 不可能为空。
    #[inline]
    fn from(pointer: NonNull<T>) -> Self {
        Unique::from_non_null(pointer)
    }
}
