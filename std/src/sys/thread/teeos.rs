use crate::mem::{self, ManuallyDrop};
use crate::sys::os;
use crate::thread::ThreadInit;
use crate::time::Duration;
use crate::{cmp, io, ptr};

pub const DEFAULT_MIN_STACK_SIZE: usize = 8 * 1024;

unsafe extern "C" {
    safe fn TEE_Wait(timeout: u32) -> u32;
}

fn min_stack_size(_: *const libc::pthread_attr_t) -> usize {
    libc::PTHREAD_STACK_MIN.try_into().expect("Infallible")
}

pub struct Thread {
    id: libc::pthread_t,
}

// 在某些平台上，pthread_t 可能是一个指针，此时我们仍然希望
// Thread 是 Send/Sync 的。
unsafe impl Send for Thread {}
unsafe impl Sync for Thread {}

impl Thread {
    // unsafe：安全性要求参见 thread::Builder::spawn_unchecked
    pub unsafe fn new(stack: usize, init: Box<ThreadInit>) -> io::Result<Thread> {
        let data = Box::into_raw(init);
        let mut native: libc::pthread_t = unsafe { mem::zeroed() };
        let mut attr: libc::pthread_attr_t = unsafe { mem::zeroed() };
        assert_eq!(unsafe { libc::pthread_attr_init(&mut attr) }, 0);
        assert_eq!(
            unsafe {
                libc::pthread_attr_settee(
                    &mut attr,
                    libc::TEESMP_THREAD_ATTR_CA_INHERIT,
                    libc::TEESMP_THREAD_ATTR_TASK_ID_INHERIT,
                    libc::TEESMP_THREAD_ATTR_HAS_SHADOW,
                )
            },
            0,
        );

        let stack_size = cmp::max(stack, min_stack_size(&attr));

        match unsafe { libc::pthread_attr_setstacksize(&mut attr, stack_size) } {
            0 => {}
            n => {
                assert_eq!(n, libc::EINVAL);
                // EINVAL 意味着 |stack_size| 要么太小，要么不是系统页大小的整数倍。
                // 由于它肯定 >= PTHREAD_STACK_MIN，所以一定是对齐问题。
                // 向上取整到最近的页边界后再试一次。
                let page_size = os::page_size();
                let stack_size =
                    (stack_size + page_size - 1) & (-(page_size as isize - 1) as usize - 1);
                assert_eq!(unsafe { libc::pthread_attr_setstacksize(&mut attr, stack_size) }, 0);
            }
        };

        let ret = unsafe { libc::pthread_create(&mut native, &attr, thread_start, data as *mut _) };
        // 注意：如果线程创建失败且此断言失败，那么 data 会被泄漏。
        // 然而，另一种设计可能导致 double-free（重复释放），那显然更糟。
        assert_eq!(unsafe { libc::pthread_attr_destroy(&mut attr) }, 0);

        return if ret != 0 {
            // 线程启动失败，因此 data 没有被消费掉。所以重新构造这个
            // box 以便它被释放是安全的。
            drop(unsafe { Box::from_raw(data) });
            Err(io::Error::from_raw_os_error(ret))
        } else {
            // 新线程最早会在下一次 yield 之后开始运行。
            // 我们在这里加一次 yield，这样用户就不必自己加了。
            yield_now();
            Ok(Thread { id: native })
        };

        extern "C" fn thread_start(data: *mut libc::c_void) -> *mut libc::c_void {
            // SAFETY：我们只是在重建先前被泄漏的 box。
            let init = unsafe { Box::from_raw(data as *mut ThreadInit) };
            let rust_start = init.init();
            rust_start();
            ptr::null_mut()
        }
    }

    /// 必须 join，因为不支持 pthread_detach
    pub fn join(self) {
        let id = self.into_id();
        let ret = unsafe { libc::pthread_join(id, ptr::null_mut()) };
        assert!(ret == 0, "failed to join thread: {}", io::Error::from_raw_os_error(ret));
    }

    pub fn into_id(self) -> libc::pthread_t {
        ManuallyDrop::new(self).id
    }
}

impl Drop for Thread {
    fn drop(&mut self) {
        // 我们无法调用 detach，所以如果线程在没有 join 的情况下被 spawn，就直接 panic
        panic!("thread must join, detach is not supported!");
    }
}

pub fn yield_now() {
    let ret = unsafe { libc::sched_yield() };
    debug_assert_eq!(ret, 0);
}

/// 在 teeos 中只有主线程能够等待一段时间
pub fn sleep(dur: Duration) {
    let sleep_millis = dur.as_millis();
    let final_sleep: u32 =
        if sleep_millis >= u32::MAX as u128 { u32::MAX } else { sleep_millis as u32 };
    TEE_Wait(final_sleep);
}
