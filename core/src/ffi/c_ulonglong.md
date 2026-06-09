等价于 C 的 `unsigned long long` 类型。

本类型几乎总是 [`u64`],但在某些系统上可能不同。C 标准技术上只要求它是与 [`long long`] 大小相同的无符号整数;实践中几乎没有系统会让 `long long` 不是 `u64`,因为大多数系统没有标准化的 [`u128`] 类型。

[`long long`]: c_longlong
