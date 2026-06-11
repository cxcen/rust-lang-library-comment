//! 检视与操作进程的环境（environment）。
//!
//! 本模块包含一些函数，用于检视各种状态，例如环境变量、进程参数、当前目录，
//! 以及其他若干重要目录。
//!
//! 本模块中有若干函数和结构体带有以 `os` 结尾的对应版本。以 `os` 结尾的会返回
//! [`OsString`]，不带 `os` 的则返回 [`String`]。

#![stable(feature = "env", since = "1.0.0")]

use crate::error::Error;
use crate::ffi::{OsStr, OsString};
use crate::num::NonZero;
use crate::ops::Try;
use crate::path::{Path, PathBuf};
use crate::sys::{env as env_imp, os as os_imp};
use crate::{array, fmt, io, sys};

/// 以 [`PathBuf`] 形式返回当前工作目录。
///
/// # 平台特定行为
///
/// 此函数[目前][currently]在 Unix 上对应 `getcwd` 函数，在 Windows 上对应
/// `GetCurrentDirectoryW` 函数。
///
/// [currently]: crate::io#platform-specific-behavior
///
/// # 错误(Errors）
///
/// 如果当前工作目录的值无效，则返回 [`Err`]。可能的情形：
///
/// * 当前目录不存在。
/// * 没有足够的权限访问当前目录。
///
/// # 示例
///
/// ```
/// use std::env;
///
/// fn main() -> std::io::Result<()> {
///     let path = env::current_dir()?;
///     println!("The current directory is {}", path.display());
///     Ok(())
/// }
/// ```
#[doc(alias = "pwd")]
#[doc(alias = "getcwd")]
#[doc(alias = "GetCurrentDirectory")]
#[stable(feature = "env", since = "1.0.0")]
pub fn current_dir() -> io::Result<PathBuf> {
    os_imp::getcwd()
}

/// 将当前工作目录更改为指定路径。
///
/// # 平台特定行为
///
/// 此函数[目前][currently]在 Unix 上对应 `chdir` 函数，在 Windows 上对应
/// `SetCurrentDirectoryW` 函数。
///
/// 如果操作失败，则返回 [`Err`]。
///
/// [currently]: crate::io#platform-specific-behavior
///
/// # 示例
///
/// ```
/// use std::env;
/// use std::path::Path;
///
/// let root = Path::new("/");
/// assert!(env::set_current_dir(&root).is_ok());
/// println!("Successfully changed working directory to {}!", root.display());
/// ```
#[doc(alias = "chdir", alias = "SetCurrentDirectory", alias = "SetCurrentDirectoryW")]
#[stable(feature = "env", since = "1.0.0")]
pub fn set_current_dir<P: AsRef<Path>>(path: P) -> io::Result<()> {
    os_imp::chdir(path.as_ref())
}

/// 对本进程环境变量的一份快照进行迭代的迭代器。
///
/// 该结构体由 [`env::vars()`] 创建。更多内容见其文档。
///
/// [`env::vars()`]: vars
#[stable(feature = "env", since = "1.0.0")]
pub struct Vars {
    inner: VarsOs,
}

/// 对本进程环境变量的一份快照进行迭代的迭代器。
///
/// 该结构体由 [`env::vars_os()`] 创建。更多内容见其文档。
///
/// [`env::vars_os()`]: vars_os
#[stable(feature = "env", since = "1.0.0")]
pub struct VarsOs {
    inner: env_imp::Env,
}

/// 返回一个迭代器，产出当前进程所有环境变量的 (变量, 值) 字符串对。
///
/// 返回的迭代器包含调用此函数时进程环境变量的一份快照。此后对环境变量的修改
/// 不会反映到返回的迭代器中。
///
/// # Panics
///
/// 在迭代过程中，如果环境中任何键或值不是有效的 unicode，返回的迭代器将 panic。
/// 若不希望如此，可考虑使用 [`env::vars_os()`]。
///
/// # 示例
///
/// ```
/// // 打印所有环境变量。
/// for (key, value) in std::env::vars() {
///     println!("{key}: {value}");
/// }
/// ```
///
/// [`env::vars_os()`]: vars_os
#[must_use]
#[stable(feature = "env", since = "1.0.0")]
pub fn vars() -> Vars {
    Vars { inner: vars_os() }
}

/// 返回一个迭代器，产出当前进程所有环境变量的 (变量, 值) OS 字符串对。
///
/// 返回的迭代器包含调用此函数时进程环境变量的一份快照。此后对环境变量的修改
/// 不会反映到返回的迭代器中。
///
/// 注意：返回的迭代器不会检查环境变量是否为有效 Unicode。如果你希望在遇到
/// 无效 UTF-8 时 panic，请改用 [`vars`] 函数。
///
/// # 示例
///
/// ```
/// // 打印所有环境变量。
/// for (key, value) in std::env::vars_os() {
///     println!("{key:?}: {value:?}");
/// }
/// ```
#[must_use]
#[stable(feature = "env", since = "1.0.0")]
pub fn vars_os() -> VarsOs {
    VarsOs { inner: env_imp::env() }
}

#[stable(feature = "env", since = "1.0.0")]
impl Iterator for Vars {
    type Item = (String, String);
    fn next(&mut self) -> Option<(String, String)> {
        self.inner.next().map(|(a, b)| (a.into_string().unwrap(), b.into_string().unwrap()))
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

#[stable(feature = "std_debug", since = "1.16.0")]
impl fmt::Debug for Vars {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self { inner: VarsOs { inner } } = self;
        f.debug_struct("Vars").field("inner", inner).finish()
    }
}

#[stable(feature = "env", since = "1.0.0")]
impl Iterator for VarsOs {
    type Item = (OsString, OsString);
    fn next(&mut self) -> Option<(OsString, OsString)> {
        self.inner.next()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

#[stable(feature = "std_debug", since = "1.16.0")]
impl fmt::Debug for VarsOs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self { inner } = self;
        f.debug_struct("VarsOs").field("inner", inner).finish()
    }
}

/// 从当前进程获取环境变量 `key`。
///
/// # 错误(Errors）
///
/// 在以下情况返回 [`VarError::NotPresent`]：
/// - 该变量未设置。
/// - 该变量名包含等号或 NUL（`'='` 或 `'\0'`）。
///
/// 如果该变量的值不是有效 Unicode，则返回 [`VarError::NotUnicode`]。
/// 若不希望如此，可考虑使用 [`var_os`]。
///
/// 如果你想在编译期检查环境变量，请改用 [`env!`] 或 [`option_env!`]。
///
/// # 示例
///
/// ```
/// use std::env;
///
/// let key = "HOME";
/// match env::var(key) {
///     Ok(val) => println!("{key}: {val:?}"),
///     Err(e) => println!("couldn't interpret {key}: {e}"),
/// }
/// ```
#[stable(feature = "env", since = "1.0.0")]
pub fn var<K: AsRef<OsStr>>(key: K) -> Result<String, VarError> {
    _var(key.as_ref())
}

fn _var(key: &OsStr) -> Result<String, VarError> {
    match var_os(key) {
        Some(s) => s.into_string().map_err(VarError::NotUnicode),
        None => Err(VarError::NotPresent),
    }
}

/// 从当前进程获取环境变量 `key`；如果该变量未设置，或发生其他错误，则返回 [`None`]。
///
/// 当环境变量名包含等号字符（`=`）或 NUL 字符时，可能返回 `None`。
///
/// 注意：此函数不会检查环境变量是否为有效 Unicode。如果你希望在遇到无效 UTF-8 时
/// 得到一个错误，请改用 [`var`] 函数。
///
/// # 示例
///
/// ```
/// use std::env;
///
/// let key = "HOME";
/// match env::var_os(key) {
///     Some(val) => println!("{key}: {val:?}"),
///     None => println!("{key} is not defined in the environment.")
/// }
/// ```
///
/// 如果期望的是一个带分隔符的变量（例如 `PATH`），可以用 [`split_paths`] 将各项分开。
#[must_use]
#[stable(feature = "env", since = "1.0.0")]
pub fn var_os<K: AsRef<OsStr>>(key: K) -> Option<OsString> {
    _var_os(key.as_ref())
}

fn _var_os(key: &OsStr) -> Option<OsString> {
    env_imp::getenv(key)
}

/// 与环境变量交互的各种操作的错误类型。
/// 可能由 [`env::var()`] 返回。
///
/// [`env::var()`]: var
#[derive(Debug, PartialEq, Eq, Clone)]
#[stable(feature = "env", since = "1.0.0")]
pub enum VarError {
    /// 指定的环境变量在当前进程的环境中不存在。
    #[stable(feature = "env", since = "1.0.0")]
    NotPresent,

    /// 找到了指定的环境变量，但它不包含有效的 unicode 数据。
    /// 找到的数据作为该变体的载荷（payload）返回。
    #[stable(feature = "env", since = "1.0.0")]
    NotUnicode(#[stable(feature = "env", since = "1.0.0")] OsString),
}

#[stable(feature = "env", since = "1.0.0")]
impl fmt::Display for VarError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            VarError::NotPresent => write!(f, "environment variable not found"),
            VarError::NotUnicode(ref s) => {
                write!(f, "environment variable was not valid unicode: {:?}", s)
            }
        }
    }
}

#[stable(feature = "env", since = "1.0.0")]
impl Error for VarError {}

/// 为当前正在运行的进程，将环境变量 `key` 设置为值 `value`。
///
/// # 安全性(Safety）
///
/// 在单线程程序中调用此函数是安全的。
///
/// 在 Windows 上，无论单线程还是多线程程序，调用此函数也总是安全的。
///
/// 在其他操作系统的多线程程序中，唯一安全的选择是完全不使用 `set_var` 或 `remove_var`。
///
/// 确切的要求是：你必须确保没有其他线程通过本模块以外的函数或全局变量并发地写入或
/// *读取*(!) 环境。问题在于，这些操作系统并未提供线程安全的环境读取方式，而且大多数 C
/// 库（包括 libc 本身）也不会声明哪些函数会读取环境。即使是 Rust 标准库中的函数也可能
/// 在不经过本模块的情况下读取环境，例如 [`std::net::ToSocketAddrs`] 进行的 DNS 查找。
/// 对于某个库未来版本中哪些函数可能读取环境，并不作任何稳定保证。所有这些都使得你实际上
/// 不可能保证没有其他线程会读取环境，因此唯一安全的选择就是在多线程程序中完全不使用
/// `set_var` 或 `remove_var`。
///
/// 关于 Unix 上这种不安全性的讨论可见于：
///
///  - [Austin Group Bugzilla (for POSIX)](https://austingroupbugs.net/view.php?id=188)
///  - [GNU C library Bugzilla](https://sourceware.org/bugzilla/show_bug.cgi?id=15607#c2)
///
/// 要把环境变量传递给子进程，可以改用 [`Command::env`]。
///
/// [`std::net::ToSocketAddrs`]: crate::net::ToSocketAddrs
/// [`Command::env`]: crate::process::Command::env
///
/// # Panics
///
/// 如果 `key` 为空、包含 ASCII 等号 `'='` 或 NUL 字符 `'\0'`，或者 `value` 包含 NUL 字符，
/// 此函数可能 panic。
///
/// # 示例
///
/// ```
/// use std::env;
///
/// let key = "KEY";
/// unsafe {
///     env::set_var(key, "VALUE");
/// }
/// assert_eq!(env::var(key), Ok("VALUE".to_string()));
/// ```
#[rustc_deprecated_safe_2024(
    audit_that = "the environment access only happens in single-threaded code"
)]
#[stable(feature = "env", since = "1.0.0")]
pub unsafe fn set_var<K: AsRef<OsStr>, V: AsRef<OsStr>>(key: K, value: V) {
    let (key, value) = (key.as_ref(), value.as_ref());
    unsafe { env_imp::setenv(key, value) }.unwrap_or_else(|e| {
        panic!("failed to set environment variable `{key:?}` to `{value:?}`: {e}")
    })
}

/// 从当前正在运行的进程的环境中移除一个环境变量。
///
/// # 安全性(Safety）
///
/// 在单线程程序中调用此函数是安全的。
///
/// 在 Windows 上，无论单线程还是多线程程序，调用此函数也总是安全的。
///
/// 在其他操作系统的多线程程序中，唯一安全的选择是完全不使用 `set_var` 或 `remove_var`。
///
/// 确切的要求是：你必须确保没有其他线程通过本模块以外的函数或全局变量并发地写入或
/// *读取*(!) 环境。问题在于，这些操作系统并未提供线程安全的环境读取方式，而且大多数 C
/// 库（包括 libc 本身）也不会声明哪些函数会读取环境。即使是 Rust 标准库中的函数也可能
/// 在不经过本模块的情况下读取环境，例如 [`std::net::ToSocketAddrs`] 进行的 DNS 查找。
/// 对于某个库未来版本中哪些函数可能读取环境，并不作任何稳定保证。所有这些都使得你实际上
/// 不可能保证没有其他线程会读取环境，因此唯一安全的选择就是在多线程程序中完全不使用
/// `set_var` 或 `remove_var`。
///
/// 关于 Unix 上这种不安全性的讨论可见于：
///
///  - [Austin Group Bugzilla](https://austingroupbugs.net/view.php?id=188)
///  - [GNU C library Bugzilla](https://sourceware.org/bugzilla/show_bug.cgi?id=15607#c2)
///
/// 要防止子进程继承某个环境变量，可以改用 [`Command::env_remove`] 或 [`Command::env_clear`]。
///
/// [`std::net::ToSocketAddrs`]: crate::net::ToSocketAddrs
/// [`Command::env_remove`]: crate::process::Command::env_remove
/// [`Command::env_clear`]: crate::process::Command::env_clear
///
/// # Panics
///
/// 如果 `key` 为空、包含 ASCII 等号 `'='` 或 NUL 字符 `'\0'`，或者值包含 NUL 字符，
/// 此函数可能 panic。
///
/// # 示例
///
/// ```no_run
/// use std::env;
///
/// let key = "KEY";
/// unsafe {
///     env::set_var(key, "VALUE");
/// }
/// assert_eq!(env::var(key), Ok("VALUE".to_string()));
///
/// unsafe {
///     env::remove_var(key);
/// }
/// assert!(env::var(key).is_err());
/// ```
#[rustc_deprecated_safe_2024(
    audit_that = "the environment access only happens in single-threaded code"
)]
#[stable(feature = "env", since = "1.0.0")]
pub unsafe fn remove_var<K: AsRef<OsStr>>(key: K) {
    let key = key.as_ref();
    unsafe { env_imp::unsetenv(key) }
        .unwrap_or_else(|e| panic!("failed to remove environment variable `{key:?}`: {e}"))
}

/// 一个迭代器，按平台特定约定将某个环境变量拆分成多个路径。
///
/// 该迭代器的元素类型为 [`PathBuf`]。
///
/// 该结构体由 [`env::split_paths()`] 创建。更多内容见其文档。
///
/// [`env::split_paths()`]: split_paths
#[must_use = "iterators are lazy and do nothing unless consumed"]
#[stable(feature = "env", since = "1.0.0")]
pub struct SplitPaths<'a> {
    inner: os_imp::SplitPaths<'a>,
}

/// 按 `PATH` 环境变量的平台约定解析输入。
///
/// 返回一个迭代器，遍历 `unparsed` 中包含的各个路径。迭代器元素类型为 [`PathBuf`]。
///
/// 在大多数 Unix 平台上，分隔符是 `:`，而在 Windows 上是 `;`。在 Windows 上还会执行去引号
/// （unquoting）处理。
///
/// 可以用 [`join_paths`] 把各元素重新组合起来。
///
/// # Panics
///
/// 在不存在带分隔符的 `PATH` 变量的系统上（例如 UEFI），此函数会 panic。
///
/// # 示例
///
/// ```
/// use std::env;
///
/// let key = "PATH";
/// match env::var_os(key) {
///     Some(paths) => {
///         for path in env::split_paths(&paths) {
///             println!("'{}'", path.display());
///         }
///     }
///     None => println!("{key} is not defined in the environment.")
/// }
/// ```
#[stable(feature = "env", since = "1.0.0")]
pub fn split_paths<T: AsRef<OsStr> + ?Sized>(unparsed: &T) -> SplitPaths<'_> {
    SplitPaths { inner: os_imp::split_paths(unparsed.as_ref()) }
}

#[stable(feature = "env", since = "1.0.0")]
impl<'a> Iterator for SplitPaths<'a> {
    type Item = PathBuf;
    fn next(&mut self) -> Option<PathBuf> {
        self.inner.next()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

#[stable(feature = "std_debug", since = "1.16.0")]
impl fmt::Debug for SplitPaths<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SplitPaths").finish_non_exhaustive()
    }
}

/// `PATH` 变量相关操作的错误类型。可能由 [`env::join_paths()`] 返回。
///
/// [`env::join_paths()`]: join_paths
#[derive(Debug)]
#[stable(feature = "env", since = "1.0.0")]
pub struct JoinPathsError {
    inner: os_imp::JoinPathsError,
}

/// 将一组 [`Path`] 适当地拼接，以用于 `PATH` 环境变量。
///
/// # 错误(Errors）
///
/// 如果某个输入 [`Path`] 包含构造 `PATH` 变量时的无效字符（Windows 上是双引号，
/// Unix 上是冒号），或者系统没有类似 `PATH` 的变量（例如 UEFI 或 WASI），
/// 则返回 [`Err`]（其中含一条错误消息）。
///
/// # 示例
///
/// 在类 Unix 平台上拼接路径：
///
/// ```
/// use std::env;
/// use std::ffi::OsString;
/// use std::path::Path;
///
/// fn main() -> Result<(), env::JoinPathsError> {
/// # if cfg!(unix) {
///     let paths = [Path::new("/bin"), Path::new("/usr/bin")];
///     let path_os_string = env::join_paths(paths.iter())?;
///     assert_eq!(path_os_string, OsString::from("/bin:/usr/bin"));
/// # }
///     Ok(())
/// }
/// ```
///
/// 在类 Unix 平台上拼接一个包含冒号的路径会导致错误：
///
/// ```
/// # if cfg!(unix) {
/// use std::env;
/// use std::path::Path;
///
/// let paths = [Path::new("/bin"), Path::new("/usr/bi:n")];
/// assert!(env::join_paths(paths.iter()).is_err());
/// # }
/// ```
///
/// 配合 [`env::split_paths()`] 使用 `env::join_paths()`，向 `PATH` 环境变量追加一项：
///
/// ```
/// use std::env;
/// use std::path::PathBuf;
///
/// fn main() -> Result<(), env::JoinPathsError> {
///     if let Some(path) = env::var_os("PATH") {
///         let mut paths = env::split_paths(&path).collect::<Vec<_>>();
///         paths.push(PathBuf::from("/home/xyz/bin"));
///         let new_path = env::join_paths(paths)?;
///         unsafe { env::set_var("PATH", &new_path); }
///     }
///
///     Ok(())
/// }
/// ```
///
/// [`env::split_paths()`]: split_paths
#[stable(feature = "env", since = "1.0.0")]
pub fn join_paths<I, T>(paths: I) -> Result<OsString, JoinPathsError>
where
    I: IntoIterator<Item = T>,
    T: AsRef<OsStr>,
{
    os_imp::join_paths(paths.into_iter()).map_err(|e| JoinPathsError { inner: e })
}

#[stable(feature = "env", since = "1.0.0")]
impl fmt::Display for JoinPathsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}

#[stable(feature = "env", since = "1.0.0")]
impl Error for JoinPathsError {
    #[allow(deprecated, deprecated_in_future)]
    fn description(&self) -> &str {
        self.inner.description()
    }
}

/// 返回当前用户主目录（home directory）的路径（如果已知）。
///
/// 如果获取该目录失败，或平台没有用户主目录的概念，可能返回 `None`。
///
/// 对于存储用户数据和配置，通常更宜使用更具体的目录。例如 Unix 上的 [XDG Base Directories]，
/// 或 Windows 上的 `LOCALAPPDATA` 与 `APPDATA` 环境变量。
///
/// [XDG Base Directories]: https://specifications.freedesktop.org/basedir-spec/latest/
///
/// # Unix
///
/// - 如果设置了 'HOME' 环境变量（且不为空字符串），返回其值。
/// - 否则，尝试通过使用当前用户的 UID 调用 `getpwuid_r` 函数来确定主目录。
///   `getpwuid_r` 函数返回的空主目录字段会被视为有效值。
/// - 如果当前用户在 /etc/passwd 文件中没有对应条目，则返回 `None`。
///
/// # Windows
///
/// - 如果设置了 'USERPROFILE' 环境变量且不为空字符串，返回其值。
/// - 否则，使用 [`GetUserProfileDirectory`][msdn] 返回该路径。这在未来可能改变。
///
/// [msdn]: https://docs.microsoft.com/en-us/windows/win32/api/userenv/nf-userenv-getuserprofiledirectorya
///
/// 在 UWP（Universal Windows Platform）目标上，此函数未实现，总是返回 `None`。
///
/// 在 Rust 1.85.0 之前，此函数在 Windows 上曾返回 'HOME' 环境变量的值，
/// 这在 Cygwin 或 Mingw 环境中可能返回像 `/home/you` 这样的非标准路径，
/// 而非 `C:\Users\you`。
///
/// # 示例
///
/// ```
/// use std::env;
///
/// match env::home_dir() {
///     Some(path) => println!("Your home directory, probably: {}", path.display()),
///     None => println!("Impossible to get your home dir!"),
/// }
/// ```
#[must_use]
#[stable(feature = "env", since = "1.0.0")]
pub fn home_dir() -> Option<PathBuf> {
    os_imp::home_dir()
}

/// 返回一个临时目录的路径。
///
/// 该临时目录可能被多个用户共享，或被具有不同权限的进程共享；因此，在临时目录中创建任何
/// 文件或目录时，都必须使用安全的方法来创建一个唯一命名的文件。使用固定或可预测名称创建
/// 文件或目录可能导致“不安全临时文件（insecure temporary file）”安全漏洞。可考虑使用某个
/// 能安全创建临时文件或目录的 crate。
///
/// 注意：返回的值可能是一个符号链接（symbolic link），而非目录。
///
/// # 平台特定行为
///
/// 在 Unix 上，如果设置了 `TMPDIR` 环境变量则返回其值，否则该值与具体 OS 有关：
/// - 在 Android 上没有全局临时文件夹（通常按应用单独分配）；如果程序运行在应用的命名空间中
///   且系统版本为 Android 13（或以上），则返回应用的 cache 目录，否则返回
///   `/data/local/tmp`。
/// - 在基于 Darwin 的系统（macOS、iOS 等）上，返回 `confstr(_CS_DARWIN_USER_TEMP_DIR, ...)`
///   提供的目录，正如 [Apple 的安全指南][appledoc]所推荐。
/// - 在所有其他基于 unix 的系统上，返回 `/tmp`。
///
/// 在 Windows 上，其行为等价于 [`GetTempPath2`][GetTempPath2] /
/// [`GetTempPath`][GetTempPath]，本函数内部即使用它们。
///
/// 注意，这[在未来可能改变][changes]。
///
/// [changes]: io#platform-specific-behavior
/// [GetTempPath2]: https://docs.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-gettemppath2a
/// [GetTempPath]: https://docs.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-gettemppatha
/// [appledoc]: https://developer.apple.com/library/archive/documentation/Security/Conceptual/SecureCodingGuide/Articles/RaceConditions.html#//apple_ref/doc/uid/TP40002585-SW10
///
/// ```no_run
/// use std::env;
///
/// fn main() {
///     let dir = env::temp_dir();
///     println!("Temporary directory: {}", dir.display());
/// }
/// ```
#[must_use]
#[doc(alias = "GetTempPath", alias = "GetTempPath2")]
#[stable(feature = "env", since = "1.0.0")]
pub fn temp_dir() -> PathBuf {
    os_imp::temp_dir()
}

/// 返回当前正在运行的可执行文件的完整文件系统路径。
///
/// # 平台特定行为
///
/// 如果该可执行文件是通过符号链接被调用的，某些平台会返回符号链接本身的路径，
/// 而另一些平台会返回符号链接目标的路径。
///
/// 如果可执行文件在运行期间被重命名，某些平台可能返回它加载时的路径，而非新路径。
///
/// # 错误(Errors）
///
/// 获取当前可执行文件的路径是一个平台特定的操作，可能因许多原因而失败。一些错误可能包括
/// 但不限于：文件系统操作失败，或一般的系统调用失败。
///
/// # 安全性(Security）
///
/// 此函数的输出不应被信任用于任何可能涉及安全的用途。基本上，如果用户能运行该可执行文件，
/// 他们就能任意改变其输出。
///
/// 举个例子，你很容易引入一个竞态条件（race condition）。过程是这样的：
///
/// 1. 你用 `current_exe()` 获取当前可执行文件的路径，并将其存入一个变量。
/// 2. 时间流逝。某个恶意行为者移除了当前可执行文件，并用一个恶意文件替换它。
/// 3. 然后你使用先前存储的路径来重新执行当前可执行文件。
///
/// 你以为在安全地执行当前可执行文件，结果却执行了完全不同的东西。你刚刚执行的代码
/// 以你的权限运行。
///
/// 这类行为在使用不当时，已知会[导致权限提升][lead to privilege escalation]。
///
/// [lead to privilege escalation]: https://securityvulns.com/Wdocument183.html
///
/// # 示例
///
/// ```
/// use std::env;
///
/// match env::current_exe() {
///     Ok(exe_path) => println!("Path of this executable is: {}",
///                              exe_path.display()),
///     Err(e) => println!("failed to get current exe path: {e}"),
/// };
/// ```
#[stable(feature = "env", since = "1.0.0")]
pub fn current_exe() -> io::Result<PathBuf> {
    os_imp::current_exe()
}

/// 对进程参数进行迭代的迭代器，为每个参数产出一个 [`String`] 值。
///
/// 该结构体由 [`env::args()`] 创建。更多内容见其文档。
///
/// 第一个元素按惯例是可执行文件的路径，但它可以被设为任意文本，甚至可能根本不存在。
/// 这意味着该属性不应被用于安全用途。
///
/// [`env::args()`]: args
#[must_use = "iterators are lazy and do nothing unless consumed"]
#[stable(feature = "env", since = "1.0.0")]
pub struct Args {
    inner: ArgsOs,
}

/// 对进程参数进行迭代的迭代器，为每个参数产出一个 [`OsString`] 值。
///
/// 该结构体由 [`env::args_os()`] 创建。更多内容见其文档。
///
/// 第一个元素按惯例是可执行文件的路径，但它可以被设为任意文本，甚至可能根本不存在。
/// 这意味着该属性不应被用于安全用途。
///
/// [`env::args_os()`]: args_os
#[must_use = "iterators are lazy and do nothing unless consumed"]
#[stable(feature = "env", since = "1.0.0")]
pub struct ArgsOs {
    inner: sys::args::Args,
}

/// 返回本程序启动时所带的参数（通常通过命令行传入）。
///
/// 第一个元素按惯例是可执行文件的路径，但它可以被设为任意文本，甚至可能根本不存在。
/// 这意味着该属性不应被用于安全用途。
///
/// 在 Unix 系统上，shell 通常会对未加引号、带 glob 模式（例如 `*` 和 `?`）的参数进行展开。
/// 在 Windows 上不会这样做，这类参数会按原样传入。
///
/// 在 glibc Linux 系统上，参数是通过把一个函数放入 `.init_array` 来获取的。glibc 会把
/// `argc`、`argv` 和 `envp` 作为非标准扩展传给 `.init_array` 中的函数。这使得
/// `std::env::args` 即使在 `cdylib` 或 `staticlib` 中也能工作，正如它在 macOS 和
/// Windows 上那样。
///
/// # Panics
///
/// 如果进程的任何参数不是有效 Unicode，返回的迭代器将在迭代过程中 panic。
/// 若不希望如此，请改用 [`args_os`] 函数。
///
/// # 示例
///
/// ```
/// use std::env;
///
/// // 每行打印一个参数
/// for argument in env::args() {
///     println!("{argument}");
/// }
/// ```
#[stable(feature = "env", since = "1.0.0")]
pub fn args() -> Args {
    Args { inner: args_os() }
}

/// 返回本程序启动时所带的参数（通常通过命令行传入）。
///
/// 第一个元素按惯例是可执行文件的路径，但它可以被设为任意文本，甚至可能根本不存在。
/// 这意味着该属性不应被用于安全用途。
///
/// 在 Unix 系统上，shell 通常会对未加引号、带 glob 模式（例如 `*` 和 `?`）的参数进行展开。
/// 在 Windows 上不会这样做，这类参数会按原样传入。
///
/// 在 glibc Linux 系统上，参数是通过把一个函数放入 `.init_array` 来获取的。glibc 会把
/// `argc`、`argv` 和 `envp` 作为非标准扩展传给 `.init_array` 中的函数。这使得
/// `std::env::args_os` 即使在 `cdylib` 或 `staticlib` 中也能工作，正如它在 macOS 和
/// Windows 上那样。
///
/// 注意：返回的迭代器不会检查进程参数是否为有效 Unicode。如果你希望在遇到无效 UTF-8 时
/// panic，请改用 [`args`] 函数。
///
/// # 示例
///
/// ```
/// use std::env;
///
/// // 每行打印一个参数
/// for argument in env::args_os() {
///     println!("{argument:?}");
/// }
/// ```
#[stable(feature = "env", since = "1.0.0")]
pub fn args_os() -> ArgsOs {
    ArgsOs { inner: sys::args::args() }
}

#[stable(feature = "env_unimpl_send_sync", since = "1.26.0")]
impl !Send for Args {}

#[stable(feature = "env_unimpl_send_sync", since = "1.26.0")]
impl !Sync for Args {}

#[stable(feature = "env", since = "1.0.0")]
impl Iterator for Args {
    type Item = String;

    fn next(&mut self) -> Option<String> {
        self.inner.next().map(|s| s.into_string().unwrap())
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }

    // 跳过参数的方法不能简单地委托给内层迭代器，因为 `env::args` 声明它会“在任何进程参数
    // 不是有效 Unicode 时于迭代过程中 panic”。
    //
    // 这有两种可能的解读：
    // - 被跳过的参数从不会在“迭代过程中”被遇到
    // - 即便被跳过的参数也算在“迭代过程中”被遇到
    //
    // 由于 panic 是可被观察到的，我们目前倾向于即使是被跳过的参数也加以校验，
    // 尽管该 API 并未明确承诺这一点。
}

#[stable(feature = "env", since = "1.0.0")]
impl ExactSizeIterator for Args {
    #[inline]
    fn len(&self) -> usize {
        self.inner.len()
    }

    #[inline]
    fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

#[stable(feature = "env_iterators", since = "1.12.0")]
impl DoubleEndedIterator for Args {
    fn next_back(&mut self) -> Option<String> {
        self.inner.next_back().map(|s| s.into_string().unwrap())
    }
}

#[stable(feature = "std_debug", since = "1.16.0")]
impl fmt::Debug for Args {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self { inner: ArgsOs { inner } } = self;
        f.debug_struct("Args").field("inner", inner).finish()
    }
}

#[stable(feature = "env_unimpl_send_sync", since = "1.26.0")]
impl !Send for ArgsOs {}

#[stable(feature = "env_unimpl_send_sync", since = "1.26.0")]
impl !Sync for ArgsOs {}

#[stable(feature = "env", since = "1.0.0")]
impl Iterator for ArgsOs {
    type Item = OsString;

    #[inline]
    fn next(&mut self) -> Option<OsString> {
        self.inner.next()
    }

    #[inline]
    fn next_chunk<const N: usize>(
        &mut self,
    ) -> Result<[OsString; N], array::IntoIter<OsString, N>> {
        self.inner.next_chunk()
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }

    #[inline]
    fn count(self) -> usize {
        self.inner.len()
    }

    #[inline]
    fn last(self) -> Option<OsString> {
        self.inner.last()
    }

    #[inline]
    fn advance_by(&mut self, n: usize) -> Result<(), NonZero<usize>> {
        self.inner.advance_by(n)
    }

    #[inline]
    fn try_fold<B, F, R>(&mut self, init: B, f: F) -> R
    where
        F: FnMut(B, Self::Item) -> R,
        R: Try<Output = B>,
    {
        self.inner.try_fold(init, f)
    }

    #[inline]
    fn fold<B, F>(self, init: B, f: F) -> B
    where
        F: FnMut(B, Self::Item) -> B,
    {
        self.inner.fold(init, f)
    }
}

#[stable(feature = "env", since = "1.0.0")]
impl ExactSizeIterator for ArgsOs {
    #[inline]
    fn len(&self) -> usize {
        self.inner.len()
    }

    #[inline]
    fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

#[stable(feature = "env_iterators", since = "1.12.0")]
impl DoubleEndedIterator for ArgsOs {
    #[inline]
    fn next_back(&mut self) -> Option<OsString> {
        self.inner.next_back()
    }

    #[inline]
    fn advance_back_by(&mut self, n: usize) -> Result<(), NonZero<usize>> {
        self.inner.advance_back_by(n)
    }
}

#[stable(feature = "std_debug", since = "1.16.0")]
impl fmt::Debug for ArgsOs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self { inner } = self;
        f.debug_struct("ArgsOs").field("inner", inner).finish()
    }
}

/// 与当前目标（target）相关联的常量
#[stable(feature = "env", since = "1.0.0")]
pub mod consts {
    use crate::sys::env_consts::os;

    /// 描述当前所用 CPU 架构的字符串。
    /// 一个示例值可能是：`"x86"`、`"arm"` 或 `"riscv64"`。
    ///
    /// <details><summary>可能取值的完整列表</summary>
    ///
    /// * `"x86"`
    /// * `"x86_64"`
    /// * `"arm"`
    /// * `"aarch64"`
    /// * `"m68k"`
    /// * `"mips"`
    /// * `"mips32r6"`
    /// * `"mips64"`
    /// * `"mips64r6"`
    /// * `"csky"`
    /// * `"powerpc"`
    /// * `"powerpc64"`
    /// * `"riscv32"`
    /// * `"riscv64"`
    /// * `"s390x"`
    /// * `"sparc"`
    /// * `"sparc64"`
    /// * `"hexagon"`
    /// * `"loongarch32"`
    /// * `"loongarch64"`
    ///
    /// </details>
    #[stable(feature = "env", since = "1.0.0")]
    pub const ARCH: &str = env!("STD_ENV_ARCH");

    /// 描述操作系统家族（family）的字符串。
    /// 一个示例值可能是：`"unix"` 或 `"windows"`。
    ///
    /// 如果家族未知，该值可能为空字符串。
    ///
    /// <details><summary>可能取值的完整列表</summary>
    ///
    /// * `"unix"`
    /// * `"windows"`
    /// * `"itron"`
    /// * `"wasm"`
    /// * `""`
    ///
    /// </details>
    #[stable(feature = "env", since = "1.0.0")]
    pub const FAMILY: &str = os::FAMILY;

    /// 描述当前所用具体操作系统的字符串。
    /// 一个示例值可能是：`"linux"` 或 `"freebsd"`。
    ///
    /// <details><summary>可能取值的完整列表</summary>
    ///
    /// * `"linux"`
    /// * `"windows"`
    /// * `"macos"`
    /// * `"android"`
    /// * `"ios"`
    /// * `"openbsd"`
    /// * `"freebsd"`
    /// * `"netbsd"`
    /// * `"wasi"`
    /// * `"hermit"`
    /// * `"aix"`
    /// * `"apple"`
    /// * `"dragonfly"`
    /// * `"emscripten"`
    /// * `"espidf"`
    /// * `"fortanix"`
    /// * `"uefi"`
    /// * `"fuchsia"`
    /// * `"haiku"`
    /// * `"hermit"`
    /// * `"watchos"`
    /// * `"visionos"`
    /// * `"tvos"`
    /// * `"horizon"`
    /// * `"hurd"`
    /// * `"illumos"`
    /// * `"l4re"`
    /// * `"nto"`
    /// * `"redox"`
    /// * `"solaris"`
    /// * `"solid_asp3"`
    /// * `"vexos"`
    /// * `"vita"`
    /// * `"vxworks"`
    /// * `"xous"`
    ///
    /// </details>
    #[stable(feature = "env", since = "1.0.0")]
    pub const OS: &str = os::OS;

    /// 指定本平台上共享库（shared libraries）所用的文件名前缀（如果有的话）。
    /// 它要么是 `"lib"`，要么是空字符串（`""`）。
    #[stable(feature = "env", since = "1.0.0")]
    pub const DLL_PREFIX: &str = os::DLL_PREFIX;

    /// 指定本平台上共享库所用的文件名后缀（如果有的话）。
    /// 一个示例值可能是：`".so"`、`".elf"` 或 `".dll"`。
    ///
    /// 其可能取值与 [`DLL_EXTENSION`] 相同，只是带上了前导的点号。
    #[stable(feature = "env", since = "1.0.0")]
    pub const DLL_SUFFIX: &str = os::DLL_SUFFIX;

    /// 指定本平台上共享库所用的文件扩展名（如果有的话），不含点号。
    /// 一个示例值可能是：`"so"`、`"elf"` 或 `"dll"`。
    ///
    /// <details><summary>可能取值的完整列表</summary>
    ///
    /// * `"so"`
    /// * `"dylib"`
    /// * `"dll"`
    /// * `"sgxs"`
    /// * `"a"`
    /// * `"elf"`
    /// * `"wasm"`
    /// * `""` (an empty string)
    ///
    /// </details>
    #[stable(feature = "env", since = "1.0.0")]
    pub const DLL_EXTENSION: &str = os::DLL_EXTENSION;

    /// 指定本平台上可执行二进制文件所用的文件名后缀（如果有的话）。
    /// 一个示例值可能是：`".exe"` 或 `".efi"`。
    ///
    /// 其可能取值与 [`EXE_EXTENSION`] 相同，只是带上了前导的点号。
    #[stable(feature = "env", since = "1.0.0")]
    pub const EXE_SUFFIX: &str = os::EXE_SUFFIX;

    /// 指定本平台上可执行二进制文件所用的文件扩展名（如果有的话）。
    /// 一个示例值可能是：`"exe"` 或空字符串（`""`）。
    ///
    /// <details><summary>可能取值的完整列表</summary>
    ///
    /// * `"bin"`
    /// * `"exe"`
    /// * `"efi"`
    /// * `"js"`
    /// * `"sgxs"`
    /// * `"elf"`
    /// * `"wasm"`
    /// * `""` (an empty string)
    ///
    /// </details>
    #[stable(feature = "env", since = "1.0.0")]
    pub const EXE_EXTENSION: &str = os::EXE_EXTENSION;
}
