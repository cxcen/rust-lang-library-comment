#[cfg(test)]
mod tests;

use hashbrown::hash_map as base;

use self::Entry::*;
use crate::alloc::{Allocator, Global};
use crate::borrow::Borrow;
use crate::collections::{TryReserveError, TryReserveErrorKind};
use crate::error::Error;
use crate::fmt::{self, Debug};
use crate::hash::{BuildHasher, Hash, RandomState};
use crate::iter::FusedIterator;
use crate::ops::Index;

/// 基于二次探测（quadratic probing）与 SIMD 查找实现的 [hash map]（哈希映射）。
///
/// 默认情况下，`HashMap` 使用的哈希算法经过专门选择，用于抵御 HashDoS 攻击。
/// 该算法使用随机种子，并尽最大努力从宿主提供的、高质量且安全的随机源中生成
/// 这个种子，同时不会阻塞程序。正因如此，种子的随机性取决于种子创建时系统随机
/// 数协程（random number coroutine）的输出质量。特别地，当系统熵池异常偏低时
/// （例如系统启动期间）生成的种子，其质量可能较低。
///
/// 当前默认的哈希算法是 SipHash 1-3，不过这一点在未来任何时候都可能变化。
/// 它在中等大小的键上性能非常有竞争力，但对于诸如整数这样的小键、以及诸如长字符串
/// 这样的大键，其他哈希算法会更胜一筹——只是那些算法通常 *不能* 抵御 HashDoS
/// 之类的攻击。
///
/// 哈希算法可以按每个 `HashMap` 单独替换，方法是使用 [`default`]、[`with_hasher`]
/// 以及 [`with_capacity_and_hasher`]。crates.io 上还有许多
/// [可替代的哈希算法][hashing algorithms available on crates.io]。
///
/// 键必须实现 [`Eq`] 与 [`Hash`] trait，不过这通常可以通过
/// `#[derive(PartialEq, Eq, Hash)]` 来达成。如果你要自己实现它们，那么务必保证
/// 下面这条性质成立：
///
/// ```text
/// k1 == k2 -> hash(k1) == hash(k2)
/// ```
///
/// 换言之，如果两个键相等，它们的哈希值必须相等。违反这条性质属于逻辑错误
/// （logic error）。
///
/// 同样属于逻辑错误的是：当键已经位于 map 中时，以某种方式修改它，使得由 [`Hash`]
/// trait 决定的键哈希值、或由 [`Eq`] trait 决定的键相等性发生改变。这通常只可能
/// 通过 [`Cell`]、[`RefCell`]、全局状态、I/O 或 unsafe 代码做到。
///
/// 由上述任一逻辑错误所导致的行为是未指定的（not specified），但会被限制在那个
/// 观测到此逻辑错误的 `HashMap` 内部，不会导致未定义行为（undefined behavior）。
/// 这可能包括 panic、错误的结果、abort、内存泄漏以及不终止（non-termination）。
///
/// 此哈希表实现是 Google [SwissTable] 的 Rust 移植版。SwissTable 的原始 C++ 版本
/// 见 [here]，而 [CppCon talk] 这场演讲概述了该算法的工作原理。
///
/// [hash map]: crate::collections#use-a-hashmap-when
/// [hashing algorithms available on crates.io]: https://crates.io/keywords/hasher
/// [SwissTable]: https://abseil.io/blog/20180927-swisstables
/// [here]: https://github.com/abseil/abseil-cpp/blob/master/absl/container/internal/raw_hash_set.h
/// [CppCon talk]: https://www.youtube.com/watch?v=ncHmEUmJZf4
///
/// # 示例
///
/// ```
/// use std::collections::HashMap;
///
/// // 类型推断让我们可以省略显式的类型标注（在本例中应为
/// // `HashMap<String, String>`）。
/// let mut book_reviews = HashMap::new();
///
/// // 评论一些书籍。
/// book_reviews.insert(
///     "Adventures of Huckleberry Finn".to_string(),
///     "My favorite book.".to_string(),
/// );
/// book_reviews.insert(
///     "Grimms' Fairy Tales".to_string(),
///     "Masterpiece.".to_string(),
/// );
/// book_reviews.insert(
///     "Pride and Prejudice".to_string(),
///     "Very enjoyable.".to_string(),
/// );
/// book_reviews.insert(
///     "The Adventures of Sherlock Holmes".to_string(),
///     "Eye lyked it alot.".to_string(),
/// );
///
/// // 检查某个特定的键。
/// // 当集合存储的是拥有所有权的值（String）时，仍可用引用（&str）进行查询。
/// if !book_reviews.contains_key("Les Misérables") {
///     println!("We've got {} reviews, but Les Misérables ain't one.",
///              book_reviews.len());
/// }
///
/// // 糟糕，这条评论有不少拼写错误，把它删掉吧。
/// book_reviews.remove("The Adventures of Sherlock Holmes");
///
/// // 查询某些键所关联的值。
/// let to_find = ["Pride and Prejudice", "Alice's Adventure in Wonderland"];
/// for &book in &to_find {
///     match book_reviews.get(book) {
///         Some(review) => println!("{book}: {review}"),
///         None => println!("{book} is unreviewed.")
///     }
/// }
///
/// // 查询某个键对应的值（如果键不存在会 panic）。
/// println!("Review for Jane: {}", book_reviews["Pride and Prejudice"]);
///
/// // 遍历所有内容。
/// for (book, review) in &book_reviews {
///     println!("{book}: \"{review}\"");
/// }
/// ```
///
/// 当项目列表已知时，`HashMap` 可以从数组初始化：
///
/// ```
/// use std::collections::HashMap;
///
/// let solar_distance = HashMap::from([
///     ("Mercury", 0.4),
///     ("Venus", 0.7),
///     ("Earth", 1.0),
///     ("Mars", 1.5),
/// ]);
/// ```
///
/// ## `Entry` API
///
/// `HashMap` 实现了 [`Entry` API](#method.entry)，它支持获取、设置、更新和删除键
/// 及其值的复杂操作：
///
/// ```
/// use std::collections::HashMap;
///
/// // 类型推断让我们可以省略显式的类型标注（在本例中应为
/// // `HashMap<&str, u8>`）。
/// let mut player_stats = HashMap::new();
///
/// fn random_stat_buff() -> u8 {
///     // 这里其实可以返回某个随机值——为简单起见，暂时返回一个固定值。
///     42
/// }
///
/// // 仅在键尚不存在时插入它
/// player_stats.entry("health").or_insert(100);
///
/// // 使用一个提供新值的函数插入键，仅在键尚不存在时调用该函数
/// player_stats.entry("defence").or_insert_with(random_stat_buff);
///
/// // 更新某个键，并防范该键可能尚未设置的情况
/// let stat = player_stats.entry("attack").or_insert(100);
/// *stat += random_stat_buff();
///
/// // 在插入之前以就地变更（in-place mutation）的方式修改一个 entry
/// player_stats.entry("mana").and_modify(|mana| *mana += 200).or_insert(100);
/// ```
///
/// ## 与自定义键类型搭配使用
///
/// 将 `HashMap` 与自定义键类型搭配使用，最简单的方式是为该类型 derive [`Eq`] 与
/// [`Hash`]。我们还必须 derive [`PartialEq`]。
///
/// [`RefCell`]: crate::cell::RefCell
/// [`Cell`]: crate::cell::Cell
/// [`default`]: Default::default
/// [`with_hasher`]: Self::with_hasher
/// [`with_capacity_and_hasher`]: Self::with_capacity_and_hasher
///
/// ```
/// use std::collections::HashMap;
///
/// #[derive(Hash, Eq, PartialEq, Debug)]
/// struct Viking {
///     name: String,
///     country: String,
/// }
///
/// impl Viking {
///     /// 创建一个新的 Viking。
///     fn new(name: &str, country: &str) -> Viking {
///         Viking { name: name.to_string(), country: country.to_string() }
///     }
/// }
///
/// // 用一个 HashMap 来存储这些 viking 的生命值。
/// let vikings = HashMap::from([
///     (Viking::new("Einar", "Norway"), 25),
///     (Viking::new("Olaf", "Denmark"), 24),
///     (Viking::new("Harald", "Iceland"), 12),
/// ]);
///
/// // 使用 derive 得到的实现打印各 viking 的状态。
/// for (viking, health) in &vikings {
///     println!("{viking:?} has {health} hp");
/// }
/// ```
///
/// # 在 `const` 与 `static` 中使用
///
/// 如上所述，`HashMap` 使用随机种子：每个 `HashMap` 实例都使用不同的种子，这意味着
/// `HashMap::new` 通常无法用在 `const` 或 `static` 初始化器中。
///
/// 不过，如果你既需要在 `const` 或 `static` 初始化器中使用 `HashMap`，又想保留随机
/// 种子的生成，可以把 `HashMap` 包裹在 [`LazyLock`] 中。
///
/// 或者，你也可以在 `const` 或 `static` 初始化器中使用一个不依赖随机种子的哈希器来
/// 构造 `HashMap`。**请注意：以这种方式创建的 `HashMap` 无法抵御 HashDoS 攻击！**
///
/// [`LazyLock`]: crate::sync::LazyLock
/// ```rust
/// use std::collections::HashMap;
/// use std::hash::{BuildHasherDefault, DefaultHasher};
/// use std::sync::{LazyLock, Mutex};
///
/// // 使用固定的、非随机哈希器的 HashMap
/// const NONRANDOM_EMPTY_MAP: HashMap<String, Vec<i32>, BuildHasherDefault<DefaultHasher>> =
///     HashMap::with_hasher(BuildHasherDefault::new());
/// static NONRANDOM_MAP: Mutex<HashMap<String, Vec<i32>, BuildHasherDefault<DefaultHasher>>> =
///     Mutex::new(HashMap::with_hasher(BuildHasherDefault::new()));
///
/// // 使用 LazyLock 以保留随机种子的 HashMap
/// const RANDOM_EMPTY_MAP: LazyLock<HashMap<String, Vec<i32>>> =
///     LazyLock::new(HashMap::new);
/// static RANDOM_MAP: LazyLock<Mutex<HashMap<String, Vec<i32>>>> =
///     LazyLock::new(|| Mutex::new(HashMap::new()));
/// ```

#[cfg_attr(not(test), rustc_diagnostic_item = "HashMap")]
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_insignificant_dtor]
pub struct HashMap<
    K,
    V,
    S = RandomState,
    #[unstable(feature = "allocator_api", issue = "32838")] A: Allocator = Global,
> {
    base: base::HashMap<K, V, S, A>,
}

impl<K, V> HashMap<K, V, RandomState> {
    /// 创建一个空的 `HashMap`。
    ///
    /// 该哈希 map 初始创建时容量为 0，因此在首次插入之前不会进行任何分配。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashMap;
    /// let mut map: HashMap<&str, i32> = HashMap::new();
    /// ```
    #[inline]
    #[must_use]
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn new() -> HashMap<K, V, RandomState> {
        Default::default()
    }

    /// 创建一个空的 `HashMap`，其容量至少为指定值。
    ///
    /// 该哈希 map 将能够在不重新分配的情况下至少容纳 `capacity` 个元素。本方法允许
    /// 为多于 `capacity` 的元素进行分配。如果 `capacity` 为零，则该哈希 map 不会分配。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashMap;
    /// let mut map: HashMap<&str, i32> = HashMap::with_capacity(10);
    /// ```
    #[inline]
    #[must_use]
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn with_capacity(capacity: usize) -> HashMap<K, V, RandomState> {
        HashMap::with_capacity_and_hasher(capacity, Default::default())
    }
}

impl<K, V, A: Allocator> HashMap<K, V, RandomState, A> {
    /// 使用给定的分配器（allocator）创建一个空的 `HashMap`。
    ///
    /// 该哈希 map 初始创建时容量为 0，因此在首次插入之前不会进行任何分配。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashMap;
    /// let mut map: HashMap<&str, i32> = HashMap::new();
    /// ```
    #[inline]
    #[must_use]
    #[unstable(feature = "allocator_api", issue = "32838")]
    pub fn new_in(alloc: A) -> Self {
        HashMap::with_hasher_in(Default::default(), alloc)
    }

    /// 使用给定的分配器创建一个空的 `HashMap`，其容量至少为指定值。
    ///
    /// 该哈希 map 将能够在不重新分配的情况下至少容纳 `capacity` 个元素。本方法允许
    /// 为多于 `capacity` 的元素进行分配。如果 `capacity` 为零，则该哈希 map 不会分配。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashMap;
    /// let mut map: HashMap<&str, i32> = HashMap::with_capacity(10);
    /// ```
    #[inline]
    #[must_use]
    #[unstable(feature = "allocator_api", issue = "32838")]
    pub fn with_capacity_in(capacity: usize, alloc: A) -> Self {
        HashMap::with_capacity_and_hasher_in(capacity, Default::default(), alloc)
    }
}

impl<K, V, S> HashMap<K, V, S> {
    /// 创建一个空的 `HashMap`，它将使用给定的哈希构造器（hash builder）来对键做哈希。
    ///
    /// 创建出的 map 具有默认的初始容量。
    ///
    /// 警告：`hash_builder` 通常是随机生成的，其设计目的是让 HashMap 能够抵御那些
    /// 制造大量哈希冲突、从而导致性能极差的攻击。通过本函数手动设置它，可能会暴露出
    /// 一个 DoS 攻击面。
    ///
    /// 传入的 `hash_builder` 应实现 [`BuildHasher`] trait，这样 `HashMap` 才有意义，
    /// 详见其文档。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashMap;
    /// use std::hash::RandomState;
    ///
    /// let s = RandomState::new();
    /// let mut map = HashMap::with_hasher(s);
    /// map.insert(1, 2);
    /// ```
    #[inline]
    #[stable(feature = "hashmap_build_hasher", since = "1.7.0")]
    #[rustc_const_stable(feature = "const_collections_with_hasher", since = "1.85.0")]
    pub const fn with_hasher(hash_builder: S) -> HashMap<K, V, S> {
        HashMap { base: base::HashMap::with_hasher(hash_builder) }
    }

    /// 创建一个空的 `HashMap`，其容量至少为指定值，并使用 `hasher` 来对键做哈希。
    ///
    /// 该哈希 map 将能够在不重新分配的情况下至少容纳 `capacity` 个元素。本方法允许
    /// 为多于 `capacity` 的元素进行分配。如果 `capacity` 为零，则该哈希 map 不会分配。
    ///
    /// 警告：`hasher` 通常是随机生成的，其设计目的是让 HashMap 能够抵御那些制造大量
    /// 哈希冲突、从而导致性能极差的攻击。通过本函数手动设置它，可能会暴露出一个 DoS
    /// 攻击面。
    ///
    /// 传入的 `hasher` 应实现 [`BuildHasher`] trait，这样 `HashMap` 才有意义，
    /// 详见其文档。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashMap;
    /// use std::hash::RandomState;
    ///
    /// let s = RandomState::new();
    /// let mut map = HashMap::with_capacity_and_hasher(10, s);
    /// map.insert(1, 2);
    /// ```
    #[inline]
    #[stable(feature = "hashmap_build_hasher", since = "1.7.0")]
    pub fn with_capacity_and_hasher(capacity: usize, hasher: S) -> HashMap<K, V, S> {
        HashMap { base: base::HashMap::with_capacity_and_hasher(capacity, hasher) }
    }
}

impl<K, V, S, A: Allocator> HashMap<K, V, S, A> {
    /// 创建一个空的 `HashMap`，它将使用给定的哈希构造器和分配器。
    ///
    /// 创建出的 map 具有默认的初始容量。
    ///
    /// 警告：`hash_builder` 通常是随机生成的，其设计目的是让 HashMap 能够抵御那些
    /// 制造大量哈希冲突、从而导致性能极差的攻击。通过本函数手动设置它，可能会暴露出
    /// 一个 DoS 攻击面。
    ///
    /// 传入的 `hash_builder` 应实现 [`BuildHasher`] trait，这样 `HashMap` 才有意义，
    /// 详见其文档。
    #[inline]
    #[unstable(feature = "allocator_api", issue = "32838")]
    pub fn with_hasher_in(hash_builder: S, alloc: A) -> Self {
        HashMap { base: base::HashMap::with_hasher_in(hash_builder, alloc) }
    }

    /// 创建一个空的 `HashMap`，其容量至少为指定值，使用 `hasher` 来对键做哈希、
    /// 并使用 `alloc` 来分配内存。
    ///
    /// 该哈希 map 将能够在不重新分配的情况下至少容纳 `capacity` 个元素。本方法允许
    /// 为多于 `capacity` 的元素进行分配。如果 `capacity` 为零，则该哈希 map 不会分配。
    ///
    /// 警告：`hasher` 通常是随机生成的，其设计目的是让 HashMap 能够抵御那些制造大量
    /// 哈希冲突、从而导致性能极差的攻击。通过本函数手动设置它，可能会暴露出一个 DoS
    /// 攻击面。
    ///
    /// 传入的 `hasher` 应实现 [`BuildHasher`] trait，这样 `HashMap` 才有意义，
    /// 详见其文档。
    ///
    #[inline]
    #[unstable(feature = "allocator_api", issue = "32838")]
    pub fn with_capacity_and_hasher_in(capacity: usize, hash_builder: S, alloc: A) -> Self {
        HashMap { base: base::HashMap::with_capacity_and_hasher_in(capacity, hash_builder, alloc) }
    }

    /// 返回该 map 在不重新分配的情况下能够容纳的元素数量。
    ///
    /// 这个数字是一个下界；`HashMap<K, V>` 可能能够容纳更多元素，但保证至少能容纳
    /// 这么多。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashMap;
    /// let map: HashMap<i32, i32> = HashMap::with_capacity(100);
    /// assert!(map.capacity() >= 100);
    /// ```
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn capacity(&self) -> usize {
        self.base.capacity()
    }

    /// 一个以任意顺序遍历所有键的迭代器。
    /// 迭代器的元素类型为 `&'a K`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashMap;
    ///
    /// let map = HashMap::from([
    ///     ("a", 1),
    ///     ("b", 2),
    ///     ("c", 3),
    /// ]);
    ///
    /// for key in map.keys() {
    ///     println!("{key}");
    /// }
    /// ```
    ///
    /// # Performance
    ///
    /// 在当前实现中，遍历键耗费 O(capacity) 的时间而非 O(len)，因为它内部也会访问
    /// 空桶（empty buckets）。
    #[rustc_lint_query_instability]
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn keys(&self) -> Keys<'_, K, V> {
        Keys { inner: self.iter() }
    }

    /// 创建一个以任意顺序遍历所有键的消耗型（consuming）迭代器。
    /// 调用之后该 map 不能再被使用。
    /// 迭代器的元素类型为 `K`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashMap;
    ///
    /// let map = HashMap::from([
    ///     ("a", 1),
    ///     ("b", 2),
    ///     ("c", 3),
    /// ]);
    ///
    /// let mut vec: Vec<&str> = map.into_keys().collect();
    /// // `IntoKeys` 迭代器以任意顺序产出键，所以必须先对键排序，才能与已排序的
    /// // 数组进行比较。
    /// vec.sort_unstable();
    /// assert_eq!(vec, ["a", "b", "c"]);
    /// ```
    ///
    /// # Performance
    ///
    /// 在当前实现中，遍历键耗费 O(capacity) 的时间而非 O(len)，因为它内部也会访问
    /// 空桶。
    #[inline]
    #[rustc_lint_query_instability]
    #[stable(feature = "map_into_keys_values", since = "1.54.0")]
    pub fn into_keys(self) -> IntoKeys<K, V, A> {
        IntoKeys { inner: self.into_iter() }
    }

    /// 一个以任意顺序遍历所有值的迭代器。
    /// 迭代器的元素类型为 `&'a V`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashMap;
    ///
    /// let map = HashMap::from([
    ///     ("a", 1),
    ///     ("b", 2),
    ///     ("c", 3),
    /// ]);
    ///
    /// for val in map.values() {
    ///     println!("{val}");
    /// }
    /// ```
    ///
    /// # Performance
    ///
    /// 在当前实现中，遍历值耗费 O(capacity) 的时间而非 O(len)，因为它内部也会访问
    /// 空桶。
    #[rustc_lint_query_instability]
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn values(&self) -> Values<'_, K, V> {
        Values { inner: self.iter() }
    }

    /// 一个以任意顺序、以可变方式遍历所有值的迭代器。
    /// 迭代器的元素类型为 `&'a mut V`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashMap;
    ///
    /// let mut map = HashMap::from([
    ///     ("a", 1),
    ///     ("b", 2),
    ///     ("c", 3),
    /// ]);
    ///
    /// for val in map.values_mut() {
    ///     *val = *val + 10;
    /// }
    ///
    /// for val in map.values() {
    ///     println!("{val}");
    /// }
    /// ```
    ///
    /// # Performance
    ///
    /// 在当前实现中，遍历值耗费 O(capacity) 的时间而非 O(len)，因为它内部也会访问
    /// 空桶。
    #[rustc_lint_query_instability]
    #[stable(feature = "map_values_mut", since = "1.10.0")]
    pub fn values_mut(&mut self) -> ValuesMut<'_, K, V> {
        ValuesMut { inner: self.iter_mut() }
    }

    /// 创建一个以任意顺序遍历所有值的消耗型迭代器。
    /// 调用之后该 map 不能再被使用。
    /// 迭代器的元素类型为 `V`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashMap;
    ///
    /// let map = HashMap::from([
    ///     ("a", 1),
    ///     ("b", 2),
    ///     ("c", 3),
    /// ]);
    ///
    /// let mut vec: Vec<i32> = map.into_values().collect();
    /// // `IntoValues` 迭代器以任意顺序产出值，所以必须先对值排序，才能与已排序的
    /// // 数组进行比较。
    /// vec.sort_unstable();
    /// assert_eq!(vec, [1, 2, 3]);
    /// ```
    ///
    /// # Performance
    ///
    /// 在当前实现中，遍历值耗费 O(capacity) 的时间而非 O(len)，因为它内部也会访问
    /// 空桶。
    #[inline]
    #[rustc_lint_query_instability]
    #[stable(feature = "map_into_keys_values", since = "1.54.0")]
    pub fn into_values(self) -> IntoValues<K, V, A> {
        IntoValues { inner: self.into_iter() }
    }

    /// 一个以任意顺序遍历所有键值对的迭代器。
    /// 迭代器的元素类型为 `(&'a K, &'a V)`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashMap;
    ///
    /// let map = HashMap::from([
    ///     ("a", 1),
    ///     ("b", 2),
    ///     ("c", 3),
    /// ]);
    ///
    /// for (key, val) in map.iter() {
    ///     println!("key: {key} val: {val}");
    /// }
    /// ```
    ///
    /// # Performance
    ///
    /// 在当前实现中，遍历 map 耗费 O(capacity) 的时间而非 O(len)，因为它内部也会访问
    /// 空桶。
    #[rustc_lint_query_instability]
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn iter(&self) -> Iter<'_, K, V> {
        Iter { base: self.base.iter() }
    }

    /// 一个以任意顺序遍历所有键值对的迭代器，且对值持有可变引用。
    /// 迭代器的元素类型为 `(&'a K, &'a mut V)`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashMap;
    ///
    /// let mut map = HashMap::from([
    ///     ("a", 1),
    ///     ("b", 2),
    ///     ("c", 3),
    /// ]);
    ///
    /// // 更新所有值
    /// for (_, val) in map.iter_mut() {
    ///     *val *= 2;
    /// }
    ///
    /// for (key, val) in &map {
    ///     println!("key: {key} val: {val}");
    /// }
    /// ```
    ///
    /// # Performance
    ///
    /// 在当前实现中，遍历 map 耗费 O(capacity) 的时间而非 O(len)，因为它内部也会访问
    /// 空桶。
    #[rustc_lint_query_instability]
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn iter_mut(&mut self) -> IterMut<'_, K, V> {
        IterMut { base: self.base.iter_mut() }
    }

    /// 返回该 map 中元素的数量。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashMap;
    ///
    /// let mut a = HashMap::new();
    /// assert_eq!(a.len(), 0);
    /// a.insert(1, "a");
    /// assert_eq!(a.len(), 1);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn len(&self) -> usize {
        self.base.len()
    }

    /// 如果该 map 不含任何元素，则返回 `true`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashMap;
    ///
    /// let mut a = HashMap::new();
    /// assert!(a.is_empty());
    /// a.insert(1, "a");
    /// assert!(!a.is_empty());
    /// ```
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn is_empty(&self) -> bool {
        self.base.is_empty()
    }

    /// 清空该 map，将所有键值对作为一个迭代器返回。保留已分配的内存以便复用。
    ///
    /// 如果返回的迭代器在被完全消耗之前就被丢弃（drop），它会丢弃剩余的键值对。
    /// 返回的迭代器对该 map 持有一个可变借用，以优化其实现。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashMap;
    ///
    /// let mut a = HashMap::new();
    /// a.insert(1, "a");
    /// a.insert(2, "b");
    ///
    /// for (k, v) in a.drain().take(1) {
    ///     assert!(k == 1 || k == 2);
    ///     assert!(v == "a" || v == "b");
    /// }
    ///
    /// assert!(a.is_empty());
    /// ```
    #[inline]
    #[rustc_lint_query_instability]
    #[stable(feature = "drain", since = "1.6.0")]
    pub fn drain(&mut self) -> Drain<'_, K, V, A> {
        Drain { base: self.base.drain() }
    }

    /// 创建一个迭代器，它使用一个闭包来判定某个元素（键值对）是否应被移除。
    ///
    /// 如果闭包返回 `true`，该元素会被从 map 中移除并产出。如果闭包返回 `false`
    /// 或发生 panic，该元素仍保留在 map 中且不会被产出。
    ///
    /// 该迭代器还允许你在闭包中变更每个元素的值，无论你选择保留还是移除它。
    ///
    /// 如果返回的 `ExtractIf` 没有被耗尽（例如未经迭代就被丢弃、或迭代发生短路），
    /// 那么剩余元素将被保留。如果你不需要返回的迭代器，请改用 [`retain`] 并传入一个
    /// 取反的谓词。
    ///
    /// [`retain`]: HashMap::retain
    ///
    /// # 示例
    ///
    /// 将一个 map 按奇偶键拆分，并复用原始 map：
    ///
    /// ```
    /// use std::collections::HashMap;
    ///
    /// let mut map: HashMap<i32, i32> = (0..8).map(|x| (x, x)).collect();
    /// let extracted: HashMap<i32, i32> = map.extract_if(|k, _v| k % 2 == 0).collect();
    ///
    /// let mut evens = extracted.keys().copied().collect::<Vec<_>>();
    /// let mut odds = map.keys().copied().collect::<Vec<_>>();
    /// evens.sort();
    /// odds.sort();
    ///
    /// assert_eq!(evens, vec![0, 2, 4, 6]);
    /// assert_eq!(odds, vec![1, 3, 5, 7]);
    /// ```
    #[inline]
    #[rustc_lint_query_instability]
    #[stable(feature = "hash_extract_if", since = "1.88.0")]
    pub fn extract_if<F>(&mut self, pred: F) -> ExtractIf<'_, K, V, F, A>
    where
        F: FnMut(&K, &mut V) -> bool,
    {
        ExtractIf { base: self.base.extract_if(pred) }
    }

    /// 只保留由谓词所指定的元素。
    ///
    /// 换言之，移除所有使 `f(&k, &mut v)` 返回 `false` 的键值对 `(k, v)`。
    /// 元素的访问顺序是未排序的（且未指定）。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashMap;
    ///
    /// let mut map: HashMap<i32, i32> = (0..8).map(|x| (x, x*10)).collect();
    /// map.retain(|&k, _| k % 2 == 0);
    /// assert_eq!(map.len(), 4);
    /// ```
    ///
    /// # Performance
    ///
    /// 在当前实现中，此操作耗费 O(capacity) 的时间而非 O(len)，因为它内部也会访问
    /// 空桶。
    #[inline]
    #[rustc_lint_query_instability]
    #[stable(feature = "retain_hash_collection", since = "1.18.0")]
    pub fn retain<F>(&mut self, f: F)
    where
        F: FnMut(&K, &mut V) -> bool,
    {
        self.base.retain(f)
    }

    /// 清空该 map，移除所有键值对。保留已分配的内存以便复用。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashMap;
    ///
    /// let mut a = HashMap::new();
    /// a.insert(1, "a");
    /// a.clear();
    /// assert!(a.is_empty());
    /// ```
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn clear(&mut self) {
        self.base.clear();
    }

    /// 返回该 map 的 [`BuildHasher`] 的引用。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashMap;
    /// use std::hash::RandomState;
    ///
    /// let hasher = RandomState::new();
    /// let map: HashMap<i32, i32> = HashMap::with_hasher(hasher);
    /// let hasher: &RandomState = map.hasher();
    /// ```
    #[inline]
    #[stable(feature = "hashmap_public_hasher", since = "1.9.0")]
    pub fn hasher(&self) -> &S {
        self.base.hasher()
    }
}

impl<K, V, S, A> HashMap<K, V, S, A>
where
    K: Eq + Hash,
    S: BuildHasher,
    A: Allocator,
{
    /// 为在 `HashMap` 中再插入至少 `additional` 个元素预留容量。该集合可能会预留
    /// 更多空间，以推测性地避免频繁的重新分配。调用 `reserve` 之后，容量将大于或
    /// 等于 `self.len() + additional`。如果容量已经足够，则什么也不做。
    ///
    /// # Panics
    ///
    /// 如果新的分配大小溢出 [`usize`]，则 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashMap;
    /// let mut map: HashMap<&str, i32> = HashMap::new();
    /// map.reserve(10);
    /// ```
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn reserve(&mut self, additional: usize) {
        self.base.reserve(additional)
    }

    /// 尝试为在 `HashMap` 中再插入至少 `additional` 个元素预留容量。该集合可能会预留
    /// 更多空间，以推测性地避免频繁的重新分配。调用 `try_reserve` 之后，如果它返回
    /// `Ok(())`，则容量将大于或等于 `self.len() + additional`。如果容量已经足够，
    /// 则什么也不做。
    ///
    /// # Errors
    ///
    /// 如果容量溢出，或分配器报告失败，则返回一个错误。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashMap;
    ///
    /// let mut map: HashMap<&str, isize> = HashMap::new();
    /// map.try_reserve(10).expect("why is the test harness OOMing on a handful of bytes?");
    /// ```
    #[inline]
    #[stable(feature = "try_reserve", since = "1.57.0")]
    pub fn try_reserve(&mut self, additional: usize) -> Result<(), TryReserveError> {
        self.base.try_reserve(additional).map_err(map_try_reserve_error)
    }

    /// 尽可能收缩该 map 的容量。它会在维持内部规则的前提下尽量降低容量，并可能根据
    /// 调整大小策略（resize policy）保留一些余量。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashMap;
    ///
    /// let mut map: HashMap<i32, i32> = HashMap::with_capacity(100);
    /// map.insert(1, 2);
    /// map.insert(3, 4);
    /// assert!(map.capacity() >= 100);
    /// map.shrink_to_fit();
    /// assert!(map.capacity() >= 2);
    /// ```
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn shrink_to_fit(&mut self) {
        self.base.shrink_to_fit();
    }

    /// 将该 map 的容量收缩到一个下限。它会在维持内部规则的前提下，降低到不低于所给
    /// 下限的水平，并可能根据调整大小策略保留一些余量。
    ///
    /// 如果当前容量小于该下限，则此操作为空操作（no-op）。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashMap;
    ///
    /// let mut map: HashMap<i32, i32> = HashMap::with_capacity(100);
    /// map.insert(1, 2);
    /// map.insert(3, 4);
    /// assert!(map.capacity() >= 100);
    /// map.shrink_to(10);
    /// assert!(map.capacity() >= 10);
    /// map.shrink_to(0);
    /// assert!(map.capacity() >= 2);
    /// ```
    #[inline]
    #[stable(feature = "shrink_to", since = "1.56.0")]
    pub fn shrink_to(&mut self, min_capacity: usize) {
        self.base.shrink_to(min_capacity);
    }

    /// 获取给定键在 map 中所对应的 entry，以便进行就地操作。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashMap;
    ///
    /// let mut letters = HashMap::new();
    ///
    /// for ch in "a short treatise on fungi".chars() {
    ///     letters.entry(ch).and_modify(|counter| *counter += 1).or_insert(1);
    /// }
    ///
    /// assert_eq!(letters[&'s'], 2);
    /// assert_eq!(letters[&'t'], 3);
    /// assert_eq!(letters[&'u'], 1);
    /// assert_eq!(letters.get(&'y'), None);
    /// ```
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn entry(&mut self, key: K) -> Entry<'_, K, V, A> {
        map_entry(self.base.rustc_entry(key))
    }

    /// 返回该键所对应的值的引用。
    ///
    /// 这个键可以是 map 键类型的任意借用形式（borrowed form），但借用形式上的
    /// [`Hash`] 与 [`Eq`] *必须* 与键类型上的保持一致。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashMap;
    ///
    /// let mut map = HashMap::new();
    /// map.insert(1, "a");
    /// assert_eq!(map.get(&1), Some(&"a"));
    /// assert_eq!(map.get(&2), None);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn get<Q: ?Sized>(&self, k: &Q) -> Option<&V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq,
    {
        self.base.get(k)
    }

    /// 返回所给键对应的键值对。这在以下情况可能有用：
    /// - 对于那些不完全相同的键也可被视为相等的键类型；
    /// - 用于从借用的 `&Q` 查找键获取存储着的 `&K` 键值；或者
    /// - 用于获取一个与该集合具有相同生命周期的键的引用。
    ///
    /// 所给的键可以是 map 键类型的任意借用形式，但借用形式上的 [`Hash`] 与 [`Eq`]
    /// *必须* 与键类型上的保持一致。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashMap;
    /// use std::hash::{Hash, Hasher};
    ///
    /// #[derive(Clone, Copy, Debug)]
    /// struct S {
    ///     id: u32,
    /// #   #[allow(unused)] // 防止出现 "field `name` is never read" 错误
    ///     name: &'static str, // 在相等性与哈希操作中被忽略
    /// }
    ///
    /// impl PartialEq for S {
    ///     fn eq(&self, other: &S) -> bool {
    ///         self.id == other.id
    ///     }
    /// }
    ///
    /// impl Eq for S {}
    ///
    /// impl Hash for S {
    ///     fn hash<H: Hasher>(&self, state: &mut H) {
    ///         self.id.hash(state);
    ///     }
    /// }
    ///
    /// let j_a = S { id: 1, name: "Jessica" };
    /// let j_b = S { id: 1, name: "Jess" };
    /// let p = S { id: 2, name: "Paul" };
    /// assert_eq!(j_a, j_b);
    ///
    /// let mut map = HashMap::new();
    /// map.insert(j_a, "Paris");
    /// assert_eq!(map.get_key_value(&j_a), Some((&j_a, &"Paris")));
    /// assert_eq!(map.get_key_value(&j_b), Some((&j_a, &"Paris"))); // 值得注意的情形
    /// assert_eq!(map.get_key_value(&p), None);
    /// ```
    #[inline]
    #[stable(feature = "map_get_key_value", since = "1.40.0")]
    pub fn get_key_value<Q: ?Sized>(&self, k: &Q) -> Option<(&K, &V)>
    where
        K: Borrow<Q>,
        Q: Hash + Eq,
    {
        self.base.get_key_value(k)
    }

    /// 尝试一次性获取 map 中 `N` 个值的可变引用。
    ///
    /// 返回一个长度为 `N` 的数组，包含每次查询的结果。出于健全性（soundness）考虑，
    /// 对任何一个值最多只会返回一个可变引用。如果某个键缺失，则相应位置为 `None`。
    ///
    /// 本方法会执行一次检查以确保没有重复的键，该检查目前的时间复杂度为 O(n^2)，
    /// 因此在传入大量键时要小心。
    ///
    /// # Panics
    ///
    /// 如果有任何键相互重叠，则 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashMap;
    ///
    /// let mut libraries = HashMap::new();
    /// libraries.insert("Bodleian Library".to_string(), 1602);
    /// libraries.insert("Athenæum".to_string(), 1807);
    /// libraries.insert("Herzogin-Anna-Amalia-Bibliothek".to_string(), 1691);
    /// libraries.insert("Library of Congress".to_string(), 1800);
    ///
    /// // 获取 Athenæum 和 Bodleian Library
    /// let [Some(a), Some(b)] = libraries.get_disjoint_mut([
    ///     "Athenæum",
    ///     "Bodleian Library",
    /// ]) else { panic!() };
    ///
    /// // Assert values of Athenæum and Library of Congress
    /// let got = libraries.get_disjoint_mut([
    ///     "Athenæum",
    ///     "Library of Congress",
    /// ]);
    /// assert_eq!(
    ///     got,
    ///     [
    ///         Some(&mut 1807),
    ///         Some(&mut 1800),
    ///     ],
    /// );
    ///
    /// // 缺失的键得到 None
    /// let got = libraries.get_disjoint_mut([
    ///     "Athenæum",
    ///     "New York Public Library",
    /// ]);
    /// assert_eq!(
    ///     got,
    ///     [
    ///         Some(&mut 1807),
    ///         None
    ///     ]
    /// );
    /// ```
    ///
    /// ```should_panic
    /// use std::collections::HashMap;
    ///
    /// let mut libraries = HashMap::new();
    /// libraries.insert("Athenæum".to_string(), 1807);
    ///
    /// // 重复的键会 panic！
    /// let got = libraries.get_disjoint_mut([
    ///     "Athenæum",
    ///     "Athenæum",
    /// ]);
    /// ```
    #[inline]
    #[doc(alias = "get_many_mut")]
    #[stable(feature = "map_many_mut", since = "1.86.0")]
    pub fn get_disjoint_mut<Q: ?Sized, const N: usize>(
        &mut self,
        ks: [&Q; N],
    ) -> [Option<&'_ mut V>; N]
    where
        K: Borrow<Q>,
        Q: Hash + Eq,
    {
        self.base.get_disjoint_mut(ks)
    }

    /// 尝试一次性获取 map 中 `N` 个值的可变引用，且不校验这些值是否互不相同（unique）。
    ///
    /// 返回一个长度为 `N` 的数组，包含每次查询的结果。如果某个键缺失，则相应位置为
    /// `None`。
    ///
    /// 关于安全的替代方案，参见 [`get_disjoint_mut`](`HashMap::get_disjoint_mut`)。
    ///
    /// # 安全性(Safety）
    ///
    /// 以相互重叠的键调用本方法属于*[未定义行为][undefined behavior]*，即使产生的
    /// 那些引用并未被使用。
    ///
    /// [undefined behavior]: https://doc.rust-lang.org/reference/behavior-considered-undefined.html
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashMap;
    ///
    /// let mut libraries = HashMap::new();
    /// libraries.insert("Bodleian Library".to_string(), 1602);
    /// libraries.insert("Athenæum".to_string(), 1807);
    /// libraries.insert("Herzogin-Anna-Amalia-Bibliothek".to_string(), 1691);
    /// libraries.insert("Library of Congress".to_string(), 1800);
    ///
    /// // SAFETY: 这些键互不重叠。
    /// let [Some(a), Some(b)] = (unsafe { libraries.get_disjoint_unchecked_mut([
    ///     "Athenæum",
    ///     "Bodleian Library",
    /// ]) }) else { panic!() };
    ///
    /// // SAFETY: 这些键互不重叠。
    /// let got = unsafe { libraries.get_disjoint_unchecked_mut([
    ///     "Athenæum",
    ///     "Library of Congress",
    /// ]) };
    /// assert_eq!(
    ///     got,
    ///     [
    ///         Some(&mut 1807),
    ///         Some(&mut 1800),
    ///     ],
    /// );
    ///
    /// // SAFETY: 这些键互不重叠。
    /// let got = unsafe { libraries.get_disjoint_unchecked_mut([
    ///     "Athenæum",
    ///     "New York Public Library",
    /// ]) };
    /// // 缺失的键得到 None
    /// assert_eq!(got, [Some(&mut 1807), None]);
    /// ```
    #[inline]
    #[doc(alias = "get_many_unchecked_mut")]
    #[stable(feature = "map_many_mut", since = "1.86.0")]
    pub unsafe fn get_disjoint_unchecked_mut<Q: ?Sized, const N: usize>(
        &mut self,
        ks: [&Q; N],
    ) -> [Option<&'_ mut V>; N]
    where
        K: Borrow<Q>,
        Q: Hash + Eq,
    {
        unsafe { self.base.get_disjoint_unchecked_mut(ks) }
    }

    /// 如果该 map 含有指定键对应的值，则返回 `true`。
    ///
    /// 这个键可以是 map 键类型的任意借用形式，但借用形式上的 [`Hash`] 与 [`Eq`]
    /// *必须* 与键类型上的保持一致。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashMap;
    ///
    /// let mut map = HashMap::new();
    /// map.insert(1, "a");
    /// assert_eq!(map.contains_key(&1), true);
    /// assert_eq!(map.contains_key(&2), false);
    /// ```
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[cfg_attr(not(test), rustc_diagnostic_item = "hashmap_contains_key")]
    pub fn contains_key<Q: ?Sized>(&self, k: &Q) -> bool
    where
        K: Borrow<Q>,
        Q: Hash + Eq,
    {
        self.base.contains_key(k)
    }

    /// 返回该键所对应的值的可变引用。
    ///
    /// 这个键可以是 map 键类型的任意借用形式，但借用形式上的 [`Hash`] 与 [`Eq`]
    /// *必须* 与键类型上的保持一致。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashMap;
    ///
    /// let mut map = HashMap::new();
    /// map.insert(1, "a");
    /// if let Some(x) = map.get_mut(&1) {
    ///     *x = "b";
    /// }
    /// assert_eq!(map[&1], "b");
    /// ```
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn get_mut<Q: ?Sized>(&mut self, k: &Q) -> Option<&mut V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq,
    {
        self.base.get_mut(k)
    }

    /// 向该 map 中插入一个键值对。
    ///
    /// 如果该 map 此前不存在这个键，则返回 [`None`]。
    ///
    /// 如果该 map 此前已存在这个键，则更新其值，并返回旧值。不过键本身不会被更新；
    /// 这对于那些可以 `==` 却并不完全相同的类型而言很重要。详见
    /// [模块级文档][module-level documentation]。
    ///
    /// [module-level documentation]: crate::collections#insert-and-complex-keys
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashMap;
    ///
    /// let mut map = HashMap::new();
    /// assert_eq!(map.insert(37, "a"), None);
    /// assert_eq!(map.is_empty(), false);
    ///
    /// map.insert(37, "b");
    /// assert_eq!(map.insert(37, "c"), Some("b"));
    /// assert_eq!(map[&37], "c");
    /// ```
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_confusables("push", "append", "put")]
    #[cfg_attr(not(test), rustc_diagnostic_item = "hashmap_insert")]
    pub fn insert(&mut self, k: K, v: V) -> Option<V> {
        self.base.insert(k, v)
    }

    /// 尝试向该 map 中插入一个键值对，并返回该 entry 中值的可变引用。
    ///
    /// 如果该 map 中已存在这个键，则什么也不更新，并返回一个错误，其中包含被占用的
    /// entry 与传入的值。
    ///
    /// # 示例
    ///
    /// 基本用法：
    ///
    /// ```
    /// #![feature(map_try_insert)]
    ///
    /// use std::collections::HashMap;
    ///
    /// let mut map = HashMap::new();
    /// assert_eq!(map.try_insert(37, "a").unwrap(), &"a");
    ///
    /// let err = map.try_insert(37, "b").unwrap_err();
    /// assert_eq!(err.entry.key(), &37);
    /// assert_eq!(err.entry.get(), &"a");
    /// assert_eq!(err.value, "b");
    /// ```
    #[unstable(feature = "map_try_insert", issue = "82766")]
    pub fn try_insert(&mut self, key: K, value: V) -> Result<&mut V, OccupiedError<'_, K, V, A>> {
        match self.entry(key) {
            Occupied(entry) => Err(OccupiedError { entry, value }),
            Vacant(entry) => Ok(entry.insert(value)),
        }
    }

    /// 从该 map 中移除一个键，如果该键此前位于 map 中，则返回该键处的值。
    ///
    /// 这个键可以是 map 键类型的任意借用形式，但借用形式上的 [`Hash`] 与 [`Eq`]
    /// *必须* 与键类型上的保持一致。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashMap;
    ///
    /// let mut map = HashMap::new();
    /// map.insert(1, "a");
    /// assert_eq!(map.remove(&1), Some("a"));
    /// assert_eq!(map.remove(&1), None);
    /// ```
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_confusables("delete", "take")]
    pub fn remove<Q: ?Sized>(&mut self, k: &Q) -> Option<V>
    where
        K: Borrow<Q>,
        Q: Hash + Eq,
    {
        self.base.remove(k)
    }

    /// 从该 map 中移除一个键，如果该键此前位于 map 中，则返回存储着的键和值。
    ///
    /// 这个键可以是 map 键类型的任意借用形式，但借用形式上的 [`Hash`] 与 [`Eq`]
    /// *必须* 与键类型上的保持一致。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashMap;
    ///
    /// # fn main() {
    /// let mut map = HashMap::new();
    /// map.insert(1, "a");
    /// assert_eq!(map.remove_entry(&1), Some((1, "a")));
    /// assert_eq!(map.remove(&1), None);
    /// # }
    /// ```
    #[inline]
    #[stable(feature = "hash_map_remove_entry", since = "1.27.0")]
    pub fn remove_entry<Q: ?Sized>(&mut self, k: &Q) -> Option<(K, V)>
    where
        K: Borrow<Q>,
        Q: Hash + Eq,
    {
        self.base.remove_entry(k)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<K, V, S, A> Clone for HashMap<K, V, S, A>
where
    K: Clone,
    V: Clone,
    S: Clone,
    A: Allocator + Clone,
{
    #[inline]
    fn clone(&self) -> Self {
        Self { base: self.base.clone() }
    }

    #[inline]
    fn clone_from(&mut self, source: &Self) {
        self.base.clone_from(&source.base);
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<K, V, S, A> PartialEq for HashMap<K, V, S, A>
where
    K: Eq + Hash,
    V: PartialEq,
    S: BuildHasher,
    A: Allocator,
{
    fn eq(&self, other: &HashMap<K, V, S, A>) -> bool {
        if self.len() != other.len() {
            return false;
        }

        self.iter().all(|(key, value)| other.get(key).map_or(false, |v| *value == *v))
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<K, V, S, A> Eq for HashMap<K, V, S, A>
where
    K: Eq + Hash,
    V: Eq,
    S: BuildHasher,
    A: Allocator,
{
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<K, V, S, A> Debug for HashMap<K, V, S, A>
where
    K: Debug,
    V: Debug,
    A: Allocator,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_map().entries(self.iter()).finish()
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_unstable(feature = "const_default", issue = "143894")]
impl<K, V, S> const Default for HashMap<K, V, S>
where
    S: [const] Default,
{
    /// 创建一个空的 `HashMap<K, V, S>`，哈希器取其 `Default` 值。
    #[inline]
    fn default() -> HashMap<K, V, S> {
        HashMap::with_hasher(Default::default())
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<K, Q: ?Sized, V, S, A> Index<&Q> for HashMap<K, V, S, A>
where
    K: Eq + Hash + Borrow<Q>,
    Q: Eq + Hash,
    S: BuildHasher,
    A: Allocator,
{
    type Output = V;

    /// 返回所给键所对应的值的引用。
    ///
    /// # Panics
    ///
    /// 如果该键不在 `HashMap` 中，则 panic。
    #[inline]
    fn index(&self, key: &Q) -> &V {
        self.get(key).expect("no entry found for key")
    }
}

#[stable(feature = "std_collections_from_array", since = "1.56.0")]
// 注意：作为目前最便捷的内置 HashMap 构造方式，对本函数的简单使用绝不能*要求*用户
// 提供类型标注来推断第三个类型参数（哈希器参数，惯例上记作 "S"）。
// 为此，本 impl 使用 RandomState 作为 S 的具体类型来定义，而非对
// `S: BuildHasher + Default` 泛型化。
// 预期那些想要指定哈希器的用户会手动使用 `with_capacity_and_hasher`。
// 假如类型参数默认值能在 impl 上生效、且类型参数默认值能与 const 泛型混用，那么
// 或许可以将其泛化。
// 另见 HashSet 上等价的 impl。
impl<K, V, const N: usize> From<[(K, V); N]> for HashMap<K, V, RandomState>
where
    K: Eq + Hash,
{
    /// 将一个 `[(K, V); N]` 转换为 `HashMap<K, V>`。
    ///
    /// 如果数组中有任何 entry 的键相等，那么对应的值中除一个之外都会被丢弃。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashMap;
    ///
    /// let map1 = HashMap::from([(1, 2), (3, 4)]);
    /// let map2: HashMap<_, _> = [(1, 2), (3, 4)].into();
    /// assert_eq!(map1, map2);
    /// ```
    fn from(arr: [(K, V); N]) -> Self {
        Self::from_iter(arr)
    }
}

/// 一个遍历 `HashMap` 各 entry 的迭代器。
///
/// 此 `struct` 由 [`HashMap`] 上的 [`iter`] 方法创建。详见其文档。
///
/// [`iter`]: HashMap::iter
///
/// # 示例
///
/// ```
/// use std::collections::HashMap;
///
/// let map = HashMap::from([
///     ("a", 1),
/// ]);
/// let iter = map.iter();
/// ```
#[stable(feature = "rust1", since = "1.0.0")]
#[cfg_attr(not(test), rustc_diagnostic_item = "hashmap_iter_ty")]
pub struct Iter<'a, K: 'a, V: 'a> {
    base: base::Iter<'a, K, V>,
}

// FIXME(#26925) 改用 `#[derive(Clone)]` 后移除此实现
#[stable(feature = "rust1", since = "1.0.0")]
impl<K, V> Clone for Iter<'_, K, V> {
    #[inline]
    fn clone(&self) -> Self {
        Iter { base: self.base.clone() }
    }
}

#[stable(feature = "default_iters_hash", since = "1.83.0")]
impl<K, V> Default for Iter<'_, K, V> {
    #[inline]
    fn default() -> Self {
        Iter { base: Default::default() }
    }
}

#[stable(feature = "std_debug", since = "1.16.0")]
impl<K: Debug, V: Debug> fmt::Debug for Iter<'_, K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.clone()).finish()
    }
}

/// 一个以可变方式遍历 `HashMap` 各 entry 的迭代器。
///
/// 此 `struct` 由 [`HashMap`] 上的 [`iter_mut`] 方法创建。详见其文档。
///
/// [`iter_mut`]: HashMap::iter_mut
///
/// # 示例
///
/// ```
/// use std::collections::HashMap;
///
/// let mut map = HashMap::from([
///     ("a", 1),
/// ]);
/// let iter = map.iter_mut();
/// ```
#[stable(feature = "rust1", since = "1.0.0")]
#[cfg_attr(not(test), rustc_diagnostic_item = "hashmap_iter_mut_ty")]
pub struct IterMut<'a, K: 'a, V: 'a> {
    base: base::IterMut<'a, K, V>,
}

impl<'a, K, V> IterMut<'a, K, V> {
    /// 返回一个对剩余各项的引用迭代器。
    #[inline]
    pub(super) fn iter(&self) -> Iter<'_, K, V> {
        Iter { base: self.base.rustc_iter() }
    }
}

#[stable(feature = "default_iters_hash", since = "1.83.0")]
impl<K, V> Default for IterMut<'_, K, V> {
    #[inline]
    fn default() -> Self {
        IterMut { base: Default::default() }
    }
}

/// 一个拥有所有权、遍历 `HashMap` 各 entry 的迭代器。
///
/// 此 `struct` 由 [`HashMap`] 上的 [`into_iter`] 方法创建（由 [`IntoIterator`]
/// trait 提供）。详见其文档。
///
/// [`into_iter`]: IntoIterator::into_iter
///
/// # 示例
///
/// ```
/// use std::collections::HashMap;
///
/// let map = HashMap::from([
///     ("a", 1),
/// ]);
/// let iter = map.into_iter();
/// ```
#[stable(feature = "rust1", since = "1.0.0")]
pub struct IntoIter<
    K,
    V,
    #[unstable(feature = "allocator_api", issue = "32838")] A: Allocator = Global,
> {
    base: base::IntoIter<K, V, A>,
}

impl<K, V, A: Allocator> IntoIter<K, V, A> {
    /// 返回一个对剩余各项的引用迭代器。
    #[inline]
    pub(super) fn iter(&self) -> Iter<'_, K, V> {
        Iter { base: self.base.rustc_iter() }
    }
}

#[stable(feature = "default_iters_hash", since = "1.83.0")]
impl<K, V> Default for IntoIter<K, V> {
    #[inline]
    fn default() -> Self {
        IntoIter { base: Default::default() }
    }
}

/// 一个遍历 `HashMap` 各键的迭代器。
///
/// 此 `struct` 由 [`HashMap`] 上的 [`keys`] 方法创建。详见其文档。
///
/// [`keys`]: HashMap::keys
///
/// # 示例
///
/// ```
/// use std::collections::HashMap;
///
/// let map = HashMap::from([
///     ("a", 1),
/// ]);
/// let iter_keys = map.keys();
/// ```
#[stable(feature = "rust1", since = "1.0.0")]
#[cfg_attr(not(test), rustc_diagnostic_item = "hashmap_keys_ty")]
pub struct Keys<'a, K: 'a, V: 'a> {
    inner: Iter<'a, K, V>,
}

// FIXME(#26925) 移除它，改用 `#[derive(Clone)]`
#[stable(feature = "rust1", since = "1.0.0")]
impl<K, V> Clone for Keys<'_, K, V> {
    #[inline]
    fn clone(&self) -> Self {
        Keys { inner: self.inner.clone() }
    }
}

#[stable(feature = "default_iters_hash", since = "1.83.0")]
impl<K, V> Default for Keys<'_, K, V> {
    #[inline]
    fn default() -> Self {
        Keys { inner: Default::default() }
    }
}

#[stable(feature = "std_debug", since = "1.16.0")]
impl<K: Debug, V> fmt::Debug for Keys<'_, K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.clone()).finish()
    }
}

/// 一个遍历 `HashMap` 各值的迭代器。
///
/// 此 `struct` 由 [`HashMap`] 上的 [`values`] 方法创建。详见其文档。
///
/// [`values`]: HashMap::values
///
/// # 示例
///
/// ```
/// use std::collections::HashMap;
///
/// let map = HashMap::from([
///     ("a", 1),
/// ]);
/// let iter_values = map.values();
/// ```
#[stable(feature = "rust1", since = "1.0.0")]
#[cfg_attr(not(test), rustc_diagnostic_item = "hashmap_values_ty")]
pub struct Values<'a, K: 'a, V: 'a> {
    inner: Iter<'a, K, V>,
}

// FIXME(#26925) 移除它，改用 `#[derive(Clone)]`
#[stable(feature = "rust1", since = "1.0.0")]
impl<K, V> Clone for Values<'_, K, V> {
    #[inline]
    fn clone(&self) -> Self {
        Values { inner: self.inner.clone() }
    }
}

#[stable(feature = "default_iters_hash", since = "1.83.0")]
impl<K, V> Default for Values<'_, K, V> {
    #[inline]
    fn default() -> Self {
        Values { inner: Default::default() }
    }
}

#[stable(feature = "std_debug", since = "1.16.0")]
impl<K, V: Debug> fmt::Debug for Values<'_, K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.clone()).finish()
    }
}

/// 一个对 `HashMap` 各 entry 进行抽空（draining）的迭代器。
///
/// 此 `struct` 由 [`HashMap`] 上的 [`drain`] 方法创建。详见其文档。
///
/// [`drain`]: HashMap::drain
///
/// # 示例
///
/// ```
/// use std::collections::HashMap;
///
/// let mut map = HashMap::from([
///     ("a", 1),
/// ]);
/// let iter = map.drain();
/// ```
#[stable(feature = "drain", since = "1.6.0")]
#[cfg_attr(not(test), rustc_diagnostic_item = "hashmap_drain_ty")]
pub struct Drain<
    'a,
    K: 'a,
    V: 'a,
    #[unstable(feature = "allocator_api", issue = "32838")] A: Allocator = Global,
> {
    base: base::Drain<'a, K, V, A>,
}

impl<'a, K, V, A: Allocator> Drain<'a, K, V, A> {
    /// 返回一个对剩余各项的引用迭代器。
    #[inline]
    pub(super) fn iter(&self) -> Iter<'_, K, V> {
        Iter { base: self.base.rustc_iter() }
    }
}

/// 一个对 `HashMap` 各 entry 进行抽空并过滤（filtering）的迭代器。
///
/// 此 `struct` 由 [`HashMap`] 上的 [`extract_if`] 方法创建。
///
/// [`extract_if`]: HashMap::extract_if
///
/// # 示例
///
/// ```
/// use std::collections::HashMap;
///
/// let mut map = HashMap::from([
///     ("a", 1),
/// ]);
/// let iter = map.extract_if(|_k, v| *v % 2 == 0);
/// ```
#[stable(feature = "hash_extract_if", since = "1.88.0")]
#[must_use = "iterators are lazy and do nothing unless consumed; \
    use `retain` to remove and discard elements"]
pub struct ExtractIf<
    'a,
    K,
    V,
    F,
    #[unstable(feature = "allocator_api", issue = "32838")] A: Allocator = Global,
> {
    base: base::ExtractIf<'a, K, V, F, A>,
}

/// 一个以可变方式遍历 `HashMap` 各值的迭代器。
///
/// 此 `struct` 由 [`HashMap`] 上的 [`values_mut`] 方法创建。详见其文档。
///
/// [`values_mut`]: HashMap::values_mut
///
/// # 示例
///
/// ```
/// use std::collections::HashMap;
///
/// let mut map = HashMap::from([
///     ("a", 1),
/// ]);
/// let iter_values = map.values_mut();
/// ```
#[stable(feature = "map_values_mut", since = "1.10.0")]
#[cfg_attr(not(test), rustc_diagnostic_item = "hashmap_values_mut_ty")]
pub struct ValuesMut<'a, K: 'a, V: 'a> {
    inner: IterMut<'a, K, V>,
}

#[stable(feature = "default_iters_hash", since = "1.83.0")]
impl<K, V> Default for ValuesMut<'_, K, V> {
    #[inline]
    fn default() -> Self {
        ValuesMut { inner: Default::default() }
    }
}

/// 一个拥有所有权、遍历 `HashMap` 各键的迭代器。
///
/// 此 `struct` 由 [`HashMap`] 上的 [`into_keys`] 方法创建。详见其文档。
///
/// [`into_keys`]: HashMap::into_keys
///
/// # 示例
///
/// ```
/// use std::collections::HashMap;
///
/// let map = HashMap::from([
///     ("a", 1),
/// ]);
/// let iter_keys = map.into_keys();
/// ```
#[stable(feature = "map_into_keys_values", since = "1.54.0")]
pub struct IntoKeys<
    K,
    V,
    #[unstable(feature = "allocator_api", issue = "32838")] A: Allocator = Global,
> {
    inner: IntoIter<K, V, A>,
}

#[stable(feature = "default_iters_hash", since = "1.83.0")]
impl<K, V> Default for IntoKeys<K, V> {
    #[inline]
    fn default() -> Self {
        IntoKeys { inner: Default::default() }
    }
}

/// 一个拥有所有权、遍历 `HashMap` 各值的迭代器。
///
/// 此 `struct` 由 [`HashMap`] 上的 [`into_values`] 方法创建。详见其文档。
///
/// [`into_values`]: HashMap::into_values
///
/// # 示例
///
/// ```
/// use std::collections::HashMap;
///
/// let map = HashMap::from([
///     ("a", 1),
/// ]);
/// let iter_keys = map.into_values();
/// ```
#[stable(feature = "map_into_keys_values", since = "1.54.0")]
pub struct IntoValues<
    K,
    V,
    #[unstable(feature = "allocator_api", issue = "32838")] A: Allocator = Global,
> {
    inner: IntoIter<K, V, A>,
}

#[stable(feature = "default_iters_hash", since = "1.83.0")]
impl<K, V> Default for IntoValues<K, V> {
    #[inline]
    fn default() -> Self {
        IntoValues { inner: Default::default() }
    }
}

/// 对 map 中单个 entry 的视图（view），它可能是空缺的（vacant）或被占用的（occupied）。
///
/// 此 `enum` 由 [`HashMap`] 上的 [`entry`] 方法构造。
///
/// [`entry`]: HashMap::entry
#[stable(feature = "rust1", since = "1.0.0")]
#[cfg_attr(not(test), rustc_diagnostic_item = "HashMapEntry")]
pub enum Entry<
    'a,
    K: 'a,
    V: 'a,
    #[unstable(feature = "allocator_api", issue = "32838")] A: Allocator = Global,
> {
    /// 一个被占用的 entry。
    #[stable(feature = "rust1", since = "1.0.0")]
    Occupied(#[stable(feature = "rust1", since = "1.0.0")] OccupiedEntry<'a, K, V, A>),

    /// 一个空缺的 entry。
    #[stable(feature = "rust1", since = "1.0.0")]
    Vacant(#[stable(feature = "rust1", since = "1.0.0")] VacantEntry<'a, K, V, A>),
}

#[stable(feature = "debug_hash_map", since = "1.12.0")]
impl<K: Debug, V: Debug> Debug for Entry<'_, K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Vacant(ref v) => f.debug_tuple("Entry").field(v).finish(),
            Occupied(ref o) => f.debug_tuple("Entry").field(o).finish(),
        }
    }
}

/// 对 `HashMap` 中一个被占用 entry 的视图。
/// 它是 [`Entry`] 枚举的组成部分。
#[stable(feature = "rust1", since = "1.0.0")]
pub struct OccupiedEntry<
    'a,
    K: 'a,
    V: 'a,
    #[unstable(feature = "allocator_api", issue = "32838")] A: Allocator = Global,
> {
    base: base::RustcOccupiedEntry<'a, K, V, A>,
}

#[stable(feature = "debug_hash_map", since = "1.12.0")]
impl<K: Debug, V: Debug, A: Allocator> Debug for OccupiedEntry<'_, K, V, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OccupiedEntry")
            .field("key", self.key())
            .field("value", self.get())
            .finish_non_exhaustive()
    }
}

/// 对 `HashMap` 中一个空缺 entry 的视图。
/// 它是 [`Entry`] 枚举的组成部分。
#[stable(feature = "rust1", since = "1.0.0")]
pub struct VacantEntry<
    'a,
    K: 'a,
    V: 'a,
    #[unstable(feature = "allocator_api", issue = "32838")] A: Allocator = Global,
> {
    base: base::RustcVacantEntry<'a, K, V, A>,
}

#[stable(feature = "debug_hash_map", since = "1.12.0")]
impl<K: Debug, V, A: Allocator> Debug for VacantEntry<'_, K, V, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("VacantEntry").field(self.key()).finish()
    }
}

/// 当键已经存在时，[`try_insert`](HashMap::try_insert) 所返回的错误。
///
/// 其中包含被占用的 entry，以及那个未被插入的值。
#[unstable(feature = "map_try_insert", issue = "82766")]
pub struct OccupiedError<
    'a,
    K: 'a,
    V: 'a,
    #[unstable(feature = "allocator_api", issue = "32838")] A: Allocator = Global,
> {
    /// map 中那个已被占用的 entry。
    pub entry: OccupiedEntry<'a, K, V, A>,
    /// 因为 entry 已被占用而未被插入的那个值。
    pub value: V,
}

#[unstable(feature = "map_try_insert", issue = "82766")]
impl<K: Debug, V: Debug, A: Allocator> Debug for OccupiedError<'_, K, V, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OccupiedError")
            .field("key", self.entry.key())
            .field("old_value", self.entry.get())
            .field("new_value", &self.value)
            .finish_non_exhaustive()
    }
}

#[unstable(feature = "map_try_insert", issue = "82766")]
impl<'a, K: Debug, V: Debug, A: Allocator> fmt::Display for OccupiedError<'a, K, V, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "failed to insert {:?}, key {:?} already exists with value {:?}",
            self.value,
            self.entry.key(),
            self.entry.get(),
        )
    }
}

#[unstable(feature = "map_try_insert", issue = "82766")]
impl<'a, K: Debug, V: Debug, A: Allocator> Error for OccupiedError<'a, K, V, A> {}

#[stable(feature = "rust1", since = "1.0.0")]
impl<'a, K, V, S, A: Allocator> IntoIterator for &'a HashMap<K, V, S, A> {
    type Item = (&'a K, &'a V);
    type IntoIter = Iter<'a, K, V>;

    #[inline]
    #[rustc_lint_query_instability]
    fn into_iter(self) -> Iter<'a, K, V> {
        self.iter()
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<'a, K, V, S, A: Allocator> IntoIterator for &'a mut HashMap<K, V, S, A> {
    type Item = (&'a K, &'a mut V);
    type IntoIter = IterMut<'a, K, V>;

    #[inline]
    #[rustc_lint_query_instability]
    fn into_iter(self) -> IterMut<'a, K, V> {
        self.iter_mut()
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<K, V, S, A: Allocator> IntoIterator for HashMap<K, V, S, A> {
    type Item = (K, V);
    type IntoIter = IntoIter<K, V, A>;

    /// 创建一个消耗型迭代器，也就是说，它以任意顺序将每个键值对移出该 map。
    /// 调用之后该 map 不能再被使用。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashMap;
    ///
    /// let map = HashMap::from([
    ///     ("a", 1),
    ///     ("b", 2),
    ///     ("c", 3),
    /// ]);
    ///
    /// // 用 .iter() 做不到这一点
    /// let vec: Vec<(&str, i32)> = map.into_iter().collect();
    /// ```
    #[inline]
    #[rustc_lint_query_instability]
    fn into_iter(self) -> IntoIter<K, V, A> {
        IntoIter { base: self.base.into_iter() }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<'a, K, V> Iterator for Iter<'a, K, V> {
    type Item = (&'a K, &'a V);

    #[inline]
    fn next(&mut self) -> Option<(&'a K, &'a V)> {
        self.base.next()
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.base.size_hint()
    }
    #[inline]
    fn count(self) -> usize {
        self.base.len()
    }
    #[inline]
    fn fold<B, F>(self, init: B, f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        self.base.fold(init, f)
    }
}
#[stable(feature = "rust1", since = "1.0.0")]
impl<K, V> ExactSizeIterator for Iter<'_, K, V> {
    #[inline]
    fn len(&self) -> usize {
        self.base.len()
    }
}

#[stable(feature = "fused", since = "1.26.0")]
impl<K, V> FusedIterator for Iter<'_, K, V> {}

#[stable(feature = "rust1", since = "1.0.0")]
impl<'a, K, V> Iterator for IterMut<'a, K, V> {
    type Item = (&'a K, &'a mut V);

    #[inline]
    fn next(&mut self) -> Option<(&'a K, &'a mut V)> {
        self.base.next()
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.base.size_hint()
    }
    #[inline]
    fn count(self) -> usize {
        self.base.len()
    }
    #[inline]
    fn fold<B, F>(self, init: B, f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        self.base.fold(init, f)
    }
}
#[stable(feature = "rust1", since = "1.0.0")]
impl<K, V> ExactSizeIterator for IterMut<'_, K, V> {
    #[inline]
    fn len(&self) -> usize {
        self.base.len()
    }
}
#[stable(feature = "fused", since = "1.26.0")]
impl<K, V> FusedIterator for IterMut<'_, K, V> {}

#[stable(feature = "std_debug", since = "1.16.0")]
impl<K, V> fmt::Debug for IterMut<'_, K, V>
where
    K: fmt::Debug,
    V: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<K, V, A: Allocator> Iterator for IntoIter<K, V, A> {
    type Item = (K, V);

    #[inline]
    fn next(&mut self) -> Option<(K, V)> {
        self.base.next()
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.base.size_hint()
    }
    #[inline]
    fn count(self) -> usize {
        self.base.len()
    }
    #[inline]
    fn fold<B, F>(self, init: B, f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        self.base.fold(init, f)
    }
}
#[stable(feature = "rust1", since = "1.0.0")]
impl<K, V, A: Allocator> ExactSizeIterator for IntoIter<K, V, A> {
    #[inline]
    fn len(&self) -> usize {
        self.base.len()
    }
}
#[stable(feature = "fused", since = "1.26.0")]
impl<K, V, A: Allocator> FusedIterator for IntoIter<K, V, A> {}

#[stable(feature = "std_debug", since = "1.16.0")]
impl<K: Debug, V: Debug, A: Allocator> fmt::Debug for IntoIter<K, V, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<'a, K, V> Iterator for Keys<'a, K, V> {
    type Item = &'a K;

    #[inline]
    fn next(&mut self) -> Option<&'a K> {
        self.inner.next().map(|(k, _)| k)
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
    #[inline]
    fn count(self) -> usize {
        self.inner.len()
    }
    #[inline]
    fn fold<B, F>(self, init: B, mut f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        self.inner.fold(init, |acc, (k, _)| f(acc, k))
    }
}
#[stable(feature = "rust1", since = "1.0.0")]
impl<K, V> ExactSizeIterator for Keys<'_, K, V> {
    #[inline]
    fn len(&self) -> usize {
        self.inner.len()
    }
}
#[stable(feature = "fused", since = "1.26.0")]
impl<K, V> FusedIterator for Keys<'_, K, V> {}

#[stable(feature = "rust1", since = "1.0.0")]
impl<'a, K, V> Iterator for Values<'a, K, V> {
    type Item = &'a V;

    #[inline]
    fn next(&mut self) -> Option<&'a V> {
        self.inner.next().map(|(_, v)| v)
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
    #[inline]
    fn count(self) -> usize {
        self.inner.len()
    }
    #[inline]
    fn fold<B, F>(self, init: B, mut f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        self.inner.fold(init, |acc, (_, v)| f(acc, v))
    }
}
#[stable(feature = "rust1", since = "1.0.0")]
impl<K, V> ExactSizeIterator for Values<'_, K, V> {
    #[inline]
    fn len(&self) -> usize {
        self.inner.len()
    }
}
#[stable(feature = "fused", since = "1.26.0")]
impl<K, V> FusedIterator for Values<'_, K, V> {}

#[stable(feature = "map_values_mut", since = "1.10.0")]
impl<'a, K, V> Iterator for ValuesMut<'a, K, V> {
    type Item = &'a mut V;

    #[inline]
    fn next(&mut self) -> Option<&'a mut V> {
        self.inner.next().map(|(_, v)| v)
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
    #[inline]
    fn count(self) -> usize {
        self.inner.len()
    }
    #[inline]
    fn fold<B, F>(self, init: B, mut f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        self.inner.fold(init, |acc, (_, v)| f(acc, v))
    }
}
#[stable(feature = "map_values_mut", since = "1.10.0")]
impl<K, V> ExactSizeIterator for ValuesMut<'_, K, V> {
    #[inline]
    fn len(&self) -> usize {
        self.inner.len()
    }
}
#[stable(feature = "fused", since = "1.26.0")]
impl<K, V> FusedIterator for ValuesMut<'_, K, V> {}

#[stable(feature = "std_debug", since = "1.16.0")]
impl<K, V: fmt::Debug> fmt::Debug for ValuesMut<'_, K, V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.inner.iter().map(|(_, val)| val)).finish()
    }
}

#[stable(feature = "map_into_keys_values", since = "1.54.0")]
impl<K, V, A: Allocator> Iterator for IntoKeys<K, V, A> {
    type Item = K;

    #[inline]
    fn next(&mut self) -> Option<K> {
        self.inner.next().map(|(k, _)| k)
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
    #[inline]
    fn count(self) -> usize {
        self.inner.len()
    }
    #[inline]
    fn fold<B, F>(self, init: B, mut f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        self.inner.fold(init, |acc, (k, _)| f(acc, k))
    }
}
#[stable(feature = "map_into_keys_values", since = "1.54.0")]
impl<K, V, A: Allocator> ExactSizeIterator for IntoKeys<K, V, A> {
    #[inline]
    fn len(&self) -> usize {
        self.inner.len()
    }
}
#[stable(feature = "map_into_keys_values", since = "1.54.0")]
impl<K, V, A: Allocator> FusedIterator for IntoKeys<K, V, A> {}

#[stable(feature = "map_into_keys_values", since = "1.54.0")]
impl<K: Debug, V, A: Allocator> fmt::Debug for IntoKeys<K, V, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.inner.iter().map(|(k, _)| k)).finish()
    }
}

#[stable(feature = "map_into_keys_values", since = "1.54.0")]
impl<K, V, A: Allocator> Iterator for IntoValues<K, V, A> {
    type Item = V;

    #[inline]
    fn next(&mut self) -> Option<V> {
        self.inner.next().map(|(_, v)| v)
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
    #[inline]
    fn count(self) -> usize {
        self.inner.len()
    }
    #[inline]
    fn fold<B, F>(self, init: B, mut f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        self.inner.fold(init, |acc, (_, v)| f(acc, v))
    }
}
#[stable(feature = "map_into_keys_values", since = "1.54.0")]
impl<K, V, A: Allocator> ExactSizeIterator for IntoValues<K, V, A> {
    #[inline]
    fn len(&self) -> usize {
        self.inner.len()
    }
}
#[stable(feature = "map_into_keys_values", since = "1.54.0")]
impl<K, V, A: Allocator> FusedIterator for IntoValues<K, V, A> {}

#[stable(feature = "map_into_keys_values", since = "1.54.0")]
impl<K, V: Debug, A: Allocator> fmt::Debug for IntoValues<K, V, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.inner.iter().map(|(_, v)| v)).finish()
    }
}

#[stable(feature = "drain", since = "1.6.0")]
impl<'a, K, V, A: Allocator> Iterator for Drain<'a, K, V, A> {
    type Item = (K, V);

    #[inline]
    fn next(&mut self) -> Option<(K, V)> {
        self.base.next()
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.base.size_hint()
    }
    #[inline]
    fn fold<B, F>(self, init: B, f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        self.base.fold(init, f)
    }
}
#[stable(feature = "drain", since = "1.6.0")]
impl<K, V, A: Allocator> ExactSizeIterator for Drain<'_, K, V, A> {
    #[inline]
    fn len(&self) -> usize {
        self.base.len()
    }
}
#[stable(feature = "fused", since = "1.26.0")]
impl<K, V, A: Allocator> FusedIterator for Drain<'_, K, V, A> {}

#[stable(feature = "std_debug", since = "1.16.0")]
impl<K, V, A: Allocator> fmt::Debug for Drain<'_, K, V, A>
where
    K: fmt::Debug,
    V: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

#[stable(feature = "hash_extract_if", since = "1.88.0")]
impl<K, V, F, A: Allocator> Iterator for ExtractIf<'_, K, V, F, A>
where
    F: FnMut(&K, &mut V) -> bool,
{
    type Item = (K, V);

    #[inline]
    fn next(&mut self) -> Option<(K, V)> {
        self.base.next()
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.base.size_hint()
    }
}

#[stable(feature = "hash_extract_if", since = "1.88.0")]
impl<K, V, F, A: Allocator> FusedIterator for ExtractIf<'_, K, V, F, A> where
    F: FnMut(&K, &mut V) -> bool
{
}

#[stable(feature = "hash_extract_if", since = "1.88.0")]
impl<K, V, F, A: Allocator> fmt::Debug for ExtractIf<'_, K, V, F, A>
where
    K: fmt::Debug,
    V: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExtractIf").finish_non_exhaustive()
    }
}

impl<'a, K, V, A: Allocator> Entry<'a, K, V, A> {
    /// 确保 entry 中存在一个值：若为空缺则插入 default，并返回该 entry 中值的可变引用。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashMap;
    ///
    /// let mut map: HashMap<&str, u32> = HashMap::new();
    ///
    /// map.entry("poneyland").or_insert(3);
    /// assert_eq!(map["poneyland"], 3);
    ///
    /// *map.entry("poneyland").or_insert(10) *= 2;
    /// assert_eq!(map["poneyland"], 6);
    /// ```
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn or_insert(self, default: V) -> &'a mut V {
        match self {
            Occupied(entry) => entry.into_mut(),
            Vacant(entry) => entry.insert(default),
        }
    }

    /// 确保 entry 中存在一个值：若为空缺则插入默认函数（default function）的返回值，
    /// 并返回该 entry 中值的可变引用。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashMap;
    ///
    /// let mut map = HashMap::new();
    /// let value = "hoho";
    ///
    /// map.entry("poneyland").or_insert_with(|| value);
    ///
    /// assert_eq!(map["poneyland"], "hoho");
    /// ```
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn or_insert_with<F: FnOnce() -> V>(self, default: F) -> &'a mut V {
        match self {
            Occupied(entry) => entry.into_mut(),
            Vacant(entry) => entry.insert(default()),
        }
    }

    /// 确保 entry 中存在一个值：若为空缺则插入默认函数的返回值。
    /// 本方法会向默认函数提供一个对那个在 `.entry(key)` 方法调用期间被移动的键的引用，
    /// 从而允许生成由键派生而来的待插入值。
    ///
    /// 之所以提供这个对被移动键的引用，是为了避免克隆或拷贝键的必要——这与
    /// `.or_insert_with(|| ... )` 不同。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashMap;
    ///
    /// let mut map: HashMap<&str, usize> = HashMap::new();
    ///
    /// map.entry("poneyland").or_insert_with_key(|key| key.chars().count());
    ///
    /// assert_eq!(map["poneyland"], 9);
    /// ```
    #[inline]
    #[stable(feature = "or_insert_with_key", since = "1.50.0")]
    pub fn or_insert_with_key<F: FnOnce(&K) -> V>(self, default: F) -> &'a mut V {
        match self {
            Occupied(entry) => entry.into_mut(),
            Vacant(entry) => {
                let value = default(entry.key());
                entry.insert(value)
            }
        }
    }

    /// 返回此 entry 的键的引用。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashMap;
    ///
    /// let mut map: HashMap<&str, u32> = HashMap::new();
    /// assert_eq!(map.entry("poneyland").key(), &"poneyland");
    /// ```
    #[inline]
    #[stable(feature = "map_entry_keys", since = "1.10.0")]
    pub fn key(&self) -> &K {
        match *self {
            Occupied(ref entry) => entry.key(),
            Vacant(ref entry) => entry.key(),
        }
    }

    /// 在向 map 进行任何可能的插入之前，提供对被占用 entry 的就地可变访问。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashMap;
    ///
    /// let mut map: HashMap<&str, u32> = HashMap::new();
    ///
    /// map.entry("poneyland")
    ///    .and_modify(|e| { *e += 1 })
    ///    .or_insert(42);
    /// assert_eq!(map["poneyland"], 42);
    ///
    /// map.entry("poneyland")
    ///    .and_modify(|e| { *e += 1 })
    ///    .or_insert(42);
    /// assert_eq!(map["poneyland"], 43);
    /// ```
    #[inline]
    #[stable(feature = "entry_and_modify", since = "1.26.0")]
    pub fn and_modify<F>(self, f: F) -> Self
    where
        F: FnOnce(&mut V),
    {
        match self {
            Occupied(mut entry) => {
                f(entry.get_mut());
                Occupied(entry)
            }
            Vacant(entry) => Vacant(entry),
        }
    }

    /// 设置该 entry 的值，并返回一个 `OccupiedEntry`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashMap;
    ///
    /// let mut map: HashMap<&str, String> = HashMap::new();
    /// let entry = map.entry("poneyland").insert_entry("hoho".to_string());
    ///
    /// assert_eq!(entry.key(), &"poneyland");
    /// ```
    #[inline]
    #[stable(feature = "entry_insert", since = "1.83.0")]
    pub fn insert_entry(self, value: V) -> OccupiedEntry<'a, K, V, A> {
        match self {
            Occupied(mut entry) => {
                entry.insert(value);
                entry
            }
            Vacant(entry) => entry.insert_entry(value),
        }
    }
}

impl<'a, K, V: Default> Entry<'a, K, V> {
    /// 确保 entry 中存在一个值：若为空缺则插入默认值（default value），并返回该
    /// entry 中值的可变引用。
    ///
    /// # 示例
    ///
    /// ```
    /// # fn main() {
    /// use std::collections::HashMap;
    ///
    /// let mut map: HashMap<&str, Option<u32>> = HashMap::new();
    /// map.entry("poneyland").or_default();
    ///
    /// assert_eq!(map["poneyland"], None);
    /// # }
    /// ```
    #[inline]
    #[stable(feature = "entry_or_default", since = "1.28.0")]
    pub fn or_default(self) -> &'a mut V {
        match self {
            Occupied(entry) => entry.into_mut(),
            Vacant(entry) => entry.insert(Default::default()),
        }
    }
}

impl<'a, K, V, A: Allocator> OccupiedEntry<'a, K, V, A> {
    /// 获取该 entry 中键的引用。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashMap;
    ///
    /// let mut map: HashMap<&str, u32> = HashMap::new();
    /// map.entry("poneyland").or_insert(12);
    /// assert_eq!(map.entry("poneyland").key(), &"poneyland");
    /// ```
    #[inline]
    #[stable(feature = "map_entry_keys", since = "1.10.0")]
    pub fn key(&self) -> &K {
        self.base.key()
    }

    /// 从 map 中取得键和值的所有权。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashMap;
    /// use std::collections::hash_map::Entry;
    ///
    /// let mut map: HashMap<&str, u32> = HashMap::new();
    /// map.entry("poneyland").or_insert(12);
    ///
    /// if let Entry::Occupied(o) = map.entry("poneyland") {
    ///     // 我们从 map 中删除这个 entry。
    ///     o.remove_entry();
    /// }
    ///
    /// assert_eq!(map.contains_key("poneyland"), false);
    /// ```
    #[inline]
    #[stable(feature = "map_entry_recover_keys2", since = "1.12.0")]
    pub fn remove_entry(self) -> (K, V) {
        self.base.remove_entry()
    }

    /// 获取该 entry 中值的引用。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashMap;
    /// use std::collections::hash_map::Entry;
    ///
    /// let mut map: HashMap<&str, u32> = HashMap::new();
    /// map.entry("poneyland").or_insert(12);
    ///
    /// if let Entry::Occupied(o) = map.entry("poneyland") {
    ///     assert_eq!(o.get(), &12);
    /// }
    /// ```
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn get(&self) -> &V {
        self.base.get()
    }

    /// 获取该 entry 中值的可变引用。
    ///
    /// 如果你需要一个生命周期可能超出 `Entry` 值销毁时刻的、对 `OccupiedEntry` 的
    /// 引用，参见 [`into_mut`]。
    ///
    /// [`into_mut`]: Self::into_mut
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashMap;
    /// use std::collections::hash_map::Entry;
    ///
    /// let mut map: HashMap<&str, u32> = HashMap::new();
    /// map.entry("poneyland").or_insert(12);
    ///
    /// assert_eq!(map["poneyland"], 12);
    /// if let Entry::Occupied(mut o) = map.entry("poneyland") {
    ///     *o.get_mut() += 10;
    ///     assert_eq!(*o.get(), 22);
    ///
    ///     // 我们可以多次使用同一个 Entry。
    ///     *o.get_mut() += 2;
    /// }
    ///
    /// assert_eq!(map["poneyland"], 24);
    /// ```
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn get_mut(&mut self) -> &mut V {
        self.base.get_mut()
    }

    /// 将 `OccupiedEntry` 转换为一个对 entry 中值的可变引用，其生命周期绑定到 map
    /// 自身。
    ///
    /// 如果你需要对 `OccupiedEntry` 的多个引用，参见 [`get_mut`]。
    ///
    /// [`get_mut`]: Self::get_mut
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashMap;
    /// use std::collections::hash_map::Entry;
    ///
    /// let mut map: HashMap<&str, u32> = HashMap::new();
    /// map.entry("poneyland").or_insert(12);
    ///
    /// assert_eq!(map["poneyland"], 12);
    /// if let Entry::Occupied(o) = map.entry("poneyland") {
    ///     *o.into_mut() += 10;
    /// }
    ///
    /// assert_eq!(map["poneyland"], 22);
    /// ```
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn into_mut(self) -> &'a mut V {
        self.base.into_mut()
    }

    /// 设置该 entry 的值，并返回该 entry 的旧值。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashMap;
    /// use std::collections::hash_map::Entry;
    ///
    /// let mut map: HashMap<&str, u32> = HashMap::new();
    /// map.entry("poneyland").or_insert(12);
    ///
    /// if let Entry::Occupied(mut o) = map.entry("poneyland") {
    ///     assert_eq!(o.insert(15), 12);
    /// }
    ///
    /// assert_eq!(map["poneyland"], 15);
    /// ```
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn insert(&mut self, value: V) -> V {
        self.base.insert(value)
    }

    /// 将值从该 entry 中取出，并返回它。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashMap;
    /// use std::collections::hash_map::Entry;
    ///
    /// let mut map: HashMap<&str, u32> = HashMap::new();
    /// map.entry("poneyland").or_insert(12);
    ///
    /// if let Entry::Occupied(o) = map.entry("poneyland") {
    ///     assert_eq!(o.remove(), 12);
    /// }
    ///
    /// assert_eq!(map.contains_key("poneyland"), false);
    /// ```
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn remove(self) -> V {
        self.base.remove()
    }
}

impl<'a, K: 'a, V: 'a, A: Allocator> VacantEntry<'a, K, V, A> {
    /// 获取一个对将通过该 `VacantEntry` 插入值时所用键的引用。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashMap;
    ///
    /// let mut map: HashMap<&str, u32> = HashMap::new();
    /// assert_eq!(map.entry("poneyland").key(), &"poneyland");
    /// ```
    #[inline]
    #[stable(feature = "map_entry_keys", since = "1.10.0")]
    pub fn key(&self) -> &K {
        self.base.key()
    }

    /// 取得该键的所有权。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashMap;
    /// use std::collections::hash_map::Entry;
    ///
    /// let mut map: HashMap<&str, u32> = HashMap::new();
    ///
    /// if let Entry::Vacant(v) = map.entry("poneyland") {
    ///     v.into_key();
    /// }
    /// ```
    #[inline]
    #[stable(feature = "map_entry_recover_keys2", since = "1.12.0")]
    pub fn into_key(self) -> K {
        self.base.into_key()
    }

    /// 以该 `VacantEntry` 的键设置该 entry 的值，并返回一个对它的可变引用。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashMap;
    /// use std::collections::hash_map::Entry;
    ///
    /// let mut map: HashMap<&str, u32> = HashMap::new();
    ///
    /// if let Entry::Vacant(o) = map.entry("poneyland") {
    ///     o.insert(37);
    /// }
    /// assert_eq!(map["poneyland"], 37);
    /// ```
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn insert(self, value: V) -> &'a mut V {
        self.base.insert(value)
    }

    /// 以该 `VacantEntry` 的键设置该 entry 的值，并返回一个 `OccupiedEntry`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashMap;
    /// use std::collections::hash_map::Entry;
    ///
    /// let mut map: HashMap<&str, u32> = HashMap::new();
    ///
    /// if let Entry::Vacant(o) = map.entry("poneyland") {
    ///     o.insert_entry(37);
    /// }
    /// assert_eq!(map["poneyland"], 37);
    /// ```
    #[inline]
    #[stable(feature = "entry_insert", since = "1.83.0")]
    pub fn insert_entry(self, value: V) -> OccupiedEntry<'a, K, V, A> {
        let base = self.base.insert_entry(value);
        OccupiedEntry { base }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<K, V, S> FromIterator<(K, V)> for HashMap<K, V, S>
where
    K: Eq + Hash,
    S: BuildHasher + Default,
{
    /// 从一个键值对迭代器构造一个 `HashMap<K, V>`。
    ///
    /// 如果该迭代器产出任何键相等的对，那么对应的值中除一个之外都会被丢弃。
    fn from_iter<T: IntoIterator<Item = (K, V)>>(iter: T) -> HashMap<K, V, S> {
        let mut map = HashMap::with_hasher(Default::default());
        map.extend(iter);
        map
    }
}

/// 插入迭代器中所有新的键值对，并把已存在键的值替换为迭代器返回的新值。
#[stable(feature = "rust1", since = "1.0.0")]
impl<K, V, S, A> Extend<(K, V)> for HashMap<K, V, S, A>
where
    K: Eq + Hash,
    S: BuildHasher,
    A: Allocator,
{
    #[inline]
    fn extend<T: IntoIterator<Item = (K, V)>>(&mut self, iter: T) {
        self.base.extend(iter)
    }

    #[inline]
    fn extend_one(&mut self, (k, v): (K, V)) {
        self.base.insert(k, v);
    }

    #[inline]
    fn extend_reserve(&mut self, additional: usize) {
        self.base.extend_reserve(additional);
    }
}

#[stable(feature = "hash_extend_copy", since = "1.4.0")]
impl<'a, K, V, S, A> Extend<(&'a K, &'a V)> for HashMap<K, V, S, A>
where
    K: Eq + Hash + Copy,
    V: Copy,
    S: BuildHasher,
    A: Allocator,
{
    #[inline]
    fn extend<T: IntoIterator<Item = (&'a K, &'a V)>>(&mut self, iter: T) {
        self.base.extend(iter)
    }

    #[inline]
    fn extend_one(&mut self, (&k, &v): (&'a K, &'a V)) {
        self.base.insert(k, v);
    }

    #[inline]
    fn extend_reserve(&mut self, additional: usize) {
        Extend::<(K, V)>::extend_reserve(self, additional)
    }
}

#[inline]
fn map_entry<'a, K: 'a, V: 'a, A: Allocator>(
    raw: base::RustcEntry<'a, K, V, A>,
) -> Entry<'a, K, V, A> {
    match raw {
        base::RustcEntry::Occupied(base) => Entry::Occupied(OccupiedEntry { base }),
        base::RustcEntry::Vacant(base) => Entry::Vacant(VacantEntry { base }),
    }
}

#[inline]
pub(super) fn map_try_reserve_error(err: hashbrown::TryReserveError) -> TryReserveError {
    match err {
        hashbrown::TryReserveError::CapacityOverflow => {
            TryReserveErrorKind::CapacityOverflow.into()
        }
        hashbrown::TryReserveError::AllocError { layout } => {
            TryReserveErrorKind::AllocError { layout, non_exhaustive: () }.into()
        }
    }
}

#[allow(dead_code)]
fn assert_covariance() {
    fn map_key<'new>(v: HashMap<&'static str, u8>) -> HashMap<&'new str, u8> {
        v
    }
    fn map_val<'new>(v: HashMap<u8, &'static str>) -> HashMap<u8, &'new str> {
        v
    }
    fn iter_key<'a, 'new>(v: Iter<'a, &'static str, u8>) -> Iter<'a, &'new str, u8> {
        v
    }
    fn iter_val<'a, 'new>(v: Iter<'a, u8, &'static str>) -> Iter<'a, u8, &'new str> {
        v
    }
    fn into_iter_key<'new>(v: IntoIter<&'static str, u8>) -> IntoIter<&'new str, u8> {
        v
    }
    fn into_iter_val<'new>(v: IntoIter<u8, &'static str>) -> IntoIter<u8, &'new str> {
        v
    }
    fn keys_key<'a, 'new>(v: Keys<'a, &'static str, u8>) -> Keys<'a, &'new str, u8> {
        v
    }
    fn keys_val<'a, 'new>(v: Keys<'a, u8, &'static str>) -> Keys<'a, u8, &'new str> {
        v
    }
    fn values_key<'a, 'new>(v: Values<'a, &'static str, u8>) -> Values<'a, &'new str, u8> {
        v
    }
    fn values_val<'a, 'new>(v: Values<'a, u8, &'static str>) -> Values<'a, u8, &'new str> {
        v
    }
    fn drain<'new>(
        d: Drain<'static, &'static str, &'static str>,
    ) -> Drain<'new, &'new str, &'new str> {
        d
    }
}
