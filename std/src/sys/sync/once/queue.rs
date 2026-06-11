// 每个 `Once` 都有一个机器字（word）大小的原子状态，通过对这个状态做 CAS
// 来决定该做什么。`Once` 共有四种可能的状态：
//
// * Incomplete（未完成）- 尚未运行过任何初始化，且当前没有线程在使用这个
//                Once。
// * Poisoned（已毒化）- 之前曾有某个线程尝试初始化这个 Once，但它 panic 了，
//              因此该 Once 现在已被毒化。当前没有其他线程在访问这个 Once。
// * Running（运行中）- 当前有某个线程正在尝试运行初始化。它可能会成功，
//             因此所有后续线程都需要等待它完成。
//             注意这个状态还附带一个载荷（payload），见下文描述。
// * Complete（已完成）- 初始化已经完成，所有后续调用都应立即返回。
//
// 有 4 种状态我们需要 2 个比特来编码，而我们把已分配的这个机器字中剩下的比特
// 用作一个「正在等待进入 RUNNING 状态的那个负责线程」的线程队列。这个队列只是
// 一个由 Waiter 节点组成的链表，其长度单调递增。每个节点都分配在等待线程自己的
// 栈上，而每当运行中的闭包结束时，它会消费整个队列并通知所有等待者去重试。
//
// 在实现中你还会看到更多细节，但要点就是这些！
//
// Futex 内存序（Futex orderings）：
// 运行 `Once` 时我们要处理多个原子量：
// `Once.state_and_queue` 和数量未知的若干 `Waiter.signaled`。
// * `state_and_queue` 被用作：(1) 状态标志，(2) 同步 `Once` 的结果，
//   (3) 同步 `Waiter` 节点。
//     - 在 `call` 函数末尾我们必须确保以 acquire 方式获取 `Once` 的结果。
//       因此每一个「可能是唯一一次读到 COMPLETED 的」load 都必须至少带有
//       acquire 内存序，也就是说这三类 load 全都要 acquire。
//     - `WaiterQueue::drop` 是唯一可能写入 COMPLETED 的地方，且必须以
//       release 内存序写入，以使结果对外可见。
//     - `wait` 会把 `Waiter` 节点作为指针插入 `state_and_queue`，并需要以
//       release 内存序使这些节点对外可见。其 `compare_exchange` 中的 load
//       可以是 relaxed，因为它只需比较该原子量，而不需要读取其他数据。
//     - `WaiterQueue::drop` 必须能看到这些 `Waiter` 节点，所以它必须以
//       acquire 内存序 load `state_and_queue`。
//     - 只有一处对 `state_and_queue` 的 store 仅把它当作状态标志使用，
//       而无需同步数据：即 `call` 中把状态从 INCOMPLETE 切换为 RUNNING。
//       这个 store 可以是 Relaxed，但由于上面提到的要求，读取必须是 Acquire。
// * `Waiter.signaled` 既用作标志，也用来保护 `Waiter` 中一个具有内部可变性
//   的字段。`Waiter.thread` 会在 `WaiterQueue::drop` 中被修改，随后它会以
//   release 内存序设置 `signaled`。当 `wait` 以 acquire 内存序 load
//   `signaled` 并看到它为 true 之后，它需要能看到这些改动，以正确地 drop
//   `Waiter` 结构体。
// * 有一处地方，`Once.state_and_queue` 和 `Waiter.signaled` 这两个原子量会
//   汇合到一起，可能会被编译器或处理器重排序。由于两者都使用 acquire 内存序，
//   这样的重排序是不被允许的，所以无需 `SeqCst`。

use crate::cell::Cell;
use crate::sync::atomic::Ordering::{AcqRel, Acquire, Release};
use crate::sync::atomic::{Atomic, AtomicBool, AtomicPtr};
use crate::sync::once::OnceExclusiveState;
use crate::thread::{self, Thread};
use crate::{fmt, ptr, sync as public};

type StateAndQueue = *mut ();

pub struct Once {
    state_and_queue: Atomic<*mut ()>,
}

pub struct OnceState {
    poisoned: bool,
    set_state_on_drop_to: Cell<StateAndQueue>,
}

// Once 可能处于的四种状态，编码在 Once 结构体中 `state_and_queue` 的低位比特里。
// 把 COMPLETE 选为全零状态后，在某些平台上 `is_completed` 检查可以稍快一些。
const INCOMPLETE: usize = 0x3;
const POISONED: usize = 0x2;
const RUNNING: usize = 0x1;
const COMPLETE: usize = 0x0;

// 用于提取状态的掩码。若处于 RUNNING 状态，则其余所有比特构成等待者队列。
const STATE_MASK: usize = 0b11;
const QUEUE_MASK: usize = !STATE_MASK;

// 等待者链表中一个节点的表示，在 RUNNING 状态期间使用。
// 注意：`Waiter` 不能持有指向下一个线程的可变指针，因为那样 `wait` 既会交出
// 一个指向其 `Waiter` 节点的可变引用，又会保留一个用于检查 `signaled` 的共享
// 引用。我们改为持有共享引用并使用内部可变性。
#[repr(align(4))] // 确保最低两位空闲，可用作状态比特。
struct Waiter {
    thread: Thread,
    signaled: Atomic<bool>,
    next: Cell<*const Waiter>,
}

// 等待者链表的头部。
// 每个节点都是位于某个等待线程栈上的一个结构体。
// 当它被 drop 时（即也包括 panic 时）会唤醒这些等待者。
struct WaiterQueue<'a> {
    state_and_queue: &'a Atomic<*mut ()>,
    set_state_on_drop_to: StateAndQueue,
}

fn to_queue(current: StateAndQueue) -> *const Waiter {
    current.mask(QUEUE_MASK).cast()
}

fn to_state(current: StateAndQueue) -> usize {
    current.addr() & STATE_MASK
}

impl Once {
    #[inline]
    pub const fn new() -> Once {
        Once { state_and_queue: AtomicPtr::new(ptr::without_provenance_mut(INCOMPLETE)) }
    }

    #[inline]
    pub fn is_completed(&self) -> bool {
        // 一个 `Acquire` load 就足够了，因为它能让所有初始化操作对我们可见；
        // 而且这是一条快速路径（fast path），较弱的内存序有助于性能。这个
        // `Acquire` 与慢速路径上的 `Release` 操作建立同步关系。
        self.state_and_queue.load(Acquire).addr() == COMPLETE
    }

    #[inline]
    pub(crate) fn state(&mut self) -> OnceExclusiveState {
        match self.state_and_queue.get_mut().addr() {
            INCOMPLETE => OnceExclusiveState::Incomplete,
            POISONED => OnceExclusiveState::Poisoned,
            COMPLETE => OnceExclusiveState::Complete,
            _ => unreachable!("invalid Once state"),
        }
    }

    #[inline]
    pub(crate) fn set_state(&mut self, new_state: OnceExclusiveState) {
        *self.state_and_queue.get_mut() = match new_state {
            OnceExclusiveState::Incomplete => ptr::without_provenance_mut(INCOMPLETE),
            OnceExclusiveState::Poisoned => ptr::without_provenance_mut(POISONED),
            OnceExclusiveState::Complete => ptr::without_provenance_mut(COMPLETE),
        };
    }

    #[cold]
    #[track_caller]
    pub fn wait(&self, ignore_poisoning: bool) {
        let mut current = self.state_and_queue.load(Acquire);
        loop {
            let state = to_state(current);
            match state {
                COMPLETE => return,
                POISONED if !ignore_poisoning => {
                    // Panic 以传播毒化（poison）状态。
                    panic!("Once instance has previously been poisoned");
                }
                _ => {
                    current = wait(&self.state_and_queue, current, !ignore_poisoning);
                }
            }
        }
    }

    // 这是一个非泛型函数，目的是降低使用 `call_once` 时的单态化（monomorphization）
    // 开销（这个实现并不算简单或短小）。
    //
    // 此外，它被标记为 `#[cold]`，因为它确实应当是冷路径，这也有助于让 LLVM 知道
    // 对该函数的调用应当被排除在快速路径之外。本质上，这有助于让 LLVM 生成更顺直
    // 的直线代码。
    //
    // 最后，它接受 `FnMut` 而非 `FnOnce`，因为目前没有办法在不引入一些分配开销的
    // 前提下，通过虚分发（virtual dispatch）调用一个 `FnOnce`。
    #[cold]
    #[track_caller]
    pub fn call(&self, ignore_poisoning: bool, init: &mut dyn FnMut(&public::OnceState)) {
        let mut current = self.state_and_queue.load(Acquire);
        loop {
            let state = to_state(current);
            match state {
                COMPLETE => break,
                POISONED if !ignore_poisoning => {
                    // Panic 以传播毒化（poison）状态。
                    panic!("Once instance has previously been poisoned");
                }
                POISONED | INCOMPLETE => {
                    // 尝试把当前线程注册为处于 RUNNING 状态的那个线程。
                    if let Err(new) = self.state_and_queue.compare_exchange_weak(
                        current,
                        current.mask(QUEUE_MASK).wrapping_byte_add(RUNNING),
                        Acquire,
                        Acquire,
                    ) {
                        current = new;
                        continue;
                    }

                    // `waiter_queue` 会管理其他等待的线程，
                    // 并在 drop 时唤醒它们。
                    let mut waiter_queue = WaiterQueue {
                        state_and_queue: &self.state_and_queue,
                        set_state_on_drop_to: ptr::without_provenance_mut(POISONED),
                    };
                    // 运行初始化函数，并告知它我们是否处于毒化状态。
                    let init_state = public::OnceState {
                        inner: OnceState {
                            poisoned: state == POISONED,
                            set_state_on_drop_to: Cell::new(ptr::without_provenance_mut(COMPLETE)),
                        },
                    };
                    init(&init_state);
                    waiter_queue.set_state_on_drop_to = init_state.inner.set_state_on_drop_to.get();
                    return;
                }
                _ => {
                    // 所有其他取值都必定是 RUNNING，且高位比特中可能带有一个
                    // 指向等待者队列的指针。
                    assert!(state == RUNNING);
                    current = wait(&self.state_and_queue, current, true);
                }
            }
        }
    }
}

fn wait(
    state_and_queue: &Atomic<*mut ()>,
    mut current: StateAndQueue,
    return_on_poisoned: bool,
) -> StateAndQueue {
    let node = &Waiter {
        thread: thread::current_or_unnamed(),
        signaled: AtomicBool::new(false),
        next: Cell::new(ptr::null()),
    };

    loop {
        let state = to_state(current);
        let queue = to_queue(current);

        // 如果初始化已经结束，则返回。
        if state == COMPLETE || (return_on_poisoned && state == POISONED) {
            return current;
        }

        // 为当前线程更新该节点。
        node.next.set(queue);

        // 尝试把该节点插到链表头部，同时确保没有其他线程刚刚替换掉链表头。
        if let Err(new) = state_and_queue.compare_exchange_weak(
            current,
            ptr::from_ref(node).wrapping_byte_add(state) as StateAndQueue,
            Release,
            Acquire,
        ) {
            current = new;
            continue;
        }

        // 我们已经把自己入队，现在开始等待。
        // 关键在于：在被 signal 之前不能返回，否则我们会 drop 掉自己的 `Waiter`
        // 节点，从而在链表中留下一个空洞（以及一个悬垂引用）。通过不断地重新
        // park 自己直到被 signal，来防范虚假唤醒（spurious wakeup）。
        while !node.signaled.load(Acquire) {
            // 如果管理线程恰好在我们还没来得及 park 自己之前就 signal 并 unpark
            // 了我们，结果可能是这个线程永远不会被 unpark。幸运的是 `park` 自带
            // 这样的保证：如果在 park 之前一个未被 park 的线程刚收到过一次
            // `unpark`，那么它不会真的 park。关键在于，我们知道这次 `unpark` 必定
            // 发生在上面的 `compare_exchange_weak` 与此处之间，而那段代码中没有
            // 其他的 `park` 会窃取我们的令牌（token）。
            // SAFETY: 我们在上面是在当前线程上获取的这个句柄。
            unsafe { node.thread.park() }
        }

        return state_and_queue.load(Acquire);
    }
}

#[stable(feature = "std_debug", since = "1.16.0")]
impl fmt::Debug for Once {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Once").finish_non_exhaustive()
    }
}

impl Drop for WaiterQueue<'_> {
    fn drop(&mut self) {
        // 用我们最终的状态把原状态 swap 出来。
        let current = self.state_and_queue.swap(self.set_state_on_drop_to, AcqRel);

        // 我们应当只可能看到一个为 RUNNING 的旧状态。
        assert_eq!(current.addr() & STATE_MASK, RUNNING);

        // 遍历整个等待者链表并逐一唤醒它们（按 LIFO 顺序，最后注册的最先被唤醒）。
        unsafe {
            // 在设置 `node.signaled = true` 之后，如果恰好发生了一次虚假唤醒
            //（spurious wakeup），另一个线程随时可能释放 `node`。
            // 所以我们必须先取出 `thread` 字段并复制指向 `next` 的指针。
            let mut queue = to_queue(current);
            while !queue.is_null() {
                let next = (*queue).next.get();
                let thread = (*queue).thread.clone();
                (*queue).signaled.store(true, Release);
                thread.unpark();
                queue = next;
            }
        }
    }
}

impl OnceState {
    #[inline]
    pub fn is_poisoned(&self) -> bool {
        self.poisoned
    }

    #[inline]
    pub fn poison(&self) {
        self.set_state_on_drop_to.set(ptr::without_provenance_mut(POISONED));
    }
}
