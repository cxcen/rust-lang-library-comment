use crate::ffi::{CStr, CString, c_char};
use crate::ops::Index;
use crate::{fmt, mem, ptr};

/// 用于管理 C 风格数组中各字符串所有权的辅助类型。
///
/// 该类型管理一个以空指针（null pointer）结尾的 C 字符串指针数组。
/// 指向该数组的指针（由 `as_ptr` 返回）可以用作 `argv` 或 `environ` 的值。
pub struct CStringArray {
    ptrs: Vec<*const c_char>,
}

impl CStringArray {
    /// 创建一个新的 `CStringArray`，其容量足以容纳 `capacity` 个字符串。
    pub fn with_capacity(capacity: usize) -> Self {
        let mut result = CStringArray { ptrs: Vec::with_capacity(capacity + 1) };
        result.ptrs.push(ptr::null());
        result
    }

    /// 替换位置 `index` 处的字符串。
    pub fn write(&mut self, index: usize, item: CString) {
        let argc = self.ptrs.len() - 1;
        let ptr = &mut self.ptrs[..argc][index];
        let old = mem::replace(ptr, item.into_raw());
        // SAFETY:
        // `CStringArray` 拥有它所有的字符串，并且它们都是用 `CString::into_raw`
        // 转换成指针的。此外，这不是空指针，因为否则上面的索引操作就会失败。
        drop(unsafe { CString::from_raw(old.cast_mut()) });
    }

    /// 向数组中追加（push）一个额外的字符串。
    pub fn push(&mut self, item: CString) {
        let argc = self.ptrs.len() - 1;
        // 替换掉数组末尾的空指针……
        self.ptrs[argc] = item.into_raw();
        // ……然后重新创建它，以恢复该数据结构的不变量（invariant）。
        self.ptrs.push(ptr::null());
    }

    /// 返回一个指向该类型所管理的 C 字符串数组的指针。
    pub fn as_ptr(&self) -> *const *const c_char {
        self.ptrs.as_ptr()
    }

    /// 返回一个遍历此数组中包含的所有 `CStr` 的迭代器。
    pub fn iter(&self) -> CStringIter<'_> {
        CStringIter { iter: self.ptrs[..self.ptrs.len() - 1].iter() }
    }
}

impl Index<usize> for CStringArray {
    type Output = CStr;
    fn index(&self, index: usize) -> &CStr {
        let ptr = self.ptrs[..self.ptrs.len() - 1][index];
        // SAFETY:
        // `CStringArray` 拥有它所有的字符串。此外，这不是空指针，
        // 因为否则上面的索引操作就会失败。
        unsafe { CStr::from_ptr(ptr) }
    }
}

impl fmt::Debug for CStringArray {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_list().entries(self.iter()).finish()
    }
}

// SAFETY: `CStringArray` 基本上就是一个 `Vec<CString>`
unsafe impl Send for CStringArray {}
// SAFETY: `CStringArray` 基本上就是一个 `Vec<CString>`
unsafe impl Sync for CStringArray {}

impl Drop for CStringArray {
    fn drop(&mut self) {
        // SAFETY:
        // `CStringArray` 拥有它所有的字符串，并且它们都是用 `CString::into_raw`
        // 转换成指针的。
        self.ptrs[..self.ptrs.len() - 1]
            .iter()
            .for_each(|&p| drop(unsafe { CString::from_raw(p.cast_mut()) }))
    }
}

/// 一个遍历 `CStringArray` 中所包含的所有 `CStr` 的迭代器。
#[derive(Clone)]
pub struct CStringIter<'a> {
    iter: crate::slice::Iter<'a, *const c_char>,
}

impl<'a> Iterator for CStringIter<'a> {
    type Item = &'a CStr;
    fn next(&mut self) -> Option<&'a CStr> {
        // SAFETY:
        // `CStringArray` 拥有它所有的字符串。此外，这不是空指针，
        // 因为创建 `iter` 时已经排除了最后一个元素。
        self.iter.next().map(|&p| unsafe { CStr::from_ptr(p) })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.iter.size_hint()
    }
}

impl<'a> ExactSizeIterator for CStringIter<'a> {
    fn len(&self) -> usize {
        self.iter.len()
    }
    fn is_empty(&self) -> bool {
        self.iter.is_empty()
    }
}
