use crate::cmp::Ordering;
use crate::ffi::CStr;
use crate::fmt;
use crate::hash::{Hash, Hasher};
use crate::marker::PhantomData;
use crate::ptr::NonNull;

/// 包含 panic 发生位置相关信息的结构体。
///
/// 该结构由 [`PanicHookInfo::location()`] 和 [`PanicInfo::location()`] 返回。它记录的是
/// 编译器在调用点嵌入的源文件、行号和列号信息；这对 panic hook、`#[panic_handler]`、
/// 日志和诊断输出都很重要。
///
/// [`PanicInfo::location()`]: crate::panic::PanicInfo::location
/// [`PanicHookInfo::location()`]: ../../std/panic/struct.PanicHookInfo.html#method.location
///
/// # 示例
///
/// ```should_panic
/// use std::panic;
///
/// panic::set_hook(Box::new(|panic_info| {
///     if let Some(location) = panic_info.location() {
///         println!("panic occurred in file '{}' at line {}", location.file(), location.line());
///     } else {
///         println!("panic occurred but can't get location information...");
///     }
/// }));
///
/// panic!("Normal panic");
/// ```
///
/// # 比较
///
/// 相等性和排序比较按文件、行号、列号的优先级进行。文件名按字符串比较，而不是按 `Path`
/// 语义比较，这在跨平台或多路径引用同一文件时可能出乎意料。更多背景见 [`Location::file`]
/// 的文档。
#[lang = "panic_location"]
#[derive(Copy, Clone)]
#[stable(feature = "panic_hooks", since = "1.10.0")]
pub struct Location<'a> {
    // 这里使用裸指针而不是引用，因为该指针实际比指针中保存的 str 长度多有效一个字节；
    // 多出的字节是 `Location::file_as_c_str` 使用的 NUL 终止符。
    filename: NonNull<str>,
    line: u32,
    col: u32,
    _filename: PhantomData<&'a str>,
}

#[stable(feature = "panic_hooks", since = "1.10.0")]
impl PartialEq for Location<'_> {
    fn eq(&self, other: &Self) -> bool {
        // 先比较 col / line：它们比较更便宜，也更可能不同，
        // 同时不会影响最终相等性结果。
        self.col == other.col && self.line == other.line && self.file() == other.file()
    }
}

#[stable(feature = "panic_hooks", since = "1.10.0")]
impl Eq for Location<'_> {}

#[stable(feature = "panic_hooks", since = "1.10.0")]
impl Ord for Location<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.file()
            .cmp(other.file())
            .then_with(|| self.line.cmp(&other.line))
            .then_with(|| self.col.cmp(&other.col))
    }
}

#[stable(feature = "panic_hooks", since = "1.10.0")]
impl PartialOrd for Location<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[stable(feature = "panic_hooks", since = "1.10.0")]
impl Hash for Location<'_> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.file().hash(state);
        self.line.hash(state);
        self.col.hash(state);
    }
}

#[stable(feature = "panic_hooks", since = "1.10.0")]
impl fmt::Debug for Location<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Location")
            .field("file", &self.file())
            .field("line", &self.line)
            .field("column", &self.col)
            .finish()
    }
}

impl<'a> Location<'a> {
    /// 返回此函数调用者的源位置。如果该调用者本身带有 `#[track_caller]`，则继续返回它的
    /// 调用位置；这个过程会沿调用栈向上推进，直到遇到第一个不被追踪的函数体内调用点。
    /// 这就是 `panic!`、`unwrap` 和 `expect` 等 API 能报告用户调用点而不是库内部位置的原因。
    ///
    /// # 示例
    ///
    /// ```standalone_crate
    /// use std::panic::Location;
    ///
    /// /// ```
    /// ///      |1        |11       |21       |31       |41
    /// ///    +-|---------|---------|---------|---------|--------
    /// /// 15 | #[track_caller]
    /// /// 16 | fn new_location() -> &'static Location<'static> {
    /// /// 17 |     Location::caller()
    /// ///    |     ------------------| 该表达式的值取决于调用者，
    /// ///    |                       | 因为函数标记了 #[track_caller]
    /// /// 18 | }
    /// /// ```
    /// #[track_caller]
    /// fn new_location() -> &'static Location<'static> {
    ///     Location::caller()
    /// }
    ///
    /// /// ```
    /// ///      |1  |5    |11       |21       |31       |41       |51
    /// ///    +-|---|-----|---------|---------|---------|---------|---
    /// /// 29 | fn constant_location() -> &'static Location<'static> {
    /// /// 30 |     new_location()
    /// ///    |     ^ constant_location() 的任何调用都会指向这里，
    /// ///    |       与它自身从哪里被调用无关
    /// /// 31 | }
    /// /// ```
    /// fn constant_location() -> &'static Location<'static> {
    ///     new_location()
    /// }
    ///
    /// fn main() {
    ///     //      |1  |5    |11       |21       |31       |41       |51
    ///     //    +-|---|-----|---------|---------|---------|---------|---
    ///     // 29 | fn constant_location() -> &'static Location<'static> {
    ///     // 30 |     new_location()
    ///     //    |     ^ `let constant` 指向这里
    ///     // 31 | }
    ///     let constant = constant_location();
    ///     assert_eq!(constant.file(), file!());
    ///     assert_eq!((constant.line(), constant.column()), (30, 5));
    ///
    ///     let constant_2 = constant_location();
    ///     assert_eq!(
    ///         (constant.file(), constant.line(), constant.column()),
    ///         (constant_2.file(), constant_2.line(), constant_2.column())
    ///     );
    ///
    ///     //      |1        |11  |16  |21       |31
    ///     //    +-|---------|----|----|---------|------
    ///     // 55 |     let here = new_location();
    ///     //    |                ^ `let here` 指向这里，因为 `new_location()` 是调用点
    ///     // 56 |     assert_eq!(here.file(), file!());
    ///     let here = new_location();
    ///     assert_eq!(here.file(), file!());
    ///     assert_eq!((here.line(), here.column()), (55, 16));
    ///
    ///     //      |1        |11       |21       ||32      |41       |51
    ///     //    +-|---------|---------|---------||--------|---------|------
    ///     // 64 |     let yet_another_location = new_location();
    ///     //    |                                ^ `let yet_another_location` 指向这里
    ///     // 65 |     assert_eq!(here.file(), yet_another_location.file());
    ///     let yet_another_location = new_location();
    ///     assert_eq!(here.file(), yet_another_location.file());
    ///     assert_ne!(
    ///         (here.line(), here.column()),
    ///         (yet_another_location.line(), yet_another_location.column())
    ///     );
    /// }
    /// ```
    #[must_use]
    #[stable(feature = "track_caller", since = "1.46.0")]
    #[rustc_const_stable(feature = "const_caller_location", since = "1.79.0")]
    #[track_caller]
    #[inline]
    pub const fn caller() -> &'static Location<'static> {
        crate::intrinsics::caller_location()
    }

    /// 返回 panic 起源位置所在的源文件名。
    ///
    /// # `&str`，不是 `&Path`
    ///
    /// 返回的名称指向编译系统上的源路径，但不能把它直接表示为 `&Path`。编译后的代码可能在
    /// 与提供源内容的系统不同的平台上运行，而目标平台的 `Path` 语义也可能不同；当前库中也
    /// 没有单独的“host path”类型来表达编译主机路径。
    ///
    /// 最容易让人意外的情况是，“同一个”文件在模块系统中可通过多个路径到达（通常来自
    /// `#[path = "..."]` 属性或类似机制）。此时看起来相同的代码，可能从此函数返回不同值。
    ///
    /// # 交叉编译
    ///
    /// 当 host 平台和 target 平台不同时，该值不适合传给 `Path::new` 或类似构造函数。
    ///
    /// # 示例
    ///
    /// ```should_panic
    /// use std::panic;
    ///
    /// panic::set_hook(Box::new(|panic_info| {
    ///     if let Some(location) = panic_info.location() {
    ///         println!("panic occurred in file '{}'", location.file());
    ///     } else {
    ///         println!("panic occurred but can't get location information...");
    ///     }
    /// }));
    ///
    /// panic!("Normal panic");
    /// ```
    #[must_use]
    #[stable(feature = "panic_hooks", since = "1.10.0")]
    #[rustc_const_stable(feature = "const_location_fields", since = "1.79.0")]
    pub const fn file(&self) -> &'a str {
        // SAFETY: `filename` 来自编译器生成的 `Location`，在生命周期 `'a` 内是有效 str。
        unsafe { self.filename.as_ref() }
    }

    /// 以 nul 终止的 `CStr` 形式返回源文件名。
    ///
    /// 这适合与期望 C/C++ `__FILE__` 或 `std::source_location::file_name` 的 API 互操作；
    /// 它们都会返回 nul 终止的 `const char*`。
    #[must_use]
    #[inline]
    #[stable(feature = "file_with_nul", since = "1.92.0")]
    #[rustc_const_stable(feature = "file_with_nul", since = "1.92.0")]
    pub const fn file_as_c_str(&self) -> &'a CStr {
        let filename = self.filename.as_ptr();

        // SAFETY: 文件名对 `filename_len+1` 字节有效，因此这里加 1 不会溢出。
        let cstr_len = unsafe { crate::mem::size_of_val_raw(filename).unchecked_add(1) };

        // SAFETY: 文件名对 `filename_len+1` 字节有效，可构造覆盖 NUL 终止符的字节切片。
        let slice = unsafe { crate::slice::from_raw_parts(filename.cast(), cstr_len) };

        // SAFETY: 文件名保证带有末尾 nul 字节，并且内部不含 nul 字节。
        unsafe { CStr::from_bytes_with_nul_unchecked(slice) }
    }

    /// 返回 panic 起源位置的行号。
    ///
    /// # 示例
    ///
    /// ```should_panic
    /// use std::panic;
    ///
    /// panic::set_hook(Box::new(|panic_info| {
    ///     if let Some(location) = panic_info.location() {
    ///         println!("panic occurred at line {}", location.line());
    ///     } else {
    ///         println!("panic occurred but can't get location information...");
    ///     }
    /// }));
    ///
    /// panic!("Normal panic");
    /// ```
    #[must_use]
    #[stable(feature = "panic_hooks", since = "1.10.0")]
    #[rustc_const_stable(feature = "const_location_fields", since = "1.79.0")]
    #[inline]
    pub const fn line(&self) -> u32 {
        self.line
    }

    /// 返回 panic 起源位置的列号。
    ///
    /// # 示例
    ///
    /// ```should_panic
    /// use std::panic;
    ///
    /// panic::set_hook(Box::new(|panic_info| {
    ///     if let Some(location) = panic_info.location() {
    ///         println!("panic occurred at column {}", location.column());
    ///     } else {
    ///         println!("panic occurred but can't get location information...");
    ///     }
    /// }));
    ///
    /// panic!("Normal panic");
    /// ```
    #[must_use]
    #[stable(feature = "panic_col", since = "1.25.0")]
    #[rustc_const_stable(feature = "const_location_fields", since = "1.79.0")]
    #[inline]
    pub const fn column(&self) -> u32 {
        self.col
    }
}

#[stable(feature = "panic_hook_display", since = "1.26.0")]
impl fmt::Display for Location<'_> {
    #[inline]
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}:{}:{}", self.file(), self.line, self.col)
    }
}

#[stable(feature = "panic_hooks", since = "1.10.0")]
unsafe impl Send for Location<'_> {}
#[stable(feature = "panic_hooks", since = "1.10.0")]
unsafe impl Sync for Location<'_> {}
