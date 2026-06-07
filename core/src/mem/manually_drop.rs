use crate::cmp::Ordering;
use crate::hash::{Hash, Hasher};
use crate::marker::{Destruct, StructuralPartialEq};
use crate::mem::MaybeDangling;
use crate::ops::{Deref, DerefMut, DerefPure};
use crate::ptr;

/// 一个包装器，用于抑制编译器自动调用 `T` 的析构逻辑（destructor）。
/// 此包装器是 0 成本的。
///
/// `ManuallyDrop<T>` 保证拥有与 `T` 相同的内存布局与位有效性（bit validity），
/// 并享受与 `T` 相同的布局优化。因此，它对编译器关于其内容所做的假设*没有任何影响*。
/// 举例来说，用 [`mem::zeroed`] 来初始化一个 `ManuallyDrop<&mut T>` 就是未定义行为（UB）。
/// 如果你需要处理未初始化的数据，请改用 [`MaybeUninit<T>`]。
///
/// 注意，访问 `ManuallyDrop<T>` 内部的值是安全的。这意味着一个内容已经被 drop 的
/// `ManuallyDrop<T>` 绝不能通过公开的安全 API 暴露出去。相应地，`ManuallyDrop::drop`
/// 是 unsafe 的。
///
/// # `ManuallyDrop` 与 drop 顺序
///
/// Rust 对值有着明确定义的 [drop 顺序][drop order]。要确保字段或局部变量按特定顺序被 drop，
/// 应当重新排列声明的顺序，使隐式的 drop 顺序正好是你想要的那个。
///
/// 也可以用 `ManuallyDrop` 来控制 drop 顺序，但这需要 unsafe 代码，
/// 而且在存在展开（unwinding）的情况下很难写对。
///
/// 例如，如果你想确保某个特定字段在其他字段*之后*才被 drop，就把它放在结构体的最后一个字段：
///
/// ```
/// struct Context;
///
/// struct Widget {
///     children: Vec<Widget>,
///     // `context` 将在 `children` 之后被 drop。
///     // Rust 保证字段按声明顺序被 drop。
///     context: Context,
/// }
/// ```
///
/// # 与 `Box` 的交互
///
/// 目前，如果你有一个 `ManuallyDrop<T>`，其中类型 `T` 是一个 `Box` 或在内部包含一个 `Box`，
/// 那么先 drop 这个 `T`、随后再 move 这个 `ManuallyDrop<T>`，[会被认为是未定义行为](https://github.com/rust-lang/unsafe-code-guidelines/issues/245)。
/// 也就是说，下面这段代码会导致未定义行为：
///
/// ```no_run
/// use std::mem::ManuallyDrop;
///
/// let mut x = ManuallyDrop::new(Box::new(42));
/// unsafe {
///     ManuallyDrop::drop(&mut x);
/// }
/// let y = x; // 未定义行为！
/// ```
///
/// 这一点[在未来很可能会改变](https://rust-lang.github.io/rfcs/3336-maybe-dangling.html)。
/// 在此之前，请考虑改用 [`MaybeUninit`]。
///
/// # 在结构体或枚举中存储 `ManuallyDrop` 时的安全隐患。
///
/// 当下面所有条件同时满足时，需要格外小心：
/// * 某个结构体或枚举包含一个 `ManuallyDrop`。
/// * 该 `ManuallyDrop` 不在某个 `union` 内部。
/// * 该结构体或枚举是公开 API 的一部分，或被存储在某个属于公开 API 的结构体或枚举中。
/// * 存在 drop 该 `ManuallyDrop` 字段内容的代码，且这段代码位于该结构体或枚举的 `Drop`
///   实现之外。
///
/// 具体而言，可能出现以下隐患：
///
/// #### 存储泛型类型
///
/// 如果该 `ManuallyDrop` 包含一个由调用方提供的泛型类型，调用方可能会把一个 `Box` 作为该类型传入。
/// 正如上一节所述，这会在该结构体或枚举随后被 move 时导致未定义行为。
/// 例如，下面这段代码会导致未定义行为：
///
/// ```no_run
/// use std::mem::ManuallyDrop;
///
/// pub struct BadOption<T> {
///     // 不变量：若 `is_some` 为 false，则它已被 drop。
///     value: ManuallyDrop<T>,
///     is_some: bool,
/// }
/// impl<T> BadOption<T> {
///     pub fn new(value: T) -> Self {
///         Self { value: ManuallyDrop::new(value), is_some: true }
///     }
///     pub fn change_to_none(&mut self) {
///         if self.is_some {
///             self.is_some = false;
///             unsafe {
///                 // SAFETY: 按照不变量，`value` 此时尚未被 drop
///                 // （这其实是不健全的！）
///                 ManuallyDrop::drop(&mut self.value);
///             }
///         }
///     }
/// }
///
/// // 在另一个 crate 中：
///
/// let mut option = BadOption::new(Box::new(42));
/// option.change_to_none();
/// let option2 = option; // 未定义行为！
/// ```
///
/// #### 派生（derive）trait
///
/// 在该结构体或枚举上派生 `Debug`、`Clone`、`PartialEq`、`PartialOrd`、`Ord` 或 `Hash`
/// 可能是不健全的，因为这些 trait 的派生实现会访问该 `ManuallyDrop` 字段。
/// 例如，下面这段代码会导致未定义行为：
///
/// ```no_run
/// use std::mem::ManuallyDrop;
///
/// // 这个 derive 与下面的 `ManuallyDrop::drop` 调用组合在一起就是不健全的。
/// #[derive(Debug)]
/// pub struct Foo {
///     value: ManuallyDrop<String>,
/// }
/// impl Foo {
///     pub fn new() -> Self {
///         let mut temp = Self {
///             value: ManuallyDrop::new(String::from("Unsafe rust is hard."))
///         };
///         unsafe {
///             // SAFETY: `value` 此时尚未被 drop。
///             ManuallyDrop::drop(&mut temp.value);
///         }
///         temp
///     }
/// }
///
/// // 在另一个 crate 中：
///
/// let foo = Foo::new();
/// println!("{:?}", foo); // 未定义行为！
/// ```
///
/// [drop order]: https://doc.rust-lang.org/reference/destructors.html
/// [`mem::zeroed`]: crate::mem::zeroed
/// [`MaybeUninit<T>`]: crate::mem::MaybeUninit
/// [`MaybeUninit`]: crate::mem::MaybeUninit
#[stable(feature = "manually_drop", since = "1.20.0")]
#[lang = "manually_drop"]
#[derive(Copy, Clone, Debug, Default)]
#[repr(transparent)]
#[rustc_pub_transparent]
pub struct ManuallyDrop<T: ?Sized> {
    value: MaybeDangling<T>,
}

impl<T> ManuallyDrop<T> {
    /// 包装一个值，使其改为手动 drop。
    ///
    /// # 示例
    ///
    /// ```rust
    /// use std::mem::ManuallyDrop;
    /// let mut x = ManuallyDrop::new(String::from("Hello World!"));
    /// x.truncate(5); // 你仍然可以安全地操作这个值
    /// assert_eq!(*x, "Hello");
    /// // 但 `Drop` 不会在这里被运行
    /// # // FIXME(https://github.com/rust-lang/miri/issues/3670):
    /// # // 在意在泄漏的测试中，使用 -Zmiri-disable-leak-check 而不是手动取出避免泄漏。
    /// # let _ = ManuallyDrop::into_inner(x);
    /// ```
    #[must_use = "if you don't need the wrapper, you can use `mem::forget` instead"]
    #[stable(feature = "manually_drop", since = "1.20.0")]
    #[rustc_const_stable(feature = "const_manually_drop", since = "1.32.0")]
    #[inline(always)]
    pub const fn new(value: T) -> ManuallyDrop<T> {
        ManuallyDrop { value: MaybeDangling::new(value) }
    }

    /// 从 `ManuallyDrop` 容器中提取出值。
    ///
    /// 这会让该值重新可以被 drop。
    ///
    /// # 示例
    ///
    /// ```rust
    /// use std::mem::ManuallyDrop;
    /// let x = ManuallyDrop::new(Box::new(()));
    /// let _: Box<()> = ManuallyDrop::into_inner(x); // 这会 drop 那个 `Box`。
    /// ```
    #[stable(feature = "manually_drop", since = "1.20.0")]
    #[rustc_const_stable(feature = "const_manually_drop", since = "1.32.0")]
    #[inline(always)]
    pub const fn into_inner(slot: ManuallyDrop<T>) -> T {
        // 不能使用 `MaybeDangling::into_inner`，因为它目前还不具备我们想要的语义。
        // SAFETY: 我们知道这是一个有效的 `T`。`slot` 不会被 drop。
        unsafe { (&raw const slot).cast::<T>().read() }
    }

    /// 把值从 `ManuallyDrop<T>` 容器中取（take）出来。
    ///
    /// 此方法主要用于在 drop 过程中 move 出值。与其用 [`ManuallyDrop::drop`] 手动 drop 该值，
    /// 你可以用此方法把值取出来，然后按你的需要随意使用它。
    ///
    /// 只要有可能，就更应该改用 [`into_inner`][`ManuallyDrop::into_inner`]，
    /// 它能避免重复（duplicate）`ManuallyDrop<T>` 的内容。
    ///
    /// # 安全性(Safety）
    ///
    /// 此函数在语义上把所含的值 move 出去，但并不阻止后续继续使用它，
    /// 并且会让此容器的状态保持不变。
    /// 确保此 `ManuallyDrop` 不再被使用，是你的责任。
    ///
    #[must_use = "if you don't need the value, you can use `ManuallyDrop::drop` instead"]
    #[stable(feature = "manually_drop_take", since = "1.42.0")]
    #[rustc_const_unstable(feature = "const_manually_drop_take", issue = "148773")]
    #[inline]
    pub const unsafe fn take(slot: &mut ManuallyDrop<T>) -> T {
        // SAFETY: 我们正在从一个引用读取，而引用保证对读取（reads）是有效的。
        unsafe { ptr::read(slot.value.as_ref()) }
    }
}

impl<T: ?Sized> ManuallyDrop<T> {
    /// 手动 drop 所含的值。
    ///
    /// 这与对所含值的指针调用 [`ptr::drop_in_place`] 完全等价。因此，除非所含的值是一个
    /// packed 结构体，否则析构逻辑会就地（in-place）被调用而不会 move 这个值，
    /// 因而它可以被用来安全地 drop [被 pin 的][pinned]数据。
    ///
    /// 如果你拥有该值的所有权，可以改用 [`ManuallyDrop::into_inner`]。
    ///
    /// # 安全性(Safety）
    ///
    /// 此函数会运行所含值的析构逻辑。除了析构逻辑自身造成的改动以外，内存保持不变，
    /// 因此在编译器看来，这块内存仍然持有一个对类型 `T` 有效的位模式（bit-pattern）。
    ///
    /// 然而，这个“僵尸（zombie）”值不应被暴露给安全代码，并且此函数不应被调用超过一次。
    /// 使用一个已被 drop 的值，或者多次 drop 一个值，都可能导致未定义行为
    ///（具体取决于 `drop` 做了什么）。通常这是由类型系统来阻止的，但 `ManuallyDrop`
    /// 的使用者必须在没有编译器协助的情况下自行维护这些保证。
    ///
    /// [pinned]: crate::pin
    #[stable(feature = "manually_drop", since = "1.20.0")]
    #[inline]
    #[rustc_const_unstable(feature = "const_drop_in_place", issue = "109342")]
    pub const unsafe fn drop(slot: &mut ManuallyDrop<T>)
    where
        T: [const] Destruct,
    {
        // SAFETY: 我们正在 drop 一个可变引用所指向的值，而该引用保证对写入（writes）有效。
        // 至于确保 `slot` 不会被再次 drop，则是调用方的责任。
        unsafe { ptr::drop_in_place(slot.value.as_mut()) }
    }
}

#[stable(feature = "manually_drop", since = "1.20.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl<T: ?Sized> const Deref for ManuallyDrop<T> {
    type Target = T;
    #[inline(always)]
    fn deref(&self) -> &T {
        self.value.as_ref()
    }
}

#[stable(feature = "manually_drop", since = "1.20.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl<T: ?Sized> const DerefMut for ManuallyDrop<T> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut T {
        self.value.as_mut()
    }
}

#[unstable(feature = "deref_pure_trait", issue = "87121")]
unsafe impl<T: ?Sized> DerefPure for ManuallyDrop<T> {}

#[stable(feature = "manually_drop", since = "1.20.0")]
impl<T: ?Sized + Eq> Eq for ManuallyDrop<T> {}

#[stable(feature = "manually_drop", since = "1.20.0")]
impl<T: ?Sized + PartialEq> PartialEq for ManuallyDrop<T> {
    fn eq(&self, other: &Self) -> bool {
        self.value.as_ref().eq(other.value.as_ref())
    }
}

#[stable(feature = "manually_drop", since = "1.20.0")]
impl<T: ?Sized> StructuralPartialEq for ManuallyDrop<T> {}

#[stable(feature = "manually_drop", since = "1.20.0")]
impl<T: ?Sized + Ord> Ord for ManuallyDrop<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.value.as_ref().cmp(other.value.as_ref())
    }
}

#[stable(feature = "manually_drop", since = "1.20.0")]
impl<T: ?Sized + PartialOrd> PartialOrd for ManuallyDrop<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.value.as_ref().partial_cmp(other.value.as_ref())
    }
}

#[stable(feature = "manually_drop", since = "1.20.0")]
impl<T: ?Sized + Hash> Hash for ManuallyDrop<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.value.as_ref().hash(state);
    }
}
