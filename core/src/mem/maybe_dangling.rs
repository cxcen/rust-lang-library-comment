#![unstable(feature = "maybe_dangling", issue = "118166")]

use crate::{mem, ptr};

/// 允许其包装的[引用][references]和[box][boxes]悬垂（dangle）。
///
/// <section class="warning">
/// 此类型尚未被正确实现，因此下面的文档目前也并不准确。
/// </section>
///
/// 也就是说，如果一个引用（或一个 `Box`）被包装在 `MaybeDangling` 中
/// （包括它位于某个被 `MaybeDangling` 包装的复合类型的（嵌套）字段中时），
/// 那么它就无需遵守指针别名（aliasing）规则，也无需保证可解引用。
///
/// 当某个值在持有它的函数仍在执行期间就可能变为悬垂时（尤其是在并发代码里），
/// 这会很有用。作为一个略显荒谬的例子，考虑下面这段代码：
///
/// ```rust,no_run
/// #![feature(box_as_ptr)]
/// # use std::alloc::{dealloc, Layout};
/// # use std::mem;
///
/// let mut boxed = Box::new(0_u32);
/// let ptr = Box::as_mut_ptr(&mut boxed);
///
/// // SAFETY: 该指针来自一个 box，因此之前已被分配；`box` 之后不再被使用
/// unsafe { dealloc(ptr.cast(), Layout::new::<u32>()) };
///
/// mem::forget(boxed); // <-- 这里是 UB！
/// ```
///
/// 尽管这个 `Box` 的析构逻辑没有被运行（因此我们不存在 double free 的 bug），
/// 这段代码仍然是 UB。原因在于：将 `boxed` move 进 `forget` 时，它的有效性不变量
/// （validity invariants）会被断言，而由于此 `Box` 已悬垂，这就触发了 UB。
/// 因此那条 safety 注释其实是错的——把 `boxed` 变量作为 `forget` 调用的一部分进行 move，
/// 这本身*就是*一次使用（use）。
///
/// 要修复它，我们可以使用 `MaybeDangling`：
///
// FIXME: 一旦相关语义被真正实现，就移除 `no_run`
/// ```rust,no_run
/// #![feature(maybe_dangling, box_as_ptr)]
/// # use std::alloc::{dealloc, Layout};
/// # use std::mem::{self, MaybeDangling};
///
/// let mut boxed = MaybeDangling::new(Box::new(0_u32));
/// let ptr = Box::as_mut_ptr(boxed.as_mut());
///
/// // SAFETY: 该指针来自一个 box，因此之前已被分配；`box` 之后不再被使用
/// unsafe { dealloc(ptr.cast(), Layout::new::<u32>()) };
///
/// mem::forget(boxed); // <-- 这里就 OK 了！
/// ```
///
/// 注意，对于被包装的类型，其位模式（bit pattern）仍然必须有效。也就是说，[引用][references]
///（以及[box][boxes]）仍然必须对齐且非空。
///
/// 另外注意，安全代码仍然可以假定 `MaybeDangling` 中的内部值**不是**悬垂的——
/// 像 [`as_ref`] 和 [`into_inner`] 这样的函数是安全的。把一个悬垂的引用以 `MaybeDangling`
/// 的形式返回给安全代码是不健全（unsound）的。然而，在你自己的代码内部持有这样的值*是*健全的——
/// 而且若没有这个类型就无法做到这一点。注意其他类型也可以使用此类型从而获得同样的效果；
/// 尤其是，[`ManuallyDrop`] 会使用 `MaybeDangling`。
///
/// 注意 `MaybeDangling` 并不会阻止 drop 被运行，而这在 drop 过程观察到一个悬垂值时可能导致 UB。
/// 如果你需要阻止 drop 被运行，请改用 [`ManuallyDrop`]。
///
/// [references]: prim@reference
/// [boxes]: ../../std/boxed/struct.Box.html
/// [`into_inner`]: MaybeDangling::into_inner
/// [`as_ref`]: MaybeDangling::as_ref
/// [`ManuallyDrop`]: crate::mem::ManuallyDrop
#[repr(transparent)]
#[rustc_pub_transparent]
#[derive(Debug, Copy, Clone, Default)]
pub struct MaybeDangling<P: ?Sized>(P);

impl<P: ?Sized> MaybeDangling<P> {
    /// 把一个值包装进 `MaybeDangling`，允许它悬垂。
    pub const fn new(x: P) -> Self
    where
        P: Sized,
    {
        MaybeDangling(x)
    }

    /// 返回内部值的一个引用。
    ///
    /// 注意，如果此时内部值正处于悬垂状态，这就是 UB。
    pub const fn as_ref(&self) -> &P {
        &self.0
    }

    /// 返回内部值的一个可变引用。
    ///
    /// 注意，如果此时内部值正处于悬垂状态，这就是 UB。
    pub const fn as_mut(&mut self) -> &mut P {
        &mut self.0
    }

    /// 从 `MaybeDangling` 容器中提取出值。
    ///
    /// 注意，如果此时内部值正处于悬垂状态，这就是 UB。
    pub const fn into_inner(self) -> P
    where
        P: Sized,
    {
        // FIXME: 当 const 检查器能够推断出 `self` 实际上并未被 drop 时，把这里替换为 `self.0`
        // SAFETY: 这等价于 `self.0`
        let x = unsafe { ptr::read(&self.0) };
        mem::forget(self);
        x
    }
}
