use crate::os::windows::io::{
    AsHandle, AsRawHandle, BorrowedHandle, FromRawHandle, HandleOrInvalid, IntoRawHandle,
    OwnedHandle, RawHandle,
};
use crate::path::Path;
use crate::sys::api::{UnicodeStrRef, WinError};
use crate::sys::fs::windows::debug_path_handle;
use crate::sys::fs::{File, OpenOptions};
use crate::sys::handle::Handle;
use crate::sys::path::{WCStr, with_native_path};
use crate::sys::{AsInner, FromInner, IntoInner, IoResult, c, to_u16s};
use crate::{fmt, fs, io, ptr};

pub struct Dir {
    handle: Handle,
}

/// 对底层 NtCreateFile 调用的封装。
///
/// 它并不完全安全，因为 `OBJECT_ATTRIBUTES` 中包含裸指针。
unsafe fn nt_create_file(
    opts: &OpenOptions,
    object_attributes: &c::OBJECT_ATTRIBUTES,
    create_options: c::NTCREATEFILE_CREATE_OPTIONS,
) -> io::Result<Handle> {
    let mut handle = ptr::null_mut();
    let mut io_status = c::IO_STATUS_BLOCK::PENDING;
    // SYNCHRONIZE 包含在 FILE_GENERIC_READ 中，但不包含在 GENERIC_READ 中，因此我们手动加上它
    let access = opts.get_access_mode()? | c::SYNCHRONIZE;
    // 后续操作要想成功，FILE_SYNCHRONOUS_IO_{,NON}ALERT 中必须有一个被设置。
    let options = create_options | c::FILE_SYNCHRONOUS_IO_NONALERT;
    let status = unsafe {
        c::NtCreateFile(
            &mut handle,
            access,
            object_attributes,
            &mut io_status,
            ptr::null(),
            c::FILE_ATTRIBUTE_NORMAL,
            opts.share_mode,
            opts.get_disposition()?,
            options,
            ptr::null(),
            0,
        )
    };
    if c::nt_success(status) {
        // SAFETY: nt_success 保证 handle 不再为 null
        unsafe { Ok(Handle::from_raw_handle(handle)) }
    } else {
        Err(WinError::new(unsafe { c::RtlNtStatusToDosError(status) })).io_result()
    }
}

impl Dir {
    pub fn open(path: &Path, opts: &OpenOptions) -> io::Result<Self> {
        with_native_path(path, &|path| Self::open_with_native(path, opts))
    }

    pub fn open_file(&self, path: &Path, opts: &OpenOptions) -> io::Result<File> {
        // 若给定绝对路径且 RootDirectory 非 null，NtCreateFile 将会失败
        if path.is_absolute() {
            return File::open(path, opts);
        }
        let path = to_u16s(path)?;
        let path = &path[..path.len() - 1]; // 去掉末尾的 0 字节
        self.open_file_native(&path, opts).map(|handle| File { handle })
    }

    fn open_with_native(path: &WCStr, opts: &OpenOptions) -> io::Result<Self> {
        let creation = opts.get_creation_mode()?;
        let sa = c::SECURITY_ATTRIBUTES {
            nLength: size_of::<c::SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: ptr::null_mut(),
            bInheritHandle: opts.inherit_handle as c::BOOL,
        };
        let handle = unsafe {
            c::CreateFileW(
                path.as_ptr(),
                opts.get_access_mode()?,
                opts.share_mode,
                &raw const sa,
                creation,
                // 打开目录需要 FILE_FLAG_BACKUP_SEMANTICS
                opts.get_flags_and_attributes() | c::FILE_FLAG_BACKUP_SEMANTICS,
                ptr::null_mut(),
            )
        };
        match OwnedHandle::try_from(unsafe { HandleOrInvalid::from_raw_handle(handle) }) {
            Ok(handle) => Ok(Self { handle: Handle::from_inner(handle) }),
            Err(_) => Err(io::Error::last_os_error()),
        }
    }

    fn open_file_native(&self, path: &[u16], opts: &OpenOptions) -> io::Result<Handle> {
        let name = UnicodeStrRef::from_slice(path);
        let object_attributes = c::OBJECT_ATTRIBUTES {
            RootDirectory: self.handle.as_raw_handle(),
            ObjectName: name.as_ptr(),
            ..c::OBJECT_ATTRIBUTES::with_length()
        };
        unsafe { nt_create_file(opts, &object_attributes, c::FILE_NON_DIRECTORY_FILE) }
    }
}

impl fmt::Debug for Dir {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut b = debug_path_handle(self.handle.as_handle(), f, "Dir");
        b.finish()
    }
}

#[unstable(feature = "dirfd", issue = "120426")]
impl AsRawHandle for fs::Dir {
    fn as_raw_handle(&self) -> RawHandle {
        self.as_inner().handle.as_raw_handle()
    }
}

#[unstable(feature = "dirhandle", issue = "120426")]
impl IntoRawHandle for fs::Dir {
    fn into_raw_handle(self) -> RawHandle {
        self.into_inner().handle.into_raw_handle()
    }
}

#[unstable(feature = "dirhandle", issue = "120426")]
impl FromRawHandle for fs::Dir {
    unsafe fn from_raw_handle(handle: RawHandle) -> Self {
        Self::from_inner(Dir { handle: unsafe { FromRawHandle::from_raw_handle(handle) } })
    }
}

#[unstable(feature = "dirhandle", issue = "120426")]
impl AsHandle for fs::Dir {
    fn as_handle(&self) -> BorrowedHandle<'_> {
        self.as_inner().handle.as_handle()
    }
}

#[unstable(feature = "dirhandle", issue = "120426")]
impl From<fs::Dir> for OwnedHandle {
    fn from(value: fs::Dir) -> Self {
        value.into_inner().handle.into_inner()
    }
}

#[unstable(feature = "dirhandle", issue = "120426")]
impl From<OwnedHandle> for fs::Dir {
    fn from(value: OwnedHandle) -> Self {
        Self::from_inner(Dir { handle: Handle::from_inner(value) })
    }
}
