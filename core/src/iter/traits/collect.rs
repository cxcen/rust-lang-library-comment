use super::TrustedLen;

/// 从 [`Iterator`] 执行转换。
///
/// 为某个类型实现 `FromIterator`，就是定义该类型如何从一个 iterator 中创建出来。
/// 这在各种集合类型上很常见，因为集合本质上通常就是一组按迭代顺序产生的元素。
///
/// 如果想从 iterator 的内容创建集合，通常优先使用 [`Iterator::collect()`]。不过，
/// 当需要显式写出目标容器类型时，[`FromIterator::from_iter()`] 往往比 turbofish
/// 写法（例如 `::<Vec<_>>()`）更容易读。更多用法示例见
/// [`Iterator::collect()`] 的文档。
///
/// 另见: [`IntoIterator`]。
///
/// # 示例
///
/// 基本用法:
///
/// ```
/// let five_fives = std::iter::repeat(5).take(5);
///
/// let v = Vec::from_iter(five_fives);
///
/// assert_eq!(v, vec![5, 5, 5, 5, 5]);
/// ```
///
/// 使用 [`Iterator::collect()`] 隐式调用 `FromIterator`:
///
/// ```
/// let five_fives = std::iter::repeat(5).take(5);
///
/// let v: Vec<i32> = five_fives.collect();
///
/// assert_eq!(v, vec![5, 5, 5, 5, 5]);
/// ```
///
/// 使用 [`FromIterator::from_iter()`]，让目标类型比
/// [`Iterator::collect()`] 的 turbofish 写法更清晰:
///
/// ```
/// use std::collections::VecDeque;
/// let first = (0..10).collect::<VecDeque<i32>>();
/// let second = VecDeque::from_iter(0..10);
///
/// assert_eq!(first, second);
/// ```
///
/// 为自己的类型实现 `FromIterator`:
///
/// ```
/// // 一个示例集合，只是 Vec<T> 的简单包装
/// #[derive(Debug)]
/// struct MyCollection(Vec<i32>);
///
/// // 给它一些方法，便于创建集合并向其中加入元素。
/// impl MyCollection {
///     fn new() -> MyCollection {
///         MyCollection(Vec::new())
///     }
///
///     fn add(&mut self, elem: i32) {
///         self.0.push(elem);
///     }
/// }
///
/// // 接着实现 FromIterator
/// impl FromIterator<i32> for MyCollection {
///     fn from_iter<I: IntoIterator<Item=i32>>(iter: I) -> Self {
///         let mut c = MyCollection::new();
///
///         for i in iter {
///             c.add(i);
///         }
///
///         c
///     }
/// }
///
/// // 现在可以创建一个新的 iterator...
/// let iter = (0..5).into_iter();
///
/// // ... 并从中构造 MyCollection
/// let c = MyCollection::from_iter(iter);
///
/// assert_eq!(c.0, vec![0, 1, 2, 3, 4]);
///
/// // collect 也同样可用！
///
/// let iter = (0..5).into_iter();
/// let c: MyCollection = iter.collect();
///
/// assert_eq!(c.0, vec![0, 1, 2, 3, 4]);
/// ```
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_on_unimplemented(
    on(
        Self = "&[{A}]",
        message = "a slice of type `{Self}` cannot be built since we need to store the elements somewhere",
        label = "try explicitly collecting into a `Vec<{A}>`",
    ),
    on(
        all(A = "{integer}", any(Self = "&[{integral}]",)),
        message = "a slice of type `{Self}` cannot be built since we need to store the elements somewhere",
        label = "try explicitly collecting into a `Vec<{A}>`",
    ),
    on(
        Self = "[{A}]",
        message = "a slice of type `{Self}` cannot be built since `{Self}` has no definite size",
        label = "try explicitly collecting into a `Vec<{A}>`",
    ),
    on(
        all(A = "{integer}", any(Self = "[{integral}]",)),
        message = "a slice of type `{Self}` cannot be built since `{Self}` has no definite size",
        label = "try explicitly collecting into a `Vec<{A}>`",
    ),
    on(
        Self = "[{A}; _]",
        message = "an array of type `{Self}` cannot be built directly from an iterator",
        label = "try collecting into a `Vec<{A}>`, then using `.try_into()`",
    ),
    on(
        all(A = "{integer}", any(Self = "[{integral}; _]",)),
        message = "an array of type `{Self}` cannot be built directly from an iterator",
        label = "try collecting into a `Vec<{A}>`, then using `.try_into()`",
    ),
    message = "a value of type `{Self}` cannot be built from an iterator \
               over elements of type `{A}`",
    label = "value of type `{Self}` cannot be built from `std::iter::Iterator<Item={A}>`"
)]
#[rustc_diagnostic_item = "FromIterator"]
pub trait FromIterator<A>: Sized {
    /// 从 iterator 创建一个值。
    ///
    /// 更多说明见[模块级文档]。
    ///
    /// [模块级文档]: crate::iter
    ///
    /// # 示例
    ///
    /// ```
    /// let five_fives = std::iter::repeat(5).take(5);
    ///
    /// let v = Vec::from_iter(five_fives);
    ///
    /// assert_eq!(v, vec![5, 5, 5, 5, 5]);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_diagnostic_item = "from_iter_fn"]
    fn from_iter<T: IntoIterator<Item = A>>(iter: T) -> Self;
}

/// 转换为 [`Iterator`]。
///
/// 为某个类型实现 `IntoIterator`，就是定义该类型如何被转换成 iterator。对于各种
/// 集合类型来说，这很常见，因为集合通常需要支持按元素迭代。
///
/// 实现 `IntoIterator` 的一个好处是，你的类型将能[配合 Rust 的 `for` 循环语法
/// 使用](crate::iter#for-loops-and-intoiterator)。
///
/// 另见: [`FromIterator`]。
///
/// # 示例
///
/// 基本用法:
///
/// ```
/// let v = [1, 2, 3];
/// let mut iter = v.into_iter();
///
/// assert_eq!(Some(1), iter.next());
/// assert_eq!(Some(2), iter.next());
/// assert_eq!(Some(3), iter.next());
/// assert_eq!(None, iter.next());
/// ```
/// 为自己的类型实现 `IntoIterator`:
///
/// ```
/// // 一个示例集合，只是 Vec<T> 的简单包装
/// #[derive(Debug)]
/// struct MyCollection(Vec<i32>);
///
/// // 给它一些方法，便于创建集合并向其中加入元素。
/// impl MyCollection {
///     fn new() -> MyCollection {
///         MyCollection(Vec::new())
///     }
///
///     fn add(&mut self, elem: i32) {
///         self.0.push(elem);
///     }
/// }
///
/// // 接着实现 IntoIterator
/// impl IntoIterator for MyCollection {
///     type Item = i32;
///     type IntoIter = std::vec::IntoIter<Self::Item>;
///
///     fn into_iter(self) -> Self::IntoIter {
///         self.0.into_iter()
///     }
/// }
///
/// // 现在可以创建一个新的集合...
/// let mut c = MyCollection::new();
///
/// // ... 向其中加入一些内容 ...
/// c.add(0);
/// c.add(1);
/// c.add(2);
///
/// // ... 然后把它转换为 Iterator:
/// for (i, n) in c.into_iter().enumerate() {
///     assert_eq!(i as i32, n);
/// }
/// ```
///
/// `IntoIterator` 常被用作 trait bound。这样输入集合的具体类型可以变化，只要它
/// 仍然能转换成 iterator 即可。也可以通过限制 `Item` 来补充更具体的约束:
///
/// ```rust
/// fn collect_as_strings<T>(collection: T) -> Vec<String>
/// where
///     T: IntoIterator,
///     T::Item: std::fmt::Debug,
/// {
///     collection
///         .into_iter()
///         .map(|item| format!("{item:?}"))
///         .collect()
/// }
/// ```
#[rustc_diagnostic_item = "IntoIterator"]
#[rustc_on_unimplemented(
    on(
        Self = "core::ops::range::RangeTo<Idx>",
        label = "if you meant to iterate until a value, add a starting value",
        note = "`..end` is a `RangeTo`, which cannot be iterated on; you might have meant to have a \
              bounded `Range`: `0..end`"
    ),
    on(
        Self = "core::ops::range::RangeToInclusive<Idx>",
        label = "if you meant to iterate until a value (including it), add a starting value",
        note = "`..=end` is a `RangeToInclusive`, which cannot be iterated on; you might have meant \
              to have a bounded `RangeInclusive`: `0..=end`"
    ),
    on(
        Self = "[]",
        label = "`{Self}` is not an iterator; try calling `.into_iter()` or `.iter()`"
    ),
    on(Self = "&[]", label = "`{Self}` is not an iterator; try calling `.iter()`"),
    on(
        Self = "alloc::vec::Vec<T, A>",
        label = "`{Self}` is not an iterator; try calling `.into_iter()` or `.iter()`"
    ),
    on(Self = "&str", label = "`{Self}` is not an iterator; try calling `.chars()` or `.bytes()`"),
    on(
        Self = "alloc::string::String",
        label = "`{Self}` is not an iterator; try calling `.chars()` or `.bytes()`"
    ),
    on(
        Self = "{integral}",
        note = "if you want to iterate between `start` until a value `end`, use the exclusive range \
              syntax `start..end` or the inclusive range syntax `start..=end`"
    ),
    on(
        Self = "{float}",
        note = "if you want to iterate between `start` until a value `end`, use the exclusive range \
              syntax `start..end` or the inclusive range syntax `start..=end`"
    ),
    label = "`{Self}` is not an iterator",
    message = "`{Self}` is not an iterator"
)]
#[rustc_skip_during_method_dispatch(array, boxed_slice)]
#[stable(feature = "rust1", since = "1.0.0")]
pub trait IntoIterator {
    /// 被迭代的元素类型。
    #[stable(feature = "rust1", since = "1.0.0")]
    type Item;

    /// 该值会被转换成哪一种 iterator。
    #[stable(feature = "rust1", since = "1.0.0")]
    type IntoIter: Iterator<Item = Self::Item>;

    /// 从一个值创建 iterator。
    ///
    /// 更多说明见[模块级文档]。
    ///
    /// [模块级文档]: crate::iter
    ///
    /// # 示例
    ///
    /// ```
    /// let v = [1, 2, 3];
    /// let mut iter = v.into_iter();
    ///
    /// assert_eq!(Some(1), iter.next());
    /// assert_eq!(Some(2), iter.next());
    /// assert_eq!(Some(3), iter.next());
    /// assert_eq!(None, iter.next());
    /// ```
    #[lang = "into_iter"]
    #[stable(feature = "rust1", since = "1.0.0")]
    fn into_iter(self) -> Self::IntoIter;
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<I: Iterator> IntoIterator for I {
    type Item = I::Item;
    type IntoIter = I;

    #[inline]
    fn into_iter(self) -> I {
        self
    }
}

/// 使用 iterator 的内容扩展集合。
///
/// Iterator 会产生一串值，而集合也可以看作一串值的容器。`Extend` trait 连接了
/// 这两者: 它允许把某个 iterator 产出的内容追加或合并进已有集合。对于带键的集合，
/// 如果扩展时遇到已存在的键，则对应条目会被更新；如果集合允许多个相等键对应的
/// 条目，则会插入新的条目。
///
/// # 示例
///
/// 基本用法:
///
/// ```
/// // 可以用一些 char 扩展 String:
/// let mut message = String::from("The first three letters are: ");
///
/// message.extend(&['a', 'b', 'c']);
///
/// assert_eq!("abc", &message[29..32]);
/// ```
///
/// 实现 `Extend`:
///
/// ```
/// // 一个示例集合，只是 Vec<T> 的简单包装
/// #[derive(Debug)]
/// struct MyCollection(Vec<i32>);
///
/// // 给它一些方法，便于创建集合并向其中加入元素。
/// impl MyCollection {
///     fn new() -> MyCollection {
///         MyCollection(Vec::new())
///     }
///
///     fn add(&mut self, elem: i32) {
///         self.0.push(elem);
///     }
/// }
///
/// // 由于 MyCollection 保存的是 i32 列表，因此为 i32 实现 Extend
/// impl Extend<i32> for MyCollection {
///
///     // 使用具体类型签名时更容易理解: 任何能转换成产出 i32 的 Iterator 的值，
///     // 都可以传给 extend，因为 MyCollection 需要放入 i32。
///     fn extend<T: IntoIterator<Item=i32>>(&mut self, iter: T) {
///
///         // 实现很直接: 遍历 iterator，把每个元素 add() 到自身。
///         for elem in iter {
///             self.add(elem);
///         }
///     }
/// }
///
/// let mut c = MyCollection::new();
///
/// c.add(5);
/// c.add(6);
/// c.add(7);
///
/// // 用另外三个数字扩展集合
/// c.extend(vec![1, 2, 3]);
///
/// // 这些元素已经被追加到末尾
/// assert_eq!("MyCollection([5, 6, 7, 1, 2, 3])", format!("{c:?}"));
/// ```
#[stable(feature = "rust1", since = "1.0.0")]
pub trait Extend<A> {
    /// 使用 iterator 的内容扩展集合。
    ///
    /// 这是该 trait 唯一必需实现的方法，因此更多细节见 [trait 级文档][trait-level]。
    ///
    /// [trait-level]: Extend
    ///
    /// # 示例
    ///
    /// ```
    /// // 可以用一些 char 扩展 String:
    /// let mut message = String::from("abc");
    ///
    /// message.extend(['d', 'e', 'f'].iter());
    ///
    /// assert_eq!("abcdef", &message);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    fn extend<T: IntoIterator<Item = A>>(&mut self, iter: T);

    /// 用恰好一个元素扩展集合。
    #[unstable(feature = "extend_one", issue = "72631")]
    fn extend_one(&mut self, item: A) {
        self.extend(Some(item));
    }

    /// 在集合中为给定数量的额外元素预留容量。
    ///
    /// 默认实现不执行任何操作。
    #[unstable(feature = "extend_one", issue = "72631")]
    fn extend_reserve(&mut self, additional: usize) {
        let _ = additional;
    }

    /// 用一个元素扩展集合，但不检查集合是否有足够容量容纳它。
    ///
    /// # 安全性(Safety）
    ///
    /// **对调用者:** 只有在已知集合拥有足够容量可容纳新元素时才能调用，例如之前已经
    /// 调用了 `extend_reserve`。
    ///
    /// **对实现者:** 如果集合要在 unsafe 代码中依赖此方法的安全前置条件（也就是说，
    /// 条件被违反时可能触发 UB），就必须正确实现 `extend_reserve`。换句话说，调用者
    /// 可以假设: 如果已经通过 `extend_reserve` 预留了足够空间，就可以调用此方法。
    // 此方法仅供内部使用。它出现在 trait 上只是因为特化的限制。
    #[unstable(feature = "extend_one_unchecked", issue = "none")]
    #[doc(hidden)]
    unsafe fn extend_one_unchecked(&mut self, item: A)
    where
        Self: Sized,
    {
        self.extend_one(item);
    }
}

#[stable(feature = "extend_for_unit", since = "1.28.0")]
impl Extend<()> for () {
    fn extend<T: IntoIterator<Item = ()>>(&mut self, iter: T) {
        iter.into_iter().for_each(drop)
    }
    fn extend_one(&mut self, _item: ()) {}
}

/// 该 trait 为长度最多十二项的元组实现。1 元组以及 3 到 12 元组的 `impl` 在
/// 2 元组之后稳定，于 1.85.0 稳定。
#[doc(fake_variadic)] // 其他实现在下方。
#[stable(feature = "extend_for_tuple", since = "1.56.0")]
impl<T, ExtendT> Extend<(T,)> for (ExtendT,)
where
    ExtendT: Extend<T>,
{
    /// 允许对一个由集合组成、且各集合也实现 `Extend` 的元组执行 `extend`。
    ///
    /// 另见: [`Iterator::unzip`]
    ///
    /// # 示例
    /// ```
    /// // 这里给出 2 元组示例，但 1 到 12 元组都受支持
    /// let mut tuple = (vec![0], vec![1]);
    /// tuple.extend([(2, 3), (4, 5), (6, 7)]);
    /// assert_eq!(tuple.0, [0, 2, 4, 6]);
    /// assert_eq!(tuple.1, [1, 3, 5, 7]);
    ///
    /// // 也允许元素中包含任意嵌套的元组
    /// let mut nested_tuple = (vec![1], (vec![2], vec![3]));
    /// nested_tuple.extend([(4, (5, 6)), (7, (8, 9))]);
    ///
    /// let (a, (b, c)) = nested_tuple;
    /// assert_eq!(a, [1, 4, 7]);
    /// assert_eq!(b, [2, 5, 8]);
    /// assert_eq!(c, [3, 6, 9]);
    /// ```
    fn extend<I: IntoIterator<Item = (T,)>>(&mut self, iter: I) {
        self.0.extend(iter.into_iter().map(|t| t.0));
    }

    fn extend_one(&mut self, item: (T,)) {
        self.0.extend_one(item.0)
    }

    fn extend_reserve(&mut self, additional: usize) {
        self.0.extend_reserve(additional)
    }

    unsafe fn extend_one_unchecked(&mut self, item: (T,)) {
        // SAFETY: 调用者保证了所有前置条件。
        unsafe { self.0.extend_one_unchecked(item.0) }
    }
}

/// 该实现会把由元组组成的 iterator 转换成一个元组，结果元组中的各类型都实现
/// [`Default`] 和 [`Extend`]。
///
/// 这类似 [`Iterator::unzip`]，但也能与其他 [`FromIterator`] 实现组合使用:
///
/// ```rust
/// # fn main() -> Result<(), core::num::ParseIntError> {
/// let string = "1,2,123,4";
///
/// // 这里给出 2 元组示例，但 1 到 12 元组都受支持
/// let (numbers, lengths): (Vec<_>, Vec<_>) = string
///     .split(',')
///     .map(|s| s.parse().map(|n: u32| (n, s.len())))
///     .collect::<Result<_, _>>()?;
///
/// assert_eq!(numbers, [1, 2, 123, 4]);
/// assert_eq!(lengths, [1, 1, 3, 1]);
/// # Ok(()) }
/// ```
#[doc(fake_variadic)] // 其他实现在下方。
#[stable(feature = "from_iterator_for_tuple", since = "1.79.0")]
impl<T, ExtendT> FromIterator<(T,)> for (ExtendT,)
where
    ExtendT: Default + Extend<T>,
{
    fn from_iter<Iter: IntoIterator<Item = (T,)>>(iter: Iter) -> Self {
        let mut res = ExtendT::default();
        res.extend(iter.into_iter().map(|t| t.0));
        (res,)
    }
}

/// [`extend`](Extend::extend) 的一个实现，对 iterator 的每个元素调用 `extend_one`
/// 或 `extend_one_unchecked`。
fn default_extend<ExtendT, I, T>(collection: &mut ExtendT, iter: I)
where
    ExtendT: Extend<T>,
    I: IntoIterator<Item = T>,
{
    // 针对 `TrustedLen` 特化，并在适用时调用 `extend_one_unchecked`。
    trait SpecExtend<I> {
        fn extend(&mut self, iter: I);
    }

    // 抽到独立函数中，避免为每种 iterator 类型都单态化这些闭包。
    fn extender<ExtendT, T>(collection: &mut ExtendT) -> impl FnMut(T) + use<'_, ExtendT, T>
    where
        ExtendT: Extend<T>,
    {
        move |item| collection.extend_one(item)
    }

    unsafe fn unchecked_extender<ExtendT, T>(
        collection: &mut ExtendT,
    ) -> impl FnMut(T) + use<'_, ExtendT, T>
    where
        ExtendT: Extend<T>,
    {
        // SAFETY: 该函数的调用点会确保有足够空间。
        move |item| unsafe { collection.extend_one_unchecked(item) }
    }

    impl<ExtendT, I, T> SpecExtend<I> for ExtendT
    where
        ExtendT: Extend<T>,
        I: Iterator<Item = T>,
    {
        default fn extend(&mut self, iter: I) {
            let (lower_bound, _) = iter.size_hint();
            if lower_bound > 0 {
                self.extend_reserve(lower_bound);
            }

            iter.for_each(extender(self))
        }
    }

    impl<ExtendT, I, T> SpecExtend<I> for ExtendT
    where
        ExtendT: Extend<T>,
        I: TrustedLen<Item = T>,
    {
        fn extend(&mut self, iter: I) {
            let (lower_bound, upper_bound) = iter.size_hint();
            if lower_bound > 0 {
                self.extend_reserve(lower_bound);
            }

            if upper_bound.is_none() {
                // 无法预留超过 `usize::MAX` 个元素，而且这种情况大概率本来也会耗尽内存。
                iter.for_each(extender(self))
            } else {
                // SAFETY: 我们按照 `size_hint` 预留了足够空间，且 iterator 是
                // `TrustedLen`，因此它的 `size_hint` 是精确的。
                iter.for_each(unsafe { unchecked_extender(self) })
            }
        }
    }

    SpecExtend::extend(collection, iter.into_iter());
}

// 为长度大于一的元组实现 `Extend` 和 `FromIterator`。
macro_rules! impl_extend_tuple {
    ($(($ty:tt, $extend_ty:tt, $index:tt)),+) => {
        #[doc(hidden)]
        #[stable(feature = "extend_for_tuple", since = "1.56.0")]
        impl<$($ty,)+ $($extend_ty,)+> Extend<($($ty,)+)> for ($($extend_ty,)+)
        where
            $($extend_ty: Extend<$ty>,)+
        {
            fn extend<T: IntoIterator<Item = ($($ty,)+)>>(&mut self, iter: T) {
                default_extend(self, iter)
            }

            fn extend_one(&mut self, item: ($($ty,)+)) {
                $(self.$index.extend_one(item.$index);)+
            }

            fn extend_reserve(&mut self, additional: usize) {
                $(self.$index.extend_reserve(additional);)+
            }

            unsafe fn extend_one_unchecked(&mut self, item: ($($ty,)+)) {
                // SAFETY: 这些正是我们的安全前置条件，并且我们正确转发了 `extend_reserve`。
                unsafe {
                    $(self.$index.extend_one_unchecked(item.$index);)+
                }
            }
        }

        #[doc(hidden)]
        #[stable(feature = "from_iterator_for_tuple", since = "1.79.0")]
        impl<$($ty,)+ $($extend_ty,)+> FromIterator<($($ty,)+)> for ($($extend_ty,)+)
        where
            $($extend_ty: Default + Extend<$ty>,)+
        {
            fn from_iter<Iter: IntoIterator<Item = ($($ty,)+)>>(iter: Iter) -> Self {
                let mut res = Self::default();
                res.extend(iter);
                res
            }
        }
    };
}

impl_extend_tuple!((A, ExA, 0), (B, ExB, 1));
impl_extend_tuple!((A, ExA, 0), (B, ExB, 1), (C, ExC, 2));
impl_extend_tuple!((A, ExA, 0), (B, ExB, 1), (C, ExC, 2), (D, ExD, 3));
impl_extend_tuple!((A, ExA, 0), (B, ExB, 1), (C, ExC, 2), (D, ExD, 3), (E, ExE, 4));
impl_extend_tuple!((A, ExA, 0), (B, ExB, 1), (C, ExC, 2), (D, ExD, 3), (E, ExE, 4), (F, ExF, 5));
impl_extend_tuple!(
    (A, ExA, 0),
    (B, ExB, 1),
    (C, ExC, 2),
    (D, ExD, 3),
    (E, ExE, 4),
    (F, ExF, 5),
    (G, ExG, 6)
);
impl_extend_tuple!(
    (A, ExA, 0),
    (B, ExB, 1),
    (C, ExC, 2),
    (D, ExD, 3),
    (E, ExE, 4),
    (F, ExF, 5),
    (G, ExG, 6),
    (H, ExH, 7)
);
impl_extend_tuple!(
    (A, ExA, 0),
    (B, ExB, 1),
    (C, ExC, 2),
    (D, ExD, 3),
    (E, ExE, 4),
    (F, ExF, 5),
    (G, ExG, 6),
    (H, ExH, 7),
    (I, ExI, 8)
);
impl_extend_tuple!(
    (A, ExA, 0),
    (B, ExB, 1),
    (C, ExC, 2),
    (D, ExD, 3),
    (E, ExE, 4),
    (F, ExF, 5),
    (G, ExG, 6),
    (H, ExH, 7),
    (I, ExI, 8),
    (J, ExJ, 9)
);
impl_extend_tuple!(
    (A, ExA, 0),
    (B, ExB, 1),
    (C, ExC, 2),
    (D, ExD, 3),
    (E, ExE, 4),
    (F, ExF, 5),
    (G, ExG, 6),
    (H, ExH, 7),
    (I, ExI, 8),
    (J, ExJ, 9),
    (K, ExK, 10)
);
impl_extend_tuple!(
    (A, ExA, 0),
    (B, ExB, 1),
    (C, ExC, 2),
    (D, ExD, 3),
    (E, ExE, 4),
    (F, ExF, 5),
    (G, ExG, 6),
    (H, ExH, 7),
    (I, ExI, 8),
    (J, ExJ, 9),
    (K, ExK, 10),
    (L, ExL, 11)
);
