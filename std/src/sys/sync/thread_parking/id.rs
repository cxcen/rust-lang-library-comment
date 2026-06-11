//! 使用线程 id 实现的线程 parking。
//!
//! 某些平台（尤其是 NetBSD）拥有语义与 `thread::park` 所提供的相匹配的线程
//! parking 原语，区别在于：要被 unpark 的线程是通过一个平台特定的线程 id 来
//! 引用的。由于线程 parker 是在那个 id 已知之前就构造出来的，因此这里用一个
//! 原子状态变量来管理 park 状态并传播线程 id。这同时也避免了在 `unpark` 先于
//! `park` 被调用的情形下进行平台调用。

use crate::cell::UnsafeCell;
use crate::pin::Pin;
use crate::sync::atomic::Ordering::{Acquire, Relaxed, Release};
use crate::sync::atomic::{Atomic, AtomicI8, fence};
use crate::sys::thread_parking::{ThreadId, current, park, park_timeout, unpark};
use crate::time::Duration;

pub struct Parker {
    state: Atomic<i8>,
    tid: UnsafeCell<Option<ThreadId>>,
}

const PARKED: i8 = -1;
const EMPTY: i8 = 0;
const NOTIFIED: i8 = 1;

impl Parker {
    pub fn new() -> Parker {
        Parker { state: AtomicI8::new(EMPTY), tid: UnsafeCell::new(None) }
    }

    /// 创建一个新的线程 parker。UNIX 要求这必须就地（in-place）进行。
    pub unsafe fn new_in_place(parker: *mut Parker) {
        parker.write(Parker::new())
    }

    /// # 安全性(Safety）
    /// * 必须始终从同一个线程调用
    /// * 必须在状态被设为 PARKED 之前调用
    unsafe fn init_tid(&self) {
        // 这个字段只会从本线程写入，所以我们在这里读取它时不需要同步。
        if self.tid.get().read().is_none() {
            // 由于这一点只会被到达一次——即在状态第一次被设为 PARKED 之前——
            // 因此这里的非原子写入不会与其他线程的读取产生冲突。
            self.tid.get().write(Some(current()));
            // 确保该写入能被所有读取状态的线程观测到。与 `unpark` 中的 acquire
            // 屏障（barrier）建立同步关系。
            fence(Release);
        }
    }

    pub unsafe fn park(self: Pin<&Self>) {
        self.init_tid();

        // 把 NOTIFIED 改为 EMPTY，把 EMPTY 改为 PARKED。
        let state = self.state.fetch_sub(1, Acquire);
        if state == EMPTY {
            // 循环以防范虚假唤醒（spurious wakeup）。
            // 状态必须以 acquire 内存序重置，以确保所有对 `unpark` 的调用都与
            // 本线程建立同步关系。
            while self.state.compare_exchange(NOTIFIED, EMPTY, Acquire, Relaxed).is_err() {
                park(self.state.as_ptr().addr());
            }
        }
    }

    pub unsafe fn park_timeout(self: Pin<&Self>, dur: Duration) {
        self.init_tid();

        let state = self.state.fetch_sub(1, Acquire).wrapping_sub(1);
        if state == PARKED {
            park_timeout(dur, self.state.as_ptr().addr());
            // 做一次 swap，以确保我们以 acquire 内存序观测到所有的状态改动。
            self.state.swap(EMPTY, Acquire);
        }
    }

    pub fn unpark(self: Pin<&Self>) {
        let state = self.state.swap(NOTIFIED, Release);
        if state == PARKED {
            // 与 `init_tid` 中的 release 栅栏（fence）建立同步关系，以观测到对
            // `tid` 的写入。
            fence(Acquire);
            // # 安全性(Safety）
            // 线程 id 是在状态第一次被设为 `PARKED` 之前初始化的，并且从那一刻起
            // 不再被写入（因此无需进行原子读取）。
            let tid = unsafe { self.tid.get().read().unwrap_unchecked() };
            // 有可能等待的线程因为超时而醒来并在本次调用发生之前就终止了。这种情况下
            // 本次调用会返回一个错误，或者唤醒一个不相关的线程。不过平台 API 和环境
            // 确实允许这种情况。
            unpark(tid, self.state.as_ptr().addr());
        }
    }
}

unsafe impl Send for Parker {}
unsafe impl Sync for Parker {}
