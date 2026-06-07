//! 一组用于约束 intrinsic 入参类型的辅助 trait，防止把完全不合理的类型传给编译器内建操作。
//!
//! 背景：intrinsic 是编译器内建操作，是 core/std 与编译器（rustc/LLVM）之间的契约层。
//! 有些 intrinsic（比如指针偏移、解引用类操作）只对“指针类”类型有意义。
//! 但 intrinsic 的签名往往写得很宽松（用泛型参数），无法在签名层面表达“必须是引用或裸指针”。
//! 这里用这些 trait 作为泛型约束，把“类型至少不是完全错误的”这一限制前移到类型系统，
//! 让误用在编译期而非运行期才暴露。

use crate::marker::PointeeSized;

/// 在运行期 MIR 中具有内建解引用操作的类型，即引用与裸指针（`&T`/`&mut T`/`*const T`/`*mut T`）。
///
/// # 安全性（Safety）
/// 该类型必须确实*是*上述指针类类型之一。实现者要为这个承诺负责：
/// 编译器会假定凡是实现了本 trait 的类型都能按指针语义解引用，
/// 若给一个并非指针的类型 unsafe 地实现本 trait，后续基于指针语义的优化会导致 UB。
pub unsafe trait BuiltinDeref: Sized {
    type Pointee: PointeeSized;
}

unsafe impl<T: PointeeSized> BuiltinDeref for &mut T {
    type Pointee = T;
}
unsafe impl<T: PointeeSized> BuiltinDeref for &T {
    type Pointee = T;
}
unsafe impl<T: PointeeSized> BuiltinDeref for *mut T {
    type Pointee = T;
}
unsafe impl<T: PointeeSized> BuiltinDeref for *const T {
    type Pointee = T;
}

pub trait ChangePointee<U: PointeeSized>: BuiltinDeref {
    type Output;
}
impl<'a, T: PointeeSized + 'a, U: PointeeSized + 'a> ChangePointee<U> for &'a mut T {
    type Output = &'a mut U;
}
impl<'a, T: PointeeSized + 'a, U: PointeeSized + 'a> ChangePointee<U> for &'a T {
    type Output = &'a U;
}
impl<T: PointeeSized, U: PointeeSized> ChangePointee<U> for *mut T {
    type Output = *mut U;
}
impl<T: PointeeSized, U: PointeeSized> ChangePointee<U> for *const T {
    type Output = *const U;
}
