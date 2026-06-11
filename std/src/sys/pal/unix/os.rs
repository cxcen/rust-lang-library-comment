//! 在 unix 系统上对 `std::os` 功能的实现

#![allow(unused_imports)] // 这里有大量 cfg 代码

#[cfg(test)]
mod tests;

use libc::{c_char, c_int, c_void};

use crate::ffi::{CStr, OsStr, OsString};
use crate::os::unix::prelude::*;
use crate::path::{self, PathBuf};
use crate::sys::cvt;
use crate::sys::helpers::run_path_with_cstr;
use crate::{fmt, io, iter, mem, ptr, slice, str};

const PATH_SEPARATOR: u8 = b':';

#[cfg(target_os = "espidf")]
pub fn getcwd() -> io::Result<PathBuf> {
    Ok(PathBuf::from("/"))
}

#[cfg(not(target_os = "espidf"))]
pub fn getcwd() -> io::Result<PathBuf> {
    let mut buf = Vec::with_capacity(512);
    loop {
        unsafe {
            let ptr = buf.as_mut_ptr() as *mut libc::c_char;
            if !libc::getcwd(ptr, buf.capacity()).is_null() {
                let len = CStr::from_ptr(buf.as_ptr() as *const libc::c_char).to_bytes().len();
                buf.set_len(len);
                buf.shrink_to_fit();
                return Ok(PathBuf::from(OsString::from_vec(buf)));
            } else {
                let error = io::Error::last_os_error();
                if error.raw_os_error() != Some(libc::ERANGE) {
                    return Err(error);
                }
            }

            // 通过要求比当前容量更大的空间，触发 `Vec` 内部的缓冲区扩容逻辑。
            let cap = buf.capacity();
            buf.set_len(cap);
            buf.reserve(1);
        }
    }
}

#[cfg(target_os = "espidf")]
pub fn chdir(_p: &path::Path) -> io::Result<()> {
    super::unsupported::unsupported()
}

#[cfg(not(target_os = "espidf"))]
pub fn chdir(p: &path::Path) -> io::Result<()> {
    let result = run_path_with_cstr(p, &|p| unsafe { Ok(libc::chdir(p.as_ptr())) })?;
    if result == 0 { Ok(()) } else { Err(io::Error::last_os_error()) }
}

// 这里不能直接写成 `impl Iterator`，因为那要求 `'a` 在 drop 时仍然存活
//（见 #146045）。
pub type SplitPaths<'a> = iter::Map<
    slice::Split<'a, u8, impl FnMut(&u8) -> bool + 'static>,
    impl FnMut(&[u8]) -> PathBuf + 'static,
>;

#[define_opaque(SplitPaths)]
pub fn split_paths(unparsed: &OsStr) -> SplitPaths<'_> {
    fn is_separator(&b: &u8) -> bool {
        b == PATH_SEPARATOR
    }

    fn into_pathbuf(part: &[u8]) -> PathBuf {
        PathBuf::from(OsStr::from_bytes(part))
    }

    unparsed.as_bytes().split(is_separator).map(into_pathbuf)
}

#[derive(Debug)]
pub struct JoinPathsError;

pub fn join_paths<I, T>(paths: I) -> Result<OsString, JoinPathsError>
where
    I: Iterator<Item = T>,
    T: AsRef<OsStr>,
{
    let mut joined = Vec::new();

    for (i, path) in paths.enumerate() {
        let path = path.as_ref().as_bytes();
        if i > 0 {
            joined.push(PATH_SEPARATOR)
        }
        if path.contains(&PATH_SEPARATOR) {
            return Err(JoinPathsError);
        }
        joined.extend_from_slice(path);
    }
    Ok(OsStringExt::from_vec(joined))
}

impl fmt::Display for JoinPathsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "path segment contains separator `{}`", char::from(PATH_SEPARATOR))
    }
}

impl crate::error::Error for JoinPathsError {}

#[cfg(target_os = "aix")]
pub fn current_exe() -> io::Result<PathBuf> {
    #[cfg(test)]
    use realstd::env;

    #[cfg(not(test))]
    use crate::env;
    use crate::io;

    let exe_path = env::args().next().ok_or(io::const_error!(
        io::ErrorKind::NotFound,
        "an executable path was not found because no arguments were provided through argv",
    ))?;
    let path = PathBuf::from(exe_path);
    if path.is_absolute() {
        return path.canonicalize();
    }
    // 搜索 PWD 以推断 current_exe。
    if let Some(pstr) = path.to_str()
        && pstr.contains("/")
    {
        return getcwd().map(|cwd| cwd.join(path))?.canonicalize();
    }
    // 搜索 PATH 以推断 current_exe。
    if let Some(p) = env::var_os(OsStr::from_bytes("PATH".as_bytes())) {
        for search_path in split_paths(&p) {
            let pb = search_path.join(&path);
            if pb.is_file()
                && let Ok(metadata) = crate::fs::metadata(&pb)
                && metadata.permissions().mode() & 0o111 != 0
            {
                return pb.canonicalize();
            }
        }
    }
    Err(io::const_error!(io::ErrorKind::NotFound, "an executable path was not found"))
}

#[cfg(any(target_os = "freebsd", target_os = "dragonfly"))]
pub fn current_exe() -> io::Result<PathBuf> {
    unsafe {
        let mut mib = [
            libc::CTL_KERN as c_int,
            libc::KERN_PROC as c_int,
            libc::KERN_PROC_PATHNAME as c_int,
            -1 as c_int,
        ];
        let mut sz = 0;
        cvt(libc::sysctl(
            mib.as_mut_ptr(),
            mib.len() as libc::c_uint,
            ptr::null_mut(),
            &mut sz,
            ptr::null_mut(),
            0,
        ))?;
        if sz == 0 {
            return Err(io::Error::last_os_error());
        }
        let mut v: Vec<u8> = Vec::with_capacity(sz);
        cvt(libc::sysctl(
            mib.as_mut_ptr(),
            mib.len() as libc::c_uint,
            v.as_mut_ptr() as *mut libc::c_void,
            &mut sz,
            ptr::null_mut(),
            0,
        ))?;
        if sz == 0 {
            return Err(io::Error::last_os_error());
        }
        v.set_len(sz - 1); // 砍掉末尾的 NUL
        Ok(PathBuf::from(OsString::from_vec(v)))
    }
}

#[cfg(target_os = "netbsd")]
pub fn current_exe() -> io::Result<PathBuf> {
    fn sysctl() -> io::Result<PathBuf> {
        unsafe {
            let mib = [libc::CTL_KERN, libc::KERN_PROC_ARGS, -1, libc::KERN_PROC_PATHNAME];
            let mut path_len: usize = 0;
            cvt(libc::sysctl(
                mib.as_ptr(),
                mib.len() as libc::c_uint,
                ptr::null_mut(),
                &mut path_len,
                ptr::null(),
                0,
            ))?;
            if path_len <= 1 {
                return Err(io::const_error!(
                    io::ErrorKind::Uncategorized,
                    "KERN_PROC_PATHNAME sysctl returned zero-length string",
                ));
            }
            let mut path: Vec<u8> = Vec::with_capacity(path_len);
            cvt(libc::sysctl(
                mib.as_ptr(),
                mib.len() as libc::c_uint,
                path.as_ptr() as *mut libc::c_void,
                &mut path_len,
                ptr::null(),
                0,
            ))?;
            path.set_len(path_len - 1); // 砍掉 NUL
            Ok(PathBuf::from(OsString::from_vec(path)))
        }
    }
    fn procfs() -> io::Result<PathBuf> {
        let curproc_exe = path::Path::new("/proc/curproc/exe");
        if curproc_exe.is_file() {
            return crate::fs::read_link(curproc_exe);
        }
        Err(io::const_error!(
            io::ErrorKind::Uncategorized,
            "/proc/curproc/exe doesn't point to regular file.",
        ))
    }
    sysctl().or_else(|_| procfs())
}

#[cfg(target_os = "openbsd")]
pub fn current_exe() -> io::Result<PathBuf> {
    unsafe {
        let mut mib = [libc::CTL_KERN, libc::KERN_PROC_ARGS, libc::getpid(), libc::KERN_PROC_ARGV];
        let mib = mib.as_mut_ptr();
        let mut argv_len = 0;
        cvt(libc::sysctl(mib, 4, ptr::null_mut(), &mut argv_len, ptr::null_mut(), 0))?;
        let mut argv = Vec::<*const libc::c_char>::with_capacity(argv_len as usize);
        cvt(libc::sysctl(mib, 4, argv.as_mut_ptr() as *mut _, &mut argv_len, ptr::null_mut(), 0))?;
        argv.set_len(argv_len as usize);
        if argv[0].is_null() {
            return Err(io::const_error!(io::ErrorKind::Uncategorized, "no current exe available"));
        }
        let argv0 = CStr::from_ptr(argv[0]).to_bytes();
        if argv0[0] == b'.' || argv0.iter().any(|b| *b == b'/') {
            crate::fs::canonicalize(OsStr::from_bytes(argv0))
        } else {
            Ok(PathBuf::from(OsStr::from_bytes(argv0)))
        }
    }
}

#[cfg(any(
    target_os = "linux",
    target_os = "cygwin",
    target_os = "hurd",
    target_os = "android",
    target_os = "nuttx",
    target_os = "emscripten"
))]
pub fn current_exe() -> io::Result<PathBuf> {
    match crate::fs::read_link("/proc/self/exe") {
        Err(ref e) if e.kind() == io::ErrorKind::NotFound => Err(io::const_error!(
            io::ErrorKind::Uncategorized,
            "no /proc/self/exe available. Is /proc mounted?",
        )),
        other => other,
    }
}

#[cfg(target_os = "nto")]
pub fn current_exe() -> io::Result<PathBuf> {
    let mut e = crate::fs::read("/proc/self/exefile")?;
    // 当前版本的 QNX Neutrino 会提供一个以 null 结尾的路径。
    // 确保这里不返回末尾的 null 字节。
    if let Some(0) = e.last() {
        e.pop();
    }
    Ok(PathBuf::from(OsString::from_vec(e)))
}

#[cfg(target_vendor = "apple")]
pub fn current_exe() -> io::Result<PathBuf> {
    unsafe {
        let mut sz: u32 = 0;
        #[expect(deprecated)]
        libc::_NSGetExecutablePath(ptr::null_mut(), &mut sz);
        if sz == 0 {
            return Err(io::Error::last_os_error());
        }
        let mut v: Vec<u8> = Vec::with_capacity(sz as usize);
        #[expect(deprecated)]
        let err = libc::_NSGetExecutablePath(v.as_mut_ptr() as *mut i8, &mut sz);
        if err != 0 {
            return Err(io::Error::last_os_error());
        }
        v.set_len(sz as usize - 1); // 砍掉末尾的 NUL
        Ok(PathBuf::from(OsString::from_vec(v)))
    }
}

#[cfg(any(target_os = "solaris", target_os = "illumos"))]
pub fn current_exe() -> io::Result<PathBuf> {
    if let Ok(path) = crate::fs::read_link("/proc/self/path/a.out") {
        Ok(path)
    } else {
        unsafe {
            let path = libc::getexecname();
            if path.is_null() {
                Err(io::Error::last_os_error())
            } else {
                let filename = CStr::from_ptr(path).to_bytes();
                let path = PathBuf::from(<OsStr as OsStrExt>::from_bytes(filename));

                // 如果路径中不包含绝对路径名，
                // 就在路径前面拼接上当前工作目录。
                if filename[0] == b'/' { Ok(path) } else { getcwd().map(|cwd| cwd.join(path)) }
            }
        }
    }
}

#[cfg(target_os = "haiku")]
pub fn current_exe() -> io::Result<PathBuf> {
    let mut name = vec![0; libc::PATH_MAX as usize];
    unsafe {
        let result = libc::find_path(
            crate::ptr::null_mut(),
            libc::B_FIND_PATH_IMAGE_PATH,
            crate::ptr::null_mut(),
            name.as_mut_ptr(),
            name.len(),
        );
        if result != libc::B_OK {
            Err(io::const_error!(io::ErrorKind::Uncategorized, "error getting executable path"))
        } else {
            // find_path 会添加 null 结束符。
            let name = CStr::from_ptr(name.as_ptr()).to_bytes();
            Ok(PathBuf::from(OsStr::from_bytes(name)))
        }
    }
}

#[cfg(target_os = "redox")]
pub fn current_exe() -> io::Result<PathBuf> {
    crate::fs::read_to_string("/scheme/sys/exe").map(PathBuf::from)
}

#[cfg(target_os = "rtems")]
pub fn current_exe() -> io::Result<PathBuf> {
    crate::fs::read_to_string("sys:exe").map(PathBuf::from)
}

#[cfg(target_os = "l4re")]
pub fn current_exe() -> io::Result<PathBuf> {
    Err(io::const_error!(io::ErrorKind::Unsupported, "not yet implemented!"))
}

#[cfg(target_os = "vxworks")]
pub fn current_exe() -> io::Result<PathBuf> {
    #[cfg(test)]
    use realstd::env;

    #[cfg(not(test))]
    use crate::env;

    let exe_path = env::args().next().unwrap();
    let path = path::Path::new(&exe_path);
    path.canonicalize()
}

#[cfg(any(target_os = "espidf", target_os = "horizon", target_os = "vita"))]
pub fn current_exe() -> io::Result<PathBuf> {
    super::unsupported::unsupported()
}

#[cfg(target_os = "fuchsia")]
pub fn current_exe() -> io::Result<PathBuf> {
    #[cfg(test)]
    use realstd::env;

    #[cfg(not(test))]
    use crate::env;

    let exe_path = env::args().next().ok_or(io::const_error!(
        io::ErrorKind::Uncategorized,
        "an executable path was not found because no arguments were provided through argv",
    ))?;
    let path = PathBuf::from(exe_path);

    // 如果路径不是绝对路径，就在前面拼接上当前工作目录。
    if !path.is_absolute() { getcwd().map(|cwd| cwd.join(path)) } else { Ok(path) }
}

#[cfg(not(target_os = "espidf"))]
pub fn page_size() -> usize {
    unsafe { libc::sysconf(libc::_SC_PAGESIZE) as usize }
}

// 返回 [`confstr(key, ...)`][posix_confstr] 的值。目前仅在 Darwin 上使用，
// 但应当能在任何 unix 上工作（以备将来我们需要获取 `_CS_PATH` 或
// `_CS_V[67]_ENV`）。
//
// [posix_confstr]:
//     https://pubs.opengroup.org/onlinepubs/9699919799/functions/confstr.html
//
// FIXME: 在 Miri 中支持 `confstr`。
#[cfg(all(target_vendor = "apple", not(miri)))]
fn confstr(key: c_int, size_hint: Option<usize>) -> io::Result<OsString> {
    let mut buf: Vec<u8> = Vec::with_capacity(0);
    let mut bytes_needed_including_nul = size_hint
        .unwrap_or_else(|| {
            // 把 "None" 当作 "额外调用一次以获取长度"。理论上我们可以把这步移到
            // 下面的循环里，但鉴于尚不能 100% 确定当缓冲区非空时为 `len` 传 0 是否
            // 合法，这么做不太容易。
            unsafe { libc::confstr(key, core::ptr::null_mut(), 0) }
        })
        .max(1);
    // 如果 `confstr` 返回的值大于传给它的 len，说明该值被截断了，意味着我们需要重试。
    // 注意，虽然 `confstr` 的结果对一个进程而言似乎不会变化，但尚不清楚这是否在任何
    // 地方有保证，所以看来确实需要循环。
    while bytes_needed_including_nul > buf.capacity() {
        // 我们写入 `buf` 的空余容量中。这让我们得以避免改动 buf 的 `len`，既简化了
        // `reserve` 的计算，又能用 `Vec<u8>` 而非 `Vec<MaybeUninit<u8>>`，并且可能
        // 避免一次拷贝，因为 Vec 知道在重新分配时这些字节都不需要保留（嗯，至少理论上如此）。
        buf.reserve(bytes_needed_including_nul);
        // `confstr` 返回：
        // - 出错时返回 0：我们 break 并返回错误。
        // - 当且仅当提供的缓冲区足以容纳整个值时，返回写入的字节数：我们 break
        //   并返回 `buf` 中的数据。
        // - 否则，返回所需的字节数（包含 nul）：我们再走一遍循环。
        bytes_needed_including_nul =
            unsafe { libc::confstr(key, buf.as_mut_ptr().cast::<c_char>(), buf.capacity()) };
    }
    // `confstr` 出错时返回 0。
    if bytes_needed_including_nul == 0 {
        return Err(io::Error::last_os_error());
    }
    // Safety: `confstr(..., buf.as_mut_ptr(), buf.capacity())` 返回了非零值，
    // 意味着已初始化了 `bytes_needed_including_nul` 个字节。
    unsafe {
        buf.set_len(bytes_needed_including_nul);
        // 移除 NUL 结束符。
        let last_byte = buf.pop();
        // ……并冒烟检查（smoke-check）它确实_是_一个 NUL 结束符。
        assert_eq!(last_byte, Some(0), "`confstr` provided a string which wasn't nul-terminated");
    };
    Ok(OsString::from_vec(buf))
}

#[cfg(all(target_vendor = "apple", not(miri)))]
fn darwin_temp_dir() -> PathBuf {
    confstr(libc::_CS_DARWIN_USER_TEMP_DIR, Some(64)).map(PathBuf::from).unwrap_or_else(|_| {
        // 无论出于何种原因失败了（有好几种可能的原因），
        // 都返回全局的那个。
        PathBuf::from("/tmp")
    })
}

pub fn temp_dir() -> PathBuf {
    crate::env::var_os("TMPDIR").map(PathBuf::from).unwrap_or_else(|| {
        cfg_select! {
            all(target_vendor = "apple", not(miri)) => darwin_temp_dir(),
            target_os = "android" => PathBuf::from("/data/local/tmp"),
            _ => PathBuf::from("/tmp"),
        }
    })
}

pub fn home_dir() -> Option<PathBuf> {
    return crate::env::var_os("HOME")
        .filter(|s| !s.is_empty())
        .or_else(|| unsafe { fallback() })
        .map(PathBuf::from);

    #[cfg(any(
        target_os = "android",
        target_os = "emscripten",
        target_os = "redox",
        target_os = "vxworks",
        target_os = "espidf",
        target_os = "horizon",
        target_os = "vita",
        target_os = "nuttx",
        all(target_vendor = "apple", not(target_os = "macos")),
    ))]
    unsafe fn fallback() -> Option<OsString> {
        None
    }
    #[cfg(not(any(
        target_os = "android",
        target_os = "emscripten",
        target_os = "redox",
        target_os = "vxworks",
        target_os = "espidf",
        target_os = "horizon",
        target_os = "vita",
        target_os = "nuttx",
        all(target_vendor = "apple", not(target_os = "macos")),
    )))]
    unsafe fn fallback() -> Option<OsString> {
        let amt = match libc::sysconf(libc::_SC_GETPW_R_SIZE_MAX) {
            n if n < 0 => 512 as usize,
            n => n as usize,
        };
        let mut buf = Vec::with_capacity(amt);
        let mut p = mem::MaybeUninit::<libc::passwd>::uninit();
        let mut result = ptr::null_mut();
        match libc::getpwuid_r(
            libc::getuid(),
            p.as_mut_ptr(),
            buf.as_mut_ptr(),
            buf.capacity(),
            &mut result,
        ) {
            0 if !result.is_null() => {
                let ptr = (*result).pw_dir as *const _;
                let bytes = CStr::from_ptr(ptr).to_bytes().to_vec();
                Some(OsStringExt::from_vec(bytes))
            }
            _ => None,
        }
    }
}

pub fn exit(code: i32) -> ! {
    crate::sys::exit_guard::unique_thread_exit();
    unsafe { libc::exit(code as c_int) }
}

pub fn getpid() -> u32 {
    unsafe { libc::getpid() as u32 }
}

pub fn getppid() -> u32 {
    unsafe { libc::getppid() as u32 }
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
pub fn glibc_version() -> Option<(usize, usize)> {
    unsafe extern "C" {
        fn gnu_get_libc_version() -> *const libc::c_char;
    }
    let version_cstr = unsafe { CStr::from_ptr(gnu_get_libc_version()) };
    if let Ok(version_str) = version_cstr.to_str() {
        parse_glibc_version(version_str)
    } else {
        None
    }
}

// 如果字符串是有效的 "x.y" 版本则返回 Some((major, minor))，
// 忽略任何额外的以点分隔的部分。否则返回 None。
#[cfg(all(target_os = "linux", target_env = "gnu"))]
fn parse_glibc_version(version: &str) -> Option<(usize, usize)> {
    let mut parsed_ints = version.split('.').map(str::parse::<usize>).fuse();
    match (parsed_ints.next(), parsed_ints.next()) {
        (Some(Ok(major)), Some(Ok(minor))) => Some((major, minor)),
        _ => None,
    }
}
