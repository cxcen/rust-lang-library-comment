//! 本模块包含用于处理 16 位字符（UCS-2 或 UTF-16）的各种构件

use crate::marker::PhantomData;
use crate::num::NonZero;
use crate::ptr::NonNull;

/// 对 LPWSTR 的一个安全迭代器
/// （即指向一串以 NULL 结尾的 UTF-16 码元的指针）。
pub struct WStrUnits<'a> {
    // 该指针绝不能为 null……
    lpwstr: NonNull<u16>,
    // ……并且它所指向的内存必须在此生命周期内始终有效。
    lifetime: PhantomData<&'a [u16]>,
}

impl WStrUnits<'_> {
    /// 创建该迭代器。如果 `lpwstr` 为 null 则返回 `None`。
    ///
    /// SAFETY: `lpwstr` 必须指向一个以 null 结尾的宽字符串，且该字符串的存活时间
    /// 至少与本结构体的生命周期一样长。
    pub unsafe fn new(lpwstr: *const u16) -> Option<Self> {
        Some(Self { lpwstr: NonNull::new(lpwstr as _)?, lifetime: PhantomData })
    }

    pub fn peek(&self) -> Option<NonZero<u16>> {
        // SAFETY: 读取当前元素总是安全的，因为我们
        // 永远不会越过数组的边界。
        unsafe { NonZero::new(*self.lpwstr.as_ptr()) }
    }

    /// 在 `predicate` 返回 true 期间持续推进迭代器。
    /// 返回它推进了多少个元素。
    pub fn advance_while<P: FnMut(NonZero<u16>) -> bool>(&mut self, mut predicate: P) -> usize {
        let mut counter = 0;
        while let Some(w) = self.peek() {
            if !predicate(w) {
                break;
            }
            counter += 1;
            self.next();
        }
        counter
    }
}

impl Iterator for WStrUnits<'_> {
    // 这里永远不会返回 0，因为 0 标志着字符串的结束。
    type Item = NonZero<u16>;

    fn next(&mut self) -> Option<Self::Item> {
        // SAFETY: 一旦到达 NULL 我们就立即返回。
        // 因此在那之后推进指针是安全的。
        unsafe {
            let next = self.peek()?;
            self.lpwstr = NonNull::new_unchecked(self.lpwstr.as_ptr().add(1));
            Some(next)
        }
    }
}
