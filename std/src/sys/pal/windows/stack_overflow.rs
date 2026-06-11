#![cfg_attr(test, allow(dead_code))]

use crate::sys::c;
use crate::thread;

/// 预留栈空间，以供栈溢出异常时使用。
pub fn reserve_stack() {
    let result = unsafe { c::SetThreadStackGuarantee(&mut 0x5000) };
    // 预留栈空间并非关键操作，因此在 libstd 的发布构建中允许它失败。
    // 不过这里仍使用 debug assert，以便 CI 能够检测出我们在调用该函数时是否犯了错误。
    debug_assert_ne!(result, 0, "failed to reserve stack space for exception handling");
}

unsafe extern "system" fn vectored_handler(ExceptionInfo: *mut c::EXCEPTION_POINTERS) -> i32 {
    // SAFETY: 由调用方（本例中即操作系统）负责确保 `ExceptionInfo` 有效。
    unsafe {
        let rec = &(*(*ExceptionInfo).ExceptionRecord);
        let code = rec.ExceptionCode;

        if code == c::EXCEPTION_STACK_OVERFLOW {
            thread::with_current_name(|name| {
                let name = name.unwrap_or("<unknown>");
                let tid = thread::current_os_id();
                rtprintpanic!("\nthread '{name}' ({tid}) has overflowed its stack\n");
            });
        }
        c::EXCEPTION_CONTINUE_SEARCH
    }
}

pub fn init() {
    // SAFETY: `vectored_handler` 具有正确的 ABI，且在异常处理期间调用它是安全的。
    unsafe {
        let result = c::AddVectoredExceptionHandler(0, Some(vectored_handler));
        // 与上面类似，安装栈溢出处理程序允许失败，但这里使用 debug assert，
        // 以便 CI 仍能检测它在正常情况下是否工作。
        debug_assert!(!result.is_null(), "failed to install exception handler");
    }
    // 为主线程设置线程栈保障（stack guarantee）。
    reserve_stack();
}
