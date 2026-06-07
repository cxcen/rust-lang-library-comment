/// 创建一个新闭包，它返回一个迭代器；该迭代器每次迭代都会把给定 generator 推进到
/// 下一个 `yield` 语句。
///
/// 类似 [`iter::from_fn`]，但允许任意控制流。
///
/// [`iter::from_fn`]: crate::iter::from_fn
///
/// # 示例
///
/// ```
/// #![feature(iter_macro, coroutines)]
///
/// let it = std::iter::iter!{|| {
///     yield 1;
///     yield 2;
///     yield 3;
/// } }();
/// let v: Vec<_> = it.collect();
/// assert_eq!(v, [1, 2, 3]);
/// ```
#[unstable(feature = "iter_macro", issue = "142269", reason = "generators are unstable")]
#[allow_internal_unstable(coroutines, iter_from_coroutine)]
#[rustc_builtin_macro]
pub macro iter($($t:tt)*) {
    /* compiler-builtin */
}
