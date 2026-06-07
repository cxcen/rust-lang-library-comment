如果指针为空(null),返回 `None`;否则返回一个包裹在 `Some` 中、指向该值的共享
切片(shared slice)。与 [`as_ref`] 不同,本方法**不**要求该值必须已初始化。

[`as_ref`]: #method.as_ref

# Safety

调用此方法时,你必须确保:**要么**指针为空,**要么**以下条件全部成立:

* 该指针对于 `ptr.len() * size_of::<T>()` 个字节而言必须对读取(reads)
  [有效(valid)][valid],并且必须正确对齐(properly aligned)。这尤其意味着:

* 该切片的整个内存范围必须包含在**单个**[分配对象(allocation)][allocation]之内!
  切片绝不能跨越多个分配对象。

* 即便是零长度(zero-length)切片,指针也必须对齐。原因之一是:枚举布局优化可能
  依赖于引用(包括任意长度的切片)是对齐且非空的,以此把它们与其他数据区分开。
  你可以用 [`NonNull::dangling()`] 获得一个可用作零长度切片 `data` 的指针。

* 切片的总大小 `ptr.len() * size_of::<T>()` 必须不大于 `isize::MAX`。
  参见 [`pointer::offset`] 的安全性文档。

* 你必须自行维护 Rust 的别名规则(aliasing rules),因为返回的生命周期 `'a` 是
  任意选定的,不一定反映数据的实际生命周期。特别地,在此引用存在期间,指针所指向
  的内存不得被修改(在 `UnsafeCell` 内部的除外)。

即便本方法的结果未被使用,上述要求依然适用!

另见 [`slice::from_raw_parts`][]。

[valid]: crate::ptr#safety
[allocation]: crate::ptr#allocation

# Panics during const evaluation

如果在 const 求值(const evaluation)期间无法确定指针是否为空,本方法将在该期间
panic。更多信息见 [`is_null`]。

[`is_null`]: #method.is_null
