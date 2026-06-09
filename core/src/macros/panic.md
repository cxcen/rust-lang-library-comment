使当前线程 panic。

这允许程序立即终止,并向程序调用者提供反馈。

这个宏非常适合在示例代码和测试中断言条件。`panic!` 与 [`Option`][ounwrap]
和 [`Result`][runwrap] 枚举的 `unwrap` 方法联系紧密:当值为 [`None`]
或 [`Err`] 变体时,这两个实现都会调用 `panic!`。

使用 `panic!()` 时,可以指定一个通过[格式化语法]构造的字符串 payload。
该 payload 会在把 panic 注入调用它的 Rust 线程时使用,从而让该线程整体 panic。

默认 `std` hook(也就是 panic 被触发后直接运行的代码)的行为,是把消息
payload 连同 `panic!()` 调用的文件/行/列信息打印到 `stderr`。可以使用
[`std::panic::set_hook()`] 覆盖 panic hook。在 hook 内部,可以把 panic
作为 `&dyn Any + Send` 访问;对于常规 `panic!()` 调用,其中包含 `&str`
或 `String`。(某次具体调用的 payload 类型是 `&str` 还是 `String` 并未指定,
并且可能变化。)如果要用另一种类型的值进行 panic,可以使用 [`panic_any`]。

另请参见用于在编译期间抛出错误的 [`compile_error!`] 宏。

# 何时使用 `panic!` 与 `Result`

Rust 语言提供了两套互补的系统,用于构造/表示、报告、传播、响应以及丢弃错误。
这些职责统称为“错误处理”。`panic!` 和 `Result` 的相似之处在于,它们各自都是
对应错误处理系统的主要接口;不过,这两个接口赋予错误的含义,以及它们在各自
错误处理系统中承担的职责并不相同。

`panic!` 宏用于构造表示程序中已检测到 bug 的错误。使用 `panic!` 时,
你提供一条描述该 bug 的消息,语言随后用该消息构造错误、报告错误并为你传播它。

另一方面,`Result` 用于包裹其他类型:它要么表示某次计算的成功结果 `Ok(T)`,
要么表示该计算预期可能遇到的运行时失败模式 `Err(E)`。`Result` 会与用户定义的
类型一起使用,这些类型表示相关计算可能遇到的各种预期运行时失败模式。
`Result` 必须手动传播,通常借助 `?` 运算符和 `Try` trait;也必须手动报告,
通常借助 `Error` trait。

有关错误处理的更多详细信息,请参阅 [book] 或 [`std::result`] 模块文档。

[ounwrap]: Option::unwrap
[runwrap]: Result::unwrap
[`std::panic::set_hook()`]: ../std/panic/fn.set_hook.html
[`panic_any`]: ../std/panic/fn.panic_any.html
[`Box`]: ../std/boxed/struct.Box.html
[`Any`]: crate::any::Any
[formatting syntax]: ../std/fmt/index.html
[book]: ../book/ch09-00-error-handling.html
[`std::result`]: ../std/result/index.html

# 当前实现

如果主线程 panic,它会终止所有线程,并让程序以退出码 `101` 结束。

# Edition

panic 宏的行为随 edition 发生过变化。

## 2021 及之后

在 Rust 2021 及之后,`panic!` 总是要求一个格式字符串和相应的格式参数,
并且在 `core` 与 `std` 中行为相同。若要用任意 payload 进行 panic,
请使用 [`std::panic::panic_any(x)`](../std/panic/fn.panic_any.html)。

## 2018 and 2015

在 2021 之前的 Rust Edition 中,带有单个参数的 `std::panic!(x)` 会直接把该参数
用作 payload。即使参数是字符串字面量也是如此。例如,
`panic!("problem: {reason}")` 会以字面值 `"problem: {reason}"`
(一个 `&'static str`)作为 payload 进行 panic。

带有单个参数的 `core::panic!(x)` 要求 `x` 是 `&str`,但除此之外行为类似
`std::panic!`。特别是,该字符串不必是字面量,也不会被解释为格式字符串。

# 示例

```should_panic
# #![allow(unreachable_code)]
panic!();
panic!("this is a terrible mistake!");
panic!("this is a {} {message}", "fancy", message = "message");
std::panic::panic_any(4); // 以值 4 进行 panic,供其他位置收集
```
