use crate::marker::PointeeSized;

/// 用于不可变解引用操作,例如 `*v`。
///
/// 除了用于在不可变上下文中通过(一元)`*` 运算符进行显式解引用外,`Deref`
/// 还会在许多场合被编译器隐式使用。这一机制被称为 ["`Deref` coercion"][coercion]
/// (`Deref` 强制转换 / 自动解引用强转)。在可变上下文中使用的是 [`DerefMut`],
/// 并会类似地发生可变 deref coercion。
///
/// **警告:** deref coercion 是一项强大的语言特性,它对每一个实现了 `Deref`
/// 的类型都有深远影响。编译器会悄无声息地插入对 `Deref::deref` 的调用。正因
/// 如此,实现 `Deref` 时应当谨慎,只有在确实需要 deref coercion 时才去实现它。
/// 关于何时通常合适、何时通常不合适,见 [下文][implementing]。
///
/// 实现了 `Deref` 或 `DerefMut` 的类型常被称为“智能指针”(smart pointer),
/// deref coercion 这一机制正是为了促成该名称所暗示的“类指针”行为而专门设计的。
/// 通常,“智能指针”类型的目的是改变所含值的所有权语义(例如 [`Rc`][rc] 或
/// [`Cow`][cow]),或者改变所含值的存储语义(例如 [`Box`][box])。
///
/// # Deref coercion(自动解引用强转)
///
/// 如果 `T` 实现了 `Deref<Target = U>`,而 `v` 是一个 `T` 类型的值,那么:
///
/// * 在不可变上下文中,`*v`(此处 `T` 既不是引用也不是裸指针)等价于
///   `*Deref::deref(&v)`。
/// * `&T` 类型的值会被强转为 `&U` 类型的值。
/// * `T` 隐式拥有类型 `U` 的所有以 `&self` 为接收者的方法。
///
/// 更多细节请参阅 [《Rust 程序设计语言》中的相应章节][book],以及 reference 中
/// 关于 [解引用运算符][ref-deref-op]、[方法解析][method resolution] 和
/// [类型强转][type coercions] 的章节。
///
/// # 何时实现 `Deref` 或 `DerefMut`
///
/// 同样的建议适用于这两个 deref trait。一般而言,在以下情况下 **应当** 实现
/// deref trait:
///
/// 1. 该类型的值在行为上透明地表现得像目标类型的值;
/// 1. deref 函数的实现是廉价的;并且
/// 1. 该类型的使用者不会被任何 deref coercion 行为所惊讶。
///
/// 一般而言,在以下情况下 **不应当** 实现 deref trait:
///
/// 1. deref 实现可能出乎意料地失败;或者
/// 1. 该类型拥有很可能与目标类型上的方法发生冲突的方法;或者
/// 1. 把 deref coercion 作为公共 API 的一部分长期承诺并不可取。
///
/// 注意,在众多目标类型上泛型地实现 deref trait,与仅针对特定目标类型实现它,
/// 二者之间存在巨大差异。
///
/// 泛型实现,比如 [`Box<T>`][box](它对每个类型都是泛型的,并解引用到 `T`),
/// 应当谨慎,只提供很少或不提供方法,因为目标类型未知,因此每个方法都可能与
/// 目标类型上的某个方法冲突,从而给使用者造成困惑。`impl<T> Box<T>` 没有任何
/// 方法(尽管有若干关联函数),部分原因正在于此。
///
/// 特定实现,比如 [`String`][string](其 `Deref` 实现的 `Target = str`),则可以
/// 拥有许多方法,因为避免冲突要容易得多。`String` 与 `str` 都有许多方法,而由于
/// deref coercion,`String` 还额外表现得仿佛拥有 `str` 的每一个方法。在这种意义
/// 下,实现该 trait 的类型本身可以是泛型的,而实现仍然是“特定”的;例如
/// [`Vec<T>`][vec] 解引用到 `[T]`,因此 `T` 的方法并不适用于它。
///
/// 还要考虑到,deref coercion 意味着 deref trait 比任何其他 trait 都更大程度地
/// 构成了类型公共 API 的一部分,因为它会被编译器隐式调用。因此,明智的做法是
/// 先考虑清楚:你是否愿意把它作为公共 API 来长期支持。
///
/// [`AsRef`] 与 [`Borrow`][core::borrow::Borrow] 这两个 trait 的签名与 `Deref`
/// 非常相似。无论是作为 deref trait 的补充还是替代,实现其中之一或两者都可能
/// 是可取的。详情请参阅它们各自的文档。
///
/// # 可失败性(Fallibility)
///
/// **本 trait 的方法绝不应当出乎意料地失败**。deref coercion 意味着编译器常常
/// 会隐式插入对 `Deref::deref` 的调用。当 `Deref` 被隐式调用时,解引用过程中的
/// 失败会极其令人困惑。在绝大多数用法中它应当是不会失败的(infallible),不过
/// 例如当类型因程序员的错误而被误用时,panic 或许是可以接受的。
///
/// 然而,不可失败性并未被强制要求,因此也得不到保证。正因如此,`unsafe` 代码
/// 一般不应为了健全性(soundness)而依赖其不可失败这一点。
///
/// [book]: ../../book/ch15-02-deref.html
/// [coercion]: #deref-coercion
/// [implementing]: #when-to-implement-deref-or-derefmut
/// [ref-deref-op]: ../../reference/expressions/operator-expr.html#the-dereference-operator
/// [method resolution]: ../../reference/expressions/method-call-expr.html
/// [type coercions]: ../../reference/type-coercions.html
/// [box]: ../../alloc/boxed/struct.Box.html
/// [string]: ../../alloc/string/struct.String.html
/// [vec]: ../../alloc/vec/struct.Vec.html
/// [rc]: ../../alloc/rc/struct.Rc.html
/// [cow]: ../../alloc/borrow/enum.Cow.html
///
/// # 示例
///
/// 一个只有单个字段、可以通过解引用该结构体来访问该字段的结构体。
///
/// ```
/// use std::ops::Deref;
///
/// struct DerefExample<T> {
///     value: T
/// }
///
/// impl<T> Deref for DerefExample<T> {
///     type Target = T;
///
///     fn deref(&self) -> &Self::Target {
///         &self.value
///     }
/// }
///
/// let x = DerefExample { value: 'a' };
/// assert_eq!('a', *x);
/// ```
#[lang = "deref"]
#[doc(alias = "*")]
#[doc(alias = "&*")]
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_diagnostic_item = "Deref"]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
pub const trait Deref: PointeeSized {
    /// 解引用之后得到的结果类型。
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_diagnostic_item = "deref_target"]
    #[lang = "deref_target"]
    type Target: ?Sized;

    /// 对该值进行解引用。
    #[must_use]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_diagnostic_item = "deref_method"]
    fn deref(&self) -> &Self::Target;
}

#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl<T: ?Sized> const Deref for &T {
    type Target = T;

    #[rustc_diagnostic_item = "noop_method_deref"]
    fn deref(&self) -> &T {
        self
    }
}
#[stable(feature = "rust1", since = "1.0.0")]
impl<T: ?Sized> !DerefMut for &T {}

#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl<T: ?Sized> const Deref for &mut T {
    type Target = T;

    fn deref(&self) -> &T {
        self
    }
}

/// 用于可变解引用操作,例如 `*v = 1;` 中那样。
///
/// 除了用于在可变上下文中通过(一元)`*` 运算符进行显式解引用外,`DerefMut`
/// 还会在许多场合被编译器隐式使用。这一机制被称为 ["可变 deref coercion"][coercion]
/// (mutable deref coercion)。在不可变上下文中使用的是 [`Deref`]。
///
/// **警告:** deref coercion 是一项强大的语言特性,它对每一个实现了 `DerefMut`
/// 的类型都有深远影响。编译器会悄无声息地插入对 `DerefMut::deref_mut` 的调用。
/// 正因如此,实现 `DerefMut` 时应当谨慎,只有在确实需要可变 deref coercion 时
/// 才去实现它。关于何时通常合适、何时通常不合适,见 [`Deref` 文档][implementing]。
///
/// 实现了 `DerefMut` 或 `Deref` 的类型常被称为“智能指针”(smart pointer),
/// deref coercion 这一机制正是为了促成该名称所暗示的“类指针”行为而专门设计的。
/// 通常,“智能指针”类型的目的是改变所含值的所有权语义(例如 [`Rc`][rc] 或
/// [`Cow`][cow]),或者改变所含值的存储语义(例如 [`Box`][box])。
///
/// # 可变 deref coercion(Mutable deref coercion)
///
/// 如果 `T` 实现了 `DerefMut<Target = U>`,而 `v` 是一个 `T` 类型的值,那么:
///
/// * 在可变上下文中,`*v`(此处 `T` 既不是引用也不是裸指针)等价于
///   `*DerefMut::deref_mut(&mut v)`。
/// * `&mut T` 类型的值会被强转为 `&mut U` 类型的值。
/// * `T` 隐式拥有类型 `U` 的所有(可变)方法。
///
/// 更多细节请参阅 [《Rust 程序设计语言》中的相应章节][book],以及 reference 中
/// 关于 [解引用运算符][ref-deref-op]、[方法解析][method resolution] 和
/// [类型强转][type coercions] 的章节。
///
/// # 可失败性(Fallibility)
///
/// **本 trait 的方法绝不应当出乎意料地失败**。deref coercion 意味着编译器常常
/// 会隐式插入对 `DerefMut::deref_mut` 的调用。当 `DerefMut` 被隐式调用时,解
/// 引用过程中的失败会极其令人困惑。在绝大多数用法中它应当是不会失败的,不过
/// 例如当类型因程序员的错误而被误用时,panic 或许是可以接受的。
///
/// 然而,不可失败性并未被强制要求,因此也得不到保证。正因如此,`unsafe` 代码
/// 一般不应为了健全性(soundness)而依赖其不可失败这一点。
///
/// [book]: ../../book/ch15-02-deref.html
/// [coercion]: #mutable-deref-coercion
/// [implementing]: Deref#when-to-implement-deref-or-derefmut
/// [ref-deref-op]: ../../reference/expressions/operator-expr.html#the-dereference-operator
/// [method resolution]: ../../reference/expressions/method-call-expr.html
/// [type coercions]: ../../reference/type-coercions.html
/// [box]: ../../alloc/boxed/struct.Box.html
/// [string]: ../../alloc/string/struct.String.html
/// [rc]: ../../alloc/rc/struct.Rc.html
/// [cow]: ../../alloc/borrow/enum.Cow.html
///
/// # 示例
///
/// 一个只有单个字段、可以通过解引用该结构体来修改该字段的结构体。
///
/// ```
/// use std::ops::{Deref, DerefMut};
///
/// struct DerefMutExample<T> {
///     value: T
/// }
///
/// impl<T> Deref for DerefMutExample<T> {
///     type Target = T;
///
///     fn deref(&self) -> &Self::Target {
///         &self.value
///     }
/// }
///
/// impl<T> DerefMut for DerefMutExample<T> {
///     fn deref_mut(&mut self) -> &mut Self::Target {
///         &mut self.value
///     }
/// }
///
/// let mut x = DerefMutExample { value: 'a' };
/// *x = 'b';
/// assert_eq!('b', x.value);
/// ```
#[lang = "deref_mut"]
#[doc(alias = "*")]
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
pub const trait DerefMut: [const] Deref + PointeeSized {
    /// 以可变方式对该值进行解引用。
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_diagnostic_item = "deref_mut_method"]
    fn deref_mut(&mut self) -> &mut Self::Target;
}

#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl<T: ?Sized> const DerefMut for &mut T {
    fn deref_mut(&mut self) -> &mut T {
        self
    }
}

/// 永久不稳定的标记 trait。表明该类型拥有一个“行为良好”的 [`Deref`](以及
/// 在适用时的 [`DerefMut`])实现。deref 模式(deref pattern)的健全性依赖于这一点。
///
/// FIXME(deref_patterns): 精确语义尚未确定;粗略的想法是:在没有中间修改的情况下
/// 连续调用 `deref`/`deref_mut` 应当是幂等的,也就是说,就模式匹配而言它们返回
/// 相同的值。调用 `deref`/`deref_mut` 同样必须保持指针本身不变。
#[unstable(feature = "deref_pure_trait", issue = "87121")]
#[lang = "deref_pure"]
pub unsafe trait DerefPure: PointeeSized {}

#[unstable(feature = "deref_pure_trait", issue = "87121")]
unsafe impl<T: ?Sized> DerefPure for &T {}

#[unstable(feature = "deref_pure_trait", issue = "87121")]
unsafe impl<T: ?Sized> DerefPure for &mut T {}

/// 表明一个结构体可以被用作方法接收者(method receiver)。也就是说,某类型可以
/// 把这个类型用作 `self` 的类型,就像这样:
/// ```compile_fail
/// # // 目前这里是 compile_fail,因为 arbitrary_self_types 在编译器一侧的部分
/// # // 尚未实现
/// use std::ops::Receiver;
///
/// struct SmartPointer<T>(T);
///
/// impl<T> Receiver for SmartPointer<T> {
///    type Target = T;
/// }
///
/// struct MyContainedType;
///
/// impl MyContainedType {
///   fn method(self: SmartPointer<Self>) {
///     // ...
///   }
/// }
///
/// fn main() {
///   let ptr = SmartPointer(MyContainedType);
///   ptr.method();
/// }
/// ```
/// 这个 trait 对任何实现了 [`Deref`] 的类型都有一揽子实现(blanket impl),这其中
/// 包括标准库的指针类型,如 `Box<T>`、`Rc<T>`、`&T` 和 `Pin<P>`。正因如此,很少
/// 需要直接实现它。通常只有当你需要实现一种无法实现 [`Deref`] 的智能指针类型时
/// 才会这么做;也许是因为你在与另一门编程语言交互,无法保证引用遵守 Rust 的别名
/// (aliasing)规则。
///
/// 在查找方法候选项时,Rust 会沿着一条可能的 `Receiver` 链去探索,因此下面这些
/// 方法都能正常工作:
/// ```
/// use std::boxed::Box;
/// use std::rc::Rc;
///
/// // `Box` 和 `Rc` 都(间接地)实现了 Receiver
///
/// struct MyContainedType;
///
/// fn main() {
///   let t = Rc::new(Box::new(MyContainedType));
///   t.method_a();
///   t.method_b();
///   t.method_c();
/// }
///
/// impl MyContainedType {
///   fn method_a(&self) {
///
///   }
///   fn method_b(self: &Box<Self>) {
///
///   }
///   fn method_c(self: &Rc<Box<Self>>) {
///
///   }
/// }
/// ```
#[lang = "receiver"]
#[unstable(feature = "arbitrary_self_types", issue = "44874")]
pub trait Receiver: PointeeSized {
    /// 可以在其上调用方法的目标类型。
    #[rustc_diagnostic_item = "receiver_target"]
    #[lang = "receiver_target"]
    #[unstable(feature = "arbitrary_self_types", issue = "44874")]
    type Target: ?Sized;
}

#[unstable(feature = "arbitrary_self_types", issue = "44874")]
impl<P: ?Sized, T: ?Sized> Receiver for P
where
    P: Deref<Target = T>,
{
    type Target = T;
}

/// 表明一个结构体可以被用作方法接收者,且无需 `arbitrary_self_types` 特性。
/// 它由标准库的指针类型实现,如 `Box<T>`、`Rc<T>`、`&T` 和 `Pin<P>`。
///
/// 这个 trait 不久后将被移除,并由一套基于当前“arbitrary self types”不稳定特性、
/// 更为通用的设施取代。那套新设施将使用上面那个名为 `Receiver` 的替代 trait,
/// 这正是它如今被命名为 `LegacyReceiver`(旧版接收者)的原因。
#[lang = "legacy_receiver"]
#[unstable(feature = "legacy_receiver_trait", issue = "none")]
#[doc(hidden)]
pub trait LegacyReceiver: PointeeSized {
    // 空。
}

#[unstable(feature = "legacy_receiver_trait", issue = "none")]
impl<T: PointeeSized> LegacyReceiver for &T {}

#[unstable(feature = "legacy_receiver_trait", issue = "none")]
impl<T: PointeeSized> LegacyReceiver for &mut T {}
