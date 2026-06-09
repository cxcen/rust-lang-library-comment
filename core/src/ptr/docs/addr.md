获取指针的"地址"(address)部分。

这类似于 `self as usize`,区别在于该指针的 [provenance(来源/可证溯性)][crate::ptr#provenance]
会被丢弃,且不会被[暴露(exposed)][crate::ptr#exposed-provenance]。这意味着:把
返回的地址再转换回指针,得到的是一个[没有 provenance 的指针][without_provenance],
解引用它属于未定义行为。要正确地恢复丢失的信息并获得一个可解引用的指针,请使用
[`with_addr`][pointer::with_addr] 或 [`map_addr`][pointer::map_addr]。

如果由于无法保留一个带有所需 provenance 的指针,导致那些 API 用不了,那么严格
来源(Strict Provenance)模型也许并不适合你。此时请改用指针-整数转换(pointer-integer
casts),或者 [`expose_provenance`][pointer::expose_provenance] 与
[`with_exposed_provenance`][with_exposed_provenance]。不过请注意,这样做会让你的
代码更不可移植,也更难被那些检查是否符合 Rust 内存模型的工具所分析。

在大多数平台上,这会产生一个与原指针字节完全相同的值,因为指针的所有字节都用于
描述地址。对于那些需要在指针中存储额外信息的平台,则可能进行一次表示形式的转换,
以产生一个只包含指针地址部分的值。这具体意味着什么,由各平台自行定义。

这是一个[严格来源(Strict Provenance)][crate::ptr#strict-provenance] API。
