#![cfg_attr(test, allow(unused))] // RT initialization logic is not compiled for test

use core::arch::global_asm;
use core::sync::atomic::{Atomic, AtomicUsize, Ordering};

use crate::alloc::System;
use crate::io::Write;

// 运行时特性
pub mod panic;
mod reloc;

// 库特性
pub mod mem;
pub mod thread;
pub mod tls;
#[macro_use]
pub mod usercalls;

#[cfg(not(test))]
global_asm!(include_str!("entry.S"), options(att_syntax));

#[repr(C)]
struct EntryReturn(u64, u64);

#[cfg(not(test))]
#[unsafe(no_mangle)]
unsafe extern "C" fn tcs_init(secondary: bool) {
    // 修改这段代码时务必非常小心：它在二进制文件被重定位之前就已运行。任何对
    // 符号的间接访问都很可能失败。
    const UNINIT: usize = 0;
    const BUSY: usize = 1;
    const DONE: usize = 2;
    // 三态自旋锁（three-state spin-lock）
    static RELOC_STATE: Atomic<usize> = AtomicUsize::new(UNINIT);

    if secondary && RELOC_STATE.load(Ordering::Relaxed) != DONE {
        rtabort!("Entered secondary TCS before main TCS!")
    }

    // 尝试原子地把 UNINIT 交换为 BUSY。返回的状态可能是：
    match RELOC_STATE.compare_exchange(UNINIT, BUSY, Ordering::Acquire, Ordering::Acquire) {
        // 本线程刚刚获得了锁，其他线程将观察到 BUSY
        Ok(_) => {
            reloc::relocate_elf_rela();
            RELOC_STATE.store(DONE, Ordering::Release);
        }
        // 我们需要等待，直到初始化完成。
        Err(BUSY) => {
            while RELOC_STATE.load(Ordering::Acquire) == BUSY {
                core::hint::spin_loop();
            }
        }
        // 初始化已完成。
        Err(DONE) => {}
        _ => unreachable!(),
    }
}

// FIXME: 此条目应当仅在它被链接进可执行文件（main 函数存在）时才存在。如果这是
// 一个库，crate 作者应当能够指定这一点
#[cfg(not(test))]
#[unsafe(no_mangle)]
extern "C" fn entry(p1: u64, p2: u64, p3: u64, secondary: bool, p4: u64, p5: u64) -> EntryReturn {
    // FIXME: 在库模式下如何支持 TLS？
    // 这里我们使用 System 分配器，以便全局分配器可以使用线程局部变量
    // （thread-locals）。
    let tls = Box::new_in(tls::Tls::new(), System);
    let tls_guard = unsafe { tls.activate() };

    if secondary {
        let join_notifier = crate::sys::thread::Thread::entry();
        drop(tls_guard);
        drop(join_notifier);

        EntryReturn(0, 0)
    } else {
        unsafe extern "C" {
            fn main(argc: isize, argv: *const *const u8) -> isize;
        }

        // 检查 entry 是否按照 ABI 被调用
        rtassert!(p3 == 0);
        rtassert!(p4 == 0);
        rtassert!(p5 == 0);

        unsafe {
            // 这些参数的实际类型为 `p1: *const Arg, p2: usize`。我们目前无法
            // 定制 Rust main 函数的参数列表，因此把它们作为标准的指针大小值
            // 在 `argc` 和 `argv` 中传入。
            let ret = main(p2 as _, p1 as _);
            exit_with_code(ret)
        }
    }
}

pub(super) fn exit_with_code(code: isize) -> ! {
    if code != 0 {
        if let Some(mut out) = panic::SgxPanicOutput::new() {
            let _ = write!(out, "Exited with status code {code}");
        }
    }
    usercalls::exit(code != 0);
}

#[cfg(not(test))]
#[unsafe(no_mangle)]
extern "C" fn abort_reentry() -> ! {
    usercalls::exit(false)
}
