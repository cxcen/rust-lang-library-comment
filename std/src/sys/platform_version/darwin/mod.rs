use self::core_foundation::{
    CFDictionaryRef, CFHandle, CFIndex, CFStringRef, CFTypeRef, kCFAllocatorDefault,
    kCFPropertyListImmutable, kCFStringEncodingUTF8,
};
use crate::borrow::Cow;
use crate::bstr::ByteStr;
use crate::ffi::{CStr, c_char};
use crate::num::{NonZero, ParseIntError};
use crate::path::{Path, PathBuf};
use crate::ptr::null_mut;
use crate::sync::atomic::{AtomicU32, Ordering};
use crate::{env, fs};

mod core_foundation;
mod public_extern;
#[cfg(test)]
mod tests;

/// 操作系统的版本。
///
/// 这里使用一个打包后的 u32，以便快速比较，并与 Mach-O 的 `LC_BUILD_VERSION` 保持一致。
type OSVersion = u32;

/// 将版本各部分组合成一个 [`OSVersion`]。
///
/// 各部分的大小本质上受限于 Mach-O 的 `LC_BUILD_VERSION`。
#[inline]
const fn pack_os_version(major: u16, minor: u8, patch: u8) -> OSVersion {
    let (major, minor, patch) = (major as u32, minor as u32, patch as u32);
    (major << 16) | (minor << 8) | patch
}

/// 同 [`pack_os_version`]，但接受 `i32` 并做饱和处理。
///
/// 而不是用诸如 `major as u16` 这样会发生截断的写法。
#[inline]
fn pack_i32_os_version(major: i32, minor: i32, patch: i32) -> OSVersion {
    let major: u16 = major.try_into().unwrap_or(u16::MAX);
    let minor: u8 = minor.try_into().unwrap_or(u8::MAX);
    let patch: u8 = patch.try_into().unwrap_or(u8::MAX);
    pack_os_version(major, minor, patch)
}

/// 获取当前 OS 版本，按 [`pack_os_version`] 的方式打包。
///
/// # 语义
///
/// 如果二进制的 SDK 版本低于 11.0，macOS 上报告的版本可能是 10.16。
/// 这是 Apple 为了处理那些假定 macOS 版本号永远以 "10" 开头的应用程序而实现的
/// 一个变通方案，参见：
/// <https://github.com/apple-oss-distributions/xnu/blob/xnu-11215.81.4/libsyscall/wrappers/system-version-compat.c>
///
/// 无论二进制的 SDK 版本如何，其实_是_有可能拿到真实版本的，Zig 就是这么做的：
/// <https://github.com/ziglang/zig/blob/0.13.0/lib/std/zig/system/darwin/macos.zig>
///
/// 我们选择不那样做，而是在这里遵循 Apple 的行为：用较旧的 SDK 编译时返回 10.16；
/// 用户应当转而升级自己的工具链。
///
/// NOTE: `rustc` 目前在用 ld64 链接时不会设置正确的 SDK 版本，因此在 x86_64 上配合
/// `-Clinker=ld` 会产生错误的行为。但那是一个 `rustc` 的 bug：
/// <https://github.com/rust-lang/rust/issues/129432>
#[inline]
fn current_version() -> OSVersion {
    // 出于性能考虑，缓存这次查询结果。
    //
    // 0.0.0 永远不会是一个有效版本（"vtool" 在 0 版本上会报告 "n/a"），所以我们用它
    // 作为哨兵值。
    static CURRENT_VERSION: AtomicU32 = AtomicU32::new(0);

    // 我们使用 relaxed 原子操作而非例如 `Once`，即便多个线程竞争地读写版本也没关系，
    // `lookup_version` 应当是幂等的，并且始终返回相同的值。
    //
    // `compiler-rt` 使用 `dispatch_once`，但鉴于上述原因那是杀鸡用牛刀。
    let version = CURRENT_VERSION.load(Ordering::Relaxed);
    if version == 0 {
        let version = lookup_version().get();
        CURRENT_VERSION.store(version, Ordering::Relaxed);
        version
    } else {
        version
    }
}

/// 查询 OS 版本。
///
/// # 中止(Aborts）
///
/// 如果读取或解析版本失败（或系统内存不足），则 abort。
///
/// 我们刻意选择 abort，因为如果让它静默地返回一个无效的 OS 版本，对用户来说将无从调试。
// 由于 `current_version` 中有缓存，这次查询代价较高，应当走 cold path（冷路径）。
#[cold]
// 微优化：我们使用 `extern "C"` 以在 panic 时 abort，从而让（被内联的）`current_version`
// 无需带有 unwind 处理逻辑。无论如何，`__isPlatformVersionAtLeast` 本就要求 abort。
extern "C" fn lookup_version() -> NonZero<OSVersion> {
    // 先尝试从 `sysctl` 读取（更快），如果失败，则回退到读取属性列表
    // （这大致就是 `_availability_version_check` 内部所做的事）。
    let version = version_from_sysctl().unwrap_or_else(version_from_plist);

    // 使用 `NonZero` 以尽量向优化器表明它永远不会返回 0。
    NonZero::new(version).expect("version cannot be 0.0.0")
}

/// 从 `kern.osproductversion` 或 `kern.iossupportversion` 读取版本。
///
/// 这比 `version_from_plist` 更快，因为它不需要调用 `dlsym`。
fn version_from_sysctl() -> Option<OSVersion> {
    // 这在模拟器中不起作用，因为 `kern.osproductversion` 返回的是宿主 macOS 的版本，
    // 而 `kern.iossupportversion` 返回的是宿主 macOS 的 iOSSupportVersion（而你可以
    // 用许多不同的 iOS 版本运行模拟器）。
    if cfg!(target_abi = "sim") {
        // 在这些目标上回退到 `version_from_plist`。
        return None;
    }

    let sysctl_version = |name: &CStr| {
        let mut buf: [u8; 32] = [0; 32];
        let mut size = buf.len();
        let ptr = buf.as_mut_ptr().cast();
        let ret = unsafe { libc::sysctlbyname(name.as_ptr(), ptr, &mut size, null_mut(), 0) };
        if ret != 0 {
            // 这个 sysctl 不可用。
            return None;
        }
        let buf = &buf[..(size - 1)];

        if buf.is_empty() {
            // 在真实的 iOS 设备上使用 `kern.iossupportversion` 时，或在 visionOS 上以
            // "Designed for iPad" 运行时，缓冲区可能为空。
            //
            // 这种情况下，回退到 `kern.osproductversion`。
            return None;
        }

        Some(parse_os_version(buf).unwrap_or_else(|err| {
            panic!("failed parsing version from sysctl ({}): {err}", ByteStr::new(buf))
        }))
    };

    // 当 `target_os = "ios"` 时，我们可能处于许多不同的状态：
    // - 原生 iOS 设备。
    // - iOS 模拟器。
    // - Mac Catalyst。
    // - Mac + "Designed for iPad"。
    // - 原生 visionOS 设备 + "Designed for iPad"。
    // - visionOS 模拟器 + "Designed for iPad"。
    //
    // 这些之中，只有原生、Mac Catalyst 和模拟器可以在编译期区分
    //（分别对应 `target_abi = ""`、`target_abi = "macabi"` 和 `target_abi = "sim"`）。
    //
    // 也就是说，"Designed for iPad" 在编译期会表现为 iOS，但 `ProductVersion`
    // 仍然会是宿主 macOS 或 visionOS 的版本。
    //
    // 更进一步，我们在运行期甚至也无法可靠地区分它们，因为
    // `dyld_get_active_platform` 并未公开。
    //
    // 幸运的是，我们并不需要知道这些；我们可以直接尝试获取
    // `iOSSupportVersion`（它在原生 iOS 上也可能被设置，但那时它会被设为宿主
    // iOS 版本），如果失败，再回退到 `ProductVersion`。
    if cfg!(target_os = "ios") {
        // https://github.com/apple-oss-distributions/xnu/blob/xnu-11215.81.4/bsd/kern/kern_sysctl.c#L2077-L2100
        if let Some(ios_support_version) = sysctl_version(c"kern.iossupportversion") {
            return Some(ios_support_version);
        }

        // 在 Mac Catalyst 上，如果查询 `iOSSupportVersion` 失败，我们不希望
        // 意外地回退到 `ProductVersion`。
        if cfg!(target_abi = "macabi") {
            return None;
        }
    }

    // 引入于 macOS 10.13.4。
    // https://github.com/apple-oss-distributions/xnu/blob/xnu-11215.81.4/bsd/kern/kern_sysctl.c#L2015-L2051
    sysctl_version(c"kern.osproductversion")
}

/// 从 `/System/Library/CoreServices/SystemVersion.plist` 查询当前的 OS 版本。
///
/// 更具体地说，是从 `ProductVersion` 和 `iOSSupportVersion` 键，以及在模拟器上从
/// `$IPHONE_SIMULATOR_ROOT/System/Library/CoreServices/SystemVersion.plist` 读取。
///
/// 该文件引入于 macOS 10.3，远低于 `rustc` 所支持的最低版本（截至撰写本文时为
/// macOS 10.12）。
///
/// # 实现
///
/// 我们这里所做的事情与 `compiler-rt` 大致相同，并动态查找 CoreFoundation 的工具
/// 来解析 PList（以避免在这里重新实现一遍，因为把一个完整的 PList 解析器引入 `std`
/// 似乎代价高昂）。
///
/// 如果发现这并不可取，我们_或许_可以通过手动解析 PList 来取巧（在所有版本里它似乎
/// 都使用纯文本的 "xml1" 编码/格式），但那看起来很脆弱。
fn version_from_plist() -> OSVersion {
    // 读取 `SystemVersion.plist`。在 Apple 平台上始终存在，读取它不会失败。
    let path = root_relative("/System/Library/CoreServices/SystemVersion.plist");
    let plist_buffer = fs::read(&path).unwrap_or_else(|e| panic!("failed reading {path:?}: {e}"));
    let cf_handle = CFHandle::new();
    parse_version_from_plist(&cf_handle, &plist_buffer)
}

/// 从给定的 PList 解析 OS 版本。
///
/// 从 [`version_from_plist`] 中拆分出来，以便测试。
fn parse_version_from_plist(cf_handle: &CFHandle, plist_buffer: &[u8]) -> OSVersion {
    let plist_data = unsafe {
        cf_handle.CFDataCreateWithBytesNoCopy(
            kCFAllocatorDefault,
            plist_buffer.as_ptr(),
            plist_buffer.len() as CFIndex,
            cf_handle.kCFAllocatorNull(),
        )
    };
    assert!(!plist_data.is_null(), "failed creating CFData");
    let _plist_data_release = Deferred(|| unsafe { cf_handle.CFRelease(plist_data) });

    let plist = unsafe {
        cf_handle.CFPropertyListCreateWithData(
            kCFAllocatorDefault,
            plist_data,
            kCFPropertyListImmutable,
            null_mut(), // 不关心 PList 的格式。
            null_mut(), // 不关心错误数据。
        )
    };
    assert!(!plist.is_null(), "failed reading PList in SystemVersion.plist");
    let _plist_release = Deferred(|| unsafe { cf_handle.CFRelease(plist) });

    assert_eq!(
        unsafe { cf_handle.CFGetTypeID(plist) },
        unsafe { cf_handle.CFDictionaryGetTypeID() },
        "SystemVersion.plist did not contain a dictionary at the top level"
    );
    let plist: CFDictionaryRef = plist.cast();

    // 与 `version_from_sysctl` 中的逻辑相同。
    if cfg!(target_os = "ios") {
        if let Some(ios_support_version) =
            unsafe { string_version_key(cf_handle, plist, c"iOSSupportVersion") }
        {
            return ios_support_version;
        }

        // 强制 Mac Catalyst 使用 iOSSupportVersion（不要回退到 ProductVersion）。
        if cfg!(target_abi = "macabi") {
            panic!("expected iOSSupportVersion in SystemVersion.plist");
        }
    }

    // 在所有其他平台上，我们只需查看 `ProductVersion` 即可得到 OS 版本。
    unsafe { string_version_key(cf_handle, plist, c"ProductVersion") }
        .expect("expected ProductVersion in SystemVersion.plist")
}

/// 在一个 CFDictionary 中查找某个字符串键，并将其转换为 [`OSVersion`]。
unsafe fn string_version_key(
    cf_handle: &CFHandle,
    plist: CFDictionaryRef,
    lookup_key: &CStr,
) -> Option<OSVersion> {
    let cf_lookup_key = unsafe {
        cf_handle.CFStringCreateWithCStringNoCopy(
            kCFAllocatorDefault,
            lookup_key.as_ptr(),
            kCFStringEncodingUTF8,
            cf_handle.kCFAllocatorNull(),
        )
    };
    assert!(!cf_lookup_key.is_null(), "failed creating CFString");
    let _lookup_key_release = Deferred(|| unsafe { cf_handle.CFRelease(cf_lookup_key) });

    let value: CFTypeRef =
        unsafe { cf_handle.CFDictionaryGetValue(plist, cf_lookup_key) }.cast_mut();
    // `CFDictionaryGetValue` 是一个 "getter"，所以我们不应释放它，
    // 该值由 CFDictionary 在内部保持存活，参见：
    // https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/MemoryMgmt/Articles/mmPractical.html#//apple_ref/doc/uid/TP40004447-SW12
    if value.is_null() {
        return None;
    }

    assert_eq!(
        unsafe { cf_handle.CFGetTypeID(value) },
        unsafe { cf_handle.CFStringGetTypeID() },
        "key in SystemVersion.plist must be a string"
    );
    let value: CFStringRef = value.cast();

    let mut version_str = [0u8; 32];
    let ret = unsafe {
        cf_handle.CFStringGetCString(
            value,
            version_str.as_mut_ptr().cast::<c_char>(),
            version_str.len() as CFIndex,
            kCFStringEncodingUTF8,
        )
    };
    assert_ne!(ret, 0, "failed getting string from CFString");

    let version_str =
        CStr::from_bytes_until_nul(&version_str).expect("failed converting CFString to CStr");

    Some(parse_os_version(version_str.to_bytes()).unwrap_or_else(|err| {
        panic!(
            "failed parsing version from PList ({}): {err}",
            ByteStr::new(version_str.to_bytes())
        )
    }))
}

/// 从形如 b"10.1" 或 b"14.3.7" 的字节串解析出 OS 版本。
fn parse_os_version(version: &[u8]) -> Result<OSVersion, ParseIntError> {
    if let Some((major, minor)) = version.split_once(|&b| b == b'.') {
        let major = u16::from_ascii(major)?;
        if let Some((minor, patch)) = minor.split_once(|&b| b == b'.') {
            let minor = u8::from_ascii(minor)?;
            let patch = u8::from_ascii(patch)?;
            Ok(pack_os_version(major, minor, patch))
        } else {
            let minor = u8::from_ascii(minor)?;
            Ok(pack_os_version(major, minor, 0))
        }
    } else {
        let major = u16::from_ascii(version)?;
        Ok(pack_os_version(major, 0, 0))
    }
}

/// 获取一个相对于根目录的路径，当前 env 的所有文件都位于该根目录下。
fn root_relative(path: &str) -> Cow<'_, Path> {
    if cfg!(target_abi = "sim") {
        let mut root = PathBuf::from(env::var_os("IPHONE_SIMULATOR_ROOT").expect(
            "environment variable `IPHONE_SIMULATOR_ROOT` must be set when executing under simulator",
        ));
        // 将绝对路径转换为相对路径，以使 `.push` 按预期工作。
        root.push(Path::new(path).strip_prefix("/").unwrap());
        root.into()
    } else {
        Path::new(path).into()
    }
}

struct Deferred<F: FnMut()>(F);

impl<F: FnMut()> Drop for Deferred<F> {
    fn drop(&mut self) {
        (self.0)();
    }
}
