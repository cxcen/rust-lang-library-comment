//! 操作系统相关的功能。

#![stable(feature = "os", since = "1.0.0")]
#![allow(missing_docs, nonstandard_style, missing_debug_implementations)]
#![allow(unsafe_op_in_unsafe_fn)]

pub mod raw;

// 下面的代码若用 `cfg_if!` 编写会更清晰。然而下面这些条目是由 `std`
// 公开导出的，而外部工具在分析它们时可能会遇到困难，因为它们使用了一个
// 并未随 Rust 一同 vendored 并包含在工具链中的宏。
// 参见 https://github.com/rust-analyzer/rust-analyzer/issues/6038。

// 目前在某些平台上，那些被写入文档的“主模块（main modules）”无法编译
//（`libc` 中缺少某些东西，因为它是空的），因此这里干脆用一个空模块将其略去，
// 并加上 "unstable" 属性。

// darwin、unix、linux、wasi 与 windows 的处理方式略有不同。
#[cfg(all(
    doc,
    any(
        all(target_arch = "wasm32", not(target_os = "wasi")),
        all(target_vendor = "fortanix", target_env = "sgx")
    )
))]
#[unstable(issue = "none", feature = "std_internals")]
pub mod darwin {}
#[cfg(all(
    doc,
    any(
        all(target_arch = "wasm32", not(target_os = "wasi")),
        all(target_vendor = "fortanix", target_env = "sgx")
    )
))]
#[unstable(issue = "none", feature = "std_internals")]
pub mod unix {}
#[cfg(all(
    doc,
    any(
        all(target_arch = "wasm32", not(target_os = "wasi")),
        all(target_vendor = "fortanix", target_env = "sgx")
    )
))]
#[unstable(issue = "none", feature = "std_internals")]
pub mod linux {}
#[cfg(all(
    doc,
    any(
        all(target_arch = "wasm32", not(target_os = "wasi")),
        all(target_vendor = "fortanix", target_env = "sgx")
    )
))]
#[unstable(issue = "none", feature = "std_internals")]
pub mod wasi {}
#[cfg(all(
    doc,
    any(
        all(target_arch = "wasm32", not(target_os = "wasi")),
        all(target_vendor = "fortanix", target_env = "sgx")
    )
))]
#[unstable(issue = "none", feature = "std_internals")]
pub mod windows {}

// darwin
#[cfg(not(all(
    doc,
    any(
        all(target_arch = "wasm32", not(target_os = "wasi")),
        all(target_vendor = "fortanix", target_env = "sgx")
    )
)))]
#[cfg(any(target_vendor = "apple", doc))]
pub mod darwin;

// unix
#[cfg(not(all(
    doc,
    any(
        all(target_arch = "wasm32", not(target_os = "wasi")),
        all(target_vendor = "fortanix", target_env = "sgx")
    )
)))]
#[cfg(all(not(target_os = "hermit"), any(unix, doc)))]
pub mod unix;

// linux
#[cfg(not(all(
    doc,
    any(
        all(target_arch = "wasm32", not(target_os = "wasi")),
        all(target_vendor = "fortanix", target_env = "sgx")
    )
)))]
#[cfg(any(target_os = "linux", doc))]
pub mod linux;

// wasi
#[cfg(not(all(
    doc,
    any(
        all(target_arch = "wasm32", not(target_os = "wasi")),
        all(target_vendor = "fortanix", target_env = "sgx")
    )
)))]
#[cfg(any(target_os = "wasi", any(target_env = "p1", target_env = "p2"), doc))]
pub mod wasi;

#[cfg(any(all(target_os = "wasi", target_env = "p2"), doc))]
pub mod wasip2;

// windows
#[cfg(not(all(
    doc,
    any(
        all(target_arch = "wasm32", not(target_os = "wasi")),
        all(target_vendor = "fortanix", target_env = "sgx")
    )
)))]
#[cfg(any(windows, doc))]
pub mod windows;

// 其他平台。
#[cfg(target_os = "aix")]
pub mod aix;
#[cfg(target_os = "android")]
pub mod android;
#[cfg(target_os = "cygwin")]
pub mod cygwin;
#[cfg(target_os = "dragonfly")]
pub mod dragonfly;
#[cfg(target_os = "emscripten")]
pub mod emscripten;
#[cfg(target_os = "espidf")]
pub mod espidf;
#[cfg(all(target_vendor = "fortanix", target_env = "sgx"))]
pub mod fortanix_sgx;
#[cfg(target_os = "freebsd")]
pub mod freebsd;
#[cfg(target_os = "fuchsia")]
pub mod fuchsia;
#[cfg(target_os = "haiku")]
pub mod haiku;
#[cfg(target_os = "hermit")]
pub mod hermit;
#[cfg(target_os = "horizon")]
pub mod horizon;
#[cfg(target_os = "hurd")]
pub mod hurd;
#[cfg(target_os = "illumos")]
pub mod illumos;
#[cfg(target_os = "ios")]
pub mod ios;
#[cfg(target_os = "l4re")]
pub mod l4re;
#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "motor")]
pub mod motor;
#[cfg(target_os = "netbsd")]
pub mod netbsd;
#[cfg(target_os = "nto")]
pub mod nto;
#[cfg(target_os = "nuttx")]
pub mod nuttx;
#[cfg(target_os = "openbsd")]
pub mod openbsd;
#[cfg(target_os = "redox")]
pub mod redox;
#[cfg(target_os = "rtems")]
pub mod rtems;
#[cfg(target_os = "solaris")]
pub mod solaris;
#[cfg(target_os = "solid_asp3")]
pub mod solid;
#[cfg(target_os = "trusty")]
pub mod trusty;
#[cfg(target_os = "uefi")]
pub mod uefi;
#[cfg(target_os = "vita")]
pub mod vita;
#[cfg(target_os = "vxworks")]
pub mod vxworks;
#[cfg(target_os = "xous")]
pub mod xous;

#[cfg(any(
    unix,
    target_os = "hermit",
    target_os = "trusty",
    target_os = "wasi",
    target_os = "motor",
    doc
))]
pub mod fd;

#[cfg(any(target_os = "linux", target_os = "android", target_os = "cygwin", doc))]
mod net;
