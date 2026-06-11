use core::arch::asm;

use crate::io;
use crate::num::NonZero;
use crate::os::xous::ffi::{
    MemoryFlags, Syscall, ThreadId, blocking_scalar, create_thread, do_yield, join_thread,
    map_memory, update_memory_flags,
};
use crate::os::xous::services::{TicktimerScalar, ticktimer_server};
use crate::thread::ThreadInit;
use crate::time::Duration;

pub struct Thread {
    tid: ThreadId,
}

pub const DEFAULT_MIN_STACK_SIZE: usize = 131072;
const MIN_STACK_SIZE: usize = 4096;
pub const GUARD_PAGE_SIZE: usize = 4096;

impl Thread {
    // unsafe：安全性要求参见 thread::Builder::spawn_unchecked
    pub unsafe fn new(stack: usize, init: Box<ThreadInit>) -> io::Result<Thread> {
        let data = Box::into_raw(init);
        let mut stack_size = crate::cmp::max(stack, MIN_STACK_SIZE);

        if (stack_size & 4095) != 0 {
            stack_size = (stack_size + 4095) & !4095;
        }

        // 一次性分配整块内存，事后再进行划分。这样可以确保即使在本函数执行期间
        // 发生上下文切换，整个栈加上守护页（guard pages）也会保持连续。
        let stack_plus_guard_pages: &mut [u8] = unsafe {
            map_memory(
                None,
                None,
                GUARD_PAGE_SIZE + stack_size + GUARD_PAGE_SIZE,
                MemoryFlags::R | MemoryFlags::W | MemoryFlags::X,
            )
        }
        .map_err(|code| io::Error::from_raw_os_error(code as i32))?;

        // 禁止访问此页。注意：只写（Write-only）页是非法的，会导致访问违例
        //（access violation）。
        unsafe {
            update_memory_flags(&mut stack_plus_guard_pages[0..GUARD_PAGE_SIZE], MemoryFlags::W)
                .map_err(|code| io::Error::from_raw_os_error(code as i32))?
        };

        // 禁止访问此页。注意：只写（Write-only）页是非法的，会导致访问违例
        //（access violation）。
        unsafe {
            update_memory_flags(
                &mut stack_plus_guard_pages[(GUARD_PAGE_SIZE + stack_size)..],
                MemoryFlags::W,
            )
            .map_err(|code| io::Error::from_raw_os_error(code as i32))?
        };

        let guard_page_pre = stack_plus_guard_pages.as_ptr() as usize;
        let tid = create_thread(
            thread_start as *mut usize,
            &mut stack_plus_guard_pages[GUARD_PAGE_SIZE..(stack_size + GUARD_PAGE_SIZE)],
            data as usize,
            guard_page_pre,
            stack_size,
            0,
        )
        .map_err(|code| io::Error::from_raw_os_error(code as i32))?;

        #[inline(never)]
        fn rust_main_thread_not_inlined(init: Box<ThreadInit>) {
            let rust_start = init.init();
            rust_start();
        }

        extern "C" fn thread_start(
            data: *mut usize,
            guard_page_pre: usize,
            stack_size: usize,
        ) -> ! {
            // SAFETY：我们只是在重建先前被泄漏的 box。
            let init = unsafe { Box::from_raw(data as *mut ThreadInit) };

            // 在一个 inline(never) 屏障内运行主线程逻辑，以防止 dealloc 调用
            // 被重排到 TLS 已被销毁之后。
            // 更多背景请参见
            // https://github.com/rust-lang/rust/pull/144465#pullrequestreview-3289729950 。
            rust_main_thread_not_inlined(init);

            // 销毁 TLS，这会释放 TLS 页并调用所有线程本地存储（如果有的话）的析构函数。
            unsafe {
                crate::sys::thread_local::key::destroy_tls();
            }

            // 释放栈内存以及守护页。之后，通过返回到魔法地址 0xff80_3000usize
            // 来退出线程，这会告诉内核回收（deallocate）此线程。
            let mapped_memory_base = guard_page_pre;
            let mapped_memory_length = GUARD_PAGE_SIZE + stack_size + GUARD_PAGE_SIZE;
            unsafe {
                asm!(
                    "ecall",
                    "ret",
                                        in("a0") Syscall::UnmapMemory as usize,
                                        in("a1") mapped_memory_base,
                                        in("a2") mapped_memory_length,
                                        in("ra") 0xff80_3000usize,
                                        options(nomem, nostack, noreturn)
                );
            }
        }

        Ok(Thread { tid })
    }

    pub fn join(self) {
        join_thread(self.tid).unwrap();
    }
}

pub fn available_parallelism() -> io::Result<NonZero<usize>> {
    // 我们目前是单核（unicore）的。
    Ok(unsafe { NonZero::new_unchecked(1) })
}

pub fn yield_now() {
    do_yield();
}

pub fn sleep(dur: Duration) {
    // 由于 sleep 服务以 `usize 毫秒` 为单位工作，因此把消息拆分成这些块。
    // 这意味着如果你在 32 位系统上试图让一个线程睡眠超过 49 天，可能会遇到问题。
    let mut millis = dur.as_millis();
    while millis > 0 {
        let sleep_duration = if millis > (usize::MAX as _) { usize::MAX } else { millis as usize };
        blocking_scalar(ticktimer_server(), TicktimerScalar::SleepMs(sleep_duration).into())
            .expect("failed to send message to ticktimer server");
        millis -= sleep_duration as u128;
    }
}
