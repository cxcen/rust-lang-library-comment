//! 可组合的外部迭代。
//!
//! 当某个集合需要逐个元素执行操作时，Rust 通常把这个过程表达为“迭代器”。
//! 迭代器把“下一项是什么、何时结束、如何组合处理步骤”抽象成统一协议，
//! 因而大量惯用 Rust 代码都会依赖它。理解本模块的 trait 契约，尤其是
//! [`Iterator::next`]、[`Iterator::size_hint`] 以及若干 unsafe 标记 trait，
//! 对实现自定义集合和适配器都很重要。
//!
//! 继续说明之前，先看本模块如何组织:
//!
//! # 组织结构
//!
//! 本模块大体按类型类别组织:
//!
//! * [Traits] 是核心部分: 这些 trait 定义了迭代器有哪些能力，以及调用方
//!   可以依赖哪些语义。多数默认方法都建立在 [`Iterator::next`] 的最小契约上。
//! * [Functions] 提供一些创建基础迭代器的辅助入口。
//! * [Structs] 通常是各个 trait 方法返回的迭代器适配器类型。读文档时，
//!   一般应先看创建该 `struct` 的方法，因为方法会说明它如何消耗、延迟执行
//!   或改变底层迭代器。更多原因见“[实现 Iterator](#implementing-iterator)”。
//!
//! [Traits]: #traits
//! [Functions]: #functions
//! [Structs]: #structs
//!
//! 下面进入迭代器本身。
//!
//! # Iterator
//!
//! 本模块的核心是 [`Iterator`] trait。它的最小形态如下:
//!
//! ```
//! trait Iterator {
//!     type Item;
//!     fn next(&mut self) -> Option<Self::Item>;
//! }
//! ```
//!
//! 迭代器只有一个必须实现的方法: [`next`]。每次调用 [`next`] 都会推进迭代器的
//! 内部状态，并返回 <code>[Option]\<Item></code>。仍有元素时返回 [`Some(Item)`]；
//! 当前迭代序列结束时返回 `None`。需要特别注意的是，普通 [`Iterator`] 并不承诺
//! “第一次 `None` 之后永远是 `None`”: 某些实现可以在之后再次返回 [`Some(Item)`]
//! (例如 [`TryIter`])。如果调用方或适配器需要永久结束的保证，应使用
//! [`FusedIterator`] 或显式调用 [`Iterator::fuse`]。
//!
//! [`Iterator`] 的完整定义还包含许多默认方法。多数默认方法都通过反复调用
//! [`next`] 来实现，所以只要实现了 [`next`]，通常就能获得这些组合能力。
//! 迭代器也天然可组合: 常见写法会把多个适配器串联起来，形成复杂但仍按需执行的
//! 处理流水线。更多细节见下面的[适配器](#adapters)小节。
//!
//! [`Some(Item)`]: Some
//! [`next`]: Iterator::next
//! [`TryIter`]: ../../std/sync/mpsc/struct.TryIter.html
//!
//! # 三种常见迭代形式
//!
//! 集合通常通过三类方法创建迭代器:
//!
//! * `iter()`: 产生 `&T`，只借用集合元素。
//! * `iter_mut()`: 产生 `&mut T`，允许逐项修改元素。
//! * `into_iter()`: 产生 `T`，按值消耗集合或值本身。
//!
//! 标准库中的类型会按自身所有权和别名规则，选择实现其中一种或多种形式。
//!
//! # 实现 Iterator {#implementing-iterator}
//!
//! 自定义迭代器通常分两步: 先创建一个保存迭代状态的 `struct`，再为这个
//! `struct` 实现 [`Iterator`]。这也是本模块中有大量 `struct` 的原因:
//! 每个迭代器源或适配器都需要保存自己的状态，例如当前位置、剩余长度、缓存项等。
//!
//! 下面实现一个从 `1` 数到 `5` 的 `Counter`:
//!
//! ```
//! // 首先定义保存状态的 struct:
//!
//! /// 一个从一数到五的迭代器
//! struct Counter {
//!     count: usize,
//! }
//!
//! // 我们希望产出的第一个值是 1，所以提供一个 new() 构造函数。
//! // 这不是严格必需的，但更方便。注意这里把 `count` 初始化为 0，
//! // 下面的 `next()` 会先递增再判断。
//! impl Counter {
//!     fn new() -> Counter {
//!         Counter { count: 0 }
//!     }
//! }
//!
//! // 然后为 `Counter` 实现 `Iterator`:
//!
//! impl Iterator for Counter {
//!     // 产出的计数值使用 usize
//!     type Item = usize;
//!
//!     // next() 是唯一必须实现的方法
//!     fn next(&mut self) -> Option<Self::Item> {
//!         // 先推进内部状态，这也是初始化为 0 的原因。
//!         self.count += 1;
//!
//!         // 根据推进后的状态决定是否还有元素。
//!         if self.count < 6 {
//!             Some(self.count)
//!         } else {
//!             None
//!         }
//!     }
//! }
//!
//! // 现在可以使用它。
//!
//! let mut counter = Counter::new();
//!
//! assert_eq!(counter.next(), Some(1));
//! assert_eq!(counter.next(), Some(2));
//! assert_eq!(counter.next(), Some(3));
//! assert_eq!(counter.next(), Some(4));
//! assert_eq!(counter.next(), Some(5));
//! assert_eq!(counter.next(), None);
//! ```
//!
//! 像这样手动反复调用 [`next`] 很快会变得冗长。Rust 提供了 `for` 循环语法，
//! 它会自动调用 [`next`]，直到看到 `None` 为止。
//!
//! 还要注意，`Iterator` 为 `nth`、`fold` 等方法提供了默认实现，这些实现通常
//! 在内部调用 `next`。如果某个迭代器可以不逐项调用 `next` 就更高效地完成同一件事，
//! 它也可以覆盖这些方法；覆盖后仍必须维护与逐项迭代一致的外部语义。
//!
//! # `for` 循环与 `IntoIterator`
//!
//! Rust 的 `for` 循环语法本质上是迭代器协议的语法糖。下面是一个基本例子:
//!
//! ```
//! let values = vec![1, 2, 3, 4, 5];
//!
//! for x in values {
//!     println!("{x}");
//! }
//! ```
//!
//! 这会逐行打印一到五。这里看起来没有显式地把 `Vec` 转成迭代器，是因为
//! `for` 会使用 [`IntoIterator`]。该 trait 通过 [`into_iter`] 把实现者转换为
//! 一个迭代器。上面的 `for` 循环会被编译器近似展开为:
//!
//! [`into_iter`]: IntoIterator::into_iter
//!
//! ```
//! let values = vec![1, 2, 3, 4, 5];
//!
//! for x in values {
//!     println!("{x}");
//! }
//! ```
//!
//! Rust 会把它脱糖为类似如下结构:
//!
//! ```
//! let values = vec![1, 2, 3, 4, 5];
//! {
//!     let result = match IntoIterator::into_iter(values) {
//!         mut iter => loop {
//!             let next;
//!             match iter.next() {
//!                 Some(val) => next = val,
//!                 None => break,
//!             };
//!             let x = next;
//!             let () = { println!("{x}"); };
//!         },
//!     };
//!     result
//! }
//! ```
//!
//! 也就是说，先对值调用 `into_iter()`，再持有返回的迭代器并反复调用 [`next`]。
//! 每次得到 `Some(val)` 就执行循环体；一旦得到 `None` 就 `break`，迭代结束。
//!
//! 这里还有一个重要细节: 标准库为所有 [`Iterator`] 都实现了 [`IntoIterator`]:
//!
//! ```ignore (only-for-syntax-highlight)
//! impl<I: Iterator> IntoIterator for I
//! ```
//!
//! 换句话说，迭代器本身也可以被当作“可转换成迭代器的值”，转换结果就是它自己。
//! 这带来两个直接后果:
//!
//! 1. 只要实现了 [`Iterator`]，该类型就可以直接用于 `for` 循环。
//! 2. 如果正在实现集合类型，为集合实现 [`IntoIterator`] 就能让它支持 `for` 循环。
//!
//! # 按引用迭代
//!
//! 因为 [`into_iter()`] 按值接收 `self`，直接对集合使用 `for` 循环通常会消耗集合。
//! 很多场景只想借用集合而不是取得所有权，因此许多集合会提供按约定命名的
//! `iter()` 和 `iter_mut()`，分别产生共享引用和可变引用:
//!
//! ```
//! let mut values = vec![41];
//! for x in values.iter_mut() {
//!     *x += 1;
//! }
//! for x in values.iter() {
//!     assert_eq!(*x, 42);
//! }
//! assert_eq!(values.len(), 1); // `values` 仍由当前函数拥有。
//! ```
//!
//! 如果集合类型 `C` 提供 `iter()`，它通常也会为 `&C` 实现 `IntoIterator`，
//! 并在实现中委托给 `iter()`。同理，提供 `iter_mut()` 的集合通常会为 `&mut C`
//! 实现 `IntoIterator`，并委托给 `iter_mut()`。这样就能写出更简洁的形式:
//!
//! ```
//! let mut values = vec![41];
//! for x in &mut values {
//!     //   ^ 等同于 `values.iter_mut()`
//!     *x += 1;
//! }
//! for x in &values {
//!     //   ^ 等同于 `values.iter()`
//!     assert_eq!(*x, 42);
//! }
//! assert_eq!(values.len(), 1);
//! ```
//!
//! 许多集合都提供 `iter()`，但不是所有集合都能安全提供 `iter_mut()`。例如，
//! 如果允许修改 [`HashSet<T>`] 中的键，键的 hash 发生变化后集合内部桶位置可能
//! 不再满足不变量，因此该集合只提供 `iter()`。
//!
//! [`into_iter()`]: IntoIterator::into_iter
//! [`HashSet<T>`]: ../../std/collections/struct.HashSet.html
//!
//! # 适配器 {#adapters}
//!
//! 接收一个 [`Iterator`] 并返回另一个 [`Iterator`] 的函数通常称为“迭代器适配器”。
//! 它们把处理逻辑包装成新的迭代器节点，而不是立即执行处理。
//!
//! 常见适配器包括 [`map`]、[`take`] 和 [`filter`]。更多语义见各自文档。
//!
//! 如果迭代器适配器发生 panic，迭代器会处于未指定但仍内存安全的状态。这个状态
//! 也不保证跨 Rust 版本保持一致，因此不要依赖 panic 后继续迭代得到的具体值。
//!
//! [`map`]: Iterator::map
//! [`take`]: Iterator::take
//! [`filter`]: Iterator::filter
//!
//! # 惰性求值
//!
//! 迭代器以及迭代器[适配器](#adapters)都是惰性的。这意味着“创建迭代器”通常只
//! 构造一个描述计算的值，并不会遍历元素、调用闭包或执行副作用；真正推进发生在
//! 调用 [`next`]、`for` 循环、[`collect`]、[`for_each`] 等消费操作时。
//! 如果只为了副作用创建迭代器，这一点很容易造成误解。例如 [`map`] 会在每个被
//! 实际取出的元素上调用闭包:
//!
//! ```
//! # #![allow(unused_must_use)]
//! # #![allow(map_unit_fn)]
//! let v = vec![1, 2, 3, 4, 5];
//! v.iter().map(|x| println!("{x}"));
//! ```
//!
//! 这不会打印任何值，因为代码只创建了一个迭代器，没有消费它。编译器会对这类
//! 行为发出警告:
//!
//! ```text
//! warning: unused result that must be used: iterators are lazy and
//! do nothing unless consumed
//! ```
//!
//! 如果目的是执行副作用，惯用写法是使用 `for` 循环，或者调用 [`for_each`]:
//!
//! ```
//! let v = vec![1, 2, 3, 4, 5];
//!
//! v.iter().for_each(|x| println!("{x}"));
//! // 或者
//! for x in &v {
//!     println!("{x}");
//! }
//! ```
//!
//! [`map`]: Iterator::map
//! [`for_each`]: Iterator::for_each
//!
//! 另一个常见消费方式是调用 [`collect`]，把迭代器中的元素收集成新的集合。
//!
//! [`collect`]: Iterator::collect
//!
//! # 无限迭代器
//!
//! 迭代器不要求有限。例如，开放区间是一个无限迭代器:
//!
//! ```
//! let numbers = 0..;
//! ```
//!
//! 常见做法是使用 [`take`] 适配器，把无限迭代器限制成有限迭代器:
//!
//! ```
//! let numbers = 0..;
//! let five_numbers = numbers.take(5);
//!
//! for number in five_numbers {
//!     println!("{number}");
//! }
//! ```
//!
//! 这会逐行打印 `0` 到 `4`。
//!
//! 需要记住，对无限迭代器调用某些方法可能永不终止。即使某个结果在数学上似乎可
//! 以有限时间确定，通用迭代器方法仍可能需要遍历所有元素。例如 [`min`] 在一般情
//! 况下必须检查每个元素，因此对无限迭代器很可能无法返回。
//!
//! ```no_run
//! let ones = std::iter::repeat(1);
//! let least = ones.min().unwrap(); // 糟糕: 这里会进入无限循环。
//! // `ones.min()` 不会结束，因此不会执行到这里。
//! println!("The smallest number one is {least}.");
//! ```
//!
//! [`take`]: Iterator::take
//! [`min`]: Iterator::min

#![stable(feature = "rust1", since = "1.0.0")]

// 需要放在这里，子模块才能使用。
macro_rules! impl_fold_via_try_fold {
    (fold -> try_fold) => {
        impl_fold_via_try_fold! { @internal fold -> try_fold }
    };
    (rfold -> try_rfold) => {
        impl_fold_via_try_fold! { @internal rfold -> try_rfold }
    };
    (spec_fold -> spec_try_fold) => {
        impl_fold_via_try_fold! { @internal spec_fold -> spec_try_fold }
    };
    (spec_rfold -> spec_try_rfold) => {
        impl_fold_via_try_fold! { @internal spec_rfold -> spec_try_rfold }
    };
    (@internal $fold:ident -> $try_fold:ident) => {
        #[inline]
        fn $fold<AAA, FFF>(mut self, init: AAA, fold: FFF) -> AAA
        where
            FFF: FnMut(AAA, Self::Item) -> AAA,
        {
            use crate::ops::NeverShortCircuit;

            self.$try_fold(init, NeverShortCircuit::wrap_mut_2(fold)).0
        }
    };
}

#[unstable(feature = "iter_array_chunks", issue = "100450")]
pub use self::adapters::ArrayChunks;
#[unstable(feature = "std_internals", issue = "none")]
pub use self::adapters::ByRefSized;
#[stable(feature = "iter_cloned", since = "1.1.0")]
pub use self::adapters::Cloned;
#[stable(feature = "iter_copied", since = "1.36.0")]
pub use self::adapters::Copied;
#[stable(feature = "iterator_flatten", since = "1.29.0")]
pub use self::adapters::Flatten;
#[stable(feature = "iter_map_while", since = "1.57.0")]
pub use self::adapters::MapWhile;
#[unstable(feature = "iter_map_windows", issue = "87155")]
pub use self::adapters::MapWindows;
#[unstable(feature = "inplace_iteration", issue = "none")]
pub use self::adapters::SourceIter;
#[stable(feature = "iterator_step_by", since = "1.28.0")]
pub use self::adapters::StepBy;
#[unstable(feature = "trusted_random_access", issue = "none")]
pub use self::adapters::TrustedRandomAccess;
#[unstable(feature = "trusted_random_access", issue = "none")]
pub use self::adapters::TrustedRandomAccessNoCoerce;
#[stable(feature = "iter_chain", since = "1.91.0")]
pub use self::adapters::chain;
pub(crate) use self::adapters::try_process;
#[stable(feature = "iter_zip", since = "1.59.0")]
pub use self::adapters::zip;
#[stable(feature = "rust1", since = "1.0.0")]
pub use self::adapters::{
    Chain, Cycle, Enumerate, Filter, FilterMap, FlatMap, Fuse, Inspect, Map, Peekable, Rev, Scan,
    Skip, SkipWhile, Take, TakeWhile, Zip,
};
#[unstable(feature = "iter_intersperse", issue = "79524")]
pub use self::adapters::{Intersperse, IntersperseWith};
#[unstable(
    feature = "step_trait",
    reason = "likely to be replaced by finer-grained traits",
    issue = "42168"
)]
pub use self::range::Step;
#[unstable(feature = "iter_macro", issue = "142269", reason = "generators are unstable")]
pub use self::sources::iter;
#[stable(feature = "iter_empty", since = "1.2.0")]
pub use self::sources::{Empty, empty};
#[unstable(
    feature = "iter_from_coroutine",
    issue = "43122",
    reason = "coroutines are unstable"
)]
pub use self::sources::{FromCoroutine, from_coroutine};
#[stable(feature = "iter_from_fn", since = "1.34.0")]
pub use self::sources::{FromFn, from_fn};
#[stable(feature = "iter_once", since = "1.2.0")]
pub use self::sources::{Once, once};
#[stable(feature = "iter_once_with", since = "1.43.0")]
pub use self::sources::{OnceWith, once_with};
#[stable(feature = "rust1", since = "1.0.0")]
pub use self::sources::{Repeat, repeat};
#[stable(feature = "iter_repeat_n", since = "1.82.0")]
pub use self::sources::{RepeatN, repeat_n};
#[stable(feature = "iterator_repeat_with", since = "1.28.0")]
pub use self::sources::{RepeatWith, repeat_with};
#[stable(feature = "iter_successors", since = "1.34.0")]
pub use self::sources::{Successors, successors};
#[stable(feature = "fused", since = "1.26.0")]
pub use self::traits::FusedIterator;
#[unstable(issue = "none", feature = "inplace_iteration")]
pub use self::traits::InPlaceIterable;
#[stable(feature = "rust1", since = "1.0.0")]
pub use self::traits::Iterator;
#[unstable(issue = "none", feature = "trusted_fused")]
pub use self::traits::TrustedFused;
#[unstable(feature = "trusted_len", issue = "37572")]
pub use self::traits::TrustedLen;
#[unstable(feature = "trusted_step", issue = "85731")]
pub use self::traits::TrustedStep;
pub(crate) use self::traits::UncheckedIterator;
#[stable(feature = "rust1", since = "1.0.0")]
pub use self::traits::{
    DoubleEndedIterator, ExactSizeIterator, Extend, FromIterator, IntoIterator, Product, Sum,
};

mod adapters;
mod range;
mod sources;
mod traits;
