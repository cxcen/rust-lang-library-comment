//! SOLID 和 macOS 一样，提供了注册 TLS 析构函数（destructors）的 API。
//! 但由于它不允许为该函数指定参数，并且不会为已终止的任务运行析构函数，
//! 因此我们仍然维护自己的一份列表。

use crate::cell::Cell;
use crate::sys::pal::abi;
use crate::sys::pal::itron::task;
use crate::sys::thread_local::destructors;

pub fn enable() {
    #[thread_local]
    static REGISTERED: Cell<bool> = Cell::new(false);

    if !REGISTERED.replace(true) {
        let tid = task::current_task_id_aborting();
        // 注册 `tls_dtor`，以确保对于那些并非由 `std::thread` 而是以其他方式
        // 创建的任务，TLS 析构函数也会被调用
        unsafe { abi::SOLID_TLS_AddDestructor(tid as i32, tls_dtor) };
    }

    unsafe extern "C" fn tls_dtor(_unused: *mut u8) {
        unsafe {
            destructors::run();
            crate::rt::thread_cleanup();
        }
    }
}
