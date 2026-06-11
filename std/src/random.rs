//! 随机值生成。
//!
//! 本模块在 `core::random` 的基础上，额外提供了一个由操作系统熵源驱动的默认随机源
//! [`DefaultRandomSource`]，以及便捷函数 [`random`]。它本身不维护任何随机状态，
//! 所有随机字节都直接向底层平台索取；因此随机质量与各平台系统调用的保证一致
//! （见下表），失败/阻塞行为也由底层系统调用决定。

#[unstable(feature = "random", issue = "130703")]
pub use core::random::*;

use crate::sys::random as sys;

/// 默认随机源。
///
/// 它向操作系统索取适用于密码学用途（例如密钥生成）的随机数据。如果安全性是关注点，
/// 请查阅下文针对各平台的文档，了解你的目标平台具体提供哪些保证。
///
/// 该随机源提供的高质量随机性意味着在某些目标平台上它可能相当慢。如果你需要大量随机数
/// 且安全性并非关注点，可考虑改用其他随机数生成器（必要时可用本随机源为其播种）。
///
/// # 底层随机源
///
/// Platform               | Source
/// -----------------------|---------------------------------------------------------------
/// Linux                  | [`getrandom`] or [`/dev/urandom`] after polling `/dev/random`
/// Windows                | [`ProcessPrng`](https://learn.microsoft.com/en-us/windows/win32/seccng/processprng)
/// Apple                  | `CCRandomGenerateBytes`
/// DragonFly              | [`arc4random_buf`](https://man.dragonflybsd.org/?command=arc4random)
/// ESP-IDF                | [`esp_fill_random`](https://docs.espressif.com/projects/esp-idf/en/latest/esp32/api-reference/system/random.html#_CPPv415esp_fill_randomPv6size_t)
/// FreeBSD                | [`arc4random_buf`](https://man.freebsd.org/cgi/man.cgi?query=arc4random)
/// Fuchsia                | [`cprng_draw`](https://fuchsia.dev/reference/syscalls/cprng_draw)
/// Haiku                  | `arc4random_buf`
/// Illumos                | [`arc4random_buf`](https://www.illumos.org/man/3C/arc4random)
/// NetBSD                 | [`arc4random_buf`](https://man.netbsd.org/arc4random.3)
/// OpenBSD                | [`arc4random_buf`](https://man.openbsd.org/arc4random.3)
/// Solaris                | [`arc4random_buf`](https://docs.oracle.com/cd/E88353_01/html/E37843/arc4random-3c.html)
/// Vita                   | `arc4random_buf`
/// Hermit                 | `read_entropy`
/// Horizon, Cygwin        | `getrandom`
/// AIX, Hurd, L4Re, QNX   | `/dev/urandom`
/// Redox                  | `/scheme/rand`
/// RTEMS                  | [`arc4random_buf`](https://docs.rtems.org/branches/master/bsp-howto/getentropy.html)
/// SGX                    | [`rdrand`](https://en.wikipedia.org/wiki/RDRAND)
/// SOLID                  | `SOLID_RNG_SampleRandomBytes`
/// TEEOS                  | `TEE_GenerateRandom`
/// UEFI                   | [`EFI_RNG_PROTOCOL`](https://uefi.org/specs/UEFI/2.10/37_Secure_Technologies.html#random-number-generator-protocol)
/// VxWorks                | `randABytes` after waiting for `randSecure` to become ready
/// WASI                   | [`random_get`](https://github.com/WebAssembly/WASI/blob/main/legacy/preview1/docs.md#-random_getbuf-pointeru8-buf_len-size---result-errno)
/// ZKVM                   | `sys_rand`
///
/// 注意：所使用的随机源可能随时间变化。
///
/// 请查阅你所支持的目标平台上底层操作的文档，以确定它们是否提供某些你期望的特定属性，
/// 例如在虚拟机 fork 操作时重新播种（reseeding on VM fork）的支持。
///
/// [`getrandom`]: https://www.man7.org/linux/man-pages/man2/getrandom.2.html
/// [`/dev/urandom`]: https://www.man7.org/linux/man-pages/man4/random.4.html
#[derive(Default, Debug, Clone, Copy)]
#[unstable(feature = "random", issue = "130703")]
pub struct DefaultRandomSource;

#[unstable(feature = "random", issue = "130703")]
impl RandomSource for DefaultRandomSource {
    fn fill_bytes(&mut self, bytes: &mut [u8]) {
        sys::fill_bytes(bytes)
    }
}

/// 使用默认随机源，从某个分布中生成一个随机值。
///
/// 这是 `dist.sample(&mut DefaultRandomSource)` 的便捷封装，会按照底层 [`Distribution`]
/// trait 实现所定义的同一分布进行采样。关于随机性如何获取的更多信息，参见
/// [`DefaultRandomSource`]。
///
/// # 示例
///
/// 生成一个以文本表示的 [version 4/variant 1 UUID]：
/// ```
/// #![feature(random)]
///
/// use std::random::random;
///
/// let bits: u128 = random(..);
/// let g1 = (bits >> 96) as u32;
/// let g2 = (bits >> 80) as u16;
/// let g3 = (0x4000 | (bits >> 64) & 0x0fff) as u16;
/// let g4 = (0x8000 | (bits >> 48) & 0x3fff) as u16;
/// let g5 = (bits & 0xffffffffffff) as u64;
/// let uuid = format!("{g1:08x}-{g2:04x}-{g3:04x}-{g4:04x}-{g5:012x}");
/// println!("{uuid}");
/// ```
///
/// [version 4/variant 1 UUID]: https://en.wikipedia.org/wiki/Universally_unique_identifier#Version_4_(random)
#[unstable(feature = "random", issue = "130703")]
pub fn random<T>(dist: impl Distribution<T>) -> T {
    dist.sample(&mut DefaultRandomSource)
}
