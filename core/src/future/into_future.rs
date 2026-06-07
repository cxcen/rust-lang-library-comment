use crate::future::Future;

/// 转换为 `Future`。
///
/// 为某个类型实现 `IntoFuture`,就定义了它如何被转换成一个 future。
///
/// # `.await` 脱糖
///
/// `.await` 关键字会先脱糖成对 `IntoFuture::into_future` 的调用,然后再把得到的 future
/// 轮询到完成。`IntoFuture` 对所有 `T: Future` 都有实现,因此 `into_future` 方法在所有
/// future 上都可用。
///
/// ```no_run
/// use std::future::IntoFuture;
///
/// # async fn foo() {
/// let v = async { "meow" };
/// let mut fut = v.into_future();
/// assert_eq!("meow", fut.await);
/// # }
/// ```
///
/// # 异步构造器(Async builders)
///
/// 手动实现 future 时,常常要在为某类型实现 `Future` 还是 `IntoFuture` 之间做选择。大多数
/// 情况下实现 `Future` 是更好的选择。但当你实现“异步构造器”类型——即允许其值在被 `.await`
/// 之前被多次修改的类型——时,实现 `IntoFuture` 最为有用。
///
/// ```rust
/// use std::future::{ready, Ready, IntoFuture};
///
/// /// 最终把两个数相乘
/// pub struct Multiply {
///     num: u16,
///     factor: u16,
/// }
///
/// impl Multiply {
///     /// 构造一个新的 `Multiply` 实例。
///     pub fn new(num: u16, factor: u16) -> Self {
///         Self { num, factor }
///     }
///
///     /// 设置要被乘的数。
///     pub fn number(mut self, num: u16) -> Self {
///         self.num = num;
///         self
///     }
///
///     /// 设置用来相乘的因子。
///     pub fn factor(mut self, factor: u16) -> Self {
///         self.factor = factor;
///         self
///     }
/// }
///
/// impl IntoFuture for Multiply {
///     type Output = u16;
///     type IntoFuture = Ready<Self::Output>;
///
///     fn into_future(self) -> Self::IntoFuture {
///         ready(self.num * self.factor)
///     }
/// }
///
/// // 注意:Rust 目前还没有内置的 `async fn main`,该功能当前只存在于生态库中。
/// async fn run() {
///     let num = Multiply::new(0, 0)  // 把构造器初始化为 number: 0、factor: 0
///         .number(2)                 // 把 number 改为 2
///         .factor(2)                 // 把 factor 改为 2
///         .await;                    // 转换为 future 并 .await
///
///     assert_eq!(num, 4);
/// }
/// ```
///
/// # 在 trait 约束中使用
///
/// 在 trait 约束中使用 `IntoFuture`,可以让一个函数同时对 `Future` 和 `IntoFuture` 泛型。
/// 这对函数的使用者很方便:调用时无需额外手动调用 `IntoFuture::into_future` 来得到一个
/// `Future` 实例:
///
/// ```rust
/// use std::future::IntoFuture;
///
/// /// 把一个 future 的输出转换为字符串。
/// async fn fut_to_string<Fut>(fut: Fut) -> String
/// where
///     Fut: IntoFuture,
///     Fut::Output: std::fmt::Debug,
/// {
///     format!("{:?}", fut.await)
/// }
/// ```
#[stable(feature = "into_future", since = "1.64.0")]
#[rustc_diagnostic_item = "IntoFuture"]
#[diagnostic::on_unimplemented(
    label = "`{Self}` is not a future",
    message = "`{Self}` is not a future",
    note = "{Self} must be a future or must implement `IntoFuture` to be awaited"
)]
pub trait IntoFuture {
    /// future 完成时将产生的输出类型。
    #[stable(feature = "into_future", since = "1.64.0")]
    type Output;

    /// 我们要把它转换成哪种 future?
    #[stable(feature = "into_future", since = "1.64.0")]
    type IntoFuture: Future<Output = Self::Output>;

    /// 从一个值创建 future。
    ///
    /// # 示例
    ///
    /// 基本用法:
    ///
    /// ```no_run
    /// use std::future::IntoFuture;
    ///
    /// # async fn foo() {
    /// let v = async { "meow" };
    /// let mut fut = v.into_future();
    /// assert_eq!("meow", fut.await);
    /// # }
    /// ```
    #[stable(feature = "into_future", since = "1.64.0")]
    #[lang = "into_future"]
    fn into_future(self) -> Self::IntoFuture;
}

#[stable(feature = "into_future", since = "1.64.0")]
impl<F: Future> IntoFuture for F {
    type Output = F::Output;
    type IntoFuture = F;

    fn into_future(self) -> Self::IntoFuture {
        self
    }
}
