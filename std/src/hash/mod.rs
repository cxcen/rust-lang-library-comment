//! 通用的 hash 支持。
//!
//! 本模块提供了一种计算某个值的 [hash] 的通用方式。
//! hash 最常与 [`HashMap`] 和 [`HashSet`] 一起使用。
//!
//! [hash]: https://en.wikipedia.org/wiki/Hash_function
//! [`HashMap`]: ../../std/collections/struct.HashMap.html
//! [`HashSet`]: ../../std/collections/struct.HashSet.html
//!
//! 让一个类型可被 hash 的最简单方式是使用 `#[derive(Hash)]`：
//!
//! # 示例
//!
//! ```rust
//! use std::hash::{DefaultHasher, Hash, Hasher};
//!
//! #[derive(Hash)]
//! struct Person {
//!     id: u32,
//!     name: String,
//!     phone: u64,
//! }
//!
//! let person1 = Person {
//!     id: 5,
//!     name: "Janet".to_string(),
//!     phone: 555_666_7777,
//! };
//! let person2 = Person {
//!     id: 5,
//!     name: "Bob".to_string(),
//!     phone: 555_666_7777,
//! };
//!
//! assert!(calculate_hash(&person1) != calculate_hash(&person2));
//!
//! fn calculate_hash<T: Hash>(t: &T) -> u64 {
//!     let mut s = DefaultHasher::new();
//!     t.hash(&mut s);
//!     s.finish()
//! }
//! ```
//!
//! 如果你需要对一个值如何被 hash 有更多的控制，那就需要实现 [`Hash`] trait：
//!
//! ```rust
//! use std::hash::{DefaultHasher, Hash, Hasher};
//!
//! struct Person {
//!     id: u32,
//!     # #[allow(dead_code)]
//!     name: String,
//!     phone: u64,
//! }
//!
//! impl Hash for Person {
//!     fn hash<H: Hasher>(&self, state: &mut H) {
//!         self.id.hash(state);
//!         self.phone.hash(state);
//!     }
//! }
//!
//! let person1 = Person {
//!     id: 5,
//!     name: "Janet".to_string(),
//!     phone: 555_666_7777,
//! };
//! let person2 = Person {
//!     id: 5,
//!     name: "Bob".to_string(),
//!     phone: 555_666_7777,
//! };
//!
//! assert_eq!(calculate_hash(&person1), calculate_hash(&person2));
//!
//! fn calculate_hash<T: Hash>(t: &T) -> u64 {
//!     let mut s = DefaultHasher::new();
//!     t.hash(&mut s);
//!     s.finish()
//! }
//! ```
#![stable(feature = "rust1", since = "1.0.0")]

pub(crate) mod random;

#[stable(feature = "rust1", since = "1.0.0")]
pub use core::hash::*;

#[stable(feature = "std_hash_exports", since = "1.76.0")]
pub use self::random::{DefaultHasher, RandomState};
