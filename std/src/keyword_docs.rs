#[doc(keyword = "as")]
//
/// 在类型之间转换、重命名导入，或限定到关联项的路径。
///
/// # Type casting
///
/// `as` 最常用于把基本类型转换为其他基本类型，但它还有别的用途，包括把指针转换为地址、
/// 把地址转换为指针，以及把指针转换为其他指针。
///
/// ```rust
/// let thing1: u8 = 89.0 as u8;
/// assert_eq!('B' as u32, 66);
/// assert_eq!(thing1 as char, 'Y');
/// let thing2: f32 = thing1 as f32 + 10.5;
/// assert_eq!(true as u8 + thing2 as u8, 100);
/// ```
///
/// 一般来说，凡是可以通过类型标注完成的转换，都可以用 `as` 来做，所以与其写
/// `let x: u32 = 123`，你也可以写 `let x = 123 as u32`（注意：在那种情形下 `let x: u32
/// = 123` 才是最佳写法）。但反过来则不成立；显式使用 `as` 允许一些隐式不被允许的强制转换，
/// 比如改变裸指针的类型，或把闭包转换为裸指针。
///
/// `as` 可以看作 `From` 和 `Into` 的基础原语：`as` 只能用于基本类型
/// （`u8`、`bool`、`str`、指针……），而 `From` 和 `Into` 还能用于 `String` 或 `Vec`
/// 这样的类型。
///
/// 当目标类型可以被推断时，`as` 也可以配合 `_` 占位符使用。注意这可能破坏类型推断，
/// 通常这类代码应当为两边都写出明确的类型，以兼顾清晰性和稳定性。它最有用的场景是
/// 用 `as *const _` 或 `as *mut _` 转换指针，不过相对 `as *const _` 更推荐使用
/// [`cast`][const-cast] 方法，`as *mut _` 也[同样][mut-cast]如此：这些方法能让意图更清晰。
///
/// # Renaming imports
///
/// `as` 也用于在 [`use`] 和 [`extern crate`][`crate`] 语句中重命名导入：
///
/// ```
/// # #[allow(unused_imports)]
/// use std::{mem as memory, net as network};
/// // 现在你可以用名字 `memory` 和 `network` 来引用 `std::mem` 和 `std::net`。
/// ```
///
/// # Qualifying paths
///
/// 你还会发现，对于 `From`、`Into`，乃至所有 trait，`as` 都用于构成
/// _完全限定路径（fully qualified path）_，这是一种消除关联项（即函数、常量和类型）歧义的手段。
/// 例如，如果某个类型实现了两个具有相同方法名的 trait（如 `Into::<u32>::into` 和
/// `Into::<u64>::into`），你可以用 `<MyThing as Into<u32>>::into(my_thing)`[^as-use-from]
/// 来明确你要用哪个方法。这写起来相当啰嗦，但幸好 Rust 的类型推断通常能让你免于此，
/// 尽管偶尔仍有必要，尤其是对于像 `Into::into` 这样返回泛型类型的方法，或者不接收
/// `self` 的方法。它更常见于宏中，可在那里提供必要的卫生性（hygiene）。
///
/// [^as-use-from]: 你大概永远不该把这种语法用于 `Into`，而应改写为
/// `T::from(my_thing)`。只是标准库里恰好没有什么好的例子能展示这种语法。另外，在撰写本文时，
/// 编译器倾向于建议用完全限定路径来修复有歧义的 `Into::into` 调用，所以这个例子但愿你会感到熟悉。
///
/// # Further reading
///
/// 关于 `as` 的更多能力，请参阅 Reference 中关于[类型转换表达式][type cast expressions]、
/// [重命名导入实体][renaming imported entities]、[重命名 `extern` crate][renaming `extern` crates]
/// 和[限定路径][qualified paths]的内容。
///
/// [type cast expressions]: ../reference/expressions/operator-expr.html#type-cast-expressions
/// [renaming imported entities]: https://doc.rust-lang.org/reference/items/use-declarations.html#as-renames
/// [renaming `extern` crates]: https://doc.rust-lang.org/reference/items/extern-crates.html#r-items.extern-crate.as
/// [qualified paths]: ../reference/paths.html#qualified-paths
/// [`crate`]: keyword.crate.html
/// [`use`]: keyword.use.html
/// [const-cast]: pointer::cast
/// [mut-cast]: primitive.pointer.html#method.cast-1
mod as_keyword {}

#[doc(keyword = "break")]
//
/// 从循环或带标签的块中提前退出。
///
/// 当遇到 `break` 时，关联循环体的执行会立即终止。
///
/// ```rust
/// let mut last = 0;
///
/// for x in 1..100 {
///     if x > 12 {
///         break;
///     }
///     last = x;
/// }
///
/// assert_eq!(last, 12);
/// println!("{last}");
/// ```
///
/// break 表达式通常关联到包含该 `break` 的最内层循环，但可以用标签来指定它影响哪一个外层循环。
///
/// ```rust
/// 'outer: for i in 1..=5 {
///     println!("outer iteration (i): {i}");
///
///     '_inner: for j in 1..=200 {
///         println!("    inner iteration (j): {j}");
///         if j >= 3 {
///             // 从内层循环中跳出，让外层循环继续。
///             break;
///         }
///         if i >= 2 {
///             // 从外层循环中跳出，直接到 "Bye"。
///             break 'outer;
///         }
///     }
/// }
/// println!("Bye.");
/// ```
///
/// 当与 `loop` 关联时，break 表达式可以用来从该循环返回一个值。
/// 这只对 `loop` 有效，对任何其他类型的循环都无效。
/// 如果 `break;` 没有指定值，它返回 `()`。
/// 同一个循环里的每个 `break` 都必须返回相同的类型。
///
/// ```rust
/// let (mut a, mut b) = (1, 1);
/// let result = loop {
///     if b > 10 {
///         break b;
///     }
///     let c = a + b;
///     a = b;
///     b = c;
/// };
/// // 斐波那契数列中第一个超过 10 的数：
/// assert_eq!(result, 13);
/// println!("{result}");
/// ```
///
/// 也可以从任意*带标签*的块中退出，并提前返回值。
/// 如果 `break;` 没有指定值，它返回 `()`。
///
/// ```rust
/// let inputs = vec!["Cow", "Cat", "Dog", "Snake", "Cod"];
///
/// let mut results = vec![];
/// for input in inputs {
///     let result = 'filter: {
///         if input.len() > 3 {
///             break 'filter Err("Too long");
///         };
///
///         if !input.contains("C") {
///             break 'filter Err("No Cs");
///         };
///
///         Ok(input.to_uppercase())
///     };
///
///     results.push(result);
/// }
///
/// // [Ok("COW"), Ok("CAT"), Err("No Cs"), Err("Too long"), Ok("COD")]
/// println!("{:?}", results)
/// ```
///
/// 更多细节请参阅 [Reference 中关于 "break expression" 的内容][Reference on "break expression"]
/// 和 [Reference 中关于 "break and loop values" 的内容][Reference on "break and
/// loop values"]。
///
/// [Reference on "break expression"]: ../reference/expressions/loop-expr.html#break-expressions
/// [Reference on "break and loop values"]:
/// ../reference/expressions/loop-expr.html#break-and-loop-values
mod break_keyword {}

#[doc(keyword = "const")]
//
/// 编译期常量、编译期块、可在编译期求值的函数，以及裸指针。
///
/// ## Compile-time constants
///
/// 有时某个值在整个程序中被多次使用，反复复制它会很不方便。更何况，把它做成一个被传递到
/// 每个需要它的函数的变量，并不总是可行或可取的。在这些情况下，`const` 关键字提供了一种
/// 避免代码重复的便捷替代方案：
///
/// ```rust
/// const THING: u32 = 0xABAD1DEA;
///
/// let foo = 123 + THING;
/// ```
///
/// 常量必须显式标注类型；与 `let` 不同，你不能省略类型而让编译器自行推断。任何常量值都可以
/// 用 `const` 定义，实践中这涵盖了大多数适合作为常量的东西（`const fn` 除外）。例如，
/// 你不能把 [`File`] 作为 `const`。
///
/// [`File`]: crate::fs::File
///
/// 常量中唯一允许的生命周期是 `'static`，它是涵盖 Rust 程序中所有其他生命周期的生命周期。
/// 例如，如果你想定义一个常量字符串，它看起来会是这样：
///
/// ```rust
/// const WORDS: &'static str = "hello rust!";
/// ```
///
/// 多亏了静态生命周期省略（static lifetime elision），你通常不必显式写出 `'static`：
///
/// ```rust
/// const WORDS: &str = "hello convenience!";
/// ```
///
/// `const` 项看起来与 `static` 项非常相似，这带来了一些关于何时该用哪一个的困惑。
/// 简单来说，常量会在使用处被内联，因此使用它们等同于直接把 `const` 的名字替换为它的值。
/// 而静态变量则指向内存中的单一位置，所有访问共享该位置。这意味着，与常量不同，静态变量
/// 不能有析构函数，并且在整个代码库中表现为单一的一个值。
///
/// 常量和静态变量一样，应当始终采用 `SCREAMING_SNAKE_CASE` 命名。
///
/// 关于 `const` 的更多细节，参见 [Rust Book] 或 [Reference]。
///
/// ## Compile-time blocks
///
/// `const` 关键字也可以用来定义一个在编译期求值的代码块。这对于确保某些计算在优化发生之前
/// 以及在运行时之前完成很有用。更多细节见 [Reference][const-blocks]。
///
/// ## Compile-time evaluable functions
///
/// `const` 关键字的另一个主要用途是用在 `const fn` 上。这把一个函数标记为可在 `const` 或
/// `static` 项的主体以及数组初始化器（通常称为"const 上下文"）中调用。`const fn` 在其能
/// 执行的操作集合上受到限制，以确保它们可以在编译期被求值。更多细节见 [Reference][const-eval]。
///
/// 把一个 `fn` 变成 `const fn` 对该函数的运行时使用没有任何影响。
///
/// ## Other uses of `const`
///
/// `const` 关键字还与 `mut` 一起用于裸指针，如 `*const T` 和 `*mut T` 所示。关于 `const`
/// 在裸指针中的用法，可在 [pointer 基本类型][pointer primitive]的 Rust 文档中读到更多。
///
/// [pointer primitive]: pointer
/// [Rust Book]: ../book/ch03-01-variables-and-mutability.html#constants
/// [Reference]: ../reference/items/constant-items.html
/// [const-blocks]: ../reference/expressions/block-expr.html#const-blocks
/// [const-eval]: ../reference/const_eval.html
mod const_keyword {}

#[doc(keyword = "continue")]
//
/// 跳到循环的下一次迭代。
///
/// 当遇到 `continue` 时，当前迭代被终止，控制权返回到循环头部，通常会继续下一次迭代。
///
/// ```rust
/// // 通过跳过偶数来打印奇数
/// for number in 1..=10 {
///     if number % 2 == 0 {
///         continue;
///     }
///     println!("{number}");
/// }
/// ```
///
/// 与 `break` 类似，`continue` 通常关联到最内层的包含循环，但可以用标签来指定受影响的循环。
///
/// ```rust
/// // 打印 30 以内、个位数 <= 5 的奇数
/// 'tens: for ten in 0..3 {
///     '_units: for unit in 0..=9 {
///         if unit % 2 == 0 {
///             continue;
///         }
///         if unit > 5 {
///             continue 'tens;
///         }
///         println!("{}", ten * 10 + unit);
///     }
/// }
/// ```
///
/// 更多细节请参阅 reference 中的 [continue 表达式][continue expressions]。
///
/// [continue expressions]: ../reference/expressions/loop-expr.html#continue-expressions
mod continue_keyword {}

#[doc(keyword = "crate")]
//
/// 一个 Rust 二进制程序或库。
///
/// `crate` 关键字的主要用途是作为 `extern crate` 声明的一部分，用来指定对当前 crate 之外的
/// 某个 crate 的依赖。Crate 是 Rust 代码的基本编译单元，可以看作库或项目。关于 crate 的更多
/// 内容可在 [Reference] 中读到。
///
/// ```rust ignore
/// extern crate rand;
/// extern crate my_crate as thing;
/// extern crate std; // 隐式地添加到每个 Rust 项目的根
/// ```
///
/// `as` 关键字可用于改变这个 crate 在你的项目中被引用的名字。如果 crate 名字包含短横线，
/// 它会被隐式导入，并将短横线替换为下划线。
///
/// `crate` 还可以与 `pub` 一起使用，表明它所附着的项仅对同一 crate 内的其他成员公开。
///
/// ```rust
/// # #[allow(unused_imports)]
/// pub(crate) use std::io::Error as IoError;
/// pub(crate) enum CoolMarkerType { }
/// pub struct PublicThing {
///     pub(crate) semi_secret_thing: bool,
/// }
/// ```
///
/// `crate` 也用于表示一个模块的绝对路径，其中 `crate` 指代当前 crate 的根。例如，
/// `crate::foo::bar` 指的是模块 `foo` 内名为 `bar` 的名字，可在同一 crate 中的任何其他位置使用。
///
/// [Reference]: ../reference/items/extern-crates.html
mod crate_keyword {}

#[doc(keyword = "else")]
//
/// 当 [`if`] 条件求值为 [`false`] 时要求值的表达式。
///
/// `else` 表达式是可选的。当没有提供 else 表达式时，它被假定求值为单元类型 `()`。
///
/// `else` 块求值得到的类型必须与 `if` 块求值得到的类型兼容。
///
/// 如下所示，`else` 后面必须跟着以下之一：`if`、`if let`，或一个块 `{}`，它会返回该表达式的值。
///
/// ```rust
/// let result = if true == false {
///     "oh no"
/// } else if "something" == "other thing" {
///     "oh dear"
/// } else if let Some(200) = "blarg".parse::<i32>().ok() {
///     "uh oh"
/// } else {
///     println!("Sneaky side effect.");
///     "phew, nothing's broken"
/// };
/// ```
///
/// 下面是另一个例子，但这里我们不尝试返回一个表达式：
///
/// ```rust
/// if true == false {
///     println!("oh no");
/// } else if "something" == "other thing" {
///     println!("oh dear");
/// } else if let Some(200) = "blarg".parse::<i32>().ok() {
///     println!("uh oh");
/// } else {
///     println!("phew, nothing's broken");
/// }
/// ```
///
/// 上面的代码_仍然_是一个表达式，但它将始终求值为 `()`。
///
/// 一个 `if` 表达式后面可以跟着的 `else` 块的数量可能没有上限，但如果你有好几个，
/// 那么用 [`match`] 表达式或许更可取。
///
/// 在 [Rust Book] 中阅读更多关于控制流的内容。
///
/// [Rust Book]: ../book/ch03-05-control-flow.html#handling-multiple-conditions-with-else-if
/// [`match`]: keyword.match.html
/// [`false`]: keyword.false.html
/// [`if`]: keyword.if.html
mod else_keyword {}

#[doc(keyword = "enum")]
//
/// 一种可以是若干变体之一的类型。
///
/// Rust 中的枚举类似于 C 等其他编译型语言中的枚举，但有一些重要差异，使其强大得多。
/// 如果你有函数式编程背景，Rust 所称的 enum 更常被称作[代数数据类型][ADT]。
/// 重要的细节在于，每个枚举变体都可以携带与之相关的数据。
///
/// ```rust
/// # struct Coord;
/// enum SimpleEnum {
///     FirstVariant,
///     SecondVariant,
///     ThirdVariant,
/// }
///
/// enum Location {
///     Unknown,
///     Anonymous,
///     Known(Coord),
/// }
///
/// enum ComplexEnum {
///     Nothing,
///     Something(u32),
///     LotsOfThings {
///         usual_struct_stuff: bool,
///         blah: String,
///     }
/// }
///
/// enum EmptyEnum { }
/// ```
///
/// 展示的第一个枚举是你在 C 风格语言中会见到的那种常见枚举。第二个展示了一个假想的例子，
/// 用来存储位置数据，其中 `Coord` 可以是任何所需的其他类型，例如一个结构体。第三个例子
/// 演示了变体可以存储的数据种类，从什么都不存、到元组、再到匿名结构体。
///
/// 实例化枚举变体需要显式地用枚举名作为命名空间，后面跟着它的某个变体。
/// `SimpleEnum::SecondVariant` 就是上面的一个例子。当变体携带数据时，比如 Rust 内置的
/// [`Option`] 类型，数据按类型所描述的方式添加，例如 `Option::Some(123)`。结构体式的变体
/// 也是如此，写起来像 `ComplexEnum::LotsOfThings { usual_struct_stuff:
/// true, blah: "hello!".to_string(), }`。空枚举与 [`!`] 类似，因为它们根本无法被实例化，
/// 主要用于以有趣的方式摆弄类型系统。
///
/// 更多信息请看 [Rust Book] 或 [Reference]。
///
/// [ADT]: https://en.wikipedia.org/wiki/Algebraic_data_type
/// [Rust Book]: ../book/ch06-01-defining-an-enum.html
/// [Reference]: ../reference/items/enumerations.html
mod enum_keyword {}

#[doc(keyword = "extern")]
//
/// 链接到或导入外部代码。
///
/// `extern` 关键字在 Rust 中用于两个地方。一个是与 [`crate`] 关键字配合，让你的 Rust 代码
/// 知道项目中的其他 Rust crate，即 `extern crate lazy_static;`。另一个用途是在外部函数接口（FFI）中。
///
/// `extern` 在 FFI 中用于两种不同的上下文。第一种是以外部块（external block）的形式，用来声明
/// 函数接口，使 Rust 代码能借此调用外部代码。`extern` 的这种用法是 unsafe 的，因为我们是在向
/// 编译器断言所有函数声明都是正确的。如果它们不正确，使用这些项可能导致未定义行为。
///
/// ```rust ignore
/// // SAFETY: 下面给出的函数声明与 `my_c_library` 的头文件一致。
/// #[link(name = "my_c_library")]
/// unsafe extern "C" {
///     fn my_c_function(x: i32) -> bool;
/// }
/// ```
///
/// 这段代码会在运行时尝试在类 Unix 系统上链接 `libmy_c_library.so`，在 Windows 上链接
/// `my_c_library.dll`，如果找不到可链接的东西就会 panic。Rust 代码随后即可像使用任何其他
/// unsafe Rust 函数一样使用 `my_c_function`。与非 Rust 语言以及 FFI 打交道本质上是 unsafe 的，
/// 因此通常会在 C API 周围构建封装。
///
/// FFI 的镜像用例同样通过 `extern` 关键字完成：
///
/// ```rust
/// #[unsafe(no_mangle)]
/// pub extern "C" fn callable_from_c(x: i32) -> bool {
///     x % 3 == 0
/// }
/// ```
///
/// 如果编译为动态库（dylib），生成的 .so 随后可以被某个 C 库链接，该函数就能像来自任何其他
/// 库一样被使用。
///
/// 关于 FFI 的更多信息，请查阅 [Rust book] 或 [Reference]。
///
/// [Rust book]:
/// ../book/ch19-01-unsafe-rust.html#using-extern-functions-to-call-external-code
/// [Reference]: ../reference/items/external-blocks.html
/// [`crate`]: keyword.crate.html
mod extern_keyword {}

#[doc(keyword = "false")]
//
/// 类型为 [`bool`] 的值，表示逻辑**假**。
///
/// `false` 是 [`true`] 的逻辑反面。
///
/// 更多信息请参阅 [`true`] 的文档。
///
/// [`true`]: keyword.true.html
mod false_keyword {}

#[doc(keyword = "fn")]
//
/// 一个函数或函数指针。
///
/// 函数是 Rust 中执行代码的主要方式。函数块，通常就简称为函数，可以定义在各种不同的地方，
/// 并被赋予许多不同的属性和修饰符。
///
/// 独立的函数，即只是位于某个模块中、不附着于其他任何东西的函数，很常见，但大多数函数最终都会
/// 处于 [`impl`] 块内，要么作用在另一个类型本身上，要么作为该类型的 trait 实现。
///
/// ```rust
/// fn standalone_function() {
///     // 代码
/// }
///
/// pub fn public_thing(argument: bool) -> String {
///     // 代码
///     # "".to_string()
/// }
///
/// struct Thing {
///     foo: i32,
/// }
///
/// impl Thing {
///     pub fn new() -> Self {
///         Self {
///             foo: 42,
///         }
///     }
/// }
/// ```
///
/// 除了以 `fn name(arg: type, ..) -> return_type` 的形式给出固定类型外，函数还可以声明一组
/// 类型参数，以及它们所满足的 trait 约束。
///
/// ```rust
/// fn generic_function<T: Clone>(x: T) -> (T, T, T) {
///     (x.clone(), x.clone(), x.clone())
/// }
///
/// fn generic_where<T>(x: T) -> T
///     where T: std::ops::Add<Output = T> + Copy
/// {
///     x + x + x
/// }
/// ```
///
/// 在尖括号中声明 trait 约束，在功能上与使用 `where` 子句完全相同。由程序员自行决定在每种
/// 情形下哪种更合适，但当内容超过一行时，`where` 往往更好。
///
/// 除了通过 `pub` 变为公开外，`fn` 还可以添加 [`extern`] 以用于 FFI。
///
/// 关于各类函数及其用法的更多信息，请查阅 [Rust book] 或 [Reference]。
///
/// [`impl`]: keyword.impl.html
/// [`extern`]: keyword.extern.html
/// [Rust book]: ../book/ch03-03-how-functions-work.html
/// [Reference]: ../reference/items/functions.html
mod fn_keyword {}

#[doc(keyword = "for")]
//
/// 配合 [`in`] 进行迭代、配合 [`impl`] 实现 trait，或[高阶 trait 约束][higher-ranked trait bounds]
/// （`for<'a>`）。
///
/// `for` 关键字用于许多语法位置：
///
/// * `for` 用于 for-in 循环（见下文）。
/// * `for` 用于实现 trait，如 `impl Trait for Type`（关于这一点更多信息见 [`impl`]）。
/// * `for` 也用于[高阶 trait 约束][higher-ranked trait bounds]，如 `for<'a> &'a T: PartialEq<i32>`。
///
/// for-in 循环，更确切地说是迭代器循环，是对 Rust 中一种常见做法的简单语法糖：即对任何实现了
/// [`IntoIterator`] 的东西进行循环，直到 `.into_iter()` 返回的迭代器返回 `None`
/// （或循环体使用了 `break`）。
///
/// ```rust
/// for i in 0..5 {
///     println!("{}", i * 2);
/// }
///
/// for i in std::iter::repeat(5) {
///     println!("turns out {i} never stops being 5");
///     break; // 否则会永远循环下去
/// }
///
/// 'outer: for x in 5..50 {
///     for y in 0..10 {
///         if x == y {
///             break 'outer;
///         }
///     }
/// }
/// ```
///
/// 如上例所示，`for` 循环（连同所有其他循环）可以打标签，使用与生命周期类似的语法（仅在视觉上
/// 相似，实践中完全不同）。给 `break` 加上相同的标签会跳出被标记的循环，这对内层循环很有用。
/// 它绝对不是 goto。
///
/// 一个 `for` 循环按如下方式展开：
///
/// ```rust
/// # fn code() { }
/// # let iterator = 0..2;
/// for loop_variable in iterator {
///     code()
/// }
/// ```
///
/// ```rust
/// # fn code() { }
/// # let iterator = 0..2;
/// {
///     let result = match IntoIterator::into_iter(iterator) {
///         mut iter => loop {
///             match iter.next() {
///                 None => break,
///                 Some(loop_variable) => { code(); },
///             };
///         },
///     };
///     result
/// }
/// ```
///
/// 关于上面所示功能的更多细节，可在 [`IntoIterator`] 文档中查看。
///
/// 关于 for 循环的更多信息，请参阅 [Rust book] 或 [Reference]。
///
/// 另见 [`loop`]、[`while`]。
///
/// [`in`]: keyword.in.html
/// [`impl`]: keyword.impl.html
/// [`loop`]: keyword.loop.html
/// [`while`]: keyword.while.html
/// [higher-ranked trait bounds]: ../reference/trait-bounds.html#higher-ranked-trait-bounds
/// [Rust book]:
/// ../book/ch03-05-control-flow.html#looping-through-a-collection-with-for
/// [Reference]: ../reference/expressions/loop-expr.html#iterator-loops
mod for_keyword {}

#[doc(keyword = "if")]
//
/// 当条件成立时求值一个块。
///
/// `if` 对大多数程序员来说是个熟悉的结构，也是你在代码中处理逻辑最常用的方式。然而，与多数
/// 语言不同，`if` 块还可以充当表达式。
///
/// ```rust
/// # let rude = true;
/// if 1 == 2 {
///     println!("whoops, mathematics broke");
/// } else {
///     println!("everything's fine!");
/// }
///
/// let greeting = if rude {
///     "sup nerd."
/// } else {
///     "hello, friend!"
/// };
///
/// if let Ok(x) = "123".parse::<i32>() {
///     println!("{} double that and you get {}!", greeting, x * 2);
/// }
/// ```
///
/// 上面展示了 `if` 块的三种典型形式。第一种是你在许多语言中会看到的常见写法，带一个可选的
/// `else` 块。第二种把 `if` 用作表达式，这仅当所有分支返回相同类型时才可行。`if` 表达式可以
/// 用在你期望的任何地方。第三种 `if` 块是 `if let` 块，它的行为类似于使用 `match` 表达式：
///
/// ```rust
/// if let Some(x) = Some(123) {
///     // 代码
///     # let _ = x;
/// } else {
///     // 别的东西
/// }
///
/// match Some(123) {
///     Some(x) => {
///         // 代码
///         # let _ = x;
///     },
///     _ => {
///         // 别的东西
///     },
/// }
/// ```
///
/// 各种 `if` 表达式可以按需混合搭配使用。
///
/// ```rust
/// if true == false {
///     println!("oh no");
/// } else if "something" == "other thing" {
///     println!("oh dear");
/// } else if let Some(200) = "blarg".parse::<i32>().ok() {
///     println!("uh oh");
/// } else {
///     println!("phew, nothing's broken");
/// }
/// ```
///
/// `if` 关键字在 Rust 中还用于另一个地方，即作为模式匹配本身的一部分，允许使用诸如
/// `Some(x) if x > 200` 这样的模式（即匹配守卫）。
///
/// 关于 `if` 表达式的更多信息，请参阅 [Rust book] 或 [Reference]。
///
/// [Rust book]: ../book/ch03-05-control-flow.html#if-expressions
/// [Reference]: ../reference/expressions/if-expr.html
mod if_keyword {}

#[doc(keyword = "impl")]
//
/// 为某个类型实现功能，或者一个实现了某种功能的类型。
///
/// 关键字 `impl` 有两种用法：
///  * `impl` 块是一个用于为某个类型实现某些功能的项。
///  * 处于类型位置的 `impl Trait` 可以用来指代某个实现了名为 `Trait` 的 trait 的类型。
///
/// # Implementing Functionality for a Type
///
/// `impl` 关键字主要用于在类型上定义实现。固有实现（inherent implementation）是独立的，
/// 而 trait 实现则用于为类型实现 trait，或实现其他 trait。
///
/// 一个实现由函数和常量的定义组成。定义在 `impl` 块中的函数可以是独立的，意味着它会像
/// `Vec::new()` 那样被调用。如果该函数以 `self`、`&self` 或 `&mut self` 作为第一个参数，
/// 它也可以用方法调用语法来调用，这是任何面向对象程序员都熟悉的特性，比如 `vec.len()`。
///
/// ## Inherent Implementations
///
/// ```rust
/// struct Example {
///     number: i32,
/// }
///
/// impl Example {
///     fn boo() {
///         println!("boo! Example::boo() was called!");
///     }
///
///     fn answer(&mut self) {
///         self.number += 42;
///     }
///
///     fn get_number(&self) -> i32 {
///         self.number
///     }
/// }
/// ```
///
/// 固有实现定义在哪里关系不大；只要它的实现类型在作用域内，它的功能就在作用域内。
///
/// ## Trait Implementations
///
/// ```rust
/// struct Example {
///     number: i32,
/// }
///
/// trait Thingy {
///     fn do_thingy(&self);
/// }
///
/// impl Thingy for Example {
///     fn do_thingy(&self) {
///         println!("doing a thing! also, number is {}!", self.number);
///     }
/// }
/// ```
///
/// trait 实现定义在哪里关系不大；通过导入它所实现的 trait，就能把它的功能引入作用域。
///
/// 关于实现的更多信息，请参阅 [Rust book][book1] 或 [Reference]。
///
/// # Designating a Type that Implements Some Functionality
///
/// `impl` 关键字的另一种用法是 `impl Trait` 语法，可以理解为"任何（或某个）实现了 Trait 的
/// 具体类型"。它可以用作变量声明的类型、用在[参数位置](https://rust-lang.github.io/rfcs/1951-expand-impl-trait.html)
/// 或[返回位置](https://rust-lang.github.io/rfcs/3425-return-position-impl-trait-in-traits.html)。
/// 一个相关的用例是处理闭包，闭包的类型是无法命名的。
///
/// ```rust
/// fn thing_returning_closure() -> impl Fn(i32) -> bool {
///     println!("here's a closure for you!");
///     |x: i32| x % 3 == 0
/// }
/// ```
///
/// 关于 `impl Trait` 语法的更多信息，请参阅 [Rust book][book2]。
///
/// [book1]: ../book/ch05-03-method-syntax.html
/// [Reference]: ../reference/items/implementations.html
/// [book2]: ../book/ch10-02-traits.html#returning-types-that-implement-traits
mod impl_keyword {}

#[doc(keyword = "in")]
//
/// 配合 [`for`] 遍历一系列值。
///
/// 紧跟在 `in` 之后的表达式必须实现 [`IntoIterator`] trait。
///
/// ## Literal Examples:
///
///    * `for _ in 1..3 {}` —— 遍历一个不含上界的区间，到 3 为止但不包含 3。
///    * `for _ in 1..=3 {}` —— 遍历一个含上界的区间，到 3 为止且包含 3。
///
/// （阅读更多关于[区间模式][range patterns]的内容）
///
/// [`IntoIterator`]: ../book/ch13-04-performance.html
/// [range patterns]: ../reference/patterns.html?highlight=range#range-patterns
/// [`for`]: keyword.for.html
///
/// `in` 的另一种用法是配合关键字 `pub`。它允许用户声明一个项仅在给定作用域内可见。
///
/// ## Literal Example:
///
///    * `pub(in crate::outer_mod) fn outer_mod_visible_fn() {}` —— fn 在 `outer_mod` 中可见
///
/// 从 2018 版次开始，`pub(in path)` 的路径必须以 `crate`、`self` 或 `super` 开头。
/// 2015 版次还可以使用以 `::` 开头的路径，或来自 crate 根的模块。
///
/// 更多信息请参阅 [Reference]。
///
/// [Reference]: ../reference/visibility-and-privacy.html#pubin-path-pubcrate-pubsuper-and-pubself
mod in_keyword {}

#[doc(keyword = "let")]
//
/// 把一个值绑定到一个变量。
///
/// `let` 关键字的主要用途是用在 `let` 语句中，它用于按照给定的模式向当前作用域引入一组新变量。
///
/// ```rust
/// # #![allow(unused_assignments)]
/// let thing1: i32 = 100;
/// let thing2 = 200 + thing1;
///
/// let mut changing_thing = true;
/// changing_thing = false;
///
/// let (part1, part2) = ("first", "second");
///
/// struct Example {
///     a: bool,
///     b: u64,
/// }
///
/// let Example { a, b: _ } = Example {
///     a: true,
///     b: 10004,
/// };
/// assert!(a);
/// ```
///
/// 这个模式最常见的是单个变量，这意味着不会进行模式匹配，给定的表达式会被绑定到该变量。
/// 除此之外，`let` 绑定中使用的模式可以按需要任意复杂，前提是该模式是穷尽（exhaustive）的。
/// 关于模式匹配的更多信息见 [Rust book][book1]。模式的类型可以在其后可选地给出，
/// 但如果留空，编译器会在可能的情况下自动推断。
///
/// Rust 中的变量默认是不可变的，需要 `mut` 关键字才能变为可变。
///
/// 可以用同一个名字定义多个变量，这被称为遮蔽（shadowing）。除了在遮蔽点之后无法直接访问
/// 原变量之外，这不会以任何方式影响原变量。它仍然留在作用域内，仅在离开作用域时才被丢弃。
/// 被遮蔽的变量不需要与遮蔽它们的变量具有相同的类型。
///
/// ```rust
/// let shadowing_example = true;
/// let shadowing_example = 123.4;
/// let shadowing_example = shadowing_example as u32;
/// let mut shadowing_example = format!("cool! {shadowing_example}");
/// shadowing_example += " something else!"; // 不是遮蔽
/// ```
///
/// `let` 关键字使用的其他地方包括与 [`if`] 一起，以 `if let` 表达式的形式出现。当被匹配的
/// 模式不是穷尽的时（比如对于枚举），它们很有用。还存在 `while let`，它运行一个循环，
/// 持续匹配模式所对应的值，直到该模式无法被匹配为止。
///
/// 关于 `let` 关键字的更多信息，请参阅 [Rust book][book2] 或 [Reference]。
///
/// [book1]: ../book/ch06-02-match.html
/// [`if`]: keyword.if.html
/// [book2]: ../book/ch18-01-all-the-places-for-patterns.html#let-statements
/// [Reference]: ../reference/statements.html#let-statements
mod let_keyword {}

#[doc(keyword = "loop")]
//
/// 无限循环。
///
/// `loop` 用于定义 Rust 支持的最简单的一种循环。它会运行其中的代码，直到代码使用了 `break`
/// 或者程序退出。
///
/// ```rust
/// loop {
///     println!("hello world forever!");
///     # break;
/// }
///
/// let mut i = 1;
/// loop {
///     println!("i is {i}");
///     if i > 100 {
///         break;
///     }
///     i *= 2;
/// }
/// assert_eq!(i, 128);
/// ```
///
/// 与 Rust 中其他种类的循环（`while`、`while let` 和 `for`）不同，`loop` 可以用作表达式，
/// 通过 `break` 返回值。
///
/// ```rust
/// let mut i = 1;
/// let something = loop {
///     i *= 2;
///     if i > 100 {
///         break i;
///     }
/// };
/// assert_eq!(something, 128);
/// ```
///
/// 一个循环中的每个 `break` 都必须有相同的类型。当没有显式给出值时，`break;` 返回 `()`。
///
/// 关于 `loop` 以及循环的一般信息，请参阅 [Reference]。
///
/// 另见 [`for`]、[`while`]。
///
/// [`for`]: keyword.for.html
/// [`while`]: keyword.while.html
/// [Reference]: ../reference/expressions/loop-expr.html
mod loop_keyword {}

#[doc(keyword = "match")]
//
/// 基于模式匹配的控制流。
///
/// `match` 可以用来有条件地运行代码。每个模式都必须被穷尽地处理，要么显式处理，
/// 要么在 `match` 中使用诸如 `_` 这样的通配符。由于 `match` 是一个表达式，也可以返回值。
///
/// ```rust
/// let opt = Option::None::<usize>;
/// let x = match opt {
///     Some(int) => int,
///     None => 10,
/// };
/// assert_eq!(x, 10);
///
/// let a_number = Option::Some(10);
/// match a_number {
///     Some(x) if x <= 5 => println!("0 to 5 num = {x}"),
///     Some(x @ 6..=10) => println!("6 to 10 num = {x}"),
///     None => panic!(),
///     // 所有其他数字
///     _ => panic!(),
/// }
/// ```
///
/// `match` 可以用来访问枚举的内部成员并直接使用它们。
///
/// ```rust
/// enum Outer {
///     Double(Option<u8>, Option<String>),
///     Single(Option<u8>),
///     Empty
/// }
///
/// let get_inner = Outer::Double(None, Some(String::new()));
/// match get_inner {
///     Outer::Double(None, Some(st)) => println!("{st}"),
///     Outer::Single(opt) => println!("{opt:?}"),
///     _ => panic!(),
/// }
/// ```
///
/// 关于 `match` 以及匹配的一般信息，请参阅 [Reference]。
///
/// [Reference]: ../reference/expressions/match-expr.html
mod match_keyword {}

#[doc(keyword = "mod")]
//
/// 把代码组织进[模块][modules]。
///
/// 使用 `mod` 来创建新的[模块][modules]，以封装代码，包括其他模块：
///
/// ```
/// mod foo {
///     mod bar {
///         type MyType = (u8, u8);
///         fn baz() {}
///     }
/// }
/// ```
///
/// 与 [`struct`] 和 [`enum`] 一样，模块及其内容默认是私有的，模块外的代码无法访问。
///
/// 要了解更多关于允许访问的内容，请参阅 [`pub`] 关键字的文档。
///
/// [`enum`]: keyword.enum.html
/// [`pub`]: keyword.pub.html
/// [`struct`]: keyword.struct.html
/// [modules]: ../reference/items/modules.html
mod mod_keyword {}

#[doc(keyword = "move")]
//
/// 以值的方式捕获[闭包][closure]的环境。
///
/// `move` 把任何以引用或可变引用方式捕获的变量转换为以值的方式捕获。
///
/// ```rust
/// let data = vec![1, 2, 3];
/// let closure = move || println!("captured {data:?} by value");
///
/// // data 不再可用，它已被闭包拥有
/// ```
///
/// 注意：`move` 闭包仍然可能实现 [`Fn`] 或 [`FnMut`]，即便它们以 `move` 方式捕获变量。
/// 这是因为闭包类型所实现的 trait 由闭包对所捕获值*做了什么*决定，而不是由它*如何*捕获它们决定：
///
/// ```rust
/// fn create_fn() -> impl Fn() {
///     let text = "Fn".to_owned();
///     move || println!("This is a: {text}")
/// }
///
/// let fn_plain = create_fn();
/// fn_plain();
/// ```
///
/// `move` 常在涉及[线程][threads]时使用。
///
/// ```rust
/// let data = vec![1, 2, 3];
///
/// std::thread::spawn(move || {
///     println!("captured {data:?} by value")
/// }).join().unwrap();
///
/// // data 已被移动到新生成的线程中，所以我们在这里不能使用它
/// ```
///
/// `move` 也可以合法地放在 async 块之前。
///
/// ```rust
/// let capture = "hello".to_owned();
/// let block = async move {
///     println!("rust says {capture} from async block");
/// };
/// ```
///
/// 关于 `move` 关键字的更多信息，请参阅 Rust book 的[闭包][closure]一节或[线程][threads]一节。
///
/// [closure]: ../book/ch13-01-closures.html
/// [threads]: ../book/ch16-01-threads.html#using-move-closures-with-threads
mod move_keyword {}

#[doc(keyword = "mut")]
//
/// 可变的变量、引用或指针。
///
/// `mut` 可以用在几种情形中。第一种是可变变量，它可以用在任何你能把一个值绑定到变量名的地方。
/// 一些例子：
///
/// ```rust
/// // 函数参数列表中的可变变量。
/// fn foo(mut x: u8, y: u8) -> u8 {
///     x += y;
///     x
/// }
///
/// // 修改一个可变变量。
/// # #[allow(unused_assignments)]
/// let mut a = 5;
/// a = 6;
///
/// assert_eq!(foo(3, 4), 7);
/// assert_eq!(a, 6);
/// ```
///
/// 第二种是可变引用。它们可以从 `mut` 变量创建，并且必须是唯一的：没有其他变量能持有可变引用，
/// 也不能持有共享引用。
///
/// ```rust
/// // 获取一个可变引用。
/// fn push_two(v: &mut Vec<u8>) {
///     v.push(2);
/// }
///
/// // 不能对一个非可变变量获取可变引用。
/// let mut v = vec![0, 1];
/// // 传入一个可变引用。
/// push_two(&mut v);
///
/// assert_eq!(v, vec![0, 1, 2]);
/// ```
///
/// ```rust,compile_fail,E0502
/// let mut v = vec![0, 1];
/// let mut_ref_v = &mut v;
/// # #[allow(unused)]
/// let ref_v = &v;
/// mut_ref_v.push(2);
/// ```
///
/// 可变裸指针的工作方式与可变引用很像，只是额外多了一种可能：它可能并不指向一个有效的对象。
/// 其语法是 `*mut Type`。
///
/// 关于可变引用和指针的更多信息，可在 [Reference] 中找到。
///
/// [Reference]: ../reference/types/pointer.html#mutable-references-mut
mod mut_keyword {}

#[doc(keyword = "pub")]
//
/// 让一个项对其他代码可见。
///
/// 关键字 `pub` 让任何模块、函数或数据结构可以从外部模块内部被访问。`pub` 关键字也可以用在
/// `use` 声明中，以从某个命名空间重新导出（re-export）一个标识符。
///
/// 关于 `pub` 关键字的更多信息，请参阅 [reference] 的可见性一节；一些示例见 [Rust by Example]。
///
/// [reference]:../reference/visibility-and-privacy.html?highlight=pub#visibility-and-privacy
/// [Rust by Example]:../rust-by-example/mod/visibility.html
mod pub_keyword {}

#[doc(keyword = "ref")]
//
/// 在模式匹配期间按引用绑定。
///
/// `ref` 标注模式绑定，使其进行借用而非移动。就匹配而言，它**不是**模式的一部分：
/// 它不影响一个值*是否*被匹配，只影响它*如何*被匹配。
///
/// 默认情况下，[`match`] 语句会尽可能地消耗（consume）它所能消耗的一切，这有时会成为问题，
/// 比如当你其实并不需要该值被移动并被取得所有权时：
///
/// ```compile_fail,E0382
/// let maybe_name = Some(String::from("Alice"));
/// // 变量 'maybe_name' 在这里被消耗……
/// match maybe_name {
///     Some(n) => println!("Hello, {n}"),
///     _ => println!("Hello, world"),
/// }
/// // ……现在它已不可用。
/// println!("Hello again, {}", maybe_name.unwrap_or("world".into()));
/// ```
///
/// 使用 `ref` 关键字后，值只是被借用，而非被移动，从而使其在 [`match`] 语句之后仍可使用：
///
/// ```
/// let maybe_name = Some(String::from("Alice"));
/// // 使用 `ref`，值被借用，而非被移动……
/// match maybe_name {
///     Some(ref n) => println!("Hello, {n}"),
///     _ => println!("Hello, world"),
/// }
/// // ……所以它在这里仍然可用！
/// println!("Hello again, {}", maybe_name.unwrap_or("world".into()));
/// ```
///
/// # `&` vs `ref`
///
/// - `&` 表示你的模式期望的是一个对对象的引用。因此 `&` 是该模式的一部分：`&Foo` 匹配的对象
/// 与 `Foo` 所匹配的不同。
///
/// - `ref` 表示你想要一个对解包后的值的引用。它本身不参与匹配：`Foo(ref foo)` 与 `Foo(foo)`
/// 匹配相同的对象。
///
/// 更多信息另见 [Reference]。
///
/// [`match`]: keyword.match.html
/// [Reference]: ../reference/patterns.html#identifier-patterns
mod ref_keyword {}

#[doc(keyword = "return")]
//
/// 从函数返回一个值。
///
/// `return` 标记函数中一条执行路径的结束：
///
/// ```
/// fn foo() -> i32 {
///     return 3;
/// }
/// assert_eq!(foo(), 3);
/// ```
///
/// 当返回的值是函数中的最后一个表达式时，不需要 `return`。这种情况下省略 `;`：
///
/// ```
/// fn foo() -> i32 {
///     3
/// }
/// assert_eq!(foo(), 3);
/// ```
///
/// `return` 会立即从函数返回（一种"提前返回"，early return）：
///
/// ```no_run
/// use std::fs::File;
/// use std::io::{Error, ErrorKind, Read, Result};
///
/// fn main() -> Result<()> {
///     let mut file = match File::open("foo.txt") {
///         Ok(f) => f,
///         Err(e) => return Err(e),
///     };
///
///     let mut contents = String::new();
///     let size = match file.read_to_string(&mut contents) {
///         Ok(s) => s,
///         Err(e) => return Err(e),
///     };
///
///     if contents.contains("impossible!") {
///         return Err(Error::new(ErrorKind::Other, "oh no!"));
///     }
///
///     if size > 9000 {
///         return Err(Error::new(ErrorKind::Other, "over 9000!"));
///     }
///
///     assert_eq!(contents, "Hello, world!");
///     Ok(())
/// }
/// ```
///
/// 在[闭包][closures]和 [`async`] 块内部，`return` 从该闭包或 `async` 块内返回一个值，
/// 而不是从其父函数返回：
///
/// ```rust
/// fn foo() -> i32 {
///     let closure = || {
///         return 5;
///     };
///
///     let future = async {
///         return 10;
///     };
///
///     return 15;
/// }
///
/// assert_eq!(foo(), 15);
/// ```
///
/// [closures]: ../book/ch13-01-closures.html
/// [`async`]: ../std/keyword.async.html
mod return_keyword {}

#[doc(keyword = "become")]
//
/// 对一个函数执行尾调用（tail-call）。
///
/// <div class="warning">
///
/// `feature(explicit_tail_calls)` 目前尚不完整，可能无法正常工作。
/// </div>
///
/// 进行尾调用时，被调用函数的栈帧不会被添加到栈上，而是直接用被调用者的栈帧替换调用者的栈帧。
/// 这意味着，只要调用图中的某个循环只使用尾调用，栈的增长就是有界的。
///
/// 这对于编写函数式风格的代码很有用（因为它能防止递归耗尽资源），也对代码优化很有用
/// （因为尾调用*可能*比普通调用更廉价，尾调用可以以类似于计算跳转（computed goto）的方式使用）。
///
/// 使用 `become` 实现函数式风格 `fold` 的例子：
/// ```
/// #![feature(explicit_tail_calls)]
/// #![expect(incomplete_features)]
///
/// fn fold<T: Copy, S>(slice: &[T], init: S, f: impl Fn(S, T) -> S) -> S {
///     match slice {
///         // 如果没有 `become`，在大输入上这很容易导致栈溢出。
///         // 使用尾调用可以保证栈不会无界增长
///         [first, rest @ ..] => become fold(rest, f(init, *first), f),
///         [] => init,
///     }
/// }
/// ```
///
/// 编译器已经可以执行"尾调用优化"（tail call optimization）——它们可以把普通调用替换为尾调用，
/// 尽管并不保证一定会这么做。然而，要执行 TCO，该调用需要是函数中发生的最后一件事，并且从函数中
/// 返回。这个要求常常被局部变量的 drop 代码打破，因为 drop 代码是在计算完返回表达式之后才运行的：
///
/// ```
/// fn example() {
///     let string = "meow".to_owned();
///     println!("{string}");
///     return help(); // 这*不是* `example` 中发生的最后一件事……
/// }
///
/// // ……因为它会被脱糖（desugar）成这样：
/// fn example_desugared() {
///     let string = "meow".to_owned();
///     println!("{string}");
///     let tmp = help();
///     drop(string);
///     return tmp;
/// }
///
/// fn help() {}
/// ```
///
/// 因此，`become` 也会改变 drop 顺序，使得局部变量在求值该调用*之前*被丢弃。
///
/// 为了保证编译器能够执行尾调用，`become` 目前有以下要求：
/// 1. 被调用者和调用者必须有相同的 ABI、参数和返回类型
/// 2. 被调用者和调用者必须不含可变参数（varargs）
/// 3. 调用者必须没有被标记 `#[track_caller]`
///    - 允许被调用者被标记 `#[track_caller]`，否则添加 `#[track_caller]` 会成为破坏性改动。
///      如果被调用者被标记了 `#[track_caller]`，则不保证进行尾调用。
/// 4. 被调用者和调用者不能是闭包
///    （除非它被强制转换为函数指针）
///
/// 可以对函数指针进行尾调用：
/// ```
/// #![feature(explicit_tail_calls)]
/// #![expect(incomplete_features)]
///
/// #[derive(Copy, Clone)]
/// enum Inst { Inc, Dec }
///
/// fn dispatch(stream: &[Inst], state: u32) -> u32 {
///     const TABLE: &[fn(&[Inst], u32) -> u32] = &[increment, decrement];
///     match stream {
///         [inst, rest @ ..] => become TABLE[*inst as usize](rest, state),
///         [] => state,
///     }
/// }
///
/// fn increment(stream: &[Inst], state: u32) -> u32 {
///     become dispatch(stream, state + 1)
/// }
///
/// fn decrement(stream: &[Inst], state: u32) -> u32 {
///     become dispatch(stream, state - 1)
/// }
///
/// let program = &[Inst::Inc, Inst::Inc, Inst::Dec, Inst::Inc];
/// assert_eq!(dispatch(program, 0), 2);
/// ```
/// ```
/// #![feature(explicit_tail_calls)]
/// #![expect(incomplete_features)]
///
/// #[derive(Copy, Clone)]
/// enum Inst { Inc, Dec }
///
/// fn dispatch(stream: &[Inst], state: u32) -> u32 {
///     const TABLE: &[fn(&[Inst], u32) -> u32] = &[increment, decrement];
///     match stream {
///         [inst, rest @ ..] => become TABLE[*inst as usize](rest, state),
///         [] => state,
///     }
/// }
///
/// fn increment(stream: &[Inst], state: u32) -> u32 {
///     become dispatch(stream, state + 1)
/// }
///
/// fn decrement(stream: &[Inst], state: u32) -> u32 {
///     become dispatch(stream, state - 1)
/// }
///
/// let program = &[Inst::Inc, Inst::Inc, Inst::Dec, Inst::Inc];
/// assert_eq!(dispatch(program, 0), 2);
/// ```
mod become_keyword {}

#[doc(keyword = "self")]
//
/// 方法的接收者（receiver），或当前模块。
///
/// `self` 用于两种情形：引用当前模块，以及标记方法的接收者。
///
/// 在路径中，`self` 可以用来引用当前模块，无论是在 [`use`] 语句中，还是在用于访问某个元素的路径中：
///
/// ```
/// # #![allow(unused_imports)]
/// use std::io::{self, Read};
/// ```
///
/// 在功能上与下面相同：
///
/// ```
/// # #![allow(unused_imports)]
/// use std::io;
/// use std::io::Read;
/// ```
///
/// 使用 `self` 访问当前模块中的一个元素：
///
/// ```
/// # #![allow(dead_code)]
/// # fn main() {}
/// fn foo() {}
/// fn bar() {
///     self::foo()
/// }
/// ```
///
/// 把 `self` 用作方法的当前接收者，多数情况下可以省略参数类型。除了这一特殊之处外，
/// `self` 的用法与任何其他参数大致相同：
///
/// ```
/// struct Foo(i32);
///
/// impl Foo {
///     // 没有 `self`。
///     fn new() -> Self {
///         Self(0)
///     }
///
///     // 消耗 `self`。
///     fn consume(self) -> Self {
///         Self(self.0 + 1)
///     }
///
///     // 借用 `self`。
///     fn borrow(&self) -> &i32 {
///         &self.0
///     }
///
///     // 可变地借用 `self`。
///     fn borrow_mut(&mut self) -> &mut i32 {
///         &mut self.0
///     }
/// }
///
/// // 这个方法必须以 `Type::` 前缀调用。
/// let foo = Foo::new();
/// assert_eq!(foo.0, 0);
///
/// // 下面两个调用产生相同的结果。
/// let foo = Foo::consume(foo);
/// assert_eq!(foo.0, 1);
/// let foo = foo.consume();
/// assert_eq!(foo.0, 2);
///
/// // 用第二种语法时，借用是自动处理的。
/// let borrow_1 = Foo::borrow(&foo);
/// let borrow_2 = foo.borrow();
/// assert_eq!(borrow_1, borrow_2);
///
/// // 用第二种语法时，可变借用也是自动处理的。
/// let mut foo = Foo::new();
/// *Foo::borrow_mut(&mut foo) += 1;
/// assert_eq!(foo.0, 1);
/// *foo.borrow_mut() += 1;
/// assert_eq!(foo.0, 2);
/// ```
///
/// 注意，调用 `foo.method()` 时的这种自动转换不限于上面的例子。更多信息请参阅 [Reference]。
///
/// [`use`]: keyword.use.html
/// [Reference]: ../reference/items/associated-items.html#methods
mod self_keyword {}

// FIXME: 等 rustdoc 能够处理大小写不敏感文件系统上的 URL 冲突后，就可以把这两行
// 替换为 `#[doc(keyword = "Self")]`，并相应更新 `CheckAttrVisitor` 中的
// `is_doc_keyword`。
#[doc(alias = "Self")]
#[doc(keyword = "SelfTy")]
//
/// 在 [`trait`] 或 [`impl`] 块中的实现类型，或在类型定义中的当前类型。
///
/// 在类型定义中：
///
/// ```
/// # #![allow(dead_code)]
/// struct Node {
///     elem: i32,
///     // 这里 `Self` 就是 `Node`。
///     next: Option<Box<Self>>,
/// }
/// ```
///
/// 在 [`impl`] 块中：
///
/// ```
/// struct Foo(i32);
///
/// impl Foo {
///     fn new() -> Self {
///         Self(0)
///     }
/// }
///
/// assert_eq!(Foo::new().0, Foo(0).0);
/// ```
///
/// 使用 `Self` 时，泛型参数是隐式的：
///
/// ```
/// # #![allow(dead_code)]
/// struct Wrap<T> {
///     elem: T,
/// }
///
/// impl<T> Wrap<T> {
///     fn new(elem: T) -> Self {
///         Self { elem }
///     }
/// }
/// ```
///
/// 在 [`trait`] 定义及其相关的 [`impl`] 块中：
///
/// ```
/// trait Example {
///     fn example() -> Self;
/// }
///
/// struct Foo(i32);
///
/// impl Example for Foo {
///     fn example() -> Self {
///         Self(42)
///     }
/// }
///
/// assert_eq!(Foo::example().0, Foo(42).0);
/// ```
///
/// [`impl`]: keyword.impl.html
/// [`trait`]: keyword.trait.html
mod self_upper_keyword {}

#[doc(keyword = "static")]
//
/// 静态项（static item）是一个在你整个程序运行期间都有效的值（具有 `'static` 生命周期）。
///
/// 表面上看，`static` 项与 [`const`] 非常相似：两者都包含一个值，都需要类型标注，
/// 并且都只能用常量函数和常量值来初始化。然而，`static` 有一个显著的不同：它表示内存中的
/// 一个位置。这意味着你可以持有对 `static` 项的引用，甚至可能修改它们，使它们本质上成为
/// 全局变量。
///
/// 静态项在程序结束时不会调用 [`drop`]。
///
/// 有两种 `static` 项：与 [`mut`] 关键字一起声明的，以及不带 `mut` 的。
///
/// 静态项不能被移动：
///
/// ```rust,compile_fail,E0507
/// static VEC: Vec<u32> = vec![];
///
/// fn move_vec(v: Vec<u32>) -> Vec<u32> {
///     v
/// }
///
/// // 这一行会导致错误
/// move_vec(VEC);
/// ```
///
/// # Simple `static`s
///
/// 访问非 [`mut`] 的 `static` 项被认为是安全的，但有一些限制。最值得注意的是，`static` 值的
/// 类型需要实现 [`Sync`] trait，这排除了像 [`RefCell`] 这样的内部可变性容器。更多信息请参阅 [Reference]。
///
/// ```rust
/// static FOO: [i32; 5] = [1, 2, 3, 4, 5];
///
/// let r1 = &FOO as *const _;
/// let r2 = &FOO as *const _;
/// // 对于严格只读的 static，引用会有相同的地址
/// assert_eq!(r1, r2);
/// // 在许多情况下，static 项可以像变量一样使用
/// println!("{FOO:?}");
/// ```
///
/// # Mutable `static`s
///
/// 如果一个 `static` 项用 [`mut`] 关键字声明，那么它就被允许被程序修改。然而，访问可变的
/// `static` 可能以多种方式导致未定义行为，例如在多线程上下文中由于数据竞争。因此，对可变
/// `static` 的所有访问都需要一个 [`unsafe`] 块。
///
/// 在可能的情况下，通常更好的做法是使用一个非可变的 `static`，配合像 [`Mutex`]、[`OnceLock`]
/// 或[原子类型][atomic]这样的内部可变类型。
///
/// 尽管可变 `static` 不安全，但在许多场景中它们是必要的：它们可以用来表示由整个程序共享的全局
/// 状态，或在 [`extern`] 块中用来绑定 C 库中的变量。
///
/// 在 [`extern`] 块中：
///
/// ```rust,no_run
/// # #![allow(dead_code)]
/// unsafe extern "C" {
///     static mut ERROR_MESSAGE: *mut std::os::raw::c_char;
/// }
/// ```
///
/// 可变 `static` 和简单 `static` 一样，也有一些适用于它们的限制。更多信息请参阅 [Reference]。
///
/// [`const`]: keyword.const.html
/// [`extern`]: keyword.extern.html
/// [`mut`]: keyword.mut.html
/// [`unsafe`]: keyword.unsafe.html
/// [`Mutex`]: sync::Mutex
/// [`OnceLock`]: sync::OnceLock
/// [`RefCell`]: cell::RefCell
/// [atomic]: sync::atomic
/// [Reference]: ../reference/items/static-items.html
mod static_keyword {}

#[doc(keyword = "struct")]
//
/// 由其他类型组合而成的类型。
///
/// Rust 中的结构体有三种风味：带具名字段的结构体、元组结构体，以及单元结构体。
///
/// ```rust
/// struct Regular {
///     field1: f32,
///     field2: String,
///     pub field3: bool
/// }
///
/// struct Tuple(u32, String);
///
/// struct Unit;
/// ```
///
/// 常规结构体是最常用的。其中定义的每个字段都有一个名字和一个类型，定义之后可以用
/// `example_struct.field` 语法访问。结构体的字段共享其可变性，所以 `foo.bar = 2;` 只有在
/// `foo` 是可变的时才有效。给字段加上 `pub` 使其对其他模块中的代码可见，并允许它被直接访问和修改。
///
/// 元组结构体与常规结构体类似，但它的字段没有名字。它们的用法像元组，可以通过
/// `let TupleStruct(x, y) = foo;` 语法解构。访问单个变量时，使用与常规元组相同的语法，
/// 即 `foo.0`、`foo.1` 等，从零开始。
///
/// 单元结构体最常用作标记（marker）。它们的大小为零字节，但与空枚举不同，它们可以被实例化，
/// 这使它们与单元类型 `()` 同构（isomorphic）。当你需要在某个东西上实现一个 trait，
/// 但不需要在其中存储任何数据时，单元结构体很有用。
///
/// # Instantiation
///
/// 结构体可以用不同的方式实例化，所有这些方式都可以按需混合搭配。创建一个新结构体最常见的方式
/// 是通过诸如 `new()` 这样的构造方法，但当它不可用时（或者你正在编写构造函数本身），
/// 就使用结构体字面量语法：
///
/// ```rust
/// # struct Foo { field1: f32, field2: String, etc: bool }
/// let example = Foo {
///     field1: 42.0,
///     field2: "blah".to_string(),
///     etc: true,
/// };
/// ```
///
/// 只有当结构体的所有字段对你都可见时，才能用结构体字面量语法直接实例化它。
///
/// 为了让编写构造函数更方便，Rust 提供了少数几种快捷写法，其中最常见的是字段初始化简写
/// （Field Init shorthand）。当存在一个变量和一个同名的字段时，赋值可以从 `field: field`
/// 简化为简单的 `field`。下面这个假想构造函数的例子演示了这一点：
///
/// ```rust
/// struct User {
///     name: String,
///     admin: bool,
/// }
///
/// impl User {
///     pub fn new(name: String) -> Self {
///         Self {
///             name,
///             admin: false,
///         }
///     }
/// }
/// ```
///
/// 还有另一种结构体实例化的快捷写法，用于当你需要创建一个新结构体，且它的大部分值与同类型的
/// 前一个结构体相同时，这称为结构体更新语法（struct update syntax）：
///
/// ```rust
/// # struct Foo { field1: String, field2: () }
/// # let thing = Foo { field1: "".to_string(), field2: () };
/// let updated_thing = Foo {
///     field1: "a new value".to_string(),
///     ..thing
/// };
/// ```
///
/// 元组结构体的实例化方式与元组本身相同，只是要以结构体的名字作为前缀：`Foo(123, false, 0.1)`。
///
/// 空结构体只用它们的名字就能实例化，不需要其他任何东西。`let thing = EmptyStruct;`
///
/// # Style conventions
///
/// 结构体总是用 UpperCamelCase 书写，少有例外。虽然结构体字段列表末尾的逗号可以省略，
/// 但通常会保留它，以便日后添加和删除字段时更方便。
///
/// 关于结构体的更多信息，请看 [Rust Book][book] 或 [Reference][reference]。
///
/// [`PhantomData`]: marker::PhantomData
/// [book]: ../book/ch05-01-defining-structs.html
/// [reference]: ../reference/items/structs.html
mod struct_keyword {}

#[doc(keyword = "super")]
//
/// 当前[模块][module]的父级。
///
/// ```rust
/// # #![allow(dead_code)]
/// # fn main() {}
/// mod a {
///     pub fn foo() {}
/// }
/// mod b {
///     pub fn foo() {
///         super::a::foo(); // 调用 a 的 foo 函数
///     }
/// }
/// ```
///
/// 也可以多次使用 `super`：`super::super::foo`，沿着祖先链向上走。
///
/// 更多信息请参阅 [Reference]。
///
/// [module]: ../reference/items/modules.html
/// [Reference]: ../reference/paths.html#super
mod super_keyword {}

#[doc(keyword = "trait")]
//
/// 一组类型的公共接口。
///
/// `trait` 就像一个数据类型可以实现的接口。当一个类型实现了某个 trait 后，
/// 就可以通过泛型或 trait 对象，把它抽象地当作该 trait 来对待。
///
/// trait 可以由三类关联项组成：
///
/// - 函数和方法
/// - 类型
/// - 常量
///
/// trait 还可以包含额外的类型参数。这些类型参数或 trait 本身可以被其他 trait 约束。
///
/// trait 可以充当标记（marker），或携带其他无法通过它的各项表达的逻辑语义。当一个类型实现了
/// 那个 trait，它就承诺遵守其契约。[`Send`] 和 [`Sync`] 就是标准库中两个这样的标记 trait。
///
/// 关于 trait 的更多得多的信息，请参阅 [Reference][Ref-Traits]。
///
/// # Examples
///
/// trait 使用 `trait` 关键字声明。类型可以用 [`impl`] `Trait` [`for`] `Type` 来实现它们：
///
/// ```rust
/// trait Zero {
///     const ZERO: Self;
///     fn is_zero(&self) -> bool;
/// }
///
/// impl Zero for i32 {
///     const ZERO: Self = 0;
///
///     fn is_zero(&self) -> bool {
///         *self == Self::ZERO
///     }
/// }
///
/// assert_eq!(i32::ZERO, 0);
/// assert!(i32::ZERO.is_zero());
/// assert!(!4.is_zero());
/// ```
///
/// 带一个关联类型：
///
/// ```rust
/// trait Builder {
///     type Built;
///
///     fn build(&self) -> Self::Built;
/// }
/// ```
///
/// trait 可以是泛型的，带约束或不带约束：
///
/// ```rust
/// trait MaybeFrom<T> {
///     fn maybe_from(value: T) -> Option<Self>
///     where
///         Self: Sized;
/// }
/// ```
///
/// trait 可以建立在其他 trait 的要求之上。在下面的例子中，`Iterator` 是一个**超 trait**
/// （supertrait），而 `ThreeIterator` 是一个**子 trait**（subtrait）：
///
/// ```rust
/// trait ThreeIterator: Iterator {
///     fn next_three(&mut self) -> Option<[Self::Item; 3]>;
/// }
/// ```
///
/// trait 可以用在函数中，作为参数：
///
/// ```rust
/// # #![allow(dead_code)]
/// fn debug_iter<I: Iterator>(it: I) where I::Item: std::fmt::Debug {
///     for elem in it {
///         println!("{elem:#?}");
///     }
/// }
///
/// // u8_len_1、u8_len_2 和 u8_len_3 是等价的
///
/// fn u8_len_1(val: impl Into<Vec<u8>>) -> usize {
///     val.into().len()
/// }
///
/// fn u8_len_2<T: Into<Vec<u8>>>(val: T) -> usize {
///     val.into().len()
/// }
///
/// fn u8_len_3<T>(val: T) -> usize
/// where
///     T: Into<Vec<u8>>,
/// {
///     val.into().len()
/// }
/// ```
///
/// 或作为返回类型：
///
/// ```rust
/// # #![allow(dead_code)]
/// fn from_zero_to(v: u8) -> impl Iterator<Item = u8> {
///     (0..v).into_iter()
/// }
/// ```
///
/// 在这个位置使用 [`impl`] 关键字，允许函数作者把具体类型作为实现细节隐藏起来，
/// 这样它就可以在不破坏用户代码的前提下改变。
///
/// # Trait objects
///
/// *trait 对象*（trait object）是另一个实现了一组 trait 的类型的不透明值。trait 对象实现了
/// 所有指定的 trait 以及它们的超 trait（如果有的话）。
///
/// 语法如下：`dyn BaseTrait + AutoTrait1 + ... AutoTraitN`。
/// 只能使用一个 `BaseTrait`，所以这段代码无法编译：
///
/// ```rust,compile_fail,E0225
/// trait A {}
/// trait B {}
///
/// let _: Box<dyn A + B>;
/// ```
///
/// 这段也不行，它是一个语法错误：
///
/// ```rust,compile_fail
/// trait A {}
/// trait B {}
///
/// let _: Box<dyn A + dyn B>;
/// ```
///
/// 另一方面，这是正确的：
///
/// ```rust
/// trait A {}
///
/// let _: Box<dyn A + Send + Sync>;
/// ```
///
/// [Reference][Ref-Trait-Objects] 中有关于 trait 对象、它们的限制以及各版次之间差异的更多信息。
///
/// # Unsafe traits
///
/// 有些 trait 实现起来可能是 unsafe 的。在 trait 声明前面使用 [`unsafe`] 关键字来标记这一点：
///
/// ```rust
/// unsafe trait UnsafeTrait {}
///
/// unsafe impl UnsafeTrait for i32 {}
/// ```
///
/// # Differences between the 2015 and 2018 editions
///
/// 在 2015 版次中，trait 不需要参数模式：
///
/// ```rust,edition2015
/// # #![allow(anonymous_parameters)]
/// trait Tr {
///     fn f(i32);
/// }
/// ```
///
/// 这种行为在 2018 版次中不再有效。
///
/// [`for`]: keyword.for.html
/// [`impl`]: keyword.impl.html
/// [`unsafe`]: keyword.unsafe.html
/// [Ref-Traits]: ../reference/items/traits.html
/// [Ref-Trait-Objects]: ../reference/types/trait-object.html
mod trait_keyword {}

#[doc(keyword = "true")]
//
/// 类型为 [`bool`] 的值，表示逻辑**真**。
///
/// 逻辑上 `true` 不等于 [`false`]。
///
/// ## Control structures that check for **true**
///
/// Rust 的若干控制结构会检查一个 `bool` 条件是否求值为**真**。
///
///   * [`if`] 表达式中的条件必须是 `bool` 类型。每当该条件求值为**真**时，`if` 表达式取第一个
///     块的值。但如果条件求值为 `false`，则表达式取 `else` 块的值（如果存在 `else` 块的话）。
///
///   * [`while`] 是另一种期望 `bool` 类型条件的控制流结构。只要条件求值为**真**，`while` 循环
///     就会持续地求值其关联块。
///
///   * [`match`] 的分支上可以带匹配守卫（guard）子句。
///
/// [`if`]: keyword.if.html
/// [`while`]: keyword.while.html
/// [`match`]: ../reference/expressions/match-expr.html#match-guards
/// [`false`]: keyword.false.html
mod true_keyword {}

#[doc(keyword = "type")]
//
/// 为一个已有类型定义一个[别名][alias]。
///
/// 语法是 `type Name = ExistingType;`。
///
/// # 示例
///
/// `type` **不会**创建一个新类型：
///
/// ```rust
/// type Meters = u32;
/// type Kilograms = u32;
///
/// let m: Meters = 3;
/// let k: Kilograms = 3;
///
/// assert_eq!(m, k);
/// ```
///
/// 类型别名可以是泛型的：
///
/// ```rust
/// # use std::sync::{Arc, Mutex};
/// type ArcMutex<T> = Arc<Mutex<T>>;
/// ```
///
/// 在 trait 中，`type` 用于声明一个[关联类型][associated type]：
///
/// ```rust
/// trait Iterator {
///     // 关联类型声明
///     type Item;
///     fn next(&mut self) -> Option<Self::Item>;
/// }
///
/// struct Once<T>(Option<T>);
///
/// impl<T> Iterator for Once<T> {
///     // 关联类型定义
///     type Item = T;
///     fn next(&mut self) -> Option<Self::Item> {
///         self.0.take()
///     }
/// }
/// ```
///
/// [`trait`]: keyword.trait.html
/// [associated type]: ../reference/items/associated-items.html#associated-types
/// [alias]: ../reference/items/type-aliases.html
mod type_keyword {}

#[doc(keyword = "unsafe")]
//
/// 其[内存安全性][memory safety]无法被类型系统验证的代码或接口。
///
/// `unsafe` 关键字有两种用途：
/// - 声明编译器无法检查的契约（contract）的存在（`unsafe fn` 和 `unsafe trait`），
/// - 以及声明程序员已经检查过这些契约已被遵守（`unsafe {}` 和 `unsafe impl`，
/// 但也包括 `unsafe fn`——见下文）。
///
/// # Unsafe abilities
///
/// **无论如何，安全 Rust（Safe Rust）都不能导致未定义行为**。这被称为[健全性][soundness]：
/// 一个良类型（well-typed）的程序确实具有所期望的性质。[Nomicon][nomicon-soundness] 对这个
/// 主题有更详细的解释。
///
/// 为了确保健全性，安全 Rust 受到了足够的限制，以至于它可以被自动检查。然而，有时确实需要编写
/// 一些因为太过巧妙、编译器无法理解其正确性的代码。在那些情况下，你需要使用非安全 Rust（Unsafe Rust）。
///
/// 以下是非安全 Rust 相比安全 Rust 额外具有的能力：
///
/// - 解引用[裸指针][raw pointers]
/// - 实现 `unsafe` 的 [`trait`]
/// - 调用 `unsafe` 函数
/// - 修改 [`static`]（包括[外部的][`extern`]）
/// - 访问 [`union`] 的字段
///
/// 然而，这些额外的能力伴随着额外的责任：现在确保健全性由你负责。`unsafe` 关键字通过清晰地
/// 标记出那些需要操心这件事的代码片段来提供帮助。
///
/// ## The different meanings of `unsafe`
///
/// `unsafe` 的所有用法并不等价：有些是用来标记程序员必须检查的某个契约的存在，
/// 另一些则是用来说"我已经检查过这个契约了，放手去做吧"。下面这场
/// [Rust Internals 上的讨论][discussion on Rust Internals]对此有更深入的解释，
/// 但这里是主要要点的总结：
///
/// - `unsafe fn`：调用这个函数意味着要遵守一个编译器无法强制保证的契约。
/// - `unsafe trait`：实现这个 [`trait`] 意味着要遵守一个编译器无法强制保证的契约。
/// - `unsafe {}`：调用块内操作所必需的契约已经被程序员检查过，并保证会被遵守。
/// - `unsafe impl`：实现该 trait 所必需的契约已经被程序员检查过，并保证会被遵守。
///
/// 更多信息请参阅 [Rustonomicon] 和 [Reference]。
///
/// # 示例
///
/// ## Marking elements as `unsafe`
///
/// `unsafe` 可以用在函数上。注意，在 [`extern`] 块中声明的函数和静态项会被隐式地标记为
/// `unsafe`（但声明为 `extern "something" fn ...` 的函数不会）。可变静态项无论在哪里声明
/// 都始终是 unsafe 的。方法也可以声明为 `unsafe`：
///
/// ```rust
/// # #![allow(dead_code)]
/// static mut FOO: &str = "hello";
///
/// unsafe fn unsafe_fn() {}
///
/// unsafe extern "C" {
///     fn unsafe_extern_fn();
///     static BAR: *mut u32;
/// }
///
/// trait SafeTraitWithUnsafeMethod {
///     unsafe fn unsafe_method(&self);
/// }
///
/// struct S;
///
/// impl S {
///     unsafe fn unsafe_method_on_struct() {}
/// }
/// ```
///
/// trait 也可以声明为 `unsafe`：
///
/// ```rust
/// unsafe trait UnsafeTrait {}
/// ```
///
/// 由于 `unsafe fn` 和 `unsafe trait` 表明存在一个编译器无法强制保证的安全契约，
/// 把它记录在文档中很重要。标准库中有许多这样的例子，比如下面这段摘自 [`Vec::set_len`] 的代码。
/// `# Safety` 一节解释了为了安全地调用该函数而必须满足的契约。
///
/// ```rust,ignore (stub-to-show-doc-example)
/// /// 强制把向量的长度设为 `new_len`。
/// ///
/// /// 这是一个底层操作，不维护该类型的任何常规不变量。通常改变向量的长度应改用
/// /// 某个安全操作来完成，比如 `truncate`、`resize`、`extend` 或 `clear`。
/// ///
/// /// # Safety
/// ///
/// /// - `new_len` 必须小于或等于 `capacity()`。
/// /// - 处于 `old_len..new_len` 的元素必须已被初始化。
/// pub unsafe fn set_len(&mut self, new_len: usize)
/// ```
///
/// ## Using `unsafe {}` blocks and `impl`s
///
/// 执行 `unsafe` 操作需要一个 `unsafe {}` 块：
///
/// ```rust
/// # #![allow(dead_code)]
/// #![deny(unsafe_op_in_unsafe_fn)]
///
/// /// 解引用给定的指针。
/// ///
/// /// # Safety
/// ///
/// /// `ptr` 必须是对齐的，且不能是悬垂（dangling）的。
/// unsafe fn deref_unchecked(ptr: *const i32) -> i32 {
///     // SAFETY: 调用者必须确保 `ptr` 是对齐的且可解引用的。
///     unsafe { *ptr }
/// }
///
/// let a = 3;
/// let b = &a as *const _;
/// // SAFETY: `a` 尚未被丢弃，且引用总是对齐的，所以 `b` 是一个有效的地址。
/// unsafe { assert_eq!(*b, deref_unchecked(b)); };
/// ```
///
/// ## `unsafe` and traits
///
/// `unsafe` 与 trait 的相互作用可能令人意外，所以让我们用两个例子来对比 `unsafe trait` 中的
/// 安全 `fn` 与安全 trait 中的 `unsafe fn` 这两种组合：
///
/// ```rust
/// /// # Safety
/// ///
/// /// `make_even` 必须返回一个偶数。
/// unsafe trait MakeEven {
///     fn make_even(&self) -> i32;
/// }
///
/// // SAFETY: 我们的 `make_even` 总是返回偶数。
/// unsafe impl MakeEven for i32 {
///     fn make_even(&self) -> i32 {
///         self << 1
///     }
/// }
///
/// fn use_make_even(x: impl MakeEven) {
///     if x.make_even() % 2 == 1 {
///         // SAFETY: 这永远不会发生，因为所有 `MakeEven` 的实现
///         // 都保证 `make_even` 返回偶数。
///         unsafe { std::hint::unreachable_unchecked() };
///     }
/// }
/// ```
///
/// 注意 trait 的安全契约是如何被实现所遵守的，而它本身又被用来遵守 `use_make_even` 所调用的
/// 非安全函数 `unreachable_unchecked` 的安全契约。`make_even` 本身是一个安全函数，因为它的
/// *调用者*不必操心任何契约，只有 `MakeEven` 的*实现*被要求遵守某个契约。`use_make_even`
/// 是安全的，因为它可以利用 `MakeEven` 实现所做出的承诺，来遵守它所调用的
/// `unsafe fn unreachable_unchecked` 的安全契约。
///
/// 在一个常规的安全 `trait` 中也可以有 `unsafe fn`：
///
/// ```rust
/// # #![feature(never_type)]
/// #![deny(unsafe_op_in_unsafe_fn)]
///
/// trait Indexable {
///     const LEN: usize;
///
///     /// # Safety
///     ///
///     /// 调用者必须确保 `idx < LEN`。
///     unsafe fn idx_unchecked(&self, idx: usize) -> i32;
/// }
///
/// // 针对 `i32` 的实现不需要做任何契约推理。
/// impl Indexable for i32 {
///     const LEN: usize = 1;
///
///     /// 安全契约见 `Indexable`。
///     unsafe fn idx_unchecked(&self, idx: usize) -> i32 {
///         debug_assert_eq!(idx, 0);
///         *self
///     }
/// }
///
/// // 针对数组的实现利用了函数契约，
/// // 从而在切片上使用 `get_unchecked` 并避免运行时检查。
/// impl Indexable for [i32; 42] {
///     const LEN: usize = 42;
///
///     /// 安全契约见 `Indexable`。
///     unsafe fn idx_unchecked(&self, idx: usize) -> i32 {
///         // SAFETY: 根据本 trait 的文档，调用者确保 `idx < 42`。
///         unsafe { *self.get_unchecked(idx) }
///     }
/// }
///
/// // 针对 never 类型的实现声明长度为 0，
/// // 这意味着 `idx_unchecked` 永远不可能被调用。
/// impl Indexable for ! {
///     const LEN: usize = 0;
///
///     /// 安全契约见 `Indexable`。
///     unsafe fn idx_unchecked(&self, idx: usize) -> i32 {
///         // SAFETY: 根据本 trait 的文档，调用者确保 `idx < 0`，
///         // 这是不可能的，所以这是死代码。
///         unsafe { std::hint::unreachable_unchecked() }
///     }
/// }
///
/// fn use_indexable<I: Indexable>(x: I, idx: usize) -> i32 {
///     if idx < I::LEN {
///         // SAFETY: 我们已经检查了 `idx < I::LEN`。
///         unsafe { x.idx_unchecked(idx) }
///     } else {
///         panic!("index out-of-bounds")
///     }
/// }
/// ```
///
/// 这一次，`use_indexable` 是安全的，因为它使用了一个运行时检查来履行 `idx_unchecked` 的安全
/// 契约。实现 `Indexable` 是安全的，因为在编写 `idx_unchecked` 时，我们不必操心：我们的*调用者*
/// 需要履行一项证明义务（就像 `use_indexable` 所做的那样），但 `get_unchecked` 的*实现*没有要
/// 应付的证明义务。当然，实现可以选择调用其他非安全操作，这时它就需要一个 `unsafe` *块*来表明
/// 它已履行了其被调用者的证明义务。为此，它可以利用其所有调用者都必须遵守的契约——即 `idx < LEN`
/// 这一事实。
///
/// 注意，与普通 `unsafe fn` 不同，trait 实现中的 `unsafe fn` 不能随意挑选一个任意的安全契约！
/// 它*必须*使用由该 trait 定义的安全契约（或一个前置条件更弱的契约）。
///
/// 形式上讲，trait 中的 `unsafe fn` 是一个具有超出参数类型所编码（如 `idx < LEN`）的*前置条件*
/// 的函数，而 `unsafe trait` 则可以声明它的某些函数具有超出返回类型所编码（如返回一个偶数）的
/// *后置条件*。如果一个 trait 需要一个既有额外前置条件又有额外后置条件的函数，那么它就需要在
/// `unsafe trait` 中放一个 `unsafe fn`。
///
/// [`extern`]: keyword.extern.html
/// [`trait`]: keyword.trait.html
/// [`static`]: keyword.static.html
/// [`union`]: keyword.union.html
/// [`impl`]: keyword.impl.html
/// [raw pointers]: ../reference/types/pointer.html
/// [memory safety]: ../book/ch19-01-unsafe-rust.html
/// [Rustonomicon]: ../nomicon/index.html
/// [nomicon-soundness]: ../nomicon/safe-unsafe-meaning.html
/// [soundness]: https://rust-lang.github.io/unsafe-code-guidelines/glossary.html#soundness-of-code--of-a-library
/// [Reference]: ../reference/unsafety.html
/// [discussion on Rust Internals]: https://internals.rust-lang.org/t/what-does-unsafe-mean/6696
mod unsafe_keyword {}

#[doc(keyword = "use")]
//
/// 从其他 crate 或模块导入或重命名项、在符合人体工程学的克隆（ergonomic clones）语义下使用值，
/// 或用 `use<..>` 指定精确捕获（precise capturing）。
///
/// ## Importing items
///
/// `use` 关键字用于缩短引用某个模块项所需的路径。该关键字可以出现在模块、块乃至函数中，
/// 通常位于顶部。
///
/// 该关键字最基本的用法是 `use path::to::item;`，不过还支持若干便捷的快捷写法：
///
///   * 同时绑定一组具有公共前缀的路径，使用类似 glob 的花括号语法
///     `use a::b::{c, d, e::f, g::h::i};`
///   * 同时绑定一组具有公共前缀的路径及其公共父模块，使用 [`self`] 关键字，
///     如 `use a::b::{self, c, d::e};`
///   * 把目标名字重新绑定为一个新的本地名字，使用语法 `use p::q::r as x;`。
///     这也可以与前两个特性一起使用：`use a::b::{self as ab, c as abc}`。
///   * 绑定所有匹配给定前缀的路径，使用星号通配符语法 `use a::b::*;`。
///   * 多层嵌套前述特性的分组，如 `use a::b::{self as ab, c, d::{*, e::f}};`
///   * 配合可见性修饰符进行重新导出，如 `pub use a::b;`
///   * 用 `_` 导入，以便只导入某个 trait 的方法而不把它绑定到一个名字上
///     （例如为了避免冲突）：`use ::std::io::Read as _;`。
///
/// 支持使用 [`crate`]、[`super`] 或 [`self`] 这样的路径限定符：`use crate::a::b;`。
///
/// 注意，当通配符 `*` 用在一个类型上时，它不会导入该类型的方法（不过对于 `enum`，
/// 它会导入其变体，如下例所示）。
///
/// ```compile_fail,edition2018
/// enum ExampleEnum {
///     VariantA,
///     VariantB,
/// }
///
/// impl ExampleEnum {
///     fn new() -> Self {
///         Self::VariantA
///     }
/// }
///
/// use ExampleEnum::*;
///
/// // 能编译。
/// let _ = VariantA;
///
/// // 不能编译！
/// let n = new();
/// ```
///
/// 关于 `use` 和路径的一般信息，请参阅 [Reference][ref-use-decls]。
///
/// 2015 与 2018 版次之间关于路径和 `use` 关键字的差异，也可以在 [Reference][ref-use-decls] 中找到。
///
/// ## Precise capturing
///
/// `use<..>` 语法用在某些 `impl Trait` 约束中，以控制捕获哪些泛型参数。这对于返回位置的
/// `impl Trait`（RPIT）类型很重要，因为它通过控制哪些泛型参数可以用在隐藏类型（hidden type）
/// 中，从而影响借用检查。
///
/// 例如，下面的函数演示了在 Rust 2021 及更早版次中不使用精确捕获时出现的错误：
///
/// ```rust,compile_fail,edition2021
/// fn f(x: &()) -> impl Sized { x }
/// ```
///
/// 通过使用 `use<'_>` 进行精确捕获，可以解决这个问题：
///
/// ```rust
/// fn f(x: &()) -> impl Sized + use<'_> { x }
/// ```
///
/// 这个语法指定省略的生命周期被捕获，因此可以用在隐藏类型中。
///
/// 在 Rust 2024 中，不透明类型（opaque type）会自动捕获作用域内的所有生命周期参数。
/// `use<..>` 语法是退出该默认行为的一种重要方式。
///
/// 关于精确捕获的更多细节，请参阅 [Reference][ref-impl-trait]。
///
/// ## Ergonomic clones
///
/// 使用一个值，如果该值实现了 `Copy` 则复制其内容，如果该值实现了 `UseCloned` 则克隆其内容，
/// 否则将其移动。
///
/// [`crate`]: keyword.crate.html
/// [`self`]: keyword.self.html
/// [`super`]: keyword.super.html
/// [ref-use-decls]: ../reference/items/use-declarations.html
/// [ref-impl-trait]: ../reference/types/impl-trait.html
mod use_keyword {}

#[doc(keyword = "where")]
//
/// 添加使用某个项时必须满足的约束。
///
/// `where` 允许为生命周期和泛型参数指定约束。引入 `where` 的 [RFC] 包含关于该关键字的详细信息。
///
/// # 示例
///
/// `where` 可以用于带 trait 的约束：
///
/// ```rust
/// fn new<T: Default>() -> T {
///     T::default()
/// }
///
/// fn new_where<T>() -> T
/// where
///     T: Default,
/// {
///     T::default()
/// }
///
/// assert_eq!(0.0, new());
/// assert_eq!(0.0, new_where());
///
/// assert_eq!(0, new());
/// assert_eq!(0, new_where());
/// ```
///
/// `where` 也可以用于生命周期。
///
/// 这段代码能编译，因为 `longer` 的存活时间长于 `shorter`，因此约束得到了满足：
///
/// ```rust
/// fn select<'short, 'long>(s1: &'short str, s2: &'long str, second: bool) -> &'short str
/// where
///     'long: 'short,
/// {
///     if second { s2 } else { s1 }
/// }
///
/// let outer = String::from("Long living ref");
/// let longer = &outer;
/// {
///     let inner = String::from("Short living ref");
///     let shorter = &inner;
///
///     assert_eq!(select(shorter, longer, false), shorter);
///     assert_eq!(select(shorter, longer, true), longer);
/// }
/// ```
///
/// 另一方面，这段代码无法编译，因为缺少 `where 'b: 'a` 子句：编译器不知道 `'b` 生命周期至少
/// 与 `'a` 一样长，这意味着此函数无法保证它总是返回一个有效的引用：
///
/// ```rust,compile_fail
/// fn select<'a, 'b>(s1: &'a str, s2: &'b str, second: bool) -> &'a str
/// {
///     if second { s2 } else { s1 }
/// }
/// ```
///
/// `where` 还可以用来表达无法用 `<T: Trait>` 语法书写的更复杂约束：
///
/// ```rust
/// fn first_or_default<I>(mut i: I) -> I::Item
/// where
///     I: Iterator,
///     I::Item: Default,
/// {
///     i.next().unwrap_or_else(I::Item::default)
/// }
///
/// assert_eq!(first_or_default([1, 2, 3].into_iter()), 1);
/// assert_eq!(first_or_default(Vec::<i32>::new().into_iter()), 0);
/// ```
///
/// `where` 在任何可以使用泛型参数和生命周期参数的地方都可用，正如标准库中的
/// [`Cow`](crate::borrow::Cow) 类型所示：
///
/// ```rust
/// # #![allow(dead_code)]
/// pub enum Cow<'a, B>
/// where
///     B: ToOwned + ?Sized,
/// {
///     Borrowed(&'a B),
///     Owned(<B as ToOwned>::Owned),
/// }
/// ```
///
/// [RFC]: https://github.com/rust-lang/rfcs/blob/master/text/0135-where.md
mod where_keyword {}

#[doc(keyword = "while")]
//
/// 当条件成立时进行循环。
///
/// `while` 表达式用于谓词循环（predicate loop）。`while` 表达式在运行循环体之前先运行条件
/// 表达式，然后在条件表达式求值为 `true` 时运行循环体，否则退出循环。
///
/// ```rust
/// let mut counter = 0;
///
/// while counter < 10 {
///     println!("{counter}");
///     counter += 1;
/// }
/// ```
///
/// 与 [`for`] 表达式一样，我们可以使用 `break` 和 `continue`。与 [`loop`] 不同，`while`
/// 表达式不能 break 出一个值，并且始终求值为 `()`。
///
/// ```rust
/// let mut i = 1;
///
/// while i < 100 {
///     i *= 2;
///     if i == 64 {
///         break; // 当 `i` 为 64 时退出。
///     }
/// }
/// ```
///
/// 正如 `if` 表达式有它在 `if let` 中的模式匹配变体，`while` 表达式也有 `while let`。
/// `while let` 表达式把模式与表达式进行匹配，匹配成功时运行循环体，否则退出循环。
/// 我们可以像在 `while` 中一样在 `while let` 表达式中使用 `break` 和 `continue`。
///
/// ```rust
/// let mut counter = Some(0);
///
/// while let Some(i) = counter {
///     if i == 10 {
///         counter = None;
///     } else {
///         println!("{i}");
///         counter = Some (i + 1);
///     }
/// }
/// ```
///
/// 关于 `while` 以及循环的一般信息，请参阅 [reference]。
///
/// 另见 [`for`]、[`loop`]。
///
/// [`for`]: keyword.for.html
/// [`loop`]: keyword.loop.html
/// [reference]: ../reference/expressions/loop-expr.html#predicate-loops
mod while_keyword {}

// 2018 版次的关键字

#[doc(alias = "promise")]
#[doc(keyword = "async")]
//
/// 返回一个 [`Future`]，而不是阻塞当前线程。
///
/// 在 `fn`、`closure`（闭包）或 `block`（块）前面使用 `async`，即可把被标记的代码变成一个
/// `Future`。因此该代码不会被立即运行，而只会在所返回的 future 被 [`.await`]（等待）时才被求值。
///
/// 我们编写了一本 [async book]，详细阐述 `async`/`await` 以及与使用线程相比的取舍。
///
/// ## Control Flow
/// `async` 块内的 [`return`] 语句和 [`?`][try operator] 运算符不会导致从父函数返回；
/// 相反，它们会使该块返回的 `Future` 带着那个值返回。
///
/// 例如，下面这个 Rust 函数会返回 `5`，使得 `x` 取得 [`!` 类型][never type]：
/// ```rust
/// #[expect(unused_variables)]
/// fn example() -> i32 {
///     let x = {
///         return 5;
///     };
/// }
/// ```
/// 相比之下，下面这个异步函数把一个 `Future<Output = i32>` 赋给 `x`，并且只有当 `x` 被
/// `.await` 时才返回 `5`：
/// ```rust
/// async fn example() -> i32 {
///     let x = async {
///         return 5;
///     };
///
///     x.await
/// }
/// ```
/// 使用 `?` 的代码行为类似——它会使 `async` 块返回一个 [`Result`]，而不影响父函数。
///
/// 注意，你不能在 `async` 块内使用 `break` 或 `continue` 来影响父函数中某个循环的控制流。
///
/// `async` 块中的控制流在 [async book][async book blocks] 中有进一步的文档说明。
///
/// ## Editions
///
/// `async` 从 2018 版次起是一个关键字。
///
/// 它从 1.39 版本起可在稳定版 Rust 中使用。
///
/// [`Future`]: future::Future
/// [`.await`]: ../std/keyword.await.html
/// [async book]: https://rust-lang.github.io/async-book/
/// [`return`]: ../std/keyword.return.html
/// [try operator]: ../reference/expressions/operator-expr.html#r-expr.try
/// [never type]: ../reference/types/never.html
/// [`Result`]: result::Result
/// [async book blocks]: https://rust-lang.github.io/async-book/part-guide/more-async-await.html#async-blocks
mod async_keyword {}

#[doc(keyword = "await")]
//
/// 暂停执行，直到某个 [`Future`] 的结果就绪。
///
/// 对一个 future 执行 `.await` 会暂停当前函数的执行，直到执行器（executor）把该 future
/// 运行至完成。
///
/// 关于 [`async`]/`await` 以及执行器如何工作的细节，请阅读 [async book]。
///
/// ## Editions
///
/// `await` 从 2018 版次起是一个关键字。
///
/// 它从 1.39 版本起可在稳定版 Rust 中使用。
///
/// [`Future`]: future::Future
/// [async book]: https://rust-lang.github.io/async-book/
/// [`async`]: ../std/keyword.async.html
mod await_keyword {}

#[doc(keyword = "dyn")]
//
/// `dyn` 是 [trait 对象][trait object]类型的前缀。
///
/// `dyn` 关键字用于强调对关联的 `Trait` 上方法的调用是[动态分发][dynamically dispatched]的。
/// 要以这种方式使用该 trait，它必须是 *dyn 兼容的*（dyn compatible）[^1]。
///
/// 与泛型参数或 `impl Trait` 不同，编译器并不知道正在被传递的具体类型。也就是说，
/// 该类型已被[擦除][erased]。因此，一个 `dyn Trait` 引用包含_两个_指针。
/// 一个指针指向数据（例如某个结构体的一个实例）。
/// 另一个指针指向一张从方法调用名字到函数指针的映射表
/// （称为虚方法表，virtual method table，或 vtable）。
///
/// 在运行时，当需要在 `dyn Trait` 上调用一个方法时，会查询 vtable 来获取函数指针，
/// 然后调用那个函数指针。
///
/// 关于 [trait 对象][ref-trait-obj]和 [dyn 兼容性][ref-dyn-compat]的更多信息，请参阅 Reference。
///
/// ## Trade-offs
///
/// 上述间接寻址（indirection）是在 `dyn Trait` 上调用函数的额外运行时开销。
/// 通过动态分发调用的方法通常无法被编译器内联。
///
/// 然而，`dyn Trait` 很可能产生比 `impl Trait`／泛型参数更小的代码，因为方法不会为每个具体类型
/// 各复制一份。
///
/// [trait object]: ../book/ch17-02-trait-objects.html
/// [dynamically dispatched]: https://en.wikipedia.org/wiki/Dynamic_dispatch
/// [ref-trait-obj]: ../reference/types/trait-object.html
/// [ref-dyn-compat]: ../reference/items/traits.html#dyn-compatibility
/// [erased]: https://en.wikipedia.org/wiki/Type_erasure
/// [^1]: 旧称*对象安全*（object safe）。
mod dyn_keyword {}

#[doc(keyword = "union")]
//
/// [C 风格联合体在 Rust 中的等价物][union]。
///
/// `union` 在声明上看起来像一个 [`struct`]，但它的所有字段都存在于同一块内存中，彼此叠加在一起。
/// 例如，如果我们想要内存中的一些位，有时把它们解释为 `u32`、有时解释为 `f32`，我们可以这样写：
///
/// ```rust
/// union IntOrFloat {
///     i: u32,
///     f: f32,
/// }
///
/// let mut u = IntOrFloat { f: 1.0 };
/// // 读取 union 的字段总是 unsafe 的
/// assert_eq!(unsafe { u.i }, 1065353216);
/// // 通过任何一个字段进行更新都会修改所有字段
/// u.i = 1073741824;
/// assert_eq!(unsafe { u.f }, 2.0);
/// ```
///
/// # Matching on unions
///
/// 可以对 `union` 使用模式匹配。必须使用单个字段名，且它必须与该 `union` 的某个字段名匹配。
/// 与从 `union` 读取一样，对 `union` 进行模式匹配需要 `unsafe`。
///
/// ```rust
/// union IntOrFloat {
///     i: u32,
///     f: f32,
/// }
///
/// let u = IntOrFloat { f: 1.0 };
///
/// unsafe {
///     match u {
///         IntOrFloat { i: 10 } => println!("Found exactly ten!"),
///         // 匹配字段 `f` 会提供一个 `f32`。
///         IntOrFloat { f } => println!("Found f = {f} !"),
///     }
/// }
/// ```
///
/// # References to union fields
///
/// `union` 中的所有字段都位于内存中的同一位置，这意味着借用其中一个就借用了整个 `union`，
/// 持续相同的生命周期：
///
/// ```rust,compile_fail,E0502
/// union IntOrFloat {
///     i: u32,
///     f: f32,
/// }
///
/// let mut u = IntOrFloat { f: 1.0 };
///
/// let f = unsafe { &u.f };
/// // 这无法编译，因为该字段已被借用，即便只是不可变地借用
/// let i = unsafe { &mut u.i };
///
/// *i = 10;
/// println!("f = {f} and i = {i}");
/// ```
///
/// 关于 `union` 的更多信息，请参阅 [Reference][union]。
///
/// [`struct`]: keyword.struct.html
/// [union]: ../reference/items/unions.html
mod union_keyword {}
