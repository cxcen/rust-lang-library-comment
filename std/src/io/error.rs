#[cfg(test)]
mod tests;

// 在 64 位平台上，`io::Error` 可能使用一种位压缩表示来缩减自身大小。然而，这种表示假定
// 错误码总是 32 位宽的。
//
// 这个假定在 64 位 UEFI 上并不成立，那里的错误码是 64 位的。因此，对于 UEFI 目标平台，
// 这种压缩表示被显式禁用，必须改用未压缩表示。
#[cfg(all(target_pointer_width = "64", not(target_os = "uefi")))]
mod repr_bitpacked;
#[cfg(all(target_pointer_width = "64", not(target_os = "uefi")))]
use repr_bitpacked::Repr;

#[cfg(any(not(target_pointer_width = "64"), target_os = "uefi"))]
mod repr_unpacked;
#[cfg(any(not(target_pointer_width = "64"), target_os = "uefi"))]
use repr_unpacked::Repr;

use crate::{error, fmt, result, sys};

/// 用于 I/O 操作的特化 [`Result`] 类型。
///
/// 这个类型在整个 [`std::io`] 中被广泛用于任何可能产生错误的操作。
///
/// 这个类型别名通常用来避免直接写出 [`io::Error`]，除此之外它就是 [`Result`] 的一个直接映射。
///
/// 尽管 Rust 的惯常风格是直接导入类型，但 [`Result`] 的别名往往不这样做，以便更容易区分它们。
/// [`Result`] 一般被默认认为是 [`std::result::Result`][`Result`]，因此本别名的使用者通常会写
/// `io::Result`，而不是去遮蔽（shadow）[预导入模块][prelude]里对
/// [`std::result::Result`][`Result`] 的导入。
///
/// [`std::io`]: crate::io
/// [`io::Error`]: Error
/// [`Result`]: crate::result::Result
/// [prelude]: crate::prelude
///
/// # 示例
///
/// 一个把 `io::Result` 向上冒泡给调用方的便捷函数：
///
/// ```
/// use std::io;
///
/// fn get_string() -> io::Result<String> {
///     let mut buffer = String::new();
///
///     io::stdin().read_line(&mut buffer)?;
///
///     Ok(buffer)
/// }
/// ```
#[stable(feature = "rust1", since = "1.0.0")]
#[doc(search_unbox)]
pub type Result<T> = result::Result<T, Error>;

/// [`Read`]、[`Write`]、[`Seek`] 等相关 trait 进行 I/O 操作时所用的错误类型。
///
/// 错误大多源自底层操作系统，但也可以通过精心构造的错误消息和特定的 [`ErrorKind`] 值，
/// 创建出自定义的 `Error` 实例。
///
/// [`Read`]: crate::io::Read
/// [`Write`]: crate::io::Write
/// [`Seek`]: crate::io::Seek
#[stable(feature = "rust1", since = "1.0.0")]
pub struct Error {
    repr: Repr,
}

#[stable(feature = "rust1", since = "1.0.0")]
impl fmt::Debug for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.repr, f)
    }
}

/// std 内部使用的常见错误常量。
#[allow(dead_code)]
impl Error {
    pub(crate) const INVALID_UTF8: Self =
        const_error!(ErrorKind::InvalidData, "stream did not contain valid UTF-8");

    pub(crate) const READ_EXACT_EOF: Self =
        const_error!(ErrorKind::UnexpectedEof, "failed to fill whole buffer");

    pub(crate) const UNKNOWN_THREAD_COUNT: Self = const_error!(
        ErrorKind::NotFound,
        "the number of hardware threads is not known for the target platform",
    );

    pub(crate) const UNSUPPORTED_PLATFORM: Self =
        const_error!(ErrorKind::Unsupported, "operation not supported on this platform");

    pub(crate) const WRITE_ALL_EOF: Self =
        const_error!(ErrorKind::WriteZero, "failed to write whole buffer");

    pub(crate) const ZERO_TIMEOUT: Self =
        const_error!(ErrorKind::InvalidInput, "cannot set a 0 duration timeout");

    pub(crate) const NO_ADDRESSES: Self =
        const_error!(ErrorKind::InvalidInput, "could not resolve to any addresses");
}

#[stable(feature = "rust1", since = "1.0.0")]
impl From<alloc::ffi::NulError> for Error {
    /// 把一个 [`alloc::ffi::NulError`] 转换成 [`Error`]。
    fn from(_: alloc::ffi::NulError) -> Error {
        const_error!(ErrorKind::InvalidInput, "data provided contains a nul byte")
    }
}

#[stable(feature = "io_error_from_try_reserve", since = "1.78.0")]
impl From<alloc::collections::TryReserveError> for Error {
    /// 把 `TryReserveError` 转换成一个带 [`ErrorKind::OutOfMemory`] 的错误。
    ///
    /// `TryReserveError` 不会作为错误的 `source()` 提供出来，但这一点未来可能改变。
    fn from(_: alloc::collections::TryReserveError) -> Error {
        // ErrorData::Custom 会进行分配，这对于处理 OOM（内存耗尽）错误来说并不理想。
        ErrorKind::OutOfMemory.into()
    }
}

// 只在测试中 derive Debug，以确保它不会被意外打印出来。
#[cfg_attr(test, derive(Debug))]
enum ErrorData<C> {
    Os(RawOsError),
    Simple(ErrorKind),
    SimpleMessage(&'static SimpleMessage),
    Custom(C),
}

/// [`Error::raw_os_error`] 所返回的原始 OS 错误码类型。
///
/// 在目前所有受支持的平台上这都是 [`i32`]，但未来新增的平台（例如 UEFI）可能使用别的原生类型，
/// 比如 [`usize`]。在适用之处使用 `as` 或 [`into`] 转换，以确保最大的可移植性。
///
/// [`into`]: Into::into
#[unstable(feature = "raw_os_error_ty", issue = "107792")]
pub type RawOsError = sys::io::RawOsError;

// `#[repr(align(4))]` 多半是冗余的，它本来就应该已经有这个对齐值或更高。我们加上它仅仅是因为
// repr_bitpacked.rs 的编码要求对齐 >= 4（注意 `#[repr(align)]` 不会降低结构体所需的对齐，
// 只会提高它）。
//
// 如果我们给 ErrorData 添加更多变体，这个值可以提高到 8，但那样大概应该放在
// `#[cfg_attr(target_pointer_width = "64", ...)]`（或者我们用来启用 `repr_bitpacked` 代码的
// 任何 cfg）之后，因为只有那个版本需要该对齐，而 8 比我们在 32 位平台上将拥有的对齐更高。
//
// （为了说清楚：这里的对齐要求只有在使用 `error/repr_bitpacked.rs` 时才有意义——对于未压缩表示
// 它完全无关紧要）
#[doc(hidden)]
#[unstable(feature = "io_const_error_internals", issue = "none")]
#[repr(align(4))]
#[derive(Debug)]
pub struct SimpleMessage {
    pub kind: ErrorKind,
    pub message: &'static str,
}

/// 从一个已知的错误种类和一个字符串字面量创建一个新的 I/O 错误。
///
/// 与 [`Error::new`] 不同，这个宏不进行分配，并且可以在 `const` 上下文中使用。
///
/// # 示例
/// ```
/// #![feature(io_const_error)]
/// use std::io::{const_error, Error, ErrorKind};
///
/// const FAIL: Error = const_error!(ErrorKind::Unsupported, "tried something that never works");
///
/// fn not_here() -> Result<(), Error> {
///     Err(FAIL)
/// }
/// ```
#[rustc_macro_transparency = "semiopaque"]
#[unstable(feature = "io_const_error", issue = "133448")]
#[allow_internal_unstable(hint_must_use, io_const_error_internals)]
pub macro const_error($kind:expr, $message:expr $(,)?) {
    $crate::hint::must_use($crate::io::Error::from_static_message(
        const { &$crate::io::SimpleMessage { kind: $kind, message: $message } },
    ))
}

// 与 `SimpleMessage` 一样：这里的 `#[repr(align(4))]` 仅仅是因为 repr_bitpacked 的编码要求它。
// 实际上它几乎可以肯定本来就已经是这个对齐或更高了。
#[derive(Debug)]
#[repr(align(4))]
struct Custom {
    kind: ErrorKind,
    error: Box<dyn error::Error + Send + Sync>,
}

/// 列举 I/O 错误的若干大类的清单。
///
/// 这份清单预计会随着时间增长，因此不建议对它做穷尽式匹配。
///
/// 它与 [`io::Error`] 类型搭配使用。
///
/// [`io::Error`]: Error
///
/// # 处理错误与对 `ErrorKind` 进行匹配
///
/// 在应用代码中，对你预期会遇到的 `ErrorKind` 值使用 `match`；用 `_` 来匹配「所有其他错误」。
///
/// 在那些追求全面、彻底、想要验证测试不会返回任何已知的错误种类的测试中，你可能想把当前这份
/// 完整的错误清单从这里复制粘贴到你的测试代码里，然后把 `_` 当作正确的情况来匹配。这看起来
/// 有违直觉，但它会让你的测试更健壮。具体来说，如果你想验证你的代码确实会产生一个无法识别的
/// 错误种类，那么健壮的做法就是检查所有已识别的错误种类，并在命中这些情况时让测试失败。
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[stable(feature = "rust1", since = "1.0.0")]
#[cfg_attr(not(test), rustc_diagnostic_item = "io_errorkind")]
#[allow(deprecated)]
#[non_exhaustive]
pub enum ErrorKind {
    /// 未找到某个实体，通常是文件。
    #[stable(feature = "rust1", since = "1.0.0")]
    NotFound,
    /// 该操作缺少完成所需的必要权限。
    #[stable(feature = "rust1", since = "1.0.0")]
    PermissionDenied,
    /// 连接被远端服务器拒绝。
    #[stable(feature = "rust1", since = "1.0.0")]
    ConnectionRefused,
    /// 连接被远端服务器重置。
    #[stable(feature = "rust1", since = "1.0.0")]
    ConnectionReset,
    /// 无法到达远端主机。
    #[stable(feature = "io_error_a_bit_more", since = "1.83.0")]
    HostUnreachable,
    /// 无法到达包含远端主机的网络。
    #[stable(feature = "io_error_a_bit_more", since = "1.83.0")]
    NetworkUnreachable,
    /// 连接被远端服务器中止（终止）。
    #[stable(feature = "rust1", since = "1.0.0")]
    ConnectionAborted,
    /// 网络操作失败，因为尚未连接。
    #[stable(feature = "rust1", since = "1.0.0")]
    NotConnected,
    /// 无法绑定某个套接字地址，因为该地址已在别处被占用。
    #[stable(feature = "rust1", since = "1.0.0")]
    AddrInUse,
    /// 请求了一个不存在的网络接口，或者请求的地址不是本地地址。
    #[stable(feature = "rust1", since = "1.0.0")]
    AddrNotAvailable,
    /// 系统的网络已关闭。
    #[stable(feature = "io_error_a_bit_more", since = "1.83.0")]
    NetworkDown,
    /// 操作失败，因为管道已被关闭。
    #[stable(feature = "rust1", since = "1.0.0")]
    BrokenPipe,
    /// 某个实体已存在，通常是文件。
    #[stable(feature = "rust1", since = "1.0.0")]
    AlreadyExists,
    /// 操作需要阻塞才能完成，但调用方要求不要发生阻塞。
    #[stable(feature = "rust1", since = "1.0.0")]
    WouldBlock,
    /// 一个文件系统对象出乎意料地不是目录。
    ///
    /// 例如，指定了一个文件系统路径，而其中某个中间目录组件实际上是一个普通文件。
    #[stable(feature = "io_error_a_bit_more", since = "1.83.0")]
    NotADirectory,
    /// 该文件系统对象出乎意料地是一个目录。
    ///
    /// 在期望非目录的地方指定了一个目录。
    #[stable(feature = "io_error_a_bit_more", since = "1.83.0")]
    IsADirectory,
    /// 在期望空目录的地方指定了一个非空目录。
    #[stable(feature = "io_error_a_bit_more", since = "1.83.0")]
    DirectoryNotEmpty,
    /// 文件系统或存储介质是只读的，却尝试了一次写操作。
    #[stable(feature = "io_error_a_bit_more", since = "1.83.0")]
    ReadOnlyFilesystem,
    /// 文件系统或 IO 子系统中存在循环；常见情况是符号链接的层级过多。
    ///
    /// 在解析某个文件系统对象或文件 IO 对象时出现了循环（或过长的链）。
    ///
    /// 在 Unix 上，这通常是符号链接循环的结果；或者是超出了系统特定的符号链接遍历深度限制。
    #[unstable(feature = "io_error_more", issue = "86442")]
    FilesystemLoop,
    /// 失效的网络文件句柄。
    ///
    /// 在某些网络文件系统中（尤其是 NFS），一个已打开的文件（或目录）可能因为网络或服务器的问题
    /// 而失效。
    #[stable(feature = "io_error_a_bit_more", since = "1.83.0")]
    StaleNetworkFileHandle,
    /// 某个参数不正确。
    #[stable(feature = "rust1", since = "1.0.0")]
    InvalidInput,
    /// 遇到了对该操作而言无效的数据。
    ///
    /// 与 [`InvalidInput`] 不同，这通常意味着操作的参数本身是有效的，但错误是由格式不正确的
    /// 输入数据引起的。
    ///
    /// 例如，一个把文件读入字符串的函数，如果文件内容不是有效的 UTF-8，就会以 `InvalidData`
    /// 报错。
    ///
    /// [`InvalidInput`]: ErrorKind::InvalidInput
    #[stable(feature = "io_invalid_data", since = "1.2.0")]
    InvalidData,
    /// I/O 操作的超时已到期，导致它被取消。
    #[stable(feature = "rust1", since = "1.0.0")]
    TimedOut,
    /// 当某个操作因为对 [`write`] 的调用返回了 [`Ok(0)`] 而无法完成时返回的错误。
    ///
    /// 这通常意味着：某个操作只有在写入了特定数量的字节时才能成功，但实际只能写入更少的字节。
    ///
    /// [`write`]: crate::io::Write::write
    /// [`Ok(0)`]: Ok
    #[stable(feature = "rust1", since = "1.0.0")]
    WriteZero,
    /// 底层存储（通常是文件系统）已满。
    ///
    /// 这不包括超出配额的错误。
    #[stable(feature = "io_error_a_bit_more", since = "1.83.0")]
    StorageFull,
    /// 在不可寻位（seek）的文件上执行 seek。
    ///
    /// 在一个不适合寻位的已打开文件句柄上尝试了 seek——例如在 Unix 上，对用 `File::open` 打开的
    /// 命名管道执行 seek。
    #[stable(feature = "io_error_a_bit_more", since = "1.83.0")]
    NotSeekable,
    /// 超出了文件系统配额或某种其他类型的配额。
    #[stable(feature = "io_error_quota_exceeded", since = "1.85.0")]
    QuotaExceeded,
    /// 文件大于允许或支持的大小。
    ///
    /// 这可能源于底层文件系统或文件访问 API 的硬性限制，或者源于管理上施加的资源限制。简单的磁盘
    /// 已满和超出配额各自有专门的错误。
    #[stable(feature = "io_error_a_bit_more", since = "1.83.0")]
    FileTooLarge,
    /// 资源正忙。
    #[stable(feature = "io_error_a_bit_more", since = "1.83.0")]
    ResourceBusy,
    /// 可执行文件正忙。
    ///
    /// 尝试写入一个同时正作为运行中程序使用的文件。（并非所有操作系统都能检测到这种情况。）
    #[stable(feature = "io_error_a_bit_more", since = "1.83.0")]
    ExecutableFileBusy,
    /// 死锁（已避免）。
    ///
    /// 某个文件加锁操作会导致死锁。这种情况通常以尽力而为（best-effort）的方式被检测出来——如果
    /// 能被检测到的话。
    #[stable(feature = "io_error_a_bit_more", since = "1.83.0")]
    Deadlock,
    /// 跨设备或跨文件系统的（硬）链接或重命名。
    #[stable(feature = "io_error_crosses_devices", since = "1.85.0")]
    CrossesDevices,
    /// 指向同一个文件系统对象的（硬）链接过多。
    ///
    /// 文件系统不支持对同一个文件创建这么多硬链接。
    #[stable(feature = "io_error_a_bit_more", since = "1.83.0")]
    TooManyLinks,
    /// 某个文件名无效。
    ///
    /// 如果名字超出了长度限制，也可能发生这个错误。
    #[stable(feature = "io_error_invalid_filename", since = "1.87.0")]
    InvalidFilename,
    /// 程序参数列表过长。
    ///
    /// 在尝试运行外部程序时，参数大小将会超出系统或进程对其的限制。
    #[stable(feature = "io_error_a_bit_more", since = "1.83.0")]
    ArgumentListTooLong,
    /// 该操作被中断。
    ///
    /// 被中断的操作通常可以重试。
    #[stable(feature = "rust1", since = "1.0.0")]
    Interrupted,

    /// 该操作在本平台上不受支持。
    ///
    /// 这意味着该操作永远不可能成功。
    #[stable(feature = "unsupported_error", since = "1.53.0")]
    Unsupported,

    // 那些主要作为 OS 错误码归类的 ErrorKind 应当添加在上面。
    //
    /// 当某个操作因为过早到达「文件结尾」（end of file）而无法完成时返回的错误。
    ///
    /// 这通常意味着：某个操作只有在读取了特定数量的字节时才能成功，但实际只能读到更少的字节。
    #[stable(feature = "read_exact", since = "1.6.0")]
    UnexpectedEof,

    /// 某个操作无法完成，因为它未能分配到足够的内存。
    #[stable(feature = "out_of_memory_error", since = "1.54.0")]
    OutOfMemory,

    /// 由于非阻塞，该操作只部分成功，需要在稍后再行检查。
    #[unstable(feature = "io_error_inprogress", issue = "130840")]
    InProgress,

    // 那些不能简单地对应到（某组）OS 错误码的「非寻常」错误种类，应当添加在紧靠此注释的上方。
    // `Other` 和 `Uncategorized` 应当保持在末尾：
    //
    /// 一个不属于任何其他 I/O 错误种类的自定义错误。
    ///
    /// 这可用于构造你自己的、不匹配任何 [`ErrorKind`] 的 [`Error`]。
    ///
    /// 标准库不会使用这个 [`ErrorKind`]。
    ///
    /// 标准库中那些不属于任何 I/O 错误种类的错误无法被 `match`，只会匹配通配符（`_`）模式。
    /// 未来可能会为其中一些错误添加新的 [`ErrorKind`]。
    #[stable(feature = "rust1", since = "1.0.0")]
    Other,

    /// 标准库中任何不属于此清单的 I/O 错误。
    ///
    /// 现在归为 `Uncategorized` 的错误，将来可能移动到另一个或新的 [`ErrorKind`] 变体。不建议
    /// 把错误与 `Uncategorized` 进行匹配；请改用通配符匹配（`_`）。
    #[unstable(feature = "io_error_uncategorized", issue = "none")]
    #[doc(hidden)]
    Uncategorized,
}

impl ErrorKind {
    pub(crate) fn as_str(&self) -> &'static str {
        use ErrorKind::*;
        match *self {
            // tidy-alphabetical-start
            AddrInUse => "address in use",
            AddrNotAvailable => "address not available",
            AlreadyExists => "entity already exists",
            ArgumentListTooLong => "argument list too long",
            BrokenPipe => "broken pipe",
            ConnectionAborted => "connection aborted",
            ConnectionRefused => "connection refused",
            ConnectionReset => "connection reset",
            CrossesDevices => "cross-device link or rename",
            Deadlock => "deadlock",
            DirectoryNotEmpty => "directory not empty",
            ExecutableFileBusy => "executable file busy",
            FileTooLarge => "file too large",
            FilesystemLoop => "filesystem loop or indirection limit (e.g. symlink loop)",
            HostUnreachable => "host unreachable",
            InProgress => "in progress",
            Interrupted => "operation interrupted",
            InvalidData => "invalid data",
            InvalidFilename => "invalid filename",
            InvalidInput => "invalid input parameter",
            IsADirectory => "is a directory",
            NetworkDown => "network down",
            NetworkUnreachable => "network unreachable",
            NotADirectory => "not a directory",
            NotConnected => "not connected",
            NotFound => "entity not found",
            NotSeekable => "seek on unseekable file",
            Other => "other error",
            OutOfMemory => "out of memory",
            PermissionDenied => "permission denied",
            QuotaExceeded => "quota exceeded",
            ReadOnlyFilesystem => "read-only filesystem or storage medium",
            ResourceBusy => "resource busy",
            StaleNetworkFileHandle => "stale network file handle",
            StorageFull => "no storage space",
            TimedOut => "timed out",
            TooManyLinks => "too many links",
            Uncategorized => "uncategorized error",
            UnexpectedEof => "unexpected end of file",
            Unsupported => "unsupported",
            WouldBlock => "operation would block",
            WriteZero => "write zero",
            // tidy-alphabetical-end
        }
    }
}

#[stable(feature = "io_errorkind_display", since = "1.60.0")]
impl fmt::Display for ErrorKind {
    /// 展示对该 `ErrorKind` 的一段人类可读的描述。
    ///
    /// 这类似于 `impl Display for Error`，但不需要先转换成 Error。
    ///
    /// # 示例
    /// ```
    /// use std::io::ErrorKind;
    /// assert_eq!("entity not found", ErrorKind::NotFound.to_string());
    /// ```
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt.write_str(self.as_str())
    }
}

/// 供那些不暴露给用户的错误使用——对这些错误而言，在堆上分配（即通过 Error::new 进行常规构造）
/// 代价过高。
#[stable(feature = "io_error_from_errorkind", since = "1.14.0")]
impl From<ErrorKind> for Error {
    /// 把一个 [`ErrorKind`] 转换成 [`Error`]。
    ///
    /// 这个转换会创建一个新错误，其中带有错误种类的简单表示。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::io::{Error, ErrorKind};
    ///
    /// let not_found = ErrorKind::NotFound;
    /// let error = Error::from(not_found);
    /// assert_eq!("entity not found", format!("{error}"));
    /// ```
    #[inline]
    fn from(kind: ErrorKind) -> Error {
        Error { repr: Repr::new_simple(kind) }
    }
}

impl Error {
    /// 从一个已知的错误种类以及一个任意的错误载荷创建一个新的 I/O 错误。
    ///
    /// 这个函数用于泛型地创建那些并非源自 OS 本身的 I/O 错误。`error` 参数是一个任意载荷，
    /// 它将被包含在这个 [`Error`] 中。
    ///
    /// 注意，这个函数会在堆上分配内存。如果不需要额外载荷，请使用从 `ErrorKind` 而来的 `From`
    /// 转换。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::io::{Error, ErrorKind};
    ///
    /// // 错误可以从字符串创建
    /// let custom_error = Error::new(ErrorKind::Other, "oh no!");
    ///
    /// // 错误也可以从其他错误创建
    /// let custom_error2 = Error::new(ErrorKind::Interrupted, custom_error);
    ///
    /// // 创建一个不带载荷（也不进行内存分配）的错误
    /// let eof_error = Error::from(ErrorKind::UnexpectedEof);
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[cfg_attr(not(test), rustc_diagnostic_item = "io_error_new")]
    #[inline(never)]
    pub fn new<E>(kind: ErrorKind, error: E) -> Error
    where
        E: Into<Box<dyn error::Error + Send + Sync>>,
    {
        Self::_new(kind, error.into())
    }

    /// 从一个任意的错误载荷创建一个新的 I/O 错误。
    ///
    /// 这个函数用于泛型地创建那些并非源自 OS 本身的 I/O 错误。它是带 [`ErrorKind::Other`] 的
    /// [`Error::new`] 的一个快捷方式。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::io::Error;
    ///
    /// // 错误可以从字符串创建
    /// let custom_error = Error::other("oh no!");
    ///
    /// // 错误也可以从其他错误创建
    /// let custom_error2 = Error::other(custom_error);
    /// ```
    #[stable(feature = "io_error_other", since = "1.74.0")]
    pub fn other<E>(error: E) -> Error
    where
        E: Into<Box<dyn error::Error + Send + Sync>>,
    {
        Self::_new(ErrorKind::Other, error.into())
    }

    fn _new(kind: ErrorKind, error: Box<dyn error::Error + Send + Sync>) -> Error {
        Error { repr: Repr::new_custom(Box::new(Custom { kind, error })) }
    }

    /// 从一个已知的错误种类以及一条常量消息创建一个新的 I/O 错误。
    ///
    /// 这个函数不进行分配。
    ///
    /// 你不应直接使用它，而应使用 `const_error!` 宏：
    /// `io::const_error!(ErrorKind::Something, "some_message")`。
    ///
    /// 将来当 const 泛型允许时，这个函数也许应当改为
    /// `from_static_message<const MSG: &'static str>(kind: ErrorKind)`。
    #[inline]
    #[doc(hidden)]
    #[unstable(feature = "io_const_error_internals", issue = "none")]
    pub const fn from_static_message(msg: &'static SimpleMessage) -> Error {
        Self { repr: Repr::new_simple_message(msg) }
    }

    /// 返回一个表示最近发生的 OS 错误的错误。
    ///
    /// 这个函数会读取目标平台上 `errno` 的值（例如 Windows 上的 `GetLastError`），并为该错误码
    /// 返回一个相应的 [`Error`] 实例。
    ///
    /// 它应当在调用某个平台函数之后立即被调用，否则错误值的状态是不确定的。具体来说，其他标准库
    /// 函数可能会调用一些平台函数，这些平台函数即便成功也可能（也可能不会）重置错误值。
    ///
    /// # 示例
    ///
    /// ```
    /// use std::io::Error;
    ///
    /// let os_error = Error::last_os_error();
    /// println!("last OS error: {os_error:?}");
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[doc(alias = "GetLastError")]
    #[doc(alias = "errno")]
    #[must_use]
    #[inline]
    pub fn last_os_error() -> Error {
        Error::from_raw_os_error(sys::io::errno())
    }

    /// 从一个特定的 OS 错误码创建一个新的 [`Error`] 实例。
    ///
    /// # 示例
    ///
    /// 在 Linux 上：
    ///
    /// ```
    /// # if cfg!(target_os = "linux") {
    /// use std::io;
    ///
    /// let error = io::Error::from_raw_os_error(22);
    /// assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    /// # }
    /// ```
    ///
    /// 在 Windows 上：
    ///
    /// ```
    /// # if cfg!(windows) {
    /// use std::io;
    ///
    /// let error = io::Error::from_raw_os_error(10022);
    /// assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    /// # }
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[must_use]
    #[inline]
    pub fn from_raw_os_error(code: RawOsError) -> Error {
        Error { repr: Repr::new_os(code) }
    }

    /// 返回这个错误所表示的 OS 错误（如果有的话）。
    ///
    /// 如果这个 [`Error`] 是通过 [`last_os_error`] 或 [`from_raw_os_error`] 构造的，那么这个函数
    /// 将返回 [`Some`]，否则将返回 [`None`]。
    ///
    /// [`last_os_error`]: Error::last_os_error
    /// [`from_raw_os_error`]: Error::from_raw_os_error
    ///
    /// # 示例
    ///
    /// ```
    /// use std::io::{Error, ErrorKind};
    ///
    /// fn print_os_error(err: &Error) {
    ///     if let Some(raw_os_err) = err.raw_os_error() {
    ///         println!("raw OS error: {raw_os_err:?}");
    ///     } else {
    ///         println!("Not an OS error");
    ///     }
    /// }
    ///
    /// fn main() {
    ///     // 将打印 "raw OS error: ..."。
    ///     print_os_error(&Error::last_os_error());
    ///     // 将打印 "Not an OS error"。
    ///     print_os_error(&Error::new(ErrorKind::Other, "oh no!"));
    /// }
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[must_use]
    #[inline]
    pub fn raw_os_error(&self) -> Option<RawOsError> {
        match self.repr.data() {
            ErrorData::Os(i) => Some(i),
            ErrorData::Custom(..) => None,
            ErrorData::Simple(..) => None,
            ErrorData::SimpleMessage(..) => None,
        }
    }

    /// 返回对这个错误所包装的内层错误的引用（如果有的话）。
    ///
    /// 如果这个 [`Error`] 是通过 [`new`] 构造的，那么这个函数将返回 [`Some`]，否则将返回 [`None`]。
    ///
    /// [`new`]: Error::new
    ///
    /// # 示例
    ///
    /// ```
    /// use std::io::{Error, ErrorKind};
    ///
    /// fn print_error(err: &Error) {
    ///     if let Some(inner_err) = err.get_ref() {
    ///         println!("Inner error: {inner_err:?}");
    ///     } else {
    ///         println!("No inner error");
    ///     }
    /// }
    ///
    /// fn main() {
    ///     // 将打印 "No inner error"。
    ///     print_error(&Error::last_os_error());
    ///     // 将打印 "Inner error: ..."。
    ///     print_error(&Error::new(ErrorKind::Other, "oh no!"));
    /// }
    /// ```
    #[stable(feature = "io_error_inner", since = "1.3.0")]
    #[must_use]
    #[inline]
    pub fn get_ref(&self) -> Option<&(dyn error::Error + Send + Sync + 'static)> {
        match self.repr.data() {
            ErrorData::Os(..) => None,
            ErrorData::Simple(..) => None,
            ErrorData::SimpleMessage(..) => None,
            ErrorData::Custom(c) => Some(&*c.error),
        }
    }

    /// 返回对这个错误所包装的内层错误的可变引用（如果有的话）。
    ///
    /// 如果这个 [`Error`] 是通过 [`new`] 构造的，那么这个函数将返回 [`Some`]，否则将返回 [`None`]。
    ///
    /// [`new`]: Error::new
    ///
    /// # 示例
    ///
    /// ```
    /// use std::io::{Error, ErrorKind};
    /// use std::{error, fmt};
    /// use std::fmt::Display;
    ///
    /// #[derive(Debug)]
    /// struct MyError {
    ///     v: String,
    /// }
    ///
    /// impl MyError {
    ///     fn new() -> MyError {
    ///         MyError {
    ///             v: "oh no!".to_string()
    ///         }
    ///     }
    ///
    ///     fn change_message(&mut self, new_message: &str) {
    ///         self.v = new_message.to_string();
    ///     }
    /// }
    ///
    /// impl error::Error for MyError {}
    ///
    /// impl Display for MyError {
    ///     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    ///         write!(f, "MyError: {}", self.v)
    ///     }
    /// }
    ///
    /// fn change_error(mut err: Error) -> Error {
    ///     if let Some(inner_err) = err.get_mut() {
    ///         inner_err.downcast_mut::<MyError>().unwrap().change_message("I've been changed!");
    ///     }
    ///     err
    /// }
    ///
    /// fn print_error(err: &Error) {
    ///     if let Some(inner_err) = err.get_ref() {
    ///         println!("Inner error: {inner_err}");
    ///     } else {
    ///         println!("No inner error");
    ///     }
    /// }
    ///
    /// fn main() {
    ///     // 将打印 "No inner error"。
    ///     print_error(&change_error(Error::last_os_error()));
    ///     // 将打印 "Inner error: ..."。
    ///     print_error(&change_error(Error::new(ErrorKind::Other, MyError::new())));
    /// }
    /// ```
    #[stable(feature = "io_error_inner", since = "1.3.0")]
    #[must_use]
    #[inline]
    pub fn get_mut(&mut self) -> Option<&mut (dyn error::Error + Send + Sync + 'static)> {
        match self.repr.data_mut() {
            ErrorData::Os(..) => None,
            ErrorData::Simple(..) => None,
            ErrorData::SimpleMessage(..) => None,
            ErrorData::Custom(c) => Some(&mut *c.error),
        }
    }

    /// 消耗这个 `Error`，返回其内层错误（如果有的话）。
    ///
    /// 如果这个 [`Error`] 是通过 [`new`] 或 [`other`] 构造的，那么这个函数将返回 [`Some`]，
    /// 否则将返回 [`None`]。
    ///
    /// [`new`]: Error::new
    /// [`other`]: Error::other
    ///
    /// # 示例
    ///
    /// ```
    /// use std::io::{Error, ErrorKind};
    ///
    /// fn print_error(err: Error) {
    ///     if let Some(inner_err) = err.into_inner() {
    ///         println!("Inner error: {inner_err}");
    ///     } else {
    ///         println!("No inner error");
    ///     }
    /// }
    ///
    /// fn main() {
    ///     // 将打印 "No inner error"。
    ///     print_error(Error::last_os_error());
    ///     // 将打印 "Inner error: ..."。
    ///     print_error(Error::new(ErrorKind::Other, "oh no!"));
    /// }
    /// ```
    #[stable(feature = "io_error_inner", since = "1.3.0")]
    #[must_use = "`self` will be dropped if the result is not used"]
    #[inline]
    pub fn into_inner(self) -> Option<Box<dyn error::Error + Send + Sync>> {
        match self.repr.into_data() {
            ErrorData::Os(..) => None,
            ErrorData::Simple(..) => None,
            ErrorData::SimpleMessage(..) => None,
            ErrorData::Custom(c) => Some(c.error),
        }
    }

    /// 尝试把自定义的、装箱的错误向下转型（downcast）为 `E`。
    ///
    /// 如果这个 [`Error`] 包含一个自定义的装箱错误，那么它会尝试对该装箱错误进行向下转型，
    /// 否则将返回 [`Err`]。
    ///
    /// 如果该自定义装箱错误的类型与 `E` 相同，它将返回 [`Ok`]，否则同样会返回 [`Err`]。
    ///
    /// 这个方法旨在作为一个便捷例程，用于对由 [`Error::into_inner`] 返回的自定义装箱错误调用
    /// `Box<dyn Error + Sync + Send>::downcast`。
    ///
    ///
    /// # 示例
    ///
    /// ```
    /// use std::fmt;
    /// use std::io;
    /// use std::error::Error;
    ///
    /// #[derive(Debug)]
    /// enum E {
    ///     Io(io::Error),
    ///     SomeOtherVariant,
    /// }
    ///
    /// impl fmt::Display for E {
    ///    // ...
    /// #    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    /// #        todo!()
    /// #    }
    /// }
    /// impl Error for E {}
    ///
    /// impl From<io::Error> for E {
    ///     fn from(err: io::Error) -> E {
    ///         err.downcast::<E>()
    ///             .unwrap_or_else(E::Io)
    ///     }
    /// }
    ///
    /// impl From<E> for io::Error {
    ///     fn from(err: E) -> io::Error {
    ///         match err {
    ///             E::Io(io_error) => io_error,
    ///             e => io::Error::new(io::ErrorKind::Other, e),
    ///         }
    ///     }
    /// }
    ///
    /// # fn main() {
    /// let e = E::SomeOtherVariant;
    /// // 把它转换成一个 io::Error
    /// let io_error = io::Error::from(e);
    /// // 再把它转回原来的变体
    /// let e = E::from(io_error);
    /// assert!(matches!(e, E::SomeOtherVariant));
    ///
    /// let io_error = io::Error::from(io::ErrorKind::AlreadyExists);
    /// // 把它转换成 E
    /// let e = E::from(io_error);
    /// // 再把它转回原来的变体
    /// let io_error = io::Error::from(e);
    /// assert_eq!(io_error.kind(), io::ErrorKind::AlreadyExists);
    /// assert!(io_error.get_ref().is_none());
    /// assert!(io_error.raw_os_error().is_none());
    /// # }
    /// ```
    #[stable(feature = "io_error_downcast", since = "1.79.0")]
    pub fn downcast<E>(self) -> result::Result<E, Self>
    where
        E: error::Error + Send + Sync + 'static,
    {
        if let ErrorData::Custom(c) = self.repr.data()
            && c.error.is::<E>()
        {
            if let ErrorData::Custom(b) = self.repr.into_data()
                && let Ok(err) = b.error.downcast::<E>()
            {
                Ok(*err)
            } else {
                // 安全性：我们刚刚检查过该条件为真
                unsafe { crate::hint::unreachable_unchecked() }
            }
        } else {
            Err(self)
        }
    }

    /// 返回这个错误所对应的 [`ErrorKind`]。
    ///
    /// 这可能是由构造自定义 `io::Error` 的 Rust 代码所设置的值；或者，如果这个 `io::Error`
    /// 源自操作系统，它将是一个从系统的错误编码推断出来的值。详见 [`last_os_error`]。
    ///
    /// [`last_os_error`]: Error::last_os_error
    ///
    /// # 示例
    ///
    /// ```
    /// use std::io::{Error, ErrorKind};
    ///
    /// fn print_error(err: Error) {
    ///     println!("{:?}", err.kind());
    /// }
    ///
    /// fn main() {
    ///     // 由于（显式上）没有发生任何错误，这可能打印任何内容！
    ///     // 它很可能会打印一个表示「未识别的（非）错误」的占位值。
    ///     print_error(Error::last_os_error());
    ///     // 将打印 "AddrInUse"。
    ///     print_error(Error::new(ErrorKind::AddrInUse, "oh no!"));
    /// }
    /// ```
    #[stable(feature = "rust1", since = "1.0.0")]
    #[must_use]
    #[inline]
    pub fn kind(&self) -> ErrorKind {
        match self.repr.data() {
            ErrorData::Os(code) => sys::io::decode_error_kind(code),
            ErrorData::Custom(c) => c.kind,
            ErrorData::Simple(kind) => kind,
            ErrorData::SimpleMessage(m) => m.kind,
        }
    }

    #[inline]
    pub(crate) fn is_interrupted(&self) -> bool {
        match self.repr.data() {
            ErrorData::Os(code) => sys::io::is_interrupted(code),
            ErrorData::Custom(c) => c.kind == ErrorKind::Interrupted,
            ErrorData::Simple(kind) => kind == ErrorKind::Interrupted,
            ErrorData::SimpleMessage(m) => m.kind == ErrorKind::Interrupted,
        }
    }
}

impl fmt::Debug for Repr {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.data() {
            ErrorData::Os(code) => fmt
                .debug_struct("Os")
                .field("code", &code)
                .field("kind", &sys::io::decode_error_kind(code))
                .field("message", &sys::io::error_string(code))
                .finish(),
            ErrorData::Custom(c) => fmt::Debug::fmt(&c, fmt),
            ErrorData::Simple(kind) => fmt.debug_tuple("Kind").field(&kind).finish(),
            ErrorData::SimpleMessage(msg) => fmt
                .debug_struct("Error")
                .field("kind", &msg.kind)
                .field("message", &msg.message)
                .finish(),
        }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl fmt::Display for Error {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.repr.data() {
            ErrorData::Os(code) => {
                let detail = sys::io::error_string(code);
                write!(fmt, "{detail} (os error {code})")
            }
            ErrorData::Custom(ref c) => c.error.fmt(fmt),
            ErrorData::Simple(kind) => write!(fmt, "{}", kind.as_str()),
            ErrorData::SimpleMessage(msg) => msg.message.fmt(fmt),
        }
    }
}

#[stable(feature = "rust1", since = "1.0.0")]
impl error::Error for Error {
    #[allow(deprecated)]
    fn cause(&self) -> Option<&dyn error::Error> {
        match self.repr.data() {
            ErrorData::Os(..) => None,
            ErrorData::Simple(..) => None,
            ErrorData::SimpleMessage(..) => None,
            ErrorData::Custom(c) => c.error.cause(),
        }
    }

    fn source(&self) -> Option<&(dyn error::Error + 'static)> {
        match self.repr.data() {
            ErrorData::Os(..) => None,
            ErrorData::Simple(..) => None,
            ErrorData::SimpleMessage(..) => None,
            ErrorData::Custom(c) => c.error.source(),
        }
    }
}

fn _assert_error_is_sync_send() {
    fn _is_sync_send<T: Sync + Send>() {}
    _is_sync_send::<Error>();
}
