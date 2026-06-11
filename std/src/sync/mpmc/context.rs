//! 线程本地（thread-local）的通道上下文。
//!
//! 当一个线程要在通道操作上阻塞，或参与 select（在多个操作中择一就绪者）时，需要一个
//! 用来协调“谁先抢到这次操作”的载体，这就是 `Context`。它通过原子的 `select` 字段实现：
//! 多个线程可能都想把同一个阻塞线程唤醒去完成某操作，但只有第一个对 `select` 做成功
//! CAS 的线程胜出，其余线程会看到该操作已被选走。每个线程缓存一份自己的上下文以复用。

use super::select::Selected;
use super::waker::current_thread_id;
use crate::cell::Cell;
use crate::ptr;
use crate::sync::Arc;
use crate::sync::atomic::{Atomic, AtomicPtr, AtomicUsize, Ordering};
use crate::thread::{self, Thread};
use crate::time::Instant;

/// 线程本地上下文。
#[derive(Debug, Clone)]
pub struct Context {
    inner: Arc<Inner>,
}

/// `Context` 的内部表示。
#[derive(Debug)]
struct Inner {
    /// 被选中的操作。
    ///
    /// 取值来自 `Selected`（`Waiting`/`Aborted`/`Disconnected`/`Operation(_)`）。初始为
    /// `Waiting`，由竞争各方通过 CAS 抢占式地改写为某个具体结果。
    select: Atomic<usize>,

    /// 一个槽位，供另一线程把指向其 `Packet` 的指针存入其中。
    packet: Atomic<*mut ()>,

    /// 线程句柄。
    thread: Thread,

    /// 线程 id。
    thread_id: usize,
}

impl Context {
    /// 在闭包执行期间创建并提供一个上下文。
    ///
    /// 优先复用线程本地缓存的上下文（先 `reset` 再使用），从而避免每次都重新分配 `Arc`。
    #[inline]
    pub fn with<F, R>(f: F) -> R
    where
        F: FnOnce(&Context) -> R,
    {
        thread_local! {
            /// 缓存的线程本地上下文。
            static CONTEXT: Cell<Option<Context>> = Cell::new(Some(Context::new()));
        }

        let mut f = Some(f);
        let mut f = |cx: &Context| -> R {
            let f = f.take().unwrap();
            f(cx)
        };

        CONTEXT
            .try_with(|cell| match cell.take() {
                None => f(&Context::new()),
                Some(cx) => {
                    cx.reset();
                    let res = f(&cx);
                    cell.set(Some(cx));
                    res
                }
            })
            .unwrap_or_else(|_| f(&Context::new()))
    }

    /// 创建一个新的 `Context`。
    #[cold]
    fn new() -> Context {
        Context {
            inner: Arc::new(Inner {
                select: AtomicUsize::new(Selected::Waiting.into()),
                packet: AtomicPtr::new(ptr::null_mut()),
                thread: thread::current_or_unnamed(),
                thread_id: current_thread_id(),
            }),
        }
    }

    /// 重置 `select` 与 `packet`。
    #[inline]
    fn reset(&self) {
        self.inner.select.store(Selected::Waiting.into(), Ordering::Release);
        self.inner.packet.store(ptr::null_mut(), Ordering::Release);
    }

    /// 尝试选中（select）一个操作。
    ///
    /// 失败时，返回此前已被选中的那个操作。
    ///
    /// 该操作以 CAS 实现：仅当当前状态仍为 `Waiting` 时才会写入 `select`。因此当多个线程
    /// 竞相唤醒同一上下文时，只有一个能成功。
    #[inline]
    pub fn try_select(&self, select: Selected) -> Result<(), Selected> {
        self.inner
            .select
            .compare_exchange(
                Selected::Waiting.into(),
                select.into(),
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .map(|_| ())
            .map_err(|e| e.into())
    }

    /// 存入一个 packet。
    ///
    /// 此方法必须在 `try_select` 成功之后、且确实有 packet 需要提供时调用。
    #[inline]
    pub fn store_packet(&self, packet: *mut ()) {
        if !packet.is_null() {
            self.inner.packet.store(packet, Ordering::Release);
        }
    }

    /// 等待直到某个操作被选中，并返回它。
    ///
    /// 如果到达截止时刻（deadline），则会选中 `Selected::Aborted`。
    ///
    /// # Safety
    /// 只能从该 `Context` 所属的那个线程调用。
    #[inline]
    pub unsafe fn wait_until(&self, deadline: Option<Instant>) -> Selected {
        loop {
            // 检查是否已有操作被选中。
            let sel = Selected::from(self.inner.select.load(Ordering::Acquire));
            if sel != Selected::Waiting {
                return sel;
            }

            // 如果设置了截止时刻，就把当前线程挂起（park）直到到达该时刻。
            if let Some(end) = deadline {
                let now = Instant::now();

                if now < end {
                    // SAFETY: 由调用方保证。
                    unsafe { self.inner.thread.park_timeout(end - now) };
                } else {
                    // 已到达截止时刻。尝试中止（abort）本次 select。
                    return match self.try_select(Selected::Aborted) {
                        Ok(()) => Selected::Aborted,
                        Err(s) => s,
                    };
                }
            } else {
                // SAFETY: 由调用方保证。
                unsafe { self.inner.thread.park() };
            }
        }
    }

    /// 唤醒（unpark）该上下文所属的线程。
    #[inline]
    pub fn unpark(&self) {
        self.inner.thread.unpark();
    }

    /// 返回该上下文所属线程的 id。
    #[inline]
    pub fn thread_id(&self) -> usize {
        self.inner.thread_id
    }
}
