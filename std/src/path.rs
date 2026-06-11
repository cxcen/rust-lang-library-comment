//! 跨平台路径处理。
//!
//! 本模块提供两个类型 [`PathBuf`] 与 [`Path`]（类比 [`String`]
//! 与 [`str`]），用于抽象地处理路径。这两个类型是对 [`OsString`] 与 [`OsStr`]
//! 的轻量包装，意味着它们直接按本地平台的路径语法在字符串上工作。
//!
//! 路径可以被解析为一系列 [`Component`]，方式是迭代 [`Path`] 上的 [`components`]
//! 方法所返回的结构。[`Component`] 大致对应路径分隔符（`/` 或 `\`）之间的子串。你可以
//! 用 [`PathBuf`] 上的 [`push`] 方法从各组件重建出一条等价的路径；注意，重建出的路径在
//! 语法上可能因 [`components`] 方法文档中描述的规范化（normalization）而有所不同。
//!
//! ## Case sensitivity
//!
//! 除非另有说明，不访问文件系统的路径方法（例如 [`Path::starts_with`] 与
//! [`Path::ends_with`]）一律是区分大小写的，无论平台或文件系统如何。Windows
//! 盘符是这一规则的一个例外。
//!
//! ## Simple usage
//!
//! 路径处理既包括从切片中解析组件，也包括构建新的、持有所有权的路径。
//!
//! 要解析一条路径，你可以从 [`str`] 切片创建一个 [`Path`] 切片，然后开始查询信息：
//!
//! ```
//! use std::path::Path;
//! use std::ffi::OsStr;
//!
//! let path = Path::new("/tmp/foo/bar.txt");
//!
//! let parent = path.parent();
//! assert_eq!(parent, Some(Path::new("/tmp/foo")));
//!
//! let file_stem = path.file_stem();
//! assert_eq!(file_stem, Some(OsStr::new("bar")));
//!
//! let extension = path.extension();
//! assert_eq!(extension, Some(OsStr::new("txt")));
//! ```
//!
//! 要构建或修改路径，请使用 [`PathBuf`]：
//!
//! ```
//! use std::path::PathBuf;
//!
//! // 这种方式可行……
//! let mut path = PathBuf::from("c:\\");
//!
//! path.push("windows");
//! path.push("system32");
//!
//! path.set_extension("dll");
//!
//! // ……但 push 最适合在你事先并不知道全部内容时使用。如果你事先全都知道，
//! // 下面这种方式更好：
//! let path: PathBuf = ["c:\\", "windows", "system32.dll"].iter().collect();
//! ```
//!
//! [`components`]: Path::components
//! [`push`]: PathBuf::push

#![stable(feature = "rust1", since = "1.0.0")]
#![deny(unsafe_op_in_unsafe_fn)]

use core::clone::CloneToUninit;

use crate::borrow::{Borrow, Cow};
use crate::collections::TryReserveError;
use crate::error::Error;
use crate::ffi::{OsStr, OsString, os_str};
use crate::hash::{Hash, Hasher};
use crate::iter::FusedIterator;
use crate::ops::{self, Deref};
use crate::rc::Rc;
use crate::str::FromStr;
use crate::sync::Arc;
use crate::sys::path::{HAS_PREFIXES, MAIN_SEP_STR, is_sep_byte, is_verbatim_sep, parse_prefix};
use crate::{cmp, fmt, fs, io, sys};

////////////////////////////////////////////////////////////////////////////////
// GENERAL NOTES（总体说明）
////////////////////////////////////////////////////////////////////////////////
//
// 本模块中的解析是通过把 OsStr 直接 transmute 成 [u8] 切片来完成的，这利用了一个事实：
// OsStr 总是把 ASCII 字符原样编码。最终这一 transmute 应当被直接使用 OsStr API 来解析所取代，
// 但要等到那些 API 可用还需要一段时间。
//
// 为什么 Path/PathBuf 不是 str/String：操作系统的路径并不保证是合法的 UTF-8——
// Unix 上路径是任意非零字节序列，Windows 上路径是可能含未配对代理项的 UTF-16。
// 因此路径建立在 OsStr/OsString（而非 str/String）之上，以容纳这些非 UTF-8 的 OS 路径。

////////////////////////////////////////////////////////////////////////////////
// Windows Prefixes（Windows 前缀）
////////////////////////////////////////////////////////////////////////////////

/// Windows 路径前缀，例如 `C:` 或 `\\server\share`。
///
/// Windows 使用多种路径前缀风格，包括对驱动器卷的引用（如 `C:`）、网络共享文件夹
///（如 `\\server\share`）等。此外，某些路径前缀是“逐字”（verbatim，即以 `\\?\` 开头）的，
/// 这种情况下 `/` *不*被当作分隔符，并且基本上不做任何规范化。
///
/// # 示例
///
/// ```
/// use std::path::{Component, Path, Prefix};
/// use std::path::Prefix::*;
/// use std::ffi::OsStr;
///
/// fn get_path_prefix(s: &str) -> Prefix<'_> {
///     let path = Path::new(s);
///     match path.components().next().unwrap() {
///         Component::Prefix(prefix_component) => prefix_component.kind(),
///         _ => panic!(),
///     }
/// }
///
/// # if cfg!(windows) {
/// assert_eq!(Verbatim(OsStr::new("pictures")),
///            get_path_prefix(r"\\?\pictures\kittens"));
/// assert_eq!(VerbatimUNC(OsStr::new("server"), OsStr::new("share")),
///            get_path_prefix(r"\\?\UNC\server\share"));
/// assert_eq!(VerbatimDisk(b'C'), get_path_prefix(r"\\?\c:\"));
/// assert_eq!(DeviceNS(OsStr::new("BrainInterface")),
///            get_path_prefix(r"\\.\BrainInterface"));
/// assert_eq!(UNC(OsStr::new("server"), OsStr::new("share")),
///            get_path_prefix(r"\\server\share"));
/// assert_eq!(Disk(b'C'), get_path_prefix(r"C:\Users\Rust\Pictures\Ferris"));
/// # }
/// ```
#[derive(Copy, Clone, Debug, Hash, PartialOrd, Ord, PartialEq, Eq)]
#[stable(feature = "rust1", since = "1.0.0")]
pub enum Prefix<'a> {
    /// 逐字（verbatim）前缀，例如 `\\?\cat_pics`。
    ///
    /// 逐字前缀由 `\\?\` 紧跟给定组件构成。
    #[stable(feature = "rust1", since = "1.0.0")]
    Verbatim(#[stable(feature = "rust1", since = "1.0.0")] &'a OsStr),

    /// 使用 Windows _**U**niform **N**aming **C**onvention_（统一命名约定）的逐字前缀，
    /// 例如 `\\?\UNC\server\share`。
    ///
    /// 逐字 UNC 前缀由 `\\?\UNC\` 紧跟服务器主机名和共享名构成。
    #[stable(feature = "rust1", since = "1.0.0")]
    VerbatimUNC(
        #[stable(feature = "rust1", since = "1.0.0")] &'a OsStr,
        #[stable(feature = "rust1", since = "1.0.0")] &'a OsStr,
    ),

    /// 逐字盘符前缀，例如 `\\?\C:`。
    ///
    /// 逐字盘符前缀由 `\\?\` 紧跟驱动器盘符和 `:` 构成。
    #[stable(feature = "rust1", since = "1.0.0")]
    VerbatimDisk(#[stable(feature = "rust1", since = "1.0.0")] u8),

    /// 设备命名空间前缀，例如 `\\.\COM42`。
    ///
    /// 设备命名空间前缀由 `\\.\`（可能用 `/` 代替 `\`）紧跟设备名构成。
    #[stable(feature = "rust1", since = "1.0.0")]
    DeviceNS(#[stable(feature = "rust1", since = "1.0.0")] &'a OsStr),

    /// 使用 Windows _**U**niform **N**aming **C**onvention_（统一命名约定）的前缀，例如
    /// `\\server\share`。
    ///
    /// UNC 前缀由服务器主机名和共享名构成。
    #[stable(feature = "rust1", since = "1.0.0")]
    UNC(
        #[stable(feature = "rust1", since = "1.0.0")] &'a OsStr,
        #[stable(feature = "rust1", since = "1.0.0")] &'a OsStr,
    ),

    /// 给定磁盘驱动器的前缀 `C:`。
    #[stable(feature = "rust1", since = "1.0.0")]
    Disk(#[stable(feature = "rust1", since = "1.0.0")] u8),
}

impl<'a> Prefix<'a> {
    #[inline]
    fn len(&self) -> usize {
        use self::Prefix::*;
        fn os_str_len(s: &OsStr) -> usize {
            s.as_encoded_bytes().len()
        }
        match *self {
            Verbatim(x) => 4 + os_str_len(x),
            VerbatimUNC(x, y) => {
                8 + os_str_len(x) + if os_str_len(y) > 0 { 1 + os_str_len(y) } else { 0 }
            }
            VerbatimDisk(_) => 6,
            UNC(x, y) => 2 + os_str_len(x) + if os_str_len(y) > 0 { 1 + os_str_len(y) } else { 0 },
            DeviceNS(x) => 4 + os_str_len(x),
            Disk(_) => 2,
        }
    }

    /// 判定该前缀是否为逐字（verbatim）前缀，即是否以 `\\?\` 开头。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::path::Prefix::*;
    /// use std::ffi::OsStr;
    ///
    /// assert!(Verbatim(OsStr::new("pictures")).is_verbatim());
    /// assert!(VerbatimUNC(OsStr::new("server"), OsStr::new("share")).is_verbatim());
    /// assert!(VerbatimDisk(b'C').is_verbatim());
    /// assert!(!DeviceNS(OsStr::new("BrainInterface")).is_verbatim());
    /// assert!(!UNC(OsStr::new("server"), OsStr::new("share")).is_verbatim());
    /// assert!(!Disk(b'C').is_verbatim());
    /// ```
    #[inline]
    #[must_use]
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn is_verbatim(&self) -> bool {
        use self::Prefix::*;
        matches!(*self, Verbatim(_) | VerbatimDisk(_) | VerbatimUNC(..))
    }

    #[inline]
    fn is_drive(&self) -> bool {
        matches!(*self, Prefix::Disk(_))
    }

    #[inline]
    fn has_implicit_root(&self) -> bool {
        !self.is_drive()
    }
}

////////////////////////////////////////////////////////////////////////////////
// Exposed parsing helpers（对外暴露的解析辅助函数）
////////////////////////////////////////////////////////////////////////////////

/// 判定该字符是否为当前平台所允许的路径分隔符之一。
///
/// # 示例
///
/// ```
/// use std::path;
///
/// assert!(path::is_separator('/')); // '/' 在 Unix 和 Windows 上都有效
/// assert!(!path::is_separator('❤'));
/// ```
#[must_use]
#[stable(feature = "rust1", since = "1.0.0")]
pub fn is_separator(c: char) -> bool {
    c.is_ascii() && is_sep_byte(c as u8)
}

/// 当前平台的主路径组件分隔符。
///
/// 例如，Unix 上为 `/`，Windows 上为 `\`。
#[stable(feature = "rust1", since = "1.0.0")]
#[cfg_attr(not(test), rustc_diagnostic_item = "path_main_separator")]
pub const MAIN_SEPARATOR: char = crate::sys::path::MAIN_SEP;

/// 当前平台的主路径组件分隔符。
///
/// 例如，Unix 上为 `/`，Windows 上为 `\`。
#[stable(feature = "main_separator_str", since = "1.68.0")]
pub const MAIN_SEPARATOR_STR: &str = crate::sys::path::MAIN_SEP_STR;

////////////////////////////////////////////////////////////////////////////////
// Misc helpers（杂项辅助函数）
////////////////////////////////////////////////////////////////////////////////

// 在 `iter` 与 `prefix` 匹配的过程中持续迭代 `iter`；若 `prefix` 不是 `iter` 的前缀则返回
// `None`，否则返回 `Some(iter_after_prefix)`，给出耗尽 `prefix` 之后的 `iter`。
fn iter_after<'a, 'b, I, J>(mut iter: I, mut prefix: J) -> Option<I>
where
    I: Iterator<Item = Component<'a>> + Clone,
    J: Iterator<Item = Component<'b>>,
{
    loop {
        let mut iter_next = iter.clone();
        match (iter_next.next(), prefix.next()) {
            (Some(ref x), Some(ref y)) if x == y => (),
            (Some(_), Some(_)) => return None,
            (Some(_), None) => return Some(iter),
            (None, None) => return Some(iter),
            (None, Some(_)) => return None,
        }
        iter = iter_next;
    }
}

////////////////////////////////////////////////////////////////////////////////
// Cross-platform, iterator-independent parsing（跨平台、与迭代器无关的解析）
////////////////////////////////////////////////////////////////////////////////

/// 判断前缀之后的第一个字节是否为分隔符。
fn has_physical_root(s: &[u8], prefix: Option<Prefix<'_>>) -> bool {
    let path = if let Some(p) = prefix { &s[p.len()..] } else { s };
    !path.is_empty() && is_sep_byte(path[0])
}

// 用于拆分主干（stem）与扩展名的基础工作函数
fn rsplit_file_at_dot(file: &OsStr) -> (Option<&OsStr>, Option<&OsStr>) {
    if file.as_encoded_bytes() == b".." {
        return (Some(file), None);
    }

    // 这里的 unsafe 源自在 &OsStr 与 &[u8] 之间来回转换。这样做是安全的，因为
    //（1）我们只查看编码中的 ASCII 内容，并且（2）新的 &OsStr 值只从已有 &OsStr 值
    // 的 ASCII 边界切片中产生。
    let mut iter = file.as_encoded_bytes().rsplitn(2, |b| *b == b'.');
    let after = iter.next();
    let before = iter.next();
    if before == Some(b"") {
        (Some(file), None)
    } else {
        unsafe {
            (
                before.map(|s| OsStr::from_encoded_bytes_unchecked(s)),
                after.map(|s| OsStr::from_encoded_bytes_unchecked(s)),
            )
        }
    }
}

fn split_file_at_dot(file: &OsStr) -> (&OsStr, Option<&OsStr>) {
    let slice = file.as_encoded_bytes();
    if slice == b".." {
        return (file, None);
    }

    // 这里的 unsafe 源自在 &OsStr 与 &[u8] 之间来回转换。这样做是安全的，因为
    //（1）我们只查看编码中的 ASCII 内容，并且（2）新的 &OsStr 值只从已有 &OsStr 值
    // 的 ASCII 边界切片中产生。
    let i = match slice[1..].iter().position(|b| *b == b'.') {
        Some(i) => i + 1,
        None => return (file, None),
    };
    let before = &slice[..i];
    let after = &slice[i + 1..];
    unsafe {
        (
            OsStr::from_encoded_bytes_unchecked(before),
            Some(OsStr::from_encoded_bytes_unchecked(after)),
        )
    }
}

/// 检查该字符串是否可作为合法的文件扩展名，否则 panic。
fn validate_extension(extension: &OsStr) {
    for &b in extension.as_encoded_bytes() {
        if is_sep_byte(b) {
            panic!("extension cannot contain path separators: {extension:?}");
        }
    }
}

////////////////////////////////////////////////////////////////////////////////
// The core iterators（核心迭代器）
////////////////////////////////////////////////////////////////////////////////

/// 组件解析通过一个双端状态机完成；位于路径头部和尾部的两个游标各自跟踪当前已消费的
/// 路径部分。
///
/// 从头到尾看，一条路径由一个前缀（prefix）、一个起始目录组件（starting directory），
/// 以及一个主体（body，由普通组件构成）组成。
#[derive(Copy, Clone, PartialEq, PartialOrd, Debug)]
enum State {
    Prefix = 0,   // c:
    StartDir = 1, // / 或 . 或 空
    Body = 2,     // foo/bar/baz
    Done = 3,
}

/// 一个结构体，包装了 Windows 路径前缀以及它未经解析的字符串表示。
///
/// 除了由 [`kind`] 返回的已解析 [`Prefix`] 信息之外，`PrefixComponent`
/// 还持有原始的、未解析的 [`OsStr`] 切片，由 [`as_os_str`] 返回。
///
/// 该 `struct` 的实例可以通过对 [`Component`] 上的 [`Prefix` variant]
/// 进行匹配来获得。
///
/// 在 Unix 上不会出现。
///
/// # 示例
///
/// ```
/// # if cfg!(windows) {
/// use std::path::{Component, Path, Prefix};
/// use std::ffi::OsStr;
///
/// let path = Path::new(r"c:\you\later\");
/// match path.components().next().unwrap() {
///     Component::Prefix(prefix_component) => {
///         assert_eq!(Prefix::Disk(b'C'), prefix_component.kind());
///         assert_eq!(OsStr::new("c:"), prefix_component.as_os_str());
///     }
///     _ => unreachable!(),
/// }
/// # }
/// ```
///
/// [`as_os_str`]: PrefixComponent::as_os_str
/// [`kind`]: PrefixComponent::kind
/// [`Prefix` variant]: Component::Prefix
#[stable(feature = "rust1", since = "1.0.0")]
#[derive(Copy, Clone, Eq, Debug)]
pub struct PrefixComponent<'a> {
    /// 未经解析的前缀 `OsStr` 切片。
    raw: &'a OsStr,

    /// 已解析的前缀数据。
    parsed: Prefix<'a>,
}

impl<'a> PrefixComponent<'a> {
    /// 返回已解析的前缀数据。
    ///
    /// 关于不同种类前缀的更多信息，参见 [`Prefix`] 的文档。
    #[stable(feature = "rust1", since = "1.0.0")]
    #[must_use]
    #[inline]
    pub fn kind(&self) -> Prefix<'a> {
        self.parsed
    }

    /// 返回该前缀的原始 [`OsStr`] 切片。
    #[stable(feature = "rust1", since = "1.0.0")]
    #[must_use]
    #[inline]
    pub fn as_os_str(&self) -> &'a OsStr {
        self.raw
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<'a> PartialEq for PrefixComponent<'a> {
    #[inline]
    fn eq(&self, other: &PrefixComponent<'a>) -> bool {
        self.parsed == other.parsed
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<'a> PartialOrd for PrefixComponent<'a> {
    #[inline]
    fn partial_cmp(&self, other: &PrefixComponent<'a>) -> Option<cmp::Ordering> {
        PartialOrd::partial_cmp(&self.parsed, &other.parsed)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl Ord for PrefixComponent<'_> {
    #[inline]
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        Ord::cmp(&self.parsed, &other.parsed)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl Hash for PrefixComponent<'_> {
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.parsed.hash(h);
    }
}

/// 路径的单个组件。
///
/// 一个 `Component` 大致对应路径分隔符（`/` 或 `\`）之间的子串。
///
/// 该 `enum` 通过迭代 [`Components`] 而产生，而 [`Components`] 又由 [`Path`] 上的
/// [`components`](Path::components) 方法创建。
///
/// # 示例
///
/// ```rust
/// use std::path::{Component, Path};
///
/// let path = Path::new("/tmp/foo/bar.txt");
/// let components = path.components().collect::<Vec<_>>();
/// assert_eq!(&components, &[
///     Component::RootDir,
///     Component::Normal("tmp".as_ref()),
///     Component::Normal("foo".as_ref()),
///     Component::Normal("bar.txt".as_ref()),
/// ]);
/// ```
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
#[stable(feature = "rust1", since = "1.0.0")]
pub enum Component<'a> {
    /// 一个 Windows 路径前缀，例如 `C:` 或 `\\server\share`。
    ///
    /// 前缀类型种类繁多，更多内容参见 [`Prefix`] 的文档。
    ///
    /// 在 Unix 上不会出现。
    #[stable(feature = "rust1", since = "1.0.0")]
    Prefix(#[stable(feature = "rust1", since = "1.0.0")] PrefixComponent<'a>),

    /// 根目录组件，出现在任何前缀之后、其余一切之前。
    ///
    /// 它表示一个标识路径从根开始的分隔符。
    #[stable(feature = "rust1", since = "1.0.0")]
    RootDir,

    /// 对当前目录的引用，即 `.`。
    #[stable(feature = "rust1", since = "1.0.0")]
    CurDir,

    /// 对父目录的引用，即 `..`。
    #[stable(feature = "rust1", since = "1.0.0")]
    ParentDir,

    /// 一个普通组件，例如 `a/b` 中的 `a` 和 `b`。
    ///
    /// 这是最常见的变体，表示对文件或目录的引用。
    #[stable(feature = "rust1", since = "1.0.0")]
    Normal(#[stable(feature = "rust1", since = "1.0.0")] &'a OsStr),
}

impl<'a> Component<'a> {
    /// 提取底层的 [`OsStr`] 切片。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::path::Path;
    ///
    /// let path = Path::new("./tmp/foo/bar.txt");
    /// let components: Vec<_> = path.components().map(|comp| comp.as_os_str()).collect();
    /// assert_eq!(&components, &[".", "tmp", "foo", "bar.txt"]);
    /// ```
    #[must_use = "`self` will be dropped if the result is not used"]
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn as_os_str(self) -> &'a OsStr {
        match self {
            Component::Prefix(p) => p.as_os_str(),
            Component::RootDir => OsStr::new(MAIN_SEP_STR),
            Component::CurDir => OsStr::new("."),
            Component::ParentDir => OsStr::new(".."),
            Component::Normal(path) => path,
        }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl AsRef<OsStr> for Component<'_> {
    #[inline]
    fn as_ref(&self) -> &OsStr {
        self.as_os_str()
    }
}

#[stable(feature = "path_component_asref", since = "1.25.0")]
impl AsRef<Path> for Component<'_> {
    #[inline]
    fn as_ref(&self) -> &Path {
        self.as_os_str().as_ref()
    }
}

/// 一个遍历 [`Path`] 的各 [`Component`] 的迭代器。
///
/// 该 `struct` 由 [`Path`] 上的 [`components`] 方法创建。
/// 更多内容参见其文档。
///
/// # 示例
///
/// ```
/// use std::path::Path;
///
/// let path = Path::new("/tmp/foo/bar.txt");
///
/// for component in path.components() {
///     println!("{component:?}");
/// }
/// ```
///
/// [`components`]: Path::components
#[derive(Clone)]
#[must_use = "iterators are lazy and do nothing unless consumed"]
#[stable(feature = "rust1", since = "1.0.0")]
pub struct Components<'a> {
    // 尚待解析出组件的剩余路径
    path: &'a [u8],

    // 最初解析出的前缀（如果有的话）
    prefix: Option<Prefix<'a>>,

    // 当路径*物理上*带有根分隔符时为 true；对大多数 Windows 前缀而言，出于规范化目的它可能带有
    // 一个“逻辑”根分隔符，例如 \\server\share == \\server\share\。
    has_physical_root: bool,

    // 迭代器是双端的，下面这两个状态分别跟踪从首端和尾端已经产出了哪些内容
    front: State,
    back: State,
}

/// 一个把 [`Path`] 的各 [`Component`] 作为 [`OsStr`] 切片来遍历的迭代器。
///
/// 该 `struct` 由 [`Path`] 上的 [`iter`] 方法创建。
/// 更多内容参见其文档。
///
/// [`iter`]: Path::iter
#[derive(Clone)]
#[must_use = "iterators are lazy and do nothing unless consumed"]
#[stable(feature = "rust1", since = "1.0.0")]
pub struct Iter<'a> {
    inner: Components<'a>,
}

#[stable(feature = "path_components_debug", since = "1.13.0")]
impl fmt::Debug for Components<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        struct DebugHelper<'a>(&'a Path);

        impl fmt::Debug for DebugHelper<'_> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_list().entries(self.0.components()).finish()
            }
        }

        f.debug_tuple("Components").field(&DebugHelper(self.as_path())).finish()
    }
}

impl<'a> Components<'a> {
    // 前缀有多长（如果有的话）？
    #[inline]
    fn prefix_len(&self) -> usize {
        if !HAS_PREFIXES {
            return 0;
        }
        self.prefix.as_ref().map(Prefix::len).unwrap_or(0)
    }

    #[inline]
    fn prefix_verbatim(&self) -> bool {
        if !HAS_PREFIXES {
            return false;
        }
        self.prefix.as_ref().map(Prefix::is_verbatim).unwrap_or(false)
    }

    /// 从迭代的角度看，前缀还剩多少未消费？
    #[inline]
    fn prefix_remaining(&self) -> usize {
        if !HAS_PREFIXES {
            return 0;
        }
        if self.front == State::Prefix { self.prefix_len() } else { 0 }
    }

    // 在已有的迭代进度下，State::Body 之前的部分还剩多少？
    #[inline]
    fn len_before_body(&self) -> usize {
        let root = if self.front <= State::StartDir && self.has_physical_root { 1 } else { 0 };
        let cur_dir = if self.front <= State::StartDir && self.include_cur_dir() { 1 } else { 0 };
        self.prefix_remaining() + root + cur_dir
    }

    // 迭代是否已完成？
    #[inline]
    fn finished(&self) -> bool {
        self.front == State::Done || self.back == State::Done || self.front > self.back
    }

    #[inline]
    fn is_sep_byte(&self, b: u8) -> bool {
        if self.prefix_verbatim() { is_verbatim_sep(b) } else { is_sep_byte(b) }
    }

    /// 提取出对应于剩余待迭代路径部分的切片。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::path::Path;
    ///
    /// let mut components = Path::new("/tmp/foo/bar.txt").components();
    /// components.next();
    /// components.next();
    ///
    /// assert_eq!(Path::new("foo/bar.txt"), components.as_path());
    /// ```
    #[must_use]
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn as_path(&self) -> &'a Path {
        let mut comps = self.clone();
        if comps.front == State::Body {
            comps.trim_left();
        }
        if comps.back == State::Body {
            comps.trim_right();
        }
        unsafe { Path::from_u8_slice(comps.path) }
    }

    /// *原始*路径是否带有根？
    fn has_root(&self) -> bool {
        if self.has_physical_root {
            return true;
        }
        if HAS_PREFIXES && let Some(p) = self.prefix {
            if p.has_implicit_root() {
                return true;
            }
        }
        false
    }

    /// 规范化后的路径是否应包含一个前导的 . ？
    fn include_cur_dir(&self) -> bool {
        if self.has_root() {
            return false;
        }
        let slice = &self.path[self.prefix_remaining()..];
        match slice {
            [b'.'] => true,
            [b'.', b, ..] => self.is_sep_byte(*b),
            _ => false,
        }
    }

    // 按照 OsStr 编码解析给定的字节序列，得到对应的路径组件
    unsafe fn parse_single_component<'b>(&self, comp: &'b [u8]) -> Option<Component<'b>> {
        match comp {
            b"." if HAS_PREFIXES && self.prefix_verbatim() => Some(Component::CurDir),
            b"." => None, // . 组件会被规范化掉，但路径开头的 . 除外，
            // 后者通过 `include_cur_dir` 单独处理
            b".." => Some(Component::ParentDir),
            b"" => None,
            _ => Some(Component::Normal(unsafe { OsStr::from_encoded_bytes_unchecked(comp) })),
        }
    }

    // 从左侧解析一个组件，并给出移除该组件需要消费多少字节
    fn parse_next_component(&self) -> (usize, Option<Component<'a>>) {
        debug_assert!(self.front == State::Body);
        let (extra, comp) = match self.path.iter().position(|b| self.is_sep_byte(*b)) {
            None => (0, self.path),
            Some(i) => (1, &self.path[..i]),
        };
        // SAFETY: `comp` 是一个合法的子串，因为它是按分隔符切分出来的。
        (comp.len() + extra, unsafe { self.parse_single_component(comp) })
    }

    // 从右侧解析一个组件，并给出移除该组件需要消费多少字节
    fn parse_next_component_back(&self) -> (usize, Option<Component<'a>>) {
        debug_assert!(self.back == State::Body);
        let start = self.len_before_body();
        let (extra, comp) = match self.path[start..].iter().rposition(|b| self.is_sep_byte(*b)) {
            None => (0, &self.path[start..]),
            Some(i) => (1, &self.path[start + i + 1..]),
        };
        // SAFETY: `comp` 是一个合法的子串，因为它是按分隔符切分出来的。
        (comp.len() + extra, unsafe { self.parse_single_component(comp) })
    }

    // 修剪掉左侧重复的分隔符（即空组件）
    fn trim_left(&mut self) {
        while !self.path.is_empty() {
            let (size, comp) = self.parse_next_component();
            if comp.is_some() {
                return;
            } else {
                self.path = &self.path[size..];
            }
        }
    }

    // 修剪掉右侧重复的分隔符（即空组件）
    fn trim_right(&mut self) {
        while self.path.len() > self.len_before_body() {
            let (size, comp) = self.parse_next_component_back();
            if comp.is_some() {
                return;
            } else {
                self.path = &self.path[..self.path.len() - size];
            }
        }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl AsRef<Path> for Components<'_> {
    #[inline]
    fn as_ref(&self) -> &Path {
        self.as_path()
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl AsRef<OsStr> for Components<'_> {
    #[inline]
    fn as_ref(&self) -> &OsStr {
        self.as_path().as_os_str()
    }
}

#[stable(feature = "path_iter_debug", since = "1.13.0")]
impl fmt::Debug for Iter<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        struct DebugHelper<'a>(&'a Path);

        impl fmt::Debug for DebugHelper<'_> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.debug_list().entries(self.0.iter()).finish()
            }
        }

        f.debug_tuple("Iter").field(&DebugHelper(self.as_path())).finish()
    }
}

impl<'a> Iter<'a> {
    /// 提取出对应于剩余待迭代路径部分的切片。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::path::Path;
    ///
    /// let mut iter = Path::new("/tmp/foo/bar.txt").iter();
    /// iter.next();
    /// iter.next();
    ///
    /// assert_eq!(Path::new("foo/bar.txt"), iter.as_path());
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[must_use]
    #[inline]
    pub fn as_path(&self) -> &'a Path {
        self.inner.as_path()
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl AsRef<Path> for Iter<'_> {
    #[inline]
    fn as_ref(&self) -> &Path {
        self.as_path()
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl AsRef<OsStr> for Iter<'_> {
    #[inline]
    fn as_ref(&self) -> &OsStr {
        self.as_path().as_os_str()
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<'a> Iterator for Iter<'a> {
    type Item = &'a OsStr;

    #[inline]
    fn next(&mut self) -> Option<&'a OsStr> {
        self.inner.next().map(Component::as_os_str)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<'a> DoubleEndedIterator for Iter<'a> {
    #[inline]
    fn next_back(&mut self) -> Option<&'a OsStr> {
        self.inner.next_back().map(Component::as_os_str)
    }
}

#[stable(feature = "fused", since = "1.26.0")]
impl FusedIterator for Iter<'_> {}

#[stable(feature = "rust1", since = "1.0.0")]
impl<'a> Iterator for Components<'a> {
    type Item = Component<'a>;

    fn next(&mut self) -> Option<Component<'a>> {
        while !self.finished() {
            match self.front {
                // 最有可能的情况放在最前面
                State::Body if !self.path.is_empty() => {
                    let (size, comp) = self.parse_next_component();
                    self.path = &self.path[size..];
                    if comp.is_some() {
                        return comp;
                    }
                }
                State::Body => {
                    self.front = State::Done;
                }
                State::StartDir => {
                    self.front = State::Body;
                    if self.has_physical_root {
                        debug_assert!(!self.path.is_empty());
                        self.path = &self.path[1..];
                        return Some(Component::RootDir);
                    } else if HAS_PREFIXES && let Some(p) = self.prefix {
                        if p.has_implicit_root() && !p.is_verbatim() {
                            return Some(Component::RootDir);
                        }
                    } else if self.include_cur_dir() {
                        debug_assert!(!self.path.is_empty());
                        self.path = &self.path[1..];
                        return Some(Component::CurDir);
                    }
                }
                _ if const { !HAS_PREFIXES } => unreachable!(),
                State::Prefix if self.prefix_len() == 0 => {
                    self.front = State::StartDir;
                }
                State::Prefix => {
                    self.front = State::StartDir;
                    debug_assert!(self.prefix_len() <= self.path.len());
                    let raw = &self.path[..self.prefix_len()];
                    self.path = &self.path[self.prefix_len()..];
                    return Some(Component::Prefix(PrefixComponent {
                        raw: unsafe { OsStr::from_encoded_bytes_unchecked(raw) },
                        parsed: self.prefix.unwrap(),
                    }));
                }
                State::Done => unreachable!(),
            }
        }
        None
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<'a> DoubleEndedIterator for Components<'a> {
    fn next_back(&mut self) -> Option<Component<'a>> {
        while !self.finished() {
            match self.back {
                State::Body if self.path.len() > self.len_before_body() => {
                    let (size, comp) = self.parse_next_component_back();
                    self.path = &self.path[..self.path.len() - size];
                    if comp.is_some() {
                        return comp;
                    }
                }
                State::Body => {
                    self.back = State::StartDir;
                }
                State::StartDir => {
                    self.back = if HAS_PREFIXES { State::Prefix } else { State::Done };
                    if self.has_physical_root {
                        self.path = &self.path[..self.path.len() - 1];
                        return Some(Component::RootDir);
                    } else if HAS_PREFIXES && let Some(p) = self.prefix {
                        if p.has_implicit_root() && !p.is_verbatim() {
                            return Some(Component::RootDir);
                        }
                    } else if self.include_cur_dir() {
                        self.path = &self.path[..self.path.len() - 1];
                        return Some(Component::CurDir);
                    }
                }
                _ if !HAS_PREFIXES => unreachable!(),
                State::Prefix if self.prefix_len() > 0 => {
                    self.back = State::Done;
                    return Some(Component::Prefix(PrefixComponent {
                        raw: unsafe { OsStr::from_encoded_bytes_unchecked(self.path) },
                        parsed: self.prefix.unwrap(),
                    }));
                }
                State::Prefix => {
                    self.back = State::Done;
                    return None;
                }
                State::Done => unreachable!(),
            }
        }
        None
    }
}

#[stable(feature = "fused", since = "1.26.0")]
impl FusedIterator for Components<'_> {}

#[stable(feature = "rust1", since = "1.0.0")]
impl<'a> PartialEq for Components<'a> {
    #[inline]
    fn eq(&self, other: &Components<'a>) -> bool {
        let Components { path: _, front: _, back: _, has_physical_root: _, prefix: _ } = self;

        // 针对完全匹配的快速路径，例如用于 hashmap 查找。
        // 不显式比较 prefix 或 has_physical_root 字段，因为它们要么已经被 `path` 缓冲区涵盖，
        // 要么只对 `prefix_verbatim()` 有意义。
        if self.path.len() == other.path.len()
            && self.front == other.front
            && self.back == State::Body
            && other.back == State::Body
            && self.prefix_verbatim() == other.prefix_verbatim()
        {
            // 未来可能的改进：如果有一个从后往前比较的 memcmp/bcmp，这里可以更早地短路退出
            if self.path == other.path {
                return true;
            }
        }

        // 从后往前比较，因为绝对路径常常共享很长的前缀
        Iterator::eq(self.clone().rev(), other.clone().rev())
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl Eq for Components<'_> {}

#[stable(feature = "rust1", since = "1.0.0")]
impl<'a> PartialOrd for Components<'a> {
    #[inline]
    fn partial_cmp(&self, other: &Components<'a>) -> Option<cmp::Ordering> {
        Some(compare_components(self.clone(), other.clone()))
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl Ord for Components<'_> {
    #[inline]
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        compare_components(self.clone(), other.clone())
    }
}

fn compare_components(mut left: Components<'_>, mut right: Components<'_>) -> cmp::Ordering {
    // 针对很长的共享前缀的快速路径
    //
    // - 比较原始字节，找到第一个不匹配之处
    // - 回退到不匹配位置之前的分隔符，以避免对 '.' 或 '..' 字符产生歧义解析
    // - 如果找到分隔符，则更新状态，使之后只对剩余部分做逐组件比较；
    //   否则就对整条路径做逐组件比较
    //
    // 对带有 PrefixComponent 的路径不走快速路径，以免回退到某个前缀的中间位置
    if left.prefix.is_none() && right.prefix.is_none() && left.front == right.front {
        // 未来可能的改进：一个 [u8]::first_mismatch 的 simd 实现
        let first_difference = match left.path.iter().zip(right.path).position(|(&a, &b)| a != b) {
            None if left.path.len() == right.path.len() => return cmp::Ordering::Equal,
            None => left.path.len().min(right.path.len()),
            Some(diff) => diff,
        };

        if let Some(previous_sep) =
            left.path[..first_difference].iter().rposition(|&b| left.is_sep_byte(b))
        {
            let mismatched_component_start = previous_sep + 1;
            left.path = &left.path[mismatched_component_start..];
            left.front = State::Body;
            right.path = &right.path[mismatched_component_start..];
            right.front = State::Body;
        }
    }

    Iterator::cmp(left, right)
}

/// 一个遍历 [`Path`] 及其各级祖先的迭代器。
///
/// 该 `struct` 由 [`Path`] 上的 [`ancestors`] 方法创建。
/// 更多内容参见其文档。
///
/// # 示例
///
/// ```
/// use std::path::Path;
///
/// let path = Path::new("/foo/bar");
///
/// for ancestor in path.ancestors() {
///     println!("{}", ancestor.display());
/// }
/// ```
///
/// [`ancestors`]: Path::ancestors
#[derive(Copy, Clone, Debug)]
#[must_use = "iterators are lazy and do nothing unless consumed"]
#[stable(feature = "path_ancestors", since = "1.28.0")]
pub struct Ancestors<'a> {
    next: Option<&'a Path>,
}

#[stable(feature = "path_ancestors", since = "1.28.0")]
impl<'a> Iterator for Ancestors<'a> {
    type Item = &'a Path;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let next = self.next;
        self.next = next.and_then(Path::parent);
        next
    }
}

#[stable(feature = "path_ancestors", since = "1.28.0")]
impl FusedIterator for Ancestors<'_> {}

////////////////////////////////////////////////////////////////////////////////
// Basic types and traits（基础类型与 trait）
////////////////////////////////////////////////////////////////////////////////

/// 一个持有所有权、可变的路径（类比 [`String`]）。
///
/// 该类型提供了诸如 [`push`] 与 [`set_extension`] 这样的方法，用于就地修改路径。
/// 它还实现了到 [`Path`] 的 [`Deref`]，意味着 [`Path`] 切片上的所有方法在 `PathBuf`
/// 值上同样可用。
///
/// [`push`]: PathBuf::push
/// [`set_extension`]: PathBuf::set_extension
///
/// 关于整体设计思路的更多细节，参见[模块级文档](self)。
///
/// # 示例
///
/// 你可以用 [`push`] 从各组件逐步构建出一个 `PathBuf`：
///
/// ```
/// use std::path::PathBuf;
///
/// let mut path = PathBuf::new();
///
/// path.push(r"C:\");
/// path.push("windows");
/// path.push("system32");
///
/// path.set_extension("dll");
/// ```
///
/// 不过，[`push`] 最适合用于动态场景。当你事先知道全部组件时，下面这种方式更好：
///
/// ```
/// use std::path::PathBuf;
///
/// let path: PathBuf = [r"C:\", "windows", "system32.dll"].iter().collect();
/// ```
///
/// 我们还能做得更好！既然这些都是字符串，我们可以使用 `From::from`：
///
/// ```
/// use std::path::PathBuf;
///
/// let path = PathBuf::from(r"C:\windows\system32.dll");
/// ```
///
/// 哪种方式最合适取决于你所处的具体场景。
///
/// 注意，`PathBuf` 并不总是对参数进行净化（sanitize），例如 [`push`]
/// 允许从含有分隔符的字符串构建路径：
///
/// ```
/// use std::path::PathBuf;
///
/// let mut path = PathBuf::new();
///
/// path.push(r"C:\");
/// path.push("windows");
/// path.push(r"..\otherdir");
/// path.push("system32");
/// ```
///
/// `PathBuf` 对于这类输入的行为在将来可能改为 panic。应当使用 [`Extend::extend`]
/// 来添加多段路径。
#[cfg_attr(not(test), rustc_diagnostic_item = "PathBuf")]
#[stable(feature = "rust1", since = "1.0.0")]
pub struct PathBuf {
    inner: OsString,
}

impl PathBuf {
    /// 分配一个空的 `PathBuf`。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::path::PathBuf;
    ///
    /// let path = PathBuf::new();
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[must_use]
    #[inline]
    #[rustc_const_stable(feature = "const_pathbuf_osstring_new", since = "1.91.0")]
    pub const fn new() -> PathBuf {
        PathBuf { inner: OsString::new() }
    }

    /// 创建一个新的 `PathBuf`，并以给定容量来创建内部的 [`OsString`]。
    /// 参见 [`OsString`] 上定义的 [`with_capacity`]。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::path::PathBuf;
    ///
    /// let mut path = PathBuf::with_capacity(10);
    /// let capacity = path.capacity();
    ///
    /// // 这次 push 不会重新分配
    /// path.push(r"C:\");
    ///
    /// assert_eq!(capacity, path.capacity());
    /// ```
    ///
    /// [`with_capacity`]: OsString::with_capacity
    #[stable(feature = "path_buf_capacity", since = "1.44.0")]
    #[must_use]
    #[inline]
    pub fn with_capacity(capacity: usize) -> PathBuf {
        PathBuf { inner: OsString::with_capacity(capacity) }
    }

    /// 强制转换为一个 [`Path`] 切片。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::path::{Path, PathBuf};
    ///
    /// let p = PathBuf::from("/test");
    /// assert_eq!(Path::new("/test"), p.as_path());
    /// ```
    #[cfg_attr(not(test), rustc_diagnostic_item = "pathbuf_as_path")]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[must_use]
    #[inline]
    pub fn as_path(&self) -> &Path {
        self
    }

    /// 消耗 `PathBuf` 并将其泄漏（leak），返回对内容的可变引用 `&'a mut Path`。
    ///
    /// 调用者可自由选择返回的生命周期，包括 'static。实际上，本函数最理想的用法是处理
    /// 生命周期持续到程序结束的数据，因为丢弃返回的引用会造成内存泄漏。
    ///
    /// 它不会重新分配或收缩 `PathBuf`，因此泄漏出的分配可能包含不属于返回切片的未用容量。
    /// 如果你想丢弃多余的容量，请改为调用 [`into_boxed_path`]，再调用 [`Box::leak`]。
    /// 但请记住，修剪容量可能导致一次重新分配和拷贝。
    ///
    /// [`into_boxed_path`]: Self::into_boxed_path
    #[stable(feature = "os_string_pathbuf_leak", since = "1.89.0")]
    #[inline]
    pub fn leak<'a>(self) -> &'a mut Path {
        Path::from_inner_mut(self.inner.leak())
    }

    /// 用 `path` 扩展 `self`。
    ///
    /// 如果 `path` 是绝对路径，它会替换掉当前路径。
    ///
    /// 在 Windows 上：
    ///
    /// * 如果 `path` 带有根但没有前缀（例如 `\windows`），它会替换掉 `self`
    ///   的前缀（如果有的话）之外的一切。
    /// * 如果 `path` 带有前缀但没有根，它会替换掉 `self`。
    /// * 如果 `self` 带有逐字前缀（例如 `\\?\C:\windows`）且 `path` 非空，
    ///   则新路径会被规范化：所有对 `.` 和 `..` 的引用都会被移除。
    ///
    /// 如果你需要一个新的 `PathBuf` 而非在克隆出的 `PathBuf` 上调用本函数，
    /// 考虑使用 [`Path::join`]。
    ///
    /// # 示例
    ///
    /// push 一个相对路径会扩展已有路径：
    ///
    /// ```
    /// use std::path::PathBuf;
    ///
    /// let mut path = PathBuf::from("/tmp");
    /// path.push("file.bk");
    /// assert_eq!(path, PathBuf::from("/tmp/file.bk"));
    /// ```
    ///
    /// push 一个绝对路径会替换已有路径：
    ///
    /// ```
    /// use std::path::PathBuf;
    ///
    /// let mut path = PathBuf::from("/tmp");
    /// path.push("/etc");
    /// assert_eq!(path, PathBuf::from("/etc"));
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_confusables("append", "put")]
    pub fn push<P: AsRef<Path>>(&mut self, path: P) {
        self._push(path.as_ref())
    }

    fn _push(&mut self, path: &Path) {
        // 一般而言，当最右侧的字节不是分隔符时，就需要补一个分隔符
        let buf = self.inner.as_encoded_bytes();
        let mut need_sep = buf.last().map(|c| !is_sep_byte(*c)).unwrap_or(false);

        // 在 Windows 上 `C:` 这种特殊情形下，*不*添加分隔符
        let comps = self.components();

        if comps.prefix_len() > 0
            && comps.prefix_len() == comps.path.len()
            && comps.prefix.unwrap().is_drive()
        {
            need_sep = false
        }

        let need_clear = if cfg!(target_os = "cygwin") {
            // 如果 path 是绝对路径且没有前缀，它形如 `/foo`，
            // 会在下面处理。
            path.prefix().is_some()
        } else {
            // 在 Unix 上：前缀总是 None。
            path.is_absolute() || path.prefix().is_some()
        };

        // 绝对的 `path` 会替换掉 `self`
        if need_clear {
            self.inner.truncate(0);

        // 逐字路径需要移除 . 和 ..
        } else if comps.prefix_verbatim() && !path.inner.is_empty() {
            let mut buf: Vec<_> = comps.collect();
            for c in path.components() {
                match c {
                    Component::RootDir => {
                        buf.truncate(1);
                        buf.push(c);
                    }
                    Component::CurDir => (),
                    Component::ParentDir => {
                        if let Some(Component::Normal(_)) = buf.last() {
                            buf.pop();
                        }
                    }
                    _ => buf.push(c),
                }
            }

            let mut res = OsString::new();
            let mut need_sep = false;

            for c in buf {
                if need_sep && c != Component::RootDir {
                    res.push(MAIN_SEP_STR);
                }
                res.push(c.as_os_str());

                need_sep = match c {
                    Component::RootDir => false,
                    Component::Prefix(prefix) => {
                        !prefix.parsed.is_drive() && prefix.parsed.len() > 0
                    }
                    _ => true,
                }
            }

            self.inner = res;
            return;

        // `path` 带有根但没有前缀，例如 `\windows`（仅限 Windows）
        } else if path.has_root() {
            let prefix_len = self.components().prefix_remaining();
            self.inner.truncate(prefix_len);

        // `path` 是一条纯粹的相对路径
        } else if need_sep {
            self.inner.push(MAIN_SEP_STR);
        }

        self.inner.push(path);
    }

    /// 将 `self` 截断到 [`self.parent`]。
    ///
    /// 若 [`self.parent`] 为 [`None`]，则返回 `false` 且什么都不做。
    /// 否则返回 `true`。
    ///
    /// [`self.parent`]: Path::parent
    ///
    /// # 示例
    ///
    /// ```
    /// use std::path::{Path, PathBuf};
    ///
    /// let mut p = PathBuf::from("/spirited/away.rs");
    ///
    /// p.pop();
    /// assert_eq!(Path::new("/spirited"), p);
    /// p.pop();
    /// assert_eq!(Path::new("/"), p);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn pop(&mut self) -> bool {
        match self.parent().map(|p| p.as_u8_slice().len()) {
            Some(len) => {
                self.inner.truncate(len);
                true
            }
            None => false,
        }
    }

    /// 设置该路径是否带有尾部[分隔符](MAIN_SEPARATOR)。
    ///
    /// 在可能的情况下，[`has_trailing_sep`](Path::has_trailing_sep) 返回的值将与所提供的值一致。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(path_trailing_sep)]
    /// use std::path::PathBuf;
    ///
    /// let mut p = PathBuf::from("dir");
    ///
    /// assert!(!p.has_trailing_sep());
    /// p.set_trailing_sep(false);
    /// assert!(!p.has_trailing_sep());
    /// p.set_trailing_sep(true);
    /// assert!(p.has_trailing_sep());
    /// p.set_trailing_sep(false);
    /// assert!(!p.has_trailing_sep());
    ///
    /// p = PathBuf::from("/");
    /// assert!(p.has_trailing_sep());
    /// p.set_trailing_sep(false);
    /// assert!(p.has_trailing_sep());
    /// ```
    #[unstable(feature = "path_trailing_sep", issue = "142503")]
    pub fn set_trailing_sep(&mut self, trailing_sep: bool) {
        if trailing_sep { self.push_trailing_sep() } else { self.pop_trailing_sep() }
    }

    /// 给路径添加一个尾部[分隔符](MAIN_SEPARATOR)。
    ///
    /// 它的作用类似于 [`Path::with_trailing_sep`]，但会就地修改底层的 `PathBuf`。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(path_trailing_sep)]
    /// use std::ffi::OsStr;
    /// use std::path::PathBuf;
    ///
    /// let mut p = PathBuf::from("dir");
    ///
    /// assert!(!p.has_trailing_sep());
    /// p.push_trailing_sep();
    /// assert!(p.has_trailing_sep());
    /// p.push_trailing_sep();
    /// assert!(p.has_trailing_sep());
    ///
    /// p = PathBuf::from("dir/");
    /// p.push_trailing_sep();
    /// assert_eq!(p.as_os_str(), OsStr::new("dir/"));
    /// ```
    #[unstable(feature = "path_trailing_sep", issue = "142503")]
    pub fn push_trailing_sep(&mut self) {
        if !self.has_trailing_sep() {
            self.push("");
        }
    }

    /// 从路径中移除尾部[分隔符](MAIN_SEPARATOR)（如果可能的话）。
    ///
    /// 它的作用类似于 [`Path::trim_trailing_sep`]，但会就地修改底层的 `PathBuf`。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(path_trailing_sep)]
    /// use std::ffi::OsStr;
    /// use std::path::PathBuf;
    ///
    /// let mut p = PathBuf::from("dir//");
    ///
    /// assert!(p.has_trailing_sep());
    /// assert_eq!(p.as_os_str(), OsStr::new("dir//"));
    /// p.pop_trailing_sep();
    /// assert!(!p.has_trailing_sep());
    /// assert_eq!(p.as_os_str(), OsStr::new("dir"));
    /// p.pop_trailing_sep();
    /// assert!(!p.has_trailing_sep());
    /// assert_eq!(p.as_os_str(), OsStr::new("dir"));
    ///
    /// p = PathBuf::from("/");
    /// assert!(p.has_trailing_sep());
    /// p.pop_trailing_sep();
    /// assert!(p.has_trailing_sep());
    /// ```
    #[unstable(feature = "path_trailing_sep", issue = "142503")]
    pub fn pop_trailing_sep(&mut self) {
        self.inner.truncate(self.trim_trailing_sep().as_os_str().len());
    }

    /// 将 [`self.file_name`] 更新为 `file_name`。
    ///
    /// 如果 [`self.file_name`] 原本为 [`None`]，这等价于 push `file_name`。
    ///
    /// 否则它等价于调用 [`pop`]、再 push `file_name`。新路径将是原路径的兄弟节点
    ///（也就是说，它们有相同的父路径）。
    ///
    /// 参数不会被净化（sanitize），因此可以包含分隔符。该行为将来可能改为 panic。
    ///
    /// [`self.file_name`]: Path::file_name
    /// [`pop`]: PathBuf::pop
    ///
    /// # 示例
    ///
    /// ```
    /// use std::path::PathBuf;
    ///
    /// let mut buf = PathBuf::from("/");
    /// assert!(buf.file_name() == None);
    ///
    /// buf.set_file_name("foo.txt");
    /// assert!(buf == PathBuf::from("/foo.txt"));
    /// assert!(buf.file_name().is_some());
    ///
    /// buf.set_file_name("bar.txt");
    /// assert!(buf == PathBuf::from("/bar.txt"));
    ///
    /// buf.set_file_name("baz");
    /// assert!(buf == PathBuf::from("/baz"));
    ///
    /// buf.set_file_name("../b/c.txt");
    /// assert!(buf == PathBuf::from("/../b/c.txt"));
    ///
    /// buf.set_file_name("baz");
    /// assert!(buf == PathBuf::from("/../b/baz"));
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn set_file_name<S: AsRef<OsStr>>(&mut self, file_name: S) {
        self._set_file_name(file_name.as_ref())
    }

    fn _set_file_name(&mut self, file_name: &OsStr) {
        if self.file_name().is_some() {
            let popped = self.pop();
            debug_assert!(popped);
        }
        self.push(file_name);
    }

    /// 将 [`self.extension`] 更新为 `Some(extension)`；若 `extension` 为空，则更新为 [`None`]。
    ///
    /// 若 [`self.file_name`] 为 [`None`]，则返回 `false` 且什么都不做；
    /// 否则返回 `true` 并更新扩展名。
    ///
    /// 若 [`self.extension`] 为 [`None`]，则添加扩展名；否则替换它。
    ///
    /// 若 `extension` 为空字符串，操作之后 [`self.extension`] 将为 [`None`]，
    /// 而非 `Some("")`。
    ///
    /// # Panics
    ///
    /// 如果传入的扩展名包含路径分隔符（参见 [`is_separator`]），则 panic。
    ///
    /// # Caveats
    ///
    /// 新的 `extension` 可以包含点号，并将被整体使用，但只有最后一个点号之后的部分
    /// 才会反映在 [`self.extension`] 中。
    ///
    /// 如果文件主干（stem）含有内部点号且 `extension` 为空，旧文件主干的一部分
    /// 将被视为新的 [`self.extension`]。
    ///
    /// 参见下面的示例。
    ///
    /// [`self.file_name`]: Path::file_name
    /// [`self.extension`]: Path::extension
    ///
    /// # 示例
    ///
    /// ```
    /// use std::path::{Path, PathBuf};
    ///
    /// let mut p = PathBuf::from("/feel/the");
    ///
    /// p.set_extension("force");
    /// assert_eq!(Path::new("/feel/the.force"), p.as_path());
    ///
    /// p.set_extension("dark.side");
    /// assert_eq!(Path::new("/feel/the.dark.side"), p.as_path());
    ///
    /// p.set_extension("cookie");
    /// assert_eq!(Path::new("/feel/the.dark.cookie"), p.as_path());
    ///
    /// p.set_extension("");
    /// assert_eq!(Path::new("/feel/the.dark"), p.as_path());
    ///
    /// p.set_extension("");
    /// assert_eq!(Path::new("/feel/the"), p.as_path());
    ///
    /// p.set_extension("");
    /// assert_eq!(Path::new("/feel/the"), p.as_path());
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn set_extension<S: AsRef<OsStr>>(&mut self, extension: S) -> bool {
        self._set_extension(extension.as_ref())
    }

    fn _set_extension(&mut self, extension: &OsStr) -> bool {
        validate_extension(extension);

        let file_stem = match self.file_stem() {
            None => return false,
            Some(f) => f.as_encoded_bytes(),
        };

        // 截断到文件主干（stem）结束的位置之后
        let end_file_stem = file_stem[file_stem.len()..].as_ptr().addr();
        let start = self.inner.as_encoded_bytes().as_ptr().addr();
        self.inner.truncate(end_file_stem.wrapping_sub(start));

        // 添加新扩展名（如果有的话）
        let new = extension.as_encoded_bytes();
        if !new.is_empty() {
            self.inner.reserve_exact(new.len() + 1);
            self.inner.push(".");
            // SAFETY: 由于刚刚 push 了一个 UTF-8 字符串，缓冲区不可能以一个代理项的一半结尾。
            unsafe { self.inner.extend_from_slice_unchecked(new) };
        }

        true
    }

    /// 在 [`self.extension`] 之后追加 `extension`。
    ///
    /// 若 [`self.file_name`] 为 [`None`]，则返回 `false` 且什么都不做；
    /// 否则返回 `true` 并更新扩展名。
    ///
    /// # Panics
    ///
    /// 如果传入的扩展名包含路径分隔符（参见 [`is_separator`]），则 panic。
    ///
    /// # Caveats
    ///
    /// 被追加的 `extension` 可以包含点号，并将被整体使用，但只有最后一个点号之后的部分
    /// 才会反映在 [`self.extension`] 中。
    ///
    /// 参见下面的示例。
    ///
    /// [`self.file_name`]: Path::file_name
    /// [`self.extension`]: Path::extension
    ///
    /// # 示例
    ///
    /// ```
    /// use std::path::{Path, PathBuf};
    ///
    /// let mut p = PathBuf::from("/feel/the");
    ///
    /// p.add_extension("formatted");
    /// assert_eq!(Path::new("/feel/the.formatted"), p.as_path());
    ///
    /// p.add_extension("dark.side");
    /// assert_eq!(Path::new("/feel/the.formatted.dark.side"), p.as_path());
    ///
    /// p.set_extension("cookie");
    /// assert_eq!(Path::new("/feel/the.formatted.dark.cookie"), p.as_path());
    ///
    /// p.set_extension("");
    /// assert_eq!(Path::new("/feel/the.formatted.dark"), p.as_path());
    ///
    /// p.add_extension("");
    /// assert_eq!(Path::new("/feel/the.formatted.dark"), p.as_path());
    /// ```
    #[stable(feature = "path_add_extension", since = "1.91.0")]
    pub fn add_extension<S: AsRef<OsStr>>(&mut self, extension: S) -> bool {
        self._add_extension(extension.as_ref())
    }

    fn _add_extension(&mut self, extension: &OsStr) -> bool {
        validate_extension(extension);

        let file_name = match self.file_name() {
            None => return false,
            Some(f) => f.as_encoded_bytes(),
        };

        let new = extension.as_encoded_bytes();
        if !new.is_empty() {
            // 截断到文件名结束的位置之后
            // 这是为了修剪掉尾部分隔符所必需的
            let end_file_name = file_name[file_name.len()..].as_ptr().addr();
            let start = self.inner.as_encoded_bytes().as_ptr().addr();
            self.inner.truncate(end_file_name.wrapping_sub(start));

            // 追加新扩展名
            self.inner.reserve_exact(new.len() + 1);
            self.inner.push(".");
            // SAFETY: 由于刚刚 push 了一个 UTF-8 字符串，缓冲区不可能以一个代理项的一半结尾。
            unsafe { self.inner.extend_from_slice_unchecked(new) };
        }

        true
    }

    /// 产出一个对底层 [`OsString`] 实例的可变引用。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::path::{Path, PathBuf};
    ///
    /// let mut path = PathBuf::from("/foo");
    ///
    /// path.push("bar");
    /// assert_eq!(path, Path::new("/foo/bar"));
    ///
    /// // OsString 的 `push` 不会添加分隔符。
    /// path.as_mut_os_string().push("baz");
    /// assert_eq!(path, Path::new("/foo/barbaz"));
    /// ```
    #[stable(feature = "path_as_mut_os_str", since = "1.70.0")]
    #[must_use]
    #[inline]
    pub fn as_mut_os_string(&mut self) -> &mut OsString {
        &mut self.inner
    }

    /// 消耗 `PathBuf`，产出其内部的 [`OsString`] 存储。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::path::PathBuf;
    ///
    /// let p = PathBuf::from("/the/head");
    /// let os_str = p.into_os_string();
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[must_use = "`self` will be dropped if the result is not used"]
    #[inline]
    pub fn into_os_string(self) -> OsString {
        self.inner
    }

    /// 将这个 `PathBuf` 转换为一个 [boxed](Box) 的 [`Path`]。
    #[stable(feature = "into_boxed_path", since = "1.20.0")]
    #[must_use = "`self` will be dropped if the result is not used"]
    #[inline]
    pub fn into_boxed_path(self) -> Box<Path> {
        let rw = Box::into_raw(self.inner.into_boxed_os_str()) as *mut Path;
        unsafe { Box::from_raw(rw) }
    }

    /// 在底层的 [`OsString`] 实例上调用 [`capacity`]。
    ///
    /// [`capacity`]: OsString::capacity
    #[stable(feature = "path_buf_capacity", since = "1.44.0")]
    #[must_use]
    #[inline]
    pub fn capacity(&self) -> usize {
        self.inner.capacity()
    }

    /// 在底层的 [`OsString`] 实例上调用 [`clear`]。
    ///
    /// [`clear`]: OsString::clear
    #[stable(feature = "path_buf_capacity", since = "1.44.0")]
    #[inline]
    pub fn clear(&mut self) {
        self.inner.clear()
    }

    /// 在底层的 [`OsString`] 实例上调用 [`reserve`]。
    ///
    /// [`reserve`]: OsString::reserve
    #[stable(feature = "path_buf_capacity", since = "1.44.0")]
    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        self.inner.reserve(additional)
    }

    /// 在底层的 [`OsString`] 实例上调用 [`try_reserve`]。
    ///
    /// [`try_reserve`]: OsString::try_reserve
    #[stable(feature = "try_reserve_2", since = "1.63.0")]
    #[inline]
    pub fn try_reserve(&mut self, additional: usize) -> Result<(), TryReserveError> {
        self.inner.try_reserve(additional)
    }

    /// 在底层的 [`OsString`] 实例上调用 [`reserve_exact`]。
    ///
    /// [`reserve_exact`]: OsString::reserve_exact
    #[stable(feature = "path_buf_capacity", since = "1.44.0")]
    #[inline]
    pub fn reserve_exact(&mut self, additional: usize) {
        self.inner.reserve_exact(additional)
    }

    /// 在底层的 [`OsString`] 实例上调用 [`try_reserve_exact`]。
    ///
    /// [`try_reserve_exact`]: OsString::try_reserve_exact
    #[stable(feature = "try_reserve_2", since = "1.63.0")]
    #[inline]
    pub fn try_reserve_exact(&mut self, additional: usize) -> Result<(), TryReserveError> {
        self.inner.try_reserve_exact(additional)
    }

    /// 在底层的 [`OsString`] 实例上调用 [`shrink_to_fit`]。
    ///
    /// [`shrink_to_fit`]: OsString::shrink_to_fit
    #[stable(feature = "path_buf_capacity", since = "1.44.0")]
    #[inline]
    pub fn shrink_to_fit(&mut self) {
        self.inner.shrink_to_fit()
    }

    /// 在底层的 [`OsString`] 实例上调用 [`shrink_to`]。
    ///
    /// [`shrink_to`]: OsString::shrink_to
    #[stable(feature = "shrink_to", since = "1.56.0")]
    #[inline]
    pub fn shrink_to(&mut self, min_capacity: usize) {
        self.inner.shrink_to(min_capacity)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl Clone for PathBuf {
    #[inline]
    fn clone(&self) -> Self {
        PathBuf { inner: self.inner.clone() }
    }

    /// 将 `source` 的内容克隆到 `self` 中。
    ///
    /// 这个方法比简单地把 `source.clone()` 赋给 `self` 更可取，因为它在可能时避免重新分配。
    #[inline]
    fn clone_from(&mut self, source: &Self) {
        self.inner.clone_from(&source.inner)
    }
}

#[stable(feature = "box_from_path", since = "1.17.0")]
impl From<&Path> for Box<Path> {
    /// 从一个引用创建一个 boxed 的 [`Path`]。
    ///
    /// 这会分配内存，并把 `path` 克隆进去。
    fn from(path: &Path) -> Box<Path> {
        Box::clone_from_ref(path)
    }
}

#[stable(feature = "box_from_mut_slice", since = "1.84.0")]
impl From<&mut Path> for Box<Path> {
    /// 从一个引用创建一个 boxed 的 [`Path`]。
    ///
    /// 这会分配内存，并把 `path` 克隆进去。
    fn from(path: &mut Path) -> Box<Path> {
        Self::from(&*path)
    }
}

#[stable(feature = "box_from_cow", since = "1.45.0")]
impl From<Cow<'_, Path>> for Box<Path> {
    /// 从一个写时克隆（clone-on-write）指针创建一个 boxed 的 [`Path`]。
    ///
    /// 从 `Cow::Owned` 转换不会克隆或分配。
    #[inline]
    fn from(cow: Cow<'_, Path>) -> Box<Path> {
        match cow {
            Cow::Borrowed(path) => Box::from(path),
            Cow::Owned(path) => Box::from(path),
        }
    }
}

#[stable(feature = "path_buf_from_box", since = "1.18.0")]
impl From<Box<Path>> for PathBuf {
    /// 将一个 <code>[Box]&lt;[Path]&gt;</code> 转换为 [`PathBuf`]。
    ///
    /// 这个转换不会分配或拷贝内存。
    #[inline]
    fn from(boxed: Box<Path>) -> PathBuf {
        boxed.into_path_buf()
    }
}

#[stable(feature = "box_from_path_buf", since = "1.20.0")]
impl From<PathBuf> for Box<Path> {
    /// 将一个 [`PathBuf`] 转换为 <code>[Box]&lt;[Path]&gt;</code>。
    ///
    /// 这个转换目前不应分配内存，但该行为在所有平台或未来所有版本上并不被保证。
    #[inline]
    fn from(p: PathBuf) -> Box<Path> {
        p.into_boxed_path()
    }
}

#[stable(feature = "more_box_slice_clone", since = "1.29.0")]
impl Clone for Box<Path> {
    #[inline]
    fn clone(&self) -> Self {
        self.to_path_buf().into_boxed_path()
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<T: ?Sized + AsRef<OsStr>> From<&T> for PathBuf {
    /// 将一个借用的 [`OsStr`] 转换为 [`PathBuf`]。
    ///
    /// 分配一个 [`PathBuf`] 并把数据拷贝进去。
    #[inline]
    fn from(s: &T) -> PathBuf {
        PathBuf::from(s.as_ref().to_os_string())
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl From<OsString> for PathBuf {
    /// 将一个 [`OsString`] 转换为 [`PathBuf`]。
    ///
    /// 这个转换不会分配或拷贝内存。
    #[inline]
    fn from(s: OsString) -> PathBuf {
        PathBuf { inner: s }
    }
}

#[stable(feature = "from_path_buf_for_os_string", since = "1.14.0")]
impl From<PathBuf> for OsString {
    /// 将一个 [`PathBuf`] 转换为 [`OsString`]
    ///
    /// 这个转换不会分配或拷贝内存。
    #[inline]
    fn from(path_buf: PathBuf) -> OsString {
        path_buf.inner
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl From<String> for PathBuf {
    /// 将一个 [`String`] 转换为 [`PathBuf`]
    ///
    /// 这个转换不会分配或拷贝内存。
    #[inline]
    fn from(s: String) -> PathBuf {
        PathBuf::from(OsString::from(s))
    }
}

#[stable(feature = "path_from_str", since = "1.32.0")]
impl FromStr for PathBuf {
    type Err = core::convert::Infallible;

    #[inline]
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(PathBuf::from(s))
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<P: AsRef<Path>> FromIterator<P> for PathBuf {
    /// 从一个迭代器的各 [`Path`] 元素创建一个新的 `PathBuf`。
    ///
    /// 这会用 [`push`](Self::push) 添加每个元素，因此可用于拼接多个路径
    /// [组件](Components)。
    ///
    /// # 示例
    /// ```
    /// # use std::path::PathBuf;
    /// let path = PathBuf::from_iter(["/tmp", "foo", "bar"]);
    /// assert_eq!(path, PathBuf::from("/tmp/foo/bar"));
    /// ```
    ///
    /// 关于路径如何被构建出来的更多细节，参见 [`push`](Self::push) 的文档。
    fn from_iter<I: IntoIterator<Item = P>>(iter: I) -> PathBuf {
        let mut buf = PathBuf::new();
        buf.extend(iter);
        buf
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl<P: AsRef<Path>> Extend<P> for PathBuf {
    /// 用来自 `iter` 的各 [`Path`] 元素扩展 `self`。
    ///
    /// 这会用 [`push`](Self::push) 添加每个元素，因此可用于拼接多个路径
    /// [组件](Components)。
    ///
    /// # 示例
    /// ```
    /// # use std::path::PathBuf;
    /// let mut path = PathBuf::from("/tmp");
    /// path.extend(["foo", "bar", "file.txt"]);
    /// assert_eq!(path, PathBuf::from("/tmp/foo/bar/file.txt"));
    /// ```
    ///
    /// 关于路径如何被构建出来的更多细节，参见 [`push`](Self::push) 的文档。
    fn extend<I: IntoIterator<Item = P>>(&mut self, iter: I) {
        iter.into_iter().for_each(move |p| self.push(p.as_ref()));
    }

    #[inline]
    fn extend_one(&mut self, p: P) {
        self.push(p.as_ref());
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl fmt::Debug for PathBuf {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, formatter)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl ops::Deref for PathBuf {
    type Target = Path;
    #[inline]
    fn deref(&self) -> &Path {
        Path::new(&self.inner)
    }
}

#[stable(feature = "path_buf_deref_mut", since = "1.68.0")]
impl ops::DerefMut for PathBuf {
    #[inline]
    fn deref_mut(&mut self) -> &mut Path {
        Path::from_inner_mut(&mut self.inner)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl Borrow<Path> for PathBuf {
    #[inline]
    fn borrow(&self) -> &Path {
        self.deref()
    }
}

#[stable(feature = "default_for_pathbuf", since = "1.17.0")]
impl Default for PathBuf {
    #[inline]
    fn default() -> Self {
        PathBuf::new()
    }
}

#[stable(feature = "cow_from_path", since = "1.6.0")]
impl<'a> From<&'a Path> for Cow<'a, Path> {
    /// 从一个对 [`Path`] 的引用创建一个写时克隆（clone-on-write）指针。
    ///
    /// 这个转换不会克隆或分配。
    #[inline]
    fn from(s: &'a Path) -> Cow<'a, Path> {
        Cow::Borrowed(s)
    }
}

#[stable(feature = "cow_from_path", since = "1.6.0")]
impl<'a> From<PathBuf> for Cow<'a, Path> {
    /// 从一个持有所有权的 [`PathBuf`] 实例创建一个写时克隆（clone-on-write）指针。
    ///
    /// 这个转换不会克隆或分配。
    #[inline]
    fn from(s: PathBuf) -> Cow<'a, Path> {
        Cow::Owned(s)
    }
}

#[stable(feature = "cow_from_pathbuf_ref", since = "1.28.0")]
impl<'a> From<&'a PathBuf> for Cow<'a, Path> {
    /// 从一个对 [`PathBuf`] 的引用创建一个写时克隆（clone-on-write）指针。
    ///
    /// 这个转换不会克隆或分配。
    #[inline]
    fn from(p: &'a PathBuf) -> Cow<'a, Path> {
        Cow::Borrowed(p.as_path())
    }
}

#[stable(feature = "pathbuf_from_cow_path", since = "1.28.0")]
impl<'a> From<Cow<'a, Path>> for PathBuf {
    /// 将一个写时克隆（clone-on-write）指针转换为一个持有所有权的路径。
    ///
    /// 从 `Cow::Owned` 转换不会克隆或分配。
    #[inline]
    fn from(p: Cow<'a, Path>) -> Self {
        p.into_owned()
    }
}

#[stable(feature = "shared_from_slice2", since = "1.24.0")]
impl From<PathBuf> for Arc<Path> {
    /// 通过把 [`PathBuf`] 的数据移动进一个新的 [`Arc`] 缓冲区，将其转换为 <code>[Arc]<[Path]></code>。
    #[inline]
    fn from(s: PathBuf) -> Arc<Path> {
        let arc: Arc<OsStr> = Arc::from(s.into_os_string());
        unsafe { Arc::from_raw(Arc::into_raw(arc) as *const Path) }
    }
}

#[stable(feature = "shared_from_slice2", since = "1.24.0")]
impl From<&Path> for Arc<Path> {
    /// 通过把 [`Path`] 的数据拷贝进一个新的 [`Arc`] 缓冲区，将其转换为 [`Arc`]。
    #[inline]
    fn from(s: &Path) -> Arc<Path> {
        let arc: Arc<OsStr> = Arc::from(s.as_os_str());
        unsafe { Arc::from_raw(Arc::into_raw(arc) as *const Path) }
    }
}

#[stable(feature = "shared_from_mut_slice", since = "1.84.0")]
impl From<&mut Path> for Arc<Path> {
    /// 通过把 [`Path`] 的数据拷贝进一个新的 [`Arc`] 缓冲区，将其转换为 [`Arc`]。
    #[inline]
    fn from(s: &mut Path) -> Arc<Path> {
        Arc::from(&*s)
    }
}

#[stable(feature = "shared_from_slice2", since = "1.24.0")]
impl From<PathBuf> for Rc<Path> {
    /// 通过把 [`PathBuf`] 的数据移动进一个新的 [`Rc`] 缓冲区，将其转换为 <code>[Rc]<[Path]></code>。
    #[inline]
    fn from(s: PathBuf) -> Rc<Path> {
        let rc: Rc<OsStr> = Rc::from(s.into_os_string());
        unsafe { Rc::from_raw(Rc::into_raw(rc) as *const Path) }
    }
}

#[stable(feature = "shared_from_slice2", since = "1.24.0")]
impl From<&Path> for Rc<Path> {
    /// 通过把 [`Path`] 的数据拷贝进一个新的 [`Rc`] 缓冲区，将其转换为 [`Rc`]。
    #[inline]
    fn from(s: &Path) -> Rc<Path> {
        let rc: Rc<OsStr> = Rc::from(s.as_os_str());
        unsafe { Rc::from_raw(Rc::into_raw(rc) as *const Path) }
    }
}

#[stable(feature = "shared_from_mut_slice", since = "1.84.0")]
impl From<&mut Path> for Rc<Path> {
    /// 通过把 [`Path`] 的数据拷贝进一个新的 [`Rc`] 缓冲区，将其转换为 [`Rc`]。
    #[inline]
    fn from(s: &mut Path) -> Rc<Path> {
        Rc::from(&*s)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl ToOwned for Path {
    type Owned = PathBuf;
    #[inline]
    fn to_owned(&self) -> PathBuf {
        self.to_path_buf()
    }
    #[inline]
    fn clone_into(&self, target: &mut PathBuf) {
        self.inner.clone_into(&mut target.inner);
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl PartialEq for PathBuf {
    #[inline]
    fn eq(&self, other: &PathBuf) -> bool {
        self.components() == other.components()
    }
}

#[stable(feature = "eq_str_for_path", since = "1.91.0")]
impl cmp::PartialEq<str> for PathBuf {
    #[inline]
    fn eq(&self, other: &str) -> bool {
        self.as_path() == other
    }
}

#[stable(feature = "eq_str_for_path", since = "1.91.0")]
impl cmp::PartialEq<PathBuf> for str {
    #[inline]
    fn eq(&self, other: &PathBuf) -> bool {
        self == other.as_path()
    }
}

#[stable(feature = "eq_str_for_path", since = "1.91.0")]
impl cmp::PartialEq<String> for PathBuf {
    #[inline]
    fn eq(&self, other: &String) -> bool {
        self.as_path() == other.as_str()
    }
}

#[stable(feature = "eq_str_for_path", since = "1.91.0")]
impl cmp::PartialEq<PathBuf> for String {
    #[inline]
    fn eq(&self, other: &PathBuf) -> bool {
        self.as_str() == other.as_path()
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl Hash for PathBuf {
    fn hash<H: Hasher>(&self, h: &mut H) {
        self.as_path().hash(h)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl Eq for PathBuf {}

#[stable(feature = "rust1", since = "1.0.0")]
impl PartialOrd for PathBuf {
    #[inline]
    fn partial_cmp(&self, other: &PathBuf) -> Option<cmp::Ordering> {
        Some(compare_components(self.components(), other.components()))
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl Ord for PathBuf {
    #[inline]
    fn cmp(&self, other: &PathBuf) -> cmp::Ordering {
        compare_components(self.components(), other.components())
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl AsRef<OsStr> for PathBuf {
    #[inline]
    fn as_ref(&self) -> &OsStr {
        &self.inner[..]
    }
}

/// 路径的切片（类比 [`str`]）。
///
/// 该类型支持若干用于审视路径的操作，包括将路径拆解为它的各组件（Unix 上以 `/` 分隔，
/// Windows 上以 `/` 或 `\` 分隔）、提取文件名、判断路径是否为绝对路径等等。
///
/// 这是一个*非定长（unsized）*类型，意味着它必须始终经由指针（例如 `&` 或 [`Box`]）使用。
/// 想要本类型的持有所有权版本，参见 [`PathBuf`]。
///
/// 关于整体设计思路的更多细节，参见[模块级文档](self)。
///
/// # 示例
///
/// ```
/// use std::path::Path;
/// use std::ffi::OsStr;
///
/// // 注意：这个示例在 Windows 上同样有效
/// let path = Path::new("./foo/bar.txt");
///
/// let parent = path.parent();
/// assert_eq!(parent, Some(Path::new("./foo")));
///
/// let file_stem = path.file_stem();
/// assert_eq!(file_stem, Some(OsStr::new("bar")));
///
/// let extension = path.extension();
/// assert_eq!(extension, Some(OsStr::new("txt")));
/// ```
#[cfg_attr(not(test), rustc_diagnostic_item = "Path")]
#[stable(feature = "rust1", since = "1.0.0")]
// `Path::new` 以及 `impl CloneToUninit for Path` 目前的实现依赖于
// `Path` 与 `OsStr` 在内存布局上兼容。
// 然而，`Path` 的布局被视为实现细节，绝不可被依赖。
#[repr(transparent)]
pub struct Path {
    inner: OsStr,
}

/// 当未找到前缀时，由 [`Path::strip_prefix`] 返回的错误。
///
/// 该 `struct` 由 [`Path`] 上的 [`strip_prefix`] 方法创建。
/// 更多内容参见其文档。
///
/// [`strip_prefix`]: Path::strip_prefix
#[derive(Debug, Clone, PartialEq, Eq)]
#[stable(since = "1.7.0", feature = "strip_prefix")]
pub struct StripPrefixError(());

/// 当 `..` 父引用会逃逸出该路径时，由 [`Path::normalize_lexically`] 返回的错误。
#[unstable(feature = "normalize_lexically", issue = "134694")]
#[derive(Debug, PartialEq)]
#[non_exhaustive]
pub struct NormalizeError;

impl Path {
    // 下面这个（私有！）函数允许从一个 u8 切片构造出一条路径，
    // 这只有在已知该切片遵循 OsStr 编码时才是安全的。
    unsafe fn from_u8_slice(s: &[u8]) -> &Path {
        unsafe { Path::new(OsStr::from_encoded_bytes_unchecked(s)) }
    }
    // 下面这个（私有！）函数揭示了 OsStr 所使用的字节编码。
    pub(crate) fn as_u8_slice(&self) -> &[u8] {
        self.inner.as_encoded_bytes()
    }

    /// 直接把一个字符串切片包装为 `Path` 切片。
    ///
    /// 这是一个零开销的转换。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::path::Path;
    ///
    /// Path::new("foo.txt");
    /// ```
    ///
    /// 你可以从 `String`、甚至其他 `Path` 创建 `Path`：
    ///
    /// ```
    /// use std::path::Path;
    ///
    /// let string = String::from("foo.txt");
    /// let from_string = Path::new(&string);
    /// let from_path = Path::new(&from_string);
    /// assert_eq!(from_string, from_path);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[rustc_const_unstable(feature = "const_convert", issue = "143773")]
    pub const fn new<S: [const] AsRef<OsStr> + ?Sized>(s: &S) -> &Path {
        unsafe { &*(s.as_ref() as *const OsStr as *const Path) }
    }

    #[rustc_const_unstable(feature = "const_convert", issue = "143773")]
    const fn from_inner_mut(inner: &mut OsStr) -> &mut Path {
        // SAFETY: Path 只是对 OsStr 的一层包装，
        // 因此把 &mut OsStr 转换为 &mut Path 是安全的。
        unsafe { &mut *(inner as *mut OsStr as *mut Path) }
    }

    /// 产出底层的 [`OsStr`] 切片。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::path::Path;
    ///
    /// let os_str = Path::new("foo.txt").as_os_str();
    /// assert_eq!(os_str, std::ffi::OsStr::new("foo.txt"));
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[must_use]
    #[inline]
    pub fn as_os_str(&self) -> &OsStr {
        &self.inner
    }

    /// 产出一个对底层 [`OsStr`] 切片的可变引用。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::path::{Path, PathBuf};
    ///
    /// let mut path = PathBuf::from("Foo.TXT");
    ///
    /// assert_ne!(path, Path::new("foo.txt"));
    ///
    /// path.as_mut_os_str().make_ascii_lowercase();
    /// assert_eq!(path, Path::new("foo.txt"));
    /// ```
    #[stable(feature = "path_as_mut_os_str", since = "1.70.0")]
    #[must_use]
    #[inline]
    pub fn as_mut_os_str(&mut self) -> &mut OsStr {
        &mut self.inner
    }

    /// 若 `Path` 是合法的 unicode，则产出一个 [`&str`] 切片。
    ///
    /// 这个转换可能需要做一次 UTF-8 合法性检查。注意之所以要做校验，是因为非 UTF-8
    /// 字符串对某些操作系统是完全合法的。
    ///
    /// [`&str`]: str
    ///
    /// # 示例
    ///
    /// ```
    /// use std::path::Path;
    ///
    /// let path = Path::new("foo.txt");
    /// assert_eq!(path.to_str(), Some("foo.txt"));
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[must_use = "this returns the result of the operation, \
                  without modifying the original"]
    #[inline]
    pub fn to_str(&self) -> Option<&str> {
        self.inner.to_str()
    }

    /// 将一个 `Path` 转换为 [`Cow<str>`]。
    ///
    /// 任何非 UTF-8 序列都会被替换为
    /// [`U+FFFD REPLACEMENT CHARACTER`][U+FFFD]（替换字符）。
    ///
    /// [U+FFFD]: super::char::REPLACEMENT_CHARACTER
    ///
    /// # 示例
    ///
    /// 对一个含合法 unicode 的 `Path` 调用 `to_string_lossy`：
    ///
    /// ```
    /// use std::path::Path;
    ///
    /// let path = Path::new("foo.txt");
    /// assert_eq!(path.to_string_lossy(), "foo.txt");
    /// ```
    ///
    /// 假如 `path` 含有非法 unicode，则 `to_string_lossy` 调用可能会返回
    /// `"fo�.txt"`。
    #[stable(feature = "rust1", since = "1.0.0")]
    #[must_use = "this returns the result of the operation, \
                  without modifying the original"]
    #[inline]
    pub fn to_string_lossy(&self) -> Cow<'_, str> {
        self.inner.to_string_lossy()
    }

    /// 将一个 `Path` 转换为一个持有所有权的 [`PathBuf`]。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::path::{Path, PathBuf};
    ///
    /// let path_buf = Path::new("foo.txt").to_path_buf();
    /// assert_eq!(path_buf, PathBuf::from("foo.txt"));
    /// ```
    #[rustc_conversion_suggestion]
    #[must_use = "this returns the result of the operation, \
                  without modifying the original"]
    #[stable(feature = "rust1", since = "1.0.0")]
    #[cfg_attr(not(test), rustc_diagnostic_item = "path_to_pathbuf")]
    pub fn to_path_buf(&self) -> PathBuf {
        PathBuf::from(self.inner.to_os_string())
    }

    /// 若 `Path` 是绝对路径（即它与当前目录无关），则返回 `true`。
    ///
    /// * 在 Unix 上，路径以根开头即为绝对路径，因此 `is_absolute` 与 [`has_root`]
    /// 是等价的。
    ///
    /// * 在 Windows 上，路径既带有前缀又以根开头时才是绝对路径：`c:\windows`
    /// 是绝对路径，而 `c:temp` 和 `\temp` 不是。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::path::Path;
    ///
    /// assert!(!Path::new("foo.txt").is_absolute());
    /// ```
    ///
    /// [`has_root`]: Path::has_root
    #[stable(feature = "rust1", since = "1.0.0")]
    #[must_use]
    #[allow(deprecated)]
    pub fn is_absolute(&self) -> bool {
        sys::path::is_absolute(self)
    }

    /// 若 `Path` 是相对路径（即不是绝对路径），则返回 `true`。
    ///
    /// 更多细节参见 [`is_absolute`] 的文档。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::path::Path;
    ///
    /// assert!(Path::new("foo.txt").is_relative());
    /// ```
    ///
    /// [`is_absolute`]: Path::is_absolute
    #[stable(feature = "rust1", since = "1.0.0")]
    #[must_use]
    #[inline]
    pub fn is_relative(&self) -> bool {
        !self.is_absolute()
    }

    pub(crate) fn prefix(&self) -> Option<Prefix<'_>> {
        self.components().prefix
    }

    /// 若 `Path` 带有根，则返回 `true`。
    ///
    /// * 在 Unix 上，路径以 `/` 开头即带有根。
    ///
    /// * 在 Windows 上，路径在以下情况下带有根：
    ///     * 没有前缀且以分隔符开头，例如 `\windows`
    ///     * 有前缀且其后紧跟一个分隔符，例如 `c:\windows`，但 `c:windows` 不算
    ///     * 带有任何非磁盘前缀，例如 `\\server\share`
    ///
    /// # 示例
    ///
    /// ```
    /// use std::path::Path;
    ///
    /// assert!(Path::new("/etc/passwd").has_root());
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[must_use]
    #[inline]
    pub fn has_root(&self) -> bool {
        self.components().has_root()
    }

    /// 返回去掉最后一个组件后的 `Path`（如果存在的话）。
    ///
    /// 这意味着对于只有一个组件的相对路径，它返回 `Some("")`。
    ///
    /// 若路径以根或前缀结尾、或它是空字符串，则返回 [`None`]。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::path::Path;
    ///
    /// let path = Path::new("/foo/bar");
    /// let parent = path.parent().unwrap();
    /// assert_eq!(parent, Path::new("/foo"));
    ///
    /// let grand_parent = parent.parent().unwrap();
    /// assert_eq!(grand_parent, Path::new("/"));
    /// assert_eq!(grand_parent.parent(), None);
    ///
    /// let relative_path = Path::new("foo/bar");
    /// let parent = relative_path.parent();
    /// assert_eq!(parent, Some(Path::new("foo")));
    /// let grand_parent = parent.and_then(Path::parent);
    /// assert_eq!(grand_parent, Some(Path::new("")));
    /// let great_grand_parent = grand_parent.and_then(Path::parent);
    /// assert_eq!(great_grand_parent, None);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[doc(alias = "dirname")]
    #[must_use]
    pub fn parent(&self) -> Option<&Path> {
        let mut comps = self.components();
        let comp = comps.next_back();
        comp.and_then(|p| match p {
            Component::Normal(_) | Component::CurDir | Component::ParentDir => {
                Some(comps.as_path())
            }
            _ => None,
        })
    }

    /// 产出一个遍历 `Path` 及其各级祖先的迭代器。
    ///
    /// 该迭代器将依次产出对 [`parent`] 方法使用零次或多次后所得到的 `Path`。若 [`parent`]
    /// 方法返回 [`None`]，迭代器也将随之停止。迭代器总是至少产出一个值，即 `Some(&self)`。
    /// 接下来它会依次产出 `&self.parent()`、`&self.parent().and_then(Path::parent)`，以此类推。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::path::Path;
    ///
    /// let mut ancestors = Path::new("/foo/bar").ancestors();
    /// assert_eq!(ancestors.next(), Some(Path::new("/foo/bar")));
    /// assert_eq!(ancestors.next(), Some(Path::new("/foo")));
    /// assert_eq!(ancestors.next(), Some(Path::new("/")));
    /// assert_eq!(ancestors.next(), None);
    ///
    /// let mut ancestors = Path::new("../foo/bar").ancestors();
    /// assert_eq!(ancestors.next(), Some(Path::new("../foo/bar")));
    /// assert_eq!(ancestors.next(), Some(Path::new("../foo")));
    /// assert_eq!(ancestors.next(), Some(Path::new("..")));
    /// assert_eq!(ancestors.next(), Some(Path::new("")));
    /// assert_eq!(ancestors.next(), None);
    /// ```
    ///
    /// [`parent`]: Path::parent
    #[stable(feature = "path_ancestors", since = "1.28.0")]
    #[inline]
    pub fn ancestors(&self) -> Ancestors<'_> {
        Ancestors { next: Some(&self) }
    }

    /// 返回 `Path` 的最后一个组件（如果存在的话）。
    ///
    /// 如果路径指向一个普通文件，这就是文件名；如果它是一个目录的路径，这就是目录名。
    ///
    /// 若路径以 `..` 结尾，则返回 [`None`]。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::path::Path;
    /// use std::ffi::OsStr;
    ///
    /// assert_eq!(Some(OsStr::new("bin")), Path::new("/usr/bin/").file_name());
    /// assert_eq!(Some(OsStr::new("foo.txt")), Path::new("tmp/foo.txt").file_name());
    /// assert_eq!(Some(OsStr::new("foo.txt")), Path::new("foo.txt/.").file_name());
    /// assert_eq!(Some(OsStr::new("foo.txt")), Path::new("foo.txt/.//").file_name());
    /// assert_eq!(None, Path::new("foo.txt/..").file_name());
    /// assert_eq!(None, Path::new("/").file_name());
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[doc(alias = "basename")]
    #[must_use]
    pub fn file_name(&self) -> Option<&OsStr> {
        self.components().next_back().and_then(|p| match p {
            Component::Normal(p) => Some(p),
            _ => None,
        })
    }

    /// 返回一条路径，当它被 join 到 `base` 上时会得到 `self`。
    ///
    /// # 错误(Errors）
    ///
    /// 若 `base` 不是 `self` 的前缀（即 [`starts_with`] 返回 `false`），则返回 [`Err`]。
    ///
    /// [`starts_with`]: Path::starts_with
    ///
    /// # 示例
    ///
    /// ```
    /// use std::path::{Path, PathBuf};
    ///
    /// let path = Path::new("/test/haha/foo.txt");
    ///
    /// assert_eq!(path.strip_prefix("/"), Ok(Path::new("test/haha/foo.txt")));
    /// assert_eq!(path.strip_prefix("/test"), Ok(Path::new("haha/foo.txt")));
    /// assert_eq!(path.strip_prefix("/test/"), Ok(Path::new("haha/foo.txt")));
    /// assert_eq!(path.strip_prefix("/test/haha/foo.txt"), Ok(Path::new("")));
    /// assert_eq!(path.strip_prefix("/test/haha/foo.txt/"), Ok(Path::new("")));
    ///
    /// assert!(path.strip_prefix("test").is_err());
    /// assert!(path.strip_prefix("/te").is_err());
    /// assert!(path.strip_prefix("/haha").is_err());
    ///
    /// let prefix = PathBuf::from("/test/");
    /// assert_eq!(path.strip_prefix(prefix), Ok(Path::new("haha/foo.txt")));
    /// ```
    #[stable(since = "1.7.0", feature = "path_strip_prefix")]
    pub fn strip_prefix<P>(&self, base: P) -> Result<&Path, StripPrefixError>
    where
        P: AsRef<Path>,
    {
        self._strip_prefix(base.as_ref())
    }

    fn _strip_prefix(&self, base: &Path) -> Result<&Path, StripPrefixError> {
        iter_after(self.components(), base.components())
            .map(|c| c.as_path())
            .ok_or(StripPrefixError(()))
    }

    /// 判定 `base` 是否为 `self` 的前缀。
    ///
    /// 只把完整的路径组件视为匹配。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::path::Path;
    ///
    /// let path = Path::new("/etc/passwd");
    ///
    /// assert!(path.starts_with("/etc"));
    /// assert!(path.starts_with("/etc/"));
    /// assert!(path.starts_with("/etc/passwd"));
    /// assert!(path.starts_with("/etc/passwd/")); // 多出来的斜杠没关系
    /// assert!(path.starts_with("/etc/passwd///")); // 多个多余的斜杠也没关系
    ///
    /// assert!(!path.starts_with("/e"));
    /// assert!(!path.starts_with("/etc/passwd.txt"));
    ///
    /// assert!(!Path::new("/etc/foo.rs").starts_with("/etc/foo"));
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[must_use]
    pub fn starts_with<P: AsRef<Path>>(&self, base: P) -> bool {
        self._starts_with(base.as_ref())
    }

    fn _starts_with(&self, base: &Path) -> bool {
        iter_after(self.components(), base.components()).is_some()
    }

    /// 判定 `child` 是否为 `self` 的后缀。
    ///
    /// 只把完整的路径组件视为匹配。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::path::Path;
    ///
    /// let path = Path::new("/etc/resolv.conf");
    ///
    /// assert!(path.ends_with("resolv.conf"));
    /// assert!(path.ends_with("etc/resolv.conf"));
    /// assert!(path.ends_with("/etc/resolv.conf"));
    ///
    /// assert!(!path.ends_with("/resolv.conf"));
    /// assert!(!path.ends_with("conf")); // 请改用 .extension()
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[must_use]
    pub fn ends_with<P: AsRef<Path>>(&self, child: P) -> bool {
        self._ends_with(child.as_ref())
    }

    fn _ends_with(&self, child: &Path) -> bool {
        iter_after(self.components().rev(), child.components().rev()).is_some()
    }

    /// 检查该 `Path` 是否为空。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(path_is_empty)]
    /// use std::path::Path;
    ///
    /// let path = Path::new("");
    /// assert!(path.is_empty());
    ///
    /// let path = Path::new("foo");
    /// assert!(!path.is_empty());
    ///
    /// let path = Path::new(".");
    /// assert!(!path.is_empty());
    /// ```
    #[unstable(feature = "path_is_empty", issue = "148494")]
    pub fn is_empty(&self) -> bool {
        self.as_os_str().is_empty()
    }

    /// 提取 [`self.file_name`] 中的主干（非扩展名）部分。
    ///
    /// [`self.file_name`]: Path::file_name
    ///
    /// 主干（stem）是：
    ///
    /// * [`None`]，如果没有文件名；
    /// * 整个文件名，如果其中没有内嵌的 `.`；
    /// * 整个文件名，如果文件名以 `.` 开头且其内部没有其他 `.`；
    /// * 否则，文件名中最后一个 `.` 之前的部分
    ///
    /// # 示例
    ///
    /// ```
    /// use std::path::Path;
    ///
    /// assert_eq!("foo", Path::new("foo.rs").file_stem().unwrap());
    /// assert_eq!("foo.tar", Path::new("foo.tar.gz").file_stem().unwrap());
    /// ```
    ///
    /// # See Also
    /// 本方法与 [`Path::file_prefix`] 类似，后者提取文件名中*第一个* `.` 之前的部分。
    ///
    /// [`Path::file_prefix`]: Path::file_prefix
    ///
    #[stable(feature = "rust1", since = "1.0.0")]
    #[must_use]
    pub fn file_stem(&self) -> Option<&OsStr> {
        self.file_name().map(rsplit_file_at_dot).and_then(|(before, after)| before.or(after))
    }

    /// 提取 [`self.file_name`] 的前缀（prefix）。
    ///
    /// 前缀是：
    ///
    /// * [`None`]，如果没有文件名；
    /// * 整个文件名，如果其中没有内嵌的 `.`；
    /// * 文件名中第一个非起始位置的 `.` 之前的部分；
    /// * 整个文件名，如果文件名以 `.` 开头且其内部没有其他 `.`；
    /// * 若文件名以 `.` 开头，则为第二个 `.` 之前的部分
    ///
    /// [`self.file_name`]: Path::file_name
    ///
    /// # 示例
    ///
    /// ```
    /// use std::path::Path;
    ///
    /// assert_eq!("foo", Path::new("foo.rs").file_prefix().unwrap());
    /// assert_eq!("foo", Path::new("foo.tar.gz").file_prefix().unwrap());
    /// assert_eq!(".config", Path::new(".config").file_prefix().unwrap());
    /// assert_eq!(".config", Path::new(".config.toml").file_prefix().unwrap());
    /// ```
    ///
    /// # See Also
    /// 本方法与 [`Path::file_stem`] 类似，后者提取文件名中*最后一个* `.` 之前的部分。
    ///
    /// [`Path::file_stem`]: Path::file_stem
    ///
    #[stable(feature = "path_file_prefix", since = "1.91.0")]
    #[must_use]
    pub fn file_prefix(&self) -> Option<&OsStr> {
        self.file_name().map(split_file_at_dot).and_then(|(before, _after)| Some(before))
    }

    /// 提取 [`self.file_name`] 的扩展名（不含前导的点号），如果可能的话。
    ///
    /// 扩展名是：
    ///
    /// * [`None`]，如果没有文件名；
    /// * [`None`]，如果没有内嵌的 `.`；
    /// * [`None`]，如果文件名以 `.` 开头且其内部没有其他 `.`；
    /// * 否则，文件名中最后一个 `.` 之后的部分
    ///
    /// [`self.file_name`]: Path::file_name
    ///
    /// # 示例
    ///
    /// ```
    /// use std::path::Path;
    ///
    /// assert_eq!("rs", Path::new("foo.rs").extension().unwrap());
    /// assert_eq!("gz", Path::new("foo.tar.gz").extension().unwrap());
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[must_use]
    pub fn extension(&self) -> Option<&OsStr> {
        self.file_name().map(rsplit_file_at_dot).and_then(|(before, after)| before.and(after))
    }

    /// 检查路径是否以一个尾部[分隔符](MAIN_SEPARATOR)结尾。
    ///
    /// 这通常用于确保某条路径被当作目录而非文件来对待，尽管它并不能真正保证这样一条路径
    /// 在底层文件系统上确实是一个目录。
    ///
    /// 尽管有这种行为，在 Rust 中两条路径无论是否带有尾部分隔符，仍被视为相同。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(path_trailing_sep)]
    /// use std::path::Path;
    ///
    /// assert!(Path::new("dir/").has_trailing_sep());
    /// assert!(!Path::new("file.rs").has_trailing_sep());
    /// ```
    #[unstable(feature = "path_trailing_sep", issue = "142503")]
    #[must_use]
    #[inline]
    pub fn has_trailing_sep(&self) -> bool {
        self.as_os_str().as_encoded_bytes().last().copied().is_some_and(is_sep_byte)
    }

    /// 确保路径带有一个尾部[分隔符](MAIN_SEPARATOR)，必要时会分配一个 [`PathBuf`]。
    ///
    /// 结果路径对 [`has_trailing_sep`](Self::has_trailing_sep) 将返回 true。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(path_trailing_sep)]
    /// use std::ffi::OsStr;
    /// use std::path::Path;
    ///
    /// assert_eq!(Path::new("dir//").with_trailing_sep().as_os_str(), OsStr::new("dir//"));
    /// assert_eq!(Path::new("dir/").with_trailing_sep().as_os_str(), OsStr::new("dir/"));
    /// assert!(!Path::new("dir").has_trailing_sep());
    /// assert!(Path::new("dir").with_trailing_sep().has_trailing_sep());
    /// ```
    #[unstable(feature = "path_trailing_sep", issue = "142503")]
    #[must_use]
    #[inline]
    pub fn with_trailing_sep(&self) -> Cow<'_, Path> {
        if self.has_trailing_sep() { Cow::Borrowed(self) } else { Cow::Owned(self.join("")) }
    }

    /// 从路径中修剪掉一个尾部[分隔符](MAIN_SEPARATOR)，如果可能的话。
    ///
    /// 对于大多数路径，结果路径对 [`has_trailing_sep`](Self::has_trailing_sep) 将返回 false。
    ///
    /// 某些路径（例如 `/`）无法以这种方式修剪。
    ///
    /// # 示例
    ///
    /// ```
    /// #![feature(path_trailing_sep)]
    /// use std::ffi::OsStr;
    /// use std::path::Path;
    ///
    /// assert_eq!(Path::new("dir//").trim_trailing_sep().as_os_str(), OsStr::new("dir"));
    /// assert_eq!(Path::new("dir/").trim_trailing_sep().as_os_str(), OsStr::new("dir"));
    /// assert_eq!(Path::new("dir").trim_trailing_sep().as_os_str(), OsStr::new("dir"));
    /// assert_eq!(Path::new("/").trim_trailing_sep().as_os_str(), OsStr::new("/"));
    /// assert_eq!(Path::new("//").trim_trailing_sep().as_os_str(), OsStr::new("//"));
    /// ```
    #[unstable(feature = "path_trailing_sep", issue = "142503")]
    #[must_use]
    #[inline]
    pub fn trim_trailing_sep(&self) -> &Path {
        if self.has_trailing_sep() && (!self.has_root() || self.parent().is_some()) {
            let mut bytes = self.inner.as_encoded_bytes();
            while let Some((last, init)) = bytes.split_last()
                && is_sep_byte(*last)
            {
                bytes = init;
            }

            // SAFETY: 修剪尾部的 ASCII 字节会保持该字符串的合法性。
            Path::new(unsafe { OsStr::from_encoded_bytes_unchecked(bytes) })
        } else {
            self
        }
    }

    /// 创建一个持有所有权的 [`PathBuf`]，其内容为把 `path` 拼接到 `self` 之后。
    ///
    /// 如果 `path` 是绝对路径，它会替换掉当前路径。
    ///
    /// 在 Windows 上：
    ///
    /// * 如果 `path` 带有根但没有前缀（例如 `\windows`），它会替换并返回 `self`
    ///   的前缀（如果有的话）之外的一切。
    /// * 如果 `path` 带有前缀但没有根，则 `self` 被忽略，直接返回 `path`。
    /// * 如果 `self` 带有逐字前缀（例如 `\\?\C:\windows`）且 `path` 非空，
    ///   则新路径会被规范化：所有对 `.` 和 `..` 的引用都会被移除。
    ///
    /// 关于“拼接一条路径”意味着什么的更多细节，参见 [`PathBuf::push`]。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::path::{Path, PathBuf};
    ///
    /// assert_eq!(Path::new("/etc").join("passwd"), PathBuf::from("/etc/passwd"));
    /// assert_eq!(Path::new("/etc").join("/bin/sh"), PathBuf::from("/bin/sh"));
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[must_use]
    pub fn join<P: AsRef<Path>>(&self, path: P) -> PathBuf {
        self._join(path.as_ref())
    }

    fn _join(&self, path: &Path) -> PathBuf {
        let mut buf = self.to_path_buf();
        buf.push(path);
        buf
    }

    /// 创建一个持有所有权的 [`PathBuf`]，它像 `self`，但带有给定的文件名。
    ///
    /// 更多细节参见 [`PathBuf::set_file_name`]。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::path::{Path, PathBuf};
    ///
    /// let path = Path::new("/tmp/foo.png");
    /// assert_eq!(path.with_file_name("bar"), PathBuf::from("/tmp/bar"));
    /// assert_eq!(path.with_file_name("bar.txt"), PathBuf::from("/tmp/bar.txt"));
    ///
    /// let path = Path::new("/tmp");
    /// assert_eq!(path.with_file_name("var"), PathBuf::from("/var"));
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[must_use]
    pub fn with_file_name<S: AsRef<OsStr>>(&self, file_name: S) -> PathBuf {
        self._with_file_name(file_name.as_ref())
    }

    fn _with_file_name(&self, file_name: &OsStr) -> PathBuf {
        let mut buf = self.to_path_buf();
        buf.set_file_name(file_name);
        buf
    }

    /// 创建一个持有所有权的 [`PathBuf`]，它像 `self`，但带有给定的扩展名。
    ///
    /// 更多细节参见 [`PathBuf::set_extension`]。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::path::Path;
    ///
    /// let path = Path::new("foo.rs");
    /// assert_eq!(path.with_extension("txt"), Path::new("foo.txt"));
    /// assert_eq!(path.with_extension(""), Path::new("foo"));
    /// ```
    ///
    /// 处理多重扩展名：
    ///
    /// ```
    /// use std::path::Path;
    ///
    /// let path = Path::new("foo.tar.gz");
    /// assert_eq!(path.with_extension("xz"), Path::new("foo.tar.xz"));
    /// assert_eq!(path.with_extension("").with_extension("txt"), Path::new("foo.txt"));
    /// ```
    ///
    /// 在原本没有扩展名的情况下添加一个扩展名：
    ///
    /// ```
    /// use std::path::Path;
    ///
    /// let path = Path::new("foo");
    /// assert_eq!(path.with_extension("rs"), Path::new("foo.rs"));
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn with_extension<S: AsRef<OsStr>>(&self, extension: S) -> PathBuf {
        self._with_extension(extension.as_ref())
    }

    fn _with_extension(&self, extension: &OsStr) -> PathBuf {
        let self_len = self.as_os_str().len();
        let self_bytes = self.as_os_str().as_encoded_bytes();

        let (new_capacity, slice_to_copy) = match self.extension() {
            None => {
                // 足够容纳扩展名和点号的容量
                let capacity = self_len + extension.len() + 1;
                let whole_path = self_bytes;
                (capacity, whole_path)
            }
            Some(previous_extension) => {
                let capacity = self_len + extension.len() - previous_extension.len();
                let path_till_dot = &self_bytes[..self_len - previous_extension.len()];
                (capacity, path_till_dot)
            }
        };

        let mut new_path = PathBuf::with_capacity(new_capacity);
        // SAFETY: 该路径为空，因此不可能含有代理项的一半。
        unsafe { new_path.inner.extend_from_slice_unchecked(slice_to_copy) };
        new_path.set_extension(extension);
        new_path
    }

    /// 创建一个持有所有权的 [`PathBuf`]，它像 `self`，但追加了一个扩展名。
    ///
    /// 更多细节参见 [`PathBuf::add_extension`]。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::path::{Path, PathBuf};
    ///
    /// let path = Path::new("foo.rs");
    /// assert_eq!(path.with_added_extension("txt"), PathBuf::from("foo.rs.txt"));
    ///
    /// let path = Path::new("foo.tar.gz");
    /// assert_eq!(path.with_added_extension(""), PathBuf::from("foo.tar.gz"));
    /// assert_eq!(path.with_added_extension("xz"), PathBuf::from("foo.tar.gz.xz"));
    /// assert_eq!(path.with_added_extension("").with_added_extension("txt"), PathBuf::from("foo.tar.gz.txt"));
    /// ```
    #[stable(feature = "path_add_extension", since = "1.91.0")]
    pub fn with_added_extension<S: AsRef<OsStr>>(&self, extension: S) -> PathBuf {
        let mut new_path = self.to_path_buf();
        new_path.add_extension(extension);
        new_path
    }

    /// 产出一个遍历路径各 [`Component`] 的迭代器。
    ///
    /// 在解析路径时，会进行少量的规范化：
    ///
    /// * 重复的分隔符会被忽略，所以 `a/b` 与 `a//b` 都以 `a` 和 `b` 作为组件。
    ///
    /// * `.` 的出现会被规范化掉，但位于路径开头的除外。例如 `a/./b`、`a/b/`、`a/b/.`
    ///   与 `a/b` 都以 `a` 和 `b` 作为组件，但 `./a/b` 会以一个额外的 [`CurDir`]
    ///   组件开头。
    ///
    /// * 尾部分隔符会被规范化掉，所以 `/a/b` 与 `/a/b/` 是等价的。
    ///
    /// 注意，不会进行任何其他规范化；特别地，`a/c` 与 `a/b/../c` 是不同的，
    /// 以考虑到 `b` 可能是一个符号链接（因此其父目录不是 `a`）的可能性。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::path::{Path, Component};
    /// use std::ffi::OsStr;
    ///
    /// let mut components = Path::new("/tmp/foo.txt").components();
    ///
    /// assert_eq!(components.next(), Some(Component::RootDir));
    /// assert_eq!(components.next(), Some(Component::Normal(OsStr::new("tmp"))));
    /// assert_eq!(components.next(), Some(Component::Normal(OsStr::new("foo.txt"))));
    /// assert_eq!(components.next(), None)
    /// ```
    ///
    /// [`CurDir`]: Component::CurDir
    #[stable(feature = "rust1", since = "1.0.0")]
    pub fn components(&self) -> Components<'_> {
        let prefix = parse_prefix(self.as_os_str());
        Components {
            path: self.as_u8_slice(),
            prefix,
            has_physical_root: has_physical_root(self.as_u8_slice(), prefix),
            // 使用一个平台特定的初始状态，以便在平台没有 Prefix 时省去状态机的一次转移。
            front: const { if HAS_PREFIXES { State::Prefix } else { State::StartDir } },
            back: State::Body,
        }
    }

    /// 产出一个迭代器，把路径的各组件视作 [`OsStr`] 切片来遍历。
    ///
    /// 关于路径如何被切分为组件的具体细节，参见 [`components`]。
    ///
    /// [`components`]: Path::components
    ///
    /// # 示例
    ///
    /// ```
    /// use std::path::{self, Path};
    /// use std::ffi::OsStr;
    ///
    /// let mut it = Path::new("/tmp/foo.txt").iter();
    /// assert_eq!(it.next(), Some(OsStr::new(&path::MAIN_SEPARATOR.to_string())));
    /// assert_eq!(it.next(), Some(OsStr::new("tmp")));
    /// assert_eq!(it.next(), Some(OsStr::new("foo.txt")));
    /// assert_eq!(it.next(), None)
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[inline]
    pub fn iter(&self) -> Iter<'_> {
        Iter { inner: self.components() }
    }

    /// 返回一个实现了 [`Display`] 的对象，用于安全地打印可能包含非 Unicode 数据的路径。
    /// 取决于平台，这可能进行有损转换。如果你想要一个会对路径做转义的实现，请改用 [`Debug`]。
    ///
    /// [`Display`]: fmt::Display
    /// [`Debug`]: fmt::Debug
    ///
    /// # 示例
    ///
    /// ```
    /// use std::path::Path;
    ///
    /// let path = Path::new("/tmp/foo.rs");
    ///
    /// println!("{}", path.display());
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[must_use = "this does not display the path, \
                  it returns an object that can be displayed"]
    #[inline]
    pub fn display(&self) -> Display<'_> {
        Display { inner: self.inner.display() }
    }

    /// 以 `&Path` 形式返回同一条路径。
    ///
    /// 直接用在 `&Path` 上时这个方法是冗余的，但它有助于把其他类 `PathBuf` 的类型
    /// 解引用为 `Path`，例如对 `Box<Path>` 或 `Arc<Path>` 的引用。
    #[inline]
    #[unstable(feature = "str_as_str", issue = "130366")]
    pub const fn as_path(&self) -> &Path {
        self
    }

    /// 查询文件系统，以获取关于某个文件、目录等的信息。
    ///
    /// 本函数会沿符号链接前进，从而查询链接目标文件的信息。
    ///
    /// 这是 [`fs::metadata`] 的别名。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::path::Path;
    ///
    /// let path = Path::new("/Minas/tirith");
    /// let metadata = path.metadata().expect("metadata call failed");
    /// println!("{:?}", metadata.file_type());
    /// ```
    #[stable(feature = "path_ext", since = "1.5.0")]
    #[inline]
    pub fn metadata(&self) -> io::Result<fs::Metadata> {
        fs::metadata(self)
    }

    /// 查询关于某个文件的元数据，且不沿符号链接前进。
    ///
    /// 这是 [`fs::symlink_metadata`] 的别名。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::path::Path;
    ///
    /// let path = Path::new("/Minas/tirith");
    /// let metadata = path.symlink_metadata().expect("symlink_metadata call failed");
    /// println!("{:?}", metadata.file_type());
    /// ```
    #[stable(feature = "path_ext", since = "1.5.0")]
    #[inline]
    pub fn symlink_metadata(&self) -> io::Result<fs::Metadata> {
        fs::symlink_metadata(self)
    }

    /// 返回该路径的规范、绝对形式，其中所有中间组件都被规范化、所有符号链接都被解析。
    ///
    /// 这是 [`fs::canonicalize`] 的别名。
    ///
    /// # 错误(Errors）
    ///
    /// 本方法会在以下情形（但不限于这些情形）返回错误：
    ///
    /// * `path` 不存在。
    /// * path 中某个非末尾组件不是目录。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::path::{Path, PathBuf};
    ///
    /// let path = Path::new("/foo/test/../test/bar.rs");
    /// assert_eq!(path.canonicalize().unwrap(), PathBuf::from("/foo/test/bar.rs"));
    /// ```
    #[stable(feature = "path_ext", since = "1.5.0")]
    #[inline]
    pub fn canonicalize(&self) -> io::Result<PathBuf> {
        fs::canonicalize(self)
    }

    /// 规范化一条路径，包括处理 `..`，且不遍历文件系统。
    ///
    /// 若规范化后会留下前导的 `..` 组件，则返回错误。
    ///
    /// <div class="warning">
    ///
    /// 本函数总是把 `..` 解析为“词法上的”父目录。
    /// 也就是说 "a/b/../c" 总会被解析为 `a/c`，这可能改变路径的含义。
    /// 特别地，`a/c` 与 `a/b/../c` 在很多系统上是不同的，因为 `b` 可能是一个符号链接，故其父目录不是 `a`。
    ///
    /// </div>
    ///
    /// [`path::absolute`](absolute) 是一个会保留 `..` 的替代方案。
    /// 或者可以用 [`Path::canonicalize`]，通过查询文件系统来解析任何 `..`。
    #[unstable(feature = "normalize_lexically", issue = "134694")]
    pub fn normalize_lexically(&self) -> Result<PathBuf, NormalizeError> {
        let mut lexical = PathBuf::new();
        let mut iter = self.components().peekable();

        // 找出根（如果有的话），并把它加入到 lexical 路径中。
        // 这里我们把 Windows 路径 "C:\" 当作单个“根”来对待，尽管
        // `components` 会把它拆分为两部分：(Prefix, RootDir)。
        let root = match iter.peek() {
            Some(Component::ParentDir) => return Err(NormalizeError),
            Some(p @ Component::RootDir) | Some(p @ Component::CurDir) => {
                lexical.push(p);
                iter.next();
                lexical.as_os_str().len()
            }
            Some(Component::Prefix(prefix)) => {
                lexical.push(prefix.as_os_str());
                iter.next();
                if let Some(p @ Component::RootDir) = iter.peek() {
                    lexical.push(p);
                    iter.next();
                }
                lexical.as_os_str().len()
            }
            None => return Ok(PathBuf::new()),
            Some(Component::Normal(_)) => 0,
        };

        for component in iter {
            match component {
                Component::RootDir => unreachable!(),
                Component::Prefix(_) => return Err(NormalizeError),
                Component::CurDir => continue,
                Component::ParentDir => {
                    // 如果 ParentDir 会导致我们越过“根”，则这是一个错误。
                    if lexical.as_os_str().len() == root {
                        return Err(NormalizeError);
                    } else {
                        lexical.pop();
                    }
                }
                Component::Normal(path) => lexical.push(path),
            }
        }
        Ok(lexical)
    }

    /// 读取一个符号链接，返回该链接所指向的文件。
    ///
    /// 这是 [`fs::read_link`] 的别名。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::path::Path;
    ///
    /// let path = Path::new("/laputa/sky_castle.rs");
    /// let path_link = path.read_link().expect("read_link call failed");
    /// ```
    #[stable(feature = "path_ext", since = "1.5.0")]
    #[inline]
    pub fn read_link(&self) -> io::Result<PathBuf> {
        fs::read_link(self)
    }

    /// 返回一个遍历某个目录内各条目的迭代器。
    ///
    /// 该迭代器将产出 <code>[io::Result]<[fs::DirEntry]></code> 类型的实例。在迭代器最初构造之后，
    /// 仍可能遇到新的错误。
    ///
    /// 这是 [`fs::read_dir`] 的别名。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::path::Path;
    ///
    /// let path = Path::new("/laputa");
    /// for entry in path.read_dir().expect("read_dir call failed") {
    ///     if let Ok(entry) = entry {
    ///         println!("{:?}", entry.path());
    ///     }
    /// }
    /// ```
    #[stable(feature = "path_ext", since = "1.5.0")]
    #[inline]
    pub fn read_dir(&self) -> io::Result<fs::ReadDir> {
        fs::read_dir(self)
    }

    /// 若路径指向一个已存在的实体，则返回 `true`。
    ///
    /// 警告：这个方法可能容易出错，考虑改用 [`try_exists()`]！
    /// 它还有引入“检查时刻到使用时刻”（[TOCTOU]）类 bug 的风险。
    ///
    /// 本函数会沿符号链接前进，从而查询链接目标文件的信息。
    ///
    /// 如果你无法访问该文件的元数据（例如由于权限错误或断裂的符号链接），这将返回 `false`。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::path::Path;
    /// assert!(!Path::new("does_not_exist.txt").exists());
    /// ```
    ///
    /// # See Also
    ///
    /// 这是一个会把错误强制转换为 false 的便捷函数。如果你想检查错误，请调用 [`Path::try_exists`]。
    ///
    /// [`try_exists()`]: Self::try_exists
    /// [TOCTOU]: fs#time-of-check-to-time-of-use-toctou
    #[stable(feature = "path_ext", since = "1.5.0")]
    #[must_use]
    #[inline]
    pub fn exists(&self) -> bool {
        fs::metadata(self).is_ok()
    }

    /// 若路径指向一个已存在的实体，则返回 `Ok(true)`。
    ///
    /// 本函数会沿符号链接前进，从而查询链接目标文件的信息。对于断裂的符号链接，这将返回 `Ok(false)`。
    ///
    /// [`Path::exists()`] 只检查路径是否既能被找到又可读。相比之下，`try_exists`
    /// 会在路径被_证实_存在或不存在时分别返回 `Ok(true)` 或 `Ok(false)`。如果其存在与否
    /// 既无法确认也无法否认，则改为传播一个 `Err(_)`。例如当某个父目录的列举权限被拒绝时，
    /// 就可能出现这种情况。
    ///
    /// 注意，虽然这避免了 `exists()` 方法的一些陷阱，但它仍然无法防止“检查时刻到使用时刻”
    ///（[TOCTOU]）类 bug。你应当只在那些此类 bug 不成问题的场景中使用它。
    ///
    /// 这是 [`std::fs::exists`](crate::fs::exists) 的别名。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::path::Path;
    /// assert!(!Path::new("does_not_exist.txt").try_exists().expect("Can't check existence of file does_not_exist.txt"));
    /// assert!(Path::new("/root/secret_file.txt").try_exists().is_err());
    /// ```
    ///
    /// [TOCTOU]: fs#time-of-check-to-time-of-use-toctou
    /// [`exists()`]: Self::exists
    #[stable(feature = "path_try_exists", since = "1.63.0")]
    #[inline]
    pub fn try_exists(&self) -> io::Result<bool> {
        fs::exists(self)
    }

    /// 若路径在磁盘上存在且指向一个常规文件，则返回 `true`。
    ///
    /// 本函数会沿符号链接前进，从而查询链接目标文件的信息。
    ///
    /// 如果你无法访问该文件的元数据（例如由于权限错误或断裂的符号链接），这将返回 `false`。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::path::Path;
    /// assert_eq!(Path::new("./is_a_directory/").is_file(), false);
    /// assert_eq!(Path::new("a_file.txt").is_file(), true);
    /// ```
    ///
    /// # See Also
    ///
    /// 这是一个会把错误强制转换为 false 的便捷函数。如果你想检查错误，请调用 [`fs::metadata`]
    /// 并处理它的 [`Result`]，然后在结果为 [`Ok`] 时调用 [`fs::Metadata::is_file`]。
    ///
    /// 当目的只是从源读取（或向其写入）时，测试该源能否被读取（或写入）的最可靠方式是打开它。
    /// 仅使用 `is_file` 可能会破坏诸如类 Unix 系统上 `diff <( prog_a )` 这样的工作流。
    /// 更多信息参见 [`fs::File::open`] 或 [`fs::OpenOptions::open`]。
    #[stable(feature = "path_ext", since = "1.5.0")]
    #[must_use]
    pub fn is_file(&self) -> bool {
        fs::metadata(self).map(|m| m.is_file()).unwrap_or(false)
    }

    /// 若路径在磁盘上存在且指向一个目录，则返回 `true`。
    ///
    /// 本函数会沿符号链接前进，从而查询链接目标文件的信息。
    ///
    /// 如果你无法访问该文件的元数据（例如由于权限错误或断裂的符号链接），这将返回 `false`。
    ///
    /// # 示例
    ///
    /// ```no_run
    /// use std::path::Path;
    /// assert_eq!(Path::new("./is_a_directory/").is_dir(), true);
    /// assert_eq!(Path::new("a_file.txt").is_dir(), false);
    /// ```
    ///
    /// # See Also
    ///
    /// 这是一个会把错误强制转换为 false 的便捷函数。如果你想检查错误，请调用 [`fs::metadata`]
    /// 并处理它的 [`Result`]，然后在结果为 [`Ok`] 时调用 [`fs::Metadata::is_dir`]。
    #[stable(feature = "path_ext", since = "1.5.0")]
    #[must_use]
    pub fn is_dir(&self) -> bool {
        fs::metadata(self).map(|m| m.is_dir()).unwrap_or(false)
    }

    /// 若路径在磁盘上存在且指向一个符号链接，则返回 `true`。
    ///
    /// 本函数不会沿符号链接前进。
    /// 对于断裂的符号链接，这同样会返回 true。
    ///
    /// 如果你无法访问包含该文件的目录（例如由于权限错误），这将返回 false。
    ///
    /// # 示例
    ///
    /// ```rust,no_run
    /// # #[cfg(unix)] {
    /// use std::path::Path;
    /// use std::os::unix::fs::symlink;
    ///
    /// let link_path = Path::new("link");
    /// symlink("/origin_does_not_exist/", link_path).unwrap();
    /// assert_eq!(link_path.is_symlink(), true);
    /// assert_eq!(link_path.exists(), false);
    /// # }
    /// ```
    ///
    /// # See Also
    ///
    /// 这是一个会把错误强制转换为 false 的便捷函数。如果你想检查错误，请调用 [`fs::symlink_metadata`]
    /// 并处理它的 [`Result`]，然后在结果为 [`Ok`] 时调用 [`fs::Metadata::is_symlink`]。
    #[must_use]
    #[stable(feature = "is_symlink", since = "1.58.0")]
    pub fn is_symlink(&self) -> bool {
        fs::symlink_metadata(self).map(|m| m.is_symlink()).unwrap_or(false)
    }

    /// 将一个 [`Box<Path>`](Box) 转换为一个 [`PathBuf`]，且不拷贝或分配。
    #[stable(feature = "into_boxed_path", since = "1.20.0")]
    #[must_use = "`self` will be dropped if the result is not used"]
    pub fn into_path_buf(self: Box<Self>) -> PathBuf {
        let rw = Box::into_raw(self) as *mut OsStr;
        let inner = unsafe { Box::from_raw(rw) };
        PathBuf { inner: OsString::from(inner) }
    }
}

#[unstable(feature = "clone_to_uninit", issue = "126799")]
unsafe impl CloneToUninit for Path {
    #[inline]
    #[cfg_attr(debug_assertions, track_caller)]
    unsafe fn clone_to_uninit(&self, dst: *mut u8) {
        // SAFETY: Path 只是对 OsStr 的一层透明（transparent）包装
        unsafe { self.inner.clone_to_uninit(dst) }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl const AsRef<OsStr> for Path {
    #[inline]
    fn as_ref(&self) -> &OsStr {
        &self.inner
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl fmt::Debug for Path {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.inner, formatter)
    }
}

/// 一个辅助结构体，用于配合 [`format!`] 和 `{}` 安全地打印路径。
///
/// [`Path`] 可能包含非 Unicode 数据。该 `struct` 以一种能缓解该问题的方式实现了
/// [`Display`] trait。它由 [`Path`] 上的 [`display`](Path::display) 方法创建。
/// 取决于平台，这可能进行有损转换。如果你想要一个会对路径做转义的实现，请改用 [`Debug`]。
///
/// # 示例
///
/// ```
/// use std::path::Path;
///
/// let path = Path::new("/tmp/foo.rs");
///
/// println!("{}", path.display());
/// ```
///
/// [`Display`]: fmt::Display
/// [`format!`]: crate::format
#[stable(feature = "rust1", since = "1.0.0")]
pub struct Display<'a> {
    inner: os_str::Display<'a>,
}

#[stable(feature = "rust1", since = "1.0.0")]
impl fmt::Debug for Display<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.inner, f)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl fmt::Display for Display<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.inner, f)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl PartialEq for Path {
    #[inline]
    fn eq(&self, other: &Path) -> bool {
        self.components() == other.components()
    }
}

#[stable(feature = "eq_str_for_path", since = "1.91.0")]
impl cmp::PartialEq<str> for Path {
    #[inline]
    fn eq(&self, other: &str) -> bool {
        let other: &OsStr = other.as_ref();
        self == other
    }
}

#[stable(feature = "eq_str_for_path", since = "1.91.0")]
impl cmp::PartialEq<Path> for str {
    #[inline]
    fn eq(&self, other: &Path) -> bool {
        other == self
    }
}

#[stable(feature = "eq_str_for_path", since = "1.91.0")]
impl cmp::PartialEq<String> for Path {
    #[inline]
    fn eq(&self, other: &String) -> bool {
        self == other.as_str()
    }
}

#[stable(feature = "eq_str_for_path", since = "1.91.0")]
impl cmp::PartialEq<Path> for String {
    #[inline]
    fn eq(&self, other: &Path) -> bool {
        self.as_str() == other
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl Hash for Path {
    fn hash<H: Hasher>(&self, h: &mut H) {
        let bytes = self.as_u8_slice();
        let (prefix_len, verbatim) = match parse_prefix(&self.inner) {
            Some(prefix) => {
                prefix.hash(h);
                (prefix.len(), prefix.is_verbatim())
            }
            None => (0, false),
        };
        let bytes = &bytes[prefix_len..];

        let mut component_start = 0;
        // 跟踪一些额外状态，以避免前缀碰撞。
        // ["foo", "bar"] 和 ["foobar"] 会有相同的有效载荷字节，
        // 但会得到不同的 chunk_bits
        let mut chunk_bits: usize = 0;

        for i in 0..bytes.len() {
            let is_sep = if verbatim { is_verbatim_sep(bytes[i]) } else { is_sep_byte(bytes[i]) };
            if is_sep {
                if i > component_start {
                    let to_hash = &bytes[component_start..i];
                    chunk_bits = chunk_bits.wrapping_add(to_hash.len());
                    chunk_bits = chunk_bits.rotate_right(2);
                    h.write(to_hash);
                }

                // 跳过分隔符，以及可选地跳过其后紧跟的一个 CurDir 项，
                // 因为 components() 会把这些规范化掉。
                component_start = i + 1;

                let tail = &bytes[component_start..];

                if !verbatim {
                    component_start += match tail {
                        [b'.'] => 1,
                        [b'.', sep, ..] if is_sep_byte(*sep) => 1,
                        _ => 0,
                    };
                }
            }
        }

        if component_start < bytes.len() {
            let to_hash = &bytes[component_start..];
            chunk_bits = chunk_bits.wrapping_add(to_hash.len());
            chunk_bits = chunk_bits.rotate_right(2);
            h.write(to_hash);
        }

        h.write_usize(chunk_bits);
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl Eq for Path {}

#[stable(feature = "rust1", since = "1.0.0")]
impl PartialOrd for Path {
    #[inline]
    fn partial_cmp(&self, other: &Path) -> Option<cmp::Ordering> {
        Some(compare_components(self.components(), other.components()))
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl Ord for Path {
    #[inline]
    fn cmp(&self, other: &Path) -> cmp::Ordering {
        compare_components(self.components(), other.components())
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl const AsRef<Path> for Path {
    #[inline]
    fn as_ref(&self) -> &Path {
        self
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
#[rustc_const_unstable(feature = "const_convert", issue = "143773")]
impl const AsRef<Path> for OsStr {
    #[inline]
    fn as_ref(&self) -> &Path {
        Path::new(self)
    }
}

#[stable(feature = "cow_os_str_as_ref_path", since = "1.8.0")]
impl AsRef<Path> for Cow<'_, OsStr> {
    #[inline]
    fn as_ref(&self) -> &Path {
        Path::new(self)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl AsRef<Path> for OsString {
    #[inline]
    fn as_ref(&self) -> &Path {
        Path::new(self)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl AsRef<Path> for str {
    #[inline]
    fn as_ref(&self) -> &Path {
        Path::new(self)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl AsRef<Path> for String {
    #[inline]
    fn as_ref(&self) -> &Path {
        Path::new(self)
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl AsRef<Path> for PathBuf {
    #[inline]
    fn as_ref(&self) -> &Path {
        self
    }
}

#[stable(feature = "path_into_iter", since = "1.6.0")]
impl<'a> IntoIterator for &'a PathBuf {
    type Item = &'a OsStr;
    type IntoIter = Iter<'a>;
    #[inline]
    fn into_iter(self) -> Iter<'a> {
        self.iter()
    }
}

#[stable(feature = "path_into_iter", since = "1.6.0")]
impl<'a> IntoIterator for &'a Path {
    type Item = &'a OsStr;
    type IntoIter = Iter<'a>;
    #[inline]
    fn into_iter(self) -> Iter<'a> {
        self.iter()
    }
}

macro_rules! impl_cmp {
    (<$($life:lifetime),*> $lhs:ty, $rhs: ty) => {
        #[stable(feature = "partialeq_path", since = "1.6.0")]
        impl<$($life),*> PartialEq<$rhs> for $lhs {
            #[inline]
            fn eq(&self, other: &$rhs) -> bool {
                <Path as PartialEq>::eq(self, other)
            }
        }

        #[stable(feature = "partialeq_path", since = "1.6.0")]
        impl<$($life),*> PartialEq<$lhs> for $rhs {
            #[inline]
            fn eq(&self, other: &$lhs) -> bool {
                <Path as PartialEq>::eq(self, other)
            }
        }

        #[stable(feature = "cmp_path", since = "1.8.0")]
        impl<$($life),*> PartialOrd<$rhs> for $lhs {
            #[inline]
            fn partial_cmp(&self, other: &$rhs) -> Option<cmp::Ordering> {
                <Path as PartialOrd>::partial_cmp(self, other)
            }
        }

        #[stable(feature = "cmp_path", since = "1.8.0")]
        impl<$($life),*> PartialOrd<$lhs> for $rhs {
            #[inline]
            fn partial_cmp(&self, other: &$lhs) -> Option<cmp::Ordering> {
                <Path as PartialOrd>::partial_cmp(self, other)
            }
        }
    };
}

impl_cmp!(<> PathBuf, Path);
impl_cmp!(<'a> PathBuf, &'a Path);
impl_cmp!(<'a> Cow<'a, Path>, Path);
impl_cmp!(<'a, 'b> Cow<'a, Path>, &'b Path);
impl_cmp!(<'a> Cow<'a, Path>, PathBuf);

macro_rules! impl_cmp_os_str {
    (<$($life:lifetime),*> $lhs:ty, $rhs: ty) => {
        #[stable(feature = "cmp_path", since = "1.8.0")]
        impl<$($life),*> PartialEq<$rhs> for $lhs {
            #[inline]
            fn eq(&self, other: &$rhs) -> bool {
                <Path as PartialEq>::eq(self, other.as_ref())
            }
        }

        #[stable(feature = "cmp_path", since = "1.8.0")]
        impl<$($life),*> PartialEq<$lhs> for $rhs {
            #[inline]
            fn eq(&self, other: &$lhs) -> bool {
                <Path as PartialEq>::eq(self.as_ref(), other)
            }
        }

        #[stable(feature = "cmp_path", since = "1.8.0")]
        impl<$($life),*> PartialOrd<$rhs> for $lhs {
            #[inline]
            fn partial_cmp(&self, other: &$rhs) -> Option<cmp::Ordering> {
                <Path as PartialOrd>::partial_cmp(self, other.as_ref())
            }
        }

        #[stable(feature = "cmp_path", since = "1.8.0")]
        impl<$($life),*> PartialOrd<$lhs> for $rhs {
            #[inline]
            fn partial_cmp(&self, other: &$lhs) -> Option<cmp::Ordering> {
                <Path as PartialOrd>::partial_cmp(self.as_ref(), other)
            }
        }
    };
}

impl_cmp_os_str!(<> PathBuf, OsStr);
impl_cmp_os_str!(<'a> PathBuf, &'a OsStr);
impl_cmp_os_str!(<'a> PathBuf, Cow<'a, OsStr>);
impl_cmp_os_str!(<> PathBuf, OsString);
impl_cmp_os_str!(<> Path, OsStr);
impl_cmp_os_str!(<'a> Path, &'a OsStr);
impl_cmp_os_str!(<'a> Path, Cow<'a, OsStr>);
impl_cmp_os_str!(<> Path, OsString);
impl_cmp_os_str!(<'a> &'a Path, OsStr);
impl_cmp_os_str!(<'a, 'b> &'a Path, Cow<'b, OsStr>);
impl_cmp_os_str!(<'a> &'a Path, OsString);
impl_cmp_os_str!(<'a> Cow<'a, Path>, OsStr);
impl_cmp_os_str!(<'a, 'b> Cow<'a, Path>, &'b OsStr);
impl_cmp_os_str!(<'a> Cow<'a, Path>, OsString);

#[stable(since = "1.7.0", feature = "strip_prefix")]
impl fmt::Display for StripPrefixError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        "prefix not found".fmt(f)
    }
}

#[stable(since = "1.7.0", feature = "strip_prefix")]
impl Error for StripPrefixError {}

#[unstable(feature = "normalize_lexically", issue = "134694")]
impl fmt::Display for NormalizeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("parent reference `..` points outside of base directory")
    }
}
#[unstable(feature = "normalize_lexically", issue = "134694")]
impl Error for NormalizeError {}

/// 在不访问文件系统的前提下使路径变为绝对路径。
///
/// 如果路径是相对路径，则以当前目录作为基准目录。所有中间组件都将依照平台特定的规则被解析，
/// 但与 [`canonicalize`][crate::fs::canonicalize] 不同，本函数不解析符号链接，且即使路径不存在
/// 也可能成功。
///
/// 如果 `path` 为空，或获取[当前目录][crate::env::current_dir]失败，则会返回一个错误。
///
/// # Platform-specific behavior
///
/// 在 POSIX 平台上，路径按 [POSIX 语义][posix-semantics]解析，但不会一直解析到符号链接。
/// 这意味着它会保留 `..` 组件和尾部分隔符。
///
/// 在 Windows 上，对于逐字路径，这将直接原样返回所给路径。对于其他路径，
/// 当前等价于调用 [`GetFullPathNameW`][windows-path]。
///
/// 在 Cygwin 上，当前等价于以 `CCP_WIN_A_TO_POSIX` 模式调用 [`cygwin_conv_path`][cygwin-path]，
/// 然后再像其他 POSIX 平台那样处理。如果给定的是一条 Windows 路径，它将被转换为一条绝对的
/// POSIX 路径，且不保留 `..`。
///
/// 注意，这些行为[将来可能改变][changes]。
///
/// # 错误(Errors）
///
/// 本函数可能在以下情形返回错误：
///
/// * 若 `path` 在语法上不合法；特别地，若它为空。
/// * 若获取[当前目录][crate::env::current_dir]失败。
///
/// # 示例
///
/// ## POSIX paths
///
/// ```
/// # #[cfg(unix)]
/// fn main() -> std::io::Result<()> {
///     use std::path::{self, Path};
///
///     // 相对路径转绝对路径
///     let absolute = path::absolute("foo/./bar")?;
///     assert!(absolute.ends_with("foo/bar"));
///
///     // 绝对路径转绝对路径
///     let absolute = path::absolute("/foo//test/.././bar.rs")?;
///     assert_eq!(absolute, Path::new("/foo/test/../bar.rs"));
///     Ok(())
/// }
/// # #[cfg(not(unix))]
/// # fn main() {}
/// ```
///
/// ## Windows paths
///
/// ```
/// # #[cfg(windows)]
/// fn main() -> std::io::Result<()> {
///     use std::path::{self, Path};
///
///     // 相对路径转绝对路径
///     let absolute = path::absolute("foo/./bar")?;
///     assert!(absolute.ends_with(r"foo\bar"));
///
///     // 绝对路径转绝对路径
///     let absolute = path::absolute(r"C:\foo//test\..\./bar.rs")?;
///
///     assert_eq!(absolute, Path::new(r"C:\foo\bar.rs"));
///     Ok(())
/// }
/// # #[cfg(not(windows))]
/// # fn main() {}
/// ```
///
/// 注意，这[将来可能改变][changes]。
///
/// [changes]: io#platform-specific-behavior
/// [posix-semantics]: https://pubs.opengroup.org/onlinepubs/9699919799/basedefs/V1_chap04.html#tag_04_13
/// [windows-path]: https://docs.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getfullpathnamew
/// [cygwin-path]: https://cygwin.com/cygwin-api/func-cygwin-conv-path.html
#[stable(feature = "absolute_path", since = "1.79.0")]
pub fn absolute<P: AsRef<Path>>(path: P) -> io::Result<PathBuf> {
    let path = path.as_ref();
    if path.as_os_str().is_empty() {
        Err(io::const_error!(io::ErrorKind::InvalidInput, "cannot make an empty path absolute"))
    } else {
        sys::path::absolute(path)
    }
}
