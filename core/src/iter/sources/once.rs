use crate::iter::{FusedIterator, TrustedLen};

/// 创建一个恰好产出一个元素的迭代器。
///
/// 这常用于把单个值接入其他迭代形式的 [`chain()`] 中。也许已有一个迭代器覆盖了
/// 几乎所有元素，但还需要额外处理一个特殊值；或者某个函数接收迭代器，而你只需要
/// 处理一个值。
///
/// [`chain()`]: Iterator::chain
///
/// # 示例
///
/// 基本用法:
///
/// ```
/// use std::iter;
///
/// // one 是最孤独的数字
/// let mut one = iter::once(1);
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
/// let dirs = fs::read_dir(".foo")?;
///
/// // 需要把 DirEntry 迭代器转换为 PathBuf 迭代器，因此使用 map。
/// let dirs = dirs.map(|file| file.unwrap().path());
///
/// // 现在为配置文件创建一个单项迭代器。
/// let config = iter::once(PathBuf::from(".foorc"));
///
/// // 把两个迭代器链接成一个大迭代器。
/// let files = dirs.chain(config);
///
/// // 这会产出 .foo 中的所有文件以及 .foorc。
/// for f in files {
///     println!("{f:?}");
/// }
/// # std::io::Result::Ok(())
/// ```
#[stable(feature = "iter_once", since = "1.2.0")]
pub fn once<T>(value: T) -> Once<T> {
    Once { inner: Some(value).into_iter() }
}

/// 恰好产出一个元素的迭代器。
///
/// 该 `struct` 由 [`once()`] 函数创建。更多信息见该函数文档。
#[derive(Clone, Debug)]
#[stable(feature = "iter_once", since = "1.2.0")]
#[rustc_diagnostic_item = "IterOnce"]
pub struct Once<T> {
    inner: crate::option::IntoIter<T>,
}

#[stable(feature = "iter_once", since = "1.2.0")]
impl<T> Iterator for Once<T> {
    type Item = T;

    fn next(&mut self) -> Option<T> {
        self.inner.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

#[stable(feature = "iter_once", since = "1.2.0")]
impl<T> DoubleEndedIterator for Once<T> {
    fn next_back(&mut self) -> Option<T> {
        self.inner.next_back()
    }
}

#[stable(feature = "iter_once", since = "1.2.0")]
impl<T> ExactSizeIterator for Once<T> {
    fn len(&self) -> usize {
        self.inner.len()
    }
}

#[unstable(feature = "trusted_len", issue = "37572")]
unsafe impl<T> TrustedLen for Once<T> {}

#[stable(feature = "fused", since = "1.26.0")]
impl<T> FusedIterator for Once<T> {}
