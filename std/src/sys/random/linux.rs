//! 使用 Linux 内核生成随机数据。
//!
//! Linux 上最早引入的随机数据接口是 `/dev/random` 和 `/dev/urandom`
//! 这两个特殊文件。由于在 chroot 内部、以及文件描述符耗尽时这些路径可能
//! 变得不可达，单凭它们还不足以为用户空间提供一个可靠的随机性来源。因此，
//! 当 OpenBSD 5.6 引入 `getentropy` 系统调用后，Linux 3.17 也得到了与之对应的、
//! 自己的 `getrandom` 系统调用。[^1] 遗憾的是，即便我们支持的最低版本足够高，
//! 我们仍然不能依赖该系统调用一定可用，因为它在 `seccomp` 中默认是被屏蔽的。
//!
//! 于是问题就变成了该使用哪一个随机数来源。从历史上看，内核包含两个池：
//! 阻塞池（blocking pool）与非阻塞池（non-blocking pool）。阻塞池使用熵估计
//! （entropy estimation）来限制可用字节的数量；而非阻塞池一旦用阻塞池完成初始化，
//! 就会使用一个 CPRNG 来返回无限量的随机字节。然而，只要 CPRNG 足够强，
//! 熵估计对安全性的贡献其实并不大，反倒成了一个绝佳的 DoS 攻击载体。因此，
//! 阻塞池在内核 5.6 版本中被移除了。[^2] 不过那个补丁并没有神奇地提升非阻塞池的
//! 质量，所以我们可以放心地认为即便在更老的内核版本中它也足够强，并无条件地使用它。
//!
//! 还需额外考虑一点：非阻塞池在早期启动（early boot）阶段并不总是已被初始化。
//! 对于 `DefaultRandomSource` 的输出，我们希望获得最佳质量的随机性，因此我们干脆
//! 等到它完成初始化为止。然而对于 `HashMap` 的键来说，这就构成了潜在的死锁来源，
//! 因为额外的熵可能只有在程序向前推进（forward progress）后才会生成。在那种情况下，
//! 我们就只用系统当时所能提供的最佳随机数据。
//!
//! 因此结论是：我们始终想要非阻塞池的输出，但可能需要等到它完成初始化。`getrandom`
//! 的默认行为是等待非阻塞池初始化完成，然后从那里取数据；所以如果 `getrandom` 可用，
//! 我们就用它的默认行为来生成字节。然而对于 `HashMap`，我们需要指定 `GRND_INSECURE`
//! 标志，但该标志只有从内核 5.6 版本起才可用。因此，如果我们检测到该标志不受支持，
//! 就改试 `GRND_NONBLOCK`，它只有在池已初始化时才会成功。如果池尚未初始化，
//! 我们就回退到文件访问的方法。
//!
//! `/dev/urandom` 的行为与 `getrandom` 相反：它总是产出数据，即便池尚未初始化。
//! 对于生成 `HashMap` 的键，这一点无关紧要，所以我们可以直接使用它。但对于安全数据，
//! 我们需要等到初始化完成，这可以通过 `poll` `/dev/random` 来做到。
//!
//! 总结（TLDR）：我们的回退策略如下：
//!
//! 安全数据                                       | `HashMap` 键
//! --------------------------------------------|------------------
//! getrandom(0)                                | getrandom(GRND_INSECURE)
//! poll("/dev/random") && read("/dev/urandom") | getrandom(GRND_NONBLOCK)
//!                                             | read("/dev/urandom")
//!
//! [^1]: <https://lwn.net/Articles/606141/>
//! [^2]: <https://lwn.net/Articles/808575/>
//!
// FIXME(2040 年左右）：一旦最低内核版本达到 5.6，就移除
// `GRND_NONBLOCK` 回退；并在需要安全数据时使用 `/dev/random`
// 而非 `/dev/urandom`。

use crate::fs::File;
use crate::io::Read;
use crate::os::fd::AsRawFd;
use crate::sync::OnceLock;
use crate::sync::atomic::Ordering::{Acquire, Relaxed, Release};
use crate::sync::atomic::{Atomic, AtomicBool};
use crate::sys::io::errno;
use crate::sys::pal::weak::syscall;

fn getrandom(mut bytes: &mut [u8], insecure: bool) {
    // 弱符号（weak symbol）允许被插桩替换（interposition），例如某些性能测量
    // 想要为了一致性而禁用随机性。否则我们会尝试发起一次原始系统调用。
    //（`getrandom` 在 glibc 2.25、musl 1.1.20、android API level 28 中被加入）
    syscall!(
        fn getrandom(
            buffer: *mut libc::c_void,
            length: libc::size_t,
            flags: libc::c_uint,
        ) -> libc::ssize_t;
    );

    static GETRANDOM_AVAILABLE: Atomic<bool> = AtomicBool::new(true);
    static GRND_INSECURE_AVAILABLE: Atomic<bool> = AtomicBool::new(true);
    static URANDOM_READY: Atomic<bool> = AtomicBool::new(false);
    static DEVICE: OnceLock<File> = OnceLock::new();

    if GETRANDOM_AVAILABLE.load(Relaxed) {
        loop {
            if bytes.is_empty() {
                return;
            }

            let flags = if insecure {
                if GRND_INSECURE_AVAILABLE.load(Relaxed) {
                    libc::GRND_INSECURE
                } else {
                    libc::GRND_NONBLOCK
                }
            } else {
                0
            };

            let ret = unsafe { getrandom(bytes.as_mut_ptr().cast(), bytes.len(), flags) };
            if ret != -1 {
                bytes = &mut bytes[ret as usize..];
            } else {
                match errno() {
                    libc::EINTR => continue,
                    // `GRND_INSECURE` 不可用，改试
                    // `GRND_NONBLOCK`。
                    libc::EINVAL if flags == libc::GRND_INSECURE => {
                        GRND_INSECURE_AVAILABLE.store(false, Relaxed);
                        continue;
                    }
                    // 池尚未初始化，暂且回退到
                    // /dev/urandom。
                    libc::EAGAIN if flags == libc::GRND_NONBLOCK => break,
                    // `getrandom` 不可用，或被 seccomp 屏蔽。
                    // 不再尝试它，回退到 /dev/urandom。
                    libc::ENOSYS | libc::EPERM => {
                        GETRANDOM_AVAILABLE.store(false, Relaxed);
                        break;
                    }
                    _ => panic!("failed to generate random data"),
                }
            }
        }
    }

    // 当我们需要密码学强度时，需要等待 CPRNG 池
    // 完成初始化。做法是 poll `/dev/random` 直到它就绪。
    if !insecure {
        if !URANDOM_READY.load(Acquire) {
            let random = File::open("/dev/random").expect("failed to open /dev/random");
            let mut fd = libc::pollfd { fd: random.as_raw_fd(), events: libc::POLLIN, revents: 0 };

            while !URANDOM_READY.load(Acquire) {
                let ret = unsafe { libc::poll(&mut fd, 1, -1) };
                match ret {
                    1 => {
                        assert_eq!(fd.revents, libc::POLLIN);
                        URANDOM_READY.store(true, Release);
                        break;
                    }
                    -1 if errno() == libc::EINTR => continue,
                    _ => panic!("poll(\"/dev/random\") failed"),
                }
            }
        }
    }

    DEVICE
        .get_or_try_init(|| File::open("/dev/urandom"))
        .and_then(|mut dev| dev.read_exact(bytes))
        .expect("failed to generate random data");
}

pub fn fill_bytes(bytes: &mut [u8]) {
    getrandom(bytes, false);
}

pub fn hashmap_random_keys() -> (u64, u64) {
    let mut bytes = [0; 16];
    getrandom(&mut bytes, true);
    let k1 = u64::from_ne_bytes(bytes[..8].try_into().unwrap());
    let k2 = u64::from_ne_bytes(bytes[8..].try_into().unwrap());
    (k1, k2)
}
