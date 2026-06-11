//! 用于 Fuchsia 的优先级继承（priority inheriting）mutex。
//!
//! 这是对 [Fuchsia libsync 中的 mutex][mutex in Fuchsia's libsync] 的移植。
//! 与原版不同的是，当检测到可重入加锁（reentrant locking）时，它不会中止
//! 进程，而是死锁（deadlock）。
//!
//! 优先级继承是通过把持有锁的线程的句柄存放在一个原子变量中实现的。Fuchsia 的
//! futex 操作支持为一个 futex 设置 owner 线程，这样在该 futex 被等待期间可以
//! 提升那个线程的优先级。
//!
//! libsync 采用以下 BSD 风格许可证授权：
//!
//! Copyright 2016 The Fuchsia Authors.
//!
//! Redistribution and use in source and binary forms, with or without
//! modification, are permitted provided that the following conditions are
//! met:
//!
//!    * Redistributions of source code must retain the above copyright
//!      notice, this list of conditions and the following disclaimer.
//!    * Redistributions in binary form must reproduce the above
//!      copyright notice, this list of conditions and the following
//!      disclaimer in the documentation and/or other materials provided
//!      with the distribution.
//!
//! THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS
//! "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
//! LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
//! A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT
//! OWNER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
//! SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT
//! LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE,
//! DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY
//! THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
//! (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
//! OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.
//!
//! [mutex in Fuchsia's libsync]: https://cs.opensource.google/fuchsia/fuchsia/+/main:zircon/system/ulib/sync/mutex.c

use crate::sync::atomic::Ordering::{Acquire, Relaxed, Release};
use crate::sync::atomic::{Atomic, AtomicU32};
use crate::sys::fuchsia::{
    ZX_ERR_BAD_HANDLE, ZX_ERR_BAD_STATE, ZX_ERR_INVALID_ARGS, ZX_ERR_TIMED_OUT, ZX_ERR_WRONG_TYPE,
    ZX_OK, ZX_TIME_INFINITE, zx_futex_wait, zx_futex_wake_single_owner, zx_handle_t,
    zx_thread_self,
};

// 一个 `zx_handle_t` 的最低两位总是被置位的，所以最低位被用来通过清除它来把
// mutex 标记为存在争用（contested）。
const CONTESTED_BIT: u32 = 1;
// 这个值永远不可能是一个有效的 `zx_handle_t`。
const UNLOCKED: u32 = 0;

pub struct Mutex {
    futex: Atomic<u32>,
}

#[inline]
fn to_state(owner: zx_handle_t) -> u32 {
    owner
}

#[inline]
fn to_owner(state: u32) -> zx_handle_t {
    state | CONTESTED_BIT
}

#[inline]
fn is_contested(state: u32) -> bool {
    state & CONTESTED_BIT == 0
}

#[inline]
fn mark_contested(state: u32) -> u32 {
    state & !CONTESTED_BIT
}

impl Mutex {
    #[inline]
    pub const fn new() -> Mutex {
        Mutex { futex: AtomicU32::new(UNLOCKED) }
    }

    #[inline]
    pub fn try_lock(&self) -> bool {
        let thread_self = zx_thread_self();
        self.futex.compare_exchange(UNLOCKED, to_state(thread_self), Acquire, Relaxed).is_ok()
    }

    #[inline]
    pub fn lock(&self) {
        let thread_self = zx_thread_self();
        if let Err(state) =
            self.futex.compare_exchange(UNLOCKED, to_state(thread_self), Acquire, Relaxed)
        {
            unsafe {
                self.lock_contested(state, thread_self);
            }
        }
    }

    /// # 安全性(Safety）
    /// `thread_self` 必须是当前线程的句柄。
    #[cold]
    unsafe fn lock_contested(&self, mut state: u32, thread_self: zx_handle_t) {
        let owned_state = mark_contested(to_state(thread_self));
        loop {
            // 如果 mutex 尚未被标记为争用（contested），则将其标记为争用。
            let contested = mark_contested(state);
            if is_contested(state)
                || self.futex.compare_exchange(state, contested, Relaxed, Relaxed).is_ok()
            {
                // mutex 已被标记为争用，等待 state 改变。
                unsafe {
                    match zx_futex_wait(
                        &self.futex,
                        AtomicU32::new(contested),
                        to_owner(state),
                        ZX_TIME_INFINITE,
                    ) {
                        ZX_OK | ZX_ERR_BAD_STATE | ZX_ERR_TIMED_OUT => (),
                        // 注意：如果某个线程句柄在其关联线程退出（且未解锁 mutex）之后
                        // 被复用，那么本次等待可能会提升某个任意线程的优先级，但目前
                        // 没有办法防止这种情况。
                        ZX_ERR_INVALID_ARGS | ZX_ERR_BAD_HANDLE | ZX_ERR_WRONG_TYPE => {
                            panic!(
                                "either the current thread is trying to lock a mutex it has
                                already locked, or the previous owner did not unlock the mutex
                                before exiting"
                            )
                        }
                        error => panic!("unexpected error in zx_futex_wait: {error}"),
                    }
                }
            }

            // state 已改变或发生了一次唤醒，尝试对 mutex 加锁。
            match self.futex.compare_exchange(UNLOCKED, owned_state, Acquire, Relaxed) {
                Ok(_) => return,
                Err(updated) => state = updated,
            }
        }
    }

    #[inline]
    pub unsafe fn unlock(&self) {
        if is_contested(self.futex.swap(UNLOCKED, Release)) {
            // 被唤醒的线程会再次把 mutex 标记为争用，并返回到这里，持续唤醒
            // 直到再没有等待者为止；在那种情况下这就是一个空操作（noop）。
            self.wake();
        }
    }

    #[cold]
    fn wake(&self) {
        unsafe {
            zx_futex_wake_single_owner(&self.futex);
        }
    }
}
