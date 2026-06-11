use crate::ffi::{OsString, c_char};
use crate::fmt;
use crate::fs::TryLockError;
use crate::hash::Hash;
use crate::io::{self, BorrowedCursor, IoSlice, IoSliceMut, SeekFrom};
use crate::path::{Path, PathBuf};
use crate::sys::helpers::run_path_with_cstr;
use crate::sys::time::SystemTime;
use crate::sys::{unsupported, unsupported_err};

#[expect(dead_code)]
#[path = "unsupported.rs"]
mod unsupported_fs;
pub use unsupported_fs::{
    DirBuilder, FileTimes, canonicalize, link, readlink, remove_dir_all, rename, rmdir, symlink,
    unlink,
};

/// VEXos 文件描述符。
///
/// 它存储一个由 VEXos 管理的、指向 [FatFs 文件对象结构体] 的不透明指针（opaque pointer），
/// 代表磁盘上一个已打开的文件。
///
/// [FatFs 文件对象结构体]: https://github.com/Xilinx/embeddedsw/blob/master/lib/sw_services/xilffs/src/include/ff.h?rgh-link-date=2025-09-23T20%3A03%3A43Z#L215
///
/// # 安全性(Safety）
///
/// 由于该平台使用的是一个指向内部文件系统结构体的指针、并且带有与之关联的生命周期
/// （而不是 UNIX 风格的文件描述符表），因此必须小心确保 `FileDesc` 所持有的指针
/// 在其存在期间始终有效。
#[derive(Debug)]
struct FileDesc(*mut vex_sdk::FIL);

// SAFETY: VEXos 的 FD 可以在并非创建它的那个线程上使用。
unsafe impl Send for FileDesc {}
// SAFETY: 我们假定环境中没有线程（即没有 RTOS）。
// （如果存在线程，则可能需要一个互斥锁（mutex）。）
unsafe impl Sync for FileDesc {}

pub struct File {
    fd: FileDesc,
}

#[derive(Clone)]
pub enum FileAttr {
    Dir,
    File { size: u64 },
}

pub struct ReadDir(!);

pub struct DirEntry {
    path: PathBuf,
}

#[derive(Clone, Debug)]
pub struct OpenOptions {
    read: bool,
    write: bool,
    append: bool,
    truncate: bool,
    create: bool,
    create_new: bool,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FilePermissions {}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct FileType {
    is_dir: bool,
}

impl FileAttr {
    pub fn size(&self) -> u64 {
        match self {
            Self::File { size } => *size,
            Self::Dir => 0,
        }
    }

    pub fn perm(&self) -> FilePermissions {
        FilePermissions {}
    }

    pub fn file_type(&self) -> FileType {
        FileType { is_dir: matches!(self, FileAttr::Dir) }
    }

    pub fn modified(&self) -> io::Result<SystemTime> {
        unsupported()
    }

    pub fn accessed(&self) -> io::Result<SystemTime> {
        unsupported()
    }

    pub fn created(&self) -> io::Result<SystemTime> {
        unsupported()
    }
}

impl FilePermissions {
    pub fn readonly(&self) -> bool {
        false
    }

    pub fn set_readonly(&mut self, _readonly: bool) {
        panic!("Permissions do not exist")
    }
}

impl FileType {
    pub fn is_dir(&self) -> bool {
        self.is_dir
    }

    pub fn is_file(&self) -> bool {
        !self.is_dir
    }

    pub fn is_symlink(&self) -> bool {
        // VEXos 中没有符号链接——条目要么是文件，要么是目录。
        false
    }
}

impl fmt::Debug for ReadDir {
    fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0
    }
}

impl Iterator for ReadDir {
    type Item = io::Result<DirEntry>;

    fn next(&mut self) -> Option<io::Result<DirEntry>> {
        self.0
    }
}

impl DirEntry {
    pub fn path(&self) -> PathBuf {
        self.path.clone()
    }

    pub fn file_name(&self) -> OsString {
        self.path.file_name().unwrap_or_default().into()
    }

    pub fn metadata(&self) -> io::Result<FileAttr> {
        stat(&self.path)
    }

    pub fn file_type(&self) -> io::Result<FileType> {
        Ok(self.metadata()?.file_type())
    }
}

impl OpenOptions {
    pub fn new() -> OpenOptions {
        OpenOptions {
            read: false,
            write: false,
            append: false,
            truncate: false,
            create: false,
            create_new: false,
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
}

impl File {
    pub fn open(path: &Path, opts: &OpenOptions) -> io::Result<File> {
        run_path_with_cstr(path, &|path| {
            // 强制保证 `create_new`/`create` 的不变量（invariants）。
            //
            // 由于 VEXos 没有任何类似 POSIX 的 `oflags` 的东西，我们需要自己来强制保证：
            // `create_new` 不能存在已有文件，且 `!create` 不会创建文件。
            if !opts.read && (opts.write || opts.append) && (opts.create_new || !opts.create) {
                let status = unsafe { vex_sdk::vexFileStatus(path.as_ptr()) };

                if opts.create_new && status != 0 {
                    return Err(io::const_error!(io::ErrorKind::AlreadyExists, "file exists",));
                } else if !opts.create && status == 0 {
                    return Err(io::const_error!(
                        io::ErrorKind::NotFound,
                        "no such file or directory",
                    ));
                }
            }

            let file = match opts {
                // read + write —— 不支持
                OpenOptions { read: true, write: true, .. } => {
                    return Err(io::const_error!(
                        io::ErrorKind::InvalidInput,
                        "opening files with read and write access is unsupported on this target",
                    ));
                }

                // read（只读）
                OpenOptions {
                    read: true,
                    write: false,
                    append: _,
                    truncate: false,
                    create: false,
                    create_new: false,
                } => unsafe { vex_sdk::vexFileOpen(path.as_ptr(), c"".as_ptr()) },

                // append（追加）
                OpenOptions {
                    read: false,
                    write: _,
                    append: true,
                    truncate: false,
                    create: _,
                    create_new: _,
                } => unsafe { vex_sdk::vexFileOpenWrite(path.as_ptr()) },

                // write（写入）
                OpenOptions {
                    read: false,
                    write: true,
                    append: false,
                    truncate,
                    create: _,
                    create_new: _,
                } => unsafe {
                    if *truncate {
                        vex_sdk::vexFileOpenCreate(path.as_ptr())
                    } else {
                        // 以追加模式打开，但跳转到文件的起始位置。
                        let fd = vex_sdk::vexFileOpenWrite(path.as_ptr());
                        vex_sdk::vexFileSeek(fd, 0, 0);
                        fd
                    }
                },

                _ => {
                    return Err(io::const_error!(io::ErrorKind::InvalidInput, "invalid argument"));
                }
            };

            if file.is_null() {
                Err(io::const_error!(io::ErrorKind::NotFound, "could not open file"))
            } else {
                Ok(Self { fd: FileDesc(file) })
            }
        })
    }

    pub fn file_attr(&self) -> io::Result<FileAttr> {
        // `vexFileSize` 出错时返回 -1，因此出错时 u64::try_from 会失败。
        if let Ok(size) = u64::try_from(unsafe {
            // SAFETY: 在该结构体的生命周期内，`self.fd` 中包含一个指向 `FIL` 的有效指针。
            vex_sdk::vexFileSize(self.fd.0)
        }) {
            Ok(FileAttr::File { size })
        } else {
            Err(io::const_error!(io::ErrorKind::InvalidData, "failed to get file size"))
        }
    }

    pub fn fsync(&self) -> io::Result<()> {
        self.flush()
    }

    pub fn datasync(&self) -> io::Result<()> {
        self.flush()
    }

    pub fn lock(&self) -> io::Result<()> {
        unsupported()
    }

    pub fn lock_shared(&self) -> io::Result<()> {
        unsupported()
    }

    pub fn try_lock(&self) -> Result<(), TryLockError> {
        Err(TryLockError::Error(unsupported_err()))
    }

    pub fn try_lock_shared(&self) -> Result<(), TryLockError> {
        Err(TryLockError::Error(unsupported_err()))
    }

    pub fn unlock(&self) -> io::Result<()> {
        unsupported()
    }

    pub fn truncate(&self, _size: u64) -> io::Result<()> {
        unsupported()
    }

    pub fn read(&self, buf: &mut [u8]) -> io::Result<usize> {
        let len = buf.len() as u32;
        let buf_ptr = buf.as_mut_ptr();
        let read = unsafe {
            // SAFETY: 在该结构体的生命周期内，`self.fd` 中包含一个指向 `FIL` 的有效指针。
            vex_sdk::vexFileRead(buf_ptr.cast::<c_char>(), 1, len, self.fd.0)
        };

        if read < 0 {
            Err(io::const_error!(io::ErrorKind::Other, "could not read from file"))
        } else {
            Ok(read as usize)
        }
    }

    pub fn read_vectored(&self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        crate::io::default_read_vectored(|b| self.read(b), bufs)
    }

    #[inline]
    pub fn is_read_vectored(&self) -> bool {
        false
    }

    pub fn read_buf(&self, cursor: BorrowedCursor<'_>) -> io::Result<()> {
        crate::io::default_read_buf(|b| self.read(b), cursor)
    }

    pub fn write(&self, buf: &[u8]) -> io::Result<usize> {
        let len = buf.len() as u32;
        let buf_ptr = buf.as_ptr();
        let written = unsafe {
            // SAFETY: 在该结构体的生命周期内，`self.fd` 中包含一个指向 `FIL` 的有效指针。
            vex_sdk::vexFileWrite(buf_ptr.cast_mut().cast::<c_char>(), 1, len, self.fd.0)
        };

        if written < 0 {
            Err(io::const_error!(io::ErrorKind::Other, "could not write to file"))
        } else {
            Ok(written as usize)
        }
    }

    pub fn write_vectored(&self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        crate::io::default_write_vectored(|b| self.write(b), bufs)
    }

    #[inline]
    pub fn is_write_vectored(&self) -> bool {
        false
    }

    pub fn flush(&self) -> io::Result<()> {
        unsafe {
            // SAFETY: 在该结构体的生命周期内，`self.fd` 中包含一个指向 `FIL` 的有效指针。
            vex_sdk::vexFileSync(self.fd.0);
        }
        Ok(())
    }

    pub fn tell(&self) -> io::Result<u64> {
        // SAFETY: 在该结构体的生命周期内，`self.fd` 中包含一个指向 `FIL` 的有效指针。
        let position = unsafe { vex_sdk::vexFileTell(self.fd.0) };

        position.try_into().map_err(|_| {
            io::const_error!(io::ErrorKind::InvalidData, "failed to get current location in file")
        })
    }

    pub fn size(&self) -> Option<io::Result<u64>> {
        None
    }

    pub fn seek(&self, pos: SeekFrom) -> io::Result<u64> {
        const SEEK_SET: i32 = 0;
        const SEEK_CUR: i32 = 1;
        const SEEK_END: i32 = 2;

        fn try_convert_offset<T: TryInto<u32>>(offset: T) -> io::Result<u32> {
            offset.try_into().map_err(|_| {
                io::const_error!(
                    io::ErrorKind::InvalidInput,
                    "cannot seek to an offset too large to fit in a 32 bit integer",
                )
            })
        }

        // SAFETY: 在该结构体的生命周期内，`self.fd` 中包含一个指向 `FIL` 的有效指针。
        match pos {
            SeekFrom::Start(offset) => unsafe {
                map_fresult(vex_sdk::vexFileSeek(self.fd.0, try_convert_offset(offset)?, SEEK_SET))?
            },
            SeekFrom::End(offset) => unsafe {
                if offset >= 0 {
                    map_fresult(vex_sdk::vexFileSeek(
                        self.fd.0,
                        try_convert_offset(offset)?,
                        SEEK_END,
                    ))?
                } else {
                    // `vexFileSeek` 不支持以负偏移量进行 seek，这意味着
                    // 我们必须自己从文件末尾计算偏移量。

                    // seek 到文件末尾，以获取打开缓冲区中的末尾位置。
                    map_fresult(vex_sdk::vexFileSeek(self.fd.0, 0, SEEK_END))?;
                    let end_position = self.tell()?;

                    map_fresult(vex_sdk::vexFileSeek(
                        self.fd.0,
                        // NOTE: 文件内部使用 32 位表示来记录流位置（stream position），
                        // 因此 `end_position as i64` 永远不会溢出。
                        try_convert_offset(end_position as i64 + offset)?,
                        SEEK_SET,
                    ))?
                }
            },
            SeekFrom::Current(offset) => unsafe {
                if offset >= 0 {
                    map_fresult(vex_sdk::vexFileSeek(
                        self.fd.0,
                        try_convert_offset(offset)?,
                        SEEK_CUR,
                    ))?
                } else {
                    // `vexFileSeek` 不支持以负偏移量进行 seek，这意味着
                    // 我们必须自己从当前流位置计算偏移量。
                    map_fresult(vex_sdk::vexFileSeek(
                        self.fd.0,
                        try_convert_offset((self.tell()? as i64) + offset)?,
                        SEEK_SET,
                    ))?
                }
            },
        }

        Ok(self.tell()?)
    }

    pub fn duplicate(&self) -> io::Result<File> {
        unsupported()
    }

    pub fn set_permissions(&self, _perm: FilePermissions) -> io::Result<()> {
        unsupported()
    }

    pub fn set_times(&self, _times: FileTimes) -> io::Result<()> {
        unsupported()
    }
}

impl fmt::Debug for File {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("File").field("fd", &self.fd.0).finish()
    }
}
impl Drop for File {
    fn drop(&mut self) {
        unsafe { vex_sdk::vexFileClose(self.fd.0) };
    }
}

pub fn readdir(_p: &Path) -> io::Result<ReadDir> {
    // 虽然 *确实* 存在一个用于读取文件目录的用户态函数，
    // 但目前还无法干净利落地完成必要的实现，因为
    // VEXos 不向用户程序暴露目录的长度。
    //
    // 这意味着我们将不得不创建一个很大的固定长度缓冲区，
    // 并寄希望于文件夹的内容不超过该缓冲区的长度，
    // 而这显然不是我们想在标准库中依赖的行为。
    unsupported()
}

pub fn set_perm(_p: &Path, _perm: FilePermissions) -> io::Result<()> {
    unsupported()
}

pub fn set_times(_p: &Path, _times: FileTimes) -> io::Result<()> {
    unsupported()
}

pub fn set_times_nofollow(_p: &Path, _times: FileTimes) -> io::Result<()> {
    unsupported()
}

pub fn exists(path: &Path) -> io::Result<bool> {
    run_path_with_cstr(path, &|path| Ok(unsafe { vex_sdk::vexFileStatus(path.as_ptr()) } != 0))
}

pub fn stat(p: &Path) -> io::Result<FileAttr> {
    // `vexFileStatus` 在给定路径是目录时返回 3，是文件时返回 1，
    // 路径不存在时返回 0。
    const FILE_STATUS_DIR: u32 = 3;

    run_path_with_cstr(p, &|c_path| {
        let file_type = unsafe { vex_sdk::vexFileStatus(c_path.as_ptr()) };

        // 如果它是目录，我们无法获取其大小，因为我们无法以文件的方式打开它
        if file_type == FILE_STATUS_DIR {
            Ok(FileAttr::Dir)
        } else {
            let mut opts = OpenOptions::new();
            opts.read(true);
            let file = File::open(p, &opts)?;
            file.file_attr()
        }
    })
}

pub fn lstat(p: &Path) -> io::Result<FileAttr> {
    // 此文件系统不支持符号链接
    stat(p)
}

// 这里不能使用 `common` 中的 `copy`，因为 `File::set_permissions` 在该目标平台上不受支持。
pub fn copy(from: &Path, to: &Path) -> io::Result<u64> {
    use crate::fs::File;

    // NOTE: 如果 `from` 是一个目录，由于 vexFileOpen* 会返回 null，本次调用应当失败。
    let mut reader = File::open(from)?;
    let mut writer = File::create(to)?;

    io::copy(&mut reader, &mut writer)
}

fn map_fresult(fresult: vex_sdk::FRESULT) -> io::Result<()> {
    // VEX 使用 FatFs 的一个衍生版本（Xilinx 的 xilffs 库）来执行文件系统操作。
    match fresult {
        vex_sdk::FRESULT::FR_OK => Ok(()),
        vex_sdk::FRESULT::FR_DISK_ERR => Err(io::const_error!(
            io::ErrorKind::Uncategorized,
            "internal function reported an unrecoverable hard error",
        )),
        vex_sdk::FRESULT::FR_INT_ERR => Err(io::const_error!(
            io::ErrorKind::Uncategorized,
            "internal error in filesystem runtime",
        )),
        vex_sdk::FRESULT::FR_NOT_READY => Err(io::const_error!(
            io::ErrorKind::Uncategorized,
            "the storage device could not be prepared to work",
        )),
        vex_sdk::FRESULT::FR_NO_FILE => Err(io::const_error!(
            io::ErrorKind::NotFound,
            "could not find the file in the directory"
        )),
        vex_sdk::FRESULT::FR_NO_PATH => Err(io::const_error!(
            io::ErrorKind::NotFound,
            "a directory in the path name could not be found",
        )),
        vex_sdk::FRESULT::FR_INVALID_NAME => Err(io::const_error!(
            io::ErrorKind::InvalidInput,
            "the given string is invalid as a path name",
        )),
        vex_sdk::FRESULT::FR_DENIED => Err(io::const_error!(
            io::ErrorKind::PermissionDenied,
            "the required access for this operation was denied",
        )),
        vex_sdk::FRESULT::FR_EXIST => Err(io::const_error!(
            io::ErrorKind::AlreadyExists,
            "an object with the same name already exists in the directory",
        )),
        vex_sdk::FRESULT::FR_INVALID_OBJECT => Err(io::const_error!(
            io::ErrorKind::Uncategorized,
            "invalid or null file/directory object",
        )),
        vex_sdk::FRESULT::FR_WRITE_PROTECTED => Err(io::const_error!(
            io::ErrorKind::PermissionDenied,
            "a write operation was performed on write-protected media",
        )),
        vex_sdk::FRESULT::FR_INVALID_DRIVE => Err(io::const_error!(
            io::ErrorKind::InvalidInput,
            "an invalid drive number was specified in the path name",
        )),
        vex_sdk::FRESULT::FR_NOT_ENABLED => Err(io::const_error!(
            io::ErrorKind::Uncategorized,
            "work area for the logical drive has not been registered",
        )),
        vex_sdk::FRESULT::FR_NO_FILESYSTEM => Err(io::const_error!(
            io::ErrorKind::Uncategorized,
            "valid FAT volume could not be found on the drive",
        )),
        vex_sdk::FRESULT::FR_MKFS_ABORTED => Err(io::const_error!(
            io::ErrorKind::Uncategorized,
            "failed to create filesystem volume"
        )),
        vex_sdk::FRESULT::FR_TIMEOUT => Err(io::const_error!(
            io::ErrorKind::TimedOut,
            "the function was canceled due to a timeout of thread-safe control",
        )),
        vex_sdk::FRESULT::FR_LOCKED => Err(io::const_error!(
            io::ErrorKind::Uncategorized,
            "the operation to the object was rejected by file sharing control",
        )),
        vex_sdk::FRESULT::FR_NOT_ENOUGH_CORE => {
            Err(io::const_error!(io::ErrorKind::OutOfMemory, "not enough memory for the operation"))
        }
        vex_sdk::FRESULT::FR_TOO_MANY_OPEN_FILES => Err(io::const_error!(
            io::ErrorKind::Uncategorized,
            "maximum number of open files has been reached",
        )),
        vex_sdk::FRESULT::FR_INVALID_PARAMETER => {
            Err(io::const_error!(io::ErrorKind::InvalidInput, "a given parameter was invalid"))
        }
        _ => unreachable!(), // C-style enum
    }
}
