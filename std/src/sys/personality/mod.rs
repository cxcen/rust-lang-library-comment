//! 本模块包含 `eh_personality` lang item 的实现。
//!
//! 其实际实现高度依赖于 target，因为 Rust 会尽可能地使用本机（native）的
//! 栈展开（stack unwinding）机制。
//!
//! 即便使用 `-C panic=abort`，这个 personality 函数仍然是必需的，因为它被用来
//! 捕获来自 `extern "C-unwind"` 的外部异常（foreign exceptions）并将其转为 abort。
//!
//! 此外，ARM EHABI 在生成 backtrace 时会使用 personality 函数。

mod dwarf;

#[cfg(not(any(test, doctest)))]
cfg_select! {
    target_os = "emscripten" => {
        mod emcc;
    }
    any(target_env = "msvc", target_family = "wasm", target_os = "motor") => {
        // 编译器要求它存在（例如它是一个 lang item），但编译器实际上从不会调用它，
        // 因为始终被使用的 personality 函数是 __CxxFrameHandler3 (msvc) /
        // __gxx_wasm_personality_v0 (wasm)。因此这里只是一个会 abort 的桩（stub）。
        #[lang = "eh_personality"]
        fn rust_eh_personality() {
            core::intrinsics::abort()
        }
    }
    any(
        all(target_family = "windows", target_env = "gnu"),
        target_os = "psp",
        target_os = "xous",
        target_os = "solid_asp3",
        all(target_family = "unix", not(target_os = "espidf"), not(target_os = "l4re"), not(target_os = "nuttx")),
        all(target_vendor = "fortanix", target_env = "sgx"),
    ) => {
        mod gcc;
    }
    _ => {
        // 不支持栈展开（unwinding）的 target。
        // - os=none（“裸机 bare metal” target）
        // - os=uefi
        // - os=espidf
        // - os=hermit
        // - nvptx64-nvidia-cuda
        // - arch=avr
    }
}
