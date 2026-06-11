use crate::ffi::CStr;
use crate::io;
use crate::num::NonZero;
use crate::thread::ThreadInit;
use crate::time::Duration;

// 抑制针对原本未被使用的 ThreadInit::init() 调用的死代码（dead code）警告。
#[expect(dead_code)]
fn dummy_init_call(init: Box<ThreadInit>) {
    drop(init.init());
}

pub struct Thread(!);

pub const DEFAULT_MIN_STACK_SIZE: usize = 64 * 1024;

impl Thread {
    // unsafe：安全性要求参见 thread::Builder::spawn_unchecked
    pub unsafe fn new(_stack: usize, _init: Box<ThreadInit>) -> io::Result<Thread> {
        Err(io::Error::UNSUPPORTED_PLATFORM)
    }

    pub fn join(self) {
        self.0
    }
}

pub fn available_parallelism() -> io::Result<NonZero<usize>> {
    Err(io::Error::UNKNOWN_THREAD_COUNT)
}

pub fn current_os_id() -> Option<u64> {
    None
}

pub fn yield_now() {
    // 什么也不做
}

pub fn set_name(_name: &CStr) {
    // 不支持
}

pub fn sleep(_dur: Duration) {
    panic!("can't sleep");
}
