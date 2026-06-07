#![unstable(feature = "async_drop", issue = "126482")]

#[allow(unused_imports)]
use core::future::Future;

#[allow(unused_imports)]
use crate::pin::Pin;
#[allow(unused_imports)]
use crate::task::{Context, Poll};

/// Drop trait 的异步版本。
///
/// 当一个值不再被需要时,Rust 会对它运行“析构器”。最常见的“不再被需要”就是值离开
/// 作用域。析构器也可能在其它情形下运行,但这里的示例只聚焦于作用域这一种。要了解其它情形,请参阅
/// [Rust 参考手册] 中关于析构器的章节。
///
/// 与普通的 [`Drop`] 不同,异步析构是一个**可被 await、可被挂起**的过程:`drop` 方法本身是一个
/// future,需要被 poll 才能推进——这意味着清理动作可以等待 I/O 等异步事件,而不必阻塞线程。
///
/// [Rust 参考手册]: https://doc.rust-lang.org/reference/destructors.html
///
/// ## `Copy` 与 ([`Drop`]|`AsyncDrop`) 互斥
///
/// 你不能在同一个类型上同时实现 [`Copy`] 和 ([`Drop`]|`AsyncDrop`)。`Copy` 类型会被编译器隐式
/// 复制,这使得“析构器何时、被运行多少次”变得极难预测。因此,这类类型不允许拥有析构器。
#[unstable(feature = "async_drop", issue = "126482")]
#[lang = "async_drop"]
pub trait AsyncDrop {
    /// 执行本类型的异步析构器。
    ///
    /// 当值离开作用域时,本方法会被隐式调用,而不能被显式调用。
    ///
    /// 当本方法被调用时,`self` 尚未被释放;底层存储的释放只会发生在本方法结束之后。
    ///
    /// # Panics
    #[allow(async_fn_in_trait)]
    async fn drop(self: Pin<&mut Self>);
}

/// 异步地原地 drop 一个值。
///
/// 这是供编译器生成代码使用的 lang item。源码中的函数体只是占位;真实实现由编译器替换。
///
/// # 安全性(Safety）
///
/// `_to_drop` 必须指向一个有效的、已经初始化的 `T` 值,并且调用方必须拥有对该值执行 drop 的唯一
/// 权限。该指针必须满足 `T` 的对齐要求、在整个异步 drop 过程中保持有效,并且不能在本函数返回前
/// 被其它路径再次 drop 或移动。违反这些前置条件会破坏 Rust 的析构与别名规则,可能导致未定义行为。
#[unstable(feature = "async_drop", issue = "126482")]
#[lang = "async_drop_in_place"]
pub async unsafe fn async_drop_in_place<T: ?Sized>(_to_drop: *mut T) {
    // 这里的代码无关紧要——它会被编译器替换为真正的实现。
}
