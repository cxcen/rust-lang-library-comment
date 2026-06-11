//! Windows 上 OsString/OsStr 的底层实现是对「WTF-8」编码的一层包装；
//! 更多内容参见 `wtf8` 模块。

use alloc::wtf8::{Wtf8, Wtf8Buf};
use core::clone::CloneToUninit;

use crate::borrow::Cow;
use crate::collections::TryReserveError;
use crate::rc::Rc;
use crate::sync::Arc;
use crate::sys::{AsInner, FromInner, IntoInner};
use crate::{fmt, mem};

#[derive(Hash)]
#[repr(transparent)]
pub struct Buf {
    pub inner: Wtf8Buf,
}

#[repr(transparent)]
pub struct Slice {
    pub inner: Wtf8,
}

impl IntoInner<Wtf8Buf> for Buf {
    fn into_inner(self) -> Wtf8Buf {
        self.inner
    }
}

impl FromInner<Wtf8Buf> for Buf {
    fn from_inner(inner: Wtf8Buf) -> Self {
        Buf { inner }
    }
}

impl AsInner<Wtf8> for Buf {
    #[inline]
    fn as_inner(&self) -> &Wtf8 {
        &self.inner
    }
}

impl fmt::Debug for Buf {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.inner, f)
    }
}

impl fmt::Display for Buf {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.inner, f)
    }
}

impl fmt::Debug for Slice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.inner, f)
    }
}

impl fmt::Display for Slice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.inner, f)
    }
}

impl Clone for Buf {
    #[inline]
    fn clone(&self) -> Self {
        Buf { inner: self.inner.clone() }
    }

    #[inline]
    fn clone_from(&mut self, source: &Self) {
        self.inner.clone_from(&source.inner)
    }
}

impl Buf {
    #[inline]
    pub fn into_encoded_bytes(self) -> Vec<u8> {
        self.inner.into_bytes()
    }

    #[inline]
    pub unsafe fn from_encoded_bytes_unchecked(s: Vec<u8>) -> Self {
        unsafe { Self { inner: Wtf8Buf::from_bytes_unchecked(s) } }
    }

    #[inline]
    pub fn into_string(self) -> Result<String, Buf> {
        self.inner.into_string().map_err(|buf| Buf { inner: buf })
    }

    #[inline]
    pub const fn from_string(s: String) -> Buf {
        Buf { inner: Wtf8Buf::from_string(s) }
    }

    #[inline]
    pub fn with_capacity(capacity: usize) -> Buf {
        Buf { inner: Wtf8Buf::with_capacity(capacity) }
    }

    #[inline]
    pub fn clear(&mut self) {
        self.inner.clear()
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        self.inner.capacity()
    }

    #[inline]
    pub fn push_slice(&mut self, s: &Slice) {
        self.inner.push_wtf8(&s.inner)
    }

    #[inline]
    pub fn push_str(&mut self, s: &str) {
        self.inner.push_str(s);
    }

    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        self.inner.reserve(additional)
    }

    #[inline]
    pub fn try_reserve(&mut self, additional: usize) -> Result<(), TryReserveError> {
        self.inner.try_reserve(additional)
    }

    #[inline]
    pub fn reserve_exact(&mut self, additional: usize) {
        self.inner.reserve_exact(additional)
    }

    #[inline]
    pub fn try_reserve_exact(&mut self, additional: usize) -> Result<(), TryReserveError> {
        self.inner.try_reserve_exact(additional)
    }

    #[inline]
    pub fn shrink_to_fit(&mut self) {
        self.inner.shrink_to_fit()
    }

    #[inline]
    pub fn shrink_to(&mut self, min_capacity: usize) {
        self.inner.shrink_to(min_capacity)
    }

    #[inline]
    pub fn as_slice(&self) -> &Slice {
        // SAFETY: Slice 只是对 Wtf8 的一个包装，
        // 而 self.inner.as_slice() 返回 &Wtf8。
        // 因此，将 &Wtf8 transmute 为 &Slice 是安全的。
        unsafe { mem::transmute(self.inner.as_slice()) }
    }

    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut Slice {
        // SAFETY: Slice 只是对 Wtf8 的一个包装，
        // 而 self.inner.as_mut_slice() 返回 &mut Wtf8。
        // 因此，将 &mut Wtf8 transmute 为 &mut Slice 是安全的。
        // 此外，应当注意确保该切片始终是有效的 Wtf8。
        unsafe { mem::transmute(self.inner.as_mut_slice()) }
    }

    #[inline]
    pub fn leak<'a>(self) -> &'a mut Slice {
        unsafe { mem::transmute(self.inner.leak()) }
    }

    #[inline]
    pub fn into_box(self) -> Box<Slice> {
        unsafe { mem::transmute(self.inner.into_box()) }
    }

    #[inline]
    pub fn from_box(boxed: Box<Slice>) -> Buf {
        let inner: Box<Wtf8> = unsafe { mem::transmute(boxed) };
        Buf { inner: Wtf8Buf::from_box(inner) }
    }

    #[inline]
    pub fn into_arc(&self) -> Arc<Slice> {
        self.as_slice().into_arc()
    }

    #[inline]
    pub fn into_rc(&self) -> Rc<Slice> {
        self.as_slice().into_rc()
    }

    /// 在不给予对 `Vec` 完全可变访问权的前提下，提供通往 `Vec::truncate` 的管道。
    ///
    /// # 安全性(Safety）
    ///
    /// 根据 `Slice::check_public_boundary`，长度必须位于一个 `OsStr` 边界上。
    #[inline]
    pub unsafe fn truncate_unchecked(&mut self, len: usize) {
        self.inner.truncate(len);
    }

    /// 在不给予对 `Vec` 完全可变访问权的前提下，提供通往
    /// `Vec::extend_from_slice` 的管道。
    ///
    /// # 安全性(Safety）
    ///
    /// 该切片必须对平台编码有效（如 `OsStr::from_encoded_bytes_unchecked`
    /// 中所述）。对于这种编码，这意味着 `other` 必须是有效的 WTF-8。
    ///
    /// 此外，本方法会绕过 WTF-8 的代理对拼接(surrogate joining），所以
    /// 要么 `self` 不能以一个前导代理半部(leading surrogate half）结尾，
    /// 要么 `other` 不能以一个后尾代理半部(trailing surrogate half）开头。
    #[inline]
    pub unsafe fn extend_from_slice_unchecked(&mut self, other: &[u8]) {
        unsafe {
            self.inner.extend_from_slice_unchecked(other);
        }
    }
}

impl Slice {
    #[inline]
    pub fn as_encoded_bytes(&self) -> &[u8] {
        self.inner.as_bytes()
    }

    #[inline]
    pub unsafe fn from_encoded_bytes_unchecked(s: &[u8]) -> &Slice {
        unsafe { mem::transmute(Wtf8::from_bytes_unchecked(s)) }
    }

    #[track_caller]
    #[inline]
    pub fn check_public_boundary(&self, index: usize) {
        self.inner.check_utf8_boundary(index);
    }

    #[inline]
    pub fn from_str(s: &str) -> &Slice {
        unsafe { mem::transmute(Wtf8::from_str(s)) }
    }

    #[inline]
    pub fn to_str(&self) -> Result<&str, crate::str::Utf8Error> {
        self.inner.as_str()
    }

    #[inline]
    pub fn to_string_lossy(&self) -> Cow<'_, str> {
        self.inner.to_string_lossy()
    }

    #[inline]
    pub fn to_owned(&self) -> Buf {
        Buf { inner: self.inner.to_owned() }
    }

    #[inline]
    pub fn clone_into(&self, buf: &mut Buf) {
        self.inner.clone_into(&mut buf.inner)
    }

    #[inline]
    pub fn empty_box() -> Box<Slice> {
        unsafe { mem::transmute(Wtf8::empty_box()) }
    }

    #[inline]
    pub fn into_arc(&self) -> Arc<Slice> {
        let arc = self.inner.into_arc();
        unsafe { Arc::from_raw(Arc::into_raw(arc) as *const Slice) }
    }

    #[inline]
    pub fn into_rc(&self) -> Rc<Slice> {
        let rc = self.inner.into_rc();
        unsafe { Rc::from_raw(Rc::into_raw(rc) as *const Slice) }
    }

    #[inline]
    pub fn make_ascii_lowercase(&mut self) {
        self.inner.make_ascii_lowercase()
    }

    #[inline]
    pub fn make_ascii_uppercase(&mut self) {
        self.inner.make_ascii_uppercase()
    }

    #[inline]
    pub fn to_ascii_lowercase(&self) -> Buf {
        Buf { inner: self.inner.to_ascii_lowercase() }
    }

    #[inline]
    pub fn to_ascii_uppercase(&self) -> Buf {
        Buf { inner: self.inner.to_ascii_uppercase() }
    }

    #[inline]
    pub fn is_ascii(&self) -> bool {
        self.inner.is_ascii()
    }

    #[inline]
    pub fn eq_ignore_ascii_case(&self, other: &Self) -> bool {
        self.inner.eq_ignore_ascii_case(&other.inner)
    }
}

#[unstable(feature = "clone_to_uninit", issue = "126799")]
unsafe impl CloneToUninit for Slice {
    #[inline]
    #[cfg_attr(debug_assertions, track_caller)]
    unsafe fn clone_to_uninit(&self, dst: *mut u8) {
        // SAFETY: 我们只是对 Wtf8 的一个透明包装
        unsafe { self.inner.clone_to_uninit(dst) }
    }
}
