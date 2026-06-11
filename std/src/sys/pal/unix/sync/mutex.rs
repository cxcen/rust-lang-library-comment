use super::super::cvt_nz;
use crate::cell::UnsafeCell;
use crate::io::Error;
use crate::mem::MaybeUninit;
use crate::pin::Pin;

pub struct Mutex {
    inner: UnsafeCell<libc::pthread_mutex_t>,
}

impl Mutex {
    pub fn new() -> Mutex {
        Mutex { inner: UnsafeCell::new(libc::PTHREAD_MUTEX_INITIALIZER) }
    }

    pub(super) fn raw(&self) -> *mut libc::pthread_mutex_t {
        self.inner.get()
    }

    /// # 安全性(Safety）
    /// 对于每个 `Self` 实例只能调用一次。
    pub unsafe fn init(self: Pin<&mut Self>) {
        // Issue #33770
        //
        // 用 PTHREAD_MUTEX_INITIALIZER 初始化的 pthread mutex 其类型为
        // PTHREAD_MUTEX_DEFAULT；当你已经持有锁、又试图从同一线程对它再次加锁
        // 时，该类型的行为是未定义的
        // (https://pubs.opengroup.org/onlinepubs/9699919799/functions/pthread_mutex_init.html)。
        // 即便 PTHREAD_MUTEX_DEFAULT == PTHREAD_MUTEX_NORMAL，情况依然如此
        // (https://github.com/rust-lang/rust/issues/33770#issuecomment-220847521)——
        // 在那种情况下，`pthread_mutexattr_settype(PTHREAD_MUTEX_DEFAULT)` 当然与
        // 把它设为 `PTHREAD_MUTEX_NORMAL` 等效，但若完全不设置任何模式，得到的
        // Mutex 在重新加锁时就是 UB。
        //
        // 实践中，glibc 利用这一未定义行为来实现硬件锁省略（hardware lock
        // elision），后者借助硬件事务内存（hardware transactional memory）来避免
        // 真正获取锁。在一个事务进行期间，该锁看起来是未上锁的。对其他线程来说
        // 这不成问题，因为一旦检测到冲突，事务内存就会中止（abort）；然而从同一
        // 线程重新加锁时，并不会产生任何 abort。
        //
        // 由于对同一个 mutex 加锁两次会导致出现两个相互别名（aliasing）的 &mut
        // 引用，我们改为用 PTHREAD_MUTEX_NORMAL 类型来创建该 mutex——它保证：若
        // 我们试图从同一线程重新加锁，便会死锁，从而避免未定义行为。
        unsafe {
            let mut attr = MaybeUninit::<libc::pthread_mutexattr_t>::uninit();
            cvt_nz(libc::pthread_mutexattr_init(attr.as_mut_ptr())).unwrap();
            let attr = AttrGuard(&mut attr);
            cvt_nz(libc::pthread_mutexattr_settype(
                attr.0.as_mut_ptr(),
                libc::PTHREAD_MUTEX_NORMAL,
            ))
            .unwrap();
            cvt_nz(libc::pthread_mutex_init(self.raw(), attr.0.as_ptr())).unwrap();
        }
    }

    /// # 安全性(Safety）
    /// * 若未对该实例调用过 `init`，则可重入加锁（reentrant locking）会导致未定义
    ///   行为。
    /// * 销毁一个已上锁的 mutex 会导致未定义行为。
    pub unsafe fn lock(self: Pin<&Self>) {
        #[cold]
        #[inline(never)]
        fn fail(r: i32) -> ! {
            let error = Error::from_raw_os_error(r);
            panic!("failed to lock mutex: {error}");
        }

        let r = unsafe { libc::pthread_mutex_lock(self.raw()) };
        // 由于我们在上面把 mutex 类型设成了 `PTHREAD_MUTEX_NORMAL`，我们预期这次
        // 加锁调用永远不会失败。然而遗憾的是，某些平台（Solaris）并不遵循标准，
        // 而是总会提供死锁检测。它们可真“贴心”！遗憾的是这意味着我们需要在这里
        // 检查错误码。为了让我们将来在其他不那么守规矩的平台上免遭 UB，我们即便
        // 在 macOS 这样“表现良好”的平台上也照做。更多背景参见 #120147。
        if r != 0 {
            fail(r)
        }
    }

    /// # 安全性(Safety）
    /// * 若未对该实例调用过 `init`，则可重入加锁（reentrant locking）会导致未定义
    ///   行为。
    /// * 销毁一个已上锁的 mutex 会导致未定义行为。
    pub unsafe fn try_lock(self: Pin<&Self>) -> bool {
        unsafe { libc::pthread_mutex_trylock(self.raw()) == 0 }
    }

    /// # 安全性(Safety）
    /// 该 mutex 必须已被当前线程锁定。
    pub unsafe fn unlock(self: Pin<&Self>) {
        let r = unsafe { libc::pthread_mutex_unlock(self.raw()) };
        debug_assert_eq!(r, 0);
    }
}

impl !Unpin for Mutex {}

unsafe impl Send for Mutex {}
unsafe impl Sync for Mutex {}

impl Drop for Mutex {
    fn drop(&mut self) {
        // SAFETY:
        // 如果调用过 `lock` 或 `init`，那么该 mutex 必定已被 pin 住，因此它仍处于
        // 同一位置。否则，`inner` 必定包含 `PTHREAD_MUTEX_INITIALIZER`，而后者在
        // 任意位置都是有效的。因此，这次调用始终销毁的是一个有效的 mutex。
        let r = unsafe { libc::pthread_mutex_destroy(self.raw()) };
        if cfg!(any(target_os = "aix", target_os = "dragonfly")) {
            // 在 AIX 和 DragonFly 上，如果对一个刚用 libc::PTHREAD_MUTEX_INITIALIZER
            // 初始化过的 mutex 调用 pthread_mutex_destroy()，它会返回 EINVAL。
            // 一旦该 mutex 被使用过（加锁/解锁）、或调用过 pthread_mutex_init()，
            // 这种行为便不再出现。
            debug_assert!(r == 0 || r == libc::EINVAL);
        } else {
            debug_assert_eq!(r, 0);
        }
    }
}

struct AttrGuard<'a>(pub &'a mut MaybeUninit<libc::pthread_mutexattr_t>);

impl Drop for AttrGuard<'_> {
    fn drop(&mut self) {
        unsafe {
            let result = libc::pthread_mutexattr_destroy(self.0.as_mut_ptr());
            assert_eq!(result, 0);
        }
    }
}
