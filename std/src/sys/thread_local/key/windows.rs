//! 针对 Windows 的 `LazyKey` 实现。
//!
//! Windows 没有运行析构函数（destructors）的原生支持，所以我们自己管理一份
//! 析构函数列表，以追踪如何销毁各个 key。随后我们安装一个回调，使其在任何线程
//! 退出时被调用，运行所有相应的析构函数（参见 [`guard`](guard) 模块文档）。
//!
//! 这一机制日后很可能需要改进，但本模块尝试实现一个“穷人版”的析构函数回调系统。
//! 一旦我们拿到要运行的列表，就遍历所有 key、检查它们的值，如果值非空就运行
//! 析构函数（在此之前先把它们置为空）。我们在一个循环中重复这样做几次，
//! 以基本匹配 Unix 的语义。如果短时间内仍未达到不动点（fixed point），
//! 那我们就只能不可避免地泄漏一些东西了。
//!
//! 该列表实现为一个由 `LazyKey` 组成的原子单链表，且不支持注销（unregistration）。
//! 遗憾的是，这意味着我们不能用竞争式（racy）初始化来创建 `LazyKey` 中的 key，
//! 因为那可能导致析构函数被遗漏。因此，我们通过 [`INIT_ONCE`](c::INIT_ONCE)
//! 来同步 key 的创建与析构函数（不能使用 `std` 的 [`Once`](crate::sync::Once)，
//! 因为它自身可能会使用 TLS）。对于没有析构函数的 key，竞争式初始化就足够了。

// FIXME：研究改用一个固定大小的数组，因为 key 的最大数量被
//        [限制为 1088](https://learn.microsoft.com/en-us/windows/win32/ProcThread/thread-local-storage)。

use crate::cell::UnsafeCell;
use crate::ptr;
use crate::sync::atomic::Ordering::{AcqRel, Acquire, Relaxed, Release};
use crate::sync::atomic::{Atomic, AtomicPtr, AtomicU32};
use crate::sys::c;
use crate::sys::thread_local::guard;

pub type Key = u32;
type Dtor = unsafe extern "C" fn(*mut u8);

pub struct LazyKey {
    /// 整体左移了一位（即加一）的 key 值。由于 TLS_OUT_OF_INDEXES == u32::MAX
    /// 不是一个有效的 key 值，这使我们能够把零用作哨兵值（sentinel value）
    /// 而不会有溢出风险。
    key: Atomic<Key>,
    dtor: Option<Dtor>,
    next: Atomic<*mut LazyKey>,
    /// 目前析构函数无法被注销，所以我们不能对 key 使用竞争式（racy）初始化。
    /// 相反，我们需要同步初始化过程。这里使用 Windows 提供的 `Once`，
    /// 因为它不需要 TLS。
    once: UnsafeCell<c::INIT_ONCE>,
}

impl LazyKey {
    #[inline]
    pub const fn new(dtor: Option<Dtor>) -> LazyKey {
        LazyKey {
            key: AtomicU32::new(0),
            dtor,
            next: AtomicPtr::new(ptr::null_mut()),
            once: UnsafeCell::new(c::INIT_ONCE_STATIC_INIT),
        }
    }

    #[inline]
    pub fn force(&'static self) -> Key {
        match self.key.load(Acquire) {
            0 => unsafe { self.init() },
            key => key - 1,
        }
    }

    #[cold]
    unsafe fn init(&'static self) -> Key {
        if self.dtor.is_some() {
            let mut pending = c::FALSE;
            let r = unsafe {
                c::InitOnceBeginInitialize(self.once.get(), 0, &mut pending, ptr::null_mut())
            };
            assert_eq!(r, c::TRUE);

            if pending == c::FALSE {
                // 其他某个线程已经初始化了该 key，加载它即可。
                self.key.load(Relaxed) - 1
            } else {
                let key = unsafe { c::TlsAlloc() };
                if key == c::TLS_OUT_OF_INDEXES {
                    // 由于我们会 abort 进程，因此无需唤醒正在等待的线程。
                    // 如果这里是 panic，则必须先唤醒它们以避免死锁。
                    rtabort!("out of TLS indexes");
                }

                unsafe {
                    register_dtor(self);
                }

                // 以 Release 存储 key 必须是我们做的最后一件事。
                // 这是因为在 `fn key()` 中，其他线程会对 key 执行 acquire 加载，
                // 如果它们看到了这次写入，就会完全绕过 `InitOnce`。因此我们需要
                // 通过 `key` 来建立同步。特别地，那次 acquire 加载必须
                // happen-after 上面的 register_dtor，以确保析构函数确实会被运行！
                self.key.store(key + 1, Release);

                let r = unsafe { c::InitOnceComplete(self.once.get(), 0, ptr::null_mut()) };
                debug_assert_eq!(r, c::TRUE);

                key
            }
        } else {
            // 如果没有需要清理的析构函数，我们就可以使用竞争式（racy）初始化。

            let key = unsafe { c::TlsAlloc() };
            if key == c::TLS_OUT_OF_INDEXES {
                rtabort!("out of TLS indexes");
            }

            match self.key.compare_exchange(0, key + 1, AcqRel, Acquire) {
                Ok(_) => key,
                Err(new) => unsafe {
                    // 其他某个线程抢先完成了初始化，所以销毁我们的 key 并使用它们的。
                    let r = c::TlsFree(key);
                    debug_assert_eq!(r, c::TRUE);
                    new - 1
                },
            }
        }
    }
}

unsafe impl Send for LazyKey {}
unsafe impl Sync for LazyKey {}

#[inline]
pub unsafe fn set(key: Key, val: *mut u8) {
    let r = unsafe { c::TlsSetValue(key, val.cast()) };
    debug_assert_eq!(r, c::TRUE);
}

#[inline]
pub unsafe fn get(key: Key) -> *mut u8 {
    unsafe { c::TlsGetValue(key).cast() }
}

static DTORS: Atomic<*mut LazyKey> = AtomicPtr::new(ptr::null_mut());

/// 每个 key 只应被调用一次，否则链表中可能出现循环或断裂。
unsafe fn register_dtor(key: &'static LazyKey) {
    guard::enable();

    let this = <*const LazyKey>::cast_mut(key);
    // 当我们以 release 内存序存储新的链表头时，使用 acquire 内存序来传递
    // 此前已注册的各个 key 所做的修改。
    let mut head = DTORS.load(Acquire);
    loop {
        key.next.store(head, Relaxed);
        match DTORS.compare_exchange_weak(head, this, Release, Acquire) {
            Ok(_) => break,
            Err(new) => head = new,
        }
    }
}

/// 此函数只会、也只能由 [`guard`] 中的析构函数回调来运行。
pub unsafe fn run_dtors() {
    for _ in 0..5 {
        let mut any_run = false;

        // 使用 acquire 内存序以观察到 key 的初始化。
        let mut cur = DTORS.load(Acquire);
        while !cur.is_null() {
            let pre_key = unsafe { (*cur).key.load(Acquire) };
            let dtor = unsafe { (*cur).dtor.unwrap() };
            cur = unsafe { (*cur).next.load(Relaxed) };

            // 在 LazyKey::init 中，我们是在设置 `key` 之前先注册析构函数的。
            // 所以如果某个线程的 `run_dtors` 与另一个线程在同一个 `LazyKey` 上
            // 执行 `init` 发生竞争，我们在这里可能会遇到值为 0 的 key。这意味着
            // 该 key 在本线程中从未被初始化，所以我们可以安全地跳过它。
            if pre_key == 0 {
                continue;
            }
            // 如果它非零，那么通过上面的 `Acquire` 加载，我们已经与所有与此 key
            // 相关的内容建立了同步。（目前尚不清楚这是否必要，因为 DTORS 上的
            // release-acquire 配对也建立了同步，但小心驶得万年船。）
            let key = pre_key - 1;

            let ptr = unsafe { c::TlsGetValue(key) };
            if !ptr.is_null() {
                unsafe {
                    c::TlsSetValue(key, ptr::null_mut());
                    dtor(ptr as *mut _);
                    any_run = true;
                }
            }
        }

        if !any_run {
            break;
        }
    }
}
