use crate::ffi::{CStr, OsStr, OsString, c_char};
use crate::fs::TryLockError;
use crate::io::{self, BorrowedCursor, Error, ErrorKind, IoSlice, IoSliceMut, SeekFrom};
use crate::os::hermit::ffi::OsStringExt;
use crate::os::hermit::hermit_abi::{
    self, DT_DIR, DT_LNK, DT_REG, DT_UNKNOWN, O_APPEND, O_CREAT, O_DIRECTORY, O_EXCL, O_RDONLY,
    O_RDWR, O_TRUNC, O_WRONLY, S_IFDIR, S_IFLNK, S_IFMT, S_IFREG, dirent64, stat as stat_struct,
};
use crate::os::hermit::io::{AsFd, AsRawFd, BorrowedFd, FromRawFd, IntoRawFd, RawFd};
use crate::path::{Path, PathBuf};
use crate::sync::Arc;
use crate::sys::fd::FileDesc;
pub use crate::sys::fs::common::{Dir, copy, exists};
use crate::sys::helpers::run_path_with_cstr;
use crate::sys::time::SystemTime;
use crate::sys::{AsInner, AsInnerMut, FromInner, IntoInner, cvt, unsupported, unsupported_err};
use crate::{fmt, mem};

#[derive(Debug)]
pub struct File(FileDesc);

#[derive(Clone)]
pub struct FileAttr {
    stat_val: stat_struct,
}

impl FileAttr {
    fn from_stat(stat_val: stat_struct) -> Self {
        Self { stat_val }
    }
}

// 所有 DirEntry 都会持有一个对该结构体的引用
struct InnerReadDir {
    root: PathBuf,
    dir: Vec<u8>,
}

impl InnerReadDir {
    pub fn new(root: PathBuf, dir: Vec<u8>) -> Self {
        Self { root, dir }
    }
}

pub struct ReadDir {
    inner: Arc<InnerReadDir>,
    pos: usize,
}

impl ReadDir {
    fn new(inner: InnerReadDir) -> Self {
        Self { inner: Arc::new(inner), pos: 0 }
    }
}

pub struct DirEntry {
    /// 指向该条目（entry）的路径
    root: PathBuf,
    /// 64 位 inode 编号
    ino: u64,
    /// 文件类型
    type_: u8,
    /// 该条目的名称
    name: OsString,
}

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
    mode: i32,
}

#[derive(Copy, Clone, Debug, Default)]
pub struct FileTimes {}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct FilePermissions {
    mode: u32,
}

#[derive(Copy, Clone, Eq, Debug)]
pub struct FileType {
    mode: u8,
}

impl PartialEq for FileType {
    fn eq(&self, other: &Self) -> bool {
        self.mode == other.mode
    }
}

impl core::hash::Hash for FileType {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.mode.hash(state);
    }
}

#[derive(Debug)]
pub struct DirBuilder {
    mode: u32,
}

impl FileAttr {
    pub fn modified(&self) -> io::Result<SystemTime> {
        Ok(SystemTime::new(self.stat_val.st_mtim.tv_sec, self.stat_val.st_mtim.tv_nsec))
    }

    pub fn accessed(&self) -> io::Result<SystemTime> {
        Ok(SystemTime::new(self.stat_val.st_atim.tv_sec, self.stat_val.st_atim.tv_nsec))
    }

    pub fn created(&self) -> io::Result<SystemTime> {
        Ok(SystemTime::new(self.stat_val.st_ctim.tv_sec, self.stat_val.st_ctim.tv_nsec))
    }

    pub fn size(&self) -> u64 {
        self.stat_val.st_size as u64
    }

    pub fn perm(&self) -> FilePermissions {
        FilePermissions { mode: self.stat_val.st_mode }
    }

    pub fn file_type(&self) -> FileType {
        let masked_mode = self.stat_val.st_mode & S_IFMT;
        let mode = match masked_mode {
            S_IFDIR => DT_DIR,
            S_IFLNK => DT_LNK,
            S_IFREG => DT_REG,
            _ => DT_UNKNOWN,
        };
        FileType { mode }
    }
}

impl FilePermissions {
    pub fn readonly(&self) -> bool {
        // 检查是否有任意一类（owner、group、others）拥有写权限
        self.mode & 0o222 == 0
    }

    pub fn set_readonly(&mut self, _readonly: bool) {
        unimplemented!()
    }

    #[allow(dead_code)]
    pub fn mode(&self) -> u32 {
        self.mode as u32
    }
}

impl FileTimes {
    pub fn set_accessed(&mut self, _t: SystemTime) {}
    pub fn set_modified(&mut self, _t: SystemTime) {}
}

impl FileType {
    pub fn is_dir(&self) -> bool {
        self.mode == DT_DIR
    }
    pub fn is_file(&self) -> bool {
        self.mode == DT_REG
    }
    pub fn is_symlink(&self) -> bool {
        self.mode == DT_LNK
    }
}

impl fmt::Debug for ReadDir {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // 它只会从 std::fs::ReadDir 中被调用，后者会添加一个 "ReadDir()" 帧。
        // 因此结果会是例如 'ReadDir("/home")' 这样的形式。
        fmt::Debug::fmt(&*self.inner.root, f)
    }
}

impl Iterator for ReadDir {
    type Item = io::Result<DirEntry>;

    fn next(&mut self) -> Option<io::Result<DirEntry>> {
        let mut counter: usize = 0;
        let mut offset: usize = 0;

        // 遍历所有目录条目，查找当前位置对应的条目
        loop {
            // 如果循环到达缓冲区末尾（即所有条目都遍历完了），就退出函数
            if offset >= self.inner.dir.len() {
                return None;
            }

            let dir = unsafe { &*(self.inner.dir.as_ptr().add(offset) as *const dirent64) };

            if counter == self.pos {
                self.pos += 1;

                // 在 dirent64 之后存放着文件名。d_reclen 表示 dirent64 的长度
                // 加上文件名的长度。因此，文件名的大小是 d_reclen 减去 dirent64
                // 的大小。文件名始终是一个 C 字符串，以 `\0` 结尾。
                // 因此，我们可以忽略最后一个字节。
                let name_bytes =
                    unsafe { CStr::from_ptr(&dir.d_name as *const _ as *const c_char).to_bytes() };
                let entry = DirEntry {
                    root: self.inner.root.clone(),
                    ino: dir.d_ino,
                    type_: dir.d_type,
                    name: OsString::from_vec(name_bytes.to_vec()),
                };

                return Some(Ok(entry));
            }

            counter += 1;

            // 移动到下一个 dirent64，它紧接着前一个之后存放
            offset = offset + usize::from(dir.d_reclen);
        }
    }
}

impl DirEntry {
    pub fn path(&self) -> PathBuf {
        self.root.join(self.file_name_os_str())
    }

    pub fn file_name(&self) -> OsString {
        self.file_name_os_str().to_os_string()
    }

    pub fn metadata(&self) -> io::Result<FileAttr> {
        let mut path = self.path();
        path.set_file_name(self.file_name_os_str());
        lstat(&path)
    }

    pub fn file_type(&self) -> io::Result<FileType> {
        Ok(FileType { mode: self.type_ })
    }

    #[allow(dead_code)]
    pub fn ino(&self) -> u64 {
        self.ino
    }

    pub fn file_name_os_str(&self) -> &OsStr {
        self.name.as_os_str()
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
            mode: 0o777,
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

    fn get_access_mode(&self) -> io::Result<i32> {
        match (self.read, self.write, self.append) {
            (true, false, false) => Ok(O_RDONLY),
            (false, true, false) => Ok(O_WRONLY),
            (true, true, false) => Ok(O_RDWR),
            (false, _, true) => Ok(O_WRONLY | O_APPEND),
            (true, _, true) => Ok(O_RDWR | O_APPEND),
            (false, false, false) => {
                Err(io::const_error!(ErrorKind::InvalidInput, "invalid access mode"))
            }
        }
    }

    fn get_creation_mode(&self) -> io::Result<i32> {
        match (self.write, self.append) {
            (true, false) => {}
            (false, false) => {
                if self.truncate || self.create || self.create_new {
                    return Err(io::const_error!(ErrorKind::InvalidInput, "invalid creation mode"));
                }
            }
            (_, true) => {
                if self.truncate && !self.create_new {
                    return Err(io::const_error!(ErrorKind::InvalidInput, "invalid creation mode"));
                }
            }
        }

        Ok(match (self.create, self.truncate, self.create_new) {
            (false, false, false) => 0,
            (true, false, false) => O_CREAT,
            (false, true, false) => O_TRUNC,
            (true, true, false) => O_CREAT | O_TRUNC,
            (_, _, true) => O_CREAT | O_EXCL,
        })
    }
}

impl File {
    pub fn open(path: &Path, opts: &OpenOptions) -> io::Result<File> {
        run_path_with_cstr(path, &|path| File::open_c(&path, opts))
    }

    pub fn open_c(path: &CStr, opts: &OpenOptions) -> io::Result<File> {
        let mut flags = opts.get_access_mode()?;
        flags = flags | opts.get_creation_mode()?;

        let mode;
        if flags & O_CREAT == O_CREAT {
            mode = opts.mode;
        } else {
            mode = 0;
        }

        let fd = unsafe { cvt(hermit_abi::open(path.as_ptr(), flags, mode))? };
        Ok(File(unsafe { FileDesc::from_raw_fd(fd as i32) }))
    }

    pub fn file_attr(&self) -> io::Result<FileAttr> {
        let mut stat_val: stat_struct = unsafe { mem::zeroed() };
        self.0.fstat(&mut stat_val)?;
        Ok(FileAttr::from_stat(stat_val))
    }

    pub fn fsync(&self) -> io::Result<()> {
        Err(Error::from_raw_os_error(22))
    }

    pub fn datasync(&self) -> io::Result<()> {
        self.fsync()
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
        Err(Error::from_raw_os_error(22))
    }

    pub fn read(&self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.read(buf)
    }

    pub fn read_vectored(&self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        self.0.read_vectored(bufs)
    }

    #[inline]
    pub fn is_read_vectored(&self) -> bool {
        self.0.is_read_vectored()
    }

    pub fn read_buf(&self, cursor: BorrowedCursor<'_>) -> io::Result<()> {
        self.0.read_buf(cursor)
    }

    pub fn write(&self, buf: &[u8]) -> io::Result<usize> {
        self.0.write(buf)
    }

    pub fn write_vectored(&self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        self.0.write_vectored(bufs)
    }

    #[inline]
    pub fn is_write_vectored(&self) -> bool {
        self.0.is_write_vectored()
    }

    #[inline]
    pub fn flush(&self) -> io::Result<()> {
        Ok(())
    }

    pub fn seek(&self, pos: SeekFrom) -> io::Result<u64> {
        self.0.seek(pos)
    }

    pub fn size(&self) -> Option<io::Result<u64>> {
        None
    }

    pub fn tell(&self) -> io::Result<u64> {
        self.0.tell()
    }

    pub fn duplicate(&self) -> io::Result<File> {
        Err(Error::from_raw_os_error(22))
    }

    pub fn set_permissions(&self, _perm: FilePermissions) -> io::Result<()> {
        Err(Error::from_raw_os_error(22))
    }

    pub fn set_times(&self, _times: FileTimes) -> io::Result<()> {
        Err(Error::from_raw_os_error(22))
    }
}

impl DirBuilder {
    pub fn new() -> DirBuilder {
        DirBuilder { mode: 0o777 }
    }

    pub fn mkdir(&self, path: &Path) -> io::Result<()> {
        run_path_with_cstr(path, &|path| {
            cvt(unsafe { hermit_abi::mkdir(path.as_ptr().cast(), self.mode.into()) }).map(|_| ())
        })
    }

    #[allow(dead_code)]
    pub fn set_mode(&mut self, mode: u32) {
        self.mode = mode;
    }
}

impl AsInner<FileDesc> for File {
    #[inline]
    fn as_inner(&self) -> &FileDesc {
        &self.0
    }
}

impl AsInnerMut<FileDesc> for File {
    #[inline]
    fn as_inner_mut(&mut self) -> &mut FileDesc {
        &mut self.0
    }
}

impl IntoInner<FileDesc> for File {
    fn into_inner(self) -> FileDesc {
        self.0
    }
}

impl FromInner<FileDesc> for File {
    fn from_inner(file_desc: FileDesc) -> Self {
        Self(file_desc)
    }
}

impl AsFd for File {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.0.as_fd()
    }
}

impl AsRawFd for File {
    #[inline]
    fn as_raw_fd(&self) -> RawFd {
        self.0.as_raw_fd()
    }
}

impl IntoRawFd for File {
    fn into_raw_fd(self) -> RawFd {
        self.0.into_raw_fd()
    }
}

impl FromRawFd for File {
    unsafe fn from_raw_fd(raw_fd: RawFd) -> Self {
        let file_desc = unsafe { FileDesc::from_raw_fd(raw_fd) };
        Self(file_desc)
    }
}

pub fn readdir(path: &Path) -> io::Result<ReadDir> {
    let fd_raw = run_path_with_cstr(path, &|path| {
        cvt(unsafe { hermit_abi::open(path.as_ptr(), O_RDONLY | O_DIRECTORY, 0) })
    })?;
    let fd = unsafe { FileDesc::from_raw_fd(fd_raw as i32) };
    let root = path.to_path_buf();

    // 读取所有目录条目
    let mut vec: Vec<u8> = Vec::new();
    let mut sz = 512;
    loop {
        // 预留内存以接收所有目录条目
        vec.resize(sz, 0);

        let readlen = unsafe {
            hermit_abi::getdents64(fd.as_raw_fd(), vec.as_mut_ptr() as *mut dirent64, sz)
        };
        if readlen > 0 {
            // 收缩到最小所需大小
            vec.resize(readlen.try_into().unwrap(), 0);
            break;
        }

        // 如果缓冲区太小，getdents64 返回 EINVAL
        // 否则，getdents64 返回一个错误码
        if readlen != (-hermit_abi::errno::EINVAL).into() {
            return Err(Error::from_raw_os_error(readlen.try_into().unwrap()));
        }

        // 内存不够 => 尝试增大向量的大小
        sz = sz * 2;

        // 用 1 MB 来容纳目录条目应该足够了
        // 在此处停止以避免死循环
        if sz > 0x100000 {
            return Err(Error::from(ErrorKind::Uncategorized));
        }
    }

    Ok(ReadDir::new(InnerReadDir::new(root, vec)))
}

pub fn unlink(path: &Path) -> io::Result<()> {
    run_path_with_cstr(path, &|path| cvt(unsafe { hermit_abi::unlink(path.as_ptr()) }).map(|_| ()))
}

pub fn rename(_old: &Path, _new: &Path) -> io::Result<()> {
    unsupported()
}

pub fn set_perm(_p: &Path, _perm: FilePermissions) -> io::Result<()> {
    Err(Error::from_raw_os_error(22))
}

pub fn set_times(_p: &Path, _times: FileTimes) -> io::Result<()> {
    Err(Error::from_raw_os_error(22))
}

pub fn set_times_nofollow(_p: &Path, _times: FileTimes) -> io::Result<()> {
    Err(Error::from_raw_os_error(22))
}

pub fn rmdir(path: &Path) -> io::Result<()> {
    run_path_with_cstr(path, &|path| cvt(unsafe { hermit_abi::rmdir(path.as_ptr()) }).map(|_| ()))
}

pub fn remove_dir_all(_path: &Path) -> io::Result<()> {
    unsupported()
}

pub fn readlink(_p: &Path) -> io::Result<PathBuf> {
    unsupported()
}

pub fn symlink(_original: &Path, _link: &Path) -> io::Result<()> {
    unsupported()
}

pub fn link(_original: &Path, _link: &Path) -> io::Result<()> {
    unsupported()
}

pub fn stat(path: &Path) -> io::Result<FileAttr> {
    run_path_with_cstr(path, &|path| {
        let mut stat_val: stat_struct = unsafe { mem::zeroed() };
        cvt(unsafe { hermit_abi::stat(path.as_ptr(), &mut stat_val) })?;
        Ok(FileAttr::from_stat(stat_val))
    })
}

pub fn lstat(path: &Path) -> io::Result<FileAttr> {
    run_path_with_cstr(path, &|path| {
        let mut stat_val: stat_struct = unsafe { mem::zeroed() };
        cvt(unsafe { hermit_abi::lstat(path.as_ptr(), &mut stat_val) })?;
        Ok(FileAttr::from_stat(stat_val))
    })
}

pub fn canonicalize(_p: &Path) -> io::Result<PathBuf> {
    unsupported()
}
