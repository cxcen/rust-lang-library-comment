use crate::sync::atomic::Ordering::{Acquire, Relaxed, Release};
use crate::sys::futex::{Futex, Primitive, futex_wait, futex_wake, futex_wake_all};

pub struct RwLock {
    // state 由一个 30 位的读者计数器、一个 'readers waiting'（有读者在等待）标志位
    // 和一个 'writers waiting'（有写者在等待）标志位组成。
    // 第 0..30 位：
    //   0: 未锁定（Unlocked）
    //   1..=0x3FFF_FFFE: 被 N 个读者锁定
    //   0x3FFF_FFFF: 被写者锁定（Write locked）
    // 第 30 位：有读者正在这个 futex 上等待。
    // 第 31 位：有写者正在 writer_notify futex 上等待。
    state: Futex,
    // 用来通知写者的「条件变量」。
    // 每次 signal（信号通知）时自增。
    writer_notify: Futex,
}

const READ_LOCKED: Primitive = 1;
const MASK: Primitive = (1 << 30) - 1;
const WRITE_LOCKED: Primitive = MASK;
const DOWNGRADE: Primitive = READ_LOCKED.wrapping_sub(WRITE_LOCKED); // READ_LOCKED - WRITE_LOCKED
const MAX_READERS: Primitive = MASK - 1;
const READERS_WAITING: Primitive = 1 << 30;
const WRITERS_WAITING: Primitive = 1 << 31;

#[inline]
fn is_unlocked(state: Primitive) -> bool {
    state & MASK == 0
}

#[inline]
fn is_write_locked(state: Primitive) -> bool {
    state & MASK == WRITE_LOCKED
}

#[inline]
fn has_readers_waiting(state: Primitive) -> bool {
    state & READERS_WAITING != 0
}

#[inline]
fn has_writers_waiting(state: Primitive) -> bool {
    state & WRITERS_WAITING != 0
}

#[inline]
fn is_read_lockable(state: Primitive) -> bool {
    // 如果尝试加读锁会导致计数器溢出，本函数也会返回 false。
    //
    // 当有读者在等待时，我们不允许加读锁，即便锁当前是未锁定状态且没有写者在等待。
    // 唯一会出现这种情况的时机是在解锁之后：此时解锁线程可能正在唤醒写者，而写者
    // 的优先级高于读者。解锁线程会在需要时清除 readers waiting 位并唤醒读者。
    state & MASK < MAX_READERS && !has_readers_waiting(state) && !has_writers_waiting(state)
}

#[inline]
fn is_read_lockable_after_wakeup(state: Primitive) -> bool {
    // 对于「一个进入休眠的读者线程被 `downgrade` 调用唤醒 *之后* 能否加读锁」
    // 这一判断，我们做了特殊处理。
    //
    // `downgrade` 会唤醒所有读者并把锁置于读模式。因此此时应当没有读者在等待，
    // 且锁应当处于读锁定状态（既不是写锁定，也不是未锁定）。
    //
    // 注意我们这里并不检查是否有写者在等待。这是因为调用 `downgrade` 意味着调用者
    // 希望其他读者去读取受锁保护的值。如果我们在 `downgrade` 之后不允许读者先于写者
    // 获取锁，那么就只有最初那个写者能读到该值，从而违背了 `downgrade` 的本意。
    state & MASK < MAX_READERS
        && !has_readers_waiting(state)
        && !is_write_locked(state)
        && !is_unlocked(state)
}

#[inline]
fn has_reached_max_readers(state: Primitive) -> bool {
    state & MASK == MAX_READERS
}

impl RwLock {
    #[inline]
    pub const fn new() -> Self {
        Self { state: Futex::new(0), writer_notify: Futex::new(0) }
    }

    #[inline]
    pub fn try_read(&self) -> bool {
        self.state
            .fetch_update(Acquire, Relaxed, |s| is_read_lockable(s).then(|| s + READ_LOCKED))
            .is_ok()
    }

    #[inline]
    pub fn read(&self) {
        let state = self.state.load(Relaxed);
        if !is_read_lockable(state)
            || self
                .state
                .compare_exchange_weak(state, state + READ_LOCKED, Acquire, Relaxed)
                .is_err()
        {
            self.read_contended();
        }
    }

    /// # 安全性(Safety）
    ///
    /// 调用此函数时，`RwLock` 必须处于读锁定状态（N 个读者）。
    #[inline]
    pub unsafe fn read_unlock(&self) {
        let state = self.state.fetch_sub(READ_LOCKED, Release) - READ_LOCKED;

        // 一个读者不可能在一个读锁定的 RwLock 上等待，
        // 除非同时也有一个写者在等待。
        debug_assert!(!has_readers_waiting(state) || has_writers_waiting(state));

        // 如果我们是最后一个读者且有写者在等待，则唤醒一个写者。
        if is_unlocked(state) && has_writers_waiting(state) {
            self.wake_writer_or_readers(state);
        }
    }

    #[cold]
    fn read_contended(&self) {
        let mut has_slept = false;
        let mut state = self.spin_read();

        loop {
            // 如果我们刚刚被唤醒，先检查是否发生过 `downgrade` 调用。
            // 否则，如果可以加读锁，就加读锁。
            if (has_slept && is_read_lockable_after_wakeup(state)) || is_read_lockable(state) {
                match self.state.compare_exchange_weak(state, state + READ_LOCKED, Acquire, Relaxed)
                {
                    Ok(_) => return, // 加锁成功！
                    Err(s) => {
                        state = s;
                        continue;
                    }
                }
            }

            // 检查是否溢出。
            assert!(!has_reached_max_readers(state), "too many active read locks on RwLock");

            // 在进入休眠前，确保 readers waiting 位已被置位。
            if !has_readers_waiting(state) {
                if let Err(s) =
                    self.state.compare_exchange(state, state | READERS_WAITING, Relaxed, Relaxed)
                {
                    state = s;
                    continue;
                }
            }

            // 等待 state 改变。
            futex_wait(&self.state, state | READERS_WAITING, None);
            has_slept = true;

            // 被唤醒后再次自旋。
            state = self.spin_read();
        }
    }

    #[inline]
    pub fn try_write(&self) -> bool {
        self.state
            .fetch_update(Acquire, Relaxed, |s| is_unlocked(s).then(|| s + WRITE_LOCKED))
            .is_ok()
    }

    #[inline]
    pub fn write(&self) {
        if self.state.compare_exchange_weak(0, WRITE_LOCKED, Acquire, Relaxed).is_err() {
            self.write_contended();
        }
    }

    /// # 安全性(Safety）
    ///
    /// 调用此函数时，`RwLock` 必须处于写锁定状态（单个写者）。
    #[inline]
    pub unsafe fn write_unlock(&self) {
        let state = self.state.fetch_sub(WRITE_LOCKED, Release) - WRITE_LOCKED;

        debug_assert!(is_unlocked(state));

        if has_writers_waiting(state) || has_readers_waiting(state) {
            self.wake_writer_or_readers(state);
        }
    }

    /// # 安全性(Safety）
    ///
    /// 调用此函数时，`RwLock` 必须处于写锁定状态（单个写者）。
    #[inline]
    pub unsafe fn downgrade(&self) {
        // 清除所有写标志位，并加上一个读标志位。
        let state = self.state.fetch_add(DOWNGRADE, Release);
        debug_assert!(is_write_locked(state), "RwLock must be write locked to call `downgrade`");

        if has_readers_waiting(state) {
            // 由于我们持有独占锁，没有其他人能清除这个位。
            self.state.fetch_sub(READERS_WAITING, Relaxed);
            futex_wake_all(&self.state);
        }
    }

    #[cold]
    fn write_contended(&self) {
        let mut state = self.spin_write();

        let mut other_writers_waiting = 0;

        loop {
            // 如果它未锁定，我们就尝试加锁。
            if is_unlocked(state) {
                match self.state.compare_exchange_weak(
                    state,
                    state | WRITE_LOCKED | other_writers_waiting,
                    Acquire,
                    Relaxed,
                ) {
                    Ok(_) => return, // 加锁成功！
                    Err(s) => {
                        state = s;
                        continue;
                    }
                }
            }

            // 置位 waiting 位，表示我们正在等待它。
            if !has_writers_waiting(state) {
                if let Err(s) =
                    self.state.compare_exchange(state, state | WRITERS_WAITING, Relaxed, Relaxed)
                {
                    state = s;
                    continue;
                }
            }

            // 现在可能也有其他写者在等待，所以一旦我们成功加锁，
            // 应当确保保留该位。
            other_writers_waiting = WRITERS_WAITING;

            // 在检查 `state` 是否改变之前先读取通知计数器，
            // 以确保我们不会漏掉任何通知。
            let seq = self.writer_notify.load(Acquire);

            // 如果锁已经变为可用，或者 writers waiting 位已不再被置位，
            // 则不要进入休眠。
            state = self.state.load(Relaxed);
            if is_unlocked(state) || !has_writers_waiting(state) {
                continue;
            }

            // 等待 state 改变。
            futex_wait(&self.writer_notify, seq, None);

            // 被唤醒后再次自旋。
            state = self.spin_write();
        }
    }

    /// 在解锁后唤醒正在等待的线程。
    ///
    /// 如果读者和写者都在等待，这只会唤醒一个写者；但如果没有写者可唤醒，
    /// 则回退为唤醒读者。
    #[cold]
    fn wake_writer_or_readers(&self, mut state: Primitive) {
        assert!(is_unlocked(state));

        // 此刻 readers waiting 位随时可能被置位，
        // 因为只要有任何线程在等待，读者就会阻塞。
        // 不过写者会直接对锁加锁，而不管这些 waiting 位，
        // 所以我们不必担心 writer waiting 位。
        //
        // 如果在此期间锁被加锁了，我们什么都不用做，
        // 因为那个加锁的线程会在它解锁时负责唤醒等待者。

        // 如果只有写者在等待，唤醒其中一个。
        if state == WRITERS_WAITING {
            match self.state.compare_exchange(state, 0, Relaxed, Relaxed) {
                Ok(_) => {
                    self.wake_writer();
                    return;
                }
                Err(s) => {
                    // 也许现在也有一些读者在等待了。所以继续执行下一个 `if`。
                    state = s;
                }
            }
        }

        // 如果写者和读者都在等待，则让读者继续等待，
        // 只唤醒一个写者。
        if state == READERS_WAITING + WRITERS_WAITING {
            if self.state.compare_exchange(state, READERS_WAITING, Relaxed, Relaxed).is_err() {
                // 锁被加锁了。已经不关我们的事了。
                return;
            }
            if self.wake_writer() {
                return;
            }
            // 实际上并没有写者阻塞在 futex_wait 上，所以我们继续转去唤醒读者，
            // 因为我们无法确定是否真的通知到了某个写者。
            state = READERS_WAITING;
        }

        // 如果有读者在等待，把它们全部唤醒。
        if state == READERS_WAITING {
            if self.state.compare_exchange(state, 0, Relaxed, Relaxed).is_ok() {
                futex_wake_all(&self.state);
            }
        }
    }

    /// 唤醒一个写者；如果我们唤醒了一个原本阻塞在 futex_wait 上的写者，则返回 true。
    ///
    /// 如果返回 false，仍有可能我们通知到了一个正准备进入休眠的写者。
    fn wake_writer(&self) -> bool {
        self.writer_notify.fetch_add(1, Release);
        futex_wake(&self.writer_notify)
        // 注意 FreeBSD 和 DragonFlyBSD 不会告诉我们它们是否唤醒了任何线程，
        // 这里总是返回 `false`。这仍然能产生正确的行为：它只是意味着在读者和写者
        // 都在等待的情况下，读者也会一并被唤醒。
    }

    /// 自旋一段时间，但在满足给定条件时立即停止。
    #[inline]
    fn spin_until(&self, f: impl Fn(Primitive) -> bool) -> Primitive {
        let mut spin = 100; // 由一次公平掷骰子选定。
        loop {
            let state = self.state.load(Relaxed);
            if f(state) || spin == 0 {
                return state;
            }
            crate::hint::spin_loop();
            spin -= 1;
        }
    }

    #[inline]
    fn spin_write(&self) -> Primitive {
        // 当锁未锁定，或者有写者在等待时停止自旋，以保持一定程度的公平性。
        self.spin_until(|state| is_unlocked(state) || has_writers_waiting(state))
    }

    #[inline]
    fn spin_read(&self) -> Primitive {
        // 当锁未锁定或处于读锁定，或者有线程在等待时停止自旋。
        self.spin_until(|state| {
            !is_write_locked(state) || has_readers_waiting(state) || has_writers_waiting(state)
        })
    }
}
