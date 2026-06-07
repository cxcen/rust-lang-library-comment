use crate::marker::{PointeeSized, Unsize};

/// 这个 trait 表明:此类型是一个指针、或是对指针的包装,并且可以对其指向的对象
/// (pointee)执行 unsizing(去尺寸化,即把已知大小类型转为 DST)。
///
/// 更多细节参见 [DST coercion RFC][dst-coerce] 以及 [Nomicon 中关于强转的条目][nomicon-coerce]。
///
/// 对于内建指针类型,若 `T: Unsize<U>`,则指向 `T` 的指针会强转为指向 `U` 的指针,
/// 其做法是从瘦指针(thin pointer)转换为胖指针(fat pointer)。
///
/// 对于自定义类型,这里的强转通过把 `Foo<T>` 强转为 `Foo<U>` 来实现,前提是存在
/// 一个 `CoerceUnsized<Foo<U>> for Foo<T>` 的实现。只有当 `Foo<T>` 仅有单个非
/// phantomdata、且涉及 `T` 的字段时,才能写出这样的实现。如果那个字段的类型是
/// `Bar<T>`,那么必须存在一个 `CoerceUnsized<Bar<U>> for Bar<T>` 的实现。强转的
/// 做法是:把那个 `Bar<T>` 字段强转为 `Bar<U>`,并把 `Foo<T>` 其余的字段填进去,
/// 从而构造出一个 `Foo<U>`。这实际上会层层向下,最终钻到一个指针字段并对其进行强转。
///
/// 一般而言,对于智能指针,你会实现
/// `CoerceUnsized<Ptr<U>> for Ptr<T> where T: Unsize<U>, U: ?Sized`,其中 `T`
/// 本身可以带一个可选的 `?Sized` 约束。对于像 `Cell<T>` 和 `RefCell<T>` 那样直接
/// 内嵌 `T` 的包装类型,你可以直接实现
/// `CoerceUnsized<Wrap<U>> for Wrap<T> where T: CoerceUnsized<U>`。这能让诸如
/// `Cell<Box<T>>` 之类的类型强转正常工作。
///
/// [`Unsize`][unsize] 用于标记那些“处于指针之后时可以被强转为 DST”的类型。它由
/// 编译器自动实现。
///
/// [dst-coerce]: https://github.com/rust-lang/rfcs/blob/master/text/0982-dst-coercion.md
/// [unsize]: crate::marker::Unsize
/// [nomicon-coerce]: ../../nomicon/coercions.html
#[unstable(feature = "coerce_unsized", issue = "18598")]
#[lang = "coerce_unsized"]
pub trait CoerceUnsized<T: PointeeSized> {
    // 空。
}

// &mut T -> &mut U
#[unstable(feature = "coerce_unsized", issue = "18598")]
impl<'a, T: PointeeSized + Unsize<U>, U: PointeeSized> CoerceUnsized<&'a mut U> for &'a mut T {}
// &mut T -> &U
#[unstable(feature = "coerce_unsized", issue = "18598")]
impl<'a, 'b: 'a, T: PointeeSized + Unsize<U>, U: PointeeSized> CoerceUnsized<&'a U> for &'b mut T {}
// &mut T -> *mut U
#[unstable(feature = "coerce_unsized", issue = "18598")]
impl<'a, T: PointeeSized + Unsize<U>, U: PointeeSized> CoerceUnsized<*mut U> for &'a mut T {}
// &mut T -> *const U
#[unstable(feature = "coerce_unsized", issue = "18598")]
impl<'a, T: PointeeSized + Unsize<U>, U: PointeeSized> CoerceUnsized<*const U> for &'a mut T {}

// &T -> &U
#[unstable(feature = "coerce_unsized", issue = "18598")]
impl<'a, 'b: 'a, T: PointeeSized + Unsize<U>, U: PointeeSized> CoerceUnsized<&'a U> for &'b T {}
// &T -> *const U
#[unstable(feature = "coerce_unsized", issue = "18598")]
impl<'a, T: PointeeSized + Unsize<U>, U: PointeeSized> CoerceUnsized<*const U> for &'a T {}

// *mut T -> *mut U
#[unstable(feature = "coerce_unsized", issue = "18598")]
impl<T: PointeeSized + Unsize<U>, U: PointeeSized> CoerceUnsized<*mut U> for *mut T {}
// *mut T -> *const U
#[unstable(feature = "coerce_unsized", issue = "18598")]
impl<T: PointeeSized + Unsize<U>, U: PointeeSized> CoerceUnsized<*const U> for *mut T {}

// *const T -> *const U
#[unstable(feature = "coerce_unsized", issue = "18598")]
impl<T: PointeeSized + Unsize<U>, U: PointeeSized> CoerceUnsized<*const U> for *const T {}

/// `DispatchFromDyn` 用在 dyn 兼容性[^1] 检查的实现中(尤其是允许任意 self 类型
/// 的场景),用来保证某个方法的接收者类型可以在其上进行动态分派(dispatch)。
///
/// 注意:`DispatchFromDyn` 曾一度被命名为 `CoerceSized`(且当时的解释略有不同)。
///
/// 设想我们有一个 trait 对象 `t`,其类型为 `&dyn Tr`,其中 `Tr` 是某个带有方法
/// `m`(定义为 `fn m(&self);`)的 trait。调用 `t.m()` 时,接收者 `t` 是一个宽指针
/// (wide pointer),但 `m` 的实现期望 `&self` 是一个窄指针(narrow pointer,即
/// 指向具体类型的引用)。编译器必须生成一个从 trait 对象 / 宽指针到具体引用 /
/// 窄指针的隐式转换。实现 `DispatchFromDyn` 表明允许这种转换,因而表明实现了
/// `DispatchFromDyn` 的类型可以安全地用作 dyn 兼容方法中的 self 类型。(在上面的
/// 例子中,编译器会要求 `&'a U` 实现 `DispatchFromDyn`。)
///
/// `DispatchFromDyn` 并不指定从宽指针到窄指针的具体转换方式;该转换是硬编码在
/// 编译器里的。为了让转换得以工作,下列性质必须成立(也就是说,只有具备这些性质
/// 的类型实现 `DispatchFromDyn` 才是安全的,编译器也会检查这些性质):
///
/// * 要么 `Self` 和 `T` 同为引用、或同为裸指针;且无论哪种情形,可变性都相同。
/// * 要么以下各项全部成立:
///   - `Self` 和 `T` 必须具有相同的类型构造器(type constructor),且仅在单个类型
///     参数(即 *被强转类型*,coerced type)上有所不同(例如
///     `impl DispatchFromDyn<Rc<T>> for Rc<U>` 是允许的,其单个类型参数(用 `T`
///     或 `U` 实例化)就是被强转类型;而 `impl DispatchFromDyn<Arc<T>> for Rc<U>`
///     则不允许)。
///   - `Self` 的定义必须是一个结构体。
///   - `Self` 的定义不得是 `#[repr(packed)]` 或 `#[repr(C)]`。
///   - 除了对齐为 1、大小为零的字段之外,`Self` 的定义必须恰好有一个字段,且该
///     字段的类型必须是被强转类型。此外,`Self` 的字段类型必须实现
///     `DispatchFromDyn<F>`,其中 `F` 是 `T` 的字段类型。
///
/// 该 trait 的一个实现示例:
///
/// ```
/// # #![feature(dispatch_from_dyn, unsize)]
/// # use std::{ops::DispatchFromDyn, marker::Unsize};
/// # struct Rc<T: ?Sized>(std::rc::Rc<T>);
/// impl<T: ?Sized, U: ?Sized> DispatchFromDyn<Rc<U>> for Rc<T>
/// where
///     T: Unsize<U>,
/// {}
/// ```
///
/// [^1]: 此前被称为 *对象安全(object safety)*。
#[unstable(feature = "dispatch_from_dyn", issue = "none")]
#[lang = "dispatch_from_dyn"]
pub trait DispatchFromDyn<T> {
    // 空。
}

// &T -> &U
#[unstable(feature = "dispatch_from_dyn", issue = "none")]
impl<'a, T: PointeeSized + Unsize<U>, U: PointeeSized> DispatchFromDyn<&'a U> for &'a T {}
// &mut T -> &mut U
#[unstable(feature = "dispatch_from_dyn", issue = "none")]
impl<'a, T: PointeeSized + Unsize<U>, U: PointeeSized> DispatchFromDyn<&'a mut U> for &'a mut T {}
// *const T -> *const U
#[unstable(feature = "dispatch_from_dyn", issue = "none")]
impl<T: PointeeSized + Unsize<U>, U: PointeeSized> DispatchFromDyn<*const U> for *const T {}
// *mut T -> *mut U
#[unstable(feature = "dispatch_from_dyn", issue = "none")]
impl<T: PointeeSized + Unsize<U>, U: PointeeSized> DispatchFromDyn<*mut U> for *mut T {}
