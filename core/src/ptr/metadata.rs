#![unstable(feature = "ptr_metadata", issue = "81513")]

use crate::clone::TrivialClone;
use crate::fmt;
use crate::hash::{Hash, Hasher};
use crate::intrinsics::{aggregate_raw_ptr, ptr_metadata};
use crate::marker::{Freeze, PointeeSized};
use crate::ptr::NonNull;

/// 提供任意被指向类型(pointed-to type)的指针元数据(metadata)类型。
///
/// # 指针元数据(Pointer metadata）
///
/// Rust 中的裸指针类型与引用类型可以被看作由两部分组成:一个**数据指针**
/// (data pointer),它保存值的内存地址;以及一些**元数据**(metadata)。
///
/// 对于静态确定大小的类型(实现了 `Sized` trait 的类型)以及 `extern` 类型,
/// 指针被称为"瘦指针"(thin):元数据是零大小的,其类型为 `()`。
///
/// 指向[动态大小类型(DST)][dst]的指针被称为"宽指针"或"胖指针"(wide / fat),
/// 它们带有非零大小的元数据:
///
/// * 对于最后一个字段是 DST 的结构体,元数据就是其最后一个字段的元数据;
/// * 对于 `str` 类型,元数据是以字节计的长度,类型为 `usize`;
/// * 对于像 `[T]` 这样的切片类型,元数据是以元素个数计的长度,类型为 `usize`;
/// * 对于像 `dyn SomeTrait` 这样的 trait 对象(trait object),元数据是
///   [`DynMetadata<Self>`][DynMetadata](例如 `DynMetadata<dyn SomeTrait>`),
///   即指向虚表(vtable)的指针。
///
/// 将来,Rust 语言可能会引入带有不同指针元数据的新种类的类型。
///
/// [dst]: https://doc.rust-lang.org/nomicon/exotic-sizes.html#dynamically-sized-types-dsts
///
///
/// # `Pointee` trait
///
/// 这个 trait 的意义在于它的关联类型 `Metadata`,如上所述,它是 `()`、`usize`
/// 或 `DynMetadata<_>`。它会**自动为每一个类型实现**。即便没有相应的 trait 约束,
/// 也可以在泛型上下文中假定它已被实现。
///
///
/// # 用法(Usage）
///
/// 裸指针可以通过其 [`to_raw_parts`] 方法被拆解为数据指针与元数据两个分量。
///
/// 或者,也可以用 [`metadata`] 函数单独提取出元数据。可以把一个引用传给
/// [`metadata`],它会被隐式强转(coerce)。
///
/// 一个(可能是胖的)指针可以用 [`from_raw_parts`] 或 [`from_raw_parts_mut`]
/// 从其数据指针与元数据重新组装出来。
///
/// [`to_raw_parts`]: *const::to_raw_parts
#[lang = "pointee_trait"]
#[rustc_deny_explicit_impl]
#[rustc_do_not_implement_via_object]
pub trait Pointee: PointeeSized {
    /// 指向 `Self` 的指针与引用所使用的元数据类型。
    #[lang = "metadata_type"]
    // 注意:请保持 `library/core/src/ptr/metadata.rs` 中
    // `static_assert_expected_bounds_for_metadata` 里的 trait 约束与此处一致:
    // 注意:`dyn Trait + 'a` 的元数据是 `DynMetadata<dyn Trait + 'a>`,
    // 因此**不能**额外加上 `'static` 约束。
    type Metadata: fmt::Debug + Copy + Send + Sync + Ord + Hash + Unpin + Freeze;
}

/// 指向实现了此 trait 别名(trait alias)的类型的指针是"瘦指针"(thin)。
///
/// 这包括静态 `Sized` 类型以及 `extern` 类型。
///
/// # 示例
///
/// ```rust
/// #![feature(ptr_metadata)]
///
/// fn this_never_panics<T: std::ptr::Thin>() {
///     assert_eq!(size_of::<&T>(), size_of::<usize>())
/// }
/// ```
#[unstable(feature = "ptr_metadata", issue = "81513")]
// 注意:在语言层面的 trait 别名(trait aliases)稳定之前,不要稳定化它?
pub trait Thin = Pointee<Metadata = ()> + PointeeSized;

/// 提取指针的元数据(metadata)分量。
///
/// 类型为 `*mut T`、`&T` 或 `&mut T` 的值可以直接传给此函数,因为它们会被隐式
/// 强转(coerce)为 `*const T`。
///
/// # 示例
///
/// ```
/// #![feature(ptr_metadata)]
///
/// assert_eq!(std::ptr::metadata("foo"), 3_usize);
/// ```
#[inline]
pub const fn metadata<T: PointeeSized>(ptr: *const T) -> <T as Pointee>::Metadata {
    ptr_metadata(ptr)
}

/// 由一个数据指针和一份元数据构造出一个(可能是胖的)裸指针。
///
/// 此函数本身是安全的,但返回的指针**不一定**可以安全地解引用。对于切片,其安全性
/// 要求见 [`slice::from_raw_parts`] 的文档。对于 trait 对象,元数据必须来自指向
/// 同一底层被擦除类型(erased type)的指针。
///
/// 如果你想在泛型上下文中拆解一个 DST 以便稍后重建,可以通过把 `*const T` 转换为
/// `*const ()` 来获得一个瘦指针(thin pointer)。
///
/// [`slice::from_raw_parts`]: crate::slice::from_raw_parts
#[unstable(feature = "ptr_metadata", issue = "81513")]
#[inline]
pub const fn from_raw_parts<T: PointeeSized>(
    data_pointer: *const impl Thin,
    metadata: <T as Pointee>::Metadata,
) -> *const T {
    aggregate_raw_ptr(data_pointer, metadata)
}

/// 执行与 [`from_raw_parts`] 相同的功能,区别在于返回的是裸 `*mut` 指针,而非裸
/// `*const` 指针。
///
/// 更多细节见 [`from_raw_parts`] 的文档。
#[unstable(feature = "ptr_metadata", issue = "81513")]
#[inline]
pub const fn from_raw_parts_mut<T: PointeeSized>(
    data_pointer: *mut impl Thin,
    metadata: <T as Pointee>::Metadata,
) -> *mut T {
    aggregate_raw_ptr(data_pointer, metadata)
}

/// `Dyn = dyn SomeTrait` 这种 trait 对象类型的元数据(metadata)。
///
/// 它是一个指向**虚表**(vtable,virtual call table)的指针,该虚表囊括了操纵
/// trait 对象内部所存储的具体类型(concrete type)所需的全部信息。虚表中尤其
/// 包含:
///
/// * 类型的大小(size);
/// * 类型的对齐(alignment);
/// * 一个指向该类型 `drop_in_place` 实现的指针(对于纯数据 POD 类型,它可能是
///   空操作);
/// * 指向该类型对此 trait 的实现中所有方法的指针。
///
/// 注意:前三项是特殊的,因为分配、丢弃(drop)与释放任意 trait 对象都需要它们。
///
/// 可以用一个并非 `dyn` trait 对象的类型参数来命名这个结构体(例如
/// `DynMetadata<u64>`),但无法获得该结构体的有意义的值。
///
/// 注意:虽然此类型实现了 `PartialEq`,但比较虚表指针是不可靠的:针对同一类型对
/// 同一 trait 的虚表,其指针可能比较为不相等(因为虚表会在多个 codegen unit 中
/// 被复制);而针对**不同**类型/trait 的虚表,其指针又可能比较为相等(因为相同的
/// 虚表可能在一个 codegen unit 内被去重合并)。
#[lang = "dyn_metadata"]
pub struct DynMetadata<Dyn: PointeeSized> {
    _vtable_ptr: NonNull<VTable>,
    _phantom: crate::marker::PhantomData<Dyn>,
}

unsafe extern "C" {
    /// 用于访问虚表的不透明类型(opaque type)。
    ///
    /// 这是 `DynMetadata::size_of` 等方法的私有实现细节。从概念上讲,这个指针背后
    /// 其实并没有任何抽象机(Abstract Machine)意义上的内存。
    type VTable;
}

impl<Dyn: PointeeSized> DynMetadata<Dyn> {
    /// 当 `DynMetadata` 作为某个宽指针的元数据字段出现时,rustc_middle 的布局
    /// (layout)计算会施加"魔法":其结果布局**不是** `FieldsShape::Aggregate`,
    /// 而是 `FieldsShape::Primitive`。这意味着同一个类型,根据它是作为宽指针的
    /// 元数据字段出现、还是作为独立类型出现,会有不同的布局。这可以理解地会让
    /// codegen 困惑,并在尝试投影(project)到 `DynMetadata` 的某个字段时导致 ICE
    /// (编译器内部错误)。为绕过该问题,我们使用 `transmute` 而非字段投影。
    #[inline]
    fn vtable_ptr(self) -> *const VTable {
        // SAFETY: 这条布局假设被硬编码在编译器中。
        // 如果它出于某种原因大小不匹配,transmute 会报错。
        unsafe { crate::mem::transmute::<Self, *const VTable>(self) }
    }

    /// 返回与此虚表相关联的类型的大小(size)。
    #[inline]
    pub fn size_of(self) -> usize {
        // 注意:"存储在虚表中的大小"与 "size_of_val_raw 的结果"**并不**相同。
        // 考虑像 `&(i32, dyn Send)` 这样的引用:虚表中只会存储 `Send` 这一部分的大小!
        // SAFETY: DynMetadata 始终包含一个有效的虚表指针。
        unsafe { crate::intrinsics::vtable_size(self.vtable_ptr() as *const ()) }
    }

    /// 返回与此虚表相关联的类型的对齐(alignment)。
    #[inline]
    pub fn align_of(self) -> usize {
        // SAFETY: DynMetadata 始终包含一个有效的虚表指针。
        unsafe { crate::intrinsics::vtable_align(self.vtable_ptr() as *const ()) }
    }

    /// 将大小与对齐一并作为一个 `Layout` 返回。
    #[inline]
    pub fn layout(self) -> crate::alloc::Layout {
        // SAFETY: 编译器是为一个具体的 Rust 类型发出(emit)这个虚表的,而该类型
        // 已知具有有效的布局。其理由与 `Layout::for_value` 中相同。
        unsafe { crate::alloc::Layout::from_size_align_unchecked(self.size_of(), self.align_of()) }
    }
}

unsafe impl<Dyn: PointeeSized> Send for DynMetadata<Dyn> {}
unsafe impl<Dyn: PointeeSized> Sync for DynMetadata<Dyn> {}

impl<Dyn: PointeeSized> fmt::Debug for DynMetadata<Dyn> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("DynMetadata").field(&self.vtable_ptr()).finish()
    }
}

// 为了避免引入 `Dyn: $Trait` 约束,这些 impl 需要手动书写。

impl<Dyn: PointeeSized> Unpin for DynMetadata<Dyn> {}

impl<Dyn: PointeeSized> Copy for DynMetadata<Dyn> {}

impl<Dyn: PointeeSized> Clone for DynMetadata<Dyn> {
    #[inline]
    fn clone(&self) -> Self {
        *self
    }
}

#[doc(hidden)]
unsafe impl<Dyn: PointeeSized> TrivialClone for DynMetadata<Dyn> {}

impl<Dyn: PointeeSized> Eq for DynMetadata<Dyn> {}

impl<Dyn: PointeeSized> PartialEq for DynMetadata<Dyn> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        crate::ptr::eq::<VTable>(self.vtable_ptr(), other.vtable_ptr())
    }
}

impl<Dyn: PointeeSized> Ord for DynMetadata<Dyn> {
    #[inline]
    #[allow(ambiguous_wide_pointer_comparisons)]
    fn cmp(&self, other: &Self) -> crate::cmp::Ordering {
        <*const VTable>::cmp(&self.vtable_ptr(), &other.vtable_ptr())
    }
}

impl<Dyn: PointeeSized> PartialOrd for DynMetadata<Dyn> {
    #[inline]
    fn partial_cmp(&self, other: &Self) -> Option<crate::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl<Dyn: PointeeSized> Hash for DynMetadata<Dyn> {
    #[inline]
    fn hash<H: Hasher>(&self, hasher: &mut H) {
        crate::ptr::hash::<VTable, _>(self.vtable_ptr(), hasher)
    }
}
