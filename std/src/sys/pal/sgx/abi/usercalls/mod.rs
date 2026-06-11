use crate::cmp;
use crate::io::{self, BorrowedCursor, IoSlice, IoSliceMut};
use crate::random::random;
use crate::time::{Duration, Instant};

pub(crate) mod alloc;
#[macro_use]
pub(crate) mod raw;
#[cfg(test)]
mod tests;

use self::raw::*;

/// Usercall `read`。更多信息请参见 ABI 文档。
///
/// 它会执行一次 `read` usercall，并把读到的数据分散写入 `bufs`。若要读入单个
/// 缓冲区，只需传入一个长度为 1 的切片。
#[unstable(feature = "sgx_platform", issue = "56975")]
pub fn read(fd: Fd, bufs: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
    unsafe {
        let total_len = bufs.iter().fold(0usize, |sum, buf| sum.saturating_add(buf.len()));
        let mut userbuf = alloc::User::<[u8]>::uninitialized(total_len);
        let ret_len = raw::read(fd, userbuf.as_mut_ptr(), userbuf.len()).from_sgx_result()?;
        let userbuf = &userbuf[..ret_len];
        let mut index = 0;
        for buf in bufs {
            let end = cmp::min(index + buf.len(), userbuf.len());
            if let Some(buflen) = end.checked_sub(index) {
                userbuf[index..end].copy_to_enclave(&mut buf[..buflen]);
                index += buf.len();
            } else {
                break;
            }
        }
        Ok(userbuf.len())
    }
}

/// 使用未初始化缓冲区的 Usercall `read`。更多信息请参见 ABI 文档。
#[unstable(feature = "sgx_platform", issue = "56975")]
pub fn read_buf(fd: Fd, mut buf: BorrowedCursor<'_>) -> io::Result<()> {
    unsafe {
        let mut userbuf = alloc::User::<[u8]>::uninitialized(buf.capacity());
        let len = raw::read(fd, userbuf.as_mut_ptr().cast(), userbuf.len()).from_sgx_result()?;
        userbuf[..len].copy_to_enclave(&mut buf.as_mut()[..len]);
        buf.advance_unchecked(len);
        Ok(())
    }
}

/// Usercall `read_alloc`。更多信息请参见 ABI 文档。
#[unstable(feature = "sgx_platform", issue = "56975")]
pub fn read_alloc(fd: Fd) -> io::Result<Vec<u8>> {
    unsafe {
        let userbuf = ByteBuffer { data: crate::ptr::null_mut(), len: 0 };
        let mut userbuf = alloc::User::new_from_enclave(&userbuf);
        raw::read_alloc(fd, userbuf.as_raw_mut_ptr()).from_sgx_result()?;
        Ok(userbuf.copy_user_buffer())
    }
}

/// Usercall `write`。更多信息请参见 ABI 文档。
///
/// 它会执行一次 `write` usercall，并把要写入的数据从 `bufs` 中聚集起来。若要从
/// 单个缓冲区写出，只需传入一个长度为 1 的切片。
#[unstable(feature = "sgx_platform", issue = "56975")]
pub fn write(fd: Fd, bufs: &[IoSlice<'_>]) -> io::Result<usize> {
    unsafe {
        let total_len = bufs.iter().fold(0usize, |sum, buf| sum.saturating_add(buf.len()));
        let mut userbuf = alloc::User::<[u8]>::uninitialized(total_len);
        let mut index = 0;
        for buf in bufs {
            let end = cmp::min(index + buf.len(), userbuf.len());
            if let Some(buflen) = end.checked_sub(index) {
                userbuf[index..end].copy_from_enclave(&buf[..buflen]);
                index += buf.len();
            } else {
                break;
            }
        }
        raw::write(fd, userbuf.as_ptr(), userbuf.len()).from_sgx_result()
    }
}

/// Usercall `flush`。更多信息请参见 ABI 文档。
#[unstable(feature = "sgx_platform", issue = "56975")]
pub fn flush(fd: Fd) -> io::Result<()> {
    unsafe { raw::flush(fd).from_sgx_result() }
}

/// Usercall `close`。更多信息请参见 ABI 文档。
#[unstable(feature = "sgx_platform", issue = "56975")]
pub fn close(fd: Fd) {
    unsafe { raw::close(fd) }
}

fn string_from_bytebuffer(buf: &alloc::UserRef<ByteBuffer>, usercall: &str, arg: &str) -> String {
    String::from_utf8(buf.copy_user_buffer())
        .unwrap_or_else(|_| rtabort!("Usercall {usercall}: expected {arg} to be valid UTF-8"))
}

/// Usercall `bind_stream`。更多信息请参见 ABI 文档。
#[unstable(feature = "sgx_platform", issue = "56975")]
pub fn bind_stream(addr: &str) -> io::Result<(Fd, String)> {
    unsafe {
        let addr_user = alloc::User::new_from_enclave(addr.as_bytes());
        let mut local = alloc::User::<ByteBuffer>::uninitialized();
        let fd = raw::bind_stream(addr_user.as_ptr(), addr_user.len(), local.as_raw_mut_ptr())
            .from_sgx_result()?;
        let local = string_from_bytebuffer(&local, "bind_stream", "local_addr");
        Ok((fd, local))
    }
}

/// Usercall `accept_stream`。更多信息请参见 ABI 文档。
#[unstable(feature = "sgx_platform", issue = "56975")]
pub fn accept_stream(fd: Fd) -> io::Result<(Fd, String, String)> {
    unsafe {
        let mut bufs = alloc::User::<[ByteBuffer; 2]>::uninitialized();
        let mut buf_it = alloc::UserRef::iter_mut(&mut *bufs); // FIXME: 能否在不强制
        // 类型强转（coercion）的情况下做到这一点？
        let (local, peer) = (buf_it.next().unwrap(), buf_it.next().unwrap());
        let fd = raw::accept_stream(fd, local.as_raw_mut_ptr(), peer.as_raw_mut_ptr())
            .from_sgx_result()?;
        let local = string_from_bytebuffer(&local, "accept_stream", "local_addr");
        let peer = string_from_bytebuffer(&peer, "accept_stream", "peer_addr");
        Ok((fd, local, peer))
    }
}

/// Usercall `connect_stream`。更多信息请参见 ABI 文档。
#[unstable(feature = "sgx_platform", issue = "56975")]
pub fn connect_stream(addr: &str) -> io::Result<(Fd, String, String)> {
    unsafe {
        let addr_user = alloc::User::new_from_enclave(addr.as_bytes());
        let mut bufs = alloc::User::<[ByteBuffer; 2]>::uninitialized();
        let mut buf_it = alloc::UserRef::iter_mut(&mut *bufs); // FIXME: 能否在不强制
        // 类型强转（coercion）的情况下做到这一点？
        let (local, peer) = (buf_it.next().unwrap(), buf_it.next().unwrap());
        let fd = raw::connect_stream(
            addr_user.as_ptr(),
            addr_user.len(),
            local.as_raw_mut_ptr(),
            peer.as_raw_mut_ptr(),
        )
        .from_sgx_result()?;
        let local = string_from_bytebuffer(&local, "connect_stream", "local_addr");
        let peer = string_from_bytebuffer(&peer, "connect_stream", "peer_addr");
        Ok((fd, local, peer))
    }
}

/// Usercall `launch_thread`。更多信息请参见 ABI 文档。
#[unstable(feature = "sgx_platform", issue = "56975")]
pub unsafe fn launch_thread() -> io::Result<()> {
    // SAFETY: 调用方必须遵守 `launch_thread` 的安全契约。
    unsafe { raw::launch_thread().from_sgx_result() }
}

/// Usercall `exit`。更多信息请参见 ABI 文档。
#[unstable(feature = "sgx_platform", issue = "56975")]
pub fn exit(panic: bool) -> ! {
    unsafe { raw::exit(panic) }
}

/// Usercall `wait`。更多信息请参见 ABI 文档。
#[unstable(feature = "sgx_platform", issue = "56975")]
pub fn wait(event_mask: u64, mut timeout: u64) -> io::Result<u64> {
    if timeout != WAIT_NO && timeout != WAIT_INDEFINITE {
        // 我们不希望人们依赖超时的精确度来在 SGX enclave 中做出安全决策。这就是
        // 为什么我们会给超时值加上一个不超过 +/- 10% 的随机量，以打消人们依赖
        // 超时精确度的念头，同时又提供了一种让事情在其他情况下能正常工作的方式。
        // 注意，在 SGX 威胁模型中，负责服务 wait usercall 的 enclave runner 并
        // 不被信任去保证超时的准确性。
        if let Ok(timeout_signed) = i64::try_from(timeout) {
            let tenth = timeout_signed / 10;
            let deviation = random::<i64>(..).checked_rem(tenth).unwrap_or(0);
            timeout = timeout_signed.saturating_add(deviation) as _;
        }
    }
    unsafe { raw::wait(event_mask, timeout).from_sgx_result() }
}

/// 尽力等待一个非虚假（non-spurious）事件，等待时长至少不少于 `duration`。
///
/// 注意，总体而言，在 SGX 模型中对时间和超时的准确性没有任何保证。服务 usercall
/// 的 enclave runner 可能在当前时间上撒谎，并且/或者忽略超时值。
///
/// 一旦观察到该事件，将使用 `should_wake_up` 来判断该事件是否为虚假事件。
#[unstable(feature = "sgx_platform", issue = "56975")]
pub fn wait_timeout<F>(event_mask: u64, duration: Duration, should_wake_up: F)
where
    F: Fn() -> bool,
{
    // 调用 wait usercall 并检查结果。如果返回了事件则返回 true，如果返回的是
    // WouldBlock/TimedOut 则返回 false。
    // 如果 duration 为 None，它将使用 WAIT_NO。
    fn wait_checked(event_mask: u64, duration: Option<Duration>) -> bool {
        let timeout = duration.map_or(raw::WAIT_NO, |duration| {
            cmp::min((u64::MAX - 1) as u128, duration.as_nanos()) as u64
        });
        match wait(event_mask, timeout) {
            Ok(eventset) => {
                if event_mask == 0 {
                    rtabort!("expected wait() to return Err, found Ok.");
                }
                rtassert!(eventset != 0 && eventset & !event_mask == 0);
                true
            }
            Err(e) => {
                rtassert!(
                    e.kind() == io::ErrorKind::TimedOut || e.kind() == io::ErrorKind::WouldBlock
                );
                false
            }
        }
    }

    match wait_checked(event_mask, Some(duration)) {
        false => return,                    // 超时
        true if should_wake_up() => return, // 被唤醒
        true => {}                          // 虚假事件
    }

    // 排空所有已缓存的事件。
    // 注意，如果执行到这里，则隐含了 `event_mask != 0`。
    loop {
        match wait_checked(event_mask, None) {
            false => break,                     // 没有更多已缓存的事件
            true if should_wake_up() => return, // 被唤醒
            true => {}                          // 虚假事件
        }
    }

    // 继续等待，但记下已花费的等待时间，以免我们永远等下去。我们刻意不在此点之前
    // 调用 `Instant::now()`，以避免在没有虚假唤醒时仍要承担 `insecure_time`
    // usercall 的开销。

    let start = Instant::now();
    let mut remaining = duration;
    loop {
        match wait_checked(event_mask, Some(remaining)) {
            false => return,                    // 超时
            true if should_wake_up() => return, // 被唤醒
            true => {}                          // 虚假事件
        }
        remaining = match duration.checked_sub(start.elapsed()) {
            Some(remaining) => remaining,
            None => break,
        }
    }
}

/// Usercall `send`。更多信息请参见 ABI 文档。
#[unstable(feature = "sgx_platform", issue = "56975")]
pub fn send(event_set: u64, tcs: Option<Tcs>) -> io::Result<()> {
    unsafe { raw::send(event_set, tcs).from_sgx_result() }
}

/// Usercall `insecure_time`。更多信息请参见 ABI 文档。
#[unstable(feature = "sgx_platform", issue = "56975")]
pub fn insecure_time() -> Duration {
    let t = unsafe { raw::insecure_time().0 };
    Duration::new(t / 1_000_000_000, (t % 1_000_000_000) as _)
}

/// Usercall `alloc`。更多信息请参见 ABI 文档。
#[unstable(feature = "sgx_platform", issue = "56975")]
pub fn alloc(size: usize, alignment: usize) -> io::Result<*mut u8> {
    unsafe { raw::alloc(size, alignment).from_sgx_result() }
}

#[unstable(feature = "sgx_platform", issue = "56975")]
#[doc(inline)]
pub use self::raw::free;

fn check_os_error(err: Result) -> i32 {
    // FIXME: 不确定如何确保 Error 的所有变体都被覆盖到
    if err == Error::NotFound as _
        || err == Error::PermissionDenied as _
        || err == Error::ConnectionRefused as _
        || err == Error::ConnectionReset as _
        || err == Error::ConnectionAborted as _
        || err == Error::NotConnected as _
        || err == Error::AddrInUse as _
        || err == Error::AddrNotAvailable as _
        || err == Error::BrokenPipe as _
        || err == Error::AlreadyExists as _
        || err == Error::WouldBlock as _
        || err == Error::InvalidInput as _
        || err == Error::InvalidData as _
        || err == Error::TimedOut as _
        || err == Error::WriteZero as _
        || err == Error::Interrupted as _
        || err == Error::Other as _
        || err == Error::UnexpectedEof as _
        || ((Error::UserRangeStart as _)..=(Error::UserRangeEnd as _)).contains(&err)
    {
        err
    } else {
        rtabort!("Usercall: returned invalid error value {err}")
    }
}

/// 翻译 SGX usercall 的原始结果。
#[unstable(feature = "sgx_platform", issue = "56975")]
pub trait FromSgxResult {
    /// 返回类型
    type Return;

    /// 翻译 SGX usercall 的原始结果。
    fn from_sgx_result(self) -> io::Result<Self::Return>;
}

#[unstable(feature = "sgx_platform", issue = "56975")]
impl<T> FromSgxResult for (Result, T) {
    type Return = T;

    fn from_sgx_result(self) -> io::Result<Self::Return> {
        if self.0 == RESULT_SUCCESS {
            Ok(self.1)
        } else {
            Err(io::Error::from_raw_os_error(check_os_error(self.0)))
        }
    }
}

#[unstable(feature = "sgx_platform", issue = "56975")]
impl FromSgxResult for Result {
    type Return = ();

    fn from_sgx_result(self) -> io::Result<Self::Return> {
        if self == RESULT_SUCCESS {
            Ok(())
        } else {
            Err(io::Error::from_raw_os_error(check_os_error(self)))
        }
    }
}
