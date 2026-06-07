//! 可重载的运算符。
//!
//! 实现这些 trait 即可重载相应的运算符。
//!
//! 其中一部分 trait 由 prelude 导入,因此在每个 Rust 程序里都可直接使用。
//! 只有背后有 trait 支撑的运算符才能被重载。例如加法运算符(`+`)可以通过
//! [`Add`] trait 重载,但赋值运算符(`=`)没有对应的 trait,因此无法重载它
//! 的语义。此外,本模块也不提供任何创建新运算符的机制。如果需要无 trait 支撑
//! 的重载或自定义运算符,应当借助宏来扩展 Rust 的语法。
//!
//! 运算符 trait 的实现应当在各自的上下文中符合直觉,牢记它们通常的含义以及
//! [运算符优先级][operator precedence]。例如实现 [`Mul`] 时,该运算应当与乘法
//! 有某种相似性(并具备人们期望的性质,如结合律)。
//!
//! 注意:`&&` 与 `||` 运算符目前不支持重载。由于它们具有短路求值的特性,需要
//! 与 [`BitAnd`] 等其他运算符 trait 不同的设计。针对它们的设计方案仍在讨论中。
//!
//! 许多运算符按值(by value)接收其操作数。在涉及内置类型的非泛型上下文中,
//! 这通常不成问题。但在泛型代码中使用这些运算符时,如果需要复用值而非让运算符
//! 消耗它们,就需要留意。一种办法是在必要时使用 [`clone`]。另一种办法是依赖相关
//! 类型为引用额外提供运算符实现。例如对于一个应当支持加法的用户自定义类型 `T`,
//! 让 `T` 与 `&T` 都实现 [`Add<T>`][`Add`] 和 [`Add<&T>`][`Add`] 通常是个好主意,
//! 这样就能写出无需多余克隆的泛型代码。
//!
//! # 示例
//!
//! 下面这个示例创建了一个实现了 [`Add`] 和 [`Sub`] 的 `Point` 结构体,然后
//! 演示对两个 `Point` 做加法和减法。
//!
//! ```rust
//! use std::ops::{Add, Sub};
//!
//! #[derive(Debug, Copy, Clone, PartialEq)]
//! struct Point {
//!     x: i32,
//!     y: i32,
//! }
//!
//! impl Add for Point {
//!     type Output = Self;
//!
//!     fn add(self, other: Self) -> Self {
//!         Self {x: self.x + other.x, y: self.y + other.y}
//!     }
//! }
//!
//! impl Sub for Point {
//!     type Output = Self;
//!
//!     fn sub(self, other: Self) -> Self {
//!         Self {x: self.x - other.x, y: self.y - other.y}
//!     }
//! }
//!
//! assert_eq!(Point {x: 3, y: 3}, Point {x: 1, y: 0} + Point {x: 2, y: 3});
//! assert_eq!(Point {x: -1, y: -3}, Point {x: 1, y: 0} - Point {x: 2, y: 3});
//! ```
//!
//! 每个 trait 的实现示例请参阅各自的文档。
//!
//! [`Fn`]、[`FnMut`] 和 [`FnOnce`] 这三个 trait 由可以像函数那样被调用的类型
//! 实现。注意 [`Fn`] 接收 `&self`,[`FnMut`] 接收 `&mut self`,而 [`FnOnce`]
//! 接收 `self`。它们对应于可以在实例上调用的三种方法:按引用调用、按可变引用
//! 调用、按值调用。这些 trait 最常见的用途是作为约束(bound),用在那些以函数
//! 或闭包作为参数的高阶函数上。
//!
//! 以 [`Fn`] 作为参数:
//!
//! ```rust
//! fn call_with_one<F>(func: F) -> usize
//!     where F: Fn(usize) -> usize
//! {
//!     func(1)
//! }
//!
//! let double = |x| x * 2;
//! assert_eq!(call_with_one(double), 2);
//! ```
//!
//! 以 [`FnMut`] 作为参数:
//!
//! ```rust
//! fn do_twice<F>(mut func: F)
//!     where F: FnMut()
//! {
//!     func();
//!     func();
//! }
//!
//! let mut x: usize = 1;
//! {
//!     let add_two_to_x = || x += 2;
//!     do_twice(add_two_to_x);
//! }
//!
//! assert_eq!(x, 5);
//! ```
//!
//! 以 [`FnOnce`] 作为参数:
//!
//! ```rust
//! fn consume_with_relish<F>(func: F)
//!     where F: FnOnce() -> String
//! {
//!     // `func` 会消耗它所捕获的变量,因此它不能被运行超过一次
//!     println!("Consumed: {}", func());
//!
//!     println!("Delicious!");
//!
//!     // 再次尝试调用 `func()` 会针对 `func` 抛出 “use of moved
//!     // value”(使用了已移动的值)错误
//! }
//!
//! let x = String::from("x");
//! let consume_and_return_x = move || x;
//! consume_with_relish(consume_and_return_x);
//!
//! // `consume_and_return_x` can no longer be invoked at this point
//! ```
//!
//! [`clone`]: Clone::clone
//! [operator precedence]: ../../reference/expressions.html#expression-precedence

#![stable(feature = "rust1", since = "1.0.0")]

mod arith;
mod async_function;
mod bit;
mod control_flow;
mod coroutine;
mod deref;
mod drop;
mod function;
mod index;
mod index_range;
mod range;
mod reborrow;
mod try_trait;
mod unsize;

#[stable(feature = "rust1", since = "1.0.0")]
pub use self::arith::{Add, Div, Mul, Neg, Rem, Sub};
#[stable(feature = "op_assign_traits", since = "1.8.0")]
pub use self::arith::{AddAssign, DivAssign, MulAssign, RemAssign, SubAssign};
#[unstable(feature = "async_fn_traits", issue = "none")]
pub use self::async_function::{AsyncFn, AsyncFnMut, AsyncFnOnce};
#[stable(feature = "rust1", since = "1.0.0")]
pub use self::bit::{BitAnd, BitOr, BitXor, Not, Shl, Shr};
#[stable(feature = "op_assign_traits", since = "1.8.0")]
pub use self::bit::{BitAndAssign, BitOrAssign, BitXorAssign, ShlAssign, ShrAssign};
#[stable(feature = "control_flow_enum_type", since = "1.55.0")]
pub use self::control_flow::ControlFlow;
#[unstable(feature = "coroutine_trait", issue = "43122")]
pub use self::coroutine::{Coroutine, CoroutineState};
#[unstable(feature = "deref_pure_trait", issue = "87121")]
pub use self::deref::DerefPure;
#[unstable(feature = "legacy_receiver_trait", issue = "none")]
pub use self::deref::LegacyReceiver;
#[unstable(feature = "arbitrary_self_types", issue = "44874")]
pub use self::deref::Receiver;
#[stable(feature = "rust1", since = "1.0.0")]
pub use self::deref::{Deref, DerefMut};
#[stable(feature = "rust1", since = "1.0.0")]
pub use self::drop::Drop;
#[stable(feature = "rust1", since = "1.0.0")]
pub use self::function::{Fn, FnMut, FnOnce};
#[stable(feature = "rust1", since = "1.0.0")]
pub use self::index::{Index, IndexMut};
pub(crate) use self::index_range::IndexRange;
#[unstable(feature = "range_into_bounds", issue = "136903")]
pub use self::range::IntoBounds;
#[stable(feature = "inclusive_range", since = "1.26.0")]
pub use self::range::{Bound, RangeBounds, RangeInclusive, RangeToInclusive};
#[unstable(feature = "one_sided_range", issue = "69780")]
pub use self::range::{OneSidedRange, OneSidedRangeBound};
#[stable(feature = "rust1", since = "1.0.0")]
pub use self::range::{Range, RangeFrom, RangeFull, RangeTo};
#[unstable(feature = "reborrow", issue = "145612")]
pub use self::reborrow::{CoerceShared, Reborrow};
#[unstable(feature = "try_trait_v2_residual", issue = "91285")]
pub use self::try_trait::Residual;
#[unstable(feature = "try_trait_v2_yeet", issue = "96374")]
pub use self::try_trait::Yeet;
pub(crate) use self::try_trait::{ChangeOutputType, NeverShortCircuit};
#[unstable(feature = "try_trait_v2", issue = "84277", old_name = "try_trait")]
pub use self::try_trait::{FromResidual, Try};
#[unstable(feature = "coerce_unsized", issue = "18598")]
pub use self::unsize::CoerceUnsized;
#[unstable(feature = "dispatch_from_dyn", issue = "none")]
pub use self::unsize::DispatchFromDyn;
