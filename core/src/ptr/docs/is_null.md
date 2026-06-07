如果指针为空(null),返回 `true`。

注意:非固定大小类型(unsized types)有许多种可能的空指针,因为这里只考虑裸的
数据指针(raw data pointer),而不考虑其长度、vtable 等元数据。因此,两个都为空的
指针之间仍然可能比较为不相等。

# Panics during const evaluation

如果在 const 求值(const evaluation)期间使用本方法,而 `self` 是一个被偏移到其
最初所指向内存边界之外的指针,那么可能没有足够的信息来判定该指针是否为空。这是
因为内存中的绝对地址在编译期是未知的。如果无法判定指针是否为空,本方法将 panic。

落在边界内(in-bounds)的指针永远不会为空,因此对于这类指针,本方法永远不会 panic。
