use crate::ffi::OsString;
use crate::io;
use crate::os::unix::ffi::OsStringExt;
use crate::sys::io::errno;

pub fn hostname() -> io::Result<OsString> {
    // 向系统查询主机名的最大长度。
    let host_name_max = match unsafe { libc::sysconf(libc::_SC_HOST_NAME_MAX) } {
        // 如果查询失败（可能是因为没有最大长度限制），
        // 则假定最大长度为 _POSIX_HOST_NAME_MAX（255）。
        -1 => 255,
        max => max as usize,
    };

    // 也为 nul 终止符预留空间。
    let mut buf = Vec::<u8>::try_with_capacity(host_name_max + 1)?;
    loop {
        // SAFETY: `buf` 中有 `buf.capacity()` 个字节可写。
        let r = unsafe { libc::gethostname(buf.as_mut_ptr().cast(), buf.capacity()) };
        match (r != 0).then(errno) {
            None => {
                // 不幸的是，UNIX 规范规定：如果名称放不下缓冲区，名称将被
                // 截断，而不会返回错误。此外，被截断的名称仍可能是以 null
                // 结尾的，因此没有可靠的办法检测截断。
                // 幸运的是，大多数平台忽略了规范的规定，会返回一个错误
                //（通常是 ENAMETOOLONG）。万一并非如此，下面的代码会在 null
                // 终止符被省略时检测出截断。注意这一检查完全不会影响性能，
                // 因为无论如何我们都需要求出字符串的长度。
                //
                // 使用 `strnlen`，因为它不对 nul 终止符之后的字节施加初始化要求。
                //
                // SAFETY: `buf` 中有 `buf.capacity()` 个字节可访问，且这些字节
                // 已被初始化，直到（并包括）一个可能存在的 nul 终止符。
                let len = unsafe { libc::strnlen(buf.as_ptr().cast(), buf.capacity()) };
                if len < buf.capacity() {
                    // 如果字符串是以 nul 结尾的，我们便认为它没有被截断，
                    // 因为其容量*应当*足以容纳 `HOST_NAME_MAX` 个字节。
                    // SAFETY: 已初始化了 `len + 1` 个字节（我们将 nul 终止符
                    // 排除在字符串之外）。
                    unsafe { buf.set_len(len) };
                    return Ok(OsString::from_vec(buf));
                }
            }
            // 由于 `buf.capacity()` 始终小于或等于 `isize::MAX`（Rust 的分配
            // 不能超过该上限），唯一可能返回 `EINVAL` 的情况是：系统用
            // `EINVAL` 来报告名称放不下所提供的缓冲区。在那种情况下（或在
            // `ENAMETOOLONG` 的情况下），扩大缓冲区后重试。
            Some(libc::EINVAL | libc::ENAMETOOLONG) => {}
            // 其他错误码（例如 EPERM）与缓冲区大小无关，应当返回给用户。
            Some(err) => return Err(io::Error::from_raw_os_error(err)),
        }

        // 调整缓冲区大小（按照 `Vec` 的扩容规则）后重试。
        buf.try_reserve(buf.capacity() + 1)?;
    }
}
