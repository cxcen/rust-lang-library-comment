//! # Rust 预导入（The Rust Prelude）
//!
//! Rust 的标准库自带各种各样的东西。然而，如果你不得不手动导入用到的每一样东西，
//! 那会非常啰嗦；但反过来，导入一大堆程序从不使用的东西也不好。需要在两者间取得平衡。
//!
//! *预导入（prelude）*是 Rust 自动导入到每个 Rust 程序中的东西的清单。它被保持得
//! 尽可能小，并聚焦于那些几乎在每个 Rust 程序中都会用到的东西，尤其是 trait。
//!
//! 设计背景：预导入项之所以“默认可用”，是因为编译器会用 `#[prelude_import]` 把对应
//! edition 的预导入隐式地 glob 导入到每个模块。本模块（`std` 的预导入）在 `core` 预导入
//! 的基础上叠加了来自 `alloc`/`std` 的少量项（如 `Box`、`String`、`Vec`、`ToOwned`），
//! 因此它维护着这样一个不变量：清单必须保持精简，且必须按 edition 分层、向后兼容。
//!
//! # 其它预导入（Other preludes）
//!
//! 预导入可以被看作一种使“同时使用多个类型”更便利的模式。因此，你会在标准库中找到
//! 其它预导入，例如 [`std::io::prelude`]。Rust 生态系统中的各种库也可能定义它们自己的
//! 预导入。
//!
//! [`std::io::prelude`]: crate::io::prelude
//!
//! “这个预导入”（即 the prelude）与那些其它预导入的区别在于：后者不会被自动 `use`，
//! 必须手动导入。不过即便如此，这仍然比逐一导入它们的全部组成项要轻松。
//!
//! # 预导入内容（Prelude contents）
//!
//! 预导入中包含的项取决于 crate 所用的 edition。
//! 预导入的第一个版本用于 Rust 2015 和 Rust 2018，
//! 它存放在 [`std::prelude::v1`] 中。
//! [`std::prelude::rust_2015`] 和 [`std::prelude::rust_2018`] 会重导出这个预导入。
//! 它重导出以下内容：
//!
//! * <code>[std::marker]::{[Copy], [Send], [Sized], [Sync], [Unpin]}</code>，
//!   表示类型基本性质的标记 trait（marker traits）。
//! * <code>[std::ops]::{[Fn], [FnMut], [FnOnce]}</code>，以及它们对应的
//!   异步 trait：<code>[std::ops]::{[AsyncFn], [AsyncFnMut], [AsyncFnOnce]}</code>。
//! * <code>[std::ops]::[Drop]</code>，用于实现析构器（destructors）。
//! * <code>[std::mem]::[drop]</code>，一个用于显式 drop 某个值的便捷函数。
//! * <code>[std::mem]::{[size_of], [size_of_val]}</code>，用于获取某个类型或值的大小。
//! * <code>[std::mem]::{[align_of], [align_of_val]}</code>，用于获取某个类型或值的
//!   对齐（alignment）。
//! * <code>[std::boxed]::[Box]</code>，一种在堆上分配值的方式。
//! * <code>[std::borrow]::[ToOwned]</code>，定义 [`to_owned`] 的转换 trait——后者是
//!   从借用类型创建拥有所有权类型的通用方法。
//! * <code>[std::clone]::[Clone]</code>，无处不在的 trait，它定义了
//!   [`clone`][Clone::clone]，即产生一个值副本的方法。
//! * <code>[std::cmp]::{[PartialEq], [PartialOrd], [Eq], [Ord]}</code>，比较
//!   trait，它们实现比较运算符，并常见于 trait 约束（trait bounds）中。
//! * <code>[std::convert]::{[AsRef], [AsMut], [Into], [From]}</code>，通用转换，
//!   被精明的 API 作者用来创建“重载式”方法。
//! * <code>[std::default]::[Default]</code>，用于拥有默认值的类型。
//! * <code>[std::iter]::{[Iterator], [Extend], [IntoIterator], [DoubleEndedIterator],
//!   [ExactSizeIterator]}</code>，各种各样的迭代器。
//! * 大多数标准宏。
//! * <code>[std::option]::[Option]::{[self][Option], [Some], [None]}</code>，一个
//!   表达“值存在或不存在”的类型。该类型用得如此频繁，以至于它的各个变体也被导出。
//! * <code>[std::result]::[Result]::{[self][Result], [Ok], [Err]}</code>，一个供
//!   “可能成功或失败”的函数使用的类型。与 [`Option`] 一样，它的各个变体也被导出。
//! * <code>[std::string]::{[String], [ToString]}</code>，堆分配的字符串。
//! * <code>[std::vec]::[Vec]</code>，一个可增长的、堆分配的向量。
//!
//! Rust 2021 所用的预导入 [`std::prelude::rust_2021`] 包含上述全部内容，
//! 并额外重导出：
//!
//! * <code>[std::convert]::{[TryFrom], [TryInto]}</code>。
//! * <code>[std::iter]::[FromIterator]</code>。
//!
//! Rust 2024 所用的预导入 [`std::prelude::rust_2024`] 包含上述全部内容，
//! 并额外重导出：
//!
//! * <code>[std::future]::{[Future], [IntoFuture]}</code>。
//!
//! [std::borrow]: crate::borrow
//! [std::boxed]: crate::boxed
//! [std::clone]: crate::clone
//! [std::cmp]: crate::cmp
//! [std::convert]: crate::convert
//! [std::default]: crate::default
//! [std::future]: crate::future
//! [std::iter]: crate::iter
//! [std::marker]: crate::marker
//! [std::mem]: crate::mem
//! [std::ops]: crate::ops
//! [std::option]: crate::option
//! [`std::prelude::v1`]: v1
//! [`std::prelude::rust_2015`]: rust_2015
//! [`std::prelude::rust_2018`]: rust_2018
//! [`std::prelude::rust_2021`]: rust_2021
//! [`std::prelude::rust_2024`]: rust_2024
//! [std::result]: crate::result
//! [std::slice]: crate::slice
//! [std::string]: crate::string
//! [std::vec]: mod@crate::vec
//! [`to_owned`]: crate::borrow::ToOwned::to_owned
//! [book-closures]: ../../book/ch13-01-closures.html
//! [book-dtor]: ../../book/ch15-03-drop.html
//! [book-enums]: ../../book/ch06-01-defining-an-enum.html
//! [book-iter]: ../../book/ch13-02-iterators.html
//! [Future]: crate::future::Future
//! [IntoFuture]: crate::future::IntoFuture

// 不做格式化：本文件除了重导出别无他物，而其顺序值得保留。
#![cfg_attr(rustfmt, rustfmt::skip)]

#![stable(feature = "rust1", since = "1.0.0")]

pub mod v1;

/// Rust 标准库预导入的 2015 版本。
///
/// 更多内容参见[模块级文档](self)。
#[stable(feature = "prelude_2015", since = "1.55.0")]
pub mod rust_2015 {
    #[stable(feature = "prelude_2015", since = "1.55.0")]
    #[doc(no_inline)]
    pub use super::v1::*;
}

/// Rust 标准库预导入的 2018 版本。
///
/// 更多内容参见[模块级文档](self)。
#[stable(feature = "prelude_2018", since = "1.55.0")]
pub mod rust_2018 {
    #[stable(feature = "prelude_2018", since = "1.55.0")]
    #[doc(no_inline)]
    pub use super::v1::*;
}

/// Rust 标准库预导入的 2021 版本。
///
/// 更多内容参见[模块级文档](self)。
#[stable(feature = "prelude_2021", since = "1.55.0")]
pub mod rust_2021 {
    #[stable(feature = "prelude_2021", since = "1.55.0")]
    #[doc(no_inline)]
    pub use super::v1::*;

    #[stable(feature = "prelude_2021", since = "1.55.0")]
    #[doc(no_inline)]
    pub use core::prelude::rust_2021::*;

    // 存在两个不同的 panic 宏，一个在 `core` 中，一个在 `std` 中。它们略有不同。
    // 对于 `std`，我们显式地想要 `std` 中定义的那一个。
    #[stable(feature = "prelude_2021", since = "1.55.0")]
    pub use super::v1::panic;
}

/// Rust 标准库预导入的 2024 版本。
///
/// 更多内容参见[模块级文档](self)。
#[stable(feature = "prelude_2024", since = "1.85.0")]
pub mod rust_2024 {
    #[stable(feature = "rust1", since = "1.0.0")]
    #[doc(no_inline)]
    pub use super::v1::*;

    #[stable(feature = "prelude_2024", since = "1.85.0")]
    #[doc(no_inline)]
    pub use core::prelude::rust_2024::*;

    // 存在两个不同的 panic 宏，一个在 `core` 中，一个在 `std` 中。它们略有不同。
    // 对于 `std`，我们显式地想要 `std` 中定义的那一个。
    #[stable(feature = "prelude_2024", since = "1.85.0")]
    pub use super::v1::panic;
}

/// Rust 标准库预导入的 Future（未来）版本。
///
/// 更多内容参见[模块级文档](self)。
#[doc(hidden)]
#[unstable(feature = "prelude_future", issue = "none")]
pub mod rust_future {
    #[stable(feature = "rust1", since = "1.0.0")]
    #[doc(no_inline)]
    pub use super::v1::*;

    #[unstable(feature = "prelude_next", issue = "none")]
    #[doc(no_inline)]
    pub use core::prelude::rust_future::*;

    // 存在两个不同的 panic 宏，一个在 `core` 中，一个在 `std` 中。它们略有不同。
    // 对于 `std`，我们显式地想要 `std` 中定义的那一个。
    #[unstable(feature = "prelude_next", issue = "none")]
    pub use super::v1::panic;
}
