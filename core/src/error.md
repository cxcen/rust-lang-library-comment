用于处理错误的接口。

# Rust 中的错误处理

Rust 语言提供两个互补系统，用于构造/表示、报告、传播、响应和丢弃错误。
这些职责统称为“错误处理”。第一套系统包括 panic 运行时和接口，最常用于表示
程序中检测到的 bug。第二套系统包括 `Result`、错误 trait 和用户定义类型，
用于表示程序中预期的运行时失败模式。

## panic 接口

以下是 panic 系统的主要接口及其承担的职责：

* [`panic!`] 和 [`panic_any`]（构造，自动传播）
* [`set_hook`]、[`take_hook`] 和 [`PanicHookInfo`]（报告）
* [`#[panic_handler]`][panic-handler] 和 [`PanicInfo`]（在 no_std 中报告）
* [`catch_unwind`] 和 [`resume_unwind`]（丢弃，传播）

以下是错误系统的主要接口及其承担的职责：

* [`Result`]（传播，响应）
* [`Error`] trait（报告）
* 用户定义类型（构造/表示）
* [`match`] 和 [`downcast`]（响应）
* 问号运算符（[`?`]）（传播）
* 部分稳定的 [`Try`] trait（传播，构造）
* [`Termination`]（报告）

## 将错误转换为 panic

panic 系统和错误系统并非完全分离。很多时候，API 中预期的运行时失败错误，
对调用方来说反而可能代表 bug。针对这类情况，标准库提供了 API，可以构造
以某个 `Error` 作为来源的 panic。

* [`Result::unwrap`]
* [`Result::expect`]

这两个函数等价：如果 `Result` 为 `Ok`，它们返回内部值；如果 `Result` 为
`Err`，它们会 panic，并把内部错误作为来源打印出来。二者唯一的区别是：
使用 `expect` 时，你会提供一条 panic 错误消息，与来源一起打印；
而 `unwrap` 使用默认消息，只说明你解包了一个 `Err`。

在二者之中，通常更推荐 `expect`，因为它的 `msg` 字段允许你表达意图和假设，
从而更容易追踪 panic 的来源。另一方面，在你可以轻易证明某段代码永远不会
panic 的场景中，`unwrap` 仍然可能很合适，例如
`"127.0.0.1".parse::<std::net::IpAddr>().unwrap()`，也适合早期原型开发。

# 常见消息风格

人们编写 `expect` 消息时常见两种风格：把消息作为面向遭遇 panic 的用户的
信息（“把 expect 当作错误消息”），或者把消息作为面向调试 panic 的开发者的
信息（“把 expect 当作前提条件”）。

在前一种情况下，expect 消息用于描述已经发生、且被视为 bug 的错误。考虑下面的例子：

```should_panic
// Read environment variable, panic if it is not present
let path = std::env::var("IMPORTANT_PATH").unwrap();
```

在“把 expect 当作错误消息”风格中，我们会使用 expect 来描述环境变量本应设置却未设置：

```should_panic
let path = std::env::var("IMPORTANT_PATH")
    .expect("env variable `IMPORTANT_PATH` is not set");
```

在“把 expect 当作前提条件”风格中，我们转而描述自己_预期_ `Result` 应为 `Ok`
的理由。采用这种风格时，我们更倾向于写成：

```should_panic
let path = std::env::var("IMPORTANT_PATH")
    .expect("env variable `IMPORTANT_PATH` should be set by `wrapper_script.sh`");
```

“把 expect 当作错误消息”风格与 std panic hook 的默认输出配合得不太好，
通常会重复被解包来源错误已经传达过的信息：

```text
thread 'main' panicked at src/main.rs:4:6:
env variable `IMPORTANT_PATH` is not set: NotPresent
```

在这个例子中，我们先说某个环境变量没有设置，随后来源消息又说该环境不存在；
我们额外传达的唯一信息只是正在检查的环境变量名称。

“把 expect 当作前提条件”风格则关注源代码可读性。在 panic 专门用来表示 bug
的场景中，它更容易让人理解究竟什么前提出了问题。另外，通过用“本应”发生什么
来避免来源错误的方式表述 expect，我们引入了独立于来源错误的新信息。

```text
thread 'main' panicked at src/main.rs:4:6:
env variable `IMPORTANT_PATH` should be set by `wrapper_script.sh`: NotPresent
```

在这个例子中，我们不仅传达了本应设置的环境变量名称，还解释了它为什么应该被设置，
并让来源错误清楚地显示出它与我们预期之间的矛盾。

**提示**：如果你很难记住如何措辞“把 expect 当作前提条件”风格的错误消息，
请记得聚焦于 `should` 这个词，例如“env variable should be set by blah”或
“the given binary should be available and executable by the current user”。

[`panic_any`]: ../../std/panic/fn.panic_any.html
[`PanicHookInfo`]: ../../std/panic/struct.PanicHookInfo.html
[`PanicInfo`]: crate::panic::PanicInfo
[`catch_unwind`]: ../../std/panic/fn.catch_unwind.html
[`resume_unwind`]: ../../std/panic/fn.resume_unwind.html
[`downcast`]: crate::error::Error
[`Termination`]: ../../std/process/trait.Termination.html
[`Try`]: crate::ops::Try
[panic hook]: ../../std/panic/fn.set_hook.html
[`set_hook`]: ../../std/panic/fn.set_hook.html
[`take_hook`]: ../../std/panic/fn.take_hook.html
[panic-handler]: <https://doc.rust-lang.org/nomicon/panic-handler.html>
[`match`]: ../../std/keyword.match.html
[`?`]: ../../std/result/index.html#the-question-mark-operator-
