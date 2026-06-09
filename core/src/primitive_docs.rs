#[rustc_doc_primitive = "bool"]
#[doc(alias = "true")]
#[doc(alias = "false")]
/// 布尔类型。
///
/// `bool` 表示只能是 [`true`] 或 [`false`] 的值。将 `bool` 转换为整数时，
/// [`true`] 为 1，[`false`] 为 0。
///
/// # 基本用法
///
/// `bool` 实现了多种 trait，例如 [`BitAnd`]、[`BitOr`]、[`Not`] 等，
/// 因而可以用 `&`、`|` 和 `!` 执行布尔运算。
///
/// [`if`] 要求条件为 `bool` 值。[`assert!`] 是测试中重要的宏，
/// 它检查表达式是否为 [`true`]，否则会 panic。
///
/// ```
/// let bool_val = true & false | false;
/// assert!(!bool_val);
/// ```
///
/// [`true`]: ../std/keyword.true.html
/// [`false`]: ../std/keyword.false.html
/// [`BitAnd`]: ops::BitAnd
/// [`BitOr`]: ops::BitOr
/// [`Not`]: ops::Not
/// [`if`]: ../std/keyword.if.html
///
/// # 示例
///
/// 一个简单的 `bool` 用法示例：
///
/// ```
/// let praise_the_borrow_checker = true;
///
/// // 使用 `if` 条件语句
/// if praise_the_borrow_checker {
///     println!("oh, yeah!");
/// } else {
///     println!("what?!!");
/// }
///
/// // ... 或者，一个 match 模式
/// match praise_the_borrow_checker {
///     true => println!("keep praising!"),
///     false => println!("you should praise!"),
/// }
/// ```
///
/// 此外，由于 `bool` 实现了 [`Copy`] trait，因此无需担心移动语义
/// （与整数和浮点数原语一样）。
///
/// 下面是将 `bool` 转换为整数类型的示例：
///
/// ```
/// assert_eq!(true as i32, 1);
/// assert_eq!(false as i32, 0);
/// ```
#[stable(feature = "rust1", since = "1.0.0")]
mod prim_bool {}

#[rustc_doc_primitive = "never"]
#[doc(alias = "!")]
//
/// `!` 类型，也称为 "never"。
///
/// `!` 表示永远不会求得任何值的计算类型。例如，[`exit`] 函数
/// `fn exit(code: i32) -> !` 会退出进程且永不返回，因此返回 `!`。
///
/// `break`、`continue` 和 `return` 表达式的类型也都是 `!`。例如可以这样写：
///
/// ```
/// #![feature(never_type)]
/// # fn foo() -> u32 {
/// let x: ! = {
///     return 123
/// };
/// # }
/// ```
///
/// 这里的 `let` 本身没有实际意义，但它展示了 `!` 的含义。由于 `x` 永远不会被赋值
/// （因为 `return` 会从整个函数返回），所以 `x` 可以具有类型 `!`。也可以将
/// `return 123` 替换为 `panic!` 或永不结束的 `loop`，这段代码仍然有效。
///
/// `!` 更真实的用法见下面的代码：
///
/// ```
/// # fn get_a_number() -> Option<u32> { None }
/// # loop {
/// let num: u32 = match get_a_number() {
///     Some(num) => num,
///     None => break,
/// };
/// # }
/// ```
///
/// 两个 match 分支都必须产生 [`u32`] 类型的值，但由于 `break` 根本不会产生值，
/// 我们知道它也不可能产生一个不是 [`u32`] 的值。这展示了 `!` 类型的另一种行为：
/// 类型为 `!` 的表达式会强制转换为任何其他类型。
///
/// [`u32`]: prim@u32
/// [`exit`]: ../std/process/fn.exit.html
///
/// # `!` 和泛型
///
/// ## 不可能发生的错误
///
/// 最常见的显式使用 `!` 的地方是泛型代码。考虑 [`FromStr`] trait：
///
/// ```
/// trait FromStr: Sized {
///     type Err;
///     fn from_str(s: &str) -> Result<Self, Self::Err>;
/// }
/// ```
///
/// 为 [`String`] 实现这个 trait 时，需要为 [`Err`] 选择一个类型。由于把字符串转换成
/// 字符串永远不会产生错误，合适的类型就是 `!`。（目前实际使用的是一个没有变体的 enum，
/// 这只是因为 `!` 是后来才加入 Rust 的，将来可能会改变。）当 [`Err`] 类型为 `!` 时，
/// 如果出于某种原因需要调用 [`String::from_str`]，结果会是 [`Result<String, !>`]，
/// 可以像这样解包：
///
/// ```
/// use std::str::FromStr;
/// let Ok(s) = String::from_str("hello");
/// ```
///
/// 由于 [`Err`] 变体包含 `!`，它永远不会出现。这意味着只处理 [`Ok`] 变体就能对
/// [`Result<T, !>`] 做穷尽匹配。这展示了 `!` 的另一种行为：它可以从 `Result`
/// 这样的泛型类型中“删除”某些 enum 变体。
///
/// ## 无限循环
///
/// [`Result<T, !>`] 对移除错误很有用，而 `!` 也可以用来移除成功值。如果把
/// [`Result<T, !>`] 理解为“如果这个函数返回了，它就没有出错”，那么
/// [`Result<!, E>`] 也很直观：如果函数返回了，它就*已经*出错。
///
/// 例如，考虑一个简单的 web server，可简化为：
///
/// ```ignore (hypothetical-example)
/// loop {
///     let (client, request) = get_request().expect("disconnected");
///     let response = request.process();
///     response.send(client);
/// }
/// ```
///
/// 这并不理想，因为只要获取新连接失败就会直接 panic。更好的做法是记录这个错误，
/// 例如：
///
/// ```ignore (hypothetical-example)
/// loop {
///     match get_request() {
///         Err(err) => break err,
///         Ok((client, request)) => {
///             let response = request.process();
///             response.send(client);
///         },
///     }
/// }
/// ```
///
/// 现在，当服务器断开连接时，会带着错误退出循环，而不是 panic。直接返回错误可能更直观，
/// 但也可以把它包装在 [`Result<!, E>`] 中：
///
/// ```ignore (hypothetical-example)
/// fn server_loop() -> Result<!, ConnectionError> {
///     loop {
///         let (client, request) = get_request()?;
///         let response = request.process();
///         response.send(client);
///     }
/// }
/// ```
///
/// 现在可以使用 `?` 代替 `match`，返回类型也更符合含义：如果循环停止了，就表示发生了错误。
/// 甚至不需要把循环包在 `Ok` 中，因为 `!` 会自动强制转换为
/// `Result<!, ConnectionError>`。
///
/// [`String::from_str`]: str::FromStr::from_str
/// [`String`]: ../std/string/struct.String.html
/// [`FromStr`]: str::FromStr
///
/// # `!` 和 trait
///
/// 编写自己的 trait 时，只要存在一个明显且不会 `panic!` 的实现，就应当为 `!` 提供
/// `impl`。原因是：如果某个函数返回 `impl Trait`，而 `!` 没有实现该 `Trait`，
/// 那么这个函数不能把发散作为唯一可能的代码路径。换句话说，它不能在每条代码路径上都返回
/// `!`。例如，下面的代码不能编译：
///
/// ```compile_fail
/// use std::ops::Add;
///
/// fn foo() -> impl Add<u32> {
///     unimplemented!()
/// }
/// ```
///
/// 但下面的代码可以：
///
/// ```
/// use std::ops::Add;
///
/// fn foo() -> impl Add<u32> {
///     if true {
///         unimplemented!()
///     } else {
///         0
///     }
/// }
/// ```
///
/// 原因是，在第一个示例中，`!` 可以强制转换为很多可能的类型，因为很多类型都实现了
/// `Add<u32>`。而在第二个示例中，`else` 分支返回 `0`，编译器会根据返回类型将其推断为
/// `u32`。由于 `u32` 是具体类型，`!` 可以且会被强制转换为它。关于 `!` 的这个细节，
/// 参见 issue [#36375]。
///
/// [#36375]: https://github.com/rust-lang/rust/issues/36375
///
/// 不过事实证明，大多数 trait 都可以为 `!` 提供 `impl`。以 [`Debug`] 为例：
///
/// ```
/// #![feature(never_type)]
/// # use std::fmt;
/// # trait Debug {
/// #     fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result;
/// # }
/// impl Debug for ! {
///     fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
///         *self
///     }
/// }
/// ```
///
/// 这里再次利用了 `!` 可以强制转换为任何其他类型的能力，本例中是 [`fmt::Result`]。
/// 由于此方法以 `&!` 作为参数，我们知道它永远不可能被调用（因为不存在可供调用的
/// `!` 类型的值）。写出 `*self` 实质上是在告诉编译器：“我们知道这段代码永远不会运行，
/// 所以把整个函数体当作 [`fmt::Result`] 类型处理即可”。为 `!` 实现 trait 时经常会用到
/// 这种模式。通常来说，任何只包含接收 `self` 参数的方法的 trait 都应有这样的 impl。
///
/// 另一方面，一个不适合实现的 trait 是 [`Default`]：
///
/// ```
/// trait Default {
///     fn default() -> Self;
/// }
/// ```
///
/// 由于 `!` 没有任何值，它也没有默认值。确实可以写一个只会 panic 的 `impl`，但任何类型
/// 都可以这么做（例如，只要让 [`default()`] panic，就能为 [`File`] `impl Default`）。
///
/// [`File`]: ../std/fs/struct.File.html
/// [`Debug`]: fmt::Debug
/// [`default()`]: Default::default
///
/// # Never 类型回退
///
/// 当编译器在[强制转换位置][coercion site]看到 `!` 类型的值时，会隐式插入一个强制转换，
/// 让类型检查器能够推断出任意类型：
///
// FIXME: `core::convert::absurd` 合并后改用它。
/// ```rust,ignore (illustrative-and-has-placeholders)
/// // 这段代码
/// let x: u8 = panic!();
///
/// // 会（本质上）被编译器转换为
/// let x: u8 = absurd(panic!());
///
/// // 其中 absurd 是一个具有如下签名的函数
/// // （这是健全的，因为 `!` 总是标记不可达代码）：
/// fn absurd<T>(_: !) -> T { ... }
/// ```
///
/// 如果无法推断出类型，这可能导致编译错误：
///
/// ```compile_fail
/// // 这段代码
/// { panic!() };
///
/// // 会被转换为这样
/// { absurd(panic!()) }; // error: can't infer the type of `absurd`
/// ```
///
/// 为了避免这类错误，编译器会记住插入 `absurd` 调用的位置；如果无法推断类型，
/// 就改用回退类型：
/// ```rust, ignore
/// type Fallback = /* 任意挑选的一个类型！ */;
/// { absurd::<Fallback>(panic!()) }
/// ```
///
/// 这就是所谓的 "never type fallback"。
///
/// 历史上，回退类型是 [`()`]，这会造成令人困惑的行为：即使没有回退时不会推断为 `()`，
/// `!` 也会自动强制转换为 `()`。在 [2024 edition] 中，回退类型已改为 `!`，
/// 以后也会在所有 edition 中改为 `!`。
///
/// [coercion site]: <https://doc.rust-lang.org/reference/type-coercions.html#coercion-sites>
/// [`()`]: prim@unit
/// [2024 edition]: <https://doc.rust-lang.org/edition-guide/rust-2024/never-type-fallback.html>
///
#[unstable(feature = "never_type", issue = "35121")]
mod prim_never {}

// 需要它来渲染 auto trait impl。
// 参见 src/librustdoc/passes/collect_trait_impls.rs:collect_trait_impls。
#[doc(hidden)]
impl ! {}

#[rustc_doc_primitive = "char"]
#[allow(rustdoc::invalid_rust_codeblocks)]
/// 字符类型。
///
/// `char` 类型表示一个单独字符。更准确地说，由于 Unicode 中的“字符”并不是定义良好的概念，
/// `char` 是一个 [Unicode scalar value]。
///
/// 本文档描述了 `char` 类型上的一些方法和 trait 实现。出于技术原因，
/// [the `std::char` module](char/index.html) 中还有额外的独立文档。
///
/// # 有效性与布局
///
/// `char` 是 [Unicode scalar value]，也就是除 [surrogate code point] 之外的任何
/// [Unicode code point]。它有固定的数值定义：code point 位于 0 到 0x10FFFF
/// 的闭区间内。UTF-16 使用的 surrogate code point 位于 0xD800 到 0xDFFF 的范围内。
///
/// 无论是作为字面量还是在运行时构造，都不能构造不是 Unicode scalar value 的 `char`。
/// 违反此规则会导致 undefined behavior。
///
/// ```compile_fail
/// // 下面每一个都是编译错误
/// ['\u{D800}', '\u{DFFF}', '\u{110000}'];
/// ```
///
/// ```should_panic
/// // 会 panic；from_u32 返回 None。
/// char::from_u32(0xDE01).unwrap();
/// ```
///
/// ```no_run
/// // Undefined behavior（未定义行为）
/// let _ = unsafe { char::from_u32_unchecked(0x110000) };
/// ```
///
/// Unicode scalar value 也正是可以编码为 UTF-8 的值的集合。由于 `char` 值都是
/// Unicode scalar value，并且函数可以假设[传入的 `str` 值是有效 UTF-8](primitive.str.html#invariant)，
/// 因此把任何 `char` 存入 `str`，或从 `str` 中把任何字符读作 `char`，都是安全的。
///
/// 编译器理解有效 `char` 值中的空洞，因此在下面的示例中，这两个范围会被认为覆盖了所有可能的
/// `char` 值，不会产生[非穷尽匹配][non-exhaustive match]错误。
///
/// ```
/// let c: char = 'a';
/// match c {
///     '\0' ..= '\u{D7FF}' => false,
///     '\u{E000}' ..= '\u{10FFFF}' => true,
/// };
/// ```
///
/// 所有 Unicode scalar value 都是有效的 `char` 值，但并非全部都表示真实字符。
/// 很多 Unicode scalar value 目前尚未分配给字符，但将来可能会分配（"reserved"）；
/// 有些永远不会是字符（"noncharacters"）；还有一些可能由不同用户赋予不同含义
/// （"private use"）。
///
/// 保证 `char` 在所有平台上都与 `u32` 具有相同的大小、对齐和函数调用 ABI。
/// ```
/// use std::alloc::Layout;
/// assert_eq!(Layout::new::<char>(), Layout::new::<u32>());
/// ```
///
/// [Unicode code point]: https://www.unicode.org/glossary/#code_point
/// [Unicode scalar value]: https://www.unicode.org/glossary/#unicode_scalar_value
/// [non-exhaustive match]: ../book/ch06-02-match.html#matches-are-exhaustive
/// [surrogate code point]: https://www.unicode.org/glossary/#surrogate_code_point
///
/// # 表示形式
///
/// `char` 的大小始终是四个字节。这不同于某个字符作为 [`String`] 一部分时的表示形式。
/// 例如：
///
/// ```
/// let v = vec!['h', 'e', 'l', 'l', 'o'];
///
/// // 五个元素，每个元素四个字节
/// assert_eq!(20, v.len() * size_of::<char>());
///
/// let s = String::from("hello");
///
/// // 五个元素，每个元素一个字节
/// assert_eq!(5, s.len() * size_of::<u8>());
/// ```
///
/// [`String`]: ../std/string/struct.String.html
///
/// 和往常一样，需要记住人们对“字符”的直觉并不一定映射到 Unicode 的定义。
/// 例如，尽管看起来相似，字符 'é' 是一个 Unicode code point，而 'é' 是两个
/// Unicode code point：
///
/// ```
/// let mut chars = "é".chars();
/// // U+00e9: 'latin small letter e with acute'
/// assert_eq!(Some('\u{00e9}'), chars.next());
/// assert_eq!(None, chars.next());
///
/// let mut chars = "é".chars();
/// // U+0065: 'latin small letter e'
/// assert_eq!(Some('\u{0065}'), chars.next());
/// // U+0301: 'combining acute accent'
/// assert_eq!(Some('\u{0301}'), chars.next());
/// assert_eq!(None, chars.next());
/// ```
///
/// 这意味着上面第一个字符串的内容_可以_放入一个 `char`，而第二个字符串的内容_不可以_。
/// 尝试用第二个字符串的内容创建 `char` 字面量会报错：
///
/// ```text
/// error: character literal may only contain one codepoint: 'é'
/// let c = 'é';
///         ^^^
/// ```
///
/// `char` 固定为 4 字节的另一个影响是：按 `char` 处理可能会占用多得多的内存：
///
/// ```
/// let s = String::from("love: ❤️");
/// let v: Vec<char> = s.chars().collect();
///
/// assert_eq!(12, size_of_val(&s[..]));
/// assert_eq!(32, size_of_val(&v[..]));
/// ```
#[stable(feature = "rust1", since = "1.0.0")]
mod prim_char {}

#[rustc_doc_primitive = "unit"]
#[doc(alias = "(")]
#[doc(alias = ")")]
#[doc(alias = "()")]
//
/// `()` 类型，也称为 "unit"。
///
/// `()` 类型恰好只有一个值 `()`，用于没有其他有意义的返回值时。
/// `()` 最常以隐式形式出现：没有 `-> ...` 的函数隐式具有返回类型 `()`，
/// 也就是说下面两种写法等价：
///
/// ```rust
/// fn long() -> () {}
///
/// fn short() {}
/// ```
///
/// 分号 `;` 可用于丢弃块末尾表达式的结果，使该表达式（从而也使该块）求值为 `()`。
/// 例如：
///
/// ```rust
/// fn returns_i64() -> i64 {
///     1i64
/// }
/// fn returns_unit() {
///     1i64;
/// }
///
/// let is_i64 = {
///     returns_i64()
/// };
/// let is_unit = {
///     returns_i64();
/// };
/// ```
///
#[stable(feature = "rust1", since = "1.0.0")]
mod prim_unit {}

// 需要它来渲染 auto trait impl。
// 参见 src/librustdoc/passes/collect_trait_impls.rs:collect_trait_impls。
#[doc(hidden)]
impl () {}

#[rustc_doc_primitive = "pointer"]
#[doc(alias = "ptr")]
#[doc(alias = "*")]
#[doc(alias = "*const")]
#[doc(alias = "*mut")]
//
/// 裸的 unsafe 指针，`*const T` 和 `*mut T`。
///
/// *另见 [`std::ptr` module](ptr)。*
///
/// 在 Rust 中使用 raw pointer 并不常见，通常局限于少数几种模式。raw pointer 可以越界、
/// 未对齐或为 [`null`]。然而，从 raw pointer 加载或向其存储时，它必须对给定访问是
/// [valid] 的，并且必须对齐。在 raw pointer 上使用字段表达式、元组索引表达式或数组/切片索引
/// 表达式时，遵循[边界内指针算术][`offset`]的规则。
///
/// 使用 `*ptr = data` 通过 raw pointer 存储会对旧值调用 `drop`，因此如果类型有 drop glue
/// 且内存尚未初始化，就必须使用 [`write`]；否则会在未初始化内存上调用 `drop`。
///
/// 使用 [`null`] 和 [`null_mut`] 函数创建空指针，并使用 `*const T` 和 `*mut T` 类型的
/// [`is_null`] 方法检查是否为空。`*const T` 和 `*mut T` 类型还定义了用于指针计算的
/// [`offset`] 方法。
///
/// # 创建 raw pointer 的常见方式
///
/// ## 1. 强制转换引用（`&T`）或可变引用（`&mut T`）。
///
/// ```
/// let my_num: i32 = 10;
/// let my_num_ptr: *const i32 = &my_num;
/// let mut my_speed: i32 = 88;
/// let my_speed_ptr: *mut i32 = &mut my_speed;
/// ```
///
/// 若要获得指向 boxed 值的指针，请解引用 box：
///
/// ```
/// let my_num: Box<i32> = Box::new(10);
/// let my_num_ptr: *const i32 = &*my_num;
/// let mut my_speed: Box<i32> = Box::new(88);
/// let my_speed_ptr: *mut i32 = &mut *my_speed;
/// ```
///
/// 这不会取得原始 allocation 的所有权，之后也不需要资源管理，
/// 但不得在其生命周期结束后使用该指针。
///
/// ## 2. 消耗 box（`Box<T>`）。
///
/// [`into_raw`] 函数会消耗一个 box 并返回 raw pointer。它不会销毁 `T`，
/// 也不会释放任何内存。
///
/// ```
/// let my_speed: Box<i32> = Box::new(88);
/// let my_speed: *mut i32 = Box::into_raw(my_speed);
///
/// // 不过由于获取了原始 `Box<T>` 的所有权，
/// // 我们有义务稍后将其重新组装以便销毁。
/// unsafe {
///     drop(Box::from_raw(my_speed));
/// }
/// ```
///
/// 请注意，这里调用 [`drop`] 是为了清楚表达：已经不再使用给定值，它应被销毁。
///
/// ## 3. 使用 `&raw` 创建
///
/// 除了将引用强制转换为 raw pointer，也可以使用 raw borrow 运算符：
/// `&raw const`（用于 `*const T`）和 `&raw mut`（用于 `*mut T`）。这些运算符允许为一些
/// 无法创建引用（否则会导致 undefined behavior）的字段创建 raw pointer，例如未对齐字段。
/// 涉及 packed struct 或未初始化内存时，可能需要这样做。
///
/// ```
/// #[derive(Debug, Default, Copy, Clone)]
/// #[repr(C, packed)]
/// struct S {
///     aligned: u8,
///     unaligned: u32,
/// }
/// let s = S::default();
/// let p = &raw const s.unaligned; // 强制转换（coercion）下不允许
/// ```
///
/// ## 4. 从 C 获取。
///
/// ```
/// # mod libc {
/// # pub unsafe fn malloc(_size: usize) -> *mut core::ffi::c_void { core::ptr::NonNull::dangling().as_ptr() }
/// # pub unsafe fn free(_ptr: *mut core::ffi::c_void) {}
/// # }
/// # #[cfg(false)]
/// #[allow(unused_extern_crates)]
/// extern crate libc;
///
/// unsafe {
///     let my_num: *mut i32 = libc::malloc(size_of::<i32>()) as *mut i32;
///     if my_num.is_null() {
///         panic!("failed to allocate memory");
///     }
///     libc::free(my_num as *mut core::ffi::c_void);
/// }
/// ```
///
/// 通常不会在 Rust 中直接使用 `malloc` 和 `free`，但 C API 往往会交出很多指针，
/// 因此它们是 Rust 中 raw pointer 的常见来源。
///
/// [`null`]: ptr::null
/// [`null_mut`]: ptr::null_mut
/// [`is_null`]: pointer::is_null
/// [`offset`]: pointer::offset
/// [`into_raw`]: ../std/boxed/struct.Box.html#method.into_raw
/// [`write`]: ptr::write
/// [valid]: ptr#safety
#[stable(feature = "rust1", since = "1.0.0")]
mod prim_pointer {}

#[rustc_doc_primitive = "array"]
#[doc(alias = "[]")]
#[doc(alias = "[T;N]")] // 遗憾的是 rustdoc 对 alias 没有模糊搜索
#[doc(alias = "[T; N]")]
/// 固定大小数组，写作 `[T; N]`，其中 `T` 是元素类型，`N` 是非负的编译期常量大小。
///
/// 创建数组有两种语法形式：
///
/// * 列出每个元素的列表，即 `[x, y, z]`。
/// * 重复表达式 `[expr; N]`，其中 `N` 是在数组中重复 `expr` 的次数。`expr` 必须是以下之一：
///
///   * 实现 [`Copy`] trait 的类型的值
///   * `const` 值
///
/// 注意，`[expr; 0]` 是允许的，并会产生空数组。不过它仍会求值 `expr`，
/// 并立即 drop 得到的值，因此需要留意副作用。
///
/// 如果元素类型允许，*任意*大小的数组都会实现以下 trait：
///
/// - [`Copy`]
/// - [`Clone`]
/// - [`Debug`]
/// - [`IntoIterator`] (implemented for `[T; N]`, `&[T; N]` and `&mut [T; N]`)
/// - [`PartialEq`], [`PartialOrd`], [`Eq`], [`Ord`]
/// - [`Hash`]
/// - [`AsRef`], [`AsMut`]
/// - [`Borrow`], [`BorrowMut`]
///
/// 如果元素类型允许，大小从 0 到 32（含）的数组会实现 [`Default`] trait。
/// 作为临时措施，trait 实现静态生成到大小 32 为止。
///
/// 大小从 1 到 12（含）的数组会实现 [`From<Tuple>`]，其中 `Tuple` 是长度适当的同质
/// [prim@tuple]。
///
/// 数组会强制转换为 [slices (`[T]`)][slice]，因此可以在数组上调用切片方法。
/// 实际上，这提供了处理数组的大部分 API。
///
/// 切片具有动态大小，不会强制转换为数组。请改用 `slice.try_into().unwrap()` 或
/// `<ArrayType>::try_from(slice).unwrap()`。
///
/// 数组的 `try_from(slice)` 实现（以及对应的 `slice.try_into()` 数组实现）会在输入切片长度
/// 与结果数组长度相同时成功。当优化器能轻易确定切片长度时，这些转换尤其容易被优化，
/// 例如 `<[u8; 4]>::try_from(&slice[4..8]).unwrap()`。数组实现
/// [TryFrom](crate::convert::TryFrom) 时会返回：
///
/// - `[T; N]` 从切片元素复制
/// - `&[T; N]` 引用原始切片的元素
/// - `&mut [T; N]` 引用原始切片的元素
///
/// 可以使用 [slice pattern] 从数组中移出元素。如果只想取一个元素，参见 [`mem::replace`]。
///
/// # 示例
///
/// ```
/// let mut array: [i32; 3] = [0; 3];
///
/// array[1] = 1;
/// array[2] = 2;
///
/// assert_eq!([1, 2], &array[1..]);
///
/// // 这个循环打印：0 1 2
/// for x in array {
///     print!("{x} ");
/// }
/// ```
///
/// 也可以通过数组元素的引用进行迭代：
///
/// ```
/// let array: [i32; 3] = [0; 3];
///
/// for x in &array { }
/// ```
///
/// 可以使用 `<ArrayType>::try_from(slice)` 或 `slice.try_into()` 从切片得到数组：
///
/// ```
/// let bytes: [u8; 3] = [1, 0, 2];
/// assert_eq!(1, u16::from_le_bytes(<[u8; 2]>::try_from(&bytes[0..2]).unwrap()));
/// assert_eq!(512, u16::from_le_bytes(bytes[1..3].try_into().unwrap()));
/// ```
///
/// 可以使用 [slice pattern] 从数组中移出元素：
///
/// ```
/// fn move_away(_: String) { /* 做一些有趣的事情。 */ }
///
/// let [john, roa] = ["John".to_string(), "Roa".to_string()];
/// move_away(john);
/// move_away(roa);
/// ```
///
/// 可以从长度适当的同质元组创建数组：
///
/// ```
/// let tuple: (u32, u32, u32) = (1, 2, 3);
/// let array: [u32; 3] = tuple.into();
/// ```
///
/// # 版本(Edition)
///
/// 在 Rust 1.53 之前，数组没有按值实现 [`IntoIterator`]，因此方法调用
/// `array.into_iter()` 会自动取引用并得到 [slice iterator](slice::iter)。目前，
/// 为了兼容性，Rust 2015 和 2018 edition 保留了旧行为，会忽略按值的 [`IntoIterator`]。
/// 将来，2015 和 2018 edition 上的行为可能会与后续 edition 保持一致。
///
/// ```rust,edition2018
/// // Rust 2015 和 2018：
///
/// # #![allow(array_into_iter)] // override our `deny(warnings)`
/// let array: [i32; 3] = [0; 3];
///
/// // 这会创建一个 slice 迭代器，产生指向每个值的引用。
/// for item in array.into_iter().enumerate() {
///     let (i, x): (usize, &i32) = item;
///     println!("array[{i}] = {x}");
/// }
///
/// // `array_into_iter` lint 建议做如下修改以保证未来兼容性：
/// for item in array.iter().enumerate() {
///     let (i, x): (usize, &i32) = item;
///     println!("array[{i}] = {x}");
/// }
///
/// // 你可以用 `IntoIterator::into_iter` 显式地按值迭代数组
/// for item in IntoIterator::into_iter(array).enumerate() {
///     let (i, x): (usize, i32) = item;
///     println!("array[{i}] = {x}");
/// }
/// ```
///
/// 从 2021 edition 开始，`array.into_iter()` 会正常使用 `IntoIterator` 按值迭代；
/// 若要像旧 edition 那样按引用迭代，应使用 `iter()`。
///
/// ```rust,edition2021
/// // Rust 2021：
///
/// let array: [i32; 3] = [0; 3];
///
/// // 这会按引用迭代：
/// for item in array.iter().enumerate() {
///     let (i, x): (usize, &i32) = item;
///     println!("array[{i}] = {x}");
/// }
///
/// // 这会按值迭代：
/// for item in array.into_iter().enumerate() {
///     let (i, x): (usize, i32) = item;
///     println!("array[{i}] = {x}");
/// }
/// ```
///
/// 未来的语言版本可能会开始把 2015 和 2018 edition 中的 `array.into_iter()` 语法视为与
/// 2021 edition 相同。因此，使用这些较旧 edition 的代码仍应将这一变化纳入考虑，
/// 以避免将来破坏兼容性。最安全的做法是在这些 edition 中避免使用 `into_iter` 语法。
/// 如果无法或不想升级 edition，有几个替代方案：
/// * 使用 `iter`，等价于旧行为，会创建引用
/// * 使用 [`IntoIterator::into_iter`]，等价于 2021 之后的行为（Rust 1.53+）
/// * 将 `for ... in array.into_iter() {` 替换为 `for ... in array {`，
///   等价于 2021 之后的行为（Rust 1.53+）
///
/// ```rust,edition2018
/// // Rust 2015 和 2018：
///
/// let array: [i32; 3] = [0; 3];
///
/// // 这会按引用迭代：
/// for item in array.iter() {
///     let x: &i32 = item;
///     println!("{x}");
/// }
///
/// // 这会按值迭代：
/// for item in IntoIterator::into_iter(array) {
///     let x: i32 = item;
///     println!("{x}");
/// }
///
/// // 这会按值迭代：
/// for item in array {
///     let x: i32 = item;
///     println!("{x}");
/// }
///
/// // IntoIter 也可以作为链式调用的起点。
/// // 这会按值迭代：
/// for item in IntoIterator::into_iter(array).enumerate() {
///     let (i, x): (usize, i32) = item;
///     println!("array[{i}] = {x}");
/// }
/// ```
///
/// [slice]: prim@slice
/// [`Debug`]: fmt::Debug
/// [`Hash`]: hash::Hash
/// [`Borrow`]: borrow::Borrow
/// [`BorrowMut`]: borrow::BorrowMut
/// [slice pattern]: ../reference/patterns.html#slice-patterns
/// [`From<Tuple>`]: convert::From
#[stable(feature = "rust1", since = "1.0.0")]
mod prim_array {}

#[rustc_doc_primitive = "slice"]
#[doc(alias = "[")]
#[doc(alias = "]")]
#[doc(alias = "[]")]
/// 指向连续序列的动态大小视图，`[T]`。
///
/// 这里的连续意味着元素按布局排列，使每个元素与相邻元素之间的距离都相同。
///
/// *另见 [`std::slice` module](crate::slice)。*
///
/// 切片是某块内存的视图，由一个指针和一个长度表示。
///
/// ```
/// // 对 Vec 进行切片
/// let vec = vec![1, 2, 3];
/// let int_slice = &vec[..];
/// // 将数组强制转换为切片
/// let str_slice: &[&str] = &["one", "two", "three"];
/// ```
///
/// 切片可以是可变的，也可以是共享的。共享切片类型是 `&[T]`，可变切片类型是 `&mut [T]`，
/// 其中 `T` 表示元素类型。例如，可以修改可变切片指向的内存块：
///
/// ```
/// let mut x = [1, 2, 3];
/// let x = &mut x[..]; // 取 `x` 的完整切片。
/// x[1] = 7;
/// assert_eq!(x, &[1, 7, 3]);
/// ```
///
/// 可以使用空范围对切片取空子范围（包括 `slice.len()..slice.len()`）：
/// ```
/// let x = [1, 2, 3];
/// let empty = &x[0..0];   // 第一个元素之前的子切片
/// assert_eq!(empty, &[]);
/// let empty = &x[..0];    // same as &x[0..0]
/// assert_eq!(empty, &[]);
/// let empty = &x[1..1];   // 中间的空子切片
/// assert_eq!(empty, &[]);
/// let empty = &x[3..3];   // 最后一个元素之后的子切片
/// assert_eq!(empty, &[]);
/// let empty = &x[3..];    // same as &x[3..3]
/// assert_eq!(empty, &[]);
/// ```
///
/// 不允许使用起始下界大于 `slice.len()` 的子范围：
/// ```should_panic
/// let x = vec![1, 2, 3];
/// let _ = &x[4..4];
/// ```
///
/// 由于切片会存储所引用序列的长度，因此它们的大小是指向
/// [`Sized`](marker/trait.Sized.html) 类型的指针的两倍。另见 reference 中关于
/// [dynamically sized types](../reference/dynamically-sized-types.html) 的说明。
///
/// ```
/// # use std::rc::Rc;
/// let pointer_size = size_of::<&u8>();
/// assert_eq!(2 * pointer_size, size_of::<&[u8]>());
/// assert_eq!(2 * pointer_size, size_of::<*const [u8]>());
/// assert_eq!(2 * pointer_size, size_of::<Box<[u8]>>());
/// assert_eq!(2 * pointer_size, size_of::<Rc<[u8]>>());
/// ```
///
/// ## Trait 实现
///
/// 如果元素类型实现了某个 trait，切片也会实现其中一些 trait，包括 [`Eq`]、[`Hash`] 和
/// [`Ord`]。
///
/// ## 迭代
///
/// 切片实现了 `IntoIterator`。该迭代器会产生切片元素的引用。
///
/// ```
/// let numbers: &[i32] = &[0, 1, 2];
/// for n in numbers {
///     println!("{n} is a number!");
/// }
/// ```
///
/// 可变切片会产生元素的可变引用：
///
/// ```
/// let mut scores: &mut [i32] = &mut [7, 8, 9];
/// for score in scores {
///     *score += 1;
/// }
/// ```
///
/// 这个迭代器会产生切片元素的可变引用，因此虽然切片的元素类型是 `i32`，
/// 迭代器的元素类型却是 `&mut i32`。
///
/// * [`.iter`] 和 [`.iter_mut`] 是显式返回默认迭代器的方法。
/// * 其他返回迭代器的方法还包括 [`.split`]、[`.splitn`]、[`.chunks`]、[`.windows`] 等。
///
/// [`Hash`]: core::hash::Hash
/// [`.iter`]: slice::iter
/// [`.iter_mut`]: slice::iter_mut
/// [`.split`]: slice::split
/// [`.splitn`]: slice::splitn
/// [`.chunks`]: slice::chunks
/// [`.windows`]: slice::windows
#[stable(feature = "rust1", since = "1.0.0")]
mod prim_slice {}

#[rustc_doc_primitive = "str"]
/// 字符串切片。
///
/// *另见 [`std::str` module](crate::str)。*
///
/// `str` 类型也称为“字符串切片”，是最基础的字符串类型。它通常以借用形式 `&str` 出现。
/// 它也是字符串字面量的类型，即 `&'static str`。
///
/// # 基本用法
///
/// 字符串字面量就是字符串切片：
///
/// ```
/// let hello_world = "Hello, World!";
/// ```
///
/// 这里声明了一个用字符串字面量初始化的字符串切片。字符串字面量具有 static 生命周期，
/// 这意味着字符串 `hello_world` 保证在整个程序持续期间都有效。也可以显式指定
/// `hello_world` 的生命周期：
///
/// ```
/// let hello_world: &'static str = "Hello, world!";
/// ```
///
/// # 表示形式
///
/// `&str` 由两个组件组成：指向某些字节的指针，以及长度。可以用 [`as_ptr`] 和 [`len`]
/// 方法查看它们：
///
/// ```
/// use std::slice;
/// use std::str;
///
/// let story = "Once upon a time...";
///
/// let ptr = story.as_ptr();
/// let len = story.len();
///
/// // story 有十九个字节
/// assert_eq!(19, len);
///
/// // 我们可以用 ptr 和 len 重新构建一个 str。这一切都是 unsafe 的，因为
/// // 我们有责任确保这两个组成部分是有效的：
/// let s = unsafe {
///     // 首先，我们构建一个 &[u8]...
///     let slice = slice::from_raw_parts(ptr, len);
///
///     // ... 然后将该切片转换为字符串切片
///     str::from_utf8(slice)
/// };
///
/// assert_eq!(s, Ok(story));
/// ```
///
/// [`as_ptr`]: str::as_ptr
/// [`len`]: str::len
///
/// 注意：此示例展示了 `&str` 的内部结构。通常不应使用 `unsafe` 来获取字符串切片，
/// 请改用 `as_str`。
///
/// # 不变量
///
/// Rust 库可以假设字符串切片始终是有效的 UTF-8。
///
/// 构造非 UTF-8 字符串切片不会立即造成 undefined behavior，但在字符串切片上调用的任何函数
/// 都可以假设它是有效 UTF-8，这意味着非 UTF-8 字符串切片之后可能导致 undefined behavior。
#[stable(feature = "rust1", since = "1.0.0")]
mod prim_str {}

#[rustc_doc_primitive = "tuple"]
#[doc(alias = "(")]
#[doc(alias = ")")]
#[doc(alias = "()")]
//
/// 有限的异质序列，`(T, U, ..)`。
///
/// 下面逐项说明：
///
/// 元组是*有限的*。换句话说，元组有长度。下面是一个长度为 `3` 的元组：
///
/// ```
/// ("hello", 5, 'c');
/// ```
///
/// 这里的“长度”有时也称为 "arity"；不同长度的每个元组都是不同的独立类型。
///
/// 元组是*异质的*。这意味着元组中的每个元素都可以具有不同类型。上面的元组具有以下类型：
///
/// ```
/// # let _:
/// (&'static str, i32, char)
/// # = ("hello", 5, 'c');
/// ```
///
/// 元组是一个*序列*。这意味着可以按位置访问其中的元素；这称为“元组索引”，形式如下：
///
/// ```rust
/// let tuple = ("hello", 5, 'c');
///
/// assert_eq!(tuple.0, "hello");
/// assert_eq!(tuple.1, 5);
/// assert_eq!(tuple.2, 'c');
/// ```
///
/// 元组的顺序性也适用于它对各种 trait 的实现。例如，在 [`PartialOrd`] 和 [`Ord`] 中，
/// 会按顺序比较元素，直到找到第一组不相等的元素。
///
/// 关于元组的更多信息，参见 [the book](../book/ch03-02-data-types.html#the-tuple-type)。
///
// src/librustdoc/html/format.rs 中硬编码的锚点。
// 链接目标为 `#trait-implementations-1`。
/// # Trait 实现
///
/// 本文档使用简写 `(T₁, T₂, …, Tₙ)` 表示不同长度的元组。使用这种写法时，
/// 任何写在 `T` 上的 trait bound 都会独立应用于元组的每个元素。注意，这只是为了避免重复文档
/// 而使用的便利记法，不是有效的 Rust 语法。
///
/// 由于 Rust 类型系统中的一个临时限制，以下 trait 只为 arity 不超过 12 的元组实现。
/// 将来这可能会改变：
///
/// * [`PartialEq`]
/// * [`Eq`]
/// * [`PartialOrd`]
/// * [`Ord`]
/// * [`Debug`]
/// * [`Default`]
/// * [`Hash`]
/// * [`From<[T; N]>`][from]
///
/// [from]: convert::From
/// [`Debug`]: fmt::Debug
/// [`Hash`]: hash::Hash
///
/// 以下 trait 为任意长度的元组实现。这些 trait 的实现由编译器自动生成，
/// 因而不受缺失语言功能的限制。
///
/// * [`Clone`]
/// * [`Copy`]
/// * [`Send`]
/// * [`Sync`]
/// * [`Unpin`]
/// * [`UnwindSafe`]
/// * [`RefUnwindSafe`]
///
/// [`UnwindSafe`]: panic::UnwindSafe
/// [`RefUnwindSafe`]: panic::RefUnwindSafe
///
/// # 示例
///
/// 基本用法：
///
/// ```
/// let tuple = ("hello", 5, 'c');
///
/// assert_eq!(tuple.0, "hello");
/// ```
///
/// 需要返回多个值时，元组常被用作返回类型：
///
/// ```
/// fn calculate_point() -> (i32, i32) {
///     // 别做真正的计算，那不是这个示例的重点
///     (4, 5)
/// }
///
/// let point = calculate_point();
///
/// assert_eq!(point.0, 4);
/// assert_eq!(point.1, 5);
///
/// // 将其与模式结合起来会更好。
///
/// let (x, y) = calculate_point();
///
/// assert_eq!(x, 4);
/// assert_eq!(y, 5);
/// ```
///
/// 可以从长度适当的数组创建同质元组：
///
/// ```
/// let array: [u32; 3] = [1, 2, 3];
/// let tuple: (u32, u32, u32) = array.into();
/// ```
///
#[stable(feature = "rust1", since = "1.0.0")]
mod prim_tuple {}

// 需要它来渲染 auto trait impl。
// 参见 src/librustdoc/passes/collect_trait_impls.rs:collect_trait_impls。
#[doc(hidden)]
impl<T> (T,) {}

#[rustc_doc_primitive = "f16"]
#[doc(alias = "half")]
/// 16 位浮点类型（具体来说，是 IEEE 754-2008 中定义的 "binary16" 类型）。
///
/// 此类型与 [`prim@f32`] 非常相似，但由于只使用一半的位数，精度较低。
/// 更多信息请参见 [`f32` 的文档](prim@f32)或 [Wikipedia on half-precision
/// values][wikipedia]。
///
/// 注意，除 Apple Silicon（也称为 M1、M2 等）处理器这个显著例外外，大多数常见平台如果不启用
/// 额外 target feature，就没有硬件 `f16` 支持。x86/x86-64 上的硬件支持需要 avx512fp16 或
/// avx10.1 feature；RISC-V 需要 Zfh；Arm/AArch64 需要 FEAT_FP16。通常，回退实现会在存在
/// `f32` 硬件时使用它，并在执行数学运算时在 `f16` 和 `f32` 之间转换。
///
/// *另见 [`std::f16::consts` module](crate::f16::consts)。*
///
/// [wikipedia]: https://en.wikipedia.org/wiki/Half-precision_floating-point_format
#[unstable(feature = "f16", issue = "116909")]
mod prim_f16 {}

#[rustc_doc_primitive = "f32"]
#[doc(alias = "single")]
/// 32 位浮点类型（具体来说，是 IEEE 754-2008 中定义的 "binary32" 类型）。
///
/// 此类型可以表示范围很广的十进制数，例如 `3.5`、`27`、`-113.75`、`0.0078125`、
/// `34359738368`、`0`、`-1`。因此，与整数类型（如 `i32`）不同，浮点类型也可以表示非整数。
///
/// 不过，能够表示这么宽的数值范围是以精度为代价的：浮点数只能表示一部分实数，
/// 使用浮点数计算时会舍入到附近可表示的数。例如，`5.0` 和 `1.0` 可以精确表示为 `f32`，
/// 但 `1.0 / 5.0` 的结果是 `0.20000000298023223876953125`，因为 `0.2` 无法精确表示为
/// `f32`。不过请注意，使用 `println` 等打印浮点数时，通常会丢弃不重要的数字：
/// `println!("{}", 1.0f32 / 5.0f32)` 会打印 `0.2`。
///
/// 此外，`f32` 还能表示一些特殊值：
///
/// - −0.0：IEEE 754 浮点数有一位表示符号，因此 −0.0 是可能的值。比较时 −0.0 = +0.0，
///   但浮点运算可以在算术操作中携带符号位。这意味着 −0.0 × +0.0 会产生 −0.0，
///   而负数被舍入到小于浮点数可表示范围的值时也会产生 −0.0。
/// - [∞](#associatedconstant.INFINITY) and
///   [−∞](#associatedconstant.NEG_INFINITY)：它们来自类似 `1.0 / 0.0` 的计算。
/// - [NaN (not a number)](#associatedconstant.NAN)：这个值来自类似 `(-1.0).sqrt()` 的计算。
///   NaN 有一些可能出乎意料的行为：
///   - 它不等于任何浮点数，包括它自己！这就是 `f32` 不实现 `Eq` trait 的原因。
///   - 它也既不小于也不大于任何浮点数，因此无法使用默认比较操作排序，
///     这就是 `f32` 不实现 `Ord` trait 的原因。
///   - 它还被认为是*传染性的*，因为只要某个操作数为 NaN，几乎所有计算的结果也会是 NaN。
///     本页的说明只有在偏离这个默认行为时，才会显式记录 NaN 操作数上的行为。
///   - 最后，有多个 bit pattern 会被视为 NaN。Rust 目前不保证 NaN 的 bit pattern 会在算术操作中
///     被保留，也不保证它们可移植，甚至不保证完全确定！这意味着检查 bit pattern 时可能看到一些
///     意外结果，因为相同计算可能产生具有不同 bit pattern 的 NaN。这也会影响 NaN 的符号：
///     在 NaN 上检查 `is_sign_positive` 或 `is_sign_negative` 是最常遇到这些意外结果的方式。
///     （检查 `x >= 0.0` 或 `x <= 0.0` 可以避免这些意外，但也会影响负零/正零的处理。）
///     关于 NaN bit pattern 具体保证了什么，请参见下面的章节。
///
/// 在此类型上执行原语运算（加法、减法、乘法或除法）时，结果会按照 IEEE 754-2008
/// 定义的 roundTiesToEven 方向舍入。这意味着：
///
/// - 如果存在唯一最接近真实值的可表示值，结果就是该值。
/// - 如果真实值恰好位于两个可表示值正中间，结果是最低有效二进制位为偶数的那个值。
/// - 如果真实值的幅度 ≥ `f32::MAX` + 2<sup>(`f32::MAX_EXP` −
///   `f32::MANTISSA_DIGITS` − 1)</sup>，结果为 ∞ 或 −∞（保留真实值的符号）。
/// - 如果求和结果恰好等于零，则结果为 +0.0，除非两个参数都是负数，此时结果为 -0.0。
///   减法 `a - b` 会被视为求和 `a + (-b)`。
///
/// 关于浮点数的更多信息，参见 [Wikipedia][wikipedia]。
///
/// *另见 [`std::f32::consts` module](crate::f32::consts)。*
///
/// [wikipedia]: https://en.wikipedia.org/wiki/Single-precision_floating-point_format
///
/// # NaN bit patterns
///
/// 本节定义浮点运算可能返回的 NaN bit pattern。
///
/// 浮点 NaN 值的 bit pattern 由以下部分定义：
/// - 符号位。
/// - quiet/signaling 位。Rust 假设 quiet/signaling 位设为 `1` 表示 quiet NaN (QNaN)，
///   值为 `0` 表示 signaling NaN (SNaN)。下文中会直接称其为 "quiet bit"。
/// - payload，它构成 significand（即 mantissa）中除 quiet bit 外的其余部分。
///
/// NaN 值的规则在*算术*操作和*非算术*（或 "bitwise"）操作之间不同。非算术操作包括一元 `-`、
/// `abs`、`copysign`、`signum`、`{to,from}_bits`、`{to,from}_{be,le,ne}_bytes` 以及
/// `is_sign_{positive,negative}`。这些操作保证精确保留输入的 bit pattern，可能改变符号位除外。
///
/// 当算术操作返回 NaN 值时，适用以下规则：
/// - 结果具有非确定性的符号。
/// - quiet bit 和 payload 会从以下选项集合中非确定性地选择：
///
///   - **Preferred NaN**：quiet bit 被设置，payload 全为零。
///   - **Quieting NaN propagation**：quiet bit 被设置，payload 从任意 NaN 输入操作数复制。
///     如果输入和输出的 payload 大小不同（即 `as` 转换），则：
///     - 如果输出小于输入，会丢弃 payload 的低位。
///     - 如果输出大于输入，会用 0 填充 payload 的低位。
///   - **Unchanged NaN propagation**：quiet bit 和 payload 从任意 NaN 输入操作数复制。
///     如果输入和输出大小不同（即 `as` 转换），适用与 "quieting NaN propagation" 相同的规则，
///     但有一个注意点：如果输出小于输入，丢弃低位可能导致 payload 为 0；payload 为 0
///     不可能对应 signaling NaN（全 0 significand 编码 infinity），因此 unchanged NaN propagation
///     对某些输入不会发生。
///   - **Target-specific NaN**：quiet bit 被设置，payload 从目标特定的“额外”可能 NaN payload
///     集合中选取。该集合可以依赖输入操作数的值。各目标上这个集合包含的具体 NaN 见下表。
///
/// 特别地，如果所有输入 NaN 都是 quiet（或者没有输入 NaN），那么输出 NaN 一定是 quiet。
/// 只有在输入值中提供 signaling NaN 时，才可能产生 signaling NaN 输出。类似地，
/// 如果所有输入 NaN 都是 preferred（或者没有输入 NaN），且目标没有任何“额外”NaN payload，
/// 那么保证输出 NaN 为 preferred。
///
/// 非确定性选择发生在操作执行时；也就是说，产生 NaN 的浮点操作的结果是稳定的 bit pattern
/// （多次查看这些位会得到一致结果），但使用相同输入运行同一操作两次可能产生不同结果。
///
/// 这些保证既不强于也不弱于 IEEE 754：IEEE 754 保证操作永不返回 signaling NaN，
/// 而 Rust 中类似 `SNAN * 1.0` 的操作可能返回 signaling NaN。反过来，IEEE 754
/// 完全不规定返回哪个 quiet NaN，而 Rust 将可能结果限制为上面列出的集合。
///
/// 除非另有说明，相同规则也适用于其他库函数返回的 NaN（例如 `min`、`minimum`、`max`、
/// `maximum`）；这些函数语义的其他方面，以及它们对应的 IEEE 754 操作，会在相应函数文档中说明。
///
/// 当算术浮点操作在 `const` 上下文中执行时，适用相同规则：不保证会返回上述哪一种 NaN bit pattern。
/// 结果不必与运行时执行同一代码时一致，并且结果可能随编译器版本、flag 等因素变化。
///
/// ### 目标特定的“额外”NaN 值
// FIXME: 有没有更合适的地方放这段?
///
/// | `target_arch` | 此平台上可能的额外 payload |
/// |---------------|------------------------------------------|
// 按字母顺序排序
/// | `aarch64`, `arm`, `arm64ec`, `loongarch64`, `powerpc` (except when `target_abi = "spe"`), `powerpc64`, `riscv32`, `riscv64`, `s390x`, `x86`, `x86_64` | 无 |
/// | `nvptx64` | 所有 payload |
/// | `sparc`, `sparc64` | 全 1 payload |
/// | `wasm32`, `wasm64` | 如果所有输入 NaN 都是 quiet 且 payload 全为零：无。<br> 否则：所有 payload。 |
///
/// 对于表中未列出的目标，所有 payload 都是可能的。
///
/// # 代数运算符
///
/// `a.algebraic_*(b)` 形式的代数运算符允许编译器使用实数通常具备的全部代数性质
/// 来优化浮点运算，即使这些性质在浮点数上*并不*成立。
/// 这可能解锁向量化，从而显著提升性能。
///
/// 具体允许的优化集合未作规定，但通常允许合并操作、根据数学性质重排一系列操作、
/// 在除法与乘以倒数之间转换，以及忽略零的符号。这意味着基本运算的结果可能具有
/// 未定义的精度，而 NaN、+/-Inf 或 -0.0 这类“非数学”值可能以意外方式表现；
/// 但这些操作绝不会造成未定义行为。
///
/// 由于编译器优化具有不可预测性，即使在单次程序运行期间，相同输入也可能产生不同结果。
/// **unsafe code 不得依赖返回值的任何性质来保证健全性。**
/// 不过，实现通常会尽力在性能和结果准确性之间选择合理的折中。
///
/// 例如：
///
/// ```
/// # #![feature(float_algebraic)]
/// # #![allow(unused_assignments)]
/// # let mut x: f32 = 0.0;
/// # let a: f32 = 1.0;
/// # let b: f32 = 2.0;
/// # let c: f32 = 3.0;
/// # let d: f32 = 4.0;
/// x = a.algebraic_add(b).algebraic_add(c).algebraic_add(d);
/// ```
///
/// 可能被改写为：
///
/// ```
/// # #![allow(unused_assignments)]
/// # let mut x: f32 = 0.0;
/// # let a: f32 = 1.0;
/// # let b: f32 = 2.0;
/// # let c: f32 = 3.0;
/// # let d: f32 = 4.0;
/// x = a + b + c + d; // As written
/// x = (a + c) + (b + d); // 重排以缩短关键路径并启用向量化
/// ```
#[stable(feature = "rust1", since = "1.0.0")]
mod prim_f32 {}

#[rustc_doc_primitive = "f64"]
#[doc(alias = "double")]
/// 64 位浮点类型（具体而言，是 IEEE 754-2008 定义的 "binary64" 类型）。
///
/// 此类型与 [`prim@f32`] 非常相似，但使用两倍的位数来提高精度。更多信息请参阅
/// [`f32` 的文档](prim@f32)或 [Wikipedia 上关于双精度值的条目][wikipedia]。
///
/// *[另请参阅 `std::f64::consts` 模块](crate::f64::consts)。*
///
/// [wikipedia]: https://en.wikipedia.org/wiki/Double-precision_floating-point_format
#[stable(feature = "rust1", since = "1.0.0")]
mod prim_f64 {}

#[rustc_doc_primitive = "f128"]
#[doc(alias = "quad")]
/// 128 位浮点类型（具体而言，是 IEEE 754-2008 定义的 "binary128" 类型）。
///
/// 此类型与 [`prim@f32`] 和 [`prim@f64`] 非常相似，但使用 `f64` 两倍的位数来提高精度。
/// 更多信息请参阅 [`f32` 的文档](prim@f32)或
/// [Wikipedia 上关于四精度值的条目][wikipedia]。
///
/// 注意，如果不启用目标特定 feature，没有任何平台为 `f128` 提供硬件支持；
/// 对所有指令集架构而言，`f128` 都被视为可选 feature。只有 Power ISA ("PowerPC")
/// 和 RISC-V（通过 Q 扩展）规定了它，并且只有部分微架构实际实现了它。
/// 对于 x86-64 和 AArch64，甚至没有规定 ISA 支持，因此它始终是明显慢于 `f64`
/// 的软件实现。
///
/// _注意：`f128` 支持尚不完整。许多平台将无法链接数学函数。尤其是在 x86 上，
/// 这些函数虽然可以链接，但结果始终不正确。_
///
/// *[另请参阅 `std::f128::consts` 模块](crate::f128::consts)。*
///
/// [wikipedia]: https://en.wikipedia.org/wiki/Quadruple-precision_floating-point_format
#[unstable(feature = "f128", issue = "116909")]
mod prim_f128 {}

#[rustc_doc_primitive = "i8"]
//
/// 8 位有符号整数类型。
#[stable(feature = "rust1", since = "1.0.0")]
mod prim_i8 {}

#[rustc_doc_primitive = "i16"]
//
/// 16 位有符号整数类型。
#[stable(feature = "rust1", since = "1.0.0")]
mod prim_i16 {}

#[rustc_doc_primitive = "i32"]
//
/// 32 位有符号整数类型。
#[stable(feature = "rust1", since = "1.0.0")]
mod prim_i32 {}

#[rustc_doc_primitive = "i64"]
//
/// 64 位有符号整数类型。
#[stable(feature = "rust1", since = "1.0.0")]
mod prim_i64 {}

#[rustc_doc_primitive = "i128"]
//
/// 128 位有符号整数类型。
///
/// # ABI 兼容性
///
/// 在提供 C `__int128` 的平台上，Rust 的 `i128` 预期与其 ABI 兼容，这包括大多数
/// 64 位架构。如果某些未规定 `__int128` 的平台更新后引入了它，相关目标上的 Rust
/// `i128` ABI 将被改为与之匹配。
///
/// 需要注意的是，在 C 中，`__int128` 与 `_BitInt(128)` _并不_相同，并且这两个类型
/// 允许具有不同的 ABI。尤其是在 x86 上，`__int128` 与 `_BitInt(128)` 不使用相同的对齐。
/// `i128` 旨在始终匹配 `__int128`，而不会在没有 `__int128` 的平台上尝试匹配
/// `_BitInt(128)`。
#[stable(feature = "i128", since = "1.26.0")]
mod prim_i128 {}

#[rustc_doc_primitive = "u8"]
//
/// 8 位无符号整数类型。
#[stable(feature = "rust1", since = "1.0.0")]
mod prim_u8 {}

#[rustc_doc_primitive = "u16"]
//
/// 16 位无符号整数类型。
#[stable(feature = "rust1", since = "1.0.0")]
mod prim_u16 {}

#[rustc_doc_primitive = "u32"]
//
/// 32 位无符号整数类型。
#[stable(feature = "rust1", since = "1.0.0")]
mod prim_u32 {}

#[rustc_doc_primitive = "u64"]
//
/// 64 位无符号整数类型。
#[stable(feature = "rust1", since = "1.0.0")]
mod prim_u64 {}

#[rustc_doc_primitive = "u128"]
//
/// 128 位无符号整数类型。
///
/// 关于 ABI 兼容性的信息，请参阅 [`i128` 的文档](prim@i128)。
#[stable(feature = "i128", since = "1.26.0")]
mod prim_u128 {}

#[rustc_doc_primitive = "isize"]
//
/// 指针大小的有符号整数类型。
///
/// 此原语的大小等于引用内存中任意位置所需的字节数。例如，在 32 位目标上它是 4 字节，
/// 在 64 位目标上它是 8 字节。
#[stable(feature = "rust1", since = "1.0.0")]
mod prim_isize {}

#[rustc_doc_primitive = "usize"]
//
/// 指针大小的无符号整数类型。
///
/// 此原语的大小等于引用内存中任意位置所需的字节数。例如，在 32 位目标上它是 4 字节，
/// 在 64 位目标上它是 8 字节。
#[stable(feature = "rust1", since = "1.0.0")]
mod prim_usize {}

#[rustc_doc_primitive = "reference"]
#[doc(alias = "&")]
#[doc(alias = "&mut")]
//
/// 引用，即 `&T` 和 `&mut T`。
///
/// 引用表示对某个被拥有值的借用。可以通过对值使用 `&` 或 `&mut` 运算符来取得引用，
/// 也可以通过 [`ref`](../std/keyword.ref.html) 或
/// <code>[ref](../std/keyword.ref.html) [mut](../std/keyword.mut.html)</code> 模式取得引用。
///
/// 对熟悉指针的人来说，引用只是一个被假定为已对齐、非 null，且指向包含有效 `T` 值的
/// 内存的指针。例如，<code>&[bool]</code> 只能指向包含整数值 `1`
/// ([`true`](../std/keyword.true.html)) 或 `0` ([`false`](../std/keyword.false.html)) 的分配，
/// 但创建一个指向包含值 `3` 的分配的 <code>&[bool]</code> 会造成未定义行为。
/// 实际上，<code>[Option]\<&T></code> 与可为 null 但已对齐的指针具有相同的内存表示，
/// 并且可以按这种形式跨 FFI 边界传递。
///
/// 在大多数情况下，引用可以像原始值一样使用。字段访问、方法调用和索引的工作方式相同
/// （当然，可变性规则除外）。此外，比较运算符会透明地委托给被引用对象的实现，
/// 因而引用可以像被拥有值一样比较。
///
/// 引用带有一个 lifetime，表示该借用有效的作用域。如果某个 lifetime 所代表的作用域
/// 与另一个一样长或更长，就称它 "outlive" 另一个 lifetime。`'static` lifetime 是最长的
/// lifetime，表示程序的整个生命周期。例如，字符串字面量具有 `'static` lifetime，
/// 因为文本数据嵌入在程序二进制文件中，而不是位于需要动态管理的分配中。
///
/// `&mut T` 引用可以自由强制转换为具有相同被引用类型的 `&T` 引用，
/// lifetime 较长的引用也可以自由强制转换为 lifetime 较短的引用。
///
/// [`PartialEq`] 会比较被引用的值。也可以通过引用到指针的强制转换，
/// 并借助 [`ptr::eq`] 的原始指针相等性来比较引用地址。
///
/// ```
/// use std::ptr;
///
/// let five = 5;
/// let other_five = 5;
/// let five_ref = &five;
/// let same_five_ref = &five;
/// let other_five_ref = &other_five;
///
/// assert!(five_ref == same_five_ref);
/// assert!(five_ref == other_five_ref);
///
/// assert!(ptr::eq(five_ref, same_five_ref));
/// assert!(!ptr::eq(five_ref, other_five_ref));
/// ```
///
/// 关于如何使用引用的更多信息，请参阅 [the book 中关于 "References and
/// Borrowing" 的章节][book-refs]。
///
/// [book-refs]: ../book/ch04-02-references-and-borrowing.html
///
/// # Trait 实现
///
/// 以下 trait 为所有 `&T` 实现，无论其被引用对象的类型是什么：
///
/// * [`Copy`]
/// * [`Clone`] \(注意，即使 `T` 存在 `Clone` 实现，也不会委托给它！)
/// * [`Deref`]
/// * [`Borrow`]
/// * [`fmt::Pointer`]
///
/// [`Deref`]: ops::Deref
/// [`Borrow`]: borrow::Borrow
///
/// `&mut T` 引用会获得以上除 `Copy` 和 `Clone` 之外的所有实现（以防止创建多个同时存在的可变借用），
/// 并且还会获得以下实现，无论其被引用对象的类型是什么：
///
/// * [`DerefMut`]
/// * [`BorrowMut`]
///
/// [`DerefMut`]: ops::DerefMut
/// [`BorrowMut`]: borrow::BorrowMut
/// [bool]: prim@bool
///
/// 如果底层的 `T` 也实现了相应 trait，则 `&T` 引用会实现以下 trait：
///
/// * [`std::fmt`] 中除 [`fmt::Pointer`]（无论被引用对象类型如何都会实现）和 [`fmt::Write`] 之外的所有 trait
/// * [`PartialOrd`]
/// * [`Ord`]
/// * [`PartialEq`]
/// * [`Eq`]
/// * [`AsRef`]
/// * [`Fn`] \(此外，如果 `T: Fn`，`&T` 引用还会获得 [`FnMut`] 和 [`FnOnce`])
/// * [`Hash`]
/// * [`ToSocketAddrs`]
/// * [`Sync`]
///
/// [`std::fmt`]: fmt
/// [`Hash`]: hash::Hash
/// [`ToSocketAddrs`]: ../std/net/trait.ToSocketAddrs.html
///
/// 如果 `T` 实现相应 trait，则 `&mut T` 引用会获得以上除 `ToSocketAddrs` 之外的所有实现，
/// 并额外获得以下实现：
///
/// * [`AsMut`]
/// * [`FnMut`] \(此外，如果 `T: FnMut`，`&mut T` 引用还会获得 [`FnOnce`])
/// * [`fmt::Write`]
/// * [`Iterator`]
/// * [`DoubleEndedIterator`]
/// * [`ExactSizeIterator`]
/// * [`FusedIterator`]
/// * [`TrustedLen`]
/// * [`Send`]
/// * [`io::Write`]
/// * [`Read`]
/// * [`Seek`]
/// * [`BufRead`]
///
/// [`FusedIterator`]: iter::FusedIterator
/// [`TrustedLen`]: iter::TrustedLen
/// [`Seek`]: ../std/io/trait.Seek.html
/// [`BufRead`]: ../std/io/trait.BufRead.html
/// [`Read`]: ../std/io/trait.Read.html
/// [`io::Write`]: ../std/io/trait.Write.html
///
/// 此外，当且仅当 `T` 实现 [`Sync`] 时，`&T` 引用才实现 [`Send`]。
///
/// 注意，由于方法调用时的 deref coercion，直接调用 trait 方法时，看起来这些方法在引用上
/// 和在被拥有值上一样可用！这里描述的实现面向泛型上下文，在这些上下文中最终类型 `T`
/// 是类型参数，或者以其他方式无法在本地确定。
///
/// # 安全性(Safety）
///
/// 对所有类型 `T: ?Sized`，以及所有 `t: &T` 或 `t: &mut T`，当这些值跨越 API 边界时，
/// 通常必须维持以下不变式：
///
/// * `t` 非 null
/// * `t` 按 `align_of_val(t)` 对齐
/// * 如果 `size_of_val(t) > 0`，则 `t` 对 `size_of_val(t)` 个字节是可解引用的
///
/// 如果 `t` 指向地址 `a`，对 N 个字节“可解引用”表示内存范围 `[a, a + N)` 全部包含在
/// 单个[分配][allocation]中。
///
/// 例如，这意味着 safe 函数中的 unsafe code 可以假定调用者传入的参数满足这些不变式，
/// 也可以假定它所调用的任何 safe 函数的返回值满足这些不变式。
///
/// 反过来情况更复杂：当 unsafe code 向 safe 函数传递参数，或从 safe 函数返回值时，
/// 它们通常*至少*不能违反这些不变式。完整要求更强，因为引用通常必须指向可安全地作为
/// 类型 `T` 使用的数据。
///
/// unsafe code 是否可以在内部数据上暂时违反这些不变式，目前尚未决定。因此，
/// 在内部数据上暂时违反这些不变式的 unsafe code，可能已经不健全，或者会随着未来
/// Rust 版本对此问题的决定而变得不健全。
///
/// [allocation]: ptr#allocation
#[stable(feature = "rust1", since = "1.0.0")]
mod prim_ref {}

#[rustc_doc_primitive = "fn"]
//
/// 函数指针，例如 `fn(usize) -> bool`。
///
/// *另请参阅 [`Fn`]、[`FnMut`] 和 [`FnOnce`] trait。*
///
/// 函数指针是指向*代码*而非数据的指针。它们可以像函数一样被调用。与引用类似，
/// 函数指针也被假定为非 null 等；因此，如果想通过 FFI 传递函数指针且需要容纳 null 指针，
/// 请用所需签名构造 [`Option<fn()>`](core::option#options-and-pointers-nullable-pointers)
/// 类型。
///
/// 注意，FFI 需要额外小心，以确保调用双方的 ABI 匹配。确切要求目前尚未文档化。
///
/// ### 安全性(Safety）
///
/// 普通函数指针可通过转换普通函数，或不捕获环境的闭包来获得：
///
/// ```
/// fn add_one(x: usize) -> usize {
///     x + 1
/// }
///
/// let ptr: fn(usize) -> usize = add_one;
/// assert_eq!(ptr(5), 6);
///
/// let clos: fn(usize) -> usize = |x| x + 5;
/// assert_eq!(clos(5), 10);
/// ```
///
/// 除了会随签名变化外，函数指针还分为 safe 和 unsafe 两种。普通 `fn()` 函数指针
/// 只能指向 safe 函数，而 `unsafe fn()` 函数指针可以指向 safe 或 unsafe 函数。
///
/// ```
/// fn add_one(x: usize) -> usize {
///     x + 1
/// }
///
/// unsafe fn add_one_unsafely(x: usize) -> usize {
///     x + 1
/// }
///
/// let safe_ptr: fn(usize) -> usize = add_one;
///
/// //ERROR: 类型不匹配：期望 normal fn，却得到 unsafe fn
/// //let bad_ptr: fn(usize) -> usize = add_one_unsafely;
///
/// let unsafe_ptr: unsafe fn(usize) -> usize = add_one_unsafely;
/// let really_safe_ptr: unsafe fn(usize) -> usize = add_one;
/// ```
///
/// ### ABI
///
/// 除此之外，函数指针还可能因所用 ABI 而变化。这通过在类型前添加 `extern` 关键字，
/// 并在其后写出相关 ABI 来实现。默认 ABI 是 "Rust"，也就是说，`fn()` 与
/// `extern "Rust" fn()` 是完全相同的类型。指向 C ABI 函数的指针类型则是
/// `extern "C" fn()`。
///
/// `extern "ABI" { ... }` 块声明具有 "ABI" ABI 的函数。这里的默认值是 "C"，
/// 也就是说，在 `extern {...}` 块中声明的函数具有 "C" ABI。
///
/// 更多信息以及受支持 ABI 的列表，请参阅 [the nomicon 中关于外部调用约定的章节][nomicon-abi]。
///
/// [nomicon-abi]: ../nomicon/ffi.html#foreign-calling-conventions
///
/// ### 可变参数函数
///
/// 使用 "C" 或 "cdecl" ABI 的 extern 函数声明也可以是*可变参数*的，允许用可变数量的参数调用。
/// 普通 Rust 函数即使带有 `extern "ABI"`，也不能是可变参数函数。更多信息请参阅
/// [the nomicon 中关于可变参数函数的章节][nomicon-variadic]。
///
/// [nomicon-variadic]: ../nomicon/ffi.html#variadic-functions
///
/// ### 创建函数指针
///
/// 当 `bar` 是函数名时，表达式 `bar` *不是*函数指针。相反，它表示一个不可命名类型的值，
/// 该类型唯一标识函数 `bar`。由于类型已经标识了该函数，这个值是零大小的。
/// 这样做的优点是“调用”这个值（它实现了 `Fn*` trait）不需要动态分发。
///
/// 这个零大小类型会*强制转换*为常规函数指针。例如：
///
/// ```rust
/// fn bar(x: i32) {}
///
/// let not_bar_ptr = bar; // `not_bar_ptr` is zero-sized, uniquely identifying `bar`
/// assert_eq!(size_of_val(&not_bar_ptr), 0);
///
/// let bar_ptr: fn(i32) = not_bar_ptr; // 强制转换为函数指针
/// assert_eq!(size_of_val(&bar_ptr), size_of::<usize>());
///
/// let footgun = &bar; // 这是对标识 `bar` 的零大小类型的共享引用
/// ```
///
/// 最后一行表明 `&bar` 也不是函数指针。它其实是对函数特定 ZST 的引用。
/// 当 `bar` 是函数时，`&bar` 基本上永远不是你想要的东西。
///
/// ### 与整数互相转换
///
/// 可以将函数指针直接转换为整数：
///
/// ```rust
/// let fnptr: fn(i32) -> i32 = |x| x+2;
/// let fnptr_addr = fnptr as usize;
/// ```
///
/// 不过，不能直接转换回来。需要使用 `transmute`：
///
/// ```rust
/// # #[cfg(not(miri))] { // FIXME: use strict provenance APIs once they are stable, then remove this `cfg`
/// # let fnptr: fn(i32) -> i32 = |x| x+2;
/// # let fnptr_addr = fnptr as usize;
/// let fnptr = fnptr_addr as *const ();
/// let fnptr: fn(i32) -> i32 = unsafe { std::mem::transmute(fnptr) };
/// assert_eq!(fnptr(40), 42);
/// # }
/// ```
///
/// 关键在于，我们在 `transmute` 为函数指针之前，先用 `as` 转换为原始指针。
/// 这避免了从整数到指针的 `transmute`，后者可能有问题。
/// 在原始指针和函数指针之间（即两个指针类型之间）进行 transmute 是可以的。
///
/// 注意，如果某个平台上的函数指针和数据指针大小不同，上述做法就不具备可移植性。
///
/// ### ABI 兼容性
///
/// 一般来说，如果函数以某个签名声明，却通过具有不同签名的函数指针调用，
/// 这两个签名必须 *ABI 兼容*，否则通过该函数指针调用函数就是未定义行为。
/// ABI 兼容性远比仅仅具有相同内存布局更严格；例如，即使 `i32` 和 `f32` 具有相同的大小和对齐，
/// 它们也可能通过不同寄存器传递，因此并不 ABI 兼容。
///
/// 只有修改函数指针类型的代码，以及通过 `extern` 块导入函数的代码，才需要关注 ABI 兼容性。
/// 修改函数指针类型极其 unsafe（也就是说，甚至比 [`transmute_copy`][mem::transmute_copy]
/// 还要 unsafe 得多），应当只在最特殊的情况下发生。大多数 Rust 代码只是通过 `use`
/// 导入函数。因此，你很可能不必担心 ABI 兼容性。
///
/// 但假定确实处于这种情况，规则是什么？本节只考虑直接 Rust-to-Rust 调用的 ABI
/// （定义和调用点都对 Rust 编译器可见），而不是一般意义上的链接。一旦通过 `extern`
/// 块导入函数，就还有更多需要考虑的事项，这里不展开。注意，这也适用于通过函数指针
/// 跨语言边界传递/调用函数。
///
/// **本节中的任何内容都不应被视为对非 Rust-to-Rust 调用的保证，即使使用的是来自
/// `core::ffi` 或 `libc` 的类型**。
///
/// 要让两个签名被认为 *ABI 兼容*，它们必须使用兼容的 ABI 字符串，必须接受相同数量的参数，
/// 并且各个参数类型和返回类型也必须 ABI 兼容。ABI 字符串通过
/// `extern "ABI" fn(...) -> ...` 声明；注意，`fn name(...) -> ...` 隐式使用 `"Rust"`
/// ABI 字符串，而 `extern fn name(...) -> ...` 隐式使用 `"C"` ABI 字符串。
///
/// 如果 ABI 字符串相同，或者调用方 ABI 字符串是 `$X-unwind` 且被调用方 ABI 字符串是 `$X`，
/// 则保证它们兼容，其中 `$X` 是以下之一：
/// "C"、"aapcs"、"fastcall"、"stdcall"、"system"、"sysv64"、"thiscall"、"vectorcall"、"win64"。
///
/// 保证以下类型 ABI 兼容：
///
/// - 对所有 `T`，`*const T`、`*mut T`、`&T`、`&mut T`、`Box<T>`（具体而言，仅
///   `Box<T, Global>`）和 `NonNull<T>` 彼此 ABI 兼容。如果不同 `T` 具有相同的
///   元数据类型（`<T as Pointee>::Metadata`），它们之间也 ABI 兼容。
/// - `usize` 与相同大小的 `uN` 整数类型 ABI 兼容；同样，`isize` 与相同大小的
///   `iN` 整数类型 ABI 兼容。
/// - `char` 与 `u32` ABI 兼容。
/// - 任意两个 `fn`（函数指针）类型只要具有相同 ABI 字符串，或者 ABI 字符串仅在末尾
///   `-unwind` 上不同，就彼此 ABI 兼容，与签名的其余部分无关。（这意味着可以把
///   `fn()` 传给期望 `fn(i32)` 的函数，并且该调用在 ABI 意义上有效。被调用方收到的是
///   将函数指针从 `fn()` transmute 到 `fn(i32)` 的结果；该 transmute 本身是明确定义的操作，
///   只是之后调用该函数指针几乎必然是 UB。）
/// - 任意两个大小为 0 且对齐为 1 的类型 ABI 兼容。
/// - `repr(transparent)` 类型 `T` 与其唯一非平凡字段 ABI 兼容，也就是唯一一个
///   大小不是 0 且对齐不是 1 的字段（如果存在这样的字段）。
/// - `i32` 与 `NonZero<i32>` ABI 兼容，其他整数类型也类似。
/// - 如果保证 `T` 受[空指针优化](option/index.html#representation)约束，
///   且 `E` 是满足以下要求的 enum，则 `T` 和 `E` ABI 兼容。这样的 enum `E`
///   称为 "option-like"。
///   - enum `E` 使用 [`Rust` representation]，且未被 `align` 或 `packed` 表示修饰符修改。
///   - enum `E` 恰好有两个变体。
///   - 一个变体恰好有一个类型为 `T` 的字段。
///   - 另一个变体的所有字段都是零大小且 1 字节对齐。
///
/// 此外，ABI 兼容性满足以下一般性质：
///
/// - 每个类型都与自身 ABI 兼容。
/// - 如果 `T1` 与 `T2` ABI 兼容，且 `T2` 与 `T3` ABI 兼容，则 `T1` 与 `T3`
///   也 ABI 兼容（即 ABI 兼容性具有传递性）。
/// - 如果 `T1` 与 `T2` ABI 兼容，则 `T2` 与 `T1` 也 ABI 兼容
///   （即 ABI 兼容性具有对称性）。
///
/// 在特定目标上，更多签名可能 ABI 兼容，但不应依赖这一点，因为它不可移植，
/// 也不是稳定保证。
///
/// 一些值得注意的、通常*不* ABI 兼容的类型情形包括：
/// * `bool` vs `u8`、`i32` vs `u32`、`char` vs `i32`：在某些目标上，这些类型的调用约定
///   对寄存器中未被值使用的剩余位所提供的保证不同。
/// * `i32` vs `f32` 也不兼容，如前文所述。
/// * `struct Foo(u32)` 和 `u32` 不兼容（没有 `repr(transparent)` 时），因为 struct
///   是聚合类型，通常以不同于 `i32` 这类原语的方式传递。
///
/// 注意，这些规则描述的是两个完全已知的类型何时 ABI 兼容。在考虑另一个 crate
/// （包括标准库）中声明的类型的 ABI 兼容性时，需要注意，任何具有私有字段或
/// `#[non_exhaustive]` 属性的类型，除非另有文档说明，都可能在非破坏性更新中改变布局。
/// 因此，例如，即使这样的类型现在是 1-ZST 或 `repr(transparent)`，它也可能随着任意库版本
/// 升级而改变。
///
/// 如果声明签名与函数指针签名 ABI 兼容，则函数调用的行为就像每个参数都从函数指针中的类型
/// [`transmute`][mem::transmute] 到函数声明处的类型，且返回值从声明中的类型
/// [`transmute`][mem::transmute] 到指针中的类型。关于 transmute 转换的所有常规注意事项和顾虑
/// 都适用；例如，如果函数期望 `NonZero<i32>`，而函数指针使用 ABI 兼容的类型
/// `Option<NonZero<i32>>`，且用作参数的值是 `None`，那么此调用就是未定义行为，
/// 因为将 `None::<NonZero<i32>>` transmute 为 `NonZero<i32>` 违反了非零要求。
///
/// ### Trait 实现
///
/// 在本文档中，简写 `fn(T₁, T₂, …, Tₙ)` 用来表示不同长度的非可变参数函数指针。
/// 注意，这只是用于避免重复文档的便利记法，并不是有效的 Rust 语法。
///
/// 以下 trait 为具有任意参数数量和任意 ABI 的函数指针实现。
///
/// * [`PartialEq`]
/// * [`Eq`]
/// * [`PartialOrd`]
/// * [`Ord`]
/// * [`Hash`]
/// * [`Pointer`]
/// * [`Debug`]
/// * [`Clone`]
/// * [`Copy`]
/// * [`Send`]
/// * [`Sync`]
/// * [`Unpin`]
/// * [`UnwindSafe`]
/// * [`RefUnwindSafe`]
///
/// 注意，虽然此类型实现了 `PartialEq`，但比较函数指针并不可靠：指向同一函数的指针可能比较为不相等
/// （因为函数会在多个 codegen unit 中重复），指向*不同*函数的指针也可能比较为相等
/// （因为相同函数可能在一个 codegen unit 内被去重）。
///
/// [`Hash`]: hash::Hash
/// [`Pointer`]: fmt::Pointer
/// [`UnwindSafe`]: panic::UnwindSafe
/// [`RefUnwindSafe`]: panic::RefUnwindSafe
/// [`Rust` representation]: <https://doc.rust-lang.org/reference/type-layout.html#the-rust-representation>
///
/// 此外，所有 *safe* 函数指针都实现 [`Fn`]、[`FnMut`] 和 [`FnOnce`]，因为这些 trait
/// 是编译器特别知道的。
#[stable(feature = "rust1", since = "1.0.0")]
mod prim_fn {}

// 需要它来渲染 auto trait impl。
// 参见 src/librustdoc/passes/collect_trait_impls.rs:collect_trait_impls。
#[doc(hidden)]
impl<Ret, T> fn(T) -> Ret {}
