//! 集合类型（Collection types）。
//!
//! Rust 的标准集合库为最常见的通用编程数据结构提供了高效的实现。通过使用这些
//! 标准实现，两个库之间应当能够在不需要大量数据转换的情况下相互通信。
//!
//! 先把话说在前头：你大概率只需要用 [`Vec`] 或 [`HashMap`] 就够了。这两个集合
//! 覆盖了通用数据存储与处理的大多数使用场景。它们在各自擅长的事情上表现极佳。
//! 标准库中所有其他集合都有各自最优的特定使用场景，但相比之下，这些场景已近乎
//! *小众*（*niche*）。即便在 `Vec` 和 `HashMap` 技术上并非最优的情形下，用它们
//! 起步通常也已经是足够好的选择。
//!
//! Rust 的集合可以归为四大类：
//!
//! * 序列（Sequences）：[`Vec`]、[`VecDeque`]、[`LinkedList`]
//! * 映射（Maps）：[`HashMap`]、[`BTreeMap`]
//! * 集合（Sets）：[`HashSet`]、[`BTreeSet`]
//! * 杂项（Misc）：[`BinaryHeap`]
//!
//! # 该在何时使用哪种集合？
//!
//! 以下是关于何时应当考虑每种集合的、相当高层而快速的归纳。关于各个集合优缺点的
//! 详细讨论，可在它们各自的文档页面找到。
//!
//! ### 在以下情况使用 [`Vec`]：
//! * 你想把一些元素收集起来，以便稍后处理或发送到别处，并且不关心所存储的实际值
//!   的任何性质。
//! * 你想要一个按特定顺序排列的元素序列，并且只会在（或靠近）末尾进行追加。
//! * 你想要一个栈（stack）。
//! * 你想要一个可变长数组（resizable array）。
//! * 你想要一个分配在堆上（heap-allocated）的数组。
//!
//! ### 在以下情况使用 [`VecDeque`]：
//! * 你想要一个支持在序列两端高效插入的 [`Vec`]。
//! * 你想要一个队列（queue）。
//! * 你想要一个双端队列（double-ended queue，deque）。
//!
//! ### 在以下情况使用 [`LinkedList`]：
//! * 你想要一个大小未知的 [`Vec`] 或 [`VecDeque`]，并且无法容忍摊还
//!   （amortization）带来的开销波动。
//! * 你想要高效地分割（split）和拼接（append）链表。
//! * 你 *绝对* 确定你 *真的*、*确确实实* 想要一个双向链表（doubly linked
//!   list）。
//!
//! ### 在以下情况使用 [`HashMap`]：
//! * 你想把任意的 key 关联到任意的 value。
//! * 你想要一个缓存（cache）。
//! * 你想要一个映射（map），不需要任何额外功能。
//!
//! ### 在以下情况使用 [`BTreeMap`]：
//! * 你想要一个按其 key 排序的映射。
//! * 你想要能够按需获取某一范围（range）的条目（entries）。
//! * 你关心的是最小或最大的键值对（key-value pair）是什么。
//! * 你想找出比某个值更小或更大的、最大或最小的 key。
//!
//! ### 在以下情况使用上述任意 `Map` 的 `Set` 变体：
//! * 你只是想记住自己见过哪些 key。
//! * 没有什么有意义的 value 需要关联到你的 key 上。
//! * 你只是想要一个集合（set）。
//!
//! ### 在以下情况使用 [`BinaryHeap`]：
//!
//! * 你想存储一堆元素，但在任意给定时刻只想处理其中“最大”或“最重要”的那一个。
//! * 你想要一个优先级队列（priority queue）。
//!
//! # 性能（Performance）
//!
//! 为工作选对集合，需要理解每种集合各自擅长什么。这里我们简要总结不同集合在若干
//! 重要操作上的性能。更多细节请参见每个类型的文档，并请注意：在某些集合上，实际
//! 方法的名称可能与下表有所不同。
//!
//! 在整篇文档中，我们对操作的复杂度记号遵循以下约定：
//!
//! * 集合的大小记为 `n`。
//! * 若涉及第二个集合，其大小记为 `m`。
//! * 元素的索引记为 `i`。
//! * 具有 *摊还*（*amortized*）开销的操作以 `*` 作后缀。
//! * 具有 *期望*（*expected*）开销的操作以 `~` 作后缀。
//!
//! 调用向集合添加元素的操作时，偶尔会需要对集合进行扩容（resize）——这是一个
//! 额外的、耗时 *O*(*n*) 的操作。
//!
//! *摊还*（*Amortized*）开销是把这类扩容操作的时间成本 *在足够长的一系列操作上*
//! 进行核算后得到的。由于集合扩容具有零星发生的性质，单次操作可能更慢也可能更快，
//! 但每次操作的平均开销会趋近于该摊还开销。
//!
//! Rust 的集合从不自动缩容（shrink），所以移除（removal）操作不涉及摊还。
//!
//! [`HashMap`] 使用 *期望*（*expected*）开销。理论上，[`HashMap`] 出现显著差于
//! 期望开销的性能是可能的，尽管非常不太可能。这是由 hash 的概率性本质所致——
//! 也就是说，对于某些输入 key，有可能生成重复的 hash，从而需要额外的计算来纠正。
//!
//! ## 集合操作的开销（Cost of Collection Operations）
//!
//!
//! |                | get(i)                 | insert(i)               | remove(i)              | append(Vec(m))    | split_off(i)           | range           | append       |
//! |----------------|------------------------|-------------------------|------------------------|-------------------|------------------------|-----------------|--------------|
//! | [`Vec`]        | *O*(1)                 | *O*(*n*-*i*)*           | *O*(*n*-*i*)           | *O*(*m*)*         | *O*(*n*-*i*)           | N/A             | N/A          |
//! | [`VecDeque`]   | *O*(1)                 | *O*(min(*i*, *n*-*i*))* | *O*(min(*i*, *n*-*i*)) | *O*(*m*)*         | *O*(min(*i*, *n*-*i*)) | N/A             | N/A          |
//! | [`LinkedList`] | *O*(min(*i*, *n*-*i*)) | *O*(min(*i*, *n*-*i*))  | *O*(min(*i*, *n*-*i*)) | *O*(1)            | *O*(min(*i*, *n*-*i*)) | N/A             | N/A          |
//! | [`HashMap`]    | *O*(1)~                | *O*(1)~*                | *O*(1)~                | N/A               | N/A                    | N/A             | N/A          |
//! | [`BTreeMap`]   | *O*(log(*n*))          | *O*(log(*n*))           | *O*(log(*n*))          | N/A               | N/A                    | *O*(log(*n*))   | *O*(*n*+*m*) |
//!
//! 注意：在出现并列（ties）的情况下，[`Vec`] 通常会比 [`VecDeque`] 更快，而
//! [`VecDeque`] 通常会比 [`LinkedList`] 更快。
//!
//! 对于各 Set，所有操作的开销与其对应的 Map 操作相同。
//!
//! # 正确而高效地使用集合（Correct and Efficient Usage of Collections）
//!
//! 当然，知道哪个集合最适合手头的工作，并不会让你立刻就能正确地使用它。这里给出
//! 一些关于如何在总体上高效且正确地使用标准集合的快速提示。如果你想了解如何使用
//! 某个特定集合，请查阅它的文档，那里有详细的讨论和代码示例。
//!
//! ## 容量管理（Capacity Management）
//!
//! 许多集合都提供了若干涉及“容量”（capacity）的构造器和方法。这些集合一般构建
//! 于一个数组之上。最理想的情况下，这个数组的大小会恰好只够容纳集合中存储的元素，
//! 但要让集合做到这一点会非常低效。如果背后的数组在任何时刻都恰好是正确的大小，
//! 那么每次插入一个元素时，集合都不得不增长该数组以容纳它。由于大多数计算机分配
//! 和管理内存的方式，这几乎必然需要分配一个全新的数组，并把旧数组中的每一个元素
//! 都拷贝到新数组里。希望你能看出，在每次操作时都这样做并不会很高效。
//!
//! 因此，大多数集合采用一种 *摊还*（*amortized*）的分配策略。它们通常会让自己
//! 留有相当数量的空闲空间，这样就只需偶尔增长。当它们确实要增长时，会分配一个
//! 大得多的数组来移入元素，从而要过相当一段时间才会再次需要增长。这种策略总体上
//! 很不错，但如果集合 *从不* 需要对其背后的数组进行扩容，那就更好了。遗憾的是，
//! 集合本身没有足够的信息来自行做到这一点。因此，这就要靠我们这些程序员来给它
//! 提示了。
//!
//! 任何 `with_capacity` 构造器都会指示集合分配足以容纳指定数量元素的空间。理想
//! 情况下，这恰好就是那么多个元素的空间，但某些实现细节可能会妨碍这一点。详情参见
//! 各集合各自的文档。总的来说，当你确切知道将要插入多少个元素，或至少对该数量有一个
//! 合理的上界（upper-bound）时，就使用 `with_capacity`。
//!
//! 当预计会有大量元素涌入时，可以使用 `reserve` 系列方法来向集合提示它应当为即将
//! 到来的元素腾出多少空间。与 `with_capacity` 一样，这些方法的确切行为也将取决于
//! 你所关注的具体集合。
//!
//! 为获得最佳性能，集合通常会避免缩容自身。如果你认为某个集合很快就不会再包含更多
//! 元素，或者你确实急需那部分内存，`shrink_to_fit` 方法会促使集合把背后的数组缩小
//! 到能够容纳其元素的最小尺寸。
//!
//! 最后，如果你想知道集合实际的容量是多少，大多数集合都提供了一个 `capacity` 方法，
//! 可以按需查询这一信息。这对于调试，或者配合 `reserve` 系列方法使用，都会很有用。
//!
//! ## 迭代器（Iterators）
//!
//! [迭代器（Iterators）][crate::iter]
//! 是一种贯穿 Rust 标准库始终使用的、强大而健壮的机制。迭代器以通用、安全、高效
//! 且便捷的方式提供一个值的序列。迭代器的内容通常是 *惰性*（*lazily*）求值的，
//! 因此只有真正需要的值才会被实际产出，也无需为临时存储它们而进行分配。迭代器主要
//! 通过 `for` 循环来消费，不过许多函数在需要一个集合或值序列时也接受迭代器。
//!
//! 所有标准集合都提供了若干迭代器，用于对其内容进行批量操作。几乎每个集合都应当
//! 提供的三个主要迭代器是 `iter`、`iter_mut` 和 `into_iter`。在那些提供这些迭代器
//! 会不健全（unsound）或不合理的集合上，其中某些迭代器不会被提供。
//!
//! `iter` 以最“自然”的顺序提供一个对集合全部内容的不可变引用的迭代器。对于像
//! [`Vec`] 这样的序列集合，这意味着元素将从索引 0 开始按索引递增的顺序产出。对于
//! 像 [`BTreeMap`] 这样的有序集合，这意味着元素将按排序顺序产出。对于像 [`HashMap`]
//! 这样的无序集合，元素将以其内部表示最为便利的任意顺序产出。这非常适合用来遍历
//! 读取集合的全部内容。
//!
//! ```
//! let vec = vec![1, 2, 3, 4];
//! for x in vec.iter() {
//!    println!("vec contained {x:?}");
//! }
//! ```
//!
//! `iter_mut` 以与 `iter` 相同的顺序提供一个 *可变*（*mutable*）引用的迭代器。
//! 这非常适合用来修改集合的全部内容。
//!
//! ```
//! let mut vec = vec![1, 2, 3, 4];
//! for x in vec.iter_mut() {
//!    *x += 1;
//! }
//! ```
//!
//! `into_iter` 把实际的集合转换为一个按值（by-value）遍历其内容的迭代器。当集合
//! 本身不再被需要、而其中的值在别处需要用到时，这非常合适。将 `extend` 与
//! `into_iter` 配合使用，是把一个集合的内容移动到另一个集合中的主要方式。`extend`
//! 会自动调用 `into_iter`，并接受任意 <code>T: [IntoIterator]</code>。在一个迭代器
//! 自身上调用 `collect` 也是把一个集合转换为另一个集合的好办法。这两个方法在内部
//! 都应当使用上一节讨论过的容量管理工具，以尽可能高效地完成转换。
//!
//! ```
//! let mut vec1 = vec![1, 2, 3, 4];
//! let vec2 = vec![10, 20, 30, 40];
//! vec1.extend(vec2);
//! ```
//!
//! ```
//! use std::collections::VecDeque;
//!
//! let vec = [1, 2, 3, 4];
//! let buf: VecDeque<_> = vec.into_iter().collect();
//! ```
//!
//! 迭代器还提供了一系列 *适配器*（*adapter*）方法，用于对序列执行常见的处理。
//! 这些适配器中有诸如 `map`、`fold`、`skip`、`take` 这样的函数式编程的经典之选。
//! 对集合而言特别值得一提的是 `rev` 适配器，它会反转任何支持该操作的迭代器。大多数
//! 集合都把可反转的迭代器作为以逆序遍历它们的方式来提供。
//!
//! ```
//! let vec = vec![1, 2, 3, 4];
//! for x in vec.iter().rev() {
//!    println!("vec contained {x:?}");
//! }
//! ```
//!
//! 另外还有若干集合方法也返回迭代器，以产出一个结果序列，但又避免为存储结果而分配
//! 一整个集合。这提供了最大的灵活性，因为如果需要，可以调用
//! [`collect`][crate::iter::Iterator::collect] 或
//! [`extend`][crate::iter::Extend::extend] 来把该序列“管道式”（pipe）地导入任意
//! 集合。否则，也可以用 `for` 循环遍历该序列。该迭代器还可以在部分使用之后被丢弃，
//! 从而避免对未用到的元素进行计算。
//!
//! ## 条目（Entries）
//!
//! `entry` API 旨在提供一种高效的机制，使得能够根据某个 key 是否存在，有条件地
//! 操作映射的内容。其主要的设计动机用例是提供高效的累加器（accumulator）映射。
//! 例如，如果有人希望维护每个 key 被看到的次数的计数，他们就不得不就“这是否是
//! 第一次看到该 key”执行一些条件逻辑。通常，这会需要一次 `find` 后跟一次 `insert`，
//! 实际上在每次插入时都把查找工作重复了一遍。
//!
//! 当用户调用 `map.entry(key)` 时，映射会搜索该 key，然后产出 `Entry` 枚举的一个
//! 变体。
//!
//! 如果产出的是 `Vacant(entry)`，那么该 key *未* 被找到。在这种情况下，唯一有效的
//! 操作是向该 entry `insert` 一个 value。完成后，这个空置（vacant）的 entry 会被
//! 消耗掉，并转换为一个指向所插入 value 的可变引用。这使得可以在搜索本身的生命周期
//! 之外对该 value 做进一步操作。当不论 value 是否刚刚被插入、都需要对其执行复杂逻辑
//! 时，这会很有用。
//!
//! 如果产出的是 `Occupied(entry)`，那么该 key *被* 找到了。在这种情况下，用户有
//! 几种选择：他们可以对这个已占用（occupied）的 entry 进行 `get`、`insert` 或
//! `remove`。此外，他们还可以把这个已占用的 entry 转换为一个指向其 value 的可变
//! 引用，从而与空置情形下的 `insert` 形成对称。
//!
//! ### 示例
//!
//! 下面是 `entry` 的两种主要用法。首先是一个简单的例子，其中对 value 执行的逻辑
//! 很平凡。
//!
//! #### 统计字符串中每个字符出现的次数
//!
//! ```
//! use std::collections::btree_map::BTreeMap;
//!
//! let mut count = BTreeMap::new();
//! let message = "she sells sea shells by the sea shore";
//!
//! for c in message.chars() {
//!     *count.entry(c).or_insert(0) += 1;
//! }
//!
//! assert_eq!(count.get(&'s'), Some(&8));
//!
//! println!("Number of occurrences of each character");
//! for (char, count) in &count {
//!     println!("{char}: {count}");
//! }
//! ```
//!
//! 当要对 value 执行的逻辑更复杂时，我们可以简单地用 `entry` API 来确保 value
//! 已被初始化，然后再去执行后续逻辑。
//!
//! #### 追踪酒吧顾客的醉酒程度
//!
//! ```
//! use std::collections::btree_map::BTreeMap;
//!
//! // 酒吧的一位客人。他们有一个血液酒精浓度。
//! struct Person { blood_alcohol: f32 }
//!
//! // 向酒吧下的所有订单，按客户 ID 记录。
//! let orders = vec![1, 2, 1, 2, 3, 4, 1, 2, 2, 3, 4, 1, 1, 1];
//!
//! // 我们的客户。
//! let mut blood_alcohol = BTreeMap::new();
//!
//! for id in orders {
//!     // 如果这是我们第一次见到这位顾客，就把他们初始化为没有血液酒精。
//!     // 否则，直接取出他们。
//!     let person = blood_alcohol.entry(id).or_insert(Person { blood_alcohol: 0.0 });
//!
//!     // 降低他们的血液酒精浓度。点单并喝完一杯啤酒是需要时间的！
//!     person.blood_alcohol *= 0.9;
//!
//!     // 检查他们是否清醒到可以再来一杯。
//!     if person.blood_alcohol > 0.3 {
//!         // 太醉了……至少现在是。
//!         println!("Sorry {id}, I have to cut you off");
//!     } else {
//!         // 再来一杯！
//!         person.blood_alcohol += 0.1;
//!     }
//! }
//! ```
//!
//! # 插入与复杂的 key（Insert and complex keys）
//!
//! 如果我们有一个更复杂的 key，对 `insert` 的调用将 *不会* 更新该 key 的取值。
//! 例如：
//!
//! ```
//! use std::cmp::Ordering;
//! use std::collections::BTreeMap;
//! use std::hash::{Hash, Hasher};
//!
//! #[derive(Debug)]
//! struct Foo {
//!     a: u32,
//!     b: &'static str,
//! }
//!
//! // 我们将仅按 `Foo` 的 `a` 值来比较 `Foo`。
//! impl PartialEq for Foo {
//!     fn eq(&self, other: &Self) -> bool { self.a == other.a }
//! }
//!
//! impl Eq for Foo {}
//!
//! // 我们将仅按 `Foo` 的 `a` 值来对 `Foo` 进行 hash。
//! impl Hash for Foo {
//!     fn hash<H: Hasher>(&self, h: &mut H) { self.a.hash(h); }
//! }
//!
//! impl PartialOrd for Foo {
//!     fn partial_cmp(&self, other: &Self) -> Option<Ordering> { self.a.partial_cmp(&other.a) }
//! }
//!
//! impl Ord for Foo {
//!     fn cmp(&self, other: &Self) -> Ordering { self.a.cmp(&other.a) }
//! }
//!
//! let mut map = BTreeMap::new();
//! map.insert(Foo { a: 1, b: "baz" }, 99);
//!
//! // 我们已经有一个 a 为 1 的 Foo，所以这次将会更新该 value。
//! map.insert(Foo { a: 1, b: "xyz" }, 100);
//!
//! // value 已被更新……
//! assert_eq!(map.values().next().unwrap(), &100);
//!
//! // ……但 key 并未改变。b 仍然是 "baz"，而不是 "xyz"。
//! assert_eq!(map.keys().next().unwrap().b, "baz");
//! ```

#![stable(feature = "rust1", since = "1.0.0")]

#[stable(feature = "try_reserve", since = "1.57.0")]
pub use alloc_crate::collections::TryReserveError;
#[unstable(
    feature = "try_reserve_kind",
    reason = "Uncertain how much info should be exposed",
    issue = "48043"
)]
pub use alloc_crate::collections::TryReserveErrorKind;
#[stable(feature = "rust1", since = "1.0.0")]
pub use alloc_crate::collections::{BTreeMap, BTreeSet, BinaryHeap};
#[stable(feature = "rust1", since = "1.0.0")]
pub use alloc_crate::collections::{LinkedList, VecDeque};
#[stable(feature = "rust1", since = "1.0.0")]
pub use alloc_crate::collections::{binary_heap, btree_map, btree_set};
#[stable(feature = "rust1", since = "1.0.0")]
pub use alloc_crate::collections::{linked_list, vec_deque};

#[stable(feature = "rust1", since = "1.0.0")]
#[doc(inline)]
pub use self::hash_map::HashMap;
#[stable(feature = "rust1", since = "1.0.0")]
#[doc(inline)]
pub use self::hash_set::HashSet;
#[stable(feature = "rust1", since = "1.0.0")]
// FIXME(#82080) 这里的弃用（deprecation）只是理论上的，实际上并不会产生警告。
#[deprecated(note = "moved to `std::ops::Bound`", since = "1.26.0")]
#[doc(hidden)]
pub use crate::ops::Bound;

mod hash;

#[stable(feature = "rust1", since = "1.0.0")]
pub mod hash_map {
    //! 一个使用二次探测（quadratic probing）与 SIMD 查找实现的 hash map。
    #[stable(feature = "rust1", since = "1.0.0")]
    pub use super::hash::map::*;
    #[stable(feature = "hashmap_build_hasher", since = "1.7.0")]
    pub use crate::hash::random::DefaultHasher;
    #[stable(feature = "hashmap_build_hasher", since = "1.7.0")]
    pub use crate::hash::random::RandomState;
}

#[stable(feature = "rust1", since = "1.0.0")]
pub mod hash_set {
    //! 一个 hash set，实现为 value 为 `()` 的 `HashMap`。
    #[stable(feature = "rust1", since = "1.0.0")]
    pub use super::hash::set::*;
}
