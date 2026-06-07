//! 处理内存的基础函数。
//!
//! 本模块包含一些用于查询类型大小与对齐方式、以及初始化与操作内存的函数。

#![stable(feature = "rust1", since = "1.0.0")]

use crate::alloc::Layout;
use crate::clone::TrivialClone;
use crate::marker::{Destruct, DiscriminantKind};
use crate::panic::const_assert;
use crate::{clone, cmp, fmt, hash, intrinsics, ptr};

mod manually_drop;
#[stable(feature = "manually_drop", since = "1.20.0")]
pub use manually_drop::ManuallyDrop;

mod maybe_uninit;
#[stable(feature = "maybe_uninit", since = "1.36.0")]
pub use maybe_uninit::MaybeUninit;

mod maybe_dangling;
#[unstable(feature = "maybe_dangling", issue = "118166")]
pub use maybe_dangling::MaybeDangling;

mod transmutability;
#[unstable(feature = "transmutability", issue = "99571")]
pub use transmutability::{Assume, TransmuteFrom};

mod drop_guard;
#[unstable(feature = "drop_guard", issue = "144426")]
pub use drop_guard::DropGuard;

// 这一项必须是再导出（re-export，而不是包装底层 intrinsic），这样我们才能在调用点（call
// site）执行那个特殊的“类型大小相等”魔法检查。
#[stable(feature = "rust1", since = "1.0.0")]
#[doc(inline)]
pub use crate::intrinsics::transmute;

#[unstable(feature = "type_info", issue = "146922")]
pub mod type_info;

/// 取得一个值的所有权，并“忘记”它，**不运行它的析构逻辑（destructor）**。
///
/// 该值所管理的任何资源（例如堆内存或文件句柄）都将永远滞留在不可达的状态。然而，这并不保证
/// 指向这块内存的指针仍然有效。
///
/// * 如果你想泄漏内存，参见 [`Box::leak`]。
/// * 如果你想获取指向这块内存的裸指针，参见 [`Box::into_raw`]。
/// * 如果你想恰当地处置一个值并运行它的析构逻辑，参见 [`mem::drop`]。
///
/// # 安全性(Safety）
///
/// `forget` 并未被标记为 `unsafe`，因为 Rust 的安全保证并不包含“析构逻辑一定会运行”这一保证。
/// 例如，程序可以用 [`Rc`][rc] 创建一个引用环（reference cycle），或调用
/// [`process::exit`][exit] 在不运行析构逻辑的情况下退出。因此，允许从安全代码中调用
/// `mem::forget` 并不会从根本上改变 Rust 的安全保证。
///
/// 话虽如此，泄漏内存或 I/O 对象之类的资源通常是不可取的。这种需求会在 FFI 或 unsafe 代码的一些
/// 特殊用例中出现，但即便如此，通常也更推荐使用 [`ManuallyDrop`]。
///
/// 由于“忘记一个值”是被允许的，你编写的任何 `unsafe` 代码都必须考虑到这种可能性。你不能返回一个
/// 值并指望调用方一定会运行该值的析构逻辑。
///
/// [rc]: ../../std/rc/struct.Rc.html
/// [exit]: ../../std/process/fn.exit.html
///
/// # 示例
///
/// `mem::forget` 在安全场景下的典型用法是绕过由 `Drop` trait 实现的某个值的析构逻辑。例如，
/// 下面这段代码会泄漏一个 `File`，也就是说会回收该变量占用的空间，但永远不会关闭底层的系统资源：
///
/// ```no_run
/// use std::mem;
/// use std::fs::File;
///
/// let file = File::open("foo.txt").unwrap();
/// mem::forget(file);
/// ```
///
/// 当底层资源的所有权此前已被转移给 Rust 之外的代码时（例如把原始的文件描述符传递给 C 代码），
/// 这会很有用。
///
/// # 与 `ManuallyDrop` 的关系
///
/// 虽然 `mem::forget` 也可以用来转移*内存*的所有权，但这样做容易出错。应该改用 [`ManuallyDrop`]。
/// 考虑下面这段代码作为例子：
///
/// ```
/// use std::mem;
///
/// let mut v = vec![65, 122];
/// // 利用 `v` 的内容构造一个 `String`
/// let s = unsafe { String::from_raw_parts(v.as_mut_ptr(), v.len(), v.capacity()) };
/// // 泄漏 `v`，因为它的内存现在由 `s` 管理
/// mem::forget(v);  // 错误 —— v 已失效，绝不能再传给任何函数
/// assert_eq!(s, "Az");
/// // `s` 被隐式 drop，其内存被释放。
/// ```
///
/// 上面的例子有两个问题：
///
/// * 如果在构造 `String` 与调用 `mem::forget()` 之间又添加了更多代码，那么其中发生 panic 就会
///   导致 double free，因为同一块内存同时被 `v` 和 `s` 处理。
/// * 在调用 `v.as_mut_ptr()` 并把数据所有权转移给 `s` 之后，`v` 这个值就失效了。即便一个值只是被
///   move 进 `mem::forget`（它并不会检视该值），某些类型对其值也有严格要求，使得它们在悬垂或不再
///   被拥有时即变为无效。以任何方式使用无效值——包括把它们传入函数或从函数返回——都构成未定义行为，
///   并可能破坏编译器所做的假设。
///
/// 改用 `ManuallyDrop` 可以避免上述两个问题：
///
/// ```
/// use std::mem::ManuallyDrop;
///
/// let v = vec![65, 122];
/// // 在我们把 `v` 拆解为它的各个原始部分之前，先确保它不会被 drop！
/// let mut v = ManuallyDrop::new(v);
/// // 现在拆解 `v`。这些操作不会 panic，因此不可能发生泄漏。
/// let (ptr, len, cap) = (v.as_mut_ptr(), v.len(), v.capacity());
/// // 最后，构造一个 `String`。
/// let s = unsafe { String::from_raw_parts(ptr, len, cap) };
/// assert_eq!(s, "Az");
/// // `s` 被隐式 drop，其内存被释放。
/// ```
///
/// `ManuallyDrop` 能稳健地防止 double free，因为我们在做其他任何事情之前就先禁用了 `v` 的析构
/// 逻辑。`mem::forget()` 做不到这一点，因为它会消耗（consume）其参数，迫使我们只能在从 `v` 中提取
/// 出所需的一切之后才调用它。即便在构造 `ManuallyDrop` 与构造字符串之间引入了一次 panic（在如上
/// 所示的代码中不会发生），其结果也只会是泄漏而不是 double free。换句话说，`ManuallyDrop` 宁可
/// 倾向于泄漏，也不倾向于（重复）drop。
///
/// 此外，`ManuallyDrop` 让我们不必在把所有权转移给 `s` 之后还去“触碰”`v`——为了在不运行析构逻辑的
/// 前提下处置 `v` 而与之交互的最后那一步，被彻底省去了。
///
/// [`Box`]: ../../std/boxed/struct.Box.html
/// [`Box::leak`]: ../../std/boxed/struct.Box.html#method.leak
/// [`Box::into_raw`]: ../../std/boxed/struct.Box.html#method.into_raw
/// [`mem::drop`]: drop
/// [ub]: ../../reference/behavior-considered-undefined.html
#[inline]
#[rustc_const_stable(feature = "const_forget", since = "1.46.0")]
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_diagnostic_item = "mem_forget"]
pub const fn forget<T>(t: T) {
    let _ = ManuallyDrop::new(t);
}

/// 与 [`forget`] 类似，但还接受不定长（unsized）的值。
///
/// 虽然 Rust 自从在 [#111942] 中移除了不定长局部变量（unsized local）之后就不再允许它们，
/// 但仍然可以从函数参数或位置表达式（place expression）出发，用不定长的值去调用函数。
///
/// ```rust
/// #![feature(unsized_fn_params, forget_unsized)]
/// #![allow(internal_features)]
///
/// use std::mem::forget_unsized;
///
/// pub fn in_place() {
///     forget_unsized(*Box::<str>::from("str"));
/// }
///
/// pub fn param(x: str) {
///     forget_unsized(x);
/// }
/// ```
///
/// 这之所以可行，是因为编译器会改写这些函数，改为通过引用来传递参数。这一技巧对于支持
/// `Box<dyn FnOnce()>: FnOnce()` 是必要的。更多信息参见 [#68304] 与 [#71170]。
///
/// [#111942]: https://github.com/rust-lang/rust/issues/111942
/// [#68304]: https://github.com/rust-lang/rust/issues/68304
/// [#71170]: https://github.com/rust-lang/rust/pull/71170
#[inline]
#[unstable(feature = "forget_unsized", issue = "none")]
pub fn forget_unsized<T: ?Sized>(t: T) {
    intrinsics::forget(t)
}

/// 返回某个类型以字节为单位的大小。
///
/// 更确切地说，这是一个以该类型为元素的数组中相邻元素之间、以字节计的偏移量（包含对齐填充
/// padding）。因此，对于任意类型 `T` 和长度 `n`，`[T; n]` 的大小为 `n * size_of::<T>()`。
///
/// 一般来说，一个类型的大小在不同编译之间并不稳定，但某些特定类型（例如原生类型）是稳定的。
///
/// 下表给出了各原生类型的大小。
///
/// 类型 | `size_of::<Type>()`
/// ---- | ---------------
/// () | 0
/// bool | 1
/// u8 | 1
/// u16 | 2
/// u32 | 4
/// u64 | 8
/// u128 | 16
/// i8 | 1
/// i16 | 2
/// i32 | 4
/// i64 | 8
/// i128 | 16
/// f32 | 4
/// f64 | 8
/// char | 4
///
/// 此外，`usize` 与 `isize` 拥有相同的大小。
///
/// 类型 [`*const T`]、`&T`、[`Box<T>`]、[`Option<&T>`] 以及 `Option<Box<T>>` 都拥有相同的大小。
/// 如果 `T` 是 `Sized`，那么所有这些类型都与 `usize` 拥有相同的大小。
///
/// 指针的可变性不会改变它的大小。因此，`&T` 与 `&mut T` 大小相同。同理，`*const T` 与 `*mut T`
/// 也大小相同。
///
/// # `#[repr(C)]` 项的大小
///
/// 各项的 `C` 表示（representation）拥有确定的布局。在此布局下，只要所有字段都拥有稳定的大小，
/// 各项的大小也是稳定的。
///
/// ## 结构体的大小
///
/// 对于 `struct`，其大小由以下算法决定。
///
/// 按声明顺序遍历结构体中的每个字段：
///
/// 1. 加上该字段的大小。
/// 2. 把当前大小向上取整到下一个字段[对齐][alignment]的最近倍数。
///
/// 最后，把结构体的大小向上取整到其自身[对齐][alignment]的最近倍数。结构体的对齐通常是其所有字段
/// 中最大的那个对齐；可以通过使用 `repr(align(N))` 来改变它。
///
/// 与 `C` 不同，零大小（zero sized）结构体的大小不会被向上取整为 1 字节。
///
/// ## 枚举的大小
///
/// 除判别值（discriminant）外不携带任何数据的枚举，与它们所编译目标平台上的 C 枚举大小相同。
///
/// ## 联合体（Union）的大小
///
/// 一个联合体的大小等于它最大字段的大小。
///
/// 与 `C` 不同，零大小联合体的大小不会被向上取整为 1 字节。
///
/// # 示例
///
/// ```
/// // 一些原生类型
/// assert_eq!(4, size_of::<i32>());
/// assert_eq!(8, size_of::<f64>());
/// assert_eq!(0, size_of::<()>());
///
/// // 一些数组
/// assert_eq!(8, size_of::<[i32; 2]>());
/// assert_eq!(12, size_of::<[i32; 3]>());
/// assert_eq!(0, size_of::<[i32; 0]>());
///
///
/// // 指针大小相等
/// assert_eq!(size_of::<&i32>(), size_of::<*const i32>());
/// assert_eq!(size_of::<&i32>(), size_of::<Box<i32>>());
/// assert_eq!(size_of::<&i32>(), size_of::<Option<&i32>>());
/// assert_eq!(size_of::<Box<i32>>(), size_of::<Option<Box<i32>>>());
/// ```
///
/// 使用 `#[repr(C)]`。
///
/// ```
/// #[repr(C)]
/// struct FieldStruct {
///     first: u8,
///     second: u16,
///     third: u8
/// }
///
/// // 第一个字段的大小为 1，因此大小加 1。大小为 1。
/// // 第二个字段的对齐为 2，因此为填充给大小加 1。大小为 2。
/// // 第二个字段的大小为 2，因此大小加 2。大小为 4。
/// // 第三个字段的对齐为 1，因此为填充给大小加 0。大小为 4。
/// // 第三个字段的大小为 1，因此大小加 1。大小为 5。
/// // 最后，结构体的对齐为 2（因为其字段中最大的对齐是 2），因此为填充给大小加 1。大小为 6。
/// assert_eq!(6, size_of::<FieldStruct>());
///
/// #[repr(C)]
/// struct TupleStruct(u8, u16, u8);
///
/// // 元组结构体遵循同样的规则。
/// assert_eq!(6, size_of::<TupleStruct>());
///
/// // 注意，重新排列字段可以减小大小。把 `third` 放在 `second` 之前，就能去掉两个填充字节。
/// #[repr(C)]
/// struct FieldStructOptimized {
///     first: u8,
///     third: u8,
///     second: u16
/// }
///
/// assert_eq!(4, size_of::<FieldStructOptimized>());
///
/// // 联合体的大小等于其最大字段的大小。
/// #[repr(C)]
/// union ExampleUnion {
///     smaller: u8,
///     larger: u16
/// }
///
/// assert_eq!(2, size_of::<ExampleUnion>());
/// ```
///
/// [alignment]: align_of
/// [`*const T`]: primitive@pointer
/// [`Box<T>`]: ../../std/boxed/struct.Box.html
/// [`Option<&T>`]: crate::option::Option
///
#[inline(always)]
#[must_use]
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_promotable]
#[rustc_const_stable(feature = "const_mem_size_of", since = "1.24.0")]
#[rustc_diagnostic_item = "mem_size_of"]
pub const fn size_of<T>() -> usize {
    <T as SizedTypeProperties>::SIZE
}

/// 返回所指向的值以字节为单位的大小。
///
/// 这通常与 [`size_of::<T>()`] 相同。但是，当 `T` *没有*静态已知的大小时（例如切片
/// [`[T]`][slice] 或 [trait 对象][trait object]），就可以用 `size_of_val` 来获取其动态已知
/// 的大小。
///
/// [trait object]: ../../book/ch17-02-trait-objects.html
///
/// # 示例
///
/// ```
/// assert_eq!(4, size_of_val(&5i32));
///
/// let x: [u8; 13] = [0; 13];
/// let y: &[u8] = &x;
/// assert_eq!(13, size_of_val(y));
/// ```
///
/// [`size_of::<T>()`]: size_of
#[inline]
#[must_use]
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_stable(feature = "const_size_of_val", since = "1.85.0")]
#[rustc_diagnostic_item = "mem_size_of_val"]
pub const fn size_of_val<T: ?Sized>(val: &T) -> usize {
    // SAFETY: `val` 是一个引用，因此它是一个有效的裸指针
    unsafe { intrinsics::size_of_val(val) }
}

/// 返回所指向的值以字节为单位的大小。
///
/// 这通常与 [`size_of::<T>()`] 相同。但是，当 `T` *没有*静态已知的大小时（例如切片
/// [`[T]`][slice] 或 [trait 对象][trait object]），就可以用 `size_of_val_raw` 来获取其动态
/// 已知的大小。
///
/// # 安全性(Safety）
///
/// 此函数只有在满足以下条件时调用才是安全的：
///
/// - 如果 `T` 是 `Sized`，调用此函数总是安全的。
/// - 如果 `T` 的不定长尾部（unsized tail）是：
///     - 一个[切片][slice]，那么该切片尾部的长度必须是一个已初始化的整数，并且*整个值*的大小
///       （动态尾部长度 + 静态固定的前缀）必须能放进 `isize`。对于动态尾部长度为 0 这一特例，
///       调用此函数是安全的。
//        注意：之所以安全，是因为如果在大小为 0 时就已经发生溢出，那么我们会停止编译，因为即便
//        是该类型“静态已知”的那部分也已经过大了（或者该调用可能位于死代码中并被优化掉，但那样
//        的话就无关紧要了）。
///     - 一个 [trait 对象][trait object]，那么指针的 vtable 部分必须指向一个经由 unsizing 强转
///       （coercion）获得的有效 vtable，并且*整个值*的大小（动态尾部长度 + 静态固定的前缀）必须
///       能放进 `isize`。
///     - 一个（不稳定的）[extern type]，那么调用此函数总是安全的，但它可能 panic 或以其他方式
///       返回错误的值，因为该 extern type 的布局是未知的。这与对一个尾部为 extern type 的类型的
///       引用调用 [`size_of_val`] 的行为相同。
///     - 否则，出于保守考虑，不允许调用此函数。
///
/// [`size_of::<T>()`]: size_of
/// [trait object]: ../../book/ch17-02-trait-objects.html
/// [extern type]: ../../unstable-book/language-features/extern-types.html
///
/// # 示例
///
/// ```
/// #![feature(layout_for_ptr)]
/// use std::mem;
///
/// assert_eq!(4, size_of_val(&5i32));
///
/// let x: [u8; 13] = [0; 13];
/// let y: &[u8] = &x;
/// assert_eq!(13, unsafe { mem::size_of_val_raw(y) });
/// ```
#[inline]
#[must_use]
#[unstable(feature = "layout_for_ptr", issue = "69835")]
pub const unsafe fn size_of_val_raw<T: ?Sized>(val: *const T) -> usize {
    // SAFETY: 调用方必须提供一个有效的裸指针
    unsafe { intrinsics::size_of_val(val) }
}

/// 返回某个类型以字节为单位、[ABI] 所要求的最小对齐。
///
/// 指向类型 `T` 的值的每一个引用，其地址都必须是这个数的倍数。
///
/// 这是用于结构体字段的对齐。它可能小于首选对齐（preferred alignment）。
///
/// [ABI]: https://en.wikipedia.org/wiki/Application_binary_interface
///
/// # 示例
///
/// ```
/// # #![allow(deprecated)]
/// use std::mem;
///
/// assert_eq!(4, mem::min_align_of::<i32>());
/// ```
#[inline]
#[must_use]
#[stable(feature = "rust1", since = "1.0.0")]
#[deprecated(note = "use `align_of` instead", since = "1.2.0", suggestion = "align_of")]
pub fn min_align_of<T>() -> usize {
    <T as SizedTypeProperties>::ALIGN
}

/// 返回 `val` 所指向的值的类型，以字节为单位、[ABI] 所要求的最小对齐。
///
/// 指向类型 `T` 的值的每一个引用，其地址都必须是这个数的倍数。
///
/// [ABI]: https://en.wikipedia.org/wiki/Application_binary_interface
///
/// # 示例
///
/// ```
/// # #![allow(deprecated)]
/// use std::mem;
///
/// assert_eq!(4, mem::min_align_of_val(&5i32));
/// ```
#[inline]
#[must_use]
#[stable(feature = "rust1", since = "1.0.0")]
#[deprecated(note = "use `align_of_val` instead", since = "1.2.0", suggestion = "align_of_val")]
pub fn min_align_of_val<T: ?Sized>(val: &T) -> usize {
    // SAFETY: val 是一个引用，因此它是一个有效的裸指针
    unsafe { intrinsics::align_of_val(val) }
}

/// 返回某个类型以字节为单位、[ABI] 所要求的最小对齐。
///
/// 指向类型 `T` 的值的每一个引用，其地址都必须是这个数的倍数。
///
/// 这是用于结构体字段的对齐。它可能小于首选对齐（preferred alignment）。
///
/// [ABI]: https://en.wikipedia.org/wiki/Application_binary_interface
///
/// # 示例
///
/// ```
/// assert_eq!(4, align_of::<i32>());
/// ```
#[inline(always)]
#[must_use]
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_promotable]
#[rustc_const_stable(feature = "const_align_of", since = "1.24.0")]
#[rustc_diagnostic_item = "mem_align_of"]
pub const fn align_of<T>() -> usize {
    <T as SizedTypeProperties>::ALIGN
}

/// 返回 `val` 所指向的值的类型，以字节为单位、[ABI] 所要求的最小对齐。
///
/// 指向类型 `T` 的值的每一个引用，其地址都必须是这个数的倍数。
///
/// [ABI]: https://en.wikipedia.org/wiki/Application_binary_interface
///
/// # 示例
///
/// ```
/// assert_eq!(4, align_of_val(&5i32));
/// ```
#[inline]
#[must_use]
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_stable(feature = "const_align_of_val", since = "1.85.0")]
pub const fn align_of_val<T: ?Sized>(val: &T) -> usize {
    // SAFETY: val 是一个引用，因此它是一个有效的裸指针
    unsafe { intrinsics::align_of_val(val) }
}

/// 返回 `val` 所指向的值的类型，以字节为单位、[ABI] 所要求的最小对齐。
///
/// 指向类型 `T` 的值的每一个引用，其地址都必须是这个数的倍数。
///
/// [ABI]: https://en.wikipedia.org/wiki/Application_binary_interface
///
/// # 安全性(Safety）
///
/// 此函数只有在满足以下条件时调用才是安全的：
///
/// - 如果 `T` 是 `Sized`，调用此函数总是安全的。
/// - 如果 `T` 的不定长尾部（unsized tail）是：
///     - 一个[切片][slice]，那么该切片尾部的长度必须是一个已初始化的整数，并且*整个值*的大小
///       （动态尾部长度 + 静态固定的前缀）必须能放进 `isize`。对于动态尾部长度为 0 这一特例，
///       调用此函数是安全的。
///     - 一个 [trait 对象][trait object]，那么指针的 vtable 部分必须指向一个经由 unsizing 强转
///       （coercion）获得的有效 vtable，并且*整个值*的大小（动态尾部长度 + 静态固定的前缀）必须
///       能放进 `isize`。
///     - 一个（不稳定的）[extern type]，那么调用此函数总是安全的，但它可能 panic 或以其他方式
///       返回错误的值，因为该 extern type 的布局是未知的。这与对一个尾部为 extern type 的类型的
///       引用调用 [`align_of_val`] 的行为相同。
///     - 否则，出于保守考虑，不允许调用此函数。
///
/// [trait object]: ../../book/ch17-02-trait-objects.html
/// [extern type]: ../../unstable-book/language-features/extern-types.html
///
/// # 示例
///
/// ```
/// #![feature(layout_for_ptr)]
/// use std::mem;
///
/// assert_eq!(4, unsafe { mem::align_of_val_raw(&5i32) });
/// ```
#[inline]
#[must_use]
#[unstable(feature = "layout_for_ptr", issue = "69835")]
pub const unsafe fn align_of_val_raw<T: ?Sized>(val: *const T) -> usize {
    // SAFETY: 调用方必须提供一个有效的裸指针
    unsafe { intrinsics::align_of_val(val) }
}

/// 当 drop 类型 `T` 的值有意义时返回 `true`。
///
/// 这纯粹是一个优化提示（optimization hint），并且可以被保守地实现：对于实际上无需 drop 的
/// 类型，它也可能返回 `true`。因此，永远返回 `true` 也会是此函数的一个有效实现。然而，如果此
/// 函数确实返回了 `false`，那么你就可以确定 drop `T` 不会产生任何副作用。
///
/// 那些需要手动 drop 自身数据的底层实现（例如各种集合类型）应当使用此函数，以避免在它们被销毁时
/// 不必要地尝试 drop 其全部内容。这在 release 构建中可能没有区别（在那里，一个没有副作用的循环
/// 很容易被检测出来并消除掉），但对 debug 构建来说往往是一大收益。
///
/// 注意，[`drop_in_place`] 已经会执行这项检查，因此如果你的工作量可以归结为少量几次
/// [`drop_in_place`] 调用，就无需使用此函数。特别要注意的是，你可以对一个切片调用
/// [`drop_in_place`]，它会针对所有的值只做一次 needs_drop 检查。
///
/// 因此，像 Vec 这样的类型只是直接 `drop_in_place(&mut self[..])`，而不会显式使用 `needs_drop`。
/// 另一方面，像 [`HashMap`] 这样的类型则必须逐个 drop 其中的值，因而应当使用此 API。
///
/// [`drop_in_place`]: crate::ptr::drop_in_place
/// [`HashMap`]: ../../std/collections/struct.HashMap.html
///
/// # 示例
///
/// 下面是一个集合类型可能如何使用 `needs_drop` 的例子：
///
/// ```
/// use std::{mem, ptr};
///
/// pub struct MyCollection<T> {
/// #   data: [T; 1],
///     /* ... */
/// }
/// # impl<T> MyCollection<T> {
/// #   fn iter_mut(&mut self) -> &mut [T] { &mut self.data }
/// #   fn free_buffer(&mut self) {}
/// # }
///
/// impl<T> Drop for MyCollection<T> {
///     fn drop(&mut self) {
///         unsafe {
///             // drop 这些数据
///             if mem::needs_drop::<T>() {
///                 for x in self.iter_mut() {
///                     ptr::drop_in_place(x);
///                 }
///             }
///             self.free_buffer();
///         }
///     }
/// }
/// ```
#[inline]
#[must_use]
#[stable(feature = "needs_drop", since = "1.21.0")]
#[rustc_const_stable(feature = "const_mem_needs_drop", since = "1.36.0")]
#[rustc_diagnostic_item = "needs_drop"]
pub const fn needs_drop<T: ?Sized>() -> bool {
    const { intrinsics::needs_drop::<T>() }
}

/// 返回由全零字节模式（all-zero byte-pattern）所表示的类型 `T` 的值。
///
/// 这意味着，举例来说，`(u8, u16)` 中的填充字节（padding byte）不一定会被置零。
///
/// 并不保证全零字节模式一定表示某个类型 `T` 的有效值。例如，对于引用类型（`&T`、`&mut T`）和
/// 函数指针，全零字节模式就不是有效值。对这类类型使用 `zeroed` 会立即导致[未定义行为][ub]，
/// 因为[Rust 编译器假定][inv]：在它认为已初始化的变量里，总是存在一个有效的值。
///
/// 这与 [`MaybeUninit::zeroed().assume_init()`][zeroed] 效果相同。它有时对 FFI 有用，但通常
/// 应当避免使用。
///
/// [zeroed]: MaybeUninit::zeroed
/// [ub]: ../../reference/behavior-considered-undefined.html
/// [inv]: MaybeUninit#initialization-invariant
///
/// # 示例
///
/// 此函数的正确用法：用零来初始化一个整数。
///
/// ```
/// use std::mem;
///
/// let x: i32 = unsafe { mem::zeroed() };
/// assert_eq!(0, x);
/// ```
///
/// 此函数的*错误*用法：用零来初始化一个引用。
///
/// ```rust,no_run
/// # #![allow(invalid_value)]
/// use std::mem;
///
/// let _x: &i32 = unsafe { mem::zeroed() }; // 未定义行为！
/// let _y: fn() = unsafe { mem::zeroed() }; // 同样是未定义行为！
/// ```
#[inline(always)]
#[must_use]
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_diagnostic_item = "mem_zeroed"]
#[track_caller]
#[rustc_const_stable(feature = "const_mem_zeroed", since = "1.75.0")]
pub const unsafe fn zeroed<T>() -> T {
    // SAFETY: 调用方必须保证全零值对 `T` 来说是有效的。
    unsafe {
        intrinsics::assert_zero_valid::<T>();
        MaybeUninit::zeroed().assume_init()
    }
}

/// 绕过 Rust 正常的内存初始化检查：它假装产生了一个类型 `T` 的值，但实际上什么都不做。
///
/// **此函数已被废弃。**请改用 [`MaybeUninit<T>`]。由于为限制此函数在遗留代码中被错误使用所造成的
/// 潜在危害，曾对其加入了一些缓解措施（mitigation），因此它也可能比使用 `MaybeUninit<T>` 更慢。
///
/// 废弃的原因在于，此函数基本上无法被正确使用：它与
/// [`MaybeUninit::uninit().assume_init()`][uninit] 效果相同。正如 [`assume_init` 文档][assume_init]
/// 所解释的那样，[Rust 编译器假定][inv]值都是被正确初始化过的。
///
/// 这里所返回的那种真正未初始化的内存很特殊，因为编译器知道它没有一个固定的值。这使得：即便一个
/// 变量是整数类型，让它持有未初始化数据也是未定义行为。
///
/// 因此，对几乎所有类型（包括整数类型以及整数类型的数组）调用此函数，都会立即导致未定义行为，
/// 即便其结果根本没有被使用。
///
/// [uninit]: MaybeUninit::uninit
/// [assume_init]: MaybeUninit::assume_init
/// [inv]: MaybeUninit#initialization-invariant
#[inline(always)]
#[must_use]
#[deprecated(since = "1.39.0", note = "use `mem::MaybeUninit` instead")]
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_diagnostic_item = "mem_uninitialized"]
#[track_caller]
pub unsafe fn uninitialized<T>() -> T {
    // SAFETY: 调用方必须保证未初始化的值对 `T` 来说是有效的。
    unsafe {
        intrinsics::assert_mem_uninitialized_valid::<T>();
        let mut val = MaybeUninit::<T>::uninit();

        // 用 0x01 填充内存，作为一种不完美的缓解措施，应对那些在 bool、nonnull 和 noundef 类型上
        // 使用此函数的旧代码。但如果我们正主动想要检测 UB，则不要这样做。
        if !cfg!(any(miri, sanitize = "memory")) {
            val.as_mut_ptr().write_bytes(0x01, 1);
        }

        val.assume_init()
    }
}

/// 交换两个可变位置上的值，且不会让其中任何一个变为未初始化（deinitialize）。
///
/// * 如果你想用一个默认值或占位值（dummy value）来交换，参见 [`take`]。
/// * 如果你想用传入的值来交换并返回旧值，参见 [`replace`]。
///
/// # 示例
///
/// ```
/// use std::mem;
///
/// let mut x = 5;
/// let mut y = 42;
///
/// mem::swap(&mut x, &mut y);
///
/// assert_eq!(42, x);
/// assert_eq!(5, y);
/// ```
#[inline]
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_stable(feature = "const_swap", since = "1.85.0")]
#[rustc_diagnostic_item = "mem_swap"]
pub const fn swap<T>(x: &mut T, y: &mut T) {
    // SAFETY: `&mut` 保证这两者在类型上既可读又可写，并且互不重叠（non-overlapping）。
    unsafe { intrinsics::typed_swap_nonoverlapping(x, y) }
}

/// 用 `T` 的默认值替换 `dest`，并返回 `dest` 之前的值。
///
/// * 如果你想替换两个变量的值，参见 [`swap`]。
/// * 如果你想用传入的值而不是默认值来替换，参见 [`replace`]。
///
/// # 示例
///
/// 一个简单的例子：
///
/// ```
/// use std::mem;
///
/// let mut v: Vec<i32> = vec![1, 2];
///
/// let old_v = mem::take(&mut v);
/// assert_eq!(vec![1, 2], old_v);
/// assert!(v.is_empty());
/// ```
///
/// `take` 允许你通过把一个结构体字段替换为“空”值来取得它的所有权。如果没有 `take`，你可能会遇到
/// 这样的问题：
///
/// ```compile_fail,E0507
/// struct Buffer<T> { buf: Vec<T> }
///
/// impl<T> Buffer<T> {
///     fn get_and_reset(&mut self) -> Vec<T> {
///         // 错误：无法从 `&mut` 指针的解引用中 move 出值
///         let buf = self.buf;
///         self.buf = Vec::new();
///         buf
///     }
/// }
/// ```
///
/// 注意，`T` 不一定实现了 [`Clone`]，所以甚至无法靠克隆来重置 `self.buf`。但 `take` 可以用来把
/// `self.buf` 的原值与 `self` 解除关联，从而允许把它返回出去：
///
/// ```
/// use std::mem;
///
/// # struct Buffer<T> { buf: Vec<T> }
/// impl<T> Buffer<T> {
///     fn get_and_reset(&mut self) -> Vec<T> {
///         mem::take(&mut self.buf)
///     }
/// }
///
/// let mut buffer = Buffer { buf: vec![0, 1] };
/// assert_eq!(buffer.buf.len(), 2);
///
/// assert_eq!(buffer.get_and_reset(), vec![0, 1]);
/// assert_eq!(buffer.buf.len(), 0);
/// ```
#[inline]
#[stable(feature = "mem_take", since = "1.40.0")]
#[rustc_const_unstable(feature = "const_default", issue = "143894")]
pub const fn take<T: [const] Default>(dest: &mut T) -> T {
    replace(dest, T::default())
}

/// 把 `src` move 进所引用的 `dest`，并返回 `dest` 之前的值。
///
/// 两个值都不会被 drop。
///
/// * 如果你想替换两个变量的值，参见 [`swap`]。
/// * 如果你想用默认值来替换，参见 [`take`]。
///
/// # 示例
///
/// 一个简单的例子：
///
/// ```
/// use std::mem;
///
/// let mut v: Vec<i32> = vec![1, 2];
///
/// let old_v = mem::replace(&mut v, vec![3, 4, 5]);
/// assert_eq!(vec![1, 2], old_v);
/// assert_eq!(vec![3, 4, 5], v);
/// ```
///
/// `replace` 允许你通过把一个结构体字段替换为另一个值来消耗（consume）它。如果没有 `replace`，
/// 你可能会遇到这样的问题：
///
/// ```compile_fail,E0507
/// struct Buffer<T> { buf: Vec<T> }
///
/// impl<T> Buffer<T> {
///     fn replace_index(&mut self, i: usize, v: T) -> T {
///         // 错误：无法从 `&mut` 指针的解引用中 move 出值
///         let t = self.buf[i];
///         self.buf[i] = v;
///         t
///     }
/// }
/// ```
///
/// 注意，`T` 不一定实现了 [`Clone`]，所以我们甚至无法靠克隆 `self.buf[i]` 来避免这次 move。但
/// `replace` 可以用来把该下标处的原值与 `self` 解除关联，从而允许把它返回出去：
///
/// ```
/// # #![allow(dead_code)]
/// use std::mem;
///
/// # struct Buffer<T> { buf: Vec<T> }
/// impl<T> Buffer<T> {
///     fn replace_index(&mut self, i: usize, v: T) -> T {
///         mem::replace(&mut self.buf[i], v)
///     }
/// }
///
/// let mut buffer = Buffer { buf: vec![0, 1] };
/// assert_eq!(buffer.buf[0], 0);
///
/// assert_eq!(buffer.replace_index(0, 2), 0);
/// assert_eq!(buffer.buf[0], 2);
/// ```
#[inline]
#[stable(feature = "rust1", since = "1.0.0")]
#[must_use = "if you don't need the old value, you can just assign the new value directly"]
#[rustc_const_stable(feature = "const_replace", since = "1.83.0")]
#[rustc_diagnostic_item = "mem_replace"]
pub const fn replace<T>(dest: &mut T, src: T) -> T {
    // 这里可能会有人想用 `swap` 来避免 `unsafe`。千万别这么做！
    // 编译器会把下面的实现优化为两次 `memcpy`，而 `swap` 则至少需要三次。详见 PR#83022。

    // SAFETY: 我们从 `dest` 读取，但随后会直接把 `src` 写入它，因此旧值不会被复制（duplicate）。
    // 这里没有任何东西被 drop，也没有任何东西会 panic。
    unsafe {
        // 理想情况下我们不会在这里使用 intrinsics，但走 `ptr` 的方法会引入两次不必要的 UbCheck，
        // 因此在我们能够为“来自引用的指针”移除这些检查之前，这里改用 intrinsics，以使其在 MIR
        //（以及 debug）中保持非常低的开销。

        let result = crate::intrinsics::read_via_copy(dest);
        crate::intrinsics::write_via_move(dest, src);
        result
    }
}

/// 处置一个值。
///
/// 对于实现了 `Copy` 的类型（例如整数），这实际上什么都不做。这类值会被复制，*然后*才被 move 进
/// 此函数，因此该值在此函数调用之后依然存在。
///
/// 此函数并不神奇；它的定义字面上就是：
///
/// ```
/// pub fn drop<T>(_x: T) {}
/// ```
///
/// 由于 `_x` 被 move 进了此函数，它会在函数返回之前被自动 [drop][drop]。
///
/// [drop]: Drop
///
/// # 示例
///
/// 基本用法：
///
/// ```
/// let v = vec![1, 2, 3];
///
/// drop(v); // 显式 drop 这个 vector
/// ```
///
/// 由于 [`RefCell`] 在运行时强制执行借用规则，`drop` 可以释放一个 [`RefCell`] 的借用：
///
/// ```
/// use std::cell::RefCell;
///
/// let x = RefCell::new(1);
///
/// let mut mutable_borrow = x.borrow_mut();
/// *mutable_borrow = 1;
///
/// drop(mutable_borrow); // 放弃对这个槽位的可变借用
///
/// let borrow = x.borrow();
/// println!("{}", *borrow);
/// ```
///
/// 整数以及其他实现了 [`Copy`] 的类型不受 `drop` 影响。
///
/// ```
/// # #![allow(dropping_copy_types)]
/// #[derive(Copy, Clone)]
/// struct Foo(u8);
///
/// let x = 1;
/// let y = Foo(2);
/// drop(x); // `x` 的一个副本被 move 进来并被 drop
/// drop(y); // `y` 的一个副本被 move 进来并被 drop
///
/// println!("x: {}, y: {}", x, y.0); // 仍然可用
/// ```
///
/// [`RefCell`]: crate::cell::RefCell
#[inline]
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_unstable(feature = "const_destruct", issue = "133214")]
#[rustc_diagnostic_item = "mem_drop"]
pub const fn drop<T>(_x: T)
where
    T: [const] Destruct,
{
}

/// 按位（bitwise）复制一个值。
///
/// 此函数并不神奇；它的定义字面上就是：
/// ```
/// pub const fn copy<T: Copy>(x: &T) -> T { *x }
/// ```
///
/// 当你想把一个函数指针传给某个组合子（combinator），而不想为此定义一个新的闭包时，它很有用。
///
/// 示例：
/// ```
/// #![feature(mem_copy_fn)]
/// use core::mem::copy;
/// let result_from_ffi_function: Result<(), &i32> = Err(&1);
/// let result_copied: Result<(), i32> = result_from_ffi_function.map_err(copy);
/// ```
#[inline]
#[unstable(feature = "mem_copy_fn", issue = "98262")]
pub const fn copy<T: Copy>(x: &T) -> T {
    *x
}

/// 把 `src` 解释为拥有类型 `&Dst`，然后在不 move 所含值的前提下读取 `src`。
///
/// 此函数会以不安全的方式假定指针 `src` 在 [`size_of::<Dst>`][size_of] 字节范围内有效：它把
/// `&Src` transmute 为 `&Dst`，然后读取这个 `&Dst`（不过这一过程是以一种即便 `&Dst` 比 `&Src`
/// 有更严格的对齐要求时也正确的方式完成的）。它还会以不安全的方式创建所含值的一个副本，而不是从
/// `src` 中 move 出来。
///
/// 即便 `Src` 与 `Dst` 大小不同，也不算编译期错误，但强烈建议只在 `Src` 与 `Dst` 大小相同的地方
/// 调用此函数。如果 `Dst` 比 `Src` 更大，此函数会触发[未定义行为][ub]。
///
/// [ub]: ../../reference/behavior-considered-undefined.html
///
/// # 示例
///
/// ```
/// use std::mem;
///
/// #[repr(packed)]
/// struct Foo {
///     bar: u8,
/// }
///
/// let foo_array = [10u8];
///
/// unsafe {
///     // 从 'foo_array' 复制数据，并把它当作一个 'Foo'
///     let mut foo_struct: Foo = mem::transmute_copy(&foo_array);
///     assert_eq!(foo_struct.bar, 10);
///
///     // 修改复制出来的数据
///     foo_struct.bar = 20;
///     assert_eq!(foo_struct.bar, 20);
/// }
///
/// // 'foo_array' 的内容不应有任何改变
/// assert_eq!(foo_array, [10]);
/// ```
#[inline]
#[must_use]
#[track_caller]
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_stable(feature = "const_transmute_copy", since = "1.74.0")]
pub const unsafe fn transmute_copy<Src, Dst>(src: &Src) -> Dst {
    assert!(
        size_of::<Src>() >= size_of::<Dst>(),
        "cannot transmute_copy if Dst is larger than Src"
    );

    // 如果 Dst 有更高的对齐要求，src 可能没有被恰当地对齐。
    if align_of::<Dst>() > align_of::<Src>() {
        // SAFETY: `src` 是一个引用，保证对读取有效。
        // 至于实际的 transmute 是否安全，则必须由调用方保证。
        unsafe { ptr::read_unaligned(src as *const Src as *const Dst) }
    } else {
        // SAFETY: `src` 是一个引用，保证对读取有效。
        // 我们刚刚检查过 `src as *const Dst` 是被恰当对齐的。
        // 至于实际的 transmute 是否安全，则必须由调用方保证。
        unsafe { ptr::read(src as *const Src as *const Dst) }
    }
}

/// 表示一个枚举的判别值（discriminant）的不透明（opaque）类型。
///
/// 更多信息参见本模块中的 [`discriminant`] 函数。
#[stable(feature = "discriminant_value", since = "1.21.0")]
pub struct Discriminant<T>(<T as DiscriminantKind>::Discriminant);

// 注意：这些 trait 实现无法通过 derive 生成，因为我们不想给 T 加上任何约束（bound）。

#[stable(feature = "discriminant_value", since = "1.21.0")]
impl<T> Copy for Discriminant<T> {}

#[stable(feature = "discriminant_value", since = "1.21.0")]
impl<T> clone::Clone for Discriminant<T> {
    fn clone(&self) -> Self {
        *self
    }
}

#[doc(hidden)]
#[unstable(feature = "trivial_clone", issue = "none")]
unsafe impl<T> TrivialClone for Discriminant<T> {}

#[stable(feature = "discriminant_value", since = "1.21.0")]
impl<T> cmp::PartialEq for Discriminant<T> {
    fn eq(&self, rhs: &Self) -> bool {
        self.0 == rhs.0
    }
}

#[stable(feature = "discriminant_value", since = "1.21.0")]
impl<T> cmp::Eq for Discriminant<T> {}

#[stable(feature = "discriminant_value", since = "1.21.0")]
impl<T> hash::Hash for Discriminant<T> {
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

#[stable(feature = "discriminant_value", since = "1.21.0")]
impl<T> fmt::Debug for Discriminant<T> {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.debug_tuple("Discriminant").field(&self.0).finish()
    }
}

/// 返回一个能唯一标识 `v` 中枚举变体的值。
///
/// 如果 `T` 不是枚举，调用此函数不会导致未定义行为，但其返回值是未指定的（unspecified）。
///
/// # 稳定性（Stability）
///
/// 如果枚举的定义发生变化，某个枚举变体的判别值也可能改变。但在使用同一个编译器进行的多次编译
/// 之间，某个变体的判别值不会改变。更多信息参见 [Reference]。
///
/// [Reference]: ../../reference/items/enumerations.html#custom-discriminant-values-for-fieldless-enumerations
///
/// [`Discriminant<T>`] 的值与 `T` 中的任何*自由生命周期（free lifetime）*无关。因此，把一个
/// `Discriminant<Foo<'a>>` 当作 `Discriminant<Foo<'b>>` 来读取或写入（无论是通过 [`transmute`]
/// 还是其他方式）总是健全的。注意，对于其他种类的泛型参数以及高阶生命周期（higher-ranked
/// lifetime）来说这**并不**成立；`Discriminant<Foo<A>>` 与 `Discriminant<Foo<B>>`，以及
/// `Discriminant<Bar<dyn for<'a> Trait<'a>>>` 与 `Discriminant<Bar<dyn Trait<'static>>>`，都可能
/// 互不兼容。
///
/// # 示例
///
/// 这可以用来在忽略实际数据的前提下，比较那些携带数据的枚举：
///
/// ```
/// use std::mem;
///
/// enum Foo { A(&'static str), B(i32), C(i32) }
///
/// assert_eq!(mem::discriminant(&Foo::A("bar")), mem::discriminant(&Foo::A("baz")));
/// assert_eq!(mem::discriminant(&Foo::B(1)), mem::discriminant(&Foo::B(2)));
/// assert_ne!(mem::discriminant(&Foo::B(3)), mem::discriminant(&Foo::C(3)));
/// ```
///
/// ## 访问判别值的数值
///
/// 注意，从 [`Discriminant`] [`transmute`] 到一个原生类型是*未定义行为*！
///
/// 如果一个枚举只有单元变体（unit variant），那么可以用 [`as`] 转换来访问判别值的数值：
///
/// ```
/// enum Enum {
///     Foo,
///     Bar,
///     Baz,
/// }
///
/// assert_eq!(0, Enum::Foo as isize);
/// assert_eq!(1, Enum::Bar as isize);
/// assert_eq!(2, Enum::Baz as isize);
/// ```
///
/// 如果一个枚举选择（opt-in）为其判别值采用[原生表示][primitive representation]，那么就可以用
/// 指针读取存储判别值的那块内存。然而，对于采用[默认表示][default representation]的枚举来说，
/// 这**不行**，因为其判别值的布局以及存储位置都是未指定的——它甚至可能根本就没有被存储！
///
/// [`as`]: ../../std/keyword.as.html
/// [primitive representation]: ../../reference/type-layout.html#primitive-representations
/// [default representation]: ../../reference/type-layout.html#the-default-representation
/// ```
/// #[repr(u8)]
/// enum Enum {
///     Unit,
///     Tuple(bool),
///     Struct { a: bool },
/// }
///
/// impl Enum {
///     fn discriminant(&self) -> u8 {
///         // SAFETY: 因为 `Self` 被标记为 `repr(u8)`，它的布局是若干个 `repr(C)` 结构体之间的
///         // 一个 `repr(C)` `union`，其中每个结构体都把 `u8` 判别值作为其第一个字段，因此我们
///         // 无需偏移指针就能读取判别值。
///         unsafe { *<*const _>::from(self).cast::<u8>() }
///     }
/// }
///
/// let unit_like = Enum::Unit;
/// let tuple_like = Enum::Tuple(true);
/// let struct_like = Enum::Struct { a: false };
/// assert_eq!(0, unit_like.discriminant());
/// assert_eq!(1, tuple_like.discriminant());
/// assert_eq!(2, struct_like.discriminant());
///
/// // ⚠️ 这是未定义行为。不要这样做。⚠️
/// // assert_eq!(0, unsafe { std::mem::transmute::<_, u8>(std::mem::discriminant(&unit_like)) });
/// ```
#[stable(feature = "discriminant_value", since = "1.21.0")]
#[rustc_const_stable(feature = "const_discriminant", since = "1.75.0")]
#[rustc_diagnostic_item = "mem_discriminant"]
#[cfg_attr(miri, track_caller)] // 即使没有 panic，这也有助于 Miri 回溯。
pub const fn discriminant<T>(v: &T) -> Discriminant<T> {
    Discriminant(intrinsics::discriminant_value(v))
}

/// 返回枚举类型 `T` 中变体的数量。
///
/// 如果 `T` 不是枚举，调用此函数不会导致未定义行为，但其返回值是未指定的。同样地，如果 `T` 是
/// 一个变体数量超过 `usize::MAX` 的枚举，其返回值也是未指定的。无人居住（uninhabited）的变体也
/// 会被计入。
///
/// 注意，将来枚举可能会作为一种非破坏性（non-breaking）变更而被扩展出更多变体，例如当它被标记为
/// `#[non_exhaustive]` 时，这会改变此函数的结果。
///
/// # 示例
///
/// ```
/// # #![feature(never_type)]
/// # #![feature(variant_count)]
///
/// use std::mem;
///
/// enum Void {}
/// enum Foo { A(&'static str), B(i32), C(i32) }
///
/// assert_eq!(mem::variant_count::<Void>(), 0);
/// assert_eq!(mem::variant_count::<Foo>(), 3);
///
/// assert_eq!(mem::variant_count::<Option<!>>(), 2);
/// assert_eq!(mem::variant_count::<Result<!, !>>(), 2);
/// ```
#[inline(always)]
#[must_use]
#[unstable(feature = "variant_count", issue = "73662")]
#[rustc_const_unstable(feature = "variant_count", issue = "73662")]
#[rustc_diagnostic_item = "mem_variant_count"]
pub const fn variant_count<T>() -> usize {
    const { intrinsics::variant_count::<T>() }
}

/// 为类型的各种有用属性提供关联常量（associated constant），以便在我们的代码中给它们一个规范
/// 形式（canonical form），并使其更易于阅读。
///
/// 它在这里只是为了简化我们在库中所需的所有 ZST 检查。它目前不处于稳定化进程中。
#[doc(hidden)]
#[unstable(feature = "sized_type_properties", issue = "none")]
pub trait SizedTypeProperties: Sized {
    #[doc(hidden)]
    #[unstable(feature = "sized_type_properties", issue = "none")]
    #[lang = "mem_size_const"]
    const SIZE: usize = intrinsics::size_of::<Self>();

    #[doc(hidden)]
    #[unstable(feature = "sized_type_properties", issue = "none")]
    #[lang = "mem_align_const"]
    const ALIGN: usize = intrinsics::align_of::<Self>();

    /// 如果此类型无需任何存储空间则为 `true`。
    /// 如果它的[大小](size_of)大于零则为 `false`。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(sized_type_properties)]
    /// use core::mem::SizedTypeProperties;
    ///
    /// fn do_something_with<T>() {
    ///     if T::IS_ZST {
    ///         // ... 特殊处理方式 ...
    ///     } else {
    ///         // ... 常规处理 ...
    ///     }
    /// }
    ///
    /// struct MyUnit;
    /// assert!(MyUnit::IS_ZST);
    ///
    /// // 对于否定式的检查，考虑使用 UFCS 来强调这个取反
    /// assert!(!<i32>::IS_ZST);
    /// // 否则它有时会隐藏在类型里
    /// assert!(!String::IS_ZST);
    /// ```
    #[doc(hidden)]
    #[unstable(feature = "sized_type_properties", issue = "none")]
    const IS_ZST: bool = Self::SIZE == 0;

    #[doc(hidden)]
    #[unstable(feature = "sized_type_properties", issue = "none")]
    const LAYOUT: Layout = {
        // SAFETY: 如果该类型被实例化（instantiate），rustc 已经确保了它的布局是有效的。使用未经
        // 检查的构造器，以避免插入一条需要被优化掉的、会 panic 的代码路径。
        unsafe { Layout::from_size_align_unchecked(Self::SIZE, Self::ALIGN) }
    };

    /// 一个 `[Self]` 所允许的最大安全长度。
    ///
    /// 任何比这更大的长度都会使 `size_of_val` 溢出 `isize::MAX`，而这对于单个对象来说是绝不
    /// 允许的。
    #[doc(hidden)]
    #[unstable(feature = "sized_type_properties", issue = "none")]
    const MAX_SLICE_LEN: usize = match Self::SIZE {
        0 => usize::MAX,
        n => (isize::MAX as usize) / n,
    };
}
#[doc(hidden)]
#[unstable(feature = "sized_type_properties", issue = "none")]
impl<T> SizedTypeProperties for T {}

/// 展开为某个字段相对于给定类型起始处、以字节为单位的偏移量。
///
/// 该类型可以是 `struct`、`enum`、`union` 或元组。
///
/// 该字段可以是嵌套字段（`field1.field2`），但不能是数组下标。该字段必须对调用点可见。
///
/// 偏移量以 [`usize`] 返回。
///
/// # 动态大小类型的偏移量，以及其内部字段的偏移量
///
/// 该字段的*类型*必须是 [`Sized`]，但它可以位于一个[动态大小][dynamically sized]容器之中。如果
/// 字段类型是动态大小的，那么你就不能使用 `offset_of!`（因为该字段的对齐、进而其偏移量也可能是
/// 动态的），而必须改为从一个指向该容器的实际指针来获取偏移量。
///
/// ```
/// # use core::mem;
/// # use core::fmt::Debug;
/// #[repr(C)]
/// pub struct Struct<T: ?Sized> {
///     a: u8,
///     b: T,
/// }
///
/// #[derive(Debug)]
/// #[repr(C, align(4))]
/// struct Align4(u32);
///
/// assert_eq!(mem::offset_of!(Struct<dyn Debug>, a), 0); // 可行 —— Sized 字段
/// assert_eq!(mem::offset_of!(Struct<Align4>, b), 4); // 可行 —— 不是 DST
///
/// // assert_eq!(mem::offset_of!(Struct<dyn Debug>, b), 1);
/// // ^^^ error[E0277]: ... 无法在编译期得知
///
/// // 要获取一个 !Sized 字段的偏移量，应检视一个具体的值，
/// // 而不是使用 offset_of!。
/// let value: Struct<Align4> = Struct { a: 1, b: Align4(2) };
/// let ref_unsized: &Struct<dyn Debug> = &value;
/// let offset_of_b = unsafe {
///     (&raw const ref_unsized.b).byte_offset_from_unsigned(ref_unsized)
/// };
/// assert_eq!(offset_of_b, 4);
/// ```
///
/// 如果你需要获取一个 `!Sized` 类型某个字段的偏移量，那么由于该偏移量可能取决于所存储的具体值
///（尤其是，`dyn Trait` 值有着动态决定的对齐），你必须从一个具体的引用或指针来获取偏移量，因此
/// 没有它你就无法使用 `offset_of!`。
///
/// # 布局可能变化
///
/// 注意，类型布局一般来说是[可能变化且平台相关的](https://doc.rust-lang.org/reference/type-layout.html)。
/// 如果需要布局稳定性，考虑使用[显式的 `repr` 属性][explicit `repr` attribute]。
///
/// Rust 保证某个给定字段在某个给定类型中的偏移量，在程序的整个生命周期内不会改变。然而，对同一个
/// 程序的两次不同编译可能产生不同的布局。此外，即便在单次程序执行内部，对于那些*相似*但并不*完全
/// 相同*的类型，也不做任何保证；例如：
///
/// ```
/// struct Wrapper<T, U>(T, U);
///
/// type A = Wrapper<u8, u8>;
/// type B = Wrapper<u8, i8>;
///
/// // 即便 `u8` 和 `i8` 拥有相同的布局，二者也不一定完全相同！
/// // assert_eq!(mem::offset_of!(A, 1), mem::offset_of!(B, 1));
///
/// #[repr(transparent)]
/// struct U8(u8);
///
/// type C = Wrapper<u8, U8>;
///
/// // 即便 `u8` 和 `U8` 拥有相同的布局，二者也不一定完全相同！
/// // assert_eq!(mem::offset_of!(A, 1), mem::offset_of!(C, 1));
///
/// struct Empty<T>(core::marker::PhantomData<T>);
///
/// // 即便 `PhantomData` 总是拥有相同的布局，二者也不一定完全相同！
/// // assert_eq!(mem::offset_of!(Empty<u8>, 0), mem::offset_of!(Empty<i8>, 0));
/// ```
///
/// [explicit `repr` attribute]: https://doc.rust-lang.org/reference/type-layout.html#representations
///
/// # 不稳定特性
///
/// 以下不稳定特性扩展了 `offset_of!` 的功能：
///
/// * [`offset_of_enum`] —— 允许像访问字段那样遍历 `enum` 的变体。
/// * [`offset_of_slice`] —— 允许获取类型为 `[T]` 的字段的偏移量。
///
/// # 示例
///
/// ```
/// use std::mem;
/// #[repr(C)]
/// struct FieldStruct {
///     first: u8,
///     second: u16,
///     third: u8
/// }
///
/// assert_eq!(mem::offset_of!(FieldStruct, first), 0);
/// assert_eq!(mem::offset_of!(FieldStruct, second), 2);
/// assert_eq!(mem::offset_of!(FieldStruct, third), 4);
///
/// #[repr(C)]
/// struct NestedA {
///     b: NestedB
/// }
///
/// #[repr(C)]
/// struct NestedB(u8);
///
/// assert_eq!(mem::offset_of!(NestedA, b.0), 0);
/// ```
///
/// [dynamically sized]: https://doc.rust-lang.org/reference/dynamically-sized-types.html
/// [`offset_of_enum`]: https://doc.rust-lang.org/nightly/unstable-book/language-features/offset-of-enum.html
/// [`offset_of_slice`]: https://doc.rust-lang.org/nightly/unstable-book/language-features/offset-of-slice.html
#[stable(feature = "offset_of", since = "1.77.0")]
#[allow_internal_unstable(builtin_syntax, core_intrinsics)]
pub macro offset_of($Container:ty, $($fields:expr)+ $(,)?) {
    // 这里的 `{}` 是为了得到更好的错误信息
    const {builtin # offset_of($Container, $($fields)+)}
}

/// 创建一个有人居住（inhabited）的 ZST 类型 `T` 的全新实例。
///
/// 在你明知 `T` 是零大小、但又没有能让你用安全代码实例化它的约束（例如 [`Default`]）的地方，
/// 应优先使用本函数，而不是 [`zeroed`]、[`uninitialized`] 或 [`transmute_copy`]。
///
/// 如果你不确定 `T` 是否是一个有人居住的 ZST，那么你应当使用 [`MaybeUninit`]，而不是此函数。
///
/// # Panics
///
/// 当 `size_of::<T>() != 0` 时。
///
/// # 安全性(Safety）
///
/// - `T` 必须是*[有人居住的][inhabited]*，即可以被构造出来。这意味着像零变体枚举（zero-variant
///   enum）和 [`!`] 这样的类型，凭空造出（conjure）它们是不健全的。
/// - 你只能以不违反该类型任何*安全性（safety）*不变量的方式来使用这个值。
///
/// 虽然创建一个有人居住的 ZST 的*有效（valid）*实例很容易——因为它的表示里没有任何位，意味着只有
/// 唯一一个可能的值——但这并不意味着这样做总是*健全（sound）*的。
///
/// 例如，某个库可能设计出一些 `!Default + !Clone` 的零大小令牌（token），把它们的创建限制在某些
/// 用于初始化某种状态或建立某个作用域的函数中。凭空造出这样一个令牌可能破坏不变量并导致不健全。
///
/// # 示例
///
/// ```
/// #![feature(mem_conjure_zst)]
/// use std::mem::conjure_zst;
///
/// assert_eq!(unsafe { conjure_zst::<()>() }, ());
/// assert_eq!(unsafe { conjure_zst::<[i32; 0]>() }, []);
/// ```
///
/// [inhabited]: https://doc.rust-lang.org/reference/glossary.html#inhabited
#[unstable(feature = "mem_conjure_zst", issue = "95383")]
pub const unsafe fn conjure_zst<T>() -> T {
    const_assert!(
        size_of::<T>() == 0,
        "mem::conjure_zst invoked on a nonzero-sized type",
        "mem::conjure_zst invoked on type {t}, which is not zero-sized",
        t: &str = stringify!(T)
    );

    // SAFETY: 因为调用方必须保证它是有人居住且零大小的，所以它的表示里没有任何东西需要设置。
    // `assume_init` 会调用 `assert_inhabited`，因此我们无需在这里再调用一次。
    unsafe {
        #[allow(clippy::uninit_assumed_init)]
        MaybeUninit::uninit().assume_init()
    }
}
