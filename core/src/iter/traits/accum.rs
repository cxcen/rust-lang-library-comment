use crate::iter;
use crate::num::{Saturating, Wrapping};

/// 表示可通过对 iterator 求和来创建的类型。
///
/// 该 trait 用于实现 [`Iterator::sum()`]。实现此 trait 的类型，可以通过在
/// iterator 上调用 [`sum()`] 方法生成。和 [`FromIterator`] 类似，通常很少需要
/// 直接调用这个 trait 的方法。
///
/// [`sum()`]: Iterator::sum
/// [`FromIterator`]: iter::FromIterator
#[stable(feature = "iter_arith_traits", since = "1.12.0")]
#[diagnostic::on_unimplemented(
    message = "a value of type `{Self}` cannot be made by summing an iterator over elements of type `{A}`",
    label = "value of type `{Self}` cannot be made by summing a `std::iter::Iterator<Item={A}>`"
)]
pub trait Sum<A = Self>: Sized {
    /// 接收一个 iterator，并通过对其中元素“求和”来生成 `Self`。
    #[stable(feature = "iter_arith_traits", since = "1.12.0")]
    fn sum<I: Iterator<Item = A>>(iter: I) -> Self;
}

/// 表示可通过把 iterator 的元素相乘来创建的类型。
///
/// 该 trait 用于实现 [`Iterator::product()`]。实现此 trait 的类型，可以通过在
/// iterator 上调用 [`product()`] 方法生成。和 [`FromIterator`] 类似，通常很少需要
/// 直接调用这个 trait 的方法。
///
/// [`product()`]: Iterator::product
/// [`FromIterator`]: iter::FromIterator
#[stable(feature = "iter_arith_traits", since = "1.12.0")]
#[diagnostic::on_unimplemented(
    message = "a value of type `{Self}` cannot be made by multiplying all elements of type `{A}` from an iterator",
    label = "value of type `{Self}` cannot be made by multiplying all elements from a `std::iter::Iterator<Item={A}>`"
)]
pub trait Product<A = Self>: Sized {
    /// 接收一个 iterator，并通过把其中元素相乘来生成 `Self`。
    #[stable(feature = "iter_arith_traits", since = "1.12.0")]
    fn product<I: Iterator<Item = A>>(iter: I) -> Self;
}

macro_rules! integer_sum_product {
    (@impls $zero:expr, $one:expr, #[$attr:meta], $($a:ty)*) => ($(
        #[$attr]
        impl Sum for $a {
            fn sum<I: Iterator<Item=Self>>(iter: I) -> Self {
                iter.fold(
                    $zero,
                    #[rustc_inherit_overflow_checks]
                    |a, b| a + b,
                )
            }
        }

        #[$attr]
        impl Product for $a {
            fn product<I: Iterator<Item=Self>>(iter: I) -> Self {
                iter.fold(
                    $one,
                    #[rustc_inherit_overflow_checks]
                    |a, b| a * b,
                )
            }
        }

        #[$attr]
        impl<'a> Sum<&'a $a> for $a {
            fn sum<I: Iterator<Item=&'a Self>>(iter: I) -> Self {
                iter.fold(
                    $zero,
                    #[rustc_inherit_overflow_checks]
                    |a, b| a + b,
                )
            }
        }

        #[$attr]
        impl<'a> Product<&'a $a> for $a {
            fn product<I: Iterator<Item=&'a Self>>(iter: I) -> Self {
                iter.fold(
                    $one,
                    #[rustc_inherit_overflow_checks]
                    |a, b| a * b,
                )
            }
        }
    )*);
    ($($a:ty)*) => (
        integer_sum_product!(@impls 0, 1,
                #[stable(feature = "iter_arith_traits", since = "1.12.0")],
                $($a)*);
        integer_sum_product!(@impls Wrapping(0), Wrapping(1),
                #[stable(feature = "wrapping_iter_arith", since = "1.14.0")],
                $(Wrapping<$a>)*);
    );
}

macro_rules! saturating_integer_sum_product {
    (@impls $zero:expr, $one:expr, $doc:expr, #[$attr:meta], $($a:ty)*) => ($(
        #[$attr]
        #[doc = $doc]
        impl Sum for $a {
            fn sum<I: Iterator<Item=Self>>(iter: I) -> Self {
                iter.fold(
                    $zero,
                    |a, b| a + b,
                )
            }
        }

        #[$attr]
        #[doc = $doc]
        impl Product for $a {
            fn product<I: Iterator<Item=Self>>(iter: I) -> Self {
                iter.fold(
                    $one,
                    |a, b| a * b,
                )
            }
        }

        #[$attr]
        #[doc = $doc]
        impl<'a> Sum<&'a $a> for $a {
            fn sum<I: Iterator<Item=&'a Self>>(iter: I) -> Self {
                iter.fold(
                    $zero,
                    |a, b| a + b,
                )
            }
        }

        #[$attr]
        #[doc = $doc]
        impl<'a> Product<&'a $a> for $a {
            fn product<I: Iterator<Item=&'a Self>>(iter: I) -> Self {
                iter.fold(
                    $one,
                    |a, b| a * b,
                )
            }
        }
    )*);
    ($($a:ty)*) => (
        saturating_integer_sum_product!(@impls Saturating(0), Saturating(1),
                "The short-circuiting behavior of this implementation is unspecified. If you care about \
                short-circuiting, use [`Iterator::fold`] directly.",
                #[stable(feature = "saturating_iter_arith", since = "1.91.0")],
                $(Saturating<$a>)*);
    );
}

macro_rules! float_sum_product {
    ($($a:ident)*) => ($(
        #[stable(feature = "iter_arith_traits", since = "1.12.0")]
        impl Sum for $a {
            fn sum<I: Iterator<Item=Self>>(iter: I) -> Self {
                iter.fold(
                    -0.0,
                    #[rustc_inherit_overflow_checks]
                    |a, b| a + b,
                )
            }
        }

        #[stable(feature = "iter_arith_traits", since = "1.12.0")]
        impl Product for $a {
            fn product<I: Iterator<Item=Self>>(iter: I) -> Self {
                iter.fold(
                    1.0,
                    #[rustc_inherit_overflow_checks]
                    |a, b| a * b,
                )
            }
        }

        #[stable(feature = "iter_arith_traits", since = "1.12.0")]
        impl<'a> Sum<&'a $a> for $a {
            fn sum<I: Iterator<Item=&'a Self>>(iter: I) -> Self {
                iter.fold(
                    -0.0,
                    #[rustc_inherit_overflow_checks]
                    |a, b| a + b,
                )
            }
        }

        #[stable(feature = "iter_arith_traits", since = "1.12.0")]
        impl<'a> Product<&'a $a> for $a {
            fn product<I: Iterator<Item=&'a Self>>(iter: I) -> Self {
                iter.fold(
                    1.0,
                    #[rustc_inherit_overflow_checks]
                    |a, b| a * b,
                )
            }
        }
    )*)
}

integer_sum_product! { i8 i16 i32 i64 i128 isize u8 u16 u32 u64 u128 usize }
saturating_integer_sum_product! { u8 u16 u32 u64 u128 usize }
float_sum_product! { f16 f32 f64 f128 }

#[stable(feature = "iter_arith_traits_result", since = "1.16.0")]
impl<T, U, E> Sum<Result<U, E>> for Result<T, E>
where
    T: Sum<U>,
{
    /// 逐个取出 [`Iterator`] 中的元素: 如果遇到 [`Err`]，就不再继续取元素，并返回
    /// 该 [`Err`]；如果没有出现 [`Err`]，则返回所有元素的和。
    ///
    /// # 示例
    ///
    /// 该示例对 vector 中的每个整数求和；如果遇到负数，则拒绝这次求和并返回错误:
    ///
    /// ```
    /// let f = |&x: &i32| if x < 0 { Err("Negative element found") } else { Ok(x) };
    /// let v = vec![1, 2];
    /// let res: Result<i32, _> = v.iter().map(f).sum();
    /// assert_eq!(res, Ok(3));
    /// let v = vec![1, -2];
    /// let res: Result<i32, _> = v.iter().map(f).sum();
    /// assert_eq!(res, Err("Negative element found"));
    /// ```
    fn sum<I>(iter: I) -> Result<T, E>
    where
        I: Iterator<Item = Result<U, E>>,
    {
        iter::try_process(iter, |i| i.sum())
    }
}

#[stable(feature = "iter_arith_traits_result", since = "1.16.0")]
impl<T, U, E> Product<Result<U, E>> for Result<T, E>
where
    T: Product<U>,
{
    /// 逐个取出 [`Iterator`] 中的元素: 如果遇到 [`Err`]，就不再继续取元素，并返回
    /// 该 [`Err`]；如果没有出现 [`Err`]，则返回所有元素的积。
    ///
    /// # 示例
    ///
    /// 该示例把字符串 vector 中的每个数字相乘；如果某个字符串无法解析，则操作返回
    /// `Err`:
    ///
    /// ```
    /// let nums = vec!["5", "10", "1", "2"];
    /// let total: Result<usize, _> = nums.iter().map(|w| w.parse::<usize>()).product();
    /// assert_eq!(total, Ok(100));
    /// let nums = vec!["5", "10", "one", "2"];
    /// let total: Result<usize, _> = nums.iter().map(|w| w.parse::<usize>()).product();
    /// assert!(total.is_err());
    /// ```
    fn product<I>(iter: I) -> Result<T, E>
    where
        I: Iterator<Item = Result<U, E>>,
    {
        iter::try_process(iter, |i| i.product())
    }
}

#[stable(feature = "iter_arith_traits_option", since = "1.37.0")]
impl<T, U> Sum<Option<U>> for Option<T>
where
    T: Sum<U>,
{
    /// 逐个取出 [`Iterator`] 中的元素: 如果遇到 [`None`]，就不再继续取元素，并返回
    /// 该 [`None`]；如果没有出现 [`None`]，则返回所有元素的和。
    ///
    /// # 示例
    ///
    /// 该示例对字符串 vector 中字符 'a' 的位置求和；如果某个单词不包含字符 'a'，
    /// 操作返回 `None`:
    ///
    /// ```
    /// let words = vec!["have", "a", "great", "day"];
    /// let total: Option<usize> = words.iter().map(|w| w.find('a')).sum();
    /// assert_eq!(total, Some(5));
    /// let words = vec!["have", "a", "good", "day"];
    /// let total: Option<usize> = words.iter().map(|w| w.find('a')).sum();
    /// assert_eq!(total, None);
    /// ```
    fn sum<I>(iter: I) -> Option<T>
    where
        I: Iterator<Item = Option<U>>,
    {
        iter::try_process(iter, |i| i.sum())
    }
}

#[stable(feature = "iter_arith_traits_option", since = "1.37.0")]
impl<T, U> Product<Option<U>> for Option<T>
where
    T: Product<U>,
{
    /// 逐个取出 [`Iterator`] 中的元素: 如果遇到 [`None`]，就不再继续取元素，并返回
    /// 该 [`None`]；如果没有出现 [`None`]，则返回所有元素的积。
    ///
    /// # 示例
    ///
    /// 该示例把字符串 vector 中的每个数字相乘；如果某个字符串无法解析，则操作返回
    /// `None`:
    ///
    /// ```
    /// let nums = vec!["5", "10", "1", "2"];
    /// let total: Option<usize> = nums.iter().map(|w| w.parse::<usize>().ok()).product();
    /// assert_eq!(total, Some(100));
    /// let nums = vec!["5", "10", "one", "2"];
    /// let total: Option<usize> = nums.iter().map(|w| w.parse::<usize>().ok()).product();
    /// assert_eq!(total, None);
    /// ```
    fn product<I>(iter: I) -> Option<T>
    where
        I: Iterator<Item = Option<U>>,
    {
        iter::try_process(iter, |i| i.product())
    }
}
