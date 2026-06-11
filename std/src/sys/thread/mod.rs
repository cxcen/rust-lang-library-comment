cfg_select! {
    target_os = "hermit" => {
        mod hermit;
        pub use hermit::{Thread, available_parallelism, sleep, yield_now, DEFAULT_MIN_STACK_SIZE};
        #[expect(dead_code)]
        mod unsupported;
        pub use unsupported::{current_os_id, set_name};
    }
    target_os = "motor" => {
        mod motor;
        pub use motor::*;
    }
    all(target_vendor = "fortanix", target_env = "sgx") => {
        mod sgx;
        pub use sgx::{Thread, current_os_id, sleep, yield_now, DEFAULT_MIN_STACK_SIZE};

        // SGX 应当保护飞地（enclave）内部数据不被外部攻击者获取，因此
        // 绝不能有任何数据泄漏给 OS，尤其不能在 SGX 线程名与 OS 线程名
        // 之间建立一一对应的映射。因此 `set_name` 被有意实现为 no-op（空操作）。
        //
        // 注意，内部可见的 SGX 线程名已经由与平台无关的 Rust 线程代码提供。
        // 这一点可以在 [`std::thread::tests::test_named_thread`] 测试中观察到，
        // 该测试在 SGX target 上原样运行即可通过。
        #[expect(dead_code)]
        mod unsupported;
        pub use unsupported::{available_parallelism, set_name};
    }
    target_os = "solid_asp3" => {
        mod solid;
        pub use solid::{Thread, sleep, yield_now, DEFAULT_MIN_STACK_SIZE};
        #[expect(dead_code)]
        mod unsupported;
        pub use unsupported::{available_parallelism, current_os_id, set_name};
    }
    target_os = "teeos" => {
        mod teeos;
        pub use teeos::{Thread, sleep, yield_now, DEFAULT_MIN_STACK_SIZE};
        #[expect(dead_code)]
        mod unsupported;
        pub use unsupported::{available_parallelism, current_os_id, set_name};
    }
    target_os = "uefi" => {
        mod uefi;
        pub use uefi::{available_parallelism, sleep};
        #[expect(dead_code)]
        mod unsupported;
        pub use unsupported::{Thread, current_os_id, set_name, yield_now, DEFAULT_MIN_STACK_SIZE};
    }
    any(target_family = "unix", target_os = "wasi") => {
        mod unix;
        pub use unix::{Thread, available_parallelism, current_os_id, sleep, yield_now, DEFAULT_MIN_STACK_SIZE};
        #[cfg(not(any(
            target_env = "newlib",
            target_os = "l4re",
            target_os = "emscripten",
            target_os = "redox",
            target_os = "hurd",
            target_os = "aix",
            target_os = "wasi",
        )))]
        pub use unix::set_name;
        #[cfg(any(
            target_os = "freebsd",
            target_os = "netbsd",
            target_os = "linux",
            target_os = "android",
            target_os = "solaris",
            target_os = "illumos",
            target_os = "dragonfly",
            target_os = "hurd",
            target_os = "fuchsia",
            target_os = "vxworks",
            target_os = "wasi",
        ))]
        pub use unix::sleep_until;
        #[expect(dead_code)]
        mod unsupported;
        #[cfg(any(
            target_env = "newlib",
            target_os = "l4re",
            target_os = "emscripten",
            target_os = "redox",
            target_os = "hurd",
            target_os = "aix",
            target_os = "wasi",
        ))]
        pub use unsupported::set_name;
    }
    target_os = "vexos" => {
        mod vexos;
        pub use vexos::{sleep, yield_now};
        #[expect(dead_code)]
        mod unsupported;
        pub use unsupported::{Thread, available_parallelism, current_os_id, set_name, DEFAULT_MIN_STACK_SIZE};
    }
    all(target_family = "wasm", target_feature = "atomics") => {
        mod wasm;
        pub use wasm::sleep;

        #[expect(dead_code)]
        mod unsupported;
        pub use unsupported::{Thread, available_parallelism, current_os_id, set_name, yield_now, DEFAULT_MIN_STACK_SIZE};
    }
    target_os = "windows" => {
        mod windows;
        pub use windows::{Thread, available_parallelism, current_os_id, set_name, set_name_wide, sleep, yield_now, DEFAULT_MIN_STACK_SIZE};
    }
    target_os = "xous" => {
        mod xous;
        pub use xous::{Thread, available_parallelism, sleep, yield_now, DEFAULT_MIN_STACK_SIZE};

        #[expect(dead_code)]
        mod unsupported;
        pub use unsupported::{current_os_id, set_name};
    }
    _ => {
        mod unsupported;
        pub use unsupported::{Thread, available_parallelism, current_os_id, set_name, sleep, yield_now, DEFAULT_MIN_STACK_SIZE};
    }
}

#[cfg(not(any(
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "linux",
    target_os = "android",
    target_os = "solaris",
    target_os = "illumos",
    target_os = "dragonfly",
    target_os = "hurd",
    target_os = "fuchsia",
    target_os = "vxworks",
    target_os = "wasi",
)))]
pub fn sleep_until(deadline: crate::time::Instant) {
    use crate::time::Instant;

    let now = Instant::now();

    if let Some(delay) = deadline.checked_duration_since(now) {
        sleep(delay);
    }
}
