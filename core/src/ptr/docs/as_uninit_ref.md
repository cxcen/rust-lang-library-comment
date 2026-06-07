如果指针为空(null),返回 `None`;否则返回一个包裹在 `Some` 中、指向该值的共享
引用(shared reference)。与 [`as_ref`] 不同,本方法**不**要求该值必须已初始化。

# 安全性(Safety）

调用此方法时,你必须确保:**要么**指针为空,**要么**该指针
[可转换为引用](crate::ptr#pointer-to-reference-conversion)。注意,由于所创建的
引用指向的是 `MaybeUninit<T>`,源指针可以指向未初始化(uninitialized)的内存。

# Panics

如果在 const 求值(const evaluation)期间无法确定指针是否为空,本方法将在该期间
panic。更多信息见 [`is_null`]。
