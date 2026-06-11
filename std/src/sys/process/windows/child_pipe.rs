use crate::io::{self, BorrowedCursor, IoSlice, IoSliceMut};
use crate::ops::Neg;
use crate::os::windows::prelude::*;
use crate::sys::handle::Handle;
use crate::sys::{FromInner, IntoInner, api, c};
use crate::{mem, ptr};

pub struct ChildPipe {
    inner: Handle,
}

impl IntoInner<Handle> for ChildPipe {
    fn into_inner(self) -> Handle {
        self.inner
    }
}

impl FromInner<Handle> for ChildPipe {
    fn from_inner(inner: Handle) -> ChildPipe {
        Self { inner }
    }
}

pub(super) struct Pipes {
    pub ours: ChildPipe,
    pub theirs: ChildPipe,
}

/// 创建一个适用于与子进程通信的匿名管道（anonymous pipe）。
///
/// 遗憾的是，Windows 没有办法在一个原本为同步操作而创建的句柄上执行异步操作。
/// 由于 `read_output` 只能用异步读取来正确实现，而 `CreatePipe` 创建的管道是同步的，
/// 我们无法用它（因而也无法用 [`io::pipe`]）来创建一个用于与子进程通信的管道。
/// 相反，本函数使用 NT API 来创建一个管道：其中一个管道句柄（`ours`）是异步的，
/// 另一个是同步的、可被子进程继承以用作控制台句柄（`theirs`）。
///
/// ours/theirs 这两个管道 *并非* 明确地分为可读或可写。它们各自只支持读或写之一，
/// 但谁是哪个取决于所给的布尔标志。如果 `ours_readable` 为 `true`，那么 `ours` 可读、
/// `theirs` 可写。反之，如果 `ours_readable` 为 `false`，那么 `ours` 可写、`theirs` 可读。
///
/// 还要注意，`ours` 管道始终是一个以重叠（overlapped）模式打开的句柄。这意味着严格来说
/// 它应当只与 `OVERLAPPED` 实例一起使用，但只要它每次只被使用一次（这一点我们确实有保证），
/// 也能正常运作。
// FIXME(joboet): 不，我们并没有保证这一点？例如 `&Stdout` 同时是 `Read` 和 `Sync`，
//                因此可能存在多个同时进行的操作。下面所有转发到内部句柄方法的函数，
//                如果被并发使用，都可能会 abort。
pub(super) fn child_pipe(ours_readable: bool, their_handle_inheritable: bool) -> io::Result<Pipes> {
    // 64kb 的管道容量与典型的 Linux 默认值相同。
    const PIPE_BUFFER_CAPACITY: u32 = 64 * 1024;

    // 注意，我们在这里特意 *不* 使用 `CreatePipe`，因为遗憾的是它返回的匿名管道
    // 不支持重叠（overlapped）操作。相反，我们使用 `NtCreateNamedPipeFile` 来创建
    // 一个带重叠支持的匿名管道。
    //
    // 一旦做完这一步，我们就通过 `NtOpenFile` 连接到它，然后返回这两个 reader/writer 半边。
    // 注意，返回值中的 `ours` 管道始终是命名管道（named pipe），而 `theirs` 只是普通文件。
    // 这有望屏蔽掉那些假定其 stdout 是命名管道的子进程——那种假定确实很古怪！
    unsafe {
        let mut io_status = c::IO_STATUS_BLOCK::default();
        let mut object_attributes = c::OBJECT_ATTRIBUTES::default();
        object_attributes.Length = size_of::<c::OBJECT_ATTRIBUTES>() as u32;

        // 打开一个指向管道文件系统（`\??\PIPE\`）的句柄。
        // 它将在创建新的匿名管道时使用。
        let pipe_fs = {
            let path = api::unicode_str!(r"\??\PIPE\");
            object_attributes.ObjectName = path.as_ptr();
            let mut pipe_fs = ptr::null_mut();
            let status = c::NtOpenFile(
                &mut pipe_fs,
                c::SYNCHRONIZE | c::GENERIC_READ,
                &object_attributes,
                &mut io_status,
                c::FILE_SHARE_READ | c::FILE_SHARE_WRITE,
                c::FILE_SYNCHRONOUS_IO_NONALERT, // 同步访问
            );
            if c::nt_success(status) {
                Handle::from_raw_handle(pipe_fs)
            } else {
                return Err(io::Error::from_raw_os_error(c::RtlNtStatusToDosError(status) as i32));
            }
        };

        // 从现在起，我们使用句柄而不是路径来创建和打开管道。
        // 因此把 `ObjectName` 设为一个零长度字符串。
        // 作为对 #143078 的一种（也许是过度谨慎的）缓解措施，对于空的 Buffer，
        // 我们使用空指针而不是 unicode_str!("")。
        // 这对操作系统本身没有区别，但有可能某些挂钩（hook）进进程的第三方 DLL
        // 会依赖于这个字符串的确切形式。
        let empty = c::UNICODE_STRING::default();
        object_attributes.ObjectName = &raw const empty;

        // 创建我们这一侧的管道，用于异步访问。
        let ours = {
            // 把管道文件系统用作根目录（root directory）。
            // 由于没有提供名称，将会创建一个匿名管道。
            object_attributes.RootDirectory = pipe_fs.as_raw_handle();

            // 负的超时值表示一段相对时间（而不是绝对时间）。
            // 时间以 100 纳秒为单位给出，所以这是 50 毫秒。
            // 选择这个值是为了与 `CreateNamedPipeW` 设置的默认超时保持一致
            // 参见：https://learn.microsoft.com/en-us/windows/win32/api/namedpipeapi/nf-namedpipeapi-createnamedpipew
            let timeout = (50_i64 * 10000).neg() as u64;

            let mut ours = ptr::null_mut();
            let status = c::NtCreateNamedPipeFile(
                &mut ours,
                c::SYNCHRONIZE | if ours_readable { c::GENERIC_READ } else { c::GENERIC_WRITE },
                &object_attributes,
                &mut io_status,
                if ours_readable { c::FILE_SHARE_WRITE } else { c::FILE_SHARE_READ },
                c::FILE_CREATE,
                0,
                c::FILE_PIPE_BYTE_STREAM_TYPE,
                c::FILE_PIPE_BYTE_STREAM_MODE,
                c::FILE_PIPE_QUEUE_OPERATION,
                // 只允许一个客户端管道
                1,
                PIPE_BUFFER_CAPACITY,
                PIPE_BUFFER_CAPACITY,
                &timeout,
            );
            if c::nt_success(status) {
                Handle::from_raw_handle(ours)
            } else {
                return Err(io::Error::from_raw_os_error(c::RtlNtStatusToDosError(status) as i32));
            }
        };

        // 打开他们那一侧的管道，用于同步访问。
        let theirs = {
            // 我们可以通过把 RootDirectory 设为该管道句柄、且不设置路径名，
            // 来重新打开（reopen）这个不带名称的匿名管道，
            object_attributes.RootDirectory = ours.as_raw_handle();

            if their_handle_inheritable {
                object_attributes.Attributes |= c::OBJ_INHERIT;
            }
            let mut theirs = ptr::null_mut();
            let status = c::NtOpenFile(
                &mut theirs,
                c::SYNCHRONIZE
                    | if ours_readable {
                        c::GENERIC_WRITE | c::FILE_READ_ATTRIBUTES
                    } else {
                        c::GENERIC_READ
                    },
                &object_attributes,
                &mut io_status,
                0,
                c::FILE_NON_DIRECTORY_FILE | c::FILE_SYNCHRONOUS_IO_NONALERT,
            );
            if c::nt_success(status) {
                Handle::from_raw_handle(theirs)
            } else {
                return Err(io::Error::from_raw_os_error(c::RtlNtStatusToDosError(status) as i32));
            }
        };

        Ok(Pipes { ours: ChildPipe { inner: ours }, theirs: ChildPipe { inner: theirs } })
    }
}

/// 接收一个异步的源管道，并返回一个适合发送给子进程的同步管道。
///
/// 这是通过创建一组新的管道、并 spawn 一个线程在源管道与同步管道之间转发消息来实现的。
pub(super) fn spawn_pipe_relay(
    source: &ChildPipe,
    ours_readable: bool,
    their_handle_inheritable: bool,
) -> io::Result<ChildPipe> {
    // 我们需要这个句柄在下面 spawn 的线程的整个生命周期内都存活。
    let source = source.try_clone()?;

    // 创建一对新的匿名管道。
    let Pipes { theirs, ours } = child_pipe(ours_readable, their_handle_inheritable)?;

    // spawn 一个线程，把消息从一个管道传递到另一个管道。
    // 任何错误都会直接导致该线程退出。
    let (reader, writer) = if ours_readable { (ours, source) } else { (source, ours) };
    crate::thread::spawn(move || {
        let mut buf = [0_u8; 4096];
        'reader: while let Ok(len) = reader.read(&mut buf) {
            if len == 0 {
                break;
            }
            let mut start = 0;
            while let Ok(written) = writer.write(&buf[start..len]) {
                start += written;
                if start == len {
                    continue 'reader;
                }
            }
            break;
        }
    });

    // 返回应当发送给子进程的那个管道。
    Ok(theirs)
}

impl ChildPipe {
    pub fn handle(&self) -> &Handle {
        &self.inner
    }
    pub fn into_handle(self) -> Handle {
        self.inner
    }

    pub fn try_clone(&self) -> io::Result<Self> {
        self.inner.duplicate(0, false, c::DUPLICATE_SAME_ACCESS).map(|inner| ChildPipe { inner })
    }

    pub fn read(&self, buf: &mut [u8]) -> io::Result<usize> {
        let result = unsafe {
            let len = crate::cmp::min(buf.len(), u32::MAX as usize) as u32;
            let ptr = buf.as_mut_ptr();
            self.alertable_io_internal(|overlapped, callback| {
                c::ReadFileEx(self.inner.as_raw_handle(), ptr, len, overlapped, callback)
            })
        };

        match result {
            // 对 BrokenPipe 的特殊处理是为了应对 Windows 的管道语义：
            // 当在另一端已关闭之后还从管道中 *读取* 时，会产生这个错误；
            // 我们把它解释为管道上的 EOF。
            Err(ref e) if e.kind() == io::ErrorKind::BrokenPipe => Ok(0),
            _ => result,
        }
    }

    pub fn read_buf(&self, mut buf: BorrowedCursor<'_>) -> io::Result<()> {
        let result = unsafe {
            let len = crate::cmp::min(buf.capacity(), u32::MAX as usize) as u32;
            let ptr = buf.as_mut().as_mut_ptr().cast::<u8>();
            self.alertable_io_internal(|overlapped, callback| {
                c::ReadFileEx(self.inner.as_raw_handle(), ptr, len, overlapped, callback)
            })
        };

        match result {
            // 对 BrokenPipe 的特殊处理是为了应对 Windows 的管道语义：
            // 当在另一端已关闭之后还从管道中 *读取* 时，会产生这个错误；
            // 我们把它解释为管道上的 EOF。
            Err(ref e) if e.kind() == io::ErrorKind::BrokenPipe => Ok(()),
            Err(e) => Err(e),
            Ok(n) => {
                unsafe {
                    buf.advance_unchecked(n);
                }
                Ok(())
            }
        }
    }

    pub fn read_vectored(&self, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        self.inner.read_vectored(bufs)
    }

    #[inline]
    pub fn is_read_vectored(&self) -> bool {
        self.inner.is_read_vectored()
    }

    pub fn read_to_end(&self, buf: &mut Vec<u8>) -> io::Result<usize> {
        self.handle().read_to_end(buf)
    }

    pub fn write(&self, buf: &[u8]) -> io::Result<usize> {
        unsafe {
            let len = crate::cmp::min(buf.len(), u32::MAX as usize) as u32;
            self.alertable_io_internal(|overlapped, callback| {
                c::WriteFileEx(self.inner.as_raw_handle(), buf.as_ptr(), len, overlapped, callback)
            })
        }
    }

    pub fn write_vectored(&self, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
        self.inner.write_vectored(bufs)
    }

    #[inline]
    pub fn is_write_vectored(&self) -> bool {
        self.inner.is_write_vectored()
    }

    /// 使用我们的匿名管道来同步异步的读取或写入。
    ///
    /// 它是对 [`ReadFileEx`] 或 [`WriteFileEx`] 的封装，借助
    /// [异步过程调用（Asynchronous Procedure Call）]（APC）来同步读取或写入。
    ///
    /// 注意：它不应被用于并非由我们创建的句柄。
    ///
    /// # 安全性(Safety）
    ///
    /// `buf` 必须是一个指向缓冲区的指针，且该缓冲区对至多 `len` 字节的读取或写入有效。
    /// `AlertableIoFn` 必须是 `ReadFileEx` 或 `WriteFileEx` 之一
    ///
    /// [`ReadFileEx`]: https://docs.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-readfileex
    /// [`WriteFileEx`]: https://docs.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-writefileex
    /// [Asynchronous Procedure Call]: https://docs.microsoft.com/en-us/windows/win32/sync/asynchronous-procedure-calls
    unsafe fn alertable_io_internal(
        &self,
        io: impl FnOnce(&mut c::OVERLAPPED, c::LPOVERLAPPED_COMPLETION_ROUTINE) -> c::BOOL,
    ) -> io::Result<usize> {
        // 使用 "alertable I/O"（可警示 I/O）来同步管道 I/O。
        // 它分为四个步骤。
        //
        // 步骤 1：启动异步 I/O 操作。
        //         这只是调用 `ReadFileEx` 或 `WriteFileEx` 之一，
        //         给它一个指向缓冲区的指针以及一个回调函数。
        //
        // 步骤 2：进入可警示（alertable）状态。
        //         步骤 1 中设置的回调要等到线程进入 "alertable" 状态后才会被调用。
        //         这可以用 `SleepEx` 来实现。
        //
        // 步骤 3：回调
        //         一旦 I/O 完成且线程处于可警示状态，该回调就会在与步骤 1 中调用
        //         `ReadFileEx` 或 `WriteFileEx` 相同的线程上运行。
        //         在回调中，我们只是设置该异步操作的结果。
        //
        // 步骤 4：返回结果。
        //         此时我们将从回调函数得到一个结果，直接返回它即可。注意，我们绝不能
        //         在更早的时候、即 I/O 仍在进行中时就返回。

        // 将由异步回调设置的结果。
        let mut async_result: Option<AsyncResult> = None;
        struct AsyncResult {
            error: u32,
            transferred: u32,
        }

        // 步骤 3：回调。
        unsafe extern "system" fn callback(
            error: u32,
            transferred: u32,
            overlapped: *mut c::OVERLAPPED,
        ) {
            // 使用一个通过 `hEvent` 偷渡（smuggle）进来的指针来设置 `async_result`。
            // SAFETY:
            // 此时，OVERLAPPED 结构体已经被操作系统写入过了，
            // 唯独我们的 `hEvent` 字段除外——我们把它设为了一个有效的 AsyncResult 指针（见下文）
            unsafe {
                let result = AsyncResult { error, transferred };
                *(*overlapped).hEvent.cast::<Option<AsyncResult>>() = Some(result);
            }
        }

        // 步骤 1：启动 I/O 操作。
        let mut overlapped: c::OVERLAPPED = unsafe { crate::mem::zeroed() };
        // `hEvent` 不被 `ReadFileEx` 和 `WriteFileEx` 使用。
        // 因此文档建议利用它来偷渡（smuggle）一个指向回调的指针。
        overlapped.hEvent = (&raw mut async_result) as *mut _;

        // 对管道进行异步读取。
        // 如果成功，`callback` 将在它完成后被调用一次。
        let result = io(&mut overlapped, Some(callback));
        if result == c::FALSE {
            // 我们可以在这里返回，因为该调用失败了。
            // 在此之后，在 I/O 完成之前我们绝不能返回。
            return Err(io::Error::last_os_error());
        }

        // 无限期地等待结果。
        let result = loop {
            // 步骤 2：进入可警示（alertable）状态。
            // `SleepEx` 的第二个参数用于使本次睡眠变为可警示的。
            unsafe { c::SleepEx(c::INFINITE, c::TRUE) };
            if let Some(result) = async_result {
                break result;
            }
        };
        // 步骤 4：返回结果。
        // 此时 `async_result` 始终为 `Some`
        match result.error {
            c::ERROR_SUCCESS => Ok(result.transferred as usize),
            error => Err(io::Error::from_raw_os_error(error as _)),
        }
    }
}

pub fn read_output(
    p1: ChildPipe,
    v1: &mut Vec<u8>,
    p2: ChildPipe,
    v2: &mut Vec<u8>,
) -> io::Result<()> {
    let p1 = p1.into_handle();
    let p2 = p2.into_handle();

    let mut p1 = AsyncPipe::new(p1, v1)?;
    let mut p2 = AsyncPipe::new(p2, v2)?;
    let objs = [p1.event.as_raw_handle(), p2.event.as_raw_handle()];

    // 在一个循环中，我们等待任一管道已调度的读取操作完成。
    // 如果该操作以 0 字节完成，那意味着到达了 EOF，在这种情况下，
    // 我们就把另一个管道彻底读完。
    //
    // 注意，重叠（overlapped）I/O 总体上极其不安全，因为我们必须小心确保参与其中的
    // 所有指针在整个 I/O 操作期间都有效（而在此期间还有大量操作可能失败）。
    // `AsyncPipe` 的析构函数最终会处理掉其中的大部分问题。
    loop {
        let res = unsafe { c::WaitForMultipleObjects(2, objs.as_ptr(), c::FALSE, c::INFINITE) };
        if res == c::WAIT_OBJECT_0 {
            if !p1.result()? || !p1.schedule_read()? {
                return p2.finish();
            }
        } else if res == c::WAIT_OBJECT_0 + 1 {
            if !p2.result()? || !p2.schedule_read()? {
                return p1.finish();
            }
        } else {
            return Err(io::Error::last_os_error());
        }
    }
}

struct AsyncPipe<'a> {
    pipe: Handle,
    event: Handle,
    overlapped: Box<c::OVERLAPPED>, // 需要一个稳定的地址
    dst: &'a mut Vec<u8>,
    state: State,
}

#[derive(PartialEq, Debug)]
enum State {
    NotReading,
    Reading,
    Read(usize),
}

impl<'a> AsyncPipe<'a> {
    fn new(pipe: Handle, dst: &'a mut Vec<u8>) -> io::Result<AsyncPipe<'a>> {
        // 创建一个事件（event），我们将用它来协调我们的重叠（overlapped）操作；
        // 这个事件将在 WaitForMultipleObjects 中使用，并作为 OVERLAPPED 句柄的一部分传入。
        //
        // 注意，我们在这里做了一件略带巧妙的事情：把该事件标记为手动重置（manually reset），
        // 并将其初始状态设为已触发（signaled）。这意味着对于刚创建的管道，我们会自然地
        // “穿过”上面的 WaitForMultipleObjects 调用；而事件唯一会回到 "unset"（未触发）
        // 状态的时刻，是在一次 I/O 操作被成功调度（schedule）之后（这正是我们想要的）。
        let event = Handle::new_event(true, true)?;
        let mut overlapped: Box<c::OVERLAPPED> = unsafe { Box::new(mem::zeroed()) };
        overlapped.hEvent = event.as_raw_handle();
        Ok(AsyncPipe { pipe, overlapped, event, dst, state: State::NotReading })
    }

    /// 执行一次重叠（overlapped）读取操作。
    ///
    /// 当前必须不在读取中；它返回该管道当前是否处于 EOF。如果管道不在 EOF，
    /// 那么之后必须调用 `result()` 来完成本次读取（可能会阻塞）；但如果管道处于 EOF，
    /// 则不应调用 `result()`，因为那只会永远阻塞下去。
    fn schedule_read(&mut self) -> io::Result<bool> {
        assert_eq!(self.state, State::NotReading);
        let amt = unsafe {
            if self.dst.capacity() == self.dst.len() {
                let additional = if self.dst.capacity() == 0 { 16 } else { 1 };
                self.dst.reserve(additional);
            }
            self.pipe.read_overlapped(self.dst.spare_capacity_mut(), &mut *self.overlapped)?
        };

        // 如果本次读取立即就完成了，那么我们的重叠（overlapped）事件将保持已触发状态
        //（它进入这里时就是已触发的），于是我们会继续往下走到下面的方法。
        //
        // 否则，I/O 操作已被调度，系统会把我们的事件设为未触发（not signaled），
        // 因此我们把自己标记进入读取（reading）状态并继续往下走。
        self.state = match amt {
            Some(0) => return Ok(false),
            Some(amt) => State::Read(amt),
            None => State::Reading,
        };
        Ok(true)
    }

    /// 等待先前执行的重叠（overlapped）操作的结果。
    ///
    /// 接收一个参数 `wait`，用于指示：当该管道当前正在被读取时，本函数是否应当
    /// 阻塞等待读取完成。
    ///
    /// 返回值：
    ///
    /// * `true`  —— 完成了任何挂起的读取，且管道未处于 EOF（继续进行）
    /// * `false` —— 完成了任何挂起的读取，且管道处于 EOF（停止发起读取）
    fn result(&mut self) -> io::Result<bool> {
        let amt = match self.state {
            State::NotReading => return Ok(true),
            State::Reading => self.pipe.overlapped_result(&mut *self.overlapped, true)?,
            State::Read(amt) => amt,
        };
        self.state = State::NotReading;
        unsafe {
            let len = self.dst.len();
            self.dst.set_len(len + amt);
        }
        Ok(amt != 0)
    }

    /// 把这个管道整个读完。
    ///
    /// 等待任何挂起的以及已调度的读取，然后在必要时调用 `read_to_end`
    /// 来读取所有剩余的信息。
    fn finish(&mut self) -> io::Result<()> {
        while self.result()? && self.schedule_read()? {
            // ...
        }
        Ok(())
    }
}

impl<'a> Drop for AsyncPipe<'a> {
    fn drop(&mut self) {
        let State::Reading = self.state else { return };

        // 如果我们有一个挂起（pending）的读取操作，那么在真正丢弃（drop）这个类型之前，
        // 我们必须确保它已经 *完成*。内核要求 `OVERLAPPED` 和缓冲区指针在整个 I/O 操作
        // 期间都保持有效。
        //
        // 为此，我们调用 `CancelIo` 来取消任何挂起的操作；如果成功，就等待重叠（overlapped）结果。
        //
        // 如果这里有任何一步失败，我们其实也没什么办法，因此我们泄漏（leak）掉
        // 缓冲区/OVERLAPPED 指针，以确保我们至少在内存上是安全的。
        if self.pipe.cancel_io().is_err() || self.result().is_err() {
            let buf = mem::take(self.dst);
            let overlapped = Box::new(unsafe { mem::zeroed() });
            let overlapped = mem::replace(&mut self.overlapped, overlapped);
            mem::forget((buf, overlapped));
        }
    }
}
