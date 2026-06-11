pub mod os;
pub mod time;

#[expect(dead_code)]
#[path = "../unsupported/common.rs"]
mod unsupported_common;

pub use unsupported_common::{init, unsupported, unsupported_err};

use crate::arch::global_asm;
use crate::ptr;
use crate::sys::stdio;
use crate::time::{Duration, Instant};

global_asm!(
    r#"
    .section .boot, "ax"
    .global _boot

    _boot:
        ldr sp, =__stack_top @ Set up the user stack.
        b _start             @ Jump to the Rust entrypoint.
    "#
);

#[cfg(not(test))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn _start() -> ! {
    unsafe extern "C" {
        static mut __bss_start: u8;
        static mut __bss_end: u8;

        fn main() -> i32;
    }

    // 通过填充零来清空 .bss（未初始化静态变量）段。
    // 这是必需的，因为编译器假定它在首次访问时已被清零。
    ptr::write_bytes(
        &raw mut __bss_start,
        0,
        (&raw mut __bss_end).offset_from_unsigned(&raw mut __bss_start),
    );

    main();

    cleanup();
    abort_internal()
}

// SAFETY: 必须在运行时清理期间仅调用一次。
// NOTE: 不保证一定会被运行，例如当程序中止（abort）时。
pub unsafe fn cleanup() {
    let exit_time = Instant::now();
    const FLUSH_TIMEOUT: Duration = Duration::from_millis(15);

    // 强制刷新串口（serial）缓冲区
    while exit_time.elapsed() < FLUSH_TIMEOUT {
        vex_sdk::vexTasksRun();

        // 如果缓冲区已被完全刷新，则退出循环
        if vex_sdk::vexSerialWriteFree(stdio::STDIO_CHANNEL) == (stdio::STDOUT_BUF_SIZE as i32) {
            break;
        }
    }
}

pub fn abort_internal() -> ! {
    unsafe {
        vex_sdk::vexSystemExitRequest();

        loop {
            vex_sdk::vexTasksRun();
        }
    }
}
