//! std::fs::remove_dir_all 的 Windows 实现。
//!
//! 它需要解决两个问题：
//!
//! - 必须无法被诱导去删除父目录之外的文件（参见 CVE-2022-21658）。
//! - 当多个线程或进程对同一路径调用 `remove_dir_all` 时，它不应失败。
//!
//! 第一个问题通过使用底层的 `NtOpenFile` API 相对于父目录来打开文件来处理。
//!
//! 第二个问题更为棘手。删除文件的方式是把它的 "disposition"（处置状态）设为删除。
//! 然而文件直到被关闭之前才会真正被删除。在这两个事件之间的间隙里，文件处于一种
//! 半生不死（limbo）的状态：它仍然存在于文件系统中，但任何试图打开它的操作都会
//! 以错误失败。
//!
//! 我们在这里采用的缓解措施是：
//!
//! - 在尝试打开文件时，我们将 ERROR_DELETE_PENDING 视为一次成功的删除。
//! - 如果到我们尝试删除父目录时该文件仍未从文件系统中移除，我们会尝试等待它完成。
//!   但我们无法无限期等待，因此在自旋若干次之后，我们就放弃并返回一个错误。
//!
//! 简而言之，在发生竞态（race）时我们无法保证这总会成功，但我们会尽最大努力使其
//! *应该* 成功。

use core::ptr;
use core::sync::atomic::{Atomic, AtomicU32, Ordering};

use super::{AsRawHandle, DirBuff, File, FromRawHandle};
use crate::sys::c;
use crate::sys::pal::api::{UnicodeStrRef, WinError, unicode_str};
use crate::thread;

// 等待删除完成时自旋的最大次数。
const MAX_RETRIES: usize = 50;

/// 对底层 NtOpenFile 调用的封装。
///
/// 它并不完全安全，因为 `OBJECT_ATTRIBUTES` 中包含裸指针。
unsafe fn nt_open_file(
    access: u32,
    object_attribute: &c::OBJECT_ATTRIBUTES,
    share: u32,
    options: u32,
) -> Result<File, WinError> {
    unsafe {
        let mut handle = ptr::null_mut();
        let mut io_status = c::IO_STATUS_BLOCK::PENDING;
        let status =
            c::NtOpenFile(&mut handle, access, object_attribute, &mut io_status, share, options);
        if c::nt_success(status) {
            Ok(File::from_raw_handle(handle))
        } else {
            // 将 NTSTATUS 转换为更熟悉的 Win32 错误码（也即 "DosError"）
            let win_error = if status == c::STATUS_DELETE_PENDING {
                // 我们对 `STATUS_DELETE_PENDING` 做特殊处理，因为否则它会被映射为
                // `ERROR_ACCESS_DENIED`，那非常没有帮助——因为后者也可能表示一个权限错误。
                WinError::DELETE_PENDING
            } else {
                WinError::new(c::RtlNtStatusToDosError(status))
            };
            Err(win_error)
        }
    }
}

/// 在目录 `parent` 中打开文件 `path`，请求给定的 `access` 访问权限。
/// `options` 会与 `FILE_OPEN_REPARSE_POINT` 做按位或。
fn open_link_no_reparse(
    parent: &File,
    path: UnicodeStrRef<'_>,
    access: u32,
    options: u32,
) -> Result<Option<File>, WinError> {
    // 这里使用更底层的 `NtOpenFile` 函数实现，因为遗憾的是 win32 函数不支持
    // 相对于父目录来打开文件。
    //
    // 参见 https://learn.microsoft.com/windows/win32/api/winternl/nf-winternl-ntopenfile

    // `OBJ_DONT_REPARSE` 属性确保我们不会被诱导去跟随某个符号链接。不过它在
    // 较早版本的 Windows 上可能不可用。
    static ATTRIBUTES: Atomic<u32> = AtomicU32::new(c::OBJ_DONT_REPARSE);

    let result = unsafe {
        let mut object = c::OBJECT_ATTRIBUTES {
            ObjectName: path.as_ptr(),
            RootDirectory: parent.as_raw_handle(),
            Attributes: ATTRIBUTES.load(Ordering::Relaxed),
            ..c::OBJECT_ATTRIBUTES::with_length()
        };
        let share = c::FILE_SHARE_DELETE | c::FILE_SHARE_READ | c::FILE_SHARE_WRITE;
        let options = c::FILE_OPEN_REPARSE_POINT | options;
        let result = nt_open_file(access, &object, share, options);

        // 如果不支持 OBJ_DONT_REPARSE，则去掉它后重试。
        if matches!(result, Err(WinError::INVALID_PARAMETER))
            && ATTRIBUTES.load(Ordering::Relaxed) == c::OBJ_DONT_REPARSE
        {
            ATTRIBUTES.store(0, Ordering::Relaxed);
            object.Attributes = 0;
            nt_open_file(access, &object, share, options)
        } else {
            result
        }
    };

    // 忽略 not found（未找到）错误
    match result {
        Ok(f) => Ok(Some(f)),
        Err(
            WinError::FILE_NOT_FOUND
            | WinError::PATH_NOT_FOUND
            | WinError::BAD_NETPATH
            | WinError::BAD_NET_NAME
            // `DELETE_PENDING` 意味着已经有别的东西在尝试删除它，
            // 因此我们假定那最终会成功。
            | WinError::DELETE_PENDING,
        ) => Ok(None),
        Err(e) => Err(e),
    }
}

fn open_dir(parent: &File, name: UnicodeStrRef<'_>) -> Result<Option<File>, WinError> {
    // 打开目录以进行同步的目录列举。
    open_link_no_reparse(
        parent,
        name,
        c::SYNCHRONIZE | c::FILE_LIST_DIRECTORY,
        // "_IO_NONALERT" 表示同步调用不会被中断。
        c::FILE_SYNCHRONOUS_IO_NONALERT,
    )
}

fn delete(parent: &File, name: UnicodeStrRef<'_>) -> Result<(), WinError> {
    // 注意 `delete` 函数会消耗（consume）打开的文件，以确保它被立即丢弃（dropped）。
    // 关于为何这一点很重要，参见模块注释。
    match open_link_no_reparse(parent, name, c::DELETE, 0) {
        Ok(Some(f)) => f.delete(),
        Ok(None) => Ok(()),
        Err(e) => Err(e),
    }
}

/// 一个简单的重试循环：当 `f` 以给定的错误码失败时持续运行它，
/// 直到达到 `MAX_RETRIES` 为止。
fn retry<T: PartialEq>(
    mut f: impl FnMut() -> Result<T, WinError>,
    ignore: WinError,
) -> Result<T, WinError> {
    let mut i = MAX_RETRIES;
    loop {
        i -= 1;
        if i == 0 {
            return f();
        } else {
            let result = f();
            if result != Err(ignore) {
                return result;
            }
        }
        thread::yield_now();
    }
}

pub fn remove_dir_all_iterative(dir: File) -> Result<(), WinError> {
    let mut buffer = DirBuff::new();
    let mut dirlist = vec![dir];

    let mut restart = true;
    'outer: while let Some(dir) = dirlist.pop() {
        let more_data = dir.fill_dir_buff(&mut buffer, restart)?;
        for (name, is_directory) in buffer.iter() {
            let name = unicode_str!(&name);
            if is_directory {
                let Some(subdir) = open_dir(&dir, name)? else { continue };
                dirlist.push(dir);
                dirlist.push(subdir);
                continue 'outer;
            } else {
                // 尝试删除，在共享冲突（sharing violation）错误时重试，因为这类错误
                // 往往非常短暂。例如某个东西释放文件句柄所需的时间比预期稍长一点。
                retry(|| delete(&dir, name), WinError::SHARING_VIOLATION)?;
            }
        }
        if more_data {
            dirlist.push(dir);
            restart = false;
        } else {
            // 尝试删除，在目录非空（not empty）错误时重试，因为我们可能需要
            // 等待一段时间，等文件从文件系统中被移除。
            let name = unicode_str!("");
            retry(|| delete(&dir, name), WinError::DIR_NOT_EMPTY)?;
            restart = true;
        }
    }
    Ok(())
}
