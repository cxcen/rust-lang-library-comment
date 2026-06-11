#[cfg(test)]
mod tests;

use hashbrown::hash_set as base;

use super::map::map_try_reserve_error;
use crate::alloc::{Allocator, Global};
use crate::borrow::Borrow;
use crate::collections::TryReserveError;
use crate::fmt;
use crate::hash::{BuildHasher, Hash, RandomState};
use crate::iter::{Chain, FusedIterator};
use crate::ops::{BitAnd, BitOr, BitXor, Sub};

/// 一个 [hash set]（哈希集合），实现为值类型为 `()` 的 `HashMap`。
///
/// 与 [`HashMap`] 类型一样，`HashSet` 要求其元素实现 [`Eq`] 与 [`Hash`] trait。
/// 这通常可以通过使用 `#[derive(PartialEq, Eq, Hash)]` 来达成。如果你要自己实现
/// 它们，那么务必保证下面这条性质成立：
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
/// 观测到此逻辑错误的 `HashSet` 内部，不会导致未定义行为（undefined behavior）。
/// 这可能包括 panic、错误的结果、abort、内存泄漏以及不终止（non-termination）。
///
/// # 示例
///
/// ```
/// use std::collections::HashSet;
/// // 类型推断让我们可以省略显式的类型标注（在本例中应为
/// // `HashSet<String>`）。
/// let mut books = HashSet::new();
///
/// // 添加一些书籍。
/// books.insert("A Dance With Dragons".to_string());
/// books.insert("To Kill a Mockingbird".to_string());
/// books.insert("The Odyssey".to_string());
/// books.insert("The Great Gatsby".to_string());
///
/// // 检查某个特定的元素。
/// if !books.contains("The Winds of Winter") {
///     println!("We have {} books, but The Winds of Winter ain't one.",
///              books.len());
/// }
///
/// // 移除一本书。
/// books.remove("The Odyssey");
///
/// // 遍历所有内容。
/// for book in &books {
///     println!("{book}");
/// }
/// ```
///
/// 将 `HashSet` 与自定义类型搭配使用，最简单的方式是 derive [`Eq`] 与 [`Hash`]。
/// 我们还必须 derive [`PartialEq`]——一旦 derive 了 [`Eq`]，就要求它。
///
/// ```
/// use std::collections::HashSet;
/// #[derive(Hash, Eq, PartialEq, Debug)]
/// struct Viking {
///     name: String,
///     power: usize,
/// }
///
/// let mut vikings = HashSet::new();
///
/// vikings.insert(Viking { name: "Einar".to_string(), power: 9 });
/// vikings.insert(Viking { name: "Einar".to_string(), power: 9 });
/// vikings.insert(Viking { name: "Olaf".to_string(), power: 4 });
/// vikings.insert(Viking { name: "Harald".to_string(), power: 8 });
///
/// // 使用 derive 得到的实现打印这些 viking。
/// for x in &vikings {
///     println!("{x:?}");
/// }
/// ```
///
/// 当项目列表已知时，`HashSet` 可以从数组初始化：
///
/// ```
/// use std::collections::HashSet;
///
/// let viking_names = HashSet::from(["Einar", "Olaf", "Harald"]);
/// ```
///
/// [hash set]: crate::collections#use-the-set-variant-of-any-of-these-maps-when
/// [`HashMap`]: crate::collections::HashMap
/// [`RefCell`]: crate::cell::RefCell
/// [`Cell`]: crate::cell::Cell
///
/// # 在 `const` 与 `static` 中使用
///
/// 与 `HashMap` 一样，`HashSet` 使用随机种子：每个 `HashSet` 实例都使用不同的种子，
/// 这意味着 `HashSet::new` 无法用在 const 上下文中。要在 `const` 或 `static` 项的
/// 初始化器中构造一个 `HashSet`，你将不得不使用一个不涉及随机种子的哈希器，如下面的
/// 示例所示。**以这种方式构造的 `HashSet` 无法抵御 HashDoS！**
///
/// ```rust
/// use std::collections::HashSet;
/// use std::hash::{BuildHasherDefault, DefaultHasher};
/// use std::sync::Mutex;
///
/// const EMPTY_SET: HashSet<String, BuildHasherDefault<DefaultHasher>> =
///     HashSet::with_hasher(BuildHasherDefault::new());
/// static SET: Mutex<HashSet<String, BuildHasherDefault<DefaultHasher>>> =
///     Mutex::new(HashSet::with_hasher(BuildHasherDefault::new()));
/// ```
#[cfg_attr(not(test), rustc_diagnostic_item = "HashSet")]
#[stable(feature = "rust1", since = "1.0.0")]
pub struct HashSet<
    T,
    S = RandomState,
    #[unstable(feature = "allocator_api", issue = "32838")] A: Allocator = Global,
> {
    base: base::HashSet<T, S, A>,
}

impl<T> HashSet<T, RandomState> {
    /// 创建一个空的 `HashSet`。
    ///
    /// 该哈希 set 初始创建时容量为 0，因此在首次插入之前不会进行任何分配。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashSet;
    /// let set: HashSet<i32> = HashSet::new();
    /// ```
    #[inline]
    #[must_use]
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn new() -> HashSet<T, RandomState> {
        Default::default()
    }

    /// 创建一个空的 `HashSet`，其容量至少为指定值。
    ///
    /// 该哈希 set 将能够在不重新分配的情况下至少容纳 `capacity` 个元素。本方法允许
    /// 为多于 `capacity` 的元素进行分配。如果 `capacity` 为零，则该哈希 set 不会分配。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashSet;
    /// let set: HashSet<i32> = HashSet::with_capacity(10);
    /// assert!(set.capacity() >= 10);
    /// ```
    #[inline]
    #[must_use]
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn with_capacity(capacity: usize) -> HashSet<T, RandomState> {
        HashSet::with_capacity_and_hasher(capacity, Default::default())
    }
}

impl<T, A: Allocator> HashSet<T, RandomState, A> {
    /// 在所提供的分配器（allocator）中创建一个空的 `HashSet`。
    ///
    /// 该哈希 set 初始创建时容量为 0，因此在首次插入之前不会进行任何分配。
    #[inline]
    #[must_use]
    #[unstable(feature = "allocator_api", issue = "32838")]
    pub fn new_in(alloc: A) -> HashSet<T, RandomState, A> {
        HashSet::with_hasher_in(Default::default(), alloc)
    }

    /// 创建一个空的 `HashSet`，其容量至少为指定值。
    ///
    /// 该哈希 set 将能够在不重新分配的情况下至少容纳 `capacity` 个元素。本方法允许
    /// 为多于 `capacity` 的元素进行分配。如果 `capacity` 为零，则该哈希 set 不会分配。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashSet;
    /// let set: HashSet<i32> = HashSet::with_capacity(10);
    /// assert!(set.capacity() >= 10);
    /// ```
    #[inline]
    #[must_use]
    #[unstable(feature = "allocator_api", issue = "32838")]
    pub fn with_capacity_in(capacity: usize, alloc: A) -> HashSet<T, RandomState, A> {
        HashSet::with_capacity_and_hasher_in(capacity, Default::default(), alloc)
    }
}

impl<T, S> HashSet<T, S> {
    /// 创建一个新的空哈希 set，它将使用给定的哈希器（hasher）来对键做哈希。
    ///
    /// 创建出的哈希 set 同样具有默认的初始容量。
    ///
    /// 警告：`hasher` 通常是随机生成的，其设计目的是让 `HashSet` 能够抵御那些制造大量
    /// 哈希冲突、从而导致性能极差的攻击。通过本函数手动设置它，可能会暴露出一个 DoS
    /// 攻击面。
    ///
    /// 传入的 `hash_builder` 应实现 [`BuildHasher`] trait，这样 `HashSet` 才有意义，
    /// 详见其文档。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashSet;
    /// use std::hash::RandomState;
    ///
    /// let s = RandomState::new();
    /// let mut set = HashSet::with_hasher(s);
    /// set.insert(2);
    /// ```
    #[inline]
    #[stable(feature = "hashmap_build_hasher", since = "1.7.0")]
    #[rustc_const_stable(feature = "const_collections_with_hasher", since = "1.85.0")]
    pub const fn with_hasher(hasher: S) -> HashSet<T, S> {
        HashSet { base: base::HashSet::with_hasher(hasher) }
    }

    /// 创建一个空的 `HashSet`，其容量至少为指定值，并使用 `hasher` 来对键做哈希。
    ///
    /// 该哈希 set 将能够在不重新分配的情况下至少容纳 `capacity` 个元素。本方法允许
    /// 为多于 `capacity` 的元素进行分配。如果 `capacity` 为零，则该哈希 set 不会分配。
    ///
    /// 警告：`hasher` 通常是随机生成的，其设计目的是让 `HashSet` 能够抵御那些制造大量
    /// 哈希冲突、从而导致性能极差的攻击。通过本函数手动设置它，可能会暴露出一个 DoS
    /// 攻击面。
    ///
    /// 传入的 `hash_builder` 应实现 [`BuildHasher`] trait，这样 `HashSet` 才有意义，
    /// 详见其文档。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashSet;
    /// use std::hash::RandomState;
    ///
    /// let s = RandomState::new();
    /// let mut set = HashSet::with_capacity_and_hasher(10, s);
    /// set.insert(1);
    /// ```
    #[inline]
    #[stable(feature = "hashmap_build_hasher", since = "1.7.0")]
    pub fn with_capacity_and_hasher(capacity: usize, hasher: S) -> HashSet<T, S> {
        HashSet { base: base::HashSet::with_capacity_and_hasher(capacity, hasher) }
    }
}

impl<T, S, A: Allocator> HashSet<T, S, A> {
    /// 创建一个新的空哈希 set，它将使用给定的哈希器来对键做哈希，并使用所提供的
    /// 分配器来分配内存。
    ///
    /// 创建出的哈希 set 同样具有默认的初始容量。
    ///
    /// 警告：`hasher` 通常是随机生成的，其设计目的是让 `HashSet` 能够抵御那些制造大量
    /// 哈希冲突、从而导致性能极差的攻击。通过本函数手动设置它，可能会暴露出一个 DoS
    /// 攻击面。
    ///
    /// 传入的 `hash_builder` 应实现 [`BuildHasher`] trait，这样 `HashSet` 才有意义，
    /// 详见其文档。
    #[inline]
    #[unstable(feature = "allocator_api", issue = "32838")]
    pub fn with_hasher_in(hasher: S, alloc: A) -> HashSet<T, S, A> {
        HashSet { base: base::HashSet::with_hasher_in(hasher, alloc) }
    }

    /// 创建一个空的 `HashSet`，其容量至少为指定值，使用 `hasher` 来对键做哈希、
    /// 并使用 `alloc` 来分配内存。
    ///
    /// 该哈希 set 将能够在不重新分配的情况下至少容纳 `capacity` 个元素。本方法允许
    /// 为多于 `capacity` 的元素进行分配。如果 `capacity` 为零，则该哈希 set 不会分配。
    ///
    /// 警告：`hasher` 通常是随机生成的，其设计目的是让 `HashSet` 能够抵御那些制造大量
    /// 哈希冲突、从而导致性能极差的攻击。通过本函数手动设置它，可能会暴露出一个 DoS
    /// 攻击面。
    ///
    /// 传入的 `hash_builder` 应实现 [`BuildHasher`] trait，这样 `HashSet` 才有意义，
    /// 详见其文档。
    #[inline]
    #[unstable(feature = "allocator_api", issue = "32838")]
    pub fn with_capacity_and_hasher_in(capacity: usize, hasher: S, alloc: A) -> HashSet<T, S, A> {
        HashSet { base: base::HashSet::with_capacity_and_hasher_in(capacity, hasher, alloc) }
    }

    /// 返回该 set 在不重新分配的情况下能够容纳的元素数量。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashSet;
    /// let set: HashSet<i32> = HashSet::with_capacity(100);
    /// assert!(set.capacity() >= 100);
    /// ```
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn capacity(&self) -> usize {
        self.base.capacity()
    }

    /// 一个以任意顺序遍历所有元素的迭代器。
    /// 迭代器的元素类型为 `&'a T`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashSet;
    /// let mut set = HashSet::new();
    /// set.insert("a");
    /// set.insert("b");
    ///
    /// // 将以任意顺序打印。
    /// for x in set.iter() {
    ///     println!("{x}");
    /// }
    /// ```
    ///
    /// # Performance
    ///
    /// 在当前实现中，遍历 set 耗费 O(capacity) 的时间而非 O(len)，因为它内部也会访问
    /// 空桶（empty buckets）。
    #[inline]
    #[rustc_lint_query_instability]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[cfg_attr(not(test), rustc_diagnostic_item = "hashset_iter")]
    pub fn iter(&self) -> Iter<'_, T> {
        Iter { base: self.base.iter() }
    }

    /// 返回该 set 中元素的数量。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashSet;
    ///
    /// let mut v = HashSet::new();
    /// assert_eq!(v.len(), 0);
    /// v.insert(1);
    /// assert_eq!(v.len(), 1);
    /// ```
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn len(&self) -> usize {
        self.base.len()
    }

    /// 如果该 set 不含任何元素，则返回 `true`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashSet;
    ///
    /// let mut v = HashSet::new();
    /// assert!(v.is_empty());
    /// v.insert(1);
    /// assert!(!v.is_empty());
    /// ```
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn is_empty(&self) -> bool {
        self.base.is_empty()
    }

    /// 清空该 set，将所有元素作为一个迭代器返回。保留已分配的内存以便复用。
    ///
    /// 如果返回的迭代器在被完全消耗之前就被丢弃（drop），它会丢弃剩余的元素。
    /// 返回的迭代器对该 set 持有一个可变借用，以优化其实现。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashSet;
    ///
    /// let mut set = HashSet::from([1, 2, 3]);
    /// assert!(!set.is_empty());
    ///
    /// // 以任意顺序打印 1、2、3
    /// for i in set.drain() {
    ///     println!("{i}");
    /// }
    ///
    /// assert!(set.is_empty());
    /// ```
    #[inline]
    #[rustc_lint_query_instability]
    #[stable(feature = "drain", since = "1.6.0")]
    pub fn drain(&mut self) -> Drain<'_, T, A> {
        Drain { base: self.base.drain() }
    }

    /// 创建一个迭代器，它使用一个闭包来判定某个元素是否应被移除。
    ///
    /// 如果闭包返回 `true`，该元素会被从 set 中移除并产出。如果闭包返回 `false`
    /// 或发生 panic，该元素仍保留在 set 中且不会被产出。
    ///
    /// 如果返回的 `ExtractIf` 没有被耗尽（例如未经迭代就被丢弃、或迭代发生短路），
    /// 那么剩余元素将被保留。如果你不需要返回的迭代器，请改用 [`retain`] 并传入一个
    /// 取反的谓词。
    ///
    /// [`retain`]: HashSet::retain
    ///
    /// # 示例
    ///
    /// 将一个 set 按奇偶值拆分，并复用原始 set：
    ///
    /// ```
    /// use std::collections::HashSet;
    ///
    /// let mut set: HashSet<i32> = (0..8).collect();
    /// let extracted: HashSet<i32> = set.extract_if(|v| v % 2 == 0).collect();
    ///
    /// let mut evens = extracted.into_iter().collect::<Vec<_>>();
    /// let mut odds = set.into_iter().collect::<Vec<_>>();
    /// evens.sort();
    /// odds.sort();
    ///
    /// assert_eq!(evens, vec![0, 2, 4, 6]);
    /// assert_eq!(odds, vec![1, 3, 5, 7]);
    /// ```
    #[inline]
    #[rustc_lint_query_instability]
    #[stable(feature = "hash_extract_if", since = "1.88.0")]
    pub fn extract_if<F>(&mut self, pred: F) -> ExtractIf<'_, T, F, A>
    where
        F: FnMut(&T) -> bool,
    {
        ExtractIf { base: self.base.extract_if(pred) }
    }

    /// 只保留由谓词所指定的元素。
    ///
    /// 换言之，移除所有使 `f(&e)` 返回 `false` 的元素 `e`。
    /// 元素的访问顺序是未排序的（且未指定）。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashSet;
    ///
    /// let mut set = HashSet::from([1, 2, 3, 4, 5, 6]);
    /// set.retain(|&k| k % 2 == 0);
    /// assert_eq!(set, HashSet::from([2, 4, 6]));
    /// ```
    ///
    /// # Performance
    ///
    /// 在当前实现中，此操作耗费 O(capacity) 的时间而非 O(len)，因为它内部也会访问
    /// 空桶。
    #[rustc_lint_query_instability]
    #[stable(feature = "retain_hash_collection", since = "1.18.0")]
    pub fn retain<F>(&mut self, f: F)
    where
        F: FnMut(&T) -> bool,
    {
        self.base.retain(f)
    }

    /// 清空该 set，移除所有值。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashSet;
    ///
    /// let mut v = HashSet::new();
    /// v.insert(1);
    /// v.clear();
    /// assert!(v.is_empty());
    /// ```
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn clear(&mut self) {
        self.base.clear()
    }

    /// 返回该 set 的 [`BuildHasher`] 的引用。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashSet;
    /// use std::hash::RandomState;
    ///
    /// let hasher = RandomState::new();
    /// let set: HashSet<i32> = HashSet::with_hasher(hasher);
    /// let hasher: &RandomState = set.hasher();
    /// ```
    #[inline]
    #[stable(feature = "hashmap_public_hasher", since = "1.9.0")]
    pub fn hasher(&self) -> &S {
        self.base.hasher()
    }
}

impl<T, S, A> HashSet<T, S, A>
where
    T: Eq + Hash,
    S: BuildHasher,
    A: Allocator,
{
    /// 为在 `HashSet` 中再插入至少 `additional` 个元素预留容量。该集合可能会预留
    /// 更多空间，以推测性地避免频繁的重新分配。调用 `reserve` 之后，容量将大于或
    /// 等于 `self.len() + additional`。如果容量已经足够，则什么也不做。
    ///
    /// # Panics
    ///
    /// 如果新的分配大小溢出 `usize`，则 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashSet;
    /// let mut set: HashSet<i32> = HashSet::new();
    /// set.reserve(10);
    /// assert!(set.capacity() >= 10);
    /// ```
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn reserve(&mut self, additional: usize) {
        self.base.reserve(additional)
    }

    /// 尝试为在 `HashSet` 中再插入至少 `additional` 个元素预留容量。该集合可能会预留
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
    /// use std::collections::HashSet;
    /// let mut set: HashSet<i32> = HashSet::new();
    /// set.try_reserve(10).expect("why is the test harness OOMing on a handful of bytes?");
    /// ```
    #[inline]
    #[stable(feature = "try_reserve", since = "1.57.0")]
    pub fn try_reserve(&mut self, additional: usize) -> Result<(), TryReserveError> {
        self.base.try_reserve(additional).map_err(map_try_reserve_error)
    }

    /// 尽可能收缩该 set 的容量。它会在维持内部规则的前提下尽量降低容量，并可能根据
    /// 调整大小策略（resize policy）保留一些余量。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashSet;
    ///
    /// let mut set = HashSet::with_capacity(100);
    /// set.insert(1);
    /// set.insert(2);
    /// assert!(set.capacity() >= 100);
    /// set.shrink_to_fit();
    /// assert!(set.capacity() >= 2);
    /// ```
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn shrink_to_fit(&mut self) {
        self.base.shrink_to_fit()
    }

    /// 将该 set 的容量收缩到一个下限。它会在维持内部规则的前提下，降低到不低于所给
    /// 下限的水平，并可能根据调整大小策略保留一些余量。
    ///
    /// 如果当前容量小于该下限，则此操作为空操作（no-op）。
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashSet;
    ///
    /// let mut set = HashSet::with_capacity(100);
    /// set.insert(1);
    /// set.insert(2);
    /// assert!(set.capacity() >= 100);
    /// set.shrink_to(10);
    /// assert!(set.capacity() >= 10);
    /// set.shrink_to(0);
    /// assert!(set.capacity() >= 2);
    /// ```
    #[inline]
    #[stable(feature = "shrink_to", since = "1.56.0")]
    pub fn shrink_to(&mut self, min_capacity: usize) {
        self.base.shrink_to(min_capacity)
    }

    /// 访问表示差集（difference）的那些值，即位于 `self` 中但不位于 `other` 中的值。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashSet;
    /// let a = HashSet::from([1, 2, 3]);
    /// let b = HashSet::from([4, 2, 3, 4]);
    ///
    /// // 可以看作 `a - b`。
    /// for x in a.difference(&b) {
    ///     println!("{x}"); // 打印 1
    /// }
    ///
    /// let diff: HashSet<_> = a.difference(&b).collect();
    /// assert_eq!(diff, [1].iter().collect());
    ///
    /// // 注意差集不是对称的，
    /// // `b - a` 表示的是另外一回事：
    /// let diff: HashSet<_> = b.difference(&a).collect();
    /// assert_eq!(diff, [4].iter().collect());
    /// ```
    #[inline]
    #[rustc_lint_query_instability]
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn difference<'a>(&'a self, other: &'a HashSet<T, S, A>) -> Difference<'a, T, S, A> {
        Difference { iter: self.iter(), other }
    }

    /// 访问表示对称差集（symmetric difference）的那些值，即位于 `self` 或 `other`
    /// 中、但不同时位于两者中的值。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashSet;
    /// let a = HashSet::from([1, 2, 3]);
    /// let b = HashSet::from([4, 2, 3, 4]);
    ///
    /// // 以任意顺序打印 1、4。
    /// for x in a.symmetric_difference(&b) {
    ///     println!("{x}");
    /// }
    ///
    /// let diff1: HashSet<_> = a.symmetric_difference(&b).collect();
    /// let diff2: HashSet<_> = b.symmetric_difference(&a).collect();
    ///
    /// assert_eq!(diff1, diff2);
    /// assert_eq!(diff1, [1, 4].iter().collect());
    /// ```
    #[inline]
    #[rustc_lint_query_instability]
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn symmetric_difference<'a>(
        &'a self,
        other: &'a HashSet<T, S, A>,
    ) -> SymmetricDifference<'a, T, S, A> {
        SymmetricDifference { iter: self.difference(other).chain(other.difference(self)) }
    }

    /// 访问表示交集（intersection）的那些值，即同时位于 `self` 与 `other` 中的值。
    ///
    /// 当某个相等的元素同时存在于 `self` 与 `other` 中时，产生的 `Intersection`
    /// 可能产出指向其中任意一个的引用。如果 `T` 含有那些不被其 `Eq` 实现所比较的
    /// 字段、且这些字段在两个 set 中两份相等的 `T` 之间取值不同，那么这一点就值得留意。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashSet;
    /// let a = HashSet::from([1, 2, 3]);
    /// let b = HashSet::from([4, 2, 3, 4]);
    ///
    /// // 以任意顺序打印 2、3。
    /// for x in a.intersection(&b) {
    ///     println!("{x}");
    /// }
    ///
    /// let intersection: HashSet<_> = a.intersection(&b).collect();
    /// assert_eq!(intersection, [2, 3].iter().collect());
    /// ```
    #[inline]
    #[rustc_lint_query_instability]
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn intersection<'a>(&'a self, other: &'a HashSet<T, S, A>) -> Intersection<'a, T, S, A> {
        if self.len() <= other.len() {
            Intersection { iter: self.iter(), other }
        } else {
            Intersection { iter: other.iter(), other: self }
        }
    }

    /// 访问表示并集（union）的那些值，即位于 `self` 或 `other` 中的所有值，不含重复。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashSet;
    /// let a = HashSet::from([1, 2, 3]);
    /// let b = HashSet::from([4, 2, 3, 4]);
    ///
    /// // 以任意顺序打印 1、2、3、4。
    /// for x in a.union(&b) {
    ///     println!("{x}");
    /// }
    ///
    /// let union: HashSet<_> = a.union(&b).collect();
    /// assert_eq!(union, [1, 2, 3, 4].iter().collect());
    /// ```
    #[inline]
    #[rustc_lint_query_instability]
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn union<'a>(&'a self, other: &'a HashSet<T, S, A>) -> Union<'a, T, S, A> {
        if self.len() >= other.len() {
            Union { iter: self.iter().chain(other.difference(self)) }
        } else {
            Union { iter: other.iter().chain(self.difference(other)) }
        }
    }

    /// 如果该 set 含有某个值，则返回 `true`。
    ///
    /// 这个值可以是 set 值类型的任意借用形式（borrowed form），但借用形式上的
    /// [`Hash`] 与 [`Eq`] *必须* 与值类型上的保持一致。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashSet;
    ///
    /// let set = HashSet::from([1, 2, 3]);
    /// assert_eq!(set.contains(&1), true);
    /// assert_eq!(set.contains(&4), false);
    /// ```
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn contains<Q: ?Sized>(&self, value: &Q) -> bool
    where
        T: Borrow<Q>,
        Q: Hash + Eq,
    {
        self.base.contains(value)
    }

    /// 返回 set 中与所给值相等的那个值的引用（如果存在的话）。
    ///
    /// 这个值可以是 set 值类型的任意借用形式，但借用形式上的 [`Hash`] 与 [`Eq`]
    /// *必须* 与值类型上的保持一致。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashSet;
    ///
    /// let set = HashSet::from([1, 2, 3]);
    /// assert_eq!(set.get(&2), Some(&2));
    /// assert_eq!(set.get(&4), None);
    /// ```
    #[inline]
    #[stable(feature = "set_recovery", since = "1.9.0")]
    pub fn get<Q: ?Sized>(&self, value: &Q) -> Option<&T>
    where
        T: Borrow<Q>,
        Q: Hash + Eq,
    {
        self.base.get(value)
    }

    /// 如果所给的 `value` 不在 set 中，则将其插入，然后返回 set 中那个值的引用。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(hash_set_entry)]
    ///
    /// use std::collections::HashSet;
    ///
    /// let mut set = HashSet::from([1, 2, 3]);
    /// assert_eq!(set.len(), 3);
    /// assert_eq!(set.get_or_insert(2), &2);
    /// assert_eq!(set.get_or_insert(100), &100);
    /// assert_eq!(set.len(), 4); // 100 被插入了
    /// ```
    #[inline]
    #[unstable(feature = "hash_set_entry", issue = "60896")]
    pub fn get_or_insert(&mut self, value: T) -> &T {
        // 尽管 raw entry 给了我们 `&mut T`，但为了与 `get` 保持一致，我们只返回 `&T`。
        // 键的变更是 "raw" 的，因为你不应当影响 `Eq` 或 `Hash`。
        self.base.get_or_insert(value)
    }

    /// 如果所给的 `value` 不在 set 中，则将由 `f` 计算出的值插入，然后返回 set 中
    /// 那个值的引用。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(hash_set_entry)]
    ///
    /// use std::collections::HashSet;
    ///
    /// let mut set: HashSet<String> = ["cat", "dog", "horse"]
    ///     .iter().map(|&pet| pet.to_owned()).collect();
    ///
    /// assert_eq!(set.len(), 3);
    /// for &pet in &["cat", "dog", "fish"] {
    ///     let value = set.get_or_insert_with(pet, str::to_owned);
    ///     assert_eq!(value, pet);
    /// }
    /// assert_eq!(set.len(), 4); // 一个新的 "fish" 被插入了
    /// ```
    #[inline]
    #[unstable(feature = "hash_set_entry", issue = "60896")]
    pub fn get_or_insert_with<Q: ?Sized, F>(&mut self, value: &Q, f: F) -> &T
    where
        T: Borrow<Q>,
        Q: Hash + Eq,
        F: FnOnce(&Q) -> T,
    {
        // 尽管 raw entry 给了我们 `&mut T`，但为了与 `get` 保持一致，我们只返回 `&T`。
        // 键的变更是 "raw" 的，因为你不应当影响 `Eq` 或 `Hash`。
        self.base.get_or_insert_with(value, f)
    }

    /// 获取所给值在 set 中所对应的 entry，以便进行就地操作。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(hash_set_entry)]
    ///
    /// use std::collections::HashSet;
    /// use std::collections::hash_set::Entry::*;
    ///
    /// let mut singles = HashSet::new();
    /// let mut dupes = HashSet::new();
    ///
    /// for ch in "a short treatise on fungi".chars() {
    ///     if let Vacant(dupe_entry) = dupes.entry(ch) {
    ///         // 我们还没见过重复项，所以
    ///         // 检查一下我们是否至少见过它一次。
    ///         match singles.entry(ch) {
    ///             Vacant(single_entry) => {
    ///                 // 我们第一次发现了一个新字符。
    ///                 single_entry.insert()
    ///             }
    ///             Occupied(single_entry) => {
    ///                 // 我们已经见过它一次了，把它 "移动" 到 dupes 中。
    ///                 single_entry.remove();
    ///                 dupe_entry.insert();
    ///             }
    ///         }
    ///     }
    /// }
    ///
    /// assert!(!singles.contains(&'t') && dupes.contains(&'t'));
    /// assert!(singles.contains(&'u') && !dupes.contains(&'u'));
    /// assert!(!singles.contains(&'v') && !dupes.contains(&'v'));
    /// ```
    #[inline]
    #[unstable(feature = "hash_set_entry", issue = "60896")]
    pub fn entry(&mut self, value: T) -> Entry<'_, T, S, A> {
        map_entry(self.base.entry(value))
    }

    /// 如果 `self` 与 `other` 没有任何共同元素，则返回 `true`。
    /// 这等价于检查交集是否为空。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashSet;
    ///
    /// let a = HashSet::from([1, 2, 3]);
    /// let mut b = HashSet::new();
    ///
    /// assert_eq!(a.is_disjoint(&b), true);
    /// b.insert(4);
    /// assert_eq!(a.is_disjoint(&b), true);
    /// b.insert(1);
    /// assert_eq!(a.is_disjoint(&b), false);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn is_disjoint(&self, other: &HashSet<T, S, A>) -> bool {
        if self.len() <= other.len() {
            self.iter().all(|v| !other.contains(v))
        } else {
            other.iter().all(|v| !self.contains(v))
        }
    }

    /// 如果该 set 是另一个 set 的子集（subset），则返回 `true`，即 `other` 至少
    /// 包含 `self` 中的所有值。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashSet;
    ///
    /// let sup = HashSet::from([1, 2, 3]);
    /// let mut set = HashSet::new();
    ///
    /// assert_eq!(set.is_subset(&sup), true);
    /// set.insert(2);
    /// assert_eq!(set.is_subset(&sup), true);
    /// set.insert(4);
    /// assert_eq!(set.is_subset(&sup), false);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn is_subset(&self, other: &HashSet<T, S, A>) -> bool {
        if self.len() <= other.len() { self.iter().all(|v| other.contains(v)) } else { false }
    }

    /// 如果该 set 是另一个 set 的超集（superset），则返回 `true`，即 `self` 至少
    /// 包含 `other` 中的所有值。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashSet;
    ///
    /// let sub = HashSet::from([1, 2]);
    /// let mut set = HashSet::new();
    ///
    /// assert_eq!(set.is_superset(&sub), false);
    ///
    /// set.insert(0);
    /// set.insert(1);
    /// assert_eq!(set.is_superset(&sub), false);
    ///
    /// set.insert(2);
    /// assert_eq!(set.is_superset(&sub), true);
    /// ```
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn is_superset(&self, other: &HashSet<T, S, A>) -> bool {
        other.is_subset(self)
    }

    /// 向该 set 中添加一个值。
    ///
    /// 返回该值是否是新插入的。也就是说：
    ///
    /// - 如果该 set 此前不含这个值，则返回 `true`。
    /// - 如果该 set 此前已含这个值，则返回 `false`，且该 set 不被修改：原有的值
    ///   不会被替换，作为实参传入的值会被丢弃。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashSet;
    ///
    /// let mut set = HashSet::new();
    ///
    /// assert_eq!(set.insert(2), true);
    /// assert_eq!(set.insert(2), false);
    /// assert_eq!(set.len(), 1);
    /// ```
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_confusables("push", "append", "put")]
    pub fn insert(&mut self, value: T) -> bool {
        self.base.insert(value)
    }

    /// 向该 set 中添加一个值，替换掉与所给值相等的那个已有值（如果存在的话）。
    /// 返回被替换掉的值。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashSet;
    ///
    /// let mut set = HashSet::new();
    /// set.insert(Vec::<i32>::new());
    ///
    /// assert_eq!(set.get(&[][..]).unwrap().capacity(), 0);
    /// set.replace(Vec::with_capacity(10));
    /// assert_eq!(set.get(&[][..]).unwrap().capacity(), 10);
    /// ```
    #[inline]
    #[stable(feature = "set_recovery", since = "1.9.0")]
    #[rustc_confusables("swap")]
    pub fn replace(&mut self, value: T) -> Option<T> {
        self.base.replace(value)
    }

    /// 从该 set 中移除一个值。返回该值此前是否存在于 set 中。
    ///
    /// 这个值可以是 set 值类型的任意借用形式，但借用形式上的 [`Hash`] 与 [`Eq`]
    /// *必须* 与值类型上的保持一致。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashSet;
    ///
    /// let mut set = HashSet::new();
    ///
    /// set.insert(2);
    /// assert_eq!(set.remove(&2), true);
    /// assert_eq!(set.remove(&2), false);
    /// ```
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_confusables("delete", "take")]
    pub fn remove<Q: ?Sized>(&mut self, value: &Q) -> bool
    where
        T: Borrow<Q>,
        Q: Hash + Eq,
    {
        self.base.remove(value)
    }

    /// 移除并返回 set 中与所给值相等的那个值（如果存在的话）。
    ///
    /// 这个值可以是 set 值类型的任意借用形式，但借用形式上的 [`Hash`] 与 [`Eq`]
    /// *必须* 与值类型上的保持一致。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashSet;
    ///
    /// let mut set = HashSet::from([1, 2, 3]);
    /// assert_eq!(set.take(&2), Some(2));
    /// assert_eq!(set.take(&2), None);
    /// ```
    #[inline]
    #[stable(feature = "set_recovery", since = "1.9.0")]
    pub fn take<Q: ?Sized>(&mut self, value: &Q) -> Option<T>
    where
        T: Borrow<Q>,
        Q: Hash + Eq,
    {
        self.base.take(value)
    }
}

#[inline]
fn map_entry<'a, K: 'a, V: 'a, A: Allocator>(raw: base::Entry<'a, K, V, A>) -> Entry<'a, K, V, A> {
    match raw {
        base::Entry::Occupied(base) => Entry::Occupied(OccupiedEntry { base }),
        base::Entry::Vacant(base) => Entry::Vacant(VacantEntry { base }),
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T, S, A> Clone for HashSet<T, S, A>
where
    T: Clone,
    S: Clone,
    A: Allocator + Clone,
{
    #[inline]
    fn clone(&self) -> Self {
        Self { base: self.base.clone() }
    }

    /// 用 `source` 内容的克隆覆盖 `self` 的内容。
    ///
    /// 相比简单地把 `source.clone()` 赋值给 `self`，本方法更受推荐，因为它会在可能的
    /// 情况下避免重新分配。
    #[inline]
    fn clone_from(&mut self, other: &Self) {
        self.base.clone_from(&other.base);
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T, S, A> PartialEq for HashSet<T, S, A>
where
    T: Eq + Hash,
    S: BuildHasher,
    A: Allocator,
{
    fn eq(&self, other: &HashSet<T, S, A>) -> bool {
        if self.len() != other.len() {
            return false;
        }

        self.iter().all(|key| other.contains(key))
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T, S, A> Eq for HashSet<T, S, A>
where
    T: Eq + Hash,
    S: BuildHasher,
    A: Allocator,
{
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T, S, A> fmt::Debug for HashSet<T, S, A>
where
    T: fmt::Debug,
    A: Allocator,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_set().entries(self.iter()).finish()
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T, S> FromIterator<T> for HashSet<T, S>
where
    T: Eq + Hash,
    S: BuildHasher + Default,
{
    #[inline]
    fn from_iter<I: IntoIterator<Item = T>>(iter: I) -> HashSet<T, S> {
        let mut set = HashSet::with_hasher(Default::default());
        set.extend(iter);
        set
    }
}

#[stable(feature = "std_collections_from_array", since = "1.56.0")]
// 注意：作为目前最便捷的内置 HashSet 构造方式，对本函数的简单使用绝不能*要求*用户
// 提供类型标注来推断第三个类型参数（哈希器参数，惯例上记作 "S"）。
// 为此，本 impl 使用 RandomState 作为 S 的具体类型来定义，而非对
// `S: BuildHasher + Default` 泛型化。
// 预期那些想要指定哈希器的用户会手动使用 `with_capacity_and_hasher`。
// 假如类型参数默认值能在 impl 上生效、且类型参数默认值能与 const 泛型混用，那么
// 或许可以将其泛化。
// 另见 HashMap 上等价的 impl。
impl<T, const N: usize> From<[T; N]> for HashSet<T, RandomState>
where
    T: Eq + Hash,
{
    /// 将一个 `[T; N]` 转换为 `HashSet<T>`。
    ///
    /// 如果该数组含有任何相等的值，那么其中除一个之外都会被丢弃。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashSet;
    ///
    /// let set1 = HashSet::from([1, 2, 3, 4]);
    /// let set2: HashSet<_> = [1, 2, 3, 4].into();
    /// assert_eq!(set1, set2);
    /// ```
    fn from(arr: [T; N]) -> Self {
        Self::from_iter(arr)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T, S, A> Extend<T> for HashSet<T, S, A>
where
    T: Eq + Hash,
    S: BuildHasher,
    A: Allocator,
{
    #[inline]
    fn extend<I: IntoIterator<Item = T>>(&mut self, iter: I) {
        self.base.extend(iter);
    }

    #[inline]
    fn extend_one(&mut self, item: T) {
        self.base.insert(item);
    }

    #[inline]
    fn extend_reserve(&mut self, additional: usize) {
        self.base.extend_reserve(additional);
    }
}

#[stable(feature = "hash_extend_copy", since = "1.4.0")]
impl<'a, T, S, A> Extend<&'a T> for HashSet<T, S, A>
where
    T: 'a + Eq + Hash + Copy,
    S: BuildHasher,
    A: Allocator,
{
    #[inline]
    fn extend<I: IntoIterator<Item = &'a T>>(&mut self, iter: I) {
        self.extend(iter.into_iter().cloned());
    }

    #[inline]
    fn extend_one(&mut self, &item: &'a T) {
        self.base.insert(item);
    }

    #[inline]
    fn extend_reserve(&mut self, additional: usize) {
        Extend::<T>::extend_reserve(self, additional)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_unstable(feature = "const_default", issue = "143894")]
impl<T, S> const Default for HashSet<T, S>
where
    S: [const] Default,
{
    /// 创建一个空的 `HashSet<T, S>`，哈希器取其 `Default` 值。
    #[inline]
    fn default() -> HashSet<T, S> {
        HashSet { base: base::HashSet::with_hasher(Default::default()) }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T, S> BitOr<&HashSet<T, S>> for &HashSet<T, S>
where
    T: Eq + Hash + Clone,
    S: BuildHasher + Default,
{
    type Output = HashSet<T, S>;

    /// 返回 `self` 与 `rhs` 的并集，作为一个新的 `HashSet<T, S>`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashSet;
    ///
    /// let a = HashSet::from([1, 2, 3]);
    /// let b = HashSet::from([3, 4, 5]);
    ///
    /// let set = &a | &b;
    ///
    /// let mut i = 0;
    /// let expected = [1, 2, 3, 4, 5];
    /// for x in &set {
    ///     assert!(expected.contains(x));
    ///     i += 1;
    /// }
    /// assert_eq!(i, expected.len());
    /// ```
    fn bitor(self, rhs: &HashSet<T, S>) -> HashSet<T, S> {
        self.union(rhs).cloned().collect()
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T, S> BitAnd<&HashSet<T, S>> for &HashSet<T, S>
where
    T: Eq + Hash + Clone,
    S: BuildHasher + Default,
{
    type Output = HashSet<T, S>;

    /// 返回 `self` 与 `rhs` 的交集，作为一个新的 `HashSet<T, S>`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashSet;
    ///
    /// let a = HashSet::from([1, 2, 3]);
    /// let b = HashSet::from([2, 3, 4]);
    ///
    /// let set = &a & &b;
    ///
    /// let mut i = 0;
    /// let expected = [2, 3];
    /// for x in &set {
    ///     assert!(expected.contains(x));
    ///     i += 1;
    /// }
    /// assert_eq!(i, expected.len());
    /// ```
    fn bitand(self, rhs: &HashSet<T, S>) -> HashSet<T, S> {
        self.intersection(rhs).cloned().collect()
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T, S> BitXor<&HashSet<T, S>> for &HashSet<T, S>
where
    T: Eq + Hash + Clone,
    S: BuildHasher + Default,
{
    type Output = HashSet<T, S>;

    /// 返回 `self` 与 `rhs` 的对称差集，作为一个新的 `HashSet<T, S>`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashSet;
    ///
    /// let a = HashSet::from([1, 2, 3]);
    /// let b = HashSet::from([3, 4, 5]);
    ///
    /// let set = &a ^ &b;
    ///
    /// let mut i = 0;
    /// let expected = [1, 2, 4, 5];
    /// for x in &set {
    ///     assert!(expected.contains(x));
    ///     i += 1;
    /// }
    /// assert_eq!(i, expected.len());
    /// ```
    fn bitxor(self, rhs: &HashSet<T, S>) -> HashSet<T, S> {
        self.symmetric_difference(rhs).cloned().collect()
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T, S> Sub<&HashSet<T, S>> for &HashSet<T, S>
where
    T: Eq + Hash + Clone,
    S: BuildHasher + Default,
{
    type Output = HashSet<T, S>;

    /// 返回 `self` 与 `rhs` 的差集，作为一个新的 `HashSet<T, S>`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashSet;
    ///
    /// let a = HashSet::from([1, 2, 3]);
    /// let b = HashSet::from([3, 4, 5]);
    ///
    /// let set = &a - &b;
    ///
    /// let mut i = 0;
    /// let expected = [1, 2];
    /// for x in &set {
    ///     assert!(expected.contains(x));
    ///     i += 1;
    /// }
    /// assert_eq!(i, expected.len());
    /// ```
    fn sub(self, rhs: &HashSet<T, S>) -> HashSet<T, S> {
        self.difference(rhs).cloned().collect()
    }
}

/// 一个遍历 `HashSet` 各项的迭代器。
///
/// 此 `struct` 由 [`HashSet`] 上的 [`iter`] 方法创建。详见其文档。
///
/// [`iter`]: HashSet::iter
///
/// # 示例
///
/// ```
/// use std::collections::HashSet;
///
/// let a = HashSet::from([1, 2, 3]);
///
/// let mut iter = a.iter();
/// ```
#[stable(feature = "rust1", since = "1.0.0")]
#[cfg_attr(not(test), rustc_diagnostic_item = "hashset_iter_ty")]
pub struct Iter<'a, K: 'a> {
    base: base::Iter<'a, K>,
}

#[stable(feature = "default_iters_hash", since = "1.83.0")]
impl<K> Default for Iter<'_, K> {
    #[inline]
    fn default() -> Self {
        Iter { base: Default::default() }
    }
}

/// 一个拥有所有权、遍历 `HashSet` 各项的迭代器。
///
/// 此 `struct` 由 [`HashSet`] 上的 [`into_iter`] 方法创建（由 [`IntoIterator`]
/// trait 提供）。详见其文档。
///
/// [`into_iter`]: IntoIterator::into_iter
///
/// # 示例
///
/// ```
/// use std::collections::HashSet;
///
/// let a = HashSet::from([1, 2, 3]);
///
/// let mut iter = a.into_iter();
/// ```
#[stable(feature = "rust1", since = "1.0.0")]
pub struct IntoIter<
    K,
    #[unstable(feature = "allocator_api", issue = "32838")] A: Allocator = Global,
> {
    base: base::IntoIter<K, A>,
}

#[stable(feature = "default_iters_hash", since = "1.83.0")]
impl<K> Default for IntoIter<K> {
    #[inline]
    fn default() -> Self {
        IntoIter { base: Default::default() }
    }
}

/// 一个对 `HashSet` 各项进行抽空（draining）的迭代器。
///
/// 此 `struct` 由 [`HashSet`] 上的 [`drain`] 方法创建。详见其文档。
///
/// [`drain`]: HashSet::drain
///
/// # 示例
///
/// ```
/// use std::collections::HashSet;
///
/// let mut a = HashSet::from([1, 2, 3]);
///
/// let mut drain = a.drain();
/// ```
#[stable(feature = "rust1", since = "1.0.0")]
#[cfg_attr(not(test), rustc_diagnostic_item = "hashset_drain_ty")]
pub struct Drain<
    'a,
    K: 'a,
    #[unstable(feature = "allocator_api", issue = "32838")] A: Allocator = Global,
> {
    base: base::Drain<'a, K, A>,
}

/// 一个对 `HashSet` 各项进行抽空并过滤（filtering）的迭代器。
///
/// 此 `struct` 由 [`HashSet`] 上的 [`extract_if`] 方法创建。
///
/// [`extract_if`]: HashSet::extract_if
///
/// # 示例
///
/// ```
/// use std::collections::HashSet;
///
/// let mut a = HashSet::from([1, 2, 3]);
///
/// let mut extract_ifed = a.extract_if(|v| v % 2 == 0);
/// ```
#[stable(feature = "hash_extract_if", since = "1.88.0")]
#[must_use = "iterators are lazy and do nothing unless consumed; \
    use `retain` to remove and discard elements"]
pub struct ExtractIf<
    'a,
    K,
    F,
    #[unstable(feature = "allocator_api", issue = "32838")] A: Allocator = Global,
> {
    base: base::ExtractIf<'a, K, F, A>,
}

/// 一个惰性（lazy）迭代器，产出位于多个 `HashSet` 交集中的元素。
///
/// 此 `struct` 由 [`HashSet`] 上的 [`intersection`] 方法创建。详见其文档。
///
/// [`intersection`]: HashSet::intersection
///
/// # 示例
///
/// ```
/// use std::collections::HashSet;
///
/// let a = HashSet::from([1, 2, 3]);
/// let b = HashSet::from([4, 2, 3, 4]);
///
/// let mut intersection = a.intersection(&b);
/// ```
#[must_use = "this returns the intersection as an iterator, \
              without modifying either input set"]
#[stable(feature = "rust1", since = "1.0.0")]
pub struct Intersection<
    'a,
    T: 'a,
    S: 'a,
    #[unstable(feature = "allocator_api", issue = "32838")] A: Allocator = Global,
> {
    // 第一个 set 的迭代器
    iter: Iter<'a, T>,
    // 第二个 set
    other: &'a HashSet<T, S, A>,
}

/// 一个惰性迭代器，产出位于多个 `HashSet` 差集中的元素。
///
/// 此 `struct` 由 [`HashSet`] 上的 [`difference`] 方法创建。详见其文档。
///
/// [`difference`]: HashSet::difference
///
/// # 示例
///
/// ```
/// use std::collections::HashSet;
///
/// let a = HashSet::from([1, 2, 3]);
/// let b = HashSet::from([4, 2, 3, 4]);
///
/// let mut difference = a.difference(&b);
/// ```
#[must_use = "this returns the difference as an iterator, \
              without modifying either input set"]
#[stable(feature = "rust1", since = "1.0.0")]
pub struct Difference<
    'a,
    T: 'a,
    S: 'a,
    #[unstable(feature = "allocator_api", issue = "32838")] A: Allocator = Global,
> {
    // 第一个 set 的迭代器
    iter: Iter<'a, T>,
    // 第二个 set
    other: &'a HashSet<T, S, A>,
}

/// 一个惰性迭代器，产出位于多个 `HashSet` 对称差集中的元素。
///
/// 此 `struct` 由 [`HashSet`] 上的 [`symmetric_difference`] 方法创建。详见其文档。
///
/// [`symmetric_difference`]: HashSet::symmetric_difference
///
/// # 示例
///
/// ```
/// use std::collections::HashSet;
///
/// let a = HashSet::from([1, 2, 3]);
/// let b = HashSet::from([4, 2, 3, 4]);
///
/// let mut intersection = a.symmetric_difference(&b);
/// ```
#[must_use = "this returns the difference as an iterator, \
              without modifying either input set"]
#[stable(feature = "rust1", since = "1.0.0")]
pub struct SymmetricDifference<
    'a,
    T: 'a,
    S: 'a,
    #[unstable(feature = "allocator_api", issue = "32838")] A: Allocator = Global,
> {
    iter: Chain<Difference<'a, T, S, A>, Difference<'a, T, S, A>>,
}

/// 一个惰性迭代器，产出位于多个 `HashSet` 并集中的元素。
///
/// 此 `struct` 由 [`HashSet`] 上的 [`union`] 方法创建。详见其文档。
///
/// [`union`]: HashSet::union
///
/// # 示例
///
/// ```
/// use std::collections::HashSet;
///
/// let a = HashSet::from([1, 2, 3]);
/// let b = HashSet::from([4, 2, 3, 4]);
///
/// let mut union_iter = a.union(&b);
/// ```
#[must_use = "this returns the union as an iterator, \
              without modifying either input set"]
#[stable(feature = "rust1", since = "1.0.0")]
pub struct Union<
    'a,
    T: 'a,
    S: 'a,
    #[unstable(feature = "allocator_api", issue = "32838")] A: Allocator = Global,
> {
    iter: Chain<Iter<'a, T>, Difference<'a, T, S, A>>,
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<'a, T, S, A: Allocator> IntoIterator for &'a HashSet<T, S, A> {
    type Item = &'a T;
    type IntoIter = Iter<'a, T>;

    #[inline]
    #[rustc_lint_query_instability]
    fn into_iter(self) -> Iter<'a, T> {
        self.iter()
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T, S, A: Allocator> IntoIterator for HashSet<T, S, A> {
    type Item = T;
    type IntoIter = IntoIter<T, A>;

    /// 创建一个消耗型迭代器，也就是说，它以任意顺序将每个值移出该 set。
    /// 调用之后该 set 不能再被使用。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::collections::HashSet;
    /// let mut set = HashSet::new();
    /// set.insert("a".to_string());
    /// set.insert("b".to_string());
    ///
    /// // 用普通的 `.iter()` 无法收集为 Vec<String>。
    /// let v: Vec<String> = set.into_iter().collect();
    ///
    /// // 将以任意顺序打印。
    /// for x in &v {
    ///     println!("{x}");
    /// }
    /// ```
    #[inline]
    #[rustc_lint_query_instability]
    fn into_iter(self) -> IntoIter<T, A> {
        IntoIter { base: self.base.into_iter() }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<K> Clone for Iter<'_, K> {
    #[inline]
    fn clone(&self) -> Self {
        Iter { base: self.base.clone() }
    }
}
#[stable(feature = "rust1", since = "1.0.0")]
impl<'a, K> Iterator for Iter<'a, K> {
    type Item = &'a K;

    #[inline]
    fn next(&mut self) -> Option<&'a K> {
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
impl<K> ExactSizeIterator for Iter<'_, K> {
    #[inline]
    fn len(&self) -> usize {
        self.base.len()
    }
}
#[stable(feature = "fused", since = "1.26.0")]
impl<K> FusedIterator for Iter<'_, K> {}

#[stable(feature = "std_debug", since = "1.16.0")]
impl<K: fmt::Debug> fmt::Debug for Iter<'_, K> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.clone()).finish()
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<K, A: Allocator> Iterator for IntoIter<K, A> {
    type Item = K;

    #[inline]
    fn next(&mut self) -> Option<K> {
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
impl<K, A: Allocator> ExactSizeIterator for IntoIter<K, A> {
    #[inline]
    fn len(&self) -> usize {
        self.base.len()
    }
}
#[stable(feature = "fused", since = "1.26.0")]
impl<K, A: Allocator> FusedIterator for IntoIter<K, A> {}

#[stable(feature = "std_debug", since = "1.16.0")]
impl<K: fmt::Debug, A: Allocator> fmt::Debug for IntoIter<K, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.base, f)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<'a, K, A: Allocator> Iterator for Drain<'a, K, A> {
    type Item = K;

    #[inline]
    fn next(&mut self) -> Option<K> {
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
#[stable(feature = "rust1", since = "1.0.0")]
impl<K, A: Allocator> ExactSizeIterator for Drain<'_, K, A> {
    #[inline]
    fn len(&self) -> usize {
        self.base.len()
    }
}
#[stable(feature = "fused", since = "1.26.0")]
impl<K, A: Allocator> FusedIterator for Drain<'_, K, A> {}

#[stable(feature = "std_debug", since = "1.16.0")]
impl<K: fmt::Debug, A: Allocator> fmt::Debug for Drain<'_, K, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.base, f)
    }
}

#[stable(feature = "hash_extract_if", since = "1.88.0")]
impl<K, F, A: Allocator> Iterator for ExtractIf<'_, K, F, A>
where
    F: FnMut(&K) -> bool,
{
    type Item = K;

    #[inline]
    fn next(&mut self) -> Option<K> {
        self.base.next()
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.base.size_hint()
    }
}

#[stable(feature = "hash_extract_if", since = "1.88.0")]
impl<K, F, A: Allocator> FusedIterator for ExtractIf<'_, K, F, A> where F: FnMut(&K) -> bool {}

#[stable(feature = "hash_extract_if", since = "1.88.0")]
impl<K, F, A: Allocator> fmt::Debug for ExtractIf<'_, K, F, A>
where
    K: fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExtractIf").finish_non_exhaustive()
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T, S, A: Allocator> Clone for Intersection<'_, T, S, A> {
    #[inline]
    fn clone(&self) -> Self {
        Intersection { iter: self.iter.clone(), ..*self }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<'a, T, S, A> Iterator for Intersection<'a, T, S, A>
where
    T: Eq + Hash,
    S: BuildHasher,
    A: Allocator,
{
    type Item = &'a T;

    #[inline]
    fn next(&mut self) -> Option<&'a T> {
        loop {
            let elt = self.iter.next()?;
            if self.other.contains(elt) {
                return Some(elt);
            }
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let (_, upper) = self.iter.size_hint();
        (0, upper)
    }

    #[inline]
    fn fold<B, F>(self, init: B, mut f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        self.iter.fold(init, |acc, elt| if self.other.contains(elt) { f(acc, elt) } else { acc })
    }
}

#[stable(feature = "std_debug", since = "1.16.0")]
impl<T, S, A> fmt::Debug for Intersection<'_, T, S, A>
where
    T: fmt::Debug + Eq + Hash,
    S: BuildHasher,
    A: Allocator,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.clone()).finish()
    }
}

#[stable(feature = "fused", since = "1.26.0")]
impl<T, S, A> FusedIterator for Intersection<'_, T, S, A>
where
    T: Eq + Hash,
    S: BuildHasher,
    A: Allocator,
{
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T, S, A: Allocator> Clone for Difference<'_, T, S, A> {
    #[inline]
    fn clone(&self) -> Self {
        Difference { iter: self.iter.clone(), ..*self }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<'a, T, S, A> Iterator for Difference<'a, T, S, A>
where
    T: Eq + Hash,
    S: BuildHasher,
    A: Allocator,
{
    type Item = &'a T;

    #[inline]
    fn next(&mut self) -> Option<&'a T> {
        loop {
            let elt = self.iter.next()?;
            if !self.other.contains(elt) {
                return Some(elt);
            }
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let (_, upper) = self.iter.size_hint();
        (0, upper)
    }

    #[inline]
    fn fold<B, F>(self, init: B, mut f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        self.iter.fold(init, |acc, elt| if self.other.contains(elt) { acc } else { f(acc, elt) })
    }
}

#[stable(feature = "fused", since = "1.26.0")]
impl<T, S, A> FusedIterator for Difference<'_, T, S, A>
where
    T: Eq + Hash,
    S: BuildHasher,
    A: Allocator,
{
}

#[stable(feature = "std_debug", since = "1.16.0")]
impl<T, S, A> fmt::Debug for Difference<'_, T, S, A>
where
    T: fmt::Debug + Eq + Hash,
    S: BuildHasher,
    A: Allocator,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.clone()).finish()
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T, S, A: Allocator> Clone for SymmetricDifference<'_, T, S, A> {
    #[inline]
    fn clone(&self) -> Self {
        SymmetricDifference { iter: self.iter.clone() }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<'a, T, S, A> Iterator for SymmetricDifference<'a, T, S, A>
where
    T: Eq + Hash,
    S: BuildHasher,
    A: Allocator,
{
    type Item = &'a T;

    #[inline]
    fn next(&mut self) -> Option<&'a T> {
        self.iter.next()
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
    #[inline]
    fn fold<B, F>(self, init: B, f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        self.iter.fold(init, f)
    }
}

#[stable(feature = "fused", since = "1.26.0")]
impl<T, S, A> FusedIterator for SymmetricDifference<'_, T, S, A>
where
    T: Eq + Hash,
    S: BuildHasher,
    A: Allocator,
{
}

#[stable(feature = "std_debug", since = "1.16.0")]
impl<T, S, A> fmt::Debug for SymmetricDifference<'_, T, S, A>
where
    T: fmt::Debug + Eq + Hash,
    S: BuildHasher,
    A: Allocator,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.clone()).finish()
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T, S, A: Allocator> Clone for Union<'_, T, S, A> {
    #[inline]
    fn clone(&self) -> Self {
        Union { iter: self.iter.clone() }
    }
}

#[stable(feature = "fused", since = "1.26.0")]
impl<T, S, A: Allocator> FusedIterator for Union<'_, T, S, A>
where
    T: Eq + Hash,
    S: BuildHasher,
{
}

#[stable(feature = "std_debug", since = "1.16.0")]
impl<T, S, A: Allocator> fmt::Debug for Union<'_, T, S, A>
where
    T: fmt::Debug + Eq + Hash,
    S: BuildHasher,
    A: Allocator,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.clone()).finish()
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<'a, T, S, A> Iterator for Union<'a, T, S, A>
where
    T: Eq + Hash,
    S: BuildHasher,
    A: Allocator,
{
    type Item = &'a T;

    #[inline]
    fn next(&mut self) -> Option<&'a T> {
        self.iter.next()
    }
    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
    #[inline]
    fn count(self) -> usize {
        self.iter.count()
    }
    #[inline]
    fn fold<B, F>(self, init: B, f: F) -> B
    where
        Self: Sized,
        F: FnMut(B, Self::Item) -> B,
    {
        self.iter.fold(init, f)
    }
}

/// 对 set 中单个 entry 的视图（view），它可能是空缺的（vacant）或被占用的
/// （occupied）。
///
/// 此 `enum` 由 [`HashSet`] 上的 [`entry`] 方法构造。
///
/// [`HashSet`]: struct.HashSet.html
/// [`entry`]: struct.HashSet.html#method.entry
///
/// # 示例
///
/// ```
/// #![feature(hash_set_entry)]
///
/// use std::collections::hash_set::HashSet;
///
/// let mut set = HashSet::new();
/// set.extend(["a", "b", "c"]);
/// assert_eq!(set.len(), 3);
///
/// // 已存在的值（insert）
/// let entry = set.entry("a");
/// let _raw_o = entry.insert();
/// assert_eq!(set.len(), 3);
/// // 不存在的值（insert）
/// set.entry("d").insert();
///
/// // 已存在的值（or_insert）
/// set.entry("b").or_insert();
/// // 不存在的值（or_insert）
/// set.entry("e").or_insert();
///
/// println!("Our HashSet: {:?}", set);
///
/// let mut vec: Vec<_> = set.iter().copied().collect();
/// // `Iter` 迭代器以任意顺序产出各项，所以必须先对各项排序，才能与已排序的数组
/// // 进行比较。
/// vec.sort_unstable();
/// assert_eq!(vec, ["a", "b", "c", "d", "e"]);
/// ```
#[unstable(feature = "hash_set_entry", issue = "60896")]
pub enum Entry<
    'a,
    T,
    S,
    #[unstable(feature = "allocator_api", issue = "32838")] A: Allocator = Global,
> {
    /// 一个被占用的 entry。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(hash_set_entry)]
    ///
    /// use std::collections::hash_set::{Entry, HashSet};
    ///
    /// let mut set = HashSet::from(["a", "b"]);
    ///
    /// match set.entry("a") {
    ///     Entry::Vacant(_) => unreachable!(),
    ///     Entry::Occupied(_) => { }
    /// }
    /// ```
    Occupied(OccupiedEntry<'a, T, S, A>),

    /// 一个空缺的 entry。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(hash_set_entry)]
    ///
    /// use std::collections::hash_set::{Entry, HashSet};
    ///
    /// let mut set = HashSet::new();
    ///
    /// match set.entry("a") {
    ///     Entry::Occupied(_) => unreachable!(),
    ///     Entry::Vacant(_) => { }
    /// }
    /// ```
    Vacant(VacantEntry<'a, T, S, A>),
}

#[unstable(feature = "hash_set_entry", issue = "60896")]
impl<T: fmt::Debug, S, A: Allocator> fmt::Debug for Entry<'_, T, S, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Entry::Vacant(ref v) => f.debug_tuple("Entry").field(v).finish(),
            Entry::Occupied(ref o) => f.debug_tuple("Entry").field(o).finish(),
        }
    }
}

/// 对 `HashSet` 中一个被占用 entry 的视图。
/// 它是 [`Entry`] 枚举的组成部分。
///
/// [`Entry`]: enum.Entry.html
///
/// # 示例
///
/// ```
/// #![feature(hash_set_entry)]
///
/// use std::collections::hash_set::{Entry, HashSet};
///
/// let mut set = HashSet::new();
/// set.extend(["a", "b", "c"]);
///
/// let _entry_o = set.entry("a").insert();
/// assert_eq!(set.len(), 3);
///
/// // 已存在的键
/// match set.entry("a") {
///     Entry::Vacant(_) => unreachable!(),
///     Entry::Occupied(view) => {
///         assert_eq!(view.get(), &"a");
///     }
/// }
///
/// assert_eq!(set.len(), 3);
///
/// // 已存在的键（take）
/// match set.entry("c") {
///     Entry::Vacant(_) => unreachable!(),
///     Entry::Occupied(view) => {
///         assert_eq!(view.remove(), "c");
///     }
/// }
/// assert_eq!(set.get(&"c"), None);
/// assert_eq!(set.len(), 2);
/// ```
#[unstable(feature = "hash_set_entry", issue = "60896")]
pub struct OccupiedEntry<
    'a,
    T,
    S,
    #[unstable(feature = "allocator_api", issue = "32838")] A: Allocator = Global,
> {
    base: base::OccupiedEntry<'a, T, S, A>,
}

#[unstable(feature = "hash_set_entry", issue = "60896")]
impl<T: fmt::Debug, S, A: Allocator> fmt::Debug for OccupiedEntry<'_, T, S, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OccupiedEntry").field("value", self.get()).finish()
    }
}

/// 对 `HashSet` 中一个空缺 entry 的视图。
/// 它是 [`Entry`] 枚举的组成部分。
///
/// [`Entry`]: enum.Entry.html
///
/// # 示例
///
/// ```
/// #![feature(hash_set_entry)]
///
/// use std::collections::hash_set::{Entry, HashSet};
///
/// let mut set = HashSet::<&str>::new();
///
/// let entry_v = match set.entry("a") {
///     Entry::Vacant(view) => view,
///     Entry::Occupied(_) => unreachable!(),
/// };
/// entry_v.insert();
/// assert!(set.contains("a") && set.len() == 1);
///
/// // 不存在的键（insert）
/// match set.entry("b") {
///     Entry::Vacant(view) => view.insert(),
///     Entry::Occupied(_) => unreachable!(),
/// }
/// assert!(set.contains("b") && set.len() == 2);
/// ```
#[unstable(feature = "hash_set_entry", issue = "60896")]
pub struct VacantEntry<
    'a,
    T,
    S,
    #[unstable(feature = "allocator_api", issue = "32838")] A: Allocator = Global,
> {
    base: base::VacantEntry<'a, T, S, A>,
}

#[unstable(feature = "hash_set_entry", issue = "60896")]
impl<T: fmt::Debug, S, A: Allocator> fmt::Debug for VacantEntry<'_, T, S, A> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("VacantEntry").field(self.get()).finish()
    }
}

impl<'a, T, S, A: Allocator> Entry<'a, T, S, A> {
    /// 设置该 entry 的值，并返回一个 OccupiedEntry。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(hash_set_entry)]
    ///
    /// use std::collections::HashSet;
    ///
    /// let mut set = HashSet::new();
    /// let entry = set.entry("horseyland").insert();
    ///
    /// assert_eq!(entry.get(), &"horseyland");
    /// ```
    #[inline]
    #[unstable(feature = "hash_set_entry", issue = "60896")]
    pub fn insert(self) -> OccupiedEntry<'a, T, S, A>
    where
        T: Hash,
        S: BuildHasher,
    {
        match self {
            Entry::Occupied(entry) => entry,
            Entry::Vacant(entry) => entry.insert_entry(),
        }
    }

    /// 若 entry 为空缺则插入，从而确保 entry 中存在一个值。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(hash_set_entry)]
    ///
    /// use std::collections::HashSet;
    ///
    /// let mut set = HashSet::new();
    ///
    /// // 不存在的键
    /// set.entry("poneyland").or_insert();
    /// assert!(set.contains("poneyland"));
    ///
    /// // 已存在的键
    /// set.entry("poneyland").or_insert();
    /// assert!(set.contains("poneyland"));
    /// assert_eq!(set.len(), 1);
    /// ```
    #[inline]
    #[unstable(feature = "hash_set_entry", issue = "60896")]
    pub fn or_insert(self)
    where
        T: Hash,
        S: BuildHasher,
    {
        if let Entry::Vacant(entry) = self {
            entry.insert();
        }
    }

    /// 返回此 entry 的值的引用。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(hash_set_entry)]
    ///
    /// use std::collections::HashSet;
    ///
    /// let mut set = HashSet::new();
    /// set.entry("poneyland").or_insert();
    ///
    /// // 已存在的键
    /// assert_eq!(set.entry("poneyland").get(), &"poneyland");
    /// // 不存在的键
    /// assert_eq!(set.entry("horseland").get(), &"horseland");
    /// ```
    #[inline]
    #[unstable(feature = "hash_set_entry", issue = "60896")]
    pub fn get(&self) -> &T {
        match *self {
            Entry::Occupied(ref entry) => entry.get(),
            Entry::Vacant(ref entry) => entry.get(),
        }
    }
}

impl<T, S, A: Allocator> OccupiedEntry<'_, T, S, A> {
    /// 获取该 entry 中值的引用。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(hash_set_entry)]
    ///
    /// use std::collections::hash_set::{Entry, HashSet};
    ///
    /// let mut set = HashSet::new();
    /// set.entry("poneyland").or_insert();
    ///
    /// match set.entry("poneyland") {
    ///     Entry::Vacant(_) => panic!(),
    ///     Entry::Occupied(entry) => assert_eq!(entry.get(), &"poneyland"),
    /// }
    /// ```
    #[inline]
    #[unstable(feature = "hash_set_entry", issue = "60896")]
    pub fn get(&self) -> &T {
        self.base.get()
    }

    /// 将值从该 entry 中取出，并返回它。
    /// 保留已分配的内存以便复用。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(hash_set_entry)]
    ///
    /// use std::collections::HashSet;
    /// use std::collections::hash_set::Entry;
    ///
    /// let mut set = HashSet::new();
    /// // 该 set 为空
    /// assert!(set.is_empty() && set.capacity() == 0);
    ///
    /// set.entry("poneyland").or_insert();
    /// let capacity_before_remove = set.capacity();
    ///
    /// if let Entry::Occupied(o) = set.entry("poneyland") {
    ///     assert_eq!(o.remove(), "poneyland");
    /// }
    ///
    /// assert_eq!(set.contains("poneyland"), false);
    /// // 现在该 set 不含任何元素，但容量与原来相等
    /// assert!(set.len() == 0 && set.capacity() == capacity_before_remove);
    /// ```
    #[inline]
    #[unstable(feature = "hash_set_entry", issue = "60896")]
    pub fn remove(self) -> T {
        self.base.remove()
    }
}

impl<'a, T, S, A: Allocator> VacantEntry<'a, T, S, A> {
    /// 获取一个对将通过该 `VacantEntry` 插入时所用值的引用。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(hash_set_entry)]
    ///
    /// use std::collections::HashSet;
    ///
    /// let mut set = HashSet::new();
    /// assert_eq!(set.entry("poneyland").get(), &"poneyland");
    /// ```
    #[inline]
    #[unstable(feature = "hash_set_entry", issue = "60896")]
    pub fn get(&self) -> &T {
        self.base.get()
    }

    /// 取得该值的所有权。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(hash_set_entry)]
    ///
    /// use std::collections::hash_set::{Entry, HashSet};
    ///
    /// let mut set = HashSet::new();
    ///
    /// match set.entry("poneyland") {
    ///     Entry::Occupied(_) => panic!(),
    ///     Entry::Vacant(v) => assert_eq!(v.into_value(), "poneyland"),
    /// }
    /// ```
    #[inline]
    #[unstable(feature = "hash_set_entry", issue = "60896")]
    pub fn into_value(self) -> T {
        self.base.into_value()
    }

    /// 以该 VacantEntry 的值设置该 entry 的值。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(hash_set_entry)]
    ///
    /// use std::collections::HashSet;
    /// use std::collections::hash_set::Entry;
    ///
    /// let mut set = HashSet::new();
    ///
    /// if let Entry::Vacant(o) = set.entry("poneyland") {
    ///     o.insert();
    /// }
    /// assert!(set.contains("poneyland"));
    /// ```
    #[inline]
    #[unstable(feature = "hash_set_entry", issue = "60896")]
    pub fn insert(self)
    where
        T: Hash,
        S: BuildHasher,
    {
        self.base.insert();
    }

    #[inline]
    fn insert_entry(self) -> OccupiedEntry<'a, T, S, A>
    where
        T: Hash,
        S: BuildHasher,
    {
        OccupiedEntry { base: self.base.insert() }
    }
}

#[allow(dead_code)]
fn assert_covariance() {
    fn set<'new>(v: HashSet<&'static str>) -> HashSet<&'new str> {
        v
    }
    fn iter<'a, 'new>(v: Iter<'a, &'static str>) -> Iter<'a, &'new str> {
        v
    }
    fn into_iter<'new>(v: IntoIter<&'static str>) -> IntoIter<&'new str> {
        v
    }
    fn difference<'a, 'new>(
        v: Difference<'a, &'static str, RandomState>,
    ) -> Difference<'a, &'new str, RandomState> {
        v
    }
    fn symmetric_difference<'a, 'new>(
        v: SymmetricDifference<'a, &'static str, RandomState>,
    ) -> SymmetricDifference<'a, &'new str, RandomState> {
        v
    }
    fn intersection<'a, 'new>(
        v: Intersection<'a, &'static str, RandomState>,
    ) -> Intersection<'a, &'new str, RandomState> {
        v
    }
    fn union<'a, 'new>(
        v: Union<'a, &'static str, RandomState>,
    ) -> Union<'a, &'new str, RandomState> {
        v
    }
    fn drain<'new>(d: Drain<'static, &'static str>) -> Drain<'new, &'new str> {
        d
    }
}
