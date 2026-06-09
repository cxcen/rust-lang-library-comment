use crate::marker::{Destruct, PhantomData};
use crate::mem::{ManuallyDrop, SizedTypeProperties, conjure_zst};
use crate::ptr::{NonNull, drop_in_place, from_raw_parts_mut, null_mut};

impl<'l, 'f, T, U, const N: usize, F: FnMut(T) -> U> Drain<'l, 'f, T, N, F> {
    /// 返回一个可在 const 上下文中按索引取出数组元素的函数对象。
    /// 这种实现比普通迭代器更容易被优化,也能 const 化。它相当于把“拥有数组的 guard”
    /// 与“迭代器”合在一起:结构体本身实现 const fn,行为上有点像实现了
    /// `UncheckedIterator` 的 `array::Iter`。
    /// 调用方真正允许调用的只有 `next()` 语义对应的路径;其他用法基本都会破坏不变量,
    /// 因此构造函数是 unsafe。已经移出的元素不会再被 drop。
    /// 本结构实际上也不会把数组按值存起来。
    ///
    /// # 安全性(Safety）
    ///
    /// 最多只能调用 `N` 次,并且调用方不得再 drop 原数组。
    // FIXME(const-hack): 这是为了表达 `let guard = Guard(array); |i| f(guard[i])` 而采用的权宜写法。
    #[rustc_const_unstable(feature = "array_try_map", issue = "79711")]
    pub(super) const unsafe fn new(array: &'l mut ManuallyDrop<[T; N]>, f: &'f mut F) -> Self {
        // 不 drop 原数组;这里把“所有权”概念上转移给 Self。
        let ptr: NonNull<T> = NonNull::from_mut(array).cast();
        // SAFETY:
        // 起始指针加上数组长度得到尾后指针。`end` 永远不会被解引用,只用于与 `ptr`
        // 做直接指针相等比较,判断 drainer 是否已经耗尽。
        unsafe {
            let end = if T::IS_ZST { null_mut() } else { ptr.as_ptr().add(N) };
            Self { ptr, end, f, l: PhantomData }
        }
    }
}

/// 见 [`Drain::new`];这是供 const-hack 权宜路径使用的假迭代器。
#[rustc_const_unstable(feature = "array_try_map", issue = "79711")]
#[unstable(feature = "array_try_map", issue = "79711")]
pub(super) struct Drain<'l, 'f, T, const N: usize, F> {
    // FIXME(const-hack): 本质上这是一个 slice::IterMut<'static>,可行时应替换。
    /// 指向下一个要返回的元素;若 drainer 为空,则指向尾后位置。
    ///
    /// 对所有 ZST 元素都会使用这个地址,且它不会改变。
    /// 因为我们概念上“拥有”这个数组,无需额外存储生命周期。
    ptr: NonNull<T>,
    /// 对非 ZST,这是指向尾后元素的非空指针。
    /// 对 ZST,这是空指针。
    end: *mut T,

    f: &'f mut F,
    l: PhantomData<&'l mut [T; N]>,
}

#[rustc_const_unstable(feature = "array_try_map", issue = "79711")]
#[unstable(feature = "array_try_map", issue = "79711")]
impl<T, U, const N: usize, F> const FnOnce<(usize,)> for &mut Drain<'_, '_, T, N, F>
where
    F: [const] FnMut(T) -> U,
{
    type Output = U;

    /// 该实现本身没有实际用途,只是满足调用 trait 的形状。
    extern "rust-call" fn call_once(mut self, args: (usize,)) -> Self::Output {
        self.call_mut(args)
    }
}
#[rustc_const_unstable(feature = "array_try_map", issue = "79711")]
#[unstable(feature = "array_try_map", issue = "79711")]
impl<T, U, const N: usize, F> const FnMut<(usize,)> for &mut Drain<'_, '_, T, N, F>
where
    F: [const] FnMut(T) -> U,
{
    // FIXME(const-hack): 理想情况下这里应当是一个 unsafe fn `next()`,使用时写成 `|_| unsafe { drain.next() }`。
    extern "rust-call" fn call_mut(
        &mut self,
        (_ /* ignore argument */,): (usize,),
    ) -> Self::Output {
        if T::IS_ZST {
            // 调用超过 N 次是 UB,因此在合法调用范围内返回 ZST 是有效的。
            // SAFETY: `T` 是 ZST,可凭类型构造一个值而不读取实际存储。
            (self.f)(unsafe { conjure_zst::<T>() })
        } else {
            // 先递增再移动;若 `f` panic,Drop 会清理剩余元素。
            let p = self.ptr;
            // SAFETY: 调用方保证最多调用 N 次(见 `Drain::new`)。
            self.ptr = unsafe { self.ptr.add(1) };
            // SAFETY: 该元素仍属于本 Drain 的活跃区,允许按值移出。
            (self.f)(unsafe { p.read() })
        }
    }
}
#[rustc_const_unstable(feature = "array_try_map", issue = "79711")]
#[unstable(feature = "array_try_map", issue = "79711")]
impl<T: [const] Destruct, const N: usize, F> const Drop for Drain<'_, '_, T, N, F> {
    fn drop(&mut self) {
        if !T::IS_ZST {
            // SAFETY: 合法使用下最多读取 N 个元素,`ptr..end` 仍是尚未移出的尾部。
            let slice = unsafe {
                from_raw_parts_mut::<[T]>(
                    self.ptr.as_ptr(),
                    // SAFETY: `start <= end`
                    self.end.offset_from_unsigned(self.ptr.as_ptr()),
                )
            };

            // SAFETY: 根据类型不变量,这些元素仍由本 Drain 拥有,因此允许全部 drop。
            unsafe { drop_in_place(slice) }
        }
    }
}
