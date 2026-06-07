use crate::fmt;
use crate::iter::{FusedIterator, TrustedLen};

/// 创建一个迭代器，通过调用给定闭包惰性地生成恰好一个值。
///
/// 这常用于把一个单值生成过程接入其他迭代形式的 [`chain()`] 中。也许已有一个迭代器
/// 覆盖了几乎所有元素，但还需要额外处理一个特殊值；或者某个函数接收迭代器，而你
/// 只需要处理一个值。
///
/// 与 [`once()`] 不同，本函数会在被请求时才惰性生成该值。
///
/// [`chain()`]: Iterator::chain
/// [`once()`]: crate::iter::once
///
/// # 示例
///
/// 基本用法:
///
/// ```
/// use std::iter;
///
/// // one 是最孤独的数字
/// let mut one = iter::once_with(|| 1);
///
/// assert_eq!(Some(1), one.next());
///
/// // 只有一个值，仅此而已。
/// assert_eq!(None, one.next());
/// ```
///
/// 与另一个迭代器链接。假设要遍历 `.foo` 目录中的每个文件，同时还要包含配置文件
/// `.foorc`:
///
/// ```no_run
/// use std::iter;
/// use std::fs;
/// use std::path::PathBuf;
///
/// let dirs = fs::read_dir(".foo").unwrap();
///
/// // 需要把 DirEntry 迭代器转换为 PathBuf 迭代器，因此使用 map。
/// let dirs = dirs.map(|file| file.unwrap().path());
///
/// // 现在为配置文件创建一个单项迭代器。
/// let config = iter::once_with(|| PathBuf::from(".foorc"));
///
/// // 把两个迭代器链接成一个大迭代器。
/// let files = dirs.chain(config);
///
/// // 这会产出 .foo 中的所有文件以及 .foorc。
/// for f in files {
///     println!("{f:?}");
/// }
/// ```
#[inline]
#[stable(feature = "iter_once_with", since = "1.43.0")]
pub fn once_with<A, F: FnOnce() -> A>(make: F) -> OnceWith<F> {
    OnceWith { make: Some(make) }
}

/// 通过调用给定闭包 `F: FnOnce() -> A` 产出单个 `A` 类型元素的迭代器。
///
/// 该 `struct` 由 [`once_with()`] 函数创建。更多信息见该函数文档。
#[derive(Clone)]
#[stable(feature = "iter_once_with", since = "1.43.0")]
pub struct OnceWith<F> {
    make: Option<F>,
}

#[stable(feature = "iter_once_with_debug", since = "1.68.0")]
impl<F> fmt::Debug for OnceWith<F> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.make.is_some() {
            f.write_str("OnceWith(Some(_))")
        } else {
            f.write_str("OnceWith(None)")
        }
    }
}

#[stable(feature = "iter_once_with", since = "1.43.0")]
impl<A, F: FnOnce() -> A> Iterator for OnceWith<F> {
    type Item = A;

    #[inline]
    fn next(&mut self) -> Option<A> {
        let f = self.make.take()?;
        Some(f())
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.make.iter().size_hint()
    }
}

#[stable(feature = "iter_once_with", since = "1.43.0")]
impl<A, F: FnOnce() -> A> DoubleEndedIterator for OnceWith<F> {
    fn next_back(&mut self) -> Option<A> {
        self.next()
    }
}

#[stable(feature = "iter_once_with", since = "1.43.0")]
impl<A, F: FnOnce() -> A> ExactSizeIterator for OnceWith<F> {
    fn len(&self) -> usize {
        self.make.iter().len()
    }
}

#[stable(feature = "iter_once_with", since = "1.43.0")]
impl<A, F: FnOnce() -> A> FusedIterator for OnceWith<F> {}

#[stable(feature = "iter_once_with", since = "1.43.0")]
unsafe impl<A, F: FnOnce() -> A> TrustedLen for OnceWith<F> {}
