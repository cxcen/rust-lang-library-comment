//! 用于比较值、为值排序的工具。
//!
//! 本模块包含各种用于比较值和排序的工具。概括如下:
//!
//! * [`PartialEq<Rhs>`] 重载 `==` 和 `!=` 运算符。当 `Rhs`(右操作数的类型)
//!   是 `Self` 时,本 trait 对应一个 *部分等价关系*(partial equivalence
//!   relation)。
//! * [`Eq`] 表明被重载的 `==` 运算符对应一个 *等价关系*(equivalence
//!   relation)。
//! * [`Ord`] 和 [`PartialOrd`] 这两个 trait 分别让你定义值之间的全序
//!   (total ordering)和偏序(partial ordering)。实现它们会重载 `<`、
//!   `<=`、`>` 和 `>=` 运算符。
//! * [`Ordering`] 是 [`Ord`] 和 [`PartialOrd`] 的主要函数所返回的枚举,
//!   它描述两个值之间的次序(小于、等于,或大于)。
//! * [`Reverse`] 是一个结构体,让你能够轻松地反转某个次序。
//! * [`max`] 和 [`min`] 是构建于 [`Ord`] 之上的函数,让你能找出两个值中的
//!   最大值或最小值。
//!
//! 更多细节,参见上述列表中各条目各自的文档。
//!
//! [`max`]: Ord::max
//! [`min`]: Ord::min

#![stable(feature = "rust1", since = "1.0.0")]

mod bytewise;
pub(crate) use bytewise::BytewiseEq;

use self::Ordering::*;
use crate::marker::{Destruct, PointeeSized};
use crate::ops::ControlFlow;

/// 用于使用相等运算符进行比较的 trait。
///
/// 为类型实现本 trait,就为这些类型提供了 `==` 和 `!=` 运算符。
///
/// `x.eq(y)` 也可以写作 `x == y`,而 `x.ne(y)` 可以写作 `x != y`。在本文档
/// 的余下部分中,我们使用更易读的中缀记法。
///
/// 本 trait 允许那些不具备完整等价关系的类型也能使用相等运算符进行比较。
/// 例如,在浮点数中 `NaN != NaN`,所以浮点类型实现了 `PartialEq` 但不实现
/// [`trait@Eq`]。正式地说,当 `Rhs == Self` 时,本 trait 对应一个
/// [部分等价关系][partial equivalence relation]。
///
/// [partial equivalence relation]: https://en.wikipedia.org/wiki/Partial_equivalence_relation
///
/// 实现必须确保 `eq` 与 `ne` 彼此一致:
///
/// - `a != b` 当且仅当 `!(a == b)`。
///
/// `ne` 的默认实现提供了这种一致性,并且几乎总是够用。没有非常充分的理由,
/// 不应当重写它。
///
/// 如果 `Self` 和 `Rhs` 还实现了 [`PartialOrd`] 或 [`Ord`],它们的方法也必须
/// 与 `PartialEq` 保持一致(确切要求见那些 trait 的文档)。通过派生其中一些
/// trait、手动实现另一些,很容易在不经意间让它们彼此矛盾。
///
/// 相等关系 `==` 必须满足以下条件(对所有 `A`、`B`、`C` 类型的 `a`、`b`、
/// `c`):
///
/// - **对称性(Symmetry)**:如果 `A: PartialEq<B>` 且 `B: PartialEq<A>`,
///   那么 **`a == b` 蕴含 `b == a`**;以及
///
/// - **传递性(Transitivity)**:如果 `A: PartialEq<B>`、`B: PartialEq<C>`
///   且 `A: PartialEq<C>`,那么 **`a == b` 且 `b == c` 蕴含 `a == c`**。
///   这对更长的链条同样必须成立,例如当 `A: PartialEq<B>`、`B: PartialEq<C>`、
///   `C: PartialEq<D>` 和 `A: PartialEq<D>` 同时存在时。
///
/// 注意,并不强制要求 `B: PartialEq<A>`(对称)和 `A: PartialEq<C>`(传递)
/// 这些 impl 必须存在,但只要它们存在,这些要求就适用。
///
/// 违反这些要求属于逻辑错误。逻辑错误所导致的行为是未指定的,但 trait 的
/// 使用者必须确保此类逻辑错误 *不会* 导致未定义行为。这意味着 `unsafe` 代码
/// **不得** 依赖这些方法的正确性。需要强调:违反对称性/传递性 *本身* 不是
/// 未定义行为,但会让排序、查找、集合去重等逻辑产生错误结果。
///
/// ## 跨 crate 的考量
///
/// 当一个 crate 为另一个 crate 的类型实现 `PartialEq`(即为了允许把自己的
/// 某个类型与标准库的类型相比较)时,要维持上述要求会变得棘手。建议是:
/// 永远不要为外来类型(foreign type)实现本 trait。换句话说,这样的 crate
/// 应当写 `impl PartialEq<ForeignType> for LocalType`,而 *不应* 写
/// `impl PartialEq<LocalType> for ForeignType`。
///
/// 这样可以避免那种横跨 crate 边界、纵横交错的传递链问题:对于所有本地
/// 类型 `T`,你可以假定没有别的 crate 会添加允许比较 `T == U` 的 impl。
/// 换句话说,如果其他 crate 添加 impl 来构建更长的传递链
/// `U1 == ... == T == V1 == ...`,那么出现在 `T` 右侧的所有类型都必须是
/// 定义 `T` 的那个 crate 早已知道的类型。这就排除了这样一种传递链:下游
/// crate 可以添加新的 impl,以违反传递性的方式把外来类型的比较“拼接”起来。
///
/// 不存在这样的外来 impl,也避免了前向兼容性问题——否则一个 crate 增加更多
/// `PartialEq` 实现就可能导致下游 crate 构建失败。
///
/// ## 可派生(Derivable)
///
/// 本 trait 可以配合 `#[derive]` 使用。在结构体上 `derive` 时,若所有字段
/// 都相等,则两个实例相等;若有任何字段不相等,则不相等。在枚举上 `derive`
/// 时,若两个实例是同一个变体且所有字段都相等,则它们相等。
///
/// ## 如何实现 `PartialEq`?
///
/// 下面是一个示例实现,在这个领域里,只要两本书的 ISBN 相同就被认为是同一
/// 本书,即便它们的版式不同:
///
/// ```
/// enum BookFormat {
///     Paperback,
///     Hardback,
///     Ebook,
/// }
///
/// struct Book {
///     isbn: i32,
///     format: BookFormat,
/// }
///
/// impl PartialEq for Book {
///     fn eq(&self, other: &Self) -> bool {
///         self.isbn == other.isbn
///     }
/// }
///
/// let b1 = Book { isbn: 3, format: BookFormat::Paperback };
/// let b2 = Book { isbn: 3, format: BookFormat::Ebook };
/// let b3 = Book { isbn: 10, format: BookFormat::Paperback };
///
/// assert!(b1 == b2);
/// assert!(b1 != b3);
/// ```
///
/// ## 如何比较两种不同的类型?
///
/// 你能与之比较的类型由 `PartialEq` 的类型参数控制。例如,我们把前面的代码
/// 稍作改动:
///
/// ```
/// // 这个 derive 实现 <BookFormat> == <BookFormat> 的比较
/// #[derive(PartialEq)]
/// enum BookFormat {
///     Paperback,
///     Hardback,
///     Ebook,
/// }
///
/// struct Book {
///     isbn: i32,
///     format: BookFormat,
/// }
///
/// // 实现 <Book> == <BookFormat> 的比较
/// impl PartialEq<BookFormat> for Book {
///     fn eq(&self, other: &BookFormat) -> bool {
///         self.format == *other
///     }
/// }
///
/// // 实现 <BookFormat> == <Book> 的比较
/// impl PartialEq<Book> for BookFormat {
///     fn eq(&self, other: &Book) -> bool {
///         *self == other.format
///     }
/// }
///
/// let b1 = Book { isbn: 3, format: BookFormat::Paperback };
///
/// assert!(b1 == BookFormat::Paperback);
/// assert!(BookFormat::Ebook != b1);
/// ```
///
/// 通过把 `impl PartialEq for Book` 改成 `impl PartialEq<BookFormat> for Book`,
/// 我们就允许了 `BookFormat` 与 `Book` 相比较。
///
/// 像上面这种忽略结构体某些字段的比较可能是危险的。它很容易在不经意间违反
/// 部分等价关系的要求。例如,如果我们保留上面那个为 `BookFormat` 实现的
/// `PartialEq<Book>`,又添加一个为 `Book` 实现的 `PartialEq<Book>`(无论是
/// 通过 `#[derive]`,还是通过第一个例子里的手动实现),那么结果就会违反
/// 传递性:
///
/// ```should_panic
/// #[derive(PartialEq)]
/// enum BookFormat {
///     Paperback,
///     Hardback,
///     Ebook,
/// }
///
/// #[derive(PartialEq)]
/// struct Book {
///     isbn: i32,
///     format: BookFormat,
/// }
///
/// impl PartialEq<BookFormat> for Book {
///     fn eq(&self, other: &BookFormat) -> bool {
///         self.format == *other
///     }
/// }
///
/// impl PartialEq<Book> for BookFormat {
///     fn eq(&self, other: &Book) -> bool {
///         *self == other.format
///     }
/// }
///
/// fn main() {
///     let b1 = Book { isbn: 1, format: BookFormat::Paperback };
///     let b2 = Book { isbn: 2, format: BookFormat::Paperback };
///
///     assert!(b1 == BookFormat::Paperback);
///     assert!(BookFormat::Paperback == b2);
///
///     // 下面这一条按传递性本应成立,实则不然。
///     assert!(b1 == b2); // <-- 会 PANIC
/// }
/// ```
///
/// # 示例
///
/// ```
/// let x: u32 = 0;
/// let y: u32 = 1;
///
/// assert_eq!(x == y, false);
/// assert_eq!(x.eq(&y), false);
/// ```
///
/// [`eq`]: PartialEq::eq
/// [`ne`]: PartialEq::ne
#[lang = "eq"]
#[stable(feature = "rust1", since = "1.0.0")]
#[doc(alias = "==")]
#[doc(alias = "!=")]
#[rustc_on_unimplemented(
    message = "can't compare `{Self}` with `{Rhs}`",
    label = "no implementation for `{Self} == {Rhs}`",
    append_const_msg
)]
#[rustc_diagnostic_item = "PartialEq"]
#[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
pub const trait PartialEq<Rhs: PointeeSized = Self>: PointeeSized {
    /// 检验 `self` 与 `other` 两个值是否相等,被 `==` 使用。
    #[must_use]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_diagnostic_item = "cmp_partialeq_eq"]
    fn eq(&self, other: &Rhs) -> bool;

    /// 检验 `!=`。默认实现几乎总是够用,没有非常充分的理由不应重写。
    #[inline]
    #[must_use]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_diagnostic_item = "cmp_partialeq_ne"]
    fn ne(&self, other: &Rhs) -> bool {
        !self.eq(other)
    }
}

/// 生成 [`PartialEq`] trait 实现的派生宏。
/// 本宏的行为在[此处](PartialEq#derivable)有详细描述。
#[rustc_builtin_macro]
#[stable(feature = "builtin_macro_prelude", since = "1.38.0")]
#[allow_internal_unstable(core_intrinsics, structural_match)]
pub macro PartialEq($item:item) {
    /* 编译器内建 */
}

/// 对应于[等价关系](https://en.wikipedia.org/wiki/Equivalence_relation)的比较 trait。
///
/// 与 [`PartialEq`] 的主要区别在于,它额外要求满足 *自反性*(reflexivity)。
/// 一个实现 [`PartialEq`] 的类型保证对所有 `a`、`b` 和 `c`:
///
/// - 对称(symmetric):`a == b` 蕴含 `b == a`,且 `a != b` 蕴含 `!(a == b)`
/// - 传递(transitive):`a == b` 且 `b == c` 蕴含 `a == c`
///
/// 构建于 [`PartialEq`] 之上的 `Eq` 还额外蕴含:
///
/// - 自反(reflexive):`a == a`
///
/// 这一性质无法由编译器检查,因此 `Eq` 是一个没有方法的 trait。
///
/// 违反这一性质属于逻辑错误。逻辑错误所导致的行为是未指定的,但 trait 的
/// 使用者必须确保此类逻辑错误 *不会* 导致未定义行为。这意味着 `unsafe` 代码
/// **不得** 依赖这些方法的正确性。
///
/// 诸如 [`f32`] 和 [`f64`] 之类的浮点类型只实现 [`PartialEq`] 而 *不* 实现
/// `Eq`,因为 `NaN` != `NaN`。这正是浮点数破坏全序、只能有偏序/部分等价的
/// 根本原因。
///
/// ## 可派生(Derivable)
///
/// 本 trait 可以配合 `#[derive]` 使用。由于 `Eq` 没有额外的方法,`derive`
/// 时它只是告诉编译器这是一个等价关系而非部分等价关系。注意,`derive` 策略
/// 要求所有字段都是 `Eq`,而这并不总是我们想要的。
///
/// ## 如何实现 `Eq`?
///
/// 如果你不能使用 `derive` 策略,就声明你的类型实现 `Eq`(它没有额外的方法):
///
/// ```
/// enum BookFormat {
///     Paperback,
///     Hardback,
///     Ebook,
/// }
///
/// struct Book {
///     isbn: i32,
///     format: BookFormat,
/// }
///
/// impl PartialEq for Book {
///     fn eq(&self, other: &Self) -> bool {
///         self.isbn == other.isbn
///     }
/// }
///
/// impl Eq for Book {}
/// ```
#[doc(alias = "==")]
#[doc(alias = "!=")]
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_diagnostic_item = "Eq"]
#[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
pub const trait Eq: [const] PartialEq<Self> + PointeeSized {
    // 这个方法仅供 `impl Eq` 或 `#[derive(Eq)]` 使用,用来断言一个类型的每个
    // 组成部分自身也都实现了 `Eq`。当前的派生基础设施意味着:若不借助本 trait
    // 上的某个方法,几乎不可能做出这一断言。
    //
    // 这个方法绝不应由手工实现。
    #[doc(hidden)]
    #[coverage(off)]
    #[inline]
    #[stable(feature = "rust1", since = "1.0.0")]
    fn assert_receiver_is_total_eq(&self) {}
}

/// 生成 [`Eq`] trait 实现的派生宏。
#[rustc_builtin_macro]
#[stable(feature = "builtin_macro_prelude", since = "1.38.0")]
#[allow_internal_unstable(core_intrinsics, derive_eq_internals, structural_match)]
#[allow_internal_unstable(coverage_attribute)]
pub macro Eq($item:item) {
    /* 编译器内建 */
}

// FIXME:这个结构体仅供 #[derive] 使用,用来断言一个类型的每个组成部分都
// 实现了 Eq。
//
// 这个结构体绝不应出现在用户代码中。
#[doc(hidden)]
#[allow(missing_debug_implementations)]
#[unstable(
    feature = "derive_eq_internals",
    reason = "deriving hack, should not be public",
    issue = "none"
)]
pub struct AssertParamIsEq<T: Eq + PointeeSized> {
    _field: crate::marker::PhantomData<T>,
}

/// `Ordering` 是对两个值进行比较所得到的结果。
///
/// # 示例
///
/// ```
/// use std::cmp::Ordering;
///
/// assert_eq!(1.cmp(&2), Ordering::Less);
///
/// assert_eq!(1.cmp(&1), Ordering::Equal);
///
/// assert_eq!(2.cmp(&1), Ordering::Greater);
/// ```
#[derive(Copy, Debug, Hash)]
#[derive_const(Clone, Eq, PartialOrd, Ord, PartialEq)]
#[stable(feature = "rust1", since = "1.0.0")]
// 它之所以是一个 lang item,仅仅是为了让 MIR 中的 `BinOp::Cmp` 能返回它。
// 它没有任何特殊行为,但确实要求 `Less`/`Equal`/`Greater` 这三个变体的判别值
// 分别保持为 `-1_i8`/`0_i8`/`+1_i8`。
#[lang = "Ordering"]
#[repr(i8)]
pub enum Ordering {
    /// 表示被比较的值小于另一个值的次序。
    #[stable(feature = "rust1", since = "1.0.0")]
    Less = -1,
    /// 表示被比较的值等于另一个值的次序。
    #[stable(feature = "rust1", since = "1.0.0")]
    Equal = 0,
    /// 表示被比较的值大于另一个值的次序。
    #[stable(feature = "rust1", since = "1.0.0")]
    Greater = 1,
}

impl Ordering {
    #[inline]
    const fn as_raw(self) -> i8 {
        // FIXME(const-hack):一旦 `PartialOrd` 变为 const,就直接拿它与 `Equal` 比较
        crate::intrinsics::discriminant_value(&self)
    }

    /// 如果该次序是 `Equal` 变体,返回 `true`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::cmp::Ordering;
    ///
    /// assert_eq!(Ordering::Less.is_eq(), false);
    /// assert_eq!(Ordering::Equal.is_eq(), true);
    /// assert_eq!(Ordering::Greater.is_eq(), false);
    /// ```
    #[inline]
    #[must_use]
    #[rustc_const_stable(feature = "ordering_helpers", since = "1.53.0")]
    #[stable(feature = "ordering_helpers", since = "1.53.0")]
    pub const fn is_eq(self) -> bool {
        // 所有 `is_*` 方法都实现为“与零比较”,以沿用 clang libcxx 中其等价实现的做法
        // <https://github.com/llvm/llvm-project/blob/60486292b79885b7800b082754153202bef5b1f0/libcxx/include/__compare/is_eq.h#L23-L28>

        self.as_raw() == 0
    }

    /// 如果该次序不是 `Equal` 变体,返回 `true`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::cmp::Ordering;
    ///
    /// assert_eq!(Ordering::Less.is_ne(), true);
    /// assert_eq!(Ordering::Equal.is_ne(), false);
    /// assert_eq!(Ordering::Greater.is_ne(), true);
    /// ```
    #[inline]
    #[must_use]
    #[rustc_const_stable(feature = "ordering_helpers", since = "1.53.0")]
    #[stable(feature = "ordering_helpers", since = "1.53.0")]
    pub const fn is_ne(self) -> bool {
        self.as_raw() != 0
    }

    /// 如果该次序是 `Less` 变体,返回 `true`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::cmp::Ordering;
    ///
    /// assert_eq!(Ordering::Less.is_lt(), true);
    /// assert_eq!(Ordering::Equal.is_lt(), false);
    /// assert_eq!(Ordering::Greater.is_lt(), false);
    /// ```
    #[inline]
    #[must_use]
    #[rustc_const_stable(feature = "ordering_helpers", since = "1.53.0")]
    #[stable(feature = "ordering_helpers", since = "1.53.0")]
    pub const fn is_lt(self) -> bool {
        self.as_raw() < 0
    }

    /// 如果该次序是 `Greater` 变体,返回 `true`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::cmp::Ordering;
    ///
    /// assert_eq!(Ordering::Less.is_gt(), false);
    /// assert_eq!(Ordering::Equal.is_gt(), false);
    /// assert_eq!(Ordering::Greater.is_gt(), true);
    /// ```
    #[inline]
    #[must_use]
    #[rustc_const_stable(feature = "ordering_helpers", since = "1.53.0")]
    #[stable(feature = "ordering_helpers", since = "1.53.0")]
    pub const fn is_gt(self) -> bool {
        self.as_raw() > 0
    }

    /// 如果该次序是 `Less` 或 `Equal` 变体之一,返回 `true`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::cmp::Ordering;
    ///
    /// assert_eq!(Ordering::Less.is_le(), true);
    /// assert_eq!(Ordering::Equal.is_le(), true);
    /// assert_eq!(Ordering::Greater.is_le(), false);
    /// ```
    #[inline]
    #[must_use]
    #[rustc_const_stable(feature = "ordering_helpers", since = "1.53.0")]
    #[stable(feature = "ordering_helpers", since = "1.53.0")]
    pub const fn is_le(self) -> bool {
        self.as_raw() <= 0
    }

    /// 如果该次序是 `Greater` 或 `Equal` 变体之一,返回 `true`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::cmp::Ordering;
    ///
    /// assert_eq!(Ordering::Less.is_ge(), false);
    /// assert_eq!(Ordering::Equal.is_ge(), true);
    /// assert_eq!(Ordering::Greater.is_ge(), true);
    /// ```
    #[inline]
    #[must_use]
    #[rustc_const_stable(feature = "ordering_helpers", since = "1.53.0")]
    #[stable(feature = "ordering_helpers", since = "1.53.0")]
    pub const fn is_ge(self) -> bool {
        self.as_raw() >= 0
    }

    /// 反转该 `Ordering`。
    ///
    /// * `Less` 变成 `Greater`。
    /// * `Greater` 变成 `Less`。
    /// * `Equal` 保持 `Equal`。
    ///
    /// # 示例
    ///
    /// 基本行为:
    ///
    /// ```
    /// use std::cmp::Ordering;
    ///
    /// assert_eq!(Ordering::Less.reverse(), Ordering::Greater);
    /// assert_eq!(Ordering::Equal.reverse(), Ordering::Equal);
    /// assert_eq!(Ordering::Greater.reverse(), Ordering::Less);
    /// ```
    ///
    /// 这个方法可以用来反转一次比较:
    ///
    /// ```
    /// let data: &mut [_] = &mut [2, 10, 5, 8];
    ///
    /// // 把数组从大到小排序。
    /// data.sort_by(|a, b| a.cmp(b).reverse());
    ///
    /// let b: &mut [_] = &mut [10, 8, 5, 2];
    /// assert!(data == b);
    /// ```
    #[inline]
    #[must_use]
    #[rustc_const_stable(feature = "const_ordering", since = "1.48.0")]
    #[stable(feature = "rust1", since = "1.0.0")]
    pub const fn reverse(self) -> Ordering {
        match self {
            Less => Greater,
            Equal => Equal,
            Greater => Less,
        }
    }

    /// 把两个次序串联起来。
    ///
    /// 当 `self` 不是 `Equal` 时返回 `self`。否则返回 `other`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::cmp::Ordering;
    ///
    /// let result = Ordering::Equal.then(Ordering::Less);
    /// assert_eq!(result, Ordering::Less);
    ///
    /// let result = Ordering::Less.then(Ordering::Equal);
    /// assert_eq!(result, Ordering::Less);
    ///
    /// let result = Ordering::Less.then(Ordering::Greater);
    /// assert_eq!(result, Ordering::Less);
    ///
    /// let result = Ordering::Equal.then(Ordering::Equal);
    /// assert_eq!(result, Ordering::Equal);
    ///
    /// let x: (i64, i64, i64) = (1, 2, 7);
    /// let y: (i64, i64, i64) = (1, 5, 3);
    /// let result = x.0.cmp(&y.0).then(x.1.cmp(&y.1)).then(x.2.cmp(&y.2));
    ///
    /// assert_eq!(result, Ordering::Less);
    /// ```
    #[inline]
    #[must_use]
    #[rustc_const_stable(feature = "const_ordering", since = "1.48.0")]
    #[stable(feature = "ordering_chaining", since = "1.17.0")]
    pub const fn then(self, other: Ordering) -> Ordering {
        match self {
            Equal => other,
            _ => self,
        }
    }

    /// 把该次序与给定函数串联起来。
    ///
    /// 当 `self` 不是 `Equal` 时返回 `self`。否则调用 `f` 并返回其结果。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::cmp::Ordering;
    ///
    /// let result = Ordering::Equal.then_with(|| Ordering::Less);
    /// assert_eq!(result, Ordering::Less);
    ///
    /// let result = Ordering::Less.then_with(|| Ordering::Equal);
    /// assert_eq!(result, Ordering::Less);
    ///
    /// let result = Ordering::Less.then_with(|| Ordering::Greater);
    /// assert_eq!(result, Ordering::Less);
    ///
    /// let result = Ordering::Equal.then_with(|| Ordering::Equal);
    /// assert_eq!(result, Ordering::Equal);
    ///
    /// let x: (i64, i64, i64) = (1, 2, 7);
    /// let y: (i64, i64, i64) = (1, 5, 3);
    /// let result = x.0.cmp(&y.0).then_with(|| x.1.cmp(&y.1)).then_with(|| x.2.cmp(&y.2));
    ///
    /// assert_eq!(result, Ordering::Less);
    /// ```
    #[inline]
    #[must_use]
    #[stable(feature = "ordering_chaining", since = "1.17.0")]
    #[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
    pub const fn then_with<F>(self, f: F) -> Ordering
    where
        F: [const] FnOnce() -> Ordering + [const] Destruct,
    {
        match self {
            Equal => f(),
            _ => self,
        }
    }
}

/// 一个用于反转次序的辅助结构体。
///
/// 本结构体是一个辅助工具,可与诸如 [`Vec::sort_by_key`] 之类的函数配合使用,
/// 用来对键的某一部分进行倒序排列。
///
/// [`Vec::sort_by_key`]: ../../std/vec/struct.Vec.html#method.sort_by_key
///
/// # 示例
///
/// ```
/// use std::cmp::Reverse;
///
/// let mut v = vec![1, 2, 3, 4, 5, 6];
/// v.sort_by_key(|&num| (num > 3, Reverse(num)));
/// assert_eq!(v, vec![3, 2, 1, 6, 5, 4]);
/// ```
#[derive(Copy, Debug, Hash)]
#[derive_const(PartialEq, Eq, Default)]
#[stable(feature = "reverse_cmp_key", since = "1.19.0")]
#[repr(transparent)]
pub struct Reverse<T>(#[stable(feature = "reverse_cmp_key", since = "1.19.0")] pub T);

#[stable(feature = "reverse_cmp_key", since = "1.19.0")]
#[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
impl<T: [const] PartialOrd> const PartialOrd for Reverse<T> {
    #[inline]
    fn partial_cmp(&self, other: &Reverse<T>) -> Option<Ordering> {
        other.0.partial_cmp(&self.0)
    }

    #[inline]
    fn lt(&self, other: &Self) -> bool {
        other.0 < self.0
    }
    #[inline]
    fn le(&self, other: &Self) -> bool {
        other.0 <= self.0
    }
    #[inline]
    fn gt(&self, other: &Self) -> bool {
        other.0 > self.0
    }
    #[inline]
    fn ge(&self, other: &Self) -> bool {
        other.0 >= self.0
    }
}

#[stable(feature = "reverse_cmp_key", since = "1.19.0")]
#[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
impl<T: [const] Ord> const Ord for Reverse<T> {
    #[inline]
    fn cmp(&self, other: &Reverse<T>) -> Ordering {
        other.0.cmp(&self.0)
    }
}

#[stable(feature = "reverse_cmp_key", since = "1.19.0")]
impl<T: Clone> Clone for Reverse<T> {
    #[inline]
    fn clone(&self) -> Reverse<T> {
        Reverse(self.0.clone())
    }

    #[inline]
    fn clone_from(&mut self, source: &Self) {
        self.0.clone_from(&source.0)
    }
}

/// 面向构成[全序](https://en.wikipedia.org/wiki/Total_order)的类型的 trait。
///
/// 实现必须与 [`PartialOrd`] 的实现保持一致,并确保 `max`、`min`、`clamp`
/// 与 `cmp` 保持一致:
///
/// - `partial_cmp(a, b) == Some(cmp(a, b))`。
/// - `max(a, b) == max_by(a, b, cmp)`(由默认实现保证)。
/// - `min(a, b) == min_by(a, b, cmp)`(由默认实现保证)。
/// - 关于 `a.clamp(min, max)`,参见[方法文档](#method.clamp)(由默认实现保证)。
///
/// 违反这些要求属于逻辑错误。逻辑错误所导致的行为是未指定的,但 trait 的
/// 使用者必须确保此类逻辑错误 *不会* 导致未定义行为。这意味着 `unsafe` 代码
/// **不得** 依赖这些方法的正确性。需要强调:违反全序约定(自反性/反对称性/
/// 传递性)本身不是未定义行为,但会让排序、二分查找等算法给出错误结果。
///
/// ## 推论
///
/// 由上述要求以及 `PartialOrd` 的要求可知,对所有 `a`、`b` 和 `c`:
///
/// - `a < b`、`a == b` 或 `a > b` 三者中恰有一个为真;以及
/// - `<` 是传递的:`a < b` 且 `b < c` 蕴含 `a < c`。`==` 和 `>` 也必须如此。
///
/// 从数学上讲,`<` 运算符定义了一个严格[弱序][weak order]。当 `==` 符合
/// 数学意义上的相等时,它还定义了一个严格[全序][total order]。
///
/// [weak order]: https://en.wikipedia.org/wiki/Weak_ordering
/// [total order]: https://en.wikipedia.org/wiki/Total_order
///
/// ## 可派生(Derivable)
///
/// 本 trait 可以配合 `#[derive]` 使用。
///
/// 在结构体上 `derive` 时,它会基于结构体成员从上到下的声明顺序,产生一个
/// [字典序](https://en.wikipedia.org/wiki/Lexicographic_order)的次序。
///
/// 在枚举上 `derive` 时,变体首先按其判别值(discriminant)排序;其次,
/// 再按其字段排序。默认情况下,靠上的变体判别值最小,靠下的变体判别值最大。
/// 这里有一个例子:
///
/// ```
/// #[derive(PartialEq, Eq, PartialOrd, Ord)]
/// enum E {
///     Top,
///     Bottom,
/// }
///
/// assert!(E::Top < E::Bottom);
/// ```
///
/// 然而,手动设置判别值可以覆盖这一默认行为:
///
/// ```
/// #[derive(PartialEq, Eq, PartialOrd, Ord)]
/// enum E {
///     Top = 2,
///     Bottom = 1,
/// }
///
/// assert!(E::Bottom < E::Top);
/// ```
///
/// ## 字典序比较
///
/// 字典序比较是一种具有以下性质的操作:
///  - 两个序列逐元素地比较。
///  - 第一个不匹配的元素决定哪个序列在字典序上更小或更大。
///  - 如果一个序列是另一个序列的前缀,那么较短的序列在字典序上更小。
///  - 如果两个序列的元素对应相等且长度相同,那么这两个序列在字典序上相等。
///  - 空序列在字典序上小于任何非空序列。
///  - 两个空序列在字典序上相等。
///
/// ## 如何实现 `Ord`?
///
/// `Ord` 要求该类型同时也是 [`PartialOrd`]、[`PartialEq`] 和 [`Eq`]。
///
/// 由于 `Ord` 蕴含着比 [`PartialOrd`] 更强的次序关系,而且 `Ord` 与
/// [`PartialOrd`] 二者必须一致,所以你必须 **首先** 决定如何实现 `Ord`。
/// 你可以选择派生它,或手动实现它。如果你派生它,就应当派生全部四个 trait。
/// 如果你手动实现它,就应当基于 `Ord` 的实现,手动实现全部四个 trait。
///
/// 下面是一个例子,你想要只按 `health` 和 `experience` 来定义 `Character`
/// 的比较,而不考虑字段 `mana`:
///
/// ```
/// use std::cmp::Ordering;
///
/// struct Character {
///     health: u32,
///     experience: u32,
///     mana: f32,
/// }
///
/// impl Ord for Character {
///     fn cmp(&self, other: &Self) -> Ordering {
///         self.experience
///             .cmp(&other.experience)
///             .then(self.health.cmp(&other.health))
///     }
/// }
///
/// impl PartialOrd for Character {
///     fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
///         Some(self.cmp(other))
///     }
/// }
///
/// impl PartialEq for Character {
///     fn eq(&self, other: &Self) -> bool {
///         self.health == other.health && self.experience == other.experience
///     }
/// }
///
/// impl Eq for Character {}
/// ```
///
/// 如果你需要的只是按某个字段的值来 `slice::sort` 一个类型,那么使用
/// `slice::sort_by_key` 也许更简单。
///
/// ## 错误的 `Ord` 实现示例
///
/// ```
/// use std::cmp::Ordering;
///
/// #[derive(Debug)]
/// struct Character {
///     health: f32,
/// }
///
/// impl Ord for Character {
///     fn cmp(&self, other: &Self) -> std::cmp::Ordering {
///         if self.health < other.health {
///             Ordering::Less
///         } else if self.health > other.health {
///             Ordering::Greater
///         } else {
///             Ordering::Equal
///         }
///     }
/// }
///
/// impl PartialOrd for Character {
///     fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
///         Some(self.cmp(other))
///     }
/// }
///
/// impl PartialEq for Character {
///     fn eq(&self, other: &Self) -> bool {
///         self.health == other.health
///     }
/// }
///
/// impl Eq for Character {}
///
/// let a = Character { health: 4.5 };
/// let b = Character { health: f32::NAN };
///
/// // 错误:浮点值并不构成全序,即便无视这一事实、用内建比较运算符来实现
/// // `Ord`,也改变不了这个现实。如果你需要对浮点值进行全序比较,请使用
/// // `f32::total_cmp`。
///
/// // 不满足 `Ord` 的自反性要求。
/// assert!(a == a);
/// assert!(b != b);
///
/// // 不满足 `Ord` 的反对称性要求。a < c 和 c < a 中只允许有一个为真,
/// // 不能两个都真,也不能两个都假。
/// assert_eq!((a < b) as u8 + (b < a) as u8, 0);
/// ```
///
/// ```
/// use std::cmp::Ordering;
///
/// #[derive(Debug)]
/// struct Character {
///     health: u32,
///     experience: u32,
/// }
///
/// impl PartialOrd for Character {
///     fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
///         Some(self.cmp(other))
///     }
/// }
///
/// impl Ord for Character {
///     fn cmp(&self, other: &Self) -> std::cmp::Ordering {
///         if self.health < 50 {
///             self.health.cmp(&other.health)
///         } else {
///             self.experience.cmp(&other.experience)
///         }
///     }
/// }
///
/// // 出于性能原因,这样实现 `PartialEq` 并非地道的写法,但在本例中它确保了
/// // `PartialEq`、`PartialOrd` 与 `Ord` 之间行为的一致性。
/// impl PartialEq for Character {
///     fn eq(&self, other: &Self) -> bool {
///         self.cmp(other) == Ordering::Equal
///     }
/// }
///
/// impl Eq for Character {}
///
/// let a = Character {
///     health: 3,
///     experience: 5,
/// };
/// let b = Character {
///     health: 10,
///     experience: 77,
/// };
/// let c = Character {
///     health: 143,
///     experience: 2,
/// };
///
/// // 错误:`Ord` 的实现根据 `self.health` 的值去比较不同的字段,结果得到的
/// // 次序不是全序。
///
/// // 不满足 `Ord` 的传递性要求。如果 a 小于 b 且 b 小于 c,那么按传递性,
/// // a 也必须小于 c。
/// assert!(a < b && b < c && c < a);
///
/// // 不满足 `Ord` 的反对称性要求。a < c 和 c < a 中只允许有一个为真,
/// // 不能两个都真,也不能两个都假。
/// assert_eq!((a < c) as u8 + (c < a) as u8, 2);
/// ```
///
/// [`PartialOrd`] 的文档包含更多例子,例如 [`PartialOrd`] 与 [`PartialEq`]
/// 彼此矛盾就是错误的。
///
/// [`cmp`]: Ord::cmp
#[doc(alias = "<")]
#[doc(alias = ">")]
#[doc(alias = "<=")]
#[doc(alias = ">=")]
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_diagnostic_item = "Ord"]
#[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
pub const trait Ord: [const] Eq + [const] PartialOrd<Self> + PointeeSized {
    /// 本方法返回 `self` 与 `other` 之间的 [`Ordering`]。
    ///
    /// 按惯例,如果表达式 `self <运算符> other` 为真,那么 `self.cmp(&other)`
    /// 返回与该运算符相匹配的次序。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::cmp::Ordering;
    ///
    /// assert_eq!(5.cmp(&10), Ordering::Less);
    /// assert_eq!(10.cmp(&5), Ordering::Greater);
    /// assert_eq!(5.cmp(&5), Ordering::Equal);
    /// ```
    #[must_use]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_diagnostic_item = "ord_cmp_method"]
    fn cmp(&self, other: &Self) -> Ordering;

    /// 比较并返回两个值中的较大者。
    ///
    /// 如果比较判定二者相等,则返回第二个参数。
    ///
    /// # 示例
    ///
    /// ```
    /// assert_eq!(1.max(2), 2);
    /// assert_eq!(2.max(2), 2);
    /// ```
    /// ```
    /// use std::cmp::Ordering;
    ///
    /// #[derive(Eq)]
    /// struct Equal(&'static str);
    ///
    /// impl PartialEq for Equal {
    ///     fn eq(&self, other: &Self) -> bool { true }
    /// }
    /// impl PartialOrd for Equal {
    ///     fn partial_cmp(&self, other: &Self) -> Option<Ordering> { Some(Ordering::Equal) }
    /// }
    /// impl Ord for Equal {
    ///     fn cmp(&self, other: &Self) -> Ordering { Ordering::Equal }
    /// }
    ///
    /// assert_eq!(Equal("self").max(Equal("other")).0, "other");
    /// ```
    #[stable(feature = "ord_max_min", since = "1.21.0")]
    #[inline]
    #[must_use]
    #[rustc_diagnostic_item = "cmp_ord_max"]
    fn max(self, other: Self) -> Self
    where
        Self: Sized + [const] Destruct,
    {
        if other < self { self } else { other }
    }

    /// 比较并返回两个值中的较小者。
    ///
    /// 如果比较判定二者相等,则返回第一个参数。
    ///
    /// # 示例
    ///
    /// ```
    /// assert_eq!(1.min(2), 1);
    /// assert_eq!(2.min(2), 2);
    /// ```
    /// ```
    /// use std::cmp::Ordering;
    ///
    /// #[derive(Eq)]
    /// struct Equal(&'static str);
    ///
    /// impl PartialEq for Equal {
    ///     fn eq(&self, other: &Self) -> bool { true }
    /// }
    /// impl PartialOrd for Equal {
    ///     fn partial_cmp(&self, other: &Self) -> Option<Ordering> { Some(Ordering::Equal) }
    /// }
    /// impl Ord for Equal {
    ///     fn cmp(&self, other: &Self) -> Ordering { Ordering::Equal }
    /// }
    ///
    /// assert_eq!(Equal("self").min(Equal("other")).0, "self");
    /// ```
    #[stable(feature = "ord_max_min", since = "1.21.0")]
    #[inline]
    #[must_use]
    #[rustc_diagnostic_item = "cmp_ord_min"]
    fn min(self, other: Self) -> Self
    where
        Self: Sized + [const] Destruct,
    {
        if other < self { other } else { self }
    }

    /// 把一个值限制(clamp)在某个区间内。
    ///
    /// 如果 `self` 大于 `max`,返回 `max`;如果 `self` 小于 `min`,返回 `min`。
    /// 否则返回 `self`。
    ///
    /// # Panics
    ///
    /// 如果 `min > max`,则会 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// assert_eq!((-3).clamp(-2, 1), -2);
    /// assert_eq!(0.clamp(-2, 1), 0);
    /// assert_eq!(2.clamp(-2, 1), 1);
    /// ```
    #[must_use]
    #[inline]
    #[stable(feature = "clamp", since = "1.50.0")]
    fn clamp(self, min: Self, max: Self) -> Self
    where
        Self: Sized + [const] Destruct,
    {
        assert!(min <= max);
        if self < min {
            min
        } else if self > max {
            max
        } else {
            self
        }
    }
}

/// 生成 [`Ord`] trait 实现的派生宏。
/// 本宏的行为在[此处](Ord#derivable)有详细描述。
#[rustc_builtin_macro]
#[stable(feature = "builtin_macro_prelude", since = "1.38.0")]
#[allow_internal_unstable(core_intrinsics)]
pub macro Ord($item:item) {
    /* 编译器内建 */
}

/// 面向构成[偏序](https://en.wikipedia.org/wiki/Partial_order)的类型的 trait。
///
/// 本 trait 的 `lt`、`le`、`gt` 和 `ge` 方法可以分别通过 `<`、`<=`、`>` 和
/// `>=` 运算符来调用。
///
/// **只有当你打算只实现 `PartialOrd` 而不实现 [`Ord`] 时**,才应当把类型的
/// 比较逻辑放在本 trait 中。否则,比较逻辑应当放在 [`Ord`] 里,而本 trait
/// 则用 `Some(self.cmp(other))` 来实现。
///
/// 本 trait 的各方法之间、以及与 [`PartialEq`] 的方法之间,都必须保持一致。
/// 以下条件必须成立:
///
/// 1. `a == b` 当且仅当 `partial_cmp(a, b) == Some(Equal)`。
/// 2. `a < b` 当且仅当 `partial_cmp(a, b) == Some(Less)`
/// 3. `a > b` 当且仅当 `partial_cmp(a, b) == Some(Greater)`
/// 4. `a <= b` 当且仅当 `a < b || a == b`
/// 5. `a >= b` 当且仅当 `a > b || a == b`
/// 6. `a != b` 当且仅当 `!(a == b)`。
///
/// 上面的条件 2–5 由默认实现保证。条件 6 已由 [`PartialEq`] 保证。
///
/// 如果 `Self` 和 `Rhs` 还实现了 [`Ord`],那么它也必须与 `partial_cmp` 保持
/// 一致(确切要求见该 trait 的文档)。通过派生其中一些 trait、手动实现另一些,
/// 很容易在不经意间让它们彼此矛盾。
///
/// 比较关系必须满足以下条件(对所有 `A`、`B`、`C` 类型的 `a`、`b`、`c`):
///
/// - **传递性(Transitivity)**:如果 `A: PartialOrd<B>`、`B: PartialOrd<C>`
///   且 `A: PartialOrd<C>`,那么 `a < b` 且 `b < c` 蕴含 `a < c`。`==` 和 `>`
///   也必须如此。这对更长的链条同样必须成立,例如当 `A: PartialOrd<B>`、
///   `B: PartialOrd<C>`、`C: PartialOrd<D>` 和 `A: PartialOrd<D>` 同时存在时。
/// - **对偶性(Duality)**:如果 `A: PartialOrd<B>` 且 `B: PartialOrd<A>`,
///   那么 `a < b` 当且仅当 `b > a`。
///
/// 注意,并不强制要求 `B: PartialOrd<A>`(对偶)和 `A: PartialOrd<C>`(传递)
/// 这些 impl 必须存在,但只要它们存在,这些要求就适用。
///
/// 违反这些要求属于逻辑错误。逻辑错误所导致的行为是未指定的,但 trait 的
/// 使用者必须确保此类逻辑错误 *不会* 导致未定义行为。这意味着 `unsafe` 代码
/// **不得** 依赖这些方法的正确性。
///
/// ## 跨 crate 的考量
///
/// 当一个 crate 为另一个 crate 的类型实现 `PartialOrd`(即为了允许把自己的
/// 某个类型与标准库的类型相比较)时,要维持上述要求会变得棘手。建议是:
/// 永远不要为外来类型实现本 trait。换句话说,这样的 crate 应当写
/// `impl PartialOrd<ForeignType> for LocalType`,而 *不应* 写
/// `impl PartialOrd<LocalType> for ForeignType`。
///
/// 这样可以避免那种横跨 crate 边界、纵横交错的传递链问题:对于所有本地
/// 类型 `T`,你可以假定没有别的 crate 会添加允许比较 `T < U` 的 impl。
/// 换句话说,如果其他 crate 添加 impl 来构建更长的传递链
/// `U1 < ... < T < V1 < ...`,那么出现在 `T` 右侧的所有类型都必须是定义 `T`
/// 的那个 crate 早已知道的类型。这就排除了这样一种传递链:下游 crate 可以
/// 添加新的 impl,以违反传递性的方式把外来类型的比较“拼接”起来。
///
/// 不存在这样的外来 impl,也避免了前向兼容性问题——否则一个 crate 增加更多
/// `PartialOrd` 实现就可能导致下游 crate 构建失败。
///
/// ## 推论
///
/// 由上述要求可得出以下推论:
///
/// - `<` 与 `>` 的非自反性(irreflexivity):`!(a < a)`、`!(a > a)`
/// - `>` 的传递性:如果 `a > b` 且 `b > c`,那么 `a > c`
/// - `partial_cmp` 的对偶性:`partial_cmp(a, b) == partial_cmp(b, a).map(Ordering::reverse)`
///
/// ## 严格与非严格偏序
///
/// `<` 与 `>` 运算符的行为遵循一个 *严格* 偏序。然而,`<=` 与 `>=` **并不**
/// 遵循一个 *非严格* 偏序。这是因为,从数学上讲,一个非严格偏序要求满足
/// 自反性,即对每一个 `a` 都需要 `a <= a` 为真。对于实现 `PartialOrd` 的
/// 类型而言,这并不总是成立,例如:
///
/// ```
/// let a = f64::NAN;
/// assert_eq!(a <= a, false);
/// ```
///
/// ## 可派生(Derivable)
///
/// 本 trait 可以配合 `#[derive]` 使用。
///
/// 在结构体上 `derive` 时,它会基于结构体成员从上到下的声明顺序,产生一个
/// [字典序](https://en.wikipedia.org/wiki/Lexicographic_order)的次序。
///
/// 在枚举上 `derive` 时,变体首先按其判别值排序;其次,再按其字段排序。
/// 默认情况下,靠上的变体判别值最小,靠下的变体判别值最大。这里有一个例子:
///
/// ```
/// #[derive(PartialEq, PartialOrd)]
/// enum E {
///     Top,
///     Bottom,
/// }
///
/// assert!(E::Top < E::Bottom);
/// ```
///
/// 然而,手动设置判别值可以覆盖这一默认行为:
///
/// ```
/// #[derive(PartialEq, PartialOrd)]
/// enum E {
///     Top = 2,
///     Bottom = 1,
/// }
///
/// assert!(E::Bottom < E::Top);
/// ```
///
/// ## 如何实现 `PartialOrd`?
///
/// `PartialOrd` 只要求实现 [`partial_cmp`] 方法,其余方法都由默认实现生成。
///
/// 不过,对于那些不具备全序的类型,仍然可以分别单独实现其余方法。例如,
/// 对于浮点数,`NaN < 0 == false` 且 `NaN >= 0 == false`(参见
/// IEEE 754-2008 第 5.11 节)。
///
/// `PartialOrd` 要求你的类型是 [`PartialEq`]。
///
/// 如果你的类型是 [`Ord`],你可以借助 [`cmp`] 来实现 [`partial_cmp`]:
///
/// ```
/// use std::cmp::Ordering;
///
/// struct Person {
///     id: u32,
///     name: String,
///     height: u32,
/// }
///
/// impl PartialOrd for Person {
///     fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
///         Some(self.cmp(other))
///     }
/// }
///
/// impl Ord for Person {
///     fn cmp(&self, other: &Self) -> Ordering {
///         self.height.cmp(&other.height)
///     }
/// }
///
/// impl PartialEq for Person {
///     fn eq(&self, other: &Self) -> bool {
///         self.height == other.height
///     }
/// }
///
/// impl Eq for Person {}
/// ```
///
/// 你也许还会发现,对你类型的字段使用 [`partial_cmp`] 很有用。下面是一个例子,
/// `Person` 类型有一个浮点的 `height` 字段,且它是排序时唯一用到的字段:
///
/// ```
/// use std::cmp::Ordering;
///
/// struct Person {
///     id: u32,
///     name: String,
///     height: f64,
/// }
///
/// impl PartialOrd for Person {
///     fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
///         self.height.partial_cmp(&other.height)
///     }
/// }
///
/// impl PartialEq for Person {
///     fn eq(&self, other: &Self) -> bool {
///         self.height == other.height
///     }
/// }
/// ```
///
/// ## 错误的 `PartialOrd` 实现示例
///
/// ```
/// use std::cmp::Ordering;
///
/// #[derive(PartialEq, Debug)]
/// struct Character {
///     health: u32,
///     experience: u32,
/// }
///
/// impl PartialOrd for Character {
///     fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
///         Some(self.health.cmp(&other.health))
///     }
/// }
///
/// let a = Character {
///     health: 10,
///     experience: 5,
/// };
/// let b = Character {
///     health: 10,
///     experience: 77,
/// };
///
/// // 错误:`PartialEq` 与 `PartialOrd` 彼此矛盾。
///
/// assert_eq!(a.partial_cmp(&b).unwrap(), Ordering::Equal); // 按 `PartialOrd`,a == b。
/// assert_ne!(a, b); // 按 `PartialEq`,a != b。
/// ```
///
/// # 示例
///
/// ```
/// let x: u32 = 0;
/// let y: u32 = 1;
///
/// assert_eq!(x < y, true);
/// assert_eq!(x.lt(&y), true);
/// ```
///
/// [`partial_cmp`]: PartialOrd::partial_cmp
/// [`cmp`]: Ord::cmp
#[lang = "partial_ord"]
#[stable(feature = "rust1", since = "1.0.0")]
#[doc(alias = ">")]
#[doc(alias = "<")]
#[doc(alias = "<=")]
#[doc(alias = ">=")]
#[rustc_on_unimplemented(
    message = "can't compare `{Self}` with `{Rhs}`",
    label = "no implementation for `{Self} < {Rhs}` and `{Self} > {Rhs}`",
    append_const_msg
)]
#[rustc_diagnostic_item = "PartialOrd"]
#[allow(multiple_supertrait_upcastable)] // FIXME(sized_hierarchy): remove this
#[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
pub const trait PartialOrd<Rhs: PointeeSized = Self>:
    [const] PartialEq<Rhs> + PointeeSized
{
    /// 本方法返回 `self` 与 `other` 两个值之间的次序(如果存在的话)。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::cmp::Ordering;
    ///
    /// let result = 1.0.partial_cmp(&2.0);
    /// assert_eq!(result, Some(Ordering::Less));
    ///
    /// let result = 1.0.partial_cmp(&1.0);
    /// assert_eq!(result, Some(Ordering::Equal));
    ///
    /// let result = 2.0.partial_cmp(&1.0);
    /// assert_eq!(result, Some(Ordering::Greater));
    /// ```
    ///
    /// 当无法比较时:
    ///
    /// ```
    /// let result = f64::NAN.partial_cmp(&1.0);
    /// assert_eq!(result, None);
    /// ```
    #[must_use]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_diagnostic_item = "cmp_partialord_cmp"]
    fn partial_cmp(&self, other: &Rhs) -> Option<Ordering>;

    /// 检验(`self` 与 `other` 的)小于关系,被 `<` 运算符使用。
    ///
    /// # 示例
    ///
    /// ```
    /// assert_eq!(1.0 < 1.0, false);
    /// assert_eq!(1.0 < 2.0, true);
    /// assert_eq!(2.0 < 1.0, false);
    /// ```
    #[inline]
    #[must_use]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_diagnostic_item = "cmp_partialord_lt"]
    fn lt(&self, other: &Rhs) -> bool {
        self.partial_cmp(other).is_some_and(Ordering::is_lt)
    }

    /// 检验(`self` 与 `other` 的)小于等于关系,被 `<=` 运算符使用。
    ///
    /// # 示例
    ///
    /// ```
    /// assert_eq!(1.0 <= 1.0, true);
    /// assert_eq!(1.0 <= 2.0, true);
    /// assert_eq!(2.0 <= 1.0, false);
    /// ```
    #[inline]
    #[must_use]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_diagnostic_item = "cmp_partialord_le"]
    fn le(&self, other: &Rhs) -> bool {
        self.partial_cmp(other).is_some_and(Ordering::is_le)
    }

    /// 检验(`self` 与 `other` 的)大于关系,被 `>` 运算符使用。
    ///
    /// # 示例
    ///
    /// ```
    /// assert_eq!(1.0 > 1.0, false);
    /// assert_eq!(1.0 > 2.0, false);
    /// assert_eq!(2.0 > 1.0, true);
    /// ```
    #[inline]
    #[must_use]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_diagnostic_item = "cmp_partialord_gt"]
    fn gt(&self, other: &Rhs) -> bool {
        self.partial_cmp(other).is_some_and(Ordering::is_gt)
    }

    /// 检验(`self` 与 `other` 的)大于等于关系,被 `>=` 运算符使用。
    ///
    /// # 示例
    ///
    /// ```
    /// assert_eq!(1.0 >= 1.0, true);
    /// assert_eq!(1.0 >= 2.0, false);
    /// assert_eq!(2.0 >= 1.0, true);
    /// ```
    #[inline]
    #[must_use]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_diagnostic_item = "cmp_partialord_ge"]
    fn ge(&self, other: &Rhs) -> bool {
        self.partial_cmp(other).is_some_and(Ordering::is_ge)
    }

    /// 如果 `self == other`,返回 `ControlFlow::Continue(())`。
    /// 否则返回 `ControlFlow::Break(self < other)`。
    ///
    /// 这在实现字典序的 `PartialOrd::lt` 时把一连串调用串联起来很有用:它让
    /// 那些(像原始类型那样)能廉价地分别检查 `==` 和 `<` 的类型直接这么做,
    /// 而无需先算出(再优化掉)三路的 `Ordering` 结果。
    #[inline]
    // 为改善元组的行为而添加;不一定走稳定化流程。
    #[unstable(feature = "partial_ord_chaining_methods", issue = "none")]
    #[doc(hidden)]
    fn __chaining_lt(&self, other: &Rhs) -> ControlFlow<bool> {
        default_chaining_impl(self, other, Ordering::is_lt)
    }

    /// 与 `__chaining_lt` 相同,只是针对 `<=` 而非 `<`。
    #[inline]
    #[unstable(feature = "partial_ord_chaining_methods", issue = "none")]
    #[doc(hidden)]
    fn __chaining_le(&self, other: &Rhs) -> ControlFlow<bool> {
        default_chaining_impl(self, other, Ordering::is_le)
    }

    /// 与 `__chaining_lt` 相同,只是针对 `>` 而非 `<`。
    #[inline]
    #[unstable(feature = "partial_ord_chaining_methods", issue = "none")]
    #[doc(hidden)]
    fn __chaining_gt(&self, other: &Rhs) -> ControlFlow<bool> {
        default_chaining_impl(self, other, Ordering::is_gt)
    }

    /// 与 `__chaining_lt` 相同,只是针对 `>=` 而非 `<`。
    #[inline]
    #[unstable(feature = "partial_ord_chaining_methods", issue = "none")]
    #[doc(hidden)]
    fn __chaining_ge(&self, other: &Rhs) -> ControlFlow<bool> {
        default_chaining_impl(self, other, Ordering::is_ge)
    }
}

#[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
const fn default_chaining_impl<T, U>(
    lhs: &T,
    rhs: &U,
    p: impl [const] FnOnce(Ordering) -> bool + [const] Destruct,
) -> ControlFlow<bool>
where
    T: [const] PartialOrd<U> + PointeeSized,
    U: PointeeSized,
{
    // 重要的是这里只调用一次 `partial_cmp`,而不是先调用 `eq`、再调用某个
    // 关系运算符。比如,我们不希望对一个 `String` 先 `bcmp` 再 `memcp`,
    // 对其他数据结构也是如此(#108157)。
    match <T as PartialOrd<U>>::partial_cmp(lhs, rhs) {
        Some(Equal) => ControlFlow::Continue(()),
        Some(c) => ControlFlow::Break(p(c)),
        None => ControlFlow::Break(false),
    }
}

/// 生成 [`PartialOrd`] trait 实现的派生宏。
/// 本宏的行为在[此处](PartialOrd#derivable)有详细描述。
#[rustc_builtin_macro]
#[stable(feature = "builtin_macro_prelude", since = "1.38.0")]
#[allow_internal_unstable(core_intrinsics)]
pub macro PartialOrd($item:item) {
    /* 编译器内建 */
}

/// 比较并返回两个值中的较小者。
///
/// 如果比较判定二者相等,则返回第一个参数。
///
/// 内部使用 [`Ord::min`] 的别名。
///
/// # 示例
///
/// ```
/// use std::cmp;
///
/// assert_eq!(cmp::min(1, 2), 1);
/// assert_eq!(cmp::min(2, 2), 2);
/// ```
/// ```
/// use std::cmp::{self, Ordering};
///
/// #[derive(Eq)]
/// struct Equal(&'static str);
///
/// impl PartialEq for Equal {
///     fn eq(&self, other: &Self) -> bool { true }
/// }
/// impl PartialOrd for Equal {
///     fn partial_cmp(&self, other: &Self) -> Option<Ordering> { Some(Ordering::Equal) }
/// }
/// impl Ord for Equal {
///     fn cmp(&self, other: &Self) -> Ordering { Ordering::Equal }
/// }
///
/// assert_eq!(cmp::min(Equal("v1"), Equal("v2")).0, "v1");
/// ```
#[inline]
#[must_use]
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_diagnostic_item = "cmp_min"]
#[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
pub const fn min<T: [const] Ord + [const] Destruct>(v1: T, v2: T) -> T {
    v1.min(v2)
}

/// 依照指定的比较函数,返回两个值中的较小者。
///
/// 如果比较判定二者相等,则返回第一个参数。
///
/// 调用 `compare` 函数时参数顺序保持不变,即 `v1` 总是作为第一个参数传入,
/// `v2` 作为第二个。
///
/// # 示例
///
/// ```
/// use std::cmp;
///
/// let abs_cmp = |x: &i32, y: &i32| x.abs().cmp(&y.abs());
///
/// let result = cmp::min_by(2, -1, abs_cmp);
/// assert_eq!(result, -1);
///
/// let result = cmp::min_by(2, -3, abs_cmp);
/// assert_eq!(result, 2);
///
/// let result = cmp::min_by(1, -1, abs_cmp);
/// assert_eq!(result, 1);
/// ```
#[inline]
#[must_use]
#[stable(feature = "cmp_min_max_by", since = "1.53.0")]
#[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
pub const fn min_by<T: [const] Destruct, F: [const] FnOnce(&T, &T) -> Ordering>(
    v1: T,
    v2: T,
    compare: F,
) -> T {
    if compare(&v1, &v2).is_le() { v1 } else { v2 }
}

/// 返回使指定函数取得最小值的那个元素。
///
/// 如果比较判定二者相等,则返回第一个参数。
///
/// # 示例
///
/// ```
/// use std::cmp;
///
/// let result = cmp::min_by_key(2, -1, |x: &i32| x.abs());
/// assert_eq!(result, -1);
///
/// let result = cmp::min_by_key(2, -3, |x: &i32| x.abs());
/// assert_eq!(result, 2);
///
/// let result = cmp::min_by_key(1, -1, |x: &i32| x.abs());
/// assert_eq!(result, 1);
/// ```
#[inline]
#[must_use]
#[stable(feature = "cmp_min_max_by", since = "1.53.0")]
#[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
pub const fn min_by_key<T, F, K>(v1: T, v2: T, mut f: F) -> T
where
    T: [const] Destruct,
    F: [const] FnMut(&T) -> K + [const] Destruct,
    K: [const] Ord + [const] Destruct,
{
    if f(&v2) < f(&v1) { v2 } else { v1 }
}

/// 比较并返回两个值中的较大者。
///
/// 如果比较判定二者相等,则返回第二个参数。
///
/// 内部使用 [`Ord::max`] 的别名。
///
/// # 示例
///
/// ```
/// use std::cmp;
///
/// assert_eq!(cmp::max(1, 2), 2);
/// assert_eq!(cmp::max(2, 2), 2);
/// ```
/// ```
/// use std::cmp::{self, Ordering};
///
/// #[derive(Eq)]
/// struct Equal(&'static str);
///
/// impl PartialEq for Equal {
///     fn eq(&self, other: &Self) -> bool { true }
/// }
/// impl PartialOrd for Equal {
///     fn partial_cmp(&self, other: &Self) -> Option<Ordering> { Some(Ordering::Equal) }
/// }
/// impl Ord for Equal {
///     fn cmp(&self, other: &Self) -> Ordering { Ordering::Equal }
/// }
///
/// assert_eq!(cmp::max(Equal("v1"), Equal("v2")).0, "v2");
/// ```
#[inline]
#[must_use]
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_diagnostic_item = "cmp_max"]
#[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
pub const fn max<T: [const] Ord + [const] Destruct>(v1: T, v2: T) -> T {
    v1.max(v2)
}

/// 依照指定的比较函数,返回两个值中的较大者。
///
/// 如果比较判定二者相等,则返回第二个参数。
///
/// 调用 `compare` 函数时参数顺序保持不变,即 `v1` 总是作为第一个参数传入,
/// `v2` 作为第二个。
///
/// # 示例
///
/// ```
/// use std::cmp;
///
/// let abs_cmp = |x: &i32, y: &i32| x.abs().cmp(&y.abs());
///
/// let result = cmp::max_by(3, -2, abs_cmp) ;
/// assert_eq!(result, 3);
///
/// let result = cmp::max_by(1, -2, abs_cmp);
/// assert_eq!(result, -2);
///
/// let result = cmp::max_by(1, -1, abs_cmp);
/// assert_eq!(result, -1);
/// ```
#[inline]
#[must_use]
#[stable(feature = "cmp_min_max_by", since = "1.53.0")]
#[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
pub const fn max_by<T: [const] Destruct, F: [const] FnOnce(&T, &T) -> Ordering>(
    v1: T,
    v2: T,
    compare: F,
) -> T {
    if compare(&v1, &v2).is_gt() { v1 } else { v2 }
}

/// 返回使指定函数取得最大值的那个元素。
///
/// 如果比较判定二者相等,则返回第二个参数。
///
/// # 示例
///
/// ```
/// use std::cmp;
///
/// let result = cmp::max_by_key(3, -2, |x: &i32| x.abs());
/// assert_eq!(result, 3);
///
/// let result = cmp::max_by_key(1, -2, |x: &i32| x.abs());
/// assert_eq!(result, -2);
///
/// let result = cmp::max_by_key(1, -1, |x: &i32| x.abs());
/// assert_eq!(result, -1);
/// ```
#[inline]
#[must_use]
#[stable(feature = "cmp_min_max_by", since = "1.53.0")]
#[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
pub const fn max_by_key<T, F, K>(v1: T, v2: T, mut f: F) -> T
where
    T: [const] Destruct,
    F: [const] FnMut(&T) -> K + [const] Destruct,
    K: [const] Ord + [const] Destruct,
{
    if f(&v2) < f(&v1) { v1 } else { v2 }
}

/// 比较并对两个值排序,返回最小值和最大值。
///
/// 如果比较判定二者相等,则返回 `[v1, v2]`。
///
/// # 示例
///
/// ```
/// #![feature(cmp_minmax)]
/// use std::cmp;
///
/// assert_eq!(cmp::minmax(1, 2), [1, 2]);
/// assert_eq!(cmp::minmax(2, 1), [1, 2]);
///
/// // 你可以用数组模式来解构结果
/// let [min, max] = cmp::minmax(42, 17);
/// assert_eq!(min, 17);
/// assert_eq!(max, 42);
/// ```
/// ```
/// #![feature(cmp_minmax)]
/// use std::cmp::{self, Ordering};
///
/// #[derive(Eq)]
/// struct Equal(&'static str);
///
/// impl PartialEq for Equal {
///     fn eq(&self, other: &Self) -> bool { true }
/// }
/// impl PartialOrd for Equal {
///     fn partial_cmp(&self, other: &Self) -> Option<Ordering> { Some(Ordering::Equal) }
/// }
/// impl Ord for Equal {
///     fn cmp(&self, other: &Self) -> Ordering { Ordering::Equal }
/// }
///
/// assert_eq!(cmp::minmax(Equal("v1"), Equal("v2")).map(|v| v.0), ["v1", "v2"]);
/// ```
#[inline]
#[must_use]
#[unstable(feature = "cmp_minmax", issue = "115939")]
#[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
pub const fn minmax<T>(v1: T, v2: T) -> [T; 2]
where
    T: [const] Ord,
{
    if v2 < v1 { [v2, v1] } else { [v1, v2] }
}

/// 依照指定的比较函数,返回最小值和最大值。
///
/// 如果比较判定二者相等,则返回 `[v1, v2]`。
///
/// 调用 `compare` 函数时参数顺序保持不变,即 `v1` 总是作为第一个参数传入,
/// `v2` 作为第二个。
///
/// # 示例
///
/// ```
/// #![feature(cmp_minmax)]
/// use std::cmp;
///
/// let abs_cmp = |x: &i32, y: &i32| x.abs().cmp(&y.abs());
///
/// assert_eq!(cmp::minmax_by(-2, 1, abs_cmp), [1, -2]);
/// assert_eq!(cmp::minmax_by(-1, 2, abs_cmp), [-1, 2]);
/// assert_eq!(cmp::minmax_by(-2, 2, abs_cmp), [-2, 2]);
///
/// // 你可以用数组模式来解构结果
/// let [min, max] = cmp::minmax_by(-42, 17, abs_cmp);
/// assert_eq!(min, 17);
/// assert_eq!(max, -42);
/// ```
#[inline]
#[must_use]
#[unstable(feature = "cmp_minmax", issue = "115939")]
#[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
pub const fn minmax_by<T, F>(v1: T, v2: T, compare: F) -> [T; 2]
where
    F: [const] FnOnce(&T, &T) -> Ordering,
{
    if compare(&v1, &v2).is_le() { [v1, v2] } else { [v2, v1] }
}

/// 依照指定的键函数,返回最小值和最大值。
///
/// 如果比较判定二者相等,则返回 `[v1, v2]`。
///
/// # 示例
///
/// ```
/// #![feature(cmp_minmax)]
/// use std::cmp;
///
/// assert_eq!(cmp::minmax_by_key(-2, 1, |x: &i32| x.abs()), [1, -2]);
/// assert_eq!(cmp::minmax_by_key(-2, 2, |x: &i32| x.abs()), [-2, 2]);
///
/// // 你可以用数组模式来解构结果
/// let [min, max] = cmp::minmax_by_key(-42, 17, |x: &i32| x.abs());
/// assert_eq!(min, 17);
/// assert_eq!(max, -42);
/// ```
#[inline]
#[must_use]
#[unstable(feature = "cmp_minmax", issue = "115939")]
#[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
pub const fn minmax_by_key<T, F, K>(v1: T, v2: T, mut f: F) -> [T; 2]
where
    F: [const] FnMut(&T) -> K + [const] Destruct,
    K: [const] Ord + [const] Destruct,
{
    if f(&v2) < f(&v1) { [v2, v1] } else { [v1, v2] }
}

// 为原始类型实现 PartialEq、Eq、PartialOrd 和 Ord
mod impls {
    use crate::cmp::Ordering::{self, Equal, Greater, Less};
    use crate::hint::unreachable_unchecked;
    use crate::marker::PointeeSized;
    use crate::ops::ControlFlow::{self, Break, Continue};
    use crate::panic::const_assert;

    macro_rules! partial_eq_impl {
        ($($t:ty)*) => ($(
            #[stable(feature = "rust1", since = "1.0.0")]
            #[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
            impl const PartialEq for $t {
                #[inline]
                fn eq(&self, other: &Self) -> bool { *self == *other }
                #[inline]
                fn ne(&self, other: &Self) -> bool { *self != *other }
            }
        )*)
    }

    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
    impl const PartialEq for () {
        #[inline]
        fn eq(&self, _other: &()) -> bool {
            true
        }
        #[inline]
        fn ne(&self, _other: &()) -> bool {
            false
        }
    }

    partial_eq_impl! {
        bool char usize u8 u16 u32 u64 u128 isize i8 i16 i32 i64 i128 f16 f32 f64 f128
    }

    macro_rules! eq_impl {
        ($($t:ty)*) => ($(
            #[stable(feature = "rust1", since = "1.0.0")]
            #[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
            impl const Eq for $t {}
        )*)
    }

    eq_impl! { () bool char usize u8 u16 u32 u64 u128 isize i8 i16 i32 i64 i128 }

    #[rustfmt::skip]
    macro_rules! partial_ord_methods_primitive_impl {
        () => {
            #[inline(always)]
            fn lt(&self, other: &Self) -> bool { *self <  *other }
            #[inline(always)]
            fn le(&self, other: &Self) -> bool { *self <= *other }
            #[inline(always)]
            fn gt(&self, other: &Self) -> bool { *self >  *other }
            #[inline(always)]
            fn ge(&self, other: &Self) -> bool { *self >= *other }

            // 对 `Ord` 和 `PartialOrd` 类型而言这些实现都是一样的,因为只要
            // 其中任何一个是 NAN,`==` 检验就会失败,于是我们落入 `Break`
            // 分支,比较会正确地返回 `false`。

            #[inline]
            fn __chaining_lt(&self, other: &Self) -> ControlFlow<bool> {
                let (lhs, rhs) = (*self, *other);
                if lhs == rhs { Continue(()) } else { Break(lhs < rhs) }
            }
            #[inline]
            fn __chaining_le(&self, other: &Self) -> ControlFlow<bool> {
                let (lhs, rhs) = (*self, *other);
                if lhs == rhs { Continue(()) } else { Break(lhs <= rhs) }
            }
            #[inline]
            fn __chaining_gt(&self, other: &Self) -> ControlFlow<bool> {
                let (lhs, rhs) = (*self, *other);
                if lhs == rhs { Continue(()) } else { Break(lhs > rhs) }
            }
            #[inline]
            fn __chaining_ge(&self, other: &Self) -> ControlFlow<bool> {
                let (lhs, rhs) = (*self, *other);
                if lhs == rhs { Continue(()) } else { Break(lhs >= rhs) }
            }
        };
    }

    macro_rules! partial_ord_impl {
        ($($t:ty)*) => ($(
            #[stable(feature = "rust1", since = "1.0.0")]
            #[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
            impl const PartialOrd for $t {
                #[inline]
                fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
                    match (*self <= *other, *self >= *other) {
                        (false, false) => None,
                        (false, true) => Some(Greater),
                        (true, false) => Some(Less),
                        (true, true) => Some(Equal),
                    }
                }

                partial_ord_methods_primitive_impl!();
            }
        )*)
    }

    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
    impl const PartialOrd for () {
        #[inline]
        fn partial_cmp(&self, _: &()) -> Option<Ordering> {
            Some(Equal)
        }
    }

    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
    impl const PartialOrd for bool {
        #[inline]
        fn partial_cmp(&self, other: &bool) -> Option<Ordering> {
            Some(self.cmp(other))
        }

        partial_ord_methods_primitive_impl!();
    }

    partial_ord_impl! { f16 f32 f64 f128 }

    macro_rules! ord_impl {
        ($($t:ty)*) => ($(
            #[stable(feature = "rust1", since = "1.0.0")]
            #[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
            impl const PartialOrd for $t {
                #[inline]
                fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
                    Some(crate::intrinsics::three_way_compare(*self, *other))
                }

                partial_ord_methods_primitive_impl!();
            }

            #[stable(feature = "rust1", since = "1.0.0")]
            #[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
            impl const Ord for $t {
                #[inline]
                fn cmp(&self, other: &Self) -> Ordering {
                    crate::intrinsics::three_way_compare(*self, *other)
                }

                #[inline]
                #[track_caller]
                fn clamp(self, min: Self, max: Self) -> Self
                {
                    const_assert!(
                        min <= max,
                        "min > max",
                        "min > max. min = {min:?}, max = {max:?}",
                        min: $t,
                        max: $t,
                    );
                    if self < min {
                        min
                    } else if self > max {
                        max
                    } else {
                        self
                    }
                }
            }
        )*)
    }

    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
    impl const Ord for () {
        #[inline]
        fn cmp(&self, _other: &()) -> Ordering {
            Equal
        }
    }

    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
    impl const Ord for bool {
        #[inline]
        fn cmp(&self, other: &bool) -> Ordering {
            // 转成 i8 再把差值转换为 Ordering,能生成更优的汇编。
            // 更多信息参见 <https://github.com/rust-lang/rust/issues/66780>。
            match (*self as i8) - (*other as i8) {
                -1 => Less,
                0 => Equal,
                1 => Greater,
                // SAFETY:bool 转 i8 得到 0 或 1,所以差值不可能是别的值
                _ => unsafe { unreachable_unchecked() },
            }
        }

        #[inline]
        fn min(self, other: bool) -> bool {
            self & other
        }

        #[inline]
        fn max(self, other: bool) -> bool {
            self | other
        }

        #[inline]
        fn clamp(self, min: bool, max: bool) -> bool {
            assert!(min <= max);
            self.max(min).min(max)
        }
    }

    ord_impl! { char usize u8 u16 u32 u64 u128 isize i8 i16 i32 i64 i128 }

    #[unstable(feature = "never_type", issue = "35121")]
    #[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
    impl const PartialEq for ! {
        #[inline]
        fn eq(&self, _: &!) -> bool {
            *self
        }
    }

    #[unstable(feature = "never_type", issue = "35121")]
    #[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
    impl const Eq for ! {}

    #[unstable(feature = "never_type", issue = "35121")]
    #[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
    impl const PartialOrd for ! {
        #[inline]
        fn partial_cmp(&self, _: &!) -> Option<Ordering> {
            *self
        }
    }

    #[unstable(feature = "never_type", issue = "35121")]
    #[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
    impl const Ord for ! {
        #[inline]
        fn cmp(&self, _: &!) -> Ordering {
            *self
        }
    }

    // & 引用

    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
    impl<A: PointeeSized, B: PointeeSized> const PartialEq<&B> for &A
    where
        A: [const] PartialEq<B>,
    {
        #[inline]
        fn eq(&self, other: &&B) -> bool {
            PartialEq::eq(*self, *other)
        }
        #[inline]
        fn ne(&self, other: &&B) -> bool {
            PartialEq::ne(*self, *other)
        }
    }
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
    impl<A: PointeeSized, B: PointeeSized> const PartialOrd<&B> for &A
    where
        A: [const] PartialOrd<B>,
    {
        #[inline]
        fn partial_cmp(&self, other: &&B) -> Option<Ordering> {
            PartialOrd::partial_cmp(*self, *other)
        }
        #[inline]
        fn lt(&self, other: &&B) -> bool {
            PartialOrd::lt(*self, *other)
        }
        #[inline]
        fn le(&self, other: &&B) -> bool {
            PartialOrd::le(*self, *other)
        }
        #[inline]
        fn gt(&self, other: &&B) -> bool {
            PartialOrd::gt(*self, *other)
        }
        #[inline]
        fn ge(&self, other: &&B) -> bool {
            PartialOrd::ge(*self, *other)
        }
        #[inline]
        fn __chaining_lt(&self, other: &&B) -> ControlFlow<bool> {
            PartialOrd::__chaining_lt(*self, *other)
        }
        #[inline]
        fn __chaining_le(&self, other: &&B) -> ControlFlow<bool> {
            PartialOrd::__chaining_le(*self, *other)
        }
        #[inline]
        fn __chaining_gt(&self, other: &&B) -> ControlFlow<bool> {
            PartialOrd::__chaining_gt(*self, *other)
        }
        #[inline]
        fn __chaining_ge(&self, other: &&B) -> ControlFlow<bool> {
            PartialOrd::__chaining_ge(*self, *other)
        }
    }
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
    impl<A: PointeeSized> const Ord for &A
    where
        A: [const] Ord,
    {
        #[inline]
        fn cmp(&self, other: &Self) -> Ordering {
            Ord::cmp(*self, *other)
        }
    }
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
    impl<A: PointeeSized> const Eq for &A where A: [const] Eq {}

    // &mut 引用

    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
    impl<A: PointeeSized, B: PointeeSized> const PartialEq<&mut B> for &mut A
    where
        A: [const] PartialEq<B>,
    {
        #[inline]
        fn eq(&self, other: &&mut B) -> bool {
            PartialEq::eq(*self, *other)
        }
        #[inline]
        fn ne(&self, other: &&mut B) -> bool {
            PartialEq::ne(*self, *other)
        }
    }
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
    impl<A: PointeeSized, B: PointeeSized> const PartialOrd<&mut B> for &mut A
    where
        A: [const] PartialOrd<B>,
    {
        #[inline]
        fn partial_cmp(&self, other: &&mut B) -> Option<Ordering> {
            PartialOrd::partial_cmp(*self, *other)
        }
        #[inline]
        fn lt(&self, other: &&mut B) -> bool {
            PartialOrd::lt(*self, *other)
        }
        #[inline]
        fn le(&self, other: &&mut B) -> bool {
            PartialOrd::le(*self, *other)
        }
        #[inline]
        fn gt(&self, other: &&mut B) -> bool {
            PartialOrd::gt(*self, *other)
        }
        #[inline]
        fn ge(&self, other: &&mut B) -> bool {
            PartialOrd::ge(*self, *other)
        }
        #[inline]
        fn __chaining_lt(&self, other: &&mut B) -> ControlFlow<bool> {
            PartialOrd::__chaining_lt(*self, *other)
        }
        #[inline]
        fn __chaining_le(&self, other: &&mut B) -> ControlFlow<bool> {
            PartialOrd::__chaining_le(*self, *other)
        }
        #[inline]
        fn __chaining_gt(&self, other: &&mut B) -> ControlFlow<bool> {
            PartialOrd::__chaining_gt(*self, *other)
        }
        #[inline]
        fn __chaining_ge(&self, other: &&mut B) -> ControlFlow<bool> {
            PartialOrd::__chaining_ge(*self, *other)
        }
    }
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
    impl<A: PointeeSized> const Ord for &mut A
    where
        A: [const] Ord,
    {
        #[inline]
        fn cmp(&self, other: &Self) -> Ordering {
            Ord::cmp(*self, *other)
        }
    }
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
    impl<A: PointeeSized> const Eq for &mut A where A: [const] Eq {}

    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
    impl<A: PointeeSized, B: PointeeSized> const PartialEq<&mut B> for &A
    where
        A: [const] PartialEq<B>,
    {
        #[inline]
        fn eq(&self, other: &&mut B) -> bool {
            PartialEq::eq(*self, *other)
        }
        #[inline]
        fn ne(&self, other: &&mut B) -> bool {
            PartialEq::ne(*self, *other)
        }
    }

    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
    impl<A: PointeeSized, B: PointeeSized> const PartialEq<&B> for &mut A
    where
        A: [const] PartialEq<B>,
    {
        #[inline]
        fn eq(&self, other: &&B) -> bool {
            PartialEq::eq(*self, *other)
        }
        #[inline]
        fn ne(&self, other: &&B) -> bool {
            PartialEq::ne(*self, *other)
        }
    }
}
