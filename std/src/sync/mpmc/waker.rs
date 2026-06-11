//! 用于唤醒阻塞在通道操作上的线程的机制。

use super::context::Context;
use super::select::{Operation, Selected};
use crate::ptr;
use crate::sync::Mutex;
use crate::sync::atomic::{Atomic, AtomicBool, Ordering};

/// 表示一个阻塞在某个特定通道操作上的线程。
pub(crate) struct Entry {
    /// 该操作。
    pub(crate) oper: Operation,

    /// 可选的 packet（用于传递数据的载体，可能为空指针）。
    pub(crate) packet: *mut (),

    /// 与持有该操作的线程关联的上下文。
    pub(crate) cx: Context,
}

/// 一个由阻塞在通道操作上的线程组成的队列。
///
/// 线程用这个数据结构来登记（register）阻塞操作，并在操作变为就绪时被唤醒。
pub(crate) struct Waker {
    /// select 操作列表。
    ///
    /// 这些线程正在等待“被另一线程选中并唤醒”去完成各自的操作。
    selectors: Vec<Entry>,

    /// 等待变为就绪的操作列表。
    ///
    /// 这些是“观察者”（observer），它们只想在通道状态变化时被通知（notify），而非被直接配对。
    observers: Vec<Entry>,
}

impl Waker {
    /// 创建一个新的 `Waker`。
    #[inline]
    pub(crate) fn new() -> Self {
        Waker { selectors: Vec::new(), observers: Vec::new() }
    }

    /// 登记一个 select 操作。
    #[inline]
    pub(crate) fn register(&mut self, oper: Operation, cx: &Context) {
        self.register_with_packet(oper, ptr::null_mut(), cx);
    }

    /// 登记一个 select 操作及其 packet。
    #[inline]
    pub(crate) fn register_with_packet(&mut self, oper: Operation, packet: *mut (), cx: &Context) {
        self.selectors.push(Entry { oper, packet, cx: cx.clone() });
    }

    /// 注销（unregister）一个 select 操作。
    #[inline]
    pub(crate) fn unregister(&mut self, oper: Operation) -> Option<Entry> {
        if let Some((i, _)) =
            self.selectors.iter().enumerate().find(|&(_, entry)| entry.oper == oper)
        {
            let entry = self.selectors.remove(i);
            Some(entry)
        } else {
            None
        }
    }

    /// 尝试找到另一线程的条目，选中其操作，并将其唤醒。
    #[inline]
    pub(crate) fn try_select(&mut self) -> Option<Entry> {
        if self.selectors.is_empty() {
            None
        } else {
            let thread_id = current_thread_id();

            self.selectors
                .iter()
                .position(|selector| {
                    // 该条目是否属于另一个（不同的）线程？
                    selector.cx.thread_id() != thread_id
                        && selector // 尝试选中这个操作。
                            .cx
                            .try_select(Selected::Operation(selector.oper))
                            .is_ok()
                        && {
                            // 提供 packet。
                            selector.cx.store_packet(selector.packet);
                            // 唤醒该线程。
                            selector.cx.unpark();
                            true
                        }
                })
                // 把该条目从队列中移除，以保持队列整洁并提升性能。
                .map(|pos| self.selectors.remove(pos))
        }
    }

    /// 通知所有正在等待变为就绪的操作。
    #[inline]
    pub(crate) fn notify(&mut self) {
        for entry in self.observers.drain(..) {
            if entry.cx.try_select(Selected::Operation(entry.oper)).is_ok() {
                entry.cx.unpark();
            }
        }
    }

    /// 通知所有已登记的操作：通道已断连（disconnected）。
    #[inline]
    pub(crate) fn disconnect(&mut self) {
        for entry in self.selectors.iter() {
            if entry.cx.try_select(Selected::Disconnected).is_ok() {
                // 唤醒该线程。
                //
                // 这里我们不把条目从队列中移除。已登记的线程必须自行从 waker 中注销。
                // 它们或许还想取回 packet 的值，并在必要时将其销毁。
                entry.cx.unpark();
            }
        }

        self.notify();
    }
}

impl Drop for Waker {
    #[inline]
    fn drop(&mut self) {
        debug_assert_eq!(self.selectors.len(), 0);
        debug_assert_eq!(self.observers.len(), 0);
    }
}

/// 一个可在多个线程间共享、且无需上层加锁即可使用的 waker。
///
/// 它是对 `Waker` 的简单封装，内部使用一把互斥锁（mutex）来做同步。
pub(crate) struct SyncWaker {
    /// 内部的 `Waker`。
    inner: Mutex<Waker>,

    /// 若 waker 为空，则为 `true`。
    ///
    /// 这是一个供快速路径使用的缓存标志：notify 时可先无锁读它，为空就直接跳过加锁。
    is_empty: Atomic<bool>,
}

impl SyncWaker {
    /// 创建一个新的 `SyncWaker`。
    #[inline]
    pub(crate) fn new() -> Self {
        SyncWaker { inner: Mutex::new(Waker::new()), is_empty: AtomicBool::new(true) }
    }

    /// 用一个操作登记当前线程。
    #[inline]
    pub(crate) fn register(&self, oper: Operation, cx: &Context) {
        let mut inner = self.inner.lock().unwrap();
        inner.register(oper, cx);
        self.is_empty
            .store(inner.selectors.is_empty() && inner.observers.is_empty(), Ordering::SeqCst);
    }

    /// 注销当前线程此前登记的某个操作。
    #[inline]
    pub(crate) fn unregister(&self, oper: Operation) -> Option<Entry> {
        let mut inner = self.inner.lock().unwrap();
        let entry = inner.unregister(oper);
        self.is_empty
            .store(inner.selectors.is_empty() && inner.observers.is_empty(), Ordering::SeqCst);
        entry
    }

    /// 尝试找到一个线程（非当前线程），选中其操作，并将其唤醒。
    ///
    /// 采用双重检查（double-checked）模式：先无锁读 `is_empty`，仅当非空时才加锁，进锁后再
    /// 复检一次，以避免在空队列上做无谓的加锁。
    #[inline]
    pub(crate) fn notify(&self) {
        if !self.is_empty.load(Ordering::SeqCst) {
            let mut inner = self.inner.lock().unwrap();
            if !self.is_empty.load(Ordering::SeqCst) {
                inner.try_select();
                inner.notify();
                self.is_empty.store(
                    inner.selectors.is_empty() && inner.observers.is_empty(),
                    Ordering::SeqCst,
                );
            }
        }
    }

    /// 通知所有线程：通道已断连（disconnected）。
    #[inline]
    pub(crate) fn disconnect(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.disconnect();
        self.is_empty
            .store(inner.selectors.is_empty() && inner.observers.is_empty(), Ordering::SeqCst);
    }
}

impl Drop for SyncWaker {
    #[inline]
    fn drop(&mut self) {
        debug_assert!(self.is_empty.load(Ordering::SeqCst));
    }
}

/// 返回当前线程的唯一 id。
#[inline]
pub fn current_thread_id() -> usize {
    // `u8` 不需要 drop，因此该变量在线程销毁期间仍然可用；
    // 而 `thread::current()` 在那时则不可用。
    thread_local! { static DUMMY: u8 = const { 0 } }
    DUMMY.with(|x| (x as *const u8).addr())
}
