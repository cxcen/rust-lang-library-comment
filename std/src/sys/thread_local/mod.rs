//! `thread_local` 宏的实现。
//!
//! 存在三种不同的线程本地（thread-local）实现：
//! * 某些 target 缺乏线程支持，因而只有一个线程，所以 TLS 数据存放在
//!   一个普通的 `static` 中。
//! * 某些 target 通过动态链接器和 C 运行时原生支持 TLS。
//! * 在某些 target 上，OS 提供了基于库（library-based）的 TLS 实现。TLS 数据
//!   在堆上分配，并通过一个 TLS key 来引用。
//!
//! 每种实现都提供一个宏，用于生成引用该 TLS 变量所用的 `LocalKey` `const`，
//! 以及用于追踪该变量初始化/销毁状态的必要辅助结构体。
//!
//! 此外，本模块还包含用于这些实现所需的 OS 接口的抽象。

#![cfg_attr(test, allow(unused))]
#![doc(hidden)]
#![forbid(unsafe_op_in_unsafe_fn)]
#![unstable(
    feature = "thread_local_internals",
    reason = "internal details of the thread_local macro",
    issue = "none"
)]

cfg_select! {
    any(
        all(target_family = "wasm", not(target_feature = "atomics")),
        target_os = "uefi",
        target_os = "zkvm",
        target_os = "trusty",
        target_os = "vexos",
    ) => {
        mod no_threads;
        pub use no_threads::{EagerStorage, LazyStorage, thread_local_inner};
        pub(crate) use no_threads::{LocalPointer, local_pointer};
    }
    target_thread_local => {
        mod native;
        pub use native::{EagerStorage, LazyStorage, thread_local_inner};
        pub(crate) use native::{LocalPointer, local_pointer};
    }
    _ => {
        mod os;
        pub use os::{Storage, thread_local_inner, value_align};
        pub(crate) use os::{LocalPointer, local_pointer};
    }
}

/// 原生 TLS 实现需要一种为其数据注册析构函数（destructors）的方式。
/// 本模块包含该“注册”机制的平台相关实现。
///
/// 然而事实证明，大多数平台都没有办法为每个变量分别注册一个析构函数。
/// 在这些平台上，我们自己来追踪这些析构函数，并（通过 [`guard`] 模块）只注册
/// 一个回调，由它运行列表中的所有析构函数。
#[cfg(all(target_thread_local, not(all(target_family = "wasm", not(target_feature = "atomics")))))]
pub(crate) mod destructors {
    cfg_select! {
        any(
            target_os = "linux",
            target_os = "android",
            target_os = "fuchsia",
            target_os = "redox",
            target_os = "hurd",
            target_os = "netbsd",
            target_os = "dragonfly"
        ) => {
            mod linux_like;
            mod list;
            pub(super) use linux_like::register;
            pub(super) use list::run;
        }
        _ => {
            mod list;
            pub(super) use list::register;
            pub(crate) use list::run;
        }
    }
}

/// 本模块提供一种机制，用于调度析构函数列表以及
/// [运行时清理](crate::rt::thread_cleanup) 函数的执行。调用 `enable`
/// 应当确保这些函数在恰当的时机被调用。
pub(crate) mod guard {
    cfg_select! {
        all(target_thread_local, target_vendor = "apple") => {
            mod apple;
            pub(crate) use apple::enable;
        }
        target_os = "windows" => {
            mod windows;
            pub(crate) use windows::enable;
        }
        any(
            all(target_family = "wasm", not(
                all(target_os = "wasi", target_env = "p1", target_feature = "atomics")
            )),
            target_os = "uefi",
            target_os = "zkvm",
            target_os = "trusty",
            target_os = "vexos",
        ) => {
            pub(crate) fn enable() {
                // FIXME：目前在 wasm 上还没有“线程退出（thread exit）”的概念，
                // 但这一概念很可能会在某个时间点以一个导出符号（exported symbol）
                // 的形式出现，wasm 运行时被期望去调用它。目前我们只是把一切都泄漏掉，
                // 但如果这样一个函数开始存在，它大概需要用下面这些函数来遍历
                // 析构函数列表：
                #[cfg(all(target_family = "wasm", target_feature = "atomics"))]
                #[allow(unused)]
                use super::destructors::run;
                #[allow(unused)]
                use crate::rt::thread_cleanup;
            }
        }
        any(
            target_os = "hermit",
            target_os = "xous",
        ) => {
            // `std` 是唯一的运行时，所以时机到来时它就自己调用析构函数。
            pub(crate) fn enable() {}
        }
        target_os = "solid_asp3" => {
            mod solid;
            pub(crate) use solid::enable;
        }
        _ => {
            mod key;
            pub(crate) use key::enable;
        }
    }
}

/// 可在 `const` 上下文中创建的 TLS key。
///
/// 大多数没有原生 TLS 的 OS 都会提供一种基于库（library-based）的方式来创建
/// TLS 存储。对每个 TLS 变量，我们创建一个 key，随后可用它来引用线程本地表
///（thread-local table）中的某个条目。这样就把每个 key 与一个指针关联起来，
/// 我们可以读取和设置该指针以存放我们的数据。
pub(crate) mod key {
    cfg_select! {
        any(
            all(
                not(target_vendor = "apple"),
                not(target_family = "wasm"),
                target_family = "unix",
            ),
            all(not(target_thread_local), target_vendor = "apple"),
            target_os = "teeos",
            all(target_os = "wasi", target_env = "p1", target_feature = "atomics"),
        ) => {
            mod racy;
            mod unix;
            #[cfg(test)]
            mod tests;
            pub(super) use racy::LazyKey;
            pub(super) use unix::{Key, set};
            #[cfg(any(not(target_thread_local), test))]
            pub(super) use unix::get;
            use unix::{create, destroy};
        }
        all(not(target_thread_local), target_os = "windows") => {
            #[cfg(test)]
            mod tests;
            mod windows;
            pub(super) use windows::{Key, LazyKey, get, run_dtors, set};
        }
        all(target_vendor = "fortanix", target_env = "sgx") => {
            mod racy;
            mod sgx;
            #[cfg(test)]
            mod tests;
            pub(super) use racy::LazyKey;
            pub(super) use sgx::{Key, get, set};
            use sgx::{create, destroy};
        }
        target_os = "xous" => {
            mod racy;
            #[cfg(test)]
            mod tests;
            mod xous;
            pub(super) use racy::LazyKey;
            pub(crate) use xous::destroy_tls;
            pub(super) use xous::{Key, get, set};
            use xous::{create, destroy};
        }
        target_os = "motor" => {
            mod racy;
            #[cfg(test)]
            mod tests;
            pub(super) use racy::LazyKey;
            pub(super) use moto_rt::tls::{Key, get, set};
            use moto_rt::tls::{create, destroy};
        }
        _ => {}
    }
}

/// 在一个绝不能 unwind（栈展开）的场景中运行回调（例如在用户 crate 中声明的
/// `extern "C" fn`）。如果回调仍然发生了 unwind，则 `rtabort`，并附带一条关于
/// 线程本地变量在 drop 时 panic 的消息。
#[inline]
#[allow(dead_code)]
fn abort_on_dtor_unwind(f: impl FnOnce()) {
    // 像这样使用一个守卫（guard）开销更低。
    let guard = DtorUnwindGuard;
    f();
    core::mem::forget(guard);

    struct DtorUnwindGuard;
    impl Drop for DtorUnwindGuard {
        #[inline]
        fn drop(&mut self) {
            // 这条消息算不上很详细，但也不需要详细，因为此时我们已经
            // 打印过一条 panic 消息了。
            rtabort!("thread local panicked on drop");
        }
    }
}
