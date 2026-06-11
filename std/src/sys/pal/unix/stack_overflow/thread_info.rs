//! TLS，但是 async-signal-safe（异步信号安全）的。
//!
//! 遗憾的是，由于线程局部存储（thread local storage）并非 async-signal-safe，我们
//! 无法在栈溢出处理函数中健全地（soundly）使用它。虽然在大多数平台上这样用没有问题，
//! 但在另一些平台（例如 GNU/Linux）上却会导致未定义行为。所幸 POSIX 规范规定了两个
//! 可以在异步信号处理函数中访问的线程特定值：`pthread_self()` 的返回值，以及 `errno`
//! 的地址。由于 `pthread_t` 是一个不透明的、平台相关的类型，我们这里使用 `errno` 的
//! 地址。由于它是线程特定的、且在一个线程的整个生命周期内不会改变，我们便可以用
//! `&errno` 作为键，去索引一个存储线程特定数据的 `BTreeMap`。
//!
//! 对该 map 的并发访问由两把锁来同步——一把外层的 [`Mutex`]，以及一把内层的自旋锁
//! （spin lock），后者还会记住持锁者的身份：
//! * 自旋锁是主要的同步手段：由于它只使用原生原子操作（native atomics），它可以在信号
//!   处理函数内部被健全地使用，这一点与 [`Mutex`] 不同——后者可能并非 async-signal-safe。
//! * [`Mutex`] 防止在 setup 逻辑中忙等待（busy-waiting），因为那里的所有访问都是在持有
//!   [`Mutex`] 的情况下进行的，这使得自旋锁在常见情况下变得多余。
//! * 最后，通过把 `errno` 的地址用作自旋锁的“已上锁”值，我们就能检测出“在线程信息正被
//!   修改的过程中发生了 SIGSEGV”的情况。

use crate::collections::BTreeMap;
use crate::hint::spin_loop;
use crate::ops::Range;
use crate::sync::Mutex;
use crate::sync::atomic::{AtomicUsize, Ordering};
use crate::sys::io::errno_location;

pub struct ThreadInfo {
    pub tid: u64,
    pub name: Option<Box<str>>,
    pub guard_page_range: Range<usize>,
}

static LOCK: Mutex<()> = Mutex::new(());
static SPIN_LOCK: AtomicUsize = AtomicUsize::new(0);
// 这里使用 `BTreeMap` 而非哈希表，因为它支持常量初始化（constant initialization），
// 并且会在条目被移除时自动减少所占用的内存。
static mut THREAD_INFO: BTreeMap<usize, ThreadInfo> = BTreeMap::new();

struct UnlockOnDrop;

impl Drop for UnlockOnDrop {
    fn drop(&mut self) {
        SPIN_LOCK.store(0, Ordering::Release);
    }
}

/// 获取当前线程的信息（若可用）。
///
/// 调用本函数可能会冻结其他线程——如果它们正试图修改自身的线程信息的话。因此，调用方
/// 应当确保在调用本函数后不久进程便会 abort。
///
/// 只要 `f` 也是 async-signal-safe 的，本函数就保证是 async-signal-safe 的。
pub fn with_current_info<R>(f: impl FnOnce(Option<&ThreadInfo>) -> R) -> R {
    let this = errno_location().addr();
    let mut attempt = 0;
    let _guard = loop {
        // 如果我们只是在无休止地自旋，那很可能是正在修改线程信息 map 的那个线程
        // 优先级比我们低，并且在我们停止运行之前它都不会继续推进。这种情况下就
        // 干脆放弃。
        if attempt == 10_000_000 {
            rtprintpanic!("deadlock in SIGSEGV handler");
            return f(None);
        }

        match SPIN_LOCK.compare_exchange(0, this, Ordering::Acquire, Ordering::Relaxed) {
            Ok(_) => break UnlockOnDrop,
            Err(owner) if owner == this => {
                rtabort!("a thread received SIGSEGV while modifying its stack overflow information")
            }
            // 自旋直到能获取到锁——没有比这更好的办法了。遗憾的是这会造成一个优先级
            // 空洞（priority hole），但反正栈溢出本就是个致命错误。
            Err(_) => {
                spin_loop();
                attempt += 1;
            }
        }
    };

    // SAFETY: 我们持有自旋锁，所以 `THREAD_INFO` 不可能被别名（aliased）。
    let thread_info = unsafe { &*(&raw const THREAD_INFO) };
    f(thread_info.get(&this))
}

fn spin_lock_in_setup(this: usize) -> UnlockOnDrop {
    loop {
        match SPIN_LOCK.compare_exchange(0, this, Ordering::Acquire, Ordering::Relaxed) {
            Ok(_) => return UnlockOnDrop,
            Err(owner) if owner == this => {
                unreachable!("the thread info setup logic isn't recursive")
            }
            // 本函数总是在持有外层锁的情况下被调用，这意味着加锁唯一可能失败的
            // 时刻，就是另一个线程遭遇了栈溢出。由于那将使进程 abort，我们在此之前
            // 就让当前线程停下来。我们使用 `pause` 而非自旋，以避免优先级反转
            // （priority inversion）。
            // SAFETY: 这没有任何安全前置条件。
            Err(_) => drop(unsafe { libc::pause() }),
        }
    }
}

pub fn set_current_info(guard_page_range: Range<usize>) {
    let tid = crate::thread::current_os_id();
    let name = crate::thread::with_current_name(|name| name.map(Box::from));

    let this = errno_location().addr();
    let _lock_guard = LOCK.lock();
    let _spin_guard = spin_lock_in_setup(this);

    // SAFETY: 我们持有自旋锁，所以 `THREAD_INFO` 不可能被别名（aliased）。
    let thread_info = unsafe { &mut *(&raw mut THREAD_INFO) };
    thread_info.insert(this, ThreadInfo { tid, name, guard_page_range });
}

pub fn delete_current_info() {
    let this = errno_location().addr();
    let _lock_guard = LOCK.lock();
    let _spin_guard = spin_lock_in_setup(this);

    // SAFETY: 我们持有自旋锁，所以 `THREAD_INFO` 不可能被别名（aliased）。
    let thread_info = unsafe { &mut *(&raw mut THREAD_INFO) };
    thread_info.remove(&this);
}
