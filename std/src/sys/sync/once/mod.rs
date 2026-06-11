// "once" 是一个相对简单的原语，而且通常操作系统本身也会提供它
//（参见 `pthread_once` 或 `InitOnceExecuteOnce`）。然而这些操作系统原语
// 往往带有一些出人意料的限制，例如 Unix 的版本不允许向回调函数传入参数。
//
// 因此，我们最终选择在标准库里自己实现它。这同时也给了我们一个机会去对实现
// 做一些优化，从而改善各调用点处的快速路径（fast path）性能。

cfg_select! {
    any(
        all(target_os = "windows", not(target_vendor="win7")),
        target_os = "linux",
        target_os = "android",
        all(target_arch = "wasm32", target_feature = "atomics"),
        target_os = "freebsd",
        target_os = "motor",
        target_os = "openbsd",
        target_os = "dragonfly",
        target_os = "fuchsia",
        target_os = "hermit",
    ) => {
        mod futex;
        pub use futex::{Once, OnceState};
    }
    any(
        windows,
        target_family = "unix",
        all(target_vendor = "fortanix", target_env = "sgx"),
        target_os = "solid_asp3",
        target_os = "xous",
    ) => {
        mod queue;
        pub use queue::{Once, OnceState};
    }
    _ => {
        mod no_threads;
        pub use no_threads::{Once, OnceState};
    }
}
