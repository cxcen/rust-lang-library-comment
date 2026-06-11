use crate::cell::Cell;
use crate::sync as public;
use crate::sync::atomic::Ordering::{Acquire, Relaxed, Release};
use crate::sync::once::OnceExclusiveState;
use crate::sys::futex::{Futex, Primitive, futex_wait, futex_wake_all};

// 在某些平台上，操作系统非常友好，会替我们管理等待者队列。
// 这意味着我们只需要一个原子量，它有 4 个状态：

/// 尚未运行过任何初始化，且当前没有线程在使用这个 Once。
const INCOMPLETE: Primitive = 3;
/// 之前曾有某个线程尝试初始化这个 Once，但它 panic 了，
/// 因此该 Once 现在已被毒化（poisoned）。当前没有其他线程在访问
/// 这个 Once。
const POISONED: Primitive = 2;
/// 当前有某个线程正在尝试运行初始化。它可能会成功，
/// 因此所有后续线程都需要等待它完成。
const RUNNING: Primitive = 1;
/// 初始化已经完成，所有后续调用都应立即返回。
/// 把这个状态选为全零状态后，在某些平台上 `is_completed` 检查
/// 可以稍快一些。
const COMPLETE: Primitive = 0;

// 另有一个额外的位指示是否存在正在等待的线程：

/// 仅当状态不为 COMPLETE 时才可置位。
const QUEUED: Primitive = 4;

// 线程通过置位 QUEUED 位并在 state 变量上调用 `futex_wait` 来等待。
// 当正在运行的线程结束时，它会用 `futex_wake_all` 唤醒所有等待的线程。

const STATE_MASK: Primitive = 0b11;

pub struct OnceState {
    poisoned: bool,
    set_state_to: Cell<Primitive>,
}

impl OnceState {
    #[inline]
    pub fn is_poisoned(&self) -> bool {
        self.poisoned
    }

    #[inline]
    pub fn poison(&self) {
        self.set_state_to.set(POISONED);
    }
}

struct CompletionGuard<'a> {
    state_and_queued: &'a Futex,
    set_state_on_drop_to: Primitive,
}

impl<'a> Drop for CompletionGuard<'a> {
    fn drop(&mut self) {
        // 使用 release 内存序，把改动传播给所有正在检查这个 Once 的线程。
        // `futex_wake_all` 会做自己的同步，因此我们不需要 `AcqRel`。
        if self.state_and_queued.swap(self.set_state_on_drop_to, Release) & QUEUED != 0 {
            futex_wake_all(self.state_and_queued);
        }
    }
}

pub struct Once {
    state_and_queued: Futex,
}

impl Once {
    #[inline]
    pub const fn new() -> Once {
        Once { state_and_queued: Futex::new(INCOMPLETE) }
    }

    #[inline]
    pub fn is_completed(&self) -> bool {
        // 使用 acquire 内存序，使所有初始化期间的改动对当前线程可见。
        self.state_and_queued.load(Acquire) == COMPLETE
    }

    #[inline]
    pub(crate) fn state(&mut self) -> OnceExclusiveState {
        match *self.state_and_queued.get_mut() {
            INCOMPLETE => OnceExclusiveState::Incomplete,
            POISONED => OnceExclusiveState::Poisoned,
            COMPLETE => OnceExclusiveState::Complete,
            _ => unreachable!("invalid Once state"),
        }
    }

    #[inline]
    pub(crate) fn set_state(&mut self, new_state: OnceExclusiveState) {
        *self.state_and_queued.get_mut() = match new_state {
            OnceExclusiveState::Incomplete => INCOMPLETE,
            OnceExclusiveState::Poisoned => POISONED,
            OnceExclusiveState::Complete => COMPLETE,
        };
    }

    #[cold]
    #[track_caller]
    pub fn wait(&self, ignore_poisoning: bool) {
        let mut state_and_queued = self.state_and_queued.load(Acquire);
        loop {
            let state = state_and_queued & STATE_MASK;
            let queued = state_and_queued & QUEUED != 0;
            match state {
                COMPLETE => return,
                POISONED if !ignore_poisoning => {
                    // Panic 以传播毒化（poison）状态。
                    panic!("Once instance has previously been poisoned");
                }
                _ => {
                    // 如果 QUEUED 位尚未置位，则将其置位。
                    if !queued {
                        state_and_queued += QUEUED;
                        if let Err(new) = self.state_and_queued.compare_exchange_weak(
                            state,
                            state_and_queued,
                            Relaxed,
                            Acquire,
                        ) {
                            state_and_queued = new;
                            continue;
                        }
                    }

                    futex_wait(&self.state_and_queued, state_and_queued, None);
                    state_and_queued = self.state_and_queued.load(Acquire);
                }
            }
        }
    }

    #[cold]
    #[track_caller]
    pub fn call(&self, ignore_poisoning: bool, f: &mut dyn FnMut(&public::OnceState)) {
        let mut state_and_queued = self.state_and_queued.load(Acquire);
        loop {
            let state = state_and_queued & STATE_MASK;
            let queued = state_and_queued & QUEUED != 0;
            match state {
                COMPLETE => return,
                POISONED if !ignore_poisoning => {
                    // Panic 以传播毒化（poison）状态。
                    panic!("Once instance has previously been poisoned");
                }
                INCOMPLETE | POISONED => {
                    // 尝试把当前线程注册为正在运行的那个线程。
                    let next = RUNNING + if queued { QUEUED } else { 0 };
                    if let Err(new) = self.state_and_queued.compare_exchange_weak(
                        state_and_queued,
                        next,
                        Acquire,
                        Acquire,
                    ) {
                        state_and_queued = new;
                        continue;
                    }

                    // `waiter_queue` 会管理其他等待的线程，
                    // 并在 drop 时唤醒它们。
                    let mut waiter_queue = CompletionGuard {
                        state_and_queued: &self.state_and_queued,
                        set_state_on_drop_to: POISONED,
                    };
                    // 运行该函数，并告知它我们是否处于毒化状态。
                    let f_state = public::OnceState {
                        inner: OnceState {
                            poisoned: state == POISONED,
                            set_state_to: Cell::new(COMPLETE),
                        },
                    };
                    f(&f_state);
                    waiter_queue.set_state_on_drop_to = f_state.inner.set_state_to.get();
                    return;
                }
                _ => {
                    // 所有其他取值都必定是 RUNNING。
                    assert!(state == RUNNING);

                    // 如果 QUEUED 位尚未置位，则将其置位。
                    if !queued {
                        state_and_queued += QUEUED;
                        if let Err(new) = self.state_and_queued.compare_exchange_weak(
                            state,
                            state_and_queued,
                            Relaxed,
                            Acquire,
                        ) {
                            state_and_queued = new;
                            continue;
                        }
                    }

                    futex_wait(&self.state_and_queued, state_and_queued, None);
                    state_and_queued = self.state_and_queued.load(Acquire);
                }
            }
        }
    }
}
