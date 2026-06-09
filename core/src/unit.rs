/// 将 iterator 中所有 unit 项折叠成一个值。
///
/// 它与更高层抽象结合时更有用，例如收集到 `Result<(), E>`，
/// 其中你只关心错误：
///
/// ```
/// use std::io::*;
/// let data = vec![1, 2, 3, 4, 5];
/// let res: Result<()> = data.iter()
///     .map(|x| writeln!(stdout(), "{x}"))
///     .collect();
/// assert!(res.is_ok());
/// ```
#[stable(feature = "unit_from_iter", since = "1.23.0")]
impl FromIterator<()> for () {
    fn from_iter<I: IntoIterator<Item = ()>>(iter: I) -> Self {
        iter.into_iter().for_each(|()| {})
    }
}

pub(crate) trait IsUnit {
    fn is_unit() -> bool;
}

impl<T: ?Sized> IsUnit for T {
    default fn is_unit() -> bool {
        false
    }
}

impl IsUnit for () {
    fn is_unit() -> bool {
        true
    }
}
