use crate::ffi::{OsStr, OsString};
use crate::mem;
use crate::sealed::Sealed;
use crate::sys::os_str::Buf;
use crate::sys::{AsInner, FromInner, IntoInner};

// 注意：本文件目前在其他 `std::os::{platform}::ffi` 模块中被复用，以减少重复。
// 当对本文件应用仅适用于 `unix` 的改动时，请记住这一点。

/// 针对 [`OsString`] 的平台特有扩展。
///
/// 该 trait 是封闭的（sealed）：它不能在标准库之外被实现。
/// 这样做是为了让未来新增的方法不会成为破坏性变更（breaking changes）。
#[stable(feature = "rust1", since = "1.0.0")]
pub trait OsStringExt: Sealed {
    /// 从一个字节向量创建 [`OsString`]。
    ///
    /// 示例参见模块文档。
    #[stable(feature = "rust1", since = "1.0.0")]
    fn from_vec(vec: Vec<u8>) -> Self;

    /// 取出此 [`OsString`] 底层的字节向量。
    ///
    /// 示例参见模块文档。
    #[stable(feature = "rust1", since = "1.0.0")]
    fn into_vec(self) -> Vec<u8>;
}

#[stable(feature = "rust1", since = "1.0.0")]
impl OsStringExt for OsString {
    #[inline]
    fn from_vec(vec: Vec<u8>) -> OsString {
        FromInner::from_inner(Buf { inner: vec })
    }
    #[inline]
    fn into_vec(self) -> Vec<u8> {
        self.into_inner().inner
    }
}

/// 针对 [`OsStr`] 的平台特有扩展。
///
/// 该 trait 是封闭的（sealed）：它不能在标准库之外被实现。
/// 这样做是为了让未来新增的方法不会成为破坏性变更（breaking changes）。
#[stable(feature = "rust1", since = "1.0.0")]
pub trait OsStrExt: Sealed {
    #[stable(feature = "rust1", since = "1.0.0")]
    /// 从一个字节切片创建 [`OsStr`]。
    ///
    /// 示例参见模块文档。
    fn from_bytes(slice: &[u8]) -> &Self;

    /// 获取 [`OsStr`] 切片底层的字节视图。
    ///
    /// 示例参见模块文档。
    #[stable(feature = "rust1", since = "1.0.0")]
    fn as_bytes(&self) -> &[u8];
}

#[stable(feature = "rust1", since = "1.0.0")]
impl OsStrExt for OsStr {
    #[inline]
    fn from_bytes(slice: &[u8]) -> &OsStr {
        unsafe { mem::transmute(slice) }
    }
    #[inline]
    fn as_bytes(&self) -> &[u8] {
        &self.as_inner().inner
    }
}
