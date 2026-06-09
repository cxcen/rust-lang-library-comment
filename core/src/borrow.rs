//! 用于处理借用数据的工具。

#![stable(feature = "rust1", since = "1.0.0")]

/// 一个用于借用数据的 trait。
///
/// 在 Rust 中,为同一类型针对不同的使用场景提供不同的表示形式是很常见的。
/// 例如,一个值的存储位置与管理方式,可以通过诸如 [`Box<T>`] 或 [`Rc<T>`]
/// 之类的指针类型,按特定用途的需要专门选定。除了这些可用于任意类型的通用
/// 包装之外,某些类型还提供一些可选的“切面”(facet),带来可能开销不菲的
/// 功能。这样一个类型的例子是 [`String`],它在基本的 [`str`] 之上增加了
/// 扩展字符串的能力。这需要保存一些对于简单的、不可变的字符串而言并不必要的
/// 额外信息。
///
/// 这些类型通过指向其底层数据类型的引用来提供对底层数据的访问。我们说它们
/// 可以“借用为”(borrowed as)那个类型。例如,[`Box<T>`] 可以借用为 `T`,
/// 而 [`String`] 可以借用为 `str`。
///
/// 类型通过实现 `Borrow<T>` 来表达自己可以借用为某个类型 `T`,具体做法是在
/// 该 trait 的 [`borrow`] 方法中提供一个指向 `T` 的引用。一个类型可以自由地
/// 借用为好几种不同的类型。如果它希望可变地借用为某个类型,从而允许修改底层
/// 数据,它可以额外实现 [`BorrowMut<T>`]。
///
/// 此外,在为额外的 trait 提供实现时,需要考虑:既然该类型充当其底层类型的
/// 一种表示,那么这些 trait 实现是否应当表现得与底层类型的实现完全一致。
/// 当泛型代码依赖这些额外 trait 实现的一致行为时,它通常会使用 `Borrow<T>`。
/// 这些 trait 很可能会作为额外的 trait 约束出现。
///
/// 特别地,`Eq`、`Ord` 和 `Hash` 对于借用值与拥有值必须是等价的:
/// `x.borrow() == y.borrow()` 应当给出与 `x == y` 相同的结果。
///
/// 如果泛型代码只是需要对所有能提供相关类型 `T` 的引用的类型都能工作,
/// 那么通常更好的选择是使用 [`AsRef<T>`],因为有更多类型能安全地实现它。
///
/// [`Box<T>`]: ../../std/boxed/struct.Box.html
/// [`Mutex<T>`]: ../../std/sync/struct.Mutex.html
/// [`Rc<T>`]: ../../std/rc/struct.Rc.html
/// [`String`]: ../../std/string/struct.String.html
/// [`borrow`]: Borrow::borrow
///
/// # 示例
///
/// 作为一种数据集合,[`HashMap<K, V>`] 同时拥有键和值。然而,如果键的实际
/// 数据被包裹在某种管理类型之中,那么应当仍然可以用一个指向键数据的引用来
/// 搜索某个值。例如,如果键是字符串,那么它很可能以 [`String`] 的形式与
/// 哈希表一起存储,而搜索时应当可以使用一个 [`&str`][`str`]。因此,`insert`
/// 需要在一个 `String` 上操作,而 `get` 则需要能够使用一个 `&str`。
///
/// 略作简化,`HashMap<K, V>` 中相关的部分看上去是这样的:
///
/// ```
/// use std::borrow::Borrow;
/// use std::hash::Hash;
///
/// pub struct HashMap<K, V> {
///     # marker: ::std::marker::PhantomData<(K, V)>,
///     // 字段省略
/// }
///
/// impl<K, V> HashMap<K, V> {
///     pub fn insert(&self, key: K, value: V) -> Option<V>
///     where K: Hash + Eq
///     {
///         # unimplemented!()
///         // ...
///     }
///
///     pub fn get<Q>(&self, k: &Q) -> Option<&V>
///     where
///         K: Borrow<Q>,
///         Q: Hash + Eq + ?Sized
///     {
///         # unimplemented!()
///         // ...
///     }
/// }
/// ```
///
/// 整个哈希表对键类型 `K` 是泛型的。由于这些键与哈希表一起存储,该类型必须
/// 拥有键的数据。在插入一个键值对时,该映射会得到这样一个 `K`,并需要基于
/// 这个 `K` 找到正确的哈希桶、检查该键是否已经存在。因此它要求 `K: Hash + Eq`。
///
/// 然而,在映射中搜索某个值时,如果必须提供一个指向 `K` 的引用来作为待搜索的
/// 键,那就总是不得不创建这样一个拥有所有权的值。对于字符串键来说,这意味着
/// 在只有 `str` 可用的情形下,也得仅仅为了搜索而创建一个 `String` 值。
///
/// 与此不同,`get` 方法对底层键数据的类型(在上面的方法签名中称为 `Q`)是
/// 泛型的。它通过要求 `K: Borrow<Q>` 来声明 `K` 借用为一个 `Q`。又通过额外
/// 要求 `Q: Hash + Eq`,它表明了这样一个要求:`K` 和 `Q` 对 `Hash` 与 `Eq`
/// trait 的实现必须产生一致的结果。
///
/// `get` 的实现尤其依赖 `Hash` 的一致实现:它通过对 `Q` 值调用 `Hash::hash`
/// 来确定该键的哈希桶,尽管它当初是基于由 `K` 值算出的哈希值来插入该键的。
///
/// 因此,如果包裹着 `Q` 值的 `K` 产生的哈希与 `Q` 不同,哈希表就会出错。
/// 例如,设想你有一个包裹字符串、但在比较时忽略 ASCII 字母大小写的类型:
///
/// ```
/// pub struct CaseInsensitiveString(String);
///
/// impl PartialEq for CaseInsensitiveString {
///     fn eq(&self, other: &Self) -> bool {
///         self.0.eq_ignore_ascii_case(&other.0)
///     }
/// }
///
/// impl Eq for CaseInsensitiveString { }
/// ```
///
/// 由于两个相等的值需要产生相同的哈希值,`Hash` 的实现也需要忽略 ASCII
/// 大小写:
///
/// ```
/// # use std::hash::{Hash, Hasher};
/// # pub struct CaseInsensitiveString(String);
/// impl Hash for CaseInsensitiveString {
///     fn hash<H: Hasher>(&self, state: &mut H) {
///         for c in self.0.as_bytes() {
///             c.to_ascii_lowercase().hash(state)
///         }
///     }
/// }
/// ```
///
/// `CaseInsensitiveString` 能实现 `Borrow<str>` 吗?它当然可以通过其内含的
/// 拥有所有权的字符串提供一个指向字符串切片的引用。但由于它的 `Hash` 实现
/// 不同,它的行为与 `str` 不一致,因此实际上 *不得* 实现 `Borrow<str>`。
/// 如果它想允许别人访问底层的 `str`,可以通过 `AsRef<str>` 来做到——后者
/// 不附带任何额外要求。
///
/// [`Hash`]: crate::hash::Hash
/// [`HashMap<K, V>`]: ../../std/collections/struct.HashMap.html
/// [`String`]: ../../std/string/struct.String.html
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_diagnostic_item = "Borrow"]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
pub const trait Borrow<Borrowed: ?Sized> {
    /// 从一个拥有所有权的值不可变地借用。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::borrow::Borrow;
    ///
    /// fn check<T: Borrow<str>>(s: T) {
    ///     assert_eq!("Hello", s.borrow());
    /// }
    ///
    /// let s = "Hello".to_string();
    ///
    /// check(s);
    ///
    /// let s = "Hello";
    ///
    /// check(s);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    fn borrow(&self) -> &Borrowed;
}

/// 一个用于可变地借用数据的 trait。
///
/// 作为 [`Borrow<T>`] 的搭档,本 trait 允许一个类型通过提供可变引用来借用为
/// 某个底层类型。关于借用为另一种类型的更多信息,参见 [`Borrow<T>`]。
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_diagnostic_item = "BorrowMut"]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
pub const trait BorrowMut<Borrowed: ?Sized>: [const] Borrow<Borrowed> {
    /// 从一个拥有所有权的值可变地借用。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::borrow::BorrowMut;
    ///
    /// fn check<T: BorrowMut<[i32]>>(mut v: T) {
    ///     assert_eq!(&mut [1, 2, 3], v.borrow_mut());
    /// }
    ///
    /// let v = vec![1, 2, 3];
    ///
    /// check(v);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    fn borrow_mut(&mut self) -> &mut Borrowed;
}

#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl<T: ?Sized> const Borrow<T> for T {
    #[rustc_diagnostic_item = "noop_method_borrow"]
    fn borrow(&self) -> &T {
        self
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl<T: ?Sized> const BorrowMut<T> for T {
    fn borrow_mut(&mut self) -> &mut T {
        self
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl<T: ?Sized> const Borrow<T> for &T {
    fn borrow(&self) -> &T {
        self
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl<T: ?Sized> const Borrow<T> for &mut T {
    fn borrow(&self) -> &T {
        self
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl<T: ?Sized> const BorrowMut<T> for &mut T {
    fn borrow_mut(&mut self) -> &mut T {
        self
    }
}
