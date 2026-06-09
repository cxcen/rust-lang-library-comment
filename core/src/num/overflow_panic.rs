//! 整数溢出时触发 panic 的集中入口。
//!
//! 这些函数主要供整数的 `strict_` 方法调用。`checked`/`wrapping`/`saturating`/
//! `overflowing` 各 API 家族都会把溢出显式编码到返回值或结果语义里，而
//! `strict_` 系列选择把溢出视为逻辑错误并立即 panic。把入口收束在这里可以让
//! panic 文本保持一致，也便于编译器把这些冷路径和正常算术热路径分开处理。

#[cold]
#[track_caller]
pub(super) const fn add() -> ! {
    panic!("attempt to add with overflow")
}

#[cold]
#[track_caller]
pub(super) const fn sub() -> ! {
    panic!("attempt to subtract with overflow")
}

#[cold]
#[track_caller]
pub(super) const fn mul() -> ! {
    panic!("attempt to multiply with overflow")
}

#[cold]
#[track_caller]
pub(super) const fn div() -> ! {
    panic!("attempt to divide with overflow")
}

#[cold]
#[track_caller]
pub(super) const fn rem() -> ! {
    panic!("attempt to calculate the remainder with overflow")
}

#[cold]
#[track_caller]
pub(super) const fn neg() -> ! {
    panic!("attempt to negate with overflow")
}

#[cold]
#[track_caller]
pub(super) const fn shr() -> ! {
    panic!("attempt to shift right with overflow")
}

#[cold]
#[track_caller]
pub(super) const fn shl() -> ! {
    panic!("attempt to shift left with overflow")
}
