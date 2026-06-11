//! 使用竞争式（racy）初始化的 `LazyKey` 实现。
//!
//! 遗憾的是，`std` 目前支持的平台中没有一个允许在编译期创建 TLS key。
//! 因此我们需要一种惰性（lazily）创建 key 的办法。我们没有使用像 `OnceLock`
//! 这样会阻塞的 API，而是采用竞争式初始化，它应当更轻量，并且避免了与 `std`
//! 其余部分之间的循环依赖。

use crate::sync::atomic::{Atomic, AtomicUsize, Ordering};

/// 用于静态分配的 TLS key 的类型。
///
/// 这基本上就是一个 `LazyLock<Key>`，但避免了阻塞以及与 `std` 其余部分之间的
/// 循环依赖。
pub struct LazyKey {
    /// 内部的静态 TLS key（内部实现细节）。
    key: Atomic<usize>,
    /// TLS 值的析构函数（destructor）。
    dtor: Option<unsafe extern "C" fn(*mut u8)>,
}

// 定义一个不太可能被当作 TLS key 返回的哨兵值（sentinel value）。
#[cfg(not(target_os = "nto"))]
const KEY_SENTVAL: usize = 0;
// 在 QNX Neutrino 上，当 key 当前未被使用时总是返回 0。
// 使用 0 将意味着总是创建两个 key，然后立即移除第一个（值为 0 的那个）。
#[cfg(target_os = "nto")]
const KEY_SENTVAL: usize = libc::PTHREAD_KEYS_MAX + 1;

impl LazyKey {
    pub const fn new(dtor: Option<unsafe extern "C" fn(*mut u8)>) -> LazyKey {
        LazyKey { key: AtomicUsize::new(KEY_SENTVAL), dtor }
    }

    #[inline]
    pub fn force(&self) -> super::Key {
        match self.key.load(Ordering::Acquire) {
            KEY_SENTVAL => self.lazy_init() as super::Key,
            n => n as super::Key,
        }
    }

    fn lazy_init(&self) -> usize {
        // POSIX 允许这里创建出来的 key 取值为 KEY_SENTVAL，但下面的 compare_exchange
        // 依赖于把 KEY_SENTVAL 用作哨兵值，以判断谁在设置这个共享 TLS key 的竞争中胜出。
        // 据我所知，并不存在一个保证不会被 posix_key_create 作为 key 返回的取值，
        // 因此也就没有任何值可以用来初始化内部 key 以证明它尚未被设置。
        // 因此，我们将继续使用 KEY_SENTVAL 这个值，但要做一些“折腾”来确保
        // 从创建例程返回的是一个非 KEY_SENTVAL 的值。
        // FIXME：这显然是个 hack，应当清理掉。
        let key1 = super::create(self.dtor);
        let key = if key1 as usize != KEY_SENTVAL {
            key1
        } else {
            let key2 = super::create(self.dtor);
            unsafe {
                super::destroy(key1);
            }
            key2
        };
        rtassert!(key as usize != KEY_SENTVAL);
        match self.key.compare_exchange(
            KEY_SENTVAL,
            key as usize,
            Ordering::Release,
            Ordering::Acquire,
        ) {
            // CAS 成功，所以我们创建的就是真正使用的 key
            Ok(_) => key as usize,
            // 如果有人抢先一步，就改用他们的 key
            Err(n) => unsafe {
                super::destroy(key);
                n
            },
        }
    }
}
