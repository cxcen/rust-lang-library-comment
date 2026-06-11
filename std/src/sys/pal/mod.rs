//! PAL（platform abstraction layer，平台抽象层）包含各平台专有的抽象实现，
//! 用于支撑其他子模块中的功能特性，例如各种系统调用绑定等。

#![allow(missing_debug_implementations)]

cfg_select! {
    unix => {
        mod unix;
        pub use self::unix::*;
    }
    windows => {
        mod windows;
        pub use self::windows::*;
    }
    target_os = "solid_asp3" => {
        mod solid;
        pub use self::solid::*;
    }
    target_os = "hermit" => {
        mod hermit;
        pub use self::hermit::*;
    }
    target_os = "motor" => {
        mod motor;
        pub use self::motor::*;
    }
    target_os = "trusty" => {
        mod trusty;
        pub use self::trusty::*;
    }
    target_os = "vexos" => {
        mod vexos;
        pub use self::vexos::*;
    }
    target_os = "wasi" => {
        mod wasi;
        pub use self::wasi::*;
    }
    target_family = "wasm" => {
        mod wasm;
        pub use self::wasm::*;
    }
    target_os = "xous" => {
        mod xous;
        pub use self::xous::*;
    }
    target_os = "uefi" => {
        mod uefi;
        pub use self::uefi::*;
    }
    all(target_vendor = "fortanix", target_env = "sgx") => {
        mod sgx;
        pub use self::sgx::*;
    }
    target_os = "teeos" => {
        mod teeos;
        pub use self::teeos::*;
    }
    target_os = "zkvm" => {
        mod zkvm;
        pub use self::zkvm::*;
    }
    _ => {
        mod unsupported;
        pub use self::unsupported::*;
    }
}
