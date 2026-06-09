//! 用于类型之间转换的 trait。
//!
//! 本模块中的 trait 提供了一种从一种类型转换为另一种类型的方式。每个 trait
//! 各有用途:
//!
//! - 实现 [`AsRef`] trait,用于廉价的“引用到引用”转换
//! - 实现 [`AsMut`] trait,用于廉价的“可变引用到可变引用”转换
//! - 实现 [`From`] trait,用于会消耗原值的“值到值”转换
//! - 实现 [`Into`] trait,用于会消耗原值、目标类型在当前 crate 之外的
//!   “值到值”转换
//! - [`TryFrom`] 和 [`TryInto`] 这两个 trait 的行为与 [`From`] 和 [`Into`]
//!   类似,但应当在转换可能失败时实现。
//!
//! 本模块中的 trait 常被用作泛型函数的 trait 约束,以便支持多种类型的参数。
//! 用法示例见各个 trait 的文档。
//!
//! 作为库作者,你应当始终优先实现 [`From<T>`][`From`] 或
//! [`TryFrom<T>`][`TryFrom`],而非 [`Into<U>`][`Into`] 或
//! [`TryInto<U>`][`TryInto`]——因为得益于标准库中的一条覆盖性(blanket)实现,
//! [`From`] 和 [`TryFrom`] 提供了更大的灵活性,并且免费提供了等价的
//! [`Into`] 或 [`TryInto`] 实现。这正是孤儿规则下优先实现 `From` 的缘由:
//! 你实现一次 `From`,就同时白得了 `Into`。当目标是 Rust 1.41 之前的版本时,
//! 在转换到当前 crate 之外的类型时,可能仍有必要直接实现 [`Into`] 或
//! [`TryInto`]。
//!
//! 关于这些转换的语义约定:[`From`] 不应失败、也不应丢失信息(它是一个
//! 全转换);而 [`TryFrom`] 用于可能失败的转换,失败时返回 `Result` 的
//! `Err`。
//!
//! # 泛型实现
//!
//! - [`AsRef`] 和 [`AsMut`] 在内部类型是引用时会自动解引用(但通常并不对
//!   所有[可解引用类型][core::ops::Deref]都如此)
//! - [`From`]`<U> for T` 蕴含 [`Into`]`<T> for U`
//! - [`TryFrom`]`<U> for T` 蕴含 [`TryInto`]`<T> for U`
//! - [`From`] 和 [`Into`] 是自反的,这意味着所有类型都能 `into` 自身、
//!   也能 `from` 自身
//!
//! 用法示例见各个 trait。

#![stable(feature = "rust1", since = "1.0.0")]

use crate::error::Error;
use crate::fmt;
use crate::hash::{Hash, Hasher};
use crate::marker::PointeeSized;

mod num;

#[unstable(feature = "convert_float_to_int", issue = "67057")]
pub use num::FloatToInt;

/// 恒等函数(identity function)。
///
/// 关于本函数有两点很重要:
///
/// - 它并不总是等价于像 `|x| x` 这样的闭包,因为闭包可能会把 `x` 强转成
///   另一种类型。
///
/// - 它会 move 传给函数的输入 `x`。
///
/// 一个只是把输入原样返回的函数听上去也许有些奇怪,但它确有一些有趣的用途。
///
/// # 示例
///
/// 用 `identity` 在一串其他有趣的函数之中充当“什么都不做”的一员:
///
/// ```rust
/// use std::convert::identity;
///
/// fn manipulation(x: u32) -> u32 {
///     // 我们假装“加一”是个有趣的函数。
///     x + 1
/// }
///
/// let _arr = &[identity, manipulation];
/// ```
///
/// 在条件分支中用 `identity` 作为“什么都不做”的基准情形:
///
/// ```rust
/// use std::convert::identity;
///
/// # let condition = true;
/// #
/// # fn manipulation(x: u32) -> u32 { x + 1 }
/// #
/// let do_stuff = if condition { manipulation } else { identity };
///
/// // 做更多有趣的事……
///
/// let _results = do_stuff(42);
/// ```
///
/// 用 `identity` 来保留一个 `Option<T>` 迭代器中的 `Some` 变体:
///
/// ```rust
/// use std::convert::identity;
///
/// let iter = [Some(1), None, Some(3)].into_iter();
/// let filtered = iter.filter_map(identity).collect::<Vec<_>>();
/// assert_eq!(vec![1, 3], filtered);
/// ```
#[stable(feature = "convert_id", since = "1.33.0")]
#[rustc_const_stable(feature = "const_identity", since = "1.33.0")]
#[inline(always)]
#[rustc_diagnostic_item = "convert_identity"]
pub const fn identity<T>(x: T) -> T {
    x
}

/// 用于进行廉价的“引用到引用”转换。
///
/// 本 trait 类似于 [`AsMut`],后者用于在可变引用之间转换。如果你需要做一次
/// 开销不菲的转换,最好改为以类型 `&T` 实现 [`From`],或者写一个自定义函数。
///
/// # 与 `Borrow` 的关系
///
/// `AsRef` 与 [`Borrow`] 具有相同的签名,但 [`Borrow`] 在几个方面有所不同:
///
/// - 与 `AsRef` 不同,[`Borrow`] 对任意 `T` 都有一条覆盖性实现,并且既可以
///   用来接收引用,也可以用来接收值。(另见下文关于 `AsRef` 自反性的说明。)
/// - [`Borrow`] 还要求:对一个借用值的 [`Hash`]、[`Eq`] 和 [`Ord`] 与对其
///   拥有值的实现是等价的。正因如此,如果你只想借用一个结构体的某个字段,
///   你可以实现 `AsRef`,却不能实现 [`Borrow`]。
///
/// **注意:本 trait 一定不能失败**。如果转换可能失败,请使用一个返回
/// [`Option<T>`] 或 [`Result<T, E>`] 的专门方法。
///
/// # 泛型实现
///
/// 如果内部类型是引用或可变引用,`AsRef` 会自动解引用(例如:无论 `foo` 的
/// 类型是 `&mut Foo` 还是 `&&mut Foo`,`foo.as_ref()` 的效果都一样)。
///
/// 注意,出于历史原因,上述行为目前并不普遍适用于所有[可解引用类型],例如
/// `foo.as_ref()` 与 `Box::new(foo).as_ref()` 的效果 *并不* 相同。相反,
/// 许多智能指针提供的 `as_ref` 实现只是返回指向[被指向值]的引用(而不会为
/// 那个值执行廉价的“引用到引用”转换)。然而,[`AsRef::as_ref`] 不应仅仅为了
/// 解引用而被使用;为此可以改用[“`Deref` 强转”]:
///
/// [可解引用类型]: core::ops::Deref
/// [被指向值]: core::ops::Deref::Target
/// [“`Deref` 强转”]: core::ops::Deref#deref-coercion
///
/// ```
/// let x = Box::new(5i32);
/// // 避免这样:
/// // let y: &i32 = x.as_ref();
/// // 最好就写:
/// let y: &i32 = &x;
/// ```
///
/// 实现了 [`Deref`] 的类型,应当考虑如下实现 `AsRef<T>`:
///
/// [`Deref`]: core::ops::Deref
///
/// ```
/// # use core::ops::Deref;
/// # struct SomeType;
/// # impl Deref for SomeType {
/// #     type Target = [u8];
/// #     fn deref(&self) -> &[u8] {
/// #         &[]
/// #     }
/// # }
/// impl<T> AsRef<T> for SomeType
/// where
///     T: ?Sized,
///     <SomeType as Deref>::Target: AsRef<T>,
/// {
///     fn as_ref(&self) -> &T {
///         self.deref().as_ref()
///     }
/// }
/// ```
///
/// # 自反性
///
/// 理想情况下,`AsRef` 应当是自反的,即应当存在一条 `impl<T: ?Sized> AsRef<T> for T`,
/// 其 [`as_ref`] 只是原样返回它的参数。由于 Rust 类型系统的技术限制,目前
/// *并未* 提供这样一条覆盖性实现(它会与另一条已存在的、针对
/// `&T where T: AsRef<U>` 的覆盖性实现重叠——正是后者让 `AsRef` 能自动
/// 解引用,见上面的“泛型实现”)。
///
/// [`as_ref`]: AsRef::as_ref
///
/// 对于特定类型 `T`,在需要或希望时,必须显式地添加 `AsRef<T> for T` 的平凡
/// 实现。不过要注意,并非所有来自 `std` 的类型都含有这样一条实现,而且由于
/// 孤儿规则,外部代码也无法为它们添加。
///
/// # 示例
///
/// 通过使用 trait 约束,只要参数能被转换为指定类型 `T`,我们就可以接收不同
/// 类型的参数。
///
/// 例如:通过创建一个接收 `AsRef<str>` 的泛型函数,我们表达了希望接收所有
/// 能被转换为 [`&str`] 的引用作为参数。由于 [`String`] 和 [`&str`] 都实现了
/// `AsRef<str>`,我们可以把两者都作为输入参数接收。
///
/// [`&str`]: primitive@str
/// [`Borrow`]: crate::borrow::Borrow
/// [`Eq`]: crate::cmp::Eq
/// [`Ord`]: crate::cmp::Ord
/// [`String`]: ../../std/string/struct.String.html
///
/// ```
/// fn is_hello<T: AsRef<str>>(s: T) {
///    assert_eq!("hello", s.as_ref());
/// }
///
/// let s = "hello";
/// is_hello(s);
///
/// let s = "hello".to_string();
/// is_hello(s);
/// ```
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_diagnostic_item = "AsRef"]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
pub const trait AsRef<T: PointeeSized>: PointeeSized {
    /// 把该类型转换为(通常被推断出来的)输入类型的共享引用。
    #[stable(feature = "rust1", since = "1.0.0")]
    fn as_ref(&self) -> &T;
}

/// 用于进行廉价的“可变引用到可变引用”转换。
///
/// 本 trait 类似于 [`AsRef`],但用于在可变引用之间转换。如果你需要做一次
/// 开销不菲的转换,最好改为以类型 `&mut T` 实现 [`From`],或者写一个自定义
/// 函数。
///
/// **注意:本 trait 一定不能失败**。如果转换可能失败,请使用一个返回
/// [`Option<T>`] 或 [`Result<T, E>`] 的专门方法。
///
/// # 泛型实现
///
/// 如果内部类型是可变引用,`AsMut` 会自动解引用(例如:无论 `foo` 的类型是
/// `&mut Foo` 还是 `&mut &mut Foo`,`foo.as_mut()` 的效果都一样)。
///
/// 注意,出于历史原因,上述行为目前并不普遍适用于所有[可变解引用类型],
/// 例如 `foo.as_mut()` 与 `Box::new(foo).as_mut()` 的效果 *并不* 相同。相反,
/// 许多智能指针提供的 `as_mut` 实现只是返回指向[被指向值]的引用(而不会为
/// 那个值执行廉价的“引用到引用”转换)。然而,[`AsMut::as_mut`] 不应仅仅为了
/// 可变解引用而被使用;为此可以改用[“`Deref` 强转”]:
///
/// [可变解引用类型]: core::ops::DerefMut
/// [被指向值]: core::ops::Deref::Target
/// [“`Deref` 强转”]: core::ops::DerefMut#mutable-deref-coercion
///
/// ```
/// let mut x = Box::new(5i32);
/// // 避免这样:
/// // let y: &mut i32 = x.as_mut();
/// // 最好就写:
/// let y: &mut i32 = &mut x;
/// ```
///
/// 实现了 [`DerefMut`] 的类型,应当考虑如下添加一个 `AsMut<T>` 实现:
///
/// [`DerefMut`]: core::ops::DerefMut
///
/// ```
/// # use core::ops::{Deref, DerefMut};
/// # struct SomeType;
/// # impl Deref for SomeType {
/// #     type Target = [u8];
/// #     fn deref(&self) -> &[u8] {
/// #         &[]
/// #     }
/// # }
/// # impl DerefMut for SomeType {
/// #     fn deref_mut(&mut self) -> &mut [u8] {
/// #         &mut []
/// #     }
/// # }
/// impl<T> AsMut<T> for SomeType
/// where
///     <SomeType as Deref>::Target: AsMut<T>,
/// {
///     fn as_mut(&mut self) -> &mut T {
///         self.deref_mut().as_mut()
///     }
/// }
/// ```
///
/// # 自反性
///
/// 理想情况下,`AsMut` 应当是自反的,即应当存在一条 `impl<T: ?Sized> AsMut<T> for T`,
/// 其 [`as_mut`] 只是原样返回它的参数。由于 Rust 类型系统的技术限制,目前
/// *并未* 提供这样一条覆盖性实现(它会与另一条已存在的、针对
/// `&mut T where T: AsMut<U>` 的覆盖性实现重叠——正是后者让 `AsMut` 能自动
/// 解引用,见上面的“泛型实现”)。
///
/// [`as_mut`]: AsMut::as_mut
///
/// 对于特定类型 `T`,在需要或希望时,必须显式地添加 `AsMut<T> for T` 的平凡
/// 实现。不过要注意,并非所有来自 `std` 的类型都含有这样一条实现,而且由于
/// 孤儿规则,外部代码也无法为它们添加。
///
/// # 示例
///
/// 把 `AsMut` 用作泛型函数的 trait 约束,我们就可以接收所有能被转换为类型
/// `&mut T` 的可变引用。与[解引用]不同(它只有单一的[目标类型]),一个类型
/// 可以有多个 `AsMut` 实现。特别地,`Vec<T>` 同时实现了 `AsMut<Vec<T>>` 和
/// `AsMut<[T]>`。
///
/// 在下面的例子中,示例函数 `caesar` 和 `null_terminate` 提供了一套泛型接口,
/// 它们能分别与任何“可以通过廉价的可变到可变转换转成字节切片(`[u8]`)或字节
/// 向量(`Vec<u8>`)”的类型协作。
///
/// [解引用]: core::ops::DerefMut
/// [目标类型]: core::ops::Deref::Target
///
/// ```
/// struct Document {
///     info: String,
///     content: Vec<u8>,
/// }
///
/// impl<T: ?Sized> AsMut<T> for Document
/// where
///     Vec<u8>: AsMut<T>,
/// {
///     fn as_mut(&mut self) -> &mut T {
///         self.content.as_mut()
///     }
/// }
///
/// fn caesar<T: AsMut<[u8]>>(data: &mut T, key: u8) {
///     for byte in data.as_mut() {
///         *byte = byte.wrapping_add(key);
///     }
/// }
///
/// fn null_terminate<T: AsMut<Vec<u8>>>(data: &mut T) {
///     // 使用一个非泛型的内部函数(它包含了大部分功能),有助于把单态化
///     // (monomorphization)的开销降到最低。
///     fn doit(data: &mut Vec<u8>) {
///         let len = data.len();
///         if len == 0 || data[len-1] != 0 {
///             data.push(0);
///         }
///     }
///     doit(data.as_mut());
/// }
///
/// fn main() {
///     let mut v: Vec<u8> = vec![1, 2, 3];
///     caesar(&mut v, 5);
///     assert_eq!(v, [6, 7, 8]);
///     null_terminate(&mut v);
///     assert_eq!(v, [6, 7, 8, 0]);
///     let mut doc = Document {
///         info: String::from("Example"),
///         content: vec![17, 19, 8],
///     };
///     caesar(&mut doc, 1);
///     assert_eq!(doc.content, [18, 20, 9]);
///     null_terminate(&mut doc);
///     assert_eq!(doc.content, [18, 20, 9, 0]);
/// }
/// ```
///
/// 不过要注意,API 并不需要是泛型的。在许多情况下,例如直接接收一个
/// `&mut [u8]` 或 `&mut Vec<u8>` 反而是更好的选择(那样调用者就需要传入
/// 正确的类型)。
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_diagnostic_item = "AsMut"]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
pub const trait AsMut<T: PointeeSized>: PointeeSized {
    /// 把该类型转换为(通常被推断出来的)输入类型的可变引用。
    #[stable(feature = "rust1", since = "1.0.0")]
    fn as_mut(&mut self) -> &mut T;
}

/// 会消耗输入值的“值到值”转换。它是 [`From`] 的反向操作。
///
/// 应当避免实现 [`Into`],而改为实现 [`From`]。得益于标准库中的覆盖性实现,
/// 实现 [`From`] 会自动为你提供一个 [`Into`] 的实现。
///
/// 在为泛型函数指定 trait 约束时,优先使用 [`Into`] 而非 [`From`],以确保
/// 那些只实现了 [`Into`] 的类型也可以被使用。
///
/// **注意:本 trait 一定不能失败**。如果转换可能失败,请使用 [`TryInto`]。
///
/// # 泛型实现
///
/// - [`From`]`<T> for U` 蕴含 `Into<U> for T`
/// - [`Into`] 是自反的,这意味着 `Into<T> for T` 已被实现
///
/// # 在旧版本 Rust 中为“转换到外部类型”实现 [`Into`]
///
/// 在 Rust 1.41 之前,如果目标类型不属于当前 crate,你就不能直接实现
/// [`From`]。例如,看下面这段代码:
///
/// ```
/// # #![allow(non_local_definitions)]
/// struct Wrapper<T>(Vec<T>);
/// impl<T> From<Wrapper<T>> for Vec<T> {
///     fn from(w: Wrapper<T>) -> Vec<T> {
///         w.0
///     }
/// }
/// ```
/// 在该语言的旧版本中,这会编译失败,因为 Rust 的孤儿规则过去要稍微严格
/// 一些。要绕过这一点,你可以直接实现 [`Into`]:
///
/// ```
/// struct Wrapper<T>(Vec<T>);
/// impl<T> Into<Vec<T>> for Wrapper<T> {
///     fn into(self) -> Vec<T> {
///         self.0
///     }
/// }
/// ```
///
/// 重要的是要理解:[`Into`] 并不提供 [`From`] 实现(而 [`From`] 会提供
/// [`Into`])。因此,你应当始终尝试先实现 [`From`],只有在 [`From`] 无法
/// 实现时才退而求其次实现 [`Into`]。
///
/// # 示例
///
/// [`String`] 实现了 [`Into`]`<`[`Vec`]`<`[`u8`]`>>`:
///
/// 为了表达我们希望某个泛型函数接收所有能被转换为指定类型 `T` 的参数,我们
/// 可以使用 [`Into`]`<T>` 的 trait 约束。例如:函数 `is_hello` 接收所有能被
/// 转换为 [`Vec`]`<`[`u8`]`>` 的参数。
///
/// ```
/// fn is_hello<T: Into<Vec<u8>>>(s: T) {
///    let bytes = b"hello".to_vec();
///    assert_eq!(bytes, s.into());
/// }
///
/// let s = "hello".to_string();
/// is_hello(s);
/// ```
///
/// [`String`]: ../../std/string/struct.String.html
/// [`Vec`]: ../../std/vec/struct.Vec.html
#[rustc_diagnostic_item = "Into"]
#[stable(feature = "rust1", since = "1.0.0")]
#[doc(search_unbox)]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
pub const trait Into<T>: Sized {
    /// 把该类型转换为(通常被推断出来的)输入类型。
    #[must_use]
    #[stable(feature = "rust1", since = "1.0.0")]
    fn into(self) -> T;
}

/// 会消耗输入值的“值到值”转换。它是 [`Into`] 的对偶(reciprocal)。
///
/// 应当始终优先实现 `From` 而非 [`Into`],因为得益于标准库中的覆盖性实现,
/// 实现 `From` 会自动为你提供一个 [`Into`] 的实现。
///
/// 只有在目标是 Rust 1.41 之前的版本、且转换到当前 crate 之外的类型时,
/// 才应当实现 [`Into`]。由于 Rust 的孤儿规则,`From` 在早先版本中无法做这类
/// 转换。更多细节参见 [`Into`]。
///
/// 在为泛型函数指定 trait 约束时,优先使用 [`Into`] 而非 [`From`],以确保
/// 那些只实现了 [`Into`] 的类型也可以被使用。
///
/// `From` trait 在做错误处理时也非常有用。当构造一个可能失败的函数时,其返回
/// 类型一般会是 `Result<T, E>` 的形式。`From` 通过让函数返回一个能封装多种
/// 错误类型的单一错误类型,简化了错误处理。更多细节参见“示例”一节以及
/// [the book][book]。
///
/// **注意:本 trait 一定不能失败**。`From` trait 用于完美(perfect)的转换。
/// 如果转换可能失败或并不完美,请使用 [`TryFrom`]。
///
/// # 泛型实现
///
/// - `From<T> for U` 蕴含 [`Into`]`<U> for T`
/// - `From` 是自反的,这意味着 `From<T> for T` 已被实现
///
/// # 何时该实现 `From`
///
/// 虽然在技术上对“用 `From` 实现可以做哪些转换”并无限制,但一般的期望是:
/// 这些转换通常应当被限制如下:
///
/// * 该转换是 *不会失败的*(infallible):如果转换可能失败,请改用
///   [`TryFrom`];不要提供一个会 panic 的 `From` 实现。
///
/// * 该转换是 *无损的*(lossless):从语义上讲,它不应丢失或丢弃信息。例如,
///   `i32: From<u16>` 存在,原值可以用 `u16: TryFrom<i32>` 恢复出来;
///   `String: From<&str>` 也存在,你可以通过 `Deref` 得到与原值等价的东西。
///   但 `From` 不能用来从 `u32` 转换到 `u16`,因为那无法以无损的方式成功。
///   (对于那些被认为在语义上不相关的信息,这里有一些回旋余地。例如,
///   `Box<[T]>: From<Vec<T>>` 存在,尽管它可能不保留容量——正如两个向量
///   尽管容量不同却可以相等。)
///
/// * 该转换是 *保值的*(value-preserving):所得值在概念上的种类与含义保持
///   不变,即便 Rust 类型和技术上的表示可能不同。例如 `-1_i8 as u8` 是
///   *无损的*,因为用 `as` 转换回来可以恢复原值,但这个转换 *无法* 通过
///   `From` 提供,因为 `-1` 和 `255` 是不同的概念值(尽管在技术上它们的比特
///   模式相同)。但 `f32: From<i16>` *是* 提供的,因为 `1_i16` 和 `1.0_f32`
///   在概念上是同一个实数(尽管它们的比特模式差异很大)。`String: From<char>`
///   是提供的,因为它们都是 *文本*,但 `String: From<u32>` *不* 提供,因为
///   `1`(一个数字)和 `"1"`(文本)差异太大。(把值转换成文本这件事改由
///   [`Display`](crate::fmt::Display) trait 负责。)
///
/// * 该转换是 *显而易见的*(obvious):它是两个类型之间唯一合理的转换。否则,
///   最好把它做成一个有名字的方法或构造器——就像 [`str::as_bytes`] 是一个
///   方法、整数有 [`u32::from_ne_bytes`]、[`u32::from_le_bytes`] 和
///   [`u32::from_be_bytes`] 这样的方法一样,这些都不是 `From` 实现。而把一个
///   [`Ipv6Addr`](crate::net::Ipv6Addr) 包装进
///   [`IpAddr`](crate::net::IpAddr) 只有一种合理的方式,因此
///   `IpAddr: From<Ipv6Addr>` 存在。
///
/// # 示例
///
/// [`String`] 实现了 `From<&str>`:
///
/// 从 `&str` 到 String 的显式转换如下进行:
///
/// ```
/// let string = "hello".to_string();
/// let other_string = String::from("hello");
///
/// assert_eq!(string, other_string);
/// ```
///
/// 在做错误处理时,为你自己的错误类型实现 `From` 往往很有用。通过把底层错误
/// 类型转换为我们自己那个封装了底层错误类型的自定义错误类型,我们就可以在
/// 不丢失底层原因信息的前提下,返回单一的错误类型。`?` 运算符会用 `From::from`
/// 自动把底层错误类型转换为我们的自定义错误类型。
///
/// ```
/// use std::fs;
/// use std::io;
/// use std::num;
///
/// enum CliError {
///     IoError(io::Error),
///     ParseError(num::ParseIntError),
/// }
///
/// impl From<io::Error> for CliError {
///     fn from(error: io::Error) -> Self {
///         CliError::IoError(error)
///     }
/// }
///
/// impl From<num::ParseIntError> for CliError {
///     fn from(error: num::ParseIntError) -> Self {
///         CliError::ParseError(error)
///     }
/// }
///
/// fn open_and_parse_file(file_name: &str) -> Result<i32, CliError> {
///     let mut contents = fs::read_to_string(&file_name)?;
///     let num: i32 = contents.trim().parse()?;
///     Ok(num)
/// }
/// ```
///
/// [`String`]: ../../std/string/struct.String.html
/// [`from`]: From::from
/// [book]: ../../book/ch09-00-error-handling.html
#[rustc_diagnostic_item = "From"]
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_on_unimplemented(on(
    all(Self = "&str", T = "alloc::string::String"),
    note = "to coerce a `{T}` into a `{Self}`, use `&*` as a prefix",
))]
#[doc(search_unbox)]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
pub const trait From<T>: Sized {
    /// 从输入类型转换到本类型。
    #[rustc_diagnostic_item = "from_fn"]
    #[must_use]
    #[stable(feature = "rust1", since = "1.0.0")]
    fn from(value: T) -> Self;
}

/// 一次会消耗 `self` 的“尝试性”转换,其开销可能大也可能不大。
///
/// 库作者通常不应直接实现本 trait,而应优先实现 [`TryFrom`] trait——后者提供
/// 更大的灵活性,并且得益于标准库中的覆盖性实现,会免费提供一个等价的
/// `TryInto` 实现。关于这一点的更多信息,参见 [`Into`] 的文档。
///
/// 在为泛型函数指定 trait 约束时,优先使用 [`TryInto`] 而非 [`TryFrom`],
/// 以确保那些只实现了 [`TryInto`] 的类型也可以被使用。
///
/// # 实现 `TryInto`
///
/// 它受到与实现 [`Into`] 相同的限制,理由也相同,详见那里。
#[rustc_diagnostic_item = "TryInto"]
#[stable(feature = "try_from", since = "1.34.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
pub const trait TryInto<T>: Sized {
    /// 转换出错时返回的类型。
    #[stable(feature = "try_from", since = "1.34.0")]
    type Error;

    /// 执行转换。
    #[stable(feature = "try_from", since = "1.34.0")]
    fn try_into(self) -> Result<T, Self::Error>;
}

/// 简单且安全的类型转换,在某些情况下可能会以一种受控的方式失败。它是
/// [`TryInto`] 的对偶(reciprocal)。
///
/// 当你在做一种可能轻易就成功、但也可能需要特殊处理的类型转换时,它很有用。
/// 例如,没有办法用 [`From`] trait 把 [`i64`] 转换成 [`i32`],因为 [`i64`]
/// 可能含有 [`i32`] 无法表示的值,那样转换就会丢失数据。这种情况可以通过把
/// [`i64`] 截断为 [`i32`]、或者干脆返回 [`i32::MAX`]、或者用其他方法来处理。
/// [`From`] trait 用于完美的转换,因此 `TryFrom` trait 会在某次类型转换可能
/// 出问题时告知程序员,并让他们自行决定如何处理。
///
/// # 泛型实现
///
/// - `TryFrom<T> for U` 蕴含 [`TryInto`]`<U> for T`
/// - [`try_from`] 是自反的,这意味着 `TryFrom<T> for T` 已被实现且不会失败
///   ——对一个 `T` 类型的值调用 `T::try_from()` 时,其关联的 `Error` 类型是
///   [`Infallible`]。当 [`!`] 类型被稳定化后,[`Infallible`] 与 [`!`] 将等价。
///
/// 在为泛型函数指定 trait 约束时,优先使用 [`TryInto`] 而非 [`TryFrom`],
/// 以确保那些只实现了 [`TryInto`] 的类型也可以被使用。
///
/// `TryFrom<T>` 可以如下实现:
///
/// ```
/// struct GreaterThanZero(i32);
///
/// impl TryFrom<i32> for GreaterThanZero {
///     type Error = &'static str;
///
///     fn try_from(value: i32) -> Result<Self, Self::Error> {
///         if value <= 0 {
///             Err("GreaterThanZero only accepts values greater than zero!")
///         } else {
///             Ok(GreaterThanZero(value))
///         }
///     }
/// }
/// ```
///
/// # 示例
///
/// 如前所述,[`i32`] 实现了 `TryFrom<`[`i64`]`>`:
///
/// ```
/// let big_number = 1_000_000_000_000i64;
/// // 静默地截断 `big_number`,需要事后检测并处理这次截断。
/// let smaller_number = big_number as i32;
/// assert_eq!(smaller_number, -727379968);
///
/// // 返回一个错误,因为 `big_number` 太大,放不进 `i32`。
/// let try_smaller_number = i32::try_from(big_number);
/// assert!(try_smaller_number.is_err());
///
/// // 返回 `Ok(3)`。
/// let try_successful_smaller_number = i32::try_from(3);
/// assert!(try_successful_smaller_number.is_ok());
/// ```
///
/// [`try_from`]: TryFrom::try_from
#[rustc_diagnostic_item = "TryFrom"]
#[stable(feature = "try_from", since = "1.34.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
pub const trait TryFrom<T>: Sized {
    /// 转换出错时返回的类型。
    #[stable(feature = "try_from", since = "1.34.0")]
    type Error;

    /// 执行转换。
    #[stable(feature = "try_from", since = "1.34.0")]
    #[rustc_diagnostic_item = "try_from_fn"]
    fn try_from(value: T) -> Result<Self, Self::Error>;
}

////////////////////////////////////////////////////////////////////////////////
// 泛型实现
////////////////////////////////////////////////////////////////////////////////

// AsRef 在 & 之上提升
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl<T: PointeeSized, U: PointeeSized> const AsRef<U> for &T
where
    T: [const] AsRef<U>,
{
    #[inline]
    fn as_ref(&self) -> &U {
        <T as AsRef<U>>::as_ref(*self)
    }
}

// AsRef 在 &mut 之上提升
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl<T: PointeeSized, U: PointeeSized> const AsRef<U> for &mut T
where
    T: [const] AsRef<U>,
{
    #[inline]
    fn as_ref(&self) -> &U {
        <T as AsRef<U>>::as_ref(*self)
    }
}

// FIXME (#45742):用下面这条更通用的实现替换上面针对 &/&mut 的那些实现:
// // AsRef 在 Deref 之上提升
// impl<D: ?Sized + Deref<Target: AsRef<U>>, U: ?Sized> AsRef<U> for D {
//     fn as_ref(&self) -> &U {
//         self.deref().as_ref()
//     }
// }

// AsMut 在 &mut 之上提升
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl<T: PointeeSized, U: PointeeSized> const AsMut<U> for &mut T
where
    T: [const] AsMut<U>,
{
    #[inline]
    fn as_mut(&mut self) -> &mut U {
        (*self).as_mut()
    }
}

// FIXME (#45742):用下面这条更通用的实现替换上面针对 &mut 的那条实现:
// // AsMut 在 DerefMut 之上提升
// impl<D: ?Sized + Deref<Target: AsMut<U>>, U: ?Sized> AsMut<U> for D {
//     fn as_mut(&mut self) -> &mut U {
//         self.deref_mut().as_mut()
//     }
// }

// From 蕴含 Into
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl<T, U> const Into<U> for T
where
    U: [const] From<T>,
{
    /// 调用 `U::from(self)`。
    ///
    /// 也就是说,这次转换具体做什么,完全取决于
    /// <code>[From]&lt;T&gt; for U</code> 的实现。
    #[inline]
    #[track_caller]
    fn into(self) -> U {
        U::from(self)
    }
}

// From(因而 Into)是自反的
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl<T> const From<T> for T {
    /// 原样返回参数。
    #[inline(always)]
    fn from(t: T) -> T {
        t
    }
}

/// **稳定性说明:** 这条 impl 目前尚不存在,但我们正在为将来添加它
/// “预留位置”。详见 [rust-lang/rust#64715][#64715]。
///
/// [#64715]: https://github.com/rust-lang/rust/issues/64715
#[stable(feature = "convert_infallible", since = "1.34.0")]
#[rustc_reservation_impl = "permitting this impl would forbid us from adding \
                            `impl<T> From<!> for T` later; see rust-lang/rust#64715 for details"]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl<T> const From<!> for T {
    fn from(t: !) -> T {
        t
    }
}

// TryFrom 蕴含 TryInto
#[stable(feature = "try_from", since = "1.34.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl<T, U> const TryInto<U> for T
where
    U: [const] TryFrom<T>,
{
    type Error = U::Error;

    #[inline]
    fn try_into(self) -> Result<U, U::Error> {
        U::try_from(self)
    }
}

// 不会失败(infallible)的转换,在语义上等价于错误类型为无人居住类型
// (uninhabited)的可失败转换。
#[stable(feature = "try_from", since = "1.34.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl<T, U> const TryFrom<U> for T
where
    U: [const] Into<T>,
{
    type Error = Infallible;

    #[inline]
    fn try_from(value: U) -> Result<Self, Self::Error> {
        Ok(U::into(value))
    }
}

////////////////////////////////////////////////////////////////////////////////
// 具体类型的实现
////////////////////////////////////////////////////////////////////////////////

#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl<T> const AsRef<[T]> for [T] {
    #[inline(always)]
    fn as_ref(&self) -> &[T] {
        self
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl<T> const AsMut<[T]> for [T] {
    #[inline(always)]
    fn as_mut(&mut self) -> &mut [T] {
        self
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl const AsRef<str> for str {
    #[inline(always)]
    fn as_ref(&self) -> &str {
        self
    }
}

#[stable(feature = "as_mut_str_for_str", since = "1.51.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl const AsMut<str> for str {
    #[inline(always)]
    fn as_mut(&mut self) -> &mut str {
        self
    }
}

////////////////////////////////////////////////////////////////////////////////
// “无错误”的错误类型
////////////////////////////////////////////////////////////////////////////////

/// 面向永远不会发生的错误的错误类型。
///
/// 由于本枚举没有任何变体,所以这个类型的值实际上永远不可能存在。这对于那些
/// 使用 [`Result`] 并把错误类型参数化的泛型 API 很有用,可以用来表明其结果
/// 永远是 [`Ok`]。
///
/// 例如,[`TryFrom`] trait(一种返回 [`Result`] 的转换)对所有“存在反向
/// [`Into`] 实现”的类型都有一条覆盖性实现。
///
/// ```ignore (illustrates std code, duplicating the impl in a doctest would be an error)
/// impl<T, U> TryFrom<U> for T where U: Into<T> {
///     type Error = Infallible;
///
///     fn try_from(value: U) -> Result<Self, Infallible> {
///         Ok(U::into(value))  // 永远不会返回 `Err`
///     }
/// }
/// ```
///
/// # 未来兼容性
///
/// 本枚举所扮演的角色与[`!`“never”类型][never]相同,而后者在本版本的 Rust
/// 中尚不稳定。当 `!` 被稳定化后,我们计划把 `Infallible` 做成它的一个类型
/// 别名:
///
/// ```ignore (illustrates future std change)
/// pub type Infallible = !;
/// ```
///
/// ……并最终弃用 `Infallible`。
///
/// 不过,有一种情形可以在 `!` 作为完备类型被稳定化之前就使用 `!` 语法:
/// 在函数返回类型的位置。具体来说,可以为两种不同的函数指针类型分别提供实现:
///
/// ```
/// trait MyTrait {}
/// impl MyTrait for fn() -> ! {}
/// impl MyTrait for fn() -> std::convert::Infallible {}
/// ```
///
/// 由于 `Infallible` 是一个枚举,这段代码是有效的。然而,一旦 `Infallible`
/// 变成 never 类型的别名,这两条 `impl` 就会开始重叠,从而被该语言的 trait
/// 一致性(coherence)规则所禁止。
#[stable(feature = "convert_infallible", since = "1.34.0")]
#[derive(Copy)]
pub enum Infallible {}

#[stable(feature = "convert_infallible", since = "1.34.0")]
#[rustc_const_unstable(feature = "const_clone", issue = "142757")]
impl const Clone for Infallible {
    fn clone(&self) -> Infallible {
        match *self {}
    }
}

#[stable(feature = "convert_infallible", since = "1.34.0")]
impl fmt::Debug for Infallible {
    fn fmt(&self, _: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {}
    }
}

#[stable(feature = "convert_infallible", since = "1.34.0")]
impl fmt::Display for Infallible {
    fn fmt(&self, _: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {}
    }
}

#[stable(feature = "str_parse_error2", since = "1.8.0")]
impl Error for Infallible {}

#[stable(feature = "convert_infallible", since = "1.34.0")]
#[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
impl const PartialEq for Infallible {
    fn eq(&self, _: &Infallible) -> bool {
        match *self {}
    }
}

#[stable(feature = "convert_infallible", since = "1.34.0")]
#[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
impl const Eq for Infallible {}

#[stable(feature = "convert_infallible", since = "1.34.0")]
#[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
impl const PartialOrd for Infallible {
    fn partial_cmp(&self, _other: &Self) -> Option<crate::cmp::Ordering> {
        match *self {}
    }
}

#[stable(feature = "convert_infallible", since = "1.34.0")]
#[rustc_const_unstable(feature = "const_cmp", issue = "143800")]
impl const Ord for Infallible {
    fn cmp(&self, _other: &Self) -> crate::cmp::Ordering {
        match *self {}
    }
}

#[stable(feature = "convert_infallible", since = "1.34.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl const From<!> for Infallible {
    #[inline]
    fn from(x: !) -> Self {
        x
    }
}

#[stable(feature = "convert_infallible_hash", since = "1.44.0")]
impl Hash for Infallible {
    fn hash<H: Hasher>(&self, _: &mut H) {
        match *self {}
    }
}
