use super::TrivialClone;
use crate::mem::{self, MaybeUninit};
use crate::ptr;

/// 由 CloneToUninit 使用的私有特化(specialization)trait,做法依据
/// [开发指南](https://std-dev-guide.rust-lang.org/policy/specialization.html)。
pub(super) unsafe trait CopySpec: Clone {
    unsafe fn clone_one(src: &Self, dst: *mut Self);
    unsafe fn clone_slice(src: &[Self], dst: *mut [Self]);
}

unsafe impl<T: Clone> CopySpec for T {
    #[inline]
    default unsafe fn clone_one(src: &Self, dst: *mut Self) {
        // SAFETY:clone_to_uninit() 的安全条件是 ptr::write() 安全条件的超集。
        unsafe {
            // 我们希望优化器能想办法原地(in-place)创建克隆出来的值,从而
            // 完全省去把它存到栈上、再复制到目标处这两步。
            ptr::write(dst, src.clone());
        }
    }

    #[inline]
    #[cfg_attr(debug_assertions, track_caller)]
    default unsafe fn clone_slice(src: &[Self], dst: *mut [Self]) {
        let len = src.len();
        // 这是最容易犯的错误,所以用一个 debug assertion 检查它。
        debug_assert_eq!(
            len,
            dst.len(),
            "clone_to_uninit() source and destination must have equal lengths",
        );

        // SAFETY:产生的 `&mut` 是有效的,因为:
        // * 调用者有义务提供一个对写入有效的指针。
        // * 被指向的所有字节都在 MaybeUninit 中,所以我们不关心这块内存的
        //   初始化状态。
        let uninit_ref = unsafe { &mut *(dst as *mut [MaybeUninit<T>]) };

        // 复制各个元素
        let mut initializing = InitializingSlice::from_fully_uninit(uninit_ref);
        for element_ref in src {
            // 如果 clone() 发生 panic,`initializing` 会负责清理。
            initializing.push(element_ref.clone());
        }
        // 如果执行到这里,说明整个切片都已被初始化,我们也就履行了对调用者的
        // 责任。通过 forget 掉它来解除清理守卫(cleanup guard)。
        mem::forget(initializing);
    }
}

// 针对那些不只是 [`Clone`]、而且是 [`TrivialClone`] 的类型的特化实现,
// 这些类型因此可以被按位复制。
unsafe impl<T: TrivialClone> CopySpec for T {
    #[inline]
    unsafe fn clone_one(src: &Self, dst: *mut Self) {
        // SAFETY:clone_to_uninit() 的安全条件是 ptr::copy_nonoverlapping()
        // 安全条件的超集。
        unsafe {
            ptr::copy_nonoverlapping(src, dst, 1);
        }
    }

    #[inline]
    #[cfg_attr(debug_assertions, track_caller)]
    unsafe fn clone_slice(src: &[Self], dst: *mut [Self]) {
        let len = src.len();
        // 这是最容易犯的错误,所以用一个 debug assertion 检查它。
        debug_assert_eq!(
            len,
            dst.len(),
            "clone_to_uninit() source and destination must have equal lengths",
        );

        // SAFETY:clone_to_uninit() 的安全条件是 ptr::copy_nonoverlapping()
        // 安全条件的超集。
        unsafe {
            ptr::copy_nonoverlapping(src.as_ptr(), dst.as_mut_ptr(), len);
        }
    }
}

/// 对一组存放在非自有的 `[MaybeUninit<T>]` 中的值的所有权,其中部分元素
/// 尚未被初始化。它有点像一个不拥有自身内存分配的 `Vec`。它的职责是:在
/// 栈展开(unwind)时通过 drop 掉那些 *已被* 初始化的值来进行清理——除非
/// 被 forget 掉而解除武装(disarmed)。
///
/// 这是 `impl<T: Clone> CloneToUninit for [T]` 的一个辅助工具。
struct InitializingSlice<'a, T> {
    data: &'a mut [MaybeUninit<T>],
    /// `*self.data` 中已被初始化的元素个数。
    initialized_len: usize,
}

impl<'a, T> InitializingSlice<'a, T> {
    #[inline]
    fn from_fully_uninit(data: &'a mut [MaybeUninit<T>]) -> Self {
        Self { data, initialized_len: 0 }
    }

    /// 向切片已初始化部分的末尾压入一个值。
    ///
    /// # Panics
    ///
    /// 如果切片已经完全初始化,则会 panic。
    #[inline]
    fn push(&mut self, value: T) {
        MaybeUninit::write(&mut self.data[self.initialized_len], value);
        self.initialized_len += 1;
    }
}

impl<'a, T> Drop for InitializingSlice<'a, T> {
    #[cold] // 只会在栈展开时被调用
    fn drop(&mut self) {
        // SAFETY:
        // * 该指针是有效的,因为它是从一个可变引用得来的
        // * 作为本类型的一个不变量,`initialized_len` 统计的是已初始化的元素
        //   个数,因此被指向的每一个元素都已被初始化,可以被 drop。
        unsafe { self.data[..self.initialized_len].assume_init_drop() };
    }
}
