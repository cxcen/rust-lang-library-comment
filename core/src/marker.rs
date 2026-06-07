//! 表示类型基本属性的原始 trait 与类型。
//!
//! Rust 的类型可以依据其内在性质从多个有用的角度进行分类。这些分类
//! 在语言中以 trait 的形式表达出来。例如:能否跨线程转移所有权
//! (`Send`)、引用能否跨线程共享(`Sync`)、大小是否在编译期已知
//! (`Sized`)、能否按位复制(`Copy`)等。这些 trait 大多是
//! “标记 trait”(marker trait):它们没有任何方法,仅仅作为类型
//! 性质的标记存在,供编译器在类型检查时依据它们做出安全性判断。

#![stable(feature = "rust1", since = "1.0.0")]

mod variance;

#[unstable(feature = "phantom_variance_markers", issue = "135806")]
pub use self::variance::{
    PhantomContravariant, PhantomContravariantLifetime, PhantomCovariant, PhantomCovariantLifetime,
    PhantomInvariant, PhantomInvariantLifetime, Variance, variance,
};
use crate::cell::UnsafeCell;
use crate::clone::TrivialClone;
use crate::cmp;
use crate::fmt::Debug;
use crate::hash::{Hash, Hasher};
use crate::pin::UnsafePinned;

// 注意:为了让 `core` 与 `minicore` 产生一致的错误信息,所有 `diagnostic` 属性
// 在 `minicore` 中(若 `minicore` 也定义了该条目)都应当被一字不差地复制过去。

/// 同时为多个类型实现某个给定的标记 trait。
///
/// 基本语法如下:
/// ```ignore private macro
/// marker_impls! { MarkerTrait for u8, i8 }
/// ```
/// 也可以实现 `unsafe` trait:
/// ```ignore private macro
/// marker_impls! { unsafe MarkerTrait for u8, i8 }
/// ```
/// 为所有 impl 添加属性:
/// ```ignore private macro
/// marker_impls! {
///     #[allow(lint)]
///     #[unstable(feature = "marker_trait", issue = "none")]
///     MarkerTrait for u8, i8
/// }
/// ```
/// 并且可以使用泛型:
/// ```ignore private macro
/// marker_impls! {
///     MarkerTrait for
///         u8, i8,
///         {T: ?Sized} *const T,
///         {T: ?Sized} *mut T,
///         {T: MarkerTrait} PhantomData<T>,
///         u32,
/// }
/// ```
#[unstable(feature = "internal_impls_macro", issue = "none")]
// Allow implementations of `UnsizedConstParamTy` even though std cannot use that feature.
#[allow_internal_unstable(unsized_const_params)]
macro marker_impls {
    ( $(#[$($meta:tt)*])* $Trait:ident for $({$($bounds:tt)*})? $T:ty $(, $($rest:tt)*)? ) => {
        $(#[$($meta)*])* impl< $($($bounds)*)? > $Trait for $T {}
        marker_impls! { $(#[$($meta)*])* $Trait for $($($rest)*)? }
    },
    ( $(#[$($meta:tt)*])* $Trait:ident for ) => {},

    ( $(#[$($meta:tt)*])* unsafe $Trait:ident for $({$($bounds:tt)*})? $T:ty $(, $($rest:tt)*)? ) => {
        $(#[$($meta)*])* unsafe impl< $($($bounds)*)? > $Trait for $T {}
        marker_impls! { $(#[$($meta)*])* unsafe $Trait for $($($rest)*)? }
    },
    ( $(#[$($meta:tt)*])* unsafe $Trait:ident for ) => {},
}

/// 可以安全地跨线程边界转移所有权的类型。
///
/// `Send` 是一个 **unsafe auto trait**(不安全的自动 trait):
/// - **语义**:`T: Send` 表示把一个 `T` 类型的值的所有权从一个线程
///   转移(move)到另一个线程是安全的。绝大多数类型都是 `Send`。
/// - **自动派生**:作为 auto trait,编译器会自动为其所有成员都是
///   `Send` 的复合类型(结构体、枚举、元组等)实现 `Send`,无需手写。
///   只要有任何一个字段不是 `Send`,整个类型就自动不是 `Send`。
/// - **unsafe 的含义**:当某个类型的字段无法让编译器自动推导(例如内部
///   持有裸指针),而作者确知它在跨线程转移时是安全的,可以手写
///   `unsafe impl Send`。此时 `unsafe` 关键字代表实现者向编译器
///   **郑重承诺**该类型确实满足线程安全契约;一旦承诺不实,就会引入
///   数据竞争,属于未定义行为。
///
/// 编译器正是依赖 `Send` 来做线程安全检查:像 `thread::spawn` 这类把
/// 闭包/值移交给新线程的 API,其参数都带有 `T: Send` 约束。只要类型
/// 不是 `Send`,这类调用就无法通过编译,从而在编译期阻止把不可跨线程
/// 的值送进别的线程。
///
/// 一个非 `Send` 类型的例子是引用计数指针 [`rc::Rc`][`Rc`]。如果两个
/// 线程同时克隆指向同一个被引用计数值的 [`Rc`],它们可能会同时更新
/// 引用计数,这是[未定义行为][ub],因为 [`Rc`] 并不使用原子操作。它的
/// 表亲 [`sync::Arc`][arc] 使用了原子操作(因而带来一些额外开销),
/// 所以是 `Send` 的。
///
/// 更多细节参见 [Nomicon](../../nomicon/send-and-sync.html) 以及 [`Sync`] trait。
///
/// [`Rc`]: ../../std/rc/struct.Rc.html
/// [arc]: ../../std/sync/struct.Arc.html
/// [ub]: ../../reference/behavior-considered-undefined.html
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_diagnostic_item = "Send"]
#[diagnostic::on_unimplemented(
    message = "`{Self}` cannot be sent between threads safely",
    label = "`{Self}` cannot be sent between threads safely"
)]
pub unsafe auto trait Send {
    // 空。
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T: PointeeSized> !Send for *const T {}
#[stable(feature = "rust1", since = "1.0.0")]
impl<T: PointeeSized> !Send for *mut T {}

// 大多数实例是自动产生的,但这一条 impl 是必需的,用来把 `T: Sync` 与
// `&T: Send` 关联起来(同时它还消除了原本会存在的、不可靠的默认实例
// `T: Send` -> `&T: Send`)。
#[stable(feature = "rust1", since = "1.0.0")]
unsafe impl<T: Sync + PointeeSized> Send for &T {}{}

/// 在编译期具有已知常量大小的类型。
///
/// `Sized` 是一个语言项(lang item),也是 Rust 类型系统中最基础的约束之一。
///
/// **隐式约束**:所有泛型类型参数都带有一条隐式的 `Sized` 约束,也就是说
/// 写 `fn foo<T>(x: T)` 等价于 `fn foo<T: Sized>(x: T)`。可以使用特殊语法
/// `?Sized` 把这条约束去掉(当它不合适时),写成 `T: ?Sized` 表示
/// “`T` 可能有大小,也可能没有”。
///
/// **为何需要它**:像切片 `[T]`、字符串切片 `str`、trait 对象 `dyn Trait`
/// 这类类型是 *动态大小类型*(DST),它们的大小在编译期无法确定,因此是
/// `!Sized` 的。这类值不能直接放在栈上、不能作为普通函数参数按值传递,
/// 必须通过 *胖指针*(fat pointer,例如 `&[T]`、`Box<dyn Trait>`)来间接
/// 访问——胖指针除了地址之外,还额外携带长度或虚表(vtable)指针等元数据,
/// 以补足运行期才知道的大小信息。
///
/// ```
/// # #![allow(dead_code)]
/// struct Foo<T>(T);
/// struct Bar<T: ?Sized>(T);
///
/// // struct FooUse(Foo<[i32]>); // 错误:[i32] 没有实现 Sized
/// struct BarUse(Bar<[i32]>); // OK
/// ```
///
/// 唯一的例外是 trait 中隐式的 `Self` 类型。trait 不带隐式的 `Sized` 约束,
/// 因为这与 [trait 对象][trait object] 不兼容——按定义,trait 需要能与所有
/// 可能的实现者协作,而它们可以是任意大小。
///
/// 虽然 Rust 允许你给 trait 加上 `Sized` 约束,但之后你将无法用它构造出
/// trait 对象:
///
/// ```
/// # #![allow(unused_variables)]
/// trait Foo { }
/// trait Bar: Sized { }
///
/// struct Impl;
/// impl Foo for Impl { }
/// impl Bar for Impl { }
///
/// let x: &dyn Foo = &Impl;    // OK
/// // let y: &dyn Bar = &Impl; // 错误:trait `Bar` 无法构造成 trait 对象
/// ```
///
/// [trait object]: ../../book/ch17-02-trait-objects.html
#[doc(alias = "?", alias = "?Sized")]
#[stable(feature = "rust1", since = "1.0.0")]
#[lang = "sized"]
#[diagnostic::on_unimplemented(
    message = "the size for values of type `{Self}` cannot be known at compilation time",
    label = "doesn't have a size known at compile-time"
)]
#[fundamental] // 例如 Default 需要它,因为 Default 要求能对 `[T]: !Default` 求值
#[rustc_specialization_trait]
#[rustc_deny_explicit_impl]
#[rustc_do_not_implement_via_object]
// `Sized` 虽然带有 supertrait 却是协归纳(coinductive)的,这没有问题:因为
// 不存在用户手写的 impl,而且仅凭内建 impl 我们就能确定——只要子 trait 成立,
// 其 supertrait 必然也成立。
#[rustc_coinductive]
pub trait Sized: MetaSized {
    // 空。
}

/// 大小可以由指针元数据推算出来的类型。
#[unstable(feature = "sized_hierarchy", issue = "144404")]
#[lang = "meta_sized"]
#[diagnostic::on_unimplemented(
    message = "the size for values of type `{Self}` cannot be known",
    label = "doesn't have a known size"
)]
#[fundamental]
#[rustc_specialization_trait]
#[rustc_deny_explicit_impl]
#[rustc_do_not_implement_via_object]
// `MetaSized` 同样因上面针对 `Sized` 所述的原因而可以是协归纳的。
#[rustc_coinductive]
pub trait MetaSized: PointeeSized {
    // 空
}

/// 可能有大小、也可能没有大小的类型。
#[unstable(feature = "sized_hierarchy", issue = "144404")]
#[lang = "pointee_sized"]
#[diagnostic::on_unimplemented(
    message = "values of type `{Self}` may or may not have a size",
    label = "may or may not have a known size"
)]
#[fundamental]
#[rustc_specialization_trait]
#[rustc_deny_explicit_impl]
#[rustc_do_not_implement_via_object]
#[rustc_coinductive]
pub trait PointeeSized {
    // 空
}

/// 可以被“去尺寸化”(unsize)成动态大小类型的类型。
///
/// 例如,定长数组类型 `[i8; 2]` 实现了 `Unsize<[i8]>` 和
/// `Unsize<dyn fmt::Debug>`。
///
/// `Unsize` 的所有实现都由编译器自动提供。这些实现包括:
///
/// - 数组 `[T; N]` 实现 `Unsize<[T]>`。
/// - 当满足以下所有条件时,某类型实现 `Unsize<dyn Trait + 'a>`:
///   - 该类型实现了 `Trait`。
///   - `Trait` 是 dyn 兼容的[^1]。
///   - 该类型是 sized 的。
///   - 该类型的存活时间长于 `'a`。
/// - 当满足以下所有条件时,trait 对象 `dyn TraitA + AutoA... + 'a` 实现
///   `Unsize<dyn TraitB + AutoB... + 'b>`:
///   - `TraitB` 是 `TraitA` 的 supertrait。
///   - `AutoB...` 是 `AutoA...` 的子集。
///   - `'a` 的存活时间长于 `'b`。
/// - 结构体 `Foo<..., T1, ..., Tn, ...>` 实现
///   `Unsize<Foo<..., U1, ..., Un, ...>>`,其中可以改变任意数量的(类型与
///   const)参数,只要满足以下所有条件:
///   - 只有 `Foo` 的最后一个字段的类型涉及参数 `T1`, ..., `Tn`。
///   - 该结构体的其余所有参数都相等。
///   - `Field<T1, ..., Tn>: Unsize<Field<U1, ..., Un>>`,其中 `Field<...>`
///     代表该结构体最后一个字段的实际类型。
///
/// `Unsize` 与 [`ops::CoerceUnsized`] 配合使用,使得像 [`Rc`] 这类
/// “用户自定义”的容器能够容纳动态大小类型。更多细节参见
/// [DST 强转 RFC][RFC982] 以及 [Nomicon 中关于强转的条目][nomicon-coerce]。
///
/// [`ops::CoerceUnsized`]: crate::ops::CoerceUnsized
/// [`Rc`]: ../../std/rc/struct.Rc.html
/// [RFC982]: https://github.com/rust-lang/rfcs/blob/master/text/0982-dst-coercion.md
/// [nomicon-coerce]: ../../nomicon/coercions.html
/// [^1]: 旧称 *对象安全*(object safe)。
#[unstable(feature = "unsize", issue = "18598")]
#[lang = "unsize"]
#[rustc_deny_explicit_impl]
#[rustc_do_not_implement_via_object]
pub trait Unsize<T: PointeeSized>: PointeeSized {
    // 空。
}

/// 用作模式匹配中常量的必备 trait。
///
/// 常量只有在以下条件成立时才允许用作模式:(a) 它的类型实现了
/// `PartialEq`,并且 (b) 把该常量的值当作模式来解释,等价于调用
/// `PartialEq`。这样可以确保用作模式的常量不会以意料之外的方式暴露
/// 实现细节,也不会造成 semver(语义化版本)上的隐患。
///
/// 本 trait 用来确保第 (b) 点成立。任何派生了 `PartialEq` 的类型都会
/// 自动实现本 trait。
///
/// 手动实现本 trait(它是 unstable 的)是类型作者显式允许对该类型的
/// const 值进行比较的一种方式;该比较操作会递归地比较所有字段
/// (包括私有字段),即便这一行为与 `PartialEq` 有所不同。这也意味着
/// 给类型新增私有字段可能会造成破坏 semver 的后果。
#[unstable(feature = "structural_match", issue = "31434")]
#[diagnostic::on_unimplemented(message = "the type `{Self}` does not `#[derive(PartialEq)]`")]
#[lang = "structural_peq"]
pub trait StructuralPartialEq {
    // 空。
}

marker_impls! {
    #[unstable(feature = "structural_match", issue = "31434")]
    StructuralPartialEq for
        usize, u8, u16, u32, u64, u128,
        isize, i8, i16, i32, i64, i128,
        bool,
        char,
        str /* 严格来说需要 `[u8]: StructuralPartialEq` */,
        (),
        {T, const N: usize} [T; N],
        {T} [T],
        {T: PointeeSized} &T,
}

/// 仅靠复制比特位就能复制其值的类型。
///
/// `Copy` 与 `Clone` 是 Rust 中两种不同层次的“复制”概念,理解二者的区别
/// 至关重要:
/// - **`Copy`**:按位复制(memcpy),复制行为隐式发生且不可重载,而且要求
///   类型 **不含 `Drop`**。一个 `Copy` 类型在被 move 之后,原值依然有效、
///   可以继续使用——因为复制并不会“消耗”原值。
/// - **`Clone`**:可能进行深拷贝、必须 **显式调用** `x.clone()`;允许执行
///   复制资源所需的任意逻辑。
///
/// `Copy` 以 `Clone` 为 supertrait(即 `Copy: Clone`),所以凡是 `Copy`
/// 的类型也必然实现 `Clone`;反之则不然。
///
/// 默认情况下,变量绑定具有“移动语义”(move semantics)。换句话说:
///
/// ```
/// #[derive(Debug)]
/// struct Foo;
///
/// let x = Foo;
///
/// let y = x;
///
/// // `x` 已经被 move 进 `y`,因此不能再被使用
///
/// // println!("{x:?}"); // 错误:使用了已被 move 的值
/// ```
///
/// 然而,如果一个类型实现了 `Copy`,它转而具有“复制语义”(copy semantics):
///
/// ```
/// // 我们可以派生 `Copy` 的实现。同时还要求 `Clone`,因为它是 `Copy`
/// // 的 supertrait。
/// #[derive(Debug, Copy, Clone)]
/// struct Foo;
///
/// let x = Foo;
///
/// let y = x;
///
/// // `y` 是 `x` 的一份拷贝
///
/// println!("{x:?}"); // 完全没问题!
/// ```
///
/// 重要的是要注意:在上面两个例子中,唯一的区别在于赋值之后你是否还能
/// 访问 `x`。在底层,无论是 copy 还是 move 都可能造成内存中比特位的复制,
/// 尽管这种复制有时会被优化掉。
///
/// ## 如何为类型实现 `Copy`?
///
/// 为类型实现 `Copy` 有两种方式。最简单的是使用 `derive`:
///
/// ```
/// #[derive(Copy, Clone)]
/// struct MyStruct;
/// ```
///
/// 你也可以手动实现 `Copy` 和 `Clone`:
///
/// ```
/// struct MyStruct;
///
/// impl Copy for MyStruct { }
///
/// impl Clone for MyStruct {
///     fn clone(&self) -> MyStruct {
///         *self
///     }
/// }
/// ```
///
/// 两者之间有一个细微差别。`derive` 策略还会给类型参数加上 `Copy` 约束:
///
/// ```
/// #[derive(Clone)]
/// struct MyStruct<T>(T);
///
/// impl<T: Copy> Copy for MyStruct<T> { }
/// ```
///
/// 这并不总是我们想要的。例如,共享引用(`&T`)无论 `T` 是否 `Copy` 都可以
/// 被复制。同理,一个含有诸如 [`PhantomData`] 之类标记的泛型结构体,也可能
/// 可以通过按位复制来复制。
///
/// ## `Copy` 与 `Clone` 有什么区别?
///
/// 复制(copy)是隐式发生的,例如作为赋值 `y = x` 的一部分。`Copy` 的行为
/// 不可重载;它永远是一次简单的按位复制。
///
/// 克隆(clone)是显式动作,即 `x.clone()`。[`Clone`] 的实现可以提供安全
/// 复制值所需的任意类型相关行为。例如,[`String`] 的 [`Clone`] 实现需要
/// 复制堆上被指向的字符串缓冲区。对 [`String`] 值做一次简单的按位复制只会
/// 复制指针,从而在后续导致重复释放(double free)。正因如此,[`String`]
/// 是 [`Clone`] 但不是 `Copy`。
///
/// [`Clone`] 是 `Copy` 的 supertrait,所以凡是 `Copy` 的类型都必须同时实现
/// [`Clone`]。如果一个类型是 `Copy`,那么它的 [`Clone`] 实现只需返回 `*self`
/// 即可(见上面的例子)。
///
/// ## 我的类型什么时候可以是 `Copy`?
///
/// 当一个类型的所有成员都实现了 `Copy` 时,它就可以实现 `Copy`。例如,
/// 下面这个结构体可以是 `Copy`:
///
/// ```
/// # #[allow(dead_code)]
/// #[derive(Copy, Clone)]
/// struct Point {
///    x: i32,
///    y: i32,
/// }
/// ```
///
/// 一个结构体可以是 `Copy`,而 [`i32`] 是 `Copy`,因此 `Point` 有资格成为
/// `Copy`。相比之下,考虑下面这个:
///
/// ```
/// # #![allow(dead_code)]
/// # struct Point;
/// struct PointList {
///     points: Vec<Point>,
/// }
/// ```
///
/// 结构体 `PointList` 无法实现 `Copy`,因为 [`Vec<T>`] 不是 `Copy`。如果我们
/// 试图派生 `Copy` 实现,会得到一个错误:
///
/// ```text
/// the trait `Copy` cannot be implemented for this type; field `points` does not implement `Copy`
/// ```
///
/// 共享引用(`&T`)也是 `Copy`,所以即使一个类型持有的是 *非* `Copy` 类型
/// `T` 的共享引用,该类型本身仍可以是 `Copy`。考虑下面这个结构体,它可以
/// 实现 `Copy`,因为它只持有指向上面那个非 `Copy` 类型 `PointList` 的
/// *共享引用*:
///
/// ```
/// # #![allow(dead_code)]
/// # struct PointList;
/// #[derive(Copy, Clone)]
/// struct PointListWrapper<'a> {
///     point_list_ref: &'a PointList,
/// }
/// ```
///
/// ## 我的类型什么时候 *不能* 是 `Copy`?
///
/// 有些类型无法被安全地复制。例如,复制 `&mut T` 会制造出别名(aliased)
/// 的可变引用。复制 [`String`] 则会让多份值同时负责管理 [`String`] 的
/// 缓冲区,从而导致重复释放。
///
/// 把后一种情形推而广之:任何实现了 [`Drop`] 的类型都不能是 `Copy`,因为
/// 它管理着除自身 [`size_of::<T>`] 字节之外的某种资源。这正是 `Copy` 要求
/// 类型不含 `Drop` 的根本原因——按位复制无法复制 `Drop` 所管理的资源,而
/// move 后原值仍然有效又意味着该资源会被释放两次。
///
/// 如果你试图在一个含有非 `Copy` 数据的结构体或枚举上实现 `Copy`,会得到
/// 错误 [E0204]。
///
/// [E0204]: ../../error_codes/E0204.html
///
/// ## 我的类型什么时候 *应该* 是 `Copy`?
///
/// 一般来说,如果你的类型 _能够_ 实现 `Copy`,那么它就应该实现。不过要记住,
/// 实现 `Copy` 属于类型公开 API 的一部分。如果该类型未来有可能变成
/// 非 `Copy`,那么现在就略去 `Copy` 实现也许是明智之举,以避免造成破坏性的
/// API 变更。
///
/// ## 额外的实现者
///
/// 除了[下方列出的实现者][impls],以下类型也实现了 `Copy`:
///
/// * 函数项类型(即为每个函数定义的那个独一无二的类型)
/// * 函数指针类型(例如 `fn() -> i32`)
/// * 闭包类型,前提是它们不从环境中捕获任何值,或者所有被捕获的值自身都实现
///   了 `Copy`。注意,以共享引用方式捕获的变量总是实现 `Copy`(即便被引用者
///   本身并不是 `Copy`),而以可变引用方式捕获的变量则永远不是 `Copy`。
///
/// [`Vec<T>`]: ../../std/vec/struct.Vec.html
/// [`String`]: ../../std/string/struct.String.html
/// [`size_of::<T>`]: size_of
/// [impls]: #implementors
#[stable(feature = "rust1", since = "1.0.0")]
#[lang = "copy"]
// 这是不可靠的,但 `hashbrown` 需要它。
// FIXME(joboet):改用 `TrivialClone` 来让 `hashbrown` 不再依赖此处。
#[rustc_unsafe_specialization_marker]
#[rustc_diagnostic_item = "Copy"]
pub trait Copy: Clone {
    // 空。
}

/// 生成 `Copy` trait 实现的派生宏。
#[rustc_builtin_macro]
#[stable(feature = "builtin_macro_prelude", since = "1.38.0")]
#[allow_internal_unstable(core_intrinsics, derive_clone_copy_internals)]
pub macro Copy($item:item) {
    /* 编译器内建 */
}

// 为原始类型实现 `Copy`。
//
// 那些无法用 Rust 描述的实现,定义在 `rustc_trait_selection` 中的
// `traits::SelectionContext::copy_clone_conditions()` 里。
marker_impls! {
    #[stable(feature = "rust1", since = "1.0.0")]
    Copy for
        usize, u8, u16, u32, u64, u128,
        isize, i8, i16, i32, i64, i128,
        f16, f32, f64, f128,
        bool, char,
        {T: PointeeSized} *const T,
        {T: PointeeSized} *mut T,

}

#[unstable(feature = "never_type", issue = "35121")]
impl Copy for ! {}

/// 共享引用可以被复制,但可变引用 *不可以*!
#[stable(feature = "rust1", since = "1.0.0")]
impl<T: PointeeSized> Copy for &T {}

/// 用于标记那些允许出现在 union 字段以及 unsafe binder 类型中的类型的标记 trait。
///
/// 已为以下类型实现:
/// * 对所有 `T`:`&T`、`&mut T`,
/// * 对所有 `T`:`ManuallyDrop<T>`,
/// * 元素都实现了 `BikeshedGuaranteedNoDrop` 的元组与数组,
/// * 或者,凡是 `Copy` 的所有类型。
///
/// 值得注意的是,出于 semver 方面的原因,它并不包含所有可平凡析构的类型。
///
/// 目前名字尚未敲定(bikeshed)。本 trait 除了反映 union 中为保证字段有效性
/// 所允许的类型集合之外,不做任何其他事情。
#[unstable(feature = "bikeshed_guaranteed_no_drop", issue = "none")]
#[lang = "bikeshed_guaranteed_no_drop"]
#[rustc_deny_explicit_impl]
#[rustc_do_not_implement_via_object]
#[doc(hidden)]
pub trait BikeshedGuaranteedNoDrop {}

/// 可以安全地在线程之间共享引用的类型。
///
/// `Sync` 同样是一个 **unsafe auto trait**,与 `Send` 互为表里。
///
/// 它的精确定义是:类型 `T` 是 [`Sync`] 当且仅当 `&T` 是 [`Send`]。
/// 换言之,当把 `&T` 引用在线程之间传递不会有任何[未定义行为][ub]
/// (包括数据竞争)之虞时,`T` 就是 `Sync`。
///
/// 与 `Send` 一样,`Sync` 由编译器 **自动派生**:只要复合类型的所有字段
/// 都是 `Sync`,该类型就自动是 `Sync`;只要有一个字段不是 `Sync`,整个
/// 类型就不是。手写 `unsafe impl Sync` 时,`unsafe` 代表实现者承诺:通过
/// 共享引用 `&T` 并发访问该类型是安全的、不会引发数据竞争;违背此承诺即为
/// 未定义行为。编译器据此进行线程安全检查——例如要在多个线程间共享某个值的
/// 引用,该值的类型就必须是 `Sync`。
///
/// 不出所料,像 [`u8`] 和 [`f64`] 这样的原始类型都是 [`Sync`],由它们构成的
/// 简单聚合类型(元组、结构体和枚举)也是。更多基本的 [`Sync`] 类型还包括
/// 像 `&T` 这样的“不可变”类型,以及那些具有简单的继承式可变性的类型,例如
/// [`Box<T>`][box]、[`Vec<T>`][vec] 以及大多数其他集合类型。(泛型参数需要是
/// [`Sync`],其容器才能是 [`Sync`]。)
///
/// 这个定义有一个略显意外的推论:`&mut T` 是 `Sync`(只要 `T` 是 `Sync`),
/// 尽管看上去它似乎能提供未经同步的可变访问。诀窍在于,处于共享引用之后的
/// 可变引用(也就是 `& &mut T`)会变成只读,如同 `& &T` 一般。因此不存在
/// 数据竞争的风险。
///
/// 一份关于 [`Sync`] 和 [`Send`] 如何与引用相关联的简短概览:
/// * `&T` 是 [`Send`] 当且仅当 `T` 是 [`Sync`]
/// * `&mut T` 是 [`Send`] 当且仅当 `T` 是 [`Send`]
/// * `&T` 和 `&mut T` 是 [`Sync`] 当且仅当 `T` 是 [`Sync`]
///
/// 不是 `Sync` 的类型,是那些以非线程安全形式具有“内部可变性”的类型,例如
/// [`Cell`][cell] 和 [`RefCell`][refcell]。这些类型允许即便通过一个不可变的
/// 共享引用也能修改其内容。例如 [`Cell<T>`][cell] 上的 `set` 方法接收
/// `&self`,因此它只需要一个共享引用 [`&Cell<T>`][cell]。该方法不执行任何
/// 同步,因此 [`Cell`][cell] 不可能是 `Sync`。
///
/// 另一个非 `Sync` 类型的例子是引用计数指针 [`Rc`][rc]。给定任意一个引用
/// [`&Rc<T>`][rc],你都可以克隆出一个新的 [`Rc<T>`][rc],从而以非原子的方式
/// 修改引用计数。
///
/// 在确实需要线程安全的内部可变性的场合,Rust 提供了[原子数据类型][atomic data types],
/// 以及通过 [`sync::Mutex`][mutex] 和 [`sync::RwLock`][rwlock] 进行的显式加锁。
/// 这些类型确保任何修改都不会引发数据竞争,因此它们是 `Sync`。同样地,
/// [`sync::Arc`][arc] 提供了 [`Rc`][rc] 的线程安全对应物。
///
/// 任何具有内部可变性的类型,都必须在那些可以通过共享引用被修改的值外面
/// 套上 [`cell::UnsafeCell`][unsafecell] 包装。不这样做即是[未定义行为][ub]。
/// 例如,从 `&T` [`transmute`][transmute] 成 `&mut T` 是无效的。
///
/// 关于 `Sync` 的更多细节,参见 [Nomicon][nomicon-send-and-sync]。
///
/// [box]: ../../std/boxed/struct.Box.html
/// [vec]: ../../std/vec/struct.Vec.html
/// [cell]: crate::cell::Cell
/// [refcell]: crate::cell::RefCell
/// [rc]: ../../std/rc/struct.Rc.html
/// [arc]: ../../std/sync/struct.Arc.html
/// [atomic data types]: crate::sync::atomic
/// [mutex]: ../../std/sync/struct.Mutex.html
/// [rwlock]: ../../std/sync/struct.RwLock.html
/// [unsafecell]: crate::cell::UnsafeCell
/// [ub]: ../../reference/behavior-considered-undefined.html
/// [transmute]: crate::mem::transmute
/// [nomicon-send-and-sync]: ../../nomicon/send-and-sync.html
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_diagnostic_item = "Sync"]
#[lang = "sync"]
#[rustc_on_unimplemented(
    on(
        Self = "core::cell::once::OnceCell<T>",
        note = "if you want to do aliasing and mutation between multiple threads, use `std::sync::OnceLock` instead"
    ),
    on(
        Self = "core::cell::Cell<u8>",
        note = "if you want to do aliasing and mutation between multiple threads, use `std::sync::RwLock` or `std::sync::atomic::AtomicU8` instead",
    ),
    on(
        Self = "core::cell::Cell<u16>",
        note = "if you want to do aliasing and mutation between multiple threads, use `std::sync::RwLock` or `std::sync::atomic::AtomicU16` instead",
    ),
    on(
        Self = "core::cell::Cell<u32>",
        note = "if you want to do aliasing and mutation between multiple threads, use `std::sync::RwLock` or `std::sync::atomic::AtomicU32` instead",
    ),
    on(
        Self = "core::cell::Cell<u64>",
        note = "if you want to do aliasing and mutation between multiple threads, use `std::sync::RwLock` or `std::sync::atomic::AtomicU64` instead",
    ),
    on(
        Self = "core::cell::Cell<usize>",
        note = "if you want to do aliasing and mutation between multiple threads, use `std::sync::RwLock` or `std::sync::atomic::AtomicUsize` instead",
    ),
    on(
        Self = "core::cell::Cell<i8>",
        note = "if you want to do aliasing and mutation between multiple threads, use `std::sync::RwLock` or `std::sync::atomic::AtomicI8` instead",
    ),
    on(
        Self = "core::cell::Cell<i16>",
        note = "if you want to do aliasing and mutation between multiple threads, use `std::sync::RwLock` or `std::sync::atomic::AtomicI16` instead",
    ),
    on(
        Self = "core::cell::Cell<i32>",
        note = "if you want to do aliasing and mutation between multiple threads, use `std::sync::RwLock` or `std::sync::atomic::AtomicI32` instead",
    ),
    on(
        Self = "core::cell::Cell<i64>",
        note = "if you want to do aliasing and mutation between multiple threads, use `std::sync::RwLock` or `std::sync::atomic::AtomicI64` instead",
    ),
    on(
        Self = "core::cell::Cell<isize>",
        note = "if you want to do aliasing and mutation between multiple threads, use `std::sync::RwLock` or `std::sync::atomic::AtomicIsize` instead",
    ),
    on(
        Self = "core::cell::Cell<bool>",
        note = "if you want to do aliasing and mutation between multiple threads, use `std::sync::RwLock` or `std::sync::atomic::AtomicBool` instead",
    ),
    on(
        all(
            Self = "core::cell::Cell<T>",
            not(Self = "core::cell::Cell<u8>"),
            not(Self = "core::cell::Cell<u16>"),
            not(Self = "core::cell::Cell<u32>"),
            not(Self = "core::cell::Cell<u64>"),
            not(Self = "core::cell::Cell<usize>"),
            not(Self = "core::cell::Cell<i8>"),
            not(Self = "core::cell::Cell<i16>"),
            not(Self = "core::cell::Cell<i32>"),
            not(Self = "core::cell::Cell<i64>"),
            not(Self = "core::cell::Cell<isize>"),
            not(Self = "core::cell::Cell<bool>")
        ),
        note = "if you want to do aliasing and mutation between multiple threads, use `std::sync::RwLock`",
    ),
    on(
        Self = "core::cell::RefCell<T>",
        note = "if you want to do aliasing and mutation between multiple threads, use `std::sync::RwLock` instead",
    ),
    message = "`{Self}` cannot be shared between threads safely",
    label = "`{Self}` cannot be shared between threads safely"
)]
pub unsafe auto trait Sync {
    // FIXME(estebank):一旦在 `rustc_on_unimplemented` 中添加 note 的支持
    // 进入 beta,并且被扩展为能够检查需求链中是否存在闭包,就按如下方式
    // 扩展它(#48534):
    // ```
    // on(
    //     closure,
    //     note="`{Self}` cannot be shared safely, consider marking the closure `move`"
    // ),
    // ```

    // 空
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T: PointeeSized> !Sync for *const T {}
#[stable(feature = "rust1", since = "1.0.0")]
impl<T: PointeeSized> !Sync for *mut T {}

/// 零大小类型(ZST),用来标记那些“表现得像”拥有一个 `T` 的东西。
///
/// 给你的类型添加一个 `PhantomData<T>` 字段,等于告诉编译器:你的类型表现得
/// 仿佛它存储着一个 `T` 类型的值,尽管实际上并没有。编译器在计算某些安全性
/// 性质时会用到这一信息。`PhantomData<T>` 本身是零大小的,既不占空间也不存
/// 任何数据,它的全部作用都在于影响编译器对类型的判断,具体包括三方面:
///
/// 1. **变型(variance)**:`PhantomData<T>` 让外层类型相对于 `T` 具有协变性,
///    `PhantomData<&'a T>` 引入生命周期 `'a`,`PhantomData<fn(T)>` 则带来逆变,
///    `PhantomData<*mut T>` / `PhantomData<Cell<T>>` 带来不变。这对裸指针包装
///    类型尤为重要——裸指针自身不携带变型/生命周期信息,需借助它来表达。
/// 2. **drop check**:见下文“所有权与 drop check”一节。
/// 3. **auto trait**(如 `Send`/`Sync`):外层类型会像真的持有 `T` 一样,
///    依据 `T` 是否满足这些 auto trait 来决定自身是否满足。
///
/// 关于如何使用 `PhantomData<T>` 的更深入解释,请参见
/// [Nomicon](../../nomicon/phantom-data.html)。
///
/// # 一条阴森的提示 👻👻👻
///
/// 尽管名字都挺吓人,`PhantomData` 与“幻影类型”(phantom type)是相关的,
/// 但并不相同。幻影类型参数,指的不过是一个从未被使用的类型参数。在 Rust 中,
/// 这通常会招致编译器抱怨,而解决办法就是借助 `PhantomData` 加上一处“占位”
/// 用途。
///
/// # 示例
///
/// ## 未被使用的生命周期参数
///
/// `PhantomData` 也许最常见的用例,就是某个结构体有一个未被使用的生命周期
/// 参数,通常出现在某些 unsafe 代码中。例如,下面这个结构体 `Slice` 含有两个
/// 类型为 `*const T` 的指针,大概是指向某处数组之中:
///
/// ```compile_fail,E0392
/// struct Slice<'a, T> {
///     start: *const T,
///     end: *const T,
/// }
/// ```
///
/// 这里的本意是:底层数据只在生命周期 `'a` 内有效,因此 `Slice` 不应活得比
/// `'a` 更久。然而,这一意图并没有在代码中表达出来,因为根本没有用到生命周期
/// `'a`,于是也就看不出它究竟作用于哪些数据。我们可以通过告诉编译器把 `Slice`
/// 结构体当作 *仿佛* 含有一个引用 `&'a T` 来纠正这一点:
///
/// ```
/// use std::marker::PhantomData;
///
/// # #[allow(dead_code)]
/// struct Slice<'a, T> {
///     start: *const T,
///     end: *const T,
///     phantom: PhantomData<&'a T>,
/// }
/// ```
///
/// 这反过来还会推断出生命周期约束 `T: 'a`,表明 `T` 中的任何引用在生命周期
/// `'a` 内都是有效的。
///
/// 初始化 `Slice` 时,你只需为字段 `phantom` 提供值 `PhantomData`:
///
/// ```
/// # #![allow(dead_code)]
/// # use std::marker::PhantomData;
/// # struct Slice<'a, T> {
/// #     start: *const T,
/// #     end: *const T,
/// #     phantom: PhantomData<&'a T>,
/// # }
/// fn borrow_vec<T>(vec: &Vec<T>) -> Slice<'_, T> {
///     let ptr = vec.as_ptr();
///     Slice {
///         start: ptr,
///         end: unsafe { ptr.add(vec.len()) },
///         phantom: PhantomData,
///     }
/// }
/// ```
///
/// ## 未被使用的类型参数
///
/// 有时你会遇到这样的情形:你有一些未被使用的类型参数,用来表明某个结构体
/// 与何种数据“绑定”,即便那份数据本身并不存在于结构体之中。下面是一个在
/// [FFI] 中出现这种情况的例子。这个外部接口使用类型为 `*mut ()` 的句柄来指代
/// 不同类型的 Rust 值。我们借助结构体 `ExternalResource`(它包装了一个句柄)
/// 上的一个幻影类型参数来追踪 Rust 类型。
///
/// [FFI]: ../../book/ch19-01-unsafe-rust.html#using-extern-functions-to-call-external-code
///
/// ```
/// # #![allow(dead_code)]
/// # trait ResType { }
/// # struct ParamType;
/// # mod foreign_lib {
/// #     pub fn new(_: usize) -> *mut () { 42 as *mut () }
/// #     pub fn do_stuff(_: *mut (), _: usize) {}
/// # }
/// # fn convert_params(_: ParamType) -> usize { 42 }
/// use std::marker::PhantomData;
///
/// struct ExternalResource<R> {
///    resource_handle: *mut (),
///    resource_type: PhantomData<R>,
/// }
///
/// impl<R: ResType> ExternalResource<R> {
///     fn new() -> Self {
///         let size_of_res = size_of::<R>();
///         Self {
///             resource_handle: foreign_lib::new(size_of_res),
///             resource_type: PhantomData,
///         }
///     }
///
///     fn do_stuff(&self, param: ParamType) {
///         let foreign_params = convert_params(param);
///         foreign_lib::do_stuff(self.resource_handle, foreign_params);
///     }
/// }
/// ```
///
/// ## 所有权与 drop check
///
/// `PhantomData` 与 drop check 之间的确切交互方式 **未来可能会改变**。
///
/// 当前,添加一个类型为 `PhantomData<T>` 的字段,表明你的类型在极少数情形下
/// *拥有* 类型为 `T` 的数据。这进而会影响 Rust 编译器的 [drop check] 分析。
/// 确切规则请参见 [drop check] 文档。
///
/// ## 内存布局
///
/// 对所有 `T`,以下保证均成立:
/// * `size_of::<PhantomData<T>>() == 0`
/// * `align_of::<PhantomData<T>>() == 1`
///
/// [drop check]: Drop#drop-check
#[lang = "phantom_data"]
#[stable(feature = "rust1", since = "1.0.0")]
pub struct PhantomData<T: PointeeSized>;

#[stable(feature = "rust1", since = "1.0.0")]
impl<T: PointeeSized> Hash for PhantomData<T> {
    #[inline]
    fn hash<H: Hasher>(&self, _: &mut H) {}
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T: PointeeSized> cmp::PartialEq for PhantomData<T> {
    fn eq(&self, _other: &PhantomData<T>) -> bool {
        true
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T: PointeeSized> cmp::Eq for PhantomData<T> {}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T: PointeeSized> cmp::PartialOrd for PhantomData<T> {
    fn partial_cmp(&self, _other: &PhantomData<T>) -> Option<cmp::Ordering> {
        Option::Some(cmp::Ordering::Equal)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T: PointeeSized> cmp::Ord for PhantomData<T> {
    fn cmp(&self, _other: &PhantomData<T>) -> cmp::Ordering {
        cmp::Ordering::Equal
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T: PointeeSized> Copy for PhantomData<T> {}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T: PointeeSized> Clone for PhantomData<T> {
    fn clone(&self) -> Self {
        Self
    }
}

#[doc(hidden)]
#[unstable(feature = "trivial_clone", issue = "none")]
unsafe impl<T: PointeeSized> TrivialClone for PhantomData<T> {}

#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_unstable(feature = "const_default", issue = "143894")]
impl<T: PointeeSized> const Default for PhantomData<T> {
    fn default() -> Self {
        Self
    }
}

#[unstable(feature = "structural_match", issue = "31434")]
impl<T: PointeeSized> StructuralPartialEq for PhantomData<T> {}

/// 编译器内部使用的 trait,用来表示枚举判别值(discriminant)的类型。
///
/// 本 trait 会自动为每一种类型实现,并且不为 [`mem::Discriminant`] 添加任何
/// 额外保证。在 `DiscriminantKind::Discriminant` 与 `mem::Discriminant` 之间
/// 进行 transmute 是 **未定义行为**。
///
/// [`mem::Discriminant`]: crate::mem::Discriminant
#[unstable(
    feature = "discriminant_kind",
    issue = "none",
    reason = "this trait is unlikely to ever be stabilized, use `mem::discriminant` instead"
)]
#[lang = "discriminant_kind"]
#[rustc_deny_explicit_impl]
#[rustc_do_not_implement_via_object]
pub trait DiscriminantKind {
    /// 判别值的类型,它必须满足 `mem::Discriminant` 所要求的 trait 约束。
    #[lang = "discriminant_type"]
    type Discriminant: Clone + Copy + Debug + Eq + PartialEq + Hash + Send + Sync + Unpin;
}

/// 用来判断一个类型内部是否含有任何 `UnsafeCell`(但不包括经由间接引用
/// 持有的情形)。这会影响诸如:该类型的 `static` 是被放进只读静态内存,
/// 还是可写静态内存。它可以用来声明某个泛型类型的常量不会含有内部可变性,
/// 进而允许把该常量放在引用之后。
///
/// # 安全性(Safety)
///
/// 本 trait 是语言的核心组成部分,只是为了方便才在 libcore 中以 trait 的形式
/// 表达。请 *不要* 为其他类型实现它。
// FIXME:最终本 trait 应当变为 `#[rustc_deny_explicit_impl]`。
// 这需要先把下面这些 impl 移植为原生的内部 impl。
#[lang = "freeze"]
#[unstable(feature = "freeze", issue = "121675")]
pub unsafe auto trait Freeze {}

#[unstable(feature = "freeze", issue = "121675")]
impl<T: PointeeSized> !Freeze for UnsafeCell<T> {}
marker_impls! {
    #[unstable(feature = "freeze", issue = "121675")]
    unsafe Freeze for
        {T: PointeeSized} PhantomData<T>,
        {T: PointeeSized} *const T,
        {T: PointeeSized} *mut T,
        {T: PointeeSized} &T,
        {T: PointeeSized} &mut T,
}

/// 用来判断一个类型内部是否含有任何 `UnsafePinned`(或 `PhantomPinned`),
/// 但不包括经由间接引用持有的情形。这会影响诸如:我们是否为 `&mut T` 生成
/// `noalias` 元数据。
///
/// 它是 [RFC 3467](https://rust-lang.github.io/rfcs/3467-unsafe-pinned.html)
/// 的一部分,追踪于
/// [#125735](https://github.com/rust-lang/rust/issues/125735)。
#[lang = "unsafe_unpin"]
pub(crate) unsafe auto trait UnsafeUnpin {}

impl<T: ?Sized> !UnsafeUnpin for UnsafePinned<T> {}
unsafe impl<T: ?Sized> UnsafeUnpin for PhantomData<T> {}
unsafe impl<T: ?Sized> UnsafeUnpin for *const T {}
unsafe impl<T: ?Sized> UnsafeUnpin for *mut T {}
unsafe impl<T: ?Sized> UnsafeUnpin for &T {}
unsafe impl<T: ?Sized> UnsafeUnpin for &mut T {}

/// 不需要任何 pin(固定)保证的类型。
///
/// 关于“pin”究竟是什么,请参见 [`pin` 模块][`pin` module] 文档。
///
/// 为 `T` 实现 `Unpin` trait,表达的是这样一个事实:`T` 对 pin 不敏感——
/// 它既不依赖、也不暴露任何 pin 相关的保证。这进而意味着,指向此类类型的
/// `Pin` 包装指针可以提供一套 *完全不受限* 的 API。换句话说,如果
/// `T: Unpin`,那么即便一个 `T` 类型的值被指向它的 [`Pin<Ptr>`]“固定”住,
/// 它也 *不会* 受到 pin 通常所施加的不变量约束。当一个 `T` 类型的值被
/// [`Pin<Ptr>`] 指向时,[`Pin`] 不会像通常那样限制对被指向值的访问,从而
/// 允许用户做任何用一个非 [`Pin`] 包装的 `Ptr` 所能做的事情。
///
/// 这个 trait 的用意,是缓解那些“为保证可靠性而要求使用 [`Pin`] 的 API”所
/// 带来的人体工程学退化,同时让那些不关心 pin 的类型也能使用这些 API。此类
/// API 的典型代表是 [`Future::poll`]。有许多 [`Future`] 类型并不关心 pin。
/// 这些 future 可以实现 `Unpin`,从而绕开该 API 中与 pin 相关的限制;与此
/// 同时,那些 *确实* 需要 pin 的 [`Future`] 子集仍然能够被可靠地实现。
///
/// 关于 [`Unpin`] 在整个 pin 系统这一更广阔背景下的后果,更多讨论见
/// [`pin` 模块][`pin` module] 中[关于 `Unpin` 的小节][section about `Unpin`]。
///
/// `Unpin` 对未被固定(non-pinned)的数据完全没有任何影响。特别地,
/// [`mem::replace`] 可以照常移动 `!Unpin` 的数据——而这些数据一旦被固定就是
/// 不可移动的([`mem::replace`] 对任意 `&mut T` 都适用,并不限于 `T: Unpin`)。
///
/// *然而*,你不能对 *已被固定* 的 `!Unpin` 数据使用 [`mem::replace`]——所谓
/// 已被固定,是指它被包裹在一个指向它的 [`Pin<Ptr>`] 之内。这是因为你无法
/// (安全地)用一个 [`Pin<Ptr>`] 取得指向其被指向值的 `&mut T`,而调用
/// [`mem::replace`] 又恰恰需要这样一个引用;*这一点* 正是整个机制得以成立的
/// 根本。
///
/// 因此,例如下面这件事,只能在实现了 `Unpin` 的类型上做:
///
/// ```rust
/// # #![allow(unused_must_use)]
/// use std::mem;
/// use std::pin::Pin;
///
/// let mut string = "this".to_string();
/// let mut pinned_string = Pin::new(&mut string);
///
/// // 我们需要一个可变引用才能调用 `mem::replace`。
/// // 我们可以通过(隐式地)调用 `Pin::deref_mut` 来取得这样一个引用,
/// // 但这之所以可行,只是因为 `String` 实现了 `Unpin`。
/// mem::replace(&mut *pinned_string, "other".to_string());
/// ```
///
/// 本 trait 会自动为几乎所有类型实现。编译器可以采取保守立场:只要构成某个
/// 类型各字段的类型也都是 [`Unpin`],就把该类型标记为 [`Unpin`]。这么做是
/// 安全的,因为如果一个类型实现了 [`Unpin`],那么该类型的实现为了可靠性而
/// 依赖 pin 相关保证就是不可靠的——*即便* 透过一个“pin”指针来看待它也是
/// 如此!对于一个为保证可靠性而依赖 pin 的类型,确保它 *不* 被标记为
/// [`Unpin`] 是其实现者的责任,做法是添加一个 [`PhantomPinned`] 字段。
/// 更多细节参见 [`pin` 模块][`pin` module] 文档。
///
/// [`mem::replace`]: crate::mem::replace "mem replace"
/// [`Future`]: crate::future::Future "Future"
/// [`Future::poll`]: crate::future::Future::poll "Future poll"
/// [`Pin`]: crate::pin::Pin "Pin"
/// [`Pin<Ptr>`]: crate::pin::Pin "Pin"
/// [`pin` module]: crate::pin "pin module"
/// [section about `Unpin`]: crate::pin#unpin "pin module docs about unpin"
/// [`unsafe`]: ../../std/keyword.unsafe.html "keyword unsafe"
#[stable(feature = "pin", since = "1.33.0")]
#[diagnostic::on_unimplemented(
    note = "consider using the `pin!` macro\nconsider using `Box::pin` if you need to access the pinned value outside of the current scope",
    message = "`{Self}` cannot be unpinned"
)]
#[lang = "unpin"]
pub auto trait Unpin {}

/// 一个不实现 `Unpin` 的标记类型。
///
/// 如果一个类型含有 `PhantomPinned`,那么它默认就不会实现 `Unpin`。
//
// FIXME(unsafe_pinned):这 *不是* 我们想要做出的稳定保证,至少现在还不是。
// 注意,为了与新的 [`UnsafePinned`] 包装类型保持向后兼容,在你的结构体中
// 放置这个标记的效果,等同于把整个结构体包裹进了一个 `UnsafePinned`。本类型
// 最终很可能会被废弃,所有新代码都应改用 `UnsafePinned`。
#[stable(feature = "pin", since = "1.33.0")]
#[derive(Debug, Default, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct PhantomPinned;

#[stable(feature = "pin", since = "1.33.0")]
impl !Unpin for PhantomPinned {}

// 这是一个小小的 hack,用来让现有那些使用 PhantomPinned 的代码能够选择退出
// noalias 并继续正常工作。理想情况下,PhantomPinned 只需包裹一个
// `UnsafePinned<()>` 就能得到同样的效果,但我们无法给一个已经稳定的单元
// 结构体新增字段——那会是破坏性变更。
impl !UnsafeUnpin for PhantomPinned {}

marker_impls! {
    #[stable(feature = "pin", since = "1.33.0")]
    Unpin for
        {T: PointeeSized} &T,
        {T: PointeeSized} &mut T,
}

marker_impls! {
    #[stable(feature = "pin_raw", since = "1.38.0")]
    Unpin for
        {T: PointeeSized} *const T,
        {T: PointeeSized} *mut T,
}

/// 标记可被 drop(析构)的类型。
///
/// 它应当用于 `[const]` 约束,因为非 const 的约束对每一种类型都恒成立。
#[unstable(feature = "const_destruct", issue = "133214")]
#[rustc_const_unstable(feature = "const_destruct", issue = "133214")]
#[lang = "destruct"]
#[rustc_on_unimplemented(message = "can't drop `{Self}`", append_const_msg)]
#[rustc_deny_explicit_impl]
#[rustc_do_not_implement_via_object]
pub const trait Destruct: PointeeSized {}

/// 标记元组类型。
///
/// 本 trait 的实现是内建的,无法为任何用户类型实现。
#[unstable(feature = "tuple_trait", issue = "none")]
#[lang = "tuple_trait"]
#[diagnostic::on_unimplemented(message = "`{Self}` is not a tuple")]
#[rustc_deny_explicit_impl]
#[rustc_do_not_implement_via_object]
pub trait Tuple {}

/// 标记那些可以用作 `const` 泛型参数类型的类型。
///
/// 这些类型必须具有恰当的等价关系(`Eq`),而且该关系必须是自动派生的
/// (`StructuralPartialEq`)。编译器中有一处硬编码的检查,确保所有字段也都是
/// `ConstParamTy`,这隐含着:递归地看,所有字段都满足 `StructuralPartialEq`。
#[lang = "const_param_ty"]
#[unstable(feature = "unsized_const_params", issue = "95174")]
#[diagnostic::on_unimplemented(message = "`{Self}` can't be used as a const parameter type")]
#[allow(multiple_supertrait_upcastable)]
// 我们给这个 trait 起的名字与派生宏不同,以便 `adt_const_params` 可以独立于
// `unsized_const_params` 使用,而不必每次都写出派生宏的完整路径。稳定化时
// 应当对其重命名。
pub trait ConstParamTy_: StructuralPartialEq + Eq {}

/// 生成 `ConstParamTy` trait 实现的派生宏。
#[rustc_builtin_macro]
#[allow_internal_unstable(unsized_const_params)]
#[unstable(feature = "adt_const_params", issue = "95174")]
pub macro ConstParamTy($item:item) {
    /* 编译器内建 */
}

// FIXME(adt_const_params):处理 `ty::FnDef`/`ty::Closure`
marker_impls! {
    #[unstable(feature = "adt_const_params", issue = "95174")]
    ConstParamTy_ for
        usize, u8, u16, u32, u64, u128,
        isize, i8, i16, i32, i64, i128,
        bool,
        char,
        (),
        {T: ConstParamTy_, const N: usize} [T; N],
}

marker_impls! {
    #[unstable(feature = "unsized_const_params", issue = "95174")]
    #[unstable_feature_bound(unsized_const_params)]
    ConstParamTy_ for
        str,
        {T: ConstParamTy_} [T],
        {T: ConstParamTy_ + ?Sized} &T,
}

/// 由所有函数指针共同实现的一个 trait。
//
// 注意,尽管本 trait 是内部的、unstable 的,它仍然作为稳定函数
// `core::ptr::fn_addr_eq` 的一个公开约束被暴露出来。
#[unstable(
    feature = "fn_ptr_trait",
    issue = "none",
    reason = "internal trait for implementing various traits for all function pointers"
)]
#[lang = "fn_ptr_trait"]
#[rustc_deny_explicit_impl]
#[rustc_do_not_implement_via_object]
pub trait FnPtr: Copy + Clone {
    /// 返回该函数指针的地址。
    #[lang = "fn_ptr_addr"]
    fn addr(self) -> *const ();
}

/// 让智能指针能够配合 trait 对象使用的派生宏。
///
/// # 这个宏做了什么
///
/// 本宏旨在用于用户自定义的指针类型,使得对该自定义指针的被指向者
/// (pointee)进行强转(coercion)成为可能。这包含两个方面:
///
/// ## 被指向者的去尺寸化强转
///
/// 借助本宏,下面这个例子就能通过编译:
/// ```
/// #![feature(derive_coerce_pointee)]
/// use std::marker::CoercePointee;
/// use std::ops::Deref;
///
/// #[derive(CoercePointee)]
/// #[repr(transparent)]
/// struct MySmartPointer<T: ?Sized>(Box<T>);
///
/// impl<T: ?Sized> Deref for MySmartPointer<T> {
///     type Target = T;
///     fn deref(&self) -> &T {
///         &self.0
///     }
/// }
///
/// trait MyTrait {}
///
/// impl MyTrait for i32 {}
///
/// fn main() {
///     let ptr: MySmartPointer<i32> = MySmartPointer(Box::new(4));
///
///     // 没有这个派生宏的话,这次强转会是一个错误。
///     let ptr: MySmartPointer<dyn MyTrait> = ptr;
/// }
/// ```
/// 如果没有 `#[derive(CoercePointee)]` 宏,该例子会以如下错误失败:
/// ```text
/// error[E0308]: mismatched types
///   --> src/main.rs:11:44
///    |
/// 11 |     let ptr: MySmartPointer<dyn MyTrait> = ptr;
///    |              ---------------------------   ^^^ expected `MySmartPointer<dyn MyTrait>`, found `MySmartPointer<i32>`
///    |              |
///    |              expected due to this
///    |
///    = note: expected struct `MySmartPointer<dyn MyTrait>`
///               found struct `MySmartPointer<i32>`
///    = help: `i32` implements `MyTrait` so you could box the found value and coerce it to the trait object `Box<dyn MyTrait>`, you will have to change the expected type as well
/// ```
///
/// ## dyn 兼容性
///
/// 本宏允许你在这个用户自定义指针类型上进行动态分发。也就是说,以该类型作为
/// 接收者(receiver)的 trait 是 dyn 兼容的。例如,下面这段代码可以编译:
///
/// ```
/// #![feature(arbitrary_self_types, derive_coerce_pointee)]
/// use std::marker::CoercePointee;
/// use std::ops::Deref;
///
/// #[derive(CoercePointee)]
/// #[repr(transparent)]
/// struct MySmartPointer<T: ?Sized>(Box<T>);
///
/// impl<T: ?Sized> Deref for MySmartPointer<T> {
///     type Target = T;
///     fn deref(&self) -> &T {
///         &self.0
///     }
/// }
///
/// // 你随时都可以定义这个 trait。(只要你启用了 #![feature(arbitrary_self_types)])
/// trait MyTrait {
///     fn func(self: MySmartPointer<Self>);
/// }
///
/// // 但使用 `dyn MyTrait` 需要 #[derive(CoercePointee)]。
/// fn call_func(value: MySmartPointer<dyn MyTrait>) {
///     value.func();
/// }
/// ```
/// 如果你从该结构体上移除 `#[derive(CoercePointee)]` 标注,那么上面的例子会以
/// 这样的错误信息失败:
/// ```text
/// error[E0038]: the trait `MyTrait` is not dyn compatible
///   --> src/lib.rs:21:36
///    |
/// 17 |     fn func(self: MySmartPointer<Self>);
///    |                   -------------------- help: consider changing method `func`'s `self` parameter to be `&self`: `&Self`
/// ...
/// 21 | fn call_func(value: MySmartPointer<dyn MyTrait>) {
///    |                                    ^^^^^^^^^^^ `MyTrait` is not dyn compatible
///    |
/// note: for a trait to be dyn compatible it needs to allow building a vtable
///       for more information, visit <https://doc.rust-lang.org/reference/items/traits.html#object-safety>
///   --> src/lib.rs:17:19
///    |
/// 16 | trait MyTrait {
///    |       ------- this trait is not dyn compatible...
/// 17 |     fn func(self: MySmartPointer<Self>);
///    |                   ^^^^^^^^^^^^^^^^^^^^ ...because method `func`'s `self` parameter cannot be dispatched on
/// ```
///
/// # 使用本宏的要求
///
/// 本宏只有在满足以下条件时才可使用:
/// * 该类型是一个 `#[repr(transparent)]` 结构体。
/// * 其非零大小字段的类型,要么是一个标准库指针类型(引用、裸指针、`NonNull`、
///   `Box`、`Rc`、`Arc` 等),要么是另一个同样使用 `#[derive(CoercePointee)]`
///   宏的用户自定义类型。
/// * 零大小字段不得提及任何泛型参数,除非该零大小字段的类型是 [`PhantomData`]。
///
/// ## 多个类型参数
///
/// 如果该类型有多个类型参数,那么你必须显式指定其中哪一个用于动态分发。例如:
/// ```
/// # #![feature(derive_coerce_pointee)]
/// # use std::marker::{CoercePointee, PhantomData};
/// #[derive(CoercePointee)]
/// #[repr(transparent)]
/// struct MySmartPointer<#[pointee] T: ?Sized, U> {
///     ptr: Box<T>,
///     _phantom: PhantomData<U>,
/// }
/// ```
/// 当结构体只有一个类型参数时,指定 `#[pointee]` 是允许的,但不是必需的。
///
/// # 示例
///
/// 一个 `Rc` 类型的自定义实现:
/// ```
/// #![feature(derive_coerce_pointee)]
/// use std::marker::CoercePointee;
/// use std::ops::Deref;
/// use std::ptr::NonNull;
///
/// #[derive(CoercePointee)]
/// #[repr(transparent)]
/// pub struct Rc<T: ?Sized> {
///     inner: NonNull<RcInner<T>>,
/// }
///
/// struct RcInner<T: ?Sized> {
///     refcount: usize,
///     value: T,
/// }
///
/// impl<T: ?Sized> Deref for Rc<T> {
///     type Target = T;
///     fn deref(&self) -> &T {
///         let ptr = self.inner.as_ptr();
///         unsafe { &(*ptr).value }
///     }
/// }
///
/// impl<T> Rc<T> {
///     pub fn new(value: T) -> Self {
///         let inner = Box::new(RcInner {
///             refcount: 1,
///             value,
///         });
///         Self {
///             inner: NonNull::from(Box::leak(inner)),
///         }
///     }
/// }
///
/// impl<T: ?Sized> Clone for Rc<T> {
///     fn clone(&self) -> Self {
///         // 真实的实现会在这里处理溢出。
///         unsafe { (*self.inner.as_ptr()).refcount += 1 };
///         Self { inner: self.inner }
///     }
/// }
///
/// impl<T: ?Sized> Drop for Rc<T> {
///     fn drop(&mut self) {
///         let ptr = self.inner.as_ptr();
///         unsafe { (*ptr).refcount -= 1 };
///         if unsafe { (*ptr).refcount } == 0 {
///             drop(unsafe { Box::from_raw(ptr) });
///         }
///     }
/// }
/// ```
#[rustc_builtin_macro(CoercePointee, attributes(pointee))]
#[allow_internal_unstable(dispatch_from_dyn, coerce_unsized, unsize, coerce_pointee_validated)]
#[rustc_diagnostic_item = "CoercePointee"]
#[unstable(feature = "derive_coerce_pointee", issue = "123430")]
pub macro CoercePointee($item:item) {
    /* 编译器内建 */
}

/// 一个为带有 `derive(CoercePointee)` 的 ADT 实现的 trait,使编译器能够在派生
/// 宏展开之后强制检查这些派生 impl 是有效的——因为该派生宏的要求比手写这些
/// impl 时更为严格。
///
/// 本 trait 不打算供用户实现,也不打算用于校验之外的任何用途,因此它永远不应
/// 被稳定化。
#[lang = "coerce_pointee_validated"]
#[unstable(feature = "coerce_pointee_validated", issue = "none")]
#[doc(hidden)]
pub trait CoercePointeeValidated {
    /* 编译器内建 */
}
