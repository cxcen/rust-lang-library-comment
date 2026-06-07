为指针加上一个无符号偏移量(unsigned offset)。

它只能让指针向前移动(或不移动)。如果你需要根据数值来决定向前或向后移动,那么
你也许想要 [`offset`](#method.offset),它接受一个有符号偏移量(signed offset)。

`count` 以 T 为单位;例如 `count` 为 3 表示一个 `3 * size_of::<T>()` 字节的指针
偏移。

# Safety

如果违反以下任一条件,结果即为未定义行为(Undefined Behavior):

* 以字节计的偏移量 `count * size_of::<T>()`,在数学整数意义上计算(不发生"回绕
wrapping around"),必须能够容纳进一个 `isize`。

* 如果计算出的偏移量非零,那么 `self` 必须[派生自][crate::ptr#provenance]一个指向
某个[分配对象(allocation)][allocation]的指针,并且 `self` 与结果指针之间的整个
内存范围都必须落在该分配对象的边界内。特别地,该范围不得"回绕"地址空间的边缘。

分配对象的大小永远不会超过 `isize::MAX` 字节,因此只要计算出的偏移量保持在分配
对象的边界内,就保证满足上述第一项要求。举例而言,这意味着
`vec.as_ptr().add(vec.len())`(对于 `vec: Vec<T>`)永远是安全的。

如果这些约束难以满足,可考虑改用 [`wrapping_add`]。本方法相较之下的唯一优势是它能
启用更激进的编译器优化。

[`wrapping_add`]: #method.wrapping_add
[allocation]: crate::ptr#allocation
