/// 析构函数中的自定义代码。
///
/// 当一个值不再被需要时,Rust 会对该值运行一个“析构函数”(destructor)。值
/// 不再被需要最常见的情形是它离开了作用域。析构函数也可能在其他场景下运行,
/// 但此处的示例只聚焦于作用域。要了解其他情形,请参阅 [reference][the reference]
/// 中关于析构函数的章节。
///
/// [the reference]: https://doc.rust-lang.org/reference/destructors.html
///
/// 一个析构函数由两部分组成:
/// - 如果该类型实现了这个特殊的 `Drop` trait,则对该值调用一次 `Drop::drop`。
/// - 自动生成的“drop glue”(析构胶水代码),它会递归调用该值所有字段的析构函数。
///
/// 由于 Rust 会自动调用所有内含字段的析构函数,大多数情况下你无需自行实现
/// `Drop`。但在某些场景下它很有用,例如对于直接管理某种资源的类型。这种资源
/// 可能是内存,可能是文件描述符,也可能是网络套接字。一旦该类型的值不再被使用,
/// 它就应当通过释放内存、关闭文件或套接字来“清理”自己的资源。这正是析构函数
/// 要做的事,也就是 `Drop::drop` 的职责。
///
/// ## 示例
///
/// 为了看到析构函数的实际效果,我们来看看下面这个程序:
///
/// ```rust
/// struct HasDrop;
///
/// impl Drop for HasDrop {
///     fn drop(&mut self) {
///         println!("Dropping HasDrop!");
///     }
/// }
///
/// struct HasTwoDrops {
///     one: HasDrop,
///     two: HasDrop,
/// }
///
/// impl Drop for HasTwoDrops {
///     fn drop(&mut self) {
///         println!("Dropping HasTwoDrops!");
///     }
/// }
///
/// fn main() {
///     let _x = HasTwoDrops { one: HasDrop, two: HasDrop };
///     println!("Running!");
/// }
/// ```
///
/// Rust 会先为 `_x` 调用 `Drop::drop`,然后再为 `_x.one` 和 `_x.two` 分别调用,
/// 也就是说运行这段程序会打印
///
/// ```text
/// Running!
/// Dropping HasTwoDrops!
/// Dropping HasDrop!
/// Dropping HasDrop!
/// ```
///
/// 即使我们移除 `HasTwoDrop` 的 `Drop` 实现,它各个字段的析构函数仍会被调用。
/// 这会输出
///
/// ```test
/// Running!
/// Dropping HasDrop!
/// Dropping HasDrop!
/// ```
///
/// ## 你不能自己调用 `Drop::drop`
///
/// 由于 `Drop::drop` 用于清理一个值,在该方法被调用之后再使用这个值可能是危险的。
/// 又因为 `Drop::drop` 并不取得其输入的所有权(它只接收 `&mut self`),Rust 通过
/// 不允许你直接调用 `Drop::drop` 来防止误用。
///
/// 换句话说,如果你试图在上面的例子里显式调用 `Drop::drop`,你会得到一个编译错误。
///
/// 如果你想显式调用某个值的析构函数,可以改用 [`mem::drop`]。
///
/// [`mem::drop`]: drop
///
/// ## 析构顺序(Drop order)
///
/// 那么我们的两个 `HasDrop` 哪个先被析构呢?对于结构体而言,析构顺序与字段声明
/// 顺序一致:先 `one`,后 `two`。如果你想亲自试一试,可以修改上面的 `HasDrop`
/// 让它包含一些数据(比如一个整数),然后在 `Drop` 内的 `println!` 中用到它。
/// 这一行为由语言层面保证。
///
/// 与结构体不同,局部变量按声明的逆序被析构:
///
/// ```rust
/// struct Foo;
///
/// impl Drop for Foo {
///     fn drop(&mut self) {
///         println!("Dropping Foo!")
///     }
/// }
///
/// struct Bar;
///
/// impl Drop for Bar {
///     fn drop(&mut self) {
///         println!("Dropping Bar!")
///     }
/// }
///
/// fn main() {
///     let _foo = Foo;
///     let _bar = Bar;
/// }
/// ```
///
/// 这会打印
///
/// ```text
/// Dropping Bar!
/// Dropping Foo!
/// ```
///
/// 完整规则请参阅 [reference][the reference]。
///
/// [the reference]: https://doc.rust-lang.org/reference/destructors.html
///
/// ## `Copy` 与 `Drop` 互斥
///
/// 你不能在同一个类型上同时实现 [`Copy`] 和 `Drop`。`Copy` 类型会被编译器隐式
/// 复制,这使得人们很难预测析构函数会在何时、以及被执行多少次。因此这类类型
/// 不能拥有析构函数。
///
/// ## 析构检查(Drop check)
///
/// 析构与借用检查器之间有着微妙的相互作用:当某个此类型的变量离开作用域、类型
/// `T` 被隐式析构时,借用检查器需要确保此刻调用 `T` 的析构函数是安全的。特别地,
/// 它还需要确保递归析构 `T` 的所有字段也是安全的。例如,像下面这样的代码必须被
/// 拒绝,这一点至关重要:
///
/// ```compile_fail,E0597
/// use std::cell::Cell;
///
/// struct S<'a>(Cell<Option<&'a S<'a>>>, Box<i32>);
/// impl Drop for S<'_> {
///     fn drop(&mut self) {
///         if let Some(r) = self.0.get() {
///             // 打印 `r` 中那个 `Box` 的内容。
///             println!("{}", r.1);
///         }
///     }
/// }
///
/// fn main() {
///     // 构造两个互相指向对方的 `S`。
///     let s1 = S(Cell::new(None), Box::new(42));
///     let s2 = S(Cell::new(Some(&s1)), Box::new(42));
///     s1.0.set(Some(&s2));
///     // 现在两者都将被析构。但无论哪一个是第二个被析构的,
///     // 它都会访问另一个里面的 `Box`,
///     // 这就是 use-after-free(释放后使用)!
/// }
/// ```
///
/// Nomicon 中更详尽地讨论了 [drop check 的必要性][drop check]。
///
/// 为了拒绝这类代码,“drop check”分析会判定在 `T` 被析构时哪些类型和生命周期
/// 仍需保持存活(live)。这一分析的确切细节目前尚未被稳定保证,且**可能变化**。
/// 当前该分析的工作方式如下:
/// - 如果 `T` 没有 drop glue,则平凡地不要求任何东西存活。当 `T` 本身及其(递归)
///   字段都没有析构函数(`impl Drop`)时即属此情形。无论字段类型为何,[`PhantomData`]、
///   长度为 0 的数组以及 [`ManuallyDrop`] 都被视作永远没有析构函数。
/// - 如果 `T` 有 drop glue,则对于被 `T` 任意字段所 *拥有(owned)* 的所有类型 `U`,
///   递归地把在 `U` 被析构时需要存活的类型和生命周期纳入要求。被拥有的类型集合
///   通过递归遍历 `T` 来确定:
///   - 递归穿过 `PhantomData`、`Box`、元组以及数组(长度为 0 的数组除外)。
///   - 在引用与裸指针类型以及函数指针、函数项处停下;它们不拥有任何东西。
///   - 在非复合类型处停下(在当前上下文中仍保持泛型的类型参数,以及整数、`bool`
///     等基础类型);这些类型被视为被拥有。
///   - 遇到带 `impl Drop` 的 ADT 时,在此停下;该类型被视为被拥有。
///   - 遇到不带 `impl Drop` 的 ADT 时,递归下降到它的字段。(对 `enum` 而言,
///     考虑所有变体的所有字段。)
/// - 此外,如果 `T` 实现了 `Drop`,那么 `T` 的所有泛型参数(生命周期与类型)都
///   必须存活。
///
/// 在上面的例子中,最后一条规则意味着 `S<'a>` 被析构时 `'a` 必须存活,因此该例
/// 被拒绝。如果我们移除 `impl Drop`,这条存活性要求就消失了,该例也就被接受。
///
/// 有一种不稳定的方式可以让类型从最后一条规则中豁免,这被称为“drop check
/// eyepatch”或 `may_dangle`。关于这个仅限 nightly 的特性的更多细节,请参阅
/// [Nomicon 中的讨论][nomicon]。
///
/// [`ManuallyDrop`]: crate::mem::ManuallyDrop
/// [`PhantomData`]: crate::marker::PhantomData
/// [drop check]: ../../nomicon/dropck.html
/// [nomicon]: ../../nomicon/phantom-data.html#an-exception-the-special-case-of-the-standard-library-and-its-unstable-may_dangle
#[lang = "drop"]
#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_unstable(feature = "const_destruct", issue = "133214")]
pub const trait Drop {
    /// 为此类型执行析构函数。
    ///
    /// 当值离开作用域时,此方法会被隐式调用,而不能被显式调用(显式调用会产生
    /// 编译错误 [E0040])。不过,可以用 prelude 中的 [`mem::drop`] 函数来调用
    /// 其参数的 `Drop` 实现。
    ///
    /// 当此方法被调用时,`self` 尚未被释放(deallocate),释放只在该方法结束之后
    /// 才发生。如果不是这样,`self` 就会成为一个悬垂引用。
    ///
    /// # Panics
    ///
    /// 实现通常应避免 [`panic!`],因为 `drop()` 本身可能在 panic 引发的栈展开
    /// 过程中被调用,若此时 `drop()` 又 panic(即“双重 panic”,double panic),
    /// 很可能会直接中止(abort)整个程序。可以先检查 [`panicking()`];对于那种
    /// 旨在报告“你在析构前没有正确收尾使用该值”这类 bug 的 `Drop` 实现来说,这
    /// 么做或许是可取的;但大多数类型都应当只是清理它们所拥有的分配或其他资源,
    /// 然后无论自身处于何种状态都正常地从 `drop()` 返回。
    ///
    /// 注意,即使本方法发生 panic,该值仍被视为已被析构;你绝不能让 `drop` 再次
    /// 被调用。通常这由编译器自动处理,但在使用 unsafe 代码时,有时会无意中发生,
    /// 尤其是在使用 [`ptr::drop_in_place`] 时。
    ///
    /// [E0040]: ../../error_codes/E0040.html
    /// [`panic!`]: crate::panic!
    /// [`panicking()`]: ../../std/thread/fn.panicking.html
    /// [`mem::drop`]: drop
    /// [`ptr::drop_in_place`]: crate::ptr::drop_in_place
    #[stable(feature = "rust1", since = "1.0.0")]
    fn drop(&mut self);
}
