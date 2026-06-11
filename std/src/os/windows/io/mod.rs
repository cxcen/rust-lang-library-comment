//! Windows 平台对通用 I/O 原语的特定扩展。
//!
//! 就像裸指针一样，裸的 Windows handle 与 socket 指向具有动态生命周期的资源，
//! 一旦它们的存活时间超过了所指向的资源，就会变成悬垂值；若由非法值构造出来，
//! 则可能是伪造的。
//!
//! 本模块提供三种类型来表示裸 handle 与 socket，分别具有不同的所有权语义：
//! 裸（raw）、借用（borrowed）、拥有（owned），它们与用于表示指针的类型一一对应。
//! 这些类型体现了 Windows 上的 [I/O 安全][io-safety] 概念。
//!
//! | 类型                   | 类比于       |
//! | ---------------------- | ------------ |
//! | [`RawHandle`]          | `*const _`   |
//! | [`RawSocket`]          | `*const _`   |
//! |                        |              |
//! | [`BorrowedHandle<'a>`] | `&'a _`      |
//! | [`BorrowedSocket<'a>`] | `&'a _`      |
//! |                        |              |
//! | [`OwnedHandle`]        | `Box<_>`     |
//! | [`OwnedSocket`]        | `Box<_>`     |
//!
//! 像裸指针一样，`RawHandle` 与 `RawSocket` 的值是原始（primitive）值。
//! 在新代码中，对它们执行 I/O 应当被视为 unsafe（类比于对裸指针解引用）。
//! Rust 早期并没有给出这一指导，因此 Rust 生态中现有的代码常常没有把对
//! `RawHandle` 与 `RawSocket` 的使用标记为 unsafe。
//! 鼓励各个库进行迁移：要么给那些会“解引用” `RawHandle` 和 `RawSocket` 值的
//! API 加上 `unsafe`，要么改用 `BorrowedHandle`、`BorrowedSocket`、`OwnedHandle`
//! 或 `OwnedSocket`。
//!
//! 像引用一样，`BorrowedHandle` 与 `BorrowedSocket` 的值与某个生命周期绑定，
//! 以确保它们不会比所指向的资源活得更久。它们用起来是安全的。
//! `BorrowedHandle` 与 `BorrowedSocket` 的值可用于那些对任意系统调用提供安全访问的
//! API，但 `CloseHandle`、`closesocket` 以及任何会在不结束 handle/socket 自身生命周期的
//! 前提下结束资源动态生命周期的调用除外。
//!
//! `BorrowedHandle` 与 `BorrowedSocket` 的值可以用于那些对 `DuplicateHandle`、
//! `WSADuplicateSocketW` 及相关函数提供安全访问的 API，因此实现了 `AsHandle`、
//! `AsSocket`、`From<OwnedHandle>` 或 `From<OwnedSocket>` 的类型不应假定自己始终
//! 对底层对象拥有独占访问权。
//!
//! 像 box 一样，`OwnedHandle` 与 `OwnedSocket` 的值在概念上拥有它们所指向的资源，
//! 并在被 drop 时释放（关闭）该资源。
//!
//! 关于 I/O 安全的总体说明，参见 [`io` 模块文档][io-safety]。
//!
//! [`BorrowedHandle<'a>`]: crate::os::windows::io::BorrowedHandle
//! [`BorrowedSocket<'a>`]: crate::os::windows::io::BorrowedSocket
//! [io-safety]: crate::io#io-safety

#![stable(feature = "rust1", since = "1.0.0")]

mod handle;
mod raw;
mod socket;

#[stable(feature = "io_safety", since = "1.63.0")]
pub use handle::*;
#[stable(feature = "rust1", since = "1.0.0")]
pub use raw::*;
#[stable(feature = "io_safety", since = "1.63.0")]
pub use socket::*;

// 测试模块，仅在测试构建时编译。
#[cfg(test)]
mod tests;
