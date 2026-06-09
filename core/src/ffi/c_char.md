等价于 C 的 `char` 类型。

[C 的 `char` 类型] 与 [Rust 的 `char` 类型] 完全不同:Rust 的 `char` 表示 Unicode 标量值,而 C 的 `char` 只是普通整数。在使用 8-bit 字节、按字节寻址的现代架构上,本类型始终是 [`i8`] 或 [`u8`]。

C char 最常见的用途是组成 C 字符串。Rust 字符串会携带长度;C 字符串则用字符 `'\0'` 标记结尾。更多信息见 `CStr`。

[C 的 `char` 类型]: https://en.wikipedia.org/wiki/C_data_types#Basic_types
[Rust 的 `char` 类型]: char
