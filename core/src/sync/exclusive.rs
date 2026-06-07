//! 定义 [`Exclusive`]:把任意 `T` 包装成无条件 `Sync` 的“仅独占访问”容器。

use core::clone::TrivialClone;
use core::cmp::Ordering;
use core::fmt;
use core::future::Future;
use core::hash::{Hash, Hasher};
use core::marker::{StructuralPartialEq, Tuple};
use core::ops::{Coroutine, CoroutineState};
use core::pin::Pin;
use core::task::{Context, Poll};

/// `Exclusive` 只对外暴露内部值的**可变**(即**独占**,exclusive)访问;只有当内部
/// 值本身实现了 [`Sync`] 时,才额外允许**不可变**(即**共享**,shared)访问。
///
/// 这看似无用,却让 `Exclusive` 能**无条件**实现 `Sync`。`Sync` 的安全契约要求:
/// 一个类型要 `Sync`,必须保证 `&Exclusive` 跨线程共享是健全的。而设计上,当 `T`
/// 不是 `Sync` 时,`&Exclusive<T>` 完全没有任何可用 API——既然 `&Exclusive` 拿不到
/// 内部值的共享引用,共享它就不会引发数据竞争,因此无害、内存安全。换言之,这里的
/// `unsafe impl Sync` 不是“假装安全”,其健全性恰恰来自“非 `Sync` 时无任何共享 API”
/// 这一刻意设计。
///
/// 像 [`Future`] 这类结构往往只需独占访问即可推进,它们通常是 `Send` 但不是 `Sync`。
/// 用 `Exclusive` 包装后,即可向编译器表明“它在实际使用中是 `Sync` 的”,从而让包含
/// 它的外层结构也能成为 `Sync`。
///
/// ## 示例
///
/// 直接持有一个非 `Sync` 的 future 会让外层结构无法 `Sync`:
///
/// ```compile_fail
/// use core::cell::Cell;
///
/// async fn other() {}
/// fn assert_sync<T: Sync>(t: T) {}
/// struct State<F> {
///     future: F
/// }
///
/// assert_sync(State {
///     future: async {
///         let cell = Cell::new(1);
///         let cell_ref = &cell;
///         other().await;
///         let value = cell_ref.get();
///     }
/// });
/// ```
///
/// 用 `Exclusive` 包装后,既保留了 future 的全部功能,又让结构变为 `Sync`:
///
/// ```
/// #![feature(exclusive_wrapper)]
/// use core::cell::Cell;
/// use core::sync::Exclusive;
///
/// async fn other() {}
/// fn assert_sync<T: Sync>(t: T) {}
/// struct State<F> {
///     future: Exclusive<F>
/// }
///
/// assert_sync(State {
///     future: Exclusive::new(async {
///         let cell = Cell::new(1);
///         let cell_ref = &cell;
///         other().await;
///         let value = cell_ref.get();
///     })
/// });
/// ```
///
/// ## 与互斥锁的类比
///
/// 某种意义上,`Exclusive` 可视为**编译期**版本的互斥锁:借用检查器保证任意值
/// 同一时刻只能存在一个 `&mut`。这与“`&` 和 `&mut` 引用合起来可视为编译期版本的
/// 读写锁”是同一思路。
#[unstable(feature = "exclusive_wrapper", issue = "98407")]
#[doc(alias = "SyncWrapper")]
#[doc(alias = "SyncCell")]
#[doc(alias = "Unique")]
// `Exclusive` 不能 derive `PartialOrd`、`Clone` 等实现:它们会通过 `&` 访问内部值,
// 而那会违反上面 `unsafe impl Sync` 的安全前提(非 `Sync` 的 `T` 不得被共享访问)。
#[derive(Default)]
#[repr(transparent)]
pub struct Exclusive<T: ?Sized> {
    inner: T,
}

// 理由见 `Exclusive` 的文档:非 `Sync` 时 `&Exclusive<T>` 没有任何共享 API,因此跨线程
// 共享无害,这个无条件的 `unsafe impl Sync` 是健全的。
#[unstable(feature = "exclusive_wrapper", issue = "98407")]
unsafe impl<T: ?Sized> Sync for Exclusive<T> {}

#[unstable(feature = "exclusive_wrapper", issue = "98407")]
impl<T: ?Sized> fmt::Debug for Exclusive<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        f.debug_struct("Exclusive").finish_non_exhaustive()
    }
}

impl<T: Sized> Exclusive<T> {
    /// 把一个值包装进 `Exclusive`。
    #[unstable(feature = "exclusive_wrapper", issue = "98407")]
    #[must_use]
    #[inline]
    pub const fn new(t: T) -> Self {
        Self { inner: t }
    }

    /// 取出 `Exclusive` 内部持有的值(按值消费,转移所有权)。
    #[unstable(feature = "exclusive_wrapper", issue = "98407")]
    #[rustc_const_unstable(feature = "exclusive_wrapper", issue = "98407")]
    #[must_use]
    #[inline]
    pub const fn into_inner(self) -> T {
        self.inner
    }
}

impl<T: ?Sized> Exclusive<T> {
    /// 获取对内部值的独占(可变)访问。
    ///
    /// 这是 `&mut Exclusive<T>` 唯一对外暴露内部值的途径,与 `Sync` 实现一致:
    /// 拿到 `&mut self` 即代表当前线程独占该值,因此返回 `&mut T` 不破坏任何约束。
    #[unstable(feature = "exclusive_wrapper", issue = "98407")]
    #[must_use]
    #[inline]
    pub const fn get_mut(&mut self) -> &mut T {
        &mut self.inner
    }

    /// 获取对内部值的“已固定(pinned)的独占”访问。
    ///
    /// `Exclusive` 被视为对内部值做**结构化固定**(structurally pin):未固定的
    /// `Exclusive` 可产出未固定的内部访问,而已固定的 `Exclusive` 只能产出已固定的
    /// 内部访问。这保证了 `Pin` 的不可移动语义能透过包装层正确传递给内部值。
    #[unstable(feature = "exclusive_wrapper", issue = "98407")]
    #[must_use]
    #[inline]
    pub const fn get_pin_mut(self: Pin<&mut Self>) -> Pin<&mut T> {
        // SAFETY: 只有当 `Exclusive` 自身未固定时才可能产出 `&mut T`;此处已持有
        // `Pin<&mut Self>`,内部值的固定性与外层一致,故重新包成 `Pin` 是健全的。
        // `Pin::map_unchecked_mut` 不是 const fn,因此这里手动完成同等转换。
        unsafe { Pin::new_unchecked(&mut self.get_unchecked_mut().inner) }
    }

    /// 直接由 `&mut T` 构造 `&mut Exclusive<T>`,免去先用 [`Exclusive::new`] 包装的步骤。
    #[unstable(feature = "exclusive_wrapper", issue = "98407")]
    #[must_use]
    #[inline]
    pub const fn from_mut(r: &'_ mut T) -> &'_ mut Exclusive<T> {
        // SAFETY: `repr` 至少是 `C`(此处为 `transparent`),故两者内存布局相同;且
        // `Exclusive` 的全部性质都与 `&mut` 访问无关,因此把 `&mut T` 重解释为
        // `&mut Exclusive<T>` 是健全的。
        unsafe { &mut *(r as *mut T as *mut Exclusive<T>) }
    }

    /// 由 `Pin<&mut T>` 构造 `Pin<&mut Exclusive<T>>`,免去先用 [`Exclusive::new`]
    /// 包装的步骤。
    #[unstable(feature = "exclusive_wrapper", issue = "98407")]
    #[must_use]
    #[inline]
    pub const fn from_pin_mut(r: Pin<&'_ mut T>) -> Pin<&'_ mut Exclusive<T>> {
        // SAFETY: 只有当 `Exclusive` 自身未固定时才可能产出 `&mut T`;固定性在
        // `T` 与 `Exclusive<T>` 之间保持一致,故重新包成 `Pin` 是健全的。
        // `Pin::map_unchecked_mut` 不是 const fn,因此这里手动完成同等转换。
        unsafe { Pin::new_unchecked(Self::from_mut(r.get_unchecked_mut())) }
    }
}

#[unstable(feature = "exclusive_wrapper", issue = "98407")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl<T> const From<T> for Exclusive<T> {
    #[inline]
    fn from(t: T) -> Self {
        Self::new(t)
    }
}

#[unstable(feature = "exclusive_wrapper", issue = "98407")]
impl<F, Args> FnOnce<Args> for Exclusive<F>
where
    F: FnOnce<Args>,
    Args: Tuple,
{
    type Output = F::Output;

    extern "rust-call" fn call_once(self, args: Args) -> Self::Output {
        self.into_inner().call_once(args)
    }
}

#[unstable(feature = "exclusive_wrapper", issue = "98407")]
impl<F, Args> FnMut<Args> for Exclusive<F>
where
    F: FnMut<Args>,
    Args: Tuple,
{
    extern "rust-call" fn call_mut(&mut self, args: Args) -> Self::Output {
        self.get_mut().call_mut(args)
    }
}

#[unstable(feature = "exclusive_wrapper", issue = "98407")]
impl<F, Args> Fn<Args> for Exclusive<F>
where
    F: Sync + Fn<Args>,
    Args: Tuple,
{
    extern "rust-call" fn call(&self, args: Args) -> Self::Output {
        self.as_ref().call(args)
    }
}

#[unstable(feature = "exclusive_wrapper", issue = "98407")]
impl<T> Future for Exclusive<T>
where
    T: Future + ?Sized,
{
    type Output = T::Output;

    #[inline]
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.get_pin_mut().poll(cx)
    }
}

#[unstable(feature = "coroutine_trait", issue = "43122")] // also #98407
impl<R, G> Coroutine<R> for Exclusive<G>
where
    G: Coroutine<R> + ?Sized,
{
    type Yield = G::Yield;
    type Return = G::Return;

    #[inline]
    fn resume(self: Pin<&mut Self>, arg: R) -> CoroutineState<Self::Yield, Self::Return> {
        G::resume(self.get_pin_mut(), arg)
    }
}

#[unstable(feature = "exclusive_wrapper", issue = "98407")]
impl<T> AsRef<T> for Exclusive<T>
where
    T: Sync + ?Sized,
{
    #[inline]
    fn as_ref(&self) -> &T {
        &self.inner
    }
}

#[unstable(feature = "exclusive_wrapper", issue = "98407")]
impl<T> Clone for Exclusive<T>
where
    T: Sync + Clone,
{
    #[inline]
    fn clone(&self) -> Self {
        Self { inner: self.inner.clone() }
    }
}

#[doc(hidden)]
#[unstable(feature = "trivial_clone", issue = "none")]
unsafe impl<T> TrivialClone for Exclusive<T> where T: Sync + TrivialClone {}

#[unstable(feature = "exclusive_wrapper", issue = "98407")]
impl<T> Copy for Exclusive<T> where T: Sync + Copy {}

#[unstable(feature = "exclusive_wrapper", issue = "98407")]
impl<T, U> PartialEq<Exclusive<U>> for Exclusive<T>
where
    T: Sync + PartialEq<U> + ?Sized,
    U: Sync + ?Sized,
{
    #[inline]
    fn eq(&self, other: &Exclusive<U>) -> bool {
        self.inner == other.inner
    }
}

#[unstable(feature = "exclusive_wrapper", issue = "98407")]
impl<T> StructuralPartialEq for Exclusive<T> where T: Sync + StructuralPartialEq + ?Sized {}

#[unstable(feature = "exclusive_wrapper", issue = "98407")]
impl<T> Eq for Exclusive<T> where T: Sync + Eq + ?Sized {}

#[unstable(feature = "exclusive_wrapper", issue = "98407")]
impl<T> Hash for Exclusive<T>
where
    T: Sync + Hash + ?Sized,
{
    #[inline]
    fn hash<H: Hasher>(&self, state: &mut H) {
        Hash::hash(&self.inner, state)
    }
}

#[unstable(feature = "exclusive_wrapper", issue = "98407")]
impl<T, U> PartialOrd<Exclusive<U>> for Exclusive<T>
where
    T: Sync + PartialOrd<U> + ?Sized,
    U: Sync + ?Sized,
{
    #[inline]
    fn partial_cmp(&self, other: &Exclusive<U>) -> Option<Ordering> {
        self.inner.partial_cmp(&other.inner)
    }
}

#[unstable(feature = "exclusive_wrapper", issue = "98407")]
impl<T> Ord for Exclusive<T>
where
    T: Sync + Ord + ?Sized,
{
    #[inline]
    fn cmp(&self, other: &Self) -> Ordering {
        self.inner.cmp(&other.inner)
    }
}
