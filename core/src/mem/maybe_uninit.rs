use crate::any::type_name;
use crate::clone::TrivialClone;
use crate::marker::Destruct;
use crate::mem::ManuallyDrop;
use crate::{fmt, intrinsics, ptr, slice};

/// 一个用于构造 `T` 的未初始化实例的包装器类型。
///
/// # 初始化不变量（Initialization invariant）
///
/// 一般来说，编译器会假定一个变量已根据其类型的要求被正确初始化。例如，一个引用类型的变量必须
/// 是对齐且非空的。这是一个*始终*都必须维护的不变量，即便在 unsafe 代码中也是如此。因此，对一个
/// 引用类型的变量进行零初始化会立即导致[未定义行为][ub]，无论这个引用是否真的被用来访问过内存：
///
/// ```rust,no_run
/// # #![allow(invalid_value)]
/// use std::mem::{self, MaybeUninit};
///
/// let x: &i32 = unsafe { mem::zeroed() }; // 未定义行为！⚠️
/// // 用 `MaybeUninit<&i32>` 写出的等价代码：
/// let x: &i32 = unsafe { MaybeUninit::zeroed().assume_init() }; // 未定义行为！⚠️
/// ```
///
/// 编译器会利用这一点进行各种优化，例如省略运行时检查、优化 `enum` 的布局。
///
/// 类似地，完全未初始化的内存可能拥有任意内容，而一个 `bool` 则必须始终是 `true` 或 `false`。
/// 因此，创建一个未初始化的 `bool` 是未定义行为：
///
/// ```rust,no_run
/// # #![allow(invalid_value)]
/// use std::mem::{self, MaybeUninit};
///
/// let b: bool = unsafe { mem::uninitialized() }; // 未定义行为！⚠️
/// // 用 `MaybeUninit<bool>` 写出的等价代码：
/// let b: bool = unsafe { MaybeUninit::uninit().assume_init() }; // 未定义行为！⚠️
/// ```
///
/// 此外，未初始化的内存很特殊，因为它没有固定的值（“固定”意为“在未被写入之前它不会改变”）。多次
/// 读取同一个未初始化字节可能得到不同的结果。这使得：即便一个变量是整数类型（它本可以持有任意
/// *固定*的位模式），让它持有未初始化数据也是未定义行为：
///
/// ```rust,no_run
/// # #![allow(invalid_value)]
/// use std::mem::{self, MaybeUninit};
///
/// let x: i32 = unsafe { mem::uninitialized() }; // 未定义行为！⚠️
/// // 用 `MaybeUninit<i32>` 写出的等价代码：
/// let x: i32 = unsafe { MaybeUninit::uninit().assume_init() }; // 未定义行为！⚠️
/// ```
/// 在此之上，请记住大多数类型除了在类型层面被视作已初始化之外，还有额外的不变量。例如，一个被
/// 初始化为 `1` 的 [`Vec<T>`] 被视为已初始化（在当前实现下；这并不构成稳定保证），因为编译器对它
/// 所知道的唯一要求就是其数据指针必须非空。创建这样一个 `Vec<T>` 不会*立即*导致未定义行为，但在
/// 进行大多数安全操作时（包括 drop 它）会导致未定义行为。
///
/// [`Vec<T>`]: ../../std/vec/struct.Vec.html
///
/// # 示例
///
/// `MaybeUninit<T>` 的作用是让 unsafe 代码能够处理未初始化的数据。它是给编译器的一个信号，表明
/// 这里的数据可能*尚未*被初始化：
///
/// ```rust
/// use std::mem::MaybeUninit;
///
/// // 创建一个显式未初始化的引用。编译器知道 `MaybeUninit<T>` 内部的数据
/// // 可能无效，因此这不是 UB：
/// let mut x = MaybeUninit::<&i32>::uninit();
/// // 把它设置为一个有效的值。
/// x.write(&0);
/// // 提取出已初始化的数据 —— 这只有在正确初始化了 `x` *之后*才被允许！
/// let x = unsafe { x.assume_init() };
/// ```
///
/// 这样一来，编译器就知道不要对这段代码做出任何不正确的假设或优化了。
///
/// 你可以把 `MaybeUninit<T>` 想象成有点像 `Option<T>`，但去掉了所有的运行时追踪，
/// 也去掉了所有的安全检查。
///
/// ## out-pointer（输出指针）
///
/// 你可以用 `MaybeUninit<T>` 来实现“out-pointer”：与其从函数返回数据，不如给函数传入一个指向某块
///（未初始化）内存的指针，让函数把结果放进去。当“调用方能控制存放结果的内存如何被分配”这一点很
/// 重要、并且你想避免不必要的 move 时，这会很有用。
///
/// ```
/// use std::mem::MaybeUninit;
///
/// unsafe fn make_vec(out: *mut Vec<i32>) {
///     // `write` 不会 drop 旧的内容，这一点很重要。
///     unsafe { out.write(vec![1, 2, 3]); }
/// }
///
/// let mut v = MaybeUninit::uninit();
/// unsafe { make_vec(v.as_mut_ptr()); }
/// // 现在我们知道 `v` 已被初始化了！这也确保了该 vector 会被正确地 drop。
/// let v = unsafe { v.assume_init() };
/// assert_eq!(&v, &[1, 2, 3]);
/// ```
///
/// ## 逐个元素地初始化一个数组
///
/// `MaybeUninit<T>` 可以被用来逐个元素地初始化一个大数组：
///
/// ```
/// use std::mem::{self, MaybeUninit};
///
/// let data = {
///     // 创建一个由 `MaybeUninit` 组成的未初始化数组。
///     let mut data: [MaybeUninit<Vec<u32>>; 1000] = [const { MaybeUninit::uninit() }; 1000];
///
///     // drop 一个 `MaybeUninit` 什么都不做，所以如果在这个循环中发生 panic，
///     // 我们会有内存泄漏，但不存在内存安全问题。
///     for elem in &mut data[..] {
///         elem.write(vec![42]);
///     }
///
///     // 一切都已初始化。把这个数组 transmute 成已初始化的类型。
///     unsafe { mem::transmute::<_, [Vec<u32>; 1000]>(data) }
/// };
///
/// assert_eq!(&data[0], &[42]);
/// ```
///
/// 你也可以处理部分初始化的数组，这类数组可能出现在底层数据结构中。
///
/// ```
/// use std::mem::MaybeUninit;
///
/// // 创建一个由 `MaybeUninit` 组成的未初始化数组。
/// let mut data: [MaybeUninit<String>; 1000] = [const { MaybeUninit::uninit() }; 1000];
/// // 统计我们已经赋过值的元素数量。
/// let mut data_len: usize = 0;
///
/// for elem in &mut data[0..500] {
///     elem.write(String::from("hello"));
///     data_len += 1;
/// }
///
/// // 对数组中的每一项，如果是我们分配的就 drop 它。
/// for elem in &mut data[0..data_len] {
///     unsafe { elem.assume_init_drop(); }
/// }
/// ```
///
/// ## 逐个字段地初始化一个结构体
///
/// 你可以用 `MaybeUninit<T>` 和 [`&raw mut`] 语法来逐个字段地初始化结构体：
///
/// ```rust
/// use std::mem::MaybeUninit;
///
/// #[derive(Debug, PartialEq)]
/// pub struct Foo {
///     name: String,
///     list: Vec<u8>,
/// }
///
/// let foo = {
///     let mut uninit: MaybeUninit<Foo> = MaybeUninit::uninit();
///     let ptr = uninit.as_mut_ptr();
///
///     // 初始化 `name` 字段
///     // 使用 `write` 而不是通过 `=` 赋值，以避免对旧的、未初始化的值调用 `drop`。
///     unsafe { (&raw mut (*ptr).name).write("Bob".to_string()); }
///
///     // 初始化 `list` 字段
///     // 如果这里发生 panic，那么 `name` 字段中的那个 `String` 会泄漏。
///     unsafe { (&raw mut (*ptr).list).write(vec![0, 1, 2]); }
///
///     // 所有字段都已初始化，所以我们调用 `assume_init` 来得到一个已初始化的 Foo。
///     unsafe { uninit.assume_init() }
/// };
///
/// assert_eq!(
///     foo,
///     Foo {
///         name: "Bob".to_string(),
///         list: vec![0, 1, 2]
///     }
/// );
/// ```
/// [`&raw mut`]: https://doc.rust-lang.org/reference/types/pointer.html#r-type.pointer.raw.constructor
/// [ub]: ../../reference/behavior-considered-undefined.html
///
/// # 布局（Layout）
///
/// `MaybeUninit<T>` 保证拥有与 `T` 相同的大小、对齐方式和 ABI：
///
/// ```rust
/// use std::mem::MaybeUninit;
/// assert_eq!(size_of::<MaybeUninit<u64>>(), size_of::<u64>());
/// assert_eq!(align_of::<MaybeUninit<u64>>(), align_of::<u64>());
/// ```
///
/// 然而请记住，一个*包含* `MaybeUninit<T>` 的类型并不一定拥有相同的布局；一般来说，Rust 并不保证
/// `Foo<T>` 的各字段与 `Foo<U>` 拥有相同的顺序，即便 `T` 和 `U` 大小与对齐都相同。此外，由于任何
/// 位值对于 `MaybeUninit<T>` 都是有效的，编译器无法应用非零/壁龛填充（niche-filling）优化，这可能
/// 导致更大的尺寸：
///
/// ```rust
/// # use std::mem::MaybeUninit;
/// assert_eq!(size_of::<Option<bool>>(), 1);
/// assert_eq!(size_of::<Option<MaybeUninit<bool>>>(), 2);
/// ```
///
/// 如果 `T` 是 FFI 安全的，那么 `MaybeUninit<T>` 也是。
///
/// 虽然 `MaybeUninit` 是 `#[repr(transparent)]`（表明它保证拥有与 `T` 相同的大小、对齐和 ABI），
/// 但这*并不*改变前面提到的任何注意事项。`Option<T>` 与 `Option<MaybeUninit<T>>` 仍然可能拥有不同
/// 的大小，而且包含一个类型为 `T` 的字段的类型，其布局（与大小）也可能不同于该字段为
/// `MaybeUninit<T>` 时的情形。`MaybeUninit` 是一个联合体（union）类型，而 union 上的
/// `#[repr(transparent)]` 是不稳定的（参见[追踪 issue](https://github.com/rust-lang/rust/issues/60405)）。
/// 随着时间推移，union 上 `#[repr(transparent)]` 的确切保证可能会演变，`MaybeUninit` 也可能仍是、
/// 也可能不再是 `#[repr(transparent)]`。话虽如此，`MaybeUninit<T>` *始终*会保证它拥有与 `T` 相同
/// 的大小、对齐和 ABI；只是 `MaybeUninit` 实现这一保证的方式可能会演变。
///
/// 注意，即便 `T` 与 `MaybeUninit<T>` 是 ABI 兼容的，把 `&mut T` transmute 为 `&mut MaybeUninit<T>`
/// 并把它暴露给安全代码仍然是不健全的，因为那会允许安全代码访问未初始化的内存：
///
/// ```rust,no_run
/// use core::mem::MaybeUninit;
///
/// fn unsound_transmute<T>(val: &mut T) -> &mut MaybeUninit<T> {
///     unsafe { core::mem::transmute(val) }
/// }
///
/// fn main() {
///     let mut code = 0;
///     let code = &mut code;
///     let code2 = unsound_transmute(code);
///     *code2 = MaybeUninit::uninit();
///     std::process::exit(*code); // UB！访问了未初始化的内存。
/// }
/// ```
///
/// # 有效性（Validity）
///
/// `MaybeUninit<T>` 没有任何有效性要求——任何长度合适的[字节][bytes]序列，无论已初始化还是未初始
/// 化，都是一个有效的表示。
///
/// move 或复制（copy）一个 `MaybeUninit<T>` 类型的值（即进行一次“带类型的复制（typed copy）”）会
/// 精确地保留该值表示中、类型 `T` 所有非填充字节的内容，包括它们的[来源（provenance）][provenance]。
///
/// 因此，如果满足两个条件，`MaybeUninit` 就可以被用来把一个值从类型 `T` 往返（round trip）到类型
/// `MaybeUninit<U>` 再回到类型 `T`，并保留原始的值。其一，类型 `U` 必须与类型 `T` 大小相同。
/// 其二，对于类型 `U` 存在填充的所有字节偏移量处，该值表示中相应的字节都必须是未初始化的。
///
/// 例如，由于类型 `[u8; size_of::<T>]` 没有填充这一事实，下面这段代码对任意类型 `T` 都是健全的，
/// 并会返回原始的值：
///
/// ```rust,no_run
/// # use core::mem::{MaybeUninit, transmute};
/// # struct T;
/// fn identity(t: T) -> T {
///     unsafe {
///         let u: MaybeUninit<[u8; size_of::<T>()]> = transmute(t);
///         transmute(u) // OK。
///     }
/// }
/// ```
///
/// 注意：复制一个含有引用的值可能会隐式地对它们重借用（reborrow），从而导致返回值的来源
///（provenance）不同于原始值。这同样适用于那个平凡的恒等函数：
///
/// ```rust,no_run
/// fn trivial_identity<T>(t: T) -> T { t }
/// ```
///
/// 注意：move 或复制一个值，若它的表示在“类型存在填充的字节偏移量处”含有已初始化的字节，则可能
/// 丢失这些字节的值，因此尽管原始的值会被保留，该值原始的字节*表示*却可能不会被保留。同样，
/// 这也适用于 `trivial_identity`。
///
/// 注意：当类型 `U` 在“原始值表示中含有已初始化字节的字节偏移量处”存在填充时，执行这种往返可能
/// 产生未定义行为或得到一个不同的值。例如，下面这段代码是不健全的，因为 `T` 要求所有字节都被
/// 初始化：
///
/// ```rust,no_run
/// # use core::mem::{MaybeUninit, transmute};
/// #[repr(C)] struct T([u8; 4]);
/// #[repr(C)] struct U(u8, u16);
/// fn unsound_identity(t: T) -> T {
///     unsafe {
///         let u: MaybeUninit<U> = transmute(t);
///         transmute(u) // UB。
///     }
/// }
/// ```
///
/// 反过来，下面这段代码是健全的，因为 `T` 允许其值表示中存在未初始化的字节，但这次往返可能改变
/// 该值：
///
/// ```rust,no_run
/// # use core::mem::{MaybeUninit, transmute};
/// #[repr(C)] struct T(MaybeUninit<[u8; 4]>);
/// #[repr(C)] struct U(u8, u16);
/// fn non_identity(t: T) -> T {
///     unsafe {
///         // 可能丢失一个已初始化的字节。
///         let u: MaybeUninit<U> = transmute(t);
///         transmute(u)
///     }
/// }
/// ```
///
/// [bytes]: ../../reference/memory-model.html#bytes
/// [provenance]: crate::ptr#provenance
#[stable(feature = "maybe_uninit", since = "1.36.0")]
// 作为 lang item，这样我们就能把其他类型包装进它。这对协程（coroutine）很有用。
#[lang = "maybe_uninit"]
#[derive(Copy)]
#[repr(transparent)]
#[rustc_pub_transparent]
pub union MaybeUninit<T> {
    uninit: (),
    value: ManuallyDrop<T>,
}

#[stable(feature = "maybe_uninit", since = "1.36.0")]
impl<T: Copy> Clone for MaybeUninit<T> {
    #[inline(always)]
    fn clone(&self) -> Self {
        // 没有调用 `T::clone()`，因为我们无法知道自己是否已初始化到足以那样做的程度。
        *self
    }
}

// SAFETY: 这个 clone 实现就是一次 copy，见上文。
#[doc(hidden)]
#[unstable(feature = "trivial_clone", issue = "none")]
unsafe impl<T> TrivialClone for MaybeUninit<T> where MaybeUninit<T>: Clone {}

#[stable(feature = "maybe_uninit_debug", since = "1.41.0")]
impl<T> fmt::Debug for MaybeUninit<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // 注意：没有 `.pad_fmt`，所以我们无法使用更简单的 `format_args!("MaybeUninit<{..}>")`。
        let full_name = type_name::<Self>();
        let prefix_len = full_name.find("MaybeUninit").unwrap();
        f.pad(&full_name[prefix_len..])
    }
}

impl<T> MaybeUninit<T> {
    /// 创建一个用给定值初始化的新 `MaybeUninit<T>`。
    /// 对此函数的返回值调用 [`assume_init`] 是安全的。
    ///
    /// 注意，drop 一个 `MaybeUninit<T>` 永远不会调用 `T` 的 drop 代码。如果 `T` 被初始化过，
    /// 确保它被 drop 是你的责任。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::mem::MaybeUninit;
    ///
    /// let v: MaybeUninit<Vec<u8>> = MaybeUninit::new(vec![42]);
    /// # // 为 Miri 防止泄漏
    /// # unsafe { let _ = MaybeUninit::assume_init(v); }
    /// ```
    ///
    /// [`assume_init`]: MaybeUninit::assume_init
    #[stable(feature = "maybe_uninit", since = "1.36.0")]
    #[rustc_const_stable(feature = "const_maybe_uninit", since = "1.36.0")]
    #[must_use = "use `forget` to avoid running Drop code"]
    #[inline(always)]
    pub const fn new(val: T) -> MaybeUninit<T> {
        MaybeUninit { value: ManuallyDrop::new(val) }
    }

    /// 创建一个处于未初始化状态的新 `MaybeUninit<T>`。
    ///
    /// 注意，drop 一个 `MaybeUninit<T>` 永远不会调用 `T` 的 drop 代码。如果 `T` 被初始化过，
    /// 确保它被 drop 是你的责任。
    ///
    /// 一些示例参见[类型级文档][MaybeUninit]。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::mem::MaybeUninit;
    ///
    /// let v: MaybeUninit<String> = MaybeUninit::uninit();
    /// ```
    #[stable(feature = "maybe_uninit", since = "1.36.0")]
    #[rustc_const_stable(feature = "const_maybe_uninit", since = "1.36.0")]
    #[must_use]
    #[inline(always)]
    #[rustc_diagnostic_item = "maybe_uninit_uninit"]
    pub const fn uninit() -> MaybeUninit<T> {
        MaybeUninit { uninit: () }
    }

    /// 创建一个处于未初始化状态、内存被填满 `0` 字节的新 `MaybeUninit<T>`。这是否已经构成正确的
    /// 初始化，取决于 `T`。例如，`MaybeUninit<usize>::zeroed()` 是已初始化的，而
    /// `MaybeUninit<&'static i32>::zeroed()` 则不是，因为引用不能为空。
    ///
    /// 注意，如果 `T` 有填充字节（padding byte），那么当此函数返回该 `MaybeUninit<T>` 值时，那些
    /// 字节*不会*被保留，所以那些字节*不会*被置零。
    ///
    /// 注意，drop 一个 `MaybeUninit<T>` 永远不会调用 `T` 的 drop 代码。如果 `T` 被初始化过，
    /// 确保它被 drop 是你的责任。
    ///
    /// # 示例
    ///
    /// 此函数的正确用法：用零初始化一个结构体，且该结构体的所有字段都能把位模式 0 当作有效值持有。
    ///
    /// ```rust
    /// use std::mem::MaybeUninit;
    ///
    /// let x = MaybeUninit::<(u8, bool)>::zeroed();
    /// let x = unsafe { x.assume_init() };
    /// assert_eq!(x, (0, false));
    /// ```
    ///
    /// 它可以用在 const 上下文中，例如用来标记插件注册中静态数组的末尾。
    ///
    /// 此函数的*错误*用法：当 `0` 不是该类型的有效位模式时，调用 `x.zeroed().assume_init()`：
    ///
    /// ```rust,no_run
    /// use std::mem::MaybeUninit;
    ///
    /// enum NotZero { One = 1, Two = 2 }
    ///
    /// let x = MaybeUninit::<(u8, NotZero)>::zeroed();
    /// let x = unsafe { x.assume_init() };
    /// // 在一个 pair 内部，我们创建了一个没有有效判别值（discriminant）的 `NotZero`。
    /// // 这是未定义行为。⚠️
    /// ```
    #[inline]
    #[must_use]
    #[rustc_diagnostic_item = "maybe_uninit_zeroed"]
    #[stable(feature = "maybe_uninit", since = "1.36.0")]
    #[rustc_const_stable(feature = "const_maybe_uninit_zeroed", since = "1.75.0")]
    pub const fn zeroed() -> MaybeUninit<T> {
        let mut u = MaybeUninit::<T>::uninit();
        // SAFETY: `u.as_mut_ptr()` 指向已分配的内存。
        unsafe { u.as_mut_ptr().write_bytes(0u8, 1) };
        u
    }

    /// 设置 `MaybeUninit<T>` 的值。
    ///
    /// 这会覆盖任何先前的值且不会 drop 它，所以要小心不要把它用两次，除非你确实想跳过运行析构逻辑。
    /// 为了方便，这还会返回一个指向 `self`（现已被安全地初始化）内容的可变引用。
    ///
    /// 由于内容存储在一个 `ManuallyDrop` 内部，所以如果该 MaybeUninit 在没有调用 [`assume_init`]、
    /// [`assume_init_drop`] 或类似方法的情况下离开作用域，其内部数据的析构逻辑不会被运行。接收此
    /// 函数所返回可变引用的代码需要牢记这一点。Rust 的安全模型把泄漏视作安全，但它们通常仍然是
    /// 不可取的。话虽如此，这个可变引用的行为与任何其他可变引用别无二致，所以给它赋一个新值会
    /// drop 旧的内容。
    ///
    /// [`assume_init`]: Self::assume_init
    /// [`assume_init_drop`]: Self::assume_init_drop
    ///
    /// # 示例
    ///
    /// 此方法的正确用法：
    ///
    /// ```rust
    /// use std::mem::MaybeUninit;
    ///
    /// let mut x = MaybeUninit::<Vec<u8>>::uninit();
    ///
    /// {
    ///     let hello = x.write((&b"Hello, world!").to_vec());
    ///     // 设置 hello 不会泄漏先前的分配，而是会 drop 它们
    ///     *hello = (&b"Hello").to_vec();
    ///     hello[0] = 'h' as u8;
    /// }
    /// // 现在 x 已被初始化：
    /// let s = unsafe { x.assume_init() };
    /// assert_eq!(b"hello", s.as_slice());
    /// ```
    ///
    /// 此方法的这种用法会导致泄漏：
    ///
    /// ```rust
    /// use std::mem::MaybeUninit;
    ///
    /// let mut x = MaybeUninit::<String>::uninit();
    ///
    /// x.write("Hello".to_string());
    /// # // FIXME(https://github.com/rust-lang/miri/issues/3670):
    /// # // 在意在泄漏的测试中，使用 -Zmiri-disable-leak-check 而不是手动取出避免泄漏。
    /// # unsafe { MaybeUninit::assume_init_drop(&mut x); }
    /// // 这会泄漏所含的那个 string：
    /// x.write("hello".to_string());
    /// // 现在 x 已被初始化：
    /// let s = unsafe { x.assume_init() };
    /// ```
    ///
    /// 此方法在某些情况下可以用来避免 unsafe。下面的例子展示了一个定长 arena 实现的一部分，
    /// 它会借出被 pin 的引用。借助 `write`，我们可以避免去通过裸指针进行写入：
    ///
    /// ```rust
    /// use core::pin::Pin;
    /// use core::mem::MaybeUninit;
    ///
    /// struct PinArena<T> {
    ///     memory: Box<[MaybeUninit<T>]>,
    ///     len: usize,
    /// }
    ///
    /// impl <T> PinArena<T> {
    ///     pub fn capacity(&self) -> usize {
    ///         self.memory.len()
    ///     }
    ///     pub fn push(&mut self, val: T) -> Pin<&mut T> {
    ///         if self.len >= self.capacity() {
    ///             panic!("Attempted to push to a full pin arena!");
    ///         }
    ///         let ref_ = self.memory[self.len].write(val);
    ///         self.len += 1;
    ///         unsafe { Pin::new_unchecked(ref_) }
    ///     }
    /// }
    /// ```
    #[inline(always)]
    #[stable(feature = "maybe_uninit_write", since = "1.55.0")]
    #[rustc_const_stable(feature = "const_maybe_uninit_write", since = "1.85.0")]
    pub const fn write(&mut self, val: T) -> &mut T {
        *self = MaybeUninit::new(val);
        // SAFETY: 我们刚刚初始化了这个值。
        unsafe { self.assume_init_mut() }
    }

    /// 获取一个指向所含值的指针。除非该 `MaybeUninit<T>` 已被初始化，否则从此指针读取或把它变成
    /// 一个引用都是未定义行为。向此指针（非传递地，non-transitively）所指向的内存写入也是未定义
    /// 行为（在 `UnsafeCell<T>` 内部时除外）。
    ///
    /// # 示例
    ///
    /// 此方法的正确用法：
    ///
    /// ```rust
    /// use std::mem::MaybeUninit;
    ///
    /// let mut x = MaybeUninit::<Vec<u32>>::uninit();
    /// x.write(vec![0, 1, 2]);
    /// // 创建一个指向该 `MaybeUninit<T>` 内部的引用。这没问题，因为我们已经初始化了它。
    /// let x_vec = unsafe { &*x.as_ptr() };
    /// assert_eq!(x_vec.len(), 3);
    /// # // 为 Miri 防止泄漏
    /// # unsafe { MaybeUninit::assume_init_drop(&mut x); }
    /// ```
    ///
    /// 此方法的*错误*用法：
    ///
    /// ```rust,no_run
    /// use std::mem::MaybeUninit;
    ///
    /// let x = MaybeUninit::<Vec<u32>>::uninit();
    /// let x_vec = unsafe { &*x.as_ptr() };
    /// // 我们创建了一个指向未初始化 vector 的引用！这是未定义行为。⚠️
    /// ```
    ///
    ///（注意，围绕“指向未初始化数据的引用”的规则尚未敲定，但在它们敲定之前，建议避免使用这类引用。）
    #[stable(feature = "maybe_uninit", since = "1.36.0")]
    #[rustc_const_stable(feature = "const_maybe_uninit_as_ptr", since = "1.59.0")]
    #[rustc_as_ptr]
    #[inline(always)]
    pub const fn as_ptr(&self) -> *const T {
        // `MaybeUninit` 与 `ManuallyDrop` 都是 `repr(transparent)`，因此我们可以转换（cast）指针。
        self as *const _ as *const T
    }

    /// 获取一个指向所含值的可变指针。除非该 `MaybeUninit<T>` 已被初始化，否则从此指针读取或把它
    /// 变成一个引用都是未定义行为。
    ///
    /// # 示例
    ///
    /// 此方法的正确用法：
    ///
    /// ```rust
    /// use std::mem::MaybeUninit;
    ///
    /// let mut x = MaybeUninit::<Vec<u32>>::uninit();
    /// x.write(vec![0, 1, 2]);
    /// // 创建一个指向该 `MaybeUninit<Vec<u32>>` 内部的引用。
    /// // 这没问题，因为我们已经初始化了它。
    /// let x_vec = unsafe { &mut *x.as_mut_ptr() };
    /// x_vec.push(3);
    /// assert_eq!(x_vec.len(), 4);
    /// # // 为 Miri 防止泄漏
    /// # unsafe { MaybeUninit::assume_init_drop(&mut x); }
    /// ```
    ///
    /// 此方法的*错误*用法：
    ///
    /// ```rust,no_run
    /// use std::mem::MaybeUninit;
    ///
    /// let mut x = MaybeUninit::<Vec<u32>>::uninit();
    /// let x_vec = unsafe { &mut *x.as_mut_ptr() };
    /// // 我们创建了一个指向未初始化 vector 的引用！这是未定义行为。⚠️
    /// ```
    ///
    ///（注意，围绕“指向未初始化数据的引用”的规则尚未敲定，但在它们敲定之前，建议避免使用这类引用。）
    #[stable(feature = "maybe_uninit", since = "1.36.0")]
    #[rustc_const_stable(feature = "const_maybe_uninit_as_mut_ptr", since = "1.83.0")]
    #[rustc_as_ptr]
    #[inline(always)]
    pub const fn as_mut_ptr(&mut self) -> *mut T {
        // `MaybeUninit` 与 `ManuallyDrop` 都是 `repr(transparent)`，因此我们可以转换（cast）指针。
        self as *mut _ as *mut T
    }

    /// 从 `MaybeUninit<T>` 容器中提取出值。这是确保数据会被 drop 的一个很好的方式，因为得到的
    /// `T` 会接受通常的 drop 处理。
    ///
    /// # 安全性（Safety）
    ///
    /// 保证该 `MaybeUninit<T>` 确实处于已初始化状态，是调用方的责任。在内容尚未完全初始化时调用
    /// 它会立即导致未定义行为。关于这个初始化不变量的更多信息，参见[类型级文档][inv]。
    ///
    /// [inv]: #initialization-invariant
    ///
    /// 在此之上，请记住大多数类型除了在类型层面被视作已初始化之外，还有额外的不变量。例如，一个被
    /// 初始化为 `1` 的 [`Vec<T>`] 被视为已初始化（在当前实现下；这并不构成稳定保证），因为编译器
    /// 对它所知道的唯一要求就是其数据指针必须非空。创建这样一个 `Vec<T>` 不会*立即*导致未定义行为，
    /// 但在进行大多数安全操作时（包括 drop 它）会导致未定义行为。
    ///
    /// [`Vec<T>`]: ../../std/vec/struct.Vec.html
    ///
    /// # 示例
    ///
    /// 此方法的正确用法：
    ///
    /// ```rust
    /// use std::mem::MaybeUninit;
    ///
    /// let mut x = MaybeUninit::<bool>::uninit();
    /// x.write(true);
    /// let x_init = unsafe { x.assume_init() };
    /// assert_eq!(x_init, true);
    /// ```
    ///
    /// 此方法的*错误*用法：
    ///
    /// ```rust,no_run
    /// use std::mem::MaybeUninit;
    ///
    /// let x = MaybeUninit::<Vec<u32>>::uninit();
    /// let x_init = unsafe { x.assume_init() };
    /// // `x` 此前尚未被初始化，所以最后这一行导致了未定义行为。⚠️
    /// ```
    #[stable(feature = "maybe_uninit", since = "1.36.0")]
    #[rustc_const_stable(feature = "const_maybe_uninit_assume_init_by_value", since = "1.59.0")]
    #[inline(always)]
    #[rustc_diagnostic_item = "assume_init"]
    #[track_caller]
    pub const unsafe fn assume_init(self) -> T {
        // SAFETY: 调用方必须保证 `self` 已被初始化。
        // 这也意味着 `self` 必须是 `value` 那个变体。
        unsafe {
            intrinsics::assert_inhabited::<T>();
            // 我们通过裸指针读取而不是 `ManuallyDrop::into_inner` 来做这件事，这样在这里 Miri 的
            // 错误信息中就不会出现 `ManuallyDrop` 的痕迹。
            (&raw const self.value).cast::<T>().read()
        }
    }

    /// 从 `MaybeUninit<T>` 容器中读取值。得到的 `T` 会接受通常的 drop 处理。
    ///
    /// 只要有可能，就更应该改用 [`assume_init`]，它能避免重复（duplicate）`MaybeUninit<T>` 的内容。
    ///
    /// # 安全性（Safety）
    ///
    /// 保证该 `MaybeUninit<T>` 确实处于已初始化状态，是调用方的责任。在内容尚未完全初始化时调用
    /// 它会导致未定义行为。关于这个初始化不变量的更多信息，参见[类型级文档][inv]。
    ///
    /// 此外，与 [`ptr::read`] 函数类似，此函数会创建内容的一份按位（bitwise）副本，无论所含类型
    /// 是否实现了 [`Copy`] trait。当使用该数据的多个副本时（无论是多次调用 `assume_init_read`，
    /// 还是先调用 `assume_init_read` 再调用 [`assume_init`]），确保该数据确实可以被复制
    ///（duplicate）是你的责任。
    ///
    /// [inv]: #initialization-invariant
    /// [`assume_init`]: MaybeUninit::assume_init
    ///
    /// # 示例
    ///
    /// 此方法的正确用法：
    ///
    /// ```rust
    /// use std::mem::MaybeUninit;
    ///
    /// let mut x = MaybeUninit::<u32>::uninit();
    /// x.write(13);
    /// let x1 = unsafe { x.assume_init_read() };
    /// // `u32` 是 `Copy`，所以我们可以多次读取。
    /// let x2 = unsafe { x.assume_init_read() };
    /// assert_eq!(x1, x2);
    ///
    /// let mut x = MaybeUninit::<Option<Vec<u32>>>::uninit();
    /// x.write(None);
    /// let x1 = unsafe { x.assume_init_read() };
    /// // 复制一个 `None` 值没问题，所以我们可以多次读取。
    /// let x2 = unsafe { x.assume_init_read() };
    /// assert_eq!(x1, x2);
    /// ```
    ///
    /// 此方法的*错误*用法：
    ///
    /// ```rust,no_run
    /// use std::mem::MaybeUninit;
    ///
    /// let mut x = MaybeUninit::<Option<Vec<u32>>>::uninit();
    /// x.write(Some(vec![0, 1, 2]));
    /// let x1 = unsafe { x.assume_init_read() };
    /// let x2 = unsafe { x.assume_init_read() };
    /// // 我们现在创建了同一个 vector 的两份副本，当它们都被 drop 时会导致 ⚠️ double-free！
    /// ```
    #[stable(feature = "maybe_uninit_extra", since = "1.60.0")]
    #[rustc_const_stable(feature = "const_maybe_uninit_assume_init_read", since = "1.75.0")]
    #[inline(always)]
    #[track_caller]
    pub const unsafe fn assume_init_read(&self) -> T {
        // SAFETY: 调用方必须保证 `self` 已被初始化。
        // 由于 `self` 应当已被初始化，从 `self.as_ptr()` 读取是安全的。
        unsafe {
            intrinsics::assert_inhabited::<T>();
            self.as_ptr().read()
        }
    }

    /// 就地（in place）drop 所含的值。
    ///
    /// 如果你拥有该 `MaybeUninit` 的所有权，也可以改用 [`assume_init`] 作为替代。
    ///
    /// # 安全性（Safety）
    ///
    /// 保证该 `MaybeUninit<T>` 确实处于已初始化状态，是调用方的责任。在内容尚未完全初始化时调用
    /// 它会导致未定义行为。
    ///
    /// 在此之上，类型 `T` 的所有额外不变量都必须得到满足，因为 `T`（或其成员）的 `Drop` 实现可能
    /// 依赖于此。例如，把一个 `Vec<T>` 设置为一个无效但非空的地址会使它变为已初始化（在当前实现
    /// 下；这并不构成稳定保证），因为编译器对它所知道的唯一要求就是其数据指针必须非空。然而，
    /// drop 这样一个 `Vec<T>` 会导致未定义行为。
    ///
    /// [`assume_init`]: MaybeUninit::assume_init
    #[stable(feature = "maybe_uninit_extra", since = "1.60.0")]
    #[rustc_const_unstable(feature = "const_drop_in_place", issue = "109342")]
    pub const unsafe fn assume_init_drop(&mut self)
    where
        T: [const] Destruct,
    {
        // SAFETY: 调用方必须保证 `self` 已被初始化、且满足 `T` 的所有不变量。
        // 在这种情况下，就地 drop 这个值是安全的。
        unsafe { ptr::drop_in_place(self.as_mut_ptr()) }
    }

    /// 获取一个指向所含值的共享引用。
    ///
    /// 当我们想访问一个已被初始化、但我们并不拥有其所有权的 `MaybeUninit`（这就无法使用
    /// `.assume_init()`）时，这会很有用。
    ///
    /// # 安全性（Safety）
    ///
    /// 在内容尚未完全初始化时调用它会导致未定义行为：保证该 `MaybeUninit<T>` 确实处于已初始化
    /// 状态，是调用方的责任。
    ///
    /// # 示例
    ///
    /// ### 此方法的正确用法：
    ///
    /// ```rust
    /// use std::mem::MaybeUninit;
    ///
    /// let mut x = MaybeUninit::<Vec<u32>>::uninit();
    /// # let mut x_mu = x;
    /// # let mut x = &mut x_mu;
    /// // 初始化 `x`：
    /// x.write(vec![1, 2, 3]);
    /// // 既然我们的 `MaybeUninit<_>` 已知被初始化，那么创建一个指向它的共享引用就没问题了：
    /// let x: &Vec<u32> = unsafe {
    ///     // SAFETY: `x` 已被初始化。
    ///     x.assume_init_ref()
    /// };
    /// assert_eq!(x, &vec![1, 2, 3]);
    /// # // 为 Miri 防止泄漏
    /// # unsafe { MaybeUninit::assume_init_drop(&mut x_mu); }
    /// ```
    ///
    /// ### 此方法的*错误*用法：
    ///
    /// ```rust,no_run
    /// use std::mem::MaybeUninit;
    ///
    /// let x = MaybeUninit::<Vec<u32>>::uninit();
    /// let x_vec: &Vec<u32> = unsafe { x.assume_init_ref() };
    /// // 我们创建了一个指向未初始化 vector 的引用！这是未定义行为。⚠️
    /// ```
    ///
    /// ```rust,no_run
    /// use std::{cell::Cell, mem::MaybeUninit};
    ///
    /// let b = MaybeUninit::<Cell<bool>>::uninit();
    /// // 使用 `Cell::set` 来初始化该 `MaybeUninit`：
    /// unsafe {
    ///     b.assume_init_ref().set(true);
    ///     //^^^^^^^^^^^^^^^ 指向未初始化 `Cell<bool>` 的引用：UB！
    /// }
    /// ```
    #[stable(feature = "maybe_uninit_ref", since = "1.55.0")]
    #[rustc_const_stable(feature = "const_maybe_uninit_assume_init_ref", since = "1.59.0")]
    #[inline(always)]
    pub const unsafe fn assume_init_ref(&self) -> &T {
        // SAFETY: 调用方必须保证 `self` 已被初始化。
        // 这也意味着 `self` 必须是 `value` 那个变体。
        unsafe {
            intrinsics::assert_inhabited::<T>();
            &*self.as_ptr()
        }
    }

    /// 获取一个指向所含值的可变（唯一）引用。
    ///
    /// 当我们想访问一个已被初始化、但我们并不拥有其所有权的 `MaybeUninit`（这就无法使用
    /// `.assume_init()`）时，这会很有用。
    ///
    /// # 安全性（Safety）
    ///
    /// 在内容尚未完全初始化时调用它会导致未定义行为：保证该 `MaybeUninit<T>` 确实处于已初始化
    /// 状态，是调用方的责任。例如，`.assume_init_mut()` 不能被用来初始化一个 `MaybeUninit`。
    ///
    /// # 示例
    ///
    /// ### 此方法的正确用法：
    ///
    /// ```rust
    /// # #![allow(unexpected_cfgs)]
    /// use std::mem::MaybeUninit;
    ///
    /// # unsafe extern "C" fn initialize_buffer(buf: *mut [u8; 1024]) { unsafe { *buf = [0; 1024] } }
    /// # #[cfg(FALSE)]
    /// extern "C" {
    ///     /// 初始化输入缓冲区的*所有*字节。
    ///     fn initialize_buffer(buf: *mut [u8; 1024]);
    /// }
    ///
    /// let mut buf = MaybeUninit::<[u8; 1024]>::uninit();
    ///
    /// // 初始化 `buf`：
    /// unsafe { initialize_buffer(buf.as_mut_ptr()); }
    /// // 现在我们知道 `buf` 已被初始化，所以我们本可以对它 `.assume_init()`。
    /// // 然而，使用 `.assume_init()` 可能触发对这 1024 个字节的一次 `memcpy`。
    /// // 为了在不复制 buffer 的前提下断言它已被初始化，我们把
    /// // `&mut MaybeUninit<[u8; 1024]>` 升级为 `&mut [u8; 1024]`：
    /// let buf: &mut [u8; 1024] = unsafe {
    ///     // SAFETY: `buf` 已被初始化。
    ///     buf.assume_init_mut()
    /// };
    ///
    /// // 现在我们可以把 `buf` 当作一个普通切片来使用：
    /// buf.sort_unstable();
    /// assert!(
    ///     buf.windows(2).all(|pair| pair[0] <= pair[1]),
    ///     "buffer is sorted",
    /// );
    /// ```
    ///
    /// ### 此方法的*错误*用法：
    ///
    /// 你不能用 `.assume_init_mut()` 来初始化一个值：
    ///
    /// ```rust,no_run
    /// use std::mem::MaybeUninit;
    ///
    /// let mut b = MaybeUninit::<bool>::uninit();
    /// unsafe {
    ///     *b.assume_init_mut() = true;
    ///     // 我们创建了一个指向未初始化 `bool` 的（可变）引用！
    ///     // 这是未定义行为。⚠️
    /// }
    /// ```
    ///
    /// 例如，你不能 [`Read`] 进一个未初始化的 buffer：
    ///
    /// [`Read`]: ../../std/io/trait.Read.html
    ///
    /// ```rust,no_run
    /// use std::{io, mem::MaybeUninit};
    ///
    /// fn read_chunk (reader: &'_ mut dyn io::Read) -> io::Result<[u8; 64]>
    /// {
    ///     let mut buffer = MaybeUninit::<[u8; 64]>::uninit();
    ///     reader.read_exact(unsafe { buffer.assume_init_mut() })?;
    ///     //                         ^^^^^^^^^^^^^^^^^^^^^^^^
    ///     // 指向未初始化内存的（可变）引用！
    ///     // 这是未定义行为。
    ///     Ok(unsafe { buffer.assume_init() })
    /// }
    /// ```
    ///
    /// 你也不能用直接的字段访问来做逐字段的渐进式初始化：
    ///
    /// ```rust,no_run
    /// use std::{mem::MaybeUninit, ptr};
    ///
    /// struct Foo {
    ///     a: u32,
    ///     b: u8,
    /// }
    ///
    /// let foo: Foo = unsafe {
    ///     let mut foo = MaybeUninit::<Foo>::uninit();
    ///     ptr::write(&mut foo.assume_init_mut().a as *mut u32, 1337);
    ///     //              ^^^^^^^^^^^^^^^^^^^^^
    ///     // 指向未初始化内存的（可变）引用！
    ///     // 这是未定义行为。
    ///     ptr::write(&mut foo.assume_init_mut().b as *mut u8, 42);
    ///     //              ^^^^^^^^^^^^^^^^^^^^^
    ///     // 指向未初始化内存的（可变）引用！
    ///     // 这是未定义行为。
    ///     foo.assume_init()
    /// };
    /// ```
    #[stable(feature = "maybe_uninit_ref", since = "1.55.0")]
    #[rustc_const_stable(feature = "const_maybe_uninit_assume_init", since = "1.84.0")]
    #[inline(always)]
    pub const unsafe fn assume_init_mut(&mut self) -> &mut T {
        // SAFETY: 调用方必须保证 `self` 已被初始化。
        // 这也意味着 `self` 必须是 `value` 那个变体。
        unsafe {
            intrinsics::assert_inhabited::<T>();
            &mut *self.as_mut_ptr()
        }
    }

    /// 从一个由 `MaybeUninit` 容器组成的数组中提取出各个值。
    ///
    /// # 安全性（Safety）
    ///
    /// 保证该数组的所有元素都处于已初始化状态，是调用方的责任。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(maybe_uninit_array_assume_init)]
    /// use std::mem::MaybeUninit;
    ///
    /// let mut array: [MaybeUninit<i32>; 3] = [MaybeUninit::uninit(); 3];
    /// array[0].write(0);
    /// array[1].write(1);
    /// array[2].write(2);
    ///
    /// // SAFETY: 既然我们已初始化了所有元素，现在就是安全的
    /// let array = unsafe {
    ///     MaybeUninit::array_assume_init(array)
    /// };
    ///
    /// assert_eq!(array, [0, 1, 2]);
    /// ```
    #[unstable(feature = "maybe_uninit_array_assume_init", issue = "96097")]
    #[inline(always)]
    #[track_caller]
    pub const unsafe fn array_assume_init<const N: usize>(array: [Self; N]) -> [T; N] {
        // SAFETY:
        // * 调用方保证该数组的所有元素都已初始化
        // * `MaybeUninit<T>` 与 T 保证拥有相同的布局
        // * `MaybeUninit` 不会 drop，因此不存在 double-free
        // 因此这次转换是安全的
        unsafe {
            intrinsics::assert_inhabited::<[T; N]>();
            intrinsics::transmute_unchecked(array)
        }
    }

    /// 把此 `MaybeUninit` 的内容作为一个由“可能未初始化的字节”组成的切片返回。
    ///
    /// 注意，即便某个 `MaybeUninit` 的内容已被初始化，该值仍可能含有保持未初始化状态的填充字节。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(maybe_uninit_as_bytes)]
    /// use std::mem::MaybeUninit;
    ///
    /// let val = 0x12345678_i32;
    /// let uninit = MaybeUninit::new(val);
    /// let uninit_bytes = uninit.as_bytes();
    /// let bytes = unsafe { uninit_bytes.assume_init_ref() };
    /// assert_eq!(bytes, val.to_ne_bytes());
    /// ```
    #[unstable(feature = "maybe_uninit_as_bytes", issue = "93092")]
    pub const fn as_bytes(&self) -> &[MaybeUninit<u8>] {
        // SAFETY: MaybeUninit<u8> 总是有效的，即便对填充字节也是如此
        unsafe {
            slice::from_raw_parts(self.as_ptr().cast::<MaybeUninit<u8>>(), super::size_of::<T>())
        }
    }

    /// 把此 `MaybeUninit` 的内容作为一个由“可能未初始化的字节”组成的可变切片返回。
    ///
    /// 注意，即便某个 `MaybeUninit` 的内容已被初始化，该值仍可能含有保持未初始化状态的填充字节。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(maybe_uninit_as_bytes)]
    /// use std::mem::MaybeUninit;
    ///
    /// let val = 0x12345678_i32;
    /// let mut uninit = MaybeUninit::new(val);
    /// let uninit_bytes = uninit.as_bytes_mut();
    /// if cfg!(target_endian = "little") {
    ///     uninit_bytes[0].write(0xcd);
    /// } else {
    ///     uninit_bytes[3].write(0xcd);
    /// }
    /// let val2 = unsafe { uninit.assume_init() };
    /// assert_eq!(val2, 0x123456cd);
    /// ```
    #[unstable(feature = "maybe_uninit_as_bytes", issue = "93092")]
    pub const fn as_bytes_mut(&mut self) -> &mut [MaybeUninit<u8>] {
        // SAFETY: MaybeUninit<u8> 总是有效的，即便对填充字节也是如此
        unsafe {
            slice::from_raw_parts_mut(
                self.as_mut_ptr().cast::<MaybeUninit<u8>>(),
                super::size_of::<T>(),
            )
        }
    }
}

impl<T> [MaybeUninit<T>] {
    /// 把元素从 `src` 复制（copy）到 `self`，并返回一个指向 `self` 现已初始化内容的可变引用。
    ///
    /// 如果 `T` 没有实现 `Copy`，请改用 [`write_clone_of_slice`]。
    ///
    /// 这与 [`slice::copy_from_slice`] 类似。
    ///
    /// # Panics
    ///
    /// 如果两个切片长度不同，此函数会 panic。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::mem::MaybeUninit;
    ///
    /// let mut dst = [MaybeUninit::uninit(); 32];
    /// let src = [0; 32];
    ///
    /// let init = dst.write_copy_of_slice(&src);
    ///
    /// assert_eq!(init, src);
    /// ```
    ///
    /// ```
    /// let mut vec = Vec::with_capacity(32);
    /// let src = [0; 16];
    ///
    /// vec.spare_capacity_mut()[..src.len()].write_copy_of_slice(&src);
    ///
    /// // SAFETY: 我们刚刚把 len 个元素全部复制进了空余容量（spare capacity）中
    /// // vec 的前 src.len() 个元素现在都是有效的。
    /// unsafe {
    ///     vec.set_len(src.len());
    /// }
    ///
    /// assert_eq!(vec, src);
    /// ```
    ///
    /// [`write_clone_of_slice`]: slice::write_clone_of_slice
    #[stable(feature = "maybe_uninit_write_slice", since = "1.93.0")]
    #[rustc_const_stable(feature = "maybe_uninit_write_slice", since = "1.93.0")]
    pub const fn write_copy_of_slice(&mut self, src: &[T]) -> &mut [T]
    where
        T: Copy,
    {
        // SAFETY: &[T] 与 &[MaybeUninit<T>] 拥有相同的布局
        let uninit_src: &[MaybeUninit<T>] = unsafe { super::transmute(src) };

        self.copy_from_slice(uninit_src);

        // SAFETY: 有效的元素刚刚被复制进了 `self`，所以它已被初始化
        unsafe { self.assume_init_mut() }
    }

    /// 把元素从 `src` 克隆（clone）到 `self`，并返回一个指向 `self` 现已初始化内容的可变引用。
    /// 任何已经初始化过的元素都不会被 drop。
    ///
    /// 如果 `T` 实现了 `Copy`，请改用 [`write_copy_of_slice`]。
    ///
    /// 这与 [`slice::clone_from_slice`] 类似，但不会 drop 已有的元素。
    ///
    /// # Panics
    ///
    /// 如果两个切片长度不同，或者 `Clone` 的实现发生 panic，此函数会 panic。
    ///
    /// 如果发生 panic，已经克隆出的元素会被 drop。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::mem::MaybeUninit;
    ///
    /// let mut dst = [const { MaybeUninit::uninit() }; 5];
    /// let src = ["wibbly", "wobbly", "timey", "wimey", "stuff"].map(|s| s.to_string());
    ///
    /// let init = dst.write_clone_of_slice(&src);
    ///
    /// assert_eq!(init, src);
    ///
    /// # // 为 Miri 防止泄漏
    /// # unsafe { std::ptr::drop_in_place(init); }
    /// ```
    ///
    /// ```
    /// let mut vec = Vec::with_capacity(32);
    /// let src = ["rust", "is", "a", "pretty", "cool", "language"].map(|s| s.to_string());
    ///
    /// vec.spare_capacity_mut()[..src.len()].write_clone_of_slice(&src);
    ///
    /// // SAFETY: 我们刚刚把 len 个元素全部克隆进了空余容量（spare capacity）中
    /// // vec 的前 src.len() 个元素现在都是有效的。
    /// unsafe {
    ///     vec.set_len(src.len());
    /// }
    ///
    /// assert_eq!(vec, src);
    /// ```
    ///
    /// [`write_copy_of_slice`]: slice::write_copy_of_slice
    #[stable(feature = "maybe_uninit_write_slice", since = "1.93.0")]
    pub fn write_clone_of_slice(&mut self, src: &[T]) -> &mut [T]
    where
        T: Clone,
    {
        // 与 copy_from_slice 不同，这里不会对该切片调用 clone_from_slice
        // 这是因为 `MaybeUninit<T: Clone>` 并没有实现 Clone。

        assert_eq!(self.len(), src.len(), "destination and source slices have different lengths");

        // 注意：我们需要显式地把它们切到相同长度，以便边界检查（bounds checking）被省略，
        // 这样优化器在简单情形下（例如 T = u8）会生成 memcpy。
        let len = self.len();
        let src = &src[..len];

        // 需要 guard，因为在 clone 过程中可能发生 panic
        let mut guard = Guard { slice: self, initialized: 0 };

        for i in 0..len {
            guard.slice[i].write(src[i].clone());
            guard.initialized += 1;
        }

        super::forget(guard);

        // SAFETY: 有效的元素刚刚被写入了 `self`，所以它已被初始化
        unsafe { self.assume_init_mut() }
    }

    /// 通过克隆 `value` 来用元素填充一个切片，并返回一个指向该切片现已初始化内容的可变引用。
    /// 任何先前已初始化的元素都不会被 drop。
    ///
    /// 这与 [`slice::fill`] 类似。
    ///
    /// # Panics
    ///
    /// 如果任何一次对 `Clone` 的调用发生 panic，此函数会 panic。
    ///
    /// 如果发生这样的 panic，本次操作中先前已初始化的任何元素都会被 drop。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(maybe_uninit_fill)]
    /// use std::mem::MaybeUninit;
    ///
    /// let mut buf = [const { MaybeUninit::uninit() }; 10];
    /// let initialized = buf.write_filled(1);
    /// assert_eq!(initialized, &mut [1; 10]);
    /// ```
    #[doc(alias = "memset")]
    #[unstable(feature = "maybe_uninit_fill", issue = "117428")]
    pub fn write_filled(&mut self, value: T) -> &mut [T]
    where
        T: Clone,
    {
        SpecFill::spec_fill(self, value);
        // SAFETY: 有效的元素刚刚被填充进了 `self`，所以它已被初始化
        unsafe { self.assume_init_mut() }
    }

    /// 通过为每个下标调用一个闭包来用其返回的元素填充一个切片。
    ///
    /// 此方法使用一个闭包来创建新值。如果你更想 `Clone` 某个给定的值，请使用
    /// [slice::write_filled]。如果你想用 `Default` trait 来生成值，可以把
    /// [`|_| Default::default()`][Default::default] 作为参数传入。
    ///
    /// # Panics
    ///
    /// 如果任何一次对所提供闭包的调用发生 panic，此函数会 panic。
    ///
    /// 如果发生这样的 panic，本次操作中先前已初始化的任何元素都会被 drop。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(maybe_uninit_fill)]
    /// use std::mem::MaybeUninit;
    ///
    /// let mut buf = [const { MaybeUninit::<usize>::uninit() }; 5];
    /// let initialized = buf.write_with(|idx| idx + 1);
    /// assert_eq!(initialized, &mut [1, 2, 3, 4, 5]);
    /// ```
    #[unstable(feature = "maybe_uninit_fill", issue = "117428")]
    pub fn write_with<F>(&mut self, mut f: F) -> &mut [T]
    where
        F: FnMut(usize) -> T,
    {
        let mut guard = Guard { slice: self, initialized: 0 };

        for (idx, element) in guard.slice.iter_mut().enumerate() {
            element.write(f(idx));
            guard.initialized += 1;
        }

        super::forget(guard);

        // SAFETY: 有效的元素刚刚被写入了 `this`，所以它已被初始化
        unsafe { self.assume_init_mut() }
    }

    /// 用一个迭代器产出的元素填充一个切片，直到所有元素都被初始化、或者该迭代器为空为止。
    ///
    /// 返回两个切片。第一个切片包含原切片中已初始化的那部分。第二个切片是原切片中仍未初始化的剩余
    /// 部分。
    ///
    /// # Panics
    ///
    /// 如果该迭代器的 `next` 函数发生 panic，此函数会 panic。
    ///
    /// 如果发生这样的 panic，本次操作中先前已初始化的任何元素都会被 drop。
    ///
    /// # 示例
    ///
    /// 完全填满该切片：
    ///
    /// ```
    /// #![feature(maybe_uninit_fill)]
    /// use std::mem::MaybeUninit;
    ///
    /// let mut buf = [const { MaybeUninit::uninit() }; 5];
    ///
    /// let iter = [1, 2, 3].into_iter().cycle();
    /// let (initialized, remainder) = buf.write_iter(iter);
    ///
    /// assert_eq!(initialized, &mut [1, 2, 3, 1, 2]);
    /// assert_eq!(remainder.len(), 0);
    /// ```
    ///
    /// 部分填充该切片：
    ///
    /// ```
    /// #![feature(maybe_uninit_fill)]
    /// use std::mem::MaybeUninit;
    ///
    /// let mut buf = [const { MaybeUninit::uninit() }; 5];
    /// let iter = [1, 2];
    /// let (initialized, remainder) = buf.write_iter(iter);
    ///
    /// assert_eq!(initialized, &mut [1, 2]);
    /// assert_eq!(remainder.len(), 3);
    /// ```
    ///
    /// 在填充一个切片之后检查迭代器：
    ///
    /// ```
    /// #![feature(maybe_uninit_fill)]
    /// use std::mem::MaybeUninit;
    ///
    /// let mut buf = [const { MaybeUninit::uninit() }; 3];
    /// let mut iter = [1, 2, 3, 4, 5].into_iter();
    /// let (initialized, remainder) = buf.write_iter(iter.by_ref());
    ///
    /// assert_eq!(initialized, &mut [1, 2, 3]);
    /// assert_eq!(remainder.len(), 0);
    /// assert_eq!(iter.as_slice(), &[4, 5]);
    /// ```
    #[unstable(feature = "maybe_uninit_fill", issue = "117428")]
    pub fn write_iter<I>(&mut self, it: I) -> (&mut [T], &mut [MaybeUninit<T>])
    where
        I: IntoIterator<Item = T>,
    {
        let iter = it.into_iter();
        let mut guard = Guard { slice: self, initialized: 0 };

        for (element, val) in guard.slice.iter_mut().zip(iter) {
            element.write(val);
            guard.initialized += 1;
        }

        let initialized_len = guard.initialized;
        super::forget(guard);

        // SAFETY: guard.initialized <= self.len()
        let (initted, remainder) = unsafe { self.split_at_mut_unchecked(initialized_len) };

        // SAFETY: 有效的元素刚刚被写入了 `init`，所以 `this` 的那部分已被初始化。
        (unsafe { initted.assume_init_mut() }, remainder)
    }

    /// 把此 `MaybeUninit` 切片的内容作为一个由“可能未初始化的字节”组成的切片返回。
    ///
    /// 注意，即便某个 `MaybeUninit` 的内容已被初始化，该值仍可能含有保持未初始化状态的填充字节。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(maybe_uninit_as_bytes)]
    /// use std::mem::MaybeUninit;
    ///
    /// let uninit = [MaybeUninit::new(0x1234u16), MaybeUninit::new(0x5678u16)];
    /// let uninit_bytes = uninit.as_bytes();
    /// let bytes = unsafe { uninit_bytes.assume_init_ref() };
    /// let val1 = u16::from_ne_bytes(bytes[0..2].try_into().unwrap());
    /// let val2 = u16::from_ne_bytes(bytes[2..4].try_into().unwrap());
    /// assert_eq!(&[val1, val2], &[0x1234u16, 0x5678u16]);
    /// ```
    #[unstable(feature = "maybe_uninit_as_bytes", issue = "93092")]
    pub const fn as_bytes(&self) -> &[MaybeUninit<u8>] {
        // SAFETY: MaybeUninit<u8> 总是有效的，即便对填充字节也是如此
        unsafe {
            slice::from_raw_parts(self.as_ptr().cast::<MaybeUninit<u8>>(), super::size_of_val(self))
        }
    }

    /// 把此 `MaybeUninit` 切片的内容作为一个由“可能未初始化的字节”组成的可变切片返回。
    ///
    /// 注意，即便某个 `MaybeUninit` 的内容已被初始化，该值仍可能含有保持未初始化状态的填充字节。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(maybe_uninit_as_bytes)]
    /// use std::mem::MaybeUninit;
    ///
    /// let mut uninit = [MaybeUninit::<u16>::uninit(), MaybeUninit::<u16>::uninit()];
    /// let uninit_bytes = uninit.as_bytes_mut();
    /// uninit_bytes.write_copy_of_slice(&[0x12, 0x34, 0x56, 0x78]);
    /// let vals = unsafe { uninit.assume_init_ref() };
    /// if cfg!(target_endian = "little") {
    ///     assert_eq!(vals, &[0x3412u16, 0x7856u16]);
    /// } else {
    ///     assert_eq!(vals, &[0x1234u16, 0x5678u16]);
    /// }
    /// ```
    #[unstable(feature = "maybe_uninit_as_bytes", issue = "93092")]
    pub const fn as_bytes_mut(&mut self) -> &mut [MaybeUninit<u8>] {
        // SAFETY: MaybeUninit<u8> 总是有效的，即便对填充字节也是如此
        unsafe {
            slice::from_raw_parts_mut(
                self.as_mut_ptr() as *mut MaybeUninit<u8>,
                super::size_of_val(self),
            )
        }
    }

    /// 就地（in place）drop 所含的各个值。
    ///
    /// # 安全性（Safety）
    ///
    /// 保证该切片中的每一个 `MaybeUninit<T>` 都确实处于已初始化状态，是调用方的责任。在内容尚未
    /// 完全初始化时调用它会导致未定义行为。
    ///
    /// 在此之上，类型 `T` 的所有额外不变量都必须得到满足，因为 `T`（或其成员）的 `Drop` 实现可能
    /// 依赖于此。例如，把一个 `Vec<T>` 设置为一个无效但非空的地址会使它变为已初始化（在当前实现
    /// 下；这并不构成稳定保证），因为编译器对它所知道的唯一要求就是其数据指针必须非空。然而，
    /// drop 这样一个 `Vec<T>` 会导致未定义行为。
    #[stable(feature = "maybe_uninit_slice", since = "1.93.0")]
    #[inline(always)]
    #[rustc_const_unstable(feature = "const_drop_in_place", issue = "109342")]
    pub const unsafe fn assume_init_drop(&mut self)
    where
        T: [const] Destruct,
    {
        if !self.is_empty() {
            // SAFETY: 调用方必须保证 `self` 的每个元素都已被初始化、且满足 `T` 的所有不变量。
            // 在这种情况下，就地 drop 这个值是安全的。
            unsafe { ptr::drop_in_place(self as *mut [MaybeUninit<T>] as *mut [T]) }
        }
    }

    /// 获取一个指向所含值的共享引用。
    ///
    /// # 安全性（Safety）
    ///
    /// 在内容尚未完全初始化时调用它会导致未定义行为：保证该切片中的每一个 `MaybeUninit<T>` 都确实
    /// 处于已初始化状态，是调用方的责任。
    #[stable(feature = "maybe_uninit_slice", since = "1.93.0")]
    #[rustc_const_stable(feature = "maybe_uninit_slice", since = "1.93.0")]
    #[inline(always)]
    pub const unsafe fn assume_init_ref(&self) -> &[T] {
        // SAFETY: 把 `slice` 转换（cast）为 `*const [T]` 是安全的，因为调用方保证 `slice` 已被
        // 初始化，且 `MaybeUninit` 保证拥有与 `T` 相同的布局。所得到的指针是有效的，因为它指向
        // 由 `slice` 所拥有的内存，而 `slice` 是一个引用，因此保证对读取有效。
        unsafe { &*(self as *const Self as *const [T]) }
    }

    /// 获取一个指向所含值的可变（唯一）引用。
    ///
    /// # 安全性（Safety）
    ///
    /// 在内容尚未完全初始化时调用它会导致未定义行为：保证该切片中的每一个 `MaybeUninit<T>` 都确实
    /// 处于已初始化状态，是调用方的责任。例如，`.assume_init_mut()` 不能被用来初始化一个
    /// `MaybeUninit` 切片。
    #[stable(feature = "maybe_uninit_slice", since = "1.93.0")]
    #[rustc_const_stable(feature = "maybe_uninit_slice", since = "1.93.0")]
    #[inline(always)]
    pub const unsafe fn assume_init_mut(&mut self) -> &mut [T] {
        // SAFETY: 与 `slice_get_ref` 的安全性说明类似，但我们持有一个可变引用，它同样保证对写入
        // 有效。
        unsafe { &mut *(self as *mut Self as *mut [T]) }
    }
}

impl<T, const N: usize> MaybeUninit<[T; N]> {
    /// 把一个 `MaybeUninit<[T; N]>` 转置（transpose）为一个 `[MaybeUninit<T>; N]`。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(maybe_uninit_uninit_array_transpose)]
    /// # use std::mem::MaybeUninit;
    ///
    /// let data: [MaybeUninit<u8>; 1000] = MaybeUninit::uninit().transpose();
    /// ```
    #[unstable(feature = "maybe_uninit_uninit_array_transpose", issue = "96097")]
    #[inline]
    pub const fn transpose(self) -> [MaybeUninit<T>; N] {
        // SAFETY: T 与 MaybeUninit<T> 拥有相同的布局
        unsafe { intrinsics::transmute_unchecked(self) }
    }
}

impl<T, const N: usize> [MaybeUninit<T>; N] {
    /// 把一个 `[MaybeUninit<T>; N]` 转置（transpose）为一个 `MaybeUninit<[T; N]>`。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(maybe_uninit_uninit_array_transpose)]
    /// # use std::mem::MaybeUninit;
    ///
    /// let data = [MaybeUninit::<u8>::uninit(); 1000];
    /// let data: MaybeUninit<[u8; 1000]> = data.transpose();
    /// ```
    #[unstable(feature = "maybe_uninit_uninit_array_transpose", issue = "96097")]
    #[inline]
    pub const fn transpose(self) -> MaybeUninit<[T; N]> {
        // SAFETY: T 与 MaybeUninit<T> 拥有相同的布局
        unsafe { intrinsics::transmute_unchecked(self) }
    }
}

struct Guard<'a, T> {
    slice: &'a mut [MaybeUninit<T>],
    initialized: usize,
}

impl<'a, T> Drop for Guard<'a, T> {
    fn drop(&mut self) {
        let initialized_part = &mut self.slice[..self.initialized];
        // SAFETY: 这个原始的子切片将只包含已初始化的对象。
        unsafe {
            initialized_part.assume_init_drop();
        }
    }
}

trait SpecFill<T> {
    fn spec_fill(&mut self, value: T);
}

impl<T: Clone> SpecFill<T> for [MaybeUninit<T>] {
    default fn spec_fill(&mut self, value: T) {
        let mut guard = Guard { slice: self, initialized: 0 };

        if let Some((last, elems)) = guard.slice.split_last_mut() {
            for el in elems {
                el.write(value.clone());
                guard.initialized += 1;
            }

            last.write(value);
        }
        super::forget(guard);
    }
}

impl<T: TrivialClone> SpecFill<T> for [MaybeUninit<T>] {
    fn spec_fill(&mut self, value: T) {
        // SAFETY: 因为 `T` 是 `TrivialClone`，这等价于对每个元素调用 `T::clone`。值得注意的是，
        // `TrivialClone` 还意味着 `clone` 实现不会 panic，所以我们可以省去初始化 guard 之类的东西。
        self.fill_with(|| MaybeUninit::new(unsafe { ptr::read(&value) }));
    }
}
