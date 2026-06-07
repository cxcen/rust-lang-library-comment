如果指针为空(null),返回 `None`;否则返回一个包裹在 `Some` 中、指向该值的共享
引用(shared reference)。如果该值可能未初始化(uninitialized),则必须改用
[`as_uninit_ref`]。

# Safety

调用此方法时,你必须确保:**要么**指针为空,**要么**该指针
[可转换为引用](crate::ptr#pointer-to-reference-conversion)(即:非空、已对齐、
指向一个已初始化的有效 `T`,并且在所选生命周期内遵守 Rust 的别名规则)。

# Panics during const evaluation

如果在 const 求值(const evaluation)期间无法确定指针是否为空,本方法将在该期间
panic。更多信息见 [`is_null`]。

# 空值未检查版本(Null-unchecked version)

如果你确信指针永远不可能为空,并且在寻找某种返回 `&T`(而非 `Option<&T>`)的
`as_ref_unchecked`,那么请知悉:你可以直接解引用该指针。
