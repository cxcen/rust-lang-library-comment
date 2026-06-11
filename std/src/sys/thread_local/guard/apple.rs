//! macOS 允许通过 _tlv_atexit 注册析构函数（destructors）。但由于在 TLS
//! 析构函数正在运行期间调用它属于 UB（未定义行为），我们仍然需要维护
//! 自己的一份析构函数列表。

use crate::cell::Cell;
use crate::ptr;
use crate::sys::thread_local::destructors;

pub fn enable() {
    #[thread_local]
    static REGISTERED: Cell<bool> = Cell::new(false);

    unsafe extern "C" {
        fn _tlv_atexit(dtor: unsafe extern "C" fn(*mut u8), arg: *mut u8);
    }

    if !REGISTERED.replace(true) {
        // SAFETY：在 TLS 析构函数正在运行期间调用 _tlv_atexit 属于 UB。
        // 但由于 run_dtors 只会在注册完成之后被调用，因此无法从它内部
        // 到达这一点。
        unsafe {
            _tlv_atexit(run_dtors, ptr::null_mut());
        }
    }

    unsafe extern "C" fn run_dtors(_: *mut u8) {
        unsafe {
            destructors::run();
            crate::rt::thread_cleanup();
        }
    }
}
