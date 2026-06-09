use crate::fmt::{self, Debug};
use crate::marker::Destruct;
use crate::mem::ManuallyDrop;
use crate::ops::{Deref, DerefMut};

/// 包装一个值，并在其被 drop 时运行一个闭包。
///
/// 这适合用来内联地、快速地创建析构逻辑（destructor）。
///
/// # 示例
///
/// ```rust
/// # #![allow(unused)]
/// #![feature(drop_guard)]
///
/// use std::mem::DropGuard;
///
/// {
///     // 围绕一个字符串创建一个新的 guard，
///     // 它会在被 drop 时打印自己的值。
///     let s = String::from("Chashu likes tuna");
///     let mut s = DropGuard::new(s, |s| println!("{s}"));
///
///     // 修改 guard 内部所包含的字符串。
///     s.push_str("!!!");
///
///     // guard 会在此处被 drop，打印出：
///     // "Chashu likes tuna!!!"
/// }
/// ```
#[unstable(feature = "drop_guard", issue = "144426")]
#[doc(alias = "ScopeGuard")]
#[doc(alias = "defer")]
pub struct DropGuard<T, F>
where
    F: FnOnce(T),
{
    inner: ManuallyDrop<T>,
    f: ManuallyDrop<F>,
}

impl<T, F> DropGuard<T, F>
where
    F: FnOnce(T),
{
    /// 创建一个新的 `DropGuard` 实例。
    ///
    /// # 示例
    ///
    /// ```rust
    /// # #![allow(unused)]
    /// #![feature(drop_guard)]
    ///
    /// use std::mem::DropGuard;
    ///
    /// let value = String::from("Chashu likes tuna");
    /// let guard = DropGuard::new(value, |s| println!("{s}"));
    /// ```
    #[unstable(feature = "drop_guard", issue = "144426")]
    #[must_use]
    pub const fn new(inner: T, f: F) -> Self {
        Self { inner: ManuallyDrop::new(inner), f: ManuallyDrop::new(f) }
    }

    /// 消耗（consume）该 `DropGuard`，返回其包装的值。
    ///
    /// 这不会执行闭包。通常更推荐调用此函数而不是 `mem::forget`，
    /// 因为它会返回存储的值，并 drop 闭包所捕获的变量，
    /// 而不是泄漏这些变量所拥有的资源。
    ///
    /// # 示例
    ///
    /// ```rust
    /// # #![allow(unused)]
    /// #![feature(drop_guard)]
    ///
    /// use std::mem::DropGuard;
    ///
    /// let value = String::from("Nori likes chicken");
    /// let guard = DropGuard::new(value, |s| println!("{s}"));
    /// assert_eq!(DropGuard::dismiss(guard), "Nori likes chicken");
    /// ```
    #[unstable(feature = "drop_guard", issue = "144426")]
    #[rustc_const_unstable(feature = "const_drop_guard", issue = "none")]
    #[inline]
    pub const fn dismiss(guard: Self) -> T
    where
        F: [const] Destruct,
    {
        // 首先确保 drop 这个 guard 时不会触发它自己的析构逻辑。
        let mut guard = ManuallyDrop::new(guard);

        // 接着我们手动从 guard 中读出存储的值。
        //
        // SAFETY: 这是安全的，因为我们已经取得了 guard 的所有权。
        let value = unsafe { ManuallyDrop::take(&mut guard.inner) };

        // 最后我们 drop 存储的闭包。我们在读出值*之后*才这样做，
        // 这样即使闭包的 `drop` 函数发生 panic，
        // 展开（unwinding）过程仍然会尝试 drop 那个值。
        //
        // SAFETY: 这是安全的，因为我们已经取得了 guard 的所有权。
        unsafe { ManuallyDrop::drop(&mut guard.f) };
        value
    }
}

#[unstable(feature = "drop_guard", issue = "144426")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl<T, F> const Deref for DropGuard<T, F>
where
    F: FnOnce(T),
{
    type Target = T;

    fn deref(&self) -> &T {
        &*self.inner
    }
}

#[unstable(feature = "drop_guard", issue = "144426")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl<T, F> const DerefMut for DropGuard<T, F>
where
    F: FnOnce(T),
{
    fn deref_mut(&mut self) -> &mut T {
        &mut *self.inner
    }
}

#[unstable(feature = "drop_guard", issue = "144426")]
#[rustc_const_unstable(feature = "const_drop_guard", issue = "none")]
impl<T, F> const Drop for DropGuard<T, F>
where
    F: [const] FnOnce(T),
{
    fn drop(&mut self) {
        // SAFETY: `DropGuard` 正处于被 drop 的过程中。
        let inner = unsafe { ManuallyDrop::take(&mut self.inner) };

        // SAFETY: `DropGuard` 正处于被 drop 的过程中。
        let f = unsafe { ManuallyDrop::take(&mut self.f) };

        f(inner);
    }
}

#[unstable(feature = "drop_guard", issue = "144426")]
impl<T, F> Debug for DropGuard<T, F>
where
    T: Debug,
    F: FnOnce(T),
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}
