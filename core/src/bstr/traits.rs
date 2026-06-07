//! `ByteStr` 的 trait 实现。
//!
//! 这些实现把 `ByteStr` 作为字节序列参与比较、哈希和索引。它们不引入 UTF-8
//! 有效性要求，所有顺序和相等性判断都基于底层 `[u8]` 的语义。

use crate::bstr::ByteStr;
use crate::cmp::Ordering;
use crate::slice::SliceIndex;
use crate::{hash, ops, range};

#[unstable(feature = "bstr", issue = "134915")]
impl Ord for ByteStr {
    #[inline]
    fn cmp(&self, other: &ByteStr) -> Ordering {
        Ord::cmp(&self.0, &other.0)
    }
}

#[unstable(feature = "bstr", issue = "134915")]
impl PartialOrd for ByteStr {
    #[inline]
    fn partial_cmp(&self, other: &ByteStr) -> Option<Ordering> {
        PartialOrd::partial_cmp(&self.0, &other.0)
    }
}

#[unstable(feature = "bstr", issue = "134915")]
impl PartialEq<ByteStr> for ByteStr {
    #[inline]
    fn eq(&self, other: &ByteStr) -> bool {
        &self.0 == &other.0
    }
}

#[unstable(feature = "bstr", issue = "134915")]
impl Eq for ByteStr {}

#[unstable(feature = "bstr", issue = "134915")]
impl hash::Hash for ByteStr {
    #[inline]
    fn hash<H: hash::Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}

#[doc(hidden)]
#[macro_export]
#[unstable(feature = "bstr_internals", issue = "none")]
macro_rules! impl_partial_eq {
    ($lhs:ty, $rhs:ty) => {
        #[allow(unused_lifetimes)]
        impl<'a> PartialEq<$rhs> for $lhs {
            #[inline]
            fn eq(&self, other: &$rhs) -> bool {
                let other: &[u8] = other.as_ref();
                PartialEq::eq(self.as_bytes(), other)
            }
        }

        #[allow(unused_lifetimes)]
        impl<'a> PartialEq<$lhs> for $rhs {
            #[inline]
            fn eq(&self, other: &$lhs) -> bool {
                let this: &[u8] = self.as_ref();
                PartialEq::eq(this, other.as_bytes())
            }
        }
    };
}

#[doc(hidden)]
#[unstable(feature = "bstr_internals", issue = "none")]
pub use impl_partial_eq;

#[doc(hidden)]
#[macro_export]
#[unstable(feature = "bstr_internals", issue = "none")]
macro_rules! impl_partial_eq_ord {
    ($lhs:ty, $rhs:ty) => {
        $crate::bstr::impl_partial_eq!($lhs, $rhs);

        #[allow(unused_lifetimes)]
        #[unstable(feature = "bstr", issue = "134915")]
        impl<'a> PartialOrd<$rhs> for $lhs {
            #[inline]
            fn partial_cmp(&self, other: &$rhs) -> Option<Ordering> {
                let other: &[u8] = other.as_ref();
                PartialOrd::partial_cmp(self.as_bytes(), other)
            }
        }

        #[allow(unused_lifetimes)]
        #[unstable(feature = "bstr", issue = "134915")]
        impl<'a> PartialOrd<$lhs> for $rhs {
            #[inline]
            fn partial_cmp(&self, other: &$lhs) -> Option<Ordering> {
                let this: &[u8] = self.as_ref();
                PartialOrd::partial_cmp(this, other.as_bytes())
            }
        }
    };
}

#[doc(hidden)]
#[unstable(feature = "bstr_internals", issue = "none")]
pub use impl_partial_eq_ord;

#[doc(hidden)]
#[macro_export]
#[unstable(feature = "bstr_internals", issue = "none")]
macro_rules! impl_partial_eq_n {
    ($lhs:ty, $rhs:ty) => {
        #[allow(unused_lifetimes)]
        #[unstable(feature = "bstr", issue = "134915")]
        impl<const N: usize> PartialEq<$rhs> for $lhs {
            #[inline]
            fn eq(&self, other: &$rhs) -> bool {
                let other: &[u8] = other.as_ref();
                PartialEq::eq(self.as_bytes(), other)
            }
        }

        #[allow(unused_lifetimes)]
        #[unstable(feature = "bstr", issue = "134915")]
        impl<const N: usize> PartialEq<$lhs> for $rhs {
            #[inline]
            fn eq(&self, other: &$lhs) -> bool {
                let this: &[u8] = self.as_ref();
                PartialEq::eq(this, other.as_bytes())
            }
        }
    };
}

#[doc(hidden)]
#[unstable(feature = "bstr_internals", issue = "none")]
pub use impl_partial_eq_n;

// 省略与 `[u8]` 的 PartialOrd，避免泛型场景中出现类型推断失败。
impl_partial_eq!(ByteStr, [u8]);
// 省略与 `&[u8]` 的 PartialOrd，避免泛型场景中出现类型推断失败。
impl_partial_eq!(ByteStr, &[u8]);
// 省略与 `str` 的 PartialOrd，避免泛型场景中出现类型推断失败。
impl_partial_eq!(ByteStr, str);
// 省略与 `&str` 的 PartialOrd，避免泛型场景中出现类型推断失败。
impl_partial_eq!(ByteStr, &str);
// 省略与 `[u8; N]` 的 PartialOrd，避免泛型场景中出现类型推断失败。
impl_partial_eq_n!(ByteStr, [u8; N]);
// 省略与 `&[u8; N]` 的 PartialOrd，避免泛型场景中出现类型推断失败。
impl_partial_eq_n!(ByteStr, &[u8; N]);

#[unstable(feature = "bstr", issue = "134915")]
impl<I> ops::Index<I> for ByteStr
where
    I: SliceIndex<ByteStr>,
{
    type Output = I::Output;

    #[inline]
    fn index(&self, index: I) -> &I::Output {
        index.index(self)
    }
}

#[unstable(feature = "bstr", issue = "134915")]
impl<I> ops::IndexMut<I> for ByteStr
where
    I: SliceIndex<ByteStr>,
{
    #[inline]
    fn index_mut(&mut self, index: I) -> &mut I::Output {
        index.index_mut(self)
    }
}

#[unstable(feature = "bstr", issue = "134915")]
unsafe impl SliceIndex<ByteStr> for ops::RangeFull {
    type Output = ByteStr;
    #[inline]
    fn get(self, slice: &ByteStr) -> Option<&Self::Output> {
        Some(slice)
    }
    #[inline]
    fn get_mut(self, slice: &mut ByteStr) -> Option<&mut Self::Output> {
        Some(slice)
    }
    #[inline]
    unsafe fn get_unchecked(self, slice: *const ByteStr) -> *const Self::Output {
        slice
    }
    #[inline]
    unsafe fn get_unchecked_mut(self, slice: *mut ByteStr) -> *mut Self::Output {
        slice
    }
    #[inline]
    fn index(self, slice: &ByteStr) -> &Self::Output {
        slice
    }
    #[inline]
    fn index_mut(self, slice: &mut ByteStr) -> &mut Self::Output {
        slice
    }
}

#[unstable(feature = "bstr", issue = "134915")]
unsafe impl SliceIndex<ByteStr> for usize {
    type Output = u8;
    #[inline]
    fn get(self, slice: &ByteStr) -> Option<&Self::Output> {
        self.get(slice.as_bytes())
    }
    #[inline]
    fn get_mut(self, slice: &mut ByteStr) -> Option<&mut Self::Output> {
        self.get_mut(slice.as_bytes_mut())
    }
    #[inline]
    unsafe fn get_unchecked(self, slice: *const ByteStr) -> *const Self::Output {
        // SAFETY: 调用方必须维护 `get_unchecked` 的安全契约；这里仅把透明包装指针转回 `[u8]` 指针。
        unsafe { self.get_unchecked(slice as *const [u8]) }
    }
    #[inline]
    unsafe fn get_unchecked_mut(self, slice: *mut ByteStr) -> *mut Self::Output {
        // SAFETY: 调用方必须维护 `get_unchecked_mut` 的安全契约；这里仅把透明包装指针转回 `[u8]` 指针。
        unsafe { self.get_unchecked_mut(slice as *mut [u8]) }
    }
    #[inline]
    fn index(self, slice: &ByteStr) -> &Self::Output {
        self.index(slice.as_bytes())
    }
    #[inline]
    fn index_mut(self, slice: &mut ByteStr) -> &mut Self::Output {
        self.index_mut(slice.as_bytes_mut())
    }
}

macro_rules! impl_slice_index {
    ($index:ty) => {
        #[unstable(feature = "bstr", issue = "134915")]
        unsafe impl SliceIndex<ByteStr> for $index {
            type Output = ByteStr;
            #[inline]
            fn get(self, slice: &ByteStr) -> Option<&Self::Output> {
                self.get(slice.as_bytes()).map(ByteStr::from_bytes)
            }
            #[inline]
            fn get_mut(self, slice: &mut ByteStr) -> Option<&mut Self::Output> {
                self.get_mut(slice.as_bytes_mut()).map(ByteStr::from_bytes_mut)
            }
            #[inline]
            unsafe fn get_unchecked(self, slice: *const ByteStr) -> *const Self::Output {
                // SAFETY: 调用方必须维护 `get_unchecked` 的安全契约；索引计算委托给底层 `[u8]` 实现。
                unsafe { self.get_unchecked(slice as *const [u8]) as *const ByteStr }
            }
            #[inline]
            unsafe fn get_unchecked_mut(self, slice: *mut ByteStr) -> *mut Self::Output {
                // SAFETY: 调用方必须维护 `get_unchecked_mut` 的安全契约；索引计算委托给底层 `[u8]` 实现。
                unsafe { self.get_unchecked_mut(slice as *mut [u8]) as *mut ByteStr }
            }
            #[inline]
            fn index(self, slice: &ByteStr) -> &Self::Output {
                ByteStr::from_bytes(self.index(slice.as_bytes()))
            }
            #[inline]
            fn index_mut(self, slice: &mut ByteStr) -> &mut Self::Output {
                ByteStr::from_bytes_mut(self.index_mut(slice.as_bytes_mut()))
            }
        }
    };
}

impl_slice_index!(ops::IndexRange);
impl_slice_index!(ops::Range<usize>);
impl_slice_index!(range::Range<usize>);
impl_slice_index!(ops::RangeTo<usize>);
impl_slice_index!(ops::RangeFrom<usize>);
impl_slice_index!(range::RangeFrom<usize>);
impl_slice_index!(ops::RangeInclusive<usize>);
impl_slice_index!(range::RangeInclusive<usize>);
impl_slice_index!(ops::RangeToInclusive<usize>);
impl_slice_index!((ops::Bound<usize>, ops::Bound<usize>));
