use crate::ffi::OsString;

/// 返回系统主机名（hostname）。
///
/// 在平台特定的错误场景下，此函数可能返回错误；
/// 例如在 uefi 与 wasm 上，这些平台并不支持
/// 主机名。
///
/// # 底层系统调用(Underlying system calls）
///
/// | 平台     | 系统调用                                                                                                |
/// |----------|---------------------------------------------------------------------------------------------------------|
/// | UNIX     | [`gethostname`](https://www.man7.org/linux/man-pages/man2/gethostname.2.html)                           |
/// | Windows  | [`GetHostNameW`](https://learn.microsoft.com/en-us/windows/win32/api/winsock2/nf-winsock2-gethostnamew) |
///
/// 注意，平台特定的行为 [将来可能发生变化][changes]。
///
/// [changes]: crate::io#platform-specific-behavior
#[unstable(feature = "gethostname", issue = "135142")]
pub fn hostname() -> crate::io::Result<OsString> {
    crate::sys::net::hostname()
}
