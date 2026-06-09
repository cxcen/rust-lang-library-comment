//! 泛型哈希支持。
//!
//! 本模块提供一套泛型接口，用来计算值的 [hash]。在 `core` 中它只定义
//! `Hash`、`Hasher` 和 `BuildHasher` 之间的协议，不负责选择具体表结构；
//! 最常见的使用者是 [`HashMap`] 和 [`HashSet`]。这些集合依赖同一个核心契约：
//! 两个由 [`Eq`] 判定相等的键必须向 [`Hasher`] 写入等价的输入，并最终产生相等的
//! hash 值。反过来，hash 相等并不表示值相等，因为碰撞总是允许存在。
//!
//! [hash]: https://en.wikipedia.org/wiki/Hash_function
//! [`HashMap`]: ../../std/collections/struct.HashMap.html
//! [`HashSet`]: ../../std/collections/struct.HashSet.html
//!
//! 让类型支持 `Hash` 的最简单方式是使用 `#[derive(Hash)]`。派生实现会按照字段顺序
//! 依次调用字段的 `Hash::hash`，因此字段参与 [`Eq`] 比较的方式通常也应当和字段参与
//! `Hash` 的方式保持一致：
//!
//! # 示例
//!
//! ```rust
//! use std::hash::{DefaultHasher, Hash, Hasher};
//!
//! #[derive(Hash)]
//! struct Person {
//!     id: u32,
//!     name: String,
//!     phone: u64,
//! }
//!
//! let person1 = Person {
//!     id: 5,
//!     name: "Janet".to_string(),
//!     phone: 555_666_7777,
//! };
//! let person2 = Person {
//!     id: 5,
//!     name: "Bob".to_string(),
//!     phone: 555_666_7777,
//! };
//!
//! assert!(calculate_hash(&person1) != calculate_hash(&person2));
//!
//! fn calculate_hash<T: Hash>(t: &T) -> u64 {
//!     let mut s = DefaultHasher::new();
//!     t.hash(&mut s);
//!     s.finish()
//! }
//! ```
//!
//! 如果某些字段不属于相等性判定的一部分，或者需要自定义写入顺序，就需要手写
//! [`Hash`] trait。手写实现时要把逻辑重点放在“哪些值在 [`Eq`] 下相等”，而不是放在
//! “怎样得到看起来随机的数字”；随机化和防碰撞策略属于具体 [`Hasher`] 的职责：
//!
//! ```rust
//! use std::hash::{DefaultHasher, Hash, Hasher};
//!
//! struct Person {
//!     id: u32,
//!     # #[allow(dead_code)]
//!     name: String,
//!     phone: u64,
//! }
//!
//! impl Hash for Person {
//!     fn hash<H: Hasher>(&self, state: &mut H) {
//!         self.id.hash(state);
//!         self.phone.hash(state);
//!     }
//! }
//!
//! let person1 = Person {
//!     id: 5,
//!     name: "Janet".to_string(),
//!     phone: 555_666_7777,
//! };
//! let person2 = Person {
//!     id: 5,
//!     name: "Bob".to_string(),
//!     phone: 555_666_7777,
//! };
//!
//! assert_eq!(calculate_hash(&person1), calculate_hash(&person2));
//!
//! fn calculate_hash<T: Hash>(t: &T) -> u64 {
//!     let mut s = DefaultHasher::new();
//!     t.hash(&mut s);
//!     s.finish()
//! }
//! ```

#![stable(feature = "rust1", since = "1.0.0")]

#[stable(feature = "rust1", since = "1.0.0")]
#[allow(deprecated)]
pub use self::sip::SipHasher;
#[unstable(feature = "hashmap_internals", issue = "none")]
#[allow(deprecated)]
#[doc(hidden)]
pub use self::sip::SipHasher13;
use crate::{fmt, marker};

mod sip;

/// 可被哈希的类型。
///
/// 实现 `Hash` 的类型可以把自身的判定数据写入某个 [`Hasher`]。`Hash` 本身并不规定
/// 最终算法，也不直接返回 `u64`；它只负责把值表示成一段有边界、顺序明确的输入流，
/// 由 [`Hasher`] 根据自己的状态和算法生成最终结果。
///
/// ## 实现 `Hash`
///
/// 如果所有字段都实现了 `Hash`，通常可以直接使用 `#[derive(Hash)]`。派生代码会按字段
/// 顺序调用每个字段的 [`hash`]，因此它天然和同样派生出来的 [`PartialEq`]、[`Eq`] 保持
/// 同一套字段语义。
///
/// ```
/// #[derive(Hash)]
/// struct Rustacean {
///     name: String,
///     country: String,
/// }
/// ```
///
/// 如果只有一部分字段参与键的身份，或者需要把多个字段归一化后再参与 hash，可以手写
/// `Hash` trait。手写实现必须和该类型的相等性语义配套设计：
///
/// ```
/// use std::hash::{Hash, Hasher};
///
/// struct Person {
///     id: u32,
///     name: String,
///     phone: u64,
/// }
///
/// impl Hash for Person {
///     fn hash<H: Hasher>(&self, state: &mut H) {
///         self.id.hash(state);
///         self.phone.hash(state);
///     }
/// }
/// ```
///
/// ## `Hash` 和 `Eq`
///
/// 当同一个类型同时实现 `Hash` 和 [`Eq`] 时，必须满足下面的性质：
///
/// ```text
/// k1 == k2 -> hash(k1) == hash(k2)
/// ```
///
/// 换句话说，如果两个键相等，它们产生的 hash 也必须相等。[`HashMap`] 和 [`HashSet`]
/// 都把这个性质当作查找、插入和去重的基础。它们仍然会处理不同值得到相同 hash 的碰撞，
/// 但如果相等值得到不同 hash，集合可能把同一个逻辑键放进不同桶中，从而出现查找失败、
/// 重复键或无法预期的迭代行为。
///
/// 同时使用 `#[derive(PartialEq, Eq, Hash)]` 时，派生实现会对同一组字段采用一致顺序，
/// 因而通常不需要手动维护这个性质。
///
/// 违反这个性质是逻辑错误。由逻辑错误导致的行为没有稳定规范，但 trait 的使用者必须保证
/// 这种错误不会升级为 undefined behavior。这一点划定了 unsafe 边界：`unsafe` 代码
/// **不得**把 `Hash`/`Eq` 实现的正确性当作内存安全前置条件，也不能依赖“相等值一定产生
/// 相等 hash”来证明指针、别名或生命周期操作是安全的。
///
/// ## 前缀碰撞
///
/// `hash` 的实现应当确保传给 `Hasher` 的数据是 prefix-free 的。也就是说，对于不相等的
/// 值，应当写入不同的值序列，并且任一序列都不应当只是另一个序列的前缀。这个要求也常被
/// 称为 domain separation：实现需要在“字段边界”“集合长度”“字符串结束”等位置写入足够
/// 的分隔信息，使不同结构不能被拼接成同一条字节流。
///
/// 例如，[`Hash` for `&str`][impl] 的标准实现会额外向 `Hasher` 写入一个 `0xFF` 字节。
/// 由于合法 UTF-8 字符串中不会出现该字节，它可以作为字符串的 domain separator，使
/// `("ab", "c")` 和 `("a", "bc")` 这样的值组合写入不同的输入序列。
///
/// ## 可移植性
///
/// 由于端序和类型大小可能因平台而异，`Hash` 写入 `Hasher` 的数据不应被视为跨平台可移植。
/// 此外，多数标准库类型写入的数据格式也不保证在不同编译器版本之间保持稳定。
///
/// 因此，测试不应断言某个硬编码 hash 值，也不应检查写给 `Hasher` 的内部字节细节；更合适
/// 的测试目标是验证它和 `Eq` 的一致性。
///
/// 需要跨平台或跨编译器版本稳定的序列化格式，应避免直接编码 hash，或者只依赖那些额外
/// 明确承诺了稳定格式的 `Hash`/`Hasher` 实现。
///
/// [`HashMap`]: ../../std/collections/struct.HashMap.html
/// [`HashSet`]: ../../std/collections/struct.HashSet.html
/// [`hash`]: Hash::hash
/// [impl]: ../../std/primitive.str.html#impl-Hash-for-str
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_diagnostic_item = "Hash"]
pub trait Hash: marker::PointeeSized {
    /// 将此值写入给定的 [`Hasher`]。
    ///
    /// 实现者应写入足以表达该值在 `Eq` 下身份的数据，并保持字段顺序和分隔规则稳定。
    /// 这个方法不应自行调用 [`Hasher::finish`]；同一个 `Hasher` 可能还要继续接收外层结构
    /// 的其他字段。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::hash::{DefaultHasher, Hash, Hasher};
    ///
    /// let mut hasher = DefaultHasher::new();
    /// 7920.hash(&mut hasher);
    /// println!("Hash is {:x}!", hasher.finish());
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    fn hash<H: Hasher>(&self, state: &mut H);

    /// 将此类型的一个切片写入给定的 [`Hasher`]。
    ///
    /// 这个方法是便利入口，但它的具体实现被有意留作未指定。它不保证等价于对每个元素重复
    /// 调用 [`hash`]。如果切片在该类型的 [`PartialEq`] 实现中并不是一个整体单元，`Hash`
    /// 实现就应当牢记这一点，直接逐项调用 [`hash`]，而不是把任意中间切片交给
    /// [`hash_slice`]。
    ///
    /// 例如，[`VecDeque`] 的实现如果天真地调用 [`as_slices`]，再分别对两个切片调用
    /// [`hash_slice`]，就是错误的：一次 [`make_contiguous`] 可以改变两个切片的划分方式，
    /// 却不改变 [`PartialEq`] 结果。因为这些切片只是更大双端队列的内部表示，而不是相等性
    /// 语义中的独立整体，所以不能在这里使用此方法。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::hash::{DefaultHasher, Hash, Hasher};
    ///
    /// let mut hasher = DefaultHasher::new();
    /// let numbers = [6, 28, 496, 8128];
    /// Hash::hash_slice(&numbers, &mut hasher);
    /// println!("Hash is {:x}!", hasher.finish());
    /// ```
    ///
    /// [`VecDeque`]: ../../std/collections/struct.VecDeque.html
    /// [`as_slices`]: ../../std/collections/struct.VecDeque.html#method.as_slices
    /// [`make_contiguous`]: ../../std/collections/struct.VecDeque.html#method.make_contiguous
    /// [`hash`]: Hash::hash
    /// [`hash_slice`]: Hash::hash_slice
    #[stable(feature = "hash_slice", since = "1.3.0")]
    fn hash_slice<H: Hasher>(data: &[Self], state: &mut H)
    where
        Self: Sized,
    {
        for piece in data {
            piece.hash(state)
        }
    }
}

// 独立模块用于从 prelude 重新导出宏 `Hash`，同时避免一并导出 trait `Hash`。
pub(crate) mod macros {
    /// 生成 trait `Hash` 实现的派生宏。
    #[rustc_builtin_macro]
    #[stable(feature = "builtin_macro_prelude", since = "1.38.0")]
    #[allow_internal_unstable(core_intrinsics)]
    pub macro Hash($item:item) {
        /* 编译器内建 */
    }
}
#[stable(feature = "builtin_macro_prelude", since = "1.38.0")]
#[doc(inline)]
pub use macros::Hash;

/// 对任意字节流进行哈希的 trait。
///
/// `Hasher` 的实例通常保存一段会随着输入而更新的内部状态。`Hash` 实现通过调用
/// `write`/`write_*` 方法把值写进这段状态，最后由 [`finish`] 读取当前状态对应的
/// `u64` 结果。
///
/// `Hasher` 只提供相当基础的接口：用 [`finish`] 取得已生成的 hash，用 [`write`] 和
/// [`write_u8`] 等方法写入字节切片或整数。绝大多数时候，`Hasher` 会和 [`Hash`] trait
/// 一起使用，由 `Hash` 决定写入哪些数据，由 `Hasher` 决定如何压缩这些数据。
///
/// 这个 trait 不保证各个 `write_*` 方法之间有任何特定等价关系，因而 [`Hash`] 实现不能假设
/// 它们会以某种方式工作。例如，不能假设一次 [`write_u32`] 调用等价于四次 [`write_u8`]
/// 调用；也不能假设相邻的 `write` 调用会被合并。因此，例如下面这段写入：
/// ```
/// # fn foo(hasher: &mut impl std::hash::Hasher) {
/// hasher.write(&[1, 2]);
/// hasher.write(&[3, 4, 5, 6]);
/// # }
/// ```
/// 和下面这段写入：
/// ```
/// # fn foo(hasher: &mut impl std::hash::Hasher) {
/// hasher.write(&[1, 2, 3, 4]);
/// hasher.write(&[5, 6]);
/// # }
/// ```
/// 可能产生不同的 hash：
///
/// 换言之，要让等价的值产生相同 hash，[`Hash`] 实现必须保证它们执行完全相同的调用序列：
/// 同样的方法、同样的参数、同样的顺序。这就是 `Hasher` 的字节流语义和 `Hash`/`Eq` 契约
/// 连接起来的地方。
///
/// # 示例
///
/// ```
/// use std::hash::{DefaultHasher, Hasher};
///
/// let mut hasher = DefaultHasher::new();
///
/// hasher.write_u32(1989);
/// hasher.write_u8(11);
/// hasher.write_u8(9);
/// hasher.write(b"Huh?");
///
/// println!("Hash is {:x}!", hasher.finish());
/// ```
///
/// [`finish`]: Hasher::finish
/// [`write`]: Hasher::write
/// [`write_u8`]: Hasher::write_u8
/// [`write_u32`]: Hasher::write_u32
#[stable(feature = "rust1", since = "1.0.0")]
pub trait Hasher {
    /// 返回到目前为止已写入值对应的 hash 值。
    ///
    /// 尽管方法名叫 `finish`，它不会重置 hasher 的内部状态。后续 [`write`] 会从当前状态
    /// 继续。如果需要开始一次全新的 hash 计算，必须创建新的 hasher。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::hash::{DefaultHasher, Hasher};
    ///
    /// let mut hasher = DefaultHasher::new();
    /// hasher.write(b"Cool!");
    ///
    /// println!("Hash is {:x}!", hasher.finish());
    /// ```
    ///
    /// [`write`]: Hasher::write
    #[stable(feature = "rust1", since = "1.0.0")]
    #[must_use]
    fn finish(&self) -> u64;

    /// 向这个 `Hasher` 写入一段字节数据。
    ///
    /// 这里的 `bytes` 是原始输入片段，不自动携带长度或字段边界信息。需要 prefix-free 的
    /// 结构应由调用方，也就是对应的 `Hash` 实现，先写入长度前缀或其他 domain separator。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::hash::{DefaultHasher, Hasher};
    ///
    /// let mut hasher = DefaultHasher::new();
    /// let data = [0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef];
    ///
    /// hasher.write(&data);
    ///
    /// println!("Hash is {:x}!", hasher.finish());
    /// ```
    ///
    /// # 实现者注意
    ///
    /// 实现这个方法时通常不应自动添加长度前缀。哪些序列需要长度或边界分隔，是
    /// [`Hash`] 实现的语义责任；它应当在需要时先调用 [`Hasher::write_length_prefix`]。
    #[stable(feature = "rust1", since = "1.0.0")]
    fn write(&mut self, bytes: &[u8]);

    /// 向这个 hasher 写入一个 `u8`。
    #[inline]
    #[stable(feature = "hasher_write", since = "1.3.0")]
    fn write_u8(&mut self, i: u8) {
        self.write(&[i])
    }
    /// 向这个 hasher 写入一个 `u16`。
    #[inline]
    #[stable(feature = "hasher_write", since = "1.3.0")]
    fn write_u16(&mut self, i: u16) {
        self.write(&i.to_ne_bytes())
    }
    /// 向这个 hasher 写入一个 `u32`。
    #[inline]
    #[stable(feature = "hasher_write", since = "1.3.0")]
    fn write_u32(&mut self, i: u32) {
        self.write(&i.to_ne_bytes())
    }
    /// 向这个 hasher 写入一个 `u64`。
    #[inline]
    #[stable(feature = "hasher_write", since = "1.3.0")]
    fn write_u64(&mut self, i: u64) {
        self.write(&i.to_ne_bytes())
    }
    /// 向这个 hasher 写入一个 `u128`。
    #[inline]
    #[stable(feature = "i128", since = "1.26.0")]
    fn write_u128(&mut self, i: u128) {
        self.write(&i.to_ne_bytes())
    }
    /// 向这个 hasher 写入一个 `usize`。
    #[inline]
    #[stable(feature = "hasher_write", since = "1.3.0")]
    fn write_usize(&mut self, i: usize) {
        self.write(&i.to_ne_bytes())
    }

    /// 向这个 hasher 写入一个 `i8`。
    #[inline]
    #[stable(feature = "hasher_write", since = "1.3.0")]
    fn write_i8(&mut self, i: i8) {
        self.write_u8(i as u8)
    }
    /// 向这个 hasher 写入一个 `i16`。
    #[inline]
    #[stable(feature = "hasher_write", since = "1.3.0")]
    fn write_i16(&mut self, i: i16) {
        self.write_u16(i as u16)
    }
    /// 向这个 hasher 写入一个 `i32`。
    #[inline]
    #[stable(feature = "hasher_write", since = "1.3.0")]
    fn write_i32(&mut self, i: i32) {
        self.write_u32(i as u32)
    }
    /// 向这个 hasher 写入一个 `i64`。
    #[inline]
    #[stable(feature = "hasher_write", since = "1.3.0")]
    fn write_i64(&mut self, i: i64) {
        self.write_u64(i as u64)
    }
    /// 向这个 hasher 写入一个 `i128`。
    #[inline]
    #[stable(feature = "i128", since = "1.26.0")]
    fn write_i128(&mut self, i: i128) {
        self.write_u128(i as u128)
    }
    /// 向这个 hasher 写入一个 `isize`。
    #[inline]
    #[stable(feature = "hasher_write", since = "1.3.0")]
    fn write_isize(&mut self, i: isize) {
        self.write_usize(i as usize)
    }

    /// 向这个 hasher 写入长度前缀，用作 prefix-free 编码的一部分。
    ///
    /// 如果正在为自定义集合实现 [`Hash`]，应在写入集合元素之前调用此方法。这样
    /// `(collection![1, 2, 3], collection![4, 5])` 和
    /// `(collection![1, 2], collection![3, 4, 5])` 会向 `Hasher` 提供不同的值序列。
    ///
    /// `impl<T> Hash for [T]` 已经包含对此方法的调用。因此，如果通过切片、数组或 `Vec`
    /// 自身的 `Hash::hash` 方法进行哈希，调用方 **不应** 再手动调用此方法。
    ///
    /// 此方法只用于提供 domain separation。如果要哈希的 `usize` 本身就是*数据*的一部分，
    /// 必须把它传给 [`Hasher::write_usize`]，而不是传给此方法；否则长度前缀和普通数据会
    /// 落入同一个语义域，破坏 `Hash` 实现的可推理性。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(hasher_prefixfree_extras)]
    /// # // 这些桩定义只用于让下面的 `impl` 通过编译
    /// # #![allow(non_local_definitions)]
    /// # struct MyCollection<T>(Option<T>);
    /// # impl<T> MyCollection<T> {
    /// #     fn len(&self) -> usize { todo!() }
    /// # }
    /// # impl<'a, T> IntoIterator for &'a MyCollection<T> {
    /// #     type Item = T;
    /// #     type IntoIter = std::iter::Empty<T>;
    /// #     fn into_iter(self) -> Self::IntoIter { todo!() }
    /// # }
    ///
    /// use std::hash::{Hash, Hasher};
    /// impl<T: Hash> Hash for MyCollection<T> {
    ///     fn hash<H: Hasher>(&self, state: &mut H) {
    ///         state.write_length_prefix(self.len());
    ///         for elt in self {
    ///             elt.hash(state);
    ///         }
    ///     }
    /// }
    /// ```
    ///
    /// # 实现者注意
    ///
    /// 如果你的 `Hasher` 有意接受 Hash-DoS 攻击风险，以换取更高性能，可以考虑忽略传入的
    /// `len` 的一部分或全部。不过这样做会削弱长度前缀作为 domain separator 的强度，适用
    /// 范围应由具体 hasher 的安全和性能目标决定。
    #[inline]
    #[unstable(feature = "hasher_prefixfree_extras", issue = "96762")]
    fn write_length_prefix(&mut self, len: usize) {
        self.write_usize(len);
    }

    /// 向这个 hasher 写入一个 `str`。
    ///
    /// 如果正在实现 [`Hash`]，通常不需要直接调用此方法；`impl Hash for str` 已经会这样做，
    /// 因而优先让 `str` 自己的 `Hash` 实现负责写入。
    ///
    /// 此方法已经包含用于 prefix-free 的 domain separator，因此调用它之前 **不应** 再调用
    /// `Self::write_length_prefix`。
    ///
    /// # 实现者注意
    ///
    /// 至少有两种合理的默认实现方式。哪一种会成为最终默认实现尚未决定，所以当前自定义
    /// `Hasher` 往往应当显式覆盖此方法。
    ///
    /// ## 通用做法
    ///
    /// 使用长度前缀实现此方法总是正确的：
    ///
    /// ```
    /// # #![feature(hasher_prefixfree_extras)]
    /// # struct Foo;
    /// # impl std::hash::Hasher for Foo {
    /// # fn finish(&self) -> u64 { unimplemented!() }
    /// # fn write(&mut self, _bytes: &[u8]) { unimplemented!() }
    /// fn write_str(&mut self, s: &str) {
    ///     self.write_length_prefix(s.len());
    ///     self.write(s.as_bytes());
    /// }
    /// # }
    /// ```
    ///
    /// 如果你的 `Hasher` 以 `usize` 块为工作单位，这通常也很高效；更复杂的方案很可能比
    /// 直接把长度参与一轮压缩更慢。
    ///
    /// ## 按字节工作的 `Hasher`
    ///
    /// `str` 是 UTF-8 的一个好处是合法字符串中永远不会出现 `b'\xFF'` 字节。因此可以把
    /// 该字节追加到参与 hash 的字节流末尾，用它维持 prefix-freedom：
    ///
    /// ```
    /// # #![feature(hasher_prefixfree_extras)]
    /// # struct Foo;
    /// # impl std::hash::Hasher for Foo {
    /// # fn finish(&self) -> u64 { unimplemented!() }
    /// # fn write(&mut self, _bytes: &[u8]) { unimplemented!() }
    /// fn write_str(&mut self, s: &str) {
    ///     self.write(s.as_bytes());
    ///     self.write_u8(0xff);
    /// }
    /// # }
    /// ```
    ///
    /// 这要求实现本身不要额外填充输入，因此通常需要维护缓冲区，只在缓冲区装满或调用
    /// `finish` 时才执行一轮压缩。
    ///
    /// 原因是，如果 `write` 会把数据填充到固定块大小，它很可能让 `"a"` 和 `"a\x00"` 最终
    /// 参与 hash 的块序列相同，从而引入冲突。
    #[inline]
    #[unstable(feature = "hasher_prefixfree_extras", issue = "96762")]
    fn write_str(&mut self, s: &str) {
        self.write(s.as_bytes());
        self.write_u8(0xff);
    }
}

#[stable(feature = "indirect_hasher_impl", since = "1.22.0")]
impl<H: Hasher + ?Sized> Hasher for &mut H {
    fn finish(&self) -> u64 {
        (**self).finish()
    }
    fn write(&mut self, bytes: &[u8]) {
        (**self).write(bytes)
    }
    fn write_u8(&mut self, i: u8) {
        (**self).write_u8(i)
    }
    fn write_u16(&mut self, i: u16) {
        (**self).write_u16(i)
    }
    fn write_u32(&mut self, i: u32) {
        (**self).write_u32(i)
    }
    fn write_u64(&mut self, i: u64) {
        (**self).write_u64(i)
    }
    fn write_u128(&mut self, i: u128) {
        (**self).write_u128(i)
    }
    fn write_usize(&mut self, i: usize) {
        (**self).write_usize(i)
    }
    fn write_i8(&mut self, i: i8) {
        (**self).write_i8(i)
    }
    fn write_i16(&mut self, i: i16) {
        (**self).write_i16(i)
    }
    fn write_i32(&mut self, i: i32) {
        (**self).write_i32(i)
    }
    fn write_i64(&mut self, i: i64) {
        (**self).write_i64(i)
    }
    fn write_i128(&mut self, i: i128) {
        (**self).write_i128(i)
    }
    fn write_isize(&mut self, i: isize) {
        (**self).write_isize(i)
    }
    fn write_length_prefix(&mut self, len: usize) {
        (**self).write_length_prefix(len)
    }
    fn write_str(&mut self, s: &str) {
        (**self).write_str(s)
    }
}

/// 用于创建 [`Hasher`] 实例的 trait。
///
/// `BuildHasher` 通常由 [`HashMap`] 这类集合使用，用来为每次键操作创建新的 [`Hasher`]。
/// 由于 [`Hasher`] 带有可变状态，不能让不同键共享同一个正在写入的 hasher；集合会通过
/// `BuildHasher` 取得相互独立但配置一致的 hasher 实例。
///
/// 对同一个 `BuildHasher` 实例而言，[`build_hasher`] 创建出的 [`Hasher`] 应当等价。也就是说，
/// 如果向这些 hasher 写入完全相同的字节流，它们也应产生相同输出。这使 [`HashMap`] 能在
/// 插入和后续查找时重新构造 hasher，而不必保存每个键的中间状态。
///
/// # 示例
///
/// ```
/// use std::hash::{BuildHasher, Hasher, RandomState};
///
/// let s = RandomState::new();
/// let mut hasher_1 = s.build_hasher();
/// let mut hasher_2 = s.build_hasher();
///
/// hasher_1.write_u32(8128);
/// hasher_2.write_u32(8128);
///
/// assert_eq!(hasher_1.finish(), hasher_2.finish());
/// ```
///
/// [`build_hasher`]: BuildHasher::build_hasher
/// [`HashMap`]: ../../std/collections/struct.HashMap.html
#[cfg_attr(not(test), rustc_diagnostic_item = "BuildHasher")]
#[stable(since = "1.7.0", feature = "build_hasher")]
pub trait BuildHasher {
    /// 将被创建的 hasher 类型。
    #[stable(since = "1.7.0", feature = "build_hasher")]
    type Hasher: Hasher;

    /// 创建一个新的 hasher。
    ///
    /// 对同一个 `BuildHasher` 实例，每次调用 `build_hasher` 都应产生配置相同的 [`Hasher`]。
    /// 这里的“相同”指同一输入流会得到同一输出，而不是要求返回同一个对象。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::hash::{BuildHasher, RandomState};
    ///
    /// let s = RandomState::new();
    /// let new_s = s.build_hasher();
    /// ```
    #[stable(since = "1.7.0", feature = "build_hasher")]
    fn build_hasher(&self) -> Self::Hasher;

    /// 计算单个值的 hash。
    ///
    /// 这是给*消费* hash 的代码使用的便利方法，例如哈希表实现，或者在单元测试中检查自定义
    /// [`Hash`] 实现是否符合预期。
    ///
    /// 任何*创建*复合 hash 的代码都不应使用它，尤其是 [`Hash`] 的实现。多个值的组合 hash
    /// 应通过同一个 [`Hasher`] 多次调用 [`Hash::hash`] 得到，而不是反复调用本方法再手动组合
    /// 若干 `u64` 结果；后者会丢失 `Hasher` 的字节流顺序和 domain separation 语义。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::cmp::{max, min};
    /// use std::hash::{BuildHasher, Hash, Hasher};
    /// struct OrderAmbivalentPair<T: Ord>(T, T);
    /// impl<T: Ord + Hash> Hash for OrderAmbivalentPair<T> {
    ///     fn hash<H: Hasher>(&self, hasher: &mut H) {
    ///         min(&self.0, &self.1).hash(hasher);
    ///         max(&self.0, &self.1).hash(hasher);
    ///     }
    /// }
    ///
    /// // 随后，在该类型的 `#[test]` 中...
    /// let bh = std::hash::RandomState::new();
    /// assert_eq!(
    ///     bh.hash_one(OrderAmbivalentPair(1, 2)),
    ///     bh.hash_one(OrderAmbivalentPair(2, 1))
    /// );
    /// assert_eq!(
    ///     bh.hash_one(OrderAmbivalentPair(10, 2)),
    ///     bh.hash_one(&OrderAmbivalentPair(2, 10))
    /// );
    /// ```
    #[stable(feature = "build_hasher_simple_hash_one", since = "1.71.0")]
    fn hash_one<T: Hash>(&self, x: T) -> u64
    where
        Self: Sized,
        Self::Hasher: Hasher,
    {
        let mut hasher = self.build_hasher();
        x.hash(&mut hasher);
        hasher.finish()
    }
}

/// 为同时实现 [`Hasher`] 和 [`Default`] 的类型创建默认 [`BuildHasher`] 实例。
///
/// 当类型 `H` 已经实现 [`Hasher`] 和 [`Default`]，但没有单独定义对应的 [`BuildHasher`] 时，
/// 可以使用 `BuildHasherDefault<H>` 作为适配器。它把“如何创建 hasher”的职责简化为
/// `H::default()`。
///
/// 任意 `BuildHasherDefault` 都是零大小([zero-sized])的。它可以用 [`default`][method.default]
/// 创建。和 [`HashMap`] 或 [`HashSet`] 一起使用时通常不需要手动创建，因为这些集合会提供
/// 合适的 [`Default`] 实现。
///
/// # 示例
///
/// 使用 `BuildHasherDefault` 为 [`HashMap`] 指定自定义 [`BuildHasher`]：
///
/// ```
/// use std::collections::HashMap;
/// use std::hash::{BuildHasherDefault, Hasher};
///
/// #[derive(Default)]
/// struct MyHasher;
///
/// impl Hasher for MyHasher {
///     fn write(&mut self, bytes: &[u8]) {
///         // 在这里放入你的哈希算法！
///        unimplemented!()
///     }
///
///     fn finish(&self) -> u64 {
///         // 在这里放入你的哈希算法！
///         unimplemented!()
///     }
/// }
///
/// type MyBuildHasher = BuildHasherDefault<MyHasher>;
///
/// let hash_map = HashMap::<u32, u32, MyBuildHasher>::default();
/// ```
///
/// [method.default]: BuildHasherDefault::default
/// [`HashMap`]: ../../std/collections/struct.HashMap.html
/// [`HashSet`]: ../../std/collections/struct.HashSet.html
/// [zero-sized]: https://doc.rust-lang.org/nomicon/exotic-sizes.html#zero-sized-types-zsts
#[stable(since = "1.7.0", feature = "build_hasher")]
pub struct BuildHasherDefault<H>(marker::PhantomData<fn() -> H>);

impl<H> BuildHasherDefault<H> {
    /// 为 hasher 类型 `H` 创建新的 `BuildHasherDefault`。
    #[stable(feature = "build_hasher_default_const_new", since = "1.85.0")]
    #[rustc_const_stable(feature = "build_hasher_default_const_new", since = "1.85.0")]
    pub const fn new() -> Self {
        BuildHasherDefault(marker::PhantomData)
    }
}

#[stable(since = "1.9.0", feature = "core_impl_debug")]
impl<H> fmt::Debug for BuildHasherDefault<H> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BuildHasherDefault").finish()
    }
}

#[stable(since = "1.7.0", feature = "build_hasher")]
impl<H: Default + Hasher> BuildHasher for BuildHasherDefault<H> {
    type Hasher = H;

    fn build_hasher(&self) -> H {
        H::default()
    }
}

#[stable(since = "1.7.0", feature = "build_hasher")]
impl<H> Clone for BuildHasherDefault<H> {
    fn clone(&self) -> BuildHasherDefault<H> {
        BuildHasherDefault(marker::PhantomData)
    }
}

#[stable(since = "1.7.0", feature = "build_hasher")]
#[rustc_const_unstable(feature = "const_default", issue = "143894")]
impl<H> const Default for BuildHasherDefault<H> {
    fn default() -> BuildHasherDefault<H> {
        Self::new()
    }
}

#[stable(since = "1.29.0", feature = "build_hasher_eq")]
impl<H> PartialEq for BuildHasherDefault<H> {
    fn eq(&self, _other: &BuildHasherDefault<H>) -> bool {
        true
    }
}

#[stable(since = "1.29.0", feature = "build_hasher_eq")]
impl<H> Eq for BuildHasherDefault<H> {}

mod impls {
    use super::*;
    use crate::slice;

    macro_rules! impl_write {
        ($(($ty:ident, $meth:ident),)*) => {$(
            #[stable(feature = "rust1", since = "1.0.0")]
            impl Hash for $ty {
                #[inline]
                fn hash<H: Hasher>(&self, state: &mut H) {
                    state.$meth(*self)
                }

                #[inline]
                fn hash_slice<H: Hasher>(data: &[$ty], state: &mut H) {
                    let newlen = size_of_val(data);
                    let ptr = data.as_ptr() as *const u8;
                    // SAFETY: `ptr` 有效且满足对齐要求，因为这个宏只用于没有 padding 的
                    // 数值基本类型。新切片只覆盖原始 `data` 的同一段内存，并且不会被修改；
                    // 它的总字节数与原始 `data` 相同，因此不会超过 `isize::MAX`。
                    state.write(unsafe { slice::from_raw_parts(ptr, newlen) })
                }
            }
        )*}
    }

    impl_write! {
        (u8, write_u8),
        (u16, write_u16),
        (u32, write_u32),
        (u64, write_u64),
        (usize, write_usize),
        (i8, write_i8),
        (i16, write_i16),
        (i32, write_i32),
        (i64, write_i64),
        (isize, write_isize),
        (u128, write_u128),
        (i128, write_i128),
    }

    #[stable(feature = "rust1", since = "1.0.0")]
    impl Hash for bool {
        #[inline]
        fn hash<H: Hasher>(&self, state: &mut H) {
            state.write_u8(*self as u8)
        }
    }

    #[stable(feature = "rust1", since = "1.0.0")]
    impl Hash for char {
        #[inline]
        fn hash<H: Hasher>(&self, state: &mut H) {
            state.write_u32(*self as u32)
        }
    }

    #[stable(feature = "rust1", since = "1.0.0")]
    impl Hash for str {
        #[inline]
        fn hash<H: Hasher>(&self, state: &mut H) {
            state.write_str(self);
        }
    }

    #[stable(feature = "never_hash", since = "1.29.0")]
    impl Hash for ! {
        #[inline]
        fn hash<H: Hasher>(&self, _: &mut H) {
            *self
        }
    }

    macro_rules! impl_hash_tuple {
        () => (
            #[stable(feature = "rust1", since = "1.0.0")]
            impl Hash for () {
                #[inline]
                fn hash<H: Hasher>(&self, _state: &mut H) {}
            }
        );

        ( $($name:ident)+) => (
            maybe_tuple_doc! {
                $($name)+ @
                #[stable(feature = "rust1", since = "1.0.0")]
                impl<$($name: Hash),+> Hash for ($($name,)+) {
                    #[allow(non_snake_case)]
                    #[inline]
                    fn hash<S: Hasher>(&self, state: &mut S) {
                        let ($(ref $name,)+) = *self;
                        $($name.hash(state);)+
                    }
                }
            }
        );
    }

    macro_rules! maybe_tuple_doc {
        ($a:ident @ #[$meta:meta] $item:item) => {
            #[doc(fake_variadic)]
            #[doc = "This trait is implemented for tuples up to twelve items long."]
            #[$meta]
            $item
        };
        ($a:ident $($rest_a:ident)+ @ #[$meta:meta] $item:item) => {
            #[doc(hidden)]
            #[$meta]
            $item
        };
    }

    impl_hash_tuple! {}
    impl_hash_tuple! { T }
    impl_hash_tuple! { T B }
    impl_hash_tuple! { T B C }
    impl_hash_tuple! { T B C D }
    impl_hash_tuple! { T B C D E }
    impl_hash_tuple! { T B C D E F }
    impl_hash_tuple! { T B C D E F G }
    impl_hash_tuple! { T B C D E F G H }
    impl_hash_tuple! { T B C D E F G H I }
    impl_hash_tuple! { T B C D E F G H I J }
    impl_hash_tuple! { T B C D E F G H I J K }
    impl_hash_tuple! { T B C D E F G H I J K L }

    #[stable(feature = "rust1", since = "1.0.0")]
    impl<T: Hash> Hash for [T] {
        #[inline]
        fn hash<H: Hasher>(&self, state: &mut H) {
            state.write_length_prefix(self.len());
            Hash::hash_slice(self, state)
        }
    }

    #[stable(feature = "rust1", since = "1.0.0")]
    impl<T: ?Sized + marker::PointeeSized + Hash> Hash for &T {
        #[inline]
        fn hash<H: Hasher>(&self, state: &mut H) {
            (**self).hash(state);
        }
    }

    #[stable(feature = "rust1", since = "1.0.0")]
    impl<T: ?Sized + marker::PointeeSized + Hash> Hash for &mut T {
        #[inline]
        fn hash<H: Hasher>(&self, state: &mut H) {
            (**self).hash(state);
        }
    }

    #[stable(feature = "rust1", since = "1.0.0")]
    impl<T: ?Sized + marker::PointeeSized> Hash for *const T {
        #[inline]
        fn hash<H: Hasher>(&self, state: &mut H) {
            let (address, metadata) = self.to_raw_parts();
            state.write_usize(address.addr());
            metadata.hash(state);
        }
    }

    #[stable(feature = "rust1", since = "1.0.0")]
    impl<T: ?Sized + marker::PointeeSized> Hash for *mut T {
        #[inline]
        fn hash<H: Hasher>(&self, state: &mut H) {
            let (address, metadata) = self.to_raw_parts();
            state.write_usize(address.addr());
            metadata.hash(state);
        }
    }
}
