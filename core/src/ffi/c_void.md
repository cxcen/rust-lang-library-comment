在用作[指针]时等价于 C 的 `void` 类型。

本质上,`*const c_void` 等价于 C 的 `const void*`,`*mut c_void` 等价于 C 的 `void*`。
但它与 C 的 `void` 返回类型*不同*:后者对应 Rust 的 `()` 类型。

在 FFI 中表示指向不透明类型的指针时,在 `extern type` 稳定之前,推荐使用包裹空字节数组的
newtype。详见 [Nomicon]。

若需要支持低至 1.1.0 的旧 Rust 编译器,可以使用 `std::os::raw::c_void`。Rust 1.30.0
之后,它由本定义重新导出。更多信息见 [RFC 2521]。

[Nomicon]: https://doc.rust-lang.org/nomicon/ffi.html#representing-opaque-structs
[RFC 2521]: https://github.com/rust-lang/rfcs/blob/master/text/2521-c_void-reunification.md
