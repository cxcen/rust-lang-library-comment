use core::ffi::c_void;

use crate::ffi::CStr;
use crate::num::NonZero;
use crate::os::windows::io::{AsRawHandle, HandleOrNull};
use crate::sys::handle::Handle;
use crate::sys::pal::time::WaitableTimer;
use crate::sys::pal::{dur2timeout, to_u16s};
use crate::sys::{FromInner, c, stack_overflow};
use crate::thread::ThreadInit;
use crate::time::Duration;
use crate::{io, ptr};

pub const DEFAULT_MIN_STACK_SIZE: usize = 2 * 1024 * 1024;

pub struct Thread {
    handle: Handle,
}

impl Thread {
    // unsafe：安全性要求参见 thread::Builder::spawn_unchecked
    #[cfg_attr(miri, track_caller)] // 即使没有 panic，这也有助于 Miri 的 backtrace
    pub unsafe fn new(stack: usize, init: Box<ThreadInit>) -> io::Result<Thread> {
        let data = Box::into_raw(init);

        // CreateThread 会把栈大小的值向上取整到最近的页大小（至少 4kb）。
        // 如果给定的值为零，则改用默认栈大小。
        // SAFETY：`thread_start` 具有适用于线程入口点的正确 ABI。
        // `data` 只是被原样传递给新线程，不会被改动。
        let ret = unsafe {
            let ret = c::CreateThread(
                ptr::null_mut(),
                stack,
                Some(thread_start),
                data as *mut _,
                c::STACK_SIZE_PARAM_IS_A_RESERVATION,
                ptr::null_mut(),
            );
            HandleOrNull::from_raw_handle(ret)
        };
        return if let Ok(handle) = ret.try_into() {
            Ok(Thread { handle: Handle::from_inner(handle) })
        } else {
            // 线程启动失败，因此 data 没有被消费掉。所以重新构造这个
            // box 以便它被释放是安全的。
            unsafe { drop(Box::from_raw(data)) };
            Err(io::Error::last_os_error())
        };

        unsafe extern "system" fn thread_start(data: *mut c_void) -> u32 {
            // SAFETY：我们只是在重建先前被泄漏的 box。
            let init = unsafe { Box::from_raw(data as *mut ThreadInit) };
            let rust_start = init.init();

            // 预留一些栈空间，以备在其他情况下耗尽栈时使用。
            stack_overflow::reserve_stack();

            rust_start();
            0
        }
    }

    pub fn join(self) {
        let rc = unsafe { c::WaitForSingleObject(self.handle.as_raw_handle(), c::INFINITE) };
        if rc == c::WAIT_FAILED {
            panic!("failed to join on thread: {}", io::Error::last_os_error());
        }
    }

    pub fn handle(&self) -> &Handle {
        &self.handle
    }

    pub fn into_handle(self) -> Handle {
        self.handle
    }
}

pub fn available_parallelism() -> io::Result<NonZero<usize>> {
    let res = unsafe {
        let mut sysinfo: c::SYSTEM_INFO = crate::mem::zeroed();
        c::GetSystemInfo(&mut sysinfo);
        sysinfo.dwNumberOfProcessors as usize
    };
    match res {
        0 => Err(io::Error::UNKNOWN_THREAD_COUNT),
        cpus => Ok(unsafe { NonZero::new_unchecked(cpus) }),
    }
}

pub fn current_os_id() -> Option<u64> {
    // SAFETY：无前置条件的 FFI 调用。
    let id: u32 = unsafe { c::GetCurrentThreadId() };

    // 返回值为 0 表示查询失败。
    if id == 0 { None } else { Some(id.into()) }
}

pub fn set_name(name: &CStr) {
    if let Ok(utf8) = name.to_str() {
        if let Ok(utf16) = to_u16s(utf8) {
            unsafe {
                // SAFETY：`to_u16s` 返回的 vec 以一个零值结尾
                set_name_wide(&utf16)
            }
        };
    };
}

/// # 安全性(Safety）
///
/// `name` 必须以一个零值结尾
pub unsafe fn set_name_wide(name: &[u16]) {
    unsafe { c::SetThreadDescription(c::GetCurrentThread(), name.as_ptr()) };
}

pub fn sleep(dur: Duration) {
    fn high_precision_sleep(dur: Duration) -> Result<(), ()> {
        let timer = WaitableTimer::high_resolution()?;
        timer.set(dur)?;
        timer.wait()
    }
    // 尝试使用高精度睡眠（Windows 10，version 1803+）。
    // 出错时回退到标准的 `Sleep` 函数。
    // 同时也保留了 `Sleep` 对零时长的处理行为。
    if dur.is_zero() || high_precision_sleep(dur).is_err() {
        unsafe { c::Sleep(dur2timeout(dur)) }
    }
}

pub fn yield_now() {
    // 如果没有其他线程可执行，此函数会返回 0；
    // 但这同时也意味着此次让出（yield）是无用的，所以这并不是一个
    // 真正需要担心的情形。
    unsafe {
        c::SwitchToThread();
    }
}
