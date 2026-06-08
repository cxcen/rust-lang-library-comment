//! 本模块重新导出 primitive 类型，使其用法不会被其他已声明类型遮蔽。
//!
//! 这通常只对宏生成的代码有用。
//!
//! 一个例子是在生成新结构体及其 impl 时：
//!
//! ```rust,compile_fail
//! pub struct bool;
//!
//! impl QueryId for bool {
//!     const SOME_PROPERTY: bool = true;
//! }
//!
//! # trait QueryId { const SOME_PROPERTY: ::core::primitive::bool; }
//! ```
//!
//! 注意，`SOME_PROPERTY` 关联常量无法编译，因为它的类型 `bool` 指向该结构体，
//! 而不是 primitive bool 类型。
//!
//! 正确实现可以写成：
//!
//! ```rust
//! # #[allow(non_camel_case_types)]
//! pub struct bool;
//!
//! impl QueryId for bool {
//!     const SOME_PROPERTY: ::core::primitive::bool = true;
//! }
//!
//! # trait QueryId { const SOME_PROPERTY: ::core::primitive::bool; }
//! ```
//!
//! 这里还使用了 `::core` 而不是 `core`，因为 `core` 也可能被遮蔽。
//! 从 Edition 2018 开始，以 `::` 开头的路径会在 [extern prelude] 中搜索。
//!
//! [extern prelude]: https://doc.rust-lang.org/nightly/reference/names/preludes.html#extern-prelude

#[stable(feature = "core_primitive", since = "1.43.0")]
pub use bool;
#[stable(feature = "core_primitive", since = "1.43.0")]
pub use char;
#[stable(feature = "core_primitive", since = "1.43.0")]
pub use f32;
#[stable(feature = "core_primitive", since = "1.43.0")]
pub use f64;
#[stable(feature = "core_primitive", since = "1.43.0")]
pub use i8;
#[stable(feature = "core_primitive", since = "1.43.0")]
pub use i16;
#[stable(feature = "core_primitive", since = "1.43.0")]
pub use i32;
#[stable(feature = "core_primitive", since = "1.43.0")]
pub use i64;
#[stable(feature = "core_primitive", since = "1.43.0")]
pub use i128;
#[stable(feature = "core_primitive", since = "1.43.0")]
pub use isize;
#[stable(feature = "core_primitive", since = "1.43.0")]
pub use str;
#[stable(feature = "core_primitive", since = "1.43.0")]
pub use u8;
#[stable(feature = "core_primitive", since = "1.43.0")]
pub use u16;
#[stable(feature = "core_primitive", since = "1.43.0")]
pub use u32;
#[stable(feature = "core_primitive", since = "1.43.0")]
pub use u64;
#[stable(feature = "core_primitive", since = "1.43.0")]
pub use u128;
#[stable(feature = "core_primitive", since = "1.43.0")]
pub use usize;
