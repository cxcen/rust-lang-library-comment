//! Unix 及许多其他系统上 OsString/OsStr 的底层实现：就是一个
//! `Vec<u8>`/`[u8]`。

use core::clone::CloneToUninit;

use crate::borrow::Cow;
use crate::collections::TryReserveError;
use crate::fmt::Write;
use crate::rc::Rc;
use crate::sync::Arc;
use crate::sys::{AsInner, FromInner, IntoInner};
use crate::{fmt, mem, str};

#[cfg(test)]
mod tests;

#[derive(Hash)]
#[repr(transparent)]
pub struct Buf {
    pub inner: Vec<u8>,
}

#[repr(transparent)]
pub struct Slice {
    pub inner: [u8],
}

impl IntoInner<Vec<u8>> for Buf {
    fn into_inner(self) -> Vec<u8> {
        self.inner
    }
}

impl FromInner<Vec<u8>> for Buf {
    fn from_inner(inner: Vec<u8>) -> Self {
        Buf { inner }
    }
}

impl AsInner<[u8]> for Buf {
    #[inline]
    fn as_inner(&self) -> &[u8] {
        &self.inner
    }
}

impl fmt::Debug for Buf {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.as_slice(), f)
    }
}

impl fmt::Display for Buf {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self.as_slice(), f)
    }
}

impl fmt::Debug for Slice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.inner.utf8_chunks().debug(), f)
    }
}

impl fmt::Display for Slice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // 如果我们是空字符串，那么迭代器实际上不会产生任何东西，
        // 所以手动执行格式化
        if self.inner.is_empty() {
            return "".fmt(f);
        }

        for chunk in self.inner.utf8_chunks() {
            let valid = chunk.valid();
            // 如果我们成功地把整个块解码为了一个有效的字符串，那么
            // 我们可以直接返回对该字符串的格式化，这样在可能的情况下
            // 也能尊重各种格式化标志。
            if chunk.invalid().is_empty() {
                return valid.fmt(f);
            }

            f.write_str(valid)?;
            f.write_char(char::REPLACEMENT_CHARACTER)?;
        }
        Ok(())
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
        self.inner
    }

    #[inline]
    pub unsafe fn from_encoded_bytes_unchecked(s: Vec<u8>) -> Self {
        Self { inner: s }
    }

    #[inline]
    pub fn into_string(self) -> Result<String, Buf> {
        String::from_utf8(self.inner).map_err(|p| Buf { inner: p.into_bytes() })
    }

    #[inline]
    pub const fn from_string(s: String) -> Buf {
        Buf { inner: s.into_bytes() }
    }

    #[inline]
    pub fn with_capacity(capacity: usize) -> Buf {
        Buf { inner: Vec::with_capacity(capacity) }
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
        self.inner.extend_from_slice(&s.inner)
    }

    #[inline]
    pub fn push_str(&mut self, s: &str) {
        self.inner.extend_from_slice(s.as_bytes());
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
        // SAFETY: Slice 只是对 [u8] 的一个包装，
        // 而 self.inner.as_slice() 返回 &[u8]。
        // 因此，将 &[u8] transmute 为 &Slice 是安全的。
        unsafe { mem::transmute(self.inner.as_slice()) }
    }

    #[inline]
    pub fn as_mut_slice(&mut self) -> &mut Slice {
        // SAFETY: Slice 只是对 [u8] 的一个包装，
        // 而 self.inner.as_mut_slice() 返回 &mut [u8]。
        // 因此，将 &mut [u8] transmute 为 &mut Slice 是安全的。
        unsafe { mem::transmute(self.inner.as_mut_slice()) }
    }

    #[inline]
    pub fn leak<'a>(self) -> &'a mut Slice {
        unsafe { mem::transmute(self.inner.leak()) }
    }

    #[inline]
    pub fn into_box(self) -> Box<Slice> {
        unsafe { mem::transmute(self.inner.into_boxed_slice()) }
    }

    #[inline]
    pub fn from_box(boxed: Box<Slice>) -> Buf {
        let inner: Box<[u8]> = unsafe { mem::transmute(boxed) };
        Buf { inner: inner.into_vec() }
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
    /// 中所述）。这种编码没有任何安全性要求。
    #[inline]
    pub unsafe fn extend_from_slice_unchecked(&mut self, other: &[u8]) {
        self.inner.extend_from_slice(other);
    }
}

impl Slice {
    #[inline]
    pub fn as_encoded_bytes(&self) -> &[u8] {
        &self.inner
    }

    #[inline]
    pub unsafe fn from_encoded_bytes_unchecked(s: &[u8]) -> &Slice {
        unsafe { mem::transmute(s) }
    }

    #[track_caller]
    #[inline]
    pub fn check_public_boundary(&self, index: usize) {
        if index == 0 || index == self.inner.len() {
            return;
        }
        if index < self.inner.len()
            && (self.inner[index - 1].is_ascii() || self.inner[index].is_ascii())
        {
            return;
        }

        slow_path(&self.inner, index);

        /// 我们押注典型的切分操作会涉及一个 ASCII 字符。
        ///
        /// 把昂贵的检查放进一个单独的函数里，会生成明显更好的汇编代码。
        #[track_caller]
        #[inline(never)]
        fn slow_path(bytes: &[u8], index: usize) {
            let (before, after) = bytes.split_at(index);

            // UTF-8 每个码点最多占 4 个字节，所以我们不需要
            // 检查超过这个数量的字节。
            let after = after.get(..4).unwrap_or(after);
            match str::from_utf8(after) {
                Ok(_) => return,
                Err(err) if err.valid_up_to() != 0 => return,
                Err(_) => (),
            }

            for len in 2..=4.min(index) {
                let before = &before[index - len..];
                if str::from_utf8(before).is_ok() {
                    return;
                }
            }

            panic!("byte index {index} is not an OsStr boundary");
        }
    }

    #[inline]
    pub fn from_str(s: &str) -> &Slice {
        unsafe { Slice::from_encoded_bytes_unchecked(s.as_bytes()) }
    }

    #[inline]
    pub fn to_str(&self) -> Result<&str, crate::str::Utf8Error> {
        str::from_utf8(&self.inner)
    }

    #[inline]
    pub fn to_string_lossy(&self) -> Cow<'_, str> {
        String::from_utf8_lossy(&self.inner)
    }

    #[inline]
    pub fn to_owned(&self) -> Buf {
        Buf { inner: self.inner.to_vec() }
    }

    #[inline]
    pub fn clone_into(&self, buf: &mut Buf) {
        self.inner.clone_into(&mut buf.inner)
    }

    #[inline]
    pub fn empty_box() -> Box<Slice> {
        let boxed: Box<[u8]> = Default::default();
        unsafe { mem::transmute(boxed) }
    }

    #[inline]
    pub fn into_arc(&self) -> Arc<Slice> {
        let arc: Arc<[u8]> = Arc::from(&self.inner);
        unsafe { Arc::from_raw(Arc::into_raw(arc) as *const Slice) }
    }

    #[inline]
    pub fn into_rc(&self) -> Rc<Slice> {
        let rc: Rc<[u8]> = Rc::from(&self.inner);
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
        // SAFETY: 我们只是对 [u8] 的一个透明包装
        unsafe { self.inner.clone_to_uninit(dst) }
    }
}
