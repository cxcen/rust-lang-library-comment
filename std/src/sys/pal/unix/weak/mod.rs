//! Unix 上对符号进行“弱链接（weak linkage）”的支持
//!
//! std 中我们做的某些 I/O 操作需要较新版本的操作系统，但目前我们仍需与较旧的
//! 发行版保持二进制兼容。为了在新功能可用时使用它，我们用本模块来做运行时探测。
//!
//! 一种可选方案是弱链接（weak linkage），但遗憾的是它实际上只在 ELF 上才真正
//! 可行。其余情况下，则改用 dlsym 在运行时取得符号的地址值。这样做也是为了与较旧
//! 版本的 glibc 兼容，并避免对 GLIBC_PRIVATE 符号产生依赖。它假定我们已经与该符号
//! 所在的库做了动态链接，不过对于 libpthread/libc 这类库来说目前总是如此。
//!
//! 很久以前这里曾对 __pthread_get_minstack 符号使用弱链接，但那导致 Debian 检测到
//! 了一个对 libc6 的、不必要地过于严格的版本化依赖（#23628），因为该符号属于
//! GLIBC_PRIVATE。现在我们改用 `dlsym` 在运行时查找该符号，以避免产生 ELF 的
//! 版本化依赖。

#![forbid(unsafe_op_in_unsafe_fn)]

cfg_select! {
    // 在非 ELF 目标上，使用 dlsym 这一对弱链接的近似实现。
    target_vendor = "apple" => {
        mod dlsym;
        pub(crate) use dlsym::weak;
    }

    // 某些目标根本不需要、也不支持弱链接……
    target_os = "espidf" => {}

    // ……但 ELF 目标支持真正的弱链接。
    _ => {
        // 控制每一次 `weak!` 调用涉及哪些目标的 `#[cfg]` 多种多样。与其试图把这
        // 一切统一起来，我们干脆允许某些 unix 目标完全不使用这个宏。
        #[cfg_attr(not(target_os = "linux"), allow(unused_macros, dead_code))]
        mod weak_linkage;
        #[cfg_attr(not(target_os = "linux"), allow(unused_imports))]
        pub(crate) use weak_linkage::weak;
    }
}

// GNU/Linux 需要使用 `dlsym` 这一变体，以避免链接到 glibc 的私有符号。
#[cfg(all(target_os = "linux", target_env = "gnu"))]
mod dlsym;
#[cfg(all(target_os = "linux", target_env = "gnu"))]
pub(crate) use dlsym::weak as dlsym;

#[cfg(any(target_os = "android", target_os = "linux"))]
mod syscall;
#[cfg(any(target_os = "android", target_os = "linux"))]
pub(crate) use syscall::syscall;
