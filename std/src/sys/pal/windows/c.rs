//! libnative 所使用、但不适合放在 liblibc 中的 C 定义

#![allow(nonstandard_style)]
#![cfg_attr(test, allow(dead_code))]
#![unstable(issue = "none", feature = "windows_c")]
#![allow(clippy::style)]

use core::ffi::{CStr, c_uint, c_ulong, c_ushort, c_void};
use core::ptr;

mod windows_sys;
pub use windows_sys::*;

pub type WCHAR = u16;

pub const INVALID_HANDLE_VALUE: HANDLE = ::core::ptr::without_provenance_mut(-1i32 as _);

// https://learn.microsoft.com/en-us/cpp/c-runtime-library/exit-success-exit-failure?view=msvc-170
pub const EXIT_SUCCESS: u32 = 0;
pub const EXIT_FAILURE: u32 = 1;

#[cfg(target_vendor = "win7")]
pub const CONDITION_VARIABLE_INIT: CONDITION_VARIABLE = CONDITION_VARIABLE { Ptr: ptr::null_mut() };
#[cfg(target_vendor = "win7")]
pub const SRWLOCK_INIT: SRWLOCK = SRWLOCK { Ptr: ptr::null_mut() };
#[cfg(not(target_thread_local))]
pub const INIT_ONCE_STATIC_INIT: INIT_ONCE = INIT_ONCE { Ptr: ptr::null_mut() };

// 某些 windows_sys 类型的符号性（sign）与我们使用的类型不同。
pub const OBJ_DONT_REPARSE: u32 = windows_sys::OBJ_DONT_REPARSE as u32;
pub const FRS_ERR_SYSVOL_POPULATE_TIMEOUT: u32 =
    windows_sys::FRS_ERR_SYSVOL_POPULATE_TIMEOUT as u32;

// 等价于 C 预处理器宏 `NT_SUCCESS`。
// 参见：https://docs.microsoft.com/en-us/windows-hardware/drivers/kernel/using-ntstatus-values
pub fn nt_success(status: NTSTATUS) -> bool {
    status >= 0
}

impl OBJECT_ATTRIBUTES {
    pub fn with_length() -> Self {
        Self {
            Length: size_of::<Self>() as _,
            RootDirectory: ptr::null_mut(),
            ObjectName: ptr::null_mut(),
            Attributes: 0,
            SecurityDescriptor: ptr::null_mut(),
            SecurityQualityOfService: ptr::null_mut(),
        }
    }
}

impl IO_STATUS_BLOCK {
    pub const PENDING: Self =
        IO_STATUS_BLOCK { Anonymous: IO_STATUS_BLOCK_0 { Status: STATUS_PENDING }, Information: 0 };
    pub fn status(&self) -> NTSTATUS {
        // SAFETY: 如果设置的是 `self.Anonymous.Status`，那么这显然是安全的。
        // 如果设置的是 `self.Anonymous.Pointer`，那么这等价于把指针转换为整数，同样是安全的。
        // 目前在本模块之外构造 `IO_STATUS_BLOCK` 的唯一安全方式是调用 `default`
        // 方法，而该方法会设置 `Status` 字段。
        unsafe { self.Anonymous.Status }
    }
}

/// NB: 谨慎使用！通常把它当作引用来使用很可能会导致 `rest` 字段的
/// provenance（来源信息）出错！
#[repr(C)]
pub struct REPARSE_DATA_BUFFER {
    pub ReparseTag: c_uint,
    pub ReparseDataLength: c_ushort,
    pub Reserved: c_ushort,
    pub rest: (),
}

/// NB: 谨慎使用！通常把它当作引用来使用很可能会导致 `PathBuffer` 字段的
/// provenance（来源信息）出错！
#[repr(C)]
pub struct SYMBOLIC_LINK_REPARSE_BUFFER {
    pub SubstituteNameOffset: c_ushort,
    pub SubstituteNameLength: c_ushort,
    pub PrintNameOffset: c_ushort,
    pub PrintNameLength: c_ushort,
    pub Flags: c_ulong,
    pub PathBuffer: WCHAR,
}

#[repr(C)]
pub struct MOUNT_POINT_REPARSE_BUFFER {
    pub SubstituteNameOffset: c_ushort,
    pub SubstituteNameLength: c_ushort,
    pub PrintNameOffset: c_ushort,
    pub PrintNameLength: c_ushort,
    pub PathBuffer: WCHAR,
}

// 桌面（Desktop）专用的函数与类型
#[cfg(not(target_vendor = "uwp"))]
pub const EXCEPTION_CONTINUE_SEARCH: i32 = 0;

// 使用 raw-dylib 来导入 ProcessPrng，因为我们无法依赖于存在对应的导入库（import library）。
#[cfg(not(target_vendor = "win7"))]
#[cfg_attr(
    target_arch = "x86",
    link(name = "bcryptprimitives", kind = "raw-dylib", import_name_type = "undecorated")
)]
#[cfg_attr(not(target_arch = "x86"), link(name = "bcryptprimitives", kind = "raw-dylib"))]
unsafe extern "system" {
    pub fn ProcessPrng(pbdata: *mut u8, cbdata: usize) -> BOOL;
}

windows_targets::link!("ntdll.dll" "system" fn NtCreateNamedPipeFile(
    filehandle: *mut HANDLE,
    desiredaccess: FILE_ACCESS_RIGHTS,
    objectattributes: *const OBJECT_ATTRIBUTES,
    iostatusblock: *mut IO_STATUS_BLOCK,
    shareaccess: FILE_SHARE_MODE,
    createdisposition: NTCREATEFILE_CREATE_DISPOSITION,
    createoptions: NTCREATEFILE_CREATE_OPTIONS,
    namedpipetype: u32,
    readmode: u32,
    completionmode: u32,
    maximuminstances: u32,
    inboundquota: u32,
    outboundquota: u32,
    defaulttimeout: *const u64,
) -> NTSTATUS);

// 这些函数并非在我们支持的每个 Windows 版本上都可用，
// 但我们仍然会使用它们，只是提供某种形式的回退（fallback）实现。
compat_fn_with_fallback! {
    pub static KERNEL32: &CStr = c"kernel32";

    // >= Win10 1607
    // https://docs.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-setthreaddescription
    pub fn SetThreadDescription(hthread: HANDLE, lpthreaddescription: PCWSTR) -> HRESULT {
        unsafe { SetLastError(ERROR_CALL_NOT_IMPLEMENTED as u32); E_NOTIMPL }
    }

    // >= Win10 1607
    // https://docs.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-getthreaddescription
    pub fn GetThreadDescription(hthread: HANDLE, lpthreaddescription: *mut PWSTR) -> HRESULT {
        unsafe { SetLastError(ERROR_CALL_NOT_IMPLEMENTED as u32); E_NOTIMPL }
    }

    // >= Win8 / Server 2012
    // https://docs.microsoft.com/en-us/windows/win32/api/sysinfoapi/nf-sysinfoapi-getsystemtimepreciseasfiletime
    #[cfg(target_vendor = "win7")]
    pub fn GetSystemTimePreciseAsFileTime(lpsystemtimeasfiletime: *mut FILETIME) -> () {
        unsafe { GetSystemTimeAsFileTime(lpsystemtimeasfiletime) }
    }

    // >= Win11 / Server 2022
    // https://docs.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-gettemppath2a
    pub fn GetTempPath2W(bufferlength: u32, buffer: PWSTR) -> u32 {
        unsafe {  GetTempPathW(bufferlength, buffer) }
    }
}

#[cfg(not(target_vendor = "win7"))]
// 使用 raw-dylib 来导入同步相关函数，以规避较旧 mingw 导入库（import library）的问题。
#[cfg_attr(
    target_arch = "x86",
    link(
        name = "api-ms-win-core-synch-l1-2-0",
        kind = "raw-dylib",
        import_name_type = "undecorated"
    )
)]
#[cfg_attr(
    not(target_arch = "x86"),
    link(name = "api-ms-win-core-synch-l1-2-0", kind = "raw-dylib")
)]
unsafe extern "system" {
    pub fn WaitOnAddress(
        address: *const c_void,
        compareaddress: *const c_void,
        addresssize: usize,
        dwmilliseconds: u32,
    ) -> BOOL;
    pub fn WakeByAddressSingle(address: *const c_void);
    pub fn WakeByAddressAll(address: *const c_void);
}

// 这些函数由 `load_synch_functions` 加载。
#[cfg(target_vendor = "win7")]
compat_fn_optional! {
    pub fn WaitOnAddress(
        address: *const c_void,
        compareaddress: *const c_void,
        addresssize: usize,
        dwmilliseconds: u32
    ) -> BOOL;
    pub fn WakeByAddressSingle(address: *const c_void);
}

#[cfg(any(target_vendor = "win7"))]
compat_fn_with_fallback! {
    pub static NTDLL: &CStr = c"ntdll";

    #[cfg(target_vendor = "win7")]
    pub fn NtCreateKeyedEvent(
        KeyedEventHandle: *mut HANDLE,
        DesiredAccess: u32,
        ObjectAttributes: *mut c_void,
        Flags: u32
    ) -> NTSTATUS {
        panic!("keyed events not available")
    }
    #[cfg(target_vendor = "win7")]
    pub fn NtReleaseKeyedEvent(
        EventHandle: HANDLE,
        Key: *const c_void,
        Alertable: bool,
        Timeout: *mut i64
    ) -> NTSTATUS {
        panic!("keyed events not available")
    }
    #[cfg(target_vendor = "win7")]
    pub fn NtWaitForKeyedEvent(
        EventHandle: HANDLE,
        Key: *const c_void,
        Alertable: bool,
        Timeout: *mut i64
    ) -> NTSTATUS {
        panic!("keyed events not available")
    }
}

cfg_select! {
    target_vendor = "uwp" => {
        windows_targets::link_raw_dylib!("ntdll.dll" "system" fn NtCreateFile(filehandle : *mut HANDLE, desiredaccess : FILE_ACCESS_RIGHTS, objectattributes : *const OBJECT_ATTRIBUTES, iostatusblock : *mut IO_STATUS_BLOCK, allocationsize : *const i64, fileattributes : FILE_FLAGS_AND_ATTRIBUTES, shareaccess : FILE_SHARE_MODE, createdisposition : NTCREATEFILE_CREATE_DISPOSITION, createoptions : NTCREATEFILE_CREATE_OPTIONS, eabuffer : *const core::ffi::c_void, ealength : u32) -> NTSTATUS);
        windows_targets::link_raw_dylib!("ntdll.dll" "system" fn NtOpenFile(filehandle : *mut HANDLE, desiredaccess : u32, objectattributes : *const OBJECT_ATTRIBUTES, iostatusblock : *mut IO_STATUS_BLOCK, shareaccess : u32, openoptions : u32) -> NTSTATUS);
        windows_targets::link_raw_dylib!("ntdll.dll" "system" fn NtReadFile(filehandle : HANDLE, event : HANDLE, apcroutine : PIO_APC_ROUTINE, apccontext : *const core::ffi::c_void, iostatusblock : *mut IO_STATUS_BLOCK, buffer : *mut core::ffi::c_void, length : u32, byteoffset : *const i64, key : *const u32) -> NTSTATUS);
        windows_targets::link_raw_dylib!("ntdll.dll" "system" fn NtWriteFile(filehandle : HANDLE, event : HANDLE, apcroutine : PIO_APC_ROUTINE, apccontext : *const core::ffi::c_void, iostatusblock : *mut IO_STATUS_BLOCK, buffer : *const core::ffi::c_void, length : u32, byteoffset : *const i64, key : *const u32) -> NTSTATUS);
        windows_targets::link_raw_dylib!("ntdll.dll" "system" fn RtlNtStatusToDosError(status : NTSTATUS) -> u32);
    }
    _ => {}
}
