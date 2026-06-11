//! 一个双向链表，其中链表节点的内存分配由调用方负责管理。

#[cfg(test)]
mod tests;

use crate::mem;
use crate::ptr::NonNull;

pub struct UnsafeListEntry<T> {
    next: NonNull<UnsafeListEntry<T>>,
    prev: NonNull<UnsafeListEntry<T>>,
    value: Option<T>,
}

impl<T> UnsafeListEntry<T> {
    fn dummy() -> Self {
        UnsafeListEntry { next: NonNull::dangling(), prev: NonNull::dangling(), value: None }
    }

    pub fn new(value: T) -> Self {
        UnsafeListEntry { value: Some(value), ..Self::dummy() }
    }
}

// 警告：自引用（self-referential）结构体！
pub struct UnsafeList<T> {
    head_tail: NonNull<UnsafeListEntry<T>>,
    head_tail_entry: Option<UnsafeListEntry<T>>,
}

impl<T> UnsafeList<T> {
    pub const fn new() -> Self {
        unsafe { UnsafeList { head_tail: NonNull::new_unchecked(1 as _), head_tail_entry: None } }
    }

    /// # 安全性(Safety）
    unsafe fn init(&mut self) {
        if self.head_tail_entry.is_none() {
            self.head_tail_entry = Some(UnsafeListEntry::dummy());
            // SAFETY: `head_tail_entry` 必须非空，而它确实非空，因为我们在上面对其赋了值。
            self.head_tail =
                unsafe { NonNull::new_unchecked(self.head_tail_entry.as_mut().unwrap()) };
            // SAFETY: `self.head_tail` 必须满足可变引用的所有要求。
            unsafe { self.head_tail.as_mut() }.next = self.head_tail;
            unsafe { self.head_tail.as_mut() }.prev = self.head_tail;
        }
    }

    pub fn is_empty(&self) -> bool {
        if self.head_tail_entry.is_some() {
            let first = unsafe { self.head_tail.as_ref() }.next;
            if first == self.head_tail {
                // ,-------> /---------\ next ---,
                // |         |head_tail|         |
                // `--- prev \---------/ <-------`
                // SAFETY: `self.head_tail` 必须满足引用的所有要求。
                unsafe { rtassert!(self.head_tail.as_ref().prev == first) };
                true
            } else {
                false
            }
        } else {
            true
        }
    }

    /// 将一个 entry 压入链表尾部。
    ///
    /// # 安全性(Safety）
    ///
    /// 该 entry 必须保持已分配状态，直到它被从链表中移除、并且执行 pop 的调用方
    /// 已用完该 entry 为止。在 `push` 的调用方中必须特别小心，确保栈展开
    /// （unwinding）不会销毁包含该 entry 的栈帧。
    pub unsafe fn push<'a>(&mut self, entry: &'a mut UnsafeListEntry<T>) -> &'a T {
        unsafe { self.init() };

        // 操作前(BEFORE)：
        //     /---------\ next ---> /---------\
        // ... |prev_tail|           |head_tail| ...
        //     \---------/ <--- prev \---------/
        //
        // 操作后(AFTER)：
        //     /---------\ next ---> /-----\ next ---> /---------\
        // ... |prev_tail|           |entry|           |head_tail| ...
        //     \---------/ <--- prev \-----/ <--- prev \---------/
        let mut entry = unsafe { NonNull::new_unchecked(entry) };
        let mut prev_tail = mem::replace(&mut unsafe { self.head_tail.as_mut() }.prev, entry);
        // SAFETY: `entry` 必须满足可变引用的所有要求。
        unsafe { entry.as_mut() }.prev = prev_tail;
        unsafe { entry.as_mut() }.next = self.head_tail;
        // SAFETY: `prev_tail` 必须满足可变引用的所有要求。
        unsafe { prev_tail.as_mut() }.next = entry;
        // unwrap ok: 在非 dummy 的 entry 上始终为 `Some`
        unsafe { (*entry.as_ptr()).value.as_ref() }.unwrap()
    }

    /// 从链表头部弹出一个 entry。
    ///
    /// # 安全性(Safety）
    ///
    /// 调用方必须确保对返回值的借用的结束与所属 entry 的释放之间是同步的。
    pub unsafe fn pop<'a>(&mut self) -> Option<&'a T> {
        unsafe { self.init() };

        if self.is_empty() {
            None
        } else {
            // 操作前(BEFORE)：
            //     /---------\ next ---> /-----\ next ---> /------\
            // ... |head_tail|           |first|           |second| ...
            //     \---------/ <--- prev \-----/ <--- prev \------/
            //
            // 操作后(AFTER)：
            //     /---------\ next ---> /------\
            // ... |head_tail|           |second| ...
            //     \---------/ <--- prev \------/
            let mut first = unsafe { self.head_tail.as_mut() }.next;
            let mut second = unsafe { first.as_mut() }.next;
            unsafe { self.head_tail.as_mut() }.next = second;
            unsafe { second.as_mut() }.prev = self.head_tail;
            unsafe { first.as_mut() }.next = NonNull::dangling();
            unsafe { first.as_mut() }.prev = NonNull::dangling();
            // unwrap ok: 在非 dummy 的 entry 上始终为 `Some`
            Some(unsafe { (*first.as_ptr()).value.as_ref() }.unwrap())
        }
    }

    /// 从链表中移除一个 entry。
    ///
    /// # 安全性(Safety）
    ///
    /// 调用方必须确保在本次调用之前 `entry` 已被压入（push）到 `self` 中，并且
    /// 自那以后没有发生移动。
    pub unsafe fn remove(&mut self, entry: &mut UnsafeListEntry<T>) {
        rtassert!(!self.is_empty());
        // 操作前(BEFORE)：
        //     /----\ next ---> /-----\ next ---> /----\
        // ... |prev|           |entry|           |next| ...
        //     \----/ <--- prev \-----/ <--- prev \----/
        //
        // 操作后(AFTER)：
        //     /----\ next ---> /----\
        // ... |prev|           |next| ...
        //     \----/ <--- prev \----/
        let mut prev = entry.prev;
        let mut next = entry.next;
        // SAFETY: `prev` 和 `next` 必须满足可变引用的所有要求。entry
        unsafe { prev.as_mut() }.next = next;
        unsafe { next.as_mut() }.prev = prev;
        entry.next = NonNull::dangling();
        entry.prev = NonNull::dangling();
    }
}
