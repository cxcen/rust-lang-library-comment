/// 用于在不可变上下文中进行索引操作(`container[index]`)。
///
/// `container[index]` 实际上是 `*container.index(index)` 的语法糖,但仅当它被
/// 当作不可变值使用时如此。如果请求的是可变值,则改用 [`IndexMut`]。这使得诸如
/// `let value = v[index]`(当 `value` 的类型实现了 [`Copy`] 时)这样的写法成为
/// 可能。
///
/// # 示例
///
/// 下面的示例为一个只读的 `NucleotideCount` 容器实现了 `Index`,从而可以用
/// 索引语法取出各个计数。
///
/// ```
/// use std::ops::Index;
///
/// enum Nucleotide {
///     A,
///     C,
///     G,
///     T,
/// }
///
/// struct NucleotideCount {
///     a: usize,
///     c: usize,
///     g: usize,
///     t: usize,
/// }
///
/// impl Index<Nucleotide> for NucleotideCount {
///     type Output = usize;
///
///     fn index(&self, nucleotide: Nucleotide) -> &Self::Output {
///         match nucleotide {
///             Nucleotide::A => &self.a,
///             Nucleotide::C => &self.c,
///             Nucleotide::G => &self.g,
///             Nucleotide::T => &self.t,
///         }
///     }
/// }
///
/// let nucleotide_count = NucleotideCount {a: 14, c: 9, g: 10, t: 12};
/// assert_eq!(nucleotide_count[Nucleotide::A], 14);
/// assert_eq!(nucleotide_count[Nucleotide::C], 9);
/// assert_eq!(nucleotide_count[Nucleotide::G], 10);
/// assert_eq!(nucleotide_count[Nucleotide::T], 12);
/// ```
#[lang = "index"]
#[diagnostic::on_unimplemented(
    message = "the type `{Self}` cannot be indexed by `{Idx}`",
    label = "`{Self}` cannot be indexed by `{Idx}`"
)]
#[stable(feature = "rust1", since = "1.0.0")]
#[doc(alias = "]")]
#[doc(alias = "[")]
#[doc(alias = "[]")]
#[rustc_const_unstable(feature = "const_index", issue = "143775")]
pub const trait Index<Idx: ?Sized> {
    /// 索引之后返回的类型。
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_diagnostic_item = "IndexOutput"]
    type Output: ?Sized;

    /// 执行索引操作(`container[index]`)。
    ///
    /// # Panics
    ///
    /// 如果索引越界,可能会 panic。
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_no_implicit_autorefs]
    #[track_caller]
    fn index(&self, index: Idx) -> &Self::Output;
}

/// 用于在可变上下文中进行索引操作(`container[index]`)。
///
/// `container[index]` 实际上是 `*container.index_mut(index)` 的语法糖,但仅当
/// 它被当作可变值使用时如此。如果请求的是不可变值,则改用 [`Index`] trait。这
/// 使得诸如 `v[index] = value` 这样的写法成为可能。
///
/// # 示例
///
/// 一个非常简单的 `Balance`(天平)结构体实现,它有两侧,每一侧都可被可变地和
/// 不可变地索引。
///
/// ```
/// use std::ops::{Index, IndexMut};
///
/// #[derive(Debug)]
/// enum Side {
///     Left,
///     Right,
/// }
///
/// #[derive(Debug, PartialEq)]
/// enum Weight {
///     Kilogram(f32),
///     Pound(f32),
/// }
///
/// struct Balance {
///     pub left: Weight,
///     pub right: Weight,
/// }
///
/// impl Index<Side> for Balance {
///     type Output = Weight;
///
///     fn index(&self, index: Side) -> &Self::Output {
///         println!("Accessing {index:?}-side of balance immutably");
///         match index {
///             Side::Left => &self.left,
///             Side::Right => &self.right,
///         }
///     }
/// }
///
/// impl IndexMut<Side> for Balance {
///     fn index_mut(&mut self, index: Side) -> &mut Self::Output {
///         println!("Accessing {index:?}-side of balance mutably");
///         match index {
///             Side::Left => &mut self.left,
///             Side::Right => &mut self.right,
///         }
///     }
/// }
///
/// let mut balance = Balance {
///     right: Weight::Kilogram(2.5),
///     left: Weight::Pound(1.5),
/// };
///
/// // 在这里,`balance[Side::Right]` 是 `*balance.index(Side::Right)` 的语法糖,
/// // 因为我们只是在 *读取* `balance[Side::Right]`,而非写入它。
/// assert_eq!(balance[Side::Right], Weight::Kilogram(2.5));
///
/// // 然而在这里,`balance[Side::Left]` 是 `*balance.index_mut(Side::Left)` 的
/// // 语法糖,因为我们在写入 `balance[Side::Left]`。
/// balance[Side::Left] = Weight::Kilogram(3.0);
/// ```
#[lang = "index_mut"]
#[rustc_on_unimplemented(
    on(
        Self = "&str",
        note = "you can use `.chars().nth()` or `.bytes().nth()`
see chapter in The Book <https://doc.rust-lang.org/book/ch08-02-strings.html#indexing-into-strings>"
    ),
    on(
        Self = "str",
        note = "you can use `.chars().nth()` or `.bytes().nth()`
see chapter in The Book <https://doc.rust-lang.org/book/ch08-02-strings.html#indexing-into-strings>"
    ),
    on(
        Self = "alloc::string::String",
        note = "you can use `.chars().nth()` or `.bytes().nth()`
see chapter in The Book <https://doc.rust-lang.org/book/ch08-02-strings.html#indexing-into-strings>"
    ),
    message = "the type `{Self}` cannot be mutably indexed by `{Idx}`",
    label = "`{Self}` cannot be mutably indexed by `{Idx}`"
)]
#[stable(feature = "rust1", since = "1.0.0")]
#[doc(alias = "[")]
#[doc(alias = "]")]
#[doc(alias = "[]")]
#[rustc_const_unstable(feature = "const_index", issue = "143775")]
pub const trait IndexMut<Idx: ?Sized>: [const] Index<Idx> {
    /// 执行可变索引操作(`container[index]`)。
    ///
    /// # Panics
    ///
    /// 如果索引越界,可能会 panic。
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_no_implicit_autorefs]
    #[track_caller]
    fn index_mut(&mut self, index: Idx) -> &mut Self::Output;
}
