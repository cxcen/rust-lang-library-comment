use crate::sync::atomic::{Atomic, AtomicBool, AtomicUsize, Ordering};
use crate::{ops, process};

/// 引用计数器的内部数据。
///
/// 通道本体被这个计数器包裹起来，由所有 `Sender` 与 `Receiver` 句柄共享。
/// 当最后一个发送者或最后一个接收者句柄被丢弃时，会触发断连（disconnect）；
/// 当两侧都归零后，底层通道才会被真正释放。
struct Counter<C> {
    /// 与该通道关联的发送者（sender）数量。
    senders: Atomic<usize>,

    /// 与该通道关联的接收者（receiver）数量。
    receivers: Atomic<usize>,

    /// 若最后一个发送者或最后一个接收者引用负责释放通道，则被置为 `true`。
    ///
    /// 这是一个“谁最后离开谁关灯”的标志：两侧各自归零时都会调用 `swap(true)`，
    /// 只有看到旧值已是 `true` 的那一方（即第二个到达者）才真正回收内存。
    destroy: Atomic<bool>,

    /// 内部通道本体。
    chan: C,
}

/// 将一个通道包装进引用计数器中。
///
/// 返回一对初始引用计数均为 1 的 `Sender` 与 `Receiver`。
pub(crate) fn new<C>(chan: C) -> (Sender<C>, Receiver<C>) {
    let counter = Box::into_raw(Box::new(Counter {
        senders: AtomicUsize::new(1),
        receivers: AtomicUsize::new(1),
        destroy: AtomicBool::new(false),
        chan,
    }));
    let s = Sender { counter };
    let r = Receiver { counter };
    (s, r)
}

/// 发送端。
pub(crate) struct Sender<C> {
    counter: *mut Counter<C>,
}

impl<C> Sender<C> {
    /// 返回内部的 `Counter`。
    fn counter(&self) -> &Counter<C> {
        unsafe { &*self.counter }
    }

    /// 获取（acquire）另一个发送者引用。
    pub(crate) fn acquire(&self) -> Sender<C> {
        let count = self.counter().senders.fetch_add(1, Ordering::Relaxed);

        // 反复克隆发送者并对克隆体调用 `mem::forget` 有可能让计数器溢出。从这种极端退化的
        // 场景中合理地恢复非常困难，因此当计数变得非常大时，我们直接中止（abort）进程。
        if count > isize::MAX as usize {
            process::abort();
        }

        Sender { counter: self.counter }
    }

    /// 释放（release）该发送者引用。
    ///
    /// 如果这是最后一个发送者引用，则会调用 `disconnect` 函数。
    pub(crate) unsafe fn release<F: FnOnce(&C) -> bool>(&self, disconnect: F) {
        if self.counter().senders.fetch_sub(1, Ordering::AcqRel) == 1 {
            disconnect(&self.counter().chan);

            if self.counter().destroy.swap(true, Ordering::AcqRel) {
                drop(unsafe { Box::from_raw(self.counter) });
            }
        }
    }
}

impl<C> ops::Deref for Sender<C> {
    type Target = C;

    fn deref(&self) -> &C {
        &self.counter().chan
    }
}

impl<C> PartialEq for Sender<C> {
    fn eq(&self, other: &Sender<C>) -> bool {
        self.counter == other.counter
    }
}

/// 接收端。
pub(crate) struct Receiver<C> {
    counter: *mut Counter<C>,
}

impl<C> Receiver<C> {
    /// 返回内部的 `Counter`。
    fn counter(&self) -> &Counter<C> {
        unsafe { &*self.counter }
    }

    /// 获取（acquire）另一个接收者引用。
    pub(crate) fn acquire(&self) -> Receiver<C> {
        let count = self.counter().receivers.fetch_add(1, Ordering::Relaxed);

        // 反复克隆接收者并对克隆体调用 `mem::forget` 有可能让计数器溢出。从这种极端退化的
        // 场景中合理地恢复非常困难，因此当计数变得非常大时，我们直接中止（abort）进程。
        if count > isize::MAX as usize {
            process::abort();
        }

        Receiver { counter: self.counter }
    }

    /// 释放（release）该接收者引用。
    ///
    /// 如果这是最后一个接收者引用，则会调用 `disconnect` 函数。
    pub(crate) unsafe fn release<F: FnOnce(&C) -> bool>(&self, disconnect: F) {
        if self.counter().receivers.fetch_sub(1, Ordering::AcqRel) == 1 {
            disconnect(&self.counter().chan);

            if self.counter().destroy.swap(true, Ordering::AcqRel) {
                drop(unsafe { Box::from_raw(self.counter) });
            }
        }
    }
}

impl<C> ops::Deref for Receiver<C> {
    type Target = C;

    fn deref(&self) -> &C {
        &self.counter().chan
    }
}

impl<C> PartialEq for Receiver<C> {
    fn eq(&self, other: &Receiver<C>) -> bool {
        self.counter == other.counter
    }
}
