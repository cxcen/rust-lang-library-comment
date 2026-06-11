//! 通过 `arc4random_buf` 生成随机数据。
//!
//! 与其名字相反，`arc4random` 实际上已不再使用那个糟糕透顶、漏洞百出的
//! RC4 密码（cypher），至少在现代系统上是这样，而是采用类似 ChaCha20、
//! 并不断从操作系统重新播种（reseeding）的算法。这使它成为获取大量
//! 密码学安全数据的理想来源——而这正是 `DefaultRandomSource` 所需要的。
//! 遗憾的是，它并非在所有 UNIX 系统上都可用，最显著的就是 Linux（直到近来才有，
//! 但它只是 `getrandom` 的一层封装。既然为了 `HashMap` 的键我们无论如何都需要
//! 直接挂接到 `getrandom`，那么我们就保留自己的版本）。

#[cfg(not(any(
    target_os = "haiku",
    target_os = "illumos",
    target_os = "solaris",
    target_os = "vita",
)))]
use libc::arc4random_buf;

// FIXME: 把这些移到 libc 中（Haiku 除外，那个需要链接到 libbsd.so）。
#[cfg(any(
    target_os = "haiku", // See https://git.haiku-os.org/haiku/tree/headers/compatibility/bsd/stdlib.h
    target_os = "illumos", // See https://www.illumos.org/man/3C/arc4random
    target_os = "solaris", // See https://docs.oracle.com/cd/E88353_01/html/E37843/arc4random-3c.html
    target_os = "vita", // See https://github.com/vitasdk/newlib/blob/b89e5bc183b516945f9ee07eef483ecb916e45ff/newlib/libc/include/stdlib.h#L74
))]
#[cfg_attr(target_os = "haiku", link(name = "bsd"))]
unsafe extern "C" {
    fn arc4random_buf(buf: *mut core::ffi::c_void, nbytes: libc::size_t);
}

pub fn fill_bytes(bytes: &mut [u8]) {
    unsafe { arc4random_buf(bytes.as_mut_ptr().cast(), bytes.len()) }
}
