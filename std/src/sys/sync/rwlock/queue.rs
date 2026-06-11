//! 不依赖 `pthread_rwlock_t` 的高效读写锁。
//!
//! `pthread` 库提供的读写锁存在若干问题，使其对 `std` 而言并非最佳选择：
//!
//! * 它不可移动（non-movable），因此需要被分配（为了让构造函数是 `const`，
//! 采用惰性分配）。
//! * `pthread` 是一个外部库，这意味着「获取一个无争用锁」这条快速路径无法被内联。
//! * 某些平台（至少 2.25 之前版本的 glibc）的实现存在 bug，若未妥善防护，
//! 很容易在安全的 Rust 代码中导致未定义行为。
//! * 在某些平台上（例如 macOS），该锁非常慢。
//!
//! 因此，我们实现了自己的 [`RwLock`]！最朴素的做法是直接用自旋锁（spinlock），
//! 但当锁存在争用时，自旋锁可能相当 [有问题][problematic]。
//!
//! 我们的 [`RwLock`] 转而借鉴了 Windows [SRWLOCK] 和 [usync] 库实现所采用的策略。
//!
//! 快速路径仍然使用自旋，但它是有界的：自旋失败后，线程会以无锁（lockless）方式
//! 把一个信息结构体（[`Node`]，内含一个 [`Thread`] 句柄）加入到与该锁关联的等待者
//! 队列中。锁的持有者在释放锁时，会扫描这个队列并酌情唤醒线程，而新被唤醒的线程
//! 随后会自行尝试获取锁。
//!
//! 由此得到的 [`RwLock`] 具备以下特性：
//!
//! * 自适应（adaptive）：在执行任何重量级的 park 操作之前会先自旋
//! * 免分配（allocation-free）：除每线程的 [`Thread`] 句柄之外不另行分配，而该句柄
//! 在使用 `std` 创建的线程时本就会被分配
//! * 偏向写者（writer-preferring）：尽管仍可能有少数读者趁机插入
//! * 不公平（unfair）：这减少了上下文切换，从而大幅提升性能
//!
//! 在大多数情况下它也相当快。
//!
//! [problematic]: https://matklad.github.io/2020/01/02/spinlocks-considered-harmful.html
//! [SRWLOCK]: https://learn.microsoft.com/en-us/windows/win32/sync/slim-reader-writer--srw--locks
//! [usync]: https://crates.io/crates/usync
//!
//! # 实现（Implementation）
//!
//! ## 状态（State）
//!
//! 用一个 [`AtomicPtr`] 作为状态变量。最低的四个比特用来指示其余比特的含义：
//!
//! | [`LOCKED`] | [`QUEUED`] | [`QUEUE_LOCKED`] | [`DOWNGRADED`] | 剩余位（Remaining） |                                                                                                                             |
//! |------------|:-----------|:-----------------|:---------------|:-------------|:----------------------------------------------------------------------------------------------------------------------------|
//! | 0          | 0          | 0                | 0              | 0            | 锁处于未锁定状态，没有线程在等待                                                                                |
//! | 1          | 0          | 0                | 0              | 0            | 锁处于写锁定状态，没有线程在等待                                                                                |
//! | 1          | 0          | 0                | 0              | n > 0        | 锁处于读锁定状态，有 n 个读者                                                                                      |
//! | 0          | 1          | *                | 0              | `*mut Node`  | 锁处于未锁定状态，但有一些线程在等待。只有写者可以对锁加锁                                          |
//! | 1          | 1          | *                | *              | `*mut Node`  | 锁处于锁定状态，但有一些线程在等待。如果锁处于读锁定状态，则队列的最后一个节点存放读者计数 |
//!
//! ## 等待者队列（Waiter Queue）
//!
//! 当有线程在等待该锁时（`QUEUE` 位被置位），锁状态指向一个等待者队列，它被实现为
//! 一个存放在栈上的节点链表，以避免内存分配。
//!
//! 为了能够无锁地把新节点入队，该链表在创建时是单向链接（singly-linked）的。
//!
//! 当锁处于读锁定状态时，锁计数（读者数量）存放在队列的最后一个链节中。线程在释放锁
//! 时必须遍历队列以找到最后一个元素。为了避免每次想访问读者计数时都要遍历整个链表，
//! 我们会把找到的尾节点指针缓存在队列（当前的）第一个元素中。
//!
//! 此外，尽管出于性能原因这个锁是不公平的，但最好还是优先唤醒尾节点（FIFO 顺序）。
//! 由于我们总是从队列尾部弹出节点，因此我们必须存储指向前驱节点的反向链接
//!（backlink），这样才能更新队列（当前的）第一个元素的 `tail` 字段。添加反向链接
//! 与查找尾节点是同时进行的（通过函数 [`find_tail_and_add_backlinks`]），因此当
//! 在某个节点上遇到一个已设置的 tail 字段时，就表明队列中其后的所有节点都已初始化。
//!
//! 太长不看（TLDR）：下面是队列大致样子的示意图：
//!
//! ```text
//! state
//!   │
//!   ▼
//! ╭───────╮ next ╭───────╮ next ╭───────╮ next ╭───────╮
//! │       ├─────►│       ├─────►│       ├─────►│ count │
//! │       │      │       │      │       │      │       │
//! │       │      │       │◄─────┤       │◄─────┤       │
//! ╰───────╯      ╰───────╯ prev ╰───────╯ prev ╰───────╯
//!                      │                           ▲
//!                      └───────────────────────────┘
//!                                  tail
//! ```
//!
//! 不变量（Invariants）：
//! 1. 至少有一个节点必须含有一个非空且最新的 `tail` 字段。
//! 2. 第一个非空的 `tail` 字段必须有效且最新。
//! 3. 该节点之前的所有节点都必须含有正确且非空的 `next` 字段。
//! 4. 该节点之后的所有节点都必须含有正确且非空的 `prev` 字段。
//!
//! 对队列的访问由 `QUEUE_LOCKED` 位控制。线程会在两种情况下尝试置位该比特：一种是
//! 线程把自己入队并主动为队列添加反向链接时（这能大幅提升性能），另一种是线程解锁
//! 之后要唤醒下一个（或多个）等待者时。
//!
//! `QUEUE_LOCKED` 与入队/解锁操作同时被原子地置位。释放 `QUEUE_LOCKED` 位的线程
//! 会检查锁的状态（特别是，是否通过 [`DOWNGRADED`] 位请求了一次降级），并酌情唤醒
//! 等待者。这保证了即使解锁线程未能获取队列锁，整体也能向前推进（forward progress）。
//!
//! ## 内存序（Memory Orderings）
//!
//! 为了正确同步对受锁保护数据的改动，加锁和解锁分别使用 [`Acquire`] 和 [`Release`]
//! 内存序。为了传播节点的初始化，对队列锁的改动也使用这些内存序。

#![forbid(unsafe_op_in_unsafe_fn)]

use crate::cell::OnceCell;
use crate::hint::spin_loop;
use crate::mem;
use crate::ptr::{self, NonNull, null_mut, without_provenance_mut};
use crate::sync::atomic::Ordering::{AcqRel, Acquire, Relaxed, Release};
use crate::sync::atomic::{Atomic, AtomicBool, AtomicPtr};
use crate::thread::{self, Thread};

/// 原子的锁状态。
type AtomicState = Atomic<State>;
/// 内部的锁状态。
type State = *mut ();

const UNLOCKED: State = without_provenance_mut(0);
const LOCKED: usize = 1 << 0;
const QUEUED: usize = 1 << 1;
const QUEUE_LOCKED: usize = 1 << 2;
const DOWNGRADED: usize = 1 << 3;
const SINGLE: usize = 1 << 4;
const STATE: usize = DOWNGRADED | QUEUE_LOCKED | QUEUED | LOCKED;
const NODE_MASK: usize = !STATE;

/// 加锁采用指数退避（exponential backoff）。`SPIN_COUNT` 表示加锁操作会重试多少次。
///
/// 换句话说，`spin_loop` 会被调用 `2.pow(SPIN_COUNT) - 1` 次。
const SPIN_COUNT: usize = 7;

/// 在可能的情况下，把状态标记为写锁定。
#[inline]
fn write_lock(state: State) -> Option<State> {
    if state.addr() & LOCKED == 0 { Some(state.map_addr(|addr| addr | LOCKED)) } else { None }
}

/// 在可能的情况下，把状态标记为读锁定。
#[inline]
fn read_lock(state: State) -> Option<State> {
    if state.addr() & QUEUED == 0 && state.addr() != LOCKED {
        Some(without_provenance_mut(state.addr().checked_add(SINGLE)? | LOCKED))
    } else {
        None
    }
}

/// 在假定 state 指向一个队列节点的前提下，通过掩掉 state 的低位比特，把 `State`
/// 转换为一个 `Node`。
///
/// # 安全性(Safety）
///
/// state 必须含有一个指向有效队列节点的有效指针。
#[inline]
unsafe fn to_node(state: State) -> NonNull<Node> {
    unsafe { NonNull::new_unchecked(state.mask(NODE_MASK)).cast() }
}

/// 一个正在锁队列上等待的线程的表示。
///
/// 我们把这些 `Node` 初始化在线程的执行栈上，以避免分配。
///
/// 注意我们需要 16 字节的对齐，以确保任何指向 `Node` 的指针其最低 4 个比特
/// 始终为零（用于模块级文档中描述的那些位标志）。
#[repr(align(16))]
struct Node {
    next: AtomicLink,
    prev: AtomicLink,
    tail: AtomicLink,
    write: bool,
    thread: OnceCell<Thread>,
    completed: Atomic<bool>,
}

/// 一个使用 relaxed 操作的原子节点指针。
struct AtomicLink(Atomic<*mut Node>);

impl AtomicLink {
    fn new(v: Option<NonNull<Node>>) -> AtomicLink {
        AtomicLink(AtomicPtr::new(v.map_or(null_mut(), NonNull::as_ptr)))
    }

    fn get(&self) -> Option<NonNull<Node>> {
        NonNull::new(self.0.load(Relaxed))
    }

    fn set(&self, v: Option<NonNull<Node>>) {
        self.0.store(v.map_or(null_mut(), NonNull::as_ptr), Relaxed);
    }
}

impl Node {
    /// 创建一个新的队列节点。
    fn new(write: bool) -> Node {
        Node {
            next: AtomicLink::new(None),
            prev: AtomicLink::new(None),
            tail: AtomicLink::new(None),
            write,
            thread: OnceCell::new(),
            completed: AtomicBool::new(false),
        }
    }

    /// 为等待准备该节点。
    fn prepare(&mut self) {
        // 回退为创建一个未命名的 `Thread` 句柄，以允许在 TLS 析构函数中加锁。
        self.thread.get_or_init(thread::current_or_unnamed);
        self.completed = AtomicBool::new(false);
    }

    /// 等待，直到该节点被另一个线程标记为 [`complete`](Node::complete)（已完成）。
    ///
    /// # 安全性(Safety）
    ///
    /// 只能从创建该节点的那个线程调用。
    unsafe fn wait(&self) {
        while !self.completed.load(Acquire) {
            unsafe {
                self.thread.get().unwrap().park();
            }
        }
    }

    /// 原子地把该节点标记为已完成。
    ///
    /// # 安全性(Safety）
    ///
    /// `node` 必须指向一个有效的 `Node`，且该节点不得在本次调用之后仍然存活。
    unsafe fn complete(node: NonNull<Node>) {
        // 由于设置 completed 标志后该节点可能立即被销毁，所以要在那之前先克隆
        // 线程句柄。
        let thread = unsafe { node.as_ref().thread.get().unwrap().clone() };
        unsafe {
            node.as_ref().completed.store(true, Release);
        }
        thread.unpark();
    }
}

/// 遍历队列以找到尾节点，并在遍历的同时为队列添加反向链接（backlink）。
///
/// 只要队列没有被修改，本函数可以同时从多个线程调用（这种情况发生在解锁多个读者时）。
///
/// # 安全性(Safety）
///
/// * `head` 必须指向一个有效队列中的节点。
/// * `head` 必须位于「上一次移除操作所使用的前一个头节点」之前。
/// * 在本次调用期间，以 `head` 开头的那部分队列不得被修改。
unsafe fn find_tail_and_add_backlinks(head: NonNull<Node>) -> NonNull<Node> {
    let mut current = head;

    // 遍历队列，直到找到一个已设置了 `tail` 的节点。
    let tail = loop {
        let c = unsafe { current.as_ref() };
        if let Some(tail) = c.tail.get() {
            break tail;
        }

        // SAFETY: 在第一个设置了 `tail` 的节点之前，所有的 `next` 字段都非空且有效
        //（根据不变量 3）。
        unsafe {
            let next = c.next.get().unwrap_unchecked();
            next.as_ref().prev.set(Some(current));
            current = next;
        }
    };

    unsafe {
        head.as_ref().tail.set(Some(tail));
        tail
    }
}

/// 把队列中以 `tail` 结尾的所有线程都 [`complete`](Node::complete)（标记为完成）。
///
/// # 安全性(Safety）
///
/// * `tail` 必须是一个完全链接好的队列的有效尾节点。
/// * 当前线程必须对该队列拥有独占访问权。
unsafe fn complete_all(tail: NonNull<Node>) {
    let mut current = tail;

    // 反向遍历队列（FIFO），并 `complete` 其中所有节点。
    loop {
        let prev = unsafe { current.as_ref().prev.get() };
        unsafe {
            Node::complete(current);
        }
        match prev {
            Some(prev) => current = prev,
            None => return,
        }
    }
}

/// 一个守卫类型，用于防范因 panic 导致节点所在栈被展开（unwind）。
struct PanicGuard;

impl Drop for PanicGuard {
    fn drop(&mut self) {
        rtabort!("tried to drop node in intrusive list.");
    }
}

/// 对外暴露的内部 `RwLock` 类型。
pub struct RwLock {
    state: AtomicState,
}

impl RwLock {
    #[inline]
    pub const fn new() -> RwLock {
        RwLock { state: AtomicPtr::new(UNLOCKED) }
    }

    #[inline]
    pub fn try_read(&self) -> bool {
        self.state.fetch_update(Acquire, Relaxed, read_lock).is_ok()
    }

    #[inline]
    pub fn read(&self) {
        if !self.try_read() {
            self.lock_contended(false)
        }
    }

    #[inline]
    pub fn try_write(&self) -> bool {
        // 原子地置位 `LOCKED` 比特。在大多数现代处理器上，这会被降级（lower）为
        // 一条原子指令（例如 x86 上的 "lock bts"、现代 AArch64 上的 "ldseta"），
        // 因此比 `fetch_update(lock(true))` 更高效——后者在有新节点被追加到队列时
        // 可能会虚假失败（spuriously fail）。
        self.state.fetch_or(LOCKED, Acquire).addr() & LOCKED == 0
    }

    #[inline]
    pub fn write(&self) {
        if !self.try_write() {
            self.lock_contended(true)
        }
    }

    #[cold]
    fn lock_contended(&self, write: bool) {
        let mut node = Node::new(write);
        let mut state = self.state.load(Relaxed);
        let mut count = 0;
        let update_fn = if write { write_lock } else { read_lock };

        loop {
            // 乐观地更新状态。
            if let Some(next) = update_fn(state) {
                // 锁可用，尝试加锁。
                match self.state.compare_exchange_weak(state, next, Acquire, Relaxed) {
                    Ok(_) => return,
                    Err(new) => state = new,
                }
                continue;
            } else if state.addr() & QUEUED == 0 && count < SPIN_COUNT {
                // 如果锁不可用且没有线程在排队，则乐观地自旋一会儿，
                // 使用指数退避（exponential backoff）以减少缓存争用。
                for _ in 0..(1 << count) {
                    spin_loop();
                }
                state = self.state.load(Relaxed);
                count += 1;
                continue;
            }
            // 乐观路径均未成功，于是回退为把线程 park 起来。

            // 首先，准备好节点。
            node.prepare();

            // 如果有线程在排队，这会把 `next` 字段设置为指向队列中第一个节点的指针。
            // 如果状态是读锁定，这会把 `next` 设置为锁计数。
            // 如果是写锁定，则会把 `next` 设置为零。
            node.next.0 = AtomicPtr::new(state.mask(NODE_MASK).cast());
            node.prev = AtomicLink::new(None);

            // 置位 `QUEUED` 比特，并保留 `LOCKED` 和 `DOWNGRADED` 比特。
            let mut next = ptr::from_ref(&node)
                .map_addr(|addr| addr | QUEUED | (state.addr() & (DOWNGRADED | LOCKED)))
                as State;

            let mut is_queue_locked = false;
            if state.addr() & QUEUED == 0 {
                // 如果这是队列中的第一个节点，把 `tail` 字段设为节点自身，
                // 以确保队列中存在一个有效的 `tail` 字段（不变量 1 和 2）。
                // 这里需要用 `set`，以避免使新指针失效。
                node.tail.set(Some(NonNull::from(&node)));
            } else {
                // 否则，队列的尾节点是未知的。
                node.tail.set(None);

                // 尝试锁住队列，以便主动添加反向链接（backlink）。
                next = next.map_addr(|addr| addr | QUEUE_LOCKED);

                // 记录我们是否把 `QUEUE_LOCKED` 比特从关变为开。
                is_queue_locked = state.addr() & QUEUE_LOCKED == 0;
            }

            // 注册该节点，使用 release 内存序把我们的改动传播给唤醒方线程。
            if let Err(new) = self.state.compare_exchange_weak(state, next, AcqRel, Relaxed) {
                // 状态已改变，重试即可。
                state = new;
                continue;
            }
            // 节点已被注册，因此在其他线程可能访问该结构体期间，不得对其进行可变
            // 访问或销毁。

            // 用一个在 drop 时会 abort 的 `PanicGuard` 来防范栈展开（unwind）。
            let guard = PanicGuard;

            // 如果当前线程锁住了队列，则解锁它，以便主动添加反向链接。
            if is_queue_locked {
                // SAFETY: 本线程在上面置位了 `QUEUE_LOCKED` 比特。
                unsafe {
                    self.unlock_queue(next);
                }
            }

            // 等待，直到该节点被从队列中移除。
            // SAFETY: 该节点是由当前线程创建的。
            unsafe {
                node.wait();
            }

            // 节点已从队列中移除，解除该守卫的武装（disarm）。
            mem::forget(guard);

            // 重新加载状态并重试。
            state = self.state.load(Relaxed);
            count = 0;
        }
    }

    #[inline]
    pub unsafe fn read_unlock(&self) {
        match self.state.fetch_update(Release, Acquire, |state| {
            if state.addr() & QUEUED == 0 {
                // 如果没有线程在排队，只需简单地把读者计数减一。
                let count = state.addr() - (SINGLE | LOCKED);
                Some(if count > 0 { without_provenance_mut(count | LOCKED) } else { UNLOCKED })
            } else if state.addr() & DOWNGRADED != 0 {
                // 这个线程曾经拥有独占访问权，但请求了一次降级。该降级尚未完成，
                // 因此我们仍然拥有独占访问权。
                // 撤销降级请求并解锁，但把唤醒新线程的工作留给已经持有队列锁的那个线程。
                Some(state.mask(!(DOWNGRADED | LOCKED)))
            } else {
                None
            }
        }) {
            Ok(_) => {}
            // 有等待者在排队，且锁计数已被移动到队列尾部。
            Err(state) => unsafe { self.read_unlock_contended(state) },
        }
    }

    /// # 安全性(Safety）
    ///
    /// * 锁上必须有线程在排队。
    /// * `state` 必须是一个指向有效队列中某节点的指针。
    /// * 不能有正在进行中的 `downgrade`。
    #[cold]
    unsafe fn read_unlock_contended(&self, state: State) {
        // SAFETY:
        // 上面是以 acquire 内存序观测到该状态的，所以当前线程将已观测到所有节点的初始化。
        // 我们还知道没有线程能够修改以 `state` 开头的队列：因为只要锁上有任何线程在
        // 排队就无法获取新的读锁，所有队列锁的持有者都会在 `self.state` 中观测到一个
        // 已置位的 `LOCKED` 比特，从而不会修改队列。线程可能修改队列的另一种情形是
        // 正有一个降级在进行中（即移除整个队列），但既然那已属于本函数安全契约的一部分，
        // 我们就可以保证没有其他线程能修改队列。
        let tail = unsafe { find_tail_and_add_backlinks(to_node(state)).as_ref() };

        // 锁计数存放在 `tail` 的 `next` 字段中。
        // 把它减一，并通过使用 acquire-release 内存序来确保观测到其他锁持有者对队列
        // 所做的全部改动。
        let was_last = tail.next.0.fetch_byte_sub(SINGLE, AcqRel).addr() - SINGLE == 0;
        if was_last {
            // SAFETY: 当有线程在排队时，其他线程无法加读锁。此外，`LOCKED` 比特仍然
            // 被置位，因此没有写者。于是当前线程独占地拥有这个锁，尽管它是一个读者。
            unsafe { self.unlock_contended(state) }
        }
    }

    #[inline]
    pub unsafe fn write_unlock(&self) {
        if let Err(state) =
            self.state.compare_exchange(without_provenance_mut(LOCKED), UNLOCKED, Release, Relaxed)
        {
            // SAFETY: 由于其他线程无法获取该锁，状态发生改变的唯一原因只能是
            // 锁上有线程在排队。
            unsafe { self.unlock_contended(state) }
        }
    }

    /// # 安全性(Safety）
    ///
    /// * 锁必须由当前线程独占持有。
    /// * 锁上必须有线程在排队。
    /// * `state` 必须是一个指向有效队列中某节点的指针。
    /// * 不能有正在进行中的 `downgrade`。
    #[cold]
    unsafe fn unlock_contended(&self, state: State) {
        debug_assert_eq!(state.addr() & (DOWNGRADED | QUEUED | LOCKED), QUEUED | LOCKED);

        let mut current = state;

        // 我们希望原子地释放锁，并尝试获取队列锁。
        loop {
            // 首先检查队列锁是否已被持有。
            if current.addr() & QUEUE_LOCKED != 0 {
                // 另一个线程持有队列锁，于是让它替我们唤醒等待者。
                let next = current.mask(!LOCKED);
                match self.state.compare_exchange_weak(current, next, Release, Relaxed) {
                    Ok(_) => return,
                    Err(new) => {
                        current = new;
                        continue;
                    }
                }
            }

            // 原子地释放锁，并尝试获取队列锁。
            let next = current.map_addr(|addr| (addr & !LOCKED) | QUEUE_LOCKED);
            match self.state.compare_exchange_weak(current, next, AcqRel, Relaxed) {
                // 既然我们已经拿到了队列锁，就可以唤醒下一个等待者。
                Ok(_) => {
                    // SAFETY: 本线程刚刚获取了队列锁，且本函数的安全契约要求锁上已经
                    // 有线程在排队。
                    unsafe { self.unlock_queue(next) };
                    return;
                }
                Err(new) => current = new,
            }
        }
    }

    /// # 安全性(Safety）
    ///
    /// * 锁必须由当前线程写锁定。
    #[inline]
    pub unsafe fn downgrade(&self) {
        // 乐观地把状态从「单个写者、无等待者的写锁定」改为
        //「单个读者、无等待者的读锁定」。
        if let Err(state) = self.state.compare_exchange(
            without_provenance_mut(LOCKED),
            without_provenance_mut(SINGLE | LOCKED),
            Release,
            Relaxed,
        ) {
            // SAFETY: 状态会发生改变的唯一可能是有线程在排队。
            // 把它们全部唤醒。
            unsafe { self.downgrade_slow(state) }
        }
    }

    /// 在等待队列上有线程等待的情况下，把锁从写锁定降级为读锁定。
    ///
    /// 本函数要么唤醒等待队列上的所有等待者，要么指派当前队列锁的持有者去替它唤醒
    /// 所有等待者。一旦等待者被唤醒，它们会继续在 `lock_contended` 的执行循环中运行。
    ///
    /// # 安全性(Safety）
    ///
    /// * 锁必须由当前线程写锁定。
    /// * `state` 必须是一个指向有效队列中某节点的指针。
    /// * 锁上必须有线程在排队。
    #[cold]
    unsafe fn downgrade_slow(&self, mut state: State) {
        debug_assert_eq!(state.addr() & (DOWNGRADED | QUEUED | LOCKED), QUEUED | LOCKED);

        // 尝试通过接管整个等待者队列来唤醒所有等待者。
        loop {
            if state.addr() & QUEUE_LOCKED != 0 {
                // 另一个线程已经持有队列锁。告诉它去唤醒所有等待者。
                // 如果那个线程在我们释放自己的锁之前成功唤醒了等待者，其效果将与
                // 我们在下面改动状态完全相同。
                // 否则，`DOWNGRADED` 比特仍会被置位，这意味着当本线程稍后调用
                // `read_unlock` 时（因为它持有一个读锁，最终必须解锁），它会意识到
                // 锁仍处于独占锁定状态并据此行事。
                let next = state.map_addr(|addr| addr | DOWNGRADED);
                match self.state.compare_exchange_weak(state, next, Release, Relaxed) {
                    Ok(_) => return,
                    Err(new) => state = new,
                }
            } else {
                // 通过把 `state` 与一个单读者状态做 swap 来抓取整个队列。
                let next = ptr::without_provenance_mut(SINGLE | LOCKED);
                if let Err(new) = self.state.compare_exchange_weak(state, next, AcqRel, Relaxed) {
                    state = new;
                    continue;
                }

                // SAFETY: 现在我们完全拥有这个队列，所以没有其他人能修改它。
                let tail = unsafe { find_tail_and_add_backlinks(to_node(state)) };

                // 唤醒所有等待者。
                // SAFETY: `tail` 刚刚计算出来，意味着整个队列都已链接好，而且我们
                // 完全拥有该队列，因此拥有独占访问权。
                unsafe { complete_all(tail) };

                return;
            }
        }
    }

    /// 解锁队列。如果请求了降级，则唤醒所有线程；否则，如果锁处于未锁定状态，
    /// 则唤醒下一个（或多个）符合条件的线程。
    ///
    /// # 安全性(Safety）
    ///
    /// * 队列锁必须由当前线程持有。
    /// * `state` 必须是一个指向有效队列中某节点的指针。
    /// * 锁上必须有线程在排队。
    unsafe fn unlock_queue(&self, mut state: State) {
        debug_assert_eq!(state.addr() & (QUEUED | QUEUE_LOCKED), QUEUED | QUEUE_LOCKED);

        loop {
            // SAFETY: 既然我们持有队列锁，就没有其他人能修改队列。
            let tail = unsafe { find_tail_and_add_backlinks(to_node(state)) };

            if state.addr() & (DOWNGRADED | LOCKED) == LOCKED {
                // 另一个线程已经锁住了这个锁，且没有请求降级。
                // 通过释放队列锁，把唤醒等待者的工作留给它们。
                match self.state.compare_exchange_weak(
                    state,
                    state.mask(!QUEUE_LOCKED),
                    Release,
                    Acquire,
                ) {
                    Ok(_) => return,
                    Err(new) => {
                        state = new;
                        continue;
                    }
                }
            }

            // 由于我们持有队列锁，且在锁已经处于读锁定状态时无法请求降级，因此我们
            // 在此对队列拥有独占控制权，可以进行修改。

            let downgrade = state.addr() & DOWNGRADED != 0;
            let is_writer = unsafe { tail.as_ref().write };
            if !downgrade
                && is_writer
                && let Some(prev) = unsafe { tail.as_ref().prev.get() }
            {
                // 如果我们不是在降级，且下一个线程是写者，则只唤醒那一个写者线程。

                // 把 `tail` 切下来。
                // 在 `state` 所指向的节点之前没有任何已设置的 `tail` 链接，所以第一个
                // 非空的 tail 字段将是最新的（不变量 2）。
                // 我们也满足不变量 4，因为已经在这个节点上调用过 `find_tail`，从而
                // 确保所有反向链接都已设置。
                unsafe {
                    to_node(state).as_ref().tail.set(Some(prev));
                }

                // 尝试释放队列锁。我们需要再次检查状态，因为另一个线程可能已经获取了
                // 锁并请求了降级。
                let next = state.mask(!QUEUE_LOCKED);
                if let Err(new) = self.state.compare_exchange_weak(state, next, Release, Acquire) {
                    // 撤销上面对 tail 的修改，以便我们可以在上面重新查找尾节点。
                    // 如上所述，我们对队列拥有独占控制权，所以没有其他线程能注意到
                    // 这个改动。
                    unsafe {
                        to_node(state).as_ref().tail.set(Some(tail));
                    }
                    state = new;
                    continue;
                }

                // 尾节点已被切下，锁也已释放。把该节点标记为已完成。
                unsafe {
                    return Node::complete(tail);
                }
            } else {
                // 我们要么是在降级，要么下一个等待者是读者，要么队列只包含一个等待者。
                // 无论哪种情况，都直接唤醒所有线程。

                // 清空队列。
                let next =
                    if downgrade { ptr::without_provenance_mut(SINGLE | LOCKED) } else { UNLOCKED };
                if let Err(new) = self.state.compare_exchange_weak(state, next, Release, Acquire) {
                    state = new;
                    continue;
                }

                // SAFETY: 我们在上面已计算出 `tail`，且自那以后不可能有新节点被加入
                //（否则上面的 CAS 会失败）。
                // 因此我们对整个队列拥有完全控制权。
                unsafe {
                    return complete_all(tail);
                }
            }
        }
    }
}
