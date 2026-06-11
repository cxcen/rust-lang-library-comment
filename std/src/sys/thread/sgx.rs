#![cfg_attr(test, allow(dead_code))] // 这为什么是必要的？

use crate::io;
use crate::sys::pal::abi::{thread, usercalls};
use crate::thread::ThreadInit;
use crate::time::Duration;

pub struct Thread(task_queue::JoinHandle);

pub const DEFAULT_MIN_STACK_SIZE: usize = 4096;

pub use self::task_queue::JoinNotifier;

mod task_queue {
    use super::wait_notify;
    use crate::sync::{Mutex, MutexGuard};
    use crate::thread::ThreadInit;

    pub type JoinHandle = wait_notify::Waiter;

    pub struct JoinNotifier(Option<wait_notify::Notifier>);

    impl Drop for JoinNotifier {
        fn drop(&mut self) {
            self.0.take().unwrap().notify();
        }
    }

    pub(super) struct Task {
        init: Box<ThreadInit>,
        done: JoinNotifier,
    }

    impl Task {
        pub(super) fn new(init: Box<ThreadInit>) -> (Task, JoinHandle) {
            let (done, recv) = wait_notify::new();
            let done = JoinNotifier(Some(done));
            (Task { init, done }, recv)
        }

        pub(super) fn run(self) -> JoinNotifier {
            let rust_start = self.init.init();
            rust_start();
            self.done
        }
    }

    // 指定 linkage/符号名，纯粹是为了确保本 crate 与其单元测试之间只有一个实例
    #[cfg_attr(test, linkage = "available_externally")]
    #[unsafe(export_name = "_ZN16__rust_internals3std3sys3pal3sgx6thread10TASK_QUEUEE")]
    static TASK_QUEUE: Mutex<Vec<Task>> = Mutex::new(Vec::new());

    pub(super) fn lock() -> MutexGuard<'static, Vec<Task>> {
        TASK_QUEUE.lock().unwrap()
    }
}

/// 此模块提供了一个不使用线程本地变量（thread local variables）的同步原语。
/// 这是为了用于发出“某个线程已结束执行”的信号。该信号在所有 TLS 析构函数
/// 都完成之后发送，此时不应再创建任何新的线程本地变量。
pub mod wait_notify {
    use crate::pin::Pin;
    use crate::sync::Arc;
    use crate::sys::sync::Parker;

    pub struct Notifier(Arc<Parker>);

    impl Notifier {
        /// 通知等待者（waiter）。等待者要么被立即通知（如果它当前正阻塞在
        /// `Waiter::wait()` 中），要么在它稍后调用 `Waiter::wait()` 方法时被通知。
        pub fn notify(self) {
            Pin::new(&*self.0).unpark()
        }
    }

    pub struct Waiter(Arc<Parker>);

    impl Waiter {
        /// 等待一个通知。如果 `Notifier::notify()` 已经被调用过，则此调用会
        /// 立即返回；否则当前线程会被阻塞，直到收到通知。
        pub fn wait(self) {
            // SAFETY：
            // 此函数只会在一个线程上被调用。
            unsafe { Pin::new(&*self.0).park() }
        }
    }

    pub fn new() -> (Notifier, Waiter) {
        let inner = Arc::new(Parker::new());
        (Notifier(inner.clone()), Waiter(inner))
    }
}

impl Thread {
    // unsafe：安全性要求参见 thread::Builder::spawn_unchecked
    pub unsafe fn new(_stack: usize, init: Box<ThreadInit>) -> io::Result<Thread> {
        let mut queue_lock = task_queue::lock();
        unsafe { usercalls::launch_thread()? };
        let (task, handle) = task_queue::Task::new(init);
        queue_lock.push(task);
        Ok(Thread(handle))
    }

    pub(crate) fn entry() -> JoinNotifier {
        let mut pending_tasks = task_queue::lock();
        let task = rtunwrap!(Some, pending_tasks.pop());
        drop(pending_tasks); // 确保持有任务队列锁的时间不超过必要
        task.run()
    }

    pub fn join(self) {
        self.0.wait();
    }
}

pub fn current_os_id() -> Option<u64> {
    Some(thread::current().addr().get() as u64)
}

pub fn sleep(dur: Duration) {
    usercalls::wait_timeout(0, dur, || true);
}

pub fn yield_now() {
    let wait_error = rtunwrap!(Err, usercalls::wait(0, usercalls::raw::WAIT_NO));
    rtassert!(wait_error.kind() == io::ErrorKind::WouldBlock);
}
