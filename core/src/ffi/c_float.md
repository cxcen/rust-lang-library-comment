等价于 C 的 `float` 类型。

本类型几乎总是 [`f32`],而 [`f32`] 在 Rust 中保证为 [IEEE 754 single-precision float]。不过 C 标准技术上只要求它是浮点数;它可能比 `f32` 精度更低,也可能完全不遵循 IEEE-754 标准。

[IEEE 754 single-precision float]: https://en.wikipedia.org/wiki/IEEE_754
