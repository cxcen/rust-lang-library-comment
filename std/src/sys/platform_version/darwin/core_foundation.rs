//! 用于与动态加载的 CoreFoundation 交互的最小化工具集。
#![allow(non_snake_case, non_upper_case_globals)]
use super::root_relative;
use crate::ffi::{CStr, c_char, c_void};
use crate::ptr::null_mut;
use crate::sys::helpers::run_path_with_cstr;

// MacTypes.h
pub(super) type Boolean = u8;
// CoreFoundation/CFBase.h
pub(super) type CFTypeID = usize;
pub(super) type CFOptionFlags = usize;
pub(super) type CFIndex = isize;
pub(super) type CFTypeRef = *mut c_void;
pub(super) type CFAllocatorRef = CFTypeRef;
pub(super) const kCFAllocatorDefault: CFAllocatorRef = null_mut();
// CoreFoundation/CFError.h
pub(super) type CFErrorRef = CFTypeRef;
// CoreFoundation/CFData.h
pub(super) type CFDataRef = CFTypeRef;
// CoreFoundation/CFPropertyList.h
pub(super) const kCFPropertyListImmutable: CFOptionFlags = 0;
pub(super) type CFPropertyListFormat = CFIndex;
pub(super) type CFPropertyListRef = CFTypeRef;
// CoreFoundation/CFString.h
pub(super) type CFStringRef = CFTypeRef;
pub(super) type CFStringEncoding = u32;
pub(super) const kCFStringEncodingUTF8: CFStringEncoding = 0x08000100;
// CoreFoundation/CFDictionary.h
pub(super) type CFDictionaryRef = CFTypeRef;

/// 对动态加载的 CoreFoundation framework 的一个打开句柄。
///
/// 它通过 `dlopen` 打开，之后再 `dlclose`。这样做是为了尽量避免在用户没有自行
/// 链接 CoreFoundation 的情况下，把 CoreFoundation 的符号 "泄漏" 到用户二进制的
/// 其余部分。
///
/// 通过这个句柄直接查找符号，也比用 `RTLD_DEFAULT` 更快。
pub(super) struct CFHandle(*mut c_void);

macro_rules! dlsym_fn {
    (
        unsafe fn $name:ident($($param:ident: $param_ty:ty),* $(,)?) $(-> $ret:ty)?;
    ) => {
        pub(super) unsafe fn $name(&self, $($param: $param_ty),*) $(-> $ret)? {
            let ptr = unsafe {
                libc::dlsym(
                    self.0,
                    concat!(stringify!($name), '\0').as_bytes().as_ptr().cast(),
                )
            };
            if ptr.is_null() {
                let err = unsafe { CStr::from_ptr(libc::dlerror()) };
                panic!("could not find function {}: {err:?}", stringify!($name));
            }

            // SAFETY: 上面刚刚检查过该符号不为 NULL，并且宏的调用方保证签名是正确的。
            let fnptr = unsafe {
                crate::mem::transmute::<
                    *mut c_void,
                    unsafe extern "C" fn($($param_ty),*) $(-> $ret)?,
                >(ptr)
            };

            // SAFETY: 由调用方保证。
            unsafe { fnptr($($param),*) }
        }
    };
}

impl CFHandle {
    /// 链接到 CoreFoundation dylib，并从中查找符号。
    pub(super) fn new() -> Self {
        // 我们这里特意使用不带版本号的路径，以便在较旧的 iOS 设备上也能工作。
        let cf_path =
            root_relative("/System/Library/Frameworks/CoreFoundation.framework/CoreFoundation");

        let handle = run_path_with_cstr(&cf_path, &|path| unsafe {
            Ok(libc::dlopen(path.as_ptr(), libc::RTLD_LAZY | libc::RTLD_LOCAL))
        })
        .expect("failed allocating string");

        if handle.is_null() {
            let err = unsafe { CStr::from_ptr(libc::dlerror()) };
            panic!("could not open CoreFoundation.framework: {err:?}");
        }

        Self(handle)
    }

    pub(super) fn kCFAllocatorNull(&self) -> CFAllocatorRef {
        // 可用性：所有 CF 版本均可用。
        let static_ptr = unsafe { libc::dlsym(self.0, c"kCFAllocatorNull".as_ptr()) };
        if static_ptr.is_null() {
            let err = unsafe { CStr::from_ptr(libc::dlerror()) };
            panic!("could not find kCFAllocatorNull: {err:?}");
        }
        unsafe { *static_ptr.cast() }
    }

    // CoreFoundation/CFBase.h
    dlsym_fn!(
        // 可用性：所有 CF 版本均可用。
        unsafe fn CFRelease(cf: CFTypeRef);
    );
    dlsym_fn!(
        // 可用性：所有 CF 版本均可用。
        unsafe fn CFGetTypeID(cf: CFTypeRef) -> CFTypeID;
    );

    // CoreFoundation/CFData.h
    dlsym_fn!(
        // 可用性：所有 CF 版本均可用。
        unsafe fn CFDataCreateWithBytesNoCopy(
            allocator: CFAllocatorRef,
            bytes: *const u8,
            length: CFIndex,
            bytes_deallocator: CFAllocatorRef,
        ) -> CFDataRef;
    );

    // CoreFoundation/CFPropertyList.h
    dlsym_fn!(
        // 可用性：自 macOS 10.6 起可用。
        unsafe fn CFPropertyListCreateWithData(
            allocator: CFAllocatorRef,
            data: CFDataRef,
            options: CFOptionFlags,
            format: *mut CFPropertyListFormat,
            error: *mut CFErrorRef,
        ) -> CFPropertyListRef;
    );

    // CoreFoundation/CFString.h
    dlsym_fn!(
        // 可用性：所有 CF 版本均可用。
        unsafe fn CFStringGetTypeID() -> CFTypeID;
    );
    dlsym_fn!(
        // 可用性：所有 CF 版本均可用。
        unsafe fn CFStringCreateWithCStringNoCopy(
            alloc: CFAllocatorRef,
            c_str: *const c_char,
            encoding: CFStringEncoding,
            contents_deallocator: CFAllocatorRef,
        ) -> CFStringRef;
    );
    dlsym_fn!(
        // 可用性：所有 CF 版本均可用。
        unsafe fn CFStringGetCString(
            the_string: CFStringRef,
            buffer: *mut c_char,
            buffer_size: CFIndex,
            encoding: CFStringEncoding,
        ) -> Boolean;
    );

    // CoreFoundation/CFDictionary.h
    dlsym_fn!(
        // 可用性：所有 CF 版本均可用。
        unsafe fn CFDictionaryGetTypeID() -> CFTypeID;
    );
    dlsym_fn!(
        // 可用性：所有 CF 版本均可用。
        unsafe fn CFDictionaryGetValue(
            the_dict: CFDictionaryRef,
            key: *const c_void,
        ) -> *const c_void;
    );
}

impl Drop for CFHandle {
    fn drop(&mut self) {
        // 关闭时忽略错误。`libloading` 也是这么做的：
        // https://docs.rs/libloading/0.8.6/src/libloading/os/unix/mod.rs.html#374
        let _ = unsafe { libc::dlclose(self.0) };
    }
}
