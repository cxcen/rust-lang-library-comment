use crate::clone::TrivialClone;
use crate::cmp::Ordering;
use crate::marker::{Destruct, PointeeSized, Unsize};
use crate::mem::{MaybeUninit, SizedTypeProperties};
use crate::num::NonZero;
use crate::ops::{CoerceUnsized, DispatchFromDyn};
use crate::pin::PinCoerceUnsized;
use crate::ptr::Unique;
use crate::slice::{self, SliceIndex};
use crate::ub_checks::assert_unsafe_precondition;
use crate::{fmt, hash, intrinsics, mem, ptr};

/// 等价于 `*mut T`,但保证非零(非空)且[协变（covariant）][covariant]。
///
/// 在用裸指针构建数据结构时,这通常是正确的选择,但由于它附带了额外的属性,使用起来
/// 终究更危险。如果你拿不准是否该用 `NonNull<T>`,那就直接用 `*mut T`!
///
/// ## 非空不变量(non-null invariant）
///
/// 与 `*mut T` 不同,这个指针必须始终非空,即便它从不会被解引用。这是为了让 enum 能把
/// 这个被禁止的值(null)用作判别式(discriminant)—— 正因如此,`Option<NonNull<T>>`
/// 与 `*mut T` 大小相同(参见下文“表示”一节)。不过,只要不解引用,该指针仍然可以悬垂
/// (dangle)。
///
/// 这个非空不变量是整个类型的核心保证:
/// - 安全构造函数 [`new`](NonNull::new) 会检查空指针,空时返回 `None`,从而把不变量的
///   维护交给类型自身;
/// - 而 `unsafe` 的 [`new_unchecked`](NonNull::new_unchecked) 跳过检查,要求调用方自行
///   保证指针非空,否则即为未定义行为(UB)。
///
/// ## niche 优化
///
/// 由于指针保证非空,null 这个位模式便空了出来,可被用作 niche:`Option<NonNull<T>>`
/// 会把 null 当作 `None` 的判别值,因此 `Option<NonNull<T>>` 与裸指针 `*mut T` 同大小、
/// 同对齐,不会多占空间。这正是“表示”一节所演示的现象。
///
/// ## 协变(covariant）
///
/// 与 `*mut T`(对 `T` 不变,invariant)不同,`NonNull<T>` 对 `T` 是协变的。对绝大多数
/// 数据结构和安全抽象(如 `Box`、`Rc`、`Arc`、`Vec`、`LinkedList`)而言,协变正是想要
/// 的行为,这也使 `NonNull<T>` 更适合作为自定义集合的构建块。但代价是:协变不附带任何
/// 借用检查,因此别名(aliasing)安全完全由调用方自行保证 —— 这一点和 `&mut`(有借用
/// 检查器把关)截然不同。
///
/// 在少数情况下,如果你的类型对外暴露了通过 `NonNull<T>` 改写 `T` 值的途径,而你又需要
/// 阻止协变带来的不健全(unsoundness)(例如 `T` 可能是一个生命周期更短的引用),你应当
/// 添加一个字段把类型变为不变,例如 `PhantomData<Cell<T>>` 或 `PhantomData<&'a mut T>`。
///
/// 必须为不变(invariant）的类型示例:
/// ```rust
/// use std::cell::Cell;
/// use std::marker::PhantomData;
/// struct Invariant<T> {
///     ptr: std::ptr::NonNull<T>,
///     _invariant: PhantomData<Cell<T>>,
/// }
/// ```
///
/// 注意 `NonNull<T>` 为 `&T` 提供了 `From` 实现。但这并不改变如下事实:通过一个(由共享
/// 引用派生而来的)指针进行改写是未定义行为,除非改写发生在 [`UnsafeCell<T>`] 内部。由
/// 共享引用创建可变引用同理。在不借助 `UnsafeCell<T>` 的前提下使用这个 `From` 实现时,
/// 确保 `as_mut` 永不被调用、`as_ptr` 永不用于改写,是你的责任。
///
/// # Representation
///
/// 得益于 [null pointer optimization](即上文的 niche 优化),`NonNull<T>` 与
/// `Option<NonNull<T>>` 保证具有相同的大小和对齐:
///
/// ```
/// use std::ptr::NonNull;
///
/// assert_eq!(size_of::<NonNull<i16>>(), size_of::<Option<NonNull<i16>>>());
/// assert_eq!(align_of::<NonNull<i16>>(), align_of::<Option<NonNull<i16>>>());
///
/// assert_eq!(size_of::<NonNull<str>>(), size_of::<Option<NonNull<str>>>());
/// assert_eq!(align_of::<NonNull<str>>(), align_of::<Option<NonNull<str>>>());
/// ```
///
/// [covariant]: https://doc.rust-lang.org/reference/subtyping.html
/// [`PhantomData`]: crate::marker::PhantomData
/// [`UnsafeCell<T>`]: crate::cell::UnsafeCell
/// [null pointer optimization]: crate::option#representation
#[stable(feature = "nonnull", since = "1.25.0")]
#[repr(transparent)]
#[rustc_layout_scalar_valid_range_start(1)]
#[rustc_nonnull_optimization_guaranteed]
#[rustc_diagnostic_item = "NonNull"]
pub struct NonNull<T: PointeeSized> {
    // 记得使用 `.as_ptr()` 而非直接访问 `.pointer`,因为对该字段做字段投影
    // (field projecting)是被禁止的,参见 <https://github.com/rust-lang/compiler-team/issues/807>。
    pointer: *const T,
}

/// `NonNull` 指针不是 `Send` 的,因为它们所引用的数据可能存在别名(aliased)。
// 注:这个 impl 并非必要,但能提供更好的错误信息。
#[stable(feature = "nonnull", since = "1.25.0")]
impl<T: PointeeSized> !Send for NonNull<T> {}

/// `NonNull` 指针不是 `Sync` 的,因为它们所引用的数据可能存在别名(aliased)。
// 注:这个 impl 并非必要,但能提供更好的错误信息。
#[stable(feature = "nonnull", since = "1.25.0")]
impl<T: PointeeSized> !Sync for NonNull<T> {}

impl<T: Sized> NonNull<T> {
    /// 用给定的地址创建一个指针,该指针不携带任何 [provenance][crate::ptr#provenance]。
    ///
    /// 更多细节请参见裸指针上的等价方法 [`ptr::without_provenance_mut`]。
    ///
    /// 这是一个 [Strict Provenance][crate::ptr#strict-provenance] API。
    #[stable(feature = "nonnull_provenance", since = "1.89.0")]
    #[rustc_const_stable(feature = "nonnull_provenance", since = "1.89.0")]
    #[must_use]
    #[inline]
    pub const fn without_provenance(addr: NonZero<usize>) -> Self {
        let pointer = crate::ptr::without_provenance(addr.get());
        // SAFETY: 我们已知 `addr` 非零。
        unsafe { NonNull { pointer } }
    }

    /// 创建一个悬垂(dangling)但已正确对齐的新 `NonNull`。
    ///
    /// 这在初始化“惰性分配”的类型时很有用,例如 `Vec::new` 就是这么做的:尚未分配内存
    /// 时用一个对齐的悬垂指针作占位。
    ///
    /// 注意:返回指针的地址有可能恰好与某个有效指针的地址相同,因此**绝不能**把它当作
    /// “尚未初始化”的哨兵值(sentinel)来使用。需要惰性分配的类型必须借助其他手段来追踪
    /// 初始化状态。
    ///
    /// 该指针**不可解引用**:它只是对齐的,并不指向任何有效的已分配内存。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::ptr::NonNull;
    ///
    /// let ptr = NonNull::<u32>::dangling();
    /// // 重要提示:在初始化之前,不要试图访问 `ptr` 所指的值!
    /// // 这个指针非空,但同样也不是有效的!
    /// ```
    #[stable(feature = "nonnull", since = "1.25.0")]
    #[rustc_const_stable(feature = "const_nonnull_dangling", since = "1.36.0")]
    #[must_use]
    #[inline]
    pub const fn dangling() -> Self {
        let align = crate::ptr::Alignment::of::<T>();
        NonNull::without_provenance(align.as_nonzero())
    }

    /// 把一个地址转换回可变指针,并拾取此前“暴露(exposed)”过的
    /// [provenance][crate::ptr#provenance]。
    ///
    /// 更多细节请参见裸指针上的等价方法 [`ptr::with_exposed_provenance_mut`]。
    ///
    /// 这是一个 [Exposed Provenance][crate::ptr#exposed-provenance] API。
    #[stable(feature = "nonnull_provenance", since = "1.89.0")]
    #[inline]
    pub fn with_exposed_provenance(addr: NonZero<usize>) -> Self {
        // SAFETY: 我们已知 `addr` 非零。
        unsafe {
            let ptr = crate::ptr::with_exposed_provenance_mut(addr.get());
            NonNull::new_unchecked(ptr)
        }
    }

    /// 返回指向该值的共享引用。与 [`as_ref`] 不同,本方法不要求该值已经初始化。
    ///
    /// 可变版本参见 [`as_uninit_mut`]。
    ///
    /// [`as_ref`]: NonNull::as_ref
    /// [`as_uninit_mut`]: NonNull::as_uninit_mut
    ///
    /// # 安全性(Safety）
    ///
    /// 调用本方法时,调用方必须保证该指针
    /// [可转换为引用](crate::ptr#pointer-to-reference-conversion)。
    /// 注意:由于所创建的引用指向的是 `MaybeUninit<T>`,源指针可以指向未初始化的内存。
    /// 此外,返回引用的生命周期 `'a` 由调用方任意选择,调用方需保证在该引用存活期间
    /// 遵守 Rust 的别名(aliasing)规则。
    #[inline]
    #[must_use]
    #[unstable(feature = "ptr_as_uninit", issue = "75402")]
    pub const unsafe fn as_uninit_ref<'a>(self) -> &'a MaybeUninit<T> {
        // SAFETY: 调用方必须保证 `self` 满足引用的全部要求。
        unsafe { &*self.cast().as_ptr() }
    }

    /// 返回指向该值的独占(unique)引用。与 [`as_mut`] 不同,本方法不要求该值已经初始化。
    ///
    /// 共享版本参见 [`as_uninit_ref`]。
    ///
    /// [`as_mut`]: NonNull::as_mut
    /// [`as_uninit_ref`]: NonNull::as_uninit_ref
    ///
    /// # 安全性(Safety）
    ///
    /// 调用本方法时,调用方必须保证该指针
    /// [可转换为引用](crate::ptr#pointer-to-reference-conversion)。
    /// 注意:由于所创建的引用指向的是 `MaybeUninit<T>`,源指针可以指向未初始化的内存。
    /// 此外,返回引用的生命周期 `'a` 由调用方任意选择;由于这是一个可变(独占)引用,
    /// 调用方必须保证在该引用存活期间,不存在任何其他指向同一内存的引用或别名访问。
    #[inline]
    #[must_use]
    #[unstable(feature = "ptr_as_uninit", issue = "75402")]
    pub const unsafe fn as_uninit_mut<'a>(self) -> &'a mut MaybeUninit<T> {
        // SAFETY: 调用方必须保证 `self` 满足引用的全部要求。
        unsafe { &mut *self.cast().as_ptr() }
    }

    /// 从指向 `T` 的指针转换为指向 `[T; N]` 的指针。
    #[inline]
    #[unstable(feature = "ptr_cast_array", issue = "144514")]
    pub const fn cast_array<const N: usize>(self) -> NonNull<[T; N]> {
        self.cast()
    }
}

impl<T: PointeeSized> NonNull<T> {
    /// 创建一个新的 `NonNull`。
    ///
    /// # 安全性(Safety）
    ///
    /// `ptr` 必须非空。这是本方法跳过空指针检查所必须依赖的不变量:若传入空指针,
    /// 将破坏 `NonNull` 的非空不变量,构成未定义行为(UB)。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::ptr::NonNull;
    ///
    /// let mut x = 0u32;
    /// let ptr = unsafe { NonNull::new_unchecked(&mut x as *mut _) };
    /// ```
    ///
    /// 本函数的*错误*用法:
    ///
    /// ```rust,no_run
    /// use std::ptr::NonNull;
    ///
    /// // 千万别这么做!!!这是未定义行为。⚠️
    /// let ptr = unsafe { NonNull::<u32>::new_unchecked(std::ptr::null_mut()) };
    /// ```
    #[stable(feature = "nonnull", since = "1.25.0")]
    #[rustc_const_stable(feature = "const_nonnull_new_unchecked", since = "1.25.0")]
    #[inline]
    #[track_caller]
    pub const unsafe fn new_unchecked(ptr: *mut T) -> Self {
        // SAFETY: 调用方必须保证 `ptr` 非空。
        unsafe {
            assert_unsafe_precondition!(
                check_language_ub,
                "NonNull::new_unchecked requires that the pointer is non-null",
                (ptr: *mut () = ptr as *mut ()) => !ptr.is_null()
            );
            NonNull { pointer: ptr as _ }
        }
    }

    /// 当 `ptr` 非空时创建一个新的 `NonNull`,否则返回 `None`。
    ///
    /// 这是安全的构造方式:空指针检查由本方法完成,从而把非空不变量的维护交给类型自身。
    ///
    /// # Panics during const evaluation
    ///
    /// 在 const 求值期间,如果无法判定指针是否为空,本方法将 panic。更多信息参见
    /// [`is_null`]。
    ///
    /// [`is_null`]: ../primitive.pointer.html#method.is_null-1
    ///
    /// # 示例
    ///
    /// ```
    /// use std::ptr::NonNull;
    ///
    /// let mut x = 0u32;
    /// let ptr = NonNull::<u32>::new(&mut x as *mut _).expect("ptr is null!");
    ///
    /// if let Some(ptr) = NonNull::<u32>::new(std::ptr::null_mut()) {
    ///     unreachable!();
    /// }
    /// ```
    #[stable(feature = "nonnull", since = "1.25.0")]
    #[rustc_const_stable(feature = "const_nonnull_new", since = "1.85.0")]
    #[inline]
    pub const fn new(ptr: *mut T) -> Option<Self> {
        if !ptr.is_null() {
            // SAFETY: 该指针已被检查过,确定不是 null。
            Some(unsafe { Self::new_unchecked(ptr) })
        } else {
            None
        }
    }

    /// 将一个共享引用转换为 `NonNull` 指针。
    #[stable(feature = "non_null_from_ref", since = "1.89.0")]
    #[rustc_const_stable(feature = "non_null_from_ref", since = "1.89.0")]
    #[inline]
    pub const fn from_ref(r: &T) -> Self {
        // SAFETY: 引用不可能为 null。
        unsafe { NonNull { pointer: r as *const T } }
    }

    /// 将一个可变引用转换为 `NonNull` 指针。
    #[stable(feature = "non_null_from_ref", since = "1.89.0")]
    #[rustc_const_stable(feature = "non_null_from_ref", since = "1.89.0")]
    #[inline]
    pub const fn from_mut(r: &mut T) -> Self {
        // SAFETY: 可变引用不可能为 null。
        unsafe { NonNull { pointer: r as *mut T } }
    }

    /// 功能与 [`std::ptr::from_raw_parts`] 相同,区别在于本方法返回的是 `NonNull` 指针,
    /// 而非裸的 `*const` 指针。
    ///
    /// 更多细节请参见 [`std::ptr::from_raw_parts`] 的文档。
    ///
    /// [`std::ptr::from_raw_parts`]: crate::ptr::from_raw_parts
    #[unstable(feature = "ptr_metadata", issue = "81513")]
    #[inline]
    pub const fn from_raw_parts(
        data_pointer: NonNull<impl super::Thin>,
        metadata: <T as super::Pointee>::Metadata,
    ) -> NonNull<T> {
        // SAFETY: `ptr::from::raw_parts_mut` 的结果非空,因为 `data_pointer` 非空。
        unsafe {
            NonNull::new_unchecked(super::from_raw_parts_mut(data_pointer.as_ptr(), metadata))
        }
    }

    /// 将一个(可能是宽指针的)指针分解为它的数据指针和元数据(metadata)两部分。
    ///
    /// 之后可用 [`NonNull::from_raw_parts`] 把它重新组合回来。
    #[unstable(feature = "ptr_metadata", issue = "81513")]
    #[must_use = "this returns the result of the operation, \
                  without modifying the original"]
    #[inline]
    pub const fn to_raw_parts(self) -> (NonNull<()>, <T as super::Pointee>::Metadata) {
        (self.cast(), super::metadata(self.as_ptr()))
    }

    /// 获取指针的“地址(address)”部分。
    ///
    /// 更多细节请参见裸指针上的等价方法 [`pointer::addr`]。
    ///
    /// 这是一个 [Strict Provenance][crate::ptr#strict-provenance] API。
    #[must_use]
    #[inline]
    #[stable(feature = "strict_provenance", since = "1.84.0")]
    pub fn addr(self) -> NonZero<usize> {
        // SAFETY: 该指针由类型保证非空,意味着其地址必然非零。
        unsafe { NonZero::new_unchecked(self.as_ptr().addr()) }
    }

    /// 暴露(expose)指针的 ["provenance"][crate::ptr#provenance] 部分,以便日后在
    /// [`with_exposed_provenance`][NonNull::with_exposed_provenance] 中使用,并返回其
    /// “地址(address)”部分。
    ///
    /// 更多细节请参见裸指针上的等价方法 [`pointer::expose_provenance`]。
    ///
    /// 这是一个 [Exposed Provenance][crate::ptr#exposed-provenance] API。
    #[stable(feature = "nonnull_provenance", since = "1.89.0")]
    pub fn expose_provenance(self) -> NonZero<usize> {
        // SAFETY: 该指针由类型保证非空,意味着其地址必然非零。
        unsafe { NonZero::new_unchecked(self.as_ptr().expose_provenance()) }
    }

    /// 创建一个新指针,使用给定的地址,并保留 `self` 的
    /// [provenance][crate::ptr#provenance]。
    ///
    /// 更多细节请参见裸指针上的等价方法 [`pointer::with_addr`]。
    ///
    /// 这是一个 [Strict Provenance][crate::ptr#strict-provenance] API。
    #[must_use]
    #[inline]
    #[stable(feature = "strict_provenance", since = "1.84.0")]
    pub fn with_addr(self, addr: NonZero<usize>) -> Self {
        // SAFETY: `ptr::with_addr` 的结果非空,因为 `addr` 保证非零。
        unsafe { NonNull::new_unchecked(self.as_ptr().with_addr(addr.get()) as *mut _) }
    }

    /// 通过把 `self` 的地址映射为一个新地址来创建一个新指针,同时保留 `self` 的
    /// [provenance][crate::ptr#provenance]。
    ///
    /// 更多细节请参见裸指针上的等价方法 [`pointer::map_addr`]。
    ///
    /// 这是一个 [Strict Provenance][crate::ptr#strict-provenance] API。
    #[must_use]
    #[inline]
    #[stable(feature = "strict_provenance", since = "1.84.0")]
    pub fn map_addr(self, f: impl FnOnce(NonZero<usize>) -> NonZero<usize>) -> Self {
        self.with_addr(f(self.addr()))
    }

    /// 获取底层的 `*mut` 指针。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::ptr::NonNull;
    ///
    /// let mut x = 0u32;
    /// let ptr = NonNull::new(&mut x).expect("ptr is null!");
    ///
    /// let x_value = unsafe { *ptr.as_ptr() };
    /// assert_eq!(x_value, 0);
    ///
    /// unsafe { *ptr.as_ptr() += 2; }
    /// let x_value = unsafe { *ptr.as_ptr() };
    /// assert_eq!(x_value, 2);
    /// ```
    #[stable(feature = "nonnull", since = "1.25.0")]
    #[rustc_const_stable(feature = "const_nonnull_as_ptr", since = "1.32.0")]
    #[rustc_never_returns_null_ptr]
    #[must_use]
    #[inline(always)]
    pub const fn as_ptr(self) -> *mut T {
        // 出于与 `NonZero::get` 相同的原因,这里采用 transmute。

        // SAFETY: `NonNull` 在 `*const T` 之上是 `transparent` 的,而 `*const T`
        // 与 `*mut T` 具有相同的布局,因此可以传递地把 `NonNull` 直接 transmute 为 `*mut T`。
        unsafe { mem::transmute::<Self, *mut T>(self) }
    }

    /// 返回指向该值的共享引用。如果该值可能未初始化,则必须改用 [`as_uninit_ref`]。
    ///
    /// 可变版本参见 [`as_mut`]。
    ///
    /// [`as_uninit_ref`]: NonNull::as_uninit_ref
    /// [`as_mut`]: NonNull::as_mut
    ///
    /// # 安全性(Safety）
    ///
    /// 调用本方法时,调用方必须保证该指针
    /// [可转换为引用](crate::ptr#pointer-to-reference-conversion)。这意味着:指针必须
    /// 已正确对齐、所指内存对 `T` 有效且已初始化;并且返回引用的生命周期 `'a` 由调用方
    /// 任意选择,调用方需保证在该引用存活期间所指数据始终有效,且遵守 Rust 的别名
    /// (aliasing)规则(在该共享引用存活期间,该内存不得被改写,除非位于 `UnsafeCell`
    /// 内部)。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::ptr::NonNull;
    ///
    /// let mut x = 0u32;
    /// let ptr = NonNull::new(&mut x as *mut _).expect("ptr is null!");
    ///
    /// let ref_x = unsafe { ptr.as_ref() };
    /// println!("{ref_x}");
    /// ```
    ///
    /// [the module documentation]: crate::ptr#safety
    #[stable(feature = "nonnull", since = "1.25.0")]
    #[rustc_const_stable(feature = "const_nonnull_as_ref", since = "1.73.0")]
    #[must_use]
    #[inline(always)]
    pub const unsafe fn as_ref<'a>(&self) -> &'a T {
        // SAFETY: 调用方必须保证 `self` 满足引用的全部要求。
        // `cast_const` 避免对可变裸指针做解引用。
        unsafe { &*self.as_ptr().cast_const() }
    }

    /// 返回指向该值的独占(unique)引用。如果该值可能未初始化,则必须改用 [`as_uninit_mut`]。
    ///
    /// 共享版本参见 [`as_ref`]。
    ///
    /// [`as_uninit_mut`]: NonNull::as_uninit_mut
    /// [`as_ref`]: NonNull::as_ref
    ///
    /// # 安全性(Safety）
    ///
    /// 调用本方法时,调用方必须保证该指针
    /// [可转换为引用](crate::ptr#pointer-to-reference-conversion)。这意味着:指针必须
    /// 已正确对齐、所指内存对 `T` 有效且已初始化;返回引用的生命周期 `'a` 由调用方任意
    /// 选择,调用方需保证在该引用存活期间所指数据始终有效。由于这是一个可变引用,调用方
    /// 还必须保证在该引用存活期间对该内存拥有**独占**访问权——不存在任何其他指向同一内存
    /// 的引用或别名访问(读或写)。
    /// # 示例
    ///
    /// ```
    /// use std::ptr::NonNull;
    ///
    /// let mut x = 0u32;
    /// let mut ptr = NonNull::new(&mut x).expect("null pointer");
    ///
    /// let x_ref = unsafe { ptr.as_mut() };
    /// assert_eq!(*x_ref, 0);
    /// *x_ref += 2;
    /// assert_eq!(*x_ref, 2);
    /// ```
    ///
    /// [the module documentation]: crate::ptr#safety
    #[stable(feature = "nonnull", since = "1.25.0")]
    #[rustc_const_stable(feature = "const_ptr_as_ref", since = "1.83.0")]
    #[must_use]
    #[inline(always)]
    pub const unsafe fn as_mut<'a>(&mut self) -> &'a mut T {
        // SAFETY: 调用方必须保证 `self` 满足可变引用的全部要求。
        unsafe { &mut *self.as_ptr() }
    }

    /// 转换为指向另一种类型的指针。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::ptr::NonNull;
    ///
    /// let mut x = 0u32;
    /// let ptr = NonNull::new(&mut x as *mut _).expect("null pointer");
    ///
    /// let casted_ptr = ptr.cast::<i8>();
    /// let raw_ptr: *mut i8 = casted_ptr.as_ptr();
    /// ```
    #[stable(feature = "nonnull_cast", since = "1.27.0")]
    #[rustc_const_stable(feature = "const_nonnull_cast", since = "1.36.0")]
    #[must_use = "this returns the result of the operation, \
                  without modifying the original"]
    #[inline]
    pub const fn cast<U>(self) -> NonNull<U> {
        // SAFETY: `self` 是一个 `NonNull` 指针,因而必然非空。
        unsafe { NonNull { pointer: self.as_ptr() as *mut U } }
    }

    /// 尝试通过检查对齐来转换为指向另一种类型的指针。
    ///
    /// 如果该指针对目标类型已正确对齐,则转换为目标类型;否则返回 `None`。
    ///
    /// # 示例
    ///
    /// ```rust
    /// #![feature(pointer_try_cast_aligned)]
    /// use std::ptr::NonNull;
    ///
    /// let mut x = 0u64;
    ///
    /// let aligned = NonNull::from_mut(&mut x);
    /// let unaligned = unsafe { aligned.byte_add(1) };
    ///
    /// assert!(aligned.try_cast_aligned::<u32>().is_some());
    /// assert!(unaligned.try_cast_aligned::<u32>().is_none());
    /// ```
    #[unstable(feature = "pointer_try_cast_aligned", issue = "141221")]
    #[must_use = "this returns the result of the operation, \
                  without modifying the original"]
    #[inline]
    pub fn try_cast_aligned<U>(self) -> Option<NonNull<U>> {
        if self.is_aligned_to(align_of::<U>()) { Some(self.cast()) } else { None }
    }

    /// 给指针加上一个偏移量。
    ///
    /// `count` 以 T 为单位计量;例如 `count` 为 3 表示指针偏移 `3 * size_of::<T>()` 个字节。
    ///
    /// # 安全性(Safety）
    ///
    /// 如果违反以下任一条件,结果即为未定义行为(Undefined Behavior):
    ///
    /// * 计算出的偏移量(`count * size_of::<T>()` 字节)不得溢出 `isize`。
    ///
    /// * 如果计算出的偏移量非零,那么 `self` 必须派生自指向某个 [allocation] 的指针,且
    ///   从 `self` 到结果指针之间的整段内存范围都必须落在该 allocation 的边界(in bounds)
    ///   之内。特别地,这段范围不得“绕回(wrap around)”地址空间的边界。
    ///
    /// allocation 的大小永远不会超过 `isize::MAX` 字节,因此只要计算出的偏移量停留在该
    /// allocation 的边界之内,就一定满足上面第一条要求。这意味着,举例来说,
    /// `vec.as_ptr().add(vec.len())`(对 `vec: Vec<T>`)总是安全的。
    ///
    /// [allocation]: crate::ptr#allocation
    ///
    /// # 示例
    ///
    /// ```
    /// use std::ptr::NonNull;
    ///
    /// let mut s = [1, 2, 3];
    /// let ptr: NonNull<u32> = NonNull::new(s.as_mut_ptr()).unwrap();
    ///
    /// unsafe {
    ///     println!("{}", ptr.offset(1).read());
    ///     println!("{}", ptr.offset(2).read());
    /// }
    /// ```
    #[inline(always)]
    #[cfg_attr(miri, track_caller)] // 即便没有 panic,这也有助于 Miri 的回溯(backtrace)
    #[must_use = "returns a new pointer rather than modifying its argument"]
    #[stable(feature = "non_null_convenience", since = "1.80.0")]
    #[rustc_const_stable(feature = "non_null_convenience", since = "1.80.0")]
    pub const unsafe fn offset(self, count: isize) -> Self
    where
        T: Sized,
    {
        // SAFETY: 调用方必须遵守 `offset` 的安全约定。
        // 此外,`offset` 的安全约定保证了结果指针指向某个 allocation,而 null 处不可能存在
        // allocation,因此可以安全地构造 `NonNull`。
        unsafe { NonNull { pointer: intrinsics::offset(self.as_ptr(), count) } }
    }

    /// 以字节为单位计算指针的偏移。
    ///
    /// `count` 以**字节**为单位计量。
    ///
    /// 这纯粹是“先转换为 `u8` 指针,再在其上使用 [offset][pointer::offset]”的便捷封装。
    /// 文档与安全要求请参见该方法。
    ///
    /// 对于非 `Sized` 的被指对象(pointee),本操作只改变数据指针,元数据(metadata)
    /// 保持不变。
    #[must_use]
    #[inline(always)]
    #[cfg_attr(miri, track_caller)] // 即便没有 panic,这也有助于 Miri 的回溯(backtrace)
    #[stable(feature = "non_null_convenience", since = "1.80.0")]
    #[rustc_const_stable(feature = "non_null_convenience", since = "1.80.0")]
    pub const unsafe fn byte_offset(self, count: isize) -> Self {
        // SAFETY: 调用方必须遵守 `offset` 的安全约定,而 `byte_offset` 的安全约定与之相同。
        // 此外,`offset` 的安全约定保证了结果指针指向某个 allocation,而 null 处不可能存在
        // allocation,因此可以安全地构造 `NonNull`。
        unsafe { NonNull { pointer: self.as_ptr().byte_offset(count) } }
    }

    /// 给指针加上一个偏移量(等价于 `.offset(count as isize)` 的便捷写法)。
    ///
    /// `count` 以 T 为单位计量;例如 `count` 为 3 表示指针偏移 `3 * size_of::<T>()` 个字节。
    ///
    /// # 安全性(Safety）
    ///
    /// 如果违反以下任一条件,结果即为未定义行为(Undefined Behavior):
    ///
    /// * 计算出的偏移量(`count * size_of::<T>()` 字节)不得溢出 `isize`。
    ///
    /// * 如果计算出的偏移量非零,那么 `self` 必须派生自指向某个 [allocation] 的指针,且
    ///   从 `self` 到结果指针之间的整段内存范围都必须落在该 allocation 的边界(in bounds)
    ///   之内。特别地,这段范围不得“绕回(wrap around)”地址空间的边界。
    ///
    /// allocation 的大小永远不会超过 `isize::MAX` 字节,因此只要计算出的偏移量停留在该
    /// allocation 的边界之内,就一定满足上面第一条要求。这意味着,举例来说,
    /// `vec.as_ptr().add(vec.len())`(对 `vec: Vec<T>`)总是安全的。
    ///
    /// [allocation]: crate::ptr#allocation
    ///
    /// # 示例
    ///
    /// ```
    /// use std::ptr::NonNull;
    ///
    /// let s: &str = "123";
    /// let ptr: NonNull<u8> = NonNull::new(s.as_ptr().cast_mut()).unwrap();
    ///
    /// unsafe {
    ///     println!("{}", ptr.add(1).read() as char);
    ///     println!("{}", ptr.add(2).read() as char);
    /// }
    /// ```
    #[inline(always)]
    #[cfg_attr(miri, track_caller)] // 即便没有 panic,这也有助于 Miri 的回溯(backtrace)
    #[must_use = "returns a new pointer rather than modifying its argument"]
    #[stable(feature = "non_null_convenience", since = "1.80.0")]
    #[rustc_const_stable(feature = "non_null_convenience", since = "1.80.0")]
    pub const unsafe fn add(self, count: usize) -> Self
    where
        T: Sized,
    {
        // SAFETY: 调用方必须遵守 `offset` 的安全约定。
        // 此外,`offset` 的安全约定保证了结果指针指向某个 allocation,而 null 处不可能存在
        // allocation,因此可以安全地构造 `NonNull`。
        unsafe { NonNull { pointer: intrinsics::offset(self.as_ptr(), count) } }
    }

    /// 以字节为单位计算指针的偏移(等价于 `.byte_offset(count as isize)` 的便捷写法)。
    ///
    /// `count` 以字节为单位计量。
    ///
    /// 这纯粹是“先转换为 `u8` 指针,再在其上使用 [`add`][NonNull::add]”的便捷封装。
    /// 文档与安全要求请参见该方法。
    ///
    /// 对于非 `Sized` 的被指对象(pointee),本操作只改变数据指针,元数据(metadata)
    /// 保持不变。
    #[must_use]
    #[inline(always)]
    #[cfg_attr(miri, track_caller)] // 即便没有 panic,这也有助于 Miri 的回溯(backtrace)
    #[stable(feature = "non_null_convenience", since = "1.80.0")]
    #[rustc_const_stable(feature = "non_null_convenience", since = "1.80.0")]
    pub const unsafe fn byte_add(self, count: usize) -> Self {
        // SAFETY: 调用方必须遵守 `add` 的安全约定,而 `byte_add` 的安全约定与之相同。
        // 此外,`add` 的安全约定保证了结果指针指向某个 allocation,而 null 处不可能存在
        // allocation,因此可以安全地构造 `NonNull`。
        unsafe { NonNull { pointer: self.as_ptr().byte_add(count) } }
    }

    /// 从指针减去一个偏移量(等价于
    /// `.offset((count as isize).wrapping_neg())` 的便捷写法)。
    ///
    /// `count` 以 T 为单位计量;例如 `count` 为 3 表示指针偏移 `3 * size_of::<T>()` 个字节。
    ///
    /// # 安全性(Safety）
    ///
    /// 如果违反以下任一条件,结果即为未定义行为(Undefined Behavior):
    ///
    /// * 计算出的偏移量(`count * size_of::<T>()` 字节)不得溢出 `isize`。
    ///
    /// * 如果计算出的偏移量非零,那么 `self` 必须派生自指向某个 [allocation] 的指针,且
    ///   从 `self` 到结果指针之间的整段内存范围都必须落在该 allocation 的边界(in bounds)
    ///   之内。特别地,这段范围不得“绕回(wrap around)”地址空间的边界。
    ///
    /// allocation 的大小永远不会超过 `isize::MAX` 字节,因此只要计算出的偏移量停留在该
    /// allocation 的边界之内,就一定满足上面第一条要求。这意味着,举例来说,
    /// `vec.as_ptr().add(vec.len())`(对 `vec: Vec<T>`)总是安全的。
    ///
    /// [allocation]: crate::ptr#allocation
    ///
    /// # 示例
    ///
    /// ```
    /// use std::ptr::NonNull;
    ///
    /// let s: &str = "123";
    ///
    /// unsafe {
    ///     let end: NonNull<u8> = NonNull::new(s.as_ptr().cast_mut()).unwrap().add(3);
    ///     println!("{}", end.sub(1).read() as char);
    ///     println!("{}", end.sub(2).read() as char);
    /// }
    /// ```
    #[inline(always)]
    #[cfg_attr(miri, track_caller)] // 即便没有 panic,这也有助于 Miri 的回溯(backtrace)
    #[must_use = "returns a new pointer rather than modifying its argument"]
    #[stable(feature = "non_null_convenience", since = "1.80.0")]
    #[rustc_const_stable(feature = "non_null_convenience", since = "1.80.0")]
    pub const unsafe fn sub(self, count: usize) -> Self
    where
        T: Sized,
    {
        if T::IS_ZST {
            // 当被指对象是 ZST(零大小类型)时,指针算术不做任何事。
            self
        } else {
            // SAFETY: 调用方必须遵守 `offset` 的安全约定。
            // 因为被指对象*不是* ZST,这意味着 `count` 至多为 `isize::MAX`,因此取负
            // 不会溢出。
            unsafe { self.offset((count as isize).unchecked_neg()) }
        }
    }

    /// 以字节为单位计算指针的偏移(等价于
    /// `.byte_offset((count as isize).wrapping_neg())` 的便捷写法)。
    ///
    /// `count` 以字节为单位计量。
    ///
    /// 这纯粹是“先转换为 `u8` 指针,再在其上使用 [`sub`][NonNull::sub]”的便捷封装。
    /// 文档与安全要求请参见该方法。
    ///
    /// 对于非 `Sized` 的被指对象(pointee),本操作只改变数据指针,元数据(metadata)
    /// 保持不变。
    #[must_use]
    #[inline(always)]
    #[cfg_attr(miri, track_caller)] // 即便没有 panic,这也有助于 Miri 的回溯(backtrace)
    #[stable(feature = "non_null_convenience", since = "1.80.0")]
    #[rustc_const_stable(feature = "non_null_convenience", since = "1.80.0")]
    pub const unsafe fn byte_sub(self, count: usize) -> Self {
        // SAFETY: 调用方必须遵守 `sub` 的安全约定,而 `byte_sub` 的安全约定与之相同。
        // 此外,`sub` 的安全约定保证了结果指针指向某个 allocation,而 null 处不可能存在
        // allocation,因此可以安全地构造 `NonNull`。
        unsafe { NonNull { pointer: self.as_ptr().byte_sub(count) } }
    }

    /// 计算同一 allocation 内两个指针之间的距离。返回值以 T 为单位:即字节距离除以
    /// `size_of::<T>()`。
    ///
    /// 它等价于 `(self as isize - origin as isize) / (size_of::<T>() as isize)`,区别在于
    /// 它有多得多的引发 UB 的可能性,作为交换,编译器能更好地理解你的意图。
    ///
    /// 本方法的主要动机是计算某个 `T` 数组/切片的 `len`,而你当前正用一对“起始(start)”
    /// 与“结束(end)”指针来表示该数组(其中“end”是“数组末尾的下一个位置”,one past the
    /// end)。在这种情形下,`end.offset_from(start)` 即可得到数组的长度。
    ///
    /// 对于上述用例,下面所有的安全要求都自然得到满足。
    ///
    /// [`offset`]: #method.offset
    ///
    /// # 安全性(Safety）
    ///
    /// 如果违反以下任一条件,结果即为未定义行为(Undefined Behavior):
    ///
    /// * `self` 和 `origin` 必须满足以下之一:
    ///
    ///   * 指向同一地址,或者
    ///   * 二者都*派生自*指向同一 [allocation] 的指针,且两指针之间的内存范围必须落在该
    ///     对象的边界(in bounds)之内。(参见下文示例。)
    ///
    /// * 两指针之间的距离(以字节计)必须是 `T` 大小的精确整数倍。
    ///
    /// 作为推论,两指针在数学整数意义上(不“绕回”)的绝对距离(以字节计)不会溢出 `isize`。
    /// 这一点由“in bounds”要求以及“任何 allocation 都不会大于 `isize::MAX` 字节”这一事实
    /// 共同保证。
    ///
    /// “两指针必须派生自同一 allocation”这条要求主要是出于 `const` 兼容性:指向*不同*
    /// 已分配对象的两个指针之间的距离在编译期是未知的。不过,该要求在运行期同样存在,并可
    /// 能被优化所利用。如果你想计算并不保证来自同一 allocation 的两个指针之差,请改用
    /// `(self as isize - origin as isize) / size_of::<T>()`。
    // FIXME: 一旦 `addr()` 稳定,就推荐用它替代 `as usize`。
    ///
    /// [`add`]: #method.add
    /// [allocation]: crate::ptr#allocation
    ///
    /// # Panics
    ///
    /// 如果 `T` 是零大小类型(Zero-Sized Type,"ZST"),本函数会 panic。
    ///
    /// # 示例
    ///
    /// 基本用法:
    ///
    /// ```
    /// use std::ptr::NonNull;
    ///
    /// let a = [0; 5];
    /// let ptr1: NonNull<u32> = NonNull::from(&a[1]);
    /// let ptr2: NonNull<u32> = NonNull::from(&a[3]);
    /// unsafe {
    ///     assert_eq!(ptr2.offset_from(ptr1), 2);
    ///     assert_eq!(ptr1.offset_from(ptr2), -2);
    ///     assert_eq!(ptr1.offset(2), ptr2);
    ///     assert_eq!(ptr2.offset(-2), ptr1);
    /// }
    /// ```
    ///
    /// *错误*用法:
    ///
    /// ```rust,no_run
    /// use std::ptr::NonNull;
    ///
    /// let ptr1 = NonNull::new(Box::into_raw(Box::new(0u8))).unwrap();
    /// let ptr2 = NonNull::new(Box::into_raw(Box::new(1u8))).unwrap();
    /// let diff = (ptr2.addr().get() as isize).wrapping_sub(ptr1.addr().get() as isize);
    /// // 让 ptr2_other 成为 ptr2.add(1) 的“别名(alias)”,但它派生自 ptr1。
    /// let diff_plus_1 = diff.wrapping_add(1);
    /// let ptr2_other = NonNull::new(ptr1.as_ptr().wrapping_byte_offset(diff_plus_1)).unwrap();
    /// assert_eq!(ptr2.addr(), ptr2_other.addr());
    /// // 由于 ptr2_other 与 ptr2 派生自指向不同对象的指针,
    /// // 计算它们之间的偏移是未定义行为,哪怕它们
    /// // 指向的地址都落在同一对象的边界之内!
    ///
    /// let one = unsafe { ptr2_other.offset_from(ptr2) }; // 未定义行为!⚠️
    /// ```
    #[inline]
    #[cfg_attr(miri, track_caller)] // 即便没有 panic,这也有助于 Miri 的回溯(backtrace)
    #[stable(feature = "non_null_convenience", since = "1.80.0")]
    #[rustc_const_stable(feature = "non_null_convenience", since = "1.80.0")]
    pub const unsafe fn offset_from(self, origin: NonNull<T>) -> isize
    where
        T: Sized,
    {
        // SAFETY: 调用方必须遵守 `offset_from` 的安全约定。
        unsafe { self.as_ptr().offset_from(origin.as_ptr()) }
    }

    /// 计算同一 allocation 内两个指针之间的距离。返回值以**字节**为单位。
    ///
    /// 这纯粹是“先转换为 `u8` 指针,再在其上使用
    /// [`offset_from`][NonNull::offset_from]”的便捷封装。文档与安全要求请参见该方法。
    ///
    /// 对于非 `Sized` 的被指对象(pointee),本操作只考虑数据指针,忽略元数据(metadata)。
    #[inline(always)]
    #[cfg_attr(miri, track_caller)] // 即便没有 panic,这也有助于 Miri 的回溯(backtrace)
    #[stable(feature = "non_null_convenience", since = "1.80.0")]
    #[rustc_const_stable(feature = "non_null_convenience", since = "1.80.0")]
    pub const unsafe fn byte_offset_from<U: ?Sized>(self, origin: NonNull<U>) -> isize {
        // SAFETY: 调用方必须遵守 `byte_offset_from` 的安全约定。
        unsafe { self.as_ptr().byte_offset_from(origin.as_ptr()) }
    }

    // 注:`wrapping_offset`、`wrapping_add` 等方法没有实现,因为它们可能绕回(wrap)到 null。

    /// 计算同一 allocation 内两个指针之间的距离,*前提是已知 `self` 大于或等于 `origin`*。
    /// 返回值以 T 为单位:即字节距离除以 `size_of::<T>()`。
    ///
    /// 它计算出的值与 [`offset_from`](#method.offset_from) 相同,但附加了一条前置条件:
    /// 偏移量保证非负。本方法等价于
    /// `usize::try_from(self.offset_from(origin)).unwrap_unchecked()`,但它能向优化器提供
    /// 略多的信息,在某些后端上有时可带来略好的优化。
    ///
    /// 本方法可以理解为“恢复”出此前传给 [`add`](#method.add) 的那个 `count`(或者,把两个
    /// 参数交换顺序,则是传给 [`sub`](#method.sub) 的 `count`)。在满足各自安全前置条件的
    /// 前提下,以下表达式全都等价:
    /// ```rust
    /// # unsafe fn blah(ptr: std::ptr::NonNull<u32>, origin: std::ptr::NonNull<u32>, count: usize) -> bool { unsafe {
    /// ptr.offset_from_unsigned(origin) == count
    /// # &&
    /// origin.add(count) == ptr
    /// # &&
    /// ptr.sub(count) == origin
    /// # } }
    /// ```
    ///
    /// # 安全性(Safety）
    ///
    /// - 两指针之间的距离必须非负(`self >= origin`)
    ///
    /// - [`offset_from`](#method.offset_from) 的*所有*安全条件同样适用于本方法;完整细节
    ///   请参见该方法。
    ///
    /// 重点提示:尽管本方法的返回类型能表示更大的偏移量,但仍然*不允许*传入相差超过
    /// `isize::MAX` *字节*的指针。因此,本方法的结果将始终小于或等于 `isize::MAX as usize`。
    ///
    /// # Panics
    ///
    /// 如果 `T` 是零大小类型(Zero-Sized Type,"ZST"),本函数会 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::ptr::NonNull;
    ///
    /// let a = [0; 5];
    /// let ptr1: NonNull<u32> = NonNull::from(&a[1]);
    /// let ptr2: NonNull<u32> = NonNull::from(&a[3]);
    /// unsafe {
    ///     assert_eq!(ptr2.offset_from_unsigned(ptr1), 2);
    ///     assert_eq!(ptr1.add(2), ptr2);
    ///     assert_eq!(ptr2.sub(2), ptr1);
    ///     assert_eq!(ptr2.offset_from_unsigned(ptr2), 0);
    /// }
    ///
    /// // 下面这样写是错误的,因为两个指针的顺序不正确:
    /// // ptr1.offset_from_unsigned(ptr2)
    /// ```
    #[inline]
    #[cfg_attr(miri, track_caller)] // 即便没有 panic,这也有助于 Miri 的回溯(backtrace)
    #[stable(feature = "ptr_sub_ptr", since = "1.87.0")]
    #[rustc_const_stable(feature = "const_ptr_sub_ptr", since = "1.87.0")]
    pub const unsafe fn offset_from_unsigned(self, subtracted: NonNull<T>) -> usize
    where
        T: Sized,
    {
        // SAFETY: 调用方必须遵守 `offset_from_unsigned` 的安全约定。
        unsafe { self.as_ptr().offset_from_unsigned(subtracted.as_ptr()) }
    }

    /// 计算同一 allocation 内两个指针之间的距离,*前提是已知 `self` 大于或等于 `origin`*。
    /// 返回值以**字节**为单位。
    ///
    /// 这纯粹是“先转换为 `u8` 指针,再在其上使用
    /// [`offset_from_unsigned`][NonNull::offset_from_unsigned]”的便捷封装。
    /// 文档与安全要求请参见该方法。
    ///
    /// 对于非 `Sized` 的被指对象(pointee),本操作只考虑数据指针,忽略元数据(metadata)。
    #[inline(always)]
    #[cfg_attr(miri, track_caller)] // 即便没有 panic,这也有助于 Miri 的回溯(backtrace)
    #[stable(feature = "ptr_sub_ptr", since = "1.87.0")]
    #[rustc_const_stable(feature = "const_ptr_sub_ptr", since = "1.87.0")]
    pub const unsafe fn byte_offset_from_unsigned<U: ?Sized>(self, origin: NonNull<U>) -> usize {
        // SAFETY: 调用方必须遵守 `byte_offset_from_unsigned` 的安全约定。
        unsafe { self.as_ptr().byte_offset_from_unsigned(origin.as_ptr()) }
    }

    /// 从 `self` 读出该值,但不移动它。这使 `self` 处的内存保持不变。
    ///
    /// 安全方面的注意事项与示例参见 [`ptr::read`]。
    ///
    /// [`ptr::read`]: crate::ptr::read()
    #[inline]
    #[cfg_attr(miri, track_caller)] // 即便没有 panic,这也有助于 Miri 的回溯(backtrace)
    #[stable(feature = "non_null_convenience", since = "1.80.0")]
    #[rustc_const_stable(feature = "non_null_convenience", since = "1.80.0")]
    pub const unsafe fn read(self) -> T
    where
        T: Sized,
    {
        // SAFETY: 调用方必须遵守 `read` 的安全约定。
        unsafe { ptr::read(self.as_ptr()) }
    }

    /// 对 `self` 处的值执行 volatile(易变)读取,但不移动它。这使 `self` 处的内存保持不变。
    ///
    /// volatile 操作意在作用于 I/O 内存,并保证编译器不会把它们消除,也不会让它们与其他
    /// volatile 操作发生重排。
    ///
    /// 安全方面的注意事项与示例参见 [`ptr::read_volatile`]。
    ///
    /// [`ptr::read_volatile`]: crate::ptr::read_volatile()
    #[inline]
    #[cfg_attr(miri, track_caller)] // 即便没有 panic,这也有助于 Miri 的回溯(backtrace)
    #[stable(feature = "non_null_convenience", since = "1.80.0")]
    pub unsafe fn read_volatile(self) -> T
    where
        T: Sized,
    {
        // SAFETY: 调用方必须遵守 `read_volatile` 的安全约定。
        unsafe { ptr::read_volatile(self.as_ptr()) }
    }

    /// 从 `self` 读出该值,但不移动它。这使 `self` 处的内存保持不变。
    ///
    /// 与 `read` 不同,本方法允许指针未对齐(unaligned)。
    ///
    /// 安全方面的注意事项与示例参见 [`ptr::read_unaligned`]。
    ///
    /// [`ptr::read_unaligned`]: crate::ptr::read_unaligned()
    #[inline]
    #[cfg_attr(miri, track_caller)] // 即便没有 panic,这也有助于 Miri 的回溯(backtrace)
    #[stable(feature = "non_null_convenience", since = "1.80.0")]
    #[rustc_const_stable(feature = "non_null_convenience", since = "1.80.0")]
    pub const unsafe fn read_unaligned(self) -> T
    where
        T: Sized,
    {
        // SAFETY: 调用方必须遵守 `read_unaligned` 的安全约定。
        unsafe { ptr::read_unaligned(self.as_ptr()) }
    }

    /// 将 `count * size_of::<T>()` 个字节从 `self` 拷贝到 `dest`。源与目标可以重叠。
    ///
    /// 注意:本方法的参数顺序与 [`ptr::copy`] *相同*。
    ///
    /// 安全方面的注意事项与示例参见 [`ptr::copy`]。
    ///
    /// [`ptr::copy`]: crate::ptr::copy()
    #[inline(always)]
    #[cfg_attr(miri, track_caller)] // 即便没有 panic,这也有助于 Miri 的回溯(backtrace)
    #[stable(feature = "non_null_convenience", since = "1.80.0")]
    #[rustc_const_stable(feature = "const_intrinsic_copy", since = "1.83.0")]
    pub const unsafe fn copy_to(self, dest: NonNull<T>, count: usize)
    where
        T: Sized,
    {
        // SAFETY: 调用方必须遵守 `copy` 的安全约定。
        unsafe { ptr::copy(self.as_ptr(), dest.as_ptr(), count) }
    }

    /// 将 `count * size_of::<T>()` 个字节从 `self` 拷贝到 `dest`。源与目标*不得*重叠。
    ///
    /// 注意:本方法的参数顺序与 [`ptr::copy_nonoverlapping`] *相同*。
    ///
    /// 安全方面的注意事项与示例参见 [`ptr::copy_nonoverlapping`]。
    ///
    /// [`ptr::copy_nonoverlapping`]: crate::ptr::copy_nonoverlapping()
    #[inline(always)]
    #[cfg_attr(miri, track_caller)] // 即便没有 panic,这也有助于 Miri 的回溯(backtrace)
    #[stable(feature = "non_null_convenience", since = "1.80.0")]
    #[rustc_const_stable(feature = "const_intrinsic_copy", since = "1.83.0")]
    pub const unsafe fn copy_to_nonoverlapping(self, dest: NonNull<T>, count: usize)
    where
        T: Sized,
    {
        // SAFETY: 调用方必须遵守 `copy_nonoverlapping` 的安全约定。
        unsafe { ptr::copy_nonoverlapping(self.as_ptr(), dest.as_ptr(), count) }
    }

    /// 将 `count * size_of::<T>()` 个字节从 `src` 拷贝到 `self`。源与目标可以重叠。
    ///
    /// 注意:本方法的参数顺序与 [`ptr::copy`] *相反*。
    ///
    /// 安全方面的注意事项与示例参见 [`ptr::copy`]。
    ///
    /// [`ptr::copy`]: crate::ptr::copy()
    #[inline(always)]
    #[cfg_attr(miri, track_caller)] // 即便没有 panic,这也有助于 Miri 的回溯(backtrace)
    #[stable(feature = "non_null_convenience", since = "1.80.0")]
    #[rustc_const_stable(feature = "const_intrinsic_copy", since = "1.83.0")]
    pub const unsafe fn copy_from(self, src: NonNull<T>, count: usize)
    where
        T: Sized,
    {
        // SAFETY: 调用方必须遵守 `copy` 的安全约定。
        unsafe { ptr::copy(src.as_ptr(), self.as_ptr(), count) }
    }

    /// 将 `count * size_of::<T>()` 个字节从 `src` 拷贝到 `self`。源与目标*不得*重叠。
    ///
    /// 注意:本方法的参数顺序与 [`ptr::copy_nonoverlapping`] *相反*。
    ///
    /// 安全方面的注意事项与示例参见 [`ptr::copy_nonoverlapping`]。
    ///
    /// [`ptr::copy_nonoverlapping`]: crate::ptr::copy_nonoverlapping()
    #[inline(always)]
    #[cfg_attr(miri, track_caller)] // 即便没有 panic,这也有助于 Miri 的回溯(backtrace)
    #[stable(feature = "non_null_convenience", since = "1.80.0")]
    #[rustc_const_stable(feature = "const_intrinsic_copy", since = "1.83.0")]
    pub const unsafe fn copy_from_nonoverlapping(self, src: NonNull<T>, count: usize)
    where
        T: Sized,
    {
        // SAFETY: 调用方必须遵守 `copy_nonoverlapping` 的安全约定。
        unsafe { ptr::copy_nonoverlapping(src.as_ptr(), self.as_ptr(), count) }
    }

    /// 执行被指向值的析构函数(如果有的话)。
    ///
    /// 安全方面的注意事项与示例参见 [`ptr::drop_in_place`]。
    ///
    /// [`ptr::drop_in_place`]: crate::ptr::drop_in_place()
    #[inline(always)]
    #[stable(feature = "non_null_convenience", since = "1.80.0")]
    #[rustc_const_unstable(feature = "const_drop_in_place", issue = "109342")]
    pub const unsafe fn drop_in_place(self)
    where
        T: [const] Destruct,
    {
        // SAFETY: 调用方必须遵守 `drop_in_place` 的安全约定。
        unsafe { ptr::drop_in_place(self.as_ptr()) }
    }

    /// 用给定的值覆写某个内存位置,既不读取也不 drop 旧值。
    ///
    /// 安全方面的注意事项与示例参见 [`ptr::write`]。
    ///
    /// [`ptr::write`]: crate::ptr::write()
    #[inline(always)]
    #[cfg_attr(miri, track_caller)] // 即便没有 panic,这也有助于 Miri 的回溯(backtrace)
    #[stable(feature = "non_null_convenience", since = "1.80.0")]
    #[rustc_const_stable(feature = "const_ptr_write", since = "1.83.0")]
    pub const unsafe fn write(self, val: T)
    where
        T: Sized,
    {
        // SAFETY: 调用方必须遵守 `write` 的安全约定。
        unsafe { ptr::write(self.as_ptr(), val) }
    }

    /// 在指定的指针上调用 memset,把从 `self` 开始的 `count * size_of::<T>()` 个字节
    /// 全部设为 `val`。
    ///
    /// 安全方面的注意事项与示例参见 [`ptr::write_bytes`]。
    ///
    /// [`ptr::write_bytes`]: crate::ptr::write_bytes()
    #[inline(always)]
    #[doc(alias = "memset")]
    #[cfg_attr(miri, track_caller)] // 即便没有 panic,这也有助于 Miri 的回溯(backtrace)
    #[stable(feature = "non_null_convenience", since = "1.80.0")]
    #[rustc_const_stable(feature = "const_ptr_write", since = "1.83.0")]
    pub const unsafe fn write_bytes(self, val: u8, count: usize)
    where
        T: Sized,
    {
        // SAFETY: 调用方必须遵守 `write_bytes` 的安全约定。
        unsafe { ptr::write_bytes(self.as_ptr(), val, count) }
    }

    /// 用给定的值对某个内存位置执行 volatile(易变)写入,既不读取也不 drop 旧值。
    ///
    /// volatile 操作意在作用于 I/O 内存,并保证编译器不会把它们消除,也不会让它们与其他
    /// volatile 操作发生重排。
    ///
    /// 安全方面的注意事项与示例参见 [`ptr::write_volatile`]。
    ///
    /// [`ptr::write_volatile`]: crate::ptr::write_volatile()
    #[inline(always)]
    #[cfg_attr(miri, track_caller)] // 即便没有 panic,这也有助于 Miri 的回溯(backtrace)
    #[stable(feature = "non_null_convenience", since = "1.80.0")]
    pub unsafe fn write_volatile(self, val: T)
    where
        T: Sized,
    {
        // SAFETY: 调用方必须遵守 `write_volatile` 的安全约定。
        unsafe { ptr::write_volatile(self.as_ptr(), val) }
    }

    /// 用给定的值覆写某个内存位置,既不读取也不 drop 旧值。
    ///
    /// 与 `write` 不同,本方法允许指针未对齐(unaligned)。
    ///
    /// 安全方面的注意事项与示例参见 [`ptr::write_unaligned`]。
    ///
    /// [`ptr::write_unaligned`]: crate::ptr::write_unaligned()
    #[inline(always)]
    #[cfg_attr(miri, track_caller)] // 即便没有 panic,这也有助于 Miri 的回溯(backtrace)
    #[stable(feature = "non_null_convenience", since = "1.80.0")]
    #[rustc_const_stable(feature = "const_ptr_write", since = "1.83.0")]
    pub const unsafe fn write_unaligned(self, val: T)
    where
        T: Sized,
    {
        // SAFETY: 调用方必须遵守 `write_unaligned` 的安全约定。
        unsafe { ptr::write_unaligned(self.as_ptr(), val) }
    }

    /// 用 `src` 替换 `self` 处的值,返回旧值,二者都不会被 drop。
    ///
    /// 安全方面的注意事项与示例参见 [`ptr::replace`]。
    ///
    /// [`ptr::replace`]: crate::ptr::replace()
    #[inline(always)]
    #[stable(feature = "non_null_convenience", since = "1.80.0")]
    #[rustc_const_stable(feature = "const_inherent_ptr_replace", since = "1.88.0")]
    pub const unsafe fn replace(self, src: T) -> T
    where
        T: Sized,
    {
        // SAFETY: 调用方必须遵守 `replace` 的安全约定。
        unsafe { ptr::replace(self.as_ptr(), src) }
    }

    /// 交换两个同类型可变内存位置上的值,二者都不会被反初始化(deinitialize)。它们可以
    /// 重叠,这一点与在其他方面等价的 `mem::swap` 不同。
    ///
    /// 安全方面的注意事项与示例参见 [`ptr::swap`]。
    ///
    /// [`ptr::swap`]: crate::ptr::swap()
    #[inline(always)]
    #[stable(feature = "non_null_convenience", since = "1.80.0")]
    #[rustc_const_stable(feature = "const_swap", since = "1.85.0")]
    pub const unsafe fn swap(self, with: NonNull<T>)
    where
        T: Sized,
    {
        // SAFETY: 调用方必须遵守 `swap` 的安全约定。
        unsafe { ptr::swap(self.as_ptr(), with.as_ptr()) }
    }

    /// 计算为使指针对齐到 `align`,需要施加给它的偏移量。
    ///
    /// 如果无法使指针对齐,实现会返回 `usize::MAX`。
    ///
    /// 偏移量以 `T` 元素的个数表示,而非字节数。
    ///
    /// 本方法绝不保证按此偏移指针后不会溢出、也不会越出指针所指向的 allocation。确保返回
    /// 的偏移量在对齐以外的所有方面都正确,是调用方的责任。
    ///
    /// 在编译期求值(目前 unstable)期间调用本方法时,实现可能在“运行期绝不会发生”的情形
    /// 下返回 `usize::MAX`。这是因为指针的实际对齐在编译期尚不可知,因此有时无法计算出一个
    /// 保证对齐的偏移量。例如,一个声明为 `[u8; N]` 的缓冲区可能被分配在奇地址或偶地址上,
    /// 但编译期还不知道是哪一种,因此执行必须对两种选择都正确。于是就不可能找到一个保证
    /// 2 字节对齐的偏移量。(与所有 unstable API 一样,这一行为可能会变化。)
    ///
    /// # Panics
    ///
    /// 如果 `align` 不是 2 的幂,本函数会 panic。
    ///
    /// # 示例
    ///
    /// 把相邻的若干 `u8` 当作 `u16` 来访问:
    ///
    /// ```
    /// use std::ptr::NonNull;
    ///
    /// # unsafe {
    /// let x = [5_u8, 6, 7, 8, 9];
    /// let ptr = NonNull::new(x.as_ptr() as *mut u8).unwrap();
    /// let offset = ptr.align_offset(align_of::<u16>());
    ///
    /// if offset < x.len() - 1 {
    ///     let u16_ptr = ptr.add(offset).cast::<u16>();
    ///     assert!(u16_ptr.read() == u16::from_ne_bytes([5, 6]) || u16_ptr.read() == u16::from_ne_bytes([6, 7]));
    /// } else {
    ///     // 虽然指针可以通过 `offset` 被对齐,但对齐后它会
    ///     // 指向 allocation 之外
    /// }
    /// # }
    /// ```
    #[inline]
    #[must_use]
    #[stable(feature = "non_null_convenience", since = "1.80.0")]
    pub fn align_offset(self, align: usize) -> usize
    where
        T: Sized,
    {
        if !align.is_power_of_two() {
            panic!("align_offset: align is not a power-of-two");
        }

        {
            // SAFETY: 上面已检查过 `align` 是 2 的幂。
            unsafe { ptr::align_offset(self.as_ptr(), align) }
        }
    }

    /// 返回该指针是否对 `T` 正确对齐。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::ptr::NonNull;
    ///
    /// // 在某些平台上,i32 的对齐小于 4。
    /// #[repr(align(4))]
    /// struct AlignedI32(i32);
    ///
    /// let data = AlignedI32(42);
    /// let ptr = NonNull::<AlignedI32>::from(&data);
    ///
    /// assert!(ptr.is_aligned());
    /// assert!(!NonNull::new(ptr.as_ptr().wrapping_byte_add(1)).unwrap().is_aligned());
    /// ```
    #[inline]
    #[must_use]
    #[stable(feature = "pointer_is_aligned", since = "1.79.0")]
    pub fn is_aligned(self) -> bool
    where
        T: Sized,
    {
        self.as_ptr().is_aligned()
    }

    /// 返回该指针是否对齐到 `align`。
    ///
    /// 对于非 `Sized` 的被指对象(pointee),本操作只考虑数据指针,忽略元数据(metadata)。
    ///
    /// # Panics
    ///
    /// 如果 `align` 不是 2 的幂(这也包括 0),本函数会 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(pointer_is_aligned_to)]
    ///
    /// // 在某些平台上,i32 的对齐小于 4。
    /// #[repr(align(4))]
    /// struct AlignedI32(i32);
    ///
    /// let data = AlignedI32(42);
    /// let ptr = &data as *const AlignedI32;
    ///
    /// assert!(ptr.is_aligned_to(1));
    /// assert!(ptr.is_aligned_to(2));
    /// assert!(ptr.is_aligned_to(4));
    ///
    /// assert!(ptr.wrapping_byte_add(2).is_aligned_to(2));
    /// assert!(!ptr.wrapping_byte_add(2).is_aligned_to(4));
    ///
    /// assert_ne!(ptr.is_aligned_to(8), ptr.wrapping_add(1).is_aligned_to(8));
    /// ```
    #[inline]
    #[must_use]
    #[unstable(feature = "pointer_is_aligned_to", issue = "96284")]
    pub fn is_aligned_to(self, align: usize) -> bool {
        self.as_ptr().is_aligned_to(align)
    }
}

impl<T> NonNull<T> {
    /// 从某类型转换为它的“可能未初始化(maybe-uninitialized)”版本。
    #[must_use]
    #[inline(always)]
    #[unstable(feature = "cast_maybe_uninit", issue = "145036")]
    pub const fn cast_uninit(self) -> NonNull<MaybeUninit<T>> {
        self.cast()
    }
}
impl<T> NonNull<MaybeUninit<T>> {
    /// 从“可能未初始化(maybe-uninitialized)”类型转换为它的已初始化版本。
    ///
    /// 这总是安全的,因为只有当指针在初始化之前被读取时才可能发生 UB。
    #[must_use]
    #[inline(always)]
    #[unstable(feature = "cast_maybe_uninit", issue = "145036")]
    pub const fn cast_init(self) -> NonNull<T> {
        self.cast()
    }
}

impl<T> NonNull<[T]> {
    /// 从一个细指针(thin pointer)和一个长度创建一个非空的裸切片指针。
    ///
    /// `len` 参数是**元素**的个数,而非字节数。
    ///
    /// 本函数是安全的,但解引用其返回值是 unsafe 的。切片的安全要求参见
    /// [`slice::from_raw_parts`] 的文档。
    ///
    /// # 示例
    ///
    /// ```rust
    /// use std::ptr::NonNull;
    ///
    /// // 当手头只有指向首元素的指针时,据此创建一个切片指针
    /// let mut x = [5, 6, 7];
    /// let nonnull_pointer = NonNull::new(x.as_mut_ptr()).unwrap();
    /// let slice = NonNull::slice_from_raw_parts(nonnull_pointer, 3);
    /// assert_eq!(unsafe { slice.as_ref()[2] }, 7);
    /// ```
    ///
    /// (注意这个示例只是人为地演示本方法的用法,实际写这类代码时
    /// `let slice = NonNull::from(&x[..]);` 是更好的写法。)
    #[stable(feature = "nonnull_slice_from_raw_parts", since = "1.70.0")]
    #[rustc_const_stable(feature = "const_slice_from_raw_parts_mut", since = "1.83.0")]
    #[must_use]
    #[inline]
    pub const fn slice_from_raw_parts(data: NonNull<T>, len: usize) -> Self {
        // SAFETY: `data` 是一个 `NonNull` 指针,因而必然非空。
        unsafe { Self::new_unchecked(super::slice_from_raw_parts_mut(data.as_ptr(), len)) }
    }

    /// 返回非空裸切片的长度。
    ///
    /// 返回值是**元素**的个数,而非字节数。
    ///
    /// 本函数是安全的,即便该非空裸切片因指针地址无效而无法被解引用为一个切片。
    ///
    /// # 示例
    ///
    /// ```rust
    /// use std::ptr::NonNull;
    ///
    /// let slice: NonNull<[i8]> = NonNull::slice_from_raw_parts(NonNull::dangling(), 3);
    /// assert_eq!(slice.len(), 3);
    /// ```
    #[stable(feature = "slice_ptr_len_nonnull", since = "1.63.0")]
    #[rustc_const_stable(feature = "const_slice_ptr_len_nonnull", since = "1.63.0")]
    #[must_use]
    #[inline]
    pub const fn len(self) -> usize {
        self.as_ptr().len()
    }

    /// 如果非空裸切片的长度为 0,返回 `true`。
    ///
    /// # 示例
    ///
    /// ```rust
    /// use std::ptr::NonNull;
    ///
    /// let slice: NonNull<[i8]> = NonNull::slice_from_raw_parts(NonNull::dangling(), 3);
    /// assert!(!slice.is_empty());
    /// ```
    #[stable(feature = "slice_ptr_is_empty_nonnull", since = "1.79.0")]
    #[rustc_const_stable(feature = "const_slice_ptr_is_empty_nonnull", since = "1.79.0")]
    #[must_use]
    #[inline]
    pub const fn is_empty(self) -> bool {
        self.len() == 0
    }

    /// 返回指向该切片缓冲区的非空指针。
    ///
    /// # 示例
    ///
    /// ```rust
    /// #![feature(slice_ptr_get)]
    /// use std::ptr::NonNull;
    ///
    /// let slice: NonNull<[i8]> = NonNull::slice_from_raw_parts(NonNull::dangling(), 3);
    /// assert_eq!(slice.as_non_null_ptr(), NonNull::<i8>::dangling());
    /// ```
    #[inline]
    #[must_use]
    #[unstable(feature = "slice_ptr_get", issue = "74265")]
    pub const fn as_non_null_ptr(self) -> NonNull<T> {
        self.cast()
    }

    /// 返回指向该切片缓冲区的裸指针。
    ///
    /// # 示例
    ///
    /// ```rust
    /// #![feature(slice_ptr_get)]
    /// use std::ptr::NonNull;
    ///
    /// let slice: NonNull<[i8]> = NonNull::slice_from_raw_parts(NonNull::dangling(), 3);
    /// assert_eq!(slice.as_mut_ptr(), NonNull::<i8>::dangling().as_ptr());
    /// ```
    #[inline]
    #[must_use]
    #[unstable(feature = "slice_ptr_get", issue = "74265")]
    #[rustc_never_returns_null_ptr]
    pub const fn as_mut_ptr(self) -> *mut T {
        self.as_non_null_ptr().as_ptr()
    }

    /// 返回指向一段可能未初始化值的共享切片引用。与 [`as_ref`] 不同,本方法不要求该值
    /// 已经初始化。
    ///
    /// 可变版本参见 [`as_uninit_slice_mut`]。
    ///
    /// [`as_ref`]: NonNull::as_ref
    /// [`as_uninit_slice_mut`]: NonNull::as_uninit_slice_mut
    ///
    /// # 安全性(Safety）
    ///
    /// 调用本方法时,调用方必须保证以下各项全部成立:
    ///
    /// * 指针必须对读取(reads)[有效][valid],可读 `ptr.len() * size_of::<T>()` 个字节,
    ///   且必须已正确对齐。具体而言:
    ///
    ///     * 该切片的整段内存范围必须包含在单个 allocation 之内!切片绝不能跨越多个
    ///       allocation。
    ///
    ///     * 即便是零长度切片,指针也必须对齐。原因之一是:enum 的布局优化可能依赖于引用
    ///       (包括任意长度的切片)是对齐且非空的,以此把它们与其他数据区分开。可以用
    ///       [`NonNull::dangling()`] 获取一个可用作零长度切片 `data` 的指针。
    ///
    /// * 切片的总大小 `ptr.len() * size_of::<T>()` 不得大于 `isize::MAX`。
    ///   参见 [`pointer::offset`] 的安全文档。
    ///
    /// * 你必须遵守 Rust 的别名(aliasing)规则,因为返回的生命周期 `'a` 是任意选择的,
    ///   未必反映数据的实际生命周期。特别地,在该引用存活期间,指针所指的内存不得被改写
    ///   (除非位于 `UnsafeCell` 内部)。
    ///
    /// 即便本方法的结果未被使用,以上要求依然适用!
    ///
    /// 另请参见 [`slice::from_raw_parts`]。
    ///
    /// [valid]: crate::ptr#safety
    #[inline]
    #[must_use]
    #[unstable(feature = "ptr_as_uninit", issue = "75402")]
    pub const unsafe fn as_uninit_slice<'a>(self) -> &'a [MaybeUninit<T>] {
        // SAFETY: 调用方必须遵守 `as_uninit_slice` 的安全约定。
        unsafe { slice::from_raw_parts(self.cast().as_ptr(), self.len()) }
    }

    /// 返回指向一段可能未初始化值的独占(unique)切片引用。与 [`as_mut`] 不同,本方法
    /// 不要求该值已经初始化。
    ///
    /// 共享版本参见 [`as_uninit_slice`]。
    ///
    /// [`as_mut`]: NonNull::as_mut
    /// [`as_uninit_slice`]: NonNull::as_uninit_slice
    ///
    /// # 安全性(Safety）
    ///
    /// 调用本方法时,调用方必须保证以下各项全部成立:
    ///
    /// * 指针必须对读取与写入(reads and writes)[有效][valid],可读写
    ///   `ptr.len() * size_of::<T>()` 个字节,且必须已正确对齐。具体而言:
    ///
    ///     * 该切片的整段内存范围必须包含在单个 allocation 之内!切片绝不能跨越多个
    ///       allocation。
    ///
    ///     * 即便是零长度切片,指针也必须对齐。原因之一是:enum 的布局优化可能依赖于引用
    ///       (包括任意长度的切片)是对齐且非空的,以此把它们与其他数据区分开。可以用
    ///       [`NonNull::dangling()`] 获取一个可用作零长度切片 `data` 的指针。
    ///
    /// * 切片的总大小 `ptr.len() * size_of::<T>()` 不得大于 `isize::MAX`。
    ///   参见 [`pointer::offset`] 的安全文档。
    ///
    /// * 你必须遵守 Rust 的别名(aliasing)规则,因为返回的生命周期 `'a` 是任意选择的,
    ///   未必反映数据的实际生命周期。特别地,在该引用存活期间,指针所指的内存不得通过
    ///   任何其他指针被访问(读或写)。
    ///
    /// 即便本方法的结果未被使用,以上要求依然适用!
    ///
    /// 另请参见 [`slice::from_raw_parts_mut`]。
    ///
    /// [valid]: crate::ptr#safety
    ///
    /// # 示例
    ///
    /// ```rust
    /// #![feature(allocator_api, ptr_as_uninit)]
    ///
    /// use std::alloc::{Allocator, Layout, Global};
    /// use std::mem::MaybeUninit;
    /// use std::ptr::NonNull;
    ///
    /// let memory: NonNull<[u8]> = Global.allocate(Layout::new::<[u8; 32]>())?;
    /// // 这是安全的,因为 `memory` 对其 `memory.len()` 个字节的读取与写入都有效。
    /// // 注意:这里不允许调用 `memory.as_mut()`,因为其内容可能尚未初始化。
    /// # #[allow(unused_variables)]
    /// let slice: &mut [MaybeUninit<u8>] = unsafe { memory.as_uninit_slice_mut() };
    /// # // 防止在 Miri 下发生内存泄漏。
    /// # unsafe { Global.deallocate(memory.cast(), Layout::new::<[u8; 32]>()); }
    /// # Ok::<_, std::alloc::AllocError>(())
    /// ```
    #[inline]
    #[must_use]
    #[unstable(feature = "ptr_as_uninit", issue = "75402")]
    pub const unsafe fn as_uninit_slice_mut<'a>(self) -> &'a mut [MaybeUninit<T>] {
        // SAFETY: 调用方必须遵守 `as_uninit_slice_mut` 的安全约定。
        unsafe { slice::from_raw_parts_mut(self.cast().as_ptr(), self.len()) }
    }

    /// 返回指向某个元素或子切片的裸指针,不做边界检查。
    ///
    /// 用越界的索引调用本方法,或在 `self` 不可解引用(dereferenceable)时调用本方法,
    /// 都是*[未定义行为][undefined behavior]*,即便所得到的指针并未被使用。
    ///
    /// [undefined behavior]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(slice_ptr_get)]
    /// use std::ptr::NonNull;
    ///
    /// let x = &mut [1, 2, 4];
    /// let x = NonNull::slice_from_raw_parts(NonNull::new(x.as_mut_ptr()).unwrap(), x.len());
    ///
    /// unsafe {
    ///     assert_eq!(x.get_unchecked_mut(1).as_ptr(), x.as_non_null_ptr().as_ptr().add(1));
    /// }
    /// ```
    #[unstable(feature = "slice_ptr_get", issue = "74265")]
    #[rustc_const_unstable(feature = "const_index", issue = "143775")]
    #[inline]
    pub const unsafe fn get_unchecked_mut<I>(self, index: I) -> NonNull<I::Output>
    where
        I: [const] SliceIndex<[T]>,
    {
        // SAFETY: 调用方保证 `self` 可解引用且 `index` 在边界内(in-bounds)。
        // 由此可知,所得到的指针不可能为 null。
        unsafe { NonNull::new_unchecked(self.as_ptr().get_unchecked_mut(index)) }
    }
}

#[stable(feature = "nonnull", since = "1.25.0")]
impl<T: PointeeSized> Clone for NonNull<T> {
    #[inline(always)]
    fn clone(&self) -> Self {
        *self
    }
}

#[stable(feature = "nonnull", since = "1.25.0")]
impl<T: PointeeSized> Copy for NonNull<T> {}

#[doc(hidden)]
#[unstable(feature = "trivial_clone", issue = "none")]
unsafe impl<T: PointeeSized> TrivialClone for NonNull<T> {}

#[unstable(feature = "coerce_unsized", issue = "18598")]
impl<T: PointeeSized, U: PointeeSized> CoerceUnsized<NonNull<U>> for NonNull<T> where T: Unsize<U> {}

#[unstable(feature = "dispatch_from_dyn", issue = "none")]
impl<T: PointeeSized, U: PointeeSized> DispatchFromDyn<NonNull<U>> for NonNull<T> where T: Unsize<U> {}

#[stable(feature = "pin", since = "1.33.0")]
unsafe impl<T: PointeeSized> PinCoerceUnsized for NonNull<T> {}

#[stable(feature = "nonnull", since = "1.25.0")]
impl<T: PointeeSized> fmt::Debug for NonNull<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Pointer::fmt(&self.as_ptr(), f)
    }
}

#[stable(feature = "nonnull", since = "1.25.0")]
impl<T: PointeeSized> fmt::Pointer for NonNull<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Pointer::fmt(&self.as_ptr(), f)
    }
}

#[stable(feature = "nonnull", since = "1.25.0")]
impl<T: PointeeSized> Eq for NonNull<T> {}

#[stable(feature = "nonnull", since = "1.25.0")]
impl<T: PointeeSized> PartialEq for NonNull<T> {
    #[inline]
    #[allow(ambiguous_wide_pointer_comparisons)]
    fn eq(&self, other: &Self) -> bool {
        self.as_ptr() == other.as_ptr()
    }
}

#[stable(feature = "nonnull", since = "1.25.0")]
impl<T: PointeeSized> Ord for NonNull<T> {
    #[inline]
    #[allow(ambiguous_wide_pointer_comparisons)]
    fn cmp(&self, other: &Self) -> Ordering {
        self.as_ptr().cmp(&other.as_ptr())
    }
}

#[stable(feature = "nonnull", since = "1.25.0")]
impl<T: PointeeSized> PartialOrd for NonNull<T> {
    #[inline]
    #[allow(ambiguous_wide_pointer_comparisons)]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.as_ptr().partial_cmp(&other.as_ptr())
    }
}

#[stable(feature = "nonnull", since = "1.25.0")]
impl<T: PointeeSized> hash::Hash for NonNull<T> {
    #[inline]
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.as_ptr().hash(state)
    }
}

#[unstable(feature = "ptr_internals", issue = "none")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl<T: PointeeSized> const From<Unique<T>> for NonNull<T> {
    #[inline]
    fn from(unique: Unique<T>) -> Self {
        unique.as_non_null_ptr()
    }
}

#[stable(feature = "nonnull", since = "1.25.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl<T: PointeeSized> const From<&mut T> for NonNull<T> {
    /// 把 `&mut T` 转换为 `NonNull<T>`。
    ///
    /// 这个转换安全且不会失败,因为引用不可能为 null。
    #[inline]
    fn from(r: &mut T) -> Self {
        NonNull::from_mut(r)
    }
}

#[stable(feature = "nonnull", since = "1.25.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl<T: PointeeSized> const From<&T> for NonNull<T> {
    /// 把 `&T` 转换为 `NonNull<T>`。
    ///
    /// 这个转换安全且不会失败,因为引用不可能为 null。
    #[inline]
    fn from(r: &T) -> Self {
        NonNull::from_ref(r)
    }
}
