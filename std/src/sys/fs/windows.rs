#![allow(nonstandard_style)]

use crate::alloc::{Layout, alloc, dealloc};
use crate::borrow::Cow;
use crate::ffi::{OsStr, OsString, c_void};
use crate::fs::TryLockError;
use crate::io::{self, BorrowedCursor, Error, IoSlice, IoSliceMut, SeekFrom};
use crate::mem::{self, MaybeUninit, offset_of};
use crate::os::windows::io::{AsHandle, BorrowedHandle};
use crate::os::windows::prelude::*;
use crate::path::{Path, PathBuf};
use crate::sync::Arc;
use crate::sys::handle::Handle;
use crate::sys::pal::api::{self, WinError, set_file_information_by_handle};
use crate::sys::pal::{IoResult, fill_utf16_buf, to_u16s, truncate_utf16_at_nul};
use crate::sys::path::{WCStr, maybe_verbatim};
use crate::sys::time::SystemTime;
use crate::sys::{Align8, AsInner, FromInner, IntoInner, c, cvt};
use crate::{fmt, ptr, slice};

mod dir;
pub use dir::Dir;
mod remove_dir_all;
use remove_dir_all::remove_dir_all_iterative;

pub struct File {
    handle: Handle,
}

#[derive(Clone)]
pub struct FileAttr {
    attributes: u32,
    creation_time: c::FILETIME,
    last_access_time: c::FILETIME,
    last_write_time: c::FILETIME,
    change_time: Option<c::FILETIME>,
    file_size: u64,
    reparse_tag: u32,
    volume_serial_number: Option<u32>,
    number_of_links: Option<u32>,
    file_index: Option<u64>,
}

#[derive(Copy, Clone, PartialEq, Eq, Hash, Debug)]
pub struct FileType {
    is_directory: bool,
    is_symlink: bool,
}

pub struct ReadDir {
    handle: Option<FindNextFileHandle>,
    root: Arc<PathBuf>,
    first: Option<c::WIN32_FIND_DATAW>,
}

struct FindNextFileHandle(c::HANDLE);

unsafe impl Send for FindNextFileHandle {}
unsafe impl Sync for FindNextFileHandle {}

pub struct DirEntry {
    root: Arc<PathBuf>,
    data: c::WIN32_FIND_DATAW,
}

unsafe impl Send for OpenOptions {}
unsafe impl Sync for OpenOptions {}

#[derive(Clone, Debug)]
pub struct OpenOptions {
    // 通用部分
    read: bool,
    write: bool,
    append: bool,
    truncate: bool,
    create: bool,
    create_new: bool,
    // 系统特定部分
    custom_flags: u32,
    access_mode: Option<u32>,
    attributes: u32,
    share_mode: u32,
    security_qos_flags: u32,
    inherit_handle: bool,
    freeze_last_access_time: bool,
    freeze_last_write_time: bool,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FilePermissions {
    attrs: u32,
}

#[derive(Copy, Clone, Debug, Default)]
pub struct FileTimes {
    accessed: Option<c::FILETIME>,
    modified: Option<c::FILETIME>,
    created: Option<c::FILETIME>,
}

impl fmt::Debug for c::FILETIME {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let time = ((self.dwHighDateTime as u64) << 32) | self.dwLowDateTime as u64;
        f.debug_tuple("FILETIME").field(&time).finish()
    }
}

#[derive(Debug)]
pub struct DirBuilder;

impl fmt::Debug for ReadDir {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // 它只会从 std::fs::ReadDir 中被调用，后者会添加一个 "ReadDir()" 帧。
        // 因此结果会是例如 'ReadDir("C:\")' 这样的形式。
        fmt::Debug::fmt(&*self.root, f)
    }
}

impl Iterator for ReadDir {
    type Item = io::Result<DirEntry>;
    fn next(&mut self) -> Option<io::Result<DirEntry>> {
        let Some(handle) = self.handle.as_ref() else {
            // 这个迭代器是以 `INVALID_HANDLE_VALUE` 作为其句柄来初始化的。
            // 这里直接返回 `None`，因为只有当构造该迭代器时 `FindFirstFileExW`
            // 返回 `ERROR_FILE_NOT_FOUND`（意味着找不到任何匹配的文件）时，
            // 才会出现这种情况。
            return None;
        };
        if let Some(first) = self.first.take() {
            if let Some(e) = DirEntry::new(&self.root, &first) {
                return Some(Ok(e));
            }
        }
        unsafe {
            let mut wfd = mem::zeroed();
            loop {
                if c::FindNextFileW(handle.0, &mut wfd) == 0 {
                    self.handle = None;
                    match api::get_last_error() {
                        WinError::NO_MORE_FILES => return None,
                        WinError { code } => {
                            return Some(Err(Error::from_raw_os_error(code as i32)));
                        }
                    }
                }
                if let Some(e) = DirEntry::new(&self.root, &wfd) {
                    return Some(Ok(e));
                }
            }
        }
    }
}

impl Drop for FindNextFileHandle {
    fn drop(&mut self) {
        let r = unsafe { c::FindClose(self.0) };
        debug_assert!(r != 0);
    }
}

impl DirEntry {
    fn new(root: &Arc<PathBuf>, wfd: &c::WIN32_FIND_DATAW) -> Option<DirEntry> {
        match &wfd.cFileName[0..3] {
            // 检查 '.' 和 '..'
            &[46, 0, ..] | &[46, 46, 0, ..] => return None,
            _ => {}
        }

        Some(DirEntry { root: root.clone(), data: *wfd })
    }

    pub fn path(&self) -> PathBuf {
        self.root.join(self.file_name())
    }

    pub fn file_name(&self) -> OsString {
        let filename = truncate_utf16_at_nul(&self.data.cFileName);
        OsString::from_wide(filename)
    }

    pub fn file_type(&self) -> io::Result<FileType> {
        Ok(FileType::new(
            self.data.dwFileAttributes,
            /* reparse_tag = */ self.data.dwReserved0,
        ))
    }

    pub fn metadata(&self) -> io::Result<FileAttr> {
        Ok(self.data.into())
    }
}

impl OpenOptions {
    pub fn new() -> OpenOptions {
        OpenOptions {
            // 通用部分
            read: false,
            write: false,
            append: false,
            truncate: false,
            create: false,
            create_new: false,
            // 系统特定部分
            custom_flags: 0,
            access_mode: None,
            share_mode: c::FILE_SHARE_READ | c::FILE_SHARE_WRITE | c::FILE_SHARE_DELETE,
            attributes: 0,
            security_qos_flags: 0,
            inherit_handle: false,
            freeze_last_access_time: false,
            freeze_last_write_time: false,
        }
    }

    pub fn read(&mut self, read: bool) {
        self.read = read;
    }
    pub fn write(&mut self, write: bool) {
        self.write = write;
    }
    pub fn append(&mut self, append: bool) {
        self.append = append;
    }
    pub fn truncate(&mut self, truncate: bool) {
        self.truncate = truncate;
    }
    pub fn create(&mut self, create: bool) {
        self.create = create;
    }
    pub fn create_new(&mut self, create_new: bool) {
        self.create_new = create_new;
    }

    pub fn custom_flags(&mut self, flags: u32) {
        self.custom_flags = flags;
    }
    pub fn access_mode(&mut self, access_mode: u32) {
        self.access_mode = Some(access_mode);
    }
    pub fn share_mode(&mut self, share_mode: u32) {
        self.share_mode = share_mode;
    }
    pub fn attributes(&mut self, attrs: u32) {
        self.attributes = attrs;
    }
    pub fn security_qos_flags(&mut self, flags: u32) {
        // 我们必须在这里设置 `SECURITY_SQOS_PRESENT`，因为我们可能收到的一个有效标志
        // 是 `SECURITY_ANONYMOUS = 0x0`，而我们之后无法对它进行检查。
        self.security_qos_flags = flags | c::SECURITY_SQOS_PRESENT;
    }
    pub fn inherit_handle(&mut self, inherit: bool) {
        self.inherit_handle = inherit;
    }
    pub fn freeze_last_access_time(&mut self, freeze: bool) {
        self.freeze_last_access_time = freeze;
    }
    pub fn freeze_last_write_time(&mut self, freeze: bool) {
        self.freeze_last_write_time = freeze;
    }

    fn get_access_mode(&self) -> io::Result<u32> {
        match (self.read, self.write, self.append, self.access_mode) {
            (.., Some(mode)) => Ok(mode),
            (true, false, false, None) => Ok(c::GENERIC_READ),
            (false, true, false, None) => Ok(c::GENERIC_WRITE),
            (true, true, false, None) => Ok(c::GENERIC_READ | c::GENERIC_WRITE),
            (false, _, true, None) => Ok(c::FILE_GENERIC_WRITE & !c::FILE_WRITE_DATA),
            (true, _, true, None) => {
                Ok(c::GENERIC_READ | (c::FILE_GENERIC_WRITE & !c::FILE_WRITE_DATA))
            }
            (false, false, false, None) => {
                // 如果没有设置任何访问模式，检查是否设置了任何创建标志（creation flags），
                // 以便提供一条更具描述性的错误信息
                if self.create || self.create_new || self.truncate {
                    Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "creating or truncating a file requires write or append access",
                    ))
                } else {
                    Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "must specify at least one of read, write, or append access",
                    ))
                }
            }
        }
    }

    fn get_cmode_disposition(&self) -> io::Result<(u32, u32)> {
        match (self.write, self.append) {
            (true, false) => {}
            (false, false) => {
                if self.truncate || self.create || self.create_new {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "creating or truncating a file requires write or append access",
                    ));
                }
            }
            (_, true) => {
                if self.truncate && !self.create_new {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "creating or truncating a file requires write or append access",
                    ));
                }
            }
        }

        Ok(match (self.create, self.truncate, self.create_new) {
            (false, false, false) => (c::OPEN_EXISTING, c::FILE_OPEN),
            (true, false, false) => (c::OPEN_ALWAYS, c::FILE_OPEN_IF),
            (false, true, false) => (c::TRUNCATE_EXISTING, c::FILE_OVERWRITE),
            // `CREATE_ALWAYS` 的语义很怪，因此我们用 `OPEN_ALWAYS` 加上一个手动的
            // 截断（truncation）步骤来模拟它。参见 #115745。
            (true, true, false) => (c::OPEN_ALWAYS, c::FILE_OVERWRITE_IF),
            (_, _, true) => (c::CREATE_NEW, c::FILE_CREATE),
        })
    }

    fn get_creation_mode(&self) -> io::Result<u32> {
        self.get_cmode_disposition().map(|(mode, _)| mode)
    }

    fn get_disposition(&self) -> io::Result<u32> {
        self.get_cmode_disposition().map(|(_, mode)| mode)
    }

    fn get_flags_and_attributes(&self) -> u32 {
        self.custom_flags
            | self.attributes
            | self.security_qos_flags
            | if self.create_new { c::FILE_FLAG_OPEN_REPARSE_POINT } else { 0 }
    }
}

impl File {
    pub fn open(path: &Path, opts: &OpenOptions) -> io::Result<File> {
        let path = maybe_verbatim(path)?;
        // SAFETY: maybe_verbatim 返回以 null 结尾的字符串
        let path = unsafe { WCStr::from_wchars_with_null_unchecked(&path) };
        Self::open_native(&path, opts)
    }

    fn open_native(path: &WCStr, opts: &OpenOptions) -> io::Result<File> {
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
                if opts.inherit_handle { &sa } else { ptr::null() },
                creation,
                opts.get_flags_and_attributes(),
                ptr::null_mut(),
            )
        };
        let handle = unsafe { HandleOrInvalid::from_raw_handle(handle) };
        if let Ok(handle) = OwnedHandle::try_from(handle) {
            if opts.freeze_last_access_time || opts.freeze_last_write_time {
                let file_time =
                    c::FILETIME { dwLowDateTime: 0xFFFFFFFF, dwHighDateTime: 0xFFFFFFFF };
                cvt(unsafe {
                    c::SetFileTime(
                        handle.as_raw_handle(),
                        core::ptr::null(),
                        if opts.freeze_last_access_time { &file_time } else { core::ptr::null() },
                        if opts.freeze_last_write_time { &file_time } else { core::ptr::null() },
                    )
                })?;
            }
            // 手动截断。参见 #115745。
            if opts.truncate
                && creation == c::OPEN_ALWAYS
                && api::get_last_error() == WinError::ALREADY_EXISTS
            {
                // 它首先尝试 `FileAllocationInfo`，但为了支持 WINE，会回退到
                // `FileEndOfFileInfo`。
                // 如果 WINE 增加了对 FileAllocationInfo 的支持，我们就应当移除这个回退。
                let alloc = c::FILE_ALLOCATION_INFO { AllocationSize: 0 };
                set_file_information_by_handle(handle.as_raw_handle(), &alloc)
                    .or_else(|_| {
                        let eof = c::FILE_END_OF_FILE_INFO { EndOfFile: 0 };
                        set_file_information_by_handle(handle.as_raw_handle(), &eof)
                    })
                    .io_result()?;
            }
            Ok(File { handle: Handle::from_inner(handle) })
        } else {
            Err(Error::last_os_error())
        }
    }

    pub fn fsync(&self) -> io::Result<()> {
        cvt(unsafe { c::FlushFileBuffers(self.handle.as_raw_handle()) })?;
        Ok(())
    }

    pub fn datasync(&self) -> io::Result<()> {
        self.fsync()
    }

    fn acquire_lock(&self, flags: c::LOCK_FILE_FLAGS) -> io::Result<()> {
        unsafe {
            let mut overlapped: c::OVERLAPPED = mem::zeroed();
            let event = c::CreateEventW(ptr::null_mut(), c::FALSE, c::FALSE, ptr::null());
            if event.is_null() {
                return Err(io::Error::last_os_error());
            }
            overlapped.hEvent = event;
            let lock_result = cvt(c::LockFileEx(
                self.handle.as_raw_handle(),
                flags,
                0,
                u32::MAX,
                u32::MAX,
                &mut overlapped,
            ));

            let final_result = match lock_result {
                Ok(_) => Ok(()),
                Err(err) => {
                    if err.raw_os_error() == Some(c::ERROR_IO_PENDING as i32) {
                        // 等待锁被获取，并获取该加锁操作的状态。
                        // 如果文件句柄是以异步 IO 方式打开的，这可能是异步发生的
                        let mut bytes_transferred = 0;
                        cvt(c::GetOverlappedResult(
                            self.handle.as_raw_handle(),
                            &mut overlapped,
                            &mut bytes_transferred,
                            c::TRUE,
                        ))
                        .map(|_| ())
                    } else {
                        Err(err)
                    }
                }
            };
            c::CloseHandle(overlapped.hEvent);
            final_result
        }
    }

    pub fn lock(&self) -> io::Result<()> {
        self.acquire_lock(c::LOCKFILE_EXCLUSIVE_LOCK)
    }

    pub fn lock_shared(&self) -> io::Result<()> {
        self.acquire_lock(0)
    }

    pub fn try_lock(&self) -> Result<(), TryLockError> {
        let result = cvt(unsafe {
            let mut overlapped = mem::zeroed();
            c::LockFileEx(
                self.handle.as_raw_handle(),
                c::LOCKFILE_EXCLUSIVE_LOCK | c::LOCKFILE_FAIL_IMMEDIATELY,
                0,
                u32::MAX,
                u32::MAX,
                &mut overlapped,
            )
        });

        match result {
            Ok(_) => Ok(()),
            Err(err) if err.raw_os_error() == Some(c::ERROR_LOCK_VIOLATION as i32) => {
                Err(TryLockError::WouldBlock)
            }
            Err(err) => Err(TryLockError::Error(err)),
        }
    }

    pub fn try_lock_shared(&self) -> Result<(), TryLockError> {
        let result = cvt(unsafe {
            let mut overlapped = mem::zeroed();
            c::LockFileEx(
                self.handle.as_raw_handle(),
                c::LOCKFILE_FAIL_IMMEDIATELY,
                0,
                u32::MAX,
                u32::MAX,
                &mut overlapped,
            )
        });

        match result {
            Ok(_) => Ok(()),
            Err(err) if err.raw_os_error() == Some(c::ERROR_LOCK_VIOLATION as i32) => {
                Err(TryLockError::WouldBlock)
            }
            Err(err) => Err(TryLockError::Error(err)),
        }
    }

    pub fn unlock(&self) -> io::Result<()> {
        // 对该句柄解锁两次，因为 LockFileEx() 允许一个文件句柄同时获得
        // 排他锁（exclusive lock）和共享锁（shared lock），在这种情况下文档指出：
        // "...需要两次解锁操作才能解锁该区域；第一次解锁操作解开排他锁，
        // 第二次解锁操作解开共享锁"
        cvt(unsafe { c::UnlockFile(self.handle.as_raw_handle(), 0, 0, u32::MAX, u32::MAX) })?;
        let result =
            cvt(unsafe { c::UnlockFile(self.handle.as_raw_handle(), 0, 0, u32::MAX, u32::MAX) });
        match result {
            Ok(_) => Ok(()),
            Err(err) if err.raw_os_error() == Some(c::ERROR_NOT_LOCKED as i32) => Ok(()),
            Err(err) => Err(err),
        }
    }

    pub fn truncate(&self, size: u64) -> io::Result<()> {
        let info = c::FILE_END_OF_FILE_INFO { EndOfFile: size as i64 };
        api::set_file_information_by_handle(self.handle.as_raw_handle(), &info).io_result()
    }

    #[cfg(not(target_vendor = "uwp"))]
    pub fn file_attr(&self) -> io::Result<FileAttr> {
        unsafe {
            let mut info: c::BY_HANDLE_FILE_INFORMATION = mem::zeroed();
            cvt(c::GetFileInformationByHandle(self.handle.as_raw_handle(), &mut info))?;
            let mut reparse_tag = 0;
            if info.dwFileAttributes & c::FILE_ATTRIBUTE_REPARSE_POINT != 0 {
                let mut attr_tag: c::FILE_ATTRIBUTE_TAG_INFO = mem::zeroed();
                cvt(c::GetFileInformationByHandleEx(
                    self.handle.as_raw_handle(),
                    c::FileAttributeTagInfo,
                    (&raw mut attr_tag).cast(),
                    size_of::<c::FILE_ATTRIBUTE_TAG_INFO>().try_into().unwrap(),
                ))?;
                if attr_tag.FileAttributes & c::FILE_ATTRIBUTE_REPARSE_POINT != 0 {
                    reparse_tag = attr_tag.ReparseTag;
                }
            }
            Ok(FileAttr {
                attributes: info.dwFileAttributes,
                creation_time: info.ftCreationTime,
                last_access_time: info.ftLastAccessTime,
                last_write_time: info.ftLastWriteTime,
                change_time: None, // Only available in FILE_BASIC_INFO
                file_size: (info.nFileSizeLow as u64) | ((info.nFileSizeHigh as u64) << 32),
                reparse_tag,
                volume_serial_number: Some(info.dwVolumeSerialNumber),
                number_of_links: Some(info.nNumberOfLinks),
                file_index: Some(
                    (info.nFileIndexLow as u64) | ((info.nFileIndexHigh as u64) << 32),
                ),
            })
        }
    }

    #[cfg(target_vendor = "uwp")]
    pub fn file_attr(&self) -> io::Result<FileAttr> {
        unsafe {
            let mut info: c::FILE_BASIC_INFO = mem::zeroed();
            let size = size_of_val(&info);
            cvt(c::GetFileInformationByHandleEx(
                self.handle.as_raw_handle(),
                c::FileBasicInfo,
                (&raw mut info) as *mut c_void,
                size as u32,
            ))?;
            let mut attr = FileAttr {
                attributes: info.FileAttributes,
                creation_time: c::FILETIME {
                    dwLowDateTime: info.CreationTime as u32,
                    dwHighDateTime: (info.CreationTime >> 32) as u32,
                },
                last_access_time: c::FILETIME {
                    dwLowDateTime: info.LastAccessTime as u32,
                    dwHighDateTime: (info.LastAccessTime >> 32) as u32,
                },
                last_write_time: c::FILETIME {
                    dwLowDateTime: info.LastWriteTime as u32,
                    dwHighDateTime: (info.LastWriteTime >> 32) as u32,
                },
                change_time: Some(c::FILETIME {
                    dwLowDateTime: info.ChangeTime as u32,
                    dwHighDateTime: (info.ChangeTime >> 32) as u32,
                }),
                file_size: 0,
                reparse_tag: 0,
                volume_serial_number: None,
                number_of_links: None,
                file_index: None,
            };
            let mut info: c::FILE_STANDARD_INFO = mem::zeroed();
            let size = size_of_val(&info);
            cvt(c::GetFileInformationByHandleEx(
                self.handle.as_raw_handle(),
                c::FileStandardInfo,
                (&raw mut info) as *mut c_void,
                size as u32,
            ))?;
            attr.file_size = info.AllocationSize as u64;
            attr.number_of_links = Some(info.NumberOfLinks);
            if attr.attributes & c::FILE_ATTRIBUTE_REPARSE_POINT != 0 {
                let mut attr_tag: c::FILE_ATTRIBUTE_TAG_INFO = mem::zeroed();
                cvt(c::GetFileInformationByHandleEx(
                    self.handle.as_raw_handle(),
                    c::FileAttributeTagInfo,
                    (&raw mut attr_tag).cast(),
                    size_of::<c::FILE_ATTRIBUTE_TAG_INFO>().try_into().unwrap(),
                ))?;
                if attr_tag.FileAttributes & c::FILE_ATTRIBUTE_REPARSE_POINT != 0 {
                    attr.reparse_tag = attr_tag.ReparseTag;
                }
            }
            Ok(attr)
        }
    }

    pub fn read(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.handle.read(buf)
    }

    pub fn read_vectored(&self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        self.handle.read_vectored(bufs)
    }

    #[inline]
    pub fn is_read_vectored(&self) -> bool {
        self.handle.is_read_vectored()
    }

    pub fn read_at(&self, buf: &mut [u8], offset: u64) -> io::Result<usize> {
        self.handle.read_at(buf, offset)
    }

    pub fn read_buf(&self, cursor: BorrowedCursor<'_>) -> io::Result<()> {
        self.handle.read_buf(cursor)
    }

    pub fn read_buf_at(&self, cursor: BorrowedCursor<'_>, offset: u64) -> io::Result<()> {
        self.handle.read_buf_at(cursor, offset)
    }

    pub fn write(&self, buf: &[u8]) -> io::Result<usize> {
        self.handle.write(buf)
    }

    pub fn write_vectored(&self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        self.handle.write_vectored(bufs)
    }

    #[inline]
    pub fn is_write_vectored(&self) -> bool {
        self.handle.is_write_vectored()
    }

    pub fn write_at(&self, buf: &[u8], offset: u64) -> io::Result<usize> {
        self.handle.write_at(buf, offset)
    }

    pub fn flush(&self) -> io::Result<()> {
        Ok(())
    }

    pub fn seek(&self, pos: SeekFrom) -> io::Result<u64> {
        let (whence, pos) = match pos {
            // 转换为 `i64` 是没问题的，`SetFilePointerEx` 会把这个整数
            // 重新解释（reinterpret）为 `u64`。
            SeekFrom::Start(n) => (c::FILE_BEGIN, n as i64),
            SeekFrom::End(n) => (c::FILE_END, n),
            SeekFrom::Current(n) => (c::FILE_CURRENT, n),
        };
        let pos = pos as i64;
        let mut newpos = 0;
        cvt(unsafe { c::SetFilePointerEx(self.handle.as_raw_handle(), pos, &mut newpos, whence) })?;
        Ok(newpos as u64)
    }

    pub fn size(&self) -> Option<io::Result<u64>> {
        let mut result = 0;
        Some(
            cvt(unsafe { c::GetFileSizeEx(self.handle.as_raw_handle(), &mut result) })
                .map(|_| result as u64),
        )
    }

    pub fn tell(&self) -> io::Result<u64> {
        self.seek(SeekFrom::Current(0))
    }

    pub fn duplicate(&self) -> io::Result<File> {
        Ok(Self { handle: self.handle.try_clone()? })
    }

    // NB: 返回的指针派生自 `space`，并具有与之匹配的来源（provenance）。
    // 这里返回裸指针而不是引用，是为了避免把来源收窄到实际的 `REPARSE_DATA_BUFFER`。
    fn reparse_point(
        &self,
        space: &mut Align8<[MaybeUninit<u8>]>,
    ) -> io::Result<(u32, *mut c::REPARSE_DATA_BUFFER)> {
        unsafe {
            let mut bytes = 0;
            cvt({
                // 提前获取它，以避免它使我们从 `space.0.as_mut_ptr()` 得到的指针失效。
                let len = space.0.len();
                c::DeviceIoControl(
                    self.handle.as_raw_handle(),
                    c::FSCTL_GET_REPARSE_POINT,
                    ptr::null_mut(),
                    0,
                    space.0.as_mut_ptr().cast(),
                    len as u32,
                    &mut bytes,
                    ptr::null_mut(),
                )
            })?;
            const _: () = assert!(align_of::<c::REPARSE_DATA_BUFFER>() <= 8);
            Ok((bytes, space.0.as_mut_ptr().cast::<c::REPARSE_DATA_BUFFER>()))
        }
    }

    fn readlink(&self) -> io::Result<PathBuf> {
        let mut space =
            Align8([MaybeUninit::<u8>::uninit(); c::MAXIMUM_REPARSE_DATA_BUFFER_SIZE as usize]);
        let (_bytes, buf) = self.reparse_point(&mut space)?;
        unsafe {
            let (path_buffer, subst_off, subst_len, relative) = match (*buf).ReparseTag {
                c::IO_REPARSE_TAG_SYMLINK => {
                    let info: *mut c::SYMBOLIC_LINK_REPARSE_BUFFER = (&raw mut (*buf).rest).cast();
                    assert!(info.is_aligned());
                    (
                        (&raw mut (*info).PathBuffer).cast::<u16>(),
                        (*info).SubstituteNameOffset / 2,
                        (*info).SubstituteNameLength / 2,
                        (*info).Flags & c::SYMLINK_FLAG_RELATIVE != 0,
                    )
                }
                c::IO_REPARSE_TAG_MOUNT_POINT => {
                    let info: *mut c::MOUNT_POINT_REPARSE_BUFFER = (&raw mut (*buf).rest).cast();
                    assert!(info.is_aligned());
                    (
                        (&raw mut (*info).PathBuffer).cast::<u16>(),
                        (*info).SubstituteNameOffset / 2,
                        (*info).SubstituteNameLength / 2,
                        false,
                    )
                }
                _ => {
                    return Err(io::const_error!(
                        io::ErrorKind::Uncategorized,
                        "Unsupported reparse point type",
                    ));
                }
            };
            let subst_ptr = path_buffer.add(subst_off.into());
            let subst = slice::from_raw_parts_mut(subst_ptr, subst_len as usize);
            // 绝对路径以 NT 内部命名空间前缀 `\??\` 开头
            // 我们不应让它泄漏出去。
            if !relative && subst.starts_with(&[92u16, 63u16, 63u16, 92u16]) {
                // 把 `\??\` 变成 `\\?\`（一个 verbatim 路径）。
                subst[1] = b'\\' as u16;
                // 尝试转换为一个更便于用户阅读的路径。
                let user = crate::sys::args::from_wide_to_user_path(
                    subst.iter().copied().chain([0]).collect(),
                )?;
                Ok(PathBuf::from(OsString::from_wide(user.strip_suffix(&[0]).unwrap_or(&user))))
            } else {
                Ok(PathBuf::from(OsString::from_wide(subst)))
            }
        }
    }

    pub fn set_permissions(&self, perm: FilePermissions) -> io::Result<()> {
        let info = c::FILE_BASIC_INFO {
            CreationTime: 0,
            LastAccessTime: 0,
            LastWriteTime: 0,
            ChangeTime: 0,
            FileAttributes: perm.attrs,
        };
        api::set_file_information_by_handle(self.handle.as_raw_handle(), &info).io_result()
    }

    pub fn set_times(&self, times: FileTimes) -> io::Result<()> {
        let is_zero = |t: c::FILETIME| t.dwLowDateTime == 0 && t.dwHighDateTime == 0;
        if times.accessed.map_or(false, is_zero)
            || times.modified.map_or(false, is_zero)
            || times.created.map_or(false, is_zero)
        {
            return Err(io::const_error!(
                io::ErrorKind::InvalidInput,
                "cannot set file timestamp to 0",
            ));
        }
        let is_max = |t: c::FILETIME| t.dwLowDateTime == u32::MAX && t.dwHighDateTime == u32::MAX;
        if times.accessed.map_or(false, is_max)
            || times.modified.map_or(false, is_max)
            || times.created.map_or(false, is_max)
        {
            return Err(io::const_error!(
                io::ErrorKind::InvalidInput,
                "cannot set file timestamp to 0xFFFF_FFFF_FFFF_FFFF",
            ));
        }
        cvt(unsafe {
            let created =
                times.created.as_ref().map(|a| a as *const c::FILETIME).unwrap_or(ptr::null());
            let accessed =
                times.accessed.as_ref().map(|a| a as *const c::FILETIME).unwrap_or(ptr::null());
            let modified =
                times.modified.as_ref().map(|a| a as *const c::FILETIME).unwrap_or(ptr::null());
            c::SetFileTime(self.as_raw_handle(), created, accessed, modified)
        })?;
        Ok(())
    }

    /// 只获取诸如属性（attributes）和文件时间这类基本的文件信息。
    fn basic_info(&self) -> io::Result<c::FILE_BASIC_INFO> {
        unsafe {
            let mut info: c::FILE_BASIC_INFO = mem::zeroed();
            let size = size_of_val(&info);
            cvt(c::GetFileInformationByHandleEx(
                self.handle.as_raw_handle(),
                c::FileBasicInfo,
                (&raw mut info) as *mut c_void,
                size as u32,
            ))?;
            Ok(info)
        }
    }

    /// 删除该文件，并消耗（consume）文件句柄，以确保删除尽可能立即发生。
    /// 它会尝试使用 `posix_delete`，但如果文件系统不支持，则回退到 `win32_delete`。
    #[allow(unused)]
    fn delete(self) -> Result<(), WinError> {
        // 如果该文件系统不支持 POSIX delete，则回退到 win32 delete。
        match self.posix_delete() {
            Err(WinError::INVALID_PARAMETER)
            | Err(WinError::NOT_SUPPORTED)
            | Err(WinError::INVALID_FUNCTION) => self.win32_delete(),
            result => result,
        }
    }

    /// 使用 POSIX 语义进行删除。
    ///
    /// 一旦句柄被关闭，文件就会被删除。该特性在 Windows 10 1607（也即 RS1）
    /// 及更高版本上受支持。不过，即便如此，某些文件系统驱动仍不会支持它，例如 FAT32。
    ///
    /// 如果该文件系统或操作系统版本不支持此操作，那么错误将会是
    /// `ERROR_NOT_SUPPORTED` 或 `ERROR_INVALID_PARAMETER`。
    #[allow(unused)]
    fn posix_delete(&self) -> Result<(), WinError> {
        let info = c::FILE_DISPOSITION_INFO_EX {
            Flags: c::FILE_DISPOSITION_FLAG_DELETE
                | c::FILE_DISPOSITION_FLAG_POSIX_SEMANTICS
                | c::FILE_DISPOSITION_FLAG_IGNORE_READONLY_ATTRIBUTE,
        };
        api::set_file_information_by_handle(self.handle.as_raw_handle(), &info)
    }

    /// 使用 win32 语义删除文件。在所有文件句柄都关闭之前，文件实际上不会被删除。
    /// 然而，把一个文件标记为待删除（marking a file for deletion）会阻止任何人
    /// 打开指向该文件的新句柄。
    #[allow(unused)]
    fn win32_delete(&self) -> Result<(), WinError> {
        let info = c::FILE_DISPOSITION_INFO { DeleteFile: true };
        api::set_file_information_by_handle(self.handle.as_raw_handle(), &info)
    }

    /// 用尽可能多的目录条目填充给定的缓冲区（buffer）。
    /// 它会记住自己的位置，并在下一次调用时从上次的位置继续，除非
    /// `restart` 被设为 `true`。
    ///
    /// 返回的 bool 表示是否还有更多条目。
    /// 如果 `self` 不是目录，则视为一个错误。
    ///
    /// # 符号链接与其他重解析点（reparse points）
    ///
    /// 在 Windows 上，一个文件要么是目录，要么是非目录。
    /// 符号链接目录其实就是一个附带了某些 "reparse"（重解析）元数据的空目录。
    /// 因此，如果你打开一个链接（而不是它的目标）并遍历该目录，
    /// 无论目标是什么，你遍历到的总是一个空目录。
    #[allow(unused)]
    fn fill_dir_buff(&self, buffer: &mut DirBuff, restart: bool) -> Result<bool, WinError> {
        let class =
            if restart { c::FileIdBothDirectoryRestartInfo } else { c::FileIdBothDirectoryInfo };

        unsafe {
            let result = c::GetFileInformationByHandleEx(
                self.as_raw_handle(),
                class,
                buffer.as_mut_ptr().cast(),
                buffer.capacity() as _,
            );
            if result == 0 {
                let err = api::get_last_error();
                if err.code == c::ERROR_NO_MORE_FILES { Ok(false) } else { Err(err) }
            } else {
                Ok(true)
            }
        }
    }
}

/// 用于存放目录条目的缓冲区。
struct DirBuff {
    buffer: Box<Align8<[MaybeUninit<u8>; Self::BUFFER_SIZE]>>,
}
impl DirBuff {
    const BUFFER_SIZE: usize = 1024;
    fn new() -> Self {
        Self {
            // Safety: `Align8<[MaybeUninit<u8>; N]>` 不需要初始化。
            buffer: unsafe { Box::new_uninit().assume_init() },
        }
    }
    fn capacity(&self) -> usize {
        self.buffer.0.len()
    }
    fn as_mut_ptr(&mut self) -> *mut u8 {
        self.buffer.0.as_mut_ptr().cast()
    }
    /// 返回一个 `DirBuffIter`。
    fn iter(&self) -> DirBuffIter<'_> {
        DirBuffIter::new(self)
    }
}
impl AsRef<[MaybeUninit<u8>]> for DirBuff {
    fn as_ref(&self) -> &[MaybeUninit<u8>] {
        &self.buffer.0
    }
}

/// 一个对存放在 `DirBuff` 中的条目进行迭代的迭代器。
///
/// 当前仅返回文件名（以 UTF-16 编码）。
struct DirBuffIter<'a> {
    buffer: Option<&'a [MaybeUninit<u8>]>,
    cursor: usize,
}
impl<'a> DirBuffIter<'a> {
    fn new(buffer: &'a DirBuff) -> Self {
        Self { buffer: Some(buffer.as_ref()), cursor: 0 }
    }
}
impl<'a> Iterator for DirBuffIter<'a> {
    type Item = (Cow<'a, [u16]>, bool);
    fn next(&mut self) -> Option<Self::Item> {
        let buffer = &self.buffer?[self.cursor..];

        // 从缓冲区中获取名称和下一个条目。
        // SAFETY:
        // - 缓冲区包含一个 `FILE_ID_BOTH_DIR_INFO` 结构体，但其最后一个字段
        //   （文件名）是不定长（unsized）的。因此必须使用一个偏移量来获取文件名切片。
        // - 操作系统保证了 `FILE_ID_BOTH_DIR_INFO` 各字段以及尾部文件名
        //   （至少 `FileNameLength` 个字节）的初始化
        let (name, is_directory, next_entry) = unsafe {
            let info = buffer.as_ptr().cast::<c::FILE_ID_BOTH_DIR_INFO>();
            // 尽管在如下文档中它被保证是对齐的
            // https://docs.microsoft.com/en-us/windows/win32/api/winbase/ns-winbase-file_id_both_dir_info
            // 但现实似乎并没有这么友善，假定它对齐曾在某些情况下导致崩溃
            //（https://github.com/rust-lang/rust/issues/104530）。
            // 这大概可以归咎于有 bug 的文件系统驱动，但谁知道呢。
            let next_entry = (&raw const (*info).NextEntryOffset).read_unaligned() as usize;
            let length = (&raw const (*info).FileNameLength).read_unaligned() as usize;
            let attrs = (&raw const (*info).FileAttributes).read_unaligned();
            let name = from_maybe_unaligned(
                (&raw const (*info).FileName).cast::<u16>(),
                length / size_of::<u16>(),
            );
            let is_directory = (attrs & c::FILE_ATTRIBUTE_DIRECTORY) != 0;

            (name, is_directory, next_entry)
        };

        if next_entry == 0 {
            self.buffer = None
        } else {
            self.cursor += next_entry
        }

        // 跳过 `.` 和 `..` 这两个伪条目（pseudo entries）。
        const DOT: u16 = b'.' as u16;
        match &name[..] {
            [DOT] | [DOT, DOT] => self.next(),
            _ => Some((name, is_directory)),
        }
    }
}

unsafe fn from_maybe_unaligned<'a>(p: *const u16, len: usize) -> Cow<'a, [u16]> {
    unsafe {
        if p.is_aligned() {
            Cow::Borrowed(crate::slice::from_raw_parts(p, len))
        } else {
            Cow::Owned((0..len).map(|i| p.add(i).read_unaligned()).collect())
        }
    }
}

impl AsInner<Handle> for File {
    #[inline]
    fn as_inner(&self) -> &Handle {
        &self.handle
    }
}

impl IntoInner<Handle> for File {
    fn into_inner(self) -> Handle {
        self.handle
    }
}

impl FromInner<Handle> for File {
    fn from_inner(handle: Handle) -> File {
        File { handle }
    }
}

impl AsHandle for File {
    fn as_handle(&self) -> BorrowedHandle<'_> {
        self.as_inner().as_handle()
    }
}

impl AsRawHandle for File {
    fn as_raw_handle(&self) -> RawHandle {
        self.as_inner().as_raw_handle()
    }
}

impl IntoRawHandle for File {
    fn into_raw_handle(self) -> RawHandle {
        self.into_inner().into_raw_handle()
    }
}

impl FromRawHandle for File {
    unsafe fn from_raw_handle(raw_handle: RawHandle) -> Self {
        unsafe {
            Self { handle: FromInner::from_inner(FromRawHandle::from_raw_handle(raw_handle)) }
        }
    }
}

fn debug_path_handle<'a, 'b>(
    handle: BorrowedHandle<'a>,
    f: &'a mut fmt::Formatter<'b>,
    name: &str,
) -> fmt::DebugStruct<'a, 'b> {
    // FIXME(#24570): 在这里添加更多信息（例如 mode）
    let mut b = f.debug_struct(name);
    b.field("handle", &handle.as_raw_handle());
    if let Ok(path) = get_path(handle) {
        b.field("path", &path);
    }
    b
}

impl fmt::Debug for File {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut b = debug_path_handle(self.handle.as_handle(), f, "File");
        b.finish()
    }
}

impl FileAttr {
    pub fn size(&self) -> u64 {
        self.file_size
    }

    pub fn perm(&self) -> FilePermissions {
        FilePermissions { attrs: self.attributes }
    }

    pub fn attrs(&self) -> u32 {
        self.attributes
    }

    pub fn file_type(&self) -> FileType {
        FileType::new(self.attributes, self.reparse_tag)
    }

    pub fn modified(&self) -> io::Result<SystemTime> {
        Ok(SystemTime::from(self.last_write_time))
    }

    pub fn accessed(&self) -> io::Result<SystemTime> {
        Ok(SystemTime::from(self.last_access_time))
    }

    pub fn created(&self) -> io::Result<SystemTime> {
        Ok(SystemTime::from(self.creation_time))
    }

    pub fn modified_u64(&self) -> u64 {
        to_u64(&self.last_write_time)
    }

    pub fn accessed_u64(&self) -> u64 {
        to_u64(&self.last_access_time)
    }

    pub fn created_u64(&self) -> u64 {
        to_u64(&self.creation_time)
    }

    pub fn changed_u64(&self) -> Option<u64> {
        self.change_time.as_ref().map(|c| to_u64(c))
    }

    pub fn volume_serial_number(&self) -> Option<u32> {
        self.volume_serial_number
    }

    pub fn number_of_links(&self) -> Option<u32> {
        self.number_of_links
    }

    pub fn file_index(&self) -> Option<u64> {
        self.file_index
    }
}
impl From<c::WIN32_FIND_DATAW> for FileAttr {
    fn from(wfd: c::WIN32_FIND_DATAW) -> Self {
        FileAttr {
            attributes: wfd.dwFileAttributes,
            creation_time: wfd.ftCreationTime,
            last_access_time: wfd.ftLastAccessTime,
            last_write_time: wfd.ftLastWriteTime,
            change_time: None,
            file_size: ((wfd.nFileSizeHigh as u64) << 32) | (wfd.nFileSizeLow as u64),
            reparse_tag: if wfd.dwFileAttributes & c::FILE_ATTRIBUTE_REPARSE_POINT != 0 {
                // 除非这是一个重解析点（reparse point），否则该字段是保留的（reserved）
                wfd.dwReserved0
            } else {
                0
            },
            volume_serial_number: None,
            number_of_links: None,
            file_index: None,
        }
    }
}

fn to_u64(ft: &c::FILETIME) -> u64 {
    (ft.dwLowDateTime as u64) | ((ft.dwHighDateTime as u64) << 32)
}

impl FilePermissions {
    pub fn readonly(&self) -> bool {
        self.attrs & c::FILE_ATTRIBUTE_READONLY != 0
    }

    pub fn set_readonly(&mut self, readonly: bool) {
        if readonly {
            self.attrs |= c::FILE_ATTRIBUTE_READONLY;
        } else {
            self.attrs &= !c::FILE_ATTRIBUTE_READONLY;
        }
    }
}

impl FileTimes {
    pub fn set_accessed(&mut self, t: SystemTime) {
        self.accessed = Some(t.into_inner());
    }

    pub fn set_modified(&mut self, t: SystemTime) {
        self.modified = Some(t.into_inner());
    }

    pub fn set_created(&mut self, t: SystemTime) {
        self.created = Some(t.into_inner());
    }
}

impl FileType {
    fn new(attributes: u32, reparse_tag: u32) -> FileType {
        let is_directory = attributes & c::FILE_ATTRIBUTE_DIRECTORY != 0;
        let is_symlink = {
            let is_reparse_point = attributes & c::FILE_ATTRIBUTE_REPARSE_POINT != 0;
            let is_reparse_tag_name_surrogate = reparse_tag & 0x20000000 != 0;
            is_reparse_point && is_reparse_tag_name_surrogate
        };
        FileType { is_directory, is_symlink }
    }
    pub fn is_dir(&self) -> bool {
        !self.is_symlink && self.is_directory
    }
    pub fn is_file(&self) -> bool {
        !self.is_symlink && !self.is_directory
    }
    pub fn is_symlink(&self) -> bool {
        self.is_symlink
    }
    pub fn is_symlink_dir(&self) -> bool {
        self.is_symlink && self.is_directory
    }
    pub fn is_symlink_file(&self) -> bool {
        self.is_symlink && !self.is_directory
    }
}

impl DirBuilder {
    pub fn new() -> DirBuilder {
        DirBuilder
    }

    pub fn mkdir(&self, p: &Path) -> io::Result<()> {
        let p = maybe_verbatim(p)?;
        cvt(unsafe { c::CreateDirectoryW(p.as_ptr(), ptr::null_mut()) })?;
        Ok(())
    }
}

pub fn readdir(p: &Path) -> io::Result<ReadDir> {
    // 我们在路径末尾追加一个 `*`，这会导致空路径被当作当前目录处理。
    // 因此，为了与其他平台保持一致，我们对空路径显式地返回错误。
    if p.as_os_str().is_empty() {
        // 返回一个与其他打开文件方式一致的错误码。
        // 例如 fs::metadata 或 File::open。
        return Err(io::Error::from_raw_os_error(c::ERROR_PATH_NOT_FOUND as i32));
    }
    let root = p.to_path_buf();
    let star = p.join("*");
    let path = maybe_verbatim(&star)?;

    unsafe {
        let mut wfd: c::WIN32_FIND_DATAW = mem::zeroed();
        // 这类似于 FindFirstFileW（参见 https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-findfirstfileexw），
        // 但使用 FindExInfoBasic 时，它应当会跳过填充 WIN32_FIND_DATAW.cAlternateFileName
        // （参见 https://learn.microsoft.com/en-us/windows/win32/api/minwinbase/ns-minwinbase-win32_find_dataw）
        //（该字段总是 null 字符串值且当前未使用），因此应当更快。
        //
        // 我们可以向 dwAdditionalFlags 传入 FIND_FIRST_EX_LARGE_FETCH 来进一步加速，
        // 但由于我们不了解用户对该函数的使用习惯（use profile），还是保守一些为好。
        let find_handle = c::FindFirstFileExW(
            path.as_ptr(),
            c::FindExInfoBasic,
            &mut wfd as *mut _ as _,
            c::FindExSearchNameMatch,
            ptr::null(),
            0,
        );

        if find_handle != c::INVALID_HANDLE_VALUE {
            Ok(ReadDir {
                handle: Some(FindNextFileHandle(find_handle)),
                root: Arc::new(root),
                first: Some(wfd),
            })
        } else {
            // 如果找不到任何匹配的文件，`FindFirstFileExW` 函数会返回状态
            // `ERROR_FILE_NOT_FOUND`，但这并不一定意味着用于查找文件的那个路径不存在。
            //
            // 因此，当 Windows 返回的最后一个 os 错误是 `ERROR_FILE_NOT_FOUND` 时，
            // 我们加入了一项检查：检查待搜索的路径是否存在，以处理这种情形。
            // 如果确实如此，则返回一个空的 `ReadDir` 迭代器，因为它在首次调用 `.next()`
            // 时会返回 `None`——这是由于 `FindNextFileW` 函数本会返回 `ERROR_NO_MORE_FILES`。
            //
            // 参见 issue #120040：https://github.com/rust-lang/rust/issues/120040。
            let last_error = api::get_last_error();
            if last_error == WinError::FILE_NOT_FOUND {
                return Ok(ReadDir { handle: None, root: Arc::new(root), first: None });
            }

            // 如果不是上述情况，就直接返回由原始 OS 错误构造出来的错误。
            //
            // 注意：如果待搜索的路径本就不存在，`FindFirstFileExW` 函数本会返回
            // `ERROR_PATH_NOT_FOUND`。
            Err(Error::from_raw_os_error(last_error.code as i32))
        }
    }
}

pub fn unlink(path: &WCStr) -> io::Result<()> {
    if unsafe { c::DeleteFileW(path.as_ptr()) } == 0 {
        let err = api::get_last_error();
        // 如果 `DeleteFileW` 以 ERROR_ACCESS_DENIED 失败，则尝试在忽略只读属性
        // （readonly attribute）的情况下移除该文件。
        // 这是通过在一个已打开的文件句柄上调用 `posix_delete` 函数来实现的。
        if err == WinError::ACCESS_DENIED {
            let mut opts = OpenOptions::new();
            opts.access_mode(c::DELETE);
            opts.custom_flags(c::FILE_FLAG_OPEN_REPARSE_POINT);
            if let Ok(f) = File::open_native(&path, &opts) {
                if f.posix_delete().is_ok() {
                    return Ok(());
                }
            }
        }
        // 如果上述任何一步失败，则返回原始的错误。
        Err(io::Error::from_raw_os_error(err.code as i32))
    } else {
        Ok(())
    }
}

pub fn rename(old: &WCStr, new: &WCStr) -> io::Result<()> {
    if unsafe { c::MoveFileExW(old.as_ptr(), new.as_ptr(), c::MOVEFILE_REPLACE_EXISTING) } == 0 {
        let err = api::get_last_error();
        // 如果 `MoveFileExW` 以 ERROR_ACCESS_DENIED 失败，则尝试在忽略只读属性
        // （readonly attribute）的情况下移动该文件。
        // 这是通过用 `FileRenameInfoEx` 调用 `SetFileInformationByHandle` 来实现的。
        if err == WinError::ACCESS_DENIED {
            let mut opts = OpenOptions::new();
            opts.access_mode(c::DELETE);
            opts.custom_flags(c::FILE_FLAG_OPEN_REPARSE_POINT | c::FILE_FLAG_BACKUP_SEMANTICS);
            let Ok(f) = File::open_native(&old, &opts) else { return Err(err).io_result() };

            // 计算我们传给 `SetFileInformation` 的 `FILE_RENAME_INFO` 的内存布局（layout）。
            // 这是一个动态大小（dynamically sized）的结构体，因此我们需要获取最后一个字段的
            // 位置来计算实际大小。
            let Ok(new_len_without_nul_in_bytes): Result<u32, _> =
                ((new.count_bytes() - 1) * 2).try_into()
            else {
                return Err(err).io_result();
            };
            let offset: u32 = offset_of!(c::FILE_RENAME_INFO, FileName).try_into().unwrap();
            let struct_size = offset + new_len_without_nul_in_bytes + 2;
            let layout =
                Layout::from_size_align(struct_size as usize, align_of::<c::FILE_RENAME_INFO>())
                    .unwrap();

            let file_rename_info;
            // SAFETY: 我们为一个完整的 FILE_RENAME_INFO 结构体和一个文件名分配了足够的内存。
            unsafe {
                file_rename_info = alloc(layout).cast::<c::FILE_RENAME_INFO>();
                if file_rename_info.is_null() {
                    return Err(io::ErrorKind::OutOfMemory.into());
                }

                (&raw mut (*file_rename_info).Anonymous).write(c::FILE_RENAME_INFO_0 {
                    Flags: c::FILE_RENAME_FLAG_REPLACE_IF_EXISTS
                        | c::FILE_RENAME_FLAG_POSIX_SEMANTICS,
                });

                (&raw mut (*file_rename_info).RootDirectory).write(ptr::null_mut());
                // 大小中不包含 NULL
                (&raw mut (*file_rename_info).FileNameLength).write(new_len_without_nul_in_bytes);

                new.as_ptr().copy_to_nonoverlapping(
                    (&raw mut (*file_rename_info).FileName).cast::<u16>(),
                    new.count_bytes(),
                );
            }

            let result = unsafe {
                c::SetFileInformationByHandle(
                    f.as_raw_handle(),
                    c::FileRenameInfoEx,
                    file_rename_info.cast::<c_void>(),
                    struct_size,
                )
            };
            unsafe { dealloc(file_rename_info.cast::<u8>(), layout) };
            if result == 0 {
                if api::get_last_error() == WinError::DIR_NOT_EMPTY {
                    return Err(WinError::DIR_NOT_EMPTY).io_result();
                } else {
                    return Err(err).io_result();
                }
            }
        } else {
            return Err(err).io_result();
        }
    }
    Ok(())
}

pub fn rmdir(p: &WCStr) -> io::Result<()> {
    cvt(unsafe { c::RemoveDirectoryW(p.as_ptr()) })?;
    Ok(())
}

pub fn remove_dir_all(path: &WCStr) -> io::Result<()> {
    // 不跟随符号链接地打开一个文件或目录。
    let mut opts = OpenOptions::new();
    opts.access_mode(c::FILE_LIST_DIRECTORY);
    // `FILE_FLAG_BACKUP_SEMANTICS` 允许打开目录。
    // `FILE_FLAG_OPEN_REPARSE_POINT` 打开链接本身而不是其目标。
    opts.custom_flags(c::FILE_FLAG_BACKUP_SEMANTICS | c::FILE_FLAG_OPEN_REPARSE_POINT);
    let file = File::open_native(path, &opts)?;

    // 测试该文件是否既不是目录、也不是指向目录的符号链接。
    if (file.basic_info()?.FileAttributes & c::FILE_ATTRIBUTE_DIRECTORY) == 0 {
        return Err(io::Error::from_raw_os_error(c::ERROR_DIRECTORY as _));
    }

    // 删除该目录及其全部内容。
    remove_dir_all_iterative(file).io_result()
}

pub fn readlink(path: &WCStr) -> io::Result<PathBuf> {
    // 以无访问模式（而不是 generic read）打开该链接。
    // 默认情况下，对联接点（junction）"C:\Documents and Settings" 而言，
    // FILE_LIST_DIRECTORY 是被拒绝的，因此对于这一常见情形，需要这样做。
    let mut opts = OpenOptions::new();
    opts.access_mode(0);
    opts.custom_flags(c::FILE_FLAG_OPEN_REPARSE_POINT | c::FILE_FLAG_BACKUP_SEMANTICS);
    let file = File::open_native(&path, &opts)?;
    file.readlink()
}

pub fn symlink(original: &Path, link: &Path) -> io::Result<()> {
    symlink_inner(original, link, false)
}

pub fn symlink_inner(original: &Path, link: &Path, dir: bool) -> io::Result<()> {
    let original = to_u16s(original)?;
    let link = maybe_verbatim(link)?;
    let flags = if dir { c::SYMBOLIC_LINK_FLAG_DIRECTORY } else { 0 };
    // 以前，创建符号链接需要 SeCreateSymbolicLink 特权。在 Windows 10
    // Creators Update 中，Microsoft 放宽了这一限制：如果计算机处于开发者模式
    // （Developer Mode），允许无特权地创建符号链接，但必须把
    // SYMBOLIC_LINK_FLAG_ALLOW_UNPRIVILEGED_CREATE 加入到 dwFlags 中才能启用此行为。
    let result = cvt(unsafe {
        c::CreateSymbolicLinkW(
            link.as_ptr(),
            original.as_ptr(),
            flags | c::SYMBOLIC_LINK_FLAG_ALLOW_UNPRIVILEGED_CREATE,
        ) as c::BOOL
    });
    if let Err(err) = result {
        if err.raw_os_error() == Some(c::ERROR_INVALID_PARAMETER as i32) {
            // 较早版本的 Windows 不接受 SYMBOLIC_LINK_FLAG_ALLOW_UNPRIVILEGED_CREATE，
            // 因此如果遇到 ERROR_INVALID_PARAMETER，就去掉该标志后重试。
            cvt(unsafe {
                c::CreateSymbolicLinkW(link.as_ptr(), original.as_ptr(), flags) as c::BOOL
            })?;
        } else {
            return Err(err);
        }
    }
    Ok(())
}

#[cfg(not(target_vendor = "uwp"))]
pub fn link(original: &WCStr, link: &WCStr) -> io::Result<()> {
    cvt(unsafe { c::CreateHardLinkW(link.as_ptr(), original.as_ptr(), ptr::null_mut()) })?;
    Ok(())
}

#[cfg(target_vendor = "uwp")]
pub fn link(_original: &WCStr, _link: &WCStr) -> io::Result<()> {
    return Err(io::const_error!(io::ErrorKind::Unsupported, "hard link are not supported on UWP"));
}

pub fn stat(path: &WCStr) -> io::Result<FileAttr> {
    match metadata(path, ReparsePoint::Follow) {
        Err(err) if err.raw_os_error() == Some(c::ERROR_CANT_ACCESS_FILE as i32) => {
            if let Ok(attrs) = lstat(path) {
                if !attrs.file_type().is_symlink() {
                    return Ok(attrs);
                }
            }
            Err(err)
        }
        result => result,
    }
}

pub fn lstat(path: &WCStr) -> io::Result<FileAttr> {
    metadata(path, ReparsePoint::Open)
}

#[repr(u32)]
#[derive(Clone, Copy, PartialEq, Eq)]
enum ReparsePoint {
    Follow = 0,
    Open = c::FILE_FLAG_OPEN_REPARSE_POINT,
}
impl ReparsePoint {
    fn as_flag(self) -> u32 {
        self as u32
    }
}

fn metadata(path: &WCStr, reparse: ReparsePoint) -> io::Result<FileAttr> {
    let mut opts = OpenOptions::new();
    // 不需要读或写权限
    opts.access_mode(0);
    opts.custom_flags(c::FILE_FLAG_BACKUP_SEMANTICS | reparse.as_flag());

    // 尝试以正常方式打开该文件。
    // 如果那以 `ERROR_SHARING_VIOLATION` 失败，则改用 `FindFirstFileExW` 重试。
    // 如果该回退方案因任何原因失败，我们就返回原始的错误。
    match File::open_native(&path, &opts) {
        Ok(file) => file.file_attr(),
        Err(e)
            if [Some(c::ERROR_SHARING_VIOLATION as _), Some(c::ERROR_ACCESS_DENIED as _)]
                .contains(&e.raw_os_error()) =>
        {
            // 当用户对该资源没有权限时，会返回 `ERROR_ACCESS_DENIED`。
            // 一个例子是默认的 `System Volume Information`，不过这种文件也可以被创建。
            // `ERROR_SHARING_VIOLATION` 几乎永远不会被返回。
            // 通常即便一个文件被锁定，你仍然能读到一些元数据。
            // 然而，存在一些特殊的系统文件，例如 `C:\hiberfil.sys`，
            // 它们被锁定的方式甚至连这一点也不允许。
            unsafe {
                // `FindFirstFileExW` 接受带通配符（wildcard）的文件名。
                // 幸运的是，通配符不是合法的文件名，而且
                // `ERROR_SHARING_VIOLATION` 意味着该文件存在（但被锁定），
                // 因此可以安全地假定所给的文件名不含通配符。
                let mut wfd: c::WIN32_FIND_DATAW = mem::zeroed();
                let handle = c::FindFirstFileExW(
                    path.as_ptr(),
                    c::FindExInfoBasic,
                    &mut wfd as *mut _ as _,
                    c::FindExSearchNameMatch,
                    ptr::null(),
                    0,
                );

                if handle == c::INVALID_HANDLE_VALUE {
                    // 如果用户对该目录没有读访问权限，这可能会失败。
                    Err(e)
                } else {
                    // 我们不再需要这个 find 句柄了。
                    c::FindClose(handle);

                    // `FindFirstFileExW` 从目录中读取缓存的文件信息。
                    // 缺点是这些元数据可能已经过时（outdated）。
                    let attrs = FileAttr::from(wfd);
                    if reparse == ReparsePoint::Follow && attrs.file_type().is_symlink() {
                        Err(e)
                    } else {
                        Ok(attrs)
                    }
                }
            }
        }
        Err(e) => Err(e),
    }
}

pub fn set_perm(p: &WCStr, perm: FilePermissions) -> io::Result<()> {
    unsafe {
        cvt(c::SetFileAttributesW(p.as_ptr(), perm.attrs))?;
        Ok(())
    }
}

pub fn set_times(p: &WCStr, times: FileTimes) -> io::Result<()> {
    let mut opts = OpenOptions::new();
    opts.write(true);
    opts.custom_flags(c::FILE_FLAG_BACKUP_SEMANTICS);
    let file = File::open_native(p, &opts)?;
    file.set_times(times)
}

pub fn set_times_nofollow(p: &WCStr, times: FileTimes) -> io::Result<()> {
    let mut opts = OpenOptions::new();
    opts.write(true);
    // 用 `FILE_FLAG_OPEN_REPARSE_POINT` 来实现 no_follow（不跟随）行为
    opts.custom_flags(c::FILE_FLAG_BACKUP_SEMANTICS | c::FILE_FLAG_OPEN_REPARSE_POINT);
    let file = File::open_native(p, &opts)?;
    file.set_times(times)
}

fn get_path(f: impl AsRawHandle) -> io::Result<PathBuf> {
    fill_utf16_buf(
        |buf, sz| unsafe {
            c::GetFinalPathNameByHandleW(f.as_raw_handle(), buf, sz, c::VOLUME_NAME_DOS)
        },
        |buf| PathBuf::from(OsString::from_wide(buf)),
    )
}

pub fn canonicalize(p: &WCStr) -> io::Result<PathBuf> {
    let mut opts = OpenOptions::new();
    // 不需要读或写权限
    opts.access_mode(0);
    // 这个标志是为了让我们也能打开目录
    opts.custom_flags(c::FILE_FLAG_BACKUP_SEMANTICS);
    let f = File::open_native(p, &opts)?;
    get_path(f.handle)
}

pub fn copy(from: &WCStr, to: &WCStr) -> io::Result<u64> {
    unsafe extern "system" fn callback(
        _TotalFileSize: i64,
        _TotalBytesTransferred: i64,
        _StreamSize: i64,
        StreamBytesTransferred: i64,
        dwStreamNumber: u32,
        _dwCallbackReason: u32,
        _hSourceFile: c::HANDLE,
        _hDestinationFile: c::HANDLE,
        lpData: *const c_void,
    ) -> u32 {
        unsafe {
            if dwStreamNumber == 1 {
                *(lpData as *mut i64) = StreamBytesTransferred;
            }
            c::PROGRESS_CONTINUE
        }
    }
    let mut size = 0i64;
    cvt(unsafe {
        c::CopyFileExW(
            from.as_ptr(),
            to.as_ptr(),
            Some(callback),
            (&raw mut size) as *mut _,
            ptr::null_mut(),
            0,
        )
    })?;
    Ok(size as u64)
}

pub fn junction_point(original: &Path, link: &Path) -> io::Result<()> {
    // 一步到位地创建并打开一个新目录。
    let mut opts = OpenOptions::new();
    opts.create_new(true);
    opts.write(true);
    opts.custom_flags(c::FILE_FLAG_BACKUP_SEMANTICS | c::FILE_FLAG_POSIX_SEMANTICS);
    opts.attributes(c::FILE_ATTRIBUTE_DIRECTORY);

    let d = File::open(link, &opts)?;

    // 我们需要获取一个绝对的、NT 风格的路径。
    let path_bytes = original.as_os_str().as_encoded_bytes();
    let abs_path: Vec<u16> = if path_bytes.starts_with(br"\\?\") || path_bytes.starts_with(br"\??\")
    {
        // 它已经是一个绝对路径，我们只需要把前缀转换为 `\??\`
        let bytes = unsafe { OsStr::from_encoded_bytes_unchecked(&path_bytes[4..]) };
        r"\??\".encode_utf16().chain(bytes.encode_wide()).collect()
    } else {
        // 获取一个绝对路径，然后把前缀转换为 `\??\`
        let abs_path = crate::path::absolute(original)?.into_os_string().into_encoded_bytes();
        if abs_path.len() > 0 && abs_path[1..].starts_with(br":\") {
            let bytes = unsafe { OsStr::from_encoded_bytes_unchecked(&abs_path) };
            r"\??\".encode_utf16().chain(bytes.encode_wide()).collect()
        } else if abs_path.starts_with(br"\\.\") {
            let bytes = unsafe { OsStr::from_encoded_bytes_unchecked(&abs_path[4..]) };
            r"\??\".encode_utf16().chain(bytes.encode_wide()).collect()
        } else if abs_path.starts_with(br"\\") {
            let bytes = unsafe { OsStr::from_encoded_bytes_unchecked(&abs_path[2..]) };
            r"\??\UNC\".encode_utf16().chain(bytes.encode_wide()).collect()
        } else {
            return Err(io::const_error!(io::ErrorKind::InvalidInput, "path is not valid"));
        }
    };
    // 内联定义，这样我们就不必去摆弄变长缓冲区了。
    #[repr(C)]
    pub struct MountPointBuffer {
        ReparseTag: u32,
        ReparseDataLength: u16,
        Reserved: u16,
        SubstituteNameOffset: u16,
        SubstituteNameLength: u16,
        PrintNameOffset: u16,
        PrintNameLength: u16,
        PathBuffer: [MaybeUninit<u16>; c::MAXIMUM_REPARSE_DATA_BUFFER_SIZE as usize],
    }
    let data_len = 12 + (abs_path.len() * 2);
    if data_len > u16::MAX as usize {
        return Err(io::const_error!(io::ErrorKind::InvalidInput, "`original` path is too long"));
    }
    let data_len = data_len as u16;
    let mut header = MountPointBuffer {
        ReparseTag: c::IO_REPARSE_TAG_MOUNT_POINT,
        ReparseDataLength: data_len,
        Reserved: 0,
        SubstituteNameOffset: 0,
        SubstituteNameLength: (abs_path.len() * 2) as u16,
        PrintNameOffset: ((abs_path.len() + 1) * 2) as u16,
        PrintNameLength: 0,
        PathBuffer: [MaybeUninit::uninit(); c::MAXIMUM_REPARSE_DATA_BUFFER_SIZE as usize],
    };
    unsafe {
        let ptr = header.PathBuffer.as_mut_ptr();
        ptr.copy_from(abs_path.as_ptr().cast_uninit(), abs_path.len());

        let mut ret = 0;
        cvt(c::DeviceIoControl(
            d.as_raw_handle(),
            c::FSCTL_SET_REPARSE_POINT,
            (&raw const header).cast::<c_void>(),
            data_len as u32 + 8,
            ptr::null_mut(),
            0,
            &mut ret,
            ptr::null_mut(),
        ))
        .map(drop)
    }
}

// 尝试查看某个文件是否存在，但与 `exists` 不同，它会上报 I/O 错误。
pub fn exists(path: &WCStr) -> io::Result<bool> {
    // 打开该文件，以确保任何符号链接都被跟随到其目标。
    let mut opts = OpenOptions::new();
    // 不需要读、写等任何访问权限。
    opts.access_mode(0);
    // Backup 语义使得既能打开文件也能打开目录。
    opts.custom_flags(c::FILE_FLAG_BACKUP_SEMANTICS);
    match File::open_native(path, &opts) {
        Err(e) => match e.kind() {
            // 该文件确定不存在
            io::ErrorKind::NotFound => Ok(false),

            // `ERROR_SHARING_VIOLATION` 意味着该文件已被另一个进程锁定。
            // 这通常是暂时的，因此我们简单地把它上报为文件存在。
            _ if e.raw_os_error() == Some(c::ERROR_SHARING_VIOLATION as i32) => Ok(true),

            // `ERROR_CANT_ACCESS_FILE` 意味着文件存在，但 `CreateFile` 无法处理
            // 其重解析点（reparse point）。
            // 这可能发生在如下这类特殊文件上：
            // * Unix 域套接字（Unix domain sockets），你需要 `connect` 它
            // * 应用执行链接（App exec links），它需要使用 `CreateProcess`
            _ if e.raw_os_error() == Some(c::ERROR_CANT_ACCESS_FILE as i32) => Ok(true),

            // 其他错误，例如 `ERROR_ACCESS_DENIED`，可能表明该文件存在。
            // 然而，这类错误通常更为持久（permanent），因此我们在这里把它们上报出去。
            _ => Err(e),
        },
        // 该文件被成功打开，因此它必定存在，
        Ok(_) => Ok(true),
    }
}
