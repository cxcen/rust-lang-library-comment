//! 供 liballoc 和 libstd 使用的 Unicode 内部表与查询函数，不是公开 API。
//!
//! 这里集中保存 `char` 和 `str` 方法依赖的 Unicode 属性、大小写转换和可打印性数据。
//! 这些数据由 Unicode 标准版本驱动，会随标准升级而变化；调用方只应依赖标准库公开 API，
//! 不应把本模块的表布局或压缩方式当作稳定契约。
#![unstable(feature = "unicode_internals", issue = "none")]
#![doc(hidden)]

// 供 alloc 使用，但不会在 std 中重新导出。
#[rustfmt::skip]
pub use unicode_data::case_ignorable::lookup as Case_Ignorable;
pub use unicode_data::cased::lookup as Cased;
pub use unicode_data::conversions;

#[rustfmt::skip]
pub(crate) use unicode_data::alphabetic::lookup as Alphabetic;
pub(crate) use unicode_data::grapheme_extend::lookup as Grapheme_Extend;
pub(crate) use unicode_data::lowercase::lookup as Lowercase;
pub(crate) use unicode_data::n::lookup as N;
pub(crate) use unicode_data::uppercase::lookup as Uppercase;
pub(crate) use unicode_data::white_space::lookup as White_Space;

pub(crate) mod printable;

#[allow(unreachable_pub)]
mod unicode_data;

/// `char` 和 `str` 中 Unicode 相关方法所依据的
/// [Unicode](https://www.unicode.org/) 版本。
///
/// Unicode 会定期发布新版本，标准库中依赖 Unicode 数据的方法也会随之更新。
/// 因此，部分 `char` 和 `str` 方法的行为以及该常量的值会随时间变化；
/// 这种随 Unicode 标准演进而发生的变化不视为破坏性变更。
///
/// 版本号规则见
/// [Unicode 11.0 或更高版本，3.1 节 Versions of the Unicode Standard](https://www.unicode.org/versions/Unicode11.0.0/ch03.pdf#page=4)。
pub const UNICODE_VERSION: (u8, u8, u8) = unicode_data::UNICODE_VERSION;
