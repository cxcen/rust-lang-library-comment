//! 面向那些无法被“隐式复制”的类型的 `Clone` trait。
//!
//! 在 Rust 中,一些简单类型是“可隐式复制的”:当你对它们赋值或把它们作为
//! 参数传递时,接收方会得到一份拷贝,而原值仍然保留在原地。这些类型在复制时
//! 既不需要堆分配,也没有终结器(finalizer,即它们不含有被拥有的 box,也不
//! 实现 [`Drop`]),因此编译器认为复制它们既廉价又安全。对于其他类型,则必须
//! 显式地进行复制——按惯例,做法是实现 [`Clone`] trait 并调用 [`clone`] 方法。
//!
//! [`clone`]: Clone::clone
//!
//! 基本用法示例:
//!
//! ```
//! let s = String::new(); // String 类型实现了 Clone
//! let copy = s.clone(); // 因此我们可以克隆它
//! ```
//!
//! 要轻松地实现 Clone trait,你也可以使用 `#[derive(Clone)]`。示例:
//!
//! ```
//! #[derive(Clone)] // 我们给 Morpheus 结构体加上 Clone trait
//! struct Morpheus {
//!    blue_pill: f32,
//!    red_pill: i64,
//! }
//!
//! fn main() {
//!    let f = Morpheus { blue_pill: 0.0, red_pill: 0 };
//!    let copy = f.clone(); // 现在我们就能克隆它了!
//! }
//! ```

#![stable(feature = "rust1", since = "1.0.0")]

use crate::marker::{Destruct, PointeeSized};

mod uninit;

/// 一个通用 trait,允许显式地创建值的副本。
///
/// 调用 [`clone`] 总是产生一个新值。然而,对于那些本身是“指向其他数据的
/// 引用”的类型(例如智能指针或引用),这个新值可能仍然指向同一份底层数据,
/// 而非把数据复制一份。更多细节参见 [`Clone::clone`]。
///
/// 当在含有诸如 `Arc<Mutex<T>>` 这类智能指针的结构体上使用 `#[derive(Clone)]`
/// 时,这一区别尤为重要——克隆出来的结构体会与原结构体共享同一份可变状态。
///
/// 它与 [`Copy`] 的区别在于:[`Copy`] 是隐式发生的、廉价的按位复制,而
/// `Clone` 永远是显式的,并且可能廉价、也可能昂贵。[`Copy`] 没有任何方法,
/// 所以你无法改变它的行为;但实现 `Clone` 时,你提供的 `clone` 方法可以运行
/// 任意代码。
///
/// 由于 `Clone` 是 [`Copy`] 的 supertrait,任何实现了 `Copy` 的类型都必须
/// 同时实现 `Clone`。
///
/// ## 可派生(Derivable)
///
/// 如果所有字段都是 `Clone`,本 trait 就可以配合 `#[derive]` 使用。`derive`
/// 出来的 [`Clone`] 实现会对每个字段调用 [`clone`]。
///
/// [`clone`]: Clone::clone
///
/// 对于泛型结构体,`#[derive]` 会通过给泛型参数添加 `Clone` 约束来有条件地
/// 实现 `Clone`。
///
/// ```
/// // 当 T 是 Clone 时,`derive` 才为 Reading<T> 实现 Clone。
/// #[derive(Clone)]
/// struct Reading<T> {
///     frequency: T,
/// }
/// ```
///
/// ## 如何实现 `Clone`?
///
/// 凡是 [`Copy`] 的类型,其 `Clone` 实现都应当是平凡(trivial)的。更正式地说:
/// 若 `T: Copy`、`x: T` 且 `y: &T`,那么 `let x = y.clone();` 等价于
/// `let x = *y;`。手动实现时应当小心维持这一不变量;但是,unsafe 代码 **不得**
/// 为了保证内存安全而依赖它。
///
/// 一个例子是持有函数指针的泛型结构体。在这种情况下,`Clone` 的实现无法被
/// `derive`,但可以这样手动实现:
///
/// ```
/// struct Generate<T>(fn() -> T);
///
/// impl<T> Copy for Generate<T> {}
///
/// impl<T> Clone for Generate<T> {
///     fn clone(&self) -> Self {
///         *self
///     }
/// }
/// ```
///
/// 如果我们使用 `derive`:
///
/// ```
/// #[derive(Copy, Clone)]
/// struct Generate<T>(fn() -> T);
/// ```
///
/// 那么自动派生出来的实现会带有不必要的 `T: Copy` 和 `T: Clone` 约束:
///
/// ```
/// # struct Generate<T>(fn() -> T);
///
/// // 自动派生
/// impl<T: Copy> Copy for Generate<T> { }
///
/// // 自动派生
/// impl<T: Clone> Clone for Generate<T> {
///     fn clone(&self) -> Generate<T> {
///         Generate(Clone::clone(&self.0))
///     }
/// }
/// ```
///
/// 这些约束是不必要的,因为显然函数本身应当是可复制、可克隆的,即便它的返回
/// 类型并非如此:
///
/// ```compile_fail,E0599
/// #[derive(Copy, Clone)]
/// struct Generate<T>(fn() -> T);
///
/// struct NotCloneable;
///
/// fn generate_not_cloneable() -> NotCloneable {
///     NotCloneable
/// }
///
/// Generate(generate_not_cloneable).clone(); // 错误:trait 约束未被满足
/// // 注意:若改用上面的手动实现,这一行就能通过编译。
/// ```
///
/// ## `Clone` 与 `PartialEq`/`Eq`
/// `Clone` 的用途是复制对象。因此,当同时实现 `Clone` 和 [`PartialEq`] 时,
/// 期望以下性质成立:
/// ```text
/// x == x -> x.clone() == x
/// ```
/// 换言之,如果一个对象与它自身比较相等,那么它的克隆也必须与原对象比较相等。
///
/// 对于同时实现了 [`Eq`] 的类型(对它们而言 `x == x` 恒成立),这意味着
/// `x.clone() == x` 必须永远为真。诸如 [`HashMap`]、[`HashSet`]、[`BTreeMap`]、
/// [`BTreeSet`] 和 [`BinaryHeap`] 之类的标准库集合,依赖它们的键遵守这一性质
/// 才能正确工作。此外,这些集合还要求克隆一个键不会改变 [`Hash`] 和 [`Ord`]
/// 方法的结果。所幸,只要 `Hash` 和 `Ord` 按它们各自的要求正确实现,这一点
/// 就会从 `x.clone() == x` 自动得出。
///
/// 当使用 `#[derive(Clone, PartialEq)]` 同时派生 `Clone` 和 [`PartialEq`],
/// 或者使用 `#[derive(Clone, PartialEq, Eq)]` 额外派生 [`Eq`] 时,只要底层
/// 类型满足这一性质,它就会被自动维持。
///
/// 违反这一性质属于逻辑错误。逻辑错误所导致的行为是未指定的,但 trait 的
/// 使用者必须确保此类逻辑错误 *不会* 导致未定义行为。这意味着 `unsafe` 代码
/// **不得** 依赖这一性质被满足。
///
/// ## 额外的实现者
///
/// 除了[下方列出的实现者][impls],以下类型也实现了 `Clone`:
///
/// * 函数项类型(即为每个函数定义的那个独一无二的类型)
/// * 函数指针类型(例如 `fn() -> i32`)
/// * 闭包类型,前提是它们不从环境中捕获任何值,或者所有被捕获的值自身都实现
///   了 `Clone`。注意,以共享引用方式捕获的变量总是实现 `Clone`(即便被引用者
///   本身并不是),而以可变引用方式捕获的变量则永远不是 `Clone`。
///
/// [`HashMap`]: ../../std/collections/struct.HashMap.html
/// [`HashSet`]: ../../std/collections/struct.HashSet.html
/// [`BTreeMap`]: ../../std/collections/struct.BTreeMap.html
/// [`BTreeSet`]: ../../std/collections/struct.BTreeSet.html
/// [`BinaryHeap`]: ../../std/collections/struct.BinaryHeap.html
/// [impls]: #implementors
#[stable(feature = "rust1", since = "1.0.0")]
#[lang = "clone"]
#[rustc_diagnostic_item = "Clone"]
#[rustc_trivial_field_reads]
#[rustc_const_unstable(feature = "const_clone", issue = "142757")]
pub const trait Clone: Sized {
    /// 返回值的一份副本。
    ///
    /// 注意,“副本”究竟意味着什么因类型而异:
    /// - 对大多数类型,这会创建一份深的、独立的拷贝
    /// - 对像 `&T` 这样的引用类型,这会创建另一个指向同一个值的引用
    /// - 对像 [`Arc`] 或 [`Rc`] 这样的智能指针,这会递增引用计数,但仍然指向
    ///   同一份底层数据
    ///
    /// [`Arc`]: ../../std/sync/struct.Arc.html
    /// [`Rc`]: ../../std/rc/struct.Rc.html
    ///
    /// # 示例
    ///
    /// ```
    /// # #![allow(noop_method_call)]
    /// let hello = "Hello"; // &str 实现了 Clone
    ///
    /// assert_eq!("Hello", hello.clone());
    /// ```
    ///
    /// 以引用计数类型为例:
    ///
    /// ```
    /// use std::sync::{Arc, Mutex};
    ///
    /// let data = Arc::new(Mutex::new(vec![1, 2, 3]));
    /// let data_clone = data.clone(); // 创建另一个指向同一个 Mutex 的 Arc
    ///
    /// {
    ///     let mut lock = data.lock().unwrap();
    ///     lock.push(4);
    /// }
    ///
    /// // 由于二者共享同一份底层数据,改动透过克隆也是可见的
    /// assert_eq!(*data_clone.lock().unwrap(), vec![1, 2, 3, 4]);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[must_use = "cloning is often expensive and is not expected to have side effects"]
    // Clone::clone 是特殊的,因为编译器会为某些类型生成 MIR 来实现它。
    // 参见 InstanceKind::CloneShim。
    #[lang = "clone_fn"]
    fn clone(&self) -> Self;

    /// 从 `source` 执行复制赋值(copy-assignment)。
    ///
    /// `a.clone_from(&b)` 在功能上等价于 `a = b.clone()`,但可以被重写,
    /// 以复用 `a` 已有的资源,从而避免不必要的内存分配。
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    fn clone_from(&mut self, source: &Self)
    where
        Self: [const] Destruct,
    {
        *self = source.clone()
    }
}

/// 表明该 `Clone` 实现等同于直接复制值。
///
/// 标准库中的一些优化会用到它:它们针对本 trait 做特化,以便为诸如
/// [`clone_from_slice`](slice::clone_from_slice) 之类的函数选择更快的实现。
/// 使用 `#[derive(Clone, Copy)]` 时会自动实现它。
///
/// 注意,本 trait 并不蕴含该类型是 `Copy`,因为例如 `core::ops::Range<i32>`
/// 也可以可靠地(soundly)实现本 trait。
///
/// # 安全性(Safety)
/// `Clone::clone` 必须等同于复制值,否则调用诸如 `slice::clone_from_slice`
/// 之类的函数可能产生未定义行为。
#[unstable(
    feature = "trivial_clone",
    reason = "this isn't part of any API guarantee",
    issue = "none"
)]
#[rustc_const_unstable(feature = "const_clone", issue = "142757")]
#[lang = "trivial_clone"]
// SAFETY:
// 针对本 trait 做特化是可靠的,因为 `clone` 的实现不可能依赖于生命周期。
// 因此,只要 `TrivialClone` 对任意一个生命周期成立,那么只要 `Clone` 被实现,
// 它的不变量就成立——即便由于生命周期约束的缘故,实际的 `TrivialClone` 约束
// 本身不会被满足。
#[rustc_unsafe_specialization_marker]
// 如果写成 `#[derive(Clone, Clone, Copy)]`,就会出现多个 `TrivialClone` 的
// 实现。为了不让它出现在错误信息中,把它做成一个 `#[marker]` trait。
#[marker]
pub const unsafe trait TrivialClone: [const] Clone {}

/// 生成 `Clone` trait 实现的派生宏。
#[rustc_builtin_macro]
#[stable(feature = "builtin_macro_prelude", since = "1.38.0")]
#[allow_internal_unstable(core_intrinsics, derive_clone_copy_internals, trivial_clone)]
pub macro Clone($item:item) {
    /* 编译器内建 */
}

/// 面向那些 [`Clone`] 实现很轻量(例如基于引用计数)的对象的 trait。
///
/// 克隆一个实现了本 trait 的对象,一般应当:
/// - 无论该对象管理多少数据,都是 O(1)(常数)时间,
/// - 不需要内存分配,
/// - 不需要复制超过大约 64 字节(一个典型缓存行的大小)的数据,
/// - 不阻塞当前线程,
/// - 不产生任何语义上的副作用(例如分配一个文件描述符),并且
/// - 开销不超过寥寥几次原子操作。
///
/// `UseCloned` trait 不提供任何方法;它只是表明 `Clone::clone` 很轻量,
/// 并允许使用 `.use` 语法。
///
/// ## .use 后缀语法
///
/// 在你想要使用的值后面加上 `.use` 后缀,即可对该值执行 `.use`。
///
/// ```ignore (this won't work until we land use)
/// fn foo(f: Foo) {
///     // 如果 `Foo` 实现了 `Copy`,f 会被复制进 x。
///     // 如果 `Foo` 实现了 `UseCloned`,f 会被克隆进 x。
///     // 否则,f 会被 move 进 x。
///     let x = f.use;
///     // ...
/// }
/// ```
///
/// ## use 闭包
///
/// use 闭包允许被捕获的值被自动使用。这类似于有一个闭包,对每个被捕获的值
/// 都执行一次 `.use`。
#[unstable(feature = "ergonomic_clones", issue = "132290")]
#[lang = "use_cloned"]
pub trait UseCloned: Clone {
    // 空。
}

macro_rules! impl_use_cloned {
    ($($t:ty)*) => {
        $(
            #[unstable(feature = "ergonomic_clones", issue = "132290")]
            impl UseCloned for $t {}
        )*
    }
}

impl_use_cloned! {
    usize u8 u16 u32 u64 u128
    isize i8 i16 i32 i64 i128
             f16 f32 f64 f128
    bool char
}

// FIXME(aburka):这些结构体仅供 #[derive] 使用,用来断言某个类型的每个
// 组成部分都实现了 Clone 或 Copy。
//
// 这些结构体绝不应出现在用户代码中。
#[doc(hidden)]
#[allow(missing_debug_implementations)]
#[unstable(
    feature = "derive_clone_copy_internals",
    reason = "deriving hack, should not be public",
    issue = "none"
)]
pub struct AssertParamIsClone<T: Clone + PointeeSized> {
    _field: crate::marker::PhantomData<T>,
}
#[doc(hidden)]
#[allow(missing_debug_implementations)]
#[unstable(
    feature = "derive_clone_copy_internals",
    reason = "deriving hack, should not be public",
    issue = "none"
)]
pub struct AssertParamIsCopy<T: Copy + PointeeSized> {
    _field: crate::marker::PhantomData<T>,
}

/// 把 [`Clone`] 推广到存放于任意容器中的[动态大小类型][DST]。
///
/// 本 trait 已为所有实现了 [`Clone`] 的类型、所有此类类型的[切片](slice),
/// 以及标准库中的其他动态大小类型实现。你也可以实现本 trait 来支持克隆自定义
/// DST(含有动态大小字段的结构体),或者把它用作 supertrait 来支持克隆一个
/// [trait 对象][trait object]。
///
/// 本 trait 通常是通过支持 DST 的容器类型上的操作来间接使用的,因此你一般
/// 不需要显式调用 `.clone_to_uninit()`,除非你正在实现这样一个容器、以其他
/// 方式显式管理某块内存分配,或者正在实现 `CloneToUninit` 本身。
///
/// # 安全性(Safety)
///
/// 实现必须确保:当 `.clone_to_uninit(dest)` 正常返回(而非 panic)时,它
/// 总是把 `*dest` 初始化为一个有效的 `Self` 类型的值。
///
/// # 示例
///
// FIXME(#126799):当 `Box::clone` 允许使用 `CloneToUninit` 时,改用它重写
// 这些示例,因为 `Rc` 在这里是个干扰项。
///
/// 如果你正在定义一个 trait,你可以把 `CloneToUninit` 添加为 supertrait,
/// 以支持克隆你这个 trait 的 `dyn` 值:
///
/// ```
/// #![feature(clone_to_uninit)]
/// use std::rc::Rc;
///
/// trait Foo: std::fmt::Debug + std::clone::CloneToUninit {
///     fn modify(&mut self);
///     fn value(&self) -> i32;
/// }
///
/// impl Foo for i32 {
///     fn modify(&mut self) {
///         *self *= 10;
///     }
///     fn value(&self) -> i32 {
///         *self
///     }
/// }
///
/// let first: Rc<dyn Foo> = Rc::new(1234);
///
/// let mut second = first.clone();
/// Rc::make_mut(&mut second).modify(); // make_mut() 会调用 clone_to_uninit()
///
/// assert_eq!(first.value(), 1234);
/// assert_eq!(second.value(), 12340);
/// ```
///
/// 下面是一个为自定义 DST 实现 `CloneToUninit` 的例子。(它本质上是
/// `derive(CloneToUninit)`(假如存在这样一个派生宏的话)所做之事的一个
/// 受限版本。)
///
/// ```
/// #![feature(clone_to_uninit)]
/// use std::clone::CloneToUninit;
/// use std::mem::offset_of;
/// use std::rc::Rc;
///
/// #[derive(PartialEq)]
/// struct MyDst<T: ?Sized> {
///     label: String,
///     contents: T,
/// }
///
/// unsafe impl<T: ?Sized + CloneToUninit> CloneToUninit for MyDst<T> {
///     unsafe fn clone_to_uninit(&self, dest: *mut u8) {
///         // `self.contents` 的偏移是动态的,因为它取决于 T 的对齐,而后者
///         // 可能是动态的(如果 `T = dyn SomeTrait`)。因此我们必须通过检查
///         // `self` 来动态地取得这个偏移,而不能使用 `offset_of!`。
///         //
///         // SAFETY:按定义,`self` 指向与 `&self.contents` 同一块分配中、
///         // 位于其之前某处的位置。
///         let offset_of_contents = unsafe {
///             (&raw const self.contents).byte_offset_from_unsigned(self)
///         };
///
///         // 克隆 `self` 那些 *定长* 的字段(本例中只有一个)。
///         // (先克隆它并临时存入一个局部变量,这样万一发生 panic,我们可以
///         // 借助局部变量的普通自动清理来避免泄漏它。这样的泄漏虽然是可靠的,
///         // 但并不可取。)
///         let label = self.label.clone();
///
///         // SAFETY:调用者必须提供一个 `dest`,使得这些字段偏移处可以安全写入。
///         unsafe {
///             // 直接把不定长字段从 `self` 克隆到 `dest`。
///             self.contents.clone_to_uninit(dest.add(offset_of_contents));
///
///             // 现在写入所有定长字段。
///             //
///             // 注意,只有在所有 clone() 和 clone_to_uninit() 调用都完成
///             // 之后我们才这样做,因此我们确知不会再有可能的 panic;这就
///             // 确保了万一 panic 也不会内存泄漏。
///             dest.add(offset_of!(Self, label)).cast::<String>().write(label);
///         }
///         // 该结构体的所有字段都已被初始化;因此该结构体已被初始化,
///         // 我们也就履行了 `unsafe impl CloneToUninit` 的义务。
///     }
/// }
///
/// fn main() {
///     // 构造 MyDst<[u8; 4]>,然后强转为 MyDst<[u8]>。
///     let first: Rc<MyDst<[u8]>> = Rc::new(MyDst {
///         label: String::from("hello"),
///         contents: [1, 2, 3, 4],
///     });
///
///     let mut second = first.clone();
///     // make_mut() 会调用 clone_to_uninit()。
///     for elem in Rc::make_mut(&mut second).contents.iter_mut() {
///         *elem *= 10;
///     }
///
///     assert_eq!(first.contents, [1, 2, 3, 4]);
///     assert_eq!(second.contents, [10, 20, 30, 40]);
///     assert_eq!(second.label, "hello");
/// }
/// ```
///
/// # 另请参阅
///
/// * [`Clone::clone_from`] 是一个安全函数,当 [`Self: Sized`](Sized) 且目标
///   已经初始化时,可以改用它;它也许能复用目标已拥有的内存分配,而
///   `clone_to_uninit` 不能,因为它假定其目标是未初始化的。
/// * [`ToOwned`],它会分配一个新的目标容器。
///
/// [`ToOwned`]: ../../std/borrow/trait.ToOwned.html
/// [DST]: https://doc.rust-lang.org/reference/dynamically-sized-types.html
/// [trait object]: https://doc.rust-lang.org/reference/types/trait-object.html
#[unstable(feature = "clone_to_uninit", issue = "126799")]
pub unsafe trait CloneToUninit {
    /// 从 `self` 向 `dest` 执行复制赋值。
    ///
    /// 这类似于 `std::ptr::write(dest.cast(), self.clone())`,区别在于 `Self`
    /// 可以是一个动态大小类型([`!Sized`](Sized))。
    ///
    /// 在本函数被调用之前,`dest` 可能指向未初始化的内存。在本函数被调用之后,
    /// `dest` 将指向已初始化的内存;此时,用该指针配合来自 `self` 的[指针
    /// 元数据][pointer metadata]创建一个 `&Self` 引用将是可靠的。
    ///
    /// # 安全性(Safety)
    ///
    /// 如果违反以下任一条件,行为即为未定义:
    ///
    /// * `dest` 对于写入 `size_of_val(self)` 个字节必须是[有效的][valid]。
    /// * `dest` 必须按 `align_of_val(self)` 正确对齐。
    ///
    /// [valid]: crate::ptr#safety
    /// [pointer metadata]: crate::ptr::metadata()
    ///
    /// # Panics
    ///
    /// 本函数可能会 panic。(例如,如果为 `self` 所拥有的某个值的克隆分配内存
    /// 失败,它就可能 panic。)如果调用发生 panic,那么 `*dest` 应被视为
    /// 未初始化的内存;它不得被读取或被 drop,因为即便它先前是有效的,此时
    /// 也可能已被部分覆写。
    ///
    /// 调用者也许希望注意:在适用的情况下,释放 `dest` 所指向的那块内存分配,
    /// 以避免内存泄漏(但这不是强制要求)。
    ///
    /// 实现者应避免泄漏值:在栈展开(unwinding)时,drop 掉所有可能已经创建出
    /// 来的组成部分的值。(例如,如果正在克隆一个长度为 3 的 `[Foo]`,而三次
    /// `Foo::clone()` 调用中的第二次发生了栈展开,那么第一个已克隆出的 `Foo`
    /// 就应当被 drop 掉。)
    unsafe fn clone_to_uninit(&self, dest: *mut u8);
}

#[unstable(feature = "clone_to_uninit", issue = "126799")]
unsafe impl<T: Clone> CloneToUninit for T {
    #[inline]
    unsafe fn clone_to_uninit(&self, dest: *mut u8) {
        // SAFETY:我们正在调用一个契约相同的特化版本
        unsafe { <T as self::uninit::CopySpec>::clone_one(self, dest.cast::<T>()) }
    }
}

#[unstable(feature = "clone_to_uninit", issue = "126799")]
unsafe impl<T: Clone> CloneToUninit for [T] {
    #[inline]
    #[cfg_attr(debug_assertions, track_caller)]
    unsafe fn clone_to_uninit(&self, dest: *mut u8) {
        let dest: *mut [T] = dest.with_metadata_of(self);
        // SAFETY:我们正在调用一个契约相同的特化版本
        unsafe { <T as self::uninit::CopySpec>::clone_slice(self, dest) }
    }
}

#[unstable(feature = "clone_to_uninit", issue = "126799")]
unsafe impl CloneToUninit for str {
    #[inline]
    #[cfg_attr(debug_assertions, track_caller)]
    unsafe fn clone_to_uninit(&self, dest: *mut u8) {
        // SAFETY:str 不过是一个带有 UTF-8 不变量的 [u8]
        unsafe { self.as_bytes().clone_to_uninit(dest) }
    }
}

#[unstable(feature = "clone_to_uninit", issue = "126799")]
unsafe impl CloneToUninit for crate::ffi::CStr {
    #[cfg_attr(debug_assertions, track_caller)]
    unsafe fn clone_to_uninit(&self, dest: *mut u8) {
        // SAFETY:目前,CStr 不过是一个带有若干不变量的 #[repr(transparent)] [c_char]。
        // 而且在所有受支持的平台上,我们都能把 [c_char] 转为 [u8](参见:to_bytes_with_nul)。
        // 指针元数据正确地保留了长度(因此 NUL 也被复制了)。
        // 参见测试中的 `cstr_metadata_is_length_with_nul`。
        unsafe { self.to_bytes_with_nul().clone_to_uninit(dest) }
    }
}

#[unstable(feature = "bstr", issue = "134915")]
unsafe impl CloneToUninit for crate::bstr::ByteStr {
    #[inline]
    #[cfg_attr(debug_assertions, track_caller)]
    unsafe fn clone_to_uninit(&self, dst: *mut u8) {
        // SAFETY:ByteStr 是一个围绕 `[u8]` 的 `#[repr(transparent)]` 包装
        unsafe { self.as_bytes().clone_to_uninit(dst) }
    }
}

/// 为原始类型实现 `Clone`。
///
/// 那些无法用 Rust 描述的实现,定义在 `rustc_trait_selection` 中的
/// `traits::SelectionContext::copy_clone_conditions()` 里。
mod impls {
    use super::TrivialClone;
    use crate::marker::PointeeSized;

    macro_rules! impl_clone {
        ($($t:ty)*) => {
            $(
                #[stable(feature = "rust1", since = "1.0.0")]
                #[rustc_const_unstable(feature = "const_clone", issue = "142757")]
                impl const Clone for $t {
                    #[inline(always)]
                    fn clone(&self) -> Self {
                        *self
                    }
                }

                #[doc(hidden)]
                #[unstable(feature = "trivial_clone", issue = "none")]
                #[rustc_const_unstable(feature = "const_clone", issue = "142757")]
                unsafe impl const TrivialClone for $t {}
            )*
        }
    }

    impl_clone! {
        usize u8 u16 u32 u64 u128
        isize i8 i16 i32 i64 i128
        f16 f32 f64 f128
        bool char
    }

    #[unstable(feature = "never_type", issue = "35121")]
    #[rustc_const_unstable(feature = "const_clone", issue = "142757")]
    impl const Clone for ! {
        #[inline]
        fn clone(&self) -> Self {
            *self
        }
    }

    #[doc(hidden)]
    #[unstable(feature = "trivial_clone", issue = "none")]
    #[rustc_const_unstable(feature = "const_clone", issue = "142757")]
    unsafe impl const TrivialClone for ! {}

    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_unstable(feature = "const_clone", issue = "142757")]
    impl<T: PointeeSized> const Clone for *const T {
        #[inline(always)]
        fn clone(&self) -> Self {
            *self
        }
    }

    #[doc(hidden)]
    #[unstable(feature = "trivial_clone", issue = "none")]
    #[rustc_const_unstable(feature = "const_clone", issue = "142757")]
    unsafe impl<T: PointeeSized> const TrivialClone for *const T {}

    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_unstable(feature = "const_clone", issue = "142757")]
    impl<T: PointeeSized> const Clone for *mut T {
        #[inline(always)]
        fn clone(&self) -> Self {
            *self
        }
    }

    #[doc(hidden)]
    #[unstable(feature = "trivial_clone", issue = "none")]
    #[rustc_const_unstable(feature = "const_clone", issue = "142757")]
    unsafe impl<T: PointeeSized> const TrivialClone for *mut T {}

    /// 共享引用可以被克隆,但可变引用 *不可以*!
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_unstable(feature = "const_clone", issue = "142757")]
    impl<T: PointeeSized> const Clone for &T {
        #[inline(always)]
        #[rustc_diagnostic_item = "noop_method_clone"]
        fn clone(&self) -> Self {
            self
        }
    }

    #[doc(hidden)]
    #[unstable(feature = "trivial_clone", issue = "none")]
    #[rustc_const_unstable(feature = "const_clone", issue = "142757")]
    unsafe impl<T: PointeeSized> const TrivialClone for &T {}

    /// 共享引用可以被克隆,但可变引用 *不可以*!
    #[stable(feature = "rust1", since = "1.0.0")]
    impl<T: PointeeSized> !Clone for &mut T {}
}
