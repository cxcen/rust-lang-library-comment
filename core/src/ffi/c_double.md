等价于 C 的 `double` 类型。

本类型几乎总是 [`f64`],而 [`f64`] 在 Rust 中保证为 [IEEE 754 double-precision float]。不过从 C 标准角度看,它只要求是精度至少不低于 [`float`] 的浮点数;在某些平台上它可能是 `f32`,也可能是完全不同于 IEEE-754 的表示。

[IEEE 754 double-precision float]: https://en.wikipedia.org/wiki/IEEE_754
[`float`]: c_float
