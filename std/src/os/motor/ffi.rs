//! 针对 [`std::ffi`] 模块中各类基础类型的 Motor OS 平台特定扩展。
#![unstable(feature = "motor_ext", issue = "147456")]

use crate::ffi::{OsStr, OsString};
use crate::sealed::Sealed;
use crate::sys::{AsInner, IntoInner};

/// 针对 [`OsString`] 的 Motor OS 平台特定扩展。
///
/// 此 trait 是密封的（sealed）：它无法在标准库之外被实现。
/// 这样做是为了使将来新增的方法不构成破坏性变更（breaking changes）。
pub trait OsStringExt: Sealed {
    /// 产出此 [`OsString`] 底层的 UTF-8 字符串。
    ///
    /// Motor OS 上的 OS 字符串保证是 UTF-8 的，因此它们就是普通字符串。
    fn into_string(self) -> String;
}

impl OsStringExt for OsString {
    #[inline]
    fn into_string(self) -> String {
        self.into_inner().inner
    }
}

/// 针对 [`OsString`] 的 Motor OS 平台特定扩展。
///
/// 此 trait 是密封的（sealed）：它无法在标准库之外被实现。
/// 这样做是为了使将来新增的方法不构成破坏性变更（breaking changes）。
pub trait OsStrExt: Sealed {
    /// 获取 [`OsStr`] 切片底层的 UTF-8 字符串视图。
    ///
    /// Motor OS 上的 OS 字符串保证是 UTF-8 的，因此它们就是普通字符串。
    fn as_str(&self) -> &str;
}

impl OsStrExt for OsStr {
    #[inline]
    fn as_str(&self) -> &str {
        &self.as_inner().inner
    }
}
