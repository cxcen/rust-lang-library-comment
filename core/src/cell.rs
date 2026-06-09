//! 可共享的可变容器。
//!
//! Rust 的内存安全建立在这样一条规则之上:给定一个对象 `T`,在任意时刻只能拥有以下二者之一:
//!
//! - 指向该对象的若干个不可变引用(`&T`)(即所谓的**别名 aliasing**)。
//! - 指向该对象的唯一一个可变引用(`&mut T`)(即所谓的**可变性 mutability**)。
//!
//! 这条规则由 Rust 编译器强制执行。然而在某些场景下它不够灵活:有时我们既需要对同一对象持有
//! 多个引用,又需要修改它。
//!
//! 可共享的可变容器正是为了在存在别名的情况下,以受控的方式允许修改而存在的。
//! [`Cell<T>`]、[`RefCell<T>`] 和 [`OnceCell<T>`] 以**单线程**的方式实现这一点——它们都不实现
//! [`Sync`]。(如果你需要在多个线程之间同时进行别名与修改,正确的数据结构是 [`Mutex<T>`]、
//! [`RwLock<T>`]、[`OnceLock<T>`] 或 [`atomic`] 原子类型。)
//!
//! `Cell<T>`、`RefCell<T>` 和 `OnceCell<T>` 类型的值可以通过共享引用(即常见的 `&T` 类型)
//! 来修改,而大多数 Rust 类型只能通过独占引用(`&mut T`)修改。我们称这些 cell 类型提供
//! “内部可变性(interior mutability)”(可经由 `&T` 修改),以区别于绝大多数 Rust 类型所
//! 体现的“继承可变性(inherited mutability)”(只能经由 `&mut T` 修改)。这不是让借用规则
//! 消失,而是把“何时允许读写”这件事转移给 cell 类型自身维护:有的通过按值移动规避引用,
//! 有的通过运行时计数检查,有的通过一次性初始化不变量限制写入次数。
//!
//! ## 设计背景:内部可变性是对借用规则的受控突破
//!
//! Rust 默认的规则是“共享即不可变(`&T`)/ 独占才可变(`&mut T`)”。内部可变性允许通过共享引用
//! `&T` 去修改其指向的数据,这是对该规则的一次**受控突破**。它之所以能够安全成立,完全依赖于一个
//! 唯一的合法基石——[`UnsafeCell<T>`](见本模块末尾的类型文档)。
//!
//! `UnsafeCell<T>` 是一个 lang item,是标准库中**唯一**能合法地“透过 `&` 得到指向内部数据的
//! `&mut`”的类型。编译器据此对 `UnsafeCell` 的内容**关闭别名优化**(即不再对其内容套用“`&T`
//! 指向的内存在引用存活期间不会改变”这一 no-aliasing 假设)。换言之,所有内部可变性最终都必须
//! 经由 `UnsafeCell` 实现。如果不经过 `UnsafeCell`,而是用 `transmute` 或裸指针强行绕过 `&T`
//! 的不可变性去修改数据,编译器仍会假设 `&T` 所指内存不变,从而产生未定义行为(UB)。这条
//! “唯有 `UnsafeCell` 才能让编译器放弃不变性假设”的契约是整个模块的根基,务必牢记。
//!
//! cell 类型共有四种风味:`Cell<T>`、`RefCell<T>`、`OnceCell<T>` 和 `LazyCell<T>`。
//! 它们各自以不同方式提供安全的内部可变性。
//! 因为这些类型允许在存在共享引用时修改内部状态,它们默认都停在单线程边界内:`Cell<T>`、
//! `RefCell<T>`、`OnceCell<T>` 和 `LazyCell<T>` 都是 `!Sync`。这条边界很重要:跨线程共享时,
//! 单靠 `UnsafeCell` 并不会提供同步,如果没有锁或原子操作来协调冲突访问,就会变成数据竞争 UB。
//!
//! ## `Cell<T>`
//!
//! [`Cell<T>`] 通过将值整体移入、移出 cell 来实现内部可变性。也就是说,永远无法取得指向内部值的
//! `&T`,而要直接拿到那个值本身,就必须用别的值把它替换出来。它没有任何运行时借用检查,因此
//! get/set/replace 等操作开销几乎为零。之所以**不能**给出指向内部值的引用,正是为了避免产生
//! 别名:既然拿不到内部引用,就不可能出现“引用还活着、值却被换掉”的情形。该类型提供以下方法:
//! 对 `Copy` 类型来说,`get` 读取的是一份按值拷贝;对所有类型来说,`set`/`replace` 都是在原位置
//! 写入一个新值并按值取出或丢弃旧值。因此 `Cell` 适合那些“状态就是一个小值”的场景,而不适合
//! 需要长期借用内部字段的场景。
//!
//!  - 对实现了 [`Copy`] 的类型,[`get`](Cell::get) 方法通过复制返回当前内部值的一份拷贝。
//!  - 对实现了 [`Default`] 的类型,[`take`](Cell::take) 方法用 [`Default::default()`] 替换
//!    当前内部值,并返回被替换出来的旧值。
//!  - 所有类型都拥有:
//!    - [`replace`](Cell::replace):替换当前内部值并返回被替换出来的旧值。
//!    - [`into_inner`](Cell::into_inner):消耗整个 `Cell<T>` 并返回其内部值。
//!    - [`set`](Cell::set):替换内部值,并丢弃(drop)被替换出来的旧值。
//!
//! `Cell<T>` 通常用于较简单、复制或移动开销不大的类型(例如各种数字),在可行时一般应优先于
//! 其他 cell 类型选用它。对于较大、不可 `Copy` 的类型,`RefCell` 则有它的优势。
//!
//! ## `RefCell<T>`
//!
//! [`RefCell<T>`] 借助 Rust 的生命周期实现“动态借用”,使调用方能够临时地、独占地、可变地访问
//! 内部值。其实质是把借用检查**从编译期推迟到运行期**:不同于 Rust 原生引用那样完全在编译期
//! 静态地完成检查,`RefCell<T>` 的借用是在**运行时**被跟踪的。
//!
//! 通过 [`borrow`](`RefCell::borrow`) 可获得指向内部值的不可变引用(`&T`);通过
//! [`borrow_mut`](`RefCell::borrow_mut`) 可获得可变借用(`&mut T`)。这些函数被调用时,会先
//! 用一个借用计数来检查是否满足 Rust 的借用规则:任意数量的不可变借用是允许的,或者单独一个
//! 可变借用是允许的,但二者绝不能同时存在。如果尝试的借用会违反这些规则,线程就会 **panic**。
//! 具体地说:当已经存在某个可变借用时再调用 `borrow`,或者当已经存在任意借用时再调用
//! `borrow_mut`,都会 panic。若不希望 panic,可改用返回 `Result` 的 `try_borrow`/`try_borrow_mut`。
//!
//! `RefCell<T>` 对应的 [`Sync`] 线程安全版本是 [`RwLock<T>`]。
//!
//! ## `OnceCell<T>`
//!
//! [`OnceCell<T>`] 在某种意义上是 `Cell` 与 `RefCell` 的混合体,适用于通常只需写入一次的值。
//! 这意味着可以在不移动、不复制内部值的情况下取得引用 `&T`(不同于 `Cell`),同时又不需要
//! 运行时检查(不同于 `RefCell`)。但代价是:一旦设置完成,除非你持有指向 `OnceCell` 本身的
//! 可变引用,否则无法再更新它的值。
//!
//! `OnceCell` 提供以下方法:
//!
//! - [`get`](OnceCell::get):取得指向内部值的引用
//! - [`set`](OnceCell::set):若内部值尚未设置则设置它(返回 `Result`)
//! - [`get_or_init`](OnceCell::get_or_init):返回内部值,必要时先初始化
//! - [`get_mut`](OnceCell::get_mut):提供指向内部值的可变引用,仅当你持有指向 cell 本身的
//!   可变引用时才可用。
//!
//! `OnceCell<T>` 对应的 [`Sync`] 线程安全版本是 [`OnceLock<T>`]。
//!
//! ## `LazyCell<T, F>`
//!
//! 使用 OnceCell 时有一种常见模式:对某个给定的 OnceCell,每次调用 [`OnceCell::get_or_init`]
//! 时都传入同一个函数。[`LazyCell`] 正是为此而生,它把存放 `T` 的 cell 与一个 `F` 类型的函数
//! 配对,并在交出 `&T` 之前总是先调用 `F`。这一切发生得很隐式——只需尝试解引用 LazyCell 去取
//! 其内容即可触发,因此对于由常量初始化的场所,它的用法要透明得多。
//!
//! 不符合这一描述的更复杂模式,则可以改用 `OnceCell<T>` 来构建。
//!
//! `LazyCell` 的工作方式是提供一个会调用该函数的 `impl Deref`,因此你可以直接通过解引用来使用它
//! (例如 `*lazy_cell` 或 `lazy_cell.deref()`)。
//! 初始化过程只能成功一次。若初始化闭包 panic,`LazyCell` 会进入毒化状态;若初始化逻辑在尚未完成时
//! 重入访问同一个 cell,也会触发对应的重入保护或 panic,因为这会破坏“一次初始化后再交出稳定引用”
//! 的核心不变量。
//!
//! `LazyCell<T, F>` 对应的 [`Sync`] 线程安全版本是 [`LazyLock<T, F>`]。
//!
//! # 何时选择内部可变性
//!
//! 更常见的继承可变性——即必须独占访问才能修改一个值——是使 Rust 能够强有力地推理指针别名、
//! 在编译期静态地防止崩溃类 bug 的关键语言要素之一。正因如此,继承可变性是首选,而内部可变性
//! 多少属于一种“最后手段”。不过,既然 cell 类型能在原本不允许修改之处实现修改,那么有些场合
//! 使用内部可变性可能是恰当的,甚至是*必须*的,例如:
//!
//! * 在某个不可变事物的“内部”引入可变性
//! * 逻辑上不可变的方法的实现细节
//! * 在 [`Clone`] 的实现中进行修改
//!
//! ## 在某个不可变事物的“内部”引入可变性
//!
//! 许多共享智能指针类型,包括 [`Rc<T>`] 和 [`Arc<T>`],都提供了可被克隆并在多方之间共享的容器。
//! 由于其中所含的值可能被多重别名,它们只能用 `&` 借出,而不能用 `&mut`。没有 cell 的话,
//! 根本无法修改这些智能指针内部的数据。
//!
//! 因此一种非常常见的做法,是在共享指针类型内部放一个 `RefCell<T>`,以重新引入可变性:
//!
//! ```
//! use std::cell::{RefCell, RefMut};
//! use std::collections::HashMap;
//! use std::rc::Rc;
//!
//! fn main() {
//!     let shared_map: Rc<RefCell<_>> = Rc::new(RefCell::new(HashMap::new()));
//!     // 新开一个代码块,以限制这次动态借用的作用域
//!     {
//!         let mut map: RefMut<'_, _> = shared_map.borrow_mut();
//!         map.insert("africa", 92388);
//!         map.insert("kyoto", 11837);
//!         map.insert("piccadilly", 11826);
//!         map.insert("marbles", 38);
//!     }
//!
//!     // 注意:如果上面对缓存的借用没有离开作用域,下面这次借用就会引发一次动态的线程 panic。
//!     // 这正是使用 `RefCell` 的主要风险所在。
//!     let total: i32 = shared_map.borrow().values().sum();
//!     println!("{total}");
//! }
//! ```
//!
//! 注意这个例子用的是 `Rc<T>` 而非 `Arc<T>`。`RefCell<T>` 是面向单线程场景的。如果你需要在
//! 多线程环境下共享可变性,请考虑使用 [`RwLock<T>`] 或 [`Mutex<T>`]。
//!
//! ## 逻辑上不可变的方法的实现细节
//!
//! 偶尔我们可能不希望在 API 中暴露出“幕后正在发生修改”这一事实。这或许是因为该操作在逻辑上
//! 是不可变的,但例如出于缓存的需要,其实现不得不进行修改;又或许是因为你必须借助修改,才能
//! 实现某个最初被定义为接收 `&self` 的 trait 方法。
//!
//! ```
//! # #![allow(dead_code)]
//! use std::cell::OnceCell;
//!
//! struct Graph {
//!     edges: Vec<(i32, i32)>,
//!     span_tree_cache: OnceCell<Vec<(i32, i32)>>
//! }
//!
//! impl Graph {
//!     fn minimum_spanning_tree(&self) -> Vec<(i32, i32)> {
//!         self.span_tree_cache
//!             .get_or_init(|| self.calc_span_tree())
//!             .clone()
//!     }
//!
//!     fn calc_span_tree(&self) -> Vec<(i32, i32)> {
//!         // 这里进行开销很大的计算
//!         vec![]
//!     }
//! }
//! ```
//!
//! ## 在 `Clone` 的实现中进行修改
//!
//! 这其实只是上一种情形的一个特例(但相当常见):为表面上不可变的操作隐藏其修改行为。
//! [`clone`](Clone::clone) 方法被期望不改变源值,且被声明为接收 `&self` 而非 `&mut self`。
//! 因此,`clone` 方法中发生的任何修改都必须借助 cell 类型来完成。例如,[`Rc<T>`] 就是把它的
//! 引用计数维护在一个 `Cell<T>` 之中。
//!
//! ```
//! use std::cell::Cell;
//! use std::ptr::NonNull;
//! use std::process::abort;
//! use std::marker::PhantomData;
//!
//! struct Rc<T: ?Sized> {
//!     ptr: NonNull<RcInner<T>>,
//!     phantom: PhantomData<RcInner<T>>,
//! }
//!
//! struct RcInner<T: ?Sized> {
//!     strong: Cell<usize>,
//!     refcount: Cell<usize>,
//!     value: T,
//! }
//!
//! impl<T: ?Sized> Clone for Rc<T> {
//!     fn clone(&self) -> Rc<T> {
//!         self.inc_strong();
//!         Rc {
//!             ptr: self.ptr,
//!             phantom: PhantomData,
//!         }
//!     }
//! }
//!
//! trait RcInnerPtr<T: ?Sized> {
//!
//!     fn inner(&self) -> &RcInner<T>;
//!
//!     fn strong(&self) -> usize {
//!         self.inner().strong.get()
//!     }
//!
//!     fn inc_strong(&self) {
//!         self.inner()
//!             .strong
//!             .set(self.strong()
//!                      .checked_add(1)
//!                      .unwrap_or_else(|| abort() ));
//!     }
//! }
//!
//! impl<T: ?Sized> RcInnerPtr<T> for Rc<T> {
//!    fn inner(&self) -> &RcInner<T> {
//!        unsafe {
//!            self.ptr.as_ref()
//!        }
//!    }
//! }
//! ```
//!
//! [`Arc<T>`]: ../../std/sync/struct.Arc.html
//! [`Rc<T>`]: ../../std/rc/struct.Rc.html
//! [`RwLock<T>`]: ../../std/sync/struct.RwLock.html
//! [`Mutex<T>`]: ../../std/sync/struct.Mutex.html
//! [`OnceLock<T>`]: ../../std/sync/struct.OnceLock.html
//! [`LazyLock<T, F>`]: ../../std/sync/struct.LazyLock.html
//! [`Sync`]: ../../std/marker/trait.Sync.html
//! [`atomic`]: crate::sync::atomic

#![stable(feature = "rust1", since = "1.0.0")]

use crate::cmp::Ordering;
use crate::fmt::{self, Debug, Display};
use crate::marker::{Destruct, PhantomData, Unsize};
use crate::mem::{self, ManuallyDrop};
use crate::ops::{self, CoerceUnsized, Deref, DerefMut, DerefPure, DispatchFromDyn};
use crate::panic::const_panic;
use crate::pin::PinCoerceUnsized;
use crate::ptr::{self, NonNull};
use crate::range;

mod lazy;
mod once;

#[stable(feature = "lazy_cell", since = "1.80.0")]
pub use lazy::LazyCell;
#[stable(feature = "once_cell", since = "1.70.0")]
pub use once::OnceCell;

/// 一处可变的内存位置。
///
/// # 内存布局 {#memory-layout}
///
/// `Cell<T>` 拥有[与 `UnsafeCell<T>` 相同的内存布局及注意事项](UnsafeCell#memory-layout)。
/// 特别地,这意味着 `Cell<T>` 在内存中的表示与其内部类型 `T` 完全相同。
///
/// # 示例
///
/// 在这个例子中,你可以看到 `Cell<T>` 是如何让一个不可变结构体内部得以被修改的。换言之,
/// 它实现了“内部可变性”。
///
/// ```
/// use std::cell::Cell;
///
/// struct SomeStruct {
///     regular_field: u8,
///     special_field: Cell<u8>,
/// }
///
/// let my_struct = SomeStruct {
///     regular_field: 0,
///     special_field: Cell::new(1),
/// };
///
/// let new_value = 100;
///
/// // 错误:`my_struct` 是不可变的
/// // my_struct.regular_field = new_value;
///
/// // 可行:尽管 `my_struct` 不可变,但 `special_field` 是一个 `Cell`,
/// // 它始终可以被修改
/// my_struct.special_field.set(new_value);
/// assert_eq!(my_struct.special_field.get(), new_value);
/// ```
///
/// 更多内容参见[模块级文档](self)。
#[rustc_diagnostic_item = "Cell"]
#[stable(feature = "rust1", since = "1.0.0")]
#[repr(transparent)]
#[rustc_pub_transparent]
pub struct Cell<T: ?Sized> {
    value: UnsafeCell<T>,
}

#[stable(feature = "rust1", since = "1.0.0")]
unsafe impl<T: ?Sized> Send for Cell<T> where T: Send {}

// 注意:从正确性角度讲这个负向 impl 并非严格必需,因为 `Cell` 包裹的是
// `UnsafeCell`,而后者本身就是 `!Sync`。
// 不过,鉴于 `Cell` 的 `!Sync` 这一性质极其重要,显式写出一个负向 impl 既有利于文档表达,
// 也能让编译器给出更友好的错误信息。
#[stable(feature = "rust1", since = "1.0.0")]
impl<T: ?Sized> !Sync for Cell<T> {}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T: Copy> Clone for Cell<T> {
    #[inline]
    fn clone(&self) -> Cell<T> {
        Cell::new(self.get())
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_unstable(feature = "const_default", issue = "143894")]
impl<T: [const] Default> const Default for Cell<T> {
    /// 创建一个 `Cell<T>`,其值为 T 的 `Default` 默认值。
    #[inline]
    fn default() -> Cell<T> {
        Cell::new(Default::default())
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T: PartialEq + Copy> PartialEq for Cell<T> {
    #[inline]
    fn eq(&self, other: &Cell<T>) -> bool {
        self.get() == other.get()
    }
}

#[stable(feature = "cell_eq", since = "1.2.0")]
impl<T: Eq + Copy> Eq for Cell<T> {}

#[stable(feature = "cell_ord", since = "1.10.0")]
impl<T: PartialOrd + Copy> PartialOrd for Cell<T> {
    #[inline]
    fn partial_cmp(&self, other: &Cell<T>) -> Option<Ordering> {
        self.get().partial_cmp(&other.get())
    }

    #[inline]
    fn lt(&self, other: &Cell<T>) -> bool {
        self.get() < other.get()
    }

    #[inline]
    fn le(&self, other: &Cell<T>) -> bool {
        self.get() <= other.get()
    }

    #[inline]
    fn gt(&self, other: &Cell<T>) -> bool {
        self.get() > other.get()
    }

    #[inline]
    fn ge(&self, other: &Cell<T>) -> bool {
        self.get() >= other.get()
    }
}

#[stable(feature = "cell_ord", since = "1.10.0")]
impl<T: Ord + Copy> Ord for Cell<T> {
    #[inline]
    fn cmp(&self, other: &Cell<T>) -> Ordering {
        self.get().cmp(&other.get())
    }
}

#[stable(feature = "cell_from", since = "1.12.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl<T> const From<T> for Cell<T> {
    /// 创建一个内含给定值的新 `Cell<T>`。
    fn from(t: T) -> Cell<T> {
        Cell::new(t)
    }
}

impl<T> Cell<T> {
    /// 创建一个内含给定值的新 `Cell`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::cell::Cell;
    ///
    /// let c = Cell::new(5);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_cell_new", since = "1.24.0")]
    #[inline]
    pub const fn new(value: T) -> Cell<T> {
        Cell { value: UnsafeCell::new(value) }
    }

    /// 设置(替换)内部所含的值。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::cell::Cell;
    ///
    /// let c = Cell::new(5);
    ///
    /// c.set(10);
    /// ```
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_unstable(feature = "const_cell_traits", issue = "147787")]
    #[rustc_should_not_be_called_on_const_items]
    pub const fn set(&self, val: T)
    where
        T: [const] Destruct,
    {
        self.replace(val);
    }

    /// 交换两个 `Cell` 各自所含的值。
    ///
    /// 它与 `std::mem::swap` 的区别在于:本函数不要求 `&mut` 引用。
    ///
    /// # Panics
    ///
    /// 如果 `self` 与 `other` 是两个不同、却又部分重叠的 `Cell`,本函数会 panic。
    /// (仅用标准库的方法不可能造出这种部分重叠的 `Cell`;但 unsafe 代码可以,例如造出两个
    /// 部分重叠的 `&Cell<[i32; 2]>`。)
    ///
    /// # 示例
    ///
    /// ```
    /// use std::cell::Cell;
    ///
    /// let c1 = Cell::new(5i32);
    /// let c2 = Cell::new(10i32);
    /// c1.swap(&c2);
    /// assert_eq!(10, c1.get());
    /// assert_eq!(5, c2.get());
    /// ```
    #[inline]
    #[stable(feature = "move_cell", since = "1.17.0")]
    #[rustc_should_not_be_called_on_const_items]
    pub fn swap(&self, other: &Self) {
        // 本函数明确声明它*会*在重叠时 panic,而 intrinsics::is_nonoverlapping 在 const 语境下
        // 不会做该检查,所以在这里使用它只会平添不必要的脆弱性。
        fn is_nonoverlapping<T>(src: *const T, dst: *const T) -> bool {
            let src_usize = src.addr();
            let dst_usize = dst.addr();
            let diff = src_usize.abs_diff(dst_usize);
            diff >= size_of::<T>()
        }

        if ptr::eq(self, other) {
            // 自己和自己交换,什么都不会改变。
            return;
        }
        if !is_nonoverlapping(self, other) {
            // 关于为何必须在此处停下,参见 <https://github.com/rust-lang/rust/issues/80778>。
            panic!("`Cell::swap` on overlapping non-identical `Cell`s");
        }
        // SAFETY:若从不同线程调用,此操作可能有风险;但 `Cell` 是 `!Sync` 的,故不会发生跨线程
        // 调用。它也不会让任何指针失效,因为 `Cell` 确保不会有其他东西指向这两个 `Cell` 之一的
        // 内部。我们还排除了诸如部分重叠的 `Cell` 这类捣乱情形,所以 `swap` 只会正确地把两个完整
        // 的 `T` 类型的值来回拷贝。
        unsafe {
            mem::swap(&mut *self.value.get(), &mut *other.value.get());
        }
    }

    /// 用 `val` 替换内部所含的值,并返回被替换出来的旧值。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::cell::Cell;
    ///
    /// let cell = Cell::new(5);
    /// assert_eq!(cell.get(), 5);
    /// assert_eq!(cell.replace(10), 5);
    /// assert_eq!(cell.get(), 10);
    /// ```
    #[inline]
    #[stable(feature = "move_cell", since = "1.17.0")]
    #[rustc_const_stable(feature = "const_cell", since = "1.88.0")]
    #[rustc_confusables("swap")]
    #[rustc_should_not_be_called_on_const_items]
    pub const fn replace(&self, val: T) -> T {
        // SAFETY:若从另一线程调用,此操作可能引发数据竞争;但 `Cell` 是 `!Sync` 的,故不会发生。
        mem::replace(unsafe { &mut *self.value.get() }, val)
    }

    /// 取出该值,同时消耗掉整个 cell。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::cell::Cell;
    ///
    /// let c = Cell::new(5);
    /// let five = c.into_inner();
    ///
    /// assert_eq!(five, 5);
    /// ```
    #[stable(feature = "move_cell", since = "1.17.0")]
    #[rustc_const_stable(feature = "const_cell_into_inner", since = "1.83.0")]
    #[rustc_allow_const_fn_unstable(const_precise_live_drops)]
    pub const fn into_inner(self) -> T {
        self.value.into_inner()
    }
}

impl<T: Copy> Cell<T> {
    /// 返回内部所含值的一份拷贝。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::cell::Cell;
    ///
    /// let c = Cell::new(5);
    ///
    /// let five = c.get();
    /// ```
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_cell", since = "1.88.0")]
    #[rustc_should_not_be_called_on_const_items]
    pub const fn get(&self) -> T {
        // SAFETY:若从另一线程调用,此操作可能引发数据竞争;但 `Cell` 是 `!Sync` 的,故不会发生。
        unsafe { *self.value.get() }
    }

    /// 用一个函数更新内部所含的值。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::cell::Cell;
    ///
    /// let c = Cell::new(5);
    /// c.update(|x| x + 1);
    /// assert_eq!(c.get(), 6);
    /// ```
    #[inline]
    #[stable(feature = "cell_update", since = "1.88.0")]
    #[rustc_const_unstable(feature = "const_cell_traits", issue = "147787")]
    #[rustc_should_not_be_called_on_const_items]
    pub const fn update(&self, f: impl [const] FnOnce(T) -> T)
    where
        // FIXME(const-hack):`Copy` 本应蕴含 `const Destruct`
        T: [const] Destruct,
    {
        let old = self.get();
        self.set(f(old));
    }
}

impl<T: ?Sized> Cell<T> {
    /// 返回一个指向该 cell 内部数据的裸指针。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::cell::Cell;
    ///
    /// let c = Cell::new(5);
    ///
    /// let ptr = c.as_ptr();
    /// ```
    #[inline]
    #[stable(feature = "cell_as_ptr", since = "1.12.0")]
    #[rustc_const_stable(feature = "const_cell_as_ptr", since = "1.32.0")]
    #[rustc_as_ptr]
    #[rustc_never_returns_null_ptr]
    pub const fn as_ptr(&self) -> *mut T {
        self.value.get()
    }

    /// 返回一个指向内部数据的可变引用。
    ///
    /// 本调用以可变方式借用 `Cell`(在编译期完成),从而保证我们持有的是唯一的那一个引用。
    ///
    /// 但要小心:本方法要求 `self` 是可变的,而使用 `Cell` 时通常并非如此。如果你需要的是
    /// “通过引用进行内部可变”,请考虑使用 `RefCell`,它的 [`borrow_mut`] 方法提供了经运行时
    /// 检查的可变借用。
    ///
    /// [`borrow_mut`]: RefCell::borrow_mut()
    ///
    /// # 示例
    ///
    /// ```
    /// use std::cell::Cell;
    ///
    /// let mut c = Cell::new(5);
    /// *c.get_mut() += 1;
    ///
    /// assert_eq!(c.get(), 6);
    /// ```
    #[inline]
    #[stable(feature = "cell_get_mut", since = "1.11.0")]
    #[rustc_const_stable(feature = "const_cell", since = "1.88.0")]
    pub const fn get_mut(&mut self) -> &mut T {
        self.value.get_mut()
    }

    /// 由一个 `&mut T` 得到一个 `&Cell<T>`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::cell::Cell;
    ///
    /// let slice: &mut [i32] = &mut [1, 2, 3];
    /// let cell_slice: &Cell<[i32]> = Cell::from_mut(slice);
    /// let slice_cell: &[Cell<i32>] = cell_slice.as_slice_of_cells();
    ///
    /// assert_eq!(slice_cell.len(), 3);
    /// ```
    #[inline]
    #[stable(feature = "as_cell", since = "1.37.0")]
    #[rustc_const_stable(feature = "const_cell", since = "1.88.0")]
    pub const fn from_mut(t: &mut T) -> &Cell<T> {
        // SAFETY:`&mut` 保证了独占访问。
        unsafe { &*(t as *mut T as *const Cell<T>) }
    }
}

impl<T: Default> Cell<T> {
    /// 取出该 cell 的值,并在原处留下 `Default::default()`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::cell::Cell;
    ///
    /// let c = Cell::new(5);
    /// let five = c.take();
    ///
    /// assert_eq!(five, 5);
    /// assert_eq!(c.into_inner(), 0);
    /// ```
    #[stable(feature = "move_cell", since = "1.17.0")]
    #[rustc_const_unstable(feature = "const_cell_traits", issue = "147787")]
    pub const fn take(&self) -> T
    where
        T: [const] Default,
    {
        self.replace(Default::default())
    }
}

#[unstable(feature = "coerce_unsized", issue = "18598")]
impl<T: CoerceUnsized<U>, U> CoerceUnsized<Cell<U>> for Cell<T> {}

// 允许那些包裹了 `Cell` 的类型也实现 `DispatchFromDyn`,从而成为可用于动态分发的方法接收者。
// 注意:目前 `Cell` 本身还不能作为方法接收者,因为它没有实现 Deref。
// 换言之:
// `self: Cell<&Self>` 不能工作
// 而 `self: CellWrapper<Self>` 则成为可能
#[unstable(feature = "dispatch_from_dyn", issue = "none")]
impl<T: DispatchFromDyn<U>, U> DispatchFromDyn<Cell<U>> for Cell<T> {}

impl<T> Cell<[T]> {
    /// 由一个 `&Cell<[T]>` 得到一个 `&[Cell<T>]`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::cell::Cell;
    ///
    /// let slice: &mut [i32] = &mut [1, 2, 3];
    /// let cell_slice: &Cell<[i32]> = Cell::from_mut(slice);
    /// let slice_cell: &[Cell<i32>] = cell_slice.as_slice_of_cells();
    ///
    /// assert_eq!(slice_cell.len(), 3);
    /// ```
    #[stable(feature = "as_cell", since = "1.37.0")]
    #[rustc_const_stable(feature = "const_cell", since = "1.88.0")]
    pub const fn as_slice_of_cells(&self) -> &[Cell<T>] {
        // SAFETY:`Cell<T>` 与 `T` 拥有相同的内存布局。
        unsafe { &*(self as *const Cell<[T]> as *const [Cell<T>]) }
    }
}

impl<T, const N: usize> Cell<[T; N]> {
    /// 由一个 `&Cell<[T; N]>` 得到一个 `&[Cell<T>; N]`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::cell::Cell;
    ///
    /// let mut array: [i32; 3] = [1, 2, 3];
    /// let cell_array: &Cell<[i32; 3]> = Cell::from_mut(&mut array);
    /// let array_cell: &[Cell<i32>; 3] = cell_array.as_array_of_cells();
    /// ```
    #[stable(feature = "as_array_of_cells", since = "1.91.0")]
    #[rustc_const_stable(feature = "as_array_of_cells", since = "1.91.0")]
    pub const fn as_array_of_cells(&self) -> &[Cell<T>; N] {
        // SAFETY:`Cell<T>` 与 `T` 拥有相同的内存布局。
        unsafe { &*(self as *const Cell<[T; N]> as *const [Cell<T>; N]) }
    }
}

/// 标记那些“对 `Cell<Self>` 进行克隆是健全的”类型。
///
/// # 安全性(Safety）
///
/// 为某个类型实现本 trait 是健全的,当且仅当下面这段代码在 T = 该类型时是健全的。
///
/// ```
/// #![feature(cell_get_cloned)]
/// # use std::cell::{CloneFromCell, Cell};
/// fn clone_from_cell<T: CloneFromCell>(cell: &Cell<T>) -> T {
///     unsafe { T::clone(&*cell.as_ptr()) }
/// }
/// ```
///
/// 重要的是,你不能随便给任意 `Copy` 类型都实现 `CloneFromCell`。例如下面这种就是不健全的:
///
/// ```rust
/// #![feature(cell_get_cloned)]
/// # use std::cell::Cell;
///
/// #[derive(Copy, Debug)]
/// pub struct Bad<'a>(Option<&'a Cell<Bad<'a>>>, u8);
///
/// impl Clone for Bad<'_> {
///     fn clone(&self) -> Self {
///         let a: &u8 = &self.1;
///         // 当 self.0 指向 self 自身时,我们在仍持有一个指向 self.1 的存活 `&u8` 的情况下
///         // 又向 self.1 写入 —— 这是 UB
///         self.0.unwrap().set(Self(None, 1));
///         dbg!((a, self));
///         Self(None, 0)
///     }
/// }
///
/// // 这是不健全的
/// // unsafe impl CloneFromCell for Bad<'_> {}
/// ```
#[unstable(feature = "cell_get_cloned", issue = "145329")]
// 允许用户代码中可能出现的重叠实现。
#[marker]
pub unsafe trait CloneFromCell: Clone {}

// `CloneFromCell` 可以为以下这类类型实现:它们不含间接引用,并且其 `Clone`
// 实现中不会去访问任何 `Cell`。这里覆盖了一个常用的子集。
#[unstable(feature = "cell_get_cloned", issue = "145329")]
unsafe impl<T: CloneFromCell, const N: usize> CloneFromCell for [T; N] {}
#[unstable(feature = "cell_get_cloned", issue = "145329")]
unsafe impl<T: CloneFromCell> CloneFromCell for Option<T> {}
#[unstable(feature = "cell_get_cloned", issue = "145329")]
unsafe impl<T: CloneFromCell, E: CloneFromCell> CloneFromCell for Result<T, E> {}
#[unstable(feature = "cell_get_cloned", issue = "145329")]
unsafe impl<T: ?Sized> CloneFromCell for PhantomData<T> {}
#[unstable(feature = "cell_get_cloned", issue = "145329")]
unsafe impl<T: CloneFromCell> CloneFromCell for ManuallyDrop<T> {}
#[unstable(feature = "cell_get_cloned", issue = "145329")]
unsafe impl<T: CloneFromCell> CloneFromCell for ops::Range<T> {}
#[unstable(feature = "cell_get_cloned", issue = "145329")]
unsafe impl<T: CloneFromCell> CloneFromCell for range::Range<T> {}

#[unstable(feature = "cell_get_cloned", issue = "145329")]
impl<T: CloneFromCell> Cell<T> {
    /// 获取该 `Cell` 的一份克隆,其中含有原值的一份拷贝。
    ///
    /// 这使得像 `Rc` 这样克隆开销低廉的类型可以被存放在 `Cell` 中,从而对外暴露其更廉价的
    /// `clone()` 方法。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(cell_get_cloned)]
    ///
    /// use core::cell::Cell;
    /// use std::rc::Rc;
    ///
    /// let rc = Rc::new(1usize);
    /// let c1 = Cell::new(rc);
    /// let c2 = c1.get_cloned();
    /// assert_eq!(*c2.into_inner(), 1);
    /// ```
    pub fn get_cloned(&self) -> Self {
        // SAFETY:T 实现了 CloneFromCell,该 trait 保证了此操作是健全的。
        Cell::new(T::clone(unsafe { &*self.as_ptr() }))
    }
}

/// 一处可变的内存位置,带有动态检查的借用规则。
///
/// 更多内容参见[模块级文档](self)。
#[rustc_diagnostic_item = "RefCell"]
#[stable(feature = "rust1", since = "1.0.0")]
pub struct RefCell<T: ?Sized> {
    borrow: Cell<BorrowCounter>,
    // 存放当前最早一次仍处于活动状态的借用所在的位置。
    // 每当借用数从零变为单个借用时,这个字段就会被更新。当发生借用冲突时,该位置会被写入
    // 生成的 `BorrowError`/`BorrowMutError` 之中。
    #[cfg(feature = "debug_refcell")]
    borrowed_at: Cell<Option<&'static crate::panic::Location<'static>>>,
    value: UnsafeCell<T>,
}

/// 由 [`RefCell::try_borrow`] 返回的错误。
#[stable(feature = "try_borrow", since = "1.13.0")]
#[non_exhaustive]
#[derive(Debug)]
pub struct BorrowError {
    #[cfg(feature = "debug_refcell")]
    location: &'static crate::panic::Location<'static>,
}

#[stable(feature = "try_borrow", since = "1.13.0")]
impl Display for BorrowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        #[cfg(feature = "debug_refcell")]
        let res = write!(
            f,
            "RefCell already mutably borrowed; a previous borrow was at {}",
            self.location
        );

        #[cfg(not(feature = "debug_refcell"))]
        let res = Display::fmt("RefCell already mutably borrowed", f);

        res
    }
}

/// 由 [`RefCell::try_borrow_mut`] 返回的错误。
#[stable(feature = "try_borrow", since = "1.13.0")]
#[non_exhaustive]
#[derive(Debug)]
pub struct BorrowMutError {
    #[cfg(feature = "debug_refcell")]
    location: &'static crate::panic::Location<'static>,
}

#[stable(feature = "try_borrow", since = "1.13.0")]
impl Display for BorrowMutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        #[cfg(feature = "debug_refcell")]
        let res = write!(f, "RefCell already borrowed; a previous borrow was at {}", self.location);

        #[cfg(not(feature = "debug_refcell"))]
        let res = Display::fmt("RefCell already borrowed", f);

        res
    }
}

// 这确保了 panic 代码会被从 `RefCell` 的 `borrow_mut` 热路径中外联出去。
#[cfg_attr(not(panic = "immediate-abort"), inline(never))]
#[track_caller]
#[cold]
const fn panic_already_borrowed(err: BorrowMutError) -> ! {
    const_panic!(
        "RefCell already borrowed",
        "{err}",
        err: BorrowMutError = err,
    )
}

// 这确保了 panic 代码会被从 `RefCell` 的 `borrow` 热路径中外联出去。
#[cfg_attr(not(panic = "immediate-abort"), inline(never))]
#[track_caller]
#[cold]
const fn panic_already_mutably_borrowed(err: BorrowError) -> ! {
    const_panic!(
        "RefCell already mutably borrowed",
        "{err}",
        err: BorrowError = err,
    )
}

// 正值表示当前活动的 `Ref`(共享借用)的数量。负值表示当前活动的 `RefMut`(可变借用)的
// 数量。只有当多个 `RefMut` 指向同一 `RefCell` 中彼此互不重叠的不同部分(例如一个切片的
// 不同区段)时,它们才可以同时活动。
//
// `Ref` 和 `RefMut` 各自的大小都是两个机器字,因此现实中几乎不可能存在多到足以让计数溢出
// `usize` 范围一半的 `Ref` 或 `RefMut`。于是 `BorrowCounter` 大概率永远不会上溢或下溢。
// 然而这并非保证:一个病态的程序可以反复地创建并随后 mem::forget 掉 `Ref` 或 `RefMut`。
// 因此,所有代码都必须显式地检查上溢与下溢以避免不安全,或者至少在溢出真的发生时仍能表现
// 正确(参见 BorrowRef::new)。
type BorrowCounter = isize;
const UNUSED: BorrowCounter = 0;

#[inline(always)]
const fn is_writing(x: BorrowCounter) -> bool {
    x < UNUSED
}

#[inline(always)]
const fn is_reading(x: BorrowCounter) -> bool {
    x > UNUSED
}

impl<T> RefCell<T> {
    /// 创建一个内含 `value` 的新 `RefCell`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::cell::RefCell;
    ///
    /// let c = RefCell::new(5);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_refcell_new", since = "1.24.0")]
    #[inline]
    pub const fn new(value: T) -> RefCell<T> {
        RefCell {
            value: UnsafeCell::new(value),
            borrow: Cell::new(UNUSED),
            #[cfg(feature = "debug_refcell")]
            borrowed_at: Cell::new(None),
        }
    }

    /// 消耗该 `RefCell`,返回其所包裹的值。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::cell::RefCell;
    ///
    /// let c = RefCell::new(5);
    ///
    /// let five = c.into_inner();
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_cell_into_inner", since = "1.83.0")]
    #[rustc_allow_const_fn_unstable(const_precise_live_drops)]
    #[inline]
    pub const fn into_inner(self) -> T {
        // 由于本函数按值接收 `self`(即整个 `RefCell`),编译器会静态地验证它当前未被借用。
        self.value.into_inner()
    }

    /// 用一个新值替换所包裹的值并返回旧值,过程中不会提前析构新值或旧值。
    ///
    /// 本函数对应 [`std::mem::replace`](../mem/fn.replace.html)。
    ///
    /// # Panics
    ///
    /// 如果该值当前正被借用,则 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::cell::RefCell;
    /// let cell = RefCell::new(5);
    /// let old_value = cell.replace(6);
    /// assert_eq!(old_value, 5);
    /// assert_eq!(cell, RefCell::new(6));
    /// ```
    #[inline]
    #[stable(feature = "refcell_replace", since = "1.24.0")]
    #[track_caller]
    #[rustc_confusables("swap")]
    #[rustc_const_unstable(feature = "const_ref_cell", issue = "137844")]
    #[rustc_should_not_be_called_on_const_items]
    pub const fn replace(&self, t: T) -> T {
        mem::replace(&mut self.borrow_mut(), t)
    }

    /// 用一个由 `f` 计算出的新值替换所包裹的值并返回旧值,过程中不会对二者中的任何一个
    /// 提前执行析构。
    ///
    /// # Panics
    ///
    /// 如果该值当前正被借用,则 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::cell::RefCell;
    /// let cell = RefCell::new(5);
    /// let old_value = cell.replace_with(|&mut old| old + 1);
    /// assert_eq!(old_value, 5);
    /// assert_eq!(cell, RefCell::new(6));
    /// ```
    #[inline]
    #[stable(feature = "refcell_replace_swap", since = "1.35.0")]
    #[track_caller]
    #[rustc_should_not_be_called_on_const_items]
    pub fn replace_with<F: FnOnce(&mut T) -> T>(&self, f: F) -> T {
        let mut_borrow = &mut *self.borrow_mut();
        let replacement = f(mut_borrow);
        mem::replace(mut_borrow, replacement)
    }

    /// 将 `self` 所包裹的值与 `other` 所包裹的值互换,过程中不会对二者中的任何一个
    /// 提前执行析构。
    ///
    /// 本函数对应 [`std::mem::swap`](../mem/fn.swap.html)。
    ///
    /// # Panics
    ///
    /// 如果两个 `RefCell` 中任意一个的值当前正被借用,或者 `self` 与 `other` 指向同一个
    /// `RefCell`,则 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::cell::RefCell;
    /// let c = RefCell::new(5);
    /// let d = RefCell::new(6);
    /// c.swap(&d);
    /// assert_eq!(c, RefCell::new(6));
    /// assert_eq!(d, RefCell::new(5));
    /// ```
    #[inline]
    #[stable(feature = "refcell_swap", since = "1.24.0")]
    #[rustc_const_unstable(feature = "const_ref_cell", issue = "137844")]
    #[rustc_should_not_be_called_on_const_items]
    pub const fn swap(&self, other: &Self) {
        mem::swap(&mut *self.borrow_mut(), &mut *other.borrow_mut())
    }
}

impl<T: ?Sized> RefCell<T> {
    /// 不可变地借用所包裹的值。
    ///
    /// 该借用一直持续到返回的 `Ref` 离开作用域为止。可以同时取得多个不可变借用。
    ///
    /// # Panics
    ///
    /// 如果该值当前正被可变借用,则 panic。若需要不会 panic 的变体,请使用
    /// [`try_borrow`](#method.try_borrow)。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::cell::RefCell;
    ///
    /// let c = RefCell::new(5);
    ///
    /// let borrowed_five = c.borrow();
    /// let borrowed_five2 = c.borrow();
    /// ```
    ///
    /// 一个会 panic 的例子:
    ///
    /// ```should_panic
    /// use std::cell::RefCell;
    ///
    /// let c = RefCell::new(5);
    ///
    /// let m = c.borrow_mut();
    /// let b = c.borrow(); // 这里会引发 panic
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    #[track_caller]
    #[rustc_const_unstable(feature = "const_ref_cell", issue = "137844")]
    #[rustc_should_not_be_called_on_const_items]
    pub const fn borrow(&self) -> Ref<'_, T> {
        match self.try_borrow() {
            Ok(b) => b,
            Err(err) => panic_already_mutably_borrowed(err),
        }
    }

    /// 不可变地借用所包裹的值;如果该值当前正被可变借用,则返回一个错误。
    ///
    /// 该借用一直持续到返回的 `Ref` 离开作用域为止。可以同时取得多个不可变借用。
    ///
    /// 这是 [`borrow`](#method.borrow) 不会 panic 的变体。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::cell::RefCell;
    ///
    /// let c = RefCell::new(5);
    ///
    /// {
    ///     let m = c.borrow_mut();
    ///     assert!(c.try_borrow().is_err());
    /// }
    ///
    /// {
    ///     let m = c.borrow();
    ///     assert!(c.try_borrow().is_ok());
    /// }
    /// ```
    #[stable(feature = "try_borrow", since = "1.13.0")]
    #[inline]
    #[cfg_attr(feature = "debug_refcell", track_caller)]
    #[rustc_const_unstable(feature = "const_ref_cell", issue = "137844")]
    #[rustc_should_not_be_called_on_const_items]
    pub const fn try_borrow(&self) -> Result<Ref<'_, T>, BorrowError> {
        match BorrowRef::new(&self.borrow) {
            Some(b) => {
                #[cfg(feature = "debug_refcell")]
                {
                    // `borrowed_at` 始终记录的是*第一个*处于活动状态的借用
                    if b.borrow.get() == 1 {
                        self.borrowed_at.replace(Some(crate::panic::Location::caller()));
                    }
                }

                // SAFETY:`BorrowRef` 保证了在借用期间只会对该值进行不可变访问。
                let value = unsafe { NonNull::new_unchecked(self.value.get()) };
                Ok(Ref { value, borrow: b })
            }
            None => Err(BorrowError {
                // 如果借用发生了冲突,那么我们此时必定已有一个未释放的借用存在,
                // 所以 `borrowed_at` 一定是 `Some`
                #[cfg(feature = "debug_refcell")]
                location: self.borrowed_at.get().unwrap(),
            }),
        }
    }

    /// 可变地借用所包裹的值。
    ///
    /// 该借用一直持续到返回的 `RefMut`、以及由它派生出的所有 `RefMut` 全部离开作用域为止。
    /// 在此借用处于活动状态期间,该值不能再被借用。
    ///
    /// # Panics
    ///
    /// 如果该值当前正被借用,则 panic。若需要不会 panic 的变体,请使用
    /// [`try_borrow_mut`](#method.try_borrow_mut)。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::cell::RefCell;
    ///
    /// let c = RefCell::new("hello".to_owned());
    ///
    /// *c.borrow_mut() = "bonjour".to_owned();
    ///
    /// assert_eq!(&*c.borrow(), "bonjour");
    /// ```
    ///
    /// 一个会 panic 的例子:
    ///
    /// ```should_panic
    /// use std::cell::RefCell;
    ///
    /// let c = RefCell::new(5);
    /// let m = c.borrow();
    ///
    /// let b = c.borrow_mut(); // 这里会引发 panic
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    #[track_caller]
    #[rustc_const_unstable(feature = "const_ref_cell", issue = "137844")]
    #[rustc_should_not_be_called_on_const_items]
    pub const fn borrow_mut(&self) -> RefMut<'_, T> {
        match self.try_borrow_mut() {
            Ok(b) => b,
            Err(err) => panic_already_borrowed(err),
        }
    }

    /// 可变地借用所包裹的值;如果该值当前正被借用,则返回一个错误。
    ///
    /// 该借用一直持续到返回的 `RefMut`、以及由它派生出的所有 `RefMut` 全部离开作用域为止。
    /// 在此借用处于活动状态期间,该值不能再被借用。
    ///
    /// 这是 [`borrow_mut`](#method.borrow_mut) 不会 panic 的变体。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::cell::RefCell;
    ///
    /// let c = RefCell::new(5);
    ///
    /// {
    ///     let m = c.borrow();
    ///     assert!(c.try_borrow_mut().is_err());
    /// }
    ///
    /// assert!(c.try_borrow_mut().is_ok());
    /// ```
    #[stable(feature = "try_borrow", since = "1.13.0")]
    #[inline]
    #[cfg_attr(feature = "debug_refcell", track_caller)]
    #[rustc_const_unstable(feature = "const_ref_cell", issue = "137844")]
    #[rustc_should_not_be_called_on_const_items]
    pub const fn try_borrow_mut(&self) -> Result<RefMut<'_, T>, BorrowMutError> {
        match BorrowRefMut::new(&self.borrow) {
            Some(b) => {
                #[cfg(feature = "debug_refcell")]
                {
                    self.borrowed_at.replace(Some(crate::panic::Location::caller()));
                }

                // SAFETY:`BorrowRefMut` 保证了独占访问。
                let value = unsafe { NonNull::new_unchecked(self.value.get()) };
                Ok(RefMut { value, borrow: b, marker: PhantomData })
            }
            None => Err(BorrowMutError {
                // 如果借用发生了冲突,那么我们此时必定已有一个未释放的借用存在,
                // 所以 `borrowed_at` 一定是 `Some`
                #[cfg(feature = "debug_refcell")]
                location: self.borrowed_at.get().unwrap(),
            }),
        }
    }

    /// 返回一个指向该 cell 内部数据的裸指针。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::cell::RefCell;
    ///
    /// let c = RefCell::new(5);
    ///
    /// let ptr = c.as_ptr();
    /// ```
    #[inline]
    #[stable(feature = "cell_as_ptr", since = "1.12.0")]
    #[rustc_as_ptr]
    #[rustc_never_returns_null_ptr]
    #[rustc_const_unstable(feature = "const_ref_cell", issue = "137844")]
    pub const fn as_ptr(&self) -> *mut T {
        self.value.get()
    }

    /// 返回一个指向内部数据的可变引用。
    ///
    /// 由于本方法以可变方式借用 `RefCell`,因此可在静态层面保证不存在任何指向内部数据的借用。
    /// 这样一来,[`borrow_mut`] 以及 `RefCell` 大多数其他方法所固有的动态检查在此就没有必要了。
    /// 注意:如果此前曾有借用被泄漏(例如对某个 [`Ref`] 或 [`RefMut`] 调用了 [`forget()`]),
    /// 本方法**不会**重置借用状态。要达到那种目的,请考虑使用尚不稳定的 [`undo_leak`] 方法。
    ///
    /// 本方法只有在 `RefCell` 能够被可变借用时才可调用,而这一般只发生在 `RefCell` 刚被创建之后。
    /// 在这些情形下,跳过前述的动态借用检查也许能带来更好的使用体验和运行时性能。
    ///
    /// 在大多数使用 `RefCell` 的场合,它都无法被可变借用。届时请改用 [`borrow_mut`] 来获得对
    /// 内部数据的可变访问。
    ///
    /// [`borrow_mut`]: RefCell::borrow_mut()
    /// [`forget()`]: mem::forget
    /// [`undo_leak`]: RefCell::undo_leak()
    ///
    /// # 示例
    ///
    /// ```
    /// use std::cell::RefCell;
    ///
    /// let mut c = RefCell::new(5);
    /// *c.get_mut() += 1;
    ///
    /// assert_eq!(c, RefCell::new(6));
    /// ```
    #[inline]
    #[stable(feature = "cell_get_mut", since = "1.11.0")]
    #[rustc_const_unstable(feature = "const_ref_cell", issue = "137844")]
    pub const fn get_mut(&mut self) -> &mut T {
        self.value.get_mut()
    }

    /// 撤销已泄漏的借用守护对 `RefCell` 借用状态造成的影响。
    ///
    /// 本调用与 [`get_mut`] 类似,但更为专门化。它以可变方式借用 `RefCell` 以确保不存在任何
    /// 借用,然后重置用于跟踪共享借用的状态。当某些 `Ref` 或 `RefMut` 借用已被泄漏时,这才有意义。
    ///
    /// [`get_mut`]: RefCell::get_mut()
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(cell_leak)]
    /// use std::cell::RefCell;
    ///
    /// let mut c = RefCell::new(0);
    /// std::mem::forget(c.borrow_mut());
    ///
    /// assert!(c.try_borrow().is_err());
    /// c.undo_leak();
    /// assert!(c.try_borrow().is_ok());
    /// ```
    #[unstable(feature = "cell_leak", issue = "69099")]
    #[rustc_const_unstable(feature = "const_ref_cell", issue = "137844")]
    pub const fn undo_leak(&mut self) -> &mut T {
        *self.borrow.get_mut() = UNUSED;
        self.get_mut()
    }

    /// 不可变地借用所包裹的值;如果该值当前正被可变借用,则返回一个错误。
    ///
    /// # 安全性(Safety）
    ///
    /// 与 `RefCell::borrow` 不同,本方法是 unsafe 的,因为它不返回 `Ref`,因而不会改动借用标志位。
    /// 在本方法所返回的引用仍存活期间对该 `RefCell` 进行可变借用,属于未定义行为。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::cell::RefCell;
    ///
    /// let c = RefCell::new(5);
    ///
    /// {
    ///     let m = c.borrow_mut();
    ///     assert!(unsafe { c.try_borrow_unguarded() }.is_err());
    /// }
    ///
    /// {
    ///     let m = c.borrow();
    ///     assert!(unsafe { c.try_borrow_unguarded() }.is_ok());
    /// }
    /// ```
    #[stable(feature = "borrow_state", since = "1.37.0")]
    #[inline]
    #[rustc_const_unstable(feature = "const_ref_cell", issue = "137844")]
    pub const unsafe fn try_borrow_unguarded(&self) -> Result<&T, BorrowError> {
        if !is_writing(self.borrow.get()) {
            // SAFETY:我们检查了当前没有任何一方在主动写入,但要确保在所返回的引用不再被使用
            // 之前都没有任何一方进行写入,则是调用者的责任。
            // 此外,`self.value.get()` 指向的是 `self` 所拥有的值,因此保证在 `self` 的整个
            // 生命周期内都是有效的。
            Ok(unsafe { &*self.value.get() })
        } else {
            Err(BorrowError {
                // 如果借用发生了冲突,那么我们此时必定已有一个未释放的借用存在,
                // 所以 `borrowed_at` 一定是 `Some`
                #[cfg(feature = "debug_refcell")]
                location: self.borrowed_at.get().unwrap(),
            })
        }
    }
}

impl<T: Default> RefCell<T> {
    /// 取出所包裹的值,并在原处留下 `Default::default()`。
    ///
    /// # Panics
    ///
    /// 如果该值当前正被借用,则 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::cell::RefCell;
    ///
    /// let c = RefCell::new(5);
    /// let five = c.take();
    ///
    /// assert_eq!(five, 5);
    /// assert_eq!(c.into_inner(), 0);
    /// ```
    #[stable(feature = "refcell_take", since = "1.50.0")]
    pub fn take(&self) -> T {
        self.replace(Default::default())
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
unsafe impl<T: ?Sized> Send for RefCell<T> where T: Send {}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T: ?Sized> !Sync for RefCell<T> {}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T: Clone> Clone for RefCell<T> {
    /// # Panics
    ///
    /// 如果该值当前正被可变借用,则 panic。
    #[inline]
    #[track_caller]
    fn clone(&self) -> RefCell<T> {
        RefCell::new(self.borrow().clone())
    }

    /// # Panics
    ///
    /// 如果 `source` 当前正被可变借用,则 panic。
    #[inline]
    #[track_caller]
    fn clone_from(&mut self, source: &Self) {
        self.get_mut().clone_from(&source.borrow())
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_unstable(feature = "const_default", issue = "143894")]
impl<T: [const] Default> const Default for RefCell<T> {
    /// 创建一个 `RefCell<T>`,其值为 T 的 `Default` 默认值。
    #[inline]
    fn default() -> RefCell<T> {
        RefCell::new(Default::default())
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T: ?Sized + PartialEq> PartialEq for RefCell<T> {
    /// # Panics
    ///
    /// 如果两个 `RefCell` 中任意一个的值当前正被可变借用,则 panic。
    #[inline]
    fn eq(&self, other: &RefCell<T>) -> bool {
        *self.borrow() == *other.borrow()
    }
}

#[stable(feature = "cell_eq", since = "1.2.0")]
impl<T: ?Sized + Eq> Eq for RefCell<T> {}

#[stable(feature = "cell_ord", since = "1.10.0")]
impl<T: ?Sized + PartialOrd> PartialOrd for RefCell<T> {
    /// # Panics
    ///
    /// 如果两个 `RefCell` 中任意一个的值当前正被可变借用,则 panic。
    #[inline]
    fn partial_cmp(&self, other: &RefCell<T>) -> Option<Ordering> {
        self.borrow().partial_cmp(&*other.borrow())
    }

    /// # Panics
    ///
    /// 如果两个 `RefCell` 中任意一个的值当前正被可变借用,则 panic。
    #[inline]
    fn lt(&self, other: &RefCell<T>) -> bool {
        *self.borrow() < *other.borrow()
    }

    /// # Panics
    ///
    /// 如果两个 `RefCell` 中任意一个的值当前正被可变借用,则 panic。
    #[inline]
    fn le(&self, other: &RefCell<T>) -> bool {
        *self.borrow() <= *other.borrow()
    }

    /// # Panics
    ///
    /// 如果两个 `RefCell` 中任意一个的值当前正被可变借用,则 panic。
    #[inline]
    fn gt(&self, other: &RefCell<T>) -> bool {
        *self.borrow() > *other.borrow()
    }

    /// # Panics
    ///
    /// 如果两个 `RefCell` 中任意一个的值当前正被可变借用,则 panic。
    #[inline]
    fn ge(&self, other: &RefCell<T>) -> bool {
        *self.borrow() >= *other.borrow()
    }
}

#[stable(feature = "cell_ord", since = "1.10.0")]
impl<T: ?Sized + Ord> Ord for RefCell<T> {
    /// # Panics
    ///
    /// 如果两个 `RefCell` 中任意一个的值当前正被可变借用,则 panic。
    #[inline]
    fn cmp(&self, other: &RefCell<T>) -> Ordering {
        self.borrow().cmp(&*other.borrow())
    }
}

#[stable(feature = "cell_from", since = "1.12.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl<T> const From<T> for RefCell<T> {
    /// 创建一个内含给定值的新 `RefCell<T>`。
    fn from(t: T) -> RefCell<T> {
        RefCell::new(t)
    }
}

#[unstable(feature = "coerce_unsized", issue = "18598")]
impl<T: CoerceUnsized<U>, U> CoerceUnsized<RefCell<U>> for RefCell<T> {}

struct BorrowRef<'b> {
    borrow: &'b Cell<BorrowCounter>,
}

impl<'b> BorrowRef<'b> {
    #[inline]
    const fn new(borrow: &'b Cell<BorrowCounter>) -> Option<BorrowRef<'b>> {
        let b = borrow.get().wrapping_add(1);
        if !is_reading(b) {
            // 借用计数自增后仍得到一个非读取值(<= 0)的情形有以下几种:
            // 1. 它原本 < 0,即存在写入借用,因此根据 Rust 的引用别名规则,我们不能再允许读取借用
            // 2. 它原本是 isize::MAX(读取借用数量的上限),自增后溢出成了 isize::MIN
            //    (写入借用数量的上限),所以我们不能再添加一个读取借用,因为 isize 表示不了
            //    这么多读取借用(只有当你 mem::forget 掉超过一个不小的常量数量的 `Ref` 时才会
            //    发生,这并非好的实践)
            None
        } else {
            // 借用计数自增后得到一个读取值(> 0)的情形有以下几种:
            // 1. 它原本 = 0,即未被借用,而我们正在取得第一个读取借用
            // 2. 它原本 > 0 且 < isize::MAX,即已存在读取借用,而 isize 足够大,可以再表示一个读取借用
            borrow.replace(b);
            Some(BorrowRef { borrow })
        }
    }
}

#[rustc_const_unstable(feature = "const_ref_cell", issue = "137844")]
impl const Drop for BorrowRef<'_> {
    #[inline]
    fn drop(&mut self) {
        let borrow = self.borrow.get();
        debug_assert!(is_reading(borrow));
        self.borrow.replace(borrow - 1);
    }
}

#[rustc_const_unstable(feature = "const_ref_cell", issue = "137844")]
impl const Clone for BorrowRef<'_> {
    #[inline]
    fn clone(&self) -> Self {
        // 既然这个 Ref 存在,我们就知道借用标志位是一个读取借用。
        let borrow = self.borrow.get();
        debug_assert!(is_reading(borrow));
        // 防止借用计数溢出而变成一个写入借用。
        assert!(borrow != BorrowCounter::MAX);
        self.borrow.replace(borrow + 1);
        BorrowRef { borrow: self.borrow }
    }
}

/// 包裹一个对 `RefCell` 盒子中某个值的借用引用。
/// 这是一个包装类型,代表从 `RefCell<T>` 中不可变借用出来的值。
///
/// 更多内容参见[模块级文档](self)。
#[stable(feature = "rust1", since = "1.0.0")]
#[must_not_suspend = "holding a Ref across suspend points can cause BorrowErrors"]
#[rustc_diagnostic_item = "RefCellRef"]
pub struct Ref<'b, T: ?Sized + 'b> {
    // 注意:这里我们使用裸指针而非 `&'b T`,以避免违反 `noalias`。因为 `Ref` 作为参数时,
    // 它并不会在其整个作用域内都保持其指向数据的不可变性,而只到它被 drop 为止。
    // `NonNull` 也像 `&T` 那样对 `T` 是协变的。
    value: NonNull<T>,
    borrow: BorrowRef<'b>,
}

#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl<T: ?Sized> const Deref for Ref<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        // SAFETY:只要我们仍持有自己的这次借用,该值就是可访问的。
        unsafe { self.value.as_ref() }
    }
}

#[unstable(feature = "deref_pure_trait", issue = "87121")]
unsafe impl<T: ?Sized> DerefPure for Ref<'_, T> {}

impl<'b, T: ?Sized> Ref<'b, T> {
    /// 复制一个 `Ref`。
    ///
    /// 此时该 `RefCell` 已经被不可变借用,所以这一操作不会失败。
    ///
    /// 这是一个关联函数,需要以 `Ref::clone(...)` 的形式使用。之所以不实现 `Clone` 或不做成
    /// 方法,是为了不与人们广泛使用的 `r.borrow().clone()`(用于克隆 `RefCell` 的内容)相冲突。
    #[stable(feature = "cell_extras", since = "1.15.0")]
    #[must_use]
    #[inline]
    #[rustc_const_unstable(feature = "const_ref_cell", issue = "137844")]
    pub const fn clone(orig: &Ref<'b, T>) -> Ref<'b, T> {
        Ref { value: orig.value, borrow: orig.borrow.clone() }
    }

    /// 为所借用数据的某个组成部分制作一个新的 `Ref`。
    ///
    /// 此时该 `RefCell` 已经被不可变借用,所以这一操作不会失败。
    ///
    /// 这是一个关联函数,需要以 `Ref::map(...)` 的形式使用。做成方法会与通过 `Deref` 访问到的
    /// `RefCell` 内容上的同名方法相冲突。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::cell::{RefCell, Ref};
    ///
    /// let c = RefCell::new((5, 'b'));
    /// let b1: Ref<'_, (u32, char)> = c.borrow();
    /// let b2: Ref<'_, u32> = Ref::map(b1, |t| &t.0);
    /// assert_eq!(*b2, 5)
    /// ```
    #[stable(feature = "cell_map", since = "1.8.0")]
    #[inline]
    pub fn map<U: ?Sized, F>(orig: Ref<'b, T>, f: F) -> Ref<'b, U>
    where
        F: FnOnce(&T) -> &U,
    {
        Ref { value: NonNull::from(f(&*orig)), borrow: orig.borrow }
    }

    /// 为所借用数据中某个可选的组成部分制作一个新的 `Ref`。如果闭包返回 `None`,
    /// 则原来的借用守护会以 `Err(..)` 的形式被返回。
    ///
    /// 此时该 `RefCell` 已经被不可变借用,所以这一操作不会失败。
    ///
    /// 这是一个关联函数,需要以 `Ref::filter_map(...)` 的形式使用。做成方法会与通过 `Deref`
    /// 访问到的 `RefCell` 内容上的同名方法相冲突。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::cell::{RefCell, Ref};
    ///
    /// let c = RefCell::new(vec![1, 2, 3]);
    /// let b1: Ref<'_, Vec<u32>> = c.borrow();
    /// let b2: Result<Ref<'_, u32>, _> = Ref::filter_map(b1, |v| v.get(1));
    /// assert_eq!(*b2.unwrap(), 2);
    /// ```
    #[stable(feature = "cell_filter_map", since = "1.63.0")]
    #[inline]
    pub fn filter_map<U: ?Sized, F>(orig: Ref<'b, T>, f: F) -> Result<Ref<'b, U>, Self>
    where
        F: FnOnce(&T) -> Option<&U>,
    {
        match f(&*orig) {
            Some(value) => Ok(Ref { value: NonNull::from(value), borrow: orig.borrow }),
            None => Err(orig),
        }
    }

    /// 尝试为所借用数据的某个组成部分制作一个新的 `Ref`。
    /// 失败时,会连同闭包返回的错误一起,把原来的借用守护返回回来。
    ///
    /// 此时该 `RefCell` 已经被不可变借用,所以这一操作不会失败。
    ///
    /// 这是一个关联函数,需要以 `Ref::try_map(...)` 的形式使用。做成方法会与通过 `Deref`
    /// 访问到的 `RefCell` 内容上的同名方法相冲突。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(refcell_try_map)]
    /// use std::cell::{RefCell, Ref};
    /// use std::str::{from_utf8, Utf8Error};
    ///
    /// let c = RefCell::new(vec![0xF0, 0x9F, 0xA6 ,0x80]);
    /// let b1: Ref<'_, Vec<u8>> = c.borrow();
    /// let b2: Result<Ref<'_, str>, _> = Ref::try_map(b1, |v| from_utf8(v));
    /// assert_eq!(&*b2.unwrap(), "🦀");
    ///
    /// let c = RefCell::new(vec![0xF0, 0x9F, 0xA6]);
    /// let b1: Ref<'_, Vec<u8>> = c.borrow();
    /// let b2: Result<_, (Ref<'_, Vec<u8>>, Utf8Error)> = Ref::try_map(b1, |v| from_utf8(v));
    /// let (b3, e) = b2.unwrap_err();
    /// assert_eq!(*b3, vec![0xF0, 0x9F, 0xA6]);
    /// assert_eq!(e.valid_up_to(), 0);
    /// ```
    #[unstable(feature = "refcell_try_map", issue = "143801")]
    #[inline]
    pub fn try_map<U: ?Sized, E>(
        orig: Ref<'b, T>,
        f: impl FnOnce(&T) -> Result<&U, E>,
    ) -> Result<Ref<'b, U>, (Self, E)> {
        match f(&*orig) {
            Ok(value) => Ok(Ref { value: NonNull::from(value), borrow: orig.borrow }),
            Err(e) => Err((orig, e)),
        }
    }

    /// 把一个 `Ref` 拆分成多个 `Ref`,分别对应所借用数据的不同组成部分。
    ///
    /// 此时该 `RefCell` 已经被不可变借用,所以这一操作不会失败。
    ///
    /// 这是一个关联函数,需要以 `Ref::map_split(...)` 的形式使用。做成方法会与通过 `Deref`
    /// 访问到的 `RefCell` 内容上的同名方法相冲突。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::cell::{Ref, RefCell};
    ///
    /// let cell = RefCell::new([1, 2, 3, 4]);
    /// let borrow = cell.borrow();
    /// let (begin, end) = Ref::map_split(borrow, |slice| slice.split_at(2));
    /// assert_eq!(*begin, [1, 2]);
    /// assert_eq!(*end, [3, 4]);
    /// ```
    #[stable(feature = "refcell_map_split", since = "1.35.0")]
    #[inline]
    pub fn map_split<U: ?Sized, V: ?Sized, F>(orig: Ref<'b, T>, f: F) -> (Ref<'b, U>, Ref<'b, V>)
    where
        F: FnOnce(&T) -> (&U, &V),
    {
        let (a, b) = f(&*orig);
        let borrow = orig.borrow.clone();
        (
            Ref { value: NonNull::from(a), borrow },
            Ref { value: NonNull::from(b), borrow: orig.borrow },
        )
    }

    /// 转换为一个指向内部数据的引用。
    ///
    /// 此后底层的 `RefCell` 将永远无法再被可变借用,并且会始终表现为已被不可变借用的状态。
    /// 泄漏超过常数个引用并不是个好主意。只要泄漏的总次数还较少,该 `RefCell` 仍可被再次不可变借用。
    ///
    /// 这是一个关联函数,需要以 `Ref::leak(...)` 的形式使用。做成方法会与通过 `Deref`
    /// 访问到的 `RefCell` 内容上的同名方法相冲突。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(cell_leak)]
    /// use std::cell::{RefCell, Ref};
    /// let cell = RefCell::new(0);
    ///
    /// let value = Ref::leak(cell.borrow());
    /// assert_eq!(*value, 0);
    ///
    /// assert!(cell.try_borrow().is_ok());
    /// assert!(cell.try_borrow_mut().is_err());
    /// ```
    #[unstable(feature = "cell_leak", issue = "69099")]
    #[rustc_const_unstable(feature = "const_ref_cell", issue = "137844")]
    pub const fn leak(orig: Ref<'b, T>) -> &'b T {
        // 通过 forget 掉这个 Ref,我们确保了在生命周期 `'b` 内,该 RefCell 的借用计数不会回到
        // UNUSED。要重置引用跟踪状态,需要一个指向被借用 RefCell 的独占引用;而从原 cell 不可能
        // 再创建出任何可变引用。
        mem::forget(orig.borrow);
        // SAFETY:forget 之后,我们就可以为生命周期 `'b` 的剩余部分构造一个引用。
        unsafe { orig.value.as_ref() }
    }
}

#[unstable(feature = "coerce_unsized", issue = "18598")]
impl<'b, T: ?Sized + Unsize<U>, U: ?Sized> CoerceUnsized<Ref<'b, U>> for Ref<'b, T> {}

#[stable(feature = "std_guard_impls", since = "1.20.0")]
impl<T: ?Sized + fmt::Display> fmt::Display for Ref<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}

impl<'b, T: ?Sized> RefMut<'b, T> {
    /// 为所借用数据的某个组成部分(例如某个枚举变体)制作一个新的 `RefMut`。
    ///
    /// 此时该 `RefCell` 已经被可变借用,所以这一操作不会失败。
    ///
    /// 这是一个关联函数,需要以 `RefMut::map(...)` 的形式使用。做成方法会与通过 `Deref`
    /// 访问到的 `RefCell` 内容上的同名方法相冲突。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::cell::{RefCell, RefMut};
    ///
    /// let c = RefCell::new((5, 'b'));
    /// {
    ///     let b1: RefMut<'_, (u32, char)> = c.borrow_mut();
    ///     let mut b2: RefMut<'_, u32> = RefMut::map(b1, |t| &mut t.0);
    ///     assert_eq!(*b2, 5);
    ///     *b2 = 42;
    /// }
    /// assert_eq!(*c.borrow(), (42, 'b'));
    /// ```
    #[stable(feature = "cell_map", since = "1.8.0")]
    #[inline]
    pub fn map<U: ?Sized, F>(mut orig: RefMut<'b, T>, f: F) -> RefMut<'b, U>
    where
        F: FnOnce(&mut T) -> &mut U,
    {
        let value = NonNull::from(f(&mut *orig));
        RefMut { value, borrow: orig.borrow, marker: PhantomData }
    }

    /// 为所借用数据中某个可选的组成部分制作一个新的 `RefMut`。如果闭包返回 `None`,
    /// 则原来的借用守护会以 `Err(..)` 的形式被返回。
    ///
    /// 此时该 `RefCell` 已经被可变借用,所以这一操作不会失败。
    ///
    /// 这是一个关联函数,需要以 `RefMut::filter_map(...)` 的形式使用。做成方法会与通过 `Deref`
    /// 访问到的 `RefCell` 内容上的同名方法相冲突。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::cell::{RefCell, RefMut};
    ///
    /// let c = RefCell::new(vec![1, 2, 3]);
    ///
    /// {
    ///     let b1: RefMut<'_, Vec<u32>> = c.borrow_mut();
    ///     let mut b2: Result<RefMut<'_, u32>, _> = RefMut::filter_map(b1, |v| v.get_mut(1));
    ///
    ///     if let Ok(mut b2) = b2 {
    ///         *b2 += 2;
    ///     }
    /// }
    ///
    /// assert_eq!(*c.borrow(), vec![1, 4, 3]);
    /// ```
    #[stable(feature = "cell_filter_map", since = "1.63.0")]
    #[inline]
    pub fn filter_map<U: ?Sized, F>(mut orig: RefMut<'b, T>, f: F) -> Result<RefMut<'b, U>, Self>
    where
        F: FnOnce(&mut T) -> Option<&mut U>,
    {
        // SAFETY:函数在其调用期间通过 `orig` 一直持有一个独占引用,而该指针仅在函数调用内部
        // 被解引用,绝不会让那个独占引用逃逸出去。
        match f(&mut *orig) {
            Some(value) => {
                Ok(RefMut { value: NonNull::from(value), borrow: orig.borrow, marker: PhantomData })
            }
            None => Err(orig),
        }
    }

    /// 尝试为所借用数据的某个组成部分制作一个新的 `RefMut`。
    /// 失败时,会连同闭包返回的错误一起,把原来的借用守护返回回来。
    ///
    /// 此时该 `RefCell` 已经被可变借用,所以这一操作不会失败。
    ///
    /// 这是一个关联函数,需要以 `RefMut::try_map(...)` 的形式使用。做成方法会与通过 `Deref`
    /// 访问到的 `RefCell` 内容上的同名方法相冲突。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(refcell_try_map)]
    /// use std::cell::{RefCell, RefMut};
    /// use std::str::{from_utf8_mut, Utf8Error};
    ///
    /// let c = RefCell::new(vec![0x68, 0x65, 0x6C, 0x6C, 0x6F]);
    /// {
    ///     let b1: RefMut<'_, Vec<u8>> = c.borrow_mut();
    ///     let b2: Result<RefMut<'_, str>, _> = RefMut::try_map(b1, |v| from_utf8_mut(v));
    ///     let mut b2 = b2.unwrap();
    ///     assert_eq!(&*b2, "hello");
    ///     b2.make_ascii_uppercase();
    /// }
    /// assert_eq!(*c.borrow(), "HELLO".as_bytes());
    ///
    /// let c = RefCell::new(vec![0xFF]);
    /// let b1: RefMut<'_, Vec<u8>> = c.borrow_mut();
    /// let b2: Result<_, (RefMut<'_, Vec<u8>>, Utf8Error)> = RefMut::try_map(b1, |v| from_utf8_mut(v));
    /// let (b3, e) = b2.unwrap_err();
    /// assert_eq!(*b3, vec![0xFF]);
    /// assert_eq!(e.valid_up_to(), 0);
    /// ```
    #[unstable(feature = "refcell_try_map", issue = "143801")]
    #[inline]
    pub fn try_map<U: ?Sized, E>(
        mut orig: RefMut<'b, T>,
        f: impl FnOnce(&mut T) -> Result<&mut U, E>,
    ) -> Result<RefMut<'b, U>, (Self, E)> {
        // SAFETY:函数在其调用期间通过 `orig` 一直持有一个独占引用,而该指针仅在函数调用内部
        // 被解引用,绝不会让那个独占引用逃逸出去。
        match f(&mut *orig) {
            Ok(value) => {
                Ok(RefMut { value: NonNull::from(value), borrow: orig.borrow, marker: PhantomData })
            }
            Err(e) => Err((orig, e)),
        }
    }

    /// 把一个 `RefMut` 拆分成多个 `RefMut`,分别对应所借用数据的不同组成部分。
    ///
    /// 底层的 `RefCell` 会一直保持可变借用状态,直到返回的两个 `RefMut` 都离开作用域为止。
    /// 这两个 `RefMut` 必须指向原值中互不重叠的部分;借用计数会用两个可变守护共同表示同一次
    /// 动态独占借用。
    ///
    /// 此时该 `RefCell` 已经被可变借用,所以这一操作不会失败。
    ///
    /// 这是一个关联函数,需要以 `RefMut::map_split(...)` 的形式使用。做成方法会与通过 `Deref`
    /// 访问到的 `RefCell` 内容上的同名方法相冲突。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::cell::{RefCell, RefMut};
    ///
    /// let cell = RefCell::new([1, 2, 3, 4]);
    /// let borrow = cell.borrow_mut();
    /// let (mut begin, mut end) = RefMut::map_split(borrow, |slice| slice.split_at_mut(2));
    /// assert_eq!(*begin, [1, 2]);
    /// assert_eq!(*end, [3, 4]);
    /// begin.copy_from_slice(&[4, 3]);
    /// end.copy_from_slice(&[2, 1]);
    /// ```
    #[stable(feature = "refcell_map_split", since = "1.35.0")]
    #[inline]
    pub fn map_split<U: ?Sized, V: ?Sized, F>(
        mut orig: RefMut<'b, T>,
        f: F,
    ) -> (RefMut<'b, U>, RefMut<'b, V>)
    where
        F: FnOnce(&mut T) -> (&mut U, &mut V),
    {
        let borrow = orig.borrow.clone();
        let (a, b) = f(&mut *orig);
        (
            RefMut { value: NonNull::from(a), borrow, marker: PhantomData },
            RefMut { value: NonNull::from(b), borrow: orig.borrow, marker: PhantomData },
        )
    }

    /// 转换为一个指向内部数据的可变引用。
    ///
    /// 此后底层的 `RefCell` 将无法再被借用,并且会始终表现为已被可变借用的状态,从而使所返回的
    /// 引用成为指向其内部的唯一引用。
    ///
    /// 这是一个关联函数,需要以 `RefMut::leak(...)` 的形式使用。做成方法会与通过 `Deref`
    /// 访问到的 `RefCell` 内容上的同名方法相冲突。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(cell_leak)]
    /// use std::cell::{RefCell, RefMut};
    /// let cell = RefCell::new(0);
    ///
    /// let value = RefMut::leak(cell.borrow_mut());
    /// assert_eq!(*value, 0);
    /// *value = 1;
    ///
    /// assert!(cell.try_borrow_mut().is_err());
    /// ```
    #[unstable(feature = "cell_leak", issue = "69099")]
    #[rustc_const_unstable(feature = "const_ref_cell", issue = "137844")]
    pub const fn leak(mut orig: RefMut<'b, T>) -> &'b mut T {
        // 通过 forget 掉这个 BorrowRefMut,我们确保了在生命周期 `'b` 内,该 RefCell 的借用计数
        // 不会回到 UNUSED。要重置引用跟踪状态,需要一个指向被借用 RefCell 的独占引用;而在那段
        // 生命周期内,从原 cell 不可能再创建出任何引用,这就使当前这次借用成为剩余生命周期内的
        // 唯一引用。
        mem::forget(orig.borrow);
        // SAFETY:forget 之后,我们就可以为生命周期 `'b` 的剩余部分构造一个引用。
        unsafe { orig.value.as_mut() }
    }
}

struct BorrowRefMut<'b> {
    borrow: &'b Cell<BorrowCounter>,
}

#[rustc_const_unstable(feature = "const_ref_cell", issue = "137844")]
impl const Drop for BorrowRefMut<'_> {
    #[inline]
    fn drop(&mut self) {
        let borrow = self.borrow.get();
        debug_assert!(is_writing(borrow));
        self.borrow.replace(borrow + 1);
    }
}

impl<'b> BorrowRefMut<'b> {
    #[inline]
    const fn new(borrow: &'b Cell<BorrowCounter>) -> Option<BorrowRefMut<'b>> {
        // 注意:与 BorrowRefMut::clone 不同,new 是用来创建初始的那个可变引用的,因此当前必须
        // 不存在任何已有的引用。于是,clone 会把可变借用计数往负方向递增,而这里我们只显式地
        // 允许从 UNUSED 变到 UNUSED - 1。
        match borrow.get() {
            UNUSED => {
                borrow.replace(UNUSED - 1);
                Some(BorrowRefMut { borrow })
            }
            _ => None,
        }
    }

    // 克隆一个 `BorrowRefMut`。
    //
    // 仅当每个 `BorrowRefMut` 都用于跟踪指向原对象中互不重叠的不同区段的可变引用时,这才是
    // 有效的。之所以不放进 Clone impl,是为了避免代码隐式地调用到它。
    #[inline]
    fn clone(&self) -> BorrowRefMut<'b> {
        let borrow = self.borrow.get();
        debug_assert!(is_writing(borrow));
        // 防止借用计数下溢。
        assert!(borrow != BorrowCounter::MIN);
        self.borrow.set(borrow - 1);
        BorrowRefMut { borrow: self.borrow }
    }
}

/// 这是一个包装类型,代表从 `RefCell<T>` 中可变借用出来的值。
///
/// 更多内容参见[模块级文档](self)。
#[stable(feature = "rust1", since = "1.0.0")]
#[must_not_suspend = "holding a RefMut across suspend points can cause BorrowErrors"]
#[rustc_diagnostic_item = "RefCellRefMut"]
pub struct RefMut<'b, T: ?Sized + 'b> {
    // 注意:这里我们使用裸指针而非 `&'b mut T`,以避免违反 `noalias`。因为 `RefMut` 作为参数时,
    // 它并不会在其整个作用域内都保持其独占性,而只到它被 drop 为止。
    value: NonNull<T>,
    borrow: BorrowRefMut<'b>,
    // `NonNull` 对 `T` 是协变的,因此我们需要重新引入不变性。
    marker: PhantomData<&'b mut T>,
}

#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl<T: ?Sized> const Deref for RefMut<'_, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &T {
        // SAFETY:只要我们仍持有自己的这次借用,该值就是可访问的。
        unsafe { self.value.as_ref() }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl<T: ?Sized> const DerefMut for RefMut<'_, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut T {
        // SAFETY:只要我们仍持有自己的这次借用,该值就是可访问的。
        unsafe { self.value.as_mut() }
    }
}

#[unstable(feature = "deref_pure_trait", issue = "87121")]
unsafe impl<T: ?Sized> DerefPure for RefMut<'_, T> {}

#[unstable(feature = "coerce_unsized", issue = "18598")]
impl<'b, T: ?Sized + Unsize<U>, U: ?Sized> CoerceUnsized<RefMut<'b, U>> for RefMut<'b, T> {}

#[stable(feature = "std_guard_impls", since = "1.20.0")]
impl<T: ?Sized + fmt::Display> fmt::Display for RefMut<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}

/// Rust 中实现内部可变性的核心原语。
///
/// 如果你持有一个引用 `&T`,那么在通常情况下,Rust 编译器会基于“`&T` 指向不可变数据”这一认知
/// 进行优化。修改那块数据——例如通过某个别名,或者把 `&T` transmute 成 `&mut T`——会被视为
/// 未定义行为。`UnsafeCell<T>` 则**退出**了针对 `&T` 的不可变性保证:共享引用 `&UnsafeCell<T>`
/// 可以指向正在被修改的数据。这就叫“内部可变性”。
///
/// 所有其他允许内部可变性的类型,例如 [`Cell<T>`] 和 [`RefCell<T>`],其内部都使用 `UnsafeCell`
/// 来包裹自己的数据。`UnsafeCell` 是一个 lang item,是编译器据以**对其内容关闭别名优化**的唯一
/// 合法基石:正因为标了 `UnsafeCell`,编译器才不再假设“透过 `&` 看到的内存不会改变”。任何
/// 绕过 `UnsafeCell`、用裸指针或 transmute 强行从 `&T` 修改数据的做法都会触发 UB。
///
/// 注意,只有针对共享引用的不可变性保证才会受 `UnsafeCell` 的影响。针对可变引用的唯一性保证
/// 则不受影响。即便有了 `UnsafeCell<T>`,也*绝没有*任何合法手段能造出彼此别名的 `&mut`。
///
/// `UnsafeCell` 本身不会为避免数据竞争做任何事;数据竞争依旧是未定义行为。如果多个线程访问
/// 同一个 `UnsafeCell`,它们必须遵循通常的[并发内存模型][concurrent memory model]规则:相互
/// 冲突的、未经同步的访问必须借助 [`core::sync::atomic`] 中的 API 来完成。
///
/// `UnsafeCell` 的 API 本身在技术上非常简单:[`.get()`] 会给你一个指向其内容的裸指针 `*mut T`。
/// 至于如何正确地使用那个裸指针,则取决于*你*这个抽象的设计者。
///
/// [`.get()`]: `UnsafeCell::get`
/// [concurrent memory model]: ../sync/atomic/index.html#memory-model-for-atomic-accesses
///
/// # 别名规则 {#aliasing-rules}
///
/// Rust 精确的别名规则目前仍在不断演变,但其主要要点并无争议:
///
/// - 如果你创建了一个生命周期为 `'a` 的安全引用(无论 `&T` 还是 `&mut T`),那么在 `'a` 的
///   剩余期间,你都不得以任何与该引用相矛盾的方式去访问那块数据。例如,这意味着:如果你从一个
///   `UnsafeCell<T>` 中取出 `*mut T` 并把它转成 `&T`,那么直到该引用的生命周期结束之前,`T`
///   中的数据都必须保持不可变(当然,`T` 内部任何属于 `UnsafeCell` 的数据除外)。同样地,如果
///   你创建了一个 `&mut T` 引用并把它交给了安全代码,那么直到该引用过期之前,你都不得去访问
///   那个 `UnsafeCell` 内部的数据。
///
/// - 对于不含 `UnsafeCell<_>` 的 `&T` 以及对于 `&mut T`,在引用过期之前你也不得释放
///   那块数据。作为一个特例:给定一个 `&T`,其中任何处于 `UnsafeCell<_>` 内部的部分,可以在该
///   引用生命周期内、在该引用最后一次被使用(被解引用或被重新借用)之后被释放。由于你无法只释放
///   一个引用所指对象的一部分,这意味着:只有当 `&T` 所指对象的*每一个部分*(包括 padding)
///   都位于某个 `UnsafeCell` 内部时,它所指向的内存才能被释放。
///
/// 然而,每当一个 `&UnsafeCell<T>` 被构造或被解引用时,它仍必须指向存活的内存;并且,如果编译器
/// 能证明这块内存尚未被释放,它就被允许插入并不来自源代码的伪读取。
///
/// 为了帮助进行正确的设计,以下场景被明确声明为对单线程代码合法:
///
/// 1. 一个 `&T` 引用可以被交给安全代码,在那里它可以与其他 `&T` 引用共存,但不能与某个 `&mut T` 共存。
///
/// 2. 一个 `&mut T` 引用可以被交给安全代码,前提是既没有其他 `&mut T` 也没有 `&T` 与它共存。
///    `&mut T` 必须始终是唯一的。
///
/// 注意,虽然通过 `&UnsafeCell<T>` 修改其内容(即便此时还有其他 `&UnsafeCell<T>` 引用对该 cell
/// 形成别名)是允许的(前提是你以其他某种方式保证了上述各项不变量),但同时持有多个
/// `&mut UnsafeCell<T>` 别名仍然是未定义行为。也就是说,`UnsafeCell` 是一个被设计为与*共享*访问
/// (*即*经由 `&UnsafeCell<_>` 引用进行的访问)产生特殊交互的包装器;在处理*独占*访问(*例如*
/// 经由 `&mut UnsafeCell<_>` 进行的访问)时,它没有任何魔法:在那个 `&mut` 借用持续期间,无论是
/// 该 cell 还是被包裹的值都不得被形成别名。
/// [`.get_mut()`] 访问器正展示了这一点——它是一个*安全的*取值方法,会交出一个 `&mut T`。
///
/// [`.get_mut()`]: `UnsafeCell::get_mut`
///
/// # 内存布局 {#memory-layout}
///
/// `UnsafeCell<T>` 在内存中的表示与其内部类型 `T` 相同。这一保证的一个推论是:可以在 `T` 与
/// `UnsafeCell<T>` 之间进行转换。当把某个 `Outer<T>` 类型内部嵌套的 `T` 转换成 `Outer<UnsafeCell<T>>`
/// 类型时,必须格外小心:如果 `Outer<T>` 类型启用了[空位优化][niche],那么这种转换就是不健全的。
/// 例如,在 64 位平台上 `Option<NonNull<u8>>` 通常占 8 字节,而 `Option<UnsafeCell<NonNull<u8>>>`
/// 却要占 16 字节空间。因此,尽管 `NonNull<u8>` 与 `UnsafeCell<NonNull<u8>>>` 拥有相同的内存
/// 布局,这种转换仍然是无效的。原因在于:`UnsafeCell` 会禁用空位优化,以防止其内部可变性属性从
/// `T` 蔓延到 `Outer` 类型上,从而在这些情形下可能造成类型大小的偏差。
///
/// 注意,要获得一个指向*共享* `UnsafeCell<T>` 内容的 `*mut T` 指针,唯一有效的途径是通过
/// [`.get()`] 或 [`.raw_get()`]。`&mut T` 引用可以通过解引用该指针、或对一个*独占的*
/// `UnsafeCell<T>` 调用 [`.get_mut()`] 来获得。尽管 `T` 与 `UnsafeCell<T>` 拥有相同的内存布局,
/// 下面这种做法仍然是不被允许的,属于未定义行为:
///
/// ```rust,compile_fail
/// # use std::cell::UnsafeCell;
/// unsafe fn not_allowed<T>(ptr: &UnsafeCell<T>) -> &mut T {
///   let t = ptr as *const UnsafeCell<T> as *mut T;
///   // 这是未定义行为,因为这个 `*mut T` 指针
///   // 既不是通过 `.get()` 也不是通过 `.raw_get()` 获得的:
///   unsafe { &mut *t }
/// }
/// ```
///
/// 应当改成这样:
///
/// ```rust
/// # use std::cell::UnsafeCell;
/// // 安全性:调用者必须保证不存在任何指向该 `UnsafeCell` *内容*的引用。
/// unsafe fn get_mut<T>(ptr: &UnsafeCell<T>) -> &mut T {
///   unsafe { &mut *ptr.get() }
/// }
/// ```
///
/// 反方向的转换,即从 `&mut T` 转成 `&UnsafeCell<T>`,则是允许的:
///
/// ```rust
/// # use std::cell::UnsafeCell;
/// fn get_shared<T>(ptr: &mut T) -> &UnsafeCell<T> {
///   let t = ptr as *mut T as *const UnsafeCell<T>;
///   // SAFETY:`T` 与 `UnsafeCell<T>` 拥有相同的内存布局
///   unsafe { &*t }
/// }
/// ```
///
/// [niche]: https://rust-lang.github.io/unsafe-code-guidelines/glossary.html#niche
/// [`.raw_get()`]: `UnsafeCell::raw_get`
///
/// # 示例
///
/// 下面这个例子展示了:即便存在多个对该 cell 形成别名的引用,该如何健全地修改一个 `UnsafeCell<_>`
/// 的内容:
///
/// ```
/// use std::cell::UnsafeCell;
///
/// let x: UnsafeCell<i32> = 42.into();
/// // 取得指向同一个 `x` 的多个 / 并发 / 共享引用。
/// let (p1, p2): (&UnsafeCell<i32>, &UnsafeCell<i32>) = (&x, &x);
///
/// unsafe {
///     // SAFETY:在此作用域内,不存在任何其他指向 `x` 内容的引用,
///     // 所以我们这个引用实际上是独占的。
///     let p1_exclusive: &mut i32 = &mut *p1.get(); // -- 借用开始 --+
///     *p1_exclusive += 27; //                                       |
/// } // <---------- 不能越过这一点 -------------------------------+
///
/// unsafe {
///     // SAFETY:在此作用域内,没有任何一方期望对 `x` 的内容拥有独占访问,
///     // 所以我们可以同时进行多个共享访问。
///     let p2_shared: &i32 = &*p2.get();
///     assert_eq!(*p2_shared, 42 + 27);
///     let p1_shared: &i32 = &*p1.get();
///     assert_eq!(*p1_shared, *p2_shared);
/// }
/// ```
///
/// 下面这个例子则展示了这样一个事实:对 `UnsafeCell<T>` 的独占访问,蕴含着对其内部 `T` 的独占访问:
///
/// ```rust
/// #![forbid(unsafe_code)]
/// // 在独占访问下,`UnsafeCell` 是一个透明的、无操作的包装器,所以这里无需 `unsafe`。
/// use std::cell::UnsafeCell;
///
/// let mut x: UnsafeCell<i32> = 42.into();
///
/// // 取得一个编译期校验过的、指向 `x` 的唯一引用。
/// let p_unique: &mut UnsafeCell<i32> = &mut x;
/// // 有了独占引用,我们就可以无代价地修改其内容。
/// *p_unique.get_mut() = 0;
/// // 或者,等价地:
/// x = UnsafeCell::new(0);
///
/// // 当我们拥有该值时,就可以无代价地把内容取出来。
/// let contents: i32 = x.into_inner();
/// assert_eq!(contents, 0);
/// ```
#[lang = "unsafe_cell"]
#[stable(feature = "rust1", since = "1.0.0")]
#[repr(transparent)]
#[rustc_pub_transparent]
pub struct UnsafeCell<T: ?Sized> {
    value: T,
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T: ?Sized> !Sync for UnsafeCell<T> {}

impl<T> UnsafeCell<T> {
    /// 构造一个新的 `UnsafeCell` 实例,用以包裹指定的值。
    ///
    /// 通过 `&UnsafeCell<T>` 对内部值的所有访问都需要 `unsafe` 代码。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::cell::UnsafeCell;
    ///
    /// let uc = UnsafeCell::new(5);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_unsafe_cell_new", since = "1.32.0")]
    #[inline(always)]
    pub const fn new(value: T) -> UnsafeCell<T> {
        UnsafeCell { value }
    }

    /// 取出该值,同时消耗掉整个 cell。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::cell::UnsafeCell;
    ///
    /// let uc = UnsafeCell::new(5);
    ///
    /// let five = uc.into_inner();
    /// ```
    #[inline(always)]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_cell_into_inner", since = "1.83.0")]
    #[rustc_allow_const_fn_unstable(const_precise_live_drops)]
    pub const fn into_inner(self) -> T {
        self.value
    }

    /// 替换该 `UnsafeCell` 中的值,并返回旧值。
    ///
    /// # 安全性(Safety）
    ///
    /// 调用者必须注意避免别名和数据竞争。
    ///
    /// - 让本调用与对被包裹值的任何其他访问发生竞争,是未定义行为。
    /// - 在仍存在任何其他指向被包裹值的引用时调用本方法,是未定义行为。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(unsafe_cell_access)]
    /// use std::cell::UnsafeCell;
    ///
    /// let uc = UnsafeCell::new(5);
    ///
    /// let old = unsafe { uc.replace(10) };
    /// assert_eq!(old, 5);
    /// ```
    #[inline]
    #[unstable(feature = "unsafe_cell_access", issue = "136327")]
    #[rustc_should_not_be_called_on_const_items]
    pub const unsafe fn replace(&self, value: T) -> T {
        // SAFETY:该指针来自 `&self`,因此天然满足各项不变量。
        unsafe { ptr::replace(self.get(), value) }
    }
}

impl<T: ?Sized> UnsafeCell<T> {
    /// 由 `&mut T` 转换为 `&mut UnsafeCell<T>`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::cell::UnsafeCell;
    ///
    /// let mut val = 42;
    /// let uc = UnsafeCell::from_mut(&mut val);
    ///
    /// *uc.get_mut() -= 1;
    /// assert_eq!(*uc.get_mut(), 41);
    /// ```
    #[inline(always)]
    #[stable(feature = "unsafe_cell_from_mut", since = "1.84.0")]
    #[rustc_const_stable(feature = "unsafe_cell_from_mut", since = "1.84.0")]
    pub const fn from_mut(value: &mut T) -> &mut UnsafeCell<T> {
        // SAFETY:得益于 #[repr(transparent)],`UnsafeCell<T>` 与 `T` 拥有相同的内存布局。
        unsafe { &mut *(value as *mut T as *mut UnsafeCell<T>) }
    }

    /// 取得一个指向被包裹值的可变指针。
    ///
    /// 它可以被转换成任意种类的指针。在创建引用时,你必须遵守别名规则;更多讨论和注意事项参见
    /// [类型级文档][UnsafeCell#aliasing-rules]。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::cell::UnsafeCell;
    ///
    /// let uc = UnsafeCell::new(5);
    ///
    /// let five = uc.get();
    /// ```
    #[inline(always)]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_stable(feature = "const_unsafecell_get", since = "1.32.0")]
    #[rustc_as_ptr]
    #[rustc_never_returns_null_ptr]
    #[rustc_should_not_be_called_on_const_items]
    pub const fn get(&self) -> *mut T {
        // 由于 #[repr(transparent)],我们可以直接把指针从 `UnsafeCell<T>` 转成 `T`。这利用了
        // 标准库的特殊地位;对于用户代码,并不保证未来版本的编译器仍会让这种做法成立!
        self as *const UnsafeCell<T> as *const T as *mut T
    }

    /// 返回一个指向内部数据的可变引用。
    ///
    /// 本调用以可变方式借用该 `UnsafeCell`(在编译期完成),从而保证我们持有的是唯一的那一个引用。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::cell::UnsafeCell;
    ///
    /// let mut c = UnsafeCell::new(5);
    /// *c.get_mut() += 1;
    ///
    /// assert_eq!(*c.get_mut(), 6);
    /// ```
    #[inline(always)]
    #[stable(feature = "unsafe_cell_get_mut", since = "1.50.0")]
    #[rustc_const_stable(feature = "const_unsafecell_get_mut", since = "1.83.0")]
    pub const fn get_mut(&mut self) -> &mut T {
        &mut self.value
    }

    /// 取得一个指向被包裹值的可变指针。
    /// 它与 [`get`] 的区别在于:本函数接收一个裸指针,这在希望避免创建临时引用时很有用。
    ///
    /// 它可以被转换成任意种类的指针。在创建引用时,你必须遵守别名规则;更多讨论和注意事项参见
    /// [类型级文档][UnsafeCell#aliasing-rules]。
    ///
    /// [`get`]: UnsafeCell::get()
    ///
    /// # 示例
    ///
    /// 对 `UnsafeCell` 进行渐进式初始化时必须用 `raw_get`,因为调用 `get` 会要求创建一个指向
    /// 未初始化数据的引用:
    ///
    /// ```
    /// use std::cell::UnsafeCell;
    /// use std::mem::MaybeUninit;
    ///
    /// let m = MaybeUninit::<UnsafeCell<i32>>::uninit();
    /// unsafe { UnsafeCell::raw_get(m.as_ptr()).write(5); }
    /// // 避免下面这种写法,它会引用到未初始化的数据
    /// // unsafe { UnsafeCell::get(&*m.as_ptr()).write(5); }
    /// let uc = unsafe { m.assume_init() };
    ///
    /// assert_eq!(uc.into_inner(), 5);
    /// ```
    #[inline(always)]
    #[stable(feature = "unsafe_cell_raw_get", since = "1.56.0")]
    #[rustc_const_stable(feature = "unsafe_cell_raw_get", since = "1.56.0")]
    #[rustc_diagnostic_item = "unsafe_cell_raw_get"]
    pub const fn raw_get(this: *const Self) -> *mut T {
        // 由于 #[repr(transparent)],我们可以直接把指针从 `UnsafeCell<T>` 转成 `T`。这利用了
        // 标准库的特殊地位;对于用户代码,并不保证未来版本的编译器仍会让这种做法成立!
        this as *const T as *mut T
    }

    /// 取得一个指向 `UnsafeCell` 内部值的共享引用。
    ///
    /// # 安全性(Safety）
    ///
    /// - 在仍存在任何指向被包裹值的可变引用时调用本方法,是未定义行为。
    /// - 在所返回的引用仍存活期间修改被包裹的值,是未定义行为。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(unsafe_cell_access)]
    /// use std::cell::UnsafeCell;
    ///
    /// let uc = UnsafeCell::new(5);
    ///
    /// let val = unsafe { uc.as_ref_unchecked() };
    /// assert_eq!(val, &5);
    /// ```
    #[inline]
    #[unstable(feature = "unsafe_cell_access", issue = "136327")]
    #[rustc_should_not_be_called_on_const_items]
    pub const unsafe fn as_ref_unchecked(&self) -> &T {
        // SAFETY:该指针来自 `&self`,因此天然满足“裸指针转引用”的各项不变量。
        unsafe { self.get().as_ref_unchecked() }
    }

    /// 取得一个指向 `UnsafeCell` 内部值的独占引用。
    ///
    /// # 安全性(Safety）
    ///
    /// - 在仍存在任何其他指向被包裹值的引用时调用本方法,是未定义行为。
    /// - 在所返回的引用仍存活期间,通过其他途径修改被包裹的值,是未定义行为。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(unsafe_cell_access)]
    /// use std::cell::UnsafeCell;
    ///
    /// let uc = UnsafeCell::new(5);
    ///
    /// unsafe { *uc.as_mut_unchecked() += 1; }
    /// assert_eq!(uc.into_inner(), 6);
    /// ```
    #[inline]
    #[unstable(feature = "unsafe_cell_access", issue = "136327")]
    #[allow(clippy::mut_from_ref)]
    #[rustc_should_not_be_called_on_const_items]
    pub const unsafe fn as_mut_unchecked(&self) -> &mut T {
        // SAFETY:该指针来自 `&self`,因此天然满足“裸指针转引用”的各项不变量。
        unsafe { self.get().as_mut_unchecked() }
    }
}

#[stable(feature = "unsafe_cell_default", since = "1.10.0")]
#[rustc_const_unstable(feature = "const_default", issue = "143894")]
impl<T: [const] Default> const Default for UnsafeCell<T> {
    /// 创建一个 `UnsafeCell`,其值为 T 的 `Default` 默认值。
    fn default() -> UnsafeCell<T> {
        UnsafeCell::new(Default::default())
    }
}

#[stable(feature = "cell_from", since = "1.12.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl<T> const From<T> for UnsafeCell<T> {
    /// 创建一个内含给定值的新 `UnsafeCell<T>`。
    fn from(t: T) -> UnsafeCell<T> {
        UnsafeCell::new(t)
    }
}

#[unstable(feature = "coerce_unsized", issue = "18598")]
impl<T: CoerceUnsized<U>, U> CoerceUnsized<UnsafeCell<U>> for UnsafeCell<T> {}

// 允许那些包裹了 `UnsafeCell` 的类型也实现 `DispatchFromDyn`,从而成为可用于动态分发的方法
// 接收者。
// 注意:目前 `UnsafeCell` 本身还不能作为方法接收者,因为它没有实现 Deref。
// 换言之:
// `self: UnsafeCell<&Self>` 不能工作
// 而 `self: UnsafeCellWrapper<Self>` 则成为可能
#[unstable(feature = "dispatch_from_dyn", issue = "none")]
impl<T: DispatchFromDyn<U>, U> DispatchFromDyn<UnsafeCell<U>> for UnsafeCell<T> {}

/// 即 [`UnsafeCell`],但实现了 [`Sync`]。
///
/// 它其实就是一个 `UnsafeCell`,区别仅在于:当 `T` 实现了 `Sync` 时,它也实现 `Sync`。
///
/// `UnsafeCell` 之所以不实现 `Sync`,是为了防止意外的误用。如果你确实有意为之,可以用
/// `SyncUnsafeCell` 取代 `UnsafeCell`,以允许它在多个线程之间共享。
/// 提供恰当的同步仍然是使用者的任务,这使得本类型与 `UnsafeCell` 一样难以安全使用。
///
/// 详情参见 [`UnsafeCell`]。
#[unstable(feature = "sync_unsafe_cell", issue = "95439")]
#[repr(transparent)]
#[rustc_diagnostic_item = "SyncUnsafeCell"]
#[rustc_pub_transparent]
pub struct SyncUnsafeCell<T: ?Sized> {
    value: UnsafeCell<T>,
}

#[unstable(feature = "sync_unsafe_cell", issue = "95439")]
unsafe impl<T: ?Sized + Sync> Sync for SyncUnsafeCell<T> {}

#[unstable(feature = "sync_unsafe_cell", issue = "95439")]
impl<T> SyncUnsafeCell<T> {
    /// 构造一个新的 `SyncUnsafeCell` 实例,用以包裹指定的值。
    #[inline]
    pub const fn new(value: T) -> Self {
        Self { value: UnsafeCell { value } }
    }

    /// 取出该值,同时消耗掉整个 cell。
    #[inline]
    #[rustc_const_unstable(feature = "sync_unsafe_cell", issue = "95439")]
    pub const fn into_inner(self) -> T {
        self.value.into_inner()
    }
}

#[unstable(feature = "sync_unsafe_cell", issue = "95439")]
impl<T: ?Sized> SyncUnsafeCell<T> {
    /// 取得一个指向被包裹值的可变指针。
    ///
    /// 它可以被转换成任意种类的指针。
    /// 当转换为 `&mut T` 时,要确保该访问是独占的(即不存在任何引用,无论可变还是不可变);
    /// 当转换为 `&T` 时,要确保此时没有任何修改或可变别名正在进行。
    #[inline]
    #[rustc_as_ptr]
    #[rustc_never_returns_null_ptr]
    #[rustc_should_not_be_called_on_const_items]
    pub const fn get(&self) -> *mut T {
        self.value.get()
    }

    /// 返回一个指向内部数据的可变引用。
    ///
    /// 本调用以可变方式借用该 `SyncUnsafeCell`(在编译期完成),从而保证我们持有的是唯一的那
    /// 一个引用。
    #[inline]
    pub const fn get_mut(&mut self) -> &mut T {
        self.value.get_mut()
    }

    /// 取得一个指向被包裹值的可变指针。
    ///
    /// 详情参见 [`UnsafeCell::get`]。
    #[inline]
    pub const fn raw_get(this: *const Self) -> *mut T {
        // 由于 SyncUnsafeCell 和 UnsafeCell 上都标了 #[repr(transparent)],我们可以直接把指针
        // 从 `SyncUnsafeCell<T>` 转成 `T`。参见 UnsafeCell::raw_get。
        this as *const T as *mut T
    }
}

#[unstable(feature = "sync_unsafe_cell", issue = "95439")]
#[rustc_const_unstable(feature = "const_default", issue = "143894")]
impl<T: [const] Default> const Default for SyncUnsafeCell<T> {
    /// 创建一个 `SyncUnsafeCell`,其值为 T 的 `Default` 默认值。
    fn default() -> SyncUnsafeCell<T> {
        SyncUnsafeCell::new(Default::default())
    }
}

#[unstable(feature = "sync_unsafe_cell", issue = "95439")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl<T> const From<T> for SyncUnsafeCell<T> {
    /// 创建一个内含给定值的新 `SyncUnsafeCell<T>`。
    fn from(t: T) -> SyncUnsafeCell<T> {
        SyncUnsafeCell::new(t)
    }
}

#[unstable(feature = "coerce_unsized", issue = "18598")]
//#[unstable(feature = "sync_unsafe_cell", issue = "95439")]
impl<T: CoerceUnsized<U>, U> CoerceUnsized<SyncUnsafeCell<U>> for SyncUnsafeCell<T> {}

// 允许那些包裹了 `SyncUnsafeCell` 的类型也实现 `DispatchFromDyn`,从而成为可用于动态分发的
// 方法接收者。
// 注意:目前 `SyncUnsafeCell` 本身还不能作为方法接收者,因为它没有实现 Deref。
// 换言之:
// `self: SyncUnsafeCell<&Self>` 不能工作
// 而 `self: SyncUnsafeCellWrapper<Self>` 则成为可能
#[unstable(feature = "dispatch_from_dyn", issue = "none")]
//#[unstable(feature = "sync_unsafe_cell", issue = "95439")]
impl<T: DispatchFromDyn<U>, U> DispatchFromDyn<SyncUnsafeCell<U>> for SyncUnsafeCell<T> {}

#[allow(unused)]
fn assert_coerce_unsized(
    a: UnsafeCell<&i32>,
    b: SyncUnsafeCell<&i32>,
    c: Cell<&i32>,
    d: RefCell<&i32>,
) {
    let _: UnsafeCell<&dyn Send> = a;
    let _: SyncUnsafeCell<&dyn Send> = b;
    let _: Cell<&dyn Send> = c;
    let _: RefCell<&dyn Send> = d;
}

#[unstable(feature = "pin_coerce_unsized_trait", issue = "150112")]
unsafe impl<T: ?Sized> PinCoerceUnsized for UnsafeCell<T> {}

#[unstable(feature = "pin_coerce_unsized_trait", issue = "150112")]
unsafe impl<T: ?Sized> PinCoerceUnsized for SyncUnsafeCell<T> {}

#[unstable(feature = "pin_coerce_unsized_trait", issue = "150112")]
unsafe impl<T: ?Sized> PinCoerceUnsized for Cell<T> {}

#[unstable(feature = "pin_coerce_unsized_trait", issue = "150112")]
unsafe impl<T: ?Sized> PinCoerceUnsized for RefCell<T> {}

#[unstable(feature = "pin_coerce_unsized_trait", issue = "150112")]
unsafe impl<'b, T: ?Sized> PinCoerceUnsized for Ref<'b, T> {}

#[unstable(feature = "pin_coerce_unsized_trait", issue = "150112")]
unsafe impl<'b, T: ?Sized> PinCoerceUnsized for RefMut<'b, T> {}
