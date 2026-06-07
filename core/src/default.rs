//! 面向具有默认值的类型的 `Default` trait。

#![stable(feature = "rust1", since = "1.0.0")]

use crate::ascii::Char as AsciiChar;

/// 一个为类型赋予有用默认值的 trait。
///
/// 有时,你想回退到某种默认值,而并不特别在意它具体是什么。这种情况在定义
/// 一组选项的 `struct` 上经常出现:
///
/// ```
/// # #[allow(dead_code)]
/// struct SomeOptions {
///     foo: i32,
///     bar: f32,
/// }
/// ```
///
/// 我们怎样才能定义一些默认值呢?你可以使用 `Default`:
///
/// ```
/// # #[allow(dead_code)]
/// #[derive(Default)]
/// struct SomeOptions {
///     foo: i32,
///     bar: f32,
/// }
///
/// fn main() {
///     let options: SomeOptions = Default::default();
/// }
/// ```
///
/// 现在,你就拿到了所有的默认值。Rust 为各种原始类型实现了 `Default`。
///
/// 如果你想覆盖某个特定选项,同时仍然保留其余的默认值:
///
/// ```
/// # #[allow(dead_code)]
/// # #[derive(Default)]
/// # struct SomeOptions {
/// #     foo: i32,
/// #     bar: f32,
/// # }
/// fn main() {
///     let options = SomeOptions { foo: 42, ..Default::default() };
/// }
/// ```
///
/// ## 可派生(Derivable)
///
/// 如果该类型的所有字段都实现了 `Default`,本 trait 就可以配合 `#[derive]`
/// 使用。`derive` 出来的实现会为每个字段的类型使用其默认值。
///
/// ### `enum`
///
/// 在 `enum` 上使用 `#[derive(Default)]` 时,你需要选择哪一个单元变体
/// (unit variant)作为默认。做法是把 `#[default]` 属性放在该变体上。
///
/// ```
/// #[derive(Default)]
/// enum Kind {
///     #[default]
///     A,
///     B,
///     C,
/// }
/// ```
///
/// 你不能在非单元变体或 non-exhaustive 变体上使用 `#[default]` 属性。
///
/// `#[default]` 属性在 Rust 1.62.0 中被稳定化。
///
/// ## 如何实现 `Default`?
///
/// 为 `default()` 方法提供一个实现,让它返回你的类型中应当作为默认的那个值:
///
/// ```
/// # #![allow(dead_code)]
/// enum Kind {
///     A,
///     B,
///     C,
/// }
///
/// impl Default for Kind {
///     fn default() -> Self { Kind::A }
/// }
/// ```
///
/// # 示例
///
/// ```
/// # #[allow(dead_code)]
/// #[derive(Default)]
/// struct SomeOptions {
///     foo: i32,
///     bar: f32,
/// }
/// ```
#[rustc_diagnostic_item = "Default"]
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_unstable(feature = "const_default", issue = "143894")]
pub const trait Default: Sized {
    /// 返回一个类型的“默认值”。
    ///
    /// 默认值通常是某种初始值、单位元(identity value),或者其他任何作为
    /// 默认讲得通的东西。
    ///
    /// # 示例
    ///
    /// 使用内建的默认值:
    ///
    /// ```
    /// let i: i8 = Default::default();
    /// let (x, y): (Option<String>, f64) = Default::default();
    /// let (a, b, (c, d)): (i32, u32, (bool, bool)) = Default::default();
    /// ```
    ///
    /// 自定义你自己的默认值:
    ///
    /// ```
    /// # #[allow(dead_code)]
    /// enum Kind {
    ///     A,
    ///     B,
    ///     C,
    /// }
    ///
    /// impl Default for Kind {
    ///     fn default() -> Self { Kind::A }
    /// }
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_diagnostic_item = "default_fn"]
    fn default() -> Self;
}

/// 生成 `Default` trait 实现的派生宏。
#[rustc_builtin_macro(Default, attributes(default))]
#[stable(feature = "builtin_macro_prelude", since = "1.38.0")]
#[allow_internal_unstable(core_intrinsics)]
pub macro Default($item:item) {
    /* 编译器内建 */
}

macro_rules! default_impl {
    ($t:ty, $v:expr, $doc:tt) => {
        #[stable(feature = "rust1", since = "1.0.0")]
        #[rustc_const_unstable(feature = "const_default", issue = "143894")]
        impl const Default for $t {
            #[inline(always)]
            #[doc = $doc]
            fn default() -> $t {
                $v
            }
        }
    };
}

default_impl! { (), (), "返回 `()` 的默认值" }
default_impl! { bool, false, "返回默认值 `false`" }
default_impl! { char, '\x00', "返回默认值 `\\x00`" }
default_impl! { AsciiChar, AsciiChar::Null, "返回默认值 `Null`" }

default_impl! { usize, 0, "返回默认值 `0`" }
default_impl! { u8, 0, "返回默认值 `0`" }
default_impl! { u16, 0, "返回默认值 `0`" }
default_impl! { u32, 0, "返回默认值 `0`" }
default_impl! { u64, 0, "返回默认值 `0`" }
default_impl! { u128, 0, "返回默认值 `0`" }

default_impl! { isize, 0, "返回默认值 `0`" }
default_impl! { i8, 0, "返回默认值 `0`" }
default_impl! { i16, 0, "返回默认值 `0`" }
default_impl! { i32, 0, "返回默认值 `0`" }
default_impl! { i64, 0, "返回默认值 `0`" }
default_impl! { i128, 0, "返回默认值 `0`" }

default_impl! { f16, 0.0f16, "返回默认值 `0.0`" }
default_impl! { f32, 0.0f32, "返回默认值 `0.0`" }
default_impl! { f64, 0.0f64, "返回默认值 `0.0`" }
default_impl! { f128, 0.0f128, "返回默认值 `0.0`" }
